#!/usr/bin/env python3
"""断角野牛动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

七段动画（全部从零写关键帧；骨链和步态相位参考 dong_sui_lu 的写法，但每一段都是按瘦骨野牛重写的）：

  idle    3.4s 循环  重喘着的呼吸：胸腔起伏、低垂的头缓缓左右摆、耳抽动、下颌嚼动、肩峰随呼吸微颤、尾一甩
  walk    1.6s 循环  慢重的对角步态（占空比 0.7）：头低垂随步点头，肩峰与鬃毛左右晃，尾缓摆
  run     0.8s 循环  笨重的腾跃奔跑：四足短暂腾空，脊椎弓伸，头压低前伸，鬃毛被气流压向身后
  attack  1.1s 一次  顶挑撕扯：低头蓄力（前蹄刨地）→ 右角下压前顶 → 猛一甩头上挑（整个前身抬起）→ 收回
  charge  1.8s 一次  低头冲撞：后蹄蹬稳、前蹄刨地、头压到最低 → 整只低头蓄势前冲（四足奔跑步态、鬃毛压向身后）
                      → 撞击瞬间头猛抬、整只被反作用力顿住 → 缓回
  hurt    0.5s 一次  被击后仰，头甩起，前腿缩，肩峰一震，尾炸起
  death   2.2s 一次  踉跄 → 前腿折跪 → 侧倒 → 四肢松开，头颈松垂，尾散垂

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
骨链：root → hips → spine_lower → spine_upper → neck_1 → neck_2 → head；后腿挂在 hips，前腿、肩峰挂在 spine_upper。
符号约定（和冻髓鹿一致，对着数值核过）：垂下的肢体 rot.x 为正 = 脚尖向前（-Z）摆；头 / 颈 rot.x 为正 = 抬头；
下颌 rot.x 为负 = 张口。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/duan_jiao_niu/gen_anim.py
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

NAME = "duan_jiao_niu"
SOURCE = HERE / "DuanJiaoNiu_tex.bbmodel"
ANIMATED = HERE / "DuanJiaoNiu_animated.bbmodel"
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


WALK = Gait(duty=0.7, fore_swing=17.0, hind_swing=16.0, fore_fold=32.0, hind_fold=26.0)
RUN = Gait(duty=0.4, fore_swing=32.0, hind_swing=30.0, fore_fold=54.0, hind_fold=44.0)
CHARGE = Gait(duty=0.38, fore_swing=34.0, hind_swing=32.0, fore_fold=58.0, hind_fold=48.0)


def stride(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (摆角, 抬腿弧)。支撑相脚从前往后划（摆角 +swing → -swing），摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    s = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(s) - 1.0), math.sin(math.pi * s)


def fore_leg(pose: Pose, side: str, u: float, gait: Gait) -> None:
    angle, lift = stride(u, gait.duty, gait.fore_swing)
    turn(pose, f"f_upper_{side}", rot=(angle, 0, 0))
    turn(pose, f"f_fore_{side}", rot=(-gait.fore_fold * lift * 0.8 - 0.25 * max(0.0, angle), 0, 0))
    turn(pose, f"f_cannon_{side}", rot=(gait.fore_fold * lift * 0.55 + 0.2 * angle, 0, 0))
    turn(pose, f"f_hoof_{side}", rot=(-16.0 * lift, 0, 0))


def hind_leg(pose: Pose, side: str, u: float, gait: Gait) -> None:
    angle, lift = stride(u, gait.duty, gait.hind_swing)
    turn(pose, f"h_thigh_{side}", rot=(angle, 0, 0))
    turn(pose, f"h_shank_{side}", rot=(-gait.hind_fold * lift * 0.8 - 0.3 * max(0.0, angle), 0, 0))
    turn(pose, f"h_cannon_{side}", rot=(gait.hind_fold * lift * 0.75 + 0.2 * angle, 0, 0))
    turn(pose, f"h_hoof_{side}", rot=(-14.0 * lift, 0, 0))


# ---------------------------------------------------------------- 尾 / 耳 / 毛 / 角
def tail(pose: Pose, t: float, amp: float, cycles: float, lift: float = 0.0) -> None:
    """四节尾行波：越靠尾梢相位越晚、幅度越大。lift > 0 尾根上翘。"""
    for i in range(1, 5):
        k = i / 4.0
        turn(pose, f"tail_{i}", rot=(lift * (1.0 - 0.5 * k), amp * (0.4 + 0.6 * k) * wave(t, cycles, -0.1 * i), 0))


def ears(pose: Pose, left: float, right: float) -> None:
    turn(pose, "ear_l", rot=(left, 0, 4.0 * left))
    turn(pose, "ear_r", rot=(right, 0, -4.0 * right))


def mane(pose: Pose, t: float, sway: float, cycles: float = 1.0, lag: float = 0.2, push_back: float = 0.0) -> None:
    """肩峰与颈背鬃毛：滞后于身体地左右晃；push_back > 0 被气流压向身后。"""
    turn(pose, "hump", rot=(-0.5 * push_back, 0, sway * 0.6 * wave(t, cycles, -lag)))
    turn(pose, "mane", rot=(-push_back, 0, sway * wave(t, cycles, -2.0 * lag)))


def horns(pose: Pose, shake: float) -> None:
    """角：整副跟着头动，细微的反向抖动（角根与头骨之间有一点松动）。"""
    turn(pose, "horn_l", rot=(-0.4 * shake, 0, 0.3 * shake))
    turn(pose, "horn_r", rot=(-0.4 * shake, 0, -0.3 * shake))


# ---------------------------------------------------------------- 七段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "spine_upper", rot=(1.0 * breath, 0, 0), pos=(0, 0.12 * breath, 0))
    turn(pose, "spine_lower", rot=(-0.5 * breath, 0, 0))
    turn(pose, "hips", rot=(0.3 * breath, 0.8 * wave(t, 1, 0.3), 0))
    turn(pose, "neck_1", rot=(1.5 * wave(t, 1, 0.1) - 1.0, 5.0 * wave(t, 1, 0.3), 0))        # 头低垂，缓缓左右摆
    turn(pose, "neck_2", rot=(1.5 * wave(t, 1, 0.2), 6.0 * wave(t, 1, 0.35), 0))
    turn(pose, "head", rot=(1.5 * wave(t, 2, 0.05), 3.5 * wave(t, 1, 0.45), 1.5 * wave(t, 1, 0.1)))
    turn(pose, "jaw", rot=(-3.0 - 4.0 * max(0.0, wave(t, 3, 0.55)), 0, 0))                    # 反刍式嚼动
    flick = math.sin(math.pi * smooth((t - 0.4) / 0.08)) if 0.4 < t < 0.48 else 0.0
    ears(pose, -3.0 * wave(t, 1, 0.2) - 22.0 * flick, 2.5 * wave(t, 1, 0.6))
    horns(pose, 1.0 * wave(t, 1, 0.45))
    mane(pose, t, 2.0, cycles=1)
    tail(pose, t, amp=7.0, cycles=1, lift=-3.0 * smooth((t - 0.6) / 0.1) * (1.0 - smooth((t - 0.75) / 0.1)))
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    phases = {"l": 0.0, "r": 0.5}                   # 左前 + 右后一组
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + phases[side]), WALK)
        hind_leg(pose, side, frac(t + phases[side] + 0.5), WALK)
    turn(pose, "hips", rot=(0, 2.2 * wave(t, 1), 0), pos=(0, 0.25 * abs(wave(t, 2, 0.25)), 0))
    turn(pose, "spine_lower", rot=(0, -2.2 * wave(t, 1), 0))
    turn(pose, "spine_upper", rot=(0.8 * wave(t, 2, 0.1), -2.0 * wave(t, 1), 0))
    turn(pose, "neck_1", rot=(-2.0 + 3.0 * wave(t, 2, 0.35), 3.0 * wave(t, 1), 0))             # 头随步点头
    turn(pose, "neck_2", rot=(-1.5 + 2.5 * wave(t, 2, 0.4), 2.5 * wave(t, 1), 0))
    turn(pose, "head", rot=(-3.0 * wave(t, 2, 0.45), 1.0 * wave(t, 1), 0))
    turn(pose, "jaw", rot=(-3.0, 0, 0))
    ears(pose, 4.0 * wave(t, 2, 0.1), 4.0 * wave(t, 2, 0.6))
    horns(pose, 2.0 * wave(t, 2, 0.5))
    mane(pose, t, 3.0, cycles=1, lag=0.12)
    tail(pose, t, amp=9.0, cycles=1)
    return pose


def run(t: float) -> Pose:
    pose = Pose()
    offsets = {"fore": {"l": 0.0, "r": 0.12}, "hind": {"l": 0.5, "r": 0.62}}
    for side, _ in SIDES:
        fore_leg(pose, side, frac(t + offsets["fore"][side]), RUN)
        hind_leg(pose, side, frac(t + offsets["hind"][side]), RUN)
    arch = wave(t, 1, 0.1)
    turn(pose, "hips", rot=(-3.5 * arch, 0.8 * wave(t, 1), 0), pos=(0, 0.7 * (0.5 + 0.5 * wave(t, 1, 0.3)), 0))
    turn(pose, "spine_lower", rot=(4.0 * arch, -1.0 * wave(t, 1), 0))
    turn(pose, "spine_upper", rot=(-3.5 * arch, 0.8 * wave(t, 1), 0))
    turn(pose, "neck_1", rot=(-8.0 + 2.5 * arch, 0, 0))               # 头压低前伸
    turn(pose, "neck_2", rot=(-6.0 - 1.5 * arch, 0, 0))
    turn(pose, "head", rot=(3.0 - 1.5 * arch, 0, 0))
    turn(pose, "jaw", rot=(-8.0 - 3.0 * wave(t, 1, 0.3), 0, 0))
    ears(pose, 16.0 + 3.0 * wave(t, 1, 0.2), 16.0 + 3.0 * wave(t, 1, 0.45))
    horns(pose, 3.0 * wave(t, 1, 0.55))
    mane(pose, t, 3.0, cycles=2, lag=0.1, push_back=9.0)
    tail(pose, t, amp=5.0, cycles=1, lift=10.0)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.32 低头蓄力、前蹄刨地；0.32–0.5 右角下压前顶；0.5–0.68 猛甩头上挑（整个前身抬起）；0.68–1 收回
    wind = keys(t, [(0.0, 0.0), (0.32, 1.0), (0.45, 0.4), (0.5, 0.0), (1.0, 0.0)])
    thrust = keys(t, [(0.0, 0.0), (0.32, 0.0), (0.5, 1.0), (0.6, 1.0), (0.68, 0.0), (1.0, 0.0)])
    toss = keys(t, [(0.0, 0.0), (0.5, 0.0), (0.6, 1.0), (0.76, 0.4), (0.9, 0.0), (1.0, 0.0)])
    paw = math.sin(math.tau * 2.0 * t) * smooth(t / 0.08) * (1.0 - smooth((t - 0.3) / 0.05)) if t < 0.35 else 0.0
    turn(pose, "root", pos=(0, 0, -4.0 * thrust + 0.8 * wind))
    turn(pose, "hips", rot=(4.0 * wind - 4.0 * thrust, 0, 0), pos=(0, -0.7 * wind, 0))
    turn(pose, "spine_lower", rot=(3.0 * wind + 3.0 * thrust, 0, 0))
    turn(pose, "spine_upper", rot=(-3.0 * wind - 5.0 * thrust + 6.0 * toss, 0, 0))
    # 颈和头：蓄力低头，顶出时压到最低，上挑时猛抬并偏向右侧（右角完整）
    turn(pose, "neck_1", rot=(-12.0 * wind - 14.0 * thrust + 26.0 * toss, 4.0 * toss, 0))
    turn(pose, "neck_2", rot=(-10.0 * wind - 12.0 * thrust + 24.0 * toss, 6.0 * toss, 0))
    turn(pose, "head", rot=(-8.0 * wind - 10.0 * thrust + 22.0 * toss, 14.0 * toss, 10.0 * toss))
    turn(pose, "jaw", rot=(-4.0 * wind - 10.0 * toss, 0, 0))
    ears(pose, 20.0 * wind + 18.0 * thrust, 20.0 * wind + 18.0 * thrust)
    horns(pose, 5.0 * toss)
    mane(pose, t, 2.0, cycles=3, push_back=-6.0 * toss)
    for side, sign in SIDES:
        # 前腿：蓄力时左蹄刨地，顶出时前伸蹬地，上挑时前腿撑直
        dig = paw if side == "l" else -0.3 * paw
        turn(pose, f"f_upper_{side}", rot=(-8.0 * wind + 16.0 * thrust + 20.0 * dig - 6.0 * toss, 0, 0))
        turn(pose, f"f_fore_{side}", rot=(16.0 * wind - 16.0 * thrust - 26.0 * max(0.0, dig), 0, 0))
        turn(pose, f"f_cannon_{side}", rot=(-6.0 * wind + 8.0 * thrust + 14.0 * max(0.0, dig), 0, 0))
        # 后腿：蓄力时蜷起，顶出时蹬直
        turn(pose, f"h_thigh_{side}", rot=(20.0 * wind - 22.0 * thrust, 0, 0))
        turn(pose, f"h_shank_{side}", rot=(-22.0 * wind + 16.0 * thrust, 0, 0))
        turn(pose, f"h_cannon_{side}", rot=(14.0 * wind - 10.0 * thrust, 0, 0))
    tail(pose, t, amp=6.0 * (1.0 - thrust), cycles=1, lift=10.0 * wind - 10.0 * thrust)
    return pose


def charge(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.28 蹬稳刨地、头压到最低；0.28–0.7 低头冲出（四足奔跑步态，整只前移）；
    #       0.7–0.78 撞击——头猛抬、身子被顿住；0.78–1 缓回
    dig = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.28, 0.6), (0.32, 0.0), (1.0, 0.0)])         # 蓄力刨地
    run_ = keys(t, [(0.0, 0.0), (0.28, 0.0), (0.38, 1.0), (0.7, 1.0), (0.78, 0.2), (0.88, 0.0), (1.0, 0.0)])    # 奔跑强度
    lower = keys(t, [(0.0, 0.0), (0.2, 1.0), (0.7, 1.0), (0.76, 0.0), (1.0, 0.0)])         # 头压低程度
    impact = keys(t, [(0.0, 0.0), (0.7, 0.0), (0.74, 1.0), (0.8, 0.8), (0.92, 0.0), (1.0, 0.0)])   # 撞击瞬间
    advance = keys(t, [(0.0, 0.0), (0.28, 0.0), (0.7, 1.0), (0.78, 0.9), (1.0, 0.0)])
    paw = math.sin(math.tau * 2.5 * t) * dig
    cycle = 2.6 * max(0.0, t - 0.28)
    for side, sign in SIDES:
        u_f = frac(cycle + (0.0 if side == "l" else 0.12))
        u_h = frac(cycle + (0.5 if side == "l" else 0.62))
        # 奔跑步态按强度混入：run_ = 0 时腿保持蓄力姿态
        angle_f, lift_f = stride(u_f, CHARGE.duty, CHARGE.fore_swing)
        angle_h, lift_h = stride(u_h, CHARGE.duty, CHARGE.hind_swing)
        turn(pose, f"f_upper_{side}", rot=(angle_f * run_ + (14.0 * paw if side == "l" else -4.0 * paw) - 8.0 * dig, 0, 0))
        turn(pose, f"f_fore_{side}", rot=(-CHARGE.fore_fold * lift_f * 0.8 * run_ + 16.0 * dig - 20.0 * max(0.0, paw if side == "l" else 0.0), 0, 0))
        turn(pose, f"f_cannon_{side}", rot=(CHARGE.fore_fold * lift_f * 0.55 * run_ + 0.2 * angle_f * run_, 0, 0))
        turn(pose, f"f_hoof_{side}", rot=(-16.0 * lift_f * run_, 0, 0))
        turn(pose, f"h_thigh_{side}", rot=(angle_h * run_ + 22.0 * dig, 0, 0))
        turn(pose, f"h_shank_{side}", rot=(-CHARGE.hind_fold * lift_h * 0.8 * run_ - 24.0 * dig, 0, 0))
        turn(pose, f"h_cannon_{side}", rot=(CHARGE.hind_fold * lift_h * 0.75 * run_ + 16.0 * dig, 0, 0))
        turn(pose, f"h_hoof_{side}", rot=(-14.0 * lift_h * run_, 0, 0))
    bound = wave(t, 2.6, 0.1) * run_
    turn(pose, "root", pos=(0, 0, -22.0 * advance + 3.0 * impact))
    turn(pose, "hips", rot=(5.0 * dig - 3.5 * bound - 4.0 * impact, 0.8 * wave(t, 2.6) * run_, 0),
         pos=(0, -0.9 * dig + 0.6 * (0.5 + 0.5 * wave(t, 2.6, 0.3)) * run_, 0))
    turn(pose, "spine_lower", rot=(4.0 * bound + 3.0 * dig, 0, 0))
    turn(pose, "spine_upper", rot=(-3.5 * bound - 4.0 * dig + 8.0 * impact, 0, 0))
    # 头颈：蓄力和冲锋时压到最低、角尖朝前；撞上的一瞬间猛抬
    turn(pose, "neck_1", rot=(-14.0 * lower + 30.0 * impact, 0, 0))
    turn(pose, "neck_2", rot=(-12.0 * lower + 26.0 * impact, 0, 0))
    turn(pose, "head", rot=(-10.0 * lower + 24.0 * impact, 6.0 * impact * wave(t, 3, 0), 0))
    turn(pose, "jaw", rot=(-3.0 * dig - 6.0 * impact, 0, 0))
    ears(pose, 22.0 * lower, 22.0 * lower)
    horns(pose, 5.0 * impact)
    mane(pose, t, 3.0, cycles=2, lag=0.1, push_back=12.0 * run_ - 8.0 * impact)
    tail(pose, t, amp=5.0, cycles=1, lift=10.0 * run_ + 6.0 * dig)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.15, 1.0), (0.5, 0.0)])         # 受击冲量：快进慢回
    shiver = math.sin(math.tau * 4.0 * t) * (1.0 - smooth((t - 0.15) / 0.3)) * smooth(t / 0.05)
    turn(pose, "root", pos=(0, 0, 1.6 * kick))
    turn(pose, "hips", rot=(-3.0 * kick, 0, 0))
    turn(pose, "spine_upper", rot=(-6.0 * kick, 3.0 * kick, 0))
    turn(pose, "neck_1", rot=(14.0 * kick, -6.0 * kick, 0))
    turn(pose, "neck_2", rot=(12.0 * kick, -8.0 * kick, 0))
    turn(pose, "head", rot=(10.0 * kick, -6.0 * kick, 4.0 * kick))
    turn(pose, "jaw", rot=(-18.0 * kick, 0, 0))
    ears(pose, 26.0 * kick, 22.0 * kick)
    horns(pose, 7.0 * shiver)
    mane(pose, t, 5.0 * shiver, cycles=3, lag=0.08, push_back=-6.0 * kick)
    for side, _ in SIDES:
        turn(pose, f"f_upper_{side}", rot=(-12.0 * kick, 0, 0))
        turn(pose, f"f_fore_{side}", rot=(-18.0 * kick, 0, 0))
        turn(pose, f"h_thigh_{side}", rot=(8.0 * kick, 0, 0))
        turn(pose, f"h_shank_{side}", rot=(-10.0 * kick, 0, 0))
    tail(pose, t, amp=3.0 * kick, cycles=2, lift=22.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    stagger = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.3, 0.6), (1.0, 0.6)])
    fall = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.66, 1.0), (1.0, 1.0)])          # 1 = 侧倒到位
    settle = keys(t, [(0.0, 0.0), (0.66, 0.0), (0.86, 1.0), (1.0, 1.0)])       # 倒下后四肢松开
    buckle = keys(t, [(0.0, 0.0), (0.3, 0.2), (0.56, 1.0), (1.0, 1.0)])
    turn(pose, "root", rot=(0, 0, 84.0 * fall), pos=(0, 0, 1.0 * stagger))
    turn(pose, "hips", rot=(-3.0 * stagger, 0, 0), pos=(0, -3.0 * keys(t, [(0, 0), (0.3, 1.0), (1, 1.0)]), 0))
    turn(pose, "spine_upper", rot=(-6.0 * stagger, 3.0 * stagger, 0))
    turn(pose, "neck_1", rot=(10.0 * stagger - 12.0 * fall, -5.0 * stagger + 10.0 * settle, 0))
    turn(pose, "neck_2", rot=(8.0 * stagger - 10.0 * fall, 6.0 * settle, 0))
    turn(pose, "head", rot=(6.0 * stagger - 8.0 * fall, 8.0 * settle, 6.0 * settle))
    turn(pose, "jaw", rot=(-16.0 * stagger - 10.0 * settle, 0, 0))
    ears(pose, 24.0 * stagger + 6.0 * settle, 20.0 * stagger + 10.0 * settle)
    horns(pose, -4.0 * settle)
    mane(pose, t, 2.0 * (1.0 - settle), cycles=1, lag=0.1, push_back=-8.0 * settle)
    for side, sign in SIDES:
        turn(pose, f"f_upper_{side}", rot=(-18.0 * stagger + 34.0 * settle * sign, 0, 0))
        turn(pose, f"f_fore_{side}", rot=(-52.0 * buckle + 14.0 * settle, 0, 0))
        turn(pose, f"f_cannon_{side}", rot=(30.0 * buckle, 0, 0))
        turn(pose, f"f_hoof_{side}", rot=(-20.0 * settle, 0, 0))
        turn(pose, f"h_thigh_{side}", rot=(14.0 * stagger - 28.0 * settle * sign, 0, 0))
        turn(pose, f"h_shank_{side}", rot=(-30.0 * buckle + 12.0 * settle, 0, 0))
        turn(pose, f"h_cannon_{side}", rot=(26.0 * buckle, 0, 0))
        turn(pose, f"h_hoof_{side}", rot=(-14.0 * settle, 0, 0))
    tail(pose, t, amp=5.0 * (1.0 - settle), cycles=1, lift=-10.0 * settle)
    return pose


CLIPS = {
    "idle": (3.4, True, idle),
    "walk": (1.6, True, walk),
    "run": (0.8, True, run),
    "attack": (1.1, False, attack),
    "charge": (1.8, False, charge),
    "hurt": (0.5, False, hurt),
    "death": (2.2, False, death),
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
