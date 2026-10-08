#!/usr/bin/env python3
"""v2 重做生物的动画：程序化采样 → 绑定稿 bbmodel → 离线 codec → client animation.json。

只落资产，不接线：写出 ``client/.../animations/<name>.animation.json``（名字都带 ``_v2``），
不改 geo、不改任何「哪个实体播哪段动画」的代码。

骨架统一来自 ``gen_rig.py``：枢轴都在真关节上，动画直接绕关节转，没有位移补偿。

坐标约定：采样函数一律按**建模稿的朝向**书写 —— 正面在 +Z，``leg_l_*`` / ``arm_l`` 在 -X，
rotation.x 为正 = 向前低头 / 前倾，作用在下垂的小腿上 = 小腿向后折。
``gen_rig`` 导出时把几何绕 Y 转了 180°（游戏约定面朝 -Z），``Poser`` 在写入关键帧前做同样的
共轭：rotation (x, y, z) → (-x, y, -z)，position (x, y, z) → (-x, y, -z)。
所以读采样函数时照建模稿想象就行，不用在脑子里再转一次。

用法::

    python3 modelScript/creatures/fauna_v2/gen_anim.py              # 四种都出并安装
    python3 modelScript/creatures/fauna_v2/gen_anim.py --only fuya_v2
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import subprocess
from pathlib import Path
from typing import Callable

from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, write_animated_bbmodel

import gen_rig

ROOT = Path(__file__).resolve().parents[3]
CODEC = ROOT / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = ROOT / "modelScript" / "out" / "fauna_v2"
CLIENT = ROOT / "client" / "src" / "main" / "resources" / "assets" / "bong"
SAMPLES = 24  # 每段动画的采样数；关键帧再由 build_tracks 按像素容差裁掉共线点

Sampler = Callable[[float], Pose]


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t ∈ [0,1] 上走 cycles 个整周期的正弦，循环动画首尾自然相接。"""

    return math.sin(math.tau * (cycles * t + phase))


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep 插值：points 是 (t01, 值) 的有序表，一次性动作用它排节奏。"""

    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


def frac(x: float) -> float:
    return x - math.floor(x)


# ---------------------------------------------------------------- 摆姿工具
class Poser:
    """往 Pose 里累加骨骼变换；输入按建模稿朝向写，输出是导出后（面朝 -Z）的关键帧。"""

    def __init__(self, rig: PoseRig):
        self.rig = rig

    def turn(self, pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0), scale=None) -> None:
        if bone not in self.rig.bones:
            raise KeyError(f"{self.rig.path.name}: 没有骨 {bone}")
        channel = pose[bone]
        channel.rot = [channel.rot[0] - rot[0], channel.rot[1] + rot[1], channel.rot[2] - rot[2]]
        channel.pos = [channel.pos[0] - pos[0], channel.pos[1] + pos[1], channel.pos[2] - pos[2]]
        if scale is not None:
            channel.scale = [float(v) for v in scale]


# ================================================================ 灰烬蛛 v2
# 每条腿两节：股节绕腿根转（前后扫 + 抬起），胫节绕膝结折（收进身下）。
# 交替四足步态：l0 r1 l2 r3 一组，r0 l1 r2 l3 另一组，两组相位差半周期。
SPIDER_GROUP_A = ("leg_l_0", "leg_r_1", "leg_l_2", "leg_r_3")
# 死态身体下沉量：腹囊底面（建模稿 y = 3.2）落到地面附近。
# 由 check_ground() 的最低点报告反推，改动作幅度后要重新核一遍。
SPIDER_DEATH_DROP = -2.4
SPIDER_LEGS = tuple(f"leg_{side}_{i}" for side in "lr" for i in range(4))


def spider_leg(poser: Poser, pose: Pose, leg: str, sweep: float, lift: float, fold: float = 0.0) -> None:
    """sweep > 0 = 腿尖往前，lift > 0 = 腿尖抬起，fold > 0 = 胫节往身下收。左右符号在这里统一。"""

    side = -1.0 if leg.startswith("leg_l") else 1.0  # 建模稿里 leg_l 在 -X
    poser.turn(pose, leg, rot=(0.0, side * sweep, side * lift))
    poser.turn(pose, "shin" + leg[3:], rot=(0.0, 0.0, -side * fold))


def spider_gait(poser: Poser, pose: Pose, t: float, sweep: float, lift: float) -> None:
    for leg in SPIDER_LEGS:
        phase = 0.0 if leg in SPIDER_GROUP_A else 0.5
        u = frac(t + phase)
        if u < 0.5:  # 摆动相：抬腿、收胫、往前送
            s = u / 0.5
            arc = math.sin(math.pi * s)
            spider_leg(poser, pose, leg, sweep * (2 * smooth(s) - 1), lift * arc, 0.8 * lift * arc)
        else:  # 支撑相：贴地往后划
            s = (u - 0.5) / 0.5
            spider_leg(poser, pose, leg, sweep * (1 - 2 * s), 0.0)


def ash_spider_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def idle(t: float) -> Pose:
        pose = Pose()
        poser.turn(pose, "body", pos=(0, 0.15 * wave(t, 2), 0))
        poser.turn(pose, "abdomen", rot=(1.5 * wave(t, 2, 0.1), 0, 0), scale=[1 + 0.03 * wave(t, 2, 0.1)] * 3)
        poser.turn(pose, "palp_l", rot=(6 * wave(t, 7), 0, 0))
        poser.turn(pose, "palp_r", rot=(6 * wave(t, 7, 0.5), 0, 0))
        # 螯肢在周期中段无征兆地开合一次
        snap = math.sin(math.pi * smooth((t - 0.55) / 0.12)) if 0.55 < t < 0.67 else 0.0
        poser.turn(pose, "chelicerae", rot=(-14 * snap, 0, 0))
        for index, leg in enumerate(SPIDER_LEGS):  # 各腿错开相位小幅挪脚
            twitch = math.sin(math.pi * frac(t * 2 + index * 0.13)) ** 8
            spider_leg(poser, pose, leg, 2 * twitch, 6 * twitch, 4 * twitch)
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        spider_gait(poser, pose, t, sweep=14, lift=14)
        poser.turn(pose, "body", rot=(0, 2 * wave(t, 1), 0), pos=(0, 0.3 * abs(wave(t, 2)), 0))
        poser.turn(pose, "palp_l", rot=(8 * wave(t, 2), 0, 0))
        poser.turn(pose, "palp_r", rot=(-8 * wave(t, 2), 0, 0))
        return pose

    def run(t: float) -> Pose:
        pose = Pose()
        spider_gait(poser, pose, t, sweep=20, lift=20)
        poser.turn(pose, "body", rot=(5, 3 * wave(t, 1), 0), pos=(0, -0.8 + 0.4 * abs(wave(t, 2)), 0))
        poser.turn(pose, "palp_l", rot=(-18, 0, 0))
        poser.turn(pose, "palp_r", rot=(-18, 0, 0))
        return pose

    def bite(t: float) -> Pose:
        pose = Pose()
        rear = keys(t, [(0, 0), (0.45, -16), (0.6, 12), (0.75, 10), (1, 0)])
        lunge = keys(t, [(0, 0), (0.45, -1.2), (0.6, 3.0), (0.75, 2.6), (1, 0)])
        rise = keys(t, [(0, 0), (0.45, 1.2), (0.6, 0), (1, 0)])
        poser.turn(pose, "body", rot=(rear, 0, 0), pos=(0, rise, lunge))
        poser.turn(pose, "chelicerae", rot=(keys(t, [(0, 0), (0.45, -30), (0.6, 8), (0.75, 0), (1, 0)]), 0, 0))
        palp = keys(t, [(0, 0), (0.45, -35), (0.6, -10), (1, 0)])
        poser.turn(pose, "palp_l", rot=(palp, 0, 0))
        poser.turn(pose, "palp_r", rot=(palp, 0, 0))
        for leg in SPIDER_LEGS:  # 前两对腿抬起亮爪，后两对腿压低撑地
            front = leg.endswith(("_0", "_1"))
            raised = keys(t, [(0, 0), (0.45, 22 if front else -4), (0.6, 4), (1, 0)])
            spider_leg(poser, pose, leg, 0.0, raised, raised * 0.6 if front else 0.0)
        return pose

    def ambush_burst(t: float) -> Pose:
        pose = Pose()
        crouch = keys(t, [(0, 0), (0.25, -1.6), (0.55, 2.4), (0.8, 1.6), (1, 1.2)])
        spread = keys(t, [(0, 0), (0.25, -10), (0.55, 30), (0.8, 18), (1, 14)])
        poser.turn(pose, "body", rot=(keys(t, [(0, 0), (0.25, 4), (0.55, -18), (1, -10)]), 0, 0),
                   pos=(0, crouch, keys(t, [(0, 0), (0.55, 1.5), (1, 1.0)])))
        poser.turn(pose, "chelicerae", rot=(keys(t, [(0, 0), (0.55, -30), (1, -20)]), 0, 0))
        for leg in SPIDER_LEGS:
            front = leg.endswith(("_0", "_1"))
            spider_leg(poser, pose, leg, 6 if front else -6, spread if front else spread * 0.4)
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "body", rot=(-8 * hit, 0, 5 * math.sin(math.tau * t)), pos=(0, 0.4 * hit, -1.2 * hit))
        for leg in SPIDER_LEGS:
            spider_leg(poser, pose, leg, 0.0, 10 * hit, 14 * hit)
        return pose

    def death(t: float) -> Pose:
        # 先挺一下，然后腹面贴地，八腿向上向内收成笼（真实蜘蛛死态），前 0.7 秒两次递减抽搐
        pose = Pose()
        twitch = 6 * math.sin(math.tau * 6 * t) * max(0.0, 1 - t / 0.7)
        poser.turn(pose, "body", rot=(keys(t, [(0, 0), (0.25, -10), (0.6, 4)]), 0, keys(t, [(0, 0), (0.6, 6)])),
                   pos=(0, keys(t, [(0, 0), (0.25, 1.0), (0.6, SPIDER_DEATH_DROP)]), 0))
        curl = keys(t, [(0, 0), (0.25, 16), (0.7, 42), (1, 48)])
        fold = keys(t, [(0, 0), (0.25, 10), (0.7, 40), (1, 46)])
        for leg in SPIDER_LEGS:
            front = leg.endswith(("_0", "_1"))
            spider_leg(poser, pose, leg, keys(t, [(0.25, 0), (1, -12 if front else 12)]), curl + twitch, fold)
        poser.turn(pose, "chelicerae", rot=(keys(t, [(0, 0), (0.6, -18)]), 0, 0))
        return pose

    return {
        "idle": (4.0, True, idle), "walk": (1.0, True, walk), "run": (0.5, True, run),
        "bite": (0.6, False, bite), "ambush_burst": (0.3, False, ambush_burst),
        "hurt": (0.4, False, hurt), "death": (1.8, False, death),
    }


# ================================================================ 骨煞 v2
# 漂浮主颅 + 五颗外挂小颅 + 肋笼 + 祭布 + 三挂铁链。没有腿，「走」就是前倾滑行，
# 祭布和铁链拖在身后；辨识度来自肋笼开合（ribcage 横向缩放）、小颅绕身环行和眼火明灭。
SKULL_SATELLITES = {  # 骨名 → 相位（五颗各自错开）
    "skull_upper_l": 0.0, "skull_upper_r": 0.5, "skull_lower_l": 0.25, "skull_lower_r": 0.75,
    "skull_front": 0.4,
}
SKULL_CLOTH = {"cloth_front": 0.2, "cloth_l": 0.35, "cloth_r": 0.1, "cloth_rear": 0.45}
SKULL_CHAINS = {"chain_l": 0.3, "chain_r": 0.6, "chain_rear": 0.0}
SKULL_DEATH_DROP = -3.0  # 死态下沉量，由 check_ground() 反推：最低处落到地面


def skull_fiend_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def drift(pose: Pose, t: float, bob: float, lean: float, cycles: int = 1) -> None:
        poser.turn(pose, "body", rot=(lean + 2 * wave(t, cycles, 0.25), 0, 1.5 * wave(t, cycles)),
                   pos=(0, bob * wave(t, cycles), 0))

    def trailing(pose: Pose, t: float, cycles: int, swing: float, drag: float) -> None:
        """祭布和铁链：各自错相摆，drag 是整体被拖向身后的角度。"""

        for bone, phase in SKULL_CLOTH.items():
            poser.turn(pose, bone, rot=(drag + swing * wave(t, cycles, phase), 0, 0.6 * swing * wave(t, cycles, phase + 0.2)))
        for bone, phase in SKULL_CHAINS.items():
            poser.turn(pose, bone, rot=(drag + swing * wave(t, cycles, phase), 0, 0.8 * swing * wave(t, cycles, phase + 0.3)))

    def idle(t: float) -> Pose:
        pose = Pose()
        drift(pose, t, bob=1.0, lean=0.0)
        poser.turn(pose, "head", rot=(0, 6 * wave(t, 1, 0.1), 0))
        chatter = sum(math.sin(math.pi * smooth((t - start) / 0.08)) for start in (0.30, 0.40)
                      if start < t < start + 0.08)
        poser.turn(pose, "jaw", rot=(10 * chatter, 0, 0))
        poser.turn(pose, "ribcage", scale=(1 + 0.08 * wave(t, 2), 1, 1 + 0.04 * wave(t, 2)))
        for bone, phase in SKULL_SATELLITES.items():  # 绕身缓慢环行 + 各自浮沉
            poser.turn(pose, bone, rot=(0, 8 * wave(t, 1, phase), 0), pos=(0, 0.6 * wave(t, 2, phase), 0))
        trailing(pose, t, cycles=1, swing=4, drag=0)
        poser.turn(pose, "eye_fire", scale=[1 + 0.18 * wave(t, 3)] * 3)
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        drift(pose, t, bob=0.6, lean=12.0)
        poser.turn(pose, "head", rot=(-6, 3 * wave(t, 1), 0))
        poser.turn(pose, "ribcage", scale=(1 + 0.06 * wave(t, 2), 1, 1))
        for bone, phase in SKULL_SATELLITES.items():
            poser.turn(pose, bone, rot=(-6, 5 * wave(t, 1, phase), 0))
        trailing(pose, t, cycles=2, swing=5, drag=-14)
        poser.turn(pose, "eye_fire", scale=[1 + 0.12 * wave(t, 2)] * 3)
        return pose

    def attack(t: float) -> Pose:
        pose = Pose()
        lean = keys(t, [(0, 0), (0.4, -16), (0.55, 14), (0.7, 12), (1, 0)])
        push = keys(t, [(0, 0), (0.4, -1.5), (0.55, 4.0), (0.7, 3.4), (1, 0)])
        rise = keys(t, [(0, 0), (0.4, 1.2), (0.55, 0), (1, 0)])
        ribs = keys(t, [(0, 1), (0.4, 1.3), (0.55, 0.9), (1, 1)])
        poser.turn(pose, "body", rot=(lean, 0, 0), pos=(0, rise, push))
        poser.turn(pose, "jaw", rot=(keys(t, [(0, 0), (0.4, 28), (0.55, 0), (0.62, 4), (1, 0)]), 0, 0))
        poser.turn(pose, "ribcage", scale=(ribs, 1, 1 + (ribs - 1) * 0.5))
        spread = keys(t, [(0, 0), (0.4, 14), (0.55, -6), (1, 0)])  # 蓄势时小颅向外张开
        for bone in SKULL_SATELLITES:
            side = -1.0 if bone.endswith("_l") else 1.0 if bone.endswith("_r") else 0.0
            poser.turn(pose, bone, rot=(-lean * 0.6, 0, side * spread))
        for bone in (*SKULL_CLOTH, *SKULL_CHAINS):
            poser.turn(pose, bone, rot=(-lean * 0.8, 0, 0))
        poser.turn(pose, "eye_fire", scale=[keys(t, [(0, 1), (0.4, 1.5), (0.55, 1.8), (1, 1)])] * 3)
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "body", rot=(-10 * hit, 0, 0), pos=(0, 0, -1.2 * hit))
        poser.turn(pose, "head", rot=(0, 0, 8 * math.sin(math.tau * 1.5 * t)))
        for bone, phase in SKULL_SATELLITES.items():  # 受击时各颗小颅错相抖一下半
            poser.turn(pose, bone, rot=(0, 10 * hit * wave(t, 1.5, phase), 0))
        poser.turn(pose, "eye_fire", scale=[1 - 0.5 * hit] * 3)
        return pose

    def death(t: float) -> Pose:
        pose = Pose()
        # 坠地：整体下沉到最低点刚好碰地（下沉量见 SKULL_DEATH_DROP）
        poser.turn(pose, "body", rot=(keys(t, [(0, 0), (0.2, -8), (1, 20)]), 0, keys(t, [(0.2, 0), (1, 28)])),
                   pos=(0, keys(t, [(0, 0), (0.2, 1.0), (1, SKULL_DEATH_DROP)]), 0))
        poser.turn(pose, "jaw", rot=(keys(t, [(0, 0), (0.3, 30), (1, 34)]), 0, 0))
        droop = keys(t, [(0.2, 0), (1, 1.0)])
        for bone, phase in SKULL_SATELLITES.items():
            side = -1.0 if bone.endswith("_l") else 1.0 if bone.endswith("_r") else 0.0
            poser.turn(pose, bone, rot=(10 * droop, 0, side * 18 * droop), pos=(0, -2.5 * droop, 0))
        poser.turn(pose, "ribcage", scale=(keys(t, [(0, 1), (0.3, 1.3), (1, 0.9)]), 1, 1))
        for bone in (*SKULL_CLOTH, *SKULL_CHAINS):
            poser.turn(pose, bone, rot=(-22 * droop, 0, 0))
        poser.turn(pose, "eye_fire", scale=[keys(t, [(0, 1), (0.2, 1.6), (0.8, 0.05)])] * 3)
        return pose

    return {
        "idle": (4.2, True, idle), "walk": (1.6, True, walk), "attack": (0.9, False, attack),
        "hurt": (0.4, False, hurt), "death": (1.6, False, death),
    }


# ================================================================ 道伥 v2
# 驼背骷髅，一条腿拖着走（右腿步幅减半、小腿不伸直）；破布各自一根骨，按滞后相位摆。
DAOXIANG_KNEEL_DROP = -0.3
DAOXIANG_FALL_DROP = 0.0
def daoxiang_rags(poser: Poser, pose: Pose, t: float, cycles: float, amount: float) -> None:
    poser.turn(pose, "skirt_front", rot=(amount * wave(t, cycles, 0.2), 0, 0))
    poser.turn(pose, "skirt_l", rot=(0, 0, -amount * 0.6 * wave(t, cycles, 0.35)))
    poser.turn(pose, "skirt_r", rot=(0, 0, amount * 0.6 * wave(t, cycles, 0.1)))
    poser.turn(pose, "robe_back", rot=(-amount * wave(t, cycles, 0.3), 0, 0))
    poser.turn(pose, "cape_l", rot=(0, 0, -amount * 0.5 * wave(t, cycles, 0.25)))
    poser.turn(pose, "cape_r", rot=(0, 0, amount * 0.5 * wave(t, cycles, 0.45)))
    poser.turn(pose, "hood_tail_l", rot=(amount * 0.6 * wave(t, cycles, 0.4), 0, 0))
    poser.turn(pose, "hood_tail_r", rot=(amount * 0.6 * wave(t, cycles, 0.55), 0, 0))
    poser.turn(pose, "hood_nape", rot=(-amount * 0.5 * wave(t, cycles, 0.35), 0, 0))


def daoxiang_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def idle(t: float) -> Pose:
        pose = Pose()
        poser.turn(pose, "torso", rot=(6 + 2.5 * wave(t, 1), 0, 0))
        twitch = math.sin(math.pi * smooth((t - 0.6) / 0.06)) if 0.6 < t < 0.66 else 0.0
        poser.turn(pose, "head", rot=(4 * wave(t, 1, 0.1), 0, 14 * twitch))
        poser.turn(pose, "jaw", rot=(5 + 3 * wave(t, 1, 0.2), 0, 0))
        for side, phase in (("l", 0.0), ("r", 0.35)):
            poser.turn(pose, f"arm_{side}", rot=(3 * wave(t, 1, phase), 0, 0))
            poser.turn(pose, f"forearm_{side}", rot=(-4 * wave(t, 1, phase + 0.15), 0, 0))
        daoxiang_rags(poser, pose, t, cycles=1, amount=4)
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        step = wave(t, 1)  # >0 左腿在前
        poser.turn(pose, "hips", rot=(0, 5 * step, 3 * wave(t, 1, 0.25)),
                   pos=(0, -0.4 * abs(wave(t, 1, 0.25)), 0))
        poser.turn(pose, "torso", rot=(14, -4 * step, 0))
        poser.turn(pose, "head", rot=(-8 + 3 * wave(t, 2), 3 * step, 0))
        # 左腿正常迈步，右腿拖行：步幅减半、小腿始终弯着
        poser.turn(pose, "leg_l", rot=(-22 * step, 0, 0))
        poser.turn(pose, "shin_l", rot=(26 * max(0.0, -wave(t, 1, 0.2)), 0, 0))
        poser.turn(pose, "leg_r", rot=(11 * step, 0, 0))
        poser.turn(pose, "shin_r", rot=(18 + 6 * wave(t, 1, 0.1), 0, 0))
        for side, sign in (("l", 1.0), ("r", -1.0)):
            poser.turn(pose, f"arm_{side}", rot=(sign * 10 * step, 0, 0))
            poser.turn(pose, f"forearm_{side}", rot=(-8 * sign * wave(t, 1, 0.15), 0, 0))
        poser.turn(pose, "jaw", rot=(6, 0, 0))
        daoxiang_rags(poser, pose, t, cycles=2, amount=7)
        return pose

    def attack(t: float) -> Pose:
        pose = Pose()
        arm = keys(t, [(0, 0), (0.4, -120), (0.58, -10), (0.7, -4), (1, 0)])
        fore = keys(t, [(0, 0), (0.4, -25), (0.58, 10), (1, 0)])
        poser.turn(pose, "torso", rot=(keys(t, [(0, 0), (0.4, -10), (0.58, 22), (0.7, 20), (1, 0)]), 0, 0))
        poser.turn(pose, "hips", pos=(0, keys(t, [(0, 0), (0.58, -0.8), (1, 0)]),
                                      keys(t, [(0, 0), (0.4, -0.8), (0.58, 2.0), (1, 0)])))
        poser.turn(pose, "head", rot=(keys(t, [(0, 0), (0.4, -18), (0.58, 8), (1, 0)]), 0, 0))
        poser.turn(pose, "jaw", rot=(keys(t, [(0, 0), (0.4, 22), (0.58, 4), (1, 0)]), 0, 0))
        for side, sign in (("l", 1.0), ("r", -1.0)):
            poser.turn(pose, f"arm_{side}", rot=(arm, 0, sign * keys(t, [(0, 0), (0.4, -12), (0.58, 6), (1, 0)])))
            poser.turn(pose, f"forearm_{side}", rot=(fore, 0, 0))
        daoxiang_rags(poser, pose, t, cycles=1, amount=6)
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "torso", rot=(-12 * hit, 0, 0))
        poser.turn(pose, "head", rot=(-16 * hit, 0, 6 * hit))
        poser.turn(pose, "hips", pos=(0, 0, -1.0 * hit))
        for side, sign in (("l", 1.0), ("r", -1.0)):
            poser.turn(pose, f"arm_{side}", rot=(-20 * hit, 0, sign * -14 * hit))
        return pose

    def death(t: float) -> Pose:
        # 先跪：双膝后折、髋下沉；再向前扑倒：髋前倾、躯干贴地。
        # 两段下沉量（DAOXIANG_KNEEL_DROP / DAOXIANG_FALL_DROP）由 check_ground() 反推，
        # 保证跪姿时膝盖落地、扑倒后身体最低处贴地而不入地。
        pose = Pose()
        kneel = keys(t, [(0, 0), (0.45, 1.0), (1, 1.0)])
        fall = keys(t, [(0.35, 0), (1, 1.0)])
        poser.turn(pose, "hips", rot=(30 * fall, 0, 0),
                   pos=(0, DAOXIANG_KNEEL_DROP * kneel + DAOXIANG_FALL_DROP * fall, 2.5 * fall))
        poser.turn(pose, "leg_l", rot=(-38 * kneel - 20 * fall, 0, 0))
        poser.turn(pose, "leg_r", rot=(-30 * kneel - 20 * fall, 0, 0))
        poser.turn(pose, "shin_l", rot=(80 * kneel, 0, 0))
        poser.turn(pose, "shin_r", rot=(75 * kneel, 0, 0))
        poser.turn(pose, "torso", rot=(10 * kneel + 50 * fall, 0, 0))
        poser.turn(pose, "head", rot=(20 * fall, 0, keys(t, [(0.2, 0), (0.5, 12), (1, 18)])))
        poser.turn(pose, "jaw", rot=(keys(t, [(0, 0), (0.5, 24)]), 0, 0))
        for side in ("l", "r"):
            poser.turn(pose, f"arm_{side}", rot=(-25 * fall, 0, 0))
            poser.turn(pose, f"forearm_{side}", rot=(12 * fall, 0, 0))
        return pose

    return {
        "idle": (3.2, True, idle), "walk": (1.4, True, walk), "attack": (0.9, False, attack),
        "hurt": (0.4, False, hurt), "death": (1.8, False, death),
    }


# ================================================================ 负压畸变体 v2
# 四足重甲兽：步序 左后 → 左前 → 右后 → 右前，每条腿只在四分之一周期里离地，
# 落脚那一刻身体下顿一下；背核一直在脉动，攻击时猛亮一下。
FUYA_LEGS = {"fl": 0.25, "bl": 0.0, "fr": 0.75, "br": 0.5}
# 死态下沉量：躯干和四腿各自落到贴地，由 check_ground() 反推
FUYA_DEATH_DROP = -1.2
FUYA_LEG_DROP = -1.0


def fuya_step(poser: Poser, pose: Pose, t: float, stride: float, lift: float) -> float:
    """四条腿走一个周期，返回本帧的「落脚下顿」量（0..1）给身体用。"""

    thud = 0.0
    for leg, phase in FUYA_LEGS.items():
        u = frac(t + phase)
        if u < 0.25:  # 抬腿前送
            s = u / 0.25
            sweep = -stride * (2 * smooth(s) - 1)
            raise_y = lift * math.sin(math.pi * s)
            knee = 20 * math.sin(math.pi * s)
        else:  # 承重后划
            s = (u - 0.25) / 0.75
            sweep = -stride * (1 - 2 * s)
            raise_y = 0.0
            knee = 0.0
            thud = max(thud, max(0.0, 1 - s / 0.15))
        poser.turn(pose, f"leg_{leg}", rot=(sweep, 0, 0), pos=(0, raise_y, 0))
        poser.turn(pose, f"foot_{leg}", rot=(knee, 0, 0))
    return thud


def fuya_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def idle(t: float) -> Pose:
        pose = Pose()
        breath = wave(t, 1)
        poser.turn(pose, "body", rot=(1.2 * breath, 0, 0), pos=(0, 0.3 * breath, 0))
        poser.turn(pose, "head", rot=(2 * wave(t, 1, 0.2), 3 * wave(t, 1, 0.35), 0))
        poser.turn(pose, "jaw", rot=(3 + 3 * wave(t, 1, 0.25), 0, 0))
        pulse = 0.5 * (wave(t, 2) + 1)
        poser.turn(pose, "core", scale=[1 + 0.09 * pulse ** 3] * 3)
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        thud = fuya_step(poser, pose, t, stride=14, lift=1.8)
        poser.turn(pose, "body", rot=(0, 3 * wave(t, 1), 3 * wave(t, 1, 0.25)), pos=(0, -0.6 * thud, 0))
        poser.turn(pose, "head", rot=(4 * thud - 2, -3 * wave(t, 1), 0))
        poser.turn(pose, "jaw", rot=(4 + 2 * thud, 0, 0))
        poser.turn(pose, "core", scale=[1 + 0.06 * thud] * 3)
        return pose

    def attack(t: float) -> Pose:
        pose = Pose()
        # 冲撞时头往前砸但不往下砸：头和下颌离地只有 1.6px，前倾幅度大了下颌就会啃进地里
        rear = keys(t, [(0, 0), (0.45, -8), (0.6, 6), (0.75, 5), (1, 0)])
        poser.turn(pose, "body", rot=(rear, 0, 0),
                   pos=(0, keys(t, [(0, 0), (0.45, 1.0), (0.6, 0.0), (1, 0)]),
                        keys(t, [(0, 0), (0.45, -1.0), (0.6, 3.0), (0.75, 2.6), (1, 0)])))
        poser.turn(pose, "head", rot=(keys(t, [(0, 0), (0.45, -16), (0.6, 8), (0.75, 6), (1, 0)]), 0, 0))
        poser.turn(pose, "jaw", rot=(keys(t, [(0, 0), (0.45, 32), (0.6, 0), (0.7, 6), (1, 0)]), 0, 0))
        poser.turn(pose, "core", scale=[keys(t, [(0, 1), (0.45, 0.85), (0.6, 1.3), (1, 1)])] * 3)
        for leg in ("fl", "fr"):  # 前腿撑地顶住冲撞
            poser.turn(pose, f"leg_{leg}", rot=(keys(t, [(0, 0), (0.45, -8), (0.6, 12), (1, 0)]), 0, 0))
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "body", rot=(-4 * hit, 0, 6 * math.sin(math.tau * 1.5 * t)), pos=(0, -0.8 * hit, 0))
        poser.turn(pose, "head", rot=(-10 * hit, 0, 0))
        poser.turn(pose, "core", scale=[1 - 0.18 * hit] * 3)
        return pose

    def death(t: float) -> Pose:
        # 先挺一下、背核爆亮，然后四腿外摊、身体整个塌下去贴地
        pose = Pose()
        sink = keys(t, [(0, 0), (0.2, 0.3), (1, 1.0)])
        poser.turn(pose, "body", rot=(4 * sink, 0, 5 * sink), pos=(0, keys(t, [(0, 0), (0.2, 0.8), (1, FUYA_DEATH_DROP)]), 0))
        for leg in FUYA_LEGS:
            outward = -1.0 if leg.endswith("l") else 1.0  # 建模稿里左腿在 -X，往 -X 摊
            poser.turn(pose, f"leg_{leg}", rot=(0, 0, outward * 16 * sink), pos=(0, FUYA_LEG_DROP * sink, 0))
            poser.turn(pose, f"foot_{leg}", rot=(0, 0, -outward * 10 * sink))
        poser.turn(pose, "head", rot=(4 * sink, 0, 6 * sink))
        poser.turn(pose, "jaw", rot=(keys(t, [(0, 0), (0.4, 12), (1, 8)]), 0, 0))
        poser.turn(pose, "core", scale=[keys(t, [(0, 1), (0.2, 1.35), (1, 0.35)])] * 3)
        return pose

    return {
        "idle": (4.0, True, idle), "walk": (1.6, True, walk), "attack": (1.0, False, attack),
        "hurt": (0.45, False, hurt), "death": (2.0, False, death),
    }


# ================================================================ 执念
# 拖地长袍的持剑残魂：双腿各只一节骨（没有膝），走动只能靠髋部摆腿，像幽魂拖着一具
# 不听使唤的躯壳。长袍前 / 后 / 两侧四片、三绺飘发各自一根骨，摆动才能错开相位；
# 右手虎口始终扣着剑柄——idle 里那只手会无征兆地一紧，是「执念」放不下的意象。
ZHINIAN_SINK = -2.6  # 死态髋部下沉量，由 check_ground() 反推


def zhinian_robe(poser: Poser, pose: Pose, t: float, cycles: float, amount: float, lag: float = 0.0) -> None:
    poser.turn(pose, "robe_front", rot=(amount * wave(t, cycles, lag), 0, 0))
    poser.turn(pose, "robe_back", rot=(-amount * 0.85 * wave(t, cycles, lag + 0.3), 0, 0))
    poser.turn(pose, "robe_side_l", rot=(0, 0, amount * 0.6 * wave(t, cycles, lag + 0.15)))
    poser.turn(pose, "robe_side_r", rot=(0, 0, -amount * 0.6 * wave(t, cycles, lag + 0.45)))


def zhinian_hair(poser: Poser, pose: Pose, t: float, cycles: float, amount: float, drag: float = 0.0) -> None:
    poser.turn(pose, "hair_l", rot=(drag + amount * wave(t, cycles, 0.1), 0, 0))
    poser.turn(pose, "hair_r", rot=(drag + amount * wave(t, cycles, 0.4), 0, 0))
    poser.turn(pose, "hair_back", rot=(drag + amount * 0.7 * wave(t, cycles, 0.25), 0, 0))


def zhinian_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def idle(t: float) -> Pose:
        pose = Pose()
        poser.turn(pose, "hips", pos=(0, 0.2 * wave(t, 1), 0))
        poser.turn(pose, "torso", rot=(2 + 1.3 * wave(t, 1, 0.1), 0, 0))
        twitch = math.sin(math.pi * smooth((t - 0.58) / 0.08)) if 0.58 < t < 0.66 else 0.0
        poser.turn(pose, "head", rot=(3 * wave(t, 1, 0.2), 10 * twitch, 0))
        for side, phase in (("l", 0.0), ("r", 0.4)):
            poser.turn(pose, f"arm_{side}", rot=(2 * wave(t, 1, phase), 0, 0))
            poser.turn(pose, f"claw_{side}", rot=(5 * wave(t, 2, phase), 0, 0))
        # 右手无征兆地一紧——放不下的执念
        grip = math.sin(math.pi * smooth((t - 0.22) / 0.1)) if 0.22 < t < 0.32 else 0.0
        poser.turn(pose, "forearm_r", rot=(-8 * grip, 0, 0))
        poser.turn(pose, "sword", rot=(14 * grip, 0, 0))
        zhinian_robe(poser, pose, t, cycles=1, amount=5)
        zhinian_hair(poser, pose, t, cycles=1, amount=4)
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        step = wave(t, 1)  # >0 = 左腿在前
        poser.turn(pose, "hips", rot=(0, 7 * step, 2 * wave(t, 1, 0.25)),
                   pos=(0, -0.35 * abs(wave(t, 1, 0.25)), 0))
        poser.turn(pose, "leg_l", rot=(-18 * step, 0, 0))
        poser.turn(pose, "leg_r", rot=(18 * step, 0, 0))
        poser.turn(pose, "torso", rot=(9, -4 * step, 0))
        poser.turn(pose, "head", rot=(-3, 4 * step, 0))
        poser.turn(pose, "arm_l", rot=(12 * step, 0, 0))
        poser.turn(pose, "claw_l", rot=(4 * wave(t, 1, 0.2), 0, 0))
        poser.turn(pose, "arm_r", rot=(-5 * step, 0, 0))  # 持剑手摆幅收着，剑不乱甩
        poser.turn(pose, "forearm_r", rot=(-3 * wave(t, 1, 0.1), 0, 0))
        zhinian_robe(poser, pose, t, cycles=2, amount=8, lag=-0.12)
        zhinian_hair(poser, pose, t, cycles=2, amount=7, drag=-6)
        return pose

    def attack(t: float) -> Pose:
        pose = Pose()
        poser.turn(pose, "hips", rot=(0, keys(t, [(0, 0), (0.35, -12), (0.55, 20), (0.7, 14), (1, 0)]), 0),
                   pos=(0, 0, keys(t, [(0, 0), (0.35, -0.6), (0.55, 1.8), (1, 0)])))
        poser.turn(pose, "torso", rot=(keys(t, [(0, 0), (0.35, -6), (0.55, 10), (0.7, 6), (1, 0)]),
                                       keys(t, [(0, 0), (0.35, -20), (0.55, 26), (0.7, 18), (1, 0)]), 0))
        poser.turn(pose, "head", rot=(0, keys(t, [(0, 0), (0.35, -10), (0.55, 14), (1, 0)]), 0))
        poser.turn(pose, "arm_r", rot=(keys(t, [(0, 0), (0.35, -75), (0.55, 35), (0.7, 10), (1, 0)]), 0,
                                       keys(t, [(0, 0), (0.35, 22), (0.55, -42), (0.7, -14), (1, 0)])))
        poser.turn(pose, "forearm_r", rot=(keys(t, [(0, 0), (0.35, -28), (0.55, 55), (0.7, 12), (1, 0)]), 0, 0))
        poser.turn(pose, "claw_r", rot=(keys(t, [(0, 0), (0.55, -18), (1, 0)]), 0, 0))
        poser.turn(pose, "sword", rot=(keys(t, [(0, 0), (0.35, 14), (0.55, -10), (1, 0)]), 0, 0))
        poser.turn(pose, "arm_l", rot=(keys(t, [(0, 0), (0.35, 16), (0.55, -22), (1, 0)]), 0, 0))
        zhinian_robe(poser, pose, t, cycles=1, amount=11, lag=0.08)
        zhinian_hair(poser, pose, t, cycles=1, amount=11, drag=keys(t, [(0, 0), (0.55, 16), (1, 0)]))
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "torso", rot=(-14 * hit, 0, 0))
        poser.turn(pose, "head", rot=(-10 * hit, 0, 8 * math.sin(math.tau * 1.5 * t)))
        poser.turn(pose, "hips", pos=(0, 0, -1.0 * hit))
        poser.turn(pose, "arm_l", rot=(-18 * hit, 0, 0))
        poser.turn(pose, "arm_r", rot=(-10 * hit, 0, 6 * hit))
        poser.turn(pose, "sword", rot=(-12 * hit, 0, 0))
        zhinian_robe(poser, pose, t, cycles=1, amount=16 * hit)
        zhinian_hair(poser, pose, t, cycles=1, amount=16 * hit)
        return pose

    def death(t: float) -> Pose:
        # 执念散尽：先一个趔趄（握剑的手先一松），再整个人往前折叠塌下去，长袍摊平、
        # 发丝垂落；腿没有膝关节，只在髋部略向外撇，不做屈膝。
        pose = Pose()
        stagger = keys(t, [(0, 0), (0.22, 1.0), (0.4, 0.5)])
        fold = keys(t, [(0.3, 0), (1, 1.0)])
        poser.turn(pose, "hips", rot=(20 * fold, 0, 0), pos=(0, ZHINIAN_SINK * fold, 2.0 * fold))
        poser.turn(pose, "torso", rot=(10 * stagger + 55 * fold, 0, 0))
        poser.turn(pose, "head", rot=(15 * fold, 0, keys(t, [(0.2, 0), (0.6, 10), (1, 16)])))
        poser.turn(pose, "leg_l", rot=(-14 * fold, 0, -8 * fold))
        poser.turn(pose, "leg_r", rot=(-14 * fold, 0, 8 * fold))
        poser.turn(pose, "arm_l", rot=(-22 * fold, 0, 0))
        poser.turn(pose, "arm_r", rot=(-30 * stagger - 10 * fold, 0, 0))
        poser.turn(pose, "forearm_r", rot=(keys(t, [(0, 0), (0.22, -14), (1, 20)]), 0, 0))
        poser.turn(pose, "claw_r", rot=(30 * fold, 0, 0))
        poser.turn(pose, "sword", rot=(keys(t, [(0, 0), (0.22, 10), (1, -24)]), 0, 0))
        zhinian_robe(poser, pose, t, cycles=1, amount=10 * (1 - fold) + 4, lag=0.1)
        zhinian_hair(poser, pose, t, cycles=1, amount=6 * (1 - fold), drag=20 * fold)
        return pose

    return {
        "idle": (3.6, True, idle), "walk": (1.4, True, walk), "attack": (0.85, False, attack),
        "hurt": (0.42, False, hurt), "death": (1.8, False, death),
    }


# ================================================================ 秘境守灵
# 石甲巨像守卫：重甲披挂，动作沉、慢、方——要的是巨像苏醒时的重量感，不是生物的呼吸感。
# 腰带前 / 后襟与两侧腰石片各自一根骨，错相摆才不会像一整块布在晃。
TSY_SINK = -4.0  # 死态下沉量（倒地），由 check_ground() 反推


def tsy_cloth(poser: Poser, pose: Pose, t: float, cycles: float, amount: float, drag: float = 0.0) -> None:
    poser.turn(pose, "tabard_front", rot=(drag + amount * wave(t, cycles, 0.1), 0, 0))
    poser.turn(pose, "tabard_back", rot=(drag + amount * 0.85 * wave(t, cycles, 0.35), 0, 0))
    for side, phase in (("l", 0.2), ("r", 0.45)):
        sign = -1.0 if side == "l" else 1.0
        poser.turn(pose, f"tabard_side_{side}", rot=(0, 0, sign * amount * 0.6 * wave(t, cycles, phase)))
        poser.turn(pose, f"hipguard_{side}", rot=(drag * 0.6 + amount * 0.5 * wave(t, cycles, phase + 0.1), 0, 0))


def tsy_sentinel_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def idle(t: float) -> Pose:
        pose = Pose()
        shift = wave(t, 1)
        poser.turn(pose, "hips", rot=(0, 0, 2 * shift), pos=(0, 0.15 * wave(t, 2), 0))
        poser.turn(pose, "torso", rot=(1.5 + 0.8 * wave(t, 1, 0.15), -2 * shift, 0))
        poser.turn(pose, "head", rot=(0, 10 * wave(t, 1, 0.3), 0))
        for side, phase in (("l", 0.0), ("r", 0.5)):
            poser.turn(pose, f"arm_{side}", rot=(1.5 * wave(t, 1, phase), 0, 0))
            poser.turn(pose, f"forearm_{side}", rot=(2 * wave(t, 2, phase), 0, 0))
        tsy_cloth(poser, pose, t, cycles=1, amount=3)
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        step = wave(t, 1)  # >0 = 右腿在前
        bend_r = max(0.0, -wave(t, 1, 0.2))
        bend_l = max(0.0, wave(t, 1, 0.2))
        thud = abs(math.cos(math.tau * t)) ** 6  # 每次落脚身体顿一下，一步两次
        poser.turn(pose, "hips", rot=(0, 10 * step, 3 * wave(t, 1, 0.25)), pos=(0, -0.5 * thud, 0))
        poser.turn(pose, "torso", rot=(6, -8 * step, 0))
        poser.turn(pose, "head", rot=(3 * thud, 4 * step, 0))
        poser.turn(pose, "leg_r", rot=(-26 * step, 0, 0))
        poser.turn(pose, "shin_r", rot=(30 * bend_r, 0, 0))
        poser.turn(pose, "leg_l", rot=(26 * step, 0, 0))
        poser.turn(pose, "shin_l", rot=(30 * bend_l, 0, 0))
        poser.turn(pose, "arm_r", rot=(-14 * step, 0, 0))
        poser.turn(pose, "arm_l", rot=(14 * step, 0, 0))
        tsy_cloth(poser, pose, t, cycles=2, amount=10, drag=-6)
        return pose

    def attack(t: float) -> Pose:
        pose = Pose()
        poser.turn(pose, "hips", pos=(0, 0, keys(t, [(0, 0), (0.4, -1.0), (0.55, 2.4), (1, 0)])))
        poser.turn(pose, "torso", rot=(keys(t, [(0, 0), (0.4, -4), (0.55, 14), (0.75, 10), (1, 0)]),
                                       keys(t, [(0, 0), (0.4, -24), (0.55, 20), (0.75, 10), (1, 0)]), 0))
        poser.turn(pose, "head", rot=(keys(t, [(0, 0), (0.4, -6), (0.55, 10), (1, 0)]), 0, 0))
        poser.turn(pose, "arm_r", rot=(keys(t, [(0, 0), (0.4, -100), (0.55, 40), (0.7, 10), (1, 0)]), 0,
                                       keys(t, [(0, 0), (0.4, 18), (0.55, -8), (1, 0)])))
        poser.turn(pose, "forearm_r", rot=(keys(t, [(0, 0), (0.4, -20), (0.55, 70), (0.7, 15), (1, 0)]), 0, 0))
        poser.turn(pose, "arm_l", rot=(keys(t, [(0, 0), (0.4, 30), (0.55, -20), (1, 0)]), 0, 0))
        poser.turn(pose, "leg_r", rot=(keys(t, [(0, 0), (0.4, -8), (0.55, 10), (1, 0)]), 0, 0))
        poser.turn(pose, "shin_r", rot=(keys(t, [(0, 0), (0.55, 14), (1, 0)]), 0, 0))
        tsy_cloth(poser, pose, t, cycles=1, amount=14, drag=keys(t, [(0, 0), (0.55, -10), (1, 0)]))
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "torso", rot=(-10 * hit, 0, 0))
        poser.turn(pose, "head", rot=(-14 * hit, 0, 0))
        poser.turn(pose, "hips", pos=(0, 0, -1.0 * hit))
        poser.turn(pose, "arm_l", rot=(-8 * hit, 0, 0))
        poser.turn(pose, "arm_r", rot=(-8 * hit, 0, 0))
        tsy_cloth(poser, pose, t, cycles=1, amount=14 * hit)
        return pose

    def death(t: float) -> Pose:
        # 先一个前栽的踉跄（重心前冲），膝盖撑不住后彻底向前扑倒——石像倒塌没有缓冲。
        pose = Pose()
        stagger = keys(t, [(0, 0), (0.25, 1.0), (0.4, 0.6)])
        topple = keys(t, [(0.3, 0), (1, 1.0)])
        poser.turn(pose, "hips", rot=(22 * topple, 0, 0), pos=(0, TSY_SINK * topple, 3.0 * topple))
        poser.turn(pose, "torso", rot=(14 * stagger + 60 * topple, 0, 0))
        poser.turn(pose, "head", rot=(20 * topple, 0, 10 * stagger))
        poser.turn(pose, "leg_r", rot=(-30 * topple, 0, 0))
        poser.turn(pose, "leg_l", rot=(-30 * topple, 0, 0))
        poser.turn(pose, "shin_r", rot=(70 * topple, 0, 0))
        poser.turn(pose, "shin_l", rot=(70 * topple, 0, 0))
        poser.turn(pose, "arm_r", rot=(-40 * topple, 0, 0))
        poser.turn(pose, "arm_l", rot=(-40 * topple, 0, 0))
        poser.turn(pose, "forearm_r", rot=(20 * topple, 0, 0))
        poser.turn(pose, "forearm_l", rot=(20 * topple, 0, 0))
        tsy_cloth(poser, pose, t, cycles=1, amount=6, drag=-24 * topple)
        return pose

    return {
        "idle": (4.4, True, idle), "walk": (1.8, True, walk), "attack": (1.1, False, attack),
        "hurt": (0.45, False, hurt), "death": (2.1, False, death),
    }


# ================================================================ 渊空畸变体
# 低伏爬行的虚空畸变躯体：前肢（爪，挂在 body 上）探地抓拽，后腿（挂在 root 上）蹬地
# 推进，对角步态（右前爪配左后腿、左前爪配右后腿）。虚空口无规律地张合、两侧前肢
# 偶尔各自不同步地抽搐——「畸变」的辨识度就在不对称，不追求生物该有的协调感。
VOID_SINK = -2.2  # 死态下沉量，由 check_ground() 反推


def void_front_limb(poser: Poser, pose: Pose, t: float, side: str, phase: float,
                     reach: float, lift: float) -> None:
    u = frac(t + phase)
    if u < 0.5:  # 抓握相：往前探、压低
        s = u / 0.5
        sweep = reach * (2 * smooth(s) - 1)
        raise_y = 0.0
    else:  # 收回相：抬起向后甩回
        s = (u - 0.5) / 0.5
        sweep = reach * (1 - 2 * s)
        raise_y = lift * math.sin(math.pi * s)
    poser.turn(pose, f"arm_{side}", rot=(sweep, 0, 0), pos=(0, raise_y, 0))
    poser.turn(pose, f"claw_{side}", rot=(-raise_y * 1.6, 0, 0))


def void_rear_leg(poser: Poser, pose: Pose, t: float, side: str, phase: float,
                   stride: float, lift: float) -> None:
    u = frac(t + phase)
    if u < 0.5:  # 摆动相：屈膝前送
        s = u / 0.5
        sweep = -stride * (2 * smooth(s) - 1)
        knee = lift * math.sin(math.pi * s)
    else:  # 支撑相：蹬地往后划
        s = (u - 0.5) / 0.5
        sweep = -stride * (1 - 2 * s)
        knee = 0.0
    poser.turn(pose, f"leg_{side}", rot=(sweep, 0, 0))
    poser.turn(pose, f"shin_{side}", rot=(knee, 0, 0))


VOID_GAIT = {"arm_r": 0.0, "leg_l": 0.0, "arm_l": 0.5, "leg_r": 0.5}  # 对角步态相位


def void_distorted_clips(poser: Poser) -> dict[str, tuple[float, bool, Sampler]]:
    def idle(t: float) -> Pose:
        pose = Pose()
        poser.turn(pose, "body", rot=(2 * wave(t, 1), 0, 1.5 * wave(t, 1, 0.3)), pos=(0, 0.3 * wave(t, 2), 0))
        # 虚空口无规律地张合，两次不等长的开合
        gape1 = math.sin(math.pi * smooth((t - 0.15) / 0.15)) if 0.15 < t < 0.30 else 0.0
        gape2 = math.sin(math.pi * smooth((t - 0.62) / 0.08)) if 0.62 < t < 0.70 else 0.0
        poser.turn(pose, "maw", rot=(-26 * gape1 - 14 * gape2, 4 * wave(t, 1, 0.4), 0))
        # 两侧前肢各自不同步地抽搐——不对称才是畸变
        twitch_r = math.sin(math.pi * smooth((t - 0.4) / 0.08)) if 0.4 < t < 0.48 else 0.0
        twitch_l = math.sin(math.pi * smooth((t - 0.75) / 0.1)) if 0.75 < t < 0.85 else 0.0
        poser.turn(pose, "arm_r", rot=(10 * twitch_r, 0, 6 * twitch_r))
        poser.turn(pose, "claw_r", rot=(-14 * twitch_r, 0, 0))
        poser.turn(pose, "arm_l", rot=(8 * twitch_l, 0, -5 * twitch_l))
        poser.turn(pose, "claw_l", rot=(-12 * twitch_l, 0, 0))
        for side, phase in (("l", 0.2), ("r", 0.55)):
            poser.turn(pose, f"leg_{side}", rot=(1.5 * wave(t, 1, phase), 0, 0))
        return pose

    def walk(t: float) -> Pose:
        pose = Pose()
        void_front_limb(poser, pose, t, "r", VOID_GAIT["arm_r"], reach=10, lift=8)
        void_front_limb(poser, pose, t, "l", VOID_GAIT["arm_l"], reach=10, lift=8)
        void_rear_leg(poser, pose, t, "r", VOID_GAIT["leg_r"], stride=12, lift=10)
        void_rear_leg(poser, pose, t, "l", VOID_GAIT["leg_l"], stride=12, lift=10)
        poser.turn(pose, "body", rot=(3 * wave(t, 2), 0, 2 * wave(t, 1)), pos=(0, 0.4 * abs(wave(t, 2)), 0))
        poser.turn(pose, "maw", rot=(4 * wave(t, 2, 0.1), 3 * wave(t, 1), 0))
        return pose

    def attack(t: float) -> Pose:
        pose = Pose()
        lunge = keys(t, [(0, 0), (0.35, -0.8), (0.55, 3.2), (0.7, 2.6), (1, 0)])
        rise = keys(t, [(0, 0), (0.35, 0.8), (0.55, -0.4), (1, 0)])
        poser.turn(pose, "body", rot=(keys(t, [(0, 0), (0.35, -6), (0.55, 10), (0.7, 6), (1, 0)]), 0, 0),
                   pos=(0, rise, lunge))
        poser.turn(pose, "maw", rot=(keys(t, [(0, 0), (0.35, -34), (0.55, 8), (0.65, 2), (1, 0)]), 0, 0))
        for side in ("l", "r"):
            poser.turn(pose, f"arm_{side}", rot=(keys(t, [(0, 0), (0.35, -14), (0.55, 24), (0.7, 10), (1, 0)]), 0, 0))
            poser.turn(pose, f"claw_{side}", rot=(keys(t, [(0, 0), (0.55, -30), (0.7, -8), (1, 0)]), 0, 0))
            poser.turn(pose, f"leg_{side}", rot=(keys(t, [(0, 0), (0.35, 10), (0.55, -12), (1, 0)]), 0, 0))
        return pose

    def hurt(t: float) -> Pose:
        pose = Pose()
        hit = math.sin(math.pi * t)
        poser.turn(pose, "body", rot=(-10 * hit, 0, 8 * math.sin(math.tau * 2 * t)), pos=(0, -0.6 * hit, -1.0 * hit))
        poser.turn(pose, "maw", rot=(18 * hit, 0, 0))
        for side in ("l", "r"):
            sign = 1.0 if side == "r" else -1.0
            poser.turn(pose, f"arm_{side}", rot=(0, 0, sign * 16 * hit))
            poser.turn(pose, f"claw_{side}", rot=(-20 * hit, 0, 0))
        return pose

    def death(t: float) -> Pose:
        # 整个身体瘫下去贴地，前肢爪子摊开松爪，后腿向外滑开，虚空口张开后不再合拢
        pose = Pose()
        sink = keys(t, [(0, 0), (0.2, 0.4), (1, 1.0)])
        poser.turn(pose, "body", rot=(8 * sink, 0, 6 * sink),
                   pos=(0, keys(t, [(0, 0), (0.2, 0.6), (1, VOID_SINK)]), 0))
        poser.turn(pose, "maw", rot=(keys(t, [(0, 0), (0.3, -30), (1, -20)]), 0, 0))
        for side in ("l", "r"):
            sign = 1.0 if side == "r" else -1.0
            poser.turn(pose, f"arm_{side}", rot=(10 * sink, 0, sign * 20 * sink))
            poser.turn(pose, f"claw_{side}", rot=(-24 * sink, 0, 0))
            poser.turn(pose, f"leg_{side}", rot=(0, 0, sign * 22 * sink))
            poser.turn(pose, f"shin_{side}", rot=(18 * sink, 0, 0))
        return pose

    return {
        "idle": (4.0, True, idle), "walk": (1.3, True, walk), "attack": (0.9, False, attack),
        "hurt": (0.4, False, hurt), "death": (1.8, False, death),
    }


# ---------------------------------------------------------------- 导出
CLIPS = {
    "ash_spider_v2": ash_spider_clips,
    "skull_fiend_v2": skull_fiend_clips,
    "daoxiang_v2": daoxiang_clips,
    "fuya_v2": fuya_clips,
    "zhinian_v2": zhinian_clips,
    "tsy_sentinel_v2": tsy_sentinel_clips,
    "void_distorted_v2": void_distorted_clips,
}


# 离线预览没有地面参照，「人悬在半空」和「半截插进地里」在图上都看不出来
# （Round 1 的道伥死态就是看图判成悬空、实际却沉到 -7.7px）。所以每段动画逐帧量最低点：
# 生物任何时刻都不能入地超过这个容差（像素）。
GROUND_TOLERANCE = 0.6


def grounded(rig: PoseRig, poser: Poser, sampler: Sampler) -> Sampler:
    """地面约束：某一帧如果有部位压进地面，把整只生物（root）托起到刚好贴地。

    程序化动作只管姿态，不知道地面在哪 —— 蜘蛛后仰时腹囊尾端、前扑时毒牙都会插进地里。
    刚体骨骼没有 IK，最朴素也最诚实的解就是「被地面顶起来」：只在入地的那几帧整体上移，
    其余帧原样不动。
    """

    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < 0.0:
            poser.turn(pose, "root", pos=(0.0, -depth, 0.0))
        return pose

    return sample


def check_ground(name: str, clip: str, rig: PoseRig, sampler: Sampler) -> float:
    """返回整段动画里模型的最低点（px）；入地超过容差直接报错，并指出是哪一刻。"""

    lowest, when = math.inf, 0.0
    for step in range(SAMPLES + 1):
        t = step / SAMPLES
        height = rig.lowest(sampler(t))
        if height < lowest:
            lowest, when = height, t
    if lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{name}.{clip}: t01={when:.2f} 时最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    return lowest


def export(name: str) -> None:
    rig = PoseRig(gen_rig.write_rig(name))
    poser = Poser(rig)

    animations = []
    for clip, (length, loop, raw_sampler) in CLIPS[name](poser).items():
        sampler = grounded(rig, poser, raw_sampler)
        lowest = check_ground(name, clip, rig, sampler)
        print(f"  {clip}: 最低点 {lowest:+.2f}px")
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})
    animated = gen_rig.OUT / f"{name}_animated.bbmodel"
    write_animated_bbmodel(rig, animations, animated, name)

    STAGING.mkdir(parents=True, exist_ok=True)
    subprocess.run(["node", str(CODEC), str(animated), str(STAGING), name], check=True)
    staged_geo = (STAGING / f"{name}.geo.json").read_bytes()
    if staged_geo != (CLIENT / "geo" / f"{name}.geo.json").read_bytes():
        raise ValueError(f"{name}: 绑定稿导出的 geo 与已安装的不一致；先跑 export.py 再生成动画")
    staged = json.loads((STAGING / f"{name}.animation.json").read_text(encoding="utf-8"))["animations"]
    expected = {f"animation.bong.{name}.{clip['name']}" for clip in animations}
    if set(staged) != expected:
        raise ValueError(f"{name}: 导出的动画集 {sorted(staged)} 与作者稿 {sorted(expected)} 不一致")

    destination = CLIENT / "animations" / f"{name}.animation.json"
    shutil.copyfile(STAGING / f"{name}.animation.json", destination)
    summary = ", ".join(f"{a['name']} {a['length']}s{' loop' if a['loop'] else ''}" for a in animations)
    print(f"{name}: {summary} → {destination.relative_to(ROOT)}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--only", choices=tuple(CLIPS), action="append")
    args = parser.parse_args()
    for name in args.only or CLIPS:
        export(name)


if __name__ == "__main__":
    main()
