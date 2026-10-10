#!/usr/bin/env python3
"""经脉工厂内景构件生成器 —— b14: meridian_damage (受损经脉管，3 个变体)

风格：A 有机型 (活体血肉、破损撕裂经脉、淤血污染、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 以 b01 meridian_straight 已放行版为基底，尺寸严格落在一格 16×16×16 空间内 (X/Z in [-8, 8], Y in [0, 16])。
2. 3 个变体：
   - meridian_cracked (开裂)：管壁正面增设 4 条 #5a1a1a (flesh_dark) 1px 斜裂纹，展现经脉受震开裂的裂痕脉络；
   - meridian_severed (断裂)：管身中段 (z in [-2.0, 2.0]，对应 0-16 空间的 z=6-10) 完全切除镂空断开，
     两端断口处各增设 2~3 个参差 1px 的 #e8bca8 (tendon_highlight) 撕裂碎片向断口空隙犬牙交错伸展；
   - meridian_contaminated (污染)：管内中段塞入 3 块 #2a1430 / #4a2048 的不规则血块 (3~4px 大小重叠结节)，
     中段管壁颜色暗一档 (#5a1a1a / #6a4440)，发光窗被暗紫淤血覆盖。
3. 门禁要求：通过 _assert_no_coplanar_faces 严格自检（0 共面冲突），带 --self-test 差分缺陷拦截验证。
4. 不写 manifest（遵守根 CLAUDE.md 纪律）。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_damage")
REF_EXPLODED = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x14_meridian_damage.png")
REF_SINGLE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b14_meridian_damage.png")

# 引入 b01 标准构件定义
sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_meridian_straight as b01

# 调色板 (对齐第 1 批规范与 b14 污染/开裂专属色)
PALETTE = {
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 / 裂纹 / 孔底
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 两端肉箍主色
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 贴面小骨环端口
    "tendon_tube":      (217, 160, 140, 150), # #d9a08c 暖粉半透明筋管壁
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋管亮边 / 撕裂碎片
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元内光芯 / 6x2 淡光斑
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 斜交筋丝
    "tube_dark":        (106, 68, 64, 255),   # #6a4440 污染段暗化筋管壁
    "clot_main":        (74, 32, 72, 255),    # #4a2048 污染紫红淤血块
    "clot_dark":        (42, 20, 48, 255),    # #2a1430 污染深色凝血块
}

MAT_UV = {
    "flesh_dark":       [0,  0, 16, 16],
    "flesh_main":       [16, 0, 32, 16],
    "bone_main":        [32, 0, 48, 16],
    "tendon_tube":      [48, 0, 64, 16],
    "qi_glow":          [0, 16, 16, 32],
    "tendon_fiber":     [16, 16, 32, 32],
    "tendon_highlight": [32, 16, 48, 32],
    "tube_dark":        [48, 16, 64, 32],
    "clot_main":        [0, 32, 16, 48],
    "clot_dark":        [16, 32, 32, 48],
}

RES = 64


# =============================================================================
# 各变体几何定义
# =============================================================================

def build_cracked_cubes() -> List[dict]:
    """1. meridian_cracked (开裂)：管壁正面增设 4 条 #5a1a1a 1px 锯齿斜裂纹。"""
    cubes = b01.all_cubes()

    # 4 条清晰斜向裂纹 (浮于正面管壁 x=4.0 处 0.14px: x in [4.01, 4.14]，绝对零共面)
    # 裂纹 1: 后下至中段 (y: 3.2..5.6, z: -3.8..-1.8)
    cubes.append({"name": "crack_1_1", "from": [4.01, 3.2, -3.8], "to": [4.14, 4.4, -2.7], "group": "cracks", "material": "flesh_dark"})
    cubes.append({"name": "crack_1_2", "from": [4.01, 4.4, -2.7], "to": [4.14, 5.6, -1.8], "group": "cracks", "material": "flesh_dark"})

    # 裂纹 2: 中上至顶端 (y: 5.6..7.6, z: -1.75..-0.45)
    cubes.append({"name": "crack_2_1", "from": [4.01, 5.6, -1.75], "to": [4.14, 6.6, -1.05], "group": "cracks", "material": "flesh_dark"})
    cubes.append({"name": "crack_2_2", "from": [4.01, 6.6, -1.05], "to": [4.14, 7.6, -0.45], "group": "cracks", "material": "flesh_dark"})

    # 裂纹 3: 中前段向右下斜裂 (y: 2.6..5.8, z: 0.35..2.65)
    cubes.append({"name": "crack_3_1", "from": [4.01, 2.6, 0.35], "to": [4.14, 4.2, 1.45], "group": "cracks", "material": "flesh_dark"})
    cubes.append({"name": "crack_3_2", "from": [4.01, 4.2, 1.45], "to": [4.14, 5.8, 2.65], "group": "cracks", "material": "flesh_dark"})

    # 裂纹 4: 前肉箍附近微裂隙 (y: 4.6..6.8, z: 2.75..4.15)
    cubes.append({"name": "crack_4_1", "from": [4.01, 4.6, 2.75], "to": [4.14, 6.8, 4.15], "group": "cracks", "material": "flesh_dark"})

    return cubes


def build_severed_cubes() -> List[dict]:
    """2. meridian_severed (断裂)：管身中段 (z in [-2.0, 2.0]) 切除，断口配撕裂碎片。"""
    cubes = []
    # 1. 肉箍与端口 (两端 z in [-7.5, -4.5] 与 [4.5, 7.5] 完整保留)
    cubes.extend(b01.part_01_flesh_collars())
    cubes.extend(b01.part_02_ports())

    # 2. 筋管主体：在 z in [-2.0, 2.0] 完全断开
    # 左侧壁：后段 [-8, -2], 前段 [2, 8]
    cubes.append({"name": "tube_wall_l_rear",  "from": [-4.0, 2.0, -8.0], "to": [-3.2, 8.0, -2.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_l_front", "from": [-4.0, 2.0,  2.0], "to": [-3.2, 8.0,  8.0], "group": "meridian_tube", "material": "tendon_tube"})

    # 右侧壁 (正面观察面)：
    # 后段：[-8.0, -4.5], 底部亮边 [-4.5, -2.0], 顶部亮边 [-4.5, -2.0], 侧翼 [-4.5, -3.0], 减截发光窗 [-3.0, -2.0]
    cubes.append({"name": "tube_wall_r_rear",         "from": [3.2, 2.0, -8.0], "to": [4.0, 8.0, -4.5], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_mid_bot_r",     "from": [3.2, 2.0, -4.5], "to": [4.0, 3.0, -2.0], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_mid_top_r",     "from": [3.2, 7.0, -4.5], "to": [4.0, 8.0, -2.0], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_flank_r",       "from": [3.2, 3.0, -4.5], "to": [4.0, 7.0, -3.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_glow_sub_b_r", "from": [3.2, 3.0, -3.0], "to": [4.0, 4.0, -2.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_glow_sub_t_r", "from": [3.2, 6.0, -3.0], "to": [4.0, 7.0, -2.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_glow_win_r",   "from": [3.2, 4.0, -3.0], "to": [4.0, 6.0, -2.0], "group": "meridian_tube", "material": "qi_glow"})

    # 前段：[4.5, 8.0], 底部亮边 [2.0, 4.5], 顶部亮边 [2.0, 4.5], 侧翼 [3.0, 4.5], 减截发光窗 [2.0, 3.0]
    cubes.append({"name": "tube_wall_r_front",         "from": [3.2, 2.0,  4.5], "to": [4.0, 8.0,  8.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_mid_bot_f",     "from": [3.2, 2.0,  2.0], "to": [4.0, 3.0,  4.5], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_mid_top_f",     "from": [3.2, 7.0,  2.0], "to": [4.0, 8.0,  4.5], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_flank_f",       "from": [3.2, 3.0,  3.0], "to": [4.0, 7.0,  4.5], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_glow_sub_b_f", "from": [3.2, 3.0,  2.0], "to": [4.0, 4.0,  3.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_glow_sub_t_f", "from": [3.2, 6.0,  2.0], "to": [4.0, 7.0,  3.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_glow_win_f",   "from": [3.2, 4.0,  2.0], "to": [4.0, 6.0,  3.0], "group": "meridian_tube", "material": "qi_glow"})

    # 顶壁与底壁：后段 [-8, -2], 前段 [2, 8]
    cubes.append({"name": "tube_wall_bot_r", "from": [-3.2, 2.0, -8.0], "to": [3.2, 2.8, -2.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_bot_f", "from": [-3.2, 2.0,  2.0], "to": [3.2, 2.8,  8.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_top_r", "from": [-3.2, 7.2, -8.0], "to": [3.2, 8.0, -2.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_top_f", "from": [-3.2, 7.2,  2.0], "to": [3.2, 8.0,  8.0], "group": "meridian_tube", "material": "tendon_tube"})

    # 3. 内部真元核心：在 [-2, 2] 断开，断口微凸发光
    cubes.append({"name": "inner_qi_core_r", "from": [-1.0, 4.0, -8.0], "to": [1.0, 6.0, -1.8], "group": "inner_qi_glow", "material": "qi_glow"})
    cubes.append({"name": "inner_qi_core_f", "from": [-1.0, 4.0,  1.8], "to": [1.0, 6.0,  8.0], "group": "inner_qi_glow", "material": "qi_glow"})

    # 4. 断口两端参差撕裂碎片 (各 2~3 个 #e8bca8 碎片，伸入空隙 0.8~1.5px)
    # 后断口 (z=-2.0 处向前伸展):
    cubes.append({"name": "shred_r_top", "from": [ 3.22, 6.2, -2.0], "to": [ 3.98, 7.8, -1.0], "group": "tear_shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_r_mid", "from": [ 3.24, 3.8, -2.0], "to": [ 3.96, 5.0, -0.6], "group": "tear_shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_r_bot", "from": [ 3.22, 2.2, -2.0], "to": [ 3.98, 3.2, -1.2], "group": "tear_shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_l_mid", "from": [-3.98, 4.2, -2.0], "to": [-3.22, 5.8, -0.8], "group": "tear_shreds", "material": "tendon_highlight"})

    # 前断口 (z=2.0 处向后伸展):
    cubes.append({"name": "shred_f_top", "from": [ 3.22, 6.8,  1.0], "to": [ 3.98, 7.8,  2.0], "group": "tear_shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_f_mid", "from": [ 3.24, 4.4,  0.5], "to": [ 3.96, 5.6,  2.0], "group": "tear_shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_f_bot", "from": [ 3.22, 2.2,  1.2], "to": [ 3.98, 3.4,  2.0], "group": "tear_shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_l_top", "from": [-3.98, 5.4,  0.8], "to": [-3.22, 7.0,  2.0], "group": "tear_shreds", "material": "tendon_highlight"})

    return cubes


def build_contaminated_cubes() -> List[dict]:
    """3. meridian_contaminated (污染)：中段管内塞入 3 块 #2a1430/#4a2048 血块，管壁暗化。"""
    cubes = []
    # 1. 肉箍与端口
    cubes.extend(b01.part_01_flesh_collars())
    cubes.extend(b01.part_02_ports())

    # 2. 筋管：两端正常，中段 z in [-3.0, 3.0] 颜色暗一档 (#6a4440)
    # 左侧壁：后段 [-8, -3], 中段 [-3, 3] (暗化), 前段 [3, 8]
    cubes.append({"name": "tube_wall_l_rear",  "from": [-4.0, 2.0, -8.0], "to": [-3.2, 8.0, -3.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_l_mid",   "from": [-4.0, 2.0, -3.0], "to": [-3.2, 8.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_l_front", "from": [-4.0, 2.0,  3.0], "to": [-3.2, 8.0,  8.0], "group": "meridian_tube", "material": "tendon_tube"})

    # 右侧壁 (正面)：
    # 后端隐藏与亮边
    cubes.append({"name": "tube_wall_r_rear",      "from": [3.2, 2.0, -8.0], "to": [4.0, 8.0, -4.5], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_mid_bot_r",  "from": [3.2, 2.0, -4.5], "to": [4.0, 3.0, -3.0], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_mid_top_r",  "from": [3.2, 7.0, -4.5], "to": [4.0, 8.0, -3.0], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_flank_r",    "from": [3.2, 3.0, -4.5], "to": [4.0, 7.0, -3.0], "group": "meridian_tube", "material": "tendon_tube"})

    # 中段污染管壁 (z in [-3.0, 3.0]，颜色暗一档，发光窗被暗紫淤血覆盖)
    cubes.append({"name": "tube_wall_r_mid_bot_m",  "from": [3.2, 2.0, -3.0], "to": [4.0, 3.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_r_mid_top_m",  "from": [3.2, 7.0, -3.0], "to": [4.0, 8.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_r_flank_m_b",  "from": [3.2, 3.0, -3.0], "to": [4.0, 4.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_r_flank_m_t",  "from": [3.2, 6.0, -3.0], "to": [4.0, 7.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_r_clot_win",   "from": [3.2, 4.0, -3.0], "to": [4.0, 6.0,  3.0], "group": "meridian_tube", "material": "clot_main"})

    # 前端隐藏与亮边
    cubes.append({"name": "tube_wall_r_front",      "from": [3.2, 2.0,  4.5], "to": [4.0, 8.0,  8.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_r_mid_bot_f",  "from": [3.2, 2.0,  3.0], "to": [4.0, 3.0,  4.5], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_mid_top_f",  "from": [3.2, 7.0,  3.0], "to": [4.0, 8.0,  4.5], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "tube_wall_r_flank_f",    "from": [3.2, 3.0,  3.0], "to": [4.0, 7.0,  4.5], "group": "meridian_tube", "material": "tendon_tube"})

    # 顶底壁
    cubes.append({"name": "tube_wall_bot_r", "from": [-3.2, 2.0, -8.0], "to": [3.2, 2.8, -3.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_bot_m", "from": [-3.2, 2.0, -3.0], "to": [3.2, 2.8,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_bot_f", "from": [-3.2, 2.0,  3.0], "to": [3.2, 2.8,  8.0], "group": "meridian_tube", "material": "tendon_tube"})

    cubes.append({"name": "tube_wall_top_r", "from": [-3.2, 7.2, -8.0], "to": [3.2, 8.0, -3.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "tube_wall_top_m", "from": [-3.2, 7.2, -3.0], "to": [3.2, 8.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_wall_top_f", "from": [-3.2, 7.2,  3.0], "to": [3.2, 8.0,  8.0], "group": "meridian_tube", "material": "tendon_tube"})

    # 3. 内部真元核心：两端正常发光，中段截断
    cubes.append({"name": "inner_qi_core_r", "from": [-1.0, 4.0, -8.0], "to": [1.0, 6.0, -3.0], "group": "inner_qi_glow", "material": "qi_glow"})
    cubes.append({"name": "inner_qi_core_f", "from": [-1.0, 4.0,  3.0], "to": [1.0, 6.0,  8.0], "group": "inner_qi_glow", "material": "qi_glow"})

    # 4. 中段管内塞入 3 块不规则血块 (#2a1430 / #4a2048, 3~4px):
    cubes.append({"name": "clot_block_1", "from": [-2.0, 3.2, -2.8], "to": [1.8, 6.8, 0.4], "group": "blood_clots", "material": "clot_dark"})
    cubes.append({"name": "clot_block_2", "from": [-1.2, 3.8, -0.2], "to": [2.2, 7.0, 2.6], "group": "blood_clots", "material": "clot_main"})
    cubes.append({"name": "clot_block_3", "from": [-2.4, 2.8,  0.5], "to": [0.8, 5.4, 2.9], "group": "blood_clots", "material": "clot_dark"})

    # 5. 斜交筋丝
    cubes.extend(b01.part_05_diagonal_fibers())

    return cubes


# =============================================================================
# 门禁与共面冲突自检
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """严格检查立方体集是否存在同向同坐标且投影相交的共面冲突 (Z-fighting)。"""
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
                rect = (min(f[1], t[1]), min(f[2], t[2]), max(f[1], t[1]), max(f[2], t[2]))
            elif axis == 1:
                rect = (min(f[0], t[0]), min(f[2], t[2]), max(f[0], t[0]), max(f[2], t[2]))
            else:
                rect = (min(f[0], t[0]), min(f[1], t[1]), max(f[0], t[0]), max(f[1], t[1]))
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


# =============================================================================
# 贴图与 Blockbench 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图。"""
    rng = np.random.default_rng(20261011)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if mat_name == "clot_main":
                    # 淤血块暗紫斑驳
                    if (x * 2 + y * 3) % 5 == 0:
                        r = int(np.clip(r + 14, 0, 255))
                        b = int(np.clip(b + 12, 0, 255))
                elif mat_name == "tube_dark":
                    # 污染管壁微血斑
                    if (x + y * 4) % 7 in (0, 1):
                        r = int(np.clip(r + 10, 0, 255))
                        g = int(np.clip(g - 6, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image, model_id: str) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": model_id,
        "folder": "meridian_factory",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": texture_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: Dict[str, List[str]] = {}

    for idx, c in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        g_name = c.get("group", "meridian")
        groups_map.setdefault(g_name, []).append(elem_uuid)

        mat_key = c.get("material", "flesh_main")
        u0, v0, u1, v1 = MAT_UV.get(mat_key, [0, 0, 16, 16])
        span_x = max(1, u1 - u0 - 4)
        span_y = max(1, v1 - v0 - 4)
        uv_u = u0 + (idx * 2) % span_x
        uv_v = v0 + (idx * 3) % span_y
        uv_box = [float(uv_u), float(uv_v), float(uv_u + 4), float(uv_v + 4)]

        faces = {
            face_name: {"uv": uv_box, "texture": 0}
            for face_name in ("north", "east", "south", "west", "up", "down")
        }

        element = {
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in c["from"]],
            "to":   [round(float(v), 4) for v in c["to"]],
            "autouv": 0,
            "color": 0,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        elements.append(element)

    outliner = []
    for g_name, u_list in groups_map.items():
        outliner.append({
            "name": g_name,
            "origin": [0.0, 0.0, 0.0],
            "color": 0,
            "uuid": str(uuid.uuid4()),
            "isOpen": True,
            "children": u_list,
        })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": f"MeridianDamage_{model_id}",
        "model_identifier": model_id,
        "visible_box": [1, 1, 1],
        "geometry_name": model_id,
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_all_bbmodels() -> Dict[str, Path]:
    """生成 3 个变体及默认 bbmodel。"""
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    tex = build_texture()

    variants = {
        "meridian_cracked":      build_cracked_cubes(),
        "meridian_severed":      build_severed_cubes(),
        "meridian_contaminated": build_contaminated_cubes(),
    }

    result_paths = {}
    for var_id, cubes in variants.items():
        _assert_no_coplanar_faces(cubes)
        out_p = MODEL_DIR / f"{var_id}.bbmodel"
        doc = build_bbmodel_doc(cubes, tex, var_id)
        out_p.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
        result_paths[var_id] = out_p

    # 默认主模型 meridian_damage.bbmodel (等同于 meridian_cracked)
    def_p = MODEL_DIR / "meridian_damage.bbmodel"
    doc_def = build_bbmodel_doc(variants["meridian_cracked"], tex, "meridian_damage")
    def_p.write_text(json.dumps(doc_def, indent=2, ensure_ascii=False), encoding="utf-8")
    result_paths["default"] = def_p

    print(f"✓ b14 受损经脉管 3 变体 + 默认主模型生成完成: {MODEL_DIR}")
    return result_paths


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(model_paths: Dict[str, Path]):
    """输出三变体 3/4 + 侧视 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    items = ["meridian_cracked", "meridian_severed", "meridian_contaminated"]
    titles = {
        "meridian_cracked":      "Cracked (开裂 - 4条斜裂纹)",
        "meridian_severed":      "Severed (断裂 - 中段镂空+撕裂碎片)",
        "meridian_contaminated": "Contaminated (污染 - 3块血块+暗化管壁)",
    }

    rendered_iso = {}
    rendered_side = {}

    for k in items:
        bb_p = model_paths[k]
        im_i, _ = render(bb_p, yaw=-35.0, pitch=25.0, size=400, bg=bg_color)
        im_s, _ = render(bb_p, yaw=0.0,   pitch=0.0,  size=400, bg=bg_color)
        rendered_iso[k]  = im_i
        rendered_side[k] = im_s

    # 1. 拼装 render.png (2 行 3 列: 上行 3/4 等轴视，下行侧视图看管壁破损/中段断开/内部血块)
    col_w = 400
    row_h = 400
    pad = 16
    margin_x = 24
    margin_y = 36

    total_w = margin_x * 2 + 3 * col_w + 2 * pad
    total_h = margin_y * 2 + 2 * row_h + 20 + pad

    canvas = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    draw.text((margin_x, 12), "b14 meridian_damage (Top Row: 3/4 View; Bottom Row: SIDE View: Cracked, Severed, Contaminated)", fill=(230, 230, 230))

    for idx, k in enumerate(items):
        px = margin_x + idx * (col_w + pad)
        py_iso  = margin_y + 20
        py_side = py_iso + row_h + pad

        canvas.paste(rendered_iso[k], (px, py_iso))
        draw.rectangle([px, py_iso, px + col_w, py_iso + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_iso + 5), f"{titles[k]} 3/4", fill=(220, 220, 220))

        canvas.paste(rendered_side[k], (px, py_side))
        draw.rectangle([px, py_side, px + col_w, py_side + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_side + 5), f"{titles[k]} SIDE", fill=(180, 200, 220))

    r_path = REVIEW_DIR / "render.png"
    canvas.save(r_path)
    print(f"✓ render.png 已输出: {r_path}")

    # 2. 拼装 check.png (左爆炸图，右 3 变体并排渲染图)
    ref_to_use = REF_EXPLODED if REF_EXPLODED.exists() else REF_SINGLE
    if ref_to_use.exists():
        ref_im = Image.open(ref_to_use).convert("RGB")
        target_h = 560
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        item_chk_w = int(target_h * 0.72)
        right_sub_w = item_chk_w * 3 + 2 * 12
        right_cv = Image.new("RGB", (right_sub_w, target_h), (119, 119, 119))

        for idx, k in enumerate(items):
            scaled_v = rendered_iso[k].resize((item_chk_w, item_chk_w), Image.Resampling.LANCZOS)
            rx = idx * (item_chk_w + 12)
            ry = (target_h - item_chk_w) // 2
            right_cv.paste(scaled_v, (rx, ry))

        total_w_check = ref_w + right_sub_w + 32
        check_cv = Image.new("RGB", (total_w_check, target_h + 36), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 28))
        c_draw.text((16, 6), "EXPLODED REFERENCE (x14_meridian_damage.png)", fill=(210, 200, 180))

        check_cv.paste(right_cv, (ref_w + 20, 28))
        c_draw.text((ref_w + 20, 6), "NOW RENDER (Cracked / Severed / Contaminated)", fill=(180, 220, 210))

        c_path = REVIEW_DIR / "check.png"
        check_cv.save(c_path)
        print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：3 变体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_meridian_damage.py 差分自证...")
    c_cracked = build_cracked_cubes()
    _assert_no_coplanar_faces(c_cracked)

    c_severed = build_severed_cubes()
    _assert_no_coplanar_faces(c_severed)

    c_cont = build_contaminated_cubes()
    _assert_no_coplanar_faces(c_cont)
    print("  [OK] 3 变体正常立方体集全部无共面冲突")

    # 注入测试缺陷 (在 cracked 中注入重叠方块)
    defect_cubes = list(c_cracked) + [{
        "name": "inject_coplanar_fail",
        "from": [4.01, 3.2, -3.8],
        "to":   [4.14, 4.4, -2.7],  # 与 crack_1_1 完全重叠
        "material": "flesh_dark",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_meridian_damage.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景构件 b14 meridian_damage 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认流程：自测 -> 导出所有模型 bbmodel -> 渲染图片
    self_test()
    model_paths = generate_all_bbmodels()
    render_views(model_paths)


if __name__ == "__main__":
    main()
