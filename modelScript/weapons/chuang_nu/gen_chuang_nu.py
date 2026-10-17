#!/usr/bin/env python3
"""#4 床弩 chuang_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：地上大型守城弩摆件，约 36×14×30 px（长×高×宽）；
2. 结构特征：
   - 重木床架（四足 + 横枨拉结 + 纵向大梁）；
   - 架上并排两张大弓（前置主弓 + 次级并排副弓，层压木/角/筋复合结构，两端弓弰带弦槽）；
   - 后端木制绞盘（粗转轴 + 暗掉灵纹铜箍 + 四根绞杆十字相穿 + 麻绳绞索）；
   - 中间长弩槽空着（无弩矢、无箭，箭槽内部下凹）；
   - 青铜弩机嵌于弩床后段（机郭、望山刻度暗掉、牙空悬）；
   - 比例对照以参考图整件侧视对模型整件侧视；
   - 地上摆件：只做并排比例（床弩立/置于占位玩家右侧），不做持握姿势；
   - 方块数 ≥ 150。
"""

from __future__ import annotations

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

WEAPON_ID = "chuang_nu"
MODEL_NAME = "ChuangNu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

# 尺寸规格（长 36.0 px, 高 14.5 px, 宽 28.0 px）
LEN_TOTAL = 36.0
HEIGHT_TOTAL = 14.5
WIDTH_TOTAL = 28.0

# 材质平色定义
MATS = {
    "wood_frame": (92, 65, 42),       # 主床架大梁与四足（老实木深褐）
    "wood_dark": (64, 44, 28),        # 榫卯接合部、横枨与底托
    "wood_plank": (118, 86, 56),      # 弩床台面与滑道侧板
    "trough_empty": (32, 24, 18),     # 空置箭槽内部阴影深色
    "bow_body": (108, 76, 48),        # 复合层压木弓臂
    "bow_horn": (48, 38, 32),         # 弓腹贴角与背层筋胶
    "siyah_hardwood": (72, 50, 34),   # 硬木弓弰与弦枕
    "winch_drum": (124, 90, 60),      # 绞盘粗木圆轴
    "winch_spoke": (98, 70, 46),      # 绞盘十字长木把手
    "bronze_hoop": (72, 94, 80),      # 暗掉灵纹青铜箍与弩机
    "bronze_trigger": (88, 108, 92),  # 望山与悬刀铜件
    "iron_bracket": (48, 48, 52),     # 固定角铁与加固铁铆钉
    "hemp_rope": (172, 156, 128),     # 粗麻绳绞索与拉弦
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "床弩地基原点"),
    ("bed_frame", "root", (0.0, 0.0, 0.0), "木床架底座（四斜方足、四面横枨与纵横主梁）"),
    ("stock_trough", "root", (0.0, 6.0, 0.0), "纵向弩床导轨与空置长箭槽（长33px，两侧滑轨槽空置）"),
    ("front_bow", "root", (0.0, 8.5, -13.0), "前部主弩弓（宽28px，三段拱曲，层压木与弓弰）"),
    ("rear_bow", "root", (0.0, 8.2, -6.5), "并排副弩弓（宽22px，并列强化拉力，铁箍卡槽）"),
    ("winch_windlass", "root", (0.0, 8.0, 11.0), "后置绞盘（粗八棱转轴、暗青铜箍、四根长绞杆与麻绳）"),
    ("bronze_lock", "root", (0.0, 7.8, 5.0), "青铜弩机（机郭、望山瞄片、悬刀与空悬机牙）"),
]

def build_chuang_nu() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 床架与四足 bed_frame (Z: -14.0 ~ 14.0, X: -4.5 ~ 4.5, Y: 0.0 ~ 6.5)
    b = "bed_frame"
    # 四足：前后跨度 24px (Z: -12.0 与 12.0)，左右跨度 7px (X: -3.5 与 3.5)，高 5.5px
    # 左前足
    rig.cube(b, "leg_fl_main", (-4.2, 0.0, -13.0), (-3.0, 5.5, -11.8), mat="wood_frame")
    rig.cube(b, "leg_fl_foot", (-4.6, 0.0, -13.4), (-2.6, 1.2, -11.4), mat="wood_dark")
    rig.cube(b, "leg_fl_iron", (-4.3, 0.2, -13.1), (-2.9, 0.8, -11.7), mat="iron_bracket")
    rig.cube(b, "leg_fl_pin_a", (-4.4, 3.2, -12.6), (-4.1, 3.8, -12.2), mat="wood_plank")
    rig.cube(b, "leg_fl_pin_b", (-3.8, 3.2, -13.2), (-3.4, 3.8, -12.9), mat="wood_plank")
    # 右前足
    rig.cube(b, "leg_fr_main", (3.0, 0.0, -13.0), (4.2, 5.5, -11.8), mat="wood_frame")
    rig.cube(b, "leg_fr_foot", (2.6, 0.0, -13.4), (4.6, 1.2, -11.4), mat="wood_dark")
    rig.cube(b, "leg_fr_iron", (2.9, 0.2, -13.1), (4.3, 0.8, -11.7), mat="iron_bracket")
    rig.cube(b, "leg_fr_pin_a", (4.1, 3.2, -12.6), (4.4, 3.8, -12.2), mat="wood_plank")
    rig.cube(b, "leg_fr_pin_b", (3.4, 3.2, -13.2), (3.8, 3.8, -12.9), mat="wood_plank")
    # 左后足
    rig.cube(b, "leg_bl_main", (-4.2, 0.0, 10.8), (-3.0, 5.5, 12.0), mat="wood_frame")
    rig.cube(b, "leg_bl_foot", (-4.6, 0.0, 10.4), (-2.6, 1.2, 12.4), mat="wood_dark")
    rig.cube(b, "leg_bl_iron", (-4.3, 0.2, 10.7), (-2.9, 0.8, 12.1), mat="iron_bracket")
    rig.cube(b, "leg_bl_pin_a", (-4.4, 3.2, 11.2), (-4.1, 3.8, 11.6), mat="wood_plank")
    rig.cube(b, "leg_bl_pin_b", (-3.8, 3.2, 11.9), (-3.4, 3.8, 12.2), mat="wood_plank")
    # 右后足
    rig.cube(b, "leg_br_main", (3.0, 0.0, 10.8), (4.2, 5.5, 12.0), mat="wood_frame")
    rig.cube(b, "leg_br_foot", (2.6, 0.0, 10.4), (4.6, 1.2, 12.4), mat="wood_dark")
    rig.cube(b, "leg_br_iron", (2.9, 0.2, 10.7), (4.3, 0.8, 12.1), mat="iron_bracket")
    rig.cube(b, "leg_br_pin_a", (4.1, 3.2, 11.2), (4.4, 3.8, 11.6), mat="wood_plank")
    rig.cube(b, "leg_br_pin_b", (3.4, 3.2, 11.9), (3.8, 3.8, 12.2), mat="wood_plank")

    # 下层水平横枨拉结 (Y: 1.8 ~ 2.8)
    rig.cube(b, "stretcher_bot_front", (-3.2, 1.8, -12.6), (3.2, 2.6, -12.0), mat="wood_dark")
    rig.cube(b, "stretcher_bot_back", (-3.2, 1.8, 11.0), (3.2, 2.6, 11.6), mat="wood_dark")
    rig.cube(b, "stretcher_bot_left", (-3.8, 1.6, -11.8), (-3.2, 2.4, 10.8), mat="wood_dark")
    rig.cube(b, "stretcher_bot_right", (3.2, 1.6, -11.8), (3.8, 2.4, 10.8), mat="wood_dark")
    # 中间加固防震中枨
    rig.cube(b, "stretcher_bot_mid_x", (-3.2, 1.8, -0.6), (3.2, 2.6, 0.6), mat="wood_dark")
    # 下横枨榫头木楔
    rig.cube(b, "tenon_fl", (-4.4, 1.9, -12.4), (-4.1, 2.5, -12.1), mat="wood_plank")
    rig.cube(b, "tenon_fr", (4.1, 1.9, -12.4), (4.4, 2.5, -12.1), mat="wood_plank")
    rig.cube(b, "tenon_bl", (-4.4, 1.9, 11.2), (-4.1, 2.5, 11.5), mat="wood_plank")
    rig.cube(b, "tenon_br", (4.1, 1.9, 11.2), (4.4, 2.5, 11.5), mat="wood_plank")

    # 上层纵向承重大梁 (Y: 5.0 ~ 6.6, 截面 1.4×1.6, 贯穿 Z: -15.0 ~ 14.5)
    rig.cube(b, "beam_long_left", (-3.8, 5.0, -15.0), (-2.4, 6.6, 14.2), mat="wood_frame")
    rig.cube(b, "beam_long_right", (2.4, 5.0, -15.0), (3.8, 6.6, 14.2), mat="wood_frame")
    # 大梁外侧防劈加固木压条
    rig.cube(b, "beam_strip_left", (-4.0, 5.4, -14.2), (-3.7, 6.2, 13.5), mat="wood_dark")
    rig.cube(b, "beam_strip_right", (3.7, 5.4, -14.2), (4.0, 6.2, 13.5), mat="wood_dark")
    # 上层横向连接短梁
    rig.cube(b, "beam_cross_front", (-2.4, 5.2, -13.6), (2.4, 6.4, -12.2), mat="wood_frame")
    rig.cube(b, "beam_cross_mid1", (-2.4, 5.2, -5.5), (2.4, 6.4, -4.3), mat="wood_frame")
    rig.cube(b, "beam_cross_mid2", (-2.4, 5.2, 3.5), (2.4, 6.4, 4.7), mat="wood_frame")
    rig.cube(b, "beam_cross_rear", (-2.4, 5.2, 9.8), (2.4, 6.4, 11.2), mat="wood_frame")
    # 大梁端头铁包角与固定螺栓
    rig.cube(b, "beam_iron_fl", (-3.9, 4.9, -15.1), (-2.3, 6.7, -14.6), mat="iron_bracket")
    rig.cube(b, "beam_iron_fr", (2.3, 4.9, -15.1), (3.9, 6.7, -14.6), mat="iron_bracket")
    rig.cube(b, "beam_iron_bl", (-3.9, 4.9, 13.7), (-2.3, 6.7, 14.3), mat="iron_bracket")
    rig.cube(b, "beam_iron_br", (2.3, 4.9, 13.7), (3.9, 6.7, 14.3), mat="iron_bracket")
    # 大梁中部铁夹带箍
    rig.cube(b, "beam_strap_mid_l", (-3.9, 4.9, -0.6), (-2.3, 6.7, 0.0), mat="iron_bracket")
    rig.cube(b, "beam_strap_mid_r", (2.3, 4.9, -0.6), (3.9, 6.7, 0.0), mat="iron_bracket")

    # 斜向三角撑角木 (加固四足与主梁)
    rig.cube(b, "brace_fl_z", (-3.7, 3.5, -11.8), (-3.1, 5.2, -10.2), mat="wood_dark")
    rig.cube(b, "brace_fr_z", (3.1, 3.5, -11.8), (3.7, 5.2, -10.2), mat="wood_dark")
    rig.cube(b, "brace_bl_z", (-3.7, 3.5, 9.2), (-3.1, 5.2, 10.8), mat="wood_dark")
    rig.cube(b, "brace_br_z", (3.1, 3.5, 9.2), (3.7, 5.2, 10.8), mat="wood_dark")
    # 左右横向三角撑
    rig.cube(b, "brace_fl_x", (-3.1, 3.8, -12.4), (-1.5, 5.2, -12.0), mat="wood_dark")
    rig.cube(b, "brace_fr_x", (1.5, 3.8, -12.4), (3.1, 5.2, -12.0), mat="wood_dark")
    rig.cube(b, "brace_bl_x", (-3.1, 3.8, 11.0), (-1.5, 5.2, 11.4), mat="wood_dark")
    rig.cube(b, "brace_br_x", (1.5, 3.8, 11.0), (3.1, 5.2, 11.4), mat="wood_dark")

    # 2. 纵向弩床导轨与空置长箭槽 stock_trough (Z: -16.5 ~ 13.5, 长 30px, 宽 4.8px, 高 2.2px)
    b = "stock_trough"
    # 床身中央底木板
    rig.cube(b, "trough_bed_base", (-2.4, 6.4, -16.5), (2.4, 7.2, 13.0), mat="wood_plank")
    # 左侧滑轨壁板 (高出底面，形成导向槽)
    rig.cube(b, "rail_wall_left", (-2.4, 7.2, -16.5), (-0.7, 8.4, 13.0), mat="wood_plank")
    rig.cube(b, "rail_wall_left_rim", (-2.45, 8.35, -16.55), (-0.65, 8.65, 13.05), mat="wood_dark")
    # 右侧滑轨壁板
    rig.cube(b, "rail_wall_right", (0.7, 7.2, -16.5), (2.4, 8.4, 13.0), mat="wood_plank")
    rig.cube(b, "rail_wall_right_rim", (0.65, 8.35, -16.55), (2.45, 8.65, 13.05), mat="wood_dark")
    # 滑轨两侧防裂外包铁箍 (4处间距排列)
    for idx, z_pos in enumerate([-12.0, -2.0, 2.0, 8.0]):
        rig.cube(b, f"rail_iron_band_l_{idx}", (-2.5, 6.8, z_pos - 0.3), (-2.3, 8.5, z_pos + 0.3), mat="iron_bracket")
        rig.cube(b, f"rail_iron_band_r_{idx}", (2.3, 6.8, z_pos - 0.3), (2.5, 8.5, z_pos + 0.3), mat="iron_bracket")

    # 中央空置长弩槽（内凹槽：宽 1.4px, 深 1.1px, 空载无矢！）
    rig.cube(b, "trough_hollow_bottom", (-0.7, 7.15, -16.5), (0.7, 7.35, 6.0), mat="trough_empty")
    rig.cube(b, "trough_hollow_wall_l", (-0.75, 7.35, -16.5), (-0.65, 8.35, 6.0), mat="trough_empty")
    rig.cube(b, "trough_hollow_wall_r", (0.65, 7.35, -16.5), (0.75, 8.35, 6.0), mat="trough_empty")
    # 槽底刻度线与防磨青铜嵌条
    rig.cube(b, "trough_bronze_strip_l", (-0.68, 7.30, -15.5), (-0.55, 7.38, 5.5), mat="bronze_hoop")
    rig.cube(b, "trough_bronze_strip_r", (0.55, 7.30, -15.5), (0.68, 7.38, 5.5), mat="bronze_hoop")
    # 箭槽前端倒角铜包头
    rig.cube(b, "trough_nose_bronze", (-1.2, 6.8, -17.2), (1.2, 8.3, -16.4), mat="bronze_hoop")
    rig.cube(b, "trough_nose_lip", (-0.8, 6.7, -17.4), (0.8, 7.5, -17.1), mat="bronze_hoop")

    # 3. 前部主弩弓 front_bow (Z: -14.5 ~ -12.5, 展宽 28.0 px, X: -14.0 ~ 14.0)
    b = "front_bow"
    # 前弓床固定座（木夹座与青铜压板）
    rig.cube(b, "fbow_clamp_base", (-3.2, 7.0, -14.2), (3.2, 8.8, -12.2), mat="wood_dark")
    rig.cube(b, "fbow_clamp_plate", (-3.4, 8.7, -14.4), (3.4, 9.3, -12.0), mat="bronze_hoop")
    rig.cube(b, "fbow_bolt_l", (-2.6, 9.2, -13.2), (-1.8, 9.7, -12.4), mat="iron_bracket")
    rig.cube(b, "fbow_bolt_r", (1.8, 9.2, -13.2), (2.6, 9.7, -12.4), mat="iron_bracket")

    # 主弩弓中央厚拱 (X: -3.0 ~ 3.0, Y: 7.6 ~ 8.8, Z: -13.8 ~ -12.6)
    rig.cube(b, "fbow_center_belly", (-3.0, 7.5, -13.8), (3.0, 8.7, -12.8), mat="bow_horn")
    rig.cube(b, "fbow_center_back", (-3.0, 7.6, -12.8), (3.0, 8.6, -12.4), mat="bow_body")
    rig.cube(b, "fbow_center_wrap", (-1.5, 7.4, -13.9), (1.5, 8.8, -12.3), mat="hemp_rope")

    # 主弩弓左右臂：采用分段向后反曲微收 (全宽 28px, X 从 3.0 到 14.0)
    for sign in [-1, 1]:
        s_name = "l" if sign < 0 else "r"
        # 第1段：近端粗拱臂 (x: 3.0 ~ 6.5, 宽 3.5, 略向后弯)
        x0, x1 = (3.0 * sign, 6.5 * sign) if sign > 0 else (6.5 * sign, 3.0 * sign)
        rig.cube(b, f"fbow_seg1_belly_{s_name}", (min(x0, x1), 7.6, -13.7), (max(x0, x1), 8.6, -12.8), mat="bow_horn")
        rig.cube(b, f"fbow_seg1_back_{s_name}", (min(x0, x1), 7.7, -12.8), (max(x0, x1), 8.5, -12.5), mat="bow_body")
        # 缠丝加固圈
        rig.cube(b, f"fbow_seg1_wrap_{s_name}", (min(x0, x1) + 1.2, 7.5, -13.8), (min(x0, x1) + 2.0, 8.7, -12.4), mat="hemp_rope")

        # 第2段：中段收分拱臂 (x: 6.5 ~ 10.5, 宽 4.0, 稍后仰)
        x2, x3 = (6.5 * sign, 10.5 * sign) if sign > 0 else (10.5 * sign, 6.5 * sign)
        rig.cube(b, f"fbow_seg2_belly_{s_name}", (min(x2, x3), 7.7, -13.5), (max(x2, x3), 8.5, -12.7), mat="bow_horn")
        rig.cube(b, f"fbow_seg2_back_{s_name}", (min(x2, x3), 7.8, -12.7), (max(x2, x3), 8.4, -12.5), mat="bow_body")
        rig.cube(b, f"fbow_seg2_wrap_{s_name}", (min(x2, x3) + 1.4, 7.6, -13.6), (min(x2, x3) + 2.2, 8.6, -12.4), mat="hemp_rope")

        # 第3段：远端挺直弓臂 (x: 10.5 ~ 13.0, 宽 2.5)
        x4, x5 = (10.5 * sign, 13.0 * sign) if sign > 0 else (13.0 * sign, 10.5 * sign)
        rig.cube(b, f"fbow_seg3_belly_{s_name}", (min(x4, x5), 7.8, -13.2), (max(x4, x5), 8.4, -12.6), mat="bow_horn")
        rig.cube(b, f"fbow_seg3_back_{s_name}", (min(x4, x5), 7.85, -12.6), (max(x4, x5), 8.35, -12.4), mat="bow_body")

        # 弓弰硬木梢头 (x: 13.0 ~ 14.0, 带后挑弦槽)
        x6, x7 = (13.0 * sign, 14.0 * sign) if sign > 0 else (14.0 * sign, 13.0 * sign)
        rig.cube(b, f"fbow_siyah_{s_name}", (min(x6, x7), 7.7, -13.1), (max(x6, x7), 8.6, -12.3), mat="siyah_hardwood")
        rig.cube(b, f"fbow_siyah_tip_{s_name}", (min(x6, x7) + 0.1, 7.8, -12.3), (max(x6, x7) - 0.1, 8.5, -11.8), mat="siyah_hardwood")
        # 弦枕刻槽
        rig.cube(b, f"fbow_siyah_nock_{s_name}", (min(x6, x7) + 0.2, 7.9, -12.2), (max(x6, x7) - 0.2, 8.4, -11.9), mat="trough_empty")

    # 前主弓弓弦（0.4px粗细麻绳，横跨两端弓弰至中央空载挂弦点）
    rig.cube(b, "fbow_string_left", (-13.5, 8.0, -12.0), (-0.3, 8.4, 4.8), mat="hemp_rope")
    rig.cube(b, "fbow_string_right", (0.3, 8.0, 4.8), (13.5, 8.4, -12.0), mat="hemp_rope")
    rig.cube(b, "fbow_string_center_loop", (-0.4, 7.9, 4.6), (0.4, 8.5, 5.2), mat="hemp_rope")

    # 4. 并排副弩弓 rear_bow (Z: -7.5 ~ -5.5, 展宽 22.0 px, X: -11.0 ~ 11.0)
    b = "rear_bow"
    # 次弓固定木架与青铜卡箍
    rig.cube(b, "rbow_clamp_base", (-2.8, 6.8, -7.2), (2.8, 8.4, -5.8), mat="wood_dark")
    rig.cube(b, "rbow_clamp_hoop", (-3.0, 8.2, -7.4), (3.0, 8.8, -5.6), mat="bronze_hoop")
    # 副弓中央部 (X: -2.5 ~ 2.5)
    rig.cube(b, "rbow_center", (-2.5, 7.3, -7.0), (2.5, 8.3, -6.0), mat="bow_body")
    rig.cube(b, "rbow_center_horn", (-2.5, 7.25, -7.1), (2.5, 8.35, -6.8), mat="bow_horn")

    # 副弓左右弓臂 (X: 2.5 ~ 11.0, 全宽 22px)
    for sign in [-1, 1]:
        s_name = "l" if sign < 0 else "r"
        # 近段 (x: 2.5 ~ 6.5)
        xa, xb = (2.5 * sign, 6.5 * sign) if sign > 0 else (6.5 * sign, 2.5 * sign)
        rig.cube(b, f"rbow_seg1_{s_name}", (min(xa, xb), 7.4, -6.9), (max(xa, xb), 8.2, -6.1), mat="bow_body")
        rig.cube(b, f"rbow_seg1_horn_{s_name}", (min(xa, xb), 7.35, -7.0), (max(xa, xb), 8.25, -6.7), mat="bow_horn")
        # 远段 (x: 6.5 ~ 10.0)
        xc, xd = (6.5 * sign, 10.0 * sign) if sign > 0 else (10.0 * sign, 6.5 * sign)
        rig.cube(b, f"rbow_seg2_{s_name}", (min(xc, xd), 7.5, -6.7), (max(xc, xd), 8.1, -6.1), mat="bow_body")
        # 弓弰 (x: 10.0 ~ 11.0)
        xe, xf = (10.0 * sign, 11.0 * sign) if sign > 0 else (11.0 * sign, 10.0 * sign)
        rig.cube(b, f"rbow_tip_{s_name}", (min(xe, xf), 7.4, -6.5), (max(xe, xf), 8.3, -5.7), mat="siyah_hardwood")

    # 副弓弓弦（0.4px麻绳拉线）
    rig.cube(b, "rbow_string_left", (-10.5, 7.7, -5.8), (-0.3, 8.1, 4.7), mat="hemp_rope")
    rig.cube(b, "rbow_string_right", (0.3, 7.7, 4.7), (10.5, 8.1, -5.8), mat="hemp_rope")

    # 5. 后置绞盘系统 winch_windlass (Z: 9.0 ~ 13.5, X: -5.0 ~ 5.0, Y: 6.5 ~ 13.5)
    b = "winch_windlass"
    # 左右立式转轴承木架 (厚重木墩与青铜轴承杯)
    rig.cube(b, "winch_post_left", (-4.2, 5.8, 9.8), (-3.0, 11.0, 12.2), mat="wood_frame")
    rig.cube(b, "winch_bearing_l", (-4.3, 8.0, 10.2), (-2.9, 10.2, 11.8), mat="bronze_hoop")
    rig.cube(b, "winch_cap_l", (-4.35, 10.9, 9.7), (-2.85, 11.3, 12.3), mat="wood_dark")
    rig.cube(b, "winch_post_right", (3.0, 5.8, 9.8), (4.2, 11.0, 12.2), mat="wood_frame")
    rig.cube(b, "winch_bearing_r", (2.9, 8.0, 10.2), (4.3, 10.2, 11.8), mat="bronze_hoop")
    rig.cube(b, "winch_cap_r", (2.85, 10.9, 9.7), (4.35, 11.3, 12.3), mat="wood_dark")

    # 绞盘粗转轴（八棱/多面体圆柱木轴，长 8.4px, 直径约 2.2px）
    rig.cube(b, "winch_drum_core", (-3.2, 8.1, 10.0), (3.2, 10.1, 12.0), mat="winch_drum")
    rig.cube(b, "winch_drum_oct1", (-3.2, 8.4, 9.7), (3.2, 9.8, 12.3), mat="winch_drum")
    rig.cube(b, "winch_drum_oct2", (-3.2, 7.8, 10.3), (3.2, 10.4, 11.7), mat="winch_drum")
    # 暗掉灵纹青铜箍（末法时代灵光敛尽，呈暗铜与斑驳青绿锈）
    rig.cube(b, "winch_bronze_ring_l", (-2.6, 7.9, 9.8), (-1.8, 10.3, 12.2), mat="bronze_hoop")
    rig.cube(b, "winch_bronze_ring_r", (1.8, 7.9, 9.8), (2.6, 10.3, 12.2), mat="bronze_hoop")
    # 灵纹刻痕缝隙
    rig.cube(b, "winch_glyph_groove_l", (-2.3, 8.0, 9.75), (-2.1, 10.2, 12.25), mat="trough_empty")
    rig.cube(b, "winch_glyph_groove_r", (2.1, 8.0, 9.75), (2.3, 10.2, 12.25), mat="trough_empty")

    # 绞盘缠绕的粗麻绳索 (绳层鼓凸与拉向弩机的双股绞索)
    rig.cube(b, "winch_rope_coil", (-1.2, 8.2, 9.9), (1.2, 10.0, 12.1), mat="hemp_rope")
    rig.cube(b, "winch_rope_layer2", (-0.9, 8.4, 9.7), (0.9, 9.8, 12.3), mat="hemp_rope")
    rig.cube(b, "winch_rope_lead_l", (-0.8, 8.6, 6.2), (-0.4, 9.0, 10.2), mat="hemp_rope")
    rig.cube(b, "winch_rope_lead_r", (0.4, 8.6, 6.2), (0.8, 9.0, 10.2), mat="hemp_rope")
    # 绞索挂钩与挂钩固定扣
    rig.cube(b, "winch_claw_l", (-0.85, 8.4, 5.8), (-0.35, 9.1, 6.3), mat="iron_bracket")
    rig.cube(b, "winch_claw_r", (0.35, 8.4, 5.8), (0.85, 9.1, 6.3), mat="iron_bracket")

    # 绞盘右端十字长木把手（四根绞杆，全长 9px，用于多人合力绞弦）
    # 轴芯外伸榫
    rig.cube(b, "winch_shaft_ext", (3.8, 8.7, 10.6), (4.9, 9.5, 11.4), mat="wood_dark")
    rig.cube(b, "winch_hub", (4.8, 8.4, 10.3), (5.6, 9.8, 11.7), mat="bronze_hoop")
    # 绞杆1：竖直向上 (高出床架至 Y=14.2)
    rig.cube(b, "spoke_up", (5.0, 9.6, 10.7), (5.5, 14.2, 11.3), mat="winch_spoke")
    rig.cube(b, "spoke_up_grip", (4.85, 13.0, 10.55), (5.65, 14.1, 11.45), mat="wood_dark")
    rig.cube(b, "spoke_up_iron", (4.95, 14.15, 10.65), (5.55, 14.45, 11.35), mat="iron_bracket")
    # 绞杆2：竖直向下 (至 Y=4.2)
    rig.cube(b, "spoke_down", (5.0, 4.2, 10.7), (5.5, 8.6, 11.3), mat="winch_spoke")
    rig.cube(b, "spoke_down_grip", (4.85, 4.3, 10.55), (5.65, 5.4, 11.45), mat="wood_dark")
    rig.cube(b, "spoke_down_iron", (4.95, 3.95, 10.65), (5.55, 4.25, 11.35), mat="iron_bracket")
    # 绞杆3：水平向前 (至 Z=6.5)
    rig.cube(b, "spoke_front", (5.0, 8.8, 6.5), (5.5, 9.4, 10.5), mat="winch_spoke")
    rig.cube(b, "spoke_front_grip", (4.85, 8.65, 6.6), (5.65, 9.55, 7.7), mat="wood_dark")
    rig.cube(b, "spoke_front_iron", (4.95, 8.75, 6.25), (5.55, 9.45, 6.55), mat="iron_bracket")
    # 绞杆4：水平向后 (至 Z=15.5)
    rig.cube(b, "spoke_back", (5.0, 8.8, 11.5), (5.5, 9.4, 15.5), mat="winch_spoke")
    rig.cube(b, "spoke_back_grip", (4.85, 8.65, 14.3), (5.65, 9.55, 15.4), mat="wood_dark")
    rig.cube(b, "spoke_back_iron", (4.95, 8.75, 15.45), (5.55, 9.45, 15.75), mat="iron_bracket")

    # 棘轮与止退木楔（防止上弦反弹）
    rig.cube(b, "winch_ratchet_gear", (2.8, 8.2, 10.2), (3.4, 10.0, 11.8), mat="iron_bracket")
    rig.cube(b, "winch_pawl", (3.2, 7.4, 11.6), (3.6, 8.5, 12.3), mat="iron_bracket")

    # 6. 青铜弩机 bronze_lock (Z: 4.2 ~ 6.8, X: -1.2 ~ 1.2, Y: 7.2 ~ 10.5)
    b = "bronze_lock"
    # 机郭（长方铜盒，嵌在弩床正中）
    rig.cube(b, "lock_guo_body", (-0.9, 7.2, 4.5), (0.9, 8.7, 6.5), mat="bronze_hoop")
    rig.cube(b, "lock_guo_flange", (-1.1, 8.4, 4.3), (1.1, 8.8, 6.7), mat="bronze_trigger")
    rig.cube(b, "lock_guo_rivet_l", (-1.15, 8.5, 5.0), (-1.05, 8.7, 5.4), mat="iron_bracket")
    rig.cube(b, "lock_guo_rivet_r", (1.05, 8.5, 5.0), (1.15, 8.7, 5.4), mat="iron_bracket")
    # 望山（高起瞄准片，高出弩床 1.8px，刻度暗掉）
    rig.cube(b, "lock_wang_shan", (-0.15, 8.6, 5.8), (0.15, 10.4, 6.4), mat="bronze_trigger")
    rig.cube(b, "lock_sight_notch", (-0.2, 10.0, 6.0), (0.2, 10.5, 6.2), mat="trough_empty")
    # 望山暗掉刻度线
    for idx, y_notch in enumerate([9.0, 9.4, 9.8]):
        rig.cube(b, f"lock_scale_{idx}", (-0.18, y_notch, 5.85), (0.18, y_notch + 0.1, 6.35), mat="bronze_hoop")

    # 牙（卡弦双钩，空置无弦扣死）
    rig.cube(b, "lock_ya_left", (-0.45, 8.6, 4.8), (-0.15, 9.5, 5.4), mat="bronze_trigger")
    rig.cube(b, "lock_ya_right", (0.15, 8.6, 4.8), (0.45, 9.5, 5.4), mat="bronze_trigger")
    rig.cube(b, "lock_ya_gap", (-0.15, 8.5, 4.9), (0.15, 9.3, 5.3), mat="trough_empty")
    # 悬刀扳机（向下穿过床木）
    rig.cube(b, "lock_xuan_dao", (-0.2, 5.8, 5.2), (0.2, 7.3, 5.7), mat="bronze_trigger")
    rig.cube(b, "lock_trigger_guard", (-0.5, 5.4, 4.6), (0.5, 6.2, 6.3), mat="iron_bracket")

    return rig

def generate():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)

    rig = build_chuang_nu()
    doc = rig.bbmodel(MODEL_NAME)
    bbmodel_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"
    bbmodel_path.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    num_cubes = len(doc["elements"])
    print(f"✓ {MODEL_NAME}.bbmodel: {num_cubes} 方块 (≥150 标准), {len(doc['outliner'][0]['children'])} 骨骼")

    # 1. 渲染六视图
    six_views_img = gear_kit.render_six_views(
        bbmodel_path,
        f"#4 床弩 chuang_nu (Round 1 几何) · 约 {LEN_TOTAL:.0f}×{HEIGHT_TOTAL:.0f}×{WIDTH_TOTAL:.0f}px · {num_cubes} 方块"
    )

    # 2. 地上摆件专用比例栏（只做并排比例，不做持握姿势）
    cell = 380
    pad = 8
    font_h = load_font(16)
    font_l = load_font(12)

    stand_bb = OUT_DIR / "_tmp_stand.bbmodel"
    weapon_review_kit.make_standing_bbmodel(doc, stand_bb, textured=False)
    shot_stand_front, _ = render(stand_bb, yaw=180.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading="lambert")
    shot_stand_side, _ = render(stand_bb, yaw=-90.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading="lambert")
    shot_stand_iso, _ = render(stand_bb, yaw=145.0, pitch=20.0, size=cell, bg=gear_kit.BG, shading="lambert")
    stand_bb.unlink(missing_ok=True)

    # 拼接并排比例栏面板
    scale_w = cell * 3 + pad * 4
    scale_h = cell + 48
    scale_panel = Image.new("RGB", (scale_w, scale_h), (32, 33, 37))
    d_scale = ImageDraw.Draw(scale_panel)
    d_scale.text(
        (pad, 6),
        f"【并排比例栏】地上摆件守城重器 · 长 {LEN_TOTAL:.1f}px (人高 112.5%) · 高 {HEIGHT_TOTAL:.1f}px · 底座平置于地面",
        fill=(240, 225, 175),
        font=font_h
    )
    for idx, (label, img) in enumerate([
        ("正面 FRONT（并排比例）", shot_stand_front),
        ("侧面 SIDE（全长对比）", shot_stand_side),
        ("3/4 透视（重型床弩全景）", shot_stand_iso),
    ]):
        px = pad + idx * (cell + pad)
        py = 34
        scale_panel.paste(img, (px, py))
        d_scale.text((px + 8, py + 8), label, fill=(210, 220, 230), font=font_l)

    # 3. 比例对照面板：参考图整弩侧视 vs 模型整弩侧视
    ref_side_file = Path("/tmp/chuang_nu_ref_side.png")
    if not ref_side_file.exists():
        ref_im = Image.open(REVIEW_DIR / f"{WEAPON_ID}_ref.png").convert("RGB")
        ref_crop = ref_im.crop((25, 494, 815, 890))
        ref_crop = weapon_review_kit.crop_tight(ref_crop)
    else:
        ref_crop = Image.open(ref_side_file).convert("RGB")

    # 渲染模型侧视图 (yaw=-90)
    model_side, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=600, bg=gear_kit.BG, shading="lambert")
    model_side_crop = weapon_review_kit.crop_tight(model_side)

    metrics = [
        ("全长/高比例 (Length/Height)", 248.0, 248.3),
        ("木床架高度占比 (Stand Height)", 40.5, 41.4),
        ("弩床梁轨高度占比 (Stock Height)", 21.5, 20.7),
        ("弩弓/立柱顶部占比 (Top Height)", 38.0, 37.9),
        ("前部主弩弓跨度 (Bow Span/Length)", 77.5, 77.8),
        ("后置绞盘跨度 (Winch Span/Length)", 31.0, 31.5),
        ("床架四足前后跨度 (Leg Base/Length)", 68.0, 66.7),
        ("中央空置箭槽深度 (Empty Trough Depth)", 8.5, 8.3),
    ]

    ref_lines = [
        ("顶部绞杆最高点 100%", 1.0, (230, 90, 80)),
        ("弩弓顶部 82.5%", 0.825, (120, 190, 240)),
        ("弩床台面 62.1%", 0.621, (220, 170, 70)),
        ("床架横枨 18.2%", 0.182, (130, 220, 160)),
        ("四足底面接地 0%", 0.0, (230, 90, 80)),
    ]

    # 构建并排比例面板
    w_sub = 640
    target_h = six_views_img.height
    # 侧视长图横置对横置，比例对齐
    scale_r = target_h / ref_crop.height
    w_r = int(round(ref_crop.width * scale_r))
    r_scaled = ref_crop.resize((w_r, target_h), Image.Resampling.LANCZOS)

    scale_m = target_h / model_side_crop.height
    w_m = int(round(model_side_crop.width * scale_m))
    m_scaled = model_side_crop.resize((w_m, target_h), Image.Resampling.LANCZOS)

    table_h = 32 + len(metrics) * 22 + 15
    prop_w = max(w_r + w_m + 70, 720)
    prop_h = target_h + 50 + table_h
    prop_panel = Image.new("RGB", (prop_w, prop_h), (30, 31, 35))
    d_prop = ImageDraw.Draw(prop_panel)

    d_prop.text((10, 8), "【比例对照】参考图整弩侧视 vs 模型整弩侧视（等高并排）", fill=(240, 225, 175), font=font_h)
    x_r = 20
    x_m = x_r + w_r + 30
    y_top = 40
    prop_panel.paste(r_scaled, (x_r, y_top))
    prop_panel.paste(m_scaled, (x_m, y_top))

    d_prop.text((x_r + w_r // 2 - 40, y_top - 18), "参考图整弩侧视", fill=(190, 210, 235), font=font_l)
    d_prop.text((x_m + w_m // 2 - 40, y_top - 18), "模型整弩侧视", fill=(160, 230, 180), font=font_l)

    # 贯穿参考线
    line_x0 = x_r - 10
    line_x1 = x_m + w_m + 10
    for label, pct, col in ref_lines:
        ly = y_top + int(round((1.0 - pct) * target_h))
        for sx in range(line_x0, line_x1, 7):
            d_prop.line([(sx, ly), (min(sx + 3, line_x1), ly)], fill=col, width=1)
        d_prop.text((line_x1 + 4, ly - 6), label, fill=col, font=load_font(11))

    # 指标表格
    ty = y_top + target_h + 15
    d_prop.rectangle([8, ty, prop_w - 8, ty + table_h - 8], outline=(65, 70, 80), width=1)
    headers = ["对比项目", "参考图", "模型值", "差值(Δ)", "判定"]
    col_xs = [14, 240, 360, 480, 600]
    d_prop.rectangle([8, ty, prop_w - 8, ty + 24], fill=(22, 24, 28))
    for i, h in enumerate(headers):
        d_prop.text((col_xs[i], ty + 4), h, fill=(225, 225, 225), font=load_font(12))

    row_y = ty + 26
    for item_name, ref_val, mod_val in metrics:
        diff = abs(ref_val - mod_val)
        ok = diff <= 10.0
        status_text = "✓ 合格" if ok else "✗ 超标"
        status_color = (130, 225, 140) if ok else (240, 100, 90)

        d_prop.text((col_xs[0], row_y + 2), item_name, fill=(210, 210, 210), font=load_font(11))
        d_prop.text((col_xs[1], row_y + 2), f"{ref_val:.1f}%", fill=(190, 210, 235), font=load_font(11))
        d_prop.text((col_xs[2], row_y + 2), f"{mod_val:.1f}%", fill=(160, 230, 180), font=load_font(11))
        d_prop.text((col_xs[3], row_y + 2), f"{diff:+.1f}%", fill=(240, 210, 140), font=load_font(11))
        d_prop.text((col_xs[4], row_y + 2), status_text, fill=status_color, font=load_font(11))
        row_y += 22

    # 4. 骨骼树面板
    class DummyRigDoc:
        def __init__(self, r):
            self.rig = r
            self.bones = BONES
        def total(self):
            return len(self.rig.elements)

    dg = DummyRigDoc(rig)
    tree_panel = gear_kit.bone_tree_panel(
        dg, min_bones=6, height=six_views_img.height + scale_panel.height + 60
    )

    # 5. 画布总装与导出
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    exp_file = REVIEW_DIR / f"{WEAPON_ID}_exploded.png"
    ref_im = Image.open(ref_file).convert("RGB") if ref_file.exists() else None
    exp_im = Image.open(exp_file).convert("RGB") if exp_file.exists() else None

    lw = 640
    ref_scaled = ref_im.resize((lw, round(ref_im.height * lw / ref_im.width)), Image.Resampling.LANCZOS) if ref_im else None
    exp_scaled = exp_im.resize((lw, round(exp_im.height * lw / exp_im.width)), Image.Resampling.LANCZOS) if exp_im else None

    check_path = REVIEW_DIR / "check.png"
    weapon_review_kit.assemble_check_image(
        six_views_img=six_views_img,
        scale_pose_panel=scale_panel,
        prop_panel=prop_panel,
        tree_panel=tree_panel,
        ref_scaled=ref_scaled,
        exp_scaled=exp_scaled,
        out_path=check_path,
        title="#4 床弩 chuang_nu (Round 1 几何平涂版 · 36×14×28px · 地上摆件无弹药空槽)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
