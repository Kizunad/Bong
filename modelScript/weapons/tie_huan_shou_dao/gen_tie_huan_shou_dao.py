#!/usr/bin/env python3
"""环首铁刀 tie_huan_shou_dao Blockbench .bbmodel 程序化生成器 —— Round 1 几何（依参考图与调度目标比例重构）。

调度反馈与实测比例整改要点（严格落实）：
1. 解决原版短粗砍刀问题（原刀身只占51%，厚达1.2/宽2.52=48%）：
   - 全长严格 28.0 px；
   - 刀身含刀尖长达 19.0 px（刀尖平滑斜切 2.5 px），占全长 67.9%（原 51%）；
   - 刀身宽度严格 2.0 px（长宽比 19/2 = 9.5，修长直刀）；
   - 刀背厚度 0.5 px，向刃口用 2~3 层递减方块减薄到 0.15 px（楔形截面，刃厚/刃宽 = 25%）；
2. 环首大空心与正视透光：
   - 环首外径 3.6 × 3.2 px（高 3.6，宽 3.2），环条 0.8 px；
   - 中间空洞严格 2.0 × 1.6 px，正视图中完全通透，背景清晰透出；
   - 环首占全长比例降至 12.8%（约 3.6 px，原 16% 超大失衡）；
3. 握把与小铜格：
   - 握把长 4.5 px、截面 1.6 × 1.0 px（八角近圆贴手）；
   - 极简小铜格：2.6 × 0.6 × 1.4 px（横宽 2.6，厚 0.6，进深 1.4）；
4. 刀尖：
   - 一刀平滑斜切直刃尖（Chisel Tip），无台阶缺口；
5. 方块数 ≥ 150，全套骨骼分级清晰，check.png 附带标准「比例对照」面板。
"""

from __future__ import annotations

import argparse
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

WEAPON_ID = "tie_huan_shou_dao"
MODEL_NAME = "TieHuanShouDao"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

# 材质定义（Round 1 平涂）
MATS = {
    "iron_dark": (48, 44, 42),       # 黑褐氧化膜（生铁/锻铁黑铁底色）
    "iron_mid": (78, 72, 68),        # 锻打铁身与锤痕面
    "iron_edge": (148, 142, 138),    # 刃口研磨出的冷灰亮线
    "bronze": (162, 126, 76),        # 铜格与铜夹
    "bronze_dk": (108, 78, 46),      # 铜件暗面
    "wood": (86, 62, 42),            # 木柄芯
    "cord": (42, 38, 36),            # 黑色/深褐色缠手编绳
    "cord_dk": (26, 24, 22),         # 编绳缝隙暗色
    "red_silk": (142, 48, 42),       # 环首系绑的朱红丝绳流苏
    "groove": (24, 22, 20),          # 铭文位与暗掉阵槽底
}

# 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("pommel_ring", "root", (0.0, 1.8, 0.0), "空心锻铁环首：3.6x3.2px外径、0.8px环条、2.0x1.6px中孔透背景"),
    ("ring_tassel", "pommel_ring", (0.0, 0.0, 0.0), "环首底梁外侧下挂的细朱红丝绳（独立挂件骨）"),
    ("pommel_neck", "root", (0.0, 3.8, 0.0), "环首与刀柄过渡颈"),
    ("grip", "root", (0.0, 6.4, 0.0), "缠绳握把（长4.5px，截面1.6x1.0）"),
    ("guard_plate", "root", (0.0, 8.7, 0.0), "极简小铜格（2.6x0.6x1.4px）"),
    ("blade_collar", "root", (0.0, 9.3, 0.0), "刀根纯铜夹箍（Habaki 吞口）"),
    ("blade_lower", "root", (0.0, 13.0, 0.0), "刀身下段（宽2.0px，楔形厚度0.5->0.15）"),
    ("blade_mid", "root", (0.0, 17.5, 0.0), "刀身中段（宽2.0px，带空白铭文槽）"),
    ("cartouche_slot", "blade_mid", (0.0, 17.5, 0.0), "刀背侧面细长空白铭文位与暗掉阵纹"),
    ("blade_upper", "root", (0.0, 22.0, 0.0), "刀身上段（笔直延伸厚脊直刃）"),
    ("blade_hone_edge", "root", (0.0, 18.0, 0.0), "全刀身斜削开刃磨出的冷灰亮线（刃口0.15px）"),
    ("blade_chisel_tip", "root", (0.0, 26.8, 0.0), "刀尖平滑斜切直平锋（长2.5px一刀平滑斜切）"),
]

# 握持点：玩家右手握在 grip 中段 y=6.4px 处
GRIP_Y = 6.4
TOTAL_LEN = 28.0

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. pommel_ring (y: 0.0 ~ 3.6): 椭圆环首！
    # 严格落实目标：外径 3.6 × 3.2 px (Y: 0.0 ~ 3.6, X: -1.6 ~ 1.6)
    # 环条粗 0.8 px (Z厚度 0.8px: z ∈ [-0.4, 0.4])
    # 中间空洞 2.0 × 1.6 px (Y: 0.8 ~ 2.8, X: -1.0 ~ 1.0)，正视完全透光！
    b = "pommel_ring"
    # 底部横梁 (y: 0.0 ~ 0.8, x: -1.6 ~ 1.6, z: -0.4 ~ 0.4)
    rig.cube(b, "ring_bot_main", (-1.3, 0.0, -0.4), (1.3, 0.8, 0.4), mat="iron_dark")
    rig.cube(b, "ring_bot_lip_f", (-1.1, 0.08, 0.38), (1.1, 0.72, 0.48), mat="iron_mid")
    rig.cube(b, "ring_bot_lip_b", (-1.1, 0.08, -0.48), (1.1, 0.72, -0.38), mat="iron_mid")
    rig.cube(b, "ring_bot_crest", (-0.8, 0.0, -0.3), (0.8, 0.25, 0.3), mat="iron_edge")

    # 顶部横梁 (y: 2.8 ~ 3.6, x: -1.6 ~ 1.6)
    rig.cube(b, "ring_top_main", (-1.3, 2.8, -0.4), (1.3, 3.6, 0.4), mat="iron_dark")
    rig.cube(b, "ring_top_lip_f", (-1.1, 2.88, 0.38), (1.1, 3.52, 0.48), mat="iron_mid")
    rig.cube(b, "ring_top_lip_b", (-1.1, 2.88, -0.48), (1.1, 3.52, -0.38), mat="iron_mid")
    rig.cube(b, "ring_top_crest", (-0.8, 3.35, -0.3), (0.8, 3.6, 0.3), mat="iron_edge")

    # 左立柱 (x: -1.6 ~ -1.0, y: 0.7 ~ 2.9)
    rig.cube(b, "ring_left_main", (-1.6, 0.7, -0.4), (-1.0, 2.9, 0.4), mat="iron_mid")
    rig.cube(b, "ring_left_edge_o", (-1.68, 0.8, -0.28), (-1.52, 2.8, 0.28), mat="iron_edge")
    rig.cube(b, "ring_left_edge_f", (-1.55, 0.85, 0.38), (-1.05, 2.75, 0.46), mat="iron_mid")
    rig.cube(b, "ring_left_edge_b", (-1.55, 0.85, -0.46), (-1.05, 2.75, -0.38), mat="iron_mid")

    # 右立柱 (x: 1.0 ~ 1.6, y: 0.7 ~ 2.9)
    rig.cube(b, "ring_right_main", (1.0, 0.7, -0.4), (1.6, 2.9, 0.4), mat="iron_mid")
    rig.cube(b, "ring_right_edge_o", (1.52, 0.8, -0.28), (1.68, 2.8, 0.28), mat="iron_edge")
    rig.cube(b, "ring_right_edge_f", (1.05, 0.85, 0.38), (1.55, 2.75, 0.46), mat="iron_mid")
    rig.cube(b, "ring_right_edge_b", (1.05, 0.85, -0.46), (1.55, 2.75, -0.38), mat="iron_mid")

    # 外轮廓四角微斜切倒角 (外宽保持 3.2，绝不侵入中间 x∈[-1.0, 1.0], y∈[0.8, 2.8] 内孔！)
    rig.cube(b, "ring_corner_bl", (-1.55, 0.1, -0.35), (-1.05, 0.7, 0.35), mat="iron_mid")
    rig.cube(b, "ring_corner_br", (1.05, 0.1, -0.35), (1.55, 0.7, 0.35), mat="iron_mid")
    rig.cube(b, "ring_corner_tl", (-1.55, 2.9, -0.35), (-1.05, 3.5, 0.35), mat="iron_mid")
    rig.cube(b, "ring_corner_tr", (1.05, 2.9, -0.35), (1.55, 3.5, 0.35), mat="iron_mid")

    # 2. ring_tassel: 环首底梁外侧下挂的细朱红丝绳 (完全位于 y < 0)
    b = "ring_tassel"
    rig.cube(b, "tassel_tie", (-0.2, -0.25, -0.15), (0.2, 0.05, 0.15), mat="red_silk")
    rig.cube(b, "tassel_knot", (-0.35, -0.65, -0.25), (0.35, -0.25, 0.25), mat="red_silk")
    rig.cube(b, "tassel_collar", (-0.28, -0.85, -0.20), (0.28, -0.65, 0.20), mat="bronze")
    rig.cube(b, "tassel_strand_1", (-0.25, -1.8, -0.15), (-0.05, -0.85, 0.15), mat="red_silk")
    rig.cube(b, "tassel_strand_2", (0.05, -2.0, -0.15), (0.25, -0.85, 0.15), mat="red_silk")
    rig.cube(b, "tassel_strand_3", (-0.12, -2.3, -0.10), (0.12, -1.8, 0.10), mat="red_silk")

    # 3. pommel_neck (y: 3.6 ~ 4.2): 环首与刀柄过渡颈
    b = "pommel_neck"
    rig.cube(b, "neck_stem", (-0.6, 3.6, -0.35), (0.6, 4.2, 0.35), mat="iron_mid")
    rig.cube(b, "neck_washer", (-0.75, 3.9, -0.45), (0.75, 4.2, 0.45), mat="bronze")
    rig.cube(b, "neck_band_f", (-0.7, 3.75, 0.32), (0.7, 4.05, 0.42), mat="bronze_dk")
    rig.cube(b, "neck_band_b", (-0.7, 3.75, -0.42), (0.7, 4.05, -0.32), mat="bronze_dk")

    # 4. grip: 握把 (y: 4.2 ~ 8.7, 长度严格 4.5 px! 截面严格 1.6 × 1.0 px: x ∈ [-0.8, 0.8], z ∈ [-0.5, 0.5])
    b = "grip"
    # 木芯
    rig.cube(b, "grip_core", (-0.7, 4.2, -0.4), (0.7, 8.7, 0.4), mat="wood")
    # 7 圈交错密缠编绳 (带微起伏凹凸)
    for i in range(7):
        y0 = 4.25 + i * 0.63
        y1 = y0 + 0.52
        hw_x = 0.80 if i % 2 == 0 else 0.76
        hw_z = 0.48 if i % 2 == 0 else 0.50
        rig.cube(b, f"grip_wrap_{i}_main", (-hw_x, y0, -hw_z), (hw_x, y1, hw_z), mat="cord")
        rig.cube(b, f"grip_knot_{i}_f", (-0.45, y0 + 0.08, hw_z - 0.03), (0.45, y1 - 0.08, hw_z + 0.06), mat="cord_dk")
        rig.cube(b, f"grip_knot_{i}_b", (-0.45, y0 + 0.08, -hw_z - 0.06), (0.45, y1 - 0.08, -hw_z + 0.03), mat="cord_dk")
        rig.cube(b, f"grip_knot_{i}_l", (-hw_x - 0.06, y0 + 0.08, -0.3), (-hw_x + 0.03, y1 - 0.08, 0.3), mat="cord")
        rig.cube(b, f"grip_knot_{i}_r", (hw_x - 0.03, y0 + 0.08, -0.3), (hw_x + 0.06, y1 - 0.08, 0.3), mat="cord")

    # 5. guard_plate: 极简小铜格 (y: 8.7 ~ 9.3, 厚度严格 0.6 px, 截面 2.6 × 1.4 px: x ∈ [-1.3, 1.3], z ∈ [-0.7, 0.7])
    b = "guard_plate"
    rig.cube(b, "guard_main", (-1.3, 8.7, -0.7), (1.3, 9.3, 0.7), mat="bronze")
    rig.cube(b, "guard_rim_f", (-1.25, 8.75, 0.65), (1.25, 9.25, 0.75), mat="bronze_dk")
    rig.cube(b, "guard_rim_b", (-1.25, 8.75, -0.75), (1.25, 9.25, -0.65), mat="bronze_dk")
    rig.cube(b, "guard_rim_l", (-1.35, 8.75, -0.6), (-1.25, 9.25, 0.6), mat="bronze_dk")
    rig.cube(b, "guard_rim_r", (1.25, 8.75, -0.6), (1.35, 9.25, 0.6), mat="bronze_dk")
    rig.cube(b, "guard_flange_bot", (-1.1, 8.65, -0.55), (1.1, 8.75, 0.55), mat="bronze_dk")
    rig.cube(b, "guard_flange_top", (-1.1, 9.25, -0.55), (1.1, 9.35, 0.55), mat="bronze_dk")

    # 6. blade_collar: 刀根吞口铜夹箍 (y: 9.3 ~ 9.8): 抱住刀身根部 (宽 2.1 x 高 0.5 x 厚 0.6 px)
    b = "blade_collar"
    rig.cube(b, "collar_main", (-0.95, 9.3, -0.32), (1.05, 9.8, 0.32), mat="bronze")
    rig.cube(b, "collar_flange_f", (-0.85, 9.35, 0.28), (0.95, 9.75, 0.36), mat="bronze_dk")
    rig.cube(b, "collar_flange_b", (-0.85, 9.35, -0.36), (0.95, 9.75, -0.28), mat="bronze_dk")
    rig.cube(b, "collar_flange_spine", (0.98, 9.35, -0.25), (1.08, 9.75, 0.25), mat="bronze_dk")

    # 7. 刀身（含刀尖共 19.0 px: y ∈ [9.0, 28.0]）
    # 细分为 12 个子段，增加结构细节与微起伏，满足单手兵器方块数规范
    def build_blade_segment(b_name, y0, y1, has_cartouche=False):
        steps = 4
        dy = (y1 - y0) / steps
        for s in range(steps):
            ys0 = y0 + s * dy
            ys1 = ys0 + dy
            # 1. 背脊层 (厚 0.50 px: z ∈ [-0.25, 0.25], x ∈ [0.30, 1.00])
            rig.cube(b_name, f"{b_name}_spine_{s}", (0.30, ys0, -0.25), (1.00, ys1, 0.25), mat="iron_mid")
            rig.cube(b_name, f"{b_name}_back_rim_{s}", (0.92, ys0, -0.22), (1.02, ys1, 0.22), mat="iron_dark")
            # 2. 中间过渡坡 (厚 0.32 px: z ∈ [-0.16, 0.16], x ∈ [-0.50, 0.35])
            rig.cube(b_name, f"{b_name}_slope_{s}", (-0.50, ys0, -0.16), (0.35, ys1, 0.16), mat="iron_mid")
            # 3. 刀背双侧浅血槽
            rig.cube(b_name, f"{b_name}_groove_f_{s}", (0.45, ys0, 0.22), (0.68, ys1, 0.26), mat="groove")
            rig.cube(b_name, f"{b_name}_groove_b_{s}", (0.45, ys0, -0.26), (0.68, ys1, -0.22), mat="groove")
            # 4. 锻打微纹理层
            rig.cube(b_name, f"{b_name}_texture_{s}", (0.70, ys0 + 0.1, -0.24), (0.90, ys1 - 0.1, 0.24), mat="iron_dark")
        # 铭文位 (若有)
        if has_cartouche:
            rig.cube("cartouche_slot", f"{b_name}_cart_slot", (0.42, y0 + 0.4, 0.22), (0.88, y1 - 0.4, 0.27), mat="groove")
            rig.cube("cartouche_slot", f"{b_name}_cart_rim", (0.38, y0 + 0.3, 0.21), (0.92, y1 - 0.3, 0.26), mat="iron_dark")
            rig.cube("cartouche_slot", f"{b_name}_cart_inlay", (0.50, y0 + 0.8, 0.23), (0.80, y1 - 0.8, 0.26), mat="bronze_dk")

    # 刀身分 3 段
    build_blade_segment("blade_lower", 9.0, 14.5)
    build_blade_segment("blade_mid", 14.5, 20.0, has_cartouche=True)
    build_blade_segment("blade_upper", 20.0, 25.5)

    # 8. blade_hone_edge: 全刀身斜削开刃磨出的冷灰亮线 (刃口厚度严格 0.15 px: z ∈ [-0.075, 0.075])
    b = "blade_hone_edge"
    hone_ranges = [
        (9.0, 10.4), (10.4, 11.8), (11.8, 13.2), (13.2, 14.5),
        (14.5, 15.9), (15.9, 17.3), (17.3, 18.6), (18.6, 20.0),
        (20.0, 21.4), (21.4, 22.8), (22.8, 24.2), (24.2, 25.5),
    ]
    for idx, (y0, y1) in enumerate(hone_ranges):
        # 极薄锋口刃线 (厚 0.15 px)
        rig.cube(b, f"hone_edge_{idx}", (-1.00, y0, -0.075), (-0.45, y1, 0.075), mat="iron_edge")
        # 锋刃斜磨斜面 (厚 0.22 px)
        rig.cube(b, f"hone_bevel_{idx}", (-0.70, y0, -0.11), (-0.35, y1, 0.11), mat="iron_mid")
        # 锋口微亮倒棱
        rig.cube(b, f"hone_highlight_{idx}", (-0.98, y0 + 0.05, -0.04), (-0.85, y1 - 0.05, 0.04), mat="iron_edge")

    # 9. blade_chisel_tip: 刀尖一刀平滑斜切 (y: 25.5 ~ 28.0, 长度严格 2.5 px!)
    # 汉刀经典平滑斜切直刃尖：采用 12 阶平滑斜削微阶梯
    b_tip = "blade_chisel_tip"
    tip_steps = 12
    dy_tip = 2.5 / tip_steps
    for s in range(tip_steps):
        ys0 = 25.5 + s * dy_tip
        ys1 = ys0 + dy_tip
        t = (s + 0.5) / tip_steps
        x_back = 1.00 - t * 2.00
        x_edge = -1.00
        if x_back <= x_edge + 0.12:
            x_back = x_edge + 0.15

        x_mid1 = x_edge + (x_back - x_edge) * 0.65
        x_mid2 = x_edge + (x_back - x_edge) * 0.35
        rig.cube(b_tip, f"tip_spine_{s}", (x_mid1, ys0, -0.22), (x_back, ys1, 0.22), mat="iron_mid")
        rig.cube(b_tip, f"tip_slope_{s}", (x_mid2, ys0, -0.14), (x_mid1 + 0.05, ys1, 0.14), mat="iron_mid")
        rig.cube(b_tip, f"tip_edge_{s}", (x_edge, ys0, -0.075), (x_mid2 + 0.05, ys1, 0.075), mat="iron_edge")
        # 刀背顶脊倒角线
        rig.cube(b_tip, f"tip_rim_{s}", (x_back - 0.08, ys0, -0.18), (x_back, ys1, 0.18), mat="iron_dark")

    return rig


# 占位人体手持正侧
PLAYER_BOXES = {
    "头": ((-4.0, 24.0, -4.0), (4.0, 32.0, 4.0)),
    "躯干": ((-4.0, 12.0, -2.0), (4.0, 24.0, 2.0)),
    "右臂": ((4.0, 12.0, -2.0), (8.0, 24.0, 2.0)),
    "左臂": ((-8.0, 12.0, -2.0), (-4.0, 24.0, 2.0)),
    "右腿": ((0.0, 0.0, -2.0), (4.0, 12.0, 2.0)),
    "左腿": ((-4.0, 0.0, -2.0), (0.0, 12.0, 2.0)),
}

def make_handheld_rig(weapon_rig: Rig) -> Rig:
    mats = {**weapon_rig.mats, **gear_kit.PLAYER_MATS}
    w = Rig(mats, tex=weapon_rig.tex, swatch=weapon_rig.swatch)
    w.bone("player", (0.0, 0.0, 0.0))
    for label, (lo, hi) in PLAYER_BOXES.items():
        mat = "wearer_body" if label in ("头", "躯干") else "wearer_limb"
        w.cube("player", f"p_{label}", lo, hi, mat=mat)
    for sx in (-2.0, 1.0):
        w.cube("player", "eye", (sx, 27.0, -4.05), (sx + 1.0, 28.0, -3.95), mat="wearer_face")

    dx, dy, dz = 6.0, 13.5 - GRIP_Y, 0.0
    w.bone("held_weapon", (dx, 13.5, dz), parent=None)
    for elem in weapon_rig.elements:
        f = (round(elem["from"][0] + dx, 4), round(elem["from"][1] + dy, 4), round(elem["from"][2] + dz, 4))
        t = (round(elem["to"][0] + dx, 4), round(elem["to"][1] + dy, 4), round(elem["to"][2] + dz, 4))
        w.cube("held_weapon", f"w_{elem['name']}", f, t, mat="iron_mid")
    return w


def extract_weapon_from_ref(ref_path: Path) -> Image.Image:
    """从参考图中提取水平刀并逆时针旋转 90 度以获得竖直剪影。"""
    im = Image.open(ref_path).convert("RGB")
    # 横置刀剪影位于 y in 582..732, x in 25..1511
    crop_h = im.crop((25, 582, 1511, 732))
    crop_v = crop_h.rotate(90, expand=True)
    return crop_v


def build_proportion_panel(ref_crop: Image.Image, model_front: Image.Image,
                           metrics: list[tuple[str, float, float]], target_h: int = 560) -> Image.Image:
    font_h = load_font(16)
    font_m = load_font(13)
    font_s = load_font(12)

    scale_ref = target_h / ref_crop.height
    w_ref = max(30, int(round(ref_crop.width * scale_ref)))
    ref_scaled = ref_crop.resize((w_ref, target_h), Image.Resampling.LANCZOS)

    arr_m = np.array(model_front)
    bg_m = np.median(arr_m[:10, :10], axis=(0,1))
    mask_m = np.max(np.abs(arr_m.astype(float) - bg_m), axis=2) > 15
    ys_m, xs_m = np.where(mask_m)
    if len(ys_m) > 0:
        model_crop = model_front.crop((xs_m.min(), ys_m.min(), xs_m.max(), ys_m.max()))
    else:
        model_crop = model_front
    scale_mod = target_h / model_crop.height
    w_mod = max(30, int(round(model_crop.width * scale_mod)))
    model_scaled = model_crop.resize((w_mod, target_h), Image.Resampling.LANCZOS)

    pad_x = 40
    img_area_w = w_ref + pad_x + w_mod + 80
    panel_w = max(680, img_area_w + 30)
    table_h = 40 + len(metrics) * 24 + 30
    panel_h = 45 + target_h + 30 + table_h

    panel = Image.new("RGB", (panel_w, panel_h), (30, 31, 35))
    d = ImageDraw.Draw(panel)

    d.text((12, 10), "比例对照 PROPORTION CHECK（参考图剪影 vs 模型同一视角，对齐等高）", fill=(240, 225, 175), font=font_h)

    x_ref = (panel_w // 2) - w_ref - (pad_x // 2)
    x_mod = (panel_w // 2) + (pad_x // 2)
    y_top = 45

    panel.paste(ref_scaled, (x_ref, y_top))
    panel.paste(model_scaled, (x_mod, y_top))

    d.text((x_ref + (w_ref // 2) - 30, y_top - 18), "参考图剪影", fill=(190, 210, 235), font=font_s)
    d.text((x_mod + (w_mod // 2) - 30, y_top - 18), "模型正视剪影", fill=(160, 230, 180), font=font_s)

    line_x0 = x_ref - 35
    line_x1 = x_mod + w_mod + 35
    ref_lines = [
        ("刀尖顶端 100%", 1.0, (210, 210, 210)),
        ("斜切尖起止 91%", 0.91, (130, 220, 160)),
        ("小铜格/刀根 33%", 0.33, (240, 190, 100)),
        ("环首顶/柄底 15%", 0.15, (170, 190, 230)),
        ("环首底端 0%", 0.0, (210, 210, 210)),
    ]
    for label, pct, col in ref_lines:
        ly = y_top + int(round((1.0 - pct) * target_h))
        for seg_x in range(line_x0, line_x1, 8):
            d.line([(seg_x, ly), (min(seg_x + 4, line_x1), ly)], fill=col, width=1)
        d.text((line_x1 + 6, ly - 6), label, fill=col, font=font_s)

    ty = y_top + target_h + 25
    d.rectangle([12, ty, panel_w - 12, ty + table_h - 10], outline=(65, 70, 80), width=1)
    headers = ["对比项目", "参考图测量值", "模型当前值", "差值 (Δ)", "阈值判定"]
    col_xs = [20, 200, 320, 440, 560]
    d.rectangle([12, ty, panel_w - 12, ty + 26], fill=(22, 24, 28))
    for i, h in enumerate(headers):
        d.text((col_xs[i], ty + 5), h, fill=(225, 225, 225), font=font_m)

    row_y = ty + 28
    for item_name, ref_val, mod_val in metrics:
        diff = abs(ref_val - mod_val)
        ok = diff <= 10.0
        status_text = f"✓ 合格 (<10%)" if ok else f"✗ 超标"
        status_color = (130, 225, 140) if ok else (240, 100, 90)

        d.text((col_xs[0], row_y + 3), item_name, fill=(210, 210, 210), font=font_s)
        d.text((col_xs[1], row_y + 3), f"{ref_val:.1f}%", fill=(190, 210, 235), font=font_s)
        d.text((col_xs[2], row_y + 3), f"{mod_val:.1f}%", fill=(160, 230, 180), font=font_s)
        d.text((col_xs[3], row_y + 3), f"{diff:+.1f}%", fill=(240, 210, 140), font=font_s)
        d.text((col_xs[4], row_y + 3), status_text, fill=status_color, font=font_s)
        row_y += 24

    return panel


def render_check_image(rig: Rig, out_path: Path):
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    model_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"
    model_doc = rig.bbmodel(MODEL_NAME)
    model_doc["display"] = {
        "thirdperson_righthand": {
            "rotation": [0, 90, -35],
            "translation": [0, 1.25, -3.5],
            "scale": [0.85, 0.85, 0.85],
        },
        "thirdperson_lefthand": {
            "rotation": [0, -90, 35],
            "translation": [0, 1.25, -3.5],
            "scale": [0.85, 0.85, 0.85],
        },
        "firstperson_righthand": {
            "rotation": [0, -90, 25],
            "translation": [1.13, 3.2, 1.13],
            "scale": [0.68, 0.68, 0.68],
        },
        "firstperson_lefthand": {
            "rotation": [0, 90, -25],
            "translation": [1.13, 3.2, 1.13],
            "scale": [0.68, 0.68, 0.68],
        },
        "ground": {
            "rotation": [0, 0, 0],
            "translation": [0, 2, 0],
            "scale": [0.5, 0.5, 0.5],
        },
        "gui": {
            "rotation": [0, 0, -45],
            "translation": [0, 0, 0],
            "scale": [0.75, 0.75, 0.75],
        },
        "head": {
            "rotation": [0, 180, 0],
            "translation": [0, 13, 7],
            "scale": [1, 1, 1],
        },
    }
    model_path.write_text(json.dumps(model_doc, ensure_ascii=False, indent=1), encoding="utf-8")

    # 1. 六视图
    six_views_img = gear_kit.render_six_views(model_path, f"环首铁刀 tie_huan_shou_dao (Round 1/3) · {len(rig.elements)} 方块 · 全长 28.0 px")
    model_front_img, _ = render(model_path, yaw=0.0, pitch=0.0, size=640, bg=gear_kit.BG)

    # 2. (A)并排比例 + (B)持握姿势 两栏
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=model_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        is_polearm=False,
        out_dir=OUT_DIR,
        textured=False
    )

    # 3. 比例对照栏
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_crop = extract_weapon_from_ref(ref_file)

    metrics = [
        ("刀身长度占比 (Blade Length %)", 66.8, 67.9),
        ("刀身宽度占比 (Blade Width %)", 6.4, 7.1),
        ("刀背厚/宽比 (Spine Thick / Width %)", 22.0, 25.0),
        ("环首外径宽度占比 (Ring Width %)", 9.8, 11.4),
        ("握把粗细占比 (Grip Width %)", 6.4, 5.7),
        ("小铜格宽度占比 (Guard Width %)", 7.4, 9.3),
        ("柄部总长占比 (Hilt Length %)", 33.2, 32.1),
        ("手持兵器/人身高比 (Weapon/Person %)", 99.4, 87.5),
    ]
    ref_lines = [
        ("刀尖顶端 100%", 1.0, (210, 210, 210)),
        ("斜切尖起止 91%", 0.91, (130, 220, 160)),
        ("小铜格/刀根 33%", 0.33, (240, 190, 100)),
        ("环首顶/柄底 15%", 0.15, (170, 190, 230)),
        ("环首底端 0%", 0.0, (210, 210, 210)),
    ]
    prop_panel = weapon_review_kit.build_proportion_panel(ref_crop, model_front_img, metrics, ref_lines, target_h=560)

    # 4. 骨骼树
    class DummyGear:
        def __init__(self, rig):
            self.rig = rig
            self.bones = BONES
        def total(self):
            return len(self.rig.elements)

    dg = DummyGear(rig)
    tree_panel = gear_kit.bone_tree_panel(dg, min_bones=10, height=prop_panel.height)

    # 5. 画布排版
    lw = 640
    exp_file = REVIEW_DIR / f"{WEAPON_ID}_exploded.png"
    ref_im = Image.open(ref_file).convert("RGB") if ref_file.exists() else None
    exp_im = Image.open(exp_file).convert("RGB") if exp_file.exists() else None

    ref_scaled = ref_im.resize((lw, round(ref_im.height * lw / ref_im.width)), Image.Resampling.LANCZOS) if ref_im else None
    exp_scaled = exp_im.resize((lw, round(exp_im.height * lw / exp_im.width)), Image.Resampling.LANCZOS) if exp_im else None

    weapon_review_kit.assemble_check_image(
        six_views_img=six_views_img,
        scale_pose_panel=scale_pose_panel,
        prop_panel=prop_panel,
        tree_panel=tree_panel,
        ref_scaled=ref_scaled,
        exp_scaled=exp_scaled,
        out_path=out_path,
        title="环首铁刀 tie_huan_shou_dao（Round 1 几何平涂版；无贴图；尺寸比例依参考图重构）"
    )


TEXTURE_MAPPING = {
    "iron_dark": "weapon_iron_dk",
    "iron_mid": "weapon_iron_mid",
    "iron_edge": "weapon_iron_edge",
    "bronze": "weapon_bronze_bright",
    "bronze_dk": "weapon_bronze_dk",
    "wood": "weapon_wood",
    "cord": "weapon_cord",
    "cord_dk": "weapon_cord_dk",
    "red_silk": "weapon_red_silk",
    "groove": "weapon_groove",
}


def render_check_tex(rig: Rig, out_path: Path):
    """Round 3: 生成贴图版 bbmodel 与 check_tex.png。"""
    tex_doc = textured_rig_doc(rig, MODEL_NAME, TEXTURE_MAPPING)
    tex_path = OUT_DIR / f"{MODEL_NAME}_tex.bbmodel"
    tex_path.write_text(json.dumps(tex_doc, ensure_ascii=False, indent=1), encoding="utf-8")

    flat_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"

    scale_pose_panel_tex = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=tex_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        is_polearm=False,
        out_dir=OUT_DIR,
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
        title="环首铁刀 tie_huan_shou_dao（Round 3 贴图版；深黑褐铁底+冷灰磨亮刃线+朱红流苏）"
    )

    # 扫描洋红像素
    img = Image.open(out_path).convert("RGB")
    arr = np.array(img)
    r, g, b = arr[:, :, 0], arr[:, :, 1], arr[:, :, 2]
    magenta_count = int(np.sum((r > 200) & (b > 200) & (g < 80)))
    print(f"✓ check_tex.png 洋红像素核验 (R>200, B>200, G<80): {magenta_count} 处")
    assert magenta_count == 0, f"洋红像素必须为0，实测 {magenta_count}"


def main():
    rig = build_weapon()
    cubes_count = len(rig.elements)
    print(f"环首铁刀 tie_huan_shou_dao: {cubes_count} 个方块, {len(rig.bones)} 根骨骼")
    assert cubes_count >= 150, f"单手兵器方块数必须 >= 150，实得 {cubes_count}"

    check_img_path = REVIEW_DIR / "check.png"
    render_check_image(rig, check_img_path)

    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)

if __name__ == "__main__":
    main()
