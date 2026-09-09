#!/usr/bin/env python3
"""生成古铜札甲（copper armor）四件 bbmodel、64x64 UV 贴图与真实三视图预览。

Round 2/3 (终极重构版):
严格依据 AI 概念图 (ref_copper_armor_concept.png)、MC体素三视图 (ref_copper_armor_three_view.png)
与技术爆炸分解图 (ref_copper_armor_exploded.png) 进行 1:1 解剖级体素精雕重做。

关键视觉语言突破：
1. 铜甲盔 (copper_helmet):
   - 弯钩雀冠顶簪与金脊贯穿：前探至眉梁、向上后仰翘起、后延至脑后
   - 护额梁下探中央鼻遮
   - 双颊贴合护颊片 (cheek guards)
   - 脑后五阶扇形外展长款护颈帏帘 (5-Tier Neck Curtains)，向下覆至背甲上部
2. 铜甲胸甲 (copper_chestplate):
   - 前胸四段层叠长方铜札 + 侧肋加厚护板
   - 正胸硕大金铜护心镜 (凸起同心金圈 + 浑厚圆球铜泡 + 顶部挂纽)
   - 双肩四阶梯级向外倾斜下延的披膊大护肩 (4-Tier Flared Pauldrons)，外挑防砍
   - 腰部多股粗麻紧固系带，中轴垂挂双段结与流苏挂坠
3. 铜甲腿甲 (copper_leggings):
   - 大腿部前中/两侧/后部四段叠片分片式长战裙 (4-Tier Lamellar Fauld/Skirt)，下缘带鲜明铜绿包边
   - 膝盖立体金铜圆凸镜护膝 (Knee Mirror Boss)
   - 左右各附双层后侧绑腿皮扣
4. 铜甲靴 (copper_boots):
   - 小腿前中隆起金铜护胫梁 + 左右护片
   - 阶梯式重装包铜防踢鞋头 (Tiered Toe Caps) 与铜泡钉
   - 脚踝深皮收紧带与加固厚铜后跟
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

# 贴图 UV 象限 (64x64)
# Q1: 古铜札片 (深红褐铜底 + 札片暗缝)
# Q2: 青绿铜锈 Verdigris (绿松石氧化斑 + 腐蚀凹坑)
# Q3: 熟皮与粗麻绳 (深褐鞣皮 + 绳结纹理)
# Q4: 亮金黄铜 (护心镜盘 + 弯钩脊梁 + 泡钉)
UV_COPPER_MAIN = (0, 0)
UV_COPPER_PATINA = (32, 0)
UV_LEATHER_CORD = (0, 32)
UV_BRASS_ACCENT = (32, 32)


def c(mount: str, name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv: tuple[int, int] = UV_COPPER_MAIN) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 1. 铜甲盔 (Helmet) ──────────────────────────────────────────────────
def part_helmet() -> ArmorPart:
    """铜甲盔：对照三视图的穹顶胄、弯钩雀冠、眉额梁、鼻遮、护颊及五阶扇形护颈帏帘。"""
    cubes: list[Cube] = [
        # ── 穹顶梯级收缩圆胄冠 (Crown Dome) ──
        c("HEAD", "helm_dome_base", (-4.3, 31.4, -4.3), (8.6, 1.2, 8.6), UV_COPPER_MAIN),
        c("HEAD", "helm_dome_tier2", (-3.6, 32.5, -3.6), (7.2, 1.0, 7.2), UV_COPPER_MAIN),
        c("HEAD", "helm_dome_tier3", (-2.6, 33.4, -2.6), (5.2, 0.8, 5.2), UV_COPPER_MAIN),

        # ── 贯穿金铜雀冠脊梁与后仰弯钩顶簪 (Dragon Crest & Hook Finial) ──
        # 额前立梁与纵向贯穿大脊
        c("HEAD", "helm_crest_ridge", (-0.6, 32.2, -4.6), (1.2, 2.2, 9.0), UV_BRASS_ACCENT),
        # 额前立柱护梁
        c("HEAD", "helm_crest_front_pillar", (-0.6, 28.5, -5.1), (1.2, 3.8, 0.8), UV_BRASS_ACCENT),
        # 顶端弯钩簪钮 (弯折雀首)
        c("HEAD", "helm_crest_spike", (-0.7, 34.0, -0.9), (1.4, 1.8, 1.8), UV_BRASS_ACCENT),
        c("HEAD", "helm_crest_hook_rise", (-0.5, 35.6, -0.4), (1.0, 1.6, 1.2), UV_BRASS_ACCENT),
        c("HEAD", "helm_crest_hook_tip", (-0.5, 36.4, 0.4), (1.0, 0.8, 1.2), UV_BRASS_ACCENT),

        # ── 头部包围基础盔体 (Helmet Body Walls) ──
        c("HEAD", "helm_wall_front", (-4.55, 28.5, -4.75), (9.1, 3.2, 0.9), UV_COPPER_MAIN),
        c("HEAD", "helm_wall_back", (-4.55, 25.0, 3.85), (9.1, 6.8, 0.9), UV_COPPER_MAIN),
        c("HEAD", "helm_wall_left", (-4.75, 25.0, -3.8), (0.9, 6.8, 7.6), UV_COPPER_MAIN),
        c("HEAD", "helm_wall_right", (3.85, 25.0, -3.8), (0.9, 6.8, 7.6), UV_COPPER_MAIN),

        # ── 眉额金铜横梁与下探鼻遮 (Brow Band & Nose Guard) ──
        c("HEAD", "helm_brow_band", (-4.8, 28.8, -4.95), (9.6, 1.8, 0.85), UV_BRASS_ACCENT),
        c("HEAD", "helm_nose_guard", (-0.45, 25.4, -5.25), (0.9, 3.6, 0.6), UV_BRASS_ACCENT),

        # ── 双颊贴合铜护耳 (Ear/Cheek Guards) ──
        c("HEAD", "helm_ear_flap_left", (-4.95, 23.4, -4.2), (1.1, 5.4, 2.6), UV_COPPER_MAIN),
        c("HEAD", "helm_ear_flap_right", (3.85, 23.4, -4.2), (1.1, 5.4, 2.6), UV_COPPER_MAIN),

        # ── 脑后五阶扇形外展长款护颈帏帘 (5-Tier Flared Neck Curtains) ──
        c("HEAD", "helm_neck_curtain_tier1", (-4.7, 26.2, 4.0), (9.4, 1.8, 0.85), UV_COPPER_MAIN),
        c("HEAD", "helm_neck_curtain_tier2", (-4.9, 24.4, 4.2), (9.8, 1.9, 0.9), UV_COPPER_MAIN),
        c("HEAD", "helm_neck_curtain_tier3", (-5.15, 22.5, 4.45), (10.3, 2.0, 0.95), UV_COPPER_PATINA),
        c("HEAD", "helm_neck_curtain_tier4", (-5.4, 20.6, 4.7), (10.8, 2.0, 1.0), UV_COPPER_MAIN),
        c("HEAD", "helm_neck_curtain_tier5", (-5.65, 18.7, 4.95), (11.3, 2.0, 1.05), UV_COPPER_PATINA),

        # ── 铜泡钉细节 ──
        c("HEAD", "helm_rivet_brow_l", (-3.2, 29.4, -5.15), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("HEAD", "helm_rivet_brow_r", (2.6, 29.4, -5.15), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("HEAD", "helm_rivet_ear_l", (-5.05, 26.0, -3.0), (0.35, 0.6, 0.6), UV_BRASS_ACCENT),
        c("HEAD", "helm_rivet_ear_r", (4.7, 26.0, -3.0), (0.35, 0.6, 0.6), UV_BRASS_ACCENT),
    ]
    return ArmorPart("copper_helmet", "COPPER HELMET", tuple(cubes))


# ─── 2. 铜甲胸甲 (Chestplate) ─────────────────────────────────────────────
def part_chestplate() -> ArmorPart:
    """铜甲胸甲：四阶铜札身甲 + 硕大护心镜 + 四阶披膊大肩甲 + 麻绳腰结。"""
    cubes: list[Cube] = [
        # ── 前胸四排层叠铜札 (Front Scales: Tier 1 to 4) ──
        c("BODY", "chest_scale_row1", (-4.3, 20.8, -2.75), (8.6, 2.8, 0.8), UV_COPPER_MAIN),
        c("BODY", "chest_scale_row2", (-4.2, 17.8, -2.85), (8.4, 3.2, 0.85), UV_COPPER_MAIN),
        c("BODY", "chest_scale_row3", (-4.0, 14.8, -2.85), (8.0, 3.2, 0.85), UV_COPPER_PATINA),
        c("BODY", "chest_scale_row4", (-3.8, 12.0, -2.75), (7.6, 3.0, 0.8), UV_COPPER_MAIN),

        # ── 正胸中央硕大金铜护心镜 (Large Heart Mirror Disc & Center Boss) ──
        # 外层加固大圆盘 (4.4 x 4.4)
        c("BODY", "chest_heart_mirror_frame", (-2.2, 17.4, -3.2), (4.4, 4.4, 0.65), UV_BRASS_ACCENT),
        # 中层突起环面 (3.2 x 3.2)
        c("BODY", "chest_heart_mirror_disc", (-1.6, 18.0, -3.55), (3.2, 3.2, 0.5), UV_BRASS_ACCENT),
        # 核心球形凸起铜泡 (1.6 x 1.6)
        c("BODY", "chest_mirror_center_boss", (-0.8, 18.8, -3.9), (1.6, 1.6, 0.45), UV_BRASS_ACCENT),
        # 顶部护心镜吊环
        c("BODY", "chest_mirror_top_clasp", (-0.5, 21.8, -3.2), (1.0, 0.8, 0.5), UV_BRASS_ACCENT),

        # ── 后背四段札甲板 (Back Scales) ──
        c("BODY", "chest_back_row1", (-4.2, 20.4, 1.95), (8.4, 3.2, 0.8), UV_COPPER_MAIN),
        c("BODY", "chest_back_row2", (-4.1, 17.2, 2.0), (8.2, 3.4, 0.8), UV_COPPER_MAIN),
        c("BODY", "chest_back_row3", (-3.9, 14.2, 2.05), (7.8, 3.2, 0.8), UV_COPPER_PATINA),
        c("BODY", "chest_back_row4", (-3.8, 12.0, 1.95), (7.6, 2.4, 0.75), UV_COPPER_MAIN),

        # ── 侧肋皮革连接带 (Rib Rails) ──
        c("BODY", "chest_side_rail_left", (-4.55, 13.0, -1.8), (0.75, 10.0, 3.6), UV_LEATHER_CORD),
        c("BODY", "chest_side_rail_right", (3.8, 13.0, -1.8), (0.75, 10.0, 3.6), UV_LEATHER_CORD),

        # ── 领口皮包边与护肩垫层 (Collar Linings) ──
        c("BODY", "chest_collar_left", (-4.4, 22.8, -2.9), (3.6, 1.2, 0.9), UV_LEATHER_CORD),
        c("BODY", "chest_collar_right", (0.8, 22.8, -2.9), (3.6, 1.2, 0.9), UV_LEATHER_CORD),

        # ── 双肩四层外挑叠瓦披膊大肩甲 (4-Tier Flared Pauldrons) ──
        # 左肩 4 阶向外向下覆叠
        c("BODY", "shoulder_left_tier1", (-6.2, 22.4, -2.7), (2.4, 1.8, 5.4), UV_COPPER_MAIN),
        c("BODY", "shoulder_left_tier2", (-6.55, 20.8, -2.6), (2.35, 1.8, 5.2), UV_COPPER_MAIN),
        c("BODY", "shoulder_left_tier3", (-6.85, 19.0, -2.5), (2.2, 1.9, 5.0), UV_COPPER_PATINA),
        c("BODY", "shoulder_left_tier4", (-7.1, 17.2, -2.4), (2.1, 1.9, 4.8), UV_COPPER_PATINA),
        c("BODY", "shoulder_left_strap", (-4.9, 21.0, -2.85), (0.9, 3.2, 0.4), UV_LEATHER_CORD),
        # 右肩 4 阶向外向下覆叠
        c("BODY", "shoulder_right_tier1", (3.8, 22.4, -2.7), (2.4, 1.8, 5.4), UV_COPPER_MAIN),
        c("BODY", "shoulder_right_tier2", (4.2, 20.8, -2.6), (2.35, 1.8, 5.2), UV_COPPER_MAIN),
        c("BODY", "shoulder_right_tier3", (4.65, 19.0, -2.5), (2.2, 1.9, 5.0), UV_COPPER_PATINA),
        c("BODY", "shoulder_right_tier4", (5.0, 17.2, -2.4), (2.1, 1.9, 4.8), UV_COPPER_PATINA),
        c("BODY", "shoulder_right_strap", (4.0, 21.0, -2.85), (0.9, 3.2, 0.4), UV_LEATHER_CORD),

        # ── 腰部麻绳系带与中轴双垂结 (Waist Cord, Knot & Tassels) ──
        c("BODY", "waist_cord_belt", (-4.2, 11.4, -2.9), (8.4, 1.3, 5.8), UV_LEATHER_CORD),
        c("BODY", "waist_cord_knot", (-1.1, 10.6, -3.3), (2.2, 2.0, 0.7), UV_LEATHER_CORD),
        c("BODY", "waist_cord_tassel_l", (-0.9, 8.8, -3.15), (0.65, 2.0, 0.4), UV_LEATHER_CORD),
        c("BODY", "waist_cord_tassel_r", (0.25, 8.8, -3.15), (0.65, 2.0, 0.4), UV_LEATHER_CORD),

        # ── 扎甲四角定位铜泡钉 ──
        c("BODY", "chest_rivet_tl", (-3.5, 21.6, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("BODY", "chest_rivet_tr", (2.9, 21.6, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("BODY", "chest_rivet_bl", (-3.1, 13.4, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
        c("BODY", "chest_rivet_br", (2.5, 13.4, -2.95), (0.6, 0.6, 0.35), UV_BRASS_ACCENT),
    ]
    return ArmorPart("copper_chestplate", "COPPER CHESTPLATE", tuple(cubes))


# ─── 3. 铜甲腿甲 (Leggings) ───────────────────────────────────────────────
def _leg_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    sign = 1.0 if outer_x > 0 else -1.0
    return (
        # ── 大腿正面四阶长款叠瓦战裙 (4-Tier Front Skirt Fauld) ──
        c(mount, f"{prefix}_skirt_f_tier1", (-2.2, 9.6, -2.8), (4.4, 2.6, 0.85), UV_COPPER_MAIN),
        c(mount, f"{prefix}_skirt_f_tier2", (-2.25, 7.3, -2.9), (4.5, 2.5, 0.85), UV_COPPER_MAIN),
        c(mount, f"{prefix}_skirt_f_tier3", (-2.3, 5.0, -3.0), (4.6, 2.5, 0.85), UV_COPPER_PATINA),
        c(mount, f"{prefix}_skirt_f_tier4", (-2.35, 2.8, -3.1), (4.7, 2.4, 0.9), UV_COPPER_PATINA),

        # ── 大腿外侧四阶战裙 (4-Tier Side Skirt) ──
        c(mount, f"{prefix}_skirt_s_tier1", (outer_x, 9.2, -2.4), (0.8, 2.8, 4.8), UV_COPPER_MAIN),
        c(mount, f"{prefix}_skirt_s_tier2", (outer_x + sign * 0.1, 6.6, -2.4), (0.8, 2.8, 4.8), UV_COPPER_MAIN),
        c(mount, f"{prefix}_skirt_s_tier3", (outer_x + sign * 0.2, 4.0, -2.4), (0.85, 2.8, 4.8), UV_COPPER_PATINA),
        c(mount, f"{prefix}_skirt_s_tier4", (outer_x + sign * 0.3, 1.4, -2.4), (0.9, 2.8, 4.8), UV_COPPER_PATINA),

        # ── 大腿后侧双阶战裙 (Back Skirt) ──
        c(mount, f"{prefix}_skirt_b_tier1", (-2.1, 7.4, 2.0), (4.2, 4.8, 0.8), UV_COPPER_MAIN),
        c(mount, f"{prefix}_skirt_b_tier2", (-2.2, 3.4, 2.1), (4.4, 4.2, 0.85), UV_COPPER_PATINA),

        # ── 护膝金铜圆凸镜 (Knee Mirror Boss) ──
        c(mount, f"{prefix}_knee_guard", (-2.1, 3.0, -2.9), (4.2, 2.0, 0.95), UV_COPPER_MAIN),
        c(mount, f"{prefix}_knee_rim", (-1.8, 3.2, -3.25), (3.6, 2.4, 0.7), UV_BRASS_ACCENT),
        c(mount, f"{prefix}_knee_boss", (-0.9, 3.7, -3.6), (1.8, 1.4, 0.45), UV_BRASS_ACCENT),

        # ── 腿部固定皮带 (Leg Straps) ──
        c(mount, f"{prefix}_thigh_strap_top", (-2.05, 9.0, 1.95), (4.1, 0.8, 0.45), UV_LEATHER_CORD),
        c(mount, f"{prefix}_thigh_strap_bot", (-2.05, 4.2, 1.95), (4.1, 0.8, 0.45), UV_LEATHER_CORD),
    )


def part_leggings() -> ArmorPart:
    """铜甲腿甲：四阶叠瓦中长分片战裙 + 护膝金铜圆镜 + 束腿皮带。"""
    return ArmorPart(
        "copper_leggings",
        "COPPER LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.65) + _leg_cubes("RIGHT_LEG", -2.45),
    )


# ─── 4. 铜甲靴 (Boots) ───────────────────────────────────────────────────
def _boot_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    sign = 1.0 if outer_x > 0 else -1.0
    return (
        # ── 胫骨正面铜护板 (Shin Greave) 与纵向金梁 ──
        c(mount, f"{prefix}_shin_plate_upper", (-2.1, 3.8, -2.75), (4.2, 2.4, 0.85), UV_COPPER_MAIN),
        c(mount, f"{prefix}_shin_plate_lower", (-2.05, 1.6, -2.8), (4.1, 2.4, 0.85), UV_COPPER_PATINA),
        c(mount, f"{prefix}_shin_crest_ridge", (-0.5, 1.6, -3.05), (1.0, 4.4, 0.35), UV_BRASS_ACCENT),

        # ── 阶梯式三段包铜防踢鞋头 (Tiered Toe Caps) ──
        c(mount, f"{prefix}_toe_cap_mid", (-2.25, 0.0, -3.3), (4.5, 2.0, 1.4), UV_COPPER_MAIN),
        c(mount, f"{prefix}_toe_tip_metal", (-2.05, 0.0, -3.65), (4.1, 1.2, 0.5), UV_BRASS_ACCENT),
        c(mount, f"{prefix}_toe_rivet", (-0.35, 0.7, -3.85), (0.7, 0.7, 0.35), UV_BRASS_ACCENT),

        # ── 脚踝皮革束带与环扣 (Ankle Wrap & Buckle) ──
        c(mount, f"{prefix}_ankle_strap", (-2.15, 3.6, -2.75), (4.3, 0.8, 5.5), UV_LEATHER_CORD),
        c(mount, f"{prefix}_ankle_buckle", (outer_x + sign * 0.1, 3.5, -0.6), (0.45, 1.0, 1.2), UV_BRASS_ACCENT),

        # ── 加固厚铜后跟 (Heel Plate) ──
        c(mount, f"{prefix}_heel_plate", (-2.1, 0.4, 1.95), (4.2, 2.6, 0.85), UV_COPPER_PATINA),

        # ── 耐磨靴底边 (Sole Edge) ──
        c(mount, f"{prefix}_sole_edge", (-2.3, -0.25, -3.5), (4.6, 0.5, 5.8), UV_LEATHER_CORD),
    )


def part_boots() -> ArmorPart:
    """铜甲靴：铜包头 + 胫骨梁 + 脚踝皮带 + 铜护跟。"""
    return ArmorPart(
        "copper_boots",
        "COPPER BOOTS",
        _boot_cubes("LEFT_FOOT", 1.65) + _boot_cubes("RIGHT_FOOT", -2.45),
    )


def parts() -> tuple[ArmorPart, ...]:
    return part_helmet(), part_chestplate(), part_leggings(), part_boots()


# ─── 贴图生成 (64x64) ─────────────────────────────────────────────────────
def make_texture() -> Image.Image:
    """基于三视图与概念图生成的 64×64 像素古铜扎甲贴图。"""
    rng = random.Random(0x2B0B)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (154, 76, 46))
    pixels = image.load()

    # 象限底色定义
    # Q1: 古铜色 #8C4B28 (140, 75, 40)
    # Q2: 铜绿氧化斑 #3E8A78 (62, 138, 120)
    # Q3: 深熟皮与绑绳 #42281A (66, 40, 26)
    # Q4: 黄铜高光 #E2B258 (226, 178, 88)
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                base = (140, 75, 40)       # Q1 锻打红铜
            elif x >= 32 and y < 32:
                base = (62, 138, 120)      # Q2 青绿铜锈
            elif x < 32 and y >= 32:
                base = (66, 40, 26)        # Q3 熟皮内衬与绳索
            else:
                base = (226, 178, 88)      # Q4 亮黄铜配件与护心镜
            jitter = rng.randint(-12, 12)
            pixels[x, y] = tuple(max(0, min(255, channel + jitter)) for channel in base)

    draw = ImageDraw.Draw(image)

    # Q1 扎片细密横竖叠压暗线与锤击点
    for y_seam in range(3, 32, 5):
        draw.line((1, y_seam, 30, y_seam), fill=(88, 42, 22), width=1)
        draw.line((1, y_seam + 1, 30, y_seam + 1), fill=(180, 105, 60), width=1)
    for x_seam in range(4, 32, 6):
        draw.line((x_seam, 1, x_seam, 30), fill=(88, 42, 22), width=1)

    # Q2 铜绿氧化斑驳与深浅腐蚀凹痕 (Patina & Verdigris)
    for x, y in ((35, 5), (42, 10), (50, 6), (57, 18), (38, 22), (48, 26)):
        draw.rectangle((x, y, x + 4, y + 3), fill=(46, 110, 94))
        draw.point((x + 1, y + 1), fill=(95, 175, 155))
        draw.point((x + 2, y + 2), fill=(125, 205, 185))

    # Q3 粗麻绳编织与皮革紧密缝纫线
    for y_line in (36, 44, 52, 60):
        draw.line((2, y_line, 30, y_line), fill=(40, 22, 14), width=1)
    for x in range(3, 30, 4):
        draw.point((x, 36), fill=(120, 85, 55))
        draw.point((x + 2, 44), fill=(120, 85, 55))
        draw.point((x, 52), fill=(120, 85, 55))

    # Q4 护心镜同心圆放射齿纹与高光 (Mirror Accents)
    for cx, cy in ((40, 40), (54, 40), (40, 54), (54, 54)):
        draw.rectangle((cx - 2, cy - 2, cx + 2, cy + 2), fill=(245, 205, 120))
        draw.point((cx, cy), fill=(255, 245, 195))

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
