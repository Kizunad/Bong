#!/usr/bin/env python3
"""碎岩猿动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧）：

  idle    3.4s 循环  驼背站定的沉重呼吸：胸腔与肩起伏、两肩一高一低、头慢慢左右扫、下颌咀嚼、十指随呼吸轻握、鬃毛飘
  walk    1.5s 循环  指节着地的四肢行走：对角步态（右臂与左腿同相），身子前倾、肩与臀反向扭，头随步低点，鬃毛与草绳滞后摆
  attack  1.4s 一次  双拳砸地：后仰挺胸、双臂高举过头、下颌张开咆哮 → 双拳同时猛砸向身前地面 → 撑着震颤 → 收回
  throw   1.4s 一次  投掷手里的石头：右臂向后高抬蓄力、身体侧扭、左臂前指找准 → 右臂猛然甩过头顶，石头脱手飞出一道抛物线 → 收势，石头回到手里
  hurt    0.5s 一次  被击后仰一缩，双臂抬起护头，头甩起，鬃毛炸起
  death   2.4s 一次  踉跄 → 膝盖打软、身子前塌 → 侧倒 → 四肢松开，头垂下，下颌松垂

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
骨链：root → hips → spine_lower → spine_upper → neck → head → jaw；腿挂在 hips；肩 shoulder → 上臂 → 前臂 → 掌 → 指，
右手的投掷石 rock_held 是掌的子骨（投掷时靠 pos / scale 单独飞出去）。
符号约定（对着数值核过）：垂下的肢体 rot.x 为正 = 末端向前（-Z）并略上抬；脊 / 颈 / 头 rot.x 为正 = 抬头（身子后仰），为负 = 前倾；
右上臂 rot.z 为正 = 手向外（+X）抬，左臂取反；下颌 rot.x 为负 = 张口；spine rot.y 为正 = 右肩向后（+Z）。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/sui_yan_yuan/gen_anim.py
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

NAME = "sui_yan_yuan"
SOURCE = HERE / "SuiYanYuan_tex.bbmodel"
ANIMATED = HERE / "SuiYanYuan_animated.bbmodel"
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


# ---------------------------------------------------------------- 腿 / 臂
def stride(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (摆角, 抬腿弧)。支撑相脚从前往后划（摆角 +swing → -swing），摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    s = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(s) - 1.0), math.sin(math.pi * s)


def leg(pose: Pose, side: str, u: float, duty: float = 0.62, swing: float = 17.0, fold: float = 40.0) -> None:
    angle, lift = stride(u, duty, swing)
    turn(pose, f"thigh_{side}", rot=(angle, 0, 0))
    turn(pose, f"shin_{side}", rot=(-fold * lift * 0.9 - 0.2 * max(0.0, angle), 0, 0))
    turn(pose, f"foot_{side}", rot=(fold * lift * 0.5 + 0.15 * angle, 0, 0))
    turn(pose, f"toes_{side}", rot=(-10.0 * lift, 0, 0))


def arm(pose: Pose, side: str, swing: float, bend: float = 0.0, out: float = 0.0) -> None:
    """一条手臂：swing 是整条臂向前摆的角（度，正 = 前），bend 是肘弯（度，正 = 前臂向前抬起），out 是向外撑开的角。
    右臂 out 为正 = 向 +X，左臂镜像。"""
    sign = 1.0 if side == "r" else -1.0
    turn(pose, f"upper_arm_{side}", rot=(swing, 0, sign * out))
    turn(pose, f"forearm_{side}", rot=(bend, 0, 0))


def hands(pose: Pose, curl: float, side: str | None = None) -> None:
    """手指握紧程度：curl > 0 = 向手心更握一点（度）。"""
    for sd, _ in SIDES:
        if side is None or side == sd:
            turn(pose, f"fingers_{sd}", rot=(curl, 0, 0))
            turn(pose, f"thumb_{sd}", rot=(0.5 * curl, 0, 0))


def mane(pose: Pose, t: float, sway: float, cycles: float = 1.0, lag: float = 0.15, push_back: float = 0.0) -> None:
    turn(pose, "mane", rot=(-push_back, 0, sway * wave(t, cycles, -lag)))
    for side, sign in SIDES:
        turn(pose, f"pauldron_{side}", rot=(0.4 * sway * wave(t, cycles, -2.0 * lag), 0, 0.4 * sway * sign * wave(t, cycles, -lag)))
    turn(pose, "back_rock_a", rot=(0.3 * sway * wave(t, cycles, -lag), 0, 0))
    turn(pose, "back_rock_b", rot=(0.3 * sway * wave(t, cycles, -lag - 0.1), 0, 0))


def head_pose(pose: Pose, pitch: float, yaw: float = 0.0, roll: float = 0.0, jaw: float = 0.0) -> None:
    """颈 + 头合起来的姿态（pitch 正 = 抬头）：颈承担 40%、头 60%；jaw < 0 张口。"""
    turn(pose, "neck", rot=(0.4 * pitch, 0.4 * yaw, 0.4 * roll))
    turn(pose, "head", rot=(0.6 * pitch, 0.6 * yaw, 0.6 * roll))
    turn(pose, "jaw", rot=(jaw, 0, 0))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "spine_upper", rot=(1.2 * breath, 2.0 * wave(t, 1, 0.3), 1.5 * wave(t, 1, 0.1)), pos=(0, 0.12 * breath, 0))
    turn(pose, "spine_lower", rot=(-0.6 * breath, 0, 0))
    turn(pose, "hips", rot=(0.4 * breath, 1.0 * wave(t, 1, 0.4), 0))
    for side, sign in SIDES:                                     # 两肩一高一低，臂跟着微摆
        turn(pose, f"shoulder_{side}", rot=(0, 0, sign * 1.2 * wave(t, 1, 0.25 if side == "r" else 0.75)))
        arm(pose, side, swing=2.0 * wave(t, 1, 0.5 if side == "l" else 0.0), bend=2.0 * breath, out=0.8 * wave(t, 1, 0.2))
    hands(pose, 5.0 + 4.0 * breath)
    head_pose(pose, pitch=-4.0 + 2.0 * wave(t, 2, 0.1), yaw=14.0 * wave(t, 1, 0.3), roll=3.0 * wave(t, 1, 0.1),
              jaw=-3.0 - 4.0 * max(0.0, wave(t, 3, 0.55)))        # 头慢慢左右扫、下颌咀嚼
    for side, sign in SIDES:
        turn(pose, f"ear_{side}", rot=(0, 0, sign * 6.0 * max(0.0, wave(t, 1, 0.55)) ** 3))
    mane(pose, t, 2.0, cycles=1)
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    # 对角步态：右臂与左腿同相；指节着地时臂向前伸直，向后蹬时微弯
    for side, u0 in (("l", 0.0), ("r", 0.5)):
        leg(pose, side, frac(t + u0))
    for side, u0 in (("r", 0.0), ("l", 0.5)):
        u = frac(t + u0)
        swing = 22.0 * math.sin(math.tau * (u + 0.25))             # 前摆为正：u = 0 时臂在最前
        reach = max(0.0, math.sin(math.tau * (u + 0.5)))
        arm(pose, side, swing=swing, bend=10.0 * reach, out=1.0)
        hands(pose, 12.0)                                           # 指节握成拳着地
    bob = abs(wave(t, 1, 0.0))
    turn(pose, "root", pos=(0, 0.6 * bob, 0))
    turn(pose, "hips", rot=(0, 3.0 * wave(t, 1), 0))
    turn(pose, "spine_lower", rot=(-8.0, -2.0 * wave(t, 1), 0))               # 身子前倾
    turn(pose, "spine_upper", rot=(-5.0 + 1.0 * wave(t, 2, 0.1), -4.0 * wave(t, 1), 2.0 * wave(t, 1, 0.25)))
    head_pose(pose, pitch=11.0 + 3.0 * wave(t, 2, 0.35), yaw=2.0 * wave(t, 1), jaw=-4.0)       # 头抬着看前面，随步点头
    for side, sign in SIDES:
        turn(pose, f"shoulder_{side}", rot=(0, 0, sign * 2.0 * wave(t, 1, 0.5 if side == "l" else 0.0)))
    mane(pose, t, 3.5, cycles=2, lag=0.12)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.4 后仰挺胸、双臂高举过头、张口咆哮；0.4–0.52 双拳猛砸向身前地面；0.52–0.7 撑着震颤；0.7–1 收回
    rear = keys(t, [(0.0, 0.0), (0.4, 1.0), (0.44, 0.7), (0.52, 0.0), (1.0, 0.0)])      # 后仰蓄力
    slam = keys(t, [(0.0, 0.0), (0.4, 0.0), (0.52, 1.0), (0.7, 1.0), (0.9, 0.2), (1.0, 0.0)])     # 砸地
    quake = math.sin(math.tau * 5.0 * (t - 0.52)) * smooth((t - 0.52) / 0.03) * (1.0 - smooth((t - 0.68) / 0.12)) if t > 0.52 else 0.0
    roar = keys(t, [(0.0, 0.0), (0.2, 0.6), (0.4, 1.0), (0.5, 0.2), (0.62, 0.0), (1.0, 0.0)])
    turn(pose, "root", pos=(0, -0.8 * slam + 0.4 * quake, -3.0 * slam))
    turn(pose, "hips", rot=(-5.0 * rear + 8.0 * slam, 0, 0))
    turn(pose, "spine_lower", rot=(10.0 * rear - 18.0 * slam, 0, 0))
    turn(pose, "spine_upper", rot=(8.0 * rear - 14.0 * slam, 0, 0))
    for side, sign in SIDES:
        # 双臂：高举过头（rot.x ≈ +150）→ 砸向前下方（≈ +40，前臂跟着甩直）
        arm(pose, side, swing=150.0 * rear + 38.0 * slam, bend=-30.0 * rear + 14.0 * slam, out=14.0 * rear + 6.0 * slam + 1.5 * quake)
        turn(pose, f"shoulder_{side}", rot=(0, 0, sign * 6.0 * rear))
    hands(pose, 14.0 * rear + 20.0 * slam)
    head_pose(pose, pitch=22.0 * rear - 24.0 * slam + 3.0 * quake, yaw=0.0, jaw=-26.0 * roar)       # 仰头咆哮，砸地时低头
    mane(pose, t, 4.0 + 6.0 * abs(quake), cycles=3, lag=0.1, push_back=-8.0 * rear + 10.0 * slam)
    for side, sign in SIDES:                                     # 双腿微屈撑住，砸地时往后顶
        turn(pose, f"thigh_{side}", rot=(12.0 * rear - 14.0 * slam, 0, 0))
        turn(pose, f"shin_{side}", rot=(-14.0 * rear + 12.0 * slam, 0, 0))
        turn(pose, f"foot_{side}", rot=(4.0 * rear - 4.0 * slam, 0, 0))
    return pose


def throw(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.42 右臂向后高抬蓄力、身体侧扭、左臂前指；0.42–0.55 猛然甩臂过顶、石头脱手（0.5）；
    #       0.55–0.78 石头飞出一道抛物线、收势；0.78–1 复位，石头回到手里
    wind = keys(t, [(0.0, 0.0), (0.42, 1.0), (0.46, 0.8), (0.55, 0.0), (1.0, 0.0)])         # 蓄力
    whip = keys(t, [(0.0, 0.0), (0.42, 0.0), (0.54, 1.0), (0.7, 0.8), (0.92, 0.0), (1.0, 0.0)])   # 甩臂
    twist = keys(t, [(0.0, 0.0), (0.42, 1.0), (0.46, 0.8), (0.58, -0.7), (0.8, -0.2), (1.0, 0.0)])   # 身体侧扭（+ 右肩向后）
    turn(pose, "root", pos=(0, 0, -1.2 * whip + 0.6 * wind))
    turn(pose, "hips", rot=(0, 10.0 * twist * 0.4, 0))
    turn(pose, "spine_lower", rot=(3.0 * wind - 8.0 * whip, 18.0 * twist * 0.5, 0))
    turn(pose, "spine_upper", rot=(4.0 * wind - 10.0 * whip, 24.0 * twist * 0.5, 0))
    # 右臂：蓄力时向后上方（rot.x 为负 = 向后）高抬，外撑；甩出时扫过头顶向前（≈ +105）
    arm(pose, "r", swing=-78.0 * wind + 108.0 * whip, bend=-34.0 * wind + 12.0 * whip, out=34.0 * wind + 10.0 * whip)
    # 左臂：蓄力时向前指（找目标），甩臂时收回护胸
    arm(pose, "l", swing=56.0 * wind - 22.0 * whip, bend=16.0 * wind, out=6.0 * wind)
    hands(pose, 22.0 * wind, side="r")
    hands(pose, 0.0, side="l")
    release = smooth((t - 0.5) / 0.04)
    hands(pose, -26.0 * release * (1.0 - smooth((t - 0.62) / 0.1)), side="r")      # 松手：手指瞬间张开，再放松
    head_pose(pose, pitch=-6.0 * wind + 6.0 * whip, yaw=-16.0 * wind * 0.6, jaw=-8.0 * wind - 14.0 * whip)
    mane(pose, t, 3.0, cycles=2, push_back=6.0 * whip)
    for side, sign in SIDES:                                     # 蓄力时重心后移、出手时前踏
        turn(pose, f"thigh_{side}", rot=((-8.0 if side == "r" else 10.0) * wind + (18.0 if side == "l" else -8.0) * whip, 0, 0))
        turn(pose, f"shin_{side}", rot=((6.0 if side == "r" else -10.0) * wind - (10.0 if side == "l" else 0.0) * whip, 0, 0))
    # 石头：0.5 前跟着手；0.5–0.78 沿抛物线飞出（pos 是在手骨坐标系里的位移，手此时已甩到前上方，所以主要朝 -Z 和向下）；
    # 落地后缩没，0.9 起在手里重新长出来
    flight = keys(t, [(0.5, 0.0), (0.78, 1.0)])
    if t < 0.78:
        visible = 1.0
    elif t < 0.9:
        visible = 0.001
    else:
        visible = max(0.001, smooth((t - 0.9) / 0.1))
    pose["rock_held"].scale = [visible, visible, visible]
    if t < 0.78:                                                  # 落地缩没之后位移归零，0.9 起石头才会在手里长出来
        turn(pose, "rock_held", pos=(0.0, 14.0 * math.sin(math.pi * flight) - 6.0 * flight, -34.0 * flight))
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.15, 1.0), (0.5, 0.0)])         # 受击冲量：快进慢回
    shiver = math.sin(math.tau * 4.0 * t) * (1.0 - smooth((t - 0.15) / 0.3)) * smooth(t / 0.05)
    turn(pose, "root", pos=(0, -0.3 * kick, 1.6 * kick))
    turn(pose, "hips", rot=(-3.0 * kick, 0, 0))
    turn(pose, "spine_lower", rot=(8.0 * kick, 0, 3.0 * shiver))
    turn(pose, "spine_upper", rot=(10.0 * kick, 4.0 * kick, 0))
    for side, sign in SIDES:                                     # 双臂抬起护头
        arm(pose, side, swing=52.0 * kick, bend=40.0 * kick, out=14.0 * kick + 3.0 * shiver)
    hands(pose, 20.0 * kick)
    head_pose(pose, pitch=14.0 * kick, yaw=-8.0 * kick, roll=4.0 * kick, jaw=-22.0 * kick)
    mane(pose, t, 6.0 * shiver, cycles=3, lag=0.08, push_back=-12.0 * kick)
    for side, _ in SIDES:
        turn(pose, f"thigh_{side}", rot=(8.0 * kick, 0, 0))
        turn(pose, f"shin_{side}", rot=(-10.0 * kick, 0, 0))
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    stagger = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.3, 0.6), (1.0, 0.6)])
    buckle = keys(t, [(0.0, 0.0), (0.25, 0.3), (0.5, 1.0), (1.0, 1.0)])         # 膝盖打软
    fall = keys(t, [(0.3, 0.0), (0.68, 1.0), (1.0, 1.0)])                         # 1 = 侧倒到位
    settle = keys(t, [(0.68, 0.0), (0.88, 1.0), (1.0, 1.0)])                      # 倒下后四肢松开
    turn(pose, "root", rot=(0, 0, 84.0 * fall), pos=(0, -2.0 * buckle * (1.0 - fall), 1.0 * stagger))
    turn(pose, "hips", rot=(-3.0 * stagger, 0, 0))
    turn(pose, "spine_lower", rot=(-6.0 * stagger - 14.0 * buckle, 0, 0))
    turn(pose, "spine_upper", rot=(-4.0 * stagger - 10.0 * buckle, 4.0 * stagger, 0))
    head_pose(pose, pitch=10.0 * stagger - 28.0 * buckle + 12.0 * settle, yaw=8.0 * settle, roll=6.0 * settle, jaw=-14.0 * stagger - 14.0 * settle)
    for side, sign in SIDES:
        arm(pose, side, swing=-10.0 * stagger + 30.0 * buckle + 22.0 * settle * sign, bend=14.0 * buckle, out=8.0 * settle)
        turn(pose, f"thigh_{side}", rot=(38.0 * buckle - 16.0 * settle * sign, 0, 0))
        turn(pose, f"shin_{side}", rot=(-70.0 * buckle + 14.0 * settle, 0, 0))
        turn(pose, f"foot_{side}", rot=(26.0 * buckle, 0, 0))
    hands(pose, -14.0 * settle)
    mane(pose, t, 2.0 * (1.0 - settle), cycles=1, push_back=-10.0 * settle)
    return pose


CLIPS = {
    "idle": (3.4, True, idle),
    "walk": (1.5, True, walk),
    "attack": (1.4, False, attack),
    "throw": (1.4, False, throw),
    "hurt": (0.5, False, hurt),
    "death": (2.4, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 8.0          # 一帧最多托起多少 px；超过说明动作本身把身体压进地里了，是动作错而不是需要托


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
