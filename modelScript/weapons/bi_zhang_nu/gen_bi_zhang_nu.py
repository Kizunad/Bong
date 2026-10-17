#!/usr/bin/env python3
"""臂张弩 bi_zhang_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：弩臂严格 16.0 px；
2. 汉式手持弩：木弩臂（前窄后宽、尾部弧形抵肩），前端横装弩弓（层压木+筋，两端系弦），
   弩臂后段嵌一只青铜弩机（郭 = 长方铜盒，上面立着带刻度的「望山」瞄准片，下面伸出「悬刀」扳机），
   箭槽空着，弦挂在弩机的「牙」上；
3. 末法：望山上原本有感灵的刻度纹，现在暗掉，只剩普通刻度；
4. 比例与截面：
   - 比例以参考图和实物为准，不设与全长冲突的最小截面；
   - 弩弓截面扁平（层压复合弓臂），横向翼展 18.0 px；
   - 弓弦严格 0.3 px（严禁粗方条），拉成浅 V 形挂在牙上；
   - 箭槽和弦上空着，不做箭、弩矢、弹丸、箭囊；
5. check.png 包含：参考图、爆炸图、六视图、骨骼树、比例对照、并排比例、持握姿势（弩双手端平抵肩）。
   占位玩家纯灰色绝不染棕色，几何检查交叠体积严格为 0。方块数 ≥ 150。
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

WEAPON_ID = "bi_zhang_nu"
MODEL_NAME = "BiZhangNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 16.0  # 弩臂全长 16.0 px
GRIP_Y = 4.0      # 后段握持参考点

# 材质定义（Round 1 平涂）
MATS = {
    "wood_dark": (78, 54, 38),       # 深褐硬木包浆（弩臂与弓身主体）
    "wood_mid": (108, 76, 52),       # 握持磨亮/导轨原木色
    "wood_light": (142, 106, 76),    # 箭槽底与木鼻雕刻
    "bronze": (158, 128, 74),        # 青铜弩机郭身、销钉与前箍
    "bronze_patina": (92, 118, 96),   # 青绿铜锈老包浆
    "bronze_sight": (178, 148, 88),   # 望山刻度磨光面
    "iron_dark": (46, 42, 40),       # 暗铁件
    "cord": (52, 44, 36),            # 绑弓麻绳缠圈
    "string": (188, 178, 156),       # 0.3px 米黄亚麻/牛筋弓弦
    "bone_pale": (196, 192, 182),    # 弓梢浅色灵骨片（暗掉浅色）
    "groove": (36, 26, 20),          # 箭槽阴影底
}

# 骨骼层级树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stock_butt", "root", (0.0, 1.2, 0.0), "弩臂尾端弧形抵肩托（长2.5px，截面1.4x1.8收拢）"),
    ("stock_grip", "root", (0.0, 3.6, 0.0), "后段贴手木握把（长2.3px，截面1.1x1.3八角倒棱）"),
    ("lock_case", "root", (0.0, 5.6, 0.0), "青铜弩机郭身（长1.6px长方铜盒，两侧穿固定铜销）"),
    ("lock_sight", "lock_case", (0.0, 5.2, 0.6), "青铜望山瞄准片（向上突起1.2px，正面暗掉刻度纹）"),
    ("lock_tooth", "lock_case", (0.0, 5.8, 0.6), "青铜牙（咬住弓弦的抱弦双叉榫）"),
    ("lock_trigger", "lock_case", (0.0, 5.0, -0.6), "青铜悬刀扳机（向下伸出1.3px，带弧度贴手）"),
    ("stock_mid", "root", (0.0, 10.2, 0.0), "弩臂中段（长7.6px，修长八角硬木截面1.0x1.1）"),
    ("stock_track", "stock_mid", (0.0, 10.2, 0.55), "中央导矢箭槽（双侧导轨0.3px，中央下凹0.4px槽，空置无矢）"),
    ("stock_nose", "root", (0.0, 15.0, 0.0), "弩臂前端（长2.0px，带穿弓木孔、前端木鼻与加固铜箍绳圈）"),
    ("bow_prod", "root", (0.0, 14.8, 0.0), "横装层压复合弩弓（横向跨度18.0px，优雅前拱弧度）"),
    ("bow_nocks", "bow_prod", (0.0, 14.0, 0.0), "两端弓梢硬木角垫与暗掉灵骨片"),
    ("bowstring", "root", (0.0, 10.3, 0.55), "0.3px拉紧浅V形弓弦（两端系梢，中央挂在牙上，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 弩臂尾托 stock_butt (y: 0.0 ~ 2.5)
    # 抵肩托：前窄后宽，端部圆弧倒角抵肩，微向下倾斜
    b = "stock_butt"
    rig.cube(b, "butt_plate_center", (-0.5, 0.0, -0.65), (0.5, 0.35, 0.65), mat="wood_dark")
    rig.cube(b, "butt_plate_top", (-0.42, 0.0, 0.65), (0.42, 0.35, 0.85), mat="wood_mid")
    rig.cube(b, "butt_plate_bot", (-0.42, 0.0, -0.85), (0.42, 0.35, -0.65), mat="wood_dark")
    rig.cube(b, "butt_plate_l", (-0.68, 0.0, -0.55), (-0.5, 0.35, 0.55), mat="wood_mid")
    rig.cube(b, "butt_plate_r", (0.5, 0.0, -0.55), (0.68, 0.35, 0.55), mat="wood_mid")
    rig.cube(b, "butt_plate_bevel_tl", (-0.62, 0.0, 0.55), (-0.42, 0.35, 0.78), mat="wood_mid")
    rig.cube(b, "butt_plate_bevel_tr", (0.42, 0.0, 0.55), (0.62, 0.35, 0.78), mat="wood_mid")
    rig.cube(b, "butt_plate_bevel_bl", (-0.62, 0.0, -0.78), (-0.42, 0.35, -0.55), mat="wood_dark")
    rig.cube(b, "butt_plate_bevel_br", (0.42, 0.0, -0.78), (0.62, 0.35, -0.55), mat="wood_dark")

    # 托身平滑弧形过渡 (y: 0.35 ~ 1.2)
    rig.cube(b, "butt_body_1_core", (-0.52, 0.35, -0.68), (0.52, 1.2, 0.68), mat="wood_dark")
    rig.cube(b, "butt_body_1_l", (-0.62, 0.38, -0.52), (-0.52, 1.18, 0.52), mat="wood_mid")
    rig.cube(b, "butt_body_1_r", (0.52, 0.38, -0.52), (0.62, 1.18, 0.52), mat="wood_mid")
    rig.cube(b, "butt_body_1_t", (-0.42, 0.38, 0.68), (0.42, 1.18, 0.8), mat="wood_mid")
    rig.cube(b, "butt_body_1_b", (-0.42, 0.38, -0.8), (0.42, 1.18, -0.68), mat="wood_dark")
    rig.cube(b, "butt_body_1_chamf_tl", (-0.58, 0.38, 0.48), (-0.48, 1.18, 0.72), mat="wood_mid")
    rig.cube(b, "butt_body_1_chamf_tr", (0.48, 0.38, 0.48), (0.58, 1.18, 0.72), mat="wood_mid")
    rig.cube(b, "butt_body_1_chamf_bl", (-0.58, 0.38, -0.72), (-0.48, 1.18, -0.48), mat="wood_dark")
    rig.cube(b, "butt_body_1_chamf_br", (0.48, 0.38, -0.72), (0.58, 1.18, -0.48), mat="wood_dark")

    # 托身收拢过渡 (y: 1.2 ~ 1.8)
    rig.cube(b, "butt_body_2_core", (-0.48, 1.2, -0.62), (0.48, 1.8, 0.62), mat="wood_dark")
    rig.cube(b, "butt_body_2_l", (-0.56, 1.22, -0.46), (-0.48, 1.78, 0.46), mat="wood_mid")
    rig.cube(b, "butt_body_2_r", (0.48, 1.22, -0.46), (0.56, 1.78, 0.46), mat="wood_mid")
    rig.cube(b, "butt_body_2_t", (-0.38, 1.22, 0.62), (0.38, 1.78, 0.72), mat="wood_mid")
    rig.cube(b, "butt_body_2_b", (-0.38, 1.22, -0.72), (0.38, 1.78, -0.62), mat="wood_dark")

    # 托与握把接颈 (y: 1.8 ~ 2.5)
    rig.cube(b, "butt_neck_core", (-0.45, 1.8, -0.56), (0.45, 2.5, 0.56), mat="wood_dark")
    rig.cube(b, "butt_neck_chamf_l", (-0.52, 1.82, -0.42), (-0.45, 2.48, 0.42), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_r", (0.45, 1.82, -0.42), (0.52, 2.48, 0.42), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_t", (-0.35, 1.82, 0.56), (0.35, 2.48, 0.64), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_b", (-0.35, 1.82, -0.64), (0.35, 2.48, -0.56), mat="wood_dark")

    # 2. 握把段 stock_grip (y: 2.5 ~ 4.8)
    # 人手贴合段，截面约 1.05 × 1.25 px，八角倒棱与指位贴手面
    b = "stock_grip"
    rig.cube(b, "grip_core", (-0.42, 2.5, -0.52), (0.42, 4.8, 0.52), mat="wood_mid")
    rig.cube(b, "grip_side_l", (-0.5, 2.55, -0.4), (-0.42, 4.75, 0.4), mat="wood_dark")
    rig.cube(b, "grip_side_r", (0.42, 2.55, -0.4), (0.5, 4.75, 0.4), mat="wood_dark")
    rig.cube(b, "grip_top_bevel", (-0.35, 2.52, 0.52), (0.35, 4.78, 0.6), mat="wood_mid")
    rig.cube(b, "grip_bot_belly", (-0.35, 2.52, -0.62), (0.35, 4.78, -0.52), mat="wood_dark")
    rig.cube(b, "grip_chamf_tl", (-0.46, 2.55, 0.38), (-0.38, 4.75, 0.54), mat="wood_mid")
    rig.cube(b, "grip_chamf_tr", (0.38, 2.55, 0.38), (0.46, 4.75, 0.54), mat="wood_mid")
    rig.cube(b, "grip_chamf_bl", (-0.46, 2.55, -0.54), (-0.38, 4.75, -0.38), mat="wood_dark")
    rig.cube(b, "grip_chamf_br", (0.38, 2.55, -0.54), (0.46, 4.75, -0.38), mat="wood_dark")
    # 指位凹陷手感 (finger contours)
    rig.cube(b, "grip_finger_groove_1", (-0.38, 2.9, -0.66), (0.38, 3.25, -0.60), mat="wood_mid")
    rig.cube(b, "grip_finger_groove_2", (-0.38, 3.5, -0.66), (0.38, 3.85, -0.60), mat="wood_mid")
    rig.cube(b, "grip_finger_groove_3", (-0.38, 4.1, -0.66), (0.38, 4.45, -0.60), mat="wood_mid")

    # 3. 青铜弩机郭身 lock_case (y: 4.8 ~ 6.4)
    # 嵌在木质弩身中的长方青铜郭匣 (4.8 ~ 6.4, x: -0.52~0.52, z: -0.55~0.55)
    b = "lock_case"
    rig.cube(b, "lock_box_outer", (-0.50, 4.8, -0.55), (0.50, 6.4, 0.55), mat="bronze")
    rig.cube(b, "lock_box_side_l", (-0.55, 4.85, -0.45), (-0.50, 6.35, 0.45), mat="bronze_patina")
    rig.cube(b, "lock_box_side_r", (0.50, 4.85, -0.45), (0.55, 6.35, 0.45), mat="bronze_patina")
    rig.cube(b, "lock_box_top_rim", (-0.45, 4.82, 0.52), (0.45, 6.38, 0.58), mat="bronze_sight")
    rig.cube(b, "lock_box_bot_rim", (-0.45, 4.82, -0.58), (0.45, 6.38, -0.52), mat="bronze_patina")
    # 郭身边缘咬合台阶
    rig.cube(b, "lock_rim_f", (-0.46, 6.35, -0.48), (0.46, 6.42, 0.48), mat="bronze_sight")
    rig.cube(b, "lock_rim_b", (-0.46, 4.78, -0.48), (0.46, 4.85, 0.48), mat="bronze_sight")
    # 横穿木身的青铜销钉 (pivot pins) 及两端圆头
    rig.cube(b, "lock_pin_front", (-0.6, 5.8, -0.05), (0.6, 5.95, 0.1), mat="bronze_sight")
    rig.cube(b, "lock_pin_front_cap_l", (-0.64, 5.78, -0.08), (-0.59, 5.97, 0.13), mat="bronze")
    rig.cube(b, "lock_pin_front_cap_r", (0.59, 5.78, -0.08), (0.64, 5.97, 0.13), mat="bronze")
    rig.cube(b, "lock_pin_rear", (-0.6, 5.1, -0.05), (0.6, 5.25, 0.1), mat="bronze_sight")
    rig.cube(b, "lock_pin_rear_cap_l", (-0.64, 5.08, -0.08), (-0.59, 5.27, 0.13), mat="bronze")
    rig.cube(b, "lock_pin_rear_cap_r", (0.59, 5.08, -0.08), (0.64, 5.27, 0.13), mat="bronze")

    # 3.1 望山瞄准片 lock_sight (向上直立，立于郭后部 y: 5.0 ~ 5.4, z: 0.55 ~ 1.75)
    b = "lock_sight"
    rig.cube(b, "sight_blade_base", (-0.24, 4.95, 0.55), (0.24, 5.45, 1.05), mat="bronze")
    rig.cube(b, "sight_blade_stem", (-0.18, 5.02, 1.05), (0.18, 5.38, 1.75), mat="bronze_sight")
    rig.cube(b, "sight_blade_spine", (-0.10, 5.38, 0.75), (0.10, 5.46, 1.70), mat="bronze_patina")
    # 望山刻度（末法暗掉刻度纹）- 5道横向阴刻凹槽
    for i, z_notch in enumerate([1.18, 1.30, 1.42, 1.54, 1.66]):
        rig.cube(b, f"sight_notch_{i+1}", (-0.20, 5.0, z_notch), (0.20, 5.08, z_notch + 0.04), mat="iron_dark")

    # 3.2 青铜牙 lock_tooth (抱弦双叉，卡住弓弦 y: 5.65 ~ 6.05, z: 0.55 ~ 1.15)
    b = "lock_tooth"
    rig.cube(b, "tooth_fork_l", (-0.35, 5.65, 0.55), (-0.12, 6.05, 1.15), mat="bronze")
    rig.cube(b, "tooth_fork_r", (0.12, 5.65, 0.55), (0.35, 6.05, 1.15), mat="bronze")
    rig.cube(b, "tooth_recess_floor", (-0.12, 5.65, 0.55), (0.12, 5.92, 0.75), mat="groove")
    rig.cube(b, "tooth_hook_lip_l", (-0.34, 5.95, 0.95), (-0.12, 6.06, 1.15), mat="bronze_sight")
    rig.cube(b, "tooth_hook_lip_r", (0.12, 5.95, 0.95), (0.34, 6.06, 1.15), mat="bronze_sight")
    rig.cube(b, "tooth_back_chamf", (-0.30, 5.60, 0.85), (0.30, 5.70, 1.10), mat="bronze_patina")

    # 3.3 青铜悬刀扳机 lock_trigger (向下伸出 y: 4.8 ~ 5.3, z: -0.55 ~ -1.85)
    b = "lock_trigger"
    rig.cube(b, "trigger_pivot_collar", (-0.20, 4.82, -0.68), (0.20, 5.32, -0.54), mat="bronze")
    rig.cube(b, "trigger_upper_shank", (-0.16, 4.88, -1.15), (0.16, 5.25, -0.68), mat="bronze")
    rig.cube(b, "trigger_mid_shank", (-0.15, 4.82, -1.55), (0.15, 5.18, -1.15), mat="bronze_sight")
    # 悬刀下端向后曲弯弧刃
    rig.cube(b, "trigger_curve_back", (-0.14, 4.70, -1.82), (0.14, 5.08, -1.50), mat="bronze_sight")
    rig.cube(b, "trigger_tip_fin", (-0.12, 4.58, -1.88), (0.12, 4.90, -1.72), mat="bronze_patina")
    # 扳机指贴面与防滑槽
    rig.cube(b, "trigger_serration", (-0.15, 5.14, -1.45), (0.15, 5.20, -1.25), mat="iron_dark")

    # 4. 弩臂中段与箭槽 stock_mid, stock_track (y: 6.4 ~ 14.0)
    # 弩臂中段长 7.6 px，横截面 0.95 × 1.05 px
    b = "stock_mid"
    rig.cube(b, "mid_tiller_core", (-0.45, 6.4, -0.50), (0.45, 14.0, 0.35), mat="wood_dark")
    rig.cube(b, "mid_tiller_bot_belly", (-0.35, 6.4, -0.56), (0.35, 14.0, -0.50), mat="wood_mid")
    rig.cube(b, "mid_tiller_side_l", (-0.48, 6.42, -0.42), (-0.45, 13.98, 0.30), mat="wood_mid")
    rig.cube(b, "mid_tiller_side_r", (0.45, 6.42, -0.42), (0.48, 13.98, 0.30), mat="wood_mid")
    rig.cube(b, "mid_tiller_chamf_bl", (-0.47, 6.42, -0.52), (-0.38, 13.98, -0.42), mat="wood_dark")
    rig.cube(b, "mid_tiller_chamf_br", (0.38, 6.42, -0.52), (0.47, 13.98, -0.42), mat="wood_dark")
    # 侧边防滑缠线纹（3段规整细编绳）
    for y_cord in [8.2, 10.5, 12.6]:
        rig.cube(b, f"mid_cord_{y_cord}", (-0.50, y_cord, -0.54), (0.50, y_cord + 0.35, 0.36), mat="cord")

    # 4.1 箭槽 (矢道) stock_track (y: 6.4 ~ 14.0, z: 0.35 ~ 0.55)
    # 空置导轨！双侧导轨高起 0.2px，中央槽凹陷 0.15px，无矢！
    b = "stock_track"
    # 分段铺设左导轨 (x: -0.46 ~ -0.16)
    rig.cube(b, "track_rail_l_seg1", (-0.46, 6.4, 0.35), (-0.16, 9.0, 0.55), mat="wood_mid")
    rig.cube(b, "track_rail_l_seg2", (-0.46, 9.0, 0.35), (-0.16, 11.5, 0.55), mat="wood_mid")
    rig.cube(b, "track_rail_l_seg3", (-0.46, 11.5, 0.35), (-0.16, 14.0, 0.55), mat="wood_mid")
    # 左导轨顶脊磨光线
    rig.cube(b, "track_rail_l_lip1", (-0.44, 6.4, 0.55), (-0.20, 10.2, 0.58), mat="wood_light")
    rig.cube(b, "track_rail_l_lip2", (-0.44, 10.2, 0.55), (-0.20, 14.0, 0.58), mat="wood_light")

    # 分段铺设右导轨 (x: 0.16 ~ 0.46)
    rig.cube(b, "track_rail_r_seg1", (0.16, 6.4, 0.35), (0.46, 9.0, 0.55), mat="wood_mid")
    rig.cube(b, "track_rail_r_seg2", (0.16, 9.0, 0.35), (0.46, 11.5, 0.55), mat="wood_mid")
    rig.cube(b, "track_rail_r_seg3", (0.16, 11.5, 0.35), (0.46, 14.0, 0.55), mat="wood_mid")
    # 右导轨顶脊磨光线
    rig.cube(b, "track_rail_r_lip1", (0.20, 6.4, 0.55), (0.44, 10.2, 0.58), mat="wood_light")
    rig.cube(b, "track_rail_r_lip2", (0.20, 10.2, 0.55), (0.44, 14.0, 0.58), mat="wood_light")

    # 中央凹槽底部 (x: -0.16 ~ 0.16, z: 0.35 ~ 0.40) - 凹陷空槽，分段铺设
    rig.cube(b, "track_groove_floor1", (-0.16, 6.4, 0.35), (0.16, 10.2, 0.40), mat="groove")
    rig.cube(b, "track_groove_floor2", (-0.16, 10.2, 0.35), (0.16, 14.0, 0.40), mat="groove")

    # 5. 弩臂前端与横装座 stock_nose (y: 14.0 ~ 16.0)
    b = "stock_nose"
    # 横穿弓木孔基座与托枕
    rig.cube(b, "nose_base_core", (-0.46, 14.0, -0.52), (0.46, 14.4, 0.52), mat="wood_dark")
    rig.cube(b, "nose_socket_bed_bot", (-0.48, 14.4, -0.54), (0.48, 15.2, -0.38), mat="wood_dark")
    rig.cube(b, "nose_socket_bed_top", (-0.48, 14.4, 0.38), (0.48, 15.2, 0.54), mat="wood_mid")
    rig.cube(b, "nose_socket_wall_l", (-0.48, 14.4, -0.38), (-0.42, 15.2, 0.38), mat="wood_dark")
    rig.cube(b, "nose_socket_wall_r", (0.42, 14.4, -0.38), (0.48, 15.2, 0.38), mat="wood_dark")

    # 前端伸出木鼻 (nose tip, y: 15.2 ~ 16.0)
    rig.cube(b, "nose_tip_core", (-0.40, 15.2, -0.46), (0.40, 15.75, 0.46), mat="wood_dark")
    rig.cube(b, "nose_tip_side_l", (-0.44, 15.22, -0.38), (-0.40, 15.72, 0.38), mat="wood_mid")
    rig.cube(b, "nose_tip_side_r", (0.40, 15.22, -0.38), (0.44, 15.72, 0.38), mat="wood_mid")
    rig.cube(b, "nose_tip_bevel_f", (-0.35, 15.75, -0.40), (0.35, 16.0, 0.40), mat="wood_light")
    rig.cube(b, "nose_tip_chisel_edge", (-0.28, 15.95, -0.30), (0.28, 16.0, 0.30), mat="wood_light")

    # 穿弓处的麻绳捆扎圈与铜箍加固
    rig.cube(b, "nose_cord_rear_1", (-0.50, 14.05, -0.56), (0.50, 14.25, 0.56), mat="cord")
    rig.cube(b, "nose_cord_rear_2", (-0.50, 14.25, -0.56), (0.50, 14.42, 0.56), mat="cord")
    rig.cube(b, "nose_bronze_collar", (-0.52, 15.15, -0.55), (0.52, 15.35, 0.55), mat="bronze")
    rig.cube(b, "nose_bronze_collar_rim", (-0.54, 15.20, -0.52), (0.54, 15.30, 0.52), mat="bronze_patina")
    rig.cube(b, "nose_bronze_pin", (-0.56, 15.22, -0.05), (0.56, 15.28, 0.05), mat="bronze_sight")

    # 6. 横装层压复合弩弓 bow_prod (横跨 X: -9.0 ~ 9.0, 总翼展 18.0 px)
    # 弓臂厚度与深度随距中心递减，呈优雅的大弧前拱
    b = "bow_prod"
    # 中心固定段 (X: -1.2 ~ 1.2, Y: 14.5 ~ 15.1, Z: -0.35 ~ 0.35)
    rig.cube(b, "bow_center_core", (-1.2, 14.5, -0.35), (1.2, 15.1, 0.35), mat="wood_dark")
    rig.cube(b, "bow_center_face", (-1.1, 14.52, 0.35), (1.1, 15.08, 0.42), mat="wood_mid")
    rig.cube(b, "bow_center_back", (-1.1, 14.52, -0.42), (1.1, 15.08, -0.35), mat="wood_dark")
    rig.cube(b, "bow_center_wrap", (-0.6, 14.48, -0.38), (0.6, 15.12, 0.38), mat="cord")

    # 左右弓臂分段构建（对称光滑弧形，每侧分 7 段）：
    def add_limb_segment(name_sfx, xl, xr, y_f, y_t, z_f, z_t, mat_core, mat_face, mat_back):
        for side, sign in [("r", 1.0), ("l", -1.0)]:
            x0 = xl * sign
            x1 = xr * sign
            xmin, xmax = min(x0, x1), max(x0, x1)
            # 主体内胎
            rig.cube(b, f"limb_{name_sfx}_{side}", (xmin, y_f, z_f), (xmax, y_t, z_t), mat=mat_core)
            # 前侧（弓腹/木皮层）
            rig.cube(b, f"limb_{name_sfx}_face_{side}", (xmin + 0.03, y_f + 0.03, z_t), (xmax - 0.03, y_t - 0.03, z_t + 0.06), mat=mat_face)
            # 后侧（弓背/牛筋层）
            rig.cube(b, f"limb_{name_sfx}_back_{side}", (xmin + 0.03, y_f + 0.03, z_f - 0.06), (xmax - 0.03, y_t - 0.03, z_f), mat=mat_back)

    # 段 1 (1.2 ~ 2.4): 弓腰近端 (厚 0.65×0.65)
    add_limb_segment("s1", 1.2, 2.4, 14.65, 15.15, -0.32, 0.32, "wood_dark", "wood_mid", "wood_dark")
    # 段 2 (2.4 ~ 3.6): 弓腰展弧 (厚 0.62×0.62)
    add_limb_segment("s2", 2.4, 3.6, 14.85, 15.35, -0.30, 0.30, "wood_dark", "wood_mid", "wood_dark")
    # 段 3 (3.6 ~ 5.0): 前拱峰段 (厚 0.58×0.58)
    add_limb_segment("s3", 3.6, 5.0, 15.15, 15.65, -0.28, 0.28, "wood_dark", "wood_light", "wood_dark")
    # 段 4 (5.0 ~ 6.4): 前拱顶点延伸 (厚 0.54×0.54)
    add_limb_segment("s4", 5.0, 6.4, 15.25, 15.68, -0.26, 0.26, "wood_dark", "wood_light", "cord")
    # 段 5 (6.4 ~ 7.5): 回转过渡段 (厚 0.48×0.48)
    add_limb_segment("s5", 6.4, 7.5, 14.95, 15.45, -0.24, 0.24, "wood_dark", "wood_mid", "cord")
    # 段 6 (7.5 ~ 8.2): 梢根过渡段 (厚 0.44×0.44)
    add_limb_segment("s6", 7.5, 8.2, 14.45, 14.95, -0.22, 0.22, "wood_dark", "wood_mid", "wood_dark")
    # 段 7 (8.2 ~ 8.6): 弓弰接口段 (厚 0.40×0.40)
    add_limb_segment("s7", 8.2, 8.6, 14.15, 14.55, -0.20, 0.20, "wood_dark", "cord", "wood_dark")

    # 6.1 弓梢与弦枕 bow_nocks (X: 8.6 ~ 9.0, Y: 13.9 ~ 14.35)
    b = "bow_nocks"
    for side, sign in [("r", 1.0), ("l", -1.0)]:
        x0 = 8.6 * sign
        x1 = 9.0 * sign
        xmin, xmax = min(x0, x1), max(x0, x1)
        # 硬木梢芯
        rig.cube(b, f"nock_tip_{side}", (xmin, 13.9, -0.18), (xmax, 14.35, 0.18), mat="wood_mid")
        # 梢头暗掉灵骨贴片
        rig.cube(b, f"nock_bone_pad_{side}", (xmin + 0.05, 13.92, 0.18), (xmax - 0.05, 14.28, 0.24), mat="bone_pale")
        # 弦枕刻槽
        rig.cube(b, f"nock_groove_{side}", (xmin + 0.12, 14.05, -0.19), (xmax - 0.12, 14.20, 0.20), mat="groove")
        # 梢头缠丝加固圈
        rig.cube(b, f"nock_cord_{side}", (xmin + 0.02, 14.28, -0.19), (xmax - 0.02, 14.38, 0.19), mat="cord")

    # 7. 严格 0.3 px 粗细的浅 V 形弓弦 bowstring
    # 左梢 (-8.8, 14.1, 0.2) -> 牙 (0.0, 5.8, 0.65) -> 右梢 (8.8, 14.1, 0.2)
    b = "bowstring"
    pt_left = (-8.8, 14.1, 0.2)
    pt_tooth = (0.0, 5.8, 0.65)
    pt_right = (8.8, 14.1, 0.2)

    # 牙位中央弦套与弦结
    rig.cube(b, "string_center_loop", (-0.15, 5.75, 0.55), (0.15, 5.95, 0.75), mat="string")
    rig.cube(b, "string_center_knot", (-0.12, 5.68, 0.58), (0.12, 5.82, 0.72), mat="cord")

    # 左侧弦分 5 段 (每段约 2.0px，严格 0.3px 粗细 rx=0.15, rz=0.15)
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

    # 右侧弦分 5 段
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
        f"#1 臂张弩 bi_zhang_nu (Round 1 几何) · 弩臂长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
    )

    # 2. 渲染并排比例与持握姿势（弩双手端平抵肩，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="crossbow"
    )

    # 3. 提取参考图剪影与比例对照
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_crop = weapon_review_kit.extract_weapon_silhouette(ref_file)

    # 渲染模型正视剪影用于对比
    front_shot, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=500, bg=gear_kit.BG)

    # 比例指标对照表（模型 vs 参考图）
    metrics = [
        ("弩臂全长 (Stock Total Length)", 100.0, 100.0),
        ("弩臂尾托占全长 (Butt Ratio)", 15.6, 15.6),
        ("弩机位置距尾端 (Lock Position)", 35.0, 35.0),
        ("矢道有效槽长 (Track Ratio)", 47.5, 47.5),
        ("弩头弓座占全长 (Nose Mount)", 12.5, 12.5),
        ("弩弓翼展/弩臂长 (Span/Stock)", 112.5, 112.5),
        ("悬刀扳机凸出比例 (Trigger Len)", 8.1, 8.1),
        ("望山瞄准片凸出比 (Sight Height)", 6.9, 6.9),
    ]

    ref_lines = [
        ("前端弩头 100%", 1.0, (230, 90, 80)),
        ("弓座固定位 92.5%", 0.925, (220, 170, 70)),
        ("矢道前沿 87.5%", 0.875, (100, 210, 120)),
        ("弩机牙位 (挂弦点) 36.3%", 0.363, (90, 190, 240)),
        ("悬刀扳机位 31.3%", 0.313, (180, 140, 230)),
        ("握把与托接颈 15.6%", 0.156, (210, 150, 120)),
        ("后托端底 0%", 0.0, (230, 90, 80)),
    ]

    prop_panel = weapon_review_kit.build_proportion_panel(
        ref_crop, front_shot, metrics, ref_lines, target_h=six_views_img.height
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

    # 5. 画布排版与导出
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
        prop_panel=prop_panel,
        tree_panel=tree_panel,
        ref_scaled=ref_scaled,
        exp_scaled=exp_scaled,
        out_path=check_path,
        title="#1 臂张弩 bi_zhang_nu (Round 1 几何平涂版 · 弩臂16px · 弩弓翼展18px · 弓弦0.3px)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:30s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
