#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b04: outer_wall_tank (外壁储罐，含 empty / half / full 三个变体)

风格：A 有机型 (活体血肉、十二条红肉棱、八角骨节帽、半透明方形筋囊、侧面梅花光斑、顶部肉质短颈管)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b04_outer_wall_tank.png
- 调度审 b04 第 1 次（2026-10-09 11:3x）更正规范：
  1. 十二条棱：#8a2a2a 肉条（2×2 截面），表面加 #5a1a1a 暗纹；
  2. 八个角：各一个 #d8ccb0 骨节帽（3×3×3，暗面 #b8a888）；
  3. 筋囊（12×12×12）：暖粉 #d9a08c，每个侧面中间 8×8 区域改为 #e8bca8（像透光的囊壁），表面 2 条 #c07868 筋丝；去掉现在凹进去的「窗」；
  4. 内部载荷改为「贴在囊壁内侧、从外面看得到的光点」：在每个侧面的 8×8 区域上，按变体放 #f6dcc4 的 2×2 亮斑（浮出 0.1px）：
     * _empty: 0 个
     * _half:  每面 2 个
     * _full:  每面 5 个（梅花位）
  5. 顶部：中间一根 #8a2a2a 肉质短颈管（4×4，高 3px，顶端开口内壁 #5a1a1a），替换骨质端口。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/outer_wall_tank")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b04_outer_wall_tank.png")

# 配色表 (完全对齐 meridian_factory.md)
PALETTE = {
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (十二条肉棱、顶部肉质短颈管)
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (棱表面暗纹、短颈管孔底)
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (八个角 3×3×3 骨节帽)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 (骨节帽暗面)
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 筋管暖粉 (12×12 筋囊外缘)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋膜透光亮色 (侧面中间 8×8 透光囊壁)
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 筋丝 (囊壁表面斜筋丝)
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元光斑 (2×2 亮斑：0/2/5 梅花位)
}

MAT_UV = {
    "flesh_main":       [0, 0, 16, 16],
    "flesh_dark":       [16, 0, 32, 16],
    "bone_main":        [32, 0, 48, 16],
    "bone_dark":        [48, 0, 64, 16],
    "tendon_tube":      [0, 16, 16, 32],
    "tendon_highlight": [16, 16, 32, 32],
    "tendon_fiber":     [32, 16, 48, 32],
    "qi_glow":          [48, 16, 64, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_ribs(variant: str = "empty") -> List[dict]:
    """十二条棱：#8a2a2a 肉条（2×2 截面），表面加 #5a1a1a 暗纹。
    肉条位于角部 3×3×3 骨节帽之间（截面 2×2，长 10px），榫卯连接。
    """
    cubes = []
    pfx = f"{variant}_"

    # 1. 四根竖向肉棱 (截面 2x2，y: 3.0..13.0，高 10px)
    cubes.append({"name": f"{pfx}rib_vert_nw", "from": [-8.0, 3.0, -8.0], "to": [-6.0, 13.0, -6.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}rib_vert_ne", "from": [ 6.0, 3.0, -8.0], "to": [ 8.0, 13.0, -6.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}rib_vert_sw", "from": [-8.0, 3.0,  6.0], "to": [-6.0, 13.0,  8.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}rib_vert_se", "from": [ 6.0, 3.0,  6.0], "to": [ 8.0, 13.0,  8.0], "group": "flesh_ribs", "material": "flesh_main"})

    # 2. 底面四条水平横肉棱 (截面 2x2，y: 0.0..2.0，长 10px: x/z in [-5.0, 5.0])
    cubes.append({"name": f"{pfx}bot_rib_n", "from": [-5.0, 0.0, -8.0], "to": [ 5.0, 2.0, -6.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}bot_rib_s", "from": [-5.0, 0.0,  6.0], "to": [ 5.0, 2.0,  8.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}bot_rib_w", "from": [-8.0, 0.0, -5.0], "to": [-6.0, 2.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}bot_rib_e", "from": [ 6.0, 0.0, -5.0], "to": [ 8.0, 2.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})

    # 3. 顶面四条水平横肉棱 (截面 2x2，y: 14.0..16.0，长 10px: x/z in [-5.0, 5.0])
    cubes.append({"name": f"{pfx}top_rib_n", "from": [-5.0, 14.0, -8.0], "to": [ 5.0, 16.0, -6.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_rib_s", "from": [-5.0, 14.0,  6.0], "to": [ 5.0, 16.0,  8.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_rib_w", "from": [-8.0, 14.0, -5.0], "to": [-6.0, 16.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}top_rib_e", "from": [ 6.0, 14.0, -5.0], "to": [ 8.0, 16.0,  5.0], "group": "flesh_ribs", "material": "flesh_main"})

    # 4. 肉条表面加 #5a1a1a 暗纹 (四根竖棱中部各贴 2 条微凸 0.05px 的暗纹条，长宽严格在肉条内)
    for y_pos, idx in [(5.5, 1), (9.5, 2)]:
        # NW 竖棱外暗纹
        cubes.append({"name": f"{pfx}rib_stripe_nw_{idx}", "from": [-8.05, y_pos, -8.05], "to": [-5.95, y_pos + 1.0, -5.95], "group": "flesh_ribs", "material": "flesh_dark"})
        # NE 竖棱外暗纹
        cubes.append({"name": f"{pfx}rib_stripe_ne_{idx}", "from": [ 5.95, y_pos, -8.05], "to": [ 8.05, y_pos + 1.0, -5.95], "group": "flesh_ribs", "material": "flesh_dark"})
        # SW 竖棱外暗纹
        cubes.append({"name": f"{pfx}rib_stripe_sw_{idx}", "from": [-8.05, y_pos,  5.95], "to": [-5.95, y_pos + 1.0,  8.05], "group": "flesh_ribs", "material": "flesh_dark"})
        # SE 竖棱外暗纹
        cubes.append({"name": f"{pfx}rib_stripe_se_{idx}", "from": [ 5.95, y_pos,  5.95], "to": [ 8.05, y_pos + 1.0,  8.05], "group": "flesh_ribs", "material": "flesh_dark"})

    return cubes


def part_02_bone_corner_caps(variant: str = "empty") -> List[dict]:
    """八个角各一个 #d8ccb0 骨节帽（3×3×3，暗面 #b8a888）。
    八个角完整包住顶点角（3×3×3 范围），并带暗面关节骨纹。
    """
    cubes = []
    pfx = f"{variant}_"

    corners = [
        # (name_suffix, x0, x1, y0, y1, z0, z1, sx0, sx1, sy0, sy1, sz0, sz1)
        # 底面 4 角 (y in [0.0, 3.0]，暗面条带在 x 轴 [-7.5, -5.5] 或 [5.5, 7.5]，高度 [1.0, 2.0]，厚度微凸 0.06px，四边严格不触碰边界)
        ("bot_nw", -8.0, -5.0,  0.0,  3.0, -8.0, -5.0, -7.5, -5.5,  1.0,  2.0, -8.06, -8.0),
        ("bot_ne",  5.0,  8.0,  0.0,  3.0, -8.0, -5.0,  5.5,  7.5,  1.0,  2.0, -8.06, -8.0),
        ("bot_sw", -8.0, -5.0,  0.0,  3.0,  5.0,  8.0, -7.5, -5.5,  1.0,  2.0,  8.0,   8.06),
        ("bot_se",  5.0,  8.0,  0.0,  3.0,  5.0,  8.0,  5.5,  7.5,  1.0,  2.0,  8.0,   8.06),
        # 顶面 4 角 (y in [13.0, 16.0]，高度 [14.0, 15.0])
        ("top_nw", -8.0, -5.0, 13.0, 16.0, -8.0, -5.0, -7.5, -5.5, 14.0, 15.0, -8.06, -8.0),
        ("top_ne",  5.0,  8.0, 13.0, 16.0, -8.0, -5.0,  5.5,  7.5, 14.0, 15.0, -8.06, -8.0),
        ("top_sw", -8.0, -5.0, 13.0, 16.0,  5.0,  8.0, -7.5, -5.5, 14.0, 15.0,  8.0,   8.06),
        ("top_se",  5.0,  8.0, 13.0, 16.0,  5.0,  8.0,  5.5,  7.5, 14.0, 15.0,  8.0,   8.06),
    ]

    for cname, x0, x1, y0, y1, z0, z1, sx0, sx1, sy0, sy1, sz0, sz1 in corners:
        # 骨节帽主体 (3x3x3, #d8ccb0 bone_main)
        cubes.append({"name": f"{pfx}bone_cap_{cname}", "from": [x0, y0, z0], "to": [x1, y1, z1], "group": "bone_caps", "material": "bone_main"})
        # 骨节帽表面暗面条带 (#b8a888 bone_dark，微凸 0.05px)
        cubes.append({"name": f"{pfx}bone_cap_dark_{cname}", "from": [sx0, sy0, sz0], "to": [sx1, sy1, sz1], "group": "bone_caps", "material": "bone_dark"})

    return cubes


def part_03_tendon_sac(variant: str = "empty") -> List[dict]:
    """筋囊 (12×12×12，x/z in [-6, 6], y in [2, 14])：
    - 暖粉 #d9a08c (tendon_tube)；
    - 每个侧面中间 8×8 区域改为 #e8bca8 (tendon_highlight，像透光的囊壁)；
    - 表面 2 条 #c07868 (tendon_fiber) 斜筋丝；
    - 彻底去掉凹进去的「窗」，侧壁整体平整饱满。
    """
    cubes = []
    pfx = f"{variant}_"

    # 筋囊底板 (y in [2.0, 2.2]) 与 顶板 (y in [13.8, 14.0])，避开骨角帽相交处
    cubes.append({"name": f"{pfx}sac_bottom_plate", "from": [-4.95, 2.0, -4.95], "to": [4.95, 2.2, 4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_top_plate",    "from": [-4.95, 13.8, -4.95], "to": [4.95, 14.0, 4.95], "group": "tendon_sac", "material": "tendon_tube"})

    # 4 个侧面结构 (平整外立面位于 z = ±6.0 和 x = ±6.0)：
    # ── 1. 正面 (+Z，外表面位于 z = 6.0) ──
    # 四周 2px 暖粉筋管边框 (#d9a08c tendon_tube)
    cubes.append({"name": f"{pfx}sac_s_frame_bot", "from": [-4.95, 2.2, 5.2], "to": [ 4.95, 4.0, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_s_frame_top", "from": [-4.95, 12.0, 5.2], "to": [ 4.95, 13.8, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_s_frame_l",   "from": [-4.95, 4.0, 5.2], "to": [-4.0, 12.0, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_s_frame_r",   "from": [ 4.0, 4.0, 5.2], "to": [ 4.95, 12.0, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    # 中间 8×8 透光囊壁 (#e8bca8 tendon_highlight，厚 0.8px: z in [5.2, 6.0]，外表面平齐)
    cubes.append({"name": f"{pfx}sac_s_glow_wall", "from": [-4.0, 4.0, 5.2], "to": [ 4.0, 12.0, 6.0], "group": "tendon_sac", "material": "tendon_highlight"})
    # 表面 2 条 #c07868 斜筋丝 (微浮出 0.05px: z 6.0..6.05)
    cubes.append({"name": f"{pfx}sac_s_fiber_1", "from": [-4.8, 2.5, 6.0], "to": [-3.2, 4.5, 6.05], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_s_fiber_2", "from": [ 3.2, 11.5, 6.0], "to": [ 4.8, 13.5, 6.05], "group": "tendon_sac", "material": "tendon_fiber"})

    # ── 2. 背面 (-Z，外表面位于 z = -6.0) ──
    cubes.append({"name": f"{pfx}sac_n_frame_bot", "from": [-4.95, 2.2, -6.0], "to": [ 4.95, 4.0, -5.2], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_frame_top", "from": [-4.95, 12.0, -6.0], "to": [ 4.95, 13.8, -5.2], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_frame_l",   "from": [-4.95, 4.0, -6.0], "to": [-4.0, 12.0, -5.2], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_frame_r",   "from": [ 4.0, 4.0, -6.0], "to": [ 4.95, 12.0, -5.2], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_glow_wall", "from": [-4.0, 4.0, -6.0], "to": [ 4.0, 12.0, -5.2], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}sac_n_fiber_1", "from": [-4.8, 2.5, -6.05], "to": [-3.2, 4.5, -6.0], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_n_fiber_2", "from": [ 3.2, 11.5, -6.05], "to": [ 4.8, 13.5, -6.0], "group": "tendon_sac", "material": "tendon_fiber"})

    # ── 3. 西侧面 (-X，外表面位于 x = -6.0) ──
    cubes.append({"name": f"{pfx}sac_w_frame_bot", "from": [-6.0, 2.2, -4.95], "to": [-5.2, 4.0,  4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_frame_top", "from": [-6.0, 12.0, -4.95], "to": [-5.2, 13.8,  4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_frame_l",   "from": [-6.0, 4.0, -4.95], "to": [-5.2, 12.0, -4.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_frame_r",   "from": [-6.0, 4.0,  4.0], "to": [-5.2, 12.0,  4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_glow_wall", "from": [-6.0, 4.0, -4.0], "to": [-5.2, 12.0,  4.0], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}sac_w_fiber_1", "from": [-6.05, 2.5, -4.8], "to": [-6.0, 4.5, -3.2], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_w_fiber_2", "from": [-6.05, 11.5,  3.2], "to": [-6.0, 13.5,  4.8], "group": "tendon_sac", "material": "tendon_fiber"})

    # ── 4. 东侧面 (+X，外表面位于 x = 6.0) ──
    cubes.append({"name": f"{pfx}sac_e_frame_bot", "from": [ 5.2, 2.2, -4.95], "to": [ 6.0, 4.0,  4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_frame_top", "from": [ 5.2, 12.0, -4.95], "to": [ 6.0, 13.8,  4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_frame_l",   "from": [ 5.2, 4.0, -4.95], "to": [ 6.0, 12.0, -4.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_frame_r",   "from": [ 5.2, 4.0,  4.0], "to": [ 6.0, 12.0,  4.95], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_glow_wall", "from": [ 5.2, 4.0, -4.0], "to": [ 6.0, 12.0,  4.0], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}sac_e_fiber_1", "from": [ 6.0, 2.5, -4.8], "to": [ 6.05, 4.5, -3.2], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}sac_e_fiber_2", "from": [ 6.0, 11.5,  3.2], "to": [ 6.05, 13.5,  4.8], "group": "tendon_sac", "material": "tendon_fiber"})

    return cubes


def part_04_qi_light_spots(variant: str = "empty") -> List[dict]:
    """内部载荷改为「贴在囊壁内侧、从外面看得到的光点」：
    在每个侧面的 8×8 区域上，按变体放 #f6dcc4 的 2×2 亮斑（浮出 0.1px: 厚 0.08px）：
    - outer_wall_tank_empty: 0 个
    - outer_wall_tank_half:  每面 2 个 (下半部左右对称)
    - outer_wall_tank_full:  每面 5 个 (标准梅花位: 四角各一 + 中心一个)
    """
    cubes = []
    pfx = f"{variant}_"

    if variant == "empty":
        return cubes

    # 4 个侧面的梅花位坐标定义 (横向 u in [-4, 4], 纵向 y in [4, 12])
    # 亮斑大小严格为 2x2
    # half: 2 个 (下半部)
    spots_2 = [
        ("bot_l", -3.0, -1.0, 5.0, 7.0),
        ("bot_r",  1.0,  3.0, 5.0, 7.0),
    ]

    # full: 5 个 (梅花位)
    spots_5 = [
        ("bot_l", -3.0, -1.0,  5.0,  7.0),
        ("bot_r",  1.0,  3.0,  5.0,  7.0),
        ("center", -1.0,  1.0,  7.0,  9.0),
        ("top_l", -3.0, -1.0,  9.0, 11.0),
        ("top_r",  1.0,  3.0,  9.0, 11.0),
    ]

    active_spots = spots_5 if variant == "full" else spots_2

    for sname, u0, u1, y0, y1 in active_spots:
        # 正面 (+Z，外浮出 0.1px: z in [6.1, 6.2])
        cubes.append({"name": f"{pfx}spot_s_{sname}", "from": [u0, y0, 6.1], "to": [u1, y1, 6.2], "group": "qi_spots", "material": "qi_glow"})
        # 背面 (-Z，外浮出 0.1px: z in [-6.2, -6.1])
        cubes.append({"name": f"{pfx}spot_n_{sname}", "from": [u0, y0, -6.2], "to": [u1, y1, -6.1], "group": "qi_spots", "material": "qi_glow"})
        # 西面 (-X，外浮出 0.1px: x in [-6.2, -6.1])
        cubes.append({"name": f"{pfx}spot_w_{sname}", "from": [-6.2, y0, u0], "to": [-6.1, y1, u1], "group": "qi_spots", "material": "qi_glow"})
        # 东面 (+X，外浮出 0.1px: x in [6.1, 6.2])
        cubes.append({"name": f"{pfx}spot_e_{sname}", "from": [ 6.1, y0, u0], "to": [ 6.2, y1, u1], "group": "qi_spots", "material": "qi_glow"})

    return cubes


def part_05_flesh_neck(variant: str = "empty") -> List[dict]:
    """顶部：中间一根 #8a2a2a 肉质短颈管（4×4，高 3px: y in [14.0, 17.0]，顶端开口内壁 #5a1a1a）。"""
    cubes = []
    pfx = f"{variant}_"

    # 四段肉管壁 (外 4x4: x in [-2, 2], z in [-2, 2]，全高 y in [14.0, 17.0])
    cubes.append({"name": f"{pfx}neck_wall_n", "from": [-2.0, 14.0, -2.0], "to": [ 2.0, 17.0, -1.0], "group": "top_neck", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}neck_wall_s", "from": [-2.0, 14.0,  1.0], "to": [ 2.0, 17.0,  2.0], "group": "top_neck", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}neck_wall_w", "from": [-2.0, 14.0, -1.0], "to": [-1.0, 17.0,  1.0], "group": "top_neck", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}neck_wall_e", "from": [ 1.0, 14.0, -1.0], "to": [ 2.0, 17.0,  1.0], "group": "top_neck", "material": "flesh_main"})

    # 内壁深陷孔底 (内孔 2x2: x/z in [-1.0, 1.0]，深陷 y: 14.0..15.9，材质 #5a1a1a flesh_dark)
    cubes.append({"name": f"{pfx}neck_dark_hole", "from": [-1.0, 14.0, -1.0], "to": [1.0, 15.9, 1.0], "group": "top_neck", "material": "flesh_dark"})

    return cubes


def all_cubes(variant: str = "empty") -> List[dict]:
    """汇总指定变体所有部件的立方体。"""
    return (
        part_01_flesh_ribs(variant)
        + part_02_bone_corner_caps(variant)
        + part_03_tendon_sac(variant)
        + part_04_qi_light_spots(variant)
        + part_05_flesh_neck(variant)
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
        uv = MAT_UV.get(mat, [0, 16, 16, 32])
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

    name = f"outer_wall_tank_{variant}"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "tank")
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
    """输出三变体并排 + 3/4 视角与正视角的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各变体视角：3/4 等轴视与正面直视
    imgs_iso = {}
    imgs_front = {}
    for var, p in paths.items():
        im_iso, _ = render(p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
        im_front, _ = render(p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
        imgs_iso[var] = im_iso
        imgs_front[var] = im_front

    # 2. 拼装 render.png (2 行 3 列：上行三变体 3/4 视角，下行三变体正视视角直观呈现 0/2/5 梅花亮斑)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 3 + 40
    canvas_h = cell_h * 2 + 40
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        # 行 1: 3/4 Isometric (看红肉棱、八角骨节帽、筋囊、顶部肉颈管)
        ("EMPTY · 3/4 Isometric (0 light spots)", imgs_iso["empty"], 10, 10),
        ("HALF · 3/4 Isometric (2 light spots per face)", imgs_iso["half"], cell_w + 20, 10),
        ("FULL · 3/4 Isometric (5 light spots plum blossom)", imgs_iso["full"], cell_w * 2 + 30, 10),
        # 行 2: FRONT (看囊壁 8x8 透光区内 0 / 2 / 5 梅花光斑)
        ("EMPTY · FRONT (Clean glowing sac wall)", imgs_front["empty"], 10, cell_h + 20),
        ("HALF · FRONT (2 light spots visible)", imgs_front["half"], cell_w + 20, cell_h + 20),
        ("FULL · FRONT (5 plum blossom light spots)", imgs_front["full"], cell_w * 2 + 30, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 380, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (三变体 3/4 视与正视对比) 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图，右当前三变体 3/4 视角大图并排等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_empty = imgs_iso["empty"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_half  = imgs_iso["half"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_full  = imgs_iso["full"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 3 + 24
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b04_outer_wall_tank.png)", fill=(210, 200, 180))

        # 贴右渲染图 (三变体 3/4 视角并排)
        rx = ref_w + 24
        check_cv.paste(c_empty, (rx, 34 + 10))
        check_cv.paste(c_half,  (rx + thumb_size + 8, 34 + 10))
        check_cv.paste(c_full,  (rx + (thumb_size + 8) * 2, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: empty · 0 spots, MID: half · 2 spots, RIGHT: full · 5 spots | 3/4 Isometric Views)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含 3/4 视角对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：三个变体正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_outer_wall_tank.py 差分自证...")
    for var in ["empty", "half", "full"]:
        cubes = all_cubes(var)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] outer_wall_tank_{var} 正常立方体集无共面冲突")

        defect_cubes = list(cubes) + [{
            "name": "inject_coplanar_fail",
            "from": [-8.0, 3.0, -8.0],
            "to":   [-6.0, 13.0, -6.0],
            "material": "flesh_main",
        }]
        caught = False
        try:
            _assert_no_coplanar_faces(defect_cubes)
        except AssertionError as e:
            caught = True
            print(f"  [OK] outer_wall_tank_{var} 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

        if not caught:
            raise RuntimeError(f"门禁失效: outer_wall_tank_{var} 注入共面冲突未被拦截!")

    print("✓ gen_outer_wall_tank.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b04 outer_wall_tank 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    paths = {}
    for var in ["empty", "half", "full"]:
        p = generate_bbmodel(variant=var, out_path=MODEL_DIR / f"outer_wall_tank_{var}.bbmodel")
        paths[var] = p

    # 同时生成默认 outer_wall_tank.bbmodel (默认为 empty) 作为主文件
    generate_bbmodel(variant="empty", out_path=MODEL_DIR / "outer_wall_tank.bbmodel")
    render_views(paths)


if __name__ == "__main__":
    main()
