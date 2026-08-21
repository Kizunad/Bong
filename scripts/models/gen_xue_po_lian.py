#!/usr/bin/env python3
"""生成雪魄莲 (XuePoLian) Blockbench .bbmodel 与三视图预览 (莲花花瓣解剖与冰晶切面重构版)。

打磨重点 (Round 2 & 3 终极冰晶莲花重构)：
1. 真实雪魄莲花解剖重构 (Crystalline Lotus Petal Geometry):
   - 彻底打破“方形冰棍”感，重构为轻薄修长、两头尖中间宽的【菱形/剑形冰晶莲瓣 (Diamond Blade Petals)】（厚度 0.35~0.5 格，宽度 1.6~2.2 格）。
   - 【外层大莲瓣 (8 瓣)】：呈八方放射状向外舒展（外展倾角 35°~45°），瓣尖微微向上回翘，晶莹透明，冰蓝渐变至霜白刃缘。
   - 【中层交错莲瓣 (8 瓣)】：与外层交错 22.5° 排布，挺拔向上（倾角 55°~65°），形成莲花饱满的第二层杯托。
   - 【内层护心莲瓣 (6 瓣)】：如捧心般向上聚拢合抱（倾角 75°~80°），半包围中央雪魄晶核。
2. 莲心雪魄晶核与极寒冰蕊 (Central Frost Pod & Core):
   - 莲心中央为一颗纯净晶莹的六棱柱雪魄冰核，周围簇生 6 枚微型冰蕊，散发幽蓝冷辉。
3. 苍古玄冰岩台 (Frosted Basalt Base):
   - 层叠天然风化玄岩台，表面覆有半透明冰川冻冰薄层，绝无科幻机械感。
4. 128x128 像素级冰魄水墨 Atlas 与 Emissive 柔和微光贴图。
"""

from __future__ import annotations

import base64
import io
import json
import math
import uuid
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = REPO / "local_models"
PREVIEW_DIR = REPO / "scripts" / "models"
TEXTURE_RES = 128
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c0012")


@dataclass(frozen=True)
class Cube:
    bone: str
    name: str
    origin: tuple[float, float, float]
    target: tuple[float, float, float]
    uv_preset: str
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    rot_origin: tuple[float, float, float] = (0.0, 0.0, 0.0)


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(UUID_NAMESPACE, ":".join(parts)))


def make_directed_petal_segment(
    bone: str,
    name: str,
    p1: tuple[float, float, float],
    p2: tuple[float, float, float],
    width: float = 1.8,
    thickness: float = 0.38,
    uv_preset: str = "ice_petal_outer",
) -> Cube:
    """创建在空间中由 p1 指向 p2 的宽薄片状冰晶花瓣段。"""
    x1, y1, z1 = p1
    x2, y2, z2 = p2
    v = np.array([x2 - x1, y2 - y1, z2 - z1], dtype=float)
    L = float(np.linalg.norm(v))
    if L < 1e-4:
        return Cube(
            bone,
            name,
            (x1 - width / 2, y1, z1 - thickness / 2),
            (x1 + width / 2, y1 + 0.1, z1 + thickness / 2),
            uv_preset,
        )

    u = v / L
    ux, uy, uz = u

    rx_rad = math.acos(max(-1.0, min(1.0, uy)))
    sin_rx = math.sin(rx_rad)
    if sin_rx > 1e-5:
        ry_rad = math.atan2(ux, uz)
    else:
        ry_rad = 0.0

    rx_deg = math.degrees(rx_rad)
    ry_deg = math.degrees(ry_rad)
    rz_deg = 0.0

    hw = width / 2.0
    ht = thickness / 2.0

    orig = (x1 - hw, y1, z1 - ht)
    targ = (x1 + hw, y1 + L, z1 + ht)
    rot_origin = (x1, y1, z1)

    return Cube(
        bone,
        name,
        orig,
        targ,
        uv_preset,
        rotation=(rx_deg, ry_deg, rz_deg),
        rot_origin=rot_origin,
    )


def part_frozen_crag_base() -> list[Cube]:
    """苍古玄冰岩台基座。"""
    cubes: list[Cube] = []

    # 1. 底部主玄冰岩台
    cubes.append(
        Cube(
            "rock_base",
            "ice_crag_base_main",
            (1.5, 0.0, 1.5),
            (14.5, 1.8, 14.5),
            "frost_slate",
            (0.0, 10.0, 0.0),
            (8.0, 0.0, 8.0),
        )
    )
    # 2. 侧翼层叠霜冻梯级碎冰岩
    cubes.append(
        Cube(
            "rock_base",
            "ice_crag_left",
            (1.0, 0.0, 3.0),
            (5.2, 2.6, 12.0),
            "frost_slate",
            (-5.0, -18.0, 8.0),
            (3.1, 0.0, 7.5),
        )
    )
    cubes.append(
        Cube(
            "rock_base",
            "ice_crag_right_back",
            (8.5, 0.0, 8.0),
            (15.0, 3.2, 14.8),
            "frost_slate",
            (8.0, 26.0, -6.0),
            (11.7, 0.0, 11.4),
        )
    )
    cubes.append(
        Cube(
            "rock_base",
            "ice_crag_front_shelf",
            (4.5, 0.0, 1.0),
            (11.5, 1.4, 3.8),
            "frost_slate",
            (0.0, 8.0, 0.0),
            (8.0, 0.0, 2.4),
        )
    )

    # 3. 莲托透明冰垫
    cubes.append(
        Cube(
            "rock_base",
            "glacier_pad_center",
            (5.0, 1.4, 5.0),
            (11.0, 2.5, 11.0),
            "deep_glacier_ice",
            (0.0, 22.5, 0.0),
            (8.0, 2.0, 8.0),
        )
    )
    return cubes


def part_outer_blade_petals() -> list[Cube]:
    """外层 8 瓣舒展菱形冰晶莲瓣 (宽大轻薄，末端上翘微翻)。"""
    cubes: list[Cube] = []
    center_x, center_y, center_z = 8.0, 2.2, 8.0

    # 8 个对称主方向
    petal_angles = [0, 45, 90, 135, 180, 225, 270, 315]

    for idx, deg in enumerate(petal_angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        # 莲瓣 3 段控制点：基部 -> 展开中段 (宽阔) -> 尖梢回翘段 (尖锐)
        p_base = (
            center_x + cos_a * 1.6,
            center_y,
            center_z + sin_a * 1.6,
        )
        p_mid = (
            center_x + cos_a * 4.5,
            center_y + 1.4,
            center_z + sin_a * 4.5,
        )
        p_tip = (
            center_x + cos_a * 6.2,
            center_y + 2.3,
            center_z + sin_a * 6.2,
        )

        # 基部至中段：由宽变展
        cubes.append(
            make_directed_petal_segment(
                "outer_petals",
                f"outer_petal_{idx:02d}_lower",
                p_base,
                p_mid,
                width=2.1,
                thickness=0.40,
                uv_preset="ice_petal_outer",
            )
        )
        # 中段至瓣尖：收窄为锐利霜刃
        cubes.append(
            make_directed_petal_segment(
                "outer_petals",
                f"outer_petal_{idx:02d}_tip",
                p_mid,
                p_tip,
                width=1.3,
                thickness=0.32,
                uv_preset="ice_petal_rim",
            )
        )

    return cubes


def part_mid_blade_petals() -> list[Cube]:
    """中层 8 瓣挺拔交错冰华莲瓣 (交错 22.5° 排布，向上 60° 倾斜)。"""
    cubes: list[Cube] = []
    center_x, center_y, center_z = 8.0, 2.4, 8.0

    # 8 个交错角度
    mid_angles = [22.5, 67.5, 112.5, 157.5, 202.5, 247.5, 292.5, 337.5]

    for idx, deg in enumerate(mid_angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        p_base = (
            center_x + cos_a * 1.4,
            center_y,
            center_z + sin_a * 1.4,
        )
        p_mid = (
            center_x + cos_a * 3.4,
            center_y + 2.5,
            center_z + sin_a * 3.4,
        )
        p_tip = (
            center_x + cos_a * 4.6,
            center_y + 4.2,
            center_z + sin_a * 4.6,
        )

        cubes.append(
            make_directed_petal_segment(
                "mid_petals",
                f"mid_petal_{idx:02d}_lower",
                p_base,
                p_mid,
                width=1.8,
                thickness=0.38,
                uv_preset="ice_petal_inner",
            )
        )
        cubes.append(
            make_directed_petal_segment(
                "mid_petals",
                f"mid_petal_{idx:02d}_tip",
                p_mid,
                p_tip,
                width=1.1,
                thickness=0.30,
                uv_preset="ice_petal_inner",
            )
        )

    return cubes


def part_inner_embrace_petals() -> list[Cube]:
    """内层 6 瓣合抱护心莲瓣 (紧贴花心，角度向上 75°~80°)。"""
    cubes: list[Cube] = []
    center_x, center_y, center_z = 8.0, 2.6, 8.0

    inner_angles = [0, 60, 120, 180, 240, 300]

    for idx, deg in enumerate(inner_angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)

        p_base = (
            center_x + cos_a * 1.1,
            center_y,
            center_z + sin_a * 1.1,
        )
        p_mid = (
            center_x + cos_a * 2.2,
            center_y + 3.2,
            center_z + sin_a * 2.2,
        )
        p_tip = (
            center_x + cos_a * 1.9,
            center_y + 5.6,
            center_z + sin_a * 1.9,
        )

        cubes.append(
            make_directed_petal_segment(
                "inner_petals",
                f"inner_petal_{idx:02d}_lower",
                p_base,
                p_mid,
                width=1.5,
                thickness=0.35,
                uv_preset="ice_petal_inner",
            )
        )
        cubes.append(
            make_directed_petal_segment(
                "inner_petals",
                f"inner_petal_{idx:02d}_apex",
                p_mid,
                p_tip,
                width=0.9,
                thickness=0.28,
                uv_preset="ice_petal_inner",
            )
        )

    return cubes


def part_frost_core_and_stamens() -> list[Cube]:
    """雪魄晶核、冰蕊细柱与飘散雪尘。"""
    cubes: list[Cube] = []
    cx, cy, cz = 8.0, 2.8, 8.0

    # 1. 中央六角雪魄晶核 (Frost Core Crystal - 晶莹凝华)
    cubes.append(
        Cube(
            "frost_core",
            "frost_core_prism_1",
            (cx - 1.1, cy + 0.6, cz - 1.1),
            (cx + 1.1, cy + 3.6, cz + 1.1),
            "frost_core_crystal",
            (0.0, 30.0, 0.0),
            (cx, cy + 2.1, cz),
        )
    )
    cubes.append(
        Cube(
            "frost_core",
            "frost_core_apex",
            (cx - 0.75, cy + 3.4, cz - 0.75),
            (cx + 0.75, cy + 4.9, cz + 0.75),
            "frost_core_crystal",
            (15.0, 45.0, -15.0),
            (cx, cy + 4.15, cz),
        )
    )

    # 2. 环绕花蕊冰柱 (Ice Stamens)
    stamen_angles = [30, 90, 150, 210, 270, 330]
    for idx, deg in enumerate(stamen_angles):
        rad = math.radians(deg)
        px = cx + math.cos(rad) * 1.4
        pz = cz + math.sin(rad) * 1.4
        p_top = (
            cx + math.cos(rad) * 1.65,
            cy + 3.5,
            cz + math.sin(rad) * 1.65,
        )
        cubes.append(
            make_directed_petal_segment(
                "frost_core",
                f"stamen_pillar_{idx:02d}",
                (px, cy + 0.4, pz),
                p_top,
                width=0.45,
                thickness=0.45,
                uv_preset="frost_stamen",
            )
        )

    # 3. 悬浮极寒冰霜微尘 (Airborne Frost Moteparticles)
    spore_coords = [
        (4.4, 6.4, 4.8),
        (11.6, 6.0, 6.2),
        (6.0, 7.2, 11.2),
        (10.2, 7.0, 10.6),
    ]
    for idx, (sx, sy, sz) in enumerate(spore_coords):
        cubes.append(
            Cube(
                "spores",
                f"frost_mote_{idx:02d}",
                (sx - 0.22, sy - 0.22, sz - 0.22),
                (sx + 0.22, sy + 0.22, sz + 0.22),
                "frost_mote",
                (15.0 * (idx + 1), 30.0 * idx, -25.0 * idx),
                (sx, sy, sz),
            )
        )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_frozen_crag_base()
        + part_outer_blade_petals()
        + part_mid_blade_petals()
        + part_inner_embrace_petals()
        + part_frost_core_and_stamens()
    )


def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


# 【素雅冷冽冰霜色系 (Pure Frost Xianxia Palettes)】
PALETTE_ICE = [
    hex_to_rgb("#2A5068"),  # 0 深幽冰髓
    hex_to_rgb("#427594"),  # 1 冷冽冰蓝
    hex_to_rgb("#6EA5C2"),  # 2 透光天青冰
    hex_to_rgb("#A4D2E6"),  # 3 浅冰晶蓝
    hex_to_rgb("#D8EFF8"),  # 4 晶莹霜白
    hex_to_rgb("#F4FBFF"),  # 5 月魄纯白高光
]

PALETTE_CORE = [
    hex_to_rgb("#7FA8BE"),  # 0 冰芯阴影
    hex_to_rgb("#A6CCE0"),  # 1 浅蓝冷辉
    hex_to_rgb("#CCE7F4"),  # 2 凝华白霜
    hex_to_rgb("#EDF7FC"),  # 3 纯净雪魄
    hex_to_rgb("#FFFFFF"),  # 4 极光晶魄白
]

PALETTE_SLATE = [
    hex_to_rgb("#181E24"),  # 0 极暗玄冰岩
    hex_to_rgb("#262F38"),  # 1 深灰青冰岩
    hex_to_rgb("#384450"),  # 2 中灰风化层
    hex_to_rgb("#4E5E6D"),  # 3 浅灰岩棱
    hex_to_rgb("#7E98AE"),  # 4 覆霜浅蓝冰痕
]


def create_texture() -> Image.Image:
    """生成 128x128 高清素雅冰魄水墨手绘 UV Atlas。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 【外层冰晶莲瓣 (0,0)~(64,48)】
    for x in range(64):
        for y in range(48):
            u = x
            v = y
            grad = (u + v * 1.5) / 110.0
            idx = int(grad * 5.0)
            c = PALETTE_ICE[min(5, max(0, idx))]

            # 手绘菱形花瓣中央脊线 (Midrib Ridge) 与晶面折射
            if u == 32:
                c = PALETTE_ICE[5]  # 中脊高光
            elif abs(u - 32) <= 4:
                c = PALETTE_ICE[4]
            elif (u * 2 + v * 3) % 23 == 0:
                c = PALETTE_ICE[3]

            if v < 3:
                c = PALETTE_ICE[5]  # 瓣尖霜白

            img.putpixel((x, y), (*c, 255))

    # 2. 【瓣尖霜刃与内瓣 (64,0)~(128,48)】
    for x in range(64, 128):
        for y in range(48):
            u = x - 64
            v = y
            dist = math.hypot(u - 32, v - 24)
            if dist < 14:
                c = PALETTE_ICE[5]
            elif dist < 30:
                c = PALETTE_ICE[4]
            else:
                c = PALETTE_ICE[3]

            if (u + v) % 19 == 0:
                c = PALETTE_ICE[2]

            img.putpixel((x, y), (*c, 255))

    # 3. 【内瓣柔和冰华 (0,48)~(64,80)】
    for x in range(64):
        for y in range(48, 80):
            v = y - 48
            c = PALETTE_ICE[3] if v < 16 else PALETTE_ICE[4]
            if (x * 3 + v * 5) % 29 == 0:
                c = PALETTE_ICE[5]
            img.putpixel((x, y), (*c, 255))

    # 4. 【雪魄晶核与极寒冰蕊 (64,48)~(96,80)】
    for x in range(64, 96):
        for y in range(48, 80):
            u = x - 64
            v = y - 48
            dx = abs(u - 16)
            dy = abs(v - 16)
            d = math.hypot(dx, dy)
            if d < 5.0:
                c = PALETTE_CORE[4]
            elif d < 12.0:
                c = PALETTE_CORE[3]
            else:
                c = PALETTE_CORE[1]
            img.putpixel((x, y), (*c, 255))

    # 5. 【悬浮冰霜微尘 (96,48)~(128,80)】
    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            d = math.hypot(u - 16, v - 16)
            if d < 4.0:
                c = PALETTE_CORE[4]
            elif d < 8.0:
                c = PALETTE_ICE[3]
            else:
                img.putpixel((x, y), (0, 0, 0, 0))
                continue
            img.putpixel((x, y), (*c, 255))

    # 6. 【霜冻玄岩基座 (0,80)~(64,128)】
    for x in range(64):
        for y in range(80, 128):
            u = x
            v = y - 80
            strata = (v * 2 + int(math.sin(u * 0.25) * 3)) % 16
            if strata < 3:
                c = PALETTE_SLATE[0]
            elif strata < 9:
                c = PALETTE_SLATE[2]
            elif strata < 13:
                c = PALETTE_SLATE[1]
            else:
                c = PALETTE_SLATE[3]

            if (u * 7 + v * 11) % 27 == 0:
                c = PALETTE_SLATE[4]

            img.putpixel((x, y), (*c, 255))

    # 7. 【冰川深冻冰层 (64,80)~(128,128)】
    for x in range(64, 128):
        for y in range(80, 128):
            u = x - 64
            v = y - 80
            d = math.hypot(u - 32, v - 24)
            if d < 8.0:
                c = PALETTE_ICE[4]
            elif d < 24.0:
                c = PALETTE_ICE[2]
            else:
                c = PALETTE_ICE[1]

            if (u * 2 + v * 3) % 17 == 0:
                c = PALETTE_ICE[3]

            img.putpixel((x, y), (*c, 255))

    return img


def create_emissive_texture() -> Image.Image:
    """生成柔和雪魄极寒微光 Emissive 贴图。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 255))

    # 1. 雪魄晶核微光 (64,48)~(96,80)
    for x in range(64, 96):
        for y in range(48, 80):
            u = x - 64
            v = y - 48
            d = math.hypot(u - 16, v - 16)
            if d < 6.0:
                val = 220
            elif d < 12.0:
                val = int(220 * (1.0 - (d - 6.0) / 6.0))
            else:
                val = 0
            img.putpixel((x, y), (val, val, val, 255))

    # 2. 悬浮冰霜微尘 (96,48)~(128,80)
    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            d = math.hypot(u - 16, v - 16)
            val = 180 if d < 4.0 else 0
            img.putpixel((x, y), (val, val, val, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "ice_petal_outer": [0, 0, 64, 48],
        "ice_petal_rim": [64, 0, 128, 48],
        "ice_petal_inner": [0, 48, 64, 80],
        "frost_core_crystal": [64, 48, 96, 80],
        "frost_stamen": [64, 48, 96, 80],
        "frost_mote": [96, 48, 128, 80],
        "frost_slate": [0, 80, 64, 128],
        "deep_glacier_ice": [64, 80, 128, 128],
    }

    elements = []
    outliner_bones: dict[str, list[str]] = {}

    for c in cubes:
        elem_uuid = stable_uuid("elem", c.name)
        uv = uv_presets.get(c.uv_preset, [0, 0, 32, 32])

        faces = {}
        for fname in ("north", "east", "south", "west", "up", "down"):
            faces[fname] = {
                "uv": uv,
                "texture": 0,
            }

        elem = {
            "name": c.name,
            "box_uv": False,
            "type": "cube",
            "from": [round(c.origin[0], 2), round(c.origin[1], 2), round(c.origin[2], 2)],
            "to": [round(c.target[0], 2), round(c.target[1], 2), round(c.target[2], 2)],
            "faces": faces,
            "uuid": elem_uuid,
        }
        if any(c.rotation):
            elem["rotation"] = [round(r, 2) for r in c.rotation]
            elem["origin"] = [round(o, 2) for o in c.rot_origin]

        elements.append(elem)

        if c.bone not in outliner_bones:
            outliner_bones[c.bone] = []
        outliner_bones[c.bone].append(elem_uuid)

    outliner = []
    for bname, children in outliner_bones.items():
        outliner.append(
            {
                "name": bname,
                "origin": [8, 0, 8],
                "color": 0,
                "uuid": stable_uuid("bone", bname),
                "export": True,
                "isOpen": True,
                "locked": False,
                "visibility": True,
                "children": children,
            }
        )

    return {
        "meta": {
            "format_version": "4.5",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "XuePoLian",
        "model_identifier": "xue_po_lian",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "xue_po_lian.png",
                "name": "xue_po_lian",
                "folder": "block",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "saved": True,
                "uuid": stable_uuid("texture", "main"),
                "source": f"data:image/png;base64,{tex_base64}",
            }
        ],
    }


def main():
    LOCAL_MODELS.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)

    bb_dict = build_bbmodel_dict()
    bb_path = LOCAL_MODELS / "XuePoLian.bbmodel"
    bb_path.write_text(json.dumps(bb_dict, indent=2), encoding="utf-8")
    print(f"✅ [花瓣重构版] 生成 BBModel: {bb_path} (分辨率: {TEXTURE_RES}x{TEXTURE_RES}, 共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "xue_po_lian_texture.png"
    create_texture().save(tex_path)
    print(f"✅ 导出高清主贴图: {tex_path}")

    emissive_path = PREVIEW_DIR / "xue_po_lian_emissive.png"
    create_emissive_texture().save(emissive_path)
    print(f"✅ 导出柔和发光贴图: {emissive_path}")


if __name__ == "__main__":
    main()
