#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b03: port (经脉端口，含 port_in 与 port_out 两个变体)

风格：A 有机型 (活体血肉、骨环领、放射肉脊、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b03_port.png (左入口 port_in、右出口 port_out)
- 调度审 b03 第 1 次（2026-10-09 08:5x）要求：
  - 肉体：#8a2a2a，x/z 1–15、y 0–6，四个竖角各切掉 2×2（做出圆角感），侧面加 #5a1a1a 竖向暗纹 2~3 条。
  - 碗形凹口（顶面向下 3 级台阶）：y 6 处开 10×10 口，y 5 处 8×8，y 4 处 6×6，最底 6×6 孔用 #5a1a1a；台阶面用 #b05050。
  - 肉脊：碗壁上 8 条 1px 宽 #c07868 浮起 0.5px 的线，四个正方向 + 四个斜方向。
    * port_in：每条线外宽内窄，靠孔一端加一个 2px 的 V 形尖头指向孔心；
    * port_out：尖头朝外、指向碗沿。
  - 骨环领：碗口一圈 #d8ccb0 宽 2px、高 2px（y 6–8），四条边中点各一根立柱 2×3×2（做到 y 8 为止），柱面 #b8a888 暗纹。
  - render.png（俯视 + 3/4 + 侧视）和 check.png（必须含 3/4 视角对标）。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/port")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b03_port.png")

# 色表修订对齐
PALETTE = {
    "flesh_dark":     (90, 26, 26, 255),    # #5a1a1a 暗血肉 (孔底深井 / 侧面暗纹)
    "flesh_main":     (138, 42, 42, 255),   # #8a2a2a 血肉 (肉体主体)
    "flesh_lit":      (176, 80, 80, 255),   # #b05050 亮肉红 (碗形台阶面)
    "bone_main":      (216, 204, 176, 255), # #d8ccb0 骨 (骨环领 / 立柱主身)
    "bone_dark":      (184, 168, 136, 255), # #b8a888 骨暗面 (立柱柱面暗纹)
    "tendon_fiber":   (192, 120, 104, 255), # #c07868 筋丝 (8 条放射状肉脊)
}

MAT_UV = {
    "flesh_dark":     [0, 0, 16, 16],
    "flesh_main":     [16, 0, 32, 16],
    "flesh_lit":      [32, 0, 48, 16],
    "bone_main":      [48, 0, 64, 16],
    "bone_dark":      [0, 16, 16, 32],
    "tendon_fiber":   [16, 16, 32, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_body(is_in_port: bool = True) -> List[dict]:
    """肉体主体：#8a2a2a，x/z 1–15 ([-7, 7])，y 0–4 为底层开 6×6 孔；
    四个竖角各切掉 2×2 (呈八边形倒角)；
    侧面加 #5a1a1a 竖向暗纹 2~3 条。
    """
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    # y in [0.0, 4.0]：底层开 6x6 孔 (x in [-3, 3], z in [-3, 3])
    # 北侧
    cubes.append({"name": f"{pfx}body_base_n", "from": [-3.0, 0.0, -7.0], "to": [ 3.0, 4.0, -3.0], "group": "flesh_body", "material": "flesh_main"})
    # 南侧
    cubes.append({"name": f"{pfx}body_base_s", "from": [-3.0, 0.0,  3.0], "to": [ 3.0, 4.0,  7.0], "group": "flesh_body", "material": "flesh_main"})
    # 西侧中部
    cubes.append({"name": f"{pfx}body_base_w_mid", "from": [-5.0, 0.0, -7.0], "to": [-3.0, 4.0,  7.0], "group": "flesh_body", "material": "flesh_main"})
    # 东侧中部
    cubes.append({"name": f"{pfx}body_base_e_mid", "from": [ 3.0, 0.0, -7.0], "to": [ 5.0, 4.0,  7.0], "group": "flesh_body", "material": "flesh_main"})
    # 西翼耳部 (切角后切掉 [-7, -5]x[-7, -5] 与 [-7, -5]x[5, 7])
    cubes.append({"name": f"{pfx}body_base_w_wing", "from": [-7.0, 0.0, -5.0], "to": [-5.0, 4.0,  5.0], "group": "flesh_body", "material": "flesh_main"})
    # 东翼耳部
    cubes.append({"name": f"{pfx}body_base_e_wing", "from": [ 5.0, 0.0, -5.0], "to": [ 7.0, 4.0,  5.0], "group": "flesh_body", "material": "flesh_main"})

    # 最底 6×6 孔底面 (#5a1a1a flesh_dark，深邃孔底)
    cubes.append({"name": f"{pfx}dark_well_bottom", "from": [-3.0, 0.0, -3.0], "to": [3.0, 0.15, 3.0], "group": "flesh_body", "material": "flesh_dark"})

    # 侧面加 #5a1a1a 竖向暗纹 2 条 (四个主要平直立面，厚度 0.05px)
    # 北面 (z = -7.0)
    cubes.append({"name": f"{pfx}stripe_n1", "from": [-3.5, 0.5, -7.05], "to": [-2.5, 4.2, -7.0], "group": "flesh_body", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}stripe_n2", "from": [ 1.5, 0.5, -7.05], "to": [ 2.5, 4.2, -7.0], "group": "flesh_body", "material": "flesh_dark"})
    # 南面 (z = 7.0)
    cubes.append({"name": f"{pfx}stripe_s1", "from": [-3.5, 0.5,  7.0], "to": [-2.5, 4.2,  7.05], "group": "flesh_body", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}stripe_s2", "from": [ 1.5, 0.5,  7.0], "to": [ 2.5, 4.2,  7.05], "group": "flesh_body", "material": "flesh_dark"})
    # 西面 (x = -7.0)
    cubes.append({"name": f"{pfx}stripe_w1", "from": [-7.05, 0.5, -3.5], "to": [-7.0, 4.2, -2.5], "group": "flesh_body", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}stripe_w2", "from": [-7.05, 0.5,  1.5], "to": [-7.0, 4.2,  2.5], "group": "flesh_body", "material": "flesh_dark"})
    # 东面 (x = 7.0)
    cubes.append({"name": f"{pfx}stripe_e1", "from": [ 7.0, 0.5, -3.5], "to": [ 7.05, 4.2, -2.5], "group": "flesh_body", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}stripe_e2", "from": [ 7.0, 0.5,  1.5], "to": [ 7.05, 4.2,  2.5], "group": "flesh_body", "material": "flesh_dark"})

    return cubes


def part_02_bowl_steps(is_in_port: bool = True) -> List[dict]:
    """碗形凹口（顶面向下 3 级台阶）：
    - y 4 处开 6×6 口，暴露 8×8 到 6×6 的 1px 宽台阶面 (#b05050)；
    - y 5 处开 8×8 口，暴露 10×10 到 8×8 的 1px 宽台阶面 (#b05050)；
    - y 6 处开 10×10 口；
    - 肉体外侧随层包裹向上延伸。
    """
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    # ── 第 1 级台阶面 (y 4 处：外 8×8，内 6×6，材质 #b05050 flesh_lit，高 y: 4.0..4.18) ──
    cubes.append({"name": f"{pfx}step1_n", "from": [-4.0, 4.0, -4.0], "to": [ 4.0, 4.18, -3.0], "group": "bowl_steps", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}step1_s", "from": [-4.0, 4.0,  3.0], "to": [ 4.0, 4.18,  4.0], "group": "bowl_steps", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}step1_w", "from": [-4.0, 4.0, -3.0], "to": [-3.0, 4.18,  3.0], "group": "bowl_steps", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}step1_e", "from": [ 3.0, 4.0, -3.0], "to": [ 4.0, 4.18,  3.0], "group": "bowl_steps", "material": "flesh_lit"})

    # y in [4.0, 5.0] 的外侧肉体 (开 8×8 口)
    cubes.append({"name": f"{pfx}body_mid_n",     "from": [-4.0, 4.0, -7.0], "to": [ 4.0, 5.0, -4.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_mid_s",     "from": [-4.0, 4.0,  4.0], "to": [ 4.0, 5.0,  7.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_mid_w_mid", "from": [-5.0, 4.0, -7.0], "to": [-4.0, 5.0,  7.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_mid_e_mid", "from": [ 4.0, 4.0, -7.0], "to": [ 5.0, 5.0,  7.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_mid_w_wing","from": [-7.0, 4.0, -5.0], "to": [-5.0, 5.0,  5.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_mid_e_wing","from": [ 5.0, 4.0, -5.0], "to": [ 7.0, 5.0,  5.0], "group": "bowl_steps", "material": "flesh_main"})

    # ── 第 2 级台阶面 (y 5 处：外 10×10，内 8×8，材质 #b05050 flesh_lit，高 y: 5.0..5.18) ──
    cubes.append({"name": f"{pfx}step2_n", "from": [-5.0, 5.0, -5.0], "to": [ 5.0, 5.18, -4.0], "group": "bowl_steps", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}step2_s", "from": [-5.0, 5.0,  4.0], "to": [ 5.0, 5.18,  5.0], "group": "bowl_steps", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}step2_w", "from": [-5.0, 5.0, -4.0], "to": [-4.0, 5.18,  4.0], "group": "bowl_steps", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}step2_e", "from": [ 4.0, 5.0, -4.0], "to": [ 5.0, 5.18,  4.0], "group": "bowl_steps", "material": "flesh_lit"})

    # y in [5.0, 6.0] 的外侧肉体 (开 10×10 口)
    cubes.append({"name": f"{pfx}body_top_n",      "from": [-5.0, 5.0, -7.0], "to": [ 5.0, 6.0, -5.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_top_s",      "from": [-5.0, 5.0,  5.0], "to": [ 5.0, 6.0,  7.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_top_w_wing", "from": [-7.0, 5.0, -5.0], "to": [-5.0, 6.0,  5.0], "group": "bowl_steps", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}body_top_e_wing", "from": [ 5.0, 5.0, -5.0], "to": [ 7.0, 6.0,  5.0], "group": "bowl_steps", "material": "flesh_main"})

    return cubes


def part_03_bone_collar(is_in_port: bool = True) -> List[dict]:
    """骨环领 + 4 根立柱：
    - 骨环领：碗口一圈 #d8ccb0 宽 2px、高 2px（y 6–8，即 x/z in [-7, 7] 外包，内孔 10×10）；
    - 四条边中点各一根立柱 2×3×2 (截面 2×2，高 3：y 5.0..8.0)，柱面加 #b8a888 暗纹。
    """
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    # 骨环领四段直梁 (y in [6.0, 7.85]，高 1.85px，宽 2.0px)
    cubes.append({"name": f"{pfx}bone_collar_n", "from": [-5.0, 6.0, -7.0], "to": [ 5.0, 7.85, -5.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bone_collar_s", "from": [-5.0, 6.0,  5.0], "to": [ 5.0, 7.85,  7.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bone_collar_w", "from": [-7.0, 6.0, -5.0], "to": [-5.0, 7.85,  5.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bone_collar_e", "from": [ 5.0, 6.0, -5.0], "to": [ 7.0, 7.85,  5.0], "group": "bone_collar", "material": "bone_main"})

    # 四边中点的 4 根立柱 (截面 2x2，高 3.1px: y 4.9..8.0，顶高微凸出骨环领 0.15px，底微下探 0.1px 锁入肉体)
    # 北立柱 (z = -7.1..-5.1)
    cubes.append({"name": f"{pfx}pillar_n", "from": [-1.0, 4.9, -7.1], "to": [1.0, 8.0, -5.1], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}pillar_n_stripe", "from": [-0.6, 5.1, -7.15], "to": [0.6, 7.85, -7.1], "group": "bone_collar", "material": "bone_dark"})
    # 南立柱 (z = 5.1..7.1)
    cubes.append({"name": f"{pfx}pillar_s", "from": [-1.0, 4.9,  5.1], "to": [1.0, 8.0,  7.1], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}pillar_s_stripe", "from": [-0.6, 5.1,  7.1], "to": [0.6, 7.85,  7.15], "group": "bone_collar", "material": "bone_dark"})
    # 西立柱 (x = -7.1..-5.1)
    cubes.append({"name": f"{pfx}pillar_w", "from": [-7.1, 4.9, -1.0], "to": [-5.1, 8.0, 1.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}pillar_w_stripe", "from": [-7.15, 5.1, -0.6], "to": [-7.1, 7.85, 0.6], "group": "bone_collar", "material": "bone_dark"})
    # 东立柱 (x = 5.1..7.1)
    cubes.append({"name": f"{pfx}pillar_e", "from": [ 5.1, 4.9, -1.0], "to": [ 7.1, 8.0, 1.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}pillar_e_stripe", "from": [ 7.1, 5.1, -0.6], "to": [ 7.15, 7.85, 0.6], "group": "bone_collar", "material": "bone_dark"})

    return cubes


def part_04_radial_ridges(is_in_port: bool = True) -> List[dict]:
    """碗壁上 8 条 1px 宽 #c07868 浮起 0.5px 的线：
    四个正方向 + 四个斜方向。
    - port_in：每条线外宽内窄，靠孔一端加一个 2px 的 V 形尖头指向孔心；
    - port_out：尖头朝外、指向碗沿。
    """
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    if is_in_port:
        # ═══════════════════════════════════════════════════════════════
        # port_in: 尖头指向孔心 (向心流向，靠孔心端为锋利 V 尖)
        # ═══════════════════════════════════════════════════════════════

        # 1. 北向 (z 负向 -> 朝 +Z 孔心)
        # 台阶2外段 (y 5.2..5.68, z: -4.8..-3.8) 宽 1.4
        cubes.append({"name": f"{pfx}ridge_n_outer", "from": [-0.7, 5.2, -4.8], "to": [0.7, 5.68, -3.8], "group": "ridges", "material": "tendon_fiber"})
        # 台阶1内段 (y 4.2..4.68, z: -3.8..-3.0) 宽 1.0
        cubes.append({"name": f"{pfx}ridge_n_inner", "from": [-0.5, 4.2, -3.8], "to": [0.5, 4.68, -3.0], "group": "ridges", "material": "tendon_fiber"})
        # 2px V形尖头 (指向孔心 +Z，尖端在 z=-2.1，两翼宽 2.0 在 z=-2.9)
        cubes.append({"name": f"{pfx}ridge_n_v_left",  "from": [-1.0, 4.2, -3.0], "to": [-0.3, 4.70, -2.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_n_v_right", "from": [ 0.3, 4.2, -3.0], "to": [ 1.0, 4.70, -2.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_n_v_tip",   "from": [-0.35, 4.2, -2.4], "to": [0.35, 4.72, -1.9], "group": "ridges", "material": "tendon_fiber"})

        # 2. 南向 (z 正向 -> 朝 -Z 孔心)
        cubes.append({"name": f"{pfx}ridge_s_outer", "from": [-0.7, 5.2,  3.8], "to": [0.7, 5.68,  4.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_inner", "from": [-0.5, 4.2,  3.0], "to": [0.5, 4.68,  3.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_v_left",  "from": [-1.0, 4.2,  2.4], "to": [-0.3, 4.70,  3.0], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_v_right", "from": [ 0.3, 4.2,  2.4], "to": [ 1.0, 4.70,  3.0], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_v_tip",   "from": [-0.35, 4.2,  1.9], "to": [0.35, 4.72,  2.4], "group": "ridges", "material": "tendon_fiber"})

        # 3. 西向 (x 负向 -> 朝 +X 孔心)
        cubes.append({"name": f"{pfx}ridge_w_outer", "from": [-4.8, 5.2, -0.7], "to": [-3.8, 5.68, 0.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_inner", "from": [-3.8, 4.2, -0.5], "to": [-3.0, 4.68, 0.5], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_v_top", "from": [-3.0, 4.2, -1.0], "to": [-2.4, 4.70, -0.3], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_v_bot", "from": [-3.0, 4.2,  0.3], "to": [-2.4, 4.70,  1.0], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_v_tip", "from": [-2.4, 4.2, -0.35], "to": [-1.9, 4.72, 0.35], "group": "ridges", "material": "tendon_fiber"})

        # 4. 东向 (x 正向 -> 朝 -X 孔心)
        cubes.append({"name": f"{pfx}ridge_e_outer", "from": [ 3.8, 5.2, -0.7], "to": [ 4.8, 5.68, 0.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_inner", "from": [ 3.0, 4.2, -0.5], "to": [ 3.8, 4.68, 0.5], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_v_top", "from": [ 2.4, 4.2, -1.0], "to": [ 3.0, 4.70, -0.3], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_v_bot", "from": [ 2.4, 4.2,  0.3], "to": [ 3.0, 4.70,  1.0], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_v_tip", "from": [ 1.9, 4.2, -0.35], "to": [ 2.4, 4.72, 0.35], "group": "ridges", "material": "tendon_fiber"})

        # 5. NW 斜向 (朝 SE 孔心)
        cubes.append({"name": f"{pfx}ridge_nw_outer", "from": [-4.5, 5.2, -4.5], "to": [-3.7, 5.68, -3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_nw_inner", "from": [-3.7, 4.2, -3.7], "to": [-2.8, 4.68, -2.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_nw_v_tip", "from": [-2.8, 4.2, -2.8], "to": [-2.0, 4.72, -2.0], "group": "ridges", "material": "tendon_fiber"})

        # 6. NE 斜向 (朝 SW 孔心)
        cubes.append({"name": f"{pfx}ridge_ne_outer", "from": [ 3.7, 5.2, -4.5], "to": [ 4.5, 5.68, -3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_ne_inner", "from": [ 2.8, 4.2, -3.7], "to": [ 3.7, 4.68, -2.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_ne_v_tip", "from": [ 2.0, 4.2, -2.8], "to": [ 2.8, 4.72, -2.0], "group": "ridges", "material": "tendon_fiber"})

        # 7. SW 斜向 (朝 NE 孔心)
        cubes.append({"name": f"{pfx}ridge_sw_outer", "from": [-4.5, 5.2,  3.7], "to": [-3.7, 5.68,  4.5], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_sw_inner", "from": [-3.7, 4.2,  2.8], "to": [-2.8, 4.68,  3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_sw_v_tip", "from": [-2.8, 4.2,  2.0], "to": [-2.0, 4.72,  2.8], "group": "ridges", "material": "tendon_fiber"})

        # 8. SE 斜向 (朝 NW 孔心)
        cubes.append({"name": f"{pfx}ridge_se_outer", "from": [ 3.7, 5.2,  3.7], "to": [ 4.5, 5.68,  4.5], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_se_inner", "from": [ 2.8, 4.2,  2.8], "to": [ 3.7, 4.68,  3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_se_v_tip", "from": [ 2.0, 4.2,  2.0], "to": [ 2.8, 4.72,  2.8], "group": "ridges", "material": "tendon_fiber"})

    else:
        # ═══════════════════════════════════════════════════════════════
        # port_out: 尖头指向碗沿 (离心发散流向，靠碗沿端为锋利 V 尖)
        # ═══════════════════════════════════════════════════════════════

        # 1. 北向 (朝 -Z 碗沿)
        # 台阶1内段基部 (y 4.2..4.68, z: -3.8..-3.0) 宽 1.0
        cubes.append({"name": f"{pfx}ridge_n_inner", "from": [-0.5, 4.2, -3.8], "to": [0.5, 4.68, -3.0], "group": "ridges", "material": "tendon_fiber"})
        # 台阶2中段 (y 5.2..5.68, z: -4.4..-3.8) 宽 1.4
        cubes.append({"name": f"{pfx}ridge_n_outer", "from": [-0.7, 5.2, -4.4], "to": [0.7, 5.68, -3.8], "group": "ridges", "material": "tendon_fiber"})
        # 2px V形尖头 (指向碗沿 -Z，尖端在 z=-5.1，两翼在 z=-4.8..-4.4 宽 2.0)
        cubes.append({"name": f"{pfx}ridge_n_v_left",  "from": [-1.0, 5.22, -4.8], "to": [-0.3, 5.70, -4.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_n_v_right", "from": [ 0.3, 5.22, -4.8], "to": [ 1.0, 5.70, -4.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_n_v_tip",   "from": [-0.35, 5.24, -5.1], "to": [0.35, 5.72, -4.7], "group": "ridges", "material": "tendon_fiber"})

        # 2. 南向 (朝 +Z 碗沿)
        cubes.append({"name": f"{pfx}ridge_s_inner", "from": [-0.5, 4.2,  3.0], "to": [0.5, 4.68,  3.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_outer", "from": [-0.7, 5.2,  3.8], "to": [0.7, 5.68,  4.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_v_left",  "from": [-1.0, 5.22,  4.4], "to": [-0.3, 5.70,  4.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_v_right", "from": [ 0.3, 5.22,  4.4], "to": [ 1.0, 5.70,  4.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_s_v_tip",   "from": [-0.35, 5.24,  4.7], "to": [0.35, 5.72,  5.1], "group": "ridges", "material": "tendon_fiber"})

        # 3. 西向 (朝 -X 碗沿)
        cubes.append({"name": f"{pfx}ridge_w_inner", "from": [-3.8, 4.2, -0.5], "to": [-3.0, 4.68, 0.5], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_outer", "from": [-4.4, 5.2, -0.7], "to": [-3.8, 5.68, 0.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_v_top", "from": [-4.8, 5.22, -1.0], "to": [-4.4, 5.70, -0.3], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_v_bot", "from": [-4.8, 5.22,  0.3], "to": [-4.4, 5.70,  1.0], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_w_v_tip", "from": [-5.1, 5.24, -0.35], "to": [-4.7, 5.72, 0.35], "group": "ridges", "material": "tendon_fiber"})

        # 4. 东向 (朝 +X 碗沿)
        cubes.append({"name": f"{pfx}ridge_e_inner", "from": [ 3.0, 4.2, -0.5], "to": [ 3.8, 4.68, 0.5], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_outer", "from": [ 3.8, 5.2, -0.7], "to": [ 4.4, 5.68, 0.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_v_top", "from": [ 4.4, 5.22, -1.0], "to": [ 4.8, 5.70, -0.3], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_v_bot", "from": [ 4.4, 5.22,  0.3], "to": [ 4.8, 5.70,  1.0], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_e_v_tip", "from": [ 4.7, 5.24, -0.35], "to": [ 5.1, 5.72, 0.35], "group": "ridges", "material": "tendon_fiber"})

        # 5. NW 斜向 (朝 NW 碗沿外射)
        cubes.append({"name": f"{pfx}ridge_nw_inner", "from": [-3.7, 4.2, -3.7], "to": [-2.8, 4.68, -2.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_nw_outer", "from": [-4.4, 5.2, -4.4], "to": [-3.7, 5.68, -3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_nw_v_tip", "from": [-5.1, 5.23, -5.1], "to": [-4.3, 5.72, -4.3], "group": "ridges", "material": "tendon_fiber"})

        # 6. NE 斜向 (朝 NE 碗沿外射)
        cubes.append({"name": f"{pfx}ridge_ne_inner", "from": [ 2.8, 4.2, -3.7], "to": [ 3.7, 4.68, -2.8], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_ne_outer", "from": [ 3.7, 5.2, -4.4], "to": [ 4.4, 5.68, -3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_ne_v_tip", "from": [ 4.3, 5.23, -5.1], "to": [ 5.1, 5.72, -4.3], "group": "ridges", "material": "tendon_fiber"})

        # 7. SW 斜向 (朝 SW 碗沿外射)
        cubes.append({"name": f"{pfx}ridge_sw_inner", "from": [-3.7, 4.2,  2.8], "to": [-2.8, 4.68,  3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_sw_outer", "from": [-4.4, 5.2,  3.7], "to": [-3.7, 5.68,  4.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_sw_v_tip", "from": [-5.1, 5.23,  4.3], "to": [-4.3, 5.72,  5.1], "group": "ridges", "material": "tendon_fiber"})

        # 8. SE 斜向 (朝 SE 碗沿外射)
        cubes.append({"name": f"{pfx}ridge_se_inner", "from": [ 2.8, 4.2,  2.8], "to": [ 3.7, 4.68,  3.7], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_se_outer", "from": [ 3.7, 5.2,  3.7], "to": [ 4.4, 5.68,  4.4], "group": "ridges", "material": "tendon_fiber"})
        cubes.append({"name": f"{pfx}ridge_se_v_tip", "from": [ 4.3, 5.23,  4.3], "to": [ 5.1, 5.72,  5.1], "group": "ridges", "material": "tendon_fiber"})

    return cubes


def all_cubes(is_in_port: bool = True) -> List[dict]:
    """汇总指定变体所有部件的立方体。"""
    return (
        part_01_flesh_body(is_in_port)
        + part_02_bowl_steps(is_in_port)
        + part_03_bone_collar(is_in_port)
        + part_04_radial_ridges(is_in_port)
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

        if "flesh" in mat_name or "tendon" in mat_name:
            noise = rng.randint(-12, 12, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(is_in_port: bool, out_path: Path) -> Path:
    """导出指定变体的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(is_in_port)
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

    name = "port_in" if is_in_port else "port_out"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "port")
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

def render_views(p_in_path: Path, p_out_path: Path):
    """输出包含（俯视 + 3/4 + 侧视）的综合拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各变体视角：俯视 (TOP)、3/4 视角 (Isometric)、侧视 (SIDE)
    # port_in 视角
    im_in_top, _ = render(p_in_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_in_iso, _ = render(p_in_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_in_side, _ = render(p_in_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)

    # port_out 视角
    im_out_top, _ = render(p_out_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_out_iso, _ = render(p_out_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_out_side, _ = render(p_out_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (2 行 3 列：左中右分别为 俯视 TOP、3/4 等轴、侧视 SIDE；上行为 port_in，下行为 port_out)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 3 + 40
    canvas_h = cell_h * 2 + 40
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        # 行 1: port_in
        ("PORT_IN · TOP (V-Arrow Points IN)", im_in_top, 10, 10),
        ("PORT_IN · 3/4 Isometric (Bowl & Pillars)", im_in_iso, cell_w + 20, 10),
        ("PORT_IN · SIDE (Half-Block & Pillars)", im_in_side, cell_w * 2 + 30, 10),
        # 行 2: port_out
        ("PORT_OUT · TOP (V-Arrow Points OUT)", im_out_top, 10, cell_h + 20),
        ("PORT_OUT · 3/4 Isometric (Bowl & Pillars)", im_out_iso, cell_w + 20, cell_h + 20),
        ("PORT_OUT · SIDE (Half-Block & Pillars)", im_out_side, cell_w * 2 + 30, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 360, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (俯视 + 3/4 + 侧视 2x3 拼版) 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图，右渲染包含 3/4 视与俯视对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        # 右侧放 4 张对比：port_in 3/4、port_out 3/4、port_in 俯视、port_out 俯视
        thumb_w = int(target_h * 0.55)
        thumb_h = int(target_h * 0.46)

        c_in_iso = im_in_iso.resize((thumb_w, thumb_h), Image.Resampling.LANCZOS)
        c_out_iso = im_out_iso.resize((thumb_w, thumb_h), Image.Resampling.LANCZOS)
        c_in_top = im_in_top.resize((thumb_w, thumb_h), Image.Resampling.LANCZOS)
        c_out_top = im_out_top.resize((thumb_w, thumb_h), Image.Resampling.LANCZOS)

        right_w = thumb_w * 2 + 20
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b03_port.png: Left In, Right Out)", fill=(210, 200, 180))

        # 贴右渲染图 (上行两张 3/4 视，下行两张 TOP 俯视)
        rx = ref_w + 24
        # 上行 3/4
        check_cv.paste(c_in_iso, (rx, 34))
        check_cv.paste(c_out_iso, (rx + thumb_w + 10, 34))
        # 下行 TOP
        check_cv.paste(c_in_top, (rx, 34 + thumb_h + 10))
        check_cv.paste(c_out_top, (rx + thumb_w + 10, 34 + thumb_h + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (Top: 3/4 Views, Bottom: TOP Views | Left: port_in, Right: port_out)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含 3/4 视角对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：两个变体正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_port.py 差分自证...")
    for is_in in [True, False]:
        name = "port_in" if is_in else "port_out"
        cubes = all_cubes(is_in)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] {name} 正常立方体集无共面冲突")

        defect_cubes = list(cubes) + [{
            "name": "inject_coplanar_fail",
            "from": [-3.0, 0.0, -7.0],
            "to":   [ 3.0, 4.0, -3.0],
            "material": "flesh_main",
        }]
        caught = False
        try:
            _assert_no_coplanar_faces(defect_cubes)
        except AssertionError as e:
            caught = True
            print(f"  [OK] {name} 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

        if not caught:
            raise RuntimeError(f"门禁失效: {name} 注入共面冲突未被拦截!")

    print("✓ gen_port.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b03 port 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p_in = generate_bbmodel(is_in_port=True, out_path=MODEL_DIR / "port_in.bbmodel")
    p_out = generate_bbmodel(is_in_port=False, out_path=MODEL_DIR / "port_out.bbmodel")
    # 同时生成默认 port.bbmodel 作为主文件
    generate_bbmodel(is_in_port=True, out_path=MODEL_DIR / "port.bbmodel")
    render_views(p_in, p_out)


if __name__ == "__main__":
    main()
