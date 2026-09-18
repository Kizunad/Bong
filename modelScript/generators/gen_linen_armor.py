#!/usr/bin/env python3
"""生成麻布僧袍（linen armor）四件 bbmodel、64x64 UV 贴图与真实三视图预览。

配方对应粗布（rough_cloth）+ 草绳（grass_rope / dried_grass）+ 异变兽骨环扣（bone_chip_mat）。
本套装包含【头盔 (helmet)】、【胸甲 (chestplate)】、【护腿 (leggings)】与【靴子 (boots)】：
- 头盔：多层缠头麻布、额前压边、双侧护耳布片与骨扣/系绳。
- 胸甲：交领右衽僧袍衣身、单肩斜披搭褡（偏衫）、兽骨环扣、多圈麻绳束腰、双臂麻布绑带（绑臂）。
- 护腿：分衩式中长僧袍下摆（随双腿各自运动，杜绝跨腿穿模撕裂）、宽松苦修麻裤、小腿十字交叉绑腿（行缠）。
- 靴子：压低脚底的麻布鞋底、分层鞋面、踝部绳带与外侧系结。

运行时真相是 client 的 ArmorPartModel.CUBE_TABLES，本文件的 --emit-java
直接吐那张表的 Java 字面量。
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

MATERIAL = "linen"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 贴图四象限规划 (64x64)：
# Q1 (0,0)-(32,32): 主粗麻布 (中性浅灰褐麻布，用于内层僧裤与基础衣身)
# Q2 (32,0)-(64,32): 偏衫与外披裙摆深色麻布 (加深茶褐/泥土色，用于单肩搭褡、外层斜倾裙摆与镶边)
# Q3 (0,32)-(32,64): 护手与绑脚白麻布 (明显偏白的漂白/原色燕麦白麻布，显著突出)
# Q4 (32,32)-(64,64): 麻绳束腰与骨环 (深色拧紧麻绳 & 象牙白骨环)
UV_LINEN_MAIN = (0, 0)
UV_LINEN_DARK = (32, 0)
UV_LINEN_WRAP = (0, 32)
UV_HEMP_ROPE = (32, 32)
UV_BONE_RING = (48, 48)


def c(mount: str, name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv: tuple[int, int] = UV_LINEN_MAIN) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 胸甲 (CHESTPLATE) ────────────────────────────────────────────────────────
# 躯干盒 x∈[-4,4] y∈[12,24] z∈[-2,2]，手臂 x∈[±4,±8] y∈[12,24] z∈[-2,2]。
# 构件划分：
# 1. 僧袍主身壳 (tunic shell): 前后片、侧缝片、交领大襟右衽叠层
# 2. 单肩搭褡偏衫 (shoulder sash): 披于左肩/右胸，斜向下延伸，带骨环扣
# 3. 双肩护肩与袖根 (shoulder caps / sleeves)
# 4. 腰部麻绳圈与垂绳 (hemp rope belt & knot)
# 5. 绑臂护腕 (arm wraps): 贴合手臂方块，肘部至手腕的紧致绑带


def _chest_tunic_shell() -> tuple[Cube, ...]:
    """交领右衽僧袍主身：贴合 8x12x4 躯干方块。"""
    return (
        # 前片基础层 (y: 12.6 -> 22.8)
        c("BODY", "tunic_front_base", (-4.3, 12.6, -2.52), (8.6, 10.2, 0.65), UV_LINEN_MAIN),
        # 右衽交领大襟叠层（从右肩向左斜包，增加层次厚度）
        c("BODY", "tunic_cross_collar", (-3.6, 15.2, -2.75), (7.4, 7.8, 0.45), UV_LINEN_MAIN),
        # 领口斜包边（深茶色麻布镶边）
        c("BODY", "tunic_collar_trim_left", (-3.8, 20.4, -2.85), (3.6, 3.2, 0.35), UV_LINEN_DARK),
        c("BODY", "tunic_collar_trim_right", (0.2, 18.2, -2.85), (3.8, 5.4, 0.35), UV_LINEN_DARK),
        # 后背大片 (y: 12.6 -> 23.85)
        c("BODY", "tunic_back", (-4.3, 12.6, 1.88), (8.6, 11.25, 0.65), UV_LINEN_MAIN),
        # 左右侧缝收边
        c("BODY", "tunic_side_left", (-4.45, 12.5, -2.25), (0.5, 11.2, 4.5), UV_LINEN_MAIN),
        c("BODY", "tunic_side_right", (3.95, 12.5, -2.25), (0.5, 11.2, 4.5), UV_LINEN_MAIN),
        # 过肩（连接前后片并盖住肩胛内侧，避开头部 y=24 原点）
        c("BODY", "tunic_yoke_left", (-4.35, 22.8, -2.5), (2.0, 1.15, 5.0), UV_LINEN_MAIN),
        c("BODY", "tunic_yoke_right", (2.35, 22.8, -2.5), (2.0, 1.15, 5.0), UV_LINEN_MAIN),
    )


def _chest_shoulder_sash() -> tuple[Cube, ...]:
    """苦修单肩偏衫（搭褡）：从右肩斜披至左腰，带有折褶与骨扣固定。"""
    return (
        # 右肩顶部的厚搭褡
        c("BODY", "sash_shoulder_top", (2.1, 23.85, -2.65), (2.4, 0.6, 5.3), UV_LINEN_DARK),
        # 前胸斜披向左下的大布褶
        c("BODY", "sash_chest_drape", (-1.8, 14.8, -2.9), (4.8, 8.4, 0.35), UV_LINEN_DARK),
        # 斜搭褡下摆收束
        c("BODY", "sash_lower_fold", (-3.2, 12.8, -2.88), (3.6, 4.2, 0.32), UV_LINEN_DARK),
        # 背面斜搭拉伸带
        c("BODY", "sash_back_drape", (-0.8, 15.0, 2.35), (4.6, 8.2, 0.35), UV_LINEN_DARK),
        # 固定搭褡的兽骨环扣 (Bone Ring Toggle)
        c("BODY", "sash_bone_ring", (1.2, 19.8, -3.15), (1.2, 1.2, 0.45), UV_BONE_RING),
        c("BODY", "sash_bone_knot", (1.4, 19.2, -3.05), (0.8, 0.8, 0.3), UV_HEMP_ROPE),
    )


def _chest_shoulder_caps() -> tuple[Cube, ...]:
    """左右长袖与前臂布条护腕（长袖+前臂绑带护腕）。"""
    cubes: list[Cube] = []
    for side, sign in (("left", -1.0), ("right", 1.0)):
        def x(inner: float, width: float) -> float:
            return inner if sign > 0 else -inner - width

        cubes.extend((
            # 1. 压住臂顶的肩头布盖 (x: 3.9 -> 8.1, y: 23.8 -> 24.35)
            c("BODY", f"sleeve_top_{side}", (x(3.9, 4.2), 23.8, -2.45), (4.2, 0.55, 4.9), UV_LINEN_MAIN),

            # 2. 大臂宽松长袖筒 (y: 17.5 -> 23.75)
            c("BODY", f"sleeve_upper_out_{side}", (x(7.65, 0.65), 17.5, -2.35), (0.65, 6.2, 4.7), UV_LINEN_MAIN),
            c("BODY", f"sleeve_upper_front_{side}", (x(4.1, 3.5), 17.5, -2.55), (3.5, 6.25, 0.55), UV_LINEN_MAIN),
            c("BODY", f"sleeve_upper_back_{side}", (x(4.1, 3.5), 17.5, 2.0), (3.5, 6.25, 0.55), UV_LINEN_MAIN),
            # 袖口微张落差折边 (y: 16.8 -> 17.6)
            c("BODY", f"sleeve_cuff_trim_{side}", (x(4.0, 4.2), 16.8, -2.62), (4.2, 0.8, 5.24), UV_LINEN_DARK),

            # 3. 前臂紧致布条护腕（绑臂，y: 12.2 -> 17.2）
            c("BODY", f"arm_wrap_core_{side}", (x(4.05, 4.0), 12.2, -2.2), (4.0, 4.8, 4.4), UV_LINEN_WRAP),
            # 护腕十字交错绑绳带 (3 道凸起绑扎)
            c("BODY", f"arm_wrap_band_top_{side}", (x(3.98, 4.15), 15.6, -2.35), (4.15, 0.55, 4.7), UV_HEMP_ROPE),
            c("BODY", f"arm_wrap_band_mid_{side}", (x(3.98, 4.15), 13.9, -2.35), (4.15, 0.55, 4.7), UV_HEMP_ROPE),
            c("BODY", f"arm_wrap_band_low_{side}", (x(3.98, 4.15), 12.35, -2.32), (4.15, 0.55, 4.64), UV_HEMP_ROPE),
            # 护腕外侧绳结
            c("BODY", f"arm_wrap_knot_{side}", (x(7.9, 0.45), 14.8, -2.4), (0.45, 0.85, 0.7), UV_HEMP_ROPE),
        ))
    return tuple(cubes)


def _chest_belt_and_ropes() -> tuple[Cube, ...]:
    """腰间粗麻绳系腰（多圈粗麻绳紧密盘绕 + 粗麻绳结与粗垂绳）。"""
    return (
        # 粗麻绳圈 第 1 圈（上圈，y: 13.0 -> 13.9，厚实粗绳）
        c("BODY", "belt_rope_upper_front", (-4.65, 13.05, -3.0), (9.3, 0.85, 0.65), UV_HEMP_ROPE),
        c("BODY", "belt_rope_upper_back", (-4.65, 13.05, 2.35), (9.3, 0.85, 0.65), UV_HEMP_ROPE),
        c("BODY", "belt_rope_upper_left", (-5.0, 13.05, -2.35), (0.65, 0.85, 4.7), UV_HEMP_ROPE),
        c("BODY", "belt_rope_upper_right", (4.35, 13.05, -2.35), (0.65, 0.85, 4.7), UV_HEMP_ROPE),

        # 粗麻绳圈 第 2 圈（下圈，y: 12.15 -> 13.0，交错缠绕）
        c("BODY", "belt_rope_lower_front", (-4.6, 12.15, -2.95), (9.2, 0.85, 0.6), UV_HEMP_ROPE),
        c("BODY", "belt_rope_lower_back", (-4.6, 12.15, 2.35), (9.2, 0.85, 0.6), UV_HEMP_ROPE),
        c("BODY", "belt_rope_lower_left", (-4.95, 12.15, -2.35), (0.6, 0.85, 4.7), UV_HEMP_ROPE),
        c("BODY", "belt_rope_lower_right", (4.35, 12.15, -2.35), (0.6, 0.85, 4.7), UV_HEMP_ROPE),

        # 粗绳斜向交缠锁扣（模拟双绳绞合感）
        c("BODY", "belt_cross_coil_left", (-2.6, 12.4, -3.12), (1.4, 1.4, 0.45), UV_HEMP_ROPE),
        c("BODY", "belt_cross_coil_right", (1.2, 12.4, -3.12), (1.4, 1.4, 0.45), UV_HEMP_ROPE),

        # 正中大粗绳结 (Main Heavy Rope Knot)
        c("BODY", "belt_main_knot_core", (-1.1, 11.8, -3.45), (2.2, 1.9, 0.85), UV_HEMP_ROPE),
        c("BODY", "belt_main_knot_cross", (-0.7, 12.1, -3.6), (1.4, 1.3, 0.55), UV_HEMP_ROPE),

        # 垂下的粗麻绳尾（左长右短，下段略微往前探避免迈腿共面）
        c("BODY", "belt_tail_left_top", (-0.9, 9.6, -3.42), (0.65, 2.4, 0.65), UV_HEMP_ROPE),
        c("BODY", "belt_tail_left_mid", (-0.95, 7.4, -3.55), (0.6, 2.3, 0.6), UV_HEMP_ROPE),
        c("BODY", "belt_tail_left_tip", (-0.92, 5.8, -3.65), (0.55, 1.7, 0.55), UV_HEMP_ROPE),

        c("BODY", "belt_tail_right_top", (0.25, 10.2, -3.42), (0.6, 1.8, 0.6), UV_HEMP_ROPE),
        c("BODY", "belt_tail_right_tip", (0.28, 8.4, -3.52), (0.55, 1.9, 0.55), UV_HEMP_ROPE),

        # 悬挂的小布囊/药包 (hanging cloth pouch)
        c("BODY", "belt_pouch", (2.4, 10.5, -3.18), (1.8, 2.2, 0.75), UV_LINEN_DARK),
        c("BODY", "belt_pouch_flap", (2.3, 12.05, -3.25), (1.95, 0.9, 0.85), UV_LINEN_MAIN),
    )


def _chest_arm_wraps() -> tuple[Cube, ...]:
    """手臂绑带（绑臂）：肘部至腕部的紧密缠布。

    注意：MC biped 挂载点只有 BODY（无独立手臂 mount），但 ARMOR 渲染在原版中由
    Minecraft 负责在装备模型渲染时同步。
    若要在 BODY mount 上渲染贴身绑臂，手臂位置在 x∈[±4, ±8]。
    """
    return ()


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "linen_chestplate",
        "LINEN CHESTPLATE",
        _chest_tunic_shell()
        + _chest_shoulder_sash()
        + _chest_shoulder_caps()
        + _chest_belt_and_ropes()
        + _chest_arm_wraps(),
    )


# ─── 头盔 (HELMET) ────────────────────────────────────────────────────────────
# 头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，脸朝 -z。
# 这里做的是软麻布护头，不把它误做成硬壳：两层顶布负责遮住头顶，额前压边、
# 护耳片与后脑搭接负责「缠住」的连续轮廓，绳带和骨扣只做外露的固定件。


def _helmet_crown() -> tuple[Cube, ...]:
    """贴着头盒分层铺布；薄片沿头顶收边，不做外伸的硬帽檐。"""
    return (
        # 内层只比头盒略收，薄而低，不形成四面外伸的平板。
        c("HEAD", "cap_top_front", (-3.72, 31.62, -3.55), (7.44, 0.30, 3.05), UV_LINEN_DARK),
        # 后半片向后错 0.12，和前半片的边缘读成搭接而不是一整块硬壳。
        c("HEAD", "cap_top_back", (-3.72, 31.74, -0.42), (7.44, 0.30, 3.80), UV_LINEN_MAIN),
        # 顶布左右的折边收在头盒侧面，形成缠布的两道纵向转折。
        c("HEAD", "cap_front_fold", (-3.88, 31.28, -3.82), (7.76, 0.28, 0.42), UV_LINEN_MAIN),
        c("HEAD", "cap_back_bridge", (-3.88, 31.18, 3.42), (7.76, 0.28, 0.42), UV_LINEN_DARK),
    )


def _helmet_wrap() -> tuple[Cube, ...]:
    """额前压布、双侧缠布和短护耳；护耳只贴左右脸颊，不垂到肩部。"""
    return (
        # 三层高差形成「额前压布 → 顶布 → 后脑搭接」的缠绕顺序；浅色只在压边。
        c("HEAD", "forehead_wrap", (-4.12, 28.72, -4.18), (8.24, 1.42, 0.44), UV_LINEN_WRAP),
        c("HEAD", "forehead_edge", (-4.18, 28.48, -4.42), (8.36, 0.26, 0.28), UV_HEMP_ROPE),
        c("HEAD", "side_wrap_left", (-4.28, 28.35, -3.35), (0.38, 2.45, 6.7), UV_LINEN_DARK),
        c("HEAD", "side_wrap_right", (3.90, 28.35, -3.35), (0.38, 2.45, 6.7), UV_LINEN_DARK),
        # 护耳片只占前侧脸颊，底边在下颌线上方，不再把头盔撑成四条桌腿。
        c("HEAD", "ear_flap_left", (-4.38, 24.72, -3.62), (0.46, 2.42, 2.35), UV_LINEN_DARK),
        c("HEAD", "ear_flap_right", (3.92, 24.72, -3.62), (0.46, 2.42, 2.35), UV_LINEN_DARK),
        c("HEAD", "ear_flap_hem_left", (-4.44, 24.48, -3.68), (0.52, 0.20, 2.45), UV_LINEN_WRAP),
        c("HEAD", "ear_flap_hem_right", (3.92, 24.48, -3.68), (0.52, 0.20, 2.45), UV_LINEN_WRAP),
        # 后脑搭接比顶布低一层，贴着后枕而非向外伸出一圈。
        c("HEAD", "back_drape", (-4.10, 25.85, 3.92), (8.20, 2.15, 0.38), UV_LINEN_DARK),
    )


def _helmet_fasteners() -> tuple[Cube, ...]:
    """外露的双侧系带与额中骨扣；固定件沿缠布边缘走，不成为桌腿。"""
    return (
        c("HEAD", "side_tie_left", (-4.82, 27.15, -3.12), (0.58, 0.64, 0.96), UV_HEMP_ROPE),
        c("HEAD", "side_tie_right", (4.24, 27.15, -3.12), (0.58, 0.64, 0.96), UV_HEMP_ROPE),
        c("HEAD", "side_knot_left", (-4.92, 26.40, -3.22), (0.78, 0.78, 0.78), UV_HEMP_ROPE),
        c("HEAD", "side_knot_right", (4.14, 26.40, -3.22), (0.78, 0.78, 0.78), UV_HEMP_ROPE),
        c("HEAD", "bone_toggle", (-0.60, 29.42, -4.52), (1.20, 0.82, 0.34), UV_BONE_RING),
    )


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "linen_helmet",
        "LINEN HELMET",
        _helmet_crown() + _helmet_wrap() + _helmet_fasteners(),
    )


# ─── 靴子 (BOOTS) ────────────────────────────────────────────────────────────
# 脚 mount 的枢轴在 y=12，但脚底的世界高度仍是 y=0；因此鞋底必须压到 y<0，
# 鞋筒从脚背向上包到 y≈4。左右脚只在局部 x 上镜像，内缘留在中线外侧，避免
# 两只软靴静止时粘成一块。


def _boot_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """一只麻布软靴：前伸鞋头、收窄后跟与明显立起的鞋筒。"""
    side = "left" if sign > 0 else "right"
    outward_clearance = 0.45

    def x(inner: float, width: float) -> float:
        # vanilla 左右脚枢轴相距 3.8，而鞋底最宽处超过 4 单位；向外侧挪开
        # 0.45，给中线留出 0.06~0.16 的可见缝，避免两只静止软靴互相穿入。
        return (
            inner + outward_clearance
            if sign > 0
            else -inner - width - outward_clearance
        )

    def c2(name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv=UV_LINEN_MAIN) -> Cube:
        return Cube(mount, f"{name}_{side}", origin, size, uv)

    return (
        # 鞋底按前掌/中掌/后跟分段，前端明显多伸一截，避免侧视读成方盒。
        c2("sole_base_toe", (x(-2.0, 4.0), -0.42, -3.82), (4.0, 0.52, 2.30), UV_LINEN_DARK),
        c2("sole_base_mid", (x(-2.0, 4.0), -0.39, -1.48), (4.0, 0.49, 2.16), UV_LINEN_DARK),
        c2("sole_base_heel", (x(-1.76, 3.52), -0.35, 0.74), (3.52, 0.45, 1.92), UV_LINEN_DARK),
        c2("sole_front_rim", (x(-2.10, 4.20), -0.02, -4.04), (4.20, 0.42, 0.40), UV_HEMP_ROPE),
        c2("sole_back_rim", (x(-1.86, 3.72), 0.01, 2.68), (3.72, 0.40, 0.34), UV_HEMP_ROPE),
        c2("sole_outer_rim", (x(2.03, 0.32), 0.01, -3.62), (0.32, 0.43, 6.00), UV_HEMP_ROPE),
        c2("sole_inner_rim", (x(-2.03, 0.28), 0.01, -3.58), (0.28, 0.40, 5.94), UV_HEMP_ROPE),

        # 鞋面深色为底：前掌向 -z 伸出，后跟向 +z 收窄，侧视能明确分出前后。
        c2("vamp_main", (x(-1.90, 3.80), 0.14, -3.48), (3.80, 0.88, 2.48), UV_LINEN_DARK),
        c2("vamp_mid", (x(-1.94, 3.88), 0.20, -1.08), (3.88, 1.02, 1.72), UV_LINEN_DARK),
        c2("vamp_toe_panel", (x(-1.84, 3.68), 0.92, -3.72), (3.68, 0.42, 0.74), UV_LINEN_MAIN),
        c2("vamp_heel_fold", (x(-1.78, 3.56), 0.22, 0.62), (3.56, 0.96, 1.36), UV_LINEN_MAIN),
        c2("vamp_lace_front", (x(-1.48, 2.96), 1.12, -2.56), (2.96, 0.34, 0.34), UV_HEMP_ROPE),
        c2("vamp_lace_back", (x(-1.34, 2.68), 1.42, -0.92), (2.68, 0.34, 0.34), UV_HEMP_ROPE),

        # 鞋筒从踝部向上立起约 3.2 单位，围住小腿但保留顶部开口；四片不做整块箱体。
        c2("shaft_front", (x(-1.80, 3.60), 1.28, -1.72), (3.60, 3.12, 0.36), UV_LINEN_DARK),
        c2("shaft_back", (x(-1.76, 3.52), 1.28, 1.34), (3.52, 3.12, 0.38), UV_LINEN_DARK),
        c2("shaft_outer", (x(1.75, 0.38), 1.28, -1.34), (0.38, 3.12, 2.70), UV_LINEN_DARK),
        c2("shaft_inner", (x(-1.72, 0.32), 1.28, -1.30), (0.32, 3.12, 2.62), UV_LINEN_MAIN),
        # 靴筒口延续护腿底部的米白横箍；只做前后和两侧窄条，不封住开口。
        c2("shaft_top_front", (x(-1.86, 3.72), 4.42, -1.78), (3.72, 0.42, 0.38), UV_LINEN_WRAP),
        c2("shaft_top_back", (x(-1.82, 3.64), 4.42, 1.40), (3.64, 0.42, 0.36), UV_LINEN_WRAP),

        # 踝部深色绳带作为点缀，避免米白面被切成密集板条。
        c2("ankle_band_low_front", (x(-1.92, 3.84), 1.44, -1.98), (3.84, 0.32, 0.30), UV_HEMP_ROPE),
        c2("ankle_band_low_back", (x(-1.88, 3.76), 1.48, 1.64), (3.76, 0.32, 0.28), UV_HEMP_ROPE),
        c2("ankle_band_low_outer", (x(1.82, 0.30), 1.46, -1.70), (0.30, 0.32, 3.42), UV_HEMP_ROPE),
        c2("ankle_band_high_front", (x(-1.90, 3.80), 2.72, -1.94), (3.80, 0.30, 0.28), UV_HEMP_ROPE),
        c2("ankle_band_high_back", (x(-1.86, 3.72), 2.76, 1.60), (3.72, 0.30, 0.26), UV_HEMP_ROPE),
        c2("ankle_band_high_outer", (x(1.80, 0.28), 2.74, -1.66), (0.28, 0.30, 3.34), UV_HEMP_ROPE),
        c2("side_knot", (x(2.12, 0.72), 2.32, -0.86), (0.72, 0.76, 0.76), UV_BONE_RING),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "linen_boots",
        "LINEN BOOTS",
        _boot_cubes("LEFT_FOOT", 1.0) + _boot_cubes("RIGHT_FOOT", -1.0),
    )


# ─── 护腿 (LEGGINGS) ──────────────────────────────────────────────────────────
# 腿盒局部坐标 x∈[-2,2] y∈[0,12] z∈[-2,2]，骨骼枢轴在 y=12。
# 构件划分：
# 1. 裤腰与胯部连接层 (hip band)
# 2. 分衩式下摆长袍裙片 (robe skirts): 前后左右独立片，长及膝部 (y=4.5~5.0)
# 3. 宽松苦修僧裤 (loose monk pants)
# 4. 小腿行缠绑腿 (calf wraps / leg bindings): 紧紧缠绕至脚踝


def _leg_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """单腿构件组装。sign=+1 左腿 (LEFT_LEG)，-1 右腿 (RIGHT_LEG)。"""
    side = "left" if sign > 0 else "right"

    def x(inner: float, width: float) -> float:
        return inner if sign > 0 else -inner - width

    def c2(name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv: tuple[int, int] = UV_LINEN_MAIN) -> Cube:
        return Cube(mount, f"{name}_{side}", origin, size, uv)

    # 左右微小错开避免背部中线共面
    zoff = 0.0 if sign > 0 else 0.05

    return (
        # ── 1. 髋部接缝层 (y: 10.2 -> 12.1) ──
        c2("hip_front", (x(-1.85, 4.3), 10.4, -2.45), (4.3, 1.7, 0.65), UV_LINEN_MAIN),
        c2("hip_back", (x(-1.85, 4.3), 10.4, 1.8), (4.3, 1.7, 0.65), UV_LINEN_MAIN),
        c2("hip_side", (x(1.95, 0.55), 10.35, -2.3), (0.55, 1.7, 4.6), UV_LINEN_MAIN),

        # ── 2. 分衩僧袍裙摆 (Robe Skirt Panels, y: 3.6 -> 10.6) ──
        # 挂在腿上随腿摆动，前后左右分片 + 带有明显大倾角斜切布条（呈深茶色，比内裤深）
        c2("skirt_front_upper", (x(-1.75, 4.1), 6.6, -2.6), (4.1, 4.0, 0.55), UV_LINEN_DARK),
        # 前下摆大倾角层（从内侧上部向外侧下部大幅度斜切展开，阶梯式呈现显著斜势）
        c2("skirt_front_slant_inner", (x(-1.72, 1.8), 5.4, -2.63), (1.8, 1.6, 0.54), UV_LINEN_DARK),
        c2("skirt_front_slant_mid", (x(-0.2, 1.8), 4.4, -2.67), (1.8, 2.4, 0.55), UV_LINEN_DARK),
        c2("skirt_front_slant_outer", (x(1.3, 1.45), 3.5, -2.71), (1.45, 3.2, 0.60), UV_LINEN_DARK),
        c2("skirt_front_slant_tip", (x(2.1, 0.8), 3.0, -2.75), (0.8, 1.2, 0.63), UV_LINEN_DARK),

        # 后下摆大倾角层
        c2("skirt_back_upper", (x(-1.75, 4.1), 6.6, 2.05 + zoff), (4.1, 4.0, 0.55), UV_LINEN_DARK),
        c2("skirt_back_slant_inner", (x(-1.72, 1.8), 5.4, 2.08 + zoff), (1.8, 1.6, 0.54), UV_LINEN_DARK),
        c2("skirt_back_slant_mid", (x(-0.2, 1.8), 4.4, 2.12 + zoff), (1.8, 2.4, 0.55), UV_LINEN_DARK),
        c2("skirt_back_slant_outer", (x(1.3, 1.45), 3.5, 2.16 + zoff), (1.45, 3.2, 0.60), UV_LINEN_DARK),
        c2("skirt_back_slant_tip", (x(2.1, 0.8), 3.0, 2.20 + zoff), (0.8, 1.2, 0.63), UV_LINEN_DARK),

        # 侧面分衩外披大飘带 (深色外披裙身 + 大倾角斜拉飘带)
        c2("skirt_outer_main", (x(1.92, 0.55), 5.6, -2.4), (0.55, 4.75, 4.8), UV_LINEN_DARK),
        c2("skirt_outer_slant_ribbon", (x(1.98, 0.52), 3.72, -2.1), (0.52, 2.85, 4.2), UV_LINEN_DARK),
        c2("skirt_outer_slant_tip", (x(2.05, 0.48), 2.85, -1.0), (0.48, 1.6, 2.4), UV_LINEN_DARK),

        # ── 3. 内层僧裤 (Pants Core, y: 3.5 -> 10.0) ──
        c2("pants_front", (x(-1.65, 3.75), 4.2, -2.32), (3.75, 6.2, 0.45), UV_LINEN_MAIN),
        c2("pants_back", (x(-1.65, 3.75), 4.2, 1.88 + zoff), (3.75, 6.2, 0.45), UV_LINEN_MAIN),
        c2("pants_inner", (x(-1.82, 0.45), 4.5, -2.1), (0.45, 5.8, 4.2), UV_LINEN_MAIN),

        # ── 4. 小腿十字行缠绑腿 (Calf Leg Wraps, y: 0.2 -> 5.4) ──
        # 紧致贴合小腿，多层交错呈现绑带体积感
        c2("wrap_lower_main", (x(-1.8, 4.0), 0.4, -2.25), (4.0, 4.4, 4.5), UV_LINEN_WRAP),
        # 凸起的十字绑绳条 (Criss-cross rope bindings)
        c2("wrap_band_1", (x(-1.85, 4.1), 3.8, -2.35), (4.1, 0.6, 4.7), UV_HEMP_ROPE),
        c2("wrap_band_2", (x(-1.85, 4.1), 2.2, -2.35), (4.1, 0.6, 4.7), UV_HEMP_ROPE),
        c2("wrap_band_3", (x(-1.85, 4.1), 0.6, -2.35), (4.1, 0.6, 4.7), UV_HEMP_ROPE),
        # 绑腿上端收口结
        c2("wrap_top_knot", (x(1.75, 0.55), 4.15, -2.4), (0.55, 0.7, 0.6), UV_HEMP_ROPE),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "linen_leggings",
        "LINEN LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.0) + _leg_cubes("RIGHT_LEG", -1.0),
    )


def parts() -> tuple[ArmorPart, ...]:
    return (part_helmet(), part_chestplate(), part_leggings(), part_boots())


# ─── 贴图生成 (64x64) ─────────────────────────────────────────────────────────


def _mottle(image: Image.Image, rng: random.Random, box: tuple[int, int, int, int], count: int, dark: tuple[int, int, int], light: tuple[int, int, int], radius: tuple[float, float]) -> None:
    """平滑低频织物杂色斑块。"""
    x0, y0, x1, y1 = box
    pixels = image.load()
    for _ in range(count):
        cx, cy = rng.uniform(x0, x1), rng.uniform(y0, y1)
        rx, ry = rng.uniform(*radius), rng.uniform(*radius)
        tint = dark if rng.random() < 0.55 else light
        peak = rng.uniform(0.18, 0.38)
        for y in range(max(y0, int(cy - ry)), min(y1, int(cy + ry) + 1)):
            for x in range(max(x0, int(cx - rx)), min(x1, int(cx + rx) + 1)):
                d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
                if d > 1.0:
                    continue
                alpha = peak * (1.0 - d)
                pixels[x, y] = tuple(
                    int(round(channel * (1 - alpha) + target * alpha))
                    for channel, target in zip(pixels[x, y], tint)
                )


def make_texture() -> Image.Image:
    """生成 64x64 粗麻布衣贴图。"""
    rng = random.Random(0x4D4F4E4B)  # "MONK" seed
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (105, 95, 80))
    pixels = image.load()

    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                base = (118, 108, 95)      # Q1: 主粗麻布 (中性浅灰褐，用于裤腿与内衬)
            elif y < 32:
                base = (64, 52, 40)        # Q2: 偏衫与外披裙摆深色麻布 (加深深茶褐，明显深于裤腿)
            elif x < 32:
                base = (178, 168, 150)     # Q3: 护手与绑脚细麻布 (明显偏白的燕麦白/漂白麻布，显著突出)
            else:
                base = (85, 72, 54)        # Q4: 麻绳与构件 (深褐搓绳)

            v_jitter = rng.randint(-5, 5)
            w_jitter = rng.randint(-3, 3)
            pixels[x, y] = tuple(
                max(0, min(255, c + v_jitter + (w_jitter if i == 0 else -w_jitter if i == 2 else 0)))
                for i, c in enumerate(base)
            )

    # 1. Q1 主粗麻布：经纬编织噪点与低频浅色渍
    _mottle(image, rng, (0, 0, 32, 32), 16, (96, 86, 74), (140, 130, 115), (3.5, 10.0))
    draw = ImageDraw.Draw(image)
    for col in range(1, 32, 2):
        for row in range(1, 32, 2):
            if rng.random() < 0.35:
                draw.point((col, row), fill=(88, 78, 66))
            elif rng.random() < 0.35:
                draw.point((col, row), fill=(145, 135, 120))

    # 2. Q2 偏衫与外裙深色麻布：深茶褐色 + 竖向粗纹理
    _mottle(image, rng, (32, 0, 64, 32), 16, (48, 38, 28), (80, 68, 54), (3.0, 8.0))
    for col in range(33, 64, 3):
        draw.line((col, 0, col, 31), fill=(52, 42, 32), width=1)

    # 3. Q3 护手与绑脚白麻布：高亮燕麦白 + 干净细密的交叉编线
    _mottle(image, rng, (0, 32, 32, 64), 16, (155, 145, 128), (202, 192, 175), (3.0, 9.0))
    for row in range(33, 64, 3):
        draw.line((0, row, 31, row), fill=(162, 152, 135), width=1)
        for col in range(1, 31, 3):
            draw.point((col, row), fill=(215, 206, 190))

    # 4. Q4 麻绳与兽骨环扣
    rope_patch = image.crop((32, 32, 64, 64))
    rope_draw = ImageDraw.Draw(rope_patch)
    for start in range(-12, 36, 4):
        rope_draw.line((start, 31, start + 10, 0), fill=(65, 54, 38), width=1)
        rope_draw.line((start + 2, 31, start + 12, 0), fill=(110, 96, 75), width=1)
    image.paste(rope_patch, (32, 32))

    # 兽骨专用色块 (UV_BONE_RING: 48, 48 -> 60, 60)
    draw.rectangle((48, 48, 60, 60), fill=(210, 204, 185))
    _mottle(image, rng, (48, 48, 60, 60), 6, (175, 168, 150), (235, 228, 210), (1.5, 3.5))
    draw.rectangle((52, 52, 56, 56), fill=(145, 138, 120))  # 环孔深色

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
    UV_LINEN_MAIN: (32, 32),
    UV_LINEN_DARK: (32, 32),
    UV_LINEN_WRAP: (32, 32),
    UV_HEMP_ROPE: (32, 32),
    UV_BONE_RING: (12, 12),
}


def _assert_uv_tiles(all_parts: tuple[ArmorPart, ...]) -> None:
    """每只 box 的展开面必须留在它声明的材质象限内。"""
    for part in all_parts:
        for cube in part.cubes:
            tile = UV_TILES.get(cube.uv)
            if tile is None:
                raise ValueError(f"{part.key}/{cube.name}: uv {cube.uv} 不在 UV_TILES")
            tile_w, tile_h = tile
            u, v = cube.uv
            sx, sy, sz = cube.size
            if 2 * (sx + sz) > tile_w + 1e-6 or sy + sz > tile_h + 1e-6:
                raise ValueError(
                    f"{part.key}/{cube.name}: box-UV {2 * (sx + sz):.2f}×{sy + sz:.2f} "
                    f"超出 {tile_w}×{tile_h} 色块"
                )


def _assert_mirror_symmetry(all_parts: tuple[ArmorPart, ...]) -> None:
    """头盔/双靴的左右件必须关于世界中线镜像。"""
    for part in all_parts:
        if part.key not in {"linen_helmet", "linen_boots"}:
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


# ─── gatekit 差分自证 ───────────────────────────────────────────────────────
# Round 2 的接触表只看本批新造的头盔/靴子；胸甲和护腿沿用既有模型，不让它们把本轮
# 的门禁结果稀释掉。门本身必须配缺陷注入器，干净通过不是差分自证。
GATE_MATS = {
    "linen": (118, 108, 95),
    "dark": (64, 52, 40),
    "wrap": (178, 168, 150),
    "rope": (85, 72, 54),
    "bone": (210, 204, 185),
}


def _gate_material(cube: Cube) -> str:
    if cube.uv == UV_LINEN_MAIN:
        return "linen"
    if cube.uv == UV_LINEN_DARK:
        return "dark"
    if cube.uv == UV_LINEN_WRAP:
        return "wrap"
    if cube.uv == UV_HEMP_ROPE:
        return "rope"
    if cube.uv == UV_BONE_RING:
        return "bone"
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
    rig._linen_parts = tuple(all_parts)
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
        check(rig._linen_parts)
    except ValueError as exc:
        return [str(exc)]
    return []


def _replace_gate_cube(rig: Rig, part_key: str, index: int, cube: Cube) -> Rig:
    updated = []
    found = False
    for part in rig._linen_parts:
        if part.key == part_key:
            cubes = list(part.cubes)
            cubes[index] = cube
            part = replace(part, cubes=tuple(cubes))
            found = True
        updated.append(part)
    if not found:
        raise ValueError(f"gate rig 中没有 {part_key}")
    rig._linen_parts = tuple(updated)
    return rig


def _inject_coplanar(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._linen_parts:
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
                    _replace_gate_cube(r, part.key, second_index,
                                       replace(second, origin=tuple(origin)))
                    return r, second.name, f"把 {second.name} 的 {'xyz'[axis]} 面移到共面"
    raise gatekit.InjectionImpossible("找不到可造共面且有投影重叠的 cube 对")


def _inject_uv(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    part = r._linen_parts[0]
    cube = part.cubes[0]
    _replace_gate_cube(r, part.key, 0, replace(cube, uv=(TEXTURE_SIZE, TEXTURE_SIZE)))
    return r, cube.name, f"把 {cube.name} 的 uv 移出 64×64 贴图"


def _inject_mirror(rig: Rig, **_) -> tuple[Rig, str, str]:
    r = copy.deepcopy(rig)
    for part in r._linen_parts:
        for index, cube in enumerate(part.cubes):
            if cube.name.endswith("_left"):
                moved = replace(cube, origin=(cube.origin[0] + 0.9, *cube.origin[1:]))
                _replace_gate_cube(r, part.key, index, moved)
                # 断言信息按成对基名报告（side_wrap），因此把同一基名交给
                # gatekit 做命中核验；描述仍保留实际被注入的左件名称。
                return r, cube.name[:-5], f"把 {cube.name} 单侧平移 0.9px"
    raise gatekit.InjectionImpossible("没有参与镜像自检的件")


class _LinenArmorGates(gatekit.AssetGates):
    def specs(self):
        return (
            ("coplanar", "单件共面 / z-fighting",
             lambda r: _gate_violations(r, _assert_no_coplanar_faces), _inject_coplanar),
            ("uv_tiles", "box-UV 越出指定色块",
             lambda r: _gate_violations(r, _assert_uv_tiles), _inject_uv),
            ("mirror", "左右件不镜像",
             lambda r: _gate_violations(r, _assert_mirror_symmetry), _inject_mirror),
        )


GATES = _LinenArmorGates("麻布甲头盔/靴子", GATE_MATS)


def emit_java(part: ArmorPart) -> str:
    """输出 ArmorPartModel.java 格式。"""
    method = "".join(word.capitalize() for word in part.key.split("_"))
    method = method[0].lower() + method[1:]
    lines = [f"    private static List<ArmorCube> {method}() {{", "        return List.of("]
    body = []
    for cube in part.cubes:
        ox, oy, oz = cube.origin
        sx, sy, sz = cube.size
        u, v = cube.uv
        body.append(
            f"            new ArmorCube(Mount.{cube.mount}, "
            f"{ox}f, {oy}f, {oz}f, {sx}f, {sy}f, {sz}f, {u}, {v})"
        )
    lines.append(",\n".join(body))
    lines.append("        );")
    lines.append("    }")
    return "\n".join(lines)


def cube_digest(part: ArmorPart) -> str:
    """复刻 ArmorPartModelTest.cubeDigest 的 FNV-1a，免得为拿 pin 值跑一趟 Java。"""
    import struct

    def fnv1a(hash_value: int, value: int) -> int:
        for _ in range(4):
            hash_value ^= value & 0xFF
            hash_value = (hash_value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
            value >>= 8
        return hash_value

    def bits(f: float) -> int:
        return struct.unpack("<I", struct.pack("<f", f))[0]

    mounts = ["HEAD", "BODY", "LEFT_LEG", "RIGHT_LEG", "LEFT_FOOT", "RIGHT_FOOT"]
    h = 0xCBF29CE484222325
    for cube in part.cubes:
        h = fnv1a(h, mounts.index(cube.mount))
        for value in (*cube.origin, *cube.size):
            h = fnv1a(h, bits(value))
        h = fnv1a(h, cube.uv[0])
        h = fnv1a(h, cube.uv[1])
    return f"{h:016x}"


def generate(render_previews: bool = True, install: bool = False) -> dict[str, Path]:
    _assert_no_coplanar_faces(parts())
    return write_material_assets(
        MATERIAL,
        parts(),
        make_texture(),
        LOCAL_MODELS,
        CLIENT_TEXTURE_ROOT if install else DRAFT_TEXTURE_ROOT,
        PREVIEW_ROOT,
        render_previews,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="生成麻布僧袍套装 3D 资产")
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel/texture")
    parser.add_argument("--emit-java", action="store_true", help="打印 ArmorPartModel 用的 Java 代码")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="gatekit 差分自证：先注入缺陷再确认每道门能报出",
    )
    parser.add_argument("--install", action="store_true", help="写入客户端正式资源目录")
    args = parser.parse_args()

    if args.self_test:
        raise SystemExit(GATES.self_test(build()))

    if args.emit_java:
        for part in parts():
            print(emit_java(part))
            print()
        return

    outputs = generate(render_previews=not args.no_preview, install=args.install)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
