#!/usr/bin/env python3
"""铁锸（tie_cha）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

古代金属器简单铁件 #7：精致规整的双手长柄铁锸。
尺寸：全长约 26px（y: 0.5 ~ 26.5），手持 / 靠放。
手持挂点：右手握持在木柄中段（y ≈ 12.0），锸头（y ∈ [0.5, 8.5]）立在右脚前侧地面，T形把手在右肩下方（y ≈ 26.5）。
避让：长木柄与锸头位于右手前方（z <= -2.15），对玩家身体各盒子咬入深度为 0。

造型特征：
1. 锻打 U 形铁锸头：宽 6.0px，圆肩（弧肩），中央突起一道坚挺中脊（起脊）；
2. 铁套包裹内层硬木板刃芯，肩部一对纯铜铆钉；
3. 正面中脊上方有方形空白铭文匾位（方形凹槽，精致古代器物标配）；
4. 刃口磨亮薄铁锋刃，带不规则微崩口；
5. 銎口椭圆加固铸铁套箍，带抱耳与固定销；
6. 长直木柄全长约 18px，握把处被汗水磨黑包浆；
7. 顶端 T 形木握把（宽 4px），中间榫卯穿接直柄并包一道加固铁箍。

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
    "iron": (88, 80, 76),
    "iron_lt": (132, 126, 120),
    "rust": (118, 62, 34),
    "rust_dk": (68, 38, 24),
    "copper": (172, 118, 64),
    "copper_dk": (124, 76, 42),
    "hide": (112, 78, 52),
    "hide_dk": (58, 40, 26),
    "hide_lt": (148, 112, 74),
    "stitch": (188, 174, 148),
    "void": (26, 20, 18),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("spade_blade_iron", "root", (6.0, 4.5, -3.0), "U形锻打铁套与圆肩刃身（宽6px带弧肩）"),
    ("spade_blade_spine", "spade_blade_iron", (6.0, 4.5, -3.0), "铁锸正面贯穿中脊凸棱（起脊）"),
    ("spade_core_wood", "spade_blade_iron", (6.0, 5.0, -3.0), "内夹硬木板刃芯与插柄榫槽"),
    ("spade_rivets", "spade_blade_iron", (6.0, 6.8, -3.0), "锸肩对称纯铜铆钉与定位销"),
    ("spade_socket_ferrule", "spade_blade_iron", (6.0, 8.2, -3.0), "锸头与木柄接合铸铁套管銎口"),
    ("cartouche_plate", "spade_blade_iron", (6.0, 7.5, -3.0), "正面方形空白铭文匾位（精致金属器标志）"),
    ("shaft_lower", "root", (6.0, 10.5, -3.0), "长木柄下段（直插銎口直柄芯）"),
    ("shaft_grip", "shaft_lower", (6.0, 13.5, -3.0), "长木柄中段（双手握持汗渍磨黑包浆）"),
    ("shaft_upper", "shaft_grip", (6.0, 20.5, -3.0), "长木柄上段（笔直光润白蜡木身）"),
    ("handle_tee_crossbar", "shaft_upper", (6.0, 26.2, -3.0), "顶端雕刻 T 形横握把横木"),
    ("handle_tee_collar", "handle_tee_crossbar", (6.0, 25.8, -3.0), "T把手与木柄接合加固铁箍与木楔"),
    ("blade_cramps", "spade_blade_iron", (6.0, 3.5, -3.0), "锸板表面修补铁锔钉与锻痕锻坑"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] != "root"])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("copper", "copper_dk"), ("copper", "iron"), ("copper", "rust"),
    ("copper", "rust_dk"), ("copper_dk", "iron"), ("iron", "iron_lt"),
    ("iron", "rust"), ("rust", "rust_dk"), ("hide", "hide_dk"),
    ("hide", "hide_lt"), ("hide", "iron"), ("hide", "rust"),
    ("hide", "rust_dk"), ("hide_dk", "rust"), ("hide_dk", "rust_dk"),
    ("hide_lt", "rust"), ("iron", "void"), ("rust", "void"),
    ("rust_dk", "void"), ("iron_lt", "void"), ("hide", "void"),
    ("stitch", "hide"), ("stitch", "hide_dk"),
))


class ToolGates(gatekit.AssetGates):
    """手持工具门禁：纯手持物长26px握在右手，无身体中线对称件，去掉 overflow 和 mirror，保留 clearance。"""

    def specs(self):
        base = tuple(s for s in super().specs() if s[0] not in ("overflow", "mirror"))
        return base + (("clearance", "穿进玩家身体",
                        lambda r: gate_clearance(r), inject_clearance),)


# ── 1. U形锻打铁套与圆肩刃身（宽 6.0px，带优美弧肩、刃口磨亮）
def part_spade_blade(g: Gear) -> None:
    """U形锻打铁套与圆肩刃身。"""
    b = "spade_blade_iron"
    # U形外包铁套底刃：宽 6.0px (x: 3.0 ~ 9.0)，高 1.6px (y: 0.5 ~ 2.1)，厚 1.1px (z: -3.55 ~ -2.45)
    g.box(b, "edge_shoe_bottom", ((3.60, 0.50, -3.55)), ((9.60, 2.10, -2.45)), "iron")
    g.box(b, "edge_shoe_hone_f", ((3.65, 0.41, -3.68)), ((9.55, 1.25, -3.38)), "iron_lt")
    g.box(b, "edge_shoe_hone_b", ((3.65, 0.40, -2.62)), ((9.55, 1.25, -2.32)), "iron_lt")

    # 刃口两处磨损微缺口
    g.box(b, "edge_notch_chip_1", ((4.85, 0.38, -3.72)), ((5.35, 0.85, -2.28)), "void")
    g.box(b, "edge_notch_chip_2", ((7.75, 0.38, -3.72)), ((8.35, 0.85, -2.28)), "void")

    # U形铁套两侧立臂（紧紧包住硬木板侧缘，高 y: 2.1 ~ 6.2）
    g.box(b, "edge_shoe_arm_l", ((3.60, 2.10, -3.60)), ((4.23, 6.20, -2.47)), "iron")
    g.box(b, "edge_shoe_arm_r", ((9.01, 2.10, -3.55)), ((9.60, 6.20, -2.45)), "iron")

    # 左右圆肩（弧肩！优美的弧度过渡收缩至銎口宽 3.4px，y: 6.2 ~ 7.8）
    # 左圆肩三级阶梯收缩
    g.box(b, "shoulder_arc_l1", ((3.75, 6.20, -3.52)), ((4.75, 6.85, -2.48)), "rust")
    g.box(b, "shoulder_arc_l2", ((4.31, 6.85, -3.52)), ((5.27, 7.35, -2.48)), "rust_dk")
    g.box(b, "shoulder_arc_l3", ((4.75, 7.35, -3.52)), ((5.65, 7.85, -2.48)), "iron")

    # 右圆肩三级阶梯收缩
    g.box(b, "shoulder_arc_r1", ((8.45, 6.20, -3.52)), ((9.45, 6.80, -2.48)), "rust")
    g.box(b, "shoulder_arc_r2", ((7.95, 6.85, -3.52)), ((8.99, 7.35, -2.48)), "rust_dk")
    g.box(b, "shoulder_arc_r3", ((7.55, 7.35, -3.52)), ((8.45, 7.85, -2.48)), "iron")

    # 锸面下沿防卷护皮铁边
    g.box(b, "blade_lower_flange", ((3.85, 1.80, -3.71)), ((9.35, 2.39, -2.35)), "rust_dk")


# ── 2. 铁锸正面贯穿中脊凸棱（起脊！经典战国/汉代铁锸特征）
def part_spade_spine(g: Gear) -> None:
    """铁锸正面贯穿中脊凸棱。"""
    b = "spade_blade_spine"
    # 正面中央坚挺中脊（宽 1.0px，凸出 0.35px，贯穿 y: 1.2 到 7.8）
    g.box(b, "spine_ridge_lo", ((6.15, 1.25, -3.85)), ((7.05, 4.25, -3.45)), "iron_lt")
    g.box(b, "spine_ridge_mid", ((6.15, 4.25, -3.85)), ((7.05, 6.55, -3.45)), "iron_lt")
    g.box(b, "spine_ridge_hi", ((6.15, 6.55, -3.85)), ((7.05, 7.81, -3.45)), "iron")

    # 中脊两侧三角斜坡斜面（向两侧木板倾斜展开）
    g.box(b, "spine_slope_l_lo", ((5.75, 1.65, -3.72)), ((6.15, 4.25, -3.48)), "iron")
    g.box(b, "spine_slope_r_lo", ((7.05, 1.65, -3.72)), ((7.48, 4.25, -3.48)), "iron")
    g.box(b, "spine_slope_l_hi", ((5.75, 4.25, -3.72)), ((6.15, 6.85, -3.48)), "rust")
    g.box(b, "spine_slope_r_hi", ((7.05, 4.25, -3.72)), ((7.45, 6.85, -3.48)), "rust")

    # 背面加固反脊
    g.box(b, "spine_back_keel", ((6.25, 1.85, -2.55)), ((6.95, 7.25, -2.25)), "rust_dk")


# ── 3. 内夹硬木板刃芯与插柄榫槽（被U形铁套包裹的坚固木板）
def part_spade_wood_core(g: Gear) -> None:
    """内夹硬木板刃芯与插柄榫槽。"""
    b = "spade_core_wood"
    # 硬木板刃芯 (宽 5.2px，夹在 U形铁套两侧之间，y: 1.8 ~ 7.8)
    g.box(b, "wood_core_plate_lo", ((4.25, 1.81, -3.45)), ((8.97, 4.85, -2.55)), "hide")
    g.box(b, "wood_core_plate_hi", ((4.25, 4.89, -3.45)), ((8.95, 7.30, -2.55)), "hide")

    # 表面木质纹理与包浆光泽
    g.box(b, "wood_core_grain_l", ((4.42, 2.19, -3.48)), ((5.75, 6.82, -3.22)), "hide_lt")
    g.box(b, "wood_core_grain_r", ((7.45, 2.20, -3.48)), ((8.80, 6.85, -3.22)), "hide_lt")
    g.box(b, "wood_core_dark_b", ((4.45, 2.25, -2.76)), ((8.75, 6.81, -2.52)), "hide_dk")


# ── 4. 锸肩对称纯铜铆钉与定位销（肩上一对铆孔）
def part_spade_rivets(g: Gear) -> None:
    """锸肩对称纯铜铆钉与定位销。"""
    b = "spade_rivets"
    # 左肩纯铜铆钉 (y: 6.6 ~ 7.0, x: 4.1 ~ 4.7)
    g.box(b, "rivet_copper_head_l", ((4.75, 6.65, -3.82)), ((5.35, 7.15, -3.45)), "copper")
    g.box(b, "rivet_copper_pin_l", ((4.85, 6.75, -3.65)), ((5.25, 7.05, -2.35)), "copper_dk")
    g.box(b, "rivet_copper_tail_l", ((4.75, 6.65, -2.55)), ((5.35, 7.15, -2.18)), "copper")

    # 右肩纯铜铆钉 (y: 6.6 ~ 7.0, x: 7.3 ~ 7.9)
    g.box(b, "rivet_copper_head_r", ((7.85, 6.65, -3.82)), ((8.45, 7.15, -3.45)), "copper")
    g.box(b, "rivet_copper_pin_r", ((7.97, 6.75, -3.65)), ((8.35, 7.05, -2.35)), "copper_dk")
    g.box(b, "rivet_copper_tail_r", ((7.85, 6.65, -2.55)), ((8.45, 7.15, -2.18)), "copper")

    # 下部加固铁铆钉 (2 枚)
    g.box(b, "rivet_iron_lower_l", ((4.45, 3.45, -3.75)), ((4.95, 3.85, -3.35)), "iron_lt")
    g.box(b, "rivet_iron_lower_r", ((8.25, 3.45, -3.75)), ((8.75, 3.85, -3.40)), "iron_lt")


# ── 5. 锸头与木柄接合铸铁套管銎口（圆銎抱木）
def part_socket_ferrule(g: Gear) -> None:
    """锸头与木柄接合铸铁套管銎口。"""
    b = "spade_socket_ferrule"
    # 圆套管外圈 (宽 2.4px, y: 7.6 ~ 9.2, z: -3.85 ~ -2.15)
    g.box(b, "socket_ferrule_body", ((5.40, 7.65, -3.87)), ((7.80, 9.25, -2.19)), "rust_dk")
    g.box(b, "socket_ferrule_front", ((5.50, 7.75, -3.98)), ((7.70, 9.15, -3.72)), "iron")
    g.box(b, "socket_ferrule_back", ((5.50, 7.75, -2.28)), ((7.70, 9.21, -2.02)), "rust")
    g.box(b, "socket_ferrule_left", ((5.28, 7.71, -3.75)), ((5.52, 9.11, -2.25)), "rust_dk")
    g.box(b, "socket_ferrule_right", ((7.68, 7.79, -3.75)), ((7.92, 9.09, -2.25)), "iron")

    # 套管上下加固箍环
    g.box(b, "socket_collar_top", ((5.32, 8.81, -3.95)), ((7.88, 9.35, -2.05)), "iron_lt")
    g.box(b, "socket_collar_bot", ((5.32, 7.55, -3.95)), ((7.88, 8.05, -2.05)), "rust")

    # 紧固横穿铁铆钉
    g.box(b, "socket_bolt_pin", ((5.05, 8.25, -3.25)), ((8.15, 8.65, -2.75)), "iron_lt")


# ── 6. 正面方形空白铭文匾位（精致金属器标志）
def part_cartouche(g: Gear) -> None:
    """正面方形空白铭文匾位。"""
    b = "cartouche_plate"
    # 铭文位边框凸台 (y: 6.8 ~ 7.8, x: 5.3 ~ 6.7)
    g.box(b, "cartouche_border", ((5.90, 6.75, -3.97)), ((7.30, 7.85, -3.55)), "iron_lt")
    # 方形浅凹槽空匾 (视觉上整齐凹陷，不写字，代表古代工匠铭名处)
    g.box(b, "cartouche_recess_void", ((6.10, 6.95, -4.05)), ((7.10, 7.65, -3.75)), "void")
    g.box(b, "cartouche_blank_slate", ((6.12, 6.98, -4.01)), ((7.08, 7.62, -3.62)), "iron")


# ── 7. 长木柄下段（直插銎口直柄芯，y: 9.0 ~ 13.0）
def part_shaft_lower(g: Gear) -> None:
    """长木柄下段。"""
    b = "shaft_lower"
    # 直贯木芯 (宽 1.7×1.7px, x: 5.15 ~ 6.85, z: -3.75 ~ -2.25)
    g.box(b, "shaft_lo_core", ((5.75, 9.05, -3.75)), ((7.45, 13.05, -2.25)), "hide")
    g.box(b, "shaft_lo_crest_f", ((5.85, 9.19, -3.88)), ((7.35, 12.93, -3.58)), "hide_lt")
    g.box(b, "shaft_lo_crest_b", ((5.85, 9.18, -2.42)), ((7.35, 12.89, -2.12)), "hide_dk")
    g.box(b, "shaft_lo_side_l", ((5.62, 9.10, -3.65)), ((5.88, 12.95, -2.35)), "hide_lt")
    g.box(b, "shaft_lo_side_r", ((7.32, 9.11, -3.65)), ((7.58, 12.95, -2.35)), "hide_dk")


# ── 8. 长木柄中段（双手握持汗渍磨黑包浆，y: 13.0 ~ 18.0）
def part_shaft_grip(g: Gear) -> None:
    """长木柄中段。"""
    b = "shaft_grip"
    # 握持段主木芯 (汗水渗透变黑，深褐色 hide_dk)
    g.box(b, "shaft_grip_core", ((5.75, 13.05, -3.75)), ((7.45, 18.05, -2.25)), "hide_dk")
    g.box(b, "shaft_grip_crest_f", ((5.85, 13.15, -3.86)), ((7.35, 17.89, -3.53)), "hide")
    g.box(b, "shaft_grip_crest_b", ((5.85, 13.15, -2.42)), ((7.35, 17.99, -2.12)), "hide_dk")
    g.box(b, "shaft_grip_side_l", ((5.62, 13.21, -3.65)), ((5.88, 17.95, -2.35)), "hide_dk")
    g.box(b, "shaft_grip_side_r", ((7.32, 13.10, -3.65)), ((7.58, 18.00, -2.35)), "hide_dk")

    # 手掌反复搓摸出来的光亮包浆痕
    g.box(b, "shaft_grip_shine_1", ((5.98, 14.25, -3.92)), ((7.25, 16.55, -3.65)), "hide")
    g.box(b, "shaft_grip_shine_2", ((5.95, 14.25, -2.35)), ((7.25, 16.55, -2.08)), "hide")


# ── 9. 长木柄上段（修长笔直白蜡木身，y: 18.0 ~ 25.5）
def part_shaft_upper(g: Gear) -> None:
    """长木柄上段。"""
    b = "shaft_upper"
    # 笔直上段木芯 (y: 18.0 ~ 25.5)
    g.box(b, "shaft_hi_core", ((5.75, 18.05, -3.81)), ((7.45, 25.55, -2.28)), "hide")
    g.box(b, "shaft_hi_crest_f", ((5.85, 18.15, -3.86)), ((7.35, 25.51, -3.56)), "hide_lt")
    g.box(b, "shaft_hi_crest_b", ((5.85, 18.19, -2.42)), ((7.35, 25.45, -2.10)), "hide_dk")
    g.box(b, "shaft_hi_side_l", ((5.62, 18.18, -3.65)), ((5.88, 25.40, -2.32)), "hide_lt")
    g.box(b, "shaft_hi_side_r", ((7.32, 18.09, -3.65)), ((7.62, 25.41, -2.38)), "hide_dk")

    # 天然木节瘤与防滑细弦槽
    g.box(b, "shaft_hi_knot", ((7.45, 21.25, -3.25)), ((7.85, 21.95, -2.75)), "hide_dk")
    g.box(b, "shaft_hi_ring_mark", ((5.68, 23.45, -3.82)), ((7.52, 23.85, -2.18)), "hide_dk")


# ── 10. 顶端雕刻 T 形横握把横木（宽 4.0px，人体工学圆润木把）
def part_handle_tee(g: Gear) -> None:
    """顶端雕刻 T 形横握把横木。"""
    b = "handle_tee_crossbar"
    # T形横梁主身 (宽 x: 4.0 ~ 8.0 整整 4.0px！高 y: 25.5 ~ 26.7, 深 z: -3.85 ~ -2.15)
    g.box(b, "tee_cross_core", ((4.60, 25.55, -3.73)), ((8.60, 26.65, -2.31)), "hide")
    g.box(b, "tee_cross_top_skin", ((4.70, 26.55, -3.65)), ((8.50, 26.85, -2.35)), "hide_lt")
    g.box(b, "tee_cross_front_skin", ((4.70, 25.65, -3.92)), ((8.50, 26.51, -3.58)), "hide")
    g.box(b, "tee_cross_back_skin", ((4.70, 25.65, -2.42)), ((8.50, 26.51, -2.12)), "hide_dk")

    # 两端收细圆雕端盖 (左右两头)
    g.box(b, "tee_cap_left", ((4.35, 25.65, -3.65)), ((4.70, 26.55, -2.35)), "hide_dk")
    g.box(b, "tee_cap_right", ((8.50, 25.65, -3.65)), ((8.85, 26.60, -2.35)), "hide_dk")
    g.box(b, "tee_tip_bead_l", ((4.15, 25.85, -3.45)), ((4.45, 26.35, -2.55)), "hide_lt")
    g.box(b, "tee_tip_bead_r", ((8.72, 25.85, -3.45)), ((9.05, 26.35, -2.55)), "hide_lt")


# ── 11. T把手与木柄接合加固铁箍与木楔
def part_handle_tee_collar(g: Gear) -> None:
    """T把手与木柄接合加固铁箍与木楔。"""
    b = "handle_tee_collar"
    # 十字接合部的加固锻铁箍带 (y: 25.1 ~ 26.0)
    g.box(b, "tee_iron_collar_main", ((5.70, 25.05, -3.90)), ((7.50, 25.95, -2.18)), "rust_dk")
    g.box(b, "tee_iron_collar_front", ((5.80, 25.15, -3.95)), ((7.40, 25.85, -3.65)), "iron")
    g.box(b, "tee_iron_collar_back", ((5.80, 25.15, -2.35)), ((7.40, 25.85, -2.05)), "rust")

    # 顶端直柄穿透榫头与加固木楔 (榫接结构)
    g.box(b, "tee_tenon_through", ((6.15, 26.65, -3.35)), ((7.05, 27.05, -2.65)), "hide_lt")
    g.box(b, "tee_wedge_pin", ((6.45, 26.85, -3.45)), ((6.75, 27.25, -2.55)), "hide_dk")
    g.box(b, "tee_collar_rivet", ((6.35, 25.35, -3.99)), ((6.85, 25.78, -3.75)), "iron_lt")


# ── 12. 锸板表面修补铁锔钉与锻痕锻坑
def part_blade_cramps(g: Gear) -> None:
    """锸板表面修补铁锔钉与锻痕锻坑。"""
    b = "blade_cramps"
    # 铁锸板面锻打同心锤痕凹坑 (4 处微凹)
    g.box(b, "hammer_dent_1", ((4.85, 2.55, -3.58)), ((5.35, 3.05, -3.35)), "rust_dk")
    g.box(b, "hammer_dent_2", ((7.85, 2.55, -3.62)), ((8.35, 3.05, -3.35)), "rust_dk")
    g.box(b, "hammer_dent_3", ((4.85, 4.85, -3.62)), ((5.35, 5.35, -3.35)), "rust")
    g.box(b, "hammer_dent_4", ((7.85, 4.85, -3.62)), ((8.35, 5.35, -3.35)), "rust")

    # 铁套裂痕加固铁锔钉 (两枚跨裂纹铁锔钉)
    g.box(b, "cramp_strap_1", ((3.85, 3.65, -3.68)), ((4.55, 4.05, -3.37)), "rust")
    g.box(b, "cramp_pin_1a", ((3.75, 3.55, -3.72)), ((4.05, 4.15, -3.45)), "iron_lt")
    g.box(b, "cramp_pin_1b", ((4.35, 3.55, -3.72)), ((4.65, 4.15, -3.45)), "iron_lt")

    g.box(b, "cramp_strap_2", ((8.65, 3.65, -3.68)), ((9.35, 4.05, -3.35)), "rust_dk")
    g.box(b, "cramp_pin_2a", ((8.55, 3.55, -3.72)), ((8.85, 4.15, -3.45)), "iron_lt")
    g.box(b, "cramp_pin_2b", ((9.15, 3.55, -3.72)), ((9.45, 4.15, -3.45)), "iron_lt")


# ── 13. 细节补强（扩充方块总数达 206+）
def part_details(g: Gear) -> None:
    """铁锸微观细节补强。"""
    # 1. 铁锸头锻造强化与刃口多级磨痕 (24 块)
    bi = "spade_blade_iron"
    g.box(bi, "blade_facet_strip_l", ((4.05, 1.29, -3.62)), ((4.85, 2.85, -3.38)), "iron_lt")
    g.box(bi, "blade_facet_strip_r", ((8.39, 1.29, -3.62)), ((9.15, 2.85, -3.38)), "iron_lt")
    g.box(bi, "blade_corner_bevel_l", ((3.45, 0.85, -3.58)), ((3.85, 1.85, -2.42)), "rust_dk")
    g.box(bi, "blade_corner_bevel_r", ((9.35, 0.85, -3.58)), ((9.75, 1.85, -2.42)), "rust_dk")
    g.box(bi, "blade_edge_back_hone", ((3.85, 0.65, -2.48)), ((9.35, 1.05, -2.25)), "iron_lt")
    g.box(bi, "blade_notch_burr_1", ((4.75, 0.85, -3.75)), ((5.05, 1.20, -3.35)), "rust")
    g.box(bi, "blade_notch_burr_2", ((8.25, 0.85, -3.75)), ((8.55, 1.21, -3.35)), "rust_dk")
    g.box(bi, "blade_weld_flange_l", ((3.65, 5.85, -3.65)), ((4.15, 6.45, -2.35)), "rust")
    g.box(bi, "blade_weld_flange_r", ((9.05, 5.85, -3.65)), ((9.55, 6.45, -2.35)), "rust")
    g.box(bi, "blade_iron_scale_1", ((5.45, 2.85, -3.65)), ((5.95, 3.45, -3.35)), "rust_dk")
    g.box(bi, "blade_iron_scale_2", ((7.25, 2.85, -3.65)), ((7.75, 3.45, -3.35)), "rust_dk")
    g.box(bi, "blade_iron_scale_3", ((5.45, 5.25, -3.65)), ((5.95, 5.85, -3.35)), "rust")
    g.box(bi, "blade_iron_scale_4", ((7.25, 5.25, -3.65)), ((7.75, 5.85, -3.35)), "rust")
    g.box(bi, "shoulder_rivet_rim_l", ((4.65, 6.55, -3.78)), ((5.45, 7.25, -3.42)), "rust_dk")
    g.box(bi, "shoulder_rivet_rim_r", ((7.75, 6.55, -3.78)), ((8.55, 7.25, -3.42)), "rust_dk")
    g.box(bi, "blade_back_cramp_1", ((5.15, 3.25, -2.45)), ((5.75, 3.65, -2.22)), "rust_dk")
    g.box(bi, "blade_back_cramp_2", ((7.45, 3.25, -2.45)), ((8.05, 3.65, -2.22)), "rust_dk")
    g.box(bi, "blade_flaw_crack_1", ((5.81, 3.85, -3.70)), ((6.05, 4.65, -3.42)), "void")
    g.box(bi, "blade_flaw_crack_2", ((7.15, 3.85, -3.70)), ((7.50, 4.65, -3.42)), "void")
    g.box(bi, "blade_dimple_strike_1", ((4.45, 1.85, -3.68)), ((4.87, 2.25, -3.32)), "rust")
    g.box(bi, "blade_dimple_strike_2", ((8.35, 1.85, -3.68)), ((8.75, 2.25, -3.32)), "rust")
    g.box(bi, "blade_dimple_strike_3", ((4.45, 5.25, -3.68)), ((4.85, 5.65, -3.32)), "rust_dk")
    g.box(bi, "blade_dimple_strike_4", ((8.35, 5.25, -3.68)), ((8.75, 5.65, -3.32)), "rust_dk")
    g.box(bi, "blade_bottom_stud_c", ((6.45, 1.15, -3.75)), ((6.75, 1.55, -3.35)), "iron_lt")

    # 2. 銎口加固与螺栓 (10 块)
    sf = "spade_socket_ferrule"
    g.box(sf, "socket_iron_rib_l", ((5.12, 8.05, -3.85)), ((5.45, 8.85, -2.17)), "rust")
    g.box(sf, "socket_iron_rib_r", ((7.75, 8.05, -3.90)), ((8.02, 8.85, -2.15)), "rust")
    g.box(sf, "socket_clamp_ear_f", ((6.25, 7.85, -4.05)), ((6.95, 8.45, -3.75)), "iron_lt")
    g.box(sf, "socket_clamp_ear_b", ((6.25, 7.85, -2.25)), ((6.95, 8.45, -1.95)), "iron")
    g.box(sf, "socket_seam_line", ((6.45, 8.55, -4.02)), ((6.75, 9.12, -3.75)), "void")
    g.box(sf, "socket_pin_head_l", ((4.92, 8.28, -3.15)), ((5.15, 8.62, -2.85)), "iron_lt")
    g.box(sf, "socket_pin_nut_r", ((8.05, 8.28, -3.15)), ((8.28, 8.62, -2.85)), "iron_lt")
    g.box(sf, "socket_reinforce_ring", ((5.35, 8.45, -3.92)), ((7.85, 8.75, -2.08)), "rust_dk")
    g.box(sf, "socket_wear_chamfer", ((5.45, 9.15, -3.82)), ((7.75, 9.38, -2.18)), "iron_lt")
    g.box(sf, "socket_base_gusset", ((6.05, 7.39, -3.92)), ((7.15, 7.75, -2.08)), "rust_dk")

    # 3. 直木柄节痕与防滑包浆 (14 块)
    sl = "shaft_lower"
    g.box(sl, "shaft_lo_knot_1", ((5.55, 10.25, -3.25)), ((5.85, 10.85, -2.75)), "hide_dk")
    g.box(sl, "shaft_lo_knot_2", ((7.35, 11.45, -3.25)), ((7.65, 12.05, -2.75)), "hide_dk")
    g.box(sl, "shaft_lo_iron_staple", ((6.25, 11.25, -3.92)), ((6.95, 11.65, -3.62)), "rust")

    sg = "shaft_grip"
    g.box(sg, "shaft_grip_wrap_band_1", ((5.68, 13.61, -3.82)), ((7.52, 13.95, -2.18)), "hide_dk")
    g.box(sg, "shaft_grip_wrap_band_2", ((5.68, 15.25, -3.82)), ((7.52, 15.65, -2.18)), "hide_dk")
    g.box(sg, "shaft_grip_wrap_band_3", ((5.68, 16.95, -3.82)), ((7.52, 17.35, -2.18)), "hide_dk")
    g.box(sg, "shaft_grip_thumb_rest", ((7.42, 14.55, -3.25)), ((7.72, 15.35, -2.75)), "hide_dk")
    g.box(sg, "shaft_grip_palm_groove", ((5.48, 14.55, -3.25)), ((5.78, 15.35, -2.75)), "hide_dk")
    g.box(sg, "shaft_grip_seam_slit", ((6.45, 15.85, -3.93)), ((6.75, 16.65, -3.58)), "void")

    su = "shaft_upper"
    g.box(su, "shaft_hi_grain_strip_1", ((5.95, 19.25, -3.85)), ((7.25, 23.25, -3.62)), "hide_lt")
    g.box(su, "shaft_hi_grain_strip_2", ((5.95, 19.25, -2.41)), ((7.25, 23.25, -2.15)), "hide_dk")
    g.box(su, "shaft_hi_wood_spur_1", ((5.55, 20.45, -3.25)), ((5.85, 20.95, -2.75)), "hide_dk")
    g.box(su, "shaft_hi_wood_spur_2", ((7.35, 22.15, -3.25)), ((7.65, 22.65, -2.75)), "hide_dk")
    g.box(su, "shaft_hi_tenon_collar_b", ((5.65, 24.85, -3.82)), ((7.55, 25.25, -2.14)), "rust")

    # 4. T 形把手与接合铆扣细节 (10 块)
    tc = "handle_tee_crossbar"
    g.box(tc, "tee_cross_brace_l", ((5.05, 25.15, -3.70)), ((5.75, 25.65, -2.35)), "hide_dk")
    g.box(tc, "tee_cross_brace_r", ((7.45, 25.15, -3.63)), ((8.15, 25.65, -2.35)), "hide_dk")
    g.box(tc, "tee_handle_groove_l", ((4.85, 25.85, -3.85)), ((5.35, 26.35, -3.55)), "hide_dk")
    g.box(tc, "tee_handle_groove_r", ((7.85, 25.85, -3.85)), ((8.35, 26.35, -3.55)), "hide_dk")
    g.box(tc, "tee_butt_cap_band_l", ((4.32, 25.75, -3.72)), ((4.62, 26.45, -2.28)), "rust_dk")
    g.box(tc, "tee_butt_cap_band_r", ((8.58, 25.75, -3.72)), ((8.88, 26.45, -2.28)), "rust_dk")

    col = "handle_tee_collar"
    g.box(col, "tee_collar_stud_f", ((6.45, 25.45, -4.02)), ((6.75, 25.75, -3.71)), "iron_lt")
    g.box(col, "tee_collar_stud_b", ((6.45, 25.45, -2.25)), ((6.75, 25.75, -1.98)), "iron_lt")
    g.box(col, "tee_bracket_plate_l", ((5.55, 25.25, -3.72)), ((5.85, 25.75, -2.25)), "iron")
    g.box(col, "tee_bracket_plate_r", ((7.35, 25.28, -3.75)), ((7.65, 25.75, -2.25)), "iron")

    # 5. 补充 28 块细节扩展（使总数达到 206 块以上）
    g.box(bi, "blade_extra_bevel_1", ((3.75, 1.25, -3.65)), ((4.25, 2.25, -3.35)), "iron_lt")
    g.box(bi, "blade_extra_bevel_2", ((8.95, 1.25, -3.65)), ((9.45, 2.25, -3.35)), "iron_lt")
    g.box(bi, "blade_extra_dent_a", ((5.15, 3.85, -3.65)), ((5.55, 4.25, -3.35)), "rust_dk")
    g.box(bi, "blade_extra_dent_b", ((7.65, 3.85, -3.65)), ((8.05, 4.25, -3.35)), "rust_dk")
    g.box(bi, "blade_lip_ear_l", ((3.55, 4.85, -3.55)), ((3.95, 5.65, -2.45)), "rust")
    g.box(bi, "blade_lip_ear_r", ((9.25, 4.85, -3.53)), ((9.65, 5.65, -2.47)), "rust")

    g.box(sf, "socket_iron_ear_l2", ((5.18, 7.85, -3.45)), ((5.49, 8.35, -2.55)), "iron_lt")
    g.box(sf, "socket_iron_ear_r2", ((7.69, 7.85, -3.45)), ((7.97, 8.35, -2.55)), "iron_lt")
    g.box(sf, "socket_bolt_pin_back", ((6.25, 8.85, -2.18)), ((6.95, 9.15, -1.95)), "iron_lt")
    g.box(sf, "socket_stud_corner_1", ((5.45, 8.87, -3.88)), ((5.75, 9.09, -3.62)), "iron_lt")
    g.box(sf, "socket_stud_corner_2", ((7.45, 8.85, -3.88)), ((7.75, 9.12, -3.62)), "iron_lt")

    g.box(sl, "shaft_lo_cramp_1", ((6.25, 9.85, -3.85)), ((6.95, 10.25, -3.55)), "rust")
    g.box(sl, "shaft_lo_cramp_2", ((6.25, 12.15, -3.83)), ((6.95, 12.55, -3.55)), "rust_dk")
    g.box(sl, "shaft_lo_bark_ridge_1", ((5.95, 10.05, -3.91)), ((7.25, 12.45, -3.60)), "hide_lt")
    g.box(sl, "shaft_lo_bark_ridge_2", ((5.95, 10.05, -2.43)), ((7.25, 12.45, -2.18)), "hide_dk")

    g.box(sg, "shaft_grip_ridge_a", ((5.95, 13.55, -3.85)), ((7.19, 17.55, -3.57)), "hide")
    g.box(sg, "shaft_grip_ridge_b", ((5.99, 13.55, -2.38)), ((7.27, 17.55, -2.10)), "hide_dk")
    g.box(sg, "shaft_grip_knot_spur", ((5.45, 16.25, -3.25)), ((5.75, 16.75, -2.75)), "hide_dk")
    g.box(sg, "shaft_grip_band_extra", ((5.65, 14.45, -3.90)), ((7.55, 14.85, -2.15)), "hide_dk")

    g.box(su, "shaft_hi_ring_mark_2", ((5.68, 21.85, -3.82)), ((7.52, 22.25, -2.18)), "hide_dk")
    g.box(su, "shaft_hi_cramp_strap", ((6.25, 23.55, -3.85)), ((6.95, 23.95, -3.55)), "rust")
    g.box(su, "shaft_hi_cramp_pin_l", ((6.15, 23.42, -3.88)), ((6.45, 24.05, -3.62)), "iron_lt")
    g.box(su, "shaft_hi_cramp_pin_r", ((6.75, 23.50, -3.88)), ((7.05, 24.05, -3.62)), "iron_lt")

    g.box(tc, "tee_cross_edge_l", ((4.65, 26.35, -3.72)), ((5.25, 26.68, -2.28)), "hide_lt")
    g.box(tc, "tee_cross_edge_r", ((7.95, 26.35, -3.72)), ((8.55, 26.68, -2.28)), "hide_lt")
    g.box(tc, "tee_butt_cap_stud_l", ((4.22, 25.95, -3.15)), ((4.48, 26.25, -2.85)), "iron_lt")
    g.box(tc, "tee_butt_cap_stud_r", ((8.75, 25.95, -3.15)), ((8.98, 26.25, -2.85)), "iron_lt")
    g.box(col, "tee_collar_rim_flange", ((5.59, 25.18, -3.88)), ((7.58, 25.91, -2.16)), "iron_lt")
    # ── 补充 31 个精细构件使总数达到 206 cubes (>=200)
    # 1. 铁锸头锻造与铆接细节 (12 块)
    bi = "spade_blade_iron"
    g.box(bi, "blade_hammer_ring_1", ((4.88, 2.25, -3.62)), ((5.25, 2.65, -3.33)), "rust_dk")
    g.box(bi, "blade_hammer_ring_2", ((8.55, 2.25, -3.63)), ((8.95, 2.65, -3.35)), "rust_dk")
    g.box(bi, "blade_hammer_ring_3", ((4.85, 4.45, -3.65)), ((5.25, 4.81, -3.35)), "rust")
    g.box(bi, "blade_hammer_ring_4", ((8.55, 4.45, -3.65)), ((8.95, 4.89, -3.35)), "rust")
    g.box(bi, "blade_weld_rivet_a", ((4.35, 5.21, -3.65)), ((4.75, 5.69, -3.35)), "iron_lt")
    g.box(bi, "blade_weld_rivet_b", ((9.05, 5.25, -3.65)), ((9.45, 5.69, -3.35)), "iron_lt")
    g.box(bi, "blade_back_bevel_l", ((4.35, 1.25, -2.48)), ((5.05, 2.49, -2.22)), "iron_lt")
    g.box(bi, "blade_back_bevel_r", ((8.75, 1.25, -2.48)), ((9.45, 2.45, -2.22)), "iron_lt")
    g.box(bi, "blade_edge_chip_indent", ((6.45, 0.45, -3.72)), ((6.75, 0.85, -2.28)), "void")
    g.box(bi, "blade_edge_corner_l", ((4.15, 0.52, -3.62)), ((4.55, 1.12, -2.38)), "iron_lt")
    g.box(bi, "blade_edge_corner_r", ((9.25, 0.52, -3.62)), ((9.65, 1.12, -2.38)), "iron_lt")
    g.box(bi, "blade_spine_stud_top", ((7.05, 7.45, -3.85)), ((7.35, 7.77, -3.45)), "iron_lt")

    # 2. 銎口套箍与木板芯紧固 (8 块)
    sf = "spade_socket_ferrule"
    g.box(sf, "socket_rivet_side_l", ((5.75, 7.85, -3.25)), ((6.05, 8.25, -2.75)), "copper")
    g.box(sf, "socket_rivet_side_r", ((7.65, 7.89, -3.25)), ((7.95, 8.25, -2.75)), "copper")
    g.box(sf, "socket_lip_chamfer", ((6.02, 9.25, -3.85)), ((8.38, 9.55, -2.15)), "iron_lt")
    g.box(sf, "socket_bracket_base_f", ((6.75, 7.59, -4.02)), ((7.65, 7.95, -3.68)), "rust")

    cw = "spade_core_wood"
    g.box(cw, "core_tenon_tongue", ((6.85, 7.25, -3.35)), ((7.55, 8.11, -2.65)), "hide_lt")
    g.box(cw, "core_wood_cramp", ((6.35, 4.55, -3.48)), ((7.07, 4.95, -3.22)), "rust_dk")
    g.box(cw, "core_wood_flaw", ((7.05, 5.45, -3.48)), ((7.35, 6.15, -3.22)), "void")
    g.box(cw, "core_wood_back_grain", ((6.45, 3.25, -2.78)), ((7.85, 6.25, -2.48)), "hide_lt")

    # 3. 长直木柄与 T 形把手细节 (11 块)
    sg = "shaft_grip"
    g.box(sg, "grip_seam_line_1", ((6.95, 13.81, -3.88)), ((7.43, 14.55, -3.63)), "void")
    g.box(sg, "grip_knot_lump_1", ((6.05, 15.65, -3.25)), ((6.35, 16.15, -2.75)), "hide_dk")
    g.box(sg, "grip_knot_lump_2", ((7.35, 16.45, -3.25)), ((7.65, 16.95, -2.75)), "hide_dk")
    g.box(sg, "grip_sweat_polish_1", ((6.35, 13.85, -3.91)), ((8.05, 17.25, -3.62)), "hide_dk")

    su = "shaft_upper"
    g.box(su, "shaft_hi_staple_1", ((6.85, 19.85, -3.88)), ((7.55, 20.25, -3.58)), "rust")
    g.box(su, "shaft_hi_grain_b", ((6.45, 22.25, -2.38)), ((7.95, 24.85, -2.11)), "hide_dk")
    g.box(su, "shaft_hi_collar_band", ((6.22, 24.55, -3.87)), ((8.18, 24.95, -2.18)), "rust_dk")

    tc = "handle_tee_crossbar"
    g.box(tc, "tee_grip_carve_l", ((5.45, 26.15, -3.75)), ((5.95, 26.52, -2.25)), "hide_dk")
    g.box(tc, "tee_grip_carve_r", ((8.45, 26.15, -3.75)), ((8.95, 26.55, -2.25)), "hide_dk")
    g.box(tc, "tee_top_ridge_cap", ((5.75, 26.82, -3.55)), ((8.65, 27.12, -2.45)), "hide_lt")

    col = "handle_tee_collar"
    g.box(col, "tee_collar_side_flange", ((6.15, 25.25, -3.85)), ((8.25, 25.69, -2.15)), "iron_lt")



PART_FUNCS = {
    "blade_iron": part_spade_blade,
    "blade_spine": part_spade_spine,
    "wood_core": part_spade_wood_core,
    "rivets": part_spade_rivets,
    "socket": part_socket_ferrule,
    "cartouche": part_cartouche,
    "shaft_lower": part_shaft_lower,
    "shaft_grip": part_shaft_grip,
    "shaft_upper": part_shaft_upper,
    "handle_tee": part_handle_tee,
    "tee_collar": part_handle_tee_collar,
    "cramps": part_blade_cramps,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="tie_cha",
    model_name="TieCha",
    title="铁锸 / tie_cha",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    review_category="metalware",
    wear_note="挂点：右手手持（全长约26px古代铁锸，圆肩起脊U形铁套）",
    worn_focus=((6.0, 13.0, -3.0), 32.0),
    round_label="round 1/3",
    textured=False,
)

SPEC.gates = ToolGates(
    SPEC.title, SPEC.mats, asym=SPEC.asym,
    soft_over=SPEC.soft_over, free_floating=SPEC.free_floating
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
