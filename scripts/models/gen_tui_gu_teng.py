#!/usr/bin/env python3
"""生成蜕骨藤 (TuiGuTeng) Blockbench .bbmodel 与三视图预览 (Round 3 Final Cut 终轮打磨)。

打磨重点 (Round 3 终轮打磨)：
1. 骷髅解剖重构与比例重校：
   - 缩小粗笨盒体，重构额骨、眉弓、颧骨、左右独立眼眶、倒三角鼻腔、上颌齿列与断裂下颌。
   - 彻底凸显右眼眶深邃暗洞与幽紫灵光，左眼眶则被粗壮紫藤贯穿破出。
   - 在额头与眉弓处增加【残骨紫裂纹 (Purple Lightning Bone Fissure)】，精准还原像素参考图特征。
2. 灵动多段藤蔓骨相绞杀 (Organic Thorny Creepers)：
   - 细化藤蔓粗细（1.2~1.8 单位），避免粗钝方块感，增加蛇形蜿蜒与多关节过渡。
   - 地根 → 咬合勒骨 → 穿入颅底 → 破左眼眶而出昂首挺立。
   - 颅顶荆棘冠冕 (Thorny Crown) 盘绕头骨后方，拔起高达 15.5 格的冲天带刺尖梢。
   - 分布 8 处方向各异的尖锐倒刺与刺芽。
3. 肋骨与风化散骨 (Rib Cage & Spine)：
   - 散落椎骨、弯曲肋骨与破土骨片自然铺陈在底部 16x16 方块空间内。
4. 专属 64x64 UV 贴图与材质贴合：
   - 额头骨裂紫光线、牙齿切线、眼眶阴影渐变、藤皮纤维纹路与刺尖粉紫高光。
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
UUID_NAMESPACE = uuid.UUID("7a3e89b2-6541-4b78-9f12-8d992f4c0003")


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


def part_skull_anatomy() -> list[Cube]:
    """精雕骷髅头骨 (包含额头裂缝、双眼眶凹陷、鼻腔、齿列与残颌)。"""
    cubes: list[Cube] = []
    # 头骨主姿态：仰角 6 度，微向左倾斜 8 度，向右偏航 10 度
    sk_rot = (6.0, 10.0, -8.0)
    sk_org = (8.0, 4.5, 6.5)

    # 1. 脑颅主舱 (Braincase Main)
    cubes.append(
        Cube(
            "skull",
            "skull_braincase",
            (5.4, 4.0, 4.5),
            (10.6, 8.2, 9.2),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 2. 颅顶穹顶 (Top Dome)
    cubes.append(
        Cube(
            "skull",
            "skull_top_dome",
            (5.8, 8.0, 4.8),
            (10.2, 9.4, 8.8),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 3. 后枕骨斜收面 (Occipital)
    cubes.append(
        Cube(
            "skull",
            "skull_occipital",
            (5.8, 3.2, 8.8),
            (10.2, 7.6, 10.2),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 4. 额骨 (Frontal Bone - 右侧带紫裂纹)
    cubes.append(
        Cube(
            "skull",
            "skull_forehead",
            (5.6, 6.2, 3.5),
            (10.4, 8.2, 4.8),
            "bone_crack",
            sk_rot,
            sk_org,
        )
    )
    # 5. 突出眉弓 (Brow Ridge)
    cubes.append(
        Cube(
            "skull",
            "skull_brow_ridge",
            (5.2, 5.6, 2.8),
            (10.8, 6.4, 3.8),
            "bone_crack",
            sk_rot,
            sk_org,
        )
    )
    # 6. 鼻骨中柱 (Nasal Septum)
    cubes.append(
        Cube(
            "skull",
            "skull_nasal_septum",
            (7.6, 4.0, 2.8),
            (8.4, 5.8, 3.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 7. 右颧骨与外眼眶 (Right Cheek & Orbit)
    cubes.append(
        Cube(
            "skull",
            "skull_cheek_right",
            (9.6, 3.6, 2.8),
            (11.0, 5.8, 4.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 8. 左颧骨 (Left Cheek - 被穿破)
    cubes.append(
        Cube(
            "skull",
            "skull_cheek_left",
            (5.0, 3.6, 2.8),
            (6.4, 5.8, 4.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 9. 【右眼眶深邃暗洞 + 灵光底】
    cubes.append(
        Cube(
            "skull",
            "skull_orbit_right_dark",
            (8.4, 4.2, 3.4),
            (9.8, 5.6, 4.5),
            "orbit_dark",
            sk_rot,
            sk_org,
        )
    )
    # 10. 上颌骨 (Maxilla)
    cubes.append(
        Cube(
            "skull",
            "skull_maxilla",
            (6.2, 2.4, 3.0),
            (9.8, 3.8, 4.6),
            "bone_clean",
            sk_rot,
            sk_org,
        )
    )
    # 11. 参差上牙列 (Upper Teeth Row)
    cubes.append(
        Cube(
            "skull",
            "skull_teeth_row",
            (6.5, 1.6, 3.1),
            (9.5, 2.5, 4.1),
            "skull_teeth",
            sk_rot,
            sk_org,
        )
    )
    # 12. 额骨紫裂缝晶亮核心 (Purple Lightning Fissure Glow)
    cubes.append(
        Cube(
            "skull",
            "skull_fissure_glow",
            (8.0, 6.4, 3.2),
            (9.2, 8.4, 4.2),
            "core_glow",
            sk_rot,
            sk_org,
        )
    )
    return cubes


def part_skeleton_bed() -> list[Cube]:
    """散落骨架残骸与脊柱 (Spine & Rib Cage)。"""
    cubes: list[Cube] = []
    # 1. 颈椎骨节
    cubes.append(
        Cube(
            "skeleton",
            "cervical_spine",
            (7.2, 0.8, 6.5),
            (8.8, 3.4, 8.2),
            "ground_bone",
            (12.0, 5.0, 0.0),
            (8.0, 1.5, 7.0),
        )
    )
    # 2. 右侧大肋骨拱弯 1 (弧形展开)
    cubes.append(
        Cube(
            "skeleton",
            "rib_arch_right_1",
            (8.8, 1.0, 7.5),
            (12.8, 2.2, 10.5),
            "bone_clean",
            (-10.0, 25.0, 15.0),
            (9.0, 1.0, 8.0),
        )
    )
    cubes.append(
        Cube(
            "skeleton",
            "rib_arch_right_tip",
            (12.2, 1.6, 9.8),
            (14.2, 2.6, 12.8),
            "bone_clean",
            (-15.0, 45.0, 25.0),
            (12.5, 2.0, 10.5),
        )
    )
    # 3. 后背散落肋骨 2
    cubes.append(
        Cube(
            "skeleton",
            "rib_arch_back",
            (9.6, 2.2, 8.5),
            (12.4, 4.4, 11.2),
            "bone_clean",
            (20.0, 10.0, -15.0),
            (10.0, 2.5, 9.0),
        )
    )
    # 4. 左侧断裂下颌骨片
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
    # 5. 地表散碎骨片堆
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


def part_vines_creeper() -> list[Cube]:
    """暗紫蜕骨藤蔓 (多段衔接、穿眼破骨、冲天藤梢)。"""
    cubes: list[Cube] = []

    # 1. 地面主根盘踞 (根部出土)
    cubes.append(
        Cube(
            "vines",
            "vine_root_base_1",
            (2.2, 0.0, 2.2),
            (4.2, 1.6, 4.8),
            "vine_main",
            (0.0, 35.0, 0.0),
            (3.0, 0.0, 3.5),
        )
    )
    # 2. 向上缠绕左颌下缘
    cubes.append(
        Cube(
            "vines",
            "vine_trunk_lower",
            (3.2, 1.2, 2.8),
            (4.8, 4.4, 4.4),
            "vine_main",
            (22.0, 10.0, -16.0),
            (4.0, 2.0, 3.5),
        )
    )
    # 3. 勒紧左颊并攀入左眼眶下缘
    cubes.append(
        Cube(
            "vines",
            "vine_trunk_mid",
            (4.2, 3.6, 2.4),
            (5.8, 5.8, 4.0),
            "vine_main",
            (12.0, -18.0, -20.0),
            (5.0, 4.2, 3.0),
        )
    )
    # 4. 【核心点睛：粗壮紫藤穿透左眼眶！】
    cubes.append(
        Cube(
            "vines",
            "vine_eye_pierce_stem",
            (5.4, 4.2, 1.4),
            (7.2, 5.8, 3.6),
            "vine_main",
            (18.0, -28.0, 14.0),
            (6.2, 4.8, 2.2),
        )
    )
    # 5. 【破眶昂首的藤芽 (Eye Sprout Stem)】
    cubes.append(
        Cube(
            "vines",
            "vine_eye_sprout_body",
            (5.0, 5.2, 0.0),
            (6.6, 8.0, 1.6),
            "vine_main",
            (-20.0, -35.0, 24.0),
            (5.8, 6.2, 0.8),
        )
    )
    # 6. 破眶藤尖 (Eye Sprout Apex)
    cubes.append(
        Cube(
            "vines",
            "vine_eye_sprout_tip",
            (4.4, 7.6, -1.0),
            (5.6, 9.8, 0.2),
            "vine_thorn",
            (-28.0, -45.0, 32.0),
            (5.0, 8.2, -0.4),
        )
    )

    # 7. 环绕头顶的荆棘冠藤 1 (Crown Vine East)
    cubes.append(
        Cube(
            "vines",
            "vine_crown_strap_east",
            (4.2, 7.8, 3.4),
            (8.6, 9.4, 5.0),
            "vine_main",
            (-8.0, 15.0, 10.0),
            (6.0, 8.5, 4.0),
        )
    )
    # 8. 荆棘冠藤 2 (Crown Vine West & Top)
    cubes.append(
        Cube(
            "vines",
            "vine_crown_strap_top",
            (8.0, 8.2, 4.0),
            (11.0, 9.8, 6.2),
            "vine_main",
            (10.0, 30.0, -8.0),
            (9.5, 9.0, 5.0),
        )
    )
    # 9. 缠绕后枕骨与颈椎后方的藤蔓
    cubes.append(
        Cube(
            "vines",
            "vine_rear_binder",
            (9.2, 4.0, 7.0),
            (11.2, 7.8, 8.8),
            "vine_main",
            (-18.0, 25.0, -10.0),
            (10.0, 5.5, 8.0),
        )
    )
    # 10. 紧勒右侧肋骨的下攀藤
    cubes.append(
        Cube(
            "vines",
            "vine_rib_choker",
            (8.6, 1.4, 8.0),
            (11.8, 3.0, 9.8),
            "vine_main",
            (10.0, -20.0, 12.0),
            (10.0, 2.0, 9.0),
        )
    )

    # 11. 【冲天主藤尖 (Spire Base)】
    cubes.append(
        Cube(
            "vines",
            "vine_spire_main_lower",
            (6.8, 9.2, 4.2),
            (8.4, 12.8, 5.8),
            "vine_main",
            (-10.0, 16.0, 12.0),
            (7.5, 10.0, 5.0),
        )
    )
    # 12. 冲天藤尖上段 (Spire Apex，直冲 15.5 格高度)
    cubes.append(
        Cube(
            "vines",
            "vine_spire_apex",
            (7.2, 12.4, 4.6),
            (8.4, 15.5, 5.6),
            "vine_thorn",
            (-15.0, 22.0, 18.0),
            (7.8, 13.5, 5.0),
        )
    )
    # 13. 冲天藤侧分枝 (Sub Spire Sprout)
    cubes.append(
        Cube(
            "vines",
            "vine_spire_side_branch",
            (8.0, 10.8, 4.0),
            (9.6, 13.4, 5.2),
            "vine_main",
            (18.0, 35.0, 28.0),
            (8.8, 11.5, 4.5),
        )
    )
    return cubes


def part_thorns_and_glows() -> list[Cube]:
    """8 处锋利倒刺与真元灵光 (Thorns & Qi Glow Cores)。"""
    cubes: list[Cube] = []

    # 1. 破眶藤尖倒刺 1
    cubes.append(
        Cube(
            "thorns",
            "thorn_eye_sprout_a",
            (4.0, 6.8, -0.6),
            (4.8, 8.2, 0.2),
            "vine_thorn",
            (30.0, -40.0, 40.0),
            (4.4, 7.5, 0.0),
        )
    )
    # 2. 破眶藤侧倒刺 2
    cubes.append(
        Cube(
            "thorns",
            "thorn_eye_sprout_b",
            (6.2, 6.0, -0.4),
            (7.0, 7.4, 0.4),
            "vine_thorn",
            (-20.0, -20.0, -45.0),
            (6.5, 6.5, 0.0),
        )
    )
    # 3. 颅顶冠藤顶部倒刺
    cubes.append(
        Cube(
            "thorns",
            "thorn_crown_top",
            (5.0, 9.6, 3.8),
            (5.8, 11.0, 4.6),
            "vine_thorn",
            (-25.0, 20.0, -35.0),
            (5.4, 10.0, 4.2),
        )
    )
    # 4. 冲天主藤中段尖刺
    cubes.append(
        Cube(
            "thorns",
            "thorn_spire_mid",
            (8.4, 11.6, 5.2),
            (9.2, 13.0, 5.8),
            "vine_thorn",
            (15.0, 35.0, 40.0),
            (8.6, 12.0, 5.4),
        )
    )
    # 5. 冲天藤侧分枝尖刺
    cubes.append(
        Cube(
            "thorns",
            "thorn_spire_sub",
            (9.4, 12.8, 3.8),
            (10.2, 14.2, 4.4),
            "vine_thorn",
            (25.0, 50.0, 20.0),
            (9.6, 13.0, 4.0),
        )
    )
    # 6. 左侧主茎外侧倒刺
    cubes.append(
        Cube(
            "thorns",
            "thorn_trunk_left",
            (2.4, 3.2, 2.6),
            (3.2, 4.6, 3.4),
            "vine_thorn",
            (10.0, 15.0, -50.0),
            (2.8, 3.8, 3.0),
        )
    )
    # 7. 右后肋骨缠藤倒刺
    cubes.append(
        Cube(
            "thorns",
            "thorn_rib_rear",
            (11.2, 2.4, 9.0),
            (12.0, 3.8, 9.8),
            "vine_thorn",
            (-15.0, 45.0, 30.0),
            (11.5, 3.0, 9.4),
        )
    )

    # 8. 【幽紫真元晶核：左眼眶贯穿核心】
    cubes.append(
        Cube(
            "core",
            "qi_crystal_eye_pierce_glow",
            (5.8, 4.4, 1.8),
            (7.0, 5.6, 3.2),
            "core_glow",
            (6.0, 10.0, -8.0),
            (8.0, 4.5, 6.5),
        )
    )
    # 9. 【颅腔深处幽光：骨髓汲取微光】
    cubes.append(
        Cube(
            "core",
            "qi_crystal_marrow_glow",
            (6.8, 5.0, 5.2),
            (9.2, 7.2, 7.5),
            "core_glow",
            (6.0, 10.0, -8.0),
            (8.0, 4.5, 6.5),
        )
    )
    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_skull_anatomy()
        + part_skeleton_bed()
        + part_vines_creeper()
        + part_thorns_and_glows()
    )


def create_texture() -> Image.Image:
    """生成 64x64 UV Atlas 贴图 (Round 3 终轮精雕贴图)。"""
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))

    # 1. 骨骼纯净/风化面 (0,0)~(32,20)
    for x in range(32):
        for y in range(20):
            noise = ((x * 17 + y * 31) % 19)
            base = 210 + noise
            r = min(255, base + 10)
            g = min(255, base + 6)
            b = max(0, base - 16)
            # 浅灰骨垢与骨孔
            if (x * 7 + y * 13) % 29 == 0:
                r, g, b = r - 45, g - 40, b - 35
            img.putpixel((x, y), (r, g, b, 255))

    # 2. 额骨紫裂纹面 (0,20)~(32,36) (额头上的标志性紫电裂缝)
    for x in range(32):
        for y in range(20, 36):
            u = x
            v = y - 20
            noise = ((x * 19 + y * 23) % 17)
            base = 205 + noise
            r = min(255, base + 8)
            g = min(255, base + 4)
            b = max(0, base - 18)
            # 闪电状紫裂缝 (Lightning Fissure)
            is_crack = (u == 12 + int(math.sin(v * 0.8) * 4)) or (u == 13 and v % 3 == 0)
            if is_crack:
                r, g, b = 215, 60, 255  # 亮紫发光裂痕
            elif abs(u - (12 + int(math.sin(v * 0.8) * 4))) == 1:
                r, g, b = 120, 35, 160  # 裂痕暗紫边缘
            img.putpixel((x, y), (r, g, b, 255))

    # 3. 牙齿与眼眶深部 (0,36)~(32,48)
    for x in range(32):
        for y in range(36, 48):
            if y < 42:
                # 上齿列：米黄骨牙 + 牙缝黑槽
                if x % 4 == 0 or y == 41:
                    img.putpixel((x, y), (45, 38, 32, 255))
                else:
                    shade = 225 + ((x * 7 + y * 5) % 15)
                    img.putpixel((x, y), (shade, shade - 10, shade - 35, 255))
            else:
                # 眼眶深渊底：近乎黑色的深紫
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

    # 5. 藤蔓主干 (32,0)~(64,24) (深暗紫黑 0x3A1848，带有韧皮纵纹)
    for x in range(32, 64):
        for y in range(24):
            u = x - 32
            fiber = ((u * 5) % 7) * 4
            noise = ((u * 13 + y * 17) % 15)
            r = 52 + fiber + noise
            g = 24 + (fiber // 2) + (noise // 2)
            b = 78 + fiber + noise * 2
            # 细微灵气紫丝
            if (u * 2 + y * 3) % 23 == 0:
                r, g, b = 145, 45, 195
            img.putpixel((x, y), (min(255, r), min(255, g), min(255, b), 255))

    # 6. 倒刺与刺尖 (32,24)~(48,44)
    for x in range(32, 48):
        for y in range(24, 44):
            u = x - 32
            v = y - 24
            grad = (u + v) / 34.0
            # 暗紫过渡到艳粉紫 0xD038B8
            r = int(65 * (1 - grad) + 215 * grad)
            g = int(25 * (1 - grad) + 55 * grad)
            b = int(95 * (1 - grad) + 235 * grad)
            img.putpixel((x, y), (min(255, r), min(255, g), min(255, b), 255))

    # 7. 真元晶核与髓光 (48,24)~(64,44)
    for x in range(48, 64):
        for y in range(24, 44):
            dx = abs(x - 56)
            dy = abs(y - 34)
            d = math.sqrt(dx * dx + dy * dy)
            if d < 2.0:
                img.putpixel((x, y), (255, 240, 255, 255))  # 白紫炽光
            elif d < 4.5:
                img.putpixel((x, y), (210, 85, 255, 255))  # 亮荧光紫
            else:
                glow = max(0.0, 1.0 - d / 8.0)
                r = int(140 * glow + 35)
                g = int(30 * glow + 10)
                b = int(225 * glow + 55)
                img.putpixel((x, y), (min(255, r), min(255, g), min(255, b), 255))

    # 8. 阴影与过渡 (32,44)~(64,64)
    for x in range(32, 64):
        for y in range(44, 64):
            val = 25 + ((x + y * 2) % 18)
            img.putpixel((x, y), (val, val, val + 15, 255))

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
    print(f"✅ [Round 3 Final] 生成 BBModel: {bb_path} (共 {len(bb_dict['elements'])} 个 Cubes)")

    tex_path = PREVIEW_DIR / "tui_gu_teng_texture_r3.png"
    create_texture().save(tex_path)
    print(f"✅ [Round 3 Final] 导出贴图: {tex_path}")


if __name__ == "__main__":
    main()
