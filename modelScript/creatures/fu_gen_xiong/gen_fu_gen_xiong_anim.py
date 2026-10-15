#!/usr/bin/env python3
"""腐根熊（fu_gen_xiong）GeckoLib 动画：idle / walk / attack / spore / hurt / death 六段，关键帧从零写。

只落资产、不接线：产物写到暂存目录（modelScript/out/fu_gen_xiong/），不装进 client 资源树。
流程：贴图版 FuGenXiong_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/fu_gen_xiong/。

动作设计（笨重的大熊，背上长着菌和枯枝）：
  idle    蹲伏呼吸：胸腹缓缓起伏，头慢慢左右晃，耳朵抖，尾根微摆；背上菌簇和肩上枯枝随呼吸轻轻颤。
  walk    对角步的沉重爬行：前肢与对侧后肢同时迈，躯干随步子左右扭、肩胛起伏，头随步点动，一步一沉；菌与枯枝跟着晃。
  attack  人立扑击：先后坐蓄力、嘴张开，再整个前身抬起人立（后腿撑地，前掌高举），随后前掌自上而下狠狠拍落、
          身体前扑压下，最后撑回四肢站稳。
  spore   菌盖喷孢子：深吸一口气（胸腔鼓起、头抬起），随即背上所有菌簇一齐猛地鼓胀再回缩、向外翻开喷出孢子，
          枯枝抖动，身体微蹲，最后缓缓收回。（孢子本身是粒子，不在本资产里，这里只做菌盖的缩放和抖动。）
  hurt    被打：身体猛缩，头甩向一侧，菌簇与枯枝被震得乱颤，晃两下回稳。
  death   侧倒：先踉跄一下，前肢软掉，整只向右侧倒地，头垂到地上，菌盖缩瘪、枯枝垂下，之后不动。

坐标与符号（面朝 -Z，+X 是右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 朝前（-z）伸出的部件（头 / 胸 / 颈）：rx > 0 = 前端上抬（抬头 / 人立），rx < 0 = 前端下压；
  - 下垂的肢体：rx > 0 = 脚往前（-z）摆，rx < 0 = 往后摆并抬起（折膝）；
  - 躯干 rz = 向右侧倾；ry = 绕竖轴扭；position：-Z = 前，+Y = 上，单位 px。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/fu_gen_xiong/gen_fu_gen_xiong_anim.py
    ... gen_fu_gen_xiong_anim.py --no-gif | --clip spore
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
NAME = "fu_gen_xiong"
MODEL = CREATURE_DIR / "FuGenXiong_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim") / NAME

GROUND_TOLERANCE = 0.8   # 落地后最低点入地不得超过它（px）
SAMPLES = 36             # 每段采样数，build_tracks 再裁掉共线帧
TAU = math.tau

FUNGUS = ("fungus_hump", "fungus_back_a", "fungus_back_b", "fungus_back_c")
FUNGUS_SIDE = ("fungus_flank", "fungus_rear")
TENDRILS = ("tendril_a", "tendril_b", "tendril_c")


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
def front_leg(poser: Poser, pose: Pose, side: str, swing: float, lift: float = 0.0, spread: float = 0.0) -> None:
    """前肢：swing = 上臂前后摆（度，+ = 脚往前）；lift = 抬脚时前臂 / 掌的折收；spread = 向外张（度）。"""
    sign = 1.0 if side == "r" else -1.0
    poser.turn(pose, f"arm_{side}", rot=(swing, 0, sign * -spread))
    poser.turn(pose, f"forearm_{side}", rot=(-0.4 * swing - lift, 0, 0))
    poser.turn(pose, f"paw_f_{side}", rot=(-0.3 * swing + 0.6 * lift, 0, 0))
    poser.turn(pose, f"toes_f_{side}", rot=(0.4 * lift, 0, 0))


def hind_leg(poser: Poser, pose: Pose, side: str, swing: float, lift: float = 0.0, splay: float = 0.0) -> None:
    """后肢：swing = 大腿前后摆；lift = 抬脚折收；splay = 向外张。"""
    sign = 1.0 if side == "r" else -1.0
    poser.turn(pose, f"thigh_{side}", rot=(swing, 0, sign * -splay))
    poser.turn(pose, f"shin_{side}", rot=(-0.6 * swing - 0.6 * lift, 0, 0))
    poser.turn(pose, f"hock_{side}", rot=(0.5 * swing + 0.7 * lift, 0, 0))
    poser.turn(pose, f"toes_b_{side}", rot=(-0.3 * lift, 0, 0))


def growth_sway(poser: Poser, pose: Pose, t: float, cycles: float, amp: float, phase: float = 0.0) -> None:
    """背上的菌簇、肩上的枯枝、跨背细根随身体轻轻晃：各骨相位错开，枯枝比菌晃得多。"""
    for n, bone in enumerate(FUNGUS):
        poser.turn(pose, bone, rot=(amp * 0.5 * wave(t, cycles, phase + 0.13 * n), 0, amp * 0.4 * wave(t, cycles, phase + 0.13 * n + 0.25)))
    for n, bone in enumerate(TENDRILS):
        poser.turn(pose, bone, rot=(0, 0, amp * 0.5 * wave(t, cycles, phase + 0.2 * n + 0.1)))
    poser.both(pose, "branch", rot_r=(amp * 1.4 * wave(t, cycles, phase + 0.05), 0, amp * 1.8 * wave(t, cycles, phase + 0.3)),
               rot_l=(amp * 1.4 * wave(t, cycles, phase + 0.45), 0, -amp * 1.8 * wave(t, cycles, phase + 0.2)))


def fungus_puff(poser: Poser, pose: Pose, size: float, tilt: float = 0.0) -> None:
    """菌簇整体鼓胀 / 缩瘪：size = 0 原样，0.4 → 1.4 倍；tilt = 菌盖向外翻开的角度。"""
    s = 1.0 + size
    for n, bone in enumerate(FUNGUS):
        poser.turn(pose, bone, scale=(s, 1.0 + 0.6 * size, s), rot=(tilt * (-1) ** n * 0.4, 0, tilt * 0.5))
    for side in ("r", "l"):
        for base in FUNGUS_SIDE:
            poser.turn(pose, f"{base}_{side}", scale=(1.0 + 0.5 * size, s, s))


def fu_gen_xiong_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 蹲伏呼吸：0.33 Hz 缓慢起伏，头慢慢左右晃，耳朵抖，尾根微摆
        pose = Pose()
        breath = wave(t, 1, 0.0)
        poser.turn(pose, "spine_lumbar", rot=(0.8 * breath, 0.6 * wave(t, 1, 0.3), 0), pos=(0, 0.18 * breath, 0))
        poser.turn(pose, "thorax", rot=(-0.6 * breath, 1.0 * wave(t, 1, 0.5), 0), scale=(1.0 + 0.012 * breath, 1.0 + 0.02 * breath, 1.0))
        poser.turn(pose, "neck_base", rot=(0.5 * wave(t, 1, 0.2), 1.5 * wave(t, 1, 0.55), 0))
        poser.turn(pose, "head", rot=(1.0 * wave(t, 1, 0.15), 3.0 * wave(t, 1, 0.6), 1.0 * wave(t, 1, 0.1)))
        poser.turn(pose, "jaw", rot=(-1.5 - 1.5 * max(0.0, wave(t, 1, 0.25)), 0, 0))
        poser.both(pose, "ear", rot_r=(0, 0, 3.0 * wave(t, 3, 0.1)), rot_l=(0, 0, -3.0 * wave(t, 3, 0.4)))
        poser.turn(pose, "tail_1", rot=(0, 3.0 * wave(t, 1, 0.2), 0))
        poser.turn(pose, "tail_2", rot=(0, 5.0 * wave(t, 1, 0.35), 0))
        front_leg(poser, pose, "r", 1.0 * breath)
        front_leg(poser, pose, "l", -1.0 * breath)
        growth_sway(poser, pose, t, 1, 1.6)
        return pose

    def walk(t: float) -> Pose:
        # 沉重的对角步：右前 + 左后同时迈，躯干随步子扭摆，肩胛起伏，每步身体沉一下
        pose = Pose()
        a = wave(t, 1, 0.0)
        b = wave(t, 1, 0.5)
        step_a = max(0.0, wave(t, 1, 0.25))
        step_b = max(0.0, wave(t, 1, 0.75))
        dip = 0.5 - 0.5 * wave(t, 2, 0.0)
        poser.turn(pose, "root", pos=(0, -0.5 * dip, 0))
        poser.turn(pose, "spine_lumbar", rot=(0, 2.5 * a, 1.5 * b))
        poser.turn(pose, "spine_mid", rot=(0, -3.0 * a, 0))
        poser.turn(pose, "hips", rot=(0, 1.5 * a, -1.0 * b))
        poser.turn(pose, "thorax", rot=(1.0 - 1.5 * dip, -2.0 * a, 0))
        poser.turn(pose, "neck_base", rot=(0, 2.0 * b, 0))
        poser.turn(pose, "head", rot=(-1.0 * dip, 2.5 * a, 0))
        poser.turn(pose, "jaw", rot=(-2.0 - 1.0 * dip, 0, 0))
        poser.both(pose, "ear", rot_r=(0, 0, 2.0 * wave(t, 2, 0.3)), rot_l=(0, 0, -2.0 * wave(t, 2, 0.6)))
        poser.turn(pose, "tail_1", rot=(0, 5.0 * a, 0))
        poser.turn(pose, "tail_2", rot=(0, 7.0 * wave(t, 1, 0.15), 0))
        front_leg(poser, pose, "r", 20 * a, lift=24 * step_a)
        front_leg(poser, pose, "l", 20 * b, lift=24 * step_b)
        hind_leg(poser, pose, "l", 16 * a, lift=20 * step_a, splay=2)
        hind_leg(poser, pose, "r", 16 * b, lift=20 * step_b, splay=2)
        growth_sway(poser, pose, t, 2, 2.6)
        return pose

    def attack(t: float) -> Pose:
        # 蓄力（0~0.3）：前身压低后坐、嘴张开；人立（0.3~0.5）：前身抬起，前掌高举；
        # 拍落（0.5~0.68）：前掌自上而下狠拍、前身压下；收（0.68~1）：撑回站稳
        pose = Pose()
        crouch = keys(t, [(0, 0), (0.28, 1), (0.34, 1), (0.5, 0), (1, 0)])
        rear = keys(t, [(0, 0), (0.3, 0), (0.5, 1), (0.56, 1), (0.68, 0), (0.85, 0.05), (1, 0)])
        slam = keys(t, [(0, 0), (0.5, 0), (0.62, 1), (0.74, 0.7), (1, 0)])
        paws = keys(t, [(0, 0), (0.3, 0), (0.5, 1), (0.56, 1), (0.64, -0.6), (0.78, -0.2), (1, 0)])        # 前掌：高举 → 拍下
        poser.turn(pose, "root", pos=(0, 0, 3.0 * crouch + 2.0 * rear - 3.0 * slam))
        poser.turn(pose, "spine_lumbar", rot=(46 * rear - 10 * crouch - 10 * slam, 0, 0))
        poser.turn(pose, "spine_mid", rot=(-27 * rear + 4 * crouch, 0, 0))
        poser.turn(pose, "hips", rot=(-19 * rear + 6 * crouch, 0, 0))
        poser.turn(pose, "thorax", rot=(10 * rear - 6 * crouch - 14 * slam, 0, 0))
        poser.turn(pose, "neck_base", rot=(-4 * rear + 4 * slam, 0, 0))
        poser.turn(pose, "head", rot=(-6 * rear + 8 * crouch - 10 * slam, 0, 0))
        poser.turn(pose, "jaw", rot=(-30 * max(crouch, rear) - 6 * slam, 0, 0))
        for side in ("r", "l"):
            sign = 1.0 if side == "r" else -1.0
            # 前掌高举时朝上前方，拍下时自上而下挥到前方地面；左右错开一点相位
            swing = 70 * paws if paws > 0 else 60 * paws
            front_leg(poser, pose, side, swing * (1.0 + 0.08 * sign), lift=-10 * paws, spread=10 * rear)
            hind_leg(poser, pose, side, -6 * rear + 10 * crouch, splay=3 * rear)
        poser.turn(pose, "tail_1", rot=(8 * rear, 0, 0))
        growth_sway(poser, pose, t, 3, 3.0 * (rear + slam))
        return pose

    def spore(t: float) -> Pose:
        # 吸气（0~0.3）：胸腔鼓起、头抬起；喷发（0.3~0.5）：所有菌簇一齐猛鼓再缩，向外翻开，枯枝抖动，身体微蹲；收（0.5~1）
        pose = Pose()
        inhale = keys(t, [(0, 0), (0.3, 1), (0.38, 1), (0.52, 0), (1, 0)])
        burst = keys(t, [(0, 0), (0.3, 0), (0.4, 1), (0.52, 0.35), (0.7, 0.1), (1, 0)])
        shake = wave(t, 7, 0.0) * keys(t, [(0, 0), (0.32, 0), (0.4, 1), (0.7, 0.4), (1, 0)])
        poser.turn(pose, "root", pos=(0, -0.8 * burst, 0))
        poser.turn(pose, "thorax", rot=(6 * inhale - 3 * burst, 0, 0), scale=(1.0 + 0.06 * inhale, 1.0 + 0.1 * inhale, 1.0 + 0.04 * inhale))
        poser.turn(pose, "spine_lumbar", rot=(-2 * burst, 0, 0))
        poser.turn(pose, "neck_base", rot=(5 * inhale, 0, 0))
        poser.turn(pose, "head", rot=(8 * inhale - 6 * burst, 0, 0))
        poser.turn(pose, "jaw", rot=(-4 - 14 * burst, 0, 0))
        fungus_puff(poser, pose, 0.45 * burst + 0.08 * inhale, tilt=18 * burst)
        poser.both(pose, "branch", rot_r=(5 * shake, 0, 6 * shake), rot_l=(-5 * shake, 0, -6 * shake))
        for n, bone in enumerate(TENDRILS):
            poser.turn(pose, bone, rot=(0, 0, 4 * shake * (-1) ** n))
        for side in ("r", "l"):
            front_leg(poser, pose, side, -2 * burst)
            hind_leg(poser, pose, side, 3 * burst, splay=3 * burst)
        poser.turn(pose, "tail_2", rot=(0, 6 * shake, 0))
        return pose

    def hurt(t: float) -> Pose:
        # 被打：身体猛缩、头甩向一侧、菌与枯枝被震得乱颤，晃两下回稳
        pose = Pose()
        hit = math.sin(math.pi * min(1.0, t * 1.1)) * (1 - 0.25 * t)
        rattle = math.sin(TAU * 2.5 * t) * (1 - t)
        poser.turn(pose, "root", pos=(0, -0.4 * hit, 2.4 * hit), rot=(0, 3 * rattle, 4 * rattle))
        poser.turn(pose, "spine_lumbar", rot=(-4 * hit, 0, 0))
        poser.turn(pose, "thorax", rot=(6 * hit, 4 * rattle, 0))
        poser.turn(pose, "neck_base", rot=(4 * hit, 8 * hit, 0))
        poser.turn(pose, "head", rot=(8 * hit, 10 * hit + 5 * rattle, 5 * hit))
        poser.turn(pose, "jaw", rot=(-20 * hit, 0, 0))
        poser.both(pose, "ear", rot_r=(0, 0, 10 * hit), rot_l=(0, 0, -10 * hit))
        for side in ("r", "l"):
            front_leg(poser, pose, side, -8 * hit, lift=10 * hit)
            hind_leg(poser, pose, side, 6 * hit, splay=4 * hit)
        fungus_puff(poser, pose, -0.1 * hit, tilt=8 * hit)
        growth_sway(poser, pose, t, 4, 5.0 * (1 - t))
        poser.turn(pose, "tail_1", rot=(0, 8 * rattle, 0))
        return pose

    def death(t: float) -> Pose:
        # 侧倒：踉跄（0~0.25），前肢软掉，整只向右侧倒（0.25~0.7），头垂到地，菌盖缩瘪、枯枝垂下，之后不动
        pose = Pose()
        stagger = math.sin(math.pi * min(1.0, t / 0.25)) if t < 0.25 else 0.0
        fall = keys(t, [(0, 0), (0.22, 0), (0.7, 1), (1, 1)])
        slack = keys(t, [(0, 0), (0.2, 0), (0.6, 1), (1, 1)])
        wilt = keys(t, [(0, 0), (0.3, 0), (0.85, 1), (1, 1)])
        poser.turn(pose, "root", pos=(0, 0, 0), rot=(0, 0, 88 * fall + 4 * stagger))
        poser.turn(pose, "spine_lumbar", rot=(0, 6 * stagger, 0))
        poser.turn(pose, "thorax", rot=(-5 * slack, 0, 4 * slack))
        poser.turn(pose, "neck_base", rot=(-8 * slack, 0, 10 * slack))
        poser.turn(pose, "head", rot=(-10 * slack, 0, 12 * slack))
        poser.turn(pose, "jaw", rot=(-14 * keys(t, [(0, 0), (0.4, 1), (1, 1)]), 0, 0))
        poser.both(pose, "ear", rot_r=(0, 0, 16 * slack), rot_l=(0, 0, -16 * slack))
        for side in ("r", "l"):
            sign = 1.0 if side == "r" else -1.0
            front_leg(poser, pose, side, -10 * slack, lift=20 * slack, spread=14 * slack)
            hind_leg(poser, pose, side, 8 * slack, lift=10 * slack, splay=10 * slack)
        fungus_puff(poser, pose, -0.5 * wilt, tilt=-20 * wilt)
        poser.both(pose, "branch", rot_r=(0, 0, 22 * wilt), rot_l=(0, 0, -22 * wilt))
        poser.turn(pose, "tail_1", rot=(0, 10 * slack, 0))
        return pose

    return {
        "idle": (3.0, True, idle), "walk": (1.2, True, walk), "attack": (1.4, False, attack),
        "spore": (2.6, False, spore), "hurt": (0.6, False, hurt), "death": (2.0, False, death),
    }


# ---------------------------------------------------------------- 落地约束 / 自检
def grounded(rig: PoseRig, poser: Poser, sampler):
    """所有段共用：整只最低点入地的话，把 root 抬到刚好贴地（只管入地，不把腾空的拉下来）。
    人立和侧倒都会让某一部分压进地里，抬一下保证「身体立起来或倒下去是在地面上，不是踩穿地面」。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < 0.0:
            poser.turn(pose, "root", pos=(0.0, -depth, 0.0))
        return pose

    return sample


def check_ground(rig: PoseRig, sampler) -> float:
    lowest = min(rig.lowest(sampler(i / 12)) for i in range(13))
    if lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{NAME}: 最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    return lowest


def loop_gap(sampler) -> tuple[float, float]:
    """循环段首尾姿态的最大差（旋转度数、位移 px；缩放差 0.01 折成 0.1）。"""
    a, b = sampler(0.0), sampler(1.0)
    rot = pos = 0.0
    for bone in set(a) | set(b):
        rot = max(rot, max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)))
        pos = max(pos, max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)))
        pos = max(pos, 10 * max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)))
    return rot, pos


# ---------------------------------------------------------------- 导出
def export() -> Path:
    rig = PoseRig(MODEL)
    poser = Poser(rig)
    author = fu_gen_xiong_clips(poser)

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
