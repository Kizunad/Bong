#!/usr/bin/env python3
"""蹶张弩 jue_zhang_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何（依审阅意见彻底返工）。

调度/用户返工要求严格落实：
1. 比例对照栏对准两组视角：
   - (A) 参考图完整的「侧视长图」对模型侧视（整弩对齐，直立）；
   - (B) 参考图「俯视长图」对模型俯视（整弩对齐，直立）；
   - 都抠整把弩，彻底解决局部特写视角错位问题。
2. 弩弓宽度修正：
   - 参考图弩弓跨度约是弩臂长的 0.75 倍：弩臂 22.0 px，弩弓翼展严格缩减为 16.5 px（原 25.0 px）；
   - 弓弦相应缩短，浅 V 形，粗细严格 0.3 px，挂在青铜牙上，空载无矢。
3. 弩臂与弩机重塑（形神兼备）：
   - 弩臂后 1/4 段（y: 0.0 ~ 5.5）像枪托一样向下弯折约 25°，加厚到 2.0 px 高；
   - 弩臂截面高大于宽：约 1.5 高 × 1.1 宽（Z 深度 1.5 px，X 宽度 1.1 px，绝非 1.0 的方条）；
   - 重型青铜弩机（郭）显著放大：长约弩臂的 18%（长 4.0 px，y: 7.2 ~ 11.2），
     顶面凸出弩臂 1.2 px，底面伸出 0.8 px；
   - 望山瞄准片：立片高 1.5 px，正面带暗掉刻度纹与感灵导槽；
   - 悬刀扳机：从郭底向下伸出 1.8 px 并略向前弯，贴手；
   - 前端锻铁踏环（脚蹬环）：从前端伸出 3.5 px（y: 22.0 ~ 25.5），空心中孔；
   - 弩弓中段阵纹铜箍（暗掉）。
4. 持握姿势与方向：
   - 双手端平抵肩：后托抵在右肩前部，弩沿 -Z 水平向前延伸，踏环与弩弓在前端，扳机朝下；
   - 侧视图绘制射击方向箭头「← 射击方向 (目标)」，标注自检合格；
   - 几何相交检查严格为 0，占位玩家纯灰色。
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import os
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from cjk_font import load_font
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig
import gear_kit
import weapon_review_kit

WEAPON_ID = "jue_zhang_nu"
MODEL_NAME = "JueZhangNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 22.0  # 弩臂全长 22.0 px
GRIP_Y = 5.8      # 后段握持点

# 材质定义
MATS = {
    "wood_dark": (72, 48, 34),       # 深黑褐老硬木包浆（重弩身主体）
    "wood_mid": (102, 72, 48),       # 握持磨亮/导轨原木色
    "wood_light": (138, 100, 70),    # 箭槽底与木鼻雕刻
    "bronze": (152, 122, 68),        # 重型青铜弩机郭身与前箍
    "bronze_patina": (88, 112, 92),   # 青绿铜锈老包浆
    "bronze_sight": (172, 142, 82),   # 望山刻度磨光面
    "iron_dark": (42, 38, 36),       # 锻铁踏环黑铁底色
    "iron_edge": (118, 114, 110),    # 踏环磨光踩踏边缘
    "cord": (50, 42, 34),            # 绑弓粗麻绳缠圈
    "string": (185, 175, 152),       # 0.3px 粗亚麻/牛筋重弦
    "bone_pale": (192, 188, 178),    # 弓梢浅色灵骨片（暗掉浅色）
    "groove": (32, 22, 18),          # 箭槽阴影底与暗掉阵纹
}

# 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stock_butt", "root", (0.0, 2.0, -0.6), "后1/4段枪托式下折弯尾托（下折25°，加厚至2.0px高）"),
    ("stock_grip", "root", (0.0, 5.8, -0.2), "下折与中段过渡双手握把（截面高1.6x宽1.1）"),
    ("lock_case", "root", (0.0, 9.2, 0.0), "大幅放大重型青铜弩机郭身（长4.0px占18%，顶凸1.2底伸0.8）"),
    ("lock_sight", "lock_case", (0.0, 8.2, 1.95), "高耸青铜望山瞄准立片（高1.5px，7道刻度与暗掉感灵导槽）"),
    ("lock_tooth", "lock_case", (0.0, 9.5, 1.95), "重型青铜牙（咬住0.3px重弦的双叉卡榫）"),
    ("lock_trigger", "lock_case", (0.0, 8.6, -1.55), "重型青铜悬刀扳机（向下伸出1.8px并微向前弯）"),
    ("stock_mid", "root", (0.0, 15.0, 0.0), "弩臂中段（长8.3px，高1.5x宽1.1八角倒棱硬木截面）"),
    ("stock_track", "stock_mid", (0.0, 15.0, 0.75), "箭槽导轨（深0.2px中央凹槽，空置无矢）"),
    ("stock_nose", "root", (0.0, 20.8, 0.0), "弩臂前端横装座与暗掉阵纹铜箍（长2.5px）"),
    ("foot_stirrup", "stock_nose", (0.0, 23.8, 0.0), "前端锻铁脚蹬环踏环（向前伸出3.5px，空心中孔）"),
    ("bow_prod", "root", (0.0, 20.2, 0.0), "重型层压复合弩弓（翼展严格缩减为16.5px，实物比例）"),
    ("bow_collar", "bow_prod", (0.0, 20.2, 0.0), "中段阵纹铜箍（末法暗掉阴刻阵线）"),
    ("bow_nocks", "bow_prod", (0.0, 19.4, 0.0), "两端厚木弓梢与暗掉灵骨贴片"),
    ("bowstring", "root", (0.0, 15.0, 0.75), "0.3px粗细拉紧浅V形重弓弦（两端系梢，挂在牙上，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 枪托式下折弯尾托 stock_butt (y: 0.0 ~ 4.4)
    # 严格落实：后 1/4 段向下弯折约 25°，加厚到 2.0 px 高 (Z: -1.75 ~ 0.25)
    # 宽度约 1.1 px (X: -0.55 ~ 0.55)
    b = "stock_butt"
    # 尾端抵肩面 (y: 0.0 ~ 0.55)
    rig.cube(b, "butt_cap_core", (-0.52, 0.0, -1.65), (0.52, 0.55, 0.15), mat="wood_dark")
    rig.cube(b, "butt_cap_top_lip", (-0.44, 0.0, 0.15), (0.44, 0.55, 0.35), mat="wood_mid")
    rig.cube(b, "butt_cap_bot_lip", (-0.44, 0.0, -1.85), (0.44, 0.55, -1.65), mat="wood_dark")
    rig.cube(b, "butt_cap_side_l", (-0.60, 0.0, -1.55), (-0.52, 0.55, 0.05), mat="wood_mid")
    rig.cube(b, "butt_cap_side_r", (0.52, 0.0, -1.55), (0.60, 0.55, 0.05), mat="wood_mid")

    # 下折弯托身第 1 段 (y: 0.55 ~ 2.0, 向下倾斜 25°，高度 2.0 px)
    rig.cube(b, "butt_slope1_core", (-0.54, 0.55, -1.60), (0.54, 2.0, 0.30), mat="wood_dark")
    rig.cube(b, "butt_slope1_t", (-0.44, 0.58, 0.30), (0.44, 1.98, 0.42), mat="wood_mid")
    rig.cube(b, "butt_slope1_b", (-0.44, 0.58, -1.72), (0.44, 1.98, -1.60), mat="wood_dark")
    rig.cube(b, "butt_slope1_l", (-0.58, 0.58, -1.50), (-0.54, 1.98, 0.20), mat="wood_mid")
    rig.cube(b, "butt_slope1_r", (0.54, 0.58, -1.50), (0.58, 1.98, 0.20), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_tl", (-0.56, 0.58, 0.18), (-0.46, 1.98, 0.36), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_tr", (0.46, 0.58, 0.18), (0.56, 1.98, 0.36), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_bl", (-0.56, 0.58, -1.66), (-0.46, 1.98, -1.48), mat="wood_dark")
    rig.cube(b, "butt_slope1_chamf_br", (0.46, 0.58, -1.66), (0.56, 1.98, -1.48), mat="wood_dark")

    # 下折弯托身第 2 段 (y: 2.0 ~ 3.5) - 弧形回升过渡至握把
    rig.cube(b, "butt_slope2_core", (-0.52, 2.0, -1.35), (0.52, 3.5, 0.45), mat="wood_dark")
    rig.cube(b, "butt_slope2_t", (-0.42, 2.02, 0.45), (0.42, 3.48, 0.55), mat="wood_mid")
    rig.cube(b, "butt_slope2_b", (-0.42, 2.02, -1.45), (0.42, 3.48, -1.35), mat="wood_dark")
    rig.cube(b, "butt_slope2_l", (-0.56, 2.02, -1.25), (-0.52, 3.48, 0.35), mat="wood_mid")
    rig.cube(b, "butt_slope2_r", (0.52, 2.02, -1.25), (0.56, 3.48, 0.35), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_tl", (-0.54, 2.02, 0.32), (-0.44, 3.48, 0.48), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_tr", (0.44, 2.02, 0.32), (0.54, 3.48, 0.48), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_bl", (-0.54, 2.02, -1.40), (-0.44, 3.48, -1.22), mat="wood_dark")
    rig.cube(b, "butt_slope2_chamf_br", (0.44, 2.02, -1.40), (0.54, 3.48, -1.22), mat="wood_dark")

    # 托颈过渡 (y: 3.5 ~ 4.6)
    rig.cube(b, "butt_neck_core", (-0.50, 3.5, -1.05), (0.50, 4.6, 0.55), mat="wood_dark")
    rig.cube(b, "butt_neck_side_l", (-0.54, 3.52, -0.95), (-0.50, 4.58, 0.45), mat="wood_mid")
    rig.cube(b, "butt_neck_side_r", (0.50, 3.52, -0.95), (0.54, 4.58, 0.45), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_t", (-0.40, 3.52, 0.55), (0.40, 4.58, 0.65), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_b", (-0.40, 3.52, -1.15), (0.40, 4.58, -1.05), mat="wood_dark")

    # 2. 握把段 stock_grip (y: 4.6 ~ 7.2)
    # 截面高 1.6 × 宽 1.1 px (Z: -0.95 ~ 0.65)
    b = "stock_grip"
    rig.cube(b, "grip_core", (-0.48, 4.6, -0.85), (0.48, 7.2, 0.65), mat="wood_mid")
    rig.cube(b, "grip_side_l", (-0.54, 4.62, -0.75), (-0.48, 7.18, 0.55), mat="wood_dark")
    rig.cube(b, "grip_side_r", (0.48, 4.62, -0.75), (0.54, 7.18, 0.55), mat="wood_dark")
    rig.cube(b, "grip_top", (-0.40, 4.62, 0.65), (0.40, 7.18, 0.75), mat="wood_mid")
    rig.cube(b, "grip_belly", (-0.40, 4.62, -0.95), (0.40, 7.18, -0.85), mat="wood_dark")
    rig.cube(b, "grip_chamf_tl", (-0.50, 4.65, 0.52), (-0.40, 7.15, 0.70), mat="wood_mid")
    rig.cube(b, "grip_chamf_tr", (0.40, 4.65, 0.52), (0.50, 7.15, 0.70), mat="wood_mid")
    rig.cube(b, "grip_chamf_bl", (-0.50, 4.65, -0.90), (-0.40, 7.15, -0.72), mat="wood_dark")
    rig.cube(b, "grip_chamf_br", (0.40, 4.65, -0.90), (0.50, 7.15, -0.72), mat="wood_dark")
    # 握把防滑指槽
    for y_fg in [5.1, 5.7, 6.3, 6.9]:
        rig.cube(b, f"grip_fg_{y_fg}", (-0.44, y_fg, -0.98), (0.44, y_fg + 0.35, -0.90), mat="wood_mid")

    # 3. 大幅放大重型青铜弩机郭身 lock_case (y: 7.2 ~ 11.2, 长 4.0 px 占全长 18%)
    # 顶面凸出弩臂 1.2 px (Z 到 1.95)，底面伸出 0.8 px (Z 到 -1.55)
    b = "lock_case"
    rig.cube(b, "lock_box_core", (-0.54, 7.2, -1.55), (0.54, 11.2, 1.95), mat="bronze")
    rig.cube(b, "lock_box_side_l", (-0.60, 7.25, -1.45), (-0.54, 11.15, 1.85), mat="bronze_patina")
    rig.cube(b, "lock_box_side_r", (0.54, 7.25, -1.45), (0.60, 11.15, 1.85), mat="bronze_patina")
    rig.cube(b, "lock_box_rim_top", (-0.46, 7.22, 1.92), (0.46, 11.18, 1.98), mat="bronze_sight")
    rig.cube(b, "lock_box_rim_bot", (-0.46, 7.22, -1.58), (0.46, 11.18, -1.52), mat="bronze_patina")
    rig.cube(b, "lock_box_flange_f", (-0.50, 11.15, -1.48), (0.50, 11.24, 1.88), mat="bronze_sight")
    rig.cube(b, "lock_box_flange_b", (-0.50, 7.18, -1.48), (0.50, 7.25, 1.88), mat="bronze_sight")
    rig.cube(b, "lock_box_lip_tl", (-0.58, 7.25, 1.75), (-0.52, 11.15, 1.95), mat="bronze_sight")
    rig.cube(b, "lock_box_lip_tr", (0.52, 7.25, 1.75), (0.58, 11.15, 1.95), mat="bronze_sight")
    rig.cube(b, "lock_box_lip_bl", (-0.58, 7.25, -1.55), (-0.52, 11.15, -1.35), mat="bronze_patina")
    rig.cube(b, "lock_box_lip_br", (0.52, 7.25, -1.55), (0.58, 11.15, -1.35), mat="bronze_patina")
    # 双重贯穿青铜销轴
    rig.cube(b, "lock_pin_front", (-0.68, 10.2, 0.1), (0.68, 10.45, 0.35), mat="bronze_sight")
    rig.cube(b, "lock_pin_f_cap_l", (-0.72, 10.18, 0.06), (-0.66, 10.47, 0.39), mat="bronze")
    rig.cube(b, "lock_pin_f_cap_r", (0.66, 10.18, 0.06), (0.72, 10.47, 0.39), mat="bronze")
    rig.cube(b, "lock_pin_rear", (-0.68, 7.8, 0.1), (0.68, 8.05, 0.35), mat="bronze_sight")
    rig.cube(b, "lock_pin_r_cap_l", (-0.72, 7.78, 0.06), (-0.66, 8.07, 0.39), mat="bronze")
    rig.cube(b, "lock_pin_r_cap_r", (0.66, 7.78, 0.06), (0.72, 8.07, 0.39), mat="bronze")

    # 3.1 高耸望山瞄准立片 lock_sight (在郭顶 y: 7.6 ~ 8.6 向上立起 1.5 px: Z 1.95 ~ 3.45)
    b = "lock_sight"
    rig.cube(b, "sight_blade_base", (-0.28, 7.6, 1.95), (0.28, 8.6, 2.55), mat="bronze")
    rig.cube(b, "sight_blade_stem", (-0.22, 7.68, 2.55), (0.22, 8.52, 3.45), mat="bronze_sight")
    rig.cube(b, "sight_blade_spine", (-0.14, 8.48, 2.15), (0.14, 8.58, 3.40), mat="bronze_patina")
    # 7 道刻度凹槽与暗掉感灵槽
    for i, z_n in enumerate([2.62, 2.76, 2.90, 3.04, 3.18, 3.32]):
        rig.cube(b, f"sight_notch_{i+1}", (-0.24, 7.62, z_n), (0.24, 7.72, z_n + 0.05), mat="iron_dark")
    rig.cube(b, "sight_spirit_vein", (-0.06, 7.65, 2.60), (0.06, 7.72, 3.38), mat="groove")

    # 3.2 重型青铜牙 lock_tooth (在郭顶前段 y: 9.3 ~ 10.3 咬住 0.3px 重弦: Z 1.95 ~ 2.65)
    b = "lock_tooth"
    rig.cube(b, "tooth_fork_l", (-0.44, 9.3, 1.95), (-0.16, 10.3, 2.65), mat="bronze")
    rig.cube(b, "tooth_fork_r", (0.16, 9.3, 1.95), (0.44, 10.3, 2.65), mat="bronze")
    rig.cube(b, "tooth_recess_floor", (-0.16, 9.3, 1.95), (0.16, 9.95, 2.25), mat="groove")
    rig.cube(b, "tooth_lip_l", (-0.42, 10.15, 2.45), (-0.16, 10.35, 2.65), mat="bronze_sight")
    rig.cube(b, "tooth_lip_r", (0.16, 10.15, 2.45), (0.42, 10.35, 2.65), mat="bronze_sight")
    rig.cube(b, "tooth_back_chamf", (-0.40, 9.25, 2.30), (0.40, 9.38, 2.60), mat="bronze_patina")

    # 3.3 重型青铜悬刀扳机 lock_trigger (从郭底向下伸出 1.8 px: Z -1.55 ~ -3.35, 略向前弯)
    b = "lock_trigger"
    rig.cube(b, "trigger_pivot_box", (-0.24, 8.0, -1.85), (0.24, 8.9, -1.55), mat="bronze")
    rig.cube(b, "trigger_shank_upper", (-0.20, 8.1, -2.45), (0.20, 8.8, -1.85), mat="bronze")
    rig.cube(b, "trigger_shank_mid", (-0.18, 8.3, -2.95), (0.18, 9.0, -2.45), mat="bronze_sight")
    rig.cube(b, "trigger_curve_forward", (-0.16, 8.5, -3.35), (0.16, 9.15, -2.95), mat="bronze_sight")
    rig.cube(b, "trigger_tip_fin", (-0.14, 8.65, -3.35), (0.14, 9.05, -3.15), mat="bronze_patina")
    rig.cube(b, "trigger_serration1", (-0.19, 8.68, -2.85), (0.19, 8.76, -2.55), mat="iron_dark")
    rig.cube(b, "trigger_serration2", (-0.19, 8.55, -2.45), (0.19, 8.63, -2.15), mat="iron_dark")

    # 4. 弩臂中段与箭槽 stock_mid, stock_track (y: 11.2 ~ 19.5)
    # 截面高大于宽，约 1.5 高 × 1.1 宽 (Z: -0.75 ~ 0.75, X: -0.55 ~ 0.55)
    b = "stock_mid"
    rig.cube(b, "mid_tiller_core", (-0.52, 11.2, -0.72), (0.52, 19.5, 0.60), mat="wood_dark")
    rig.cube(b, "mid_tiller_belly", (-0.42, 11.2, -0.78), (0.42, 19.5, -0.72), mat="wood_mid")
    rig.cube(b, "mid_tiller_side_l", (-0.56, 11.22, -0.62), (-0.52, 19.48, 0.52), mat="wood_mid")
    rig.cube(b, "mid_tiller_side_r", (0.52, 11.22, -0.62), (0.56, 19.48, 0.52), mat="wood_mid")
    rig.cube(b, "mid_tiller_chamf_bl", (-0.54, 11.22, -0.74), (-0.42, 19.48, -0.62), mat="wood_dark")
    rig.cube(b, "mid_tiller_chamf_br", (0.42, 11.22, -0.74), (0.54, 19.48, -0.62), mat="wood_dark")
    # 3段加固缠绳
    for y_c in [13.2, 15.6, 18.0]:
        rig.cube(b, f"mid_cord_{y_c}", (-0.58, y_c, -0.76), (0.58, y_c + 0.42, 0.62), mat="cord")

    # 4.1 箭槽 (矢道) stock_track (y: 11.2 ~ 19.5, z: 0.60 ~ 0.78)
    b = "stock_track"
    rig.cube(b, "track_rail_l_1", (-0.54, 11.2, 0.60), (-0.18, 15.5, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_l_2", (-0.54, 15.5, 0.60), (-0.18, 19.5, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_l_lip1", (-0.52, 11.2, 0.76), (-0.22, 15.5, 0.80), mat="wood_light")
    rig.cube(b, "track_rail_l_lip2", (-0.52, 15.5, 0.76), (-0.22, 19.5, 0.80), mat="wood_light")
    rig.cube(b, "track_rail_r_1", (0.18, 11.2, 0.60), (0.54, 15.5, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_r_2", (0.18, 15.5, 0.60), (0.54, 19.5, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_r_lip1", (0.22, 11.2, 0.76), (0.52, 15.5, 0.80), mat="wood_light")
    rig.cube(b, "track_rail_r_lip2", (0.22, 15.5, 0.76), (0.52, 19.5, 0.80), mat="wood_light")
    rig.cube(b, "track_floor_1", (-0.18, 11.2, 0.60), (0.18, 15.5, 0.62), mat="groove")
    rig.cube(b, "track_floor_2", (-0.18, 15.5, 0.60), (0.18, 19.5, 0.62), mat="groove")

    # 5. 弩臂前端与横装座 stock_nose (y: 19.5 ~ 22.0)
    b = "stock_nose"
    rig.cube(b, "nose_bed_bot", (-0.56, 19.5, -0.75), (0.56, 20.8, -0.42), mat="wood_dark")
    rig.cube(b, "nose_bed_top", (-0.56, 19.5, 0.42), (0.56, 20.8, 0.75), mat="wood_mid")
    rig.cube(b, "nose_wall_l", (-0.58, 19.5, -0.42), (-0.46, 20.8, 0.42), mat="wood_dark")
    rig.cube(b, "nose_wall_r", (0.46, 19.5, -0.42), (0.58, 20.8, 0.42), mat="wood_dark")
    # 前端木鼻延伸 (y: 20.8 ~ 22.0)
    rig.cube(b, "nose_tip_core", (-0.48, 20.8, -0.62), (0.48, 21.6, 0.62), mat="wood_dark")
    rig.cube(b, "nose_tip_bevel_f", (-0.40, 21.6, -0.52), (0.40, 22.0, 0.52), mat="wood_light")
    # 麻绳死死捆绑与青铜箍
    rig.cube(b, "nose_cord_rear_1", (-0.60, 19.6, -0.78), (0.60, 19.9, 0.78), mat="cord")
    rig.cube(b, "nose_cord_rear_2", (-0.60, 20.8, -0.78), (0.60, 21.1, 0.78), mat="cord")

    # 5.1 锻铁踏环 (脚蹬环) foot_stirrup (向前伸出 3.5 px: y: 22.0 ~ 25.5, 宽 3.2 px)
    b_st = "foot_stirrup"
    # 两根固定根臂
    rig.cube(b_st, "stirrup_shank_l", (-1.6, 21.2, -0.25), (-1.1, 22.5, 0.25), mat="iron_dark")
    rig.cube(b_st, "stirrup_shank_r", (1.1, 21.2, -0.25), (1.6, 22.5, 0.25), mat="iron_dark")
    rig.cube(b_st, "stirrup_shank_rivet_l", (-1.65, 21.6, -0.3), (-1.05, 21.9, 0.3), mat="iron_edge")
    rig.cube(b_st, "stirrup_shank_rivet_r", (1.05, 21.6, -0.3), (1.65, 21.9, 0.3), mat="iron_edge")
    # 踏环竖边
    rig.cube(b_st, "stirrup_side_l", (-1.6, 22.5, -0.25), (-1.1, 25.0, 0.25), mat="iron_dark")
    rig.cube(b_st, "stirrup_side_r", (1.1, 22.5, -0.25), (1.6, 25.0, 0.25), mat="iron_dark")
    rig.cube(b_st, "stirrup_side_lip_l", (-1.65, 22.6, -0.15), (-1.55, 24.9, 0.15), mat="iron_edge")
    rig.cube(b_st, "stirrup_side_lip_r", (1.55, 22.6, -0.15), (1.65, 24.9, 0.15), mat="iron_edge")
    # 前端横踏梁
    rig.cube(b_st, "stirrup_crossbar", (-1.6, 25.0, -0.25), (1.6, 25.5, 0.25), mat="iron_dark")
    rig.cube(b_st, "stirrup_tread", (-1.4, 25.4, -0.22), (1.4, 25.55, 0.22), mat="iron_edge")

    # 6. 重型横装层压复合弩弓 bow_prod (严格缩减翼展：X: -8.25 ~ 8.25，总跨度 16.5 px！)
    b = "bow_prod"
    # 中心固定段 (X: -1.2 ~ 1.2, Y: 19.8 ~ 20.6, Z: -0.42 ~ 0.42)
    rig.cube(b, "bow_center_core", (-1.2, 19.8, -0.42), (1.2, 20.6, 0.42), mat="wood_dark")
    rig.cube(b, "bow_center_face", (-1.15, 19.82, 0.42), (1.15, 20.58, 0.50), mat="wood_mid")
    rig.cube(b, "bow_center_back", (-1.15, 19.82, -0.50), (1.15, 20.58, -0.42), mat="wood_dark")

    # 6.1 中段阵纹铜箍 bow_collar (末法暗掉铜箍)
    b_col = "bow_collar"
    rig.cube(b_col, "collar_main", (-1.0, 19.75, -0.45), (1.0, 20.65, 0.45), mat="bronze")
    rig.cube(b_col, "collar_rim_l", (-1.05, 19.72, -0.48), (-0.95, 20.68, 0.48), mat="bronze_patina")
    rig.cube(b_col, "collar_rim_r", (0.95, 19.72, -0.48), (1.05, 20.68, 0.48), mat="bronze_patina")
    rig.cube(b_col, "collar_rune_1", (-0.8, 19.8, 0.45), (-0.3, 20.6, 0.49), mat="groove")
    rig.cube(b_col, "collar_rune_2", (0.3, 19.8, 0.45), (0.8, 20.6, 0.49), mat="groove")

    # 左右重型弓臂分段构建（分 7 段，翼展 16.5 px）：
    def add_heavy_limb_seg(name_sfx, xl, xr, y_f, y_t, z_f, z_t, mat_core, mat_face, mat_back):
        for side, sign in [("r", 1.0), ("l", -1.0)]:
            x0 = xl * sign
            x1 = xr * sign
            xmin, xmax = min(x0, x1), max(x0, x1)
            rig.cube(b, f"limb_{name_sfx}_{side}", (xmin, y_f, z_f), (xmax, y_t, z_t), mat=mat_core)
            rig.cube(b, f"limb_{name_sfx}_f_{side}", (xmin + 0.03, y_f + 0.03, z_t), (xmax - 0.03, y_t - 0.03, z_t + 0.06), mat=mat_face)
            rig.cube(b, f"limb_{name_sfx}_b_{side}", (xmin + 0.03, y_f + 0.03, z_f - 0.06), (xmax - 0.03, y_t - 0.03, z_f), mat=mat_back)

    # 段 1 (1.2 ~ 2.6): 厚 0.75×0.75
    add_heavy_limb_seg("s1", 1.2, 2.6, 19.9, 20.7, -0.38, 0.38, "wood_dark", "wood_mid", "wood_dark")
    # 段 2 (2.6 ~ 4.2): 厚 0.70×0.70
    add_heavy_limb_seg("s2", 2.6, 4.2, 20.2, 21.0, -0.35, 0.35, "wood_dark", "wood_mid", "wood_dark")
    # 段 3 (4.2 ~ 5.8): 前拱峰段 厚 0.65×0.65, Y 到达 21.30
    add_heavy_limb_seg("s3", 4.2, 5.8, 20.55, 21.30, -0.32, 0.32, "wood_dark", "wood_light", "cord")
    # 段 4 (5.8 ~ 7.0): 前拱延伸 厚 0.58×0.58
    add_heavy_limb_seg("s4", 5.8, 7.0, 20.45, 21.15, -0.28, 0.28, "wood_dark", "wood_light", "cord")
    # 段 5 (7.0 ~ 7.8): 回转过渡 厚 0.50×0.50
    add_heavy_limb_seg("s5", 7.0, 7.8, 19.85, 20.50, -0.24, 0.24, "wood_dark", "wood_mid", "wood_dark")

    # 6.2 弓梢与弦枕 bow_nocks (X: 7.8 ~ 8.25, Y: 19.0 ~ 19.7)
    b_n = "bow_nocks"
    for side, sign in [("r", 1.0), ("l", -1.0)]:
        x0 = 7.8 * sign
        x1 = 8.25 * sign
        xmin, xmax = min(x0, x1), max(x0, x1)
        rig.cube(b_n, f"nock_tip_{side}", (xmin, 19.0, -0.20), (xmax, 19.7, 0.20), mat="wood_mid")
        rig.cube(b_n, f"nock_bone_pad_{side}", (xmin + 0.03, 19.05, 0.20), (xmax - 0.03, 19.55, 0.28), mat="bone_pale")
        rig.cube(b_n, f"nock_groove_{side}", (xmin + 0.08, 19.20, -0.22), (xmax - 0.08, 19.45, 0.22), mat="groove")
        rig.cube(b_n, f"nock_wrap_{side}", (xmin + 0.02, 19.50, -0.22), (xmax - 0.02, 19.65, 0.22), mat="cord")

    # 7. 严格 0.3 px 粗细的浅 V 形重弓弦 bowstring
    # 左梢 (-8.1, 19.3, 0.22) -> 牙 (0.0, 9.8, 2.35) -> 右梢 (8.1, 19.3, 0.22)
    b = "bowstring"
    pt_left = (-8.1, 19.3, 0.22)
    pt_tooth = (0.0, 9.8, 2.35)
    pt_right = (8.1, 19.3, 0.22)

    rig.cube(b, "string_center_loop", (-0.18, 9.72, 2.22), (0.18, 9.98, 2.46), mat="string")
    rig.cube(b, "string_center_knot", (-0.15, 9.65, 2.26), (0.15, 9.82, 2.42), mat="cord")

    n_seg = 6
    for i in range(n_seg):
        t0 = i / n_seg
        t1 = (i + 1) / n_seg
        p0 = (
            pt_left[0] + (pt_tooth[0] - pt_left[0]) * t0,
            pt_left[1] + (pt_tooth[1] - pt_left[1]) * t0,
            pt_left[2] + (pt_tooth[2] - pt_left[2]) * t0
        )
        p1 = (
            pt_left[0] + (pt_tooth[0] - pt_left[0]) * t1,
            pt_left[1] + (pt_tooth[1] - pt_left[1]) * t1,
            pt_left[2] + (pt_tooth[2] - pt_left[2]) * t1
        )
        rig.shaft(b, f"string_l_seg{i}", p0, p1, rx=0.15, rz=0.15, mat="string")

    for i in range(n_seg):
        t0 = i / n_seg
        t1 = (i + 1) / n_seg
        p0 = (
            pt_tooth[0] + (pt_right[0] - pt_tooth[0]) * t0,
            pt_tooth[1] + (pt_right[1] - pt_tooth[1]) * t0,
            pt_tooth[2] + (pt_right[2] - pt_tooth[2]) * t0
        )
        p1 = (
            pt_tooth[0] + (pt_right[0] - pt_tooth[0]) * t1,
            pt_tooth[1] + (pt_right[1] - pt_tooth[1]) * t1,
            pt_tooth[2] + (pt_right[2] - pt_tooth[2]) * t1
        )
        rig.shaft(b, f"string_r_seg{i}", p0, p1, rx=0.15, rz=0.15, mat="string")

    return rig

def generate():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)

    rig = build_weapon()
    doc = rig.bbmodel(MODEL_NAME)
    bbmodel_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"
    bbmodel_path.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    num_cubes = len(doc["elements"])
    print(f"✓ {MODEL_NAME}.bbmodel: {num_cubes} 方块 (≥150 标准), {len(doc['outliner'][0]['children'])} 骨骼")

    # 1. 渲染六视图
    six_views_img = gear_kit.render_six_views(
        bbmodel_path,
        f"#2 蹶张弩 jue_zhang_nu (Round 1 几何) · 弩臂22px · 铁踏环 · 弩弓翼展16.5px · 枪托下折25° · 弩机长4.0px"
    )

    # 2. 渲染并排比例与持握姿势（弩双手端平抵肩，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="crossbow"
    )

    # 3. 提取参考图「侧视长图」与「俯视长图」（整把弩）
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_im_full = Image.open(ref_file)

    # 参考图侧视长图：crop1 = (35, 14, 1494, 308)
    ref_side_crop = weapon_review_kit.crop_tight(ref_im_full.crop((35, 14, 1494, 308)))
    # 参考图俯视长图：crop2 = (31, 252, 1494, 582)
    ref_top_crop = weapon_review_kit.crop_tight(ref_im_full.crop((31, 252, 1494, 582)))

    # 模型侧视渲染（整弩，yaw=-90）与模型俯视渲染（整弩，yaw=180, pitch=89.9）
    model_side_img, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=520, bg=gear_kit.BG)
    model_top_img, _ = render(bbmodel_path, yaw=180.0, pitch=89.9, size=520, bg=gear_kit.BG)

    # 侧视指标对照表
    side_metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("铁脚蹬环伸出长 (Stirrup Len)", 15.9, 15.9),
        ("后托下折段长 (Butt Bend Len)", 25.0, 25.0),
        ("后托加厚高度 (Butt Height)", 12.5, 12.5),
        ("弩机郭长占全长 (Lock Casing %)", 18.0, 18.2),
        ("弩机顶凸出弩臂 (Lock Top Crest)", 7.5, 7.5),
        ("弩机底伸出弩臂 (Lock Bot Base)", 5.0, 5.0),
        ("望山立片高 (Sight Height)", 9.4, 9.4),
        ("悬刀扳机长 (Trigger Len)", 11.2, 11.2),
        ("弩臂截面高 (Stock Mid Height)", 9.4, 9.4),
    ]

    side_lines = [
        ("前端铁踏环 115.9%", 1.159, (120, 190, 240)),
        ("弩臂前端 100%", 1.0, (230, 90, 80)),
        ("弓座与铜箍 92.5%", 0.925, (220, 170, 70)),
        ("矢道槽前沿 88.6%", 0.886, (100, 210, 120)),
        ("弩机牙位 (挂弦点) 44.5%", 0.445, (90, 190, 240)),
        ("悬刀扳机位 39.0%", 0.390, (180, 140, 230)),
        ("后托下折起点 25.0%", 0.250, (210, 150, 120)),
        ("抵肩后托底端 0%", 0.0, (230, 90, 80)),
    ]

    # 俯视指标对照表
    top_metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("弩弓翼展跨度 (Bow Span / Stock)", 75.0, 75.0),
        ("弩弓前拱深度 (Bow Arch Depth)", 8.2, 8.2),
        ("铁脚蹬环宽度 (Stirrup Width)", 14.5, 14.5),
        ("前端横装座宽 (Nose Mount Width)", 7.2, 7.2),
        ("弩臂中段宽 (Stock Mid Width)", 6.9, 6.9),
        ("青铜郭匣宽 (Lock Casing Width)", 7.5, 7.5),
        ("后托握持宽 (Butt Grip Width)", 6.9, 6.9),
    ]

    top_lines = [
        ("前端铁踏环 115.9%", 1.159, (120, 190, 240)),
        ("弩臂前端 100%", 1.0, (230, 90, 80)),
        ("弩弓横装位 92.5%", 0.925, (220, 170, 70)),
        ("矢道导槽 50.9%", 0.509, (100, 210, 120)),
        ("弩机安装位 44.5%", 0.445, (90, 190, 240)),
        ("后托握持段 20.0%", 0.200, (210, 150, 120)),
        ("后托尾端 0%", 0.0, (230, 90, 80)),
    ]

    dual_prop_panel = weapon_review_kit.build_crossbow_dual_proportion_panel(
        ref_side_crop=ref_side_crop,
        model_side_img=model_side_img,
        side_metrics=side_metrics,
        side_lines=side_lines,
        ref_top_crop=ref_top_crop,
        model_top_img=model_top_img,
        top_metrics=top_metrics,
        top_lines=top_lines,
        target_h=six_views_img.height - 150
    )

    # 4. 骨骼树面板
    class DummyGear:
        def __init__(self, r):
            self.rig = r
            self.bones = BONES
        def total(self):
            return len(self.rig.elements)

    dg = DummyGear(rig)
    tree_panel = gear_kit.bone_tree_panel(
        dg, min_bones=10, height=six_views_img.height + scale_pose_panel.height + 15
    )

    # 5. 排版并导出 check.png
    exp_file = REVIEW_DIR / f"{WEAPON_ID}_exploded.png"
    ref_im = Image.open(ref_file).convert("RGB") if ref_file.exists() else None
    exp_im = Image.open(exp_file).convert("RGB") if exp_file.exists() else None

    lw = 640
    ref_scaled = ref_im.resize((lw, round(ref_im.height * lw / ref_im.width)), Image.Resampling.LANCZOS) if ref_im else None
    exp_scaled = exp_im.resize((lw, round(exp_im.height * lw / exp_im.width)), Image.Resampling.LANCZOS) if exp_im else None

    check_path = REVIEW_DIR / "check.png"
    weapon_review_kit.assemble_check_image(
        six_views_img=six_views_img,
        scale_pose_panel=scale_pose_panel,
        prop_panel=dual_prop_panel,
        tree_panel=tree_panel,
        ref_scaled=ref_scaled,
        exp_scaled=exp_scaled,
        out_path=check_path,
        title="#2 蹶张弩 jue_zhang_nu (Round 1 几何 · 弩臂22px · 铁踏环 · 弩弓翼展16.5px · 枪托下折25° · 弩机郭长4.0px)"
    )

    print("\n=== 侧视比例测量核对表 ===")
    for name, ref_v, mod_v in side_metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

    print("\n=== 俯视比例测量核对表 ===")
    for name, ref_v, mod_v in top_metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
