#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b02: meridian_bend (经脉内腔 90° 弯段)

风格：A 有机型 (活体血肉、暖粉半透明筋管、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b02_meridian_bend.png

规范落实：
1. 统一约定与截面契约：
   - 输入端位于 -Z 侧 (z = -8.0)，截面居中宽 8px × 高 6px、底边离地 2px (x: -4..4, y: 2..8)；
   - 输出端位于 +X 侧 (x = 8.0)，截面居中宽 8px × 高 6px、底边离地 2px (z: -4..4, y: 2..8)；
   - 管道 90° 平滑弯转，整体严格落在 16×16×16 方块空间内。
2. 两端四面包覆肉箍与小骨环端口 (完全继承 b01)：
   - 两端各一圈肉箍 (#8a2a2a)，宽 3px (输入端 z: -7.5..-4.5，输出端 x: 4.5..7.5)；
   - 比管身四周各外凸 1px (截面外廓宽 10px、高 8px，四面全包覆)；
   - 贴面小骨环端口 (#d8ccb0)：外 3×3、内孔 1×1、微凸 0.5px，孔底深色 (#5a1a1a)，四个角各一个 (顶前、顶后、底前、底后)。
3. 暖粉半透明筋管与透光内芯：
   - 筋管壁全面使用暖粉色 #d9a08c (tendon_tube, alpha~60%)；
   - 外弧侧壁上下各留 1px 亮边 #e8bca8 (tendon_highlight)；
   - 外弧中央设有一块 6×2 晶莹透光的淡光斑 #f6dcc4 (qi_glow)；
   - 管内长轴细芯采用 #f6dcc4 沿中线 90° 贯穿。
4. 斜交筋丝与连续肉沿：
   - 外弧浮起 0.5px 配置斜交筋丝 #c07868 (tendon_fiber)，连成完整对角线 X 形交织；
   - 贴地 (y: 0.0..1.2) 配置暗血肉 #5a1a1a 连续肉沿，平滑连接两端肉箍。
5. 门禁验证：_assert_no_coplanar_faces 0 共面冲突，--self-test 缺陷拦截自测全绿。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "meridian_bend.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_bend")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b02_meridian_bend.png")

PALETTE = {
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨
    "tendon_tube":      (217, 160, 140, 150), # #d9a08c 暖粉半透明筋管壁 (alpha 60%)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 亮边
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 内光细芯 / 6x2 淡光斑
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 斜交筋丝
}

MAT_UV = {
    "flesh_dark":       [0, 0, 16, 16],
    "flesh_main":       [16, 0, 32, 16],
    "bone_main":        [32, 0, 48, 16],
    "tendon_tube":      [48, 0, 64, 16],
    "qi_glow":          [0, 16, 16, 32],
    "tendon_fiber":     [16, 16, 32, 32],
    "tendon_highlight": [32, 16, 48, 32],
}

RES = 64

def part_01_flesh_collars() -> List[dict]:
    """两端四面包覆的肉箍：宽 3px，比管身外凸 1px。"""
    cubes = []
    # Part 1: 肉箍 (两端四面包覆，外凸 1px，宽 3px)
    # 输入端肉箍 (-Z): z: -7.5..-4.5, x 范围取 [-5.0, 4.45] 避免与输出肉箍 corner 碰上
    # 输出端肉箍 (+X): x:  4.5.. 7.5, z 范围取 [-4.45, 5.0]
    # =========================================================================
    cz_in = -6.0
    # 输入端顶板 (y: 8.0..9.0)
    cubes.append({"name": "collar_in_top_l", "from": [-5.0, 8.0, -7.5], "to": [-0.5, 9.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_top_r", "from": [0.5, 8.0, -7.5], "to": [4.45, 9.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_top_mb", "from": [-0.5, 8.0, -7.5], "to": [0.5, 9.0, -6.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_top_mf", "from": [-0.5, 8.0, -5.5], "to": [0.5, 9.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})

    # 输入端底板 (y: 1.0..2.0)
    cubes.append({"name": "collar_in_bot_l", "from": [-5.0, 1.0, -7.5], "to": [-0.5, 2.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_bot_r", "from": [0.5, 1.0, -7.5], "to": [4.45, 2.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_bot_mb", "from": [-0.5, 1.0, -7.5], "to": [0.5, 2.0, -6.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_bot_mf", "from": [-0.5, 1.0, -5.5], "to": [0.5, 2.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})

    # 输入端左右侧板 (y: 2.0..8.0)
    cubes.append({"name": "collar_in_side_l", "from": [-5.0, 2.0, -7.5], "to": [-4.0, 8.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_side_r", "from": [4.0, 2.0, -7.5], "to": [4.45, 8.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})

    # 输出端 (+X 端, x: 4.5..7.5)
    cx_out = 6.0
    # 输出端顶板 (y: 8.0..9.0, z: -4.45..5.0)
    cubes.append({"name": "collar_out_top_b", "from": [4.5, 8.0, -4.45], "to": [7.5, 9.0, -0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_top_f", "from": [4.5, 8.0, 0.5], "to": [7.5, 9.0, 5.0], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_top_ml", "from": [4.5, 8.0, -0.5], "to": [5.5, 9.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_top_mr", "from": [6.5, 8.0, -0.5], "to": [7.5, 9.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})

    # 输出端底板 (y: 1.0..2.0)
    cubes.append({"name": "collar_out_bot_b", "from": [4.5, 1.0, -4.45], "to": [7.5, 2.0, -0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_bot_f", "from": [4.5, 1.0, 0.5], "to": [7.5, 2.0, 5.0], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_bot_ml", "from": [4.5, 1.0, -0.5], "to": [5.5, 2.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_bot_mr", "from": [6.5, 1.0, -0.5], "to": [7.5, 2.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})

    # 输出端前后侧板 (y: 2.0..8.0)
    cubes.append({"name": "collar_out_side_b", "from": [4.5, 2.0, -4.45], "to": [7.5, 8.0, -4.0], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_side_f", "from": [4.5, 2.0, 4.0], "to": [7.5, 8.0, 5.0], "group": "flesh_collars", "material": "flesh_main"})

    # =========================================================================
    return cubes

def part_02_ports() -> List[dict]:
    """四个贴面小骨环端口，外 3x3, 内孔 1x1, 凸出 0.5px。"""
    cubes = []
    # Part 2: 端口贴面骨环 (外 3x3, 内孔 1x1, 凸出 0.5px)
    # =========================================================================
    # 输入端端口 (x=0, z=-6.0)
    cubes.append({"name": "port_in_top_n", "from": [-1.5, 9.0, -7.5], "to": [1.5, 9.5, -6.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_s", "from": [-1.5, 9.0, -5.5], "to": [1.5, 9.5, -4.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_w", "from": [-1.5, 9.0, -6.5], "to": [-0.5, 9.5, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_e", "from": [0.5, 9.0, -6.5], "to": [1.5, 9.5, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_core", "from": [-0.48, 8.0, -6.48], "to": [0.48, 8.95, -5.52], "group": "ports", "material": "flesh_dark"})

    cubes.append({"name": "port_in_bot_n", "from": [-1.5, 0.5, -7.5], "to": [1.5, 1.0, -6.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_s", "from": [-1.5, 0.5, -5.5], "to": [1.5, 1.0, -4.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_w", "from": [-1.5, 0.5, -6.5], "to": [-0.5, 1.0, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_e", "from": [0.5, 0.5, -6.5], "to": [1.5, 1.0, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_core", "from": [-0.48, 1.05, -6.48], "to": [0.48, 2.0, -5.52], "group": "ports", "material": "flesh_dark"})

    # 输出端端口 (x=6.0, z=0)
    cubes.append({"name": "port_out_top_w", "from": [4.5, 9.0, -1.5], "to": [5.5, 9.5, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_e", "from": [6.5, 9.0, -1.5], "to": [7.5, 9.5, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_s", "from": [5.5, 9.0, -1.5], "to": [6.5, 9.5, -0.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_n", "from": [5.5, 9.0, 0.5], "to": [6.5, 9.5, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_core", "from": [5.52, 8.0, -0.48], "to": [6.48, 8.95, 0.48], "group": "ports", "material": "flesh_dark"})

    cubes.append({"name": "port_out_bot_w", "from": [4.5, 0.5, -1.5], "to": [5.5, 1.0, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_e", "from": [6.5, 0.5, -1.5], "to": [7.5, 1.0, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_s", "from": [5.5, 0.5, -1.5], "to": [6.5, 1.0, -0.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_n", "from": [5.5, 0.5, 0.5], "to": [6.5, 1.0, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_core", "from": [5.52, 1.05, -0.48], "to": [6.48, 2.0, 0.48], "group": "ports", "material": "flesh_dark"})

    # =========================================================================
    return cubes

def part_03_meridian_tube() -> List[dict]:
    """90° 弯管主体：暖粉半透明筋管 #d9a08c，外弧半径大，内弧半径小。"""
    cubes = []
    # Part 3: 管道与分层无共面设计
    # 底板厚度: y: 2.0..2.8 (底板由若干不重叠 XZ 块铺成)
    # 顶板厚度: y: 7.2..8.0 (顶板由完全相同的 XZ 块铺成)
    # 侧壁高度: y: 2.8..7.2 (只做中间高度，绝不碰 2.0 与 8.0！)
    # =========================================================================
    # ── 顶底板网格切分 (XZ 平面互不重叠，由输入口到输出口覆盖完整弯管) ──
    # 1. 输入直通段: x: [-4.0, 4.0], z: [-8.0, -4.5]
    # 2. 中段弯曲扇面分 4 个阶梯步：
    #    步 1: x: [-4.0,  4.0], z: [-4.5, -2.0]
    #    步 2: x: [-2.5,  4.5], z: [-2.0,  0.5]
    #    步 3: x: [-0.5,  4.5], z: [ 0.5,  2.5]
    #    步 4: x: [ 1.5,  4.5], z: [ 2.5,  4.0]
    # 3. 输出直通段: x: [ 4.5, 8.0], z: [-4.0,  4.0]
    floor_grid = [
        ("fl_in_pipe",  -4.0,  4.0, -8.0, -4.5),
        ("fl_bend_st1", -4.0,  4.0, -4.5, -2.0),
        ("fl_bend_st2", -2.5,  4.5, -2.0,  0.5),
        ("fl_bend_st3", -0.5,  4.5,  0.5,  2.5),
        ("fl_bend_st4",  1.5,  4.5,  2.5,  4.0),
        ("fl_out_pipe",  4.5,  8.0, -4.0,  4.0),
    ]
    for gname, gx0, gx1, gz0, gz1 in floor_grid:
        # 底板
        cubes.append({"name": f"{gname}_bot", "from": [gx0, 2.0, gz0], "to": [gx1, 2.8, gz1], "group": "meridian_tube", "material": "tendon_tube"})
        # 顶板
        cubes.append({"name": f"{gname}_top", "from": [gx0, 7.2, gz0], "to": [gx1, 8.0, gz1], "group": "meridian_tube", "material": "tendon_tube"})

    # ── 侧壁 (高度严格限制在 y: 2.8..7.2，厚 0.8px) ──
    # 1. 输入口暴露壁段 (z: -8.0..-7.5)
    cubes.append({"name": "wall_in_l", "from": [-4.0, 2.8, -8.0], "to": [-3.2, 7.2, -7.5], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "wall_in_r", "from": [3.2, 2.8, -8.0], "to": [4.0, 7.2, -7.5], "group": "meridian_tube", "material": "tendon_tube"})

    # 2. 输出口暴露壁段 (x: 7.5..8.0)
    cubes.append({"name": "wall_out_b", "from": [7.5, 2.8, -4.0], "to": [8.0, 7.2, -3.2], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "wall_out_f", "from": [7.5, 2.8, 3.2], "to": [8.0, 7.2, 4.0], "group": "meridian_tube", "material": "tendon_tube"})

    # 3. 外弧侧壁 (凸侧大弧度：顺着外边缘 x=-4 -> x=-2.5 -> x=-0.5 -> x=1.5 -> z=4.0)
    # y: 2.8..7.2，厚 0.8px
    outer_walls = [
        ("wall_out_seg1", -4.0, -3.2, -7.5, -2.0),
        ("wall_out_cor1", -3.2, -2.5, -2.0, -1.2),
        ("wall_out_seg2", -2.5, -1.7, -2.0,  0.5),
        ("wall_out_cor2", -1.7, -0.5,  0.5,  1.3),
        ("wall_out_seg3", -0.5,  0.3,  0.5,  2.5),
        ("wall_out_cor3",  0.3,  1.5,  2.5,  3.2),
        ("wall_out_seg4",  1.5,  4.5,  3.2,  4.0),
        ("wall_out_seg5",  4.5,  7.5,  3.2,  4.0),
    ]
    for wname, wx0, wx1, wz0, wz1 in outer_walls:
        # 下亮边 (y: 2.8..3.5)
        cubes.append({"name": f"{wname}_rim_b", "from": [wx0, 2.8, wz0], "to": [wx1, 3.5, wz1], "group": "meridian_tube", "material": "tendon_highlight"})
        # 中间主体 (y: 3.5..6.5)
        cubes.append({"name": f"{wname}_mid", "from": [wx0, 3.5, wz0], "to": [wx1, 6.5, wz1], "group": "meridian_tube", "material": "tendon_tube"})
        # 上亮边 (y: 6.5..7.2)
        cubes.append({"name": f"{wname}_rim_t", "from": [wx0, 6.5, wz0], "to": [wx1, 7.2, wz1], "group": "meridian_tube", "material": "tendon_highlight"})

    # 4. 内弧侧壁 (凹侧紧凑内角：在 x: 3.2..4.0, z: -4.5..-2.0 及内角处)
    inner_walls = [
        ("wall_in_seg1", 3.2, 4.0, -7.5, -4.5),
        ("wall_in_seg2", 3.2, 4.0, -4.5, -2.0),
        ("wall_in_cor",  4.0, 4.5, -2.0, -1.2),
        ("wall_in_seg3", 4.5, 7.5, -4.0, -3.2),
    ]
    for iname, ix0, ix1, iz0, iz1 in inner_walls:
        cubes.append({"name": iname, "from": [ix0, 2.8, iz0], "to": [ix1, 7.2, iz1], "group": "meridian_tube", "material": "tendon_tube"})

    # 5. 外弧正面 6×2 淡光斑 (#f6dcc4 qi_glow)
    # 贴在外弧中央最凸面 (wall_out_seg2 外侧 x: -2.55..-2.45, y: 4.0..6.0, z: -1.0..1.0)
    cubes.append({
        "name": "tube_bend_glow_window",
        "from": [-2.54, 4.0, -1.0],
        "to":   [-2.00, 6.0,  1.0],
        "group": "meridian_tube",
        "material": "qi_glow",
    })

    # =========================================================================
    return cubes

def part_04_inner_qi_glow() -> List[dict]:
    """管内长轴弧形贯穿细芯淡光 #f6dcc4，截面 2.0x2.0。"""
    cubes = []
    # Part 4: 内光细芯 (#f6dcc4 qi_glow)
    # y: 4.0..6.0, 宽 2.0, 沿弯管中线贯通
    # =========================================================================
    qi_segments = [
        ("qi_core_1", -1.0, 1.0, -8.0, -3.5),
        ("qi_core_2", -0.5, 2.0, -3.5, -1.0),
        ("qi_core_3",  0.8, 3.5, -1.0,  1.2),
        ("qi_core_4",  3.5, 8.0, -1.0,  1.0),
    ]
    for qname, qx0, qx1, qz0, qz1 in qi_segments:
        cubes.append({"name": qname, "from": [qx0, 4.0, qz0], "to": [qx1, 6.0, qz1], "group": "inner_qi_glow", "material": "qi_glow"})

    # =========================================================================
    return cubes

def part_05_diagonal_fibers() -> List[dict]:
    """斜交筋丝 #c07868，浮起 0.5px，立体交叉成完整 X。"""
    cubes = []
    # Part 5: 斜交筋丝 (#c07868 tendon_fiber，浮起 0.5px，立体交叉成完整 X)
    # 在外弧凸面上对角交叉
    # =========================================================================
    # 对角线 1 (斜向上)
    cubes.append({"name": "fib_d1_1", "from": [-4.45, 2.5, -4.2], "to": [-3.98, 3.8, -2.2], "group": "diagonal_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_d1_2", "from": [-2.95, 3.8, -2.2], "to": [-2.48, 5.0, -0.2], "group": "diagonal_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_d1_3", "from": [-0.95, 5.0, -0.2], "to": [-0.48, 6.2,  1.8], "group": "diagonal_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_d1_4", "from": [ 1.05, 6.2,  1.8], "to": [ 1.52, 7.5,  3.8], "group": "diagonal_fibers", "material": "tendon_fiber"})

    # 对角线 2 (斜向下，中心段微外浮跨越)
    cubes.append({"name": "fib_d2_1", "from": [-4.45, 6.2, -4.2], "to": [-3.98, 7.5, -2.2], "group": "diagonal_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_d2_2", "from": [-2.95, 5.0, -2.2], "to": [-2.48, 6.2, -0.2], "group": "diagonal_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_d2_3_bridge", "from": [-1.05, 3.8, -0.2], "to": [-0.38, 5.0, 1.8], "group": "diagonal_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_d2_4", "from": [ 1.05, 2.5,  1.8], "to": [ 1.52, 3.8,  3.8], "group": "diagonal_fibers", "material": "tendon_fiber"})

    # =========================================================================
    return cubes

def part_06_flesh_skirt() -> List[dict]:
    """连续肉沿：沿外弧与内弧连续铺展，贴地 y: 0.0..1.2。"""
    cubes = []
    # Part 6: 连续肉沿 (贴地 y: 0.0..1.2，沿外弧和内弧连续铺开，暗血肉 #5a1a1a)
    # =========================================================================
    # 外弧连续肉沿
    skirt_outer = [
        ("skirt_out_1", -5.8, -4.0, -6.5, -2.0),
        ("skirt_out_2", -4.5, -2.2, -2.0,  0.5),
        ("skirt_out_3", -2.2,  0.5,  0.5,  2.8),
        ("skirt_out_4",  0.5,  3.5,  2.8,  4.8),
        ("skirt_out_5",  3.5,  6.5,  4.0,  5.8),
    ]
    for sname, sx0, sx1, sz0, sz1 in skirt_outer:
        cubes.append({"name": sname, "from": [sx0, 0.0, sz0], "to": [sx1, 1.2, sz1], "group": "flesh_skirt", "material": "flesh_dark"})

    # 内弧肉托
    cubes.append({"name": "skirt_inner_anchor", "from": [4.0, 0.0, -5.5], "to": [5.5, 1.2, -4.0], "group": "flesh_skirt", "material": "flesh_dark"})
    return cubes



def all_cubes() -> List[dict]:
    """汇总所有 6 个部件的立方体。"""
    return (
        part_01_flesh_collars()
        + part_02_ports()
        + part_03_meridian_tube()
        + part_04_inner_qi_glow()
        + part_05_diagonal_fibers()
        + part_06_flesh_skirt()
    )


def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标且重叠的共面冲突。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
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
            key = (side, round(val, 4))
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
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
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图，严格使用 meridian_factory.md 修订后的暖粉配色。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if mat_name == "tendon_tube":
            for x in range(w):
                fib = rng.randint(-8, 8)
                tile[:, x, 0] = np.clip(tile[:, x, 0].astype(int) + fib, 0, 255)
                tile[:, x, 1] = np.clip(tile[:, x, 1].astype(int) + fib, 0, 255)
                tile[:, x, 2] = np.clip(tile[:, x, 2].astype(int) + fib, 0, 255)
                tile[:, x, 3] = np.clip(base_color[3] + rng.randint(-10, 10), 125, 175)
        elif mat_name == "tendon_highlight":
            for y in range(h):
                tile[y, :, :3] = np.clip(tile[y, :, :3].astype(int) + rng.randint(-6, 6), 0, 255)
        elif mat_name == "qi_glow":
            for y in range(h):
                for x in range(w):
                    r_dist = np.hypot(x - w / 2, y - h / 2) / (w / 2)
                    glow = int(14 * (1.0 - np.clip(r_dist, 0.0, 1.0)))
                    tile[y, x, :3] = np.clip(tile[y, x, :3].astype(int) + glow, 0, 255)
        elif mat_name == "tendon_fiber":
            for y in range(h):
                tile[y, :, :3] = np.clip(tile[y, :, :3].astype(int) + rng.randint(-6, 6), 0, 255)
        elif "flesh" in mat_name:
            noise = rng.randint(-12, 12, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: List[dict] | None = None) -> Path:
    """生成并导出 Blockbench bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = cubes_override if cubes_override is not None else all_cubes()
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
        uv = MAT_UV.get(mat, [16, 0, 32, 16])
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

    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "meridian_bend")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "meridian_bend",
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": "meridian_bend",
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
    print(f"✓ meridian_bend bbmodel 写入成功: {rel}")
    return out_path


def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    canvas_w = 1040
    canvas_h = 1040
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("FRONT (+Z View)", im_front, 20, 20),
        ("SIDE (+X Output)", im_side, 540, 20),
        ("3/4 ISOMETRIC", im_iso, 20, 540),
        ("TOP (90-Deg Bend)", im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 230, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png 已输出: {render_path}")

    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        top_scale_w = int(im_top.width * target_h / im_top.height)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)
        top_scaled = im_top.resize((top_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = top_scale_w + iso_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (b02_meridian_bend.png)", fill=(210, 200, 180))

        rx = ref_w + 24
        check_cv.paste(top_scaled, (rx, 30))
        check_cv.paste(iso_scaled, (rx + top_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (TOP VIEW + 3/4 VIEW)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_meridian_bend.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [4.0, 2.8, -4.5],
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_meridian_bend.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b02 meridian_bend 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    bb_path = generate_bbmodel()
    render_views(bb_path)


if __name__ == "__main__":
    main()
