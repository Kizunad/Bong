#!/usr/bin/env python3
"""生成固元根 (GuYuanGen) Blockbench .bbmodel 与三视图预览 (地生盘龙老桩与多片仙草复叶重构版)。

针对当前问题的深度重构：
1. 块根体量与盘结感重构 (Ancient Gnarled Burl Root):
   - 彻底打破“立式萝卜桩”——老根应该是横卧在地表、盘曲隆起的苍老树瘤老桩 (Burl / Rhizome)！
   - 高度压低，水平横展，形成主根盘结拱起、左右虬根扣石的雄浑老桩地势。
2. 仙草叶片丰满度与层次 (Lush Compound Foliage):
   - 之前叶片太高太细、像豆芽。
   - 现在重构为主生短粗叶柄 + 5 片层叠舒展的心卵形厚质叶片（带自然弧度向四方微微垂下），充满生机与仙草药力。
3. 枯荣对比强化：
   - 下部焦赤暗红沧桑老根 vs 上部青翠碧绿欲滴仙草。
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
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c0015")


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


def make_oriented_segment(
    bone: str,
    name: str,
    p1: tuple[float, float, float],
    p2: tuple[float, float, float],
    width: float = 1.6,
    thickness: float = 1.2,
    uv_preset: str = "tuber_bark",
) -> Cube:
    """在空间中两点之间创建定向段。"""
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


def part_earth_bed() -> list[Cube]:
    """枯土与地脉暗岩碎石地台。"""
    cubes: list[Cube] = []

    # 1. 底部主深土石台
    cubes.append(
        Cube(
            "earth_bed",
            "earth_slab_main",
            (1.5, 0.0, 1.5),
            (14.5, 1.8, 14.5),
            "earth_soil",
            (0.0, 8.0, 0.0),
            (8.0, 0.0, 8.0),
        )
    )
    # 2. 侧向隆起风化土垄与碎石
    cubes.append(
        Cube(
            "earth_bed",
            "earth_ridge_left",
            (1.0, 0.0, 3.2),
            (5.6, 2.4, 11.8),
            "earth_soil",
            (-4.0, -15.0, 6.0),
            (3.3, 0.0, 7.5),
        )
    )
    cubes.append(
        Cube(
            "earth_bed",
            "earth_ridge_back",
            (8.2, 0.0, 8.8),
            (14.8, 2.8, 14.6),
            "earth_soil",
            (6.0, 24.0, -5.0),
            (11.5, 0.0, 11.7),
        )
    )
    cubes.append(
        Cube(
            "earth_bed",
            "earth_front_mound",
            (4.8, 0.0, 1.0),
            (11.2, 1.4, 3.8),
            "earth_soil",
            (0.0, 6.0, 0.0),
            (8.0, 0.0, 2.4),
        )
    )
    return cubes


def part_gnarled_dragon_tuber() -> list[Cube]:
    """苍老横卧盘虬的赤火大块根 (横展厚实老桩)。"""
    cubes: list[Cube] = []

    # 1. 主根中央厚实隆起老桩躯干 (Main Burl Core)
    cubes.append(
        Cube(
            "tuber_root",
            "tuber_central_burl_core",
            (5.2, 0.8, 5.2),
            (10.8, 3.8, 10.8),
            "tuber_bark",
            (4.0, 15.0, -6.0),
            (8.0, 2.0, 8.0),
        )
    )
    # 2. 老桩前侧膨大盘根 (Front Tuber Bulb)
    cubes.append(
        Cube(
            "tuber_root",
            "tuber_front_bulb",
            (5.6, 1.0, 3.6),
            (9.6, 3.4, 6.4),
            "tuber_bark",
            (-6.0, -12.0, 8.0),
            (7.6, 2.0, 5.0),
        )
    )
    # 3. 老桩后侧隆起节瘤 (Back Tuber Burl)
    cubes.append(
        Cube(
            "tuber_root",
            "tuber_back_burl",
            (6.4, 1.2, 9.2),
            (10.8, 3.6, 12.4),
            "tuber_bark",
            (10.0, 25.0, -5.0),
            (8.6, 2.4, 10.8),
        )
    )

    # 4. 4 条如龙爪般扣地的侧向老虬根
    # 4.1 右前爪根
    p_rf1 = (9.2, 2.2, 5.4)
    p_rf2 = (11.4, 1.2, 4.0)
    p_rf3 = (13.2, 0.0, 2.6)
    cubes.append(make_oriented_segment("tuber_root", "claw_rf_1", p_rf1, p_rf2, 1.6, 1.3, "tuber_bark"))
    cubes.append(make_oriented_segment("tuber_root", "claw_rf_2", p_rf2, p_rf3, 1.2, 0.9, "tuber_bark"))

    # 4.2 右后爪根
    p_rb1 = (9.8, 2.0, 8.6)
    p_rb2 = (11.8, 1.2, 10.2)
    p_rb3 = (13.6, 0.0, 11.4)
    cubes.append(make_oriented_segment("tuber_root", "claw_rb_1", p_rb1, p_rb2, 1.5, 1.2, "tuber_bark"))
    cubes.append(make_oriented_segment("tuber_root", "claw_rb_2", p_rb2, p_rb3, 1.1, 0.8, "tuber_bark"))

    # 4.3 左前出土斜根
    p_lf1 = (5.8, 1.8, 4.6)
    p_lf2 = (4.0, 0.8, 3.4)
    p_lf3 = (2.4, 0.0, 2.2)
    cubes.append(make_oriented_segment("tuber_root", "claw_lf_1", p_lf1, p_lf2, 1.4, 1.1, "tuber_bark"))
    cubes.append(make_oriented_segment("tuber_root", "claw_lf_2", p_lf2, p_lf3, 1.0, 0.7, "tuber_bark"))

    # 4.4 左后大支柱根
    p_lb1 = (5.6, 1.6, 8.4)
    p_lb2 = (3.8, 0.8, 9.6)
    p_lb3 = (2.2, 0.0, 10.8)
    cubes.append(make_oriented_segment("tuber_root", "claw_lb_1", p_lb1, p_lb2, 1.5, 1.2, "tuber_bark"))
    cubes.append(make_oriented_segment("tuber_root", "claw_lb_2", p_lb2, p_lb3, 1.1, 0.8, "tuber_bark"))

    # 5. 顶端老根出芽节瘤座 (Crown Sprouting Node)
    cubes.append(
        Cube(
            "tuber_root",
            "tuber_crown_sprout_node",
            (6.6, 3.2, 6.2),
            (9.0, 4.6, 8.6),
            "tuber_bark",
            (5.0, -8.0, 6.0),
            (7.8, 3.8, 7.4),
        )
    )

    return cubes


def part_lush_vital_foliage() -> list[Cube]:
    """舒展丰满的青翠仙草复叶丛 (5片层叠舒展心卵形仙叶与顶生新芽)。"""
    cubes: list[Cube] = []
    # 发芽基准原点 (位于老桩顶部 Y=4.4)
    bx, by, bz = 7.8, 4.4, 7.4

    # 1. 粗壮短促的草木嫩茎 (Short Vital Stalk)
    cubes.append(
        Cube(
            "fresh_sprouts",
            "vital_stalk_core",
            (7.3, by, bz - 0.5),
            (8.3, by + 1.8, bz + 0.5),
            "fresh_stem",
            (6.0, -10.0, 5.0),
            (bx, by, bz),
        )
    )

    # 叶丛分叉中心
    cx, cy, cz = 7.7, by + 1.6, bz

    # ─────────────────────────────────────────────────────────────
    # 【叶片 1：前向主叶 (Front Main Leaflet)】
    # 向前方舒展微垂
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_front_base",
            (cx, cy, cz),
            (7.5, cy + 1.4, cz - 2.4),
            width=2.4,
            thickness=0.35,
            uv_preset="fresh_leaf_green",
        )
    )
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_front_tip",
            (7.5, cy + 1.4, cz - 2.4),
            (7.2, cy + 1.8, cz - 4.4),
            width=1.6,
            thickness=0.30,
            uv_preset="fresh_leaf_green",
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【叶片 2：左前侧叶 (Left-Front Leaflet)】
    # 向左前方展开
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_lf_base",
            (cx, cy, cz),
            (5.4, cy + 1.5, cz - 1.4),
            width=2.2,
            thickness=0.35,
            uv_preset="fresh_leaf_green",
        )
    )
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_lf_tip",
            (5.4, cy + 1.5, cz - 1.4),
            (3.6, cy + 2.0, cz - 2.4),
            width=1.4,
            thickness=0.30,
            uv_preset="fresh_leaf_green",
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【叶片 3：右前侧叶 (Right-Front Leaflet)】
    # 向右前方展开
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_rf_base",
            (cx, cy, cz),
            (9.8, cy + 1.6, cz - 1.0),
            width=2.2,
            thickness=0.35,
            uv_preset="fresh_leaf_green",
        )
    )
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_rf_tip",
            (9.8, cy + 1.6, cz - 1.0),
            (11.6, cy + 2.2, cz - 1.8),
            width=1.4,
            thickness=0.30,
            uv_preset="fresh_leaf_green",
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【叶片 4：左后侧叶 (Left-Back Leaflet)】
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_lb_base",
            (cx, cy, cz),
            (5.6, cy + 1.4, cz + 1.6),
            width=2.0,
            thickness=0.35,
            uv_preset="fresh_leaf_green",
        )
    )
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_lb_tip",
            (5.6, cy + 1.4, cz + 1.6),
            (4.0, cy + 1.8, cz + 2.8),
            width=1.3,
            thickness=0.30,
            uv_preset="fresh_leaf_green",
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【叶片 5：右后侧叶 (Right-Back Leaflet)】
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_rb_base",
            (cx, cy, cz),
            (9.6, cy + 1.5, cz + 1.4),
            width=2.0,
            thickness=0.35,
            uv_preset="fresh_leaf_green",
        )
    )
    cubes.append(
        make_oriented_segment(
            "fresh_sprouts",
            "leaf_rb_tip",
            (9.6, cy + 1.5, cz + 1.4),
            (11.2, cy + 2.0, cz + 2.4),
            width=1.3,
            thickness=0.30,
            uv_preset="fresh_leaf_green",
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【6. 顶端直立初生幼芽 (Central Vital Bud Apex)】
    # 在叶丛正中心拔起直立嫩芽
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        Cube(
            "fresh_sprouts",
            "central_vital_bud",
            (cx - 0.45, cy + 0.2, cz - 0.45),
            (cx + 0.45, cy + 2.6, cz + 0.45),
            "fresh_leaf_tip",
            (12.0, 20.0, -10.0),
            (cx, cy + 1.0, cz),
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【7. 仙草叶尖晶莹灵露微粒 (Dewdrop Moteparticles)】
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        Cube(
            "spores",
            "dew_mote_1",
            (7.0, cy + 2.0, cz - 4.6),
            (7.4, cy + 2.4, cz - 4.2),
            "dew_particle",
            (10.0, 20.0, 30.0),
            (7.2, cy + 2.2, cz - 4.4),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "dew_mote_2",
            (3.4, cy + 2.2, cz - 2.6),
            (3.8, cy + 2.6, cz - 2.2),
            "dew_particle",
            (-15.0, 45.0, 20.0),
            (3.6, cy + 2.4, cz - 2.4),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "dew_mote_3",
            (11.4, cy + 2.4, cz - 2.0),
            (11.8, cy + 2.8, cz - 1.6),
            "dew_particle",
            (25.0, -35.0, 40.0),
            (11.6, cy + 2.6, cz - 1.8),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_earth_bed()
        + part_gnarled_dragon_tuber()
        + part_lush_vital_foliage()
    )


def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


# 【古朴沉香枯荣色系 (GuYuanGen Palettes)】

# 1. 沉香老根树皮 (Aged Crimson Tuber Bark)
PALETTE_BARK = [
    hex_to_rgb("#201210"),  # 0 炭黑深阴影
    hex_to_rgb("#3A1B16"),  # 1 焦赤暗木
    hex_to_rgb("#5E221A"),  # 2 沉红老根 (#5E221A 主体)
    hex_to_rgb("#822C20"),  # 3 熟赭赤色
    hex_to_rgb("#A63826"),  # 4 根棱浅赤
    hex_to_rgb("#D84C2C"),  # 5 裂隙微赤火
]

# 2. 青翠新生叶片 (Fresh Vital Leaf Green - 枯荣对立)
PALETTE_SPROUT = [
    hex_to_rgb("#1C4424"),  # 0 深翠叶脉
    hex_to_rgb("#2C6636"),  # 1 浓绿叶背
    hex_to_rgb("#44904E"),  # 2 纯净草绿
    hex_to_rgb("#62B66C"),  # 3 青翠嫩绿 (主体)
    hex_to_rgb("#8CE094"),  # 4 嫩芽高光
    hex_to_rgb("#D2FFD6"),  # 5 灵露月白光
]

# 3. 枯土石基 (Loam Earth)
PALETTE_EARTH = [
    hex_to_rgb("#201B18"),  # 0 极暗腐殖土
    hex_to_rgb("#322A25"),  # 1 深褐荒土
    hex_to_rgb("#483C35"),  # 2 中褐土层
    hex_to_rgb("#5E4F46"),  # 3 风化干土面
    hex_to_rgb("#766559"),  # 4 碎石浅棱
]


def create_texture() -> Image.Image:
    """生成 128x128 高清赤根青芽水墨像素手绘 UV Atlas。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 【沉香老根树皮 (0,0)~(64,48)】
    for x in range(64):
        for y in range(48):
            u = x
            v = y
            bark_fiber = (u * 3 + int(math.sin(v * 0.4) * 4)) % 14
            if bark_fiber < 3:
                c = PALETTE_BARK[1]
            elif bark_fiber < 9:
                c = PALETTE_BARK[2]
            elif bark_fiber < 12:
                c = PALETTE_BARK[3]
            else:
                c = PALETTE_BARK[4]

            if (u + v * 3) % 31 == 0:
                c = PALETTE_BARK[5]
            elif (u * 7 + v * 11) % 47 == 0:
                c = PALETTE_BARK[0]

            img.putpixel((x, y), (*c, 255))

    # 2. 【老桩节瘤与暗凹 (64,0)~(128,48)】
    for x in range(64, 128):
        for y in range(48):
            u = x - 64
            v = y
            dist = math.hypot(u - 32, v - 24)
            if dist < 6:
                c = PALETTE_BARK[5]
            elif dist < 20:
                c = PALETTE_BARK[2]
            else:
                c = PALETTE_BARK[1]

            if (u * 2 + v * 2) % 17 == 0:
                c = PALETTE_BARK[4]

            img.putpixel((x, y), (*c, 255))

    # 3. 【青翠复叶表面 (0,48)~(64,80)】
    for x in range(64):
        for y in range(48, 80):
            u = x
            v = y - 48
            if u == 32:
                c = PALETTE_SPROUT[4]  # 浅翠主脉 #8CE094
            elif abs(u - 32) <= 3:
                c = PALETTE_SPROUT[3]  # 青翠嫩绿 #62B66C
            elif (u * 3 + v * 2) % 19 == 0:
                c = PALETTE_SPROUT[1]
            else:
                c = PALETTE_SPROUT[2]  # 纯净草绿

            if u < 3 or u > 60:
                c = PALETTE_SPROUT[4]

            img.putpixel((x, y), (*c, 255))

    # 4. 【嫩茎与叶尖 (64,48)~(96,80)】
    for x in range(64, 96):
        for y in range(48, 80):
            u = x - 64
            v = y - 48
            if v < 16:
                c = PALETTE_SPROUT[4]
            else:
                c = PALETTE_SPROUT[3]
            if u % 4 == 0:
                c = PALETTE_SPROUT[2]
            img.putpixel((x, y), (*c, 255))

    # 5. 【晶莹灵露微粒 (96,48)~(128,80)】
    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            d = math.hypot(u - 16, v - 16)
            if d < 3.5:
                c = PALETTE_SPROUT[5]
            elif d < 7.0:
                c = PALETTE_SPROUT[4]
            else:
                img.putpixel((x, y), (0, 0, 0, 0))
                continue
            img.putpixel((x, y), (*c, 255))

    # 6. 【枯土石基地台 (0,80)~(64,128)】
    for x in range(64):
        for y in range(80, 128):
            u = x
            v = y - 80
            strata = (v * 2 + int(math.sin(u * 0.3) * 3)) % 14
            if strata < 3:
                c = PALETTE_EARTH[1]
            elif strata < 9:
                c = PALETTE_EARTH[2]
            elif strata < 12:
                c = PALETTE_EARTH[3]
            else:
                c = PALETTE_EARTH[4]

            if (u * 9 + v * 13) % 31 == 0:
                c = PALETTE_EARTH[0]

            img.putpixel((x, y), (*c, 255))

    # 7. 【地下地脉暗红土层 (64,80)~(128,128)】
    for x in range(64, 128):
        for y in range(80, 128):
            u = x - 64
            v = y - 80
            d = math.hypot(u - 32, v - 24)
            if d < 8.0:
                c = PALETTE_BARK[4]
            elif d < 22.0:
                c = PALETTE_BARK[2]
            else:
                c = PALETTE_EARTH[1]

            if (u * 2 + v * 3) % 19 == 0:
                c = PALETTE_BARK[5]

            img.putpixel((x, y), (*c, 255))

    return img


def create_emissive_texture() -> Image.Image:
    """生成固元根地火赤脉微光 Emissive 贴图。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 255))

    for x in range(64):
        for y in range(48):
            u = x
            v = y
            val = 0
            if (u + v * 3) % 31 == 0:
                val = 180
            img.putpixel((x, y), (val, val // 3, 0, 255))

    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            d = math.hypot(u - 16, v - 16)
            val = 160 if d < 3.5 else 0
            img.putpixel((x, y), (val // 2, val, val // 2, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "tuber_bark": [0, 0, 64, 48],
        "tuber_crevice": [64, 0, 128, 48],
        "fresh_leaf_green": [0, 48, 64, 80],
        "fresh_stem": [64, 48, 96, 80],
        "fresh_leaf_tip": [64, 48, 96, 80],
        "dew_particle": [96, 48, 128, 80],
        "earth_soil": [0, 80, 64, 128],
        "deep_qi_rock": [64, 80, 128, 128],
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
        "name": "GuYuanGen",
        "model_identifier": "gu_yuan_gen",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "gu_yuan_gen.png",
                "name": "gu_yuan_gen",
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
    bb_path = LOCAL_MODELS / "GuYuanGen.bbmodel"
    bb_path.write_text(json.dumps(bb_dict, indent=2), encoding="utf-8")
    print(f"✅ [盘龙老根与仙叶重构版] 生成 BBModel: {bb_path} (分辨率: {TEXTURE_RES}x{TEXTURE_RES}, 共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "gu_yuan_gen_texture.png"
    create_texture().save(tex_path)
    print(f"✅ 导出高清主贴图: {tex_path}")

    emissive_path = PREVIEW_DIR / "gu_yuan_gen_emissive.png"
    create_emissive_texture().save(emissive_path)
    print(f"✅ 导出地火微光贴图: {emissive_path}")


if __name__ == "__main__":
    main()
