#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o01: lung (肺)

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

调度审第 2 次精确坐标规范落实：
1. 肺叶逐行坐标与宽度表 (严格按 2px 一行往上堆，共 13 行)：
   - 左叶每行宽度 (从下到上)：14, 15, 16, 16, 16, 15, 15, 14, 13, 12, 10, 8, 6 px；
   - 右叶每行宽度 (从下到上)：15, 16, 17, 17, 17, 16, 16, 15, 14, 13, 11, 9, 7 px；
   - 每行中缝侧做成竖直直线（左叶内侧固定 x = +1.5，右叶内侧固定 x = -1.5），两叶间保留严格 3px 中缝；
   - 整体呈现下宽上窄、顶部圆润收尖。
2. 前后厚度随高度三段变化：
   - 下半（行 0..5，y: 10..22）：厚度 28px (z in [-14, 14])；
   - 上半（行 6..10，y: 22..32）：厚度 24px (z in [-12, 12])；
   - 最上两行（行 11..12，y: 32..36）：厚度 20px (z in [-10, 10])。
3. 正面连续树状支气管：
   - 气管下端在中缝离地约 30px 处分出左右两条主支；
   - 每条用 1×1×1 骨色方块连续斜向下外走 8 步（每步 x 外移 1、y 下降 1），贴在叶片正面外；
   - 每条主支在第 4 步分出一条 4 步长小支（向上外，每步 x 外移 1、y 上升 1）；
   - 在第 8 步分出一条 4 步长小支（向下外，每步 x 外移 1、y 下降 1），连贯不断开。
4. 顶部气管（长气管、分节软骨环与骨口）：
   - 气管从中缝 y=30 向上伸展至 y=42（高出叶顶整整 6px）；
   - 设 5 节分节软骨环（环 #d8ccb0、环缝 #6a5a48）与顶端空心骨口、内腔暗槽。
5. 海绵斑点表面质感：
   - 血肉主色 #8a2a2a 为底，密布小块肺泡粉 #b05050 斑点与 #5a1a1a 孔隙暗部。
6. 底部统一截面输出端口：
   - 8×6 标准连接截面（内孔宽 8px × 高 6px，x: -4..4, y: 2..8）；
   - 外包 2px 厚肉质管套（#8a2a2a，x: -6..6, y: 0..10, z: 8..16，底边贴地 y=0）；
   - 8×6 方口内壁衬有亮肉内壁衬层（#b05050）与真元光芯（#f4ece0）。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "lung.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/lung")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o01_lung")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o01_lung.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":   (90, 26, 26, 255),    # #5a1a1a 暗血肉 (裂隙、内腔、暗部基底)
    "flesh_main":   (138, 42, 42, 255),   # #8a2a2a 血肉 (肺叶主色、肉套外壁)
    "flesh_lit":    (176, 80, 80, 255),   # #b05050 亮肉 / 肺泡粉 (海绵肺泡簇、方口内壁)
    "bone_main":    (216, 204, 176, 255), # #d8ccb0 骨 (气管软骨环、支气管主干)
    "bone_dark":    (184, 168, 136, 255), # #b8a888 骨暗面 / 支气管分支
    "bone_crevice": (106, 90, 72, 255),   # #6a5a48 骨缝 / 环间凹槽
    "tendon_fascia":(232, 216, 208, 180), # #e8d8d0 半透明筋膜
    "qi_glow":      (244, 236, 224, 255), # #f4ece0 筋管内光 (淡光流)
}

MAT_UV = {
    "flesh_main":    [0, 0, 16, 16],
    "flesh_lit":     [16, 0, 32, 16],
    "flesh_dark":    [32, 0, 48, 16],
    "bone_main":     [48, 0, 64, 16],
    "bone_dark":     [0, 16, 16, 32],
    "bone_crevice":  [16, 16, 32, 32],
    "tendon_fascia": [32, 16, 48, 32],
    "qi_glow":       [48, 16, 64, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_trachea() -> List[dict]:
    """1. 顶部气管（分节软骨环与空心骨口）。

    气管从中缝 y=29.8 向上伸出至 y=42.0（高出肺叶顶部 y=36 整整 6px）。
    包含 5 节分节软骨环（环 #d8ccb0、环缝 #6a5a48），顶端设有一圈空心骨口与内腔通道。
    """
    cubes = []
    # ── 顶端空心骨口 (y: 41.0..42.0) ──
    cubes.append({"name": "trachea_rim_n", "from": [-2.8, 41.0, -3.8], "to": [ 2.8, 42.0, -2.4], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_rim_s", "from": [-2.8, 41.0,  0.4], "to": [ 2.8, 42.0,  1.8], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_rim_w", "from": [-2.8, 41.0, -2.4], "to": [-1.4, 42.0,  0.4], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_rim_e", "from": [ 1.4, 41.0, -2.4], "to": [ 2.8, 42.0,  0.4], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_stoma_floor", "from": [-1.4, 40.2, -2.4], "to": [ 1.4, 40.8, 0.4], "group": "trachea", "material": "flesh_dark"})

    # ── 分节软骨环与环缝 (y: 29.8..41.0) ──
    # Ring 5 (y: 39.8..41.0)
    cubes.append({"name": "trachea_ring_5", "from": [-2.85, 39.8, -3.85], "to": [ 2.85, 41.0,  1.85], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_joint_4", "from": [-2.3, 38.8, -3.3], "to": [ 2.3, 39.8,  1.3], "group": "trachea", "material": "bone_crevice"})
    # Ring 4 (y: 37.6..38.8)
    cubes.append({"name": "trachea_ring_4", "from": [-2.8, 37.6, -3.8], "to": [ 2.8, 38.8,  1.8], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_joint_3", "from": [-2.3, 36.6, -3.3], "to": [ 2.3, 37.6,  1.3], "group": "trachea", "material": "bone_crevice"})
    # Ring 3 (y: 35.4..36.6, 对应肺顶水平 y=36)
    cubes.append({"name": "trachea_ring_3", "from": [-2.85, 35.4, -3.85], "to": [ 2.85, 36.6,  1.85], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_joint_2", "from": [-2.3, 34.4, -3.3], "to": [ 2.3, 35.4,  1.3], "group": "trachea", "material": "bone_crevice"})
    # Ring 2 (y: 33.2..34.4)
    cubes.append({"name": "trachea_ring_2", "from": [-2.8, 33.2, -3.8], "to": [ 2.8, 34.4,  1.8], "group": "trachea", "material": "bone_main"})
    cubes.append({"name": "trachea_joint_1", "from": [-2.3, 32.2, -3.3], "to": [ 2.3, 33.2,  1.3], "group": "trachea", "material": "bone_crevice"})
    # Ring 1 隆嵴底座 (y: 29.8..32.2)
    cubes.append({"name": "trachea_ring_1", "from": [-2.9, 29.8, -3.9], "to": [ 2.9, 32.2,  1.9], "group": "trachea", "material": "bone_main"})

    return cubes


def part_02_bronchial_tree() -> List[dict]:
    """2. 正面树状支气管系统 (bronchial_tree)。

    气管下端（中缝里，离地约 30px）分出两条主支，每条用 1×1×1 的骨色方块连续斜向下外走 8 步
    （每步 x 外移 1、y 下降 1），贴在叶片正面外；
    每条主支在第 4 步和第 8 步各分出一条 4 步长的小支（一条向上外、一条向下外）。不产生断开。
    """
    cubes = []

    # ── 右肺支气管系统 (x < 0) ──
    # 主支 8 步 (从 x=-1.5, y=30 斜向下外，步长 dx=-1, dy=-1)
    for s in range(1, 9):
        x_in  = -1.5 - (s - 1) * 1.0
        x_out = -1.5 - s * 1.0
        y_top = 30.0 - (s - 1) * 1.0
        y_bot = 30.0 - s * 1.0
        z_f = 12.2 if y_bot >= 22.0 else 14.2
        cubes.append({
            "name": f"bronchus_r_main_{s}",
            "from": [min(x_in, x_out), y_bot, z_f],
            "to":   [max(x_in, x_out), y_top, z_f + 1.0],
            "group": "bronchial_tree",
            "material": "bone_main",
        })

    # 第 4 步向上外小支 (4 步长, 每步 dx=-1, dy=+1)
    for k in range(1, 5):
        x_in  = -5.5 - (k - 1) * 1.0
        x_out = -5.5 - k * 1.0
        y_bot = 27.0 + (k - 1) * 1.0
        y_top = 27.0 + k * 1.0
        cubes.append({
            "name": f"bronchus_r_up_{k}",
            "from": [min(x_in, x_out), y_bot, 12.2],
            "to":   [max(x_in, x_out), y_top, 13.2],
            "group": "bronchial_tree",
            "material": "bone_dark",
        })

    # 第 8 步向下外小支 (4 步长, 每步 dx=-1, dy=-1)
    for k in range(1, 5):
        x_in  = -9.5 - (k - 1) * 1.0
        x_out = -9.5 - k * 1.0
        y_top = 22.0 - (k - 1) * 1.0
        y_bot = 22.0 - k * 1.0
        cubes.append({
            "name": f"bronchus_r_down_{k}",
            "from": [min(x_in, x_out), y_bot, 14.2],
            "to":   [max(x_in, x_out), y_top, 15.2],
            "group": "bronchial_tree",
            "material": "bone_dark",
        })

    # ── 左肺支气管系统 (x > 0) ──
    # 主支 8 步 (从 x=1.5, y=30 斜向下外，步长 dx=+1, dy=-1)
    for s in range(1, 9):
        x_in  = 1.5 + (s - 1) * 1.0
        x_out = 1.5 + s * 1.0
        y_top = 30.0 - (s - 1) * 1.0
        y_bot = 30.0 - s * 1.0
        z_f = 12.2 if y_bot >= 22.0 else 14.2
        cubes.append({
            "name": f"bronchus_l_main_{s}",
            "from": [min(x_in, x_out), y_bot, z_f],
            "to":   [max(x_in, x_out), y_top, z_f + 1.0],
            "group": "bronchial_tree",
            "material": "bone_main",
        })

    # 第 4 步向上外小支 (4 步长, 每步 dx=+1, dy=+1)
    for k in range(1, 5):
        x_in  = 5.5 + (k - 1) * 1.0
        x_out = 5.5 + k * 1.0
        y_bot = 27.0 + (k - 1) * 1.0
        y_top = 27.0 + k * 1.0
        cubes.append({
            "name": f"bronchus_l_up_{k}",
            "from": [min(x_in, x_out), y_bot, 12.2],
            "to":   [max(x_in, x_out), y_top, 13.2],
            "group": "bronchial_tree",
            "material": "bone_dark",
        })

    # 第 8 步向下外小支 (4 步长, 每步 dx=+1, dy=-1)
    for k in range(1, 5):
        x_in  = 9.5 + (k - 1) * 1.0
        x_out = 9.5 + k * 1.0
        y_top = 22.0 - (k - 1) * 1.0
        y_bot = 22.0 - k * 1.0
        cubes.append({
            "name": f"bronchus_l_down_{k}",
            "from": [min(x_in, x_out), y_bot, 14.2],
            "to":   [max(x_in, x_out), y_top, 15.2],
            "group": "bronchial_tree",
            "material": "bone_dark",
        })

    return cubes


def part_03_right_lobe() -> List[dict]:
    """3. 右肺叶 (right_lobe, x < 0)。

    按 2px 一行往上堆，共 13 行 (y: 10..36)。
    右叶每行宽度 (从下到上)：15, 16, 17, 17, 17, 16, 16, 15, 14, 13, 11, 9, 7 px。
    中缝侧严格对齐 x = -1.5 保持竖直，厚度下半 28px、上半 24px、顶两行 20px。
    """
    cubes = []
    right_widths = [15, 16, 17, 17, 17, 16, 16, 15, 14, 13, 11, 9, 7]

    for i in range(13):
        y0 = 10.0 + i * 2.0
        y1 = y0 + 2.0
        if i <= 5:
            z0, z1 = -14.0, 14.0
        elif i <= 10:
            z0, z1 = -12.0, 12.0
        else:
            z0, z1 = -10.0, 10.0

        w_r = right_widths[i]
        cubes.append({
            "name": f"lobe_r_row_{i}",
            "from": [-1.5 - w_r, y0, z0],
            "to":   [-1.5,       y1, z1],
            "group": "right_lobe",
            "material": "flesh_main" if i % 2 == 0 else "flesh_lit",
        })

    return cubes


def part_04_left_lobe() -> List[dict]:
    """4. 左肺叶 (left_lobe, x > 0)。

    按 2px 一行往上堆，共 13 行 (y: 10..36)。
    左叶每行宽度 (从下到上)：14, 15, 16, 16, 16, 15, 15, 14, 13, 12, 10, 8, 6 px。
    中缝侧严格对齐 x = +1.5 保持竖直，厚度下半 28px、上半 24px、顶两行 20px。
    """
    cubes = []
    left_widths = [14, 15, 16, 16, 16, 15, 15, 14, 13, 12, 10, 8, 6]

    for i in range(13):
        y0 = 10.0 + i * 2.0
        y1 = y0 + 2.0
        if i <= 5:
            z0, z1 = -14.0, 14.0
        elif i <= 10:
            z0, z1 = -12.0, 12.0
        else:
            z0, z1 = -10.0, 10.0

        w_l = left_widths[i]
        cubes.append({
            "name": f"lobe_l_row_{i}",
            "from": [ 1.5,       y0, z0],
            "to":   [ 1.5 + w_l, y1, z1],
            "group": "left_lobe",
            "material": "flesh_main" if i % 2 == 1 else "flesh_lit",
        })

    return cubes


def part_05_output_port() -> List[dict]:
    """5. 底部统一截面输出端口 (output_port)。

    统一接口截面：宽 8 px × 高 6 px、居中 (x: -4.0..4.0)、底边离地 2 px (y: 2.0..8.0)。
    外面一圈肉质管套 (#8a2a2a，厚度 2px，外廓宽 12px × 高 10px，底边贴地 y=0.0)，
    从两叶下方中央向前朝 +Z 伸出至 z = 16.0，内部带有亮肉内壁衬层 (#b05050) 与真元光流核 (#f4ece0)。
    """
    cubes = []
    # ── 1. 肉质管套四壁 (厚度 2px, z: 8.0..16.0, 外框 12x10, 内孔 8x6) ──
    # 左管套壁 (x: -6.0..-4.0, y: 2.0..8.0)
    cubes.append({"name": "port_sleeve_l", "from": [-6.0, 2.0,  8.0], "to": [-4.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    # 右管套壁 (x: 4.0..6.0, y: 2.0..8.0)
    cubes.append({"name": "port_sleeve_r", "from": [ 4.0, 2.0,  8.0], "to": [ 6.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    # 顶管套壁 (x: -6.0..6.0, y: 8.0..10.0)
    cubes.append({"name": "port_sleeve_t", "from": [-6.0, 8.0,  8.0], "to": [ 6.0, 10.0, 16.0], "group": "output_port", "material": "flesh_main"})
    # 底管套壁 (x: -6.0..6.0, y: 0.0..2.0, 贴地 y=0.0)
    cubes.append({"name": "port_sleeve_b", "from": [-6.0, 0.0,  8.0], "to": [ 6.0, 2.0, 16.0], "group": "output_port", "material": "flesh_main"})

    # ── 2. 亮肉方口内壁衬层 (带亮肉内壁的方口 #b05050, 位于 8x6 方口内缘) ──
    cubes.append({"name": "port_lining_l", "from": [-4.0, 2.0, 14.8], "to": [-3.2, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_r", "from": [ 3.2, 2.0, 14.8], "to": [ 4.0, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_t", "from": [-3.2, 7.2, 14.8], "to": [ 3.2, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_b", "from": [-3.2, 2.0, 14.8], "to": [ 3.2, 2.8, 16.0], "group": "output_port", "material": "flesh_lit"})

    # ── 3. 内腔真元光流核与深部腔道 (z: 13.0..14.8) ──
    cubes.append({"name": "port_lumen_glow", "from": [-3.2, 2.8, 13.0], "to": [ 3.2, 7.2, 14.8], "group": "output_port", "material": "qi_glow"})

    # ── 4. 汇流漏斗连接颈与底部锚定肉垫 ──
    cubes.append({"name": "port_duct_back", "from": [-5.0, 2.0, -2.0], "to": [5.0, 9.8, 8.0], "group": "output_port", "material": "flesh_dark"})
    cubes.append({"name": "port_ground_bed", "from": [-14.0, 0.0, -10.0], "to": [14.0, 1.8, 8.0], "group": "output_port", "material": "flesh_dark"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总肺器官全部 5 大部件立方体。"""
    return (
        part_01_trachea()
        + part_02_bronchial_tree()
        + part_03_right_lobe()
        + part_04_left_lobe()
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
# 贴图与 Blockbench 序列化 (海绵质感丰富着色)
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图，严格使用有机型配色，并强化海绵状斑点与微孔质感。"""
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

                # 肺叶血肉质感：#8a2a2a 为主，散布 #b05050 肺泡斑点，#5a1a1a 孔隙暗部
                if mat_name == "flesh_main":
                    # 散布 #b05050 小斑点 (约 20% 面积)
                    if (x * 7 + y * 11) % 13 in (0, 1, 2):
                        r = int(np.clip(176 + noise, 0, 255))
                        g = int(np.clip(80 + noise, 0, 255))
                        b = int(np.clip(80 + noise, 0, 255))
                    # 散布 #5a1a1a 暗孔隙 (约 15% 面积)
                    elif (x * 5 + y * 3) % 11 in (0, 1):
                        r = int(np.clip(90 + noise, 0, 255))
                        g = int(np.clip(26 + noise, 0, 255))
                        b = int(np.clip(26 + noise, 0, 255))
                elif mat_name == "flesh_lit":
                    if (x * 4 + y * 6) % 9 in (0, 1):
                        r = int(np.clip(r + 12, 0, 255))
                        g = int(np.clip(g + 10, 0, 255))
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
        "name": "lung",
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
        g_name = c.get("group", "lung")
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
    for g_name in ["trachea", "bronchial_tree", "right_lobe", "left_lobe", "output_port"]:
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
        "name": "Lung",
        "model_identifier": "lung",
        "visible_box": [3, 3, 2],
        "geometry_name": "lung",
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
    print(f"✓ 肺器官 bbmodel 写入成功: {rel}")
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
        ("FRONT (Lobes, Bronchial Tree & Port)", im_front, 20, 20),
        ("SIDE (Depth Profile)",                 im_side, 540, 20),
        ("3/4 ISOMETRIC (Organic Structure)",    im_iso, 20, 540),
        ("TOP (Cartilage Rings & Apex)",         im_top, 540, 540),
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
        c_draw.text((16, 8), "REFERENCE (o01_lung.png: Left FRONT / Right 3/4)", fill=(210, 200, 180))

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
    print("运行 gen_lung.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-2.8, 41.0, -3.8],
        "to":   [ 2.8, 42.0, -2.4],  # 与 trachea_rim_n 完全重叠
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
    print("✓ gen_lung.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o01 lung 生成器")
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
