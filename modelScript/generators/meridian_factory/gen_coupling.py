#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b08: coupling (耦合接口)

风格：A 有机型 (活体血肉、暖粉筋管、肉箍骨夹耳、三节逐级外扩空心壳喇叭口、内壁衬层、底部内光、紧贴外壳放射筋丝)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b08_coupling.png
- 调度审第 1 次修改意见（2026-10-09 15:4x 严格对齐给定坐标）：
  * 沿 z 走向，0 是接经脉的窄端 (-Z 端)，16 是喇叭口 (+Z 端)；
  * z 0–3 (z_model: -8..-5)：筋管短节 x 4–12 (x: -4..4)、y 2–8 (暖粉 #d9a08c)，外包肉箍 z 1–3 (z: -7..-5)、x 3–13 (x: -5..5)、y 1–9 (#8a2a2a)；
  * z 3–5 (z_model: -5..-3)：骨夹箍，外框 x 2–14 (x: -6..6)、y 0–10，壁厚 2 (#d8ccb0)，左右各一个骨夹耳 x 0–2 / 14–16 (x: -8..-6 / 6..8)、y 3–7、z 3–5；
  * 喇叭口用 3 节空心壳（壁厚 2，#8a2a2a，内壁面 #b05050）：
    - 第 1 节：z 5–9 (z: -3..1) 外 x 2–14 (x: -6..6)、y 1–11；
    - 第 2 节：z 9–13 (z: 1..5) 外 x 1–15 (x: -7..7)、y 0–13；
    - 第 3 节：z 13–16 (z: 5..8) 外 x 0–16 (x: -8..8)、y 0–15，最外一节四个竖角各切 1×1；
  * 喇叭底（z=5 / z_model=-3 处，壳内）一块 #f6dcc4 4×4 当内光，中心对着经脉；
  * 外表面 8 条 #c07868 放射筋丝：上、下、左、右、四个斜角各一条，每条从 z 5 沿外壳表面走到 z 16，贴面浮 0.5px，跟着三节外壳逐级外扩，不漂在外面；
  * 删掉所有浮板。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/coupling")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b08_coupling.png")

# 配色表 (完全对齐 meridian_factory.md)
PALETTE = {
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (肉箍主体、三节肉质外壳)
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (内腔深处阴影)
    "flesh_lit":        (176, 80, 80, 255),   # #b05050 亮肉红 (喇叭口内壁面衬层)
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (骨夹箍、左右骨夹耳)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 (骨夹耳暗纹插销)
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 筋管暖粉 (-Z端8x6筋管短节)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋膜亮粉 (接口上下 1px 亮边)
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 筋丝 (紧贴外壳走台阶的 8 条放射筋丝)
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元内光 (喇叭口底部 4x4 内光)
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
# 各部件几何定义 (part_* 严格按调度给定坐标构建)
# =============================================================================

def part_01_narrow_inlet() -> List[dict]:
    """z 0–3 (z_model: -8..-5)：
    - 筋管短节 x 4–12 (x: -4..4)、y 2–8 (暖粉 #d9a08c)；
    - 上下各附 1px 亮粉高光边 #e8bca8；
    - 外包肉箍 z 1–3 (z: -7..-5)、x 3–13 (x: -5..5)、y 1–9 (#8a2a2a)。
    """
    cubes = []

    # 1. 筋管短节 (z in [-8.0, -5.0]，x in [-4.0, 4.0]，y in [2.0, 8.0])
    cubes.append({
        "name": "inlet_tube",
        "from": [-4.0, 2.0, -8.0],
        "to":   [ 4.0, 8.0, -5.0],
        "group": "inlet",
        "material": "tendon_tube",
    })

    # 上下 1px 亮粉高光边 (贴于外露端面 z in [-8.0, -7.05])
    cubes.append({
        "name": "inlet_hl_bot",
        "from": [-3.9, 1.95, -7.95],
        "to":   [ 3.9, 2.8,  -7.05],
        "group": "inlet",
        "material": "tendon_highlight",
    })
    cubes.append({
        "name": "inlet_hl_top",
        "from": [-3.9, 7.2,  -7.95],
        "to":   [ 3.9, 8.05, -7.05],
        "group": "inlet",
        "material": "tendon_highlight",
    })

    # 2. 外包肉箍：z 1–3 (z in [-7.0, -5.0])，外廓 x in [-5.0, 5.0]，y in [1.0, 9.0]
    # 底套 (y in [1.0, 2.0], x in [-5.0, 5.0])
    cubes.append({
        "name": "collar_bot",
        "from": [-5.0, 1.0, -7.0],
        "to":   [ 5.0, 2.0, -5.0],
        "group": "inlet",
        "material": "flesh_main",
    })
    # 顶套 (y in [8.0, 9.0], x in [-5.0, 5.0])
    cubes.append({
        "name": "collar_top",
        "from": [-5.0, 8.0, -7.0],
        "to":   [ 5.0, 9.0, -5.0],
        "group": "inlet",
        "material": "flesh_main",
    })
    # 左套 (x in [-5.0, -4.0], y in [2.0, 8.0])
    cubes.append({
        "name": "collar_l",
        "from": [-5.0, 2.0, -7.0],
        "to":   [-4.0, 8.0, -5.0],
        "group": "inlet",
        "material": "flesh_main",
    })
    # 右套 (x in [4.0, 5.0], y in [2.0, 8.0])
    cubes.append({
        "name": "collar_r",
        "from": [ 4.0, 2.0, -7.0],
        "to":   [ 5.0, 8.0, -5.0],
        "group": "inlet",
        "material": "flesh_main",
    })

    return cubes


def part_02_bone_clamp() -> List[dict]:
    """z 3–5 (z_model: -5..-3)：
    - 骨夹箍：外框 x 2–14 (x: -6..6)、y 0–10，壁厚 2 (#d8ccb0)；内腔对齐 x -4..4, y 2..8；
    - 左右各一个骨夹耳：x 0–2 / 14–16 (x: -8..-6 / 6..8)、y 3–7、z 3–5 (z: -5..-3)。
    """
    cubes = []

    # 1. 骨夹箍外框 4 块壁 (z in [-5.0, -3.0], 壁厚 2px)
    # 底壁 (y in [0.0, 2.0], x in [-6.0, 6.0])
    cubes.append({
        "name": "clamp_frame_bot",
        "from": [-6.0, 0.0, -5.0],
        "to":   [ 6.0, 2.0, -3.0],
        "group": "bone_clamp",
        "material": "bone_main",
    })
    # 顶壁 (y in [8.0, 10.0], x in [-6.0, 6.0])
    cubes.append({
        "name": "clamp_frame_top",
        "from": [-6.0, 8.0, -5.0],
        "to":   [ 6.0, 10.0, -3.0],
        "group": "bone_clamp",
        "material": "bone_main",
    })
    # 左壁 (x in [-6.0, -4.0], y in [2.0, 8.0])
    cubes.append({
        "name": "clamp_frame_l",
        "from": [-6.0, 2.0, -5.0],
        "to":   [-4.0, 8.0, -3.0],
        "group": "bone_clamp",
        "material": "bone_main",
    })
    # 右壁 (x in [4.0, 6.0], y in [2.0, 8.0])
    cubes.append({
        "name": "clamp_frame_r",
        "from": [ 4.0, 2.0, -5.0],
        "to":   [ 6.0, 8.0, -3.0],
        "group": "bone_clamp",
        "material": "bone_main",
    })

    # 2. 左右各一个骨夹耳 (z in [-5.0, -3.0], y in [3.0, 7.0])
    # 西侧骨夹耳 (x in [-8.0, -6.0])
    cubes.append({
        "name": "clamp_ear_w",
        "from": [-8.0, 3.0, -5.0],
        "to":   [-6.0, 7.0, -3.0],
        "group": "bone_clamp",
        "material": "bone_main",
    })
    # 西夹耳锁销暗纹 (#b8a888 bone_dark，微浮出 0.05px)
    cubes.append({
        "name": "clamp_ear_w_pin",
        "from": [-8.05, 4.2, -4.6],
        "to":   [-5.95, 5.8, -3.4],
        "group": "bone_clamp",
        "material": "bone_dark",
    })

    # 东侧骨夹耳 (x in [6.0, 8.0])
    cubes.append({
        "name": "clamp_ear_e",
        "from": [ 6.0, 3.0, -5.0],
        "to":   [ 8.0, 7.0, -3.0],
        "group": "bone_clamp",
        "material": "bone_main",
    })
    # 东夹耳锁销暗纹
    cubes.append({
        "name": "clamp_ear_e_pin",
        "from": [ 5.95, 4.2, -4.6],
        "to":   [ 8.05, 5.8, -3.4],
        "group": "bone_clamp",
        "material": "bone_dark",
    })

    return cubes


def part_03_flaring_bell() -> List[dict]:
    """喇叭口用 3 节空心壳（壁厚 2，#8a2a2a，内壁面 #b05050）：
    - 喇叭底（z=5 / z_model: -3.0 处，壳内）：一块 #f6dcc4 4×4 当内光，中心对着经脉；
    - 第 1 节：z 5–9 (z_model: -3..1) 外 x 2–14 (x: -6..6)、y 1–11；内腔 x in [-4, 4], y in [3, 9]；
    - 第 2 节：z 9–13 (z_model: 1..5) 外 x 1–15 (x: -7..7)、y 0–13；内腔 x in [-5, 5], y in [2, 11]；
    - 第 3 节：z 13–16 (z_model: 5..8) 外 x 0–16 (x: -8..8)、y 0–15，最外一节四个竖角各切 1×1；内腔 x in [-6, 6], y in [2, 13]。
    """
    cubes = []

    # ═══════════ 喇叭底内光 (#f6dcc4 qi_glow，4x4 居中对齐经脉中心 y=5) ═══════════
    # 位于 z_model = -3.0 处壳内，x in [-2.0, 2.0], y in [3.0, 7.0], z in [-3.0, -2.6]
    cubes.append({
        "name": "bell_core_qi",
        "from": [-2.0, 3.0, -3.0],
        "to":   [ 2.0, 7.0, -2.6],
        "group": "flaring_bell",
        "material": "qi_glow",
    })

    # ═══════════ 第 1 节空心壳 (z in [-3.0, 1.0]，外 12x10: x in [-6, 6], y in [1, 11]) ═══════════
    # 壁厚 2px: 外层 1.8px #8a2a2a，内表面层 0.2px #b05050 (无缝实心结构，杜绝浮板)
    # 底壁：外层 y in [1.0, 2.8]，内层 y in [2.8, 3.0]
    cubes.append({"name": "shell1_bot_out", "from": [-6.0, 1.0, -3.0], "to": [ 6.0, 2.8,  1.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell1_bot_in",  "from": [-6.0, 2.8, -3.0], "to": [ 6.0, 3.0,  1.0], "group": "flaring_bell", "material": "flesh_lit"})
    # 顶壁：内层 y in [9.0, 9.2]，外层 y in [9.2, 11.0]
    cubes.append({"name": "shell1_top_in",  "from": [-6.0, 9.0, -3.0], "to": [ 6.0, 9.2,  1.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "shell1_top_out", "from": [-6.0, 9.2, -3.0], "to": [ 6.0, 11.0, 1.0], "group": "flaring_bell", "material": "flesh_main"})
    # 左壁：外层 x in [-6.0, -4.2]，内层 x in [-4.2, -4.0]，y in [3.0, 9.0]
    cubes.append({"name": "shell1_l_out",   "from": [-6.0, 3.0, -3.0], "to": [-4.2, 9.0,  1.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell1_l_in",    "from": [-4.2, 3.0, -3.0], "to": [-4.0, 9.0,  1.0], "group": "flaring_bell", "material": "flesh_lit"})
    # 右壁：内层 x in [4.0, 4.2]，外层 x in [4.2, 6.0]，y in [3.0, 9.0]
    cubes.append({"name": "shell1_r_in",    "from": [ 4.0, 3.0, -3.0], "to": [ 4.2, 9.0,  1.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "shell1_r_out",   "from": [ 4.2, 3.0, -3.0], "to": [ 6.0, 9.0,  1.0], "group": "flaring_bell", "material": "flesh_main"})

    # ═══════════ 第 2 节空心壳 (z in [1.0, 5.0]，外 14x13: x in [-7, 7], y in [0, 13]) ═══════════
    # 内腔：x in [-5, 5], y in [2, 11]
    # 底壁：外层 y in [0.0, 1.8]，内层 y in [1.8, 2.0]
    cubes.append({"name": "shell2_bot_out", "from": [-7.0, 0.0,  1.0], "to": [ 7.0, 1.8,  5.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell2_bot_in",  "from": [-7.0, 1.8,  1.0], "to": [ 7.0, 2.0,  5.0], "group": "flaring_bell", "material": "flesh_lit"})
    # 顶壁：内层 y in [11.0, 11.2]，外层 y in [11.2, 13.0]
    cubes.append({"name": "shell2_top_in",  "from": [-7.0, 11.0, 1.0], "to": [ 7.0, 11.2, 5.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "shell2_top_out", "from": [-7.0, 11.2, 1.0], "to": [ 7.0, 13.0, 5.0], "group": "flaring_bell", "material": "flesh_main"})
    # 左壁：外层 x in [-7.0, -5.2]，内层 x in [-5.2, -5.0]，y in [2.0, 11.0]
    cubes.append({"name": "shell2_l_out",   "from": [-7.0, 2.0,  1.0], "to": [-5.2, 11.0, 5.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell2_l_in",    "from": [-5.2, 2.0,  1.0], "to": [-5.0, 11.0, 5.0], "group": "flaring_bell", "material": "flesh_lit"})
    # 右壁：内层 x in [5.0, 5.2]，外层 x in [5.2, 7.0]，y in [2.0, 11.0]
    cubes.append({"name": "shell2_r_in",    "from": [ 5.0, 2.0,  1.0], "to": [ 5.2, 11.0, 5.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "shell2_r_out",   "from": [ 5.2, 2.0,  1.0], "to": [ 7.0, 11.0, 5.0], "group": "flaring_bell", "material": "flesh_main"})

    # ═══════════ 第 3 节空心壳 (z in [5.0, 8.0]，外 16x15: x in [-8, 8], y in [0, 15]，四竖角切 1x1) ═══════════
    # 内腔：x in [-6, 6], y in [2, 13]
    # 底壁 (主体 x in [-6, 6], y in [0, 2]，切角避让后外层/内层)
    cubes.append({"name": "shell3_bot_out", "from": [-6.0, 0.0,  5.0], "to": [ 6.0, 1.8,  8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell3_bot_in",  "from": [-6.0, 1.8,  5.0], "to": [ 6.0, 2.0,  8.0], "group": "flaring_bell", "material": "flesh_lit"})
    # 底壁左右斜切阶梯角 (左下切角保留 x in [-7, -6], y in [0, 2]，切掉 x in [-8, -7] 的 y in [0, 1])
    cubes.append({"name": "shell3_bl_chamfer", "from": [-7.0, 0.0, 5.0], "to": [-6.0, 2.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell3_br_chamfer", "from": [ 6.0, 0.0, 5.0], "to": [ 7.0, 2.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})

    # 顶壁 (主体 x in [-6, 6], y in [13, 15])
    cubes.append({"name": "shell3_top_in",  "from": [-6.0, 13.0, 5.0], "to": [ 6.0, 13.2, 8.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "shell3_top_out", "from": [-6.0, 13.2, 5.0], "to": [ 6.0, 15.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    # 顶壁左右斜切阶梯角 (左上切角保留 x in [-7, -6], y in [13, 15]，切掉 x in [-8, -7] 的 y in [14, 15])
    cubes.append({"name": "shell3_tl_chamfer", "from": [-7.0, 13.0, 5.0], "to": [-6.0, 15.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell3_tr_chamfer", "from": [ 6.0, 13.0, 5.0], "to": [ 7.0, 15.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})

    # 左壁 (主体 x in [-8, -6], y in [2, 13])
    cubes.append({"name": "shell3_l_out",   "from": [-8.0, 2.0,  5.0], "to": [-6.2, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "shell3_l_in",    "from": [-6.2, 2.0,  5.0], "to": [-6.0, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_lit"})
    # 右壁 (主体 x in [6, 8], y in [2, 13])
    cubes.append({"name": "shell3_r_in",    "from": [ 6.0, 2.0,  5.0], "to": [ 6.2, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "shell3_r_out",   "from": [ 6.2, 2.0,  5.0], "to": [ 8.0, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})

    return cubes


def part_04_radiating_fibers() -> List[dict]:
    """外表面 8 条 #c07868 放射筋丝：
    上、下、左、右、四个斜角各一条，每条从 z 5 (z_model: -3.0) 沿外壳表面走到 z 16 (z_model: 8.0)，
    贴面浮 0.5px，跟着三节外壳逐级外扩，紧密贴合外表面，绝不悬空漂浮！
    """
    cubes = []

    # ═══════════ 1. 上筋丝 (中轴 x in [-0.5, 0.5]) ═══════════
    # 第 1 节顶面 (y=11.0 贴面浮 0.5px: y in [11.0, 11.5], z in [-2.95, 0.95])
    cubes.append({"name": "fiber_top_s1", "from": [-0.5, 11.0, -2.95], "to": [0.5, 11.5, 0.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 1->2 爬升竖段 (在 z=1 处外壳前立面贴紧 y in [11.25, 13.25])
    cubes.append({"name": "fiber_top_step1", "from": [-0.5, 11.25, 0.95], "to": [0.5, 13.25, 1.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 2 节顶面 (y=13.0 贴面浮 0.5px: y in [13.0, 13.5], z in [1.05, 4.95])
    cubes.append({"name": "fiber_top_s2", "from": [-0.5, 13.0, 1.05], "to": [0.5, 13.5, 4.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 2->3 爬升竖段 (在 z=5 处外壳立面贴紧 y in [13.25, 15.25])
    cubes.append({"name": "fiber_top_step2", "from": [-0.5, 13.25, 4.95], "to": [0.5, 15.25, 5.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 3 节顶面 (y=15.0 贴面浮 0.5px: y in [15.0, 15.5], z in [5.05, 7.95])
    cubes.append({"name": "fiber_top_s3", "from": [-0.5, 15.0, 5.05], "to": [0.5, 15.5, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 2. 下筋丝 (中轴 x in [-0.5, 0.5]) ═══════════
    # 第 1 节底面 (y=1.0 贴面浮 0.5px: y in [0.5, 1.0], z in [-2.95, 0.95])
    cubes.append({"name": "fiber_bot_s1", "from": [-0.5, 0.5, -2.95], "to": [0.5, 1.0, 0.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 1->2 下延竖段 (在 z=1 处外壳立面贴紧 y in [-0.25, 0.75])
    cubes.append({"name": "fiber_bot_step1", "from": [-0.5, -0.25, 0.95], "to": [0.5, 0.75, 1.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 2、3 节底面保持 y=0.0 (贴面浮 0.5px: y in [-0.5, 0.0], z in [1.05, 7.95])
    cubes.append({"name": "fiber_bot_s2", "from": [-0.5, -0.5, 1.05], "to": [0.5, 0.0, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 3. 左筋丝 (-X 侧，中轴 y in [5.5, 6.5]) ═══════════
    # 第 1 节左壁 (x=-6.0 贴面浮 0.5px: x in [-6.5, -6.0], z in [-2.95, 0.95])
    cubes.append({"name": "fiber_l_s1", "from": [-6.5, 5.5, -2.95], "to": [-6.0, 6.5, 0.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 1->2 外展横段 (在 z=1 处立面贴紧 x in [-7.25, -5.75])
    cubes.append({"name": "fiber_l_step1", "from": [-7.25, 5.6, 0.95], "to": [-5.75, 6.4, 1.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 2 节左壁 (x=-7.0 贴面浮 0.5px: x in [-7.5, -7.0], z in [1.05, 4.95])
    cubes.append({"name": "fiber_l_s2", "from": [-7.5, 5.5, 1.05], "to": [-7.0, 6.5, 4.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 2->3 外展横段 (在 z=5 处立面贴紧 x in [-8.25, -6.75])
    cubes.append({"name": "fiber_l_step2", "from": [-8.25, 5.6, 4.95], "to": [-6.75, 6.4, 5.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 3 节左壁 (x=-8.0 贴面浮 0.5px: x in [-8.5, -8.0], z in [5.05, 7.95])
    cubes.append({"name": "fiber_l_s3", "from": [-8.5, 5.5, 5.05], "to": [-8.0, 6.5, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 4. 右筋丝 (+X 侧，中轴 y in [5.5, 6.5]) ═══════════
    # 第 1 节右壁 (x=6.0 贴面浮 0.5px: x in [6.0, 6.5], z in [-2.95, 0.95])
    cubes.append({"name": "fiber_r_s1", "from": [ 6.0, 5.5, -2.95], "to": [ 6.5, 6.5, 0.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 1->2 外展横段 (在 z=1 处立面贴紧 x in [5.75, 7.25])
    cubes.append({"name": "fiber_r_step1", "from": [ 5.75, 5.6, 0.95], "to": [ 7.25, 6.4, 1.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 2 节右壁 (x=7.0 贴面浮 0.5px: x in [7.0, 7.5], z in [1.05, 4.95])
    cubes.append({"name": "fiber_r_s2", "from": [ 7.0, 5.5, 1.05], "to": [ 7.5, 6.5, 4.95], "group": "fibers", "material": "tendon_fiber"})
    # 台阶 2->3 外展横段 (在 z=5 处立面贴紧 x in [6.75, 8.25])
    cubes.append({"name": "fiber_r_step2", "from": [ 6.75, 5.6, 4.95], "to": [ 8.25, 6.4, 5.05], "group": "fibers", "material": "tendon_fiber"})
    # 第 3 节右壁 (x=8.0 贴面浮 0.5px: x in [8.0, 8.5], z in [5.05, 7.95])
    cubes.append({"name": "fiber_r_s3", "from": [ 8.0, 5.5, 5.05], "to": [ 8.5, 6.5, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 5. 右上斜角筋丝 (贴于右上拐角) ═══════════
    cubes.append({"name": "fiber_tr_s1", "from": [ 5.6, 10.6, -2.95], "to": [ 6.1, 11.1, 0.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tr_step1", "from": [ 5.8, 10.8, 0.95], "to": [ 7.1, 13.1, 1.05], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tr_s2", "from": [ 6.6, 12.6, 1.05], "to": [ 7.1, 13.1, 4.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tr_step2", "from": [ 6.8, 12.8, 4.95], "to": [ 8.1, 14.6, 5.05], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tr_s3", "from": [ 6.8, 13.6, 5.05], "to": [ 7.3, 14.1, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 6. 左上斜角筋丝 (贴于左上拐角) ═══════════
    cubes.append({"name": "fiber_tl_s1", "from": [-6.1, 10.6, -2.95], "to": [-5.6, 11.1, 0.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tl_step1", "from": [-7.1, 10.8, 0.95], "to": [-5.8, 13.1, 1.05], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tl_s2", "from": [-7.1, 12.6, 1.05], "to": [-6.6, 13.1, 4.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tl_step2", "from": [-8.1, 12.8, 4.95], "to": [-6.8, 14.6, 5.05], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_tl_s3", "from": [-7.3, 13.6, 5.05], "to": [-6.8, 14.1, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 7. 右下斜角筋丝 (贴于右下拐角) ═══════════
    cubes.append({"name": "fiber_br_s1", "from": [ 5.6, 0.9, -2.95], "to": [ 6.1, 1.4, 0.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_br_step1", "from": [ 5.8, -0.2, 0.95], "to": [ 7.1, 1.1, 1.05], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_br_s2", "from": [ 6.6, -0.1, 1.05], "to": [ 7.1, 0.4, 4.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_br_s3", "from": [ 6.8, 0.9, 5.05], "to": [ 7.3, 1.4, 7.95], "group": "fibers", "material": "tendon_fiber"})

    # ═══════════ 8. 左下斜角筋丝 (贴于左下拐角) ═══════════
    cubes.append({"name": "fiber_bl_s1", "from": [-6.1, 0.9, -2.95], "to": [-5.6, 1.4, 0.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_bl_step1", "from": [-7.1, -0.2, 0.95], "to": [-5.8, 1.1, 1.05], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_bl_s2", "from": [-7.1, -0.1, 1.05], "to": [-6.6, 0.4, 4.95], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_bl_s3", "from": [-7.3, 0.9, 5.05], "to": [-6.8, 1.4, 7.95], "group": "fibers", "material": "tendon_fiber"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_narrow_inlet()
        + part_02_bone_clamp()
        + part_03_flaring_bell()
        + part_04_radiating_fibers()
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


def generate_bbmodel(out_path: Path) -> Path:
    """导出 coupling 的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes()
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

    name = "coupling"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "coupling")
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

def render_views(model_p: Path):
    """输出包含（3/4 视 + 侧视 + 正对喇叭口直视 + 背面接口视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各视角：
    # 3/4 等轴视 (看三节阶梯空心外扩喇叭口全貌、骨夹箍夹耳、紧贴外壳放射筋丝)
    im_iso, _ = render(model_p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    # 侧视 (SIDE: yaw=90, pitch=0 看从 -Z 到 +Z 三级阶梯逐级外扩与放射筋丝紧贴走台阶)
    im_side, _ = render(model_p, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # 正对喇叭口直视 (yaw=180, pitch=0 从 +Z 正面直视 16x15 切角喇叭口内壁与底部 4x4 真元内光)
    im_mouth, _ = render(model_p, yaw=180.0, pitch=0.0, size=500, bg=bg_color)
    # 背面直视 (yaw=0, pitch=0 从 -Z 直视 8x6 统一截面经脉接口与骨夹箍耳)
    im_inlet, _ = render(model_p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格拼版：3/4 等轴视、侧视、正对喇叭口、背面接口)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("3/4 ISOMETRIC (Flaring Bell & Bone Clamp)", im_iso, 10, 10),
        ("SIDE VIEW (Stepped Flaring 8x6 -> 12x10 -> 16x15)", im_side, cell_w + 20, 10),
        ("FRONT VIEW (+Z Straight Into Bell Mouth & Qi)", im_mouth, 10, cell_h + 20),
        ("BACK VIEW (-Z Standard 8x6 Inlet & Clamps)", im_inlet, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 440, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (3/4 + 侧视 + 正对喇叭口 + 接口 2x2 拼版) 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图，右当前模型 3/4 等轴视与正对喇叭口视并排等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_iso = im_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_mouth = im_mouth.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b08_coupling.png)", fill=(210, 200, 180))

        # 贴右渲染图 (左 3/4 视，右正对喇叭口视)
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_mouth, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: Frontal Bell Mouth View)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含 3/4 视与喇叭口对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_coupling.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] coupling 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [ 4.0, 8.0, -5.0],
        "material": "tendon_tube",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] coupling 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: coupling 注入共面冲突未被拦截!")

    print("✓ gen_coupling.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b08 coupling 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p = generate_bbmodel(out_path=MODEL_DIR / "coupling.bbmodel")
    render_views(p)


if __name__ == "__main__":
    main()
