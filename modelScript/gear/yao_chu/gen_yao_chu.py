#!/usr/bin/env python3
"""药锄含腰后挂架（yao_chu）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

对应 NPC：采药人（cai_yao_ren）。
挂点：后腰中线偏右（x∈[0.2, 3.8], y∈[7.2, 22.8], z∈[2.08, 3.85]）。
避让：避开右胯灵草囊（x>4.0, z<0）、避开左胯防潮包（x<-4.0, z<0），避开躯干与双腿（z >= 2.08 在后方）。

外形：厚兽皮腰挂皮套筒、小号卷口粗铸锈铁锄面（刃口残缺卷口、脊肋加固）、粗弯枯木锄柄（多层防滑粗麻绳缠把、柄顶木瘤）；
挂架侧绑一捆刚采下的野生干灵草捆（草绳双道绑扎、外翘草叶羽叶、下垂泥渍根须）；
挂有一枚辨药骨符与挂带。

方块数 ≥200，分骨 ≥10（挂件单独一骨）。
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
    ("holster_base", "belt_mount", (1.8, 11.5, 2.6), "腰后偏右厚兽皮套筒主体"),
    ("holster_strap", "holster_base", (1.8, 12.8, 2.7), "皮套加固搭扣与金属眼"),
    ("hoe_head_socket", "holster_base", (1.5, 10.5, 2.8), "粗铸锈铁锄眼套管与紧固木楔"),
    ("hoe_head_blade", "hoe_head_socket", (1.4, 8.5, 2.9), "卷口厚铁锄面与加固背脊"),
    ("hoe_handle_lo", "hoe_head_socket", (1.8, 12.2, 2.8), "枯木柄下段穿柄芯"),
    ("hoe_handle_mid", "hoe_handle_lo", (2.2, 15.5, 2.9), "枯木柄中段防滑麻绳粗缠握把"),
    ("hoe_handle_hi", "hoe_handle_mid", (2.8, 19.2, 3.1), "枯木柄上段斜向挑起曲柄"),
    ("hoe_handle_knob", "hoe_handle_hi", (3.4, 21.8, 3.3), "柄顶粗圆木瘤与防脱皮帽"),
    ("herb_bundle_main", "holster_base", (0.6, 11.2, 2.9), "干草药捆主体：紧扎草药茎束"),
    ("herb_bundle_ties", "herb_bundle_main", (0.6, 11.4, 2.9), "草药捆双道紧扎草绳与打结"),
    ("herb_leaves_splayed", "herb_bundle_main", (0.6, 13.8, 3.0), "草药捆外翘参差干燥叶尖与羽叶"),
    ("herb_roots_drape", "herb_bundle_main", (0.6, 8.2, 2.9), "草药捆下垂外露泥渍粗细须根"),
    ("hoe_tally_tag", "holster_base", (2.6, 11.2, 3.2), "辨药骨符与挂绳"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "belt_mount")])
SOFT_OVER = (
    ("bone", "bone_lt"), ("bone", "bone_stain"), ("hide", "hide_dk"),
    ("hide", "hide_lt"), ("cord", "cord_dk"), ("straw", "straw_dk"),
    ("straw", "straw_lt"), ("iron", "rust"), ("rust", "rust_dk"),
    ("iron", "iron_lt"), ("hide", "cord"), ("hide", "stitch"),
    ("hide", "iron"), ("hide", "bone"), ("iron", "cord"),
    ("straw", "cord"), ("bone", "void"), ("iron", "bone"),
)


# ── 1. 后腰对称皮带与对称带扣（完全对称骨骼，满足 mirror 自检）
def part_belt(g: Gear) -> None:
    """后腰对称皮带与对称带扣。"""
    b = "belt_mount"
    g.box(b, "belt_center_core", (-1.25, 12.12, 2.08), (1.25, 13.65, 2.45), "hide_dk")
    g.box(b, "belt_center_cover", (-0.95, 12.22, 2.42), (0.95, 13.58, 2.72), "hide")
    g.box(b, "belt_center_bone_pin", (-0.35, 12.02, 2.68), (0.35, 13.78, 2.98), "bone")
    g.box(b, "belt_pin_head", (-0.28, 13.74, 2.71), (0.28, 14.05, 2.96), "bone_lt")
    g.box(b, "belt_pin_tail", (-0.25, 11.75, 2.71), (0.25, 12.06, 2.96), "bone_stain")

    # 左右对称成对延伸皮带
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.10), (4.48, 13.62, 2.44), "hide_dk")
    g.pair(b, "belt_wing_outer", (1.24, 12.25, 2.41), (4.44, 13.55, 2.68), "hide")
    g.pair(b, "belt_edge_top", (1.22, 13.58, 2.13), (4.46, 13.88, 2.52), "hide_dk")
    g.pair(b, "belt_edge_bot", (1.22, 11.92, 2.13), (4.46, 12.22, 2.52), "hide_dk")

    # 对称骨铆钉与粗缝线 (dx, dy, dz >= 0.26)
    g.pair(b, "belt_rivet_mid", (1.82, 12.70, 2.65), (2.18, 13.10, 2.95), "bone")
    g.pair(b, "belt_rivet_lat", (3.22, 12.70, 2.65), (3.58, 13.10, 2.95), "bone")
    g.pair(b, "belt_stitch_1", (1.32, 13.22, 2.65), (1.68, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_2", (2.52, 13.22, 2.65), (2.88, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_3", (3.82, 13.22, 2.65), (4.18, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_b1", (1.32, 12.26, 2.65), (1.68, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.65), (2.88, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")


# ── 2. 腰后偏右厚兽皮套筒主体
def part_holster_base(g: Gear) -> None:
    """腰后偏右厚兽皮套筒主体。"""
    b = "holster_base"
    # 中空中套四壁 (空腔留给药锄：x∈[1.25, 2.05], z∈[2.52, 3.18])
    g.box(b, "holster_wall_back", (1.05, 10.01, 2.25), (2.25, 13.45, 2.55), "hide_dk")
    g.box(b, "holster_wall_front", (1.05, 10.09, 3.15), (2.25, 13.45, 3.48), "hide")
    g.box(b, "holster_wall_left", (0.85, 10.03, 2.48), (1.18, 13.40, 3.27), "hide_lt")
    g.box(b, "holster_wall_right", (2.12, 10.05, 2.48), (2.45, 13.48, 3.22), "hide_dk")

    # 上下沿加厚封边口与前护垫
    g.box(b, "holster_rim_top", (0.80, 13.35, 2.22), (2.48, 13.72, 3.52), "hide_dk")
    g.box(b, "holster_rim_bot", (0.80, 9.69, 2.22), (2.48, 10.12, 3.52), "hide_dk")
    g.box(b, "holster_front_pad", (1.15, 10.60, 3.42), (2.15, 12.89, 3.72), "hide_lt")

    # 双列缝线
    g.box(b, "holster_stitch_l", (0.90, 10.35, 3.40), (1.28, 13.15, 3.69), "stitch")
    g.box(b, "holster_stitch_r", (2.02, 10.35, 3.40), (2.40, 13.12, 3.69), "stitch")


# ── 3. 皮套加固搭扣与金属眼
def part_holster_strap(g: Gear) -> None:
    """皮套加固搭扣与金属眼。"""
    b = "holster_strap"
    g.box(b, "hstrap_front_band", (0.75, 12.15, 3.46), (2.55, 12.85, 3.75), "hide_dk")
    g.box(b, "hstrap_back_band", (0.75, 12.15, 2.12), (2.55, 12.85, 2.42), "hide_dk")
    g.box(b, "hstrap_iron_buckle", (1.35, 12.05, 3.68), (1.95, 12.95, 3.98), "rust")
    # 舌片与锁孔 (dx >= 0.25)
    g.box(b, "hstrap_iron_tongue", (1.52, 11.95, 3.72), (1.78, 13.05, 4.05), "iron")
    g.box(b, "hstrap_leather_tongue", (1.42, 10.65, 3.62), (1.88, 12.15, 3.88), "hide")
    g.box(b, "hstrap_eyelet_metal", (1.48, 10.85, 3.78), (1.82, 11.35, 4.02), "iron_lt")


# ── 4. 粗铸锈铁锄眼套管与紧固木楔
def part_hoe_socket(g: Gear) -> None:
    """粗铸锈铁锄眼套管与紧固木楔。"""
    b = "hoe_head_socket"
    # 圆筒状厚铁套管
    g.box(b, "socket_iron_body", (1.25, 9.85, 2.52), (2.05, 11.85, 3.18), "rust_dk")
    g.box(b, "socket_rim_upper", (1.20, 11.65, 2.46), (2.10, 11.95, 3.24), "iron")
    g.box(b, "socket_rim_lower", (1.20, 9.72, 2.46), (2.10, 10.05, 3.24), "rust")

    # 紧固木楔与铁钉
    g.box(b, "socket_wood_wedge", (1.42, 11.75, 2.65), (1.88, 12.25, 3.05), "hide_lt")
    g.box(b, "socket_pin_nail", (1.12, 10.65, 2.75), (2.21, 11.05, 2.95), "iron_lt")
    g.box(b, "socket_side_flange", (1.15, 10.25, 2.38), (2.15, 10.85, 2.60), "rust_dk")


# ── 5. 卷口厚铁锄面与加固背脊
def part_hoe_blade(g: Gear) -> None:
    """卷口厚铁锄面与加固背脊。"""
    b = "hoe_head_blade"
    # 锄面主板
    g.box(b, "blade_neck_joint", (1.32, 9.45, 2.58), (1.98, 10.15, 3.15), "rust_dk")
    g.box(b, "blade_body_upper", (1.15, 8.65, 2.65), (2.15, 9.65, 3.25), "iron")
    g.box(b, "blade_body_lower", (1.05, 7.65, 2.72), (2.25, 8.81, 3.35), "iron")

    # 锄背中心加厚加强脊肋
    g.box(b, "blade_spine_rib", (1.45, 7.85, 2.52), (1.85, 9.85, 2.85), "iron_lt")

    # 锄面两侧微卷翘边
    g.box(b, "blade_edge_flange_l", (0.92, 7.75, 2.80), (1.22, 9.25, 3.38), "rust")
    g.box(b, "blade_edge_flange_r", (2.08, 7.75, 2.80), (2.38, 9.25, 3.38), "rust")

    # 锋刃卷口与缺损崩齿
    g.box(b, "blade_cutting_edge", (1.02, 7.32, 2.80), (2.28, 7.75, 3.38), "iron_lt")
    g.box(b, "blade_notch_curl1", (1.12, 7.22, 2.90), (1.48, 7.55, 3.42), "rust")
    g.box(b, "blade_notch_void", (1.46, 7.40, 2.95), (1.78, 7.68, 3.28), "void")
    g.box(b, "blade_notch_curl2", (1.82, 7.22, 2.90), (2.14, 7.55, 3.42), "rust_dk")


# ── 6. 枯木柄下段穿柄芯
def part_handle_lo(g: Gear) -> None:
    """枯木柄下段穿柄芯。"""
    b = "hoe_handle_lo"
    g.box(b, "handle_lo_wood_core", (1.41, 11.23, 2.62), (2.03, 14.65, 3.08), "hide")
    g.box(b, "handle_lo_crest_f", (1.45, 11.35, 2.48), (2.01, 14.55, 2.75), "hide_lt")
    g.box(b, "handle_lo_crest_b", (1.45, 11.35, 2.95), (2.02, 14.55, 3.22), "hide_dk")

    # 紧固麻绳缠扎环
    g.box(b, "handle_lo_cord_1", (1.35, 12.45, 2.55), (2.15, 13.05, 3.19), "cord_dk")
    g.box(b, "handle_lo_cord_2", (1.38, 13.65, 2.55), (2.12, 14.23, 3.15), "cord")


# ── 7. 枯木柄中段防滑麻绳粗缠握把
def part_handle_mid(g: Gear) -> None:
    """枯木柄中段防滑麻绳粗缠握把。"""
    b = "hoe_handle_mid"
    g.box(b, "handle_mid_wood_core", (1.88, 14.45, 2.65), (2.65, 18.65, 3.15), "hide")
    g.box(b, "handle_mid_crest_f", (1.92, 14.55, 2.52), (2.58, 18.55, 2.78), "hide_lt")
    g.box(b, "handle_mid_crest_b", (1.96, 14.55, 3.02), (2.58, 18.55, 3.28), "hide_dk")

    # 4 段手握把
    g.box(b, "handle_mid_wrap_1", (1.77, 14.85, 2.58), (2.68, 15.52, 3.22), "cord_dk")
    g.box(b, "handle_mid_wrap_2", (1.95, 15.75, 2.58), (2.81, 16.45, 3.22), "cord")
    g.box(b, "handle_mid_wrap_3", (2.08, 16.65, 2.58), (2.95, 17.30, 3.22), "cord_dk")
    g.box(b, "handle_mid_wrap_4", (2.22, 17.55, 2.58), (3.08, 18.25, 3.22), "cord")

    # 木结与皮垫
    g.box(b, "handle_mid_knot_knob", (2.42, 16.15, 3.05), (2.88, 16.65, 3.42), "hide_dk")
    g.box(b, "handle_mid_grip_leather", (2.02, 15.25, 2.48), (2.78, 16.95, 2.85), "hide_lt")


# ── 8. 枯木柄上段斜向挑起曲柄
def part_handle_hi(g: Gear) -> None:
    """枯木柄上段斜向挑起曲柄。"""
    b = "hoe_handle_hi"
    g.box(b, "handle_hi_wood_core", (2.52, 18.45, 2.82), (3.28, 21.65, 3.35), "hide")
    g.box(b, "handle_hi_crest_f", (2.55, 18.55, 2.68), (3.25, 21.55, 3.01), "hide_lt")
    g.box(b, "handle_hi_crest_b", (2.55, 18.55, 3.22), (3.25, 21.55, 3.52), "hide_dk")

    # 防裂铁皮箍与麻绳圈
    g.box(b, "handle_hi_iron_ferrule", (2.62, 19.85, 2.75), (3.38, 20.45, 3.42), "rust")
    g.box(b, "handle_hi_cord_band", (2.72, 20.85, 2.85), (3.48, 21.45, 3.48), "cord_dk")


# ── 9. 柄顶粗圆木瘤与防脱皮帽
def part_handle_knob(g: Gear) -> None:
    """柄顶粗圆木瘤与防脱皮帽。"""
    b = "hoe_handle_knob"
    g.box(b, "handle_knob_core", (3.12, 21.45, 3.10), (3.88, 22.65, 3.75), "hide_dk")
    g.box(b, "handle_knob_flare_t", (3.18, 22.45, 3.15), (3.79, 22.85, 3.70), "hide")
    g.box(b, "handle_knob_side_b", (3.05, 21.65, 3.15), (3.42, 22.35, 3.70), "hide_lt")

    # 防脱皮帽与骨销
    g.box(b, "handle_knob_leather_cap", (3.22, 22.15, 3.02), (3.98, 22.88, 3.82), "hide_lt")
    g.box(b, "handle_knob_bone_pin", (3.38, 22.35, 3.62), (3.82, 22.75, 3.95), "bone_lt")


# ── 10. 干草药捆主体：紧扎草药茎束
def part_herb_main(g: Gear) -> None:
    """干草药捆主体：紧扎草药茎束。"""
    b = "herb_bundle_main"
    g.box(b, "herb_sheaf_core", (0.22, 10.05, 2.55), (1.08, 13.05, 3.25), "straw")
    g.box(b, "herb_sheaf_front", (0.25, 10.18, 3.15), (1.05, 12.97, 3.45), "straw_lt")
    g.box(b, "herb_sheaf_back", (0.25, 10.12, 2.35), (1.05, 12.93, 2.65), "straw_dk")
    g.box(b, "herb_sheaf_side_l", (0.00, 10.15, 2.58), (0.42, 12.95, 3.24), "straw_dk")
    g.box(b, "herb_sheaf_side_r", (0.84, 10.15, 2.58), (1.25, 12.91, 3.22), "straw_lt")

    # 外层各色干燥草茎条纹
    g.box(b, "herb_stem_stripe_1", (0.32, 9.65, 3.35), (0.68, 13.47, 3.58), "straw_dk")
    g.box(b, "herb_stem_stripe_2", (0.65, 9.61, 3.37), (0.97, 13.45, 3.61), "straw_lt")
    g.box(b, "herb_stem_stripe_3", (0.06, 9.65, 2.72), (0.33, 13.45, 3.08), "straw")


# ── 11. 草药捆双道紧扎草绳与打结
def part_herb_ties(g: Gear) -> None:
    """草药捆双道紧扎草绳与打结。"""
    b = "herb_bundle_ties"
    g.box(b, "herb_tie_upper_f", (0.12, 12.20, 3.38), (1.18, 12.75, 3.62), "cord_dk")
    g.box(b, "herb_tie_upper_b", (0.12, 12.17, 2.28), (1.21, 12.75, 2.52), "cord_dk")
    g.box(b, "herb_tie_upper_knot", (-0.02, 12.00, 2.75), (0.38, 12.85, 3.22), "cord")

    g.box(b, "herb_tie_lower_f", (0.12, 10.45, 3.38), (1.18, 11.05, 3.62), "cord")
    g.box(b, "herb_tie_lower_b", (0.12, 10.45, 2.28), (1.16, 11.05, 2.52), "cord_dk")
    g.box(b, "herb_tie_lower_knot", (-0.02, 10.35, 2.75), (0.38, 11.20, 3.22), "cord_dk")

    # 绳尾下垂流苏
    g.box(b, "herb_tie_tail_1", (0.02, 9.45, 2.82), (0.32, 10.53, 3.12), "cord")
    g.box(b, "herb_tie_tail_2", (0.08, 9.15, 2.88), (0.43, 10.55, 3.18), "cord_dk")


# ── 12. 草药捆外翘参差干燥叶尖与羽叶
def part_herb_leaves(g: Gear) -> None:
    """草药捆外翘参差干燥叶尖与羽叶。"""
    b = "herb_leaves_splayed"
    g.box(b, "leaf_spike_mid", (0.38, 12.80, 2.72), (0.92, 14.85, 3.12), "straw_lt")
    g.box(b, "leaf_spike_front", (0.42, 12.85, 3.28), (0.88, 14.55, 3.53), "straw")
    g.box(b, "leaf_spike_back", (0.42, 12.85, 2.18), (0.88, 14.55, 2.52), "straw_dk")
    g.box(b, "leaf_spike_left", (-0.02, 12.90, 2.62), (0.38, 14.25, 3.18), "straw_dk")
    g.box(b, "leaf_spike_right", (0.88, 12.85, 2.62), (1.32, 14.25, 3.18), "straw_lt")

    # 干燥羽叶与药草籽粒穗
    g.box(b, "herb_seed_head_1", (0.48, 14.65, 2.75), (0.82, 15.35, 3.08), "straw_lt")
    g.box(b, "herb_seed_head_2", (0.18, 14.05, 2.55), (0.52, 14.75, 2.88), "straw")
    g.box(b, "herb_curled_tip_l", (-0.15, 13.85, 2.75), (0.22, 14.45, 3.14), "straw_dk")
    g.box(b, "herb_curled_tip_r", (1.08, 13.85, 2.75), (1.45, 14.45, 3.12), "straw_lt")


# ── 13. 草药捆下垂外露泥渍粗细须根
def part_herb_roots(g: Gear) -> None:
    """草药捆下垂外露泥渍粗细须根。"""
    b = "herb_roots_drape"
    g.box(b, "root_tuft_main", (0.39, 8.45, 2.65), (0.95, 10.25, 3.15), "straw_dk")
    g.box(b, "root_strand_a", (0.38, 7.45, 2.68), (0.72, 8.63, 2.98), "cord_dk")
    g.box(b, "root_strand_b", (0.65, 7.25, 2.78), (1.01, 8.67, 3.08), "hide_dk")
    g.box(b, "root_strand_c", (0.22, 7.85, 2.72), (0.52, 8.85, 3.05), "straw_dk")
    g.box(b, "root_strand_d", (0.82, 7.60, 2.75), (1.12, 8.85, 3.05), "cord_dk")

    # 根茎带泥土块与碎石粒
    g.box(b, "root_mud_clod_1", (0.42, 8.85, 2.58), (0.88, 9.35, 2.92), "hide_dk")
    g.box(b, "root_mud_clod_2", (0.45, 8.83, 2.88), (0.85, 9.33, 3.22), "hide_dk")


# ── 14. 辨药骨符与挂绳
def part_tally_tag(g: Gear) -> None:
    """辨药骨符与挂绳。"""
    b = "hoe_tally_tag"
    g.box(b, "tally_cord_loop", (2.42, 11.65, 3.17), (2.88, 12.35, 3.45), "cord_dk")
    g.box(b, "tally_bone_body", (2.32, 10.33, 3.15), (2.98, 11.75, 3.52), "bone")
    g.box(b, "tally_bone_top", (2.38, 11.55, 3.12), (2.92, 11.88, 3.55), "bone_lt")
    # 暗孔与下垂流苏 (dx, dy, dz >= 0.25)
    g.box(b, "tally_bone_void", (2.50, 11.32, 3.25), (2.80, 11.65, 3.48), "void")
    g.box(b, "tally_cord_knot", (2.48, 9.95, 3.18), (2.82, 10.45, 3.51), "cord_dk")
    g.box(b, "tally_cord_tassel", (2.50, 9.42, 3.20), (2.80, 10.03, 3.46), "cord")


# ── 15. 细节补强（使全模型方块总数达 205+）
def part_details(g: Gear) -> None:
    """腰带挂环、药锄加固铁箍与草药捆细节。"""
    # 腰带外延挂带 (4 块)
    b = "belt_mount"
    g.pair(b, "belt_sub_clasp", (2.05, 12.60, 2.72), (2.42, 13.20, 3.02), "hide_dk")
    g.pair(b, "belt_sub_ring", (3.05, 12.60, 2.72), (3.42, 13.20, 3.02), "rust")

    # 药锄中段防滑凸绳结 (6 块)
    hm = "hoe_handle_mid"
    g.box(hm, "handle_braid_knot_1", (1.82, 15.15, 3.08), (2.15, 15.55, 3.35), "cord_dk")
    g.box(hm, "handle_braid_knot_2", (2.12, 16.05, 3.08), (2.45, 16.47, 3.35), "cord")
    g.box(hm, "handle_braid_knot_3", (2.42, 16.95, 3.08), (2.75, 17.35, 3.35), "cord_dk")
    g.box(hm, "handle_braid_knot_4", (1.82, 15.15, 2.45), (2.15, 15.55, 2.72), "cord")
    g.box(hm, "handle_braid_knot_5", (2.12, 16.05, 2.42), (2.45, 16.48, 2.72), "cord_dk")
    g.box(hm, "handle_braid_knot_6", (2.42, 16.95, 2.45), (2.75, 17.35, 2.72), "cord")

    # 锄头套管防滑铁钉 (4 块)
    hs = "hoe_head_socket"
    g.box(hs, "socket_iron_rivet_1", (1.09, 10.45, 2.82), (1.35, 10.85, 3.12), "iron_lt")
    g.box(hs, "socket_iron_rivet_2", (1.95, 10.45, 2.82), (2.18, 10.85, 3.12), "iron_lt")
    g.box(hs, "socket_strap_loop", (1.35, 10.15, 2.40), (1.95, 10.55, 2.68), "cord_dk")
    g.box(hs, "socket_wedge_peg", (1.48, 11.95, 2.75), (1.82, 12.35, 2.95), "bone_lt")

    # 草药捆侧挂吊绳与小骨签 (6 块)
    hbm = "herb_bundle_main"
    g.box(hbm, "herb_hang_cord_l", (0.35, 12.82, 2.41), (0.65, 13.55, 2.68), "cord_dk")
    g.box(hbm, "herb_hang_cord_r", (0.65, 12.89, 2.45), (1.00, 13.55, 2.68), "cord")
    g.box(hbm, "herb_charm_bone_root", (0.42, 11.85, 3.42), (0.78, 12.45, 3.72), "bone")
    g.box(hbm, "herb_charm_bone_tip", (0.48, 11.45, 3.45), (0.72, 11.95, 3.68), "bone_lt")
    g.box(hbm, "herb_stem_fluff_1", (0.22, 13.15, 3.25), (0.48, 13.65, 3.52), "straw_lt")
    g.box(hbm, "herb_stem_fluff_2", (0.75, 13.15, 3.25), (1.02, 13.65, 3.48), "straw_dk")

    # ── 补充 64 块构件使方块总数达到 206+
    # 1. 锄头加厚卷刃与正面背肋 (16 块)
    hb = "hoe_head_blade"
    g.box(hb, "blade_curl_edge_1", (1.08, 7.15, 2.85), (1.42, 7.45, 3.32), "rust_dk")
    g.box(hb, "blade_curl_edge_2", (1.88, 7.15, 2.85), (2.22, 7.45, 3.32), "rust_dk")
    g.box(hb, "blade_curl_roll_a", (1.10, 7.19, 3.32), (1.38, 7.52, 3.58), "rust")
    g.box(hb, "blade_curl_roll_b", (1.92, 7.17, 3.32), (2.18, 7.52, 3.58), "rust")
    g.box(hb, "blade_rib_front_1", (1.25, 8.25, 3.22), (1.55, 9.45, 3.48), "iron_lt")
    g.box(hb, "blade_rib_front_2", (1.75, 8.25, 3.22), (2.05, 9.45, 3.48), "iron_lt")
    g.box(hb, "blade_cramp_plate_1", (1.35, 8.45, 2.62), (1.65, 8.85, 2.88), "rust")
    g.box(hb, "blade_cramp_plate_2", (1.65, 8.45, 2.62), (1.95, 8.85, 2.88), "rust_dk")
    g.box(hb, "blade_socket_ring_t", (1.22, 11.25, 2.45), (2.00, 11.55, 3.25), "rust")
    g.box(hb, "blade_socket_ring_b", (1.22, 10.10, 2.45), (2.08, 10.45, 3.25), "rust_dk")
    g.box(hb, "blade_socket_rivet_a", (1.08, 10.85, 2.55), (1.35, 11.15, 2.82), "iron_lt")
    g.box(hb, "blade_socket_rivet_b", (1.95, 10.85, 2.55), (2.18, 11.15, 2.82), "iron_lt")
    g.box(hb, "blade_notch_flaw_1", (1.32, 7.55, 2.78), (1.58, 7.85, 3.05), "void")
    g.box(hb, "blade_notch_flaw_2", (1.72, 7.55, 2.78), (1.98, 7.85, 3.05), "void")
    g.box(hb, "blade_scab_patch_1", (1.02, 8.15, 2.95), (1.32, 8.65, 3.28), "rust_dk")
    g.box(hb, "blade_scab_patch_2", (1.98, 8.15, 2.95), (2.28, 8.65, 3.28), "rust_dk")

    # 2. 枯木长柄防滑螺旋圈与木瘤 (18 块)
    hl = "hoe_handle_lo"
    g.box(hl, "handle_lo_ring_1", (1.40, 11.85, 2.52), (2.08, 12.18, 3.18), "cord_dk")
    g.box(hl, "handle_lo_ring_2", (1.42, 13.05, 2.52), (2.08, 13.38, 3.18), "cord")
    g.box(hl, "handle_lo_ring_3", (1.42, 14.15, 2.52), (2.08, 14.48, 3.18), "cord_dk")
    g.box(hl, "handle_lo_knob_a", (1.33, 12.65, 2.85), (1.68, 13.00, 3.15), "hide_dk")
    g.box(hl, "handle_lo_knob_b", (1.82, 13.85, 2.85), (2.16, 14.25, 3.13), "hide_dk")

    g.box(hm, "handle_mid_ring_1", (1.89, 15.35, 2.55), (2.62, 15.68, 3.25), "cord_dk")
    g.box(hm, "handle_mid_ring_2", (2.05, 16.25, 2.55), (2.75, 16.58, 3.25), "cord")
    g.box(hm, "handle_mid_ring_3", (2.18, 17.15, 2.55), (2.88, 17.48, 3.25), "cord_dk")
    g.box(hm, "handle_mid_ring_4", (2.32, 18.05, 2.55), (3.02, 18.38, 3.25), "cord")
    g.box(hm, "handle_mid_stud_1", (2.25, 15.85, 2.45), (2.55, 16.15, 2.75), "iron_lt")
    g.box(hm, "handle_mid_stud_2", (2.45, 16.75, 2.50), (2.80, 17.05, 2.75), "iron_lt")

    hhi = "hoe_handle_hi"
    g.box(hhi, "handle_hi_ring_1", (2.65, 19.25, 2.78), (3.35, 19.58, 3.44), "cord_dk")
    g.box(hhi, "handle_hi_ring_2", (2.75, 20.35, 2.78), (3.45, 20.68, 3.38), "cord")
    g.box(hhi, "handle_hi_stud_a", (2.85, 19.55, 3.12), (3.15, 19.85, 3.42), "iron_lt")
    g.box(hhi, "handle_hi_stud_b", (3.05, 20.55, 3.12), (3.35, 20.85, 3.42), "iron_lt")
    g.box(hhi, "handle_hi_knob_c", (2.58, 18.85, 2.92), (2.88, 19.25, 3.25), "hide_dk")
    g.box(hhi, "handle_hi_seam_1", (2.72, 19.95, 2.71), (3.02, 20.25, 2.98), "cord_dk")
    g.box(hhi, "handle_hi_seam_2", (2.88, 20.95, 2.71), (3.18, 21.25, 2.98), "cord")

    # 3. 草药捆多层羽叶、根须与结扎绳 (18 块)
    hlv = "herb_leaves_splayed"
    g.box(hlv, "herb_leaf_ext_1", (0.32, 13.25, 2.45), (0.62, 14.65, 2.75), "straw_lt")
    g.box(hlv, "herb_leaf_ext_2", (0.68, 13.25, 2.50), (0.98, 14.65, 2.75), "straw")
    g.box(hlv, "herb_leaf_ext_3", (0.32, 13.25, 3.05), (0.62, 14.65, 3.35), "straw")
    g.box(hlv, "herb_leaf_ext_4", (0.68, 13.25, 3.05), (0.98, 14.65, 3.35), "straw_lt")
    g.box(hlv, "herb_leaf_ext_5", (-0.08, 13.15, 2.82), (0.25, 14.35, 3.12), "straw_dk")
    g.box(hlv, "herb_leaf_ext_6", (1.05, 13.15, 2.82), (1.38, 14.35, 3.10), "straw_dk")

    hr = "herb_roots_drape"
    g.box(hr, "root_strand_ext_1", (0.32, 7.15, 2.62), (0.58, 8.35, 2.92), "straw_dk")
    g.box(hr, "root_strand_ext_2", (0.72, 6.95, 2.62), (0.98, 8.35, 2.92), "straw_dk")
    g.box(hr, "root_strand_ext_3", (0.42, 6.85, 2.82), (0.68, 8.05, 3.12), "cord_dk")
    g.box(hr, "root_strand_ext_4", (0.62, 6.82, 2.80), (0.88, 8.02, 3.15), "cord_dk")
    g.box(hr, "root_mud_ball_1", (0.35, 8.25, 2.75), (0.65, 8.67, 3.02), "hide_dk")
    g.box(hr, "root_mud_ball_2", (0.68, 8.25, 2.73), (0.93, 8.65, 3.10), "hide_dk")

    ht = "herb_bundle_ties"
    g.box(ht, "tie_ribbon_fold_1", (0.05, 12.35, 2.65), (0.28, 13.00, 2.95), "cord_dk")
    g.box(ht, "tie_ribbon_fold_2", (0.05, 10.65, 2.65), (0.28, 11.25, 2.95), "cord_dk")
    g.box(ht, "tie_knot_bead_1", (-0.05, 12.25, 2.95), (0.22, 12.65, 3.25), "bone_lt")
    g.box(ht, "tie_knot_bead_2", (-0.05, 10.55, 2.95), (0.22, 10.95, 3.25), "bone_lt")
    g.box(ht, "tie_cord_ring_t", (0.17, 12.05, 2.38), (0.98, 12.35, 3.48), "cord")
    g.box(ht, "tie_cord_ring_b", (0.27, 10.85, 2.38), (0.98, 11.15, 3.48), "cord")

    # 4. 皮套加固护片与骨符细节 (12 块)
    h_part = "holster_base"
    g.box(h_part, "holster_corner_pad_1", (0.82, 12.55, 3.32), (1.21, 13.25, 3.65), "hide_dk")
    g.box(h_part, "holster_corner_pad_2", (2.12, 12.55, 3.32), (2.48, 13.25, 3.65), "hide_dk")
    g.box(h_part, "holster_corner_pad_3", (0.82, 10.25, 3.32), (1.16, 10.95, 3.65), "hide_dk")
    g.box(h_part, "holster_corner_pad_4", (2.12, 10.25, 3.32), (2.48, 10.99, 3.65), "hide_dk")
    g.box(h_part, "holster_brass_nail_1", (0.95, 12.85, 3.52), (1.18, 13.19, 3.75), "iron_lt")
    g.box(h_part, "holster_brass_nail_2", (2.15, 12.85, 3.52), (2.38, 13.15, 3.75), "iron_lt")

    tt = "hoe_tally_tag"
    g.box(tt, "tally_notch_line_1", (2.38, 10.65, 3.22), (2.92, 10.95, 3.48), "void")
    g.box(tt, "tally_notch_line_2", (2.38, 11.05, 3.22), (2.92, 11.35, 3.46), "void")
    g.box(tt, "tally_wood_bead_a", (2.48, 11.85, 3.18), (2.78, 12.25, 3.48), "hide_dk")
    g.box(tt, "tally_wood_bead_b", (2.52, 9.65, 3.22), (2.78, 10.05, 3.53), "hide_dk")
    g.box(tt, "tally_charm_bone_tip", (2.42, 10.15, 3.22), (2.88, 10.55, 3.48), "bone_lt")
    g.box(tt, "tally_drape_strand", (2.52, 9.15, 3.24), (2.75, 9.65, 3.45), "cord")


PART_FUNCS = {
    "belt": part_belt,
    "holster_base": part_holster_base,
    "holster_strap": part_holster_strap,
    "hoe_socket": part_hoe_socket,
    "hoe_blade": part_hoe_blade,
    "handle_lo": part_handle_lo,
    "handle_mid": part_handle_mid,
    "handle_hi": part_handle_hi,
    "handle_knob": part_handle_knob,
    "herb_main": part_herb_main,
    "herb_ties": part_herb_ties,
    "herb_leaves": part_herb_leaves,
    "herb_roots": part_herb_roots,
    "tally_tag": part_tally_tag,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="yao_chu",
    model_name="YaoChu",
    title="药锄含腰后挂架 / yao_chu",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：后腰中线偏右（皮套药锄锄头朝下、柄斜挑出 + 侧绑干灵草药捆）",
    worn_focus=((1.8, 14.5, 3.0), 24.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
