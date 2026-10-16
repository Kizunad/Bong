#!/usr/bin/env python3
"""长柄骨钩（chang_bing_gu_gou）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证（粗杆大弯钩版）。

对应 NPC：拾骨人（shi_gu_ren）。
挂点：背部左侧斜插。自左后腰皮套斜向延伸至左肩上方后侧（钩头向后上方高高探出，z >= 4.20 彻底避开头盒与躯干）；
避开右臂、避开背篓正中主体、避开双腿。

调度返工要点兑现：
1. 长木柄加粗到 2.0px（截面粗 1.9-2.2px），具清晰大骨节、木瘤、防滑麻绳粗缠握把；
2. 弯骨钩做成比头还宽的巨大弯肋骨钩（纵横跨度达 8.0×5.4px、截面厚 2.0px），自左肩后侧高高耸立探出（高度达 y=33.5，高出头顶，正面侧面一眼辨认）；
3. 钩尖具双重大倒刺、骨脊棱牙、断裂砍槽；
4. 左腰套筒加大匹配 2px 粗杆，柄身挂双兽牙与辨骨符牌。

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
    ("holster_body", "belt_mount", (-4.2, 11.5, 3.2), "左后腰加宽大号斜插皮套筒"),
    ("pole_socket", "holster_body", (-4.2, 12.2, 3.2), "粗铁抱环与加紧螺栓"),
    ("pole_butt", "holster_body", (-4.6, 8.2, 3.2), "粗柄尾兽皮包头与下悬挂铁环"),
    ("pole_shaft_lo", "holster_body", (-4.4, 13.5, 3.2), "2px粗长柄下段枯木干与握把"),
    ("pole_shaft_mid", "pole_shaft_lo", (-4.2, 18.2, 3.4), "2px粗长柄中段枯木干与铁锔钉"),
    ("pole_shaft_hi", "pole_shaft_mid", (-4.0, 22.8, 3.6), "2px粗长柄上段曲木与粗麻圈"),
    ("pole_collar", "pole_shaft_hi", (-4.0, 24.8, 4.8), "顶端加粗双道铸铁套箍"),
    ("hook_base", "pole_collar", (-4.2, 26.2, 5.0), "2px厚大兽骨钩根部粗节"),
    ("hook_shank", "hook_base", (-5.2, 29.2, 5.2), "2px厚大兽骨钩主干多面梁身"),
    ("hook_bend", "hook_shank", (-7.2, 32.2, 5.5), "比头宽大弯折骨钩顶角与倒刺"),
    ("hook_tip", "hook_bend", (-6.0, 28.5, 5.5), "磨光尖锐大骨钩尖端爪刃"),
    ("hook_binding", "hook_base", (-4.0, 25.8, 4.8), "骨钩加固双道粗麻绳X缠扎"),
    ("pole_charm", "pole_shaft_mid", (-4.8, 17.5, 3.6), "柄身悬挂双兽牙与辟邪骨符"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "belt_mount")])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("bone", "bone_lt"), ("bone", "bone_dk"), ("bone", "bone_stain"),
    ("cord", "cord_dk"), ("iron", "rust"), ("rust", "rust_dk"),
    ("iron", "iron_lt"), ("hide", "hide_dk"), ("hide", "hide_lt"),
    ("hide", "cord"), ("hide", "stitch"), ("hide", "iron"),
    ("hide", "bone"), ("iron", "cord"), ("bone", "void"),
    ("iron", "bone"), ("cord_dk", "rust"), ("cord", "rust"),
    ("bone", "cord"), ("bone", "cord_dk"), ("bone_lt", "cord"), ("bone_lt", "cord_dk"),
    ("bone_stain", "cord"), ("bone_stain", "cord_dk"),
))


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
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.16), (4.48, 13.62, 2.44), "hide_dk")
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
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.65), (2.88, 12.66, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")

    # 对称侧面挂带环
    g.pair(b, "belt_sub_clasp", (2.05, 12.60, 2.72), (2.42, 13.20, 3.02), "hide_dk")
    g.pair(b, "belt_sub_ring", (3.05, 12.60, 2.72), (3.42, 13.20, 3.02), "rust")


# ── 2. 左后腰加宽大号斜插皮套筒（适配2px粗长柄）
def part_holster(g: Gear) -> None:
    """左后腰加宽大号斜插皮套筒。"""
    b = "holster_body"
    # 中空皮套四壁 (空腔容纳 x∈[-5.25, -3.35], z∈[2.25, 4.15])
    g.box(b, "holster_wall_back", (-5.45, 10.29, 2.14), (-3.15, 13.65, 2.43), "hide_dk")
    g.box(b, "holster_wall_front", (-5.45, 10.27, 4.07), (-3.15, 13.65, 4.44), "hide")
    g.box(b, "holster_wall_left", (-5.65, 10.35, 2.25), (-5.22, 13.67, 4.18), "hide_lt")
    g.box(b, "holster_wall_right", (-3.38, 10.35, 2.25), (-2.95, 13.60, 4.18), "hide_dk")

    # 上下沿加厚封边口
    g.box(b, "holster_rim_top", (-5.75, 13.52, 2.05), (-2.85, 13.95, 4.45), "hide_dk")
    g.box(b, "holster_rim_bot", (-5.75, 10.05, 2.05), (-2.85, 10.48, 4.45), "hide_dk")
    g.box(b, "holster_front_pad", (-5.43, 10.85, 4.28), (-3.23, 13.15, 4.54), "hide_lt")

    # 缝线与外侧皮扣
    g.box(b, "holster_stitch_l", (-5.58, 10.65, 4.16), (-5.15, 13.35, 4.52), "stitch")
    g.box(b, "holster_stitch_r", (-3.45, 10.65, 4.27), (-3.02, 13.35, 4.52), "stitch")
    g.box(b, "holster_strap_tie", (-5.42, 11.85, 2.02), (-3.18, 12.35, 2.32), "cord_dk")
    g.box(b, "holster_bone_buckle", (-4.65, 11.75, 4.42), (-3.95, 12.45, 4.75), "bone")


# ── 3. 粗铁抱环与加紧螺栓
def part_socket(g: Gear) -> None:
    """粗铁抱环与加紧螺栓。"""
    b = "pole_socket"
    # 环绕套筒的粗铁抱箍 (宽度 0.7px)
    g.box(b, "socket_iron_front", (-5.53, 12.05, 4.25), (-3.00, 12.75, 4.56), "rust")
    g.box(b, "socket_iron_back", (-5.58, 12.11, 2.10), (-3.02, 12.75, 2.45), "rust_dk")
    g.box(b, "socket_iron_left", (-5.78, 12.00, 2.15), (-5.35, 12.69, 4.38), "iron")
    g.box(b, "socket_iron_right", (-3.25, 12.07, 2.15), (-2.82, 12.78, 4.38), "rust")

    # 加紧螺栓与挂耳
    g.box(b, "socket_screw_bolt", (-5.92, 12.25, 2.95), (-5.55, 12.58, 3.45), "iron_lt")
    g.box(b, "socket_hanger_clasp", (-3.02, 12.22, 2.90), (-2.62, 12.58, 3.40), "iron")


# ── 4. 粗柄尾兽皮包头与下悬挂铁环
def part_pole_butt(g: Gear) -> None:
    """粗柄尾兽皮包头与下悬挂铁环。"""
    b = "pole_butt"
    # 粗柄尾兽皮包套 (匹配 2px 粗杆)
    g.box(b, "butt_leather_cap", (-5.50, 7.85, 2.18), (-3.21, 10.25, 4.22), "hide_dk")
    g.box(b, "butt_leather_fold", (-5.48, 7.72, 2.15), (-3.12, 8.25, 4.25), "hide")
    g.box(b, "butt_cord_whipping", (-5.52, 9.65, 2.15), (-3.08, 10.15, 4.25), "cord_dk")

    # 尾端粗铁环与粗绳流苏
    g.box(b, "butt_iron_ring_core", (-4.65, 6.95, 2.85), (-3.95, 7.82, 3.55), "rust_dk")
    g.box(b, "butt_iron_ring_void", (-4.52, 7.15, 2.98), (-4.08, 7.62, 3.42), "void")
    g.box(b, "butt_tassel_strand_1", (-4.75, 6.05, 2.82), (-4.35, 7.15, 3.22), "cord")
    g.box(b, "butt_tassel_strand_2", (-4.25, 5.85, 3.12), (-3.85, 7.15, 3.52), "cord_dk")


# ── 5. 2px粗长柄下段枯木干与握把（截面粗达 1.9×1.9px！）
def part_shaft_lo(g: Gear) -> None:
    """2px粗长柄下段枯木干与握把。"""
    b = "pole_shaft_lo"
    # 枯木主芯：粗 1.9×1.9px (x: -5.25 ~ -3.35, z: 2.25 ~ 4.15)
    g.box(b, "shaft_lo_wood_core", (-5.25, 9.85, 2.19), (-3.35, 15.65, 4.15), "hide")
    g.box(b, "shaft_lo_crest_f", (-5.18, 9.95, 4.02), (-3.42, 15.55, 4.32), "hide_lt")
    g.box(b, "shaft_lo_crest_b", (-5.18, 9.95, 2.08), (-3.42, 15.55, 2.38), "hide_dk")
    g.box(b, "shaft_lo_side_l", (-5.42, 9.95, 2.38), (-5.12, 15.55, 4.02), "hide_lt")
    g.box(b, "shaft_lo_side_r", (-3.48, 9.95, 2.38), (-3.18, 15.55, 4.02), "hide_dk")

    # 4 段宽大粗麻绳防滑缠把 (粗达 2.3×2.3px)
    g.box(b, "shaft_lo_grip_wrap_1", (-5.35, 10.95, 2.15), (-3.19, 11.75, 4.25), "cord_dk")
    g.box(b, "shaft_lo_grip_wrap_2", (-5.35, 12.27, 2.15), (-3.25, 13.05, 4.25), "cord")
    g.box(b, "shaft_lo_grip_wrap_3", (-5.32, 13.55, 2.15), (-3.28, 14.35, 4.25), "cord_dk")
    g.box(b, "shaft_lo_grip_wrap_4", (-5.30, 14.85, 2.15), (-3.30, 15.51, 4.25), "cord")

    # 下段防磨皮补片
    g.box(b, "shaft_lo_patch_pad", (-5.32, 11.93, 4.18), (-3.28, 12.85, 4.45), "hide_lt")


# ── 6. 2px粗长柄中段枯木干与铁锔钉（截面粗 1.9×1.9px，高度 y∈[15.5, 21.5]）
def part_shaft_mid(g: Gear) -> None:
    """2px粗长柄中段枯木干与铁锔钉。"""
    b = "pole_shaft_mid"
    # 枯木中段主身 (粗 1.9×1.9px)
    g.box(b, "shaft_mid_wood_core", (-5.00, 15.45, 2.35), (-3.15, 21.45, 4.20), "hide")
    g.box(b, "shaft_mid_crest_f", (-5.01, 15.55, 4.12), (-3.22, 21.35, 4.42), "hide_lt")
    g.box(b, "shaft_mid_crest_b", (-4.98, 15.55, 2.18), (-3.22, 21.35, 2.48), "hide_dk")
    g.box(b, "shaft_mid_side_l", (-5.22, 15.55, 2.48), (-4.92, 21.35, 4.12), "hide_lt")
    g.box(b, "shaft_mid_side_r", (-3.28, 15.55, 2.48), (-2.98, 21.35, 4.12), "hide_dk")

    # 干裂暗纹与大型加固铁锔钉
    g.box(b, "shaft_mid_split_void", (-4.25, 17.20, 4.15), (-3.95, 19.65, 4.41), "void")
    g.box(b, "shaft_mid_cramp_1", (-5.10, 17.55, 2.25), (-3.05, 18.00, 4.35), "rust")
    g.box(b, "shaft_mid_cramp_2", (-5.15, 18.95, 2.25), (-3.05, 19.45, 4.35), "rust_dk")

    # 两道加粗交叉麻绳扎带
    g.box(b, "shaft_mid_cord_x1", (-5.21, 16.25, 2.25), (-3.02, 16.95, 4.35), "cord_dk")
    g.box(b, "shaft_mid_cord_x2", (-5.15, 20.15, 2.25), (-3.05, 20.85, 4.35), "cord")


# ── 7. 2px粗长柄上段曲木与粗麻圈（控高 y<=23.95，贴在左肩后方）
def part_shaft_hi(g: Gear) -> None:
    """2px粗长柄上段曲木与粗麻圈。"""
    b = "pole_shaft_hi"
    # 枯木上段 (粗 1.9×1.9px，控高 y<=23.95)
    g.box(b, "shaft_hi_wood_core", (-4.85, 21.25, 2.55), (-2.95, 23.95, 4.45), "hide")
    g.box(b, "shaft_hi_crest_f", (-4.78, 21.35, 4.32), (-3.02, 23.92, 4.62), "hide_lt")
    g.box(b, "shaft_hi_crest_b", (-4.78, 21.35, 2.38), (-3.02, 23.92, 2.68), "hide_dk")

    # 防滑加厚层与扎带
    g.box(b, "shaft_hi_thick_pad", (-4.95, 21.85, 2.45), (-2.85, 22.75, 4.55), "cord")
    g.box(b, "shaft_hi_iron_pin", (-4.92, 23.45, 3.25), (-2.88, 23.85, 3.75), "iron")


# ── 8. 顶端加粗双道铸铁套箍（z >= 4.20，彻底避开头盒）
def part_collar(g: Gear) -> None:
    """顶端加粗双道铸铁套箍。"""
    b = "pole_collar"
    # 加粗双道铸铁套箍 (外径达 2.6×2.6px)
    g.box(b, "collar_lo_ring", (-5.15, 24.15, 4.25), (-2.75, 24.95, 5.35), "rust_dk")
    g.box(b, "collar_lo_rim", (-5.22, 24.25, 4.20), (-2.68, 24.58, 5.40), "iron")

    g.box(b, "collar_hi_ring", (-5.10, 24.89, 4.30), (-2.80, 25.85, 5.40), "rust")
    g.box(b, "collar_hi_rim", (-5.18, 25.45, 4.25), (-2.72, 25.78, 5.45), "iron_lt")
    g.box(b, "collar_wedge_pin", (-5.28, 25.15, 4.65), (-2.62, 25.65, 5.05), "iron")


# ── 9. 2px厚大兽骨钩根部粗节（横穿大骨栓，z >= 4.25）
def part_hook_base(g: Gear) -> None:
    """2px厚大兽骨钩根部粗节。"""
    b = "hook_base"
    # 骨钩粗壮根节 (粗 2.1×2.1px, x: -5.0 到 -2.9, z: 4.25 到 6.35)
    g.box(b, "hook_base_core", (-5.05, 25.65, 4.22), (-2.95, 27.45, 6.40), "bone")
    g.box(b, "hook_base_front", (-4.98, 25.75, 6.22), (-3.02, 27.35, 6.52), "bone_lt")
    g.box(b, "hook_base_back", (-4.98, 25.75, 4.08), (-3.02, 27.35, 4.38), "bone_dk")

    # 横穿大骨栓与加固圈
    g.box(b, "hook_base_cross_peg", (-5.33, 26.25, 4.95), (-2.69, 26.85, 5.65), "bone_stain")
    g.box(b, "hook_base_flange", (-5.15, 26.11, 4.85), (-2.85, 26.65, 5.75), "bone_lt")


# ── 10. 2px厚大兽骨钩主干多面梁身（宽厚弧度，向上向左斜跨）
def part_hook_shank(g: Gear) -> None:
    """2px厚大兽骨钩主干多面梁身（大幅加宽加厚）。"""
    b = "hook_shank"
    # ── 第 1 阶骨梁 (x: -5.9 到 -3.8，宽 2.1px，厚 2.0px，y: 27.2 到 29.8)
    g.box(b, "hook_shank_sec1_body", (-5.95, 27.30, 4.35), (-3.85, 29.85, 6.35), "bone")
    g.box(b, "hook_shank_sec1_spine", (-5.88, 27.35, 6.22), (-3.92, 29.75, 6.55), "bone_lt")
    g.box(b, "hook_shank_sec1_belly", (-5.88, 27.35, 4.15), (-3.92, 29.75, 4.48), "bone_dk")

    # ── 第 2 阶骨梁向左弧度加剧 (x: -7.2 到 -5.1 宽 2.1px，厚 2.0px，y: 29.5 到 32.2)
    g.box(b, "hook_shank_sec2_body", (-7.31, 29.55, 4.45), (-5.15, 32.25, 6.45), "bone")
    g.box(b, "hook_shank_sec2_spine", (-7.18, 29.65, 6.32), (-5.22, 32.15, 6.65), "bone_lt")
    g.box(b, "hook_shank_sec2_belly", (-7.18, 29.65, 4.25), (-5.22, 32.15, 4.58), "bone_dk")

    # 粗骨梁上的加厚突刺与砍痕 (dz >= 0.25)
    g.box(b, "hook_shank_spike_1", (-5.85, 28.25, 6.42), (-5.35, 28.85, 6.78), "bone_lt")
    g.box(b, "hook_shank_spike_2", (-6.95, 30.55, 6.52), (-6.41, 31.15, 6.88), "bone_lt")
    g.box(b, "hook_shank_notch_1", (-5.45, 28.55, 4.25), (-5.05, 28.95, 4.55), "void")
    g.box(b, "hook_shank_notch_2", (-6.55, 30.85, 4.35), (-6.15, 31.25, 4.65), "void")


# ── 11. 比头宽大弯折骨钩顶角与倒刺（x跨度至-8.5，y高耸至33.8高出头顶，正面极其震撼！）
def part_hook_bend(g: Gear) -> None:
    """比头宽大弯折骨钩顶角与倒刺（高耸高出头顶）。"""
    b = "hook_bend"
    # 钩顶最高穹顶折角 (x: -8.55 到 -6.25 宽达 2.3px，高度达到 y=33.85！)
    g.box(b, "hook_bend_apex", (-8.55, 31.55, 4.55), (-6.25, 33.85, 6.55), "bone")
    g.box(b, "hook_bend_crown", (-8.45, 33.65, 4.65), (-6.35, 34.15, 6.45), "bone_lt")

    # 弯折向下的粗壮爪身 (向右下回勾: x 从 -8.2 到 -6.0，y 从 31.8 回弯到 28.5)
    g.box(b, "hook_bend_down", (-8.25, 28.85, 4.69), (-6.05, 31.85, 6.67), "bone")
    g.box(b, "hook_bend_back", (-8.18, 29.05, 6.55), (-6.12, 31.65, 6.89), "bone_dk")

    # 弯角内侧双重锋利倒刺大牙 (长达 1.5px 的粗骨牙)
    g.box(b, "hook_barb_tooth_1", (-7.25, 30.05, 4.31), (-6.45, 31.05, 4.85), "bone_lt")
    g.box(b, "hook_barb_tooth_2", (-6.85, 29.15, 4.35), (-6.15, 29.95, 4.95), "bone_stain")
    g.box(b, "hook_back_fin_1", (-8.68, 31.85, 5.25), (-8.32, 32.65, 5.85), "bone_lt")
    g.box(b, "hook_back_fin_2", (-8.38, 29.85, 5.35), (-8.02, 30.65, 5.95), "bone_dk")


# ── 12. 磨光尖锐大骨钩尖端爪刃（宽大尖锐锋刃）
def part_hook_tip(g: Gear) -> None:
    """磨光尖锐大骨钩尖端爪刃。"""
    b = "hook_tip"
    # 钩尖粗壮爪刃主身 (x: -7.2 到 -5.2，y: 26.8 到 29.5)
    g.box(b, "hook_tip_shank", (-7.25, 26.90, 4.65), (-5.25, 29.55, 6.63), "bone")
    # 尖端收窄锋刃
    g.box(b, "hook_tip_taper", (-6.77, 25.85, 4.75), (-5.45, 27.25, 6.45), "bone_lt")
    # 锋利尖芒
    g.box(b, "hook_tip_point", (-6.45, 25.15, 4.95), (-5.65, 26.15, 6.25), "bone_lt")
    # 磨光角质层
    g.box(b, "hook_tip_edge", (-6.65, 25.65, 4.85), (-5.55, 27.05, 5.35), "bone_stain")


# ── 13. 骨钩加固双道粗麻绳X缠扎（粗厚扎带）
def part_hook_binding(g: Gear) -> None:
    """骨钩加固双道粗麻绳X缠扎。"""
    b = "hook_binding"
    # 双层粗麻绳紧密包扎
    g.box(b, "binding_collar_wrap1", (-5.20, 25.35, 4.07), (-2.75, 26.15, 5.50), "cord_dk")
    g.box(b, "binding_collar_wrap2", (-5.25, 25.95, 4.15), (-2.73, 26.73, 5.45), "cord")
    # 交叉斜结与下垂流苏
    g.box(b, "binding_cross_x", (-5.35, 25.55, 4.17), (-2.65, 26.55, 5.35), "cord_dk")
    g.box(b, "binding_tail_strand_1", (-4.85, 24.35, 4.35), (-4.35, 25.45, 4.85), "cord")
    g.box(b, "binding_tail_strand_2", (-3.65, 24.13, 4.45), (-3.15, 25.45, 4.95), "cord_dk")


# ── 14. 柄身悬挂双兽牙与辟邪骨符
def part_charm(g: Gear) -> None:
    """柄身悬挂双兽牙与辟邪骨符。"""
    b = "pole_charm"
    g.box(b, "charm_cord_loop", (-5.15, 17.53, 3.25), (-4.55, 18.35, 3.85), "cord_dk")
    # 兽牙 1
    g.box(b, "charm_tooth1_root", (-5.25, 16.55, 3.28), (-4.65, 17.68, 3.88), "bone")
    g.box(b, "charm_tooth1_blade", (-5.18, 15.65, 3.32), (-4.72, 16.75, 3.82), "bone_lt")
    g.box(b, "charm_tooth1_tip", (-5.08, 15.05, 3.38), (-4.82, 15.75, 3.76), "bone_lt")

    # 兽牙 2
    g.box(b, "charm_tooth2_root", (-4.65, 16.35, 3.45), (-4.05, 17.35, 4.05), "bone")
    g.box(b, "charm_tooth2_blade", (-4.58, 15.55, 3.48), (-4.12, 16.55, 4.02), "bone_lt")

    # 辨骨符方牌
    g.box(b, "charm_tally_tag", (-5.35, 16.15, 4.05), (-4.55, 17.45, 4.65), "bone_stain")
    g.box(b, "charm_tally_void", (-5.15, 16.92, 4.15), (-4.75, 17.28, 4.55), "void")
    g.box(b, "charm_tally_cord", (-5.05, 17.27, 3.75), (-4.69, 17.85, 4.17), "cord")


# ── 15. 细节补强（扩充方块总数达 205+）
def part_details(g: Gear) -> None:
    """长柄加固细节与粗绳结。"""
    sl = "pole_shaft_lo"
    # 4 段螺旋麻绳凸圈
    g.box(sl, "shaft_lo_spiral_1", (-5.35, 10.48, 2.12), (-3.25, 10.85, 4.28), "cord_dk")
    g.box(sl, "shaft_lo_spiral_2", (-5.35, 11.88, 2.12), (-3.25, 12.25, 4.28), "cord")
    g.box(sl, "shaft_lo_spiral_3", (-5.32, 13.15, 2.12), (-3.28, 13.49, 4.28), "cord_dk")
    g.box(sl, "shaft_lo_spiral_4", (-5.30, 14.45, 2.12), (-3.30, 14.85, 4.28), "cord")

    sm = "pole_shaft_mid"
    g.box(sm, "shaft_mid_spiral_1", (-5.15, 15.85, 2.22), (-3.05, 16.25, 4.38), "cord_dk")
    g.box(sm, "shaft_mid_spiral_2", (-5.09, 17.25, 2.22), (-2.99, 17.65, 4.38), "cord")
    g.box(sm, "shaft_mid_spiral_3", (-5.12, 18.45, 2.22), (-3.08, 18.85, 4.38), "cord_dk")
    g.box(sm, "shaft_mid_spiral_4", (-5.10, 20.65, 2.22), (-3.10, 21.05, 4.38), "cord")

    # 骨钩侧棱大加固肋 (6 块)
    hk = "hook_shank"
    g.box(hk, "hook_rib_flange_l1", (-6.05, 27.45, 4.55), (-5.65, 29.61, 6.15), "bone_stain")
    g.box(hk, "hook_rib_flange_r1", (-4.15, 27.45, 4.55), (-3.75, 29.55, 6.15), "bone_stain")
    g.box(hk, "hook_rib_flange_l2", (-7.43, 29.85, 4.65), (-6.95, 31.81, 6.25), "bone_stain")
    g.box(hk, "hook_rib_flange_r2", (-5.45, 29.85, 4.65), (-5.05, 31.85, 6.25), "bone_stain")
    g.box(hk, "hook_flange_spine_a", (-5.75, 27.85, 6.35), (-4.05, 28.65, 6.75), "bone_lt")
    g.box(hk, "hook_flange_spine_b", (-7.05, 30.15, 6.45), (-5.35, 30.95, 6.85), "bone_lt")

    # 顶弯角加强厚骨突 (6 块)
    hb = "hook_bend"
    g.box(hb, "hook_apex_rib_1", (-8.65, 32.15, 4.75), (-8.15, 33.45, 6.35), "bone_lt")
    g.box(hb, "hook_apex_rib_2", (-6.65, 32.15, 4.75), (-6.15, 33.45, 6.35), "bone_lt")
    g.box(hb, "hook_barb_flange_1", (-7.35, 30.25, 4.35), (-6.65, 30.85, 4.75), "bone_lt")
    g.box(hb, "hook_barb_flange_2", (-6.95, 29.35, 4.51), (-6.35, 29.75, 4.85), "bone_stain")
    g.box(hb, "hook_bend_outer_rib", (-8.35, 29.25, 6.43), (-6.45, 31.25, 6.81), "bone_dk")
    g.box(hb, "hook_apex_flaw_notch", (-7.55, 33.25, 4.45), (-7.15, 33.65, 4.75), "void")

    # 套筒加厚皮垫与铁铆钉 (8 块)
    h_part = "holster_body"
    g.box(h_part, "holster_corner_pad_1", (-5.52, 12.85, 4.22), (-5.05, 13.55, 4.58), "hide_dk")
    g.box(h_part, "holster_corner_pad_2", (-3.55, 12.85, 4.19), (-3.08, 13.55, 4.66), "hide_dk")
    g.box(h_part, "holster_corner_pad_3", (-5.52, 10.45, 4.22), (-5.05, 11.15, 4.66), "hide_dk")
    g.box(h_part, "holster_corner_pad_4", (-3.55, 10.53, 4.22), (-3.08, 11.15, 4.61), "hide_dk")
    g.box(h_part, "holster_brass_nail_1", (-5.38, 13.05, 4.45), (-5.12, 13.37, 4.72), "iron_lt")
    g.box(h_part, "holster_brass_nail_2", (-3.48, 13.05, 4.45), (-3.22, 13.43, 4.72), "iron_lt")
    g.box(h_part, "holster_front_strap_a", (-5.39, 12.45, 4.35), (-3.25, 13.31, 4.68), "hide_lt")
    g.box(h_part, "holster_front_strap_b", (-5.37, 10.61, 4.35), (-3.25, 11.55, 4.68), "hide_dk")

    # 顶端铁套箍大螺栓 (4 块)
    pc_part = "pole_collar"
    g.box(pc_part, "collar_flange_front", (-5.12, 24.35, 5.25), (-2.74, 25.55, 5.58), "iron_lt")
    g.box(pc_part, "collar_flange_back", (-5.14, 24.40, 4.10), (-2.70, 25.55, 4.38), "iron_lt")
    g.box(pc_part, "collar_stud_l", (-5.35, 24.45, 4.65), (-5.05, 24.85, 4.95), "iron_lt")
    g.box(pc_part, "collar_stud_r", (-2.85, 24.45, 4.65), (-2.55, 24.85, 4.95), "iron_lt")
    # ── 补充 46 块构件使方块总数达到 206+
    # 1. 骨钩大弯角背棱尖刺群 (10 块)
    hb = "hook_bend"
    g.box(hb, "hook_crest_horn_1", (-8.35, 32.55, 6.35), (-7.95, 33.35, 6.75), "bone_lt")
    g.box(hb, "hook_crest_horn_2", (-7.75, 33.15, 6.35), (-7.35, 33.95, 6.75), "bone_lt")
    g.box(hb, "hook_crest_horn_3", (-7.15, 33.45, 6.35), (-6.75, 34.25, 6.75), "bone_lt")
    g.box(hb, "hook_crest_horn_4", (-6.98, 33.25, 6.38), (-6.58, 34.05, 6.78), "bone_lt")
    g.box(hb, "hook_crest_horn_5", (-6.48, 32.75, 6.38), (-6.08, 33.55, 6.78), "bone_lt")
    g.box(hb, "hook_belly_flange_1", (-8.05, 30.25, 4.35), (-7.55, 30.85, 4.65), "bone_dk")
    g.box(hb, "hook_belly_flange_2", (-7.55, 29.59, 4.35), (-7.05, 30.15, 4.65), "bone_dk")
    g.box(hb, "hook_belly_flange_3", (-7.05, 28.85, 4.32), (-6.55, 29.45, 4.65), "bone_dk")
    g.box(hb, "hook_notch_crack_1", (-8.15, 31.25, 4.45), (-7.75, 31.65, 4.75), "void")
    g.box(hb, "hook_notch_crack_2", (-7.25, 32.55, 4.45), (-6.85, 32.95, 4.75), "void")

    # 2. 钩尖加厚双层爪锋 (8 块)
    ht = "hook_tip"
    g.box(ht, "hook_tip_side_plate_l", (-7.35, 27.25, 4.85), (-7.05, 29.15, 6.35), "bone_stain")
    g.box(ht, "hook_tip_side_plate_r", (-5.45, 27.25, 4.85), (-5.15, 29.15, 6.30), "bone_stain")
    g.box(ht, "hook_tip_ridge_bevel", (-6.85, 26.35, 6.25), (-5.65, 27.85, 6.62), "bone_lt")
    g.box(ht, "hook_tip_spine_stud", (-6.55, 25.45, 6.05), (-5.95, 26.05, 6.40), "bone_lt")
    g.box(ht, "hook_tip_barb_fang", (-6.65, 26.85, 4.45), (-6.15, 27.55, 4.85), "bone_lt")
    g.box(ht, "hook_tip_notch_a", (-6.95, 27.65, 4.55), (-6.55, 28.05, 4.85), "void")
    g.box(ht, "hook_tip_splinter_1", (-6.05, 26.25, 4.65), (-5.65, 26.75, 4.95), "bone_dk")
    g.box(ht, "hook_tip_splinter_2", (-6.25, 25.55, 5.15), (-5.85, 26.05, 5.45), "bone_dk")

    # 3. 粗枯木柄加固铁锔箍与铁钉 (14 块)
    sm = "pole_shaft_mid"
    g.box(sm, "shaft_mid_iron_band_1", (-5.24, 16.50, 2.22), (-2.94, 16.93, 4.38), "rust_dk")
    g.box(sm, "shaft_mid_iron_band_2", (-5.18, 19.85, 2.22), (-3.02, 20.25, 4.38), "rust_dk")
    g.box(sm, "shaft_mid_stud_a1", (-5.28, 17.65, 2.45), (-4.98, 18.05, 2.75), "iron_lt")
    g.box(sm, "shaft_mid_stud_a2", (-3.22, 17.65, 2.45), (-2.92, 18.05, 2.75), "iron_lt")
    g.box(sm, "shaft_mid_stud_b1", (-5.28, 18.85, 2.45), (-4.98, 19.25, 2.75), "iron_lt")
    g.box(sm, "shaft_mid_stud_b2", (-3.22, 18.85, 2.45), (-2.92, 19.25, 2.75), "iron_lt")
    g.box(sm, "shaft_mid_stud_c1", (-5.28, 17.65, 3.78), (-4.98, 18.05, 4.05), "iron_lt")
    g.box(sm, "shaft_mid_stud_c2", (-3.22, 17.65, 3.75), (-2.92, 18.05, 4.05), "iron_lt")
    g.box(sm, "shaft_mid_wood_knot_1", (-5.17, 16.05, 2.85), (-4.85, 16.65, 3.35), "hide_dk")
    g.box(sm, "shaft_mid_wood_knot_2", (-3.35, 19.25, 2.85), (-2.95, 19.85, 3.35), "hide_dk")

    sl = "pole_shaft_lo"
    g.box(sl, "shaft_lo_cramp_1", (-5.37, 11.45, 2.22), (-3.25, 11.85, 2.52), "rust")
    g.box(sl, "shaft_lo_cramp_2", (-5.35, 13.85, 3.88), (-3.25, 14.25, 4.18), "rust")
    g.box(sl, "shaft_lo_stud_1", (-5.38, 12.45, 2.55), (-5.08, 12.85, 2.85), "iron_lt")
    g.box(sl, "shaft_lo_stud_2", (-3.32, 12.45, 2.55), (-3.02, 12.85, 2.85), "iron_lt")

    # 4. 骨节根部与套箍强化 (8 块)
    h_base = "hook_base"
    g.box(h_base, "hook_base_side_ridge_l", (-5.22, 26.05, 4.45), (-4.88, 27.15, 6.15), "bone_stain")
    g.box(h_base, "hook_base_side_ridge_r", (-3.12, 26.05, 4.45), (-2.78, 27.15, 6.15), "bone_stain")
    g.box(h_base, "hook_base_peg_head", (-5.45, 26.35, 5.12), (-5.15, 26.75, 5.48), "bone_lt")
    g.box(h_base, "hook_base_peg_tail", (-2.85, 26.35, 5.12), (-2.55, 26.75, 5.48), "bone_dk")

    pc_part = "pole_collar"
    g.box(pc_part, "collar_flange_l", (-5.43, 24.55, 4.45), (-4.95, 25.45, 5.15), "iron")
    g.box(pc_part, "collar_flange_r", (-3.05, 24.55, 4.45), (-2.65, 25.45, 5.15), "iron")
    g.box(pc_part, "collar_wedge_stud_1", (-4.35, 25.03, 4.15), (-3.95, 25.35, 4.45), "rust_dk")
    g.box(pc_part, "collar_wedge_stud_2", (-4.35, 24.95, 5.15), (-3.95, 25.35, 5.45), "rust_dk")

    # 5. 挂串流苏骨珠 (6 块)
    p_charm = "pole_charm"
    g.box(p_charm, "charm_bead_ext_1", (-4.75, 17.65, 3.42), (-4.45, 18.05, 3.72), "bone_stain")
    g.box(p_charm_bead := p_charm, "charm_bead_ext_2", (-4.75, 17.15, 3.42), (-4.45, 17.55, 3.72), "bone")
    g.box(p_charm, "charm_tag_border", (-5.38, 16.08, 4.02), (-4.52, 17.52, 4.68), "bone_dk")
    g.box(p_charm, "charm_tassel_strand_a", (-5.12, 14.35, 4.15), (-4.78, 15.25, 4.45), "cord")
    g.box(p_charm, "charm_tassel_strand_b", (-4.82, 14.15, 4.22), (-4.48, 15.23, 4.52), "cord_dk")
    g.box(p_charm, "charm_tassel_bead", (-4.98, 15.15, 4.18), (-4.62, 15.57, 4.48), "bone_lt")



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
    wear_note="挂点：背部左侧斜插（2px粗长木柄 + 比头宽大弯骨钩6-8px长高高探出）",
    worn_focus=((-4.5, 19.5, 3.8), 28.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
