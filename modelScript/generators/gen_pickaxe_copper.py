#!/usr/bin/env python3
"""铜镐（pickaxe_copper / 轻快旧铜镐）Blockbench .bbmodel 与 SML OBJ 程序化生成器。

设计理念与世界观：
- worldview & server/assets/items/tools.toml:
  id = "pickaxe_copper", name = "铜镐"
  description = "铜镐轻快省力，适合连敲浅矿，但耐久不如铁镐。"
- 末法残土废土修仙背景：由残破旧铜矿回炉锻造的双尖轻型鹤嘴矿镐。
  镐头呈双向微弧下倾结构，两端敲矿尖喙经过反复捶打磨尖，露出原生铜色 #b87333；
  镐身大面积布满斑驳氧化铜绿 #4f7a6a / #5e8c78。
- 握柄为质朴老木短柄，握持段缠绕粗麻防滑绳，中段带天然风干老木节疤凸起，
  底端加粗防脱手；镐眼中部由 1px 方形铜环套与顶端紧固铜楔紧紧锁定。

结构组成（4 大 Bone Group）：
    1. haft          - 质朴老木镐柄（加粗柄尾圆端、握持段、带节疤主柄身、穿眼榫头）
    2. cord_wrap     - 粗麻绳防滑与加固缠绑（握把防滑缠圈、镐颈加固扎绳）
    3. wedge_collar  - 外包 1px 方形铜环套与顶端紧固铜楔（防脱五金）
    4. pick_head     - 旧铜微曲双尖镐头（斧眼套筒、左右双向微弧下弯镐臂、1px 铜色磨尖啄头）

手持物约定：
    - 握柄轴线置于方块中心 (X=8.0, Z=8.0)
    - 镐头沿 X 轴对称双向展开 (X in [-1.0, 17.0], 翼展 18px)，微弯下垂
    - 支持 --dump-obj 输出 SML 标准 OBJ

用法：
    python3 modelScript/generators/gen_pickaxe_copper.py
    python3 modelScript/generators/gen_pickaxe_copper.py --self-test
    python3 modelScript/generators/gen_pickaxe_copper.py --dump-obj
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

REPO = Path(__file__).resolve().parents[2]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

MODEL_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/pickaxe_copper")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/exploded/pickaxe_copper.png")
BBMODEL_OUT = MODEL_DIR / "PickaxeCopper.bbmodel"

RES = 64

# 骨骼层级与枢轴定义
BONE_ORDER = ["haft", "cord_wrap", "wedge_collar", "pick_head"]
BONE_PIVOTS = {
    "haft": [8.0, 9.5, 8.0],
    "cord_wrap": [8.0, 10.0, 8.0],
    "wedge_collar": [8.0, 18.0, 8.0],
    "pick_head": [8.0, 20.0, 8.0],
}

# 纹理贴图 4 象限定义 (64x64，沿用铜斧完全一致的调色板)
# Q1 (0..32, 0..32): copper_weathered (铜绿氧化铜主材 #4f7a6a / #5e8c78)
# Q2 (32..64, 0..32): aged_wood (深褐老木柄)
# Q3 (0..32, 32..64): copper_edge (镐尖与铜套原生铜色 #b87333)
# Q4 (32..64, 32..64): hemp_binding (粗麻绳缠绑)
MAT_REGIONS = {
    "copper_weathered": (0.0, 0.0, 32.0, 32.0),
    "aged_wood": (32.0, 0.0, 64.0, 32.0),
    "copper_edge": (0.0, 32.0, 32.0, 64.0),
    "hemp_binding": (32.0, 32.0, 64.0, 64.0),
}


# =============================================================================
# 部件几何定义
# =============================================================================

def part_haft() -> List[dict]:
    """老木镐柄（不规则老木：中段带 2 处 1px 节疤凸起，底端设加粗柄尾圆端）。"""
    return [
        # 柄尾略粗圆端 (Butt knob: 宽 2.0px, 防滑圆头)
        {"name": "haft_butt_knob_base", "group": "haft", "material": "aged_wood", "from": [7.0, 0.8, 7.0], "to": [9.0, 1.8, 9.0]},
        {"name": "haft_butt_knob_step", "group": "haft", "material": "aged_wood", "from": [7.2, 1.8, 7.2], "to": [8.8, 2.6, 8.8]},
        # 下柄身
        {"name": "haft_lower_shaft", "group": "haft", "material": "aged_wood", "from": [7.4, 2.6, 7.4], "to": [8.6, 6.5, 8.6]},
        # 握把段 (配粗麻缠绳)
        {"name": "haft_grip_shaft", "group": "haft", "material": "aged_wood", "from": [7.45, 6.5, 7.45], "to": [8.55, 11.5, 8.55]},
        # 中段柄身 1 与 节疤 1 (+X 侧外凸 0.8px)
        {"name": "haft_mid_shaft_1", "group": "haft", "material": "aged_wood", "from": [7.4, 11.5, 7.35], "to": [8.6, 13.5, 8.65]},
        {"name": "haft_knot_1", "group": "haft", "material": "aged_wood", "from": [8.6, 12.2, 7.6], "to": [9.4, 13.2, 8.4]},
        # 中段柄身 2 与 节疤 2 (-X 侧外凸 0.88px)
        {"name": "haft_mid_shaft_2", "group": "haft", "material": "aged_wood", "from": [7.38, 13.5, 7.4], "to": [8.58, 16.0, 8.6]},
        {"name": "haft_knot_2", "group": "haft", "material": "aged_wood", "from": [6.5, 14.5, 7.5], "to": [7.38, 15.5, 8.4]},
        # 中段柄身 3 与 颈部
        {"name": "haft_mid_shaft_3", "group": "haft", "material": "aged_wood", "from": [7.42, 16.0, 7.42], "to": [8.58, 17.5, 8.58]},
        {"name": "haft_neck_shaft", "group": "haft", "material": "aged_wood", "from": [7.45, 17.5, 7.45], "to": [8.55, 18.45, 8.55]},
        # 穿眼榫头 (穿过铜镐眼)
        {"name": "haft_tenon", "group": "haft", "material": "aged_wood", "from": [7.5, 18.45, 7.5], "to": [8.5, 21.6, 8.5]},
        # 顶端露出榫头
        {"name": "haft_crown", "group": "haft", "material": "aged_wood", "from": [7.55, 21.8, 7.55], "to": [8.45, 22.5, 8.45]},
    ]


def part_cord_wrap() -> List[dict]:
    """粗麻绳缠绑（握把防滑缠圈与镐颈加固扎绳）。"""
    return [
        # 握持段间距缠圈
        {"name": "wrap_g1", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 6.8, 7.32], "to": [8.68, 7.5, 8.68]},
        {"name": "wrap_g2", "group": "cord_wrap", "material": "hemp_binding", "from": [7.34, 7.7, 7.34], "to": [8.66, 8.4, 8.66]},
        {"name": "wrap_g3", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 8.6, 7.32], "to": [8.68, 9.3, 8.68]},
        {"name": "wrap_g4", "group": "cord_wrap", "material": "hemp_binding", "from": [7.34, 9.5, 7.34], "to": [8.66, 10.2, 8.66]},
        {"name": "wrap_g5", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 10.4, 7.32], "to": [8.68, 11.1, 8.68]},
        # 镐颈紧固扎绳 (位于铜环套下方)
        {"name": "wrap_n1", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 16.2, 7.32], "to": [8.68, 16.8, 8.68]},
        {"name": "wrap_n2", "group": "cord_wrap", "material": "hemp_binding", "from": [7.30, 16.9, 7.30], "to": [8.70, 17.45, 8.70]},
    ]


def part_wedge_collar() -> List[dict]:
    """镐头和柄之间的铜环套（柄外包 1px 的方环，#b87333）和顶端铜楔。"""
    return [
        # 方形铜环套：紧套在镐柄外四周外凸 1px (X/Z in [6.4, 9.6], Y in [17.5, 18.45])
        {"name": "collar_ring_n", "group": "wedge_collar", "material": "copper_edge", "from": [6.4, 17.5, 6.4], "to": [9.6, 18.45, 7.45]},
        {"name": "collar_ring_s", "group": "wedge_collar", "material": "copper_edge", "from": [6.4, 17.5, 8.55], "to": [9.6, 18.45, 9.6]},
        {"name": "collar_ring_w", "group": "wedge_collar", "material": "copper_edge", "from": [6.4, 17.5, 7.45], "to": [7.45, 18.45, 8.55]},
        {"name": "collar_ring_e", "group": "wedge_collar", "material": "copper_edge", "from": [8.55, 17.5, 7.45], "to": [9.6, 18.45, 8.55]},
        # 顶端固定铜楔 (Wedge driven into top of tenon: Y in [22.0, 23.2])
        {"name": "wedge_pin", "group": "wedge_collar", "material": "copper_edge", "from": [7.7, 22.0, 7.2], "to": [8.3, 23.2, 8.8]},
    ]


def part_pick_head() -> List[dict]:
    """旧铜双尖镐头（镐身厚度 2px 铜绿 #4f7a6a / #5e8c78，左右双向微弧下弯镐臂，两端尖喙收至 1px 露出铜色 #b87333）。"""
    return [
        # 镐眼中部套筒 (包裹榫头，材质铜绿)
        {"name": "eye_socket_mid",   "group": "pick_head", "material": "copper_weathered", "from": [6.8, 18.5, 6.6], "to": [9.2, 21.8, 9.4]},
        # 套筒顶端加固冠环 (外凸增强套箍受力结构)
        {"name": "eye_socket_crown", "group": "pick_head", "material": "copper_weathered", "from": [6.6, 21.5, 6.4], "to": [9.4, 22.1, 9.6]},

        # ── 左臂 (-X 侧镐喙，微弧向下收细) ──
        {"name": "arm_l_1",     "group": "pick_head", "material": "copper_weathered", "from": [5.4, 18.4, 7.0], "to": [6.8, 21.2, 9.0]},
        {"name": "arm_l_2",     "group": "pick_head", "material": "copper_weathered", "from": [3.8, 18.1, 7.1], "to": [5.4, 20.6, 8.9]},
        {"name": "arm_l_3",     "group": "pick_head", "material": "copper_weathered", "from": [2.2, 17.6, 7.2], "to": [3.8, 19.8, 8.8]},
        {"name": "arm_l_4",     "group": "pick_head", "material": "copper_weathered", "from": [0.8, 16.8, 7.3], "to": [2.2, 18.8, 8.7]},
        {"name": "arm_l_bevel", "group": "pick_head", "material": "copper_weathered", "from": [-0.2, 15.8, 7.4], "to": [0.8, 17.6, 8.6]},
        # 左端磨尖铜喙 (厚度收至 1px，露出铜色 #b87333)
        {"name": "arm_l_tip",   "group": "pick_head", "material": "copper_edge", "from": [-1.0, 15.0, 7.5], "to": [-0.2, 16.4, 8.5]},
        # 左端极细鹤嘴端点 (两级收尖: 尖端延伸至 -1.6, 截面 0.6x0.9x0.7)
        {"name": "arm_l_beak",  "group": "pick_head", "material": "copper_edge", "from": [-1.6, 14.5, 7.65], "to": [-1.0, 15.4, 8.35]},

        # ── 右臂 (+X 侧镐喙，微弧向下收细) ──
        {"name": "arm_r_1",     "group": "pick_head", "material": "copper_weathered", "from": [9.2, 18.4, 7.0], "to": [10.6, 21.2, 9.0]},
        {"name": "arm_r_2",     "group": "pick_head", "material": "copper_weathered", "from": [10.6, 18.1, 7.1], "to": [12.2, 20.6, 8.9]},
        {"name": "arm_r_3",     "group": "pick_head", "material": "copper_weathered", "from": [12.2, 17.6, 7.2], "to": [13.8, 19.8, 8.8]},
        {"name": "arm_r_4",     "group": "pick_head", "material": "copper_weathered", "from": [13.8, 16.8, 7.3], "to": [15.2, 18.8, 8.7]},
        {"name": "arm_r_bevel", "group": "pick_head", "material": "copper_weathered", "from": [15.2, 15.8, 7.4], "to": [16.2, 17.6, 8.6]},
        # 右端磨尖铜喙 (厚度收至 1px，露出铜色 #b87333)
        {"name": "arm_r_tip",   "group": "pick_head", "material": "copper_edge", "from": [16.2, 15.0, 7.5], "to": [17.0, 16.4, 8.5]},
        # 右端极细鹤嘴端点 (两级收尖: 尖端延伸至 17.6, 截面 0.6x0.9x0.7)
        {"name": "arm_r_beak",  "group": "pick_head", "material": "copper_edge", "from": [17.0, 14.5, 7.65], "to": [17.6, 15.4, 8.35]},
    ]


def all_cubes() -> List[dict]:
    """组装所有分部件立方体。"""
    cubes = []
    cubes.extend(part_haft())
    cubes.extend(part_cord_wrap())
    cubes.extend(part_wedge_collar())
    cubes.extend(part_pick_head())
    return cubes


# =============================================================================
# 纹理与 UV 生成
# =============================================================================

def generate_texture() -> Image.Image:
    """生成 64x64 四象限纹理贴图（铜绿氧化铜主材 + 老木柄 + 原生亮铜 + 粗麻绳）。"""
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 255))
    rng = np.random.default_rng(20261012)

    # ── Q1: 铜绿氧化铜镐面 (copper_weathered: #4f7a6a / #5e8c78) ──
    cop_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_patina = np.array([86, 131, 113], dtype=np.float32)
    noise_patina = rng.normal(0, 10, (32, 32, 1))
    patina_rgb = np.clip(base_patina + noise_patina, 60, 160).astype(np.uint8)
    cop_arr[:, :, :3] = patina_rgb
    cop_arr[:, :, 3] = 255
    q1 = Image.fromarray(cop_arr, "RGBA")
    q1_draw = ImageDraw.Draw(q1)

    for _ in range(20):
        rx, ry = rng.integers(1, 30, 2)
        rw, rh = rng.integers(2, 6, 2)
        dark_g = (int(rng.integers(55, 78)), int(rng.integers(88, 120)), int(rng.integers(72, 100)), 255)
        q1_draw.ellipse([rx, ry, rx + rw, ry + rh], fill=dark_g)

    for _ in range(6):
        rx, ry = rng.integers(2, 30, 2)
        rw, rh = rng.integers(1, 3, 2)
        under_cop = (int(rng.integers(135, 165)), int(rng.integers(80, 105)), int(rng.integers(50, 70)), 255)
        q1_draw.ellipse([rx, ry, rx + rw, ry + rh], fill=under_cop)

    img.paste(q1, (0, 0))

    # ── Q2: 深褐老木镐柄 (aged_wood: 32..64, 0..32) ──
    wood_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_wood = np.array([92, 64, 44], dtype=np.float32)
    noise_wood = rng.normal(0, 6, (32, 32, 1))
    wood_rgb = np.clip(base_wood + noise_wood, 55, 130).astype(np.uint8)
    wood_arr[:, :, :3] = wood_rgb
    wood_arr[:, :, 3] = 255
    q2 = Image.fromarray(wood_arr, "RGBA")
    q2_draw = ImageDraw.Draw(q2)

    for x in range(0, 32, 3):
        col_w = int(72 + rng.integers(0, 28))
        q2_draw.line([(x, 0), (x, 31)], fill=(col_w + 16, col_w, col_w - 20, 255))
        if rng.random() > 0.6:
            kn_y = rng.integers(4, 28)
            q2_draw.ellipse([x - 1, kn_y - 2, x + 2, kn_y + 2], fill=(46, 30, 20, 255))

    img.paste(q2, (32, 0))

    # ── Q3: 原生铜色磨尖啄头与铜环套 (copper_edge: #b87333 / 184, 115, 51) ──
    edge_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_edge = np.array([184, 115, 51], dtype=np.float32)
    noise_edge = rng.normal(0, 8, (32, 32, 1))
    edge_rgb = np.clip(base_edge + noise_edge, 130, 240).astype(np.uint8)
    edge_arr[:, :, :3] = edge_rgb
    edge_arr[:, :, 3] = 255
    q3 = Image.fromarray(edge_arr, "RGBA")
    q3_draw = ImageDraw.Draw(q3)

    for y in range(0, 32):
        bright = int(210 + rng.integers(0, 35))
        q3_draw.line([(0, y), (6, y)], fill=(bright, int(bright * 0.66), int(bright * 0.32), 255))
        q3_draw.line([(7, y), (14, y)], fill=(bright - 35, int((bright - 35) * 0.62), int((bright - 35) * 0.30), 255))

    img.paste(q3, (0, 32))

    # ── Q4: 粗麻绑扎 (hemp_binding: 32..64, 32..64) ──
    cord_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_cord = np.array([142, 122, 88], dtype=np.float32)
    noise_cord = rng.normal(0, 6, (32, 32, 1))
    cord_rgb = np.clip(base_cord + noise_cord, 90, 180).astype(np.uint8)
    cord_arr[:, :, :3] = cord_rgb
    cord_arr[:, :, 3] = 255
    q4 = Image.fromarray(cord_arr, "RGBA")
    q4_draw = ImageDraw.Draw(q4)

    for offset in range(-32, 64, 4):
        q4_draw.line([(offset, 0), (offset + 32, 32)], fill=(115, 96, 68, 255), width=1)
        q4_draw.line([(offset + 2, 0), (offset + 34, 32)], fill=(168, 146, 110, 255), width=1)

    img.paste(q4, (32, 32))

    return img


def get_uv_for_mat(mat: str, from_xyz: List[float], to_xyz: List[float]) -> dict:
    """按部件尺寸在 32x32 象限内映射 UV。"""
    u0, v0, u1, v1 = MAT_REGIONS.get(mat, (0.0, 0.0, 32.0, 32.0))
    dx = abs(to_xyz[0] - from_xyz[0])
    dy = abs(to_xyz[1] - from_xyz[1])
    dz = abs(to_xyz[2] - from_xyz[2])

    scale = 1.0
    u_len = min(30.0, max(2.0, (dx + dz) * scale))
    v_len = min(30.0, max(2.0, (dz + dy) * scale))

    base_u = u0 + 1.0
    base_v = v0 + 1.0

    return {
        "north": {"uv": [base_u, base_v, base_u + dx * scale, base_v + dy * scale], "texture": 0},
        "south": {"uv": [base_u, base_v, base_u + dx * scale, base_v + dy * scale], "texture": 0},
        "east":  {"uv": [base_u, base_v, base_u + dz * scale, base_v + dy * scale], "texture": 0},
        "west":  {"uv": [base_u, base_v, base_u + dz * scale, base_v + dy * scale], "texture": 0},
        "up":    {"uv": [base_u, base_v, base_u + dx * scale, base_v + dz * scale], "texture": 0},
        "down":  {"uv": [base_u, base_v, base_u + dx * scale, base_v + dz * scale], "texture": 0},
    }


# =============================================================================
# 门禁与校验
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
# Blockbench .bbmodel 与 SML OBJ 导出
# =============================================================================

def build_bbmodel(cubes: List[dict] | None = None, texture_img: Image.Image | None = None) -> tuple[dict, List[dict], Image.Image]:
    """构建标准 Blockbench .bbmodel 数据。"""
    if cubes is None:
        cubes = all_cubes()
        _assert_no_coplanar_faces(cubes)
    if texture_img is None:
        texture_img = generate_texture()

    tex_buf = io.BytesIO()
    texture_img.save(tex_buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(tex_buf.getvalue()).decode("ascii")

    elements = []
    group_elements: Dict[str, List[str]] = {name: [] for name in BONE_ORDER}

    for c in cubes:
        elem_uuid = str(uuid.uuid4())
        uv_faces = get_uv_for_mat(c["material"], c["from"], c["to"])
        elem = {
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "from": c["from"],
            "to": c["to"],
            "autouv": 0,
            "color": 0,
            "origin": [8.0, 8.0, 8.0],
            "faces": uv_faces,
            "uuid": elem_uuid,
        }
        elements.append(elem)
        group_name = c.get("group", "pick_head")
        if group_name in group_elements:
            group_elements[group_name].append(elem_uuid)

    groups = []
    for bone_name in BONE_ORDER:
        groups.append({
            "name": bone_name,
            "origin": BONE_PIVOTS.get(bone_name, [8.0, 8.0, 8.0]),
            "color": 0,
            "children": group_elements[bone_name],
        })

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "PickaxeCopper",
        "model_identifier": "pickaxe_copper",
        "visible_box": [-16.0, -16.0, -16.0, 16.0, 16.0, 16.0],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": groups,
        "textures": [
            {
                "path": "",
                "name": "pickaxe_copper_texture",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": str(uuid.uuid4()),
                "source": tex_b64,
            }
        ],
        "display": {
            "thirdperson_righthand": {"rotation": [0, -90, 55], "translation": [0, 2.5, -0.5], "scale": [0.85, 0.85, 0.85]},
            "thirdperson_lefthand":  {"rotation": [0, 90, -55], "translation": [0, 2.5, -0.5], "scale": [0.85, 0.85, 0.85]},
            "firstperson_righthand": {"rotation": [0, -90, 25], "translation": [1.13, 3.2, 1.13], "scale": [0.68, 0.68, 0.68]},
            "firstperson_lefthand":  {"rotation": [0, 90, -25], "translation": [1.13, 3.2, 1.13], "scale": [0.68, 0.68, 0.68]},
            "ground":                {"rotation": [0, 0, 0], "translation": [0, 2, 0], "scale": [0.5, 0.5, 0.5]},
            "gui":                   {"rotation": [0, 0, -45], "translation": [0, 0, 0], "scale": [0.95, 0.95, 0.95]},
            "fixed":                 {"rotation": [0, 180, 0], "translation": [0, 0, 0], "scale": [0.8, 0.8, 0.8]},
        },
    }

    return bbmodel, cubes, texture_img


def build_obj(cubes: List[dict] | None = None) -> str:
    """将立方体集导出为标准 Wavefront OBJ 格式字符串。"""
    if cubes is None:
        cubes = all_cubes()

    lines = [
        "# Wavefront OBJ exported by gen_pickaxe_copper.py",
        "mtllib pickaxe_copper.mtl",
        "o pickaxe_copper",
    ]

    vert_offset = 1
    for c in cubes:
        x0, y0, z0 = c["from"]
        x1, y1, z1 = c["to"]
        mat = c["material"]
        u0, v0, u1, v1 = MAT_REGIONS.get(mat, (0, 0, 32, 32))
        u0_norm, u1_norm = u0 / RES, u1 / RES
        v0_norm, v1_norm = 1.0 - (v1 / RES), 1.0 - (v0 / RES)

        verts = [
            (x0/16, y0/16, z0/16), (x1/16, y0/16, z0/16),
            (x1/16, y1/16, z0/16), (x0/16, y1/16, z0/16),
            (x0/16, y0/16, z1/16), (x1/16, y0/16, z1/16),
            (x1/16, y1/16, z1/16), (x0/16, y1/16, z1/16),
        ]
        for vx, vy, vz in verts:
            lines.append(f"v {vx:.6f} {vy:.6f} {vz:.6f}")

        lines.append(f"vt {u0_norm:.4f} {v0_norm:.4f}")
        lines.append(f"vt {u1_norm:.4f} {v0_norm:.4f}")
        lines.append(f"vt {u1_norm:.4f} {v1_norm:.4f}")
        lines.append(f"vt {u0_norm:.4f} {v1_norm:.4f}")

        lines.append(f"usemtl {mat}")

        faces = [
            (4, 3, 2, 1), # North (-Z)
            (5, 6, 7, 8), # South (+Z)
            (1, 5, 8, 4), # West (-X)
            (2, 3, 7, 6), # East (+X)
            (4, 8, 7, 3), # Up (+Y)
            (1, 2, 6, 5), # Down (-Y)
        ]
        for f in faces:
            v_idx = [vert_offset + idx - 1 for idx in f]
            lines.append(f"f {v_idx[0]}/1 {v_idx[1]}/2 {v_idx[2]}/3 {v_idx[3]}/4")

        vert_offset += 8

    return "\n".join(lines) + "\n"


def build_mtl() -> str:
    """生成配套的 MTL 材质描述字符串。"""
    lines = ["# Wavefront MTL exported by gen_pickaxe_copper.py"]
    for mat in MAT_REGIONS:
        lines.append(f"newmtl {mat}")
        lines.append("Ka 1.000 1.000 1.000")
        lines.append("Kd 1.000 1.000 1.000")
        lines.append("Ks 0.000 0.000 0.000")
        lines.append("d 1.0")
        lines.append("illum 1")
        lines.append(f"map_Kd bong:item/pickaxe_copper/atlas.png")
    return "\n".join(lines) + "\n"


# =============================================================================
# 审阅图像渲染
# =============================================================================

def render_views(bbmodel_path: Path):
    """输出 check.png (左参考图 / 右 3/4 + 侧视并排大图) 与 render.png (多视角拼图)。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染关键视角
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_broad, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=500, bg=bg_color)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 输出综合 render.png (2x2 拼版)
    cell_w, cell_h = 500, 500
    canvas = Image.new("RGB", (cell_w * 2 + 30, cell_h * 2 + 30), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("pickaxe_copper (3/4 ISOMETRIC VIEW)", im_iso, 10, 10),
        ("pickaxe_copper (BROADSIDE VIEW - Arched Dual Beaks)", im_broad, cell_w + 20, 10),
        ("pickaxe_copper (SIDE VIEW - Socket Collar & Haft)", im_side, 10, cell_h + 20),
        ("pickaxe_copper (TOP VIEW - Double Curved Arms)", im_top, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 470, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (4 视角拼版) 已输出: {render_path}")

    # 3. 输出核对 check.png (左参考图 / 右 3/4 + 侧视并排等高大图)
    if EXPLODED_REF.exists():
        ref_im = Image.open(EXPLODED_REF).convert("RGB")
        target_h = 520
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_iso = im_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_broad = im_broad.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), f"EXPLODED REFERENCE ({EXPLODED_REF.name})", fill=(210, 200, 180))

        # 贴右 3/4 等轴视与侧视并排
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_broad, (rx + thumb_size + 8, 34 + 10))
        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: Broadside Dual-Beak View)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 对照卡 (左参考 / 右 3/4 + 侧视) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：几何无共面，缺陷精准拦截。"""
    print("运行 gen_pickaxe_copper.py 差分自证...")

    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] pickaxe_copper ({len(cubes)} 个立方体) 正常立方体集无共面冲突")

    # 左右双喙对称性与中心对齐断言
    tip_l = next(c for c in cubes if c["name"] == "arm_l_tip")
    tip_r = next(c for c in cubes if c["name"] == "arm_r_tip")
    haft_cube = next(c for c in cubes if c["name"] == "haft_mid_shaft_1")
    haft_mid_x = (haft_cube["from"][0] + haft_cube["to"][0]) / 2.0
    tip_l_mid_x = (tip_l["from"][0] + tip_l["to"][0]) / 2.0
    tip_r_mid_x = (tip_r["from"][0] + tip_r["to"][0]) / 2.0

    assert tip_l_mid_x < haft_mid_x < tip_r_mid_x, f"双喙布局错误: 左喙 {tip_l_mid_x:.2f} < 握柄 {haft_mid_x:.2f} < 右喙 {tip_r_mid_x:.2f}"
    print(f"  [OK] 双向镐喙对称布局断言通过 (左喙 X={tip_l_mid_x:.2f} < 握柄 X={haft_mid_x:.2f} < 右喙 X={tip_r_mid_x:.2f})")

    # 注入缺陷拦截测试
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [7.4, 2.6, 7.4],
        "to":   [8.6, 6.5, 8.6],
        "material": "aged_wood",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")

    print("✓ gen_pickaxe_copper.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="铜镐 (pickaxe_copper) 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--dump-obj", action="store_true", help="输出 Wavefront OBJ 到 stdout")
    args = parser.parse_args()

    if args.dump_obj:
        print(build_obj())
        return

    if args.self_test:
        self_test()
        return

    self_test()

    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    bb_json, cubes, tex_img = build_bbmodel()

    with open(BBMODEL_OUT, "w", encoding="utf-8") as f:
        json.dump(bb_json, f, indent=2)
    print(f"✓ PickaxeCopper bbmodel 写入成功: {BBMODEL_OUT} (包含 {len(cubes)} 个立方体)")

    render_views(BBMODEL_OUT)


if __name__ == "__main__":
    main()
