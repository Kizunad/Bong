#!/usr/bin/env python3
"""铜斧（axe_copper / 旧铜轻斧）Blockbench .bbmodel 与 SML OBJ 程序化生成器。

设计理念与世界观：
- worldview & server/assets/items/tools.toml:
  id = "axe_copper", name = "铜斧"
  description = "旧铜矿回炉的轻斧，挥得快，刃口也更容易卷。"
- 末法残土废土修仙背景：由残破旧铜回炉重铸的伐木与防身轻斧。
  依据调度审第 1 次指示修改：
  1. 斧面主色改为铜绿 #4f7a6a / #5e8c78 斑驳氧化铜，刃口 1px 一圈与斧背锤头露出原生铜色 #b87333；
  2. 斧身厚度 2px，刃口一侧收至 1px，斧背设 2×2 锤背凸起；
  3. 斧头与柄之间设铜环套（柄外包 1px 方环）与紧固铜楔；
  4. 老木柄中段带 2 处 1px 节疤凸起，底端设略粗的柄尾圆端。

结构组成（4 大 Bone Group）：
    1. haft          - 质朴老木斧柄（加粗柄尾圆端、握持段、带节疤主柄身、穿眼榫头）
    2. cord_wrap     - 粗麻绳防滑与加固缠绑（握把防滑缠圈、斧颈加固环缠）
    3. wedge_collar  - 外包 1px 方形铜环套与顶端紧固铜楔（防脱五金）
    4. axe_head      - 旧铜斧头（2px 铜绿斧身、2×2 铜色斧背平锤、1px 铜色月牙刃口）

手持物约定：
    - 握柄轴线置于方块中心 (X=8.0, Z=8.0)
    - 斧刃严格朝向 -X 侧（严格遵循两把 v2 斧与 test_axe_edge_direction.py 契约）
    - 斧背平锤朝向 +X 侧
    - 支持 --dump-obj 输出 SML 标准 OBJ

用法：
    python3 modelScript/generators/gen_axe_copper.py
    python3 modelScript/generators/gen_axe_copper.py --self-test
    python3 modelScript/generators/gen_axe_copper.py --dump-obj
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/axe_copper")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/exploded/axe_copper.png")
BBMODEL_OUT = MODEL_DIR / "AxeCopper.bbmodel"

RES = 64

# 骨骼层级与枢轴定义
BONE_ORDER = ["haft", "cord_wrap", "wedge_collar", "axe_head"]
BONE_PIVOTS = {
    "haft": [8.0, 9.5, 8.0],
    "cord_wrap": [8.0, 10.0, 8.0],
    "wedge_collar": [8.0, 20.5, 8.0],
    "axe_head": [8.0, 23.5, 8.0],
}

# 纹理贴图 4 象限定义 (64x64)
# Q1 (0..32, 0..32): copper_weathered (铜绿氧化铜主材 #4f7a6a / #5e8c78)
# Q2 (32..64, 0..32): aged_wood (老木斧柄)
# Q3 (0..32, 32..64): copper_edge (开刃刃口与斧背裸铜色 #b87333)
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
    """老木斧柄（不规则老木：中段带 2 处 1px 节疤凸起，底端设略粗的柄尾圆端）。"""
    return [
        # 柄尾略粗圆端 (Butt knob: 宽 2.0px, 带有层级防滑圆头)
        {"name": "haft_butt_knob_base", "group": "haft", "material": "aged_wood", "from": [7.0, 0.8, 7.0], "to": [9.0, 1.8, 9.0]},
        {"name": "haft_butt_knob_step", "group": "haft", "material": "aged_wood", "from": [7.2, 1.8, 7.2], "to": [8.8, 2.6, 8.8]},
        # 下柄身
        {"name": "haft_lower_shaft", "group": "haft", "material": "aged_wood", "from": [7.4, 2.6, 7.4], "to": [8.6, 7.0, 8.6]},
        # 握把段 (配合防滑缠圈)
        {"name": "haft_grip_shaft", "group": "haft", "material": "aged_wood", "from": [7.45, 7.0, 7.45], "to": [8.55, 12.0, 8.55]},
        # 中段柄身 1 与 节疤 1 (+X 侧外凸 0.8px)
        {"name": "haft_mid_shaft_1", "group": "haft", "material": "aged_wood", "from": [7.4, 12.0, 7.35], "to": [8.6, 14.5, 8.65]},
        {"name": "haft_knot_1", "group": "haft", "material": "aged_wood", "from": [8.6, 13.2, 7.6], "to": [9.4, 14.2, 8.4]},
        # 中段柄身 2 与 节疤 2 (-X 侧外凸 0.88px)
        {"name": "haft_mid_shaft_2", "group": "haft", "material": "aged_wood", "from": [7.38, 14.5, 7.4], "to": [8.58, 17.0, 8.6]},
        {"name": "haft_knot_2", "group": "haft", "material": "aged_wood", "from": [6.5, 15.6, 7.5], "to": [7.38, 16.6, 8.4]},
        # 中段柄身 3 与 颈部
        {"name": "haft_mid_shaft_3", "group": "haft", "material": "aged_wood", "from": [7.42, 17.0, 7.42], "to": [8.58, 19.5, 8.58]},
        {"name": "haft_neck_shaft", "group": "haft", "material": "aged_wood", "from": [7.45, 19.5, 7.45], "to": [8.55, 20.8, 8.55]},
        # 穿眼榫头 (穿过铜斧眼)
        {"name": "haft_tenon", "group": "haft", "material": "aged_wood", "from": [7.5, 20.8, 7.5], "to": [8.5, 25.3, 8.5]},
        # 顶端露出榫头
        {"name": "haft_crown", "group": "haft", "material": "aged_wood", "from": [7.55, 25.7, 7.55], "to": [8.45, 26.6, 8.45]},
    ]


def part_cord_wrap() -> List[dict]:
    """粗麻绳缠绑（握把防滑缠圈与颈部加固扎绳）。"""
    return [
        # 握持段间距缠圈
        {"name": "wrap_g1", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 7.2, 7.32], "to": [8.68, 8.0, 8.68]},
        {"name": "wrap_g2", "group": "cord_wrap", "material": "hemp_binding", "from": [7.34, 8.2, 7.34], "to": [8.66, 9.0, 8.66]},
        {"name": "wrap_g3", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 9.2, 7.32], "to": [8.68, 10.0, 8.68]},
        {"name": "wrap_g4", "group": "cord_wrap", "material": "hemp_binding", "from": [7.34, 10.2, 7.34], "to": [8.66, 11.0, 8.66]},
        {"name": "wrap_g5", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 11.2, 7.32], "to": [8.68, 11.8, 8.68]},
        # 斧颈紧固扎绳 (位于铜环套下方)
        {"name": "wrap_n1", "group": "cord_wrap", "material": "hemp_binding", "from": [7.32, 18.2, 7.32], "to": [8.68, 18.9, 8.68]},
        {"name": "wrap_n2", "group": "cord_wrap", "material": "hemp_binding", "from": [7.30, 19.0, 7.30], "to": [8.70, 19.7, 8.70]},
    ]


def part_wedge_collar() -> List[dict]:
    """斧头和柄之间的铜环套（柄外包 1px 的方环，#b87333）和一个楔子。"""
    return [
        # 方形铜环套：紧套在斧柄外四周外凸 1px (X/Z in [6.4, 9.6], Y in [19.8, 20.95])
        {"name": "collar_ring_n", "group": "wedge_collar", "material": "copper_edge", "from": [6.4, 19.8, 6.4], "to": [9.6, 20.95, 7.45]},
        {"name": "collar_ring_s", "group": "wedge_collar", "material": "copper_edge", "from": [6.4, 19.8, 8.55], "to": [9.6, 20.95, 9.6]},
        {"name": "collar_ring_w", "group": "wedge_collar", "material": "copper_edge", "from": [6.4, 19.8, 7.45], "to": [7.45, 20.95, 8.55]},
        {"name": "collar_ring_e", "group": "wedge_collar", "material": "copper_edge", "from": [8.55, 19.8, 7.45], "to": [9.6, 20.95, 8.55]},
        # 顶端固定铜楔 (Wedge driven into top of tenon: Y in [26.0, 27.2])
        {"name": "wedge_pin", "group": "wedge_collar", "material": "copper_edge", "from": [7.7, 26.0, 7.2], "to": [8.3, 27.2, 8.8]},
    ]


def part_axe_head() -> List[dict]:
    """旧铜斧头（斧身厚度 2px 铜绿 #4f7a6a / #5e8c78，刃口一侧收到 1px 露出铜色 #b87333，斧背设 2×2 锤背凸起）。"""
    return [
        # 斧眼中部套筒 (包裹榫头，材质铜绿)
        {"name": "eye_socket_mid", "group": "axe_head", "material": "copper_weathered", "from": [6.8, 21.0, 6.6], "to": [9.2, 25.5, 9.4]},
        # 斧背平敲锤颈 (+X 侧，露出铜色 #b87333)
        {"name": "poll_neck", "group": "axe_head", "material": "copper_edge", "from": [9.2, 21.8, 7.0], "to": [10.0, 24.8, 9.0]},
        # 锤颈过渡凸缘 (增强铸造粗粝感)
        {"name": "poll_rim",  "group": "axe_head", "material": "copper_edge", "from": [9.7, 22.0, 6.8], "to": [10.3, 24.6, 9.2]},
        # 斧背 2×2 锤背凸起 (截面 2×2: Y in [22.3, 24.3], Z in [7.0, 9.0], 向 +X 突出至 11.8，露出铜色 #b87333)
        {"name": "poll_hammer_2x2", "group": "axe_head", "material": "copper_edge", "from": [10.3, 22.3, 7.0], "to": [11.8, 24.3, 9.0]},

        # 斧身主体 (厚度严格为 2px: Z in [7.0, 9.0]，斧面主色铜绿 #4f7a6a / #5e8c78)
        {"name": "blade_throat", "group": "axe_head", "material": "copper_weathered", "from": [5.5, 21.0, 7.0], "to": [6.8, 25.8, 9.0]},
        {"name": "blade_fan_1",  "group": "axe_head", "material": "copper_weathered", "from": [4.0, 19.8, 7.0], "to": [5.5, 26.8, 9.0]},
        {"name": "blade_fan_2",  "group": "axe_head", "material": "copper_weathered", "from": [2.5, 18.5, 7.0], "to": [4.0, 27.8, 9.0]},
        {"name": "blade_fan_3",  "group": "axe_head", "material": "copper_weathered", "from": [1.2, 17.2, 7.0], "to": [2.5, 28.6, 9.0]},

        # 刃口开刃斜面细化过渡 (厚度 1.6px 与 1.3px 梯级削薄，铜绿渐变)
        {"name": "blade_bevel_mid", "group": "axe_head", "material": "copper_weathered", "from": [0.4, 16.5, 7.2], "to": [1.2, 29.0, 8.8]},
        {"name": "blade_bevel_sub", "group": "axe_head", "material": "copper_weathered", "from": [-0.2, 16.0, 7.35], "to": [0.4, 29.4, 8.65]},

        # 刃口一圈 1px (Z in [7.5, 8.5], 厚度严格收到 1px，露出铜色 #b87333)
        {"name": "blade_edge_1px_main", "group": "axe_head", "material": "copper_edge", "from": [-0.8, 15.6, 7.5], "to": [-0.2, 29.8, 8.5]},

        # 月牙上端微卷月牙角 (两级微卷倒勾，露出铜色 #b87333)
        {"name": "blade_edge_tip_top1", "group": "axe_head", "material": "copper_edge", "from": [-0.5, 29.8, 7.5], "to": [0.8, 30.5, 8.5]},
        {"name": "blade_edge_tip_top2", "group": "axe_head", "material": "copper_edge", "from": [0.8, 29.6, 7.5], "to": [1.8, 30.2, 8.5]},

        # 月牙下端微卷月牙角 (两级微卷内钩，露出铜色 #b87333)
        {"name": "blade_edge_tip_bot1", "group": "axe_head", "material": "copper_edge", "from": [-0.5, 14.8, 7.5], "to": [0.8, 15.6, 8.5]},
        {"name": "blade_edge_tip_bot2", "group": "axe_head", "material": "copper_edge", "from": [0.8, 15.2, 7.5], "to": [1.8, 15.8, 8.5]},
    ]


def all_cubes() -> List[dict]:
    """组装所有分部件立方体。"""
    cubes = []
    cubes.extend(part_haft())
    cubes.extend(part_cord_wrap())
    cubes.extend(part_wedge_collar())
    cubes.extend(part_axe_head())
    return cubes


# =============================================================================
# 纹理与 UV 生成
# =============================================================================

def generate_texture() -> Image.Image:
    """生成 64x64 四象限纹理贴图：
    Q1 (0..32, 0..32): copper_weathered - 铜绿 #4f7a6a / #5e8c78 斑驳氧化铜
    Q2 (32..64, 0..32): aged_wood - 深褐老木、天然纵向木纹、树节暗斑
    Q3 (0..32, 32..64): copper_edge - 刃口 1px 与斧背锤头露出原生铜色 #b87333
    Q4 (32..64, 32..64): hemp_binding - 粗麻编织纤维纹、阴影转折
    """
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 255))
    rng = np.random.default_rng(20261012)

    # ── Q1: 铜绿氧化铜斧面 (copper_weathered: #4f7a6a / #5e8c78) ──
    cop_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_patina = np.array([86, 131, 113], dtype=np.float32)
    noise_patina = rng.normal(0, 10, (32, 32, 1))
    patina_rgb = np.clip(base_patina + noise_patina, 60, 160).astype(np.uint8)
    cop_arr[:, :, :3] = patina_rgb
    cop_arr[:, :, 3] = 255
    q1 = Image.fromarray(cop_arr, "RGBA")
    q1_draw = ImageDraw.Draw(q1)

    # 铜绿深浅斑块 (#4f7a6a / #5e8c78 错落分布)
    for _ in range(20):
        rx, ry = rng.integers(1, 30, 2)
        rw, rh = rng.integers(2, 6, 2)
        dark_g = (int(rng.integers(55, 78)), int(rng.integers(88, 120)), int(rng.integers(72, 100)), 255)
        q1_draw.ellipse([rx, ry, rx + rw, ry + rh], fill=dark_g)

    # 底层偶尔若隐若现露出的暗铜微粒
    for _ in range(6):
        rx, ry = rng.integers(2, 30, 2)
        rw, rh = rng.integers(1, 3, 2)
        under_cop = (int(rng.integers(135, 165)), int(rng.integers(80, 105)), int(rng.integers(50, 70)), 255)
        q1_draw.ellipse([rx, ry, rx + rw, ry + rh], fill=under_cop)

    img.paste(q1, (0, 0))

    # ── Q2: 深褐老木斧柄 (aged_wood: 32..64, 0..32) ──
    wood_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_wood = np.array([92, 64, 44], dtype=np.float32)
    noise_wood = rng.normal(0, 6, (32, 32, 1))
    wood_rgb = np.clip(base_wood + noise_wood, 55, 130).astype(np.uint8)
    wood_arr[:, :, :3] = wood_rgb
    wood_arr[:, :, 3] = 255
    q2 = Image.fromarray(wood_arr, "RGBA")
    q2_draw = ImageDraw.Draw(q2)

    # 纵向老木纤维纹理
    for x in range(0, 32, 3):
        col_w = int(72 + rng.integers(0, 28))
        q2_draw.line([(x, 0), (x, 31)], fill=(col_w + 16, col_w, col_w - 20, 255))
        if rng.random() > 0.6:
            kn_y = rng.integers(4, 28)
            q2_draw.ellipse([x - 1, kn_y - 2, x + 2, kn_y + 2], fill=(46, 30, 20, 255))

    img.paste(q2, (32, 0))

    # ── Q3: 露出原生铜色 (copper_edge: #b87333 / 184, 115, 51) ──
    edge_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_edge = np.array([184, 115, 51], dtype=np.float32)
    noise_edge = rng.normal(0, 8, (32, 32, 1))
    edge_rgb = np.clip(base_edge + noise_edge, 130, 240).astype(np.uint8)
    edge_arr[:, :, :3] = edge_rgb
    edge_arr[:, :, 3] = 255
    q3 = Image.fromarray(edge_arr, "RGBA")
    q3_draw = ImageDraw.Draw(q3)

    # 锤打研磨亮铜光条
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

    # 斜向纤维编织纹理
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
        group_name = c.get("group", "axe_head")
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
        "name": "AxeCopper",
        "model_identifier": "axe_copper",
        "visible_box": [-16.0, -16.0, -16.0, 16.0, 16.0, 16.0],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": groups,
        "textures": [
            {
                "path": "",
                "name": "axe_copper_texture",
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
        "# Wavefront OBJ exported by gen_axe_copper.py",
        "mtllib axe_copper.mtl",
        "o axe_copper",
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
    lines = ["# Wavefront MTL exported by gen_axe_copper.py"]
    for mat in MAT_REGIONS:
        lines.append(f"newmtl {mat}")
        lines.append("Ka 1.000 1.000 1.000")
        lines.append("Kd 1.000 1.000 1.000")
        lines.append("Ks 0.000 0.000 0.000")
        lines.append("d 1.0")
        lines.append("illum 1")
        lines.append(f"map_Kd bong:item/axe_copper/atlas.png")
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
    im_edge, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=500, bg=bg_color)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 输出综合 render.png (2x2 拼版)
    cell_w, cell_h = 500, 500
    canvas = Image.new("RGB", (cell_w * 2 + 30, cell_h * 2 + 30), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("axe_copper (3/4 ISOMETRIC VIEW - Round 2)", im_iso, 10, 10),
        ("axe_copper (BROADSIDE PROFILE - 2px Body & 1px Edge)", im_broad, cell_w + 20, 10),
        ("axe_copper (CUTTING EDGE PROFILE - 2x2 Hammer Poll)", im_edge, 10, cell_h + 20),
        ("axe_copper (TOP VIEW - Square Collar & Knot)", im_top, cell_w + 20, cell_h + 20),
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

        # 贴右 3/4 等轴视与侧面轮廓视
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_broad, (rx + thumb_size + 8, 34 + 10))
        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: Broadside Profile View)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 对照卡 (左参考 / 右 3/4 + 侧视) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：几何无共面，缺陷精准拦截，刃口方向朝 -X。"""
    print("运行 gen_axe_copper.py 差分自证...")

    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] axe_copper ({len(cubes)} 个立方体) 正常立方体集无共面冲突")

    # 刃口方向断言 (-X 侧必须为斧刃)
    blade_cube = next(c for c in cubes if c["name"] == "blade_edge_1px_main")
    haft_cube = next(c for c in cubes if c["name"] == "haft_mid_shaft_1")
    blade_mid_x = (blade_cube["from"][0] + blade_cube["to"][0]) / 2.0
    haft_mid_x = (haft_cube["from"][0] + haft_cube["to"][0]) / 2.0
    assert blade_mid_x < haft_mid_x, f"契约破坏: 斧刃 X={blade_mid_x:.2f} 必须在握柄 X={haft_mid_x:.2f} 的 -X 侧!"
    print(f"  [OK] 斧刃朝向契约验证通过 (斧刃 X={blade_mid_x:.2f} < 握柄 X={haft_mid_x:.2f})")

    # 注入缺陷拦截测试
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [7.4, 2.6, 7.4],
        "to":   [8.6, 7.0, 8.6],
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

    print("✓ gen_axe_copper.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="铜斧 (axe_copper) 生成器")
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
    print(f"✓ AxeCopper bbmodel 写入成功: {BBMODEL_OUT} (包含 {len(cubes)} 个立方体)")

    render_views(BBMODEL_OUT)


if __name__ == "__main__":
    main()
