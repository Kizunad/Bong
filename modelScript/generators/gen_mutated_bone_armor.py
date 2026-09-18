#!/usr/bin/env python3
"""生成异兽刺骨甲（mutated_bone_armor）四件 bbmodel、64x64 UV 贴图与真实三视图预览。

配方对应异变兽骨（bone_chip_mat / mutated_bone_shard）+ 熟皮革带（tanned_hide_strap / dark_leather）+ 粗布内衬（rough_cloth）。
设计原则（对齐 worldview §四「截脉/震爆流」与 §十「异兽骨骼载体」）：
- 胸甲 (Chestplate)：
  - 内层与布衬：腹部粗麻缠布裹腹、右胸斜披战布内衬、右肩护肩软衬与腰布折边。
  - 骨骼装甲：紧凑弧形兽肋护胸、正中龙骨形胸骨板（sternum keel/ridge）、后背单列脊柱骨节。
  - 单肩装饰：左肩（+x 侧）固定小型异兽头颅肩甲（带吻部、犬齿獠牙与深凹眼眶，以及多道加固皮带）。
  - 骨架固定：双肩皮带、侧腰系绳、后背 X-Harness、双层粗麻绳腰带、骨扣与垂落绳结。
  - 双臂护腕：小臂粗麻缠布内衬 + 外侧纵向刺骨护板 + 肘尖刺突 + 三道紧致皮绳绑带。
- 护腿 (Leggings)：
  - 小腿正面：纵向弧形异兽胫骨护胫（shinbone greaves）+ 纵向骨脊 + 膝下防撞骨节 + 三道交叉绑腿皮绳。
  - 大腿/侧腰：大腿双层皮质固定环带 + 外侧加固骨扣。
- 头盔 (Helmet)：
  - 贴头分段颅盖、眉骨中脊、双侧太阳穴/护耳骨片与后脑脊骨，不做宽平顶硬壳。
- 靴子 (Boots)：
  - 前伸趾甲、收窄后跟、分层脚背与开口胫筒，配皮绳固定和外侧骨刺，明确读出脚的前后。

运行时真相是 client 的 ArmorPartModel.CUBE_TABLES，本文件的 --emit-java
可直接输出该表的 Java 字面量。
"""

from __future__ import annotations

import argparse
import copy
import random
from dataclasses import replace
from pathlib import Path

from PIL import Image, ImageDraw

import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.gates import gatekit
from bbmodel_maker.model.armor_model_common import (
    ArmorPart,
    Cube,
    MOUNT_X,
    TEXTURE_SIZE,
    write_material_assets,
)
from bbmodel_maker.rig.rigkit import Rig

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
PREVIEW_ROOT = Path(__file__).resolve().parents[1] / "out"

MATERIAL = "mutated_bone"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 贴图四象限规划 (64x64)：
# Q1 (0,0)-(32,32): 主兽骨质（象牙白、骨板自然粗糙裂纹）
# Q2 (32,0)-(64,32): 粗麻布裹布与战布内衬（中性灰褐/茶褐粗织纹理、磨损边）
# Q3 (0,32)-(32,64): 熟皮带与麻绳绑扎（深棕黑硬鞣皮、双股拧紧麻绳、青铜铆钉点）
# Q4 (32,32)-(64,64): 兽颅眼眶孔槽、獠牙、风化深骨与氧化残血暗斑
UV_BONE_MAIN = (0, 0)
UV_CLOTH_LINING = (32, 0)
UV_LEATHER_STRAP = (0, 32)
UV_SKULL_ACCENT = (32, 32)


def c(mount: str, name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv: tuple[int, int] = UV_BONE_MAIN) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 胸甲 (CHESTPLATE) ──────────────────────────────────────────────────────────
# 躯干局部坐标 x∈[-4,4], y∈[12,24], z∈[-2,2]，骨骼枢轴在 y=24。
# 双臂局部坐标 (在 BODY mount 上): 左臂 x∈[4,8], 右臂 x∈[-8,-4], y∈[12,24], z∈[-2,2]。

def _chest_cloth_lining() -> tuple[Cube, ...]:
    """粗麻布衬底与腹胸缠布（增加衣物层次与真实穿戴感）。"""
    return (
        # 1. 腹部粗麻缠布护腹 (Abdomen wrap, y: 12.1 -> 16.5)
        c("BODY", "cloth_wrap_abdomen_front", (-4.06, 12.12, -2.18), (8.12, 4.35, 0.35), UV_CLOTH_LINING),
        c("BODY", "cloth_wrap_abdomen_back", (-4.06, 12.12, 1.83), (8.12, 4.35, 0.35), UV_CLOTH_LINING),
        c("BODY", "cloth_wrap_abdomen_left", (3.83, 12.18, -2.15), (0.35, 4.25, 4.3), UV_CLOTH_LINING),
        c("BODY", "cloth_wrap_abdomen_right", (-4.18, 12.18, -2.15), (0.35, 4.25, 4.3), UV_CLOTH_LINING),

        # 2. 右胸至左腰斜披战布衬 (Diagonal chest sash / wrap)
        c("BODY", "cloth_sash_chest_front", (-3.65, 16.2, -2.35), (3.4, 6.75, 0.38), UV_CLOTH_LINING),
        c("BODY", "cloth_sash_chest_back", (-3.65, 16.2, 1.97), (3.4, 6.75, 0.38), UV_CLOTH_LINING),

        # 3. 右肩粗布护肩软衬 (Right shoulder padded cloth cap)
        c("BODY", "cloth_shoulder_cap_r", (-8.15, 22.8, -2.25), (4.3, 1.35, 4.5), UV_CLOTH_LINING),
        # 左肩兽头下方的垫肩厚布 (Left shoulder skull base padding)
        c("BODY", "cloth_shoulder_pad_l", (3.92, 22.6, -2.32), (4.35, 1.45, 4.64), UV_CLOTH_LINING),

        # 4. 垂落腰布折边 (Waist cloth loin flap)
        c("BODY", "cloth_loin_flap_front", (-2.6, 9.5, -2.48), (3.6, 2.7, 0.38), UV_CLOTH_LINING),
        c("BODY", "cloth_loin_flap_back", (-2.4, 9.8, 2.1), (3.4, 2.4, 0.38), UV_CLOTH_LINING),
    )


def _chest_torso_plates() -> tuple[Cube, ...]:
    """核心胸骨、肋骨板与骨节。"""
    return (
        # 1. 核心胸骨中脊 (Sternum ridge)
        c("BODY", "chest_sternum_core", (-1.1, 15.6, -2.88), (2.2, 7.2, 0.92), UV_BONE_MAIN),
        c("BODY", "chest_sternum_keel", (-0.6, 16.4, -3.28), (1.2, 5.8, 0.44), UV_BONE_MAIN),
        c("BODY", "chest_sternum_boss", (-1.3, 19.5, -3.18), (2.6, 1.6, 0.52), UV_SKULL_ACCENT),

        # 2. 弧形肋骨板 (Rib plates - 左右对称各 2 道)
        # 上肋 (贴合胸肌上沿)
        c("BODY", "chest_rib_top_l", (1.0, 19.8, -2.82), (2.75, 1.45, 0.74), UV_BONE_MAIN),
        c("BODY", "chest_rib_top_r", (-3.75, 19.8, -2.82), (2.75, 1.45, 0.74), UV_BONE_MAIN),
        # 下肋 (微倾，包裹肋弓)
        c("BODY", "chest_rib_mid_l", (0.9, 16.8, -2.78), (2.85, 1.4, 0.71), UV_BONE_MAIN),
        c("BODY", "chest_rib_mid_r", (-3.75, 16.8, -2.78), (2.85, 1.4, 0.71), UV_BONE_MAIN),

        # 3. 锁骨骨板 (Collarbone bars)
        c("BODY", "chest_collar_l", (0.8, 22.4, -2.74), (3.1, 0.95, 0.82), UV_SKULL_ACCENT),
        c("BODY", "chest_collar_r", (-3.9, 22.4, -2.74), (3.1, 0.95, 0.82), UV_SKULL_ACCENT),

        # 4. 后背脊柱骨节 (Spine segments - 纵向突起骨脊)
        c("BODY", "spine_ridge_upper", (-0.85, 18.2, 2.05), (1.7, 4.8, 0.78), UV_BONE_MAIN),
        c("BODY", "spine_ridge_lower", (-0.75, 13.5, 2.05), (1.5, 4.4, 0.72), UV_SKULL_ACCENT),
        c("BODY", "spine_knob_top", (-0.95, 20.8, 2.65), (1.9, 1.2, 0.45), UV_SKULL_ACCENT),
        c("BODY", "spine_knob_mid", (-0.85, 16.5, 2.65), (1.7, 1.1, 0.42), UV_SKULL_ACCENT),

        # 5. 肋骨与胸骨加固绑绳 (Bone lashings - 扎紧骨板的细麻绳)
        c("BODY", "rib_lashing_top_l", (1.8, 19.55, -2.96), (0.45, 1.95, 0.35), UV_LEATHER_STRAP),
        c("BODY", "rib_lashing_top_r", (-2.25, 19.55, -2.96), (0.45, 1.95, 0.35), UV_LEATHER_STRAP),
        c("BODY", "rib_lashing_mid_l", (1.6, 16.55, -2.92), (0.45, 1.9, 0.35), UV_LEATHER_STRAP),
        c("BODY", "rib_lashing_mid_r", (-2.05, 16.55, -2.92), (0.45, 1.9, 0.35), UV_LEATHER_STRAP),
        c("BODY", "sternum_lashing_upper", (-0.75, 21.1, -3.38), (1.5, 0.45, 0.35), UV_LEATHER_STRAP),
        c("BODY", "sternum_lashing_lower", (-0.75, 17.6, -3.38), (1.5, 0.45, 0.35), UV_LEATHER_STRAP),
    )


def _chest_harness_and_ropes() -> tuple[Cube, ...]:
    """皮革背带、交叉系绳与多圈粗麻绳腰带。"""
    return (
        # 1. 前胸斜向主皮带 (连向双肩与肋下)
        c("BODY", "harness_strap_fl", (2.2, 14.5, -2.55), (0.75, 7.8, 0.32), UV_LEATHER_STRAP),
        c("BODY", "harness_strap_fr", (-2.95, 14.5, -2.55), (0.75, 7.8, 0.32), UV_LEATHER_STRAP),

        # 2. 侧腰交叉系带 (Side flank lacing)
        c("BODY", "harness_flank_strap_l1", (3.94, 15.6, -2.25), (0.32, 0.65, 4.5), UV_LEATHER_STRAP),
        c("BODY", "harness_flank_strap_l2", (3.94, 13.8, -2.25), (0.32, 0.65, 4.5), UV_LEATHER_STRAP),
        c("BODY", "harness_flank_strap_r1", (-4.26, 15.6, -2.25), (0.32, 0.65, 4.5), UV_LEATHER_STRAP),
        c("BODY", "harness_flank_strap_r2", (-4.26, 13.8, -2.25), (0.32, 0.65, 4.5), UV_LEATHER_STRAP),

        # 3. 后背交叉背带 (X-Harness) 与脊椎加固绳圈
        c("BODY", "harness_back_strap_a", (-3.6, 17.2, 2.02), (7.2, 0.95, 0.28), UV_LEATHER_STRAP),
        c("BODY", "harness_back_strap_b", (-3.6, 14.8, 2.02), (7.2, 0.95, 0.28), UV_LEATHER_STRAP),
        c("BODY", "spine_tie_upper", (-0.98, 19.4, 2.68), (1.96, 0.45, 0.32), UV_LEATHER_STRAP),
        c("BODY", "spine_tie_lower", (-0.88, 15.2, 2.68), (1.76, 0.45, 0.32), UV_LEATHER_STRAP),

        # 4. 腰部双层麻绳束腰 (Double-coil waist rope)
        c("BODY", "waist_rope_coil_upper", (-4.22, 13.25, -2.62), (8.44, 0.62, 5.24), UV_LEATHER_STRAP),
        c("BODY", "waist_rope_coil_lower", (-4.22, 12.25, -2.62), (8.44, 0.62, 5.24), UV_LEATHER_STRAP),

        # 5. 腰前骨质带扣与垂落绳结 (Bone buckle & hanging cords)
        c("BODY", "waist_buckle_bone", (-1.2, 12.05, -2.95), (2.4, 1.8, 0.48), UV_BONE_MAIN),
        c("BODY", "waist_rope_knot_main", (2.1, 11.4, -2.85), (1.3, 1.4, 0.65), UV_LEATHER_STRAP),
        c("BODY", "waist_rope_dangle_a", (2.2, 8.4, -2.76), (0.45, 3.2, 0.45), UV_LEATHER_STRAP),
        c("BODY", "waist_rope_dangle_b", (2.8, 9.1, -2.76), (0.45, 2.4, 0.45), UV_LEATHER_STRAP),
    )


def _chest_skull_pauldron() -> tuple[Cube, ...]:
    """左肩（+x 侧）异兽头颅残骸肩甲装饰（配多圈固定皮带与绳结）。"""
    # 左肩骨骼基准 x∈[4,8], y=24 附近。
    return (
        # 1. 颅骨底座与固定皮带
        c("BODY", "skull_base_mount", (4.1, 22.8, -2.4), (4.3, 1.3, 4.8), UV_LEATHER_STRAP),
        c("BODY", "skull_tie_cord_f", (4.4, 21.6, -2.65), (3.6, 1.3, 0.4), UV_LEATHER_STRAP),
        c("BODY", "skull_tie_cord_b", (4.4, 21.6, 2.25), (3.6, 1.3, 0.4), UV_LEATHER_STRAP),
        c("BODY", "skull_strap_across", (6.1, 27.6, -2.38), (0.75, 0.45, 4.76), UV_LEATHER_STRAP),

        # 2. 脑颅穹顶 (Cranium dome)
        c("BODY", "skull_cranium_main", (4.4, 24.2, -2.3), (4.2, 3.4, 4.6), UV_BONE_MAIN),
        c("BODY", "skull_sagittal_crest", (5.8, 27.4, -1.8), (1.4, 1.2, 3.6), UV_SKULL_ACCENT),

        # 3. 吻部与前额 (Snout / Brow)
        c("BODY", "skull_snout_upper", (7.8, 24.4, -2.1), (1.8, 2.2, 4.2), UV_BONE_MAIN),
        c("BODY", "skull_brow_ridge", (7.4, 26.2, -2.25), (1.6, 1.1, 4.5), UV_SKULL_ACCENT),

        # 4. 眼眶孔洞与深色凹槽 (Orbit sockets)
        c("BODY", "skull_orbit_front", (6.2, 24.8, -2.55), (1.8, 1.8, 0.38), UV_SKULL_ACCENT),
        c("BODY", "skull_orbit_back", (6.2, 24.8, 2.18), (1.8, 1.8, 0.38), UV_SKULL_ACCENT),

        # 5. 上颌裂齿/犬齿獠牙 (Predator Fangs)
        c("BODY", "skull_fang_front", (8.65, 23.1, -1.9), (0.85, 1.5, 0.85), UV_SKULL_ACCENT),
        c("BODY", "skull_fang_rear", (8.65, 23.1, 1.05), (0.85, 1.5, 0.85), UV_SKULL_ACCENT),
        c("BODY", "skull_side_spur", (4.6, 26.8, 2.05), (0.9, 1.8, 0.9), UV_BONE_MAIN),
    )


def _chest_armguards() -> tuple[Cube, ...]:
    """双臂小臂内衬布条 + 刺骨护臂 + 十字绑扎皮绳。"""
    cubes = []
    for side in ("l", "r"):
        def x(base: float, span: float) -> float:
            return base if side == "l" else -(base + span)

        cubes.extend((
            # 1. 小臂内层紧密粗麻裹布 (Inner cloth arm wrap, y: 12.2 -> 17.6)
            c("BODY", f"arm_cloth_wrap_{side}", (x(3.95, 4.1), 12.2, -2.12), (4.1, 5.4, 4.24), UV_CLOTH_LINING),

            # 2. 小臂外侧主护骨板 (y: 12.4 -> 17.2)
            c("BODY", f"armguard_plate_outer_{side}", (x(7.9, 0.65), 12.4, -1.8), (0.65, 4.8, 3.6), UV_BONE_MAIN),
            # 骨板中央骨脊与小刺突
            c("BODY", f"armguard_plate_spine_{side}", (x(8.4, 0.38), 13.2, -1.2), (0.38, 3.4, 2.3), UV_SKULL_ACCENT),
            c("BODY", f"armguard_spur_elbow_{side}", (x(8.25, 0.55), 16.5, 1.15), (0.55, 1.2, 0.8), UV_SKULL_ACCENT),

            # 3. 小臂紧致熟皮护衬与十字绑带 (上中下三道绑扎圈)
            c("BODY", f"armguard_wrap_top_{side}", (x(3.96, 4.14), 16.1, -2.22), (4.14, 0.65, 4.44), UV_LEATHER_STRAP),
            c("BODY", f"armguard_wrap_mid_{side}", (x(3.96, 4.14), 14.3, -2.22), (4.14, 0.65, 4.44), UV_LEATHER_STRAP),
            c("BODY", f"armguard_wrap_low_{side}", (x(3.96, 4.14), 12.5, -2.22), (4.14, 0.65, 4.44), UV_LEATHER_STRAP),

            # 4. 绑扎外侧皮绳结扣与垂带
            c("BODY", f"armguard_knot_{side}", (x(8.25, 0.45), 14.3, -2.25), (0.45, 0.75, 0.75), UV_LEATHER_STRAP),
        ))
    return tuple(cubes)


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "mutated_bone_chestplate",
        "MUTATED BONE CHESTPLATE",
        _chest_cloth_lining()
        + _chest_torso_plates()
        + _chest_harness_and_ropes()
        + _chest_skull_pauldron()
        + _chest_armguards(),
    )


# ─── 头盔 (HELMET) ────────────────────────────────────────────────────────────
# 头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，脸朝 -z。颅盖只在头顶分段贴合，
# 侧护耳收在 x=±4.75、y≈24.35；前缘不越过 -z=5，避免做成宽平顶面具。


def _helmet_crown() -> tuple[Cube, ...]:
    """贴头分段颅盖：左右骨板、前后搭接与一条窄中脊。"""
    return (
        c("HEAD", "crown_left", (-4.55, 29.55, -2.90), (1.35, 2.55, 5.50), UV_BONE_MAIN),
        c("HEAD", "crown_right", (3.20, 29.55, -2.90), (1.35, 2.55, 5.50), UV_BONE_MAIN),
        c("HEAD", "crown_center_front", (-1.75, 30.25, -3.65), (3.50, 1.65, 1.45), UV_BONE_MAIN),
        c("HEAD", "crown_center_back", (-1.55, 30.15, -2.00), (3.10, 1.50, 4.55), UV_BONE_MAIN),
        c("HEAD", "crown_ridge", (-0.55, 31.55, -2.25), (1.10, 0.65, 3.50), UV_SKULL_ACCENT),
    )


def _helmet_brow() -> tuple[Cube, ...]:
    """眉骨与额中骨：分成左右承重段，中间用窄骨脊连接。"""
    return (
        c("HEAD", "brow_left", (-4.25, 28.10, -4.65), (3.50, 1.15, 0.80), UV_BONE_MAIN),
        c("HEAD", "brow_right", (0.75, 28.10, -4.65), (3.50, 1.15, 0.80), UV_BONE_MAIN),
        c("HEAD", "brow_keel", (-0.55, 28.00, -4.82), (1.10, 2.00, 0.65), UV_SKULL_ACCENT),
        # 前额带要吃进眉骨一点：只贴边会在 SIDE 视角读成悬空薄片。
        c("HEAD", "brow_lash_front", (-4.70, 27.95, -4.72), (9.40, 0.30, 0.30), UV_LEATHER_STRAP),
    )


def _helmet_side_plates() -> tuple[Cube, ...]:
    """太阳穴到耳下的连续侧骨片；不在头的前后四角另立柱。"""
    return (
        c("HEAD", "temple_left", (-4.60, 26.55, -3.65), (0.80, 3.10, 3.20), UV_BONE_MAIN),
        c("HEAD", "temple_right", (3.80, 26.55, -3.65), (0.80, 3.10, 3.20), UV_BONE_MAIN),
        c("HEAD", "ear_guard_left", (-4.75, 24.35, -3.30), (0.75, 4.00, 2.60), UV_SKULL_ACCENT),
        c("HEAD", "ear_guard_right", (4.00, 24.35, -3.30), (0.75, 4.00, 2.60), UV_SKULL_ACCENT),
        c("HEAD", "jaw_tip_left", (-4.80, 24.45, -3.55), (0.55, 1.00, 1.10), UV_BONE_MAIN),
        c("HEAD", "jaw_tip_right", (4.25, 24.45, -3.55), (0.55, 1.00, 1.10), UV_BONE_MAIN),
    )


def _helmet_rear_and_fasteners() -> tuple[Cube, ...]:
    """后脑骨脊与皮绳固定件，给颅盖明确的前后收束。"""
    return (
        c("HEAD", "rear_rail", (-3.65, 28.75, 3.75), (7.30, 1.00, 0.55), UV_BONE_MAIN),
        c("HEAD", "rear_spine", (-0.70, 27.55, 3.60), (1.40, 1.90, 0.55), UV_SKULL_ACCENT),
        # 后脑带与后轨保持少量体积交叠，避免侧视出现一条断开的细片。
        c("HEAD", "rear_lash", (-4.10, 28.65, 4.12), (8.20, 0.30, 0.25), UV_LEATHER_STRAP),
        # 绑带左右各吃入护耳 0.03 格；仍是细固定件，但不悬在侧面。
        c("HEAD", "ear_binding_left", (-5.00, 25.70, -2.95), (0.28, 0.40, 1.85), UV_LEATHER_STRAP),
        c("HEAD", "ear_binding_right", (4.72, 25.70, -2.95), (0.28, 0.40, 1.85), UV_LEATHER_STRAP),
        c("HEAD", "ear_spur_left", (-5.10, 26.15, -1.90), (0.40, 0.80, 0.80), UV_SKULL_ACCENT),
        c("HEAD", "ear_spur_right", (4.70, 26.15, -1.90), (0.40, 0.80, 0.80), UV_SKULL_ACCENT),
    )


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "mutated_bone_helmet",
        "MUTATED BONE HELMET",
        _helmet_crown() + _helmet_brow() + _helmet_side_plates() + _helmet_rear_and_fasteners(),
    )


# ─── 靴子 (BOOTS) ────────────────────────────────────────────────────────────
# 脚 mount 的局部 x/z 与 y=0 脚底约定沿用既有 bone armor；前方为 -z。


def _boot_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """一只异兽骨靴：趾甲前伸、后跟收窄，胫筒四片围合但保留顶部开口。"""
    side = "left" if sign > 0 else "right"
    outward_clearance = 0.45

    def x(inner: float, width: float) -> float:
        return inner + outward_clearance if sign > 0 else -inner - width - outward_clearance

    def c2(
        name: str,
        origin: tuple[float, float, float],
        size: tuple[float, float, float],
        uv: tuple[int, int] = UV_BONE_MAIN,
    ) -> Cube:
        return c(mount, f"{name}_{side}", origin, size, uv)

    return (
        # 分段鞋底：前掌更长、后跟更短，先建立脚的方向性。
        c2("sole_toe", (x(-1.95, 3.90), -0.48, -4.02), (3.90, 0.55, 2.10), UV_SKULL_ACCENT),
        c2("sole_mid", (x(-1.85, 3.70), -0.43, -1.82), (3.70, 0.50, 2.00), UV_BONE_MAIN),
        c2("sole_heel", (x(-1.65, 3.30), -0.36, 0.25), (3.30, 0.55, 2.30), UV_SKULL_ACCENT),
        c2("sole_front_rim", (x(-2.05, 4.10), 0.02, -4.18), (4.10, 0.38, 0.36), UV_LEATHER_STRAP),
        c2("sole_back_rim", (x(-1.70, 3.40), 0.03, 2.52), (3.40, 0.36, 0.32), UV_LEATHER_STRAP),

        # 趾甲与脚背骨板：前端只在 -z 伸出，不做前后对称木箱。
        c2("toe_claw_center", (x(-0.55, 1.10), 0.08, -4.48), (1.10, 1.00, 1.20), UV_BONE_MAIN),
        c2("toe_claw_outer", (x(0.78, 0.62), 0.12, -4.15), (0.62, 0.88, 0.95), UV_SKULL_ACCENT),
        c2("vamp_front", (x(-1.70, 3.40), 0.10, -3.34), (3.40, 1.00, 1.82), UV_BONE_MAIN),
        c2("vamp_back", (x(-1.55, 3.10), 0.24, -1.54), (3.10, 1.18, 1.48), UV_CLOTH_LINING),
        c2("vamp_ridge", (x(-1.02, 2.04), 1.03, -3.72), (2.04, 0.42, 0.48), UV_SKULL_ACCENT),
        c2("heel_plate", (x(-1.45, 2.90), 0.20, 1.76), (2.90, 1.72, 0.68), UV_BONE_MAIN),

        # 胫筒前后和两侧骨片围住脚踝，顶部保持开口并露出内衬。
        c2("shaft_front", (x(-1.65, 3.30), 1.30, -1.94), (3.30, 3.00, 0.56), UV_BONE_MAIN),
        c2("shaft_back", (x(-1.60, 3.20), 1.30, 1.48), (3.20, 3.00, 0.52), UV_BONE_MAIN),
        c2("shaft_outer", (x(1.72, 0.42), 1.30, -1.62), (0.42, 3.00, 3.20), UV_SKULL_ACCENT),
        c2("shaft_inner", (x(-1.66, 0.34), 1.34, -1.58), (0.34, 3.00, 3.12), UV_CLOTH_LINING),
        c2("shaft_top_front", (x(-1.72, 3.44), 4.45, -2.00), (3.44, 0.38, 0.34), UV_LEATHER_STRAP),
        c2("shaft_top_back", (x(-1.66, 3.32), 4.45, 1.54), (3.32, 0.38, 0.32), UV_LEATHER_STRAP),

        # 踝部皮绳与外侧骨刺让固定逻辑延续胸甲/护腿，而非只剩一块骨盒。
        c2("ankle_lash_front", (x(-1.78, 3.56), 1.48, -2.12), (3.56, 0.34, 0.28), UV_LEATHER_STRAP),
        c2("ankle_lash_back", (x(-1.72, 3.44), 1.52, 1.72), (3.44, 0.34, 0.26), UV_LEATHER_STRAP),
        c2("ankle_lash_outer", (x(1.80, 0.28), 1.50, -1.82), (0.28, 0.34, 3.60), UV_LEATHER_STRAP),
        c2("ankle_spur", (x(2.05, 0.68), 2.25, -1.35), (0.68, 0.90, 0.88), UV_SKULL_ACCENT),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "mutated_bone_boots",
        "MUTATED BONE BOOTS",
        _boot_cubes("LEFT_FOOT", 1.0) + _boot_cubes("RIGHT_FOOT", -1.0),
    )


# ─── 护腿 (LEGGINGS) ──────────────────────────────────────────────────────────
# 腿盒局部坐标 x∈[-2,2], y∈[0,12], z∈[-2,2]，骨骼枢轴在 y=12。
# 左右腿独立分侧 (LEFT_LEG: x_offset=+1.9, RIGHT_LEG: x_offset=-1.9)。

def _leggings_single_leg(mount: str) -> tuple[Cube, ...]:
    prefix = mount.lower()
    is_left = "left" in prefix
    dy = 0.05 if is_left else -0.05
    dz = 0.03 if is_left else -0.03

    def ox(base: float, span: float) -> float:
        """外侧偏移辅助。"""
        return base if is_left else -(base + span)

    return (
        # 1. 胫骨前侧弧形主骨板 (Shinbone greave - 覆盖小腿正面 y: 2.0 -> 8.5)
        c(mount, f"{prefix}_shin_plate_main", (-1.4, 2.2 + dy, -2.82 + dz), (2.8, 6.2, 0.85), UV_BONE_MAIN),
        # 胫骨纵向骨脊 (突出强化)
        c(mount, f"{prefix}_shin_plate_ridge", (-0.6, 2.8 + dy, -3.15 + dz), (1.2, 5.2, 0.42), UV_BONE_MAIN),
        # 膝盖下端防撞骨节
        c(mount, f"{prefix}_shin_knee_boss", (-1.2, 7.8 + dy, -3.1 + dz), (2.4, 1.4, 0.45), UV_SKULL_ACCENT),

        # 2. 小腿外侧副骨片 (Lateral bone splint)
        c(mount, f"{prefix}_shin_splint_side", (ox(1.25, 0.65), 3.0 + dy, -1.6 + dz), (0.65, 4.8, 3.2), UV_SKULL_ACCENT),

        # 3. 小腿皮绳交叉绑扎 (上中下三道绑腿带)
        c(mount, f"{prefix}_shin_strap_top", (-2.12, 7.6 + dy, -2.12 + dz), (4.24, 0.65, 4.24), UV_LEATHER_STRAP),
        c(mount, f"{prefix}_shin_strap_mid", (-2.12, 5.2 + dy, -2.12 + dz), (4.24, 0.65, 4.24), UV_LEATHER_STRAP),
        c(mount, f"{prefix}_shin_strap_low", (-2.12, 2.6 + dy, -2.12 + dz), (4.24, 0.65, 4.24), UV_LEATHER_STRAP),

        # 4. 大腿镂空处的极简固定皮带环 (Thigh strap)
        c(mount, f"{prefix}_thigh_strap_upper", (-2.15, 10.4 + dy, -2.15 + dz), (4.3, 0.8, 4.3), UV_LEATHER_STRAP),
        c(mount, f"{prefix}_thigh_strap_lower", (-2.12, 8.8 + dy, -2.12 + dz), (4.24, 0.65, 4.24), UV_LEATHER_STRAP),
        # 大腿外侧加固骨扣 (Bone buckle)
        c(mount, f"{prefix}_thigh_buckle", (ox(1.7, 0.55), 10.2 + dy, -0.6 + dz), (0.55, 1.2, 1.2), UV_BONE_MAIN),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "mutated_bone_leggings",
        "MUTATED BONE LEGGINGS",
        _leggings_single_leg("LEFT_LEG") + _leggings_single_leg("RIGHT_LEG"),
    )


def parts() -> tuple[ArmorPart, ...]:
    return part_helmet(), part_chestplate(), part_leggings(), part_boots()


# ─── 贴图生成 (64x64 异兽骨、粗麻布与熟皮材质) ──────────────────────────────────

def make_texture() -> Image.Image:
    """生成包含 4 种微观材质特性的 64x64 骨甲贴图。"""
    rng = random.Random(0xDEAD_B0AE)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (218, 210, 192))
    pixels = image.load()

    # 1. 基础噪点与象限底色铺设
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                # Q1: 主兽骨质（象牙白底色 + 微弱风化黄斑）
                base = (224, 216, 198) if (x + y) % 3 != 0 else (212, 204, 186)
            elif x >= 32 and y < 32:
                # Q2: 粗麻战布与衬底（中性灰褐/茶褐粗织纹理）
                base = (138, 124, 106) if (x + y) % 2 == 0 else (122, 110, 92)
            elif x < 32 and y >= 32:
                # Q3: 熟皮革带与麻绳（深棕黑硬鞣皮与草麻绳）
                base = (64, 46, 34) if y % 2 == 0 else (52, 36, 26)
            else:
                # Q4: 兽颅细部、眼眶孔洞与暗色氧化血渍
                base = (42, 34, 30) if (x + y) % 4 == 0 else (118, 52, 42)

            noise = rng.randint(-7, 7)
            warm = rng.randint(-3, 3)
            pixels[x, y] = (
                max(0, min(255, base[0] + noise + warm)),
                max(0, min(255, base[1] + noise)),
                max(0, min(255, base[2] + noise - warm)),
            )

    draw = ImageDraw.Draw(image)

    # 2. Q1 主骨区：骨质天然纵向纤维与裂纹 (Hairline marrow cracks)
    for crack in (
        ((4, 4), (8, 12), (6, 24)),
        ((16, 2), (14, 14), (20, 28)),
        ((24, 6), (28, 16), (25, 26)),
    ):
        draw.line(crack, fill=(142, 130, 110), width=1)

    # 3. Q2 粗麻战布区：经纬编织细纹与深色褶皱 (Weave texture & fold shadows)
    for y in range(2, 30, 3):
        draw.line((32, y, 63, y), fill=(98, 86, 72), width=1)
    for x in range(34, 62, 4):
        draw.line((x, 0, x, 31), fill=(154, 140, 122), width=1)

    # 4. Q3 熟皮与绳圈区：皮带缝线、双股绳纹与青铜铆钉点
    for y in (36, 42, 48, 54, 60):
        # 缝线凹槽
        draw.line((0, y, 31, y), fill=(34, 24, 18), width=1)
        # 缝线高光
        draw.line((0, y + 1, 31, y + 1), fill=(86, 64, 48), width=1)

    # 麻绳斜向双股绞绳纹 (Q3 局部)
    for x_start in range(0, 30, 4):
        draw.line((x_start, 48, x_start + 3, 63), fill=(108, 84, 56), width=1)

    # 青铜铆钉点
    for rx, ry in ((6, 38), (14, 44), (22, 50), (10, 56), (26, 38)):
        draw.point((rx, ry), fill=(168, 142, 78))      # 古铜黄金高光
        draw.point((rx + 1, ry), fill=(78, 92, 64))    # 铜绿晕影

    # 5. Q4 兽颅区：暗色眼眶深洞与牙齿微光
    # 眼眶深洞填充 (纯黑洞孔)
    draw.rectangle((36, 36, 46, 46), fill=(22, 18, 16))
    draw.rectangle((48, 36, 58, 46), fill=(22, 18, 16))
    # 獠牙象牙高光条
    draw.line(((38, 52), (38, 62)), fill=(240, 236, 224), width=2)
    draw.line(((44, 52), (44, 62)), fill=(240, 236, 224), width=2)
    draw.line(((50, 52), (50, 62)), fill=(240, 236, 224), width=2)

    return image


def _assert_no_coplanar_faces(all_parts: tuple[ArmorPart, ...]) -> None:
    """严格检查同平面共面 Z-fighting。"""
    from bbmodel_maker.model.armor_model_common import MOUNT_X

    def bounds(cube: Cube) -> tuple[tuple[float, ...], tuple[float, ...]]:
        offset = MOUNT_X[cube.mount]
        low = (cube.origin[0] + offset, cube.origin[1], cube.origin[2])
        return low, tuple(low[i] + cube.size[i] for i in range(3))

    for part in all_parts:
        cubes = part.cubes
        for i in range(len(cubes)):
            for j in range(i + 1, len(cubes)):
                first, second = cubes[i], cubes[j]
                low_a, high_a = bounds(first)
                low_b, high_b = bounds(second)
                for axis in range(3):
                    overlap = 1.0
                    for other in (k for k in range(3) if k != axis):
                        overlap *= max(0.0, min(high_a[other], high_b[other]) - max(low_a[other], low_b[other]))
                    if overlap <= 0.02:
                        continue
                    for face, value_a, value_b in (
                        ("max", high_a[axis], high_b[axis]),
                        ("min", low_a[axis], low_b[axis]),
                    ):
                        if abs(value_a - value_b) < 1e-6:
                            raise ValueError(
                                f"{part.key}: {first.name} 与 {second.name} 的 "
                                f"{'xyz'[axis]}-{face} 面共面于 {value_a}，"
                                f"投影相交 {overlap:.2f}——会产生 z-fighting 噪点，需微调偏置"
                            )


UV_TILES = {
    UV_BONE_MAIN: (32, 32),
    UV_CLOTH_LINING: (32, 32),
    UV_LEATHER_STRAP: (32, 32),
    UV_SKULL_ACCENT: (32, 32),
}


def _assert_uv_tiles(all_parts: tuple[ArmorPart, ...]) -> None:
    """每只 box 的展开面必须留在其声明的材质象限内。"""
    for part in all_parts:
        for cube in part.cubes:
            tile = UV_TILES.get(cube.uv)
            if tile is None:
                raise ValueError(f"{part.key}/{cube.name}: uv {cube.uv} 不在 UV_TILES")
            tile_w, tile_h = tile
            sx, sy, sz = cube.size
            if 2 * (sx + sz) > tile_w + 1e-6 or sy + sz > tile_h + 1e-6:
                raise ValueError(
                    f"{part.key}/{cube.name}: box-UV {2 * (sx + sz):.2f}×{sy + sz:.2f} "
                    f"超出 {tile_w}×{tile_h} 色块"
                )


def _assert_mirror_symmetry(all_parts: tuple[ArmorPart, ...]) -> None:
    """头盔/双靴的左右件必须关于世界中线镜像。"""
    for part in all_parts:
        if part.key not in {"mutated_bone_helmet", "mutated_bone_boots"}:
            continue
        by_name = {cube.name: cube for cube in part.cubes}
        left = {name[:-5]: cube for name, cube in by_name.items() if name.endswith("_left")}
        right = {name[:-6]: cube for name, cube in by_name.items() if name.endswith("_right")}
        if set(left) != set(right):
            raise ValueError(f"{part.key}: 左右件名不成对 {set(left) ^ set(right)}")
        for name, left_cube in left.items():
            right_cube = right[name]
            left_low = left_cube.origin[0] + MOUNT_X[left_cube.mount]
            right_high = right_cube.origin[0] + MOUNT_X[right_cube.mount] + right_cube.size[0]
            if abs(left_low + right_high) > 1e-6:
                raise ValueError(
                    f"{part.key}/{name}: 左右不镜像（左 x0={left_low:.3f}，右 x1={right_high:.3f}）"
                )
            if left_cube.size != right_cube.size or left_cube.origin[1:] != right_cube.origin[1:]:
                raise ValueError(f"{part.key}/{name}: 左右 y/z/size 不一致")


def _assert_helmet_front_projection(all_parts: tuple[ArmorPart, ...]) -> None:
    """头盔任何前缘不得越过 -z=5，避免裸甲和玩家脸部穿插。"""
    helmet = next((part for part in all_parts if part.key == "mutated_bone_helmet"), None)
    if helmet is None:
        raise ValueError("缺少 mutated_bone_helmet，无法核对前缘")
    for cube in helmet.cubes:
        if cube.origin[2] < -5.0 - 1e-6:
            raise ValueError(
                f"mutated_bone_helmet/{cube.name} 前缘 z={cube.origin[2]:.2f}，超过 -5.00"
            )


# ─── gatekit 差分自证 ───────────────────────────────────────────────────────
# 接触表只看本批新造的头盔/靴子；每道门都必须先注入坏几何再证明自己能报错。
GATE_MATS = {
    "bone": (224, 216, 198),
    "cloth": (138, 124, 106),
    "leather": (64, 46, 34),
    "skull": (118, 52, 42),
}


def _gate_material(cube: Cube) -> str:
    if cube.uv == UV_BONE_MAIN:
        return "bone"
    if cube.uv == UV_CLOTH_LINING:
        return "cloth"
    if cube.uv == UV_LEATHER_STRAP:
        return "leather"
    if cube.uv == UV_SKULL_ACCENT:
        return "skull"
    raise ValueError(f"{cube.name}: 未知 uv {cube.uv}")


def _world_box(cube: Cube) -> tuple[tuple[float, float], ...]:
    offset = MOUNT_X[cube.mount]
    origin = (cube.origin[0] + offset, cube.origin[1], cube.origin[2])
    return tuple((origin[i], origin[i] + cube.size[i]) for i in range(3))


def _cube_bounds(cube: Cube) -> tuple[tuple[float, ...], tuple[float, ...]]:
    box = _world_box(cube)
    return tuple(axis[0] for axis in box), tuple(axis[1] for axis in box)


def _gate_rig(all_parts: tuple[ArmorPart, ...]) -> Rig:
    rig = Rig(GATE_MATS)
    rig._mutated_bone_parts = tuple(all_parts)
    for part in all_parts:
        rig.bone(part.key, (0.0, 0.0, 0.0))
        for cube in part.cubes:
            low, high = _cube_bounds(cube)
            rig.cube(part.key, cube.name, low, high, mat=_gate_material(cube))
    return rig


def build() -> Rig:
    """供接触表与 gatekit 使用的头盔/靴子适配 Rig。"""
    return _gate_rig((part_helmet(), part_boots()))


def _gate_violations(rig: Rig, check) -> list[str]:
    try:
        check(rig._mutated_bone_parts)
    except ValueError as exc:
        return [str(exc)]
    return []


def _replace_gate_cube(rig: Rig, part_key: str, index: int, cube: Cube) -> Rig:
    updated = []
    found = False
    for part in rig._mutated_bone_parts:
        if part.key == part_key:
            cubes = list(part.cubes)
            cubes[index] = cube
            part = replace(part, cubes=tuple(cubes))
            found = True
        updated.append(part)
    if not found:
        raise ValueError(f"gate rig 中没有 {part_key}")
    rig._mutated_bone_parts = tuple(updated)
    return rig


def _inject_coplanar(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._mutated_bone_parts:
        for first_index, first in enumerate(part.cubes):
            low_a, high_a = _cube_bounds(first)
            for second_index in range(first_index + 1, len(part.cubes)):
                second = part.cubes[second_index]
                low_b, high_b = _cube_bounds(second)
                for axis in range(3):
                    projection = 1.0
                    for other in (k for k in range(3) if k != axis):
                        projection *= max(
                            0.0,
                            min(high_a[other], high_b[other])
                            - max(low_a[other], low_b[other]),
                        )
                    if projection <= 0.02:
                        continue
                    origin = list(second.origin)
                    offset = MOUNT_X[second.mount] if axis == 0 else 0.0
                    origin[axis] = high_a[axis] - second.size[axis] - offset
                    _replace_gate_cube(
                        r,
                        part.key,
                        second_index,
                        replace(second, origin=tuple(origin)),
                    )
                    return r, second.name, f"把 {second.name} 的 {'xyz'[axis]} 面移到共面"
    raise gatekit.InjectionImpossible("找不到可造共面且有投影重叠的 cube 对")


def _inject_uv(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    part = r._mutated_bone_parts[0]
    cube = part.cubes[0]
    _replace_gate_cube(r, part.key, 0, replace(cube, uv=(TEXTURE_SIZE, TEXTURE_SIZE)))
    return r, cube.name, f"把 {cube.name} 的 uv 移出 64×64 贴图"


def _inject_mirror(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._mutated_bone_parts:
        for index, cube in enumerate(part.cubes):
            if cube.name.endswith("_left"):
                moved = replace(cube, origin=(cube.origin[0] + 0.9, *cube.origin[1:]))
                _replace_gate_cube(r, part.key, index, moved)
                return r, cube.name[:-5], f"把 {cube.name} 单侧平移 0.9 格"
    raise gatekit.InjectionImpossible("没有参与镜像自检的件")


def _inject_front(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    part = next(part for part in r._mutated_bone_parts if part.key == "mutated_bone_helmet")
    for index, cube in enumerate(part.cubes):
        if cube.name == "brow_keel":
            moved = replace(cube, origin=(cube.origin[0], cube.origin[1], -5.2))
            _replace_gate_cube(r, part.key, index, moved)
            return r, cube.name, "把 brow_keel 前移到脸部禁区"
    raise gatekit.InjectionImpossible("缺少 brow_keel")


class _MutatedBoneArmorGates(gatekit.AssetGates):
    def specs(self):
        return (
            ("coplanar", "单件共面 / z-fighting",
             lambda r: _gate_violations(r, _assert_no_coplanar_faces), _inject_coplanar),
            ("uv_tiles", "box-UV 越出指定色块",
             lambda r: _gate_violations(r, _assert_uv_tiles), _inject_uv),
            ("mirror", "左右件不镜像",
             lambda r: _gate_violations(r, _assert_mirror_symmetry), _inject_mirror),
            ("front_projection", "头盔前缘越过脸部禁区",
             lambda r: _gate_violations(r, _assert_helmet_front_projection), _inject_front),
        )


GATES = _MutatedBoneArmorGates("异变骨甲头盔/靴子", GATE_MATS)


def emit_java(all_parts: tuple[ArmorPart, ...]) -> str:
    """生成注入 ArmorPartModel.java 的字面量。"""
    lines = []
    for part in all_parts:
        method_name = "".join(
            w.capitalize() if i > 0 else w
            for i, w in enumerate(part.key.split("_"))
        )
        lines.append(f"    private static List<ArmorCube> {method_name}() {{")
        lines.append("        return List.of(")
        cube_lines = []
        for c_ in part.cubes:
            ox, oy, oz = c_.origin
            sx, sy, sz = c_.size
            u, v = c_.uv
            cube_lines.append(
                f"            new ArmorCube(Mount.{c_.mount}, {ox:.2f}f, {oy:.2f}f, {oz:.2f}f, "
                f"{sx:.2f}f, {sy:.2f}f, {sz:.2f}f, {u}, {v})"
            )
        lines.append(",\n".join(cube_lines))
        lines.append("        );")
        lines.append("    }\n")
    return "\n".join(lines)


def cube_digest(part: ArmorPart) -> str:
    """复刻 ArmorPartModelTest.cubeDigest 的 FNV-1a，免得手抄 pin 值。"""
    import struct

    def fnv1a(hash_value: int, value: int) -> int:
        for _ in range(4):
            hash_value ^= value & 0xFF
            hash_value = (hash_value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
            value >>= 8
        return hash_value

    def bits(value: float) -> int:
        return struct.unpack("<I", struct.pack("<f", value))[0]

    mounts = ["HEAD", "BODY", "LEFT_LEG", "RIGHT_LEG", "LEFT_FOOT", "RIGHT_FOOT"]
    digest = 0xCBF29CE484222325
    for cube in part.cubes:
        digest = fnv1a(digest, mounts.index(cube.mount))
        for value in (*cube.origin, *cube.size):
            digest = fnv1a(digest, bits(value))
        digest = fnv1a(digest, cube.uv[0])
        digest = fnv1a(digest, cube.uv[1])
    return f"{digest:016x}"


def generate(render_previews: bool = True, install: bool = False) -> dict[str, Path]:
    all_parts = parts()
    _assert_no_coplanar_faces(all_parts)
    _assert_uv_tiles(all_parts)
    _assert_mirror_symmetry(all_parts)
    _assert_helmet_front_projection(all_parts)
    texture = make_texture()
    outputs = write_material_assets(
        MATERIAL,
        all_parts,
        texture,
        LOCAL_MODELS,
        CLIENT_TEXTURE_ROOT if install else DRAFT_TEXTURE_ROOT,
        PREVIEW_ROOT,
        render_previews,
    )

    # 同步写出 128x128 OnPlayer 人体穿戴合模资产，保证生成脚本幂等输出全部交付物
    import sys as _sys
    _sys.path.insert(0, str(REPO / "modelScript" / "tools"))
    from bbmodel_maker.workbench.preview_armor_on_body import make_player_skin, write_player_bbmodel
    skin = make_player_skin()
    model_dir = LOCAL_MODELS / "armor" / MATERIAL
    for part in all_parts:
        on_player_path = write_player_bbmodel(part, skin, texture, MATERIAL, model_dir)
        outputs[f"model_on_player:{part.key}"] = on_player_path

    return outputs


def main() -> None:
    parser = argparse.ArgumentParser(description="生成异兽刺骨甲 3D 程序化资产与贴图。")
    parser.add_argument("--no-preview", action="store_true", help="跳过三视图渲染")
    parser.add_argument("--emit-java", action="store_true", help="输出 ArmorPartModel Java 代码")
    parser.add_argument("--self-test", action="store_true", help="gatekit 差分自证")
    parser.add_argument("--install", action="store_true", help="写入客户端正式资源目录")
    args = parser.parse_args()

    if args.self_test:
        raise SystemExit(GATES.self_test(build()))

    if args.emit_java:
        print(emit_java(parts()))
        return

    outputs = generate(render_previews=not args.no_preview, install=args.install)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
