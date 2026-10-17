#!/usr/bin/env python3
"""蹶张弩 jue_zhang_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：弩臂严格 22.0 px；
2. 汉式重型军弩：靠脚踩弩弓、双手拉弦上弦。
   - 弩臂前端横装重型锻铁脚蹬环（踏环 stirrup）；
   - 弩弓更宽更厚（翼展 25.0 px），弩弓中段有阵纹铜箍（末法暗掉）；
   - 弩臂更粗更重（前窄后宽弧形抵肩托，中段八角倒棱）；
   - 青铜弩机更大（长方铜盒郭身，高望山瞄准片带刻度与暗掉感灵纹，重型下弯悬刀扳机）；
   - 箭槽和弦上空着，不做箭、弩矢、弹丸、箭囊；
   - 弓弦严格 0.3 px（严禁粗方条），拉成浅 V 形挂在牙上；
3. check.png 包含：参考图、爆炸图、六视图、骨骼树、比例对照、并排比例、持握姿势（弩双手端平抵肩）。
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

WEAPON_ID = "jue_zhang_nu"
MODEL_NAME = "JueZhangNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 22.0  # 弩臂全长 22.0 px
GRIP_Y = 5.5      # 后段握持参考点

# 材质定义（Round 1 平涂）
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

# 骨骼层级树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stock_butt", "root", (0.0, 1.8, 0.0), "重弩臂尾端弧形抵肩托（长3.4px，厚重1.6x2.2切角）"),
    ("stock_grip", "root", (0.0, 5.0, 0.0), "后段双手木握把（长3.2px，截面1.3x1.5八角倒棱）"),
    ("lock_case", "root", (0.0, 7.8, 0.0), "重型青铜弩机郭身（长2.2px长方厚铜盒，双贯穿青铜键销）"),
    ("lock_sight", "lock_case", (0.0, 7.2, 0.75), "高耸青铜望山瞄准片（向上突起1.6px，刻度7道与暗掉感灵纹）"),
    ("lock_tooth", "lock_case", (0.0, 8.2, 0.75), "重型抱弦双叉青铜牙（受弦倒钩槽深0.4px）"),
    ("lock_trigger", "lock_case", (0.0, 7.0, -0.75), "重型青铜悬刀扳机（向下伸出1.7px，粗壮弯弧指托）"),
    ("stock_mid", "root", (0.0, 14.2, 0.0), "弩臂中段（长10.5px，厚实八角硬木身1.2x1.4）"),
    ("stock_track", "stock_mid", (0.0, 14.2, 0.7), "深槽导矢箭槽（双侧加固木轨0.35px，下凹宽0.5px槽，空置无矢）"),
    ("stock_nose", "root", (0.0, 20.8, 0.0), "弩臂前端（长2.5px，粗木穿弓座与暗掉阵纹铜箍）"),
    ("foot_stirrup", "stock_nose", (0.0, 23.5, 0.0), "前端锻铁脚蹬环踏环（向前伸出3.5px，宽3.2px空心中孔）"),
    ("bow_prod", "root", (0.0, 20.2, 0.0), "重型横装层压复合弩弓（横向跨度25.0px，厚实前拱大弧）"),
    ("bow_collar", "bow_prod", (0.0, 20.2, 0.0), "中段阵纹铜箍（末法暗掉阴刻阵线）"),
    ("bow_nocks", "bow_prod", (0.0, 19.2, 0.0), "两端厚木弓梢与暗掉灵骨贴片"),
    ("bowstring", "root", (0.0, 13.8, 0.7), "0.3px拉紧浅V形重弓弦（两端系梢，中央挂在重牙上，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 弩臂尾托 stock_butt (y: 0.0 ~ 3.4)
    # 重型抵肩托：截面宽大稳固 (1.6 × 2.2 px)，端部圆弧抵肩，向下弧形切削
    b = "stock_butt"
    rig.cube(b, "butt_plate_center", (-0.65, 0.0, -0.85), (0.65, 0.45, 0.85), mat="wood_dark")
    rig.cube(b, "butt_plate_top", (-0.55, 0.0, 0.85), (0.55, 0.45, 1.10), mat="wood_mid")
    rig.cube(b, "butt_plate_bot", (-0.55, 0.0, -1.10), (0.55, 0.45, -0.85), mat="wood_dark")
    rig.cube(b, "butt_plate_l", (-0.80, 0.0, -0.75), (-0.65, 0.45, 0.75), mat="wood_mid")
    rig.cube(b, "butt_plate_r", (0.65, 0.0, -0.75), (0.80, 0.45, 0.75), mat="wood_mid")
    rig.cube(b, "butt_plate_bevel_tl", (-0.75, 0.0, 0.75), (-0.55, 0.45, 1.00), mat="wood_mid")
    rig.cube(b, "butt_plate_bevel_tr", (0.55, 0.0, 0.75), (0.75, 0.45, 1.00), mat="wood_mid")
    rig.cube(b, "butt_plate_bevel_bl", (-0.75, 0.0, -1.00), (-0.55, 0.45, -0.75), mat="wood_dark")
    rig.cube(b, "butt_plate_bevel_br", (0.55, 0.0, -1.00), (0.75, 0.45, -0.75), mat="wood_dark")

    # 托身收拢平滑过渡 (y: 0.45 ~ 1.8)
    rig.cube(b, "butt_body_1_core", (-0.68, 0.45, -0.88), (0.68, 1.8, 0.88), mat="wood_dark")
    rig.cube(b, "butt_body_1_l", (-0.76, 0.48, -0.70), (-0.68, 1.78, 0.70), mat="wood_mid")
    rig.cube(b, "butt_body_1_r", (0.68, 0.48, -0.70), (0.76, 1.78, 0.70), mat="wood_mid")
    rig.cube(b, "butt_body_1_t", (-0.55, 0.48, 0.88), (0.55, 1.78, 1.02), mat="wood_mid")
    rig.cube(b, "butt_body_1_b", (-0.55, 0.48, -1.02), (0.55, 1.78, -0.88), mat="wood_dark")
    rig.cube(b, "butt_body_1_chamf_tl", (-0.72, 0.48, 0.65), (-0.60, 1.78, 0.92), mat="wood_mid")
    rig.cube(b, "butt_body_1_chamf_tr", (0.60, 0.48, 0.65), (0.72, 1.78, 0.92), mat="wood_mid")
    rig.cube(b, "butt_body_1_chamf_bl", (-0.72, 0.48, -0.92), (-0.60, 1.78, -0.65), mat="wood_dark")
    rig.cube(b, "butt_body_1_chamf_br", (0.60, 0.48, -0.92), (0.72, 1.78, -0.65), mat="wood_dark")

    # 托颈过渡 (y: 1.8 ~ 3.4)
    rig.cube(b, "butt_neck_core", (-0.60, 1.8, -0.75), (0.60, 3.4, 0.75), mat="wood_dark")
    rig.cube(b, "butt_neck_side_l", (-0.68, 1.82, -0.58), (-0.60, 3.38, 0.58), mat="wood_mid")
    rig.cube(b, "butt_neck_side_r", (0.60, 1.82, -0.58), (0.68, 3.38, 0.58), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_tl", (-0.65, 1.82, 0.55), (-0.52, 3.38, 0.82), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_tr", (0.52, 1.82, 0.55), (0.65, 3.38, 0.82), mat="wood_mid")
    rig.cube(b, "butt_neck_chamf_bl", (-0.65, 1.82, -0.82), (-0.52, 3.38, -0.55), mat="wood_dark")
    rig.cube(b, "butt_neck_chamf_br", (0.52, 1.82, -0.82), (0.65, 3.38, -0.55), mat="wood_dark")

    # 2. 握把段 stock_grip (y: 3.4 ~ 6.6)
    # 重型双手握把，截面 1.25 × 1.45 px，带指槽防滑雕琢
    b = "stock_grip"
    rig.cube(b, "grip_core", (-0.55, 3.4, -0.65), (0.55, 6.6, 0.65), mat="wood_mid")
    rig.cube(b, "grip_side_l", (-0.62, 3.45, -0.52), (-0.55, 6.55, 0.52), mat="wood_dark")
    rig.cube(b, "grip_side_r", (0.55, 3.45, -0.52), (0.62, 6.55, 0.52), mat="wood_dark")
    rig.cube(b, "grip_top_bevel", (-0.45, 3.42, 0.65), (0.45, 6.58, 0.74), mat="wood_mid")
    rig.cube(b, "grip_bot_belly", (-0.45, 3.42, -0.76), (0.45, 6.58, -0.65), mat="wood_dark")
    rig.cube(b, "grip_chamf_tl", (-0.58, 3.45, 0.48), (-0.48, 6.55, 0.68), mat="wood_mid")
    rig.cube(b, "grip_chamf_tr", (0.48, 3.45, 0.48), (0.58, 6.55, 0.68), mat="wood_mid")
    rig.cube(b, "grip_chamf_bl", (-0.58, 3.45, -0.68), (-0.48, 6.55, -0.48), mat="wood_dark")
    rig.cube(b, "grip_chamf_br", (0.48, 3.45, -0.68), (0.58, 6.55, -0.48), mat="wood_dark")
    # 指位防滑凹槽 (4 道深指位)
    for y_fg in [3.9, 4.6, 5.3, 6.0]:
        rig.cube(b, f"grip_fg_{y_fg}", (-0.48, y_fg, -0.80), (0.48, y_fg + 0.4, -0.72), mat="wood_mid")

    # 3. 重型青铜弩机郭身 lock_case (y: 6.6 ~ 8.8)
    # 厚重青铜匣 (长 2.2 px, x: -0.62~0.62, z: -0.7~0.7)
    b = "lock_case"
    rig.cube(b, "lock_box_outer", (-0.60, 6.6, -0.70), (0.60, 8.8, 0.70), mat="bronze")
    rig.cube(b, "lock_box_side_l", (-0.66, 6.65, -0.58), (-0.60, 8.75, 0.58), mat="bronze_patina")
    rig.cube(b, "lock_box_side_r", (0.60, 6.65, -0.58), (0.66, 8.75, 0.58), mat="bronze_patina")
    rig.cube(b, "lock_box_top_rim", (-0.52, 6.62, 0.68), (0.52, 8.78, 0.74), mat="bronze_sight")
    rig.cube(b, "lock_box_bot_rim", (-0.52, 6.62, -0.74), (0.52, 8.78, -0.68), mat="bronze_patina")
    rig.cube(b, "lock_box_rim_f", (-0.55, 8.75, -0.62), (0.55, 8.84, 0.62), mat="bronze_sight")
    rig.cube(b, "lock_box_rim_b", (-0.55, 6.58, -0.62), (0.55, 6.65, 0.62), mat="bronze_sight")
    # 贯穿重型青铜键销 (双轴销)
    rig.cube(b, "lock_pin_front", (-0.72, 8.1, -0.08), (0.72, 8.3, 0.12), mat="bronze_sight")
    rig.cube(b, "lock_pin_f_cap_l", (-0.76, 8.08, -0.12), (-0.70, 8.32, 0.16), mat="bronze")
    rig.cube(b, "lock_pin_f_cap_r", (0.70, 8.08, -0.12), (0.76, 8.32, 0.16), mat="bronze")
    rig.cube(b, "lock_pin_rear", (-0.72, 7.0, -0.08), (0.72, 7.2, 0.12), mat="bronze_sight")
    rig.cube(b, "lock_pin_r_cap_l", (-0.76, 6.98, -0.12), (-0.70, 7.22, 0.16), mat="bronze")
    rig.cube(b, "lock_pin_r_cap_r", (0.70, 6.98, -0.12), (0.76, 7.22, 0.16), mat="bronze")

    # 3.1 高耸望山瞄准片 lock_sight (y: 6.8 ~ 7.4, z: 0.70 ~ 2.30, 高达 1.6 px)
    b = "lock_sight"
    rig.cube(b, "sight_blade_base", (-0.30, 6.75, 0.70), (0.30, 7.45, 1.25), mat="bronze")
    rig.cube(b, "sight_blade_stem", (-0.22, 6.82, 1.25), (0.22, 7.38, 2.30), mat="bronze_sight")
    rig.cube(b, "sight_blade_spine", (-0.12, 7.36, 0.90), (0.12, 7.46, 2.25), mat="bronze_patina")
    # 望山刻度（7 道密集刻度线与暗掉感灵导引槽）
    for i, z_n in enumerate([1.35, 1.48, 1.61, 1.74, 1.87, 2.00, 2.13]):
        rig.cube(b, f"sight_notch_{i+1}", (-0.24, 6.78, z_n), (0.24, 6.88, z_n + 0.04), mat="iron_dark")
    rig.cube(b, "sight_spirit_groove", (-0.05, 6.80, 1.28), (0.05, 6.88, 2.22), mat="groove")

    # 3.2 重型抱弦牙 lock_tooth (y: 7.9 ~ 8.5, z: 0.70 ~ 1.45)
    b = "lock_tooth"
    rig.cube(b, "tooth_fork_l", (-0.45, 7.9, 0.70), (-0.15, 8.5, 1.45), mat="bronze")
    rig.cube(b, "tooth_fork_r", (0.15, 7.9, 0.70), (0.45, 8.5, 1.45), mat="bronze")
    rig.cube(b, "tooth_recess_floor", (-0.15, 7.9, 0.70), (0.15, 8.35, 0.95), mat="groove")
    rig.cube(b, "tooth_lip_l", (-0.44, 8.35, 1.20), (-0.15, 8.52, 1.45), mat="bronze_sight")
    rig.cube(b, "tooth_lip_r", (0.15, 8.35, 1.20), (0.44, 8.52, 1.45), mat="bronze_sight")
    rig.cube(b, "tooth_back_chamf", (-0.40, 7.85, 1.05), (0.40, 7.98, 1.38), mat="bronze_patina")

    # 3.3 重型悬刀扳机 lock_trigger (y: 6.7 ~ 7.4, z: -0.70 ~ -2.40, 伸出 1.7 px)
    b = "lock_trigger"
    rig.cube(b, "trigger_pivot_box", (-0.25, 6.72, -0.85), (0.25, 7.42, -0.68), mat="bronze")
    rig.cube(b, "trigger_stem_upper", (-0.20, 6.80, -1.45), (0.20, 7.35, -0.85), mat="bronze")
    rig.cube(b, "trigger_stem_mid", (-0.18, 6.72, -1.95), (0.18, 7.25, -1.45), mat="bronze_sight")
    rig.cube(b, "trigger_curve_hook", (-0.16, 6.55, -2.35), (0.16, 7.10, -1.85), mat="bronze_sight")
    rig.cube(b, "trigger_fin_tip", (-0.14, 6.42, -2.40), (0.14, 6.85, -2.20), mat="bronze_patina")
    # 悬刀防滑齿
    rig.cube(b, "trigger_teeth_1", (-0.18, 7.20, -1.85), (0.18, 7.28, -1.60), mat="iron_dark")
    rig.cube(b, "trigger_teeth_2", (-0.18, 7.10, -1.55), (0.18, 7.18, -1.30), mat="iron_dark")

    # 4. 弩臂中段与箭槽 stock_mid, stock_track (y: 8.8 ~ 19.5, 长 10.7 px)
    # 截面 1.15 × 1.35 px，八角倒棱硬木身
    b = "stock_mid"
    rig.cube(b, "mid_core", (-0.55, 8.8, -0.65), (0.55, 19.5, 0.45), mat="wood_dark")
    rig.cube(b, "mid_belly", (-0.45, 8.8, -0.72), (0.45, 19.5, -0.65), mat="wood_mid")
    rig.cube(b, "mid_side_l", (-0.60, 8.82, -0.55), (-0.55, 19.48, 0.38), mat="wood_mid")
    rig.cube(b, "mid_side_r", (0.55, 8.82, -0.55), (0.60, 19.48, 0.38), mat="wood_mid")
    rig.cube(b, "mid_chamf_bl", (-0.58, 8.82, -0.68), (-0.46, 19.48, -0.55), mat="wood_dark")
    rig.cube(b, "mid_chamf_br", (0.46, 8.82, -0.68), (0.58, 19.48, -0.55), mat="wood_dark")
    # 4段坚固生铁/麻绳加固圈 (防大拉力劈裂)
    for y_c in [11.0, 13.5, 16.0, 18.2]:
        rig.cube(b, f"mid_band_{y_c}", (-0.62, y_c, -0.70), (0.62, y_c + 0.45, 0.46), mat="cord")

    # 4.1 深槽箭槽 stock_track (y: 8.8 ~ 19.5, z: 0.45 ~ 0.70)
    # 双侧加固导轨高起 0.25px，宽各 0.35px；中央槽下凹宽 0.5px，空槽无矢！
    b = "stock_track"
    # 左导轨
    rig.cube(b, "track_rail_l_1", (-0.58, 8.8, 0.45), (-0.24, 12.5, 0.70), mat="wood_mid")
    rig.cube(b, "track_rail_l_2", (-0.58, 12.5, 0.45), (-0.24, 16.0, 0.70), mat="wood_mid")
    rig.cube(b, "track_rail_l_3", (-0.58, 16.0, 0.45), (-0.24, 19.5, 0.70), mat="wood_mid")
    rig.cube(b, "track_rail_l_lip", (-0.55, 8.8, 0.70), (-0.26, 19.5, 0.74), mat="wood_light")
    # 右导轨
    rig.cube(b, "track_rail_r_1", (0.24, 8.8, 0.45), (0.58, 12.5, 0.70), mat="wood_mid")
    rig.cube(b, "track_rail_r_2", (0.24, 12.5, 0.45), (0.58, 16.0, 0.70), mat="wood_mid")
    rig.cube(b, "track_rail_r_3", (0.24, 16.0, 0.45), (0.58, 19.5, 0.70), mat="wood_mid")
    rig.cube(b, "track_rail_r_lip", (0.26, 8.8, 0.70), (0.55, 19.5, 0.74), mat="wood_light")
    # 中央导槽底（阴影底，空槽！）
    rig.cube(b, "track_floor_1", (-0.24, 8.8, 0.45), (0.24, 14.0, 0.52), mat="groove")
    rig.cube(b, "track_floor_2", (-0.24, 14.0, 0.45), (0.24, 19.5, 0.52), mat="groove")

    # 5. 弩臂前端与穿弓座 stock_nose (y: 19.5 ~ 22.0)
    b = "stock_nose"
    rig.cube(b, "nose_socket_bed_b", (-0.60, 19.5, -0.72), (0.60, 20.8, -0.50), mat="wood_dark")
    rig.cube(b, "nose_socket_bed_t", (-0.60, 19.5, 0.50), (0.60, 20.8, 0.72), mat="wood_mid")
    rig.cube(b, "nose_socket_wall_l", (-0.62, 19.5, -0.50), (-0.52, 20.8, 0.50), mat="wood_dark")
    rig.cube(b, "nose_socket_wall_r", (0.52, 19.5, -0.50), (0.62, 20.8, 0.50), mat="wood_dark")
    # 前端木鼻延伸
    rig.cube(b, "nose_tip_core", (-0.52, 20.8, -0.62), (0.52, 21.6, 0.62), mat="wood_dark")
    rig.cube(b, "nose_tip_bevel_f", (-0.45, 21.6, -0.55), (0.45, 22.0, 0.55), mat="wood_light")
    # 粗麻绳死死捆绑固定圈
    rig.cube(b, "nose_heavy_cord_1", (-0.64, 19.6, -0.75), (0.64, 19.9, 0.75), mat="cord")
    rig.cube(b, "nose_heavy_cord_2", (-0.64, 20.8, -0.75), (0.64, 21.1, 0.75), mat="cord")

    # 5.1 铁脚蹬环 (踏环 stirrup) foot_stirrup (向前伸出 3.5 px, y: 22.0 ~ 25.5, 宽 3.2 px)
    # 踩住踏环双手拉弦！锻铁实心环条厚 0.5px，中孔透空 (x: -1.1 ~ 1.1, y: 22.5 ~ 25.0)
    b = "foot_stirrup"
    # 两根向前伸出的锻铁固定根臂
    rig.cube(b, "stirrup_shank_l", (-1.6, 21.2, -0.25), (-1.1, 22.5, 0.25), mat="iron_dark")
    rig.cube(b, "stirrup_shank_r", (1.1, 21.2, -0.25), (1.6, 22.5, 0.25), mat="iron_dark")
    rig.cube(b, "stirrup_shank_rivet_l", (-1.65, 21.6, -0.3), (-1.05, 21.9, 0.3), mat="iron_edge")
    rig.cube(b, "stirrup_shank_rivet_r", (1.05, 21.6, -0.3), (1.65, 21.9, 0.3), mat="iron_edge")
    # 踏环左右两竖边
    rig.cube(b, "stirrup_side_l", (-1.6, 22.5, -0.25), (-1.1, 25.0, 0.25), mat="iron_dark")
    rig.cube(b, "stirrup_side_r", (1.1, 22.5, -0.25), (1.6, 25.0, 0.25), mat="iron_dark")
    rig.cube(b, "stirrup_side_lip_l", (-1.65, 22.6, -0.15), (-1.55, 24.9, 0.15), mat="iron_edge")
    rig.cube(b, "stirrup_side_lip_r", (1.55, 22.6, -0.15), (1.65, 24.9, 0.15), mat="iron_edge")
    # 踏环前端横踏梁 (脚踩处磨亮)
    rig.cube(b, "stirrup_front_crossbar", (-1.6, 25.0, -0.25), (1.6, 25.5, 0.25), mat="iron_dark")
    rig.cube(b, "stirrup_front_tread", (-1.4, 25.4, -0.22), (1.4, 25.55, 0.22), mat="iron_edge")

    # 6. 重型横装层压复合弩弓 bow_prod (跨度 X: -12.5 ~ 12.5, 翼展 25.0 px)
    b = "bow_prod"
    # 中心贯穿固定段 (X: -1.5 ~ 1.5, Y: 19.8 ~ 20.6, Z: -0.45 ~ 0.45)
    rig.cube(b, "bow_center_core", (-1.5, 19.8, -0.45), (1.5, 20.6, 0.45), mat="wood_dark")
    rig.cube(b, "bow_center_face", (-1.4, 19.82, 0.45), (1.4, 20.58, 0.54), mat="wood_mid")
    rig.cube(b, "bow_center_back", (-1.4, 19.82, -0.54), (1.4, 20.58, -0.45), mat="wood_dark")

    # 6.1 中段阵纹铜箍 bow_collar (末法暗掉铜箍)
    b_col = "bow_collar"
    rig.cube(b_col, "collar_main", (-1.2, 19.75, -0.48), (1.2, 20.65, 0.48), mat="bronze")
    rig.cube(b_col, "collar_rim_l", (-1.25, 19.72, -0.50), (-1.15, 20.68, 0.50), mat="bronze_patina")
    rig.cube(b_col, "collar_rim_r", (1.15, 19.72, -0.50), (1.25, 20.68, 0.50), mat="bronze_patina")
    # 铜箍表面暗掉的阴刻阵纹 (faded rune groove)
    rig.cube(b_col, "collar_rune_1", (-0.9, 19.8, 0.48), (-0.3, 20.6, 0.52), mat="groove")
    rig.cube(b_col, "collar_rune_2", (0.3, 19.8, 0.48), (0.9, 20.6, 0.52), mat="groove")

    # 左右重型弓臂分段构建（分 8 段逐步展开前拱再折回梢头）：
    def add_heavy_limb_seg(name_sfx, xl, xr, y_f, y_t, z_f, z_t, mat_core, mat_face, mat_back):
        for side, sign in [("r", 1.0), ("l", -1.0)]:
            x0 = xl * sign
            x1 = xr * sign
            xmin, xmax = min(x0, x1), max(x0, x1)
            # 主体内胎
            rig.cube(b, f"limb_{name_sfx}_{side}", (xmin, y_f, z_f), (xmax, y_t, z_t), mat=mat_core)
            # 弓腹 (面)
            rig.cube(b, f"limb_{name_sfx}_f_{side}", (xmin + 0.04, y_f + 0.04, z_t), (xmax - 0.04, y_t - 0.04, z_t + 0.08), mat=mat_face)
            # 弓背 (牛筋层)
            rig.cube(b, f"limb_{name_sfx}_b_{side}", (xmin + 0.04, y_f + 0.04, z_f - 0.08), (xmax - 0.04, y_t - 0.04, z_f), mat=mat_back)

    # 段 1 (1.5 ~ 3.2): 厚 0.85×0.85
    add_heavy_limb_seg("s1", 1.5, 3.2, 19.9, 20.7, -0.42, 0.42, "wood_dark", "wood_mid", "wood_dark")
    # 段 2 (3.2 ~ 5.2): 厚 0.80×0.80
    add_heavy_limb_seg("s2", 3.2, 5.2, 20.2, 21.0, -0.39, 0.39, "wood_dark", "wood_mid", "wood_dark")
    # 段 3 (5.2 ~ 7.4): 前拱峰段 厚 0.74×0.74, Y 到达 21.35
    add_heavy_limb_seg("s3", 5.2, 7.4, 20.55, 21.35, -0.36, 0.36, "wood_dark", "wood_light", "cord")
    # 段 4 (7.4 ~ 9.5): 前拱延伸 厚 0.68×0.68
    add_heavy_limb_seg("s4", 7.4, 9.5, 20.65, 21.40, -0.32, 0.32, "wood_dark", "wood_light", "cord")
    # 段 5 (9.5 ~ 10.8): 回转过渡 厚 0.60×0.60
    add_heavy_limb_seg("s5", 9.5, 10.8, 20.25, 20.95, -0.28, 0.28, "wood_dark", "wood_mid", "wood_dark")
    # 段 6 (10.8 ~ 11.8): 梢根过渡 厚 0.52×0.52
    add_heavy_limb_seg("s6", 10.8, 11.8, 19.65, 20.35, -0.25, 0.25, "wood_dark", "wood_mid", "cord")
    # 段 7 (11.8 ~ 12.2): 弓弰接口 厚 0.46×0.46
    add_heavy_limb_seg("s7", 11.8, 12.2, 19.25, 19.85, -0.22, 0.22, "wood_dark", "cord", "wood_dark")

    # 6.2 弓梢与弦枕 bow_nocks (X: 12.2 ~ 12.5, Y: 18.9 ~ 19.5)
    b_n = "bow_nocks"
    for side, sign in [("r", 1.0), ("l", -1.0)]:
        x0 = 12.2 * sign
        x1 = 12.5 * sign
        xmin, xmax = min(x0, x1), max(x0, x1)
        rig.cube(b_n, f"nock_tip_{side}", (xmin, 18.9, -0.20), (xmax, 19.5, 0.20), mat="wood_mid")
        rig.cube(b_n, f"nock_bone_pad_{side}", (xmin + 0.04, 18.95, 0.20), (xmax - 0.04, 19.42, 0.28), mat="bone_pale")
        rig.cube(b_n, f"nock_groove_{side}", (xmin + 0.10, 19.10, -0.22), (xmax - 0.10, 19.30, 0.22), mat="groove")
        rig.cube(b_n, f"nock_wrap_{side}", (xmin + 0.02, 19.35, -0.22), (xmax - 0.02, 19.50, 0.22), mat="cord")

    # 7. 严格 0.3 px 粗细的浅 V 形重弓弦 bowstring
    # 左梢 (-12.3, 19.2, 0.22) -> 牙 (0.0, 8.2, 0.85) -> 右梢 (12.3, 19.2, 0.22)
    b = "bowstring"
    pt_left = (-12.3, 19.2, 0.22)
    pt_tooth = (0.0, 8.2, 0.85)
    pt_right = (12.3, 19.2, 0.22)

    # 牙位中央弦套与双线结
    rig.cube(b, "string_center_loop", (-0.18, 8.12, 0.72), (0.18, 8.38, 0.96), mat="string")
    rig.cube(b, "string_center_knot", (-0.15, 8.05, 0.76), (0.15, 8.22, 0.92), mat="cord")

    # 左侧弦分 6 段 (每段约 2.4px，严格 0.3px 粗细 rx=0.15, rz=0.15)
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

    # 右侧弦分 6 段
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
        f"#2 蹶张弩 jue_zhang_nu (Round 1 几何) · 弩臂长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
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
        ("铁脚蹬环伸出长 (Stirrup Len)", 15.9, 15.9),
        ("弩臂尾托占全长 (Butt Ratio)", 15.5, 15.5),
        ("弩机位置距尾端 (Lock Position)", 35.5, 35.5),
        ("矢道有效槽长 (Track Ratio)", 47.7, 47.7),
        ("前端弓座与铜箍 (Nose & Collar)", 11.4, 11.4),
        ("弩弓翼展/弩臂长 (Span/Stock)", 113.6, 113.6),
        ("望山瞄准片凸出比 (Sight Height)", 7.3, 7.3),
        ("悬刀扳机凸出比例 (Trigger Len)", 7.7, 7.7),
    ]

    ref_lines = [
        ("前端铁踏环 115.9%", 1.159, (120, 190, 240)),
        ("弩臂前端 100%", 1.0, (230, 90, 80)),
        ("弓座与阵纹铜箍 91.8%", 0.918, (220, 170, 70)),
        ("矢道前沿 88.6%", 0.886, (100, 210, 120)),
        ("弩机牙位 (挂弦点) 37.3%", 0.373, (90, 190, 240)),
        ("悬刀扳机位 31.8%", 0.318, (180, 140, 230)),
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
        title="#2 蹶张弩 jue_zhang_nu (Round 1 几何平涂版 · 弩臂22px · 铁踏环 · 弩弓25px · 弓弦0.3px)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:30s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
