#!/usr/bin/env python3
"""生成续元蕊 (XuYuanRui) Blockbench .bbmodel 与三视图预览 (天然层叠玄曜岩簇与灵髓重构版)。

【世界观与设定背景】：
- 丹道稀有灵草，生长在灵眼与高灵气浓度灵脉旁（spirit_qi > 0.8）。
- 物理机制：灵气极高压强下凝华结晶（survival_mode: spirit_crystallize），
  是炼制高阶保命丹药「续元丹」与「化形大丹」的核心药材。
- 视觉特征与玄曜石基座彻底重构：
  1. 天然层叠玄曜岩簇基座 (Tiered Fractured Obsidian Base)：
     - 彻底破除“方块桶/围栏感/整齐白边”：重构为天然地质层叠裂开的黑曜石矿床。
     - 结构由【底层层叠平铺碎岩阶】+【8 块大小高低错落、参差咬合的向外爆裂楔形碎岩】+【深凹聚灵石缝】组成。
     - 材质光学：真正火山黑曜岩与贝壳状断口质感 (0x141620 ~ 0x363C4E)，消除工业化边缘白圈，代之以天然晶面漫射与石缝内生细微金砂。
  2. 金华蕊柱簇 (Golden Stamen Cluster)：
     - 8 根高低错落、平滑微弯向外展开的花蕊立柱 (高度 7.5~14.8 格)，顶端膨大为复合水滴菱形固化灵气晶滴 (Spirit Crystal Drops)。
     - 主蕊柱位于中心，带有白金炽光高亮核心。
  3. 悬浮灵砾 (Ambient Floating Shards)：
     - 周围悬浮微小清透金髓碎粒与玄武岩微晶，与主体浑然一体。

【技术规范】：
- 尺寸边界：严格控制在 1x1 方块内 (X: 2.0~14.0, Z: 2.0~14.0, Y: 0.0~15.5)。
- 3D 倾斜与真实旋转：使用 origin + rotation 呈现弯曲与向外张开的花萼，严禁 AABB 阶梯方块。
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
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = REPO / "local_models"
PREVIEW_DIR = REPO / "scripts" / "models"
TEXTURE_RES = 64
UUID_NAMESPACE = uuid.UUID("a74f28c1-8419-482a-9e32-5d8819280004")


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


def make_directed_segment(
    bone: str,
    name: str,
    p1: tuple[float, float, float],
    p2: tuple[float, float, float],
    thickness: float = 0.55,
    uv_preset: str = "stamen_stem",
) -> Cube:
    """在空间两点 p1, p2 之间创建一节真实倾斜旋转的体素柱。"""
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


def build_stamen_chain(
    bone: str,
    prefix: str,
    path_points: list[tuple[float, float, float]],
    thickness_start: float = 0.7,
    thickness_end: float = 0.45,
    uv_preset: str = "stamen_stem",
) -> list[Cube]:
    """生成一条连续平滑、自然微弯的花蕊柱。"""
    cubes: list[Cube] = []
    n = len(path_points)
    for i in range(n - 1):
        p1 = path_points[i]
        p2 = path_points[i + 1]
        progress = i / max(1, n - 2)
        t = thickness_start + (thickness_end - thickness_start) * progress
        cubes.append(
            make_directed_segment(bone, f"{prefix}_{i:02d}", p1, p2, t, uv_preset)
        )
    return cubes


def make_crystal_droplet(
    bone: str,
    prefix: str,
    center: tuple[float, float, float],
    size: tuple[float, float, float] = (1.2, 1.4, 1.2),
    tilt: tuple[float, float, float] = (0.0, 0.0, 0.0),
    is_main: bool = False,
) -> list[Cube]:
    """生成具有水滴/菱形结晶感的多面灵气晶滴。"""
    cx, cy, cz = center
    sx, sy, sz = size
    cubes = []

    # 1. 晶滴主腹 (Main Drop Body)
    cubes.append(
        Cube(
            bone,
            f"{prefix}_body",
            (cx - sx / 2, cy - sy * 0.35, cz - sz / 2),
            (cx + sx / 2, cy + sy * 0.35, cz + sz / 2),
            "crystal_core" if is_main else "crystal_facet",
            rotation=tilt,
            rot_origin=center,
        )
    )

    # 2. 晶滴 45 度交错结晶棱面 (Crystal Facet Shell)
    fsx, fsy, fsz = sx * 0.85, sy * 0.75, sz * 0.85
    cubes.append(
        Cube(
            bone,
            f"{prefix}_facet",
            (cx - fsx / 2, cy - fsy * 0.38, cz - fsz / 2),
            (cx + fsx / 2, cy + fsy * 0.38, cz + fsz / 2),
            "crystal_facet",
            rotation=(tilt[0], tilt[1] + 45.0, tilt[2]),
            rot_origin=center,
        )
    )

    # 3. 顶端聚灵尖锥 (Top Crystal Tip)
    tx, ty, tz = sx * 0.5, sy * 0.35, sz * 0.5
    cubes.append(
        Cube(
            bone,
            f"{prefix}_tip",
            (cx - tx / 2, cy + sy * 0.3, cz - tz / 2),
            (cx + tx / 2, cy + sy * 0.65, cz + tz / 2),
            "crystal_core",
            rotation=tilt,
            rot_origin=center,
        )
    )

    return cubes


def part_obsidian_base() -> list[Cube]:
    """部件1：天然层叠玄曜岩簇基座 (Tiered Fractured Obsidian Base)。

    有机地貌设计：
    - 底层错落碎岩基盘 (3 块层叠咬合的扁平岩板 Y: 0.0 ~ 1.0)
    - 中外层 8 块高低起伏、向外开裂的天然斜切黑曜石岩瓣 (Y: 0.4 ~ 4.2)
    - 4 块低位伴生碎矿角石
    - 深凹的中心聚灵金石底槽
    """
    cubes: list[Cube] = []

    # 1. 底层层叠地质基盘 (Tiered Base Slabs)
    # 主基底
    cubes.append(
        Cube(
            "base",
            "base_slab_core",
            (4.2, 0.0, 4.2),
            (11.8, 0.6, 11.8),
            "rock_dark",
        )
    )
    # 东向延伸碎阶
    cubes.append(
        Cube(
            "base",
            "base_slab_east",
            (10.8, 0.0, 4.6),
            (13.2, 0.5, 11.2),
            "rock_dark",
            rotation=(0.0, -5.0, -3.0),
            rot_origin=(11.5, 0.0, 8.0),
        )
    )
    # 西北延伸碎阶
    cubes.append(
        Cube(
            "base",
            "base_slab_nw",
            (2.8, 0.0, 3.4),
            (5.4, 0.5, 10.4),
            "rock_dark",
            rotation=(0.0, 8.0, 3.0),
            rot_origin=(4.0, 0.0, 7.0),
        )
    )

    # 2. 8 块向外爆裂、高低参差的天然斜切黑曜岩块 (8 Fractured Obsidian Rocks)
    # (1) 正北主岩峰 (North Ridge: 宽 4.2, 厚 1.8, 高 4.0, 倾角 -24°)
    cubes.append(
        Cube(
            "base",
            "rock_n_main",
            (5.9, 0.4, 3.4),
            (10.1, 4.2, 5.2),
            "rock_vein_a",
            rotation=(-24.0, 0.0, 0.0),
            rot_origin=(8.0, 0.4, 4.3),
        )
    )
    # 北岩阶梯侧削面
    cubes.append(
        Cube(
            "base",
            "rock_n_sub",
            (6.6, 0.4, 2.6),
            (9.4, 2.8, 3.8),
            "rock_facet",
            rotation=(-30.0, 0.0, 0.0),
            rot_origin=(8.0, 0.4, 3.2),
        )
    )

    # (2) 东北高耸劈裂岩 (NE Tall Rock: 宽 2.4, 厚 2.2, 高 4.8, 复合倾斜)
    cubes.append(
        Cube(
            "base",
            "rock_ne_tall",
            (10.0, 0.4, 3.8),
            (12.4, 4.8, 6.0),
            "rock_vein_b",
            rotation=(-16.0, 35.0, -22.0),
            rot_origin=(11.2, 0.4, 4.9),
        )
    )

    # (3) 正东低平厚岩 (East Flat Rock: 宽 2.0, 厚 3.8, 高 2.8, 倾角 -18°)
    cubes.append(
        Cube(
            "base",
            "rock_e_flat",
            (10.6, 0.4, 6.2),
            (12.6, 3.2, 10.0),
            "rock_facet",
            rotation=(0.0, 5.0, -18.0),
            rot_origin=(11.6, 0.4, 8.1),
        )
    )

    # (4) 东南爆裂尖岩 (SE Angular Rock: 宽 2.6, 厚 2.4, 高 4.4, 复合倾斜)
    cubes.append(
        Cube(
            "base",
            "rock_se_shard",
            (9.6, 0.4, 9.8),
            (12.2, 4.4, 12.2),
            "rock_vein_a",
            rotation=(20.0, -32.0, -18.0),
            rot_origin=(10.9, 0.4, 11.0),
        )
    )

    # (5) 正南开裂厚岩 (South Ridge: 宽 4.0, 厚 1.8, 高 3.8, 倾角 25°)
    cubes.append(
        Cube(
            "base",
            "rock_s_main",
            (6.0, 0.4, 10.8),
            (10.0, 3.8, 12.6),
            "rock_vein_b",
            rotation=(25.0, 0.0, 0.0),
            rot_origin=(8.0, 0.4, 11.7),
        )
    )

    # (6) 西南高耸错落岩 (SW Tall Rock: 宽 2.4, 厚 2.2, 高 4.6, 复合倾斜)
    cubes.append(
        Cube(
            "base",
            "rock_sw_tall",
            (3.6, 0.4, 9.8),
            (6.0, 4.6, 12.0),
            "rock_vein_a",
            rotation=(18.0, 40.0, 22.0),
            rot_origin=(4.8, 0.4, 10.9),
        )
    )

    # (7) 正西平削岩 (West Flat Rock: 宽 2.0, 厚 3.8, 高 2.6, 倾角 18°)
    cubes.append(
        Cube(
            "base",
            "rock_w_flat",
            (3.4, 0.4, 6.0),
            (5.4, 3.0, 9.8),
            "rock_facet",
            rotation=(0.0, -6.0, 18.0),
            rot_origin=(4.4, 0.4, 7.9),
        )
    )

    # (8) 西北爆裂岩峰 (NW Angular Rock: 宽 2.6, 厚 2.4, 高 4.2, 复合倾斜)
    cubes.append(
        Cube(
            "base",
            "rock_nw_shard",
            (3.8, 0.4, 3.8),
            (6.4, 4.2, 6.2),
            "rock_vein_b",
            rotation=(-18.0, -42.0, 20.0),
            rot_origin=(5.1, 0.4, 5.0),
        )
    )

    # 3. 4 块低位天然伴生碎岩 (Flanking Debris)
    cubes.append(
        Cube(
            "base",
            "debris_ne",
            (11.4, 0.2, 6.8),
            (12.8, 1.6, 8.2),
            "rock_dark",
            rotation=(10.0, 20.0, -15.0),
            rot_origin=(12.1, 0.2, 7.5),
        )
    )
    cubes.append(
        Cube(
            "base",
            "debris_sw",
            (3.2, 0.2, 7.8),
            (4.6, 1.6, 9.2),
            "rock_dark",
            rotation=(-10.0, 15.0, 15.0),
            rot_origin=(3.9, 0.2, 8.5),
        )
    )

    # 4. 中心内凹聚灵金晶涌泉池 (Vein Core Puddle Y: 0.6 ~ 2.0)
    cubes.append(
        Cube(
            "base",
            "vein_core_socket",
            (6.0, 0.5, 6.0),
            (10.0, 1.8, 10.0),
            "rock_dark",
        )
    )
    cubes.append(
        Cube(
            "base",
            "golden_crystal_mound",
            (6.6, 1.1, 6.6),
            (9.4, 2.3, 9.4),
            "crystal_core",
            rotation=(0.0, 45.0, 0.0),
            rot_origin=(8.0, 1.7, 8.0),
        )
    )

    return cubes


def part_stamen_cluster() -> list[Cube]:
    """部件2：金华蕊柱簇 (Golden Stamen Cluster)。

    包含 8 根高低错落、平滑微弯向外展开的花蕊柱与顶端水滴菱形灵晶滴。
    1. 主蕊柱 (Main Core): 高度 14.8
    2. 东北长蕊 (NE Long): 高度 12.8
    3. 东南长蕊 (SE Long): 高度 12.4
    4. 西南长蕊 (SW Long): 高度 13.0
    5. 西北长蕊 (NW Long): 高度 12.2
    6. 正东中蕊 (East Mid): 高度 10.5
    7. 正西中蕊 (West Mid): 高度 10.2
    8. 正南短蕊 (South Short): 高度 8.2
    """
    cubes: list[Cube] = []

    # ─────────────────────────────────────────────────────────────
    # 1. 主蕊柱 (Core Main Stamen, Height 14.8)
    main_path = [
        (8.0, 1.8, 8.0),
        (8.05, 4.8, 7.95),
        (7.9, 8.0, 7.85),
        (8.05, 11.2, 7.8),
        (8.0, 13.6, 7.8),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_main",
            "stamen_core",
            main_path,
            thickness_start=0.85,
            thickness_end=0.6,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_main",
            "main_crystal_core",
            center=(8.0, 14.2, 7.8),
            size=(1.6, 2.0, 1.6),
            tilt=(0.0, 25.0, 0.0),
            is_main=True,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 2. 东北长蕊 (NE Long Stamen, Height 12.8)
    ne_path = [
        (8.3, 1.8, 7.6),
        (8.8, 4.8, 7.0),
        (9.4, 8.0, 6.4),
        (9.8, 11.4, 5.8),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_ne",
            ne_path,
            thickness_start=0.68,
            thickness_end=0.48,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_ne",
            center=(9.9, 12.0, 5.7),
            size=(1.2, 1.5, 1.2),
            tilt=(-14.0, 30.0, -8.0),
            is_main=False,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 3. 东南长蕊 (SE Long Stamen, Height 12.4)
    se_path = [
        (8.4, 1.8, 8.3),
        (9.1, 4.6, 8.8),
        (9.9, 7.8, 9.4),
        (10.4, 11.0, 9.9),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_se",
            se_path,
            thickness_start=0.68,
            thickness_end=0.48,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_se",
            center=(10.5, 11.6, 10.0),
            size=(1.2, 1.5, 1.2),
            tilt=(12.0, -25.0, -15.0),
            is_main=False,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 4. 西南长蕊 (SW Long Stamen, Height 13.0)
    sw_path = [
        (7.7, 1.8, 8.4),
        (7.1, 5.0, 9.1),
        (6.4, 8.4, 9.8),
        (5.8, 11.6, 10.3),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_sw",
            sw_path,
            thickness_start=0.68,
            thickness_end=0.48,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_sw",
            center=(5.7, 12.2, 10.4),
            size=(1.2, 1.5, 1.2),
            tilt=(15.0, 20.0, 12.0),
            is_main=False,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 5. 西北长蕊 (NW Long Stamen, Height 12.2)
    nw_path = [
        (7.6, 1.8, 7.6),
        (6.9, 4.6, 7.1),
        (6.1, 7.6, 6.5),
        (5.5, 10.8, 6.0),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_nw",
            nw_path,
            thickness_start=0.68,
            thickness_end=0.48,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_nw",
            center=(5.4, 11.4, 5.9),
            size=(1.2, 1.5, 1.2),
            tilt=(-12.0, -35.0, 14.0),
            is_main=False,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 6. 正东中蕊 (East Mid Stamen, Height 10.5)
    e_path = [
        (8.5, 1.8, 8.0),
        (9.4, 4.4, 8.0),
        (10.1, 7.2, 7.9),
        (10.5, 9.4, 7.8),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_e_mid",
            e_path,
            thickness_start=0.62,
            thickness_end=0.44,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_e_mid",
            center=(10.6, 10.0, 7.8),
            size=(1.1, 1.3, 1.1),
            tilt=(0.0, 0.0, -18.0),
            is_main=False,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 7. 正西中蕊 (West Mid Stamen, Height 10.2)
    w_path = [
        (7.5, 1.8, 8.0),
        (6.6, 4.4, 8.0),
        (5.9, 7.0, 8.1),
        (5.5, 9.2, 8.2),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_w_mid",
            w_path,
            thickness_start=0.62,
            thickness_end=0.44,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_w_mid",
            center=(5.4, 9.8, 8.2),
            size=(1.1, 1.3, 1.1),
            tilt=(0.0, 0.0, 18.0),
            is_main=False,
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 8. 正南短蕊 (South Short Stamen, Height 8.2)
    s_path = [
        (8.0, 1.8, 8.5),
        (8.0, 4.0, 9.4),
        (8.0, 6.4, 10.1),
        (8.0, 7.6, 10.5),
    ]
    cubes.extend(
        build_stamen_chain(
            "stamen_sub",
            "stamen_s_short",
            s_path,
            thickness_start=0.58,
            thickness_end=0.42,
            uv_preset="stamen_stem",
        )
    )
    cubes.extend(
        make_crystal_droplet(
            "stamen_sub",
            "droplet_s_short",
            center=(8.0, 8.1, 10.6),
            size=(1.0, 1.2, 1.0),
            tilt=(20.0, 0.0, 0.0),
            is_main=False,
        )
    )

    return cubes


def part_floating_shards() -> list[Cube]:
    """部件3：悬浮灵砾 (Ambient Floating Shards)。

    8 颗呈螺旋上升逸散态势的微小悬浮灵砾（白金晶滴、微光金晶与黑曜石晶核）。
    """
    cubes: list[Cube] = []

    # 1. 悬浮白金灵滴 1 (西北高位, Height ~11.8)
    cubes.append(
        Cube(
            "shards",
            "shard_gold_nw_high",
            (4.1, 11.4, 4.4),
            (4.8, 12.2, 5.1),
            "floating_gold",
            rotation=(22.0, 45.0, 15.0),
            rot_origin=(4.45, 11.8, 4.75),
        )
    )
    # 2. 悬浮金晶微粒 2 (东北高位, Height ~13.5)
    cubes.append(
        Cube(
            "shards",
            "shard_gold_ne_top",
            (10.6, 13.0, 4.6),
            (11.3, 13.8, 5.3),
            "floating_gold",
            rotation=(-18.0, 35.0, -12.0),
            rot_origin=(10.95, 13.4, 4.95),
        )
    )
    # 3. 悬浮金晶微粒 3 (西南高位, Height ~13.8)
    cubes.append(
        Cube(
            "shards",
            "shard_gold_sw_top",
            (4.6, 13.4, 11.2),
            (5.3, 14.2, 11.9),
            "floating_gold",
            rotation=(16.0, -25.0, 20.0),
            rot_origin=(4.95, 13.8, 11.55),
        )
    )
    # 4. 悬浮金晶微粒 4 (东南中位, Height ~9.5)
    cubes.append(
        Cube(
            "shards",
            "shard_gold_se_mid",
            (11.4, 9.0, 11.2),
            (12.1, 9.8, 11.9),
            "floating_gold",
            rotation=(25.0, -15.0, -18.0),
            rot_origin=(11.75, 9.4, 11.55),
        )
    )
    # 5. 悬浮黑曜晶刃碎粒 1 (西低位, Height ~6.8)
    cubes.append(
        Cube(
            "shards",
            "shard_obs_w_low",
            (3.2, 6.4, 6.2),
            (3.9, 7.2, 6.9),
            "rock_dark",
            rotation=(12.0, 18.0, -25.0),
            rot_origin=(3.55, 6.8, 6.55),
        )
    )
    # 6. 悬浮黑曜晶刃碎粒 2 (东低位, Height ~6.2)
    cubes.append(
        Cube(
            "shards",
            "shard_obs_e_low",
            (12.2, 5.8, 8.6),
            (12.9, 6.6, 9.3),
            "rock_dark",
            rotation=(-14.0, -20.0, 22.0),
            rot_origin=(12.55, 6.2, 8.95),
        )
    )
    # 7. 悬浮微光金星 1 (正北中位, Height ~8.8)
    cubes.append(
        Cube(
            "shards",
            "shard_star_n",
            (8.4, 8.4, 3.8),
            (9.0, 9.0, 4.4),
            "floating_gold",
            rotation=(-10.0, 45.0, 0.0),
            rot_origin=(8.7, 8.7, 4.1),
        )
    )
    # 8. 悬浮微光金星 2 (南偏西中位, Height ~10.4)
    cubes.append(
        Cube(
            "shards",
            "shard_star_sw",
            (7.2, 10.0, 11.6),
            (7.8, 10.6, 12.2),
            "floating_gold",
            rotation=(15.0, 45.0, 10.0),
            rot_origin=(7.5, 10.3, 11.9),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return part_obsidian_base() + part_stamen_cluster() + part_floating_shards()


def create_texture() -> Image.Image:
    """生成 64x64 UV Atlas 天然玄曜石与金华贴图。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # ─────────────────────────────────────────────────────────────
    # 1. 天然黑曜石内生金矿脉 A (rock_vein_a: 0,0~32,20)
    # 整体为冷暗墨蓝黑岩石 (0x181A24 ~ 0x2A2E3E)，深嵌分形细微金砂裂痕
    for x in range(32):
        for y in range(20):
            u = x
            v = y

            # 天然断口大面漫反射（斜向大明暗分面，非单一硬框）
            plane_shade = int((u * 0.7 + (19 - v) * 0.9) / 32.0 * 20)
            noise = ((u * 17 + v * 31) % 7) - 3
            base = 28 + plane_shade + noise

            r = max(14, min(65, base - 5))
            g = max(14, min(68, base - 3))
            b = max(18, min(85, base + 8))

            # 微细天然金矿裂痕 (断续、细窄、深陷)
            crack_pos = 12 + int(math.sin(v * 0.5) * 5 + math.cos(u * 0.4) * 2)
            d = abs(u - crack_pos)

            if d == 0 and 4 <= v <= 16:
                # 金矿核心
                r, g, b = 238, 195, 52
            elif d == 1 and 3 <= v <= 17:
                # 暗金浸润
                r, g, b = 135, 92, 28
            elif d == 2 and 2 <= v <= 18:
                # 矿槽深色阴影
                r, g, b = 18, 16, 24

            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 2. 天然黑曜石内生金矿脉 B (rock_vein_b: 0,20~32,40)
    for x in range(32):
        for y in range(20, 40):
            u = x
            v = y - 20

            plane_shade = int(((31 - u) * 0.8 + (19 - v) * 0.8) / 32.0 * 20)
            noise = ((u * 19 + v * 23) % 7) - 3
            base = 28 + plane_shade + noise

            r = max(14, min(65, base - 5))
            g = max(14, min(68, base - 3))
            b = max(18, min(85, base + 8))

            crack_pos = 20 - int(v * 0.7 + math.sin(u * 0.5) * 2)
            d = abs(u - crack_pos)

            if d == 0 and 3 <= v <= 15:
                r, g, b = 235, 190, 48
            elif d == 1 and 2 <= v <= 16:
                r, g, b = 130, 88, 25
            elif d == 2 and 2 <= v <= 17:
                r, g, b = 18, 16, 24

            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 3. 黑曜石贝壳状解理断口面 (rock_facet: 0,40~32,52)
    # 冷青灰与墨蓝断口大块明暗，呈现火山玻璃自然断面
    for x in range(32):
        for y in range(40, 52):
            u = x
            v = y - 40

            conchoidal = math.sin(u * 0.3 + v * 0.5) * 8
            facet_grad = (u + v) / 42.0
            noise = ((u * 13 + v * 17) % 7) - 3

            val = int(58 * (1 - facet_grad) + 24 * facet_grad + conchoidal + noise)
            r = max(14, min(75, val - 4))
            g = max(14, min(78, val - 2))
            b = max(18, min(95, val + 10))

            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 4. 天然黑曜玄岩暗底 (rock_dark: 0,52~32,64)
    for x in range(32):
        for y in range(52, 64):
            u = x
            v = y - 52
            noise = ((u * 17 + v * 29) % 9) - 4
            base = 24 + noise
            r = max(12, min(42, base - 4))
            g = max(12, min(44, base - 2))
            b = max(16, min(54, base + 6))
            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 5. 蕊柱纵向平滑渐变光纤 stamen_stem (32,0)~(64,32)
    # 色调：根部暗赭褐 (0x4E371C) -> 中段琥珀金 (0xB88C32) -> 上段温润玉黄 (0xEAD48A)
    for x in range(32, 64):
        for y in range(32):
            u = x - 32
            v = y
            grad = (31 - v) / 31.0  # 1.0(顶部) -> 0.0(底部)
            fiber = math.sin(u * 1.57) * 4

            if grad < 0.35:
                t = grad / 0.35
                r = int(68 * (1 - t) + 145 * t + fiber)
                g = int(48 * (1 - t) + 102 * t + fiber * 0.7)
                b = int(22 * (1 - t) + 38 * t)
            elif grad < 0.75:
                t = (grad - 0.35) / 0.40
                r = int(145 * (1 - t) + 215 * t + fiber)
                g = int(102 * (1 - t) + 172 * t + fiber * 0.8)
                b = int(38 * (1 - t) + 78 * t)
            else:
                t = (grad - 0.75) / 0.25
                r = int(215 * (1 - t) + 242 * t + fiber * 0.5)
                g = int(172 * (1 - t) + 222 * t + fiber * 0.5)
                b = int(78 * (1 - t) + 148 * t)

            r = max(0, min(255, r))
            g = max(0, min(255, g))
            b = max(0, min(255, b))
            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 6. 凝华温润白金晶核 crystal_core (32,32)~(48,48)
    for x in range(32, 48):
        for y in range(32, 48):
            dx = abs(x - 40)
            dy = abs(y - 40)
            d = math.sqrt(dx * dx + dy * dy)
            if d < 2.0:
                img.putpixel((x, y), (255, 252, 244, 255))
            elif d < 4.5:
                glow = (4.5 - d) / 2.5
                r = int(255 * glow + 245 * (1 - glow))
                g = int(250 * glow + 220 * (1 - glow))
                b = int(235 * glow + 155 * (1 - glow))
                img.putpixel((x, y), (r, g, b, 255))
            else:
                img.putpixel((x, y), (232, 198, 108, 255))

    # ─────────────────────────────────────────────────────────────
    # 7. 晶滴侧面矿晶折射面 crystal_facet (48,32)~(64,48)
    for x in range(48, 64):
        for y in range(32, 48):
            u = x - 48
            v = y - 32
            rad = math.sqrt((u - 8) ** 2 + (v - 8) ** 2) / 11.3
            diag = math.sin((u + v) * 0.6) * 0.12
            shade = max(0.0, min(1.0, rad + diag))

            r = int(238 * (1.0 - shade) + 172 * shade)
            g = int(210 * (1.0 - shade) + 130 * shade)
            b = int(128 * (1.0 - shade) + 48 * shade)

            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 8. 悬浮微晶金华 floating_gold (32,48)~(48,64)
    for x in range(32, 48):
        for y in range(48, 64):
            u = x - 32
            v = y - 48
            center_dist = math.sqrt((u - 8) ** 2 + (v - 8) ** 2)
            if center_dist < 2.5:
                img.putpixel((x, y), (252, 246, 230, 255))
            elif center_dist < 5.5:
                img.putpixel((x, y), (242, 220, 150, 255))
            else:
                img.putpixel((x, y), (205, 168, 88, 255))

    # ─────────────────────────────────────────────────────────────
    # 9. 悬浮曜岩碎粒 (48,48)~(64,64)
    for x in range(48, 64):
        for y in range(48, 64):
            u = x - 48
            v = y - 48
            noise = ((u * 13 + v * 17) % 7) - 3
            img.putpixel((x, y), (36 + noise, 38 + noise, 50 + noise, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "rock_vein_a": [0, 0, 32, 20],
        "rock_vein_b": [0, 20, 32, 40],
        "rock_facet": [0, 40, 32, 52],
        "rock_dark": [0, 52, 32, 64],
        "stamen_stem": [32, 0, 64, 32],
        "crystal_core": [32, 32, 48, 48],
        "crystal_facet": [48, 32, 64, 48],
        "floating_gold": [32, 48, 48, 64],
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
        "name": "XuYuanRui",
        "model_identifier": "xu_yuan_rui",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "xu_yuan_rui.png",
                "name": "xu_yuan_rui",
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
    out_bbmodel = LOCAL_MODELS / "XuYuanRui.bbmodel"
    out_bbmodel.write_text(json.dumps(bb_dict, indent=2))
    print(f"Generated BBModel: {out_bbmodel} (Elements: {len(bb_dict['elements'])})")

    # 保存 UV 贴图预览
    tex = create_texture()
    tex.save(PREVIEW_DIR / "xu_yuan_rui_texture.png")
    print(f"Saved texture atlas: {PREVIEW_DIR / 'xu_yuan_rui_texture.png'}")


if __name__ == "__main__":
    main()
