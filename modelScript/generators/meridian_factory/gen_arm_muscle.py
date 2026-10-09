#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o03: arm_muscle (上肢肌肉组)

风格：A 有机型 (活体血肉、半透明筋管/肌腱、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 外包围与尺寸：2 宽 × 2 高 × 3 长方块 (严格落在 32×32×48 px 空间内：x in [-16..16], y in [0..32], z in [-24..24])，
   原点位于底面中心 (0.0, 0.0, 0.0)。
2. 7 束长红肌纤维束 (6~8 束)：
   - #8a2a2a (flesh_main) 红肌主色，每束由一串沿长轴 Z 逐渐收细的长方块构成；
   - 束间嵌有 #5a1a1a (flesh_dark) 细缝，突出多束肌纤维编织立体感；
   - 表面隆起棱线上配有 #b05050 (flesh_lit) 纵向高光条；
   - 另一端 (z = -24.0) 纤维呈散开、圆钝的饱满肌腹头端。
3. 白色肌腱收拢汇聚段：
   - 向 +Z 端逐渐收细汇拢，由渐变暖粉白 #e8bca8 (tendon_trans) 汇入坚实致密的白色骨化肌腱 #d8ccb0 (bone_main)。
4. 骨环 + 8×6 统一截面耦合插口：
   - 肌腱末端位于 +Z 侧端面 (z = 24.0)；
   - 严格遵循统一连接截面：内孔宽 8 px × 高 6 px、底边离地 2 px (整件底面 y=0.0，所以孔径在 y in [2.0, 8.0], x in [-4.0, 4.0])；
   - 外周紧扣四面一体的骨环外箍 (#d8ccb0 / #b8a888)，插口内衬亮色内衬并透出淡光内芯 (#f6dcc4)。
5. 门禁要求：通过 _assert_no_coplanar_faces 自检，带 --self-test 差分缺陷拦截验证。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "arm_muscle.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/arm_muscle")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o03_arm_muscle")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o03_arm_muscle.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 束间细缝与暗部
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 红肌纤维主色
    "flesh_lit":     (176, 80, 80, 255),   # #b05050 肌纤维高光条与圆钝端光泽
    "tendon_trans":  (232, 188, 168, 255), # #e8bca8 渐变白色肌腱 (过渡)
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨环与坚实致密肌腱
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨暗面
    "qi_glow":       (246, 220, 196, 255), # #f6dcc4 插口光流
}

MAT_UV = {
    "flesh_main":    [0, 0, 16, 16],
    "flesh_lit":     [16, 0, 32, 16],
    "flesh_dark":    [32, 0, 48, 16],
    "tendon_trans":  [48, 0, 64, 16],
    "bone_main":     [0, 16, 16, 32],
    "bone_dark":     [16, 16, 32, 32],
    "qi_glow":       [32, 16, 48, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_muscle_bundles() -> List[dict]:
    """1. 7 束长红肌纤维束（沿长轴渐变收细，含高光条、束间细缝与散开圆钝端）。

    7 束纤维环绕肌腹核心排列，向 -Z 端散开圆钝，向 +Z 端收细汇拢：
    - 束 0: 中央核心主束
    - 束 1: 背侧上肌束 (带背脊高光条)
    - 束 2: 腹侧底肌束 (靠地暗面)
    - 束 3: 左外侧肌束 (大弧外凸，带侧面高光)
    - 束 4: 右外侧肌束 (大弧外凸，带侧面高光)
    - 束 5: 左上斜向肌束
    - 束 6: 右上斜向肌束
    """
    cubes = []

    # ── 束 0: 中央主束 (core) ──
    # z: -23.8 -> -14.0 -> -2.0 -> 10.0 -> 17.5
    cubes.append({"name": "b0_seg_1", "from": [-3.5, 10.0, -23.8], "to": [ 3.5, 17.0, -14.0], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b0_seg_2", "from": [-4.5, 11.0, -14.0], "to": [ 4.5, 19.0,  -2.0], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b0_seg_3", "from": [-3.5,  8.0,  -2.0], "to": [ 3.5, 14.5,  10.0], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b0_seg_4", "from": [-2.5,  5.5,  10.0], "to": [ 2.5, 10.0,  17.5], "group": "muscle_bundles", "material": "flesh_lit"})

    # ── 束 1: 背侧上束 (dorsal top) ──
    # z: -23.0 -> -13.2 -> -1.2 -> 10.8 -> 17.8
    cubes.append({"name": "b1_seg_1", "from": [-3.0, 17.5, -23.0], "to": [ 3.0, 23.5, -13.2], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b1_hl_1",  "from": [-1.5, 23.6, -22.2], "to": [ 1.5, 24.6, -13.6], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b1_seg_2", "from": [-3.8, 19.5, -13.2], "to": [ 3.8, 26.5,  -1.2], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b1_hl_2",  "from": [-1.8, 26.6, -12.8], "to": [ 1.8, 27.8,  -1.6], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b1_seg_3", "from": [-2.8, 15.0,  -1.2], "to": [ 2.8, 20.5,  10.8], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b1_hl_3",  "from": [-1.2, 20.6,  -0.8], "to": [ 1.2, 21.6,  10.2], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b1_seg_4", "from": [-1.8, 10.2,  10.8], "to": [ 1.8, 14.5,  17.8], "group": "muscle_bundles", "material": "tendon_trans"})

    # ── 束 2: 腹侧底束 (ventral bottom) ──
    # z: -23.2 -> -14.8 -> -2.8 -> 9.2 -> 17.2
    cubes.append({"name": "b2_seg_1", "from": [-3.0,  3.5, -23.2], "to": [ 3.0,  9.5, -14.8], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "b2_seg_2", "from": [-3.8,  3.0, -14.8], "to": [ 3.8, 10.5,  -2.8], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "b2_seg_3", "from": [-3.0,  2.0,  -2.8], "to": [ 3.0,  7.5,   9.2], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "b2_seg_4", "from": [-2.2,  1.8,   9.2], "to": [ 2.2,  5.2,  17.2], "group": "muscle_bundles", "material": "flesh_main"})

    # ── 束 3: 左外侧束 (lateral left, x < 0) ──
    # z: -23.5 -> -13.6 -> -1.6 -> 10.4 -> 17.4
    cubes.append({"name": "b3_seg_1", "from": [-11.5,  8.5, -23.5], "to": [-4.0, 16.5, -13.6], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b3_hl_1",  "from": [-12.6, 11.5, -22.5], "to": [-11.6, 14.5, -14.0], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b3_seg_2", "from": [-14.5,  9.5, -13.6], "to": [-5.0, 18.5,  -1.6], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b3_hl_2",  "from": [-15.7, 12.5, -13.0], "to": [-14.6, 16.0,  -2.2], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b3_seg_3", "from": [-10.5,  7.0,  -1.6], "to": [-3.8, 14.0,  10.4], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b3_hl_3",  "from": [-11.4,  9.5,  -1.0], "to": [-10.6, 12.5,   9.8], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b3_seg_4", "from": [ -6.5,  4.5,  10.4], "to": [-2.8,  9.5,  17.4], "group": "muscle_bundles", "material": "tendon_trans"})

    # ── 束 4: 右外侧束 (lateral right, x > 0) ──
    # z: -23.5 -> -14.4 -> -2.4 -> 9.6 -> 17.6
    cubes.append({"name": "b4_seg_1", "from": [ 4.0,  8.5, -23.5], "to": [11.5, 16.5, -14.4], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b4_hl_1",  "from": [11.6, 11.5, -22.5], "to": [12.6, 14.5, -14.8], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b4_seg_2", "from": [ 5.0,  9.5, -14.4], "to": [14.5, 18.5,  -2.4], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b4_hl_2",  "from": [14.6, 12.5, -13.8], "to": [15.7, 16.0,  -2.8], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b4_seg_3", "from": [ 3.8,  7.0,  -2.4], "to": [10.5, 14.0,   9.6], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b4_hl_3",  "from": [10.6,  9.5,  -1.8], "to": [11.4, 12.5,   9.0], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b4_seg_4", "from": [ 2.8,  4.5,   9.6], "to": [ 6.5,  9.3,  17.6], "group": "muscle_bundles", "material": "tendon_trans"})

    # ── 束 5: 左上斜束 (dorso-lateral left) ──
    # z: -22.8 -> -13.0 -> -1.0 -> 11.0 -> 18.0
    cubes.append({"name": "b5_seg_1", "from": [-9.0, 15.5, -22.8], "to": [-3.2, 21.5, -13.0], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b5_hl_1",  "from": [-9.8, 19.0, -22.0], "to": [-6.5, 22.2, -13.5], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b5_seg_2", "from": [-11.0, 17.5, -13.0], "to": [-4.0, 24.5,  -1.0], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b5_hl_2",  "from": [-12.0, 22.0, -12.5], "to": [-8.0, 25.4,  -1.5], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b5_seg_3", "from": [ -8.0, 13.0,  -1.0], "to": [-3.0, 18.5,  11.0], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b5_hl_3",  "from": [ -8.8, 16.5,  -0.5], "to": [-5.5, 19.4,  10.4], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b5_seg_4", "from": [ -5.0,  8.5,  11.0], "to": [-2.0, 13.0,  18.0], "group": "muscle_bundles", "material": "tendon_trans"})

    # ── 束 6: 右上斜束 (dorso-lateral right) ──
    # z: -22.8 -> -14.2 -> -2.2 -> 9.8 -> 17.0
    cubes.append({"name": "b6_seg_1", "from": [ 3.2, 15.5, -22.8], "to": [ 9.0, 21.5, -14.2], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b6_hl_1",  "from": [ 6.5, 19.0, -22.0], "to": [ 9.8, 22.2, -14.6], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b6_seg_2", "from": [ 4.0, 17.5, -14.2], "to": [11.0, 24.5,  -2.2], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b6_hl_2",  "from": [ 8.0, 22.0, -13.8], "to": [12.0, 25.4,  -2.6], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b6_seg_3", "from": [ 3.0, 13.0,  -2.2], "to": [ 8.0, 18.5,   9.8], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "b6_hl_3",  "from": [ 5.5, 16.5,  -1.8], "to": [ 8.8, 19.4,   9.2], "group": "muscle_bundles", "material": "flesh_lit"})

    cubes.append({"name": "b6_seg_4", "from": [ 2.0,  8.5,   9.8], "to": [ 5.0, 13.0,  17.0], "group": "muscle_bundles", "material": "tendon_trans"})

    # ── 束间深色细缝 (#5a1a1a) ──
    cubes.append({"name": "fissure_dorsal_l",  "from": [-3.6, 16.8, -17.5], "to": [-2.9, 19.8, 3.5], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "fissure_dorsal_r",  "from": [ 2.9, 16.8, -17.5], "to": [ 3.6, 19.8, 3.5], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "fissure_lateral_l", "from": [-8.1, 10.6, -17.5], "to": [-4.6, 11.4, 3.5], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "fissure_lateral_r", "from": [ 4.6, 10.6, -17.5], "to": [ 8.1, 11.4, 3.5], "group": "muscle_bundles", "material": "flesh_dark"})

    # ── 另一端散开圆钝端帽 (z: -24.0..-22.6) ──
    cubes.append({"name": "blunt_cap_core",    "from": [-3.0, 10.5, -24.0], "to": [ 3.0, 16.5, -22.8], "group": "muscle_bundles", "material": "flesh_main"})
    cubes.append({"name": "blunt_cap_left",    "from": [-10.0, 9.5, -23.8], "to": [-4.5, 15.5, -22.6], "group": "muscle_bundles", "material": "flesh_lit"})
    cubes.append({"name": "blunt_cap_right",   "from": [  4.5, 9.5, -23.8], "to": [10.0, 15.5, -22.6], "group": "muscle_bundles", "material": "flesh_lit"})
    cubes.append({"name": "blunt_cap_top",     "from": [-2.5, 17.6, -23.2], "to": [ 2.5, 22.4, -22.2], "group": "muscle_bundles", "material": "flesh_lit"})

    return cubes


def part_02_tendon_core() -> List[dict]:
    """2. 白色肌腱汇拢段 (z: 17.5..21.0, #e8bca8 -> #d8ccb0)。"""
    cubes = []
    # 渐变过渡肌腱外鞘 (z: 17.5..19.2, 暖粉白 #e8bca8)
    cubes.append({"name": "tendon_sheath_trans", "from": [-4.8, 1.8, 17.5], "to": [4.8, 9.5, 19.2], "group": "tendon_core", "material": "tendon_trans"})
    # 紧致汇拢坚实白色肌腱核心 (z: 19.2..21.0, 骨白 #d8ccb0)
    cubes.append({"name": "tendon_dense_core",   "from": [-4.6, 1.9, 19.2], "to": [4.6, 9.1, 21.0], "group": "tendon_core", "material": "bone_main"})
    return cubes


def part_03_coupling_socket() -> List[dict]:
    """3. 骨环 + 8x6 统一截面耦合插口 (z: 21.0..24.0)。

    统一接口截面：宽 8 px × 高 6 px、居中 (x: -4.0..4.0)、底边离地 2 px (整件底面 y=0.0，y in [2.0, 8.0])。
    肌腱末端包扣四面骨环框架 (#d8ccb0 / #b8a888)，插口开孔朝向 +Z (z=24.0)，内壁衬亮色内衬并透出淡光内芯 (#f6dcc4)。
    """
    cubes = []
    # ── 骨环外箍框架 (左右两侧通高 y: 0.5..9.5，上下两边横扣 x: -4.0..4.0，四角精准无重叠对接) ──
    cubes.append({"name": "socket_bone_ring_l", "from": [-5.5, 0.5, 21.0], "to": [-4.0, 9.5, 24.0], "group": "coupling_socket", "material": "bone_main"})
    cubes.append({"name": "socket_bone_ring_r", "from": [ 4.0, 0.5, 21.0], "to": [ 5.5, 9.5, 24.0], "group": "coupling_socket", "material": "bone_main"})
    cubes.append({"name": "socket_bone_ring_t", "from": [-4.0, 8.0, 21.0], "to": [ 4.0, 9.5, 24.0], "group": "coupling_socket", "material": "bone_main"})
    cubes.append({"name": "socket_bone_ring_b", "from": [-4.0, 0.5, 21.0], "to": [ 4.0, 2.0, 24.0], "group": "coupling_socket", "material": "bone_main"})

    # ── 耦合插口 8x6 内壁衬层 (开孔范围 x: -4.0..4.0, y: 2.0..8.0, z: 22.8..24.0) ──
    cubes.append({"name": "socket_lining_l", "from": [-4.0, 2.0, 22.8], "to": [-3.4, 8.0, 24.0], "group": "coupling_socket", "material": "tendon_trans"})
    cubes.append({"name": "socket_lining_r", "from": [ 3.4, 2.0, 22.8], "to": [ 4.0, 8.0, 24.0], "group": "coupling_socket", "material": "tendon_trans"})
    cubes.append({"name": "socket_lining_t", "from": [-3.4, 7.4, 22.8], "to": [ 3.4, 8.0, 24.0], "group": "coupling_socket", "material": "tendon_trans"})
    cubes.append({"name": "socket_lining_b", "from": [-3.4, 2.0, 22.8], "to": [ 3.4, 2.6, 24.0], "group": "coupling_socket", "material": "tendon_trans"})

    # ── 插口深部内腔真元光芯 (z: 21.0..22.8) ──
    cubes.append({"name": "socket_lumen_glow", "from": [-3.4, 2.6, 21.0], "to": [ 3.4, 7.4, 22.8], "group": "coupling_socket", "material": "qi_glow"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总上肢肌肉组全部 3 大部件立方体。"""
    return (
        part_01_muscle_bundles()
        + part_02_tendon_core()
        + part_03_coupling_socket()
    )


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
    """生成 64×64 RGBA 贴图，严格使用有机型与红肌纤维配色。"""
    rng = np.random.default_rng(20261009)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if mat_name == "flesh_main":
                    # 肌纤维纵向丝缕纹理
                    if x % 4 in (0, 1):
                        r = int(np.clip(r + 8, 0, 255))
                        g = int(np.clip(g + 4, 0, 255))
                elif mat_name == "tendon_trans":
                    if (x + y) % 5 == 0:
                        r = int(np.clip(r - 6, 0, 255))
                        g = int(np.clip(g - 6, 0, 255))
                        b = int(np.clip(b - 4, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "arm_muscle",
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
        g_name = c.get("group", "muscle")
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
    for g_name in ["muscle_bundles", "tendon_core", "coupling_socket"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [0.0, 0.0, 0.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "ArmMuscle",
        "model_identifier": "arm_muscle",
        "visible_box": [2, 2, 3],
        "geometry_name": "arm_muscle",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_bbmodel(out_path: Path = BBMODEL_OUT) -> Path:
    """执行标准生成流程并落盘 bbmodel。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    tex = build_texture()
    doc = build_bbmodel_doc(cubes, tex)

    out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    rel = out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path
    print(f"✓ 上肢肌肉组 bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 渲染四视角 (正视、侧视、3/4 等轴、俯视)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格)
    canvas = Image.new("RGB", (1040, 1040), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("FRONT (Socket, Tendon & Bundles)", im_front, 20, 20),
        ("SIDE (Full Length 48px Profile)",  im_side, 540, 20),
        ("3/4 ISOMETRIC (Fusiform Fascicles)",im_iso, 20, 540),
        ("TOP (Longitudinal Convergence)",   im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 300, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 3. 拼装 check.png (左参考图右渲染正面与 3/4 视并排)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        front_scale_w = int(im_front.width * target_h / im_front.height)
        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        front_scaled = im_front.resize((front_scale_w, target_h), Image.Resampling.LANCZOS)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = front_scale_w + iso_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考
        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (o03_arm_muscle.png: Left FRONT / Right 3/4)", fill=(210, 200, 180))

        # 贴右渲染
        rx = ref_w + 24
        check_cv.paste(front_scaled, (rx, 30))
        check_cv.paste(iso_scaled, (rx + front_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (FRONT VIEW + 3/4 ISOMETRIC VIEW)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_arm_muscle.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-3.5, 10.0, -23.8],
        "to":   [ 3.5, 17.0, -14.0],  # 与 b0_seg_1 完全重叠
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_arm_muscle.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o03 arm_muscle 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认流程：自测 -> 导出 bbmodel -> 渲染图片
    self_test()
    bb_path = generate_bbmodel()
    render_views(bb_path)


if __name__ == "__main__":
    main()
