#!/usr/bin/env python3
"""腐沼蟾（fu_zhao_chan）GeckoLib 动画：idle / walk / attack / swell / hurt / death 六段，关键帧从零写。

只落资产、不接线：产物写到暂存目录（modelScript/out/fu_zhao_chan/），不装进 client 资源树。
流程：贴图版 FuZhaoChan_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/fu_zhao_chan/。

动作设计：
  idle    蹲伏呼吸：喉鼓囊以约 1.7 Hz 一下一下鼓动（本只的标志），身体随呼吸微起伏，偶尔眨眼（凸眼缩一下），
          灵草随呼吸轻晃，嘴角微张微合。
  walk    笨重的蹲伏爬行：前肢左右交替撑拉、后肢交替蹬，躯干左右扭摆，头随步子点动，一步一沉。
  attack  长舌弹射：先蓄力（后坐、口张开、舌缩在嘴里），再整条舌沿 -z 弹出去（舌根骨沿 z 拉长 1.6 倍，舌尖下卷），
          身体前扑，随后猛地收舌合口回稳。
  swell   吸饱灵气：深吸一口气后整个身子鼓胀（躯干沿三轴撑大到约 1.22 倍，喉鼓囊撑满，四肢被撑开），
          停留一会儿后缓缓泄回；头 / 四肢 / 灵草做反向缩放抵消，只让躯干膨大，不把脚和脸一起放大。
  hurt    被打：身体被打得向后一缩，凸眼眯起，舌甩出来，口张开，晃两下回稳。
  death   泄气瘫倒：身体先猛吸一口胀一下，随即泄气瘪下去、四肢软开、舌头垂出，整只趴到地上不动。

坐标与符号（面朝 -Z，+X 是右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 朝前（-z）伸出的部件（头 / 下颌 / 舌）：rx < 0 = 前端向下（低头 / 张口），rx > 0 = 前端上抬；
  - 下垂的肢体：rx > 0 = 脚往前（-z）摆，rx < 0 = 往后摆；
  - 躯干 rz = 向右侧倾；ry = 绕竖轴扭；position：-Z = 前，+Y = 上，单位 px。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/fu_zhao_chan/gen_fu_zhao_chan_anim.py
    ... gen_fu_zhao_chan_anim.py --no-gif | --clip swell
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import subprocess
import sys
from base64 import b64decode
from pathlib import Path

from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, write_animated_bbmodel

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
NAME = "fu_zhao_chan"
MODEL = CREATURE_DIR / "FuZhaoChan_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim") / NAME

GROUND_TOLERANCE = 0.8   # 落地后（death 末段）最低点入地不得超过它（px）
SAMPLES = 36             # 每段采样数，build_tracks 再裁掉共线帧
TAU = math.tau

# 躯干三节（臀 → 腰 → 胸）是一条链，缩放沿链累加；这些骨在躯干膨胀时要做反向缩放，免得被一起放大
BODY_CHILDREN = (
    "head", "leg_f1_r", "leg_f1_l", "leg_h1_r", "leg_h1_l",
    "herb_a_r", "herb_a_l", "herb_b_r", "herb_b_l", "herb_c_r", "herb_c_l", "twig_r", "twig_l",
)
HERBS = ("herb_a", "herb_b", "herb_c")


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t01 上走 cycles 个整周期的正弦；循环动画首尾自然相接。"""
    return math.sin(TAU * (cycles * t + phase))


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep：points 为 (t01, 值)，一次性动作用它排节奏。"""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


class Poser:
    """往 Pose 里累加骨骼变换；骨名拼错直接报错。"""

    def __init__(self, rig: PoseRig):
        self.rig = rig

    def turn(self, pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0), scale=None) -> None:
        if bone not in self.rig.bones:
            raise KeyError(f"{NAME}: 没有骨 {bone}")
        channel = pose[bone]
        channel.rot = [a + b for a, b in zip(channel.rot, rot)]
        channel.pos = [a + b for a, b in zip(channel.pos, pos)]
        if scale is not None:
            channel.scale = [float(v) for v in scale]

    def both(self, pose: Pose, stem: str, rot_r=(0.0, 0.0, 0.0), rot_l=None) -> None:
        """左右成对的骨：rot_r 给右侧；rot_l 缺省按镜像（y / z 分量取反）。"""
        if rot_l is None:
            rot_l = (rot_r[0], -rot_r[1], -rot_r[2])
        self.turn(pose, f"{stem}_r", rot=rot_r)
        self.turn(pose, f"{stem}_l", rot=rot_l)


# ---------------------------------------------------------------- 共用动作件
def bloat(poser: Poser, pose: Pose, amount: float) -> None:
    """躯干整体膨胀：amount = 0 → 原样，0.22 → 约 1.22 倍（高度只胀一半，像气球鼓成扁圆）。
    臀骨按倍数放大，它下面所有骨（腰 / 胸 / 头 / 四肢 / 灵草）都会跟着放大，所以头 / 四肢 / 灵草用倒数缩回去，
    它们只被「挪开」（跟着躯干变大的位置），自己的大小不变。"""
    s = (1.0 + amount, 1.0 + 0.55 * amount, 1.0 + amount)
    poser.turn(pose, "body_hips", scale=s)
    inverse = tuple(1.0 / v for v in s)
    for bone in BODY_CHILDREN:
        poser.turn(pose, bone, scale=inverse)


def throat_pulse(poser: Poser, pose: Pose, size: float) -> None:
    """喉鼓囊撑大：size = 0 → 原样；0.3 → 横向竖向都撑大 30%，往前鼓出一点。"""
    poser.turn(pose, "throat", scale=(1.0 + size, 1.0 + 1.1 * size, 1.0 + 0.6 * size), pos=(0, -3.0 * size, -2.4 * size))


def blink(poser: Poser, pose: Pose, closed: float) -> None:
    """凸眼眨一下：眼球沿 y 压扁并往头里缩一点。closed = 0 睁眼，1 全闭。"""
    for stem in ("eye_r", "eye_l"):
        poser.turn(pose, stem, scale=(1.0, 1.0 - 0.85 * closed, 1.0), pos=(0, -1.2 * closed, 0))


def herbs_sway(poser: Poser, pose: Pose, t: float, cycles: float, amp: float) -> None:
    for n, stem in enumerate(HERBS):
        poser.both(pose, stem, rot_r=(amp * wave(t, cycles, 0.17 * n), 0, amp * 0.8 * wave(t, cycles, 0.17 * n + 0.25)))
    poser.both(pose, "twig", rot_r=(0.4 * amp * wave(t, cycles, 0.4), 0, 0.5 * amp * wave(t, cycles, 0.65)))


def tongue_pose(poser: Poser, pose: Pose, length: float, curl: float = 0.0, droop: float = 0.0) -> None:
    """舌：length = 沿 z 拉长的倍数（0.28 缩在嘴里，1 原长，1.6 弹出去）；curl = 舌尖下卷度；droop = 整条舌的下垂（度）。
    舌根骨拉长会带着后面两节一起拉，所以后两节不再单独缩放。"""
    poser.turn(pose, "tongue_1", scale=(1.0, 1.0, length), rot=(-droop, 0, 0))
    poser.turn(pose, "tongue_2", rot=(-droop * 0.5 - 6 * curl, 0, 0))
    poser.turn(pose, "tongue_3", rot=(-14 * curl - droop * 0.5, 0, 0))


def front_leg(poser: Poser, pose: Pose, side: str, swing: float, lift: float = 0.0) -> None:
    """前肢：swing = 上臂前后摆（度，+ = 脚往前）；lift = 抬脚时前臂 / 手掌的折收（度）。"""
    poser.turn(pose, f"leg_f1_{side}", rot=(swing, 0, 0))
    poser.turn(pose, f"leg_f2_{side}", rot=(-0.5 * swing - lift, 0, 0))
    poser.turn(pose, f"leg_f3_{side}", rot=(-0.4 * swing + 0.7 * lift, 0, 0))
    poser.turn(pose, f"toe_f_{side}", rot=(0.5 * lift, 0, 0))


def hind_leg(poser: Poser, pose: Pose, side: str, push: float, lift: float = 0.0, splay: float = 0.0) -> None:
    """后肢：push = 蹬伸（度，+ = 大腿向前摆、小腿 / 跗跖随之展开，用于起跳 / 蹬地）；lift = 抬脚折收；splay = 向外张。"""
    sign = 1.0 if side == "r" else -1.0
    poser.turn(pose, f"leg_h1_{side}", rot=(push * 0.6 + lift * 0.3, 0, sign * -splay * 0.6))
    poser.turn(pose, f"leg_h2_{side}", rot=(-push * 0.9 - lift * 0.5, 0, 0))
    poser.turn(pose, f"leg_h3_{side}", rot=(push * 0.5 + lift * 0.4, 0, 0))
    poser.turn(pose, f"toe_h_{side}", rot=(-push * 0.3, 0, 0))


def fu_zhao_chan_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 蹲伏呼吸：胸 / 腹 0.5 Hz 缓缓起伏，喉鼓囊 1.7 Hz（2.4s 内 4 下）一下一下鼓动，嘴随之微张，偶尔眨眼
        pose = Pose()
        breath = wave(t, 1, 0.0)
        pulse = 0.5 + 0.5 * wave(t, 4, -0.25)                       # 0 → 1 的鼓动，t=0 时在最低
        poser.turn(pose, "body_hips", rot=(0.8 * breath, 0, 0), pos=(0, 0.25 * breath, 0))
        poser.turn(pose, "body_chest", rot=(-0.8 * breath, 0.8 * wave(t, 1, 0.3), 0))
        poser.turn(pose, "head", rot=(1.2 * wave(t, 1, 0.2) - 1.0 * pulse, 2.0 * wave(t, 1, 0.55), 0.8 * wave(t, 1, 0.1)))
        poser.turn(pose, "jaw", rot=(-1.5 - 3.0 * pulse, 0, 0))
        throat_pulse(poser, pose, 0.28 * pulse)
        blink(poser, pose, keys(t, [(0, 0), (0.58, 0), (0.62, 1), (0.68, 0), (1, 0)]))
        front_leg(poser, pose, "r", 1.5 * breath)
        front_leg(poser, pose, "l", -1.5 * breath)
        tongue_pose(poser, pose, 0.3, curl=0.0)                   # 舌缩在嘴里，只露一点
        herbs_sway(poser, pose, t, 1, 2.5)
        return pose

    def walk(t: float) -> Pose:
        # 笨重的蹲伏爬行：前肢和对侧后肢同时迈（对角步），躯干随步子左右扭摆，头点动，每一步身体沉一下
        pose = Pose()
        a = wave(t, 1, 0.0)
        b = wave(t, 1, 0.5)
        step_a = max(0.0, wave(t, 1, 0.25))                        # 右前 + 左后 抬脚的窗口
        step_b = max(0.0, wave(t, 1, 0.75))                        # 左前 + 右后
        dip = 0.5 - 0.5 * wave(t, 2, 0.0)                          # 一周期两步，每步中段身体略沉
        poser.turn(pose, "body_hips", pos=(0, -0.8 * dip, 0), rot=(0, 3.5 * a, 2.0 * b))
        poser.turn(pose, "body_mid", rot=(0, -2.0 * a, 0))
        poser.turn(pose, "body_chest", rot=(0, -3.0 * a, -1.5 * b), pos=(0, 0, 0))
        poser.turn(pose, "head", rot=(-2.0 * dip + 1.5, 2.5 * b, 0))
        poser.turn(pose, "jaw", rot=(-2.0 - 1.5 * dip, 0, 0))
        throat_pulse(poser, pose, 0.12 * (0.5 + 0.5 * wave(t, 2, 0.2)))
        front_leg(poser, pose, "r", 22 * a, lift=26 * step_a)
        front_leg(poser, pose, "l", 22 * b, lift=26 * step_b)
        hind_leg(poser, pose, "l", 18 * a, lift=20 * step_a, splay=4)
        hind_leg(poser, pose, "r", 18 * b, lift=20 * step_b, splay=4)
        tongue_pose(poser, pose, 0.3 + 0.04 * wave(t, 2, 0.1))
        herbs_sway(poser, pose, t, 2, 3.5)
        return pose

    def attack(t: float) -> Pose:
        # 蓄力（0~0.3）：身体后坐压低、口张开、前肢下压；弹舌（0.3~0.5）：舌沿 -z 弹出 1.6 倍长，身体前扑；
        # 停（0.5~0.6）：舌挂在半空；收（0.6~1）：猛地缩回来合口，身体回稳
        pose = Pose()
        wind = keys(t, [(0, 0), (0.3, 1), (0.36, 1), (0.5, 0), (1, 0)])
        lunge = keys(t, [(0, 0), (0.3, 0), (0.42, 1), (0.6, 0.9), (0.85, 0.15), (1, 0)])
        length = keys(t, [(0, 0.3), (0.3, 0.3), (0.42, 1.65), (0.6, 1.6), (0.7, 1.0), (0.82, 0.3), (1, 0.3)])
        open_ = keys(t, [(0, 0), (0.22, 1), (0.6, 1), (0.74, 0.1), (0.8, 0), (1, 0)])
        curl = keys(t, [(0, 0), (0.42, 0), (0.6, 1), (0.74, 0.3), (0.85, 0), (1, 0)])
        poser.turn(pose, "root", pos=(0, -0.4 * wind, 2.0 * wind - 4.5 * lunge))
        poser.turn(pose, "body_hips", rot=(5 * wind - 5 * lunge, 0, 0))
        poser.turn(pose, "body_chest", rot=(-3 * wind + 4 * lunge, 0, 0))
        poser.turn(pose, "head", rot=(-6 * wind + 14 * open_ - 6 * lunge, 0, 0))
        poser.turn(pose, "jaw", rot=(-34 * open_, 0, 0))
        throat_pulse(poser, pose, 0.34 * keys(t, [(0, 0), (0.3, 1), (0.42, 0), (1, 0)]))
        blink(poser, pose, 0.0)
        tongue_pose(poser, pose, length, curl=curl, droop=-36 * open_)          # 下颌张开会把舌带得朝下，这里反向抬回水平
        for side in ("r", "l"):
            front_leg(poser, pose, side, -8 * wind + 12 * lunge)
            hind_leg(poser, pose, side, 10 * lunge, splay=2 * wind)
        herbs_sway(poser, pose, t, 1, 4.0)
        return pose

    def swell(t: float) -> Pose:
        # 吸饱灵气（2.6s，一次性）：0~0.2 蓄气收缩一点、嘴微张；0.2~0.5 整个身子鼓胀，喉囊撑满，四肢被撑开；
        # 0.5~0.72 憋住不动（身子轻颤）；0.72~1 缓缓泄回原样
        pose = Pose()
        pre = keys(t, [(0, 0), (0.12, 1), (0.22, 0), (1, 0)])
        full = keys(t, [(0, 0), (0.2, 0), (0.5, 1), (0.72, 1), (1, 0)])
        tremble = wave(t, 9, 0.0) * full * keys(t, [(0, 0), (0.5, 0), (0.56, 1), (0.72, 1), (0.78, 0), (1, 0)])
        bloat(poser, pose, -0.05 * pre + 0.22 * full + 0.012 * tremble)
        throat_pulse(poser, pose, 0.12 * pre + 0.5 * full)
        poser.turn(pose, "head", rot=(-2.0 * pre + 5.0 * full, 0, 0))
        poser.turn(pose, "jaw", rot=(-5.0 * pre - 3.0 * full, 0, 0))
        blink(poser, pose, 0.35 * full)                           # 憋气时眼睛眯起来
        for side in ("r", "l"):
            sign = 1.0 if side == "r" else -1.0
            front_leg(poser, pose, side, -3.0 * full)
            poser.turn(pose, f"leg_f1_{side}", rot=(0, 0, sign * -6.0 * full))             # 前肢被撑得向外开
            hind_leg(poser, pose, side, -4.0 * full, splay=9.0 * full)
        tongue_pose(poser, pose, 0.3)
        herbs_sway(poser, pose, t, 3, 2.0 + 4.0 * full)           # 灵草被撑得抖
        return pose

    def hurt(t: float) -> Pose:
        # 被打：身体猛地后缩、凸眼眯紧，嘴张开舌头甩出来，前肢蜷起，晃两下回稳
        pose = Pose()
        hit = math.sin(math.pi * min(1.0, t * 1.1)) * (1 - 0.25 * t)
        rattle = math.sin(TAU * 2 * t) * (1 - t)
        poser.turn(pose, "root", pos=(0, -0.5 * hit, 3.0 * hit), rot=(0, 3 * rattle, 5 * rattle))
        poser.turn(pose, "body_hips", rot=(-5 * hit, 0, 0))
        poser.turn(pose, "body_chest", rot=(6 * hit, 0, 0))
        poser.turn(pose, "head", rot=(14 * hit, 6 * rattle, 0))
        poser.turn(pose, "jaw", rot=(-24 * hit, 0, 0))
        blink(poser, pose, hit)
        throat_pulse(poser, pose, 0.3 * hit)
        tongue_pose(poser, pose, 0.3 + 0.9 * hit, curl=hit, droop=10 * hit)
        for side in ("r", "l"):
            front_leg(poser, pose, side, -10 * hit, lift=14 * hit)
            hind_leg(poser, pose, side, 8 * hit, splay=5 * hit)
        herbs_sway(poser, pose, t, 3, 5.0 * (1 - t))
        return pose

    def death(t: float) -> Pose:
        # 泄气瘫倒：0~0.15 猛吸一口胀一下，0.15~0.55 泄气瘪下去（躯干缩到 0.9 倍），四肢软开，头垂到地，
        # 舌头滑出来垂着，眼睛闭上；之后不动
        pose = Pose()
        gulp = keys(t, [(0, 0), (0.15, 1), (0.3, 0.3), (0.55, 0), (1, 0)])
        flat = keys(t, [(0, 0), (0.2, 0), (0.6, 1), (1, 1)])
        sprawl = keys(t, [(0, 0), (0.25, 0), (0.7, 1), (1, 1)])
        bloat(poser, pose, 0.14 * gulp - 0.12 * flat)
        throat_pulse(poser, pose, 0.4 * gulp - 0.15 * flat)
        poser.turn(pose, "root", pos=(0, -2.6 * flat, 0), rot=(0, 0, 8 * flat))
        poser.turn(pose, "body_chest", rot=(-2 * flat, 0, 0))
        poser.turn(pose, "head", rot=(-7 * flat, 0, 5 * flat))
        poser.turn(pose, "jaw", rot=(-20 * keys(t, [(0, 0), (0.4, 1), (1, 1)]), 0, 0))
        blink(poser, pose, keys(t, [(0, 0), (0.3, 0), (0.55, 1), (1, 1)]))
        tongue_pose(poser, pose, 0.3 + 1.0 * keys(t, [(0, 0), (0.4, 0), (0.75, 1), (1, 1)]), curl=0.0, droop=12 * flat)
        for side in ("r", "l"):
            sign = 1.0 if side == "r" else -1.0
            front_leg(poser, pose, side, -6 * sprawl)
            poser.turn(pose, f"leg_f1_{side}", rot=(0, 0, sign * -20 * sprawl))
            hind_leg(poser, pose, side, -6 * sprawl, splay=16 * sprawl)
        herbs_sway(poser, pose, t, 1, 2.0 * (1 - flat))
        return pose

    return {
        "idle": (2.4, True, idle), "walk": (1.0, True, walk), "attack": (1.2, False, attack),
        "swell": (2.6, False, swell), "hurt": (0.6, False, hurt), "death": (1.8, False, death),
    }


# ---------------------------------------------------------------- 落地约束 / 自检
def grounded(rig: PoseRig, poser: Poser, sampler):
    """所有段共用：整只最低点入地的话，把 root 抬到刚好贴地（只管入地，不把腾空的拉下来）。
    膨胀会让躯干和肚皮陷进地里，抬一下就能保证「胀大了是往上撑，不是把地踩穿」。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < 0.0:
            poser.turn(pose, "root", pos=(0.0, -depth, 0.0))
        return pose

    return sample


def check_ground(rig: PoseRig, sampler) -> float:
    """整段的最低点；入地超过容差报错。"""
    lowest = min(rig.lowest(sampler(i / 12)) for i in range(13))
    if lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{NAME}: 最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    return lowest


def loop_gap(sampler) -> tuple[float, float]:
    """循环段首尾姿态的最大差（旋转度数、位移 px）。"""
    a, b = sampler(0.0), sampler(1.0)
    rot = pos = 0.0
    for bone in set(a) | set(b):
        rot = max(rot, max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)))
        pos = max(pos, max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)))
        pos = max(pos, 10 * max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)))   # 缩放差 0.01 ≈ 0.1px
    return rot, pos


# ---------------------------------------------------------------- 导出
def export() -> Path:
    rig = PoseRig(MODEL)
    poser = Poser(rig)
    author = fu_zhao_chan_clips(poser)

    animations = []
    for clip, (length, loop, raw) in author.items():
        sampler = grounded(rig, poser, raw)
        line = f"  {clip}: 最低点 {check_ground(rig, sampler):+.2f}px"
        if loop:
            rot, pos = loop_gap(sampler)
            if rot > 0.5 or pos > 0.1:
                raise ValueError(f"{NAME}.{clip}: 循环首尾不同值（旋转差 {rot:.2f}° 位移 / 缩放差 {pos:.2f}）")
            line += f"，首尾差 {rot:.2f}° / {pos:.2f}"
        print(line)
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})

    STAGING.mkdir(parents=True, exist_ok=True)
    animated = STAGING / f"{NAME}_animated.bbmodel"
    write_animated_bbmodel(rig, animations, animated, NAME)
    subprocess.run(["node", str(CODEC), str(animated), str(STAGING), NAME], check=True)

    # creature_anim_frames.py 按 <assets>/{geo,animations,textures/entity/fauna} 取文件，把暂存目录摆成这个形状
    for sub, name in (("geo", f"{NAME}.geo.json"), ("animations", f"{NAME}.animation.json")):
        (STAGING / sub).mkdir(exist_ok=True)
        shutil.copyfile(STAGING / name, STAGING / sub / name)
    texture = json.loads(MODEL.read_text(encoding="utf-8"))["textures"][0]["source"].split(",", 1)[1]
    (STAGING / "textures" / "entity" / "fauna").mkdir(parents=True, exist_ok=True)
    (STAGING / "textures" / "entity" / "fauna" / f"{NAME}.png").write_bytes(b64decode(texture))

    produced = json.loads((STAGING / f"{NAME}.animation.json").read_text(encoding="utf-8"))["animations"]
    expected = {f"animation.bong.{NAME}.{a['name']}" for a in animations}
    if set(produced) != expected:
        raise ValueError(f"导出的动画集 {sorted(produced)} 与作者稿 {sorted(expected)} 不一致")
    print(f"{NAME}: " + ", ".join(f"{a['name']} {a['length']}s{' loop' if a['loop'] else ''}" for a in animations)
          + f" → {STAGING}")
    return STAGING


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-gif", action="store_true")
    parser.add_argument("--clip", action="append", help="只重渲这几段 GIF（短名）")
    args = parser.parse_args()
    staging = export()
    if args.no_gif:
        return
    GIF_DIR.mkdir(parents=True, exist_ok=True)
    subprocess.run([sys.executable, str(REPO / "modelScript" / "tools" / "creature_anim_frames.py"),
                    "--name", NAME, "--assets", str(staging), "--gif", "--size", "440", "--out", str(GIF_DIR),
                    *[a for c in (args.clip or []) for a in ("--clip", c)]], check=True)


if __name__ == "__main__":
    main()
