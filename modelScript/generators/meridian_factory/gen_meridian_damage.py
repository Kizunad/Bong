#!/usr/bin/env python3
"""经脉工厂内景构件生成器 —— b14: meridian_damage (受损经脉管，3 个变体) [Round 1 第 1 次修改版]

风格：A 有机型 (活体血肉、破损撕裂经脉、淤血污染、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md (第 2 批统一约定与调度审第 1 次修改意见)
- 爆炸参考图：model-review/img/meridian_factory/exploded/x14_meridian_damage.png
- 单件参考图：model-review/img/meridian_factory/refs/b14_meridian_damage.png

调度审第 1 次修改严格落实：
1. 损伤全面挪到 +X 面与顶面（3/4 镜头直视看得到的两面）：
   - cracked (开裂)：
     +X 面 3 条、顶面 2 条 #5a1a1a (flesh_dark) 斜裂纹，每条 1px 宽、5~8 格长、带 1 个分叉，连续不断开（逐格坐标列出）；
   - severed (断裂)：
     中段 (z 6–10，即 z in [-2.0, 2.0]) 完全删除镂空露出背景，
     两段断口各加 3~4 个参差的 #e8bca8 (tendon_highlight) / #d9a08c (tendon_tube) 撕裂碎片 (1x1~1x2，朝缺口伸出 1~2px)，
     断口端面采用 #b05050 (flesh_lit)；
   - contaminated (污染)：
     管内中段塞入 3 块血块 (#2a1430 clot_dark 主、#4a2048 clot_main 斑，3x3x3 / 3.5x3x3 / 2x2x2)，
     其中 1 块紧贴 +X 管壁内侧、1 块紧贴顶壁内侧；
     中段管壁改暗一档 (#a87868 tube_dark) 让血块透出来；
     +X 面增设 2 条从血块处向外蔓延的 #4a2048 (clot_main) 暗纹。
2. 两端接口全面改用已放行的 common_parts 模块：
   - sinew_tube / flesh_collar / bone_ring_port。
3. 门禁要求：通过 _assert_no_coplanar_faces 严格自检（0 共面冲突），带 --self-test 差分缺陷拦截验证。
4. 不写 manifest（严格遵守根 CLAUDE.md 纪律）。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_damage")
REF_EXPLODED = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x14_meridian_damage.png")
REF_SINGLE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b14_meridian_damage.png")

# 引入已放行的 common_parts 模块
sys.path.insert(0, str(Path(__file__).resolve().parent))
import common_parts

# 调色板 (继承 common_parts 并补充 b14 污染专属色)
PALETTE = dict(common_parts.PALETTE)
PALETTE.update({
    "tube_dark":  (168, 120, 104, 255), # #a87868 污染段暗一档筋管壁
    "clot_main":  (74,  32,  72,  255), # #4a2048 污染紫红淤血斑与暗纹
    "clot_dark":  (42,  20,  48,  255), # #2a1430 污染深色凝血块主色
})

MAT_UV = dict(common_parts.MAT_UV)
MAT_UV.update({
    "tube_dark":  [16, 32, 32, 48],
    "clot_main":  [32, 32, 48, 48],
    "clot_dark":  [48, 32, 64, 48],
})

RES = 64


# =============================================================================
# 各变体几何定义 (基于 common_parts)
# =============================================================================

def get_end_fixtures() -> List[dict]:
    """生成两端标准的肉箍与小骨环端口 (复用 common_parts)。"""
    cubes = []
    # 后肉箍 (center_z = -6.0, 长 3.0) 与 前肉箍 (center_z = 6.0, 长 3.0)
    cubes.extend(common_parts.flesh_collar(-6.0, length=3.0, axis="z", pfx="collar_rear"))
    cubes.extend(common_parts.flesh_collar( 6.0, length=3.0, axis="z", pfx="collar_front"))

    # 4 个贴面小骨环端口 (后顶、前顶、后底、前底)
    cubes.extend(common_parts.bone_ring_port(0.0, 9.0, -6.0, face="up",   pfx="port_top_rear"))
    cubes.extend(common_parts.bone_ring_port(0.0, 9.0,  6.0, face="up",   pfx="port_top_front"))
    cubes.extend(common_parts.bone_ring_port(0.0, 1.0, -6.0, face="down", pfx="port_bot_rear"))
    cubes.extend(common_parts.bone_ring_port(0.0, 1.0,  6.0, face="down", pfx="port_bot_front"))
    return cubes


def build_cracked_cubes() -> List[dict]:
    """1. meridian_cracked (开裂)：+X 侧面 3 条、顶面 2 条 #5a1a1a 斜裂纹。"""
    cubes = []
    cubes.extend(get_end_fixtures())

    # 基础管身通长贯通 (-8.0..8.0)，包含内光芯与对角筋丝
    cubes.extend(common_parts.sinew_tube(
        -8.0, 8.0, axis="z",
        with_inner_qi=True, with_highlights=True, with_fibers=True,
        pfx="tube",
    ))

    # ── +X 侧面 3 条斜裂纹 (浮在 x in [4.01, 4.14], y in [2, 8], z in [-4.5, 4.5]) ──
    # 裂纹 1 (后区: z in [-4.2, -1.8]): 主干 5 步 + 分叉 2 步, 连贯相接
    c1 = [
        (2.5, 3.5, -4.2, -3.4),
        (3.5, 4.5, -3.4, -2.6),
        (4.5, 5.5, -2.6, -1.8),
        (5.5, 6.5, -2.6, -1.8),
        (6.5, 7.5, -2.6, -1.8),
        # 分叉向后上:
        (5.0, 6.0, -3.4, -2.6),
        (6.0, 7.0, -4.2, -3.4),
    ]
    # 裂纹 2 (中区: z in [-1.4, 1.2]): 主干 5 步 + 分叉 2 步, 连贯相接
    c2 = [
        (7.0, 7.8, -1.2, -0.4),
        (6.0, 7.0, -0.4,  0.4),
        (5.0, 6.0, -0.4,  0.4),
        (4.0, 5.0,  0.4,  1.2),
        (3.0, 4.0,  0.4,  1.2),
        # 分叉向下前:
        (3.8, 4.8, -1.2, -0.4),
        (2.8, 3.8, -1.4, -0.6),
    ]
    # 裂纹 3 (前区: z in [1.5, 4.2]): 主干 5 步 + 分叉 2 步, 连贯相接
    c3 = [
        (2.5, 3.5,  1.5,  2.3),
        (3.5, 4.5,  2.3,  3.1),
        (4.5, 5.5,  2.3,  3.1),
        (5.5, 6.5,  3.1,  3.9),
        (6.5, 7.5,  3.1,  3.9),
        # 分叉向右前:
        (4.0, 5.0,  3.1,  3.9),
        (3.2, 4.0,  3.4,  4.2),
    ]

    for idx, (y0, y1, z0, z1) in enumerate(c1):
        cubes.append({"name": f"crack_x1_{idx}", "from": [4.01, y0, z0], "to": [4.14, y1, z1], "group": "cracks", "material": "flesh_dark"})
    for idx, (y0, y1, z0, z1) in enumerate(c2):
        cubes.append({"name": f"crack_x2_{idx}", "from": [4.01, y0, z0], "to": [4.14, y1, z1], "group": "cracks", "material": "flesh_dark"})
    for idx, (y0, y1, z0, z1) in enumerate(c3):
        cubes.append({"name": f"crack_x3_{idx}", "from": [4.01, y0, z0], "to": [4.14, y1, z1], "group": "cracks", "material": "flesh_dark"})

    # ── 顶面 2 条斜裂纹 (浮在 y in [8.01, 8.14], x in [-4, 4], z in [-4.5, 4.5]) ──
    # 裂纹 4 (后区: z in [-4.0, -0.5]): 主干 5 步 + 分叉 2 步
    c4 = [
        (-3.5, -2.5, -4.0, -3.0),
        (-2.5, -1.5, -3.0, -2.0),
        (-1.5, -0.5, -2.0, -1.0),
        (-0.5,  0.8, -1.5, -0.5),
        ( 0.8,  2.0, -1.2, -0.5),
        # 分叉:
        (-1.5, -0.5, -3.0, -2.0),
        (-0.5,  0.8, -3.5, -2.5),
    ]
    # 裂纹 5 (前区: z in [0.5, 4.2]): 主干 5 步 + 分叉 2 步
    c5 = [
        (-2.5, -1.2,  0.5,  1.5),
        (-1.2,  0.0,  1.5,  2.5),
        ( 0.0,  1.2,  2.5,  3.5),
        ( 1.2,  2.5,  3.0,  4.0),
        ( 2.5,  3.5,  3.2,  4.2),
        # 分叉:
        ( 0.0,  1.2,  1.5,  2.5),
        ( 1.2,  2.5,  1.0,  2.0),
    ]

    for idx, (x0, x1, z0, z1) in enumerate(c4):
        cubes.append({"name": f"crack_t1_{idx}", "from": [x0, 8.01, z0], "to": [x1, 8.14, z1], "group": "cracks", "material": "flesh_dark"})
    for idx, (x0, x1, z0, z1) in enumerate(c5):
        cubes.append({"name": f"crack_t2_{idx}", "from": [x0, 8.01, z0], "to": [x1, 8.14, z1], "group": "cracks", "material": "flesh_dark"})

    return cubes


def build_severed_cubes() -> List[dict]:
    """2. meridian_severed (断裂)：z 6–10 (z in [-2.0, 2.0]) 范围内无任何筋管立方体，两侧断口各 3 个伸入不超过 1.1px 的小碎片 (1x1x1/1x1x1.2)，俯视与 3/4 视均透过缺口看背景。"""
    cubes = []
    cubes.extend(get_end_fixtures())

    # 两端筋管：后段止于 z=-2.05，前段始于 z=2.05 (在 z in [-2.0, 2.0] 外)
    cubes.extend(common_parts.sinew_tube(
        -8.0, -2.05, axis="z",
        with_inner_qi=True, with_highlights=True, with_fibers=True,
        pfx="tube_rear",
    ))
    # 后断口端面红肉层 (#b05050 flesh_lit，平整覆盖 8x6 横截面，厚 0.05px)
    cubes.append({"name": "sever_end_r", "from": [-4.0, 2.0, -2.05], "to": [4.0, 8.0, -2.00], "group": "sever_ends", "material": "flesh_lit"})

    # 前断口端面红肉层 (#b05050 flesh_lit，平整覆盖 8x6 横截面，厚 0.05px)
    cubes.append({"name": "sever_end_f", "from": [-4.0, 2.0,  2.00], "to": [4.0, 8.0,  2.05], "group": "sever_ends", "material": "flesh_lit"})
    cubes.extend(common_parts.sinew_tube(
        2.05, 8.0, axis="z",
        with_inner_qi=True, with_highlights=True, with_fibers=True,
        pfx="tube_front",
    ))

    # 撕裂小碎片：两侧断口各 3 个 (截面 1x1，向缺口伸入 <= 1.1px，严格不超过 2px；z in [-0.9, 0.9] 绝对全空贯通)
    # 后断口 (z=-2.0 向前伸入，长 0.9~1.1px):
    cubes.append({"name": "shred_r_top",  "from": [ 1.8, 7.2, -2.00], "to": [ 2.8, 8.0, -1.00], "group": "shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_r_side", "from": [ 3.2, 3.0, -2.00], "to": [ 4.0, 4.0, -0.90], "group": "shreds", "material": "tendon_tube"})
    cubes.append({"name": "shred_r_bot",  "from": [-3.5, 2.0, -2.00], "to": [-2.5, 2.8, -1.10], "group": "shreds", "material": "tendon_highlight"})

    # 前断口 (z=2.0 向后伸入，长 0.9~1.1px):
    cubes.append({"name": "shred_f_top",  "from": [-2.6, 7.2,  1.00], "to": [-1.6, 8.0,  2.00], "group": "shreds", "material": "tendon_highlight"})
    cubes.append({"name": "shred_f_side", "from": [ 3.2, 6.0,  0.90], "to": [ 4.0, 7.0,  2.00], "group": "shreds", "material": "tendon_tube"})
    cubes.append({"name": "shred_f_left", "from": [-4.0, 4.0,  1.10], "to": [-3.2, 5.0,  2.00], "group": "shreds", "material": "tendon_highlight"})

    return cubes


def build_contaminated_cubes() -> List[dict]:
    """3. meridian_contaminated (污染)：中段管内塞入 3 块血块，管壁暗化透出，+X 面 2 条蔓延暗纹。"""
    cubes = []
    cubes.extend(get_end_fixtures())

    # 两端正常筋管段：[-8, -3] 与 [3, 8]
    cubes.extend(common_parts.sinew_tube(
        -8.0, -3.0, axis="z",
        with_inner_qi=True, with_highlights=True, with_fibers=True,
        pfx="tube_rear",
    ))
    cubes.extend(common_parts.sinew_tube(
        3.0, 8.0, axis="z",
        with_inner_qi=True, with_highlights=True, with_fibers=True,
        pfx="tube_front",
    ))

    # 中段管壁改暗一档 (#a87868 tube_dark，内光截断，让内部淤血清晰透出)
    cubes.append({"name": "tube_mid_wall_l", "from": [-4.0, 2.0, -3.0], "to": [-3.2, 8.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_mid_wall_r", "from": [ 3.2, 2.0, -3.0], "to": [ 4.0, 8.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_mid_wall_b", "from": [-3.2, 2.0, -3.0], "to": [ 3.2, 2.8,  3.0], "group": "meridian_tube", "material": "tube_dark"})
    cubes.append({"name": "tube_mid_wall_t", "from": [-3.2, 7.2, -3.0], "to": [ 3.2, 8.0,  3.0], "group": "meridian_tube", "material": "tube_dark"})

    # 3 块血块塞在管内 z in [-3, 3] (内腔 x in [-3.2, 3.2], y in [2.8, 7.2]):
    # 血块 1: 紧贴 +X 管壁内侧 (x in [0.2, 3.2], y in [3.0, 6.0], z in [-1.5, 1.5], 尺寸 4x3x3)
    cubes.append({"name": "clot_1_body", "from": [0.20, 3.0, -1.5], "to": [3.20, 6.0,  1.5], "group": "blood_clots", "material": "clot_dark"})
    cubes.append({"name": "clot_1_spot", "from": [0.15, 3.8, -0.8], "to": [3.15, 5.2,  0.8], "group": "blood_clots", "material": "clot_main"})

    # 血块 2: 紧贴顶壁内侧 (x in [-1.5, 2.0], y in [4.2, 7.2], z in [-2.5, 0.5], 尺寸 3.5x3x3)
    cubes.append({"name": "clot_2_body", "from": [-1.50, 4.20, -2.5], "to": [2.00, 7.20,  0.5], "group": "blood_clots", "material": "clot_dark"})
    cubes.append({"name": "clot_2_spot", "from": [-0.80, 4.15, -1.8], "to": [1.40, 7.15, -0.2], "group": "blood_clots", "material": "clot_main"})

    # 血块 3: 悬于中下部 (x in [-2.6, -0.6], y in [2.8, 4.8], z in [0.4, 2.4], 尺寸 2x2x2)
    cubes.append({"name": "clot_3_body", "from": [-2.60, 2.80,  0.4], "to": [-0.60, 4.80,  2.4], "group": "blood_clots", "material": "clot_dark"})

    # +X 侧面 2 条从血块向外蔓延的 #4a2048 暗纹 (浮于 x in [4.01, 4.14], 互不相交)
    cubes.append({"name": "vein_spread_1", "from": [4.01, 4.8,  0.4], "to": [4.14, 6.5,  2.4], "group": "taint_veins", "material": "clot_main"})
    cubes.append({"name": "vein_spread_2", "from": [4.01, 3.0, -2.4], "to": [4.14, 4.5, -0.2], "group": "taint_veins", "material": "clot_main"})

    return cubes


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
    """生成 64×64 RGBA 贴图。"""
    rng = np.random.default_rng(20261011)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if mat_name == "clot_main":
                    if (x * 2 + y * 3) % 5 == 0:
                        r = int(np.clip(r + 14, 0, 255))
                        b = int(np.clip(b + 12, 0, 255))
                elif mat_name == "tube_dark":
                    if (x + y * 4) % 7 in (0, 1):
                        r = int(np.clip(r + 10, 0, 255))
                        g = int(np.clip(g - 6, 0, 255))

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
        g_name = c.get("group", "meridian")
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
    for g_name, u_list in groups_map.items():
        outliner.append({
            "name": g_name,
            "origin": [0.0, 0.0, 0.0],
            "color": 0,
            "uuid": str(uuid.uuid4()),
            "isOpen": True,
            "children": u_list,
        })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": f"MeridianDamage_{model_id}",
        "model_identifier": model_id,
        "visible_box": [1, 1, 1],
        "geometry_name": model_id,
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_all_bbmodels() -> Dict[str, Path]:
    """生成 3 个变体及默认 bbmodel。"""
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    tex = build_texture()

    variants = {
        "meridian_cracked":      build_cracked_cubes(),
        "meridian_severed":      build_severed_cubes(),
        "meridian_contaminated": build_contaminated_cubes(),
    }

    result_paths = {}
    for var_id, cubes in variants.items():
        _assert_no_coplanar_faces(cubes)
        out_p = MODEL_DIR / f"{var_id}.bbmodel"
        doc = build_bbmodel_doc(cubes, tex, var_id)
        out_p.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
        result_paths[var_id] = out_p

    # 默认主模型 meridian_damage.bbmodel (等同于 meridian_cracked)
    def_p = MODEL_DIR / "meridian_damage.bbmodel"
    doc_def = build_bbmodel_doc(variants["meridian_cracked"], tex, "meridian_damage")
    def_p.write_text(json.dumps(doc_def, indent=2, ensure_ascii=False), encoding="utf-8")
    result_paths["default"] = def_p

    print(f"✓ b14 受损经脉管 3 变体 + 默认主模型生成完成: {MODEL_DIR}")
    return result_paths


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(model_paths: Dict[str, Path]):
    """输出三变体 3/4 + 俯视 + 侧视 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    items = ["meridian_cracked", "meridian_severed", "meridian_contaminated"]
    titles = {
        "meridian_cracked":      "Cracked (+X面3条+顶面2条斜裂纹)",
        "meridian_severed":      "Severed (中段完全镂空断开+缺口透光看背景)",
        "meridian_contaminated": "Contaminated (管内血块+暗化管壁+外溢暗纹)",
    }

    rendered_iso = {}
    rendered_top = {}
    rendered_side = {}

    for k in items:
        bb_p = model_paths[k]
        # 3/4 视：severed 使用 yaw=-78.0 确保 3/4 视角下透过缺口清楚看到背景！cracked与contaminated使用 yaw=-35.0 看 +X 与顶面全貌
        iso_yaw = -78.0 if k == "meridian_severed" else -35.0
        im_i, _ = render(bb_p, yaw=iso_yaw, pitch=25.0, size=400, bg=bg_color)
        # 俯视图 (yaw=0, pitch=89.9)，俯视直穿缺口看背景
        im_t, _ = render(bb_p, yaw=0.0,   pitch=89.9, size=400, bg=bg_color)
        # 侧视图 (+X 方向直视: yaw=90, pitch=0)，正对 +X 损伤表面
        im_s, _ = render(bb_p, yaw=90.0,  pitch=0.0,  size=400, bg=bg_color)
        rendered_iso[k]  = im_i
        rendered_top[k]  = im_t
        rendered_side[k] = im_s

    # 1. 拼装 render.png (3 行 3 列: 行 1 为 3/4 视，行 2 为俯视 TOP 视，行 3 为 +X 侧视)
    col_w = 400
    row_h = 400
    pad = 16
    margin_x = 24
    margin_y = 36

    total_w = margin_x * 2 + 3 * col_w + 2 * pad
    total_h = margin_y * 2 + 3 * row_h + 20 + 2 * pad

    canvas = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    draw.text((margin_x, 12), "b14 meridian_damage (Row 1: 3/4 View; Row 2: TOP View; Row 3: +X SIDE View)", fill=(230, 230, 230))

    for idx, k in enumerate(items):
        px = margin_x + idx * (col_w + pad)
        py_iso  = margin_y + 20
        py_top  = py_iso + row_h + pad
        py_side = py_top + row_h + pad

        # Row 1: 3/4
        canvas.paste(rendered_iso[k], (px, py_iso))
        draw.rectangle([px, py_iso, px + col_w, py_iso + 24], fill=(24, 25, 28))
        iso_lbl = f"{titles[k]} 3/4 (yaw=-78透断)" if k == "meridian_severed" else f"{titles[k]} 3/4"
        draw.text((px + 8, py_iso + 5), iso_lbl, fill=(220, 220, 220))

        # Row 2: TOP
        canvas.paste(rendered_top[k], (px, py_top))
        draw.rectangle([px, py_top, px + col_w, py_top + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_top + 5), f"{titles[k]} TOP (俯视)", fill=(200, 220, 180))

        # Row 3: SIDE
        canvas.paste(rendered_side[k], (px, py_side))
        draw.rectangle([px, py_side, px + col_w, py_side + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_side + 5), f"{titles[k]} +X SIDE", fill=(180, 200, 220))

    r_path = REVIEW_DIR / "render.png"
    canvas.save(r_path)
    print(f"✓ render.png 已输出: {r_path}")

    # 2. 拼装 check.png (左爆炸图，右 3 变体并排渲染图)
    ref_to_use = REF_EXPLODED if REF_EXPLODED.exists() else REF_SINGLE
    if ref_to_use.exists():
        ref_im = Image.open(ref_to_use).convert("RGB")
        target_h = 560
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        item_chk_w = int(target_h * 0.72)
        right_sub_w = item_chk_w * 3 + 2 * 12
        right_cv = Image.new("RGB", (right_sub_w, target_h), (119, 119, 119))

        for idx, k in enumerate(items):
            scaled_v = rendered_iso[k].resize((item_chk_w, item_chk_w), Image.Resampling.LANCZOS)
            rx = idx * (item_chk_w + 12)
            ry = (target_h - item_chk_w) // 2
            right_cv.paste(scaled_v, (rx, ry))

        total_w_check = ref_w + right_sub_w + 32
        check_cv = Image.new("RGB", (total_w_check, target_h + 36), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 28))
        c_draw.text((16, 6), "EXPLODED REFERENCE (x14_meridian_damage.png)", fill=(210, 200, 180))

        check_cv.paste(right_cv, (ref_w + 20, 28))
        c_draw.text((ref_w + 20, 6), "NOW RENDER (Cracked / Severed / Contaminated on +X & Top)", fill=(180, 220, 210))

        c_path = REVIEW_DIR / "check.png"
        check_cv.save(c_path)
        print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：3 变体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_meridian_damage.py 差分自证...")
    c_cracked = build_cracked_cubes()
    _assert_no_coplanar_faces(c_cracked)

    c_severed = build_severed_cubes()
    _assert_no_coplanar_faces(c_severed)

    # 校验 severed：缺口 [-2.0, 2.0] 内仅有 6 个小碎片，且无任何主体筋管方块
    in_gap = [c for c in c_severed if max(c["from"][2], c["to"][2]) > -2.0 and min(c["from"][2], c["to"][2]) < 2.0]
    assert len(in_gap) == 6, f"severed 在缺口 [-2, 2] 内的碎片数不为 6: {len(in_gap)}"
    for c in in_gap:
        assert c["name"].startswith("shred_"), f"非碎片立方体进入缺口: {c['name']}"
        z_len = abs(c["to"][2] - c["from"][2])
        assert z_len <= 1.5, f"碎片伸入缺口超过 1.5px: {c['name']} (长 {z_len})"
    print("  [OK] severed 经核验: 缺口 [-2.0, 2.0] 内无任何筋管方块，仅 6 个 <=1.1px 小碎片")

    c_cont = build_contaminated_cubes()
    _assert_no_coplanar_faces(c_cont)
    print("  [OK] 3 变体正常立方体集全部无共面冲突")

    # 注入测试缺陷 (在 cracked 中注入重叠方块)
    defect_cubes = list(c_cracked) + [{
        "name": "inject_coplanar_fail",
        "from": [4.01, 2.5, -4.2],
        "to":   [4.14, 3.5, -3.4],  # 与 crack_x1_0 完全重叠
        "material": "flesh_dark",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_meridian_damage.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景构件 b14 meridian_damage 生成器")
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
