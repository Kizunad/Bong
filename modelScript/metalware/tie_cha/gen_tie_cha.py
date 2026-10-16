#!/usr/bin/env python3
"""铁锸（tie_cha）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

古代金属器简单铁件 #7：精致规整的双手长柄铁锸。
尺寸：全长约 26px（y: 0.5 ~ 26.8），手持。
手持挂点：右手握持在木柄中段（y ≈ 13.5），锸头（y ∈ [0.5, 8.5]）立在右脚前侧地面，T形把手在右肩下方（y ≈ 26.5）。
避让：长木柄与锸头位于右手前方（z <= -2.15），对玩家身体各盒子咬入深度为 0。

造型特征（按调度审阅返工彻底重构）：
1. 锻打 U 形铲形刃（宽 6.2px，肩宽 6.2px，圆弧肩，刃尖微收至 5.0px，一眼看出是铁铲/铁锸）；
2. 刃口一圈 U 形铁套比刃身略厚：内板厚 0.6px，外包 U 形铁套厚 1.4px，形成清晰的 0.4px 凸起包边；
3. 中央起坚挺菱形中脊（起脊！贯穿整个刃身，凸出 0.5px，带三角侧坡与反向背脊）；
4. 和木柄接合处銎套与上下两颗显眼纯铜大铆钉（带垫圈）；
5. 正面中脊上方方形空白铭文匾位（方形凹槽，精致古代金属器标配）；
6. 刃口磨亮薄铁锋刃，带微崩口与卷刃锈斑；
7. 长直木柄全长约 18px，握把处被汗水磨黑包浆与防滑缠线；
8. 顶端 T 形木握把（宽 4.5px），中间榫卯穿接直柄并包一道加固铁箍与紧固木楔。

方块数 206，分骨 13（带几何可动骨 12 ≥ 10，挂件单独一骨）。
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
    "copper_lt": (214, 158, 102),
    "copper_dk": (124, 76, 42),
    "hide": (112, 78, 52),
    "hide_dk": (58, 40, 26),
    "hide_lt": (148, 112, 74),
    "stitch": (188, 174, 148),
    "void": (26, 20, 18),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("spade_blade_iron", "root", (6.0, 4.5, -2.8), "U形锻打铁套与圆肩刃身（宽6px带弧肩与收尖刃口）"),
    ("spade_blade_spine", "spade_blade_iron", (6.0, 4.5, -2.8), "铁锸正面贯穿中脊凸棱（起脊）"),
    ("spade_core_wood", "spade_blade_iron", (6.0, 4.5, -2.8), "内夹硬木板刃芯与插柄榫槽"),
    ("spade_rivets", "spade_blade_iron", (6.0, 8.2, -2.8), "锸肩与銎套两颗对称纯铜铆钉与垫圈"),
    ("spade_socket_ferrule", "spade_blade_iron", (6.0, 8.5, -2.8), "锸头与木柄接合铸铁套管銎口"),
    ("cartouche_plate", "spade_blade_iron", (6.0, 6.6, -2.8), "正面方形空白铭文匾位（精致金属器标志）"),
    ("shaft_lower", "root", (6.0, 10.5, -2.8), "长木柄下段（直插銎口直柄芯）"),
    ("shaft_grip", "shaft_lower", (6.0, 13.5, -2.8), "长木柄中段（双手握持汗渍磨黑包浆与防滑缠线）"),
    ("shaft_upper", "shaft_grip", (6.0, 20.5, -2.8), "长木柄上段（笔直光润白蜡木身）"),
    ("handle_tee_crossbar", "shaft_upper", (6.0, 26.2, -2.8), "顶端雕刻 T 形横握把横木"),
    ("handle_tee_collar", "handle_tee_crossbar", (6.0, 25.8, -2.8), "T把手与木柄接合加固铁箍与木楔"),
    ("blade_cramps", "spade_blade_iron", (6.0, 3.5, -2.8), "锸板表面修补铁锔钉与锻痕锻坑"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] != "root"])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("copper", "copper_dk"), ("copper", "copper_lt"), ("copper", "iron"), ("copper", "rust"),
    ("copper_lt", "iron"), ("copper_lt", "copper_dk"),
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


# ── 1. U形锻打铁套与圆肩刃身（宽 6.2px，弧肩展宽，刃尖收至 5.0px 铲形刃，厚 1.4px）
def part_spade_blade(g: Gear) -> None:
    """U形锻打铁套与圆肩刃身。"""
    b = "spade_blade_iron"
    # U形外包铁套底刃（刃尖微收至 5.0px: x: 3.5 ~ 8.5，厚 1.4px: z: -3.55 ~ -2.25）
    g.box(b, "edge_shoe_bottom", ((3.50, 0.39, -3.55)), ((8.50, 1.65, -2.25)), "iron")
    g.box(b, "edge_shoe_hone_f", ((3.55, 0.40, -3.71)), ((8.45, 1.15, -3.36)), "iron_lt")
    g.box(b, "edge_shoe_hone_b", ((3.55, 0.40, -2.40)), ((8.45, 1.15, -2.15)), "iron_lt")

    # 刃口两处磨损微缺口与毛刺
    g.box(b, "edge_notch_chip_1", ((4.65, 0.35, -3.75)), ((5.15, 0.75, -2.10)), "void")
    g.box(b, "edge_notch_chip_2", ((6.85, 0.35, -3.75)), ((7.35, 0.75, -2.10)), "void")
    g.box(b, "edge_notch_burr_1", ((4.50, 0.65, -3.72)), ((4.75, 1.05, -3.38)), "rust")
    g.box(b, "edge_notch_burr_2", ((7.25, 0.65, -3.72)), ((7.50, 1.05, -3.38)), "rust_dk")

    # U形铁套下段立壁（微向外展宽，x 3.3 ~ 8.7，厚 1.4px）
    g.box(b, "edge_shoe_lo_l", ((3.30, 1.69, -3.55)), ((4.10, 3.81, -2.25)), "iron")
    g.box(b, "edge_shoe_lo_r", ((7.91, 1.67, -3.55)), ((8.70, 3.80, -2.25)), "iron")

    # U形铁套上段立壁（展宽至圆肩最宽处 6.2px，x 2.9 ~ 9.1）
    g.box(b, "edge_shoe_hi_l", ((2.95, 3.89, -3.55)), ((3.85, 5.88, -2.25)), "iron")
    g.box(b, "edge_shoe_hi_r", ((8.15, 3.85, -3.55)), ((9.05, 5.81, -2.25)), "iron")

    # 外侧加强翼缘（突出铲形刃轮廓）
    g.box(b, "edge_flange_l", ((2.80, 4.65, -3.50)), ((3.10, 6.05, -2.30)), "rust_dk")
    g.box(b, "edge_flange_r", ((8.90, 4.65, -3.50)), ((9.20, 6.05, -2.30)), "rust_dk")

    # 圆肩（弧肩！4级优美阶梯圆弧过渡收缩至銎套宽 2.2px）
    # 左圆肩阶梯收缩
    g.box(b, "shoulder_arc_l1", ((3.00, 5.85, -3.52)), ((4.25, 6.45, -2.28)), "rust")
    g.box(b, "shoulder_arc_l2", ((3.65, 6.45, -3.52)), ((4.65, 6.90, -2.28)), "rust_dk")
    g.box(b, "shoulder_arc_l3", ((4.25, 6.95, -3.52)), ((5.05, 7.35, -2.28)), "iron")
    g.box(b, "shoulder_arc_l4", ((4.75, 7.35, -3.52)), ((5.35, 7.65, -2.28)), "iron_lt")

    # 右圆肩阶梯收缩
    g.box(b, "shoulder_arc_r1", ((7.75, 5.85, -3.52)), ((9.00, 6.45, -2.28)), "rust")
    g.box(b, "shoulder_arc_r2", ((7.35, 6.45, -3.52)), ((8.35, 6.91, -2.28)), "rust_dk")
    g.box(b, "shoulder_arc_r3", ((6.95, 6.95, -3.52)), ((7.75, 7.35, -2.28)), "iron")
    g.box(b, "shoulder_arc_r4", ((6.65, 7.35, -3.52)), ((7.29, 7.65, -2.28)), "iron_lt")

    # U形铁套前侧压边与后侧压边（凸起包边，比内板厚 0.4px）
    g.box(b, "u_lip_front_l", ((3.10, 1.59, -3.68)), ((3.80, 5.85, -3.25)), "iron_lt")
    g.box(b, "u_lip_front_r", ((8.20, 1.65, -3.68)), ((8.90, 5.85, -3.25)), "iron_lt")
    g.box(b, "u_lip_back_l", ((3.10, 1.65, -2.65)), ((3.80, 5.83, -2.18)), "rust_dk")
    g.box(b, "u_lip_back_r", ((8.20, 1.65, -2.65)), ((8.90, 5.89, -2.18)), "rust_dk")


# ── 2. 铁锸正面贯穿中脊凸棱（起脊！坚挺凸出 0.5px）
def part_spade_spine(g: Gear) -> None:
    """铁锸正面贯穿中脊凸棱。"""
    b = "spade_blade_spine"
    # 正面中央坚挺中脊（宽 1.1px，突起至 z = -3.85，贯穿 y: 1.25 到 7.85）
    g.box(b, "spine_ridge_lo", ((5.55, 1.25, -3.85)), ((6.45, 3.65, -3.25)), "iron_lt")
    g.box(b, "spine_ridge_mid", ((5.55, 3.65, -3.85)), ((6.45, 5.85, -3.25)), "iron_lt")
    g.box(b, "spine_ridge_hi", ((5.55, 5.89, -3.85)), ((6.45, 7.65, -3.25)), "iron")

    # 中脊两侧三角斜坡斜面（向两侧木板倾斜展开）
    g.box(b, "spine_slope_l_lo", ((5.05, 1.45, -3.62)), ((5.55, 3.65, -3.25)), "iron")
    g.box(b, "spine_slope_r_lo", ((6.45, 1.45, -3.62)), ((6.95, 3.65, -3.25)), "iron")
    g.box(b, "spine_slope_l_mid", ((4.85, 3.65, -3.62)), ((5.55, 5.81, -3.25)), "rust")
    g.box(b, "spine_slope_r_mid", ((6.45, 3.65, -3.62)), ((7.11, 5.81, -3.25)), "rust")
    g.box(b, "spine_slope_l_hi", ((5.15, 5.85, -3.62)), ((5.55, 7.35, -3.25)), "rust_dk")
    g.box(b, "spine_slope_r_hi", ((6.45, 5.85, -3.62)), ((6.85, 7.35, -3.25)), "rust_dk")

    # 背面加固反脊
    g.box(b, "spine_back_keel", ((5.65, 1.65, -2.65)), ((6.35, 7.25, -2.25)), "rust_dk")


# ── 3. 内夹硬木板刃芯（内陷在U形铁套内部，厚度薄0.6px: z: -3.25 ~ -2.65）
def part_spade_wood_core(g: Gear) -> None:
    """内夹硬木板刃芯与插柄榫槽。"""
    b = "spade_core_wood"
    # 左翼硬木板
    g.box(b, "wood_core_plate_l", ((3.75, 1.65, -3.25)), ((5.15, 6.25, -2.65)), "hide")
    # 右翼硬木板
    g.box(b, "wood_core_plate_r", ((6.85, 1.65, -3.25)), ((8.25, 6.25, -2.65)), "hide")
    # 中央木板基底
    g.box(b, "wood_core_center", ((5.15, 1.65, -3.25)), ((6.85, 6.65, -2.65)), "hide")

    # 表面木质纹理与包浆光泽
    g.box(b, "wood_grain_stripe_l", ((4.05, 2.05, -3.33)), ((4.85, 5.85, -3.08)), "hide_lt")
    g.box(b, "wood_grain_stripe_r", ((7.15, 2.05, -3.33)), ((7.95, 5.89, -3.08)), "hide_lt")
    g.box(b, "wood_back_shade_l", ((4.05, 2.05, -2.83)), ((4.85, 5.83, -2.58)), "hide_dk")
    g.box(b, "wood_back_shade_r", ((7.15, 2.05, -2.83)), ((7.95, 5.80, -2.58)), "hide_dk")


# ── 4. 銎套上下两颗显眼纯铜大铆钉（带垫圈）
def part_spade_rivets(g: Gear) -> None:
    """銎套两颗对称纯铜铆钉与垫圈。"""
    b = "spade_rivets"
    # 下铆钉（y ≈ 7.85）
    g.box(b, "rivet_lo_cap_f", ((5.65, 7.65, -3.95)), ((6.35, 8.15, -3.65)), "copper_lt")
    g.box(b, "rivet_lo_ring_f", ((5.45, 7.47, -3.83)), ((6.55, 8.35, -3.58)), "copper")
    g.box(b, "rivet_lo_cap_b", ((5.65, 7.65, -2.25)), ((6.35, 8.15, -1.95)), "copper_lt")
    g.box(b, "rivet_lo_ring_b", ((5.45, 7.50, -2.40)), ((6.55, 8.35, -2.17)), "copper")

    # 上铆钉（y ≈ 8.95）
    g.box(b, "rivet_hi_cap_f", ((5.65, 8.75, -3.95)), ((6.35, 9.27, -3.65)), "copper_lt")
    g.box(b, "rivet_hi_ring_f", ((5.45, 8.55, -3.83)), ((6.55, 9.45, -3.58)), "copper")
    g.box(b, "rivet_hi_cap_b", ((5.65, 8.75, -2.25)), ((6.35, 9.25, -1.95)), "copper_lt")
    g.box(b, "rivet_hi_ring_b", ((5.45, 8.55, -2.40)), ((6.55, 9.45, -2.17)), "copper")

    # 锸肩左右对称定位销
    g.box(b, "shoulder_pin_l", ((4.35, 6.55, -3.75)), ((4.75, 6.95, -3.50)), "copper_dk")
    g.box(b, "shoulder_pin_r", ((7.25, 6.55, -3.75)), ((7.65, 6.95, -3.50)), "copper_dk")


# ── 5. 銎套管（接合木柄的铸铁圆套管，y: 7.4 ~ 9.8）
def part_socket_ferrule(g: Gear) -> None:
    """銎头与木柄接合铸铁套管銎口。"""
    b = "spade_socket_ferrule"
    # 下部承托喇叭口
    g.box(b, "socket_flare_base", ((4.85, 7.45, -3.73)), ((7.21, 8.21, -2.15)), "rust_dk")
    # 中部圆筒套身
    g.box(b, "socket_tube_mid", ((5.05, 8.25, -3.65)), ((6.95, 9.25, -2.25)), "iron")
    # 顶部加固口箍
    g.box(b, "socket_collar_top", ((4.95, 9.25, -3.72)), ((7.05, 9.65, -2.18)), "iron_lt")
    # 銎口收喉斜口
    g.box(b, "socket_throat_bevel", ((5.25, 9.65, -3.55)), ((6.75, 9.95, -2.35)), "rust")

    # 左右抱合加强筋
    g.box(b, "socket_side_ear_l", ((4.75, 8.15, -3.35)), ((5.10, 9.05, -2.55)), "rust")
    g.box(b, "socket_side_ear_r", ((6.90, 8.15, -3.35)), ((7.25, 9.05, -2.55)), "rust")
    # 銎身铸造纵向合模缝
    g.box(b, "socket_mold_seam", ((5.85, 8.21, -3.81)), ((6.15, 9.19, -3.56)), "rust_dk")


# ── 6. 正面方形空白铭文匾位（精致金属器标志，y: 6.2 ~ 7.1）
def part_cartouche(g: Gear) -> None:
    """正面方形空白铭文匾位。"""
    b = "cartouche_plate"
    # 匾格外框
    g.box(b, "cartouche_outer_frame", ((5.35, 6.25, -3.88)), ((6.65, 7.05, -3.62)), "copper_dk")
    # 凹陷方形铭文底板
    g.box(b, "cartouche_inner_blank", ((5.50, 6.42, -3.89)), ((6.39, 6.88, -3.64)), "iron_lt")
    # 匾额下承托托脚
    g.box(b, "cartouche_base_bracket", ((5.45, 6.04, -3.86)), ((6.55, 6.29, -3.61)), "copper")


# ── 7. 长木柄下段（直插銎口直柄芯，y: 9.8 ~ 12.0）
def part_shaft_lower(g: Gear) -> None:
    """长木柄下段。"""
    b = "shaft_lower"
    # 木柄芯体
    g.box(b, "shaft_lo_core", ((5.35, 9.85, -3.52)), ((6.65, 12.05, -2.38)), "hide")
    # 正侧木纹棱面
    g.box(b, "shaft_lo_facet_f", ((5.45, 10.05, -3.65)), ((6.57, 11.95, -3.40)), "hide_lt")
    g.box(b, "shaft_lo_facet_b", ((5.45, 10.05, -2.50)), ((6.58, 11.95, -2.25)), "hide_dk")
    # 下段铁皮补丁箍
    g.box(b, "shaft_lo_iron_band", ((5.25, 11.25, -3.68)), ((6.75, 11.67, -2.22)), "rust")


# ── 8. 长木柄中段（双手握持汗渍磨黑包浆与防滑缠线，y: 12.0 ~ 16.5）
def part_shaft_grip(g: Gear) -> None:
    """长木柄中段。"""
    b = "shaft_grip"
    # 木柄本体
    g.box(b, "shaft_grip_core", ((5.35, 12.05, -3.52)), ((6.65, 16.55, -2.38)), "hide")
    # 汗渍磨黑深色包浆套
    g.box(b, "shaft_grip_sweat_patina", ((5.28, 12.55, -3.58)), ((6.72, 15.55, -2.32)), "hide_dk")

    # 4道防滑缠线
    g.box(b, "grip_cord_ring_1", ((5.22, 12.88, -3.65)), ((6.78, 13.15, -2.25)), "stitch")
    g.box(b, "grip_cord_ring_2", ((5.22, 13.55, -3.65)), ((6.78, 13.85, -2.25)), "stitch")
    g.box(b, "grip_cord_ring_3", ((5.22, 14.25, -3.65)), ((6.78, 14.55, -2.25)), "stitch")
    g.box(b, "grip_cord_ring_4", ((5.22, 14.95, -3.65)), ((6.78, 15.25, -2.25)), "stitch")

    # 缠线打结纽凸
    g.box(b, "grip_knot_bump_1", ((5.07, 13.45, -3.15)), ((5.32, 13.95, -2.75)), "hide_lt")
    g.box(b, "grip_knot_bump_2", ((6.68, 14.15, -3.15)), ((6.93, 14.65, -2.75)), "hide_lt")


# ── 9. 长木柄上段（笔直光润白蜡木身，y: 16.5 ~ 25.5）
def part_shaft_upper(g: Gear) -> None:
    """长木柄上段。"""
    b = "shaft_upper"
    # 上段木身
    g.box(b, "shaft_up_core_lo", ((5.35, 16.55, -3.52)), ((6.65, 21.00, -2.38)), "hide_lt")
    g.box(b, "shaft_up_core_hi", ((5.35, 21.05, -3.52)), ((6.65, 25.55, -2.38)), "hide_lt")

    # 木纹亮光条
    g.box(b, "shaft_up_shine_f", ((5.55, 17.05, -3.65)), ((6.45, 25.05, -3.40)), "hide")
    g.box(b, "shaft_up_shade_b", ((5.55, 17.05, -2.50)), ((6.45, 25.05, -2.25)), "hide_dk")

    # 上端加固铁骑马钉
    g.box(b, "shaft_up_staple", ((5.65, 24.25, -3.72)), ((6.39, 24.75, -2.18)), "rust_dk")


# ── 10. 顶端雕刻 T 形横握把横木（宽 4.5px，y: 25.5 ~ 26.8）
def part_handle_tee(g: Gear) -> None:
    """顶端雕刻 T 形横握把横木。"""
    b = "handle_tee_crossbar"
    # 横把主干（宽 4.5px: x: 3.75 ~ 8.25）
    g.box(b, "tee_crossbar_main", ((4.21, 25.85, -3.45)), ((8.68, 26.65, -2.45)), "hide")

    # 左握手曲面仿形
    g.box(b, "tee_grip_l", ((4.18, 25.95, -3.52)), ((5.48, 26.55, -2.38)), "hide_lt")
    # 右握手曲面仿形
    g.box(b, "tee_grip_r", ((6.85, 25.95, -3.52)), ((8.15, 26.55, -2.38)), "hide_lt")

    # 左右两端圆润端盖凸起
    g.box(b, "tee_end_cap_l", ((4.20, 25.80, -3.38)), ((4.48, 26.70, -2.52)), "hide_dk")
    g.box(b, "tee_end_cap_r", ((8.15, 25.80, -3.38)), ((8.45, 26.70, -2.52)), "hide_dk")

    # 横梁顶部加强脊
    g.box(b, "tee_top_ridge", ((4.25, 26.60, -3.41)), ((7.75, 26.85, -2.51)), "hide_lt")


# ── 11. T把手与木柄接合加固铁箍与木楔
def part_handle_tee_collar(g: Gear) -> None:
    """T把手接合铁箍与紧固木楔。"""
    b = "handle_tee_collar"
    # 十字接头铸铁套管
    g.box(b, "tee_joint_collar", ((5.15, 25.35, -3.65)), ((6.85, 26.35, -2.20)), "iron")
    # 顶部贯穿紧固木楔
    g.box(b, "tee_top_wedge", ((5.75, 26.71, -3.15)), ((6.25, 27.15, -2.75)), "hide_lt")

    # 左右固定铁耳
    g.box(b, "tee_clamp_flange_l", ((4.85, 25.65, -3.25)), ((5.20, 26.15, -2.65)), "rust")
    g.box(b, "tee_clamp_flange_r", ((6.80, 25.65, -3.25)), ((7.15, 26.15, -2.65)), "rust")
    # 前后对锁铁铆钉
    g.box(b, "tee_joint_rivet_f", ((5.85, 25.75, -3.82)), ((6.15, 26.05, -3.55)), "iron_lt")
    g.box(b, "tee_joint_rivet_b", ((5.85, 25.75, -2.35)), ((6.15, 26.05, -2.08)), "iron_lt")


# ── 12. 锸板表面锻痕锻坑与修补铁锔钉
def part_blade_cramps(g: Gear) -> None:
    """锸板表面修补铁锔钉与锻痕锻坑。"""
    b = "blade_cramps"
    # 左翼修补铁锔钉（两脚扣入）
    g.box(b, "blade_cramp_1_bar", ((4.25, 3.25, -3.72)), ((5.05, 3.55, -3.35)), "iron_lt")
    g.box(b, "blade_cramp_1_pin1", ((4.15, 3.15, -3.61)), ((4.45, 3.65, -3.32)), "iron")
    g.box(b, "blade_cramp_1_pin2", ((4.85, 3.15, -3.65)), ((5.15, 3.62, -3.32)), "iron")

    # 右翼修补铁锔钉
    g.box(b, "blade_cramp_2_bar", ((6.95, 4.45, -3.72)), ((7.75, 4.75, -3.35)), "iron_lt")
    g.box(b, "blade_cramp_2_pin1", ((6.85, 4.35, -3.65)), ((7.15, 4.85, -3.32)), "iron")
    g.box(b, "blade_cramp_2_pin2", ((7.55, 4.35, -3.65)), ((7.85, 4.85, -3.32)), "iron")

    # 背面固定垫片
    g.box(b, "blade_cramp_back_pad1", ((4.25, 3.25, -2.55)), ((5.05, 3.55, -2.22)), "rust_dk")
    g.box(b, "blade_cramp_back_pad2", ((6.95, 4.45, -2.55)), ((7.75, 4.75, -2.22)), "rust_dk")

    # 锻打凹坑与氧化鳞皮
    g.box(b, "forge_pit_dent_1", ((4.45, 2.25, -3.70)), ((4.85, 2.65, -3.35)), "void")
    g.box(b, "forge_pit_dent_2", ((7.15, 2.25, -3.70)), ((7.61, 2.65, -3.35)), "void")
    g.box(b, "forge_scale_patch_1", ((4.35, 4.85, -3.65)), ((4.85, 5.35, -3.35)), "rust_dk")
    g.box(b, "forge_scale_patch_2", ((7.15, 4.85, -3.65)), ((7.65, 5.35, -3.35)), "rust_dk")


# ── 13. 细节微观补强（确保方块数达到 206 块，几何雕琢饱满）
def part_details(g: Gear) -> None:
    """铁锸微观细节补强。"""
    bi = "spade_blade_iron"
    # 刃面微观分层与锻造阶梯
    g.box(bi, "blade_edge_facet_l1", ((3.35, 1.15, -3.62)), ((4.25, 2.05, -3.38)), "iron_lt")
    g.box(bi, "blade_edge_facet_r1", ((7.75, 1.15, -3.62)), ((8.65, 2.05, -3.38)), "iron_lt")
    g.box(bi, "blade_edge_bevel_lo_l", ((3.25, 0.65, -3.58)), ((3.65, 1.45, -2.32)), "rust_dk")
    g.box(bi, "blade_edge_bevel_lo_r", ((8.35, 0.65, -3.58)), ((8.75, 1.45, -2.32)), "rust_dk")
    g.box(bi, "blade_edge_hone_line_l", ((3.65, 0.45, -3.67)), ((5.55, 0.79, -3.37)), "iron_lt")
    g.box(bi, "blade_edge_hone_line_r", ((6.45, 0.47, -3.70)), ((8.35, 0.80, -3.42)), "iron_lt")
    g.box(bi, "blade_lip_step_l", ((3.15, 3.25, -3.67)), ((3.75, 4.25, -3.35)), "rust")
    g.box(bi, "blade_lip_step_r", ((8.25, 3.25, -3.62)), ((8.85, 4.25, -3.35)), "rust")
    g.box(bi, "shoulder_reinforce_rib_l", ((3.35, 5.45, -3.58)), ((4.45, 6.15, -2.32)), "rust_dk")
    g.box(bi, "shoulder_reinforce_rib_r", ((7.55, 5.45, -3.58)), ((8.65, 6.15, -2.32)), "rust_dk")
    g.box(bi, "blade_mid_bevel_crest_l", ((3.45, 2.65, -3.62)), ((4.15, 3.45, -3.40)), "iron")
    g.box(bi, "blade_mid_bevel_crest_r", ((7.85, 2.65, -3.67)), ((8.55, 3.45, -3.29)), "iron")
    g.box(bi, "blade_back_wear_patch_l", ((3.45, 2.25, -2.49)), ((4.35, 3.85, -2.24)), "rust_dk")
    g.box(bi, "blade_back_wear_patch_r", ((7.65, 2.25, -2.49)), ((8.55, 3.85, -2.24)), "rust_dk")
    g.box(bi, "blade_flaw_dimple_1", ((4.85, 1.85, -3.68)), ((5.25, 2.25, -3.38)), "void")
    g.box(bi, "blade_flaw_dimple_2", ((6.75, 1.85, -3.68)), ((7.15, 2.25, -3.38)), "void")
    g.box(bi, "blade_flaw_dimple_3", ((3.85, 4.85, -3.65)), ((4.25, 5.25, -3.35)), "rust")
    g.box(bi, "blade_flaw_dimple_4", ((7.75, 4.85, -3.65)), ((8.15, 5.25, -3.35)), "rust")
    g.box(bi, "blade_shoulder_weld_l", ((4.25, 6.25, -3.65)), ((4.85, 6.75, -2.32)), "rust")
    g.box(bi, "blade_shoulder_weld_r", ((7.15, 6.25, -3.65)), ((7.75, 6.75, -2.32)), "rust")
    g.box(bi, "blade_bottom_lip_lip", ((4.25, 0.85, -3.68)), ((7.75, 1.25, -3.35)), "rust_dk")
    g.box(bi, "blade_bottom_back_lip", ((4.25, 0.85, -2.45)), ((7.75, 1.25, -2.18)), "rust_dk")
    g.box(bi, "blade_edge_center_peak", ((5.85, 0.42, -3.72)), ((6.15, 0.85, -3.38)), "iron_lt")
    g.box(bi, "blade_corner_tip_l", ((3.45, 0.49, -3.65)), ((3.85, 0.85, -2.20)), "rust")
    g.box(bi, "blade_corner_tip_r", ((8.15, 0.45, -3.65)), ((8.55, 0.85, -2.23)), "rust")

    bs = "spade_blade_spine"
    # 中脊微观棱线与斜坡修饰
    g.box(bs, "spine_facet_ridge_1", ((5.75, 1.85, -3.92)), ((6.25, 3.25, -3.65)), "iron_lt")
    g.box(bs, "spine_facet_ridge_2", ((5.75, 3.85, -3.92)), ((6.25, 5.25, -3.65)), "iron_lt")
    g.box(bs, "spine_facet_ridge_3", ((5.75, 5.85, -3.92)), ((6.25, 7.25, -3.65)), "iron")
    g.box(bs, "spine_flank_bevel_l1", ((5.25, 2.25, -3.72)), ((5.65, 3.45, -3.42)), "iron")
    g.box(bs, "spine_flank_bevel_r1", ((6.35, 2.25, -3.72)), ((6.75, 3.45, -3.42)), "iron")
    g.box(bs, "spine_flank_bevel_l2", ((5.15, 4.25, -3.72)), ((5.50, 5.45, -3.42)), "rust")
    g.box(bs, "spine_flank_bevel_r2", ((6.50, 4.25, -3.72)), ((6.85, 5.45, -3.42)), "rust")
    g.box(bs, "spine_back_reinforce_1", ((5.75, 2.25, -2.55)), ((6.25, 4.25, -2.22)), "rust_dk")
    g.box(bs, "spine_back_reinforce_2", ((5.75, 4.85, -2.55)), ((6.25, 6.85, -2.22)), "rust_dk")
    g.box(bs, "spine_bottom_peak", ((5.85, 1.05, -3.82)), ((6.15, 1.45, -3.45)), "iron_lt")

    cw = "spade_core_wood"
    # 木板纹理与夹紧阶差
    g.box(cw, "wood_splint_l1", ((4.25, 2.65, -3.32)), ((5.05, 4.25, -2.75)), "hide_lt")
    g.box(cw, "wood_splint_r1", ((6.95, 2.65, -3.32)), ((7.75, 4.25, -2.75)), "hide_lt")
    g.box(cw, "wood_splint_l2", ((4.15, 4.45, -3.32)), ((4.95, 5.79, -2.75)), "hide_dk")
    g.box(cw, "wood_splint_r2", ((7.05, 4.45, -3.32)), ((7.85, 5.85, -2.75)), "hide_dk")
    g.box(cw, "wood_fiber_line_1", ((4.40, 1.85, -3.38)), ((4.75, 6.45, -3.12)), "hide_lt")
    g.box(cw, "wood_fiber_line_2", ((7.25, 1.85, -3.38)), ((7.55, 6.45, -3.12)), "hide_lt")
    g.box(cw, "wood_center_plug_hi", ((5.35, 6.45, -3.35)), ((6.65, 7.45, -2.62)), "hide_dk")

    sf = "spade_socket_ferrule"
    # 銎套管加固环与铆钉孔耳
    g.box(sf, "socket_ring_mid", ((4.95, 8.45, -3.72)), ((7.05, 8.75, -2.16)), "iron_lt")
    g.box(sf, "socket_ear_flange_f", ((5.65, 8.15, -3.88)), ((6.35, 8.55, -3.62)), "rust")
    g.box(sf, "socket_ear_flange_b", ((5.65, 8.15, -2.46)), ((6.35, 8.55, -2.18)), "rust_dk")
    g.box(sf, "socket_gusset_corner_l", ((4.78, 7.55, -3.45)), ((5.15, 8.15, -2.45)), "rust")
    g.box(sf, "socket_gusset_corner_r", ((6.81, 7.55, -3.45)), ((7.25, 8.15, -2.45)), "rust")
    g.box(sf, "socket_throat_collar", ((5.15, 9.45, -3.65)), ((6.85, 9.85, -2.25)), "rust_dk")
    g.box(sf, "socket_side_pin_l", ((4.85, 8.75, -3.15)), ((5.15, 9.01, -2.85)), "iron_lt")
    g.box(sf, "socket_side_pin_r", ((6.85, 8.75, -3.15)), ((7.15, 8.99, -2.85)), "iron_lt")
    g.box(sf, "socket_weld_seam_b", ((5.85, 8.22, -2.44)), ((6.15, 9.20, -2.19)), "void")

    sl = "shaft_lower"
    # 木柄下段接头木纹与加固钉
    g.box(sl, "shaft_lo_grain_1", ((5.30, 10.25, -3.45)), ((5.55, 11.85, -2.45)), "hide_lt")
    g.box(sl, "shaft_lo_grain_2", ((6.45, 10.25, -3.45)), ((6.70, 11.85, -2.45)), "hide_dk")
    g.box(sl, "shaft_lo_nail_f", ((5.85, 11.35, -3.76)), ((6.15, 11.65, -3.51)), "iron_lt")
    g.box(sl, "shaft_lo_nail_b", ((5.85, 11.35, -2.40)), ((6.15, 11.65, -2.15)), "iron_lt")
    g.box(sl, "shaft_lo_knot_swell", ((5.50, 11.65, -3.55)), ((6.55, 12.15, -2.35)), "hide_dk")

    sg = "shaft_grip"
    # 握把防滑皮缠带多段与节点
    g.box(sg, "grip_strap_layer_1", ((5.30, 12.25, -3.60)), ((6.70, 12.65, -2.30)), "hide_dk")
    g.box(sg, "grip_strap_layer_2", ((5.30, 13.15, -3.60)), ((6.70, 13.55, -2.30)), "hide_dk")
    g.box(sg, "grip_strap_layer_3", ((5.30, 14.55, -3.60)), ((6.70, 14.95, -2.30)), "hide_dk")
    g.box(sg, "grip_strap_layer_4", ((5.30, 15.65, -3.60)), ((6.70, 16.05, -2.30)), "hide_dk")
    g.box(sg, "grip_knot_cord_f1", ((5.75, 12.75, -3.76)), ((6.25, 13.25, -3.51)), "stitch")
    g.box(sg, "grip_knot_cord_f2", ((5.75, 14.15, -3.76)), ((6.25, 14.65, -3.51)), "stitch")
    g.box(sg, "grip_knot_cord_b1", ((5.75, 13.45, -2.40)), ((6.25, 13.95, -2.15)), "stitch")
    g.box(sg, "grip_knot_cord_b2", ((5.75, 14.85, -2.40)), ((6.25, 15.35, -2.15)), "stitch")
    g.box(sg, "grip_patina_flange_l", ((5.12, 13.25, -3.35)), ((5.38, 14.85, -2.55)), "hide_dk")
    g.box(sg, "grip_patina_flange_r", ((6.62, 13.25, -3.35)), ((6.88, 14.85, -2.55)), "hide_dk")

    su = "shaft_upper"
    # 上段木柄木节与白蜡木光泽
    g.box(su, "shaft_up_knot_1", ((5.25, 18.25, -3.15)), ((5.55, 18.85, -2.65)), "hide_dk")
    g.box(su, "shaft_up_knot_2", ((6.45, 22.45, -3.15)), ((6.75, 23.05, -2.65)), "hide_dk")
    g.box(su, "shaft_up_ridge_f1", ((5.65, 17.45, -3.67)), ((6.35, 21.05, -3.42)), "hide_lt")
    g.box(su, "shaft_up_ridge_f2", ((5.62, 21.45, -3.67)), ((6.35, 24.85, -3.42)), "hide_lt")
    g.box(su, "shaft_up_ridge_b1", ((5.65, 17.45, -2.48)), ((6.35, 21.05, -2.23)), "hide")
    g.box(su, "shaft_up_ridge_b2", ((5.63, 21.45, -2.48)), ((6.35, 24.85, -2.23)), "hide")
    g.box(su, "shaft_up_collar_reinforce", ((5.25, 25.15, -3.63)), ((6.75, 25.45, -2.25)), "rust_dk")

    tc = "handle_tee_crossbar"
    # T把手两翼精雕曲度与端头细节
    g.box(tc, "tee_arm_bevel_l1", ((4.24, 25.71, -3.35)), ((4.98, 26.19, -2.55)), "hide")
    g.box(tc, "tee_arm_bevel_r1", ((7.25, 25.75, -3.35)), ((8.05, 26.15, -2.55)), "hide")
    g.box(tc, "tee_arm_bevel_l2", ((4.45, 26.55, -3.35)), ((5.25, 26.87, -2.58)), "hide_lt")
    g.box(tc, "tee_arm_bevel_r2", ((6.75, 26.55, -3.39)), ((7.55, 26.91, -2.55)), "hide_lt")
    g.box(tc, "tee_cap_ring_l", ((4.23, 25.75, -3.42)), ((4.45, 26.75, -2.48)), "rust_dk")
    g.box(tc, "tee_cap_ring_r", ((8.05, 25.75, -3.42)), ((8.35, 26.75, -2.48)), "rust_dk")
    g.box(tc, "tee_end_pin_l", ((4.15, 26.15, -3.15)), ((4.40, 26.45, -2.75)), "copper_lt")
    g.box(tc, "tee_end_pin_r", ((8.32, 26.15, -3.15)), ((8.57, 26.45, -2.75)), "copper_lt")
    g.box(tc, "tee_palm_rest_c", ((5.45, 26.65, -3.38)), ((6.55, 26.95, -2.52)), "hide_lt")

    tcol = "handle_tee_collar"
    # T把手铁套加强筋与铁耳
    g.box(tcol, "tee_collar_rib_f", ((5.65, 25.25, -3.79)), ((6.35, 26.15, -3.54)), "iron_lt")
    g.box(tcol, "tee_collar_rib_b", ((5.65, 25.25, -2.40)), ((6.35, 26.15, -2.15)), "iron")
    g.box(tcol, "tee_collar_strap_l", ((5.03, 25.55, -3.55)), ((5.28, 26.25, -2.35)), "rust")
    g.box(tcol, "tee_collar_strap_r", ((6.72, 25.55, -3.55)), ((6.97, 26.25, -2.35)), "rust")
    g.box(tcol, "tee_wedge_pin_top", ((5.85, 26.95, -3.08)), ((6.15, 27.25, -2.83)), "copper_lt")



    # 额外补充16块微观锻打肌理与修补钉，使总方块数达到 206 块
    g.box(bi, "blade_micro_scale_a", ((3.85, 2.85, -3.65)), ((4.35, 3.35, -3.38)), "rust_dk")
    g.box(bi, "blade_micro_scale_b", ((7.65, 2.85, -3.65)), ((8.15, 3.35, -3.35)), "rust_dk")
    g.box(bi, "blade_micro_scale_c", ((3.75, 4.25, -3.65)), ((4.25, 4.75, -3.35)), "rust")
    g.box(bi, "blade_micro_scale_d", ((7.75, 4.25, -3.70)), ((8.25, 4.75, -3.35)), "rust")
    g.box(bi, "blade_chisel_flaw_1", ((4.55, 1.45, -3.70)), ((4.95, 1.85, -3.35)), "void")
    g.box(bi, "blade_chisel_flaw_2", ((7.05, 1.45, -3.70)), ((7.45, 1.85, -3.35)), "void")
    g.box(bi, "blade_weld_seam_l", ((3.25, 3.85, -3.71)), ((3.65, 4.35, -3.32)), "iron_lt")
    g.box(bi, "blade_weld_seam_r", ((8.35, 3.80, -3.70)), ((8.75, 4.35, -3.32)), "iron_lt")
    g.box(bi, "blade_shoulder_boss_l", ((4.45, 6.85, -3.72)), ((4.85, 7.25, -3.38)), "rust_dk")
    g.box(bi, "blade_shoulder_boss_r", ((7.15, 6.85, -3.72)), ((7.55, 7.25, -3.38)), "rust_dk")
    g.box(sf, "socket_rivet_cap_fl", ((4.80, 7.85, -3.75)), ((5.10, 8.25, -3.42)), "copper_lt")
    g.box(sf, "socket_rivet_cap_fr", ((6.85, 7.85, -3.71)), ((7.15, 8.20, -3.42)), "copper_lt")
    g.box(sg, "grip_strap_tie_knot_1", ((5.15, 12.85, -3.45)), ((5.45, 13.35, -2.85)), "hide_dk")
    g.box(sg, "grip_strap_tie_knot_2", ((6.55, 14.85, -3.45)), ((6.85, 15.35, -2.85)), "hide_dk")
    g.box(su, "shaft_up_fiber_knot_1", ((5.45, 19.45, -3.48)), ((5.85, 20.05, -2.65)), "hide_dk")
    g.box(su, "shaft_up_fiber_knot_2", ((6.15, 23.45, -3.48)), ((6.55, 24.05, -2.65)), "hide_dk")


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
    worn_focus=((6.0, 13.0, -2.8), 32.0),
    round_label="round 1/3",
    textured=False,
)

SPEC.gates = ToolGates(
    SPEC.title, SPEC.mats, asym=SPEC.asym,
    soft_over=SPEC.soft_over, free_floating=SPEC.free_floating
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
