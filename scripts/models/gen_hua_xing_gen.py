#!/usr/bin/env python3
"""生成化形根 (HuaXingGen) Blockbench .bbmodel 与三视图预览 (万年老参灵胎·解剖级精修终极完美版)。

【世界观正典与设定依据】：
- 定位：丹道传说级灵草 (Legendary)，生长在死域深处 (spirit_qi: -1.0 ~ 0.0)。
- 物理机制：在极度负压死域中吞噬煞气孕育逆天生机 (survival_mode: neg_pressure_feed)，
  触碰时微动，为炼制「化形大丹」与重塑肉身的无上神药。
- 终极解剖精雕 (Refined Visual Anatomy)：
  1. 人形头颅与通灵神貌 (Meditative Cranium & Face)：
     - 象牙乳白莹润玉质 (0xF9F6ED)，饱满圆润的婴孩/老参颅相；
     - 恬淡闭目入定神态，清秀安宁，仅有一对清秀柔和的闭目眼睑线与秀气鼻梁、安详唇吻；
     - 颅顶簇生「三出青翠灵叶」(0x489662)、「顶生高光朱红血髓灵珠」(0xD82838) 与灵气青白发须。
  2. 灵胎盘坐身躯 (Meditative Root Body & Torso)：
     - 饱满微凸的灵胎腹部与自然微曲的入定身躯，胸腹脊柱木质经络节理紧密平滑咬合无缝隙；
     - 檀中穴与气海穴内蕴微弱青金灵髓温润光晕；
     - 浑然一体的通灵象牙玉白根质，各关节处呈自然微温润暖白过渡。
  3. 环抱双臂与指状根须 (Clasping Arms & Root Fingers)：
     - 双臂自肩部顺滑贴合胸侧，在小腹前优雅交叠合抱，末端分叉成纤细修长的指状须根。
  4. 盘坐分叉主根双腿与缠绕根系 (Crossed Taproot Legs & Root Strands)：
     - 健硕分叉的主根大腿呈入定盘坐之态，膝节微屈，足端化为多段主须根深入死土裂隙。
  5. 死域腐土与玄石基盘 (Deadzone Soil & Basalt Substrate)：
     - 细密乳白须根抓地，深扎于暗黑死域腐土 (0x242028) 与冷灰玄武岩 (0x484B52) 裂隙之中。
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
UUID_NAMESPACE = uuid.UUID("a3b4c5d6-e7f8-4901-b234-56789abcdef0")


@dataclass(frozen=True)
class Cube:
    bone: str
    name: str
    origin: tuple[float, float, float]
    target: tuple[float, float, float]
    front_uv: str
    side_uv: str = "root_plain_ivory"
    back_uv: str = "root_plain_ivory"
    top_uv: str = "root_plain_ivory"
    bottom_uv: str = "root_plain_ivory"
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    rot_origin: tuple[float, float, float] = (0.0, 0.0, 0.0)


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(UUID_NAMESPACE, ":".join(parts)))


# ─────────────────────────────────────────────────────────────────────────────
# 1. 结构拆解：人形头颅灵芽、微曲身躯胸腹、手足分叉与死域地基
# ─────────────────────────────────────────────────────────────────────────────

def part_head_and_crown() -> list[Cube]:
    """人形头颅与顶生灵芽 (Head & Crown Sprouts)：
    位于 Y: 8.2 ~ 13.8。
    具有清晰安详闭目的五官面容、圆润颅骨与顶生朱红灵珠三出灵叶。
    """
    cubes: list[Cube] = []
    HEAD_ROT = (4.0, 0.0, 0.0)
    HEAD_ORG = (8.0, 8.4, 7.8)

    # ── 1. 头部颅骨多盒有机拟合 ──
    # (1) 颅骨核心主体 (Cranium Core Y: 9.0 ~ 11.6, X: 6.5 ~ 9.5, Z: 6.5 ~ 9.1)
    cubes.append(
        Cube(
            "head",
            "head_cranium_main",
            (6.5, 9.0, 6.5),
            (9.5, 11.6, 9.1),
            front_uv="head_face_calm",
            side_uv="root_plain_ivory",
            back_uv="head_back_ivory",
            top_uv="root_plain_ivory",
            bottom_uv="root_plain_ivory",
            rotation=HEAD_ROT,
            rot_origin=HEAD_ORG,
        )
    )
    # (2) 前额与天灵盖微弧 (Forehead Top Crown Y: 11.2 ~ 12.0)
    cubes.append(
        Cube(
            "head",
            "head_forehead_crown",
            (6.8, 11.2, 6.7),
            (9.2, 12.0, 8.9),
            front_uv="head_forehead",
            side_uv="root_plain_ivory",
            back_uv="head_back_ivory",
            top_uv="root_plain_ivory",
            bottom_uv="root_plain_ivory",
            rotation=HEAD_ROT,
            rot_origin=HEAD_ORG,
        )
    )
    # (3) 鼻梁微隆 (Nose Ridge Protrusion Y: 9.6 ~ 10.4, Z: 6.1 ~ 6.5)
    cubes.append(
        Cube(
            "head",
            "head_nose_ridge",
            (7.6, 9.6, 6.1),
            (8.4, 10.4, 6.5),
            front_uv="head_nose",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=HEAD_ROT,
            rot_origin=HEAD_ORG,
        )
    )
    # (4) 下颌收颏与颈项连接 (Chin & Neck Y: 8.2 ~ 9.2)
    cubes.append(
        Cube(
            "head",
            "head_chin_neck",
            (6.8, 8.2, 6.8),
            (9.2, 9.2, 8.8),
            front_uv="root_joint_ivory",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=HEAD_ROT,
            rot_origin=HEAD_ORG,
        )
    )

    # ── 2. 顶生三出灵叶、朱红灵珠与灵气发须 ──
    # 顶生主茎 (Sprout Central Stem Y: 11.8 ~ 13.0)
    cubes.append(
        Cube(
            "crown",
            "crown_stem_main",
            (7.7, 11.8, 7.5),
            (8.3, 13.0, 8.1),
            front_uv="sprout_stem",
            side_uv="sprout_stem",
            back_uv="sprout_stem",
            rotation=(-4.0, 0.0, 0.0),
            rot_origin=(8.0, 11.8, 7.8),
        )
    )
    # 顶生朱红血髓灵珠 (Red Spirit Berry Y: 12.4 ~ 13.2, 象征万年灵力凝华)
    cubes.append(
        Cube(
            "crown",
            "crown_spirit_berry",
            (7.5, 12.4, 7.0),
            (8.5, 13.2, 7.8),
            front_uv="spirit_berry_red",
            side_uv="spirit_berry_red",
            back_uv="spirit_berry_red",
            top_uv="spirit_berry_red",
            bottom_uv="spirit_berry_red",
            rotation=(-4.0, 0.0, 0.0),
            rot_origin=(8.0, 12.4, 7.4),
        )
    )
    # 左侧灵叶 (Left Leaflet - 舒展微卷)
    cubes.append(
        Cube(
            "crown",
            "crown_leaf_left",
            (5.6, 12.6, 7.4),
            (7.6, 13.8, 8.2),
            front_uv="sprout_leaf",
            side_uv="sprout_leaf",
            back_uv="sprout_leaf",
            rotation=(-12.0, 24.0, -32.0),
            rot_origin=(7.7, 12.6, 7.8),
        )
    )
    # 右侧灵叶 (Right Leaflet)
    cubes.append(
        Cube(
            "crown",
            "crown_leaf_right",
            (8.4, 12.6, 7.4),
            (10.4, 13.8, 8.2),
            front_uv="sprout_leaf",
            side_uv="sprout_leaf",
            back_uv="sprout_leaf",
            rotation=(-12.0, -24.0, 32.0),
            rot_origin=(8.3, 12.6, 7.8),
        )
    )
    # 顶中灵叶 (Center Top Leaflet)
    cubes.append(
        Cube(
            "crown",
            "crown_leaf_center",
            (7.2, 13.0, 7.6),
            (8.8, 14.2, 8.4),
            front_uv="sprout_leaf",
            side_uv="sprout_leaf",
            back_uv="sprout_leaf",
            rotation=(-20.0, 0.0, 0.0),
            rot_origin=(8.0, 13.0, 8.0),
        )
    )
    # 后脑灵气青白发须 (Tendril Hair Topknot)
    cubes.append(
        Cube(
            "crown",
            "crown_tendril_hair",
            (7.6, 11.4, 8.6),
            (8.4, 13.4, 9.2),
            front_uv="sprout_stem",
            side_uv="sprout_stem",
            back_uv="sprout_stem",
            rotation=(24.0, 0.0, 0.0),
            rot_origin=(8.0, 11.4, 8.9),
        )
    )

    return cubes


def part_torso_and_spine() -> list[Cube]:
    """躯干与胸腹脊柱 (Root Torso & Belly)：
    位于 Y: 3.8 ~ 8.4，饱满微凸的灵胎入定腹部与微前倾体态。
    """
    cubes: list[Cube] = []
    TORSO_ROT = (4.0, 0.0, 0.0)
    TORSO_ORG = (8.0, 4.0, 7.8)

    # 1. 颈胸过渡与上胸腔 (Upper Chest Y: 7.2 ~ 8.4)
    cubes.append(
        Cube(
            "torso",
            "torso_chest_upper",
            (6.5, 7.2, 6.7),
            (9.5, 8.4, 8.9),
            front_uv="torso_chest",
            side_uv="root_plain_ivory",
            back_uv="torso_spine",
            rotation=TORSO_ROT,
            rot_origin=TORSO_ORG,
        )
    )
    # 2. 中胸腔 (Mid Chest Y: 5.8 ~ 7.2)
    cubes.append(
        Cube(
            "torso",
            "torso_chest_mid",
            (6.2, 5.8, 6.5),
            (9.8, 7.2, 9.1),
            front_uv="torso_chest",
            side_uv="root_plain_ivory",
            back_uv="torso_spine",
            rotation=TORSO_ROT,
            rot_origin=TORSO_ORG,
        )
    )
    # 3. 饱满微凸灵胎腹 (Belly Core Y: 4.2 ~ 5.8)
    cubes.append(
        Cube(
            "torso",
            "torso_belly_main",
            (6.0, 4.2, 6.4),
            (10.0, 5.8, 9.2),
            front_uv="torso_belly",
            side_uv="root_plain_ivory",
            back_uv="torso_spine",
            rotation=TORSO_ROT,
            rot_origin=TORSO_ORG,
        )
    )
    # 4. 腹部前凸圆拱 (Belly Dome Protrusion)
    cubes.append(
        Cube(
            "torso",
            "torso_belly_dome",
            (6.8, 4.6, 5.8),
            (9.2, 5.8, 6.4),
            front_uv="torso_belly",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=TORSO_ROT,
            rot_origin=TORSO_ORG,
        )
    )
    # 5. 后背脊骨木质节瘤 (Back Spine Ridge)
    cubes.append(
        Cube(
            "torso",
            "torso_spine_ridge",
            (7.5, 4.8, 9.1),
            (8.5, 8.0, 9.7),
            front_uv="root_joint_ivory",
            side_uv="root_joint_ivory",
            back_uv="torso_spine",
            rotation=TORSO_ROT,
            rot_origin=TORSO_ORG,
        )
    )
    # 6. 下腹与骨盆实心连接 (Lower Abdomen Core Y: 3.4 ~ 4.4)
    cubes.append(
        Cube(
            "torso",
            "torso_pelvis_bridge",
            (6.4, 3.4, 6.6),
            (9.6, 4.6, 9.0),
            front_uv="torso_belly",
            side_uv="root_plain_ivory",
            back_uv="torso_spine",
            rotation=TORSO_ROT,
            rot_origin=TORSO_ORG,
        )
    )

    return cubes


def part_arms_and_hands() -> list[Cube]:
    """双臂与手爪根须 (Arms & Root Hands)：
    双臂自肩部顺滑贴合胸侧，在小腹前方自然环抱，手指交错。
    """
    cubes: list[Cube] = []

    # ── 左臂 (Left Arm) ──
    # 左肩上臂 (紧贴胸侧，向前下方倾斜)
    cubes.append(
        Cube(
            "arms",
            "arm_left_upper",
            (5.1, 6.2, 6.7),
            (6.3, 8.0, 8.1),
            front_uv="arm_limb",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(16.0, -10.0, 14.0),
            rot_origin=(6.2, 7.8, 7.6),
        )
    )
    # 左前臂 (自然环抱小腹)
    cubes.append(
        Cube(
            "arms",
            "arm_left_forearm",
            (5.4, 4.8, 5.8),
            (6.6, 6.4, 7.0),
            front_uv="arm_limb",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(10.0, 24.0, -14.0),
            rot_origin=(5.8, 6.2, 6.7),
        )
    )
    # 左手掌主指根须 (交叠抱于小腹)
    cubes.append(
        Cube(
            "arms",
            "hand_left_palm_root",
            (6.3, 4.4, 5.4),
            (7.4, 5.6, 6.1),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(8.0, 15.0, -8.0),
            rot_origin=(6.4, 5.2, 5.7),
        )
    )
    # 左手指尖修长根须
    cubes.append(
        Cube(
            "arms",
            "hand_left_finger_tip",
            (6.5, 3.8, 5.3),
            (7.3, 4.8, 5.8),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(16.0, 10.0, -6.0),
            rot_origin=(6.6, 4.6, 5.5),
        )
    )

    # ── 右臂 (Right Arm) ──
    # 右肩上臂
    cubes.append(
        Cube(
            "arms",
            "arm_right_upper",
            (9.7, 6.2, 6.7),
            (10.9, 8.0, 8.1),
            front_uv="arm_limb",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(16.0, 10.0, -14.0),
            rot_origin=(9.8, 7.8, 7.6),
        )
    )
    # 右前臂
    cubes.append(
        Cube(
            "arms",
            "arm_right_forearm",
            (9.4, 4.8, 5.8),
            (10.6, 6.4, 7.0),
            front_uv="arm_limb",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(10.0, -24.0, 14.0),
            rot_origin=(10.2, 6.2, 6.7),
        )
    )
    # 右手掌主指根须
    cubes.append(
        Cube(
            "arms",
            "hand_right_palm_root",
            (8.6, 4.4, 5.4),
            (9.7, 5.6, 6.1),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(8.0, -15.0, 8.0),
            rot_origin=(9.6, 5.2, 5.7),
        )
    )
    # 右手指尖修长根须
    cubes.append(
        Cube(
            "arms",
            "hand_right_finger_tip",
            (8.7, 3.8, 5.3),
            (9.5, 4.8, 5.8),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(16.0, -10.0, 6.0),
            rot_origin=(9.4, 4.6, 5.5),
        )
    )

    return cubes


def part_legs_and_feet() -> list[Cube]:
    """双腿与分叉足根 (Legs & Root Feet)：
    位于 Y: 0.6 ~ 4.4，紧凑盘坐入定，双腿饱满外展，膝节微屈，底座密实无空洞。
    """
    cubes: list[Cube] = []

    # ── 骨盆裆部稳固中枢 (Pelvic Center Core Y: 2.4 ~ 4.4) ──
    cubes.append(
        Cube(
            "legs",
            "pelvis_center_block",
            (6.4, 2.4, 6.5),
            (9.6, 4.2, 8.9),
            front_uv="torso_belly",
            side_uv="root_plain_ivory",
            back_uv="torso_spine",
            rotation=(0.0, 0.0, 0.0),
            rot_origin=(8.0, 3.4, 7.7),
        )
    )

    # ── 左腿主根 (Left Leg Root) ──
    # 左大腿 (斜向左前外展)
    cubes.append(
        Cube(
            "legs",
            "leg_left_thigh",
            (5.0, 2.2, 6.5),
            (7.0, 4.2, 8.5),
            front_uv="leg_thigh",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(6.0, 18.0, -12.0),
            rot_origin=(6.6, 3.8, 7.8),
        )
    )
    # 左膝前曲节瘤 (Knee Joint)
    cubes.append(
        Cube(
            "legs",
            "leg_left_knee",
            (4.8, 1.4, 5.8),
            (6.6, 2.8, 7.4),
            front_uv="root_joint_ivory",
            side_uv="root_joint_ivory",
            back_uv="root_joint_ivory",
            rotation=(-4.0, 12.0, -8.0),
            rot_origin=(5.7, 2.4, 6.8),
        )
    )
    # 左足爪主根 (Foot Taproot - 向前下方稳固抓地)
    cubes.append(
        Cube(
            "legs",
            "leg_left_foot_root",
            (4.6, 0.5, 5.2),
            (6.2, 1.8, 6.8),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(10.0, 8.0, -4.0),
            rot_origin=(5.5, 1.5, 6.2),
        )
    )

    # ── 右腿主根 (Right Leg Root) ──
    # 右大腿 (斜向右前外展)
    cubes.append(
        Cube(
            "legs",
            "leg_right_thigh",
            (9.0, 2.2, 6.5),
            (11.0, 4.2, 8.5),
            front_uv="leg_thigh",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(6.0, -18.0, 12.0),
            rot_origin=(9.4, 3.8, 7.8),
        )
    )
    # 右膝前曲节瘤
    cubes.append(
        Cube(
            "legs",
            "leg_right_knee",
            (9.4, 1.4, 5.8),
            (11.2, 2.8, 7.4),
            front_uv="root_joint_ivory",
            side_uv="root_joint_ivory",
            back_uv="root_joint_ivory",
            rotation=(-4.0, -12.0, 8.0),
            rot_origin=(10.3, 2.4, 6.8),
        )
    )
    # 右足爪主根
    cubes.append(
        Cube(
            "legs",
            "leg_right_foot_root",
            (9.8, 0.5, 5.2),
            (11.4, 1.8, 6.8),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(10.0, -8.0, 4.0),
            rot_origin=(10.5, 1.5, 6.2),
        )
    )

    # ── 盘坐前根横桥 (Center Cross Root - 象牙玉质与大腿平滑融接) ──
    cubes.append(
        Cube(
            "legs",
            "cross_root_front",
            (6.5, 0.8, 5.8),
            (9.5, 2.2, 7.2),
            front_uv="leg_thigh",
            side_uv="root_plain_ivory",
            back_uv="root_plain_ivory",
            rotation=(4.0, 0.0, 0.0),
            rot_origin=(8.0, 1.5, 6.5),
        )
    )

    return cubes


def part_rootlets_and_substrate() -> list[Cube]:
    """地表须根网络与死域基盘 (Rootlets & Deadzone Soil)：
    - 细密乳白须根抓地扎入死土；
    - 死域腐殖黑土台与风化玄武碎岩。
    """
    cubes: list[Cube] = []

    # 1. 抓地须根群 (Fibrous Rootlets)
    # 前向中须根 (向前扎入石缝)
    cubes.append(
        Cube(
            "rootlets",
            "rootlet_front_mid",
            (7.4, 0.0, 4.6),
            (8.6, 0.8, 6.0),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(6.0, 0.0, 0.0),
            rot_origin=(8.0, 0.0, 5.8),
        )
    )
    # 左侧外展抓根
    cubes.append(
        Cube(
            "rootlets",
            "rootlet_left_flank",
            (3.6, 0.0, 6.0),
            (5.0, 0.8, 7.4),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(0.0, 30.0, -10.0),
            rot_origin=(4.6, 0.0, 6.8),
        )
    )
    # 右侧外展抓根
    cubes.append(
        Cube(
            "rootlets",
            "rootlet_right_flank",
            (11.0, 0.0, 6.0),
            (12.4, 0.8, 7.4),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(0.0, -30.0, 10.0),
            rot_origin=(11.4, 0.0, 6.8),
        )
    )
    # 后侧主支撑须根
    cubes.append(
        Cube(
            "rootlets",
            "rootlet_back_anchor",
            (7.4, 0.0, 8.4),
            (8.6, 0.9, 10.0),
            front_uv="finger_root",
            side_uv="finger_root",
            back_uv="finger_root",
            rotation=(-8.0, 0.0, 0.0),
            rot_origin=(8.0, 0.0, 8.6),
        )
    )

    # 2. 死域暗腐殖土与冷灰玄岩 (Deadzone Soil & Basalt Slabs)
    # 中心腐土底盘
    cubes.append(
        Cube(
            "substrate",
            "deadzone_soil_core",
            (4.8, 0.0, 4.4),
            (11.2, 0.6, 11.2),
            front_uv="deadzone_soil",
            side_uv="deadzone_soil",
            back_uv="deadzone_soil",
        )
    )
    # 左前方死域玄武石片
    cubes.append(
        Cube(
            "substrate",
            "basalt_shard_left",
            (3.2, 0.0, 3.8),
            (6.0, 0.8, 6.0),
            front_uv="deadzone_rock",
            side_uv="deadzone_rock",
            back_uv="deadzone_rock",
            rotation=(4.0, 25.0, -6.0),
            rot_origin=(4.6, 0.0, 4.9),
        )
    )
    # 右后方死域玄武石片
    cubes.append(
        Cube(
            "substrate",
            "basalt_shard_right",
            (10.0, 0.0, 8.0),
            (12.8, 1.0, 10.6),
            front_uv="deadzone_rock",
            side_uv="deadzone_rock",
            back_uv="deadzone_rock",
            rotation=(-6.0, -32.0, 8.0),
            rot_origin=(11.4, 0.0, 9.3),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_head_and_crown()
        + part_torso_and_spine()
        + part_arms_and_hands()
        + part_legs_and_feet()
        + part_rootlets_and_substrate()
    )


# ─────────────────────────────────────────────────────────────────────────────
# 2. 64x64 UV Atlas 高清贴图烘焙 (精雕无杂色)
# ─────────────────────────────────────────────────────────────────────────────

UV_MAP = {
    # ── 象牙乳白通灵根肉 ──
    "head_face_calm":     [2.0, 2.0, 16.0, 16.0],
    "head_forehead":      [16.0, 2.0, 26.0, 10.0],
    "head_nose":          [26.0, 2.0, 32.0, 8.0],
    "head_back_ivory":    [32.0, 2.0, 44.0, 14.0],
    "root_plain_ivory":   [44.0, 2.0, 62.0, 16.0],

    "torso_chest":        [2.0, 16.0, 16.0, 30.0],
    "torso_belly":        [16.0, 16.0, 30.0, 30.0],
    "torso_spine":        [30.0, 16.0, 42.0, 30.0],

    "arm_limb":           [2.0, 30.0, 14.0, 44.0],
    "leg_thigh":          [14.0, 30.0, 26.0, 44.0],
    "finger_root":        [26.0, 30.0, 36.0, 44.0],
    "root_joint_ivory":   [36.0, 30.0, 46.0, 44.0],

    # ── 顶生青翠灵叶、朱红灵珠与灵须 ──
    "sprout_leaf":        [44.0, 18.0, 54.0, 28.0],
    "sprout_stem":        [54.0, 18.0, 62.0, 28.0],
    "spirit_berry_red":   [46.0, 28.0, 56.0, 38.0],

    # ── 死域暗腐土与玄武碎岩 ──
    "deadzone_soil":      [2.0, 46.0, 24.0, 62.0],
    "deadzone_rock":      [24.0, 46.0, 46.0, 62.0],
}


def create_texture() -> Image.Image:
    """烘焙 64x64 UV Atlas 贴图：
    - 化形根体表：莹润乳白象牙质 (0xF8F5EB ~ 0xECE4D2)，纯净柔滑的木质纤维，无杂乱色斑。
    - 头面部：仅有一对清晰安详的闭目眼睑与清秀鼻唇印痕。
    - 顶芽：三出翠绿灵叶 (0x489662) 与顶生高光朱红血髓灵珠 (0xD82838)。
    - 死域地基：暗黑腐殖土 (0x242028) 与冷灰玄岩 (0x484B52)。
    """
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # 1. 象牙通灵玉白底色 (0,0)~(64,46)
    for x in range(64):
        for y in range(46):
            noise = ((x * 17 + y * 23) % 7) - 3
            base_r = 248
            base_g = 245
            base_b = 236

            # 纵向极其微弱的纤维拉丝
            if x % 4 == 0:
                base_r -= 4
                base_g -= 4
                base_b -= 6

            # 关节处的柔和温润微晕 (自然暖白 0xEFE8D8)
            if y in range(30, 46) and x in range(36, 46):
                base_r, base_g, base_b = 238, 232, 218

            r = int(np.clip(base_r + noise, 205, 255))
            g = int(np.clip(base_g + noise, 195, 255))
            b = int(np.clip(base_b + noise, 175, 250))

            img.putpixel((x, y), (r, g, b, 255))

    # 2. 精绘人形面容 (2,2)~(16,16) - 仅此唯一一对闭目清秀眼睑
    # 柔和闭目眼睑线 (y=7)
    draw.line([(5, 7), (7, 7)], fill=(162, 142, 118, 255), width=1)
    draw.line([(10, 7), (12, 7)], fill=(162, 142, 118, 255), width=1)
    # 鼻梁与鼻翼阴影 (y=9, 10)
    img.putpixel((8, 9), (255, 253, 246, 255))
    img.putpixel((9, 9), (255, 253, 246, 255))
    img.putpixel((8, 10), (205, 188, 162, 255))
    img.putpixel((9, 10), (205, 188, 162, 255))
    # 安详微合唇线 (y=13)
    draw.line([(7, 13), (10, 13)], fill=(190, 165, 142, 255), width=1)

    # 3. 前额区 (16,2)~(26,10) - 纯净光洁天灵盖
    for x in range(16, 26):
        for y in range(2, 10):
            noise = ((x * 11 + y * 19) % 5) - 2
            img.putpixel((x, y), (250 + noise, 246 + noise, 238 + noise, 255))

    # 4. 精绘胸腹经络节理 (2,16)~(30,30)
    # 胸骨微凹中线
    draw.line([(8, 18), (8, 28)], fill=(225, 214, 198, 255), width=1)
    # 檀中穴与气海穴温润灵髓微光 (柔和青金)
    img.putpixel((8, 20), (255, 248, 195, 255))
    img.putpixel((22, 23), (255, 248, 195, 255))
    img.putpixel((23, 23), (255, 248, 195, 255))

    # 5. 顶生三出灵叶与细茎 (44,18)~(64,28)
    for x in range(44, 64):
        for y in range(18, 28):
            noise = ((x * 13 + y * 23) % 11) - 5
            if x in range(44, 54):
                # 翠绿灵叶 0x489662 (72, 150, 98)
                r = int(np.clip(72 + noise * 2.0, 45, 115))
                g = int(np.clip(150 + noise * 2.5, 105, 205))
                b = int(np.clip(98 + noise * 2.0, 65, 145))
                if (x + y) % 3 == 0:
                    g += 24
            else:
                # 嫩芽青白细茎
                r = int(np.clip(95 + noise * 1.5, 65, 135))
                g = int(np.clip(135 + noise * 2.0, 95, 175))
                b = int(np.clip(85 + noise * 1.5, 55, 125))
            img.putpixel((x, y), (r, g, b, 255))

    # 6. 顶生朱红血髓灵珠 (46,28)~(56,38) - 鲜亮高光血红 0xD82838
    for x in range(46, 56):
        for y in range(28, 38):
            dx = abs(x - 51)
            dy = abs(y - 33)
            d = math.sqrt(dx * dx + dy * dy)
            if d < 1.5:
                # 灵珠高光核心
                img.putpixel((x, y), (255, 120, 135, 255))
            elif d < 4.0:
                # 鲜亮血红真元
                img.putpixel((x, y), (216, 40, 56, 255))
            else:
                # 灵珠外缘暗红
                img.putpixel((x, y), (145, 24, 38, 255))

    # 7. 死域腐殖黑土区 (2,46)~(24,62)
    for x in range(2, 24):
        for y in range(46, 62):
            noise = ((x * 17 + y * 31) % 19) - 9
            base = 38 + noise
            r = int(np.clip(base - 2, 18, 65))
            g = int(np.clip(base - 5, 16, 60))
            b = int(np.clip(base + 4, 22, 75))
            if (x * 3 + y * 7) % 11 == 0:
                r += 8
                g += 6
                b += 10
            img.putpixel((x, y), (r, g, b, 255))

    # 8. 死域玄武碎岩区 (24,46)~(46,62)
    for x in range(24, 46):
        for y in range(46, 62):
            noise = ((x * 29 + y * 19) % 17) - 8
            base = 74 + noise * 2
            r = int(np.clip(base - 4, 42, 115))
            g = int(np.clip(base - 1, 45, 118))
            b = int(np.clip(base + 6, 52, 128))
            if (x + y) % 4 == 0:
                r += 6
                g += 6
                b += 8
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
        top_box = UV_MAP.get(c.top_uv, side_box)
        bot_box = UV_MAP.get(c.bottom_uv, side_box)

        faces = {
            "north": {"uv": front_box, "texture": 0},
            "south": {"uv": back_box, "texture": 0},
            "east":  {"uv": side_box, "texture": 0},
            "west":  {"uv": side_box, "texture": 0},
            "up":    {"uv": top_box, "texture": 0},
            "down":  {"uv": bot_box, "texture": 0},
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
        "name": "HuaXingGen",
        "model_identifier": "hua_xing_gen",
        "visible_box": [1, 1, 0],
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "hua_xing_gen.png",
                "name": "hua_xing_gen",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": stable_uuid("texture", "hua_xing_gen"),
                "source": tex_b64,
            }
        ],
    }
    return model_dict


def main():
    LOCAL_MODELS.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)

    bbmodel = build_bbmodel()
    model_path = LOCAL_MODELS / "HuaXingGen.bbmodel"
    model_path.write_text(json.dumps(bbmodel, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✅ Generated BBModel: {model_path.relative_to(REPO)}")

    tex = create_texture()
    tex_path = PREVIEW_DIR / "hua_xing_gen_texture_r4.png"
    tex.save(tex_path)
    print(f"✅ Saved Texture: {tex_path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
