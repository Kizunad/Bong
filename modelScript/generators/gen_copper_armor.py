#!/usr/bin/env python3
"""生成古铜札甲（copper armor）四件 bbmodel、64x64 UV 贴图与真实三视图预览。

Round 1/3 (依据正式参考图重构):
设计与形制依据：
- 概念图: modelScript/assets/refs/ref_copper_armor_concept.png
- 物品图标: modelScript/assets/refs/ref_copper_armor_icon.png
- MC体素三视图: modelScript/assets/refs/ref_copper_armor_three_view.png
- 爆炸分解图: modelScript/assets/refs/ref_copper_armor_exploded.png

核心形制拆解：
1. 铜甲盔 (copper_helmet):
   - 穹顶弧形胄体 (helm_dome_crown) + 正中纵向贯穿金铜脊梁 (helm_crest_ridge) + 顶簪 (helm_crest_spike)
   - 额前横向加厚铜梁 (helm_brow_band)
   - 左右双侧下探铜护耳 (helm_ear_flap_left / right)
   - 脑后三阶叠层下垂护颈帏帘 (helm_neck_curtain_top / mid / low)
2. 铜甲胸甲 (copper_chestplate):
   - 前胸三段式叠片古铜札甲 (chest_front_scales_top / mid / low)
   - 正胸中央圆形凸起古铜护心镜（外圈凸缘 + 中心圆凸镜盘）
   - 双肩二段外挑台阶式披膊护肩 (shoulder_left_tier1/2, shoulder_right_tier1/2)
   - 腰部麻绳/皮绳多圈系腰与绳结挂坠 (waist_cord_belt, waist_cord_knot)
3. 铜甲腿甲 (copper_leggings):
   - 大腿部前/侧分片式叠瓦铜札战裙 (skirt_front / skirt_side)
   - 护膝凸起铜圆泡 (knee_guard)
   - 腿部固定皮带 (thigh_straps)
4. 铜甲靴 (copper_boots):
   - 胫骨纵向铜护梁与甲片 (shin_greave)
   - 脚趾防踢铜包面 (toe_cap)
   - 脚踝皮质系带与加固护跟 (ankle_wrap / heel_plate)

贴图四象限规划 (64x64):
- Q1 (0, 0)   - 主古铜色锻打面 (Hammered Copper，带氧化暗纹与细密札片缝)
- Q2 (32, 0)  - 铜绿氧化层 (Green Patina / Verdigris，绿松石锈斑与青灰蚀痕)
- Q3 (0, 32)  - 熟皮与系带内衬 (Leather & Cord Binding，深棕鞣皮与粗麻绳)
- Q4 (32, 32) - 金黄铜高光饰件 (Brass Crest & Mirror Disc，亮黄铜脊梁、护心镜凸盘与泡钉)
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

from PIL import Image, ImageDraw

# --- modelScript 路径引导：共用底座在 core/ ---
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, TEXTURE_SIZE, write_material_assets

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
PREVIEW_ROOT = Path(__file__).resolve().parents[1] / "out"

MATERIAL = "copper"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 贴图 UV 象限
UV_COPPER_MAIN = (0, 0)
UV_COPPER_PATINA = (32, 0)
UV_LEATHER_CORD = (0, 32)
UV_BRASS_ACCENT = (32, 32)


def c(mount: str, name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv: tuple[int, int] = UV_COPPER_MAIN) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 1. 铜甲盔 (Helmet) ──────────────────────────────────────────────────
def part_helmet() -> ArmorPart:
    """铜甲盔：对照三视图的穹顶胄、贯穿铜脊、护额、双侧护耳及三阶护颈帏帘。"""
    cubes: list[Cube] = [
        # 穹顶顶部与阶梯弧度 (Crown Dome)
        c("HEAD", "helm_dome_crown", (-4.3, 31.8, -4.3), (8.6, 1.2, 8.6), UV_COPPER_MAIN),
        c("HEAD", "helm_dome_tier2", (-3.6, 32.7, -3.6), (7.2, 0.8, 7.2), UV_COPPER_MAIN),

        # 贯穿纵向金铜脊梁与顶簪 (Crest Ridge & Spike)
        c("HEAD", "helm_crest_ridge", (-0.6, 31.8, -4.7), (1.2, 2.0, 9.4), UV_BRASS_ACCENT),
        c("HEAD", "helm_crest_spike", (-0.7, 33.6, -0.7), (1.4, 2.2, 1.4), UV_BRASS_ACCENT),
        c("HEAD", "helm_crest_finial", (-0.4, 35.6, -0.4), (0.8, 0.8, 0.8), UV_BRASS_ACCENT),

        # 头部四面包围甲身 (Helmet Walls)
        c("HEAD", "helm_wall_front", (-4.6, 28.5, -4.7), (9.2, 3.4, 0.9), UV_COPPER_MAIN),
        c("HEAD", "helm_wall_back", (-4.6, 25.0, 3.8), (9.2, 7.0, 0.9), UV_COPPER_MAIN),
        c("HEAD", "helm_wall_left", (-4.75, 25.0, -3.8), (0.9, 7.0, 7.6), UV_COPPER_MAIN),
        c("HEAD", "helm_wall_right", (3.85, 25.0, -3.8), (0.9, 7.0, 7.6), UV_COPPER_MAIN),

        # 护额加厚铜梁 (Brow Band)
        c("HEAD", "helm_brow_band", (-4.8, 29.2, -4.95), (9.6, 1.6, 0.8), UV_BRASS_ACCENT),

        # 双侧下探铜护耳 (Ear Flaps) - 对应分解图的左右铜护耳
        c("HEAD", "helm_ear_flap_left", (-4.95, 23.6, -4.2), (1.1, 5.0, 2.4), UV_COPPER_PATINA),
        c("HEAD", "helm_ear_flap_right", (3.85, 23.6, -4.2), (1.1, 5.0, 2.4), UV_COPPER_PATINA),

        # 脑后三层阶梯下垂护颈帏帘 (Neck Curtains - 三阶叠片)
        c("HEAD", "helm_neck_curtain_top", (-4.7, 24.2, 3.9), (9.4, 1.8, 0.9), UV_COPPER_MAIN),
        c("HEAD", "helm_neck_curtain_mid", (-4.85, 22.6, 4.05), (9.7, 1.8, 0.9), UV_COPPER_PATINA),
        c("HEAD", "helm_neck_curtain_low", (-5.0, 21.0, 4.2), (10.0, 1.8, 0.9), UV_COPPER_MAIN),

        # 泡钉细节 (Brass Rivets)
        c("HEAD", "helm_rivet_brow_l", (-3.2, 29.6, -5.15), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("HEAD", "helm_rivet_brow_r", (2.6, 29.6, -5.15), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("HEAD", "helm_rivet_ear_l", (-5.05, 27.8, -3.0), (0.35, 0.6, 0.6), UV_BRASS_ACCENT),
        c("HEAD", "helm_rivet_ear_r", (4.7, 27.8, -3.0), (0.35, 0.6, 0.6), UV_BRASS_ACCENT),
    ]
    return ArmorPart("copper_helmet", "COPPER HELMET", tuple(cubes))


# ─── 2. 铜甲胸甲 (Chestplate) ─────────────────────────────────────────────
def part_chestplate() -> ArmorPart:
    """铜甲胸甲：三段式铜札身甲 + 凸起金铜护心镜 + 二阶披膊护肩 + 腰绳。"""
    cubes: list[Cube] = [
        # 前胸三段层叠札片 (Front Scales: Top, Mid, Low)
        c("BODY", "chest_front_scales_top", (-4.3, 18.2, -2.75), (8.6, 5.8, 0.8), UV_COPPER_MAIN),
        c("BODY", "chest_front_scales_mid", (-4.1, 14.8, -2.85), (8.2, 3.6, 0.85), UV_COPPER_PATINA),
        c("BODY", "chest_front_scales_low", (-3.8, 12.2, -2.75), (7.6, 2.8, 0.8), UV_COPPER_MAIN),

        # 正胸中央古铜护心镜 (Heart Mirror: Frame & Disc)
        c("BODY", "chest_heart_mirror_frame", (-1.8, 18.2, -3.2), (3.6, 3.6, 0.65), UV_BRASS_ACCENT),
        c("BODY", "chest_heart_mirror_disc", (-1.3, 18.7, -3.5), (2.6, 2.6, 0.45), UV_BRASS_ACCENT),

        # 后背札甲板 (Back Scales)
        c("BODY", "chest_back_main", (-4.2, 17.6, 1.95), (8.4, 6.4, 0.8), UV_COPPER_MAIN),
        c("BODY", "chest_back_lower", (-3.9, 12.4, 1.95), (7.8, 5.4, 0.75), UV_COPPER_PATINA),

        # 侧肋皮革与札带 (Rib Rails)
        c("BODY", "chest_side_left", (-4.55, 13.8, -1.8), (0.7, 9.2, 3.6), UV_LEATHER_CORD),
        c("BODY", "chest_side_right", (3.85, 13.8, -1.8), (0.7, 9.2, 3.6), UV_LEATHER_CORD),

        # 领口护颈与皮包边 (Collar Linings)
        c("BODY", "chest_collar_left", (-4.4, 22.6, -2.9), (3.6, 1.2, 0.9), UV_BRASS_ACCENT),
        c("BODY", "chest_collar_right", (0.8, 22.6, -2.9), (3.6, 1.2, 0.9), UV_BRASS_ACCENT),

        # 双肩二段外挑披膊护肩 (Shoulder Pauldrons: Tier 1 & Tier 2)
        # 左肩
        c("BODY", "shoulder_left_tier1", (-6.1, 21.6, -2.6), (2.2, 2.2, 5.2), UV_COPPER_MAIN),
        c("BODY", "shoulder_left_tier2", (-6.4, 22.6, -2.7), (2.4, 1.0, 5.4), UV_COPPER_PATINA),
        c("BODY", "shoulder_left_strap", (-4.8, 21.0, -2.8), (0.9, 3.0, 0.4), UV_LEATHER_CORD),
        # 右肩
        c("BODY", "shoulder_right_tier1", (3.9, 21.6, -2.6), (2.2, 2.2, 5.2), UV_COPPER_MAIN),
        c("BODY", "shoulder_right_tier2", (4.0, 22.6, -2.7), (2.4, 1.0, 5.4), UV_COPPER_PATINA),
        c("BODY", "shoulder_right_strap", (3.9, 21.0, -2.8), (0.9, 3.0, 0.4), UV_LEATHER_CORD),

        # 腰部麻绳系带与绳结 (Waist Cord Belt & Knot) - 对照三视图正中绳结
        c("BODY", "waist_cord_belt", (-4.2, 11.6, -2.85), (8.4, 1.1, 5.7), UV_LEATHER_CORD),
        c("BODY", "waist_cord_knot", (-0.9, 11.0, -3.2), (1.8, 1.8, 0.6), UV_LEATHER_CORD),
        c("BODY", "waist_cord_tail_l", (-0.8, 9.6, -3.1), (0.6, 1.6, 0.4), UV_LEATHER_CORD),
        c("BODY", "waist_cord_tail_r", (0.2, 9.6, -3.1), (0.6, 1.6, 0.4), UV_LEATHER_CORD),

        # 四枚定位铜泡钉
        c("BODY", "chest_rivet_tl", (-3.4, 21.6, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("BODY", "chest_rivet_tr", (2.8, 21.6, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("BODY", "chest_rivet_bl", (-3.0, 13.8, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("BODY", "chest_rivet_br", (2.4, 13.8, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
    ]
    return ArmorPart("copper_chestplate", "COPPER CHESTPLATE", tuple(cubes))


# ─── 3. 铜甲腿甲 (Leggings) ───────────────────────────────────────────────
def _leg_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        # 大腿前侧与外侧战裙铜札 (Skirt Plates: Front & Side)
        c(mount, f"{prefix}_skirt_front", (-2.2, 9.4, -2.8), (4.4, 2.8, 0.85), UV_COPPER_MAIN),
        c(mount, f"{prefix}_skirt_side", (outer_x, 8.8, -2.2), (0.75, 3.4, 4.4), UV_COPPER_PATINA),
        # 大腿中段铜札片
        c(mount, f"{prefix}_thigh_mid_plate", (-1.9, 6.4, -2.65), (3.8, 3.2, 0.75), UV_COPPER_MAIN),
        # 护膝凸起铜圆泡 (Knee Guard & Boss)
        c(mount, f"{prefix}_knee_guard", (-2.1, 3.0, -2.9), (4.2, 2.0, 0.95), UV_COPPER_MAIN),
        c(mount, f"{prefix}_knee_boss", (-0.7, 3.4, -3.2), (1.4, 1.4, 0.45), UV_BRASS_ACCENT),
        # 腿部外侧护梁
        c(mount, f"{prefix}_outer_rail", (outer_x, 4.4, -1.9), (0.65, 6.8, 1.2), UV_COPPER_MAIN),
        # 后侧绑扎皮带 (Thigh Straps)
        c(mount, f"{prefix}_thigh_straps", (-2.0, 8.6, 1.95), (4.0, 0.7, 0.45), UV_LEATHER_CORD),
        c(mount, f"{prefix}_knee_strap", (-2.0, 3.8, 1.95), (4.0, 0.7, 0.45), UV_LEATHER_CORD),
        # 护膝泡钉
        c(mount, f"{prefix}_knee_rivet", (-0.3, 10.4, -2.95), (0.6, 0.6, 0.3), UV_BRASS_ACCENT),
    )


def part_leggings() -> ArmorPart:
    """铜甲腿甲：分片式下摆战裙 + 护膝铜镜 + 绑扎皮带。"""
    return ArmorPart(
        "copper_leggings",
        "COPPER LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.65) + _leg_cubes("RIGHT_LEG", -2.3),
    )


# ─── 4. 铜甲靴 (Boots) ───────────────────────────────────────────────────
def _boot_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        # 胫骨正面铜护板 (Shin Greave)
        c(mount, f"{prefix}_shin_greave", (-2.05, 2.0, -2.7), (4.1, 3.8, 0.8), UV_COPPER_MAIN),
        c(mount, f"{prefix}_shin_crest", (-0.5, 2.2, -2.9), (1.0, 3.4, 0.35), UV_BRASS_ACCENT),
        # 脚趾防踢铜包头 (Toe Cap)
        c(mount, f"{prefix}_toe_cap", (-2.25, 0.0, -3.15), (4.5, 1.8, 1.3), UV_COPPER_MAIN),
        c(mount, f"{prefix}_toe_rivet", (-0.35, 0.7, -3.35), (0.7, 0.7, 0.35), UV_BRASS_ACCENT),
        # 脚踝皮带环绕 (Ankle Wrap)
        c(mount, f"{prefix}_ankle_wrap", (-2.15, 3.8, -2.75), (4.3, 0.75, 5.5), UV_LEATHER_CORD),
        c(mount, f"{prefix}_ankle_buckle", (outer_x - 0.1, 3.7, -0.6), (0.45, 0.9, 1.2), UV_BRASS_ACCENT),
        # 后跟铜护片 (Heel Plate)
        c(mount, f"{prefix}_heel_plate", (-2.05, 0.4, 1.95), (4.1, 2.2, 0.75), UV_COPPER_PATINA),
        # 靴底包边 (Sole Edge)
        c(mount, f"{prefix}_sole_edge", (-2.3, -0.2, -3.2), (4.6, 0.45, 5.4), UV_LEATHER_CORD),
    )


def part_boots() -> ArmorPart:
    """铜甲靴：铜包头 + 胫骨梁 + 脚踝绑带 + 铜护跟。"""
    return ArmorPart(
        "copper_boots",
        "COPPER BOOTS",
        _boot_cubes("LEFT_FOOT", 1.65) + _boot_cubes("RIGHT_FOOT", -2.3),
    )


def parts() -> tuple[ArmorPart, ...]:
    return part_helmet(), part_chestplate(), part_leggings(), part_boots()


# ─── 贴图生成 (64x64) ─────────────────────────────────────────────────────
def make_texture() -> Image.Image:
    """基于三视图与概念图生成的 64×64 像素古铜扎甲贴图。"""
    rng = random.Random(0x2B0B)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (184, 115, 51))
    pixels = image.load()

    # 象限底色定义
    # Q1: 古铜色 #A86430 (168, 100, 48)
    # Q2: 铜绿氧化斑 #42826E (66, 130, 110)
    # Q3: 深熟皮与绑绳 #503624 (80, 54, 36)
    # Q4: 黄铜脊梁与护心镜 #DCA44E (220, 164, 78)
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                base = (168, 100, 48)      # Q1 锻打古铜
            elif x >= 32 and y < 32:
                base = (66, 130, 110)      # Q2 铜绿氧化
            elif x < 32 and y >= 32:
                base = (80, 54, 36)        # Q3 熟皮内衬与绳索
            else:
                base = (220, 164, 78)      # Q4 亮黄铜配件
            jitter = rng.randint(-12, 12)
            pixels[x, y] = tuple(max(0, min(255, channel + jitter)) for channel in base)

    draw = ImageDraw.Draw(image)

    # Q1 扎片细密横竖缝线与凹痕
    for y_seam in range(4, 32, 6):
        draw.line((2, y_seam, 30, y_seam), fill=(120, 68, 30), width=1)
    for x, y in ((4, 7), (12, 13), (22, 19), (14, 25), (6, 28)):
        draw.point((x, y), fill=(210, 140, 75))

    # Q2 铜绿与氧化渐变斑驳 (Verdigris Patina)
    for x, y in ((36, 6), (44, 12), (52, 8), (58, 20), (38, 24), (48, 26)):
        draw.rectangle((x, y, x + 3, y + 2), fill=(50, 108, 90))
        draw.point((x + 1, y + 1), fill=(95, 165, 140))

    # Q3 粗麻绳编织与皮革缝纫线
    for y_line in (38, 46, 54):
        draw.line((2, y_line, 30, y_line), fill=(44, 28, 18), width=1)
    for x in range(4, 30, 3):
        draw.point((x, 38), fill=(130, 95, 65))
        draw.point((x + 1, 46), fill=(130, 95, 65))

    # Q4 护心镜放射纹与同心圆高光 (Mirror Accents)
    for cx, cy in ((42, 42), (54, 42), (42, 54), (54, 54)):
        draw.rectangle((cx - 1, cy - 1, cx + 1, cy + 1), fill=(245, 200, 115))
        draw.point((cx, cy), fill=(255, 240, 185))

    return image


def generate(render_previews: bool = True, install: bool = False) -> dict[str, Path]:
    tex_dir = CLIENT_TEXTURE_ROOT if install else DRAFT_TEXTURE_ROOT
    outputs = write_material_assets(
        "copper",
        parts(),
        make_texture(),
        LOCAL_MODELS,
        tex_dir,
        PREVIEW_ROOT,
        render_previews,
    )
    return outputs


def main() -> None:
    parser = argparse.ArgumentParser(description="生成铜甲四件套 bbmodel、贴图与预览")
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel/texture，不渲染预览")
    parser.add_argument("--install", action="store_true", help="写入 client 正式贴图目录")
    args = parser.parse_args()

    outputs = generate(render_previews=not args.no_preview, install=args.install)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
