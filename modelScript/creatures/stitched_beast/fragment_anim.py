#!/usr/bin/env python3
"""异变缝合兽 —— 碎片动画：撕下来的那块怎么活着跑掉。

**约束和母体完全一样，只是身体换了。** 蠕动仍然是双锚循环、锚段仍然必须在世界系里
静止、体积仍然守恒；只是"前后两团肉"换成了碎片重新分出来的前后两节（见 fragment）。
这不是把核心动画复制一份改骨名——那样做，碎片会以母体的行程和母体的速度爬。

三个数全部按碎片自己的几何重算：

    行程 d = 锚段间距 × STRETCH
    频率   = STRAIN_RATE / (2·STRETCH)      —— 与体长**无关**，所以周期和母体一样
    速度 v = d × 频率 = 应变率 × 锚段间距 / 2   —— 正比于体长，所以它爬得比母体慢

于是"小碎片逃得慢"是算出来的：core_fore 那块锚段只隔 5.0px，母体隔 23px，它的逃窜
速度只有母体的四分之一。它跑不掉——所以它得躲、得等、得去找尸体。

芽的那一套（乱抽、逐槽嫁接）直接复用核心的运动学，只是把槽换成碎片带得走的那几个：
同一套速度上限、同一套痉挛波形，力臂换成碎片自己的芽。

用法:
  python3 modelScript/creatures/stitched_beast/fragment_anim.py
  python3 modelScript/creatures/stitched_beast/fragment_anim.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from functools import lru_cache
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import core_anim as A  # noqa: E402
import fragment as FR  # noqa: E402
import gen_fragment as GF  # noqa: E402
from bbmodel_maker.rig.anim_rig import Pose, Rig, build_tracks, euler, euler_of, smooth, wrap  # noqa: E402
from bbmodel_maker.rig.anim_rig import write_bbmodel, write_geckolib  # noqa: E402

LOBES = FR.default_lobes()
MODEL = GF.OUT
OUT_ANIM = GF.OUT_DIR / "StitchedBeastShardRig.bbmodel"
OUT_GECKO = HERE / "stitched_beast_shard.animation.json"

GEOM = FR.geom(LOBES)
SPAN = GEOM.span                              # 锚段间距 —— 这块肉的"体长"
CRAWL_D = SPAN * A.STRETCH                    # 每周期净前进
CRAWL_HZ = A.STRAIN_RATE / (2.0 * A.STRETCH)  # 与体长无关：周期和母体一样
GRIP_LIFT = 1.0                               # 自由段抬起的高度（px）。碎片只有母体
                                              # 三分之一高，照抄母体的 1.7 会读成在跳


@lru_cache(maxsize=1)
def axis() -> tuple[int, float]:
    """伸缩轴：`forward` 在模型轴上的主分量。返回（轴序号, 符号）。

    骨骼缩放只能沿模型轴，而爬行方向是从头槽法向（或背离母体的方向）推出来的，一般
    不正好落在轴上。所以伸缩沿主轴做、位移也沿主轴做——两者一致就不会撕开。对默认
    碎片 forward=(-0.04, 0, -1.00)，主轴就是 −z，误差可以忽略；自检里直接量这个对齐度。
    """
    i = int(np.argmax(np.abs(GEOM.forward)))
    return i, float(np.sign(GEOM.forward[i]) or 1.0)


def alignment() -> float:
    """主轴与真实爬行方向的余弦。1.0 = 正好对齐。"""
    i, s = axis()
    return abs(float(GEOM.forward[i]))


def _seg(name: str) -> str:
    return {"fore": GEOM.fore, "mid": GEOM.mid, "hind": GEOM.hind}[name]


def anim_crawl(rig: Rig, t: float, length: float) -> Pose:
    """蠕动。与 core_anim.anim_crawl 同一套约束与同一套父缩放补偿，行程换成自己的。"""
    p = Pose()
    u = wrap(t)
    i, sgn = axis()
    wf, wh, gf, gh = A.crawl_world(u, CRAWL_D)
    drift = CRAWL_D * u                       # 减掉每周期净位移，动画才循环得上
    zf, zh = wf + drift, wh + drift
    mid = 0.5 * (zf + zh)

    body = SPAN + (zh - zf)
    k = body / SPAN                           # 沿伸缩轴的拉伸比 ≥1
    r = 1.0 / math.sqrt(k)                    # 另两轴收细，体积守恒（r²·k = 1）

    S = np.ones(3) * r
    S[i] = k
    fore, hind, mid_b = _seg("fore"), _seg("hind"), _seg("mid")
    p[mid_b].scale = list(S)
    o_mid = rig.bones[mid_b].origin

    def child(name: str, along: float, lift: float, grip: float = 0.0) -> None:
        """子骨的位移要补偿父骨缩放**连枢轴一起放大**那一项：
        pos = S⁻¹·(Δ + off) − Δ（同 core_anim.anim_crawl 的推导，那儿踩过一次）。

        `along` 来自 `crawl_world`，那是**核心坐标系下的 z 坐标**（核心朝 −z 走，所以
        前进时它变负）。换到任意主轴上要取负号再乘朝向号：off = −along·sgn。漏掉这个
        负号，碎片会沿自己的 forward **倒着爬**——锚段不滑步、体积也守恒，全部自检
        照样绿，只有方向是反的。自检里单列一条量行进方向与 forward 的点积。
        """
        off = np.zeros(3)
        off[i] = -along * sgn
        off[1] += lift
        delta = rig.bones[name].origin - o_mid
        p[name].pos = list((delta + off) / S - delta)
        # 逐轴写**世界**缩放再除以父缩放，别拿一个标量去凑：
        #   伸缩轴   1.0      —— 只有中段负责桥接，锚段自己不拉长
        #   另两轴   r        —— 跟着整体收细，body 才是均匀的一条
        #   抓地     横向 ×(1+grip)、纵向 ÷(1+grip) —— 压扁，不是鼓胀
        #
        # 母体那版三个轴一起乘同一个 bulge，纵向也跟着涨；母体腹底离地 13.7px 无所谓，
        # 碎片贴着地，多出来的 10% 直接扎地 0.65px。第一次改的时候把标量写成 1/bulge，
        # 而 bulge 里本来就含着 r，结果纵向world 缩放变成 √k —— 渲出来整只往上抽长
        # 成一根柱子（实测）。所以这里逐轴写清楚。
        gf_ = 1.0 + 0.10 * grip
        w = np.ones(3) * r
        w[i] = 1.0
        for j in (0, 2):
            if j != i:
                w[j] *= gf_
        w[1] /= gf_
        p[name].scale = list(w / S)

    child(fore, zf - mid, GRIP_LIFT * (1.0 - gf), gf)
    child(hind, zh - mid, GRIP_LIFT * (1.0 - gh), gh)
    p["root"].pos = [0.0, 0.0, 0.0]
    p["root"].pos[i] = -mid * sgn
    dormant(p)
    return p


def dormant(p: Pose, active: str | None = None) -> None:
    for n in GEOM.sockets:
        if n != active:
            p[f"bud_{n}"].scale = [A.BUD_DORMANT] * 3


def anim_idle(rig: Rig, t: float, length: float) -> Pose:
    """静止搏动。三节各自的相位——它是一块肉，不是一台泵，各节不同步。"""
    p = Pose()
    for i, b in enumerate((GEOM.hind, GEOM.mid, GEOM.fore)):
        s = 1.0 + (0.034 + 0.010 * (i % 2)) * A.breathe(t, 0.5 + 0.17 * i, 0.23 * i, length)
        p[b].scale = [s, s, s]
    # 只往**上**浮：它肚子贴着地，往下没有余量。母体那版是绕静止位上下摆，照抄过来
    # 直接扎地 0.67px（实测）。贴地生物的呼吸本来也只能顶起来，压不下去。
    p["root"].pos = [0.0, 0.35 * (1.0 + A.breathe(t, 0.5, 0.0, length)), 0.0]
    dormant(p)
    return p


@lru_cache(maxsize=1)
def thrash_scale() -> float:
    """碎片乱抽时的芽缩放：组织预算按质量比缩（见 fragment.FragGeom.growth）。"""
    budget = C.graft_budget() * GEOM.mass / sum(C.lobe_mass().values())
    return C.spread_scale(tuple(GEOM.sockets.values()), budget)


def anim_thrash(rig: Rig, t: float, length: float) -> Pose:
    """乱抽。同一套速度上限、同一套痉挛波形，力臂换成碎片自己的芽。

    碎片的预算更小 ⇒ 茬更短 ⇒ 力臂更小 ⇒ **抽得比母体还快**。一小块肉比整只兽抖得
    更急，这是同一条 f ∝ 1/L 推出来的，不是为了区分而调的。
    """
    p = Pose()
    sc = thrash_scale()
    for n in GEOM.sockets:
        p[f"bud_{n}"].scale = [sc] * 3
        A.tendril_pose(p, n, wrap(t), length, A.joint_flicks(n, sc, length))
    gained = sum(C.bud_tissue(s, 1.0) for s in GEOM.sockets.values()) * sc ** 3
    shrink = (1.0 - gained / (GEOM.mass * C.VOX ** 3)) ** (1.0 / 3.0)
    for b in (GEOM.fore, GEOM.mid, GEOM.hind):
        p[b].scale = [shrink, shrink, shrink]
    # 底下那几条茬往下抽时会顶到地——碎片肚子本来就贴着地。于是它被自己的茬**撑着
    # 一颠一颠**，这不是穿模钳制，是同一条"拿地面撑自己"（见 ground_lift）。母体腹底
    # 离地 13.7px 没有这个问题，所以只有碎片这条要抬。
    p["root"].pos = [0.0, ground_lift(rig, p, list(GEOM.sockets)), 0.0]
    return p


def ground_lift(rig: Rig, pose: Pose, names) -> float:
    """朝下长的那条芽把身体顶起来多少（px）。

    碎片肚子贴着地，而 `limb_fl` 这类槽的法向朝下——芽一长出来、或者乱抽时往下弯，
    就会插进地里（嫁接实测最深 −4.9px，乱抽 −0.69px）。这不是要钳掉的穿模，是
    **拿地面撑自己**：朝下的那条顶到地，身体就得抬起来。运动层早就把这件事当成解出来的量（`locomotion.solve_ride_height`），
    这里是它在无肢阶段的边界情形——只有一条腿时，抬起来的高度正好等于那条腿伸到
    地面以下的深度。

    直接对**骨链做正解**再取最低点，不再闭式估：芽现在是四节各自旋转的链，闭式解要
    把四层变换重新推一遍，而这里本来就有 rig。root 的平移是最后叠加的纯位移，所以
    先按 root=0 量、再把量出来的值写回 root，两者不打架。
    """
    W = rig.world(pose)
    lo = 1e9
    for name in ([names] if isinstance(names, str) else names):
        for bone in A.tendril(name)[0]:
            pts = rig.bone_points(bone)
            if len(pts):
                lo = min(lo, float((pts @ W[bone][:3, :3].T + W[bone][:3, 3])[:, 1].min()))
    return max(0.0, -lo)


def anim_graft_at(name: str, rig: Rig, t: float, length: float) -> Pose:
    """碎片上某个槽的嫁接。和母体同一条曲线，外加"朝下长就把自己撑起来"。"""
    p = Pose()
    j = C._noise(name, "graft") * 0.10 - 0.05
    steps = (0.0, 0.34 + j, 0.38 + j, 0.72 + j, 0.76 + j, 1.0)
    v = float(np.interp(t, steps, (0.0, 0.45, 0.45, 0.82, 0.82, 1.0)))
    tgt = f"bud_{name}"
    sc = A.BUD_DORMANT + (1.0 - A.BUD_DORMANT) * v
    dormant(p, active=name)
    p[tgt].scale = [sc] * 3
    moving = 1.0 if (t < steps[1] or steps[2] < t < steps[3] or t > steps[4]) else 0.0
    A.tendril_pose(p, name, wrap(t), length, A.joint_flicks(name, 1.0, length),
                   gain=A.GRAFT_TWITCH * moving, sway=0.55)
    p[GEOM.mid].scale = [1.0 + 0.04 * v] * 3
    p["root"].pos = [0.0, ground_lift(rig, p, name), 0.0]
    return p


def anim_hurt(rig: Rig, t: float, length: float) -> Pose:
    """受击：整块一震，中段塌陷回弹。"""
    p = Pose()
    decay = math.exp(-4.6 * t)
    # 纵向只准弹起不准下陷（贴地，见 anim_idle）
    p["root"].pos = [1.4 * decay * math.sin(t * 46.0),
                     0.7 * decay * abs(math.sin(t * 33.0)), 0.0]
    dent = decay * (1.0 - math.exp(-15.0 * t))
    p[GEOM.mid].scale = [1.0 - 0.16 * dent, 1.0 + 0.10 * dent, 1.0 - 0.12 * dent]
    dormant(p)
    for n in GEOM.sockets:
        p[f"bud_{n}"].rot = [18.0 * decay * math.sin(t * 55.0), 0.0, 0.0]
    return p


def anim_death(rig: Rig, t: float, length: float) -> Pose:
    """死：逐节泄气摊平。前段先塌——那是离创面最远、最先失去供给的一端。

    不下沉太多：它本来就贴着地，能塌的只有自己那点厚度。
    """
    p = Pose()
    for b, t0 in ((GEOM.fore, 0.00), (GEOM.hind, 0.14), (GEOM.mid, 0.30)):
        s = smooth(float(np.clip((t - t0) / 0.34, 0.0, 1.0)))
        p[b].scale = [1.0 + 0.20 * s, 1.0 - 0.58 * s, 1.0 + 0.18 * s]
    for n in GEOM.sockets:
        s = smooth(float(np.clip((t - 0.05) / 0.35, 0.0, 1.0)))
        p[f"bud_{n}"].scale = [A.BUD_DORMANT * (1.0 - 0.8 * s)] * 3
        p[f"bud_{n}"].rot = [46.0 * s, 0.0, 0.0]
    return p


def _thrash_samples(length: float) -> int:
    sc = thrash_scale()
    top = max(k for n in GEOM.sockets for k, _p in A.joint_flicks(n, sc, length))
    return int(math.ceil(top / A.FLICK_ATTACK * A.FLICK_KEYS))


ANIMS: dict[str, tuple[float, bool, int, object]] = {
    "shard_idle": (4.4, True, 36, anim_idle),
    "shard_crawl": (1.0 / CRAWL_HZ, True, 36, anim_crawl),
    "shard_hurt": (0.40, False, 20, anim_hurt),
    "shard_death": (1.70, False, 32, anim_death),
}
ANIMS["shard_thrash"] = (A.THRASH_LEN, True, _thrash_samples(A.THRASH_LEN), anim_thrash)
for _n in GEOM.sockets:
    ANIMS[f"shard_graft_{_n}"] = (A.graft_length(_n), False, 44,
                                  lambda rig, t, ln, nm=_n: anim_graft_at(nm, rig, t, ln))


def sample(rig: Rig, name: str, t: float) -> Pose:
    length, _loop, _n, fn = ANIMS[name]
    return fn(rig, t, length)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()
    if not MODEL.exists():
        print(f"缺 {MODEL}，先跑 gen_fragment.py")
        return 1
    rig = Rig(MODEL)

    print(f"碎片 {'+'.join(LOBES)}：锚段间距 {SPAN:.1f}px（母体 {A.LOBE_SPAN:.1f}px）")
    print(f"蠕动：每周期前进 {CRAWL_D:.1f}px  {CRAWL_HZ:.2f}Hz = "
          f"{CRAWL_D * CRAWL_HZ / 16:.3f} 格/s"
          f"（母体 {A.CRAWL_D * A.CRAWL_HZ / 16:.3f} 格/s）")
    print(f"伸缩主轴 {'xyz'[axis()[0]]}{'+' if axis()[1] > 0 else '-'}，"
          f"与爬行方向对齐度 {alignment():.3f}")
    if args.list:
        for n, (ln, loop, ns, _f) in ANIMS.items():
            print(f"  {n:<24} {ln:>5.2f}s  {'循环' if loop else '单次':<4} {ns:>3} 采样")
        return 0

    entries = []
    for name, (length, loop, n, _fn) in ANIMS.items():
        tracks = build_tracks(rig, lambda t, nm=name: sample(rig, nm, t), length, loop, n)
        entries.append((name, length, loop, tracks))
        kf = sum(len(v) for c in tracks.values() for v in c.values())
        print(f"  {name:<24} {length:>5.2f}s {'循环' if loop else '单次'}  "
              f"{len(tracks):>2} 骨  {kf:>4} 关键帧")
    write_bbmodel(MODEL, OUT_ANIM, "StitchedBeastShardRig", entries)
    write_geckolib(OUT_GECKO, "bong", "stitched_beast_shard", entries)
    print(f"→ {OUT_ANIM}\n→ {OUT_GECKO}（参考用）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
