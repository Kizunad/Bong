#!/usr/bin/env python3
"""生成龙鳞苔 (LongLinTai) Blockbench .bbmodel 与三视图预览 (手绘像素级高质量材质与明暗贴图终极版)。

遵循专业像素贴图设计规范：
1. 材质一眼可辨：
   - 灰青角质龙鳞：硬质平滑大面 (65% 干净休息区) + 鳞片向光上部浅色 + 向光边缘亮青高光棱 + 四周深色手绘 AO 凹槽 + 极少微钙化点。
   - 苍翠生机苔藓：簇状手绘像素团块，顶面嫩黄绿 -> 侧面生机绿 -> 根部墨绿 AO，带初生芽孢。
   - 坍缩渊深冷玄岩：大块面朝向明暗 (顶亮侧暗) + 坚硬棱线高光 + 岩体节理与接触深阴影。
2. 严禁全图随机数学噪声：
   - 严格使用受模型结构、表面朝向与材质 Mask 约束的有限调色板 (每材质 4~6 色)。
   - 绝不使用纯黑 (#000000) 与纯白 (#FFFFFF)。
3. 3 层结构贴图规范：
   - Layer 1: 大尺度朝向明暗与手绘 AO 接触阴影。
   - Layer 2: 材质典型纹理（鳞片同心生长环、岩石解理面、苔藓绒毛团块）。
   - Layer 3: 克制的微小细节（稀疏钙化斑、棱角微磨损、初生孢子尖端）。
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
UUID_NAMESPACE = uuid.UUID("d894b321-4567-4e12-8921-9988aa110005")


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


# 统一斜坡主旋转参数 (倾角 32°, 绕斜对角线旋转 45°)
SLOPE_ROT = (-32.0, 45.0, 0.0)
SLOPE_ORG = (8.0, 2.2, 8.0)


def part_abyssal_base() -> list[Cube]:
    """部件1：坍缩渊侵蚀玄岩基床 (Collapsing Rift Rock Base)。"""
    cubes: list[Cube] = []

    # 1. 绝对世界坐标下的底层地台 (Y: 0.0 ~ 1.2)
    cubes.append(
        Cube(
            "rock_base",
            "base_slab_bottom",
            (3.4, 0.0, 3.4),
            (12.6, 1.2, 12.6),
            "abyssal_rock_dark",
        )
    )
    # 东侧延伸台阶
    cubes.append(
        Cube(
            "rock_base",
            "base_step_e",
            (11.0, 0.0, 4.8),
            (13.2, 1.6, 10.4),
            "abyssal_rock_dark",
            rotation=(0.0, -8.0, -8.0),
            rot_origin=(12.0, 0.0, 7.6),
        )
    )
    # 西侧延伸台阶
    cubes.append(
        Cube(
            "rock_base",
            "base_step_w",
            (2.8, 0.0, 5.6),
            (5.0, 1.6, 11.2),
            "abyssal_rock_dark",
            rotation=(0.0, 8.0, 8.0),
            rot_origin=(3.9, 0.0, 8.4),
        )
    )

    # 2. 斜坡岩床（在斜坡坐标系下，Y: -2.0 ~ 0.0，贴在鳞片底面下方）
    cubes.append(
        Cube(
            "rock_base",
            "base_slope_bed",
            (8.0 - 4.8, 2.2 - 2.0, 8.0 - 4.8),
            (8.0 + 4.8, 2.2 + 0.0, 8.0 + 4.8),
            "abyssal_rock",
            rotation=SLOPE_ROT,
            rot_origin=SLOPE_ORG,
        )
    )

    return cubes


def make_aligned_scale(
    bone: str,
    prefix: str,
    u_pos: float,  # 横向坐标 (左负右正)
    v_pos: float,  # 纵向坐标 (下/前负，上/后正)
    w: float = 2.1,
    d: float = 1.9,
    layer_idx: int = 0,
    uv_preset: str = "scale_plate_a",
    micro_yaw: float = 0.0,
) -> list[Cube]:
    """在斜坡局部平面上生成一块独立六边形龙鳞。"""
    lx = 8.0 + u_pos
    lz = 8.0 - v_pos
    ly = 2.2 + 0.06 + layer_idx * 0.18  # 覆瓦层阶高度

    # 包含微扰动旋转
    rot = (SLOPE_ROT[0], SLOPE_ROT[1] + micro_yaw, SLOPE_ROT[2])

    cubes = []
    # 1. 鳞片主躯体
    cubes.append(
        Cube(
            bone,
            f"{prefix}_plate",
            (lx - w / 2, ly, lz - d / 2),
            (lx + w / 2, ly + 0.32, lz + d / 2),
            uv_preset,
            rotation=rot,
            rot_origin=SLOPE_ORG,
        )
    )
    # 2. 鳞片前缘弧形高光加厚棱 (沿下边缘)
    fw, fd = w * 0.88, d * 0.40
    cubes.append(
        Cube(
            bone,
            f"{prefix}_edge",
            (lx - fw / 2, ly + 0.08, lz + d * 0.10),
            (lx + fw / 2, ly + 0.44, lz + d / 2 + 0.06),
            "scale_edge",
            rotation=rot,
            rot_origin=SLOPE_ORG,
        )
    )

    return cubes


def part_dragon_scales() -> list[Cube]:
    """部件2：平滑紧密覆瓦排列的灰青龙鳞甲片主簇 (Shingled Dragon Scale Plates)。

    严格对齐原画的 3-4-4-3 菱形交错网格：
    - Row 0 (前沿低位 3 片, layer 0): v = -2.9, u = -2.1, 0.0, 2.1
    - Row 1 (中低位 4 片, layer 1):   v = -0.95, u = -3.15, -1.05, 1.05, 3.15
    - Row 2 (中高位 4 片, layer 2):   v = +1.00, u = -3.15, -1.05, 1.05, 3.15
    - Row 3 (顶端位 3 片, layer 3):   v = +2.95, u = -2.1, 0.0, 2.1
    """
    cubes: list[Cube] = []

    # Row 0 (低位前沿 3 片)
    r0 = [
        ("s0_0", -2.1, -2.9, "scale_plate_a", -1.0),
        ("s0_1", 0.0, -2.9, "scale_plate_b", 0.5),
        ("s0_2", 2.1, -2.9, "scale_plate_a", 1.0),
    ]
    for name, u, v, uv, myaw in r0:
        cubes.extend(make_aligned_scale("scales", name, u, v, w=2.08, d=1.85, layer_idx=0, uv_preset=uv, micro_yaw=myaw))

    # Row 1 (中低位 4 片, 严格半格交错)
    r1 = [
        ("s1_0", -3.15, -0.95, "scale_plate_b", 1.5),
        ("s1_1", -1.05, -0.95, "scale_plate_a", -0.5),
        ("s1_2", 1.05, -0.95, "scale_plate_b", 0.5),
        ("s1_3", 3.15, -0.95, "scale_plate_a", -1.5),
    ]
    for name, u, v, uv, myaw in r1:
        cubes.extend(make_aligned_scale("scales", name, u, v, w=2.08, d=1.85, layer_idx=1, uv_preset=uv, micro_yaw=myaw))

    # Row 2 (中高位 4 片)
    r2 = [
        ("s2_0", -3.15, 1.00, "scale_plate_a", -1.0),
        ("s2_1", -1.05, 1.00, "scale_plate_b", 0.8),
        ("s2_2", 1.05, 1.00, "scale_plate_a", -0.8),
        ("s2_3", 3.15, 1.00, "scale_plate_b", 1.0),
    ]
    for name, u, v, uv, myaw in r2:
        cubes.extend(make_aligned_scale("scales", name, u, v, w=2.08, d=1.85, layer_idx=2, uv_preset=uv, micro_yaw=myaw))

    # Row 3 (顶端位 3 片)
    r3 = [
        ("s3_0", -2.1, 2.95, "scale_plate_b", 1.2),
        ("s3_1", 0.0, 2.95, "scale_plate_a", -0.5),
        ("s3_2", 2.1, 2.95, "scale_plate_b", -1.2),
    ]
    for name, u, v, uv, myaw in r3:
        cubes.extend(make_aligned_scale("scales", name, u, v, w=2.08, d=1.85, layer_idx=3, uv_preset=uv, micro_yaw=myaw))

    return cubes


def part_creeping_moss() -> list[Cube]:
    """部件3：贴石蔓延的苍翠苔藓边缘与气生微须 (Creeping Moss Margins & Tendrils)。"""
    cubes: list[Cube] = []

    # 贴着斜坡边缘的紧密苔藓团 (在斜坡局部坐标系下)
    moss_patches = [
        # 右侧侧翼边缘
        ("m_r0", 4.3, -1.0, 1.4, 1.8, 0.45, "moss_vibrant"),
        ("m_r1", 4.4, 1.2, 1.3, 1.6, 0.45, "moss_deep"),
        ("m_r2", 3.3, -2.9, 1.3, 1.5, 0.40, "moss_vibrant"),
        ("m_r3", 3.4, 2.9, 1.4, 1.5, 0.45, "moss_vibrant"),
        # 左侧侧翼边缘
        ("m_l0", -4.3, -1.0, 1.4, 1.8, 0.45, "moss_vibrant"),
        ("m_l1", -4.4, 1.2, 1.3, 1.6, 0.45, "moss_deep"),
        ("m_l2", -3.3, -2.9, 1.3, 1.5, 0.40, "moss_vibrant"),
        ("m_l3", -3.4, 2.9, 1.4, 1.5, 0.45, "moss_vibrant"),
        # 顶端群苔
        ("m_top0", 0.0, 4.2, 2.6, 1.4, 0.55, "moss_vibrant"),
        ("m_top1", -1.8, 4.1, 1.4, 1.2, 0.50, "moss_deep"),
        ("m_top2", 1.8, 4.1, 1.4, 1.2, 0.50, "moss_deep"),
        # 底端群苔
        ("m_bot0", 0.0, -4.2, 2.5, 1.3, 0.40, "moss_vibrant"),
        ("m_bot1", -1.7, -4.1, 1.3, 1.1, 0.40, "moss_deep"),
        ("m_bot2", 1.7, -4.1, 1.3, 1.1, 0.40, "moss_deep"),
        # 穿插在鳞片缝隙的小苔藓斑
        ("m_gap0", -2.1, -1.9, 0.6, 0.6, 0.25, "moss_vibrant"),
        ("m_gap1", 2.1, -1.9, 0.6, 0.6, 0.25, "moss_vibrant"),
        ("m_gap2", -2.1, 1.9, 0.6, 0.6, 0.25, "moss_vibrant"),
        ("m_gap3", 2.1, 1.9, 0.6, 0.6, 0.25, "moss_vibrant"),
    ]

    for name, u, v, mw, md, mh, uv in moss_patches:
        lx = 8.0 + u
        lz = 8.0 - v
        ly = 2.2 + 0.05
        cubes.append(
            Cube(
                "moss",
                name,
                (lx - mw / 2, ly, lz - md / 2),
                (lx + mw / 2, ly + mh, lz + md / 2),
                uv,
                rotation=SLOPE_ROT,
                rot_origin=SLOPE_ORG,
            )
        )

    # 伸向边缘空气中的微小孢子须
    tendrils_local = [
        ("t0", 4.9, 0.0, 0.25, 0.25, 0.8, (15.0, 20.0, 25.0)),
        ("t1", -4.9, 0.0, 0.25, 0.25, 0.8, (-15.0, -20.0, -25.0)),
        ("t2", 0.0, 4.6, 0.25, 0.25, 0.9, (20.0, 0.0, 0.0)),
        ("t3", 2.2, -4.4, 0.25, 0.25, 0.7, (-20.0, 15.0, 0.0)),
        ("t4", -2.2, -4.4, 0.25, 0.25, 0.7, (-20.0, -15.0, 0.0)),
    ]
    for name, u, v, tw, td, th, t_rot in tendrils_local:
        lx = 8.0 + u
        lz = 8.0 - v
        ly = 2.2 + 0.30
        cubes.append(
            Cube(
                "moss",
                name,
                (lx - tw / 2, ly, lz - td / 2),
                (lx + tw / 2, ly + th, lz + td / 2),
                "moss_sprout",
                rotation=(SLOPE_ROT[0] + t_rot[0], SLOPE_ROT[1] + t_rot[1], SLOPE_ROT[2] + t_rot[2]),
                rot_origin=(lx, ly, lz),
            )
        )

    return cubes


def all_cubes() -> list[Cube]:
    return part_abyssal_base() + part_dragon_scales() + part_creeping_moss()


# ─────────────────────────────────────────────────────────────────────────────
# 精确匹配原画的专业手绘 Palette (4~6 色，绝无纯黑纯白)
# ─────────────────────────────────────────────────────────────────────────────

# 1. 灰青角质龙鳞 (Dragon Scale)
SCALE_PALETTE = {
    "rim":    (178, 202, 192),  # #B2CAA0 - 原画最亮前沿高光
    "light":  (130, 158, 148),  # #829E94 - 鳞片向光上部浅灰青
    "mid":    (88, 114, 106),   # #58726A - 鳞片主体核心灰青 (65% 干净休息区)
    "dark":   (56, 78, 72),     # #384E48 - 鳞片下缘与侧向过渡
    "shadow": (36, 52, 48),     # #243430 - 鳞片接触手绘 AO 凹槽
    "calc":   (148, 172, 162),  # #94ACA2 - 微小角质钙化斑点
}

# 2. 鲜活苍翠苔藓 (Vibrant Moss)
MOSS_VIBRANT_PALETTE = {
    "sprout": (196, 232, 58),   # #C4E83A - 原画初生鲜黄绿亮芽
    "light":  (152, 192, 42),   # #98C02A - 苔藓向光面
    "mid":    (102, 146, 30),   # #66921E - 苔丛主体生机绿
    "dark":   (58, 90, 24),     # #3A5A18 - 苔丛内凹阴影
    "root":   (30, 52, 18),     # #1E3412 - 苔藓贴石根部 AO
}

# 3. 幽暗深层苔藓 (Deep Moss)
MOSS_DEEP_PALETTE = {
    "light":  (70, 102, 28),    # #46661C
    "mid":    (46, 74, 22),     # #2E4A16
    "dark":   (26, 44, 14),     # #1A2C0E
    "root":   (16, 26, 10),     # #101A0A
}

# 4. 坍缩渊深冷玄岩 (Abyssal Rock)
ROCK_PALETTE = {
    "high":   (76, 86, 106),    # #4C566A - 坚硬棱线冷灰高光
    "light":  (56, 64, 80),     # #384050 - 向光大块面 (干净休息区)
    "mid":    (38, 44, 56),     # #262C38 - 玄岩主体中灰
    "dark":   (24, 28, 38),     # #181C26 - 阴面与凹槽
    "ao":     (14, 16, 24),     # #0E1018 - 接触手绘 AO
}


def create_texture() -> Image.Image:
    """生成 64x64 UV Atlas 高清龙鳞苔贴图 (手绘像素级高质量材质与明暗版)。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # ─────────────────────────────────────────────────────────────
    # 1. 灰青龙鳞硬面 A (scale_plate_a: 0,0~32,16)
    # 大面平滑干净 (65% 休息区) + 顶部向光微明 + 四周深色手绘 AO 槽 + 稀疏角质环
    for x in range(32):
        for y in range(16):
            u = x
            v = y

            # 边框距离判定 (用于手绘 AO 缝与边缘)
            edge_dist = min(u, 31 - u, v, 15 - v)

            if edge_dist == 0:
                # 最外圈接触阴影 AO
                col = SCALE_PALETTE["shadow"]
            elif edge_dist == 1:
                # 倒角过渡暗色
                col = SCALE_PALETTE["dark"]
            elif v <= 4:
                # 鳞片向光上部浅色面
                col = SCALE_PALETTE["light"]
            else:
                # 鳞片平滑主体色 (干净休息区)
                col = SCALE_PALETTE["mid"]

            # 顶部中段微高光
            if v == 1 and 8 <= u <= 23:
                col = SCALE_PALETTE["rim"]

            # 稀疏自然的微小角质纹与钙化点 (固定结构，绝无全图噪点)
            if (u == 12 and v == 8) or (u == 20 and v == 9):
                col = SCALE_PALETTE["calc"]
            elif (u == 16 and v == 12):
                col = SCALE_PALETTE["dark"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 2. 灰青龙鳞硬面 B (scale_plate_b: 0,16~32,32)
    # 伴生片，具有对称位置的向光面与微钙化点，消除重复
    for x in range(32):
        for y in range(16, 32):
            u = x
            v = y - 16

            edge_dist = min(u, 31 - u, v, 15 - v)

            if edge_dist == 0:
                col = SCALE_PALETTE["shadow"]
            elif edge_dist == 1:
                col = SCALE_PALETTE["dark"]
            elif v <= 3:
                col = SCALE_PALETTE["light"]
            else:
                col = SCALE_PALETTE["mid"]

            if v == 1 and 6 <= u <= 25:
                col = SCALE_PALETTE["rim"]

            # 独立的稀疏钙化斑
            if (u == 9 and v == 9) or (u == 23 and v == 7):
                col = SCALE_PALETTE["calc"]
            elif (u == 15 and v == 11):
                col = SCALE_PALETTE["dark"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 3. 龙鳞前缘锐利高光倒角棱 (scale_edge: 0,32~32,48)
    # 向光切面：顶端高光亮青白 -> 中段浅青 -> 下缘暗色倒角
    for x in range(32):
        for y in range(32, 48):
            u = x
            v = y - 32

            if v <= 2:
                # 倒角顶端锐利反光
                col = SCALE_PALETTE["rim"]
            elif v <= 6:
                # 倒角向光斜面
                col = SCALE_PALETTE["light"]
            elif v <= 12:
                # 倒角主体
                col = SCALE_PALETTE["mid"]
            elif v <= 14:
                # 倒角背光下缘
                col = SCALE_PALETTE["dark"]
            else:
                # 倒角底部接触暗槽
                col = SCALE_PALETTE["shadow"]

            # 边缘两侧暗角
            if u == 0 or u == 31:
                col = SCALE_PALETTE["dark"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 4. 鲜活苍翠苔藓 (moss_vibrant: 32,0~64,24)
    # 簇状手绘像素团块：外凸亮黄绿 -> 侧面生机绿 -> 深处墨绿 AO
    for x in range(32, 64):
        for y in range(24):
            u = x - 32
            v = y

            # 手绘块状簇 (Cluster pattern)
            cluster_cell = ((u // 4) + (v // 4) * 3) % 4
            cell_u = u % 4
            cell_v = v % 4

            if cluster_cell == 0:
                if cell_v <= 1:
                    col = MOSS_VIBRANT_PALETTE["sprout"]
                elif cell_u <= 2:
                    col = MOSS_VIBRANT_PALETTE["light"]
                else:
                    col = MOSS_VIBRANT_PALETTE["mid"]
            elif cluster_cell == 1:
                if cell_v == 0 and cell_u == 1:
                    col = MOSS_VIBRANT_PALETTE["sprout"]
                elif cell_v <= 2:
                    col = MOSS_VIBRANT_PALETTE["light"]
                else:
                    col = MOSS_VIBRANT_PALETTE["mid"]
            elif cluster_cell == 2:
                if cell_v <= 1:
                    col = MOSS_VIBRANT_PALETTE["mid"]
                else:
                    col = MOSS_VIBRANT_PALETTE["dark"]
            else:
                if cell_v <= 1:
                    col = MOSS_VIBRANT_PALETTE["mid"]
                elif cell_v <= 2:
                    col = MOSS_VIBRANT_PALETTE["dark"]
                else:
                    col = MOSS_VIBRANT_PALETTE["root"]

            # 底部接触面强制 AO
            if v >= 22:
                col = MOSS_VIBRANT_PALETTE["root"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 5. 幽暗深层苔藓 (moss_deep: 32,24~64,40)
    for x in range(32, 64):
        for y in range(24, 40):
            u = x - 32
            v = y - 24

            cluster_cell = ((u // 4) + (v // 4) * 2) % 3
            cell_v = v % 4

            if cluster_cell == 0:
                col = MOSS_DEEP_PALETTE["light"] if cell_v <= 1 else MOSS_DEEP_PALETTE["mid"]
            elif cluster_cell == 1:
                col = MOSS_DEEP_PALETTE["mid"] if cell_v <= 2 else MOSS_DEEP_PALETTE["dark"]
            else:
                col = MOSS_DEEP_PALETTE["dark"] if cell_v <= 1 else MOSS_DEEP_PALETTE["root"]

            if v >= 14:
                col = MOSS_DEEP_PALETTE["root"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 6. 初生气生孢子芽 (moss_sprout: 0,48~32,64)
    for x in range(32):
        for y in range(48, 64):
            u = x
            v = y - 48

            if v <= 3:
                col = MOSS_VIBRANT_PALETTE["sprout"]
            elif v <= 9:
                col = MOSS_VIBRANT_PALETTE["light"]
            elif v <= 13:
                col = MOSS_VIBRANT_PALETTE["mid"]
            else:
                col = MOSS_VIBRANT_PALETTE["dark"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 7. 坍缩渊冷暗玄岩基质 (abyssal_rock: 32,40~64,52)
    # 大块面朝向明暗 (顶亮侧暗) + 坚硬石面节理
    for x in range(32, 64):
        for y in range(40, 52):
            u = x - 32
            v = y - 40

            if v <= 2:
                col = ROCK_PALETTE["high"]
            elif v <= 5:
                col = ROCK_PALETTE["light"]
            elif v <= 9:
                col = ROCK_PALETTE["mid"]
            else:
                col = ROCK_PALETTE["dark"]

            # 单条天然斜向节理裂隙 (手绘结构)
            if (u == v * 2 or u == v * 2 + 1) and 3 <= v <= 8:
                col = ROCK_PALETTE["dark"]

            img.putpixel((x, y), (*col, 255))

    # ─────────────────────────────────────────────────────────────
    # 8. 坍缩渊深层暗底 (abyssal_rock_dark: 32,52~64,64)
    for x in range(32, 64):
        for y in range(52, 64):
            u = x - 32
            v = y - 52

            if v <= 1:
                col = ROCK_PALETTE["mid"]
            elif v <= 8:
                col = ROCK_PALETTE["dark"]
            else:
                col = ROCK_PALETTE["ao"]

            img.putpixel((x, y), (*col, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "scale_plate_a": [0, 0, 32, 16],
        "scale_plate_b": [0, 16, 32, 32],
        "scale_edge": [0, 32, 32, 48],
        "moss_vibrant": [32, 0, 64, 24],
        "moss_deep": [32, 24, 64, 40],
        "moss_sprout": [0, 48, 32, 64],
        "abyssal_rock": [32, 40, 64, 52],
        "abyssal_rock_dark": [32, 52, 64, 64],
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
        "name": "LongLinTai",
        "model_identifier": "long_lin_tai",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "long_lin_tai.png",
                "name": "long_lin_tai",
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
    out_bbmodel = LOCAL_MODELS / "LongLinTai.bbmodel"
    out_bbmodel.write_text(json.dumps(bb_dict, indent=2))
    print(f"Generated BBModel: {out_bbmodel} (Elements: {len(bb_dict['elements'])})")

    # 保存 UV 贴图预览
    tex = create_texture()
    tex.save(PREVIEW_DIR / "long_lin_tai_texture.png")
    print(f"Saved texture atlas: {PREVIEW_DIR / 'long_lin_tai_texture.png'}")


if __name__ == "__main__":
    main()
