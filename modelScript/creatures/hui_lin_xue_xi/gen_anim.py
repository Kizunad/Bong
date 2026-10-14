#!/usr/bin/env python3
"""灰鳞穴蜥动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧，没有套用旧动画）：

  idle    3.0s 循环  伏地呼吸 / 头颈缓慢左右探 / 舌头缩伸两次（舌头是它「看」的方式）/ 尾梢慢摆
  walk    1.2s 循环  贴地扭身爬行：对角步（左前 + 右后一组），脊柱 S 形蛇行，尾巴反向甩，头稳住，舌头探出探入
  attack  1.0s 一次  身体向一侧盘起蓄力 → 整只前扑（下颌张到最大，前爪向前抓）→ 咬合甩头 → 收回
  hurt    0.45s 一次  被击后缩、头甩向一侧、尾卷起、舌头缩回
  death   1.8s 一次  扭动 → 四肢外摊软下 → 侧翻肚皮朝上，下颌张开，舌头拖出
  climb   1.4s 循环  贴壁攀爬：整只竖起头朝上（root 绕 X 转 90°），四肢交替向上抓壁，脊柱 S 形蛇行，尾巴拖在下面左右甩

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴。
蜥蜴四肢横向外展，所以：
  · 上臂 / 大腿的 rot.y 是前后摆（右侧 + = 向前，左侧反号，代码里用 sign 乘一下）；
  · 上臂 / 大腿的 rot.z 是抬起肘膝（右侧 + = 抬起，左侧反号）；
  · 对头 / 颈，rot.x 为正 = 抬头；对下颌，rot.x 为负 = 张口；rot.y 是左右转。
这些符号靠逐帧渲染核过。

用法::

    python3 modelScript/creatures/hui_lin_xue_xi/gen_anim.py        # 出 geo / animation / 贴图，并在 out/ 下做审阅资产目录
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

NAME = "hui_lin_xue_xi"
SOURCE = HERE / "HuiLinXueXi_tex.bbmodel"                  # 带贴图的作者 rig（gen_hui_lin_xue_xi → gen_tex 先出）
ANIMATED = HERE / "HuiLinXueXi_animated.bbmodel"
EXPORT_DIR = HERE / "export"                                # 安装就绪的 geo / animation / 贴图（不装进 client，等接线）
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME   # 审阅 GIF 用的资源根（gitignored）
SAMPLES = 28                                                # 每段动画的采样数，关键帧再按像素容差裁共线点

Sampler = Callable[[float], Pose]
SIDES = (("l", -1.0), ("r", 1.0))
TAIL = ("tail_1", "tail_2", "tail_3", "tail_4", "tail_5", "tail_6")


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
def stride(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (摆角, 抬腿弧)。支撑相脚从前往后划（摆角 +swing → -swing），摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    s = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(s) - 1.0), math.sin(math.pi * s)


def crawl_leg(pose: Pose, front: bool, side: str, sign: float, u: float, duty: float, swing: float, raise_deg: float) -> None:
    """伏地爬行的一条腿：上臂 / 大腿前后摆（rot.y），摆动相把肘膝抬起（rot.z），趾尖随抬起上翘。"""
    angle, lift = stride(u, duty, swing)
    if front:
        turn(pose, f"arm_{side}", rot=(0, sign * angle, sign * raise_deg * lift))
        turn(pose, f"forearm_{side}", rot=(-8.0 * lift, 0, -sign * 10.0 * lift))
        turn(pose, f"hand_{side}", rot=(-10.0 * lift, 0, 0))
        for toe in ("toe_f_in", "toe_f_out"):
            turn(pose, f"{toe}_{side}", rot=(-24.0 * lift, 0, 0))
    else:
        turn(pose, f"thigh_{side}", rot=(0, sign * angle, sign * raise_deg * lift))
        turn(pose, f"shin_{side}", rot=(8.0 * lift, 0, -sign * 10.0 * lift))
        turn(pose, f"foot_{side}", rot=(-10.0 * lift, 0, 0))
        for toe in ("toe_b_in", "toe_b_out"):
            turn(pose, f"{toe}_{side}", rot=(-24.0 * lift, 0, 0))


def tail_wave(pose: Pose, t: float, amp: float, cycles: float, phase: float = 0.0, lag: float = 0.09, droop: float = 0.0) -> None:
    """六节尾行波：越靠尾梢相位越晚、幅度越大；droop > 0 尾梢下垂。"""
    for i, bone in enumerate(TAIL, start=1):
        k = i / len(TAIL)
        turn(pose, bone, rot=(-droop * k, amp * (0.3 + 0.7 * k) * wave(t, cycles, phase - lag * i), 0))


def tongue(pose: Pose, reach: float, wag: float = 0.0, spread: float = 0.0, drop: float = 0.0) -> None:
    """舌头：reach 是伸出长度（1 = 建模稿原长，0.35 = 缩回大半），wag 左右甩（度），spread 舌叉分开（度），drop 舌根下垂（度）。"""
    pose["tongue_1"].scale = [1.0, 1.0, max(0.05, reach)]
    turn(pose, "tongue_1", rot=(-drop, 0.5 * wag, 0))
    turn(pose, "tongue_2", rot=(0, wag, 0))
    for side, sign in SIDES:
        turn(pose, f"tongue_fork_{side}", rot=(0, sign * spread, 0))


def nodules(pose: Pose, shiver: float) -> None:
    """头骨上一排感振的骨瘤：像在接收地面传来的振动，两侧反相微颤。"""
    for side, sign in SIDES:
        turn(pose, f"nodule_{side}", rot=(0.5 * shiver, 0, 0.8 * shiver * sign))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "thorax", rot=(0.8 * breath, 0, 0), pos=(0, 0.1 * breath, 0))
    turn(pose, "spine_lumbar", rot=(-0.4 * breath, 1.2 * wave(t, 1, 0.2), 0))
    turn(pose, "neck_base", rot=(1.0 * wave(t, 1, 0.1), 6.0 * wave(t, 1, 0.3), 0))
    turn(pose, "neck_upper", rot=(1.0 * wave(t, 1, 0.2), 8.0 * wave(t, 1, 0.35), 0))
    turn(pose, "head", rot=(1.5 * wave(t, 2, 0.05), 5.0 * wave(t, 1, 0.45), 1.0 * wave(t, 1, 0.1)))
    turn(pose, "jaw", rot=(-3.0 - 2.0 * max(0.0, wave(t, 1, 0.55)), 0, 0))
    # 舌头每周期探出两次：缩回 → 探出 → 缩回
    tongue(pose, 0.55 + 0.45 * max(0.0, wave(t, 2, -0.1)) ** 2, wag=12.0 * wave(t, 2, 0.2), spread=8.0 * max(0.0, wave(t, 2, -0.1)))
    nodules(pose, 1.0 * wave(t, 3, 0.1))
    tail_wave(pose, t, amp=6.0, cycles=1, droop=2.0)
    for side, sign in SIDES:
        turn(pose, f"arm_{side}", rot=(0, sign * 1.2 * breath, 0))
        turn(pose, f"thigh_{side}", rot=(0, -sign * 1.0 * breath, 0))
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    duty, fore_swing, hind_swing = 0.64, 26.0, 22.0
    phases = {"l": 0.0, "r": 0.5}                   # 左前 + 右后一组
    for side, sign in SIDES:
        crawl_leg(pose, True, side, sign, frac(t + phases[side]), duty, fore_swing, 24.0)
        crawl_leg(pose, False, side, sign, frac(t + phases[side] + 0.5), duty, hind_swing, 24.0)
    # 脊柱 S 形蛇行：肩和臀反向转，尾巴再反一下，头稳住
    turn(pose, "thorax", rot=(0, 7.0 * wave(t, 1), 0), pos=(0, 0.1 * abs(wave(t, 2, 0.25)), 0))
    turn(pose, "spine_lumbar", rot=(0, -9.0 * wave(t, 1), 0))
    turn(pose, "hips", rot=(0, 7.0 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(0, -6.0 * wave(t, 1), 0))
    turn(pose, "neck_upper", rot=(0, -5.0 * wave(t, 1), 0))
    turn(pose, "head", rot=(-1.0 * wave(t, 2, 0.45), -3.0 * wave(t, 1), 0))
    turn(pose, "jaw", rot=(-5.0, 0, 0))
    tongue(pose, 0.6 + 0.4 * max(0.0, wave(t, 2, 0.1)) ** 2, wag=14.0 * wave(t, 2, 0.4), spread=8.0 * max(0.0, wave(t, 2, 0.1)))
    nodules(pose, 1.5 * wave(t, 4, 0.2))
    tail_wave(pose, t, amp=14.0, cycles=1, phase=0.5, droop=2.0)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.3 身体向一侧盘起蓄力；0.3–0.5 整只前扑；0.5–0.62 咬合甩头；0.62–1 收回
    coil = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.45, 0.0), (1.0, 0.0)])
    lunge = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.5, 1.0), (0.62, 1.0), (1.0, 0.0)])
    bite = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.48, 1.0), (0.58, 0.0), (1.0, 0.0)])   # 1 = 口张到最大
    shake = math.sin(math.tau * 3.0 * (t - 0.5)) * smooth((t - 0.5) / 0.04) * (1.0 - smooth((t - 0.62) / 0.1)) if 0.5 < t < 0.75 else 0.0
    turn(pose, "root", pos=(0, 0, -6.0 * lunge + 0.8 * coil))
    # 蓄力：脊柱向一侧弯成 C 形，头颈回拉；扑出时拉直，前身抬起
    turn(pose, "thorax", rot=(-4.0 * lunge, 16.0 * coil, 0))
    turn(pose, "spine_lumbar", rot=(0, 20.0 * coil, 0))
    turn(pose, "hips", rot=(0, 12.0 * coil, 0))
    turn(pose, "neck_base", rot=(8.0 * coil - 10.0 * lunge, -22.0 * coil, 0))
    turn(pose, "neck_upper", rot=(6.0 * coil - 8.0 * lunge, -24.0 * coil, 0))
    turn(pose, "head", rot=(4.0 * coil - 6.0 * lunge, 6.0 * shake - 8.0 * coil, 5.0 * shake))
    turn(pose, "jaw", rot=(-10.0 * coil - 42.0 * bite * lunge - 8.0 * lunge, 0, 0))
    tongue(pose, 0.5 + 0.5 * (1.0 - lunge) + 0.3 * lunge, wag=30.0 * math.sin(math.tau * 3.0 * t) * coil, spread=14.0 * coil)
    nodules(pose, 4.0 * coil)
    for side, sign in SIDES:
        # 前爪：蓄力时撑住，扑出时向前抓出去（上臂前摆 + 抬起）
        turn(pose, f"arm_{side}", rot=(0, sign * (-8.0 * coil + 34.0 * lunge), sign * 18.0 * lunge))
        turn(pose, f"forearm_{side}", rot=(-12.0 * lunge, 0, -sign * 10.0 * lunge))
        for toe in ("toe_f_in", "toe_f_out"):
            turn(pose, f"{toe}_{side}", rot=(-30.0 * lunge, 0, 0))
        # 后腿：蓄力蜷起，扑出时蹬直向后
        turn(pose, f"thigh_{side}", rot=(0, sign * (12.0 * coil - 26.0 * lunge), 0))
        turn(pose, f"shin_{side}", rot=(0, 0, -sign * 8.0 * coil))
    tail_wave(pose, t, amp=10.0 * (1.0 - lunge) + 4.0, cycles=1, phase=0.0, droop=0.0)
    for i, bone in enumerate(TAIL, start=1):
        turn(pose, bone, rot=(0, -14.0 * coil / len(TAIL) * i * 0.5, 0))      # 尾巴随身体盘成 C 形
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.15, 1.0), (0.45, 0.0)])         # 受击冲量：快进慢回
    shiver = math.sin(math.tau * 4.0 * t) * (1.0 - smooth((t - 0.15) / 0.3)) * smooth(t / 0.05)
    turn(pose, "root", pos=(0, 0, 1.2 * kick))
    turn(pose, "thorax", rot=(-3.0 * kick, 8.0 * kick, 0))
    turn(pose, "spine_lumbar", rot=(0, -10.0 * kick, 0))
    turn(pose, "hips", rot=(0, 8.0 * kick, 0))
    turn(pose, "neck_base", rot=(6.0 * kick, -18.0 * kick, 0))
    turn(pose, "neck_upper", rot=(8.0 * kick, -14.0 * kick, 0))
    turn(pose, "head", rot=(8.0 * kick, -8.0 * kick, 4.0 * kick))
    turn(pose, "jaw", rot=(-26.0 * kick, 0, 0))
    tongue(pose, 1.0 - 0.7 * kick, wag=10.0 * shiver, spread=-4.0 * kick)
    nodules(pose, 5.0 * shiver)
    for side, sign in SIDES:
        turn(pose, f"arm_{side}", rot=(0, sign * -14.0 * kick, 0))
        turn(pose, f"thigh_{side}", rot=(0, sign * 10.0 * kick, 0))
    tail_wave(pose, t, amp=10.0 * kick, cycles=2, droop=-12.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    writhe = keys(t, [(0.0, 0.0), (0.12, 1.0), (0.4, 0.5), (0.6, 0.0), (1.0, 0.0)])
    flop = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.62, 1.0), (1.0, 1.0)])           # 1 = 侧翻肚皮朝上
    limp = keys(t, [(0.0, 0.0), (0.5, 0.0), (0.8, 1.0), (1.0, 1.0)])            # 倒下后四肢外摊软下
    twitch = math.sin(math.tau * 5.0 * t) * smooth((t - 0.6) / 0.05) * (1.0 - smooth((t - 0.75) / 0.2))
    turn(pose, "root", rot=(0, 0, 168.0 * flop), pos=(0, 0, 0.6 * writhe))
    turn(pose, "thorax", rot=(0, 14.0 * writhe * math.sin(math.tau * 2.0 * t), 0))
    turn(pose, "spine_lumbar", rot=(0, -18.0 * writhe * math.sin(math.tau * 2.0 * t), 0))
    turn(pose, "hips", rot=(0, 14.0 * writhe * math.sin(math.tau * 2.0 * t), 0))
    turn(pose, "neck_base", rot=(-6.0 * flop, 12.0 * limp, 0))
    turn(pose, "neck_upper", rot=(-6.0 * flop, 14.0 * limp, 0))
    turn(pose, "head", rot=(-8.0 * flop, 10.0 * limp, 6.0 * limp))
    turn(pose, "jaw", rot=(-16.0 * writhe - 22.0 * limp, 0, 0))
    tongue(pose, 1.0, wag=18.0 * limp * (1.0 - 0.5 * twitch), spread=16.0 * limp, drop=10.0 * limp)
    nodules(pose, 3.0 * twitch)
    for side, sign in SIDES:
        turn(pose, f"arm_{side}", rot=(0, sign * (-16.0 * writhe + 24.0 * limp), sign * -12.0 * limp))
        turn(pose, f"forearm_{side}", rot=(0, 0, sign * 18.0 * limp))
        turn(pose, f"thigh_{side}", rot=(0, sign * (10.0 * writhe - 22.0 * limp), sign * -10.0 * limp))
        turn(pose, f"shin_{side}", rot=(0, 0, sign * 16.0 * limp))
        for toe in ("toe_f_in", "toe_f_out", "toe_b_in", "toe_b_out"):
            turn(pose, f"{toe}_{side}", rot=(10.0 * limp * (1 if toe.startswith("toe_f") else 0), 0, sign * 12.0 * limp))
    tail_wave(pose, t, amp=10.0 * (1.0 - limp), cycles=1, droop=-8.0 * limp)
    return pose


def climb(t: float) -> Pose:
    pose = Pose()
    # 整只竖起：root 绕 X 转 90°，头朝上、腹朝 -Z（贴着 z≈1 处的壁面）；root 抬高让尾尖刚好离地
    turn(pose, "root", rot=(90.0, 0, 0), pos=(0, 23.0, 0))
    duty, swing = 0.6, 28.0
    phases = {"l": 0.0, "r": 0.5}                   # 对角：左前 + 右后一组向上抓，另一组向下蹬
    for side, sign in SIDES:
        crawl_leg(pose, True, side, sign, frac(t + phases[side]), duty, swing, 22.0)
        crawl_leg(pose, False, side, sign, frac(t + phases[side] + 0.5), duty, swing * 0.9, 22.0)
        # 趾抓壁：支撑相趾向壁面扣紧，摆动相张开
        grip = 1.0 if frac(t + phases[side]) < duty else 0.0
        for toe in ("toe_f_in", "toe_f_out"):
            turn(pose, f"{toe}_{side}", rot=(14.0 * grip, 0, 0))
    turn(pose, "thorax", rot=(0, 8.0 * wave(t, 1), 0), pos=(0, 0.08 * abs(wave(t, 2, 0.25)), 0))
    turn(pose, "spine_lumbar", rot=(0, -10.0 * wave(t, 1), 0))
    turn(pose, "hips", rot=(0, 8.0 * wave(t, 1), 0))
    turn(pose, "neck_base", rot=(-4.0, -7.0 * wave(t, 1), 0))       # 抬头看上面的壁
    turn(pose, "neck_upper", rot=(-3.0, -6.0 * wave(t, 1), 0))
    turn(pose, "head", rot=(-2.0 * wave(t, 2, 0.45), -3.0 * wave(t, 1), 0))
    turn(pose, "jaw", rot=(-4.0, 0, 0))
    tongue(pose, 0.65 + 0.35 * max(0.0, wave(t, 2, 0.1)) ** 2, wag=14.0 * wave(t, 2, 0.4), spread=8.0 * max(0.0, wave(t, 2, 0.1)))
    nodules(pose, 1.5 * wave(t, 4, 0.2))
    tail_wave(pose, t, amp=16.0, cycles=1, phase=0.5, droop=0.0)
    return pose


CLIPS = {
    "idle": (3.0, True, idle),
    "walk": (1.2, True, walk),
    "attack": (1.0, False, attack),
    "hurt": (0.45, False, hurt),
    "death": (1.8, False, death),
    "climb": (1.4, True, climb),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只蜥蜴（root）托起到刚好贴地。
# 只在入地的帧上移，其余帧原样不动；容差以内的轻微入地（爪尖 / 骨刺）不动。
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
    """两个姿态的最大缩放差。舌头骨在循环首尾必须同样长。"""
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
                raise ValueError(f"{NAME}.{clip}: 循环首尾缩放接不上（舌头骨）")
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
