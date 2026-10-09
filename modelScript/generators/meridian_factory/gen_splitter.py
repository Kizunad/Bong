#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b05: splitter (分流器)

风格：A 有机型 (活体血肉、骨轨立柱、T 形传送带、中心紧凑筋壳、外置三角分流骨楔)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md「传送带与分流器的统一约定」
- 调度审 b05 第 1 次（2026-10-09 13:1x）更正指标：
  1. 骨轨：截面正好 2 宽 × 2 高，放在带面两侧，底在 y=4、顶在 y=6（即只比带面高 2px）；
  2. 立柱：2×2，底在 y=4、顶在 y=8，三条带两侧按立柱规格各一根（共 6 根），柱面 #b8a888 暗纹；
  3. 带面：宽 8px 居中，顶在 y=4；带面下 y=0–4 是筋质底座 (#d9a08c)；
  4. 骨轨外侧到方块边留 2px 空，从侧面能看到筋质底座和斜筋丝；
  5. 筋质外壳缩小到 x/z in [-3, 3] (对应 5–11 网格)、y 0–8，不要盖住带面；三条带在外壳边缘截止；
  6. 骨楔看得见：放在外壳正对入口那一面 (z=-3) 外面，三角形俯视轮廓（底 6、尖朝 -Z 伸出 3px 到 z=-6），高 y 4–8，#d8ccb0；
  7. 去掉四角那 4 根大骨柱。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/splitter")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b05_splitter.png")

# 配色表 (完全对齐 meridian_factory.md)
PALETTE = {
    "bone_main":        (216, 204, 176, 255), # #d8ccb0 骨 (骨轨、6 根立柱、外置分流骨楔)
    "bone_dark":        (184, 168, 136, 255), # #b8a888 骨暗面 (立柱柱面暗纹、骨楔刀锋脊线)
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 红肌纤维 (传送带带面主肋)
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 深红肌纤维 (传送带带面暗肋)
    "tendon_tube":      (217, 160, 140, 255), # #d9a08c 暖粉筋质 (中心外壳主体、带面筋底座)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 筋质高光亮面 (外壳顶面与后侧亮板)
    "tendon_fiber":     (192, 120, 104, 255), # #c07868 筋丝 (外壳与底座表面斜交筋丝)
}

MAT_UV = {
    "bone_main":        [0, 0, 16, 16],
    "bone_dark":        [16, 0, 32, 16],
    "flesh_main":       [32, 0, 48, 16],
    "flesh_dark":       [48, 0, 64, 16],
    "tendon_tube":      [0, 16, 16, 32],
    "tendon_highlight": [16, 16, 32, 32],
    "tendon_fiber":     [32, 16, 48, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_conveyor_in() -> List[dict]:
    """入口传送带（-Z 面进，从 z = -8.0 到 z = -3.0）：
    - 宽 8px 居中 (x in [-4, 4])，带面顶高 y=4 (y in [3.8, 4.0])；
    - 沿 Z 轴运动方向每 2px 一条横肋 (#8a2a2a 与 #5a1a1a 交替)；
    - 带面下方 y=0–4 是筋质底座 (#d9a08c，y in [0.0, 3.8])；
    - 端头外侧两侧各一根立柱 2×2，底在 y=4、顶在 y=8 (z in [-8, -6]，柱面带 #b8a888 暗纹)；
    - 骨轨外侧到方块边留 2px 空 (x=-8..-6 与 6..8 是空的)，侧面能看到筋质底座与斜筋丝。
    """
    cubes = []

    # 1. 筋质底座 (宽 8px，y in [0.0, 3.8]，z in [-8.0, -3.0])
    cubes.append({"name": "in_base", "from": [-4.0, 0.0, -8.0], "to": [4.0, 3.8, -3.0], "group": "conveyor_in", "material": "tendon_tube"})

    # 底座侧面斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "in_base_fiber_w", "from": [-4.05, 0.8, -7.5], "to": [-4.0, 2.8, -5.5], "group": "conveyor_in", "material": "tendon_fiber"})
    cubes.append({"name": "in_base_fiber_e", "from": [ 4.0,  0.8, -7.5], "to": [ 4.05, 2.8, -5.5], "group": "conveyor_in", "material": "tendon_fiber"})

    # 2. 带面横肋 (顶高 y=4: y in [3.8, 4.0]，长 2px 交替)
    # 肋 1: z in [-8.0, -6.0], #8a2a2a (flesh_main)
    cubes.append({"name": "in_belt_rib_1", "from": [-4.0, 3.8, -8.0], "to": [4.0, 4.0, -6.0], "group": "conveyor_in", "material": "flesh_main"})
    # 肋 2: z in [-6.0, -4.0], #5a1a1a (flesh_dark)
    cubes.append({"name": "in_belt_rib_2", "from": [-4.0, 3.8, -6.0], "to": [4.0, 4.0, -4.0], "group": "conveyor_in", "material": "flesh_dark"})
    # 肋 3: z in [-4.0, -3.0], #8a2a2a (flesh_main)
    cubes.append({"name": "in_belt_rib_3", "from": [-4.0, 3.8, -4.0], "to": [4.0, 4.0, -3.0], "group": "conveyor_in", "material": "flesh_main"})

    # 3. 外端两侧各一根立柱 (截面 2x2，底在 y=4、顶在 y=8: y in [4.0, 8.0], z in [-8.0, -6.0])
    # 西立柱: x in [-6.0, -4.0]
    cubes.append({"name": "in_post_w", "from": [-6.0, 4.0, -8.0], "to": [-4.0, 8.0, -6.0], "group": "conveyor_in", "material": "bone_main"})
    cubes.append({"name": "in_post_w_stripe", "from": [-6.05, 5.5, -7.5], "to": [-3.95, 6.5, -6.5], "group": "conveyor_in", "material": "bone_dark"})
    # 东立柱: x in [4.0, 6.0]
    cubes.append({"name": "in_post_e", "from": [ 4.0, 4.0, -8.0], "to": [ 6.0, 8.0, -6.0], "group": "conveyor_in", "material": "bone_main"})
    cubes.append({"name": "in_post_e_stripe", "from": [ 3.95, 5.5, -7.5], "to": [ 6.05, 6.5, -6.5], "group": "conveyor_in", "material": "bone_dark"})

    return cubes


def part_02_conveyors_out_and_rails() -> List[dict]:
    """左右两条出口传送带及连通转角骨轨系统：
    - 左出口带 (-X 向)：x in [-8.0, -4.0]，带面宽 8px 居中 (z in [-4, 4])，顶高 y=4；
    - 右出口带 (+X 向)：x in [ 4.0,  8.0]，带面宽 8px 居中 (z in [-4, 4])，顶高 y=4；
    - 骨轨：截面 2 宽 × 2 高，底在 y=4、顶在 y=6；
      * 西北角转角轨: x in [-6, -4], z in [-6, -4], y in [4, 6]
      * 东北角转角轨: x in [ 4,  6], z in [-6, -4], y in [4, 6]
      * 左出南骨轨:   x in [-6, -3], z in [ 4,  6], y in [4, 6]
      * 右出南骨轨:   x in [ 3,  6], z in [ 4,  6], y in [4, 6]
    - 四根出口立柱 (左出 2 根，右出 2 根，截面 2x2，底在 y=4、顶在 y=8，带 #b8a888 暗纹)。
    """
    cubes = []

    # ═══════════ 左出口带 (-X 向) ═══════════
    # 1. 筋质底座 (x in [-8.0, -4.0], y in [0.0, 3.8], z in [-4.0, 4.0])
    cubes.append({"name": "out_l_base", "from": [-8.0, 0.0, -4.0], "to": [-4.0, 3.8, 4.0], "group": "conveyors_out", "material": "tendon_tube"})

    # 底座侧面斜筋丝
    cubes.append({"name": "out_l_base_fiber_s", "from": [-7.5, 0.8, 4.0], "to": [-5.5, 2.8, 4.05], "group": "conveyors_out", "material": "tendon_fiber"})

    # 2. 带面横肋 (y in [3.8, 4.0])
    cubes.append({"name": "out_l_rib_1", "from": [-8.0, 3.8, -4.0], "to": [-6.0, 4.0, 4.0], "group": "conveyors_out", "material": "flesh_main"})
    cubes.append({"name": "out_l_rib_2", "from": [-6.0, 3.8, -4.0], "to": [-4.0, 4.0, 4.0], "group": "conveyors_out", "material": "flesh_dark"})

    # 3. 外端两侧各一根立柱 (截面 2x2，y in [4.0, 8.0], x in [-8.0, -6.0])
    # 北立柱: z in [-6.0, -4.0]
    cubes.append({"name": "out_l_post_n", "from": [-8.0, 4.0, -6.0], "to": [-6.0, 8.0, -4.0], "group": "conveyors_out", "material": "bone_main"})
    cubes.append({"name": "out_l_post_n_stripe", "from": [-7.5, 5.5, -6.05], "to": [-6.5, 6.5, -3.95], "group": "conveyors_out", "material": "bone_dark"})
    # 南立柱: z in [4.0, 6.0]
    cubes.append({"name": "out_l_post_s", "from": [-8.0, 4.0,  4.0], "to": [-6.0, 8.0,  6.0], "group": "conveyors_out", "material": "bone_main"})
    cubes.append({"name": "out_l_post_s_stripe", "from": [-7.5, 5.5,  3.95], "to": [-6.5, 6.5,  6.05], "group": "conveyors_out", "material": "bone_dark"})

    # 4. 南骨轨 (截面 2x2，底在 y=4、顶在 y=6: x in [-6.0, -3.0], z in [4.0, 6.0])
    cubes.append({"name": "out_l_rail_s", "from": [-6.0, 4.0,  4.0], "to": [-3.0, 6.0,  6.0], "group": "conveyors_out", "material": "bone_main"})

    # ═══════════ 右出口带 (+X 向) ═══════════
    # 1. 筋质底座 (x in [4.0, 8.0], y in [0.0, 3.8], z in [-4.0, 4.0])
    cubes.append({"name": "out_r_base", "from": [4.0, 0.0, -4.0], "to": [8.0, 3.8, 4.0], "group": "conveyors_out", "material": "tendon_tube"})

    # 底座侧面斜筋丝
    cubes.append({"name": "out_r_base_fiber_s", "from": [5.5, 0.8, 4.0], "to": [7.5, 2.8, 4.05], "group": "conveyors_out", "material": "tendon_fiber"})

    # 2. 带面横肋 (y in [3.8, 4.0])
    cubes.append({"name": "out_r_rib_1", "from": [4.0, 3.8, -4.0], "to": [6.0, 4.0, 4.0], "group": "conveyors_out", "material": "flesh_dark"})
    cubes.append({"name": "out_r_rib_2", "from": [6.0, 3.8, -4.0], "to": [8.0, 4.0, 4.0], "group": "conveyors_out", "material": "flesh_main"})

    # 3. 外端两侧各一根立柱 (截面 2x2，y in [4.0, 8.0], x in [6.0, 8.0])
    # 北立柱: z in [-6.0, -4.0]
    cubes.append({"name": "out_r_post_n", "from": [6.0, 4.0, -6.0], "to": [8.0, 8.0, -4.0], "group": "conveyors_out", "material": "bone_main"})
    cubes.append({"name": "out_r_post_n_stripe", "from": [6.5, 5.5, -6.05], "to": [7.5, 6.5, -3.95], "group": "conveyors_out", "material": "bone_dark"})
    # 南立柱: z in [4.0, 6.0]
    cubes.append({"name": "out_r_post_s", "from": [6.0, 4.0,  4.0], "to": [8.0, 8.0,  6.0], "group": "conveyors_out", "material": "bone_main"})
    cubes.append({"name": "out_r_post_s_stripe", "from": [6.5, 5.5,  3.95], "to": [7.5, 6.5,  6.05], "group": "conveyors_out", "material": "bone_dark"})

    # 4. 南骨轨 (截面 2x2，底在 y=4、顶在 y=6: x in [3.0, 6.0], z in [4.0, 6.0])
    cubes.append({"name": "out_r_rail_s", "from": [3.0, 4.0,  4.0], "to": [6.0, 6.0,  6.0], "group": "conveyors_out", "material": "bone_main"})

    # ═══════════ 连通拐角骨轨 (西北角与东北角) ═══════════
    # 西北转角骨轨: 连接入口西立柱 (z=-6) 与左出北立柱 (x=-6)，截面 2x2，底在 y=4、顶在 y=6
    cubes.append({"name": "corner_rail_nw", "from": [-6.0, 4.0, -6.0], "to": [-4.0, 6.0, -4.0], "group": "conveyors_out", "material": "bone_main"})
    # 东北转角骨轨: 连接入口东立柱 (z=-6) 与右出北立柱 (x=6)，截面 2x2，底在 y=4、顶在 y=6
    cubes.append({"name": "corner_rail_ne", "from": [ 4.0, 4.0, -6.0], "to": [ 6.0, 6.0, -4.0], "group": "conveyors_out", "material": "bone_main"})

    return cubes


def part_03_central_sac() -> List[dict]:
    """筋质外壳缩小到 x/z in [-3, 3] (对应 5–11 网格)、y 0–8，不要盖住带面；
    三条带在外壳边缘截止。
    - 顶面圆角：y 7.2..8.0 向内微缩 0.5px (x/z in [-2.5, 2.5])；
    - 顶面亮面 #e8bca8 与斜筋丝 #c07868；
    - 正面(+Z)为封闭后背壁，配 #e8bca8 亮面与斜筋丝。
    """
    cubes = []

    # 1. 筋质外壳主体下半部 (y in [0.0, 7.2]，x/z in [-3.0, 3.0])
    cubes.append({"name": "sac_body", "from": [-3.0, 0.0, -3.0], "to": [3.0, 7.2, 3.0], "group": "central_sac", "material": "tendon_tube"})

    # 2. 顶面圆钝阶梯顶盖 (y in [7.2, 8.0]，四周内缩 0.5px: x/z in [-2.5, 2.5])
    cubes.append({"name": "sac_top_cap", "from": [-2.5, 7.2, -2.5], "to": [2.5, 8.0, 2.5], "group": "central_sac", "material": "tendon_tube"})

    # 顶面透光亮板 (#e8bca8 tendon_highlight，贴于顶面微凸 0.05px: y in [8.0, 8.06])
    cubes.append({"name": "sac_top_glow", "from": [-1.8, 8.0, -1.8], "to": [1.8, 8.06, 1.8], "group": "central_sac", "material": "tendon_highlight"})

    # 顶面斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "sac_top_fiber", "from": [-1.6, 8.06, -0.8], "to": [1.6, 8.12, 0.8], "group": "central_sac", "material": "tendon_fiber"})

    # 3. 正面 (+Z，即 z = 3.0 背侧盲端封闭壁) 亮色衬板 (#e8bca8 tendon_highlight，z in [3.0, 3.06], y in [1.5, 6.5])
    cubes.append({"name": "sac_back_glow", "from": [-2.0, 1.5, 3.0], "to": [2.0, 6.5, 3.06], "group": "central_sac", "material": "tendon_highlight"})

    # 正面 (+Z) 斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "sac_back_fiber_1", "from": [-2.5, 2.0, 3.06], "to": [-0.5, 4.0, 3.12], "group": "central_sac", "material": "tendon_fiber"})
    cubes.append({"name": "sac_back_fiber_2", "from": [ 0.5, 4.5, 3.06], "to": [ 2.5, 6.5, 3.12], "group": "central_sac", "material": "tendon_fiber"})

    return cubes


def part_04_bone_wedge() -> List[dict]:
    """骨楔看得见：放在外壳正对入口的那面 (z=-3) 外面，
    三角形俯视轮廓（底 6、尖朝 -Z 伸出 3px 到 z=-6），高 y 4–8 (y in [4.05, 8.0])，#d8ccb0。
    立在入口带面 (y=4) 之上，迎风将物料流分向左右出口带。
    """
    cubes = []

    # 1. 迎风锋利尖刀 (尖端位于 z = -6.0，深 z in [-6.0, -5.0]，宽 1.4px: x in [-0.7, 0.7]，高 y in [4.05, 8.0])
    cubes.append({"name": "wedge_tip", "from": [-0.7, 4.05, -6.0], "to": [0.7, 8.0, -5.0], "group": "bone_wedge", "material": "bone_main"})

    # 2. 中段过渡梯形 (深 z in [-5.0, -4.0]，宽 3.4px: x in [-1.7, 1.7]，高 y in [4.05, 8.0])
    cubes.append({"name": "wedge_mid", "from": [-1.7, 4.05, -5.0], "to": [1.7, 8.0, -4.0], "group": "bone_wedge", "material": "bone_main"})

    # 3. 根部大底座 (紧贴外壳北壁 z=-3.0，深 z in [-4.0, -3.0]，宽 5.9px ≈ 6px: x in [-2.95, 2.95]，高 y in [4.05, 8.0])
    cubes.append({"name": "wedge_base", "from": [-2.95, 4.05, -4.0], "to": [2.95, 8.0, -3.0], "group": "bone_wedge", "material": "bone_main"})

    # 4. 骨楔正中迎风刀锋暗面脊线 (#b8a888 bone_dark，沿劈尖最前端凸出 0.05px)
    cubes.append({"name": "wedge_ridge_spine", "from": [-0.25, 4.5, -6.06], "to": [0.25, 7.8, -6.0], "group": "bone_wedge", "material": "bone_dark"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_conveyor_in()
        + part_02_conveyors_out_and_rails()
        + part_03_central_sac()
        + part_04_bone_wedge()
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

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(out_path: Path) -> Path:
    """导出 splitter 的 bbmodel 文件。"""
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
        mat = c.get("material", "tendon_tube")
        uv = MAT_UV.get(mat, [0, 16, 16, 32])
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

    name = "splitter"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "splitter")
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
    """输出包含（俯视 + 3/4 + 侧视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各视角：俯视 (TOP)、3/4 等轴视 (Isometric)、侧视 (SIDE: 观察带面与筋底座)、正视 (FRONT)
    im_top, _ = render(model_p, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_iso, _ = render(model_p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_side, _ = render(model_p, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_front, _ = render(model_p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格拼版：俯视、3/4 视、侧视、正视)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("TOP VIEW (T-Conveyor & Visible Bone Wedge)", im_top, 10, 10),
        ("3/4 ISOMETRIC (Compact Shell, Wedge & 6 Posts)", im_iso, cell_w + 20, 10),
        ("SIDE VIEW (Rails y:4-6, Posts y:4-8, Base y:0-4)", im_side, 10, cell_h + 20),
        ("FRONT VIEW (+Z Closed Back Wall)", im_front, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 400, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (俯视 + 3/4 + 侧视 + 正视 2x2 拼版) 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图，右当前模型 3/4 等轴视与俯视并排等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_iso = im_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_top = im_top.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), "REFERENCE (b05_splitter.png)", fill=(210, 200, 180))

        # 贴右渲染图 (左 3/4 视，右 TOP 俯视)
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_top, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: TOP View | Visible Wedge & T-Conveyors)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图 (含 3/4 视角与俯视对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_splitter.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] splitter 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 0.0, -8.0],
        "to":   [ 4.0, 3.8, -3.0],
        "material": "tendon_tube",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] splitter 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: splitter 注入共面冲突未被拦截!")

    print("✓ gen_splitter.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b05 splitter 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p = generate_bbmodel(out_path=MODEL_DIR / "splitter.bbmodel")
    render_views(p)


if __name__ == "__main__":
    main()
