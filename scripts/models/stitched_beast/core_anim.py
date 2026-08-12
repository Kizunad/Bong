#!/usr/bin/env python3
"""异变缝合兽 —— 核心阶段动画：还没有腿的时候，这团肉怎么活着。

正典（《异兽三形考》§兽·噬）：缝合兽不是刷出来的成品。它先是一团核心，靠捡尸体
一件件把部件长进去——那头幼兽只有三条腿，第四条从野狗尸体上"借"来，花了约七日。
所以**无肢阶段是它必经的一段人生**，需要一整套自己的动作，不能拿有腿的步态凑。

## 蠕动：和步态同一条物理约束

没有腿的一团肉只能靠蠕动。约束和有腿时**完全一样**——着地的那一段在世界系里必须
静止，只是"脚"换成了身体的"锚段"。

双锚循环（蚯蚓那套）：

    相 A：后段锚地不动，前段向前伸长 d      → 身体拉长 d
    相 B：前段锚地不动，后段收上来 d        → 身体缩回原长

一个周期净前进 d。锚段世界静止是**硬约束**，前进量由此推出，不是画出来的。

导出的是循环动画，所以要把每周期 d 的净位移减掉（`ŵ = w + u·d`），首末帧才对得上；
自检反过来加回去，验世界系锚段位移为零。

体积近似守恒：拉长时变细（`scale_xy ∝ √(L₀/L)`），锚着的那段再额外鼓一点——鼓起来
才抓得住地，这也是蚯蚓看起来一节节鼓的原因。

## 为什么爬得比走慢

爬行速度 = d × f，d 受组织可拉伸量限制（~25% 体长），f 受收缩速率限制。算出来
约 0.2 格/s，而有腿步态 0.56-1.10 格/s（见 locomotion）。**没捡到腿之前它又慢又脆**
——这正是它必须去捡尸体的原因，进化压力是算出来的，不是设定出来的。

用法:
  python3 scripts/models/stitched_beast/core_anim.py
  python3 scripts/models/stitched_beast/core_anim.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import gen_core as G  # noqa: E402
from anim_rig import Pose, Rig, build_tracks, smooth, wrap, write_bbmodel, write_geckolib  # noqa: E402

MODEL = G.OUT
OUT_ANIM = G.OUT_DIR / "StitchedBeastCoreRig.bbmodel"
OUT_GECKO = HERE / "stitched_beast_core.animation.json"

# ---- 蠕动参数（全部有物理来源，不是手调的观感值）----
LOBE_SPAN = 21.0          # core_fore 与 core_hind 的 z 间距，即"体长"基准
STRETCH = 0.40            # 组织可拉伸比例。无骨软组织的保守值——水蛭能伸到两倍体长，
                          # 25% 试过：渲出来八帧几乎一模一样，读不出它在蠕动
CONTRACT_RATE = 6.0       # 组织收缩速率上限（px/s），决定一个相位要多久
CRAWL_D = LOBE_SPAN * STRETCH               # 每周期净前进量
CRAWL_HZ = CONTRACT_RATE / (2.0 * CRAWL_D)  # 两个相位各走 d/rate 秒

LOBES_MAIN = ("core_fore", "core_mid", "core_hind", "core_sag")
LOBES_LUMP = ("lump_l", "lump_dorsal", "nodule_r")
BUDS = tuple(f"bud_{n}" for n in C.sockets())
BUD_DORMANT = 0.10        # 未嫁接时芽的缩放。几何按满尺寸建，休眠态靠 scale 压下去
GRAFT_SOCKET = "limb_fl"  # core_graft 演示用的槽；运行时由服务端直接驱动任意 bud_<槽>


def dormant_buds(p: Pose, active: str | None = None) -> None:
    """把所有芽压到休眠尺寸；active 指定的那个留给调用方自己驱动。

    每条动画都得显式压——不压的话 17 个满尺寸的芽会让核心变成一只海胆。
    """
    for n in BUDS:
        if n != active:
            p[n].scale = [BUD_DORMANT] * 3


# ---------------------------------------------------------------- 曲线
def breathe(t: float, hz: float, phase: float, length: float) -> float:
    """整周期数的正弦，保证首末帧严格相等（循环接缝为零）。

    直接写 sin(2π·hz·t·length) 会在 hz·length 非整数时于接缝处炸出一个跳变——
    拟态灰烬蛛的触肢微颤就是这么翻的车（实测 7.27 单位跳变）。这里强制取整。
    """
    n = max(1, round(hz * length))
    return math.sin(2.0 * math.pi * (n * t + phase))


def crawl_world(u: float) -> tuple[float, float, float, float]:
    """蠕动周期相位 u∈[0,1) 处的（前段世界 z, 后段世界 z, 前段抓地, 后段抓地）。

    世界 z 单调不增（朝 -z 前进）。锚着的那一段在自己的相位内**严格常量**——这是
    不滑步的定义，自检直接按它断言。
    """
    d = CRAWL_D
    if u < 0.5:                       # 相 A：后段锚地，前段前伸
        s = smooth(u / 0.5)
        return -d * s, 0.0, 1.0 - s, 1.0
    s = smooth((u - 0.5) / 0.5)       # 相 B：前段锚地，后段跟上
    return -d, -d * s, 1.0, s


def anim_crawl(rig: Rig, t: float, length: float) -> Pose:
    """蠕动前进。"""
    p = Pose()
    wf, wh, gf, gh = crawl_world(wrap(t))
    drift = CRAWL_D * wrap(t)         # 减掉每周期净位移，动画才循环得上
    zf, zh = wf + drift, wh + drift

    # root 跟随体中点；前后段相对它反向偏移
    mid = 0.5 * (zf + zh)
    p["root"].pos = [0.0, 0.0, mid]
    p["core_fore"].pos = [0.0, 0.0, zf - mid]
    p["core_hind"].pos = [0.0, 0.0, zh - mid]

    # 体积近似守恒：拉长则变细
    body = LOBE_SPAN + (zh - zf)
    r = math.sqrt(LOBE_SPAN / max(body, 1e-6))
    for name, grip in (("core_fore", gf), ("core_hind", gh)):
        s = r * (1.0 + 0.10 * grip)   # 抓地的那段再鼓一点，才抓得住
        p[name].scale = [s, s, 1.0]
    p["core_mid"].scale = [r, r, 1.0]

    # 抓地 = 压下去摊开，自由段抬起来。这一层是"看得出在抓地"的主要来源：
    # 只做前后伸缩的话，侧视图上八帧几乎一模一样（实测）。
    grip = 0.5 * (gf + gh)
    p["core_sag"].scale = [1.0 + 0.16 * grip, 1.0 - 0.14 * grip, 1.0]
    p["core_sag"].pos = [0.0, -1.6 * grip, 0.0]
    p["core_fore"].pos = [0.0, 1.7 * (1.0 - gf), zf - mid]
    p["core_hind"].pos = [0.0, 1.7 * (1.0 - gh), zh - mid]

    # 赘生物被整体拖着晃，各自滞后
    for i, n in enumerate(LOBES_LUMP):
        p[n].pos = [0.0, 0.0, 1.1 * breathe(wrap(t), 1.0, 0.13 * i + 0.5, length)]
    dormant_buds(p)
    return p


def anim_idle(rig: Rig, t: float, length: float) -> Pose:
    """静止搏动。**各 lobe 各自的相位与频率**——整体同频呼吸会读成"一只动物在喘"，
    而这东西是几团来源不同的组织挤在一张皮里，各喘各的才对。"""
    p = Pose()
    for i, n in enumerate(LOBES_MAIN):
        a = 0.030 + 0.012 * (i % 2)
        s = 1.0 + a * breathe(t, 0.5 + 0.13 * i, 0.21 * i, length)
        p[n].scale = [s, s, s]
    for i, n in enumerate(LOBES_LUMP):
        # 赘生物抽得更快更浅：它们不是这具身体的一部分，节律对不上
        s = 1.0 + 0.045 * breathe(t, 1.4 + 0.31 * i, 0.37 * i, length)
        p[n].scale = [s, s, s]
    p["root"].pos = [0.0, 0.7 * breathe(t, 0.5, 0.0, length), 0.0]
    dormant_buds(p)
    return p


LUNGE_WINDUP = 0.72       # 蓄力占整条动画的比例；剩下的是爆发段


def anim_lunge(rig: Rig, t: float, length: float) -> Pose:
    """扑向尸体：缓慢积蓄 → 一瞬弹出。

    蓄与放的**速度比**是恐惧的唯一来源（拟态灰烬蛛同则：突刺必须快过蓄力 2.5 倍以上）。
    初版蓄 0.62 / 放 0.38、位移又小，速度比只有 1.7×，渲出来八帧几乎看不出在干什么。
    这版把时间压成 0.72/0.28 并把位移拉大，自检直接断言速度比。

    蓄力不只是后退，更是**整体压缩**：前段往回缩、后段往前挤、横向鼓出去——一团肉
    没有骨头可绷，蓄力只能靠把自己挤成一坨。放出去时反过来抻长抻细。
    """
    p = Pose()
    if t < LUNGE_WINDUP:
        s = smooth(t / LUNGE_WINDUP)
        p["root"].pos = [0.0, -3.5 * s, 5.0 * s]                 # 压低 + 后缩
        p["core_fore"].pos = [0.0, 0.0, 4.2 * s]                 # 前段缩回来
        p["core_hind"].pos = [0.0, 0.0, -2.4 * s]                # 后段挤上来
        b = 1.0 + 0.22 * s                                       # 横向鼓出
        p["core_fore"].scale = [b, b, 1.0 - 0.26 * s]
        p["core_mid"].scale = [1.0 + 0.16 * s, 1.0 + 0.12 * s, 1.0 - 0.14 * s]
    else:
        s = smooth((t - LUNGE_WINDUP) / (1.0 - LUNGE_WINDUP))
        p["root"].pos = [0.0, -3.5 + 6.0 * s, 5.0 - 21.0 * s]    # 弹出
        p["core_fore"].pos = [0.0, 0.0, 4.2 - 11.4 * s]          # 前段甩到最前
        p["core_hind"].pos = [0.0, 0.0, -2.4 + 3.6 * s]
        b = 1.22 - 0.40 * s                                      # 抻长则变细
        p["core_fore"].scale = [b, b, 0.74 + 0.62 * s]
        p["core_mid"].scale = [1.16 - 0.26 * s, 1.12 - 0.20 * s, 0.86 + 0.30 * s]
    dormant_buds(p)
    return p


def anim_engulf(rig: Rig, t: float, length: float) -> Pose:
    """包裹：前段裂开、罩住、合拢并把东西压进体内。

    没有嘴——正典里核心没有面部器官。所以"吃"只能是**整段前体裂开再合上**，
    这比长一张嘴更难看，也更对。
    """
    p = Pose()
    # 三段：张开罩住 → 猛然合拢 → 把东西挤进主体。初版三段全叠在一起、幅度又小，
    # 渲出来只是前段轻微鼓胀，读成"在喘气"而不是"裂开吞下去"。
    op = smooth(np.clip(t / 0.40, 0.0, 1.0))                     # 张开
    cl = smooth(np.clip((t - 0.42) / 0.28, 0.0, 1.0))            # 合拢
    sw = smooth(np.clip((t - 0.68) / 0.32, 0.0, 1.0))            # 咽下（质量迁回主体）
    p["core_fore"].scale = [1.0 + 0.58 * op - 0.74 * cl + 0.14 * sw,
                            1.0 + 0.48 * op - 0.66 * cl + 0.16 * sw,
                            1.0 - 0.30 * op + 0.62 * cl - 0.30 * sw]
    p["core_fore"].pos = [0.0, -3.2 * op + 3.6 * cl - 0.4 * sw,
                          -5.0 * op + 6.2 * cl + 1.4 * sw]
    # 咽下时主体鼓起来：吃进去的东西得有个去处，凭空消失是吞质量
    p["core_mid"].scale = [1.0 + 0.06 * cl + 0.20 * sw,
                           1.0 + 0.05 * cl + 0.17 * sw,
                           1.0 + 0.04 * cl + 0.12 * sw]
    p["root"].pos = [0.0, -2.0 * op + 1.4 * cl, -1.6 * op + 1.0 * cl]
    dormant_buds(p)
    return p


def anim_graft(rig: Rig, t: float, length: float) -> Pose:
    """嫁接：芽在挂载点上鼓起来。

    正典里这个过程要**七日**。动画只能给一段压缩表现，所以重点不是"长大"而是
    "长得不顺"：鼓胀是阶梯式的（三次推进夹两次停滞），中间还抽搐——那不是一条
    被神经支配的肢体，是一团正在被强行编织进来的别人的组织。
    """
    p = Pose()
    steps = (0.0, 0.34, 0.38, 0.72, 0.76, 1.0)       # 推进/停滞交替
    v = float(np.interp(t, steps, (0.0, 0.45, 0.45, 0.82, 0.82, 1.0)))
    tgt = f"bud_{GRAFT_SOCKET}"
    dormant_buds(p, active=tgt)
    p[tgt].scale = [BUD_DORMANT + (1.0 - BUD_DORMANT) * v] * 3
    # 抽搐：高频小幅，且只在推进段有——停滞时它是死的，更瘆人
    moving = 1.0 if t < 0.34 or 0.38 < t < 0.72 or t > 0.76 else 0.0
    p[tgt].rot = [9.0 * moving * math.sin(t * 61.0),
                  6.0 * moving * math.sin(t * 47.0 + 1.1), 0.0]
    p["core_mid"].scale = [1.0 + 0.04 * v] * 3
    return p


def anim_hurt(rig: Rig, t: float, length: float) -> Pose:
    """受击：整体一震 + 被打的那侧塌陷回弹。软组织没有骨架撑着，凹得比硬壳生物深。"""
    p = Pose()
    decay = math.exp(-4.2 * t)
    p["root"].pos = [1.9 * decay * math.sin(t * 44.0), 0.9 * decay * math.sin(t * 31.0), 0.0]
    dent = decay * (1.0 - math.exp(-14.0 * t))
    p["lump_l"].scale = [1.0 - 0.30 * dent, 1.0 - 0.22 * dent, 1.0 - 0.26 * dent]
    p["core_mid"].scale = [1.0 - 0.10 * dent, 1.0 + 0.07 * dent, 1.0 - 0.08 * dent]
    dormant_buds(p)
    for n in BUDS:
        p[n].rot = [16.0 * decay * math.sin(t * 53.0), 0.0, 0.0]
    return p


def anim_death(rig: Rig, t: float, length: float) -> Pose:
    """死：**逐 lobe 依次泄气**，不是整体倒下。

    每团组织本来就各活各的，死也该各死各的：赘生物先瘪（它们本来就接得最勉强），
    主体最后塌。整体只往下沉，不翻倒——一团肉没有"倒"这个姿势。
    """
    p = Pose()
    order = (("nodule_r", 0.00), ("lump_dorsal", 0.08), ("lump_l", 0.16),
             ("core_fore", 0.30), ("core_hind", 0.42), ("core_mid", 0.55), ("core_sag", 0.62))
    for n, t0 in order:
        s = smooth(np.clip((t - t0) / 0.30, 0.0, 1.0))
        p[n].scale = [1.0 + 0.16 * s, 1.0 - 0.55 * s, 1.0 + 0.14 * s]
    sink = smooth(np.clip((t - 0.25) / 0.6, 0.0, 1.0))
    p["root"].pos = [0.0, -5.4 * sink, 0.0]
    for n in BUDS:
        s = smooth(np.clip((t - 0.05) / 0.35, 0.0, 1.0))
        p[n].scale = [BUD_DORMANT * (1.0 - 0.75 * s)] * 3
        p[n].rot = [42.0 * s, 0.0, 0.0]
    return p


# (名字, 时长秒, 是否循环, 采样数, 函数)
ANIMS: dict[str, tuple[float, bool, int, object]] = {
    "core_idle": (5.0, True, 40, anim_idle),
    "core_crawl": (1.0 / CRAWL_HZ, True, 36, anim_crawl),
    "core_lunge": (0.75, False, 24, anim_lunge),
    "core_engulf": (1.40, False, 30, anim_engulf),
    "core_graft": (3.00, False, 44, anim_graft),
    "core_hurt": (0.45, False, 20, anim_hurt),
    "core_death": (2.20, False, 36, anim_death),
}


def sample(rig: Rig, name: str, t: float) -> Pose:
    length, _loop, _n, fn = ANIMS[name]
    return fn(rig, t, length)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if not MODEL.exists():
        print(f"缺 {MODEL}，先跑 gen_core.py")
        return 1
    rig = Rig(MODEL)

    print(f"蠕动：每周期前进 {CRAWL_D:.1f}px  {CRAWL_HZ:.2f}Hz  "
          f"= {CRAWL_D * CRAWL_HZ / 16:.2f} 格/s（对比有腿步态 0.56-1.10 格/s）")
    if args.list:
        for n, (ln, loop, ns, _f) in ANIMS.items():
            print(f"  {n:<12} {ln:>5.2f}s  {'循环' if loop else '单次':<4} {ns:>3} 采样")
        return 0

    entries = []
    for name, (length, loop, n, _fn) in ANIMS.items():
        tracks = build_tracks(rig, lambda t, nm=name: sample(rig, nm, t), length, loop, n)
        entries.append((name, length, loop, tracks))
        kf = sum(len(v) for c in tracks.values() for v in c.values())
        print(f"  {name:<12} {length:>5.2f}s {'循环' if loop else '单次'}  "
              f"{len(tracks):>2} 骨  {kf:>4} 关键帧")
    write_bbmodel(MODEL, OUT_ANIM, "StitchedBeastCoreRig", entries)
    write_geckolib(OUT_GECKO, "bong", "stitched_beast_core", entries)
    print(f"→ {OUT_ANIM}\n→ {OUT_GECKO}（参考用，正经导出走 bbmodel_to_geckolib.py）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
