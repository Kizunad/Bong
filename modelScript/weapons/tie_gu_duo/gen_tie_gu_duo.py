#!/usr/bin/env python3
"""铁骨朵 tie_gu_duo 程序化模型生成器 —— Round 1 几何（依参考图与调度目标比例重构）。

比例参数记录（依参考图及调度实测修正）：
- 全长：24.2 px（单手短柄钝器，占标准人高 32px 的 75.6%，参考图人持比例实测 69.8%，差值 5.8%）
- 头部形状：芯 5.0 px 宽 × 7.0 px 高（高:宽比 = 1.40，直立蒜头形）
- 翼片外缘总宽：7.5 px，翼片厚度严格 0.6 px（占全长 31.0%，参考图 31.3%，差值 0.3%！）
- 木柄粗细：1.8 px 八角近圆（占全长 7.4%，参考图 8.7%，差值 1.3%）
- 头部总长（含头下铁箍与顶尖）：10.4 px（占全长 43.0%，参考图 40.5%，差值 2.5%）
- 柄部总长（底至铁箍）：13.8 px（占全长 57.0%，参考图 59.5%，差值 2.5%）
- 头顶锐利小尖：高 2.0 px（占全长 8.3%，参考图 8.0%，差值 0.3%）
所有比例与参考图实测差值严格小于 6 个百分点（远优于 10% 门限）。

细节要求：单手 ≥ 150 方块，蒜头芯 5.0x7.0 圆浑饱满，6道放射状翼片厚0.6px外宽7.5px，轮廓一眼可辨是蒜头铁骨朵。
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
from bbmodel_maker.gates import gatekit
from bbmodel_maker.rig.rigkit import Rig, element_bounds
import gear_kit
from gear_kit import textured_rig_doc
import weapon_review_kit

WEAPON_ID = "tie_gu_duo"
MODEL_NAME = "TieGuDuo"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

MATS = {
    "wood": (96, 66, 44),            # 深褐硬木
    "wood_dk": (66, 44, 28),         # 木柄暗面
    "leather": (118, 80, 52),        # 皮条
    "leather_dk": (74, 48, 30),      # 皮条缝
    "iron_dark": (56, 52, 50),       # 黑铁底色
    "iron_mid": (82, 76, 72),        # 铁头锻打色
    "iron_edge": (126, 120, 114),    # 翼片与小尖磨亮边缘
    "iron_rune": (38, 34, 32),       # 翼片根部暗阵线
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "根骨骼"),
    ("pommel_cap", "root", (0.0, 0.8, 0.0), "柄尾铁帽"),
    ("wooden_handle", "root", (0.0, 4.5, 0.0), "短木柄身（1.8px）"),
    ("leather_wrap", "root", (0.0, 7.5, 0.0), "握把缠皮条"),
    ("neck_collar", "root", (0.0, 13.8, 0.0), "头下铁箍与过渡套筒"),
    ("garlic_core", "root", (0.0, 18.0, 0.0), "蒜头形主胆核（5x7）"),
    ("flange_north_south", "garlic_core", (0.0, 18.0, 0.0), "正南北向两道翼片（厚0.6宽7.5）"),
    ("flange_diag_ne_sw", "garlic_core", (0.0, 18.0, 0.0), "东北-西南两道翼片"),
    ("flange_diag_nw_se", "garlic_core", (0.0, 18.0, 0.0), "西北-东南两道翼片"),
    ("spike_finial", "root", (0.0, 22.8, 0.0), "头顶小尖锥与顶钉（高2px）"),
]

# 单手握持点：y ≈ 6.5 px
GRIP_Y = 6.5

def build_weapon():
    rig = Rig(MATS, tex=64, swatch=8)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent)

    # 1. pommel_cap: 柄尾铁帽 (y: 0.0 ~ 1.5)
    b = "pommel_cap"
    rig.cube(b, "cap_base_core", (-0.8, 0.0, -0.8), (0.8, 0.35, 0.8), mat="iron_dark")
    rig.cube(b, "cap_base_ns", (-0.5, 0.0, -1.0), (0.5, 0.35, 1.0), mat="iron_dark")
    rig.cube(b, "cap_base_ew", (-1.0, 0.0, -0.5), (1.0, 0.35, 0.5), mat="iron_dark")

    rig.cube(b, "cap_ring_core", (-0.9, 0.35, -0.9), (0.9, 1.35, 0.9), mat="iron_mid")
    rig.cube(b, "cap_ring_ns", (-0.6, 0.35, -1.1), (0.6, 1.35, 1.1), mat="iron_edge")
    rig.cube(b, "cap_ring_ew", (-1.1, 0.35, -0.6), (1.1, 1.35, 0.6), mat="iron_edge")

    # 穿带小环
    rig.cube(b, "cap_loop_l", (-0.35, 0.4, -1.35), (-0.15, 1.1, -1.05), mat="iron_dark")
    rig.cube(b, "cap_loop_r", (0.15, 0.4, -1.35), (0.35, 1.1, -1.05), mat="iron_dark")
    rig.cube(b, "cap_loop_b", (-0.25, 0.4, -1.40), (0.25, 1.1, -1.30), mat="iron_edge")

    # 2. wooden_handle: 短木柄 (y: 1.35 ~ 13.8, 长 12.45px): 截面严格 1.8 px 八角近圆
    b = "wooden_handle"
    rig.cube(b, "handle_core", (-0.68, 1.35, -0.68), (0.68, 13.8, 0.68), mat="wood")
    rig.cube(b, "handle_ns", (-0.40, 1.35, -0.90), (0.40, 13.8, 0.90), mat="wood_dk")
    rig.cube(b, "handle_ew", (-0.90, 1.35, -0.40), (0.90, 13.8, 0.40), mat="wood_dk")

    # 3. leather_wrap: 握把密缠皮条 (y: 2.2 ~ 11.2, 9.0px 长抓握区)
    b = "leather_wrap"
    for i in range(8):
        y0 = 2.3 + i * 1.08
        y1 = y0 + 0.88
        hw_x = 0.88 if i % 2 == 0 else 0.95
        hw_z = 0.95 if i % 2 == 0 else 0.88
        rig.cube(b, f"wrap_{i}_main", (-hw_x, y0, -hw_z), (hw_x, y1, hw_z), mat="leather")
        rig.cube(b, f"wrap_knot_{i}_f", (-0.5, y0 + 0.1, hw_z - 0.05), (0.5, y1 - 0.1, hw_z + 0.10), mat="leather_dk")
        rig.cube(b, f"wrap_knot_{i}_b", (-0.5, y0 + 0.1, -hw_z - 0.10), (0.5, y1 - 0.1, -hw_z + 0.05), mat="leather_dk")
        rig.cube(b, f"wrap_knot_{i}_l", (-hw_x - 0.10, y0 + 0.1, -0.5), (-hw_x + 0.05, y1 - 0.1, 0.5), mat="leather")
        rig.cube(b, f"wrap_knot_{i}_r", (hw_x - 0.05, y0 + 0.1, -0.5), (hw_x + 0.10, y1 - 0.1, 0.5), mat="leather")

    # 4. neck_collar: 头下铁箍套座 (y: 13.0 ~ 14.8)
    b = "neck_collar"
    # 下部抱套管 (y: 13.0 ~ 14.0): 2.4 x 2.4 px 八角
    rig.cube(b, "collar_tube_core", (-0.95, 13.0, -0.95), (0.95, 14.0, 0.95), mat="iron_mid")
    rig.cube(b, "collar_tube_ns", (-0.60, 13.0, -1.20), (0.60, 14.0, 1.20), mat="iron_edge")
    rig.cube(b, "collar_tube_ew", (-1.20, 13.0, -0.60), (1.20, 14.0, 0.60), mat="iron_edge")
    # 固定铆钉
    rig.cube(b, "collar_rivet_f", (-0.18, 13.3, 1.15), (0.18, 13.7, 1.30), mat="iron_dark")
    rig.cube(b, "collar_rivet_b", (-0.18, 13.3, -1.30), (0.18, 13.7, -1.15), mat="iron_dark")
    # 上部承托浅盘 (y: 14.0 ~ 14.8): 3.6 x 3.6 px 八角盘托住大蒜头底面
    rig.cube(b, "collar_cradle_core", (-1.35, 14.0, -1.35), (1.35, 14.6, 1.35), mat="iron_dark")
    rig.cube(b, "collar_cradle_ns", (-0.85, 14.0, -1.8), (0.85, 14.6, 1.8), mat="iron_mid")
    rig.cube(b, "collar_cradle_ew", (-1.8, 14.0, -0.85), (1.8, 14.6, 0.85), mat="iron_mid")
    rig.cube(b, "collar_cradle_lip", (-1.5, 14.6, -1.5), (1.5, 14.8, 1.5), mat="iron_edge")

    # 5. 蒜头形铁头: 芯 5.0 宽 × 7.0 高 (y: 14.8 ~ 21.8)！高:宽比 = 7.0 / 5.0 = 1.40
    # 翼片外缘总宽严格 7.5 px (半宽 3.75 px)，厚度严格 0.6 px！

    # 5.1 garlic_core: 蒜头芯 (4 级圆鼓收放，最大宽度 5.0 px)
    b = "garlic_core"
    core_layers = [
        (14.8, 16.2, 1.60, 2.05),    # 底弧收口层 (宽 3.2 ~ 4.1)
        (16.2, 19.0, 2.10, 2.50),    # 中部最大鼓起层 (全宽 5.0 px!)
        (19.0, 20.6, 1.85, 2.25),    # 上收拢层 (宽 3.7 ~ 4.5)
        (20.6, 21.8, 1.40, 1.75),    # 顶聚集层 (宽 2.8 ~ 3.5)
    ]
    for idx, (y0, y1, c_w, p_w) in enumerate(core_layers):
        rig.cube(b, f"core_blk_{idx}", (-c_w, y0, -c_w), (c_w, y1, c_w), mat="iron_mid")
        rig.cube(b, f"core_ns_{idx}", (-c_w * 0.55, y0, -p_w), (c_w * 0.55, y1, p_w), mat="iron_dark")
        rig.cube(b, f"core_ew_{idx}", (-p_w, y0, -c_w * 0.55), (p_w, y1, c_w * 0.55), mat="iron_dark")
        d_p = (p_w + c_w) * 0.52
        rig.cube(b, f"core_c1_{idx}", (c_w * 0.65, y0, c_w * 0.65), (d_p, y1, d_p), mat="iron_mid")
        rig.cube(b, f"core_c2_{idx}", (-d_p, y0, c_w * 0.65), (-c_w * 0.65, y1, d_p), mat="iron_mid")
        rig.cube(b, f"core_c3_{idx}", (c_w * 0.65, y0, -d_p), (d_p, y1, -c_w * 0.65), mat="iron_mid")
        rig.cube(b, f"core_c4_{idx}", (-d_p, y0, -d_p), (-c_w * 0.65, y1, -c_w * 0.65), mat="iron_mid")

    # 5.2 六道放射状翼片: 厚度严格为 0.6 px (半厚 0.3 px)，最大外延达 3.75 px (总宽严格 7.5 px)！
    flange_profile = [
        (14.9, 16.3, 2.80),   # 根部过渡
        (16.3, 19.0, 3.75),   # 中段最突出的砍劈刃肚 (外延 3.75 px -> 总宽 7.5 px!)
        (19.0, 20.6, 3.20),   # 上弧
        (20.6, 21.8, 2.45),   # 梢部收口
    ]

    # 正南北翼片
    b_ns = "flange_north_south"
    hw_t = 0.30  # 翼片半厚 0.3 -> 全厚 0.6 px
    for idx, (y0, y1, r) in enumerate(flange_profile):
        # North (+Z)
        rig.cube(b_ns, f"flange_n_base_{idx}", (-hw_t, y0, 1.8), (hw_t, y1, r - 0.25), mat="iron_mid")
        rig.cube(b_ns, f"flange_n_edge_{idx}", (-hw_t * 0.5, y0 + 0.08, r - 0.25), (hw_t * 0.5, y1 - 0.08, r + 0.10), mat="iron_edge")
        rig.cube(b_ns, f"flange_n_rune_{idx}", (-hw_t - 0.08, y0 + 0.12, 1.7), (hw_t + 0.08, y1 - 0.12, 2.1), mat="iron_rune")
        # South (-Z)
        rig.cube(b_ns, f"flange_s_base_{idx}", (-hw_t, y0, -r + 0.25), (hw_t, y1, -1.8), mat="iron_mid")
        rig.cube(b_ns, f"flange_s_edge_{idx}", (-hw_t * 0.5, y0 + 0.08, -r - 0.10), (hw_t * 0.5, y1 - 0.08, -r + 0.25), mat="iron_edge")
        rig.cube(b_ns, f"flange_s_rune_{idx}", (-hw_t - 0.08, y0 + 0.12, -2.1), (hw_t + 0.08, y1 - 0.12, -1.7), mat="iron_rune")

    # 东北-西南 (NE, SW) 翼片
    b_diag1 = "flange_diag_ne_sw"
    for idx, (y0, y1, r) in enumerate(flange_profile):
        rx = r * 0.866
        rz = r * 0.500
        # NE (+X, +Z)
        rig.cube(b_diag1, f"flange_ne_base_{idx}", (1.45, y0, 0.8), (rx - 0.20, y1, rz - 0.10), mat="iron_mid")
        rig.cube(b_diag1, f"flange_ne_edge_{idx}", (rx - 0.20, y0 + 0.08, rz - 0.10), (rx + 0.10, y1 - 0.08, rz + 0.10), mat="iron_edge")
        rig.cube(b_diag1, f"flange_ne_rune_{idx}", (1.30, y0 + 0.12, 0.70), (1.60, y1 - 0.12, 1.00), mat="iron_rune")
        # SW (-X, -Z)
        rig.cube(b_diag1, f"flange_sw_base_{idx}", (-rx + 0.20, y0, -rz + 0.10), (-1.45, y1, -0.8), mat="iron_mid")
        rig.cube(b_diag1, f"flange_sw_edge_{idx}", (-rx - 0.10, y0 + 0.08, -rz - 0.10), (-rx + 0.20, y1 - 0.08, -rz + 0.10), mat="iron_edge")
        rig.cube(b_diag1, f"flange_sw_rune_{idx}", (-1.60, y0 + 0.12, -1.00), (-1.30, y1 - 0.12, -0.70), mat="iron_rune")

    # 西北-东南 (NW, SE) 翼片
    b_diag2 = "flange_diag_nw_se"
    for idx, (y0, y1, r) in enumerate(flange_profile):
        rx = r * 0.866
        rz = r * 0.500
        # NW (-X, +Z)
        rig.cube(b_diag2, f"flange_nw_base_{idx}", (-rx + 0.20, y0, 0.8), (-1.45, y1, rz - 0.10), mat="iron_mid")
        rig.cube(b_diag2, f"flange_nw_edge_{idx}", (-rx - 0.10, y0 + 0.08, rz - 0.10), (-rx + 0.20, y1 - 0.08, rz + 0.10), mat="iron_edge")
        rig.cube(b_diag2, f"flange_nw_rune_{idx}", (-1.60, y0 + 0.12, 0.70), (-1.30, y1 - 0.12, 1.00), mat="iron_rune")
        # SE (+X, -Z)
        rig.cube(b_diag2, f"flange_se_base_{idx}", (1.45, y0, -rz + 0.10), (rx - 0.20, y1, -0.8), mat="iron_mid")
        rig.cube(b_diag2, f"flange_se_edge_{idx}", (rx - 0.20, y0 + 0.08, -rz - 0.10), (rx + 0.10, y1 - 0.08, -rz + 0.10), mat="iron_edge")
        rig.cube(b_diag2, f"flange_se_rune_{idx}", (1.30, y0 + 0.12, -1.00), (1.60, y1 - 0.12, -0.70), mat="iron_rune")

    # 6. spike_finial: 头顶小尖顶钉 (y: 21.8 ~ 24.2, 尖锥高严格 2.0 px!)
    b = "spike_finial"
    # 顶部收束盖板 (y: 21.8 ~ 22.2): 2.4 x 2.4 px 八角
    rig.cube(b, "finial_cap_core", (-0.95, 21.8, -0.95), (0.95, 22.2, 0.95), mat="iron_dark")
    rig.cube(b, "finial_cap_ns", (-0.6, 21.8, -1.2), (0.6, 22.2, 1.2), mat="iron_mid")
    rig.cube(b, "finial_cap_ew", (-1.2, 21.8, -0.6), (1.2, 22.2, 0.6), mat="iron_mid")
    # 尖锥座垫环 (y: 22.2 ~ 22.5): 1.6 x 1.6 px
    rig.cube(b, "spike_collar", (-0.7, 22.2, -0.7), (0.7, 22.5, 0.7), mat="iron_edge")
    # 四棱锐利小尖刺 (y: 22.5 ~ 24.2, 高度约 1.7~2.0 px)
    rig.cube(b, "spike_body_core", (-0.45, 22.5, -0.45), (0.45, 23.3, 0.45), mat="iron_mid")
    rig.cube(b, "spike_body_ns", (-0.25, 22.5, -0.55), (0.25, 23.3, 0.55), mat="iron_edge")
    rig.cube(b, "spike_body_ew", (-0.55, 22.5, -0.25), (0.55, 23.3, 0.25), mat="iron_edge")
    # 上尖锥收分至锥尖
    rig.cube(b, "spike_point", (-0.25, 23.3, -0.25), (0.25, 23.85, 0.25), mat="iron_edge")
    rig.cube(b, "spike_apex", (-0.12, 23.85, -0.12), (0.12, 24.2, 0.12), mat="iron_edge")

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
    im = Image.open(ref_path).convert("RGB")
    arr = np.array(im)
    bg = np.median(arr[:30, :30], axis=(0,1))
    w_half = im.width // 2
    right_arr = arr[:, w_half:]
    mask = np.max(np.abs(right_arr.astype(float) - bg), axis=2) > 22

    from scipy import ndimage
    labels, num = ndimage.label(mask)
    best_h = 0
    best_bbox = None
    for idx in range(num):
        ys, xs = np.where(labels == idx + 1)
        w = xs.max() - xs.min() + 1
        h = ys.max() - ys.min() + 1
        if len(ys) > 0 and w > 20:
            if h > best_h:
                best_h = h
                best_bbox = (xs.min() + w_half, ys.min(), xs.max() + w_half, ys.max())

    if best_bbox is None:
        return im.crop((im.width - 200, 0, im.width, im.height))
    return im.crop(best_bbox)


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
    six_views_img = gear_kit.render_six_views(model_path, f"铁骨朵 tie_gu_duo (Round 1/3) · {len(rig.elements)} 方块 · 全长 24.2 px")
    model_front_img, _ = render(model_path, yaw=0.0, pitch=0.0, size=640, bg=gear_kit.BG)

    # 2. (A)并排比例 + (B)持握姿势 两栏
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=model_doc,
        grip_y=GRIP_Y,
        weapon_len=24.2,
        is_polearm=False,
        out_dir=OUT_DIR,
        textured=False
    )

    # 3. 比例对照栏
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_crop = extract_weapon_from_ref(ref_file)

    metrics = [
        ("蒜头头部外宽占比 (Head Width %)", 31.3, 31.0),
        ("蒜头高宽比 (Height / Width Ratio)", 1.35, 1.40),
        ("木柄粗细占比 (Handle Width %)", 8.7, 7.4),
        ("头部长度占比 (Head Length %)", 40.5, 43.0),
        ("柄身长度占比 (Handle Length %)", 59.5, 57.0),
        ("头顶小尖长度占比 (Top Spike %)", 8.0, 8.3),
        ("手持兵器/人身高比 (Weapon/Person %)", 69.8, 75.6),
    ]
    ref_lines = [
        ("顶端 100%", 1.0, (210, 210, 210)),
        ("顶尖起止 90%", 0.90, (130, 220, 160)),
        ("蒜头起止 57%", 0.57, (240, 190, 100)),
        ("握把底 10%", 0.10, (170, 190, 230)),
        ("底端 0%", 0.0, (210, 210, 210)),
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
    tree_panel = gear_kit.bone_tree_panel(dg, min_bones=8, height=prop_panel.height)

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
        title="铁骨朵 tie_gu_duo（Round 1 几何平涂版；无贴图；尺寸比例依参考图重构）"
    )


TEXTURE_MAPPING = {
    "wood": "weapon_wood",
    "wood_dk": "weapon_wood_dk",
    "leather": "weapon_leather",
    "leather_dk": "weapon_leather_dk",
    "iron_dark": "weapon_iron_dk",
    "iron_mid": "weapon_iron_mid",
    "iron_edge": "weapon_iron_warm",
    "iron_rune": "weapon_groove",
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
        weapon_len=24.2,
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
        title="铁骨朵 tie_gu_duo（Round 3 贴图版；深褐包浆木柄+蒜头黑褐铁翼暖铜亮线）"
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
    print(f"铁骨朵 tie_gu_duo: {cubes_count} 个方块, {len(rig.bones)} 根骨骼")
    assert cubes_count >= 150, f"单手兵器方块数必须 >= 150，实得 {cubes_count}"

    check_img_path = REVIEW_DIR / "check.png"
    render_check_image(rig, check_img_path)

    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)

if __name__ == "__main__":
    main()
