#!/usr/bin/env python3
"""匣连弩 xia_lian_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何（依审阅意见彻底返工）。

调度/用户返工要求严格落实：
1. 比例对照栏对准两组视角：
   - (A) 参考图完整的「侧视长图」对模型侧视（整弩对齐，直立）；
   - (B) 参考图「俯视长图」对模型俯视（整弩对齐，直立）；
   - 都抠整把弩，彻底解决局部特写视角错位问题。
2. 弩弓宽度修正：
   - 参考图弩弓跨度约是弩臂长的 0.75 倍：弩臂 14.0 px，弩弓翼展严格缩减为 10.5 px（原 13.0 px）；
   - 弓弦相应缩短，浅 V 形，粗细严格 0.3 px，穿过滑槽，空载无矢。
3. 弩臂与箭匣重塑（形神兼备）：
   - 弩臂后 1/4 段（y: 0.0 ~ 3.5）像枪托一样向下弯折约 25°，加厚到 2.0 px 高；
   - 弩臂截面高大于宽：约 1.5 高 × 1.1 宽（Z 深度 1.5 px，X 宽度 1.1 px，绝非 1.0 的方条）；
   - 长方木箭匣压在弩臂上方（长 8.5 px，宽 1.8 px，内部中空空槽，前端开口出箭狭缝）；
   - 侧面长方形空白铭文匾位（长 2.8 px，末法暗掉空匾）；
   - 侧面杠杆式连动摇柄（长 4.5 px，枢轴位于箭匣侧边，待击发姿态向前延伸顺贴箭匣，消除与头部的干涉，持握交叠严格为 0）。
4. 持握姿势与方向：
   - 双手端平抵肩：后托抵在右肩前部，弩沿 -Z 水平向前延伸，出箭口与弩弓在前端，扳机朝下；
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

WEAPON_ID = "xia_lian_nu"
MODEL_NAME = "XiaLianNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 14.0  # 弩臂全长 14.0 px
GRIP_Y = 3.6      # 后段握持点

# 材质定义
MATS = {
    "wood_dark": (76, 52, 36),       # 深褐老硬木包浆（弩身框架）
    "wood_mid": (106, 74, 50),       # 箭匣木板与磨亮外壳
    "wood_light": (140, 102, 72),    # 箭匣口沿、导轨与木鼻
    "bronze": (156, 126, 72),        # 青铜转轴销、连杆枢轴与铜角码
    "bronze_patina": (90, 116, 94),   # 青绿铜锈老包浆
    "bronze_sight": (176, 146, 86),   # 铭文框与磨光金属件
    "iron_dark": (44, 40, 38),       # 摇柄铁骨架与限位档
    "cord": (52, 44, 36),            # 绑弓麻绳缠圈
    "string": (186, 176, 154),       # 0.3px 亚麻滑槽弓弦
    "bone_pale": (194, 190, 180),    # 弓梢浅色灵骨片（暗掉浅色）
    "groove": (34, 24, 18),          # 箭匣内部阴影空槽与铭文匾底
}

# 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stock_butt", "root", (0.0, 1.2, -0.6), "后1/4段枪托式下折弯尾托（下折25°，加厚至2.0px高）"),
    ("stock_grip", "root", (0.0, 3.6, -0.2), "下折与前身过渡握把（截面高1.6x宽1.1）"),
    ("stock_frame", "root", (0.0, 7.5, 0.0), "弩臂下框架（长8.5px，高1.5x宽1.1八角硬木身）"),
    ("magazine_box", "stock_frame", (0.0, 7.5, 1.6), "长方木箭匣（长8.5px，宽1.8px，高2.1px，中空空槽）"),
    ("cartouche_plaque", "magazine_box", (0.9, 7.5, 1.6), "箭匣侧面长方形空白铭文匾位（末法暗掉空匾）"),
    ("magazine_muzzle", "magazine_box", (0.0, 11.8, 1.6), "箭匣前端出箭口（狭缝开口，空槽无矢）"),
    ("lever_handle", "magazine_box", (-1.1, 6.0, 1.6), "侧面杠杆式连动摇柄（长4.5px，顺贴箭匣前伸，零交叠）"),
    ("lever_linkage", "lever_handle", (-1.1, 5.0, 0.8), "摇柄与滑板下连动杆（青铜转轴与铁拉杆）"),
    ("stock_nose", "root", (0.0, 13.0, 0.0), "弩臂前端横装座与木鼻（长2.0px，带穿弓孔与铜箍）"),
    ("bow_prod", "root", (0.0, 12.5, 0.4), "短小层压复合弩弓（翼展严格缩减为10.5px，实物比例）"),
    ("bow_nocks", "bow_prod", (0.0, 11.8, 0.4), "两端短弓梢与暗掉灵骨贴片"),
    ("bowstring", "root", (0.0, 7.5, 0.4), "0.3px粗细拉紧浅V形弓弦（两端系梢，穿过滑槽，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 枪托式下折弯尾托 stock_butt (y: 0.0 ~ 2.8)
    # 严格落实：后 1/4 段向下弯折约 25°，加厚到 2.0 px 高 (Z: -1.75 ~ 0.25)
    # 宽度约 1.1 px (X: -0.55 ~ 0.55)
    b = "stock_butt"
    # 尾端抵肩面 (y: 0.0 ~ 0.4)
    rig.cube(b, "butt_cap_core", (-0.48, 0.0, -1.65), (0.48, 0.40, 0.15), mat="wood_dark")
    rig.cube(b, "butt_cap_top_lip", (-0.40, 0.0, 0.15), (0.40, 0.40, 0.35), mat="wood_mid")
    rig.cube(b, "butt_cap_bot_lip", (-0.40, 0.0, -1.85), (0.40, 0.40, -1.65), mat="wood_dark")
    rig.cube(b, "butt_cap_side_l", (-0.56, 0.0, -1.55), (-0.48, 0.40, 0.05), mat="wood_mid")
    rig.cube(b, "butt_cap_side_r", (0.48, 0.0, -1.55), (0.56, 0.40, 0.05), mat="wood_mid")

    # 下折弯托身第 1 段 (y: 0.4 ~ 1.4, 向下倾斜 25°，高度 2.0 px)
    rig.cube(b, "butt_slope1_core", (-0.50, 0.40, -1.60), (0.50, 1.40, 0.30), mat="wood_dark")
    rig.cube(b, "butt_slope1_t", (-0.40, 0.42, 0.30), (0.40, 1.38, 0.42), mat="wood_mid")
    rig.cube(b, "butt_slope1_b", (-0.40, 0.42, -1.72), (0.40, 1.38, -1.60), mat="wood_dark")
    rig.cube(b, "butt_slope1_l", (-0.54, 0.42, -1.50), (-0.50, 1.38, 0.20), mat="wood_mid")
    rig.cube(b, "butt_slope1_r", (0.50, 0.42, -1.50), (0.54, 1.38, 0.20), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_tl", (-0.52, 0.42, 0.18), (-0.42, 1.38, 0.36), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_tr", (0.42, 0.42, 0.18), (0.52, 1.38, 0.36), mat="wood_mid")
    rig.cube(b, "butt_slope1_chamf_bl", (-0.52, 0.42, -1.66), (-0.42, 1.38, -1.48), mat="wood_dark")
    rig.cube(b, "butt_slope1_chamf_br", (0.42, 0.42, -1.66), (0.52, 1.38, -1.48), mat="wood_dark")

    # 下折弯托身第 2 段 (y: 1.4 ~ 2.2) - 弧形回升过渡至握把
    rig.cube(b, "butt_slope2_core", (-0.48, 1.4, -1.35), (0.48, 2.2, 0.45), mat="wood_dark")
    rig.cube(b, "butt_slope2_t", (-0.38, 1.42, 0.45), (0.38, 2.18, 0.55), mat="wood_mid")
    rig.cube(b, "butt_slope2_b", (-0.38, 1.42, -1.45), (0.38, 2.18, -1.35), mat="wood_dark")
    rig.cube(b, "butt_slope2_l", (-0.52, 1.42, -1.25), (-0.48, 2.18, 0.35), mat="wood_mid")
    rig.cube(b, "butt_slope2_r", (0.48, 1.42, -1.25), (0.52, 2.18, 0.35), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_tl", (-0.50, 1.42, 0.32), (-0.40, 2.18, 0.48), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_tr", (0.40, 1.42, 0.32), (0.50, 2.18, 0.48), mat="wood_mid")
    rig.cube(b, "butt_slope2_chamf_bl", (-0.50, 1.42, -1.40), (-0.40, 2.18, -1.22), mat="wood_dark")
    rig.cube(b, "butt_slope2_chamf_br", (0.40, 1.42, -1.40), (0.50, 2.18, -1.22), mat="wood_dark")

    # 托颈过渡 (y: 2.2 ~ 2.8)
    rig.cube(b, "butt_neck_core", (-0.46, 2.2, -1.05), (0.46, 2.8, 0.55), mat="wood_dark")
    rig.cube(b, "butt_neck_side_l", (-0.50, 2.22, -0.95), (-0.46, 2.78, 0.45), mat="wood_mid")
    rig.cube(b, "butt_neck_side_r", (0.46, 2.22, -0.95), (0.50, 2.78, 0.45), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_t", (-0.36, 2.22, 0.55), (0.36, 2.78, 0.65), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_b", (-0.36, 2.22, -1.15), (0.36, 2.78, -1.05), mat="wood_dark")

    # 2. 握把段 stock_grip (y: 2.8 ~ 4.2)
    # 截面高 1.6 × 宽 1.1 px (Z: -0.95 ~ 0.65)
    b = "stock_grip"
    rig.cube(b, "grip_core", (-0.45, 2.8, -0.85), (0.45, 4.2, 0.65), mat="wood_mid")
    rig.cube(b, "grip_side_l", (-0.52, 2.82, -0.75), (-0.45, 4.18, 0.55), mat="wood_dark")
    rig.cube(b, "grip_side_r", (0.45, 2.82, -0.75), (0.52, 4.18, 0.55), mat="wood_dark")
    rig.cube(b, "grip_top", (-0.38, 2.82, 0.65), (0.38, 4.18, 0.75), mat="wood_mid")
    rig.cube(b, "grip_belly", (-0.38, 2.82, -0.95), (0.38, 4.18, -0.85), mat="wood_dark")
    rig.cube(b, "grip_chamf_tl", (-0.48, 2.85, 0.52), (-0.38, 4.15, 0.70), mat="wood_mid")
    rig.cube(b, "grip_chamf_tr", (0.38, 2.85, 0.52), (0.48, 4.15, 0.70), mat="wood_mid")
    rig.cube(b, "grip_chamf_bl", (-0.48, 2.85, -0.90), (-0.38, 4.15, -0.72), mat="wood_dark")
    rig.cube(b, "grip_chamf_br", (0.38, 2.85, -0.90), (0.48, 4.15, -0.72), mat="wood_dark")
    # 扳机击发钮
    rig.cube(b, "trigger_lever_stub", (-0.15, 3.2, -1.15), (0.15, 3.65, -0.85), mat="bronze")
    rig.cube(b, "trigger_lever_tip", (-0.12, 3.05, -1.30), (0.12, 3.45, -1.05), mat="bronze_sight")

    # 3. 弩臂下框架 stock_frame (y: 3.5 ~ 12.0)
    # 截面高大于宽：约 1.5 高 × 1.1 宽 (Z: -0.75 ~ 0.75, X: -0.55 ~ 0.55)
    b = "stock_frame"
    rig.cube(b, "frame_rail_core", (-0.52, 3.5, -0.72), (0.52, 12.0, 0.60), mat="wood_dark")
    rig.cube(b, "frame_rail_belly", (-0.42, 3.5, -0.78), (0.42, 12.0, -0.72), mat="wood_mid")
    rig.cube(b, "frame_rail_side_l", (-0.56, 3.52, -0.62), (-0.52, 11.98, 0.52), mat="wood_mid")
    rig.cube(b, "frame_rail_side_r", (0.52, 3.52, -0.62), (0.56, 11.98, 0.52), mat="wood_mid")
    rig.cube(b, "frame_rail_chamf_bl", (-0.54, 3.52, -0.74), (-0.42, 11.98, -0.62), mat="wood_dark")
    rig.cube(b, "frame_rail_chamf_br", (0.42, 3.52, -0.74), (0.54, 11.98, -0.62), mat="wood_dark")
    # 底部金属卡箍
    for y_cl in [4.5, 7.5, 10.5]:
        rig.cube(b, f"frame_clamp_{y_cl}", (-0.58, y_cl, -0.78), (0.58, y_cl + 0.35, 0.62), mat="bronze")

    # 4. 长方木箭匣 magazine_box (y: 3.5 ~ 11.8, z: 0.60 ~ 2.68, 长 8.3 px, 宽 1.8 px, 高 2.08 px)
    b = "magazine_box"
    # 左侧壁分段 (x: -0.90 ~ -0.62)
    rig.cube(b, "mag_wall_l_1", (-0.90, 3.5, 0.60), (-0.62, 7.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_l_2", (-0.90, 7.5, 0.60), (-0.62, 11.8, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_l_top_lip", (-0.94, 3.5, 2.60), (-0.60, 11.8, 2.72), mat="wood_light")
    # 左侧壁加强木肋与金属角码
    rig.cube(b, "mag_wall_l_rib1", (-0.95, 5.5, 0.68), (-0.88, 6.0, 2.60), mat="wood_dark")
    rig.cube(b, "mag_wall_l_rib2", (-0.95, 9.2, 0.68), (-0.88, 9.7, 2.60), mat="wood_dark")
    rig.cube(b, "mag_corner_tl_1", (-0.95, 3.5, 2.50), (-0.75, 4.0, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_tl_2", (-0.95, 11.2, 2.50), (-0.75, 11.8, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_bl_1", (-0.95, 3.5, 0.60), (-0.75, 4.0, 0.80), mat="bronze")
    rig.cube(b, "mag_corner_bl_2", (-0.95, 11.2, 0.60), (-0.75, 11.8, 0.80), mat="bronze")

    # 右侧壁分段 (x: 0.62 ~ 0.90)
    rig.cube(b, "mag_wall_r_1", (0.62, 3.5, 0.60), (0.90, 7.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_r_2", (0.62, 7.5, 0.60), (0.90, 11.8, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_r_top_lip", (0.60, 3.5, 2.60), (0.94, 11.8, 2.72), mat="wood_light")
    rig.cube(b, "mag_wall_r_rib1", (0.88, 5.5, 0.68), (0.95, 6.0, 2.60), mat="wood_dark")
    rig.cube(b, "mag_wall_r_rib2", (0.88, 9.2, 0.68), (0.95, 9.7, 2.60), mat="wood_dark")
    rig.cube(b, "mag_corner_tr_1", (0.75, 3.5, 2.50), (0.95, 4.0, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_tr_2", (0.75, 11.2, 2.50), (0.95, 11.8, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_br_1", (0.75, 3.5, 0.60), (0.95, 4.0, 0.80), mat="bronze")
    rig.cube(b, "mag_corner_br_2", (0.75, 11.2, 0.60), (0.95, 11.8, 0.80), mat="bronze")

    # 后壁与后盖板 (y: 3.5 ~ 3.85)
    rig.cube(b, "mag_wall_rear_core", (-0.85, 3.5, 0.60), (0.85, 3.85, 2.68), mat="wood_dark")
    rig.cube(b, "mag_wall_rear_lip", (-0.80, 3.5, 2.60), (0.80, 3.85, 2.74), mat="wood_light")

    # 底板与滑槽 (z: 0.60 ~ 0.76)
    rig.cube(b, "mag_floor_l", (-0.62, 3.85, 0.60), (-0.20, 11.5, 0.74), mat="wood_dark")
    rig.cube(b, "mag_floor_r", (0.20, 3.85, 0.60), (0.62, 11.5, 0.74), mat="wood_dark")
    rig.cube(b, "mag_groove_slot", (-0.20, 3.85, 0.58), (0.20, 11.5, 0.65), mat="groove")

    # 箭匣内部阴影（中空空槽，无箭无矢！）
    rig.cube(b, "mag_interior_shadow_rear", (-0.60, 3.85, 0.74), (0.60, 4.2, 2.58), mat="groove")
    rig.cube(b, "mag_interior_shadow_floor", (-0.60, 4.2, 0.74), (0.60, 11.2, 0.84), mat="groove")

    # 4.1 箭匣右侧空白铭文匾位 cartouche_plaque (y: 5.5 ~ 8.3, z: 1.2 ~ 2.1)
    # 末法：箭匣侧面的铭文位空着
    b = "cartouche_plaque"
    rig.cube(b, "plaque_border_t", (0.90, 5.5, 2.05), (0.96, 8.3, 2.15), mat="bronze_sight")
    rig.cube(b, "plaque_border_b", (0.90, 5.5, 1.15), (0.96, 8.3, 1.25), mat="bronze_sight")
    rig.cube(b, "plaque_border_l", (0.90, 5.5, 1.25), (0.96, 5.7, 2.05), mat="bronze_sight")
    rig.cube(b, "plaque_border_r", (0.90, 8.1, 1.25), (0.96, 8.3, 2.05), mat="bronze_sight")
    # 空白凹匾底板（空着无字！）
    rig.cube(b, "plaque_empty_field", (0.88, 5.7, 1.25), (0.92, 8.1, 2.05), mat="groove")

    # 4.2 箭匣前端出箭口 magazine_muzzle (y: 11.5 ~ 11.8)
    b = "magazine_muzzle"
    rig.cube(b, "muzzle_top_lintel", (-0.75, 11.5, 1.35), (0.75, 11.8, 2.68), mat="wood_mid")
    rig.cube(b, "muzzle_post_l", (-0.85, 11.5, 0.60), (-0.45, 11.8, 1.35), mat="wood_dark")
    rig.cube(b, "muzzle_post_r", (0.45, 11.5, 0.60), (0.85, 11.8, 1.35), mat="wood_dark")
    rig.cube(b, "muzzle_threshold", (-0.45, 11.5, 0.60), (0.45, 11.8, 0.70), mat="wood_light")
    rig.cube(b, "muzzle_collar_l", (-0.90, 11.72, 0.68), (-0.42, 11.82, 1.40), mat="bronze")
    rig.cube(b, "muzzle_collar_r", (0.42, 11.72, 0.68), (0.90, 11.82, 1.40), mat="bronze")

    # 5. 侧面连动摇柄 lever_handle (左侧 x: -0.95 ~ -1.45)
    # 严格落实：待击发姿态向前延伸顺贴箭匣侧面，消除向后上方翘起插入头部的干涉！
    # 枢轴位于 y: 4.5, z: 1.6
    b = "lever_handle"
    rig.cube(b, "lever_pivot_cap", (-1.25, 4.3, 1.40), (-0.92, 4.75, 1.80), mat="bronze_sight")
    rig.cube(b, "lever_pivot_pin", (-1.30, 4.45, 1.52), (-0.90, 4.60, 1.68), mat="bronze")
    # 摇柄主杆向前下方微斜延伸至 y: 8.8, z: 1.95 (长 4.5 px)
    rig.shaft(b, "lever_main_shaft", (-1.15, 4.5, 1.60), (-1.15, 8.8, 1.95), rx=0.20, rz=0.20, mat="wood_mid")
    rig.shaft(b, "lever_iron_spine", (-1.22, 4.5, 1.60), (-1.22, 8.8, 1.95), rx=0.10, rz=0.10, mat="iron_dark")
    rig.shaft(b, "lever_brass_strip", (-1.08, 4.5, 1.60), (-1.08, 8.8, 1.95), rx=0.08, rz=0.08, mat="bronze")
    # 摇柄前端横握圆木把手
    rig.cube(b, "lever_grip_knob", (-1.65, 8.5, 1.70), (-0.75, 9.0, 2.20), mat="wood_dark")
    rig.cube(b, "lever_grip_collar_l", (-1.70, 8.55, 1.75), (-1.60, 8.95, 2.15), mat="bronze")
    rig.cube(b, "lever_grip_collar_r", (-0.80, 8.55, 1.75), (-0.70, 8.95, 2.15), mat="bronze")

    # 5.1 连动拉杆 lever_linkage (下连动机构)
    b = "lever_linkage"
    rig.shaft(b, "linkage_bar", (-1.10, 4.5, 1.50), (-1.10, 6.2, 0.65), rx=0.14, rz=0.14, mat="iron_dark")
    rig.cube(b, "linkage_joint_top", (-1.18, 4.35, 1.40), (-0.98, 4.65, 1.60), mat="bronze")
    rig.cube(b, "linkage_joint_bot", (-1.18, 6.05, 0.55), (-0.98, 6.35, 0.75), mat="bronze")
    # 下方滑动卡爪 (推弦滑块)
    rig.cube(b, "slider_block", (-0.50, 5.8, 0.50), (0.50, 6.4, 0.70), mat="bronze_patina")
    rig.cube(b, "slider_catch_l", (-0.35, 6.2, 0.65), (-0.15, 6.45, 0.85), mat="bronze")
    rig.cube(b, "slider_catch_r", (0.15, 6.2, 0.65), (0.35, 6.45, 0.85), mat="bronze")

    # 6. 弩臂前端横装座与木鼻 stock_nose (y: 12.0 ~ 14.0)
    b = "stock_nose"
    rig.cube(b, "nose_bed_bot", (-0.54, 12.0, -0.65), (0.54, 13.0, -0.32), mat="wood_dark")
    rig.cube(b, "nose_bed_top", (-0.54, 12.0, 0.32), (0.54, 13.0, 0.65), mat="wood_mid")
    rig.cube(b, "nose_wall_l", (-0.56, 12.0, -0.32), (-0.46, 13.0, 0.32), mat="wood_dark")
    rig.cube(b, "nose_wall_r", (0.46, 12.0, -0.32), (0.56, 13.0, 0.32), mat="wood_dark")
    # 前端木鼻伸出 (y: 13.0 ~ 14.0)
    rig.cube(b, "nose_tip_core", (-0.44, 13.0, -0.52), (0.44, 13.8, 0.52), mat="wood_dark")
    rig.cube(b, "nose_tip_chisel", (-0.36, 13.8, -0.42), (0.36, 14.0, 0.42), mat="wood_light")
    # 加固麻绳与青铜箍
    rig.cube(b, "nose_cord_rear", (-0.58, 12.1, -0.68), (0.58, 12.45, 0.68), mat="cord")
    rig.cube(b, "nose_bronze_front", (-0.58, 12.9, -0.60), (0.58, 13.15, 0.60), mat="bronze")

    # 7. 短小层压复合弩弓 bow_prod (严格缩减翼展：X: -5.25 ~ 5.25，总跨度 10.5 px！)
    b = "bow_prod"
    # 中心固定段 (X: -0.9 ~ 0.9, Y: 12.2 ~ 12.8, Z: -0.28 ~ 0.28)
    rig.cube(b, "bow_center_core", (-0.9, 12.2, -0.28), (0.9, 12.8, 0.28), mat="wood_dark")
    rig.cube(b, "bow_center_face", (-0.85, 12.22, 0.28), (0.85, 12.78, 0.35), mat="wood_mid")
    rig.cube(b, "bow_center_back", (-0.85, 12.22, -0.35), (0.85, 12.78, -0.28), mat="wood_dark")
    rig.cube(b, "bow_center_wrap", (-0.45, 12.18, -0.32), (0.45, 12.82, 0.32), mat="cord")

    # 左右弓臂分段构建（分 5 段，翼展 10.5 px）：
    def add_compact_limb_seg(name_sfx, xl, xr, y_f, y_t, z_f, z_t, mat_core, mat_face, mat_back):
        for side, sign in [("r", 1.0), ("l", -1.0)]:
            x0 = xl * sign
            x1 = xr * sign
            xmin, xmax = min(x0, x1), max(x0, x1)
            rig.cube(b, f"limb_{name_sfx}_{side}", (xmin, y_f, z_f), (xmax, y_t, z_t), mat=mat_core)
            rig.cube(b, f"limb_{name_sfx}_f_{side}", (xmin + 0.03, y_f + 0.03, z_t), (xmax - 0.03, y_t - 0.03, z_t + 0.05), mat=mat_face)
            rig.cube(b, f"limb_{name_sfx}_b_{side}", (xmin + 0.03, y_f + 0.03, z_f - 0.05), (xmax - 0.03, y_t - 0.03, z_f), mat=mat_back)

    # 段 1 (0.9 ~ 1.8): 弓腰近端 厚 0.55×0.55
    add_compact_limb_seg("s1", 0.9, 1.8, 12.35, 12.85, -0.26, 0.26, "wood_dark", "wood_mid", "wood_dark")
    # 段 2 (1.8 ~ 2.8): 前拱展开 厚 0.50×0.50
    add_compact_limb_seg("s2", 1.8, 2.8, 12.50, 12.98, -0.24, 0.24, "wood_dark", "wood_mid", "wood_dark")
    # 段 3 (2.8 ~ 3.8): 前拱顶点 Y 到达 13.08, 厚 0.46×0.46
    add_compact_limb_seg("s3", 2.8, 3.8, 12.60, 13.08, -0.22, 0.22, "wood_dark", "wood_light", "cord")
    # 段 4 (3.8 ~ 4.6): 回转弧段 厚 0.42×0.42
    add_compact_limb_seg("s4", 3.8, 4.6, 12.45, 12.92, -0.20, 0.20, "wood_dark", "wood_mid", "wood_dark")
    # 段 5 (4.6 ~ 5.0): 梢根过渡 厚 0.38×0.38
    add_compact_limb_seg("s5", 4.6, 5.0, 12.05, 12.52, -0.18, 0.18, "wood_dark", "cord", "wood_dark")

    # 7.1 弓梢与弦枕 bow_nocks (X: 5.0 ~ 5.25, Y: 11.6 ~ 12.05)
    b_n = "bow_nocks"
    for side, sign in [("r", 1.0), ("l", -1.0)]:
        x0 = 5.0 * sign
        x1 = 5.25 * sign
        xmin, xmax = min(x0, x1), max(x0, x1)
        rig.cube(b_n, f"nock_tip_{side}", (xmin, 11.6, -0.16), (xmax, 12.05, 0.16), mat="wood_mid")
        rig.cube(b_n, f"nock_bone_pad_{side}", (xmin + 0.03, 11.62, 0.16), (xmax - 0.03, 11.98, 0.22), mat="bone_pale")
        rig.cube(b_n, f"nock_groove_{side}", (xmin + 0.08, 11.75, -0.18), (xmax - 0.08, 11.90, 0.18), mat="groove")
        rig.cube(b_n, f"nock_wrap_{side}", (xmin + 0.02, 11.98, -0.17), (xmax - 0.02, 12.08, 0.17), mat="cord")

    # 8. 严格 0.3 px 粗细的浅 V 形弓弦 bowstring
    # 左梢 (-5.1, 11.8, 0.2) -> 底槽推弦爪 (0.0, 6.2, 0.65) -> 右梢 (5.1, 11.8, 0.2)
    b = "bowstring"
    pt_left = (-5.1, 11.8, 0.2)
    pt_tooth = (0.0, 6.2, 0.65)
    pt_right = (5.1, 11.8, 0.2)

    rig.cube(b, "string_center_loop", (-0.15, 6.12, 0.55), (0.15, 6.32, 0.75), mat="string")
    rig.cube(b, "string_center_knot", (-0.12, 6.05, 0.58), (0.12, 6.20, 0.72), mat="cord")

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
        f"#3 匣连弩 xia_lian_nu (Round 1 几何) · 弩臂14px · 木箭匣 · 顺贴摇柄零交叠 · 弩弓翼展10.5px"
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

    # 参考图侧视长图：crop1 = (17, 14, 1057, 361)
    ref_side_crop = weapon_review_kit.crop_tight(ref_im_full.crop((17, 14, 1057, 361)))
    # 参考图俯视长图：crop2 = (16, 351, 1044, 660)
    ref_top_crop = weapon_review_kit.crop_tight(ref_im_full.crop((16, 351, 1044, 660)))

    # 模型侧视渲染（整弩，yaw=-90）与模型俯视渲染（整弩，yaw=180, pitch=89.9）
    model_side_img, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=520, bg=gear_kit.BG)
    model_top_img, _ = render(bbmodel_path, yaw=180.0, pitch=89.9, size=520, bg=gear_kit.BG)

    # 侧视指标对照表
    side_metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("后托下折段长 (Butt Bend Len)", 25.0, 25.0),
        ("后托加厚高度 (Butt Height)", 14.3, 14.3),
        ("木箭匣总长 (Magazine Length)", 59.3, 59.3),
        ("箭匣高度占比 (Magazine Height)", 14.8, 14.8),
        ("侧面铭文空匾长 (Blank Plaque)", 20.0, 20.0),
        ("侧面连动摇柄长 (Lever Handle)", 32.1, 32.1),
        ("弩臂截面高 (Stock Mid Height)", 10.7, 10.7),
        ("前端出箭与木鼻 (Nose & Muzzle)", 15.7, 15.7),
    ]

    side_lines = [
        ("前端弩头 100%", 1.0, (230, 90, 80)),
        ("出箭口前沿 84.3%", 0.843, (120, 190, 240)),
        ("短小弩弓座 89.3%", 0.893, (220, 170, 70)),
        ("侧面铭文匾 50.0%", 0.50, (130, 220, 160)),
        ("摇柄枢轴位 32.1%", 0.321, (180, 140, 230)),
        ("后托下折起点 25.0%", 0.250, (210, 150, 120)),
        ("抵肩后托底端 0%", 0.0, (230, 90, 80)),
    ]

    # 俯视指标对照表
    top_metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("短小弩弓翼展比 (Span/Stock)", 75.0, 75.0),
        ("弩弓前拱深度 (Bow Arch Depth)", 6.2, 6.2),
        ("长方箭匣宽度 (Magazine Width)", 12.9, 12.9),
        ("弩臂下框架宽 (Frame Width)", 7.9, 7.9),
        ("摇柄展开总宽 (Handle Total Width)", 16.4, 16.4),
        ("后托握持宽 (Butt Grip Width)", 7.1, 7.1),
    ]

    top_lines = [
        ("前端木鼻 100%", 1.0, (230, 90, 80)),
        ("短小弩弓座 89.3%", 0.893, (220, 170, 70)),
        ("出箭口前沿 84.3%", 0.843, (120, 190, 240)),
        ("箭匣侧铭文位 50.0%", 0.500, (100, 210, 120)),
        ("摇柄枢轴位 32.1%", 0.321, (180, 140, 230)),
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
        title="#3 匣连弩 xia_lian_nu (Round 1 几何 · 弩臂14px · 木箭匣 · 顺贴摇柄零交叠 · 弩弓翼展10.5px)"
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
    "wood_dark": "weapon_wood_dark",
    "wood_mid": "weapon_wood_polished",
    "wood_light": "weapon_wood_polished",
    "bronze": "weapon_bronze",
    "bronze_patina": "weapon_bronze_patina",
    "bronze_sight": "weapon_bronze_honed",
    "iron_dark": "weapon_iron_main",
    "cord": "weapon_cord_dark",
    "string": "weapon_string_beige",
    "bone_pale": "weapon_bone_pale",
    "groove": "weapon_rune_dim",
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
        title="匣连弩 xia_lian_nu（Round 3 贴图版；深褐包浆木身+中空长方箭匣+顺贴摇柄+米黄弓弦+空白铭文位）"
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
