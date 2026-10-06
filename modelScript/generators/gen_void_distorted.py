#!/usr/bin/env python3
"""渊空畸变体的逐部件 Blockbench 生成器。

本轮只实现建造计划的 ``01_spine_ribcage``：驼背骨脊、短骨刺和外露
肋笼。后续部件会在调度确认本轮形状后逐件加入，避免用一个整件模型掩盖
接缝、比例或材质的问题。

门禁：
  - ``_assert_no_coplanar_faces`` 拦截同平面重叠，避免渲染时闪烁；
  - ``--self-test`` 注入一个共面骨片，证明门禁确实能拒绝缺陷；
  - ``--part 01`` 生成单件正面、侧面、3/4 渲染和参考图对照卡。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
from pathlib import Path
import random
from typing import Iterable
import uuid

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "VoidDistorted.bbmodel"
PARTS_DIR = Path(
    "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/"
    "void_distorted/parts"
)
PARTS_REF_DIR = Path(
    "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/"
    "void_distorted/parts_ref"
)

RESOLUTION = 64
PALETTE = {
    "bone": [216, 204, 176],
    "bone_shadow": [184, 168, 136],
    "rust_dark": [74, 64, 56],
    "rust_light": [106, 90, 72],
    "rust_spot": [138, 96, 64],
    "flesh_red": [138, 42, 42],
    "flesh_dark": [90, 26, 26],
    "void_black": [14, 12, 12],
}

# 每种材质占贴图上的一个 16×16 区块。材质名和坐标都集中在这里，后续
# 部件只引用 PALETTE 中的名字，避免生成器里出现未审定的颜色字面量。
MATERIAL_UV = {
    "bone": [0, 0, 16, 16],
    "bone_shadow": [16, 0, 32, 16],
    "rust_dark": [32, 0, 48, 16],
    "rust_light": [48, 0, 64, 16],
    "rust_spot": [0, 16, 16, 32],
    "flesh_red": [16, 16, 32, 32],
    "flesh_dark": [32, 16, 48, 32],
    "void_black": [48, 16, 64, 32],
}


def _cube(
    name: str,
    low: tuple[float, float, float],
    high: tuple[float, float, float],
    material: str,
) -> dict:
    """创建一个带有语义名称的轴对齐骨片。"""
    if material not in PALETTE:
        raise ValueError(f"未审定的材质：{material}")
    if any(a >= b for a, b in zip(low, high)):
        raise ValueError(f"部件 {name} 的 from/to 无效：{low} -> {high}")
    return {
        "name": name,
        "from": list(low),
        "to": list(high),
        "group": "spine_ribcage",
        "material": material,
    }


# 脊椎：9 节首尾贴合的椎骨，沿 Z 从后（低）向前拱起，前端略回落；
# 前端之后由 02 号部件的头颅接上。相邻两节底面高差 ≤ 1.0 < 椎高，
# 保证上下错位时仍有侧面相接，不会断成悬空块。
VERTEBRA_BOTTOMS = [11.0, 11.8, 12.8, 13.8, 14.8, 15.6, 16.2, 16.4, 16.0]
VERTEBRA_Z_REAR = -9.0
VERTEBRA_LENGTH = 2.0
VERTEBRA_HEIGHT = 2.0
SPIKE_HEIGHTS = [2.0, 2.6, 2.2, 3.0, 2.4, 2.8, 2.2, 3.0, 2.0]


def _vertebra_z(index: int) -> tuple[float, float]:
    z0 = VERTEBRA_Z_REAR + index * VERTEBRA_LENGTH
    return z0, z0 + VERTEBRA_LENGTH


def _vertebrae() -> list[dict]:
    """骨椎宽窄交替（±1.0 / ±1.2），节与节之间不留缝。"""
    cubes = []
    for index, bottom in enumerate(VERTEBRA_BOTTOMS):
        z0, z1 = _vertebra_z(index)
        half_width = 1.0 if index % 2 == 0 else 1.2
        cubes.append(
            _cube(
                f"spine_vertebra_{index:02d}",
                (-half_width, bottom, z0),
                (half_width, bottom + VERTEBRA_HEIGHT, z1),
                "bone" if index % 2 == 0 else "bone_shadow",
            )
        )
    return cubes


def _spine_spikes() -> list[dict]:
    """1×1 的骨刺立在每节椎骨顶面中线上，高度参差。"""
    cubes = []
    for index, bottom in enumerate(VERTEBRA_BOTTOMS):
        z0, z1 = _vertebra_z(index)
        zc = (z0 + z1) / 2
        top = bottom + VERTEBRA_HEIGHT
        cubes.append(
            _cube(
                f"spine_spike_{index:02d}",
                (-0.5, top, zc - 0.5),
                (0.5, top + SPIKE_HEIGHTS[index], zc + 0.5),
                "bone",
            )
        )
    return cubes


def _mirrored_box(
    name: str,
    x_range: tuple[float, float],
    y_range: tuple[float, float],
    z_range: tuple[float, float],
    material: str,
) -> list[dict]:
    """x_range 取正值，生成 left / right 两个镜像立方体。"""
    return [
        _cube(f"{name}_l", (-x_range[1], y_range[0], z_range[0]), (-x_range[0], y_range[1], z_range[1]), material),
        _cube(f"{name}_r", (x_range[0], y_range[0], z_range[0]), (x_range[1], y_range[1], z_range[1]), material),
    ]


# 肋骨：每根是一条 0.8×0.8 的细骨条，3 段拼成「⊃」形——
# A 从椎骨侧面水平伸出，B 在外端垂直下挂，C 在 B 的底端向内钩回。
# 6 对肋骨长在第 2~7 节椎骨上（7 是最靠近头部的一节），沿脊椎方向相邻肋骨之间
# 的空隙 = 椎骨节距 − 肋宽 = 1.2，侧面能透过去。
RIB_SECTION = 0.8
RIB_OUT_LENGTH = 2.0
RIB_VERTEBRA_INDICES = range(2, 8)
RIB_DROP_REAR = 3.0
RIB_DROP_STEP = 0.2


def _ribs() -> list[dict]:
    """左右各 6 根「⊃」形细肋，越靠前（头部方向）垂得越长。"""
    cubes = []
    for index in RIB_VERTEBRA_INDICES:
        z0, z1 = _vertebra_z(index)
        zc = (z0 + z1) / 2
        z_range = (zc - RIB_SECTION / 2, zc + RIB_SECTION / 2)
        half_width = 1.0 if index % 2 == 0 else 1.2
        outer = half_width + RIB_OUT_LENGTH
        top = VERTEBRA_BOTTOMS[index] + VERTEBRA_HEIGHT - 0.4
        drop = RIB_DROP_REAR + RIB_DROP_STEP * (index - RIB_VERTEBRA_INDICES.start)
        material = "bone" if index % 2 == 0 else "bone_shadow"
        down_top = top - RIB_SECTION
        down_bottom = down_top - drop
        cubes += _mirrored_box(
            f"rib_{index}_out", (half_width, outer), (down_top, top), z_range, material
        )
        cubes += _mirrored_box(
            f"rib_{index}_down",
            (outer - RIB_SECTION, outer),
            (down_bottom, down_top),
            z_range,
            material,
        )
        cubes += _mirrored_box(
            f"rib_{index}_hook",
            (outer - 2 * RIB_SECTION, outer - RIB_SECTION),
            (down_bottom, down_bottom + RIB_SECTION),
            z_range,
            material,
        )
    return cubes


def part_spine_ribcage() -> list[dict]:
    """01 部件：连续拱形骨脊 + 脊顶骨刺 + 左右各 6 根「⊃」形细肋。

    坐标：X 左右、Y 向上、Z 朝向生物正面（前高后低）。对照
    parts_ref/01_spine_ribcage.png。红肉条属于 03 号部件，锈甲属于 04 号，
    本件不含；肋骨之间的空隙留给 03 的肉条。
    """
    return _vertebrae() + _spine_spikes() + _ribs()


def _tag(cubes: list[dict], group: str) -> list[dict]:
    """把一个部件的所有立方体归到同一个 outliner 分组。"""
    for cube in cubes:
        cube["group"] = group
    return cubes


# 02 号部件挂在脊椎前端（最后一节椎骨的 +Z 面，z=9）。
MAW_FRONT_Z = VERTEBRA_Z_REAR + VERTEBRA_LENGTH * len(VERTEBRA_BOTTOMS)

# 虚空口按 7×7 网格摆：格边长 1.2，中心 (x=0, y=MAW_CENTER_Y)。
# 三圈由外到内——外圈（max(|i|,|j|)==3，亮红）、中圈（==2，暗红）、内圈（==1，暗红）——
# 前缘依次后退 MAW_STEP，中心一格是虚空黑，退得最深，所以侧视是往里收的台阶漏斗。
MAW_CELL = 1.2
MAW_CENTER_Y = 13.5
MAW_STEP = 0.9
MAW_OUTER_FRONT = 2.7  # 外圈前缘离 z0 的距离
# 外圈不规则：四个角整格缺失，每边再缺 1~2 格；个别格子前缘多凸 / 少凸（格坐标 → 增量）。
MAW_OUTER_MISSING = {
    (-3, 3), (3, 3), (3, -3), (-3, -3),
    (-3, 1), (2, 3), (3, -1), (-1, -3), (1, -3),
}
MAW_OUTER_DEPTH_SKEW = {(-3, -1): 0.4, (3, 2): 0.4, (-2, 3): -0.3, (3, 1): -0.25, (0, -3): 0.3}
# 外圈下垂的肉须：(格 i, 须宽, 长度)，从最下一行外圈格子底下垂。须宽 0.6~0.8。
MAW_WHISKERS = [(-2, 0.7, 2.2), (0, 0.6, 1.2), (1, 0.8, 3.0), (2, 0.6, 1.7)]


def _cell_box(
    name: str, i: int, j: int, z_back: float, z_front: float, material: str
) -> dict:
    x0 = i * MAW_CELL - MAW_CELL / 2
    y0 = MAW_CENTER_Y + j * MAW_CELL - MAW_CELL / 2
    return _cube(name, (x0, y0, z_back), (x0 + MAW_CELL, y0 + MAW_CELL, z_front), material)


def _maw_funnel() -> list[dict]:
    """往里收的三圈血肉漏斗，中心虚空黑；外圈参差、下垂细肉须。"""
    z0 = MAW_FRONT_Z
    cubes = []
    for ring, front, material in (
        (3, MAW_OUTER_FRONT, "flesh_red"),
        (2, MAW_OUTER_FRONT - MAW_STEP, "flesh_dark"),
        (1, MAW_OUTER_FRONT - 2 * MAW_STEP, "flesh_dark"),
    ):
        for i in range(-ring, ring + 1):
            for j in range(-ring, ring + 1):
                if max(abs(i), abs(j)) != ring:
                    continue
                if ring == 3 and (i, j) in MAW_OUTER_MISSING:
                    continue
                # 中圈四个角也缺掉，让内部轮廓偏圆。
                if ring == 2 and abs(i) == 2 and abs(j) == 2:
                    continue
                skew = MAW_OUTER_DEPTH_SKEW.get((i, j), 0.0) if ring == 3 else 0.0
                cubes.append(_cell_box(f"maw_r{ring}_{i}_{j}", i, j, z0, z0 + front + skew, material))
    # 中心虚空黑：前缘比内圈再退 MAW_STEP，只比 z0 高一点点。
    cubes.append(
        _cell_box("maw_void", 0, 0, z0 - 0.4, z0 + MAW_OUTER_FRONT - 3 * MAW_STEP, "void_black")
    )
    # 细肉须：从最下一行外圈格子底边垂下，长短不一、宽 0.6~0.8。
    bottom = MAW_CENTER_Y - 3.5 * MAW_CELL
    for index, (i, width, length) in enumerate(MAW_WHISKERS):
        x_center = i * MAW_CELL
        z_mid = z0 + MAW_OUTER_FRONT / 2
        cubes.append(
            _cube(
                f"maw_whisker_{index}",
                (x_center - width / 2, bottom - length, z_mid - width / 2),
                (x_center + width / 2, bottom, z_mid + width / 2),
                "flesh_red" if index % 2 == 0 else "flesh_dark",
            )
        )
    return cubes


SKULL_INNER_X = 4.0  # 颅骨内缘到中轴的距离；比外圈最宽处（4.2）略小，让颅骨压住肉圈边缘
SKULL_WIDTH = 3.8
SKULL_Z0 = MAW_FRONT_Z - 0.2
SKULL_Z1 = SKULL_Z0 + 2.6

# 颅骨自下而上的横截面：(y0, y1, 内缩, 外缩, 前缘后退)。内缩/外缩是相对
# 颅骨最宽处的收进量，前三级（顶部）逐级收圆；颧骨那一行向外多凸 0.3（负外缩）。
SKULL_ROWS = [
    ("cheek", 11.8, 12.6, 0.5, 0.2, 0.3),
    ("zygoma", 12.6, 13.4, 0.0, -0.3, 0.0),
    ("brow", 13.4, 14.6, 0.0, 0.0, 0.0),
    ("upper", 14.6, 15.4, 0.3, 0.3, 0.0),
    ("dome1", 15.4, 16.1, 0.7, 0.7, 0.2),
    ("dome2", 16.1, 16.6, 1.2, 1.2, 0.4),
]
# 眼窝在颅骨宽度上的位置（离内缘的距离范围）；中间 1.1 到 1.8 是鼻梁。
SOCKET_RANGES = [(0.5, 1.5), (2.3, 3.3)]
SOCKET_BOTTOM = 13.5
SOCKET_RECESS = 0.8  # 眼窝黑块比颅骨前面缩进的深度


def _skull(side: str, sign: int) -> list[dict]:
    """真骷髅：颅顶三级收圆、颧骨外凸、深黑眼窝带骨缘、倒三角鼻孔、一排参差牙齿。

    骨色以 bone 为主，bone_shadow 做斑驳，颅缝用 rust_light 细线。颅骨压在肉圈边缘上，
    再用两块暗红肉连到肉圈，不是并排摆的三块东西。右颗比左颗略高。
    """
    lift = 0.25 if sign > 0 else 0.0
    inner = SKULL_INNER_X
    outer = SKULL_INNER_X + SKULL_WIDTH

    def box(name, xs, ys, zs, material):
        lo, hi = (xs[0], xs[1]) if sign > 0 else (-xs[1], -xs[0])
        return _cube(f"skull_{side}_{name}", (lo, ys[0] + lift, zs[0]), (hi, ys[1] + lift, zs[1]), material)

    z0, z1 = SKULL_Z0, SKULL_Z1
    cubes = []
    for name, y0, y1, in_cut, out_cut, back in SKULL_ROWS:
        material = "bone_shadow" if name in ("cheek", "dome2") else "bone"
        x0, x1 = inner + in_cut, outer - out_cut
        if name != "brow":
            cubes.append(box(name, (x0, x1), (y0, y1), (z0, z1 - back), material))
            continue
        # 眉弓那一行被两个眼窝切开：外缘、鼻梁、内缘三块骨，眼窝里嵌深黑块。
        (a0, a1), (b0, b1) = ((inner + lo, inner + hi) for lo, hi in SOCKET_RANGES)
        cubes += [
            box("brow_inner", (x0, a0), (y0, y1), (z0, z1), "bone"),
            box("brow_bridge", (a1, b0), (y0, y1), (z0, z1), "bone_shadow"),
            box("brow_outer", (b1, x1), (y0, y1), (z0, z1), "bone"),
        ]
        for index, (s0, s1) in enumerate(((a0, a1), (b0, b1))):
            cubes.append(box(f"socket_{index}", (s0, s1), (SOCKET_BOTTOM, y1), (z0, z1 - SOCKET_RECESS), "void_black"))
            # 骨缘：眼窝上沿和外沿各压一条凸出的细骨，让眼窝有边。
            cubes.append(box(f"socket_{index}_rim_top", (s0 - 0.15, s1 + 0.15), (y1 - 0.15, y1 + 0.2), (z1 - 0.1, z1 + 0.2), "bone_shadow"))
            cubes.append(box(f"socket_{index}_rim_low", (s0 - 0.15, s1 + 0.15), (SOCKET_BOTTOM - 0.25, SOCKET_BOTTOM), (z1 - 0.1, z1 + 0.15), "bone_shadow"))

    # 倒三角鼻孔：贴在颧骨行前面的两级黑块，上宽下窄。
    nose_c = inner + (SOCKET_RANGES[0][1] + SOCKET_RANGES[1][0]) / 2
    cubes.append(box("nose_top", (nose_c - 0.45, nose_c + 0.45), (12.95, 13.4), (z1, z1 + 0.12), "void_black"))
    cubes.append(box("nose_bottom", (nose_c - 0.2, nose_c + 0.2), (12.6, 12.95), (z1, z1 + 0.12), "void_black"))

    # 骨色斑驳与颅缝：不同深浅的小块贴在颅骨前面，位置左右各异。
    shift = 0.0 if sign > 0 else 0.6
    for index, (px, py, pw, ph, material) in enumerate(
        [
            (0.2, 14.8, 0.8, 0.5, "bone_shadow"),
            (2.6, 15.6, 0.7, 0.4, "bone_shadow"),
            (1.65, 13.5, 0.4, 0.3, "rust_light"),
            (3.0, 12.7, 0.6, 0.5, "bone_shadow"),
        ]
    ):
        px = min(px + (0.0 if index == 2 else shift), SKULL_WIDTH - pw - 0.1)
        cubes.append(box(f"patch_{index}", (inner + px, inner + px + pw), (py, py + ph), (z1 - 0.05, z1 + 0.1), material))
    suture = inner + SKULL_WIDTH / 2
    cubes.append(box("suture", (suture - 0.1, suture + 0.1), (15.5, 16.0), (z1 - 0.25, z1 - 0.1), "rust_light"))

    # 一排参差牙齿：粗细不一、长短不一，有断齿。
    teeth = [(0.6, 0.45, 0.9), (1.1, 0.35, 0.5), (1.55, 0.5, 1.1), (2.15, 0.35, 0.6), (2.6, 0.45, 1.0), (3.1, 0.35, 0.4)]
    for index, (tx, width, length) in enumerate(teeth):
        cubes.append(
            box(
                f"tooth_{index}",
                (inner + tx, inner + tx + width),
                (11.8 - length, 11.8),
                (z1 - 1.2, z1 - 0.7),
                "bone" if index % 2 == 0 else "bone_shadow",
            )
        )
    return cubes


def _sinews() -> list[dict]:
    """暗红肉把颅骨和肉圈连起来：上下各一条，从肉圈边缘搭到颅骨内侧。"""
    z0 = MAW_FRONT_Z
    cubes = []
    for side, sign in (("l", -1), ("r", 1)):
        for name, y0, y1 in (("low", 11.5, 12.0), ("high", 14.55, 15.15)):
            lo, hi = (3.8, 5.2) if sign > 0 else (-5.2, -3.8)
            cubes.append(_cube(f"sinew_{side}_{name}", (lo, y0, z0 + 0.4), (hi, y1, z0 + 1.8), "flesh_dark"))
    return cubes


def part_void_maw_skulls() -> list[dict]:
    """02 部件：层层收拢的血肉漏斗 + 左右各一颗真骷髅，暗红肉连成一体，挂在脊椎前端。

    对照 parts_ref/02_void_maw_skulls.png。红肉条下垂（03）和骨冠不在本件内。
    """
    cubes = _maw_funnel() + _skull("l", -1) + _skull("r", 1) + _sinews()
    return _tag(cubes, "void_maw_skulls")


# 03 号部件：从脊椎腹面垂下的细红肉条，长短不一，夹几块骨片。
# 每节椎骨下方有两个 z 槽、6 列 x，每个格子一条肉条；各椎骨底下先横一根短肉梁
# 把外侧几列和脊椎接牢（外侧列的正上方没有骨头，没有肉梁就会悬空）。
STRAND_X_COLUMNS = [-2.4, -1.45, -0.5, 0.45, 1.4, 2.35]
STRAND_Z_OFFSETS = (0.1, 1.05)  # 相对椎骨起点 z0 的两个槽
STRAND_GROUND_CLEARANCE = 2.3  # 肉条最低端不低于这个高度，给后腿和前臂留出落地空间
BEAM_HEIGHT = 0.7
# 肉条分三档：多数短而粗，一部分中等，少数细长到底。(长度范围, 宽度范围, 占比上限)
STRAND_SHORT = ((2.0, 4.0), (0.7, 0.8))
STRAND_MEDIUM = ((4.0, 6.0), (0.55, 0.7))
STRAND_LONG = ((8.0, 11.0), (0.4, 0.5))
STRAND_SHORT_SHARE = 0.65
STRAND_MEDIUM_SHARE = 0.88  # 累计占比；剩下约 12% 是长条
# 长条末端：最后 LONG_TAIL_FRACTION 收成细丝，丝尾挂一颗滴状肉球。
LONG_TAIL_FRACTION = 0.25
LONG_TAIL_WIDTH = 0.3
DRIP_SIZE = (0.75, 0.9)  # 肉球的 水平边长, 高
# 肉带上沿呈浅 V：|x| 超过 BAND_FLAT_HALF_WIDTH 后每向外 1 格上翘 BAND_V_SLOPE。
BAND_FLAT_HALF_WIDTH = 0.5
BAND_V_SLOPE = 0.5
BAND_JITTER = 0.15  # 每块肉带高度的随机起伏，同时避免相邻块顶面共面
BAND_OVERLAP = 0.12  # 相邻肉带块在 x 方向互相压住，不留缝
# 大骨块：(椎骨序号, z 槽序号, 列序号, 朝向)。朝向 out = 向身体外侧凸出（侧视和正视都能看到），
# rear = 向后（-z）凸出，后视能看到。块头约 1.5~2 格，露在肉条外面。
STRAND_BONE_CHUNKS = [
    (0, 0, 0, "out"),
    (1, 1, 5, "out"),
    (2, 0, 0, "out"),
    (3, 1, 5, "out"),
    (4, 0, 5, "out"),
    (5, 1, 0, "out"),
    (6, 0, 5, "out"),
    (0, 0, 3, "rear"),
]


def _flesh_strands() -> list[dict]:
    """脊椎腹面一条起伏的 V 形肉带 + 垂下的长短悬殊的肉条 + 露在外面的大骨块。"""
    rng = random.Random(3)
    chunk_specs = {(v, s, c): facing for v, s, c, facing in STRAND_BONE_CHUNKS}
    cubes = []
    for vertebra, bottom in enumerate(VERTEBRA_BOTTOMS):
        z0, _ = _vertebra_z(vertebra)
        for slot, z_offset in enumerate(STRAND_Z_OFFSETS):
            band_z = z0 + z_offset
            band_width = 0.95 - 0.02 * slot
            for column, x_nominal in enumerate(STRAND_X_COLUMNS):
                x_center = x_nominal + rng.uniform(-0.04, 0.04)
                v_lift = BAND_V_SLOPE * max(0.0, abs(x_nominal) - BAND_FLAT_HALF_WIDTH)
                lift = v_lift + rng.uniform(0.0, BAND_JITTER)
                band_bottom = bottom - BEAM_HEIGHT + lift
                band_top = bottom + 0.3 + lift
                z_jitter = rng.uniform(0.0, 0.05)  # 相邻肉带块前后面错开，避免共面
                half_pitch = (STRAND_X_COLUMNS[1] - STRAND_X_COLUMNS[0]) / 2 + BAND_OVERLAP
                tag = f"{vertebra}_{slot}_{column}"
                cubes.append(
                    _cube(
                        f"strand_band_{tag}",
                        (x_nominal - half_pitch, band_bottom, band_z + z_jitter),
                        (x_nominal + half_pitch, band_top, band_z + band_width - z_jitter),
                        "flesh_dark",
                    )
                )

                roll = rng.random()
                if roll < STRAND_SHORT_SHARE:
                    (length_lo, length_hi), (width_lo, width_hi) = STRAND_SHORT
                    kind = "short"
                elif roll < STRAND_MEDIUM_SHARE:
                    (length_lo, length_hi), (width_lo, width_hi) = STRAND_MEDIUM
                    kind = "medium"
                else:
                    (length_lo, length_hi), (width_lo, width_hi) = STRAND_LONG
                    kind = "long"
                width = rng.uniform(width_lo, width_hi)
                length = rng.uniform(length_lo, length_hi)
                top = band_bottom + 0.15
                length = min(length, top - STRAND_GROUND_CLEARANCE)
                z_start = band_z + (band_width - width) / 2
                material = "flesh_red" if rng.random() < 0.55 else "flesh_dark"
                other = "flesh_dark" if material == "flesh_red" else "flesh_red"

                if kind != "long":
                    cubes.append(
                        _cube(
                            f"strand_{tag}",
                            (x_center - width / 2, top - length, z_start),
                            (x_center + width / 2, top, z_start + width),
                            material,
                        )
                    )
                else:
                    body_bottom = top - length * (1 - LONG_TAIL_FRACTION)
                    thread_bottom = top - length
                    thread_inset = (width - LONG_TAIL_WIDTH) / 2
                    cubes.append(
                        _cube(
                            f"strand_{tag}",
                            (x_center - width / 2, body_bottom, z_start),
                            (x_center + width / 2, top, z_start + width),
                            material,
                        )
                    )
                    cubes.append(
                        _cube(
                            f"strand_thread_{tag}",
                            (x_center - LONG_TAIL_WIDTH / 2, thread_bottom, z_start + thread_inset),
                            (x_center + LONG_TAIL_WIDTH / 2, body_bottom + 0.1, z_start + thread_inset + LONG_TAIL_WIDTH),
                            other,
                        )
                    )
                    drip_side, drip_height = DRIP_SIZE
                    z_mid = z_start + width / 2
                    cubes.append(
                        _cube(
                            f"strand_drip_{tag}",
                            (x_center - drip_side / 2, thread_bottom - drip_height + 0.2, z_mid - drip_side / 2),
                            (x_center + drip_side / 2, thread_bottom + 0.2, z_mid + drip_side / 2),
                            "flesh_red",
                        )
                    )

                facing = chunk_specs.get((vertebra, slot, column))
                if facing:
                    cubes += _bone_chunk(tag, facing, x_nominal, x_center, band_bottom, band_top, band_z, band_width, rng)
    return cubes


def _bone_chunk(
    tag: str,
    facing: str,
    x_nominal: float,
    x_center: float,
    band_bottom: float,
    band_top: float,
    band_z: float,
    band_width: float,
    rng: random.Random,
) -> list[dict]:
    """成团的米色大骨块（亮面 + 一块暗面），压在肉带上并朝外凸出，不会被肉条盖住。"""
    size = rng.uniform(1.5, 1.9)
    height = rng.uniform(1.6, 2.0)
    y_top = band_top + 0.35
    y_bottom = y_top - height
    z_mid = band_z + band_width / 2
    if facing == "out":
        sign = 1.0 if x_nominal > 0 else -1.0
        inner = x_center - sign * 0.3
        outer = x_center + sign * size
        x_lo, x_hi = min(inner, outer), max(inner, outer)
        z_lo, z_hi = z_mid - size * 0.45, z_mid + size * 0.45
        shade = (x_lo, x_hi - size * 0.4, y_bottom, y_bottom + height * 0.45, z_lo, z_hi) if sign > 0 else (
            x_lo + size * 0.4, x_hi, y_bottom, y_bottom + height * 0.45, z_lo, z_hi
        )
    else:
        x_lo, x_hi = x_center - size * 0.45, x_center + size * 0.45
        z_hi = z_mid + 0.3
        z_lo = z_mid - size
        shade = (x_lo, x_hi, y_bottom, y_bottom + height * 0.45, z_lo + size * 0.4, z_hi)
    main = _cube(f"strand_bone_{tag}", (x_lo, y_bottom, z_lo), (x_hi, y_top, z_hi), "bone")
    # 暗面块比亮面略宽略短，贴在块的下半部，露出一圈暗边，避免与亮面共面。
    sx0, sx1, sy0, sy1, sz0, sz1 = shade
    dark = _cube(
        f"strand_bone_shade_{tag}",
        (sx0 - 0.1, sy0 - 0.15, sz0 - 0.1),
        (sx1 + 0.1, sy1, sz1 + 0.1),
        "bone_shadow",
    )
    return [main, dark]


def part_flesh_strands() -> list[dict]:
    """03 部件：V 形肉带下垂挂长短悬殊的肉条，带滴状肉球，嵌 8 块大骨块。

    对照 parts_ref/03_flesh_strands.png。肉带随脊椎前高后低倾斜（侧视是斜坡），
    最低端离地不少于 STRAND_GROUND_CLEARANCE。
    """
    return _tag(_flesh_strands(), "flesh_strands")


# 04 号部件：沿脊椎两侧一片压一片的瓦状甲片。
# 每侧 PLATE_ROWS 行，行从脊线向外铺：每行比上一行外移 PLATE_ROW_PITCH、下沉
# PLATE_ROW_DROP，相邻行在 x 上压住。每片甲沿 z 一节椎骨一片，由两块板拼成——
# 内半较高、外半低 PLATE_TILT，侧面看是向外下翻的斜面；外缘再压一道锈褐边。
# 内缘从 x=PLATE_INNER_X 起，落在椎骨宽度（≤1.2）之内，所以紧贴脊椎、不留空沟。
PLATE_ROWS = 5
PLATE_INNER_X = 0.9
PLATE_ROW_PITCH = 1.5
PLATE_WIDTH = 2.1  # 大于行距，相邻行 x 上压住 0.6
PLATE_ROW_DROP = 0.7
PLATE_TOP_OFFSET = 1.7  # 第一行内半顶面相对椎骨底面的高度，略低于脊椎顶面
PLATE_THICKNESS = 1.0  # 大于行落差 0.7，上下行在 y 上压住 0.3，不悬空
PLATE_TILT = 0.35  # 同一片外半比内半低多少
PLATE_RIM_WIDTH = 0.35
PLATE_JITTER = 0.08
GAP_CHANCE = 0.15  # 某片缩短，让出 0.5 的缝，缝里露暗红肉
PEEK_CHANCE = 0.4  # 某片内缘下方露一小块暗红肉
OUTER_ROW_CHANCE = 0.7  # 最外一行只在部分节上出现，边缘参差


def _plating() -> list[dict]:
    rng = random.Random(5)
    cubes = []
    for vertebra, bottom in enumerate(VERTEBRA_BOTTOMS):
        z0, z1 = _vertebra_z(vertebra)
        for side, sign in (("l", -1.0), ("r", 1.0)):
            for row in range(PLATE_ROWS):
                if row == PLATE_ROWS - 1 and rng.random() > OUTER_ROW_CHANCE:
                    continue
                tag = f"{side}_{vertebra}_{row}"
                x_in = PLATE_INNER_X + row * PLATE_ROW_PITCH + rng.uniform(0.0, PLATE_JITTER)
                x_out = x_in + PLATE_WIDTH + rng.uniform(-PLATE_JITTER, PLATE_JITTER)
                x_mid = (x_in + x_out) / 2
                top = bottom + PLATE_TOP_OFFSET - row * PLATE_ROW_DROP + rng.uniform(0.0, PLATE_JITTER)
                short = rng.random() < GAP_CHANCE
                plate_z1 = z1 - (0.5 if short else 0.0) + rng.uniform(0.0, PLATE_JITTER)
                plate_z0 = z0 + rng.uniform(0.0, PLATE_JITTER)
                body = "rust_light" if rng.random() < 0.75 else "rust_dark"

                def slab(name, xa, xb, y_top, material, inset=0.0, z_pad=0.0):
                    lo, hi = sorted((sign * xa, sign * xb))
                    return _cube(
                        f"plate_{name}_{tag}",
                        (lo, y_top - PLATE_THICKNESS + inset, plate_z0 + inset - z_pad),
                        (hi, y_top + (0.04 if inset else 0.0), plate_z1 - inset + z_pad),
                        material,
                    )

                outer_top = top - PLATE_TILT
                cubes.append(slab("inner", x_in, x_mid + 0.1, top, body))
                cubes.append(slab("outer", x_mid, x_out, outer_top, body, z_pad=0.04))  # 比内半略宽，前后面不共面
                # 外缘锈褐边：略出头、略缩进，不与外半板共面。
                cubes.append(slab("rim", x_out - PLATE_RIM_WIDTH, x_out + 0.1, outer_top, "rust_spot", inset=0.05))
                # 大锈斑：2~3 块不同大小的薄块叠成不规则一团，高度各不相同。
                spot_x = rng.uniform(x_in + 0.2, x_mid - 0.2)
                spot_z = rng.uniform(plate_z0 + 0.2, plate_z1 - 0.9)
                for blot in range(rng.randint(2, 3)):
                    width = rng.uniform(0.5, 0.95)
                    depth = rng.uniform(0.4, 0.8)
                    dx = rng.uniform(-0.3, 0.3)
                    dz = rng.uniform(-0.2, 0.35)
                    xa, xb = sorted((sign * (spot_x + dx), sign * (spot_x + dx + width)))
                    cubes.append(
                        _cube(
                            f"plate_blot_{tag}_{blot}",
                            (xa, top - 0.05 - 0.02 * blot, spot_z + dz),
                            (xb, top + 0.05 + 0.02 * (blot + 1), spot_z + dz + depth),
                            "rust_spot",
                        )
                    )
                if short:
                    # 缩短的片留出的缝里塞暗红肉，顶面比板面低 0.5。
                    lo, hi = sorted((sign * x_in, sign * x_out))
                    cubes.append(
                        _cube(f"plate_gap_flesh_{tag}", (lo, top - 1.2, plate_z1 + 0.05), (hi, top - 0.4, z1 + 0.3 + 0.03 * row), "flesh_dark")
                    )
                if rng.random() < PEEK_CHANCE:
                    # 板内缘下方露一小块暗红肉，从侧面和下方能看到。
                    lo, hi = sorted((sign * (x_in + 0.1), sign * (x_in + 0.8)))
                    cubes.append(
                        _cube(
                            f"plate_peek_flesh_{tag}",
                            (lo, top - PLATE_THICKNESS - 0.5, plate_z0 + 0.3),
                            (hi, top - PLATE_THICKNESS + 0.3, plate_z1 - 0.3),
                            "flesh_dark",
                        )
                    )
    return cubes


def part_back_plating() -> list[dict]:
    """04 部件：脊椎两侧每侧 5 行、一片压一片的瓦状锈铁甲，向外下斜铺，缝里露暗红肉。

    对照 parts_ref/04_back_plating.png。内缘压住脊椎侧面，甲面主色 rust_light，
    外缘锈褐边 + 大锈斑；外斜骨刺属于 01，本件不做。
    """
    return _tag(_plating(), "back_plating")


# 05 号部件：两条肩宽腕窄、肘部弯折前倾的两段前臂，瓦状叠甲，末端宽掌 + 粗长内勾骨爪。
# 臂是 ARM_BAND_COUNT 节自上而下收窄的甲环，相邻节在 y 上压住。上臂每节略向后（-z），
# 到 ELBOW_BAND 节是肘（最靠后、略鼓），前臂每节向前（+z）FOREARM_Z_STEP，侧视呈「>」形。
# 每节正面压两片上下错开的前甲（下沿锈褐边 + 锯齿），外侧挑一片外翻甲角；肩顶斜刺一簇，
# 肩内侧垂一帘暗红肉丝（做法同 03：长条收细成丝、丝尾挂肉球）；手腕下是宽骨掌和四根内勾粗爪。
ARM_CENTER_X = 6.0
ARM_CENTER_Z = 4.5
ARM_TOP_Y = 15.0  # 肩顶，藏在 04 外侧甲片下面
ARM_BAND_COUNT = 11
ARM_BAND_STEP = 1.1  # 相邻两节顶面的落差
ARM_BAND_HEIGHT = 1.45  # 大于落差，节间在 y 上压住 0.35
ARM_WIDTH_TOP = 4.4
ARM_WIDTH_SHRINK = 0.2  # 肩 4.4 → 腕 2.4，肩宽约为腕宽的 1.8 倍
ARM_DEPTH_TOP = 3.6
ARM_DEPTH_SHRINK = 0.14
ELBOW_BAND = 5
ELBOW_BULGE = 0.4
UPPER_Z_STEP = -0.2  # 上臂每节向后挪
FOREARM_Z_STEP = 0.5  # 前臂每节向前挪；0.5 / 1.1 约 24° 前倾
ARM_JITTER = 0.08
SHOULDER_SPIKE_LEAN = 0.5  # 肩刺每升一级向外挪多少，约 29°
SHOULDER_SPIKE_BASES = ((-1.2, -0.6, 3), (-0.5, 0.5, 3), (0.2, -0.5, 2), (0.9, 0.4, 3), (1.6, -0.2, 2))
SIDE_SPIKE_BANDS = (0, 2, 4, 7, 9)
FLESH_LENGTHS = (2.5, 6.5, 3.5, 7.5, 3.0, 5.0, 2.8)
FLESH_LONG_FROM = 6.0
CURTAIN_Z_START = -1.5
CURTAIN_Z_PITCH = 0.5
# 爪：宽厚骨掌 + 4 根三节指。每指第 1 节平伸向前、第 2 节下折（约 35°，用台阶近似）、
# 第 3 节更陡地下勾（约 60°+）并收细成钩尖；外侧（朝身体中线）一根略短当拇指。
# 掌比腕宽（腕约 2.4）近一倍，所以掌上沿压进最后两节腕环里，掌侧露在腕外像一圈护腕。
PALM_WIDTH = 5.0
PALM_DEPTH = 2.6
PALM_HEIGHT = 1.5
PALM_BOTTOM_Y = 2.75
FINGER_X = (-1.875, -0.625, 0.625, 1.875)  # 局部 x，负值朝身体中线；第 0 根是拇指
FINGER_WIDTH = 1.1
FINGER_TOP_Y = 3.3
FINGER_THICKNESS = 1.2  # 第 1 节的高
SEG1_LEN = 1.9
SEG2_LEN = 1.7
SEG2_Z_OVERLAP = 0.5  # 第 2 节在 z 上压住第 1 节前端
SEG2_DROP = 1.1  # 第 2 节整体比第 1 节低多少
SEG3_TOP_Y = 1.3
SEG3_WIDTH = 0.6
SEG3_Z_LEN = 0.8
SEG3_Z_OVERLAP = 0.3
TIP_HEIGHT = 0.45  # 最末一小截（略暗），落到 y=0
SEG2_SPREAD = 0.1  # 外侧指第 2 节向外散开
SEG3_INWARD = 0.3  # 外侧指第 3 节向手心内勾
THUMB_REACH = 0.7  # 拇指各节前伸长度的比例
SEAM_PAD = 0.05  # 节间暗缝比指身宽出多少
SEAM_HEIGHT = 0.2


def _arm(side: str, sign: float, rng: random.Random) -> list[dict]:
    cx = sign * ARM_CENTER_X
    cubes = []

    def box(name, x, y, z, material, dz=0.0):
        """x 以臂中线为原点、朝外为正，自动按 sign 镜像；z 相对 ARM_CENTER_Z + dz。"""
        xa, xb = sorted((cx + sign * x[0], cx + sign * x[1]))
        return _cube(f"arm_{side}_{name}", (xa, y[0], ARM_CENTER_Z + dz + z[0]), (xb, y[1], ARM_CENTER_Z + dz + z[1]), material)

    def z_offset(k: int) -> float:
        if k <= ELBOW_BAND:
            return UPPER_Z_STEP * k
        return UPPER_Z_STEP * ELBOW_BAND + FOREARM_Z_STEP * (k - ELBOW_BAND)

    inner_face = 0.0
    for k in range(ARM_BAND_COUNT):
        dz = z_offset(k)
        half_w = (ARM_WIDTH_TOP - ARM_WIDTH_SHRINK * k) / 2 + rng.uniform(-ARM_JITTER, ARM_JITTER)
        half_d = (ARM_DEPTH_TOP - ARM_DEPTH_SHRINK * k) / 2 + rng.uniform(-ARM_JITTER, ARM_JITTER)
        if k == ELBOW_BAND:
            half_w += ELBOW_BULGE / 2
            half_d += ELBOW_BULGE / 2
        if k == 0:
            inner_face = half_w
        top = ARM_TOP_Y - k * ARM_BAND_STEP
        bottom = top - ARM_BAND_HEIGHT
        even = k % 2 == 0
        cubes.append(box(f"band_{k}", (-half_w, half_w), (bottom, top), (-half_d, half_d), "rust_dark" if even else "rust_light", dz))

        # 前甲：上下错开的两片瓦，下沿锈褐边 + 两颗锯齿。
        front_w = half_w - 0.4
        for name, x, plate_bottom, plate_top, z_add, material in (
            ("plate_a", (-front_w, 0.1 + 0.03 * k), bottom + 0.15, top - 0.15, 0.0, "rust_light" if even else "rust_dark"),
            ("plate_b", (-0.1 - 0.03 * k, front_w), bottom + 0.05, top - 0.35, 0.03, "rust_dark" if even else "rust_light"),
        ):
            cubes.append(box(f"{name}_{k}", x, (plate_bottom, plate_top), (half_d - 0.05 + z_add, half_d + 0.3 + z_add), material, dz))
            cubes.append(box(f"{name}_rim_{k}", (x[0] - 0.06, x[1] + 0.06), (plate_bottom - 0.05, plate_bottom + 0.32), (half_d + 0.05 + z_add, half_d + 0.42 + z_add), "rust_spot", dz))
            for tooth in range(2):
                tx = x[0] + 0.05 + tooth * (x[1] - x[0]) / 2 + rng.uniform(0.0, max(0.0, (x[1] - x[0]) / 2 - 0.7))  # 两颗齿分占左右半边
                cubes.append(
                    box(
                        f"{name}_tooth_{k}_{tooth}",
                        (tx, tx + rng.uniform(0.4, 0.6)),
                        (plate_bottom - 0.4 - 0.03 * tooth, plate_bottom + 0.1 - 0.03 * tooth),
                        (half_d + 0.1 + z_add, half_d + 0.34 + z_add),
                        "rust_light" if material == "rust_dark" else "rust_dark",
                        dz,
                    )
                )
        # 大锈斑：两块高度不同的薄块。
        for blot in range(2):
            bx = rng.uniform(-front_w + 0.1, front_w - 0.9)
            by = rng.uniform(bottom + 0.6, top - 0.8)
            cubes.append(box(f"blot_{k}_{blot}", (bx, bx + rng.uniform(0.5, 0.9)), (by, by + rng.uniform(0.3, 0.5)), (half_d + 0.28 + 0.03 * blot, half_d + 0.38 + 0.03 * blot), "rust_spot", dz))
        # 外翻甲角：外侧挑出的一片甲，比环略窄。
        cubes.append(box(f"flare_{k}", (half_w - 0.1, half_w + 0.5), (bottom + 0.25, top - 0.05 - 0.04 * k), (-half_d + 0.4, half_d - 0.4), "rust_light" if even else "rust_dark", dz))

        # 外侧大骨刺：先水平挑出、尖端再上翘，侧视能看出是刺。
        if k in SIDE_SPIKE_BANDS:
            cubes.append(box(f"side_spike_{k}", (half_w + 0.4, half_w + 1.7), (top - 1.0, top - 0.3), (-0.35, 0.35), "bone", dz))
            cubes.append(box(f"side_spike_tip_{k}", (half_w + 1.6, half_w + 2.4), (top - 0.75, top + 0.15), (-0.25, 0.25), "bone_shadow", dz))
        if k == ELBOW_BAND:
            # 肘刺：向后（-z）挑出，是侧视「>」形的尖。
            cubes.append(box("elbow_spike", (-0.5, 0.5), (top - 1.2, top - 0.3), (-half_d - 1.8, -half_d + 0.2), "bone", dz))
            cubes.append(box("elbow_spike_tip", (-0.35, 0.35), (top - 0.95, top + 0.05), (-half_d - 2.7, -half_d - 1.7), "bone_shadow", dz))

    # 肩顶斜刺：一级一级向外挪，约 29° 外斜。
    for index, (x, z, steps) in enumerate(SHOULDER_SPIKE_BASES):
        y = ARM_TOP_Y - 0.03 * index  # 底面错开，避免相邻刺共面
        for step in range(steps):
            width = (0.75, 0.6, 0.45)[step]
            height = 0.9 + 0.1 * index
            x_center = x + SHOULDER_SPIKE_LEAN * step
            cubes.append(box(f"shoulder_spike_{index}_{step}", (x_center - width / 2, x_center + width / 2), (y, y + height), (z - width / 2, z + width / 2), "bone" if (index + step) % 2 == 0 else "bone_shadow"))
            y += height - 0.05

    # 肩内侧肉帘：一根肉梁挂 7 条长短不一的肉丝；长条下端收成细丝，丝尾挂肉球（同 03）。
    curtain_top = ARM_TOP_Y - 0.7
    cubes.append(box("curtain_beam", (-inner_face - 0.7, -inner_face + 0.2), (ARM_TOP_Y - 0.75, ARM_TOP_Y - 0.1), (-1.7, 1.7), "flesh_dark"))
    for index, length in enumerate(FLESH_LENGTHS):
        width = 0.4 + 0.02 * (index % 4)
        x_hi = -inner_face - 0.05 - 0.06 * (index % 3)
        z_lo = CURTAIN_Z_START + CURTAIN_Z_PITCH * index
        material = "flesh_red" if index % 2 == 0 else "flesh_dark"
        other = "flesh_dark" if material == "flesh_red" else "flesh_red"
        if length < FLESH_LONG_FROM:
            cubes.append(box(f"curtain_{index}", (x_hi - width, x_hi), (curtain_top - length, curtain_top), (z_lo, z_lo + width), material))
            continue
        body_bottom = curtain_top - length * 0.75
        thread = 0.26
        inset = (width - thread) / 2
        cubes.append(box(f"curtain_{index}", (x_hi - width, x_hi), (body_bottom, curtain_top), (z_lo, z_lo + width), material))
        cubes.append(box(f"curtain_thread_{index}", (x_hi - width + inset, x_hi - inset), (curtain_top - length, body_bottom + 0.1), (z_lo + inset, z_lo + inset + thread), other))
        cubes.append(box(f"curtain_drip_{index}", (x_hi - width / 2 - 0.35, x_hi - width / 2 + 0.35), (curtain_top - length - 0.7, curtain_top - length + 0.2), (z_lo + width / 2 - 0.35, z_lo + width / 2 + 0.35), "flesh_red"))

    last_dz = z_offset(ARM_BAND_COUNT - 1)
    palm_dz = last_dz + FOREARM_Z_STEP
    palm_top = PALM_BOTTOM_Y + PALM_HEIGHT
    half_palm_w = PALM_WIDTH / 2
    half_palm_d = PALM_DEPTH / 2
    cubes.append(box("palm", (-half_palm_w, half_palm_w), (PALM_BOTTOM_Y, palm_top), (-half_palm_d, half_palm_d), "bone", palm_dz))
    # 掌前下缘一条暗色指根（骨 bone_shadow），掌背留一块暗红肉。
    cubes.append(box("palm_knuckles", (-half_palm_w - 0.1, half_palm_w + 0.1), (PALM_BOTTOM_Y - 0.05, PALM_BOTTOM_Y + 0.4), (half_palm_d - 0.25, half_palm_d + 0.25), "bone_shadow", palm_dz))
    cubes.append(box("palm_back_flesh", (-1.5, 1.5), (PALM_BOTTOM_Y + 0.3, palm_top - 0.45), (-half_palm_d - 0.14, -half_palm_d + 0.1), "flesh_dark", palm_dz))

    for finger, fx in enumerate(FINGER_X):
        reach = THUMB_REACH if finger == 0 else 1.0
        outer = abs(fx) > 1.0
        direction = 1.0 if fx > 0 else -1.0  # 局部 x 的正方向是朝外
        half = FINGER_WIDTH / 2
        z1 = half_palm_d - 0.3
        z1_end = z1 + SEG1_LEN * reach
        # 第 1 节：平伸向前。
        cubes.append(box(f"finger_{finger}_0", (fx - half, fx + half), (FINGER_TOP_Y - FINGER_THICKNESS, FINGER_TOP_Y), (z1, z1_end), "bone", palm_dz))
        # 第 2 节：下折，前端落在第 1 节前端下方，外侧指略向外散。
        x2 = fx + (direction * SEG2_SPREAD if outer else 0.0)
        y2_top = FINGER_TOP_Y - SEG2_DROP
        z2 = z1_end - SEG2_Z_OVERLAP
        z2_end = z2 + SEG2_LEN * reach
        cubes.append(box(f"finger_{finger}_1", (x2 - half + 0.02, x2 + half - 0.02), (y2_top - FINGER_THICKNESS, y2_top), (z2, z2_end), "bone", palm_dz))
        # 第 3 节：更陡下勾，收细，外侧指向手心内勾；末端再接一小截略暗的钩尖。
        x3 = x2 - (direction * SEG3_INWARD if outer else 0.0)
        z3 = z2_end - SEG3_Z_OVERLAP
        z3_end = z3 + SEG3_Z_LEN
        cubes.append(box(f"finger_{finger}_2", (x3 - SEG3_WIDTH / 2, x3 + SEG3_WIDTH / 2), (TIP_HEIGHT - 0.05, SEG3_TOP_Y), (z3, z3_end), "bone", palm_dz))
        cubes.append(box(f"finger_tip_{finger}", (x3 - 0.22, x3 + 0.22), (0.0, TIP_HEIGHT), (z3 + 0.15, z3_end + 0.1), "bone_shadow", palm_dz))
        # 关节暗缝：略宽于指身的薄片，包在第 1/2、2/3 节交界处。
        for name, x, y, z0, z1_ in (
            ("a", x2, y2_top + 0.05, z2 - 0.05, z1_end + 0.05),
            ("b", x3, SEG3_TOP_Y - 0.15, z3 - 0.05, z2_end + 0.05),
        ):
            seam_half = (half if name == "a" else SEG3_WIDTH / 2 + 0.15) + SEAM_PAD
            cubes.append(box(f"finger_seam_{finger}_{name}", (x - seam_half, x + seam_half), (y - SEAM_HEIGHT / 2, y + SEAM_HEIGHT / 2), (z0, z1_), "bone_shadow", palm_dz))
    return cubes


def part_front_arms_claws() -> list[dict]:
    """05 部件：左右两条肩宽腕窄、肘部弯折前倾的前臂，瓦状叠甲，肩顶斜刺，肩内侧肉帘，宽掌 + 粗长内勾骨爪贴地。

    对照 parts_ref/05_front_arms_claws.png。臂从 04 的外侧甲片下伸出，
    左右各用独立随机序列，形状略有差异。
    """
    rng_left = random.Random(5)
    rng_right = random.Random(17)
    cubes = _arm("l", -1.0, rng_left) + _arm("r", 1.0, rng_right)
    return _tag(cubes, "front_arms_claws")


# 建造顺序；编号就是调度的部件编号。后续部件按调度「过」之后逐件追加。
PARTS = {
    "01": ("spine_ribcage", part_spine_ribcage),
    "02": ("void_maw_skulls", part_void_maw_skulls),
    "03": ("flesh_strands", part_flesh_strands),
    "04": ("back_plating", part_back_plating),
    "05": ("front_arms_claws", part_front_arms_claws),
}


def cubes_up_to(number: str) -> list[dict]:
    """01..number 的累计部件，用于累计拼装图。"""
    return [cube for key in sorted(PARTS) if key <= number for cube in PARTS[key][1]()]


def all_cubes() -> list[dict]:
    """返回当前已建造的全部部件。"""
    return cubes_up_to(max(PARTS))


def _assert_no_coplanar_faces(cubes: Iterable[dict]) -> None:
    """拒绝同一朝向、同一平面上有面积重叠的两个面。"""
    faces = {"+X": [], "-X": [], "+Y": [], "-Y": [], "+Z": [], "-Z": []}
    for cube in cubes:
        low = cube["from"]
        high = cube["to"]
        name = cube["name"]
        faces["-X"].append((low[0], (low[1], high[1], low[2], high[2]), name))
        faces["+X"].append((high[0], (low[1], high[1], low[2], high[2]), name))
        faces["-Y"].append((low[1], (low[0], high[0], low[2], high[2]), name))
        faces["+Y"].append((high[1], (low[0], high[0], low[2], high[2]), name))
        faces["-Z"].append((low[2], (low[0], high[0], low[1], high[1]), name))
        faces["+Z"].append((high[2], (low[0], high[0], low[1], high[1]), name))

    for axis, plane_faces in faces.items():
        for index, (position, (u0, u1, v0, v1), name) in enumerate(plane_faces):
            for other_position, (other_u0, other_u1, other_v0, other_v1), other_name in plane_faces[index + 1 :]:
                if abs(position - other_position) >= 1e-4:
                    continue
                overlap_u = max(0.0, min(u1, other_u1) - max(u0, other_u0))
                overlap_v = max(0.0, min(v1, other_v1) - max(v0, other_v0))
                if overlap_u > 1e-3 and overlap_v > 1e-3:
                    raise ValueError(
                        f"共面冲突: {name} 与 {other_name} 在 {axis} 面共面 "
                        f"({position:.4f}), 重叠区域 ({overlap_u:.3f}x{overlap_v:.3f})"
                    )


def build_texture() -> Image.Image:
    """生成只含计划配色的 64×64 材质图集。"""
    texture = Image.new("RGBA", (RESOLUTION, RESOLUTION), tuple(PALETTE["void_black"] + [255]))
    draw = ImageDraw.Draw(texture)
    for material, (x0, y0, x1, y1) in MATERIAL_UV.items():
        draw.rectangle((x0, y0, x1 - 1, y1 - 1), fill=tuple(PALETTE[material] + [255]))
    return texture


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: list[dict] | None = None) -> Path:
    """写出当前部件集合的 Blockbench free-format 模型。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = list(all_cubes() if cubes_override is None else cubes_override)
    _assert_no_coplanar_faces(cubes)

    texture = build_texture()
    buffer = io.BytesIO()
    texture.save(buffer, format="PNG")
    texture_base64 = "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")

    elements = []
    for cube in cubes:
        uv = MATERIAL_UV[cube["material"]]
        faces = {side: {"uv": uv, "texture": 0} for side in ("north", "south", "east", "west", "up", "down")}
        elements.append(
            {
                "name": cube["name"],
                "box_uv": False,
                "from": cube["from"],
                "to": cube["to"],
                "faces": faces,
                "uuid": str(uuid.uuid4()),
            }
        )

    model = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "void_distorted",
        "resolution": {"width": RESOLUTION, "height": RESOLUTION},
        "elements": elements,
        "outliner": [
            {
                "name": group,
                "origin": [0.0, 10.0, 0.0],
                "children": [
                    element["uuid"]
                    for element, cube in zip(elements, cubes)
                    if cube["group"] == group
                ],
            }
            for group in dict.fromkeys(cube["group"] for cube in cubes)
        ],
        "textures": [
            {
                "name": "void_distorted",
                "folder": "entity",
                "namespace": "bong",
                "id": 0,
                "source": texture_base64,
            }
        ],
    }
    out_path.write_text(json.dumps(model, indent=2), encoding="utf-8")
    print(f"✓ VoidDistorted 01_spine_ribcage 写入成功: {out_path}")
    return out_path


def _crop_tight(image: Image.Image, threshold: float = 15.0) -> Image.Image:
    """按渲染背景裁掉空白，只保留当前部件。"""
    pixels = np.asarray(image.convert("RGB"))
    background = pixels[0, 0].astype(float)
    changed = np.linalg.norm(pixels.astype(float) - background, axis=2) > threshold
    ys, xs = np.where(changed)
    if len(xs) == 0:
        return image.convert("RGB")
    return image.convert("RGB").crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))


def _crop_reference_panels(path: Path) -> tuple[Image.Image, Image.Image]:
    """从左正面、右侧面的参考图中裁出部件本身，去掉灰色背景。"""
    image = Image.open(path).convert("RGB")
    array = np.asarray(image).astype(float)
    midpoint = image.width // 2
    panels = []
    for start, end in ((0, midpoint), (midpoint, image.width)):
        panel = array[:, start:end]
        border = np.concatenate(
            [
                panel[:20].reshape(-1, 3),
                panel[-20:].reshape(-1, 3),
                panel[:, :20].reshape(-1, 3),
                panel[:, -20:].reshape(-1, 3),
            ]
        )
        background = np.median(border, axis=0)
        changed = np.linalg.norm(panel - background, axis=2) > 20
        ys, xs = np.where(changed)
        if len(xs) == 0:
            raise ValueError(f"参考图 {path} 的面板 {start}:{end} 没有可裁剪部件")
        panels.append(image.crop((xs.min() + start, ys.min(), xs.max() + start + 1, ys.max() + 1)))
    return panels[0], panels[1]


def _scale_to_height(image: Image.Image, height: int) -> Image.Image:
    width = max(1, int(image.width * height / image.height))
    return image.resize((width, height), Image.Resampling.LANCZOS)


RENDER_VIEWS = (("Front", 0, 0), ("Side", 90, 0), ("3/4", -35, 20))


def _render_views(cubes: list[dict], tag: str) -> list[Image.Image]:
    """把一组立方体渲成 正面 / 侧面 / 3/4 三张，已裁掉空白并缩放到同一高度。"""
    from bbmodel_maker.render.render_bbmodel import render

    temporary_model = Path(f"/tmp/VoidDistorted_{tag}.bbmodel")
    generate_bbmodel(temporary_model, cubes_override=cubes)
    images = []
    for _, yaw, pitch in RENDER_VIEWS:
        image, _ = render(str(temporary_model), yaw=yaw, pitch=pitch, size=600)
        images.append(_scale_to_height(_crop_tight(image), 600))
    temporary_model.unlink(missing_ok=True)
    return images


def _strip(title: str, images: list[Image.Image]) -> Image.Image:
    gap = 20
    width = sum(image.width for image in images) + gap * (len(images) + 1)
    sheet = Image.new("RGB", (width, 640), (28, 30, 34))
    ImageDraw.Draw(sheet).text((20, 10), title, fill=(230, 230, 230))
    x = gap
    for image in images:
        sheet.paste(image, (x, 30))
        x += image.width + gap
    return sheet


def render_part(number: str) -> None:
    """输出部件 number 的单件三视图、与参考图的并排对照卡，以及累计拼装图。"""
    name, build = PARTS[number]
    label = f"{number}_{name}"
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    rendered = _render_views(build(), f"step_{label}")
    reference_path = PARTS_REF_DIR / f"{label}.png"
    if not reference_path.exists():
        raise FileNotFoundError(f"未找到参考图：{reference_path}")
    reference = [_scale_to_height(image, 600) for image in _crop_reference_panels(reference_path)]

    gap = 20
    card = Image.new(
        "RGB", (sum(image.width for image in reference + rendered) + gap * 5 + 40, 680), (28, 30, 34)
    )
    draw = ImageDraw.Draw(card)
    x = 20
    draw.text((25, 20), f"REF: {label} (Front / Side)", fill=(216, 204, 176))
    for image in reference:
        card.paste(image, (x, 60))
        x += image.width + gap
    draw.line((x - gap // 2, 20, x - gap // 2, 660), fill=(80, 84, 92), width=2)
    draw.text((x + 10, 20), f"NOW: part {label} (Front / Side / 3/4)", fill=(230, 230, 230))
    for image in rendered:
        card.paste(image, (x, 60))
        x += image.width + gap
    card.save(PARTS_DIR / f"check_{label}.png")
    _strip(f"{label} (Front / Side / 3/4)", rendered).save(PARTS_DIR / f"{label}.png")
    print(f"✓ {label} 对照卡与单件图已输出到 {PARTS_DIR}")

    if number != min(PARTS):
        accumulated = _render_views(cubes_up_to(number), f"accum_{label}")
        _strip(f"accumulated 01..{number} (Front / Side / 3/4)", accumulated).save(
            PARTS_DIR / f"accum_{number}.png"
        )
        print(f"✓ 累计拼装图: {PARTS_DIR / f'accum_{number}.png'}")


def self_test() -> None:
    """验证正常部件通过，并验证注入共面骨片会被拒绝。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] 部件 {', '.join(sorted(PARTS))} 累计无共面冲突")

    # 注入缺陷由第一节椎骨派生：另放一块骨片，顶面与它同高、水平投影部分重叠。
    victim = next(cube for cube in cubes if cube["name"].startswith("spine_vertebra_"))
    low, high = victim["from"], victim["to"]
    defective = list(cubes)
    defective.append(
        _cube(
            "inject_coplanar_fail",
            (low[0] + 0.3, high[1] - 0.8, low[2] + 0.5),
            (high[0] + 0.6, high[1], high[2] + 0.5),
            "bone",
        )
    )
    try:
        _assert_no_coplanar_faces(defective)
    except ValueError as error:
        print(f"  [OK] 成功捕获注入缺陷: {error}")
    else:
        raise RuntimeError("FAIL: 缺陷注入未被共面检查拦截")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成渊空畸变体逐部件模型")
    parser.add_argument("--self-test", action="store_true", help="运行共面门禁差分自证")
    parser.add_argument("--part", default=max(PARTS), choices=sorted(PARTS), help="要出图的部件编号")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return
    generate_bbmodel(args.out)
    render_part(args.part)


if __name__ == "__main__":
    main()
