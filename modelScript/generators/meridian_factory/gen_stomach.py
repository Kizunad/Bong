#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o02: stomach (胃)

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 外包围与尺寸：3 宽 × 3 高 × 2 深方块 (严格落在 48×48×32 px 空间内：x in [-24..24], y in [0..48], z in [-16..16])，
   原点位于底面中心 (0.0, 0.0, 0.0)。
2. 弯曲的肌肉囊（逐行坐标与宽度表，共 14 行，每行 2px 高，y in [10..38]）：
   - 展现解剖学典型的 J 形弯曲囊腔：
     - 顶部与贲门（y: 36..38）平滑承接顶部入口管；
     - 胃底向左上方拱起（y: 32..38, x 展开至 -17.5px）；
     - 胃体沿左侧大弯大幅外凸膨出（y: 20..30, x 展开至 -22.5px，宽度达 31.0px）；
     - 下腹胃窦与幽门向右下方平滑弯曲回转收敛（y: 10..18, x 转向 +4..+14）。
   - 前后厚度随高度自然阶梯收缩（下半部厚 26~28px，上半部厚 22~24px，顶行 15~19px）。
3. 顶部入口管（食道/贲门入口）：
   - 位于顶部偏右侧 (y: 38.0..48.0, x: 0.5..6.5, z: -3.0..3.0)；
   - 包含 3 节分节软骨环（环骨色 #d8ccb0、环间凹缝 #6a5a48）与顶端骨口套圈、深色内腔。
4. 正面开口露出琥珀色消化腔（胃液琥珀 #c88a30）：
   - 正面 (+Z) 中段 (y: 20.0..30.0, x: -12.0..1.5) 开设有大型肌肉观视窗口；
   - 内部深嵌充满波光与生机活性的琥珀色胃酸消化腔液 (#c88a30) 与高光酶核 (#e0a848)；
   - 窗口外围环绕一圈有机起伏的亮肉粉黏膜外唇 (#b05050)。
5. 侧下方输出端口（8×6 统一截面）：
   - 严格遵循统一连接截面：内孔宽 8 px × 高 6 px、底边离地 2 px (x: 4.0..12.0, y: 2.0..8.0)；
   - 外包 2px 厚肉质管套 (#8a2a2a，x: 2.0..14.0, y: 0.0..10.0, z: 8.0..16.0，底边贴地 y=0.0)；
   - 朝向 +Z 延伸至 z = 16.0；
   - 8×6 方口内壁衬有亮肉内壁衬层 (#b05050) 与琥珀流光内芯。
6. 门禁要求：通过 _assert_no_coplanar_faces 自检，带 --self-test 差分缺陷拦截验证。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "stomach.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/stomach")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o02_stomach")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o02_stomach.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 暗血肉 (裂隙、内腔、暗部基底)
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 血肉 (胃囊肌肉层主色、肉套外壁)
    "flesh_lit":     (176, 80, 80, 255),   # #b05050 亮肉 / 黏膜粉 (窗口内唇、方口内壁)
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨 (入口骨环、加固骨肋)
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨暗面
    "bone_crevice":  (106, 90, 72, 255),   # #6a5a48 骨缝 / 环间凹槽
    "amber_fluid":   (200, 138, 48, 255),  # #c88a30 胃液琥珀 (消化腔液)
    "amber_glow":    (224, 168, 72, 255),  # 琥珀高光与酶核
    "tendon_fascia": (217, 160, 140, 200), # #d9a08c 半透明肌腱束与韧带
}

MAT_UV = {
    "flesh_main":    [0, 0, 16, 16],
    "flesh_lit":     [16, 0, 32, 16],
    "flesh_dark":    [32, 0, 48, 16],
    "bone_main":     [48, 0, 64, 16],
    "bone_dark":     [0, 16, 16, 32],
    "bone_crevice":  [16, 16, 32, 32],
    "amber_fluid":   [32, 16, 48, 32],
    "amber_glow":    [48, 16, 64, 32],
    "tendon_fascia": [0, 32, 16, 48],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_entrance_tube() -> List[dict]:
    """1. 顶部入口管（食道/贲门入口管）。

    位于胃囊顶部偏右侧 (y: 38.0..48.0, x: 0.5..6.5, z: -3.0..3.0)。
    顶端设有一圈空心骨口与内腔通道，中段包含 3 节分节软骨环与肌肉过渡喇叭口。
    """
    cubes = []
    # ── 顶端空心骨口 (y: 46.8..48.0) ──
    cubes.append({"name": "tube_rim_n", "from": [0.5, 46.8, -3.0], "to": [6.5, 48.0, -1.8], "group": "entrance_tube", "material": "bone_main"})
    cubes.append({"name": "tube_rim_s", "from": [0.5, 46.8,  1.8], "to": [6.5, 48.0,  3.0], "group": "entrance_tube", "material": "bone_main"})
    cubes.append({"name": "tube_rim_w", "from": [0.5, 46.8, -1.8], "to": [1.7, 48.0,  1.8], "group": "entrance_tube", "material": "bone_main"})
    cubes.append({"name": "tube_rim_e", "from": [5.3, 46.8, -1.8], "to": [6.5, 48.0,  1.8], "group": "entrance_tube", "material": "bone_main"})
    # 空心骨口内底
    cubes.append({"name": "tube_stoma_floor", "from": [1.7, 45.6, -1.8], "to": [5.3, 46.6, 1.8], "group": "entrance_tube", "material": "flesh_dark"})

    # ── 分节软骨环与环间凹槽 (y: 40.6..46.8) ──
    # Ring 3 (y: 45.4..46.7)
    cubes.append({"name": "tube_ring_3", "from": [0.4, 45.4, -3.1], "to": [6.6, 46.7,  3.1], "group": "entrance_tube", "material": "bone_main"})
    cubes.append({"name": "tube_joint_2", "from": [0.9, 44.4, -2.6], "to": [6.1, 45.4,  2.6], "group": "entrance_tube", "material": "bone_crevice"})

    # Ring 2 (y: 43.0..44.4)
    cubes.append({"name": "tube_ring_2", "from": [0.4, 43.0, -3.1], "to": [6.6, 44.4,  3.1], "group": "entrance_tube", "material": "bone_main"})
    cubes.append({"name": "tube_joint_1", "from": [0.9, 42.0, -2.6], "to": [6.1, 43.0,  2.6], "group": "entrance_tube", "material": "bone_crevice"})

    # Ring 1 (y: 40.6..42.0)
    cubes.append({"name": "tube_ring_1", "from": [0.3, 40.6, -3.2], "to": [6.7, 42.0,  3.2], "group": "entrance_tube", "material": "bone_main"})

    # 肌肉过渡套筒 (y: 38.0..40.6, 外扩融入胃囊)
    cubes.append({"name": "tube_muscle_flare", "from": [0.0, 38.0, -3.5], "to": [7.0, 40.6, 3.5], "group": "entrance_tube", "material": "flesh_main"})

    return cubes


def part_02_stomach_sac() -> List[dict]:
    """2. 弯曲的肌肉囊（stomach_sac）。

    按 Y 轴分为 14 行，每行 2px 高（y in [10..38]）。
    依逐行宽度与轮廓表建造，形成饱满有力的 J 形胃囊：
    - 胃底在左上方拱起 (y: 32..38, x 达 -17.5)；
    - 胃体在左侧大弯大幅外凸 (y: 20..30, x 达 -22.5，总宽 31.0px)；
    - 胃窦与幽门向右下方回转延伸进入侧下方输出端口；
    - 在中段 y: 20..30 正面预留大型矩形消化腔开口。
    """
    cubes = []
    # 逐行外廓表: (x_min, x_max, z_min, z_max)
    rows = [
        # Row 0: y in [10, 12] - 幽门管颈与输出端口过渡
        (-2.0, 13.5, -9.0, 9.0),
        # Row 1: y in [12, 14]
        (-6.0, 13.0, -10.0, 10.0),
        # Row 2: y in [14, 16]
        (-10.0, 12.5, -11.0, 11.0),
        # Row 3: y in [16, 18] - 下胃体弧形回扫
        (-14.0, 11.5, -12.0, 12.0),
        # Row 4: y in [18, 20]
        (-17.5, 10.5, -13.0, 13.0),
        # Row 5: y in [20, 22] - 最大腹径
        (-20.5,  9.5, -13.5, 13.5),
        # Row 6: y in [22, 24] - 胃大弯最大外凸点
        (-22.5,  8.5, -14.0, 14.0),
        # Row 7: y in [24, 26]
        (-22.5,  8.0, -14.0, 14.0),
        # Row 8: y in [26, 28] - 胃体中段琥珀窗口区
        (-22.0,  7.5, -13.5, 13.5),
        # Row 9: y in [28, 30]
        (-21.0,  7.5, -13.0, 13.0),
        # Row 10: y in [30, 32] - 上胃体收缩
        (-19.5,  7.5, -12.0, 12.0),
        # Row 11: y in [32, 34] - 胃底弧顶
        (-17.5,  7.5, -11.0, 11.0),
        # Row 12: y in [34, 36] - 胃底顶部收敛
        (-15.0,  7.0, -9.5,  9.5),
        # Row 13: y in [36, 38] - 贲门与胃底穹顶
        (-11.5,  6.5, -7.5,  7.5),
    ]

    for i in range(14):
        x0, x1, z0, z1 = rows[i]
        y0 = 10.0 + i * 2.0
        y1 = y0 + 2.0

        # 行 5..9 (y: 20..30) 正面开口，为琥珀色消化腔预留窗口 (x in [-12.0, 1.5], z > 6.0)
        if 5 <= i <= 9:
            # 肌肉囊背侧与中腹实体
            cubes.append({
                "name": f"sac_row_{i}_back",
                "from": [x0, y0, z0],
                "to":   [x1, y1,  6.0],
                "group": "stomach_sac",
                "material": "flesh_main" if i % 2 == 0 else "flesh_dark",
            })
            # 窗口左侧肌肉柱壁
            if x0 < -12.0:
                cubes.append({
                    "name": f"sac_row_{i}_pillar_l",
                    "from": [x0, y0, 6.0],
                    "to":   [-12.0, y1, z1],
                    "group": "stomach_sac",
                    "material": "flesh_main",
                })
            # 窗口右侧肌肉柱壁
            if x1 > 1.5:
                cubes.append({
                    "name": f"sac_row_{i}_pillar_r",
                    "from": [1.5, y0, 6.0],
                    "to":   [x1, y1, z1],
                    "group": "stomach_sac",
                    "material": "flesh_main",
                })
        else:
            # 完整肌层实块
            cubes.append({
                "name": f"sac_row_{i}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "stomach_sac",
                "material": "flesh_main" if i % 2 == 0 else "flesh_lit",
            })

    return cubes


def part_03_amber_cavity() -> List[dict]:
    """3. 琥珀色消化腔与黏膜视窗 (amber_cavity)。

    正面开口露出充满生机光泽的琥珀色消化腔液 (#c88a30) 与高光酶核 (#e0a848)。
    外围配有起伏的亮肉粉黏膜外唇 (#b05050)，形成鲜明的视觉焦点。
    """
    cubes = []
    # 消化腔内深部琥珀色液体核心
    cubes.append({
        "name": "amber_fluid_core",
        "from": [-11.8, 20.2, 6.2],
        "to":   [  1.2, 29.8, 12.8],
        "group": "amber_cavity",
        "material": "amber_fluid",
    })
    # 消化液内部透亮高光流
    cubes.append({
        "name": "amber_fluid_glow",
        "from": [-9.5, 22.0, 7.5],
        "to":   [-1.5, 28.0, 12.2],
        "group": "amber_cavity",
        "material": "amber_glow",
    })

    # 视窗外缘起伏黏膜唇圈 (#b05050)
    # 底唇 (y: 19.8..21.2)
    cubes.append({
        "name": "amber_lip_bottom",
        "from": [-12.2, 19.8, 12.5],
        "to":   [  1.8, 21.2, 13.8],
        "group": "amber_cavity",
        "material": "flesh_lit",
    })
    # 顶唇 (y: 28.8..30.2)
    cubes.append({
        "name": "amber_lip_top",
        "from": [-12.2, 28.8, 12.2],
        "to":   [  1.8, 30.2, 13.5],
        "group": "amber_cavity",
        "material": "flesh_lit",
    })
    # 左唇 (x: -12.4..-11.2, y: 21.2..28.8)
    cubes.append({
        "name": "amber_lip_left",
        "from": [-12.4, 21.2, 12.6],
        "to":   [-11.2, 28.8, 13.9],
        "group": "amber_cavity",
        "material": "flesh_lit",
    })
    # 右唇 (x: 0.8..2.0, y: 21.2..28.8)
    cubes.append({
        "name": "amber_lip_right",
        "from": [ 0.8, 21.2, 12.4],
        "to":   [ 2.0, 28.8, 13.7],
        "group": "amber_cavity",
        "material": "flesh_lit",
    })

    return cubes


def part_04_reinforcing_ribs() -> List[dict]:
    """4. 加强骨肋与肌腱束 (reinforcing_ribs)。

    大弯外侧弧形骨质加固卡箍 (#d8ccb0 / #b8a888) 与小弯侧肌腱束 (#d9a08c)。
    强化经脉活体作坊的工业器官结构感。
    """
    cubes = []
    # 胃大弯外侧加固骨肋
    cubes.append({
        "name": "bone_rib_curvature_upper",
        "from": [-20.0, 29.0, -8.0],
        "to":   [-18.8, 33.5,  8.0],
        "group": "reinforcing_ribs",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bone_rib_curvature_mid",
        "from": [-23.2, 21.5, -9.0],
        "to":   [-22.0, 27.5,  9.0],
        "group": "reinforcing_ribs",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bone_rib_curvature_lower",
        "from": [-18.5, 15.0, -8.5],
        "to":   [-17.2, 19.5,  8.5],
        "group": "reinforcing_ribs",
        "material": "bone_dark",
    })

    # 胃小弯内侧紧固肌腱束
    cubes.append({
        "name": "tendon_lesser_curv",
        "from": [6.0, 25.8, -5.0],
        "to":   [7.8, 33.8,  5.0],
        "group": "reinforcing_ribs",
        "material": "tendon_fascia",
    })

    return cubes


def part_05_output_port() -> List[dict]:
    """5. 侧下方输出端口（8×6 统一截面）。

    幽门侧下方输出端口严格遵循统一连接截面：
    内孔宽 8 px × 高 6 px、底边离地 2 px (x in [4.0, 12.0], y in [2.0, 8.0])。
    外包一圈 2px 厚肉质管套 (#8a2a2a，x: 2.0..14.0, y: 0.0..10.0, z: 8.0..16.0，底边贴地 y=0.0)，
    从右下方朝 +Z 伸出至 z = 16.0。方口内壁衬亮肉粉内壁 (#b05050) 与琥珀光芯。
    """
    cubes = []
    # ── 1. 肉质管套四壁 (厚度 2px, z: 8.0..16.0, 外框 12x10, 内孔 8x6) ──
    # 左管套壁 (x: 2.0..4.0, y: 2.0..8.0)
    cubes.append({"name": "port_sleeve_l", "from": [ 2.0, 2.0,  8.0], "to": [ 4.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    # 右管套壁 (x: 12.0..14.0, y: 2.0..8.0)
    cubes.append({"name": "port_sleeve_r", "from": [12.0, 2.0,  8.0], "to": [14.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    # 顶管套壁 (x: 2.0..14.0, y: 8.0..10.0)
    cubes.append({"name": "port_sleeve_t", "from": [ 2.0, 8.0,  8.0], "to": [14.0, 10.0, 16.0], "group": "output_port", "material": "flesh_main"})
    # 底管套壁 (x: 2.0..14.0, y: 0.0..2.0, 贴地 y=0.0)
    cubes.append({"name": "port_sleeve_b", "from": [ 2.0, 0.0,  8.0], "to": [14.0, 2.0, 16.0], "group": "output_port", "material": "flesh_main"})

    # ── 2. 亮肉方口内壁衬层 (带亮肉内壁的方口 #b05050) ──
    cubes.append({"name": "port_lining_l", "from": [ 4.0, 2.0, 14.8], "to": [ 4.8, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_r", "from": [11.2, 2.0, 14.8], "to": [12.0, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_t", "from": [ 4.8, 7.2, 14.8], "to": [11.2, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_b", "from": [ 4.8, 2.0, 14.8], "to": [11.2, 2.8, 16.0], "group": "output_port", "material": "flesh_lit"})

    # ── 3. 端口内腔琥珀色消化液与流光核 (z: 13.0..14.8) ──
    cubes.append({"name": "port_lumen_amber", "from": [4.8, 2.8, 13.0], "to": [11.2, 7.2, 14.8], "group": "output_port", "material": "amber_fluid"})

    # ── 4. 幽门汇流通道 (从胃体向右下方幽门管套倾斜导引, z: -2.0..8.0, y: 2.0..10.0) ──
    cubes.append({"name": "pylorus_duct_main", "from": [1.0, 2.0, -2.0], "to": [13.0, 9.8, 8.0], "group": "output_port", "material": "flesh_dark"})

    # ── 5. 底部暗血肉锚定地面肉垫 ──
    cubes.append({"name": "stomach_ground_bed", "from": [-16.0, 0.0, -10.0], "to": [14.0, 1.8, 8.0], "group": "output_port", "material": "flesh_dark"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总胃器官全部 5 大部件立方体。"""
    return (
        part_01_entrance_tube()
        + part_02_stomach_sac()
        + part_03_amber_cavity()
        + part_04_reinforcing_ribs()
        + part_05_output_port()
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
    """生成 64×64 RGBA 贴图，严格使用有机型与胃液琥珀配色。"""
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
                    if (x * 7 + y * 11) % 13 in (0, 1):
                        r = int(np.clip(176 + noise, 0, 255))
                        g = int(np.clip(80 + noise, 0, 255))
                        b = int(np.clip(80 + noise, 0, 255))
                    elif (x * 5 + y * 3) % 11 in (0, 1):
                        r = int(np.clip(90 + noise, 0, 255))
                        g = int(np.clip(26 + noise, 0, 255))
                        b = int(np.clip(26 + noise, 0, 255))
                elif mat_name == "amber_fluid":
                    if (x + y * 3) % 7 == 0:
                        r = int(np.clip(r + 15, 0, 255))
                        g = int(np.clip(g + 12, 0, 255))
                        b = int(np.clip(b + 10, 0, 255))
                elif mat_name in ("bone_main", "bone_dark"):
                    if (x + y * 2) % 5 == 0:
                        r = int(np.clip(r - 8, 0, 255))
                        g = int(np.clip(g - 8, 0, 255))
                        b = int(np.clip(b - 6, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "stomach",
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
        g_name = c.get("group", "stomach")
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
    for g_name in ["entrance_tube", "stomach_sac", "amber_cavity", "reinforcing_ribs", "output_port"]:
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
        "name": "Stomach",
        "model_identifier": "stomach",
        "visible_box": [3, 3, 2],
        "geometry_name": "stomach",
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
    print(f"✓ 胃器官 bbmodel 写入成功: {rel}")
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
        ("FRONT (Curved Sac, Amber Cavity & Port)", im_front, 20, 20),
        ("SIDE (Profile & Depth)",                  im_side, 540, 20),
        ("3/4 ISOMETRIC (Organic Structure)",       im_iso, 20, 540),
        ("TOP (Entrance Tube & Fundus Arch)",       im_top, 540, 540),
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
        c_draw.text((16, 8), "REFERENCE (o02_stomach.png: Left FRONT / Right 3/4)", fill=(210, 200, 180))

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
    print("运行 gen_stomach.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [0.5, 46.8, -3.0],
        "to":   [6.5, 48.0, -1.8],  # 与 tube_rim_n 完全重叠
        "material": "bone_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_stomach.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o02 stomach 生成器")
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
