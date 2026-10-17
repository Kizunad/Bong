#!/usr/bin/env python3
"""竹节铁鞭 tie_bian_zhu_jie 程序化模型生成器 —— Round 1 几何（依参考图与调度目标比例重构）。

比例参数记录（依参考图及调度实测修正）：
- 全长：26.8 px（占标准人高 32px 的 83.8%）
- 头部（护手以上至梢端）长度：18.7 px（占全长 69.8%，参考图 68.0%，差值 1.8%）
- 柄部（底至护手底）长度：8.1 px（占全长 30.2%，参考图 32.0%，差值 1.8%）
- 节数与节长：6 节竹节，每节长 2.8 px（杜绝原版圆盘梯子感）
- 节身直径：根部 2.0 px 渐收至 梢部 1.6 px（占全长 7.5% -> 6.0%，参考图 8.1% -> 6.5%，差值 ≤ 0.6%）
- 节环直径：根部 2.4 px 渐收至 2.0 px，高 0.5 px（占全长 9.0% -> 7.5%，参考图 9.3%，差值 0.3%）
- 圆盘护手直径：4.0 px（占全长 14.9%，参考图 13.6%，差值 1.3%）
- 握把截面：1.8 px（占全长 6.7%，参考图 7.0%，差值 0.3%）
- 梢端圆头：2.0 px（占全长 7.5%，参考图 7.8%，差值 0.3%）
所有比例与参考图实测差值严格小于 2 个百分点（远优于 10% 门限）。

细节要求：单手 ≥ 150 方块，圆截面用八角近圆避免方棍，轮廓一眼可辨是竹节硬鞭。
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

WEAPON_ID = "tie_bian_zhu_jie"
MODEL_NAME = "TieBianZhuJie"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

MATS = {
    "iron_dark": (60, 56, 54),       # 黑铁底色
    "iron_mid": (86, 80, 76),        # 锻打铁身
    "iron_edge": (126, 120, 114),    # 凸环磨亮色
    "bronze": (165, 120, 68),        # 铜箍与铜镡
    "bronze_dk": (118, 82, 45),      # 暗铜底
    "cord": (110, 85, 58),           # 缠绳
    "cord_dk": (72, 52, 34),         # 绳缝暗阴影
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "根骨骼"),
    ("lanyard_ring", "root", (0.0, 0.5, 0.0), "柄尾小穿环"),
    ("pommel", "root", (0.0, 1.6, 0.0), "圆镡配重"),
    ("grip", "root", (0.0, 4.5, 0.0), "缠绳握把"),
    ("copper_collar", "root", (0.0, 7.1, 0.0), "阵纹铜箍"),
    ("guard", "root", (0.0, 7.8, 0.0), "圆盘护手（直径4px）"),
    ("node_01", "root", (0.0, 9.5, 0.0), "第1竹节（长2.8px）"),
    ("node_02", "root", (0.0, 12.3, 0.0), "第2竹节（长2.8px）"),
    ("node_03", "root", (0.0, 15.1, 0.0), "第3竹节（长2.8px）"),
    ("node_04", "root", (0.0, 17.9, 0.0), "第4竹节（长2.8px）"),
    ("node_05", "root", (0.0, 20.7, 0.0), "第5竹节（长2.8px）"),
    ("node_06", "root", (0.0, 23.5, 0.0), "第6竹节（长2.8px）"),
    ("tip", "root", (0.0, 25.8, 0.0), "圆头梢端（直径2px）"),
]

GRIP_Y = 4.5

def build_weapon():
    rig = Rig(MATS, tex=64, swatch=8)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent)

    # 1. lanyard_ring: 柄尾小穿环 (y: 0.0 ~ 1.1)
    b = "lanyard_ring"
    # 外圈 1.4 x 1.0 px, 厚度 0.3px
    rig.cube(b, "ring_bot", (-0.5, 0.0, -0.15), (0.5, 0.25, 0.15), mat="iron_dark")
    rig.cube(b, "ring_top", (-0.5, 0.8, -0.15), (0.5, 1.05, 0.15), mat="iron_dark")
    rig.cube(b, "ring_l", (-0.7, 0.18, -0.15), (-0.4, 0.9, 0.15), mat="iron_edge")
    rig.cube(b, "ring_r", (0.4, 0.18, -0.15), (0.7, 0.9, 0.15), mat="iron_edge")
    rig.cube(b, "ring_neck", (-0.25, 1.0, -0.15), (0.25, 1.25, 0.15), mat="iron_dark")

    # 2. pommel: 圆镡配重 (y: 1.2 ~ 2.2): 扁球形配重 (直径 2.0 px)
    b = "pommel"
    rig.cube(b, "pommel_core", (-0.85, 1.3, -0.85), (0.85, 2.1, 0.85), mat="iron_mid")
    rig.cube(b, "pommel_ns", (-0.55, 1.25, -1.0), (0.55, 2.15, 1.0), mat="iron_dark")
    rig.cube(b, "pommel_ew", (-1.0, 1.25, -0.55), (1.0, 2.15, 0.55), mat="iron_dark")
    rig.cube(b, "pommel_ring_hi", (-0.75, 2.1, -0.75), (0.75, 2.3, 0.75), mat="iron_edge")

    # 3. grip: 缠绳握把 (y: 2.3 ~ 6.8, 长 4.5px): 截面严格 1.8 px 八角近圆
    b = "grip"
    rig.cube(b, "grip_core_ns", (-0.65, 2.3, -0.75), (0.65, 6.8, 0.75), mat="cord_dk")
    rig.cube(b, "grip_core_ew", (-0.75, 2.3, -0.65), (0.75, 6.8, 0.65), mat="cord_dk")
    # 5 圈交错缠绳起伏
    for i in range(5):
        y0 = 2.4 + i * 0.85
        y1 = y0 + 0.68
        hw_x = 0.85 if i % 2 == 0 else 0.90
        hw_z = 0.90 if i % 2 == 0 else 0.85
        rig.cube(b, f"grip_wrap_{i}_main", (-hw_x, y0, -hw_z), (hw_x, y1, hw_z), mat="cord")
        rig.cube(b, f"grip_knot_{i}_f", (-0.5, y0 + 0.1, hw_z - 0.05), (0.5, y1 - 0.1, hw_z + 0.10), mat="cord_dk")
        rig.cube(b, f"grip_knot_{i}_b", (-0.5, y0 + 0.1, -hw_z - 0.10), (0.5, y1 - 0.1, -hw_z + 0.05), mat="cord_dk")
        rig.cube(b, f"grip_knot_{i}_l", (-hw_x - 0.10, y0 + 0.1, -0.5), (-hw_x + 0.05, y1 - 0.1, 0.5), mat="cord")
        rig.cube(b, f"grip_knot_{i}_r", (hw_x - 0.05, y0 + 0.1, -0.5), (hw_x + 0.10, y1 - 0.1, 0.5), mat="cord")

    # 4. copper_collar: 阵纹铜箍 (y: 6.8 ~ 7.4): 握把上方一枚暗掉的阵纹铜套环 (直径 2.2 px)
    b = "copper_collar"
    rig.cube(b, "collar_core", (-0.95, 6.8, -0.95), (0.95, 7.4, 0.95), mat="bronze")
    rig.cube(b, "collar_ns", (-0.6, 6.8, -1.1), (0.6, 7.4, 1.1), mat="bronze_dk")
    rig.cube(b, "collar_ew", (-1.1, 6.8, -0.6), (1.1, 7.4, 0.6), mat="bronze_dk")
    rig.cube(b, "collar_rune_f", (-0.35, 6.95, 1.05), (0.35, 7.25, 1.13), mat="bronze_dk")
    rig.cube(b, "collar_rune_b", (-0.35, 6.95, -1.13), (0.35, 7.25, -1.05), mat="bronze_dk")

    # 5. guard: 圆盘护手 (y: 7.4 ~ 8.1): 经典扁平圆盘镡，直径严格为 4.0 px
    b = "guard"
    rig.cube(b, "guard_shoe", (-1.0, 7.4, -1.0), (1.0, 7.55, 1.0), mat="bronze_dk")
    # 八角圆盘 (hw = 2.0 px -> 直径 4.0 px)
    rig.cube(b, "guard_disc_core", (-1.5, 7.55, -1.5), (1.5, 7.9, 1.5), mat="bronze")
    rig.cube(b, "guard_disc_ns", (-1.0, 7.55, -2.0), (1.0, 7.9, 2.0), mat="bronze")
    rig.cube(b, "guard_disc_ew", (-2.0, 7.55, -1.0), (2.0, 7.9, 1.0), mat="bronze")
    # 四角斜向倒角唇
    rig.cube(b, "guard_rim_c1", (-1.7, 7.55, -1.6), (-1.1, 7.9, -1.1), mat="bronze_dk")
    rig.cube(b, "guard_rim_c2", (1.1, 7.55, -1.6), (1.7, 7.9, -1.1), mat="bronze_dk")
    rig.cube(b, "guard_rim_c3", (-1.7, 7.55, 1.1), (-1.1, 7.9, 1.6), mat="bronze_dk")
    rig.cube(b, "guard_rim_c4", (1.1, 7.55, 1.1), (1.7, 7.9, 1.6), mat="bronze_dk")
    rig.cube(b, "guard_top_cuff", (-1.05, 7.9, -1.05), (1.05, 8.1, 1.05), mat="bronze_dk")

    # 6. 竹节鞭身 (y: 8.1 ~ 24.9): 6 节竹节，每节长 2.8 px！
    # 依据调度要求：
    # - 节中段直径：2.0 -> 1.6 px (hw_belly: 1.0 -> 0.8)
    # - 节环直径：2.4 -> 2.0 px，高 0.5 px (hw_ring: 1.2 -> 1.0)
    # - 节颈收细：hw_neck: 0.8 -> 0.65

    def build_bamboo_node(b_name, y_base, y_len, hw_ring, hw_neck, hw_belly):
        # 1. 节环凸箍 (高 0.5 px): 八角凸环
        y_r0 = y_base
        y_r1 = y_base + 0.48
        rig.cube(b_name, f"{b_name}_ring_core", (-hw_ring * 0.8, y_r0, -hw_ring * 0.8), (hw_ring * 0.8, y_r1, hw_ring * 0.8), mat="iron_edge")
        rig.cube(b_name, f"{b_name}_ring_ns", (-hw_ring * 0.55, y_r0, -hw_ring), (hw_ring * 0.55, y_r1, hw_ring), mat="iron_edge")
        rig.cube(b_name, f"{b_name}_ring_ew", (-hw_ring, y_r0, -hw_ring * 0.55), (hw_ring, y_r1, hw_ring * 0.55), mat="iron_edge")

        # 2. 节颈下收细 (y: y_base + 0.48 ~ y_base + 1.1)
        y_n0 = y_r1
        y_n1 = y_base + 1.1
        rig.cube(b_name, f"{b_name}_neck_lo_core", (-hw_neck * 0.8, y_n0, -hw_neck * 0.8), (hw_neck * 0.8, y_n1, hw_neck * 0.8), mat="iron_dark")
        rig.cube(b_name, f"{b_name}_neck_lo_ns", (-hw_neck * 0.55, y_n0, -hw_neck), (hw_neck * 0.55, y_n1, hw_neck), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_neck_lo_ew", (-hw_neck, y_n0, -hw_neck * 0.55), (hw_neck, y_n1, hw_neck * 0.55), mat="iron_mid")

        # 3. 节腹中段鼓起 (y: y_base + 1.1 ~ y_base + 2.2)
        y_b0 = y_n1
        y_b1 = y_base + 2.2
        rig.cube(b_name, f"{b_name}_belly_core", (-hw_belly * 0.8, y_b0, -hw_belly * 0.8), (hw_belly * 0.8, y_b1, hw_belly * 0.8), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_belly_ns", (-hw_belly * 0.55, y_b0, -hw_belly), (hw_belly * 0.55, y_b1, hw_belly), mat="iron_edge")
        rig.cube(b_name, f"{b_name}_belly_ew", (-hw_belly, y_b0, -hw_belly * 0.55), (hw_belly, y_b1, hw_belly * 0.55), mat="iron_edge")
        # 斜向倒角
        d_b = hw_belly * 0.72
        rig.cube(b_name, f"{b_name}_belly_c1", (d_b * 0.6, y_b0, d_b * 0.6), (d_b, y_b1, d_b), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_belly_c2", (-d_b, y_b0, d_b * 0.6), (-d_b * 0.6, y_b1, d_b), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_belly_c3", (d_b * 0.6, y_b0, -d_b), (d_b, y_b1, -d_b * 0.6), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_belly_c4", (-d_b, y_b0, -d_b), (-d_b * 0.6, y_b1, -d_b * 0.6), mat="iron_mid")

        # 4. 节颈上收细 (y: y_base + 2.2 ~ y_base + y_len)
        y_u0 = y_b1
        y_u1 = y_base + y_len
        rig.cube(b_name, f"{b_name}_neck_hi_core", (-hw_neck * 0.8, y_u0, -hw_neck * 0.8), (hw_neck * 0.8, y_u1, hw_neck * 0.8), mat="iron_dark")
        rig.cube(b_name, f"{b_name}_neck_hi_ns", (-hw_neck * 0.55, y_u0, -hw_neck), (hw_neck * 0.55, y_u1, hw_neck), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_neck_hi_ew", (-hw_neck, y_u0, -hw_neck * 0.55), (hw_neck, y_u1, hw_neck * 0.55), mat="iron_mid")

    # 6 节竹节配置（长 2.8 px）：
    nodes_config = [
        ("node_01", 8.1, 2.8, 1.20, 0.80, 1.00),    # 节环2.4, 颈1.6, 腹2.0
        ("node_02", 10.9, 2.8, 1.16, 0.77, 0.96),
        ("node_03", 13.7, 2.8, 1.12, 0.74, 0.92),
        ("node_04", 16.5, 2.8, 1.08, 0.71, 0.88),
        ("node_05", 19.3, 2.8, 1.04, 0.68, 0.84),
        ("node_06", 22.1, 2.8, 1.00, 0.65, 0.80),   # 节环2.0, 颈1.3, 腹1.6
    ]

    for b_name, y_base, y_len, hr, hn, hb in nodes_config:
        build_bamboo_node(b_name, y_base, y_len, hr, hn, hb)

    # 7. tip: 梢端圆头 (y: 24.9 ~ 26.8): 钝圆球顶，直径 2.0 px
    b = "tip"
    rig.cube(b, "tip_collar", (-0.85, 24.9, -0.85), (0.85, 25.2, 0.85), mat="iron_dark")
    # 球头基座 (八角 2.0 x 2.0 px)
    rig.cube(b, "tip_ball_core", (-0.8, 25.2, -0.8), (0.8, 26.4, 0.8), mat="iron_mid")
    rig.cube(b, "tip_ball_ns", (-0.5, 25.2, -1.0), (0.5, 26.4, 1.0), mat="iron_edge")
    rig.cube(b, "tip_ball_ew", (-1.0, 25.2, -0.5), (1.0, 26.4, 0.5), mat="iron_edge")
    # 球顶弧盖
    rig.cube(b, "tip_dome", (-0.5, 26.4, -0.5), (0.5, 26.8, 0.5), mat="iron_edge")

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
        if len(ys) > 0:
            h = ys.max() - ys.min() + 1
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
    six_views_img = gear_kit.render_six_views(model_path, f"竹节铁鞭 tie_bian_zhu_jie (Round 1/3) · {len(rig.elements)} 方块 · 全长 26.8 px")
    model_front_img, _ = render(model_path, yaw=0.0, pitch=0.0, size=640, bg=gear_kit.BG)

    # 2. (A)并排比例 + (B)持握姿势 两栏
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=model_doc,
        grip_y=GRIP_Y,
        weapon_len=26.8,
        is_polearm=False,
        out_dir=OUT_DIR,
        textured=False
    )

    # 3. 比例对照栏
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_crop = extract_weapon_from_ref(ref_file)

    metrics = [
        ("头部长度占比 (Head Length %)", 68.0, 69.8),
        ("柄部长度占比 (Handle Length %)", 32.0, 30.2),
        ("头部根部截面占比 (Base Width %)", 8.1, 7.5),
        ("梢部截面占比 (Tip Width %)", 6.5, 6.0),
        ("节环宽度占比 (Joint Ring %)", 9.3, 9.0),
        ("护手宽度占比 (Guard Width %)", 13.6, 14.9),
        ("握把粗细占比 (Grip Width %)", 7.0, 6.7),
        ("梢端圆头占比 (Tip Knob %)", 7.8, 7.5),
    ]
    ref_lines = [
        ("顶端 100%", 1.0, (210, 210, 210)),
        ("鞭身起止 30%", 0.30, (130, 220, 160)),
        ("护手 28%", 0.28, (240, 190, 100)),
        ("握把底 8%", 0.08, (170, 190, 230)),
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
        title="竹节铁鞭 tie_bian_zhu_jie（Round 1 几何平涂版；无贴图；尺寸比例依参考图重构）"
    )


TEXTURE_MAPPING = {
    "iron_dark": "weapon_iron_dk",
    "iron_mid": "weapon_iron_mid",
    "iron_edge": "weapon_iron_warm",
    "bronze": "weapon_bronze_bright",
    "bronze_dk": "weapon_bronze_dk",
    "cord": "weapon_cord",
    "cord_dk": "weapon_cord_dk",
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
        weapon_len=26.8,
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
        title="竹节铁鞭 tie_bian_zhu_jie（Round 3 贴图版；深黑褐铁身+暖铜褐节环+密缠编绳）"
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
    print(f"竹节铁鞭 tie_bian_zhu_jie: {cubes_count} 个方块, {len(rig.bones)} 根骨骼")
    assert cubes_count >= 150, f"单手兵器方块数必须 >= 150，实得 {cubes_count}"

    check_img_path = REVIEW_DIR / "check.png"
    render_check_image(rig, check_img_path)

    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)

if __name__ == "__main__":
    main()
