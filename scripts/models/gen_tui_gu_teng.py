#!/usr/bin/env python3
"""生成蜕骨藤 (TuiGuTeng) Blockbench .bbmodel 与三视图预览 (满量多藤纠缠与荆棘冠冕终极版)。

【核心突破点】：
1. 彻底解决“单薄线圈感”：
   - 之前只有 1 根主藤斜切，显得像一条皮带。
   - 现在引入 4 组交织藤系，形成原画中标志性的【荆棘环形囚笼 (Full Thorny Wreath)】：
     * 藤系 1 (主斜切绞杀藤)：从下颌横切咬死面部，过右太阳穴盘向颅顶。
     * 藤系 2 (颅顶荆棘王冠与高耸双角藤)：从后枕骨分叉直冲云霄，左右双角分叉挺拔，顶部自然向内回钩卷须。
     * 藤系 3 (左眼眶贯穿与前伸主怒芽)：破眶而出向左前方挺立，带多段卷曲与斜刺。
     * 藤系 4 (左脸颊与下颌合围藤)：从左下生根向上环抱，与主藤 1 形成交叉“X”字形绞杀锁！
     * 藤系 5 (脊柱缠骨与地缚触手)：深扎后背肋骨缝隙与碎骨堆。
2. 倒刺密集化与形态化 (32+ 处尖锐倒钩刺群)：
   - 沿每根藤蔓的外弧线密集长出倾斜外扬的荆棘倒钩刺（0.25×0.65×0.25），呈现刺痛与危险感。
3. 骷髅骨相与幽紫真元：
   - 额头紫电裂隙、左右眼眶深邃反差、上颌牙列、肋骨散骸。
4. 64x64 UV Atlas 高清烘焙：
   - 暗黑紫木质老皮（0x2A1230）+ 螺旋扭曲纤维 + 鲜亮粉紫尖刺（0xFA48C8）+ 额头紫电发光裂痕。
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
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c0008")


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


def make_directed_vine_segment(
    bone: str,
    name: str,
    p1: tuple[float, float, float],
    p2: tuple[float, float, float],
    thickness: float = 0.75,
    uv_preset: str = "vine_main",
) -> Cube:
    """在空间两点 p1, p2 之间创建一节真实倾斜旋转的藤身体素柱。"""
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


def build_vine_chain(
    bone: str,
    prefix: str,
    path_points: list[tuple[float, float, float]],
    thickness_start: float = 0.85,
    thickness_end: float = 0.55,
    preset: str = "vine_main",
    thorn_preset: str = "vine_thorn",
) -> list[Cube]:
    """生成一条连续光滑的柔性藤蔓链。"""
    cubes: list[Cube] = []
    n = len(path_points)
    for i in range(n - 1):
        p1 = path_points[i]
        p2 = path_points[i + 1]
        progress = i / max(1, n - 2)
        t = thickness_start + (thickness_end - thickness_start) * progress
        p = preset if i < n - 3 else thorn_preset
        cubes.append(
            make_directed_vine_segment(bone, f"{prefix}_{i:02d}", p1, p2, t, p)
        )
    return cubes


def part_skull_anatomy() -> list[Cube]:
    """精雕骷髅头骨。"""
    cubes: list[Cube] = []
    sk_rot = (6.0, 12.0, -8.0)
    sk_org = (8.0, 4.5, 6.5)

    # 1. 脑颅主舱
    cubes.append(
        Cube(
            "skull",
            "skull_braincase",
            (5.4, 3.8, 4.5),
            (10.6, 8.0, 9.2),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 2. 颅顶穹顶
    cubes.append(
        Cube(
            "skull",
            "skull_top_dome",
            (5.8, 7.8, 4.8),
            (10.2, 9.2, 8.8),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 3. 后枕骨斜收面
    cubes.append(
        Cube(
            "skull",
            "skull_occipital",
            (5.8, 3.0, 8.8),
            (10.2, 7.4, 10.2),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 4. 额骨 (带紫裂纹)
    cubes.append(
        Cube(
            "skull",
            "skull_forehead",
            (5.6, 6.0, 3.5),
            (10.4, 8.0, 4.8),
            "bone_crack",
            sk_rot,
            sk_org,
        )
    )
    # 5. 眉弓骨
    cubes.append(
        Cube(
            "skull",
            "skull_brow_ridge",
            (5.2, 5.4, 2.8),
            (10.8, 6.2, 3.8),
            "bone_crack",
            sk_rot,
            sk_org,
        )
    )
    # 6. 鼻骨中柱
    cubes.append(
        Cube(
            "skull",
            "skull_nasal_septum",
            (7.6, 3.8, 2.8),
            (8.4, 5.6, 3.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 7. 右颧骨与外眼眶
    cubes.append(
        Cube(
            "skull",
            "skull_cheek_right",
            (9.6, 3.4, 2.8),
            (11.0, 5.6, 4.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 8. 左颧骨 (被穿破)
    cubes.append(
        Cube(
            "skull",
            "skull_cheek_left",
            (5.0, 3.4, 2.8),
            (6.4, 5.6, 4.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 9. 右眼眶深邃暗洞
    cubes.append(
        Cube(
            "skull",
            "skull_orbit_right_dark",
            (8.4, 4.0, 3.4),
            (9.8, 5.4, 4.5),
            "orbit_dark",
            sk_rot,
            sk_org,
        )
    )
    # 10. 上颌骨
    cubes.append(
        Cube(
            "skull",
            "skull_maxilla",
            (6.2, 2.2, 3.0),
            (9.8, 3.6, 4.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 11. 参差牙列
    cubes.append(
        Cube(
            "skull",
            "skull_teeth_row",
            (6.5, 1.4, 3.1),
            (9.5, 2.3, 4.1),
            "skull_teeth",
            sk_rot,
            sk_org,
        )
    )
    # 12. 额骨紫电裂隙
    cubes.append(
        Cube(
            "skull",
            "skull_fissure_glow",
            (8.0, 6.2, 3.2),
            (9.2, 8.2, 4.2),
            "core_glow",
            sk_rot,
            sk_org,
        )
    )
    return cubes


def part_skeleton_bed() -> list[Cube]:
    """散落骨架残骸与肋骨。"""
    cubes: list[Cube] = []
    # 颈椎
    cubes.append(
        Cube(
            "skeleton",
            "cervical_spine",
            (7.2, 0.6, 6.5),
            (8.8, 3.2, 8.2),
            "ground_bone",
            (12.0, 5.0, 0.0),
            (8.0, 1.5, 7.0),
        )
    )
    # 右大肋骨
    cubes.append(
        Cube(
            "skeleton",
            "rib_arch_right_1",
            (8.8, 0.8, 7.5),
            (12.8, 2.0, 10.5),
            "bone_clean",
            (-10.0, 25.0, 15.0),
            (9.0, 1.0, 8.0),
        )
    )
    cubes.append(
        Cube(
            "skeleton",
            "rib_arch_right_tip",
            (12.2, 1.4, 9.8),
            (14.2, 2.4, 12.8),
            "bone_clean",
            (-15.0, 45.0, 25.0),
            (12.5, 2.0, 10.5),
        )
    )
    # 后肋骨
    cubes.append(
        Cube(
            "skeleton",
            "rib_arch_back",
            (9.6, 2.0, 8.5),
            (12.4, 4.2, 11.2),
            "bone_clean",
            (20.0, 10.0, -15.0),
            (10.0, 2.5, 9.0),
        )
    )
    # 断裂下颌骨
    cubes.append(
        Cube(
            "skeleton",
            "broken_mandible_left",
            (3.8, 0.2, 4.0),
            (5.4, 1.8, 7.2),
            "ground_bone",
            (5.0, -25.0, 10.0),
            (4.5, 0.5, 5.5),
        )
    )
    # 地面碎骨堆
    cubes.append(
        Cube(
            "skeleton",
            "bone_fragments_ground",
            (1.8, 0.0, 3.0),
            (5.8, 0.7, 8.5),
            "ground_bone",
            (0.0, -15.0, 0.0),
            (3.5, 0.0, 5.5),
        )
    )
    cubes.append(
        Cube(
            "skeleton",
            "bone_fragment_right",
            (10.5, 0.0, 2.0),
            (13.8, 0.8, 5.5),
            "ground_bone",
            (0.0, 30.0, 0.0),
            (12.0, 0.0, 3.5),
        )
    )
    return cubes


def part_full_intertwined_vines() -> list[Cube]:
    """五大交织藤系，形成完整的荆棘环形绞杀笼 (Full Thorny Wreath)。"""
    cubes: list[Cube] = []

    # ─────────────────────────────────────────────────────────────
    # 【藤系 1：主对角线绞杀藤 (Primary Diagonal Vine)】
    # 从右侧地面出土 -> 划过下颌中线 -> 攀上左颧骨 -> 勒过眉弓与额顶
    # ─────────────────────────────────────────────────────────────
    path_vine1 = [
        (11.8, 0.0, 2.0),
        (10.4, 0.6, 2.2),
        (8.8, 1.6, 2.2),
        (7.0, 2.8, 2.1),
        (5.2, 4.0, 2.4),
        (4.4, 5.4, 3.0),
        (4.8, 6.8, 3.8),
        (6.4, 7.8, 4.4),
        (8.4, 8.4, 5.0),
    ]
    cubes.extend(
        build_vine_chain(
            "vines", "vine1_diag", path_vine1, 0.85, 0.70, "vine_main"
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【藤系 2：破眼而出的前伸怒首藤 (Orbital Eruption & Front Sprout)】
    # 从颅腔真元晶核爆发 -> 破左眼眶而出 -> 向前上方挺拔成蝎尾卷须
    # ─────────────────────────────────────────────────────────────
    path_vine2 = [
        (7.4, 4.5, 4.5),
        (6.5, 4.6, 3.2),
        (5.6, 4.8, 1.8),
        (4.8, 5.6, 0.4),
        (4.2, 6.8, -0.6),
        (3.6, 8.0, -1.2),
        (3.2, 9.2, -1.0),
        (3.0, 10.2, -0.6),
    ]
    cubes.extend(
        build_vine_chain(
            "vines", "vine2_eye", path_vine2, 0.80, 0.50, "vine_main"
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【藤系 3：颅顶荆棘王冠与高耸双角主藤 (Crown Wreath & High Spire A/B)】
    # 3A 主尖角：从后枕骨分叉直冲天际，达到 15.8 格高度，向内微卷
    # ─────────────────────────────────────────────────────────────
    path_vine3A = [
        (8.4, 8.4, 5.0),  # 颅顶汇合点
        (9.4, 9.6, 5.6),  # 向上收拢
        (9.0, 11.2, 5.2),  # 拔起主干
        (8.2, 12.8, 4.8),  # 向上收缩
        (7.6, 14.4, 4.4),  # 顶段微弯
        (7.0, 15.8, 4.2),  # 最高尖梢 (15.8 格)
    ]
    cubes.extend(
        build_vine_chain(
            "vines", "vine3A_spire", path_vine3A, 0.75, 0.45, "vine_main"
        )
    )

    # 3B 侧角分枝 (Crown Horn B)：向右前方展开的第二根高耸荆棘角
    path_vine3B = [
        (9.0, 11.2, 5.2),  # 分叉点
        (10.0, 12.4, 4.6),  # 向右前伸出
        (10.8, 13.8, 4.0),  # 向上挺立
        (11.2, 14.8, 3.6),  # 侧角尖梢 (14.8 格)
    ]
    cubes.extend(
        build_vine_chain(
            "vines", "vine3B_horn", path_vine3B, 0.65, 0.40, "vine_thorn"
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【藤系 4：左侧合围与下颌锁紧藤 (Left Jaw & Cheek Enclosure)】
    # 从左下地面生根 -> 向上包抄左颌骨 -> 与主藤 1 形成交叉环抱
    # ─────────────────────────────────────────────────────────────
    path_vine4 = [
        (2.4, 0.0, 2.6),  # 左地生根
        (3.4, 1.2, 2.8),  # 爬向下颌角
        (4.2, 2.6, 3.4),  # 勒紧左下颌骨
        (4.6, 4.2, 4.2),  # 穿过左颧骨内侧
        (5.2, 5.8, 5.2),  # 汇入左颞骨
        (6.2, 7.2, 5.8),  # 盘向后颅顶
        (8.4, 8.4, 5.0),  # 与颅顶主冠汇合！
    ]
    cubes.extend(
        build_vine_chain(
            "vines", "vine4_left_wrap", path_vine4, 0.75, 0.60, "vine_main"
        )
    )

    # ─────────────────────────────────────────────────────────────
    # 【藤系 5：脊柱缠骨与后背肋骨藤 (Spine & Ribcage Constrictor)】
    # 从颈椎发源 -> 缠死大肋骨 -> 深入右后方地面
    # ─────────────────────────────────────────────────────────────
    path_vine5 = [
        (7.6, 1.8, 7.6),
        (8.8, 3.0, 8.4),
        (9.8, 2.2, 8.0),
        (10.8, 1.2, 8.8),
        (12.2, 0.4, 9.6),
        (13.4, 0.0, 10.4),
    ]
    cubes.extend(
        build_vine_chain(
            "vines", "vine5_rib_choker", path_vine5, 0.70, 0.50, "vine_main"
        )
    )

    return cubes


def part_dense_thorns_and_glows() -> list[Cube]:
    """密集荆棘倒钩刺群 (28+ 处) 与真元灵核。"""
    cubes: list[Cube] = []

    # 倒刺规格：小巧长薄柱 (0.24 x 0.65 x 0.24)，锚入藤身，尖端向外斜挑
    # 格式: (name, position, size, rotation, rot_origin)
    thorns = [
        # 主藤 1 倒刺
        ("th1_01", (9.8, 1.2, 1.6), (0.24, 0.65, 0.24), (25, 45, -35), (10.0, 1.5, 1.8)),
        ("th1_02", (7.8, 2.2, 1.5), (0.24, 0.65, 0.24), (-20, 20, 45), (8.0, 2.5, 1.7)),
        ("th1_03", (5.8, 3.4, 1.8), (0.24, 0.65, 0.24), (-35, -30, 40), (6.0, 3.7, 2.0)),
        ("th1_04", (4.2, 4.8, 2.4), (0.24, 0.65, 0.24), (15, -40, -55), (4.4, 5.1, 2.6)),
        ("th1_05", (4.4, 6.2, 3.2), (0.24, 0.65, 0.24), (30, 15, -45), (4.6, 6.5, 3.4)),
        ("th1_06", (6.0, 7.2, 3.8), (0.24, 0.65, 0.24), (-25, -25, 30), (6.2, 7.5, 4.0)),
        ("th1_07", (8.0, 8.0, 4.4), (0.24, 0.65, 0.24), (20, 35, 45), (8.2, 8.3, 4.6)),

        # 破眼藤 2 倒刺
        ("th2_01", (5.0, 5.2, 0.8), (0.24, 0.65, 0.24), (35, -45, 40), (5.2, 5.5, 1.0)),
        ("th2_02", (4.2, 6.4, -0.2), (0.24, 0.65, 0.24), (-20, -50, -35), (4.4, 6.7, 0.0)),
        ("th2_03", (3.6, 7.6, -0.8), (0.24, 0.65, 0.24), (45, -60, 50), (3.8, 7.9, -0.6)),
        ("th2_04", (3.2, 8.8, -1.2), (0.24, 0.65, 0.24), (-35, -30, -45), (3.4, 9.1, -1.0)),
        ("th2_05", (2.8, 9.8, -0.8), (0.24, 0.65, 0.24), (50, -70, 60), (3.0, 10.1, -0.6)),

        # 冲天主藤 3A 倒刺
        ("th3A_01", (9.6, 9.8, 5.2), (0.24, 0.65, 0.24), (40, 50, -30), (9.8, 10.1, 5.4)),
        ("th3A_02", (9.2, 11.2, 5.0), (0.24, 0.65, 0.24), (-30, 25, 45), (9.4, 11.5, 5.2)),
        ("th3A_03", (8.6, 12.6, 4.6), (0.24, 0.65, 0.24), (25, -35, -40), (8.8, 12.9, 4.8)),
        ("th3A_04", (7.8, 14.0, 4.2), (0.24, 0.65, 0.24), (-40, 40, 30), (8.0, 14.3, 4.4)),
        ("th3A_05", (7.2, 15.2, 4.0), (0.24, 0.65, 0.24), (45, 55, 35), (7.4, 15.5, 4.2)),

        # 冲天侧角 3B 倒刺
        ("th3B_01", (10.2, 12.2, 4.4), (0.24, 0.65, 0.24), (30, 60, 30), (10.4, 12.5, 4.6)),
        ("th3B_02", (11.0, 13.6, 3.8), (0.24, 0.65, 0.24), (45, 75, 40), (11.2, 13.9, 4.0)),
        ("th3B_03", (11.4, 14.6, 3.4), (0.24, 0.65, 0.24), (55, 80, 50), (11.6, 14.9, 3.6)),

        # 左侧合围藤 4 倒刺
        ("th4_01", (2.8, 1.0, 2.6), (0.24, 0.65, 0.24), (10, 25, -60), (3.0, 1.3, 2.8)),
        ("th4_02", (3.8, 2.4, 3.2), (0.24, 0.65, 0.24), (-25, 35, -45), (4.0, 2.7, 3.4)),
        ("th4_03", (4.4, 3.8, 3.8), (0.24, 0.65, 0.24), (20, -20, -50), (4.6, 4.1, 4.0)),
        ("th4_04", (5.0, 5.4, 4.8), (0.24, 0.65, 0.24), (35, 10, -40), (5.2, 5.7, 5.0)),
        ("th4_05", (6.0, 6.8, 5.4), (0.24, 0.65, 0.24), (-30, 45, 35), (6.2, 7.1, 5.6)),

        # 肋骨藤 5 倒刺
        ("th5_01", (10.2, 2.6, 8.4), (0.24, 0.65, 0.24), (-20, 45, 35), (10.4, 2.9, 8.6)),
        ("th5_02", (11.2, 1.4, 9.0), (0.24, 0.65, 0.24), (25, -40, -30), (11.4, 1.7, 9.2)),
        ("th5_03", (12.4, 0.6, 9.8), (0.24, 0.65, 0.24), (10, -55, -25), (12.6, 0.9, 10.0)),
    ]

    for name, pos, size, rot, org in thorns:
        x, y, z = pos
        sx, sy, sz = size
        cubes.append(
            Cube(
                "thorns",
                name,
                (x, y, z),
                (x + sx, y + sy, z + sz),
                "vine_thorn",
                rot,
                org,
            )
        )

    # 幽紫真元晶核
    cubes.append(
        Cube(
            "core",
            "qi_crystal_eye_core",
            (5.8, 4.4, 1.8),
            (7.0, 5.6, 3.2),
            "core_glow",
            (6.0, 12.0, -8.0),
            (8.0, 4.5, 6.5),
        )
    )
    cubes.append(
        Cube(
            "core",
            "qi_crystal_marrow_glow",
            (6.8, 5.0, 5.2),
            (9.2, 7.2, 7.5),
            "core_glow",
            (6.0, 12.0, -8.0),
            (8.0, 4.5, 6.5),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_skull_anatomy()
        + part_skeleton_bed()
        + part_full_intertwined_vines()
        + part_dense_thorns_and_glows()
    )


def create_texture() -> Image.Image:
    """生成 64x64 UV Atlas 贴图。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 骨骼纯净/风化面 (0,0)~(32,20)
    for x in range(32):
        for y in range(20):
            noise = ((x * 17 + y * 31) % 19)
            base = 210 + noise
            r = min(255, base + 10)
            g = min(255, base + 6)
            b = max(0, base - 16)
            if (x * 7 + y * 13) % 29 == 0:
                r, g, b = r - 45, g - 40, b - 35
            img.putpixel((x, y), (r, g, b, 255))

    # 2. 额骨紫裂纹面 (0,20)~(32,36)
    for x in range(32):
        for y in range(20, 36):
            u = x
            v = y - 20
            noise = ((x * 19 + y * 23) % 17)
            base = 205 + noise
            r = min(255, base + 8)
            g = min(255, base + 4)
            b = max(0, base - 18)
            is_crack = (u == 12 + int(math.sin(v * 0.8) * 4)) or (u == 13 and v % 3 == 0)
            if is_crack:
                r, g, b = 215, 60, 255
            elif abs(u - (12 + int(math.sin(v * 0.8) * 4))) == 1:
                r, g, b = 120, 35, 160
            img.putpixel((x, y), (r, g, b, 255))

    # 3. 牙齿与眼眶深部 (0,36)~(32,48)
    for x in range(32):
        for y in range(36, 48):
            if y < 42:
                if x % 4 == 0 or y == 41:
                    img.putpixel((x, y), (45, 38, 32, 255))
                else:
                    shade = 225 + ((x * 7 + y * 5) % 15)
                    img.putpixel((x, y), (shade, shade - 10, shade - 35, 255))
            else:
                img.putpixel((x, y), (20, 10, 28, 255))

    # 4. 地面碎骨与土污 (0,48)~(32,64)
    for x in range(32):
        for y in range(48, 64):
            noise = ((x * 13 + y * 29) % 23)
            base = 145 + noise
            r = base + 8
            g = base + 4
            b = base - 14
            if (x + y) % 4 == 0:
                r, g, b = r - 40, g - 30, b - 40
            img.putpixel((x, y), (max(0, r), max(0, g), max(0, b), 255))

    # 5. 【暗黑紫藤老皮 (32,0)~(64,24)】
    for x in range(32, 64):
        for y in range(24):
            u = x - 32
            v = y
            twist = (u * 3 + v * 4) % 12
            bark_groove = int(math.sin(twist / 12.0 * math.pi * 2) * 20)
            noise = ((u * 19 + v * 23) % 11)
            r = max(0, min(255, 46 + bark_groove + noise))
            g = max(0, min(255, 18 + (bark_groove // 2) + (noise // 2)))
            b = max(0, min(255, 68 + bark_groove * 2 + noise))
            if twist == 6:
                r, g, b = min(255, r + 90), min(255, g + 35), min(255, b + 130)
            img.putpixel((x, y), (r, g, b, 255))

    # 6. 【鲜亮荆棘倒刺 (32,24)~(48,44)】
    for x in range(32, 48):
        for y in range(24, 44):
            u = x - 32
            v = y - 24
            grad = (u + v) / 34.0
            r = int(45 * (1 - grad) + 245 * grad)
            g = int(15 * (1 - grad) + 65 * grad)
            b = int(75 * (1 - grad) + 225 * grad)
            img.putpixel((x, y), (min(255, r), min(255, g), min(255, b), 255))

    # 7. 真元晶核与髓光 (48,24)~(64,44)
    for x in range(48, 64):
        for y in range(24, 44):
            dx = abs(x - 56)
            dy = abs(y - 34)
            d = math.sqrt(dx * dx + dy * dy)
            if d < 2.0:
                img.putpixel((x, y), (255, 240, 255, 255))
            elif d < 4.5:
                img.putpixel((x, y), (210, 85, 255, 255))
            else:
                glow = max(0.0, 1.0 - d / 8.0)
                r = int(140 * glow + 35)
                g = int(30 * glow + 10)
                b = int(225 * glow + 55)
                img.putpixel((x, y), (min(255, r), min(255, g), min(255, b), 255))

    # 8. 阴影与过渡 (32,44)~(64,64)
    for x in range(32, 64):
        for y in range(44, 64):
            val = 22 + ((x + y * 2) % 15)
            img.putpixel((x, y), (val, val, val + 12, 255))

    return img


def build_bbmodel_dict() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_base64 = base64.b64encode(buf.getvalue()).decode("ascii")

    uv_presets = {
        "bone_clean": [0, 0, 32, 20],
        "bone_crack": [0, 20, 32, 36],
        "skull_teeth": [0, 36, 32, 42],
        "orbit_dark": [0, 42, 32, 48],
        "ground_bone": [0, 48, 32, 64],
        "vine_main": [32, 0, 64, 24],
        "vine_thorn": [32, 24, 48, 44],
        "core_glow": [48, 24, 64, 44],
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
        "name": "TuiGuTeng",
        "model_identifier": "tui_gu_teng",
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "tui_gu_teng.png",
                "name": "tui_gu_teng",
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
    bb_path = LOCAL_MODELS / "TuiGuTeng.bbmodel"
    bb_path.write_text(json.dumps(bb_dict, indent=2), encoding="utf-8")
    print(f"✅ [终极满量荆棘藤] 生成 BBModel: {bb_path} (共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "tui_gu_teng_texture_r8.png"
    create_texture().save(tex_path)
    print(f"✅ [终极满量荆棘藤] 导出贴图: {tex_path}")


if __name__ == "__main__":
    main()
