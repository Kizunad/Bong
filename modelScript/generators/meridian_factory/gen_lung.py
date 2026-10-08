#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o01: lung (肺)

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

统一约定落实：
1. 尺寸与外包围：3 宽 × 3 高 × 2 深方块 (严格落在 48×48×32 px 空间内：x: -24..24, y: 0..48, z: -16..16)。
   原点放在底面中心 (0, 0, 0)。
2. 朝向：正面朝向 +Z，输入端位于 -Z/顶部气管，输出端口位于 +Z 侧底部。
3. 连接面：底部输出端口严格遵循统一接口截面：
   宽 8 px × 高 6 px、居中 (x: -4.0..4.0)、底边离地 2 px (y: 2.0..8.0)，朝向 +Z 延伸至 z = 16.0，
   配有坚实骨环卡箍领圈 (#d8ccb0) 与输出内腔通道 (#5a1a1a / #f4ece0)。
4. 结构要点：
   - 顶部骨环气管 (骨环 #d8ccb0 与深色凹槽 #6a5a48) 以及气管隆嵴分支支气管；
   - 一对海绵状肺叶（左右略不对称，右肺三叶偏丰满展开，左肺两叶带近心切迹）；
   - 肺泡海绵状粉红凸起簇 (#b05050)、血肉主基底 (#8a2a2a) 与裂隙暗部 (#5a1a1a)；
   - 肺门脉管组织与半透明筋膜 (#e8d8d0)；
   - 底部汇流管腔与统一截面输出端口。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "lung.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/lung")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o01_lung")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o01_lung.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":   (90, 26, 26, 255),    # #5a1a1a 暗血肉 (裂隙、内腔、暗部基底)
    "flesh_main":   (138, 42, 42, 255),   # #8a2a2a 血肉 (肺叶主色、肌肉管壁)
    "flesh_lit":    (176, 80, 80, 255),   # #b05050 亮肉 / 肺泡粉 (海绵状肺泡簇、表层结节)
    "bone_main":    (216, 204, 176, 255), # #d8ccb0 骨 (气管环、端口外环、加固卡箍)
    "bone_dark":    (184, 168, 136, 255), # #b8a888 骨暗面
    "bone_crevice": (106, 90, 72, 255),   # #6a5a48 骨缝 / 阴影
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
    """1. 顶部骨环气管与分支支气管 (trachea)。

    气管由 4 节环状骨环与深色凹槽节段交替构成，下接主支气管分叉伸入左右肺门。
    """
    cubes = []
    # ── 顶部气管主干 (y: 39.0..48.0, x: -3.5..3.5, z: -4.5..2.5) ──
    # 骨环 1 (顶环, y: 46.5..48.0)
    cubes.append({
        "name": "trachea_ring_1",
        "from": [-3.5, 46.5, -4.5],
        "to":   [ 3.5, 48.0,  2.5],
        "group": "trachea",
        "material": "bone_main",
    })
    # 环间深色凹槽 1 (y: 45.5..46.5)
    cubes.append({
        "name": "trachea_crevice_1",
        "from": [-3.1, 45.5, -4.1],
        "to":   [ 3.1, 46.5,  2.1],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # 骨环 2 (y: 44.0..45.5)
    cubes.append({
        "name": "trachea_ring_2",
        "from": [-3.5, 44.0, -4.5],
        "to":   [ 3.5, 45.5,  2.5],
        "group": "trachea",
        "material": "bone_main",
    })
    # 环间深色凹槽 2 (y: 43.0..44.0)
    cubes.append({
        "name": "trachea_crevice_2",
        "from": [-3.1, 43.0, -4.1],
        "to":   [ 3.1, 44.0,  2.1],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # 骨环 3 (y: 41.5..43.0)
    cubes.append({
        "name": "trachea_ring_3",
        "from": [-3.5, 41.5, -4.5],
        "to":   [ 3.5, 43.0,  2.5],
        "group": "trachea",
        "material": "bone_main",
    })
    # 环间深色凹槽 3 (y: 40.5..41.5)
    cubes.append({
        "name": "trachea_crevice_3",
        "from": [-3.1, 40.5, -4.1],
        "to":   [ 3.1, 41.5,  2.1],
        "group": "trachea",
        "material": "bone_crevice",
    })
    # 骨环 4 / 气管隆嵴底座 (y: 39.0..40.8)
    cubes.append({
        "name": "trachea_ring_4",
        "from": [-3.6, 39.0, -4.6],
        "to":   [ 3.6, 40.8,  2.6],
        "group": "trachea",
        "material": "bone_main",
    })

    # ── 左右主支气管分叉 (y: 33.8..39.0) ──
    # 右肺主支气管 (向 -X 倾斜伸入右肺)
    cubes.append({
        "name": "trachea_bronchus_r_top",
        "from": [-7.5, 36.5, -4.0],
        "to":   [-3.4, 39.0,  2.0],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_bronchus_r_mid",
        "from": [-11.0, 33.8, -3.6],
        "to":   [-7.3,  36.5,  1.6],
        "group": "trachea",
        "material": "bone_dark",
    })
    # 左肺主支气管 (向 +X 倾斜伸入左肺)
    cubes.append({
        "name": "trachea_bronchus_l_top",
        "from": [ 3.4, 36.5, -3.9],
        "to":   [ 7.5, 39.0,  1.9],
        "group": "trachea",
        "material": "bone_main",
    })
    cubes.append({
        "name": "trachea_bronchus_l_mid",
        "from": [ 7.3, 33.8, -3.5],
        "to":   [10.5, 36.5,  1.5],
        "group": "trachea",
        "material": "bone_dark",
    })

    return cubes


def part_02_right_lobe() -> List[dict]:
    """2. 海绵状右肺叶 (right_lobe, 前视图左侧 x < 0)。

    解剖学三叶结构（上叶、中叶、下叶），体积较左侧更丰满开阔，中叶向外拱出达 x = -22.5px，
    表面呈海绵状肺泡簇起伏。
    """
    cubes = []
    # ── 右肺上叶 (y: 28.5..42.0) ──
    # 肺尖圆拱 (apex)
    cubes.append({
        "name": "lobe_r_upper_apex",
        "from": [-16.0, 38.0, -9.0],
        "to":   [-4.0,  42.0,  8.0],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    # 上叶主核体
    cubes.append({
        "name": "lobe_r_upper_core",
        "from": [-19.5, 32.0, -11.0],
        "to":   [-3.0,  38.0,  10.0],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    # 上叶前凸海绵状肺泡结节簇
    cubes.append({
        "name": "lobe_r_upper_nodule_f",
        "from": [-18.5, 34.0,  9.8],
        "to":   [-6.0,  39.0, 12.0],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    # 上叶背侧暗部隆起
    cubes.append({
        "name": "lobe_r_upper_nodule_b",
        "from": [-18.5, 33.5, -13.0],
        "to":   [-6.0,  38.5, -10.8],
        "group": "right_lobe",
        "material": "flesh_dark",
    })
    # 上叶外侧海绵突
    cubes.append({
        "name": "lobe_r_upper_lat",
        "from": [-21.2, 33.0, -8.0],
        "to":   [-19.2, 37.5,  7.0],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    # 水平裂浅凹槽 (分割上叶与中叶)
    cubes.append({
        "name": "lobe_r_fissure_horiz",
        "from": [-20.0, 28.5, -11.5],
        "to":   [-3.2,  32.0,  10.5],
        "group": "right_lobe",
        "material": "flesh_dark",
    })

    # ── 右肺中叶 (y: 19.0..29.5, 整个肺部横向最宽凸起) ──
    # 中叶主体
    cubes.append({
        "name": "lobe_r_mid_core",
        "from": [-21.5, 20.0, -12.5],
        "to":   [-3.5,  29.5,  11.5],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    # 中叶外侧弧拱 (外扩至 x = -22.5px)
    cubes.append({
        "name": "lobe_r_mid_bulge_lat",
        "from": [-22.5, 21.5, -9.0],
        "to":   [-21.2, 28.0,  8.0],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    # 中叶正面海绵隆起
    cubes.append({
        "name": "lobe_r_mid_bulge_f",
        "from": [-19.0, 21.0, 11.3],
        "to":   [-6.0,  28.5, 13.5],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    # 中叶背侧隆起
    cubes.append({
        "name": "lobe_r_mid_bulge_b",
        "from": [-19.0, 21.0, -14.2],
        "to":   [-6.0,  28.5, -12.3],
        "group": "right_lobe",
        "material": "flesh_dark",
    })
    # 斜裂凹槽 (分割中叶与下叶)
    cubes.append({
        "name": "lobe_r_fissure_oblique",
        "from": [-21.0, 18.0, -11.8],
        "to":   [-3.6,  20.2,  10.8],
        "group": "right_lobe",
        "material": "flesh_dark",
    })

    # ── 右肺下叶 (y: 6.0..19.0) ──
    # 下叶主核
    cubes.append({
        "name": "lobe_r_low_core",
        "from": [-20.0, 10.4, -12.0],
        "to":   [-3.0,  18.5,  11.0],
        "group": "right_lobe",
        "material": "flesh_main",
    })
    # 下叶外侧翼缘
    cubes.append({
        "name": "lobe_r_low_lat",
        "from": [-21.5, 11.0, -8.5],
        "to":   [-19.8, 17.5,  7.5],
        "group": "right_lobe",
        "material": "flesh_lit",
    })
    # 下叶底座弧面 (向内下方收缩)
    cubes.append({
        "name": "lobe_r_low_base",
        "from": [-18.0,  6.0, -10.0],
        "to":   [-3.5,  10.4,   9.0],
        "group": "right_lobe",
        "material": "flesh_dark",
    })

    return cubes


def part_03_left_lobe() -> List[dict]:
    """3. 海绵状左肺叶 (left_lobe, 前视图右侧 x > 0)。

    两叶结构（上叶、下叶），内侧保留近心切迹凹陷，下端带有狭长向外延伸的外展下叶，
    与右肺形成不对称。
    """
    cubes = []
    # ── 左肺上叶 (y: 27.0..40.4, 略微紧凑) ──
    # 左肺尖圆拱 (apex)
    cubes.append({
        "name": "lobe_l_upper_apex",
        "from": [ 3.5, 37.0, -8.5],
        "to":   [15.5, 40.4,  7.5],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    # 上叶主核 (内侧为心切迹凹槽留空 x < 3.0)
    cubes.append({
        "name": "lobe_l_upper_core",
        "from": [ 3.0, 30.0, -10.5],
        "to":   [18.5, 37.2,   9.5],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    # 上叶前凸肺泡结节
    cubes.append({
        "name": "lobe_l_upper_nodule_f",
        "from": [ 5.5, 32.0,  9.3],
        "to":   [17.5, 38.0, 11.5],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    # 上叶背凸
    cubes.append({
        "name": "lobe_l_upper_nodule_b",
        "from": [ 5.5, 31.5, -12.5],
        "to":   [17.5, 37.5, -10.3],
        "group": "left_lobe",
        "material": "flesh_dark",
    })
    # 上叶外侧弧拱
    cubes.append({
        "name": "lobe_l_upper_lat",
        "from": [18.2, 31.0, -7.5],
        "to":   [20.0, 36.5,  6.5],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    # 斜裂深凹槽
    cubes.append({
        "name": "lobe_l_fissure_oblique",
        "from": [ 3.2, 27.0, -11.0],
        "to":   [19.0, 30.2,  10.0],
        "group": "left_lobe",
        "material": "flesh_dark",
    })

    # ── 左肺下叶 (y: 6.0..28.5) ──
    # 下叶主核
    cubes.append({
        "name": "lobe_l_low_core",
        "from": [ 3.5, 12.0, -12.0],
        "to":   [19.5, 28.5,  11.0],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    # 下叶向外展延的狭长侧翼 (x 展至 21.0px)
    cubes.append({
        "name": "lobe_l_low_lat",
        "from": [19.2, 12.5, -8.0],
        "to":   [21.0, 22.0,  7.5],
        "group": "left_lobe",
        "material": "flesh_lit",
    })
    # 下叶正面饱满肉隆
    cubes.append({
        "name": "lobe_l_low_front",
        "from": [ 5.0, 11.6, 10.8],
        "to":   [18.0, 23.0, 12.8],
        "group": "left_lobe",
        "material": "flesh_main",
    })
    # 下叶背侧隆起
    cubes.append({
        "name": "lobe_l_low_back",
        "from": [ 5.0, 11.6, -13.5],
        "to":   [18.0, 23.0, -11.8],
        "group": "left_lobe",
        "material": "flesh_dark",
    })
    # 下叶底座
    cubes.append({
        "name": "lobe_l_low_base",
        "from": [ 3.2,  6.0, -10.0],
        "to":   [17.0, 12.2,   9.5],
        "group": "left_lobe",
        "material": "flesh_dark",
    })

    return cubes


def part_04_hilum_vessels() -> List[dict]:
    """4. 肺门脉管组织与深层筋膜 (hilum_vessels)。

    两肺叶之间的纵隔连接体、肺动静脉主干与半透明筋膜脉管束。
    """
    cubes = []
    # 纵隔核心脉管柱
    cubes.append({
        "name": "hilum_core_vascular",
        "from": [-3.2, 22.0, -4.5],
        "to":   [ 3.2, 34.0,  4.5],
        "group": "hilum_vessels",
        "material": "flesh_dark",
    })
    # 跨叶连接筋膜板 (半透明筋膜)
    cubes.append({
        "name": "hilum_fascia_mid",
        "from": [-3.0, 16.0, -4.0],
        "to":   [ 3.0, 22.2,  4.0],
        "group": "hilum_vessels",
        "material": "tendon_fascia",
    })
    # 内部真元流光核心柱
    cubes.append({
        "name": "hilum_qi_core",
        "from": [-1.5, 17.0, -2.0],
        "to":   [ 1.5, 33.0,  2.0],
        "group": "hilum_vessels",
        "material": "qi_glow",
    })
    # 右肺血管分支斜伸
    cubes.append({
        "name": "hilum_branch_r",
        "from": [-6.5, 24.0, -3.0],
        "to":   [-3.1, 28.0,  3.0],
        "group": "hilum_vessels",
        "material": "flesh_main",
    })
    # 左肺血管分支斜伸
    cubes.append({
        "name": "hilum_branch_l",
        "from": [ 3.1, 23.0, -2.5],
        "to":   [ 6.5, 27.0,  2.5],
        "group": "hilum_vessels",
        "material": "flesh_main",
    })

    return cubes


def part_05_output_port() -> List[dict]:
    """5. 底部汇流管腔与统一截面输出端口 (output_port)。

    汇流两叶底部的输出管腔，并严格以统一连接截面延伸至 +Z 侧端面：
    截面：宽 8 px × 高 6 px、居中 (x: -4.0..4.0)、底边离地 2 px (y: 2.0..8.0)，延伸至 z = 16.0，
    配有加固骨箍领圈与输出内腔开口，下方配有与地面贴合的暗血肉锚定底座 (y: 0.0..2.0)。
    """
    cubes = []
    # ── 1. 底部汇流漏斗体 ──
    cubes.append({
        "name": "port_funnel_upper",
        "from": [-5.5,  9.6, -5.0],
        "to":   [ 5.5, 16.2,  8.0],
        "group": "output_port",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "port_funnel_neck",
        "from": [-4.5,  4.0,  0.0],
        "to":   [ 4.5, 10.6, 10.0],
        "group": "output_port",
        "material": "flesh_dark",
    })

    # ── 2. 统一截面输出管体 (严格宽 8px x 高 6px, x in -4..4, y in 2..8, z in 9.8..15.2) ──
    cubes.append({
        "name": "port_sleeve_main",
        "from": [-4.0, 2.0,  9.8],
        "to":   [ 4.0, 8.0, 15.2],
        "group": "output_port",
        "material": "flesh_main",
    })

    # ── 3. 输出端口外围加固骨箍 (z: 14.8..16.0) ──
    # 顶骨梁 (y: 7.8..8.4, x: -4.5..4.5)
    cubes.append({
        "name": "port_bone_collar_top",
        "from": [-4.5, 7.8, 14.8],
        "to":   [ 4.5, 8.4, 16.0],
        "group": "output_port",
        "material": "bone_main",
    })
    # 底骨梁 (y: 1.6..2.2, x: -4.5..4.5)
    cubes.append({
        "name": "port_bone_collar_bot",
        "from": [-4.5, 1.6, 14.8],
        "to":   [ 4.5, 2.2, 16.0],
        "group": "output_port",
        "material": "bone_main",
    })
    # 左骨柱 (x: -4.4..-3.8, y: 2.2..7.8, z: 14.9..15.9)
    cubes.append({
        "name": "port_bone_collar_left",
        "from": [-4.4, 2.2, 14.9],
        "to":   [-3.8, 7.8, 15.9],
        "group": "output_port",
        "material": "bone_dark",
    })
    # 右骨柱 (x: 3.8..4.4, y: 2.2..7.8, z: 14.9..15.9)
    cubes.append({
        "name": "port_bone_collar_right",
        "from": [ 3.8, 2.2, 14.9],
        "to":   [ 4.4, 7.8, 15.9],
        "group": "output_port",
        "material": "bone_dark",
    })

    # ── 4. 输出端口内腔通道与真元光 (z: 15.2..16.0, x: -2.8..2.8, y: 3.2..6.8) ──
    cubes.append({
        "name": "port_lumen_glow",
        "from": [-2.8, 3.2, 15.2],
        "to":   [ 2.8, 6.8, 16.0],
        "group": "output_port",
        "material": "qi_glow",
    })

    # ── 5. 底面暗血肉锚定底座 (两层阶梯式底座沿 Y 轴堆叠，y: 0.0..0.8 与 y: 0.8..2.0) ──
    cubes.append({
        "name": "port_base_footing",
        "from": [-15.5, 0.0, -10.5],
        "to":   [ 15.5, 0.8,  11.8],
        "group": "output_port",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "port_base_pedestal",
        "from": [-13.0, 0.8,  -8.0],
        "to":   [ 13.0, 2.0,   9.8],
        "group": "output_port",
        "material": "flesh_dark",
    })

    return cubes


def all_cubes() -> List[dict]:
    """汇总肺器官全部 5 大部件立方体。"""
    return (
        part_01_trachea()
        + part_02_right_lobe()
        + part_03_left_lobe()
        + part_04_hilum_vessels()
        + part_05_output_port()
    )


# =============================================================================
# 门禁与无共面面核验
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标且重叠的共面冲突。"""
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
    """生成 64×64 RGBA 贴图，严格使用 meridian_factory.md 的有机型配色。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
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
                
                # 有机海绵状肺泡与气管纹理增强
                if mat_name in ("flesh_main", "flesh_lit"):
                    if (x * 3 + y * 5) % 7 == 0:
                        r = int(np.clip(r - 8, 0, 255))
                        g = int(np.clip(g - 4, 0, 255))
                    elif (x * 2 - y * 3) % 9 == 0:
                        r = int(np.clip(r + 8, 0, 255))
                        g = int(np.clip(g + 6, 0, 255))
                elif mat_name in ("bone_main", "bone_dark"):
                    if (x + y * 2) % 6 == 0:
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
    for g_name in ["trachea", "right_lobe", "left_lobe", "hilum_vessels", "output_port"]:
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
    bg_color = (119, 119, 119)  # 严格对齐参考图的中性灰底色

    # 1. 渲染四视角 (正视、侧视、3/4 等轴、俯视)
    # 正视 (看 +Z 输出端口与正面肺叶)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    # 侧视 (看整个 32px 深度的纵深与肺叶侧缘)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # 3/4 等轴透视
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    # 俯视 (看顶部气管骨环与双肺叶顶面开展)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格)
    canvas_w = 1040
    canvas_h = 1040
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("FRONT (+Z Output Port & Lobes)", im_front, 20, 20),
        ("SIDE (Depth 32px Profile)",       im_side, 540, 20),
        ("3/4 ISOMETRIC (Organic Structure)", im_iso, 20, 540),
        ("TOP (Trachea & Apices)",          im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 250, py + 26], fill=(24, 25, 28))
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
        "from": [-3.5, 46.5, -4.5],
        "to":   [ 3.5, 48.0,  2.5],  # 与 trachea_ring_1 完全重叠
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
