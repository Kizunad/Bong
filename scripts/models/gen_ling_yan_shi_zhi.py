#!/usr/bin/env python3
"""生成灵眼石芝 (LingYanShiZhi) Blockbench .bbmodel 与三视图预览 (次世代美术品质升级版 - 顶级如意玉芝定稿)。

核心视觉优化：
1. 伞盖结构彻底如意扇形化与白玉全覆盖：
   - 彻底废弃生硬的平铺阶梯框，将主伞盖重构为 5 片轻薄优雅扇叶（厚度 0.35 格），层层平滑外展。
   - 伞盖上表面 100% 呈现温润白玉材质，带有从中心辐射与同心生长的温润光泽与浅金玉纹。
   - 仅在最外缘（极薄 0.25 格）勾勒一圈仙道法宝级古金暗纹卷边。
2. 伞底菌孔层内敛收束：
   - 菌孔层（Golden Gills）完全收缩在伞底内部，厚度仅 0.15 格，平视和俯视时完全隐藏，仅仰视透出仙韵微金。
3. 灵眼泉眼核心与灵晶簇：
   - 青碧灵晶高亮簇生，基座玄岩非对称阵纹静穆深沉。
4. 128x128 像素级 UV Atlas 与 Emissive 黑白发光贴图。
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
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c000f")


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
    """玄武岩阵纹基座与青碧灵眼晶簇。"""
    cubes: list[Cube] = []

    # 1. 底部主玄武岩座
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
    # 2. 侧翼梯级岩板
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

    # 3. 灵眼青碧灵晶簇
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
    cubes.append(
        Cube(
            "crystal",
            "qi_well_core_glow",
            (5.4, 0.4, 5.4),
            (10.6, 1.6, 10.6),
            "qi_core_white",
        )
    )
    return cubes


def part_curved_stipes() -> list[Cube]:
    """高挑柔美的温润羊脂白玉菌柄。"""
    cubes: list[Cube] = []

    # 1. 主芝白玉主柄 (S 形微弯托举主冠)
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
        t = 1.0 - i * 0.1
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
        t = 0.8 - i * 0.1
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
            make_oriented_cube("stipe", f"baby_stipe_seg_{i:02d}", p1, p2, 0.65, "jade_stipe")
        )

    return cubes


def part_fan_pileus_sculpture() -> list[Cube]:
    """如意扇面白玉灵芝伞盖群（全白玉如意扇面 + 极细金包边 + 完全内收菌孔）。"""
    cubes: list[Cube] = []

    # ─────────────────────────────────────────────────────────────
    # 【1. 主灵芝扇形伞盖 (Main Jade Pileus)】
    # 核心高度 Y=8.8~9.25，展幅 9x8，舒展如意扇形
    # ─────────────────────────────────────────────────────────────
    cap1_rot = (8.0, -26.0, 12.0)
    cap1_org = (5.8, 8.6, 4.8)

    # 1.1 伞核中盘 (Hilus Center - 白玉)
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
    # 1.2 前向正中主扇翼 (Central Fan Wing - 白玉)
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
    # 1.3 左向侧扇翼 (Left Fan Wing - 白玉)
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
    # 1.4 右前外弧扇翼 (Right-Front Fan Wing - 白玉)
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
    # 1.5 左前角扇片 (Left-Front Corner Wing - 白玉)
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
    # 1.6 极外沿古金微卷边 (极薄微细包边，厚度 0.3，紧贴外弧)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_lip_front",
            (2.0, 8.72, 1.7),
            (8.8, 9.10, 2.1),
            "ancient_gold_rim",
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
            "ancient_gold_rim",
            cap1_rot,
            cap1_org,
        )
    )
    # 1.7 伞底深度内嵌菌孔面 (厚度仅 0.15，彻底收缩于伞底内部)
    cubes.append(
        Cube(
            "main_cap",
            "cap1_gills_recessed",
            (3.0, 8.58, 2.8),
            (6.8, 8.75, 5.8),
            "golden_gills",
            cap1_rot,
            cap1_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【2. 次灵芝扇形伞盖 (Secondary Jade Pileus)】
    # 位于 Y=5.8~6.3 中层偏右后方
    # ─────────────────────────────────────────────────────────────
    cap2_rot = (-12.0, 46.0, -8.0)
    cap2_org = (9.8, 6.0, 9.0)

    # 2.1 次伞盖核心 (白玉)
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
    # 2.2 次伞盖前展扇翼 (白玉)
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
    # 2.3 次伞盖右展扇翼 (白玉)
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
    # 2.4 次伞盖外缘古金卷边 (极薄)
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_lip_outer",
            (9.4, 5.92, 6.3),
            (14.2, 6.30, 6.7),
            "ancient_gold_rim",
            cap2_rot,
            cap2_org,
        )
    )
    # 2.5 次伞底内嵌微金孔面 (内收)
    cubes.append(
        Cube(
            "sec_cap",
            "cap2_gills_recessed",
            (9.6, 5.80, 7.6),
            (12.6, 5.95, 10.0),
            "golden_gills",
            cap2_rot,
            cap2_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【3. 幼芝嫩玉伞盖 (Baby Jade Sprout)】
    # 位于 Y=2.6~3.0 正前方低矮处
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
            "ancient_gold_rim",
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
            "golden_gills",
            cap3_rot,
            cap3_org,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【4. 浮空阵法符印节点 (Floating Qi Talisman Nodes - 精巧微粒)】
    # ─────────────────────────────────────────────────────────────
    cubes.append(
        Cube(
            "spores",
            "talisman_node_1",
            (2.8, 7.6, 2.2),
            (3.3, 8.1, 2.7),
            "talisman_node",
            (15.0, 30.0, 45.0),
            (3.05, 7.85, 2.45),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "talisman_node_2",
            (8.4, 8.0, 2.0),
            (8.9, 8.5, 2.5),
            "talisman_node",
            (-20.0, 45.0, 15.0),
            (8.65, 8.25, 2.25),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "talisman_node_3",
            (11.2, 5.2, 6.4),
            (11.7, 5.7, 6.9),
            "talisman_node",
            (10.0, -25.0, 30.0),
            (11.45, 5.45, 6.65),
        )
    )
    cubes.append(
        Cube(
            "spores",
            "talisman_node_4",
            (4.8, 10.0, 3.8),
            (5.3, 10.5, 4.3),
            "talisman_node",
            (-30.0, 10.0, 60.0),
            (5.05, 10.25, 4.05),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_basalt_and_spirit_eye()
        + part_curved_stipes()
        + part_fan_pileus_sculpture()
    )


def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


PALETTE_JADE = [
    hex_to_rgb("#6F7069"),  # 0 最深阴影
    hex_to_rgb("#92928A"),  # 1 深灰玉色
    hex_to_rgb("#B9B7AA"),  # 2 中间色
    hex_to_rgb("#D8D4C2"),  # 3 主体象牙白
    hex_to_rgb("#EEE8D3"),  # 4 高光
    hex_to_rgb("#FFF4D8"),  # 5 极少高光
]

PALETTE_GOLD = [
    hex_to_rgb("#33220D"),  # 0 最深凹槽
    hex_to_rgb("#553812"),  # 1 暗铜
    hex_to_rgb("#765118"),  # 2 古金
    hex_to_rgb("#9A711D"),  # 3 中间金
    hex_to_rgb("#C49A32"),  # 4 华彩金
    hex_to_rgb("#E2C15C"),  # 5 边缘高光
]

PALETTE_BASALT = [
    hex_to_rgb("#111817"),  # 0 最深黑底
    hex_to_rgb("#172220"),  # 1 深灰玄石
    hex_to_rgb("#202C29"),  # 2 中间黑绿
    hex_to_rgb("#283531"),  # 3 暗青面
    hex_to_rgb("#33403B"),  # 4 亮青灰棱
]

PALETTE_QI = [
    hex_to_rgb("#0C746B"),  # 0 暗青
    hex_to_rgb("#109D8B"),  # 1 青绿
    hex_to_rgb("#20CDB0"),  # 2 亮青
    hex_to_rgb("#48E8CC"),  # 3 灵气青
    hex_to_rgb("#83FBE4"),  # 4 炽光青
    hex_to_rgb("#D0FFF5"),  # 5 核心白青
]


def create_texture() -> Image.Image:
    """生成 128x128 高清像素手绘 UV Atlas。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 【白玉菌盖上表面 (0,0)~(64,48)】
    # 75% 纯净象牙白 #D8D4C2，15% 中间色 #B9B7AA，10% 高光 #EEE8D3，温润大色块
    for x in range(64):
        for y in range(48):
            dist = math.hypot(x - 32, y - 6)
            if dist < 14:
                c = PALETTE_JADE[4]  # 高光中心
            elif dist < 36:
                ring = int(math.sin(dist * 0.35) * 1.5)
                idx = 3 if ring >= 0 else 2
                c = PALETTE_JADE[idx]
            else:
                c = PALETTE_JADE[2]

            if (x * 7 + y * 13) % 47 == 0:
                c = PALETTE_JADE[1]
            elif (x * 11 + y * 5) % 59 == 0:
                c = PALETTE_JADE[5]

            # 顶部微定向光
            if y < 4:
                c = PALETTE_JADE[min(5, PALETTE_JADE.index(c) + 1)] if c in PALETTE_JADE else c

            img.putpixel((x, y), (*c, 255))

    # 2. 【暗金古铜包边 (64,0)~(128,48)】
    for x in range(64, 128):
        for y in range(48):
            u = x - 64
            edge_dist = min(u % 8, 7 - (u % 8))
            if edge_dist == 0:
                c = PALETTE_GOLD[5]  # 边缘亮金 #E2C15C
            elif edge_dist == 1:
                c = PALETTE_GOLD[4]  # 华彩金 #C49A32
            elif edge_dist == 2:
                c = PALETTE_GOLD[3]  # 古金 #9A711D
            else:
                c = PALETTE_GOLD[1]  # 暗铜 #553812

            if (u + y) % 16 == 0:
                c = PALETTE_GOLD[0]  # 刻痕深阴影

            img.putpixel((x, y), (*c, 255))

    # 3. 【白玉菌柄 (0,48)~(64,80)】
    for x in range(64):
        for y in range(48, 80):
            v = y - 48
            stripe = (x * 3) % 17
            if stripe < 3:
                c = PALETTE_JADE[2]
            elif stripe == 4:
                c = PALETTE_JADE[4]
            else:
                c = PALETTE_JADE[3]

            if v > 28:
                c = PALETTE_JADE[1]  # 底部 AO

            img.putpixel((x, y), (*c, 255))

    # 4. 【伞底内嵌暗金菌孔层 (64,48)~(96,80)】
    for x in range(64, 96):
        for y in range(48, 80):
            u = x - 64
            v = y - 48
            if (u + v) % 3 == 0:
                c = PALETTE_GOLD[4]
            elif (u % 2 == 0):
                c = PALETTE_GOLD[2]
            else:
                c = PALETTE_GOLD[0]
            img.putpixel((x, y), (*c, 255))

    # 5. 【浮空阵法符印节点 (96,48)~(128,80)】
    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            dx = abs(u - 16)
            dy = abs(v - 16)
            if dx >= 13 or dy >= 13:
                c = PALETTE_GOLD[1]
            elif dx >= 11 or dy >= 11:
                c = PALETTE_GOLD[5]
            elif dx <= 4 and dy <= 4:
                c = PALETTE_QI[4]
            elif dx <= 8 and dy <= 8:
                c = PALETTE_QI[2]
            else:
                c = PALETTE_GOLD[0]
            img.putpixel((x, y), (*c, 255))

    # 6. 【玄铁玄武岩阵纹基座 (0,80)~(64,128)】
    for x in range(64):
        for y in range(80, 128):
            u = x
            v = y - 80
            noise = ((u * 13 + v * 29) % 7)
            base_idx = 0 if noise < 4 else 1
            c = PALETTE_BASALT[base_idx]

            is_vein = False
            is_node = False

            if (v == 12 and 8 <= u <= 36) or (u == 36 and 12 <= v <= 32) or (v == 32 and 36 <= u <= 56):
                is_vein = True
            elif (u == 18 and 24 <= v <= 44) or (v == 44 and 18 <= u <= 28):
                is_vein = True
            elif (v == 22 and 4 <= u <= 14) or (u == 48 and 4 <= v <= 18):
                is_vein = True

            if (u == 36 and v == 12) or (u == 36 and v == 32) or (u == 18 and v == 44):
                is_node = True

            if is_node:
                c = PALETTE_QI[4]
            elif is_vein:
                c = PALETTE_QI[1]

            img.putpixel((x, y), (*c, 255))

    # 7. 【灵眼泉眼核心与硬朗灵晶 (64,80)~(128,128)】
    for x in range(64, 128):
        for y in range(80, 128):
            u = x - 64
            v = y - 80
            dx = abs(u - 32)
            dy = abs(v - 24)
            d = math.hypot(dx, dy)

            if d < 5.0:
                c = PALETTE_QI[5]  # 炽白核心 #D0FFF5
            elif d < 12.0:
                c = PALETTE_QI[4]  # 炽光青 #83FBE4
            elif d < 20.0:
                c = PALETTE_QI[3]  # 灵气青 #48E8CC
            elif d < 28.0:
                c = PALETTE_QI[2]  # 亮青 #20CDB0
            elif d < 38.0:
                if (u * 2 + v * 3) % 11 < 3:
                    c = PALETTE_QI[3]
                else:
                    c = PALETTE_QI[1]
            else:
                c = PALETTE_QI[0]

            img.putpixel((x, y), (*c, 255))

    return img


def create_emissive_texture() -> Image.Image:
    """生成专属 Emissive Texture (黑白蒙版发光贴图)。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 255))

    # 1. 灵眼核心与灵晶发光区 (64,80)~(128,128)
    for x in range(64, 128):
        for y in range(80, 128):
            u = x - 64
            v = y - 80
            d = math.hypot(u - 32, v - 24)
            if d < 6.0:
                val = 255
            elif d < 14.0:
                val = int(255 * (1.0 - (d - 6.0) / 8.0))
            elif d < 24.0 and ((u * 2 + v * 3) % 11 < 2):
                val = 140
            else:
                val = 0
            img.putpixel((x, y), (val, val, val, 255))

    # 2. 阵法灵脉节点发光区 (0,80)~(64,128)
    for x in range(64):
        for y in range(80, 128):
            u = x
            v = y - 80
            val = 0
            if (u == 36 and v == 12) or (u == 36 and v == 32) or (u == 18 and v == 44):
                val = 220
            elif (v == 12 and 8 <= u <= 36) or (u == 36 and 12 <= v <= 32):
                val = 80
            img.putpixel((x, y), (val, val, val, 255))

    # 3. 浮空符印核心发光区 (96,48)~(128,80)
    for x in range(96, 128):
        for y in range(48, 80):
            u = x - 96
            v = y - 48
            dx = abs(u - 16)
            dy = abs(v - 16)
            val = 160 if dx <= 4 and dy <= 4 else 0
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
        "ancient_gold_rim": [64, 0, 128, 48],
        "jade_stipe": [0, 48, 64, 80],
        "golden_gills": [64, 48, 96, 80],
        "talisman_node": [96, 48, 128, 80],
        "dark_rock": [0, 80, 64, 128],
        "cyan_crystal": [64, 80, 128, 128],
        "qi_core_white": [88, 96, 104, 112],
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
    print(f"✅ [顶级如意玉芝定稿] 生成 BBModel: {bb_path} (分辨率: {TEXTURE_RES}x{TEXTURE_RES}, 共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "ling_yan_shi_zhi_texture_remaster.png"
    create_texture().save(tex_path)
    print(f"✅ [顶级如意玉芝定稿] 导出高清主贴图: {tex_path}")

    emissive_path = PREVIEW_DIR / "ling_yan_shi_zhi_emissive.png"
    create_emissive_texture().save(emissive_path)
    print(f"✅ [顶级如意玉芝定稿] 导出发光 Emissive 贴图: {emissive_path}")


if __name__ == "__main__":
    main()
