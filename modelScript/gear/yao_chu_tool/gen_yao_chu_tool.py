#!/usr/bin/env python3
"""手持采药小锄（yao_chu_tool）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

对应 NPC：采药人（cai_yao_ren）。
挂点：手持（右手握持）。全长约 15–16px，握在右手里。
避让：握持段贴右手掌心下沿（y <= 12.0），前段与锄头出右手掌前（z <= -2.08），Clearance 咬入深度为 0。

造型特征：
1. 凡器档的采药小锄：厚铁刃带刃口磨亮和两道缺口、卷刃、背脊加强肋；
2. 锄头与柄的粗铸铁箍接合、加固木楔与横穿铁铆钉；
3. 弯曲枯木柄有握把粗麻缠绳、防滑结、木节瘤与柄顶大木瘤兽皮包头；
4. 柄尾编织粗麻绳挂圈、镂空辨药小骨牌与流苏。

方块数 ≥200，分骨 ≥10（带几何可动骨 ≥10，挂件单独一骨）。
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "modelScript" / "gear" / "_kit"))

import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec

from bbmodel_maker.gates import gatekit
from gear_kit import gate_clearance, inject_clearance


class ToolGates(gatekit.AssetGates):
    """手持工具门禁：纯手持物全长14-16px握在右手，无身体中线对称件，去掉 overflow 和 mirror，加上 clearance。"""

    def specs(self):
        base = tuple(s for s in super().specs() if s[0] not in ("overflow", "mirror"))
        return base + (("clearance", "穿进玩家身体",
                        lambda r: gate_clearance(r), inject_clearance),)
  # noqa: E402

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
    ("hoe_socket", "root", (6.0, 14.5, -2.8), "粗铸铁套管与加固木楔"),
    ("hoe_collar_iron", "hoe_socket", (6.0, 14.8, -2.8), "加厚双道铸铁套箍与横穿加固铆钉"),
    ("blade_neck", "hoe_socket", (6.0, 14.2, -3.4), "锄头弯折粗连接颈与过渡段"),
    ("blade_body", "blade_neck", (6.0, 12.5, -4.5), "宽厚主锄板与贯穿背脊加强肋"),
    ("blade_edge", "blade_body", (6.0, 10.2, -5.8), "磨亮锋刃前缘与两道不规则崩口"),
    ("blade_patches", "blade_body", (6.0, 12.2, -4.5), "翻卷护边与铁锔钉修补片"),
    ("handle_upper", "hoe_socket", (6.0, 13.2, -2.4), "枯木柄上段斜伸木芯"),
    ("handle_grip", "root", (6.0, 10.8, -0.5), "右手掌心正下方主握把木芯"),
    ("handle_wraps", "handle_grip", (6.0, 10.8, -0.5), "握把多段立体粗麻绳紧密缠绕与防滑结"),
    ("handle_lower", "handle_grip", (6.0, 8.2, 0.8), "枯木柄下段弯曲段与木节瘤"),
    ("handle_butt", "handle_lower", (6.0, 5.5, 1.8), "粗圆大木节瘤柄尾与防脱兽皮包头"),
    ("tally_cord", "handle_butt", (6.0, 3.8, 2.2), "柄尾编织粗麻绳挂圈与绳结"),
    ("tally_charm", "tally_cord", (6.0, 2.5, 2.2), "辨药镂空小骨牌与下垂细流苏"),
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


# ── 1. 粗铸铁套管与加固木楔（圆筒套管连接木柄与锄刃）
def part_socket(g: Gear) -> None:
    """粗铸铁套管与加固木楔。"""
    b = "hoe_socket"
    # 圆筒套管主身 (x: 4.95 ~ 7.05 宽 2.1px, y: 13.55 ~ 15.55 高 2.0px, z: -3.85 ~ -1.75 粗 2.1px)
    g.box(b, "socket_iron_core", (4.99, 13.59, -4.30), (7.03, 15.61, -2.20), "rust_dk")
    g.box(b, "socket_iron_front", (5.05, 13.60, -4.43), (6.95, 15.37, -4.17), "iron")
    g.box(b, "socket_iron_back", (5.05, 13.65, -2.33), (6.95, 15.45, -2.07), "rust")
    g.box(b, "socket_iron_left", (4.82, 13.61, -4.20), (5.08, 15.53, -2.30), "rust_dk")
    g.box(b, "socket_iron_right", (6.92, 13.62, -4.20), (7.18, 15.41, -2.30), "iron")

    # 顶端固定大木楔与铁挡销
    g.box(b, "socket_wood_wedge", (5.69, 15.45, -3.40), (6.35, 16.15, -2.70), "hide_lt")
    g.box(b, "socket_cross_pin", (4.65, 14.29, -3.20), (7.35, 14.81, -2.94), "iron_lt")
    g.box(b, "socket_cramp_top", (5.45, 15.10, -4.37), (6.55, 15.63, -4.07), "rust")


# ── 2. 加厚双道铸铁套箍与横穿加固铆钉
def part_collar_iron(g: Gear) -> None:
    """加厚双道铸铁套箍与横穿加固铆钉。"""
    b = "hoe_collar_iron"
    # 上下双道粗铸铁凸箍 (环绕套管，外径达 2.4×2.4px)
    g.box(b, "collar_upper_ring", (4.85, 15.05, -4.40), (7.15, 15.65, -2.10), "rust")
    g.box(b, "collar_upper_lip", (4.78, 15.15, -4.47), (7.22, 15.55, -2.03), "iron_lt")
    g.box(b, "collar_lower_ring", (4.87, 13.41, -4.32), (7.10, 14.05, -2.10), "rust_dk")
    g.box(b, "collar_lower_lip", (4.78, 13.55, -4.47), (7.22, 13.95, -2.03), "iron")

    # 四角加固方形铁铆钉 (4 颗)
    g.box(b, "collar_rivet_fl", (4.95, 14.45, -4.50), (5.35, 14.85, -4.20), "iron_lt")
    g.box(b, "collar_rivet_fr", (6.65, 14.45, -4.50), (6.99, 14.85, -4.20), "iron_lt")
    g.box(b, "collar_rivet_bl", (4.95, 14.45, -2.30), (5.35, 14.85, -2.00), "iron_lt")
    g.box(b, "collar_rivet_br", (6.65, 14.45, -2.30), (7.05, 14.85, -2.00), "iron_lt")


# ── 3. 锄头弯折粗连接颈与过渡段（自套管向前下方斜探弯折）
def part_blade_neck(g: Gear) -> None:
    """锄头弯折粗连接颈与过渡段。"""
    b = "blade_neck"
    # 连接颈粗厚铁梁 (从套管向前下方探出: y从14.8降到13.5，z从-3.5向前延伸到-4.6)
    g.box(b, "neck_main_beam", (5.12, 13.71, -5.00), (6.82, 14.95, -3.90), "rust_dk")
    g.box(b, "neck_top_ridge", (5.35, 14.85, -4.93), (6.65, 15.17, -3.97), "iron")
    g.box(b, "neck_bot_brace", (5.35, 13.35, -4.80), (6.65, 13.79, -4.07), "rust")

    # 过渡段向两侧展宽 (宽度展至 3.2px)
    g.box(b, "neck_flare_l", (4.45, 13.45, -5.10), (5.25, 14.65, -4.10), "rust_dk")
    g.box(b, "neck_flare_r", (6.75, 13.45, -5.10), (7.55, 14.65, -4.07), "rust")
    g.box(b, "neck_weld_seam", (4.85, 14.55, -4.40), (7.15, 14.87, -4.00), "iron_lt")


# ── 4. 宽厚主锄板与贯穿背脊加强肋（宽达 4.2px，厚 1.5px，向前下方斜向刨出）
def part_blade_body(g: Gear) -> None:
    """宽厚主锄板与贯穿背脊加强肋。"""
    b = "blade_body"
    # 宽阔主锄板 (宽 x: 3.9 ~ 8.1 宽达 4.2px！长 z: -6.0 ~ -4.3，高度 y: 11.2 ~ 14.0)
    g.box(b, "blade_body_plate", (3.95, 11.45, -6.30), (8.05, 13.85, -4.68), "iron")
    g.box(b, "blade_front_bevel", (4.05, 11.50, -6.47), (7.95, 13.75, -6.17), "iron_lt")
    g.box(b, "blade_back_bevel", (4.05, 11.55, -4.83), (7.95, 13.75, -4.53), "rust")

    # 贯穿锄面中央的粗大加强背脊肋骨 (凸起厚达 1.4px)
    g.box(b, "blade_spine_main", (5.55, 11.65, -6.56), (6.41, 14.25, -4.50), "iron_lt")
    g.box(b, "blade_spine_crest", (5.72, 11.85, -6.73), (6.30, 14.05, -4.40), "iron_lt")
    g.box(b, "blade_spine_anchor", (5.65, 13.65, -4.90), (6.35, 14.51, -4.20), "rust_dk")

    # 锄板上段防崩折加固厚台
    g.box(b, "blade_shoulder_l", (3.82, 12.77, -5.90), (4.45, 14.05, -4.80), "rust")
    g.box(b, "blade_shoulder_r", (7.55, 12.89, -5.90), (8.18, 14.05, -4.80), "rust")


# ── 5. 磨亮锋刃前缘与两道不规则崩口（刃口磨亮，凡器刨地磨痕）
def part_blade_edge(g: Gear) -> None:
    """磨亮锋刃前缘与两道不规则崩口。"""
    b = "blade_edge"
    # 前缘 1px 磨亮薄刃主体 (y: 9.8 ~ 11.6, z: -6.95 ~ -5.75)
    g.box(b, "blade_cutting_edge", (3.92, 9.89, -7.30), (8.08, 11.55, -6.20), "iron_lt")
    g.box(b, "blade_edge_shiny_lip", (4.02, 9.75, -7.43), (7.98, 10.85, -7.10), "iron_lt")

    # 两道深浅不一的不规则崩齿与缺口 (使用 void 形成真实残损)
    g.box(b, "blade_notch_void_1", (4.85, 9.68, -7.50), (5.35, 10.35, -7.00), "void")
    g.box(b, "blade_notch_void_2", (6.65, 9.68, -7.50), (7.25, 10.51, -7.00), "void")

    # 缺口旁翻卷翘起的钝铁毛刺与卷刃
    g.box(b, "blade_burr_lip_1", (4.75, 9.95, -7.53), (5.05, 10.55, -7.13), "rust")
    g.box(b, "blade_burr_lip_2", (7.15, 9.95, -7.53), (7.45, 10.65, -7.13), "rust_dk")
    g.box(b, "blade_burr_pad_3", (5.85, 9.72, -7.40), (6.25, 10.25, -7.04), "iron")


# ── 6. 翻卷护边与铁锔钉修补片
def part_blade_patches(g: Gear) -> None:
    """翻卷护边与铁锔钉修补片。"""
    b = "blade_patches"
    # 锄板两侧向上翻起的 1.2px 高防漏土浅翻边
    g.box(b, "blade_lip_wall_l", (3.78, 11.55, -6.20), (4.15, 13.55, -4.90), "rust_dk")
    g.box(b, "blade_lip_crest_l", (3.72, 12.65, -6.10), (4.02, 13.65, -5.00), "rust")
    g.box(b, "blade_lip_wall_r", (7.85, 11.55, -6.22), (8.22, 13.55, -4.90), "rust_dk")
    g.box(b, "blade_lip_crest_r", (7.98, 12.65, -6.10), (8.28, 13.65, -5.00), "rust")

    # 锄板裂纹加固铁锔钉 (2 枚两脚铁锔钉)
    g.box(b, "blade_cramp_plate_1", (4.45, 12.25, -5.90), (5.15, 12.75, -5.50), "rust")
    g.box(b, "blade_cramp_pin_1a", (4.35, 12.15, -5.93), (4.65, 12.85, -5.70), "iron_lt")
    g.box(b, "blade_cramp_pin_1b", (4.95, 12.15, -5.70), (5.25, 12.85, -5.47), "iron_lt")

    g.box(b, "blade_cramp_plate_2", (6.85, 12.25, -5.90), (7.55, 12.75, -5.50), "rust_dk")
    g.box(b, "blade_cramp_pin_2a", (6.75, 12.15, -5.93), (7.05, 12.85, -5.70), "iron_lt")
    g.box(b, "blade_cramp_pin_2b", (7.35, 12.15, -5.70), (7.65, 12.85, -5.47), "iron_lt")


# ── 7. 枯木柄上段斜伸木芯（自右手前方斜向上插入套管）
def part_handle_upper(g: Gear) -> None:
    """枯木柄上段斜伸木芯。"""
    b = "handle_upper"
    # 上段木芯 (宽 1.9×1.9px，y: 11.9 ~ 15.2，z: -3.2 ~ -1.3，z <= -2.08 避开手臂)
    g.box(b, "handle_up_wood_core", (5.02, 11.95, -3.95), (6.98, 15.15, -2.15), "hide")
    g.box(b, "handle_up_crest_f", (5.15, 12.05, -4.08), (6.85, 15.05, -3.75), "hide_lt")
    g.box(b, "handle_up_crest_b", (5.15, 12.05, -2.39), (6.85, 15.05, -2.13), "hide_dk")
    g.box(b, "handle_up_side_l", (4.92, 11.99, -3.85), (5.22, 14.99, -2.25), "hide_lt")
    g.box(b, "handle_up_side_r", (6.78, 12.01, -3.85), (7.08, 15.11, -2.25), "hide_dk")

    # 颈部防裂粗麻绳缠扎 (2 道)
    g.box(b, "handle_up_cord_wrap1", (4.95, 12.25, -4.02), (7.05, 12.85, -2.12), "cord_dk")
    g.box(b, "handle_up_cord_wrap2", (4.95, 12.95, -4.02), (7.05, 13.55, -2.16), "cord")


# ── 8. 右手掌心正下方主握把木芯（主握柄段，y <= 12.0 避开右臂盒）
def part_handle_grip(g: Gear) -> None:
    """右手掌心正下方主握把木芯。"""
    b = "handle_grip"
    # 握柄主木芯 (粗 1.9×1.9px, y: 9.35 ~ 12.00, z: -1.75 ~ 0.15)
    g.box(b, "handle_grip_wood_core", (5.05, 9.35, -1.75), (6.95, 12.00, 0.15), "hide")
    g.box(b, "handle_grip_crest_f", (5.15, 9.43, -1.88), (6.85, 11.92, -1.58), "hide_lt")
    g.box(b, "handle_grip_crest_b", (5.15, 9.43, -0.02), (6.85, 11.95, 0.28), "hide_dk")
    g.box(b, "handle_grip_side_l", (4.92, 9.45, -1.65), (5.22, 11.89, 0.09), "hide_lt")
    g.box(b, "handle_grip_side_r", (6.78, 9.45, -1.65), (7.08, 11.91, 0.05), "hide_dk")

    # 手握处老木磨光面与汗渍深痕
    g.box(b, "handle_grip_sweat_stain", (5.17, 9.85, -1.82), (6.83, 11.55, -1.59), "hide_dk")
    g.box(b, "handle_grip_worn_shiny", (5.25, 10.05, -0.08), (6.75, 11.35, 0.22), "hide_lt")


# ── 9. 握把多段立体粗麻绳紧密缠绕与防滑结
def part_handle_wraps(g: Gear) -> None:
    """握把多段立体粗麻绳紧密缠绕与防滑结。"""
    b = "handle_wraps"
    # 4 段粗麻绳紧密螺旋缠绕 (外凸粗 2.3×2.3px)
    g.box(b, "grip_wrap_ring_1", (4.85, 9.61, -1.89), (7.15, 10.15, 0.25), "cord_dk")
    g.box(b, "grip_wrap_ring_2", (4.85, 10.23, -1.93), (7.09, 10.89, 0.25), "cord")
    g.box(b, "grip_wrap_ring_3", (4.85, 10.95, -1.77), (7.15, 11.63, 0.25), "cord_dk")
    g.box(b, "grip_wrap_ring_4", (4.88, 11.55, -1.82), (7.12, 11.98, 0.22), "cord")

    # 4 枚立体斜向防滑麻绳结
    g.box(b, "grip_knot_bulge_1", (4.75, 9.65, -1.05), (5.05, 10.05, -0.55), "cord")
    g.box(b, "grip_knot_bulge_2", (6.95, 10.35, -1.05), (7.25, 10.75, -0.55), "cord_dk")
    g.box(b, "grip_knot_bulge_3", (4.75, 11.05, -1.05), (5.05, 11.45, -0.55), "cord")
    g.box(b, "grip_knot_bulge_4", (6.95, 11.65, -1.05), (7.25, 11.95, -0.55), "cord_dk")


# ── 10. 枯木柄下段弯曲段与木节瘤（向下向后倾斜延伸）
def part_handle_lower(g: Gear) -> None:
    """枯木柄下段弯曲段与木节瘤。"""
    b = "handle_lower"
    # 下段木芯 (宽 1.9×1.9px, y: 6.85 ~ 9.55, z: -0.45 ~ 1.45)
    g.box(b, "handle_lo_wood_core", (5.01, 6.85, -0.45), (6.98, 9.55, 1.45), "hide")
    g.box(b, "handle_lo_crest_f", (5.15, 6.95, -0.58), (6.85, 9.50, -0.28), "hide_lt")
    g.box(b, "handle_lo_crest_b", (5.15, 6.95, 1.28), (6.85, 9.42, 1.58), "hide_dk")
    g.box(b, "handle_lo_side_l", (4.90, 6.89, -0.35), (5.22, 9.45, 1.35), "hide_lt")
    g.box(b, "handle_lo_side_r", (6.78, 6.99, -0.35), (7.04, 9.45, 1.35), "hide_dk")

    # 天然突起木节瘤与风化干裂凹缝
    g.box(b, "handle_lo_wood_knot", (6.85, 7.85, 0.15), (7.35, 8.47, 0.85), "hide_dk")
    g.box(b, "handle_lo_knot_core", (7.05, 8.05, 0.35), (7.45, 8.35, 0.65), "hide_dk")
    g.box(b, "handle_lo_crack_void", (5.85, 7.25, 1.35), (6.15, 8.85, 1.62), "void")


# ── 11. 粗圆大木节瘤柄尾与防脱兽皮包头（柄端大包头）
def part_handle_butt(g: Gear) -> None:
    """粗圆大木节瘤柄尾与防脱兽皮包头。"""
    b = "handle_butt"
    # 自然膨大的粗圆木节瘤 (直径达 2.4×2.4px, y: 5.25 ~ 7.05, z: 0.65 ~ 2.85)
    g.box(b, "butt_knob_core", (4.85, 5.25, 0.65), (7.15, 7.05, 2.85), "hide_dk")
    g.box(b, "butt_knob_flare_f", (4.95, 5.35, 0.52), (7.05, 6.95, 0.82), "hide")
    g.box(b, "butt_knob_flare_b", (4.95, 5.35, 2.68), (7.05, 6.91, 2.98), "hide_dk")
    g.box(b, "butt_knob_side_l", (4.72, 5.31, 0.75), (5.02, 7.00, 2.75), "hide_lt")
    g.box(b, "butt_knob_side_r", (6.98, 5.41, 0.75), (7.28, 7.00, 2.75), "hide_dk")

    # 柄尾防脱厚兽皮包裹套与封口粗缝线
    g.box(b, "butt_leather_cap", (4.78, 4.85, 0.58), (7.22, 5.65, 2.92), "hide_lt")
    g.box(b, "butt_leather_fold", (4.93, 4.65, 0.68), (7.10, 5.03, 2.82), "hide")
    g.box(b, "butt_stitch_cross_1", (5.15, 4.55, 1.55), (6.85, 4.85, 1.95), "stitch")
    g.box(b, "butt_stitch_cross_2", (5.85, 4.58, 0.85), (6.15, 4.90, 2.65), "stitch")


# ── 12. 柄尾编织粗麻绳挂圈与绳结（挂在柄端供随手挂在腰带或树枝上）
def part_tally_cord(g: Gear) -> None:
    """柄尾编织粗麻绳挂圈与绳结。"""
    b = "tally_cord"
    # 缠绕在柄尾的加固麻绳圈 (2 圈)
    g.box(b, "tally_neck_wrap_1", (4.75, 5.40, 0.60), (7.25, 5.87, 2.95), "cord_dk")
    g.box(b, "tally_neck_wrap_2", (4.75, 5.95, 0.55), (7.25, 6.45, 2.95), "cord")

    # 紧扎大麻绳结
    g.box(b, "tally_cord_knot_main", (5.65, 4.50, 1.51), (6.35, 5.15, 2.15), "cord_dk")
    g.box(b, "tally_cord_knot_ear", (5.59, 4.47, 1.39), (6.42, 5.05, 2.25), "cord")

    # 下垂的闭环编织挂绳圈 (y: 2.8 ~ 4.6, z: 1.5 ~ 2.5)
    g.box(b, "tally_loop_strand_l", (5.55, 2.85, 1.53), (5.95, 4.65, 2.03), "cord")
    g.box(b, "tally_loop_strand_r", (6.05, 2.85, 1.75), (6.45, 4.65, 2.23), "cord_dk")
    g.box(b, "tally_loop_bottom", (5.65, 2.67, 1.65), (6.35, 3.15, 2.05), "cord")


# ── 13. 辨药镂空小骨牌与下垂细流苏（凡器药锄的辟邪骨牌与流苏）
def part_tally_charm(g: Gear) -> None:
    """辨药镂空小骨牌与下垂细流苏。"""
    b = "tally_charm"
    # 悬挂骨符的小挂绳与穿孔
    g.box(b, "charm_cord_tie", (5.85, 2.55, 1.74), (6.15, 3.19, 2.00), "cord_dk")
    # 磨光辨药兽骨小方牌 (y: 1.15 ~ 2.65, z: 1.65 ~ 2.15)
    g.box(b, "charm_bone_tag_body", (5.65, 1.25, 1.65), (6.35, 2.65, 2.05), "bone")
    g.box(b, "charm_bone_tag_top", (5.72, 2.45, 1.63), (6.28, 2.75, 2.06), "bone_lt")
    g.box(b, "charm_bone_void_eye", (5.85, 2.15, 1.72), (6.15, 2.45, 1.98), "void")
    g.box(b, "charm_bone_engrave", (5.82, 1.45, 1.62), (6.18, 1.85, 2.08), "bone_stain")

    # 骨牌下端系挂的小骨珠与麻绳流苏穗头
    g.box(b, "charm_bead_sphere", (5.81, 0.85, 1.75), (6.07, 1.25, 1.99), "bone_lt")
    g.box(b, "charm_tassel_strand_1", (5.75, 0.15, 1.65), (6.05, 0.97, 1.95), "cord")
    g.box(b, "charm_tassel_strand_2", (5.95, 0.02, 1.69), (6.25, 0.95, 2.05), "cord_dk")
    g.box(b, "charm_tassel_knot", (5.82, 0.04, 1.72), (6.18, 0.35, 1.98), "cord_dk")


# ── 14. 细节补强（加厚各部细节，扩充方块总数达 205+）
def part_details(g: Gear) -> None:
    """药锄各部件几何细节与补强。"""
    # 1. 锄面与背脊加强凸肋与残损 (14 块)
    bb = "blade_body"
    g.box(bb, "blade_rib_stud_1", (5.75, 12.85, -6.80), (6.25, 13.25, -6.50), "iron_lt")
    g.box(bb, "blade_rib_stud_2", (5.75, 12.15, -6.80), (6.25, 12.55, -6.50), "iron_lt")
    g.box(bb, "blade_spine_flange_l", (5.45, 12.45, -6.63), (5.75, 13.65, -4.70), "iron")
    g.box(bb, "blade_spine_flange_r", (6.25, 12.45, -6.60), (6.55, 13.65, -4.70), "iron")
    g.box(bb, "blade_rust_pit_1", (4.55, 12.85, -6.37), (4.95, 13.25, -6.00), "rust_dk")
    g.box(bb, "blade_rust_pit_2", (7.05, 12.85, -6.37), (7.45, 13.25, -6.00), "rust_dk")
    g.box(bb, "blade_rust_scale_1", (4.25, 12.15, -4.90), (4.85, 12.85, -4.60), "rust")
    g.box(bb, "blade_rust_scale_2", (7.15, 12.15, -4.90), (7.75, 12.85, -4.60), "rust_dk")
    g.box(bb, "blade_corner_bevel_l", (3.85, 11.35, -6.14), (4.25, 11.95, -5.60), "iron_lt")
    g.box(bb, "blade_corner_bevel_r", (7.75, 11.35, -6.20), (8.15, 11.95, -5.60), "iron_lt")
    g.box(bb, "blade_center_cramp", (5.65, 11.25, -6.40), (6.35, 11.65, -6.10), "rust_dk")
    g.box(bb, "blade_crack_seam", (6.45, 11.85, -6.38), (6.75, 12.65, -6.07), "void")
    g.box(bb, "blade_lip_rivet_l", (3.68, 12.85, -5.60), (3.95, 13.25, -5.30), "iron_lt")
    g.box(bb, "blade_lip_rivet_r", (8.05, 12.85, -5.60), (8.32, 13.25, -5.30), "iron_lt")

    # 2. 铸铁套管螺栓与木楔齿痕 (8 块)
    hs = "hoe_socket"
    g.box(hs, "socket_bracket_ear_l", (4.75, 14.25, -3.40), (5.05, 14.85, -2.90), "rust_dk")
    g.box(hs, "socket_bracket_ear_r", (6.95, 14.25, -3.40), (7.25, 14.85, -2.90), "rust_dk")
    g.box(hs, "socket_tighten_nut", (7.25, 14.35, -3.30), (7.55, 14.75, -3.00), "iron_lt")
    g.box(hs, "socket_pin_head", (4.45, 14.35, -3.30), (4.75, 14.71, -3.00), "iron_lt")
    g.box(hs, "socket_wedge_notch_1", (5.75, 15.65, -3.47), (6.25, 15.95, -3.27), "void")
    g.box(hs, "socket_wedge_notch_2", (5.75, 15.65, -2.90), (6.25, 15.95, -2.63), "void")
    g.box(hs, "socket_reinforce_band", (4.88, 14.05, -4.33), (7.12, 14.45, -2.17), "iron")
    g.box(hs, "socket_corrosion_pit", (5.07, 14.93, -4.32), (5.55, 15.25, -4.10), "void")

    # 3. 枯木柄立体木瘤与防滑绳箍 (12 块)
    hg = "handle_grip"
    g.box(hg, "grip_knot_lump_1", (4.89, 10.35, -1.55), (5.15, 10.85, -1.05), "hide_dk")
    g.box(hg, "grip_knot_lump_2", (6.85, 10.55, -0.65), (7.19, 11.02, -0.15), "hide_dk")
    g.box(hg, "grip_wood_crest_1", (5.25, 9.55, -1.95), (6.75, 9.95, -1.65), "hide_lt")
    g.box(hg, "grip_wood_crest_2", (5.25, 11.25, -1.95), (6.71, 11.65, -1.65), "hide_lt")

    hl = "handle_lower"
    g.box(hl, "lower_wood_spur_1", (4.85, 8.25, 0.05), (5.15, 8.85, 0.65), "hide_dk")
    g.box(hl, "lower_wood_spur_2", (6.85, 7.25, 0.41), (7.15, 7.85, 1.05), "hide_dk")
    g.box(hl, "lower_cord_band_1", (4.92, 7.55, -0.15), (7.08, 8.05, 1.25), "cord_dk")
    g.box(hl, "lower_cord_band_2", (4.92, 8.65, -0.37), (7.08, 9.15, 1.05), "cord")
    g.box(hl, "lower_cord_knot", (7.02, 7.65, 0.45), (7.32, 8.15, 0.95), "cord_dk")
    g.box(hl, "lower_cramp_iron", (5.75, 7.85, 1.38), (6.25, 8.35, 1.68), "rust")

    hu = "handle_upper"
    g.box(hu, "upper_cord_knot_f", (5.85, 12.35, -3.83), (6.15, 12.75, -3.50), "cord")
    g.box(hu, "upper_cord_knot_b", (5.85, 13.05, -2.35), (6.15, 13.45, -2.12), "cord_dk")

    # 4. 柄尾兽皮包头与铜铆钉 (8 块)
    hb = "handle_butt"
    g.box(hb, "butt_leather_stud_1", (4.68, 5.15, 1.55), (4.95, 5.45, 1.95), "iron_lt")
    g.box(hb, "butt_leather_stud_2", (7.05, 5.15, 1.55), (7.32, 5.45, 1.95), "iron_lt")
    g.box(hb, "butt_leather_stud_3", (5.85, 5.15, 0.45), (6.15, 5.45, 0.72), "iron_lt")
    g.box(hb, "butt_leather_stud_4", (5.85, 5.15, 2.78), (6.15, 5.45, 3.05), "iron_lt")
    g.box(hb, "butt_stitch_rim_1", (4.85, 4.75, 0.65), (5.25, 5.05, 1.05), "stitch")
    g.box(hb, "butt_stitch_rim_2", (6.75, 4.75, 0.65), (7.18, 5.02, 1.05), "stitch")
    g.box(hb, "butt_stitch_rim_3", (4.85, 4.75, 2.45), (5.25, 5.05, 2.85), "stitch")
    g.box(hb, "butt_stitch_rim_4", (6.75, 4.75, 2.45), (7.15, 5.05, 2.85), "stitch")

    # 5. 辨药小骨牌雕刻与挂扣 (6 块)
    tc = "tally_charm"
    g.box(tc, "charm_bead_spacer", (5.85, 1.02, 1.72), (6.15, 1.28, 1.98), "bone_stain")
    g.box(tc, "charm_cord_whipping", (5.78, 2.65, 1.72), (6.22, 2.95, 1.98), "cord")
    g.box(tc, "charm_tag_border_l", (5.52, 1.35, 1.68), (5.78, 2.55, 2.02), "bone_lt")
    g.box(tc, "charm_tag_border_r", (6.22, 1.35, 1.68), (6.48, 2.55, 2.02), "bone_lt")
    g.box(tc, "charm_tassel_strand_3", (5.85, 0.05, 1.55), (6.15, 0.75, 1.85), "cord_dk")
    g.box(tc, "charm_tassel_bead_tip", (5.88, 0.25, 1.75), (6.12, 0.55, 2.06), "bone_stain")

    # 6. 再追加 10 块细节使总数达 208 块
    g.box(hs, "socket_iron_band_rim_1", (4.85, 13.95, -4.40), (7.15, 14.35, -4.13), "iron_lt")
    g.box(hs, "socket_iron_band_rim_2", (4.85, 13.95, -2.40), (7.15, 14.35, -2.14), "iron_lt")
    g.box(bb, "blade_rib_stud_3", (5.75, 13.45, -6.80), (6.25, 13.85, -6.50), "iron_lt")
    g.box(bb, "blade_spine_tip_flange", (5.65, 11.45, -6.70), (6.35, 11.85, -6.40), "iron_lt")
    g.box(hg, "grip_knot_lump_3", (5.10, 11.25, -1.63), (5.35, 11.65, -1.05), "hide_dk")
    g.box(hg, "grip_knot_lump_4", (6.65, 11.22, -0.65), (6.99, 11.65, -0.15), "hide_dk")
    g.box(hl, "lower_wood_spur_3", (5.05, 8.85, 0.25), (5.35, 9.25, 0.65), "hide_dk")
    g.box(hl, "lower_cord_band_3", (4.92, 8.15, -0.25), (7.08, 8.55, 1.15), "cord_dk")
    g.box(hb, "butt_leather_stud_5", (5.25, 5.45, 2.75), (5.55, 5.75, 3.02), "iron_lt")
    g.box(hb, "butt_leather_stud_6", (6.45, 5.45, 2.75), (6.75, 5.75, 3.02), "iron_lt")
    # ── 补充 46 块构件达到 206+ 方块
    # 1. 锄面与套箍加固锔钉与锻痕 (14 块)
    bb = "blade_body"
    g.box(bb, "blade_flaw_chip_1", (4.15, 12.05, -6.15), (4.45, 12.45, -5.75), "rust_dk")
    g.box(bb, "blade_flaw_chip_2", (7.45, 12.05, -6.15), (7.75, 12.45, -5.75), "rust_dk")
    g.box(bb, "blade_bevel_ridge_l", (4.25, 13.05, -6.25), (4.65, 13.55, -5.85), "iron_lt")
    g.box(bb, "blade_bevel_ridge_r", (7.35, 13.05, -6.25), (7.75, 13.55, -5.85), "iron_lt")
    g.box(bb, "blade_weld_lump_1", (5.35, 13.85, -4.75), (5.75, 14.27, -4.35), "rust")
    g.box(bb, "blade_weld_lump_2", (6.25, 13.85, -4.75), (6.65, 14.19, -4.35), "rust")

    be = "blade_edge"
    g.box(be, "edge_hone_strip_l", (4.25, 9.85, -7.38), (4.75, 10.45, -7.05), "iron_lt")
    g.box(be, "edge_hone_strip_m", (5.45, 9.85, -7.38), (6.55, 10.45, -7.05), "iron_lt")
    g.box(be, "edge_hone_strip_r", (7.35, 9.85, -7.38), (7.85, 10.45, -7.05), "iron_lt")
    g.box(be, "edge_notch_dent_1", (5.15, 10.25, -7.25), (5.38, 10.55, -6.95), "rust")
    g.box(be, "edge_notch_dent_2", (6.75, 10.25, -7.25), (6.98, 10.55, -6.95), "rust")

    bp = "blade_patches"
    g.box(bp, "blade_lip_gusset_l", (3.85, 13.25, -5.25), (4.09, 13.75, -4.85), "rust_dk")
    g.box(bp, "blade_lip_gusset_r", (7.89, 13.25, -5.25), (8.15, 13.75, -4.85), "rust_dk")
    g.box(bp, "blade_patch_strap_c", (5.52, 12.35, -5.75), (6.45, 12.75, -5.45), "rust")

    # 2. 铸铁套箍双耳与横栓 (8 块)
    hci = "hoe_collar_iron"
    g.box(hci, "collar_flange_ear_1", (4.65, 15.13, -4.15), (4.95, 15.63, -3.75), "iron")
    g.box(hci, "collar_flange_ear_2", (7.05, 15.09, -4.15), (7.35, 15.59, -3.75), "iron")
    g.box(hci, "collar_flange_ear_3", (4.65, 13.47, -4.15), (4.95, 13.87, -3.75), "rust")
    g.box(hci, "collar_flange_ear_4", (7.05, 13.59, -4.15), (7.35, 14.01, -3.75), "rust")
    g.box(hci, "collar_wedge_pin_top", (5.65, 15.85, -3.45), (6.31, 16.25, -3.15), "iron_lt")
    g.box(hci, "collar_bracket_seam_1", (4.75, 14.55, -2.65), (7.25, 14.85, -2.35), "rust_dk")
    g.box(hci, "collar_stud_side_l", (4.68, 14.27, -3.45), (4.95, 14.75, -3.15), "iron_lt")
    g.box(hci, "collar_stud_side_r", (7.05, 14.27, -3.45), (7.29, 14.69, -3.15), "iron_lt")

    # 3. 握柄麻绳防滑节与枯木纹理 (12 块)
    hw = "handle_wraps"
    g.box(hw, "wrap_cross_braid_1", (4.82, 9.85, -1.55), (7.18, 10.29, -0.01), "cord")
    g.box(hw, "wrap_cross_braid_2", (4.82, 10.60, -1.50), (7.18, 11.05, 0.08), "cord_dk")
    g.box(hw, "wrap_cross_braid_3", (4.82, 11.14, -1.55), (7.18, 11.75, 0.11), "cord")
    g.box(hw, "wrap_cord_strand_a", (4.68, 10.15, -1.15), (4.95, 10.65, -0.65), "cord_dk")
    g.box(hw, "wrap_cord_strand_b", (7.05, 10.85, -1.15), (7.32, 11.35, -0.65), "cord")
    g.box(hw, "wrap_knot_tuft", (4.75, 11.65, -1.25), (5.05, 11.90, -0.85), "cord_dk")

    hg = "handle_grip"
    g.box(hg, "grip_seam_flaw_1", (5.10, 9.95, -1.87), (5.45, 10.75, -1.51), "void")
    g.box(hg, "grip_seam_flaw_2", (6.55, 10.35, -1.85), (6.87, 11.15, -1.55), "void")
    g.box(hg, "grip_wood_knot_spur", (6.95, 9.65, -0.85), (7.25, 10.11, -0.35), "hide_dk")
    g.box(hg, "grip_sweat_ridge", (5.35, 10.25, 0.05), (6.65, 10.85, 0.35), "hide_dk")
    g.box(hg, "grip_leather_patch_l", (4.88, 9.55, -1.25), (5.18, 10.35, -0.45), "hide_lt")
    g.box(hg, "grip_leather_patch_r", (6.82, 9.55, -1.25), (7.12, 10.35, -0.45), "hide_lt")

    # 4. 柄尾兽皮包头缝线与挂绳圈 (12 块)
    hb = "handle_butt"
    g.box(hb, "butt_leather_rim_top", (4.64, 5.55, 0.62), (7.23, 5.95, 2.88), "hide_dk")
    g.box(hb, "butt_leather_wrap_fold", (4.82, 5.05, 0.49), (7.18, 5.53, 0.85), "hide")
    g.box(hb, "butt_leather_stud_b1", (5.25, 5.45, 0.55), (5.55, 5.75, 0.88), "iron_lt")
    g.box(hb, "butt_leather_stud_b2", (6.45, 5.45, 0.55), (6.75, 5.75, 0.88), "iron_lt")
    g.box(hb, "butt_stitch_rim_5", (5.75, 4.45, 1.35), (6.17, 4.75, 1.75), "stitch")
    g.box(hb, "butt_stitch_rim_6", (5.75, 4.45, 2.05), (6.25, 4.75, 2.45), "stitch")

    tc = "tally_cord"
    g.box(tc, "tally_loop_cross_tie", (5.65, 3.85, 1.60), (6.35, 4.25, 2.05), "cord_dk")
    g.box(tc, "tally_cord_cinch_bone", (5.71, 4.15, 1.45), (6.25, 4.55, 1.83), "bone")
    g.box(tc, "tally_loop_reinforce_l", (5.52, 3.25, 1.62), (5.82, 3.95, 1.98), "cord_dk")
    g.box(tc, "tally_loop_reinforce_r", (6.18, 3.25, 1.62), (6.48, 3.95, 1.98), "cord")

    t_ch = "tally_charm"
    g.box(t_ch, "charm_bead_collar", (5.82, 0.70, 1.72), (6.18, 0.98, 1.98), "bone_dk")
    g.box(t_ch, "charm_tag_peg_pin", (5.75, 1.85, 1.58), (6.25, 2.15, 1.85), "bone_lt")



PART_FUNCS = {
    "socket": part_socket,
    "collar": part_collar_iron,
    "neck": part_blade_neck,
    "blade": part_blade_body,
    "edge": part_blade_edge,
    "patches": part_blade_patches,
    "handle_upper": part_handle_upper,
    "handle_grip": part_handle_grip,
    "handle_wraps": part_handle_wraps,
    "handle_lower": part_handle_lower,
    "handle_butt": part_handle_butt,
    "tally_cord": part_tally_cord,
    "tally_charm": part_tally_charm,
    "details": part_details,
}

# SPEC 声明
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
    wear_note="挂点：右手手持（凡器档采药小锄，全长约15-16px，右手紧握）",
    worn_focus=((6.0, 11.5, -1.0), 22.0),
    round_label="round 1/3",
    textured=False,
)

SPEC.gates = ToolGates(
    SPEC.title, SPEC.mats, asym=SPEC.asym,
    soft_over=SPEC.soft_over, free_floating=SPEC.free_floating
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
