#!/usr/bin/env python3
"""末法残土兵甲手套 (BingJiaShouTao / bing_jia_shou_tao) Blockbench .bbmodel 生成器。

严格依据 three_view.png 与调度审部件反馈打磨：
- dorsal_plate / knuckle_plates (金属板)：锈蚀铁板质感，灰 #6a6a70 与亮 #8a8a90 斑驳交错，夹杂暗锈坑点 #4a4440、
  少量锈褐 #6a4a32，四周边缘磨亮；保留圆拱形金属固定铆钉。
  knuckle_plates 彻底重构为 4 块弧形铁甲片，分别独立盖在 4 根指头上（每块略拱起，带独立铆钉），杜绝整条平整平板。
- leather_base / thumb_guard (皮革)：磨旧棕皮质感，#6a4a32 / #8a6448 斑驳磨损，边缘有一道暗凹缝线暗纹与浅色针脚。
  leather_base 在指部直接做出 4 根清晰并排的独立手指（小指、无名指、中指、食指，留缝分明，清晰位于指节甲片下）。
  拇指单独一块外展包覆。
- wrist_straps (腕带)：两条紧束皮带，外侧配置方形银灰带扣 #9a9aa0（2×2 镂空方框，中间空透出皮带），带扣针与固定铆钉。

门禁与自检：
  - _assert_no_coplanar_faces 检查共面冲突 (Z-fighting)
  - --self-test 注入缺陷自证门禁有效性
  - --export-parts 逐个导出各部件并在中灰背景 (122, 122, 122) 下渲染存入 parts/，同时严格从 three_view.png 正面持拳范围裁切放大生成等高对标卡
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import os
import subprocess
import sys
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "BingJiaShouTao.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "bing_jia_shou_tao_preview.png"
REVIEW_PARTS_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/bing_jia_shou_tao/parts")

PX = 16.0
RES = 64

# 手持居中对齐偏移（以掌心 y=6.00 处为持握中心）
HAND_CENTER_Y = 6.00
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - HAND_CENTER_Y, BLOCK_CENTRE_PX)

# 贴图象限规划 (64x64)
MAT_ZONE = {
    "leather_base": (0, 0, 32, 32),
    "leather_dark": (0, 0, 32, 32),
    "leather_strap": (32, 0, 64, 16),
    "metal_buckle": (32, 16, 64, 24),
    "metal_rivet": (32, 24, 64, 32),
    "iron_plate_rusted": (0, 32, 32, 64),
    "iron_knuckle_arch": (32, 32, 64, 48),
    "iron_thumb_plate": (32, 48, 64, 64),
}


def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0), faces=None):
    fx, tx = min(x0, x1), max(x0, x1)
    fy, ty = min(y0, y1), max(y0, y1)
    fz, tz = min(z0, z1), max(z0, z1)
    return (bone, mat, name, [fx, fy, fz], [tx, ty, tz], tuple(rot), faces)


def part_leather_base() -> list[tuple]:
    """1. 熟兽皮长筒手套基底 (leather_glove_base)。

    落实调度审要求：
    - 磨旧棕皮质感（#6a4a32 / #8a6448 斑驳），边缘缝线暗纹；
    - 手指部分清晰做出 4 根并排独立的包布皮手指（小指、无名指、中指、食指，留缝分明，位于指节铁甲片下方）。
    """
    cubes = []
    # 1. 小臂长筒袖口外翻加厚卷边 (y: 1.00..2.05, hw: 2.30, hz: 2.30)
    cubes.append(block("leather_base", "leather_dark", "sleeve_cuff_rim", -2.30, 2.30, 1.00, 2.05, -2.30, 2.30,
                       faces={"south": {"uv": [0.0, 0.0, 32.0, 8.0], "texture": 0}, "north": {"uv": [0.0, 0.0, 32.0, 8.0], "texture": 0}}))
    # 2. 小臂皮套筒身 (y: 1.95..5.10, hw: 2.15, hz: 2.15)
    cubes.append(block("leather_base", "leather_base", "sleeve_forearm_core", -2.15, 2.15, 1.95, 5.10, -2.15, 2.15,
                       faces={"south": {"uv": [2.0, 4.0, 30.0, 20.0], "texture": 0}, "north": {"uv": [2.0, 4.0, 30.0, 20.0], "texture": 0}}))
    # 3. 手腕皮套收束过渡段 (y: 5.00..6.40, hw: 2.08, hz: 2.08)
    cubes.append(block("leather_base", "leather_base", "sleeve_wrist_neck", -2.08, 2.08, 5.00, 6.40, -2.08, 2.08,
                       faces={"south": {"uv": [4.0, 16.0, 28.0, 24.0], "texture": 0}, "north": {"uv": [4.0, 16.0, 28.0, 24.0], "texture": 0}}))
    # 4. 掌身主皮套 (y: 6.30..9.35, hw: 2.22, hz: 2.18)
    cubes.append(block("leather_base", "leather_base", "sleeve_palm_main", -2.22, 2.22, 6.30, 9.35, -2.18, 2.18,
                       faces={"south": {"uv": [2.0, 8.0, 30.0, 28.0], "texture": 0}, "north": {"uv": [2.0, 8.0, 30.0, 28.0], "texture": 0}}))

    # 5. 4 根并排独立的熟皮手指（小指、无名指、中指、食指，清晰并排留缝）
    cubes.append(block("leather_base", "leather_base", "sleeve_finger_pinky",  -2.15, -1.18, 9.30, 10.30, -2.02, 2.05,
                       faces={"south": {"uv": [0.0, 20.0, 8.0, 32.0], "texture": 0}, "north": {"uv": [0.0, 20.0, 8.0, 32.0], "texture": 0}}))
    cubes.append(block("leather_base", "leather_base", "sleeve_finger_ring",   -1.10, -0.12, 9.30, 10.45, -2.02, 2.05,
                       faces={"south": {"uv": [8.0, 20.0, 16.0, 32.0], "texture": 0}, "north": {"uv": [8.0, 20.0, 16.0, 32.0], "texture": 0}}))
    cubes.append(block("leather_base", "leather_base", "sleeve_finger_middle", -0.04,  0.94, 9.30, 10.58, -2.02, 2.05,
                       faces={"south": {"uv": [16.0, 20.0, 24.0, 32.0], "texture": 0}, "north": {"uv": [16.0, 20.0, 24.0, 32.0], "texture": 0}}))
    cubes.append(block("leather_base", "leather_base", "sleeve_finger_index",   1.02,  2.00, 9.30, 10.40, -2.02, 2.05,
                       faces={"south": {"uv": [24.0, 20.0, 32.0, 32.0], "texture": 0}, "north": {"uv": [24.0, 20.0, 32.0, 32.0], "texture": 0}}))

    return cubes


def part_wrist_straps() -> list[tuple]:
    """2. 腕部双道皮带扣 (wrist_straps)。

    落实调度审要求：
    - 两条紧束皮带；
    - 外侧配置方形银灰带扣 #9a9aa0（2×2 镂空方框，中间空），带扣针与固定铆钉。
    """
    cubes = []
    # ── 下皮带 (y: 3.20..3.95, 外套在小臂上, hw: 2.26, hz: 2.26) ──
    cubes.append(block("wrist_straps", "leather_strap", "strap_lower_band", -2.26, 2.26, 3.20, 3.95, -2.26, 2.26,
                       faces={"south": {"uv": [32.0, 2.0, 64.0, 8.0], "texture": 0}, "north": {"uv": [32.0, 2.0, 64.0, 8.0], "texture": 0}}))
    # 下皮带方形镂空银灰带扣 (外框 1.6x0.9px, 亮面 #9a9aa0)
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_l_top",   0.35, 1.95, 3.82, 4.02, 2.28, 2.50,
                       faces={"south": {"uv": [32.0, 14.0, 48.0, 16.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_l_bot",   0.35, 1.95, 3.12, 3.32, 2.28, 2.50,
                       faces={"south": {"uv": [32.0, 22.0, 48.0, 24.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_l_left",  0.35, 0.65, 3.32, 3.82, 2.28, 2.50,
                       faces={"south": {"uv": [32.0, 16.0, 35.0, 22.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_l_right", 1.65, 1.95, 3.32, 3.82, 2.28, 2.50,
                       faces={"south": {"uv": [45.0, 16.0, 48.0, 22.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_l_pin",   1.05, 1.25, 3.22, 3.92, 2.30, 2.54,
                       faces={"south": {"uv": [39.0, 15.0, 41.0, 23.0], "texture": 0}}))
    # 下皮带固定铆钉
    cubes.append(block("wrist_straps", "metal_rivet", "strap_lower_rivet", -0.60, -0.20, 3.42, 3.78, 2.24, 2.42,
                       faces={"south": {"uv": [34.0, 26.0, 38.0, 30.0], "texture": 0}}))

    # ── 上皮带 (y: 4.40..5.15, hw: 2.22, hz: 2.22) ──
    cubes.append(block("wrist_straps", "leather_strap", "strap_upper_band", -2.22, 2.22, 4.40, 5.15, -2.22, 2.22,
                       faces={"south": {"uv": [32.0, 6.0, 64.0, 12.0], "texture": 0}, "north": {"uv": [32.0, 6.0, 64.0, 12.0], "texture": 0}}))
    # 上皮带方形镂空银灰带扣
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_u_top",   0.35, 1.95, 5.02, 5.22, 2.24, 2.46,
                       faces={"south": {"uv": [48.0, 14.0, 64.0, 16.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_u_bot",   0.35, 1.95, 4.32, 4.52, 2.24, 2.46,
                       faces={"south": {"uv": [48.0, 22.0, 64.0, 24.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_u_left",  0.35, 0.65, 4.52, 5.02, 2.24, 2.46,
                       faces={"south": {"uv": [48.0, 16.0, 51.0, 22.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_u_right", 1.65, 1.95, 4.52, 5.02, 2.24, 2.46,
                       faces={"south": {"uv": [61.0, 16.0, 64.0, 22.0], "texture": 0}}))
    cubes.append(block("wrist_straps", "metal_buckle", "buckle_u_pin",   1.05, 1.25, 4.42, 5.12, 2.26, 2.50,
                       faces={"south": {"uv": [55.0, 15.0, 57.0, 23.0], "texture": 0}}))
    # 上皮带固定铆钉
    cubes.append(block("wrist_straps", "metal_rivet", "strap_upper_rivet", -0.60, -0.20, 4.62, 4.98, 2.20, 2.38,
                       faces={"south": {"uv": [42.0, 26.0, 46.0, 30.0], "texture": 0}}))

    return cubes


def part_dorsal_plate() -> list[tuple]:
    """3. 手背外嵌防冻铁甲片 (dorsal_plate)。

    落实调度审要求：
    - 锈蚀铁板：灰 #6a6a70 / 亮 #8a8a90 斑驳交错，夹杂暗锈坑点 #4a4440、少量锈褐 #6a4a32，四周边缘磨亮；
    - 保留四角凸起金属加固铆钉。
    """
    cubes = []
    # 1. 垫衬加厚皮垫托 (x: -2.05..2.05, y: 6.40..8.60, z: 2.16..2.32)
    cubes.append(block("dorsal_plate", "leather_dark", "plate_dorsal_underlay", -2.05, 2.05, 6.40, 8.60, 2.16, 2.32,
                       faces={"south": {"uv": [0.0, 0.0, 32.0, 32.0], "texture": 0}}))
    # 2. 手背主防冻铁甲片 (深灰锻打铁板, x: -1.90..1.90, y: 6.55..8.55, z: 2.30..2.54)
    cubes.append(block("dorsal_plate", "iron_plate_rusted", "plate_dorsal_main", -1.90, 1.90, 6.55, 8.55, 2.30, 2.54,
                       faces={"south": {"uv": [2.0, 34.0, 30.0, 62.0], "texture": 0}}))
    # 3. 四角固定圆拱形铆钉 (rivets)
    cubes.append(block("dorsal_plate", "metal_rivet", "plate_rivet_bl", -1.65, -1.35, 6.70, 7.00, 2.52, 2.68,
                       faces={"south": {"uv": [34.0, 26.0, 38.0, 30.0], "texture": 0}}))
    cubes.append(block("dorsal_plate", "metal_rivet", "plate_rivet_br",  1.35,  1.65, 6.70, 7.00, 2.52, 2.68,
                       faces={"south": {"uv": [42.0, 26.0, 46.0, 30.0], "texture": 0}}))
    cubes.append(block("dorsal_plate", "metal_rivet", "plate_rivet_tl", -1.65, -1.35, 8.10, 8.40, 2.52, 2.68,
                       faces={"south": {"uv": [50.0, 26.0, 54.0, 30.0], "texture": 0}}))
    cubes.append(block("dorsal_plate", "metal_rivet", "plate_rivet_tr",  1.35,  1.65, 8.10, 8.40, 2.52, 2.68,
                       faces={"south": {"uv": [58.0, 26.0, 62.0, 30.0], "texture": 0}}))

    return cubes


def part_knuckle_plates() -> list[tuple]:
    """4. 掌指关节与四指铁甲 (knuckle_plates)。

    落实调度审要求：
    - 彻底重构分成 4 块各盖在一根指头上（每块略拱起），不是一整条板上立 4 块；
    - 锈蚀斑驳铁质（灰 #6a6a70、亮 #8a8a90、暗锈坑 #4a4440、锈褐 #6a4a32、边缘磨亮）；
    - 每块指甲片配有加固铆钉。
    """
    cubes = []
    # (a) 小指拱形铁甲片 (覆盖小指关节与指面, x: -2.20..-1.12, y: 8.62..10.25, z: 2.17..2.58)
    cubes.append(block("knuckle_plates", "iron_knuckle_arch", "knuckle_arch_pinky", -2.20, -1.12, 8.62, 10.25, 2.17, 2.58,
                       faces={"south": {"uv": [32.0, 32.0, 40.0, 48.0], "texture": 0}}))
    cubes.append(block("knuckle_plates", "metal_rivet", "knuckle_rivet_pinky", -1.80, -1.52, 9.10, 9.40, 2.56, 2.70,
                       faces={"south": {"uv": [34.0, 26.0, 38.0, 30.0], "texture": 0}}))

    # (b) 无名指拱形铁甲片 (x: -1.14..-0.08, y: 8.64..10.42, z: 2.19..2.62)
    cubes.append(block("knuckle_plates", "iron_knuckle_arch", "knuckle_arch_ring", -1.14, -0.08, 8.64, 10.42, 2.19, 2.62,
                       faces={"south": {"uv": [40.0, 32.0, 48.0, 48.0], "texture": 0}}))
    cubes.append(block("knuckle_plates", "metal_rivet", "knuckle_rivet_ring", -0.75, -0.47, 9.20, 9.50, 2.60, 2.74,
                       faces={"south": {"uv": [42.0, 26.0, 46.0, 30.0], "texture": 0}}))

    # (c) 中指拱形铁甲片 (x: -0.06..1.00, y: 8.66..10.55, z: 2.21..2.65, 最长最拱起)
    cubes.append(block("knuckle_plates", "iron_knuckle_arch", "knuckle_arch_middle", -0.06, 1.00, 8.66, 10.55, 2.21, 2.65,
                       faces={"south": {"uv": [48.0, 32.0, 56.0, 48.0], "texture": 0}}))
    cubes.append(block("knuckle_plates", "metal_rivet", "knuckle_rivet_middle", 0.33, 0.61, 9.30, 9.60, 2.63, 2.77,
                       faces={"south": {"uv": [50.0, 26.0, 54.0, 30.0], "texture": 0}}))

    # (d) 食指拱形铁甲片 (x: 1.02..2.08, y: 8.64..10.38, z: 2.18..2.60)
    cubes.append(block("knuckle_plates", "iron_knuckle_arch", "knuckle_arch_index", 1.02, 2.08, 8.64, 10.38, 2.18, 2.60,
                       faces={"south": {"uv": [56.0, 32.0, 64.0, 48.0], "texture": 0}}))
    cubes.append(block("knuckle_plates", "metal_rivet", "knuckle_rivet_index", 1.41, 1.69, 9.20, 9.50, 2.58, 2.72,
                       faces={"south": {"uv": [58.0, 26.0, 62.0, 30.0], "texture": 0}}))

    return cubes


def part_thumb_guard() -> list[tuple]:
    """5. 拇指皮套与防护小甲片 (thumb_guard)。

    虎口侧向外展的独立大拇指熟皮指套（磨旧棕皮斑驳与缝线），拇指背处外嵌有一块微型保护铁甲片（锈蚀铁板）与固定铆钉。
    """
    cubes = []
    # 1. 拇指熟皮基座 (虎口侧向外展, x: 2.12..3.35, y: 5.40..6.95, z: -0.65..1.10, rot=(0.0, -15.0, -18.0))
    cubes.append(block("thumb_guard", "leather_base", "thumb_leather_base", 2.12, 3.35, 5.40, 6.95, -0.65, 1.10, rot=(0.0, -15.0, -18.0),
                       faces={"south": {"uv": [0.0, 0.0, 16.0, 16.0], "texture": 0}}))
    # 2. 拇指指节皮套 (x: 2.70..3.75, y: 6.35..7.70, z: -0.45..0.95, rot=(0.0, -20.0, -25.0))
    cubes.append(block("thumb_guard", "leather_dark", "thumb_knuckle_wrap", 2.70, 3.75, 6.35, 7.70, -0.45, 0.95, rot=(0.0, -20.0, -25.0),
                       faces={"south": {"uv": [16.0, 0.0, 32.0, 16.0], "texture": 0}}))
    # 3. 拇指防护小铁甲片 (外嵌在拇指背侧, x: 2.85..3.65, y: 6.55..7.55, z: 0.88..1.18, rot=(0.0, -20.0, -25.0))
    cubes.append(block("thumb_guard", "iron_thumb_plate", "thumb_iron_plate", 2.85, 3.65, 6.55, 7.55, 0.88, 1.18, rot=(0.0, -20.0, -25.0),
                       faces={"south": {"uv": [32.0, 48.0, 48.0, 64.0], "texture": 0}}))
    # 4. 拇指铁片加固铆钉
    cubes.append(block("thumb_guard", "metal_rivet", "thumb_plate_rivet", 3.15, 3.40, 6.90, 7.15, 1.16, 1.28, rot=(0.0, -20.0, -25.0),
                       faces={"south": {"uv": [34.0, 26.0, 38.0, 30.0], "texture": 0}}))

    return cubes


def all_cubes() -> list[tuple]:
    """汇总拳套所有 5 大部件立方体。"""
    cubes = []
    cubes.extend(part_leather_base())
    cubes.extend(part_wrist_straps())
    cubes.extend(part_dorsal_plate())
    cubes.extend(part_knuckle_plates())
    cubes.extend(part_thumb_guard())
    return cubes


def _assert_no_coplanar_faces(cubes: list[tuple]) -> None:
    """门禁：严格校验各立方体之间是否存在重叠或共面 Z-Fighting。"""
    n = len(cubes)
    tol = 1e-4
    for i in range(n):
        c1 = cubes[i]
        b1_min = c1[3]
        b1_max = c1[4]
        for j in range(i + 1, n):
            c2 = cubes[j]
            b2_min = c2[3]
            b2_max = c2[4]

            overlap_x = min(b1_max[0], b2_max[0]) - max(b1_min[0], b2_min[0])
            overlap_y = min(b1_max[1], b2_max[1]) - max(b1_min[1], b2_min[1])
            overlap_z = min(b1_max[2], b2_max[2]) - max(b1_min[2], b2_min[2])

            if overlap_x > tol and overlap_y > tol and overlap_z > tol:
                for axis, name in [(0, "X"), (1, "Y"), (2, "Z")]:
                    if abs(b1_min[axis] - b2_min[axis]) < tol:
                        raise ValueError(f"共面冲突: {c1[2]} 与 {c2[2]} 在 -{name} 面共面 ({b1_min[axis]:.4f})")
                    if abs(b1_max[axis] - b2_max[axis]) < tol:
                        raise ValueError(f"共面冲突: {c1[2]} 与 {c2[2]} 在 +{name} 面共面 ({b1_max[axis]:.4f})")


def make_texture_atlas() -> Image.Image:
    """生成 64x64 兵甲手套 Texture Atlas，严格精绘磨旧棕皮缝线、锈蚀斑驳铁板磨痕划痕与方形银灰带扣。"""
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.default_rng(20261004)

    # ── 1. Q1 (0..32, 0..32): 磨旧棕皮底套 (leather_base / leather_dark) ──
    # 磨旧棕皮 #6a4a32 [106, 74, 50] / #8a6448 [138, 100, 72] 斑驳，边缘缝线暗纹
    q1 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            interp = ((x * 2 + y * 3) % 7) / 7.0
            r = int(106 * (1 - interp) + 138 * interp + noise)
            g = int(74 * (1 - interp) + 100 * interp + noise)
            b = int(50 * (1 - interp) + 72 * interp + noise)
            if (x * 4 - y * 3) % 9 in (0, 1):
                r += 14; g += 12; b += 10
            elif (x + y * 3) % 11 in (0, 1):
                r -= 16; g -= 14; b -= 12
            # 缝线暗槽与浅色针脚
            if x in (4, 12, 20, 28) or y in (4, 16, 28):
                if (x + y) % 3 == 0:
                    r, g, b = 175, 145, 115
                else:
                    r, g, b = 48, 30, 18
            q1[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]
    img.paste(Image.fromarray(q1, "RGBA"), (0, 0))

    # ── 2. Q2 (32..64, 0..32): 腕部双道皮带扣与加固皮带 (带方形银灰带扣 #9a9aa0 与加固铆钉) ──
    q2 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            if y < 14:
                # 紧束皮带带体 (#6a4a32 / #4a3422)
                r, g, b = 100 + noise, 70 + noise, 46 + noise
                if y in (0, 13):
                    r, g, b = 45, 30, 18
                elif y in (1, 12):
                    r, g, b = 135, 100, 70
            elif y < 24:
                # 方形银灰带扣 #9a9aa0 [154, 154, 160] (中间镂空，扣框带银灰光泽)
                fx = x % 16
                fy = y - 14
                if fx in (0, 1, 14, 15) or fy in (0, 9):
                    r, g, b = 110, 110, 116
                elif fx in (2, 3, 12, 13) or fy in (1, 8):
                    r, g, b = 190, 192, 202
                elif fx in (7, 8):
                    r, g, b = 215, 220, 228
                else:
                    r, g, b = 44, 32, 22
            else:
                # 铆钉贴图区 (圆拱形高光金属铆钉)
                cx = x % 8
                cy = (y - 24) % 8
                dist = np.hypot(cx - 3.5, cy - 3.5)
                if dist < 1.4:
                    r, g, b = 240, 245, 252
                elif dist < 2.6:
                    r, g, b = 165, 172, 182
                elif dist < 3.6:
                    r, g, b = 48, 50, 56
                else:
                    r, g, b = 106, 74, 50
            q2[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]
    img.paste(Image.fromarray(q2, "RGBA"), (32, 0))

    # ── 3. Q3 (0..32, 32..64): 手背主铁甲片 (锈蚀铁板：灰 #6a6a70 / 亮 #8a8a90 斑驳 + 暗锈坑点 #4a4440 + 少量锈褐 #6a4a32 + 边缘磨亮) ──
    q3 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-5, 6)
            m = ((x * 3 + y * 2) % 5) / 5.0
            r = int(106 * (1 - m) + 138 * m + noise)
            g = int(106 * (1 - m) + 138 * m + noise)
            b = int(112 * (1 - m) + 144 * m + noise)
            if (x * 4 - y * 3) % 11 in (0, 1):
                r = 106 + noise; g = 74 + noise; b = 50 + noise
            if (x * 5 + y * 7) % 13 == 0:
                r = 74; g = 68; b = 64
            if x in (0, 31) or y in (0, 31):
                r, g, b = 188, 196, 210
            elif x in (1, 30) or y in (1, 30):
                r, g, b = 150, 158, 168
            q3[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]

    # 精绘数道金属磨痕划痕
    scratches = [
        (5, 9), (6, 10), (7, 11), (8, 12), (9, 13), (10, 14), (11, 15), (12, 16), (13, 17), (14, 18), (15, 19),
        (19, 8), (20, 8), (21, 7), (22, 7), (23, 7), (24, 8), (25, 8),
        (9, 25), (10, 25), (11, 26), (12, 26), (13, 27), (14, 27), (15, 28)
    ]
    for sx, sy in scratches:
        q3[sy, sx, :3] = [200, 210, 225]
        if sy + 1 < 32:
            q3[sy + 1, sx, :3] = [48, 50, 56]
    img.paste(Image.fromarray(q3, "RGBA"), (0, 32))

    # ── 4. Q4 (32..64, 32..64): 4块独立指节铁甲片与拇指小铁甲 (锈蚀斑驳铁板 + 亮边) ──
    q4 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            m = ((x * 2 + y * 3) % 4) / 4.0
            r = int(106 * (1 - m) + 138 * m + noise)
            g = int(106 * (1 - m) + 138 * m + noise)
            b = int(112 * (1 - m) + 144 * m + noise)
            if (x * 3 + y * 5) % 9 == 0:
                r, g, b = 74, 68, 64
            elif (x * 7 - y * 4) % 10 == 0:
                r, g, b = 106, 74, 50
            fx = x % 8
            if fx in (0, 7) or y in (0, 15, 16, 31):
                r, g, b = 184, 192, 205
            elif fx in (1, 6):
                r, g, b = 60, 56, 54
            q4[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]
    img.paste(Image.fromarray(q4, "RGBA"), (32, 32))

    return img


def build_bbmodel(cubes: list[tuple], tex_img: Image.Image, model_name: str = "BingJiaShouTao") -> dict:
    """组装 Blockbench 格式的 .bbmodel JSON 数据。"""
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    tex_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "bing_jia_shou_tao",
        "folder": "item",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": tex_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: dict[str, list[str]] = {}

    for idx, (bone_name, mat_key, cube_name, f_pos, t_pos, rot, custom_faces) in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        groups_map.setdefault(bone_name, []).append(elem_uuid)

        from_coord = [
            round(f_pos[0] + EMIT_OFFSET[0], 4),
            round(f_pos[1] + EMIT_OFFSET[1], 4),
            round(f_pos[2] + EMIT_OFFSET[2], 4),
        ]
        to_coord = [
            round(t_pos[0] + EMIT_OFFSET[0], 4),
            round(t_pos[1] + EMIT_OFFSET[1], 4),
            round(t_pos[2] + EMIT_OFFSET[2], 4),
        ]

        if custom_faces is not None:
            faces = {}
            for face_name in ("north", "east", "south", "west", "up", "down"):
                if face_name in custom_faces:
                    faces[face_name] = custom_faces[face_name]
                else:
                    zx0, zy0, zx1, zy1 = MAT_ZONE[mat_key]
                    faces[face_name] = {"uv": [float(zx0), float(zy0), float(zx0 + 4), float(zy0 + 4)], "texture": 0}
        else:
            zx0, zy0, zx1, zy1 = MAT_ZONE[mat_key]
            span_x = max(1, zx1 - zx0 - 4)
            span_y = max(1, zy1 - zy0 - 4)
            uv_u0 = zx0 + (idx * 2) % span_x
            uv_v0 = zy0 + (idx * 3) % span_y
            uv_box = [float(uv_u0), float(uv_v0), float(uv_u0 + 4), float(uv_v0 + 4)]
            faces = {
                face_name: {
                    "uv": uv_box,
                    "texture": 0,
                }
                for face_name in ("north", "east", "south", "west", "up", "down")
            }

        element = {
            "name": cube_name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": from_coord,
            "to": to_coord,
            "autouv": 0,
            "color": 0,
            "origin": [BLOCK_CENTRE_PX, BLOCK_CENTRE_PX, BLOCK_CENTRE_PX],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        if any(abs(r) > 1e-4 for r in rot):
            origin = [
                (from_coord[0] + to_coord[0]) / 2.0,
                (from_coord[1] + to_coord[1]) / 2.0,
                (from_coord[2] + to_coord[2]) / 2.0,
            ]
            element["origin"] = origin
            element["rotation"] = list(rot)

        elements.append(element)

    out_groups = []
    for g_name in ["leather_base", "wrist_straps", "dorsal_plate", "knuckle_plates", "thumb_guard"]:
        if g_name in groups_map:
            out_groups.append({
                "name": g_name,
                "origin": [BLOCK_CENTRE_PX, BLOCK_CENTRE_PX, BLOCK_CENTRE_PX],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    bb_data = {
        "meta": {
            "format_version": "4.8",
            "model_format": "free",
            "box_uv": False,
        },
        "name": model_name,
        "model_identifier": "bing_jia_shou_tao",
        "visible_box": [1, 1, 0],
        "geometry_name": "bing_jia_shou_tao",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": out_groups,
        "textures": [texture_entry],
    }
    return bb_data


def generate(out_path: Path = BBMODEL_OUT) -> Path:
    """执行标准生成流程。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    PREVIEW_OUT.parent.mkdir(parents=True, exist_ok=True)

    tex = make_texture_atlas()
    bb_json = build_bbmodel(cubes, tex)

    out_path.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 已输出 Blockbench 模型: {out_path}")
    return out_path


def export_parts() -> None:
    """逐部件打磨导出并单件渲染，存入 model-review/img/bing_jia_shou_tao/parts/<name>.png，并生成与 three_view 对标卡。"""
    REVIEW_PARTS_DIR.mkdir(parents=True, exist_ok=True)
    tex = make_texture_atlas()

    parts = [
        ("leather_base",   part_leather_base(),   -35.0, 20.0),
        ("wrist_straps",   part_wrist_straps(),   -35.0, 20.0),
        ("dorsal_plate",   part_dorsal_plate(),   -35.0, 20.0),
        ("knuckle_plates", part_knuckle_plates(), -35.0, 20.0),
        ("thumb_guard",    part_thumb_guard(),    -35.0, 20.0),
    ]

    from bbmodel_maker.render.render_bbmodel import render
    print("开始逐部件单件渲染 (5 大部件，中灰背景 122, 122, 122)...")
    for part_name, part_cubes, yaw, pitch in parts:
        _assert_no_coplanar_faces(part_cubes)
        tmp_bb = REPO / "modelScript" / "out" / f"tmp_BingJiaShouTao_part_{part_name}.bbmodel"
        tmp_bb.parent.mkdir(parents=True, exist_ok=True)
        bb_json = build_bbmodel(part_cubes, tex, model_name=f"BingJiaShouTao_part_{part_name}")
        tmp_bb.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")

        target_png = REVIEW_PARTS_DIR / f"{part_name}.png"
        img, _ = render(str(tmp_bb), yaw=yaw, pitch=pitch, size=600, bg=(122, 122, 122))
        img.save(target_png)
        print(f"  ✓ 单部件渲染已落盘: {target_png}")
        tmp_bb.unlink(missing_ok=True)

    print("开始生成部件与 three_view.png 参考图的对标比对卡 (严格裁准正面持拳对应部位放大、中灰背景)...")
    ref_path = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/bing_jia_shou_tao/three_view.png")
    if not ref_path.exists():
        print(f"  [WARN] 未找到参考图: {ref_path}")
        return

    ref_img = Image.open(ref_path).convert("RGB")

    # 严格从 three_view.png 正面持拳各部位精确裁切放大：
    # leather_base: 小臂长筒袖套与腕部基底 (x: 25..195, y: 345..465)
    # wrist_straps: 手腕双道皮带环扣 (x: 30..195, y: 440..530)
    # dorsal_plate: 手背外嵌铁甲板与四角铆钉 (x: 35..185, y: 510..605)
    # knuckle_plates: 指关节与四指握拳护甲片 (x: 35..185, y: 590..695)
    # thumb_guard: 虎口外展大拇指套与护片 (x: 120..195, y: 520..630)
    ref_crops = {
        "leather_base":   ref_img.crop((25, 345, 195, 465)),
        "wrist_straps":   ref_img.crop((30, 440, 195, 530)),
        "dorsal_plate":   ref_img.crop((35, 510, 185, 605)),
        "knuckle_plates": ref_img.crop((35, 590, 185, 695)),
        "thumb_guard":    ref_img.crop((120, 520, 195, 630)),
    }

    for part_name, ref_crop in ref_crops.items():
        part_img = Image.open(REVIEW_PARTS_DIR / f"{part_name}.png")
        target_h = 600
        p_w = int(part_img.width * (target_h / part_img.height))
        p_scaled = part_img.resize((p_w, target_h), Image.Resampling.LANCZOS)
        r_w = int(ref_crop.width * (target_h / ref_crop.height))
        r_scaled = ref_crop.resize((r_w, target_h), Image.Resampling.LANCZOS)

        gap = 30
        card_w = p_w + r_w + gap + 40
        card_h = target_h + 80
        # 统一中灰背景 (122, 122, 122)
        card = Image.new("RGB", (card_w, card_h), (122, 122, 122))
        draw = ImageDraw.Draw(card)

        card.paste(p_scaled, (20, 60))
        card.paste(r_scaled, (20 + p_w + gap, 60))

        # 中缝分割线与参考图边框
        div_x = 20 + p_w + gap // 2
        draw.line([(div_x, 15), (div_x, card_h - 15)], fill=(70, 70, 70), width=2)
        draw.rectangle([20 + p_w + gap - 1, 59, 20 + p_w + gap + r_w, 60 + target_h], outline=(70, 70, 70), width=1)

        draw.text((25, 20), f"NOW (Single Part: {part_name})", fill=(20, 20, 20))
        draw.text((25 + p_w + gap, 20), f"REF (three_view.png: {part_name})", fill=(20, 20, 20))

        card_path = REVIEW_PARTS_DIR / f"check_{part_name}_vs_ref.png"
        card.save(card_path)
        print(f"  ✓ 对标卡已输出: {card_path}")

    print("✓ 全部 5 个单部件渲染与对标比对卡已输出完毕！")


def self_test() -> None:
    """门禁差分自证：注入共面冲突，验证 _assert_no_coplanar_faces 能准确拦截。"""
    print("运行 gen_bing_jia_shou_tao.py 差分自证...")
    clean_cubes = all_cubes()
    _assert_no_coplanar_faces(clean_cubes)
    print("  [OK] 正常立方体集无共面冲突")

    bad_cubes = list(clean_cubes)
    c0 = bad_cubes[0]
    bad_cube = ("leather_base", "leather_dark", "inject_coplanar", (c0[3][0], c0[3][1], c0[3][2]), (c0[4][0], c0[4][1], c0[4][2]), (0, 0, 0), None)
    bad_cubes.append(bad_cube)

    caught = False
    try:
        _assert_no_coplanar_faces(bad_cubes)
    except ValueError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e}")

    if not caught:
        raise AssertionError("门禁失效: 未能拦截注入的共面缺陷！")
    print("✓ gen_bing_jia_shou_tao.py 差分自证全部通过！")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成兵甲手套 BingJiaShouTao .bbmodel 模型")
    parser.add_argument("--export-parts", action="store_true", help="逐部件导出单件三视图至 model-review")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    generate()

    if args.export_parts:
        export_parts()


if __name__ == "__main__":
    main()
