#!/usr/bin/env python3
"""剥皮豺动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧；骨链和步态相位参考 dong_sui_lu / ku_pi_lang 的写法，但每一段都是按剥皮豺重写的）：

  idle    3.0s 循环  压低重心的呼吸：胸腔起伏、头颈缓缓左右扫、耳抽动、下颌微张喘息、叉尾两股一前一后摆、肩背乱毛随呼吸颤
  walk    1.2s 循环  对角小跑式踱步（左前 + 右后一组），头压低探向前，肩胛与臀反向转，尾低垂，肩背毛片滞后摆
  run     0.6s 循环  腾跃奔跑：四足有腾空相，脊椎弓伸，脖子前伸，下颌大张，叉尾两股向后劈开，毛片被气流压向身后
  attack  1.2s 一次  扑咬撕扯：伏身蓄力（后腿蜷、头后缩、下颌张开）→ 整只扑出（前爪探前、下颌张到最大）→ 一口咬合 →
                      叼住后甩头撕扯（颈来回猛甩、前爪向后拽）→ 松口收回
  hurt    0.5s 一次  被击后仰，头甩起，下颌张开，前腿缩，尾叉炸开，乱毛炸起
  death   2.0s 一次  踉跄 → 前腿折跪 → 侧倒 → 四肢松开，下颌松垂，叉尾散垂

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
骨链：root → hips → spine_mid → spine_lumbar → thorax → neck_base → neck_upper → head；后腿、尾挂在 hips，前腿挂在 thorax（所以转 hips 会带动全身）。
符号约定（和冻髓鹿一致，对着数值核过）：垂下的肢体 rot.x 为正 = 脚尖向前（-Z）摆；头 / 颈 rot.x 为正 = 抬头；
下颌：建模稿里已经张开 28°，rot.x 为正 = 向上合拢、为负 = 张得更大；叉尾：fork_N_r rot.y 为正 = 向 +X 外张，左股取反。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/bo_pi_chai/gen_anim.py
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

NAME = "bo_pi_chai"
SOURCE = HERE / "BoPiChai_tex.bbmodel"
ANIMATED = HERE / "BoPiChai_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 28

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


WALK = Gait(duty=0.62, fore_swing=22.0, hind_swing=20.0, fore_fold=44.0, hind_fold=34.0)
RUN = Gait(duty=0.34, fore_swing=40.0, hind_swing=38.0, fore_fold=68.0, hind_fold=54.0)


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
    turn(pose, f"paw_f_{side}", rot=(gait.fore_fold * lift * 0.55 + 0.2 * angle, 0, 0))
    turn(pose, f"toes_f_{side}", rot=(-16.0 * lift, 0, 0))


def hind_leg(pose: Pose, side: str, u: float, gait: Gait) -> None:
    angle, lift = stride(u, gait.duty, gait.hind_swing)
    turn(pose, f"thigh_{side}", rot=(angle, 0, 0))
    turn(pose, f"shin_{side}", rot=(-gait.hind_fold * lift * 0.8 - 0.3 * max(0.0, angle), 0, 0))
    turn(pose, f"hock_{side}", rot=(gait.hind_fold * lift * 0.75 + 0.2 * angle, 0, 0))
    turn(pose, f"paw_b_{side}", rot=(gait.hind_fold * 0.2 * lift, 0, 0))
    turn(pose, f"toes_b_{side}", rot=(-14.0 * lift, 0, 0))


# ---------------------------------------------------------------- 尾 / 耳 / 毛
def tail(pose: Pose, t: float, amp: float, cycles: float, lift: float = 0.0, spread: float = 0.0) -> None:
    """尾根两节 + 左右各三节的叉尾：越靠尾梢相位越晚、幅度越大；两股一前一后（相差半周期）摆，spread > 0 两股向外劈开。"""
    turn(pose, "tail_1", rot=(lift, 0.5 * amp * wave(t, cycles, 0.0), 0))
    turn(pose, "tail_2", rot=(lift * 0.6, 0.8 * amp * wave(t, cycles, -0.1), 0))
    for side, sign in SIDES:
        half = 0.0 if sign > 0 else 0.5
        for k in range(1, 4):
            turn(pose, f"fork_{k}_{side}", rot=(lift * 0.4 + 0.3 * amp * wave(t, cycles, half - 0.12 * k),
                                                sign * (spread + 0.6 * amp * k / 3.0 * wave(t, cycles, half - 0.12 * k + 0.25)), 0))


def ears(pose: Pose, left: float, right: float) -> None:
    turn(pose, "ear_l", rot=(left, 0, 4.0 * left))
    turn(pose, "ear_r", rot=(right, 0, -4.0 * right))


def scruff(pose: Pose, t: float, sway: float, cycles: float = 1.0, lag: float = 0.2, push_back: float = 0.0) -> None:
    """肩背乱毛：挂在肋侧 / 肩毛骨上，随身体滞后（lag）地左右抖；push_back > 0 被气流压向身后（绕 x 轴向后倒）。"""
    for bone, shift in (("fur_shoulder", 0.0), ("fur_flank", lag), ("fur_flank_b", 2.0 * lag)):
        for side, sign in SIDES:
            turn(pose, f"{bone}_{side}", rot=(-push_back * (1.0 + shift), 0, sign * sway * wave(t, cycles, -shift + (0.0 if sign > 0 else 0.2))))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "thorax", rot=(1.2 * breath, 0, 0), pos=(0, 0.12 * breath, 0))
    turn(pose, "spine_lumbar", rot=(-0.6 * breath, 0, 0))
    turn(pose, "hips", rot=(0.4 * breath, 1.0 * wave(t, 1, 0.3), 0))
    turn(pose, "neck_base", rot=(-3.0 + 2.0 * wave(t, 1, 0.1), 6.0 * wave(t, 1, 0.3), 0))      # 脖子压低、缓缓左右扫
    turn(pose, "neck_upper", rot=(-2.0 + 1.5 * wave(t, 1, 0.2), 7.0 * wave(t, 1, 0.35), 0))
    turn(pose, "head", rot=(1.0 * wave(t, 2, 0.05), 4.0 * wave(t, 1, 0.45), 2.0 * wave(t, 1, 0.1)))
    turn(pose, "jaw", rot=(10.0 - 5.0 * max(0.0, wave(t, 2, 0.15)), 0, 0))                     # 喘息：张口幅度随呼吸
    flick = math.sin(math.pi * smooth((t - 0.35) / 0.1)) if 0.35 < t < 0.45 else 0.0
    ears(pose, -3.0 * wave(t, 1, 0.2) - 22.0 * flick, 2.5 * wave(t, 1, 0.6))
    tail(pose, t, amp=6.0, cycles=1, lift=-4.0)
    scruff(pose, t, 2.0, cycles=1)
    for side, _ in SIDES:                                         # 重心压低：前爪微微扒地
        turn(pose, f"arm_{side}", rot=(2.0 * wave(t, 1, 0.5 if side == "l" else 0.0), 0, 0))
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    phases = {"l": 0.0, "r": 0.5}                   # 左前 + 右后一组
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + phases[side]), WALK)
        hind_leg(pose, side, frac(t + phases[side] + 0.5), WALK)
    turn(pose, "hips", rot=(0, 2.6 * wave(t, 1), 0), pos=(0, 0.25 * abs(wave(t, 2, 0.25)), 0))
    turn(pose, "spine_mid", rot=(0, -1.6 * wave(t, 1), 0))
    turn(pose, "spine_lumbar", rot=(0, -2.0 * wave(t, 1), 0))
    turn(pose, "thorax", rot=(0.8 * wave(t, 2, 0.1), -2.4 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(-6.0 + 2.5 * wave(t, 2, 0.35), 3.5 * wave(t, 1), 0))          # 头压低探向前
    turn(pose, "neck_upper", rot=(-5.0 + 2.0 * wave(t, 2, 0.4), 3.0 * wave(t, 1), 0))
    turn(pose, "head", rot=(-2.0 * wave(t, 2, 0.45), 1.0 * wave(t, 1), 0))
    turn(pose, "jaw", rot=(6.0, 0, 0))
    ears(pose, 6.0 + 3.0 * wave(t, 2, 0.1), 6.0 + 3.0 * wave(t, 2, 0.6))
    tail(pose, t, amp=8.0, cycles=1, lift=-6.0)
    scruff(pose, t, 3.5, cycles=2, lag=0.12)
    return pose


def run(t: float) -> Pose:
    pose = Pose()
    # 腾跃步态：前腿一对略错开，后腿一对略错开，前后相隔半周期；占空比 0.34 → 有腾空相
    offsets = {"fore": {"l": 0.0, "r": 0.1}, "hind": {"l": 0.5, "r": 0.6}}
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + offsets["fore"][side]), RUN)
        hind_leg(pose, side, frac(t + offsets["hind"][side]), RUN)
    arch = wave(t, 1, 0.1)
    turn(pose, "hips", rot=(-4.5 * arch, 1.0 * wave(t, 1), 0), pos=(0, 0.9 * (0.5 + 0.5 * wave(t, 1, 0.3)), 0))
    turn(pose, "spine_mid", rot=(3.5 * arch, -0.8 * wave(t, 1), 0))
    turn(pose, "spine_lumbar", rot=(5.0 * arch, -1.2 * wave(t, 1), 0))
    turn(pose, "thorax", rot=(-4.5 * arch, 1.0 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(-14.0 + 3.0 * arch, 0, 0))       # 脖子前伸压低
    turn(pose, "neck_upper", rot=(-8.0 - 2.0 * arch, 0, 0))
    turn(pose, "head", rot=(5.0 - 2.0 * arch, 0, 0))              # 头抬平，眼睛朝前
    turn(pose, "jaw", rot=(-6.0 - 4.0 * wave(t, 1, 0.3), 0, 0))
    ears(pose, 20.0 + 4.0 * wave(t, 1, 0.2), 20.0 + 4.0 * wave(t, 1, 0.45))
    tail(pose, t, amp=5.0, cycles=1, lift=8.0, spread=14.0)       # 叉尾向后劈开
    scruff(pose, t, 3.0, cycles=2, lag=0.1, push_back=10.0)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.28 伏身蓄力；0.28–0.5 整只扑出；0.5–0.56 一口咬合；0.56–0.86 叼住甩头撕扯；0.86–1 松口收回
    wind = keys(t, [(0.0, 0.0), (0.28, 1.0), (0.4, 0.4), (0.5, 0.0), (1.0, 0.0)])
    pounce = keys(t, [(0.0, 0.0), (0.28, 0.0), (0.5, 1.0), (0.7, 0.8), (0.9, 0.2), (1.0, 0.0)])
    jaw_open = keys(t, [(0.0, 0.0), (0.2, 0.5), (0.28, 0.6), (0.44, 1.0), (0.5, 1.0), (0.56, -1.0), (0.86, -0.6), (0.95, 0.0), (1.0, 0.0)])
    tear = smooth((t - 0.56) / 0.06) * (1.0 - smooth((t - 0.84) / 0.06))       # 撕扯段的包络
    shake = math.sin(math.tau * 3.0 * (t - 0.56)) * tear                         # 3 个来回，每个来回 0.1s
    turn(pose, "root", pos=(0, 2.6 * math.sin(math.pi * smooth((t - 0.28) / 0.3)) if 0.28 < t < 0.58 else 0.0, -9.0 * pounce))
    turn(pose, "hips", rot=(7.0 * wind - 8.0 * pounce, 0, 0), pos=(0, -1.6 * wind, 0))     # 蓄力时臀高肩低，扑出时拉平
    turn(pose, "spine_mid", rot=(3.0 * wind + 3.0 * pounce, 0, 0))
    turn(pose, "spine_lumbar", rot=(4.0 * wind + 4.0 * pounce, 0, 0))
    turn(pose, "thorax", rot=(-5.0 * wind - 6.0 * pounce, 0, 0))
    turn(pose, "neck_base", rot=(10.0 * wind - 14.0 * pounce, 11.0 * shake, 0))
    turn(pose, "neck_upper", rot=(8.0 * wind - 10.0 * pounce, 15.0 * shake, 0))
    turn(pose, "head", rot=(-4.0 * wind + 8.0 * pounce, 22.0 * shake, 7.0 * shake))
    turn(pose, "jaw", rot=(-30.0 * jaw_open if jaw_open > 0 else -36.0 * jaw_open, 0, 0))     # 张到最大，再一口咬合（合拢到闭口）
    ears(pose, 24.0 * wind + 14.0 * pounce, 24.0 * wind + 14.0 * pounce)
    for side, sign in SIDES:
        # 前爪：蓄力时扒地，扑出时前探，撕扯时向后拽
        turn(pose, f"arm_{side}", rot=(-14.0 * wind + 36.0 * pounce - 24.0 * tear + 8.0 * shake * sign, 0, 0))
        turn(pose, f"forearm_{side}", rot=(16.0 * wind - 20.0 * pounce, 0, 0))
        turn(pose, f"paw_f_{side}", rot=(-6.0 * wind + 14.0 * pounce, 0, 0))
        # 后腿：蓄力时蜷起，扑出时蹬直
        turn(pose, f"thigh_{side}", rot=(30.0 * wind - 34.0 * pounce, 0, 0))
        turn(pose, f"shin_{side}", rot=(-32.0 * wind + 26.0 * pounce, 0, 0))
        turn(pose, f"hock_{side}", rot=(20.0 * wind - 16.0 * pounce, 0, 0))
    tail(pose, t, amp=7.0 * (1.0 - pounce), cycles=1, lift=-4.0 * wind + 12.0 * pounce, spread=8.0 * pounce)
    scruff(pose, t, 3.0 + 3.0 * tear, cycles=3, lag=0.1, push_back=8.0 * pounce)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.15, 1.0), (0.5, 0.0)])         # 受击冲量：快进慢回
    shiver = math.sin(math.tau * 4.0 * t) * (1.0 - smooth((t - 0.15) / 0.3)) * smooth(t / 0.05)
    turn(pose, "root", pos=(0, 0, 1.6 * kick))
    turn(pose, "hips", rot=(-3.0 * kick, 0, 0))
    turn(pose, "spine_lumbar", rot=(-3.0 * kick, 0, 0))
    turn(pose, "thorax", rot=(-7.0 * kick, 3.0 * kick, 0))
    turn(pose, "neck_base", rot=(15.0 * kick, -6.0 * kick, 0))
    turn(pose, "neck_upper", rot=(12.0 * kick, -8.0 * kick, 0))
    turn(pose, "head", rot=(10.0 * kick, -6.0 * kick, 4.0 * kick))
    turn(pose, "jaw", rot=(-16.0 * kick, 0, 0))
    ears(pose, 28.0 * kick, 24.0 * kick)
    for side, _ in SIDES:
        turn(pose, f"arm_{side}", rot=(-14.0 * kick, 0, 0))
        turn(pose, f"forearm_{side}", rot=(-20.0 * kick, 0, 0))
        turn(pose, f"thigh_{side}", rot=(10.0 * kick, 0, 0))
        turn(pose, f"shin_{side}", rot=(-12.0 * kick, 0, 0))
    tail(pose, t, amp=4.0 * kick, cycles=2, lift=20.0 * kick, spread=18.0 * kick)
    scruff(pose, t, 6.0 * shiver, cycles=3, lag=0.08, push_back=-8.0 * kick)           # 乱毛炸起
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
    turn(pose, "jaw", rot=(-14.0 * stagger - 14.0 * settle, 0, 0))
    ears(pose, 24.0 * stagger + 6.0 * settle, 20.0 * stagger + 10.0 * settle)
    for side, sign in SIDES:
        turn(pose, f"arm_{side}", rot=(-18.0 * stagger + 36.0 * settle * sign, 0, 0))
        turn(pose, f"forearm_{side}", rot=(-52.0 * buckle + 14.0 * settle, 0, 0))
        turn(pose, f"paw_f_{side}", rot=(30.0 * buckle, 0, 0))
        turn(pose, f"toes_f_{side}", rot=(-20.0 * settle, 0, 0))
        turn(pose, f"thigh_{side}", rot=(14.0 * stagger - 30.0 * settle * sign, 0, 0))
        turn(pose, f"shin_{side}", rot=(-30.0 * buckle + 12.0 * settle, 0, 0))
        turn(pose, f"hock_{side}", rot=(26.0 * buckle, 0, 0))
        turn(pose, f"toes_b_{side}", rot=(-14.0 * settle, 0, 0))
    tail(pose, t, amp=5.0 * (1.0 - settle), cycles=1, lift=-10.0 * settle, spread=-4.0 * settle)
    scruff(pose, t, 3.0 * (1.0 - settle), cycles=1, lag=0.1, push_back=-10.0 * settle)
    return pose


CLIPS = {
    "idle": (3.0, True, idle),
    "walk": (1.2, True, walk),
    "run": (0.6, True, run),
    "attack": (1.2, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.0, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 6.0          # 一帧最多托起多少 px；超过说明动作本身把身体压进地里了，是动作错而不是需要托


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


def build() -> list[dict]:
    rig = PoseRig(SOURCE)
    animations = []
    for clip, (length, loop, raw) in CLIPS.items():
        if loop:
            gap = seam_gap(raw(0.0), raw(1.0))
            if gap > 0.5:
                raise ValueError(f"{NAME}.{clip}: 循环首尾旋转差 {gap:.2f}° 接不上")
        sampler = grounded(rig, raw)
        lowest, highest = check_ground(clip, rig, sampler)
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})
        print(f"  {clip:7s} {length:.2f}s {'loop' if loop else 'once'}  最低点 {lowest:+.2f}px  最高 {highest:+.2f}px  骨轨道 {len(tracks)}")
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
