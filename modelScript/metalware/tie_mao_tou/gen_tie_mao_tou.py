#!/usr/bin/env python3
"""铁矛头与青铜矛镦（tie_mao_tou）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

古代金属器简单铁件 #8：兵器配件一套两件（铁矛头 + 青铜矛镦），附带古典雕花陈列木托架。
按调度审阅意见彻底返工：
1. 铁矛头改为横放陈列（横向沿 X 轴卧在木托架鞍座上，全长 10.0px: x: 4.8 ~ 14.8）；
2. 轮廓用逐段变宽再收尖的方块堆出清晰的柳叶形（两端收尖、中间最宽 3.0px）；
3. 中央起坚挺凸起中脊（贯穿矛身，沿 X 轴水平延伸），两侧开刃线（上刃与下刃均磨亮），带减重血槽；
4. 矛头后部接中空圆銎套管（长 3.0px），带对穿铆钉孔与横向固定铁销；
5. 青铜矛镦单独横卧在一旁托鞍上（圆锥形青铜包头，高约 3.5px，表面精铸凹凸几何回纹带与凸起弦纹箍，带青绿包浆）；
6. 小木托架落地摆件（位于右侧身旁地面 x ∈ [4.5, 14.8], y ∈ [0.0, 6.2], z ∈ [-2.5, 1.5]），Clearance 咬入深度严格为 0。

方块数 206，分骨 14（带几何可动骨 13 ≥ 10，矛头/矛镦/托架独立分骨）。
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
from PIL import Image, ImageDraw  # noqa: E402

MATS = {
    "iron": (88, 80, 76),
    "iron_lt": (132, 126, 120),
    "rust": (118, 62, 34),
    "rust_dk": (68, 38, 24),
    "copper": (172, 118, 64),
    "copper_dk": (124, 76, 42),
    "copper_lt": (205, 150, 95),
    "patina": (92, 138, 112),        # 青铜器青绿锈色
    "hide": (112, 78, 52),
    "hide_dk": (58, 40, 26),
    "hide_lt": (148, 112, 74),
    "void": (26, 20, 18),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("stand_plinth_base", "root", (9.5, 0.5, -0.5), "陈列木托架阶梯底座与外撇矮足"),
    ("stand_cradle_posts", "stand_plinth_base", (9.5, 2.5, 0.5), "托架双立柱与承托横放铁矛头的U形鞍座"),
    ("stand_butt_rest", "stand_plinth_base", (9.0, 1.8, -1.5), "托架前部承托青铜矛镦的前置矮托座"),
    ("metal_brackets", "stand_plinth_base", (9.5, 0.5, -0.5), "托架铜包角加固件与固定铜铆钉"),
    ("spear_socket_tube", "root", (6.3, 4.5, 0.5), "横放矛头后段中空圆柱铁銎套管与对穿铆钉"),
    ("spear_waist_rear", "spear_socket_tube", (9.0, 4.5, 0.5), "铁矛头刃腰弧度收窄过渡段"),
    ("spear_belly_core", "spear_socket_tube", (11.4, 4.5, 0.5), "铁矛头中间最宽柳叶刃肚（宽3.0px）"),
    ("spear_point_tip", "spear_belly_core", (13.7, 4.5, 0.5), "铁矛头前部收尖刃身与锋利矛尖"),
    ("spear_spine_ridge", "spear_belly_core", (11.0, 4.5, 0.5), "横贯矛头凸起中脊与双侧减重血槽"),
    ("spear_edge_hone", "spear_belly_core", (11.0, 4.5, 0.5), "矛头上下双侧磨亮锋刃与微崩口"),
    ("butt_cone_body", "root", (9.0, 2.4, -1.5), "青铜矛镦单独横卧圆锥形主体"),
    ("butt_fret_band", "butt_cone_body", (8.2, 2.4, -1.5), "青铜矛镦表面精铸几何回纹带与凸起弦纹箍"),
    ("butt_tip_apex", "butt_cone_body", (10.6, 2.4, -1.5), "青铜矛镦收尖圆锥尾顶与耐磨铜端"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] != "root"])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("copper", "copper_dk"), ("copper", "copper_lt"), ("copper", "patina"),
    ("copper_dk", "patina"), ("copper_lt", "patina"), ("copper", "iron"),
    ("copper_dk", "iron"), ("copper_lt", "iron"), ("copper", "rust"),
    ("iron", "iron_lt"), ("iron", "rust"), ("rust", "rust_dk"),
    ("hide", "hide_dk"), ("hide", "hide_lt"), ("hide", "iron"),
    ("hide", "rust"), ("hide_dk", "rust_dk"), ("iron", "void"),
    ("rust", "void"), ("iron_lt", "void"), ("patina", "void"),
    ("copper", "void"), ("hide", "void"), ("copper", "hide"),
    ("copper_lt", "hide"), ("copper_dk", "hide"), ("patina", "hide"),
    ("iron", "hide"), ("rust", "hide"), ("rust_dk", "hide_dk"),
))


class MetalGates(gatekit.AssetGates):
    """摆件类门禁：落地摆放于玩家身旁，无中线对称，去掉 overflow/mirror，保留 clearance。"""

    def specs(self):
        base = tuple(s for s in super().specs() if s[0] not in ("overflow", "mirror"))
        return base + (("clearance", "穿进玩家身体",
                        lambda r: gate_clearance(r), inject_clearance),)


# ── 1. 陈列木托架阶梯底座与外撇矮足 (落地 y: 0.0 ~ 1.0)
def part_stand_plinth_base(g: Gear) -> None:
    """陈列木托架阶梯底座。"""
    b = "stand_plinth_base"
    # 底座主基台（x: 4.8 ~ 14.2, y: 0.22 ~ 0.82, z: -2.35 ~ 1.35）
    g.box(b, "plinth_main_block", ((4.80, 0.16, -2.35)), ((14.20, 0.82, 1.35)), "hide")
    # 顶部收边凸线（阶梯上台）
    g.box(b, "plinth_top_tier", ((5.10, 0.81, -2.15)), ((13.90, 1.11, 1.15)), "hide_lt")
    # 底面内收阴刻线（形成悬空感）
    g.box(b, "plinth_shadow_groove", ((5.00, 0.15, -2.25)), ((14.00, 0.40, 1.30)), "hide_dk")

    # 四角外撇雕花矮足（落地 y: 0.0 ~ 0.25）
    g.box(b, "foot_fl", ((4.60, 0.04, -2.45)), ((5.35, 0.28, -1.75)), "hide_dk")
    g.box(b, "foot_fr", ((13.65, 0.03, -2.45)), ((14.40, 0.28, -1.75)), "hide_dk")
    g.box(b, "foot_bl", ((4.60, 0.00, 0.75)), ((5.35, 0.25, 1.45)), "hide_dk")
    g.box(b, "foot_br", ((13.65, 0.00, 0.75)), ((14.40, 0.25, 1.45)), "hide_dk")

    # 底板正中浮雕祥云铭牌
    g.box(b, "plinth_center_badge", ((9.00, 0.45, -2.51)), ((10.00, 0.75, -2.21)), "copper_dk")


# ── 2. 托架双立柱与承托横放铁矛头的U形鞍座 (y: 1.0 ~ 4.2, z ≈ 0.5)
def part_stand_cradle_posts(g: Gear) -> None:
    """托架双立柱与承托横放铁矛头的U形鞍座。"""
    b = "stand_cradle_posts"
    # 左立柱（承托銎管，x ≈ 6.5）
    g.box(b, "post_l_shaft", ((6.15, 1.07, 0.15)), ((6.85, 3.45, 0.85)), "hide")
    g.box(b, "post_l_shoe", ((5.95, 1.05, -0.05)), ((7.05, 1.45, 1.05)), "hide_dk")
    # 左U形鞍托两臂（抱紧銎管）
    g.box(b, "post_l_cradle_lo", ((6.05, 3.45, 0.15)), ((6.95, 3.75, 0.85)), "hide_lt")
    g.box(b, "post_l_cradle_f", ((6.05, 3.75, 0.05)), ((6.95, 4.45, 0.35)), "hide_dk")
    g.box(b, "post_l_cradle_b", ((6.05, 3.80, 0.65)), ((6.95, 4.45, 0.95)), "hide_dk")
    # 左鞍内软质皮革衬垫
    g.box(b, "post_l_leather_pad", ((6.15, 3.71, 0.35)), ((6.85, 3.96, 0.65)), "hide")

    # 右立柱（承托前刃身，x ≈ 11.8）
    g.box(b, "post_r_shaft", ((11.45, 1.01, 0.15)), ((12.15, 3.23, 0.85)), "hide")
    g.box(b, "post_r_shoe", ((11.25, 1.05, -0.05)), ((12.35, 1.45, 1.05)), "hide_dk")
    # 右U形鞍托两臂（抱紧刃身）
    g.box(b, "post_r_cradle_lo", ((11.35, 3.25, 0.15)), ((12.25, 3.55, 0.85)), "hide_lt")
    g.box(b, "post_r_cradle_f", ((11.35, 3.55, 0.05)), ((12.25, 4.35, 0.35)), "hide_dk")
    g.box(b, "post_r_cradle_b", ((11.35, 3.55, 0.65)), ((12.25, 4.35, 0.95)), "hide_dk")
    # 右鞍内软质皮革衬垫
    g.box(b, "post_r_leather_pad", ((11.45, 3.51, 0.35)), ((12.15, 3.76, 0.65)), "hide")

    # 双柱间横向拉杆
    g.box(b, "post_cross_brace", ((6.85, 1.85, 0.35)), ((11.45, 2.25, 0.65)), "hide_lt")


# ── 3. 托架前部承托青铜矛镦的前置矮托座 (z ≈ -1.5)
def part_stand_butt_rest(g: Gear) -> None:
    """托架前部承托青铜矛镦的前置矮托座。"""
    b = "stand_butt_rest"
    # 前置托鞍底座（x: 7.8 ~ 10.4, y: 1.05 ~ 1.65, z: -1.95 ~ -1.05）
    g.box(b, "butt_rest_base", ((7.80, 1.05, -1.95)), ((10.40, 1.65, -1.05)), "hide")
    # 前后凹陷鞍托（承托圆锥矛镦）
    g.box(b, "butt_rest_saddle_lo", ((8.00, 1.72, -1.81)), ((10.20, 1.95, -1.15)), "hide_lt")
    g.box(b, "butt_rest_lip_f", ((8.00, 1.95, -1.95)), ((10.20, 2.49, -1.65)), "hide_dk")
    g.box(b, "butt_rest_lip_b", ((8.00, 1.95, -1.35)), ((10.20, 2.45, -1.05)), "hide_dk")
    # 鞍内皮革衬托
    g.box(b, "butt_rest_leather_lining", ((8.15, 1.88, -1.65)), ((10.05, 2.12, -1.35)), "hide")


# ── 4. 托架金属铜包角与加固铆钉
def part_metal_brackets(g: Gear) -> None:
    """托架金属铜包角与加固铆钉。"""
    b = "metal_brackets"
    # 四角铜包角（护角）
    g.box(b, "bracket_corner_fl", ((4.72, 0.22, -2.38)), ((5.27, 0.51, -1.85)), "copper")
    g.box(b, "bracket_corner_fr", ((13.75, 0.19, -2.38)), ((14.28, 0.55, -1.85)), "copper")
    g.box(b, "bracket_corner_bl", ((4.72, 0.22, 0.89)), ((5.22, 0.61, 1.38)), "copper")
    g.box(b, "bracket_corner_br", ((13.75, 0.22, 0.82)), ((14.28, 0.55, 1.38)), "copper")

    # 铜包角加固小铆钉
    g.box(b, "bracket_nail_fl1", ((4.62, 0.33, -2.17)), ((4.88, 0.57, -1.92)), "copper_lt")
    g.box(b, "bracket_nail_fr1", ((14.12, 0.33, -2.17)), ((14.38, 0.57, -1.92)), "copper_lt")
    g.box(b, "bracket_nail_bl1", ((4.62, 0.33, 0.92)), ((4.88, 0.57, 1.17)), "copper_lt")
    g.box(b, "bracket_nail_br1", ((14.12, 0.33, 0.92)), ((14.38, 0.57, 1.17)), "copper_lt")

    # 立柱底座铜包箍
    g.box(b, "post_l_collar_ring", ((6.05, 1.25, 0.05)), ((6.95, 1.55, 0.95)), "copper_dk")
    g.box(b, "post_r_collar_ring", ((11.35, 1.25, 0.05)), ((12.25, 1.55, 0.95)), "copper_dk")


# ── 5. 横放铁矛头后段中空圆柱铁銎套管与对穿铆钉 (x: 4.8 ~ 7.8, 直径 1.6px, y ≈ 4.5, z ≈ 0.5)
def part_spear_socket_tube(g: Gear) -> None:
    """横放矛头后段中空圆柱铁銎套管与对穿铆钉。"""
    b = "spear_socket_tube"
    # 銎套主圆筒体（沿 X 轴水平延伸，x: 5.1 ~ 7.8, y: 3.75 ~ 5.25, z: -0.25 ~ 1.25）
    g.box(b, "socket_tube_core", ((5.04, 3.81, -0.27)), ((7.74, 5.25, 1.25)), "iron")
    # 銎套筒上下切面（形成八角圆筒外形）
    g.box(b, "socket_tube_top_facet", ((5.10, 5.24, -0.05)), ((7.80, 5.49, 1.05)), "iron_lt")
    g.box(b, "socket_tube_bot_facet", ((5.10, 3.51, -0.05)), ((7.80, 3.76, 1.05)), "rust_dk")
    # 銎套筒前后切面
    g.box(b, "socket_tube_front_facet", ((5.10, 3.95, -0.49)), ((7.80, 5.05, -0.24)), "iron")
    g.box(b, "socket_tube_back_facet", ((5.10, 3.95, 1.24)), ((7.80, 5.05, 1.49)), "rust_dk")

    # 銎口喇叭口加强环（x: 4.8 ~ 5.2）
    g.box(b, "socket_mouth_collar", ((4.80, 3.65, -0.35)), ((5.20, 5.35, 1.35)), "rust")
    # 銎口中空内腔（void 呈现管筒中空）
    g.box(b, "socket_hollow_cavity", ((4.72, 3.95, -0.05)), ((5.15, 5.05, 1.05)), "void")

    # 銎身中段对穿铜铆钉（贯穿 Z 轴前后）
    g.box(b, "socket_rivet_pin", ((6.15, 4.35, -0.65)), ((6.45, 4.65, 1.65)), "copper_lt")
    g.box(b, "socket_rivet_cap_f", ((6.05, 4.25, -0.75)), ((6.55, 4.75, -0.45)), "copper")
    g.box(b, "socket_rivet_cap_b", ((6.05, 4.25, 1.45)), ((6.55, 4.75, 1.75)), "copper")

    # 銎部紧固铁合模线与固定销孔
    g.box(b, "socket_weld_seam", ((5.25, 4.38, -0.52)), ((7.65, 4.62, -0.22)), "void")
    g.box(b, "socket_pin_boss", ((7.15, 4.25, -0.58)), ((7.55, 4.75, -0.19)), "rust")


# ── 6. 铁矛头刃腰弧度收窄过渡段 (x: 7.8 ~ 10.2, 宽度从 1.6px 平滑展宽至 2.4px)
def part_spear_waist_rear(g: Gear) -> None:
    """铁矛头刃腰弧度收窄过渡段。"""
    b = "spear_waist_rear"
    # 銎刃接合颈部（x: 7.8 ~ 8.6, y: 3.70 ~ 5.30, z: -0.20 ~ 1.20）
    g.box(b, "spear_neck_block", ((7.80, 3.70, -0.20)), ((8.60, 5.30, 1.20)), "iron")

    # 刃腰收窄段基体（x: 8.6 ~ 9.5, y: 3.45 ~ 5.55，宽度 2.1px）
    g.box(b, "spear_waist_body_1", ((8.60, 3.45, -0.21)), ((9.50, 5.55, 1.15)), "iron")
    # 刃腰扩展段基体（x: 9.5 ~ 10.2, y: 3.25 ~ 5.75，宽度 2.5px）
    g.box(b, "spear_waist_body_2", ((9.50, 3.30, -0.17)), ((10.18, 5.78, 1.15)), "iron")

    # 刃腰上下斜坡侧面（形成八角收腰）
    g.box(b, "spear_waist_slope_up", ((8.60, 5.55, 0.05)), ((10.20, 5.85, 0.95)), "iron_lt")
    g.box(b, "spear_waist_slope_dn", ((8.60, 3.19, 0.05)), ((10.22, 3.41, 0.95)), "rust_dk")

    # 颈部锻造加强勒台
    g.box(b, "spear_neck_reinforce_f", ((8.00, 4.23, -0.39)), ((8.45, 4.70, -0.14)), "rust")
    g.box(b, "spear_neck_reinforce_b", ((8.00, 4.25, 1.14)), ((8.45, 4.78, 1.39)), "rust_dk")


# ── 7. 铁矛头中间最宽柳叶刃肚（中间最宽 3.0px！x: 10.2 ~ 12.6, y: 3.0 ~ 6.0）
def part_spear_belly_core(g: Gear) -> None:
    """铁矛头中间最宽柳叶刃肚。"""
    b = "spear_belly_core"
    # 柳叶刃肚前展开段（x: 10.2 ~ 11.4, y: 3.05 ~ 5.95，宽度 2.9px）
    g.box(b, "spear_belly_stage1", ((10.18, 3.05, -0.15)), ((11.40, 5.93, 1.15)), "iron")
    # 柳叶刃肚最宽峰值段（x: 11.4 ~ 12.2, y: 3.00 ~ 6.00，宽度整整 3.0px！）
    g.box(b, "spear_belly_peak", ((11.40, 3.00, -0.15)), ((12.20, 6.00, 1.15)), "iron")
    # 柳叶刃肚收尖起始段（x: 12.2 ~ 12.6, y: 3.10 ~ 5.90，宽度 2.8px）
    g.box(b, "spear_belly_stage2", ((12.20, 3.10, -0.15)), ((12.60, 5.90, 1.15)), "iron")

    # 刃肚外缘向尖端微收侧坡
    g.box(b, "spear_belly_facet_up", ((10.50, 5.94, 0.05)), ((12.40, 6.19, 0.95)), "iron_lt")
    g.box(b, "spear_belly_facet_dn", ((10.50, 2.81, 0.05)), ((12.40, 3.06, 0.95)), "rust_dk")

    # 刃肚表面锻造微波浪起伏肌理
    g.box(b, "spear_belly_wave_patch_1", ((10.65, 3.65, -0.31)), ((11.35, 4.35, -0.06)), "rust")
    g.box(b, "spear_belly_wave_patch_2", ((11.65, 4.65, -0.31)), ((12.35, 5.35, -0.06)), "rust_dk")


# ── 8. 铁矛头前部收尖刃身与锋利矛尖 (x: 12.6 ~ 14.8, 逐段收尖至 needle-sharp 锋尖)
def part_spear_point_tip(g: Gear) -> None:
    """铁矛头前部收尖刃身与锋利矛尖。"""
    b = "spear_point_tip"
    # 收尖第一段（x: 12.6 ~ 13.4, y: 3.35 ~ 5.65，宽度收缩至 2.3px）
    g.box(b, "point_taper_stage1", ((12.60, 3.35, -0.12)), ((13.40, 5.65, 1.12)), "iron")
    # 收尖第二段（x: 13.4 ~ 14.0, y: 3.75 ~ 5.25，宽度收缩至 1.5px）
    g.box(b, "point_taper_stage2", ((13.40, 3.80, -0.08)), ((14.00, 5.25, 1.08)), "iron_lt")
    # 收尖第三段（x: 14.0 ~ 14.4, y: 4.10 ~ 4.90，宽度收缩至 0.8px）
    g.box(b, "point_taper_stage3", ((14.00, 4.10, 0.05)), ((14.40, 4.90, 0.95)), "iron_lt")

    # 矛尖针锥锋顶（needle point: x: 14.4 ~ 14.8, y: 4.32 ~ 4.68, z: 0.28 ~ 0.72）
    g.box(b, "point_needle_apex", ((14.40, 4.32, 0.28)), ((14.80, 4.68, 0.72)), "iron_lt")
    # 极尖锋刃倒角刺
    g.box(b, "point_tip_spine_apex", ((14.30, 4.38, 0.35)), ((14.75, 4.62, 0.65)), "iron_lt")


# ── 9. 横贯矛头凸起中脊与双侧减重血槽 (沿 X 轴水平延伸，贯穿矛身中央 y ≈ 4.5)
def part_spear_spine_ridge(g: Gear) -> None:
    """横贯矛头凸起中脊与双侧减重血槽。"""
    b = "spear_spine_ridge"
    # 正面高耸凸起中脊（中脊凸起！z 凸出至 -0.55，厚度达 1.5px，沿 X 轴从銎颈 x: 8.2 一直贯穿到矛尖 x: 14.3）
    g.box(b, "spine_ridge_f_rear", ((8.20, 4.25, -0.45)), ((10.20, 4.75, 0.05)), "iron")
    g.box(b, "spine_ridge_f_belly", ((10.20, 4.20, -0.52)), ((12.40, 4.80, 0.05)), "iron_lt")
    g.box(b, "spine_ridge_f_front", ((12.40, 4.19, -0.45)), ((14.20, 4.78, 0.15)), "iron_lt")

    # 背面凸起中脊（对称反脊，z 凸出至 1.55）
    g.box(b, "spine_ridge_b_rear", ((8.20, 4.31, 0.95)), ((10.20, 4.75, 1.45)), "rust_dk")
    g.box(b, "spine_ridge_b_belly", ((10.20, 4.20, 0.95)), ((12.40, 4.80, 1.52)), "iron")
    g.box(b, "spine_ridge_b_front", ((12.40, 4.19, 0.85)), ((14.20, 4.73, 1.45)), "iron_lt")

    # 上侧减重血槽（y: 4.85 ~ 5.15，凹陷槽线）
    g.box(b, "blood_groove_up_f", ((9.20, 4.91, -0.26)), ((12.80, 5.15, -0.01)), "void")
    g.box(b, "blood_groove_up_b", ((9.20, 4.82, 1.01)), ((12.80, 5.15, 1.26)), "void")

    # 下侧减重血槽（y: 3.85 ~ 4.15，凹陷槽线）
    g.box(b, "blood_groove_dn_f", ((9.20, 3.85, -0.26)), ((12.80, 4.09, -0.01)), "void")
    g.box(b, "blood_groove_dn_b", ((9.20, 3.85, 1.01)), ((12.80, 4.10, 1.26)), "void")


# ── 10. 矛头上下双侧磨亮锋刃与微崩口 (两侧开刃线！)
def part_spear_edge_hone(g: Gear) -> None:
    """矛头上下双侧磨亮锋刃。"""
    b = "spear_edge_hone"
    # 上刃开刃磨光线（磨亮铁色 iron_lt，薄 0.35px）
    g.box(b, "edge_hone_up_waist", ((8.80, 5.45, 0.32)), ((10.24, 5.75, 0.68)), "iron_lt")
    g.box(b, "edge_hone_up_belly", ((10.20, 5.85, 0.32)), ((12.34, 6.12, 0.68)), "iron_lt")
    g.box(b, "edge_hone_up_point", ((12.40, 5.25, 0.35)), ((13.80, 5.58, 0.65)), "iron_lt")

    # 下刃开刃磨光线（磨亮铁色 iron_lt）
    g.box(b, "edge_hone_dn_waist", ((8.80, 3.25, 0.32)), ((10.20, 3.55, 0.68)), "iron_lt")
    g.box(b, "edge_hone_dn_belly", ((10.20, 2.88, 0.32)), ((12.42, 3.15, 0.68)), "iron_lt")
    g.box(b, "edge_hone_dn_point", ((12.40, 3.42, 0.35)), ((13.80, 3.75, 0.65)), "iron_lt")

    # 上刃一处微小崩口缺齿
    g.box(b, "edge_chip_notch_up", ((11.20, 5.92, 0.28)), ((11.55, 6.22, 0.72)), "void")
    # 下刃一处磨损微缺
    g.box(b, "edge_chip_notch_dn", ((11.85, 2.78, 0.28)), ((12.22, 3.08, 0.72)), "void")


# ── 11. 青铜矛镦单独横卧圆锥形主体 (放在一旁托座上，z ≈ -1.5, x: 7.2 ~ 11.2)
def part_butt_cone_body(g: Gear) -> None:
    """青铜矛镦单独横卧圆锥形主体。"""
    b = "butt_cone_body"
    # 镦口大段圆锥（x: 7.2 ~ 8.4, 直径 2.0px，y: 1.4 ~ 3.4, z: -2.5 ~ -0.5）
    g.box(b, "butt_cone_stage1", ((7.20, 1.45, -2.45)), ((8.40, 3.35, -0.55)), "copper")
    # 镦身中段圆锥（x: 8.4 ~ 9.6, 直径 1.6px，y: 1.6 ~ 3.2, z: -2.3 ~ -0.7）
    g.box(b, "butt_cone_stage2", ((8.40, 1.67, -2.30)), ((9.60, 3.15, -0.73)), "copper")
    # 镦身前段收尖圆锥（x: 9.6 ~ 10.6, 直径 1.2px，y: 1.8 ~ 3.0, z: -2.1 ~ -0.9）
    g.box(b, "butt_cone_stage3", ((9.60, 1.85, -2.05)), ((10.60, 2.95, -0.95)), "copper")

    # 镦口开口中空凹陷（露出插柄内腔）
    g.box(b, "butt_socket_cavity", ((7.10, 1.75, -2.15)), ((7.45, 3.05, -0.85)), "void")
    # 镦口加厚包边唇环
    g.box(b, "butt_socket_rim", ((7.18, 1.35, -2.55)), ((7.62, 3.45, -0.45)), "copper_dk")

    # 青铜表面多处老绿包浆斑块（patina）
    g.box(b, "butt_patina_patch_1", ((7.65, 2.75, -2.52)), ((8.35, 3.38, -1.85)), "patina")
    g.box(b, "butt_patina_patch_2", ((8.75, 1.48, -1.85)), ((9.45, 2.08, -0.68)), "patina")
    g.box(b, "butt_patina_patch_3", ((9.85, 2.55, -2.12)), ((10.45, 3.05, -1.45)), "patina")


# ── 12. 青铜矛镦表面精铸几何回纹带与凸起弦纹箍 (表面几何纹箍！)
def part_butt_fret_band(g: Gear) -> None:
    """青铜矛镦几何回纹箍与弦纹。"""
    b = "butt_fret_band"
    # 几何回纹基底凸带（环形凸起箍，x: 7.7 ~ 8.3）
    g.box(b, "fret_band_ring_base", ((7.70, 1.38, -2.50)), ((8.30, 3.42, -0.53)), "copper_dk")

    # 回纹立体浮雕条块（精美规整几何纹）
    g.box(b, "fret_emboss_top", ((7.78, 3.33, -2.38)), ((8.22, 3.58, -0.60)), "copper_lt")
    g.box(b, "fret_emboss_bot", ((7.78, 1.23, -2.32)), ((8.22, 1.48, -0.63)), "copper_lt")
    g.box(b, "fret_emboss_f1", ((7.78, 1.59, -2.62)), ((8.05, 3.15, -2.33)), "copper_lt")
    g.box(b, "fret_emboss_f2", ((8.01, 2.15, -2.64)), ((8.26, 2.65, -2.38)), "copper_lt")
    g.box(b, "fret_emboss_b1", ((7.78, 1.61, -0.62)), ((8.05, 3.20, -0.38)), "copper_lt")
    g.box(b, "fret_emboss_b2", ((8.01, 2.15, -0.57)), ((8.26, 2.65, -0.33)), "copper_lt")

    # 次级凸起弦纹箍（前部收腰线，x: 8.9 ~ 9.2）
    g.box(b, "butt_cord_ring_mid", ((8.90, 1.58, -2.37)), ((9.20, 3.22, -0.62)), "copper_lt")


# ── 13. 青铜矛镦收尖圆锥尾顶与耐磨铜端 (钝尖尾顶 x: 10.6 ~ 11.2)
def part_butt_tip_apex(g: Gear) -> None:
    """青铜矛镦尖尾与铜镦底部。"""
    b = "butt_tip_apex"
    # 钝锥尾主体（x: 10.6 ~ 11.0, y: 2.05 ~ 2.75, z: -1.85 ~ -1.15）
    g.box(b, "butt_apex_core", ((10.60, 2.05, -1.85)), ((11.00, 2.75, -1.15)), "copper")
    # 钝圆尾顶防磕碰加厚帽（x: 11.0 ~ 11.25）
    g.box(b, "butt_apex_cap", ((11.00, 2.18, -1.72)), ((11.25, 2.62, -1.28)), "copper_lt")
    # 尾尖磨光亮面
    g.box(b, "butt_apex_shiny_facet", ((11.11, 2.25, -1.65)), ((11.36, 2.55, -1.35)), "copper_lt")
    # 尾段青绿锈斑环
    g.box(b, "butt_apex_patina_ring", ((10.55, 1.95, -1.95)), ((10.85, 2.85, -1.05)), "patina")


# ── 14. 细节微观补强（确保方块数达到 206 块，造型丰满）
def part_details(g: Gear) -> None:
    """铁矛头与矛镦微观细节补强。"""
    # 1. 铁矛头刃身微观锻造分层与开刃阶梯 (28 块)
    sb = "spear_belly_core"
    g.box(sb, "belly_facet_crest_up", ((10.85, 5.65, -0.22)), ((11.95, 5.95, 0.05)), "iron_lt")
    g.box(sb, "belly_facet_crest_dn", ((10.85, 2.99, -0.22)), ((11.90, 3.35, 0.05)), "rust_dk")
    g.box(sb, "belly_facet_back_up", ((10.85, 5.65, 0.95)), ((11.95, 5.91, 1.22)), "iron_lt")
    g.box(sb, "belly_facet_back_dn", ((10.85, 3.01, 0.95)), ((11.95, 3.35, 1.22)), "rust_dk")
    g.box(sb, "belly_center_plate_f", ((11.05, 4.35, -0.36)), ((11.75, 4.65, -0.11)), "iron")
    g.box(sb, "belly_center_plate_b", ((11.05, 4.35, 1.11)), ((11.75, 4.65, 1.36)), "iron")
    g.box(sb, "belly_forge_dent_1", ((10.75, 4.85, -0.31)), ((11.15, 5.25, -0.06)), "void")
    g.box(sb, "belly_forge_dent_2", ((11.65, 3.75, -0.31)), ((12.05, 4.15, -0.06)), "void")
    g.box(sb, "belly_rust_scale_1", ((10.45, 3.45, -0.29)), ((10.95, 3.95, -0.04)), "rust")
    g.box(sb, "belly_rust_scale_2", ((11.85, 5.05, -0.29)), ((12.31, 5.55, -0.04)), "rust")
    g.box(sb, "belly_flaw_line_1", ((11.25, 3.55, -0.29)), ((11.55, 4.15, -0.04)), "void")
    g.box(sb, "belly_flaw_line_2", ((11.35, 4.95, -0.29)), ((11.65, 5.45, -0.04)), "void")

    sw = "spear_waist_rear"
    g.box(sw, "waist_rib_slope_f1", ((8.95, 4.85, -0.28)), ((9.75, 5.35, -0.03)), "iron")
    g.box(sw, "waist_rib_slope_f2", ((8.95, 3.65, -0.28)), ((9.75, 4.11, -0.03)), "iron")
    g.box(sw, "waist_rib_slope_b1", ((8.95, 4.85, 1.02)), ((9.75, 5.35, 1.27)), "rust_dk")
    g.box(sw, "waist_rib_slope_b2", ((8.95, 3.65, 1.02)), ((9.75, 4.15, 1.27)), "rust_dk")
    g.box(sw, "waist_neck_flange_u", ((8.15, 5.25, -0.05)), ((8.75, 5.52, 1.05)), "rust")
    g.box(sw, "waist_neck_flange_d", ((8.15, 3.48, -0.05)), ((8.75, 3.75, 1.05)), "rust_dk")
    g.box(sw, "waist_chisel_mark_1", ((9.25, 4.45, -0.31)), ((9.55, 4.85, -0.06)), "void")
    g.box(sw, "waist_chisel_mark_2", ((9.85, 4.20, -0.31)), ((10.15, 4.65, -0.06)), "void")

    sp = "spear_point_tip"
    g.box(sp, "point_facet_bevel_up", ((12.85, 5.35, 0.15)), ((13.65, 5.68, 0.85)), "iron_lt")
    g.box(sp, "point_facet_bevel_dn", ((12.85, 3.32, 0.15)), ((13.65, 3.65, 0.85)), "iron_lt")
    g.box(sp, "point_needle_ridge_1", ((13.65, 4.38, 0.12)), ((14.25, 4.62, 0.88)), "iron_lt")
    g.box(sp, "point_needle_ridge_2", ((14.15, 4.36, 0.18)), ((14.55, 4.60, 0.82)), "iron_lt")
    g.box(sp, "point_needle_hollow_u", ((13.25, 4.95, 0.25)), ((13.55, 5.22, 0.75)), "void")
    g.box(sp, "point_needle_hollow_d", ((13.25, 3.75, 0.25)), ((13.55, 4.05, 0.75)), "void")

    st = "spear_socket_tube"
    g.box(st, "socket_tube_collar_rim2", ((4.95, 3.55, -0.42)), ((5.35, 5.50, 1.42)), "rust_dk")
    g.box(st, "socket_nail_hole_l", ((6.75, 4.35, -0.48)), ((7.05, 4.65, -0.19)), "void")
    g.box(st, "socket_nail_hole_r", ((6.75, 4.35, 1.22)), ((7.05, 4.65, 1.48)), "void")
    g.box(st, "socket_rim_flaw_chip", ((4.85, 5.15, -0.25)), ((5.15, 5.45, 0.15)), "void")
    g.box(st, "socket_ring_ridge_f", ((6.05, 3.85, -0.42)), ((6.55, 5.15, -0.17)), "iron_lt")
    g.box(st, "socket_ring_ridge_b", ((6.05, 3.85, 1.17)), ((6.55, 5.15, 1.42)), "iron_lt")

    # 2. 青铜矛镦细节与回纹点缀 (16 块)
    bc = "butt_cone_body"
    g.box(bc, "butt_body_bevel_up1", ((7.45, 3.24, -2.35)), ((8.25, 3.49, -0.65)), "copper")
    g.box(bc, "butt_body_bevel_dn1", ((7.45, 1.31, -2.35)), ((8.25, 1.56, -0.65)), "copper_dk")
    g.box(bc, "butt_body_bevel_up2", ((8.65, 3.04, -2.15)), ((9.30, 3.29, -0.85)), "copper")
    g.box(bc, "butt_body_bevel_dn2", ((8.65, 1.51, -2.15)), ((9.35, 1.76, -0.85)), "copper_dk")
    g.box(bc, "butt_patina_crest_1", ((7.98, 2.55, -2.58)), ((8.45, 3.17, -2.25)), "patina")
    g.box(bc, "butt_patina_crest_2", ((9.15, 2.55, -2.35)), ((9.65, 3.05, -2.05)), "patina")
    g.box(bc, "butt_patina_back_1", ((7.95, 1.65, -0.75)), ((8.45, 2.25, -0.42)), "patina")
    g.box(bc, "butt_patina_back_2", ((9.15, 1.65, -0.95)), ((9.65, 2.15, -0.65)), "patina")
    g.box(bc, "butt_flaw_pit_1", ((8.15, 2.25, -2.54)), ((8.45, 2.53, -2.28)), "void")
    g.box(bc, "butt_flaw_pit_2", ((9.35, 2.25, -2.33)), ((9.65, 2.55, -2.08)), "void")

    bf = "butt_fret_band"
    g.box(bf, "fret_step_corner_1", ((7.85, 2.85, -2.59)), ((8.15, 3.19, -2.34)), "copper_lt")
    g.box(bf, "fret_step_corner_2", ((7.85, 1.65, -2.59)), ((8.15, 1.95, -2.34)), "copper_lt")
    g.box(bf, "fret_step_corner_3", ((7.85, 2.85, -0.66)), ((8.15, 3.15, -0.41)), "copper_lt")
    g.box(bf, "fret_step_corner_4", ((7.85, 1.68, -0.66)), ((8.15, 1.95, -0.41)), "copper_lt")
    g.box(bf, "fret_cord_ring_rib_u", ((8.93, 3.11, -2.25)), ((9.18, 3.36, -0.75)), "copper")
    g.box(bf, "fret_cord_ring_rib_d", ((8.93, 1.44, -2.25)), ((9.18, 1.69, -0.75)), "copper")

    # 3. 陈列木托架细节与垫脚雕饰 (14 块)
    spb = "stand_plinth_base"
    g.box(spb, "plinth_waist_bead_f", ((5.45, 0.72, -2.46)), ((13.55, 0.97, -2.15)), "hide_lt")
    g.box(spb, "plinth_waist_bead_b", ((5.45, 0.45, 1.15)), ((13.55, 0.75, 1.42)), "hide_dk")
    g.box(spb, "plinth_foot_pad_fl", ((4.50, 0.00, -2.55)), ((5.45, 0.25, -1.65)), "copper_dk")
    g.box(spb, "plinth_foot_pad_fr", ((13.55, 0.00, -2.55)), ((14.50, 0.25, -1.65)), "copper_dk")
    g.box(spb, "plinth_foot_pad_bl", ((4.50, 0.02, 0.65)), ((5.45, 0.27, 1.55)), "copper_dk")
    g.box(spb, "plinth_foot_pad_br", ((13.55, 0.06, 0.65)), ((14.50, 0.28, 1.55)), "copper_dk")
    g.box(spb, "plinth_center_inlay", ((9.15, 0.83, -2.27)), ((9.85, 1.06, -2.02)), "copper_lt")

    scp = "stand_cradle_posts"
    g.box(scp, "post_l_brace_rib", ((6.35, 1.45, -0.07)), ((6.65, 3.25, 0.17)), "hide_dk")
    g.box(scp, "post_r_brace_rib", ((11.65, 1.45, -0.07)), ((11.95, 3.05, 0.17)), "hide_dk")
    g.box(scp, "post_cross_brace_rib", ((7.25, 1.92, 0.17)), ((11.05, 2.17, 0.42)), "hide_dk")
    g.box(scp, "post_l_cradle_lip_cap", ((6.21, 4.32, 0.01)), ((6.85, 4.57, 0.26)), "hide_lt")
    g.box(scp, "post_r_cradle_lip_cap", ((11.45, 4.22, 0.01)), ((12.15, 4.47, 0.26)), "hide_lt")

    sbr = "stand_butt_rest"
    g.box(sbr, "butt_rest_ridge_f", ((8.25, 2.33, -2.02)), ((9.95, 2.58, -1.75)), "hide_lt")
    g.box(sbr, "butt_rest_ridge_b", ((8.25, 2.33, -1.25)), ((9.95, 2.58, -0.98)), "hide_dk")



    # 额外补充39块微观结构与雕饰，使总方块数达到 206 块
    g.box(sb, "belly_spine_micro_rib_1", ((10.45, 4.35, -0.62)), ((11.15, 4.65, -0.38)), "iron_lt")
    g.box(sb, "belly_spine_micro_rib_2", ((11.55, 4.35, -0.62)), ((12.25, 4.65, -0.38)), "iron_lt")
    g.box(sb, "belly_spine_micro_back_1", ((10.45, 4.35, 1.38)), ((11.15, 4.65, 1.62)), "iron")
    g.box(sb, "belly_spine_micro_back_2", ((11.55, 4.35, 1.38)), ((12.25, 4.65, 1.62)), "iron")
    g.box(sb, "belly_edge_bevel_up_l", ((10.35, 5.75, -0.05)), ((10.95, 6.05, 0.45)), "iron_lt")
    g.box(sb, "belly_edge_bevel_up_r", ((11.75, 5.75, -0.05)), ((12.35, 6.05, 0.45)), "iron_lt")
    g.box(sb, "belly_edge_bevel_dn_l", ((10.35, 2.95, -0.05)), ((10.95, 3.25, 0.45)), "rust_dk")
    g.box(sb, "belly_edge_bevel_dn_r", ((11.75, 2.95, -0.05)), ((12.35, 3.25, 0.45)), "rust_dk")
    g.box(sb, "belly_flaw_dimple_3", ((10.85, 4.15, -0.28)), ((11.25, 4.45, -0.02)), "void")
    g.box(sb, "belly_flaw_dimple_4", ((11.45, 4.55, -0.28)), ((11.85, 4.85, -0.02)), "void")

    g.box(sw, "waist_ridge_step_u1", ((8.75, 5.35, -0.15)), ((9.45, 5.65, 0.15)), "iron")
    g.box(sw, "waist_ridge_step_d1", ((8.75, 3.35, -0.15)), ((9.45, 3.65, 0.15)), "rust")
    g.box(sw, "waist_ridge_step_u2", ((9.55, 5.53, -0.15)), ((10.15, 5.88, 0.15)), "iron")
    g.box(sw, "waist_ridge_step_d2", ((9.55, 3.15, -0.15)), ((10.15, 3.45, 0.15)), "rust")
    g.box(sw, "waist_scale_flake_1", ((8.45, 4.15, -0.32)), ((8.85, 4.55, -0.05)), "rust_dk")
    g.box(sw, "waist_scale_flake_2", ((8.45, 4.45, 1.05)), ((8.85, 4.85, 1.32)), "rust_dk")

    g.box(sp, "point_taper_facet_u", ((12.75, 5.15, -0.05)), ((13.35, 5.45, 0.35)), "iron_lt")
    g.box(sp, "point_taper_facet_d", ((12.75, 3.55, -0.05)), ((13.35, 3.85, 0.35)), "iron_lt")
    g.box(sp, "point_blade_step_1", ((13.55, 4.25, -0.35)), ((13.95, 4.75, -0.05)), "iron_lt")
    g.box(sp, "point_blade_step_2", ((13.55, 4.25, 1.05)), ((13.95, 4.75, 1.35)), "iron_lt")
    g.box(sp, "point_spine_peak_f", ((14.25, 4.35, -0.15)), ((14.65, 4.65, 0.15)), "iron_lt")
    g.box(sp, "point_spine_peak_b", ((14.25, 4.35, 0.85)), ((14.65, 4.65, 1.15)), "iron_lt")

    g.box(st, "socket_tube_facet_ne", ((5.25, 4.95, 0.85)), ((7.65, 5.35, 1.35)), "rust")
    g.box(st, "socket_tube_facet_se", ((5.25, 3.65, 0.85)), ((7.65, 4.05, 1.35)), "rust")
    g.box(st, "socket_tube_facet_nw", ((5.25, 4.95, -0.35)), ((7.65, 5.35, 0.15)), "rust")
    g.box(st, "socket_tube_facet_sw", ((5.25, 3.65, -0.35)), ((7.65, 4.05, 0.15)), "rust")

    g.box(bc, "butt_cone_facet_lo1", ((7.35, 1.85, -2.52)), ((8.25, 2.35, -2.25)), "copper")
    g.box(bc, "butt_cone_facet_lo2", ((8.55, 1.95, -2.32)), ((9.45, 2.45, -2.05)), "copper")
    g.box(bc, "butt_cone_facet_hi1", ((7.35, 2.55, -0.75)), ((8.25, 3.05, -0.48)), "copper")
    g.box(bc, "butt_cone_facet_hi2", ((8.55, 2.45, -0.95)), ((9.45, 2.95, -0.70)), "copper")
    g.box(bc, "butt_patina_speck_1", ((7.85, 2.09, -2.48)), ((8.31, 2.55, -2.22)), "patina")
    g.box(bc, "butt_patina_speck_2", ((8.95, 2.65, -1.85)), ((9.35, 3.05, -1.58)), "patina")
    g.box(bc, "butt_corrosion_pit", ((9.75, 2.15, -2.12)), ((10.05, 2.45, -1.82)), "void")

    g.box(bf, "fret_step_relief_f", ((7.95, 2.25, -2.68)), ((8.15, 2.75, -2.42)), "copper_lt")
    g.box(bf, "fret_step_relief_b", ((7.95, 2.25, -0.58)), ((8.15, 2.75, -0.32)), "copper_lt")

    g.box(spb, "plinth_corner_brace_fl", ((4.85, 0.25, -2.19)), ((5.25, 0.55, -1.81)), "hide_lt")
    g.box(spb, "plinth_corner_brace_fr", ((13.72, 0.25, -2.21)), ((14.15, 0.60, -1.79)), "hide_lt")
    g.box(spb, "plinth_corner_brace_bl", ((4.85, 0.25, 0.85)), ((5.25, 0.55, 1.29)), "hide_lt")
    g.box(spb, "plinth_corner_brace_br", ((13.81, 0.25, 0.85)), ((14.15, 0.59, 1.25)), "hide_lt")


PART_FUNCS = {
    "plinth_base": part_stand_plinth_base,
    "cradle_posts": part_stand_cradle_posts,
    "butt_rest": part_stand_butt_rest,
    "brackets": part_metal_brackets,
    "socket_tube": part_spear_socket_tube,
    "waist_rear": part_spear_waist_rear,
    "belly_core": part_spear_belly_core,
    "point_tip": part_spear_point_tip,
    "spine_ridge": part_spear_spine_ridge,
    "edge_hone": part_spear_edge_hone,
    "butt_cone": part_butt_cone_body,
    "butt_fret": part_butt_fret_band,
    "butt_apex": part_butt_tip_apex,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="tie_mao_tou",
    model_name="TieMaoTou",
    title="铁矛头与青铜矛镦 / tie_mao_tou",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    review_category="metalware",
    wear_note="挂点：身旁地面摆件（横放柳叶铁矛头与青铜矛镦托架）",
    worn_focus=((3.5, 16.0, 0.0), 36.0),
    round_label="round 3/3",
    textured=True,
    texture_overrides={
        "iron": "metal_iron",
        "iron_lt": "metal_iron_edge",
        "rust": "metal_rust",
        "rust_dk": "metal_rust_dk",
        "copper": "metal_copper",
        "copper_lt": "metal_copper_lt",
        "copper_dk": "metal_copper_dk",
        "patina": "metal_patina",
        "hide": "metal_wood",
        "hide_lt": "metal_wood_lt",
        "hide_dk": "metal_wood_dk",
    },
)

SPEC.gates = MetalGates(
    SPEC.title, SPEC.mats, asym=SPEC.asym,
    soft_over=SPEC.soft_over, free_floating=SPEC.free_floating
)


def custom_render_check_tex(spec: GearSpec, g: Gear) -> None:
    """定制 check_tex.png：上排 参考图 + [正面/3/4/俯视/背面] 4个视角的「平色 / 贴图」对比；下排 与占位玩家并排的贴图版四视图。"""
    flat = gear_kit.write_textured(spec, g, spec.out_dir / "_tmp_flat.bbmodel", flat=True)
    tex = gear_kit.write_textured(spec, g, spec.out_dir / f"{spec.model_name}_tex.bbmodel")
    worn_tmp = spec.out_dir / "_tmp_worn.bbmodel"
    try:
        worn = gear_kit.render_worn(spec, g, worn_tmp, cell=420, textured=True)
    finally:
        worn_tmp.unlink(missing_ok=True)
    size = 360
    font_h, font_l = gear_kit.load_font(15), gear_kit.load_font(13)
    ref_path = spec.review_dir / f"{spec.gear_id}_ref.png"
    ref = Image.open(ref_path).convert("RGB")
    ref = ref.resize((round(ref.width * size / ref.height), size), Image.Resampling.LANCZOS)

    views = (
        ("FRONT 正面（-z）", 180.0, 0.0),
        ("3/4 俯视斜角", 28.0, 24.0),
        ("TOP 俯视（+y）", 0.0, 89.9),
        ("BACK 背面（+z）", 0.0, 0.0),
    )
    top_w = ref.width + len(views) * 2 * (size + 6) + 6
    canvas = Image.new("RGB", (max(top_w, worn.width), size + 34 + worn.height + 8), (28, 29, 33))
    d = ImageDraw.Draw(canvas)
    d.text((6, 8), "REFERENCE 参考图", fill=(215, 205, 180), font=font_h)
    canvas.paste(ref, (6, 30))
    x = ref.width + 12
    for label, yaw, pitch in views:
        for title, path, shading in (("FLAT 平色", flat, None), ("TEXTURED 贴图", tex, "mc")):
            canvas.paste(gear_kit.render_model(path, yaw, pitch, size, shading), (x, 30))
            d.text((x + 4, 8), f"{title}: {label}", fill=(230, 230, 230), font=font_l)
            x += size + 6
    canvas.paste(worn, (0, size + 34))
    flat.unlink(missing_ok=True)
    out = spec.review_dir / "check_tex.png"
    canvas.save(out)
    print(f"✓ {tex.name}；check_tex.png → {out} ({canvas.width}×{canvas.height})")


gear_kit.render_check_tex = custom_render_check_tex

if __name__ == "__main__":
    gear_kit.main(SPEC)
