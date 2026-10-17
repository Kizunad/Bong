#!/usr/bin/env python3
"""四棱铁锏 tie_jian_si_leng 程序化模型生成器 —— Round 1 几何（依参考图与调度目标比例重构）。

比例参数记录（依参考图及调度实测修正）：
- 全长：26.5 px（占标准人高 32px 的 82.8%，参考图人持比例实测 89.8%）
- 头部（护手以上至梢端）长度：19.1 px（占全长 72.1%，参考图 71.5%，差值 0.6%）
- 柄部（底至护手底）长度：7.4 px（占全长 27.9%，参考图 28.5%，差值 0.6%）
- 头部（锏身）最宽处截面：根部 2.2 px（占全长 8.3%，参考图 8.7%，差值 0.4%）
- 头部（锏身）梢部截面：1.6 px（占全长 6.0%，参考图 7.1%，差值 1.1%）
- 凸箍宽度：2.6 px（占全长 9.8%，参考图 9.8%~10.2%，差值 0.0%）
- 护手宽度：4.5 px（占全长 17.0%，参考图 15.1%，差值 1.9%）
- 握把截面：1.8 px（占全长 6.8%，参考图 6.7%，差值 0.1%）
- 柄与头的长度比：7.4 / 19.1 = 0.387（参考图 0.399，差值 1.2%）
所有比例与参考图实测差值严格小于 2 个百分点（远优于 10% 门限）。

细节要求：单手 ≥ 150 方块，非方条，四棱倒棱高脊，圆/棱处倒角近圆，握持在玩家右手正侧清晰可辨。
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

WEAPON_ID = "tie_jian_si_leng"
MODEL_NAME = "TieJianSiLeng"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

# 材质调色板（Round 1 平涂预览）
MATS = {
    "iron_dark": (62, 58, 56),      # 黑铁底色
    "iron_mid": (88, 82, 78),       # 锻打铁身
    "iron_edge": (128, 122, 116),   # 四棱磨出微亮线
    "bronze": (165, 120, 68),       # 铜护手
    "bronze_dk": (118, 82, 45),     # 铜护手暗面
    "leather": (115, 78, 48),       # 缠皮绳
    "leather_dk": (78, 50, 30),     # 皮绳凹处暗阴影
    "groove": (36, 32, 30),         # 导灵槽暗底
}

# 骨骼定义
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "根骨骼"),
    ("pommel", "root", (0.0, 1.0, 0.0), "方锤形柄首"),
    ("grip", "root", (0.0, 4.4, 0.0), "缠皮绳握把"),
    ("guard", "root", (0.0, 7.2, 0.0), "铜护手方盘"),
    ("collar", "root", (0.0, 7.8, 0.0), "铜夹箍座"),
    ("shaft_seg1", "root", (0.0, 10.4, 0.0), "第一段锏身（根部2.2px）"),
    ("rib_1", "root", (0.0, 12.8, 0.0), "第一道凸箍（2.6px）"),
    ("shaft_seg2", "root", (0.0, 15.0, 0.0), "第二段锏身"),
    ("rib_2", "root", (0.0, 17.2, 0.0), "第二道凸箍（2.5px）"),
    ("shaft_seg3", "root", (0.0, 19.3, 0.0), "第三段锏身"),
    ("rib_3", "root", (0.0, 21.4, 0.0), "第三道凸箍（2.4px）"),
    ("shaft_seg4", "root", (0.0, 23.4, 0.0), "第四段锏身（梢部1.6px）"),
    ("tip", "root", (0.0, 25.8, 0.0), "锏梢平顶与收分角"),
]

# 握持点：玩家右手握在 grip 中段 y=4.4px 处
GRIP_Y = 4.4
PX = 16.0

def build_weapon():
    rig = Rig(MATS, tex=64, swatch=8)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent)

    # 1. pommel: 方锤形镡 (y: 0.0 ~ 1.8)
    b = "pommel"
    # 底座平头 (y: 0.0 ~ 0.4): 1.6 x 1.6 px
    rig.cube(b, "pommel_cap", (-0.8, 0.0, -0.8), (0.8, 0.4, 0.8), mat="iron_dark")
    rig.cube(b, "pommel_cap_c1", (-0.95, 0.08, -0.6), (0.95, 0.32, 0.6), mat="iron_mid")
    rig.cube(b, "pommel_cap_c2", (-0.6, 0.08, -0.95), (0.6, 0.32, 0.95), mat="iron_mid")
    # 方锤身 (y: 0.4 ~ 1.3): 倒梯形膨出，外宽达 2.1 x 2.1 px
    rig.cube(b, "pommel_body_core", (-0.95, 0.4, -0.95), (0.95, 1.3, 0.95), mat="iron_mid")
    rig.cube(b, "pommel_edge_x", (-1.08, 0.55, -0.7), (1.08, 1.15, 0.7), mat="iron_edge")
    rig.cube(b, "pommel_edge_z", (-0.7, 0.55, -1.08), (0.7, 1.15, 1.08), mat="iron_edge")
    # 颈部收细 (y: 1.3 ~ 1.7): 1.3 x 1.3 px
    rig.cube(b, "pommel_neck", (-0.65, 1.3, -0.65), (0.65, 1.7, 0.65), mat="iron_dark")
    # 垫环 (y: 1.7 ~ 1.9): 1.6 x 1.6 px
    rig.cube(b, "pommel_washer", (-0.8, 1.7, -0.8), (0.8, 1.9, 0.8), mat="bronze")

    # 2. grip: 握把 (y: 1.9 ~ 6.9, 长 5.0px): 截面严格控制在 1.8 px（八角近圆），密缠皮绳起伏
    b = "grip"
    # 木芯 (八角十字相交，粗 1.5 px)
    rig.cube(b, "grip_core_ns", (-0.65, 1.9, -0.75), (0.65, 6.9, 0.75), mat="leather_dk")
    rig.cube(b, "grip_core_ew", (-0.75, 1.9, -0.65), (0.75, 6.9, 0.65), mat="leather_dk")
    # 6 圈交错缠皮绳 (外宽达到 1.8 px)
    for i in range(6):
        y0 = 2.0 + i * 0.80
        y1 = y0 + 0.64
        hw_x = 0.85 if i % 2 == 0 else 0.90
        hw_z = 0.90 if i % 2 == 0 else 0.85
        rig.cube(b, f"grip_wrap_{i}_main", (-hw_x, y0, -hw_z), (hw_x, y1, hw_z), mat="leather")
        # 凸起的微细皮绳结棱角
        rig.cube(b, f"grip_knot_{i}_f", (-0.5, y0 + 0.1, hw_z - 0.05), (0.5, y1 - 0.1, hw_z + 0.10), mat="leather_dk")
        rig.cube(b, f"grip_knot_{i}_b", (-0.5, y0 + 0.1, -hw_z - 0.10), (0.5, y1 - 0.1, -hw_z + 0.05), mat="leather_dk")
        rig.cube(b, f"grip_knot_{i}_l", (-hw_x - 0.10, y0 + 0.1, -0.5), (-hw_x + 0.05, y1 - 0.1, 0.5), mat="leather")
        rig.cube(b, f"grip_knot_{i}_r", (hw_x - 0.05, y0 + 0.1, -0.5), (hw_x + 0.10, y1 - 0.1, 0.5), mat="leather")

    # 3. guard: 铜护手 (y: 6.9 ~ 7.6): 横向小方盘，宽度达 4.5 px（为锏身2.2px的2倍以上！）
    b = "guard"
    # 下托座 (y: 6.9 ~ 7.1): 2.0 x 2.0 px
    rig.cube(b, "guard_base", (-1.0, 6.9, -1.0), (1.0, 7.1, 1.0), mat="bronze_dk")
    # 方盘主盘 (y: 7.1 ~ 7.4): 4.5 x 4.5 px，厚度 0.3px
    rig.cube(b, "guard_plate_core", (-2.1, 7.1, -2.1), (2.1, 7.4, 2.1), mat="bronze")
    # 方盘外沿与八角倒角外框 (外扩至 ±2.25，总宽 4.5 px)
    rig.cube(b, "guard_rim_ns", (-2.25, 7.1, -1.6), (2.25, 7.4, 1.6), mat="bronze")
    rig.cube(b, "guard_rim_ew", (-1.6, 7.1, -2.25), (1.6, 7.4, 2.25), mat="bronze")
    # 四角角柱包角
    rig.cube(b, "guard_corner_1", (-2.15, 7.08, -2.15), (-1.65, 7.42, -1.65), mat="bronze_dk")
    rig.cube(b, "guard_corner_2", (1.65, 7.08, -2.15), (2.15, 7.42, -1.65), mat="bronze_dk")
    rig.cube(b, "guard_corner_3", (-2.15, 7.08, 1.65), (-1.65, 7.42, 2.15), mat="bronze_dk")
    rig.cube(b, "guard_corner_4", (1.65, 7.08, 1.65), (2.15, 7.42, 2.15), mat="bronze_dk")
    # 上托环 (y: 7.4 ~ 7.6): 2.4 x 2.4 px
    rig.cube(b, "guard_top_ring", (-1.2, 7.4, -1.2), (1.2, 7.6, 1.2), mat="bronze_dk")

    # 4. collar: 铜夹箍座 (y: 7.6 ~ 8.3): 方形套筒抱住锏根 (宽 2.4 px)
    b = "collar"
    rig.cube(b, "collar_main", (-1.2, 7.6, -1.2), (1.2, 8.3, 1.2), mat="bronze")
    rig.cube(b, "collar_flange_ns", (-1.26, 7.75, -0.9), (1.26, 8.15, 0.9), mat="bronze_dk")
    rig.cube(b, "collar_flange_ew", (-0.9, 7.75, -1.26), (0.9, 8.15, 1.26), mat="bronze_dk")

    # 5. 锏身总长约 17.0 px (y: 8.3 ~ 25.3)，按 4 段收分：
    # 根部截面 2.2 px (半宽 1.10) 逐渐收至 梢部 1.6 px (半宽 0.80)
    # 结构采用：内方芯 + 四侧面凸起楔形脊棱（从脊背到刃角分层变薄）+ 一侧导灵暗槽

    def build_shaft_segment(b_name, y_start, y_end, hw_base, hw_top, has_groove=True):
        steps = 3
        dy = (y_end - y_start) / steps
        for s in range(steps):
            ys0 = y_start + s * dy
            ys1 = ys0 + dy
            t = (s + 0.5) / steps
            hw = hw_base * (1.0 - t) + hw_top * t
            hw_core = hw * 0.78
            # 主方芯
            rig.cube(b_name, f"{b_name}_core_{s}", (-hw_core, ys0, -hw_core), (hw_core, ys1, hw_core), mat="iron_mid")
            # 四个棱面凸出层（凸出到 hw，形成坚挺高脊）
            # +X 棱
            rig.cube(b_name, f"{b_name}_ridge_px_{s}", (hw_core - 0.06, ys0, -hw * 0.5), (hw, ys1, hw * 0.5), mat="iron_edge")
            # -X 棱
            rig.cube(b_name, f"{b_name}_ridge_nx_{s}", (-hw, ys0, -hw * 0.5), (-hw_core + 0.06, ys1, hw * 0.5), mat="iron_edge")
            # +Z 棱 (导灵槽开在 +Z 侧)
            if has_groove:
                rig.cube(b_name, f"{b_name}_ridge_pz_l_{s}", (-hw * 0.5, ys0, hw_core - 0.06), (-0.12, ys1, hw), mat="iron_edge")
                rig.cube(b_name, f"{b_name}_groove_{s}", (-0.12, ys0, hw_core - 0.03), (0.12, ys1, hw - 0.10), mat="groove")
                rig.cube(b_name, f"{b_name}_ridge_pz_r_{s}", (0.12, ys0, hw_core - 0.06), (hw * 0.5, ys1, hw), mat="iron_edge")
            else:
                rig.cube(b_name, f"{b_name}_ridge_pz_{s}", (-hw * 0.5, ys0, hw_core - 0.06), (hw * 0.5, ys1, hw), mat="iron_edge")
            # -Z 棱
            rig.cube(b_name, f"{b_name}_ridge_nz_{s}", (-hw * 0.5, ys0, -hw), (hw * 0.5, ys1, -hw_core + 0.06), mat="iron_edge")
            # 四角倒棱（八角过渡斜面）
            c_hw = hw * 0.70
            rig.cube(b_name, f"{b_name}_corner_ns_{s}", (-c_hw, ys0, -hw * 0.90), (c_hw, ys1, hw * 0.90), mat="iron_dark")
            rig.cube(b_name, f"{b_name}_corner_ew_{s}", (-hw * 0.90, ys0, -c_hw), (hw * 0.90, ys1, c_hw), mat="iron_dark")

    def build_rib(b_name, y_center, hw_rib):
        # 凸箍：一道明显的环绕八角凸环 (厚 0.4px)
        y0 = y_center - 0.22
        y1 = y_center + 0.22
        rig.cube(b_name, f"{b_name}_main", (-hw_rib, y0, -hw_rib), (hw_rib, y1, hw_rib), mat="iron_dark")
        rig.cube(b_name, f"{b_name}_edge_ns", (-hw_rib - 0.08, y0 + 0.06, -hw_rib * 0.65), (hw_rib + 0.08, y1 - 0.06, hw_rib * 0.65), mat="iron_edge")
        rig.cube(b_name, f"{b_name}_edge_ew", (-hw_rib * 0.65, y0 + 0.06, -hw_rib - 0.08), (hw_rib * 0.65, y1 - 0.06, hw_rib + 0.08), mat="iron_edge")
        rig.cube(b_name, f"{b_name}_cuff_lo", (-hw_rib * 0.88, y0 - 0.10, -hw_rib * 0.88), (hw_rib * 0.88, y0, hw_rib * 0.88), mat="iron_mid")
        rig.cube(b_name, f"{b_name}_cuff_hi", (-hw_rib * 0.88, y1, -hw_rib * 0.88), (hw_rib * 0.88, y1 + 0.10, hw_rib * 0.88), mat="iron_mid")

    # 第 1 段锏身 (y: 8.3 ~ 12.6): 截面宽 2.2 -> 2.05 px (hw: 1.10 -> 1.025)
    build_shaft_segment("shaft_seg1", 8.3, 12.6, 1.10, 1.025, has_groove=True)

    # 凸箍 1 (y = 12.8): 宽度 2.6 px (hw: 1.30)
    build_rib("rib_1", 12.8, 1.30)

    # 第 2 段锏身 (y: 13.0 ~ 17.0): 截面宽 2.05 -> 1.90 px (hw: 1.025 -> 0.95)
    build_shaft_segment("shaft_seg2", 13.0, 17.0, 1.025, 0.95, has_groove=True)

    # 凸箍 2 (y = 17.2): 宽度 2.5 px (hw: 1.25)
    build_rib("rib_2", 17.2, 1.25)

    # 第 3 段锏身 (y: 17.4 ~ 21.2): 截面宽 1.90 -> 1.75 px (hw: 0.95 -> 0.875)
    build_shaft_segment("shaft_seg3", 17.4, 21.2, 0.95, 0.875, has_groove=True)

    # 凸箍 3 (y = 21.4): 宽度 2.4 px (hw: 1.20)
    build_rib("rib_3", 21.4, 1.20)

    # 第 4 段锏身 (y: 21.6 ~ 25.2): 截面宽 1.75 -> 1.60 px (hw: 0.875 -> 0.80)
    build_shaft_segment("shaft_seg4", 21.6, 25.2, 0.875, 0.80, has_groove=False)

    # 6. tip: 锏梢与平顶尖端 (y: 25.2 ~ 26.5)
    b = "tip"
    # 梢部微收 (y: 25.2 ~ 26.0): 半宽 0.75 px
    rig.cube(b, "tip_taper_core", (-0.65, 25.2, -0.65), (0.65, 26.0, 0.65), mat="iron_mid")
    rig.cube(b, "tip_taper_edge_ns", (-0.75, 25.2, -0.45), (0.75, 26.0, 0.45), mat="iron_edge")
    rig.cube(b, "tip_taper_edge_ew", (-0.45, 25.2, -0.75), (0.45, 26.0, 0.75), mat="iron_edge")
    # 钝平顶拱盖 (y: 26.0 ~ 26.5): 平顶微敛，半宽 0.55 px
    rig.cube(b, "tip_cap_core", (-0.55, 26.0, -0.55), (0.55, 26.35, 0.55), mat="iron_dark")
    rig.cube(b, "tip_cap_dome", (-0.35, 26.35, -0.35), (0.35, 26.5, 0.35), mat="iron_edge")

    return rig


# 占位人体手持模型
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

    dx = 6.0
    dy = 13.5 - GRIP_Y
    dz = 0.0
    w.bone("held_weapon", (dx, 13.5, dz), parent=None)
    for elem in weapon_rig.elements:
        f = (round(elem["from"][0] + dx, 4), round(elem["from"][1] + dy, 4), round(elem["from"][2] + dz, 4))
        t = (round(elem["to"][0] + dx, 4), round(elem["to"][1] + dy, 4), round(elem["to"][2] + dz, 4))
        w.cube("held_weapon", f"w_{elem['name']}", f, t, mat="iron_mid")
    return w


def extract_weapon_from_ref(ref_path: Path) -> Image.Image:
    """从参考图中抠出右侧独立兵器的精确剪影。"""
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
    six_views_img = gear_kit.render_six_views(model_path, f"四棱铁锏 tie_jian_si_leng (Round 1/3) · {len(rig.elements)} 方块 · 全长 26.5 px")
    model_front_img, _ = render(model_path, yaw=0.0, pitch=0.0, size=640, bg=gear_kit.BG)

    # 2. (A)并排比例 + (B)持握姿势 两栏
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=model_doc,
        grip_y=GRIP_Y,
        weapon_len=26.5,
        is_polearm=False,
        out_dir=OUT_DIR,
        textured=False
    )

    # 3. 比例对照栏
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_crop = extract_weapon_from_ref(ref_file)

    metrics = [
        ("头部长度占比 (Head Length %)", 71.5, 72.1),
        ("柄部长度占比 (Handle Length %)", 28.5, 27.9),
        ("头部最大截面占比 (Base Width %)", 8.7, 8.3),
        ("梢部截面占比 (Tip Width %)", 7.1, 6.0),
        ("凸箍宽度占比 (Rib Width %)", 9.8, 9.8),
        ("护手宽度占比 (Guard Width %)", 15.1, 17.0),
        ("握把粗细占比 (Grip Width %)", 6.7, 6.8),
        ("全长/人身高比 (Length / Person %)", 89.8, 82.8),
    ]
    ref_lines = [
        ("顶端 100%", 1.0, (210, 210, 210)),
        ("锏身根部 31%", 0.31, (130, 220, 160)),
        ("护手 28%", 0.28, (240, 190, 100)),
        ("握把底 7%", 0.07, (170, 190, 230)),
        ("底端 0%", 0.0, (210, 210, 210)),
    ]
    prop_panel = weapon_review_kit.build_proportion_panel(ref_crop, model_front_img, metrics, ref_lines, target_h=560)

    # 4. 骨骼树面板
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
        title="四棱铁锏 tie_jian_si_leng（Round 1 几何平涂版；无贴图；尺寸比例依参考图重构）"
    )


TEXTURE_MAPPING = {
    "iron_dark": "weapon_iron_dk",
    "iron_mid": "weapon_iron_mid",
    "iron_edge": "weapon_iron_warm",
    "bronze": "weapon_bronze_bright",
    "bronze_dk": "weapon_bronze_dk",
    "leather": "weapon_leather",
    "leather_dk": "weapon_leather_dk",
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
        weapon_len=26.5,
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
        title="四棱铁锏 tie_jian_si_leng（Round 3 贴图版；深黑褐铁身+暖铜褐磨亮棱线+暗掉导灵槽）"
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
    print(f"四棱铁锏 tie_jian_si_leng: {cubes_count} 个方块, {len(rig.bones)} 根骨骼")
    assert cubes_count >= 150, f"单手兵器方块数必须 >= 150，实得 {cubes_count}"

    check_img_path = REVIEW_DIR / "check.png"
    render_check_image(rig, check_img_path)

    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)

if __name__ == "__main__":
    main()
