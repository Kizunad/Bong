#!/usr/bin/env python3
"""生成麻布僧袍（linen armor）胸甲与护腿 bbmodel、64x64 UV 贴图与真实三视图预览。

配方对应粗布（rough_cloth）+ 草绳（grass_rope / dried_grass）+ 异变兽骨环扣（bone_chip_mat）。
本套装聚焦在【胸甲 (chestplate)】与【护腿 (leggings)】：
- 胸甲：交领右衽僧袍衣身、单肩斜披搭褡（偏衫）、兽骨环扣、多圈麻绳束腰、双臂麻布绑带（绑臂）。
- 护腿：分衩式中长僧袍下摆（随双腿各自运动，杜绝跨腿穿模撕裂）、宽松苦修麻裤、小腿十字交叉绑腿（行缠）。

运行时真相是 client 的 ArmorPartModel.CUBE_TABLES，本文件的 --emit-java
直接吐那张表的 Java 字面量。
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
    return (part_chestplate(), part_leggings())


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
    parser.add_argument("--install", action="store_true", help="写入客户端正式资源目录")
    args = parser.parse_args()

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
