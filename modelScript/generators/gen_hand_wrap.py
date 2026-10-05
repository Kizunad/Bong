#!/usr/bin/env python3
"""麻布拳套 / 缠手布 (hand_wrap / HandWrap) Blockbench .bbmodel 生成器。

严格依据 three_view.png 左臂正面视图与调度审部件第 2 轮意见打磨：
- 斜向层叠的布带：小臂到手背是一条条斜着绕、一圈压一圈的粗麻布带（每带宽约 1.5px），带与带之间有 1px 深色接缝 #4a3e30，
  布面米白 #c8b89a / 暗部 #8a7a64 斑驳起毛（贴图精绘斜纹 + 接缝）。
- 粗麻绳：截面加粗至约 1.3~1.5px，绕 2~3 圈螺旋，绳身用深浅棕交替的斜纹 #6a4a2e / #8a6a44 / #4a3420 画出立体「拧股」感，
  在手腕和小臂中段各有一圈明显的厚实凸起圈。
- 拳头指节：手掌前端握拳状，正面清晰呈现 4 根并排包布手指（每根独立包布指身，小指、无名指、中指、食指），
  每根指头末端露出一小块朝前下方的浅灰指尖 #9a9a9a。
- 起毛线头：布带边缘配有几处 1px 细长微翘的浅色短条起毛线头 (#ded2b8)。
- 结构顺序与 5 大部件划分：
    1. base_sleeve - 掌腕基础底衬套筒（贴身素麻布包裹基础层，覆盖手掌至手腕筒体，开出指槽）
    2. wrist_wrap  - 腕臂螺旋多层缠带（4 圈斜缠布带 + 外绕厚实粗麻绳螺旋圈 + 起毛线头）
    3. knuckle_pad - 指节打击区（4 根并排包布指头 + 露出浅灰指尖 #9a9a9a + 横向加固扎带 + 边缘线头）
    4. palm_cross  - 掌心交叉锁紧带与锁结（掌心 X 形交叉固定的紧箍绳结，带侧后绑扎绳结与垂落碎线头）
    5. thumb_wrap  - 拇指外展独立环缠带（虎口独立外展包缠拇指根部的半指短带，保护拇指关节，带微翘线头）

门禁与自检：
  - _assert_no_coplanar_faces 检查共面冲突 (Z-fighting)
  - --self-test 注入缺陷自证门禁有效性
  - --export-parts 逐个导出各部件并在中灰背景 (122, 122, 122) 下渲染存入 parts/，同时从 three_view.png 左臂正面持拳范围裁切放大生成等高对标卡
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
MODEL_SCRIPT_DIR = Path(__file__).resolve().parents[1]
if str(MODEL_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(MODEL_SCRIPT_DIR))

import bbmodel_maker.gates
_local_gates = str(MODEL_SCRIPT_DIR / "bbmodel_maker" / "gates")
if _local_gates not in bbmodel_maker.gates.__path__:
    bbmodel_maker.gates.__path__.append(_local_gates)

from bbmodel_maker.gates.coplanar import assert_no_coplanar_faces
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "HandWrap.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "hand_wrap_preview.png"
REVIEW_PARTS_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/hand_wrap/parts")

PX = 16.0
RES = 64

# ── 纵向与结构坐标规划（y: 1.00 -> 10.68）────────
WRIST_Y0 = 1.00
WRIST_Y1 = 5.50

PALM_Y0 = 4.80
PALM_Y1 = 8.20

KNUCKLE_Y0 = 7.90
KNUCKLE_Y1 = 10.68

# 手持居中对齐偏移（以掌心 y=6.00 处为持握中心）
HAND_CENTER_Y = 6.00
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - HAND_CENTER_Y, BLOCK_CENTRE_PX)

# 贴图象限规划 (64x64)
MAT_ZONE = {
    "base_sleeve": (0, 0, 32, 32),
    "wrist_wrap": (32, 0, 64, 20),
    "coarse_rope": (32, 20, 64, 32),
    "knuckle_pad": (0, 32, 32, 56),
    "fingertip_light": (0, 56, 16, 64),
    "palm_cross": (32, 32, 64, 46),
    "thumb_wrap": (32, 46, 64, 56),
    "fray_thread": (16, 56, 32, 64),
}


def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0)):
    fx, tx = min(x0, x1), max(x0, x1)
    fy, ty = min(y0, y1), max(y0, y1)
    fz, tz = min(z0, z1), max(z0, z1)
    return (bone, mat, name, [fx, fy, fz], [tx, ty, tz], tuple(rot))


def part_base_sleeve() -> list[tuple]:
    """1. 掌腕基础底衬套筒 (base_sleeve)。

    紧贴手掌和手腕的基础麻布套筒，形成内层包裹形态，留出四指孔与手腕管。
    """
    cubes = []
    # 1. 下腕部素麻底套筒（覆盖小臂下段与手腕, y: 1.20 -> 4.80, hw: 2.05, hz: 2.05）
    cubes.append(block("base_sleeve", "base_sleeve", "sleeve_wrist_core", -2.05, 2.05, 1.20, 4.80, -2.05, 2.05))

    # 2. 掌身基础底衬主层（覆盖手掌核心区, y: 4.80 -> 8.20, hw: 2.15, hz: 2.15）
    cubes.append(block("base_sleeve", "base_sleeve", "sleeve_palm_core", -2.15, 2.15, 4.80, 8.20, -2.15, 2.15))

    # 3. 四指根部包边套筒（y: 8.20 -> 9.60, hw: 2.22, hz: 2.08）
    cubes.append(block("base_sleeve", "base_sleeve", "sleeve_knuckle_base", -2.22, 2.22, 8.20, 9.60, -2.08, 2.08))

    # 4. 指孔露指边缘滚边（四指开孔收口小折边, y: 9.60 -> 10.05, hw: 2.10, hz: 1.95）
    cubes.append(block("base_sleeve", "base_sleeve", "sleeve_finger_rim", -2.10, 2.10, 9.60, 10.05, -1.95, 1.95))

    return cubes


def part_wrist_wrap() -> list[tuple]:
    """2. 腕臂螺旋多层缠带与粗麻绳 (wrist_wrap)。

    落实调度第 2 轮要求：
    - 麻绳改粗：截面约 1.3~1.5px，在小臂中段和手腕各形成一圈明显厚实凸起粗绳圈，带螺旋连接跨带；
    - 绳身贴图用深浅棕交替斜纹 #6a4a2e / #8a6a44 / #4a3420 画出立体「拧股」感；
    - 斜向层叠麻布带与起毛线头。
    """
    cubes = []
    # ── 1. 底层斜向交叠麻布带 4 圈 (y: 1.00..5.50) ──
    cubes.append(block("wrist_wrap", "wrist_wrap", "wrist_band_0", -2.26, 2.26, 1.00, 2.10, -2.26, 2.26))
    cubes.append(block("wrist_wrap", "wrist_wrap", "wrist_band_1", -2.32, 2.32, 1.95, 3.25, -2.32, 2.32, rot=(1.2, 0.0, -1.5)))
    cubes.append(block("wrist_wrap", "wrist_wrap", "wrist_band_2", -2.28, 2.28, 3.10, 4.40, -2.28, 2.28, rot=(-1.0, 0.0, 1.8)))
    cubes.append(block("wrist_wrap", "wrist_wrap", "wrist_band_3", -2.22, 2.22, 4.25, 5.50, -2.22, 2.22))

    # ── 2. 加粗麻绳 (截面约 1.3~1.5px，小臂中段与手腕各一圈明显厚实凸起圈) ──
    # (a) 小臂中段厚实粗绳圈 (y: 2.15..3.45, 截面高 1.30px, 前后左右外凸至 2.68px)
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_mid_f", -2.52, 2.52, 2.15, 3.45, 2.35, 2.68, rot=(0.0, 0.0, -8.0)))
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_mid_b", -2.52, 2.52, 2.35, 3.65, -2.68, -2.35, rot=(0.0, 0.0, 8.0)))
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_mid_l", -2.68, -2.35, 2.25, 3.55, -2.45, 2.45, rot=(6.0, 0.0, 0.0)))
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_mid_r", 2.35, 2.68, 2.25, 3.55, -2.45, 2.45, rot=(-6.0, 0.0, 0.0)))

    # (b) 手腕处厚实粗绳圈 (紧箍在手腕，y: 3.95..5.15, 截面高 1.20px, 外凸至 2.58px)
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_wrist_f", -2.48, 2.48, 3.95, 5.15, 2.28, 2.58, rot=(0.0, 0.0, -6.0)))
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_wrist_b", -2.48, 2.48, 4.10, 5.26, -2.58, -2.28, rot=(0.0, 0.0, 6.0)))
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_wrist_l", -2.58, -2.28, 4.02, 5.22, -2.38, 2.38, rot=(5.0, 0.0, 0.0)))
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_wrist_r", 2.28, 2.58, 4.02, 5.22, -2.38, 2.38, rot=(-5.0, 0.0, 0.0)))

    # (c) 斜向螺旋连接粗绳 (从小臂中段斜跨向上到手腕圈，厚 1.1px)
    cubes.append(block("wrist_wrap", "coarse_rope", "wrist_rope_spiral_trans", -2.46, 2.46, 3.10, 4.30, 2.32, 2.62, rot=(0.0, 0.0, -18.0)))

    # ── 3. 布带边缘起毛线头 ──
    cubes.append(block("wrist_wrap", "fray_thread", "wrist_fray_0", 2.28, 2.58, 2.45, 2.75, 1.80, 2.20, rot=(0.0, 15.0, 20.0)))
    cubes.append(block("wrist_wrap", "fray_thread", "wrist_fray_1", -2.62, -2.32, 4.60, 4.90, -1.80, -1.40, rot=(0.0, -12.0, -22.0)))

    return cubes


def part_knuckle_pad() -> list[tuple]:
    """3. 指节打击区 (knuckle_pad)。

    落实调度第 2 轮要求：
    - 改成 4 根并排包布指头（每根约 1.2~1.4px 宽，被布包着）；
    - 每根指头末端露出一点浅灰指尖 #9a9a9a，指尖朝前下方；
    - 横向加固勒扎带紧箍；
    - 边缘起毛线头。
    """
    cubes = []
    # ── 1. 拳锋基础底板 (y: 7.90..9.75, 宽 4.65, 前伸加厚) ──
    cubes.append(block("knuckle_pad", "knuckle_pad", "knuckle_pad_main", -2.32, 2.33, 7.90, 9.75, 1.10, 2.45))

    # ── 2. 指节横向勒扎加固紧箍带 (y: 8.05..8.85) ──
    cubes.append(block("knuckle_pad", "knuckle_pad", "knuckle_strap_band", -2.40, 2.40, 8.05, 8.85, 1.05, 2.52))

    # ── 3. 4 根并排包布手指 (从左到右小指、无名指、中指、食指，每根独立包布指身 + 末端露浅灰指尖) ──
    # (a) 小指 (pinky): x: -2.30..-1.22, 指尖 y: 9.75..10.38
    cubes.append(block("knuckle_pad", "knuckle_pad", "knuckle_pinky_body", -2.30, -1.22, 8.75, 10.15, 2.46, 2.80))
    cubes.append(block("knuckle_pad", "fingertip_light", "knuckle_pinky_tip",  -2.22, -1.30, 9.75, 10.38, 2.52, 2.92))

    # (b) 无名指 (ring): x: -1.14..-0.06, 指尖 y: 9.95..10.58
    cubes.append(block("knuckle_pad", "knuckle_pad", "knuckle_ring_body", -1.14, -0.06, 8.95, 10.35, 2.46, 2.86))
    cubes.append(block("knuckle_pad", "fingertip_light", "knuckle_ring_tip",  -1.06, -0.14, 9.95, 10.58, 2.52, 2.98))

    # (c) 中指 (middle): x: 0.06..1.14, 指尖 y: 10.05..10.68 (最长最凸)
    cubes.append(block("knuckle_pad", "knuckle_pad", "knuckle_middle_body", 0.06, 1.14, 9.10, 10.45, 2.46, 2.92))
    cubes.append(block("knuckle_pad", "fingertip_light", "knuckle_middle_tip",  0.14, 1.06, 10.05, 10.68, 2.52, 3.04))

    # (d) 食指 (index): x: 1.22..2.30, 指尖 y: 9.90..10.52
    cubes.append(block("knuckle_pad", "knuckle_pad", "knuckle_index_body", 1.22, 2.30, 8.90, 10.30, 2.46, 2.84))
    cubes.append(block("knuckle_pad", "fingertip_light", "knuckle_index_tip",  1.30, 2.22, 9.90, 10.52, 2.52, 2.96))

    # ── 4. 打击面边缘起毛线头 ──
    cubes.append(block("knuckle_pad", "fray_thread", "knuckle_fray_0", 2.20, 2.55, 9.30, 9.60, 2.20, 2.60, rot=(0.0, 0.0, 25.0)))
    cubes.append(block("knuckle_pad", "fray_thread", "knuckle_fray_1", -2.55, -2.20, 9.10, 9.40, 2.10, 2.50, rot=(0.0, 0.0, -25.0)))

    return cubes


def part_palm_cross() -> list[tuple]:
    """4. 掌心交叉锁紧带与锁结 (palm_cross)。

    掌心 X 形交叉固定的紧箍绳结，带侧后绑扎绳结结扣与下垂碎线头。
    """
    cubes = []
    # 1. 掌心 X 形交叉绑带 1 (z: -2.38 -> -2.22)
    cubes.append(block("palm_cross", "palm_cross", "palm_cross_band_a", -1.95, 1.95, 5.20, 7.80, -2.38, -2.22, rot=(0.0, 0.0, -24.0)))
    # 2. 掌心 X 形交叉绑带 2 (z: -2.52 -> -2.39)
    cubes.append(block("palm_cross", "palm_cross", "palm_cross_band_b", -1.95, 1.95, 5.20, 7.80, -2.52, -2.39, rot=(0.0, 0.0, 24.0)))
    # 3. 掌腕横向收口锁带
    cubes.append(block("palm_cross", "palm_cross", "palm_lock_strap", -2.20, 2.20, 4.75, 5.42, -2.42, -2.18))
    # 4. 侧后方立体绑扎结扣
    cubes.append(block("palm_cross", "palm_cross", "palm_tie_knot_core", 1.85, 2.65, 4.40, 5.30, -2.30, -1.40, rot=(5.0, -10.0, 15.0)))
    cubes.append(block("palm_cross", "palm_cross", "palm_tie_knot_lip", 2.05, 2.80, 4.60, 5.15, -2.26, -1.48, rot=(10.0, -5.0, 25.0)))
    # 5. 下垂碎线头
    cubes.append(block("palm_cross", "palm_cross", "palm_dangle_tail_1", 2.15, 2.45, 3.05, 4.48, -2.05, -1.75, rot=(0.0, 0.0, -12.0)))
    cubes.append(block("palm_cross", "palm_cross", "palm_dangle_tail_2", 1.95, 2.25, 3.35, 4.58, -1.85, -1.55, rot=(0.0, 0.0, 8.0)))

    return cubes


def part_thumb_wrap() -> list[tuple]:
    """5. 拇指外展独立环缠带 (thumb_wrap)。

    虎口侧面专门分叉独立包裹大拇指根部与第一节指骨的半指套环，露出拇指尖并带起毛线头。
    """
    cubes = []
    # 1. 虎口外展基座
    cubes.append(block("thumb_wrap", "thumb_wrap", "thumb_socket_base", 2.05, 3.25, 5.20, 6.80, -0.65, 1.15, rot=(0.0, -15.0, -18.0)))
    # 2. 拇指指节外包缠带
    cubes.append(block("thumb_wrap", "thumb_wrap", "thumb_knuckle_loop", 2.65, 3.65, 6.20, 7.60, -0.45, 0.95, rot=(0.0, -20.0, -25.0)))
    # 3. 拇指露指开孔收口小折边
    cubes.append(block("thumb_wrap", "thumb_wrap", "thumb_open_rim", 2.85, 3.60, 7.45, 7.85, -0.30, 0.80, rot=(0.0, -20.0, -25.0)))
    # 4. 拇指外侧起毛线头
    cubes.append(block("thumb_wrap", "fray_thread", "thumb_fray_0", 3.45, 3.75, 6.70, 7.00, 0.20, 0.60, rot=(0.0, -15.0, -35.0)))

    return cubes


def all_cubes() -> list[tuple]:
    """汇总拳套所有 5 大部件立方体。"""
    cubes = []
    cubes.extend(part_base_sleeve())
    cubes.extend(part_wrist_wrap())
    cubes.extend(part_knuckle_pad())
    cubes.extend(part_palm_cross())
    cubes.extend(part_thumb_wrap())
    return cubes


def make_texture_atlas() -> Image.Image:
    """生成 64x64 麻布拳套 Texture Atlas，严格落实调度要求的斜纹、接缝、粗麻绳拧股与浅灰指尖。"""
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.default_rng(20261003)

    # ── 1. 掌腕底衬套筒 (Q1: 0..32, 0..32) ──
    # 米白 #c8b89a [200, 184, 154] / 暗部 #8a7a64 [138, 122, 100] + 1px 深色接缝 #4a3e30 [74, 62, 48]
    sleeve_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-5, 6)
            phase = (x + y * 2) % 8
            if phase == 0:
                r, g, b = 74, 62, 48    # 1px 深色接缝 #4a3e30
            elif phase in (1, 2):
                r, g, b = 200, 184, 154 # 布带斜向高光米白 #c8b89a
            else:
                r, g, b = 138, 122, 100 # 暗部斑驳起毛 #8a7a64
            r = int(np.clip(r + noise, 0, 255))
            g = int(np.clip(g + noise, 0, 255))
            b = int(np.clip(b + noise, 0, 255))
            sleeve_arr[y, x] = [r, g, b, 255]
    img.paste(Image.fromarray(sleeve_arr, "RGBA"), (0, 0))

    # ── 2. 腕臂螺旋麻布带 (32..64, 0..20) ──
    wrist_arr = np.zeros((20, 32, 4), dtype=np.uint8)
    for y in range(20):
        for x in range(32):
            noise = rng.integers(-4, 5)
            phase = (x * 2 - y) % 7
            if phase == 0:
                r, g, b = 74, 62, 48     # 深色接缝 #4a3e30
            elif phase in (1, 2):
                r, g, b = 205, 188, 158   # 浅米白 #c8b89a
            else:
                r, g, b = 142, 126, 104   # 暗部 #8a7a64
            wrist_arr[y, x] = [
                int(np.clip(r + noise, 0, 255)),
                int(np.clip(g + noise, 0, 255)),
                int(np.clip(b + noise, 0, 255)),
                255,
            ]
    img.paste(Image.fromarray(wrist_arr, "RGBA"), (32, 0))

    # ── 3. 粗麻绳拧股斜纹 (32..64, 20..32) ──
    # 深浅棕交替斜纹 #6a4a2e [106, 74, 46] / #8a6a44 [138, 106, 68] / #4a3420 [74, 52, 32] 做出立体「拧股」感
    rope_arr = np.zeros((12, 32, 4), dtype=np.uint8)
    for y in range(12):
        for x in range(32):
            noise = rng.integers(-3, 4)
            twist = (x * 2 + y * 3) % 6
            if twist in (0, 1):
                r, g, b = 138, 106, 68   # 粗绳亮部高光 #8a6a44
            elif twist in (2, 3):
                r, g, b = 106, 74, 46    # 粗绳主体色 #6a4a2e
            else:
                r, g, b = 74, 52, 32     # 拧股接缝暗阴影 #4a3420
            rope_arr[y, x] = [
                int(np.clip(r + noise, 0, 255)),
                int(np.clip(g + noise, 0, 255)),
                int(np.clip(b + noise, 0, 255)),
                255,
            ]
    img.paste(Image.fromarray(rope_arr, "RGBA"), (32, 20))

    # ── 4. 指节打击区加厚布带 (0..32, 32..56) ──
    knuckle_arr = np.zeros((24, 32, 4), dtype=np.uint8)
    for y in range(24):
        for x in range(32):
            noise = rng.integers(-4, 5)
            if y % 6 == 0:
                r, g, b = 74, 62, 48     # 折痕接缝 #4a3e30
            elif y % 6 in (1, 2):
                r, g, b = 196, 180, 150  # 凸起受力层
            else:
                r, g, b = 145, 130, 106  # 暗部斑驳
            if x in (7, 15, 23):
                r = max(0, r - 32)
                g = max(0, g - 26)
                b = max(0, b - 20)
            knuckle_arr[y, x] = [
                int(np.clip(r + noise, 0, 255)),
                int(np.clip(g + noise, 0, 255)),
                int(np.clip(b + noise, 0, 255)),
                255,
            ]
    img.paste(Image.fromarray(knuckle_arr, "RGBA"), (0, 32))

    # ── 5. 浅灰指尖 #9a9a9a (0..16, 56..64) ──
    # 4 根手指末端露出的浅灰指尖
    tip_arr = np.zeros((8, 16, 4), dtype=np.uint8)
    for y in range(8):
        for x in range(16):
            noise = rng.integers(-4, 5)
            r = int(np.clip(154 + noise, 0, 255))
            g = int(np.clip(154 + noise, 0, 255))
            b = int(np.clip(154 + noise, 0, 255))
            tip_arr[y, x] = [r, g, b, 255]
    img.paste(Image.fromarray(tip_arr, "RGBA"), (0, 56))

    # ── 6. 掌心锁结、拇指带与起毛线头 (32..64, 32..64) ──
    misc_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            if y < 14:
                # 掌心锁结硬搓粗麻绳结 #4a3e30 / #6a5a44
                twist = (x * 2 + y * 3) % 5
                base = [88, 72, 54] if twist < 2 else [64, 52, 38]
            elif y < 24:
                # 拇指环缠带米白斜纹
                phase = (x + y) % 4
                base = [195, 180, 152] if phase < 2 else [142, 126, 104]
            else:
                # 起毛线头微白米白 #ded2b8 [222, 210, 184]
                base = [222, 210, 184]
            misc_arr[y, x] = [
                int(np.clip(base[0] + noise, 0, 255)),
                int(np.clip(base[1] + noise, 0, 255)),
                int(np.clip(base[2] + noise, 0, 255)),
                255,
            ]
    img.paste(Image.fromarray(misc_arr, "RGBA"), (32, 32))

    return img


def build_bbmodel(cubes: list[tuple], tex_img: Image.Image, model_name: str = "HandWrap") -> dict:
    """组装 Blockbench 格式的 .bbmodel JSON 数据。"""
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    tex_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "hand_wrap",
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

    for idx, (bone_name, mat_key, cube_name, f_pos, t_pos, rot) in enumerate(cubes):
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
    for g_name in ["base_sleeve", "wrist_wrap", "knuckle_pad", "palm_cross", "thumb_wrap"]:
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
        "model_identifier": "hand_wrap",
        "visible_box": [1, 1, 0],
        "geometry_name": "hand_wrap",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": out_groups,
        "textures": [texture_entry],
    }
    return bb_data


def generate() -> Path:
    """执行标准生成流程。"""
    cubes = all_cubes()
    assert_no_coplanar_faces(cubes)

    BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
    PREVIEW_OUT.parent.mkdir(parents=True, exist_ok=True)

    tex = make_texture_atlas()
    bb_json = build_bbmodel(cubes, tex)

    BBMODEL_OUT.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 已输出 Blockbench 模型: {BBMODEL_OUT}")
    return BBMODEL_OUT


def export_parts() -> None:
    """逐部件打磨导出并单件渲染，存入 model-review/img/hand_wrap/parts/<name>.png，并生成与 three_view 对标卡。"""
    REVIEW_PARTS_DIR.mkdir(parents=True, exist_ok=True)
    tex = make_texture_atlas()

    parts = [
        ("base_sleeve", part_base_sleeve(), -35.0, 20.0),
        ("wrist_wrap",  part_wrist_wrap(),  -35.0, 20.0),
        ("knuckle_pad", part_knuckle_pad(), -35.0, 20.0),
        ("palm_cross",  part_palm_cross(),  145.0, 15.0),
        ("thumb_wrap",  part_thumb_wrap(),  -35.0, 20.0),
    ]

    from bbmodel_maker.render.render_bbmodel import render
    print("开始逐部件单件渲染 (5 大部件，中灰背景 122, 122, 122)...")
    for part_name, part_cubes, yaw, pitch in parts:
        assert_no_coplanar_faces(part_cubes)
        tmp_bb = REPO / "modelScript" / "out" / f"tmp_HandWrap_part_{part_name}.bbmodel"
        tmp_bb.parent.mkdir(parents=True, exist_ok=True)
        bb_json = build_bbmodel(part_cubes, tex, model_name=f"HandWrap_part_{part_name}")
        tmp_bb.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")

        target_png = REVIEW_PARTS_DIR / f"{part_name}.png"
        img, _ = render(str(tmp_bb), yaw=yaw, pitch=pitch, size=600, bg=(122, 122, 122))
        img.save(target_png)
        print(f"  ✓ 单部件渲染已落盘: {target_png}")
        tmp_bb.unlink(missing_ok=True)

    print("开始生成部件与 three_view.png 参考图的对标比对卡 (严格裁准左臂正面持拳各部位放大、中灰背景)...")
    ref_path = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/hand_wrap/three_view.png")
    if not ref_path.exists():
        print(f"  [WARN] 未找到参考图: {ref_path}")
        return

    ref_img = Image.open(ref_path).convert("RGB")

    # 严格从 three_view.png 正面左臂缠手部位精确裁切 (调度指定范围: x 25..170, y 290..580):
    # wrist_wrap: 小臂至手腕螺旋缠绕麻带与粗绳 (x: 45..170, y: 290..415)
    # base_sleeve: 掌腕底衬套筒主层 (x: 35..170, y: 350..490)
    # knuckle_pad: 前端握拳 4 段指节打击部 (x: 25..155, y: 470..580)
    # palm_cross: 掌心内扣锁紧带部位 (x: 35..165, y: 420..530)
    # thumb_wrap: 虎口侧面独立包裹大拇指部位 (x: 25..110, y: 460..550)
    ref_crops = {
        "wrist_wrap":  ref_img.crop((45, 290, 170, 415)),
        "base_sleeve": ref_img.crop((35, 350, 170, 490)),
        "knuckle_pad": ref_img.crop((25, 470, 155, 580)),
        "palm_cross":  ref_img.crop((35, 420, 165, 530)),
        "thumb_wrap":  ref_img.crop((25, 460, 110, 550)),
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
        comp_path = REVIEW_PARTS_DIR / f"{part_name}_comparison.png"
        card.save(comp_path)
        print(f"  ✓ 对标卡已输出: {card_path}")

    print("✓ 全部 5 个单部件渲染与对标比对卡已输出完毕！")


def self_test() -> None:
    """门禁差分自证：注入共面冲突，验证 assert_no_coplanar_faces 能准确拦截。"""
    print("运行 gen_hand_wrap.py 差分自证...")
    clean_cubes = all_cubes()
    assert_no_coplanar_faces(clean_cubes)
    print("  [OK] 正常立方体集无共面冲突")

    bad_cubes = list(clean_cubes)
    c0 = bad_cubes[0]
    bad_cube = ("base_sleeve", "base_sleeve", "inject_coplanar", (c0[3][0], c0[3][1], c0[3][2]), (c0[4][0], c0[4][1], c0[4][2]), (0, 0, 0))
    bad_cubes.append(bad_cube)

    caught = False
    try:
        assert_no_coplanar_faces(bad_cubes)
    except (ValueError, AssertionError) as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e}")

    if not caught:
        raise AssertionError("门禁失效: 未能拦截注入的共面缺陷！")
    print("✓ gen_hand_wrap.py 差分自证全部通过！")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成麻布拳套 HandWrap .bbmodel 模型")
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
