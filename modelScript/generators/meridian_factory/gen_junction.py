#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b10: junction (经脉三通 / 四通)

风格：A 有机型 (活体血肉、暖粉半透明筋管、两端肉箍小骨环、中心肉质枢纽、交叉筋丝)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b10_junction.png

调度规范（整格 16×16×16，坐标 [-8, 8]）：
1. 中心肉质枢纽：x/z 4–12 (模型坐标 X/Z in [-4.0, 4.0])、y 1–9 (Y in [1.0, 9.0]，高 8px)，
   材质 #8a2a2a (flesh_main)，四个竖角切 1px 倒角；
   枢纽顶面 (y=9.0) 设 2 条 #c07868 (tendon_fiber) 交叉筋丝 (浮起 0.5px: y in [9.0, 9.5])。
2. 延伸管道：
   每个分支方向一节 b01 同截面暖粉筋管 #d9a08c (截面 8x6，底边离地 2px: Y in [2.0, 8.0]) 伸到方块边；
   管壁上下各带 1px 亮粉边 #e8bca8，管内贯穿 2x2 #f6dcc4 真元内光芯汇聚于枢纽；
   接口处一圈 b01 同款肉箍 #8a2a2a (宽 2px: [-7.5, -5.5]，四周外凸 1px: 宽 10x高 8，留出 0.5px 标准对接端面)；
   肉箍顶底面各配一个外 3x3、内孔 1x1、微凸 0.5px 的贴面小骨环端口 #d8ccb0。
3. 两大变体：
   - junction_t (T形三通)：-Z (北)、+X (东)、-X (西) 三向，南壁 (+Z) 为封闭肉质枢纽后壁；
   - junction_x (十字四通)：-Z (北)、+Z (南)、+X (东)、-X (西) 四向全通。
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
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/junction")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b10_junction.png")

# 配色表 (完全对齐 meridian_factory.md)
PALETTE = {
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (孔内深色底/暗面)
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (肉质枢纽主体、肉箍主色)
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (贴面小骨环端口)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 暖粉筋管壁
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋管上下 1px 亮边 / 顶面亮斑
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元内光芯
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 交叉筋丝
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
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_central_hub(is_cross: bool = False) -> List[dict]:
    """中心肉质枢纽：
    x/z 4–12 (模型坐标 X/Z in [-4.0, 4.0])、y 1–9 (Y in [1.0, 9.0]，高 8px)，材质 #8a2a2a；
    四个竖角切 1px 倒角；
    枢纽顶面 (y=9.0) 设 2 条 #c07868 交叉筋丝 (浮起 0.5px: y in [9.0, 9.5])；
    若为 junction_t (T形)，南面 (+Z) 为封闭肉质后壁，外侧附 #e8bca8 亮面与斜筋丝。
    """
    cubes = []
    pfx = "x_" if is_cross else "t_"

    # 1. 枢纽底板与顶板 (覆盖中心 X/Z in [-4.0, 4.0]，四竖角切除 1x1 倒角)
    # 底板: Y in [1.0, 2.0]
    cubes.append({"name": f"{pfx}hub_bot_mid", "from": [-3.0, 1.0, -4.0], "to": [3.0, 2.0, 4.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_bot_w",   "from": [-4.0, 1.0, -3.0], "to": [-3.0, 2.0, 3.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_bot_e",   "from": [ 3.0, 1.0, -3.0], "to": [ 4.0, 2.0, 3.0], "group": "central_hub", "material": "flesh_main"})

    # 顶板: Y in [8.0, 9.0]
    cubes.append({"name": f"{pfx}hub_top_mid", "from": [-3.0, 8.0, -4.0], "to": [3.0, 9.0, 4.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_top_w",   "from": [-4.0, 8.0, -3.0], "to": [-3.0, 9.0, 3.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_top_e",   "from": [ 3.0, 8.0, -3.0], "to": [ 4.0, 9.0, 3.0], "group": "central_hub", "material": "flesh_main"})

    # 四个切角圆钝立柱 (Y in [2.0, 8.0]，填充 4 个转角净空：NW, NE, SW, SE)
    cubes.append({"name": f"{pfx}hub_corner_nw", "from": [-4.0, 2.0, -4.0], "to": [-3.0, 8.0, -3.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_corner_ne", "from": [ 3.0, 2.0, -4.0], "to": [ 4.0, 8.0, -3.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_corner_sw", "from": [-4.0, 2.0,  3.0], "to": [-3.0, 8.0,  4.0], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_corner_se", "from": [ 3.0, 2.0,  3.0], "to": [ 4.0, 8.0,  4.0], "group": "central_hub", "material": "flesh_main"})

    # 2. 枢纽顶面 2 条 #c07868 交叉筋丝 (浮起 0.5px: Y in [9.0, 9.5])
    # 筋丝 1: 沿 X 轴方向主索 (X in [-3.2, 3.2], Z in [-0.5, 0.5])
    cubes.append({"name": f"{pfx}hub_fiber_x", "from": [-3.2, 9.0, -0.5], "to": [3.2, 9.5, 0.5], "group": "central_hub", "material": "tendon_fiber"})
    # 筋丝 2: 沿 Z 轴方向主索 (北段与南段交汇于中心)
    cubes.append({"name": f"{pfx}hub_fiber_z_n", "from": [-0.5, 9.0, -3.2], "to": [0.5, 9.5, -0.5], "group": "central_hub", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}hub_fiber_z_s", "from": [-0.5, 9.0,  0.5], "to": [0.5, 9.5,  3.2], "group": "central_hub", "material": "tendon_fiber"})
    # 顶面中心交汇亮斑 (#e8bca8 tendon_highlight)
    cubes.append({"name": f"{pfx}hub_fiber_center", "from": [-0.7, 9.48, -0.7], "to": [0.7, 9.58, 0.7], "group": "central_hub", "material": "tendon_highlight"})

    # 3. 枢纽内部真元核心 (#f6dcc4 qi_glow，截面 2x2, Y in [4.0, 6.0])
    cubes.append({"name": f"{pfx}hub_qi_core", "from": [-1.0, 4.0, -1.0], "to": [1.0, 6.0, 1.0], "group": "central_hub", "material": "qi_glow"})

    # 4. T 形三通专属：南壁 (+Z 面) 封闭墙体 (Z in [3.2, 4.0], X in [-3.0, 3.0], Y in [2.0, 8.0])
    if not is_cross:
        cubes.append({"name": "t_hub_south_wall", "from": [-3.0, 2.0, 3.2], "to": [3.0, 8.0, 4.0], "group": "central_hub", "material": "flesh_main"})
        cubes.append({"name": "t_hub_south_hl",   "from": [-2.0, 4.0, 3.95], "to": [2.0, 6.0, 4.05], "group": "central_hub", "material": "tendon_highlight"})

    return cubes


def _branch_geometry(pfx: str, dir_name: str) -> List[dict]:
    """生成单一分支管道（长 4px，从中心 4.0 延伸到边界 8.0）、肉箍与贴面小骨环端口。
    dir_name: 'n' (-Z), 's' (+Z), 'w' (-X), 'e' (+X)
    """
    cubes = []

    if dir_name == "n":  # -Z 方向
        # 1. 暖粉筋管 (Z in [-8.0, -4.0], X in [-4.0, 4.0], Y in [2.0, 8.0])
        cubes.append({"name": f"{pfx}tube_n", "from": [-4.0, 2.0, -8.0], "to": [4.0, 8.0, -4.0], "group": "branches", "material": "tendon_tube"})
        # 筋管上下 1px 亮边
        cubes.append({"name": f"{pfx}tube_n_hl_bot", "from": [-3.9, 1.95, -7.95], "to": [3.9, 2.8, -4.05], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_n_hl_top", "from": [-3.9, 7.2,  -7.95], "to": [3.9, 8.05, -4.05], "group": "branches", "material": "tendon_highlight"})
        # 内部真元内光芯
        cubes.append({"name": f"{pfx}tube_n_qi", "from": [-1.0, 4.0, -7.95], "to": [1.0, 6.0, -1.0], "group": "branches", "material": "qi_glow"})
        # 2. 肉箍 (Z in [-7.5, -5.5]，宽 2.0px，外廓 X in [-5, 5], Y in [1, 9])
        cubes.append({"name": f"{pfx}collar_n_bot", "from": [-5.0, 1.0, -7.5], "to": [ 5.0, 2.0, -5.5], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_n_top", "from": [-5.0, 8.0, -7.5], "to": [ 5.0, 9.0, -5.5], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_n_l",   "from": [-5.0, 2.0, -7.5], "to": [-4.0, 8.0, -5.5], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_n_r",   "from": [ 4.0, 2.0, -7.5], "to": [ 5.0, 8.0, -5.5], "group": "branches", "material": "flesh_main"})
        # 3. 贴面小骨环端口 (外 3x3: x in [-1.5, 1.5], z in [-7.0, -6.0]; 内孔 1x1: x in [-0.5, 0.5], z in [-6.75, -6.25])
        # 顶骨环
        cubes.append({"name": f"{pfx}port_n_t_w", "from": [-1.5, 9.0, -7.0], "to": [-0.5, 9.5, -6.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_t_e", "from": [ 0.5, 9.0, -7.0], "to": [ 1.5, 9.5, -6.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_t_n", "from": [-0.5, 9.0, -7.0], "to": [ 0.5, 9.5, -6.75], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_t_s", "from": [-0.5, 9.0, -6.25], "to": [ 0.5, 9.5, -6.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_t_hole", "from": [-0.5, 8.95, -6.75], "to": [0.5, 9.15, -6.25], "group": "branches", "material": "flesh_dark"})
        # 底骨环
        cubes.append({"name": f"{pfx}port_n_b_w", "from": [-1.5, 0.5, -7.0], "to": [-0.5, 1.0, -6.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_b_e", "from": [ 0.5, 0.5, -7.0], "to": [ 1.5, 1.0, -6.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_b_n", "from": [-0.5, 0.5, -7.0], "to": [ 0.5, 1.0, -6.75], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_b_s", "from": [-0.5, 0.5, -6.25], "to": [ 0.5, 1.0, -6.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_n_b_hole", "from": [-0.5, 0.85, -6.75], "to": [0.5, 1.05, -6.25], "group": "branches", "material": "flesh_dark"})

    elif dir_name == "s":  # +Z 方向
        cubes.append({"name": f"{pfx}tube_s", "from": [-4.0, 2.0,  4.0], "to": [4.0, 8.0,  8.0], "group": "branches", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}tube_s_hl_bot", "from": [-3.9, 1.95,  4.05], "to": [3.9, 2.8,  7.95], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_s_hl_top", "from": [-3.9, 7.2,   4.05], "to": [3.9, 8.05, 7.95], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_s_qi", "from": [-1.0, 4.0,  1.0], "to": [1.0, 6.0,  7.95], "group": "branches", "material": "qi_glow"})
        cubes.append({"name": f"{pfx}collar_s_bot", "from": [-5.0, 1.0,  5.5], "to": [ 5.0, 2.0,  7.5], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_s_top", "from": [-5.0, 8.0,  5.5], "to": [ 5.0, 9.0,  7.5], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_s_l",   "from": [-5.0, 2.0,  5.5], "to": [-4.0, 8.0,  7.5], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_s_r",   "from": [ 4.0, 2.0,  5.5], "to": [ 5.0, 8.0,  7.5], "group": "branches", "material": "flesh_main"})
        # 顶骨环
        cubes.append({"name": f"{pfx}port_s_t_w", "from": [-1.5, 9.0, 6.0], "to": [-0.5, 9.5, 7.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_t_e", "from": [ 0.5, 9.0, 6.0], "to": [ 1.5, 9.5, 7.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_t_n", "from": [-0.5, 9.0, 6.0], "to": [ 0.5, 9.5, 6.25], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_t_s", "from": [-0.5, 9.0, 6.75], "to": [ 0.5, 9.5, 7.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_t_hole", "from": [-0.5, 8.95, 6.25], "to": [0.5, 9.15, 6.75], "group": "branches", "material": "flesh_dark"})
        # 底骨环
        cubes.append({"name": f"{pfx}port_s_b_w", "from": [-1.5, 0.5, 6.0], "to": [-0.5, 1.0, 7.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_b_e", "from": [ 0.5, 0.5, 6.0], "to": [ 1.5, 1.0, 7.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_b_n", "from": [-0.5, 0.5, 6.0], "to": [ 0.5, 1.0, 6.25], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_b_s", "from": [-0.5, 0.5, 6.75], "to": [ 0.5, 1.0, 7.0], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_s_b_hole", "from": [-0.5, 0.85, 6.25], "to": [0.5, 1.05, 6.75], "group": "branches", "material": "flesh_dark"})

    elif dir_name == "w":  # -X 方向
        cubes.append({"name": f"{pfx}tube_w", "from": [-8.0, 2.0, -4.0], "to": [-4.0, 8.0, 4.0], "group": "branches", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}tube_w_hl_bot", "from": [-7.95, 1.95, -3.9], "to": [-4.05, 2.8, 3.9], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_w_hl_top", "from": [-7.95, 7.2,  -3.9], "to": [-4.05, 8.05, 3.9], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_w_qi", "from": [-7.95, 4.0, -1.0], "to": [-1.0, 6.0, 1.0], "group": "branches", "material": "qi_glow"})
        cubes.append({"name": f"{pfx}collar_w_bot", "from": [-7.5, 1.0, -5.0], "to": [-5.5, 2.0,  5.0], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_w_top", "from": [-7.5, 8.0, -5.0], "to": [-5.5, 9.0,  5.0], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_w_n",   "from": [-7.5, 2.0, -5.0], "to": [-5.5, 8.0, -4.0], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_w_s",   "from": [-7.5, 2.0,  4.0], "to": [-5.5, 8.0,  5.0], "group": "branches", "material": "flesh_main"})
        # 顶骨环
        cubes.append({"name": f"{pfx}port_w_t_n", "from": [-7.0, 9.0, -1.5], "to": [-6.0, 9.5, -0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_t_s", "from": [-7.0, 9.0,  0.5], "to": [-6.0, 9.5,  1.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_t_w", "from": [-7.0, 9.0, -0.5], "to": [-6.75, 9.5, 0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_t_e", "from": [-6.25, 9.0, -0.5], "to": [-6.0, 9.5,  0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_t_hole", "from": [-6.75, 8.95, -0.5], "to": [-6.25, 9.15, 0.5], "group": "branches", "material": "flesh_dark"})
        # 底骨环
        cubes.append({"name": f"{pfx}port_w_b_n", "from": [-7.0, 0.5, -1.5], "to": [-6.0, 1.0, -0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_b_s", "from": [-7.0, 0.5,  0.5], "to": [-6.0, 1.0,  1.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_b_w", "from": [-7.0, 0.5, -0.5], "to": [-6.75, 1.0, 0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_b_e", "from": [-6.25, 0.5, -0.5], "to": [-6.0, 1.0,  0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_w_b_hole", "from": [-6.75, 0.85, -0.5], "to": [-6.25, 1.05, 0.5], "group": "branches", "material": "flesh_dark"})

    elif dir_name == "e":  # +X 方向
        cubes.append({"name": f"{pfx}tube_e", "from": [ 4.0, 2.0, -4.0], "to": [ 8.0, 8.0, 4.0], "group": "branches", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}tube_e_hl_bot", "from": [ 4.05, 1.95, -3.9], "to": [ 7.95, 2.8, 3.9], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_e_hl_top", "from": [ 4.05, 7.2,  -3.9], "to": [ 7.95, 8.05, 3.9], "group": "branches", "material": "tendon_highlight"})
        cubes.append({"name": f"{pfx}tube_e_qi", "from": [ 1.0, 4.0, -1.0], "to": [ 7.95, 6.0, 1.0], "group": "branches", "material": "qi_glow"})
        cubes.append({"name": f"{pfx}collar_e_bot", "from": [ 5.5, 1.0, -5.0], "to": [ 7.5, 2.0,  5.0], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_e_top", "from": [ 5.5, 8.0, -5.0], "to": [ 7.5, 9.0,  5.0], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_e_n",   "from": [ 5.5, 2.0, -5.0], "to": [ 7.5, 8.0, -4.0], "group": "branches", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}collar_e_s",   "from": [ 5.5, 2.0,  4.0], "to": [ 7.5, 8.0,  5.0], "group": "branches", "material": "flesh_main"})
        # 顶骨环
        cubes.append({"name": f"{pfx}port_e_t_n", "from": [ 6.0, 9.0, -1.5], "to": [ 7.0, 9.5, -0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_t_s", "from": [ 6.0, 9.0,  0.5], "to": [ 7.0, 9.5,  1.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_t_w", "from": [ 6.0, 9.0, -0.5], "to": [ 6.25, 9.5, 0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_t_e", "from": [ 6.75, 9.0, -0.5], "to": [ 7.0, 9.5,  0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_t_hole", "from": [ 6.25, 8.95, -0.5], "to": [ 6.75, 9.15, 0.5], "group": "branches", "material": "flesh_dark"})
        # 底骨环
        cubes.append({"name": f"{pfx}port_e_b_n", "from": [ 6.0, 0.5, -1.5], "to": [ 7.0, 1.0, -0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_b_s", "from": [ 6.0, 0.5,  0.5], "to": [ 7.0, 1.0,  1.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_b_w", "from": [ 6.0, 0.5, -0.5], "to": [ 6.25, 1.0, 0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_b_e", "from": [ 6.75, 0.5, -0.5], "to": [ 7.0, 1.0,  0.5], "group": "branches", "material": "bone_main"})
        cubes.append({"name": f"{pfx}port_e_b_hole", "from": [ 6.25, 0.85, -0.5], "to": [ 6.75, 1.05, 0.5], "group": "branches", "material": "flesh_dark"})

    return cubes


def part_02_branches(is_cross: bool = False) -> List[dict]:
    """生成所有分支管道、肉箍与小骨环端口。
    - junction_t: 'n', 'w', 'e' (三向)
    - junction_x: 'n', 's', 'w', 'e' (四向)
    """
    cubes = []
    pfx = "x_" if is_cross else "t_"
    dirs = ["n", "s", "w", "e"] if is_cross else ["n", "w", "e"]

    for d in dirs:
        cubes.extend(_branch_geometry(pfx, d))

    return cubes


def all_cubes(is_cross: bool = False) -> List[dict]:
    """汇总指定变体的所有立方体。"""
    return part_01_central_hub(is_cross) + part_02_branches(is_cross)


# =============================================================================
# 门禁与无共面面核验
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


# =============================================================================
# 贴图与 bbmodel 序列化
# =============================================================================

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


def generate_bbmodel(is_cross: bool, out_path: Path) -> Path:
    """导出 junction 变体的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(is_cross)
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

    name = "junction_x" if is_cross else "junction_t"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "junction")
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


# =============================================================================
# 渲染与并排对标卡输出
# =============================================================================

def render_views(p_t_path: Path, p_x_path: Path):
    """输出包含 junction_t 与 junction_x（3/4 视 + 俯视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 渲染 junction_t (T形三通)
    im_t_iso, _ = render(p_t_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_t_top, _ = render(p_t_path, yaw=0.0,   pitch=89.9, size=500, bg=bg_color)

    # 渲染 junction_x (十字四通)
    im_x_iso, _ = render(p_x_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_x_top, _ = render(p_x_path, yaw=0.0,   pitch=89.9, size=500, bg=bg_color)

    # 1. 拼装 render.png (2x2 网格拼版：上行 3/4 等轴视对比，下行 俯视对比)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("junction_t (3/4 ISOMETRIC VIEW)", im_t_iso, 10, 10),
        ("junction_x (3/4 ISOMETRIC VIEW)", im_x_iso, cell_w + 20, 10),
        ("junction_t (TOP VIEW - T Junction)", im_t_top, 10, cell_h + 20),
        ("junction_x (TOP VIEW - Cross Junction)", im_x_top, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 380, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (两变体 3/4 视 + 俯视 2x2 拼版) 已输出: {render_path}")

    # 2. 拼装 check.png (左参考图，右当前两变体 3/4 视与俯视并排等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_t_iso = im_t_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_x_iso = im_x_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b10_junction.png)", fill=(210, 200, 180))

        # 贴右渲染图 (左 junction_t，右 junction_x)
        rx = ref_w + 24
        check_cv.paste(c_t_iso, (rx, 34 + 10))
        check_cv.paste(c_x_iso, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: junction_t 3-way, RIGHT: junction_x 4-way Cross)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含两变体 3/4 等高对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_junction.py 差分自证...")

    # 1. junction_t 测试
    cubes_t = all_cubes(is_cross=False)
    _assert_no_coplanar_faces(cubes_t)
    print("  [OK] junction_t 正常立方体集无共面冲突")

    defect_t = list(cubes_t) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [ 4.0, 8.0, -4.0],
        "material": "tendon_tube",
    }]
    caught_t = False
    try:
        _assert_no_coplanar_faces(defect_t)
    except AssertionError as e:
        caught_t = True
        print(f"  [OK] junction_t 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
    if not caught_t:
        raise RuntimeError("门禁失效: junction_t 注入共面冲突未被拦截!")

    # 2. junction_x 测试
    cubes_x = all_cubes(is_cross=True)
    _assert_no_coplanar_faces(cubes_x)
    print("  [OK] junction_x 正常立方体集无共面冲突")

    defect_x = list(cubes_x) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, 4.0],
        "to":   [ 4.0, 8.0, 8.0],
        "material": "tendon_tube",
    }]
    caught_x = False
    try:
        _assert_no_coplanar_faces(defect_x)
    except AssertionError as e:
        caught_x = True
        print(f"  [OK] junction_x 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
    if not caught_x:
        raise RuntimeError("门禁失效: junction_x 注入共面冲突未被拦截!")

    print("✓ gen_junction.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b10 junction 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()

    p_t = generate_bbmodel(is_cross=False, out_path=MODEL_DIR / "junction_t.bbmodel")
    p_x = generate_bbmodel(is_cross=True,  out_path=MODEL_DIR / "junction_x.bbmodel")
    generate_bbmodel(is_cross=False, out_path=MODEL_DIR / "junction.bbmodel")

    render_views(p_t, p_x)


if __name__ == "__main__":
    main()
