#!/usr/bin/env python3
"""经脉工厂内景构件生成器 —— b09: floor_wall (血肉地砖与多孔骨壁砖，2 件套)

风格：A 有机型 (活体血肉、骨梁嵌红肉壁砖、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

调度审第 1 次严格修改落实：
1. wall_bone (骨梁格架嵌红肉壁砖，更正说明版)：
   - 满格实心 16×16×16 立方体，绝不镂空；
   - 中心实心血肉核 (尺寸 12×12×14, y in [0, 14], 材质 #8a2a2a flesh_main)，凹底位于外表面向内凹 2px 处 (±6.0)；
   - 外层骨梁格架 (#d8ccb0 bone_main，厚度 2px: 在 ±6.0..±8.0 之间)：
     四根 2×2 骨质角柱通高到 y=16；
     四个侧面各由竖梁、横梁交织，交点加粗，梁走向带 1px 弯折；
     格架之间形成 4 个不规则大凹格 (每个 4~6px 见方，四角切掉 1px 显得圆润)，真实向内凹陷 2px；
   - 凹底露出的血肉上有 2 条 #b05050 弯曲细血管 (浮起 0.35px)；
   - 顶面做成骨梁截面 (全 #d8ccb0 带 #b8a888 斑)。
2. floor_flesh (血肉地砖，调亮与斜向血管纹版)：
   - 主色改 #8a2a2a (flesh_main)，满格实心方块 (y: 0..16)；
   - 表面用 #5a1a1a (flesh_dark) 铺设 4 块大尺寸不规则暗斑 (3~5px 见方，厚度 0.15px)；
   - 血管纹改 #b05050 (flesh_lit)，每条用 1×1 方块斜向逐格走 (每步 dx=±1 或 dz=±1，连续自然弯曲)，
     共 4 条，从西、东、北、南四边缘跨边进出，以便相邻地砖无缝对接；
   - 表面点缀 3 个 #c88a7a (flesh_glow) 的 1×1 亮点。
3. 交付要求：
   - 出 render.png (两件各单块 3/4 视 + 2×2 拼贴接缝预览)；
   - 出 check.png (左参考图，右渲染图等高对标)；
   - 门禁自证全绿 (0 共面冲突，--self-test 拦截注入缺陷)；
   - 点名器 Manifest 0 缺项全绿。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/floor_wall")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/b09_floor_wall")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b09_floor_wall.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 地砖主色与壁砖凹底血肉
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 地砖大块暗斑
    "flesh_lit":     (176, 80, 80, 255),   # #b05050 顶面斜向血管纹与壁砖凹格细血管
    "flesh_glow":    (200, 138, 122, 255), # #c88a7a 地砖 1x1 亮点
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨壁梁架与角柱
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨壁暗面与顶板斑块
}

MAT_UV_FLESH = {
    "flesh_main": [0,  0, 32, 32],
    "flesh_dark": [32, 0, 64, 32],
    "flesh_lit":  [0, 32, 32, 64],
    "flesh_glow": [32, 32, 64, 64],
}

MAT_UV_BONE = {
    "bone_main":  [0,  0, 32, 32],
    "bone_dark":  [32, 0, 64, 32],
    "flesh_main": [0, 32, 32, 64],
    "flesh_lit":  [32, 32, 64, 64],
}

RES = 64


# =============================================================================
# 各构件几何定义
# =============================================================================

def build_floor_flesh_cubes() -> List[dict]:
    """1. floor_flesh (血肉地砖，主色 #8a2a2a，调度指定连续血管网 + 3 块暗斑 + 2 个亮点)。"""
    cubes = []
    # ── 满格实心基底 (主色 #8a2a2a, y in [0, 16]) ──
    cubes.append({"name": "flesh_block", "from": [-8.0, 0.0, -8.0], "to": [8.0, 16.0, 8.0], "group": "base_block", "material": "flesh_main"})

    # ── 调度指定固定路径血管网络 (#b05050, 浮起 0.5px: y in [16.0, 16.5], 加粗至 2px 宽管条) ──
    # 主血管 A: (0,5)(1,5)(2,6)(3,6)(4,7)(5,7)(6,8)(7,8)(8,8)(9,9)(10,9)(11,10)(12,10)(13,11)(14,11)(15,12)
    path_a = [(0, 5), (1, 5), (2, 6), (3, 6), (4, 7), (5, 7), (6, 8), (7, 8), (8, 8), (9, 9), (10, 9), (11, 10), (12, 10), (13, 11), (14, 11), (15, 12)]
    # 支 B (从 A 的 (7,8) 分出向上至北缘 (10,0)): (7,7)(7,6)(8,5)(8,4)(9,3)(9,2)(10,1)(10,0)
    path_b = [(7, 7), (7, 6), (8, 5), (8, 4), (9, 3), (9, 2), (10, 1), (10, 0)]
    # 支 C (从 A 的 (11,10) 分出向下至南缘 (9,15)): (11,11)(11,12)(10,13)(10,14)(9,15)
    path_c = [(11, 11), (11, 12), (10, 13), (10, 14), (9, 15)]
    # 细支 D (从 A 的 (2,6) 分出向西至西缘 (0,10)): (2,7)(1,8)(1,9)(0,10)
    path_d = [(2, 7), (1, 8), (1, 9), (0, 10)]

    w2_cells = set()
    for (x, z) in path_a:
        w2_cells.add((x, z))
        if z - 1 >= 0:
            w2_cells.add((x, z - 1))
    for (x, z) in path_b:
        w2_cells.add((x, z))
        if x - 1 >= 0:
            w2_cells.add((x - 1, z))
    for (x, z) in path_c:
        w2_cells.add((x, z))
        if x + 1 < 16:
            w2_cells.add((x + 1, z))
    for (x, z) in path_d:
        w2_cells.add((x, z))
        if z + 1 < 16:
            w2_cells.add((x, z + 1))

    for idx, (gx, gz) in enumerate(sorted(w2_cells)):
        cubes.append({
            "name": f"vein_cell_{idx}",
            "from": [float(gx - 8), 16.0, float(gz - 8)],
            "to":   [float(gx - 7), 16.5, float(gz - 7)],
            "group": "veins",
            "material": "flesh_lit",
        })

    # ── 3 块大暗斑 (#5a1a1a, y in [16.0, 16.15]): 4x3, 3x4, 3x3 (避开血管空白区) ──
    spots = [
        ("dark_spot_4x3", -7.0, -3.0, -7.0, -4.0),  # x in [1, 5], z in [1, 4] -> 4x3
        ("dark_spot_3x4",  4.0,  7.0, -6.0, -2.0),  # x in [12, 15], z in [2, 6] -> 3x4
        ("dark_spot_3x3", -4.0, -1.0,  3.0,  6.0),  # x in [4, 7], z in [11, 14] -> 3x3
    ]
    for sname, x0, x1, z0, z1 in spots:
        cubes.append({
            "name": sname,
            "from": [x0, 16.0, z0],
            "to":   [x1, 16.15, z1],
            "group": "spots",
            "material": "flesh_dark",
        })

    # ── 2 个 1x1 亮点 (#c88a7a, y in [16.0, 16.25]) ──
    sparkles = [
        ("sparkle_1", -5.0, 2.0),  # 网格 (3, 10)
        ("sparkle_2",  5.0, 0.0),  # 网格 (13, 8)
    ]
    for sp_name, sx, sz in sparkles:
        cubes.append({
            "name": sp_name,
            "from": [sx, 16.0, sz],
            "to":   [sx + 1.0, 16.25, sz + 1.0],
            "group": "sparkles",
            "material": "flesh_glow",
        })

    return cubes


def build_wall_bone_cubes() -> List[dict]:
    """2. wall_bone (骨梁格架嵌红肉壁砖，4 个侧面凹 2px 露血肉 + 细血管，顶面骨梁截面)。"""
    cubes = []
    # ── 1. 中心实心血肉核 (凹底在 +/-6.0, 尺寸 12x12x14, y in [0, 14], 主色 #8a2a2a) ──
    cubes.append({"name": "flesh_core", "from": [-6.0, 0.0, -6.0], "to": [6.0, 14.0, 6.0], "group": "flesh_core", "material": "flesh_main"})

    # ── 2. 四根 2x2 骨质角柱 (通高 y in [0, 16], #d8ccb0, 完美封闭四角并消除侧面棱共面) ──
    cubes.append({"name": "pillar_nw", "from": [-8.0, 0.0, -8.0], "to": [-6.0, 16.0, -6.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": "pillar_ne", "from": [ 6.0, 0.0, -8.0], "to": [ 8.0, 16.0, -6.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": "pillar_sw", "from": [-8.0, 0.0,  6.0], "to": [-6.0, 16.0,  8.0], "group": "bone_frame", "material": "bone_main"})
    cubes.append({"name": "pillar_se", "from": [ 6.0, 0.0,  6.0], "to": [ 8.0, 16.0,  8.0], "group": "bone_frame", "material": "bone_main"})

    # ── 3. 顶盖 (y in [14, 16], x/z in [-6, 6], 全 #d8ccb0 带 #b8a888 斑) ──
    cubes.append({"name": "top_bone_plate", "from": [-6.0, 14.0, -6.0], "to": [6.0, 16.0, 6.0], "group": "top_plate", "material": "bone_main"})
    cubes.append({"name": "top_spot_1", "from": [-4.0, 15.9, -4.0], "to": [-2.0, 16.05, -2.0], "group": "top_plate", "material": "bone_dark"})
    cubes.append({"name": "top_spot_2", "from": [ 1.0, 15.9,  1.0], "to": [ 4.0, 16.05,  3.0], "group": "top_plate", "material": "bone_dark"})

    # ── 4. 四个侧面的骨梁格架 (厚度 2px: 在 ±6.0..±8.0 之间) ──
    def add_side_grid(side_name: str, axis: str, sign: int, bend: float = 1.0):
        # 中间竖梁上段向一侧微弯折 0.5px，下段向相反方向微弯折 0.5px，交点加粗
        u_u0 = -1.5 + bend * 0.5
        u_u1 =  0.5 + bend * 0.5
        u_d0 = -0.5 - bend * 0.5
        u_d1 =  1.5 - bend * 0.5

        raw_parts = [
            ("beam_bot",   -6.0,  6.0,  0.0,  2.5),  # 底横梁
            ("beam_top",   -6.0,  6.0, 12.0, 14.0),  # 顶横梁
            ("beam_mid_l", -6.0, -1.5,  6.5,  8.5),  # 中腰梁左段
            ("beam_mid_r",  1.5,  6.0,  6.5,  8.5),  # 中腰梁右段
            ("beam_knot",  -1.5,  1.5,  6.0,  9.0),  # 中间交叉节点 (加粗)
            ("beam_v_up",  u_u0, u_u1,  9.0, 12.0),  # 上竖梁 (带微弯折)
            ("beam_v_dn",  u_d0, u_d1,  2.5,  6.0),  # 下竖梁 (带微弯折)
            # 4 个凹格四角切角 (补 1x1x2 小块，切掉四角显得圆润):
            ("cor_lu_1", -6.0, -5.0, 11.0, 12.0),
            ("cor_lu_2", -6.0, -5.0,  8.5,  9.5),
            ("cor_ru_1",  5.0,  6.0, 11.0, 12.0),
            ("cor_ru_2",  5.0,  6.0,  8.5,  9.5),
            ("cor_ld_1", -6.0, -5.0,  5.5,  6.5),
            ("cor_ld_2", -6.0, -5.0,  2.5,  3.5),
            ("cor_rd_1",  5.0,  6.0,  5.5,  6.5),
            ("cor_rd_2",  5.0,  6.0,  2.5,  3.5),
        ]

        for pname, u0, u1, v0, v1 in raw_parts:
            if axis == 'z':
                z0 = -8.0 if sign < 0 else 6.0
                z1 = -6.0 if sign < 0 else 8.0
                cubes.append({
                    "name": f"{side_name}_{pname}",
                    "from": [round(u0, 2), round(v0, 2), z0],
                    "to":   [round(u1, 2), round(v1, 2), z1],
                    "group": "bone_frame",
                    "material": "bone_main",
                })
            elif axis == 'x':
                x0 = -8.0 if sign < 0 else 6.0
                x1 = -6.0 if sign < 0 else 8.0
                cubes.append({
                    "name": f"{side_name}_{pname}",
                    "from": [x0, round(v0, 2), round(u0, 2)],
                    "to":   [x1, round(v1, 2), round(u1, 2)],
                    "group": "bone_frame",
                    "material": "bone_main",
                })

        # ── 凹底血肉上的 2 条弯曲细血管 (#b05050, 浮起 0.35px) ──
        vein_steps = [
            # 左上格弯曲血管 (对角相连)
            (-4.2, -3.2,  9.5, 10.5),
            (-3.2, -2.2, 10.5, 11.5),
            # 右下格弯曲血管 (对角相连)
            ( 2.5,  3.5,  3.8,  4.8),
            ( 3.5,  4.5,  4.5,  5.5),
        ]
        for v_idx, (vu0, vu1, vv0, vv1) in enumerate(vein_steps):
            if axis == 'z':
                vz0 = -6.35 if sign < 0 else 6.0
                vz1 = -6.0  if sign < 0 else 6.35
                cubes.append({
                    "name": f"recess_vein_{side_name}_{v_idx}",
                    "from": [vu0, vv0, vz0],
                    "to":   [vu1, vv1, vz1],
                    "group": "recess_veins",
                    "material": "flesh_lit",
                })
            elif axis == 'x':
                vx0 = -6.35 if sign < 0 else 6.0
                vx1 = -6.0  if sign < 0 else 6.35
                cubes.append({
                    "name": f"recess_vein_{side_name}_{v_idx}",
                    "from": [vx0, vv0, vu0],
                    "to":   [vx1, vv1, vu1],
                    "group": "recess_veins",
                    "material": "flesh_lit",
                })

    add_side_grid("wall_n", "z", -1, bend=1.0)
    add_side_grid("wall_s", "z",  1, bend=-1.0)
    add_side_grid("wall_w", "x", -1, bend=-1.0)
    add_side_grid("wall_e", "x",  1, bend=1.0)

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

def build_texture(model_type: str, res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图。"""
    mat_uv = MAT_UV_FLESH if model_type == "floor_flesh" else MAT_UV_BONE
    rng = np.random.default_rng(20261009)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in mat_uv.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if model_type == "floor_flesh" and mat_name == "flesh_main":
                    # 侧面保持纯 #8a2a2a，上沿 1px 设置 #5a1a1a
                    if y == v0 + 15:
                        dark_c = PALETTE["flesh_dark"]
                        r, g, b = dark_c[0], dark_c[1], dark_c[2]
                elif mat_name == "bone_main":
                    if (x + y * 3) % 9 in (0, 1):
                        r = int(np.clip(r - 8, 0, 255))
                        g = int(np.clip(g - 8, 0, 255))
                        b = int(np.clip(b - 6, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image, model_id: str, mat_uv: dict) -> dict:
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
        g_name = c.get("group", "block")
        groups_map.setdefault(g_name, []).append(elem_uuid)

        mat_key = c.get("material", "base")
        u0, v0, u1, v1 = mat_uv.get(mat_key, [0, 0, 32, 32])
        span_x = max(1, u1 - u0 - 4)
        span_y = max(1, v1 - v0 - 4)
        uv_u = u0 + (idx * 3) % span_x
        uv_v = v0 + (idx * 5) % span_y
        uv_box = [float(uv_u), float(uv_v), float(uv_u + 4), float(uv_v + 4)]

        if c["name"] == "flesh_block":
            # 满格底座：侧面贴满 0..16 呈现纯 #8a2a2a + 顶端 1px #5a1a1a 上沿；顶底面贴 0..15 纯红
            faces = {
                "north": {"uv": [0.0, 0.0, 16.0, 16.0], "texture": 0},
                "east":  {"uv": [0.0, 0.0, 16.0, 16.0], "texture": 0},
                "south": {"uv": [0.0, 0.0, 16.0, 16.0], "texture": 0},
                "west":  {"uv": [0.0, 0.0, 16.0, 16.0], "texture": 0},
                "up":    {"uv": [0.0, 0.0, 16.0, 15.0], "texture": 0},
                "down":  {"uv": [0.0, 0.0, 16.0, 15.0], "texture": 0},
            }
        else:
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
        "name": f"FloorWall_{model_id}",
        "model_identifier": model_id,
        "visible_box": [1, 1, 1],
        "geometry_name": model_id,
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_all_bbmodels() -> Dict[str, Path]:
    """生成 floor_flesh、wall_bone 及 2x2 拼贴模型。"""
    MODEL_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 单块 floor_flesh
    cubes_f = build_floor_flesh_cubes()
    _assert_no_coplanar_faces(cubes_f)
    tex_f = build_texture("floor_flesh")
    doc_f = build_bbmodel_doc(cubes_f, tex_f, "floor_flesh", MAT_UV_FLESH)
    p_f = MODEL_DIR / "floor_flesh.bbmodel"
    p_f.write_text(json.dumps(doc_f, indent=2, ensure_ascii=False), encoding="utf-8")

    # 2. 单块 wall_bone
    cubes_w = build_wall_bone_cubes()
    _assert_no_coplanar_faces(cubes_w)
    tex_w = build_texture("wall_bone")
    doc_w = build_bbmodel_doc(cubes_w, tex_w, "wall_bone", MAT_UV_BONE)
    p_w = MODEL_DIR / "wall_bone.bbmodel"
    p_w.write_text(json.dumps(doc_w, indent=2, ensure_ascii=False), encoding="utf-8")

    # 3. 默认主文件 floor_wall.bbmodel (等同于 floor_flesh)
    p_def = MODEL_DIR / "floor_wall.bbmodel"
    p_def.write_text(json.dumps(doc_f, indent=2, ensure_ascii=False), encoding="utf-8")

    # 4. 2x2 拼贴模型 (用于真实 3D 渲染看接缝)
    # floor_flesh 2x2 (平铺在 XZ 平面上: [-8, 24] x [-8, 24])
    cubes_f_2x2 = []
    for dx, dz in [(0, 0), (16, 0), (0, 16), (16, 16)]:
        for c in cubes_f:
            f = c["from"]
            t = c["to"]
            cubes_f_2x2.append({
                "name": f"{c['name']}_{dx}_{dz}",
                "from": [f[0] + dx, f[1], f[2] + dz],
                "to":   [t[0] + dx, t[1], t[2] + dz],
                "group": c["group"],
                "material": c["material"],
            })
    doc_f_2x2 = build_bbmodel_doc(cubes_f_2x2, tex_f, "floor_flesh_2x2", MAT_UV_FLESH)
    p_f_2x2 = MODEL_DIR / "floor_flesh_2x2.bbmodel"
    p_f_2x2.write_text(json.dumps(doc_f_2x2, indent=2, ensure_ascii=False), encoding="utf-8")

    # wall_bone 2x2 (平铺在 XY 平面上构成一面高 32px 宽 32px 的骨墙: [-8, 24] x [0, 32])
    cubes_w_2x2 = []
    for dx, dy in [(0, 0), (16, 0), (0, 16), (16, 16)]:
        for c in cubes_w:
            f = c["from"]
            t = c["to"]
            cubes_w_2x2.append({
                "name": f"{c['name']}_{dx}_{dy}",
                "from": [f[0] + dx, f[1] + dy, f[2]],
                "to":   [t[0] + dx, t[1] + dy, t[2]],
                "group": c["group"],
                "material": c["material"],
            })
    doc_w_2x2 = build_bbmodel_doc(cubes_w_2x2, tex_w, "wall_bone_2x2", MAT_UV_BONE)
    p_w_2x2 = MODEL_DIR / "wall_bone_2x2.bbmodel"
    p_w_2x2.write_text(json.dumps(doc_w_2x2, indent=2, ensure_ascii=False), encoding="utf-8")

    print(f"✓ floor_flesh 与 wall_bone 模型及 2x2 拼贴生成完成: {MODEL_DIR}")
    return {
        "floor_flesh": p_f,
        "wall_bone":   p_w,
        "default":     p_def,
        "floor_2x2":   p_f_2x2,
        "wall_2x2":    p_w_2x2,
    }


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(model_paths: Dict[str, Path]):
    """输出两件各 3/4 视 + 2×2 拼贴接缝预览 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 渲染两件单块 3/4 视
    im_flesh_iso, _ = render(model_paths["floor_flesh"], yaw=-35.0, pitch=35.0, size=400, bg=bg_color)
    im_bone_iso, _  = render(model_paths["wall_bone"],   yaw=-35.0, pitch=25.0, size=400, bg=bg_color)

    # 2. 渲染两件 2x2 拼贴接缝预览
    im_flesh_2x2, _ = render(model_paths["floor_2x2"], yaw=-35.0, pitch=45.0, size=400, bg=bg_color)
    im_bone_2x2, _  = render(model_paths["wall_2x2"],  yaw=-35.0, pitch=25.0, size=400, bg=bg_color)

    # 3. 拼装 render.png (2 行 2 列: 左列 floor_flesh 单块+2x2，右列 wall_bone 单块+2x2)
    canvas = Image.new("RGB", (880, 920), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    draw.text((20, 12), "b09 floor_wall (Left Column: floor_flesh 3/4 & 2x2 Tiling; Right Column: wall_bone 3/4 & 2x2 Tiling)", fill=(230, 230, 230))

    # 左列: floor_flesh
    canvas.paste(im_flesh_iso, (24, 40))
    draw.rectangle([24, 40, 424, 64], fill=(24, 25, 28))
    draw.text((32, 45), "floor_flesh (Single Block 3/4 View: #8a2a2a & Veins)", fill=(220, 220, 220))

    canvas.paste(im_flesh_2x2, (24, 470))
    draw.rectangle([24, 470, 424, 494], fill=(24, 25, 28))
    draw.text((32, 475), "floor_flesh 2x2 Tiling (Seam & Diagonal Veins)", fill=(180, 220, 200))

    # 右列: wall_bone
    canvas.paste(im_bone_iso, (456, 40))
    draw.rectangle([456, 40, 856, 64], fill=(24, 25, 28))
    draw.text((464, 45), "wall_bone (Single Block 3/4: Bone Grid & Red Recesses)", fill=(220, 220, 220))

    canvas.paste(im_bone_2x2, (456, 470))
    draw.rectangle([456, 470, 856, 494], fill=(24, 25, 28))
    draw.text((464, 475), "wall_bone 2x2 Tiling (Bone Grid Wall Assembly & Seam)", fill=(180, 200, 220))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 4. 拼装 check.png (左参考图右 2 件渲染等高对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 560
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        item_w = int(target_h * 0.72)
        right_sub_w = item_w * 2 + 16
        right_cv = Image.new("RGB", (right_sub_w, target_h), (119, 119, 119))
        
        scaled_f = im_flesh_iso.resize((item_w, item_w), Image.Resampling.LANCZOS)
        scaled_w = im_bone_iso.resize((item_w, item_w), Image.Resampling.LANCZOS)
        
        ry = (target_h - item_w) // 2
        right_cv.paste(scaled_f, (0, ry))
        right_cv.paste(scaled_w, (item_w + 16, ry))

        total_w_check = ref_w + right_sub_w + 32
        check_cv = Image.new("RGB", (total_w_check, target_h + 36), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 28))
        c_draw.text((16, 6), "REFERENCE (b09_floor_wall.png: Top=wall_bone / Bottom=floor_flesh)", fill=(210, 200, 180))

        check_cv.paste(right_cv, (ref_w + 20, 28))
        c_draw.text((ref_w + 20, 6), "NOW RENDER (floor_flesh & wall_bone Grid)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_floor_wall.py 差分自证...")
    cubes_f = build_floor_flesh_cubes()
    _assert_no_coplanar_faces(cubes_f)

    cubes_w = build_wall_bone_cubes()
    _assert_no_coplanar_faces(cubes_w)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷 (在 floor_flesh 中注入重叠方块)
    defect_cubes = list(cubes_f) + [{
        "name": "inject_coplanar_fail",
        "from": [-8.0, 0.0, -8.0],
        "to":   [ 8.0, 16.0,  8.0],  # 与 flesh_block 完全重叠
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_floor_wall.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景构件 b09 floor_wall 生成器")
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
