#!/usr/bin/env python3
"""生成灵眼石芝 (LingYanShiZhi) Blockbench .bbmodel 与三视图预览 (Round 3 Final Cut 终轮打磨)。

打磨重点 (Round 3 终极艺术化重构)：
1. 真实扇形/如意灵芝冠 (Arcuate Fan-Shaped Kidney Pileus):
   - 彻底重构伞盖几何：采用多边扇形圆弧外轮廓拼接（由扇核、内环弧面、外展扇翼、侧翼与同心环脊组成）。
   - 伞盖微呈碗状下凹/边缘下扣，表面带有凸起的同心生长环脊 (Concentric Growth Ridges)。
   - 主伞盖 (Main Jade Pileus)：高挑挺立至 Y=8.5~10.5，展幅约 9x8 格，尽显仙家至宝灵芝的雍容舒展。
   - 次伞盖 (Secondary Pileus)：依附于主柄侧下方 (Y=5.5~7.0)，阶梯呼应。
   - 幼芝嫩盖 (Sprouting Jade Button)：依附于岩缝灵眼核心 (Y=2.8~4.0)。
2. 内敛金润孔面 (Recessed Radiant Pore Underbelly):
   - 菌褶层严格内嵌于白玉伞盖下缘内侧（缩进 0.3~0.5），俯视不露破绽，仰视/侧视金光灿烂。
3. 优雅曲柄 (Graceful S-Curved Stipe):
   - 偏心侧生柄（Bracket Fungus Eccentric Stipe），从灵眼石缝拔地而起，呈 S 形微弯托举主冠。
4. 灵眼玄晶地台 (Spirit Eye Geode & Basalt Bed):
   - 包含玄武岩层叠岩台、破裂岩脊、以及簇生在岩缝间的青碧翡翠灵晶柱群。
5. 细腻 64x64 UV Atlas 烘焙：
   - 羊脂温润白玉、同心生长金环、半透明水润金边、放射金黄菌孔与翡翠灵晶高光。
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
TEXTURE_RES = 64
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c000c")


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


def part_basalt_and_spirit_eye() -> list[Cube]:
    """玄岩晶洞基座与青碧灵眼晶簇。"""
    cubes: list[Cube] = []

    # 1. 底部主玄武岩座 (Basalt Main Platform)
    cubes.append(
        Cube(
            "rock_base",
            "rock_base_main",
            (1.5, 0.0, 1.5),
            (14.5, 2.0, 14.5),
            "dark_rock",
            (0.0, 8.0, 0.0),
            (8.0, 0.0, 8.0),
        )
    )
    # 2. 侧翼层叠梯级岩板 (Step Slabs)
    cubes.append(
        Cube(
            "rock_base",
            "rock_step_left",
            (1.0, 0.0, 3.5),
            (5.5, 2.8, 12.0),
            "dark_rock",
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
            "dark_rock",
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
            "dark_rock",
            (0.0, 10.0, 0.0),
            (8.0, 0.0, 2.3),
        )
    )

    # 3. 灵眼青碧灵晶簇 (Spirit Eye Cyan Crystals)
    # 3.1 主晶柱 (向右后倾斜拔起)
    cubes.append(
        Cube(
            "crystal",
            "spirit_crystal_primary",
            (6.8, 1.2, 6.4),
            (8.6, 5.4, 8.2),
            "cyan_crystal",
            (14.0, 18.0, -12.0),
            (7.7, 1.2, 7.3),
        )
    )
    # 3.2 次晶柱
    cubes.append(
        Cube(
            "crystal",
            "spirit_crystal_sec",
            (4.6, 0.8, 7.6),
            (6.0, 4.0, 9.0),
            "cyan_crystal",
            (-16.0, -28.0, 20.0),
            (5.3, 1.0, 8.3),
        )
    )
    # 3.3 细碎小晶晶体
    cubes.append(
        Cube(
            "crystal",
            "spirit_crystal_shard_1",
            (9.6, 1.0, 5.0),
            (10.8, 3.2, 6.2),
            "cyan_crystal",
            (22.0, 40.0, -15.0),
            (10.2, 1.2, 5.6),
        )
    )
    cubes.append(
        Cube(
            "crystal",
            "spirit_crystal_shard_2",
            (4.0, 0.6, 4.4),
            (5.0, 2.4, 5.4),
            "cyan_crystal",
            (15.0, -45.0, 10.0),
            (4.5, 0.8, 4.9),
        )
    )
    # 3.4 灵眼泉眼核心溢光 (Qi Well Core Glow)
    cubes.append(
        Cube(
            "crystal",
            "qi_well_core_glow",
            (5.4, 0.4, 5.4),
            (10.6, 1.6, 10.6),
            "cyan_crystal",
        )
    )
    return cubes


def part_curved_stipes() -> list[Cube]:
    """高挑柔美的白玉菌柄 (S-Curved Jade Stipes)。"""
    cubes: list[Cube] = []

    # 1. 主芝白玉主柄 (Main Stipe - 从岩缝拔起，优雅向左前上方托起主冠)
    stipe_main_nodes = [
        (7.8, 1.6, 7.4),  # 灵眼晶簇生根
        (7.6, 3.4, 7.0),  # 垂直拔升
        (7.2, 5.2, 6.4),  # 向左前倾斜
        (6.6, 7.0, 5.6),  # 向上收拢
        (5.8, 8.6, 4.8),  # 托住主伞盖偏心下盘
    ]
    for i in range(len(stipe_main_nodes) - 1):
        p1 = stipe_main_nodes[i]
        p2 = stipe_main_nodes[i + 1]
        t = 1.0 - i * 0.1
        cubes.append(
            make_oriented_cube("stipe", f"main_stipe_seg_{i:02d}", p1, p2, t, "jade_stipe")
        )

    # 2. 次芝白玉副柄 (Secondary Stipe - 从主柄中下部分叉，向右后方托起次冠)
    stipe_sec_nodes = [
        (7.6, 3.4, 7.0),  # 分叉点
        (8.6, 4.6, 8.0),  # 向右后倾斜
        (9.8, 6.0, 9.0),  # 托住次伞盖
    ]
    for i in range(len(stipe_sec_nodes) - 1):
        p1 = stipe_sec_nodes[i]
        p2 = stipe_sec_nodes[i + 1]
        t = 0.8 - i * 0.1
        cubes.append(
            make_oriented_cube("stipe", f"sec_stipe_seg_{i:02d}", p1, p2, t, "jade_stipe")
        )

    # 3. 幼芝嫩玉柄 (Baby Stipe - 在最前低矮处生出)
    stipe_baby_nodes = [
        (6.0, 1.4, 5.2),
        (5.2, 2.6, 4.2),
    ]
    for i in range(len(stipe_baby_nodes) - 1):
        p1 = stipe_baby_nodes[i]
        p2 = stipe_baby_nodes[i + 1]
        cubes.append(
            make_oriented_cube("stipe", f"baby_stipe_seg_{i:02d}", p1, p2, 0.65, "jade_stipe")
        )

    return cubes


def part_fan_pileus_sculpture() -> list[Cube]:
    """真实扇形肾面白玉灵芝伞盖群 (主伞盖、次伞盖、幼芝嫩盖)。"""
    cubes: list[Cube] = []

    # ─────────────────────────────────────────────────────────────
    # 【1. 主灵芝扇形伞盖 (Main Jade Pileus)】
    # 位于 Y=8.4~9.6 高位，以柄接合点 (5.8, 8.6, 4.8) 为轴心，呈弧形向外辐射展开扇面！
    # ─────────────────────────────────────────────────────────────
    cap1_rot = (8.0, -26.0, 12.0)
    cap1_org = (5.8, 8.6, 4.8)

    # 1.1 伞核基盘 (Hilus Hub - 菌柄连接点)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_hub",
            (4.8, 8.6, 3.8),
            (7.2, 9.2, 6.2),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.2 前向正中主扇翼 (Central Fan Wing)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_center",
            (3.6, 8.7, 2.2),
            (6.8, 9.15, 4.2),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.3 左向延伸侧扇翼 (Left Fan Wing)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_left",
            (2.2, 8.7, 3.6),
            (4.4, 9.15, 6.8),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.4 右前外弧扇翼 (Right-Front Fan Wing)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_rf",
            (6.4, 8.7, 2.4),
            (8.8, 9.15, 4.8),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.5 左前斜角拼接扇片 (Left-Front Corner Fan)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_wing_lf",
            (2.6, 8.75, 2.6),
            (4.2, 9.18, 4.0),
            "jade_cap",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.6 表面同心凸起生长环脊 (Concentric Growth Ridge)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_growth_ridge_arc",
            (3.0, 9.1, 2.6),
            (7.6, 9.35, 5.4),
            "jade_rim",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.7 极外缘半透明金玉微卷边 (Translucent Golden Margin Lip)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_lip_front",
            (2.0, 8.65, 1.8),
            (8.6, 9.05, 2.4),
            "jade_rim",
            cap1_rot,
            cap1_org,
        )
    )
    cubes.append(
        Cube(
            "main_cap",
            "cap1_lip_left",
            (1.6, 8.65, 2.2),
            (2.2, 9.05, 7.2),
            "jade_rim",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.8 伞底内嵌式微金菌褶孔面 (Recessed Golden Pore Underbelly)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_gills_recessed",
            (2.6, 8.2, 2.4),
            (7.8, 8.6, 6.4),
            "golden_gills",
            cap1_rot,
            cap1_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【2. 次灵芝扇形伞盖 (Secondary Jade Pileus)】
    # 位于 Y=5.8~6.8 中层偏右后方，展幅约 7x6 格
    # ─────────────────────────────────────────────────────────────
    cap2_rot = (-12.0, 46.0, -8.0)
    cap2_org = (9.8, 6.0, 9.0)

    # 2.1 次伞盖核心
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_hub",
            (8.8, 6.0, 8.0),
            (11.8, 6.55, 10.8),
            "jade_cap",
            cap2_rot,
            cap2_org,
        )
    )
    # 2.2 次伞盖前展扇翼
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_wing_front",
            (9.8, 6.05, 6.8),
            (13.2, 6.5, 9.0),
            "jade_cap",
            cap2_rot,
            cap2_org,
        )
    )
    # 2.3 次伞盖右展扇翼
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_wing_right",
            (11.2, 6.05, 8.4),
            (13.8, 6.5, 11.4),
            "jade_cap",
            cap2_rot,
            cap2_org,
        )
    )
    # 2.4 次伞盖外缘金边
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_lip_outer",
            (9.4, 6.0, 6.2),
            (14.2, 6.42, 6.8),
            "jade_rim",
            cap2_rot,
            cap2_org,
        )
    )
    # 2.5 次伞底内嵌微金孔面
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_gills_recessed",
            (9.2, 5.6, 7.2),
            (13.0, 6.0, 10.4),
            "golden_gills",
            cap2_rot,
            cap2_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【3. 幼芝嫩玉伞盖 (Baby Jade Sprout)】
    # 位于 Y=2.6~3.4 正前方低矮处，展幅约 4x4 格
    # ─────────────────────────────────────────────────────────────
    cap3_rot = (18.0, -15.0, 10.0)
    cap3_org = (5.2, 2.6, 4.2)

    cubes.append(
        Cube(
            "baby_cap",
            "cap3_fan_body",
            (4.0, 2.6, 2.8),
            (6.6, 3.05, 5.2),
            "jade_cap",
            cap3_rot,
            cap3_org,
        )
    )
    cubes.append(
        Cube(
            "baby_cap",
            "cap3_lip_front",
            (3.4, 2.6, 2.2),
            (6.2, 2.98, 2.8),
            "jade_rim",
            cap3_rot,
            cap3_org,
        )
    )
    cubes.append(
        Cube(
            "baby_cap",
            "cap3_gills",
            (4.2, 2.2, 3.0),
            (6.0, 2.6, 4.8),
            "golden_gills",
            cap3_rot,
            cap3_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【4. 灵眼真元孢子光晕 (Spirit Spores & Qi Halos)】
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        Cube(
            "spores",
            "spore_halo_1",
            (2.8, 7.6, 2.2),
            (3.4, 8.2, 2.8),
            "spore_halo",
            (15.0, 30.0, 45.0),
            (3.1, 7.9, 2.5),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "spore_halo_2",
            (8.4, 8.0, 2.0),
            (9.0, 8.6, 2.6),
            "spore_halo",
            (-20.0, 45.0, 15.0),
            (8.7, 8.3, 2.3),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "spore_halo_3",
            (11.2, 5.2, 6.4),
            (11.8, 5.8, 7.0),
            "spore_halo",
            (10.0, -25.0, 30.0),
            (11.5, 5.5, 6.7),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "spore_halo_4",
            (4.8, 10.0, 3.8),
            (5.4, 10.6, 4.4),
            "spore_halo",
            (-30.0, 10.0, 60.0),
            (5.1, 10.3, 4.1),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_basalt_and_spirit_eye()
        + part_curved_stipes()
        + part_fan_pileus_sculpture()
    )


def create_texture() -> Image.Image:
    """生成 64x64 UV Atlas 贴图 (温润白玉与灵气同心纹终极烘焙)。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 白玉菌盖上表面 (0,0)~(32,24) (细腻温润羊脂玉 + 环形同心生长轮)
    for x in range(32):
        for y in range(24):
            noise = ((x * 17 + y * 23) % 9)
            dist = math.hypot(x - 16, y - 2)
            # 细腻灵气生长同心环纹 (Concentric Growth Rings)
            ring = int(math.sin(dist * 0.85) * 6)
            r = min(255, 242 + noise + ring)
            g = min(255, 238 + noise + ring)
            b = min(255, 226 + (noise // 2) + ring)
            # 半透明淡金水润光斑
            if (x * 7 + y * 11) % 23 == 0:
                r, g, b = min(255, r + 8), min(255, g + 5), max(0, b - 10)
            img.putpixel((x, y), (r, g, b, 255))

    # 2. 菌盖外缘金玉波浪卷唇 (32,0)~(64,24)
    for x in range(32, 64):
        for y in range(24):
            u = x - 32
            grad = (u + y) / 45.0
            r = min(255, int(248 + grad * 7))
            g = min(255, int(238 + grad * 8))
            b = min(255, int(205 + grad * 35))
            if u % 3 == 0 or y % 4 == 0:
                r, g, b = min(255, r + 5), min(255, g + 5), min(255, b + 5)
            img.putpixel((x, y), (r, g, b, 255))

    # 3. 白玉菌柄 (0,24)~(32,40)
    for x in range(32):
        for y in range(24, 40):
            noise = ((x * 19 + y * 29) % 11)
            base = 228 + noise
            r = min(255, base + 6)
            g = min(255, base + 4)
            b = max(0, base - 8)
            if (x + y * 2) % 6 == 0:
                # 浅青白玉丝
                r, g, b = r - 12, g + 3, b + 6
            img.putpixel((x, y), (r, g, b, 255))

    # 4. 金色放射菌孔层 (32,24)~(48,40)
    for x in range(32, 48):
        for y in range(24, 40):
            u = x - 32
            if u % 2 == 0:
                r, g, b = 255, 222, 60  # 鲜亮金黄
            else:
                r, g, b = 180, 135, 30  # 暗金底色
            img.putpixel((x, y), (r, g, b, 255))

    # 5. 悬浮金孢子微粒 (48,24)~(64,40)
    for x in range(48, 64):
        for y in range(24, 40):
            dx = abs(x - 56)
            dy = abs(y - 32)
            d = math.hypot(dx, dy)
            if d < 1.8:
                img.putpixel((x, y), (255, 252, 240, 255))  # 白金核心
            elif d < 4.5:
                img.putpixel((x, y), (255, 218, 70, 255))  # 金芒光晕
            else:
                img.putpixel((x, y), (150, 105, 20, 255))

    # 6. 玄武岩玄石基座 (0,40)~(32,64)
    for x in range(32):
        for y in range(40, 64):
            noise = ((x * 17 + y * 31) % 19)
            val = 42 + noise
            r, g, b = val - 4, val, val + 6
            if (x * 2 + y) % 9 == 0:
                r, g, b = 40, 175, 140  # 石缝青碧微光
            img.putpixel((x, y), (r, g, b, 255))

    # 7. 灵眼青碧灵晶 (32,40)~(64,64)
    for x in range(32, 64):
        for y in range(40, 64):
            u = x - 32
            v = y - 40
            diag = (u * 2 + v * 3) % 15
            crystal_light = int(math.sin(diag / 15.0 * math.pi * 2) * 35)
            r = max(0, min(255, 45 + crystal_light // 2))
            g = max(0, min(255, 225 + crystal_light))
            b = max(0, min(255, 180 + crystal_light))
            if diag == 7:
                r, g, b = 210, 255, 245  # 晶体高光棱边
            img.putpixel((x, y), (r, g, b, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "jade_cap": [0, 0, 32, 24],
        "jade_rim": [32, 0, 64, 24],
        "jade_stipe": [0, 24, 32, 40],
        "golden_gills": [32, 24, 48, 40],
        "spore_halo": [48, 24, 64, 40],
        "dark_rock": [0, 40, 32, 64],
        "cyan_crystal": [32, 40, 64, 64],
    }

    elements = []
    outliner_bones: dict[str, list[str]] = {}

    for c in cubes:
        elem_uuid = stable_uuid("elem", c.name)
        uv = uv_presets.get(c.uv_preset, [0, 0, 16, 16])

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
    print(f"✅ [Round 3 Final] 生成 BBModel: {bb_path} (共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "ling_yan_shi_zhi_texture_r3.png"
    create_texture().save(tex_path)
    print(f"✅ [Round 3 Final] 导出贴图: {tex_path}")


if __name__ == "__main__":
    main()
