#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b02: meridian_bend (经脉内腔 90° 弯段) [Round 1 第 1 次返工版]

风格：A 有机型 (活体血肉、暖粉半透明筋管、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md

调度审第 1 次修改落实：
1. 坐标与圆弧中心线：
   - 方块内居中建模 [-8, 8] (对应 0-16 空间)；
   - 管从 -Z 面中心进 (入口截面中心 x=0, z=-8)，从 +X 面中心出 (出口截面中心 x=8, z=0)；
   - 中心线是以 (8, -8) 为圆心、半径 8 的四分之一圆弧；
2. 4 段旋转直管拼弧：
   - 每段沿切线方向放一节 8×6 截面的管 (暖粉 #d9a08c，上下 1px #e8bca8 亮边)；
   - 段与段之间转 22.5° (绕 Y 旋转原点 (8.0, 5.0, -8.0)，四段分别 11.25°、33.75°、56.25°、78.75°)；
   - 相邻两段在转角处重叠约 1px，严丝合缝不留缝；
   - 整件严格落在 16×16×16 内，绝对不越界。
3. 两端肉箍与小骨环端口 (同 b01)：
   - 两端各一圈包管肉箍 (#8a2a2a，宽 3px，比管身外凸 1px：输入端 z: -7.5..-4.5，输出端 x: 4.5..7.5)；
   - 骨环端口贴在两个肉箍顶面 (y: 9.0..9.5) 与底面 (y: 0.5..1.0) 各一个 (外 3×3、内孔 1×1、凸 0.5px、深色底 #5a1a1a)。
4. 弧线外侧正面加一条连续的筋丝 #c07868：
   - 随 4 段直管一同旋转，在外弧侧壁表面 (浮起 0.5px) 连成一条完整的连续弧形筋丝。
5. 管内沿长轴放一根细芯 #f6dcc4 (截面 2×2)，随圆弧贯穿直通。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "meridian_bend.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_bend")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b02_meridian_bend.png")

# 调色板 (严格对齐 2026-10-09 05:3x 调度审修订值)
PALETTE = {
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (孔内深色底)
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (两端肉箍主色)
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (贴面小骨环)
    "tendon_tube":      (217, 160, 140, 150), # #d9a08c 暖粉半透明筋管壁 (alpha 约 60%)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋管上下 1px 亮边
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 内光细芯
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 外弧连续筋丝 (浮起 0.5px)
}

MAT_UV = {
    "flesh_dark":       [0, 0, 16, 16],
    "flesh_main":       [16, 0, 32, 16],
    "bone_main":        [32, 0, 48, 16],
    "tendon_tube":      [48, 0, 64, 16],
    "qi_glow":          [0, 16, 16, 32],
    "tendon_fiber":     [16, 16, 32, 32],
    "tendon_highlight": [32, 16, 48, 32],
}

RES = 64
BEND_ORIGIN = [8.0, 5.0, -8.0]


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_collars() -> List[dict]:
    """两端四面包覆的肉箍：宽 3px，比管身外凸 1px，四周包覆 (同 b01)。
    输入端肉箍 (-Z): z: -7.5..-4.5，外轮廓 x: -5.0..4.45, y: 1.0..9.0
    输出端肉箍 (+X): x:  4.5.. 7.5，外轮廓 z: -4.45..5.0, y: 1.0..9.0
    """
    cubes = []

    # ── 1. -Z 输入端肉箍 (z: -7.5..-4.5) ──
    # 顶板 (y: 8.0..9.0, 跨度 x: -5.0..4.45, 避让孔道 x: -0.5..0.5, z: -6.5..-5.5)
    cubes.append({"name": "collar_in_top_l", "from": [-5.0, 8.0, -7.5], "to": [-0.5, 9.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_top_r", "from": [0.5, 8.0, -7.5], "to": [4.45, 9.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_top_mb", "from": [-0.5, 8.0, -7.5], "to": [0.5, 9.0, -6.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_top_mf", "from": [-0.5, 8.0, -5.5], "to": [0.5, 9.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})

    # 底板 (y: 1.0..2.0)
    cubes.append({"name": "collar_in_bot_l", "from": [-5.0, 1.0, -7.5], "to": [-0.5, 2.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_bot_r", "from": [0.5, 1.0, -7.5], "to": [4.45, 2.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_bot_mb", "from": [-0.5, 1.0, -7.5], "to": [0.5, 2.0, -6.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_bot_mf", "from": [-0.5, 1.0, -5.5], "to": [0.5, 2.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})

    # 左右侧板 (y: 2.0..8.0, 厚 1px)
    cubes.append({"name": "collar_in_side_l", "from": [-5.0, 2.0, -7.5], "to": [-4.0, 8.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_in_side_r", "from": [4.0, 2.0, -7.5], "to": [4.45, 8.0, -4.5], "group": "flesh_collars", "material": "flesh_main"})

    # ── 2. +X 输出端肉箍 (x: 4.5..7.5) ──
    # 顶板 (y: 8.0..9.0, 跨度 z: -4.45..5.0, 避让孔道 x: 5.5..6.5, z: -0.5..0.5)
    cubes.append({"name": "collar_out_top_b", "from": [4.5, 8.0, -4.45], "to": [7.5, 9.0, -0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_top_f", "from": [4.5, 8.0, 0.5], "to": [7.5, 9.0, 5.0], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_top_ml", "from": [4.5, 8.0, -0.5], "to": [5.5, 9.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_top_mr", "from": [6.5, 8.0, -0.5], "to": [7.5, 9.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})

    # 底板 (y: 1.0..2.0)
    cubes.append({"name": "collar_out_bot_b", "from": [4.5, 1.0, -4.45], "to": [7.5, 2.0, -0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_bot_f", "from": [4.5, 1.0, 0.5], "to": [7.5, 2.0, 5.0], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_bot_ml", "from": [4.5, 1.0, -0.5], "to": [5.5, 2.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_bot_mr", "from": [6.5, 1.0, -0.5], "to": [7.5, 2.0, 0.5], "group": "flesh_collars", "material": "flesh_main"})

    # 前后侧板 (y: 2.0..8.0, 厚 1px)
    cubes.append({"name": "collar_out_side_b", "from": [4.5, 2.0, -4.45], "to": [7.5, 8.0, -4.0], "group": "flesh_collars", "material": "flesh_main"})
    cubes.append({"name": "collar_out_side_f", "from": [4.5, 2.0, 4.0], "to": [7.5, 8.0, 5.0], "group": "flesh_collars", "material": "flesh_main"})

    return cubes


def part_02_ports() -> List[dict]:
    """贴面小骨环端口：外 3×3、内孔 1×1，凸出 0.5px，深色底 #5a1a1a (同 b01)。
    四个角各一个：输入端顶底各一、输出端顶底各一。
    """
    cubes = []

    # ── A. 输入端端口 (中心 x=0, z=-6.0) ──
    # 顶面端口 (y: 9.0..9.5)
    cubes.append({"name": "port_in_top_n", "from": [-1.5, 9.0, -7.5], "to": [1.5, 9.5, -6.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_s", "from": [-1.5, 9.0, -5.5], "to": [1.5, 9.5, -4.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_w", "from": [-1.5, 9.0, -6.5], "to": [-0.5, 9.5, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_e", "from": [0.5, 9.0, -6.5], "to": [1.5, 9.5, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_top_core", "from": [-0.48, 8.0, -6.48], "to": [0.48, 8.95, -5.52], "group": "ports", "material": "flesh_dark"})

    # 底面端口 (y: 0.5..1.0)
    cubes.append({"name": "port_in_bot_n", "from": [-1.5, 0.5, -7.5], "to": [1.5, 1.0, -6.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_s", "from": [-1.5, 0.5, -5.5], "to": [1.5, 1.0, -4.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_w", "from": [-1.5, 0.5, -6.5], "to": [-0.5, 1.0, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_e", "from": [0.5, 0.5, -6.5], "to": [1.5, 1.0, -5.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_in_bot_core", "from": [-0.48, 1.05, -6.48], "to": [0.48, 2.0, -5.52], "group": "ports", "material": "flesh_dark"})

    # ── B. 输出端端口 (中心 x=6.0, z=0) ──
    # 顶面端口 (y: 9.0..9.5)
    cubes.append({"name": "port_out_top_w", "from": [4.5, 9.0, -1.5], "to": [5.5, 9.5, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_e", "from": [6.5, 9.0, -1.5], "to": [7.5, 9.5, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_s", "from": [5.5, 9.0, -1.5], "to": [6.5, 9.5, -0.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_n", "from": [5.5, 9.0, 0.5], "to": [6.5, 9.5, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_top_core", "from": [5.52, 8.0, -0.48], "to": [6.48, 8.95, 0.48], "group": "ports", "material": "flesh_dark"})

    # 底面端口 (y: 0.5..1.0)
    cubes.append({"name": "port_out_bot_w", "from": [4.5, 0.5, -1.5], "to": [5.5, 1.0, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_e", "from": [6.5, 0.5, -1.5], "to": [7.5, 1.0, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_s", "from": [5.5, 0.5, -1.5], "to": [6.5, 1.0, -0.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_n", "from": [5.5, 0.5, 0.5], "to": [6.5, 1.0, 1.5], "group": "ports", "material": "bone_main"})
    cubes.append({"name": "port_out_bot_core", "from": [5.52, 1.05, -0.48], "to": [6.48, 2.0, 0.48], "group": "ports", "material": "flesh_dark"})

    return cubes


def part_03_interface_stubs() -> List[dict]:
    """两端 0.5px 标准接口露出段：严格居中为宽 8px x 高 6px、离地 2px，保证对接契约。"""
    cubes = []
    # 输入端 (z: -8.0..-7.5, x: -4..4, y: 2..8)
    cubes.append({"name": "in_stub_l", "from": [-4.0, 2.8, -8.0], "to": [-3.2, 7.2, -7.5], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "in_stub_r", "from": [3.2, 2.8, -8.0], "to": [4.0, 7.2, -7.5], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "in_stub_bot", "from": [-4.0, 2.0, -8.0], "to": [4.0, 2.8, -7.5], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "in_stub_top", "from": [-4.0, 7.2, -8.0], "to": [4.0, 8.0, -7.5], "group": "meridian_tube", "material": "tendon_highlight"})

    # 输出端 (x: 7.5..8.0, z: -4..4, y: 2..8)
    cubes.append({"name": "out_stub_b", "from": [7.5, 2.8, -4.0], "to": [8.0, 7.2, -3.2], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "out_stub_f", "from": [7.5, 2.8, 3.2], "to": [8.0, 7.2, 4.0], "group": "meridian_tube", "material": "tendon_tube"})
    cubes.append({"name": "out_stub_bot", "from": [7.5, 2.0, -4.0], "to": [8.0, 2.8, 4.0], "group": "meridian_tube", "material": "tendon_highlight"})
    cubes.append({"name": "out_stub_top", "from": [7.5, 7.2, -4.0], "to": [8.0, 8.0, 4.0], "group": "meridian_tube", "material": "tendon_highlight"})
    return cubes


def part_04_rotary_arc_tube() -> List[dict]:
    """用 4 段直管旋转 22.5° 拼合出半径 8px 的四分之一圆弧：
    段 1: 11.25°, 段 2: 33.75°, 段 3: 56.25°, 段 4: 78.75°。
    相邻两段在转角处重叠约 1px，严丝合缝不留缝。
    暖粉管壁 #d9a08c，上下各留 1px #e8bca8 亮边。
    """
    cubes = []
    angles = [11.25, 33.75, 56.25, 78.75]
    z_ranges = [(-8.6, -5.7), (-10.0, -6.0), (-10.0, -6.0), (-10.3, -7.4)]

    for i, (ang, (z0, z1)) in enumerate(zip(angles, z_ranges)):
        rot = [0, ang, 0]
        pfx = f"seg_{i+1}"
        # 1. 底部 1px 亮边 (y: 2.0..2.8)
        cubes.append({
            "name": f"tube_{pfx}_bot_rim",
            "from": [-4.0, 2.0, z0],
            "to":   [4.0, 2.8, z1],
            "origin": BEND_ORIGIN,
            "rotation": rot,
            "group": "meridian_tube",
            "material": "tendon_highlight",
        })
        # 2. 顶部 1px 亮边 (y: 7.2..8.0)
        cubes.append({
            "name": f"tube_{pfx}_top_rim",
            "from": [-4.0, 7.2, z0],
            "to":   [4.0, 8.0, z1],
            "origin": BEND_ORIGIN,
            "rotation": rot,
            "group": "meridian_tube",
            "material": "tendon_highlight",
        })
        # 3. 外弧侧壁 (x: -4.0..-3.2, y: 2.8..7.2, 暖粉色 #d9a08c)
        cubes.append({
            "name": f"tube_{pfx}_wall_out",
            "from": [-4.0, 2.8, z0],
            "to":   [-3.2, 7.2, z1],
            "origin": BEND_ORIGIN,
            "rotation": rot,
            "group": "meridian_tube",
            "material": "tendon_tube",
        })
        # 4. 内弧侧壁 (x: 3.2..4.0, y: 2.8..7.2, 暖粉色 #d9a08c)
        cubes.append({
            "name": f"tube_{pfx}_wall_in",
            "from": [3.2, 2.8, z0],
            "to":   [4.0, 7.2, z1],
            "origin": BEND_ORIGIN,
            "rotation": rot,
            "group": "meridian_tube",
            "material": "tendon_tube",
        })

    return cubes


def part_05_inner_qi_glow() -> List[dict]:
    """管内沿长轴细芯淡光 #f6dcc4 (截面 2x2, y: 4.0..6.0)：随 4 段直管一同旋转贯穿圆弧。"""
    cubes = []
    angles = [11.25, 33.75, 56.25, 78.75]
    z_ranges = [(-8.6, -5.7), (-10.0, -6.0), (-10.0, -6.0), (-10.3, -7.4)]

    for i, (ang, (z0, z1)) in enumerate(zip(angles, z_ranges)):
        rot = [0, ang, 0]
        pfx = f"seg_{i+1}"
        cubes.append({
            "name": f"qi_core_{pfx}",
            "from": [-1.0, 4.0, z0],
            "to":   [1.0, 6.0, z1],
            "origin": BEND_ORIGIN,
            "rotation": rot,
            "group": "inner_qi_glow",
            "material": "qi_glow",
        })
    return cubes


def part_06_diagonal_fibers() -> List[dict]:
    """弧线外侧正面连续筋丝 #c07868 (浮起 0.5px)：随管身一同旋转，连成完整对角弧形筋丝。"""
    cubes = []
    angles = [11.25, 33.75, 56.25, 78.75]
    z_ranges = [(-8.6, -5.7), (-10.0, -6.0), (-10.0, -6.0), (-10.3, -7.4)]

    for i, (ang, (z0, z1)) in enumerate(zip(angles, z_ranges)):
        rot = [0, ang, 0]
        pfx = f"seg_{i+1}"
        cubes.append({
            "name": f"tendon_fiber_{pfx}",
            "from": [-4.5, 4.6, z0],
            "to":   [-4.0, 5.4, z1],
            "origin": BEND_ORIGIN,
            "rotation": rot,
            "group": "diagonal_fibers",
            "material": "tendon_fiber",
        })
    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件的立方体。"""
    return (
        part_01_flesh_collars()
        + part_02_ports()
        + part_03_interface_stubs()
        + part_04_rotary_arc_tube()
        + part_05_inner_qi_glow()
        + part_06_diagonal_fibers()
    )


# =============================================================================
# 门禁与无共面面核验
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标同旋转且重叠的共面冲突。"""
    faces: Dict[Tuple[str, float, tuple], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        rot = tuple(c.get("rotation", [0, 0, 0]))
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (f[1], f[2], t[1], t[2])
            elif axis == 1:
                rect = (f[0], f[2], t[0], t[2])
            else:
                rect = (f[0], f[1], t[0], t[1])
            key = (side, round(val, 4), rot)
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val, rot), entries in faces.items():
        if len(entries) < 2:
            continue
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                u0 = max(r1[0], r2[0])
                u1 = min(r1[2], r2[2])
                v0 = max(r1[1], r2[1])
                v1 = min(r1[3], r2[3])
                if u1 - u0 > 0.001 and v1 - v0 > 0.001:
                    conflicts.append(
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}, rot={rot}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


# =============================================================================
# 贴图与 bbmodel 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图，严格使用 meridian_factory.md 修订后的暖粉配色。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if mat_name == "tendon_tube":
            for x in range(w):
                fib = rng.randint(-8, 8)
                tile[:, x, 0] = np.clip(tile[:, x, 0].astype(int) + fib, 0, 255)
                tile[:, x, 1] = np.clip(tile[:, x, 1].astype(int) + fib, 0, 255)
                tile[:, x, 2] = np.clip(tile[:, x, 2].astype(int) + fib, 0, 255)
                tile[:, x, 3] = np.clip(base_color[3] + rng.randint(-10, 10), 125, 175)
        elif mat_name == "tendon_highlight":
            for y in range(h):
                tile[y, :, :3] = np.clip(tile[y, :, :3].astype(int) + rng.randint(-6, 6), 0, 255)
        elif mat_name == "qi_glow":
            for y in range(h):
                for x in range(w):
                    r_dist = np.hypot(x - w / 2, y - h / 2) / (w / 2)
                    glow = int(14 * (1.0 - np.clip(r_dist, 0.0, 1.0)))
                    tile[y, x, :3] = np.clip(tile[y, x, :3].astype(int) + glow, 0, 255)
        elif mat_name == "tendon_fiber":
            for y in range(h):
                tile[y, :, :3] = np.clip(tile[y, :, :3].astype(int) + rng.randint(-6, 6), 0, 255)
        elif "flesh" in mat_name:
            noise = rng.randint(-12, 12, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: List[dict] | None = None) -> Path:
    """生成并导出 Blockbench bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = cubes_override if cubes_override is not None else all_cubes()
    _assert_no_coplanar_faces(cubes)

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for c in cubes:
        f = c["from"]
        t = c["to"]
        mat = c.get("material", "flesh_main")
        uv = MAT_UV.get(mat, [16, 0, 32, 16])
        faces = {}
        for side in ["north", "south", "east", "west", "up", "down"]:
            faces[side] = {"uv": uv, "texture": 0}

        elem = {
            "name": c["name"],
            "box_uv": False,
            "from": f,
            "to": t,
            "faces": faces,
            "uuid": str(uuid.uuid4()),
        }
        if "rotation" in c:
            elem["rotation"] = c["rotation"]
        if "origin" in c:
            elem["origin"] = c["origin"]

        elements.append(elem)

    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "meridian_bend")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "meridian_bend",
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": "meridian_bend",
                "folder": "block",
                "namespace": "bong",
                "id": 0,
                "source": tex_base64,
            }
        ],
    }

    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    try:
        rel = out_path.relative_to(REPO)
    except ValueError:
        rel = out_path
    print(f"✓ meridian_bend bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 渲染与并排对标卡输出
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    # 俯视图：一眼看清四分之一圆弧
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    canvas_w = 1040
    canvas_h = 1040
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("FRONT (+Z View)", im_front, 20, 20),
        ("SIDE (+X Output)", im_side, 540, 20),
        ("3/4 ISOMETRIC", im_iso, 20, 540),
        ("TOP (Quarter Arc Bend)", im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 250, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png 已输出: {render_path}")

    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        top_scale_w = int(im_top.width * target_h / im_top.height)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)
        top_scaled = im_top.resize((top_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = top_scale_w + iso_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (b02_meridian_bend.png)", fill=(210, 200, 180))

        rx = ref_w + 24
        check_cv.paste(top_scaled, (rx, 30))
        check_cv.paste(iso_scaled, (rx + top_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (TOP VIEW + 3/4 VIEW)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_meridian_bend.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [-4.0, 8.0, -7.5],
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_meridian_bend.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b02 meridian_bend 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    bb_path = generate_bbmodel()
    render_views(bb_path)


if __name__ == "__main__":
    main()
