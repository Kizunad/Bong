#!/usr/bin/env python3
"""游商傀儡（trade_puppet）GeckoLib 动画：idle / walk / attack / hurt / death / lock 六段，关键帧从零写。

只落资产、不接线：产物写到 scratchpad 式的暂存目录（默认 modelScript/out/trade_puppet/），不装进 client 资源树。
流程：贴图版 TradePuppet_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ 用 creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/trade_puppet/。

坐标与符号（模型面朝 -Z，+X 是傀儡右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 枢轴之下的肢体：rx > 0 = 往前（-Z）摆；枢轴之上的部位（脊 / 颈 / 头）：rx < 0 = 往前倾、rx > 0 = 后仰；
  - 下颌：rx < 0 = 张口；下垂的右臂 rz > 0 = 往外展，左臂相反（rz < 0 = 外展）。
  - position：-Z 方向 = 前，单位 px（1 格 = 16px）。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/trade_puppet/gen_trade_puppet_anim.py          # 出动画 + GIF
    ... gen_trade_puppet_anim.py --no-gif
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
NAME = "trade_puppet"
MODEL = CREATURE_DIR / "TradePuppet_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path(__file__).resolve().parents[2] / "out" / "review" / "anim" / NAME

GROUND_TOLERANCE = 0.6   # 任何时刻模型最低点入地不得超过它（px）
SAMPLES = 32             # 每段采样数，build_tracks 再裁掉共线帧

TAU = math.tau


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


def frac(x: float) -> float:
    return x - math.floor(x)


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

    def both(self, pose: Pose, stem: str, rot_r=(0.0, 0.0, 0.0), rot_l=None, pos=(0.0, 0.0, 0.0)) -> None:
        """左右成对的骨：rot_r 给右侧；rot_l 缺省按镜像（y / z 分量取反）。"""
        if rot_l is None:
            rot_l = (rot_r[0], -rot_r[1], -rot_r[2])
        self.turn(pose, f"{stem}_r", rot=rot_r, pos=pos)
        self.turn(pose, f"{stem}_l", rot=rot_l, pos=(-pos[0], pos[1], pos[2]))


# ---------------------------------------------------------------- 各段动画
HANGERS = ("bundle_a", "bundle_b", "bundle_c", "rope_coil", "coin_string")   # 货箱外挂的东西


def hanging_sway(poser: Poser, pose: Pose, t: float, amp: float, cycles: float, lag: float = 0.0) -> None:
    """布包 / 麻绳 / 骨币串 / 裆布 / 头侧挂件随身体晃，每件相位错开；晃动幅度 amp（度）。"""
    for i, bone in enumerate(HANGERS):
        poser.turn(pose, bone, rot=(0.6 * amp * wave(t, cycles, lag + 0.13 * i), 0, amp * wave(t, cycles, lag + 0.21 * i)))
    poser.turn(pose, "cloth_front", rot=(amp * 1.4 * wave(t, cycles, lag + 0.1), 0, 0))
    poser.turn(pose, "cloth_back", rot=(-amp * 1.2 * wave(t, cycles, lag + 0.35), 0, 0))
    poser.both(pose, "charm", rot_r=(0.5 * amp * wave(t, cycles, lag + 0.3), 0, amp * wave(t, cycles, lag + 0.45)),
               rot_l=(0.5 * amp * wave(t, cycles, lag + 0.55), 0, -amp * wave(t, cycles, lag + 0.7)))


def trade_puppet_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 站着微微起伏：胸廓呼吸、头缓缓左右扫、下颌偶尔开合一下、阵盘小幅摆动并轻轻胀缩
        pose = Pose()
        breath = wave(t, 1)
        poser.turn(pose, "spine_lower", rot=(0.8 * breath, 0, 0))
        poser.turn(pose, "spine_upper", rot=(-1.4 * breath, 1.5 * wave(t, 1, 0.25), 0.6 * wave(t, 1, 0.5)))
        poser.turn(pose, "neck", rot=(0.8 * breath, 2.5 * wave(t, 1, 0.3), 0))
        poser.turn(pose, "head", rot=(1.0 * wave(t, 1, 0.15), 6.0 * wave(t, 1, 0.35), 1.2 * wave(t, 1, 0.6)))
        gape = math.sin(math.pi * smooth((t - 0.55) / 0.12)) if 0.55 < t < 0.67 else 0.0
        poser.turn(pose, "jaw", rot=(-9 * gape - 1.5 * (1 - breath) / 2, 0, 0))
        poser.turn(pose, "array_disc", rot=(0, 0, 5 * wave(t, 1, 0.1)), scale=[1 + 0.03 * wave(t, 2), 1 + 0.03 * wave(t, 2), 1])
        for stem, sign in (("upper_arm", 1), ("forearm", 1), ("hand", 1)):
            amount = {"upper_arm": 2.2, "forearm": 2.5, "hand": 3.0}[stem]
            poser.both(pose, stem, rot_r=(amount * wave(t, 1, 0.5), 0, amount * 0.7 * wave(t, 1, 0.2)),
                       rot_l=(amount * wave(t, 1, 0.5), 0, -amount * 0.7 * wave(t, 1, 0.2)))
        poser.both(pose, "claws", rot_r=(5 * wave(t, 1, 0.4), 0, 0), rot_l=(5 * wave(t, 1, 0.65), 0, 0))
        poser.both(pose, "thumb", rot_r=(4 * wave(t, 1, 0.5), 0, 0), rot_l=(4 * wave(t, 1, 0.75), 0, 0))
        poser.turn(pose, "crate", rot=(0.7 * wave(t, 1, 0.5), 0, 0.5 * wave(t, 1, 0.1)))
        for i, bone in enumerate(("drawer_top", "drawer_mid", "drawer_bot")):
            poser.turn(pose, bone, pos=(0, 0, 0.25 * max(0.0, wave(t, 1, 0.2 + 0.12 * i))))
        hanging_sway(poser, pose, t, amp=2.2, cycles=1, lag=0.1)
        poser.turn(pose, "hips", pos=(0, 0.15 * wave(t, 1), 0))
        return pose

    def walk(t: float) -> Pose:
        # 瘦高骨架的大步：腿摆动相屈膝抬脚，对侧臂反摆，躯干扭转，货箱和挂件滞后晃，整体贴地起伏
        pose = Pose()
        for side, phase in (("l", 0.0), ("r", 0.5)):
            u = frac(t + phase)
            thigh = 24.0 * math.sin(TAU * u)
            swing = max(0.0, math.cos(TAU * u))          # 腿正往前摆的那段
            shin = -50.0 * swing - 4.0
            foot = -0.45 * (thigh + shin) + 6.0 * max(0.0, -math.cos(TAU * u))
            poser.turn(pose, f"leg_thigh_{side}", rot=(thigh, 0, 0))
            poser.turn(pose, f"leg_shin_{side}", rot=(shin, 0, 0))
            poser.turn(pose, f"foot_{side}", rot=(foot, 0, 0))
            poser.turn(pose, f"toes_{side}", rot=(-16.0 * max(0.0, -math.cos(TAU * u)) + 8.0 * swing, 0, 0))
            arm_phase = frac(t + phase + 0.5)          # 对侧臂：与同侧腿反相
            forward = math.sin(TAU * arm_phase)
            outward = 1.0 if side == "r" else -1.0
            poser.turn(pose, f"upper_arm_{side}", rot=(-15.0 * math.sin(TAU * u), 0, outward * 2.5))
            poser.turn(pose, f"forearm_{side}", rot=(10.0 + 9.0 * max(0.0, forward), 0, 0))
            poser.turn(pose, f"hand_{side}", rot=(5.0 * forward, 0, 0))
            poser.turn(pose, f"claws_{side}", rot=(10.0 + 6.0 * forward, 0, 0))
        twist = wave(t, 1)
        poser.turn(pose, "hips", rot=(0, -5.0 * twist, 1.5 * wave(t, 2, 0.25)))
        poser.turn(pose, "spine_lower", rot=(0, 3.0 * twist, 1.2 * wave(t, 1, 0.25)))
        poser.turn(pose, "spine_upper", rot=(-3.0 + 1.0 * wave(t, 2, 0.25), 4.0 * twist, 1.5 * wave(t, 1, 0.5)))
        poser.turn(pose, "neck", rot=(2.0, -3.0 * twist, 0))
        poser.turn(pose, "head", rot=(-2.0 + 1.5 * wave(t, 2, 0.4), -3.0 * twist, 1.2 * wave(t, 1, 0.25)))
        poser.turn(pose, "jaw", rot=(-3.0 - 3.0 * max(0.0, wave(t, 2, 0.3)), 0, 0))
        poser.turn(pose, "array_disc", rot=(0, 0, 4 * wave(t, 1, 0.1)))
        poser.turn(pose, "crate", rot=(1.0 * wave(t, 2, 0.15), 0, 2.0 * wave(t, 1, 0.5)))
        for i, bone in enumerate(("drawer_top", "drawer_mid", "drawer_bot")):
            poser.turn(pose, bone, pos=(0, 0.2 * wave(t, 2, 0.3 + 0.1 * i), 0))
        hanging_sway(poser, pose, t, amp=7.0, cycles=2, lag=0.2)
        return pose

    def attack(t: float) -> Pose:
        # 蓄力（后仰、右臂后拉、下颌张开）→ 前扑劈抓（右臂自上而下扫、躯干前压、整体前冲）→ 回左爪补一下 → 收势
        pose = Pose()
        wind = keys(t, [(0, 0), (0.32, 1), (0.42, 1), (0.6, 0), (1, 0)])      # 蓄力量
        lunge = keys(t, [(0, 0), (0.32, 0), (0.46, 1), (0.66, 0.85), (1, 0)])   # 前冲量
        follow = keys(t, [(0, 0), (0.5, 0), (0.62, 1), (0.8, 0.4), (1, 0)])     # 左爪补抓
        poser.turn(pose, "root", pos=(0, 0, -3.5 * lunge))
        poser.turn(pose, "hips", rot=(0, -10 * lunge, 0), pos=(0, -1.0 * wind - 0.6 * lunge, 0))
        poser.turn(pose, "spine_lower", rot=(5 * wind - 9 * lunge, 8 * lunge, 0))
        poser.turn(pose, "spine_upper", rot=(8 * wind - 16 * lunge, 12 * lunge - 6 * wind, 0))
        poser.turn(pose, "neck", rot=(3 * wind - 8 * lunge, 0, 0))
        poser.turn(pose, "head", rot=(5 * wind - 10 * lunge, 0, 0))
        poser.turn(pose, "jaw", rot=(-28 * keys(t, [(0, 0), (0.3, 0.7), (0.5, 1), (0.62, 0.15), (0.7, 0), (1, 0)]), 0, 0))
        # 右臂：后拉 → 前下扫
        swing_r = keys(t, [(0, 0), (0.32, -55), (0.42, -55), (0.55, 95), (0.7, 70), (1, 0)])
        poser.turn(pose, "shoulder_r", rot=(0, 0, 0))
        poser.turn(pose, "upper_arm_r", rot=(swing_r, 0, 6 * wind))
        poser.turn(pose, "forearm_r", rot=(keys(t, [(0, 0), (0.32, 35), (0.42, 35), (0.55, 10), (1, 0)]), 0, 0))
        poser.turn(pose, "hand_r", rot=(keys(t, [(0, 0), (0.32, -15), (0.55, 15), (1, 0)]), 0, 0))
        poser.turn(pose, "claws_r", rot=(keys(t, [(0, 0), (0.32, -25), (0.5, -35), (0.58, 25), (1, 0)]), 0, 0))
        poser.turn(pose, "thumb_r", rot=(keys(t, [(0, 0), (0.4, -20), (0.58, 20), (1, 0)]), 0, 0))
        # 左臂：垫在身侧，随后补抓
        poser.turn(pose, "upper_arm_l", rot=(-20 * wind + 100 * follow, 0, 0))
        poser.turn(pose, "forearm_l", rot=(25 * follow + 12 * wind, 0, 0))
        poser.turn(pose, "claws_l", rot=(-20 * follow + 8 * wind, 0, 0))
        # 腿：前脚蹬、后脚撑
        poser.turn(pose, "leg_thigh_r", rot=(-8 * wind + 20 * lunge, 0, 0))
        poser.turn(pose, "leg_shin_r", rot=(-10 * lunge - 6 * wind, 0, 0))
        poser.turn(pose, "foot_r", rot=(8 * lunge, 0, 0))
        poser.turn(pose, "leg_thigh_l", rot=(10 * wind - 14 * lunge, 0, 0))
        poser.turn(pose, "leg_shin_l", rot=(-8 * wind - 10 * lunge, 0, 0))
        poser.turn(pose, "foot_l", rot=(-6 * lunge, 0, 0))
        poser.turn(pose, "array_disc", scale=[1 + 0.08 * lunge, 1 + 0.08 * lunge, 1])
        poser.turn(pose, "crate", rot=(6 * wind - 8 * lunge, 0, 0))
        for i, bone in enumerate(HANGERS):
            poser.turn(pose, bone, rot=(10 * lunge - 6 * wind, 0, 8 * math.sin(math.pi * lunge + i)))
        poser.turn(pose, "cloth_front", rot=(-20 * lunge, 0, 0))
        return pose

    def hurt(t: float) -> Pose:
        # 被打：整体后仰缩一下，下颌张开，双臂外抖，货箱和挂件甩动，阵盘闪一下
        pose = Pose()
        hit = math.sin(math.pi * min(1.0, t * 1.15)) * (1 - 0.3 * t)
        rattle = math.sin(TAU * 3 * t) * (1 - t)
        poser.turn(pose, "root", pos=(0, 0, 1.4 * hit))
        poser.turn(pose, "hips", rot=(-4 * hit, 3 * rattle, 0))
        poser.turn(pose, "spine_lower", rot=(5 * hit, 0, 2 * rattle))
        poser.turn(pose, "spine_upper", rot=(10 * hit, -6 * hit, 3 * rattle))
        poser.turn(pose, "neck", rot=(6 * hit, 5 * rattle, 0))
        poser.turn(pose, "head", rot=(12 * hit, -8 * hit + 4 * rattle, 0))
        poser.turn(pose, "jaw", rot=(-20 * hit, 0, 0))
        poser.both(pose, "upper_arm", rot_r=(-10 * hit, 0, 16 * hit + 3 * rattle), rot_l=(-10 * hit, 0, -16 * hit - 3 * rattle))
        poser.both(pose, "forearm", rot_r=(14 * hit, 0, 0), rot_l=(14 * hit, 0, 0))
        poser.both(pose, "claws", rot_r=(-18 * hit, 0, 0), rot_l=(-18 * hit, 0, 0))
        poser.both(pose, "leg_thigh", rot_r=(-6 * hit, 0, 2 * hit), rot_l=(-6 * hit, 0, -2 * hit))
        poser.both(pose, "leg_shin", rot_r=(5 * hit, 0, 0), rot_l=(5 * hit, 0, 0))
        poser.turn(pose, "crate", rot=(10 * hit, 0, 5 * rattle))
        for i, bone in enumerate(HANGERS):
            poser.turn(pose, bone, rot=(8 * rattle, 0, 14 * rattle * (1 if i % 2 else -1)))
        poser.turn(pose, "cloth_front", rot=(18 * hit, 0, 0))
        flash = 1 + 0.12 * math.sin(math.pi * min(1.0, t * 2.0))
        poser.turn(pose, "array_disc", scale=[flash, flash, 1])
        return pose

    def death(t: float) -> Pose:
        # 阵盘散劲：先晃，膝一软跪下（货箱压得更深），再整个人前扑倒地，手臂和挂件松垂；阵盘缩小停转
        pose = Pose()
        totter = math.sin(TAU * 2 * t) * keys(t, [(0, 0), (0.2, 1), (0.4, 0.4), (1, 0)])
        kneel = keys(t, [(0, 0), (0.22, 0.05), (0.5, 1), (1, 1)])
        fall = keys(t, [(0, 0), (0.5, 0), (0.88, 1), (1, 1)])
        settle = keys(t, [(0, 0), (0.88, 0), (0.95, 0.35), (1, 0)])     # 倒地反弹一下
        poser.turn(pose, "root", rot=(-84 * fall + 4 * settle * fall, 0, 0))
        poser.turn(pose, "hips", rot=(0, 0, 4 * totter), pos=(0, -1.0 * kneel, 0))
        poser.both(pose, "leg_thigh", rot_r=(62 * kneel + 3 * totter, 0, 4 * kneel), rot_l=(55 * kneel - 3 * totter, 0, -4 * kneel))
        poser.both(pose, "leg_shin", rot_r=(-95 * kneel, 0, 0), rot_l=(-88 * kneel, 0, 0))
        poser.both(pose, "foot", rot_r=(30 * kneel, 0, 0), rot_l=(24 * kneel, 0, 0))
        poser.both(pose, "toes", rot_r=(-10 * kneel, 0, 0), rot_l=(-6 * kneel, 0, 0))
        poser.turn(pose, "spine_lower", rot=(-18 * kneel, 0, 3 * totter))
        poser.turn(pose, "spine_upper", rot=(-20 * kneel - 6 * fall, 0, -4 * totter))
        poser.turn(pose, "neck", rot=(-14 * kneel, 4 * totter, 0))
        poser.turn(pose, "head", rot=(-12 * kneel + 8 * fall, 12 * totter, 6 * kneel))
        poser.turn(pose, "jaw", rot=(-26 * keys(t, [(0, 0), (0.3, 0.8), (1, 1)]), 0, 0))
        # 倒地后双臂顺着躯干垂向脚的一侧、略外摊，不能再往前举（前举会在趴姿里指向天空）
        poser.both(pose, "upper_arm", rot_r=(22 * kneel - 20 * fall, 0, 22 * kneel + 10 * totter + 14 * fall),
                   rot_l=(30 * kneel - 20 * fall, 0, -26 * kneel + 10 * totter - 14 * fall))
        poser.both(pose, "forearm", rot_r=(24 * kneel, 0, 0), rot_l=(32 * kneel, 0, 0))
        poser.both(pose, "hand", rot_r=(12 * kneel, 0, 0), rot_l=(18 * kneel, 0, 0))
        poser.both(pose, "claws", rot_r=(24 * kneel, 0, 0), rot_l=(30 * kneel, 0, 0))
        poser.turn(pose, "crate", rot=(-8 * kneel + 10 * fall, 0, 5 * totter))
        for i, bone in enumerate(HANGERS):
            poser.turn(pose, bone, rot=(12 * fall, 0, (14 if i % 2 else -14) * kneel))
        poser.turn(pose, "cloth_front", rot=(30 * kneel, 0, 0))
        poser.turn(pose, "cloth_back", rot=(-20 * kneel, 0, 0))
        shrink = 1 - 0.28 * keys(t, [(0, 0), (0.6, 1), (1, 1)])
        poser.turn(pose, "array_disc", rot=(0, 0, 40 * keys(t, [(0, 0), (0.55, 1), (1, 1)])), scale=[shrink, shrink, 1])
        return pose

    def lock(t: float) -> Pose:
        # 被抢时自锁：阵盘急转并胀亮，整体收拢——屈膝下蹲、脊背弓起、头埋进胸前、双臂抱胸护住货箱；末段停住不动
        pose = Pose()
        curl = keys(t, [(0, 0), (0.12, 0.1), (0.45, 1), (1, 1)])
        flare = math.sin(math.pi * min(1.0, t / 0.55)) if t < 0.55 else 0.0
        tremble = math.sin(TAU * 6 * t) * keys(t, [(0, 0), (0.45, 1), (1, 0.15)]) * (1 - curl * 0.6)
        poser.turn(pose, "hips", pos=(0, -3.2 * curl, 0.8 * curl), rot=(-6 * curl, 0, 0))
        poser.both(pose, "leg_thigh", rot_r=(48 * curl, 0, 5 * curl), rot_l=(48 * curl, 0, -5 * curl))
        poser.both(pose, "leg_shin", rot_r=(-78 * curl, 0, 0), rot_l=(-78 * curl, 0, 0))
        poser.both(pose, "foot", rot_r=(30 * curl, 0, 0), rot_l=(30 * curl, 0, 0))
        poser.turn(pose, "spine_lower", rot=(-20 * curl, 0, 0.8 * tremble))
        poser.turn(pose, "spine_upper", rot=(-26 * curl, 0, 1.2 * tremble))
        poser.turn(pose, "neck", rot=(-22 * curl, 0, 0))
        poser.turn(pose, "head", rot=(-26 * curl, 0, 1.5 * tremble))
        poser.turn(pose, "jaw", rot=(-3 * curl, 0, 0))
        # 上臂绕竖轴往内转（ry：右臂 +、左臂 −），前臂屈 108° 后就横过胸前，两臂交叠抱住自己
        poser.both(pose, "upper_arm", rot_r=(35 * curl, 42 * curl, -22 * curl), rot_l=(35 * curl, -42 * curl, 22 * curl))
        poser.both(pose, "forearm", rot_r=(108 * curl, 0, 0), rot_l=(108 * curl, 0, 0))
        poser.both(pose, "hand", rot_r=(8 * curl, 0, 0), rot_l=(8 * curl, 0, 0))
        poser.both(pose, "claws", rot_r=(40 * curl, 0, 0), rot_l=(40 * curl, 0, 0))
        poser.both(pose, "thumb", rot_r=(30 * curl, 0, 0), rot_l=(30 * curl, 0, 0))
        poser.turn(pose, "crate", rot=(-10 * curl, 0, 0), pos=(0, -0.4 * curl, 0))
        for i, bone in enumerate(HANGERS):
            poser.turn(pose, bone, rot=(-6 * curl, 0, 4 * math.sin(TAU * 4 * t + i) * (1 - curl)))
        poser.turn(pose, "cloth_front", rot=(24 * curl, 0, 0))
        # 阵盘：急转 + 胀大一圈再回落到比平时略大，最终停在亮着的状态
        size = 1 + 0.34 * flare + 0.12 * curl
        poser.turn(pose, "array_disc", rot=(0, 0, -270 * smooth(t / 0.6) + 12 * math.sin(TAU * 4 * t) * (1 - curl)),
                   scale=[size, size, 1])
        return pose

    return {
        "idle": (4.0, True, idle), "walk": (1.2, True, walk), "attack": (1.0, False, attack),
        "hurt": (0.5, False, hurt), "death": (2.0, False, death), "lock": (1.6, False, lock),
    }


# ---------------------------------------------------------------- 地面约束
def grounded(rig: PoseRig, poser: Poser, sampler, pin: bool):
    """pin=True：整体贴地（行走时最低的脚始终踩在地上）；pin=False：只在入地时把整体托起来。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if pin or depth < 0.0:
            poser.turn(pose, "root", pos=(0.0, -depth, 0.0))
        return pose

    return sample


def check_ground(clip: str, rig: PoseRig, sampler) -> float:
    lowest, when = math.inf, 0.0
    for step in range(SAMPLES + 1):
        t = step / SAMPLES
        height = rig.lowest(sampler(t))
        if height < lowest:
            lowest, when = height, t
    if lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{NAME}.{clip}: t01={when:.2f} 时最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    return lowest


def loop_gap(sampler) -> tuple[float, float]:
    """循环段首尾姿态的最大差（旋转度数、位移 px）。"""
    a, b = sampler(0.0), sampler(1.0)
    rot = pos = 0.0
    for bone in set(a) | set(b):
        rot = max(rot, max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)))
        pos = max(pos, max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)))
    return rot, pos


# ---------------------------------------------------------------- 导出
def export() -> Path:
    rig = PoseRig(MODEL)
    poser = Poser(rig)
    author = trade_puppet_clips(poser)

    animations = []
    for clip, (length, loop, raw) in author.items():
        sampler = grounded(rig, poser, raw, pin=(clip == "walk"))
        lowest = check_ground(clip, rig, sampler)
        line = f"  {clip}: 最低点 {lowest:+.2f}px"
        if loop:
            rot, pos = loop_gap(sampler)
            if rot > 0.5 or pos > 0.1:
                raise ValueError(f"{NAME}.{clip}: 循环首尾不同值（旋转差 {rot:.2f}° 位移差 {pos:.2f}px）")
            line += f"，首尾差 {rot:.2f}° / {pos:.2f}px"
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
