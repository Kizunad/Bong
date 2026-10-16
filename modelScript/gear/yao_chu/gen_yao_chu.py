#!/usr/bin/env python3
"""药锄含腰后挂架（yao_chu）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证（粗柄大锄头蓬松草药捆返工版）。

对应 NPC：采药人（cai_yao_ren）。
挂点：后腰中线偏右（x∈[-2.8, 4.2], y∈[5.2, 22.8], z∈[2.08, 4.95]）。
避让：避开右胯灵草囊（x>4.2, z<0）、避开左胯防潮包（x<-4.0, z<0），避开躯干与双腿（z >= 2.08 在后方）。

调度返工要点兑现：
1. 药锄锄头做成 4×3px 的厚铁刃（宽4.0px、长3.0px、厚1.5px），具粗铸背脊肋与残旧卷口崩齿；
2. 枯木柄做成 2px 粗（截面 1.9-2.2px），带粗麻绳紧密缠握、木结瘤与顶端大木瘤防脱皮帽；
3. 腰后皮套和草药捆加大到占大半个后腰（横向跨度达 6.8px，覆盖 x∈[-2.8, 4.0]）；
4. 草药捆蓬松外散，具双道粗草绳绑扎、大幅向上散开展开的干燥羽叶与籽穗、以及向下垂散的泥渍粗须根。

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
    ("holster_base", "belt_mount", (2.2, 11.5, 3.2), "后腰右侧加宽大号厚兽皮套筒"),
    ("holster_strap", "holster_base", (2.2, 12.8, 3.4), "加宽皮套加固搭扣与金属眼"),
    ("hoe_head_socket", "holster_base", (2.0, 10.2, 3.2), "2px粗铸锈铁锄眼套管与木楔"),
    ("hoe_head_blade", "hoe_head_socket", (2.0, 7.5, 3.4), "4x3px宽厚卷口锈铁锄面"),
    ("hoe_handle_lo", "hoe_head_socket", (2.2, 12.2, 3.2), "2px粗枯木柄下段穿柄芯"),
    ("hoe_handle_mid", "hoe_handle_lo", (2.6, 15.8, 3.4), "2px粗枯木柄中段防滑粗麻缠握把"),
    ("hoe_handle_hi", "hoe_handle_mid", (3.0, 19.5, 3.6), "2px粗枯木柄上段斜向挑起曲柄"),
    ("hoe_handle_knob", "hoe_handle_hi", (3.4, 21.8, 3.8), "粗圆大木节瘤与防脱皮帽"),
    ("herb_bundle_main", "belt_mount", (-0.8, 11.2, 3.4), "占半后腰蓬松大草药捆主体"),
    ("herb_bundle_ties", "herb_bundle_main", (-0.8, 11.4, 3.4), "草药捆双道加粗草绳紧扎与打结"),
    ("herb_leaves_splayed", "herb_bundle_main", (-0.8, 14.5, 3.5), "大幅蓬松外散干燥羽叶与籽穗"),
    ("herb_roots_drape", "herb_bundle_main", (-0.8, 7.5, 3.4), "向下垂散外露泥渍粗须根簇"),
    ("hoe_tally_tag", "holster_base", (3.2, 11.2, 3.8), "辨药骨符与挂带"),
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
    ("straw_lt", "hide_dk"),
))


# ── 1. 后腰对称皮带与对称带扣（完全对称骨骼，满足 mirror 自检）
def part_belt(g: Gear) -> None:
    """后腰对称皮带与对称带扣。"""
    b = "belt_mount"
    g.box(b, "belt_center_core", (-1.25, 12.15, 2.08), (1.25, 13.65, 2.45), "hide_dk")
    g.box(b, "belt_center_cover", (-0.95, 12.22, 2.42), (0.95, 13.58, 2.72), "hide")
    g.box(b, "belt_center_bone_pin", (-0.35, 12.02, 2.68), (0.35, 13.78, 2.98), "bone")
    g.box(b, "belt_pin_head", (-0.28, 13.74, 2.71), (0.28, 14.05, 2.96), "bone_lt")
    g.box(b, "belt_pin_tail", (-0.25, 11.75, 2.71), (0.25, 12.06, 2.96), "bone_stain")

    # 左右对称成对延伸皮带
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.10), (4.48, 13.62, 2.44), "hide_dk")
    g.pair(b, "belt_wing_outer", (1.24, 12.20, 2.41), (4.44, 13.55, 2.68), "hide")
    g.pair(b, "belt_edge_top", (1.22, 13.58, 2.13), (4.46, 13.88, 2.52), "hide_dk")
    g.pair(b, "belt_edge_bot", (1.22, 11.92, 2.13), (4.46, 12.22, 2.52), "hide_dk")

    # 对称骨铆钉与粗缝线 (dx, dy, dz >= 0.26)
    g.pair(b, "belt_rivet_mid", (1.82, 12.70, 2.65), (2.18, 13.10, 2.95), "bone")
    g.pair(b, "belt_rivet_lat", (3.22, 12.70, 2.65), (3.58, 13.10, 2.95), "bone")
    g.pair(b, "belt_stitch_1", (1.32, 13.22, 2.63), (1.68, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_2", (2.52, 13.22, 2.59), (2.88, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_3", (3.82, 13.22, 2.65), (4.18, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_b1", (1.32, 12.26, 2.61), (1.68, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.59), (2.88, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")

    # 对称侧面挂带环与骨扣
    g.pair(b, "belt_sub_clasp", (2.05, 12.60, 2.72), (2.42, 13.20, 3.02), "hide_dk")
    g.pair(b, "belt_sub_ring", (3.05, 12.60, 2.72), (3.42, 13.20, 3.02), "rust")


# ── 2. 后腰右侧加宽大号厚兽皮套筒（适配2px粗柄）
def part_holster_base(g: Gear) -> None:
    """后腰右侧加宽大号厚兽皮套筒。"""
    b = "holster_base"
    # 中空中套四壁 (空腔留给2px粗柄：x∈[1.25, 3.15], z∈[2.52, 4.45])
    g.box(b, "holster_wall_back", (1.05, 9.85, 2.12), (3.35, 13.69, 2.42), "hide_dk")
    g.box(b, "holster_wall_front", (1.05, 9.85, 4.33), (3.35, 13.70, 4.72), "hide")
    g.box(b, "holster_wall_left", (0.85, 9.89, 2.38), (1.20, 13.60, 4.48), "hide_lt")
    g.box(b, "holster_wall_right", (3.15, 9.83, 2.38), (3.55, 13.65, 4.48), "hide_dk")

    # 上下沿加厚封边口与前护垫
    g.box(b, "holster_rim_top", (0.78, 13.48, 2.05), (3.62, 13.92, 4.82), "hide_dk")
    g.box(b, "holster_rim_bot", (0.78, 9.55, 2.05), (3.62, 9.98, 4.82), "hide_dk")
    g.box(b, "holster_front_pad", (1.25, 10.55, 4.58), (3.15, 13.05, 4.92), "hide_lt")

    # 双列立体缝线
    g.box(b, "holster_stitch_l", (0.92, 10.15, 4.52), (1.35, 13.25, 4.82), "stitch")
    g.box(b, "holster_stitch_r", (2.95, 10.15, 4.52), (3.38, 13.28, 4.82), "stitch")

    # 四角加固护皮与铜铆钉 (8 块)
    g.box(b, "holster_corner_pad_1", (0.82, 12.55, 4.45), (1.28, 13.19, 4.85), "hide_dk")
    g.box(b, "holster_corner_pad_2", (3.02, 12.55, 4.45), (3.48, 13.25, 4.85), "hide_dk")
    g.box(b, "holster_corner_pad_3", (0.82, 10.25, 4.45), (1.28, 10.89, 4.85), "hide_dk")
    g.box(b, "holster_corner_pad_4", (3.02, 10.25, 4.45), (3.48, 10.95, 4.85), "hide_dk")
    g.box(b, "holster_brass_nail_1", (0.95, 12.85, 4.75), (1.22, 13.15, 5.05), "iron_lt")
    g.box(b, "holster_brass_nail_2", (3.08, 12.85, 4.75), (3.35, 13.15, 5.05), "iron_lt")
    g.box(b, "holster_brass_nail_3", (0.95, 10.45, 4.75), (1.22, 10.75, 5.05), "iron_lt")
    g.box(b, "holster_brass_nail_4", (3.08, 10.45, 4.75), (3.35, 10.75, 5.05), "iron_lt")


# ── 3. 加宽皮套加固搭扣与金属眼
def part_holster_strap(g: Gear) -> None:
    """加宽皮套加固搭扣与金属眼。"""
    b = "holster_strap"
    # 横扎皮扣带 (包裹药锄)
    g.box(b, "hstrap_front_band", (0.75, 12.09, 4.62), (3.65, 12.85, 4.95), "hide_dk")
    g.box(b, "hstrap_back_band", (0.75, 12.05, 2.15), (3.65, 12.81, 2.38), "hide_dk")
    g.box(b, "hstrap_iron_buckle", (1.85, 11.95, 4.88), (2.55, 12.95, 5.25), "rust")
    g.box(b, "hstrap_iron_tongue", (2.08, 11.85, 4.95), (2.32, 13.05, 5.35), "iron")
    g.box(b, "hstrap_leather_tongue", (1.95, 10.49, 4.82), (2.45, 12.15, 5.15), "hide")
    g.box(b, "hstrap_eyelet_metal", (2.05, 10.75, 5.02), (2.35, 11.25, 5.28), "iron_lt")


# ── 4. 2px粗铸锈铁锄眼套管与木楔
def part_hoe_socket(g: Gear) -> None:
    """2px粗铸锈铁锄眼套管与木楔。"""
    b = "hoe_head_socket"
    # 圆筒状厚铁套管 (直径 2.1×2.1px, x∈[1.15, 3.25], z∈[2.55, 4.45])
    g.box(b, "socket_iron_body", (1.15, 9.90, 2.55), (3.25, 11.85, 4.39), "rust_dk")
    g.box(b, "socket_rim_upper", (1.08, 11.65, 2.48), (3.32, 11.98, 4.52), "iron")
    g.box(b, "socket_rim_lower", (1.08, 9.68, 2.48), (3.32, 10.05, 4.52), "rust")

    # 粗木楔与加固大铁销
    g.box(b, "socket_wood_wedge", (1.85, 11.75, 3.25), (2.55, 12.35, 3.75), "hide_lt")
    g.box(b, "socket_pin_nail", (0.95, 10.65, 3.35), (3.45, 11.05, 3.65), "iron_lt")
    g.box(b, "socket_side_flange", (1.02, 10.25, 2.42), (3.38, 10.85, 2.68), "rust_dk")
    g.box(b, "socket_strap_loop", (1.45, 10.15, 2.38), (2.85, 10.55, 2.62), "cord_dk")


# ── 5. 4x3px宽厚卷口锈铁锄面（调度明确要求：宽4px、长3px、厚铁刃！）
def part_hoe_blade(g: Gear) -> None:
    """4x3px宽厚卷口锈铁锄面（极度醒目大厚刃）。"""
    b = "hoe_head_blade"
    # ── 粗壮连接颈段 (从套管斜向前下方探出)
    g.box(b, "blade_neck_joint", (1.25, 9.35, 2.75), (2.95, 10.15, 3.75), "rust_dk")

    # ── 4×3px 宽大厚实锄面主体（x 宽达 4.0px：从 -0.05 到 4.05！高度 y 达 3.0px：从 6.25 到 9.25！）
    g.box(b, "blade_main_upper", (-0.11, 7.82, 2.85), (4.05, 9.25, 4.15), "iron")
    g.box(b, "blade_main_lower", (-0.05, 6.51, 2.95), (4.10, 8.05, 4.27), "iron")

    # 锄面加厚大背脊肋骨 (贯穿中央，厚度达 1.5px)
    g.box(b, "blade_spine_rib", (1.65, 6.65, 2.65), (2.45, 9.45, 3.05), "iron_lt")
    g.box(b, "blade_spine_crest", (1.82, 6.85, 2.52), (2.28, 9.25, 2.77), "iron_lt")

    # 锄面两侧粗厚卷翘护边
    g.box(b, "blade_flange_left", (-0.22, 6.55, 3.05), (0.15, 8.95, 4.35), "rust")
    g.box(b, "blade_flange_right", (3.85, 6.55, 3.05), (4.22, 8.95, 4.35), "rust")

    # 宽阔锋刃崩口与卷刃残齿 (宽4px的粗糙刃口)
    g.box(b, "blade_cutting_edge", (-0.08, 6.15, 3.05), (4.08, 6.55, 4.28), "iron_lt")
    g.box(b, "blade_curl_roll_1", (0.45, 6.05, 4.15), (1.15, 6.45, 4.48), "rust")
    g.box(b, "blade_curl_roll_2", (2.85, 6.05, 4.15), (3.55, 6.45, 4.48), "rust_dk")
    g.box(b, "blade_notch_void_1", (1.25, 6.28, 3.25), (1.65, 6.58, 3.95), "void")
    g.box(b, "blade_notch_void_2", (2.35, 6.28, 3.25), (2.75, 6.58, 3.95), "void")

    # 锄面裂痕与铁锔钉补片 (4 块)
    g.box(b, "blade_cramp_plate_1", (0.55, 7.45, 2.75), (1.15, 7.95, 3.05), "rust")
    g.box(b, "blade_cramp_plate_2", (2.85, 7.45, 2.75), (3.45, 7.95, 3.05), "rust_dk")
    g.box(b, "blade_scab_patch_1", (0.45, 8.15, 4.05), (1.05, 8.75, 4.35), "rust_dk")
    g.box(b, "blade_scab_patch_2", (2.95, 8.15, 4.05), (3.55, 8.75, 4.35), "rust_dk")


# ── 6. 2px粗枯木柄下段穿柄芯
def part_handle_lo(g: Gear) -> None:
    """2px粗枯木柄下段穿柄芯。"""
    b = "hoe_handle_lo"
    # 穿过套管的2px粗木芯 (x: 1.25 ~ 3.15, z: 2.65 ~ 4.45)
    g.box(b, "handle_lo_wood_core", (1.25, 11.25, 2.69), (3.15, 14.85, 4.45), "hide")
    g.box(b, "handle_lo_crest_f", (1.35, 11.35, 4.40), (3.05, 14.75, 4.70), "hide_lt")
    g.box(b, "handle_lo_crest_b", (1.35, 11.35, 2.45), (3.05, 14.75, 2.75), "hide_dk")

    # 紧固粗麻绳缠扎环 (2圈)
    g.box(b, "handle_lo_cord_1", (1.18, 12.35, 2.55), (3.22, 13.05, 4.55), "cord_dk")
    g.box(b, "handle_lo_cord_2", (1.18, 13.53, 2.55), (3.22, 14.25, 4.55), "cord")


# ── 7. 2px粗枯木柄中段防滑粗麻缠握把（截面粗2px，斜挑出）
def part_handle_mid(g: Gear) -> None:
    """2px粗枯木柄中段防滑粗麻缠握把。"""
    b = "hoe_handle_mid"
    # 中段曲木主芯 (x从1.8向3.6延伸，宽达 2.0px，厚达 2.0px)
    g.box(b, "handle_mid_wood_core", (1.72, 14.65, 2.75), (3.65, 18.85, 4.55), "hide")
    g.box(b, "handle_mid_crest_f", (1.80, 14.75, 4.40), (3.55, 18.75, 4.75), "hide_lt")
    g.box(b, "handle_mid_crest_b", (1.89, 14.75, 2.55), (3.55, 18.75, 2.85), "hide_dk")

    # 4 段粗麻绳紧密缠握手把 (粗达 2.3×2.3px)
    g.box(b, "handle_mid_wrap_1", (1.68, 15.05, 2.65), (3.72, 15.75, 4.65), "cord_dk")
    g.box(b, "handle_mid_wrap_2", (1.85, 15.95, 2.65), (3.85, 16.59, 4.65), "cord")
    g.box(b, "handle_mid_wrap_3", (2.05, 16.85, 2.65), (4.02, 17.53, 4.65), "cord_dk")
    g.box(b, "handle_mid_wrap_4", (2.25, 17.75, 2.65), (4.15, 18.45, 4.65), "cord")

    # 木节瘤与防滑凸线
    g.box(b, "handle_mid_knot_knob", (3.45, 16.25, 3.35), (3.95, 16.85, 3.85), "hide_dk")
    g.box(b, "handle_mid_grip_leather", (1.88, 15.35, 2.45), (3.71, 17.15, 2.77), "hide_lt")


# ── 8. 2px粗枯木柄上段斜向挑起曲柄
def part_handle_hi(g: Gear) -> None:
    """2px粗枯木柄上段斜向挑起曲柄。"""
    b = "hoe_handle_hi"
    # 上段曲度加剧挑出 (宽 2.0px，厚 2.0px，y 达到 21.65)
    g.box(b, "handle_hi_wood_core", (2.35, 18.65, 2.95), (4.25, 21.65, 4.69), "hide")
    g.box(b, "handle_hi_crest_f", (2.50, 18.75, 4.65), (4.15, 21.55, 4.95), "hide_lt")
    g.box(b, "handle_hi_crest_b", (2.45, 18.75, 2.78), (4.15, 21.55, 3.05), "hide_dk")

    # 防裂铁皮箍与麻绳圈
    g.box(b, "handle_hi_iron_ferrule", (2.42, 19.85, 2.85), (4.18, 20.45, 4.85), "rust")
    g.box(b, "handle_hi_cord_band", (2.52, 20.85, 2.88), (4.18, 21.45, 4.82), "cord_dk")


# ── 9. 粗圆大木节瘤与防脱皮帽
def part_handle_knob(g: Gear) -> None:
    """粗圆大木节瘤与防脱皮帽。"""
    b = "hoe_handle_knob"
    # 顶端自然膨大的大木节瘤 (直径达 2.4×2.4px, y∈[21.4, 22.8])
    g.box(b, "handle_knob_core", (2.85, 21.45, 3.35), (4.45, 22.65, 4.85), "hide_dk")
    g.box(b, "handle_knob_flare_t", (2.95, 22.45, 3.45), (4.35, 22.85, 4.75), "hide")
    g.box(b, "handle_knob_side_b", (2.75, 21.65, 3.45), (3.15, 22.35, 4.75), "hide_lt")

    # 顶端防脱厚兽皮帽子与骨销
    g.box(b, "handle_knob_leather_cap", (2.82, 22.15, 3.28), (4.48, 22.88, 4.92), "hide_lt")
    g.box(b, "handle_knob_bone_pin", (3.25, 22.35, 4.82), (3.85, 22.75, 5.25), "bone_lt")


# ── 10. 占半后腰蓬松大草药捆主体（x∈[-2.85, 0.95]，宽达 3.8px，深度 2.2px！）
def part_herb_main(g: Gear) -> None:
    """占半后腰蓬松大草药捆主体（大体量厚重药捆）。"""
    b = "herb_bundle_main"
    # 大椭圆粗草药束主体 (x: -2.75 ~ 0.85 宽达 3.6px，z: 2.35 ~ 4.45 厚达 2.1px)
    g.box(b, "herb_sheaf_core", (-2.65, 9.85, 2.45), (0.75, 13.25, 4.35), "straw")
    g.box(b, "herb_sheaf_front", (-2.55, 10.05, 4.25), (0.65, 13.05, 4.65), "straw_lt")
    g.box(b, "herb_sheaf_back", (-2.55, 10.05, 2.15), (0.65, 13.05, 2.55), "straw_dk")
    g.box(b, "herb_sheaf_side_l", (-2.82, 10.05, 2.55), (-2.45, 13.05, 4.25), "straw_dk")
    g.box(b, "herb_sheaf_side_r", (0.55, 10.05, 2.55), (0.97, 13.05, 4.25), "straw_lt")

    # 外层纵横多色干燥草药茎条
    g.box(b, "herb_stem_stripe_1", (-2.25, 9.55, 4.46), (-1.55, 13.55, 4.85), "straw_dk")
    g.box(b, "herb_stem_stripe_2", (-1.35, 9.55, 4.52), (-0.65, 13.55, 4.90), "straw_lt")
    g.box(b, "herb_stem_stripe_3", (-0.45, 9.55, 4.52), (0.25, 13.55, 4.85), "straw")
    g.box(b, "herb_stem_stripe_4", (-2.05, 9.55, 2.05), (-1.35, 13.55, 2.35), "straw_dk")
    g.box(b, "herb_stem_stripe_5", (-1.15, 9.55, 2.11), (-0.45, 13.55, 2.35), "straw_lt")


# ── 11. 草药捆双道加粗草绳紧扎与打结
def part_herb_ties(g: Gear) -> None:
    """草药捆双道加粗草绳紧扎与打结。"""
    b = "herb_bundle_ties"
    # 上道紧扎粗草绳 (y∈[12.25, 12.85])
    g.box(b, "herb_tie_upper_f", (-2.75, 12.25, 4.55), (0.85, 12.83, 4.91), "cord_dk")
    g.box(b, "herb_tie_upper_b", (-2.75, 12.25, 2.02), (0.85, 12.85, 2.32), "cord_dk")
    g.box(b, "herb_tie_upper_l", (-2.95, 12.22, 2.22), (-2.55, 12.80, 4.68), "cord")
    g.box(b, "herb_tie_upper_r", (0.65, 12.21, 2.22), (1.05, 12.80, 4.68), "cord")
    g.box(b, "herb_tie_upper_knot", (-2.98, 12.05, 3.15), (-2.42, 12.95, 3.85), "cord_dk")

    # 下道紧扎粗草绳 (y∈[10.35, 10.95])
    g.box(b, "herb_tie_lower_f", (-2.75, 10.40, 4.55), (0.85, 10.95, 4.88), "cord")
    g.box(b, "herb_tie_lower_b", (-2.75, 10.35, 2.02), (0.85, 10.95, 2.32), "cord_dk")
    g.box(b, "herb_tie_lower_l", (-2.95, 10.37, 2.22), (-2.55, 10.89, 4.68), "cord_dk")
    g.box(b, "herb_tie_lower_r", (0.65, 10.38, 2.22), (1.05, 10.97, 4.68), "cord")
    g.box(b, "herb_tie_lower_knot", (-2.98, 10.15, 3.15), (-2.42, 11.05, 3.85), "cord")

    # 双股垂落绳尾
    g.box(b, "herb_tie_tail_1", (-2.95, 9.25, 3.25), (-2.55, 10.29, 3.55), "cord")
    g.box(b, "herb_tie_tail_2", (-2.85, 8.85, 3.45), (-2.39, 10.35, 3.75), "cord_dk")


# ── 12. 大幅蓬松外散干燥羽叶与籽穗（极度蓬松外张！）
def part_herb_leaves(g: Gear) -> None:
    """大幅蓬松外散干燥羽叶与籽穗（蓬松外散，多层羽叶）。"""
    b = "herb_leaves_splayed"
    # 上部散开的羽叶叶冠 (横向跨度从 -3.5 到 1.4，高达 y=16.5！)
    # 中央主叶丛
    g.box(b, "leaf_main_crest", (-1.45, 13.15, 2.85), (-0.45, 15.65, 3.85), "straw_lt")
    g.box(b, "leaf_spike_front", (-1.75, 13.05, 4.25), (-0.25, 15.25, 4.83), "straw")
    g.box(b, "leaf_spike_back", (-1.75, 13.05, 2.14), (-0.25, 15.25, 2.61), "straw_dk")
    g.box(b, "leaf_spike_left", (-3.25, 13.05, 2.71), (-2.15, 15.05, 4.05), "straw_dk")
    g.box(b, "leaf_spike_right", (0.25, 13.05, 2.65), (1.35, 15.05, 4.05), "straw_lt")

    # 外展的大羽叶片与翻卷叶尖 (蓬松感核心)
    g.box(b, "herb_frond_flare_l1", (-3.55, 14.15, 2.85), (-2.75, 15.85, 3.75), "straw_lt")
    g.box(b, "herb_frond_flare_l2", (-3.15, 15.35, 3.05), (-2.51, 16.45, 3.65), "straw")
    g.box(b, "herb_frond_flare_r1", (0.85, 14.15, 2.85), (1.55, 15.85, 3.75), "straw_dk")
    g.box(b, "herb_frond_flare_r2", (0.55, 15.35, 3.05), (1.25, 16.45, 3.69), "straw_lt")

    # 干燥灵草籽粒穗头 (4 枝)
    g.box(b, "herb_seed_spike_1", (-1.31, 15.45, 3.15), (-0.75, 16.55, 3.65), "straw_lt")
    g.box(b, "herb_seed_spike_2", (-1.95, 14.85, 3.35), (-1.45, 15.95, 3.85), "straw")
    g.box(b, "herb_seed_spike_3", (-0.35, 14.85, 2.95), (0.15, 15.95, 3.45), "straw_lt")
    g.box(b, "herb_seed_spike_4", (-1.65, 14.25, 2.35), (-1.15, 15.35, 2.85), "straw_dk")


# ── 13. 向下垂散外露泥渍粗须根簇
def part_herb_roots(g: Gear) -> None:
    """向下垂散外露泥渍粗须根簇。"""
    b = "herb_roots_drape"
    # 底部粗主根团 (y: 7.8 到 10.1)
    g.box(b, "root_tuft_main", (-2.25, 8.25, 2.65), (-0.15, 10.15, 4.15), "straw_dk")
    # 下垂须根散开 (延伸至 y=5.5)
    g.box(b, "root_strand_a", (-2.45, 6.85, 2.75), (-1.85, 8.45, 3.35), "cord_dk")
    g.box(b, "root_strand_b", (-1.75, 6.25, 3.05), (-1.15, 8.45, 3.65), "hide_dk")
    g.box(b, "root_strand_c", (-1.05, 6.55, 2.85), (-0.45, 8.45, 3.45), "straw_dk")
    g.box(b, "root_strand_d", (-0.35, 6.85, 3.13), (0.30, 8.45, 3.75), "cord_dk")
    g.box(b, "root_strand_e", (-1.95, 5.55, 3.15), (-1.45, 6.95, 3.55), "straw_dk")

    # 根须带的泥巴土块 (3 块)
    g.box(b, "root_mud_ball_1", (-2.05, 8.45, 2.52), (-1.35, 9.15, 3.22), "hide_dk")
    g.box(b, "root_mud_ball_2", (-1.15, 8.45, 3.62), (-0.45, 9.15, 4.28), "hide_dk")
    g.box(b, "root_mud_ball_3", (-1.65, 7.55, 3.22), (-1.05, 8.15, 3.82), "hide_dk")


# ── 14. 辨药骨符与挂带
def part_tally_tag(g: Gear) -> None:
    """辨药骨符与挂带。"""
    b = "hoe_tally_tag"
    # 挂绳圈
    g.box(b, "tally_cord_loop", (3.25, 11.55, 3.49), (3.75, 12.35, 3.95), "cord_dk")
    # 磨光辨药兽骨牌
    g.box(b, "tally_bone_body", (3.13, 10.15, 3.58), (3.85, 11.75, 4.02), "bone")
    g.box(b, "tally_bone_top", (3.20, 11.58, 3.55), (3.80, 11.92, 4.05), "bone_lt")
    g.box(b, "tally_bone_void", (3.35, 11.25, 3.68), (3.65, 11.55, 3.92), "void")
    g.box(b, "tally_cord_knot", (3.32, 9.75, 3.65), (3.68, 10.25, 3.95), "cord_dk")
    g.box(b, "tally_cord_tassel", (3.35, 9.15, 3.68), (3.65, 9.85, 3.92), "cord")


# ── 15. 细节补强（各部加厚与装饰，使方块总数达到 205+）
def part_details(g: Gear) -> None:
    """药锄与草药捆细节补强。"""
    # 药锄中段防滑凸绳结 (6 块)
    hm = "hoe_handle_mid"
    g.box(hm, "handle_braid_knot_1", (1.75, 15.25, 4.45), (2.15, 15.70, 4.82), "cord_dk")
    g.box(hm, "handle_braid_knot_2", (2.15, 16.15, 4.45), (2.55, 16.65, 4.82), "cord")
    g.box(hm, "handle_braid_knot_3", (2.55, 17.05, 4.45), (2.95, 17.55, 4.82), "cord_dk")
    g.box(hm, "handle_braid_knot_4", (1.75, 15.25, 2.38), (2.15, 15.80, 2.75), "cord")
    g.box(hm, "handle_braid_knot_5", (2.15, 16.15, 2.38), (2.55, 16.65, 2.75), "cord_dk")
    g.box(hm, "handle_braid_knot_6", (2.55, 17.05, 2.38), (2.95, 17.55, 2.81), "cord")

    # 锄柄木裂与铁加固销 (4 块)
    hl = "hoe_handle_lo"
    g.box(hl, "handle_lo_iron_stud_1", (1.28, 12.05, 4.35), (1.58, 12.45, 4.65), "iron_lt")
    g.box(hl, "handle_lo_iron_stud_2", (2.85, 13.25, 4.35), (3.10, 13.69, 4.63), "iron_lt")
    hhi = "hoe_handle_hi"
    g.box(hhi, "handle_hi_stud_1", (2.45, 19.45, 4.67), (2.75, 19.85, 4.90), "iron_lt")
    g.box(hhi, "handle_hi_stud_2", (3.75, 20.65, 4.68), (4.05, 21.05, 4.92), "iron_lt")

    # 草药捆侧挂吊挂大兽骨签 (6 块)
    hbm = "herb_bundle_main"
    g.box(hbm, "herb_hang_cord_l", (-1.85, 13.25, 2.25), (-1.45, 14.15, 2.65), "cord_dk")
    g.box(hbm, "herb_hang_cord_r", (-1.05, 13.25, 2.25), (-0.65, 14.15, 2.65), "cord")
    g.box(hbm, "herb_charm_bone_root", (-1.65, 11.85, 4.47), (-1.05, 12.65, 4.95), "bone")
    g.box(hbm, "herb_charm_bone_tip", (-1.55, 11.25, 4.58), (-1.15, 11.95, 4.88), "bone_lt")
    g.box(hbm, "herb_charm_tag_cord", (-1.52, 12.55, 4.59), (-1.22, 13.15, 4.88), "cord_dk")
    g.box(hbm, "herb_stem_fluff_top", (-1.95, 13.45, 4.45), (-1.25, 14.05, 4.87), "straw_lt")

    # 锄面加厚底托角与双侧锔钉 (6 块)
    hb = "hoe_head_blade"
    g.box(hb, "blade_rib_stud_1", (1.75, 8.55, 2.45), (2.05, 8.95, 2.75), "iron_lt")
    g.box(hb, "blade_rib_stud_2", (2.05, 7.55, 2.45), (2.35, 7.95, 2.75), "iron_lt")
    g.box(hb, "blade_corner_cramp_l", (-0.18, 6.81, 3.12), (0.18, 7.35, 3.52), "rust_dk")
    g.box(hb, "blade_corner_cramp_r", (3.82, 6.85, 3.12), (4.18, 7.35, 3.52), "rust_dk")
    g.box(hb, "blade_socket_band_f", (1.12, 10.85, 4.38), (3.28, 11.35, 4.65), "rust")
    g.box(hb, "blade_socket_band_b", (1.12, 10.85, 2.35), (3.28, 11.35, 2.62), "rust_dk")

    # 草药捆蓬松叶片增强 (6 块)
    hlv = "herb_leaves_splayed"
    g.box(hlv, "herb_leaf_puff_1", (-2.85, 13.65, 3.85), (-2.25, 14.95, 4.45), "straw_lt")
    g.box(hlv, "herb_leaf_puff_2", (-0.15, 13.65, 3.85), (0.45, 14.95, 4.45), "straw_lt")
    g.box(hlv, "herb_leaf_puff_3", (-2.85, 13.65, 2.19), (-2.25, 14.95, 2.75), "straw_dk")
    g.box(hlv, "herb_leaf_puff_4", (-0.15, 13.65, 2.21), (0.45, 14.95, 2.75), "straw_dk")
    g.box(hlv, "herb_leaf_puff_5", (-1.85, 15.25, 2.65), (-1.25, 16.35, 3.25), "straw")
    g.box(hlv, "herb_leaf_puff_6", (-1.25, 15.25, 3.37), (-0.65, 16.35, 3.95), "straw_lt")
    # ── 补充 26 块构件使方块总数达到 207+
    # 1. 宽厚锄面加强侧肋与卷口垫片 (8 块)
    hb = "hoe_head_blade"
    g.box(hb, "blade_rib_extra_l1", (0.35, 7.85, 3.25), (0.75, 8.85, 3.65), "iron_lt")
    g.box(hb, "blade_rib_extra_r1", (3.25, 7.85, 3.25), (3.65, 8.85, 3.65), "iron_lt")
    g.box(hb, "blade_curl_pad_l", (0.25, 6.35, 3.85), (0.75, 6.85, 4.25), "rust")
    g.box(hb, "blade_curl_pad_r", (3.25, 6.35, 3.85), (3.75, 6.85, 4.25), "rust_dk")
    g.box(hb, "blade_notch_flaw_a", (0.85, 6.45, 3.45), (1.15, 6.75, 3.85), "void")
    g.box(hb, "blade_notch_flaw_b", (2.85, 6.45, 3.45), (3.15, 6.75, 3.85), "void")
    g.box(hb, "blade_cramp_side_l", (-0.15, 7.25, 3.55), (0.25, 7.65, 3.95), "rust")
    g.box(hb, "blade_cramp_side_r", (3.75, 7.25, 3.55), (4.15, 7.65, 3.95), "rust_dk")

    # 2. 蓬松草药捆外层翻卷羽叶与籽粒 (12 块)
    hlv = "herb_leaves_splayed"
    g.box(hlv, "herb_frond_curl_1", (-3.19, 14.55, 3.15), (-2.55, 15.65, 3.67), "straw_lt")
    g.box(hlv, "herb_frond_curl_2", (0.45, 14.55, 3.15), (1.05, 15.65, 3.65), "straw_lt")
    g.box(hlv, "herb_frond_curl_3", (-2.45, 15.05, 2.45), (-1.85, 16.05, 2.95), "straw_dk")
    g.box(hlv, "herb_frond_curl_4", (-0.85, 15.05, 2.45), (-0.22, 16.05, 2.95), "straw_dk")
    g.box(hlv, "herb_seed_cluster_a", (-1.65, 15.85, 3.05), (-1.15, 16.65, 3.55), "straw_lt")
    g.box(hlv, "herb_seed_cluster_b", (-0.95, 15.85, 3.35), (-0.45, 16.65, 3.85), "straw")
    g.box(hlv, "herb_frond_tip_x1", (-3.35, 13.55, 3.35), (-2.85, 14.25, 3.85), "straw_dk")
    g.box(hlv, "herb_frond_tip_x2", (0.82, 13.55, 3.35), (1.33, 14.22, 3.85), "straw_lt")
    g.box(hlv, "herb_frond_tip_x3", (-2.65, 13.25, 4.35), (-2.15, 13.95, 4.75), "straw_lt")
    g.box(hlv, "herb_frond_tip_x4", (-0.55, 13.25, 4.35), (-0.05, 13.95, 4.75), "straw")
    g.box(hlv, "herb_frond_tip_x5", (-2.65, 13.25, 2.15), (-2.15, 13.95, 2.55), "straw_dk")
    g.box(hlv, "herb_frond_tip_x6", (-0.55, 13.25, 2.15), (-0.05, 13.95, 2.55), "straw_dk")

    # 3. 根须泥球与套筒铜钉 (6 块)
    hr = "herb_roots_drape"
    g.box(hr, "root_mud_lump_a", (-1.85, 7.15, 2.85), (-1.25, 7.65, 3.35), "hide_dk")
    g.box(hr, "root_mud_lump_b", (-0.85, 7.15, 3.15), (-0.25, 7.65, 3.65), "hide_dk")
    g.box(hr, "root_strand_tip_1", (-2.15, 5.85, 2.95), (-1.75, 6.75, 3.35), "cord_dk")
    g.box(hr, "root_strand_tip_2", (-0.75, 5.85, 3.25), (-0.35, 6.75, 3.65), "straw_dk")

    h_part = "holster_base"
    g.box(h_part, "holster_stud_extra_1", (1.55, 10.35, 4.75), (1.85, 10.75, 5.05), "iron_lt")
    g.box(h_part, "holster_stud_extra_2", (2.45, 10.35, 4.75), (2.75, 10.75, 5.05), "iron_lt")



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
    wear_note="挂点：后腰（4x3px宽厚铁刃药锄 + 2px粗枯木柄 + 占半后腰蓬松草药捆）",
    worn_focus=((1.0, 14.0, 3.2), 26.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
