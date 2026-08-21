#!/usr/bin/env python3
"""生成续元蕊 (XuYuanRui) Blockbench .bbmodel 与三视图预览 (Round 3 终极打磨版)。

【世界观与设定背景】：
- 丹道稀有灵草，生长在灵眼与高灵气浓度灵脉旁（spirit_qi > 0.8）。
- 物理机制：灵气极高压强下凝华结晶（survival_mode: spirit_crystallize），
  是炼制高阶保命丹药「续元丹」与「化形大丹」的核心药材。
- 视觉特征：
  1. 裂隙黑曜石花萼基座 (Obsidian Mineral Base)：
     底部多层错落、向外自然剥裂开壳的深黑玄岩/黑曜石矿簇岩片（8 块不同倾角和长短的天然矿晶岩），
     表面具有多种随机分形金脉裂纹、晶面反光与暗紫玄晶质感，内凹聚灵金槽。
  2. 金华蕊柱簇 (Golden Stamen Cluster)：
     中央拔地而起的高低错落金黄色发光花蕊群（8 根带真实 3D 平滑弧度弯曲旋转的立柱），
     底部聚拢于玄岩裂缝，中上部自然向外发散如灵华绽放，顶端膨大为水滴菱形「灵气固化晶滴 (Spirit Crystal Drops)」。
     中心最长为主蕊柱（带白金炽光高亮核心）。
  3. 悬浮灵砾 (Ambient Floating Shards)：
     周围斜向悬浮 8 颗微小黑曜石碎粒与金晶微粒，呈螺旋逸散微动分布。

【技术规范】：
- 尺寸边界：严格控制在 1x1 方块内 (X: 2.0~14.0, Z: 2.0~14.0, Y: 0.0~15.5)。
- 3D 倾斜与真实旋转：使用 origin + rotation 呈现弯曲与向外张开的花萼，严禁 AABB 阶梯方块。
- 64x64 UV Atlas 高清贴图分区：
  * obsidian_dark: 玄石暗黑面 (0x1C1C22 + 暗紫玄晶微粒与棱面折射)
  * obsidian_vein_a / b: 树状金脉裂纹面 A/B (0xFFB800 裂纹 + 0x24222E 暗岩，双向纹理防止重复感)
  * stamen_grad: 发光花蕊纵向金黄渐变 (0xBD7E00 -> 0xFFD700 -> 0xFFF9C0)，柔和光纤丝缕
  * crystal_core: 水滴晶滴顶端白金核心 (0xFFFFFF / 0xFFECC0 / 0xFFD700 柔和光晕)
  * crystal_facet: 晶滴侧面折射面 (0xFFE880 / 0xF5C400 / 0xE6A800 棱晶渐变)
  * floating_gold: 悬浮金晶砾 (0xFFECC0 白金星芒)
  * floating_obs: 悬浮曜岩碎粒 (0x26222E + 金砂斑)
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
    uv_preset: str = "stamen_grad",
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
    uv_preset: str = "stamen_grad",
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
    """部件1：裂隙黑曜石花萼基座 (Obsidian Mineral Base)。

    层叠错落、向外爆裂剥开的 8 块玄岩/黑曜石矿簇岩片，缝隙透出分形树状金脉。
    """
    cubes: list[Cube] = []

    # 1. 底部地基岩台 (层级阶梯碎岩基底 Y: 0.0 ~ 1.5)
    cubes.append(
        Cube(
            "base",
            "base_slab_bottom",
            (3.8, 0.0, 3.8),
            (12.2, 0.8, 12.2),
            "obsidian_dark",
        )
    )
    cubes.append(
        Cube(
            "base",
            "base_slab_step_e",
            (11.2, 0.0, 5.0),
            (13.4, 0.6, 11.0),
            "obsidian_dark",
            rotation=(0.0, -8.0, -4.0),
            rot_origin=(12.0, 0.0, 8.0),
        )
    )
    cubes.append(
        Cube(
            "base",
            "base_slab_step_w",
            (2.6, 0.0, 5.2),
            (4.8, 0.6, 11.2),
            "obsidian_dark",
            rotation=(0.0, 6.0, 4.0),
            rot_origin=(4.0, 0.0, 8.0),
        )
    )

    # 2. 8 块层叠爆裂的黑曜石花萼岩片 (8 Obsidian Petal Shards)
    # Petal 1: 正北主岩柱 (North High Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_n",
            (6.0, 0.6, 2.6),
            (10.0, 4.6, 4.8),
            "obsidian_vein_a",
            rotation=(-22.0, 2.0, 0.0),
            rot_origin=(8.0, 0.6, 3.7),
        )
    )
    # Petal 2: 东北高耸尖石 (North-East Tall Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_ne",
            (10.0, 0.6, 3.6),
            (13.2, 5.2, 7.2),
            "obsidian_vein_b",
            rotation=(-16.0, 38.0, -22.0),
            rot_origin=(11.5, 0.6, 5.4),
        )
    )
    # Petal 3: 正东低位横岩 (East Low Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_e",
            (10.8, 0.5, 6.8),
            (13.6, 3.8, 10.2),
            "obsidian_vein_a",
            rotation=(0.0, 10.0, -24.0),
            rot_origin=(12.0, 0.5, 8.5),
        )
    )
    # Petal 4: 东南爆裂岩柱 (South-East Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_se",
            (9.2, 0.6, 9.8),
            (12.8, 4.8, 13.2),
            "obsidian_vein_b",
            rotation=(20.0, -32.0, -18.0),
            rot_origin=(11.0, 0.6, 11.5),
        )
    )
    # Petal 5: 正南中位岩柱 (South Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_s",
            (5.8, 0.6, 11.0),
            (10.2, 4.4, 13.4),
            "obsidian_vein_a",
            rotation=(24.0, 0.0, 0.0),
            rot_origin=(8.0, 0.6, 12.0),
        )
    )
    # Petal 6: 西南高耸尖石 (South-West Tall Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_sw",
            (2.8, 0.6, 9.2),
            (6.2, 5.4, 12.6),
            "obsidian_vein_b",
            rotation=(18.0, 42.0, 22.0),
            rot_origin=(4.5, 0.6, 10.9),
        )
    )
    # Petal 7: 正西低位横岩 (West Low Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_w",
            (2.4, 0.5, 5.8),
            (5.2, 3.6, 9.2),
            "obsidian_vein_a",
            rotation=(0.0, -12.0, 25.0),
            rot_origin=(3.8, 0.5, 7.5),
        )
    )
    # Petal 8: 西北高位尖石 (North-West Shard)
    cubes.append(
        Cube(
            "base",
            "calyx_petal_nw",
            (3.0, 0.6, 3.2),
            (6.4, 4.9, 6.8),
            "obsidian_vein_b",
            rotation=(-18.0, -45.0, 20.0),
            rot_origin=(4.7, 0.6, 5.0),
        )
    )

    # 3. 内部凹槽聚灵金脉涌泉 (Vein Core Puddle)
    cubes.append(
        Cube(
            "base",
            "vein_core_socket",
            (5.8, 0.6, 5.8),
            (10.2, 2.0, 10.2),
            "obsidian_vein_a",
        )
    )
    cubes.append(
        Cube(
            "base",
            "vein_core_gold_pool",
            (6.4, 1.4, 6.4),
            (9.6, 2.2, 9.6),
            "crystal_core",
        )
    )

    return cubes


def part_stamen_cluster() -> list[Cube]:
    """部件2：金华蕊柱簇 (Golden Stamen Cluster)。

    包含 8 根高低错落、微弯向外展开的花蕊柱与顶端水滴菱形灵晶滴。
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
            uv_preset="stamen_grad",
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
    # 5. 悬浮黑曜金斑碎粒 1 (西低位, Height ~6.8)
    cubes.append(
        Cube(
            "shards",
            "shard_obs_w_low",
            (3.2, 6.4, 6.2),
            (3.9, 7.2, 6.9),
            "floating_obs",
            rotation=(12.0, 18.0, -25.0),
            rot_origin=(3.55, 6.8, 6.55),
        )
    )
    # 6. 悬浮黑曜金斑碎粒 2 (东低位, Height ~6.2)
    cubes.append(
        Cube(
            "shards",
            "shard_obs_e_low",
            (12.2, 5.8, 8.6),
            (12.9, 6.6, 9.3),
            "floating_obs",
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
    """生成 64x64 UV Atlas 高清贴图 (Round 3 终极精绘版)。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 玄石暗黑面 obsidian_dark (0,0)~(32,32)
    # 色调：0x1C1C22 基础，带有玄武岩暗紫灰与细微晶簇噪点
    for x in range(32):
        for y in range(32):
            noise = ((x * 17 + y * 31) % 13) - 6
            r = max(0, min(255, 28 + noise))
            g = max(0, min(255, 28 + noise))
            b = max(0, min(255, 34 + noise + (noise // 2)))
            if (x * 7 + y * 13) % 23 == 0:
                r, g, b = r + 16, g + 14, b + 22
            img.putpixel((x, y), (r, g, b, 255))

    # 2. 树状金脉裂纹面 A obsidian_vein_a (0,32)~(32,48)
    # 树状分形向上延伸金脉
    for x in range(32):
        for y in range(32, 48):
            u = x
            v = y - 32
            main_c1 = 10 + int(math.sin(v * 0.45) * 6 + math.cos(v * 0.9) * 2)
            main_c2 = 24 - int(v * 0.5 + math.sin(u * 0.4) * 2)
            branch1 = 999
            if 4 <= v <= 14:
                branch1 = 10 + int(math.sin(6 * 0.45) * 6) + int((v - 4) * 1.1)

            d1 = abs(u - main_c1)
            d2 = abs(u - main_c2)
            d3 = abs(u - branch1)
            min_dist = min(d1, d2, d3)
            is_speck = ((u * 13 + v * 19) % 29 == 0) and (min_dist <= 3)

            if min_dist == 0 or is_speck:
                r, g, b = 255, 235, 90
            elif min_dist == 1:
                r, g, b = 245, 175, 20
            elif min_dist == 2:
                r, g, b = 115, 75, 25
            else:
                noise = ((x * 19 + y * 23) % 11) - 5
                r = max(0, min(255, 30 + noise))
                g = max(0, min(255, 28 + noise))
                b = max(0, min(255, 36 + noise))

            img.putpixel((x, y), (r, g, b, 255))

    # 3. 树状金脉裂纹面 B obsidian_vein_b (0,48)~(32,64)
    # 斜交与多网状金脉
    for x in range(32):
        for y in range(48, 64):
            u = x
            v = y - 48
            diag_c1 = 16 + int(math.cos(v * 0.5) * 7)
            diag_c2 = int(v * 1.2) - 2
            d1 = abs(u - diag_c1)
            d2 = abs(u - diag_c2)
            min_dist = min(d1, d2)
            is_speck = ((u * 17 + v * 11) % 31 == 0) and (min_dist <= 3)

            if min_dist == 0 or is_speck:
                r, g, b = 255, 238, 95
            elif min_dist == 1:
                r, g, b = 242, 170, 18
            elif min_dist == 2:
                r, g, b = 110, 70, 22
            else:
                noise = ((x * 23 + y * 17) % 11) - 5
                r = max(0, min(255, 29 + noise))
                g = max(0, min(255, 27 + noise))
                b = max(0, min(255, 35 + noise))

            img.putpixel((x, y), (r, g, b, 255))

    # 4. 发光花蕊金黄渐变 stamen_grad (32,0)~(64,32)
    # 色调：纵向金黄渐变 (0xBD7E00 -> 0xFFD700 -> 0xFFF9C0)
    for x in range(32, 64):
        for y in range(32):
            u = x - 32
            v = y
            grad = (31 - v) / 31.0
            fiber = math.sin(u * 1.57) * 7

            r = int(190 * (1.0 - grad) + 255 * grad + fiber)
            g = int(125 * (1.0 - grad) + 248 * grad + fiber * 0.8)
            b = int(0 * (1.0 - grad) + 185 * grad + fiber * 0.4)

            r = max(0, min(255, r))
            g = max(0, min(255, g))
            b = max(0, min(255, b))
            img.putpixel((x, y), (r, g, b, 255))

    # 5. 白金晶滴核心 crystal_core (32,32)~(48,48)
    # 色调：水滴晶滴顶端白金核心 (0xFFFFFF -> 0xFFECC0 -> 0xFFD700)
    for x in range(32, 48):
        for y in range(32, 48):
            dx = abs(x - 40)
            dy = abs(y - 40)
            d = math.sqrt(dx * dx + dy * dy)
            if d < 2.2:
                img.putpixel((x, y), (255, 255, 255, 255))
            elif d < 5.0:
                glow = (5.0 - d) / 2.8
                r = int(255)
                g = int(248 * glow + 225 * (1 - glow))
                b = int(235 * glow + 120 * (1 - glow))
                img.putpixel((x, y), (r, g, b, 255))
            else:
                img.putpixel((x, y), (255, 215, 65, 255))

    # 6. 晶滴侧面折射面 crystal_facet (48,32)~(64,48)
    # 色调：柔和连续的棱晶金黄 (0xFFE880 -> 0xF5C400 -> 0xE6A800)
    for x in range(48, 64):
        for y in range(32, 48):
            u = x - 48
            v = y - 32
            # 柔和的径向 + 对角微渐变
            rad = math.sqrt((u - 8) ** 2 + (v - 8) ** 2) / 11.3
            diag = math.sin((u + v) * 0.5) * 0.15
            shade = max(0.0, min(1.0, rad + diag))

            r = int(255 * (1.0 - shade) + 235 * shade)
            g = int(235 * (1.0 - shade) + 175 * shade)
            b = int(130 * (1.0 - shade) + 15 * shade)

            img.putpixel((x, y), (r, g, b, 255))

    # 7. 悬浮金晶微粒 floating_gold (32,48)~(48,64)
    # 色调：轻灵纯净的聚灵白金星华 (0xFFECC0)
    for x in range(32, 48):
        for y in range(48, 64):
            u = x - 32
            v = y - 48
            center_dist = math.sqrt((u - 8) ** 2 + (v - 8) ** 2)
            if center_dist < 3.0:
                img.putpixel((x, y), (255, 255, 220, 255))
            elif center_dist < 6.0:
                img.putpixel((x, y), (255, 230, 100, 255))
            else:
                img.putpixel((x, y), (235, 180, 45, 255))

    # 8. 悬浮曜岩碎粒 floating_obs (48,48)~(64,64)
    # 色调：暗黑玄岩带聚灵金砂斑 (0x26222E + 0xFFC830)
    for x in range(48, 64):
        for y in range(48, 64):
            u = x - 48
            v = y - 48
            if (u * 5 + v * 7) % 11 == 0:
                img.putpixel((x, y), (255, 220, 70, 255))
            else:
                noise = ((u * 13 + v * 17) % 9) - 4
                img.putpixel((x, y), (38 + noise, 34 + noise, 46 + noise, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "obsidian_dark": [0, 0, 32, 32],
        "obsidian_vein_a": [0, 32, 32, 48],
        "obsidian_vein_b": [0, 48, 32, 64],
        "stamen_grad": [32, 0, 64, 32],
        "crystal_core": [32, 32, 48, 48],
        "crystal_facet": [48, 32, 64, 48],
        "floating_gold": [32, 48, 48, 64],
        "floating_obs": [48, 48, 64, 64],
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
