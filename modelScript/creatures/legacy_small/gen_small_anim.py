#!/usr/bin/env python3
"""给七只小型旧生物补齐缺的动画段（只追加，已有的段一帧不改）。

这七只的 geo 是 Blockbench 手搓的旧模型，骨头叫 ``bone14`` 这类名字，没有作者稿可重导出，
所以不走 ``fauna_v2`` 的「绑定稿 → codec」流水线，而是直接往已安装的
``client/.../animations/<id>.animation.json`` 里**追加**新段：

- 关键帧值就是 GeckoLib 的值（rotation 单位度，position 单位 px，已经是游戏里的符号），
  预览工具 ``creature_anim_frames.py`` 读同一份文件，看到的就是游戏里的样子；
- 写入是文本拼接：原文件字节一个不动，新段接在 ``animations`` 末尾；
- 重跑幂等：同名的新段会被替换，旧段永远不碰。

谁缺什么（任务卡）：

    green_spider / blue_spider   hurt death
    jungle_scorpion              hurt
    ice_scorpion                 walk hurt death
    cockade_snake / mandrake_snake  attack hurt death
    devour_rat                   hurt death

用法::

    python3 modelScript/creatures/legacy_small/gen_small_anim.py            # 全部
    python3 modelScript/creatures/legacy_small/gen_small_anim.py --only ice_scorpion
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import numpy as np

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "modelScript" / "tools"))
sys.path.insert(0, str(ROOT / "modelScript" / "exporters"))

from creature_anim_frames import CLIENT, load_rig  # noqa: E402
from review_creature_assets import sample  # noqa: E402

BAKE_STEP = 1 / 24  # 烘焙采样间隔（秒）；之后按容差裁掉共线点
KEY_TOLERANCE = 0.04  # 裁点容差：线性插值误差小于它的中间关键帧会被删（度 / px）
GROUND_TOLERANCE = 0.6  # 任何一帧入地超过这个像素数就报错（见 fauna_v2 的 Round 1 教训：肉眼看不出悬空/入地）

Vec = list[float]
ChannelPose = dict[str, Vec]  # "rotation" / "position" / "scale" → 向量
Pose = dict[str, ChannelPose]  # 骨名 → 该骨这一刻的变换


# ---------------------------------------------------------------- 曲线工具
def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep：points 是 (秒, 值) 的有序表，一次性动作用它排节奏。"""

    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


def wave(u: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """u ∈ [0,1] 上走 cycles 个整周期，循环动画首尾自然相接。"""

    return math.sin(math.tau * (cycles * u + phase))


# ---------------------------------------------------------------- 摆姿
def put(pose: Pose, bone: str, rot=None, pos=None, scale=None) -> None:
    """往某根骨累加变换；同一根骨可以被多处调用叠加（比如整体晃动 + 局部动作）。"""

    channels = pose.setdefault(bone, {})
    for channel, value in (("rotation", rot), ("position", pos), ("scale", scale)):
        if value is None:
            continue
        current = channels.setdefault(channel, [1.0, 1.0, 1.0] if channel == "scale" else [0.0, 0.0, 0.0])
        for axis in range(3):
            if channel == "scale":
                current[axis] *= value[axis]
            else:
                current[axis] += value[axis]


@dataclass
class Clip:
    length: float
    loop: bool
    sampler: Callable[[float], Pose]
    ground_root: str | None = None  # 非空 = 每帧量最低点，入地就把这根骨托起来


def bake(clip: Clip, lift: Callable[[Pose], float] | None) -> dict:
    """把采样函数烘成 GeckoLib 的 bones 表，再裁掉共线关键帧。"""

    count = max(2, round(clip.length / BAKE_STEP))
    times = [clip.length * index / count for index in range(count + 1)]
    if clip.loop:
        times[-1] = clip.length  # 末帧值必须等于首帧；采样函数是周期函数，由调用方保证

    poses = []
    for t in times:
        pose = clip.sampler(min(t, clip.length))
        if clip.loop and t == clip.length:
            pose = poses[0]
        if lift is not None and clip.ground_root:
            put(pose, clip.ground_root, pos=(0.0, lift(pose), 0.0))
        poses.append(pose)

    bones: dict[str, dict[str, dict[str, Vec]]] = {}
    names = sorted({bone for pose in poses for bone in pose})
    for bone in names:
        for channel in ("rotation", "position", "scale"):
            neutral = [1.0, 1.0, 1.0] if channel == "scale" else [0.0, 0.0, 0.0]
            series = [pose.get(bone, {}).get(channel, neutral) for pose in poses]
            if all(max(abs(a - b) for a, b in zip(value, neutral)) < 1e-6 for value in series):
                continue
            kept = simplify(times, series)
            bones.setdefault(bone, {})[channel] = {
                stamp(times[index]): [round(component, 3) for component in series[index]] for index in kept
            }
    return bones


def stamp(seconds: float) -> str:
    """关键帧时间键：与现有文件一致，最多四位小数、不带多余的 0（0 写作 "0.0"）。"""

    text = f"{seconds:.4f}".rstrip("0")
    return text + "0" if text.endswith(".") else text


def simplify(times: list[float], series: list[Vec]) -> list[int]:
    """保留首尾，递归地只留下线性插值偏差超过容差的点（Ramer–Douglas–Peucker 的时间轴版）。"""

    keep = {0, len(times) - 1}

    def split(lo: int, hi: int) -> None:
        worst, worst_error = None, KEY_TOLERANCE
        for index in range(lo + 1, hi):
            s = (times[index] - times[lo]) / (times[hi] - times[lo])
            error = max(
                abs(series[index][axis] - (series[lo][axis] + (series[hi][axis] - series[lo][axis]) * s))
                for axis in range(3)
            )
            if error > worst_error:
                worst, worst_error = index, error
        if worst is not None:
            keep.add(worst)
            split(lo, worst)
            split(worst, hi)

    split(0, len(times) - 1)
    return sorted(keep)


# ---------------------------------------------------------------- 写入（文本拼接，不动旧字节）
def indent_unit(raw: str) -> str:
    second_line = raw.splitlines()[1]
    return "\t" if second_line.startswith("\t") else " " * (len(second_line) - len(second_line.lstrip()))


def strip_generated(raw: str, names: list[str], unit: str) -> str:
    """去掉上次生成的段，使重跑幂等。

    生成的段总是接在 animations 末尾，所以从第一个同名段处截断；截断前先核对被截掉的部分
    只含本脚本的段名——万一同名的是手工旧段，宁可报错也不删。
    """

    closing = f"\n{unit}}}\n}}"
    if not raw.rstrip().endswith(closing):
        raise ValueError("animation.json 结尾不是预期的两层闭合，不敢拼接")
    cuts = [raw.find(f'\n{unit * 2}"{name}": {{') for name in names]
    cuts = [cut for cut in cuts if cut >= 0]
    if not cuts:
        return raw
    cut = min(cuts)
    removed = json.loads("{" + raw[cut : raw.rstrip().rindex(closing)].lstrip(",\n \t") + "}")
    foreign = set(removed) - set(names)
    if foreign:
        raise ValueError(f"同名段后面还有不是本脚本生成的段 {sorted(foreign)}，不敢截断")
    head = raw[:cut].rstrip()
    if not head.endswith(","):
        raise ValueError("截断点前不是逗号，文件结构异常")
    return head[:-1] + closing + ("\n" if raw.endswith("\n") else "")


def append_clips(raw: str, additions: dict[str, str], unit: str) -> str:
    closing = f"\n{unit}}}\n}}"
    trailing = raw[len(raw.rstrip()) :]
    body = raw.rstrip()[: -len(closing)]
    blocks = []
    for text in additions.values():
        # clip_text 去掉了最外层花括号，里面的内容已经带着一层缩进，这里只需再补一层就到 animations 的子级
        blocks.append("\n".join(unit + line for line in text.splitlines()))
    return body + ",\n" + ",\n".join(blocks) + closing + trailing


def clip_text(full_name: str, clip: Clip, bones: dict, unit: str) -> str:
    body: dict = {}
    if clip.loop:
        body["loop"] = True
    body["animation_length"] = clip.length
    body["bones"] = bones
    text = json.dumps({full_name: body}, indent=unit, ensure_ascii=False)
    # 三个数的向量压成一行（ice_scorpion 的手工段就是这个样子），否则一个关键帧占五行
    text = re.sub(r"\[\s+(-?[\d.]+),\s+(-?[\d.]+),\s+(-?[\d.]+)\s+\]", r"[\1, \2, \3]", text)
    return "\n".join(text.splitlines()[1:-1])  # 去掉最外层花括号，只留 "名字": {...} 这一段


# ---------------------------------------------------------------- 绿蛛 / 蓝蛛（同一套 geo，只有贴图不同）
#
# 骨骼语义（读 geo + 世界包围盒 + 探针得到，不是猜的）：
#   bone      根（无方块）            bone2   躯干组（枢轴在腹下）
#   bone14    胸                      bone13  腹囊      bone15  头
#   bone3/4   螯牙（+x / -x 侧）
#   八条腿按前→后四对：(bone5, bone6) (bone10, bone7) (bone8, bone9) (bone11, bone12)，
#   每对第一根在 -x 侧、第二根在 +x 侧。
#
# 符号约定（探针 probe 实测，世界 x 以渲染器为准）：
#   rotation.x 为正 = 躯干前倾低头；负 = 后仰。
#   腿 rotation.z：+x 侧为正 = 抬腿，-x 侧要取负，所以抬腿量统一写 side * amount。
#   腿 rotation.y：-x 侧为正 = 向前迈，所以前迈量写 -side * amount。
SPIDER_LEG_PAIRS = (("bone5", "bone6"), ("bone10", "bone7"), ("bone8", "bone9"), ("bone11", "bone12"))
SIDE_OF_LEFT_RIGHT = (-1.0, 1.0)  # 一对腿里第一根在 -x 侧，第二根在 +x 侧


def spider_legs(
    pose: Pose,
    lift: Callable[[int, float], float],
    sweep: Callable[[int], float] = lambda pair: 0.0,
) -> None:
    """按对设置八条腿：lift(pair, side) 是抬腿角、sweep(pair) 是前迈角（度）。

    lift 带上 side 是为了让左右腿可以略有不同——死透的蜘蛛不会左右完全对称。
    """

    for pair, bones in enumerate(SPIDER_LEG_PAIRS):
        for bone, side in zip(bones, SIDE_OF_LEFT_RIGHT):
            put(pose, bone, rot=(0.0, -side * sweep(pair), side * lift(pair, side)))


def spider_hurt(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        put(pose, "bone", pos=(0.0, 0.0, keys(t, [(0, 0), (0.08, 3.6), (0.5, 0)])))
        put(pose, "bone", rot=(0.0, 0.0, keys(t, [(0, 0), (0.1, 7), (0.22, -5), (0.34, 3), (0.5, 0)])))
        put(pose, "bone2", rot=(keys(t, [(0, 0), (0.08, -16), (0.22, 4), (0.5, 0)]), 0.0, 0.0))
        put(pose, "bone13", rot=(keys(t, [(0, 0), (0.1, 12), (0.3, -5), (0.5, 0)]), 0.0, 0.0))
        put(pose, "bone15", rot=(keys(t, [(0, 0), (0.08, -14), (0.3, 4), (0.5, 0)]), 0.0, 0.0))
        for fang, side in (("bone3", 1.0), ("bone4", -1.0)):
            put(pose, fang, rot=(0.0, -side * keys(t, [(0, 0), (0.1, 24), (0.4, 6), (0.5, 0)]), 0.0))
        # 受惊时八条腿一起弹起，前后对错开 30ms 左右，像一阵抖而不是整体刚体抬升
        spider_legs(
            pose,
            lift=lambda pair, side: keys(t, [(0, 0), (0.1 + 0.02 * pair, 34 - 4 * pair), (0.32, 8), (0.5, 0)]),
            sweep=lambda pair: keys(t, [(0, 0), (0.1, 14 - 8 * pair), (0.5, 0)]),
        )
        return pose

    return Clip(0.5, False, sampler, ground_root="bone")


def spider_death(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        # 被打飞后侧翻成肚皮朝天：根骨绕 z 翻 180°，落地高度由 ground_root 托住
        put(pose, "bone", rot=(0.0, 0.0, keys(t, [(0, 0), (0.16, 6), (0.85, 172), (1.1, 180), (1.5, 180)])))
        put(pose, "bone", pos=(0.0, 0.0, keys(t, [(0, 0), (0.16, 2.8), (0.5, 3.4), (1.5, 3.4)])))
        put(pose, "bone2", rot=(keys(t, [(0, 0), (0.16, -12), (0.6, -4), (1.5, -4)]), 0.0, 0.0))
        put(pose, "bone13", rot=(keys(t, [(0, 0), (0.2, 12), (0.9, 18), (1.5, 20)]), 0.0, 0.0))
        put(pose, "bone15", rot=(keys(t, [(0, 0), (0.16, -16), (0.9, 10), (1.5, 14)]), 0.0, 0.0))
        for fang, side in (("bone3", 1.0), ("bone4", -1.0)):
            put(pose, fang, rot=(0.0, -side * keys(t, [(0, 0), (0.2, 30), (1.5, 18)]), 0.0))

        def lift(pair: int, side: float) -> float:
            # 先乱蹬（抬高并随翻身摆动），肚皮朝天后八条腿向腹下蜷缩，尖端收到肚子上方
            flail = 12 * math.sin(math.tau * (t * 3.2 - 0.18 * pair)) * keys(t, [(0, 0), (0.25, 1), (0.85, 1), (1.1, 0)])
            curl = keys(t, [(0.0, 0), (0.18, 30), (0.85, 36), (1.5, -(104 + 7 * pair) + 7 * side)])
            twitch = 4 * math.sin(math.tau * (t * 5.0 + 0.3 * pair)) * keys(t, [(1.0, 0), (1.15, 1), (1.5, 0)])
            return curl + flail + twitch

        spider_legs(pose, lift=lift, sweep=lambda pair: keys(t, [(0.0, 0), (0.85, 12), (1.5, -10 + 4 * pair)]))
        return pose

    return Clip(1.5, False, sampler, ground_root="bone")


# ---------------------------------------------------------------- 丛林蝎（只缺 hurt）
#
# geo 里整只蝎子挂在 ``nose`` 下，而 nose 本身绑定旋转了 87.5°，所以子骨的局部轴和世界轴是错开的：
#   腿（局部）rotation.z = 前后摆，-x 侧为正 = 向后；rotation.y = 抬落，+x 侧为正 = 抬腿。
#   钳臂 rotation.z：-x 侧为正 = 向外并后摆。尾节 rotation.x 为负 = 尾尖向后上翘。
JUNGLE_LEGS = (("bone24", "bone32"), ("bone26", "bone34"), ("bone27", "bone36"))  # (-x 侧, +x 侧)，后→前


def jungle_hurt(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        put(pose, "nose", pos=(0.0, 0.0, keys(t, [(0, 0), (0.08, 3.5), (0.5, 0)])))
        put(pose, "nose", rot=(keys(t, [(0, 0), (0.08, -9), (0.24, 3), (0.5, 0)]), 0.0, keys(t, [(0, 0), (0.12, 6), (0.28, -4), (0.5, 0)])))
        put(pose, "bone4", rot=(keys(t, [(0, 0), (0.08, -16), (0.26, 4), (0.5, 0)]), 0.0, 0.0))
        # 尾巴被抽得向后上甩，然后沿尾节逐段落回：每段比前一段晚 25ms
        for index, bone in enumerate(("bone6", "bone7", "bone8", "bone9")):
            delay = 0.03 * index
            put(pose, bone, rot=(keys(t, [(0, 0), (0.1 + delay, -15), (0.3 + delay, 5), (0.5, 0)]), 0.0, 0.0))
        for index, (right, left) in enumerate(JUNGLE_LEGS):
            lift = keys(t, [(0, 0), (0.09 + 0.02 * index, 22), (0.32, 6), (0.5, 0)])
            put(pose, right, rot=(0.0, -lift, 0.0))
            put(pose, left, rot=(0.0, lift, 0.0))
        for bone, side in (("bone12", -1.0), ("bone18", 1.0)):
            put(pose, bone, rot=(0.0, 0.0, -side * keys(t, [(0, 0), (0.1, 20), (0.34, -6), (0.5, 0)])))
        return pose

    return Clip(0.5, False, sampler, ground_root="nose")


# ---------------------------------------------------------------- 冰蝎（缺 walk / hurt / death）
#
# 骨骼语义：bone9 根；bone41 前身+头甲、bone31 面具头；bone50 / bone49 中后身；
# 尾巴一串 bone25→35→36→38→39→40→37（37 是毒针）；
# 钳：-x 侧 bone42→43→bone6（→钳指 bone/2/3/4），+x 侧 bone17→18→19（→20…）；
# 六条腿（上段，下段是它的子骨）：-x 侧 bone7 bone26 bone28，+x 侧 bone46 bone33 bone30，前→后。
# 符号（探针实测）：腿 rotation.z：+x 侧为正 = 抬腿；rotation.y：-x 侧为正 = 向前迈。
#   尾节 rotation.x 为正 = 尾尖向前下弯；头 rotation.x 为负 = 抬头。
ICE_LEGS = (("bone7", "bone46"), ("bone26", "bone33"), ("bone28", "bone30"))  # (-x 侧, +x 侧)，前→后
ICE_TAIL = ("bone25", "bone35", "bone36", "bone38", "bone39", "bone40", "bone37")


def ice_walk(rig) -> Clip:
    def sampler(u: float) -> Pose:
        pose: Pose = {}
        # 对角步态：-x 前 / +x 中 / -x 后 同相，另外三条反相
        for index, (right, left) in enumerate(ICE_LEGS):
            for bone, side, phase in ((right, -1.0, 0.5 * (index % 2)), (left, 1.0, 0.5 * ((index + 1) % 2))):
                cycle = wave(u, phase=phase)
                lifted = max(0.0, math.cos(math.tau * (u + phase)))  # 迈腿相才抬
                put(pose, bone, rot=(0.0, -side * 17 * cycle, side * 14 * lifted))
        put(pose, "bone9", pos=(0.0, 0.55 * (1 - math.cos(math.tau * 2 * u)) / 2 * 2, 0.0), rot=(0.0, 0.0, 1.4 * wave(u)))
        put(pose, "bone41", rot=(0.0, 2.2 * wave(u, phase=0.5), 0.0))
        put(pose, "bone31", rot=(2 * wave(u, 2, 0.25), 0.0, 0.0))
        for side, bone in ((-1.0, "bone42"), (1.0, "bone17")):
            put(pose, bone, rot=(0.0, side * 4 * wave(u, phase=0.25 * side), 0.0))
        # 尾巴像钟摆：整体随步伐左右摇，越靠近毒针越晚、越大
        for index, bone in enumerate(ICE_TAIL):
            put(pose, bone, rot=(0.0, 0.0, 0.0))
            put(pose, bone, rot=(1.2 * wave(u, 2, 0.1 * index), 1.5 * (index + 1) * 0.35 * wave(u, phase=-0.06 * index), 0.0))
        return pose

    return Clip(0.9, True, sampler, ground_root="bone9")


def ice_hurt(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        put(pose, "bone9", pos=(0.0, 0.0, keys(t, [(0, 0), (0.09, 5), (0.55, 0)])))
        put(pose, "bone9", rot=(keys(t, [(0, 0), (0.09, -5), (0.26, 2), (0.55, 0)]), 0.0, keys(t, [(0, 0), (0.12, 3), (0.3, -2), (0.55, 0)])))
        put(pose, "bone31", rot=(keys(t, [(0, 0), (0.09, -22), (0.3, 6), (0.55, 0)]), keys(t, [(0, 0), (0.12, 10), (0.34, -5), (0.55, 0)]), 0.0))
        for side, bone in ((-1.0, "bone42"), (1.0, "bone17")):
            put(pose, bone, rot=(0.0, -side * keys(t, [(0, 0), (0.1, 20), (0.36, -5), (0.55, 0)]), 0.0))
        for index, bone in enumerate(ICE_TAIL):
            delay = 0.025 * index
            put(pose, bone, rot=(keys(t, [(0, 0), (0.11 + delay, -7), (0.34 + delay, 3), (0.55, 0)]), 0.0, 0.0))
        for index, (right, left) in enumerate(ICE_LEGS):
            lift = keys(t, [(0, 0), (0.1 + 0.02 * index, 20), (0.34, 5), (0.55, 0)])
            put(pose, right, rot=(0.0, 0.0, -lift))
            put(pose, left, rot=(0.0, 0.0, lift))
        return pose

    return Clip(0.55, False, sampler, ground_root="bone9")


def ice_death(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        put(pose, "bone9", pos=(0.0, 0.0, keys(t, [(0, 0), (0.14, 6), (0.5, 7), (1.7, 7)])))
        # 侧翻倒地：先被打得后仰，再向 +x 侧倒下，倒后不再弹回
        put(pose, "bone9", rot=(keys(t, [(0, 0), (0.14, -8), (0.6, -2), (1.7, 0)]), 0.0, keys(t, [(0, 0), (0.14, 4), (0.9, 78), (1.1, 72), (1.7, 74)])))
        put(pose, "bone31", rot=(keys(t, [(0, 0), (0.14, -24), (0.9, 12), (1.7, 18)]), 0.0, 0.0))
        for side, bone in ((-1.0, "bone42"), (1.0, "bone17")):
            put(pose, bone, rot=(0.0, -side * keys(t, [(0, 0), (0.14, 22), (0.9, 40), (1.7, 52)]), 0.0))
        # 尾巴先高高甩起，再一节一节垮下来，毒针尖落地
        for index, bone in enumerate(ICE_TAIL):
            delay = 0.05 * index
            put(pose, bone, rot=(keys(t, [(0, 0), (0.14, -6), (0.5 + delay, 2), (1.2 + delay, 11 + index), (1.7, 12 + index)]), 0.0, 0.0))
        for index, (right, left) in enumerate(ICE_LEGS):
            flail = 9 * math.sin(math.tau * (t * 3.0 - 0.17 * index)) * keys(t, [(0, 0), (0.2, 1), (0.9, 1), (1.2, 0)])
            curl = keys(t, [(0, 0), (0.18, 22), (0.9, 30), (1.7, 62 + 4 * index)])
            put(pose, right, rot=(0.0, 0.0, -(curl + flail)))
            put(pose, left, rot=(0.0, 0.0, curl - flail))
        return pose

    return Clip(1.7, False, sampler, ground_root="bone9")


# ---------------------------------------------------------------- 彩冠蛇 / 曼陀蛇（缺 attack / hurt / death）
#
# 两条蛇共用一套骨架（彩冠蛇多一个冠 bone21，曼陀蛇没有）。这是一条**从头到尾串成一条链**的蛇：
#
#   bone20（根） → bone10（舌，同时是整条链的「头部坐标系」） → bone11（头）
#                → bone → bone3 → bone2 → bone4 → bone14 → … → bone19 → bone5 → … → bone9（尾尖）
#
# 绑定姿态是一条沿 +z 笔直躺平的蛇，每节枢轴在这一节的**前端**（朝头的一端），向后 7.5px。
# 所以任何一根骨转动，它后面的整条身子都跟着转。
#
# 摆姿思路：不逐骨猜角度，而是直接写「每一节朝后的绝对方向」：
#   pitch[i]：第 i 节朝后的方向的俯仰（度），正 = 向后上扬；负 = 向后下垂
#   yaw[i]  ：第 i 节朝后的方向的偏航（度），正 = 朝 -x 偏
# 再用相邻两节之差换成每根骨的相对旋转（探针实测 rotation.x 正 = 后段上抬，rotation.y 正 = 后段朝 -x）。
# 头部坐标系（bone10）自己再带一个整体俯仰/偏航/平移。
#
# 冠 bone21 不在链上，枢轴在原点，只能跟着头部坐标系的刚体运动一起搬：
#   先让 bone10 照刚体运动摆好，再把「原点经过这个刚体运动之后的位置」写到冠的 position，
#   旋转取同一组欧拉角。
SNAKE_SPINE = ("bone", "bone3", "bone2", "bone4", "bone14", "bone15", "bone16", "bone17", "bone18", "bone19",
               "bone5", "bone6", "bone7", "bone8", "bone9")

# 蓄势昂首：头朝前略低，颈弯成 S，竖起一根立柱，尾段在地上盘一圈
REARING_PITCH = [18, 6, -28, -62, -82, -88, -86, -74, -46, -16, 0, 0, 0, 0, 0]
REARING_YAW = [0, 0, 0, 0, 0, 0, 0, 0, 20, 45, 70, 95, 120, 145, 170]
# 软趴在地：整条身子贴地，带一条懒散的 S 形
LIMP_PITCH = [0] * 15
LIMP_YAW = [24 * math.sin(0.75 * index + 0.4) for index in range(15)]


def blend_profiles(a: list[float], b: list[float], weight: Callable[[int], float]) -> list[float]:
    """逐节混合两条剖面：weight(i) 为 0 取 a、为 1 取 b。让身体能从头到尾依次塌下来。"""

    return [a[i] + (b[i] - a[i]) * weight(i) for i in range(len(a))]


def snake_pose(
    rig,
    pitch: list[float],
    yaw: list[float],
    head_rot=(0.0, 0.0, 0.0),
    head_pos=(0.0, 0.0, 0.0),
) -> Pose:
    pose: Pose = {}
    put(pose, "bone10", rot=head_rot, pos=head_pos)
    previous_pitch, previous_yaw = head_rot[0], head_rot[1]
    for bone, bone_pitch, bone_yaw in zip(SNAKE_SPINE, pitch, yaw):
        put(pose, bone, rot=(bone_pitch - previous_pitch, bone_yaw - previous_yaw, 0.0))
        previous_pitch, previous_yaw = bone_pitch, bone_yaw
    if "bone21" in rig.order:
        carry_comb(rig, pose)
    return pose


def carry_comb(rig, pose: Pose) -> None:
    """让冠跟着头部坐标系走（见上面的说明）。"""

    head_only = {"bone10": pose["bone10"]}
    moved = rig.world(sample({"bones": head_only}, rig, 0.0))["bone10"]
    bound = rig.world(sample({"bones": {}}, rig, 0.0))["bone10"]
    rigid = moved @ np.linalg.inv(bound)
    shift = rigid[:3, 3]
    put(pose, "bone21", rot=pose["bone10"].get("rotation", (0.0, 0.0, 0.0)), pos=(-shift[0], shift[1], shift[2]))


def snake_hurt(rig) -> Clip:
    def sampler(t: float) -> Pose:
        shock = keys(t, [(0, 0), (0.07, 1), (0.5, 0)])
        # 颈部被打得向后一缩，随后一道波沿着身体传向尾巴并衰减
        pitch = [p + shock * k for p, k in zip(REARING_PITCH, (-58, -46, -28, -12, -4) + (0,) * 10)]
        fade = keys(t, [(0.05, 0), (0.12, 1), (0.5, 0)])
        yaw = [y + fade * 14 * math.sin(math.tau * (3.0 * t - 0.11 * i)) * (1 if i > 1 else 0.4) for i, y in enumerate(REARING_YAW)]
        head_rot = (-14 * shock, 26 * keys(t, [(0, 0), (0.08, 1), (0.28, -0.5), (0.5, 0)]), 0.0)
        return snake_pose(rig, pitch, yaw, head_rot, head_pos=(0.0, 3.0 * shock, 9.0 * shock))

    return Clip(0.5, False, sampler, ground_root="bone20")


def snake_attack(rig) -> Clip:
    def sampler(t: float) -> Pose:
        # 0–0.34 蓄力：颈向后收紧成更深的 S；0.34–0.46 突刺：颈猛地前探、头下压；0.46–0.5 咬住；之后缓缓收回
        coil = keys(t, [(0, 0), (0.34, 1), (0.44, 0), (0.8, 0)])
        strike = keys(t, [(0.32, 0), (0.44, 1), (0.52, 1), (0.9, 0)])
        pitch = list(REARING_PITCH)
        for index, (pull, thrust) in enumerate(zip((-52, -42, -26, -10, -3), (82, 62, 40, 18, 6))):
            pitch[index] += pull * coil + thrust * strike
        yaw = [y + strike * 4 * math.sin(i) for i, y in enumerate(REARING_YAW)]
        head_rot = (-16 * coil + 14 * strike, 0.0, 0.0)
        head_pos = (0.0, 4.0 * coil - 8.0 * strike, 12.0 * coil - 32.0 * strike)
        return snake_pose(rig, pitch, yaw, head_rot, head_pos)

    return Clip(0.9, False, sampler, ground_root="bone20")


def snake_death(rig) -> Clip:
    def sampler(t: float) -> Pose:
        # 身体从上往下依次垮塌：越靠近头，塌得越早
        def fallen(index: int) -> float:
            return keys(t, [(0.12 + 0.045 * index, 0), (0.62 + 0.045 * index, 1)])

        pitch = blend_profiles(REARING_PITCH, LIMP_PITCH, fallen)
        yaw = blend_profiles(REARING_YAW, LIMP_YAW, fallen)
        shock = keys(t, [(0, 0), (0.1, 1), (0.4, 0)])
        pitch = [p + shock * k for p, k in zip(pitch, (-30, -18, -8) + (0,) * 12)]
        # 塌平之后侧翻，肚皮朝天（白色腹面露出来），头软软地歪向一边
        roll = keys(t, [(0.95, 0), (1.5, 168)])
        head_rot = (0.0, 12 * keys(t, [(0.3, 0), (0.9, 1)]), roll)
        return snake_pose(rig, pitch, yaw, head_rot)

    return Clip(1.8, False, sampler, ground_root="bone20")


def snake_clips(rig) -> dict[str, Clip]:
    return {"attack": snake_attack(rig), "hurt": snake_hurt(rig), "death": snake_death(rig)}


# ---------------------------------------------------------------- 噬灵鼠（缺 hurt / death）
#
# 整只鼠很小（身长 11px、站高不到 8px），位移要用小数。骨骼：``body`` 是根，头 ``head``、
# 四腿 ``leg_fl/fr/bl/br``、尾 ``tail``（再接 bone→bone2→bone3 三节尾梢）都挂在它下面。
# 符号（探针）：body/head rotation.x 为正 = 低头；腿 rotation.x 为正 = 腿向后；尾 rotation.y 为正 = 尾尖朝 -x。
RAT_LEGS = ("leg_fl", "leg_fr", "leg_bl", "leg_br")
RAT_TAIL = ("tail", "bone", "bone2", "bone3")


def rat_hurt(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        put(pose, "body", pos=(0.0, keys(t, [(0, 0), (0.06, 0.7), (0.18, 0)]), keys(t, [(0, 0), (0.06, 1.3), (0.4, 0)])))
        put(pose, "body", rot=(keys(t, [(0, 0), (0.06, -12), (0.2, 4), (0.4, 0)]), 11 * math.sin(math.tau * 3.5 * t) * keys(t, [(0, 0), (0.05, 1), (0.4, 0)]), 0.0))
        put(pose, "head", rot=(keys(t, [(0, 0), (0.06, -26), (0.22, 6), (0.4, 0)]), 0.0, 0.0))
        # 前爪被惊得向后缩，后腿蹬一下，尾巴甩向一侧再荡回来
        for bone, kick in (("leg_fl", -18), ("leg_fr", -18), ("leg_bl", 16), ("leg_br", 16)):
            put(pose, bone, rot=(keys(t, [(0, 0), (0.07, kick), (0.24, -kick / 3), (0.4, 0)]), 0.0, 0.0))
        for index, bone in enumerate(RAT_TAIL):
            put(pose, bone, rot=(0.0, 14 * keys(t, [(0.04 + 0.025 * index, 0), (0.12 + 0.025 * index, 1), (0.34, -0.3), (0.4, 0)]), 0.0))
        return pose

    return Clip(0.4, False, sampler, ground_root="body")


def rat_death(rig) -> Clip:
    def sampler(t: float) -> Pose:
        pose: Pose = {}
        # 被打得原地一弹，侧翻倒地，四肢先蹬后僵直，尾巴软下来
        put(pose, "body", pos=(0.0, keys(t, [(0, 0), (0.1, 1.2), (0.3, 0)]), keys(t, [(0, 0), (0.1, 1.4), (0.4, 1.8), (1.2, 1.8)])))
        put(pose, "body", rot=(keys(t, [(0, 0), (0.1, -14), (0.45, 2), (1.2, 0)]), 0.0, keys(t, [(0, 0), (0.1, 10), (0.5, 88), (0.62, 80), (1.2, 82)])))
        put(pose, "head", rot=(keys(t, [(0, 0), (0.1, -24), (0.6, 12), (1.2, 18)]), keys(t, [(0.3, 0), (0.7, -16)]), 0.0))
        for index, bone in enumerate(RAT_LEGS):
            flail = 22 * math.sin(math.tau * (t * 3.6 + 0.25 * index)) * keys(t, [(0.05, 0), (0.2, 1), (0.6, 1), (0.85, 0)])
            stiff = keys(t, [(0, 0), (0.6, 0), (0.95, 1)])
            reach = (-30, -26, 34, 30)[index]  # 僵直时前腿向前伸、后腿向后伸
            put(pose, bone, rot=(flail + stiff * reach, 0.0, 0.0))
        for index, bone in enumerate(RAT_TAIL):
            put(pose, bone, rot=(0.0, 8 * keys(t, [(0.1, 0), (0.5 + 0.05 * index, 1), (1.2, 0.6)]), 0.0))
        return pose

    return Clip(1.2, False, sampler, ground_root="body")


SPECIES: dict[str, Callable[[object], dict[str, Clip]]] = {
    "devour_rat": lambda rig: {"hurt": rat_hurt(rig), "death": rat_death(rig)},
    "cockade_snake": snake_clips,
    "mandrake_snake": snake_clips,
    "green_spider": lambda rig: {"hurt": spider_hurt(rig), "death": spider_death(rig)},
    "blue_spider": lambda rig: {"hurt": spider_hurt(rig), "death": spider_death(rig)},
    "jungle_scorpion": lambda rig: {"hurt": jungle_hurt(rig)},
    "ice_scorpion": lambda rig: {"walk": ice_walk(rig), "hurt": ice_hurt(rig), "death": ice_death(rig)},
}


# ---------------------------------------------------------------- 命令行


def install(name: str) -> None:
    path = CLIENT / "animations" / f"{name}.animation.json"
    raw = path.read_text(encoding="utf-8")
    unit = indent_unit(raw)
    texture = CLIENT / "textures" / "entity" / "fauna" / f"{name}.png"
    with tempfile.TemporaryDirectory() as scratch:
        rig, _ = load_rig(name, CLIENT / "geo" / f"{name}.geo.json", texture, Path(scratch))
        clips = SPECIES[name](rig)

        def lift(pose: Pose) -> float:
            """让最低点贴地：入地就托起，悬空不管（受击跳起、尾巴甩起都是合法的悬空）。"""

            depth = rig.lowest(sample({"bones": pose}, rig, 0.0))
            return -depth if depth < 0 else 0.0

        full = {f"animation.bong.{name}.{short}": clip for short, clip in clips.items()}
        base = strip_generated(raw, list(full), unit)
        existing = json.loads(base)["animations"]
        clash = sorted(set(full) & set(existing))
        if clash:
            raise ValueError(f"{name}: 这些段已经有手工版本，本脚本不覆盖: {clash}")
        additions = {}
        for full_name, clip in full.items():
            bones = bake(clip, lift)
            check_ground(full_name, rig, bones, clip)
            additions[full_name] = clip_text(full_name, clip, bones, unit)
        path.write_text(append_clips(base, additions, unit), encoding="utf-8")
    print(f"{name}: " + ", ".join(f"{short} {clip.length}s{' loop' if clip.loop else ''}" for short, clip in clips.items()))


def check_ground(full_name: str, rig, bones: dict, clip: Clip) -> None:
    """烘完之后用「游戏会读的那份关键帧」再量一次最低点，入地超容差报错。"""

    profile = {"bones": bones}
    steps = max(2, round(clip.length / BAKE_STEP))
    for index in range(steps + 1):
        moment = clip.length * index / steps
        depth = rig.lowest(sample(profile, rig, min(moment, clip.length - 1e-6)))
        if depth < -GROUND_TOLERANCE:
            raise ValueError(f"{full_name}: t={moment:.2f}s 最低点 {depth:.2f}px，入地超过 {GROUND_TOLERANCE}px")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--only", choices=sorted(SPECIES), action="append")
    args = parser.parse_args()
    for name in args.only or SPECIES:
        install(name)


if __name__ == "__main__":
    main()


