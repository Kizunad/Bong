#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o01: lung (肺)

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

调度审第 1 次针对性返工落实：
1. 肺叶轮廓（上窄下宽、顶部收圆）：
   - 按 Y 轴分 5 级台阶，顶上两级（Tier 4 与 Tier 5）明显收窄；
   - 顶部圆拱收尖（dome apex），消除扁平方盒感；
   - 两叶之间保留清晰中缝（宽约 3~5px）；
   - 左叶略小于右叶（左肺两叶、右肺三叶），左叶内侧明显凹进形成近心切迹（cardiac notch）。
2. 正面 Y 形分叉支气管系统：
   - 气管底部分叉形成 Y 形主支气管，延伸贴附于两叶正面；
   - 两侧各自分支 2~3 次（主干 → 二级支气管 → 三级终末细支）；
   - 骨色 #d8ccb0 与暗面 #b8a888，宽约 1.0px，在叶片正表面微凸起浮起约 0.4~0.5px。
3. 顶部气管（长气管、分节软骨环与骨口）：
   - 气管从两肺叶顶部明显向上伸出约 6px（伸出段 y: 42.0..48.0）；
   - 由 6 节清晰分节软骨环（环 #d8ccb0、环缝 #6a5a48 / #b8a888）构成；
   - 顶端设有一圈空心骨口与内部幽深暗腔通道。
4. 海绵斑点表面质感：
   - 血肉主基底 #8a2a2a；
   - 散布小块肺泡粉斑点 #b05050 与凸起海绵结节；
   - 边缘与裂隙深凹暗部 #5a1a1a，拒绝大面积单色扁平。
5. 底部统一截面输出端口：
   - 严格遵循统一连接截面：宽 8 px × 高 6 px、居中（x: -4.0..4.0）、底边离地 2 px（y: 2.0..8.0）；
   - 端口外围包裹一圈 2px 厚度的肉质管套（#8a2a2a，外廓 12 宽 × 10 高，底边贴地 y=0.0）；
   - 从两叶下方中央向前朝 +Z 伸出至 z = 16.0；
   - 方口内壁衬有明亮的亮肉粉内衬（#b05050）与真元灵流光芯（#f4ece0）。
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
    "flesh_lit":    (176, 80, 80, 255),   # #b05050 亮肉 / 肺泡粉 (海绵肺泡簇、表层结节、方口内壁)
    "bone_main":    (216, 204, 176, 255), # #d8ccb0 骨 (气管软骨环、支气管前脊)
    "bone_dark":    (184, 168, 136, 255), # #b8a888 骨暗面 / 支气管底面
    "bone_crevice": (106, 90, 72, 255),   # #6a5a48 骨缝 / 环间凹槽
    "tendon_fascia":(232, 216, 208, 180), # #e8d8d0 半透明筋膜 (alpha 约 60~70%)
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
    """1. 顶部骨环气管与骨口 (trachea)。

    气管向上伸出叶顶约 6px（肺顶 y=42.0，气管顶端 y=48.0）。
    由 6 节清晰分节软骨环（环 #d8ccb0、环缝 #6a5a48）交替构成，顶端设有一圈空心骨口与内腔通道。
    """
    cubes = []
    # ── 顶部空心气管口 (y: 46.8..48.0, 顶端一圈骨口) ──
    cubes.append({
        "name": "trachea_rim_north",
        "from": [-3.2, 46.8, -4.2],
        "to":   [ 3.2, 48.0, -2.8],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_rim_south",
        "from": [-3.2, 46.8,  0.8],
        "to":   [ 3.2, 48.0,  2.2],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_rim_west",
        "from": [-3.2, 46.8, -2.8],
        "to":   [-1.8, 48.0,  0.8],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_rim_east",
        "from": [ 1.8, 46.8, -2.8],
        "to":   [ 3.2, 48.0,  0.8],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_lumen_floor",
        "from": [-1.8, 45.8, -2.8],
        "to":   [ 1.8, 46.6,  0.8],
        "group": "trachea",
        "material": "flesh_dark",
    })

    # ── 分节软骨环 (环 #d8ccb0, 缝 #6a5a48) ──
    # Ring 6 (y: 45.6..46.7)
    cubes.append({
        "name": "trachea_ring_6",
        "from": [-3.25, 45.6, -4.25],
        "to":   [ 3.25, 46.7,  2.25],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_joint_5",
        "from": [-2.7, 44.6, -3.7],
        "to":   [ 2.7, 45.6,  1.7],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # Ring 5 (y: 43.4..44.6)
    cubes.append({
        "name": "trachea_ring_5",
        "from": [-3.2, 43.4, -4.2],
        "to":   [ 3.2, 44.6,  2.2],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_joint_4",
        "from": [-2.7, 42.4, -3.7],
        "to":   [ 2.7, 43.4,  1.7],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # Ring 4 (y: 41.2..42.4, 此处对应肺尖水平线 y=42.0)
    cubes.append({
        "name": "trachea_ring_4",
        "from": [-3.2, 41.2, -4.2],
        "to":   [ 3.2, 42.4,  2.2],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_joint_3",
        "from": [-2.7, 40.2, -3.7],
        "to":   [ 2.7, 41.2,  1.7],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # Ring 3 (y: 39.0..40.2)
    cubes.append({
        "name": "trachea_ring_3",
        "from": [-3.2, 39.0, -4.2],
        "to":   [ 3.2, 40.2,  2.2],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_joint_2",
        "from": [-2.7, 38.0, -3.7],
        "to":   [ 2.7, 39.0,  1.7],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # Ring 2 (y: 36.6..38.0)
    cubes.append({
        "name": "trachea_ring_2",
        "from": [-3.3, 36.6, -4.3],
        "to":   [ 3.3, 38.0,  2.3],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_joint_1",
        "from": [-2.8, 35.6, -3.8],
        "to":   [ 2.8, 36.6,  1.8],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # 隆嵴底座 (y: 34.0..35.6)
    cubes.append({
        "name": "trachea_carina_base",
        "from": [-3.6, 34.0, -4.5],
        "to":   [ 3.6, 35.6,  2.5],
        "group": "trachea",
        "material": "bone_main",
    })

    return cubes


def part_02_bronchial_tree() -> List[dict]:
    """2. 正面 Y 形分叉支气管系统 (bronchial_tree)。

    从气管底部分出 Y 形主支气管伸进两叶，再各自分出 2~3 次叉，贴在叶片正面。
    骨色 #d8ccb0 / 暗面 #b8a888，宽约 1.0px，在叶片正表面微凸起浮起约 0.4~0.5px。
    """
    cubes = []
    # ── 右肺支气管系统 (伸向右肺叶正面 x < 0) ──
    # 右主干 (从隆嵴伸向右叶正表面)
    cubes.append({
        "name": "bronchus_r_trunk_1",
        "from": [-4.5, 33.6, 3.2],
        "to":   [-0.5, 35.1, 4.3],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_r_trunk_2",
        "from": [-8.2, 31.2, 7.6],
        "to":   [-4.0, 33.5, 8.8],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })

    # 右上分叉 (深入上叶)
    cubes.append({
        "name": "bronchus_r_sec_up",
        "from": [-12.2, 33.4, 7.7],
        "to":   [-7.5,  35.7, 8.7],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_r_twig_a1",
        "from": [-15.2, 35.9, 6.3],
        "to":   [-11.5, 38.1, 7.4],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })
    cubes.append({
        "name": "bronchus_r_twig_a2",
        "from": [-14.0, 32.4, 7.8],
        "to":   [-10.8, 34.3, 8.9],
        "group": "bronchial_tree",
        "material": "bone_main",
    })

    # 右中分叉 (深入中叶)
    cubes.append({
        "name": "bronchus_r_sec_mid",
        "from": [-14.2, 28.6,  8.9],
        "to":   [-8.0,  31.1, 10.1],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_r_twig_b1",
        "from": [-18.2, 27.4,  9.1],
        "to":   [-13.5, 29.3, 10.2],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })
    cubes.append({
        "name": "bronchus_r_twig_b2",
        "from": [-17.2, 24.6,  9.5],
        "to":   [-13.0, 27.1, 10.6],
        "group": "bronchial_tree",
        "material": "bone_main",
    })

    # 右下分叉 (深入下叶)
    cubes.append({
        "name": "bronchus_r_sec_low",
        "from": [-10.8, 22.6,  9.3],
        "to":   [-7.0,  28.4, 10.4],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_r_twig_c1",
        "from": [-11.8, 16.6,  9.8],
        "to":   [-8.5,  22.4, 10.8],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })
    cubes.append({
        "name": "bronchus_r_twig_c2",
        "from": [-16.2, 18.6,  9.3],
        "to":   [-11.0, 20.7, 10.3],
        "group": "bronchial_tree",
        "material": "bone_main",
    })

    # ── 左肺支气管系统 (伸向左肺叶正面 x > 0) ──
    # 左主干
    cubes.append({
        "name": "bronchus_l_trunk_1",
        "from": [ 0.5, 33.6, 3.2],
        "to":   [ 4.5, 35.1, 4.3],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_l_trunk_2",
        "from": [ 3.8, 31.2, 7.1],
        "to":   [ 7.8, 33.5, 8.3],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })

    # 左上分叉
    cubes.append({
        "name": "bronchus_l_sec_up",
        "from": [ 7.0, 33.4, 7.2],
        "to":   [11.2, 35.7, 8.2],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_l_twig_a1",
        "from": [10.5, 35.6, 5.8],
        "to":   [13.8, 37.9, 6.9],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })

    # 左中分叉
    cubes.append({
        "name": "bronchus_l_sec_mid",
        "from": [ 7.0, 28.2, 8.6],
        "to":   [12.2, 30.7, 9.8],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_l_twig_b1",
        "from": [11.8, 27.2, 8.7],
        "to":   [16.2, 29.1, 9.9],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })
    cubes.append({
        "name": "bronchus_l_twig_b2",
        "from": [11.5, 24.2, 9.0],
        "to":   [15.2, 26.9, 10.1],
        "group": "bronchial_tree",
        "material": "bone_main",
    })

    # 左下分叉
    cubes.append({
        "name": "bronchus_l_sec_low",
        "from": [ 6.5, 22.3,  8.8],
        "to":   [ 9.8, 27.9, 10.0],
        "group": "bronchial_tree",
        "material": "bone_main",
    })
    cubes.append({
        "name": "bronchus_l_twig_c1",
        "from": [ 7.5, 16.2,  9.0],
        "to":   [10.5, 21.8, 10.1],
        "group": "bronchial_tree",
        "material": "bone_dark",
    })
    cubes.append({
        "name": "bronchus_l_twig_c2",
        "from": [10.0, 18.2,  8.8],
        "to":   [14.2, 20.5,  9.8],
        "group": "bronchial_tree",
        "material": "bone_main",
    })

    return cubes


def part_03_right_lobe() -> List[dict]:
    """3. 海绵状右肺叶 (right_lobe, 前视图左侧 x < 0)。

    按 Y 轴分 5 级台阶，呈现上窄下宽、顶部收圆的解剖结构。
    体积偏大偏饱满，最大宽度在 Tier 2（腹部）展开至 x = -22.5px，带有起伏肺泡粉结节簇。
    """
    cubes = []
    # ── Tier 5: 肺尖圆拱 (Apex, y: 37.0..42.0, 顶上收圆收窄) ──
    cubes.append({
        "name": "lobe_r_t5_dome",
        "from": [-10.5, 40.5, -4.5],
        "to":   [-4.0,  42.0,  4.5],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_r_t5_body",
        "from": [-13.5, 37.0, -6.5],
        "to":   [-2.2,  40.5,  6.5],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_r_t5_nod_f",
        "from": [-12.0, 37.4,  6.5],
        "to":   [-3.5,  40.2,  7.3],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_r_t5_nod_b",
        "from": [-12.0, 37.4, -7.3],
        "to":   [-3.5,  40.2, -6.5],
        "group": "right_lobe",
        "material": "flesh_dark",
    })

    # ── Tier 4: 上叶 (Upper Lobe, y: 30.0..37.0, 明显收窄过渡段) ──
    cubes.append({
        "name": "lobe_r_t4_body",
        "from": [-17.5, 30.0, -8.5],
        "to":   [-1.8,  37.0,  8.5],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_r_t4_nod_f",
        "from": [-16.0, 30.8,  8.5],
        "to":   [-3.0,  36.3,  9.6],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_r_t4_nod_b",
        "from": [-16.0, 30.8, -9.6],
        "to":   [-3.0,  36.3, -8.5],
        "group": "right_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_r_t4_lat",
        "from": [-18.8, 30.8, -6.5],
        "to":   [-17.4, 36.2,  6.5],
        "group": "right_lobe",
        "material": "flesh_main",
    })

    # ── Tier 3: 中叶 (Middle Lobe, y: 22.0..30.0, 中段展开) ──
    cubes.append({
        "name": "lobe_r_t3_body",
        "from": [-21.0, 22.0, -10.0],
        "to":   [-1.5,  30.0,  10.0],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_r_t3_nod_f",
        "from": [-19.5, 22.8,  10.1],
        "to":   [-3.0,  29.2,  11.3],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_r_t3_nod_b",
        "from": [-19.5, 22.8, -11.3],
        "to":   [-3.0,  29.2, -10.1],
        "group": "right_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_r_t3_lat",
        "from": [-22.4, 22.8, -8.0],
        "to":   [-20.9, 29.2,  8.0],
        "group": "right_lobe",
        "material": "flesh_lit",
    })

    # ── Tier 2: 下叶上腹 (Lower Lobe Upper, y: 14.0..22.0, 最大宽度展开至 x = -22.5) ──
    cubes.append({
        "name": "lobe_r_t2_body",
        "from": [-22.5, 14.0, -10.5],
        "to":   [-1.2,  22.0,  10.5],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_r_t2_nod_f",
        "from": [-21.0, 14.8,  10.6],
        "to":   [-2.5,  21.2,  11.9],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_r_t2_nod_b",
        "from": [-21.0, 14.8, -11.9],
        "to":   [-2.5,  21.2, -10.6],
        "group": "right_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_r_t2_lat",
        "from": [-23.6, 14.8, -8.5],
        "to":   [-22.4, 21.2,  8.5],
        "group": "right_lobe",
        "material": "flesh_lit",
    })

    # ── Tier 1: 肺底座 (Lung Base, y: 7.0..14.0, 底部圆润收敛) ──
    cubes.append({
        "name": "lobe_r_t1_body",
        "from": [-20.5,  7.0, -9.0],
        "to":   [-1.8,  14.0,  9.0],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_r_t1_base",
        "from": [-18.5,  6.0, -8.0],
        "to":   [-2.5,  7.5,   7.8],
        "group": "right_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_r_t1_lat",
        "from": [-21.8,  8.5, -7.0],
        "to":   [-20.4, 13.0,  7.0],
        "group": "right_lobe",
        "material": "flesh_lit",
    })

    return cubes


def part_04_left_lobe() -> List[dict]:
    """4. 海绵状左肺叶 (left_lobe, 前视图右侧 x > 0)。

    按 Y 轴分 5 级台阶，同样上窄下宽、顶部收圆。
    略小于右叶，内侧形成明显的近心切迹（cardiac notch，向内凹陷凹槽）。
    """
    cubes = []
    # ── Tier 5: 肺尖圆拱 (Apex, y: 37.0..42.0, 顶上收圆收窄) ──
    cubes.append({
        "name": "lobe_l_t5_dome",
        "from": [ 3.5, 40.5, -4.0],
        "to":   [ 9.5, 42.0,  4.0],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_l_t5_body",
        "from": [ 2.2, 37.0, -6.0],
        "to":   [12.5, 40.5,  6.0],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_l_t5_nod_f",
        "from": [ 3.5, 37.4,  6.0],
        "to":   [11.5, 40.2,  6.8],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_l_t5_nod_b",
        "from": [ 3.5, 37.4, -6.8],
        "to":   [11.5, 40.2, -6.0],
        "group": "left_lobe",
        "material": "flesh_dark",
    })

    # ── Tier 4: 上叶 (Upper Lobe, y: 30.0..37.0, 明显收窄过渡段) ──
    cubes.append({
        "name": "lobe_l_t4_body",
        "from": [ 1.8, 30.0, -8.0],
        "to":   [16.0, 37.0,  8.0],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_l_t4_nod_f",
        "from": [ 2.8, 30.8,  8.1],
        "to":   [14.5, 36.3,  9.1],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_l_t4_nod_b",
        "from": [ 2.8, 30.8, -9.1],
        "to":   [14.5, 36.3, -8.1],
        "group": "left_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_l_t4_lat",
        "from": [15.9, 30.8, -6.0],
        "to":   [17.2, 36.2,  6.0],
        "group": "left_lobe",
        "material": "flesh_main",
    })

    # ── Tier 3: 中段 (Mid, y: 22.0..30.0, 心切迹凹槽开始出现，内缘退至 x=3.0) ──
    cubes.append({
        "name": "lobe_l_t3_body",
        "from": [ 3.0, 22.0, -9.5],
        "to":   [18.5, 30.0,  9.5],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_l_t3_nod_f",
        "from": [ 3.8, 22.8,  9.6],
        "to":   [17.0, 29.2, 10.7],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "lobe_l_t3_nod_b",
        "from": [ 3.8, 22.8, -10.7],
        "to":   [17.0, 29.2, -9.6],
        "group": "left_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_l_t3_lat",
        "from": [18.4, 22.8, -7.5],
        "to":   [19.8, 29.2,  7.5],
        "group": "left_lobe",
        "material": "flesh_lit",
    })

    # ── Tier 2: 下叶腹部 (Lower Lobe Belly, y: 14.0..22.0, 心切迹最深处，内缘退至 x=3.8) ──
    cubes.append({
        "name": "lobe_l_t2_body",
        "from": [ 3.8, 14.0, -10.0],
        "to":   [20.0, 22.0,  10.0],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_l_t2_nod_f",
        "from": [ 4.5, 14.8,  10.1],
        "to":   [18.5, 21.2,  11.3],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_l_t2_nod_b",
        "from": [ 4.5, 14.8, -11.3],
        "to":   [18.5, 21.2, -10.1],
        "group": "left_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_l_t2_lat",
        "from": [19.9, 14.8, -8.0],
        "to":   [21.2, 21.2,  8.0],
        "group": "left_lobe",
        "material": "flesh_lit",
    })

    # ── Tier 1: 肺底座 (Lung Base, y: 7.0..14.0, 底部圆润收敛) ──
    cubes.append({
        "name": "lobe_l_t1_body",
        "from": [ 2.5,  7.0, -8.5],
        "to":   [18.0, 14.0,  8.5],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "lobe_l_t1_base",
        "from": [ 3.5,  6.0, -7.5],
        "to":   [16.5,  7.5,   7.8],
        "group": "left_lobe",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "lobe_l_t1_lat",
        "from": [17.9,  8.5, -6.5],
        "to":   [19.4, 13.0,  6.5],
        "group": "left_lobe",
        "material": "flesh_lit",
    })

    return cubes


def part_05_output_port() -> List[dict]:
    """5. 底部统一截面输出端口 (output_port)。

    统一接口截面：宽 8 px × 高 6 px、居中 (x: -4.0..4.0)、底边离地 2 px (y: 2.0..8.0)。
    外面一圈肉质管套 (#8a2a2a，厚度 2px，外廓宽 12px × 高 10px，底边贴地 y=0.0)，
    从两叶下方中央向前朝 +Z 伸出至 z = 16.0，内部带有亮肉内壁衬层 (#b05050) 与真元光流核 (#f4ece0)。
    """
    cubes = []
    # ── 1. 汇流漏斗连接颈 (y: 6.0..12.5, z: 0.0..7.8) ──
    cubes.append({
        "name": "port_duct_upper",
        "from": [-5.2, 7.8, -0.4],
        "to":   [ 5.2, 12.5, 7.8],
        "group": "output_port",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "port_duct_lower",
        "from": [-5.0, 2.1,  0.2],
        "to":   [ 5.0, 7.8,  7.9],
        "group": "output_port",
        "material": "flesh_dark",
    })

    # ── 2. 统一截面肉质管套 (外面一圈肉质管套 #8a2a2a, 2px 厚, z: 8.0..16.0) ──
    # 标准端口内孔为 x: -4..4, y: 2..8
    # 左套壁 (x: -6.0..-4.0, y: 2.0..8.0)
    cubes.append({
        "name": "port_sleeve_left",
        "from": [-6.0, 2.0,  8.0],
        "to":   [-4.0, 8.0, 16.0],
        "group": "output_port",
        "material": "flesh_main",
    })
    # 右套壁 (x: 4.0..6.0, y: 2.0..8.0)
    cubes.append({
        "name": "port_sleeve_right",
        "from": [ 4.0, 2.0,  8.0],
        "to":   [ 6.0, 8.0, 16.0],
        "group": "output_port",
        "material": "flesh_main",
    })
    # 顶套壁 (x: -6.0..6.0, y: 8.0..10.0)
    cubes.append({
        "name": "port_sleeve_top",
        "from": [-6.0,  8.0,  8.0],
        "to":   [ 6.0, 10.0, 16.0],
        "group": "output_port",
        "material": "flesh_main",
    })
    # 底套壁 (x: -6.0..6.0, y: 0.0..2.0, 贴地 y=0.0)
    cubes.append({
        "name": "port_sleeve_bot",
        "from": [-6.0, 0.0,  8.0],
        "to":   [ 6.0, 2.0, 16.0],
        "group": "output_port",
        "material": "flesh_main",
    })

    # ── 3. 亮肉方口内壁衬层 (带亮肉内壁的方口 #b05050, 位于 8x6 方口内缘) ──
    cubes.append({
        "name": "port_lining_left",
        "from": [-4.0, 2.0, 14.5],
        "to":   [-3.2, 8.0, 16.0],
        "group": "output_port",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "port_lining_right",
        "from": [ 3.2, 2.0, 14.5],
        "to":   [ 4.0, 8.0, 16.0],
        "group": "output_port",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "port_lining_top",
        "from": [-3.2, 7.2, 14.5],
        "to":   [ 3.2, 8.0, 16.0],
        "group": "output_port",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "port_lining_bot",
        "from": [-3.2, 2.0, 14.5],
        "to":   [ 3.2, 2.8, 16.0],
        "group": "output_port",
        "material": "flesh_lit",
    })

    # ── 4. 内腔真元光流核与深部暗腔 (z: 12.0..14.5) ──
    cubes.append({
        "name": "port_lumen_glow",
        "from": [-3.2, 2.8, 12.0],
        "to":   [ 3.2, 7.2, 14.5],
        "group": "output_port",
        "material": "qi_glow",
    })

    # ── 5. 底部暗血肉锚定肉垫 ──
    cubes.append({
        "name": "port_base_footing",
        "from": [-15.0, 0.0, -8.0],
        "to":   [ 15.0, 1.8,  8.0],
        "group": "output_port",
        "material": "flesh_dark",
    })

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
        ("SIDE (Depth 32px Profile)",            im_side, 540, 20),
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
        "from": [-3.2, 46.8, -4.2],
        "to":   [ 3.2, 48.0, -2.8],  # 与 trachea_rim_north 完全重叠
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
