#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b06: prep_buffer (局部预备槽，含 empty / full 两个变体)

风格：A 有机型 (活体血肉、十二条红肉棱、八角骨节帽、竖立半透明筋囊、侧面竖排光斑、底面8x6入口、顶面小骨环出口)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b06_prep_buffer.png (左空右满)
- 调度要求：
  1. 外包尺寸：x/z 3–13、y 0–14 (X/Z in [-5.0, 5.0], Y in [0.0, 14.0]，比 b04 小一圈、竖长)；
  2. 十二条棱：#8a2a2a 肉条（2×2 截面），八个角 #d8ccb0 骨节帽（2×2×2，暗面 #b8a888）；
  3. 中间竖立筋腔：暖粉 #d9a08c，侧面中间区域 #e8bca8 (透光囊壁)，表面 2 条 #c07868 筋丝；
  4. 侧面亮斑 #f6dcc4 2×2：prep_buffer_empty 每面 0 个、_full 每面 3 个（竖排）；
  5. 底面中心一个 8×6 统一截面的入口（-Z 面，底边离地 2px: x in [-4, 4], y in [2, 8]）；
  6. 顶面一个 b01 同款小骨环出口 (#d8ccb0，外 3×3，内孔 1×1，微凸 0.5px，深色底 #5a1a1a)。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/prep_buffer")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b06_prep_buffer.png")

# 配色表 (完全对齐 meridian_factory.md)
PALETTE = {
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (十二条肉棱、8x6入口管套)
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (棱暗纹、顶出口深孔底)
    "flesh_lit":        (176, 80, 80, 255),   # #b05050 亮肉红 (8x6入口方孔内壁衬层)
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (八角 2×2×2 骨节帽、顶面小骨环出口)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 (骨节帽暗面暗纹)
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 筋管暖粉 (竖立筋囊外壁主体)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋膜透光亮色 (侧面中间透光囊壁)
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 筋丝 (囊壁表面斜筋丝)
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元光斑 (侧面 2×2 亮斑：empty 0 个 / full 竖排 3 个)
}

MAT_UV = {
    "flesh_main":       [0, 0, 16, 16],
    "flesh_dark":       [16, 0, 32, 16],
    "flesh_lit":        [32, 0, 48, 16],
    "bone_main":        [48, 0, 64, 16],
    "bone_dark":        [0, 16, 16, 32],
    "tendon_tube":      [16, 16, 32, 32],
    "tendon_highlight": [32, 16, 48, 32],
    "tendon_fiber":     [48, 16, 64, 32],
    "qi_glow":          [0, 32, 16, 48],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_ribs(variant: str = "empty") -> List[dict]:
    """十二条棱：#8a2a2a 肉条（2×2 截面），八个角之间榫卯咬合，表面加 #5a1a1a 暗纹。
    外包 X/Z in [-5.0, 5.0]，Y in [0.0, 14.0]。
    """
    cubes = []
    pfx = f"{variant}_"

    # 1. 四根通高竖肉棱 (截面 2x2，y in [2.0, 12.0]，高 10px)
    cubes.append({"name": f"{pfx}rib_vert_nw", "from": [-5.0, 2.0, -5.0], "to": [-3.0, 12.0, -3.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}rib_vert_ne", "from": [ 3.0, 2.0, -5.0], "to": [ 5.0, 12.0, -3.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}rib_vert_sw", "from": [-5.0, 2.0,  3.0], "to": [-3.0, 12.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}rib_vert_se", "from": [ 3.0, 2.0,  3.0], "to": [ 5.0, 12.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})

    # 2. 底面四条水平横肉棱 (截面 2x2，y in [0.0, 2.0]，长 6px: x/z in [-3.0, 3.0])
    cubes.append({"name": f"{pfx}bot_rib_n", "from": [-3.0, 0.0, -5.0], "to": [ 3.0, 2.0, -3.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}bot_rib_s", "from": [-3.0, 0.0,  3.0], "to": [ 3.0, 2.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}bot_rib_w", "from": [-5.0, 0.0, -3.0], "to": [-3.0, 2.0,  3.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}bot_rib_e", "from": [ 3.0, 0.0, -3.0], "to": [ 5.0, 2.0,  3.0], "group": "flesh_ribs", "material": "flesh_main"})

    # 3. 顶面四条水平横肉棱 (截面 2x2，y in [12.0, 14.0]，长 6px: x/z in [-3.0, 3.0])
    cubes.append({"name": f"{pfx}top_rib_n", "from": [-3.0, 12.0, -5.0], "to": [ 3.0, 14.0, -3.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_rib_s", "from": [-3.0, 12.0,  3.0], "to": [ 3.0, 14.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_rib_w", "from": [-5.0, 12.0, -3.0], "to": [-3.0, 14.0,  3.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_rib_e", "from": [ 3.0, 12.0, -3.0], "to": [ 5.0, 14.0,  3.0], "group": "flesh_ribs", "material": "flesh_main"})

    # 4. 肉条表面暗纹 (#5a1a1a flesh_dark，在四根竖棱中部各贴 2 条微凸 0.05px 的暗纹条)
    for y_pos, idx in [(4.5, 1), (8.5, 2)]:
        cubes.append({"name": f"{pfx}rib_stripe_nw_{idx}", "from": [-5.05, y_pos, -5.05], "to": [-2.95, y_pos + 1.0, -2.95], "group": "flesh_ribs", "material": "flesh_dark"})
        cubes.append({"name": f"{pfx}rib_stripe_ne_{idx}", "from": [ 2.95, y_pos, -5.05], "to": [ 5.05, y_pos + 1.0, -2.95], "group": "flesh_ribs", "material": "flesh_dark"})
        cubes.append({"name": f"{pfx}rib_stripe_sw_{idx}", "from": [-5.05, y_pos,  2.95], "to": [-2.95, y_pos + 1.0,  5.05], "group": "flesh_ribs", "material": "flesh_dark"})
        cubes.append({"name": f"{pfx}rib_stripe_se_{idx}", "from": [ 2.95, y_pos,  2.95], "to": [ 5.05, y_pos + 1.0,  5.05], "group": "flesh_ribs", "material": "flesh_dark"})

    return cubes


def part_02_bone_corner_caps(variant: str = "empty") -> List[dict]:
    """八个角各一个 #d8ccb0 骨节帽（2×2×2，暗面 #b8a888）。"""
    cubes = []
    pfx = f"{variant}_"

    corners = [
        # (name_suffix, x0, x1, y0, y1, z0, z1, sx0, sx1, sy0, sy1, sz0, sz1)
        # 底面 4 角 (y in [0.0, 2.0], 暗面条带在 y in [0.5, 1.5])
        ("bot_nw", -5.0, -3.0,  0.0,  2.0, -5.0, -3.0, -4.5, -3.5, 0.5, 1.5, -5.05, -5.0),
        ("bot_ne",  3.0,  5.0,  0.0,  2.0, -5.0, -3.0,  3.5,  4.5, 0.5, 1.5, -5.05, -5.0),
        ("bot_sw", -5.0, -3.0,  0.0,  2.0,  3.0,  5.0, -4.5, -3.5, 0.5, 1.5,  5.0,   5.05),
        ("bot_se",  3.0,  5.0,  0.0,  2.0,  3.0,  5.0,  3.5,  4.5, 0.5, 1.5,  5.0,   5.05),
        # 顶面 4 角 (y in [12.0, 14.0], 暗面条带在 y in [12.5, 13.5])
        ("top_nw", -5.0, -3.0, 12.0, 14.0, -5.0, -3.0, -4.5, -3.5, 12.5, 13.5, -5.05, -5.0),
        ("top_ne",  3.0,  5.0, 12.0, 14.0, -5.0, -3.0,  3.5,  4.5, 12.5, 13.5, -5.05, -5.0),
        ("top_sw", -5.0, -3.0, 12.0, 14.0,  3.0,  5.0, -4.5, -3.5, 12.5, 13.5,  5.0,   5.05),
        ("top_se",  3.0,  5.0, 12.0, 14.0,  3.0,  5.0,  3.5,  4.5, 12.5, 13.5,  5.0,   5.05),
    ]

    for cname, x0, x1, y0, y1, z0, z1, sx0, sx1, sy0, sy1, sz0, sz1 in corners:
        # 骨节帽主体 (2x2x2, #d8ccb0 bone_main)
        cubes.append({"name": f"{pfx}bone_cap_{cname}", "from": [x0, y0, z0], "to": [x1, y1, z1], "group": "bone_caps", "material": "bone_main"})
        # 骨节帽表面暗面条带 (#b8a888 bone_dark，微凸 0.05px)
        cubes.append({"name": f"{pfx}bone_cap_dark_{cname}", "from": [sx0, sy0, sz0], "to": [sx1, sy1, sz1], "group": "bone_caps", "material": "bone_dark"})

    return cubes


def part_03_tendon_sac(variant: str = "empty") -> List[dict]:
    """中间竖立筋腔：暖粉 #d9a08c (tendon_tube)，内嵌于四根竖棱之间的净空 (x/z in [-2.95, 2.95], y in [2.05, 11.95])；
    完全在 y in [2.05, 11.95] 内部，与底横棱 (y<=2.0) 和顶横棱 (y>=12.0) 严格避让零共面；
    侧面中间区域改为 #e8bca8 (tendon_highlight，透光囊壁)；
    表面附着 2 条 #c07868 (tendon_fiber) 斜筋丝。
    """
    cubes = []
    pfx = f"{variant}_"

    # 筋腔底板 (y in [2.05, 2.3]) 与 顶板 (y in [11.7, 11.95])
    cubes.append({"name": f"{pfx}sac_bottom_plate", "from": [-2.95, 2.05, -2.95], "to": [2.95, 2.3, 2.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_top_plate",    "from": [-2.95, 11.7, -2.95], "to": [2.95, 11.95, 2.95], "group": "tendon_sac", "material": "tendon_tube"})

    # ── 1. 南面 (+Z，外表面位于 z = 2.95) ──
    # 下边框 (y: 2.3..3.5)
    cubes.append({"name": f"{pfx}sac_s_frame_bot", "from": [-2.95, 2.3, 2.45], "to": [ 2.95,  3.5, 2.95], "group": "tendon_sac", "material": "tendon_tube"})
    # 上边框 (y: 10.5..11.7)
    cubes.append({"name": f"{pfx}sac_s_frame_top", "from": [-2.95, 10.5, 2.45], "to": [ 2.95, 11.7, 2.95], "group": "tendon_sac", "material": "tendon_tube"})
    # 左侧壁 (x: -2.95..-1.8, y: 3.5..10.5)
    cubes.append({"name": f"{pfx}sac_s_frame_l",   "from": [-2.95, 3.5, 2.45], "to": [-1.8, 10.5, 2.95], "group": "tendon_sac", "material": "tendon_tube"})
    # 右侧壁 (x: 1.8..2.95, y: 3.5..10.5)
    cubes.append({"name": f"{pfx}sac_s_frame_r",   "from": [ 1.8, 3.5, 2.45], "to": [ 2.95, 10.5, 2.95], "group": "tendon_sac", "material": "tendon_tube"})
    # 中间透光囊壁 (#e8bca8 tendon_highlight，宽 3.6px: x in [-1.8, 1.8], 高 7px: y in [3.5, 10.5])
    cubes.append({"name": f"{pfx}sac_s_glow_wall", "from": [-1.8, 3.5, 2.45], "to": [ 1.8, 10.5, 2.95], "group": "tendon_sac", "material": "tendon_highlight"})
    # 表面 2 条 #c07868 斜筋丝 (微浮出 0.05px: z in [2.95, 3.0])
    cubes.append({"name": f"{pfx}sac_s_fiber_1", "from": [-2.8, 2.4, 2.95], "to": [-1.6, 3.6, 3.0], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_s_fiber_2", "from": [ 1.6, 10.4, 2.95], "to": [ 2.8, 11.6, 3.0], "group": "tendon_sac", "material": "tendon_fiber"})

    # ── 2. 西面 (-X，外表面位于 x = -2.95) ──
    # 下边框 (y: 2.3..3.5)
    cubes.append({"name": f"{pfx}sac_w_frame_bot", "from": [-2.95, 2.3, -2.45], "to": [-2.45,  3.5,  2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 上边框 (y: 10.5..11.7)
    cubes.append({"name": f"{pfx}sac_w_frame_top", "from": [-2.95, 10.5, -2.45], "to": [-2.45, 11.7,  2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 侧壁后段 (z: -2.45..-1.8)
    cubes.append({"name": f"{pfx}sac_w_frame_l",   "from": [-2.95, 3.5, -2.45], "to": [-2.45, 10.5, -1.8], "group": "tendon_sac", "material": "tendon_tube"})
    # 侧壁前段 (z: 1.8..2.45)
    cubes.append({"name": f"{pfx}sac_w_frame_r",   "from": [-2.95, 3.5,  1.8], "to": [-2.45, 10.5,  2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 中间透光囊壁 (#e8bca8 tendon_highlight)
    cubes.append({"name": f"{pfx}sac_w_glow_wall", "from": [-2.95, 3.5, -1.8], "to": [-2.45, 10.5,  1.8], "group": "tendon_sac", "material": "tendon_highlight"})
    # 表面 2 条 #c07868 斜筋丝
    cubes.append({"name": f"{pfx}sac_w_fiber_1", "from": [-3.0, 2.4, -2.8], "to": [-2.95, 3.6, -1.6], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_w_fiber_2", "from": [-3.0, 10.4,  1.6], "to": [-2.95, 11.6,  2.8], "group": "tendon_sac", "material": "tendon_fiber"})

    # ── 3. 东面 (+X，外表面位于 x = 2.95) ──
    # 下边框 (y: 2.3..3.5)
    cubes.append({"name": f"{pfx}sac_e_frame_bot", "from": [ 2.45, 2.3, -2.45], "to": [ 2.95,  3.5,  2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 上边框 (y: 10.5..11.7)
    cubes.append({"name": f"{pfx}sac_e_frame_top", "from": [ 2.45, 10.5, -2.45], "to": [ 2.95, 11.7,  2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 侧壁后段 (z: -2.45..-1.8)
    cubes.append({"name": f"{pfx}sac_e_frame_l",   "from": [ 2.45, 3.5, -2.45], "to": [ 2.95, 10.5, -1.8], "group": "tendon_sac", "material": "tendon_tube"})
    # 侧壁前段 (z: 1.8..2.45)
    cubes.append({"name": f"{pfx}sac_e_frame_r",   "from": [ 2.45, 3.5,  1.8], "to": [ 2.95, 10.5,  2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 中间透光囊壁 (#e8bca8 tendon_highlight)
    cubes.append({"name": f"{pfx}sac_e_glow_wall", "from": [ 2.45, 3.5, -1.8], "to": [ 2.95, 10.5,  1.8], "group": "tendon_sac", "material": "tendon_highlight"})
    # 表面 2 条 #c07868 斜筋丝
    cubes.append({"name": f"{pfx}sac_e_fiber_1", "from": [ 2.95, 2.4, -2.8], "to": [ 3.0, 3.6, -1.6], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_e_fiber_2", "from": [ 2.95, 10.4,  1.6], "to": [ 3.0, 11.6,  2.8], "group": "tendon_sac", "material": "tendon_fiber"})

    # ── 4. 北面 (-Z，z = -2.95，避开下方 8x6 入口管套连接处) ──
    # 北面顶框梁 (y: 10.5..11.7)
    cubes.append({"name": f"{pfx}sac_n_frame_top", "from": [-2.95, 10.5, -2.95], "to": [2.95, 11.7, -2.45], "group": "tendon_sac", "material": "tendon_tube"})
    # 北面高位透光囊壁 (y: 8.5..10.5)
    cubes.append({"name": f"{pfx}sac_n_glow_wall", "from": [-1.8,  8.5, -2.95], "to": [1.8, 10.5, -2.45], "group": "tendon_sac", "material": "tendon_highlight"})

    return cubes

    return cubes

    return cubes


def part_04_qi_light_spots(variant: str = "empty") -> List[dict]:
    """侧面亮斑 #f6dcc4 2×2 (qi_glow，浮出 0.1px)：
    - prep_buffer_empty：每面 0 个；
    - prep_buffer_full：每面 3 个（竖排！低中高纵向排布）。
    在南面、西面、东面三侧清晰呈现竖排光点。
    """
    cubes = []
    pfx = f"{variant}_"

    if variant == "empty":
        return cubes

    # 侧面中心竖排 3 个 2x2 光斑坐标 (横向 u in [-1.0, 1.0])
    # 低位 y: 3.5..5.5，中位 y: 6.5..8.5，高位 y: 9.5..11.5
    y_spots = [
        ("bot", 3.5, 5.5),
        ("mid", 6.5, 8.5),
        ("top", 9.5, 11.5),
    ]

    for sname, y0, y1 in y_spots:
        # 南面 (+Z，外浮出 0.08px: z in [2.95, 3.03])
        cubes.append({"name": f"{pfx}spot_s_{sname}", "from": [-1.0, y0, 2.95], "to": [1.0, y1, 3.03], "group": "qi_spots", "material": "qi_glow"})
        # 西面 (-X，外浮出 0.08px: x in [-3.03, -2.95])
        cubes.append({"name": f"{pfx}spot_w_{sname}", "from": [-3.03, y0, -1.0], "to": [-2.95, y1, 1.0], "group": "qi_spots", "material": "qi_glow"})
        # 东面 (+X，外浮出 0.08px: x in [2.95, 3.03])
        cubes.append({"name": f"{pfx}spot_e_{sname}", "from": [ 2.95, y0, -1.0], "to": [ 3.03, y1, 1.0], "group": "qi_spots", "material": "qi_glow"})

    return cubes


def part_05_inlet_and_outlet(variant: str = "empty") -> List[dict]:
    """进出口系统：
    1. 底面中心 8×6 统一截面的入口（-Z 面，底边离地 2px: x in [-4, 4], y in [2, 8]）；
       外包 1px 厚肉套 #8a2a2a，方口内壁衬 #b05050，贯通至方块边界 z = -8.0；
    2. 顶面中心 b01 同款小骨环出口 (#d8ccb0 bone_main，外 3×3，内孔 1×1，微凸 0.5px，深色底 #5a1a1a)。
    """
    cubes = []
    pfx = f"{variant}_"

    # ── 1. -Z 面 8×6 统一截面入口 (z in [-8.0, -5.05]，止于北面外壁 z=-5.05，严格不与北壁共面) ──
    # 外围四段肉质套管壁 (#8a2a2a flesh_main，外廓微缩 0.05px 避开边缘: x in [-4.95, 4.95], y in [1.05, 8.95])
    # 底管壁 (y: 1.05..2.0, x: -4.95..4.95)
    cubes.append({"name": f"{pfx}inlet_cuff_bot", "from": [-4.95, 1.05, -8.0], "to": [ 4.95, 2.0, -5.05], "group": "inlet_outlet", "material": "flesh_main"})
    # 顶管壁 (y: 8.0..8.95, x: -4.95..4.95)
    cubes.append({"name": f"{pfx}inlet_cuff_top", "from": [-4.95, 8.0, -8.0], "to": [ 4.95, 8.95, -5.05], "group": "inlet_outlet", "material": "flesh_main"})
    # 左管壁 (x: -4.95..-4.0, y: 2.0..8.0)
    cubes.append({"name": f"{pfx}inlet_cuff_l",   "from": [-4.95, 2.0, -8.0], "to": [-4.0, 8.0, -5.05], "group": "inlet_outlet", "material": "flesh_main"})
    # 右管壁 (x: 4.0..4.95, y: 2.0..8.0)
    cubes.append({"name": f"{pfx}inlet_cuff_r",   "from": [ 4.0, 2.0, -8.0], "to": [ 4.95, 8.0, -5.05], "group": "inlet_outlet", "material": "flesh_main"})

    # 8x6 方孔内壁贴亮肉衬层 (#b05050 flesh_lit，薄 0.08px 衬层)
    # 底内衬
    cubes.append({"name": f"{pfx}inlet_lining_bot", "from": [-4.0, 2.0, -8.0], "to": [ 4.0, 2.08, -5.08], "group": "inlet_outlet", "material": "flesh_lit"})
    # 顶内衬
    cubes.append({"name": f"{pfx}inlet_lining_top", "from": [-4.0, 7.92, -8.0], "to": [ 4.0, 8.0, -5.08], "group": "inlet_outlet", "material": "flesh_lit"})
    # 左内衬
    cubes.append({"name": f"{pfx}inlet_lining_l",   "from": [-4.0, 2.08, -8.0], "to": [-3.92, 7.92, -5.08], "group": "inlet_outlet", "material": "flesh_lit"})
    # 右内衬
    cubes.append({"name": f"{pfx}inlet_lining_r",   "from": [ 3.92, 2.08, -8.0], "to": [ 4.0, 7.92, -5.08], "group": "inlet_outlet", "material": "flesh_lit"})

    # ── 2. 顶面中心肉质颈管 (#8a2a2a flesh_main，对齐参考图与 b04 规范) ──
    # 颈管下段 (y: 12.0..14.5，截面 4x4: x in [-2, 2], z in [-2, 2])
    cubes.append({"name": f"{pfx}top_neck_lower", "from": [-2.0, 12.0, -2.0], "to": [2.0, 14.5, 2.0], "group": "inlet_outlet", "material": "flesh_main"})
    # 颈管上段空心中空管头 (y: 14.5..16.0，截面 4x4 微向 +Z 偏转: x in [-2, 2], z in [-1.6, 2.4]，内孔 x in [-1, 1], z in [-0.6, 1.4])
    cubes.append({"name": f"{pfx}top_neck_wall_s", "from": [-2.0, 14.5,  1.4], "to": [ 2.0, 16.0,  2.4], "group": "inlet_outlet", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_neck_wall_n", "from": [-2.0, 14.5, -1.6], "to": [ 2.0, 16.0, -0.6], "group": "inlet_outlet", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_neck_wall_w", "from": [-2.0, 14.5, -0.6], "to": [-1.0, 16.0,  1.4], "group": "inlet_outlet", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_neck_wall_e", "from": [ 1.0, 14.5, -0.6], "to": [ 2.0, 16.0,  1.4], "group": "inlet_outlet", "material": "flesh_main"})
    # 顶端中空孔底板 (#5a1a1a flesh_dark，深陷 1.5px: y in [14.45, 14.55])
    cubes.append({"name": f"{pfx}top_neck_hole_bot", "from": [-1.0, 14.45, -0.6], "to": [1.0, 14.55, 1.4], "group": "inlet_outlet", "material": "flesh_dark"})

    return cubes


def all_cubes(variant: str = "empty") -> List[dict]:
    """汇总指定变体所有部件的立方体。"""
    return (
        part_01_flesh_ribs(variant)
        + part_02_bone_corner_caps(variant)
        + part_03_tendon_sac(variant)
        + part_04_qi_light_spots(variant)
        + part_05_inlet_and_outlet(variant)
    )


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

        if "tendon" in mat_name or "flesh" in mat_name:
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


def generate_bbmodel(variant: str, out_path: Path) -> Path:
    """导出指定变体的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(variant)
    _assert_no_coplanar_faces(cubes)

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for c in cubes:
        f = c["from"]
        t = c["to"]
        mat = c.get("material", "tendon_tube")
        uv = MAT_UV.get(mat, [16, 16, 32, 32])
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

    name = f"prep_buffer_{variant}"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "buffer")
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

def render_views(paths: Dict[str, Path]):
    """输出包含（两变体 3/4 视 + 侧视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各视角：3/4 等轴视 (Isometric)、侧视 (SIDE: 看竖排光斑与底面8x6入口)
    imgs_iso = {}
    imgs_side = {}
    for var, p in paths.items():
        im_iso, _ = render(p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
        im_side, _ = render(p, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
        imgs_iso[var] = im_iso
        imgs_side[var] = im_side

    # 2. 拼装 render.png (2 行 2 列：上行两变体 3/4 视角，下行两变体侧面视角直观对比 0 个 vs 竖排 3 个光斑)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        # 行 1: 3/4 Isometric (看立式构型、骨节帽、8x6入口、小骨环出口)
        ("EMPTY · 3/4 Isometric (0 light spots)", imgs_iso["empty"], 10, 10),
        ("FULL · 3/4 Isometric (3 vertical light spots)", imgs_iso["full"], cell_w + 20, 10),
        # 行 2: SIDE (看侧面 0 个 vs 竖排 3 个 2x2 真元光斑)
        ("EMPTY · SIDE (Clean glowing sac wall)", imgs_side["empty"], 10, cell_h + 20),
        ("FULL · SIDE (3 vertical light spots on wall)", imgs_side["full"], cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 380, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (两变体 3/4 视与侧视 2x2 拼版) 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图，右当前两变体 3/4 视角大图并排等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_empty = imgs_iso["empty"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_full  = imgs_iso["full"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b06_prep_buffer.png: Left Empty, Right Full)", fill=(210, 200, 180))

        # 贴右渲染图 (左 empty，右 full)
        rx = ref_w + 24
        check_cv.paste(c_empty, (rx, 34 + 10))
        check_cv.paste(c_full,  (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: prep_buffer_empty, RIGHT: prep_buffer_full | 3/4 Isometric Views)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含 3/4 视角对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：两个变体正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_prep_buffer.py 差分自证...")
    for var in ["empty", "full"]:
        cubes = all_cubes(var)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] prep_buffer_{var} 正常立方体集无共面冲突")

        defect_cubes = list(cubes) + [{
            "name": "inject_coplanar_fail",
            "from": [-5.0, 2.0, -5.0],
            "to":   [-3.0, 12.0, -3.0],
            "material": "flesh_main",
        }]
        caught = False
        try:
            _assert_no_coplanar_faces(defect_cubes)
        except AssertionError as e:
            caught = True
            print(f"  [OK] prep_buffer_{var} 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

        if not caught:
            raise RuntimeError(f"门禁失效: prep_buffer_{var} 注入共面冲突未被拦截!")

    print("✓ gen_prep_buffer.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b06 prep_buffer 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    paths = {}
    for var in ["empty", "full"]:
        p = generate_bbmodel(variant=var, out_path=MODEL_DIR / f"prep_buffer_{var}.bbmodel")
        paths[var] = p

    # 同时生成默认 prep_buffer.bbmodel (默认为 empty) 作为主文件
    generate_bbmodel(variant="empty", out_path=MODEL_DIR / "prep_buffer.bbmodel")
    render_views(paths)


if __name__ == "__main__":
    main()
