#!/usr/bin/env python3
"""残卷（remnant_scroll / CanJuan）Blockbench .bbmodel 生成器。

参考图：`生成一份三视图 #34-1254 x 1254.png`（正 / 侧 / 背 / 透视三视图）。
优化重点：
  - **晶体结构聚集成堆、强烈突出**：
    1. 左下角巨型放射晶刺堆（多根长晶刺以 15°~55° 强烈向左下方放射突起，外挑至 X=-7.6，Y=-0.6）。
    2. 左上角冲天晶冠簇（主尖刺向上向外挺拔挑出）。
    3. 右下角外挑晶堆（双重斜刺与抱爪）。
    4. 右上角挺拔晶堆（上挑主尖与侧刺）。
    5. 中部留出干净自然卷轴边缘，晶体四角扎堆，剪影特征极强。
  - 晶石材质：深邃黑曜玄紫 + 帝王紫晶面 + 耀白锐利解理高光 + 根部紫白电芒。
  - 经卷主体：微弧起伏古卷，高精古篆书法与九叠篆朱砂大印。

用法:
  python3 modelScript/generators/gen_remnant_scroll.py
  python3 modelScript/generators/gen_remnant_scroll.py --check
  python3 modelScript/generators/gen_remnant_scroll.py --preview-only
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
MODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "RemnantScroll.bbmodel"
OUT_DIR = Path(__file__).resolve().parents[1] / "out"
PREVIEW_OUT = OUT_DIR / "remnant_scroll_preview.png"
THREE_VIEW_OUT = OUT_DIR / "remnant_scroll_render_three_view.png"

# 分辨率：256x256
TEX_W = 256
TEX_H = 256

BONE_ORDER = ["parchment", "crystals_left", "crystals_right", "crystals_top_bottom", "fringes"]
BONE_PIVOTS = {
    "parchment": [0.0, 8.0, 0.0],
    "crystals_left": [-4.8, 8.0, 0.0],
    "crystals_right": [4.8, 8.0, 0.0],
    "crystals_top_bottom": [0.0, 8.0, 0.0],
    "fringes": [0.0, 8.0, 0.0],
}
BONE_COLORS = {
    "parchment": (180, 150, 110),
    "crystals_left": (130, 40, 200),
    "crystals_right": (130, 40, 200),
    "crystals_top_bottom": (160, 60, 220),
    "fringes": (140, 115, 85),
}

UV_ZONES = {
    "front": (0, 0, 128, 192),
    "back": (128, 0, 256, 192),
    "crystal": (0, 192, 128, 256),
    "fringe": (128, 192, 256, 256),
}


@dataclass
class CubeDef:
    bone: str
    material: str  # "front", "back", "crystal", "fringe"
    name: str
    frm: list[float]
    to: list[float]
    origin: list[float] | None = None
    rotation: list[float] | None = None
    custom_uv: dict[str, list[float]] | None = None


def part_parchment_body() -> list[CubeDef]:
    """经卷主体：微弧起伏、收腰、边缘带有自然撕裂阶梯。"""
    cubes: list[CubeDef] = []

    # 纵向 5 段微弧过渡 (y: 1.6 ~ 16.5, 宽度 9.6~10.4)
    # 段 1 (底部 y: 1.6 ~ 4.8, 宽 10.2, 微前翘)
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_body_1_bot",
        frm=[-5.1, 1.6, 0.08], to=[5.1, 4.9, 0.58],
        origin=[0.0, 3.2, 0.33], rotation=[-2.0, 0.0, 0.5]
    ))
    # 段 2 (中下部 y: 4.8 ~ 8.0, 微收腰 9.6)
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_body_2_midlow",
        frm=[-4.8, 4.8, -0.15], to=[4.8, 8.1, 0.35],
        origin=[0.0, 6.5, 0.1], rotation=[1.2, 0.0, -0.3]
    ))
    # 段 3 (中上部 y: 8.0 ~ 11.5, 最收腰 9.3, 凹陷)
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_body_3_midhigh",
        frm=[-4.65, 8.0, -0.28], to=[4.65, 11.6, 0.22],
        origin=[0.0, 9.8, -0.03], rotation=[-0.8, 0.0, 0.4]
    ))
    # 段 4 (顶部中段 y: 11.5 ~ 14.5, 展开至 9.8)
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_body_4_topmid",
        frm=[-4.9, 11.5, -0.12], to=[4.9, 14.6, 0.38],
        origin=[0.0, 13.0, 0.13], rotation=[1.5, 0.0, -0.2]
    ))
    # 段 5 (顶部边沿 y: 14.5 ~ 16.5, 上翘 10.0)
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_body_5_top",
        frm=[-5.0, 14.5, -0.25], to=[5.0, 16.6, 0.25],
        origin=[0.0, 15.5, 0.0], rotation=[3.2, 0.0, 0.6]
    ))

    # 左右边缘褶皱衬板
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_edge_under_l",
        frm=[-5.25, 2.5, -0.05], to=[-4.45, 15.2, 0.45],
        origin=[-4.85, 9.0, 0.2], rotation=[0.0, -3.0, 0.8]
    ))
    cubes.append(CubeDef(
        bone="parchment", material="front", name="parch_edge_under_r",
        frm=[4.45, 2.5, -0.05], to=[5.25, 15.2, 0.45],
        origin=[4.85, 9.0, 0.2], rotation=[0.0, 3.0, -0.8]
    ))

    return cubes


def part_parchment_torn_edges() -> list[CubeDef]:
    """经卷顶部与底部的撕裂毛边、断茬絮边。"""
    cubes: list[CubeDef] = []

    # 顶部参差撕裂断茬 (y: 16.2 ~ 17.6)
    top_fringes = [
        ("fringe_top_1", [-5.1, 16.3, -0.26], [-3.8, 17.4, 0.18], [-4.4, 16.4, 0.0], [-4.0, 2.0, 5.0]),
        ("fringe_top_2", [-3.9, 16.4, -0.22], [-2.3, 17.1, 0.20], [-3.1, 16.5, 0.0], [3.0, -2.0, -3.0]),
        ("fringe_top_3", [-2.4, 16.5, -0.20], [-0.8, 17.6, 0.22], [-1.6, 16.5, 0.0], [-2.0, 1.0, 2.0]),
        ("fringe_top_4", [-0.9, 16.4, -0.22], [0.8, 17.3, 0.20], [0.0, 16.5, 0.0], [4.0, -1.0, -2.5]),
        ("fringe_top_5", [0.7, 16.3, -0.24], [2.4, 17.5, 0.18], [1.5, 16.4, 0.0], [-3.0, 3.0, 4.0]),
        ("fringe_top_6", [2.3, 16.2, -0.28], [3.9, 17.0, 0.15], [3.1, 16.3, 0.0], [2.0, -3.0, -3.5]),
        ("fringe_top_7", [3.8, 16.1, -0.30], [5.2, 16.8, 0.12], [4.5, 16.2, 0.0], [-4.0, 4.0, 6.0]),
    ]
    for name, frm, to, orig, rot in top_fringes:
        cubes.append(CubeDef(
            bone="fringes", material="fringe", name=name,
            frm=frm, to=to, origin=orig, rotation=rot
        ))

    # 底部参差撕裂絮边 (y: 0.3 ~ 2.0)
    bottom_fringes = [
        ("fringe_bot_1", [-5.2, 0.4, 0.02], [-3.8, 1.8, 0.54], [-4.5, 1.6, 0.28], [5.0, -4.0, -3.0]),
        ("fringe_bot_2", [-3.9, 0.7, 0.05], [-2.3, 1.9, 0.50], [-3.1, 1.6, 0.28], [-3.0, 2.0, 2.0]),
        ("fringe_bot_3", [-2.4, 0.3, 0.08], [-0.7, 1.8, 0.46], [-1.5, 1.6, 0.27], [4.0, 0.0, -2.5]),
        ("fringe_bot_4", [-0.8, 0.5, 0.06], [0.9, 1.7, 0.48], [0.1, 1.6, 0.27], [-2.0, -2.0, 3.0]),
        ("fringe_bot_5", [0.8, 0.3, 0.08], [2.5, 1.9, 0.46], [1.6, 1.6, 0.27], [3.0, 3.0, -1.5]),
        ("fringe_bot_6", [2.4, 0.6, 0.04], [3.9, 1.8, 0.50], [3.1, 1.6, 0.27], [-4.0, -3.0, 2.5]),
        ("fringe_bot_7", [3.8, 0.4, 0.02], [5.3, 1.7, 0.54], [4.6, 1.6, 0.28], [4.0, 4.0, -4.0]),
    ]
    for name, frm, to, orig, rot in bottom_fringes:
        cubes.append(CubeDef(
            bone="fringes", material="fringe", name=name,
            frm=frm, to=to, origin=orig, rotation=rot
        ))

    return cubes


def part_crystals_left() -> list[CubeDef]:
    """左侧紫曜晶簇：四角扎堆成簇，强烈向外突出。"""
    cubes: list[CubeDef] = []

    # ═════════════════════════════════════════════════════════════════════════
    # 1. 左下角巨型放射状晶体堆（核心视觉焦点）
    # ═════════════════════════════════════════════════════════════════════════
    # 晶堆基核
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_bot_cluster_core",
        frm=[-5.8, 0.8, -0.65], to=[-4.4, 3.2, 0.65],
        origin=[-4.9, 2.0, 0.0], rotation=[10.0, -8.0, 22.0]
    ))
    # 主巨晶刺：向左下方强烈伸展，尖端突出至 X=-7.6, Y=-0.5
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_giant_spike_1",
        frm=[-7.6, -0.4, -0.55], to=[-4.5, 2.2, 0.55],
        origin=[-4.8, 1.5, 0.0], rotation=[15.0, -12.0, 38.0]
    ))
    # 伴生下刺：贴地下扫
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_giant_spike_2",
        frm=[-6.9, 0.4, -0.45], to=[-4.6, 1.8, 0.45],
        origin=[-4.8, 1.2, 0.0], rotation=[6.0, 18.0, 22.0]
    ))
    # 伴生上扬刺：向左上外侧挑出
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_giant_spike_3",
        frm=[-6.6, 1.6, -0.5], to=[-4.4, 3.8, 0.5],
        origin=[-4.7, 2.6, 0.0], rotation=[-12.0, 8.0, 48.0]
    ))
    # 晶簇基底抱爪（前）
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_bot_clasp_f",
        frm=[-5.5, 1.0, 0.25], to=[-4.3, 3.4, 0.85],
        origin=[-4.8, 2.2, 0.5], rotation=[6.0, -15.0, 16.0]
    ))
    # 晶簇基底抱爪（后）
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_bot_clasp_b",
        frm=[-5.5, 1.0, -0.85], to=[-4.3, 3.4, -0.25],
        origin=[-4.8, 2.2, -0.5], rotation=[-6.0, 15.0, 16.0]
    ))

    # ═════════════════════════════════════════════════════════════════════════
    # 2. 左上角突出晶冠簇（扎堆在左肩角，向上与向外突起）
    # ═════════════════════════════════════════════════════════════════════════
    # 晶冠基核
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_top_cluster_core",
        frm=[-5.8, 13.0, -0.55], to=[-4.4, 15.6, 0.55],
        origin=[-4.9, 14.3, 0.0], rotation=[-12.0, 10.0, -25.0]
    ))
    # 冲天晶尖刺（向上挑起，突出至 Y=18.0）
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_top_spire_main",
        frm=[-5.6, 14.8, -0.4], to=[-4.4, 18.0, 0.4],
        origin=[-4.8, 16.2, 0.0], rotation=[-6.0, -8.0, -18.0]
    ))
    # 侧向斜晶刺（向左外侧挑出至 X=-6.8）
    cubes.append(CubeDef(
        bone="crystals_left", material="crystal", name="crys_l_top_spire_side",
        frm=[-6.8, 13.8, -0.45], to=[-4.6, 15.6, 0.45],
        origin=[-4.9, 14.6, 0.0], rotation=[-15.0, 14.0, -42.0]
    ))

    return cubes


def part_crystals_right() -> list[CubeDef]:
    """右侧紫曜晶簇：右下与右上扎堆成簇，强烈向外突出。"""
    cubes: list[CubeDef] = []

    # ═════════════════════════════════════════════════════════════════════════
    # 1. 右下角突起晶堆
    # ═════════════════════════════════════════════════════════════════════════
    # 右下晶核
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_bot_cluster_core",
        frm=[4.4, 0.6, -0.55], to=[5.8, 3.2, 0.55],
        origin=[4.9, 1.9, 0.0], rotation=[10.0, 10.0, -22.0]
    ))
    # 右下主斜刺（突出至 X=7.0）
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_bot_spike_1",
        frm=[4.5, 0.1, -0.5], to=[7.0, 2.3, 0.5],
        origin=[4.8, 1.5, 0.0], rotation=[14.0, 12.0, -36.0]
    ))
    # 右下向上挑刺
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_bot_spike_2",
        frm=[4.5, 1.5, -0.45], to=[6.4, 3.5, 0.45],
        origin=[4.8, 2.4, 0.0], rotation=[-10.0, -10.0, -46.0]
    ))
    # 右下抱爪
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_bot_clasp_f",
        frm=[4.3, 0.9, 0.22], to=[5.5, 3.0, 0.8],
        origin=[4.8, 2.0, 0.5], rotation=[5.0, 12.0, -14.0]
    ))

    # ═════════════════════════════════════════════════════════════════════════
    # 2. 右上角突起晶堆
    # ═════════════════════════════════════════════════════════════════════════
    # 右上晶核
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_top_cluster_core",
        frm=[4.4, 13.2, -0.52], to=[5.8, 15.6, 0.52],
        origin=[4.9, 14.4, 0.0], rotation=[10.0, -10.0, 22.0]
    ))
    # 右上冲天晶尖（突出至 Y=17.8）
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_top_spire_main",
        frm=[4.4, 14.8, -0.38], to=[5.6, 17.8, 0.38],
        origin=[4.8, 16.0, 0.0], rotation=[5.0, 6.0, 16.0]
    ))
    # 右上外挑斜刺（突出至 X=6.7）
    cubes.append(CubeDef(
        bone="crystals_right", material="crystal", name="crys_r_top_spire_side",
        frm=[4.6, 13.8, -0.45], to=[6.7, 15.8, 0.45],
        origin=[4.9, 14.7, 0.0], rotation=[14.0, -14.0, 38.0]
    ))

    return cubes


def part_crystals_top_bottom() -> list[CubeDef]:
    """微型零星晶粒。"""
    cubes: list[CubeDef] = []

    # 顶边微晶点缀
    cubes.append(CubeDef(
        bone="crystals_top_bottom", material="crystal", name="crys_top_nodule_1",
        frm=[-1.5, 16.2, -0.3], to=[-0.7, 17.1, 0.3],
        origin=[-1.1, 16.6, 0.0], rotation=[8.0, 4.0, 10.0]
    ))

    return cubes


def all_cubes() -> list[CubeDef]:
    return (
        part_parchment_body()
        + part_parchment_torn_edges()
        + part_crystals_left()
        + part_crystals_right()
        + part_crystals_top_bottom()
    )


# ─── 高精度贴图合成 (256x256) ──────────────────────────────────────────────────

def make_texture(w: int = TEX_W, h: int = TEX_H, seed: int = 108) -> Image.Image:
    """生成 256x256 贴图：高质感古卷书法、九叠篆朱砂大印、高对比晶体切面与四角电芒。"""
    rng = np.random.default_rng(seed)
    img = Image.new("RGBA", (w, h), (0, 0, 0, 255))

    # ─────────────────────────────────────────────────────────────────────────
    # 1. 正面经卷 (0..128, 0..192)
    # ─────────────────────────────────────────────────────────────────────────
    fw, fh = 128, 192
    f_arr = np.zeros((fh, fw, 4), dtype=np.float32)
    f_arr[..., 3] = 255.0

    # 底色：参考图真实古卷质感 [160, 138, 108]
    base_col = np.array([162.0, 138.0, 108.0], dtype=np.float32)
    noise_fine = (rng.random((fh, fw, 1)) - 0.5) * 22.0
    noise_coarse = Image.fromarray((rng.random((fh // 4, fw // 4)) * 255).astype(np.uint8)).resize((fw, fh), Image.Resampling.BICUBIC)
    noise_coarse_arr = (np.array(noise_coarse, dtype=np.float32)[..., None] / 255.0 - 0.5) * 32.0

    fiber_x = np.sin(np.linspace(0, 48 * np.pi, fw))[None, :, None] * 6.0
    y_grid, x_grid = np.mgrid[0:fh, 0:fw]
    dist_x = np.minimum(x_grid, fw - 1 - x_grid) / (fw / 2.0)
    dist_y = np.minimum(y_grid, fh - 1 - y_grid) / (fh / 2.0)
    vignette = (1.0 - np.clip(np.minimum(dist_x * 2.2, 1.0) * np.minimum(dist_y * 2.5, 1.0), 0.0, 1.0))[..., None] * -42.0

    f_rgb = np.clip(base_col + noise_fine + noise_coarse_arr + fiber_x + vignette, 15.0, 240.0)
    f_arr[..., :3] = f_rgb
    front_img = Image.fromarray(f_arr.astype(np.uint8), "RGBA")
    f_draw = ImageDraw.Draw(front_img)

    # ── 绘制古篆行草书法（纵向排布，浓淡干湿兼备） ──
    cols = [
        (18, 16, 172),
        (34, 14, 174),
        (50, 18, 170),
        (66, 16, 168),
        (82, 18, 116),   # 印章上方
        (100, 20, 108),  # 印章上方右侧
        (114, 22, 90),   # 最右列
    ]

    ink_deep = (28, 22, 18, 245)
    ink_mid = (48, 38, 30, 220)
    ink_light = (78, 62, 48, 170)

    for cx, y_start, y_end in cols:
        cy = y_start
        while cy < y_end:
            ch_h = rng.integers(8, 16)
            ch_w = rng.integers(6, 11)
            style = rng.integers(0, 6)

            if style == 0:  # 纵向悬针竖 + 左右两点
                f_draw.line([(cx, cy), (cx + rng.integers(-1, 2), cy + ch_h - 1)], fill=ink_deep, width=2)
                f_draw.line([(cx - ch_w // 2, cy + 3), (cx - 1, cy + 5)], fill=ink_mid, width=1)
                f_draw.line([(cx + 1, cy + 6), (cx + ch_w // 2, cy + 8)], fill=ink_deep, width=1)
                f_draw.line([(cx, cy + ch_h - 1), (cx - 2, cy + ch_h - 3)], fill=ink_deep, width=1)
            elif style == 1:  # 方折回纹
                f_draw.rectangle([cx - ch_w // 2, cy, cx + ch_w // 2, cy + ch_h - 1], outline=ink_deep, width=1)
                f_draw.line([(cx - ch_w // 2 + 2, cy + ch_h // 2), (cx + ch_w // 2 - 2, cy + ch_h // 2)], fill=ink_mid)
                f_draw.line([(cx, cy + 2), (cx, cy + ch_h - 3)], fill=ink_light)
            elif style == 2:  # 草书连绵撇捺
                pts = [
                    (cx - ch_w // 2, cy + 2),
                    (cx + ch_w // 2, cy + 3),
                    (cx - 2, cy + ch_h // 2),
                    (cx + ch_w // 2 - 1, cy + ch_h - 2),
                    (cx - ch_w // 2 + 1, cy + ch_h - 1)
                ]
                f_draw.line(pts, fill=ink_deep, width=2)
            elif style == 3:  # 密点与短横排叠
                f_draw.line([(cx - ch_w // 2, cy), (cx + ch_w // 2, cy)], fill=ink_deep, width=2)
                f_draw.line([(cx - ch_w // 2 + 1, cy + 3), (cx + ch_w // 2 - 1, cy + 3)], fill=ink_mid, width=1)
                f_draw.line([(cx, cy + 4), (cx, cy + ch_h - 2)], fill=ink_deep, width=1)
                f_draw.point((cx - 2, cy + ch_h - 1), fill=ink_deep)
                f_draw.point((cx + 2, cy + ch_h - 1), fill=ink_deep)
            elif style == 4:  # 飞白枯笔
                f_draw.line([(cx + 1, cy), (cx - ch_w // 2, cy + ch_h - 1)], fill=ink_light, width=2)
                f_draw.line([(cx - 1, cy + 2), (cx + ch_w // 2, cy + ch_h - 2)], fill=ink_mid, width=1)
            else:  # 复合篆体
                f_draw.arc([cx - ch_w // 2, cy, cx + ch_w // 2, cy + ch_h // 2], 0, 180, fill=ink_deep, width=1)
                f_draw.line([(cx, cy + ch_h // 2), (cx, cy + ch_h - 1)], fill=ink_deep, width=2)
                f_draw.line([(cx - ch_w // 2 + 1, cy + ch_h - 2), (cx + ch_w // 2 - 1, cy + ch_h - 2)], fill=ink_mid)

            cy += ch_h + rng.integers(3, 7)

    # ── 绘制右下角朱砂大印 (位于 x: 92, y: 144, 半径 18) ──
    scx, scy, sr = 92, 144, 18
    seal_color_main = (184, 42, 28, 235)
    seal_color_bright = (226, 68, 46, 255)
    seal_color_dark = (135, 24, 16, 200)

    f_draw.ellipse([scx - sr, scy - sr, scx + sr, scy + sr], outline=seal_color_bright, width=2)
    f_draw.ellipse([scx - sr + 2, scy - sr + 2, scx + sr - 2, scy + sr - 2], outline=seal_color_main, width=1)

    f_draw.line([(scx, scy - sr + 3), (scx, scy + sr - 3)], fill=seal_color_bright, width=1)
    f_draw.line([(scx - sr + 3, scy), (scx + sr - 3, scy)], fill=seal_color_bright, width=1)

    f_draw.rectangle([scx - 12, scy - 12, scx - 4, scy - 4], outline=seal_color_bright, width=1)
    f_draw.line([(scx - 8, scy - 11), (scx - 8, scy - 5)], fill=seal_color_main)
    f_draw.arc([scx + 4, scy - 12, scx + 12, scy - 4], 0, 270, fill=seal_color_bright, width=1)
    f_draw.line([(scx + 8, scy - 9), (scx + 11, scy - 5)], fill=seal_color_dark)
    f_draw.line([(scx - 12, scy + 5), (scx - 4, scy + 12)], fill=seal_color_bright, width=1)
    f_draw.line([(scx - 4, scy + 5), (scx - 12, scy + 12)], fill=seal_color_main, width=1)
    f_draw.rectangle([scx + 4, scy + 4, scx + 12, scy + 12], outline=seal_color_main, width=1)
    f_draw.point((scx + 8, scy + 8), fill=seal_color_bright)

    for _ in range(35):
        ox, oy = rng.integers(-sr, sr), rng.integers(-sr, sr)
        if ox*ox + oy*oy < sr*sr:
            f_draw.point((scx + ox, scy + oy), fill=(162, 138, 108, 180))

    # ── 围绕四角晶簇扎堆区域的紫电脉络 ──
    glow_core = (255, 248, 255, 255)
    glow_inner = (230, 150, 255, 245)
    glow_mid = (175, 55, 240, 200)
    glow_outer = (110, 25, 175, 130)

    def render_energy_crackle(start_pt, end_pt, num_segments=6):
        pts = [start_pt]
        cur = start_pt
        for i in range(1, num_segments):
            progress = i / num_segments
            target_x = start_pt[0] + (end_pt[0] - start_pt[0]) * progress
            target_y = start_pt[1] + (end_pt[1] - start_pt[1]) * progress
            nx = target_x + rng.uniform(-3.0, 3.0)
            ny = target_y + rng.uniform(-3.0, 3.0)
            pts.append((nx, ny))
        pts.append(end_pt)

        for i in range(len(pts) - 1):
            p0, p1 = pts[i], pts[i+1]
            f_draw.line([p0, p1], fill=glow_outer, width=4)
            f_draw.line([p0, p1], fill=glow_mid, width=2)
            f_draw.line([p0, p1], fill=glow_inner, width=1)
            f_draw.line([p0, p1], fill=glow_core, width=1)

    # 左下巨晶堆向内放射紫电
    render_energy_crackle((0, 155), (24, 142))
    render_energy_crackle((0, 170), (28, 158))
    render_energy_crackle((0, 185), (20, 175))

    # 左上晶冠向内放射紫电
    render_energy_crackle((0, 20), (22, 28))
    render_energy_crackle((0, 35), (26, 45))

    # 右下晶堆向内放射紫电
    render_energy_crackle((127, 160), (105, 150))
    render_energy_crackle((127, 175), (108, 168))

    # 右上晶堆向内放射紫电
    render_energy_crackle((127, 25), (106, 32))
    render_energy_crackle((127, 40), (102, 50))

    img.paste(front_img, (0, 0))

    # ─────────────────────────────────────────────────────────────────────────
    # 2. 背面经卷 (128..256, 0..192)
    # ─────────────────────────────────────────────────────────────────────────
    b_arr = np.zeros((fh, fw, 4), dtype=np.float32)
    b_arr[..., 3] = 255.0

    back_col = np.array([108.0, 88.0, 68.0], dtype=np.float32)
    b_noise = (rng.random((fh, fw, 1)) - 0.5) * 36.0
    b_coarse = (np.array(noise_coarse, dtype=np.float32)[..., None] / 255.0 - 0.5) * 45.0
    cracks = ((x_grid * 2 + y_grid * 3) % 29 < 1) | ((y_grid * 3 - x_grid * 2) % 37 < 1)

    b_rgb = np.clip(back_col + b_noise + b_coarse - cracks[..., None] * 32.0 + vignette * 1.3, 10.0, 220.0)
    b_arr[..., :3] = b_rgb
    back_img = Image.fromarray(b_arr.astype(np.uint8), "RGBA")
    b_draw = ImageDraw.Draw(back_img)

    def render_back_crackle(start_pt, end_pt):
        pts = [start_pt]
        for i in range(1, 5):
            nx = start_pt[0] + (end_pt[0] - start_pt[0]) * (i / 5) + rng.uniform(-2, 2)
            ny = start_pt[1] + (end_pt[1] - start_pt[1]) * (i / 5) + rng.uniform(-2, 2)
            pts.append((nx, ny))
        pts.append(end_pt)
        for i in range(len(pts) - 1):
            b_draw.line([pts[i], pts[i+1]], fill=glow_outer, width=3)
            b_draw.line([pts[i], pts[i+1]], fill=glow_inner, width=1)

    render_back_crackle((0, 30), (18, 38))
    render_back_crackle((0, 165), (20, 155))
    render_back_crackle((127, 35), (108, 45))
    render_back_crackle((127, 165), (106, 158))

    img.paste(back_img, (128, 0))

    # ─────────────────────────────────────────────────────────────────────────
    # 3. 晶体解理面材质 (0..128, 192..256) —— 高对比度紫晶解理切面
    # ─────────────────────────────────────────────────────────────────────────
    cw, ch = 128, 64
    c_arr = np.zeros((ch, cw, 4), dtype=np.float32)
    c_arr[..., 3] = 255.0

    cy_grid, cx_grid = np.mgrid[0:ch, 0:cw]

    # 强对比玄紫黑曜基底到艳紫晶棱
    c_base_grad = (cx_grid / cw * 0.5 + cy_grid / ch * 0.5)
    facet_stripes = np.sin(cx_grid * 0.22 + cy_grid * 0.15) * 0.5 + 0.5
    facet_diagonals = np.cos(cx_grid * 0.3 - cy_grid * 0.4) * 0.5 + 0.5

    cr = 32.0 + c_base_grad * 110.0 + facet_stripes * 55.0 + facet_diagonals * 40.0
    cg = 10.0 + c_base_grad * 42.0 + facet_stripes * 30.0 + facet_diagonals * 55.0
    cb = 65.0 + c_base_grad * 145.0 + facet_stripes * 65.0 + facet_diagonals * 60.0

    c_rgb = np.stack([cr, cg, cb], axis=-1)
    c_rgb += (rng.random((ch, cw, 1)) - 0.5) * 18.0
    c_arr[..., :3] = np.clip(c_rgb, 6.0, 255.0)
    crys_img = Image.fromarray(c_arr.astype(np.uint8), "RGBA")
    c_draw = ImageDraw.Draw(crys_img)

    # 耀白晶棱高光
    for _ in range(16):
        x0 = rng.integers(0, cw)
        y0 = rng.integers(0, ch)
        length = rng.integers(14, 32)
        angle = rng.choice([-0.75, -0.4, 0.4, 0.85])
        x1 = int(x0 + length * math.cos(angle))
        y1 = int(y0 + length * math.sin(angle))
        c_draw.line([(x0, y0), (x1, y1)], fill=(255, 240, 255, 235), width=1)

    img.paste(crys_img, (0, 192))

    # ─────────────────────────────────────────────────────────────────────────
    # 4. 毛边与碎屑材质 (128..256, 192..256)
    # ─────────────────────────────────────────────────────────────────────────
    rw, rh = 128, 64
    r_arr = np.zeros((rh, rw, 4), dtype=np.float32)
    r_arr[..., 3] = 255.0
    r_base = np.array([142.0, 118.0, 86.0], dtype=np.float32)
    r_noise = (rng.random((rh, rw, 1)) - 0.5) * 38.0
    r_stripes = (np.sin(np.linspace(0, 32 * np.pi, rw))[None, :, None]) * 18.0
    r_rgb = np.clip(r_base + r_noise + r_stripes, 20.0, 225.0)
    r_arr[..., :3] = r_rgb
    fringe_img = Image.fromarray(r_arr.astype(np.uint8), "RGBA")
    fr_draw = ImageDraw.Draw(fringe_img)

    for _ in range(60):
        fx = rng.integers(0, rw)
        fy = rng.integers(0, rh)
        fr_draw.point((fx, fy), fill=(70, 52, 35, 255))
        fr_draw.point((fx, (fy + 1) % rh), fill=(215, 192, 145, 255))

    img.paste(fringe_img, (128, 192))

    return img


# ─── UV 映射计算 ──────────────────────────────────────────────────────────────

class UVZonePacker:
    """按部件包围盒与材质类型，智能分派与投影 UV 坐标。"""

    def __init__(self):
        self.crystal_idx = 0
        self.fringe_idx = 0

    def compute_faces_uv(self, cube: CubeDef) -> dict[str, dict]:
        faces = {}
        mat = cube.material

        if mat == "front":
            x0, x1 = min(cube.frm[0], cube.to[0]), max(cube.frm[0], cube.to[0])
            y0, y1 = min(cube.frm[1], cube.to[1]), max(cube.frm[1], cube.to[1])

            # 归一化到 [0..1]
            u0 = np.clip((x0 + 5.3) / 10.6, 0.0, 1.0)
            u1 = np.clip((x1 + 5.3) / 10.6, 0.0, 1.0)
            v0 = np.clip((17.0 - y1) / 16.0, 0.0, 1.0)
            v1 = np.clip((17.0 - y0) / 16.0, 0.0, 1.0)

            # 正面 (north)
            fn_u0, fn_u1 = u0 * 128.0, u1 * 128.0
            fn_v0, fn_v1 = v0 * 192.0, v1 * 192.0
            faces["north"] = {"uv": [round(fn_u0, 2), round(fn_v0, 2), round(fn_u1, 2), round(fn_v1, 2)], "texture": 0}

            # 背面 (south) 水平镜像
            bs_u0, bs_u1 = 128.0 + (1.0 - u1) * 128.0, 128.0 + (1.0 - u0) * 128.0
            bs_v0, bs_v1 = v0 * 192.0, v1 * 192.0
            faces["south"] = {"uv": [round(bs_u0, 2), round(bs_v0, 2), round(bs_u1, 2), round(bs_v1, 2)], "texture": 0}

            # 侧面与顶底走毛边/侧边带
            faces["east"] = {"uv": [136.0, 200.0, 144.0, 240.0], "texture": 0}
            faces["west"] = {"uv": [148.0, 200.0, 156.0, 240.0], "texture": 0}
            faces["up"] = {"uv": [160.0, 200.0, 220.0, 208.0], "texture": 0}
            faces["down"] = {"uv": [160.0, 224.0, 220.0, 232.0], "texture": 0}

        elif mat == "crystal":
            self.crystal_idx += 1
            slot_x = (self.crystal_idx % 8) * 16.0
            slot_y = 192.0 + ((self.crystal_idx // 8) % 4) * 16.0
            for f in ["north", "south", "east", "west", "up", "down"]:
                faces[f] = {"uv": [slot_x, slot_y, slot_x + 15.0, slot_y + 15.0], "texture": 0}

        elif mat == "fringe":
            self.fringe_idx += 1
            slot_x = 128.0 + (self.fringe_idx % 8) * 16.0
            slot_y = 192.0 + ((self.fringe_idx // 8) % 4) * 16.0
            for f in ["north", "south", "east", "west", "up", "down"]:
                faces[f] = {"uv": [slot_x, slot_y, slot_x + 15.0, slot_y + 15.0], "texture": 0}

        return faces


# ─── Blockbench .bbmodel 组装 ──────────────────────────────────────────────────

def build_bbmodel() -> tuple[dict, list[CubeDef], Image.Image]:
    cubes = all_cubes()
    packer = UVZonePacker()
    elements = []
    bone_children = {bone: [] for bone in BONE_ORDER}

    for cube in cubes:
        euid = str(uuid.uuid4())
        elem = {
            "name": cube.name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": euid,
            "from": [round(v, 3) for v in cube.frm],
            "to": [round(v, 3) for v in cube.to],
            "autouv": 0,
            "color": BONE_ORDER.index(cube.bone),
            "origin": [round(v, 3) for v in (cube.origin or cube.frm)],
            "faces": packer.compute_faces_uv(cube),
        }
        if cube.rotation is not None:
            elem["rotation"] = [round(v, 2) for v in cube.rotation]

        elements.append(elem)
        bone_children[cube.bone].append(euid)

    outliner = []
    for bone in BONE_ORDER:
        outliner.append({
            "name": bone,
            "origin": list(BONE_PIVOTS[bone]),
            "color": BONE_ORDER.index(bone),
            "uuid": str(uuid.uuid4()),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children[bone],
        })

    tex = make_texture()
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    data_url = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()

    model = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "remnant_scroll",
        "model_identifier": "geometry.bong.remnant_scroll",
        "visible_box": [1, 2, 0.5],
        "resolution": {"width": TEX_W, "height": TEX_H},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "",
            "name": "remnant_scroll.png",
            "folder": "item",
            "namespace": "bong",
            "id": "0",
            "width": TEX_W,
            "height": TEX_H,
            "uv_width": TEX_W,
            "uv_height": TEX_H,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": str(uuid.uuid4()),
            "source": data_url,
        }],
    }

    return model, cubes, tex


# ─── 自检与渲染 ───────────────────────────────────────────────────────────────

def run_self_check(cubes: list[CubeDef]) -> None:
    """执行几何健康与特征校验。"""
    print(f"[*] 执行自检: 总计 {len(cubes)} 个 Cuboid 元素...")

    assert len(cubes) >= 25, f"模型精细度不足: 仅 {len(cubes)} 个 cube"

    min_x = min(c.frm[0] for c in cubes)
    max_x = max(c.to[0] for c in cubes)
    min_y = min(c.frm[1] for c in cubes)
    max_y = max(c.to[1] for c in cubes)
    min_z = min(c.frm[2] for c in cubes)
    max_z = max(c.to[2] for c in cubes)

    width = max_x - min_x
    height = max_y - min_y
    depth = max_z - min_z

    print(f"[*] 包围盒尺寸: X={width:.2f}, Y={height:.2f}, Z={depth:.2f}")
    assert 11.0 <= width <= 18.0, f"宽度 {width} 不符合残卷比例 (期望 11~18)"
    assert 15.0 <= height <= 20.0, f"高度 {height} 不符合残卷比例 (期望 15~20)"
    assert 1.0 <= depth <= 5.0, f"厚度 {depth} 不符合残卷比例 (期望 1.0~5)"

    has_large_left_spike = any("giant_spike" in c.name and "crys_l" in c.name for c in cubes)
    assert has_large_left_spike, "缺失左侧标志性紫晶大尖刺"

    has_fringes = any(c.bone == "fringes" for c in cubes)
    assert has_fringes, "缺失上下撕裂毛边"

    print("[✓] 自检全部通过！")


def main():
    parser = argparse.ArgumentParser(description="生成残卷 .bbmodel 与预览图")
    parser.add_argument("--check", action="store_true", help="仅运行自检")
    parser.add_argument("--preview-only", action="store_true", help="仅生成预览图")
    args = parser.parse_args()

    model, cubes, tex = build_bbmodel()
    run_self_check(cubes)

    if args.check:
        return

    MODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
    MODEL_OUT.write_text(json.dumps(model, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"[✓] 成功写入模型: {MODEL_OUT}")

    tex_path = OUT_DIR / "remnant_scroll_texture.png"
    tex.save(tex_path)
    print(f"[✓] 成功保存贴图: {tex_path}")

    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
    try:
        import render_bbmodel
        print("[*] 正在渲染三视图与等轴透视图...")
        three_view_img, _ = render_bbmodel.render_three_view(MODEL_OUT)
        three_view_img.save(THREE_VIEW_OUT)
        print(f"[✓] 三视图渲染成功: {THREE_VIEW_OUT}")

        iso_view_img, _ = render_bbmodel.render(MODEL_OUT, yaw=145, pitch=22, size=512)
        iso_view_img.save(PREVIEW_OUT)
        print(f"[✓] 透视预览图渲染成功: {PREVIEW_OUT}")

    except Exception as e:
        print(f"[!] 调用渲染器遇到异常: {e}")


if __name__ == "__main__":
    main()
