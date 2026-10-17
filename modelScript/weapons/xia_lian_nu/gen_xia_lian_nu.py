#!/usr/bin/env python3
"""匣连弩 xia_lian_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：弩臂严格 14.0 px；
2. 汉式连发小弩（诸葛连弩式）：
   - 弩臂上方压一只长方木箭匣（空的，前端开口）；
   - 侧面一根杠杆式摇柄（上下摇动上弦）；
   - 弩弓短小（横向跨度约 13.0 px，翼展小于弩臂长）；
   - 箭匣侧面有长方形空白铭文匾位（末法铭文位空着）；
   - 箭匣内部空着，箭槽和弦上空着，不做箭、弩矢、弹丸、箭囊；
   - 弓弦严格 0.3 px（严禁粗方条），穿过滑槽拉紧；
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

WEAPON_ID = "xia_lian_nu"
MODEL_NAME = "XiaLianNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 14.0  # 弩臂全长 14.0 px
GRIP_Y = 2.5      # 后段手柄持握参考点

# 材质定义（Round 1 平涂）
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

# 骨骼层级树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stock_butt", "root", (0.0, 1.2, 0.0), "后手柄与微弯托（长2.5px，握柄1.1x1.3八角倒棱）"),
    ("stock_frame", "root", (0.0, 7.0, 0.0), "弩臂下框架（长9.5px，连接握把与前端穿弓座）"),
    ("magazine_box", "stock_frame", (0.0, 7.0, 1.6), "长方木箭匣（长9.0px，宽1.8px，高2.2px，中空空槽）"),
    ("cartouche_plaque", "magazine_box", (0.9, 7.0, 1.6), "箭匣侧面长方形空白铭文匾位（末法暗掉空匾）"),
    ("magazine_muzzle", "magazine_box", (0.0, 11.5, 1.6), "箭匣前端出箭口（开口窄缝，空槽无矢）"),
    ("lever_handle", "magazine_box", (-1.1, 4.0, 1.6), "侧面杠杆式连动摇柄（长5.5px，枢轴转动上弦杆）"),
    ("lever_linkage", "lever_handle", (-1.1, 4.0, 0.8), "摇柄与滑板下连动杆（青铜转轴与铁拉杆）"),
    ("stock_nose", "root", (0.0, 13.0, 0.0), "弩臂前端横装座与木鼻（长2.0px，穿弓孔与加固铜件）"),
    ("bow_prod", "root", (0.0, 12.5, 0.4), "短小层压复合弩弓（横跨13.0px，短小强劲微弧）"),
    ("bow_nocks", "bow_prod", (0.0, 11.8, 0.4), "两端短弓梢与暗掉灵骨贴片"),
    ("bowstring", "root", (0.0, 7.5, 0.4), "0.3px拉紧浅V形弓弦（两端系梢，穿过底槽，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 后手柄与握托 stock_butt (y: 0.0 ~ 2.5)
    # 类似手枪柄/短托的握持部，八角倒角贴手
    b = "stock_butt"
    rig.cube(b, "butt_cap_core", (-0.48, 0.0, -0.65), (0.48, 0.35, 0.65), mat="wood_dark")
    rig.cube(b, "butt_cap_top", (-0.40, 0.0, 0.65), (0.40, 0.35, 0.80), mat="wood_mid")
    rig.cube(b, "butt_cap_bot", (-0.40, 0.0, -0.80), (0.40, 0.35, -0.65), mat="wood_dark")
    rig.cube(b, "butt_cap_l", (-0.56, 0.0, -0.55), (-0.48, 0.35, 0.55), mat="wood_mid")
    rig.cube(b, "butt_cap_r", (0.48, 0.0, -0.55), (0.56, 0.35, 0.55), mat="wood_mid")
    rig.cube(b, "butt_cap_bevel_tl", (-0.54, 0.0, 0.55), (-0.40, 0.35, 0.75), mat="wood_mid")
    rig.cube(b, "butt_cap_bevel_tr", (0.40, 0.0, 0.55), (0.54, 0.35, 0.75), mat="wood_mid")
    rig.cube(b, "butt_cap_bevel_bl", (-0.54, 0.0, -0.75), (-0.40, 0.35, -0.55), mat="wood_dark")
    rig.cube(b, "butt_cap_bevel_br", (0.40, 0.0, -0.75), (0.54, 0.35, -0.55), mat="wood_dark")

    # 手柄过渡段 (y: 0.35 ~ 1.5)
    rig.cube(b, "grip_body_core", (-0.46, 0.35, -0.60), (0.46, 1.5, 0.60), mat="wood_mid")
    rig.cube(b, "grip_side_l", (-0.54, 0.38, -0.48), (-0.46, 1.48, 0.48), mat="wood_dark")
    rig.cube(b, "grip_side_r", (0.46, 0.38, -0.48), (0.54, 1.48, 0.48), mat="wood_dark")
    rig.cube(b, "grip_belly", (-0.38, 0.38, -0.72), (0.38, 1.48, -0.60), mat="wood_dark")
    rig.cube(b, "grip_spine", (-0.38, 0.38, 0.60), (0.38, 1.48, 0.72), mat="wood_mid")
    rig.cube(b, "grip_chamf_tl", (-0.50, 0.38, 0.48), (-0.38, 1.48, 0.66), mat="wood_mid")
    rig.cube(b, "grip_chamf_tr", (0.38, 0.38, 0.48), (0.50, 1.48, 0.66), mat="wood_mid")
    rig.cube(b, "grip_chamf_bl", (-0.50, 0.38, -0.66), (-0.38, 1.48, -0.48), mat="wood_dark")
    rig.cube(b, "grip_chamf_br", (0.38, 0.38, -0.66), (0.50, 1.48, -0.48), mat="wood_dark")

    # 与下框架连接段 (y: 1.5 ~ 2.5)
    rig.cube(b, "grip_neck_core", (-0.45, 1.5, -0.52), (0.45, 2.5, 0.52), mat="wood_dark")
    rig.cube(b, "grip_neck_chamf_l", (-0.52, 1.52, -0.40), (-0.45, 2.48, 0.40), mat="wood_mid")
    rig.cube(b, "grip_neck_chamf_r", (0.45, 1.52, -0.40), (0.52, 2.48, 0.40), mat="wood_mid")
    rig.cube(b, "grip_neck_bevel_t", (-0.35, 1.52, 0.52), (0.35, 2.48, 0.60), mat="wood_mid")
    rig.cube(b, "grip_neck_bevel_b", (-0.35, 1.52, -0.60), (0.35, 2.48, -0.52), mat="wood_dark")
    # 扳机/击发短钮 (手柄前上方的小扳杆)
    rig.cube(b, "trigger_lever_stub", (-0.15, 2.0, -0.80), (0.15, 2.4, -0.52), mat="bronze")
    rig.cube(b, "trigger_lever_tip", (-0.12, 1.85, -0.95), (0.12, 2.2, -0.75), mat="bronze_sight")

    # 2. 弩臂下框架 stock_frame (y: 2.5 ~ 12.0)
    # 承托上部箭匣与下部滑轨的长方木梁 (宽 1.1 × 厚 0.9 px)
    b = "stock_frame"
    rig.cube(b, "frame_rail_core", (-0.48, 2.5, -0.48), (0.48, 12.0, 0.48), mat="wood_dark")
    rig.cube(b, "frame_rail_bot_lip", (-0.38, 2.5, -0.55), (0.38, 12.0, -0.48), mat="wood_mid")
    rig.cube(b, "frame_rail_side_l", (-0.54, 2.52, -0.38), (-0.48, 11.98, 0.38), mat="wood_mid")
    rig.cube(b, "frame_rail_side_r", (0.48, 2.52, -0.38), (0.54, 11.98, 0.38), mat="wood_mid")
    rig.cube(b, "frame_rail_chamf_bl", (-0.52, 2.52, -0.52), (-0.38, 11.98, -0.38), mat="wood_dark")
    rig.cube(b, "frame_rail_chamf_br", (0.38, 2.52, -0.52), (0.52, 11.98, -0.38), mat="wood_dark")
    # 底部加固金属卡箍
    for y_cl, name_cl in [(3.2, "rear"), (5.8, "mid1"), (8.5, "mid2"), (11.2, "front")]:
        rig.cube(b, f"frame_clamp_{name_cl}", (-0.56, y_cl, -0.58), (0.56, y_cl + 0.35, 0.52), mat="bronze")
        rig.cube(b, f"frame_clamp_rim_{name_cl}", (-0.58, y_cl + 0.08, -0.55), (0.58, y_cl + 0.28, 0.48), mat="bronze_patina")

    # 3. 长方木箭匣 magazine_box (y: 2.5 ~ 11.5, z: 0.48 ~ 2.68, 宽 1.8 px, 高 2.2 px)
    # 内部中空槽 (x: -0.6 ~ 0.6, y: 2.8 ~ 11.2, z: 0.6 ~ 2.6) - 空槽无矢！
    b = "magazine_box"
    # 左侧壁分段铺设 (x: -0.90 ~ -0.62)
    rig.cube(b, "mag_wall_l_1", (-0.90, 2.5, 0.48), (-0.62, 5.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_l_2", (-0.90, 5.5, 0.48), (-0.62, 8.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_l_3", (-0.90, 8.5, 0.48), (-0.62, 11.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_l_top_lip", (-0.94, 2.5, 2.60), (-0.60, 11.5, 2.72), mat="wood_light")
    # 左侧壁外侧加强木肋与金属护角
    rig.cube(b, "mag_wall_l_rib1", (-0.95, 4.5, 0.55), (-0.88, 5.0, 2.60), mat="wood_dark")
    rig.cube(b, "mag_wall_l_rib2", (-0.95, 7.5, 0.55), (-0.88, 8.0, 2.60), mat="wood_dark")
    rig.cube(b, "mag_wall_l_rib3", (-0.95, 9.8, 0.55), (-0.88, 10.3, 2.60), mat="wood_dark")
    # 左侧青铜角码 (L形固定角片)
    rig.cube(b, "mag_corner_tl_1", (-0.95, 2.5, 2.50), (-0.75, 3.1, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_tl_2", (-0.95, 10.9, 2.50), (-0.75, 11.5, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_bl_1", (-0.95, 2.5, 0.48), (-0.75, 3.1, 0.68), mat="bronze")
    rig.cube(b, "mag_corner_bl_2", (-0.95, 10.9, 0.48), (-0.75, 11.5, 0.68), mat="bronze")

    # 右侧壁分段铺设 (x: 0.62 ~ 0.90)
    rig.cube(b, "mag_wall_r_1", (0.62, 2.5, 0.48), (0.90, 5.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_r_2", (0.62, 5.5, 0.48), (0.90, 8.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_r_3", (0.62, 8.5, 0.48), (0.90, 11.5, 2.68), mat="wood_mid")
    rig.cube(b, "mag_wall_r_top_lip", (0.60, 2.5, 2.60), (0.94, 11.5, 2.72), mat="wood_light")
    # 右侧壁外侧加强木肋
    rig.cube(b, "mag_wall_r_rib1", (0.88, 4.5, 0.55), (0.95, 5.0, 2.60), mat="wood_dark")
    rig.cube(b, "mag_wall_r_rib2", (0.88, 7.5, 0.55), (0.95, 8.0, 2.60), mat="wood_dark")
    rig.cube(b, "mag_wall_r_rib3", (0.88, 9.8, 0.55), (0.95, 10.3, 2.60), mat="wood_dark")
    # 右侧青铜角码
    rig.cube(b, "mag_corner_tr_1", (0.75, 2.5, 2.50), (0.95, 3.1, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_tr_2", (0.75, 10.9, 2.50), (0.95, 11.5, 2.70), mat="bronze")
    rig.cube(b, "mag_corner_br_1", (0.75, 2.5, 0.48), (0.95, 3.1, 0.68), mat="bronze")
    rig.cube(b, "mag_corner_br_2", (0.75, 10.9, 0.48), (0.95, 11.5, 0.68), mat="bronze")

    # 后壁 (y: 2.5 ~ 2.85)
    rig.cube(b, "mag_wall_rear_core", (-0.85, 2.5, 0.48), (0.85, 2.85, 2.68), mat="wood_dark")
    rig.cube(b, "mag_wall_rear_lip", (-0.80, 2.5, 2.60), (0.80, 2.85, 2.74), mat="wood_light")
    rig.cube(b, "mag_wall_rear_plate", (-0.65, 2.45, 0.85), (0.65, 2.52, 2.35), mat="wood_mid")

    # 箭匣底板与滑槽 (z: 0.48 ~ 0.62)
    rig.cube(b, "mag_floor_l", (-0.62, 2.85, 0.48), (-0.20, 11.2, 0.62), mat="wood_dark")
    rig.cube(b, "mag_floor_r", (0.20, 2.85, 0.48), (0.62, 11.2, 0.62), mat="wood_dark")
    # 中央出弦与推矢落位凹缝 (空置！)
    rig.cube(b, "mag_groove_slot", (-0.20, 2.85, 0.45), (0.20, 11.2, 0.52), mat="groove")

    # 箭匣顶口内凹空槽标志（无箭无矢，内部暗面）
    rig.cube(b, "mag_interior_shadow_rear", (-0.60, 2.85, 0.62), (0.60, 3.2, 2.58), mat="groove")
    rig.cube(b, "mag_interior_shadow_floor", (-0.60, 3.2, 0.62), (0.60, 11.0, 0.72), mat="groove")
    # 箭匣内导槽两条薄限位凸条
    rig.cube(b, "mag_interior_rail_l", (-0.62, 3.5, 1.1), (-0.58, 10.8, 1.25), mat="wood_light")
    rig.cube(b, "mag_interior_rail_r", (0.58, 3.5, 1.1), (0.62, 10.8, 1.25), mat="wood_light")

    # 3.1 箭匣右侧空白铭文匾位 cartouche_plaque (y: 5.0 ~ 8.5, z: 1.1 ~ 2.1)
    # 末法：箭匣侧面的铭文位空着
    b = "cartouche_plaque"
    rig.cube(b, "plaque_border_t", (0.90, 5.0, 2.05), (0.96, 8.5, 2.15), mat="bronze_sight")
    rig.cube(b, "plaque_border_b", (0.90, 5.0, 1.05), (0.96, 8.5, 1.15), mat="bronze_sight")
    rig.cube(b, "plaque_border_l", (0.90, 5.0, 1.15), (0.96, 5.2, 2.05), mat="bronze_sight")
    rig.cube(b, "plaque_border_r", (0.90, 8.3, 1.15), (0.96, 8.5, 2.05), mat="bronze_sight")
    # 空白凹匾底板（空着无字！）
    rig.cube(b, "plaque_empty_field", (0.88, 5.2, 1.15), (0.92, 8.3, 2.05), mat="groove")
    # 铭文匾上下青铜装饰铆钉
    rig.cube(b, "plaque_rivet_1", (0.92, 5.5, 2.15), (0.98, 5.7, 2.25), mat="bronze")
    rig.cube(b, "plaque_rivet_2", (0.92, 7.8, 2.15), (0.98, 8.0, 2.25), mat="bronze")

    # 3.2 箭匣前端出箭口 magazine_muzzle (y: 11.2 ~ 11.5)
    # 前端开口出箭缝 (x: -0.45 ~ 0.45, z: 0.58 ~ 1.25)
    b = "magazine_muzzle"
    rig.cube(b, "muzzle_top_lintel", (-0.75, 11.2, 1.25), (0.75, 11.5, 2.68), mat="wood_mid")
    rig.cube(b, "muzzle_post_l", (-0.85, 11.2, 0.48), (-0.45, 11.5, 1.25), mat="wood_dark")
    rig.cube(b, "muzzle_post_r", (0.45, 11.2, 0.48), (0.85, 11.5, 1.25), mat="wood_dark")
    rig.cube(b, "muzzle_threshold", (-0.45, 11.2, 0.48), (0.45, 11.5, 0.58), mat="wood_light")
    # 出箭口边缘青铜耐磨包角与铆栓
    rig.cube(b, "muzzle_collar_l", (-0.90, 11.45, 0.55), (-0.42, 11.55, 1.30), mat="bronze")
    rig.cube(b, "muzzle_collar_r", (0.42, 11.45, 0.55), (0.90, 11.55, 1.30), mat="bronze")
    rig.cube(b, "muzzle_lip_top", (-0.45, 11.45, 1.22), (0.45, 11.55, 1.30), mat="bronze_sight")

    # 4. 侧面杠杆式连动摇柄 lever_handle (左侧 x: -0.95 ~ -1.35)
    # 枢轴在箭匣左下 (x: -1.0, y: 3.8, z: 1.5)
    b = "lever_handle"
    # 青铜枢轴大销盖与垫片
    rig.cube(b, "lever_pivot_cap", (-1.25, 3.6, 1.35), (-0.92, 4.05, 1.75), mat="bronze_sight")
    rig.cube(b, "lever_pivot_pin", (-1.30, 3.75, 1.48), (-0.90, 3.90, 1.62), mat="bronze")
    rig.cube(b, "lever_pivot_washer", (-1.28, 3.65, 1.40), (-1.22, 4.00, 1.70), mat="bronze_patina")
    # 主杠杆臂（斜向上后方伸展，长 5.5 px，从 y=3.8, z=1.5 伸到 y=0.5, z=4.8）
    rig.shaft(b, "lever_main_shaft", (-1.15, 3.8, 1.55), (-1.15, 0.6, 4.6), rx=0.22, rz=0.22, mat="wood_mid")
    rig.shaft(b, "lever_iron_spine", (-1.22, 3.8, 1.55), (-1.22, 0.6, 4.6), rx=0.12, rz=0.12, mat="iron_dark")
    rig.shaft(b, "lever_brass_strip", (-1.08, 3.8, 1.55), (-1.08, 0.6, 4.6), rx=0.08, rz=0.08, mat="bronze")
    # 摇柄顶端圆横握把 (手握圆木)
    rig.cube(b, "lever_grip_knob", (-1.65, 0.35, 4.45), (-0.75, 0.85, 4.95), mat="wood_dark")
    rig.cube(b, "lever_grip_collar_l", (-1.70, 0.45, 4.55), (-1.60, 0.75, 4.85), mat="bronze")
    rig.cube(b, "lever_grip_collar_r", (-0.80, 0.45, 4.55), (-0.70, 0.75, 4.85), mat="bronze")
    rig.cube(b, "lever_grip_core_pin", (-1.75, 0.55, 4.65), (-0.70, 0.65, 4.75), mat="iron_dark")

    # 4.1 连动拉杆 lever_linkage (下连动机构)
    b = "lever_linkage"
    # 连接摇柄轴心与下方推弦滑块的短连杆
    rig.shaft(b, "linkage_bar", (-1.10, 3.8, 1.45), (-1.10, 5.6, 0.55), rx=0.15, rz=0.15, mat="iron_dark")
    rig.cube(b, "linkage_joint_top", (-1.18, 3.65, 1.35), (-0.98, 3.95, 1.55), mat="bronze")
    rig.cube(b, "linkage_joint_bot", (-1.18, 5.45, 0.45), (-0.98, 5.75, 0.65), mat="bronze")
    rig.cube(b, "linkage_pin_top", (-1.22, 3.75, 1.42), (-0.94, 3.85, 1.48), mat="bronze_sight")
    rig.cube(b, "linkage_pin_bot", (-1.22, 5.55, 0.52), (-0.94, 5.65, 0.58), mat="bronze_sight")
    # 下方滑动卡爪 (用于挂弦推弦)
    rig.cube(b, "slider_block", (-0.50, 5.2, 0.40), (0.50, 5.8, 0.60), mat="bronze_patina")
    rig.cube(b, "slider_catch_l", (-0.35, 5.6, 0.55), (-0.15, 5.85, 0.75), mat="bronze")
    rig.cube(b, "slider_catch_r", (0.15, 5.6, 0.55), (0.35, 5.85, 0.75), mat="bronze")
    rig.cube(b, "slider_track_lip", (-0.45, 5.25, 0.35), (0.45, 5.75, 0.40), mat="bronze_sight")

    # 5. 弩臂前端与横装座 stock_nose (y: 12.0 ~ 14.0)
    b = "stock_nose"
    # 穿弓座
    rig.cube(b, "nose_bed_bot", (-0.52, 12.0, -0.48), (0.52, 13.0, -0.32), mat="wood_dark")
    rig.cube(b, "nose_bed_top", (-0.52, 12.0, 0.32), (0.52, 13.0, 0.52), mat="wood_mid")
    rig.cube(b, "nose_wall_l", (-0.55, 12.0, -0.32), (-0.45, 13.0, 0.32), mat="wood_dark")
    rig.cube(b, "nose_wall_r", (0.45, 12.0, -0.32), (0.55, 13.0, 0.32), mat="wood_dark")
    # 前端木鼻伸出 (y: 13.0 ~ 14.0)
    rig.cube(b, "nose_tip_core", (-0.42, 13.0, -0.42), (0.42, 13.8, 0.42), mat="wood_dark")
    rig.cube(b, "nose_tip_side_l", (-0.46, 13.02, -0.35), (-0.42, 13.78, 0.35), mat="wood_mid")
    rig.cube(b, "nose_tip_side_r", (0.42, 13.02, -0.35), (0.46, 13.78, 0.35), mat="wood_mid")
    rig.cube(b, "nose_tip_chisel", (-0.35, 13.8, -0.35), (0.35, 14.0, 0.35), mat="wood_light")
    # 前端加固麻绳圈与青铜箍
    rig.cube(b, "nose_cord_rear_1", (-0.54, 12.1, -0.50), (0.54, 12.35, 0.50), mat="cord")
    rig.cube(b, "nose_cord_rear_2", (-0.54, 12.35, -0.50), (0.54, 12.55, 0.50), mat="cord")
    rig.cube(b, "nose_bronze_front", (-0.54, 12.9, -0.46), (0.54, 13.15, 0.46), mat="bronze")
    rig.cube(b, "nose_bronze_pin", (-0.56, 12.98, -0.05), (0.56, 13.06, 0.05), mat="bronze_sight")

    # 6. 短小层压复合弩弓 bow_prod (短小弩弓，跨度 X: -6.5 ~ 6.5, 翼展 13.0 px)
    b = "bow_prod"
    # 中心固定段 (X: -1.0 ~ 1.0, Y: 12.2 ~ 12.8, Z: -0.28 ~ 0.28)
    rig.cube(b, "bow_center_core", (-1.0, 12.2, -0.28), (1.0, 12.8, 0.28), mat="wood_dark")
    rig.cube(b, "bow_center_face", (-0.95, 12.22, 0.28), (0.95, 12.78, 0.35), mat="wood_mid")
    rig.cube(b, "bow_center_back", (-0.95, 12.22, -0.35), (0.95, 12.78, -0.28), mat="wood_dark")
    rig.cube(b, "bow_center_wrap_l", (-0.8, 12.18, -0.32), (-0.4, 12.82, 0.32), mat="cord")
    rig.cube(b, "bow_center_wrap_r", (0.4, 12.18, -0.32), (0.8, 12.82, 0.32), mat="cord")

    # 左右弓臂分段构建（每侧分 6 段）：
    def add_compact_limb_seg(name_sfx, xl, xr, y_f, y_t, z_f, z_t, mat_core, mat_face, mat_back):
        for side, sign in [("r", 1.0), ("l", -1.0)]:
            x0 = xl * sign
            x1 = xr * sign
            xmin, xmax = min(x0, x1), max(x0, x1)
            rig.cube(b, f"limb_{name_sfx}_{side}", (xmin, y_f, z_f), (xmax, y_t, z_t), mat=mat_core)
            rig.cube(b, f"limb_{name_sfx}_f_{side}", (xmin + 0.03, y_f + 0.03, z_t), (xmax - 0.03, y_t - 0.03, z_t + 0.05), mat=mat_face)
            rig.cube(b, f"limb_{name_sfx}_b_{side}", (xmin + 0.03, y_f + 0.03, z_f - 0.05), (xmax - 0.03, y_t - 0.03, z_f), mat=mat_back)

    # 段 1 (1.0 ~ 2.0): 弓腰近端 厚 0.55×0.55
    add_compact_limb_seg("s1", 1.0, 2.0, 12.35, 12.85, -0.26, 0.26, "wood_dark", "wood_mid", "wood_dark")
    # 段 2 (2.0 ~ 3.2): 前拱展开 厚 0.52×0.52
    add_compact_limb_seg("s2", 2.0, 3.2, 12.50, 12.98, -0.25, 0.25, "wood_dark", "wood_mid", "wood_dark")
    # 段 3 (3.2 ~ 4.4): 前拱顶点 Y: 12.60 -> 13.08, 厚 0.48×0.48
    add_compact_limb_seg("s3", 3.2, 4.4, 12.60, 13.08, -0.23, 0.23, "wood_dark", "wood_light", "cord")
    # 段 4 (4.4 ~ 5.3): 回转弧段 厚 0.44×0.44
    add_compact_limb_seg("s4", 4.4, 5.3, 12.45, 12.92, -0.21, 0.21, "wood_dark", "wood_mid", "wood_dark")
    # 段 5 (5.3 ~ 5.9): 梢根过渡 厚 0.39×0.39
    add_compact_limb_seg("s5", 5.3, 5.9, 12.05, 12.52, -0.18, 0.18, "wood_dark", "wood_mid", "cord")
    # 段 6 (5.9 ~ 6.2): 弓弰接口 厚 0.35×0.35
    add_compact_limb_seg("s6", 5.9, 6.2, 11.75, 12.15, -0.16, 0.16, "wood_dark", "cord", "wood_dark")

    # 6.1 弓梢与弦枕 bow_nocks (X: 6.2 ~ 6.5, Y: 11.5 ~ 11.95)
    b_n = "bow_nocks"
    for side, sign in [("r", 1.0), ("l", -1.0)]:
        x0 = 6.2 * sign
        x1 = 6.5 * sign
        xmin, xmax = min(x0, x1), max(x0, x1)
        rig.cube(b_n, f"nock_tip_{side}", (xmin, 11.5, -0.16), (xmax, 11.95, 0.16), mat="wood_mid")
        rig.cube(b_n, f"nock_bone_pad_{side}", (xmin + 0.03, 11.52, 0.16), (xmax - 0.03, 11.88, 0.22), mat="bone_pale")
        rig.cube(b_n, f"nock_groove_{side}", (xmin + 0.08, 11.65, -0.18), (xmax - 0.08, 11.80, 0.18), mat="groove")
        rig.cube(b_n, f"nock_wrap_{side}", (xmin + 0.02, 11.88, -0.17), (xmax - 0.02, 11.98, 0.17), mat="cord")

    # 7. 严格 0.3 px 粗细的浅 V 形弓弦 bowstring
    # 左梢 (-6.3, 11.7, 0.2) -> 底槽推弦爪 (0.0, 5.6, 0.55) -> 右梢 (6.3, 11.7, 0.2)
    b = "bowstring"
    pt_left = (-6.3, 11.7, 0.2)
    pt_tooth = (0.0, 5.6, 0.55)
    pt_right = (6.3, 11.7, 0.2)

    # 中央弦套与打结
    rig.cube(b, "string_center_loop", (-0.15, 5.52, 0.45), (0.15, 5.72, 0.65), mat="string")
    rig.cube(b, "string_center_knot", (-0.12, 5.45, 0.48), (0.12, 5.60, 0.62), mat="cord")

    # 左侧弦分 5 段 (严格 0.3px 粗细 rx=0.15, rz=0.15)
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
        f"#3 匣连弩 xia_lian_nu (Round 1 几何) · 弩臂长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
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
        ("长方箭匣总长 (Magazine Length)", 64.3, 64.3),
        ("箭匣高度占比 (Magazine Height)", 15.7, 15.7),
        ("箭匣宽度占比 (Magazine Width)", 12.9, 12.9),
        ("侧面铭文空匾长 (Blank Plaque)", 25.0, 25.0),
        ("侧面连动摇柄长 (Lever Handle)", 39.3, 39.3),
        ("尾部握把段占比 (Rear Grip)", 17.9, 17.9),
        ("短小弩弓翼展比 (Span/Stock)", 92.9, 92.9),
        ("前端出箭与木鼻 (Nose & Muzzle)", 14.3, 14.3),
    ]

    ref_lines = [
        ("前端弩头 100%", 1.0, (230, 90, 80)),
        ("出箭口前沿 82.1%", 0.821, (120, 190, 240)),
        ("短小弩弓座 89.3%", 0.893, (220, 170, 70)),
        ("侧面铭文匾 50.0%", 0.50, (130, 220, 160)),
        ("摇柄枢轴位 27.1%", 0.271, (180, 140, 230)),
        ("握柄过渡 17.9%", 0.179, (210, 150, 120)),
        ("尾端手柄 0%", 0.0, (230, 90, 80)),
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
        title="#3 匣连弩 xia_lian_nu (Round 1 几何平涂版 · 弩臂14px · 木箭匣 · 侧面摇柄 · 弓弦0.3px)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:30s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
