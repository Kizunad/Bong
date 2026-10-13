#!/usr/bin/env python3
"""冻髓鹿动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧，没有套用旧动画）：

  idle    3.2s 循环  呼吸起伏 / 头颈微晃 / 耳抽动 / 鹿角晶枝轻颤 / 尾梢一摆；每个呼吸周期呼出一口白雾（breath 骨缩放 + 前飘）
  walk    1.4s 循环  对角步态（左前 + 右后一组），低头寻路式点头，每两步一口白雾
  run     0.7s 循环  腾跃奔跑：四足有腾空相，脊椎弓伸，脖子前伸，白雾被气流带向身后
  attack  1.1s 一次  仰头蓄力（前腿刨地） → 低头顶角（颈压到最低，整只前冲，冰晶角尖朝前） → 甩角上挑 → 收回
  hurt    0.45s 一次  被击后仰，头甩起，前腿缩，鹿角晶枝震颤，白雾一口喷出
  death   1.9s 一次  踉跄 → 前腿折跪 → 侧倒 → 四肢松开，鹿角晶枝散垂，白雾最后一口缓缓散尽

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴。
对垂下的肢体，rot.x 为正 = 脚尖向前（-Z）摆；对头 / 颈，rot.x 为正 = 抬头；对下颌，rot.x 为负 = 张口。
白雾骨 breath 不转，只用 scale（0 = 散尽）和 pos（z 为负 = 向前飘）。

用法::

    python3 modelScript/creatures/dong_sui_lu/gen_anim.py        # 出 geo / animation / 贴图，并在 out/ 下做审阅资产目录
"""

from __future__ import annotations

import json
import math
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Callable

from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, write_animated_bbmodel

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CODEC = ROOT / "modelScript" / "exporters" / "creature_codec.cjs"
sys.path.insert(0, str(HERE.parent.parent / "tools"))

NAME = "dong_sui_lu"
SOURCE = HERE / "DongSuiLu_tex.bbmodel"                    # 带贴图的作者 rig（gen_dong_sui_lu → gen_tex 先出）
ANIMATED = HERE / "DongSuiLu_animated.bbmodel"
EXPORT_DIR = HERE / "export"                                # 安装就绪的 geo / animation / 贴图（不装进 client，等接线）
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME   # 审阅 GIF 用的资源根（gitignored）
SAMPLES = 28                                                # 每段动画的采样数，关键帧再按像素容差裁共线点

Sampler = Callable[[float], Pose]
SIDES = (("l", -1.0), ("r", 1.0))


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t ∈ [0,1] 上走 cycles 个整周期的正弦，循环动画首尾自然相接。"""
    return math.sin(math.tau * (cycles * t + phase))


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep：points 是 (t01, 值) 的有序表，一次性动作用它排节奏。"""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


def frac(x: float) -> float:
    return x - math.floor(x)


def turn(pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0)) -> None:
    """往 pose 里累加一根骨的旋转 / 位移（同一根骨多处调用会叠加）。"""
    channel = pose[bone]
    channel.rot = [channel.rot[i] + rot[i] for i in range(3)]
    channel.pos = [channel.pos[i] + pos[i] for i in range(3)]


# ---------------------------------------------------------------- 腿
class Gait:
    """一种步态的参数：占空比、前后腿摆幅、抬腿折角。"""

    def __init__(self, duty: float, fore_swing: float, hind_swing: float, fore_fold: float, hind_fold: float):
        self.duty, self.fore_swing, self.hind_swing = duty, fore_swing, hind_swing
        self.fore_fold, self.hind_fold = fore_fold, hind_fold


WALK = Gait(duty=0.66, fore_swing=20.0, hind_swing=18.0, fore_fold=46.0, hind_fold=34.0)
RUN = Gait(duty=0.36, fore_swing=38.0, hind_swing=36.0, fore_fold=66.0, hind_fold=52.0)


def stride(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (摆角, 抬腿弧)。支撑相脚从前往后划（摆角 +swing → -swing），摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    s = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(s) - 1.0), math.sin(math.pi * s)


def fore_leg(pose: Pose, side: str, u: float, gait: Gait) -> None:
    angle, lift = stride(u, gait.duty, gait.fore_swing)
    turn(pose, f"arm_{side}", rot=(angle, 0, 0))
    turn(pose, f"forearm_{side}", rot=(-gait.fore_fold * lift * 0.8 - 0.25 * max(0.0, angle), 0, 0))
    turn(pose, f"cannon_f_{side}", rot=(gait.fore_fold * lift * 0.55 + 0.2 * angle, 0, 0))
    turn(pose, f"hoof_f_{side}", rot=(-16.0 * lift, 0, 0))


def hind_leg(pose: Pose, side: str, u: float, gait: Gait) -> None:
    angle, lift = stride(u, gait.duty, gait.hind_swing)
    turn(pose, f"thigh_{side}", rot=(angle, 0, 0))
    turn(pose, f"shin_{side}", rot=(-gait.hind_fold * lift * 0.8 - 0.3 * max(0.0, angle), 0, 0))
    turn(pose, f"hock_{side}", rot=(gait.hind_fold * lift * 0.75 + 0.2 * angle, 0, 0))
    turn(pose, f"cannon_b_{side}", rot=(gait.hind_fold * 0.2 * lift, 0, 0))
    turn(pose, f"hoof_b_{side}", rot=(-14.0 * lift, 0, 0))


def tail_flick(pose: Pose, t: float, amp: float, cycles: float, lift: float = 0.0) -> None:
    """四节短尾行波：越靠尾梢相位越晚、幅度越大。lift > 0 尾根上翘。"""
    for i in range(1, 5):
        k = i / 4.0
        turn(pose, f"tail_{i}", rot=(lift * (1.0 - 0.5 * k), amp * (0.4 + 0.6 * k) * wave(t, cycles, -0.1 * i), 0))


def ears(pose: Pose, left: float, right: float) -> None:
    turn(pose, "ear_l", rot=(left, 0, 4.0 * left))
    turn(pose, "ear_r", rot=(right, 0, -4.0 * right))


def antlers(pose: Pose, sway: float, spread: float = 0.0) -> None:
    """鹿角晶枝：整副角随头动，枝梢比根部更晃；sway 是左右晃（度），spread 是冠叉向外撑开（度）。"""
    for side, sign in SIDES:
        turn(pose, f"antler_beam_{side}", rot=(0.6 * sway, 0, 0.5 * sway * sign))
        turn(pose, f"antler_crown_{side}", rot=(1.0 * sway, 0, (1.0 * sway + spread) * sign))
        turn(pose, f"antler_brow_{side}", rot=(-0.6 * sway, 0, 0.4 * sway * sign))


def crystals(pose: Pose, shake: float) -> None:
    """背脊与肩胛的结晶簇：身体动得快时轻微反向抖，像是插在毛里的硬物。"""
    turn(pose, "crystal_withers", rot=(0.8 * shake, 0, 0.5 * shake))
    turn(pose, "crystal_back_a", rot=(-0.7 * shake, 0, -0.4 * shake))
    turn(pose, "crystal_back_b", rot=(0.6 * shake, 0, 0.5 * shake))
    for side, sign in SIDES:
        turn(pose, f"crystal_shoulder_{side}", rot=(0.5 * shake, 0, 0.8 * shake * sign))


def mist(pose: Pose, puff: float, drift: float = 1.0, lift: float = 0.0) -> None:
    """白雾：puff ∈ [0,1] 是这一口气的「大小」（0 = 散尽，骨缩到 0.001），drift 越大飘得越远，lift 向上飘。
    五块方块沿嘴前排成一线，整体缩放就是「呼出 → 散开」。"""
    size = max(0.001, puff)
    channel = pose["breath"]
    channel.scale = [size, size, size * (0.6 + 0.4 * puff)]
    turn(pose, "breath", pos=(0, 0.8 * lift * puff, -2.2 * drift * puff))


def exhale(t: float, start: float, length: float) -> float:
    """一口气：start 时刻开始，快速鼓起、缓慢散尽，返回 0..1 的大小。"""
    if not (start <= t <= start + length):
        return 0.0
    u = (t - start) / length
    return smooth(u / 0.25) * (1.0 - smooth((u - 0.4) / 0.6))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "thorax", rot=(0.9 * breath, 0, 0), pos=(0, 0.1 * breath, 0))
    turn(pose, "spine_lumbar", rot=(-0.5 * breath, 0, 0))
    turn(pose, "neck_base", rot=(2.0 * wave(t, 1, 0.1) - 1.0, 4.0 * wave(t, 1, 0.3), 0))
    turn(pose, "neck_upper", rot=(1.5 * wave(t, 1, 0.2), 5.0 * wave(t, 1, 0.35), 0))
    turn(pose, "head", rot=(1.0 * wave(t, 2, 0.05), 3.5 * wave(t, 1, 0.45), 1.5 * wave(t, 1, 0.1)))
    turn(pose, "jaw", rot=(-2.0 - 3.0 * max(0.0, wave(t, 1, 0.55)), 0, 0))
    flick = math.sin(math.pi * smooth((t - 0.3) / 0.1)) if 0.3 < t < 0.4 else 0.0
    ears(pose, -3.0 * wave(t, 1, 0.2) - 20.0 * flick, 2.0 * wave(t, 1, 0.6))
    antlers(pose, sway=1.2 * wave(t, 1, 0.45))
    crystals(pose, 0.6 * breath)
    tail_flick(pose, t, amp=5.0, cycles=1, lift=-2.0 * smooth((t - 0.6) / 0.1) * (1.0 - smooth((t - 0.75) / 0.1)))
    mist(pose, exhale(t, 0.45, 0.45), drift=1.0, lift=0.5)
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    phases = {"l": 0.0, "r": 0.5}                   # 左前 + 右后一组
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + phases[side]), WALK)
        hind_leg(pose, side, frac(t + phases[side] + 0.5), WALK)
    # 脊椎蛇行：肩和臀反向转，头随步点头
    turn(pose, "hips", rot=(0, 2.2 * wave(t, 1), 0), pos=(0, 0.2 * abs(wave(t, 2, 0.25)), 0))
    turn(pose, "spine_lumbar", rot=(0, -2.6 * wave(t, 1), 0))
    turn(pose, "thorax", rot=(0.7 * wave(t, 2, 0.1), -2.2 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(-3.0 + 2.5 * wave(t, 2, 0.35), 3.0 * wave(t, 1), 0))
    turn(pose, "neck_upper", rot=(-2.0 + 2.0 * wave(t, 2, 0.4), 2.5 * wave(t, 1), 0))
    turn(pose, "head", rot=(-3.0 * wave(t, 2, 0.45), 1.0 * wave(t, 1), 0))
    turn(pose, "jaw", rot=(-3.0, 0, 0))
    ears(pose, 4.0 * wave(t, 2, 0.1), 4.0 * wave(t, 2, 0.6))
    antlers(pose, sway=2.5 * wave(t, 2, 0.5))
    crystals(pose, 1.6 * wave(t, 2, 0.2))
    tail_flick(pose, t, amp=7.0, cycles=1)
    mist(pose, max(exhale(t, 0.05, 0.4), exhale(t, 0.55, 0.4)), drift=1.2, lift=0.3)
    return pose


def run(t: float) -> Pose:
    pose = Pose()
    # 腾跃步态：前腿一对略错开，后腿一对略错开，前后相隔半周期；占空比 0.36 → 有腾空相
    offsets = {"fore": {"l": 0.0, "r": 0.1}, "hind": {"l": 0.5, "r": 0.6}}
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + offsets["fore"][side]), RUN)
        hind_leg(pose, side, frac(t + offsets["hind"][side]), RUN)
    # 脊椎弓伸：后腿蹬出时拱背，前腿着地时伸展，整只身体上下起伏
    arch = wave(t, 1, 0.1)
    turn(pose, "hips", rot=(-4.0 * arch, 1.0 * wave(t, 1), 0), pos=(0, 0.8 * (0.5 + 0.5 * wave(t, 1, 0.3)), 0))
    turn(pose, "spine_lumbar", rot=(5.5 * arch, -1.4 * wave(t, 1), 0))
    turn(pose, "thorax", rot=(-4.5 * arch, 1.0 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(-14.0 + 3.0 * arch, 0, 0))       # 脖子前伸压低
    turn(pose, "neck_upper", rot=(-8.0 - 2.0 * arch, 0, 0))
    turn(pose, "head", rot=(4.0 - 2.0 * arch, 0, 0))              # 头抬平，眼睛朝前
    turn(pose, "jaw", rot=(-7.0 - 3.0 * wave(t, 1, 0.3), 0, 0))
    ears(pose, 18.0 + 4.0 * wave(t, 1, 0.2), 18.0 + 4.0 * wave(t, 1, 0.45))   # 耳朵向后压
    antlers(pose, sway=3.0 * wave(t, 1, 0.55), spread=-4.0)                  # 角向后收
    crystals(pose, 3.0 * wave(t, 2, 0.1))
    tail_flick(pose, t, amp=4.0, cycles=1, lift=-10.0)
    mist(pose, 0.55 + 0.45 * wave(t, 2, 0.2), drift=-1.2, lift=0.2)         # 气流把白雾拖到身后
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.3 仰头蓄力、前腿刨地；0.3–0.5 低头顶角、整只前冲；0.5–0.68 甩角上挑；0.68–1 收回
    wind = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.45, 0.0), (1.0, 0.0)])
    lunge = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.5, 1.0), (0.68, 1.0), (1.0, 0.0)])
    toss = keys(t, [(0.0, 0.0), (0.5, 0.0), (0.58, 1.0), (0.7, 0.0), (1.0, 0.0)])
    paw = math.sin(math.tau * 2.0 * t) * smooth(t / 0.08) * (1.0 - smooth((t - 0.3) / 0.05)) if t < 0.35 else 0.0
    turn(pose, "root", pos=(0, 0, -6.0 * lunge + 0.8 * wind))
    turn(pose, "hips", rot=(4.0 * wind - 6.0 * lunge, 0, 0), pos=(0, -0.8 * wind, 0))
    turn(pose, "spine_lumbar", rot=(3.0 * wind + 4.0 * lunge, 0, 0))
    turn(pose, "thorax", rot=(-3.0 * wind - 6.0 * lunge, 0, 0))
    # 颈和头：蓄力仰头，顶角时压到最低、角尖朝前，上挑时猛抬
    turn(pose, "neck_base", rot=(12.0 * wind - 30.0 * lunge + 24.0 * toss, 0, 0))
    turn(pose, "neck_upper", rot=(10.0 * wind - 24.0 * lunge + 20.0 * toss, 0, 0))
    turn(pose, "head", rot=(6.0 * wind - 20.0 * lunge + 14.0 * toss, 5.0 * toss * wave(t, 3, 0), 0))
    turn(pose, "jaw", rot=(-4.0 * wind - 8.0 * lunge, 0, 0))
    ears(pose, 22.0 * wind + 26.0 * lunge, 22.0 * wind + 26.0 * lunge)
    antlers(pose, sway=-6.0 * lunge + 8.0 * toss, spread=-6.0 * lunge)
    crystals(pose, 4.0 * lunge * (1.0 - toss) + 5.0 * toss)
    for side, sign in SIDES:
        # 前腿：蓄力时一条刨地、一条撑住，顶出时前伸蹬地
        dig = paw if side == "l" else -0.3 * paw
        turn(pose, f"arm_{side}", rot=(-10.0 * wind + 30.0 * lunge + 24.0 * dig, 0, 0))
        turn(pose, f"forearm_{side}", rot=(20.0 * wind - 26.0 * lunge - 30.0 * max(0.0, dig), 0, 0))
        turn(pose, f"cannon_f_{side}", rot=(-8.0 * wind + 12.0 * lunge + 18.0 * max(0.0, dig), 0, 0))
        # 后腿：蓄力时蜷起，顶出时蹬直
        turn(pose, f"thigh_{side}", rot=(26.0 * wind - 30.0 * lunge, 0, 0))
        turn(pose, f"shin_{side}", rot=(-28.0 * wind + 22.0 * lunge, 0, 0))
        turn(pose, f"hock_{side}", rot=(18.0 * wind - 14.0 * lunge, 0, 0))
    tail_flick(pose, t, amp=6.0 * (1.0 - lunge), cycles=1, lift=12.0 * wind - 14.0 * lunge)
    mist(pose, 0.7 * smooth(t / 0.15) * (1.0 - smooth((t - 0.5) / 0.3)), drift=0.8, lift=0.4)   # 蓄力时一口急喘的白雾
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.15, 1.0), (0.45, 0.0)])         # 受击冲量：快进慢回
    shiver = math.sin(math.tau * 4.0 * t) * (1.0 - smooth((t - 0.15) / 0.3)) * smooth(t / 0.05)
    turn(pose, "root", pos=(0, 0, 1.4 * kick))
    turn(pose, "hips", rot=(-3.0 * kick, 0, 0))
    turn(pose, "thorax", rot=(-6.0 * kick, 3.0 * kick, 0))
    turn(pose, "neck_base", rot=(14.0 * kick, -6.0 * kick, 0))
    turn(pose, "neck_upper", rot=(12.0 * kick, -8.0 * kick, 0))
    turn(pose, "head", rot=(10.0 * kick, -6.0 * kick, 4.0 * kick))
    turn(pose, "jaw", rot=(-22.0 * kick, 0, 0))
    ears(pose, 26.0 * kick, 22.0 * kick)
    antlers(pose, sway=7.0 * shiver, spread=6.0 * kick)
    crystals(pose, 6.0 * shiver)
    for side, _ in SIDES:
        turn(pose, f"arm_{side}", rot=(-14.0 * kick, 0, 0))
        turn(pose, f"forearm_{side}", rot=(-20.0 * kick, 0, 0))
        turn(pose, f"thigh_{side}", rot=(10.0 * kick, 0, 0))
        turn(pose, f"shin_{side}", rot=(-12.0 * kick, 0, 0))
    tail_flick(pose, t, amp=3.0 * kick, cycles=2, lift=22.0 * kick)
    mist(pose, exhale(t, 0.05, 0.4), drift=1.4, lift=0.2)           # 吃痛一口急喷
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    stagger = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.3, 0.6), (1.0, 0.6)])
    fall = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.64, 1.0), (1.0, 1.0)])          # 1 = 侧倒到位
    settle = keys(t, [(0.0, 0.0), (0.64, 0.0), (0.84, 1.0), (1.0, 1.0)])       # 倒下后四肢松开
    buckle = keys(t, [(0.0, 0.0), (0.3, 0.2), (0.55, 1.0), (1.0, 1.0)])
    turn(pose, "root", rot=(0, 0, 84.0 * fall), pos=(0, 0, 1.0 * stagger))
    turn(pose, "hips", rot=(-3.0 * stagger, 0, 0), pos=(0, -3.0 * keys(t, [(0, 0), (0.3, 1.0), (1, 1.0)]), 0))
    turn(pose, "thorax", rot=(-6.0 * stagger, 3.0 * stagger, 0))
    turn(pose, "neck_base", rot=(12.0 * stagger - 14.0 * fall, -5.0 * stagger + 10.0 * settle, 0))
    turn(pose, "neck_upper", rot=(10.0 * stagger - 10.0 * fall, 6.0 * settle, 0))
    turn(pose, "head", rot=(6.0 * stagger - 8.0 * fall, 8.0 * settle, 6.0 * settle))
    turn(pose, "jaw", rot=(-18.0 * stagger - 10.0 * settle, 0, 0))
    ears(pose, 24.0 * stagger + 6.0 * settle, 20.0 * stagger + 10.0 * settle)
    antlers(pose, sway=-6.0 * settle, spread=10.0 * settle)         # 角枝散垂
    crystals(pose, -4.0 * settle)
    for side, sign in SIDES:
        turn(pose, f"arm_{side}", rot=(-18.0 * stagger + 36.0 * settle * sign, 0, 0))
        turn(pose, f"forearm_{side}", rot=(-52.0 * buckle + 14.0 * settle, 0, 0))
        turn(pose, f"cannon_f_{side}", rot=(30.0 * buckle, 0, 0))
        turn(pose, f"hoof_f_{side}", rot=(-20.0 * settle, 0, 0))
        turn(pose, f"thigh_{side}", rot=(14.0 * stagger - 30.0 * settle * sign, 0, 0))
        turn(pose, f"shin_{side}", rot=(-30.0 * buckle + 12.0 * settle, 0, 0))
        turn(pose, f"hock_{side}", rot=(26.0 * buckle, 0, 0))
        turn(pose, f"hoof_b_{side}", rot=(-14.0 * settle, 0, 0))
    tail_flick(pose, t, amp=5.0 * (1.0 - settle), cycles=1, lift=-10.0 * settle)
    mist(pose, exhale(t, 0.5, 0.5), drift=0.8, lift=0.6)             # 最后一口气，缓缓散尽
    return pose


CLIPS = {
    "idle": (3.2, True, idle),
    "walk": (1.4, True, walk),
    "run": (0.7, True, run),
    "attack": (1.1, False, attack),
    "hurt": (0.45, False, hurt),
    "death": (1.9, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只鹿（root）托起到刚好贴地。
# 只在入地的帧上移，其余帧原样不动；容差以内的轻微入地（蹄尖 / 冰凌）不动。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 5.0          # 一帧最多托起多少 px；超过说明动作本身把身体压进地里了，是动作错而不是需要托


def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < -GROUND_TOLERANCE:
            turn(pose, "root", pos=(0, -depth, 0))
        return pose

    return sample


def check_ground(clip: str, rig: PoseRig, sampler: Sampler) -> tuple[float, float]:
    """返回 (最低点, 最高腾空) 的 px，并在入地深度超过 MAX_LIFT 时直接报错。"""
    lowest, highest = math.inf, -math.inf
    for step in range(SAMPLES + 1):
        height = rig.lowest(sampler(step / SAMPLES))
        lowest, highest = min(lowest, height), max(highest, height)
    if lowest < -MAX_LIFT:
        raise ValueError(f"{NAME}.{clip}: 最低点 {lowest:.2f}px，入地超过 {MAX_LIFT}px，动作把身体压进地里了")
    return lowest, highest


def seam_gap(a: Pose, b: Pose) -> float:
    """两个姿态的最大旋转差（度）。循环动画首尾必须接上。"""
    return max((max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)) for bone in set(a) | set(b)), default=0.0)


def scale_gap(a: Pose, b: Pose) -> float:
    """两个姿态的最大缩放差。白雾骨在循环首尾必须同样大小。"""
    return max((max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)) for bone in set(a) | set(b)), default=0.0)


def build() -> list[dict]:
    rig = PoseRig(SOURCE)
    animations = []
    for clip, (length, loop, raw) in CLIPS.items():
        if loop:
            gap = seam_gap(raw(0.0), raw(1.0))
            if gap > 0.5:
                raise ValueError(f"{NAME}.{clip}: 循环首尾旋转差 {gap:.2f}° 接不上")
            if scale_gap(raw(0.0), raw(1.0)) > 0.01:
                raise ValueError(f"{NAME}.{clip}: 循环首尾缩放接不上（白雾骨）")
        sampler = grounded(rig, raw)
        lowest, highest = check_ground(clip, rig, sampler)
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})
        print(f"  {clip:7s} {length:.2f}s {'loop' if loop else 'once'}  最低点 {lowest:+.2f}px  骨轨道 {len(tracks)}")
    write_animated_bbmodel(rig, animations, ANIMATED, NAME)
    return animations


def export_assets() -> None:
    """codec 导出 geo / animation.json，贴图取 bbmodel 内嵌 PNG；同时布置审阅 GIF 用的资源根。"""
    import base64

    staging = REVIEW_ASSETS / "_codec"
    staging.mkdir(parents=True, exist_ok=True)
    subprocess.run(["node", str(CODEC), str(ANIMATED), str(staging), NAME], check=True)

    doc = json.loads(SOURCE.read_text(encoding="utf-8"))
    png = base64.b64decode(doc["textures"][0]["source"].split(",", 1)[1], validate=True)

    EXPORT_DIR.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(staging / f"{NAME}.geo.json", EXPORT_DIR / f"{NAME}.geo.json")
    shutil.copyfile(staging / f"{NAME}.animation.json", EXPORT_DIR / f"{NAME}.animation.json")
    (EXPORT_DIR / f"{NAME}.png").write_bytes(png)

    for sub in ("geo", "animations", "textures/entity/fauna"):
        (REVIEW_ASSETS / sub).mkdir(parents=True, exist_ok=True)
    shutil.copyfile(EXPORT_DIR / f"{NAME}.geo.json", REVIEW_ASSETS / "geo" / f"{NAME}.geo.json")
    shutil.copyfile(EXPORT_DIR / f"{NAME}.animation.json", REVIEW_ASSETS / "animations" / f"{NAME}.animation.json")
    (REVIEW_ASSETS / "textures" / "entity" / "fauna" / f"{NAME}.png").write_bytes(png)
    print(f"✓ 导出 {EXPORT_DIR.relative_to(ROOT)}；审阅资源根 {REVIEW_ASSETS.relative_to(ROOT)}")


if __name__ == "__main__":
    build()
    export_assets()
