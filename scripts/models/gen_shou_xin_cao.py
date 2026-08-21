#!/usr/bin/env python3
"""生成兽心草 (ShouXinCao) Blockbench .bbmodel 与三视图预览 (Round 3 终极打磨·精雕无瑕完美版)。

【世界观正典与设定依据】：
- 定位：丹道珍稀灵草，生长在异变兽巢穴与废墟荒冢（RuinDensity > 0.2, spirit_qi > 0.5）。
- 物理机制：吸食异变兽残存气血与煞气而生（survival_mode: qi_absorb），为炼制「兽心丹」与异变进阶药物的必备药引。
- 参考资产：
  * 像素原画：client/src/main/resources/assets/bong/textures/item/shou_xin_cao.png
  * 源码设定：server/src/dandao/herbs.rs (tint_rgb: 0x8B2020, display_name: "兽心草")

【视觉解剖特征与部件拆解】：
1. 心形肉质主叶片 (Heart-shaped Main Leaf)：
   - 墨绿皮革肉质感 (0x1B3524)，后仰 20° 舒展；
   - 顶部圆润双生心耳 (Left/Right Lobes) 夹着心凹 (Heart Notch)；
   - 中部饱满舒展，下部顺滑收拢汇聚于心尖 (Heart Apex)；
   - 心尖向前下方自然微垂，呈现滴血灵动之态。
2. 血红血管搏动脉络系统 (Blood Vein System)：
   - 3D 浮雕粗壮中央主动脉与心室搏动核心节点 (Pulsing Heart Node)；
   - 64x64 UV Atlas 贴图深度烘焙毛细血管分叉网，向两侧心耳与叶缘辐射扩散。
3. 支撑主茎与顶生双生幼芽 (Stem & Twin Leaflets)：
   - 紫红血质主茎紧密贴合主叶后背，从地面一气呵成直达心凹分叉点；
   - 顶部对称分叉出修长纤细的 V 字形细叶柄（±28°），分别托起一对生机盎然的微型小心形幼芽与鲜红幼脉。
4. 兽巢废墟基盘 (Beast Bones & Ruin Substrate)：
   - 风化兽肋骨、椎骨残块与玄武瓦砾石片错落有致，紫红气血须根盘绕抓地扎入深土。
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
UUID_NAMESPACE = uuid.UUID("e2b1f804-9a3d-4c8e-b5f7-3d84a1e94c20")


@dataclass(frozen=True)
class Cube:
    bone: str
    name: str
    origin: tuple[float, float, float]
    target: tuple[float, float, float]
    front_uv: str
    side_uv: str = "leaf_plain_dark"
    back_uv: str = "leaf_plain_dark"
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    rot_origin: tuple[float, float, float] = (0.0, 0.0, 0.0)


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(UUID_NAMESPACE, ":".join(parts)))


# ─────────────────────────────────────────────────────────────────────────────
# 1. 结构拆解：心形主叶、血脉网络、双生幼芽、叶茎与碎骨基盘
# ─────────────────────────────────────────────────────────────────────────────

def part_main_leaf() -> list[Cube]:
    """心形肉质主叶片 (Heart-shaped Main Leaf)：
    叶盘统一后仰倾斜 20° (围绕基准轴 X=8.0, Y=0.0, Z=7.0 旋转)。
    前表面朝向 North (面向正面相机)，各层紧密平滑咬合。
    """
    cubes: list[Cube] = []
    LEAF_ROT = (20.0, 0.0, 0.0)
    LEAF_ORG = (8.0, 0.0, 7.0)

    # ── 1. 顶层心耳区 (Y: 9.0 ~ 11.6) ──
    # 左心耳顶部圆弧
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_lobe_left_top",
            (4.6, 10.2, 6.7),
            (7.4, 11.6, 7.3),
            front_uv="leaf_lobe_left",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 左心耳内侧连接
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_lobe_left_mid",
            (3.8, 9.0, 6.7),
            (7.6, 10.2, 7.3),
            front_uv="leaf_lobe_left",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # 右心耳顶部圆弧
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_lobe_right_top",
            (8.6, 10.2, 6.7),
            (11.4, 11.6, 7.3),
            front_uv="leaf_lobe_right",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 右心耳内侧连接
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_lobe_right_mid",
            (8.4, 9.0, 6.7),
            (12.2, 10.2, 7.3),
            front_uv="leaf_lobe_right",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # 心凹陷中心处 (Heart Notch)
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_notch_center",
            (7.4, 8.8, 6.7),
            (8.6, 9.6, 7.3),
            front_uv="leaf_center",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # ── 2. 最宽叶身中上段 (Y: 7.2 ~ 9.0, 宽度 X: 3.2 ~ 12.8) ──
    # 左翼中上段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_wing_left_upper",
            (3.2, 7.2, 6.7),
            (6.8, 9.0, 7.3),
            front_uv="leaf_wing_left",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 中轴主室中上段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_center_upper",
            (6.8, 7.2, 6.7),
            (9.2, 9.0, 7.3),
            front_uv="leaf_center",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 右翼中上段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_wing_right_upper",
            (9.2, 7.2, 6.7),
            (12.8, 9.0, 7.3),
            front_uv="leaf_wing_right",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # ── 3. 叶身中段平滑过渡 (Y: 5.4 ~ 7.2, 宽度 X: 3.8 ~ 12.2) ──
    # 左翼中段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_wing_left_mid",
            (3.8, 5.4, 6.7),
            (6.8, 7.2, 7.3),
            front_uv="leaf_wing_left",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 中轴主室中段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_center_mid",
            (6.8, 5.4, 6.7),
            (9.2, 7.2, 7.3),
            front_uv="leaf_center",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 右翼中段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_wing_right_mid",
            (9.2, 5.4, 6.7),
            (12.2, 7.2, 7.3),
            front_uv="leaf_wing_right",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # ── 4. 叶身下段收束 (Y: 3.6 ~ 5.4, 宽度 X: 4.8 ~ 11.2) ──
    # 左下收拢段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_taper_left_mid",
            (4.8, 3.6, 6.7),
            (6.8, 5.4, 7.3),
            front_uv="leaf_taper",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 中轴主室下段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_center_lower",
            (6.8, 3.6, 6.7),
            (9.2, 5.4, 7.3),
            front_uv="leaf_center",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 右下收拢段
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_taper_right_mid",
            (9.2, 3.6, 6.7),
            (11.2, 5.4, 7.3),
            front_uv="leaf_taper",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # ── 5. 心尖近端收束 (Y: 2.0 ~ 3.6, 宽度 X: 5.8 ~ 10.2) ──
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_taper_near_tip",
            (5.8, 2.0, 6.7),
            (10.2, 3.6, 7.3),
            front_uv="leaf_taper",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # ── 6. 心尖末端尖喙 (Y: 0.8 ~ 2.0, 宽度 X: 7.0 ~ 9.0) ──
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_apex_base",
            (7.0, 0.8, 6.7),
            (9.0, 2.0, 7.3),
            front_uv="leaf_apex",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # ── 7. 心尖前下方微垂滴血嘴 ──
    cubes.append(
        Cube(
            "main_leaf",
            "leaf_apex_droop_lip",
            (7.4, 0.1, 7.4),
            (8.6, 1.1, 8.0),
            front_uv="leaf_apex",
            rotation=(-12.0, 0.0, 0.0),
            rot_origin=(8.0, 1.0, 7.6),
        )
    )

    return cubes


def part_blood_veins() -> list[Cube]:
    """血红血管搏动脉络系统 (Blood Vein System)：
    - 3D 浮雕粗壮中央主动脉、心室搏动核心节点 (Pulsing Heart Node)。
    - 紧贴主叶正面 (Z: 6.25 ~ 6.75)，凸出于叶肉之上。
    """
    cubes: list[Cube] = []
    LEAF_ROT = (20.0, 0.0, 0.0)
    LEAF_ORG = (8.0, 0.0, 7.0)

    # 1. 纵贯中央的深红主动脉 (Main Aorta)
    # 下段主动脉
    cubes.append(
        Cube(
            "veins",
            "vein_aorta_lower",
            (7.75, 2.0, 6.25),
            (8.25, 5.4, 6.75),
            front_uv="vein_aorta_red",
            side_uv="vein_side_plain",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 中段心室搏动核心 (膨大节点，高光真元)
    cubes.append(
        Cube(
            "veins",
            "vein_heart_pulse_node",
            (7.45, 5.4, 6.15),
            (8.55, 8.8, 6.75),
            front_uv="vein_pulse_core",
            side_uv="vein_side_plain",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 上段主动脉 (通向心凹分叉)
    cubes.append(
        Cube(
            "veins",
            "vein_aorta_upper_fork",
            (7.75, 8.8, 6.25),
            (8.25, 9.8, 6.75),
            front_uv="vein_aorta_red",
            side_uv="vein_side_plain",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 叶尖滴血珠 (Blood Drop Tip)
    cubes.append(
        Cube(
            "veins",
            "vein_apex_blood_drop",
            (7.65, 0.2, 7.25),
            (8.35, 1.2, 7.85),
            front_uv="vein_pulse_core",
            side_uv="vein_side_plain",
            rotation=(-12.0, 0.0, 0.0),
            rot_origin=(8.0, 1.0, 7.6),
        )
    )

    return cubes


def part_stem_and_twin_leaflets() -> list[Cube]:
    """主茎秆与顶生双生幼芽 (Stem & Twin Leaflets)：
    - 连续木质血茎紧密贴合主叶后背 (Z: 7.3 ~ 8.4)，从地面一气呵成直达心凹分叉点。
    - 顶部对称分叉出修长纤细的 V 字形细叶柄（±26°），紧密托起一对微型小心形幼芽与鲜红幼脉。
    """
    cubes: list[Cube] = []
    LEAF_ROT = (20.0, 0.0, 0.0)
    LEAF_ORG = (8.0, 0.0, 7.0)

    # 1. 底部血质主茎 (Main Stem)
    # 茎基段 (破土直达叶背)
    cubes.append(
        Cube(
            "stem",
            "stem_base_ground",
            (7.4, 0.0, 6.6),
            (8.6, 2.5, 7.8),
            front_uv="stem_bark",
            side_uv="stem_bark",
            back_uv="stem_bark",
            rotation=(0.0, 0.0, 0.0),
            rot_origin=(8.0, 0.0, 7.2),
        )
    )
    # 茎下段支撑 (贴合叶背 Y: 2.0 ~ 5.5, Z: 7.3 ~ 8.3)
    cubes.append(
        Cube(
            "stem",
            "stem_support_lower",
            (7.4, 2.0, 7.3),
            (8.6, 5.5, 8.3),
            front_uv="stem_bark",
            side_uv="stem_bark",
            back_uv="stem_bark",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )
    # 茎中上段支撑与分叉中枢 (贴合叶背 Y: 5.5 ~ 9.0, Z: 7.3 ~ 8.3)
    cubes.append(
        Cube(
            "stem",
            "stem_support_upper_hub",
            (7.4, 5.5, 7.3),
            (8.6, 9.0, 8.3),
            front_uv="stem_bark",
            side_uv="stem_bark",
            back_uv="stem_bark",
            rotation=LEAF_ROT,
            rot_origin=LEAF_ORG,
        )
    )

    # 2. V 字形双生细叶柄 (Twin Petioles)
    # 主茎分叉中心世界坐标 P_HUB = (8.0, 8.1836, 10.8299)
    P_HUB = (8.0, 8.1836, 10.8299)
    cubes.append(
        Cube(
            "twin_leaflets",
            "petiole_left",
            (7.75, 8.1836, 10.5799),
            (8.25, 10.3836, 11.0799),
            front_uv="stem_petiole",
            side_uv="stem_petiole",
            back_uv="stem_petiole",
            rotation=(20.0, -16.0, 26.0),
            rot_origin=P_HUB,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "petiole_right",
            (7.75, 8.1836, 10.5799),
            (8.25, 10.3836, 11.0799),
            front_uv="stem_petiole",
            side_uv="stem_petiole",
            back_uv="stem_petiole",
            rotation=(20.0, 16.0, -26.0),
            rot_origin=P_HUB,
        )
    )

    # 3. 顶生小心形幼芽 (Twin Leaflets)
    # 左叶柄梢端 P_TIP_L = (6.907, 9.948, 11.55)
    P_TIP_L = (6.907, 9.948, 11.55)
    ROT_L = (20.0, -16.0, 26.0)
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_left_blade",
            (5.907, 9.948, 11.25),
            (7.907, 11.548, 11.75),
            front_uv="leaflet_blade",
            side_uv="leaflet_plain",
            back_uv="leaflet_plain",
            rotation=ROT_L,
            rot_origin=P_TIP_L,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_left_lobe_l",
            (5.207, 10.448, 11.25),
            (6.407, 11.848, 11.70),
            front_uv="leaflet_lobe",
            side_uv="leaflet_plain",
            back_uv="leaflet_plain",
            rotation=ROT_L,
            rot_origin=P_TIP_L,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_left_lobe_r",
            (7.407, 10.548, 11.25),
            (8.607, 11.848, 11.70),
            front_uv="leaflet_lobe",
            side_uv="leaflet_plain",
            back_uv="leaflet_plain",
            rotation=ROT_L,
            rot_origin=P_TIP_L,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_left_vein",
            (6.707, 10.048, 11.10),
            (7.107, 11.548, 11.35),
            front_uv="leaflet_vein",
            side_uv="vein_side_plain",
            back_uv="leaflet_plain",
            rotation=ROT_L,
            rot_origin=P_TIP_L,
        )
    )

    # 右叶柄梢端 P_TIP_R = (9.093, 9.948, 11.55)
    P_TIP_R = (9.093, 9.948, 11.55)
    ROT_R = (20.0, 16.0, -26.0)
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_right_blade",
            (8.093, 9.948, 11.25),
            (10.093, 11.548, 11.75),
            front_uv="leaflet_blade",
            side_uv="leaflet_plain",
            back_uv="leaflet_plain",
            rotation=ROT_R,
            rot_origin=P_TIP_R,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_right_lobe_r",
            (9.593, 10.448, 11.25),
            (10.793, 11.848, 11.70),
            front_uv="leaflet_lobe",
            side_uv="leaflet_plain",
            back_uv="leaflet_plain",
            rotation=ROT_R,
            rot_origin=P_TIP_R,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_right_lobe_l",
            (7.393, 10.548, 11.25),
            (8.593, 11.848, 11.70),
            front_uv="leaflet_lobe",
            side_uv="leaflet_plain",
            back_uv="leaflet_plain",
            rotation=ROT_R,
            rot_origin=P_TIP_R,
        )
    )
    cubes.append(
        Cube(
            "twin_leaflets",
            "sprout_right_vein",
            (8.893, 10.048, 11.10),
            (9.293, 11.548, 11.35),
            front_uv="leaflet_vein",
            side_uv="vein_side_plain",
            back_uv="leaflet_plain",
            rotation=ROT_R,
            rot_origin=P_TIP_R,
        )
    )

    return cubes


def part_root_and_substrate() -> list[Cube]:
    """根系与地表碎石 (Root Pad & Substrate)：
    - 风化小兽骨片（兽肋骨、椎骨残块）、荒冢瓦砾石片。
    - 紫红色须根扎入土中。
    """
    cubes: list[Cube] = []

    # 1. 须根网络 (Fibrous Roots)
    # 前主抓根
    cubes.append(
        Cube(
            "root_pad",
            "root_front",
            (7.6, 0.0, 5.8),
            (8.4, 0.8, 7.0),
            front_uv="stem_root",
            side_uv="stem_root",
            back_uv="stem_root",
            rotation=(12.0, 0.0, 0.0),
            rot_origin=(8.0, 0.0, 6.8),
        )
    )
    # 左侧抓根
    cubes.append(
        Cube(
            "root_pad",
            "root_left",
            (6.2, 0.0, 6.4),
            (7.5, 0.8, 7.6),
            front_uv="stem_root",
            side_uv="stem_root",
            back_uv="stem_root",
            rotation=(0.0, 24.0, -10.0),
            rot_origin=(7.5, 0.0, 7.0),
        )
    )
    # 右侧抓根
    cubes.append(
        Cube(
            "root_pad",
            "root_right",
            (8.5, 0.0, 6.4),
            (9.8, 0.8, 7.6),
            front_uv="stem_root",
            side_uv="stem_root",
            back_uv="stem_root",
            rotation=(0.0, -24.0, 10.0),
            rot_origin=(8.5, 0.0, 7.0),
        )
    )
    # 后侧支撑根
    cubes.append(
        Cube(
            "root_pad",
            "root_back",
            (7.5, 0.0, 7.6),
            (8.5, 0.9, 9.0),
            front_uv="stem_root",
            side_uv="stem_root",
            back_uv="stem_root",
            rotation=(-10.0, 0.0, 0.0),
            rot_origin=(8.0, 0.0, 7.6),
        )
    )

    # 2. 地表风化兽骨碎片 (Weathered Beast Bones)
    # 兽骨 1: 左侧兽肋骨碎片 (弯曲斜插)
    cubes.append(
        Cube(
            "substrate",
            "bone_rib_left",
            (4.2, 0.0, 4.8),
            (6.6, 0.9, 6.4),
            front_uv="bone_weathered",
            side_uv="bone_weathered",
            back_uv="bone_weathered",
            rotation=(6.0, 30.0, -8.0),
            rot_origin=(5.4, 0.0, 5.6),
        )
    )
    # 兽骨 2: 右后侧兽脊椎骨残块
    cubes.append(
        Cube(
            "substrate",
            "bone_vertebra_right",
            (9.5, 0.0, 7.6),
            (12.0, 1.2, 9.8),
            front_uv="bone_weathered",
            side_uv="bone_weathered",
            back_uv="bone_weathered",
            rotation=(-8.0, -36.0, 12.0),
            rot_origin=(10.7, 0.0, 8.7),
        )
    )
    # 兽骨 3: 前方尖骨刺
    cubes.append(
        Cube(
            "substrate",
            "bone_splinter_front",
            (8.8, 0.0, 4.2),
            (10.2, 0.7, 5.6),
            front_uv="bone_weathered",
            side_uv="bone_weathered",
            back_uv="bone_weathered",
            rotation=(0.0, -18.0, 6.0),
            rot_origin=(9.5, 0.0, 4.9),
        )
    )

    # 3. 荒冢废墟暗灰瓦砾石片 (Ruin Rubble)
    # 前向玄武瓦砾石板
    cubes.append(
        Cube(
            "substrate",
            "ruin_stone_front",
            (6.0, 0.0, 3.8),
            (8.8, 0.5, 5.6),
            front_uv="ruin_stone",
            side_uv="ruin_stone",
            back_uv="ruin_stone",
            rotation=(0.0, 10.0, 0.0),
            rot_origin=(7.4, 0.0, 4.7),
        )
    )
    # 后向玄武瓦砾石板
    cubes.append(
        Cube(
            "substrate",
            "ruin_stone_back",
            (7.6, 0.0, 8.4),
            (10.6, 0.6, 10.4),
            front_uv="ruin_stone",
            side_uv="ruin_stone",
            back_uv="ruin_stone",
            rotation=(0.0, -15.0, 0.0),
            rot_origin=(9.1, 0.0, 9.4),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_main_leaf()
        + part_blood_veins()
        + part_stem_and_twin_leaflets()
        + part_root_and_substrate()
    )


# ─────────────────────────────────────────────────────────────────────────────
# 2. 64x64 UV Atlas 高清贴图烘焙
# ─────────────────────────────────────────────────────────────────────────────

UV_MAP = {
    # ── 主叶表面（墨绿皮革 + 手绘血管网络） ──
    "leaf_center":      [2.0, 2.0, 14.0, 18.0],
    "leaf_wing_left":   [2.0, 18.0, 14.0, 32.0],
    "leaf_lobe_left":   [14.0, 18.0, 24.0, 28.0],
    "leaf_wing_right":  [24.0, 18.0, 36.0, 32.0],
    "leaf_lobe_right":  [24.0, 6.0, 34.0, 16.0],
    "leaf_taper":       [2.0, 32.0, 16.0, 44.0],
    "leaf_apex":        [34.0, 6.0, 44.0, 16.0],
    "leaf_plain_dark":  [16.0, 2.0, 24.0, 10.0],

    # ── 3D 浮雕主动脉与搏动核心 ──
    "vein_aorta_red":   [44.0, 2.0, 52.0, 20.0],
    "vein_pulse_core":  [52.0, 2.0, 62.0, 18.0],
    "vein_branch":      [44.0, 20.0, 56.0, 30.0],
    "vein_side_plain":  [56.0, 20.0, 62.0, 28.0],

    # ── 双生幼芽 ──
    "leaflet_blade":    [32.0, 36.0, 44.0, 48.0],
    "leaflet_lobe":     [44.0, 36.0, 54.0, 46.0],
    "leaflet_vein":     [54.0, 36.0, 62.0, 46.0],
    "leaflet_plain":    [36.0, 24.0, 44.0, 32.0],

    # ── 叶茎与细叶柄 ──
    "stem_bark":        [2.0, 46.0, 14.0, 62.0],
    "stem_petiole":     [14.0, 46.0, 22.0, 62.0],
    "stem_root":        [22.0, 46.0, 30.0, 62.0],

    # ── 风化兽骨与荒冢瓦砾 ──
    "bone_weathered":   [30.0, 48.0, 46.0, 62.0],
    "ruin_stone":       [46.0, 48.0, 62.0, 62.0],
}


def create_texture() -> Image.Image:
    """烘焙 64x64 UV Atlas 贴图：
    - 主叶正面：深墨绿皮革肉质 (0x1B3524) + 精美手绘血管树分叉 (鲜亮深红与暗紫红血晕)。
    - 主叶侧面/背面：纯净深墨绿皮革 (0x172E20)。
    - 主动脉：高饱和真元血红 (0xD82438) + 心室发光光晕。
    - 木质茎：紫红血筋木质感 (0x5E1C38)。
    - 兽骨：风化象牙灰白 (0xD4CEB8) + 骨裂斑痕。
    - 瓦砾：玄武暗灰 (0x4A4E52)。
    """
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # 1. 绘制墨绿皮革叶肉底色 (0,0)~(44,46)
    for x in range(44):
        for y in range(46):
            noise = ((x * 17 + y * 31) % 19) - 9
            r = int(np.clip(26 + noise * 1.0, 16, 48))
            g = int(np.clip(56 + noise * 1.8, 34, 82))
            b = int(np.clip(38 + noise * 1.2, 22, 60))
            if (x * 7 + y * 13) % 6 == 0:
                r += 4
                g += 6
                b += 5
            img.putpixel((x, y), (r, g, b, 255))

    # 纯净深墨绿侧背面专用区 (16,2)~(24,10)
    for x in range(16, 24):
        for y in range(2, 10):
            noise = ((x * 13 + y * 23) % 11) - 5
            r = int(np.clip(22 + noise, 14, 35))
            g = int(np.clip(46 + noise * 1.5, 30, 65))
            b = int(np.clip(32 + noise, 20, 48))
            img.putpixel((x, y), (r, g, b, 255))

    # 2. 在主叶各正面 UV 区域精准绘制有机血管分支网络 (Vascular Network)
    # 中心区 (2,2)~(14,18)：纵向主动脉血槽与左右微晕
    for y in range(2, 18):
        for x in range(2, 14):
            dist_to_center = abs(x - 8)
            if dist_to_center <= 1:
                img.putpixel((x, y), (175, 26, 42, 255))
            elif dist_to_center == 2:
                img.putpixel((x, y), (105, 32, 38, 255))

    # 左翼区 (2,18)~(14,32)：向左下方与左上方辐射的毛细侧脉
    draw.line([(12, 25), (4, 21)], fill=(160, 24, 38, 255), width=1)
    draw.line([(12, 25), (5, 28)], fill=(145, 20, 34, 255), width=1)
    draw.line([(8, 23), (4, 25)], fill=(130, 18, 30, 255), width=1)

    # 左心耳区 (14,18)~(24,28)：弧形向心耳顶部分叉的侧脉
    draw.line([(15, 26), (20, 20)], fill=(155, 22, 36, 255), width=1)
    draw.line([(18, 23), (22, 24)], fill=(135, 18, 32, 255), width=1)

    # 右翼区 (24,18)~(36,32)：向右下方与右上方辐射的毛细侧脉
    draw.line([(26, 25), (34, 21)], fill=(160, 24, 38, 255), width=1)
    draw.line([(26, 25), (33, 28)], fill=(145, 20, 34, 255), width=1)
    draw.line([(30, 23), (34, 25)], fill=(130, 18, 30, 255), width=1)

    # 右心耳区 (24,6)~(34,16)
    draw.line([(25, 14), (31, 8)], fill=(155, 22, 36, 255), width=1)
    draw.line([(28, 11), (32, 12)], fill=(135, 18, 32, 255), width=1)

    # 收尖区 (2,32)~(16,44)
    draw.line([(14, 34), (8, 42)], fill=(140, 20, 32, 255), width=1)
    draw.line([(2, 34), (8, 42)], fill=(140, 20, 32, 255), width=1)

    # 3. 3D 浮雕主动脉与搏动核心 (44,0)~(64,30)
    for x in range(44, 64):
        for y in range(30):
            u = x - 44
            noise = ((x * 23 + y * 29) % 13) - 6
            if 0 <= u < 8 and 2 <= y < 20:
                # 主动脉 (鲜亮血红 0xAC1A2A) [44, 2, 52, 20]
                r = int(np.clip(176 + noise * 3.0, 135, 225))
                g = int(np.clip(24 + noise * 1.0, 12, 48))
                b = int(np.clip(38 + noise * 1.2, 18, 60))
            elif 8 <= u < 18 and 2 <= y < 18:
                # 搏动心室高光 (高饱和真元光泽 0xD82438) [52, 2, 62, 18]
                r = int(np.clip(228 + noise * 3.5, 190, 255))
                g = int(np.clip(36 + noise * 1.5, 20, 72))
                b = int(np.clip(52 + noise * 1.8, 24, 92))
            elif 0 <= u < 12 and 20 <= y < 30:
                # 侧脉分叉 [44, 20, 56, 30]
                r = int(np.clip(152 + noise * 2.5, 110, 195))
                g = int(np.clip(20 + noise * 0.9, 10, 42))
                b = int(np.clip(32 + noise * 1.1, 14, 54))
            elif 12 <= u < 18 and 20 <= y < 28:
                # 血管侧边平原暗红 [56, 20, 62, 28]
                r, g, b = 120, 16, 26
            else:
                r, g, b = 135, 18, 30
            img.putpixel((x, y), (r, g, b, 255))

    # 4. 双生幼芽区 (32,36)~(64,48)
    for x in range(32, 64):
        for y in range(36, 48):
            noise = ((x * 13 + y * 19) % 15) - 7
            if x >= 54:
                # 幼芽鲜红脉
                r = int(np.clip(192 + noise * 3.0, 150, 245))
                g = int(np.clip(26 + noise * 1.0, 12, 52))
                b = int(np.clip(40 + noise * 1.2, 18, 65))
            else:
                # 嫩绿带血晕
                r = int(np.clip(46 + noise * 2.0, 24, 90))
                g = int(np.clip(78 + noise * 2.2, 42, 115))
                b = int(np.clip(44 + noise * 1.5, 22, 70))
                if (x + y) % 4 == 0:
                    r += 24
                    g -= 8
            img.putpixel((x, y), (r, g, b, 255))

    # 幼芽纯暗绿背侧区 (36,24)~(44,32)
    for x in range(36, 44):
        for y in range(24, 32):
            img.putpixel((x, y), (35, 62, 38, 255))

    # 5. 紫红血茎、细叶柄与气血须根 (0,46)~(30,64)
    # 0x5E1C38 (94, 28, 56)
    for x in range(30):
        for y in range(46, 64):
            noise = ((x * 19 + y * 17) % 17) - 8
            is_fiber = (x % 3 == 0)
            base_r = 98 if not is_fiber else 118
            base_g = 28 if not is_fiber else 34
            base_b = 54 if not is_fiber else 68
            if x >= 22:
                # 须根更偏深暗紫
                base_r -= 15
                base_b += 8
            r = int(np.clip(base_r + noise * 2.0, 55, 145))
            g = int(np.clip(base_g + noise * 1.0, 12, 55))
            b = int(np.clip(base_b + noise * 1.4, 30, 90))
            img.putpixel((x, y), (r, g, b, 255))

    # 6. 风化兽骨碎片 (30,48)~(46,64)
    # 象牙风化兽骨 0xD4CEB8 (212, 206, 184)
    for x in range(30, 46):
        for y in range(48, 64):
            noise = ((x * 23 + y * 13) % 19) - 9
            base = 208 + noise * 2
            r = int(np.clip(base + 8, 150, 242))
            g = int(np.clip(base + 2, 145, 235))
            b = int(np.clip(base - 14, 125, 218))
            # 风化裂纹与暗斑
            if (x * 7 + y * 11) % 19 == 0:
                r -= 60
                g -= 55
                b -= 50
            img.putpixel((x, y), (r, g, b, 255))

    # 7. 荒冢废墟暗灰玄武瓦砾 (46,48)~(64,64)
    # 暗灰玄武岩 0x4A4E52
    for x in range(46, 64):
        for y in range(48, 64):
            noise = ((x * 31 + y * 19) % 17) - 8
            base = 74 + noise * 2
            r = int(np.clip(base - 2, 44, 112))
            g = int(np.clip(base + 2, 46, 118))
            b = int(np.clip(base + 6, 50, 126))
            if (x + y) % 4 == 0:
                r += 5
                g += 5
                b += 7
            img.putpixel((x, y), (r, g, b, 255))
    if img.size != (TEXTURE_RES, TEXTURE_RES):
        img = img.resize((TEXTURE_RES, TEXTURE_RES), resample=Image.Resampling.NEAREST)

    return img


# ─────────────────────────────────────────────────────────────────────────────
# 3. BBModel 文件生成
# ─────────────────────────────────────────────────────────────────────────────

def build_bbmodel() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    outlines = {}

    for idx, c in enumerate(cubes):
        elem_uuid = stable_uuid("element", str(idx), c.name)

        front_box = UV_MAP.get(c.front_uv, [0, 0, 16, 16])
        side_box = UV_MAP.get(c.side_uv, front_box)
        back_box = UV_MAP.get(c.back_uv, front_box)

        faces = {
            "north": {"uv": front_box, "texture": 0},
            "south": {"uv": back_box, "texture": 0},
            "east":  {"uv": side_box, "texture": 0},
            "west":  {"uv": side_box, "texture": 0},
            "up":    {"uv": side_box, "texture": 0},
            "down":  {"uv": side_box, "texture": 0},
        }

        elem = {
            "name": c.name,
            "box_uv": False,
            "type": "cube",
            "uuid": elem_uuid,
            "from": [round(float(x), 4) for x in c.origin],
            "to": [round(float(x), 4) for x in c.target],
            "faces": faces,
        }

        if any(abs(r) > 1e-4 for r in c.rotation):
            elem["rotation"] = [round(float(r), 4) for r in c.rotation]
            elem["origin"] = [round(float(o), 4) for o in c.rot_origin]

        elements.append(elem)

        if c.bone not in outlines:
            outlines[c.bone] = []
        outlines[c.bone].append(elem_uuid)

    outliner = []
    for bone_name, children_uuids in outlines.items():
        outliner.append(
            {
                "name": bone_name,
                "origin": [8.0, 0.0, 8.0],
                "color": 0,
                "uuid": stable_uuid("group", bone_name),
                "export": True,
                "isOpen": True,
                "locked": False,
                "visibility": True,
                "autouv": 0,
                "children": children_uuids,
            }
        )

    model_dict = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "ShouXinCao",
        "model_identifier": "shou_xin_cao",
        "visible_box": [1, 1, 0],
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "shou_xin_cao.png",
                "name": "shou_xin_cao",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": stable_uuid("texture", "shou_xin_cao"),
                "source": tex_b64,
            }
        ],
    }
    return model_dict


def main():
    LOCAL_MODELS.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)

    bbmodel = build_bbmodel()
    model_path = LOCAL_MODELS / "ShouXinCao.bbmodel"
    model_path.write_text(json.dumps(bbmodel, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✅ Generated BBModel: {model_path.relative_to(REPO)}")

    tex = create_texture()
    tex_path = PREVIEW_DIR / "shou_xin_cao_texture_r3.png"
    tex.save(tex_path)
    print(f"✅ Saved Texture: {tex_path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
