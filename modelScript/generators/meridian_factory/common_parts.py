#!/usr/bin/env python3
"""经脉工厂内景 —— 共享通用接口件模块 (common_parts.py)

风格：A 有机型 (活体血肉、暖粉半透明筋管、骨质构件、肌纤维传送带)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md (第 2 批调整与统一约定)
- 爆炸图：model-review/img/meridian_factory/exploded/x00_common_interfaces.png

本模块集中封装工厂内景 7 大标准通用接口件，供所有经脉方块与器官生成器 import 复用：
1. sinew_tube(p0, p1, axis, ...)：b01 标准暖粉筋管段 (截面 8x6 居中离地 2px: Y in [2, 8]，上下 1px 亮边、内光芯、斜筋丝，支持 X/Y/Z 轴向)
2. flesh_collar(center_pos, length, axis, ...)：b01 标准包管肉箍 (#8a2a2a，四面包覆外凸 1px，截面 10x8，长度可调)
3. bone_ring_port(center_x, center_y, center_z, face, ...)：b01 标准贴面小骨环端口 (外 3x3、内孔 1x1、微凸 0.5px，孔底深陷 #5a1a1a)
4. outlet_8x6(center_x, center_y, center_z, facing, length, ...)：8x6 统一截面输出口与 2px 肉质套管 (#8a2a2a + #b05050 + #5a1a1a)
5. bone_wheel(center_x, center_y, center_z, radius, ...)：阀门/骨轮 (6x1x6 十字辐条与外圆环，#d8ccb0 带中心轴 #b8a888)
6. bone_rail(p0, p1, axis, side, ...)：传送带骨轨 (截面 2x2，底在 y=4.0、顶在 y=6.0)
7. bone_post(center_x, center_z, y_bot, height, ...)：传送带骨立柱 (截面 2x2，底在 y=4.0、顶在 y=8.0，带 #b8a888 节纹)
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/common_parts")
DEMO_BBMODEL = REPO / "modelScript" / "models" / "meridian_factory" / "common_parts_demo.bbmodel"

# 全厂标准统一调色板
PALETTE = {
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (孔底/阴影/暗斑)
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (肉箍/枢纽/套管主色)
    "flesh_lit":        (176, 80, 80, 255),   # #b05050 亮肉 / 肌肉红
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (骨环/骨轨/立柱/骨轮)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 / 节纹 / 锁销
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 暖粉半透明筋管壁
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋管上下 1px 亮边 / 顶面亮斑
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元内光芯
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 斜交筋丝 / 交叉筋索
}

MAT_UV = {
    "flesh_dark":       [0, 0, 16, 16],
    "flesh_main":       [16, 0, 32, 16],
    "bone_main":        [32, 0, 48, 16],
    "tendon_tube":      [48, 0, 64, 16],
    "qi_glow":          [0, 16, 16, 32],
    "tendon_fiber":     [16, 16, 32, 32],
    "tendon_highlight": [32, 16, 48, 32],
    "bone_dark":        [48, 16, 64, 32],
    "flesh_lit":        [0, 32, 16, 48],
}

RES = 64


# =============================================================================
# 7 大标准通用接口件函数
# =============================================================================

def sinew_tube(
    p0: float,
    p1: float,
    axis: str = "z",
    offset_x: float = 0.0,
    offset_y: float = 0.0,
    offset_z: float = 0.0,
    with_inner_qi: bool = True,
    with_highlights: bool = True,
    with_fibers: bool = False,
    group: str = "meridian_tube",
    pfx: str = "tube",
) -> List[dict]:
    """生成 b01 标准暖粉筋管段。
    尺寸出处：b01 meridian_straight (截面宽 8px x 高 6px，居中，底边离地 2px: Y in [2, 8])。
    - axis == 'z': 长轴沿 Z (X in [-4, 4], Y in [2, 8], Z in [p0, p1])
    - axis == 'x': 长轴沿 X (Z in [-4, 4], Y in [2, 8], X in [p0, p1])
    - axis == 'y': 长轴沿 Y (X in [-4, 4], Z in [-3, 3], Y in [p0, p1])
    """
    cubes = []
    z_min, z_max = min(p0, p1), max(p0, p1)

    if axis == "z":
        # 1. 筋管主体 (暖粉色 #d9a08c)
        cubes.append({
            "name": f"{pfx}_main",
            "from": [-4.0 + offset_x, 2.0 + offset_y, z_min + offset_z],
            "to":   [ 4.0 + offset_x, 8.0 + offset_y, z_max + offset_z],
            "group": group,
            "material": "tendon_tube",
        })

        # 2. 上下各 1px 亮粉高光边 (#e8bca8，贴于表面微缩 0.05px 避开端面共面)
        if with_highlights:
            cubes.append({
                "name": f"{pfx}_hl_bot",
                "from": [-3.9 + offset_x, 1.95 + offset_y, z_min + 0.05 + offset_z],
                "to":   [ 3.9 + offset_x, 2.80 + offset_y, z_max - 0.05 + offset_z],
                "group": group,
                "material": "tendon_highlight",
            })
            cubes.append({
                "name": f"{pfx}_hl_top",
                "from": [-3.9 + offset_x, 7.20 + offset_y, z_min + 0.05 + offset_z],
                "to":   [ 3.9 + offset_x, 8.05 + offset_y, z_max - 0.05 + offset_z],
                "group": group,
                "material": "tendon_highlight",
            })

        # 3. 内部贯穿 2x2 真元内光芯 (#f6dcc4 qi_glow)
        if with_inner_qi:
            cubes.append({
                "name": f"{pfx}_qi_core",
                "from": [-1.0 + offset_x, 4.0 + offset_y, z_min + 0.05 + offset_z],
                "to":   [ 1.0 + offset_x, 6.0 + offset_y, z_max - 0.05 + offset_z],
                "group": group,
                "material": "qi_glow",
            })

        # 4. 外壁斜交筋丝 (#c07868 tendon_fiber)
        if with_fibers:
            cubes.append({
                "name": f"{pfx}_fiber_front",
                "from": [-3.8 + offset_x, 2.6 + offset_y, z_min + 0.2 + offset_z],
                "to":   [ 3.8 + offset_x, 7.4 + offset_y, z_max - 0.2 + offset_z],
                "group": group,
                "material": "tendon_fiber",
            })

    elif axis == "x":
        cubes.append({
            "name": f"{pfx}_main",
            "from": [z_min + offset_x, 2.0 + offset_y, -4.0 + offset_z],
            "to":   [z_max + offset_x, 8.0 + offset_y,  4.0 + offset_z],
            "group": group,
            "material": "tendon_tube",
        })
        if with_highlights:
            cubes.append({
                "name": f"{pfx}_hl_bot",
                "from": [z_min + 0.05 + offset_x, 1.95 + offset_y, -3.9 + offset_z],
                "to":   [z_max - 0.05 + offset_x, 2.80 + offset_y,  3.9 + offset_z],
                "group": group,
                "material": "tendon_highlight",
            })
            cubes.append({
                "name": f"{pfx}_hl_top",
                "from": [z_min + 0.05 + offset_x, 7.20 + offset_y, -3.9 + offset_z],
                "to":   [z_max - 0.05 + offset_x, 8.05 + offset_y,  3.9 + offset_z],
                "group": group,
                "material": "tendon_highlight",
            })
        if with_inner_qi:
            cubes.append({
                "name": f"{pfx}_qi_core",
                "from": [z_min + 0.05 + offset_x, 4.0 + offset_y, -1.0 + offset_z],
                "to":   [z_max - 0.05 + offset_x, 6.0 + offset_y,  1.0 + offset_z],
                "group": group,
                "material": "qi_glow",
            })

    elif axis == "y":
        cubes.append({
            "name": f"{pfx}_main",
            "from": [-4.0 + offset_x, z_min + offset_y, -3.0 + offset_z],
            "to":   [ 4.0 + offset_x, z_max + offset_y,  3.0 + offset_z],
            "group": group,
            "material": "tendon_tube",
        })
        if with_inner_qi:
            cubes.append({
                "name": f"{pfx}_qi_core",
                "from": [-1.0 + offset_x, z_min + 0.05 + offset_y, -1.0 + offset_z],
                "to":   [ 1.0 + offset_x, z_max - 0.05 + offset_y,  1.0 + offset_z],
                "group": group,
                "material": "qi_glow",
            })

    return cubes


def flesh_collar(
    center_pos: float,
    length: float = 3.0,
    axis: str = "z",
    offset_x: float = 0.0,
    offset_y: float = 0.0,
    offset_z: float = 0.0,
    group: str = "flesh_collars",
    pfx: str = "collar",
) -> List[dict]:
    """生成 b01 标准包管肉箍。
    尺寸出处：b01 meridian_straight (四面包覆，截面宽 10px x 高 8px: 比 8x6 筋管四周各外凸 1px)。
    - length 可调 (标准为 3.0px，或短版为 2.0px)；
    - 材质为 #8a2a2a (flesh_main)。
    """
    cubes = []
    half_l = length / 2.0
    p_min = center_pos - half_l
    p_max = center_pos + half_l

    if axis == "z":
        # 底套 (Y in [1.0, 2.0], X in [-5.0, 5.0])
        cubes.append({"name": f"{pfx}_bot", "from": [-5.0 + offset_x, 1.0 + offset_y, p_min + offset_z], "to": [ 5.0 + offset_x, 2.0 + offset_y, p_max + offset_z], "group": group, "material": "flesh_main"})
        # 顶套 (Y in [8.0, 9.0], X in [-5.0, 5.0])
        cubes.append({"name": f"{pfx}_top", "from": [-5.0 + offset_x, 8.0 + offset_y, p_min + offset_z], "to": [ 5.0 + offset_x, 9.0 + offset_y, p_max + offset_z], "group": group, "material": "flesh_main"})
        # 左套 (X in [-5.0, -4.0], Y in [2.0, 8.0])
        cubes.append({"name": f"{pfx}_l",   "from": [-5.0 + offset_x, 2.0 + offset_y, p_min + offset_z], "to": [-4.0 + offset_x, 8.0 + offset_y, p_max + offset_z], "group": group, "material": "flesh_main"})
        # 右套 (X in [4.0, 5.0], Y in [2.0, 8.0])
        cubes.append({"name": f"{pfx}_r",   "from": [ 4.0 + offset_x, 2.0 + offset_y, p_min + offset_z], "to": [ 5.0 + offset_x, 8.0 + offset_y, p_max + offset_z], "group": group, "material": "flesh_main"})

    elif axis == "x":
        cubes.append({"name": f"{pfx}_bot", "from": [p_min + offset_x, 1.0 + offset_y, -5.0 + offset_z], "to": [p_max + offset_x, 2.0 + offset_y,  5.0 + offset_z], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_top", "from": [p_min + offset_x, 8.0 + offset_y, -5.0 + offset_z], "to": [p_max + offset_x, 9.0 + offset_y,  5.0 + offset_z], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_n",   "from": [p_min + offset_x, 2.0 + offset_y, -5.0 + offset_z], "to": [p_max + offset_x, 8.0 + offset_y, -4.0 + offset_z], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_s",   "from": [p_min + offset_x, 2.0 + offset_y,  4.0 + offset_z], "to": [p_max + offset_x, 8.0 + offset_y,  5.0 + offset_z], "group": group, "material": "flesh_main"})

    return cubes


def bone_ring_port(
    center_x: float = 0.0,
    center_y: float = 9.0,
    center_z: float = 0.0,
    face: str = "up",
    group: str = "ports",
    pfx: str = "port",
) -> List[dict]:
    """生成 b01 标准贴面小骨环端口。
    尺寸出处：b01 meridian_straight (外 3x3、内孔 1x1、微凸 0.5px，孔底深陷 #5a1a1a)。
    - face: 'up' (顶面 y=9), 'down' (底面 y=1), 'north', 'south', 'west', 'east'。
    """
    cubes = []

    if face == "up":
        y0, y1 = center_y, center_y + 0.5
        # 4 段骨板环绕 (外 3x3: x/z in [-1.5, 1.5], 内孔 1x1: x/z in [-0.5, 0.5])
        cubes.append({"name": f"{pfx}_w",    "from": [center_x - 1.5, y0, center_z - 1.5], "to": [center_x - 0.5, y1, center_z + 1.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_e",    "from": [center_x + 0.5, y0, center_z - 1.5], "to": [center_x + 1.5, y1, center_z + 1.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_n",    "from": [center_x - 0.5, y0, center_z - 1.5], "to": [center_x + 0.5, y1, center_z - 0.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_s",    "from": [center_x - 0.5, y0, center_z + 0.5], "to": [center_x + 0.5, y1, center_z + 1.5], "group": group, "material": "bone_main"})
        # 孔底深陷暗孔底 (#5a1a1a)
        cubes.append({"name": f"{pfx}_hole", "from": [center_x - 0.5, y0 - 0.05, center_z - 0.5], "to": [center_x + 0.5, y0 + 0.15, center_z + 0.5], "group": group, "material": "flesh_dark"})

    elif face == "down":
        y0, y1 = center_y - 0.5, center_y
        cubes.append({"name": f"{pfx}_w",    "from": [center_x - 1.5, y0, center_z - 1.5], "to": [center_x - 0.5, y1, center_z + 1.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_e",    "from": [center_x + 0.5, y0, center_z - 1.5], "to": [center_x + 1.5, y1, center_z + 1.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_n",    "from": [center_x - 0.5, y0, center_z - 1.5], "to": [center_x + 0.5, y1, center_z - 0.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_s",    "from": [center_x - 0.5, y0, center_z + 0.5], "to": [center_x + 0.5, y1, center_z + 1.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_hole", "from": [center_x - 0.5, y1 - 0.15, center_z - 0.5], "to": [center_x + 0.5, y1 + 0.05, center_z + 0.5], "group": group, "material": "flesh_dark"})

    return cubes


def outlet_8x6(
    center_x: float = 0.0,
    center_y: float = 5.0,
    center_z: float = 0.0,
    facing: str = "+z",
    length: float = 2.0,
    group: str = "outlets",
    pfx: str = "outlet_8x6",
) -> List[dict]:
    """生成 8×6 统一截面输出口与 2px 肉质套管。
    尺寸出处：o01 lung / o03 arm_muscle (截面宽 8px x 高 6px，外包 1px #8a2a2a 肉套，内衬 #b05050)。
    - facing: '+z', '-z', '+x', '-x', '-y' (底尖输出口)
    """
    cubes = []

    if facing == "+z":
        z0, z1 = center_z, center_z + length
        # 外层肉质套管 (外廓宽 10x高 8: X in [-5, 5], Y in [2, 8])
        cubes.append({"name": f"{pfx}_cuff_bot", "from": [center_x - 5.0, center_y - 4.0, z0], "to": [center_x + 5.0, center_y - 3.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_top", "from": [center_x - 5.0, center_y + 3.0, z0], "to": [center_x + 5.0, center_y + 4.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_l",   "from": [center_x - 5.0, center_y - 3.0, z0], "to": [center_x - 4.0, center_y + 3.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_r",   "from": [center_x + 4.0, center_y - 3.0, z0], "to": [center_x + 5.0, center_y + 3.0, z1], "group": group, "material": "flesh_main"})
        # 8x6 内腔衬层 (#b05050) 与深邃孔底 (#5a1a1a)
        cubes.append({"name": f"{pfx}_lining",   "from": [center_x - 4.0, center_y - 3.0, z0 + 0.05], "to": [center_x + 4.0, center_y + 3.0, z1 - 0.05], "group": group, "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}_hole_dark","from": [center_x - 3.8, center_y - 2.8, z0 - 0.1],  "to": [center_x + 3.8, center_y + 2.8, z0 + 0.2],   "group": group, "material": "flesh_dark"})

    elif facing == "-z":
        z0, z1 = center_z - length, center_z
        cubes.append({"name": f"{pfx}_cuff_bot", "from": [center_x - 5.0, center_y - 4.0, z0], "to": [center_x + 5.0, center_y - 3.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_top", "from": [center_x - 5.0, center_y + 3.0, z0], "to": [center_x + 5.0, center_y + 4.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_l",   "from": [center_x - 5.0, center_y - 3.0, z0], "to": [center_x - 4.0, center_y + 3.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_r",   "from": [center_x + 4.0, center_y - 3.0, z0], "to": [center_x + 5.0, center_y + 3.0, z1], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_lining",   "from": [center_x - 4.0, center_y - 3.0, z0 + 0.05], "to": [center_x + 4.0, center_y + 3.0, z1 - 0.05], "group": group, "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}_hole_dark","from": [center_x - 3.8, center_y - 2.8, z1 - 0.2],  "to": [center_x + 3.8, center_y + 2.8, z1 + 0.1],   "group": group, "material": "flesh_dark"})

    elif facing == "-y":
        y0, y1 = center_y - length, center_y
        cubes.append({"name": f"{pfx}_cuff_w",   "from": [center_x - 5.0, y0, center_z - 4.0], "to": [center_x - 4.0, y1, center_z + 4.0], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_e",   "from": [center_x + 4.0, y0, center_z - 4.0], "to": [center_x + 5.0, y1, center_z + 4.0], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_n",   "from": [center_x - 4.0, y0, center_z - 4.0], "to": [center_x + 4.0, y1, center_z - 3.0], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_cuff_s",   "from": [center_x - 4.0, y0, center_z + 3.0], "to": [center_x + 4.0, y1, center_z + 4.0], "group": group, "material": "flesh_main"})
        cubes.append({"name": f"{pfx}_lining",   "from": [center_x - 4.0, y0 + 0.05, center_z - 3.0], "to": [center_x + 4.0, y1 - 0.05, center_z + 3.0], "group": group, "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}_hole_dark","from": [center_x - 3.8, y1 - 0.2,  center_z - 2.8], "to": [center_x + 3.8, y1 + 0.1,  center_z + 2.8], "group": group, "material": "flesh_dark"})

    return cubes


def bone_wheel(
    center_x: float = 0.0,
    center_y: float = 0.0,
    center_z: float = 0.0,
    radius: float = 3.0,
    thickness: float = 1.0,
    axis: str = "y",
    group: str = "bone_wheel",
    pfx: str = "wheel",
) -> List[dict]:
    """生成阀门 / 骨轮 (6×1×6 十字辐条 + 外圆环)。
    尺寸出处：b11 valve / 阀门构件约定 (6x1x6 外圆骨环 + 十字辐条与中心轴)。
    """
    cubes = []
    r = radius
    th = thickness

    if axis == "y":
        y0 = center_y - th / 2.0
        y1 = center_y + th / 2.0
        # 1. 外圆骨环 (4 段圆弧板围成 2*r 见方骨环)
        cubes.append({"name": f"{pfx}_ring_n", "from": [center_x - r, y0, center_z - r], "to": [center_x + r, y1, center_z - r + 1.0], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_ring_s", "from": [center_x - r, y0, center_z + r - 1.0], "to": [center_x + r, y1, center_z + r], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_ring_w", "from": [center_x - r, y0, center_z - r + 1.0], "to": [center_x - r + 1.0, y1, center_z + r - 1.0], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_ring_e", "from": [center_x + r - 1.0, y0, center_z - r + 1.0], "to": [center_x + r, y1, center_z + r - 1.0], "group": group, "material": "bone_main"})

        # 2. 十字辐条 (宽 1.0px: 横向辐条与纵向辐条)
        cubes.append({"name": f"{pfx}_spoke_x", "from": [center_x - r + 1.0, y0 + 0.1, center_z - 0.5], "to": [center_x + r - 1.0, y1 - 0.1, center_z + 0.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_spoke_z_n", "from": [center_x - 0.5, y0 + 0.1, center_z - r + 1.0], "to": [center_x + 0.5, y1 - 0.1, center_z - 0.5], "group": group, "material": "bone_main"})
        cubes.append({"name": f"{pfx}_spoke_z_s", "from": [center_x - 0.5, y0 + 0.1, center_z + 0.5], "to": [center_x + 0.5, y1 - 0.1, center_z + r - 1.0], "group": group, "material": "bone_main"})

        # 3. 中心轴柱 (#b8a888 bone_dark，截面 1.4x1.4)
        cubes.append({"name": f"{pfx}_hub_pin", "from": [center_x - 0.7, y0 - 0.2, center_z - 0.7], "to": [center_x + 0.7, y1 + 0.2, center_z + 0.7], "group": group, "material": "bone_dark"})

    return cubes


def bone_rail(
    p0: float,
    p1: float,
    axis: str = "z",
    side: str = "left",
    center_y: float = 5.0,
    offset_x: float = 0.0,
    offset_z: float = 0.0,
    group: str = "bone_rails",
    pfx: str = "rail",
) -> List[dict]:
    """生成传送带骨轨 (截面 2x2，底在 y=4.0、顶在 y=6.0)。
    尺寸出处：b07 belt / 传送带统一约定。
    """
    cubes = []
    z_min, z_max = min(p0, p1), max(p0, p1)
    y0 = center_y - 1.0  # 4.0
    y1 = center_y + 1.0  # 6.0

    if axis == "z":
        # 宽度 2px (left: x in [-6, -4], right: x in [4, 6])
        if side == "left":
            x0, x1 = -6.0 + offset_x, -4.0 + offset_x
        else:
            x0, x1 = 4.0 + offset_x, 6.0 + offset_x

        cubes.append({
            "name": f"{pfx}_{side}",
            "from": [x0, y0, z_min + offset_z],
            "to":   [x1, y1, z_max + offset_z],
            "group": group,
            "material": "bone_main",
        })

    elif axis == "x":
        if side == "north":
            z0, z1 = -6.0 + offset_z, -4.0 + offset_z
        else:
            z0, z1 = 4.0 + offset_z, 6.0 + offset_z

        cubes.append({
            "name": f"{pfx}_{side}",
            "from": [z_min + offset_x, y0, z0],
            "to":   [z_max + offset_x, y1, z1],
            "group": group,
            "material": "bone_main",
        })

    return cubes


def bone_post(
    center_x: float,
    center_z: float,
    y_bot: float = 4.0,
    height: float = 4.0,
    group: str = "bone_posts",
    pfx: str = "post",
) -> List[dict]:
    """生成传送带骨立柱 (截面 2x2，底在 y=4.0、顶在 y=8.0，带 #b8a888 节纹)。
    尺寸出处：b07 belt / b05 splitter 传送带立柱规范。
    """
    cubes = []
    y0 = y_bot
    y1 = y_bot + height

    # 1. 骨立柱主体 (截面 2x2: X in [cx-1, cx+1], Z in [cz-1, cz+1])
    cubes.append({
        "name": f"{pfx}_body",
        "from": [center_x - 1.0, y0, center_z - 1.0],
        "to":   [center_x + 1.0, y1, center_z + 1.0],
        "group": group,
        "material": "bone_main",
    })

    # 2. 中段暗纹节带 (#b8a888 bone_dark，高 1.0px，微浮 0.05px)
    mid_y = y0 + height * 0.45
    cubes.append({
        "name": f"{pfx}_stripe",
        "from": [center_x - 1.05, mid_y, center_z - 1.05],
        "to":   [center_x + 1.05, mid_y + 1.0, center_z + 1.05],
        "group": group,
        "material": "bone_dark",
    })

    return cubes


# =============================================================================
# 门禁核验与序列化
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标同旋转且重叠的共面冲突。"""
    faces: Dict[Tuple[str, float, tuple], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        rot = tuple(c.get("rotation", [0, 0, 0]))
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (f[1], f[2], t[1], t[2])
            elif axis == 1:
                rect = (f[0], f[2], t[0], t[2])
            else:
                rect = (f[0], f[1], t[0], t[1])
            key = (side, round(val, 4), rot)
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val, rot), entries in faces.items():
        if len(entries) < 2:
            continue
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                u0 = max(r1[0], r2[0])
                u1 = min(r1[2], r2[2])
                v0 = max(r1[1], r2[1])
                v1 = min(r1[3], r2[3])
                if u1 - u0 > 0.001 and v1 - v0 > 0.001:
                    conflicts.append(
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}, rot={rot}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if "flesh" in mat_name or "tendon" in mat_name:
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            noise = rng.randint(-8, 8, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "qi" in mat_name:
            noise = rng.randint(-6, 6, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_demo_bbmodel(out_path: Path = DEMO_BBMODEL) -> Path:
    """生成 7 大通用接口件并排排布的演示 bbmodel。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = []

    # 1. sinew_tube (放置于 X=-22)
    cubes.extend(sinew_tube(-6.0, 6.0, axis="z", offset_x=-22.0, with_fibers=True, pfx="demo_sinew_tube"))

    # 2. flesh_collar (放置于 X=-11)
    cubes.extend(sinew_tube(-5.0, 5.0, axis="z", offset_x=-11.0, pfx="demo_collar_tube"))
    cubes.extend(flesh_collar(0.0, length=3.0, axis="z", offset_x=-11.0, pfx="demo_flesh_collar"))

    # 3. bone_ring_port (放置于 X=-2, Y=4.0)
    cubes.extend(bone_ring_port(center_x=-2.0, center_y=4.0, center_z=-4.0, face="up", pfx="demo_port_top"))
    cubes.extend(bone_ring_port(center_x=-2.0, center_y=4.0, center_z= 4.0, face="down", pfx="demo_port_bot"))

    # 4. outlet_8x6 (放置于 X=7, facing +z)
    cubes.extend(outlet_8x6(center_x=7.0, center_y=5.0, center_z=0.0, facing="+z", length=3.0, pfx="demo_outlet_8x6"))

    # 5. bone_wheel (放置于 X=16)
    cubes.extend(bone_wheel(center_x=16.0, center_y=4.0, center_z=0.0, radius=3.0, thickness=1.0, pfx="demo_bone_wheel"))

    # 6 & 7. bone_rail 与 bone_post (放置于 X=26)
    # 左侧：前端立柱 (Z=-5)、后端立柱 (Z=5)、中间骨轨 (Z in [-4, 4])
    cubes.extend(bone_post(21.0, -5.0, y_bot=4.0, height=4.0, pfx="demo_post_w1"))
    cubes.extend(bone_post(21.0,  5.0, y_bot=4.0, height=4.0, pfx="demo_post_w2"))
    cubes.extend(bone_rail(-4.0, 4.0, axis="z", side="left", offset_x=26.0, pfx="demo_rail_l"))

    # 右侧：前端立柱 (Z=-5)、后端立柱 (Z=5)、中间骨轨 (Z in [-4, 4])
    cubes.extend(bone_post(31.0, -5.0, y_bot=4.0, height=4.0, pfx="demo_post_e1"))
    cubes.extend(bone_post(31.0,  5.0, y_bot=4.0, height=4.0, pfx="demo_post_e2"))
    cubes.extend(bone_rail(-4.0, 4.0, axis="z", side="right", offset_x=26.0, pfx="demo_rail_r"))

    _assert_no_coplanar_faces(cubes)

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for c in cubes:
        f = c["from"]
        t = c["to"]
        mat = c.get("material", "flesh_main")
        uv = MAT_UV.get(mat, [0, 0, 16, 16])
        faces = {}
        for side in ["north", "south", "east", "west", "up", "down"]:
            faces[side] = {"uv": uv, "texture": 0}

        elem = {
            "name": c["name"],
            "box_uv": False,
            "from": f,
            "to": t,
            "faces": faces,
            "uuid": str(uuid.uuid4()),
        }
        elements.append(elem)

    name = "common_parts_demo"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "common_parts")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": name,
                "folder": "block",
                "namespace": "bong",
                "id": 0,
                "source": tex_base64,
            }
        ],
    }

    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    try:
        rel = out_path.relative_to(REPO)
    except ValueError:
        rel = out_path
    print(f"✓ {name} bbmodel 写入成功: {rel}")
    return out_path


def render_views(model_p: Path = DEMO_BBMODEL):
    """输出包含 7 大通用件全景与各部件并排细节的展示图 render.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染全景与多视角
    im_iso, _ = render(model_p, yaw=-35.0, pitch=30.0, size=600, bg=bg_color)
    im_top, _ = render(model_p, yaw=0.0,   pitch=89.9, size=600, bg=bg_color)
    im_front, _ = render(model_p, yaw=0.0, pitch=0.0,  size=600, bg=bg_color)
    im_side, _ = render(model_p, yaw=90.0, pitch=0.0,  size=600, bg=bg_color)

    # 2. 拼装 2x2 综合展示拼版
    cell_w, cell_h = 600, 600
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("COMMON INTERFACES (3/4 ISOMETRIC VIEW)", im_iso, 10, 10),
        ("TOP VIEW (7 Common Components Overview)", im_top, cell_w + 20, 10),
        ("FRONT VIEW (sinew_tube / flesh_collar / port / outlet / wheel / rail / post)", im_front, 10, cell_h + 20),
        ("SIDE VIEW (Components Section & Alignment)", im_side, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 520, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (7大通用件 2x2 拼版) 已输出: {render_path}")


def self_test():
    """运行门禁差分自证：正常通用件无共面冲突，注入冲突能准确拦截。"""
    print("运行 common_parts.py 差分自证...")

    # 1. 验证 7 大函数组合无冲突
    cubes = []
    cubes.extend(sinew_tube(-6.0, 6.0, axis="z", offset_x=-22.0, pfx="t1"))
    cubes.extend(flesh_collar(0.0, length=3.0, axis="z", offset_x=-11.0, pfx="c1"))
    cubes.extend(bone_ring_port(center_x=-2.0, center_y=4.0, center_z=-4.0, face="up", pfx="p1"))
    cubes.extend(outlet_8x6(center_x=7.0, center_y=5.0, center_z=0.0, facing="+z", length=3.0, pfx="o1"))
    cubes.extend(bone_wheel(center_x=16.0, center_y=4.0, center_z=0.0, radius=3.0, thickness=1.0, pfx="w1"))
    cubes.extend(bone_rail(-4.0, 4.0, axis="z", side="left", offset_x=26.0, pfx="r1"))
    cubes.extend(bone_post(21.0, -5.0, y_bot=4.0, height=4.0, pfx="po1"))

    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] 7 大通用接口件正常组合 ({len(cubes)} 个立方体) 0 项共面冲突")

    # 2. 注入冲突拦截验证
    defect = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-26.0, 2.0, -6.0],
        "to":   [-18.0, 8.0,  6.0],
        "material": "tendon_tube",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
    if not caught:
        raise RuntimeError("门禁失效: common_parts 注入共面冲突未被拦截!")

    print("✓ common_parts.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景共享通用接口件模块")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p = generate_demo_bbmodel(DEMO_BBMODEL)
    render_views(p)


if __name__ == "__main__":
    main()
