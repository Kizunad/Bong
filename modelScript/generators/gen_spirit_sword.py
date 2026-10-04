#!/usr/bin/env python3
"""末法残土灵剑 (SpiritSword / spirit_sword) Blockbench .bbmodel 程序化生成器。

严格依据 three_view.png 与调度审部件第 2 轮意见打磨：
- dark_iron_blade: 暗灰直刃，刃口一侧配浅灰亮边 #8a8a90；中间导流槽为内敛无刺眼高光的暗铜褐凹线 #6a5a44（略带折线），
  两级平滑对称收分的锐利直剑尖与刺针（已通过）。
- diamond_guard: 菱角几何折面剑格。中心为一个浅灰褐石框 #8a8070 / #b0a898，框里嵌一个旋转 45° 的暗晶菱形 #2a2830，
  菱形中心一道浅色高光 #9a96a0；两侧剑格横臂同色浅灰褐、向外延伸且末端收窄，整体宽约剑身 3 倍（宽约 8.2px）。
- bound_hilt: 修长缠柄。深蓝灰主体（#30363c），横向间隔排布 3~4 道浅褐缠带 #8a7458，上下两端配加固金属套箍（已通过）。
- crystal_pommel: 灵晶嵌顶剑首。去掉两侧翅膀板，改成一颗圆润的暗紫灰晶球（约 3×3px，用中心块 + 各轴收缩块叠出圆润球体外形），
  亮面 #8a8098、暗面 #3a3446，直接嵌在柄尾。

门禁与自检：
  - _assert_no_coplanar_faces 检查共面冲突 (Z-fighting)
  - --self-test 注入缺陷自证门禁区分力
  - --parts 逐个导出各部件并在中灰背景 (122, 122, 122) 下渲染存入 parts/，同时严格按调度指定的像素范围从 three_view.png 裁切放大生成等高对标卡
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "SpiritSword.bbmodel"
PARTS_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/spirit_sword/parts")

RES = 64

# ── 调色板基准 (严格对齐调度审审定 hex 色值) ──
BLADE_DARK_IRON     = [58, 62, 66]     # 暗灰直刃基底
BLADE_EDGE_LIGHT    = [138, 138, 144]  # 浅灰刃口亮边 #8a8a90
BLADE_COPPER_GROOVE = [106, 90, 68]    # 暗铜褐凹线导流槽 #6a5a44

GUARD_STONE_BASE    = [138, 128, 112]  # 浅灰褐石框 #8a8070
GUARD_STONE_LIGHT   = [176, 168, 152]  # 浅灰褐石框亮面/横臂末端 #b0a898
GUARD_DIAMOND_DARK  = [42, 40, 48]     # 旋转 45° 暗晶菱形 #2a2830
GUARD_DIAMOND_HILITE= [154, 150, 160]  # 菱形中心浅色高光 #9a96a0

GRIP_DARK_BASE      = [48, 54, 60]     # 深蓝灰缠带主体
GRIP_LIGHT_TAN      = [138, 116, 88]   # 浅褐缠带 #8a7458
GRIP_METAL_FERRULE  = [72, 68, 64]     # 加固金属套箍

CRYSTAL_DARK_BASE   = [58, 52, 70]     # 圆润晶球暗面 #3a3446
CRYSTAL_LIGHT_FACET = [138, 128, 152]  # 圆润晶球亮面 #8a8098


def part_pommel() -> List[dict]:
    """1. crystal_pommel: 圆润暗紫灰晶球剑首。

    落实调度第 2 轮要求：
    - 去掉两侧翅膀板；
    - 改成一颗圆润的暗紫灰晶球（约 3×3px，用 3×3 中心块 + 四面各缩 1px 的块叠出圆润球体外形）；
    - 亮面 #8a8098、暗面 #3a3446，直接嵌在柄尾。
    """
    cubes = []
    # ── 1. 晶球中心核心主块 (约 2.4x2.4x2.4px, y: 2.70..5.10, x: 6.80..9.20, z: 6.80..9.20) ──
    cubes.append({
        "name": "pommel_ball_core",
        "from": [6.80, 2.70, 6.80],
        "to": [9.20, 5.10, 9.20],
        "group": "pommel",
        "material": "crystal_dark",
    })

    # ── 2. X 轴横向圆弧延展层 (两头各缩在 Y/Z，横向延展至 6.45..9.55，亮面 #8a8098) ──
    cubes.append({
        "name": "pommel_ball_x",
        "from": [6.45, 3.02, 7.18],
        "to": [9.55, 4.78, 8.82],
        "group": "pommel",
        "material": "crystal_light",
    })

    # ── 3. Z 轴前后圆弧延展层 (两头各缩在 X/Y，前后延展至 6.45..9.55，亮面 #8a8098) ──
    cubes.append({
        "name": "pommel_ball_z",
        "from": [7.12, 3.08, 6.45],
        "to": [8.88, 4.72, 9.55],
        "group": "pommel",
        "material": "crystal_light",
    })

    # ── 4. Y 轴纵向圆弧延伸层 (顶底收口，纵向延展至 2.35..5.45，直接嵌接柄尾) ──
    cubes.append({
        "name": "pommel_ball_y",
        "from": [7.22, 2.35, 7.24],
        "to": [8.78, 5.45, 8.76],
        "group": "pommel",
        "material": "crystal_light",
    })

    return cubes


def part_grip() -> List[dict]:
    """2. bound_hilt: 修长缠柄 (第 2 轮已通过)。

    深蓝灰主体（#30363c），横向间隔排布 3~4 道浅褐缠带 #8a7458，上下两端配加固金属套箍。
    """
    cubes = []
    # ── 握柄内芯木柱 (y: 5.50..10.50, 截面 1.16x1.16px, x: 7.42..8.58, z: 7.42..8.58) ──
    cubes.append({
        "name": "grip_core",
        "from": [7.42, 5.50, 7.42],
        "to": [8.58, 10.50, 8.58],
        "group": "grip",
        "material": "grip_dark",
    })
    # ── 下端加固套箍 (y: 5.48..5.92, x: 7.28..8.72, z: 7.28..8.72) ──
    cubes.append({
        "name": "grip_ferrule_b",
        "from": [7.28, 5.48, 7.28],
        "to": [8.72, 5.92, 8.72],
        "group": "grip",
        "material": "grip_metal",
    })
    # ── 上端加固套箍 (y: 10.08..10.52, x: 7.28..8.72, z: 7.28..8.72) ──
    cubes.append({
        "name": "grip_ferrule_t",
        "from": [7.28, 10.08, 7.28],
        "to": [8.72, 10.52, 8.72],
        "group": "grip",
        "material": "grip_metal",
    })

    # ── 5 段缠带：深蓝灰与浅褐缠带 #8a7458 交替 (呈现 3 道明显的浅褐横向缠带) ──
    wraps = [
        ("w0_light", 5.92, 6.76, 7.33, 8.75, 7.24, 8.66, "grip_light_tan"),
        ("w1_dark",  6.76, 7.60, 7.25, 8.67, 7.34, 8.76, "grip_dark"),
        ("w2_light", 7.60, 8.44, 7.33, 8.75, 7.24, 8.66, "grip_light_tan"),
        ("w3_dark",  8.44, 9.28, 7.25, 8.67, 7.34, 8.76, "grip_dark"),
        ("w4_light", 9.28, 10.08, 7.33, 8.75, 7.24, 8.66, "grip_light_tan"),
    ]
    for tag, y0, y1, x0, x1, z0, z1, mat in wraps:
        cubes.append({
            "name": f"grip_wrap_{tag}",
            "from": [x0, y0, z0],
            "to": [x1, y1, z1],
            "group": "grip",
            "material": mat,
        })
    return cubes


def part_guard() -> List[dict]:
    """3. diamond_guard: 浅褐方框里嵌旋转 45° 的暗晶菱形 + 两侧横臂。

    落实调度第 2 轮要求：
    - 十字横臂浅褐保持；
    - 中心改成一个浅褐方框 #8a8070 / #b0a898（比横臂略宽）；
    - 框里嵌一个旋转 45° 的暗晶菱形（#2a2830，中线浅色高光 #9a96a0）；
    - 两侧剑格横臂同色浅灰褐、向外延伸且末端收窄，整体宽约剑身 3 倍（宽约 8.2px）。
    """
    cubes = []
    # ── 1. 中央核心套筒 (x: 7.20..8.80, y: 10.52..12.50, z: 7.20..8.80) ──
    cubes.append({
        "name": "guard_center",
        "from": [7.20, 10.52, 7.20],
        "to": [8.80, 12.50, 8.80],
        "group": "guard",
        "material": "guard_stone_base",
    })

    # ── 2. 前后浅褐方形框体 (比横臂略宽，围绕中心，x: 6.80..9.20, y: 10.60..12.70) ──
    cubes.append({
        "name": "guard_frame_f",
        "from": [6.80, 10.60, 8.80],
        "to": [9.20, 12.70, 9.25],
        "group": "guard",
        "material": "guard_stone_base",
    })
    cubes.append({
        "name": "guard_frame_b",
        "from": [6.80, 10.60, 6.75],
        "to": [9.20, 12.70, 7.20],
        "group": "guard",
        "material": "guard_stone_base",
    })

    # ── 3. 框里嵌旋转 45° 的暗晶菱形 (#2a2830) ──
    # 前面旋转 45° 菱形 (中心位于 x=8.0, y=11.65，边长约 1.16px，旋转 45° 呈标准竖菱形)
    cubes.append({
        "name": "guard_diamond_rot_f",
        "from": [7.42, 11.07, 9.26],
        "to": [8.58, 12.23, 9.46],
        "group": "guard",
        "material": "guard_diamond_dark",
        "rotation": [0.0, 0.0, 45.0],
    })
    # 后面旋转 45° 菱形
    cubes.append({
        "name": "guard_diamond_rot_b",
        "from": [7.42, 11.07, 6.54],
        "to": [8.58, 12.23, 6.74],
        "group": "guard",
        "material": "guard_diamond_dark",
        "rotation": [0.0, 0.0, 45.0],
    })

    # 菱形中心一道浅色高光棱 (#9a96a0)
    cubes.append({
        "name": "guard_diamond_glow_f",
        "from": [7.82, 11.35, 9.47],
        "to": [8.18, 11.95, 9.57],
        "group": "guard",
        "material": "guard_diamond_highlight",
    })
    cubes.append({
        "name": "guard_diamond_glow_b",
        "from": [7.82, 11.35, 6.43],
        "to": [8.18, 11.95, 6.53],
        "group": "guard",
        "material": "guard_diamond_highlight",
    })

    # ── 4. 两侧浅灰褐剑格横臂 (#8a8070 / #b0a898, 末端收窄，整体宽约剑身 3 倍: x in 3.90..12.10) ──
    # 右横臂内段 (x: 8.80..10.50, y: 11.10..12.35, z: 7.35..8.65)
    cubes.append({
        "name": "guard_arm_r_inner",
        "from": [8.80, 11.10, 7.35],
        "to": [10.50, 12.35, 8.65],
        "group": "guard",
        "material": "guard_stone_base",
    })
    # 右横臂外段收窄 (x: 10.50..12.10, y: 11.25..12.20, z: 7.55..8.45)
    cubes.append({
        "name": "guard_arm_r_outer",
        "from": [10.50, 11.25, 7.55],
        "to": [12.10, 12.20, 8.45],
        "group": "guard",
        "material": "guard_stone_light",
    })

    # 左横臂内段 (x: 5.50..7.20, y: 11.10..12.35, z: 7.35..8.65)
    cubes.append({
        "name": "guard_arm_l_inner",
        "from": [5.50, 11.10, 7.35],
        "to": [7.20, 12.35, 8.65],
        "group": "guard",
        "material": "guard_stone_base",
    })
    # 左横臂外段收窄 (x: 3.90..5.50, y: 11.25..12.20, z: 7.55..8.45)
    cubes.append({
        "name": "guard_arm_l_outer",
        "from": [3.90, 11.25, 7.55],
        "to": [5.50, 12.20, 8.45],
        "group": "guard",
        "material": "guard_stone_light",
    })

    # ── 5. 上接浅灰褐石质吞口套筒 (Habaki, y: 12.45..12.80, x: 7.05..8.95, z: 7.05..8.95) ──
    cubes.append({
        "name": "guard_habaki",
        "from": [7.05, 12.45, 7.05],
        "to": [8.95, 12.80, 8.95],
        "group": "guard",
        "material": "guard_stone_base",
    })

    return cubes


def part_blade() -> List[dict]:
    """4. dark_iron_blade: 暗灰直刃 + 浅灰亮边 #8a8a90 + 暗铜褐凹线导流槽 #6a5a44 (第 2 轮已通过)。"""
    cubes = []

    # ── 1. 主剑身骨板 (y: 12.80..26.80, x: 6.90..9.10, z: 7.72..8.28, 厚 0.56px) ──
    cubes.append({
        "name": "blade_core",
        "from": [6.90, 12.80, 7.72],
        "to": [9.10, 26.80, 8.28],
        "group": "blade",
        "material": "blade_dark_iron",
        "faces": {
            "south": {"uv": [2.0, 6.0, 30.0, 32.0], "texture": 0},
            "north": {"uv": [2.0, 6.0, 30.0, 32.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 16.0], "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 16.0], "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],  "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],  "texture": 0},
        },
    })

    # ── 2. 开刃锋线：一侧配置浅灰亮边 #8a8a90 ──
    cubes.append({
        "name": "blade_edge_l",
        "from": [9.10, 12.80, 7.84],
        "to": [9.35, 26.80, 8.16],
        "group": "blade",
        "material": "blade_edge_light",
    })
    cubes.append({
        "name": "blade_edge_r",
        "from": [6.65, 12.80, 7.84],
        "to": [6.90, 26.80, 8.16],
        "group": "blade",
        "material": "blade_edge_light",
    })

    # ── 3. 剑尖两级平滑对称收分与刺针 ──
    cubes.append({
        "name": "blade_tip_1",
        "from": [7.10, 26.80, 7.75],
        "to": [8.90, 28.60, 8.25],
        "group": "blade",
        "material": "blade_dark_iron",
        "faces": {
            "south": {"uv": [4.0, 3.0, 28.0, 6.0], "texture": 0},
            "north": {"uv": [4.0, 3.0, 28.0, 6.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 8.0],  "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 8.0],  "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],  "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],  "texture": 0},
        },
    })
    cubes.append({
        "name": "blade_tip_2",
        "from": [7.50, 28.60, 7.80],
        "to": [8.50, 29.60, 8.20],
        "group": "blade",
        "material": "blade_dark_iron",
        "faces": {
            "south": {"uv": [8.0, 1.0, 24.0, 3.0], "texture": 0},
            "north": {"uv": [8.0, 1.0, 24.0, 3.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 4.0],  "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 4.0],  "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],  "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],  "texture": 0},
        },
    })
    cubes.append({
        "name": "blade_tip_point",
        "from": [7.88, 29.60, 7.85],
        "to": [8.12, 30.50, 8.15],
        "group": "blade",
        "material": "blade_edge_light",
    })

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体定义。"""
    cubes = []
    cubes.extend(part_pommel())
    cubes.extend(part_grip())
    cubes.extend(part_guard())
    cubes.extend(part_blade())
    return cubes


def _assert_no_coplanar_faces(cubes: List[dict]):
    """门禁：严格检测任意两立方体之间的共面接触 (Z-fighting)。"""
    n = len(cubes)
    tol = 1e-4
    for i in range(n):
        c1 = cubes[i]
        b1_min = c1["from"]
        b1_max = c1["to"]
        for j in range(i + 1, n):
            c2 = cubes[j]
            b2_min = c2["from"]
            b2_max = c2["to"]

            overlap_x = min(b1_max[0], b2_max[0]) - max(b1_min[0], b2_min[0])
            overlap_y = min(b1_max[1], b2_max[1]) - max(b1_min[1], b2_min[1])
            overlap_z = min(b1_max[2], b2_max[2]) - max(b1_min[2], b2_min[2])

            if overlap_x > tol and overlap_y > tol and overlap_z > tol:
                for axis, name in [(0, "X"), (1, "Y"), (2, "Z")]:
                    if abs(b1_min[axis] - b2_min[axis]) < tol:
                        other_axes = [a for a in range(3) if a != axis]
                        oa_span = [
                            min(b1_max[a], b2_max[a]) - max(b1_min[a], b2_min[a])
                            for a in other_axes
                        ]
                        raise AssertionError(
                            f"共面冲突: {c1['name']} 与 {c2['name']} 在 -{name} 面共面 "
                            f"({b1_min[axis]:.4f}), 重叠区域 ({oa_span[0]:.3f}x{oa_span[1]:.3f})"
                        )
                    if abs(b1_max[axis] - b2_max[axis]) < tol:
                        other_axes = [a for a in range(3) if a != axis]
                        oa_span = [
                            min(b1_max[a], b2_max[a]) - max(b1_min[a], b2_min[a])
                            for a in other_axes
                        ]
                        raise AssertionError(
                            f"共面冲突: {c1['name']} 与 {c2['name']} 在 +{name} 面共面 "
                            f"({b1_max[axis]:.4f}), 重叠区域 ({oa_span[0]:.3f}x{oa_span[1]:.3f})"
                        )


def make_texture_atlas() -> Image.Image:
    """生成 64x64 Texture Atlas，满足贴图绘制纹理细节与视觉纪律。"""
    atlas = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.default_rng(20261003)

    # ── 1. 剑身玄铁暗灰、浅灰亮边 #8a8a90 与暗铜褐凹线 #6a5a44 (Q1: 0..32, 0..32) ──
    blade_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            # 暗灰基底
            r = int(np.clip(BLADE_DARK_IRON[0] + noise, 0, 255))
            g = int(np.clip(BLADE_DARK_IRON[1] + noise, 0, 255))
            b = int(np.clip(BLADE_DARK_IRON[2] + noise, 0, 255))

            # 纵向矿物结晶微细斑驳纹理
            if (x + y * 2) % 5 == 0:
                r += 8; g += 8; b += 8
            elif (x * 3 - y) % 7 == 0:
                r -= 6; g -= 6; b -= 6

            # 浅灰开刃亮边 #8a8a90 [138, 138, 144]
            if x < 4 or x > 27:
                r = int(np.clip(BLADE_EDGE_LIGHT[0] + noise, 0, 255))
                g = int(np.clip(BLADE_EDGE_LIGHT[1] + noise, 0, 255))
                b = int(np.clip(BLADE_EDGE_LIGHT[2] + noise, 0, 255))

            blade_arr[y, x] = [r, g, b, 255]

    # 中间导流槽：暗铜褐凹线 #6a5a44 [106, 90, 68]（略带折线）
    copper_brown = np.array(BLADE_COPPER_GROOVE, dtype=np.uint8)
    copper_dark = np.array([86, 72, 54], dtype=np.uint8)
    for y in range(32):
        offset = 1 if (y // 3) % 2 == 1 else 0
        cx = 15 + offset
        blade_arr[y, cx, :3] = copper_brown
        blade_arr[y, cx + 1, :3] = copper_dark
        blade_arr[y, cx - 1, :3] = [50, 52, 56]
        blade_arr[y, cx + 2, :3] = [50, 52, 56]

    atlas.paste(Image.fromarray(blade_arr, "RGBA"), (0, 0))

    # ── 2. 剑格浅灰褐石框、旋转暗晶菱形与浅色高光 (Q2: 32..64, 0..32) ──
    guard_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            if y < 16:
                # 浅灰褐石框主色 #8a8070 [138, 128, 112]
                base = GUARD_STONE_BASE
                if (x + y) % 6 == 0:
                    base = GUARD_STONE_LIGHT  # #b0a898 石框亮面
            elif y < 26:
                # 暗晶菱形 #2a2830 [42, 40, 48]
                base = GUARD_DIAMOND_DARK
            else:
                # 菱形中心浅色高光 #9a96a0 [154, 150, 160]
                base = GUARD_DIAMOND_HILITE
            guard_arr[y, x] = [
                int(np.clip(base[0] + noise, 0, 255)),
                int(np.clip(base[1] + noise, 0, 255)),
                int(np.clip(base[2] + noise, 0, 255)),
                255,
            ]
    atlas.paste(Image.fromarray(guard_arr, "RGBA"), (32, 0))

    # ── 3. 握柄深蓝灰主体、3~4道浅褐缠带 #8a7458 与加固套箍 (Q3: 0..32, 32..64) ──
    grip_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            if y < 16:
                base = GRIP_DARK_BASE
                if (x + y * 2) % 6 in (0, 1):
                    base = [58, 64, 72]
            elif y < 28:
                base = GRIP_LIGHT_TAN
                if (x + y * 2) % 6 in (0, 1):
                    base = [154, 132, 102]
            else:
                base = GRIP_METAL_FERRULE
            grip_arr[y, x] = [
                int(np.clip(base[0] + noise, 0, 255)),
                int(np.clip(base[1] + noise, 0, 255)),
                int(np.clip(base[2] + noise, 0, 255)),
                255,
            ]
    atlas.paste(Image.fromarray(grip_arr, "RGBA"), (0, 32))

    # ── 4. 剑首圆润晶球：亮面 #8a8098、暗面 #3a3446 (Q4: 32..64, 32..64) ──
    pommel_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            if y < 16:
                # 晶球暗面 #3a3446 [58, 52, 70]
                base = CRYSTAL_DARK_BASE
                if (x + y) % 7 == 0:
                    base = [50, 44, 62]
            else:
                # 晶球亮面 #8a8098 [138, 128, 152]
                base = CRYSTAL_LIGHT_FACET
                if (x - y) % 5 == 0:
                    base = [150, 140, 164]
            pommel_arr[y, x] = [
                int(np.clip(base[0] + noise, 0, 255)),
                int(np.clip(base[1] + noise, 0, 255)),
                int(np.clip(base[2] + noise, 0, 255)),
                255,
            ]
    atlas.paste(Image.fromarray(pommel_arr, "RGBA"), (32, 32))

    return atlas


MATERIAL_UV_BOXES = {
    "blade_dark_iron": [0.0, 0.0, 32.0, 32.0],
    "blade_edge_light": [28.0, 0.0, 32.0, 16.0],
    "guard_stone_base": [32.0, 0.0, 64.0, 16.0],
    "guard_stone_light": [32.0, 0.0, 48.0, 16.0],
    "guard_diamond_dark": [32.0, 16.0, 48.0, 26.0],
    "guard_diamond_highlight": [48.0, 26.0, 64.0, 32.0],
    "grip_dark": [0.0, 32.0, 32.0, 48.0],
    "grip_light_tan": [0.0, 48.0, 32.0, 60.0],
    "grip_metal": [0.0, 60.0, 32.0, 64.0],
    "crystal_dark": [32.0, 32.0, 64.0, 48.0],
    "crystal_light": [32.0, 48.0, 64.0, 64.0],
}


def build_bbmodel_data(cubes: List[dict], tex_img: Image.Image) -> dict:
    """生成标准 Blockbench 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "spirit_sword",
        "folder": "item",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": texture_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: Dict[str, List[str]] = {}

    for cube in cubes:
        elem_uuid = str(uuid.uuid4())
        group_name = cube.get("group", "main")
        groups_map.setdefault(group_name, []).append(elem_uuid)

        f = cube["from"]
        t = cube["to"]
        mat = cube["material"]

        if "faces" in cube:
            faces = cube["faces"]
        else:
            uv_box = MATERIAL_UV_BOXES.get(mat, [0.0, 0.0, 4.0, 4.0])
            faces = {
                face_name: {
                    "uv": uv_box,
                    "texture": 0,
                }
                for face_name in ("north", "east", "south", "west", "up", "down")
            }

        elem = {
            "name": cube["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in f],
            "to": [round(float(v), 4) for v in t],
            "autouv": 0,
            "color": 0,
            "origin": [8.0, 8.0, 8.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }

        if "rotation" in cube:
            center = [
                (float(f[0]) + float(t[0])) / 2.0,
                (float(f[1]) + float(t[1])) / 2.0,
                (float(f[2]) + float(t[2])) / 2.0,
            ]
            elem["origin"] = center
            elem["rotation"] = [round(float(r), 2) for r in cube["rotation"]]

        elements.append(elem)

    outliner = []
    for g_name in ["pommel", "grip", "guard", "blade"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [8.0, 8.0, 8.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {
            "format_version": "4.8",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "SpiritSword",
        "model_identifier": "spirit_sword",
        "visible_box": [1, 1, 0],
        "geometry_name": "spirit_sword",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_bbmodel(out_path: Path, cubes_override: List[dict] | None = None) -> Path:
    """输出完整的 SpiritSword.bbmodel 文件。"""
    cubes = cubes_override if cubes_override is not None else all_cubes()
    _assert_no_coplanar_faces(cubes)
    tex_img = make_texture_atlas()
    doc = build_bbmodel_data(cubes, tex_img)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    rel = out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path
    print(f"✓ SpiritSword bbmodel 写入成功: {rel}")
    return out_path


def render_parts_individually():
    """将 4 大部件分别导出独立单件 bbmodel 并在中灰背景 (122, 122, 122) 下渲染存入 parts/，同时输出与 three_view 对标卡。"""
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    parts_map = {
        "crystal_pommel": (part_pommel(), -35.0, 20.0),
        "bound_hilt": (part_grip(), -35.0, 20.0),
        "diamond_guard": (part_guard(), -35.0, 20.0),
        "dark_iron_blade": (part_blade(), -35.0, 20.0),
    }

    from bbmodel_maker.render.render_bbmodel import render
    print("开始逐部件单件渲染 (4 大部件，中灰背景)...")
    for part_name, (cubes, yaw, pitch) in parts_map.items():
        tmp_model = Path(f"/tmp/SpiritSword_part_{part_name}.bbmodel")
        generate_bbmodel(tmp_model, cubes_override=cubes)
        out_png = PARTS_DIR / f"{part_name}.png"
        img, _ = render(str(tmp_model), yaw=yaw, pitch=pitch, size=600, bg=(122, 122, 122))
        img.save(out_png)
        tmp_model.unlink(missing_ok=True)
        print(f"  ✓ 单部件渲染完成: {out_png}")

    print("开始生成部件与 three_view.png 参考图的对标比对卡 (裁准调度指定部位放大、中灰背景)...")
    ref_path = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/spirit_sword/three_view.png")
    if not ref_path.exists():
        print(f"  [WARN] 未找到参考图: {ref_path}")
        return

    ref_img = Image.open(ref_path).convert("RGB")

    # 严格按调度审定的大致像素范围裁切放大：
    # 剑格: x 155~240, y 355~430
    # 剑首: x 205~240, y 300~335
    # 缠柄: 两者之间 (x 180~240, y 325~370)
    # 剑身: x 75~200, y 420~650
    ref_crops = {
        "crystal_pommel": ref_img.crop((205, 300, 240, 335)),
        "bound_hilt": ref_img.crop((180, 325, 240, 370)),
        "diamond_guard": ref_img.crop((155, 355, 240, 430)),
        "dark_iron_blade": ref_img.crop((75, 420, 200, 650)),
    }

    for part_name, ref_crop in ref_crops.items():
        part_img = Image.open(PARTS_DIR / f"{part_name}.png")
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

        card_path = PARTS_DIR / f"check_{part_name}_vs_ref.png"
        card.save(card_path)
        print(f"  ✓ 比对卡已输出: {card_path}")

    print("✓ 全部 4 个单部件渲染与比对卡已输出完毕！")


def self_test():
    """差分自证：验证正常模型共面校验通过，并能成功捕获注入的共面缺陷。"""
    print("运行 gen_spirit_sword.py 差分自证...")
    cubes = all_cubes()
    try:
        _assert_no_coplanar_faces(cubes)
        print("  [OK] 正常立方体集无共面冲突")
    except AssertionError as e:
        print(f"  [FAIL] 正常立方体集出现共面冲突: {e}")
        sys.exit(1)

    # 注入缺陷：故意引入完全共面的重叠面
    defect_cubes = list(cubes)
    defect_cubes.append({
        "name": "inject_coplanar_fail",
        "from": [7.20, 10.52, 7.20],
        "to": [8.80, 12.50, 8.80],  # 与 guard_center 完全重叠
        "group": "guard",
        "material": "guard_stone_base",
    })
    try:
        _assert_no_coplanar_faces(defect_cubes)
        print("  [FAIL] 未能捕获注入的共面缺陷！")
        sys.exit(1)
    except AssertionError as e:
        print(f"  [OK] 成功捕获注入缺陷: {e}")

    print("✓ gen_spirit_sword.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="生成末法残土灵剑 Blockbench 模型")
    parser.add_argument("--self-test", action="store_true", help="运行差分自证门禁检查")
    parser.add_argument("--parts", action="store_true", help="逐个导出并渲染 4 大部件")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    generate_bbmodel(BBMODEL_OUT)

    if args.parts:
        render_parts_individually()


if __name__ == "__main__":
    main()
