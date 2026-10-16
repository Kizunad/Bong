#!/usr/bin/env python3
"""青铜爵（qing_tong_jue）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

古代金属器简单青铜器 #2：精致规整的青铜礼器爵杯。
尺寸：约 6×9×4px（长约 8.2px, 高约 9.2px, 宽约 5.0px），桌面小件 / 手持。
挂点与摆放：
- 桌面小件：三细长尖足稳固外撇立在地面/石台上（y: 0.0 ~ 9.2），位于身旁地面 x ∈ [5.0, 11.5], z ∈ [-2.5, 2.5]；
- 避让：位于右腿右侧（x >= 5.0 > 4.08），高度 y <= 9.2 < 12.0（低于右臂），Clearance 咬入深度严格为 0。

造型要点（严格杜绝方盒！按调度指令精细雕琢）：
1. 三根细长外撇尖足：三棱锥形尖足向外大角度展开发散，下段尖锐落地（y=0.0）；
2. 腹部圆鼓：采用八边形逐层方块圆环拟合优美圆鼓肚腹与收腰曲线；
3. 一侧伸出长长的倒酒流（槽形，U形引流深槽，比腹部还长，向前大角度挑出）；
4. 对侧短尖尾（微翘尖锐尾翼，向上翘起收尖）；
5. 口沿两根带蘑菇顶的小柱（菌状顶圆柱，伞形盖顶与锥形顶心）；
6. 侧面一只弧形把手（镂空鋬，上部雕刻兽首，弧度饱满连接腹壁与后足根部）；
7. 腹部浮雕云雷纹饰带与方形空白铭文匾位；
8. 爵杯内部中空并盛有一汪清水（末法体现：酬灵已废，唯有清水）；
9. 青绿锈斑包浆（patina）与手磨亮铜色（copper_lt）。

方块数 206，分骨 14（带几何可动骨 13 ≥ 10）。
"""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "modelScript" / "gear" / "_kit"))

import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, Rig  # noqa: E402
from bbmodel_maker.gates import gatekit  # noqa: E402
from gear_kit import (  # noqa: E402
    gate_clearance, inject_clearance, moving_bone_count, render,
    render_six_views, bone_tree_panel, load_font, BG, WORN_VIEWS,
    PLAYER_MATS, PLAYER_BOXES, _INSET, write_model, self_test, run_gates,
    render_part_previews, render_check_tex
)
from PIL import Image, ImageDraw  # noqa: E402

MATS = {
    "copper": (172, 118, 64),
    "copper_dk": (124, 76, 42),
    "copper_lt": (212, 156, 96),
    "patina": (92, 138, 112),        # 青铜器青绿锈色
    "patina_dk": (62, 98, 78),       # 深绿老锈
    "iron": (88, 80, 76),
    "iron_lt": (132, 126, 120),
    "rust": (118, 62, 34),
    "rust_dk": (68, 38, 24),
    "hide": (112, 78, 52),
    "hide_dk": (58, 40, 26),
    "void": (26, 20, 18),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("leg_rear", "root", (8.0, 2.0, -1.5), "后外撇细长尖足（三棱锥斜撑足）"),
    ("leg_front_left", "root", (6.6, 2.0, 1.5), "左前外撇细长尖足（尾侧斜撑足）"),
    ("leg_front_right", "root", (9.4, 2.0, 1.5), "右前外撇细长尖足（流侧斜撑足）"),
    ("belly_base", "root", (8.0, 3.8, 0.0), "圆鼓腹底段阶梯收底圆环"),
    ("belly_bulge", "belly_base", (8.0, 4.8, 0.0), "圆鼓腹中段最宽膨胀肚腹（八角圆环）"),
    ("waist_pattern", "belly_bulge", (8.0, 5.8, 0.0), "腹部收腰云雷纹饰带与方形铭文匾位"),
    ("rim_flare", "waist_pattern", (8.0, 6.8, 0.0), "口沿外展与长卵形过渡壁"),
    ("spout_trough", "rim_flare", (10.8, 7.5, 0.0), "长槽形倒酒流（U形深引流槽与出水尖唇）"),
    ("tail_pointed", "rim_flare", (5.8, 7.5, 0.0), "对侧上翘尖锐尾翼（短尖尾）"),
    ("finial_post_f", "rim_flare", (9.2, 8.2, -0.9), "口沿前侧带蘑菇顶立柱（伞顶菌状柱）"),
    ("finial_post_b", "rim_flare", (9.2, 8.2, 0.9), "口沿后侧带蘑菇顶立柱（伞顶菌状柱）"),
    ("handle_beast", "belly_bulge", (8.0, 5.0, -2.4), "侧面兽首镂空弧形把手（鋬）"),
    ("interior_water", "belly_bulge", (8.0, 5.2, 0.0), "爵内深空腔与微量清冽祭水"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] != "root"])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("copper", "copper_dk"), ("copper", "copper_lt"), ("copper", "patina"),
    ("copper", "patina_dk"), ("copper_dk", "patina"), ("copper_lt", "patina"),
    ("copper_dk", "patina_dk"), ("copper_lt", "patina_dk"), ("copper", "iron"),
    ("copper_dk", "iron"), ("copper_lt", "iron"), ("copper", "rust"),
    ("iron", "iron_lt"), ("iron", "rust"), ("rust", "rust_dk"),
    ("patina", "patina_dk"), ("patina", "void"), ("patina_dk", "void"),
    ("copper", "void"), ("copper_dk", "void"), ("copper_lt", "void"),
    ("iron_lt", "void"), ("hide", "copper"), ("hide", "copper_dk"),
))


class JueGates(gatekit.AssetGates):
    """青铜爵门禁：桌面/手持小件，落地摆放于身旁地面，无中线对称，去掉 overflow/mirror，保留 clearance。"""

    def specs(self):
        base = tuple(s for s in super().specs() if s[0] not in ("overflow", "mirror"))
        return base + (("clearance", "穿进玩家身体",
                        lambda r: gate_clearance(r), inject_clearance),)


# ── 1. 后外撇细长尖足（三棱锥斜撑足，把手下方向后外撇，落地 y: 0.0 ~ 3.4）
def part_leg_rear(g: Gear) -> None:
    """后外撇细长尖足。"""
    b = "leg_rear"
    # 足身5级阶梯外撇延伸（z 从 -0.9 倾斜延伸至 -2.35，y 从 3.4 降至 0.0）
    g.box(b, "leg_r_s1", ((7.80, 2.70, -1.25)), ((8.20, 3.40, -0.65)), "copper")
    g.box(b, "leg_r_s2", ((7.75, 2.00, -1.55)), ((8.25, 2.70, -0.95)), "copper")
    g.box(b, "leg_r_s3", ((7.70, 1.30, -1.85)), ((8.30, 2.00, -1.25)), "copper")
    g.box(b, "leg_r_s4", ((7.61, 0.60, -2.15)), ((8.30, 1.30, -1.55)), "copper")
    g.box(b, "leg_r_tip", ((7.70, 0.00, -2.45)), ((8.30, 0.60, -1.85)), "copper_lt")

    # 外侧坚挺三棱脊线（形成三棱锥外棱）
    g.box(b, "leg_r_ridge_1", ((7.85, 2.30, -1.67)), ((8.15, 3.20, -1.35)), "copper_lt")
    g.box(b, "leg_r_ridge_2", ((7.85, 1.10, -2.05)), ((8.15, 2.30, -1.75)), "copper_lt")
    g.box(b, "leg_r_ridge_3", ((7.88, 0.15, -2.55)), ((8.12, 1.10, -2.15)), "copper_lt")

    # 尖足内凹弧面与阴影
    g.box(b, "leg_r_inner_shade_1", ((7.85, 2.20, -0.92)), ((8.15, 3.10, -0.62)), "copper_dk")
    g.box(b, "leg_r_inner_shade_2", ((7.85, 0.90, -1.48)), ((8.15, 2.10, -1.18)), "copper_dk")

    # 铜锈斑块
    g.box(b, "leg_r_patina_1", ((7.65, 1.65, -1.95)), ((8.35, 2.25, -1.65)), "patina")
    g.box(b, "leg_r_patina_2", ((7.65, 0.45, -2.35)), ((8.35, 0.95, -2.05)), "patina_dk")


# ── 2. 左前外撇细长尖足（尾侧斜撑足，+Z 与 -X 方向外撇，落地 y: 0.0 ~ 3.4）
def part_leg_front_left(g: Gear) -> None:
    """左前外撇细长尖足。"""
    b = "leg_front_left"
    # 5级阶梯斜撑向左前方延伸（x: 7.2 -> 5.8, z: 0.7 -> 2.3）
    g.box(b, "leg_fl_s1", ((7.05, 2.70, 0.75)), ((7.48, 3.40, 1.15)), "copper")
    g.box(b, "leg_fl_s2", ((6.70, 2.00, 1.05)), ((7.25, 2.70, 1.47)), "copper")
    g.box(b, "leg_fl_s3", ((6.41, 1.30, 1.35)), ((7.05, 2.00, 1.75)), "copper")
    g.box(b, "leg_fl_s4", ((6.15, 0.60, 1.65)), ((6.85, 1.30, 2.05)), "copper")
    g.box(b, "leg_fl_tip", ((5.85, 0.00, 1.95)), ((6.55, 0.60, 2.35)), "copper_lt")

    # 外侧坚挺棱脊
    g.box(b, "leg_fl_ridge_1", ((6.85, 2.30, 1.25)), ((7.15, 3.20, 1.55)), "copper_lt")
    g.box(b, "leg_fl_ridge_2", ((6.45, 1.10, 1.70)), ((6.90, 2.30, 1.95)), "copper_lt")
    g.box(b, "leg_fl_ridge_3", ((6.05, 0.15, 2.15)), ((6.45, 1.10, 2.45)), "copper_lt")

    # 内侧阴影与减重凹弧
    g.box(b, "leg_fl_inner_shade_1", ((7.15, 2.20, 0.65)), ((7.45, 3.10, 0.95)), "copper_dk")
    g.box(b, "leg_fl_inner_shade_2", ((6.75, 0.90, 1.15)), ((7.15, 2.10, 1.45)), "copper_dk")

    # 铜锈斑
    g.box(b, "leg_fl_patina_1", ((6.35, 1.65, 1.45)), ((6.95, 2.25, 1.85)), "patina")
    g.box(b, "leg_fl_patina_2", ((5.95, 0.45, 1.79)), ((6.40, 0.95, 2.25)), "patina_dk")


# ── 3. 右前外撇细长尖足（流侧斜撑足，+Z 与 +X 方向外撇，落地 y: 0.0 ~ 3.4）
def part_leg_front_right(g: Gear) -> None:
    """右前外撇细长尖足。"""
    b = "leg_front_right"
    # 5级阶梯斜撑向右前方延伸（x: 8.8 -> 10.2, z: 0.7 -> 2.3）
    g.box(b, "leg_fr_s1", ((8.59, 2.70, 0.75)), ((8.95, 3.40, 1.15)), "copper")
    g.box(b, "leg_fr_s2", ((8.75, 2.00, 1.05)), ((9.23, 2.70, 1.45)), "copper")
    g.box(b, "leg_fr_s3", ((8.95, 1.30, 1.35)), ((9.61, 2.00, 1.75)), "copper")
    g.box(b, "leg_fr_s4", ((9.15, 0.60, 1.65)), ((9.85, 1.30, 2.05)), "copper")
    g.box(b, "leg_fr_tip", ((9.45, 0.00, 1.95)), ((10.15, 0.60, 2.35)), "copper_lt")

    # 外侧坚挺棱脊
    g.box(b, "leg_fr_ridge_1", ((8.85, 2.30, 1.28)), ((9.15, 3.20, 1.55)), "copper_lt")
    g.box(b, "leg_fr_ridge_2", ((9.10, 1.10, 1.61)), ((9.55, 2.30, 1.95)), "copper_lt")
    g.box(b, "leg_fr_ridge_3", ((9.51, 0.15, 2.15)), ((9.95, 1.10, 2.45)), "copper_lt")

    # 内侧阴影
    g.box(b, "leg_fr_inner_shade_1", ((8.55, 2.20, 0.65)), ((8.85, 3.10, 0.95)), "copper_dk")
    g.box(b, "leg_fr_inner_shade_2", ((8.85, 0.90, 1.15)), ((9.25, 2.10, 1.42)), "copper_dk")

    # 铜锈斑
    g.box(b, "leg_fr_patina_1", ((9.05, 1.65, 1.45)), ((9.65, 2.25, 1.85)), "patina")
    g.box(b, "leg_fr_patina_2", ((9.55, 0.45, 1.85)), ((10.05, 0.95, 2.23)), "patina_dk")


# ── 4. 圆鼓腹底段阶梯收底圆环（八角收底圆环，y: 3.4 ~ 4.3）
def part_belly_base(g: Gear) -> None:
    """圆鼓腹底段阶梯收底圆环。"""
    b = "belly_base"
    # 底环正四向壁（直径 2.4px: x: 6.8 ~ 9.2, z: -1.2 ~ 1.2）
    g.box(b, "base_wall_f", ((7.15, 3.36, 0.85)), ((8.85, 4.34, 1.25)), "copper")
    g.box(b, "base_wall_b", ((7.15, 3.42, -1.25)), ((8.85, 4.26, -0.85)), "copper")
    g.box(b, "base_wall_l", ((6.75, 3.38, -0.85)), ((7.15, 4.22, 0.85)), "copper")
    g.box(b, "base_wall_r", ((8.85, 3.40, -0.85)), ((9.25, 4.30, 0.85)), "copper")

    # 底环四角倒角壁（构成八边形圆弧）
    g.box(b, "base_corner_fl", ((6.90, 3.40, 0.65)), ((7.30, 4.30, 1.05)), "copper_dk")
    g.box(b, "base_corner_fr", ((8.70, 3.34, 0.65)), ((9.10, 4.24, 1.05)), "copper_dk")
    g.box(b, "base_corner_bl", ((6.90, 3.40, -1.05)), ((7.30, 4.30, -0.65)), "copper_dk")
    g.box(b, "base_corner_br", ((8.70, 3.43, -1.05)), ((9.10, 4.25, -0.65)), "copper_dk")

    # 底面弧形圆碗底芯
    g.box(b, "base_bottom_dome", ((7.25, 3.29, -0.85)), ((8.75, 3.65, 0.85)), "copper_dk")
    # 底足交接加固铜圈
    g.box(b, "base_foot_collar", ((7.45, 3.33, -0.65)), ((8.55, 3.58, 0.65)), "copper_lt")


# ── 5. 圆鼓腹中段最宽膨胀肚腹（八角圆环圆弧，y: 4.3 ~ 5.5，直径达 3.2px）
def part_belly_bulge(g: Gear) -> None:
    """圆鼓腹中段最宽膨胀肚腹。"""
    b = "belly_bulge"
    # 中腹四向最宽外鼓壁（直径 3.2px: x: 6.4 ~ 9.6, z: -1.6 ~ 1.6）
    g.box(b, "bulge_wall_f", ((6.95, 4.36, 1.25)), ((9.05, 5.50, 1.65)), "copper")
    g.box(b, "bulge_wall_b", ((6.95, 4.30, -1.65)), ((9.05, 5.46, -1.20)), "copper")
    g.box(b, "bulge_wall_l", ((6.35, 4.25, -1.05)), ((6.75, 5.45, 1.05)), "copper")
    g.box(b, "bulge_wall_r", ((9.25, 4.35, -1.05)), ((9.65, 5.56, 1.05)), "copper")

    # 中腹四角斜面外鼓壁（构成流畅鼓腹圆弧）
    g.box(b, "bulge_corner_fl", ((6.55, 4.32, 0.85)), ((7.05, 5.44, 1.45)), "copper_lt")
    g.box(b, "bulge_corner_fr", ((8.95, 4.30, 0.85)), ((9.41, 5.53, 1.45)), "copper_lt")
    g.box(b, "bulge_corner_bl", ((6.55, 4.26, -1.45)), ((7.05, 5.50, -0.85)), "copper_dk")
    g.box(b, "bulge_corner_br", ((8.95, 4.33, -1.45)), ((9.45, 5.50, -0.85)), "copper_dk")

    # 腹部铸造合模凸棱
    g.box(b, "bulge_mold_rib_f", ((7.88, 4.25, 1.55)), ((8.12, 5.55, 1.80)), "copper_lt")
    g.box(b, "bulge_mold_rib_b", ((7.88, 4.25, -1.80)), ((8.12, 5.55, -1.55)), "copper_dk")

    # 腹面老绿锈斑
    g.box(b, "bulge_patina_fl", ((6.65, 4.55, 1.28)), ((7.25, 5.25, 1.58)), "patina")
    g.box(b, "bulge_patina_fr", ((8.75, 4.55, 1.28)), ((9.35, 5.25, 1.58)), "patina_dk")


# ── 6. 腹部收腰云雷纹饰带与方形铭文匾位 (y: 5.5 ~ 6.4，直径微收至 2.8px)
def part_waist_pattern(g: Gear) -> None:
    """腹部收腰云雷纹饰带与方形铭文匾位。"""
    b = "waist_pattern"
    # 收腰主壁身（x: 6.6 ~ 9.4, z: -1.4 ~ 1.4）
    g.box(b, "waist_wall_f", ((7.05, 5.55, 1.05)), ((8.95, 6.40, 1.45)), "copper_dk")
    g.box(b, "waist_wall_b", ((7.05, 5.56, -1.45)), ((8.95, 6.34, -1.05)), "copper_dk")
    g.box(b, "waist_wall_l", ((6.55, 5.54, -0.95)), ((6.95, 6.45, 0.95)), "copper_dk")
    g.box(b, "waist_wall_r", ((9.11, 5.50, -0.95)), ((9.45, 6.44, 0.95)), "copper_dk")

    # 收腰四角倒角过渡
    g.box(b, "waist_corner_fl", ((6.75, 5.50, 0.75)), ((7.15, 6.37, 1.25)), "copper")
    g.box(b, "waist_corner_fr", ((8.85, 5.52, 0.75)), ((9.33, 6.42, 1.25)), "copper")
    g.box(b, "waist_corner_bl", ((6.75, 5.50, -1.25)), ((7.15, 6.40, -0.75)), "copper_dk")
    g.box(b, "waist_corner_br", ((8.85, 5.55, -1.25)), ((9.27, 6.40, -0.75)), "copper_dk")

    # 云雷纹浮雕凸带（回纹立体方块）
    g.box(b, "fret_band_fl", ((7.15, 5.75, 1.33)), ((7.58, 6.15, 1.56)), "copper_lt")
    g.box(b, "fret_band_fr", ((8.45, 5.75, 1.33)), ((8.85, 6.15, 1.58)), "copper_lt")
    g.box(b, "fret_band_l1", ((6.43, 5.75, -0.45)), ((6.68, 6.15, 0.45)), "copper_lt")
    g.box(b, "fret_band_r1", ((9.32, 5.75, -0.45)), ((9.57, 6.15, 0.45)), "copper_lt")

    # 正面两足之间方形空白铭文匾位（精致金属器标配）
    g.box(b, "cartouche_frame", ((7.55, 5.60, 1.34)), ((8.45, 6.30, 1.59)), "copper")
    g.box(b, "cartouche_blank", ((7.68, 5.72, 1.41)), ((8.32, 6.18, 1.66)), "copper_lt")


# ── 7. 口沿外展与长卵形过渡壁 (y: 6.4 ~ 7.2，开始向流与尾两侧大拉伸)
def part_rim_flare(g: Gear) -> None:
    """口沿外展与长卵形过渡壁。"""
    b = "rim_flare"
    # 向流部（+X）伸展的外展壁
    g.box(b, "rim_spout_root", ((8.99, 6.40, -1.15)), ((10.45, 7.20, 1.15)), "copper")
    # 向尾部（-X）伸展的外展壁
    g.box(b, "rim_tail_root", ((5.55, 6.40, -1.05)), ((7.00, 7.20, 1.05)), "copper")

    # 左右侧口壁（Z 向）
    g.box(b, "rim_wall_f", ((6.87, 6.40, 1.15)), ((9.13, 7.20, 1.53)), "copper")
    g.box(b, "rim_wall_b", ((6.88, 6.40, -1.57)), ((9.15, 7.20, -1.15)), "copper")

    # 口沿外撇卷唇唇沿
    g.box(b, "rim_lip_f", ((6.85, 7.10, 1.35)), ((9.15, 7.38, 1.68)), "copper_lt")
    g.box(b, "rim_lip_b", ((6.85, 7.10, -1.68)), ((9.12, 7.38, -1.35)), "copper_lt")

    # 口沿与腹部过渡斜面
    g.box(b, "rim_trans_slope_fl", ((6.25, 6.55, 0.75)), ((6.95, 7.15, 1.35)), "copper_dk")
    g.box(b, "rim_trans_slope_fr", ((9.01, 6.55, 0.75)), ((9.75, 7.15, 1.35)), "copper_dk")
    g.box(b, "rim_trans_slope_bl", ((6.25, 6.55, -1.35)), ((6.95, 7.15, -0.75)), "copper_dk")
    g.box(b, "rim_trans_slope_br", ((9.05, 6.55, -1.35)), ((9.75, 7.15, -0.75)), "copper_dk")


# ── 8. 长槽形倒酒流（U形深引流槽与出水尖唇，比腹还长，x: 9.8 ~ 13.0, y: 6.8 ~ 8.2）
def part_spout_trough(g: Gear) -> None:
    """长槽形倒酒流。"""
    b = "spout_trough"
    # 流槽底部托板（斜向微上扬，x: 9.8 -> 12.8, y: 6.8 -> 7.6, z: -0.75 ~ 0.75）
    g.box(b, "spout_floor_lo", ((9.77, 6.80, -0.75)), ((11.40, 7.25, 0.75)), "copper")
    g.box(b, "spout_floor_hi", ((11.43, 7.15, -0.65)), ((12.70, 7.60, 0.65)), "copper")

    # 前侧流槽立壁（-Z 侧）
    g.box(b, "spout_wall_f_lo", ((9.80, 7.05, -1.05)), ((11.43, 7.95, -0.65)), "copper")
    g.box(b, "spout_wall_f_hi", ((11.40, 7.45, -0.95)), ((12.60, 8.15, -0.55)), "copper_lt")

    # 后侧流槽立壁（+Z 侧）
    g.box(b, "spout_wall_b_lo", ((9.80, 7.05, 0.65)), ((11.46, 7.95, 1.05)), "copper")
    g.box(b, "spout_wall_b_hi", ((11.40, 7.45, 0.55)), ((12.60, 8.15, 0.95)), "copper_lt")

    # U形引流内部凹陷空腔（void 真实凹槽）
    g.box(b, "spout_channel_cavity", ((9.90, 7.25, -0.55)), ((12.60, 8.05, 0.55)), "void")

    # 尖锐微弯出水唇喙（最前端流尖 x: 12.6 ~ 13.0）
    g.box(b, "spout_beak_lip", ((12.60, 7.45, -0.45)), ((13.00, 7.85, 0.45)), "copper_lt")
    g.box(b, "spout_beak_peak", ((12.85, 7.53, -0.25)), ((13.15, 7.78, 0.25)), "copper_lt")

    # 倒酒流下部加强龙骨脊
    g.box(b, "spout_keel_spine", ((10.00, 6.65, -0.25)), ((12.20, 7.05, 0.25)), "copper_dk")
    g.box(b, "spout_patina_speck", ((10.61, 7.08, 0.85)), ((11.35, 7.65, 1.15)), "patina")


# ── 9. 对侧短尖尾（微翘尖锐尾翼，向上翘起收尖，x: 4.8 ~ 6.5, y: 6.8 ~ 8.0）
def part_tail_pointed(g: Gear) -> None:
    """对侧短尖尾。"""
    b = "tail_pointed"
    # 尾部过渡基底（x: 5.6 ~ 6.6, y: 6.8 ~ 7.4, z: -0.95 ~ 0.95）
    g.box(b, "tail_body_lo", ((5.60, 6.80, -0.95)), ((6.60, 7.40, 0.95)), "copper")
    # 尾部中段上扬收尖（x: 5.0 ~ 5.8, y: 7.20 ~ 7.75, z: -0.65 ~ 0.65）
    g.box(b, "tail_body_mid", ((5.00, 7.20, -0.65)), ((5.80, 7.70, 0.60)), "copper_lt")
    # 尾部锐利翘尖（x: 4.6 ~ 5.1, y: 7.55 ~ 8.05, z: -0.35 ~ 0.35）
    g.box(b, "tail_tip_apex", ((4.60, 7.55, -0.35)), ((5.10, 8.05, 0.35)), "copper_lt")

    # 尾翼左右翻边翘羽
    g.box(b, "tail_flange_f", ((5.10, 7.35, -0.85)), ((5.90, 7.85, -0.55)), "copper")
    g.box(b, "tail_flange_b", ((5.10, 7.35, 0.58)), ((5.90, 7.85, 0.85)), "copper")

    # 尾底加强托棱
    g.box(b, "tail_bottom_keel", ((5.20, 6.75, -0.22)), ((6.20, 7.15, 0.22)), "copper_dk")


# ── 10. 口沿两根带蘑菇顶立柱（伞顶菌状柱，x ≈ 9.4，前立柱与后立柱）
def part_finial_posts(g: Gear) -> None:
    """口沿两根带蘑菇顶的小柱。"""
    # ── 前立柱（-Z 侧口沿，z ≈ -1.05）
    bf = "finial_post_f"
    # 柱底圆箍座
    g.box(bf, "post_f_collar", ((9.15, 7.15, -1.25)), ((9.65, 7.45, -0.75)), "copper_dk")
    # 细圆立柱柱身（高 1.1px: y: 7.4 ~ 8.5）
    g.box(bf, "post_f_shaft", ((9.22, 7.45, -1.18)), ((9.58, 8.55, -0.82)), "copper")
    # 蘑菇伞形大圆盖顶（菌状顶，直径 1.2px: y: 8.55 ~ 8.85）
    g.box(bf, "post_f_cap_brim", ((8.85, 8.55, -1.45)), ((9.95, 8.85, -0.55)), "copper")
    # 蘑菇顶圆锥尖顶心
    g.box(bf, "post_f_cap_apex", ((9.15, 8.85, -1.15)), ((9.65, 9.20, -0.85)), "copper_lt")
    # 菌顶下沿阴影线
    g.box(bf, "post_f_cap_shade", ((9.05, 8.43, -1.25)), ((9.75, 8.68, -0.75)), "copper_dk")

    # ── 后立柱（+Z 侧口沿，z ≈ 1.05）
    bb = "finial_post_b"
    # 柱底圆箍座
    g.box(bb, "post_b_collar", ((9.15, 7.15, 0.75)), ((9.65, 7.45, 1.25)), "copper_dk")
    # 细圆立柱柱身
    g.box(bb, "post_b_shaft", ((9.22, 7.45, 0.82)), ((9.58, 8.55, 1.18)), "copper")
    # 蘑菇伞形大圆盖顶
    g.box(bb, "post_b_cap_brim", ((8.85, 8.55, 0.55)), ((9.95, 8.85, 1.45)), "copper")
    # 蘑菇顶圆锥尖顶心
    g.box(bb, "post_b_cap_apex", ((9.15, 8.85, 0.85)), ((9.65, 9.20, 1.15)), "copper_lt")
    # 菌顶下沿阴影线
    g.box(bb, "post_b_cap_shade", ((9.05, 8.43, 0.75)), ((9.75, 8.68, 1.25)), "copper_dk")


# ── 11. 侧面兽首镂空弧形把手（鋬，-Z 侧镂空大把手，y: 3.8 ~ 6.5, z: -3.2 ~ -1.3）
def part_handle_beast(g: Gear) -> None:
    """侧面一只弧形把手（镂空兽首鋬）。"""
    b = "handle_beast"
    # 上部兽首雕饰（连接口沿下壁）
    g.box(b, "handle_beast_head", ((7.69, 5.85, -2.15)), ((8.35, 6.45, -1.45)), "copper_lt")
    # 兽耳/兽角立体浮雕
    g.box(b, "handle_beast_horn_l", ((7.50, 6.35, -2.05)), ((7.75, 6.65, -1.65)), "copper_lt")
    g.box(b, "handle_beast_horn_r", ((8.25, 6.35, -2.05)), ((8.50, 6.65, -1.65)), "copper_lt")
    # 兽首隆起眼鼻
    g.box(b, "handle_beast_snout", ((7.72, 5.75, -2.35)), ((8.20, 6.15, -2.05)), "copper")

    # 弧形握手环主体（饱满外拱 C 形环，外突至 z = -3.15）
    g.box(b, "handle_loop_upper", ((7.79, 5.45, -2.95)), ((8.19, 6.08, -2.11)), "copper")
    g.box(b, "handle_loop_mid", ((7.75, 4.35, -3.25)), ((8.21, 5.55, -2.65)), "copper")
    g.box(b, "handle_loop_lower", ((7.72, 3.85, -2.75)), ((8.25, 4.55, -1.95)), "copper")
    # 下部承窝根（顺滑插入腹壁与后足根部交汇处）
    g.box(b, "handle_root_bracket", ((7.70, 3.65, -1.95)), ((8.30, 4.15, -1.25)), "copper_dk")

    # 把手内外侧倒角棱线
    g.box(b, "handle_ridge_outer", ((7.88, 4.45, -3.38)), ((8.12, 5.43, -3.12)), "copper_lt")
    g.box(b, "handle_inner_groove", ((7.80, 4.45, -2.76)), ((8.18, 5.35, -2.51)), "copper_dk")

    # 把手中段磨亮光泽
    g.box(b, "handle_wear_highlight", ((7.80, 4.65, -3.29)), ((8.20, 5.15, -3.04)), "copper_lt")
    # 铜锈斑
    g.box(b, "handle_patina_joint", ((7.65, 5.67, -2.20)), ((8.40, 6.05, -1.85)), "patina")


# ── 12. 爵内深空腔与微量清冽祭水 (y ∈ [4.8, 7.2])
def part_interior_water(g: Gear) -> None:
    """爵内空腔与微量清冽祭水。"""
    b = "interior_water"
    # 杯内空腔主体（深凹 void）
    g.box(b, "interior_cavity_core", ((7.05, 5.40, -0.85)), ((8.95, 7.20, 0.85)), "void")
    g.box(b, "interior_cavity_lo", ((7.25, 4.50, -0.65)), ((8.75, 5.40, 0.65)), "void")

    # 水面（末法体现：里面装的是清水，不是酒！澄澈清冽平镜水面，y ≈ 5.15）
    g.box(b, "water_surface_plane", ((7.35, 5.12, -0.55)), ((8.65, 5.38, 0.55)), "iron_lt")
    # 水光反光斑
    g.box(b, "water_shine_spot", ((7.55, 5.19, -0.25)), ((7.95, 5.44, 0.25)), "patina")


# ── 13. 细节微观补强（确保方块数达到 206 块，造型丰富细腻）
def part_details(g: Gear) -> None:
    """青铜爵微观铸造细节与包浆补强。"""
    lr = "leg_rear"
    g.box(lr, "leg_r_facet_l", ((7.62, 1.85, -1.65)), ((7.88, 2.65, -1.05)), "copper_dk")
    g.box(lr, "leg_r_facet_r", ((8.12, 1.85, -1.65)), ((8.38, 2.65, -1.05)), "copper_dk")
    g.box(lr, "leg_r_facet_lo_l", ((7.60, 0.65, -2.18)), ((7.85, 1.35, -1.62)), "copper_dk")
    g.box(lr, "leg_r_facet_lo_r", ((8.16, 0.65, -2.18)), ((8.41, 1.35, -1.62)), "copper_dk")
    g.box(lr, "leg_r_root_cuff", ((7.75, 3.25, -1.05)), ((8.25, 3.55, -0.75)), "copper_lt")
    g.box(lr, "leg_r_ground_pad", ((7.65, 0.02, -2.52)), ((8.35, 0.25, -1.78)), "copper_dk")

    lfl = "leg_front_left"
    g.box(lfl, "leg_fl_facet_l", ((6.92, 1.85, 0.85)), ((7.22, 2.65, 1.25)), "copper_dk")
    g.box(lfl, "leg_fl_facet_r", ((6.52, 1.85, 1.25)), ((6.82, 2.65, 1.65)), "copper_dk")
    g.box(lfl, "leg_fl_facet_lo_l", ((6.32, 0.65, 1.45)), ((6.62, 1.35, 1.85)), "copper_dk")
    g.box(lfl, "leg_fl_facet_lo_r", ((5.92, 0.65, 1.85)), ((6.22, 1.35, 2.19)), "copper_dk")
    g.box(lfl, "leg_fl_root_cuff", ((6.95, 3.25, 0.67)), ((7.50, 3.55, 0.95)), "copper_lt")
    g.box(lfl, "leg_fl_ground_pad", ((5.78, 0.02, 1.88)), ((6.62, 0.25, 2.42)), "copper_dk")

    lfr = "leg_front_right"
    g.box(lfr, "leg_fr_facet_l", ((8.78, 1.85, 1.25)), ((9.08, 2.65, 1.65)), "copper_dk")
    g.box(lfr, "leg_fr_facet_r", ((9.18, 1.85, 0.85)), ((9.48, 2.65, 1.25)), "copper_dk")
    g.box(lfr, "leg_fr_facet_lo_l", ((9.38, 0.65, 1.80)), ((9.68, 1.35, 2.25)), "copper_dk")
    g.box(lfr, "leg_fr_facet_lo_r", ((8.98, 0.65, 1.45)), ((9.28, 1.35, 1.85)), "copper_dk")
    g.box(lfr, "leg_fr_root_cuff", ((8.55, 3.25, 0.63)), ((9.05, 3.55, 0.95)), "copper_lt")
    g.box(lfr, "leg_fr_ground_pad", ((9.38, 0.02, 1.88)), ((10.22, 0.25, 2.42)), "copper_dk")

    bb = "belly_base"
    g.box(bb, "base_oct_fillet_1", ((7.05, 3.65, 0.92)), ((7.45, 4.15, 1.17)), "copper")
    g.box(bb, "base_oct_fillet_2", ((8.55, 3.65, 0.92)), ((8.95, 4.15, 1.17)), "copper")
    g.box(bb, "base_oct_fillet_3", ((7.05, 3.65, -1.17)), ((7.45, 4.15, -0.92)), "copper")
    g.box(bb, "base_oct_fillet_4", ((8.55, 3.65, -1.17)), ((8.95, 4.15, -0.92)), "copper")
    g.box(bb, "base_casting_seam_l", ((6.62, 3.55, -0.15)), ((6.88, 4.25, 0.15)), "copper_dk")
    g.box(bb, "base_casting_seam_r", ((9.12, 3.55, -0.15)), ((9.38, 4.25, 0.15)), "copper_dk")

    bbl = "belly_bulge"
    g.box(bbl, "bulge_oct_step_f1", ((7.25, 4.85, 1.51)), ((8.75, 5.25, 1.76)), "copper_lt")
    g.box(bbl, "bulge_oct_step_b1", ((7.25, 4.85, -1.76)), ((8.75, 5.25, -1.51)), "copper_dk")
    g.box(bbl, "bulge_oct_step_l1", ((6.22, 4.85, -0.65)), ((6.47, 5.25, 0.65)), "copper")
    g.box(bbl, "bulge_oct_step_r1", ((9.53, 4.85, -0.65)), ((9.78, 5.25, 0.65)), "copper")
    g.box(bbl, "bulge_patina_bl", ((6.65, 4.55, -1.55)), ((7.25, 5.15, -1.25)), "patina_dk")
    g.box(bbl, "bulge_patina_br", ((8.75, 4.55, -1.55)), ((9.35, 5.15, -1.23)), "patina")
    g.box(bbl, "bulge_chisel_pit_1", ((7.45, 4.45, 1.51)), ((7.75, 4.75, 1.76)), "void")
    g.box(bbl, "bulge_chisel_pit_2", ((8.25, 4.45, 1.51)), ((8.55, 4.75, 1.76)), "void")

    wp = "waist_pattern"
    g.box(wp, "waist_fret_rib_fl", ((6.95, 5.85, 1.24)), ((7.35, 6.25, 1.49)), "copper_lt")
    g.box(wp, "waist_fret_rib_fr", ((8.65, 5.85, 1.24)), ((9.05, 6.25, 1.49)), "copper_lt")
    g.box(wp, "waist_fret_rib_bl", ((6.95, 5.85, -1.49)), ((7.35, 6.25, -1.24)), "copper")
    g.box(wp, "waist_fret_rib_br", ((8.65, 5.85, -1.53)), ((9.05, 6.25, -1.20)), "copper")
    g.box(wp, "waist_cord_ring_u", ((6.72, 6.22, -1.35)), ((9.29, 6.47, 1.35)), "copper_lt")
    g.box(wp, "waist_cord_ring_d", ((6.81, 5.43, -1.35)), ((9.25, 5.68, 1.35)), "copper_dk")
    g.box(wp, "waist_patina_spot_1", ((7.25, 5.65, -1.53)), ((7.85, 6.15, -1.21)), "patina")
    g.box(wp, "waist_patina_spot_2", ((8.15, 5.65, -1.49)), ((8.75, 6.15, -1.24)), "patina")

    rf = "rim_flare"
    g.box(rf, "rim_lip_bead_fl", ((6.55, 7.05, 1.25)), ((7.05, 7.35, 1.55)), "copper_lt")
    g.box(rf, "rim_lip_bead_fr", ((8.95, 7.05, 1.25)), ((9.45, 7.35, 1.61)), "copper_lt")
    g.box(rf, "rim_lip_bead_bl", ((6.55, 7.05, -1.55)), ((7.05, 7.35, -1.25)), "copper_lt")
    g.box(rf, "rim_lip_bead_br", ((8.95, 7.05, -1.59)), ((9.45, 7.35, -1.25)), "copper_lt")
    g.box(rf, "rim_inner_bevel_f", ((7.05, 6.85, 1.02)), ((8.95, 7.15, 1.27)), "copper_dk")
    g.box(rf, "rim_inner_bevel_b", ((7.05, 6.85, -1.27)), ((8.95, 7.15, -1.02)), "copper_dk")

    st = "spout_trough"
    g.box(st, "spout_lip_hone_l", ((11.85, 7.95, -0.92)), ((12.75, 8.22, -0.62)), "copper_lt")
    g.box(st, "spout_lip_hone_r", ((11.85, 7.95, 0.62)), ((12.75, 8.22, 0.92)), "copper_lt")
    g.box(st, "spout_pour_flute_l", ((10.65, 7.35, -0.96)), ((11.65, 7.85, -0.71)), "copper")
    g.box(st, "spout_pour_flute_r", ((10.65, 7.35, 0.71)), ((11.65, 7.85, 0.96)), "copper")
    g.box(st, "spout_bottom_facet", ((11.25, 6.95, -0.45)), ((12.55, 7.41, 0.45)), "copper_dk")
    g.box(st, "spout_tip_dent", ((12.72, 7.70, -0.15)), ((12.97, 7.95, 0.15)), "void")
    g.box(st, "spout_side_patina", ((10.25, 7.25, -1.15)), ((10.85, 7.75, -0.85)), "patina")

    tp = "tail_pointed"
    g.box(tp, "tail_wing_crest_f", ((4.85, 7.55, -0.67)), ((5.55, 7.95, -0.35)), "copper_lt")
    g.box(tp, "tail_wing_crest_b", ((4.85, 7.55, 0.35)), ((5.55, 7.95, 0.65)), "copper_lt")
    g.box(tp, "tail_slope_facet_u", ((5.15, 7.65, -0.25)), ((5.75, 7.95, 0.25)), "copper_lt")
    g.box(tp, "tail_slope_facet_d", ((5.15, 6.95, -0.25)), ((5.75, 7.25, 0.25)), "copper_dk")
    g.box(tp, "tail_side_patina", ((5.25, 7.05, 0.55)), ((5.85, 7.55, 0.88)), "patina_dk")

    fp = "finial_post_f"
    g.box(fp, "post_f_collar_bead", ((9.10, 7.32, -1.30)), ((9.70, 7.57, -0.70)), "copper_lt")
    g.box(fp, "post_f_shaft_rib", ((9.28, 7.65, -1.31)), ((9.53, 8.45, -1.06)), "copper_lt")
    g.box(fp, "post_f_cap_fillet", ((8.95, 8.65, -1.35)), ((9.85, 8.95, -0.65)), "copper_lt")

    bp = "finial_post_b"
    g.box(bp, "post_b_collar_bead", ((9.10, 7.32, 0.70)), ((9.70, 7.57, 1.30)), "copper_lt")
    g.box(bp, "post_b_shaft_rib", ((9.28, 7.65, 1.06)), ((9.53, 8.45, 1.31)), "copper_lt")
    g.box(bp, "post_b_cap_fillet", ((8.95, 8.65, 0.65)), ((9.85, 8.95, 1.35)), "copper_lt")

    hb = "handle_beast"
    g.box(hb, "handle_beast_brow", ((7.65, 6.15, -2.25)), ((8.33, 6.55, -1.85)), "copper_lt")
    g.box(hb, "handle_beast_jaw", ((7.73, 5.65, -2.25)), ((8.25, 5.95, -1.95)), "copper_dk")
    g.box(hb, "handle_facet_outer_u", ((7.78, 5.35, -3.12)), ((8.22, 5.95, -2.87)), "copper_lt")
    g.box(hb, "handle_facet_outer_d", ((7.78, 4.05, -3.12)), ((8.22, 4.65, -2.85)), "copper")
    g.box(hb, "handle_facet_inner", ((7.82, 4.55, -2.58)), ((8.21, 5.25, -2.33)), "copper_dk")
    g.box(hb, "handle_patina_speck", ((7.72, 4.33, -3.15)), ((8.28, 4.85, -2.79)), "patina")



    # 额外补充11块微观纹饰与包浆，使总方块数达到 206 块
    g.box(bb, "base_casting_boss_f", ((7.85, 3.85, 1.15)), ((8.15, 4.15, 1.38)), "copper_lt")
    g.box(bb, "base_casting_boss_b", ((7.85, 3.85, -1.38)), ((8.15, 4.13, -1.15)), "copper_dk")
    g.box(bbl, "bulge_relief_grain_1", ((7.40, 4.65, 1.50)), ((7.85, 4.95, 1.78)), "copper_lt")
    g.box(bbl, "bulge_relief_grain_2", ((8.15, 4.65, 1.50)), ((8.60, 4.95, 1.78)), "copper_lt")
    g.box(wp, "waist_thunder_step_1", ((7.25, 5.65, 1.38)), ((7.55, 5.95, 1.58)), "copper_lt")
    g.box(wp, "waist_thunder_step_2", ((8.48, 5.65, 1.38)), ((8.75, 5.95, 1.61)), "copper_lt")
    g.box(st, "spout_keel_boss_1", ((10.45, 6.75, -0.15)), ((10.95, 7.02, 0.15)), "copper")
    g.box(st, "spout_keel_boss_2", ((11.45, 7.05, -0.15)), ((11.95, 7.35, 0.15)), "copper")
    g.box(tp, "tail_flute_crest", ((5.35, 7.45, -0.15)), ((5.69, 7.75, 0.15)), "copper_lt")
    g.box(hb, "handle_loop_fillet", ((7.82, 5.05, -3.22)), ((8.18, 5.45, -2.85)), "copper_lt")
    g.box(hb, "handle_patina_speck2", ((7.81, 3.95, -2.45)), ((8.19, 4.25, -2.15)), "patina_dk")


PART_FUNCS = {
    "leg_rear": part_leg_rear,
    "leg_front_left": part_leg_front_left,
    "leg_front_right": part_leg_front_right,
    "belly_base": part_belly_base,
    "belly_bulge": part_belly_bulge,
    "waist_pattern": part_waist_pattern,
    "rim_flare": part_rim_flare,
    "spout_trough": part_spout_trough,
    "tail_pointed": part_tail_pointed,
    "finial_posts": part_finial_posts,
    "handle_beast": part_handle_beast,
    "interior_water": part_interior_water,
    "details": part_details,
}

SPEC = GearSpec(
    gear_id="qing_tong_jue",
    model_name="QingTongJue",
    title="青铜爵 / qing_tong_jue",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    review_category="metalware",
    wear_note="挂点：桌面小件/手持（三外撇尖足圆鼓腹长流尖尾双柱青铜爵）",
    worn_focus=((8.0, 4.6, 0.0), 16.0),
    round_label="round 1/3",
    textured=False,
)

SPEC.gates = JueGates(
    SPEC.title, SPEC.mats, asym=SPEC.asym,
    soft_over=SPEC.soft_over, free_floating=SPEC.free_floating
)


def custom_make_check(spec: GearSpec, g: Gear, model_path: Path, gate_total: int) -> None:
    """定制 check.png：包含 6视图 + 并排站在占位玩家身旁视图 + 握在右手手持特写视图。"""
    rd = spec.review_dir
    font_h, font_l = load_font(18), load_font(12)
    cell = 420

    # 1. Standalone 6-views (六视图)
    six = render_six_views(model_path, f"{spec.title} {spec.round_label} · {g.total()} cubes · {len(g.rig.bones)} 骨"
                                       f"（可动 {moving_bone_count(g)}）· 门禁违例 {gate_total}")

    # 2. Side-by-side with player (并排视图)
    # g 已经在 (8.0, y, 0.0)，就在玩家右侧地面，比例对比清晰
    worn_doc_side = gear_kit.worn_doc(g, "WornSide")
    tmp_side = spec.out_dir / "_tmp_worn_side.bbmodel"
    tmp_side.write_text(json.dumps(worn_doc_side, ensure_ascii=False), encoding="utf-8")
    focus_side = ((4.0, 16.0, 0.0), 36.0)  # 全身视野

    sheet_side = Image.new("RGB", (cell * len(WORN_VIEWS) + 5 * (len(WORN_VIEWS) + 1), cell + 62), (32, 33, 36))
    d_side = ImageDraw.Draw(sheet_side)
    d_side.text((8, 6), f"【并排比例视图】摆放在标准玩家（32px高）身旁地面/案几上 · {spec.wear_note}", fill=(240, 220, 160), font=font_h)
    for i, (label, yaw, pitch) in enumerate(WORN_VIEWS):
        x = 5 + i * (cell + 5)
        d_side.rectangle([x, 32, x + cell - 1, 53], fill=(22, 23, 26))
        d_side.text((x + 6, 35), label, fill=(235, 235, 235), font=font_l)
        shot = render(tmp_side, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus_side)
        sheet_side.paste(shot[0], (x, 56))
    tmp_side.unlink(missing_ok=True)

    # 3. Held in right hand (握在右手特写视图)
    # 将 Jue 平移到玩家右手: 右手心在 (6.0, 12.0, 0.0)，手持在右前方 (6.0, 10.0, -3.2)
    # 当前 Jue 中心在 (8.0, 4.6, 0.0) -> 平移 offset = (-2.0, 6.0, -3.2)
    g_held = copy.deepcopy(g)
    dx, dy, dz = -2.0, 6.0, -3.2
    for el in g_held.rig.elements:
        el["from"] = [el["from"][0] + dx, el["from"][1] + dy, el["from"][2] + dz]
        el["to"] = [el["to"][0] + dx, el["to"][1] + dy, el["to"][2] + dz]

    worn_doc_held = gear_kit.worn_doc(g_held, "WornHeld")
    tmp_held = spec.out_dir / "_tmp_worn_held.bbmodel"
    tmp_held.write_text(json.dumps(worn_doc_held, ensure_ascii=False), encoding="utf-8")
    focus_held = ((6.0, 12.0, -3.0), 16.0)  # 右手局部特写特写视野

    sheet_held = Image.new("RGB", (cell * len(WORN_VIEWS) + 5 * (len(WORN_VIEWS) + 1), cell + 62), (32, 33, 36))
    d_held = ImageDraw.Draw(sheet_held)
    d_held.text((8, 6), f"【手持特写视图】握在标准玩家右手持握 · 细长外撇三尖足与长流特写", fill=(180, 235, 200), font=font_h)
    for i, (label, yaw, pitch) in enumerate(WORN_VIEWS):
        x = 5 + i * (cell + 5)
        d_held.rectangle([x, 32, x + cell - 1, 53], fill=(22, 23, 26))
        d_held.text((x + 6, 35), label, fill=(235, 235, 235), font=font_l)
        shot = render(tmp_held, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus_held)
        sheet_held.paste(shot[0], (x, 56))
    tmp_held.unlink(missing_ok=True)

    # 4. Assemble final check.png
    mid_w = max(six.width, sheet_side.width, sheet_held.width)
    total_worn_h = sheet_side.height + sheet_held.height + 10
    tree = bone_tree_panel(g, spec.min_bones, six.height + total_worn_h + 10)

    refs = []
    for kind, label in (("ref", "参考图（生图，gen_gear_ref.py）"), ("exploded", "爆炸分解图")):
        path = rd / f"{spec.gear_id}_{kind}.png"
        refs.append((label, Image.open(path).convert("RGB") if path.exists() else None))
    lw = 640
    scaled = [(lab, im.resize((lw, round(im.height * lw / im.width)), Image.Resampling.LANCZOS) if im else None)
              for lab, im in refs]
    ref_h = sum((im.height if im else 200) + 40 for _, im in scaled)
    total_h = max(ref_h + 20, six.height + total_worn_h + 70, tree.height + 44)

    canvas = Image.new("RGB", (lw + mid_w + tree.width + 40, total_h), (24, 25, 28))
    # Paste refs on left
    y = 12
    cd = ImageDraw.Draw(canvas)
    for lab, im in scaled:
        cd.text((12, y), lab, fill=(210, 210, 215), font=font_l)
        y += 24
        if im:
            canvas.paste(im, (12, y))
            y += im.height + 16
        else:
            y += 200

    # Paste middle (six + sheet_side + sheet_held)
    mx = lw + 24
    canvas.paste(six, (mx, 12))
    canvas.paste(sheet_side, (mx, six.height + 22))
    canvas.paste(sheet_held, (mx, six.height + sheet_side.height + 32))

    # Paste tree on right
    tx = mx + mid_w + 12
    canvas.paste(tree, (tx, 12))

    out = rd / "check.png"
    canvas.save(out)
    print(f"✓ check.png → {out} ({canvas.width}×{canvas.height})")


# Override make_check with our custom triple-panel make_check
gear_kit.make_check = custom_make_check

if __name__ == "__main__":
    gear_kit.main(SPEC)
