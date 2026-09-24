#!/usr/bin/env python3
"""生成残卷缠甲（scroll_wrap / CanJuan Armor）头盔、胸甲、护腿与靴子的程序化 3D 建模生成器。

参考图来源与结构对应（世界观：末法残土符法经卷甲冑）：
- 概念与三视图（ref_concept.png, ref_three_view_1.png, ref_three_view_2.png, ref_exploded.png）：
  1. 盔甲主体材质由古旧符咒经卷纸片（暗黄纸基、焦黑撕裂磨损边缘、米芾风朱砂/黑墨符文真言）层叠搭接而成。
  2. 头部 (Helmet)：
     - 顶部符纸层层搭接围合（多层符纸自顶向下垂直拼接）。
     - 头围粗麻绳/熟皮绳箍环绕固定，右侧精细打结、带悬垂绳尾与铜铃/金粒坠饰。
     - 周圈长条垂落符纸悬挂，自然遮挡面部与后颈，边缘参差撕裂。
     - 内部深色布帽托（防磨内衬结构）。
  3. 胸甲 (Chestplate)：
     - 正面多排矩形符纸甲片（鱼鳞/札甲式自上向下层层压叠，带焦黑磨损边与朱砂印记）。
     - 双肩多层重叠外挑的符纸披膊/肩甲（左右各 4~5 层阶梯外展）。
     - 领口、胸前与侧肋由深棕麻绳与皮绳纵横绞结穿线固定（带正面中轴绳结与双侧绳扣）。
     - 下摆垂挂流苏符条护腰。
  4. 护腿 (Leggings)：
     - 腰部多圈粗麻绳束腰环绕，前系双垂绳结。
     - 大腿部斜向缠绕的符纸裹层。
     - 膝盖处加固的弧形护膝骨架/皮圈，外附符文护膝板。
     - 小腿至脚踝紧密缠绕的经卷纸带与交叉绑腿绳索。
  5. 战靴 (Boots)：
     - 符卷包裹鞋面与鞋帮（高帮设计，顶口参差撕裂）。
     - 粗麻绳环踝加固缠绕，鞋底缝线粗犷耐磨，带脚背系绳。

挂载点 (Mount) 规范：
- HEAD -> Helmet
- BODY -> Chestplate (含肩甲与腰摆)
- LEFT_LEG / RIGHT_LEG -> Leggings (按腿劈开)
- LEFT_FOOT / RIGHT_FOOT -> Boots (按脚劈开)

运行时真相为 client 的 ArmorPartModel.CUBE_TABLES。
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

MATERIAL = "scroll_wrap"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 贴图 64x64 四象限规划：
# 每个象限的原点留出足够空间放置 box-uv：2*(sx+sz) 宽, sy+sz 高
# Q1 (0,0): 古旧暗黄符纸基
# Q2 (24,0): 朱砂印纸
# Q3 (0,28): 熟皮带、粗麻绳与绳结
# Q4 (24,28): 内衬布、深色阴影
UV_PAPER_MAIN = (0, 0)
UV_PAPER_CINNABAR = (24, 0)
UV_ROPE_STRAP = (0, 28)
UV_LINING_DARK = (24, 28)


def c(mount: str, name: str, origin: tuple[float, float, float], size: tuple[float, float, float], uv: tuple[int, int] = UV_PAPER_MAIN) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 1. 头盔 (HELMET) ──────────────────────────────────────────────────────────
# 头部基准: x∈[-4,4], y∈[24,32], z∈[-4,4]，骨骼枢轴在 y=24。

def _helmet_inner_cap() -> tuple[Cube, ...]:
    """内部黑色布帽托内衬。"""
    return (
        # 拆分为前后两半，微调 z 分割线避免共面
        c("HEAD", "helm_inner_cap_top_f", (-4.1, 31.05, -4.1), (8.2, 1.2, 4.14), UV_LINING_DARK),
        c("HEAD", "helm_inner_cap_top_b", (-4.1, 31.05, 0.04), (8.2, 1.2, 4.06), UV_LINING_DARK),
        c("HEAD", "helm_inner_cap_band_f", (-4.18, 26.2, -4.18), (8.36, 4.9, 4.14), UV_LINING_DARK),
        c("HEAD", "helm_inner_cap_band_b", (-4.18, 26.2, -0.04), (8.36, 4.9, 4.22), UV_LINING_DARK),
    )


def _helmet_top_dome() -> tuple[Cube, ...]:
    """顶部多层符纸垂直拼接围合的穹顶。"""
    return (
        # 穹顶最顶层中脊符纸 (拆分前后，适配 UV 限制)
        c("HEAD", "helm_dome_crown_f", (-3.5, 32.25, -3.8), (7.0, 0.45, 3.84), UV_PAPER_MAIN),
        c("HEAD", "helm_dome_crown_b", (-3.5, 32.25, 0.04), (7.0, 0.45, 3.76), UV_PAPER_MAIN),
        c("HEAD", "helm_dome_crest_f", (-2.2, 32.38, -4.1), (4.4, 0.38, 3.8), UV_PAPER_CINNABAR),
        c("HEAD", "helm_dome_crest_b", (-2.2, 32.38, 0.3), (4.4, 0.38, 3.8), UV_PAPER_MAIN),

        # 穹顶前后斜坡收拢 (微调高度偏置避免角部重叠)
        c("HEAD", "helm_dome_slope_front", (-4.2, 31.42, -4.35), (8.4, 0.94, 1.8), UV_PAPER_CINNABAR),
        c("HEAD", "helm_dome_slope_back", (-4.2, 31.42, 2.55), (8.4, 0.94, 1.8), UV_PAPER_MAIN),
        c("HEAD", "helm_dome_slope_left", (2.55, 31.36, -4.2), (1.8, 0.94, 4.3), UV_PAPER_MAIN),
        c("HEAD", "helm_dome_slope_left_b", (2.55, 31.36, 0.1), (1.8, 0.94, 4.1), UV_PAPER_MAIN),
        c("HEAD", "helm_dome_slope_right", (-4.35, 31.36, -4.2), (1.8, 0.94, 4.3), UV_PAPER_MAIN),
        c("HEAD", "helm_dome_slope_right_b", (-4.35, 31.36, 0.1), (1.8, 0.94, 4.1), UV_PAPER_MAIN),
    )


def _helmet_cord_and_knots() -> tuple[Cube, ...]:
    """环绕头部的粗麻绳箍与右侧打结垂坠。"""
    return (
        # 头围环绕麻绳箍 (y ≈ 28.5 ~ 29.5，微调高低避免交角共面)
        c("HEAD", "helm_rope_front", (-4.45, 28.62, -4.55), (8.9, 0.85, 0.6), UV_ROPE_STRAP),
        c("HEAD", "helm_rope_back", (-4.45, 28.62, 3.95), (8.9, 0.85, 0.6), UV_ROPE_STRAP),
        c("HEAD", "helm_rope_left", (3.95, 28.58, -4.45), (0.6, 0.85, 8.9), UV_ROPE_STRAP),
        c("HEAD", "helm_rope_right", (-4.55, 28.58, -4.45), (0.6, 0.85, 8.9), UV_ROPE_STRAP),

        # 右侧绳结与金属/铜铃坠饰 (位于右耳附近 x ≈ -4.6, z ≈ -0.5)
        c("HEAD", "helm_rope_knot_main", (-5.05, 28.2, -1.2), (0.95, 1.4, 1.8), UV_ROPE_STRAP),
        c("HEAD", "helm_rope_tail_a", (-4.85, 24.2, -1.4), (0.45, 4.1, 0.45), UV_ROPE_STRAP),
        c("HEAD", "helm_rope_tail_b", (-4.85, 24.8, -0.4), (0.45, 3.5, 0.45), UV_ROPE_STRAP),
        c("HEAD", "helm_bell_charm", (-5.15, 25.4, -0.9), (0.85, 0.85, 0.85), UV_ROPE_STRAP),
    )


def _helmet_hanging_scrolls() -> tuple[Cube, ...]:
    """周圈垂直悬垂的长条符纸（自然遮挡面部与后颈）。"""
    return (
        # 1. 正面悬垂符纸（中央大朱砂主符 + 左右次符）
        c("HEAD", "helm_scroll_front_mid", (-1.8, 22.8, -4.68), (3.6, 6.2, 0.35), UV_PAPER_CINNABAR),
        c("HEAD", "helm_scroll_front_l", (1.85, 23.6, -4.62), (2.4, 5.2, 0.35), UV_PAPER_MAIN),
        c("HEAD", "helm_scroll_front_r", (-4.25, 23.6, -4.62), (2.4, 5.2, 0.35), UV_PAPER_MAIN),

        # 2. 左右两侧悬垂符纸
        c("HEAD", "helm_scroll_left_f", (4.15, 22.6, -3.8), (0.35, 6.2, 2.6), UV_PAPER_CINNABAR),
        c("HEAD", "helm_scroll_left_b", (4.15, 22.2, -0.8), (0.35, 6.6, 4.4), UV_PAPER_MAIN),
        c("HEAD", "helm_scroll_right_f", (-4.5, 22.6, -3.8), (0.35, 6.2, 2.6), UV_PAPER_MAIN),
        c("HEAD", "helm_scroll_right_b", (-4.5, 22.2, -0.8), (0.35, 6.6, 4.4), UV_PAPER_CINNABAR),

        # 3. 后背多层下垂符纸（后帘，自上而下阶梯式覆盖后颈）
        c("HEAD", "helm_scroll_back_layer1", (-4.1, 23.4, 4.12), (8.2, 5.6, 0.38), UV_PAPER_CINNABAR),
        c("HEAD", "helm_scroll_back_layer2", (-3.6, 21.6, 4.38), (7.2, 6.8, 0.38), UV_PAPER_MAIN),
        c("HEAD", "helm_scroll_back_tail_l", (1.2, 20.2, 4.6), (2.2, 5.2, 0.35), UV_PAPER_MAIN),
        c("HEAD", "helm_scroll_back_tail_r", (-3.4, 20.2, 4.6), (2.2, 5.2, 0.35), UV_PAPER_CINNABAR),
    )


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "scroll_wrap_helmet",
        "SCROLL WRAP HELMET",
        _helmet_inner_cap()
        + _helmet_top_dome()
        + _helmet_cord_and_knots()
        + _helmet_hanging_scrolls(),
    )


# ─── 2. 胸甲 (CHESTPLATE) ────────────────────────────────────────────────────────
# 躯干基准: x∈[-4,4], y∈[12,24], z∈[-2,2]，骨骼枢轴在 y=24。
# 双臂基准: 左臂 x∈[4,8], 右臂 x∈[-8,-4], y∈[12,24], z∈[-2,2] (均挂在 BODY 上)。

def _chest_torso_plates() -> tuple[Cube, ...]:
    """躯干层叠经卷符札甲片（自上而下 4 层鱼鳞叠压）。"""
    return (
        # 1. 前胸札甲层 (自上而下 4 段)
        # 第一层 (上胸/护心，y: 20.0 ~ 23.6)
        c("BODY", "chest_front_row1", (-4.1, 20.0, -2.62), (8.2, 3.6, 0.65), UV_PAPER_CINNABAR),
        # 第二层 (中胸，y: 17.2 ~ 20.4，微外探)
        c("BODY", "chest_front_row2", (-4.2, 17.2, -2.76), (8.4, 3.4, 0.72), UV_PAPER_MAIN),
        # 第三层 (上腹，y: 14.4 ~ 17.6)
        c("BODY", "chest_front_row3", (-4.15, 14.4, -2.7), (8.3, 3.4, 0.7), UV_PAPER_CINNABAR),
        # 第四层 (下腹，y: 12.0 ~ 14.8)
        c("BODY", "chest_front_row4", (-4.08, 12.0, -2.6), (8.16, 3.0, 0.65), UV_PAPER_MAIN),

        # 2. 后背札甲层 (对应 4 段)
        c("BODY", "chest_back_row1", (-4.1, 20.0, 1.97), (8.2, 3.6, 0.65), UV_PAPER_MAIN),
        c("BODY", "chest_back_row2", (-4.2, 17.2, 2.04), (8.4, 3.4, 0.72), UV_PAPER_CINNABAR),
        c("BODY", "chest_back_row3", (-4.15, 14.4, 2.0), (8.3, 3.4, 0.7), UV_PAPER_MAIN),
        c("BODY", "chest_back_row4", (-4.08, 12.0, 1.95), (8.16, 3.0, 0.65), UV_PAPER_CINNABAR),

        # 3. 侧肋贴合与防护
        c("BODY", "chest_flank_l", (3.88, 12.24, -2.15), (0.45, 11.16, 4.3), UV_LINING_DARK),
        c("BODY", "chest_flank_r", (-4.33, 12.24, -2.15), (0.45, 11.16, 4.3), UV_LINING_DARK),
    )


def _chest_harness_cords() -> tuple[Cube, ...]:
    """编织穿线、领口打结、十字系带与束腰麻绳。"""
    return (
        # 1. 领口固定绳箍与胸前大绳结 (y ≈ 22.8)
        c("BODY", "chest_collar_rope", (-3.6, 23.0, -2.85), (7.2, 0.75, 0.5), UV_ROPE_STRAP),
        c("BODY", "chest_collar_knot", (-0.9, 22.4, -3.15), (1.8, 1.8, 0.65), UV_ROPE_STRAP),

        # 2. 纵向符板穿扎绳线（左右各一列纵向紧致皮绳）
        c("BODY", "chest_stitch_cord_l", (2.1, 12.4, -2.88), (0.45, 10.4, 0.35), UV_ROPE_STRAP),
        c("BODY", "chest_stitch_cord_r", (-2.55, 12.4, -2.88), (0.45, 10.4, 0.35), UV_ROPE_STRAP),
        c("BODY", "chest_stitch_cord_bl", (2.1, 12.4, 2.53), (0.45, 10.4, 0.35), UV_ROPE_STRAP),
        c("BODY", "chest_stitch_cord_br", (-2.55, 12.4, 2.53), (0.45, 10.4, 0.35), UV_ROPE_STRAP),

        # 3. 侧肋系绳锁扣 (左右各两对)
        c("BODY", "chest_side_tie_l1", (3.96, 18.5, -2.28), (0.35, 0.65, 4.56), UV_ROPE_STRAP),
        c("BODY", "chest_side_tie_l2", (3.96, 14.5, -2.28), (0.35, 0.65, 4.56), UV_ROPE_STRAP),
        c("BODY", "chest_side_tie_r1", (-4.31, 18.5, -2.28), (0.35, 0.65, 4.56), UV_ROPE_STRAP),
        c("BODY", "chest_side_tie_r2", (-4.31, 14.5, -2.28), (0.35, 0.65, 4.56), UV_ROPE_STRAP),

        # 4. 腰部粗绳与下摆悬挂符条 (Flap tassles)
        c("BODY", "chest_waist_rope", (-4.35, 12.15, -2.78), (8.7, 0.85, 5.56), UV_ROPE_STRAP),
        c("BODY", "chest_waist_flap_f", (-2.4, 9.4, -2.72), (4.8, 2.9, 0.38), UV_PAPER_CINNABAR),
        c("BODY", "chest_waist_flap_b", (-2.4, 9.4, 2.34), (4.8, 2.9, 0.38), UV_PAPER_MAIN),
    )


def _chest_pauldrons_and_arms() -> tuple[Cube, ...]:
    """左右多层重叠展开的经卷护肩与小臂符带。"""
    cubes = []
    for side in ("l", "r"):
        def x(base: float, span: float) -> float:
            return base if side == "l" else -(base + span)

        cubes.extend((
            # 1. 多层阶梯外展护肩 (Pauldrons, 挂在 BODY 上, 左臂 x: 4~8, 右臂 x: -8~-4)
            # 顶层护肩基底 (y: 22.8 ~ 24.2)
            c("BODY", f"pauldron_base_{side}", (x(3.85, 4.4), 22.8, -2.4), (4.4, 1.4, 4.8), UV_LINING_DARK),
            # 肩甲第 1 层符纸 (外挑 0.5)
            c("BODY", f"pauldron_layer1_{side}", (x(4.0, 4.6), 23.4, -2.55), (4.6, 1.2, 5.1), UV_PAPER_CINNABAR),
            # 肩甲第 2 层符纸 (向外下倾阶梯展开 y: 21.6 ~ 23.2)
            c("BODY", f"pauldron_layer2_{side}", (x(4.6, 4.5), 21.6, -2.45), (4.5, 1.8, 4.9), UV_PAPER_MAIN),
            # 肩甲第 3 层外挑符纸翼 (y: 19.8 ~ 21.8)
            c("BODY", f"pauldron_layer3_{side}", (x(5.4, 4.2), 19.8, -2.35), (4.2, 2.2, 4.7), UV_PAPER_CINNABAR),
            # 绑在肩甲顶部的皮绳与结扣
            c("BODY", f"pauldron_strap_{side}", (x(4.4, 3.8), 24.15, -2.45), (3.8, 0.5, 4.9), UV_ROPE_STRAP),

            # 2. 小臂紧密缠绕的经卷护腕 (Armguards, y: 12.2 ~ 17.5)
            c("BODY", f"arm_wrap_scroll_{side}", (x(3.92, 4.16), 12.2, -2.16), (4.16, 5.4, 4.32), UV_PAPER_MAIN),
            c("BODY", f"arm_strap_upper_{side}", (x(3.9, 4.2), 16.2, -2.25), (4.2, 0.65, 4.5), UV_ROPE_STRAP),
            c("BODY", f"arm_strap_lower_{side}", (x(3.9, 4.2), 12.8, -2.25), (4.2, 0.65, 4.5), UV_ROPE_STRAP),
        ))
    return tuple(cubes)


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "scroll_wrap_chestplate",
        "SCROLL WRAP CHESTPLATE",
        _chest_torso_plates()
        + _chest_harness_cords()
        + _chest_pauldrons_and_arms(),
    )


# ─── 3. 护腿 (LEGGINGS) ────────────────────────────────────────────────────────
# 腿盒基准: x∈[-2,2], y∈[0,12], z∈[-2,2]，骨骼枢轴在 y=12。
# 左右腿独立分侧 (LEFT_LEG: x_offset=+1.9, RIGHT_LEG: x_offset=-1.9)。

def _leggings_single_leg(mount: str) -> tuple[Cube, ...]:
    prefix = mount.lower()
    is_left = "left" in prefix
    dy = 0.04 if is_left else -0.04
    dz = 0.02 if is_left else -0.02

    def ox(base: float, span: float) -> float:
        return base if is_left else -(base + span)

    return (
        # 1. 大腿斜向缠绕经卷层 (y: 6.8 ~ 11.5)
        c(mount, f"{prefix}_thigh_wrap_main", (-2.12, 6.8 + dy, -2.12 + dz), (4.24, 4.8, 4.24), UV_PAPER_MAIN),
        c(mount, f"{prefix}_thigh_accent_front", (-1.8, 7.8 + dy, -2.38 + dz), (3.6, 3.4, 0.35), UV_PAPER_CINNABAR),
        c(mount, f"{prefix}_thigh_strap_top", (-2.18, 10.4 + dy, -2.18 + dz), (4.36, 0.75, 4.36), UV_ROPE_STRAP),
        c(mount, f"{prefix}_thigh_strap_mid", (-2.18, 7.2 + dy, -2.18 + dz), (4.36, 0.75, 4.36), UV_ROPE_STRAP),

        # 2. 膝盖骨架与符文护膝板 (y: 4.8 ~ 7.4)
        c(mount, f"{prefix}_knee_frame", (-2.1, 4.8 + dy, -2.6 + dz), (4.2, 2.6, 0.75), UV_LINING_DARK),
        c(mount, f"{prefix}_knee_plate", (-1.6, 5.0 + dy, -2.92 + dz), (3.2, 2.2, 0.45), UV_PAPER_CINNABAR),
        c(mount, f"{prefix}_knee_strap", (-2.22, 5.8 + dy, -2.22 + dz), (4.44, 0.7, 4.44), UV_ROPE_STRAP),

        # 3. 小腿至踝部经卷绑腿 (y: 1.8 ~ 5.2)
        c(mount, f"{prefix}_shin_wrap_main", (-2.08, 1.8 + dy, -2.08 + dz), (4.16, 3.4, 4.16), UV_PAPER_MAIN),
        c(mount, f"{prefix}_shin_strap_cross1", (-2.14, 3.6 + dy, -2.14 + dz), (4.28, 0.65, 4.28), UV_ROPE_STRAP),
        c(mount, f"{prefix}_shin_strap_cross2", (-2.14, 2.0 + dy, -2.14 + dz), (4.28, 0.65, 4.28), UV_ROPE_STRAP),
        # 侧面加固绳结
        c(mount, f"{prefix}_shin_side_knot", (ox(1.7, 0.55), 3.4 + dy, -0.6 + dz), (0.55, 1.1, 1.2), UV_ROPE_STRAP),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "scroll_wrap_leggings",
        "SCROLL WRAP LEGGINGS",
        _leggings_single_leg("LEFT_LEG") + _leggings_single_leg("RIGHT_LEG"),
    )


# ─── 4. 战靴 (BOOTS) ───────────────────────────────────────────────────────────
# 脚部基准: x∈[-2,2], y∈[0,12], z∈[-2,2]，骨骼枢轴在 y=0。

def _boots_single_foot(mount: str) -> tuple[Cube, ...]:
    prefix = mount.lower()
    is_left = "left" in prefix
    dy = 0.03 if is_left else -0.03
    dz = 0.02 if is_left else -0.02

    return (
        # 1. 厚实战靴底底板与缝线边 (y: -0.05 ~ 1.2)
        c(mount, f"{prefix}_sole_base", (-2.2, -0.05 + dy, -3.2 + dz), (4.4, 1.2, 5.4), UV_LINING_DARK),
        c(mount, f"{prefix}_sole_stitch_rim", (-2.32, 0.2 + dy, -3.32 + dz), (4.64, 0.55, 5.64), UV_ROPE_STRAP),

        # 2. 经卷鞋面与脚趾护甲 (y: 0.8 ~ 2.6)
        c(mount, f"{prefix}_toe_scroll_vamp", (-2.05, 0.8 + dy, -3.05 + dz), (4.1, 1.8, 3.2), UV_PAPER_CINNABAR),
        c(mount, f"{prefix}_toe_stitch_cross", (-1.2, 1.4 + dy, -3.2 + dz), (2.4, 0.65, 0.35), UV_ROPE_STRAP),

        # 3. 高帮经卷鞋筒与顶口撕裂边 (y: 1.6 ~ 5.4)
        c(mount, f"{prefix}_boot_shaft_main", (-2.12, 1.6 + dy, -2.12 + dz), (4.24, 3.6, 4.24), UV_PAPER_MAIN),
        c(mount, f"{prefix}_boot_shaft_lip", (-2.18, 4.6 + dy, -2.18 + dz), (4.36, 1.0, 4.36), UV_PAPER_CINNABAR),

        # 4. 环踝粗麻绳双圈束紧与系绳 (y: 1.8 ~ 3.4)
        c(mount, f"{prefix}_ankle_rope_lower", (-2.24, 1.8 + dy, -2.35 + dz), (4.48, 0.65, 4.7), UV_ROPE_STRAP),
        c(mount, f"{prefix}_ankle_rope_upper", (-2.24, 2.8 + dy, -2.35 + dz), (4.48, 0.65, 4.7), UV_ROPE_STRAP),
        c(mount, f"{prefix}_ankle_rope_knot", (-0.75, 2.6 + dy, -2.6 + dz), (1.5, 1.1, 0.55), UV_ROPE_STRAP),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "scroll_wrap_boots",
        "SCROLL WRAP BOOTS",
        _boots_single_foot("LEFT_FOOT") + _boots_single_foot("RIGHT_FOOT"),
    )


def parts() -> tuple[ArmorPart, ...]:
    return part_helmet(), part_chestplate(), part_leggings(), part_boots()


# ─── 贴图生成 (64x64 残卷暗黄纸、朱砂印、熟皮绳与内衬) ───────────────────────────

def make_texture() -> Image.Image:
    """生成包含 4 象限完整特性的 64x64 残卷缠甲贴图。"""
    rng = random.Random(0x5C90_11AA)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (220, 204, 168))
    pixels = image.load()

    # 1. 基础噪点与四象限底色
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                # Q1: 暗黄古卷纸（微黄粗纤维与自然纸浆斑）
                base = (226, 208, 168) if (x + y) % 3 != 0 else (210, 192, 152)
            elif x >= 32 and y < 32:
                # Q2: 朱砂印纸与炭化边缘（焦黄边 + 暖红印）
                base = (218, 186, 142) if (x + y) % 2 == 0 else (204, 172, 128)
            elif x < 32 and y >= 32:
                # Q3: 熟皮绳与草麻绳（深棕黑硬鞣皮绳、棕黄粗麻绳）
                base = (68, 50, 36) if y % 2 == 0 else (56, 40, 28)
            else:
                # Q4: 内衬布、阴影褶皱与暗部
                base = (34, 30, 28) if (x + y) % 4 == 0 else (48, 42, 38)

            noise = rng.randint(-6, 6)
            warm = rng.randint(-3, 3)
            pixels[x, y] = (
                max(0, min(255, base[0] + noise + warm)),
                max(0, min(255, base[1] + noise)),
                max(0, min(255, base[2] + noise - warm)),
            )

    draw = ImageDraw.Draw(image)

    # 2. Q1: 古卷纸墨迹与米芾风狂草符文线 (Calligraphy brush strokes)
    for row in range(2, 30, 3):
        # 墨迹笔画
        draw.line((2, row, 29, row), fill=(44, 38, 34), width=1)
        # 笔锋飞白断点
        for gap in (rng.randint(4, 12), rng.randint(16, 26)):
            draw.point((gap, row), fill=(210, 192, 152))
            draw.point((gap + 1, row), fill=(210, 192, 152))

    # 3. Q2: 朱砂大印与真言符咒 (Cinnabar Seals & Incantations)
    # 朱砂方印与圆印
    draw.rectangle((36, 4, 48, 16), outline=(178, 48, 38), width=1)
    draw.rectangle((38, 6, 46, 14), fill=(160, 42, 32))
    draw.rectangle((50, 14, 60, 24), fill=(148, 36, 30))
    # 印内白文与金泥微芒
    draw.line(((40, 8), (44, 8)), fill=(230, 212, 180), width=1)
    draw.line(((40, 12), (44, 12)), fill=(230, 212, 180), width=1)
    draw.line(((52, 18), (58, 18)), fill=(218, 168, 64), width=1)
    # 边缘焦黑碳化痕 (Burnt edges)
    for bx in range(32, 64):
        draw.point((bx, 0), fill=(38, 28, 22))
        draw.point((bx, 31), fill=(48, 36, 28))

    # 4. Q3: 熟皮与麻绳编织纹 (Leather & Rope weave)
    for y in (36, 42, 48, 54, 60):
        # 绳索凹槽与高光
        draw.line((0, y, 31, y), fill=(32, 22, 16), width=1)
        draw.line((0, y + 1, 31, y + 1), fill=(92, 72, 54), width=1)
    # 麻绳绞花斜线
    for x_start in range(0, 30, 4):
        draw.line((x_start, 48, x_start + 3, 63), fill=(116, 92, 64), width=1)
    # 青铜坠饰小点
    for bx, by in ((6, 38), (14, 44), (22, 50), (10, 56), (26, 38)):
        draw.point((bx, by), fill=(182, 154, 82))
        draw.point((bx + 1, by), fill=(84, 98, 72))

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

    # 导出 OnPlayer 穿戴资产
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
    parser = argparse.ArgumentParser(description="生成残卷缠甲 (scroll_wrap) 3D 程序化资产与贴图。")
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
