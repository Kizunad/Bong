#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b08: coupling (耦合接口)

风格：A 有机型 (活体血肉、暖粉筋管、肉箍骨夹耳、三级肉质喇叭口、内壁亮红衬层、中心真元内光、放射筋丝)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b08_coupling.png

调度要求（整格 16×16×16，长轴沿 z 方向）：
1. -Z 端：8×6 统一截面的经脉接口（暖粉筋管短节 #d9a08c + b01 同款肉箍 #8a2a2a，底边离地 2px: x in [-4, 4], y in [2, 8]）；
   窄端外面一圈骨夹箍（#d8ccb0，宽 2px，左右两侧各一个 2×3 的骨夹耳）。
2. 往 +Z 逐渐外扩成肉质喇叭口（#8a2a2a，3 级台阶：截面 8×6 → 12×10 → 14×14，最外圈边缘切角）：
   - 喇叭口内壁 #b05050，中心 #f6dcc4 一块当内光。
3. 喇叭外表面 6~8 条 #c07868 放射筋丝从窄端连到宽端。
4. 渲染图要求：render.png（3/4 + 侧视 + 正对喇叭口）/ check.png。
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
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/coupling")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b08_coupling.png")

# 配色表 (完全对齐 meridian_factory.md)
PALETTE = {
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉 (肉箍主体、三级肉质喇叭口)
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉 (接口深孔内壁/深陷孔底)
    "flesh_lit":        (176, 80, 80, 255),   # #b05050 亮肉红 (喇叭口内壁衬层)
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (骨夹箍、左右骨夹耳)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 (骨夹耳暗纹插销)
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 筋管暖粉 (-Z端8x6筋管短节)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋膜亮粉 (接口上下 1px 亮边)
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 筋丝 (喇叭外表面 8 条放射筋丝)
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 真元内光 (喇叭口中心内光核)
}

MAT_UV = {
    "flesh_main":       [0, 0, 16, 16],
    "flesh_dark":       [16, 0, 32, 16],
    "flesh_lit":        [32, 0, 48, 16],
    "bone_main":        [48, 0, 64, 16],
    "bone_dark":        [0, 16, 16, 32],
    "tendon_tube":      [16, 16, 32, 32],
    "tendon_highlight": [32, 16, 48, 32],
    "tendon_fiber":     [48, 16, 64, 32],
    "qi_glow":          [0, 32, 16, 48],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_narrow_inlet() -> List[dict]:
    """-Z 端 8×6 统一截面的经脉接口：
    - 暖粉筋管短节 #d9a08c (z: -8.0..-7.5，暴露 0.5px，截面 8×6: x in [-4, 4], y in [2, 8])；
    - 上下各 1px 亮边 #e8bca8；
    - b01 同款四面包覆肉箍 #8a2a2a (z: -7.5..-4.5，宽 3px，四周外凸 1px: x in [-5, 5], y in [1, 9])。
    """
    cubes = []

    # 1. 暖粉筋管短节 (z in [-8.0, -7.5], 宽 8px, 高 6px, 底边离地 2px)
    cubes.append({"name": "inlet_tube", "from": [-4.0, 2.0, -8.0], "to": [4.0, 8.0, -7.5], "group": "inlet", "material": "tendon_tube"})

    # 上下各 1px 亮边 (#e8bca8 tendon_highlight，贴于顶底表面微凸 0.05px，z in [-7.95, -7.55] 避开端面共面)
    cubes.append({"name": "inlet_tube_hl_bot", "from": [-3.9, 1.95, -7.95], "to": [3.9, 2.8, -7.55], "group": "inlet", "material": "tendon_highlight"})
    cubes.append({"name": "inlet_tube_hl_top", "from": [-3.9, 7.2, -7.95], "to": [3.9, 8.05, -7.55], "group": "inlet", "material": "tendon_highlight"})

    # 2. b01 同款肉箍四段包壁 (z in [-7.5, -4.5]，宽 3px，外廓 x in [-5, 5], y in [1, 9])
    # 底管套 (y: 1.0..2.0, x: -5..5)
    cubes.append({"name": "collar_flesh_bot", "from": [-5.0, 1.0, -7.5], "to": [ 5.0, 2.0, -4.5], "group": "inlet", "material": "flesh_main"})
    # 顶管套 (y: 8.0..9.0, x: -5..5)
    cubes.append({"name": "collar_flesh_top", "from": [-5.0, 8.0, -7.5], "to": [ 5.0, 9.0, -4.5], "group": "inlet", "material": "flesh_main"})
    # 左管套 (x: -5.0..-4.0, y: 2.0..8.0)
    cubes.append({"name": "collar_flesh_l",   "from": [-5.0, 2.0, -7.5], "to": [-4.0, 8.0, -4.5], "group": "inlet", "material": "flesh_main"})
    # 右管套 (x: 4.0..5.0, y: 2.0..8.0)
    cubes.append({"name": "collar_flesh_r",   "from": [ 4.0, 2.0, -7.5], "to": [ 5.0, 8.0, -4.5], "group": "inlet", "material": "flesh_main"})

    return cubes


def part_02_bone_clamp() -> List[dict]:
    """窄端外面一圈骨夹箍（#d8ccb0，宽 2px，左右两侧各一个 2×3 的骨夹耳）。
    位于 z in [-6.5, -4.5]（宽 2px），环绕在肉箍外周并向两侧伸出坚固夹耳。
    """
    cubes = []

    # 1. 骨夹箍外圈环带 (z in [-6.5, -4.5]，高 2px，厚 0.4px，外廓微凸出肉箍表面)
    # 底环板: y in [0.6, 1.0], x in [-5.0, 5.0]
    cubes.append({"name": "clamp_ring_bot", "from": [-5.0, 0.6, -6.5], "to": [ 5.0, 1.0, -4.5], "group": "bone_clamp", "material": "bone_main"})
    # 顶环板: y in [9.0, 9.4], x in [-5.0, 5.0]
    cubes.append({"name": "clamp_ring_top", "from": [-5.0, 9.0, -6.5], "to": [ 5.0, 9.4, -4.5], "group": "bone_clamp", "material": "bone_main"})
    # 左立板: x in [-5.4, -5.0], y in [1.0, 9.0]
    cubes.append({"name": "clamp_ring_l",   "from": [-5.4, 1.0, -6.5], "to": [-5.0, 9.0, -4.5], "group": "bone_clamp", "material": "bone_main"})
    # 右立板: x in [5.0, 5.4], y in [1.0, 9.0]
    cubes.append({"name": "clamp_ring_r",   "from": [ 5.0, 1.0, -6.5], "to": [ 5.4, 9.0, -4.5], "group": "bone_clamp", "material": "bone_main"})

    # 2. 左右两侧各一个 2×3 的骨夹耳 (厚 2px: z in [-6.5, -4.5])
    # 西侧夹耳 (宽 2px: x in [-7.4, -5.4], 高 3px: y in [3.5, 6.5])
    cubes.append({"name": "clamp_ear_w", "from": [-7.4, 3.5, -6.5], "to": [-5.4, 6.5, -4.5], "group": "bone_clamp", "material": "bone_main"})
    # 西夹耳锁孔暗纹 (#b8a888 bone_dark，贴于侧面)
    cubes.append({"name": "clamp_ear_w_pin", "from": [-7.45, 4.5, -5.9], "to": [-5.35, 5.5, -5.1], "group": "bone_clamp", "material": "bone_dark"})

    # 东侧夹耳 (宽 2px: x in [5.4, 7.4], 高 3px: y in [3.5, 6.5])
    cubes.append({"name": "clamp_ear_e", "from": [ 5.4, 3.5, -6.5], "to": [ 7.4, 6.5, -4.5], "group": "bone_clamp", "material": "bone_main"})
    # 东夹耳锁孔暗纹
    cubes.append({"name": "clamp_ear_e_pin", "from": [ 5.35, 4.5, -5.9], "to": [ 7.45, 5.5, -5.1], "group": "bone_clamp", "material": "bone_dark"})

    return cubes


def part_03_flaring_bell() -> List[dict]:
    """往 +Z 逐渐外扩成肉质喇叭口（#8a2a2a，3 级台阶：截面 8×6 → 12×10 → 14×14，最外圈边缘切角）。
    内壁使用 #b05050 (flesh_lit)，中心嵌入一块 #f6dcc4 (qi_glow) 当真元内光。
    """
    cubes = []

    # ═══════════ 第 1 级台阶 (过渡颈段：z in [-4.5, -0.5]，外 10x8，内 8x6) ═══════════
    # 外截面：x in [-5, 5], y in [1, 9]；内孔：x in [-4, 4], y in [2, 8]
    cubes.append({"name": "bell_step1_bot", "from": [-5.0, 1.0, -4.5], "to": [ 5.0, 2.0, -0.5], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_step1_top", "from": [-5.0, 8.0, -4.5], "to": [ 5.0, 9.0, -0.5], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_step1_l",   "from": [-5.0, 2.0, -4.5], "to": [-4.0, 8.0, -0.5], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_step1_r",   "from": [ 4.0, 2.0, -4.5], "to": [ 5.0, 8.0, -0.5], "group": "flaring_bell", "material": "flesh_main"})

    # 第 1 级内壁衬层 (#b05050 flesh_lit)
    cubes.append({"name": "bell_lining1_bot", "from": [-4.0, 2.0, -4.5], "to": [ 4.0, 2.1, -0.5], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining1_top", "from": [-4.0, 7.9, -4.5], "to": [ 4.0, 8.0, -0.5], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining1_l",   "from": [-4.0, 2.1, -4.5], "to": [-3.9, 7.9, -0.5], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining1_r",   "from": [ 3.9, 2.1, -4.5], "to": [ 4.0, 7.9, -0.5], "group": "flaring_bell", "material": "flesh_lit"})

    # ═══════════ 第 2 级台阶 (中段扩口：z in [-0.5, 3.5]，外 12x10，内 10x8) ═══════════
    # 外截面：x in [-6, 6], y in [0.5, 10.5]；内孔：x in [-5, 5], y in [1.5, 9.5]
    cubes.append({"name": "bell_step2_bot", "from": [-6.0, 0.5, -0.5], "to": [ 6.0, 1.5,  3.5], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_step2_top", "from": [-6.0, 9.5, -0.5], "to": [ 6.0, 10.5, 3.5], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_step2_l",   "from": [-6.0, 1.5, -0.5], "to": [-5.0, 9.5,  3.5], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_step2_r",   "from": [ 5.0, 1.5, -0.5], "to": [ 6.0, 9.5,  3.5], "group": "flaring_bell", "material": "flesh_main"})

    # 第 2 级内壁衬层 (#b05050 flesh_lit)
    cubes.append({"name": "bell_lining2_bot", "from": [-5.0, 1.5, -0.5], "to": [ 5.0, 1.6, 3.5], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining2_top", "from": [-5.0, 9.4, -0.5], "to": [ 5.0, 9.5, 3.5], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining2_l",   "from": [-5.0, 1.6, -0.5], "to": [-4.9, 9.4, 3.5], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining2_r",   "from": [ 4.9, 1.6, -0.5], "to": [ 5.0, 9.4, 3.5], "group": "flaring_bell", "material": "flesh_lit"})

    # ═══════════ 第 3 级台阶 (大喇叭敞口：z in [3.5, 8.0]，外 14x14 边缘切角，内 12x12) ═══════════
    # 外截面：x in [-7, 7], y in [0, 14]，四个竖角切除 2x2 做圆钝八边形喇叭花轮廓
    # 底壁 (y: 0..1, x in [-5, 5])
    cubes.append({"name": "bell_step3_bot", "from": [-5.0, 0.0, 3.5], "to": [ 5.0, 1.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    # 顶壁 (y: 13..14, x in [-5, 5])
    cubes.append({"name": "bell_step3_top", "from": [-5.0, 13.0, 3.5], "to": [ 5.0, 14.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    # 左壁 (x: -7..-6, y in [2, 12])
    cubes.append({"name": "bell_step3_l",   "from": [-7.0, 2.0, 3.5], "to": [-6.0, 12.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    # 右壁 (x: 6..7, y in [2, 12])
    cubes.append({"name": "bell_step3_r",   "from": [ 6.0, 2.0, 3.5], "to": [ 7.0, 12.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})

    # 四个切角圆钝过渡块 (在四个角切角处垫角)
    cubes.append({"name": "bell_chamfer_bl", "from": [-6.0, 1.0, 3.5], "to": [-5.0, 2.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_chamfer_br", "from": [ 5.0, 1.0, 3.5], "to": [ 6.0, 2.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_chamfer_tl", "from": [-6.0, 12.0, 3.5], "to": [-5.0, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})
    cubes.append({"name": "bell_chamfer_tr", "from": [ 5.0, 12.0, 3.5], "to": [ 6.0, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_main"})

    # 第 3 级内壁衬层 (#b05050 flesh_lit)
    cubes.append({"name": "bell_lining3_bot", "from": [-5.0, 1.0, 3.5], "to": [ 5.0, 1.1, 8.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining3_top", "from": [-5.0, 12.9, 3.5], "to": [ 5.0, 13.0, 8.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining3_l",   "from": [-6.0, 2.0, 3.5], "to": [-5.9, 12.0, 8.0], "group": "flaring_bell", "material": "flesh_lit"})
    cubes.append({"name": "bell_lining3_r",   "from": [ 5.9, 2.0, 3.5], "to": [ 6.0, 12.0, 8.0], "group": "flaring_bell", "material": "flesh_lit"})

    # ═══════════ 中心真元内光 (#f6dcc4 qi_glow，深陷于喇叭深部) ═══════════
    # 位于 z in [-0.4, 2.0]，截面 3.6x3.6: x in [-1.8, 1.8], y in [3.7, 7.3]
    cubes.append({"name": "core_qi_glow", "from": [-1.8, 3.7, -0.4], "to": [1.8, 7.3, 2.0], "group": "flaring_bell", "material": "qi_glow"})

    return cubes


def part_04_radiating_fibers() -> List[dict]:
    """喇叭外表面 8 条 #c07868 放射筋丝从窄端连到宽端。
    微浮出外壁 0.08px，由窄端 (-Z = -4.5) 向宽端 (+Z = 7.8) 斜向外展放射贯穿全长。
    """
    cubes = []

    # 顶面 2 条 (微浮出顶壁 y in [14.05, 14.12], z in [-4.5, 7.8])
    cubes.append({"name": "fiber_top_l", "from": [-3.5, 14.05, -4.5], "to": [-1.0, 14.12, 7.8], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_top_r", "from": [ 1.0, 14.05, -4.5], "to": [ 3.5, 14.12, 7.8], "group": "fibers", "material": "tendon_fiber"})

    # 底面 2 条 (微浮出底壁 y in [-0.12, -0.05], z in [-4.5, 7.8])
    cubes.append({"name": "fiber_bot_l", "from": [-3.5, -0.12, -4.5], "to": [-1.0, -0.05, 7.8], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_bot_r", "from": [ 1.0, -0.12, -4.5], "to": [ 3.5, -0.05, 7.8], "group": "fibers", "material": "tendon_fiber"})

    # 西侧面 (-X) 2 条 (微浮出外壁 x in [-7.12, -7.05], z in [-4.5, 7.8])
    cubes.append({"name": "fiber_w_top", "from": [-7.12, 9.0, -4.5], "to": [-7.05, 11.5, 7.8], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_w_bot", "from": [-7.12, 2.5, -4.5], "to": [-7.05,  5.0, 7.8], "group": "fibers", "material": "tendon_fiber"})

    # 东侧面 (+X) 2 条 (微浮出外壁 x in [7.05, 7.12], z in [-4.5, 7.8])
    cubes.append({"name": "fiber_e_top", "from": [ 7.05, 9.0, -4.5], "to": [ 7.12, 11.5, 7.8], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_e_bot", "from": [ 7.05, 2.5, -4.5], "to": [ 7.12,  5.0, 7.8], "group": "fibers", "material": "tendon_fiber"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_narrow_inlet()
        + part_02_bone_clamp()
        + part_03_flaring_bell()
        + part_04_radiating_fibers()
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
    """生成 64×64 RGBA 贴图。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if "flesh" in mat_name or "tendon" in mat_name:
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            noise = rng.randint(-8, 8, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "qi" in mat_name:
            noise = rng.randint(-6, 6, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(out_path: Path) -> Path:
    """导出 coupling 的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes()
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
        uv = MAT_UV.get(mat, [0, 0, 16, 16])
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
        elements.append(elem)

    name = "coupling"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "coupling")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": name,
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
    print(f"✓ {name} bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 渲染与并排对标卡输出
# =============================================================================

def render_views(model_p: Path):
    """输出包含（3/4 视 + 侧视 + 正对喇叭口直视 + 背部接口视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各视角：
    # 3/4 等轴视 (看外扩喇叭全貌、骨夹箍夹耳、放射筋丝)
    im_iso, _ = render(model_p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    # 侧视 (SIDE: yaw=90, pitch=0 看从 -Z 到 +Z 阶梯外扩与放射筋丝)
    im_side, _ = render(model_p, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # 正对喇叭口直视 (yaw=180, pitch=0 从 +Z 正面直视 14x14 喇叭口内壁与真元内光)
    im_mouth, _ = render(model_p, yaw=180.0, pitch=0.0, size=500, bg=bg_color)
    # 背面直视 (yaw=0, pitch=0 从 -Z 直视 8x6 统一截面经脉接口与骨夹耳)
    im_inlet, _ = render(model_p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格拼版：3/4 等轴视、侧视、正对喇叭口、背面接口)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("3/4 ISOMETRIC (Flaring Bell & Bone Clamp)", im_iso, 10, 10),
        ("SIDE VIEW (8x6 -> 12x10 -> 14x14 Flaring)", im_side, cell_w + 20, 10),
        ("FRONT VIEW (+Z Straight Into Bell Mouth & Qi)", im_mouth, 10, cell_h + 20),
        ("BACK VIEW (-Z Standard 8x6 Inlet & Clamps)", im_inlet, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 420, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (3/4 + 侧视 + 正对喇叭口 + 接口 2x2 拼版) 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图，右当前模型 3/4 等轴视与正对喇叭口视并排等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_iso = im_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_mouth = im_mouth.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b08_coupling.png)", fill=(210, 200, 180))

        # 贴右渲染图 (左 3/4 视，右正对喇叭口视)
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_mouth, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: Frontal Bell Mouth View)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含 3/4 视与喇叭口对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_coupling.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] coupling 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [ 4.0, 8.0, -7.5],
        "material": "tendon_tube",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] coupling 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: coupling 注入共面冲突未被拦截!")

    print("✓ gen_coupling.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b08 coupling 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p = generate_bbmodel(out_path=MODEL_DIR / "coupling.bbmodel")
    render_views(p)


if __name__ == "__main__":
    main()
