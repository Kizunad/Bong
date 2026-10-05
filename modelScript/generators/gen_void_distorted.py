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
MAW_OUTER_DEPTH = 2.0  # 外圈红肉：z0 .. z0+2.0（最靠前）
MAW_MID_DEPTH = 1.4  # 内圈暗红肉：前缘比外圈缩进 0.6
MAW_VOID_DEPTH = 0.8  # 中心虚空黑：再缩进 0.6，三级台阶就是漏斗
MAW_HOLE_HALF_WIDTH = 2.6
MAW_HOLE_BOTTOM = 11.6
MAW_HOLE_TOP = 15.4
MAW_MID_THICKNESS = 0.8

# 外圈红肉按行堆成参差的八边形：(y0, y1, 左半宽, 右半宽, 该行是否被洞穿)。
# 左右半宽故意不相等，边缘才不会像一个规整的框。
MAW_ROWS = [
    (9.9, 10.9, 1.4, 1.7, False),
    (10.9, 11.6, 2.6, 2.3, False),
    (11.6, 12.8, 3.4, 3.2, True),
    (12.8, 14.2, 3.7, 3.5, True),
    (14.2, 15.4, 3.2, 3.5, True),
    (15.4, 16.2, 2.6, 2.9, False),
    (16.2, 17.0, 1.7, 1.5, False),
]
# 外圈边缘的几块凸起肉瘤：(侧, y0, y1, 突出的半宽范围, z 前缘退让)
MAW_NUBS = [
    (-1, 13.0, 13.7, (3.7, 4.0)),
    (1, 14.6, 15.2, (3.5, 3.8)),
    (-1, 11.7, 12.3, (3.4, 3.7)),
    (1, 11.9, 12.5, (3.2, 3.6)),
]
# 垂下的短肉须：(x 中心, 长度)。
MAW_WHISKERS = [(-1.0, 1.3), (-0.2, 0.8), (0.6, 1.7), (1.3, 1.0)]


def _maw_funnel() -> list[dict]:
    """层层往里收的血肉漏斗：外圈参差红肉 → 内圈暗红 → 中心虚空黑，边缘几条短肉须。

    三层前缘依次缩进 0.6，正面看是有台阶的深洞，不是一个框。
    """
    z0 = MAW_FRONT_Z
    outer_z1 = z0 + MAW_OUTER_DEPTH
    cubes = []
    hole = MAW_HOLE_HALF_WIDTH
    for index, (y0, y1, left, right, pierced) in enumerate(MAW_ROWS):
        # 外圈以亮红为主，只有两行暗红夹在里面做斑驳，和内圈的暗红拉开层次。
        material = "flesh_dark" if index in (1, 5) else "flesh_red"
        if pierced:
            cubes.append(_cube(f"maw_outer{index}_l", (-left, y0, z0), (-hole, y1, outer_z1), material))
            cubes.append(_cube(f"maw_outer{index}_r", (hole, y0, z0), (right, y1, outer_z1), material))
        else:
            cubes.append(_cube(f"maw_outer{index}", (-left, y0, z0), (right, y1, outer_z1), material))
    for index, (sign, y0, y1, (x0, x1)) in enumerate(MAW_NUBS):
        lo, hi = (x0, x1) if sign > 0 else (-x1, -x0)
        cubes.append(
            _cube(f"maw_nub{index}", (lo, y0, z0 + 0.4), (hi, y1, outer_z1 - 0.5), "flesh_red")
        )

    # 内圈暗红：贴着洞壁的一圈，前缘比外圈缩进。
    mid_z1 = z0 + MAW_MID_DEPTH
    inner = hole - MAW_MID_THICKNESS
    bottom = MAW_HOLE_BOTTOM + MAW_MID_THICKNESS
    top = MAW_HOLE_TOP - MAW_MID_THICKNESS
    cubes += [
        _cube("maw_mid_top", (-hole, top, z0), (hole, MAW_HOLE_TOP, mid_z1), "flesh_dark"),
        _cube("maw_mid_bottom", (-hole, MAW_HOLE_BOTTOM, z0), (hole, bottom, mid_z1), "flesh_dark"),
        _cube("maw_mid_left", (-hole, bottom, z0), (-inner, top, mid_z1), "flesh_dark"),
        _cube("maw_mid_right", (inner, bottom, z0), (hole, top, mid_z1), "flesh_dark"),
        _cube(
            "maw_void",
            (-inner, bottom, z0),
            (inner, top, z0 + MAW_VOID_DEPTH),
            "void_black",
        ),
    ]
    # 洞口切角：四个角各塞一块暗红肉，把方洞削成近似八边形。块比内圈再缩进，
    # 且不盖住虚空板正面（板在 z0+0.8，块的前缘在 z0+1.1）。
    chamfer = 0.6
    for corner, (sx, y_edge, sy) in enumerate(
        [(-1, bottom, 1), (1, bottom, 1), (-1, top, -1), (1, top, -1)]
    ):
        x_far = sx * (inner + 0.2)
        x_near = sx * (inner - chamfer)
        y_near = y_edge + sy * chamfer
        y_edge = y_edge - sy * 0.2
        cubes.append(
            _cube(
                f"maw_chamfer_{corner}",
                (min(x_far, x_near), min(y_edge, y_near), z0 + 0.3),
                (max(x_far, x_near), max(y_edge, y_near), z0 + 1.1),
                "flesh_dark",
            )
        )
    # 肉须：从最下一行肉圈垂下，长短不一、粗细 0.5。
    for index, (x_center, length) in enumerate(MAW_WHISKERS):
        cubes.append(
            _cube(
                f"maw_whisker_{index}",
                (x_center - 0.25, MAW_ROWS[0][0] - length, z0 + 1.0),
                (x_center + 0.25, MAW_ROWS[0][0], z0 + 1.5),
                "flesh_red" if index % 2 == 0 else "flesh_dark",
            )
        )
    return cubes


SKULL_INNER_X = 3.4  # 颅骨内缘到中轴的距离；比肉圈最宽处略小，让颅骨压住肉圈边缘
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
    lift = 0.3 if sign > 0 else 0.0
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
        for name, y0, y1 in (("low", 11.4, 12.2), ("high", 14.75, 15.3)):
            lo, hi = (2.8, 4.6) if sign > 0 else (-4.6, -2.8)
            cubes.append(_cube(f"sinew_{side}_{name}", (lo, y0, z0 + 0.5), (hi, y1, z0 + 1.45), "flesh_dark"))
    return cubes


def part_void_maw_skulls() -> list[dict]:
    """02 部件：层层收拢的血肉漏斗 + 左右各一颗真骷髅，暗红肉连成一体，挂在脊椎前端。

    对照 parts_ref/02_void_maw_skulls.png。红肉条下垂（03）和骨冠不在本件内。
    """
    cubes = _maw_funnel() + _skull("l", -1) + _skull("r", 1) + _sinews()
    return _tag(cubes, "void_maw_skulls")


# 建造顺序；编号就是调度的部件编号。后续部件按调度「过」之后逐件追加。
PARTS = {
    "01": ("spine_ribcage", part_spine_ribcage),
    "02": ("void_maw_skulls", part_void_maw_skulls),
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
