#!/usr/bin/env python3
"""天道尸蛆动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

五段动画（全部从零写关键帧）：

  idle    3.0s 循环  体节沿身长的呼吸蠕动（节距一松一紧）、口器缓缓开合、三层齿环慢速逆转、触须与呼吸管轻摆、脏器微胀
  crawl   1.8s 循环  波浪蠕行：收缩波从尾传到头（节距收紧 + 小角度拱起），足垫随波依次抬落，头探路
  attack  1.4s 一次  前段昂起后仰蓄力 → 整条身子前扑，四瓣唇翻开、三层齿环依次外翻伸出 → 唇猛合 / 齿环缩回吞咬 → 吞咽波后传
  hurt    0.6s 一次  整条身子一缩、头甩起，唇抽动、齿环乱转，腐液滴一抖
  death   3.0s 一次  抽搐 → 体节从头到尾依次鼓胀 → 爆浆塌陷（体节压扁摊宽、脏器与灵光碎块外溢、腐液拉长）→ 口器松垂

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨的旋转 / 缩放 / 位移。
符号约定（对着数值核过）：
  · 体节链 seg_0 → seg_13 是父子链（头、足、脏器、尾环都挂在它上面）。seg_i 的 rot.x 为正 = 它之后的身体绕它的枢轴
    向下压（也就是「它前面那一端相对抬起」）；rot.y 为正 = 它之后的身体往 +X 摆；
  · 头 rot.x 为正 = 头前端抬起；唇骨 lip_top rot.x 为正 = 上唇前缘抬起（张开），lip_bot 取反，
    lip_l rot.y 为正 = 左唇前缘往 -X 外翻（张开），lip_r 取反；
  · 足：leg_i_a rot.x 为正 = 足向前（-Z）摆；leg_i_b pos.y 为正 = 足垫抬起；
  · 子骨的 pos 是在父骨坐标系里的位移，会被父骨的累积缩放同比缩放——死亡段压扁体节时按这个反推位移。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/tian_dao_shi_qu/gen_anim.py
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
sys.path.insert(0, str(HERE))

import gen_tian_dao_shi_qu as geo  # noqa: E402  体型参数（节数、节高、头高）直接取建模稿的权威值

NAME = "tian_dao_shi_qu"
SOURCE = HERE / "TianDaoShiQu_tex.bbmodel"
ANIMATED = HERE / "TianDaoShiQu_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 32

N_SEG = geo.N_SEG
N_RING = len(geo.MOUTH_RINGS)
LEG_SEGS = tuple(geo.LEG_SEGS)
SIDES = (("l", -1.0), ("r", 1.0))   # (骨名后缀, 符号)：右侧在 +X
Sampler = Callable[[float], Pose]


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


def grow(pose: Pose, bone: str, scale=(1.0, 1.0, 1.0)) -> None:
    channel = pose[bone]
    channel.scale = [channel.scale[i] * scale[i] for i in range(3)]


# ---------------------------------------------------------------- 口器 / 触须 / 尾
def mouth(pose: Pose, open_: float, lip_deg: float = 34.0) -> None:
    """四瓣唇张合。open_ = 0 闭合，1 全开，< 0 咬紧（略过闭合）。"""
    turn(pose, "lip_top", rot=(lip_deg * open_, 0, 0))
    turn(pose, "lip_bot", rot=(-lip_deg * open_, 0, 0))
    turn(pose, "lip_l", rot=(0, lip_deg * open_, 0))
    turn(pose, "lip_r", rot=(0, -lip_deg * open_, 0))


def rings_spin(pose: Pose, angle: float) -> None:
    """三层齿环逆向旋转：相邻两层反向，越往里转得越快。"""
    for k in range(N_RING):
        turn(pose, f"ring_{k}", rot=(0, 0, (1.0 if k % 2 == 0 else -1.0) * angle * (1.0 + 0.25 * k)))


def rings_evert(pose: Pose, amount: float) -> None:
    """齿环外翻伸出：三层像望远镜依次往前（-Z）顶出口外，越往里伸得越远，同时略胀。"""
    for k in range(N_RING):
        turn(pose, f"ring_{k}", pos=(0, 0, -4.2 * (k + 1) * amount))
        grow(pose, f"ring_{k}", (1.0 + 0.12 * amount, 1.0 + 0.12 * amount, 1.0))
    turn(pose, "throat", pos=(0, 0, -4.2 * (N_RING + 1) * amount))


def palps(pose: Pose, t: float, sway: float, droop: float = 0.0) -> None:
    for side, sign in SIDES:
        turn(pose, f"palp_a_{side}", rot=(sway * wave(t, 1, 0.1) + droop, sign * sway * 1.3 * wave(t, 1, 0.35), 0))
        turn(pose, f"palp_b_{side}", rot=(sway * 1.6 * wave(t, 1, -0.05) + droop * 1.4, sign * sway * 2.0 * wave(t, 1, 0.2), 0))


def tail(pose: Pose, t: float, sway: float, cycles: float = 1.0, phase: float = 0.0, droop: float = 0.0) -> None:
    """尾环三节 + 呼吸管三节 + 气门板：越靠末端相位越晚、幅度越大。"""
    for k in range(3):
        turn(pose, f"telson_{k}", rot=(0.4 * sway * wave(t, cycles, phase - 0.06 * k) + droop * 0.3,
                                       0.8 * sway * wave(t, cycles, phase - 0.06 * k + 0.2), 0))
    for k in range(3):
        turn(pose, f"tube_{k}", rot=(1.2 * sway * wave(t, cycles, phase - 0.1 * k - 0.15) + droop * 0.8,
                                     1.0 * sway * wave(t, cycles, phase - 0.1 * k + 0.1), 0))
    turn(pose, "plate", rot=(0, 0, 1.5 * sway * wave(t, cycles, phase + 0.3)))


def seg_spacing(pose: Pose, i: int, delta: float) -> None:
    """第 i 节与前一节的节距变化（px，< 0 收紧）。子节挂在父节上，所以是逐节差分。"""
    turn(pose, f"seg_{i}", pos=(0, 0, delta))


def prolegs(pose: Pose, i: int, swing: float, lift: float) -> None:
    for side, _ in SIDES:
        turn(pose, f"leg_{i}_a_{side}", rot=(swing, 0, 0))
        turn(pose, f"leg_{i}_b_{side}", pos=(0, lift, 0))


# ---------------------------------------------------------------- 五段动画
def idle(t: float) -> Pose:
    pose = Pose()
    for i in range(N_SEG):
        # 呼吸蠕动：节距一松一紧的波沿身长往前传（相位随节号推进），体节微微俯仰蛇行
        seg_spacing(pose, i, 0.45 * wave(t, 1, 0.05 * i))
        turn(pose, f"seg_{i}", rot=(0.7 * wave(t, 1, -0.05 * i), 0.5 * wave(t, 1, 0.3 - 0.04 * i), 0))
        if i in geo.WINDOW_SEGS:
            swell = 0.045 * wave(t, 1, 0.05 * i + 0.2)           # 透皮窗里的脏器随呼吸微微胀缩
            grow(pose, f"gut_{i}", (1.0 + swell, 1.0 + swell, 1.0))
        grow(pose, f"ooze_{i}", (1.0, 1.0 + 0.12 * wave(t, 2, 0.07 * i), 1.0))
    turn(pose, "head", rot=(1.5 * wave(t, 1, 0.1), 2.5 * wave(t, 1, 0.35), 0))
    mouth(pose, 0.3 + 0.3 * wave(t, 1, 0.0))
    rings_spin(pose, 22.0 * wave(t, 1, 0.2))
    grow(pose, "throat", (1.0 + 0.05 * wave(t, 2, 0.1), 1.0 + 0.05 * wave(t, 2, 0.1), 1.0))
    palps(pose, t, 5.0)
    tail(pose, t, 3.0)
    for i in LEG_SEGS:                                           # 足垫隔节轻点
        tap = max(0.0, wave(t, 2, 0.11 * i)) ** 3
        prolegs(pose, i, 1.5 * tap, 0.8 * tap)
    return pose


def crawl(t: float) -> Pose:
    pose = Pose()
    for i in range(N_SEG):
        # 收缩波从尾传到头：相位 +0.07·i（后面的节先收缩）；收缩时节距收紧、身体小角度拱起
        phase = 0.07 * i
        seg_spacing(pose, i, -1.1 * max(0.0, wave(t, 1, phase)) + 0.35)
        turn(pose, f"seg_{i}", rot=(1.0 * wave(t, 1, phase), 1.2 * wave(t, 1, -0.05 * i), 0))
        if i in geo.WINDOW_SEGS:
            squeeze = 0.07 * max(0.0, wave(t, 1, phase))         # 收缩时脏器被挤得略扁、略宽
            grow(pose, f"gut_{i}", (1.0 + squeeze, 1.0 - squeeze * 0.6, 1.0))
        grow(pose, f"ooze_{i}", (1.0, 1.0 + 0.2 * wave(t, 1, phase + 0.2), 1.0))
    for i in LEG_SEGS:
        u = wave(t, 1, 0.07 * i + 0.25)
        step = max(0.0, u)
        prolegs(pose, i, 9.0 * u, 2.6 * step * step)             # 收缩时足垫抬起前移，舒张时落地后蹬
    turn(pose, "head", rot=(4.0 * wave(t, 1, 0.07 * N_SEG), 5.0 * wave(t, 1, -0.02), 0))      # 头探路
    mouth(pose, 0.35 + 0.35 * wave(t, 1, 0.25))
    rings_spin(pose, 34.0 * wave(t, 1, 0.05))
    palps(pose, t, 7.0)
    tail(pose, t, 4.0, phase=0.1)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.28 前段昂起蓄力；0.28–0.46 前扑，唇翻开、齿环依次外翻；0.46–0.6 吞咬（唇猛合、齿环缩回）；0.6–1 落回 + 吞咽波
    rise = keys(t, [(0.0, 0.0), (0.28, 1.0), (0.42, 0.7), (0.56, -0.1), (0.78, 0.0), (1.0, 0.0)])     # 前段昂起
    lunge = keys(t, [(0.0, 0.0), (0.28, -0.12), (0.46, 1.0), (0.58, 0.8), (0.82, 0.05), (1.0, 0.0)])   # 整体前扑
    flare = keys(t, [(0.0, 0.0), (0.24, 0.25), (0.34, 0.5), (0.46, 1.0), (0.52, 1.0), (0.6, -0.25), (0.74, 0.0), (1.0, 0.0)])
    evert = keys(t, [(0.0, 0.0), (0.32, 0.0), (0.47, 1.0), (0.52, 1.0), (0.64, 0.0), (1.0, 0.0)])
    # 前段昂起：seg_0 整体仰 36°，之后每节回收 7.2°，到 seg_5 又放平——只有前 6 节拱起，后面还趴在地上
    turn(pose, "seg_0", rot=(36.0 * rise, 0, 0))
    for i in range(1, 6):
        turn(pose, f"seg_{i}", rot=(-7.2 * rise, 0, 0))
    turn(pose, "root", pos=(0, 0, 5.0 * max(0.0, rise) - 13.0 * lunge))
    for i in range(N_SEG):
        # 蓄力时全身节距收紧（像弹簧压缩），前扑时拉开
        seg_spacing(pose, i, -0.8 * max(0.0, rise) * smooth(t / 0.28) + 0.5 * max(0.0, lunge))
    turn(pose, "head", rot=(-28.0 * lunge + 6.0 * rise, 3.5 * wave(t, 4, 0.0) * smooth((t - 0.46) / 0.1) * (1.0 - smooth((t - 0.62) / 0.15)), 0))
    mouth(pose, flare, lip_deg=48.0)
    rings_evert(pose, evert)
    rings_spin(pose, 160.0 * smooth(t / 0.45) * (1.0 - 0.5 * smooth((t - 0.6) / 0.3)))
    palps(pose, t, 4.0, droop=-18.0 * lunge)                      # 前扑时触须向后伏倒
    tail(pose, t, 5.0, cycles=2, droop=-6.0 * lunge)
    # 吞咽波：0.6 之后一团鼓包从头滚到尾，经过的节脏器胀大、节距拉开
    for i in range(N_SEG):
        bulge = math.exp(-((t - 0.62 - 0.28 * i / N_SEG) / 0.07) ** 2)
        if i in geo.WINDOW_SEGS:
            grow(pose, f"gut_{i}", (1.0 + 0.22 * bulge, 1.0 + 0.16 * bulge, 1.0))
        seg_spacing(pose, i, 0.5 * bulge)
    for i in LEG_SEGS:
        prolegs(pose, i, 8.0 * lunge, 2.4 * abs(lunge))
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.16, 1.0), (0.5, 0.0), (1.0, 0.0)])         # 受击冲量：快进慢回
    for i in range(N_SEG):
        # S 形一缩：前后反向弯，波往后传；节距整体一收
        turn(pose, f"seg_{i}", rot=(-1.6 * kick * (1.0 if i < 5 else 0.0), 5.5 * kick * math.sin(math.pi * (i + 1) / 7.0), 0))
        seg_spacing(pose, i, -0.9 * kick)
        grow(pose, f"ooze_{i}", (1.0, 1.0 + 0.6 * kick * wave(t, 3, 0.1 * i), 1.0))
        if i in geo.WINDOW_SEGS:
            grow(pose, f"gut_{i}", (1.0 + 0.1 * kick, 1.0 + 0.1 * kick, 1.0))
    turn(pose, "root", pos=(0, 0, 3.0 * kick))
    turn(pose, "head", rot=(12.0 * kick, -8.0 * kick, 4.0 * kick))
    mouth(pose, 0.9 * kick + 0.15 * wave(t, 4, 0.0) * kick)
    rings_spin(pose, 70.0 * kick)
    palps(pose, t, 12.0 * kick)
    tail(pose, t, 9.0 * kick, cycles=3)
    for i in LEG_SEGS:
        prolegs(pose, i, -6.0 * kick, 2.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    twitch = keys(t, [(0.0, 0.0), (0.06, 1.0), (0.2, 0.7), (0.34, 0.0), (1.0, 0.0)])
    # 体节从头到尾依次：先鼓胀（0.14–0.5），再爆浆塌陷（0.42–0.9）。每节有自己的延迟，波往后传。
    sx, sy, burst = [], [], []
    for i in range(N_SEG):
        delay = i / (N_SEG - 1)
        swell = keys(t, [(0.12 + 0.2 * delay, 0.0), (0.4 + 0.2 * delay, 1.0)])
        flat = keys(t, [(0.44 + 0.26 * delay, 0.0), (0.7 + 0.2 * delay, 1.0)])
        sx.append(1.0 + 0.1 * swell + 0.38 * flat)
        sy.append(1.0 + 0.1 * swell - (0.1 * swell + 0.72) * flat)
        burst.append(flat)

    # 体节缩放：子节继承父节的累积缩放，所以写 seg_i.scale = 目标累积缩放 / 父节累积缩放；
    # 枢轴高度按「脚底（离枢轴 seg_y）落在地面」反推位移，压扁后躺在地上而不是悬空。
    prev_sx = prev_sy = 1.0
    prev_world_y, prev_origin_y = 0.0, 0.0
    for i in range(N_SEG):
        grow(pose, f"seg_{i}", (sx[i] / prev_sx, sy[i] / prev_sy, 1.0))
        origin_y = geo.seg_y(i)
        target_y = origin_y * sy[i]
        pos_y = (target_y - prev_world_y) / prev_sy - (origin_y - prev_origin_y)
        turn(pose, f"seg_{i}", pos=(0, pos_y, 0))
        turn(pose, f"seg_{i}", rot=(-1.0 * twitch * (1.0 if i < 5 else 0.0), 2.0 * twitch * wave(t, 3, -0.1 * i), 0))
        prev_sx, prev_sy, prev_world_y, prev_origin_y = sx[i], sy[i], target_y, origin_y

    # 头：只塌一半（颅壳硬一些），同样反推位移落到地面
    head_s = 1.0 - 0.45 * keys(t, [(0.45, 0.0), (0.85, 1.0)])
    grow(pose, "head", (1.0 / sx[0] * (1.0 + 0.2 * burst[0]), head_s / sy[0], 1.0))
    head_y = geo.HEAD_Y
    turn(pose, "head", pos=(0, ((geo.HEAD_A + 0.5) * head_s - sy[0] * geo.seg_y(0)) / sy[0] - (head_y - geo.seg_y(0)), 0))
    turn(pose, "head", rot=(10.0 * twitch - 14.0 * keys(t, [(0.5, 0.0), (0.9, 1.0)]), 6.0 * twitch, 0))
    # 口器：抽搐时大张，随后松垂半张；齿环停转，慢慢外翻一点又塌回
    mouth(pose, 0.9 * twitch + 0.55 * keys(t, [(0.3, 0.0), (0.7, 1.0)]) * (1.0 - twitch), lip_deg=40.0)
    rings_spin(pose, 200.0 * keys(t, [(0.0, 0.0), (0.25, 1.0), (0.6, 1.25), (1.0, 1.3)]))
    rings_evert(pose, 0.35 * keys(t, [(0.5, 0.0), (0.7, 1.0), (1.0, 0.6)]))
    palps(pose, t, 4.0 * (1.0 - burst[0]), droop=40.0 * keys(t, [(0.4, 0.0), (0.9, 1.0)]))

    # 爆浆：脏器与灵光碎块挣破皮环外溢——抵消皮环的压扁，再往两侧摊宽；腐液滴摊开
    for i in range(N_SEG):
        b = burst[i]
        if i in geo.WINDOW_SEGS:
            gx, gy = 1.0 + 0.9 * b, 1.0 - 0.35 * b + 0.35 * math.sin(math.pi * b)
            grow(pose, f"gut_{i}", (gx / sx[i], gy / sy[i], 1.0 + 0.25 * b))
        grow(pose, f"ooze_{i}", (1.0 + 0.8 * b, 1.0 + 0.4 * b, 1.0))     # 腐液跟着压扁的体节走（继承缩放），只再摊宽一点
    for i in LEG_SEGS:                                  # 足垫缩进压扁的腹下，抽搐几下
        prolegs(pose, i, -10.0 * burst[i], 1.0 * twitch * wave(t, 4, 0.07 * i))
    tail(pose, t, 2.0 * twitch, cycles=3, droop=36.0 * keys(t, [(0.5, 0.0), (0.95, 1.0)]))
    return pose


CLIPS = {
    "idle": (3.0, True, idle),
    "crawl": (1.8, True, crawl),
    "attack": (1.4, False, attack),
    "hurt": (0.6, False, hurt),
    "death": (3.0, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
# 前段昂起会让后段翘起 / 沉下十几 px，所以托起上限比小型生物宽。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 40.0


def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < -GROUND_TOLERANCE:
            turn(pose, "root", pos=(0, -depth, 0))
        return pose

    return sample


def check_ground(clip: str, rig: PoseRig, sampler: Sampler) -> float:
    lowest = min(rig.lowest(sampler(step / SAMPLES)) for step in range(SAMPLES + 1))
    if lowest < -MAX_LIFT:
        raise ValueError(f"{NAME}.{clip}: 最低点 {lowest:.2f}px，入地超过 {MAX_LIFT}px，动作把身体压进地里了")
    return lowest


def seam_gap(a: Pose, b: Pose) -> float:
    """两个姿态的最大旋转 / 位移差。循环动画首尾必须接上。"""
    rot = max((max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)) for bone in set(a) | set(b)), default=0.0)
    pos = max((max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)) for bone in set(a) | set(b)), default=0.0)
    return max(rot, pos * 2.0)


def build() -> list[dict]:
    rig = PoseRig(SOURCE)
    animations = []
    for clip, (length, loop, raw) in CLIPS.items():
        if loop:
            gap = seam_gap(raw(0.0), raw(1.0))
            if gap > 0.5:
                raise ValueError(f"{NAME}.{clip}: 循环首尾差 {gap:.2f} 接不上")
        sampler = grounded(rig, raw)
        lowest = check_ground(clip, rig, sampler)
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
