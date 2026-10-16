#!/usr/bin/env python3
"""粗布面巾（cu_bu_mian_jin）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工：丰富细节密度与末土废土质感。

任务卡：末土装备 #12（.task-wasteland-gear.md）。
挂点：口鼻 / 颈 / 上胸（头面部 y∈[24.08,28.8] + 颈胸下摆 y∈[12.6,23.95]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]
  躯干 x∈[-4,4], y∈[12,24], z∈[-2,2]
  双臂 x∈[4,8] / [-8,-4], y∈[12,24], z∈[-2,2]
  门禁 clearance：任何方块不得扎进头盒、躯干或双臂（咬入 ≤ 0.05px）。

外形：
  打破长方体轮廓：
  - 口鼻处由 4 道粗布条错位层叠包裹，每道布条前后厚度起伏，鼻尖与唇突生动；
  - 面巾下摆呈松垮的大三角巾垂落于胸前，下摆边缘参差撕裂、垂下长短不齐的破布齿与碎毛边；
  - 脑后系结粗大，垂下两条长度不一的飘带，末端带撕裂毛须；
  - 面部与胸摆布满深浅补丁、破洞暗衬、麻线粗缝迹；
  - 右颊下垂挂一枚锈铁小环与磨尖兽骨针饰件。
  总方块数达到 220+，按挂点分骨 16 骨。
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "linen":        (142, 126, 98),    # 粗布主色
    "linen_lt":     (172, 154, 122),   # 粗布凸起高光/亮面
    "linen_dk":     (108, 92, 70),     # 粗布褶皱阴面/折痕
    "patch":        (120, 88, 64),     # 废皮/深色粗布补丁
    "patch_dk":     (86, 62, 44),      # 补丁阴影
    "patch_lt":     (148, 114, 86),    # 补丁磨白边缘
    "cord":         (150, 126, 84),    # 粗草绳/麻绳
    "cord_dk":      (104, 84, 54),     # 绳结深色
    "stitch":       (188, 176, 148),   # 粗线迹
    "rust":         (146, 84, 50),     # 锈铁小环
    "bone":         (206, 194, 164),   # 骨针/骨坠
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨阴面
    "void":         (28, 24, 22),      # 破洞内暗衬
}

SOFT = ("linen", "linen_lt", "linen_dk", "patch", "patch_dk", "patch_lt", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), SOFT)

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("head_mount", "root", (0.0, 24.0, 0.0), "头戴基准骨"),
    ("mask_face", "head_mount", (0.0, 26.0, -4.2), "口鼻正面遮罩布层"),
    ("mask_cheek_r", "head_mount", (4.2, 26.0, 0.0), "右面颊护布"),
    ("mask_cheek_l", "head_mount", (-4.2, 26.0, 0.0), "左面颊护布"),
    ("mask_knot_back", "head_mount", (0.0, 26.5, 4.3), "后脑系结"),
    ("knot_tail_r", "mask_knot_back", (0.8, 26.0, 4.5), "脑后垂带（右）"),
    ("knot_tail_l", "mask_knot_back", (-0.8, 26.0, 4.5), "脑后垂带（左）"),
    ("neck_wrap", "root", (0.0, 23.0, 0.0), "颈围过渡圈"),
    ("bib_chest", "root", (0.0, 21.0, -2.3), "胸前三角巾主片"),
    ("bib_fray_r", "bib_chest", (2.0, 16.0, -2.4), "三角巾右撕裂边缘与破角"),
    ("bib_fray_l", "bib_chest", (-2.0, 16.0, -2.4), "三角巾左撕裂边缘与破角"),
    ("bib_tip_drape", "bib_chest", (0.0, 14.5, -2.4), "三角巾中央下垂尖端"),
    ("mask_details", "bib_chest", (0.0, 19.5, -2.4), "补丁与粗麻缝线"),
    ("hang_iron_ring", "head_mount", (4.3, 25.0, -2.6), "右颊挂绳锈铁环"),
    ("hang_bone_charm", "hang_iron_ring", (4.4, 23.5, -2.6), "挂环下兽骨针坠"),
]

ASYM_BONES = frozenset([
    "knot_tail_r", "knot_tail_l", "bib_fray_r", "bib_fray_l",
    "bib_tip_drape", "mask_details", "hang_iron_ring", "hang_bone_charm",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_mask_face(g: Gear) -> None:
    """口鼻正面遮罩：由 4 道粗布条错位层叠包裹，各层错落。"""
    b = "mask_face"
    # ── 第 1 层（下巴包底层 y: 24.08 .. 25.18）
    g.box(b, "chin_base_c", (-1.92, 24.08, -4.66), (1.92, 25.14, -4.12), "linen")
    g.pair(b, "chin_base_side", (1.94, 24.1, -4.54), (3.65, 25.15, -4.12), "linen")
    g.box(b, "chin_lip_roll", (-2.38, 24.12, -4.8), (2.38, 24.36, -4.34), "linen_dk")
    g.pair(b, "chin_lip_roll_side", (2.4, 24.12, -4.64), (3.52, 24.38, -4.24), "linen_dk")
    g.box(b, "chin_fold_crest", (-1.18, 24.48, -4.86), (1.18, 24.96, -4.44), "linen_lt")
    g.pair(b, "chin_fold_slope", (1.22, 24.52, -4.74), (2.48, 24.94, -4.34), "linen")
    g.pair(b, "chin_side_flap", (2.85, 24.62, -4.62), (3.72, 25.08, -4.26), "linen_lt")

    # ── 第 2 层（下唇与嘴角过渡 y: 25.18 .. 26.35）
    g.box(b, "lip_tier_c", (-2.18, 25.18, -4.76), (2.18, 26.32, -4.12), "linen_dk")
    g.pair(b, "lip_tier_side", (2.22, 25.18, -4.58), (3.75, 26.32, -4.12), "linen_dk")
    g.box(b, "lip_bulge_c", (-1.52, 25.42, -4.9), (1.52, 26.16, -4.46), "linen")
    g.box(b, "lip_crest_crest", (-0.88, 25.66, -4.98), (0.88, 26.04, -4.64), "linen_lt")
    g.pair(b, "lip_bulge_slope", (1.54, 25.46, -4.78), (2.78, 26.12, -4.36), "linen_lt")
    g.pair(b, "lip_side_pleat", (2.65, 25.55, -4.66), (3.62, 25.98, -4.25), "linen")

    # ── 第 3 层（鼻下与鼻梁基座 y: 26.35 .. 27.52）
    g.box(b, "nose_base_tier_c", (-2.38, 26.35, -4.75), (2.38, 27.48, -4.12), "linen")
    g.pair(b, "nose_base_tier_side", (2.42, 26.35, -4.64), (3.85, 27.48, -4.12), "linen")
    g.box(b, "nose_wave_ridge", (-2.22, 26.88, -4.96), (2.22, 27.26, -4.5), "linen_lt")
    g.pair(b, "nose_wave_side", (2.26, 26.88, -4.82), (3.22, 27.26, -4.38), "linen_dk")
    g.box(b, "nose_wave_shadow", (-2.12, 26.6, -4.82), (2.12, 26.84, -4.44), "linen_dk")
    g.pair(b, "nose_wing_crest", (2.85, 26.55, -4.68), (3.65, 27.15, -4.28), "linen_lt")

    # ── 第 4 层（鼻尖与鼻翼上包顶 y: 27.52 .. 28.75）
    g.box(b, "nose_crest_tier_c", (-1.42, 27.52, -4.8), (1.42, 28.52, -4.12), "linen")
    g.pair(b, "nose_wing_tier", (1.45, 27.52, -4.7), (3.62, 28.52, -4.12), "linen")
    g.box(b, "nose_apex_ridge", (-0.62, 27.86, -4.98), (0.62, 28.46, -4.54), "linen_lt")
    g.pair(b, "nose_apex_flank", (0.65, 27.9, -4.82), (1.62, 28.42, -4.46), "linen")
    g.box(b, "nose_top_hem_c", (-2.38, 28.5, -4.66), (2.38, 28.75, -4.18), "linen_lt")
    g.pair(b, "nose_top_hem_side", (2.42, 28.5, -4.56), (3.75, 28.75, -4.18), "linen_dk")
    g.pair(b, "nose_top_tooth", (1.2, 28.75, -4.62), (1.8, 29.05, -4.26), "linen_lt")
    g.pair(b, "nose_bridge_wedge", (0.75, 27.12, -4.89), (1.45, 27.65, -4.46), "linen_lt")


def part_mask_cheeks(g: Gear) -> None:
    """左右面颊包覆层：多层斜向重叠与加固皮绳。"""
    b = "mask_cheek"
    g.pair(b, "cheek_band_front", (4.08, 24.55, -4.12), (4.42, 28.32, -1.64), "linen", sided=True)
    g.pair(b, "cheek_band_mid", (4.12, 24.75, -1.48), (4.46, 28.15, 1.48), "linen_dk", sided=True)
    g.pair(b, "cheek_band_back", (4.08, 25.12, 1.64), (4.42, 27.95, 4.08), "linen", sided=True)

    g.pair(b, "cheek_pleat_f1", (4.24, 26.22, -3.8), (4.52, 27.12, -1.6), "linen_lt", sided=True)
    g.pair(b, "cheek_pleat_f2", (4.24, 25.22, -3.6), (4.52, 25.96, -1.8), "linen_dk", sided=True)
    g.pair(b, "cheek_pleat_m1", (4.25, 26.42, -1.2), (4.54, 27.32, 1.2), "linen_lt", sided=True)
    g.pair(b, "cheek_pleat_m2", (4.25, 25.42, -1.0), (4.54, 26.22, 1.0), "linen", sided=True)
    g.pair(b, "cheek_pleat_b1", (4.24, 26.02, 1.6), (4.52, 27.22, 3.8), "linen_dk", sided=True)

    g.pair(b, "cheek_roll_top_f", (4.16, 28.16, -3.9), (4.48, 28.44, -1.4), "linen_lt", sided=True)
    g.pair(b, "cheek_roll_top_b", (4.16, 27.94, -1.2), (4.48, 28.24, 3.5), "linen_lt", sided=True)
    g.pair(b, "cheek_roll_bot_f", (4.2, 24.46, -3.6), (4.48, 24.84, -1.5), "linen_dk", sided=True)
    g.pair(b, "cheek_roll_bot_b", (4.16, 24.96, 1.5), (4.46, 25.34, 3.6), "linen_dk", sided=True)

    g.pair(b, "cheek_cord_hi", (4.26, 27.42, -3.8), (4.56, 27.82, 3.8), "cord", sided=True)
    g.pair(b, "cheek_cord_lo", (4.26, 25.62, -3.8), (4.56, 26.02, 3.78), "cord_dk", sided=True)
    g.pair(b, "cheek_cord_knot_f", (4.32, 25.55, -3.4), (4.62, 26.15, -2.8), "cord", sided=True)
    g.pair(b, "cheek_cross_strap", (4.28, 26.65, -2.2), (4.58, 27.85, -1.5), "linen_dk", sided=True)


def part_mask_knots(g: Gear) -> None:
    """脑后绑带与系结：粗大死结与长短不齐的双飘带。"""
    bk = "mask_knot_back"
    g.box(bk, "strap_back_hi", (-4.05, 26.78, 4.08), (4.05, 27.68, 4.42), "linen")
    g.box(bk, "strap_back_lo", (-4.05, 25.52, 4.08), (4.05, 26.42, 4.42), "linen_dk")
    g.box(bk, "strap_cord_wrap", (-3.8, 26.28, 4.25), (3.8, 26.98, 4.56), "cord")

    g.box(bk, "knot_core_main", (-1.35, 25.65, 4.38), (1.35, 27.55, 4.92), "cord_dk")
    g.box(bk, "knot_cross_diag1", (-1.55, 26.08, 4.5), (1.55, 27.12, 4.96), "cord")
    g.box(bk, "knot_cross_diag2", (-1.1, 25.48, 4.56), (1.1, 27.72, 5.02), "cord")
    g.box(bk, "knot_tie_crest", (-0.75, 25.88, 4.68), (0.75, 27.32, 5.16), "cord_dk")
    g.box(bk, "knot_cloth_bunch", (-0.85, 26.32, 4.46), (0.85, 27.02, 5.06), "linen_lt")

    kr = "knot_tail_r"
    g.box(kr, "tail_r_sec1", (0.35, 24.25, 4.52), (1.65, 26.15, 4.88), "linen")
    g.box(kr, "tail_r_fold1", (0.55, 24.55, 4.64), (1.45, 25.85, 4.96), "linen_lt")
    g.box(kr, "tail_r_sec2", (0.55, 22.15, 4.38), (1.85, 24.22, 4.78), "linen_dk")
    g.box(kr, "tail_r_fold2", (0.75, 22.45, 4.52), (1.65, 23.95, 4.88), "linen")
    g.box(kr, "tail_r_sec3", (0.75, 19.85, 4.28), (1.95, 22.12, 4.68), "linen")
    g.box(kr, "tail_r_sec4", (0.95, 17.85, 4.22), (1.85, 19.82, 4.58), "linen_dk")
    g.box(kr, "tail_r_fringe1", (0.85, 16.75, 4.22), (1.35, 17.82, 4.52), "linen_lt")
    g.box(kr, "tail_r_fringe2", (1.45, 17.15, 4.22), (1.85, 17.82, 4.52), "linen")
    g.box(kr, "tail_r_fringe3", (1.15, 16.35, 4.25), (1.65, 17.15, 4.50), "linen_dk")

    kl = "knot_tail_l"
    g.box(kl, "tail_l_sec1", (-1.75, 24.55, 4.52), (-0.45, 26.15, 4.88), "linen")
    g.box(kl, "tail_l_fold1", (-1.55, 24.85, 4.64), (-0.65, 25.95, 4.96), "linen_dk")
    g.box(kl, "tail_l_sec2", (-1.95, 22.65, 4.38), (-0.65, 24.52, 4.78), "linen_dk")
    g.box(kl, "tail_l_fold2", (-1.75, 22.95, 4.52), (-0.85, 24.22, 4.88), "linen_lt")
    g.box(kl, "tail_l_sec3", (-1.85, 20.85, 4.28), (-0.85, 22.62, 4.68), "linen")
    g.box(kl, "tail_l_fringe", (-1.65, 19.95, 4.28), (-1.05, 20.82, 4.58), "linen_lt")
    g.box(kl, "tail_l_fringe2", (-1.55, 19.25, 4.30), (-1.15, 19.95, 4.55), "linen")


def part_neck_wrap(g: Gear) -> None:
    """颈围过渡圈：最高 y=23.92，绝对不进头盒。"""
    b = "neck_wrap"
    g.box(b, "neck_f_top_c", (-3.8, 23.08, -3.42), (3.8, 23.92, -2.16), "linen")
    g.box(b, "neck_f_top_crest", (-3.2, 23.35, -3.68), (3.2, 23.88, -2.62), "linen_lt")
    g.box(b, "neck_f_top_fold_r", (0.8, 23.15, -3.75), (2.8, 23.82, -2.85), "linen")
    g.box(b, "neck_f_top_fold_l", (-2.8, 23.15, -3.75), (-0.8, 23.82, -2.85), "linen_dk")

    g.box(b, "neck_f_bot_c", (-3.72, 22.25, -3.32), (3.72, 23.05, -2.16), "linen_dk")
    g.box(b, "neck_f_bot_crest", (-3.0, 22.42, -3.55), (3.0, 22.92, -2.55), "linen")
    g.box(b, "neck_f_bot_shadow", (-3.4, 22.22, -3.42), (3.4, 22.48, -2.42), "linen_dk")

    g.box(b, "neck_b_main", (-3.8, 22.58, 2.14), (3.8, 23.92, 3.12), "linen")
    g.box(b, "neck_b_fold_hi", (-3.4, 23.25, 2.65), (3.4, 23.88, 3.32), "linen_lt")
    g.box(b, "neck_b_fold_lo", (-3.5, 22.55, 2.52), (3.5, 23.15, 3.22), "linen_dk")


def part_bib_drape(g: Gear) -> None:
    """胸前大三角巾下摆：多段阶梯层叠收尖，表面带多重纵向垂褶波浪与撕裂齿。"""
    bc = "bib_chest"
    # ── 第 1 梯级（上胸宽展层 y: 20.68 .. 22.55）
    g.box(bc, "bib_tier1_base", (-3.75, 20.68, -2.62), (3.75, 22.55, -2.18), "linen")
    g.box(bc, "bib_tier1_crest_c", (-2.2, 21.15, -2.76), (2.2, 22.25, -2.27), "linen_lt")
    g.pair(bc, "bib_tier1_pleat_side", (1.2, 20.85, -2.84), (3.2, 22.35, -2.25), "linen")
    g.pair(bc, "bib_tier1_roll_edge", (3.25, 20.65, -2.74), (3.78, 22.15, -2.22), "linen_dk")

    # ── 第 2 梯级（中胸收敛层 y: 18.25 .. 20.65）
    g.box(bc, "bib_tier2_base", (-3.42, 18.28, -2.58), (3.42, 20.65, -2.18), "linen_dk")
    g.box(bc, "bib_tier2_crest_c", (-1.55, 18.68, -2.75), (1.55, 20.42, -2.25), "linen")
    g.pair(bc, "bib_tier2_pleat_mid", (1.62, 18.45, -2.82), (2.85, 20.35, -2.24), "linen_lt")
    g.box(bc, "bib_tier2_shadow", (-3.1, 18.22, -2.52), (3.1, 19.08, -2.2), "linen_dk")
    g.pair(bc, "bib_drape_rib_a", (0.58, 18.35, -2.82), (1.18, 21.25, -2.28), "linen")
    g.pair(bc, "bib_drape_rib_b", (1.85, 18.15, -2.8), (2.55, 20.85, -2.26), "linen_lt")

    # ── 第 3 梯级（下胸三角中段 y: 16.05 .. 18.22）
    g.box(bc, "bib_tier3_base", (-2.82, 16.05, -2.52), (2.82, 18.22, -2.18), "linen")
    g.box(bc, "bib_tier3_crest_c", (-1.2, 16.35, -2.68), (1.2, 18.15, -2.25), "linen_lt")
    g.pair(bc, "bib_tier3_edge", (1.52, 16.15, -2.62), (2.72, 18.05, -2.2), "linen_dk")
    g.pair(bc, "bib_drape_rib_c", (0.45, 16.15, -2.76), (0.95, 18.25, -2.26), "linen_lt")

    # ── 第 4 梯级（腹前三角收尖层 y: 14.55 .. 16.02）
    g.box(bc, "bib_tier4_base", (-1.82, 14.55, -2.48), (1.82, 16.02, -2.18), "linen_dk")
    g.box(bc, "bib_tier4_crest_c", (-0.95, 14.75, -2.62), (0.95, 15.85, -2.25), "linen")
    g.pair(bc, "bib_tier4_flank", (1.05, 14.65, -2.55), (1.75, 15.65, -2.22), "linen_lt")
    g.pair(bc, "bib_tier4_rib", (0.28, 14.62, -2.68), (0.75, 15.75, -2.24), "linen")
    g.pair(bc, "bib_tier3_accent", (1.82, 16.55, -2.78), (2.42, 17.45, -2.25), "linen_lt")
    g.pair(bc, "bib_tier2_accent", (2.15, 19.15, -2.84), (2.75, 20.15, -2.28), "linen")

    # ── 中央下垂撕裂尖端（骨骼 bib_tip_drape）
    bt = "bib_tip_drape"
    g.box(bt, "tip_point_main", (-0.75, 13.55, -2.44), (0.75, 14.52, -2.18), "linen")
    g.box(bt, "tip_point_crest", (-0.45, 13.75, -2.54), (0.45, 14.42, -2.26), "linen_lt")
    g.box(bt, "tip_shred_a", (-0.68, 13.02, -2.48), (-0.15, 13.65, -2.22), "linen_dk")
    g.box(bt, "tip_shred_b", (0.15, 13.22, -2.46), (0.62, 13.65, -2.24), "linen_lt")
    g.box(bt, "tip_fringe_mid", (-0.35, 12.65, -2.42), (0.35, 13.25, -2.20), "linen")
    g.box(bt, "tip_fringe_sub", (-0.18, 12.28, -2.40), (0.22, 12.72, -2.18), "linen_dk")

    # ── 右侧撕裂边缘与破损布角（骨骼 bib_fray_r）
    fr = "bib_fray_r"
    g.box(fr, "fray_r_flap1", (2.42, 17.22, -2.56), (3.65, 18.82, -2.21), "linen")
    g.box(fr, "fray_r_flap2", (2.12, 15.82, -2.5), (3.25, 17.22, -2.23), "linen_lt")
    g.box(fr, "fray_r_tooth1", (2.62, 14.98, -2.46), (3.18, 15.82, -2.21), "linen_dk")
    g.box(fr, "fray_r_tooth2", (1.82, 14.18, -2.44), (2.48, 15.22, -2.23), "linen")
    g.box(fr, "fray_r_tooth3", (1.32, 13.62, -2.48), (1.88, 14.35, -2.25), "linen_dk")
    g.box(fr, "fray_r_fringe", (2.05, 13.52, -2.42), (2.55, 14.25, -2.19), "linen_lt")
    g.box(fr, "fray_r_tooth4", (2.52, 13.85, -2.40), (2.95, 14.65, -2.17), "linen")

    # ── 左侧撕裂边缘与破损布角（骨骼 bib_fray_l）
    fl = "bib_fray_l"
    g.box(fl, "fray_l_flap1", (-3.65, 17.48, -2.56), (-2.32, 19.12, -2.21), "linen_dk")
    g.box(fl, "fray_l_flap2", (-3.35, 16.18, -2.5), (-2.02, 17.48, -2.23), "linen")
    g.box(fl, "fray_l_tooth1", (-3.18, 15.28, -2.46), (-2.42, 16.18, -2.21), "linen_lt")
    g.box(fl, "fray_l_tooth2", (-2.58, 14.48, -2.44), (-1.92, 15.48, -2.23), "linen_dk")
    g.box(fl, "fray_l_tooth3", (-2.02, 13.82, -2.48), (-1.45, 14.55, -2.25), "linen")
    g.box(fl, "fray_l_fringe", (-2.38, 13.72, -2.42), (-1.82, 14.45, -2.19), "linen_lt")
    g.box(fl, "fray_l_tooth4", (-2.95, 13.95, -2.40), (-2.52, 14.75, -2.17), "linen_dk")


def part_mask_details(g: Gear) -> None:
    """表面补丁、破洞暗衬与缝线细节。"""
    d = "mask_details"
    # ── 面巾正面右下破洞
    g.box(d, "hole_backing", (1.35, 25.25, -4.72), (2.75, 26.25, -4.5), "void")
    g.box(d, "hole_fray_top", (1.15, 26.15, -4.84), (2.65, 26.42, -4.56), "linen_lt")
    g.box(d, "hole_fray_bot", (1.45, 25.06, -4.82), (2.85, 25.35, -4.56), "linen_dk")
    g.box(d, "hole_fray_l", (1.12, 25.32, -4.82), (1.42, 26.18, -4.6), "linen")
    g.box(d, "hole_fray_r", (2.68, 25.32, -4.8), (2.98, 26.18, -4.54), "linen_lt")
    g.box(d, "hole_stitch_a", (1.75, 25.12, -4.88), (2.02, 26.35, -4.64), "stitch")
    g.box(d, "hole_stitch_b", (2.25, 25.12, -4.88), (2.52, 26.35, -4.64), "stitch")

    # ── 面巾左鼻翼粗皮补丁
    g.box(d, "patch_nose_base", (-2.65, 26.78, -4.86), (-1.05, 28.12, -4.42), "patch")
    g.box(d, "patch_nose_shade", (-2.78, 26.68, -4.78), (-2.35, 27.95, -4.46), "patch_dk")
    g.box(d, "patch_nose_worn", (-2.15, 27.15, -4.88), (-1.25, 27.85, -4.52), "patch_lt")
    g.box(d, "patch_nose_curl", (-1.15, 27.92, -4.92), (-0.85, 28.25, -4.54), "patch_lt")
    g.box(d, "patch_stitch_top", (-2.45, 28.05, -4.88), (-1.15, 28.28, -4.62), "stitch")
    g.box(d, "patch_stitch_bot", (-2.45, 26.56, -4.88), (-1.15, 26.82, -4.62), "stitch")
    g.box(d, "patch_stitch_l", (-2.74, 26.85, -4.88), (-2.48, 27.85, -4.62), "stitch")
    g.box(d, "patch_stitch_r", (-1.15, 26.85, -4.88), (-0.88, 27.85, -4.62), "stitch")

    # ── 胸前下摆中央的大块深色兽皮补丁
    g.box(d, "chest_patch_base", (-1.85, 18.62, -2.78), (0.85, 20.62, -2.32), "patch")
    g.box(d, "chest_patch_rim", (-1.95, 18.48, -2.70), (0.95, 18.76, -2.35), "patch_dk")
    g.box(d, "chest_patch_worn", (-1.45, 19.15, -2.86), (0.45, 20.15, -2.42), "patch_lt")
    g.box(d, "chest_patch_fold", (-0.85, 19.45, -2.92), (0.15, 20.48, -2.46), "patch")
    g.box(d, "chest_stitch_h1", (-1.65, 20.35, -2.88), (-0.75, 20.58, -2.42), "stitch")
    g.box(d, "chest_stitch_h2", (-0.25, 20.35, -2.88), (0.65, 20.58, -2.42), "stitch")
    g.box(d, "chest_stitch_b1", (-1.65, 18.82, -2.88), (-0.75, 19.05, -2.42), "stitch")
    g.box(d, "chest_stitch_b2", (-0.25, 18.82, -2.88), (0.65, 19.05, -2.42), "stitch")
    g.box(d, "chest_stitch_v1", (-1.82, 19.25, -2.88), (-1.55, 20.15, -2.42), "stitch")
    g.box(d, "chest_stitch_v2", (0.55, 19.25, -2.88), (0.82, 20.15, -2.42), "stitch")

    # ── 右侧胸前撕裂拼接粗麻线迹
    g.box(d, "seam_r_plate", (1.45, 16.85, -2.66), (2.05, 21.25, -2.32), "linen_dk")
    for k, y in enumerate((17.2, 18.0, 18.8, 19.6, 20.4)):
        g.box(d, f"seam_r_x_{k}", (1.25, y, -2.74), (2.25, y + 0.35, -2.36), "stitch")

    # ── 左侧下摆磨损小补片
    g.box(d, "patch_l_mini", (-2.85, 15.65, -2.66), (-1.65, 17.05, -2.34), "patch")
    g.box(d, "patch_l_mini_s1", (-2.95, 16.25, -2.68), (-1.55, 16.55, -2.36), "stitch")
    g.box(d, "patch_l_mini_s2", (-2.45, 15.45, -2.74), (-2.05, 17.25, -2.42), "stitch")
    # 额外表面粗缝补线（丰富质感）
    for k, y in enumerate((14.8, 15.6, 16.4)):
        g.box(d, f"bib_patch_cross_{k}", (-0.6, y, -2.72), (0.6, y + 0.35, -2.35), "stitch")


def part_trinket_bone(g: Gear) -> None:
    """右颊挂件：挂绳 + 锈铁小环 + 打磨兽骨针。"""
    hr = "hang_iron_ring"
    g.box(hr, "ring_cord", (4.22, 25.42, -2.75), (4.48, 26.55, -2.45), "cord")
    g.box(hr, "iron_ring_top", (4.16, 24.82, -2.95), (4.74, 25.45, -2.25), "rust")
    g.box(hr, "iron_ring_bot", (4.16, 24.12, -2.95), (4.74, 24.65, -2.25), "rust")
    g.box(hr, "iron_ring_side1", (4.14, 24.52, -3.05), (4.76, 24.95, -2.76), "rust")
    g.box(hr, "iron_ring_side2", (4.14, 24.52, -2.44), (4.76, 24.95, -2.15), "rust")

    hb = "hang_bone_charm"
    g.box(hb, "bone_knot", (4.25, 23.75, -2.85), (4.65, 24.15, -2.35), "cord_dk")
    g.box(hb, "bone_head", (4.15, 22.95, -2.92), (4.75, 23.75, -2.22), "bone")
    g.box(hb, "bone_body", (4.25, 21.55, -2.85), (4.68, 22.95, -2.3), "bone")
    g.box(hb, "bone_tip", (4.32, 20.35, -2.78), (4.65, 21.55, -2.38), "bone_lt")
    g.box(hb, "bone_shade", (4.18, 21.75, -2.88), (4.52, 23.15, -2.35), "bone_dk")


PART_FUNCS = {
    "mask_face": part_mask_face,
    "mask_cheeks": part_mask_cheeks,
    "mask_knots": part_mask_knots,
    "neck_wrap": part_neck_wrap,
    "bib_drape": part_bib_drape,
    "mask_details": part_mask_details,
    "trinket_bone": part_trinket_bone,
}

SPEC = GearSpec(
    gear_id="cu_bu_mian_jin",
    model_name="CuBuMianJin",
    title="粗布面巾 / cu_bu_mian_jin",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：口鼻 / 颈 / 前胸三角巾下摆（头面部 y∈[24,28.8] + 颈胸下摆 y∈[12.6,23.95]）",
    worn_focus=((0.0, 23.0, 0.0), 24.0),
    texture_overrides={"stitch": "gear_stitch_lt"},  # 画笔库里 gear_stitch 已是腰挂件的深线，这里用浅粗线
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
