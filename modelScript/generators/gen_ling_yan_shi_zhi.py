#!/usr/bin/env python3
"""生成灵眼石芝 (LingYanShiZhi) Blockbench .bbmodel 与三视图预览 (素雅水墨 / 淡色单色古韵版)。

根据用户反馈“科幻感太强，淡色单色看看”全面去科幻化：
1. 彻底去除科幻元素与电子感：
   - 彻底删除玄岩基座上的“电路板式直角阵纹与发光线路”！
   - 改为【水墨山石 / 苍古青灰岩】：采用天然层理、风化断口、岩石微缝与极淡灰绿地衣苔痕。
   - 剔除浮空科幻方块/机械控制盒，改为极其微小、轻盈飘渺的【淡金真元孢子 / 灵气尘光】。
2. 建立素雅淡色调色彩体系 (Pale & Muted Natural Xianxia Palette):
   - 【羊脂月白玉芝】：极为干净、温润、素雅的象牙白与月白色系 (#C8C5BB -> #E2DFD5 -> #F0EDE4)，大面积素白留白，水墨晕染同心纹。
   - 【天然菌褶与柔和玉边】：去金属感！使用淡赭色、天然菌耳柔木金与古朴浅灰金 (#6B543C -> #B59A76 -> #D6BF9E)，自然内敛。
   - 【月白冰种翡翠 / 淡青灵石】：将原本高饱和霓虹青改为【冷润淡青玉 / 冰种透玉】(#467772 -> #6CA29B -> #C6EAE6 -> #ECFAF8 柔光月白)，呈现天然璞玉感。
3. 结构修整：
   - 伞盖薄片化、弧度圆润、菌柄若天鹅颈般柔美侧生，散发天然野生草木之灵气与古风仙韵。
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
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
PREVIEW_DIR = Path(__file__).resolve().parents[1] / "out"
TEXTURE_RES = 128
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c0010")


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


def make_oriented_cube(
    bone: str,
    name: str,
    p1: tuple[float, float, float],
    p2: tuple[float, float, float],
    thickness: float = 0.8,
    uv_preset: str = "jade_cap",
) -> Cube:
    """创建由 p1 指向 p2 的定向方柱。"""
    x1, y1, z1 = p1
    x2, y2, z2 = p2
    v = np.array([x2 - x1, y2 - y1, z2 - z1], dtype=float)
    L = float(np.linalg.norm(v))
    if L < 1e-4:
        return Cube(
            bone,
            name,
            (x1 - thickness / 2, y1, z1 - thickness / 2),
            (x1 + thickness / 2, y1 + 0.1, z1 + thickness / 2),
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

    ht = thickness / 2.0
    orig = (x1 - ht, y1, z1 - ht)
    targ = (x1 + ht, y1 + L, z1 + ht)
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


def part_ancient_crag_and_raw_jade() -> list[Cube]:
    """苍古灰岩基座与天然冰种青玉晶石。"""
    cubes: list[Cube] = []

    # 1. 底部主苍石台 (Ancient Slate Base)
    cubes.append(
        Cube(
            "rock_base",
            "rock_base_main",
            (1.5, 0.0, 1.5),
            (14.5, 2.0, 14.5),
            "pale_slate",
            (0.0, 8.0, 0.0),
            (8.0, 0.0, 8.0),
        )
    )
    # 2. 侧翼层叠风化石块
    cubes.append(
        Cube(
            "rock_base",
            "rock_step_left",
            (1.0, 0.0, 3.5),
            (5.5, 2.8, 12.0),
            "pale_slate",
            (-6.0, -15.0, 8.0),
            (3.2, 0.0, 7.8),
        )
    )
    cubes.append(
        Cube(
            "rock_base",
            "rock_ridge_back",
            (8.0, 0.0, 8.5),
            (15.0, 3.4, 14.8),
            "pale_slate",
            (8.0, 24.0, -6.0),
            (11.5, 0.0, 11.6),
        )
    )
    cubes.append(
        Cube(
            "rock_base",
            "rock_front_crag",
            (4.8, 0.0, 1.0),
            (11.2, 1.4, 3.6),
            "pale_slate",
            (0.0, 10.0, 0.0),
            (8.0, 0.0, 2.3),
        )
    )

    # 3. 灵眼天然冰青原石与伴生玉晶
    # 3.1 主冰青玉柱
    cubes.append(
        Cube(
            "crystal",
            "ice_jade_primary",
            (6.8, 1.2, 6.4),
            (8.6, 5.2, 8.2),
            "ice_jade",
            (14.0, 18.0, -12.0),
            (7.7, 1.2, 7.3),
        )
    )
    # 3.2 次冰青玉柱
    cubes.append(
        Cube(
            "crystal",
            "ice_jade_sec",
            (4.6, 0.8, 7.6),
            (6.0, 3.8, 9.0),
            "ice_jade",
            (-16.0, -28.0, 20.0),
            (5.3, 1.0, 8.3),
        )
    )
    # 3.3 小巧伴生玉砾
    cubes.append(
        Cube(
            "crystal",
            "ice_jade_shard_1",
            (9.6, 1.0, 5.0),
            (10.8, 3.0, 6.2),
            "ice_jade",
            (22.0, 40.0, -15.0),
            (10.2, 1.2, 5.6),
        )
    )
    cubes.append(
        Cube(
            "crystal",
            "ice_jade_shard_2",
            (4.0, 0.6, 4.4),
            (5.0, 2.2, 5.4),
            "ice_jade",
            (15.0, -45.0, 10.0),
            (4.5, 0.8, 4.9),
        )
    )
    # 3.4 灵眼石隙泉眼柔和水光
    cubes.append(
        Cube(
            "crystal",
            "qi_well_soft_glow",
            (5.4, 0.4, 5.4),
            (10.6, 1.5, 10.6),
            "qi_soft_white",
        )
    )
    return cubes


def part_curved_stipes() -> list[Cube]:
    """柔美温润的素白玉菌柄。"""
    cubes: list[Cube] = []

    # 1. 主芝白玉主柄
    stipe_main_nodes = [
        (7.8, 1.6, 7.4),
        (7.6, 3.4, 7.0),
        (7.2, 5.2, 6.4),
        (6.6, 7.0, 5.6),
        (5.8, 8.6, 4.8),
    ]
    for i in range(len(stipe_main_nodes) - 1):
        p1 = stipe_main_nodes[i]
        p2 = stipe_main_nodes[i + 1]
        t = 0.95 - i * 0.09
        cubes.append(
            make_oriented_cube("stipe", f"main_stipe_seg_{i:02d}", p1, p2, t, "jade_stipe")
        )

    # 2. 次芝白玉副柄
    stipe_sec_nodes = [
        (7.6, 3.4, 7.0),
        (8.6, 4.6, 8.0),
        (9.8, 6.0, 9.0),
    ]
    for i in range(len(stipe_sec_nodes) - 1):
        p1 = stipe_sec_nodes[i]
        p2 = stipe_sec_nodes[i + 1]
        t = 0.78 - i * 0.08
        cubes.append(
            make_oriented_cube("stipe", f"sec_stipe_seg_{i:02d}", p1, p2, t, "jade_stipe")
        )

    # 3. 幼芝嫩玉柄
    stipe_baby_nodes = [
        (6.0, 1.4, 5.2),
        (5.2, 2.6, 4.2),
    ]
    for i in range(len(stipe_baby_nodes) - 1):
        p1 = stipe_baby_nodes[i]
        p2 = stipe_baby_nodes[i + 1]
        cubes.append(
            make_oriented_cube("stipe", f"baby_stipe_seg_{i:02d}", p1, p2, 0.60, "jade_stipe")
        )

    return cubes


def part_pure_jade_pileus() -> list[Cube]:
    """如意素白玉灵芝伞盖群（大面积月白羊脂玉 + 极其素雅淡赭金缘）。"""
    cubes: list[Cube] = []

    # ─────────────────────────────────────────────────────────────
    # 【1. 主灵芝扇形伞盖 (Main White Jade Pileus)】
    # ─────────────────────────────────────────────────────────────
    cap1_rot = (8.0, -26.0, 12.0)
    cap1_org = (5.8, 8.6, 4.8)

    # 1.1 伞核中盘
    cubes.append(
        Cube(
            "main_cap",
            "cap1_hub",
            (4.4, 8.75, 3.4),
            (7.4, 9.15, 6.4),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.2 前向正中主扇翼
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_center",
            (3.4, 8.78, 2.0),
            (7.0, 9.16, 3.6),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.3 左向侧扇翼
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_left",
            (2.0, 8.78, 3.2),
            (4.6, 9.16, 7.0),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.4 右前外弧扇翼
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_rf",
            (6.6, 8.78, 2.2),
            (9.0, 9.16, 4.8),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.5 左前角扇片
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_lf",
            (2.4, 8.80, 2.2),
            (4.2, 9.18, 3.8),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.6 极外沿素朴淡金微卷边
    cubes.append(
        Cube(
            "main_cap",
            "cap1_lip_front",
            (2.0, 8.72, 1.7),
            (8.8, 9.10, 2.1),
            "soft_ochre_rim",
            cap1_rot,
            cap1_org,
        )
    )
    cubes.append(
        Cube(
            "main_cap",
            "cap1_lip_left",
            (1.7, 8.72, 2.0),
            (2.1, 9.10, 7.2),
            "soft_ochre_rim",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.7 伞底收缩式淡赭菌孔面
    cubes.append(
        Cube(
            "main_cap",
            "cap1_gills_recessed",
            (3.0, 8.58, 2.8),
            (6.8, 8.75, 5.8),
            "soft_gills",
            cap1_rot,
            cap1_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【2. 次灵芝扇形伞盖 (Secondary Jade Pileus)】
    # ─────────────────────────────────────────────────────────────
    cap2_rot = (-12.0, 46.0, -8.0)
    cap2_org = (9.8, 6.0, 9.0)

    cubes.append(
        Cube(
            "sec_cap",
            "cap2_hub",
            (8.6, 5.95, 7.8),
            (12.0, 6.35, 11.0),
            "jade_cap",
            cap2_rot,
            cap2_org,
        )
    )
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_wing_front",
            (9.6, 5.98, 6.6),
            (13.4, 6.36, 8.8),
            "jade_cap",
            cap2_rot,
            cap2_org,
        )
    )
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_wing_right",
            (11.4, 5.98, 8.2),
            (14.0, 6.36, 11.6),
            "jade_cap",
            cap2_rot,
            cap2_org,
        )
    )
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_lip_outer",
            (9.4, 5.92, 6.3),
            (14.2, 6.30, 6.7),
            "soft_ochre_rim",
            cap2_rot,
            cap2_org,
        )
    )
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_gills_recessed",
            (9.6, 5.80, 7.6),
            (12.6, 5.95, 10.0),
            "soft_gills",
            cap2_rot,
            cap2_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【3. 幼芝嫩玉伞盖 (Baby Jade Sprout)】
    # ─────────────────────────────────────────────────────────────
    cap3_rot = (18.0, -15.0, 10.0)
    cap3_org = (5.2, 2.6, 4.2)

    cubes.append(
        Cube(
            "baby_cap",
            "cap3_fan_body",
            (3.8, 2.55, 2.6),
            (6.8, 2.95, 5.4),
            "jade_cap",
            cap3_rot,
            cap3_org,
        )
    )
    cubes.append(
        Cube(
            "baby_cap",
            "cap3_lip_front",
            (3.5, 2.52, 2.3),
            (6.5, 2.90, 2.7),
            "soft_ochre_rim",
            cap3_rot,
            cap3_org,
        )
    )
    cubes.append(
        Cube(
            "baby_cap",
            "cap3_gills",
            (4.4, 2.40, 3.2),
            (5.8, 2.55, 4.5),
            "soft_gills",
            cap3_rot,
            cap3_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【4. 轻灵孢子尘光 (Delicate Mote Spores - 细小柔和光尘)】
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        Cube(
            "spores",
            "mote_spore_1",
            (2.9, 7.7, 2.3),
            (3.2, 8.0, 2.6),
            "spore_mote",
            (15.0, 30.0, 45.0),
            (3.05, 7.85, 2.45),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "mote_spore_2",
            (8.5, 8.1, 2.1),
            (8.8, 8.4, 2.4),
            "spore_mote",
            (-20.0, 45.0, 15.0),
            (8.65, 8.25, 2.25),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "mote_spore_3",
            (11.3, 5.3, 6.5),
            (11.6, 5.6, 6.8),
            "spore_mote",
            (10.0, -25.0, 30.0),
            (11.45, 5.45, 6.65),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "mote_spore_4",
            (4.9, 10.1, 3.9),
            (5.2, 10.4, 4.2),
            "spore_mote",
            (-30.0, 10.0, 60.0),
            (5.05, 10.25, 4.05),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_ancient_crag_and_raw_jade()
        + part_curved_stipes()
        + part_pure_jade_pileus()
    )


def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


# 【素雅淡色色彩体系 (Pale Monochrome Xianxia Palettes)】

# 1. 羊脂月白玉 (Mutton-Fat Pure Jade - 干净素雅、温润暖白)
PALETTE_JADE = [
    hex_to_rgb("#8A8982"),  # 0 极柔阴影
    hex_to_rgb("#A8A69E"),  # 1 浅灰玉质
    hex_to_rgb("#C8C5BB"),  # 2 中间象牙色
    hex_to_rgb("#E2DFD5"),  # 3 主体羊脂白 (大面积)
    hex_to_rgb("#F0EDE4"),  # 4 柔润高光
    hex_to_rgb("#F9F7F0"),  # 5 月白晶莹
]

# 2. 淡赭柔金 (Muted Raw Ochre & Wood-Gold - 去金属化、古朴木石感)
PALETTE_OCHRE = [
    hex_to_rgb("#4A3828"),  # 0 极暗木纹
    hex_to_rgb("#6B543C"),  # 1 暗赭色
    hex_to_rgb("#8E7355"),  # 2 浅古木金
    hex_to_rgb("#B59A76"),  # 3 柔和小麦金
    hex_to_rgb("#D6BF9E"),  # 4 素雅浅金边
    hex_to_rgb("#EADAC2"),  # 5 淡金微光
]

# 3. 苍古灰岩 (Pale Weathered Slate / Mountain Crag - 天然水墨岩石，无电路板纹)
PALETTE_SLATE = [
    hex_to_rgb("#2A2D30"),  # 0 水墨暗石底
    hex_to_rgb("#3D4246"),  # 1 深灰青石
    hex_to_rgb("#52585D"),  # 2 中灰岩层
    hex_to_rgb("#687076"),  # 3 风化浅灰岩面 (大面积主体)
    hex_to_rgb("#7E878E"),  # 4 浅岩棱角
    hex_to_rgb("#4E584A"),  # 5 极淡灰绿地衣苔痕 (天然点缀)
]

# 4. 冰种月白透青玉 (Pale Celadon / Ice Jade - 柔和通透水青，非电光霓虹)
PALETTE_ICE_JADE = [
    hex_to_rgb("#2B524E"),  # 0 深青底
    hex_to_rgb("#467772"),  # 1 沉静青绿
    hex_to_rgb("#6CA29B"),  # 2 柔和水青
    hex_to_rgb("#98CAC4"),  # 3 冰种透玉青
    hex_to_rgb("#C6EAE6"),  # 4 月白微青
    hex_to_rgb("#ECFAF8"),  # 5 纯净月白泉心
]


def create_texture() -> Image.Image:
    """生成 128x128 高清素雅水墨像素手绘 UV Atlas。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 【白玉菌盖上表面 (0,0)~(64,48)】
    # 80% 纯净大色块羊脂月白 #E2DFD5 与 #F0EDE4，极淡水墨同心晕染
    for x in range(64):
        for y in range(48):
            dist = math.hypot(x - 32, y - 6)
            if dist < 16:
                c = PALETTE_JADE[4]  # 月白高光中心 #F0EDE4
            elif dist < 38:
                ring = int(math.sin(dist * 0.3) * 1.2)
                idx = 3 if ring >= 0 else 2
                c = PALETTE_JADE[idx]
            else:
                c = PALETTE_JADE[2]

            # 极少微细玉脉
            if (x * 7 + y * 13) % 59 == 0:
                c = PALETTE_JADE[1]
            elif (x * 11 + y * 5) % 71 == 0:
                c = PALETTE_JADE[5]

            # 顶部柔和明暗
            if y < 3:
                c = PALETTE_JADE[min(5, PALETTE_JADE.index(c) + 1)] if c in PALETTE_JADE else c

            img.putpixel((x, y), (*c, 255))

    # 2. 【淡赭素雅包边 (64,0)~(128,48)】
    # 柔和淡赭金，无金属反光或高频条纹
    for x in range(64, 128):
        for y in range(48):
            u = x - 64
            edge_dist = min(u % 8, 7 - (u % 8))
            if edge_dist == 0:
                c = PALETTE_OCHRE[4]  # 素雅浅金边 #D6BF9E
            elif edge_dist == 1:
                c = PALETTE_OCHRE[3]  # 小麦柔金 #B59A76
            else:
                c = PALETTE_OCHRE[2]  # 浅木金 #8E7355

            # 极克制的木石纹理
            if (u + y * 2) % 24 == 0:
                c = PALETTE_OCHRE[1]

            img.putpixel((x, y), (*c, 255))

    # 3. 【白玉菌柄 (0,48)~(64,80)】
    # 纵向柔和素白玉质，底部 1px 接触软阴影
    for x in range(64):
        for y in range(48, 80):
            v = y - 48
            stripe = (x * 3) % 19
            if stripe < 3:
                c = PALETTE_JADE[2]
            elif stripe == 4:
                c = PALETTE_JADE[4]
            else:
                c = PALETTE_JADE[3]

            if v > 29:
                c = PALETTE_JADE[1]  # 根部淡阴影

            img.putpixel((x, y), (*c, 255))

    # 4. 【伞底淡赭菌孔层 (64,48)~(96,80)】
    for x in range(64, 96):
        for y in range(48, 80):
            u = x - 64
            v = y - 48
            if (u + v) % 3 == 0:
                c = PALETTE_OCHRE[3]
            elif (u % 2 == 0):
                c = PALETTE_OCHRE[2]
            else:
                c = PALETTE_OCHRE[1]
            img.putpixel((x, y), (*c, 255))

    # 5. 【微粒灵光尘屑 (96,48)~(128,80)】
    # 柔和淡金光点与月白微粒
    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            dx = abs(u - 16)
            dy = abs(v - 16)
            d = math.hypot(dx, dy)
            if d < 3.0:
                c = PALETTE_ICE_JADE[5]  # 月白微心 #ECFAF8
            elif d < 6.0:
                c = PALETTE_OCHRE[4]     # 淡金晕
            elif d < 10.0:
                c = PALETTE_OCHRE[2]
            else:
                c = (0, 0, 0)
                img.putpixel((x, y), (0, 0, 0, 0))
                continue
            img.putpixel((x, y), (*c, 255))

    # 6. 【苍古青灰岩基座 (0,80)~(64,128)】
    # 彻底去除科幻电路板线路！大面积水墨岩层（#687076）、天然风化断层、极淡地衣苔痕
    for x in range(64):
        for y in range(80, 128):
            u = x
            v = y - 80
            # 天然水墨岩层纹路 (横向缓坡层理)
            strata = (v * 2 + int(math.sin(u * 0.2) * 3)) % 14
            if strata < 3:
                c = PALETTE_SLATE[1]  # 深灰石纹
            elif strata < 9:
                c = PALETTE_SLATE[3]  # 主体浅灰岩面 #687076
            elif strata < 12:
                c = PALETTE_SLATE[2]  # 中灰石质
            else:
                c = PALETTE_SLATE[4]  # 浅灰棱线

            # 天然零星风化地衣苔痕 (不规则点缀)
            if (u * 13 + v * 19) % 37 == 0:
                c = PALETTE_SLATE[5]  # 苍绿地衣 #4E584A
            elif (u * 17 + v * 7) % 53 == 0:
                c = PALETTE_SLATE[0]  # 细微石缝

            img.putpixel((x, y), (*c, 255))

    # 7. 【天然冰种透青玉晶石与泉心 (64,80)~(128,128)】
    # 通透素雅水青色（#6CA29B -> #98CAC4 -> #ECFAF8），自然玉质折射，绝非刺眼霓虹
    for x in range(64, 128):
        for y in range(80, 128):
            u = x - 64
            v = y - 80
            dx = abs(u - 32)
            dy = abs(v - 24)
            d = math.hypot(dx, dy)

            if d < 6.0:
                c = PALETTE_ICE_JADE[5]  # 柔润月白泉心 #ECFAF8
            elif d < 14.0:
                c = PALETTE_ICE_JADE[4]  # 月白微青 #C6EAE6
            elif d < 22.0:
                c = PALETTE_ICE_JADE[3]  # 冰种透玉 #98CAC4
            elif d < 32.0:
                c = PALETTE_ICE_JADE[2]  # 水青色 #6CA29B
            elif d < 40.0:
                if (u + v * 2) % 9 < 2:
                    c = PALETTE_ICE_JADE[3]  # 天然玉棱反光
                else:
                    c = PALETTE_ICE_JADE[1]  # 沉静青绿
            else:
                c = PALETTE_ICE_JADE[0]

            img.putpixel((x, y), (*c, 255))

    return img


def create_emissive_texture() -> Image.Image:
    """生成柔和天然真元微光 Emissive 贴图 (去除刺眼高亮)。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 255))

    # 仅灵眼泉心中心产生温润微光
    for x in range(64, 128):
        for y in range(80, 128):
            u = x - 64
            v = y - 80
            d = math.hypot(u - 32, v - 24)
            if d < 6.0:
                val = 180  # 柔和 70% 灵气微光
            elif d < 12.0:
                val = int(180 * (1.0 - (d - 6.0) / 6.0))
            else:
                val = 0
            img.putpixel((x, y), (val, val, val, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "jade_cap": [0, 0, 64, 48],
        "soft_ochre_rim": [64, 0, 128, 48],
        "jade_stipe": [0, 48, 64, 80],
        "soft_gills": [64, 48, 96, 80],
        "spore_mote": [96, 48, 128, 80],
        "pale_slate": [0, 80, 64, 128],
        "ice_jade": [64, 80, 128, 128],
        "qi_soft_white": [88, 96, 104, 112],
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
        "name": "LingYanShiZhi",
        "model_identifier": "ling_yan_shi_zhi",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "ling_yan_shi_zhi.png",
                "name": "ling_yan_shi_zhi",
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
    bb_path = LOCAL_MODELS / "LingYanShiZhi.bbmodel"
    bb_path.write_text(json.dumps(bb_dict, indent=2), encoding="utf-8")
    print(f"✅ [素雅淡色古韵版] 生成 BBModel: {bb_path} (分辨率: {TEXTURE_RES}x{TEXTURE_RES}, 共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "ling_yan_shi_zhi_texture_remaster.png"
    create_texture().save(tex_path)
    print(f"✅ [素雅淡色古韵版] 导出高清主贴图: {tex_path}")

    emissive_path = PREVIEW_DIR / "ling_yan_shi_zhi_emissive.png"
    create_emissive_texture().save(emissive_path)
    print(f"✅ [素雅淡色古韵版] 导出柔和发光贴图: {emissive_path}")


if __name__ == "__main__":
    main()
