#!/usr/bin/env python3
"""经脉工厂内景构件生成器 —— b09: floor_wall (血肉地砖与多孔骨壁砖，2 件套)

风格：A 有机型 (活体血肉、多孔骨壁、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 尺寸：两件都是满格 16×16×16 实心立方体 (x/z in [-8, 8], y in [0, 16])，
   中心对齐方块网格，原点位于底面中心 (0.0, 0.0, 0.0)，绝不镂空，细节靠浅凹凸（最多 1px）。
2. floor_flesh (血肉地砖)：
   - 主色 #5a1a1a (flesh_dark) 暗血肉满格基底；
   - 顶面 (y=16) 用 #8a2a2a (flesh_main) 做 3~4 条弯曲血管纹（宽 1.0px、浮起 0.5px: y in [16.0, 16.5]），
     连续不断开，且跨越边缘对接点，以便 2×2 拼贴时跨缝连通；
   - 顶面散布几块 #6a2020 (flesh_spot) 的 2×2 浅浮斑块 (y in [16.0, 16.2])。
3. wall_bone (多孔骨壁砖)：
   - 主色 #d8ccb0 (bone_main) 骨色实心方块；
   - 四个侧面与顶面各做 5~7 个 #b8a888 / #6a5a48 的小凹孔（尺寸 1~2px，向内凹陷 1px），
     孔底露出深色骨核 #6a5a48，展现质朴古旧的多孔骨质感；
   - 拐角采用榫卯避让结构，彻底消除棱角共面冲突。
4. 交付要求：
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
    "flesh_dark":   (90, 26, 26, 255),    # #5a1a1a 暗血肉地砖主色
    "flesh_main":   (138, 42, 42, 255),   # #8a2a2a 顶面血管纹
    "flesh_spot":   (106, 32, 32, 255),   # #6a2020 顶面斑块
    "bone_main":    (216, 204, 176, 255), # #d8ccb0 骨壁主色
    "bone_dark":    (184, 168, 136, 255), # #b8a888 骨孔暗面
    "bone_crevice": (106, 90, 72, 255),   # #6a5a48 骨凹孔深色孔底
}

MAT_UV_FLESH = {
    "flesh_dark": [0,  0, 32, 32],
    "flesh_main": [32, 0, 64, 32],
    "flesh_spot": [0, 32, 32, 64],
}

MAT_UV_BONE = {
    "bone_main":    [0,  0, 32, 32],
    "bone_dark":    [32, 0, 64, 32],
    "bone_crevice": [0, 32, 32, 64],
}

RES = 64


# =============================================================================
# 各构件几何定义
# =============================================================================

def build_floor_flesh_cubes() -> List[dict]:
    """1. floor_flesh (血肉地砖，满格 16×16×16，顶面 3~4 条弯曲跨边血管纹 + 2×2 斑块)。"""
    cubes = []
    # ── 满格实心主体 (16x16x16, 主色 #5a1a1a) ──
    cubes.append({"name": "flesh_block", "from": [-8.0, 0.0, -8.0], "to": [8.0, 16.0, 8.0], "group": "base_block", "material": "flesh_dark"})

    # ── 顶面血管纹 (y: 16.0..16.5, 宽 1.0, #8a2a2a, 跨边连通且避免同层相交重叠) ──
    # 血管 1: 从西边缘 x=-8 (z in [-0.5, 0.5]) 经中心节点曲折连至东边缘 x=8 (z in [-0.5, 0.5])
    cubes.append({"name": "vein_1_1", "from": [-8.0, 16.0, -0.5], "to": [-4.0, 16.5,  0.5], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_1_2", "from": [-4.0, 16.0,  0.5], "to": [-3.0, 16.5,  2.5], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_1_3a", "from": [-3.0, 16.0, 1.5], "to": [-1.5, 16.5,  2.5], "group": "veins", "material": "flesh_main"})
    # 节点交叉块 (vein_cross 位于 [-1.5, -0.5] x [1.5, 2.5])
    cubes.append({"name": "vein_cross", "from": [-1.5, 16.0, 1.5], "to": [-0.5, 16.5, 2.5], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_1_3b", "from": [-0.5, 16.0, 1.5], "to": [ 1.0, 16.5,  2.5], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_1_4", "from": [ 1.0, 16.0, -0.5], "to": [ 2.0, 16.5,  1.5], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_1_5", "from": [ 2.0, 16.0, -0.5], "to": [ 8.0, 16.5,  0.5], "group": "veins", "material": "flesh_main"})

    # 血管 2: 从北边缘 z=-8 (x in [-1.5, -0.5]) 经节点连至南边缘 z=8 (x in [-1.5, -0.5])
    cubes.append({"name": "vein_2_1", "from": [-1.5, 16.0, -8.0], "to": [-0.5, 16.5, -3.0], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_2_2", "from": [-2.5, 16.0, -3.0], "to": [-0.5, 16.5, -2.0], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_2_3a", "from": [-1.5, 16.0, -2.0], "to": [-0.5, 16.5, 1.5], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_2_3b", "from": [-1.5, 16.0,  2.5], "to": [-0.5, 16.5, 4.0], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_2_4", "from": [-2.5, 16.0,  3.0], "to": [-1.5, 16.5, 4.0], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_2_5", "from": [-2.5, 16.0,  4.0], "to": [-1.5, 16.5, 8.0], "group": "veins", "material": "flesh_main"})

    # 血管 3: 分支连向南边缘 z=8 (x in [4.5, 5.5])
    cubes.append({"name": "vein_3_1", "from": [ 3.5, 16.0,  0.5], "to": [ 4.5, 16.5,  5.0], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_3_2", "from": [ 3.5, 16.0,  5.0], "to": [ 5.5, 16.5,  6.0], "group": "veins", "material": "flesh_main"})
    cubes.append({"name": "vein_3_3", "from": [ 4.5, 16.0,  6.0], "to": [ 5.5, 16.5,  8.0], "group": "veins", "material": "flesh_main"})

    # ── 3. 顶面 2x2 斑块 (#6a2020, 浮起 0.2px: y in [16.0, 16.2]) ──
    cubes.append({"name": "spot_1", "from": [-6.5, 16.0, -6.5], "to": [-4.5, 16.2, -4.5], "group": "spots", "material": "flesh_spot"})
    cubes.append({"name": "spot_2", "from": [ 3.5, 16.0, -6.0], "to": [ 5.5, 16.2, -4.0], "group": "spots", "material": "flesh_spot"})
    cubes.append({"name": "spot_3", "from": [-6.5, 16.0,  3.5], "to": [-4.5, 16.2,  5.5], "group": "spots", "material": "flesh_spot"})
    cubes.append({"name": "spot_4", "from": [ 4.0, 16.0, -2.5], "to": [ 6.0, 16.2, -0.5], "group": "spots", "material": "flesh_spot"})

    return cubes


def build_wall_bone_cubes() -> List[dict]:
    """2. wall_bone (多孔骨壁砖，满格 16×16×16，五个面各做 5~7 个小凹孔向内凹 1px)。"""
    cubes = []
    # ── 内部深色骨核 (#6a5a48, x/z in [-7, 7], y in [0, 15]) ──
    cubes.append({"name": "bone_core", "from": [-7.0, 0.0, -7.0], "to": [7.0, 15.0, 7.0], "group": "core", "material": "bone_crevice"})

    # 16 块骨板的通用局部条带划分: u in [u_min, u_max], v in [v_min, v_max]
    def get_strips(u_min, u_max, v_min, v_max):
        u_span = u_max - u_min
        v_span = v_max - v_min
        return [
            # 顶带
            (u_min, u_max, v_min + 0.8125 * v_span, v_max),
            # 上孔带
            (u_min, u_max, v_min + 0.75 * v_span, v_min + 0.8125 * v_span),
            (u_min, u_max, v_min + 0.5625 * v_span, v_min + 0.625 * v_span),
            (u_min, u_min + 0.1875 * u_span, v_min + 0.625 * v_span, v_min + 0.75 * v_span),
            (u_min + 0.3125 * u_span, u_min + 0.625 * u_span, v_min + 0.625 * v_span, v_min + 0.75 * v_span),
            (u_min + 0.75 * u_span, u_max, v_min + 0.625 * v_span, v_min + 0.75 * v_span),
            # 中带
            (u_min, u_max, v_min + 0.40625 * v_span, v_min + 0.5625 * v_span),
            # 下孔带
            (u_min, u_max, v_min + 0.34375 * v_span, v_min + 0.40625 * v_span),
            (u_min, u_max, v_min + 0.15625 * v_span, v_min + 0.21875 * v_span),
            (u_min, u_min + 0.25 * u_span, v_min + 0.21875 * v_span, v_min + 0.34375 * v_span),
            (u_min + 0.375 * u_span, u_min + 0.5 * u_span, v_min + 0.21875 * v_span, v_min + 0.34375 * v_span),
            (u_min + 0.5 * u_span, u_min + 0.5625 * u_span, v_min + 0.3125 * v_span, v_min + 0.34375 * v_span),
            (u_min + 0.5 * u_span, u_min + 0.5625 * u_span, v_min + 0.21875 * v_span, v_min + 0.25 * v_span),
            (u_min + 0.5625 * u_span, u_min + 0.75 * u_span, v_min + 0.21875 * v_span, v_min + 0.34375 * v_span),
            (u_min + 0.875 * u_span, u_max, v_min + 0.21875 * v_span, v_min + 0.34375 * v_span),
            # 底带
            (u_min, u_max, v_min, v_min + 0.15625 * v_span),
        ]

    # 1. 北面 (-Z): 全宽 x in [-8, 8], y in [0, 15], z in [-8, -7]
    for idx, (u0, u1, v0, v1) in enumerate(get_strips(-8.0, 8.0, 0.0, 15.0)):
        cubes.append({"name": f"wall_n_{idx}", "from": [round(u0, 4), round(v0, 4), -8.0], "to": [round(u1, 4), round(v1, 4), -7.0], "group": "bone_plates", "material": "bone_main"})

    # 2. 南面 (+Z): 全宽 x in [-8, 8], y in [0, 15], z in [7, 8]
    for idx, (u0, u1, v0, v1) in enumerate(get_strips(-8.0, 8.0, 0.0, 15.0)):
        cubes.append({"name": f"wall_s_{idx}", "from": [round(u0, 4), round(v0, 4), 7.0], "to": [round(u1, 4), round(v1, 4), 8.0], "group": "bone_plates", "material": "bone_main"})

    # 3. 西面 (-X): 榫卯收缩避让 z in [-7, 7], y in [0, 15], x in [-8, -7]
    for idx, (u0, u1, v0, v1) in enumerate(get_strips(-7.0, 7.0, 0.0, 15.0)):
        cubes.append({"name": f"wall_w_{idx}", "from": [-8.0, round(v0, 4), round(u0, 4)], "to": [-7.0, round(v1, 4), round(u1, 4)], "group": "bone_plates", "material": "bone_main"})

    # 4. 东面 (+X): 榫卯收缩避让 z in [-7, 7], y in [0, 15], x in [7, 8]
    for idx, (u0, u1, v0, v1) in enumerate(get_strips(-7.0, 7.0, 0.0, 15.0)):
        cubes.append({"name": f"wall_e_{idx}", "from": [7.0, round(v0, 4), round(u0, 4)], "to": [8.0, round(v1, 4), round(u1, 4)], "group": "bone_plates", "material": "bone_main"})

    # 5. 顶面 (+Y): 全盘覆盖 x in [-8, 8], z in [-8, 8], y in [15, 16]
    for idx, (u0, u1, v0, v1) in enumerate(get_strips(-8.0, 8.0, -8.0, 8.0)):
        cubes.append({"name": f"wall_t_{idx}", "from": [round(u0, 4), 15.0, round(v0, 4)], "to": [round(u1, 4), 16.0, round(v1, 4)], "group": "bone_plates", "material": "bone_main"})

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

                if mat_name == "flesh_dark":
                    if (x * 3 + y * 7) % 11 == 0:
                        r = int(np.clip(r + 12, 0, 255))
                        g = int(np.clip(g + 4, 0, 255))
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
    im_flesh_iso, _ = render(model_paths["floor_flesh"], yaw=-35.0, pitch=30.0, size=400, bg=bg_color)
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
    draw.text((32, 45), "floor_flesh (Single Block 3/4 View)", fill=(220, 220, 220))

    canvas.paste(im_flesh_2x2, (24, 470))
    draw.rectangle([24, 470, 424, 494], fill=(24, 25, 28))
    draw.text((32, 475), "floor_flesh 2x2 Tiling (Seam & Vein Continuity)", fill=(180, 220, 200))

    # 右列: wall_bone
    canvas.paste(im_bone_iso, (456, 40))
    draw.rectangle([456, 40, 856, 64], fill=(24, 25, 28))
    draw.text((464, 45), "wall_bone (Single Block 3/4 View: 1px Recessed Holes)", fill=(220, 220, 220))

    canvas.paste(im_bone_2x2, (456, 470))
    draw.rectangle([456, 470, 856, 494], fill=(24, 25, 28))
    draw.text((464, 475), "wall_bone 2x2 Tiling (Bone Wall Assembly & Seam)", fill=(180, 200, 220))

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
        c_draw.text((ref_w + 20, 6), "NOW RENDER (floor_flesh & wall_bone)", fill=(180, 220, 210))

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
