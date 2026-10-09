#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o03: arm_muscle (上肢肌肉组)

风格：A 有机型 (活体血肉、半透明筋管/肌腱、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

调度审第 1 次修改精确指标落实：
1. 外包围与尺寸：2 宽 × 2 高 × 3 长方块 (严格落在 32×32×48 px 空间内：x in [-16..16], y in [0..32], z in [0..48])，
   长轴为 z 轴，肌腱端在 z = 0，肌腹端在 z = 44..48。
2. 7 束肌纤维按 3-2-2 排列（上 3、中 2、下 2）：
   - 上 3: 束 0 (中上), 束 1 (左上), 束 2 (右上)
   - 中 2: 束 3 (中左), 束 4 (中右)
   - 下 2: 束 5 (下左), 束 6 (下右)
   - 每束由 4 节沿 z 首尾相接的长方块组成：
     - 节 1 (z 30–44): 截面 8×8
     - 节 2 (z 20–30): 截面 7×7
     - 节 3 (z 12–20): 截面 5×5
     - 节 4 (z 8–12): 截面 3×3
     并且每节往中心轴收 1px，让整束在肌腱端完美汇拢为 8×8 截面。
   - 各束末端（z 44 那头）长度错开 0–4px（z 到 44.0..47.5px），外圈束外角切 1px，做出饱满圆钝的肌腹。
3. 高光条与束缝：
   - 每束顶面沿长轴一条 1px 宽 #b05050 高光条，各节立体隆起；
   - 束与束之间的缝隙露出内部衬垫的暗血肉 #5a1a1a。
4. 肌腱汇拢段 (z 8–2)：
   - 截面从 8×8 (z=8) 渐缩收到 6×6 (z=2)，材质 #e8bca8；
   - 表面正中配置一条 1px 宽贯通的 #f6dcc4 亮线。
5. 骨环插口 (z 2–0)：
   - 骨环 #d8ccb0 四面包覆 8×6 统一截面开口（内孔宽 8px × 高 6px、居中 x in [-4, 4]、底边离地 2px y in [2, 8]）；
   - 方口内壁衬亮肉粉 #b05050，中心嵌入真元内光 #f6dcc4。
6. 门禁要求：通过 _assert_no_coplanar_faces 严格自检（0 共面冲突），带 --self-test 差分缺陷拦截验证。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "arm_muscle.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/arm_muscle")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o03_arm_muscle")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o03_arm_muscle.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 束间细缝与暗部
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 红肌纤维主色
    "flesh_lit":     (176, 80, 80, 255),   # #b05050 顶面高光条与插口内衬
    "tendon_trans":  (232, 188, 168, 255), # #e8bca8 肌腱白色/暖粉白
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨环
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨暗面
    "qi_glow":       (246, 220, 196, 255), # #f6dcc4 肌腱亮线与真元内光
}

MAT_UV = {
    "flesh_main":    [0, 0, 16, 16],
    "flesh_lit":     [16, 0, 32, 16],
    "flesh_dark":    [32, 0, 48, 16],
    "tendon_trans":  [48, 0, 64, 16],
    "bone_main":     [0, 16, 16, 32],
    "bone_dark":     [16, 16, 32, 32],
    "qi_glow":       [32, 16, 48, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_coupling_socket() -> List[dict]:
    """1. 骨环 + 8x6 统一截面耦合插口 (z: 0..2)。

    插口开口在 z=0 端面，严格遵循统一接口规范：
    - 内孔宽 8 px × 高 6 px、居中 (x in [-4.0, 4.0])、底边离地 2 px (整件底面 y=0.0，y in [2.0, 8.0])；
    - 外包四面骨环外箍 #d8ccb0；
    - 方孔内壁贴 #b05050 亮肉衬层，中心一块 #f6dcc4 当内光。
    """
    cubes = []
    # ── 骨环四面外框 (四角精准无重叠对接) ──
    # 左右骨柱 (x: [-5.5..-4.0] 与 [4.0..5.5], y: 0.5..9.5)
    cubes.append({"name": "socket_ring_l", "from": [-5.5, 0.5, 0.0], "to": [-4.0, 9.5, 2.0], "group": "coupling_socket", "material": "bone_main"})
    cubes.append({"name": "socket_ring_r", "from": [ 4.0, 0.5, 0.0], "to": [ 5.5, 9.5, 2.0], "group": "coupling_socket", "material": "bone_main"})
    # 顶底横骨梁 (x: [-4.0..4.0], y: 8.0..9.5 与 0.5..2.0)
    cubes.append({"name": "socket_ring_t", "from": [-4.0, 8.0, 0.0], "to": [ 4.0, 9.5, 2.0], "group": "coupling_socket", "material": "bone_main"})
    cubes.append({"name": "socket_ring_b", "from": [-4.0, 0.5, 0.0], "to": [ 4.0, 2.0, 2.0], "group": "coupling_socket", "material": "bone_main"})

    # ── 8x6 内壁衬层 (#b05050) ──
    cubes.append({"name": "socket_lining_l", "from": [-4.0, 2.0, 0.0], "to": [-3.2, 8.0, 1.2], "group": "coupling_socket", "material": "flesh_lit"})
    cubes.append({"name": "socket_lining_r", "from": [ 3.2, 2.0, 0.0], "to": [ 4.0, 8.0, 1.2], "group": "coupling_socket", "material": "flesh_lit"})
    cubes.append({"name": "socket_lining_t", "from": [-3.2, 7.2, 0.0], "to": [ 3.2, 8.0, 1.2], "group": "coupling_socket", "material": "flesh_lit"})
    cubes.append({"name": "socket_lining_b", "from": [-3.2, 2.0, 0.0], "to": [ 3.2, 2.8, 1.2], "group": "coupling_socket", "material": "flesh_lit"})

    # ── 中心真元内光 (#f6dcc4) ──
    cubes.append({"name": "socket_lumen_glow", "from": [-3.2, 2.8, 1.0], "to": [ 3.2, 7.2, 2.0], "group": "coupling_socket", "material": "qi_glow"})

    return cubes


def part_02_tendon_core() -> List[dict]:
    """2. 白色肌腱 (z: 2..8, 截面从 8x8 收到 6x6, #e8bca8, 中间一条 #f6dcc4 亮线)。"""
    cubes = []
    # 段 1: z 2..5 截面 6x6 (x: -3.0..3.0, y: 2.0..8.0)
    cubes.append({"name": "tendon_seg_1", "from": [-3.0, 2.0, 2.0], "to": [3.0, 8.0, 5.0], "group": "tendon_core", "material": "tendon_trans"})
    # 段 2: z 5..8 截面渐扩至 8x8 (x: -4.0..4.0, y: 2.0..10.0)
    cubes.append({"name": "tendon_seg_2", "from": [-4.0, 2.0, 5.0], "to": [4.0, 10.0, 8.0], "group": "tendon_core", "material": "tendon_trans"})

    # 中间一条 #f6dcc4 贯通亮线 (宽 0.8px, 浮起 0.3px)
    cubes.append({"name": "tendon_hl_1", "from": [-0.4, 8.05, 2.0], "to": [0.4, 8.35, 5.0], "group": "tendon_core", "material": "qi_glow"})
    cubes.append({"name": "tendon_hl_2", "from": [-0.4, 10.05, 5.0], "to": [0.4, 10.35, 7.9], "group": "tendon_core", "material": "qi_glow"})

    return cubes


def part_03_muscle_bundles() -> List[dict]:
    """3. 7 束肌纤维按 3-2-2 排列（上 3、中 2、下 2）。

    每束由 4 节沿 z 首尾相接的长方块组成：
    - z 44–30 截面 8×8 (节 1)
    - z 30–20 截面 7×7 (节 2)
    - z 20–12 截面 5×5 (节 3)
    - z 12–8 截面 3×3 (节 4)
    并且每节往中心轴收 1px，让整束在肌腱端 (z=8) 汇拢为 8×8 截面。
    各束末端（z 44 那头）长度错开 0–4px，外圈束的外角切 1px 做圆钝肌腹。
    每束顶面沿长轴一条 1px 宽 #b05050 高光条，束与束之间露出 #5a1a1a 细缝。
    各束节间 Z 切面微幅错开，彻底避免同向同坐标共面。
    """
    cubes = []

    bundle_configs = [
        # 束 0: 上中 (Top-Center)
        {
            "name": "b0",
            "z": [8.0, 12.0, 20.0, 30.0, 44.0],
            "ext": 3.0,
            "secs": [
                (-1.5, 1.5, 7.1, 10.1),    # 3x3
                (-2.5, 2.5, 10.2, 15.2),   # 5x5
                (-3.5, 3.5, 13.2, 20.2),   # 7x7
                (-4.0, 4.0, 15.6, 23.6),   # 8x8
            ]
        },
        # 束 1: 上左 (Top-Left)
        {
            "name": "b1",
            "z": [8.2, 12.3, 20.3, 30.3, 44.2],
            "ext": 1.5,
            "secs": [
                (-4.3, -1.3, 6.3, 9.3),
                (-6.8, -1.8, 8.8, 13.8),
                (-9.7, -2.7, 11.4, 18.4),
                (-11.8, -3.8, 13.7, 21.7),
            ]
        },
        # 束 2: 上右 (Top-Right)
        {
            "name": "b2",
            "z": [7.8, 11.7, 19.7, 29.7, 43.8],
            "ext": 2.0,
            "secs": [
                ( 1.3, 4.3, 6.3, 9.3),
                ( 1.8, 6.8, 8.8, 13.8),
                ( 2.7, 9.7, 11.4, 18.4),
                ( 3.8, 11.8, 13.7, 21.7),
            ]
        },
        # 束 3: 中左 (Mid-Left)
        {
            "name": "b3",
            "z": [8.3, 12.5, 20.5, 30.5, 43.9],
            "ext": 0.0,
            "secs": [
                (-4.2, -1.2, 3.4, 6.4),
                (-8.15, -3.2, 5.2, 10.2),
                (-12.3, -5.3, 6.2, 13.2),
                (-15.2, -7.2, 7.2, 15.2),
            ]
        },
        # 束 4: 中右 (Mid-Right)
        {
            "name": "b4",
            "z": [7.7, 11.5, 19.5, 29.5, 44.1],
            "ext": 0.5,
            "secs": [
                ( 1.2, 4.2, 3.4, 6.4),
                ( 3.2, 8.15, 5.2, 10.2),
                ( 5.3, 12.3, 6.2, 13.2),
                ( 7.2, 15.2, 7.2, 15.2),
            ]
        },
        # 束 5: 下左 (Bottom-Left)
        {
            "name": "b5",
            "z": [8.1, 12.2, 20.2, 30.2, 44.3],
            "ext": 3.5,
            "secs": [
                (-3.2, -0.2, 1.6, 4.6),
                (-5.7, -0.7, 1.6, 6.6),
                (-8.25, -1.2, 1.2, 8.2),
                (-9.7, -1.7, 1.2, 9.2),
            ]
        },
        # 束 6: 下右 (Bottom-Right)
        {
            "name": "b6",
            "z": [7.9, 11.8, 19.8, 29.8, 43.7],
            "ext": 2.5,
            "secs": [
                ( 0.2, 3.2, 1.6, 4.6),
                ( 0.7, 5.7, 1.6, 6.6),
                ( 1.2, 8.25, 1.2, 8.2),
                ( 1.7, 9.7, 1.2, 9.2),
            ]
        },
    ]

    for b in bundle_configs:
        b_name = b["name"]
        zs = b["z"]
        secs = b["secs"]
        ext = b["ext"]

        # 4 节
        for s_idx in range(4):
            x0, x1, y0, y1 = secs[s_idx]
            z0 = zs[s_idx]
            z1 = zs[s_idx + 1]
            cubes.append({
                "name": f"{b_name}_s{4-s_idx}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "muscle_bundles",
                "material": "flesh_main",
            })
            # 顶面沿长轴 1px 宽 #b05050 高光条 (浮起 0.25px)
            x_mid = (x0 + x1) / 2.0
            cubes.append({
                "name": f"{b_name}_s{4-s_idx}_hl",
                "from": [x_mid - 0.45, y1 + 0.05, z0],
                "to":   [x_mid + 0.45, y1 + 0.35, z1],
                "group": "muscle_bundles",
                "material": "flesh_lit",
            })

        # 延伸段 (z 44 那头长度错开 0-4px，两级阶梯切角做饱满圆钝的半球肌腹)
        ext_len = ext if ext > 0.05 else 1.2
        x0, x1, y0, y1 = secs[3]
        z_end_s1 = zs[4]
        dx_sign = -1.0 if (x0 + x1) < 0 else 1.0

        if b_name != "b0":
            x0_c1 = x0 + 1.2 if dx_sign < 0 else x0 + 0.3
            x1_c1 = x1 - 0.3 if dx_sign < 0 else x1 - 1.2
            y0_c1, y1_c1 = y0 + 0.6, y1 - 0.6
        else:
            x0_c1, x1_c1 = x0 + 0.6, x1 - 0.6
            y0_c1, y1_c1 = y0 + 0.6, y1 - 0.6

        z_m1 = z_end_s1 + ext_len * 0.6
        cubes.append({
            "name": f"{b_name}_ext1",
            "from": [round(x0_c1, 2), round(y0_c1, 2), round(z_end_s1, 2)],
            "to":   [round(x1_c1, 2), round(y1_c1, 2), round(z_m1, 2)],
            "group": "muscle_bundles",
            "material": "flesh_main",
        })
        x_mid_c1 = (x0_c1 + x1_c1) / 2.0
        cubes.append({
            "name": f"{b_name}_ext1_hl",
            "from": [x_mid_c1 - 0.45, y1_c1 + 0.05, round(z_end_s1, 2)],
            "to":   [x_mid_c1 + 0.45, y1_c1 + 0.35, round(z_m1, 2)],
            "group": "muscle_bundles",
            "material": "flesh_lit",
        })

        # 级 2 进一步收缩圆钝顶帽
        x0_c2, x1_c2 = x0_c1 + 0.8, x1_c1 - 0.8
        y0_c2, y1_c2 = y0_c1 + 0.5, y1_c1 - 0.5
        z_m2 = z_end_s1 + ext_len
        cubes.append({
            "name": f"{b_name}_ext2_dome",
            "from": [round(x0_c2, 2), round(y0_c2, 2), round(z_m1, 2)],
            "to":   [round(x1_c2, 2), round(y1_c2, 2), round(z_m2, 2)],
            "group": "muscle_bundles",
            "material": "flesh_main",
        })

    # 束缝底衬暗色芯 (#5a1a1a)
    cubes.append({"name": "fissure_core_mid", "from": [-2.0, 7.0, 13.0], "to": [2.0, 11.0, 29.0], "group": "muscle_bundles", "material": "flesh_dark"})
    cubes.append({"name": "fissure_core_fat", "from": [-3.0, 9.0, 31.0], "to": [3.0, 15.0, 42.0], "group": "muscle_bundles", "material": "flesh_dark"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总上肢肌肉组全部 3 大部件立方体。"""
    return (
        part_01_coupling_socket()
        + part_02_tendon_core()
        + part_03_muscle_bundles()
    )


# =============================================================================
# 门禁与共面冲突自检
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """严格检查立方体集是否存在同向同坐标且投影相交的共面冲突 (Z-fighting)。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
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
    """生成 64×64 RGBA 贴图，严格使用有机型与红肌纤维配色。"""
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
                    # 肌纤维纵向丝缕纹理
                    if x % 4 in (0, 1):
                        r = int(np.clip(r + 8, 0, 255))
                        g = int(np.clip(g + 4, 0, 255))
                elif mat_name == "tendon_trans":
                    if (x + y) % 5 == 0:
                        r = int(np.clip(r - 6, 0, 255))
                        g = int(np.clip(g - 6, 0, 255))
                        b = int(np.clip(b - 4, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "arm_muscle",
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
        g_name = c.get("group", "muscle")
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
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        elements.append(element)

    outliner = []
    for g_name in ["coupling_socket", "tendon_core", "muscle_bundles"]:
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
        "name": "ArmMuscle",
        "model_identifier": "arm_muscle",
        "visible_box": [2, 2, 3],
        "geometry_name": "arm_muscle",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_bbmodel(out_path: Path = BBMODEL_OUT) -> Path:
    """执行标准生成流程并落盘 bbmodel。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    tex = build_texture()
    doc = build_bbmodel_doc(cubes, tex)

    out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    rel = out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path
    print(f"✓ 上肢肌肉组 bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 渲染四视角 (插口直视、长轴侧视、3/4 等轴、俯视)
    # yaw=180: 从 -Z 看向 +Z，直视 z=0 处的 8x6 骨环耦合插口
    im_socket, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=500, bg=bg_color)
    # yaw=90: 从 +X 看向 -X，左侧为红肌腹、右侧为肌腱插口，与参考图左视完全对齐
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # yaw=145, pitch=25: 从 (+X, +Y, -Z) 俯瞰，左前方为白色肌腱插口，右后方为延伸肌腹，与参考图右视 3/4 完全对齐
    im_iso, _ = render(bbmodel_path, yaw=145.0, pitch=25.0, size=500, bg=bg_color)
    # yaw=90, pitch=89.9: 俯视长轴
    im_top, _ = render(bbmodel_path, yaw=90.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格)
    canvas = Image.new("RGB", (1040, 1040), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("SOCKET (Bone Ring & 8x6 Port at z=0)", im_socket, 20, 20),
        ("SIDE (Full 48px Profile: Left Sac, Right Tendon)", im_side, 540, 20),
        ("3/4 ISOMETRIC (3-2-2 Bundles Converging)", im_iso, 20, 540),
        ("TOP (Longitudinal Convergence)",       im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 360, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 3. 拼装 check.png (左参考图右渲染正面与 3/4 视并排)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        side_scale_w = int(im_side.width * target_h / im_side.height)
        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        side_scaled = im_side.resize((side_scale_w, target_h), Image.Resampling.LANCZOS)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = side_scale_w + iso_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考
        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (o03_arm_muscle.png: Left SIDE / Right 3/4)", fill=(210, 200, 180))

        # 贴右渲染 (SIDE VIEW + 3/4 ISOMETRIC VIEW 与参考图严格左右对应)
        rx = ref_w + 24
        check_cv.paste(side_scaled, (rx, 30))
        check_cv.paste(iso_scaled, (rx + side_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (SIDE VIEW + 3/4 ISOMETRIC VIEW)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_arm_muscle.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-5.5, 0.5, 0.0],
        "to":   [-4.0, 9.5, 2.0],  # 与 socket_ring_l 完全重叠
        "material": "bone_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_arm_muscle.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o03 arm_muscle 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认流程：自测 -> 导出 bbmodel -> 渲染图片
    self_test()
    bb_path = generate_bbmodel()
    render_views(bb_path)


if __name__ == "__main__":
    main()
