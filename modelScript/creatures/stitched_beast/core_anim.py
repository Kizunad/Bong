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
  python3 modelScript/creatures/stitched_beast/core_anim.py
  python3 modelScript/creatures/stitched_beast/core_anim.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import gen_core as G  # noqa: E402
from functools import lru_cache  # noqa: E402

from bbmodel_maker.rig.anim_rig import (Pose, Rig, build_tracks, euler_of, smooth, wrap,  # noqa: E402
                      write_bbmodel, write_geckolib)

MODEL = G.OUT
OUT_ANIM = G.OUT_DIR / "StitchedBeastCoreRig.bbmodel"
OUT_GECKO = HERE / "stitched_beast_core.animation.json"

# ---- 蠕动参数（全部有物理来源，不是手调的观感值）----
LOBE_SPAN = 21.0          # core_fore 与 core_hind 的 z 间距，即"体长"基准
STRETCH = 0.40            # 组织可拉伸比例。无骨软组织的保守值——水蛭能伸到两倍体长，
                          # 25% 试过：渲出来八帧几乎一模一样，读不出它在蠕动
# 尺寸无关的量是**应变率**（每秒收缩掉体长的几分之几），不是绝对收缩速度。
# 绝对速度 rate = STRAIN_RATE × 体长，于是爬行速度 v = rate/2 ∝ **体长**：
# 大块爬得快、小碎片爬得慢。写成固定 px/s 的话每块碎片都以同一速度逃窜，
# 「不同部位不同逃法」就只剩方向不同了（见 fission）。
STRAIN_RATE = 0.286       # 1/s；对整只兽 = 6.0 px/s ÷ 21 px 体长
CONTRACT_RATE = STRAIN_RATE * LOBE_SPAN     # 整只兽的绝对收缩速度（px/s）
CRAWL_D = LOBE_SPAN * STRETCH               # 每周期净前进量
CRAWL_HZ = CONTRACT_RATE / (2.0 * CRAWL_D)  # 两个相位各走 d/rate 秒


def crawl_speed(span: float) -> float:
    """给定体长的蠕动速度（px/s）。v = 应变率 × 体长 / 2。"""
    return STRAIN_RATE * span / 2.0

LOBES_MAIN = ("core_fore", "core_mid", "core_hind", "core_sag")
LOBES_LUMP = ("lump_l", "lump_dorsal", "nodule_r")
BUDS = tuple(f"bud_{n}" for n in C.sockets())
BUD_DORMANT = 0.10        # 未嫁接时芽的缩放。几何按满尺寸建，休眠态靠 scale 压下去
GRAFT_SOCKET = "limb_fl"  # 组织合成速率的锚定槽（见 graft_length）
GRAFT_REF_LEN = 3.00      # 锚定槽长满所需秒数；其余槽按用料等比推
GRAFT_MIN = 0.50          # 嫁接时长下限：五段推进/停滞 × 每段至少两 tick


def place_world(rig: Rig, pose: Pose, name: str, offset) -> None:
    """把 name 的枢轴放到「静止位置 + offset」的**世界**位置，自动补偿父链。

    骨骼是树，而父骨的局部变换里既有平移也有**旋转和缩放**——子骨写的 `pos` 会被父骨
    的 R·S 一起作用。所以「子骨位移 = 目标 − 父骨位移」这种加法补偿是错的：只在父骨
    纯平移时成立。缝合兽的爆体里父骨同时在鼓胀（scale 1.2）和翻滚，实测 nodule_r 被
    父链放大到 −32px，钳制怎么调都拦不住。

    正解不做近似，直接用父骨**完整的世界矩阵**反解：

        W_parent · (o + pos) = o_world_rest + offset   ⇒   pos = W_parent⁻¹·目标 − o

    调用前必须已经设好本骨的 rot（旋转绕自身枢轴，不影响枢轴位置）与所有祖先的通道——
    所以要按父先于子的顺序调。同一坑在蠕动的中段桥接里也踩过一次（见 anim_crawl）。
    """
    b = rig.bones[name]
    rest = rig.joint(name, rig.world(Pose()))
    W = rig.world(pose)
    parent = b.parent
    P = W[parent] if parent else np.eye(4)
    target = np.append(rest + np.asarray(offset, float), 1.0)
    pose[name].pos = list((np.linalg.inv(P) @ target)[:3] - b.origin)


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


def crawl_world(u: float, d: float = CRAWL_D) -> tuple[float, float, float, float]:
    """蠕动周期相位 u∈[0,1) 处的（前段世界 z, 后段世界 z, 前段抓地, 后段抓地）。

    世界 z 单调不增（朝 -z 前进）。锚着的那一段在自己的相位内**严格常量**——这是
    不滑步的定义，自检直接按它断言。

    `d` 是每周期净前进量。碎片用自己的锚段间距算出来的那个（见 fragment_anim）——
    同一条相位曲线，换一个行程，因为约束是同一条。
    """
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

    mid = 0.5 * (zf + zh)
    body = LOBE_SPAN + (zh - zf)
    k = body / LOBE_SPAN                       # 纵向拉伸比 ≥1
    r = 1.0 / math.sqrt(k)                     # 横向收细，体积守恒（r²·k = 1）

    # **中段必须跟着沿 z 伸长**去桥接前后两团。round 2 把 core_mid 的 z 缩放留在 1.0，
    # 前后段一分开，原本刻出来的凹槽就被拉宽成可见裂缝（3/4 视角实测）。
    p["core_mid"].scale = [r, r, k]

    # core_mid 是前后段与赘生物的父骨，它的缩放会连**子骨枢轴**一起推开：子骨枢轴
    # 相对父枢轴的距离被乘上 k。只补 pos 不补枢轴那一项是不够的——core_fore 枢轴离
    # core_mid 枢轴 10.5px，k=1.4 时凭空多出 4.2px，自检实测锚段位移 4.19px（蹭地）。
    #
    # 逐骨解：设 Δ = 子枢轴 − 父枢轴，父缩放 S，要让子枢轴的世界位置落在 o_child+off，
    # 需 S·(Δ + pos) = Δ + off ⇒ **pos = S⁻¹·(Δ + off) − Δ**。
    # 子骨自身缩放同理取 S⁻¹·目标世界缩放。
    S = np.array([r, r, k])
    o_mid = rig.bones["core_mid"].origin

    def child(name: str, off, own_xy: float = 1.0, own_z: float = 1.0) -> None:
        delta = rig.bones[name].origin - o_mid
        p[name].pos = list((delta + np.array(off, float)) / S - delta)
        p[name].scale = [own_xy / r, own_xy / r, own_z / k]

    grip = 0.5 * (gf + gh)
    # 抓地 = 压下去摊开、自由段抬起来。这一层是"看得出在抓地"的主要来源：
    # 只做前后伸缩的话侧视图上八帧几乎一模一样（实测）。
    child("core_fore", (0.0, 1.7 * (1.0 - gf), zf - mid), r * (1.0 + 0.10 * gf), 1.0)
    child("core_hind", (0.0, 1.7 * (1.0 - gh), zh - mid), r * (1.0 + 0.10 * gh), 1.0)
    child("core_sag", (0.0, -1.6 * grip, 0.0), 1.0, 1.0)
    p["core_sag"].scale[0] *= 1.0 + 0.16 * grip
    p["core_sag"].scale[1] *= 1.0 - 0.14 * grip

    # root 跟随体中点
    p["root"].pos = [0.0, 0.0, mid]

    # 赘生物被整体拖着晃，各自滞后；同样补偿父缩放，免得跟着抻变形
    for i, n in enumerate(LOBES_LUMP):
        child(n, (0.0, 0.0, 1.1 * breathe(wrap(t), 1.0, 0.13 * i + 0.5, length)))
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


# ---------------------------------------------------------------- 芽的运动学
#
# 一条芽能动多快，由**整只兽做得到的最快速度**封顶：没有哪个部位能比它扑击时更快。
# 扑击的爆发段是全模型速度的上界（见 anim_lunge），于是
#
#     尖端线速度 = 摆角 × 力臂 ≤ V_max     ⇒     频率 ≤ V_max·ATTACK / (摆角 × 力臂)
#
# 力臂就是那条芽伸出去多长。**短茬甩得快、长条甩得慢**——和肢体的复摆律
# （locomotion.natural_hz，f ∝ 1/√L）是同一件事的两种极限，这里是被速度封顶而不是
# 被重力回复，所以指数是 1/L 不是 1/√L。
#
# **但频率并不散**，这点得说清楚：17 个挂载点的 girth 大多挤在 3.2–4.0，力臂跟着挤在
# 4.4–5.7px，算出来的次数因此集中在 5 上下，只有 `vest_dr`（girth 1.40）是个例外。
# 拉长循环时长提高整数分辨率也没用——实测 0.9s/1.4s/1.8s/2.4s 四档下不同值分别只有
# 4/4/5/6 个。所以别写"17 个各不相同的频率"，那是想当然。
#
# 集体之所以找不出节律，真正的来源是**相位**：每个槽的相位取自槽名的确定性噪声，
# 铺满 [0,1)。同一个抽动速率下 17 个互不相同的相位，意味着任何时刻都有几条在甩、
# 几条在落，且没有两条同时开始。自检里不靠这段说明，直接量两两轨迹的互相关。
LUNGE_PUSH = 21.0         # 扑击爆发段 root 前冲的位移（见 anim_lunge）
LUNGE_FORE = 11.4         # 爆发段前段再甩出去的位移
FLICK_ATTACK = 0.18       # 一次抽搐里"甩出去"占的比例，其余是回落
FLICK_MAX_HZ = 12.0       # 单节抽搐频率上限。**这条是渲染/知觉的界，不是物理的界**，
                          # 别把它说成推导：梢节的力臂只有零点几像素，按速度上限反推
                          # 出来是 69 Hz，那已经不是"抽"是"振"，而且采样数会直接爆到
                          # 689。真正的物理上界该来自组织的最大应变率，但仓库里那个
                          # STRAIN_RATE=0.286/s 是整体蠕动的慢速率，拿来当肌肉快抽的
                          # 上界会把整条动画压死（算出 0.42 Hz）。缺一个像样的模型，
                          # 所以这里明写成常数。
FLICK_KEYS = 2            # 起手段至少落几帧：两帧（起点+峰值）就够表达"猛地一甩"，
                          # 再多只是把线性斜坡采得更密。要三帧的话采样数直接翻半倍
THRASH_LEN = 0.70         # 乱抽循环时长（秒）。短是有原因的：抽得最快的那一节定
                          # 采样密度，而现在每条芽有四节独立驱动，关键帧是
                          # 骨数 × 采样数。0.9s + 18Hz 上限实测 12220 帧 / 8.2MB，
                          # 对一条动画太重。68 个各自发火的关节，0.7s 已经够乱


def strike_speed() -> float:
    """这具身体做得到的**最快尖端速度**（px/s）：扑击爆发段的峰值。

    smoothstep 的峰值速度是平均速度的 1.5 倍。不采样动画、直接从 anim_lunge 的常数
    推——采样要建 Rig，而这个数在建 Rig 之前就要用。
    """
    dur = (1.0 - LUNGE_WINDUP) * ANIMS["core_lunge"][0]
    return 1.5 * (LUNGE_PUSH + LUNGE_FORE) / dur


def bud_reach(name: str, scale: float = 1.0) -> float:
    """芽尖离根部的距离（px）—— 摆动的力臂。"""
    s = C.sockets()[name]
    return scale * max(float(np.linalg.norm(c - s.pos)) + r
                       for c, r, _m in C.bud_shape(s, 1.0))


@lru_cache(maxsize=8)
def bud_flicks(scale: float, length: float) -> dict[str, tuple[int, float]]:
    """每个槽在一条 `length` 秒的循环里抽几下，以及它的相位。

    整数次数是**循环闭合的硬要求**（非整数会在首末帧之间炸出跳变，拟态灰烬蛛的触肢
    微颤就是这么翻的车）。取整只准**往下**走：往上会超过速度上限，那是这具身体做不到的。

    次数**不要求两两不同**。本来写成互不相同，理由是"次数一样的两条会被眼睛配成一对"
    ——但 17 条要互不相同的整数就得占满 17 个整数格，而速度上限给出的频率跨度只有
    2.8 倍，凑够 17 格得把循环拉到 5 秒以上，采样数随之爆掉。实际上配成对的前提是
    **同频且同相**，而相位逐槽取自确定性噪声，本来就各不相同。所以只保留真正必要的
    那条：全体最大公约数为 1，否则集体图案会在 length/gcd 处重复，比循环本身还早。
    """
    amp = math.radians(C.BUD_FOLD_DEG)
    vmax = strike_speed()
    count: dict[str, int] = {}
    for n in sorted(C.sockets()):
        count[n] = max(1, int(vmax * FLICK_ATTACK / (amp * bud_reach(n, scale)) * length))
    g = 0
    for k in count.values():
        g = math.gcd(g, k)
    if g != 1:                            # 把最大的那条减 1 打散，集体周期回到整条循环
        top = max(count, key=lambda n: count[n])
        count[top] -= 1

    # 相位**构造**出来，不是掷出来的。这是全篇唯一一处不从物理推的量，说明白：
    # 频率既然挤在一起（见上面那段），"各抽各的"就全压在相位上；而按噪声取相位是
    # 会撞的——实测 head_l 与 head_dorsal 撞到轨迹互相关 0.90，看上去就是一对在
    # 打拍子。所以把同频的那几条在一个周期里**均匀铺开**，任意两条不同时起手。
    # 组间再各自错开一点，免得不同频的几组在 t=0 一起甩。
    out: dict[str, tuple[int, float]] = {}
    groups: dict[int, list[str]] = {}
    for n, k in count.items():
        groups.setdefault(k, []).append(n)
    for k, members in groups.items():
        for j, n in enumerate(sorted(members)):
            out[n] = (k, (j / len(members) + C._noise(k, "grp") * 0.5) % 1.0)
    return out


@lru_cache(maxsize=32)
def _flick_frame(name: str) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """芽的（外法向, 切向 1, 切向 2）。摆动平面由**挂载点自己的法向**定——
    17 个槽朝 17 个方向，于是 17 条芽各在各的平面里抽，不需要外加随机方向。"""
    s = C.sockets()[name]
    t1, t2 = C._tangent_basis(s.normal)
    return s.normal, t1, t2


SWAY_DEG = 7.0            # 每节常驻的漂移幅度（度）。抽搐之间**不许冻住**——一条
                          # 只在发火那一瞬间动、其余时间纹丝不动的触手读成机械臂


@lru_cache(maxsize=1)
def _sock_order() -> tuple[str, ...]:
    return tuple(C.sockets())


def _sock_index(name: str) -> int:
    return _sock_order().index(name)


@lru_cache(maxsize=32)
def tendril(name: str) -> tuple[tuple[str, ...], tuple[float, ...], tuple[float, ...]]:
    """一条芽的骨链：（骨名, 各节剩余力臂 px, 各节自然摆频 Hz）。

    **每一节各自被驱动，不是整条一起转，也不是一节带一节。**

    先写成"根部驱动、每节滞后四分之一周期"的鞭子模型，算出来不成立：这些节的自然摆频
    只有 0.7–1.4 Hz，而抽搐频率在 5 Hz 以上——远高于共振点的受迫振子，响应又小又乱，
    "末端跟着根部甩"这个图景根本不适用。实测滞后累积到 0.99 秒，比整条循环还长，末端
    响应的是两拍之前的驱动，看着不是拖尾是脱节。

    正解来自正典本身：**这东西没有神经**（"不是一条被神经支配的肢体，是别人的组织被
    强行编织进来"）。没有神经就没有沿肢体传导的激活波，每一节只能被局部激发——所以
    每节有**自己的节律、自己的方向序列、自己的发火图案**，彼此不协调。整条于是不是
    圆弧也不是鞭子，是一条不断改变形状的 S 形，这恰恰是最瘆人的读法。

    每节的节律仍走同一条速度上限（见本节开头），只是力臂换成**这一节以外那一截**的
    长度：越靠梢剩得越短、抽得越快。实测根部约 2 下/循环、梢部约 9 下。
    """
    import locomotion as L                       # 只用一条式子，放这儿避免顶层耦合

    s = C.sockets()[name]
    segs = C.bud_segments(s)
    chain = tuple(f"bud_{name}" if j == 0 else f"bud_{name}_{j}" for j in range(len(segs)))
    arm = tuple(float(sum(segs[j:])) for j in range(len(segs)))
    hz = tuple(max(L.natural_hz(tuple(segs[j:])), 1e-3) for j in range(len(segs)))
    return chain, arm, hz


def joint_flicks(name: str, scale: float, length: float) -> list[tuple[int, float]]:
    """骨链上每一节在一条 `length` 秒循环里抽几下、相位多少。

    速度上限按节数均分（四节各分四分之一的尖端速度预算），弯角也均分——没有理由认为
    哪一节更该弯，均分之下四节各自的节律差异就已经把形状搅乱了。
    """
    _chain, arm, _hz = tendril(name)
    amp = math.radians(C.BUD_FOLD_DEG) / len(arm)
    budget = strike_speed() / len(arm)
    # 上限**逐槽错开**：所有被顶到同一个整数的关节会共享节律，于是又同步了
    # （实测速率互相关被顶回 0.59）。不同整数周期在整条循环上正交，错开就散了。
    cap = max(1, int(FLICK_MAX_HZ * length) - _sock_index(name) % 3)
    out = []
    for j, a in enumerate(arm):
        k = max(1, min(cap, int(budget * FLICK_ATTACK / (amp * a * scale) * length)))
        out.append((k, C._noise(name, j, "jph")))
    return out


FLICK_DUTY = 0.62         # 每一拍真的抽出去的概率。1.0 = 节拍器，太规整；太低则大半
                          # 时间不动，读成"死了"


@lru_cache(maxsize=64)
def _fires(name: str, count: int) -> tuple[bool, ...]:
    """这条芽在一个循环的第 k 拍发不发。按槽名确定性生成，所以可复现。

    至少保证发一次——一条整条循环都不动的芽，在"所有触手都在摆"里就是没在摆。
    """
    hits = tuple(C._noise(name, k, "fire") < FLICK_DUTY for k in range(max(1, count)))
    return hits if any(hits) else (True,) + hits[1:]


def flick(name: str, u: float, count: int, phase: float,
          j: int = 0) -> tuple[np.ndarray, float]:
    """一条芽在归一化时刻 u 的**抽搐驱动**：返回（旋转轴, 归一化幅度 0..1）。

    只给驱动不给最终姿态：骨链上每一节各有自己的节律与方向序列（见 tendril_pose），
    所以这里不能替整条把旋转算死。`j` 是节序号——发火图案与方向序列都按 (槽, 节) 取，
    于是同一条芽的四节彼此不协调。

    不是正弦。正弦读成"水草在飘"——那是被介质推着的被动运动。这团东西没有神经支配
    （正典：不是一条被神经支配的肢体，是别人的组织被强行编织进来），它只会痉挛：
    **猛地甩出去，然后松掉**。所以波形是快起慢落的脉冲串。

    每一下甩的方向按黄金角在切平面上转，同一条芽连着两下不往同一边倒；方向序列按
    `count` 取模，所以整条循环严丝合缝地首尾相接。

    **每一拍发不发是各自的事**（`_fires`）。这一条是必须的：17 个挂载点的几何几乎
    一样，从几何推出来的节律就几乎一样，15 条会挤在同一个次数上。把相位均匀铺开也
    救不了——同频信号相邻相位只差一个周期的十五分之一，在 5Hz 下就是 12 毫秒，眼睛
    看到的是一圈茬依次抽过去的**行波**，那是有节律的，不是乱的。痉挛本来也不是
    节拍器：同一个最快节律之下，每一拍发不发各自决定，整体才真的找不出拍子。
    """
    k = int((u * count + phase) // 1.0) % max(1, count)
    if not _fires(f"{name}#{j}", count)[k]:
        return np.zeros(3), 0.0
    x = (u * count + phase) % 1.0
    # 快起慢落，且两端严格为 0 —— 循环接缝由波形本身保证，不靠首末帧对齐去补
    env = (smooth(x / FLICK_ATTACK) if x < FLICK_ATTACK
           else 1.0 - smooth((x - FLICK_ATTACK) / (1.0 - FLICK_ATTACK)))
    n, t1, t2 = _flick_frame(name)
    # 黄金角：连着几下不重方向；加上节序号的偏置，同一条芽各节也不往同一边倒
    a = 2.0 * math.pi * (((k + j * 0.37) * 0.6180339887) % 1.0)
    d = math.cos(a) * t1 + math.sin(a) * t2
    return np.cross(n, d), env


def tendril_pose(p: Pose, name: str, u: float, length: float,
                 joints: list[tuple[int, float]], *, gain: float = 1.0,
                 sway: float = 1.0) -> list[np.ndarray]:
    """把一条芽的骨链摆到 u 时刻，返回各节**累积**的世界旋转（落地钳制要用）。

    每节 = 自己那一拍的抽搐 × gain + 常驻漂移。两者都写在同一根骨上，所以"没在抽的
    时候"也不是静止的——那一刻它在按自己这一节的自然摆频慢慢晃。只在发火瞬间动、
    其余时间冻住，眼睛立刻读成机械臂而不是活物，这是"机械感"的主要来源。

    各节的旋转是**相对父节**的，所以世界角度沿链累积——四节各转各的，整条就成了一条
    不断改变形状的 S 形，而不是一根绕根部倾倒的棍子。
    """
    chain, _arm, hz = tendril(name)
    n, t1, t2 = _flick_frame(name)
    acc = np.eye(3)
    out = []
    amp = math.radians(C.BUD_FOLD_DEG) / len(chain)
    for j, bone in enumerate(chain):
        count, phase = joints[j]
        axis, env = flick(name, u, count, phase, j)
        R = np.eye(3)
        if env > 1e-6:
            R = C.axis_angle(axis, amp * env * gain)
        if sway > 1e-6:
            # 常驻漂移：两条切向各一个相位，尖端于是画小椭圆而不是来回摆。
            #
            # 周期数与相位都是**构造**的，不是取噪声。本来按各节自己的自然摆频取，
            # 但循环闭合要求整数周期，而 0.66–2.39 Hz 乘上 0.7 秒四舍五入之后只剩
            # 1 和 2 两种——十七条芽的漂移于是同频，再配上随机相位，总有几对撞在一起：
            # 实测只留漂移时两两互相关高达 0.970。
            # 现在：周期数按节序递增（越靠梢自然频率越高，这个序是真的），相位按槽序
            # 在整条循环里均匀铺开，任意两条芽的同序节都不同相。
            # 周期数必须**逐槽也不同**，不能只逐节不同：同频正弦无论相位怎么铺，相邻
            # 两条的相关都下不来（十七条均匀铺开，相邻差 1/17 周期 = 42°，相关 0.74）。
            # 而**不同整数周期的正弦在整条循环上正交**，相关严格为零。所以周期数同时
            # 吃节序与槽序。
            cyc = 1 + j + (_sock_index(name) % 4)
            base = (_sock_index(name) + 0.37 * j) / max(1, len(C.sockets()))
            a1 = math.radians(SWAY_DEG * sway) * breathe(u, cyc / length, base, length)
            a2 = math.radians(SWAY_DEG * sway) * breathe(u, cyc / length, base + 0.27, length)
            R = R @ C.axis_angle(np.cross(n, t1), a1) @ C.axis_angle(np.cross(n, t2), a2)
        p[bone].rot = euler_of(R)
        acc = acc @ R
        out.append(acc.copy())
    return out


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


GRAFT_TWITCH = 0.22       # 嫁接抽搐相对乱抽的幅度：同一套痉挛，收着来


def graft_length(name: str) -> float:
    """长满**这一个**槽要多久（秒）。

    组织合成速度是整只兽的代谢属性，是一个常数（px³/s）；所以长一条芽要多久，正比于
    这条芽的**用料**。用料差别很大——`vest_dr` 的挂载面只有 girth 1.40，`limb_mr`
    有 4.01，两者的组织量差一个数量级。于是「每一支都有自己的生长动画」不是复制 17 份，
    是 17 条**长度各不相同**的动画。

    速率由 `GRAFT_SOCKET` 那条锚定（它就是原来那条 3 秒的），其余按用料等比推。

    下限 `GRAFT_MIN` 不是保险丝，是**可读性的物理下限**：这条动画的内容是"推进—停滞—
    推进"共五段，每段至少要活过两个 tick（0.1s）才看得出来。`vest_dr` 的挂载面只有
    girth 1.40，按用料算出来 0.11 秒——比例没错，但那已经短到根本不是一段动画了。
    """
    rate = C.bud_tissue(C.sockets()[GRAFT_SOCKET], 1.0) / GRAFT_REF_LEN
    return round(max(GRAFT_MIN, C.bud_tissue(C.sockets()[name], 1.0) / rate), 2)


def anim_graft_at(name: str, rig: Rig, t: float, length: float) -> Pose:
    """嫁接：芽在**某一个**挂载点上鼓起来。

    正典里这个过程要**七日**。动画只能给一段压缩表现，所以重点不是"长大"而是
    "长得不顺"：鼓胀是阶梯式的（推进夹停滞），中间还抽搐——那不是一条被神经支配的
    肢体，是一团正在被强行编织进来的别人的组织。

    停滞的位置逐槽不同（按槽名的确定性噪声挪），抽搐用的是和乱抽同一套痉挛，只是
    收着来。所以 17 条动画彼此**看得出不一样**，而不是同一条改个骨名。

    这一条**不缩本体**：嫁接的料来自尸体（正典：从野狗尸体上"借"的第四条腿），
    不是从自己身上抽。乱抽才缩（见 anim_thrash）——料的来源不同，结果就不同。
    """
    p = Pose()
    j = C._noise(name, "graft") * 0.10 - 0.05
    steps = (0.0, 0.34 + j, 0.38 + j, 0.72 + j, 0.76 + j, 1.0)
    v = float(np.interp(t, steps, (0.0, 0.45, 0.45, 0.82, 0.82, 1.0)))
    tgt = f"bud_{name}"
    dormant_buds(p, active=tgt)
    p[tgt].scale = [BUD_DORMANT + (1.0 - BUD_DORMANT) * v] * 3
    # 抽搐只在推进段有——停滞时它是死的，更瘆人
    # 推进段才抽搐；停滞段只剩常驻漂移——它没死，只是长不动了，这比完全冻住瘆人
    moving = 1.0 if (t < steps[1] or steps[2] < t < steps[3] or t > steps[4]) else 0.0
    tendril_pose(p, name, wrap(t), length, joint_flicks(name, 1.0, length),
                 gain=GRAFT_TWITCH * moving, sway=0.55)
    p["core_mid"].scale = [1.0 + 0.04 * v] * 3
    return p


@lru_cache(maxsize=1)
def thrash_scale() -> float:
    """乱抽时每条芽的骨缩放。

    组织不是凭空来的。同时能持有的未分化组织只有**一条部件的量**（`core.graft_budget`
    从正典"一次一条、每条七日"反推）。要 17 个槽同时冒芽，那一份料就得摊到 17 处——
    于是全身冒出来的只能是**短茬**，不是触手。

    这不是妥协，是这段动画的内容：它把自己能调动的组织全推到体表去试探，一处都不肯
    落下，代价是每一处都只够冒出一小截。而短茬的力臂小，按速度上限反而抽得更快
    （见 bud_flicks）——"又短又急"是同一个预算推出来的两件事。
    """
    return C.spread_scale(tuple(C.sockets().values()), C.graft_budget())


def anim_thrash(rig: Rig, t: float, length: float) -> Pose:
    """乱抽：每个挂载点都冒出一小截，同时朝各自的方向急抽。

    要读成"诡异"而不是"水草"，靠的是三件事，全部是推出来的：

      · **各抽各的**：频率由各自的力臂定（f ∝ 1/L），17 条长短不一 ⇒ 17 个频率，
        且循环次数两两不同、最大公约数为 1 ⇒ 整条循环里集体图案不重复
      · **痉挛不是波动**：快甩慢落的脉冲串，不是正弦（无神经支配的组织不会平滑运动）
      · **本体几乎不动**：主体只有极缓的一点起伏。一堆东西在抽、而载体是静的，
        才瘆人；本体跟着一起晃就读成"整只在抖"

    本体还要**变小**：摊出去的料是从自己身上抽的，等体积守恒（自检直接断言）。
    """
    p = Pose()
    sc = thrash_scale()
    for n in C.sockets():
        p[f"bud_{n}"].scale = [sc] * 3   # 茬已经在外面了：顶出来那一段属于嫁接，不在这里
        tendril_pose(p, n, wrap(t), length, joint_flicks(n, sc, length))

    # 抽出去的组织从本体来：芽的总增量体积 = 本体的减少量。整条动画里茬的尺寸不变，
    # 所以这是个常数——本体在这条动画里**一动不动**，只是比平时小一圈。
    #
    # 静止是刻意的。一堆东西在抽、而载体纹丝不动，才瘆人；载体跟着一起起伏就读成
    # "整只在抖"，抽搐反而被稀释掉了。顺带把本体的通道压成两帧常量，整条动画的
    # 关键帧于是只剩芽的旋转。
    gained = sum(C.bud_tissue(s, 1.0) for s in C.sockets().values()) * sc ** 3
    shrink = (1.0 - gained / (sum(C.lobe_mass().values()) * C.VOX ** 3)) ** (1.0 / 3.0)
    for n in LOBES_MAIN + LOBES_LUMP:
        p[n].scale = [shrink, shrink, shrink]
    return p


BURST_SWELL = 0.14        # 爆体前的鼓胀段占比：内压把表皮撑到极限
BURST_G = 90.0            # 碎片下落加速度（px/s²）。比真实重力小——碎片是黏的，带阻尼
BURST_DRAG = 0.55         # 水平速度的指数衰减系数（1/s）：黏肉飞不远
BURST_REST = 0.5          # 碎片落地后**最低点**离地高度（px）：黏肉摊在地上，不是悬着
BURST_SPIN_MAX = 30.0     # 翻滚角上限（度）。黏肉是瘫倒不是翻筋斗——不封顶时位移大的
                          # 碎片转到 120°，绕枢轴一甩就把几何插进地里 48px（实测）


@lru_cache(maxsize=1)
def _bone_reach() -> dict[str, tuple[float, float]]:
    """每根骨的（静止几何最低点 y, 枢轴到几何的最大距离）。

    落地钳制要按**几何最低点**而不是质心：质心离地还有富余时，底下的肉早已扎进地里。
    还要按最大半径算翻滚甩幅——绕枢轴转 θ 会让远端再下沉 extent·sinθ，不算这一项
    钳完仍会穿地（实测从 -48 只降到 -29）。
    """
    rig = Rig(MODEL)
    out: dict[str, tuple[float, float]] = {}
    for n in rig.order:
        pts = rig.bone_points(n)
        if len(pts):
            o = rig.bones[n].origin
            out[n] = (float(pts[:, 1].min()),
                      float(np.linalg.norm(pts - o, axis=1).max()))
    return out


def anim_burst(rig: Rig, t: float, length: float) -> Pose:
    """爆体：癒合痕同时崩开，各团肉沿自己的方向飞散。

    这是 core_death 的另一条路——不是泄气塌成一滩，是**撑破**。哪几块、朝哪飞、飞多快
    全部来自 fission（接合面积定裂法，动量守恒定速度），这里只负责把它演出来。

    弹道用最朴素的抛体 + 水平阻尼：黏肉不是弹片，飞不远。**不额外加"炸飞"的整体位移**
    ——净动量为零是 fission 的硬约束，整团往一边飞就读成被外力炸了，而不是自己崩开。

    骨骼是树，父骨的位移会传给子骨，所以子骨只写**相对父骨的增量**。
    """
    import fission as F                        # 循环依赖：只在用到时导入

    p = Pose()
    swell = smooth(np.clip(t / BURST_SWELL, 0.0, 1.0))
    for n in LOBES_MAIN + LOBES_LUMP:
        s = 1.0 + 0.20 * swell
        p[n].scale = [s, s, s]
    dormant_buds(p)
    if t <= BURST_SWELL:
        return p

    tau = (t - BURST_SWELL) * length           # 崩开后经过的真实秒数
    reach = _bone_reach()
    swing_max = math.sin(math.radians(BURST_SPIN_MAX))
    disp: dict[str, np.ndarray] = {}
    for frag in F.build_fragments():
        v = frag.launch
        damp = (1.0 - math.exp(-BURST_DRAG * tau)) / BURST_DRAG
        dy = v[1] * tau - 0.5 * BURST_G * tau * tau
        for n in frag.lobes:
            # **落地即停**：抛体到地面为止，黏肉不弹也不继续下沉。不钳的话 1.6 秒的
            # 自由落体有 115px，碎片直接穿地掉出画面（实测末两帧全在地平线以下）。
            low, ext = reach.get(n, (12.0, 8.0))
            floor = BURST_REST - low + ext * swing_max     # 含翻滚甩幅
            disp[n] = np.array([v[0] * damp, max(dy, floor), v[2] * damp])

    spins = {n: min(float(np.linalg.norm(d)) * 3.0, BURST_SPIN_MAX) for n, d in disp.items()}
    par = {lb.name: (lb.parent or "root") for lb in C.LOBES}
    for n in (lb.name for lb in C.LOBES):        # 父先于子（C.LOBES 已按此序）
        s = spins[n] - spins.get(par[n], 0.0)
        p[n].rot = [s * 0.6, s * 0.3, -s * 0.45]
        place_world(rig, p, n, disp[n])
    return p


SPLIT_GAP = 26.0          # 分裂终了两半的间距（px）


def anim_split(rig: Rig, t: float, length: float) -> Pose:
    """健康分裂：沿最小割把自己扯成两半，各自走开。这是它的繁殖。

    和爆体的区别不在快慢，在**主动**：爆体是被撑破，各团被动飞散；分裂是自己拽，
    所以有一段明显的「僵持」——两半各自往反方向使劲，癒合痕先绷紧、变细，撑到极限
    才断。断之后两半都完好，各自带着自己那份挂载点离开。

    切面来自 fission.split_seam（最小割 + 两半都能活），不是画的。
    """
    import fission as F

    p = Pose()
    sp = F.split_seam()
    if sp is None:                              # 没有可行切面就只是憋着
        return anim_idle(rig, t, length)
    side_a, side_b, _cut = sp

    strain = smooth(np.clip(t / 0.62, 0.0, 1.0))     # 僵持：绷紧、变细
    tear = smooth(np.clip((t - 0.62) / 0.38, 0.0, 1.0))  # 断开：各自退走
    # 绷紧时整体被拉细（组织被扯薄），断开后各自回弹
    thin = 1.0 - 0.16 * strain + 0.10 * tear
    for n in LOBES_MAIN + LOBES_LUMP:
        p[n].scale = [thin, thin, 1.0 + 0.10 * strain]
    dormant_buds(p)

    mass = C.lobe_mass()
    cen = C.lobe_centroid()
    ca = sum(cen[n] * mass[n] for n in side_a) / sum(mass[n] for n in side_a)
    cb = sum(cen[n] * mass[n] for n in side_b) / sum(mass[n] for n in side_b)
    axis = ca - cb
    axis[1] = 0.0                                # 水平掰开，不是上下扯
    axis /= max(float(np.linalg.norm(axis)), 1e-6)
    # 僵持段只挪一点点（绷着），断开后才真正拉开——距离曲线的拐点就是"啪"的那一下
    reach = SPLIT_GAP * (0.18 * strain + 0.82 * tear)
    ma, mb = sum(mass[n] for n in side_a), sum(mass[n] for n in side_b)
    # 反冲按质量分配：轻的那半退得远（动量守恒，同 fission 的爆体）
    da, db = axis * reach * mb / (ma + mb), -axis * reach * ma / (ma + mb)

    for n in (lb.name for lb in C.LOBES):        # 父先于子
        place_world(rig, p, n, da if n in side_a else db)
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


def _thrash_samples(length: float) -> int:
    """乱抽的采样数：由**最快的那一节**定，不是拍一个数。

    脉冲的起手段只占一个周期的 FLICK_ATTACK，要让它在导出后还是"猛地一甩"而不是被
    插值抹平，起手段至少要落上 3 帧。抽最快的那条决定了整条动画的采样密度。
    """
    sc = thrash_scale()
    top = max(k for n in C.sockets() for k, _ph in joint_flicks(n, sc, length))
    return int(math.ceil(top / FLICK_ATTACK * FLICK_KEYS))


# (名字, 时长秒, 是否循环, 采样数, 函数)
ANIMS: dict[str, tuple[float, bool, int, object]] = {
    "core_idle": (5.0, True, 40, anim_idle),
    "core_crawl": (1.0 / CRAWL_HZ, True, 36, anim_crawl),
    "core_lunge": (0.75, False, 24, anim_lunge),
    "core_engulf": (1.40, False, 30, anim_engulf),
    "core_burst": (1.60, False, 34, anim_burst),
    "core_split": (2.60, False, 36, anim_split),
    "core_hurt": (0.45, False, 20, anim_hurt),
    "core_death": (2.20, False, 36, anim_death),
}

# 乱抽：循环。采样密度由最快的那条芽反推
ANIMS["core_thrash"] = (THRASH_LEN, True, _thrash_samples(THRASH_LEN), anim_thrash)

# 每个挂载点一条嫁接动画。时长各不相同（正比于该处的组织用料，见 graft_length），
# 停滞点与抽搐节奏也各不相同——「每一支都有生长动画」指的是 17 条真的不一样的动画。
for _n in C.sockets():
    ANIMS[f"graft_{_n}"] = (graft_length(_n), False, 44,
                            lambda rig, t, ln, nm=_n: anim_graft_at(nm, rig, t, ln))


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
