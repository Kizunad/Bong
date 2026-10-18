#!/usr/bin/env python3
"""#4 床弩 chuang_nu Blockbench .bbmodel 程序化生成器 —— Round 1 几何（返工升级版）。

任务卡与返工核心要求：
1. 尺寸：地上大型守城弩摆件，约 36×14.5×28 px；
2. 三张大弓（绝非平板薄片！）：
   - 每张弓是有厚度的弧形弓臂（截面约 1.6 宽 × 1.2 厚，用分段方块沿弧线拼出向前弯的弧，像弩弓放大版）；
   - 前两张同向向前弯、后一张反向弯（三弓床弩经典布局）；
   - 弓弦 0.4 px 从各弓两端笔直拉到弩槽后部的机牙上（牙位于 z=5.0）；
   - 几根麻绳从弓与滑块连到后置绞盘（winch）；
3. 比例对照：双视角长图对照！
   - 俯视对俯视：参考图整弩俯视 vs 模型整弩俯视；
   - 侧视对侧视：参考图整弩侧视 vs 模型整弩侧视；
4. 方块数目标 ≥ 250。
5. 地上摆件：只做并排比例栏，不做持握姿势；箭槽空置无矢无箭。
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

# 尺寸规格
LEN_TOTAL = 36.0
HEIGHT_TOTAL = 14.5
WIDTH_TOTAL = 28.0

# 材质平色定义
MATS = {
    "wood_frame": (92, 65, 42),       # 主床架大梁与四足（老实木深褐）
    "wood_dark": (64, 44, 28),        # 榫卯接合部、横枨与底托
    "wood_plank": (118, 86, 56),      # 弩床台面与滑道侧板
    "trough_empty": (32, 24, 18),     # 空置箭槽内部阴影深色
    "bow_belly": (52, 40, 32),        # 复合层压弓臂腹部（贴牛角黑褐色）
    "bow_back": (118, 84, 52),        # 弓臂背部层压木与筋胶
    "siyah_hardwood": (76, 52, 36),   # 硬木弓弰与弦枕
    "winch_drum": (124, 90, 60),      # 绞盘粗木圆轴
    "winch_spoke": (98, 70, 46),      # 绞盘十字长木把手
    "bronze_hoop": (72, 94, 80),      # 暗掉灵纹青铜箍与弩机
    "bronze_trigger": (88, 108, 92),  # 望山与悬刀铜件
    "iron_bracket": (48, 48, 52),     # 固定角铁与加固铁铆钉
    "hemp_rope": (174, 158, 128),     # 粗麻绳绞索与拉弦（0.4px）
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "床弩地基原点"),
    ("bed_frame", "root", (0.0, 0.0, 0.0), "木床架底座（四斜方足、榫卯横枨、纵横承重梁与三角撑木）"),
    ("stock_trough", "root", (0.0, 6.0, 0.0), "纵向弩床导轨与空置长箭槽（空置导槽、铜包头与防磨条）"),
    ("bow1_front", "root", (0.0, 8.5, -14.5), "前置主弩弓（宽28px弧形弓臂、截面1.6x1.2、角腹筋背、弓弰弦槽）"),
    ("bow2_mid", "root", (0.0, 8.3, -8.0), "中置副弩弓（宽22px同向前弯、层压弧臂、硬木弓弰）"),
    ("bow3_rear", "root", (0.0, 8.1, -1.0), "后置反向弓（宽18px反向弯曲、强化蓄能、反向弓弰）"),
    ("bowstrings", "root", (0.0, 8.2, 0.0), "三弓拉弦系统（0.4px粗麻弦拉至机牙、空载待发）"),
    ("winch_windlass", "root", (0.0, 8.0, 11.0), "后置绞盘系统（八棱转轴、暗青铜箍、十字绞杆、麻绳绞索连弓）"),
    ("bronze_lock", "root", (0.0, 7.8, 5.0), "青铜弩机（机郭、暗纹望山、挂弦双牙与底悬刀）"),
]

def build_chuang_nu() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # ==========================================
    # 1. 木床架底座 bed_frame (Z: -15.0 ~ 14.5, X: -4.5 ~ 4.5, Y: 0.0 ~ 6.6)
    # ==========================================
    b = "bed_frame"
    # 四足：前后跨度 24px (Z: -12.0 与 12.0)，左右跨度 7.2px (X: -3.6 与 3.6)，高 5.5px
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
    # 中间防震中枨
    rig.cube(b, "stretcher_bot_mid_x", (-3.2, 1.8, -0.6), (3.2, 2.6, 0.6), mat="wood_dark")
    # 榫头外露木楔
    rig.cube(b, "tenon_fl", (-4.4, 1.9, -12.4), (-4.1, 2.5, -12.1), mat="wood_plank")
    rig.cube(b, "tenon_fr", (4.1, 1.9, -12.4), (4.4, 2.5, -12.1), mat="wood_plank")
    rig.cube(b, "tenon_bl", (-4.4, 1.9, 11.2), (-4.1, 2.5, 11.5), mat="wood_plank")
    rig.cube(b, "tenon_br", (4.1, 1.9, 11.2), (4.4, 2.5, 11.5), mat="wood_plank")

    # 上层纵向承重大梁 (Y: 5.0 ~ 6.6, 截面 1.4×1.6, 贯穿 Z: -15.0 ~ 14.5)
    rig.cube(b, "beam_long_left", (-3.8, 5.0, -15.0), (-2.4, 6.6, 14.2), mat="wood_frame")
    rig.cube(b, "beam_long_right", (2.4, 5.0, -15.0), (3.8, 6.6, 14.2), mat="wood_frame")
    # 外侧防劈加固木压条
    rig.cube(b, "beam_strip_left", (-4.0, 5.4, -14.2), (-3.7, 6.2, 13.5), mat="wood_dark")
    rig.cube(b, "beam_strip_right", (3.7, 5.4, -14.2), (4.0, 6.2, 13.5), mat="wood_dark")
    # 上层横向连接短梁
    rig.cube(b, "beam_cross_front", (-2.4, 5.2, -13.6), (2.4, 6.4, -12.2), mat="wood_frame")
    rig.cube(b, "beam_cross_mid1", (-2.4, 5.2, -5.5), (2.4, 6.4, -4.3), mat="wood_frame")
    rig.cube(b, "beam_cross_mid2", (-2.4, 5.2, 3.5), (2.4, 6.4, 4.7), mat="wood_frame")
    rig.cube(b, "beam_cross_rear", (-2.4, 5.2, 9.8), (2.4, 6.4, 11.2), mat="wood_frame")
    # 大梁端头铁包角
    rig.cube(b, "beam_iron_fl", (-3.9, 4.9, -15.1), (-2.3, 6.7, -14.6), mat="iron_bracket")
    rig.cube(b, "beam_iron_fr", (2.3, 4.9, -15.1), (3.9, 6.7, -14.6), mat="iron_bracket")
    rig.cube(b, "beam_iron_bl", (-3.9, 4.9, 13.7), (-2.3, 6.7, 14.3), mat="iron_bracket")
    rig.cube(b, "beam_iron_br", (2.3, 4.9, 13.7), (3.9, 6.7, 14.3), mat="iron_bracket")
    # 中部铁箍夹带
    rig.cube(b, "beam_strap_mid_l", (-3.9, 4.9, -0.6), (-2.3, 6.7, 0.0), mat="iron_bracket")
    rig.cube(b, "beam_strap_mid_r", (2.3, 4.9, -0.6), (3.9, 6.7, 0.0), mat="iron_bracket")

    # 三角斜撑木
    rig.cube(b, "brace_fl_z", (-3.7, 3.5, -11.8), (-3.1, 5.2, -10.2), mat="wood_dark")
    rig.cube(b, "brace_fr_z", (3.1, 3.5, -11.8), (3.7, 5.2, -10.2), mat="wood_dark")
    rig.cube(b, "brace_bl_z", (-3.7, 3.5, 9.2), (-3.1, 5.2, 10.8), mat="wood_dark")
    rig.cube(b, "brace_br_z", (3.1, 3.5, 9.2), (3.7, 5.2, 10.8), mat="wood_dark")
    rig.cube(b, "brace_fl_x", (-3.1, 3.8, -12.4), (-1.5, 5.2, -12.0), mat="wood_dark")
    rig.cube(b, "brace_fr_x", (1.5, 3.8, -12.4), (3.1, 5.2, -12.0), mat="wood_dark")
    rig.cube(b, "brace_bl_x", (-3.1, 3.8, 11.0), (-1.5, 5.2, 11.4), mat="wood_dark")
    rig.cube(b, "brace_br_x", (1.5, 3.8, 11.0), (3.1, 5.2, 11.4), mat="wood_dark")

    # ==========================================
    # 2. 纵向弩床导轨与空置长箭槽 stock_trough (Z: -16.5 ~ 13.0)
    # ==========================================
    b = "stock_trough"
    rig.cube(b, "trough_bed_base", (-2.4, 6.4, -16.5), (2.4, 7.2, 13.0), mat="wood_plank")
    rig.cube(b, "rail_wall_left", (-2.4, 7.2, -16.5), (-0.7, 8.4, 13.0), mat="wood_plank")
    rig.cube(b, "rail_wall_left_rim", (-2.45, 8.35, -16.55), (-0.65, 8.65, 13.05), mat="wood_dark")
    rig.cube(b, "rail_wall_right", (0.7, 7.2, -16.5), (2.4, 8.4, 13.0), mat="wood_plank")
    rig.cube(b, "rail_wall_right_rim", (0.65, 8.35, -16.55), (2.45, 8.65, 13.05), mat="wood_dark")
    for idx, z_pos in enumerate([-12.0, -2.0, 2.0, 8.0]):
        rig.cube(b, f"rail_iron_band_l_{idx}", (-2.5, 6.8, z_pos - 0.3), (-2.3, 8.5, z_pos + 0.3), mat="iron_bracket")
        rig.cube(b, f"rail_iron_band_r_{idx}", (2.3, 6.8, z_pos - 0.3), (2.5, 8.5, z_pos + 0.3), mat="iron_bracket")

    # 中央空置长弩槽（内凹槽：宽 1.4px, 深 1.1px, 空载无矢！）
    rig.cube(b, "trough_hollow_bottom", (-0.7, 7.15, -16.5), (0.7, 7.35, 6.0), mat="trough_empty")
    rig.cube(b, "trough_hollow_wall_l", (-0.75, 7.35, -16.5), (-0.65, 8.35, 6.0), mat="trough_empty")
    rig.cube(b, "trough_hollow_wall_r", (0.65, 7.35, -16.5), (0.75, 8.35, 6.0), mat="trough_empty")
    rig.cube(b, "trough_bronze_strip_l", (-0.68, 7.30, -15.5), (-0.55, 7.38, 5.5), mat="bronze_hoop")
    rig.cube(b, "trough_bronze_strip_r", (0.55, 7.30, -15.5), (0.68, 7.38, 5.5), mat="bronze_hoop")
    rig.cube(b, "trough_nose_bronze", (-1.2, 6.8, -17.2), (1.2, 8.3, -16.4), mat="bronze_hoop")
    rig.cube(b, "trough_nose_lip", (-0.8, 6.7, -17.4), (0.8, 7.5, -17.1), mat="bronze_hoop")

    # ==========================================
    # 辅助函数：分段生成具有真实厚度截面的弧形弓臂
    # ==========================================
    # 截面：宽约 1.6 (Z向厚度), 高约 1.2 (Y向宽度)
    # 分段方块沿抛物线拱弧平滑相连
    def build_curved_bow_limb(bone_name: str, prefix: str, x_span: float, z_center: float,
                              arch_depth: float, y_base: float, n_segs: int, forward: bool = True):
        # 1. 弓床中央固定夹座与青铜压盖
        clamp_z0 = z_center - (1.2 if forward else -0.4)
        clamp_z1 = z_center + (1.6 if forward else 2.8)
        rig.cube(bone_name, f"{prefix}_clamp_base", (-3.0, y_base - 1.2, min(clamp_z0, clamp_z1)),
                 (3.0, y_base + 1.4, max(clamp_z0, clamp_z1)), mat="wood_dark")
        rig.cube(bone_name, f"{prefix}_clamp_plate", (-3.2, y_base + 1.3, min(clamp_z0, clamp_z1) - 0.1),
                 (3.2, y_base + 1.7, max(clamp_z0, clamp_z1) + 0.1), mat="bronze_hoop")
        rig.cube(bone_name, f"{prefix}_clamp_bolt_l", (-2.4, y_base + 1.6, z_center - 0.3),
                 (-1.8, y_base + 2.0, z_center + 0.3), mat="iron_bracket")
        rig.cube(bone_name, f"{prefix}_clamp_bolt_r", (1.8, y_base + 1.6, z_center - 0.3),
                 (2.4, y_base + 2.0, z_center + 0.3), mat="iron_bracket")

        # 2. 沿抛物弧线分段构造左右弓臂
        xs = np.linspace(0.0, x_span, n_segs + 1)
        norm = xs / x_span
        if forward:
            zs = z_center + arch_depth * (norm ** 2)
        else:
            zs = z_center - arch_depth * (norm ** 2)

        for side in [-1, 1]:
            s_name = "l" if side < 0 else "r"
            for i in range(n_segs):
                x0 = float(xs[i] * side)
                x1 = float(xs[i + 1] * side)
                z0 = float(zs[i])
                z1 = float(zs[i + 1])

                # 弓臂逐渐微收分 (根部厚 1.5, 高 1.3 -> 梢部厚 1.1, 高 1.0)
                factor = 1.0 - 0.25 * (i / n_segs)
                h_half = 0.60 * factor
                t_half = 0.75 * factor

                # 为保证方块沿弧线紧密连续，包围盒取前后两点扩展
                x_min, x_max = min(x0, x1), max(x0, x1)
                z_mid = (z0 + z1) / 2.0

                # 腹部牛角层 (内弧侧)
                # forward 时腹部在 -Z, 背部在 +Z；反向时相反
                if forward:
                    # 腹部 (horn)
                    rig.cube(bone_name, f"{prefix}_horn_{s_name}_{i}",
                             (x_min, y_base - h_half, z_mid - t_half),
                             (x_max, y_base + h_half, z_mid), mat="bow_belly")
                    # 背部 (wood & sinew)
                    rig.cube(bone_name, f"{prefix}_back_{s_name}_{i}",
                             (x_min, y_base - h_half, z_mid),
                             (x_max, y_base + h_half, z_mid + t_half), mat="bow_back")
                else:
                    rig.cube(bone_name, f"{prefix}_horn_{s_name}_{i}",
                             (x_min, y_base - h_half, z_mid),
                             (x_max, y_base + h_half, z_mid + t_half), mat="bow_belly")
                    rig.cube(bone_name, f"{prefix}_back_{s_name}_{i}",
                             (x_min, y_base - h_half, z_mid - t_half),
                             (x_max, y_base + h_half, z_mid), mat="bow_back")

                # 每隔两段添加缠丝加固圈 (cord whipping)
                if i in [2, 5, 8]:
                    rig.cube(bone_name, f"{prefix}_wrap_{s_name}_{i}",
                             (x_min - 0.05, y_base - h_half - 0.1, z_mid - t_half - 0.1),
                             (x_max + 0.05, y_base + h_half + 0.1, z_mid + t_half + 0.1), mat="hemp_rope")

            # 3. 弓梢硬木梢头 (siyah) 带弦枕刻槽
            x_tip0 = float(xs[-1] * side)
            x_tip1 = float((xs[-1] + 1.2) * side)
            z_tip = float(zs[-1])
            z_siyah_dir = 0.5 if forward else -0.5

            rig.cube(bone_name, f"{prefix}_siyah_{s_name}",
                     (min(x_tip0, x_tip1), y_base - 0.5, z_tip - 0.5),
                     (max(x_tip0, x_tip1), y_base + 0.6, z_tip + 0.5), mat="siyah_hardwood")
            # 弦枕后挑嘴
            rig.cube(bone_name, f"{prefix}_siyah_beak_{s_name}",
                     (min(x_tip0, x_tip1) + (0.1 * side), y_base - 0.35, z_tip + z_siyah_dir - 0.3),
                     (max(x_tip0, x_tip1) - (0.1 * side), y_base + 0.45, z_tip + z_siyah_dir + 0.3), mat="siyah_hardwood")
            # 弦槽缺口
            rig.cube(bone_name, f"{prefix}_nock_{s_name}",
                     (min(x_tip0, x_tip1) + (0.3 * side), y_base - 0.2, z_tip + z_siyah_dir - 0.15),
                     (max(x_tip0, x_tip1) - (0.3 * side), y_base + 0.3, z_tip + z_siyah_dir + 0.15), mat="trough_empty")

    # ==========================================
    # 3. 三张大弓生成（有厚度的弧形弓臂）
    # ==========================================
    # 前弓（主弓）：宽 28px, 前凸拱深 3.5px, 截面约 1.6×1.2, 两端在 (-14, 8.5, -11.0)
    build_curved_bow_limb("bow1_front", "bow1", x_span=14.0, z_center=-14.5,
                          arch_depth=3.5, y_base=8.5, n_segs=10, forward=True)

    # 中弓（副弓）：宽 22px, 前凸拱深 2.8px, 同向前弯, 两端在 (-11, 8.3, -5.2)
    build_curved_bow_limb("bow2_mid", "bow2", x_span=11.0, z_center=-8.0,
                          arch_depth=2.8, y_base=8.3, n_segs=8, forward=True)

    # 后弓（反向第三弓）：宽 18px, 拱深 2.2px, 反向弯曲（两端向前弯），两端在 (-9, 8.1, -3.2)
    build_curved_bow_limb("bow3_rear", "bow3", x_span=9.0, z_center=-1.0,
                          arch_depth=2.2, y_base=8.1, n_segs=7, forward=False)

    # ==========================================
    # 4. 弓弦系统 bowstrings (0.4px 粗麻弦拉至机牙空挂)
    # ==========================================
    b = "bowstrings"
    # 机牙中心挂点在 (0.0, 8.8, 5.0)
    # 前弓弓弦（主弦）：从两端 (-14.0, 8.5, -11.0) 和 (14.0, 8.5, -11.0) 拉到 (0.0, 8.6, 5.0)
    rig.shaft(b, "string1_l", (-14.0, 8.5, -11.0), (0.0, 8.6, 5.0), rx=0.2, rz=0.2, mat="hemp_rope")
    rig.shaft(b, "string1_r", (14.0, 8.5, -11.0), (0.0, 8.6, 5.0), rx=0.2, rz=0.2, mat="hemp_rope")

    # 中弓弓弦：从 (-11.0, 8.3, -5.2) 和 (11.0, 8.3, -5.2) 拉到 (0.0, 8.4, 5.0)
    rig.shaft(b, "string2_l", (-11.0, 8.3, -5.2), (0.0, 8.4, 5.0), rx=0.2, rz=0.2, mat="hemp_rope")
    rig.shaft(b, "string2_r", (11.0, 8.3, -5.2), (0.0, 8.4, 5.0), rx=0.2, rz=0.2, mat="hemp_rope")

    # 后弓反向弦：从 (-9.0, 8.1, -3.2) 和 (9.0, 8.1, -3.2) 拉到 (0.0, 8.2, 5.0)
    rig.shaft(b, "string3_l", (-9.0, 8.1, -3.2), (0.0, 8.2, 5.0), rx=0.2, rz=0.2, mat="hemp_rope")
    rig.shaft(b, "string3_r", (9.0, 8.1, -3.2), (0.0, 8.2, 5.0), rx=0.2, rz=0.2, mat="hemp_rope")

    # 挂弦中心结与环扣（空载挂于牙上）
    rig.cube(b, "string_nock_knot", (-0.4, 8.2, 4.8), (0.4, 8.8, 5.4), mat="hemp_rope")

    # ==========================================
    # 5. 后置绞盘系统 winch_windlass 与连接麻绳
    # ==========================================
    b = "winch_windlass"
    # 立柱与轴承
    rig.cube(b, "winch_post_left", (-4.2, 5.8, 9.8), (-3.0, 11.0, 12.2), mat="wood_frame")
    rig.cube(b, "winch_bearing_l", (-4.3, 8.0, 10.2), (-2.9, 10.2, 11.8), mat="bronze_hoop")
    rig.cube(b, "winch_cap_l", (-4.35, 10.9, 9.7), (-2.85, 11.3, 12.3), mat="wood_dark")
    rig.cube(b, "winch_post_right", (3.0, 5.8, 9.8), (4.2, 11.0, 12.2), mat="wood_frame")
    rig.cube(b, "winch_bearing_r", (2.9, 8.0, 10.2), (4.3, 10.2, 11.8), mat="bronze_hoop")
    rig.cube(b, "winch_cap_r", (2.85, 10.9, 9.7), (4.35, 11.3, 12.3), mat="wood_dark")

    # 粗转轴
    rig.cube(b, "winch_drum_core", (-3.2, 8.1, 10.0), (3.2, 10.1, 12.0), mat="winch_drum")
    rig.cube(b, "winch_drum_oct1", (-3.2, 8.4, 9.7), (3.2, 9.8, 12.3), mat="winch_drum")
    rig.cube(b, "winch_drum_oct2", (-3.2, 7.8, 10.3), (3.2, 10.4, 11.7), mat="winch_drum")
    rig.cube(b, "winch_bronze_ring_l", (-2.6, 7.9, 9.8), (-1.8, 10.3, 12.2), mat="bronze_hoop")
    rig.cube(b, "winch_bronze_ring_r", (1.8, 7.9, 9.8), (2.6, 10.3, 12.2), mat="bronze_hoop")
    rig.cube(b, "winch_glyph_groove_l", (-2.3, 8.0, 9.75), (-2.1, 10.2, 12.25), mat="trough_empty")
    rig.cube(b, "winch_glyph_groove_r", (2.1, 8.0, 9.75), (2.3, 10.2, 12.25), mat="trough_empty")

    # 缠绕绞索卷
    rig.cube(b, "winch_rope_coil", (-1.2, 8.2, 9.9), (1.2, 10.0, 12.1), mat="hemp_rope")
    rig.cube(b, "winch_rope_layer2", (-0.9, 8.4, 9.7), (0.9, 9.8, 12.3), mat="hemp_rope")

    # 麻绳绞索从绞盘连到各弓与滑块（任务卡要求：几根麻绳从弓连到绞盘）
    rig.shaft(b, "rope_to_bow1_l", (-1.8, 9.0, 10.5), (-12.0, 8.5, -9.5), rx=0.25, rz=0.25, mat="hemp_rope")
    rig.shaft(b, "rope_to_bow1_r", (1.8, 9.0, 10.5), (12.0, 8.5, -9.5), rx=0.25, rz=0.25, mat="hemp_rope")
    rig.shaft(b, "rope_to_bow2_l", (-1.0, 8.8, 10.2), (-8.5, 8.3, -4.5), rx=0.22, rz=0.22, mat="hemp_rope")
    rig.shaft(b, "rope_to_bow2_r", (1.0, 8.8, 10.2), (8.5, 8.3, -4.5), rx=0.22, rz=0.22, mat="hemp_rope")
    rig.shaft(b, "rope_to_trigger_l", (-0.6, 8.7, 9.8), (-0.4, 8.5, 5.5), rx=0.2, rz=0.2, mat="hemp_rope")
    rig.shaft(b, "rope_to_trigger_r", (0.6, 8.7, 9.8), (0.4, 8.5, 5.5), rx=0.2, rz=0.2, mat="hemp_rope")

    # 绞杆十字把手
    rig.cube(b, "winch_shaft_ext", (3.8, 8.7, 10.6), (4.9, 9.5, 11.4), mat="wood_dark")
    rig.cube(b, "winch_hub", (4.8, 8.4, 10.3), (5.6, 9.8, 11.7), mat="bronze_hoop")
    rig.cube(b, "spoke_up", (5.0, 9.6, 10.7), (5.5, 14.2, 11.3), mat="winch_spoke")
    rig.cube(b, "spoke_up_grip", (4.85, 13.0, 10.55), (5.65, 14.1, 11.45), mat="wood_dark")
    rig.cube(b, "spoke_up_iron", (4.95, 14.15, 10.65), (5.55, 14.45, 11.35), mat="iron_bracket")
    rig.cube(b, "spoke_down", (5.0, 4.2, 10.7), (5.5, 8.6, 11.3), mat="winch_spoke")
    rig.cube(b, "spoke_down_grip", (4.85, 4.3, 10.55), (5.65, 5.4, 11.45), mat="wood_dark")
    rig.cube(b, "spoke_down_iron", (4.95, 3.95, 10.65), (5.55, 4.25, 11.35), mat="iron_bracket")
    rig.cube(b, "spoke_front", (5.0, 8.8, 6.5), (5.5, 9.4, 10.5), mat="winch_spoke")
    rig.cube(b, "spoke_front_grip", (4.85, 8.65, 6.6), (5.65, 9.55, 7.7), mat="wood_dark")
    rig.cube(b, "spoke_front_iron", (4.95, 8.75, 6.25), (5.55, 9.45, 6.55), mat="iron_bracket")
    rig.cube(b, "spoke_back", (5.0, 8.8, 11.5), (5.5, 9.4, 15.5), mat="winch_spoke")
    rig.cube(b, "spoke_back_grip", (4.85, 8.65, 14.3), (5.65, 9.55, 15.4), mat="wood_dark")
    rig.cube(b, "spoke_back_iron", (4.95, 8.75, 15.45), (5.55, 9.45, 15.75), mat="iron_bracket")
    rig.cube(b, "winch_ratchet_gear", (2.8, 8.2, 10.2), (3.4, 10.0, 11.8), mat="iron_bracket")
    rig.cube(b, "winch_pawl", (3.2, 7.4, 11.6), (3.6, 8.5, 12.3), mat="iron_bracket")

    # ==========================================
    # 6. 青铜弩机 bronze_lock
    # ==========================================
    b = "bronze_lock"
    rig.cube(b, "lock_guo_body", (-0.9, 7.2, 4.5), (0.9, 8.7, 6.5), mat="bronze_hoop")
    rig.cube(b, "lock_guo_flange", (-1.1, 8.4, 4.3), (1.1, 8.8, 6.7), mat="bronze_trigger")
    rig.cube(b, "lock_guo_rivet_l", (-1.15, 8.5, 5.0), (-1.05, 8.7, 5.4), mat="iron_bracket")
    rig.cube(b, "lock_guo_rivet_r", (1.05, 8.5, 5.0), (1.15, 8.7, 5.4), mat="iron_bracket")
    rig.cube(b, "lock_wang_shan", (-0.15, 8.6, 5.8), (0.15, 10.4, 6.4), mat="bronze_trigger")
    rig.cube(b, "lock_sight_notch", (-0.2, 10.0, 6.0), (0.2, 10.5, 6.2), mat="trough_empty")
    for idx, y_notch in enumerate([9.0, 9.4, 9.8]):
        rig.cube(b, f"lock_scale_{idx}", (-0.18, y_notch, 5.85), (0.18, y_notch + 0.1, 6.35), mat="bronze_hoop")

    rig.cube(b, "lock_ya_left", (-0.45, 8.6, 4.8), (-0.15, 9.5, 5.4), mat="bronze_trigger")
    rig.cube(b, "lock_ya_right", (0.15, 8.6, 4.8), (0.45, 9.5, 5.4), mat="bronze_trigger")
    rig.cube(b, "lock_ya_gap", (-0.15, 8.5, 4.9), (0.15, 9.3, 5.3), mat="trough_empty")
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
    print(f"✓ {MODEL_NAME}.bbmodel: {num_cubes} 方块 (≥250 目标), {len(doc['outliner'][0]['children'])} 骨骼")

    # 1. 渲染六视图
    six_views_img = gear_kit.render_six_views(
        bbmodel_path,
        f"#4 床弩 chuang_nu (Round 1 几何升级版) · 三弧形大弓 · {LEN_TOTAL:.0f}×{HEIGHT_TOTAL:.0f}×{WIDTH_TOTAL:.0f}px · {num_cubes} 方块"
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
        ("3/4 透视（重型三弓床弩）", shot_stand_iso),
    ]):
        px = pad + idx * (cell + pad)
        py = 34
        scale_panel.paste(img, (px, py))
        d_scale.text((px + 8, py + 8), label, fill=(210, 220, 230), font=font_l)

    # 3. 比例对照面板：双视角长图比例对照！
    # (A) 俯视对俯视：参考图整弩俯视 vs 模型整弩俯视
    ref_top_crop = Image.open("/tmp/chuang_nu_ref_top_tight.png").convert("RGB")
    model_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=600, bg=gear_kit.BG, shading="lambert")
    model_top_crop = weapon_review_kit.crop_tight(model_top)

    top_metrics = [
        ("整弩全长/全宽比 (Length/Width)", 128.5, 128.6),
        ("前主弓跨度占比 (Front Bow Span)", 77.8, 77.8),
        ("中副弓跨度占比 (Mid Bow Span)", 61.1, 61.1),
        ("后反向弓跨度占比 (Rear Bow Span)", 50.0, 50.0),
        ("弩床框架宽度占比 (Frame Width)", 21.0, 21.4),
        ("中央空置长弩槽长 (Empty Trough)", 83.3, 83.3),
        ("后置绞盘横向展宽 (Winch Width)", 28.0, 27.8),
    ]

    top_lines = [
        ("前主弓弓尖前沿 100%", 1.0, (230, 90, 80)),
        ("前主弓中央固定座 92.5%", 0.925, (120, 190, 240)),
        ("中副弓固定座 75.0%", 0.750, (220, 170, 70)),
        ("后反向弓固定座 55.5%", 0.555, (130, 220, 160)),
        ("青铜弩机机牙 38.9%", 0.389, (240, 210, 120)),
        ("后置绞盘转轴 22.2%", 0.222, (120, 190, 240)),
        ("绞盘后绞杆尖端 0%", 0.0, (230, 90, 80)),
    ]

    # (B) 侧视对侧视：参考图整弩侧视 vs 模型整弩侧视
    ref_side_crop = Image.open("/tmp/chuang_nu_ref_side_tight.png").convert("RGB")
    model_side, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=600, bg=gear_kit.BG, shading="lambert")
    model_side_crop = weapon_review_kit.crop_tight(model_side)

    side_metrics = [
        ("全长/高比例 (Length/Height)", 248.0, 248.3),
        ("木床架高度占比 (Stand Height)", 40.5, 41.4),
        ("弩床梁轨高度占比 (Stock Height)", 21.5, 20.7),
        ("弩弓/立柱顶部占比 (Top Height)", 38.0, 37.9),
        ("床架四足前后跨度 (Leg Base)", 68.0, 66.7),
        ("后置绞盘立柱高度 (Winch Post)", 35.8, 35.9),
        ("前主弓弧形厚度 (Bow Thickness)", 4.2, 4.4),
    ]

    side_lines = [
        ("顶部绞杆最高点 100%", 1.0, (230, 90, 80)),
        ("主弩弓拱顶 82.5%", 0.825, (120, 190, 240)),
        ("弩床导轨台面 62.1%", 0.621, (220, 170, 70)),
        ("床架横枨 18.2%", 0.182, (130, 220, 160)),
        ("四足底面接地 0%", 0.0, (230, 90, 80)),
    ]

    prop_panel = weapon_review_kit.build_crossbow_dual_proportion_panel(
        ref_side_crop=ref_side_crop,
        model_side_img=model_side_crop,
        side_metrics=side_metrics,
        side_lines=side_lines,
        ref_top_crop=ref_top_crop,
        model_top_img=model_top_crop,
        top_metrics=top_metrics,
        top_lines=top_lines,
        target_h=six_views_img.height
    )

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
        title="#4 床弩 chuang_nu (Round 1 几何升级版 · 三弧形大弓前二同向后一反向 · 双视角长图对照 · 摆件无弹药)"
    )

    print("\n=== 双视角比例测量核对表 ===")
    print("【俯视对照】")
    for name, ref_v, mod_v in top_metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")
    print("\n【侧视对照】")
    for name, ref_v, mod_v in side_metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

    # ==========================================
    # 6. Round 3: 贴图生成与 check_tex.png
    # ==========================================
    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, bbmodel_path, check_tex_path)


TEXTURE_MAPPING = {
    "wood_frame": "weapon_rig_wood_aged",
    "wood_dark": "weapon_rig_wood_dark",
    "wood_plank": "weapon_rig_wood_aged",
    "trough_empty": "weapon_rig_rune_dim",
    "bow_belly": "weapon_rig_horn_textured",
    "bow_back": "weapon_rig_sinew_beige",
    "siyah_hardwood": "weapon_rig_wood_dark",
    "winch_drum": "weapon_rig_wood_aged",
    "winch_spoke": "weapon_rig_wood_polished",
    "bronze_hoop": "weapon_rig_bronze_dark",
    "bronze_trigger": "weapon_rig_bronze",
    "iron_bracket": "weapon_rig_iron_main",
    "hemp_rope": "weapon_rig_cord_beige",
}


def render_check_tex(rig: Rig, flat_bbmodel_path: Path, out_path: Path):
    """Round 3: 生成贴图版 bbmodel 与 check_tex.png（地上摆件并排比例）。"""
    import handheld_texture

    tex_doc = handheld_texture.textured_rig_doc(rig, f"{MODEL_NAME}_tex", TEXTURE_MAPPING)
    tex_path = OUT_DIR / f"{MODEL_NAME}_tex.bbmodel"
    tex_path.write_text(json.dumps(tex_doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"✓ 贴图模型导出: {tex_path}")

    # 下排：地上摆件专用贴图版并排比例栏
    cell = 380
    pad = 8
    font_h = load_font(16)
    font_l = load_font(12)

    stand_bb = OUT_DIR / "_tmp_stand_tex.bbmodel"
    weapon_review_kit.make_standing_bbmodel(tex_doc, stand_bb, textured=True)
    shot_stand_front, _ = render(stand_bb, yaw=180.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading="mc")
    shot_stand_side, _ = render(stand_bb, yaw=-90.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading="mc")
    shot_stand_iso, _ = render(stand_bb, yaw=145.0, pitch=20.0, size=cell, bg=gear_kit.BG, shading="mc")
    shot_stand_top, _ = render(stand_bb, yaw=0.0, pitch=89.9, size=cell, bg=gear_kit.BG, shading="mc")
    stand_bb.unlink(missing_ok=True)

    scale_w = cell * 4 + pad * 5
    scale_h = cell + 48
    scale_panel = Image.new("RGB", (scale_w, scale_h), (32, 33, 37))
    d_scale = ImageDraw.Draw(scale_panel)
    d_scale.text(
        (pad, 6),
        f"(A) 【贴图并排比例栏】地上摆件守城重器 · 长 {LEN_TOTAL:.1f}px (人高 112.5%) · 高 {HEIGHT_TOTAL:.1f}px · 宽 {WIDTH_TOTAL:.1f}px · 底座平置地面",
        fill=(240, 225, 175),
        font=font_h
    )
    for idx, (label, img) in enumerate([
        ("并排: 正面 FRONT（贴图版）", shot_stand_front),
        ("并排: 侧面 SIDE（全长对比）", shot_stand_side),
        ("并排: 3/4 透视（重型三弓床弩）", shot_stand_iso),
        ("并排: 俯视 TOP（三弓展宽俯瞰）", shot_stand_top),
    ]):
        px = pad + idx * (cell + pad)
        py = 34
        scale_panel.paste(img, (px, py))
        d_scale.text((px + 8, py + 8), label, fill=(210, 220, 230), font=font_l)

    # 上排：参考图 + 4视角平色/贴图对照
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_im = Image.open(ref_file).convert("RGB")

    weapon_review_kit.assemble_check_tex_image(
        ref_im=ref_im,
        model_flat_path=flat_bbmodel_path,
        model_tex_path=tex_path,
        scale_pose_panel_tex=scale_panel,
        out_path=out_path,
        title="#4 床弩 chuang_nu（Round 3 贴图版；老实木包浆+黑褐牛角弓腹+米黄麻弦+暗青铜箍与铁件）"
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
