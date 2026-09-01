#!/usr/bin/env python3
"""采药刀 / 采药小弯刀 (Herb Sickle / cai_yao_dao) Blockbench .bbmodel 生成器。

定位与世界观设定（worldview §四 / docs/finished_plans/plan-tools-v1.md / tools.toml）：
- 散修与底层采药人自制的极度简陋凡器：
  “凡铁小刀，刃薄而短，只够割根须和药茎；急了也能划人，刃口很快卷。”
- 比凡铁剑/匕首更加破旧、粗糙、手工拼凑：
  1. 碎铁弯刃 (blade) —— 废弃薄铁片粗糙敲弯而成的镰月小刃，带粗磨锯齿刃口与暗红铁锈。
  2. 乱绳紧固 (binding) —— 刃根插在劈开的木柄上，用枯黄草绳与粗麻绳紧密缠绕，垂下碎麻纤维。
  3. 枯枝木柄 (handle) —— 粗糙开裂的野外枯木折枝，保留天然木节起伏与干裂断茬。

分 3 大 Group 骨骼节点：
    1. handle   - 枯木握柄（含粗木柄身、握持粗节与底部碎裂断茬）
    2. binding  - 乱绳草纤维缠绕束（含上下箍圈、粗麻绳缠绕层与垂下草绳端头）
    3. blade    - 碎铁弯月小镰刃（含插入柄中的刀根、阶梯状弯弧刀背、内弧粗磨锋刃与微翘刀尖）

尺寸规范（MC px，16px = 1 格）：
    总长约 18.2px ≈ 1.14 格（刀柄长约 11.0px，缠绳段约 3.5px，弯刃高约 7.2px，向侧方弯出约 5.6px）。
    握把半宽约 0.90px（Ø1.8px），适合玩家手部握持。

贴图规范（64×64 四象限 Atlas）：
    - Q1 (0..32, 0..32): 碎铁锈刃 (scrap_iron: 铁锈红褐色、磨光白刃线、氧化黑斑)
    - Q2 (32..64, 0..32): 粗麻绳与草纤维 (hemp_binding: 枯草黄、麻绳编织纹、阴影缝隙)
    - Q3 (0..32, 32..64): 枯木老树皮 (dry_wood: 风化干灰褐色、开裂深纹、粗糙木节)
    - Q4 (32..64, 32..64): 截面断茬与木芯 (wood_core: 浅黄褐木芯、年轮断裂面)
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import random
import uuid
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
MODEL_DIR = Path(__file__).resolve().parents[1] / "models"
OUT_DIR = Path(__file__).resolve().parents[1] / "out"
BBMODEL_OUT = MODEL_DIR / "HerbSickle.bbmodel"
PREVIEW_OUT = OUT_DIR / "herb_sickle_preview.png"

PX = 16.0
RES = 64

# ── 尺寸与高度分段（y: 握柄底端=0.0，刀刃朝 +Y 偏向 -X 弯曲）───────────────
HANDLE_BUTT_Y = (0.00, 1.20)      # 底部粗糙断茬
HANDLE_LOWER_Y = (1.20, 4.80)     # 握把下段（带一道防滑小草绳）
HANDLE_MID_Y = (4.80, 8.50)       # 主握持区（握把中心约在 y=6.0）
HANDLE_TOP_Y = (8.50, 11.20)      # 插入刃部的木柄上端劈口区

BINDING_LOWER_Y = (2.20, 2.80)    # 下部防滑细绳圈
BINDING_MAIN_Y = (8.20, 11.40)    # 上部主紧固缠绳区（长 3.2px）

# 握把中心（用于玩家握持点对齐）
GRIP_PX = 6.00
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - GRIP_PX, BLOCK_CENTRE_PX)

BONE_ORDER = ["handle", "binding", "blade"]
BONE_COLORS = {
    "handle": (92, 70, 48),       # 枯木
    "binding": (168, 142, 86),    # 麻绳/草绳
    "blade": (132, 105, 92),      # 锈铁刃
}

# 贴图象限规划 (64x64)
MAT_ZONE = {
    "scrap_iron": (0, 0, 32, 32),
    "hemp_binding": (32, 0, 64, 32),
    "dry_wood": (0, 32, 32, 64),
    "wood_core": (32, 32, 64, 64),
}


def build_cubes():
    """构建采药刀的所有体素方块元素，组织进对应 Group。
    返回列表: [(bone_name, mat_key, cube_name, from_xyz, to_xyz, rot_xyz, pivot_xyz)]
    """
    cubes: list[tuple] = []

    def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0), pivot=None):
        fx, tx = min(x0, x1), max(x0, x1)
        fy, ty = min(y0, y1), max(y0, y1)
        fz, tz = min(z0, z1), max(z0, z1)
        if pivot is None:
            p = ((fx + tx) / 2.0, (fy + ty) / 2.0, (fz + tz) / 2.0)
        else:
            p = pivot
        cubes.append((bone, mat, name, [fx, fy, fz], [tx, ty, tz], tuple(rot), tuple(p)))

    def oct_stick(bone, mat, name, r, y0, y1, ox=0.0, oz=0.0):
        """生成八角截面的粗糙树枝木干。"""
        block(bone, mat, f"{name}_c", ox - r, ox + r, y0, y1, oz - r, oz + r)
        d = r * 0.72
        block(bone, mat, f"{name}_diag", ox - d, ox + d, y0, y1, oz - d, oz + d, rot=(0.0, 45.0, 0.0), pivot=(ox, (y0 + y1) / 2.0, oz))

    # ═════════════════════════════════════════════════════════════════════════
    # 1. HANDLE（粗枯枝木柄与断茬）
    # ═════════════════════════════════════════════════════════════════════════
    # 底部粗糙木头断茬（参差不齐）
    block("handle", "wood_core", "handle_butt_core", -0.65, 0.60, 0.00, 0.70, -0.60, 0.60)
    block("handle", "dry_wood", "handle_butt_splinter1", -0.80, -0.20, 0.00, 1.20, -0.70, -0.10)
    block("handle", "dry_wood", "handle_butt_splinter2", 0.10, 0.75, 0.20, 1.40, 0.05, 0.70)

    # 握柄下段
    oct_stick("handle", "dry_wood", "handle_body_lower", 0.80, 0.70, 4.80, ox=-0.05, oz=0.0)
    # 木柄上的一个天然粗糙木节 (Knot)
    block("handle", "dry_wood", "handle_knot_l", 0.65, 1.10, 3.20, 4.10, -0.35, 0.35)

    # 握柄中段（主手握持区，略有握槽曲度）
    oct_stick("handle", "dry_wood", "handle_body_mid", 0.85, 4.80, 8.50, ox=0.0, oz=-0.05)

    # 握柄上端劈开夹刃部
    block("handle", "dry_wood", "handle_body_top_l", 0.15, 0.90, 8.50, 11.20, -0.75, 0.75)
    block("handle", "dry_wood", "handle_body_top_r", -0.90, -0.15, 8.50, 10.90, -0.75, 0.75)
    block("handle", "wood_core", "handle_cleft_inner", -0.20, 0.20, 8.50, 9.80, -0.65, 0.65)

    # ═════════════════════════════════════════════════════════════════════════
    # 2. BINDING（粗麻绳与枯草纤维缠扎）
    # ═════════════════════════════════════════════════════════════════════════
    # 下段防滑细草绳圈
    block("binding", "hemp_binding", "cord_tie_lower1", -0.92, 0.88, 2.20, 2.55, -0.92, 0.88)
    block("binding", "hemp_binding", "cord_tie_lower2", -0.88, 0.92, 2.50, 2.85, -0.88, 0.92, rot=(0.0, 15.0, 0.0), pivot=(0.0, 2.65, 0.0))
    block("binding", "hemp_binding", "cord_tie_knot", 0.75, 1.20, 2.10, 2.70, 0.35, 0.85)

    # 上端主紧固缠绳群（逐圈交错咬合包裹木柄与刀根）
    # 第 1 圈（底层）
    block("binding", "hemp_binding", "binding_wrap_1", -1.10, 1.10, 8.20, 8.90, -1.00, 1.00)
    # 第 2 圈（交叉加固）
    block("binding", "hemp_binding", "binding_wrap_2", -1.15, 1.15, 8.80, 9.50, -1.05, 1.05, rot=(0.0, -8.0, 0.0), pivot=(0.0, 9.15, 0.0))
    # 第 3 圈（中层最厚包裹）
    block("binding", "hemp_binding", "binding_wrap_3", -1.20, 1.20, 9.40, 10.15, -1.10, 1.10, rot=(0.0, 12.0, 0.0), pivot=(0.0, 9.75, 0.0))
    # 第 4 圈（上层收口）
    block("binding", "hemp_binding", "binding_wrap_4", -1.15, 1.15, 10.05, 10.75, -1.05, 1.05, rot=(0.0, -5.0, 0.0), pivot=(0.0, 10.4, 0.0))
    # 第 5 圈（封顶勒紧）
    block("binding", "hemp_binding", "binding_wrap_5", -1.05, 1.05, 10.65, 11.35, -0.95, 0.95)

    # 缠绳垂落的麻纤维毛边与绳结 (Fringes & Ties)
    block("binding", "hemp_binding", "binding_fringe_knot", -1.35, -0.95, 9.20, 10.20, 0.45, 1.05)
    block("binding", "hemp_binding", "binding_fringe_drop1", -1.45, -1.15, 7.60, 9.40, 0.55, 0.90, rot=(0.0, 0.0, 10.0), pivot=(-1.3, 9.4, 0.7))
    block("binding", "hemp_binding", "binding_fringe_drop2", -1.35, -1.05, 6.80, 8.00, 0.45, 0.80, rot=(0.0, 0.0, -12.0), pivot=(-1.2, 8.0, 0.6))
    block("binding", "hemp_binding", "binding_fringe_side", 0.98, 1.38, 9.60, 10.40, -0.85, -0.25)

    # ═════════════════════════════════════════════════════════════════════════
    # 3. BLADE（碎铁弯月小镰刃）—— 经典采药小弯刀
    # ═════════════════════════════════════════════════════════════════════════
    # 刀根（深插在木柄与缠绳内）
    block("blade", "scrap_iron", "blade_tang_root", -0.12, 0.12, 8.80, 11.20, -0.45, 0.45)

    # 弯月形体素阶梯段（刀身向 -X 方向弧度延展，内弧为刃口，外弧为厚刀背）
    # 段 0: 刀颈直立过渡段 (y: 11.0 -> 12.6, x: -0.2 -> 0.2)
    block("blade", "scrap_iron", "blade_arc_neck", -0.22, 0.22, 11.00, 12.60, -0.40, 0.40)
    block("blade", "scrap_iron", "blade_edge_neck", -0.10, 0.10, 11.10, 12.50, -0.65, -0.38)

    # 段 1: 刀身斜升起弧 (y: 12.4 -> 14.4, x: -1.4 -> -0.1)
    block("blade", "scrap_iron", "blade_arc_1", -1.40, -0.10, 12.40, 14.40, -0.36, 0.36)
    block("blade", "scrap_iron", "blade_edge_1", -1.25, -0.05, 12.20, 14.10, -0.58, -0.32)

    # 段 2: 刀身中部弯月腹 (y: 14.0 -> 16.0, x: -2.8 -> -1.2)
    block("blade", "scrap_iron", "blade_arc_2", -2.80, -1.20, 14.00, 16.00, -0.32, 0.32)
    block("blade", "scrap_iron", "blade_edge_2", -2.60, -1.30, 13.70, 15.60, -0.52, -0.28)

    # 段 3: 刀身向刀尖回勾过渡 (y: 15.2 -> 17.2, x: -4.3 -> -2.5)
    block("blade", "scrap_iron", "blade_arc_3", -4.30, -2.50, 15.20, 17.20, -0.28, 0.28)
    block("blade", "scrap_iron", "blade_edge_3", -4.10, -2.60, 14.80, 16.70, -0.45, -0.24)

    # 段 4: 刀尖向内向下弯勾峰顶 (y: 15.8 -> 17.5, x: -5.4 -> -3.8)
    block("blade", "scrap_iron", "blade_tip_hook", -5.30, -3.80, 15.80, 17.40, -0.24, 0.24)
    block("blade", "scrap_iron", "blade_tip_edge", -5.20, -4.00, 15.40, 16.90, -0.38, -0.20)

    # 尖端最末锋尖 (Tip apex) (y: 15.0 -> 16.2, x: -5.6 -> -4.8)
    block("blade", "scrap_iron", "blade_tip_apex", -5.60, -4.80, 15.00, 16.20, -0.18, 0.18)

    # 刀背加强粗棱脊 (Spine ridge)
    block("blade", "scrap_iron", "blade_arc_spine", -3.60, -1.50, 14.60, 16.40, 0.25, 0.45)

    return cubes


def generate_texture_atlas() -> Image.Image:
    """生成 64x64 四象限手绘风格材质贴图：
    - Q1 (0..32, 0..32): scrap_iron (碎铁生锈、斑驳磨损、粗糙暗黑锈斑)
    - Q2 (32..64, 0..32): hemp_binding (粗麻绳、草绳编织与阴影)
    - Q3 (0..32, 32..64): dry_wood (老树枝干木纹、开裂树皮)
    - Q4 (32..64, 32..64): wood_core (截面木芯、风化年轮)
    """
    atlas = Image.new("RGBA", (RES, RES), (0, 0, 0, 255))
    rng = random.Random(0x73A1)

    # ── Q1: 碎铁生锈与磨光刃线 (0, 0, 32, 32) ───────────────────────────────
    for y in range(0, 32):
        for x in range(0, 32):
            noise = rng.uniform(-10, 10)
            # 偏深褐色与铁黑锈色
            base_r = int(np.clip(100 + noise * 1.5, 0, 255))
            base_g = int(np.clip(75 + noise * 1.0, 0, 255))
            base_b = int(np.clip(65 + noise * 0.8, 0, 255))

            # 暗部氧化铁锈斑 (Rust pit)
            if (x * 7 + y * 13) % 11 < 3:
                base_r = int(np.clip(base_r + 28, 0, 255))
                base_g = int(np.clip(base_g - 18, 0, 255))
                base_b = int(np.clip(base_b - 20, 0, 255))
            elif (x + y * 5) % 8 == 0:
                base_r = int(np.clip(base_r - 35, 0, 255))
                base_g = int(np.clip(base_g - 28, 0, 255))
                base_b = int(np.clip(base_b - 25, 0, 255))

            # 刃口手工磨线高光带
            if y > 22:
                base_r = int(np.clip(base_r + 48, 0, 255))
                base_g = int(np.clip(base_g + 52, 0, 255))
                base_b = int(np.clip(base_b + 58, 0, 255))

            atlas.putpixel((x, y), (base_r, base_g, base_b, 255))

    # ── Q2: 粗麻绳与草纤维束 (32, 0, 64, 32) ─────────────────────────────────
    for y in range(0, 32):
        for x in range(32, 64):
            noise = rng.uniform(-8, 8)
            # 枯麻黄色基调
            base_r = int(np.clip(155 + noise * 1.2, 0, 255))
            base_g = int(np.clip(128 + noise * 1.0, 0, 255))
            base_b = int(np.clip(78 + noise * 0.8, 0, 255))

            # 扭绳纤维条纹
            braid = ((x - 32) * 2 + y) % 6
            if braid <= 1:
                base_r = int(np.clip(base_r - 38, 0, 255))
                base_g = int(np.clip(base_g - 32, 0, 255))
                base_b = int(np.clip(base_b - 22, 0, 255))
            elif braid == 3:
                base_r = int(np.clip(base_r + 24, 0, 255))
                base_g = int(np.clip(base_g + 20, 0, 255))
                base_b = int(np.clip(base_b + 12, 0, 255))

            atlas.putpixel((x, y), (base_r, base_g, base_b, 255))

    # ── Q3: 枯木树皮与开裂 (0, 32, 32, 64) ──────────────────────────────────
    for y in range(32, 64):
        for x in range(0, 32):
            noise = rng.uniform(-8, 8)
            # 深灰褐色老树皮
            base_r = int(np.clip(92 + noise * 1.2, 0, 255))
            base_g = int(np.clip(68 + noise * 1.0, 0, 255))
            base_b = int(np.clip(46 + noise * 0.8, 0, 255))

            # 垂直树皮裂纹
            if x in (5, 6, 17, 18, 26) and (y % 5 != 0):
                base_r = int(np.clip(base_r - 32, 0, 255))
                base_g = int(np.clip(base_g - 26, 0, 255))
                base_b = int(np.clip(base_b - 20, 0, 255))
            elif x in (4, 19):
                base_r = int(np.clip(base_r + 18, 0, 255))
                base_g = int(np.clip(base_g + 16, 0, 255))
                base_b = int(np.clip(base_b + 10, 0, 255))

            atlas.putpixel((x, y), (base_r, base_g, base_b, 255))

    # ── Q4: 木芯与截面断茬 (32, 32, 64, 64) ──────────────────────────────────
    for y in range(32, 64):
        for x in range(32, 64):
            dx = (x - 48)
            dy = (y - 48)
            dist = math.hypot(dx, dy)
            noise = rng.uniform(-6, 6)

            ring = int(dist * 1.8) % 2
            base_r = int(np.clip(132 + ring * 16 + noise, 0, 255))
            base_g = int(np.clip(104 + ring * 12 + noise, 0, 255))
            base_b = int(np.clip(72 + ring * 8 + noise, 0, 255))

            if dist > 13:
                base_r = int(np.clip(base_r - 28, 0, 255))
                base_g = int(np.clip(base_g - 22, 0, 255))
                base_b = int(np.clip(base_b - 18, 0, 255))

            atlas.putpixel((x, y), (base_r, base_g, base_b, 255))

    return atlas


def build_bbmodel_json() -> dict:
    """组装符合 Blockbench 4.10 标准规范的 .bbmodel JSON 数据。"""
    atlas_img = generate_texture_atlas()
    buffered = io.BytesIO()
    atlas_img.save(buffered, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buffered.getvalue()).decode("utf-8")

    tex_uuid = str(uuid.uuid4())
    raw_cubes = build_cubes()

    elements = []
    bone_children = {name: [] for name in BONE_ORDER}

    for idx, (bone_name, mat_key, cube_name, f_pos, t_pos, rot, p_pos) in enumerate(raw_cubes):
        c_uuid = str(uuid.uuid4())
        zone = MAT_ZONE[mat_key]
        zx0, zy0, zx1, zy1 = zone

        # UV box 分配
        uv_u0 = zx0 + (idx * 3) % (zx1 - zx0 - 6)
        uv_v0 = zy0 + (idx * 3) % (zy1 - zy0 - 6)
        uv_box = [uv_u0, uv_v0, uv_u0 + 4.0, uv_v0 + 4.0]

        faces = {}
        for face_name in ["north", "east", "south", "west", "up", "down"]:
            faces[face_name] = {
                "uv": uv_box,
                "texture": 0,
            }

        # 移动到方块空间坐标（手持物中心 offset 规范）
        from_pos = [f_pos[0] + EMIT_OFFSET[0], f_pos[1] + EMIT_OFFSET[1], f_pos[2] + EMIT_OFFSET[2]]
        to_pos = [t_pos[0] + EMIT_OFFSET[0], t_pos[1] + EMIT_OFFSET[1], t_pos[2] + EMIT_OFFSET[2]]
        pivot_pos = [p_pos[0] + EMIT_OFFSET[0], p_pos[1] + EMIT_OFFSET[1], p_pos[2] + EMIT_OFFSET[2]]

        elem = {
            "name": cube_name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 3) for v in from_pos],
            "to": [round(float(v), 3) for v in to_pos],
            "autouv": 0,
            "color": 0,
            "origin": [round(float(v), 3) for v in pivot_pos],
            "faces": faces,
            "uuid": c_uuid,
            "type": "cube",
        }
        if any(abs(r) > 1e-4 for r in rot):
            elem["rotation"] = [round(float(r), 2) for r in rot]

        elements.append(elem)
        bone_children[bone_name].append(c_uuid)

    out_groups = []
    for bone_name in BONE_ORDER:
        out_groups.append({
            "name": bone_name,
            "origin": [8.0, 8.0, 8.0],
            "color": 0,
            "uuid": str(uuid.uuid4()),
            "export": True,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children[bone_name],
        })

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "HerbSickle",
        "model_identifier": "cai_yao_dao",
        "visible_box": [1, 1, 0],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": out_groups,
        "textures": [
            {
                "path": "",
                "name": "herb_sickle_texture",
                "folder": "block",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "saved": True,
                "uuid": tex_uuid,
                "source": tex_base64,
            }
        ],
        "display": {
            "thirdperson_righthand": {
                "rotation": [0, 60, -45],
                "translation": [0, 2.5, 1.5],
                "scale": [0.85, 0.85, 0.85],
            },
            "firstperson_righthand": {
                "rotation": [0, -75, 25],
                "translation": [1.13, 3.2, 1.13],
                "scale": [0.72, 0.72, 0.72],
            },
            "ground": {
                "rotation": [0, 0, 0],
                "translation": [0, 2, 0],
                "scale": [0.65, 0.65, 0.65],
            },
            "gui": {
                "rotation": [0, -90, -45],
                "translation": [1.0, 1.0, 0],
                "scale": [0.85, 0.85, 0.85],
            },
        },
    }
    return bbmodel


def self_test():
    """差分门禁自测（确保点名器与判据能正常识别有效特征）。"""
    print("=== 采药刀 (HerbSickle) 差分自测试验 ===")
    cubes = build_cubes()
    print(f"  ✓ 成功构建 {len(cubes)} 个体素元素")
    bone_counts = {}
    for bone, _, _, _, _, _, _ in cubes:
        bone_counts[bone] = bone_counts.get(bone, 0) + 1
    for bone, count in bone_counts.items():
        print(f"    - [{bone}]: {count} 个体素块")

    assert "handle" in bone_counts and bone_counts["handle"] >= 4, "handle 骨骼件不足"
    assert "binding" in bone_counts and bone_counts["binding"] >= 6, "binding 骨骼件不足"
    assert "blade" in bone_counts and bone_counts["blade"] >= 6, "blade 骨骼件不足"
    print("  ✓ 骨骼与体素数量验证 PASS")


def main():
    parser = argparse.ArgumentParser(description="HerbSickle .bbmodel 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行差分门禁自测试验")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    bbmodel_data = build_bbmodel_json()
    BBMODEL_OUT.write_text(json.dumps(bbmodel_data, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 成功导出 .bbmodel: {BBMODEL_OUT.relative_to(REPO)}")

    atlas = generate_texture_atlas()
    tex_path = OUT_DIR / "herb_sickle_texture.png"
    atlas.save(tex_path)
    print(f"✓ 成功导出贴图: {tex_path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
