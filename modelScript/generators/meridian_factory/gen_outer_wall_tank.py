#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b04: outer_wall_tank (外壁储罐，含 empty / half / full 三个变体)

风格：A 有机型 (活体血肉、骨架支撑、半透明方形筋囊、qi 载荷小方块)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b04_outer_wall_tank.png

调度要求（整格 16×16×16）：
1. 外部骨架：
   - 四根竖骨肋 (#d8ccb0 bone_main，2×16×2，在四个角，带 #b8a888 节纹)；
   - 上下各一圈骨框 (#d8ccb0，宽 2px，连接四角竖骨肋)。
2. 中间方形筋囊：
   - 暖粉 #d9a08c (tendon_tube)，尺寸 12×12×12 (x: -6..6, y: 2..14, z: -6..6)；
   - 侧面上下各 1px #e8bca8 (tendon_highlight) 亮边；
   - 表面 2 条 #c07868 (tendon_fiber) 斜筋丝；
   - 侧面正中各留一个 6×6 的「窗」(凹进 1px，用 #f6dcc4 qi_glow) 能看到里面。
3. 内部 qi 载荷小方块 (4×4×4，#f2f0ea qi_packet)：
   - 三个变体：
     * outer_wall_tank_empty：0 块
     * outer_wall_tank_half：4 块铺底 (2×2 阵列，y: 2.1..6.0)
     * outer_wall_tank_full：8 块两层 (底层 4 块 + 顶层 4 块，y: 2.1..10.0)
     只改内部方块数量。
4. 顶面中心一个 b01 同款小骨环端口 (#d8ccb0，外 3×3，内孔 1×1，深陷暗孔 #5a1a1a)。
5. 门禁验证与交付物：
   - 几何体共面自检 0 冲突，--self-test 差分注入缺陷拦截自测全绿；
   - 特征点名器 Manifest 全视角全绿，所有材质全部上镜；
   - render.png (三变体并排 + 3/4 视角)、check.png 落盘到 model-review/img/meridian_factory/outer_wall_tank/。
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

# 配色表
PALETTE = {
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (四根竖骨肋、上下骨框、小骨环)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 (竖骨肋节纹)
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 筋管暖粉 (方形筋囊外壁)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋膜亮粉 (侧面上下 1px 亮边)
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 筋丝 (表面斜筋丝)
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 内光淡黄 (6×6 视窗)
    "qi_packet":        (242, 240, 234, 255), # #f2f0ea 白玉真元方块 (内部 4×4×4 载荷)
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗孔底
}

MAT_UV = {
    "bone_main":        [0, 0, 16, 16],
    "bone_dark":        [16, 0, 32, 16],
    "tendon_tube":      [32, 0, 48, 16],
    "tendon_highlight": [48, 0, 64, 16],
    "tendon_fiber":     [0, 16, 16, 32],
    "qi_glow":          [16, 16, 32, 32],
    "qi_packet":        [32, 16, 48, 32],
    "flesh_dark":       [48, 16, 64, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_bone_frame(variant: str = "empty") -> List[dict]:
    """外部骨架：四根竖骨肋 (2×16×2，在四个角，带 #b8a888 节纹) + 上下各一圈骨框 (宽 2px)。"""
    cubes = []
    pfx = f"{variant}_"

    # 1. 四根竖骨肋 (截面 2x2，全高 y: 0.0..16.0)
    # NW: x in [-8, -6], z in [-8, -6]
    cubes.append({"name": f"{pfx}rib_nw", "from": [-8.0, 0.0, -8.0], "to": [-6.0, 16.0, -6.0], "group": "bone_frame", "material": "bone_main"})
    # NE: x in [6, 8], z in [-8, -6]
    cubes.append({"name": f"{pfx}rib_ne", "from": [ 6.0, 0.0, -8.0], "to": [ 8.0, 16.0, -6.0], "group": "bone_frame", "material": "bone_main"})
    # SW: x in [-8, -6], z in [6, 8]
    cubes.append({"name": f"{pfx}rib_sw", "from": [-8.0, 0.0,  6.0], "to": [-6.0, 16.0,  8.0], "group": "bone_frame", "material": "bone_main"})
    # SE: x in [6, 8], z in [6, 8]
    cubes.append({"name": f"{pfx}rib_se", "from": [ 6.0, 0.0,  6.0], "to": [ 8.0, 16.0,  8.0], "group": "bone_frame", "material": "bone_main"})

    # 竖骨肋节纹 (#b8a888 bone_dark，分别在 y=5.0 与 y=10.0 处，高 0.8px，微外凸 0.05px)
    for y_pos, idx in [(5.0, 1), (10.0, 2)]:
        # NW 角节纹
        cubes.append({"name": f"{pfx}rib_nw_joint{idx}", "from": [-8.05, y_pos, -8.05], "to": [-5.95, y_pos + 0.8, -5.95], "group": "bone_frame", "material": "bone_dark"})
        # NE 角节纹
        cubes.append({"name": f"{pfx}rib_ne_joint{idx}", "from": [ 5.95, y_pos, -8.05], "to": [ 8.05, y_pos + 0.8, -5.95], "group": "bone_frame", "material": "bone_dark"})
        # SW 角节纹
        cubes.append({"name": f"{pfx}rib_sw_joint{idx}", "from": [-8.05, y_pos,  5.95], "to": [-5.95, y_pos + 0.8,  8.05], "group": "bone_frame", "material": "bone_dark"})
        # SE 角节纹
        cubes.append({"name": f"{pfx}rib_se_joint{idx}", "from": [ 5.95, y_pos,  5.95], "to": [ 8.05, y_pos + 0.8,  8.05], "group": "bone_frame", "material": "bone_dark"})

    # 2. 上下各一圈骨框 (宽 2px，厚 2px)
    # 底骨框 (y: 0.0..2.0)
    cubes.append({"name": f"{pfx}bottom_frame_n", "from": [-6.0, 0.0, -8.0], "to": [ 6.0, 2.0, -6.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bottom_frame_s", "from": [-6.0, 0.0,  6.0], "to": [ 6.0, 2.0,  8.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bottom_frame_w", "from": [-8.0, 0.0, -6.0], "to": [-6.0, 2.0,  6.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bottom_frame_e", "from": [ 6.0, 0.0, -6.0], "to": [ 8.0, 2.0,  6.0], "group": "bone_frame", "material": "bone_main"})

    # 顶骨框 (y: 14.0..16.0)
    cubes.append({"name": f"{pfx}top_frame_n", "from": [-6.0, 14.0, -8.0], "to": [ 6.0, 16.0, -6.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": f"{pfx}top_frame_s", "from": [-6.0, 14.0,  6.0], "to": [ 6.0, 16.0,  8.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": f"{pfx}top_frame_w", "from": [-8.0, 14.0, -6.0], "to": [-6.0, 16.0,  6.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": f"{pfx}top_frame_e", "from": [ 6.0, 14.0, -6.0], "to": [ 8.0, 16.0,  6.0], "group": "bone_frame", "material": "bone_main"})

    return cubes


def part_02_tendon_sac(variant: str = "empty") -> List[dict]:
    """中间方形筋囊 (暖粉 #d9a08c，12×12×12，x/z in [-6, 6], y in [2, 14])：
    - 侧面上下各 1px #e8bca8 亮边 (y: 2..3 与 y: 13..14)；
    - 表面 2 条 #c07868 斜筋丝；
    - 侧面正中各留一个 6×6 的「窗」(凹进 1px，用 #f6dcc4 qi_glow) 能看到里面。
    """
    cubes = []
    pfx = f"{variant}_"

    # 底板 (y in [2.0, 2.2]) 与 顶板 (y in [13.8, 14.0])
    cubes.append({"name": f"{pfx}sac_bottom_plate", "from": [-5.9, 2.0, -5.9], "to": [5.9, 2.2, 5.9], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_top_plate",    "from": [-5.9, 13.8, -5.9], "to": [5.9, 14.0, 5.9], "group": "tendon_sac", "material": "tendon_tube"})

    # 4 个侧面围绕 6×6 窗口 (x/z in [-3, 3], y in [5, 11]) 的外壁结构：
    # ── 正面 (+Z，外表面位于 z = 6.0，凹窗位于 z = 5.0，四周环绕 #f6dcc4 窗框，中空透视内部) ──
    cubes.append({"name": f"{pfx}sac_s_bot", "from": [-5.9, 2.2, 5.0], "to": [5.9, 5.0, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_s_top", "from": [-5.9, 11.0, 5.0], "to": [5.9, 13.8, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_s_left", "from": [-5.9, 5.0, 5.0], "to": [-3.0, 11.0, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_s_right", "from": [ 3.0, 5.0, 5.0], "to": [ 5.9, 11.0, 6.0], "group": "tendon_sac", "material": "tendon_tube"})
    # 6×6 凹窗框 (#f6dcc4 qi_glow，凹进 1px: z 4.95..5.05，中空透视内部 qi 载荷)
    cubes.append({"name": f"{pfx}sac_win_s_top", "from": [-3.0, 10.3, 4.95], "to": [ 3.0, 11.0, 5.05], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_s_bot", "from": [-3.0,  5.0, 4.95], "to": [ 3.0,  5.7, 5.05], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_s_l",   "from": [-3.0,  5.7, 4.95], "to": [-2.3, 10.3, 5.05], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_s_r",   "from": [ 2.3,  5.7, 4.95], "to": [ 3.0, 10.3, 5.05], "group": "tendon_sac", "material": "qi_glow"})

    # ── 背面 (-Z，外表面位于 z = -6.0，凹窗位于 z = -5.0) ──
    cubes.append({"name": f"{pfx}sac_n_bot", "from": [-5.9, 2.2, -6.0], "to": [5.9, 5.0, -5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_top", "from": [-5.9, 11.0, -6.0], "to": [5.9, 13.8, -5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_left", "from": [-5.9, 5.0, -6.0], "to": [-3.0, 11.0, -5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_n_right", "from": [ 3.0, 5.0, -6.0], "to": [ 5.9, 11.0, -5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_win_n_top", "from": [-3.0, 10.3, -5.05], "to": [ 3.0, 11.0, -4.95], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_n_bot", "from": [-3.0,  5.0, -5.05], "to": [ 3.0,  5.7, -4.95], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_n_l",   "from": [-3.0,  5.7, -5.05], "to": [-2.3, 10.3, -4.95], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_n_r",   "from": [ 2.3,  5.7, -5.05], "to": [ 3.0, 10.3, -4.95], "group": "tendon_sac", "material": "qi_glow"})

    # ── 西侧面 (-X，外表面位于 x = -6.0，凹窗位于 x = -5.0) ──
    cubes.append({"name": f"{pfx}sac_w_bot", "from": [-6.0, 2.2, -5.0], "to": [-5.0, 5.0, 5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_top", "from": [-6.0, 11.0, -5.0], "to": [-5.0, 13.8, 5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_front", "from": [-6.0, 5.0,  3.0], "to": [-5.0, 11.0, 5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_w_back",  "from": [-6.0, 5.0, -5.0], "to": [-5.0, 11.0, -3.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_win_w_top", "from": [-5.05, 10.3, -3.0], "to": [-4.95, 11.0, 3.0], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_w_bot", "from": [-5.05,  5.0, -3.0], "to": [-4.95,  5.7, 3.0], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_w_l",   "from": [-5.05,  5.7, -3.0], "to": [-4.95, 10.3, -2.3], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_w_r",   "from": [-5.05,  5.7,  2.3], "to": [-4.95, 10.3, 3.0], "group": "tendon_sac", "material": "qi_glow"})

    # ── 东侧面 (+X，外表面位于 x = 6.0，凹窗位于 x = 5.0) ──
    cubes.append({"name": f"{pfx}sac_e_bot", "from": [ 5.0, 2.2, -5.0], "to": [ 6.0, 5.0, 5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_top", "from": [ 5.0, 11.0, -5.0], "to": [ 6.0, 13.8, 5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_front", "from": [ 5.0, 5.0,  3.0], "to": [ 6.0, 11.0, 5.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_e_back",  "from": [ 5.0, 5.0, -5.0], "to": [ 6.0, 11.0, -3.0], "group": "tendon_sac", "material": "tendon_tube"})
    cubes.append({"name": f"{pfx}sac_win_e_top", "from": [ 4.95, 10.3, -3.0], "to": [ 5.05, 11.0, 3.0], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_e_bot", "from": [ 4.95,  5.0, -3.0], "to": [ 5.05,  5.7, 3.0], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_e_l",   "from": [ 4.95,  5.7, -3.0], "to": [ 5.05, 10.3, -2.3], "group": "tendon_sac", "material": "qi_glow"})
    cubes.append({"name": f"{pfx}sac_win_e_r",   "from": [ 4.95,  5.7,  2.3], "to": [ 5.05, 10.3, 3.0], "group": "tendon_sac", "material": "qi_glow"})

    # ── 侧面上下各 1px 亮边 (#e8bca8 tendon_highlight) ──
    # 下亮边 (y: 2.2..3.0，高 0.8px，贴在外立面微凸 0.05px)
    cubes.append({"name": f"{pfx}highlight_s_bot", "from": [-5.8, 2.2, 6.0], "to": [5.8, 3.0, 6.05], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}highlight_n_bot", "from": [-5.8, 2.2, -6.05], "to": [5.8, 3.0, -6.0], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}highlight_w_bot", "from": [-6.05, 2.2, -5.8], "to": [-6.0, 3.0, 5.8], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}highlight_e_bot", "from": [ 6.0, 2.2, -5.8], "to": [ 6.05, 3.0, 5.8], "group": "tendon_sac", "material": "tendon_highlight"})

    # 上亮边 (y: 13.0..13.8，高 0.8px)
    cubes.append({"name": f"{pfx}highlight_s_top", "from": [-5.8, 13.0, 6.0], "to": [5.8, 13.8, 6.05], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}highlight_n_top", "from": [-5.8, 13.0, -6.05], "to": [5.8, 13.8, -6.0], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}highlight_w_top", "from": [-6.05, 13.0, -5.8], "to": [-6.0, 13.8, 5.8], "group": "tendon_sac", "material": "tendon_highlight"})
    cubes.append({"name": f"{pfx}highlight_e_top", "from": [ 6.0, 13.0, -5.8], "to": [ 6.05, 13.8, 5.8], "group": "tendon_sac", "material": "tendon_highlight"})

    # ── 表面 2 条 #c07868 (tendon_fiber) 斜筋丝 ──
    # 在正面 (+Z) 窗框两侧斜跨 2 条筋丝
    cubes.append({"name": f"{pfx}fiber_s1", "from": [-5.5, 3.5, 6.05], "to": [-3.2, 5.5, 6.15], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}fiber_s2", "from": [ 3.2, 10.5, 6.05], "to": [ 5.5, 12.5, 6.15], "group": "tendon_sac", "material": "tendon_fiber"})
    # 在背面 (-Z)
    cubes.append({"name": f"{pfx}fiber_n1", "from": [-5.5, 3.5, -6.15], "to": [-3.2, 5.5, -6.05], "group": "tendon_sac", "material": "tendon_fiber"})
    cubes.append({"name": f"{pfx}fiber_n2", "from": [ 3.2, 10.5, -6.15], "to": [ 5.5, 12.5, -6.05], "group": "tendon_sac", "material": "tendon_fiber"})

    return cubes


def part_03_top_port(variant: str = "empty") -> List[dict]:
    """顶面中心 b01 同款小骨环端口 (#d8ccb0 bone_main)：
    - 外 3×3 (x/z in [-1.5, 1.5])，内孔 1×1 (x/z in [-0.5, 0.5])；
    - 位于顶面中心 y: 15.5..16.5 (微凸出顶骨框 0.5px)；
    - 孔内深陷孔底暗色 #5a1a1a (flesh_dark)。
    """
    cubes = []
    pfx = f"{variant}_"

    # 中心连接骨盘 (y: 14.0..15.8，连接筋囊顶与顶骨环)
    cubes.append({"name": f"{pfx}top_port_riser", "from": [-2.0, 14.0, -2.0], "to": [2.0, 15.8, 2.0], "group": "top_port", "material": "bone_main"})

    # 小骨环端口 4 段 (y: 15.8..16.5，外 3x3，内孔 1x1，高 0.7px，微凸顶骨框 0.5px)
    cubes.append({"name": f"{pfx}port_ring_n", "from": [-1.5, 15.8, -1.5], "to": [ 1.5, 16.5, -0.5], "group": "top_port", "material": "bone_main"})
    cubes.append({"name": f"{pfx}port_ring_s", "from": [-1.5, 15.8,  0.5], "to": [ 1.5, 16.5,  1.5], "group": "top_port", "material": "bone_main"})
    cubes.append({"name": f"{pfx}port_ring_w", "from": [-1.5, 15.8, -0.5], "to": [-0.5, 16.5,  0.5], "group": "top_port", "material": "bone_main"})
    cubes.append({"name": f"{pfx}port_ring_e", "from": [ 0.5, 15.8, -0.5], "to": [ 1.5, 16.5,  0.5], "group": "top_port", "material": "bone_main"})

    # 内孔深孔底 (y: 15.8..16.05，深色 #5a1a1a)
    cubes.append({"name": f"{pfx}port_dark_hole", "from": [-0.5, 15.8, -0.5], "to": [0.5, 16.05, 0.5], "group": "top_port", "material": "flesh_dark"})

    return cubes


def part_04_qi_packets(variant: str = "empty") -> List[dict]:
    """内部 qi 载荷小方块 (4×4×4，#f2f0ea qi_packet)：
    - outer_wall_tank_empty：0 块
    - outer_wall_tank_half：4 块铺底 (2×2 阵列，y: 2.6..6.6)
    - outer_wall_tank_full：8 块两层 (底层 4 块 y: 2.6..6.6 + 顶层 4 块 y: 6.8..10.8)
    为彻底杜绝共面冲突，每块长宽微留 0.1px 缝隙 (3.8×4.0×3.8)。
    """
    cubes = []
    pfx = f"{variant}_"

    # 罐内底托板 (#5a1a1a flesh_dark，y: 2.2..2.6，位于底板之上承托方块)
    cubes.append({"name": f"{pfx}qi_base_pad", "from": [-4.5, 2.2, -4.5], "to": [4.5, 2.6, 4.5], "group": "qi_packets", "material": "flesh_dark"})

    if variant == "empty":
        return cubes

    # 底层 4 块 (y: 2.6..6.6，高 4.0px，在 6×6 视窗中下半部 y: 5.7..6.6 清晰露出)
    # NW: x in [-3.9, -0.1], z in [-3.9, -0.1]
    cubes.append({"name": f"{pfx}qi_b1_nw", "from": [-3.9, 2.6, -3.9], "to": [-0.1, 6.6, -0.1], "group": "qi_packets", "material": "qi_packet"})
    # NE: x in [0.1, 3.9], z in [-3.9, -0.1]
    cubes.append({"name": f"{pfx}qi_b2_ne", "from": [ 0.1, 2.6, -3.9], "to": [ 3.9, 6.6, -0.1], "group": "qi_packets", "material": "qi_packet"})
    # SW: x in [-3.9, -0.1], z in [0.1, 3.9]
    cubes.append({"name": f"{pfx}qi_b3_sw", "from": [-3.9, 2.6,  0.1], "to": [-0.1, 6.6,  3.9], "group": "qi_packets", "material": "qi_packet"})
    # SE: x in [0.1, 3.9], z in [0.1, 3.9]
    cubes.append({"name": f"{pfx}qi_b4_se", "from": [ 0.1, 2.6,  0.1], "to": [ 3.9, 6.6,  3.9], "group": "qi_packets", "material": "qi_packet"})

    if variant == "full":
        # 顶层 4 块 (y: 6.8..10.8，高 4.0px，在 6×6 视窗上半部 y: 6.8..10.3 清晰露出)
        cubes.append({"name": f"{pfx}qi_t1_nw", "from": [-3.9, 6.8, -3.9], "to": [-0.1, 10.8, -0.1], "group": "qi_packets", "material": "qi_packet"})
        cubes.append({"name": f"{pfx}qi_t2_ne", "from": [ 0.1, 6.8, -3.9], "to": [ 3.9, 10.8, -0.1], "group": "qi_packets", "material": "qi_packet"})
        cubes.append({"name": f"{pfx}qi_t3_sw", "from": [-3.9, 6.8,  0.1], "to": [-0.1, 10.8,  3.9], "group": "qi_packets", "material": "qi_packet"})
        cubes.append({"name": f"{pfx}qi_t4_se", "from": [ 0.1, 6.8,  0.1], "to": [ 3.9, 10.8,  3.9], "group": "qi_packets", "material": "qi_packet"})

    return cubes


def all_cubes(variant: str = "empty") -> List[dict]:
    """汇总指定变体所有部件的立方体。"""
    return (
        part_01_bone_frame(variant)
        + part_02_tendon_sac(variant)
        + part_03_top_port(variant)
        + part_04_qi_packets(variant)
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
        uv = MAT_UV.get(mat, [32, 0, 48, 16])
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
    """输出三变体并排 + 3/4 视角的综合拼图 render.png 与左右并排对照卡 check.png。"""
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

    # 2. 拼装 render.png (2 行 3 列：上行三变体 3/4 视角，下行三变体正视视角透视 6x6 视窗与内部 qi 方块)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 3 + 40
    canvas_h = cell_h * 2 + 40
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        # 行 1: 3/4 Isometric (看骨架/窗/顶端口)
        ("EMPTY · 3/4 Isometric (0 qi blocks)", imgs_iso["empty"], 10, 10),
        ("HALF · 3/4 Isometric (4 qi blocks bottom)", imgs_iso["half"], cell_w + 20, 10),
        ("FULL · 3/4 Isometric (8 qi blocks two layers)", imgs_iso["full"], cell_w * 2 + 30, 10),
        # 行 2: FRONT (看 6x6 观察窗内 qi 载荷状态)
        ("EMPTY · FRONT (Empty interior cavity)", imgs_front["empty"], 10, cell_h + 20),
        ("HALF · FRONT (4 blocks visible in window)", imgs_front["half"], cell_w + 20, cell_h + 20),
        ("FULL · FRONT (8 blocks stacked full)", imgs_front["full"], cell_w * 2 + 30, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 360, py + 26], fill=(24, 25, 28))
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

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: empty · 0 blocks, MID: half · 4 blocks, RIGHT: full · 8 blocks | 3/4 Isometric Views)", fill=(180, 220, 210))

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
            "from": [-8.0, 0.0, -8.0],
            "to":   [-6.0, 16.0, -6.0],
            "material": "bone_main",
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
