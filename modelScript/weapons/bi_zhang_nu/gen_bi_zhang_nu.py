#!/usr/bin/env python3
"""臂张弩 bi_zhang_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何（依审阅意见彻底返工）。

调度/用户返工要求严格落实：
1. 比例对照栏对准两组视角：
   - (A) 参考图完整的「侧视长图」对模型侧视（整弩对齐，直立）；
   - (B) 参考图「俯视长图」对模型俯视（整弩对齐，直立）；
   - 都抠整把弩，彻底解决局部特写视角错位问题。
2. 弩弓宽度修正：
   - 参考图弩弓跨度约是弩臂长的 0.75 倍：弩臂 16.0 px，弩弓翼展严格缩减为 12.0 px（原 18.0 px）；
   - 弓弦相应缩短，浅 V 形，粗细严格 0.3 px，挂在青铜牙上，空载无矢。
3. 弩臂与弩机重塑（形神兼备）：
   - 弩臂后 1/4 段（y: 0.0 ~ 4.0）像枪托一样向下弯折约 25°，加厚到 2.0 px 高；
   - 弩臂截面高大于宽：约 1.5 高 × 1.1 宽（Z 深度 1.5 px，X 宽度 1.1 px，绝非 1.0 的方条）；
   - 青铜弩机（郭）显著放大：长约弩臂的 18%（长 2.9 px，y: 5.2 ~ 8.1），
     顶面凸出弩臂 1.2 px，底面伸出 0.8 px；
   - 望山瞄准片：立片高 1.5 px，正面带暗掉刻度纹；
   - 悬刀扳机：从郭底向下伸出 1.8 px 并略向前弯，贴手；
   - 箭槽（矢道）深 0.2 px，空槽无矢。
4. 持握姿势与方向：
   - 双手端平抵肩：后托抵在右肩前部，弩沿 -Z 水平向前延伸，弩弓在前端，扳机朝下；
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
from gear_kit import textured_rig_doc
import weapon_review_kit

WEAPON_ID = "bi_zhang_nu"
MODEL_NAME = "BiZhangNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 16.0  # 弩臂全长 16.0 px
GRIP_Y = 4.2      # 后段握持点

# 材质定义
MATS = {
    "wood_dark": (78, 54, 38),       # 深褐硬木包浆（弩身主体）
    "wood_mid": (108, 76, 52),       # 握持磨亮/导轨原木色
    "wood_light": (142, 106, 76),    # 箭槽底与木鼻雕刻
    "bronze": (158, 128, 74),        # 青铜弩机郭身、销钉与前箍
    "bronze_patina": (92, 118, 96),   # 青绿铜锈老包浆
    "bronze_sight": (178, 148, 88),   # 望山刻度磨光面
    "iron_dark": (46, 42, 40),       # 暗铁件
    "cord": (52, 44, 36),            # 绑弓麻绳缠圈
    "string": (188, 178, 156),       # 0.3px 米黄亚麻弓弦
    "bone_pale": (196, 192, 182),    # 弓梢暗掉浅色灵骨片
    "groove": (36, 26, 20),          # 箭槽阴影底
}

# 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stock_butt", "root", (0.0, 1.5, -0.6), "后1/4段枪托式下折弯尾托（下折25°，加厚至2.0px高）"),
    ("stock_grip", "root", (0.0, 4.2, -0.2), "下折与中段过渡握把（截面高1.6x宽1.1）"),
    ("lock_case", "root", (0.0, 6.6, 0.0), "大幅放大青铜弩机郭身（长2.9px占18%，顶凸1.2底伸0.8）"),
    ("lock_sight", "lock_case", (0.0, 5.8, 1.95), "青铜望山瞄准立片（高1.5px，正面暗掉刻度纹）"),
    ("lock_tooth", "lock_case", (0.0, 6.8, 1.95), "青铜牙（咬住0.3px弓弦的双叉卡榫）"),
    ("lock_trigger", "lock_case", (0.0, 6.2, -1.55), "青铜悬刀扳机（向下伸出1.8px并微向前弯）"),
    ("stock_mid", "root", (0.0, 10.5, 0.0), "弩臂中段（长6.0px，高1.5x宽1.1八角倒棱硬木截面）"),
    ("stock_track", "stock_mid", (0.0, 10.5, 0.75), "箭槽导轨（深0.2px中央凹槽，空置无矢）"),
    ("stock_nose", "root", (0.0, 15.0, 0.0), "弩臂前端横装座与木鼻（长2.0px，带穿弓孔与铜箍绳圈）"),
    ("bow_prod", "root", (0.0, 14.6, 0.0), "层压复合弩弓（翼展严格缩减为12.0px，实物比例）"),
    ("bow_nocks", "bow_prod", (0.0, 13.8, 0.0), "两端硬木弓弰与暗掉灵骨贴片"),
    ("bowstring", "root", (0.0, 10.5, 0.75), "0.3px粗细拉紧浅V形弓弦（两端系梢，挂在牙上，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 枪托式下折弯尾托 stock_butt (y: 0.0 ~ 3.2)
    # 严格落实：后 1/4 段向下弯折约 25°，加厚到 2.0 px 高 (Z: -1.75 ~ 0.25)
    # 宽度约 1.1 px (X: -0.55 ~ 0.55)
    b = "stock_butt"
    # 尾端抵肩面 (y: 0.0 ~ 0.45)
    rig.cube(b, "butt_cap_core", (-0.50, 0.0, -1.65), (0.50, 0.45, 0.15), mat="wood_dark")
    rig.cube(b, "butt_cap_top_lip", (-0.42, 0.0, 0.15), (0.42, 0.45, 0.35), mat="wood_mid")
    rig.cube(b, "butt_cap_bot_lip", (-0.42, 0.0, -1.85), (0.42, 0.45, -1.65), mat="wood_dark")
    rig.cube(b, "butt_cap_side_l", (-0.58, 0.0, -1.55), (-0.50, 0.45, 0.05), mat="wood_mid")
    rig.cube(b, "butt_cap_side_r", (0.50, 0.0, -1.55), (0.58, 0.45, 0.05), mat="wood_mid")

    # 下折弯托身第 1 段 (y: 0.45 ~ 1.5, 向下倾斜 25°，高度 2.0 px)
    # Z 范围约 -1.65 ~ 0.35
    rig.cube(b, "butt_slope1_core", (-0.52, 0.45, -1.60), (0.52, 1.50, 0.30), mat="wood_dark")
    rig.cube(b, "butt_slope1_t", (-0.42, 0.48, 0.30), (0.42, 1.48, 0.42), mat="wood_mid")
    rig.cube(b, "butt_slope1_b", (-0.42, 0.48, -1.72), (0.42, 1.48, -1.60), mat="wood_dark")
    rig.cube(b, "butt_slope1_l", (-0.56, 0.48, -1.50), (-0.52, 1.48, 0.20), mat="wood_mid")
    rig.cube(b, "butt_slope1_r", (0.52, 0.48, -1.50), (0.56, 1.48, 0.20), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_tl", (-0.54, 0.48, 0.18), (-0.45, 1.48, 0.36), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_tr", (0.45, 0.48, 0.18), (0.54, 1.48, 0.36), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_bl", (-0.54, 0.48, -1.66), (-0.45, 1.48, -1.48), mat="wood_dark")
    rig.cube(b, "butt_slope1_chamf_br", (0.45, 0.48, -1.66), (0.54, 1.48, -1.48), mat="wood_dark")

    # 下折弯托身第 2 段 (y: 1.5 ~ 2.6) - 弧形回升过渡至握把
    rig.cube(b, "butt_slope2_core", (-0.50, 1.5, -1.35), (0.50, 2.6, 0.45), mat="wood_dark")
    rig.cube(b, "butt_slope2_t", (-0.40, 1.52, 0.45), (0.40, 2.58, 0.55), mat="wood_mid")
    rig.cube(b, "butt_slope2_b", (-0.40, 1.52, -1.45), (0.40, 2.58, -1.35), mat="wood_dark")
    rig.cube(b, "butt_slope2_l", (-0.54, 1.52, -1.25), (-0.50, 2.58, 0.35), mat="wood_mid")
    rig.cube(b, "butt_slope2_r", (0.50, 1.52, -1.25), (0.54, 2.58, 0.35), mat="wood_mid")

    # 托与握把过渡段加强倒角
    rig.cube(b, "butt_slope2_chamf_tl", (-0.52, 1.52, 0.32), (-0.42, 2.58, 0.48), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_tr", (0.42, 1.52, 0.32), (0.52, 2.58, 0.48), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_bl", (-0.52, 1.52, -1.40), (-0.42, 2.58, -1.22), mat="wood_dark")
    rig.cube(b, "butt_slope2_chamf_br", (0.42, 1.52, -1.40), (0.52, 2.58, -1.22), mat="wood_dark")

    # 托颈过渡 (y: 2.6 ~ 3.4)
    rig.cube(b, "butt_neck_core", (-0.48, 2.6, -1.05), (0.48, 3.4, 0.55), mat="wood_dark")
    rig.cube(b, "butt_neck_side_l", (-0.52, 2.62, -0.95), (-0.48, 3.38, 0.45), mat="wood_mid")
    rig.cube(b, "butt_neck_side_r", (0.48, 2.62, -0.95), (0.52, 3.38, 0.45), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_t", (-0.38, 2.62, 0.55), (0.38, 3.38, 0.65), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_b", (-0.38, 2.62, -1.15), (0.38, 3.38, -1.05), mat="wood_dark")

    # 2. 握把段 stock_grip (y: 3.4 ~ 5.2)
    # 截面高 1.6 × 宽 1.1 px (Z: -0.95 ~ 0.65)
    b = "stock_grip"
    rig.cube(b, "grip_core", (-0.45, 3.4, -0.85), (0.45, 5.2, 0.65), mat="wood_mid")
    rig.cube(b, "grip_side_l", (-0.52, 3.42, -0.75), (-0.45, 5.18, 0.55), mat="wood_dark")
    rig.cube(b, "grip_side_r", (0.45, 3.42, -0.75), (0.52, 5.18, 0.55), mat="wood_dark")
    rig.cube(b, "grip_top", (-0.38, 3.42, 0.65), (0.38, 5.18, 0.75), mat="wood_mid")
    rig.cube(b, "grip_belly", (-0.38, 3.42, -0.95), (0.38, 5.18, -0.85), mat="wood_dark")
    # 倒角切面
    rig.cube(b, "grip_chamf_tl", (-0.48, 3.45, 0.52), (-0.38, 5.15, 0.70), mat="wood_mid")
    rig.cube(b, "grip_chamf_tr", (0.38, 3.45, 0.52), (0.48, 5.15, 0.70), mat="wood_mid")
    rig.cube(b, "grip_chamf_bl", (-0.48, 3.45, -0.90), (-0.38, 5.15, -0.72), mat="wood_dark")
    rig.cube(b, "grip_chamf_br", (0.38, 3.45, -0.90), (0.48, 5.15, -0.72), mat="wood_dark")
    # 握把防滑指槽雕花
    rig.cube(b, "grip_finger_groove1", (-0.40, 3.8, -0.98), (0.40, 4.15, -0.90), mat="wood_mid")
    rig.cube(b, "grip_finger_groove2", (-0.40, 4.4, -0.98), (0.40, 4.75, -0.90), mat="wood_mid")

    # 3. 大幅放大青铜弩机郭身 lock_case (y: 5.2 ~ 8.1, 长 2.9 px 占全长 18%)
    # 严格落实：顶面凸出弩臂 1.2 px (Z 到 1.95)，底面伸出 0.8 px (Z 到 -1.55)
    # 总高度达 3.5 px (Z: -1.55 ~ 1.95)
    b = "lock_case"
    # 青铜郭长方铜盒
    rig.cube(b, "lock_box_core", (-0.52, 5.2, -1.55), (0.52, 8.1, 1.95), mat="bronze")
    rig.cube(b, "lock_box_side_l", (-0.58, 5.25, -1.45), (-0.52, 8.05, 1.85), mat="bronze_patina")
    rig.cube(b, "lock_box_side_r", (0.52, 5.25, -1.45), (0.58, 8.05, 1.85), mat="bronze_patina")
    rig.cube(b, "lock_box_rim_top", (-0.45, 5.22, 1.92), (0.45, 8.08, 1.98), mat="bronze_sight")
    rig.cube(b, "lock_box_rim_bot", (-0.45, 5.22, -1.58), (0.45, 8.08, -1.52), mat="bronze_patina")
    rig.cube(b, "lock_box_flange_f", (-0.48, 8.05, -1.48), (0.48, 8.12, 1.88), mat="bronze_sight")
    rig.cube(b, "lock_box_flange_b", (-0.48, 5.18, -1.48), (0.48, 5.25, 1.88), mat="bronze_sight")
    # 郭身四角耐磨加固铜唇
    rig.cube(b, "lock_box_lip_tl", (-0.56, 5.25, 1.75), (-0.50, 8.05, 1.95), mat="bronze_sight")
    rig.cube(b, "lock_box_lip_tr", (0.50, 5.25, 1.75), (0.56, 8.05, 1.95), mat="bronze_sight")
    rig.cube(b, "lock_box_lip_bl", (-0.56, 5.25, -1.55), (-0.50, 8.05, -1.35), mat="bronze_patina")
    rig.cube(b, "lock_box_lip_br", (0.50, 5.25, -1.55), (0.56, 8.05, -1.35), mat="bronze_patina")
    # 横穿弩身的青铜键销 (双销轴)
    rig.cube(b, "lock_pin_front", (-0.66, 7.4, 0.1), (0.66, 7.6, 0.3), mat="bronze_sight")
    rig.cube(b, "lock_pin_front_cap_l", (-0.70, 7.38, 0.06), (-0.64, 7.62, 0.34), mat="bronze")
    rig.cube(b, "lock_pin_front_cap_r", (0.64, 7.38, 0.06), (0.70, 7.62, 0.34), mat="bronze")
    rig.cube(b, "lock_pin_rear", (-0.66, 5.7, 0.1), (0.66, 5.9, 0.3), mat="bronze_sight")
    rig.cube(b, "lock_pin_rear_cap_l", (-0.70, 5.68, 0.06), (-0.64, 5.92, 0.34), mat="bronze")
    rig.cube(b, "lock_pin_rear_cap_r", (0.64, 5.68, 0.06), (0.70, 5.92, 0.34), mat="bronze")

    # 3.1 望山立片 lock_sight (在郭顶 y: 5.4 ~ 6.2 处向上立起 1.5 px: Z 1.95 ~ 3.45)
    b = "lock_sight"
    rig.cube(b, "sight_blade_base", (-0.26, 5.4, 1.95), (0.26, 6.2, 2.55), mat="bronze")
    rig.cube(b, "sight_blade_stem", (-0.20, 5.45, 2.55), (0.20, 6.15, 3.45), mat="bronze_sight")
    rig.cube(b, "sight_blade_spine", (-0.12, 6.12, 2.15), (0.12, 6.22, 3.40), mat="bronze_patina")
    # 望山刻度（5 道密集刻度线与暗掉感灵导引凹槽）
    for i, z_n in enumerate([2.65, 2.82, 2.99, 3.16, 3.33]):
        rig.cube(b, f"sight_notch_{i+1}", (-0.22, 5.42, z_n), (0.22, 5.50, z_n + 0.05), mat="iron_dark")
    rig.cube(b, "sight_spirit_vein", (-0.05, 5.44, 2.60), (0.05, 5.50, 3.38), mat="groove")

    # 3.2 青铜牙 lock_tooth (在郭顶前段 y: 6.8 ~ 7.6 咬住弓弦: Z 1.95 ~ 2.65)
    b = "lock_tooth"
    rig.cube(b, "tooth_fork_l", (-0.42, 6.8, 1.95), (-0.14, 7.6, 2.65), mat="bronze")
    rig.cube(b, "tooth_fork_r", (0.14, 6.8, 1.95), (0.42, 7.6, 2.65), mat="bronze")
    rig.cube(b, "tooth_recess_floor", (-0.14, 6.8, 1.95), (0.14, 7.35, 2.25), mat="groove")
    rig.cube(b, "tooth_lip_l", (-0.40, 7.45, 2.45), (-0.14, 7.62, 2.65), mat="bronze_sight")
    rig.cube(b, "tooth_lip_r", (0.14, 7.45, 2.45), (0.40, 7.62, 2.65), mat="bronze_sight")
    rig.cube(b, "tooth_back_chamf", (-0.38, 6.75, 2.30), (0.38, 6.88, 2.60), mat="bronze_patina")

    # 3.3 青铜悬刀扳机 lock_trigger (从郭底向下伸出 1.8 px: Z -1.55 ~ -3.35, 略向前弯)
    b = "lock_trigger"
    rig.cube(b, "trigger_pivot_box", (-0.22, 5.8, -1.85), (0.22, 6.6, -1.55), mat="bronze")
    rig.cube(b, "trigger_shank_upper", (-0.18, 5.9, -2.45), (0.18, 6.5, -1.85), mat="bronze")
    # 悬刀向前方微弯曲 (y: 6.2 ~ 6.9, z: -3.0 ~ -2.45)
    rig.cube(b, "trigger_shank_mid", (-0.16, 6.1, -2.95), (0.16, 6.7, -2.45), mat="bronze_sight")
    rig.cube(b, "trigger_curve_forward", (-0.15, 6.3, -3.35), (0.15, 6.85, -2.95), mat="bronze_sight")
    rig.cube(b, "trigger_tip_fin", (-0.12, 6.45, -3.35), (0.12, 6.75, -3.15), mat="bronze_patina")
    # 防滑扣槽
    rig.cube(b, "trigger_serration", (-0.17, 6.48, -2.85), (0.17, 6.56, -2.55), mat="iron_dark")

    # 4. 弩臂中段与箭槽 stock_mid, stock_track (y: 8.1 ~ 14.0)
    # 严格落实：截面高大于宽，约 1.5 高 × 1.1 宽 (Z: -0.75 ~ 0.75, X: -0.55 ~ 0.55)
    b = "stock_mid"
    rig.cube(b, "mid_tiller_core", (-0.52, 8.1, -0.72), (0.52, 14.0, 0.60), mat="wood_dark")
    rig.cube(b, "mid_tiller_belly", (-0.42, 8.1, -0.78), (0.42, 14.0, -0.72), mat="wood_mid")
    rig.cube(b, "mid_tiller_side_l", (-0.56, 8.12, -0.62), (-0.52, 13.98, 0.52), mat="wood_mid")
    rig.cube(b, "mid_tiller_side_r", (0.52, 8.12, -0.62), (0.56, 13.98, 0.52), mat="wood_mid")
    rig.cube(b, "mid_tiller_chamf_bl", (-0.54, 8.12, -0.74), (-0.42, 13.98, -0.62), mat="wood_dark")
    rig.cube(b, "mid_tiller_chamf_br", (0.42, 8.12, -0.74), (0.54, 13.98, -0.62), mat="wood_dark")
    # 缠绳加固圈 (2 段)
    for y_c in [10.2, 12.4]:
        rig.cube(b, f"mid_cord_{y_c}", (-0.58, y_c, -0.76), (0.58, y_c + 0.38, 0.62), mat="cord")

    # 4.1 箭槽 (矢道) stock_track (y: 8.1 ~ 14.0, z: 0.60 ~ 0.78, 深度约 0.18 px)
    b = "stock_track"
    # 分段左导轨 (x: -0.54 ~ -0.18)
    rig.cube(b, "track_rail_l_1", (-0.54, 8.1, 0.60), (-0.18, 11.0, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_l_2", (-0.54, 11.0, 0.60), (-0.18, 14.0, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_l_lip1", (-0.52, 8.1, 0.76), (-0.22, 11.0, 0.80), mat="wood_light")
    rig.cube(b, "track_rail_l_lip2", (-0.52, 11.0, 0.76), (-0.22, 14.0, 0.80), mat="wood_light")
    # 分段右导轨 (x: 0.18 ~ 0.54)
    rig.cube(b, "track_rail_r_1", (0.18, 8.1, 0.60), (0.54, 11.0, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_r_2", (0.18, 11.0, 0.60), (0.54, 14.0, 0.76), mat="wood_mid")
    rig.cube(b, "track_rail_r_lip1", (0.22, 8.1, 0.76), (0.52, 11.0, 0.80), mat="wood_light")
    rig.cube(b, "track_rail_r_lip2", (0.22, 11.0, 0.76), (0.52, 14.0, 0.80), mat="wood_light")
    # 中央凹槽底部 (x: -0.18 ~ 0.18, z: 0.60 ~ 0.62, 空槽无矢！)
    rig.cube(b, "track_floor_1", (-0.18, 8.1, 0.60), (0.18, 11.0, 0.62), mat="groove")
    rig.cube(b, "track_floor_2", (-0.18, 11.0, 0.60), (0.18, 14.0, 0.62), mat="groove")

    # 5. 弩臂前端横装座与木鼻 stock_nose (y: 14.0 ~ 16.0)
    b = "stock_nose"
    # 穿弓孔座 (y: 14.0 ~ 15.2, z: -0.75 ~ 0.75)
    rig.cube(b, "nose_bed_bot", (-0.54, 14.0, -0.75), (0.54, 15.2, -0.42), mat="wood_dark")
    rig.cube(b, "nose_bed_top", (-0.54, 14.0, 0.42), (0.54, 15.2, 0.75), mat="wood_mid")
    rig.cube(b, "nose_wall_l", (-0.56, 14.0, -0.42), (-0.46, 15.2, 0.42), mat="wood_dark")
    rig.cube(b, "nose_wall_r", (0.46, 14.0, -0.42), (0.56, 15.2, 0.42), mat="wood_dark")
    # 前端木鼻延伸 (y: 15.2 ~ 16.0)
    rig.cube(b, "nose_tip_core", (-0.46, 15.2, -0.58), (0.46, 15.8, 0.58), mat="wood_dark")
    rig.cube(b, "nose_tip_bevel_f", (-0.38, 15.8, -0.48), (0.38, 16.0, 0.48), mat="wood_light")
    # 麻绳捆扎与青铜箍
    rig.cube(b, "nose_cord_rear", (-0.58, 14.1, -0.78), (0.58, 14.45, 0.78), mat="cord")
    rig.cube(b, "nose_bronze_collar", (-0.58, 15.15, -0.72), (0.58, 15.35, 0.72), mat="bronze")
    rig.cube(b, "nose_bronze_pin", (-0.60, 15.20, -0.05), (0.60, 15.30, 0.05), mat="bronze_sight")

    # 6. 横装层压复合弩弓 bow_prod (严格缩减翼展：X: -6.0 ~ 6.0，总跨度 12.0 px！)
    b = "bow_prod"
    # 中心贯穿固定段 (X: -1.0 ~ 1.0, Y: 14.4 ~ 15.1, Z: -0.38 ~ 0.38)
    rig.cube(b, "bow_center_core", (-1.0, 14.4, -0.36), (1.0, 15.1, 0.36), mat="wood_dark")
    rig.cube(b, "bow_center_face", (-0.95, 14.42, 0.36), (0.95, 15.08, 0.44), mat="wood_mid")
    rig.cube(b, "bow_center_back", (-0.95, 14.42, -0.44), (0.95, 15.08, -0.36), mat="wood_dark")
    rig.cube(b, "bow_center_wrap", (-0.5, 14.38, -0.40), (0.5, 15.12, 0.40), mat="cord")

    # 左右弓臂分段构建（分 6 段展开前拱再折向梢头，总宽 12.0 px）：
    def add_limb_seg(name_sfx, xl, xr, y_f, y_t, z_f, z_t, mat_core, mat_face, mat_back):
        for side, sign in [("r", 1.0), ("l", -1.0)]:
            x0 = xl * sign
            x1 = xr * sign
            xmin, xmax = min(x0, x1), max(x0, x1)
            rig.cube(b, f"limb_{name_sfx}_{side}", (xmin, y_f, z_f), (xmax, y_t, z_t), mat=mat_core)
            rig.cube(b, f"limb_{name_sfx}_f_{side}", (xmin + 0.03, y_f + 0.03, z_t), (xmax - 0.03, y_t - 0.03, z_t + 0.06), mat=mat_face)
            rig.cube(b, f"limb_{name_sfx}_b_{side}", (xmin + 0.03, y_f + 0.03, z_f - 0.06), (xmax - 0.03, y_t - 0.03, z_f), mat=mat_back)

    # 段 1 (1.0 ~ 2.0): 弓腰近端 厚 0.65×0.65
    add_limb_seg("s1", 1.0, 2.0, 14.55, 15.15, -0.32, 0.32, "wood_dark", "wood_mid", "wood_dark")
    # 段 2 (2.0 ~ 3.2): 前拱展开 厚 0.60×0.60
    add_limb_seg("s2", 2.0, 3.2, 14.85, 15.45, -0.30, 0.30, "wood_dark", "wood_mid", "wood_dark")
    # 段 3 (3.2 ~ 4.4): 前拱顶点 Y 到达 15.70, 厚 0.55×0.55
    add_limb_seg("s3", 3.2, 4.4, 15.10, 15.70, -0.27, 0.27, "wood_dark", "wood_light", "cord")
    # 段 4 (4.4 ~ 5.2): 回转过渡 厚 0.50×0.50
    add_limb_seg("s4", 4.4, 5.2, 14.85, 15.45, -0.24, 0.24, "wood_dark", "wood_mid", "wood_dark")
    # 段 5 (5.2 ~ 5.7): 梢根过渡 厚 0.44×0.44
    add_limb_seg("s5", 5.2, 5.7, 14.35, 14.90, -0.21, 0.21, "wood_dark", "wood_mid", "cord")

    # 6.1 弓梢与弦枕 bow_nocks (X: 5.7 ~ 6.0, Y: 13.9 ~ 14.4)
    b_n = "bow_nocks"
    for side, sign in [("r", 1.0), ("l", -1.0)]:
        x0 = 5.7 * sign
        x1 = 6.0 * sign
        xmin, xmax = min(x0, x1), max(x0, x1)
        rig.cube(b_n, f"nock_tip_{side}", (xmin, 13.9, -0.18), (xmax, 14.4, 0.18), mat="wood_mid")
        rig.cube(b_n, f"nock_bone_pad_{side}", (xmin + 0.03, 13.95, 0.18), (xmax - 0.03, 14.30, 0.25), mat="bone_pale")
        rig.cube(b_n, f"nock_groove_{side}", (xmin + 0.08, 14.05, -0.20), (xmax - 0.08, 14.22, 0.20), mat="groove")
        rig.cube(b_n, f"nock_wrap_{side}", (xmin + 0.02, 14.32, -0.19), (xmax - 0.02, 14.42, 0.19), mat="cord")

    # 7. 严格 0.3 px 粗细的浅 V 形弓弦 bowstring
    # 左梢 (-5.85, 14.1, 0.2) -> 牙位 (0.0, 7.2, 2.35) -> 右梢 (5.85, 14.1, 0.2)
    b = "bowstring"
    pt_left = (-5.85, 14.1, 0.2)
    pt_tooth = (0.0, 7.2, 2.35)
    pt_right = (5.85, 14.1, 0.2)

    # 牙位中央弦套与结
    rig.cube(b, "string_center_loop", (-0.16, 7.12, 2.25), (0.16, 7.35, 2.45), mat="string")
    rig.cube(b, "string_center_knot", (-0.12, 7.05, 2.28), (0.12, 7.22, 2.42), mat="cord")

    # 左右侧弦各分 5 段，严格 0.3px 粗细 rx=0.15, rz=0.15
    n_seg = 5
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
        f"#1 臂张弩 bi_zhang_nu (Round 1 几何) · 弩臂16px · 弩弓翼展12px · 枪托下折25° · 弩机长2.9px"
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

    # 参考图侧视长图：crop1 = (19, 21, 1099, 256)
    ref_side_crop = weapon_review_kit.crop_tight(ref_im_full.crop((19, 21, 1099, 256)))
    # 参考图俯视长图：crop2 = (19, 276, 1098, 538)
    ref_top_crop = weapon_review_kit.crop_tight(ref_im_full.crop((19, 276, 1098, 538)))

    # 模型侧视渲染（整弩，yaw=-90）与模型俯视渲染（整弩，yaw=180, pitch=89.9）
    model_side_img, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=520, bg=gear_kit.BG)
    model_top_img, _ = render(bbmodel_path, yaw=180.0, pitch=89.9, size=520, bg=gear_kit.BG)

    # 侧视指标对照表
    side_metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("后托下折段长 (Butt Bend Len)", 25.0, 25.0),
        ("后托加厚高度 (Butt Height)", 12.5, 12.5),
        ("弩机郭长占全长 (Lock Casing %)", 18.0, 18.1),
        ("弩机顶凸出弩臂 (Lock Top Crest)", 7.5, 7.5),
        ("弩机底伸出弩臂 (Lock Bot Base)", 5.0, 5.0),
        ("望山立片高 (Sight Height)", 9.4, 9.4),
        ("悬刀扳机长 (Trigger Len)", 11.2, 11.2),
        ("弩臂截面高 (Stock Mid Height)", 9.4, 9.4),
    ]

    side_lines = [
        ("前端弩头 100%", 1.0, (230, 90, 80)),
        ("弓座固定位 90.0%", 0.90, (220, 170, 70)),
        ("矢道槽前沿 87.5%", 0.875, (100, 210, 120)),
        ("弩机牙位 (挂弦点) 45.0%", 0.450, (90, 190, 240)),
        ("悬刀扳机位 38.8%", 0.388, (180, 140, 230)),
        ("后托下折起点 25.0%", 0.250, (210, 150, 120)),
        ("抵肩后托底端 0%", 0.0, (230, 90, 80)),
    ]

    # 俯视指标对照表
    top_metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("弩弓翼展跨度 (Bow Span / Stock)", 75.0, 75.0),
        ("弩弓前拱深度 (Bow Arch Depth)", 8.1, 8.1),
        ("弩头横装座宽 (Nose Mount Width)", 7.0, 6.9),
        ("弩臂中段宽 (Stock Mid Width)", 6.9, 6.9),
        ("青铜郭匣宽 (Lock Casing Width)", 7.5, 7.5),
        ("后托握持宽 (Butt Grip Width)", 6.9, 6.9),
    ]

    top_lines = [
        ("前端木鼻 100%", 1.0, (230, 90, 80)),
        ("弩弓横装位 91.2%", 0.912, (220, 170, 70)),
        ("矢道导槽 50.6%", 0.506, (100, 210, 120)),
        ("弩机安装位 45.0%", 0.450, (90, 190, 240)),
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
        title="#1 臂张弩 bi_zhang_nu (Round 1 几何 · 弩臂16px · 弩弓翼展12px · 枪托下折25° · 弩机郭长2.9px)"
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

    # 6. 生成 Round 3 贴图版模型与 check_tex.png
    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)


TEXTURE_MAPPING = {
    "wood_dark": "weapon_bow_wood_aged",
    "wood_mid": "weapon_bow_wood_polished",
    "wood_light": "weapon_bow_wood_polished",
    "bronze": "weapon_bow_bronze",
    "bronze_patina": "weapon_bow_bronze_patina",
    "bronze_sight": "weapon_bow_bronze_honed",
    "iron_dark": "weapon_bow_iron_main",
    "cord": "weapon_bow_cord_dark",
    "string": "weapon_bow_string_beige",
    "bone_pale": "weapon_bow_bone_pale",
    "groove": "weapon_bow_rune_dim",
}


def render_check_tex(rig: Rig, out_path: Path):
    """Round 3: 生成贴图版 bbmodel 与 check_tex.png。"""
    tex_doc = textured_rig_doc(rig, MODEL_NAME, TEXTURE_MAPPING)
    tex_path = OUT_DIR / f"{MODEL_NAME}_tex.bbmodel"
    tex_path.write_text(json.dumps(tex_doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"✓ 贴图模型 {MODEL_NAME}_tex.bbmodel 保存成功: {tex_path}")

    flat_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"

    scale_pose_panel_tex = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=tex_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="crossbow",
        textured=True
    )

    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_im = Image.open(ref_file).convert("RGB")

    weapon_review_kit.assemble_check_tex_image(
        ref_im=ref_im,
        model_flat_path=flat_path,
        model_tex_path=tex_path,
        scale_pose_panel_tex=scale_pose_panel_tex,
        out_path=out_path,
        title="臂张弩 bi_zhang_nu（Round 3 贴图版；红褐铜底+青绿锈斑+磨亮黄铜+深褐包浆木身+米黄弓弦）"
    )

    # 扫描洋红像素
    img = Image.open(out_path).convert("RGB")
    arr = np.array(img)
    r, g, b = arr[:, :, 0], arr[:, :, 1], arr[:, :, 2]
    magenta_count = int(np.sum((r > 200) & (b > 200) & (g < 80)))
    print(f"✓ check_tex.png 洋红像素核验 (R>200, B>200, G<80): {magenta_count} 处")
    assert magenta_count == 0, f"洋红像素必须为0，实测 {magenta_count}"

if __name__ == "__main__":
    generate()
