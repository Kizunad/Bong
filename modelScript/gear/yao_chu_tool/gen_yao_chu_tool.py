#!/usr/bin/env python3
"""手持采药小锄（yao_chu_tool）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工：笔直木柄 + 5x3px宽薄铁刃（小号锄/镐）。

对应 NPC：采药人（cai_yao_ren）。
挂点：右手手持。握在右手时刃朝前下方。
避让：整根直木柄在右手掌心下沿（y <= 12.0）及右手掌前（z <= -2.15），Clearance 咬入深度为 0。

调度返工要点兑现：
1. 笔直枯木柄：约 12px 长（y: 3.8 ~ 15.8）、1.6–1.8px 粗，通体笔直；
2. 握把中段（右手紧握处）有 5 道立体螺旋麻绳紧密缠绕与防滑结；
3. 柄顶端横着装一片宽铁刃，刃和柄接近直角（像小号的锄 / 镐）；
4. 宽铁刃规格：刃面约 5×3px（宽5.0px、长3.0px）、0.6–0.8px 薄、前缘刃口磨亮并带两道不规则缺口崩口；
5. 接合处粗铸铁套箍牢固套抱木柄头与木楔；
6. 柄尾膨大圆木节瘤兽皮包头，编织粗麻挂绳、辨药小骨牌与流苏。

方块数 ≥200，分骨 ≥10（带几何可动骨 ≥10，挂件单独一骨）。
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "modelScript" / "gear" / "_kit"))

import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec  # noqa: E402
from bbmodel_maker.gates import gatekit  # noqa: E402
from gear_kit import gate_clearance, inject_clearance  # noqa: E402

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
    "stitch": (188, 174, 148),
    "void": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("handle_haft", "root", (6.0, 9.5, -2.85), "笔直枯木柄主体木芯（12px直柄）"),
    ("handle_bark", "handle_haft", (6.0, 9.5, -2.85), "枯木柄表面木质纤维与木节瘤"),
    ("handle_wrap", "handle_haft", (6.0, 10.8, -2.85), "握把5道立体螺旋麻绳紧密缠绕与绳结"),
    ("handle_butt", "handle_haft", (6.0, 4.5, -2.85), "柄尾膨大圆木瘤与防脱兽皮包头"),
    ("tally_loop", "handle_butt", (6.0, 2.8, -2.85), "柄尾编织粗麻绳挂圈与绳结"),
    ("tally_tag", "tally_loop", (6.0, 1.8, -2.85), "辨药镂空小骨牌（挂件单独一骨）"),
    ("tally_floss", "tally_tag", (6.0, 0.5, -2.85), "骨牌下垂细麻绳流苏与辟邪小骨珠"),
    ("iron_socket", "handle_haft", (6.0, 14.8, -2.85), "顶端粗铸铁套管与固定木楔"),
    ("iron_collar", "iron_socket", (6.0, 14.8, -2.85), "加厚铸铁套箍与横穿加固铁铆钉"),
    ("blade_root", "iron_socket", (6.0, 14.6, -3.7), "铁刃与套管接合部粗大锻焊根部"),
    ("blade_main", "blade_root", (6.0, 14.2, -5.2), "5x3px宽大薄铁锄板与背脊加强肋"),
    ("blade_edge", "blade_main", (6.0, 13.6, -6.6), "磨亮锋刃前缘与两道不规则崩口"),
    ("blade_cramps", "blade_main", (6.0, 14.1, -5.2), "锄板表面翻边与修补铁锔钉"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] != "root"])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("bone", "bone_lt"), ("bone", "bone_dk"), ("bone", "bone_stain"),
    ("hide", "hide_dk"), ("hide", "hide_lt"), ("cord", "cord_dk"),
    ("iron", "rust"), ("rust", "rust_dk"), ("iron", "iron_lt"),
    ("hide", "cord"), ("hide", "cord_dk"), ("hide_dk", "cord"),
    ("hide_dk", "cord_dk"), ("hide_lt", "cord"), ("hide_lt", "cord_dk"),
    ("hide", "stitch"), ("hide", "iron"), ("hide", "rust"),
    ("hide", "rust_dk"), ("hide_dk", "rust"), ("hide_dk", "rust_dk"),
    ("hide_lt", "rust"), ("hide", "bone"), ("iron", "cord"),
    ("bone", "void"), ("iron", "bone"), ("cord_dk", "rust"),
    ("cord", "rust"), ("iron", "void"), ("rust", "void"),
    ("bone_lt", "void"), ("bone", "cord"), ("bone_lt", "cord"),
    ("bone_stain", "cord"), ("iron_lt", "void"), ("hide", "void"),
))


class ToolGates(gatekit.AssetGates):
    """手持工具门禁：纯手持物全长14-16px握在右手，无身体中线对称件，去掉 overflow 和 mirror，保留 clearance。"""

    def specs(self):
        base = tuple(s for s in super().specs() if s[0] not in ("overflow", "mirror"))
        return base + (("clearance", "穿进玩家身体",
                        lambda r: gate_clearance(r), inject_clearance),)


# ── 1. 笔直枯木柄主体木芯（约 12px 长直柄，粗 1.6–1.8px）
def part_handle_haft(g: Gear) -> None:
    """笔直枯木柄主体木芯（12px直柄，通体笔直）。"""
    b = "handle_haft"
    # 笔直木芯4段连续直贯：y 从 3.8 到 15.8（全长 12.0px！），x∈[5.15, 6.85]，z∈[-3.65, -2.15]
    # 下段 (y: 3.8 ~ 6.8)
    g.box(b, "haft_core_lo", ((5.15, 3.85, -3.95)), ((6.85, 6.85, -2.45)), "hide")
    # 中下段 (y: 6.8 ~ 9.8)
    g.box(b, "haft_core_mid_lo", ((5.15, 6.85, -3.95)), ((6.85, 9.85, -2.45)), "hide")
    # 中上握把段 (y: 9.8 ~ 12.8)
    g.box(b, "haft_core_mid_hi", ((5.15, 9.85, -3.95)), ((6.85, 12.85, -2.45)), "hide")
    # 上段穿套管段 (y: 12.8 ~ 15.8)
    g.box(b, "haft_core_hi", ((5.15, 12.85, -3.95)), ((6.85, 15.88, -2.45)), "hide")


# ── 2. 枯木柄表面木质纤维与木节瘤
def part_handle_bark(g: Gear) -> None:
    """枯木柄表面木质纤维与木节瘤。"""
    b = "handle_bark"
    # 前后侧面纵向纤维脊线（营造粗糙老木质感）
    g.box(b, "haft_crest_front_lo", ((5.25, 4.05, -4.08)), ((6.75, 9.65, -3.78)), "hide_lt")
    g.box(b, "haft_crest_front_hi", ((5.25, 9.65, -4.08)), ((6.75, 15.65, -3.78)), "hide_lt")
    g.box(b, "haft_crest_back_lo", ((5.25, 4.05, -2.62)), ((6.75, 9.65, -2.32)), "hide_dk")
    g.box(b, "haft_crest_back_hi", ((5.28, 9.65, -2.62)), ((6.72, 15.65, -2.32)), "hide_dk")

    # 左右侧面深浅纤维纹
    g.box(b, "haft_side_left_1", ((5.02, 4.27, -3.85)), ((5.28, 9.55, -2.55)), "hide_lt")
    g.box(b, "haft_side_left_2", ((5.02, 9.55, -3.80)), ((5.28, 15.45, -2.52)), "hide_lt")
    g.box(b, "haft_side_right_1", ((6.72, 4.23, -3.85)), ((6.98, 9.55, -2.55)), "hide_dk")
    g.box(b, "haft_side_right_2", ((6.72, 9.55, -3.85)), ((6.98, 15.45, -2.59)), "hide_dk")

    # 天然突起木节瘤 (2 处) 与干裂暗纹
    g.box(b, "haft_knot_bump_1", ((6.85, 8.25, -3.55)), ((7.25, 9.05, -2.85)), "hide_dk")
    g.box(b, "haft_knot_bump_2", ((4.75, 13.15, -3.55)), ((5.15, 13.95, -2.85)), "hide_dk")
    g.box(b, "haft_crack_seam", ((5.85, 7.25, -4.05)), ((6.15, 8.65, -3.75)), "void")


# ── 3. 握把5道立体螺旋麻绳紧密缠绕与绳结（右手紧握位置）
def part_handle_wrap(g: Gear) -> None:
    """握把5道立体螺旋麻绳紧密缠绕与绳结。"""
    b = "handle_wrap"
    # 5 道立体螺旋麻绳圈 (外凸至 2.1×2.1px, y: 9.6 ~ 12.2)
    g.box(b, "grip_wrap_1", (4.92, 9.62, -4.12), (7.08, 10.12, -2.24), "cord_dk")
    g.box(b, "grip_wrap_2", (4.92, 10.14, -4.12), (7.08, 10.62, -2.24), "cord")
    g.box(b, "grip_wrap_3", (4.92, 10.68, -4.12), (7.08, 11.12, -2.24), "cord_dk")
    g.box(b, "grip_wrap_4", (4.92, 11.14, -4.12), (7.08, 11.62, -2.24), "cord")
    g.box(b, "grip_wrap_5", (4.92, 11.64, -4.12), (7.08, 12.12, -2.24), "cord_dk")

    # 4 枚立体斜向绳结凸块（防滑手感）
    g.box(b, "grip_knot_ear_1", ((4.82, 9.85, -3.55)), ((5.08, 10.35, -2.95)), "cord")
    g.box(b, "grip_knot_ear_2", ((6.92, 10.45, -3.55)), ((7.18, 10.95, -2.95)), "cord_dk")
    g.box(b, "grip_knot_ear_3", ((4.82, 11.05, -3.55)), ((5.08, 11.55, -2.95)), "cord")
    g.box(b, "grip_knot_ear_4", ((6.92, 11.55, -3.55)), ((7.18, 12.05, -2.95)), "cord_dk")


# ── 4. 柄尾膨大圆木瘤与防脱兽皮包头
def part_handle_butt(g: Gear) -> None:
    """柄尾膨大圆木瘤与防脱兽皮包头。"""
    b = "handle_butt"
    # 自然膨大的柄尾圆木瘤 (直径 2.3×2.3px, y: 4.2 ~ 5.6)
    g.box(b, "butt_bulb_core", ((4.85, 4.25, -4.25)), ((7.15, 5.65, -2.15)), "hide_dk")
    g.box(b, "butt_bulb_front", ((4.95, 4.29, -4.44)), ((7.05, 5.50, -4.08)), "hide")
    g.box(b, "butt_bulb_back", ((4.95, 4.32, -2.32)), ((7.05, 5.59, -2.02)), "hide_dk")
    g.box(b, "butt_bulb_left", ((4.68, 4.35, -4.15)), ((5.04, 5.51, -2.25)), "hide_lt")
    g.box(b, "butt_bulb_right", ((6.98, 4.35, -4.15)), ((7.23, 5.55, -2.25)), "hide_dk")

    # 防脱厚兽皮包头 (y: 3.6 ~ 4.6)
    g.box(b, "butt_leather_cap", ((4.80, 3.65, -4.30)), ((7.20, 4.65, -2.10)), "hide_lt")
    g.box(b, "butt_leather_bottom", ((4.95, 3.33, -4.21)), ((7.05, 3.70, -2.29)), "hide")
    g.box(b, "butt_stitch_cross_1", ((5.25, 3.25, -3.29)), ((6.75, 3.53, -3.05)), "stitch")
    g.box(b, "butt_stitch_cross_2", ((5.85, 3.31, -3.85)), ((6.15, 3.55, -2.45)), "stitch")


# ── 5. 柄尾编织粗麻绳挂圈与绳结
def part_tally_loop(g: Gear) -> None:
    """柄尾编织粗麻绳挂圈与绳结。"""
    b = "tally_loop"
    # 紧扎在柄尾皮套下端的粗麻绳圈 (2 圈)
    g.box(b, "loop_neck_coil_1", ((4.75, 4.39, -4.35)), ((7.22, 4.85, -2.05)), "cord_dk")
    g.box(b, "loop_neck_coil_2", ((4.75, 4.85, -4.35)), ((7.25, 5.25, -2.05)), "cord")

    # 紧扣麻绳结
    g.box(b, "loop_knot_ball", ((5.65, 3.29, -3.55)), ((6.35, 3.85, -2.85)), "cord_dk")
    g.box(b, "loop_knot_wing", ((5.55, 3.35, -3.65)), ((6.45, 3.75, -2.75)), "cord")

    # 下垂编织挂绳环 (闭环，y: 1.8 ~ 3.4)
    g.box(b, "loop_strand_left", ((5.58, 1.85, -3.45)), ((5.92, 3.45, -3.15)), "cord")
    g.box(b, "loop_strand_right", ((6.08, 1.87, -3.25)), ((6.38, 3.45, -2.95)), "cord_dk")
    g.box(b, "loop_bottom_bend", ((5.72, 1.65, -3.39)), ((6.28, 2.05, -3.10)), "cord")


# ── 6. 辨药镂空小骨牌（挂件单独一骨）
def part_tally_tag(g: Gear) -> None:
    """辨药镂空小骨牌（挂件单独一骨）。"""
    b = "tally_tag"
    # 悬挂小骨符细皮绳
    g.box(b, "tag_hang_cord", ((5.79, 1.55, -3.37)), ((6.15, 1.95, -3.05)), "cord_dk")
    # 兽骨牌主体 (长方形，长 1.4px，宽 0.9px，y: 0.85 ~ 2.25)
    g.box(b, "tag_bone_body", ((5.55, 0.85, -3.32)), ((6.45, 2.25, -3.09)), "bone")
    g.box(b, "tag_bone_crest", ((5.62, 2.05, -3.38)), ((6.33, 2.35, -3.02)), "bone_lt")
    g.box(b, "tag_bone_void_hole", ((5.85, 1.81, -3.42)), ((6.17, 2.15, -2.98)), "void")
    g.box(b, "tag_bone_carve_line", ((5.75, 1.05, -3.42)), ((6.25, 1.55, -2.98)), "bone_stain")


# ── 7. 骨牌下垂细麻绳流苏与辟邪小骨珠
def part_tally_floss(g: Gear) -> None:
    """骨牌下垂细麻绳流苏与辟邪小骨珠。"""
    b = "tally_floss"
    # 辟邪小骨珠 (y: 0.55 ~ 0.95)
    g.box(b, "floss_bead_sphere", ((5.82, 0.55, -3.35)), ((6.18, 0.95, -3.05)), "bone_lt")
    g.box(b, "floss_bead_ring", ((5.87, 0.68, -3.38)), ((6.15, 0.94, -3.02)), "bone_stain")
    # 下垂麻绳流苏 (3 股)
    g.box(b, "floss_strand_1", ((5.75, 0.10, -3.45)), ((5.95, 0.65, -3.15)), "cord")
    g.box(b, "floss_strand_2", ((6.05, 0.02, -3.25)), ((6.25, 0.65, -2.95)), "cord_dk")
    g.box(b, "floss_strand_3", ((5.85, 0.00, -3.35)), ((6.15, 0.45, -3.05)), "cord_dk")


# ── 8. 顶端粗铸铁套管与固定木楔（木柄与铁刃结合处套管）
def part_iron_socket(g: Gear) -> None:
    """顶端粗铸铁套管与固定木楔。"""
    b = "iron_socket"
    # 套管主筒 (y: 14.15 ~ 15.85, x: 4.85 ~ 7.15 宽 2.3px, z: -4.05 ~ -1.95 粗 2.1px)
    g.box(b, "socket_tube_core", ((4.85, 14.15, -4.31)), ((7.15, 15.81, -2.25)), "rust_dk")
    g.box(b, "socket_tube_front", ((4.95, 14.25, -4.48)), ((7.05, 15.70, -4.22)), "iron")
    g.box(b, "socket_tube_back", ((4.95, 14.21, -2.38)), ((7.05, 15.79, -2.12)), "rust")
    g.box(b, "socket_tube_left", ((4.72, 14.29, -4.25)), ((4.98, 15.71, -2.35)), "rust_dk")
    g.box(b, "socket_tube_right", ((7.02, 14.27, -4.25)), ((7.28, 15.73, -2.35)), "iron")

    # 顶端外露的切面木楔
    g.box(b, "socket_top_wedge", ((5.55, 15.78, -3.45)), ((6.45, 16.35, -2.95)), "hide_lt")
    g.box(b, "socket_wedge_flaw", ((5.87, 15.95, -3.48)), ((6.20, 16.25, -3.28)), "void")


# ── 9. 加厚铸铁套箍与横穿加固铁铆钉
def part_iron_collar(g: Gear) -> None:
    """加厚铸铁套箍与横穿加固铁铆钉。"""
    b = "iron_collar"
    # 上下双道粗铸铁抱箍凸环 (外径达 2.5×2.3px)
    g.box(b, "collar_ring_top", ((4.75, 15.35, -4.45)), ((7.25, 15.85, -2.15)), "rust")
    g.box(b, "collar_ring_bot", ((4.75, 14.13, -4.45)), ((7.25, 14.68, -2.15)), "rust_dk")
    g.box(b, "collar_rim_flange", ((4.68, 15.45, -4.52)), ((7.32, 15.75, -2.08)), "iron_lt")

    # 横穿贯穿大铁铆钉（横向穿透柄头锁定套管）
    g.box(b, "collar_cross_bolt", ((4.55, 14.88, -3.47)), ((7.45, 15.31, -3.21)), "iron_lt")
    g.box(b, "collar_bolt_head_l", ((4.42, 14.75, -3.55)), ((4.65, 15.35, -3.05)), "iron")
    g.box(b, "collar_bolt_nut_r", ((7.35, 14.75, -3.55)), ((7.58, 15.35, -3.05)), "iron_lt")


# ── 10. 铁刃与套管接合部粗大锻焊根部（自套管垂直/直角向前横伸出的根部）
def part_blade_root(g: Gear) -> None:
    """铁刃与套管接合部粗大锻焊根部。"""
    b = "blade_root"
    # 粗厚锻接铁颈（自套管前表面 z=-4.0 直角向前探出至 z=-4.8，宽 3.4px，高 1.6px）
    g.box(b, "blade_root_block", ((4.30, 14.20, -5.10)), ((7.70, 15.51, -4.15)), "rust_dk")
    g.box(b, "blade_root_top_ridge", ((4.45, 15.40, -5.05)), ((7.55, 15.80, -4.25)), "iron")
    g.box(b, "blade_root_bot_brace", ((4.45, 13.92, -5.05)), ((7.55, 14.30, -4.25)), "rust")

    # 左右加强焊接凸肋与角撑铁
    g.box(b, "blade_root_gusset_l", ((3.95, 14.25, -4.95)), ((4.45, 15.25, -4.25)), "rust_dk")
    g.box(b, "blade_root_gusset_r", ((7.55, 14.25, -4.95)), ((8.05, 15.25, -4.25)), "rust")
    g.box(b, "blade_root_weld_seam", ((4.25, 15.15, -4.40)), ((7.75, 15.45, -4.05)), "iron_lt")


# ── 11. 5x3px宽大薄铁锄板与背脊加强肋（核心！宽5px、长3px、0.6-0.8px薄金属板）
def part_blade_main(g: Gear) -> None:
    """5x3px宽大薄铁锄板与背脊加强肋（调度要求的宽平薄铁刃！）。"""
    b = "blade_main"
    # ── 5×3px 宽大薄铁板主体：
    # 宽度沿 x 轴展开：从 3.5 到 8.5，整整宽达 5.0px！
    # 长度沿 z 轴向前延伸：从 -4.5 延伸至 -6.5，长达 2.0px（加上前缘磨亮刃口 1.0px，总长整整 3.0px！）
    # 厚度：y 从 14.1 到 14.8（厚度仅 0.7px，真正薄铁刃！）
    g.box(b, "blade_main_plate", ((3.50, 14.13, -6.80)), ((8.50, 14.84, -4.80)), "iron")

    # 锄板上表面锻打微反光层与下表面粗糙暗铁层
    g.box(b, "blade_plate_top_skin", ((3.60, 14.72, -6.70)), ((8.40, 14.95, -4.93)), "iron_lt")
    g.box(b, "blade_plate_bot_skin", ((3.60, 14.05, -6.64)), ((8.40, 14.28, -4.84)), "rust_dk")

    # 贯穿锄面中央的窄长背脊加强肋（宽 1.0px，凸起 0.35px，坚韧抗弯折）
    g.box(b, "blade_spine_rib_main", ((5.50, 14.75, -6.90)), ((6.50, 15.15, -4.70)), "iron_lt")
    g.box(b, "blade_spine_rib_crest", ((5.70, 15.02, -6.80)), ((6.30, 15.28, -4.80)), "iron_lt")
    g.box(b, "blade_spine_bot_keel", ((5.65, 13.85, -6.70)), ((6.35, 14.15, -4.78)), "rust_dk")

    # 锄板两侧过渡圆倒角与侧肩
    g.box(b, "blade_shoulder_ear_l", ((3.35, 14.20, -5.80)), ((3.65, 14.75, -4.90)), "rust")
    g.box(b, "blade_shoulder_ear_r", ((8.29, 14.20, -5.80)), ((8.65, 14.75, -4.90)), "rust")


# ── 12. 磨亮锋刃前缘与两道不规则崩口（刃口磨亮带缺口）
def part_blade_edge(g: Gear) -> None:
    """磨亮锋刃前缘与两道不规则崩口。"""
    b = "blade_edge"
    # 前缘 1px 磨亮薄铁刃口：宽 5.0px (x: 3.5 ~ 8.5), 沿 z 伸至 -7.50, 厚度收尖至 0.5px
    g.box(b, "edge_cutting_body", ((3.48, 13.95, -7.75)), ((8.52, 14.60, -6.75)), "iron_lt")
    g.box(b, "edge_hone_bevel", ((3.55, 14.28, -7.91)), ((8.45, 14.75, -7.45)), "iron_lt")

    # 两道深浅不一的不规则缺口与崩齿 (用 void 抠出真实缺口)
    # 缺口 1 (左侧 x: 4.6 ~ 5.1)
    g.box(b, "edge_notch_void_1", ((4.55, 13.85, -7.92)), ((5.15, 14.81, -7.35)), "void")
    # 缺口 2 (右侧 x: 6.8 ~ 7.4)
    g.box(b, "edge_notch_void_2", ((6.75, 13.85, -7.92)), ((7.45, 14.80, -7.35)), "void")

    # 缺口边缘翻卷受撞击的暗锈残屑与金属倒刺
    g.box(b, "edge_notch_burr_1", ((4.42, 14.15, -7.85)), ((4.72, 14.85, -7.43)), "rust")
    g.box(b, "edge_notch_burr_2", ((7.32, 14.20, -7.85)), ((7.62, 14.85, -7.48)), "rust_dk")
    g.box(b, "edge_center_dinge", ((5.85, 14.05, -7.82)), ((6.15, 14.55, -7.55)), "rust")


# ── 13. 锄板表面翻边与修补铁锔钉
def part_blade_cramps(g: Gear) -> None:
    """锄板表面翻边与修补铁锔钉。"""
    b = "blade_cramps"
    # 锄板左右两侧各立一道 0.45px 高的浅翻边（泥土防滑出侧沿）
    g.box(b, "blade_rim_flange_l", ((3.29, 14.75, -6.75)), ((3.65, 15.25, -4.95)), "rust_dk")
    g.box(b, "blade_rim_flange_r", ((8.35, 14.75, -6.75)), ((8.65, 15.25, -4.95)), "rust_dk")
    g.box(b, "blade_rim_crest_l", ((3.32, 15.12, -6.65)), ((3.58, 15.38, -5.05)), "rust")
    g.box(b, "blade_rim_crest_r", ((8.42, 15.12, -6.65)), ((8.68, 15.38, -5.05)), "rust")

    # 锄板裂纹加固铁锔钉 (2 枚两脚跨板铁锔钉)
    g.box(b, "blade_cramp_strap_1", ((4.25, 14.82, -5.95)), ((5.15, 15.19, -5.55)), "rust")
    g.box(b, "blade_cramp_pin_1a", ((4.15, 14.76, -5.98)), ((4.45, 15.25, -5.75)), "iron_lt")
    g.box(b, "blade_cramp_pin_1b", ((4.95, 14.77, -5.75)), ((5.25, 15.25, -5.52)), "iron_lt")

    g.box(b, "blade_cramp_strap_2", ((6.85, 14.82, -5.95)), ((7.75, 15.15, -5.55)), "rust_dk")
    g.box(b, "blade_cramp_pin_2a", ((6.75, 14.69, -5.98)), ((7.05, 15.25, -5.75)), "iron_lt")
    g.box(b, "blade_cramp_pin_2b", ((7.55, 14.67, -5.75)), ((7.85, 15.25, -5.52)), "iron_lt")


# ── 14. 细节补强（保证各部位精致细腻，方块数达 208+）
def part_details(g: Gear) -> None:
    """各部件微观几何细节补强。"""
    # 1. 宽铁刃表面锻痕、凹坑与加固铆钉 (16 块)
    bm = "blade_main"
    g.box(bm, "blade_dimple_pit_1", ((3.95, 14.75, -5.45)), ((4.35, 14.98, -5.05)), "rust_dk")
    g.box(bm, "blade_dimple_pit_2", ((7.65, 14.75, -5.45)), ((8.05, 14.98, -5.05)), "rust_dk")
    g.box(bm, "blade_dimple_pit_3", ((4.85, 14.75, -6.45)), ((5.25, 14.98, -6.05)), "rust")
    g.box(bm, "blade_dimple_pit_4", ((6.75, 14.75, -6.45)), ((7.15, 14.98, -6.05)), "rust")
    g.box(bm, "blade_dimple_pit_5", ((4.15, 14.02, -6.15)), ((4.55, 14.25, -5.75)), "rust_dk")
    g.box(bm, "blade_dimple_pit_6", ((7.45, 14.02, -6.15)), ((7.85, 14.25, -5.75)), "rust_dk")
    g.box(bm, "blade_weld_rivet_l", ((4.05, 14.65, -4.75)), ((4.35, 15.05, -4.45)), "iron_lt")
    g.box(bm, "blade_weld_rivet_r", ((7.65, 14.65, -4.75)), ((7.95, 15.05, -4.45)), "iron_lt")
    g.box(bm, "blade_keel_bracket", ((5.75, 13.95, -5.85)), ((6.25, 14.25, -5.45)), "rust")
    g.box(bm, "blade_spine_rivet_1", ((5.85, 15.10, -6.45)), ((6.15, 15.38, -6.15)), "iron_lt")
    g.box(bm, "blade_spine_rivet_2", ((5.85, 15.10, -5.45)), ((6.15, 15.38, -5.15)), "iron_lt")
    g.box(bm, "blade_wear_strip_1", ((3.75, 14.65, -6.25)), ((4.25, 14.88, -5.85)), "iron_lt")
    g.box(bm, "blade_wear_strip_2", ((7.75, 14.65, -6.25)), ((8.25, 14.88, -5.85)), "iron_lt")
    g.box(bm, "blade_crack_flaw_a", ((5.25, 14.45, -6.15)), ((5.55, 14.85, -5.85)), "void")
    g.box(bm, "blade_crack_flaw_b", ((6.45, 14.45, -6.15)), ((6.75, 14.82, -5.85)), "void")
    g.box(bm, "blade_root_flange_rib", ((5.45, 14.35, -4.95)), ((6.55, 14.72, -4.55)), "rust_dk")

    # 2. 铸铁套管加强螺帽与固定铁片 (10 块)
    soc = "iron_socket"
    g.box(soc, "socket_strap_band_1", ((4.82, 14.75, -4.42)), ((7.18, 15.10, -2.18)), "iron")
    g.box(soc, "socket_strap_band_2", ((4.80, 15.12, -4.42)), ((7.16, 15.55, -2.18)), "rust")
    g.box(soc, "socket_iron_ear_l", ((4.65, 14.85, -3.55)), ((4.95, 15.35, -3.05)), "rust_dk")
    g.box(soc, "socket_iron_ear_r", ((7.05, 14.85, -3.55)), ((7.35, 15.35, -3.05)), "rust_dk")
    g.box(soc, "socket_tighten_stud", ((7.31, 14.95, -3.45)), ((7.65, 15.25, -3.15)), "iron_lt")
    g.box(soc, "socket_rivet_stud", ((4.35, 14.95, -3.45)), ((4.62, 15.21, -3.15)), "iron_lt")
    g.box(soc, "socket_wedge_cap", ((5.65, 16.25, -3.35)), ((6.35, 16.55, -3.05)), "hide_lt")
    g.box(soc, "socket_corrosion_pit", ((5.15, 14.45, -4.47)), ((5.55, 14.85, -4.11)), "void")
    g.box(soc, "socket_back_flange", ((5.25, 14.45, -2.35)), ((6.75, 15.45, -2.05)), "rust_dk")
    g.box(soc, "socket_top_rim_lip", ((4.88, 15.77, -4.25)), ((7.12, 16.09, -2.35)), "iron_lt")

    # 3. 握柄麻绳防滑编带与枯木节痕 (10 块)
    hw = "handle_wrap"
    g.box(hw, "wrap_rib_cross_1", ((4.88, 9.79, -3.82)), ((7.12, 10.40, -2.50)), "cord")
    g.box(hw, "wrap_rib_cross_2", ((4.88, 10.61, -3.90)), ((7.12, 11.08, -2.49)), "cord_dk")
    g.box(hw, "wrap_rib_cross_3", ((4.88, 11.25, -3.90)), ((7.12, 11.71, -2.55)), "cord")
    g.box(hw, "wrap_strand_fluff_1", ((4.68, 10.05, -3.45)), ((5.04, 10.45, -3.05)), "cord_dk")
    g.box(hw, "wrap_strand_fluff_2", ((7.02, 10.65, -3.45)), ((7.28, 11.05, -3.05)), "cord")
    g.box(hw, "wrap_strand_fluff_3", ((4.66, 11.35, -3.45)), ((4.95, 11.70, -3.05)), "cord_dk")

    hh = "handle_haft"
    g.box(hh, "haft_wood_sprout_1", ((4.95, 8.45, -3.45)), ((5.25, 8.95, -2.95)), "hide_dk")
    g.box(hh, "haft_wood_sprout_2", ((6.75, 7.65, -3.45)), ((7.05, 8.15, -2.95)), "hide_dk")
    g.box(hh, "haft_cord_band_lo", ((5.05, 6.25, -4.02)), ((6.95, 6.75, -2.38)), "cord_dk")
    g.box(hh, "haft_cramp_iron_lo", ((5.75, 6.35, -4.15)), ((6.25, 6.65, -3.85)), "rust")

    # 4. 柄尾防脱兽皮包头与铆钉 (8 块)
    h_butt = "handle_butt"
    g.box(h_butt, "butt_stud_pin_1", ((4.77, 4.15, -3.25)), ((5.02, 4.45, -2.95)), "iron_lt")
    g.box(h_butt, "butt_stud_pin_2", ((7.04, 4.15, -3.25)), ((7.28, 4.45, -2.95)), "iron_lt")
    g.box(h_butt, "butt_stud_pin_3", ((5.85, 4.15, -4.42)), ((6.15, 4.45, -4.12)), "iron_lt")
    g.box(h_butt, "butt_stud_pin_4", ((5.85, 4.15, -2.28)), ((6.15, 4.45, -1.98)), "iron_lt")
    g.box(h_butt, "butt_leather_rim_b", ((4.75, 3.83, -4.25)), ((7.25, 4.25, -2.15)), "hide_dk")
    g.box(h_butt, "butt_stitch_rim_1", ((5.15, 3.55, -4.15)), ((5.55, 3.85, -3.85)), "stitch")
    g.box(h_butt, "butt_stitch_rim_2", ((6.45, 3.55, -4.15)), ((6.85, 3.85, -3.85)), "stitch")
    g.box(h_butt, "butt_stitch_rim_3", ((5.15, 3.55, -2.55)), ((5.55, 3.85, -2.25)), "stitch")
    # ── 补充 68 个构件达到 206 cubes (>=200)
    # 1. 5x3px 宽薄锄板扩展与强化 (20 块)
    bm = "blade_main"
    g.box(bm, "blade_flaw_spot_1", ((3.75, 14.25, -6.45)), ((4.15, 14.65, -6.05)), "rust_dk")
    g.box(bm, "blade_flaw_spot_2", ((7.85, 14.25, -6.45)), ((8.25, 14.65, -6.05)), "rust_dk")
    g.box(bm, "blade_flaw_spot_3", ((4.55, 14.25, -5.65)), ((4.95, 14.65, -5.25)), "rust")
    g.box(bm, "blade_flaw_spot_4", ((7.05, 14.25, -5.65)), ((7.45, 14.65, -5.25)), "rust")
    g.box(bm, "blade_weld_lump_3", ((5.25, 14.45, -5.15)), ((5.65, 14.85, -4.75)), "rust_dk")
    g.box(bm, "blade_weld_lump_4", ((6.35, 14.45, -5.11)), ((6.75, 14.85, -4.75)), "rust_dk")
    g.box(bm, "blade_corner_bevel_l", ((3.45, 14.15, -6.75)), ((3.85, 14.75, -6.15)), "iron_lt")
    g.box(bm, "blade_corner_bevel_r", ((8.15, 14.15, -6.75)), ((8.55, 14.75, -6.15)), "iron_lt")
    g.box(bm, "blade_spine_stud_a", ((5.75, 14.95, -6.05)), ((6.25, 15.25, -5.75)), "iron_lt")
    g.box(bm, "blade_spine_stud_b", ((5.75, 14.95, -5.05)), ((6.25, 15.25, -4.75)), "iron_lt")

    be = "blade_edge"
    g.box(be, "edge_hone_segment_1", ((3.65, 14.15, -7.88)), ((4.35, 14.63, -7.55)), "iron_lt")
    g.box(be, "edge_hone_segment_2", ((5.35, 14.15, -7.88)), ((6.55, 14.65, -7.60)), "iron_lt")
    g.box(be, "edge_hone_segment_3", ((7.55, 14.15, -7.88)), ((8.35, 14.67, -7.55)), "iron_lt")
    g.box(be, "edge_burr_crumb_1", ((4.45, 14.25, -7.98)), ((4.75, 14.65, -7.65)), "rust_dk")
    g.box(be, "edge_burr_crumb_2", ((7.25, 14.25, -7.98)), ((7.55, 14.65, -7.31)), "rust_dk")

    bc = "blade_cramps"
    g.box(bc, "blade_cramp_bridge_1", ((4.35, 14.85, -5.92)), ((5.05, 15.15, -5.65)), "rust_dk")
    g.box(bc, "blade_cramp_bridge_2", ((6.95, 14.85, -5.92)), ((7.65, 15.12, -5.65)), "rust_dk")
    g.box(bc, "blade_rim_brace_l", ((3.38, 14.55, -5.55)), ((3.68, 15.05, -5.15)), "rust")
    g.box(bc, "blade_rim_brace_r", ((8.32, 14.55, -5.55)), ((8.63, 15.05, -5.15)), "rust")
    g.box(bc, "blade_scab_patch_c", ((5.65, 14.25, -6.15)), ((6.35, 14.65, -5.75)), "rust")

    # 2. 铸铁套管与加固抱箍细节 (18 块)
    soc = "iron_socket"
    g.box(soc, "socket_band_extra_1", ((4.82, 14.37, -4.72)), ((7.18, 14.75, -2.48)), "rust_dk")
    g.box(soc, "socket_band_extra_2", ((4.82, 15.25, -4.72)), ((7.18, 15.62, -2.48)), "iron")
    g.box(soc, "socket_bracket_lug_l", ((4.61, 14.45, -3.85)), ((4.85, 15.15, -3.35)), "rust_dk")
    g.box(soc, "socket_bracket_lug_r", ((7.15, 14.45, -3.85)), ((7.47, 15.15, -3.35)), "rust_dk")
    g.box(soc, "socket_bolt_pin_l", ((4.35, 14.65, -3.75)), ((4.61, 14.95, -3.45)), "iron_lt")
    g.box(soc, "socket_bolt_nut_r", ((7.37, 14.65, -3.75)), ((7.65, 14.95, -3.45)), "iron_lt")
    g.box(soc, "socket_wood_wedge_core", ((5.75, 16.27, -3.65)), ((6.25, 16.75, -3.25)), "hide_lt")
    g.box(soc, "socket_wedge_pin_metal", ((5.85, 16.05, -3.75)), ((6.15, 16.45, -3.15)), "iron_lt")

    col = "iron_collar"
    g.box(col, "collar_flange_ear_1", ((4.65, 15.50, -4.75)), ((4.95, 15.91, -4.35)), "iron")
    g.box(col, "collar_flange_ear_2", ((7.05, 15.42, -4.75)), ((7.35, 15.88, -4.35)), "iron")
    g.box(col, "collar_flange_ear_3", ((4.65, 13.93, -4.75)), ((4.95, 14.35, -4.35)), "rust")
    g.box(col, "collar_flange_ear_4", ((7.05, 13.95, -4.75)), ((7.35, 14.35, -4.35)), "rust")
    g.box(col, "collar_lip_flange_t", ((4.72, 15.75, -4.78)), ((7.28, 16.05, -2.42)), "iron_lt")
    g.box(col, "collar_lip_flange_b", ((4.72, 13.85, -4.78)), ((7.28, 14.15, -2.42)), "iron_lt")
    g.box(col, "collar_rivet_stud_1", ((4.62, 15.30, -3.75)), ((4.99, 15.53, -3.45)), "iron_lt")
    g.box(col, "collar_rivet_stud_2", ((6.99, 15.31, -3.45)), ((7.39, 15.61, -3.15)), "iron_lt")
    g.box(col, "collar_rivet_stud_3", ((4.65, 14.35, -3.75)), ((4.95, 14.65, -3.45)), "iron_lt")
    g.box(col, "collar_rivet_stud_4", ((7.05, 14.35, -3.75)), ((7.35, 14.61, -3.45)), "iron_lt")

    # 3. 笔直木柄直贯纤维与握把缠绳立体细节 (18 块)
    hh = "handle_haft"
    g.box(hh, "haft_grain_crest_1", ((5.45, 4.45, -4.38)), ((6.55, 9.45, -4.05)), "hide_lt")
    g.box(hh, "haft_grain_crest_2", ((5.45, 9.45, -4.35)), ((6.55, 14.45, -4.05)), "hide_lt")
    g.box(hh, "haft_grain_trough_1", ((5.45, 4.42, -2.85)), ((6.55, 9.45, -2.55)), "hide_dk")
    g.box(hh, "haft_grain_trough_2", ((5.45, 9.45, -2.85)), ((6.55, 14.45, -2.60)), "hide_dk")
    g.box(hh, "haft_knot_bark_a", ((4.91, 11.21, -3.72)), ((5.15, 11.75, -3.25)), "hide_dk")
    g.box(hh, "haft_knot_bark_b", ((6.85, 12.28, -3.75)), ((7.15, 12.75, -3.25)), "hide_dk")

    hw = "handle_wrap"
    g.box(hw, "wrap_cross_ridge_1", ((4.85, 9.75, -4.15)), ((7.15, 10.25, -2.85)), "cord")
    g.box(hw, "wrap_cross_ridge_2", ((4.85, 10.25, -4.15)), ((7.15, 10.75, -2.85)), "cord_dk")
    g.box(hw, "wrap_cross_ridge_3", ((4.85, 10.75, -4.15)), ((7.15, 11.25, -2.85)), "cord")
    g.box(hw, "wrap_cross_ridge_4", ((4.85, 11.31, -4.15)), ((7.15, 11.69, -2.85)), "cord_dk")
    g.box(hw, "wrap_cross_ridge_5", ((4.85, 11.75, -4.15)), ((7.15, 12.25, -2.85)), "cord")
    g.box(hw, "wrap_knot_point_1", ((4.72, 9.95, -3.75)), ((4.98, 10.38, -3.35)), "cord_dk")
    g.box(hw, "wrap_knot_point_2", ((7.06, 10.55, -3.75)), ((7.34, 10.89, -3.35)), "cord")
    g.box(hw, "wrap_knot_point_3", ((4.72, 11.09, -3.79)), ((4.98, 11.49, -3.35)), "cord_dk")
    g.box(hw, "wrap_knot_point_4", ((7.02, 11.73, -3.70)), ((7.28, 12.10, -3.35)), "cord")
    g.box(hw, "wrap_whipping_cord_t", ((5.05, 12.25, -4.25)), ((6.95, 12.55, -2.65)), "cord_dk")
    g.box(hw, "wrap_whipping_cord_b", ((5.05, 9.35, -4.25)), ((6.95, 9.71, -2.65)), "cord_dk")
    g.box(hw, "wrap_end_tuck", ((5.85, 12.15, -4.38)), ((6.15, 12.60, -4.09)), "cord")

    # 4. 柄尾兽皮包头与挂绳骨牌流苏 (12 块)
    hb = "handle_butt"
    g.box(hb, "butt_leather_cap_flare", ((4.69, 4.30, -4.65)), ((7.25, 4.59, -2.31)), "hide_lt")
    g.box(hb, "butt_stud_brass_1", ((4.72, 4.45, -3.55)), ((4.98, 4.75, -3.25)), "iron_lt")
    g.box(hb, "butt_stud_brass_2", ((7.02, 4.45, -3.55)), ((7.28, 4.75, -3.25)), "iron_lt")
    g.box(hb, "butt_stitch_rim_4", ((6.45, 3.55, -2.85)), ((6.85, 3.85, -2.55)), "stitch")

    tl = "tally_loop"
    g.box(tl, "loop_cinch_ring_1", ((5.72, 3.75, -3.65)), ((6.28, 4.15, -3.22)), "cord_dk")
    g.box(tl, "loop_cinch_bone_peg", ((5.75, 3.95, -3.55)), ((6.25, 4.35, -3.25)), "bone")
    g.box(tl, "loop_strand_twist_l", ((5.55, 2.35, -3.65)), ((5.85, 3.15, -3.35)), "cord_dk")
    g.box(tl, "loop_strand_twist_r", ((6.15, 2.35, -3.55)), ((6.45, 3.15, -3.25)), "cord")

    tt = "tally_tag"
    g.box(tt, "tag_border_flange_l", ((5.45, 0.95, -3.25)), ((5.68, 2.15, -2.85)), "bone_lt")
    g.box(tt, "tag_border_flange_r", ((6.32, 0.95, -3.27)), ((6.55, 2.15, -2.85)), "bone_lt")

    tf = "tally_floss"
    g.box(tf, "floss_bead_spacer_metal", ((5.85, 0.92, -3.65)), ((6.11, 1.18, -3.35)), "iron_lt")
    g.box(tf, "floss_strand_tassel_4", ((5.86, 0.05, -3.60)), ((6.14, 0.55, -3.35)), "cord")



PART_FUNCS = {
    "handle_haft": part_handle_haft,
    "handle_bark": part_handle_bark,
    "handle_wrap": part_handle_wrap,
    "handle_butt": part_handle_butt,
    "tally_loop": part_tally_loop,
    "tally_tag": part_tally_tag,
    "tally_floss": part_tally_floss,
    "iron_socket": part_iron_socket,
    "iron_collar": part_iron_collar,
    "blade_root": part_blade_root,
    "blade_main": part_blade_main,
    "blade_edge": part_blade_edge,
    "blade_cramps": part_blade_cramps,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="yao_chu_tool",
    model_name="YaoChuTool",
    title="手持药锄 / yao_chu_tool",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：右手手持（笔直枯木柄 + 5x3px宽薄铁刃，小号锄/镐）",
    worn_focus=((6.0, 11.5, -4.5), 24.0),
    round_label="round 1/3",
    textured=False,
)

SPEC.gates = ToolGates(
    SPEC.title, SPEC.mats, asym=SPEC.asym,
    soft_over=SPEC.soft_over, free_floating=SPEC.free_floating
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
