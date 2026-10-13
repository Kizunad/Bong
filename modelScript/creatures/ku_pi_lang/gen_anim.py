#!/usr/bin/env python3
"""枯皮狼动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧，没有套用旧动画）：

  idle    3.0s 循环  呼吸 / 头颈微晃 / 耳抽动 / 尾梢摆 / 下颌微张
  walk    1.0s 循环  对角步态（左前+右后一组），脊椎横向蛇行，碎皮拖在腿后面晃
  run     0.6s 循环  腾跃式奔跑：四足有腾空相，脊椎上下弓伸，尾拖直
  attack  0.9s 一次  后坐蓄力 → 扑出（下颌张到最大）→ 咬合甩头 → 收回
  hurt    0.4s 一次  被击后仰，头甩起，尾竖，前腿缩
  death   1.6s 一次  踉跄 → 前腿折 → 侧倒 → 四肢松开，下颌张开不再合拢

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴。
对垂下的肢体，rot.x 为正 = 脚尖向前（-Z）摆；对头 / 颈，rot.x 为正 = 抬头；对下颌，rot.x 为负 = 张口。
这些符号约定靠渲染逐帧核过（见 check_signs）。

用法::

    python3 modelScript/creatures/ku_pi_lang/gen_anim.py        # 出 geo / animation / 贴图，并在 out/ 下做审阅资产目录
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

NAME = "ku_pi_lang"
SOURCE = HERE / "KuPiLang_tex.bbmodel"                     # 带贴图的作者 rig（gen_ku_pi_lang → gen_tex 先出）
ANIMATED = HERE / "KuPiLang_animated.bbmodel"
EXPORT_DIR = HERE / "export"                                # 安装就绪的 geo / animation / 贴图（不装进 client，等接线）
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME   # 审阅 GIF 用的资源根（gitignored）
SAMPLES = 24                                                # 每段动画的采样数，关键帧再按像素容差裁共线点

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


WALK = Gait(duty=0.62, fore_swing=24.0, hind_swing=22.0, fore_fold=42.0, hind_fold=40.0)
RUN = Gait(duty=0.38, fore_swing=42.0, hind_swing=40.0, fore_fold=62.0, hind_fold=60.0)


def stride(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (摆角, 抬腿弧)。支撑相脚从前往后划（摆角 +swing → -swing），摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    s = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(s) - 1.0), math.sin(math.pi * s)


def fore_leg(pose: Pose, side: str, u: float, gait: Gait, scale: float = 1.0) -> float:
    angle, lift = stride(u, gait.duty, gait.fore_swing * scale)
    turn(pose, f"arm_{side}", rot=(angle, 0, 0))
    turn(pose, f"forearm_{side}", rot=(-gait.fore_fold * lift * 0.7 - 0.3 * max(0.0, angle), 0, 0))
    turn(pose, f"paw_f_{side}", rot=(gait.fore_fold * lift * 0.55 + 0.25 * angle, 0, 0))
    turn(pose, f"toes_f_{side}", rot=(-14.0 * lift, 0, 0))
    # 碎皮拖在后面：腿往前摆它往后甩，反之亦然
    turn(pose, f"elbow_rag_{side}", rot=(-0.55 * angle - 10.0 * lift, 0, 0))
    return angle


def hind_leg(pose: Pose, side: str, u: float, gait: Gait, scale: float = 1.0) -> float:
    angle, lift = stride(u, gait.duty, gait.hind_swing * scale)
    turn(pose, f"thigh_{side}", rot=(angle, 0, 0))
    turn(pose, f"shin_{side}", rot=(-gait.hind_fold * lift * 0.75 - 0.35 * max(0.0, angle), 0, 0))
    turn(pose, f"hock_{side}", rot=(gait.hind_fold * lift * 0.7 + 0.2 * angle, 0, 0))
    turn(pose, f"paw_b_{side}", rot=(gait.hind_fold * 0.25 * lift, 0, 0))
    turn(pose, f"toes_b_{side}", rot=(-12.0 * lift, 0, 0))
    turn(pose, f"shin_rag_{side}", rot=(-0.5 * angle - 8.0 * lift, 0, 0))
    turn(pose, f"thigh_rag_{side}", rot=(-0.6 * angle, 0, 0))
    return angle


def tail_wave(pose: Pose, t: float, amp: float, cycles: float, droop: float = 0.0, lift: float = 0.0) -> None:
    """尾巴 7 节行波：越靠尾梢相位越晚、幅度越大。droop > 0 尾梢下垂，lift > 0 尾根上翘。"""
    for i in range(1, 8):
        k = i / 7.0
        turn(pose, f"tail_{i}", rot=(lift * (1.0 - k) - droop * k, amp * (0.35 + 0.65 * k) * wave(t, cycles, -0.09 * i), 0))


def ears(pose: Pose, left: float, right: float) -> None:
    turn(pose, "ear_l", rot=(left, 0, 4.0 * left))
    turn(pose, "ear_r", rot=(right, 0, -4.0 * right))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "thorax", rot=(0.8 * breath, 0, 0), pos=(0, 0.12 * breath, 0))
    turn(pose, "spine_lumbar", rot=(-0.4 * breath, 0, 0))
    turn(pose, "neck_base", rot=(2.0 * wave(t, 1, 0.1), 3.0 * wave(t, 1, 0.3), 0))
    turn(pose, "neck_upper", rot=(1.5 * wave(t, 1, 0.2), 4.0 * wave(t, 1, 0.35), 0))
    turn(pose, "head", rot=(1.0 * wave(t, 2, 0.05), 3.0 * wave(t, 1, 0.45), 1.5 * wave(t, 1, 0.1)))
    # 下颌随呼吸微开，中段无征兆地抖一下牙
    chatter = math.sin(math.pi * smooth((t - 0.6) / 0.08)) if 0.6 < t < 0.68 else 0.0
    turn(pose, "jaw", rot=(-3.0 - 2.0 * (0.5 + 0.5 * breath) - 6.0 * chatter, 0, 0))
    turn(pose, "tongue", rot=(2.0 * wave(t, 1, 0.3), 0, 0))
    flick = math.sin(math.pi * smooth((t - 0.2) / 0.1)) if 0.2 < t < 0.3 else 0.0
    ears(pose, -3.0 * wave(t, 1, 0.2) - 14.0 * flick, 2.0 * wave(t, 1, 0.6))
    tail_wave(pose, t, amp=7.0, cycles=1, droop=4.0)
    turn(pose, "pelt_flank_l", rot=(1.2 * breath, 0, 0))
    turn(pose, "pelt_flank_r", rot=(1.2 * breath, 0, 0))
    for side, _ in SIDES:
        turn(pose, f"elbow_rag_{side}", rot=(1.5 * wave(t, 1, 0.15), 0, 0))
        turn(pose, f"thigh_rag_{side}", rot=(1.5 * wave(t, 1, 0.3), 0, 0))
        turn(pose, f"shin_rag_{side}", rot=(1.5 * wave(t, 1, 0.45), 0, 0))
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    phases = {"l": 0.0, "r": 0.5}                   # 前左 + 后右一组
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + phases[side]), WALK)
        hind_leg(pose, side, frac(t + phases[side] + 0.5), WALK)
    # 脊椎蛇行：肩和臀反向转，头顶住
    turn(pose, "hips", rot=(0, 2.5 * wave(t, 1), 0), pos=(0, 0.25 * abs(wave(t, 2, 0.25)), 0))
    turn(pose, "spine_lumbar", rot=(0, -3.0 * wave(t, 1), 0))
    turn(pose, "thorax", rot=(0.8 * wave(t, 2, 0.1), -2.5 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(1.5 * wave(t, 2, 0.35), 3.0 * wave(t, 1), 0))
    turn(pose, "neck_upper", rot=(1.5 * wave(t, 2, 0.4), 2.5 * wave(t, 1), 0))
    turn(pose, "head", rot=(-1.5 * wave(t, 2, 0.45), 1.0 * wave(t, 1), 0))
    turn(pose, "jaw", rot=(-5.0 - 2.0 * wave(t, 2, 0.2), 0, 0))
    ears(pose, 4.0 * wave(t, 2, 0.1), 4.0 * wave(t, 2, 0.6))
    tail_wave(pose, t, amp=9.0, cycles=1, droop=5.0)
    for side, _ in SIDES:
        turn(pose, f"pelt_flank_{side}", rot=(2.0 * wave(t, 2, 0.4), 0, 0))
    return pose


def run(t: float) -> Pose:
    pose = Pose()
    # 腾跃步态：前腿一对略错开，后腿一对略错开，前后相隔半周期；占空比 0.38 → 有腾空相
    offsets = {"fore": {"l": 0.0, "r": 0.1}, "hind": {"l": 0.5, "r": 0.6}}
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + offsets["fore"][side]), RUN)
        hind_leg(pose, side, frac(t + offsets["hind"][side]), RUN)
    # 脊椎弓伸：后腿蹬出时拱背，前腿着地时伸展，整只身体上下起伏
    arch = wave(t, 1, 0.1)
    turn(pose, "hips", rot=(-4.0 * arch, 1.2 * wave(t, 1), 0), pos=(0, 0.9 * (0.5 + 0.5 * wave(t, 1, 0.3)), 0))
    turn(pose, "spine_lumbar", rot=(6.0 * arch, -1.5 * wave(t, 1), 0))
    turn(pose, "thorax", rot=(-5.0 * arch, 1.0 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(-8.0 + 3.0 * arch, 0, 0))        # 脖子伸平
    turn(pose, "neck_upper", rot=(-6.0 - 2.0 * arch, 0, 0))
    turn(pose, "head", rot=(-2.0 * arch, 0, 0))
    turn(pose, "jaw", rot=(-14.0 - 4.0 * wave(t, 1, 0.3), 0, 0))  # 张口喘
    turn(pose, "tongue", rot=(-8.0 + 5.0 * wave(t, 2, 0.1), 0, 0))
    ears(pose, 16.0 + 4.0 * wave(t, 1, 0.2), 16.0 + 4.0 * wave(t, 1, 0.45))   # 耳朵向后压
    tail_wave(pose, t, amp=5.0, cycles=1, droop=-4.0, lift=-6.0)              # 尾巴拖直后飘
    for side, _ in SIDES:
        turn(pose, f"pelt_flank_{side}", rot=(5.0 * wave(t, 1, 0.5), 0, 0))
        turn(pose, f"pelt_shoulder_{side}", rot=(4.0 * wave(t, 1, 0.55), 0, 0))
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.28 后坐蓄力；0.28–0.5 扑出；0.5–0.62 咬合甩头；0.62–1 收回
    crouch = keys(t, [(0.0, 0.0), (0.28, 1.0), (0.5, 0.0), (1.0, 0.0)])
    lunge = keys(t, [(0.0, 0.0), (0.28, 0.0), (0.5, 1.0), (0.62, 1.0), (1.0, 0.0)])
    bite = keys(t, [(0.0, 0.0), (0.28, 0.0), (0.5, 1.0), (0.58, 0.0), (1.0, 0.0)])   # 1 = 口张到最大
    shake = math.sin(math.tau * 3.0 * (t - 0.5)) * smooth((t - 0.5) / 0.04) * (1.0 - smooth((t - 0.62) / 0.1)) if 0.5 < t < 0.75 else 0.0
    turn(pose, "root", pos=(0, 0, -6.5 * lunge + 1.0 * crouch))
    turn(pose, "hips", rot=(5.0 * crouch - 8.0 * lunge, 0, 0), pos=(0, -1.6 * crouch + 0.8 * lunge, 0))
    turn(pose, "spine_lumbar", rot=(4.0 * crouch + 6.0 * lunge, 0, 0))
    turn(pose, "thorax", rot=(-4.0 * crouch - 3.0 * lunge, 0, 0))
    turn(pose, "neck_base", rot=(10.0 * crouch - 16.0 * lunge, 0, 0))
    turn(pose, "neck_upper", rot=(8.0 * crouch - 14.0 * lunge, 0, 0))
    turn(pose, "head", rot=(4.0 * crouch - 6.0 * lunge, 8.0 * shake, 6.0 * shake))
    turn(pose, "jaw", rot=(-18.0 * crouch - 40.0 * bite * lunge - 6.0 * lunge, 0, 0))
    turn(pose, "tongue", rot=(-10.0 * bite, 0, 0))
    ears(pose, 24.0 * crouch + 18.0 * lunge, 24.0 * crouch + 18.0 * lunge)
    for side, _ in SIDES:
        # 前腿：蓄力时撑住前倾，扑出时伸向前方，爪张开
        turn(pose, f"arm_{side}", rot=(-14.0 * crouch + 52.0 * lunge, 0, 0))
        turn(pose, f"forearm_{side}", rot=(26.0 * crouch - 28.0 * lunge, 0, 0))
        turn(pose, f"paw_f_{side}", rot=(-8.0 * crouch + 20.0 * lunge, 0, 0))
        turn(pose, f"toes_f_{side}", rot=(0, 0, 0))
        # 后腿：蓄力蜷起（大腿向前、小腿折），扑出时蹬直向后
        turn(pose, f"thigh_{side}", rot=(34.0 * crouch - 38.0 * lunge, 0, 0))
        turn(pose, f"shin_{side}", rot=(-36.0 * crouch + 26.0 * lunge, 0, 0))
        turn(pose, f"hock_{side}", rot=(22.0 * crouch - 18.0 * lunge, 0, 0))
        turn(pose, f"elbow_rag_{side}", rot=(-30.0 * lunge, 0, 0))
        turn(pose, f"thigh_rag_{side}", rot=(28.0 * lunge - 10.0 * crouch, 0, 0))
        turn(pose, f"shin_rag_{side}", rot=(24.0 * lunge, 0, 0))
    tail_wave(pose, t, amp=10.0 * (1.0 - lunge), cycles=1, droop=0.0, lift=14.0 * crouch - 18.0 * lunge)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.15, 1.0), (0.4, 0.0)])         # 受击冲量：快进慢回
    turn(pose, "root", pos=(0, 0, 1.4 * kick))
    turn(pose, "hips", rot=(-3.0 * kick, 0, 0))
    turn(pose, "thorax", rot=(-7.0 * kick, 4.0 * kick, 0))
    turn(pose, "neck_base", rot=(14.0 * kick, -6.0 * kick, 0))
    turn(pose, "neck_upper", rot=(12.0 * kick, -8.0 * kick, 0))
    turn(pose, "head", rot=(10.0 * kick, -6.0 * kick, 4.0 * kick))
    turn(pose, "jaw", rot=(-30.0 * kick, 0, 0))
    turn(pose, "tongue", rot=(-12.0 * kick, 0, 0))
    ears(pose, 26.0 * kick, 22.0 * kick)
    for side, _ in SIDES:
        turn(pose, f"arm_{side}", rot=(-16.0 * kick, 0, 0))
        turn(pose, f"forearm_{side}", rot=(20.0 * kick, 0, 0))
        turn(pose, f"thigh_{side}", rot=(10.0 * kick, 0, 0))
        turn(pose, f"shin_{side}", rot=(-12.0 * kick, 0, 0))
        turn(pose, f"elbow_rag_{side}", rot=(20.0 * kick * (1 if side == "l" else -1), 0, 0))
        turn(pose, f"thigh_rag_{side}", rot=(-22.0 * kick, 0, 0))
    tail_wave(pose, t, amp=4.0 * kick, cycles=2, droop=-10.0 * kick, lift=24.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    stagger = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.3, 0.6), (1.0, 0.6)])
    fall = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.62, 1.0), (1.0, 1.0)])          # 1 = 侧倒到位
    settle = keys(t, [(0.0, 0.0), (0.62, 0.0), (0.82, 1.0), (1.0, 1.0)])      # 倒下后四肢松开
    turn(pose, "root", rot=(0, 0, 84.0 * fall), pos=(0, 0, 1.0 * stagger))
    turn(pose, "hips", rot=(-3.0 * stagger, 0, 0), pos=(0, -3.0 * keys(t, [(0, 0), (0.3, 1.0), (1, 1.0)]), 0))
    turn(pose, "thorax", rot=(-6.0 * stagger, 3.0 * stagger, 0))
    turn(pose, "neck_base", rot=(12.0 * stagger - 14.0 * fall, -5.0 * stagger + 10.0 * settle, 0))
    turn(pose, "neck_upper", rot=(10.0 * stagger - 10.0 * fall, 6.0 * settle, 0))
    turn(pose, "head", rot=(6.0 * stagger - 8.0 * fall, 8.0 * settle, 6.0 * settle))
    turn(pose, "jaw", rot=(-26.0 * stagger - 12.0 * settle, 0, 0))
    turn(pose, "tongue", rot=(-12.0 * stagger - 8.0 * settle, 0, 0))
    ears(pose, 24.0 * stagger + 6.0 * settle, 20.0 * stagger + 10.0 * settle)
    for side, sign in SIDES:
        buckle = keys(t, [(0.0, 0.0), (0.3, 0.2), (0.55, 1.0), (1.0, 1.0)])
        turn(pose, f"arm_{side}", rot=(-18.0 * stagger + 36.0 * settle * sign, 0, 0))
        turn(pose, f"forearm_{side}", rot=(-52.0 * buckle + 14.0 * settle, 0, 0))
        turn(pose, f"paw_f_{side}", rot=(30.0 * buckle, 0, 0))
        turn(pose, f"toes_f_{side}", rot=(-20.0 * settle, 0, 0))
        turn(pose, f"thigh_{side}", rot=(14.0 * stagger - 30.0 * settle * sign, 0, 0))
        turn(pose, f"shin_{side}", rot=(-30.0 * buckle + 12.0 * settle, 0, 0))
        turn(pose, f"hock_{side}", rot=(26.0 * buckle, 0, 0))
        turn(pose, f"toes_b_{side}", rot=(-14.0 * settle, 0, 0))
        turn(pose, f"elbow_rag_{side}", rot=(18.0 * settle, 0, 0))
        turn(pose, f"thigh_rag_{side}", rot=(-16.0 * settle, 0, 0))
    tail_wave(pose, t, amp=6.0 * (1.0 - settle), cycles=1, droop=20.0 * settle, lift=18.0 * stagger * (1.0 - fall))
    return pose


CLIPS = {
    "idle": (3.0, True, idle),
    "walk": (1.0, True, walk),
    "run": (0.6, True, run),
    "attack": (0.9, False, attack),
    "hurt": (0.4, False, hurt),
    "death": (1.6, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只狼（root）托起到刚好贴地。
# 只在入地的帧上移，其余帧原样不动；容差以内的轻微入地（爪尖 / 毛刺）不动。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 4.0          # 一帧最多托起多少 px；超过说明动作本身把身体压进地里了，是动作错而不是需要托


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
        print(f"  {clip:7s} {length:.1f}s {'loop' if loop else 'once'}  最低点 {lowest:+.2f}px  骨轨道 {len(tracks)}")
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
