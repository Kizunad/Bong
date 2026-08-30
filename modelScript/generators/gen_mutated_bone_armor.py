#!/usr/bin/env python3
"""生成异兽刺骨甲（mutated_bone_armor）胸甲与护腿 bbmodel、64x64 UV 贴图与真实三视图预览。

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

运行时真相是 client 的 ArmorPartModel.CUBE_TABLES，本文件的 --emit-java
可直接输出该表的 Java 字面量。
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

from PIL import Image, ImageDraw

import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, TEXTURE_SIZE, write_material_assets

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
    return part_chestplate(), part_leggings()


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


def generate(render_previews: bool = True) -> dict[str, Path]:
    all_parts = parts()
    _assert_no_coplanar_faces(all_parts)
    texture = make_texture()
    outputs = write_material_assets(
        MATERIAL,
        all_parts,
        texture,
        LOCAL_MODELS,
        CLIENT_TEXTURE_ROOT,
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
    args = parser.parse_args()

    if args.emit_java:
        print(emit_java(parts()))
        return

    outputs = generate(render_previews=not args.no_preview)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
