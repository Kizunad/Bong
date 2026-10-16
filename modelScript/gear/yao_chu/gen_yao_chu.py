#!/usr/bin/env python3
"""药锄腰后挂架（yao_chu）——空皮套 + 草药捆版（移除手持药锄本体）。

对应 NPC：采药人（cai_yao_ren）。
根据用户要求：药锄本体单独做成手持工具（yao_chu_tool），本件保留腰后挂架（空皮套 + 草药捆）。
挂点：后腰中线偏右（x∈[-3.5, 3.8], y∈[5.2, 17.5], z∈[2.08, 4.95]）。
避让：避开右胯灵草囊（x>4.2, z<0）、避开左胯防潮包（x<-4.0, z<0），避开躯干与双腿（z >= 2.08 在后方）。

方块数 ≥200，分骨 ≥10（带几何可动骨 ≥10，挂件单独一骨）。
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "modelScript" / "gear" / "_kit"))

import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec  # noqa: E402

MATS = {
    "hide": (106, 70, 48),
    "hide_dk": (70, 48, 31),
    "hide_lt": (138, 100, 62),
    "iron": (88, 80, 76),
    "iron_lt": (128, 120, 112),
    "rust": (122, 66, 36),
    "rust_dk": (78, 42, 26),
    "bone": (220, 212, 192),
    "bone_lt": (235, 228, 210),
    "bone_dk": (175, 160, 138),
    "bone_stain": (150, 135, 115),
    "cord": (168, 140, 88),
    "cord_dk": (122, 96, 56),
    "straw": (140, 114, 68),
    "straw_dk": (94, 72, 40),
    "straw_lt": (168, 142, 88),
    "stitch": (188, 174, 148),
    "void": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_mount", "root", (0.0, 12.8, 2.4), "后腰对称皮带与对称带扣"),
    ("holster_base", "belt_mount", (2.2, 11.5, 3.2), "后腰右侧加宽大号空皮套筒"),
    ("holster_strap", "holster_base", (2.2, 12.8, 3.4), "空套筒加固搭扣与金属眼"),
    ("holster_lining", "holster_base", (2.2, 11.8, 3.2), "空套筒内部防磨硬衬与插口边框"),
    ("herb_bundle_main", "belt_mount", (-0.8, 11.2, 3.4), "占半后腰蓬松大草药捆主体"),
    ("herb_bundle_ties", "herb_bundle_main", (-0.8, 11.4, 3.4), "草药捆双道加粗草绳紧扎与打结"),
    ("herb_leaves_mid", "herb_bundle_main", (-0.8, 14.5, 3.5), "草药捆直挺主茎与羽状叶丛"),
    ("herb_leaves_outer", "herb_leaves_mid", (-0.8, 15.2, 3.5), "大幅蓬松外散干燥羽叶与翻卷叶尖"),
    ("herb_seed_heads", "herb_leaves_mid", (-0.8, 16.2, 3.4), "干燥灵草籽粒穗头与顶端茸毛"),
    ("herb_roots_main", "herb_bundle_main", (-0.8, 8.5, 3.4), "草药捆底部粗壮主根团与附着泥块"),
    ("herb_roots_drape", "herb_roots_main", (-0.8, 6.5, 3.4), "向下垂散外露的细长泥渍粗须根簇"),
    ("hoe_tally_tag", "holster_base", (3.2, 11.2, 3.8), "辨药骨符与挂带"),
    ("tally_beads", "hoe_tally_tag", (3.4, 9.2, 3.8), "辟邪骨珠与下垂双色麻绳流苏"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "belt_mount")])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("bone", "bone_lt"), ("bone", "bone_stain"), ("hide", "hide_dk"),
    ("hide", "hide_lt"), ("cord", "cord_dk"), ("straw", "straw_dk"),
    ("straw", "straw_lt"), ("iron", "rust"), ("rust", "rust_dk"),
    ("iron", "iron_lt"), ("hide", "cord"), ("hide", "stitch"),
    ("hide", "iron"), ("hide", "rust"), ("hide", "rust_dk"),
    ("hide_dk", "rust"), ("hide_dk", "rust_dk"), ("hide_lt", "rust"),
    ("hide", "bone"), ("iron", "cord"), ("straw", "cord"),
    ("bone", "void"), ("iron", "bone"), ("cord_dk", "rust"),
    ("cord", "rust"), ("straw_dk", "hide_dk"), ("straw", "hide_dk"),
    ("straw_lt", "hide_dk"), ("straw", "straw_dk"), ("straw", "straw_lt"),
    ("straw_lt", "straw_dk"), ("cord", "straw_lt"), ("cord_dk", "straw_dk"),
    ("bone", "cord"), ("bone_lt", "cord"), ("bone_stain", "cord"),
    ("void", "hide_dk"), ("void", "iron"), ("void", "rust"),
))


# ── 1. 后腰对称皮带与对称带扣（完全对称骨骼，满足 mirror 自检）
def part_belt(g: Gear) -> None:
    """后腰对称皮带与对称带扣。"""
    b = "belt_mount"
    g.box(b, "belt_center_core", (-1.25, 12.15, 2.08), (1.25, 13.65, 2.49), "hide_dk")
    g.box(b, "belt_center_cover", (-0.98, 12.22, 2.42), (0.98, 13.58, 2.72), "hide")
    g.box(b, "belt_center_bone_pin", (-0.35, 12.02, 2.68), (0.35, 13.78, 2.98), "bone")
    g.box(b, "belt_pin_head", (-0.28, 13.74, 2.71), (0.28, 14.05, 2.96), "bone_lt")
    g.box(b, "belt_pin_tail", (-0.25, 11.75, 2.71), (0.25, 12.06, 2.96), "bone_stain")

    # 左右对称成对延伸皮带
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.14), (4.48, 13.62, 2.44), "hide_dk")
    g.pair(b, "belt_wing_outer", (1.24, 12.25, 2.41), (4.44, 13.55, 2.68), "hide")
    g.pair(b, "belt_edge_top", (1.22, 13.58, 2.13), (4.46, 13.88, 2.46), "hide_dk")
    g.pair(b, "belt_edge_bot", (1.22, 11.92, 2.13), (4.46, 12.22, 2.52), "hide_dk")

    # 对称骨铆钉与粗缝线 (dx, dy, dz >= 0.26)
    g.pair(b, "belt_rivet_mid", (1.82, 12.70, 2.65), (2.18, 13.10, 2.95), "bone")
    g.pair(b, "belt_rivet_lat", (3.22, 12.70, 2.65), (3.58, 13.10, 2.95), "bone")
    g.pair(b, "belt_stitch_1", (1.32, 13.22, 2.65), (1.68, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_2", (2.52, 13.22, 2.63), (2.88, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_3", (3.82, 13.22, 2.65), (4.18, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_b1", (1.32, 12.26, 2.65), (1.68, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.65), (2.88, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")

    # 对称侧面挂带环与骨扣
    g.pair(b, "belt_sub_clasp", (2.05, 12.60, 2.72), (2.42, 13.20, 3.02), "hide_dk")
    g.pair(b, "belt_sub_ring", (3.05, 12.60, 2.72), (3.42, 13.20, 3.02), "rust")


# ── 2. 后腰右侧加宽大号空皮套筒（空套筒！有明确的空腔开口和深度）
def part_holster_base(g: Gear) -> None:
    """后腰右侧加宽大号空皮套筒。"""
    b = "holster_base"
    # 中空皮套外四壁 (容纳宽大锄头銎的空腔：x∈[1.35, 3.15], z∈[2.52, 4.45])
    g.box(b, "holster_wall_back", (1.05, 9.64, 2.12), (3.45, 13.56, 2.45), "hide_dk")
    g.box(b, "holster_wall_front", (1.05, 9.49, 4.35), (3.45, 13.65, 4.72), "hide")
    g.box(b, "holster_wall_left", (0.85, 9.55, 2.38), (1.22, 13.60, 4.48), "hide_lt")
    g.box(b, "holster_wall_right", (3.27, 9.61, 2.38), (3.59, 13.61, 4.48), "hide_dk")

    # 套筒上下加固封边翻唇
    g.box(b, "holster_rim_top_f", (0.75, 13.48, 4.28), (3.75, 13.95, 4.82), "hide_dk")
    g.box(b, "holster_rim_top_b", (0.75, 13.48, 2.05), (3.75, 13.95, 2.52), "hide_dk")
    g.box(b, "holster_rim_bot", (0.78, 9.25, 2.05), (3.72, 9.75, 4.82), "hide_dk")

    # 套筒前胸防护垫与背面防磨厚革
    g.box(b, "holster_front_pad", (1.25, 10.29, 4.58), (3.25, 13.05, 4.92), "hide_lt")
    g.box(b, "holster_back_pad_reinforce", (1.25, 10.35, 2.05), (3.25, 13.05, 2.32), "hide_dk")

    # 双列粗针脚缝线
    g.box(b, "holster_stitch_l", (0.92, 9.85, 4.52), (1.35, 13.25, 4.82), "stitch")
    g.box(b, "holster_stitch_r", (2.95, 9.85, 4.52), (3.38, 13.27, 4.82), "stitch")

    # 四角加固角皮与铜铆钉 (8 块)
    g.box(b, "holster_corner_pad_1", (0.82, 12.55, 4.45), (1.28, 13.20, 4.85), "hide_dk")
    g.box(b, "holster_corner_pad_2", (3.02, 12.55, 4.45), (3.48, 13.25, 4.85), "hide_dk")
    g.box(b, "holster_corner_pad_3", (0.82, 9.95, 4.45), (1.31, 10.65, 4.85), "hide_dk")
    g.box(b, "holster_corner_pad_4", (3.02, 9.95, 4.45), (3.48, 10.65, 4.85), "hide_dk")
    g.box(b, "holster_brass_nail_1", (0.95, 12.85, 4.75), (1.22, 13.15, 5.05), "iron_lt")
    g.box(b, "holster_brass_nail_2", (3.08, 12.85, 4.75), (3.35, 13.15, 5.05), "iron_lt")
    g.box(b, "holster_brass_nail_3", (0.95, 10.15, 4.75), (1.22, 10.45, 5.05), "iron_lt")
    g.box(b, "holster_brass_nail_4", (3.08, 10.15, 4.75), (3.35, 10.45, 5.05), "iron_lt")


# ── 3. 空套筒加固搭扣与金属眼
def part_holster_strap(g: Gear) -> None:
    """空套筒加固搭扣与金属眼。"""
    b = "holster_strap"
    # 横扎皮扣带 (原本扣住锄头，空着时自然扣在皮套表面)
    g.box(b, "hstrap_front_band", (0.75, 11.85, 4.62), (3.75, 12.65, 4.95), "hide_dk")
    g.box(b, "hstrap_back_band", (0.75, 11.85, 2.10), (3.75, 12.65, 2.38), "hide_dk")
    g.box(b, "hstrap_iron_buckle", (1.85, 11.75, 4.88), (2.65, 12.75, 5.25), "rust")
    g.box(b, "hstrap_iron_tongue", (2.08, 11.65, 4.95), (2.35, 12.85, 5.35), "iron")
    g.box(b, "hstrap_leather_tongue", (1.95, 10.25, 4.82), (2.48, 11.95, 5.15), "hide")
    g.box(b, "hstrap_eyelet_metal", (2.05, 10.55, 5.02), (2.38, 11.05, 5.28), "iron_lt")

    # 悬挂侧环与固定铁钉
    g.box(b, "hstrap_suspension_ring", (0.68, 12.85, 3.25), (0.99, 13.45, 3.85), "rust_dk")
    g.box(b, "hstrap_ring_clasp", (0.78, 12.95, 3.35), (1.15, 13.35, 3.75), "iron_lt")


# ── 4. 空套筒内部防磨硬衬与插口边框（空腔开口与骨质防磨板）
def part_holster_lining(g: Gear) -> None:
    """空套筒内部防磨硬衬与插口边框。"""
    b = "holster_lining"
    # 空套筒内深暗腔底（视觉上清晰看到中空的开口）
    g.box(b, "lining_interior_void", (1.28, 10.25, 2.40), (3.22, 13.51, 4.35), "void")
    # 套筒口内衬防磨骨护条 (4 条环绕口部，防止铁锄锋刃切破皮套)
    g.box(b, "lining_bone_rim_f", (1.22, 13.35, 4.15), (3.28, 13.78, 4.42), "bone_lt")
    g.box(b, "lining_bone_rim_b", (1.17, 13.35, 2.42), (3.28, 13.77, 2.66), "bone_dk")
    g.box(b, "lining_bone_rim_l", (1.15, 13.38, 2.58), (1.38, 13.73, 4.25), "bone")
    g.box(b, "lining_bone_rim_r", (3.12, 13.38, 2.58), (3.35, 13.75, 4.25), "bone")

    # 口部外侧铁护角与防豁裂铁锔钉
    g.box(b, "lining_iron_clamp_l", (0.78, 13.25, 4.25), (1.25, 13.85, 4.65), "iron")
    g.box(b, "lining_iron_clamp_r", (3.25, 13.25, 4.25), (3.72, 13.85, 4.65), "rust")
    g.box(b, "lining_cramp_pin", (2.12, 13.55, 4.58), (2.48, 13.88, 4.88), "iron_lt")


# ── 5. 占半后腰蓬松大草药捆主体（极度厚重饱满）
def part_herb_main(g: Gear) -> None:
    """占半后腰蓬松大草药捆主体。"""
    b = "herb_bundle_main"
    # 大椭圆粗草药束主体 (x: -2.75 ~ 0.85 宽达 3.6px，z: 2.35 ~ 4.45 厚达 2.1px)
    g.box(b, "herb_sheaf_core", (-2.65, 9.85, 2.39), (0.75, 13.25, 4.35), "straw")
    g.box(b, "herb_sheaf_front", (-2.55, 10.05, 4.25), (0.65, 13.05, 4.65), "straw_lt")
    g.box(b, "herb_sheaf_back", (-2.55, 10.05, 2.15), (0.65, 13.05, 2.55), "straw_dk")
    g.box(b, "herb_sheaf_side_l", (-2.85, 10.05, 2.55), (-2.51, 13.05, 4.25), "straw_dk")
    g.box(b, "herb_sheaf_side_r", (0.49, 10.05, 2.55), (0.95, 13.05, 4.25), "straw_lt")

    # 外层纵横多色干燥草药茎条
    g.box(b, "herb_stem_stripe_1", (-2.25, 9.55, 4.46), (-1.55, 13.55, 4.85), "straw_dk")
    g.box(b, "herb_stem_stripe_2", (-1.35, 9.55, 4.52), (-0.65, 13.55, 4.80), "straw_lt")
    g.box(b, "herb_stem_stripe_3", (-0.45, 9.55, 4.52), (0.25, 13.55, 4.85), "straw")
    g.box(b, "herb_stem_stripe_4", (-2.05, 9.55, 2.05), (-1.35, 13.55, 2.35), "straw_dk")
    g.box(b, "herb_stem_stripe_5", (-1.15, 9.55, 2.09), (-0.45, 13.55, 2.35), "straw_lt")


# ── 6. 草药捆双道加粗草绳紧扎与打结
def part_herb_ties(g: Gear) -> None:
    """草药捆双道加粗草绳紧扎与打结。"""
    b = "herb_bundle_ties"
    # 上道紧扎粗草绳 (y∈[12.25, 12.85])
    g.box(b, "herb_tie_upper_f", (-2.75, 12.29, 4.55), (0.85, 12.85, 4.88), "cord_dk")
    g.box(b, "herb_tie_upper_b", (-2.75, 12.25, 2.02), (0.85, 12.85, 2.32), "cord_dk")
    g.box(b, "herb_tie_upper_l", (-2.95, 12.27, 2.22), (-2.55, 12.79, 4.68), "cord")
    g.box(b, "herb_tie_upper_r", (0.69, 12.21, 2.22), (1.00, 12.91, 4.68), "cord")
    g.box(b, "herb_tie_upper_knot", (-2.98, 12.05, 3.15), (-2.42, 12.95, 3.85), "cord_dk")

    # 下道紧扎粗草绳 (y∈[10.35, 10.95])
    g.box(b, "herb_tie_lower_f", (-2.75, 10.35, 4.55), (0.85, 10.95, 4.88), "cord")
    g.box(b, "herb_tie_lower_b", (-2.75, 10.35, 2.02), (0.85, 10.95, 2.32), "cord_dk")
    g.box(b, "herb_tie_lower_l", (-2.95, 10.41, 2.22), (-2.55, 10.98, 4.68), "cord_dk")
    g.box(b, "herb_tie_lower_r", (0.65, 10.39, 2.22), (1.05, 11.01, 4.68), "cord")
    g.box(b, "herb_tie_lower_knot", (-2.98, 10.15, 3.15), (-2.42, 11.05, 3.85), "cord")

    # 双股垂落绳尾
    g.box(b, "herb_tie_tail_1", (-2.95, 9.25, 3.25), (-2.55, 10.33, 3.55), "cord")
    g.box(b, "herb_tie_tail_2", (-2.90, 8.85, 3.45), (-2.45, 10.35, 3.75), "cord_dk")


# ── 7. 草药捆直挺主茎与羽状叶丛（中央主叶丛）
def part_herb_leaves_mid(g: Gear) -> None:
    """草药捆直挺主茎与羽状叶丛。"""
    b = "herb_leaves_mid"
    g.box(b, "leaf_main_crest", (-1.45, 13.15, 2.85), (-0.45, 15.65, 3.85), "straw_lt")
    g.box(b, "leaf_spike_front", (-1.75, 13.05, 4.25), (-0.25, 15.25, 4.82), "straw")
    g.box(b, "leaf_spike_back", (-1.75, 13.05, 2.10), (-0.25, 15.25, 2.67), "straw_dk")
    g.box(b, "leaf_spike_left", (-3.25, 13.05, 2.62), (-2.11, 15.05, 4.05), "straw_dk")
    g.box(b, "leaf_spike_right", (0.25, 13.05, 2.63), (1.37, 15.05, 4.05), "straw_lt")

    # 粗挺草茎
    g.box(b, "leaf_stalk_core_1", (-1.85, 13.55, 3.15), (-1.35, 15.85, 3.65), "straw_dk")
    g.box(b, "leaf_stalk_core_2", (-0.65, 13.55, 3.15), (-0.15, 15.85, 3.65), "straw")


# ── 8. 大幅蓬松外散干燥羽叶与翻卷叶尖（向四周极度散开）
def part_herb_leaves_outer(g: Gear) -> None:
    """大幅蓬松外散干燥羽叶与翻卷叶尖。"""
    b = "herb_leaves_outer"
    # 外展的大羽叶片与翻卷叶尖 (蓬松感核心，横跨至 x=-3.6)
    g.box(b, "herb_frond_flare_l1", (-3.55, 14.15, 2.85), (-2.75, 15.85, 3.75), "straw_lt")
    g.box(b, "herb_frond_flare_l2", (-3.15, 15.35, 3.05), (-2.61, 16.45, 3.71), "straw")
    g.box(b, "herb_frond_flare_r1", (0.81, 14.15, 2.85), (1.55, 15.85, 3.75), "straw_dk")
    g.box(b, "herb_frond_flare_r2", (0.55, 15.35, 3.05), (1.20, 16.45, 3.65), "straw_lt")

    # 外翻大叶尖簇
    g.box(b, "herb_frond_curl_1", (-3.10, 14.55, 3.15), (-2.55, 15.65, 3.65), "straw_lt")
    g.box(b, "herb_frond_curl_2", (0.45, 14.55, 3.15), (1.05, 15.65, 3.62), "straw_lt")
    g.box(b, "herb_frond_curl_3", (-2.45, 15.05, 2.45), (-1.85, 16.05, 2.95), "straw_dk")
    g.box(b, "herb_frond_curl_4", (-0.85, 15.05, 2.45), (-0.21, 16.05, 2.95), "straw_dk")

    # 四角外探细叶尖
    g.box(b, "herb_frond_tip_x1", (-3.35, 13.55, 3.35), (-2.85, 14.25, 3.85), "straw_dk")
    g.box(b, "herb_frond_tip_x2", (0.91, 13.55, 3.35), (1.35, 14.25, 3.85), "straw_lt")
    g.box(b, "herb_frond_tip_x3", (-2.65, 13.25, 4.35), (-2.15, 13.95, 4.75), "straw_lt")
    g.box(b, "herb_frond_tip_x4", (-0.55, 13.25, 4.35), (-0.05, 13.95, 4.75), "straw")
    g.box(b, "herb_frond_tip_x5", (-2.65, 13.25, 2.15), (-2.15, 13.95, 2.55), "straw_dk")
    g.box(b, "herb_frond_tip_x6", (-0.55, 13.25, 2.15), (-0.05, 13.95, 2.55), "straw_dk")

    # 上翘大羽叶垫
    g.box(b, "herb_leaf_puff_1", (-2.85, 13.65, 3.85), (-2.25, 14.95, 4.45), "straw_lt")
    g.box(b, "herb_leaf_puff_2", (-0.15, 13.65, 3.85), (0.45, 14.95, 4.45), "straw_lt")
    g.box(b, "herb_leaf_puff_3", (-2.85, 13.65, 2.09), (-2.25, 14.95, 2.75), "straw_dk")
    g.box(b, "herb_leaf_puff_4", (-0.15, 13.65, 2.21), (0.45, 14.95, 2.75), "straw_dk")
    g.box(b, "herb_leaf_puff_5", (-1.88, 15.25, 2.65), (-1.25, 16.35, 3.25), "straw")
    g.box(b, "herb_leaf_puff_6", (-1.28, 15.25, 3.30), (-0.65, 16.35, 3.95), "straw_lt")


# ── 9. 干燥灵草籽粒穗头与顶端茸毛（籽穗丛）
def part_herb_seed_heads(g: Gear) -> None:
    """干燥灵草籽粒穗头与顶端茸毛。"""
    b = "herb_seed_heads"
    # 干燥灵草籽粒穗头 (高达 y=16.65)
    g.box(b, "herb_seed_spike_1", (-1.25, 15.45, 3.15), (-0.75, 16.55, 3.65), "straw_lt")
    g.box(b, "herb_seed_spike_2", (-1.95, 14.85, 3.35), (-1.45, 15.95, 3.85), "straw")
    g.box(b, "herb_seed_spike_3", (-0.35, 14.85, 2.95), (0.15, 15.95, 3.45), "straw_lt")
    g.box(b, "herb_seed_spike_4", (-1.65, 14.25, 2.35), (-1.15, 15.35, 2.85), "straw_dk")

    # 顶端籽粒小球与干草茸花
    g.box(b, "herb_seed_cluster_a", (-1.65, 15.83, 3.05), (-1.15, 16.65, 3.55), "straw_lt")
    g.box(b, "herb_seed_cluster_b", (-0.95, 15.85, 3.35), (-0.45, 16.65, 3.80), "straw")
    g.box(b, "herb_seed_tuft_1", (-1.15, 16.55, 3.22), (-0.82, 17.15, 3.55), "straw_lt")
    g.box(b, "herb_seed_tuft_2", (-1.85, 15.85, 3.45), (-1.55, 16.45, 3.75), "straw_lt")


# ── 10. 草药捆底部粗壮主根团与附着泥块
def part_herb_roots_main(g: Gear) -> None:
    """草药捆底部粗壮主根团与附着泥块。"""
    b = "herb_roots_main"
    # 底部粗主根团 (y: 7.8 到 10.1)
    g.box(b, "root_tuft_main", (-2.25, 8.25, 2.65), (-0.15, 10.15, 4.15), "straw_dk")
    g.box(b, "root_tuft_front", (-2.05, 8.49, 3.95), (-0.35, 9.85, 4.35), "straw_dk")
    g.box(b, "root_tuft_back", (-2.01, 8.45, 2.35), (-0.35, 9.85, 2.85), "straw_dk")

    # 附着带根泥土泥块 (大泥坨)
    g.box(b, "root_mud_ball_1", (-2.05, 8.41, 2.52), (-1.35, 9.15, 3.22), "hide_dk")
    g.box(b, "root_mud_ball_2", (-1.15, 8.45, 3.62), (-0.45, 9.15, 4.28), "hide_dk")
    g.box(b, "root_mud_ball_3", (-1.65, 7.55, 3.22), (-1.05, 8.15, 3.82), "hide_dk")
    g.box(b, "root_mud_lump_a", (-1.85, 7.15, 2.85), (-1.25, 7.65, 3.35), "hide_dk")
    g.box(b, "root_mud_lump_b", (-0.85, 7.15, 3.15), (-0.25, 7.65, 3.65), "hide_dk")


# ── 11. 向下垂散外露的细长泥渍粗须根簇
def part_herb_roots_drape(g: Gear) -> None:
    """向下垂散外露的细长泥渍粗须根簇。"""
    b = "herb_roots_drape"
    # 5 股长短散开的细长须根
    g.box(b, "root_strand_a", (-2.45, 6.85, 2.75), (-1.85, 8.45, 3.35), "cord_dk")
    g.box(b, "root_strand_b", (-1.75, 6.25, 3.05), (-1.15, 8.45, 3.62), "hide_dk")
    g.box(b, "root_strand_c", (-1.05, 6.55, 2.82), (-0.41, 8.45, 3.45), "straw_dk")
    g.box(b, "root_strand_d", (-0.35, 6.85, 3.09), (0.25, 8.45, 3.75), "cord_dk")
    g.box(b, "root_strand_e", (-1.95, 5.55, 3.15), (-1.45, 6.95, 3.55), "straw_dk")

    # 细尖端与小干泥渣
    g.box(b, "root_strand_tip_1", (-2.15, 5.85, 2.95), (-1.75, 6.75, 3.35), "cord_dk")
    g.box(b, "root_strand_tip_2", (-0.75, 5.85, 3.25), (-0.35, 6.75, 3.65), "straw_dk")
    g.box(b, "root_tail_whisk_1", (-1.85, 5.25, 3.22), (-1.55, 5.75, 3.48), "straw_dk")
    g.box(b, "root_tail_whisk_2", (-1.02, 6.15, 2.92), (-0.72, 6.65, 3.18), "cord_dk")


# ── 12. 辨药骨符与挂带（挂在空套筒外侧）
def part_tally_tag(g: Gear) -> None:
    """辨药骨符与挂带。"""
    b = "hoe_tally_tag"
    # 挂绳圈
    g.box(b, "tally_cord_loop", (3.25, 11.57, 3.59), (3.75, 12.35, 3.95), "cord_dk")
    # 磨光辨药兽骨牌
    g.box(b, "tally_bone_body", (3.15, 10.15, 3.58), (3.85, 11.75, 4.02), "bone")
    g.box(b, "tally_bone_top", (3.20, 11.55, 3.55), (3.80, 11.92, 4.05), "bone_lt")
    g.box(b, "tally_bone_void", (3.35, 11.25, 3.68), (3.65, 11.55, 3.92), "void")
    g.box(b, "tally_cord_knot", (3.32, 9.75, 3.65), (3.68, 10.25, 3.95), "cord_dk")
    g.box(b, "tally_cord_tassel", (3.35, 9.15, 3.68), (3.65, 9.85, 3.92), "cord")


# ── 13. 辟邪骨珠与下垂双色麻绳流苏
def part_tally_beads(g: Gear) -> None:
    """辟邪骨珠与下垂双色麻绳流苏。"""
    b = "tally_beads"
    # 两颗骨珠
    g.box(b, "bead_upper_sphere", (3.32, 8.85, 3.65), (3.68, 9.25, 3.95), "bone_lt")
    g.box(b, "bead_lower_sphere", (3.32, 8.35, 3.65), (3.68, 8.75, 3.95), "bone_stain")
    # 下垂细麻绳流苏
    g.box(b, "bead_strand_a", (3.28, 7.45, 3.58), (3.55, 8.47, 3.85), "cord")
    g.box(b, "bead_strand_b", (3.48, 7.25, 3.75), (3.75, 8.45, 4.05), "cord_dk")
    g.box(b, "bead_knot_end", (3.35, 7.15, 3.65), (3.65, 7.61, 3.95), "cord_dk")


# ── 14. 细节补强（加厚挂架皮带、防磨皮垫与捆药标签，方块数达 205+）
def part_details(g: Gear) -> None:
    """挂架与草药捆细节补强。"""
    # 草药捆侧挂干灵草小药标与吊绳 (6 块)
    hbm = "herb_bundle_main"
    g.box(hbm, "herb_hang_cord_l", (-1.85, 13.25, 2.25), (-1.45, 14.15, 2.65), "cord_dk")
    g.box(hbm, "herb_hang_cord_r", (-1.05, 13.25, 2.25), (-0.65, 14.15, 2.65), "cord")
    g.box(hbm, "herb_charm_bone_root", (-1.65, 11.85, 4.48), (-1.05, 12.65, 4.95), "bone")
    g.box(hbm, "herb_charm_bone_tip", (-1.55, 11.25, 4.58), (-1.15, 11.95, 4.88), "bone_lt")
    g.box(hbm, "herb_charm_tag_cord", (-1.52, 12.55, 4.59), (-1.22, 13.15, 4.85), "cord_dk")
    g.box(hbm, "herb_stem_fluff_top", (-1.95, 13.45, 4.45), (-1.25, 14.05, 4.88), "straw_lt")

    # 套筒外皮加强护带与铜钉 (8 块)
    h_part = "holster_base"
    g.box(h_part, "holster_stud_extra_1", (1.55, 10.38, 4.75), (1.85, 10.75, 5.05), "iron_lt")
    g.box(h_part, "holster_stud_extra_2", (2.45, 10.35, 4.75), (2.75, 10.75, 5.05), "iron_lt")
    g.box(h_part, "holster_side_patch_l", (0.82, 11.25, 2.65), (1.18, 12.45, 4.19), "hide")
    g.box(h_part, "holster_side_patch_r", (3.32, 11.28, 2.65), (3.68, 12.45, 4.15), "hide")
    g.box(h_part, "holster_rivet_patch_1", (0.95, 11.65, 2.45), (1.25, 12.05, 2.75), "iron_lt")
    g.box(h_part, "holster_rivet_patch_2", (3.25, 11.65, 2.45), (3.55, 12.05, 2.75), "iron_lt")
    g.box(h_part, "holster_rear_loop_1", (1.31, 12.65, 2.09), (1.65, 13.35, 2.35), "hide_dk")
    g.box(h_part, "holster_rear_loop_2", (2.85, 12.65, 2.09), (3.29, 13.35, 2.35), "hide_dk")

    # 草药捆与皮套中间的连接粗麻绳索 (6 块)
    h_tie = "herb_bundle_ties"
    g.box(h_tie, "tie_bridge_band_1", (0.55, 12.35, 2.45), (1.15, 12.75, 2.85), "cord_dk")
    g.box(h_tie, "tie_bridge_band_2", (0.50, 12.35, 3.85), (1.15, 12.75, 4.20), "cord")
    g.box(h_tie, "tie_bridge_knot_a", (0.75, 12.23, 2.52), (1.05, 12.85, 2.95), "cord")
    g.box(h_tie, "tie_bridge_knot_b", (0.75, 12.19, 3.75), (1.05, 12.85, 4.15), "cord_dk")
    g.box(h_tie, "tie_cinch_bone_peg", (0.65, 12.15, 3.20), (1.15, 12.95, 3.55), "bone")
    g.box(h_tie, "tie_cinch_loop", (0.72, 11.95, 3.15), (1.08, 12.47, 3.65), "cord_dk")
    # ── 补充 34 个方块使总数达到 207+
    # 1. 空套筒口部与内腔加固骨垫 (8 块)
    hl = "holster_lining"
    g.box(hl, "lining_bone_strut_1", (1.35, 11.25, 2.45), (1.65, 12.85, 2.75), "bone_stain")
    g.box(hl, "lining_bone_strut_2", (2.85, 11.25, 2.45), (3.15, 12.85, 2.75), "bone_stain")
    g.box(hl, "lining_bottom_pad", (1.35, 10.35, 2.55), (3.15, 10.85, 4.25), "hide_dk")
    g.box(hl, "lining_rim_bracket_l", (0.87, 13.45, 2.45), (1.21, 13.85, 2.85), "iron_lt")
    g.box(hl, "lining_rim_bracket_r", (3.25, 13.45, 2.45), (3.65, 13.85, 2.85), "iron_lt")
    g.box(hl, "lining_rivet_rim_1", (1.55, 13.45, 4.45), (1.85, 13.85, 4.75), "iron_lt")
    g.box(hl, "lining_rivet_rim_2", (2.65, 13.45, 4.45), (2.95, 13.85, 4.75), "iron_lt")
    g.box(hl, "lining_drain_eyelet", (2.05, 9.45, 3.25), (2.45, 9.85, 3.65), "rust")

    # 2. 蓬松草药捆外层翻卷羽叶与草茎 (14 块)
    hlv = "herb_leaves_outer"
    g.box(hlv, "herb_frond_extra_l1", (-3.45, 14.85, 2.65), (-2.85, 16.05, 3.25), "straw_lt")
    g.box(hlv, "herb_frond_extra_l2", (-2.95, 15.65, 3.35), (-2.35, 16.55, 3.85), "straw")
    g.box(hlv, "herb_frond_extra_r1", (0.65, 14.85, 2.62), (1.25, 16.05, 3.25), "straw_dk")
    g.box(hlv, "herb_frond_extra_r2", (0.35, 15.65, 3.35), (0.95, 16.55, 3.85), "straw_lt")
    g.box(hlv, "herb_frond_extra_f1", (-2.25, 14.25, 4.45), (-1.65, 15.45, 4.85), "straw_lt")
    g.box(hlv, "herb_frond_extra_f2", (-1.15, 14.25, 4.45), (-0.55, 15.45, 4.85), "straw")
    g.box(hlv, "herb_frond_extra_b1", (-2.25, 14.25, 2.05), (-1.65, 15.45, 2.45), "straw_dk")
    g.box(hlv, "herb_frond_extra_b2", (-1.15, 14.25, 2.05), (-0.55, 15.45, 2.45), "straw_dk")
    g.box(hlv, "herb_leaf_notch_1", (-3.05, 14.65, 3.55), (-2.65, 15.01, 3.85), "void")
    g.box(hlv, "herb_leaf_notch_2", (0.29, 14.65, 3.55), (0.65, 14.99, 3.85), "void")
    g.box(hlv, "herb_leaf_stalk_ext_1", (-2.65, 13.85, 3.13), (-2.15, 15.15, 3.55), "straw_dk")
    g.box(hlv, "herb_leaf_stalk_ext_2", (-0.75, 13.85, 3.13), (-0.25, 15.15, 3.55), "straw_lt")
    g.box(hlv, "herb_frond_crown_a", (-1.62, 16.19, 2.85), (-1.12, 16.85, 3.45), "straw_lt")
    g.box(hlv, "herb_frond_crown_b", (-1.35, 16.15, 3.25), (-0.85, 16.83, 3.85), "straw")

    # 3. 根须泥块与下垂细须 (6 块)
    hr = "herb_roots_drape"
    g.box(hr, "root_clump_crumb_1", (-2.25, 7.55, 2.95), (-1.75, 8.15, 3.45), "hide_dk")
    g.box(hr, "root_clump_crumb_2", (-0.95, 7.55, 3.05), (-0.45, 8.15, 3.55), "hide_dk")
    g.box(hr, "root_whisker_long_1", (-2.05, 5.05, 3.05), (-1.65, 6.15, 3.45), "straw_dk")
    g.box(hr, "root_whisker_long_2", (-0.85, 5.15, 3.15), (-0.45, 6.25, 3.55), "cord_dk")
    g.box(hr, "root_whisker_curl_1", (-1.55, 5.65, 3.25), (-1.25, 6.45, 3.65), "straw_dk")
    g.box(hr, "root_whisker_curl_2", (-1.25, 5.45, 2.85), (-0.95, 6.85, 3.25), "cord_dk")

    # 4. 骨符挂绳与辟邪配饰 (6 块)
    tb = "tally_beads"
    g.box(tb, "tally_bead_bone_3", (3.32, 7.85, 3.65), (3.68, 8.25, 3.95), "bone")
    g.box(tb, "tally_bead_metal_spacer", (3.35, 8.25, 3.68), (3.65, 8.48, 3.92), "iron_lt")
    g.box(tb, "tally_floss_strand_1", (3.25, 6.88, 3.55), (3.52, 7.58, 3.82), "cord")
    g.box(tb, "tally_floss_strand_2", (3.51, 6.85, 3.73), (3.73, 7.55, 4.02), "cord_dk")
    g.box(tb, "tally_charm_bone_speck", (3.38, 7.05, 3.68), (3.62, 7.45, 3.92), "bone_lt")
    g.box(tb, "tally_bone_peg_fastener", (3.22, 10.65, 3.45), (3.78, 11.05, 3.72), "bone_stain")



PART_FUNCS = {
    "belt": part_belt,
    "holster_base": part_holster_base,
    "holster_strap": part_holster_strap,
    "holster_lining": part_holster_lining,
    "herb_main": part_herb_main,
    "herb_ties": part_herb_ties,
    "herb_leaves_mid": part_herb_leaves_mid,
    "herb_leaves_outer": part_herb_leaves_outer,
    "herb_seed_heads": part_herb_seed_heads,
    "herb_roots_main": part_herb_roots_main,
    "herb_roots_drape": part_herb_roots_drape,
    "tally_tag": part_tally_tag,
    "tally_beads": part_tally_beads,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="yao_chu",
    model_name="YaoChu",
    title="药锄腰后挂架 / yao_chu",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：后腰（药锄空皮套 + 占半后腰蓬松草药捆）",
    worn_focus=((0.5, 12.0, 3.2), 24.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
