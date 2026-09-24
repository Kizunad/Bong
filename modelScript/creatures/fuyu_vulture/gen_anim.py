#!/usr/bin/env python3
"""腐羽鹫 —— 动画生成：程序化步态 + 飞行 + 情绪动作，写回 bbmodel 关键帧。

原则和建模层一致：**逐层派生，别凭手感拧角度**。
  · 着地的脚一律只给「该踩在哪」，关节角由 rig.solve_foot 逆解 —— 支撑相里脚锁死在世
    界坐标上，躯干怎么起伏侧摆都不会带着脚一起飘。滑步是两足动画的头号破绽。
  · 两足没有四足那个静态三角支撑，**每一步都在单支撑相**：重心必须横移到支撑脚上方，
    少了这一下走路就是在冰面上平移。所以 walk/run 里 `shift_over()` 是必需件不是润色。
  · 收腿姿的关节角由 rig.tuck_angles() 搜出来（三档腿比例不同），不写死。

**收翼 ↔ 展翼**：羽的几何是按姿态烘焙的（收翼时飞羽顺体轴叠成一摞、展翼才铺成扇面），
所以两者不是同一批方块转个角度的关系。解法是**每根飞羽自带一根骨**（gen_pelt 的 quill：
骨 pivot 落在羽根、绑定旋转烙住羽轴、元素换算进这根骨的坐标系），于是收→展退化成逐羽的
旋转 + 沿羽轴缩放 —— `rig.unfold_pose()` 直接从两份模型解出这个姿态，`unfold`/`fold` 两
条动作就是它的淡入淡出。世界几何逐件对拍过（残差 < 0.35px，见 check_anim 第 8 条）。

仍然是两份绑定姿模型：地面动作跑在收翼模型上、飞行动作跑在展翼模型上。把飞行那五条也
搬到收翼模型（先叠 unfold 再叠拍翼）技术上已经通了，但那要把五条动作全部重新表达一遍，
留到接引擎时一起做。

输出（modelScript/models/fuyu_vulture/）：
  FuyuVultureRig<档>.bbmodel         收翼绑定姿 + 地面动作
  FuyuVultureRig<档>Flight.bbmodel   展翼绑定姿 + 飞行动作
  fuyu_vulture_<档>.animation.json   GeckoLib 动画（参考/兜底，见 animkit.write_geckolib）
源 Pelt 模型只读不写 —— 用户在 Blockbench 里的手改留在那份里。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

from bbmodel_maker.rig.animkit import (  # noqa: E402
    Pose, build_tracks, keyed, pulse, smooth, write_animated_bbmodel, write_geckolib,
)
from rig import (  # noqa: E402
    MODELS, BipedGait, VultureRig, blend_pose, default_rig, unfold_pose,
)

NAMESPACE = "bong"
MODEL_ID = "fuyu_vulture"
SIZES = ("small", "mid", "large")


# ================================================================ 每档常量

@dataclass
class Body:
    """一具绑定姿派生出的尺度常量与步态。全部从模型量，没有写死的绝对长度。"""

    rig: VultureRig
    H: float          # 髋高（站姿垂直预算的基准）
    U: float          # 建模层的尺度单位
    rest: dict
    walk: BipedGait
    run: BipedGait


_CACHE: dict[str, Body] = {}


def K(rig: VultureRig) -> Body:
    key = str(rig.path)
    if key not in _CACHE:
        H = float(rig.bones["femur_l"].origin[1])
        # 步幅按**髋高**给，不按 U：三档的腿/体比例并不相同，按 U 给的话大档迈的是小
        # 碎步、小档劈叉。占空比 0.62 > 0.5 是走（有双支撑），0.42 < 0.5 才有腾空段。
        walk = BipedGait(rig, duty=0.62, fwd=0.22 * H, back=0.26 * H, lift=0.15 * H,
                         swing_ease=1.7, toe_off=24.0, medial=0.40)
        run = BipedGait(rig, duty=0.42, fwd=0.30 * H, back=0.32 * H, lift=0.26 * H,
                        swing_ease=2.6, toe_off=26.0, medial=0.55)
        _CACHE[key] = Body(rig, H, rig.U, rig.rest_stance(), walk, run)
    return _CACHE[key]


# 跑的腾空窗口：l 支撑 [0,0.42)、r 支撑 [0.5,0.92)，并集之外是 [0.42,0.5) 与 [0.92,1)。
RUN_FLIGHT = ((0.42, 0.50), (0.92, 1.00))


def run_arc(t: float) -> float:
    """腾空抛物弧：两个窗口各自两端为 0、中点为 1，窗口外为 0。

    不能用正弦：正弦在落地瞬间还在高位，而腿的余量有限，落地帧直接够不着地面（狮子
    那边实测前掌悬空 2.9 单位）。真实腾空是抛物线，离地和落地时高度都回到常态。
    """
    for f0, f1 in RUN_FLIGHT:
        if f0 <= t <= f1:
            s = (t - f0) / (f1 - f0)
            return 4.0 * s * (1.0 - s)
    return 0.0


def head_hold(t: float, n_step: int, stride: float) -> float:
    """鸟走路的"停-冲"头动：支撑段头锁在世界坐标里，换步时猛地前送。

    原地循环里表现为：头相对身体匀速后移（= 世界里不动），到换步瞬间快速前冲归零。
    返回的是**相对躯干的后移量**（正 = 往后）。
    """
    u = (t * n_step) % 1.0
    hold, dash = 0.72, 0.28
    if u < hold:
        return stride * (u / hold)
    return stride * (1.0 - smooth((u - hold) / dash))


# ================================================================ 地面动作

def anim_idle(rig: VultureRig, t: float) -> Pose:
    """怠：缩着脖子立在腐肉旁，呼吸慢，眼睛缓缓扫视，偶尔抖一下羽。

    这只鸟的存在感靠"几乎不动"——它在等你死。所以底噪压到最低，只留呼吸和一次抖羽；
    循环里所有频率都取整周数，否则 t=1 处相位落不回 0，每轮接缝肉眼可见地咯噔一下。
    """
    k = K(rig)
    p = Pose()
    rig.breathe(p, t, rate=2.0, depth=0.62)
    ruffle = pulse(t, 0.68, 0.030)          # 抖羽：一瞬间的高频颤

    # 颈：缩着（总 pitch 负 = 头低），慢扫左右，抖羽时整条一弹
    scan = math.sin(2.0 * math.pi * t)
    rig.neck_curve(p, pitch=-7.0 - 2.0 * math.sin(2.0 * math.pi * 2.0 * t) - 9.0 * ruffle,
                   yaw=9.0 * scan, bias=1.5)
    p["skull"].rot[0] += 5.0 + 1.6 * math.sin(2.0 * math.pi * 2.0 * t - 0.9) + 7.0 * ruffle
    p["skull"].rot[1] += 6.0 * math.sin(2.0 * math.pi * t - 0.7)
    p["jaw"].rot[0] += 1.2 + 5.0 * ruffle

    # 翼：贴着体侧，只在抖羽时耸一下
    rig.wings(p, shrug=1.2 * math.sin(2.0 * math.pi * 2.0 * t) + 9.0 * ruffle,
              flex=6.0 * ruffle)
    rig.tail_pose(p, pitch=2.5 + 1.2 * math.sin(2.0 * math.pi * t - 1.1),
                  yaw=2.0 * math.sin(2.0 * math.pi * t + 0.4))
    p["trunk_front"].rot[2] += 2.6 * ruffle
    p["root"].pos[1] += 0.05 * k.U * math.sin(2.0 * math.pi * 2.0 * t)
    rig.plant(p, k.rest)
    return p


def anim_walk(rig: VultureRig, t: float) -> Pose:
    """慢步：左右交替，重心横压到支撑脚上，头"停-冲"。

    秃鹫走路是**摇摆步**：腿短、身子重，每一步整个躯干都要压过去。躯干不侧摆的两足
    走路一眼假 —— 那是在冰面上平移，不是在走。
    """
    k = K(rig)
    g = k.walk
    p = Pose()
    stride = g.fwd + g.back

    # 重心横移：单支撑窗口是 l[0.12,0.5) / r[0.62,1.0)，正弦峰谷正好落在两段中点
    lat = -math.sin(2.0 * math.pi * (t - 0.06))
    rig.shift_over(p, 0.85 * abs(g.rest["l"][0]) * lat, roll=5.2 * lat)
    p["trunk_front"].rot[2] += -1.9 * lat        # 上身比骨盆稳，反向补一点
    p["root"].pos[1] += k.H * (0.012 * math.cos(2.0 * math.pi * 2.0 * (t - 0.10)) - 0.010)

    rig.neck_curve(p, pitch=-5.0 + 2.4 * math.sin(2.0 * math.pi * 2.0 * t),
                   yaw=-4.5 * lat, bias=1.4)
    rig.head_bob(p, back=head_hold(t, 2, 0.30 * stride),
                 lift=0.05 * k.H * math.sin(2.0 * math.pi * 2.0 * t))
    p["skull"].rot[0] += 3.0 - 2.0 * math.sin(2.0 * math.pi * 2.0 * t)
    p["jaw"].rot[0] += 1.0

    rig.wings(p, shrug=2.4 * math.sin(2.0 * math.pi * 2.0 * t), flex=1.5)
    rig.tail_pose(p, pitch=3.0, yaw=-3.5 * lat)
    rig.breathe(p, t, rate=2.0, depth=0.18)
    g.solve(p, t)
    return p


def anim_run(rig: VultureRig, t: float) -> Pose:
    """奔逃/助跑：两个腾空段，身子前倾，翼半开当平衡杆。

    秃鹫在地上跑不是为了追猎，是为了起飞或者抢食。所以姿态是前倾、翼半张、脖子探出去
    —— 和 idle 的缩颈是同一只鸟的两个极端。
    """
    k = K(rig)
    g = k.run
    p = Pose()
    arc = run_arc(t)
    # 单支撑窗口 l[0,0.42) / r[0.5,0.92)，余弦的峰谷压在两段中点 0.21 / 0.71 上。
    # 早先照走路那组相位挪了个 −0.04 的正弦，t≈0 那几帧质心还留在右侧、支撑脚却是左脚
    # —— 后验的"质心同侧率"当场掉到 90%，看图完全看不出来。
    lat = -math.cos(2.0 * math.pi * (t - 0.21))

    rig.shift_over(p, 0.80 * abs(g.rest["l"][0]) * lat, roll=3.4 * lat)
    p["root"].pos[1] += k.H * (0.055 * arc - 0.030)
    p["hips"].rot[0] += -6.0 + 2.5 * arc         # 前倾：骨盆抬后缘
    p["trunk_front"].rot[0] += -4.0 + 2.0 * arc

    rig.neck_curve(p, pitch=-16.0 + 4.0 * arc, yaw=-3.0 * lat, bias=0.85)
    rig.head_bob(p, back=head_hold(t, 2, 0.16 * (g.fwd + g.back)))
    p["skull"].rot[0] += 12.0 - 3.0 * arc        # 颈探出去、头仍抬平看前方
    p["jaw"].rot[0] += 6.0 + 4.0 * math.sin(2.0 * math.pi * 2.0 * t)

    # 翼半开：跑动中的平衡杆兼预备起飞。用和威慑同一条通道（前掠为主），幅度取三分之一
    rig.wings(p, elev=10.0 + 4.0 * math.sin(2.0 * math.pi * 2.0 * t),
              sweep=20.0 + 5.0 * arc, flex=-6.0, shrug=4.0 + 3.0 * arc)
    rig.tail_pose(p, pitch=-7.0 - 3.0 * arc, yaw=-4.0 * lat)
    g.solve(p, t)
    return p


def anim_peck(rig: VultureRig, t: float) -> Pose:
    """啄食：探颈下扎 → 咬合 → 整个身子往后拽。

    秃鹫的进食动作核心是**拽**不是啄：喙钩住腐肉，靠整具躯干后仰把肉撕开。只做低头
    抬头的话读起来像鸡在啄米。
    """
    k = K(rig)
    p = Pose()
    dive = keyed(t, [(0.0, 0.0), (0.10, -0.18), (0.30, 1.0), (0.42, 1.0), (0.62, 0.15), (1.0, 0.0)])
    gape = keyed(t, [(0.0, 0.05), (0.24, 1.0), (0.33, 0.0), (0.70, 0.0), (0.86, 0.25), (1.0, 0.05)])
    tug = keyed(t, [(0.36, 0.0), (0.50, 1.0), (0.66, 0.75), (0.86, 0.0)])
    shake = math.sin(2.0 * math.pi * 13.0 * t) * tug

    # 躯干俯仰的符号：hips.rot[0] 为**正**是抬头（前段绕髋往上转）。扎下去要负、往回
    # 拽要正 —— 早先两处都写反了，于是"低头扎肉"变成挺着胸把脖子往下伸，喙尖离地还有
    # 大半个身位。量过：颈总角 −68 配躯干 −14 时颅底能压到 y≈3（髋高 17）。
    p["root"].pos[2] += k.H * (0.055 * dive - 0.085 * tug)
    p["root"].pos[1] += -k.H * 0.075 * dive + k.H * 0.020 * tug
    p["hips"].rot[0] += -14.0 * dive + 16.0 * tug
    p["trunk_front"].rot[0] += -10.0 * dive + 12.0 * tug

    rig.neck_curve(p, pitch=-68.0 * dive + 30.0 * tug + 1.5 * shake, bias=1.25)
    # 光靠弯是够不着的：S 形颈弯到 −68° 时头只是**蜷回胸口**，不是探出去。再叠一段
    # 沿颈摊开的平移，把头真正送到身前下方（拽的时候反向抽回来）。
    rig.head_bob(p, back=k.H * (-0.50 * dive + 0.26 * tug),
                 lift=k.H * (-0.22 * dive + 0.10 * tug))
    p["skull"].rot[0] += -20.0 * dive + 24.0 * tug + 2.6 * shake
    p["jaw"].rot[0] += 34.0 * gape + 1.8 * shake

    rig.wings(p, shrug=6.0 * dive + 9.0 * tug, flex=4.0 * tug)
    rig.tail_pose(p, pitch=-9.0 * dive + 7.0 * tug)
    # 腐肉是有厚度的：喙尖停在离地约 0.06 髋高处，不是插进土里
    rig.head_floor(p, floor=0.06 * k.H)
    rig.plant(p, k.rest)
    return p


def anim_threat(rig: VultureRig, t: float) -> Pose:
    """威慑：张翼罩住食物（mantling）+ 压低前伸的颈 + 嘶鸣。

    秃鹫不叫（没有鸣管），威胁全靠体型与嘶气。所以这条动作没有"仰头长啸"，只有把自己
    撑到最大再压低了朝你逼过来 —— 张翼是遮挡，不是求偶。
    """
    k = K(rig)
    p = Pose()
    rise = keyed(t, [(0.0, 0.0), (0.20, 1.0), (0.72, 1.0), (1.0, 0.0)])
    lunge = keyed(t, [(0.26, 0.0), (0.42, 1.0), (0.60, 0.72), (0.86, 0.0)])
    hiss = keyed(t, [(0.30, 0.0), (0.40, 1.0), (0.56, 0.2), (0.62, 0.9), (0.78, 0.0)])
    tremor = math.sin(2.0 * math.pi * 17.0 * t) * hiss

    p["root"].pos[2] += -k.H * 0.070 * lunge
    p["root"].pos[1] += -k.H * 0.045 * rise
    p["hips"].rot[0] += -5.0 * rise - 4.0 * lunge
    p["trunk_front"].rot[0] += -9.0 * rise - 7.0 * lunge

    rig.neck_curve(p, pitch=-24.0 * rise - 12.0 * lunge + 1.2 * tremor, bias=0.75)
    p["skull"].rot[0] += 20.0 * rise + 12.0 * lunge + 2.2 * tremor
    p["jaw"].rot[0] += 12.0 * rise + 30.0 * hiss + 2.4 * tremor

    # 张翼罩食：主力是**前掠**不是抬举 —— 收翼姿下整片翼折在体侧偏后，只有把它往前甩
    # 才把正面轮廓撑开。扫过 (肱骨 z, 肱骨 y, 尺骨 y) 三维网格挑正面最宽且翼尖不扫地的
    # 一组，换算成本层语义是 elev≈28 / sweep≈55 / flex≈−18；早先那组 flex=40 只把前臂
    # 往体侧掰，正面轮廓宽度几乎没变（渲图上完全读不出"张翼"）。
    rig.wings(p, elev=28.0 * rise + 5.0 * lunge, sweep=55.0 * rise + 6.0 * lunge,
              flex=-18.0 * rise, shrug=10.0 * rise, twist=-6.0 * rise)
    rig.tail_pose(p, pitch=-16.0 * rise, roll=2.0 * tremor)
    rig.plant(p, k.rest)
    return p


def anim_hurt(rig: VultureRig, t: float) -> Pose:
    """受击：一记硬顿挫 + 递减抖动 + 翼一弹，不是平滑正弦。"""
    k = K(rig)
    p = Pose()
    hit = keyed(t, [(0.0, 0.0), (0.10, 1.0), (0.32, 0.42), (1.0, 0.0)])
    shake = math.sin(2.0 * math.pi * 10.0 * t) * math.exp(-5.0 * t)

    p["root"].pos[2] += k.H * 0.070 * hit
    p["root"].pos[1] += -k.H * 0.045 * hit
    p["hips"].rot[0] += 8.0 * hit + 1.4 * shake
    p["hips"].rot[2] += 5.0 * hit
    p["trunk_front"].rot[0] += 11.0 * hit + 2.0 * shake
    p["trunk_front"].rot[2] += -6.0 * hit

    rig.neck_curve(p, pitch=17.0 * hit + 3.0 * shake, roll=-7.0 * hit, bias=1.1)
    p["skull"].rot[0] += -14.0 * hit + 4.0 * shake
    p["skull"].rot[2] += 9.0 * hit
    p["jaw"].rot[0] += 26.0 * hit

    rig.wings(p, elev=16.0 * hit, sweep=30.0 * hit, flex=-8.0 * hit,
              shrug=12.0 * hit + 3.0 * shake)
    rig.tail_pose(p, pitch=10.0 * hit, roll=6.0 * hit)
    rig.plant(p, k.rest)
    return p


def anim_death(rig: VultureRig, t: float) -> Pose:
    """倒地：腿先软 → 身子沉 → 侧翻 → 翼摊开 → 头最后落下。

    侧翻绕的是 y=0 那条轴而身体横跨中线，翻过去时背离侧会转到地面以下 —— 必须逐帧把
    最低点夹回地面（狮子那边写死下沉量，实测颈根扎进地下 1.2 米）。
    """
    k = K(rig)
    p = Pose()
    buckle = keyed(t, [(0.0, 0.0), (0.34, 1.0)])
    sink = keyed(t, [(0.16, 0.0), (0.58, 1.0)])
    roll = keyed(t, [(0.34, 0.0), (0.78, 1.0)])
    headfall = keyed(t, [(0.50, 0.0), (0.90, 1.0)])

    # 腿一软身子就得**掉下去**：光靠关节角摆不出"倒地"，早先整只从头到尾悬在站立高度
    # 上翻了个身。给一个超过体高的下沉量，再由 ground_clamp 把最低点顶回地面 —— 谁先
    # 触地由几何决定，不用我猜。
    p["root"].pos[1] += -k.H * 0.90 * sink
    p["root"].rot[2] += 72.0 * roll
    p["hips"].rot[0] += 8.0 * sink
    p["trunk_front"].rot[0] += -12.0 * buckle + 6.0 * sink

    rig.neck_curve(p, pitch=-10.0 * buckle - 30.0 * headfall, roll=-9.0 * roll, bias=1.3)
    p["skull"].rot[0] += 6.0 * buckle - 22.0 * headfall
    p["skull"].rot[2] += -12.0 * roll
    p["jaw"].rot[0] += 16.0 * buckle + 8.0 * (1.0 - headfall)

    rig.wings(p, elev=10.0 * buckle - 14.0 * sink, sweep=26.0 * buckle + 12.0 * roll,
              flex=-12.0 * buckle, shrug=-8.0 * sink, twist=10.0 * roll)
    rig.tail_pose(p, pitch=12.0 * (1.0 - sink) - 6.0 * sink, roll=-8.0 * roll)

    # 腿：先曲（脚还锁在地上）后瘫（随身体翻过去蜷起来）。
    # 不能让脚全程锁在静止落点上：身体侧翻 72° 之后那两个落点在髋的可达域外，闭式解只
    # 能顶着可达环边界解，相邻帧一跳三十几度 —— 后验的"抽搐"就是抓这个。死了的鸟腿本来
    # 也该是蜷着的，翻过去的过程正好把逆解交还给蜷腿姿。
    for s, tgt in k.rest.items():
        pull = np.array([0.0, k.H * 0.02 * buckle, -0.14 * k.H * buckle])
        rig.solve_foot(p, s, tgt + pull, pitch=-20.0 * buckle)
    rig.tuck_legs(p, amount=0.80 * roll)
    rig.ground_clamp(p)
    return p


_UNFOLD: dict[str, Pose] = {}


def unfold_of(rig: VultureRig) -> Pose:
    """这具收翼绑定姿摆成展翼外形所需的姿态（解一次缓存住，见 rig.unfold_pose）。"""
    key = str(rig.path)
    if key not in _UNFOLD:
        spread = VultureRig(str(rig.path).replace(str(MODELS), str(MODELS / "layers"))
                            .replace(".bbmodel", "_spread.bbmodel"))
        _UNFOLD[key] = unfold_pose(rig, spread)
    return _UNFOLD[key]


def anim_unfold(rig: VultureRig, t: float) -> Pose:
    """展翼：从收翼站姿把两翼张满。起飞前的那一下，也是威慑升级的第二段。

    翼的全部通道（每根羽的朝向 / 羽根位置 / 长度与截面）都由 rig.unfold_pose 从两份模型
    解出来，这里只负责**怎么张开**：身体先微沉蓄一下、翼过冲一点再落回，末帧正好停在展翼
    外形上 —— 所以它可以直接接飞行动作，不会有跳变。
    """
    k = K(rig)
    w = keyed(t, [(0.0, 0.0), (0.14, 0.05), (0.60, 1.0), (0.76, 1.05), (1.0, 1.0)])
    p = blend_pose(rig, unfold_of(rig), w)
    sink = keyed(t, [(0.0, 0.0), (0.18, 1.0), (0.46, 0.0)])
    p["root"].pos[1] += -k.H * 0.055 * sink + k.H * 0.020 * w
    p["hips"].rot[0] += -5.0 * sink + 3.0 * w
    p["trunk_front"].rot[0] += -4.0 * sink + 5.0 * w
    rig.neck_curve(p, pitch=-8.0 * sink + 6.0 * w, bias=1.3)
    p["skull"].rot[0] += 4.0 * sink + 5.0 * w
    p["jaw"].rot[0] += 3.0 + 9.0 * sink
    rig.tail_pose(p, pitch=-6.0 * w + 4.0 * sink)
    rig.plant(p, k.rest)
    return p


def anim_fold(rig: VultureRig, t: float) -> Pose:
    """收翼：从展翼收回体侧，末尾抖一下把羽理顺。落地站定之后接的就是它。"""
    k = K(rig)
    w = keyed(t, [(0.0, 1.0), (0.12, 1.04), (0.66, 0.0), (1.0, 0.0)])
    p = blend_pose(rig, unfold_of(rig), w)
    settle = keyed(t, [(0.60, 0.0), (0.74, 1.0), (1.0, 0.0)])
    shake = math.sin(2.0 * math.pi * 12.0 * t) * settle
    p["root"].pos[1] += k.H * 0.018 * w - k.H * 0.020 * settle
    p["hips"].rot[0] += 3.0 * w + 2.0 * shake
    p["trunk_front"].rot[0] += 4.0 * w + 2.6 * shake
    p["trunk_front"].rot[2] += 2.4 * shake
    rig.neck_curve(p, pitch=5.0 * w - 6.0 * settle + 1.2 * shake, bias=1.4)
    p["skull"].rot[0] += 4.0 * w + 5.0 * settle
    p["jaw"].rot[0] += 2.0 + 4.0 * settle
    rig.tail_pose(p, pitch=-5.0 * w + 5.0 * settle)
    rig.plant(p, k.rest)
    return p


# ================================================================ 飞行动作

def wing_beat(rig: VultureRig, p: Pose, phase: float, *, amp_up: float, amp_dn: float,
              tuck: float = 1.0) -> None:
    """一次拍翼。phase ∈ [0,1)：0 = 上死点，下扑占前 45%，上挥占后 55%。

    下扑快、上挥慢是真鸟的节奏（下扑做功、上挥只是收回），倒过来读着像在划水。上挥时
    前臂与手必须**折起来**减小迎风面积，不折的话上挥等于反向推力，翅膀白拍。
    """
    down = phase < 0.45
    s = phase / 0.45 if down else (phase - 0.45) / 0.55
    if down:
        elev = amp_up + (-amp_dn - amp_up) * smooth(s)
        # 下扑全程翼是**摊平**的：这一记要吃满迎风面积。早先让它从 34° 折角起手，于是
        # 上死点两侧一边 34°、一边 0°，循环接缝当场撕开 34 度（后验一眼抓到）。
        flex = 0.0
        twist = -14.0 * math.sin(math.pi * s)
        sweep = 9.0 * math.sin(math.pi * s)
    else:
        elev = -amp_dn + (amp_up + amp_dn) * smooth(s)
        flex = 34.0 * tuck * math.sin(math.pi * s) ** 0.7
        twist = 9.0 * math.sin(math.pi * s)
        sweep = -7.0 * math.sin(math.pi * s)
    rig.wings(p, elev=elev, sweep=sweep, twist=twist, flex=flex, hand=flex * 0.55,
              hand_twist=-twist * 0.5)


def anim_flap(rig: VultureRig, t: float) -> Pose:
    """振翅：一个完整拍翼周期。躯干在下扑末端被抬起来 —— 升力不是凭空的。"""
    k = K(rig)
    p = Pose()
    wing_beat(rig, p, t, amp_up=40.0, amp_dn=34.0)
    # 躯干起伏比翼慢四分之一拍：下扑做功、身子随后才被顶上去
    p["root"].pos[1] += 0.055 * k.H * math.sin(2.0 * math.pi * (t - 0.28))
    p["hips"].rot[0] += 2.6 * math.cos(2.0 * math.pi * (t - 0.20))
    rig.neck_curve(p, pitch=-6.0 - 3.0 * math.sin(2.0 * math.pi * (t - 0.10)), bias=1.2)
    p["skull"].rot[0] += 7.0 + 2.0 * math.sin(2.0 * math.pi * (t - 0.10))
    rig.tail_pose(p, pitch=5.0 + 3.5 * math.sin(2.0 * math.pi * (t - 0.32)))
    rig.tuck_legs(p)
    return p


def anim_glide(rig: VultureRig, t: float) -> Pose:
    """滑翔：翼撑成微上反角，只有极慢的气流修正。秃鹫一天里九成时间是这个姿态。

    这条动作的难点是**克制**：一旦给了明显的周期性动作，观感立刻从"乘着上升气流"掉成
    "在扇翅膀"。振幅压在 3° 上下，靠尾和头的错相位维持"活着"的感觉。
    """
    p = Pose()
    slow = math.sin(2.0 * math.pi * t)
    slow2 = math.sin(2.0 * math.pi * 2.0 * t - 0.8)
    rig.wings(p, elev=7.0 + 2.6 * slow, sweep=-3.0 + 1.6 * slow2, twist=2.0 * slow2,
              flex=3.0 - 2.0 * slow, hand=2.0 * slow2, hand_twist=-2.5 * slow)
    p["hips"].rot[2] += 2.2 * slow                    # 微侧倾 = 在盘旋
    p["hips"].rot[0] += -1.4 + 0.8 * slow2
    rig.neck_curve(p, pitch=-8.0, yaw=7.0 * slow, bias=1.4)
    p["skull"].rot[0] += 9.0
    p["skull"].rot[1] += 9.0 * math.sin(2.0 * math.pi * t - 1.2)
    rig.tail_pose(p, pitch=3.0 + 1.8 * slow2, roll=-3.0 * slow, yaw=-2.4 * slow)
    rig.tuck_legs(p)
    return p


def anim_takeoff(rig: VultureRig, t: float) -> Pose:
    """起飞：蹲伏蓄力 → 翼上举 → 蹬地 + 首次下扑 → 收腿。

    大型秃鹫起飞是**跳**出来的：先把翼举到顶，蹬地的同时第一记下扑落下，两者错开就飞
    不起来。离地前脚必须仍锁在地上逆解，否则蓄力帧脚会陷进土里。
    """
    k = K(rig)
    p = Pose()
    crouch = keyed(t, [(0.0, 0.0), (0.20, 1.0), (0.30, 0.85), (0.38, 0.0)])
    push = keyed(t, [(0.20, 0.0), (0.40, 1.0), (0.56, 0.4), (0.74, 0.0)])
    air = keyed(t, [(0.34, 0.0), (0.52, 1.0)])          # 离地程度（也用来混收腿）
    climb = keyed(t, [(0.36, 0.0), (1.0, 1.0)])

    # 翼：0→0.30 举到顶，0.30→0.60 第一记下扑，之后接第二拍的上挥
    beat = keyed(t, [(0.0, 0.0), (0.30, 0.0), (0.62, 0.45), (1.0, 0.86)])
    wing_beat(rig, p, beat, amp_up=48.0, amp_dn=30.0, tuck=0.85)

    # 爬升量给得克制：实体的世界坐标由引擎推，动画只负责「离地」这一段读感；给满一个
    # 体高的话，切到飞行态的那一帧模型会往下掉一大截。
    p["root"].pos[1] += -k.H * 0.13 * crouch + k.H * 0.60 * climb
    p["root"].pos[2] += k.H * 0.05 * crouch - k.H * 0.22 * climb
    # 蓄力时压低（负 = 低头），蹬地与爬升都要**抬头**（正）—— 爬升给负号的话，鸟一边
    # 往上窜一边把头栽下去，读起来是坠机不是起飞。
    p["hips"].rot[0] += -6.0 * crouch + 12.0 * push + 8.0 * climb
    p["trunk_front"].rot[0] += -5.0 * crouch + 8.0 * push

    rig.neck_curve(p, pitch=-14.0 * crouch + 10.0 * push - 6.0 * climb, bias=1.2)
    p["skull"].rot[0] += 10.0 * crouch + 6.0 * push
    p["jaw"].rot[0] += 5.0 + 16.0 * push
    rig.tail_pose(p, pitch=-14.0 * crouch + 10.0 * push + 4.0 * climb)

    if air < 0.999:                                    # 还有脚在地上：锁地逆解
        for s, tgt in k.rest.items():
            rig.solve_foot(p, s, tgt + np.array([0.0, 0.0, -0.05 * k.H * push]),
                           pitch=-30.0 * push)
    rig.tuck_legs(p, amount=air)
    return p


def anim_land(rig: VultureRig, t: float) -> Pose:
    """降落：翼前扑刹车（flare）→ 腿前伸探地 → 触地缓冲 → 翼仍高举稳住。

    刹车靠的是把整片翼转到迎风面（前掠 + 大迎角），不是靠拍。结尾**不收翼** —— 秃鹫落
    地后还会举着翼站一会儿；这里也正好把收翼交给切回地面绑定姿的那一刻，避免在展翼几何
    上做收翼动作（羽是按姿态烘焙的，硬折会露馅）。
    """
    k = K(rig)
    p = Pose()
    flare = keyed(t, [(0.0, 0.0), (0.30, 1.0), (0.52, 0.9), (0.78, 0.35), (1.0, 0.22)])
    # reach 一路留在 1：它是"腿伸出去了没有"，不是一次挥动。早先让它在触地后衰减回 0，
    # 于是鸟落地之后又把腿蜷了回去，末帧整只悬在离地半个身位的空中。
    reach = keyed(t, [(0.04, 0.0), (0.46, 1.0), (1.0, 1.0)])
    stand = keyed(t, [(0.46, 0.0), (0.88, 1.0), (1.0, 1.0)])   # 落点从前伸收回静止站姿
    touch = keyed(t, [(0.50, 0.0), (0.62, 1.0), (0.82, 0.25), (1.0, 0.0)])
    drop = keyed(t, [(0.0, 1.0), (0.58, 0.0)])          # 高度：从空中落到地面

    rig.wings(p, elev=26.0 + 30.0 * flare, sweep=34.0 * flare, twist=20.0 * flare,
              flex=-6.0 * flare, hand=-10.0 * flare, hand_twist=16.0 * flare)
    body_y = k.H * (0.55 * drop - 0.10 * touch)
    p["root"].pos[1] += body_y
    p["root"].pos[2] += -k.H * 0.16 * drop
    # 刹车 = 整只**仰**起来把翼面迎向气流（正），触地后再落回水平（负）
    p["hips"].rot[0] += 20.0 * flare - 9.0 * touch
    p["trunk_front"].rot[0] += 12.0 * flare - 6.0 * touch
    rig.neck_curve(p, pitch=-6.0 - 10.0 * touch, bias=1.1)
    p["skull"].rot[0] += 12.0 * flare + 8.0 * touch
    p["jaw"].rot[0] += 4.0 + 14.0 * flare
    rig.tail_pose(p, pitch=-22.0 * flare + 8.0 * touch)

    # 腿：先按落点逆解，**再**往收腿姿混 —— 顺序反了就不是混合而是覆盖：逆解是直接写
    # 死 rot 的，摆在它前面的收腿姿会被整根抹掉，腿在 reach 刚过阈值那一帧硬跳 78°。
    # 落点**跟着躯干走**，不是钉在地面上：还在空中时躯干高出静止姿 0.55H，脚若仍以地面
    # 为目标，腿要凭空拉长半个体高 —— 闭式解只能顶在可达环边界上，相邻帧一跳三十度。
    # 跟随之后腿保持静止姿的伸展量，drop 归零时目标自然落到地面。
    for s, tgt in k.rest.items():
        aim = tgt + np.array([0.0, body_y, -k.H * 0.20 * (1.0 - stand) * reach])
        rig.solve_foot(p, s, aim, pitch=-24.0 * reach * (1.0 - stand))
    rig.tuck_legs(p, amount=1.0 - reach)
    if drop < 0.02:
        rig.ground_clamp(p)
    return p


def anim_dive(rig: VultureRig, t: float) -> Pose:
    """俯冲扑击：翼后掠收成三角 → 压头下坠 → 临击前爪前甩、翼张开刹车。

    秃鹫大多吃死物，但饿到极处会去啄将死之物 —— 这条动作要的是那种**扑下来抢**的压迫
    感：收翼段身子几乎垂直，最后一下爪子先到。
    """
    k = K(rig)
    p = Pose()
    fold = keyed(t, [(0.0, 0.0), (0.22, 1.0), (0.62, 1.0), (0.76, 0.15), (1.0, 0.0)])
    plunge = keyed(t, [(0.10, 0.0), (0.34, 1.0), (0.66, 1.0), (0.86, 0.2), (1.0, 0.0)])
    strike = keyed(t, [(0.62, 0.0), (0.76, 1.0), (0.90, 0.6), (1.0, 0.2)])

    rig.wings(p, elev=-8.0 * fold + 46.0 * strike, sweep=-40.0 * fold + 20.0 * strike,
              twist=-16.0 * fold + 24.0 * strike, flex=52.0 * fold - 20.0 * strike,
              hand=40.0 * fold - 14.0 * strike, hand_twist=12.0 * fold)
    p["hips"].rot[0] += -34.0 * plunge + 26.0 * strike
    # 俯冲的高度同样由引擎推实体，动画只留一点下坠读感 —— 给满会让模型扎到实体脚下
    p["root"].pos[1] += -k.H * 0.16 * plunge
    p["root"].pos[2] += -k.H * 0.12 * plunge
    rig.neck_curve(p, pitch=-18.0 * plunge + 8.0 * strike, bias=1.0)
    p["skull"].rot[0] += 26.0 * plunge - 10.0 * strike
    p["jaw"].rot[0] += 6.0 + 26.0 * strike
    rig.tail_pose(p, pitch=-8.0 * fold + 26.0 * strike)

    rig.tuck_legs(p, amount=1.0 - strike)
    if strike > 0.02:                                   # 爪前甩：绕髋整条腿往前抡
        for s in ("l", "r"):
            chain = rig.leg(s)
            p[chain[0]].rot[0] += 26.0 * strike
            p[chain[1]].rot[0] += -18.0 * strike
            p[chain[2]].rot[0] += 34.0 * strike
            p[chain[3]].rot[0] += -30.0 * strike
    return p


# ================================================================ 动画表

@dataclass
class Clip:
    length: float
    loop: bool
    samples: int
    fn: Callable[[VultureRig, float], Pose]
    kind: str                        # "ground"（收翼绑定姿）/ "flight"（展翼绑定姿）
    at: tuple[float, ...] = ()       # 必须落采样点的时刻（动作里的真折角，见 build_tracks）


# 采样数是**量出来的**，不是拍脑袋：拿最大档（最苛刻）逐档扫 24…144，取导出位移偏差稳定
# 落在 0.55px 以下的最小值（连着两档都达标才算，躲开偶然的低谷）。所以数字参差不齐 ——
# 振翅要 96 是因为翼尖离肩四十多个单位，同样的角度误差在那儿放大最狠；滑翔只要 28 是因为
# 它整条几乎不动。冗余帧交给 animkit 的力臂加权共线裁剪收回去。
ANIMS: dict[str, Clip] = {
    "idle":    Clip(6.40, True, 48, anim_idle, "ground"),
    "walk":    Clip(1.15, True, 48, anim_walk, "ground"),
    "run":     Clip(0.68, True, 64, anim_run, "ground"),
    "peck":    Clip(0.95, False, 88, anim_peck, "ground"),
    "threat":  Clip(1.90, False, 72, anim_threat, "ground"),
    "hurt":    Clip(0.50, False, 64, anim_hurt, "ground"),
    "death":   Clip(2.60, False, 64, anim_death, "ground"),
    "unfold":  Clip(0.55, False, 56, anim_unfold, "ground"),
    "fold":    Clip(0.62, False, 80, anim_fold, "ground"),
    "flap":    Clip(0.92, True, 96, anim_flap, "flight", at=(0.45,)),
    "glide":   Clip(4.20, True, 28, anim_glide, "flight"),
    "takeoff": Clip(1.30, False, 56, anim_takeoff, "flight"),
    "land":    Clip(1.20, False, 72, anim_land, "flight"),
    "dive":    Clip(1.50, False, 104, anim_dive, "flight"),
}
GROUND = [n for n, c in ANIMS.items() if c.kind == "ground"]
FLIGHT = [n for n, c in ANIMS.items() if c.kind == "flight"]


def sample(rig: VultureRig, name: str, t01: float) -> Pose:
    return ANIMS[name].fn(rig, t01)


# ================================================================ 导出

def _tracks(rig: VultureRig, name: str) -> dict:
    c = ANIMS[name]
    return {
        "name": name,
        "length": c.length,
        "loop": c.loop,
        "tracks": build_tracks(rig, lambda t: sample(rig, name, t), c.length, c.loop,
                               c.samples, c.at),
    }


def build(size: str, morph: str, names: list[str]) -> list[tuple[str, Path, list[dict]]]:
    tag = {"small": "Small", "mid": "Mid", "large": "Large"}[size]
    out = []
    for kind, suffix in (("ground", ""), ("flight", "Flight")):
        picked = [n for n in names if ANIMS[n].kind == kind]
        if not picked:
            continue
        rig = default_rig(size, morph, spread=(kind == "flight"))
        anims = [_tracks(rig, n) for n in picked]
        model = f"FuyuVultureRig{tag}{suffix}"
        path = MODELS / f"{model}.bbmodel"
        write_animated_bbmodel(rig, anims, path, model)
        # 落盘后回读断言：格式必须是 4.x 且骨骼内联在 outliner 里。5.0 把骨骼拆去
        # groups[]，4.x 的读盘器重建不出骨树 —— 打开是空场景，不报任何错。这条不能只靠
        # 写盘那一侧的兜底，兜底本身也会被改坏。
        doc = json.loads(path.read_text())
        fv = doc.get("meta", {}).get("format_version", "?")
        inline = doc["outliner"] and isinstance(doc["outliner"][0], dict) and "name" in doc["outliner"][0]
        if fv.startswith("5") or doc.get("groups") or not inline:
            raise SystemExit(f"{model}: 落盘成了 format_version={fv} "
                             f"groups={len(doc.get('groups', []))} 骨骼内联={inline}；"
                             f"4.x 的 Blockbench 打开会是空场景")
        out.append((model, path, anims))
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫动画生成")
    ap.add_argument("--size", choices=SIZES, help="只出一档（默认三档全出）")
    ap.add_argument("--morph", default="jin", help="用哪个变色的几何当绑定姿")
    ap.add_argument("--only", nargs="*", help="只生成这些动画")
    args = ap.parse_args()

    names = args.only or list(ANIMS)
    bad = [n for n in names if n not in ANIMS]
    if bad:
        raise SystemExit(f"没有这些动画: {bad}（现有 {list(ANIMS)}）")

    for size in ([args.size] if args.size else list(SIZES)):
        allanims: list[dict] = []
        for model, path, anims in build(size, args.morph, names):
            kf = sum(len(v) for a in anims for c in a["tracks"].values() for v in c.values())
            print(f"  {model:<26} 动作 {len(anims):2d}  关键帧 {kf}")
            for a in anims:
                print(f"      {a['name']:<8} {a['length']:4.2f}s "
                      f"{'循环' if a['loop'] else '单次'}  骨 {len(a['tracks']):2d}")
            allanims += anims
        js = MODELS / f"{MODEL_ID}_{size}.animation.json"
        write_geckolib(allanims, js, NAMESPACE, MODEL_ID)
        print(f"  → {js.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
