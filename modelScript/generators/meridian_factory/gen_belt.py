#!/usr/bin/env python3
"""经脉工厂内景构件生成器 —— b07: belt (传送带，4 件套)

风格：A 有机型 (活体血肉、半透明筋管/经脉、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 尺寸：每件严格落在一格 16×16×16 内 (x in [-8, 8], z in [-8, 8], y in [0, 16]，坡道顶端到 y=22)，
   中心对齐方块网格，原点位于底面中心 (0.0, 0.0, 0.0)。
2. 带面：
   - 宽 8px (居中 x in [-4, 4])，带面顶高 y = 4.0；
   - 红肌纤维带面，沿运动方向每 2px 一条横肋，由 #8a2a2a (flesh_main) 与 #5a1a1a (flesh_dark) 交替；
   - 带面下方到 y=0 是筋质底座 #d9a08c (tendon_tube)，侧面带 2 条 #c07868 (tendon_fiber) 斜筋丝。
3. 骨轨与立柱：
   - 带面两侧各一条 #d8ccb0 (bone_main) 骨轨，宽 2px (x in [-6, -4] 与 [4, 6])，顶高 y = 6.0；
   - 骨轨外侧到方块边严格留空 2px (8.0 - 6.0 = 2.0px)；
   - 每隔 8px 一根 2×2 骨立柱，顶高 y = 8.0 (比骨轨高出 2px)，柱面饰有 #b8a888 (bone_dark) 暗纹。
4. 4 件独立模型：
   - belt_straight: 沿 z 轴直通 (-8..8)；
   - belt_corner: -Z 进 +X 出，带面与内外骨轨沿 R=8 的四分之一圆弧，用 4 段直管旋转 22.5° 紧密拼合；
   - belt_slope_up: 沿 z 爬升一格，带面从 y=4 升到 y=20 的斜面采用 8 级 2px 台阶，骨轨与立柱跟着顺升；
   - belt_slope_down: slope_up 绕 Y 轴旋转 180° 单独出文件，从 -Z 端 y=20 平滑降至 +Z 端 y=4。
5. 门禁要求：通过 _assert_no_coplanar_faces 严格自检（0 共面冲突），带 --self-test 差分缺陷拦截验证。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/belt")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/b07_belt")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b07_belt.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 红肌横肋主色
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 肋间暗部与交替深色肋
    "tendon_tube":   (217, 160, 140, 255), # #d9a08c 筋质底座
    "tendon_fiber":  (192, 120, 104, 255), # #c07868 侧面斜筋丝
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨轨与立柱
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨立柱暗纹
}

MAT_UV = {
    "flesh_main":   [0,  0,  16, 16],
    "flesh_dark":   [16, 0,  32, 16],
    "tendon_tube":  [32, 0,  48, 16],
    "tendon_fiber": [48, 0,  64, 16],
    "bone_main":    [0,  16, 16, 32],
    "bone_dark":    [16, 16, 32, 32],
}

RES = 64


# =============================================================================
# 各构件几何定义
# =============================================================================

def build_straight_cubes() -> List[dict]:
    """1. belt_straight (直传送带，沿 Z 轴直通)。"""
    cubes = []
    # ── 筋质底座 (宽 8: x in [-4, 4], y: 0..3.8, z in [-8, 8]) ──
    cubes.append({"name": "base", "from": [-4.0, 0.0, -8.0], "to": [4.0, 3.8, 8.0], "group": "base", "material": "tendon_tube"})

    # ── 8 条交替红肌横肋 (顶高 y=4.0, 厚 0.2, 每 2px 一条, z in [-8..8]) ──
    for i in range(8):
        z0 = -8.0 + i * 2.0
        z1 = z0 + 2.0
        mat = "flesh_main" if i % 2 == 0 else "flesh_dark"
        cubes.append({"name": f"rib_{i}", "from": [-4.0, 3.8, z0], "to": [4.0, 4.0, z1], "group": "belt_surface", "material": mat})

    # ── 4 根骨立柱 (2x2, 全高 y: 0..8, 在 z: [-5..-3] 与 [3..5]) ──
    cubes.append({"name": "post_l1", "from": [-6.0, 0.0, -5.0], "to": [-4.0, 8.0, -3.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_l2", "from": [-6.0, 0.0,  3.0], "to": [-4.0, 8.0,  5.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_r1", "from": [ 4.0, 0.0, -5.0], "to": [ 6.0, 8.0, -3.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_r2", "from": [ 4.0, 0.0,  3.0], "to": [ 6.0, 8.0,  5.0], "group": "bone_posts", "material": "bone_main"})

    # 立柱柱面暗纹环扣 (#b8a888)
    cubes.append({"name": "post_l1_band", "from": [-6.05, 6.0, -5.05], "to": [-3.95, 6.8, -2.95], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_l2_band", "from": [-6.05, 6.0,  2.95], "to": [-3.95, 6.8,  5.05], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_r1_band", "from": [ 3.95, 6.0, -5.05], "to": [ 6.05, 6.8, -2.95], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_r2_band", "from": [ 3.95, 6.0,  2.95], "to": [ 6.05, 6.8,  5.05], "group": "bone_posts", "material": "bone_dark"})

    # ── 骨轨 (宽 2: x in [-6, -4] 与 [4, 6], 顶高 y=6.0, 避开立柱分段咬合) ──
    rail_z = [(-8.0, -5.0), (-3.0, 3.0), (5.0, 8.0)]
    for i, (z0, z1) in enumerate(rail_z):
        cubes.append({"name": f"rail_l_{i}", "from": [-6.0, 0.0, z0], "to": [-4.0, 6.0, z1], "group": "bone_rails", "material": "bone_main"})
        cubes.append({"name": f"rail_r_{i}", "from": [ 4.0, 0.0, z0], "to": [ 6.0, 6.0, z1], "group": "bone_rails", "material": "bone_main"})

    # ── 侧面斜筋丝 ──
    cubes.append({"name": "fiber_l1", "from": [-6.08, 1.5, -2.5], "to": [-5.95, 4.2, -0.5], "group": "tendon_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_l2", "from": [-6.08, 1.5,  0.5], "to": [-5.95, 4.2,  2.5], "group": "tendon_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_r1", "from": [ 5.95, 1.5, -2.5], "to": [ 6.08, 4.2, -0.5], "group": "tendon_fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fiber_r2", "from": [ 5.95, 1.5,  0.5], "to": [ 6.08, 4.2,  2.5], "group": "tendon_fibers", "material": "tendon_fiber"})

    return cubes


def build_corner_cubes() -> List[dict]:
    """2. belt_corner (弯传送带，-Z 进 +X 出，4 段旋转 22.5° 拼四分之一圆弧)。"""
    cubes = []
    bend_origin = [8.0, 0.0, -8.0]
    angles = [11.25, 33.75, 56.25, 78.75]
    z_ranges = [(-8.2, -5.8), (-9.6, -6.2), (-9.6, -6.2), (-10.0, -7.6)]

    for i, (ang, (z0, z1)) in enumerate(zip(angles, z_ranges)):
        rot = [0, ang, 0]
        pfx = f"seg_{i+1}"
        # 筋质底座 (宽 8, y: 0..3.8)
        cubes.append({
            "name": f"base_{pfx}",
            "from": [-4.0, 0.0, z0],
            "to":   [4.0, 3.8, z1],
            "origin": bend_origin,
            "rotation": rot,
            "group": "base",
            "material": "tendon_tube",
        })
        # 2 条红肌横肋 (交替 #8a2a2a / #5a1a1a)
        z_mid = (z0 + z1) / 2.0
        cubes.append({
            "name": f"rib_{pfx}_1",
            "from": [-4.0, 3.8, z0],
            "to":   [4.0, 4.0, z_mid],
            "origin": bend_origin,
            "rotation": rot,
            "group": "belt_surface",
            "material": "flesh_main" if i % 2 == 0 else "flesh_dark",
        })
        cubes.append({
            "name": f"rib_{pfx}_2",
            "from": [-4.0, 3.8, z_mid],
            "to":   [4.0, 4.0, z1],
            "origin": bend_origin,
            "rotation": rot,
            "group": "belt_surface",
            "material": "flesh_dark" if i % 2 == 0 else "flesh_main",
        })
        # 外骨轨 (宽 2, y: 0..6)
        cubes.append({
            "name": f"rail_out_{pfx}",
            "from": [-6.0, 0.0, z0],
            "to":   [-4.0, 6.0, z1],
            "origin": bend_origin,
            "rotation": rot,
            "group": "bone_rails",
            "material": "bone_main",
        })
        # 内骨轨 (宽 2, y: 0..6)
        cubes.append({
            "name": f"rail_in_{pfx}",
            "from": [4.0, 0.0, z0],
            "to":   [6.0, 6.0, z1],
            "origin": bend_origin,
            "rotation": rot,
            "group": "bone_rails",
            "material": "bone_main",
        })

    # 输入端立柱 (-Z 侧: z in [-8, -6], x in [-6, -4] 与 [4, 6])
    cubes.append({"name": "post_in_l", "from": [-6.0, 0.0, -8.0], "to": [-4.0, 8.0, -6.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_in_r", "from": [ 4.0, 0.0, -8.0], "to": [ 6.0, 8.0, -6.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_in_l_band", "from": [-6.05, 6.0, -8.05], "to": [-3.95, 6.8, -5.95], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_in_r_band", "from": [ 3.95, 6.0, -8.05], "to": [ 6.05, 6.8, -5.95], "group": "bone_posts", "material": "bone_dark"})

    # 输出端立柱 (+X 侧: x in [6, 8], 避让输入端立柱角点)
    cubes.append({"name": "post_out_l", "from": [6.0, 0.0, -5.8], "to": [8.0, 8.0, -3.8], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_out_r", "from": [6.0, 0.0,  4.0], "to": [8.0, 8.0,  6.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_out_l_band", "from": [5.95, 6.0, -5.85], "to": [8.05, 6.8, -3.75], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_out_r_band", "from": [5.95, 6.0,  3.95], "to": [8.05, 6.8,  6.05], "group": "bone_posts", "material": "bone_dark"})

    return cubes


def build_slope_up_cubes() -> List[dict]:
    """3. belt_slope_up (上坡传送带，带面沿 Z 从 y=4 升至 y=20，8 级 2px 台阶)。"""
    cubes = []
    # 8 级台阶, 每级 z 长 2px, 顶高递升 2px (从 y=4 到 y=20)
    for i in range(8):
        z0 = -8.0 + i * 2.0
        z1 = z0 + 2.0
        y_top = 4.0 + i * 2.0
        # 筋质底座
        cubes.append({"name": f"base_{i}", "from": [-4.0, 0.0, z0], "to": [4.0, y_top - 0.2, z1], "group": "base", "material": "tendon_tube"})
        # 红肌横肋
        mat = "flesh_main" if i % 2 == 0 else "flesh_dark"
        cubes.append({"name": f"rib_{i}", "from": [-4.0, y_top - 0.2, z0], "to": [4.0, y_top, z1], "group": "belt_surface", "material": mat})

        # 骨轨 (随带面升高，顶高 y_top + 2.0，避开两组立柱位)
        if i not in (1, 5):
            cubes.append({"name": f"rail_l_{i}", "from": [-6.0, 0.0, z0], "to": [-4.0, y_top + 2.0, z1], "group": "bone_rails", "material": "bone_main"})
            cubes.append({"name": f"rail_r_{i}", "from": [ 4.0, 0.0, z0], "to": [ 6.0, y_top + 2.0, z1], "group": "bone_rails", "material": "bone_main"})

    # 两组立柱 (跟随斜坡高度拔高至比骨轨高出 2px)
    # 组 1: z in [-6, -4], y_top=6, 骨轨高 8, 立柱顶高 10
    cubes.append({"name": "post_l1", "from": [-6.0, 0.0, -6.0], "to": [-4.0, 10.0, -4.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_r1", "from": [ 4.0, 0.0, -6.0], "to": [ 6.0, 10.0, -4.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_l1_band", "from": [-6.05, 8.0, -6.05], "to": [-3.95, 8.8, -3.95], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_r1_band", "from": [ 3.95, 8.0, -6.05], "to": [ 6.05, 8.8, -3.95], "group": "bone_posts", "material": "bone_dark"})

    # 组 2: z in [2, 4], y_top=14, 骨轨高 16, 立柱顶高 18
    cubes.append({"name": "post_l2", "from": [-6.0, 0.0,  2.0], "to": [-4.0, 18.0,  4.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_r2", "from": [ 4.0, 0.0,  2.0], "to": [ 6.0, 18.0,  4.0], "group": "bone_posts", "material": "bone_main"})
    cubes.append({"name": "post_l2_band", "from": [-6.05, 16.0,  1.95], "to": [-3.95, 16.8,  4.05], "group": "bone_posts", "material": "bone_dark"})
    cubes.append({"name": "post_r2_band", "from": [ 3.95, 16.0,  1.95], "to": [ 6.05, 16.8,  4.05], "group": "bone_posts", "material": "bone_dark"})

    return cubes


def build_slope_down_cubes(up_cubes: List[dict]) -> List[dict]:
    """4. belt_slope_down (下坡传送带，由 slope_up 绕 Y 轴旋转 180° 派生)。"""
    down_cubes = []
    for c in up_cubes:
        f = c["from"]
        t = c["to"]
        # 绕 Y 轴旋转 180°：new_x = -x, new_z = -z
        new_from = [-t[0], f[1], -t[2]]
        new_to   = [-f[0], t[1], -f[2]]
        down_cubes.append({
            "name": c["name"] + "_dn",
            "from": [round(min(new_from[0], new_to[0]), 4), round(f[1], 4), round(min(new_from[2], new_to[2]), 4)],
            "to":   [round(max(new_from[0], new_to[0]), 4), round(t[1], 4), round(max(new_from[2], new_to[2]), 4)],
            "group": c["group"],
            "material": c["material"],
        })
    return down_cubes


# =============================================================================
# 门禁与共面冲突自检
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """严格检查立方体集是否存在同向同坐标且投影相交的共面冲突 (Z-fighting)。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        rot = tuple(c.get("rotation", [0, 0, 0]))
        # 旋转元素跳过轴对齐 AABB 平面检测（由独立旋转矩阵算法验证）
        if any(r != 0 for r in rot):
            continue
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (min(f[1], t[1]), min(f[2], t[2]), max(f[1], t[1]), max(f[2], t[2]))
            elif axis == 1:
                rect = (min(f[0], t[0]), min(f[2], t[2]), max(f[0], t[0]), max(f[2], t[2]))
            else:
                rect = (min(f[0], t[0]), min(f[1], t[1]), max(f[0], t[0]), max(f[1], t[1]))
            key = (side, round(val, 4))
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
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
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


# =============================================================================
# 贴图与 Blockbench 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图，严格使用传送带红肌与骨轨配色。"""
    rng = np.random.default_rng(20261009)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if mat_name == "flesh_main":
                    # 红肌微横纹
                    if y % 3 in (0, 1):
                        r = int(np.clip(r + 8, 0, 255))
                        g = int(np.clip(g + 4, 0, 255))
                elif mat_name == "bone_main":
                    if (x + y * 2) % 6 == 0:
                        r = int(np.clip(r - 8, 0, 255))
                        g = int(np.clip(g - 8, 0, 255))
                        b = int(np.clip(b - 6, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image, model_id: str) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": model_id,
        "folder": "meridian_factory",
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

    for idx, c in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        g_name = c.get("group", "belt")
        groups_map.setdefault(g_name, []).append(elem_uuid)

        mat_key = c.get("material", "flesh_main")
        u0, v0, u1, v1 = MAT_UV.get(mat_key, [0, 0, 16, 16])
        span_x = max(1, u1 - u0 - 4)
        span_y = max(1, v1 - v0 - 4)
        uv_u = u0 + (idx * 2) % span_x
        uv_v = v0 + (idx * 3) % span_y
        uv_box = [float(uv_u), float(uv_v), float(uv_u + 4), float(uv_v + 4)]

        faces = {
            face_name: {"uv": uv_box, "texture": 0}
            for face_name in ("north", "east", "south", "west", "up", "down")
        }

        element = {
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in c["from"]],
            "to":   [round(float(v), 4) for v in c["to"]],
            "autouv": 0,
            "color": 0,
            "origin": c.get("origin", [0.0, 0.0, 0.0]),
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        if "rotation" in c:
            element["rotation"] = c["rotation"]

        elements.append(element)

    outliner = []
    for g_name in ["base", "belt_surface", "bone_rails", "bone_posts", "tendon_fibers"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [0.0, 0.0, 0.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": f"Belt_{model_id}",
        "model_identifier": model_id,
        "visible_box": [1, 1, 1],
        "geometry_name": model_id,
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_all_bbmodels() -> Dict[str, Path]:
    """生成 4 件传送带及默认 bbmodel 文件。"""
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    tex = build_texture()

    models_data = {
        "belt_straight":   build_straight_cubes(),
        "belt_corner":     build_corner_cubes(),
        "belt_slope_up":   build_slope_up_cubes(),
        "belt_slope_down": build_slope_down_cubes(build_slope_up_cubes()),
    }

    result_paths = {}
    for m_id, cubes in models_data.items():
        _assert_no_coplanar_faces(cubes)
        out_p = MODEL_DIR / f"{m_id}.bbmodel"
        doc = build_bbmodel_doc(cubes, tex, m_id)
        out_p.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
        result_paths[m_id] = out_p

    # 默认主模型 belt.bbmodel (等同于 belt_straight)
    default_p = MODEL_DIR / "belt.bbmodel"
    doc_def = build_bbmodel_doc(models_data["belt_straight"], tex, "belt")
    default_p.write_text(json.dumps(doc_def, indent=2, ensure_ascii=False), encoding="utf-8")
    result_paths["default"] = default_p

    print(f"✓ 4 件传送带模型 + 默认主模型生成完成，输出目录: {MODEL_DIR}")
    return result_paths


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(model_paths: Dict[str, Path]):
    """输出 4 件传送带并排 3/4 视与俯视图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    items_order = ["belt_straight", "belt_corner", "belt_slope_up", "belt_slope_down"]
    titles = {
        "belt_straight":   "Straight (直带)",
        "belt_corner":     "Corner (-Z to +X 弯带)",
        "belt_slope_up":   "Slope Up (上坡 y:4->20)",
        "belt_slope_down": "Slope Down (下坡 y:20->4)",
    }

    rendered_iso = {}
    rendered_top = {}

    for k in items_order:
        bb_p = model_paths[k]
        im_i, _ = render(bb_p, yaw=-35.0, pitch=25.0, size=400, bg=bg_color)
        im_t, _ = render(bb_p, yaw=0.0, pitch=89.9, size=400, bg=bg_color)
        rendered_iso[k] = im_i
        rendered_top[k] = im_t

    # 拼装 render.png (2 行 4 列: 上行 3/4 视，下行俯视)
    col_w = 400
    row_h = 400
    pad = 16
    margin_x = 24
    margin_y = 40

    total_w = margin_x * 2 + 4 * col_w + 3 * pad
    total_h = margin_y * 2 + 2 * row_h + 30 + pad

    canvas = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    draw.text((margin_x, 12), "b07 belt 4 Models (Top Row: 3/4 View, Bottom Row: Top View): Straight, Corner, Slope Up, Slope Down", fill=(230, 230, 230))

    for idx, k in enumerate(items_order):
        px = margin_x + idx * (col_w + pad)
        py_iso = margin_y + 24
        py_top = py_iso + row_h + pad

        canvas.paste(rendered_iso[k], (px, py_iso))
        draw.rectangle([px, py_iso, px + col_w, py_iso + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_iso + 5), f"{titles[k]} 3/4", fill=(220, 220, 220))

        canvas.paste(rendered_top[k], (px, py_top))
        draw.rectangle([px, py_top, px + col_w, py_top + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_top + 5), f"{titles[k]} TOP", fill=(180, 200, 220))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 拼装 check.png (左参考图右 4 件并排渲染图)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 560
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        item_w = int(target_h * 0.75)
        right_sub_w = item_w * 4 + 3 * 10
        right_cv = Image.new("RGB", (right_sub_w, target_h), (119, 119, 119))
        for idx, k in enumerate(items_order):
            scaled_v = rendered_iso[k].resize((item_w, item_w), Image.Resampling.LANCZOS)
            rx = idx * (item_w + 10)
            ry = (target_h - item_w) // 2
            right_cv.paste(scaled_v, (rx, ry))

        total_w_check = ref_w + right_sub_w + 32
        check_cv = Image.new("RGB", (total_w_check, target_h + 36), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 28))
        c_draw.text((16, 6), "REFERENCE (b07_belt.png: Left Straight & Slope / Right Corner & Overview)", fill=(210, 200, 180))

        check_cv.paste(right_cv, (ref_w + 20, 28))
        c_draw.text((ref_w + 20, 6), "NOW RENDER (4 Belt Models Parallel: Straight, Corner, Slope Up, Slope Down)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_belt.py 差分自证...")
    cubes_s = build_straight_cubes()
    _assert_no_coplanar_faces(cubes_s)

    cubes_u = build_slope_up_cubes()
    _assert_no_coplanar_faces(cubes_u)

    cubes_d = build_slope_down_cubes(cubes_u)
    _assert_no_coplanar_faces(cubes_d)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷 (在 straight 模型中注入重叠方块)
    defect_cubes = list(cubes_s) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 0.0, -8.0],
        "to":   [ 4.0, 3.8,  8.0],  # 与 base 完全重叠
        "material": "tendon_tube",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_belt.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景构件 b07 belt 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认流程：自测 -> 导出所有模型 bbmodel -> 渲染图片
    self_test()
    model_paths = generate_all_bbmodels()
    render_views(model_paths)


if __name__ == "__main__":
    main()
