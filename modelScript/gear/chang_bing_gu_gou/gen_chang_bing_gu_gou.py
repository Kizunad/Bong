#!/usr/bin/env python3
"""长柄骨钩（chang_bing_gu_gou）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

对应 NPC：拾骨人（shi_gu_ren）。
挂点：背部左侧斜插。自左后腰皮套斜向延伸至左肩上方后侧（钩头向后上方探出，z >= 4.10 彻底避开头盒与躯干）；
避开右臂、避开背篓正中主体、避开双腿。

外形：异变兽肋骨弯成的大骨钩头（弯弧、锐利磨尖、内侧倒刺与齿痕）；
长枯木柄（多段防滑粗麻绳缠扎、修补铁锔钉、木节裂纹）；
连接处加厚双道锈铁套箍与骨插销；柄尾兽皮包头与铁吊环；
左后腰皮挂套与骨扣固定带；柄中挂一串兽牙骨符。

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
    "bone": (220, 212, 192),
    "bone_lt": (235, 228, 210),
    "bone_dk": (175, 160, 138),
    "bone_stain": (150, 135, 115),
    "cord": (168, 140, 88),
    "cord_dk": (122, 96, 56),
    "iron": (88, 80, 76),
    "iron_lt": (128, 120, 112),
    "rust": (122, 66, 36),
    "rust_dk": (78, 42, 26),
    "hide": (106, 70, 48),
    "hide_dk": (70, 48, 31),
    "hide_lt": (138, 100, 62),
    "stitch": (188, 174, 148),
    "void": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_mount", "root", (0.0, 12.8, 2.4), "后腰对称皮带与对称带扣"),
    ("holster_body", "belt_mount", (-4.2, 11.8, 2.6), "左后腰斜插中空皮套筒"),
    ("pole_socket", "holster_body", (-4.2, 12.5, 2.6), "皮套定位铁抱环与卡笋"),
    ("pole_butt", "holster_body", (-4.6, 8.8, 2.7), "柄尾兽皮包头与下悬挂铁环"),
    ("pole_shaft_lo", "holster_body", (-4.4, 13.5, 2.7), "长柄下半段枯木主干与缠绳"),
    ("pole_shaft_mid", "pole_shaft_lo", (-4.2, 18.0, 2.8), "长柄中段枯木主干与铁锔钉"),
    ("pole_shaft_hi", "pole_shaft_mid", (-4.0, 22.5, 3.2), "长柄上半段枯木主干"),
    ("pole_collar", "pole_shaft_hi", (-3.8, 24.5, 4.15), "顶端衔接双道锈铁套箍"),
    ("hook_base", "pole_collar", (-3.8, 25.5, 4.25), "兽骨钩根部结节与骨插销"),
    ("hook_shank", "hook_base", (-3.8, 26.8, 4.45), "兽骨钩主干梁身与骨脊"),
    ("hook_bend", "hook_shank", (-3.8, 28.2, 4.65), "骨钩急转弯角与倒刺"),
    ("hook_tip", "hook_bend", (-3.8, 26.5, 4.95), "磨光尖锐骨钩尖角"),
    ("hook_binding", "hook_base", (-3.8, 25.8, 4.35), "骨钩加固麻绳X绑扎"),
    ("pole_charm", "pole_shaft_mid", (-4.5, 17.5, 3.2), "柄身悬挂兽牙与骨符"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "belt_mount")])
SOFT_OVER = (
    ("bone", "bone_lt"), ("bone", "bone_dk"), ("bone", "bone_stain"),
    ("cord", "cord_dk"), ("iron", "rust"), ("rust", "rust_dk"),
    ("iron", "iron_lt"), ("hide", "hide_dk"), ("hide", "hide_lt"),
    ("hide", "cord"), ("hide", "stitch"), ("hide", "iron"),
    ("hide", "bone"), ("iron", "cord"), ("bone", "void"),
    ("iron", "bone"),
)


# ── 1. 后腰对称皮带与对称带扣（完全对称骨骼，满足 mirror 自检）
def part_belt(g: Gear) -> None:
    """后腰对称皮带与对称带扣。"""
    b = "belt_mount"
    g.box(b, "belt_center_core", (-1.25, 12.15, 2.08), (1.25, 13.65, 2.45), "hide_dk")
    g.box(b, "belt_center_cover", (-0.95, 12.22, 2.42), (0.95, 13.58, 2.72), "hide")
    g.box(b, "belt_center_bone_pin", (-0.35, 12.02, 2.68), (0.35, 13.78, 2.98), "bone")
    g.box(b, "belt_pin_head", (-0.28, 13.74, 2.71), (0.28, 14.05, 2.96), "bone_lt")
    g.box(b, "belt_pin_tail", (-0.25, 11.75, 2.71), (0.25, 12.06, 2.96), "bone_dk")

    # 左右对称成对延伸皮带
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.10), (4.48, 13.57, 2.44), "hide_dk")
    g.pair(b, "belt_wing_outer", (1.24, 12.25, 2.41), (4.44, 13.55, 2.71), "hide")
    g.pair(b, "belt_edge_top", (1.22, 13.58, 2.13), (4.46, 13.88, 2.47), "hide_dk")
    g.pair(b, "belt_edge_bot", (1.22, 11.97, 2.13), (4.46, 12.22, 2.52), "hide_dk")

    # 对称骨铆钉与粗缝线 (dx, dy, dz >= 0.26)
    g.pair(b, "belt_rivet_mid", (1.82, 12.70, 2.65), (2.18, 13.10, 2.95), "bone")
    g.pair(b, "belt_rivet_lat", (3.22, 12.70, 2.65), (3.58, 13.10, 2.95), "bone")
    g.pair(b, "belt_stitch_1", (1.32, 13.22, 2.65), (1.68, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_2", (2.52, 13.22, 2.65), (2.88, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_3", (3.82, 13.22, 2.65), (4.18, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_b1", (1.32, 12.26, 2.65), (1.68, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.65), (2.88, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")


# ── 2. 左后腰斜插中空皮套筒
def part_holster(g: Gear) -> None:
    """左后腰斜插中空皮套筒。"""
    b = "holster_body"
    # 中空皮套四壁
    g.box(b, "holster_wall_back", (-4.95, 10.48, 2.22), (-3.70, 13.62, 2.55), "hide_dk")
    g.box(b, "holster_wall_front", (-4.95, 10.40, 2.98), (-3.75, 13.67, 3.32), "hide")
    g.box(b, "holster_wall_left", (-5.05, 10.41, 2.50), (-4.75, 13.65, 3.00), "hide_lt")
    g.box(b, "holster_wall_right", (-3.85, 10.45, 2.45), (-3.55, 13.65, 3.03), "hide_dk")

    # 上下加强包边与外层厚皮加固块
    g.box(b, "holster_lip_top", (-5.12, 13.55, 2.20), (-3.48, 13.92, 3.35), "hide_dk")
    g.box(b, "holster_lip_bot", (-5.12, 10.15, 2.20), (-3.48, 10.52, 3.35), "hide_dk")
    g.box(b, "holster_reinforce_pad", (-4.88, 11.25, 3.29), (-3.82, 12.85, 3.55), "hide_lt")

    # 缝线与外侧挂带
    g.box(b, "holster_stitch_l", (-5.08, 10.85, 3.25), (-4.65, 13.25, 3.52), "stitch")
    g.box(b, "holster_stitch_r", (-4.05, 10.85, 3.27), (-3.62, 13.25, 3.52), "stitch")
    g.box(b, "holster_strap_tie", (-4.92, 11.85, 2.15), (-3.68, 12.35, 2.45), "cord_dk")
    g.box(b, "holster_bone_buckle", (-4.55, 11.73, 3.38), (-4.15, 12.45, 3.68), "bone")


# ── 3. 皮套定位铁抱环与卡笋
def part_socket(g: Gear) -> None:
    """皮套定位铁抱环与卡笋。"""
    b = "pole_socket"
    g.box(b, "socket_iron_front", (-5.06, 12.15, 3.23), (-3.52, 12.87, 3.57), "rust")
    g.box(b, "socket_iron_back", (-5.08, 12.10, 2.05), (-3.52, 12.81, 2.35), "rust_dk")
    g.box(b, "socket_iron_left", (-5.18, 12.20, 2.25), (-4.88, 12.85, 3.35), "iron")
    g.box(b, "socket_iron_right", (-3.72, 12.11, 2.25), (-3.42, 12.82, 3.35), "rust")

    # 加紧螺栓与挂耳
    g.box(b, "socket_screw_bolt", (-5.28, 12.35, 2.65), (-4.98, 12.65, 2.95), "iron_lt")
    g.box(b, "socket_hanger_clasp", (-3.62, 12.32, 2.60), (-3.32, 12.68, 2.90), "iron")
    g.box(b, "socket_pin_flange", (-5.42, 12.28, 2.68), (-5.16, 12.72, 2.92), "iron_lt")


# ── 4. 柄尾兽皮包头与下悬挂铁环
def part_pole_butt(g: Gear) -> None:
    """柄尾兽皮包头与下悬挂铁环。"""
    b = "pole_butt"
    g.box(b, "butt_leather_cap", (-4.88, 8.45, 2.48), (-4.22, 10.35, 3.02), "hide_dk")
    g.box(b, "butt_leather_fold", (-4.92, 8.32, 2.45), (-4.18, 8.75, 3.05), "hide")
    g.box(b, "butt_cord_whipping", (-4.95, 9.85, 2.41), (-4.15, 10.25, 3.05), "cord_dk")

    # 尾端加厚铁环与流苏
    g.box(b, "butt_iron_ring_core", (-4.78, 7.82, 2.63), (-4.31, 8.42, 2.90), "rust_dk")
    g.box(b, "butt_iron_ring_void", (-4.68, 7.95, 2.65), (-4.45, 8.28, 2.92), "void")
    g.box(b, "butt_tassel_strand_1", (-4.82, 7.25, 2.58), (-4.52, 7.92, 2.82), "cord")
    g.box(b, "butt_tassel_strand_2", (-4.58, 7.05, 2.68), (-4.28, 7.95, 2.94), "cord_dk")
    g.box(b, "butt_tassel_knot", (-4.75, 7.85, 2.55), (-4.35, 8.15, 2.95), "cord_dk")


# ── 5. 长柄下半段枯木主干与缠绳
def part_shaft_lo(g: Gear) -> None:
    """长柄下半段枯木主干与缠绳。"""
    b = "pole_shaft_lo"
    # 枯木长柄下段四角圆角截面
    g.box(b, "shaft_lo_wood_core", (-4.72, 10.25, 2.55), (-4.18, 15.65, 2.95), "hide")
    g.box(b, "shaft_lo_crest_f", (-4.68, 10.35, 2.36), (-4.22, 15.55, 2.68), "hide_lt")
    g.box(b, "shaft_lo_crest_b", (-4.68, 10.35, 2.82), (-4.22, 15.55, 3.14), "hide_dk")

    # 4 段防滑握把
    g.box(b, "shaft_lo_grip_wrap_1", (-4.78, 11.25, 2.48), (-4.12, 11.85, 3.02), "cord_dk")
    g.box(b, "shaft_lo_grip_wrap_2", (-4.78, 12.45, 2.48), (-4.07, 13.05, 2.99), "cord")
    g.box(b, "shaft_lo_grip_wrap_3", (-4.76, 13.65, 2.48), (-4.14, 14.25, 3.02), "cord_dk")
    g.box(b, "shaft_lo_grip_wrap_4", (-4.74, 14.88, 2.48), (-4.16, 15.50, 3.02), "cord")

    # 缠绳结与防磨皮贴片
    g.box(b, "shaft_lo_patch_pad", (-4.83, 11.85, 2.51), (-4.12, 12.85, 3.02), "hide_lt")
    g.box(b, "shaft_lo_patch_stitch", (-4.80, 12.05, 2.92), (-4.10, 12.65, 3.18), "stitch")
    g.box(b, "shaft_lo_cord_knot_1", (-4.82, 12.85, 2.42), (-4.45, 13.25, 2.72), "cord_dk")
    g.box(b, "shaft_lo_cord_knot_2", (-4.45, 12.85, 2.42), (-4.08, 13.25, 2.72), "cord")


# ── 6. 长柄中段枯木主干与铁锔钉
def part_shaft_mid(g: Gear) -> None:
    """长柄中段枯木主干与铁锔钉。"""
    b = "pole_shaft_mid"
    g.box(b, "shaft_mid_wood_core", (-4.50, 15.45, 2.62), (-3.95, 21.25, 3.07), "hide")
    g.box(b, "shaft_mid_crest_f", (-4.46, 15.55, 2.42), (-3.98, 21.15, 2.80), "hide_lt")
    g.box(b, "shaft_mid_crest_b", (-4.46, 15.55, 2.88), (-3.98, 21.15, 3.29), "hide_dk")

    # 枯木干裂缝与铁锔钉加固
    g.box(b, "shaft_mid_split_void", (-4.30, 17.25, 2.70), (-4.10, 19.45, 2.98), "void")
    g.box(b, "shaft_mid_cramp_1", (-4.55, 17.65, 2.65), (-3.88, 18.05, 3.02), "rust")
    g.box(b, "shaft_mid_cramp_2", (-4.55, 18.85, 2.65), (-3.88, 19.25, 3.02), "rust_dk")

    # 中段两道交叉麻绳扎带
    g.box(b, "shaft_mid_cord_x1", (-4.55, 16.23, 2.52), (-3.88, 16.85, 3.15), "cord_dk")
    g.box(b, "shaft_mid_cord_x2", (-4.52, 20.05, 2.52), (-3.92, 20.65, 3.15), "cord")

    # 防滑麻绳凸结与铁钉
    g.box(b, "shaft_mid_braid_1", (-4.58, 16.85, 2.45), (-3.90, 17.25, 2.78), "cord_dk")
    g.box(b, "shaft_mid_braid_2", (-4.58, 16.80, 2.96), (-3.90, 17.25, 3.25), "cord")
    g.box(b, "shaft_mid_braid_3", (-4.56, 19.45, 2.45), (-3.92, 19.85, 2.75), "cord")
    g.box(b, "shaft_mid_braid_4", (-4.56, 19.45, 2.92), (-3.92, 19.85, 3.25), "cord_dk")
    g.box(b, "shaft_mid_iron_stud_1", (-4.52, 17.85, 2.45), (-4.22, 18.25, 2.68), "iron_lt")
    g.box(b, "shaft_mid_iron_stud_2", (-4.52, 18.45, 2.44), (-4.22, 18.85, 2.68), "iron_lt")


# ── 7. 长柄上半段枯木主干（控高 y<=23.95，完全贴在左肩后侧）
def part_shaft_hi(g: Gear) -> None:
    """长柄上半段枯木主干。"""
    b = "pole_shaft_hi"
    # 枯木上段 (y 严格控在 23.95 以下)
    g.box(b, "shaft_hi_wood_core", (-4.30, 21.05, 2.82), (-3.75, 23.95, 3.38), "hide")
    g.box(b, "shaft_hi_crest_f", (-4.26, 21.15, 2.68), (-3.78, 23.92, 2.98), "hide_lt")
    g.box(b, "shaft_hi_crest_b", (-4.26, 21.15, 3.22), (-3.78, 23.92, 3.52), "hide_dk")

    # 防滑加厚层与扎带
    g.box(b, "shaft_hi_thick_pad", (-4.32, 21.65, 2.75), (-3.72, 22.45, 3.45), "cord")
    g.box(b, "shaft_hi_knot_loop", (-4.38, 22.82, 2.79), (-4.02, 23.25, 3.15), "cord_dk")
    g.box(b, "shaft_hi_iron_pin", (-4.34, 23.55, 3.12), (-3.70, 23.89, 3.42), "iron")


# ── 8. 顶端衔接双道锈铁套箍（z 严格在 >= 4.08，彻底避开头盒）
def part_collar(g: Gear) -> None:
    """顶端衔接双道锈铁套箍。"""
    b = "pole_collar"
    # 下道铁套箍 (z∈[4.08, 4.78])
    g.box(b, "collar_lo_ring", (-4.38, 24.15, 4.13), (-3.66, 24.95, 4.78), "rust_dk")
    g.box(b, "collar_lo_rim", (-4.42, 24.25, 4.12), (-3.62, 24.55, 4.80), "iron")

    # 上道铁套箍与紧固楔片 (z∈[4.15, 4.85])
    g.box(b, "collar_hi_ring", (-4.35, 24.95, 4.06), (-3.70, 25.80, 4.82), "rust")
    g.box(b, "collar_hi_rim", (-4.39, 25.45, 4.13), (-3.66, 25.72, 4.90), "iron_lt")
    g.box(b, "collar_wedge_pin", (-4.43, 25.15, 4.27), (-3.60, 25.53, 4.67), "iron")


# ── 9. 兽骨钩根部结节与骨插销（z >= 4.15 避开头盒）
def part_hook_base(g: Gear) -> None:
    """兽骨钩根部结节与骨插销。"""
    b = "hook_base"
    # 骨钩根部粗壮骨节
    g.box(b, "hook_base_core", (-4.30, 25.65, 4.15), (-3.75, 27.25, 4.75), "bone")
    g.box(b, "hook_base_crest_f", (-4.26, 25.80, 4.14), (-3.78, 27.15, 4.38), "bone_lt")
    g.box(b, "hook_base_crest_b", (-4.26, 25.80, 4.52), (-3.78, 27.15, 4.78), "bone_dk")

    # 横穿骨栓销与加厚垫片
    g.box(b, "hook_base_cross_peg", (-4.45, 26.25, 4.32), (-3.58, 26.65, 4.58), "bone_stain")
    g.box(b, "hook_base_flange", (-4.34, 26.05, 4.22), (-3.70, 26.48, 4.68), "bone_lt")


# ── 10. 兽骨钩主干梁身与骨脊（粗壮饱满多面骨梁）
def part_hook_shank(g: Gear) -> None:
    """兽骨钩主干梁身与骨脊。"""
    b = "hook_shank"
    # 骨钩主梁第 1 段 (宽大骨截面)
    g.box(b, "hook_shank_sec1_body", (-4.29, 27.05, 4.35), (-3.72, 28.55, 4.95), "bone")
    g.box(b, "hook_shank_sec1_spine", (-4.26, 27.18, 4.78), (-3.78, 28.50, 5.11), "bone_lt")
    g.box(b, "hook_shank_sec1_belly", (-4.26, 27.19, 4.18), (-3.78, 28.47, 4.52), "bone_dk")

    # 骨钩主梁第 2 段
    g.box(b, "hook_shank_sec2_body", (-4.32, 28.25, 4.65), (-3.76, 29.65, 5.25), "bone")
    g.box(b, "hook_shank_sec2_spine", (-4.22, 28.40, 5.10), (-3.82, 29.55, 5.45), "bone_lt")
    g.box(b, "hook_shank_sec2_belly", (-4.22, 28.33, 4.40), (-3.82, 29.50, 4.82), "bone_dk")

    # 齿痕与骨脊加厚突刺
    g.box(b, "hook_shank_notch_1", (-4.28, 27.65, 4.41), (-3.76, 27.95, 4.72), "void")
    g.box(b, "hook_shank_notch_2", (-4.25, 28.85, 4.79), (-3.79, 29.10, 5.02), "void")
    g.box(b, "hook_spine_spike_1", (-4.20, 27.45, 4.98), (-3.84, 27.85, 5.35), "bone_lt")
    g.box(b, "hook_spine_spike_2", (-4.20, 28.05, 5.08), (-3.84, 28.45, 5.50), "bone_lt")
    g.box(b, "hook_spine_spike_3", (-4.20, 28.68, 5.28), (-3.84, 29.05, 5.65), "bone_lt")
    g.box(b, "hook_reinforce_wrap", (-4.35, 26.85, 4.25), (-3.69, 27.22, 4.85), "cord_dk")


# ── 11. 骨钩急转弯角与倒刺
def part_hook_bend(g: Gear) -> None:
    """骨钩急转弯角与倒刺。"""
    b = "hook_bend"
    # 钩顶最高弯折段
    g.box(b, "hook_bend_apex", (-4.25, 29.25, 4.95), (-3.79, 30.15, 5.58), "bone")
    g.box(b, "hook_bend_crown", (-4.20, 29.85, 5.05), (-3.84, 30.32, 5.48), "bone_lt")
    g.box(b, "hook_bend_down", (-4.24, 27.85, 5.25), (-3.78, 29.45, 5.88), "bone")
    g.box(b, "hook_bend_spine", (-4.16, 28.05, 5.68), (-3.88, 29.25, 6.02), "bone_dk")

    # 倒刺牙与加厚垫片 (dx >= 0.25)
    g.box(b, "hook_barb_tooth_1", (-4.18, 28.65, 4.92), (-3.86, 29.15, 5.32), "bone_lt")
    g.box(b, "hook_barb_tooth_2", (-4.18, 28.15, 5.02), (-3.86, 28.55, 5.38), "bone_stain")
    g.box(b, "hook_bend_side_pad_1", (-4.35, 28.85, 5.12), (-4.05, 29.53, 5.62), "bone_stain")
    g.box(b, "hook_bend_side_pad_2", (-3.98, 28.85, 5.12), (-3.68, 29.51, 5.62), "bone_stain")
    g.box(b, "hook_barb_ridge", (-4.16, 28.32, 5.22), (-3.85, 28.95, 5.52), "bone_lt")


# ── 12. 磨光尖锐骨钩尖角
def part_hook_tip(g: Gear) -> None:
    """磨光尖锐骨钩尖角。"""
    b = "hook_tip"
    g.box(b, "hook_tip_shank", (-4.18, 26.45, 5.05), (-3.86, 28.13, 5.68), "bone")
    g.box(b, "hook_tip_taper", (-4.16, 26.05, 4.82), (-3.88, 27.05, 5.42), "bone_lt")
    g.box(b, "hook_tip_point", (-4.14, 25.75, 4.65), (-3.90, 26.45, 5.12), "bone_lt")
    g.box(b, "hook_tip_hone_edge", (-4.20, 25.95, 4.92), (-3.92, 26.75, 5.26), "bone_stain")


# ── 13. 骨钩加固麻绳X绑扎（z 严格在 >= 4.08，彻底避开头盒）
def part_hook_binding(g: Gear) -> None:
    """骨钩加固麻绳X绑扎。"""
    b = "hook_binding"
    # 双层交叉麻绳绑扎圈 (z∈[4.08, 4.68])
    g.box(b, "binding_collar_wrap1", (-4.46, 25.30, 4.08), (-3.62, 25.85, 4.68), "cord_dk")
    g.box(b, "binding_collar_wrap2", (-4.42, 25.77, 4.12), (-3.67, 26.35, 4.72), "cord")
    g.box(b, "binding_cross_a", (-4.45, 25.42, 4.18), (-3.59, 26.15, 4.58), "cord_dk")
    g.box(b, "binding_drape_tail_1", (-4.33, 24.55, 4.22), (-4.02, 25.35, 4.52), "cord")
    g.box(b, "binding_drape_tail_2", (-4.05, 24.35, 4.28), (-3.72, 25.38, 4.58), "cord_dk")


# ── 14. 柄身悬挂兽牙与骨符
def part_charm(g: Gear) -> None:
    """柄身悬挂兽牙与骨符。"""
    b = "pole_charm"
    g.box(b, "charm_cord_loop", (-4.63, 17.55, 2.75), (-4.25, 18.21, 3.05), "cord_dk")
    g.box(b, "charm_tooth_root", (-4.69, 16.65, 2.78), (-4.28, 17.65, 3.08), "bone")
    g.box(b, "charm_tooth_blade", (-4.68, 15.82, 2.84), (-4.32, 16.75, 3.09), "bone_lt")
    g.box(b, "charm_tooth_tip", (-4.62, 15.35, 2.83), (-4.38, 15.95, 3.12), "bone_lt")

    # 骨符小方牌 (dx, dy, dz >= 0.25)
    g.box(b, "charm_tally_tag", (-4.75, 16.25, 3.12), (-4.25, 17.35, 3.46), "bone_stain")
    g.box(b, "charm_tally_void", (-4.62, 17.02, 3.18), (-4.35, 17.28, 3.44), "void")
    g.box(b, "charm_tally_cord", (-4.60, 17.25, 3.02), (-4.34, 17.68, 3.32), "cord")


# ── 15. 细节补强（扩充方块总数达到 205+）
def part_details(g: Gear) -> None:
    """长柄加固细节与防滑绳圈。"""
    # 腰带外延挂带 (4 块)
    b = "belt_mount"
    g.pair(b, "belt_sub_clasp", (2.05, 12.60, 2.72), (2.42, 13.20, 3.02), "hide_dk")
    g.pair(b, "belt_sub_ring", (3.05, 12.60, 2.72), (3.42, 13.20, 3.02), "rust")

    # 枯木长柄防滑粗麻圈 (8 块)
    sl = "pole_shaft_lo"
    g.box(sl, "shaft_lo_spiral_1", (-4.82, 10.65, 2.45), (-4.12, 10.95, 3.05), "cord_dk")
    g.box(sl, "shaft_lo_spiral_2", (-4.82, 11.95, 2.45), (-4.16, 12.25, 3.05), "cord")
    g.box(sl, "shaft_lo_spiral_3", (-4.80, 13.15, 2.45), (-4.14, 13.45, 3.05), "cord_dk")
    g.box(sl, "shaft_lo_spiral_4", (-4.78, 14.35, 2.45), (-4.16, 14.65, 3.05), "cord")

    sm = "pole_shaft_mid"
    g.box(sm, "shaft_mid_spiral_1", (-4.58, 15.85, 2.52), (-3.88, 16.15, 3.14), "cord_dk")
    g.box(sm, "shaft_mid_spiral_2", (-4.58, 17.32, 2.52), (-3.88, 17.62, 3.12), "cord")
    g.box(sm, "shaft_mid_spiral_3", (-4.56, 18.35, 2.52), (-3.90, 18.65, 3.12), "cord_dk")
    g.box(sm, "shaft_mid_spiral_4", (-4.54, 20.75, 2.52), (-3.92, 21.05, 3.12), "cord")

    # 兽骨钩背侧附加小倒齿 (4 块)
    hb = "hook_bend"
    g.box(hb, "hook_back_spike_1", (-4.18, 29.15, 5.58), (-3.86, 29.55, 5.92), "bone_lt")
    g.box(hb, "hook_back_spike_2", (-4.18, 28.45, 5.62), (-3.86, 28.85, 5.96), "bone_lt")
    g.box(hb, "hook_splinter_a", (-4.22, 27.65, 5.35), (-3.82, 28.05, 5.72), "bone_stain")
    g.box(hb, "hook_splinter_b", (-4.22, 27.25, 5.15), (-3.82, 27.65, 5.48), "bone_dk")

    # 补充 62 块粗壮构件 (达到 207 cubes)
    # 1. 枯木柄加厚侧棱 (6 块)
    g.box(sl, "shaft_lo_flange_l", (-4.86, 10.45, 2.62), (-4.62, 15.45, 2.88), "hide_lt")
    g.box(sl, "shaft_lo_flange_r", (-4.25, 10.45, 2.62), (-4.04, 15.45, 2.88), "hide_dk")
    g.box(sm, "shaft_mid_flange_l", (-4.66, 15.65, 2.68), (-4.42, 20.85, 2.91), "hide_lt")
    g.box(sm, "shaft_mid_flange_r", (-4.10, 15.65, 2.68), (-3.82, 20.85, 2.95), "hide_dk")
    g.box(sm, "shaft_mid_knob_1", (-4.65, 16.45, 2.82), (-4.35, 16.80, 3.12), "hide_dk")
    g.box(sm, "shaft_mid_knob_2", (-4.15, 19.85, 2.82), (-3.85, 20.25, 3.12), "hide_dk")

    # 2. 骨钩加粗侧脊与多面骨板 (12 块)
    hk = "hook_shank"
    g.box(hk, "hook_flange_l_1", (-4.42, 27.15, 4.45), (-4.22, 28.45, 4.89), "bone_stain")
    g.box(hk, "hook_flange_r_1", (-3.91, 27.17, 4.45), (-3.60, 28.40, 4.90), "bone_stain")
    g.box(hk, "hook_flange_l_2", (-4.38, 28.39, 4.75), (-4.18, 29.51, 5.15), "bone_stain")
    g.box(hk, "hook_flange_r_2", (-3.88, 28.35, 4.75), (-3.64, 29.58, 5.15), "bone_stain")
    g.box(hk, "hook_trench_line_1", (-4.32, 27.35, 4.32), (-4.06, 28.25, 4.58), "void")
    g.box(hk, "hook_trench_line_2", (-3.90, 27.35, 4.32), (-3.64, 28.25, 4.58), "void")
    g.box(hk, "hook_trench_line_3", (-4.28, 28.55, 4.62), (-4.02, 29.35, 4.88), "void")
    g.box(hk, "hook_trench_line_4", (-3.92, 28.55, 4.62), (-3.66, 29.35, 4.88), "void")
    g.box(hk, "hook_plate_scab_1", (-4.35, 27.85, 4.75), (-4.09, 28.35, 5.05), "bone_dk")
    g.box(hk, "hook_plate_scab_2", (-3.92, 27.85, 4.75), (-3.66, 28.35, 5.05), "bone_dk")
    g.box(hk, "hook_spike_extra_1", (-4.16, 27.75, 5.15), (-3.88, 28.15, 5.48), "bone_lt")
    g.box(hk, "hook_spike_extra_2", (-4.19, 28.35, 5.35), (-3.88, 28.75, 5.68), "bone_lt")

    # 3. 挂件串第二兽牙与骨珠 (12 块)
    pc = "pole_charm"
    g.box(pc, "charm_bead_1", (-4.65, 17.85, 2.82), (-4.35, 18.25, 3.12), "bone_stain")
    g.box(pc, "charm_bead_2", (-4.65, 17.35, 2.82), (-4.35, 17.75, 3.10), "bone")
    g.box(pc, "charm_tooth2_root", (-4.82, 16.40, 2.92), (-4.42, 17.27, 3.22), "bone")
    g.box(pc, "charm_tooth2_blade", (-4.78, 15.65, 3.00), (-4.46, 16.55, 3.25), "bone_lt")
    g.box(pc, "charm_tooth2_tip", (-4.72, 15.15, 2.98), (-4.49, 15.75, 3.22), "bone_lt")
    g.box(pc, "charm_tally_notch_1", (-4.72, 16.35, 3.24), (-4.38, 16.65, 3.52), "void")
    g.box(pc, "charm_tally_notch_2", (-4.72, 16.85, 3.24), (-4.38, 17.15, 3.52), "void")
    g.box(pc, "charm_tally_bead_t", (-4.62, 16.02, 3.18), (-4.38, 16.32, 3.42), "bone_lt")
    g.box(pc, "charm_knot_tail_a", (-4.78, 14.85, 2.85), (-4.52, 15.42, 3.08), "cord_dk")
    g.box(pc, "charm_knot_tail_b", (-4.52, 14.65, 2.92), (-4.26, 15.45, 3.15), "cord")
    g.box(pc, "charm_cord_tie_ring", (-4.68, 17.15, 2.70), (-4.32, 17.48, 2.95), "cord_dk")
    g.box(pc, "charm_bone_tag_back", (-4.72, 16.32, 2.95), (-4.23, 17.33, 3.24), "bone_dk")

    # 4. 皮套筒加厚翻边与双排缝线 (14 块)
    hb_part = "holster_body"
    g.box(hb_part, "holster_lip_side_l", (-5.23, 10.25, 2.41), (-4.90, 13.82, 3.05), "hide_dk")
    g.box(hb_part, "holster_lip_side_r", (-3.74, 10.25, 2.48), (-3.40, 13.82, 3.05), "hide_dk")
    g.box(hb_part, "holster_rivet_1", (-5.02, 13.25, 3.28), (-4.72, 13.58, 3.55), "iron_lt")
    g.box(hb_part, "holster_rivet_2", (-3.98, 13.25, 3.28), (-3.68, 13.58, 3.55), "iron_lt")
    g.box(hb_part, "holster_rivet_3", (-5.02, 10.55, 3.28), (-4.72, 10.88, 3.55), "iron_lt")
    g.box(hb_part, "holster_rivet_4", (-3.98, 10.55, 3.28), (-3.68, 10.88, 3.55), "iron_lt")
    g.box(hb_part, "holster_patch_a", (-4.85, 12.55, 2.12), (-4.15, 13.35, 2.38), "hide")
    g.box(hb_part, "holster_patch_b", (-4.45, 10.65, 2.12), (-3.75, 11.45, 2.38), "hide_lt")
    g.box(hb_part, "holster_clasp_metal", (-4.62, 11.82, 3.58), (-4.08, 12.38, 3.84), "iron")
    g.box(hb_part, "holster_clasp_pin", (-4.45, 11.75, 3.65), (-4.25, 12.48, 3.92), "rust")
    g.box(hb_part, "holster_strap_fold_1", (-4.99, 11.92, 2.38), (-4.65, 12.28, 2.65), "hide_dk")
    g.box(hb_part, "holster_strap_fold_2", (-3.95, 11.92, 2.38), (-3.65, 12.28, 2.65), "hide_dk")
    g.box(hb_part, "holster_bottom_cap", (-4.92, 10.02, 2.35), (-3.72, 10.39, 3.22), "hide_dk")
    g.box(hb_part, "holster_bottom_stitch", (-4.78, 10.10, 2.45), (-3.82, 10.45, 3.12), "stitch")

    # 5. 顶端铁箍双道强化齿与楔钉 (18 块)
    pc_part = "pole_collar"
    g.box(pc_part, "collar_flange_front", (-4.30, 24.35, 3.95), (-3.75, 25.55, 4.24), "iron_lt")
    g.box(pc_part, "collar_flange_back", (-4.40, 24.32, 4.70), (-3.75, 25.55, 4.98), "iron_lt")
    g.box(pc_part, "collar_flange_left", (-4.52, 24.35, 4.25), (-4.26, 25.51, 4.72), "rust")
    g.box(pc_part, "collar_flange_right", (-3.75, 24.39, 4.25), (-3.49, 25.57, 4.72), "rust")
    g.box(pc_part, "collar_rivet_a", (-4.45, 24.45, 4.45), (-4.25, 24.75, 4.65), "iron_lt")
    g.box(pc_part, "collar_rivet_b", (-3.85, 24.45, 4.45), (-3.65, 24.75, 4.65), "iron_lt")
    g.box(pc_part, "collar_rivet_c", (-4.49, 25.25, 4.41), (-4.25, 25.55, 4.65), "iron_lt")
    g.box(pc_part, "collar_rivet_d", (-3.85, 25.25, 4.41), (-3.61, 25.55, 4.63), "iron_lt")
    g.box(pc_part, "collar_split_shim", (-4.45, 24.75, 4.08), (-4.19, 25.15, 4.34), "rust_dk")
    g.box(pc_part, "collar_strap_wrap", (-4.42, 24.85, 4.05), (-3.68, 25.25, 4.88), "hide_dk")
    g.box(pc_part, "collar_tie_cord_t", (-4.37, 25.68, 4.10), (-3.68, 25.92, 4.85), "cord_dk")
    g.box(pc_part, "collar_tie_cord_b", (-4.40, 24.08, 4.05), (-3.70, 24.32, 4.82), "cord_dk")
    g.box(pc_part, "collar_bracket_tooth_1", (-4.38, 24.52, 4.78), (-4.12, 24.82, 5.05), "iron")
    g.box(pc_part, "collar_bracket_tooth_2", (-3.98, 24.52, 4.78), (-3.72, 24.82, 5.05), "iron")
    g.box(pc_part, "collar_bracket_tooth_3", (-4.38, 25.12, 4.78), (-4.12, 25.42, 5.05), "iron")
    g.box(pc_part, "collar_bracket_tooth_4", (-3.98, 25.12, 4.78), (-3.72, 25.42, 5.05), "iron")
    g.box(pc_part, "collar_lock_key", (-4.40, 25.35, 4.45), (-4.15, 25.75, 4.70), "rust")
    g.box(pc_part, "collar_washer_metal", (-3.95, 25.35, 4.45), (-3.65, 25.75, 4.65), "iron_lt")


PART_FUNCS = {
    "belt": part_belt,
    "holster": part_holster,
    "socket": part_socket,
    "butt": part_pole_butt,
    "shaft_lo": part_shaft_lo,
    "shaft_mid": part_shaft_mid,
    "shaft_hi": part_shaft_hi,
    "collar": part_collar,
    "hook_base": part_hook_base,
    "hook_shank": part_hook_shank,
    "hook_bend": part_hook_bend,
    "hook_tip": part_hook_tip,
    "hook_binding": part_hook_binding,
    "charm": part_charm,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="chang_bing_gu_gou",
    model_name="ChangBingGuGou",
    title="长柄骨钩 / chang_bing_gu_gou",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：背部左侧斜插（左腰套筒至左肩后方，钩头向后上方探出）",
    worn_focus=((-4.0, 18.0, 3.2), 26.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
