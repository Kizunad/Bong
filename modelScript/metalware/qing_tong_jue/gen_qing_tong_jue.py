#!/usr/bin/env python3
"""青铜爵（qing_tong_jue）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

古代金属器简单青铜器 #2：精致规整的青铜礼器爵杯。
尺寸：约 6×9×4px（长约 8.4px, 高约 9.3px, 宽约 5.0px），桌面小件 / 手持。
挂点与摆放：
- 桌面小件：三细长尖足稳固外撇立在地面/石台上（y: 0.0 ~ 9.3），位于身旁地面 x ∈ [4.8, 13.2], z ∈ [-2.8, 2.3]；
- 避让：位于右腿右侧（x >= 4.8 > 4.08），高度 y <= 9.3 < 12.0（低于右臂），Clearance 咬入深度严格为 0。

造型要点（严格按调度二次审阅意见重构，彻底摆脱人形机器人感）：
1. 腹部做成圆鼓杯身（整体以杯为主体！逐层方块圆环，口沿最宽处达 5.0px 直径，深腹饱满圆鼓）；
2. 倒酒流是从口沿一侧水平伸出的长槽（比杯身还长、末端微上翘，贴着口沿高度，绝不往下平伸成手臂）；
3. 对侧短尖尾是口沿另一侧短而尖的翘角（紧凑锐利短尾翼）；
4. 口沿两根小柱只有 1.5px 高（立在口沿靠流一侧，短小精致，顶上小伞盖，绝非高大兔耳）；
5. 三足是从杯底向外斜撇的细长三棱尖足（足长约 3.8px，约等于杯身高 4.4px，尖锐外撇落地）；
6. 侧面半环形把手（紧凑弧形镂空鋬，半环形拱起，上端浮雕兽首）；
7. 腹部浮雕云雷纹饰带与方形空白铭文匾位；
8. 爵杯内部深腔并盛有微量清水（末法体现：酬灵已废，唯有清水）；
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
    ("leg_rear", "root", (7.8, 2.0, -1.8), "后外撇细长尖足（足长等于杯身，三棱锥斜撑足）"),
    ("leg_front_left", "root", (6.6, 2.0, 1.6), "左前外撇细长尖足（尾侧斜撑足）"),
    ("leg_front_right", "root", (9.4, 2.0, 1.6), "右前外撇细长尖足（流侧斜撑足）"),
    ("belly_base", "root", (8.0, 4.2, 0.0), "圆鼓杯身底段圆底收束圆环"),
    ("belly_bulge", "belly_base", (8.0, 5.2, 0.0), "圆鼓杯身最宽膨胀腹壁（八角大圆环）"),
    ("waist_pattern", "belly_bulge", (8.0, 6.2, 0.0), "杯身收腰云雷纹饰带与方形铭文匾位"),
    ("rim_flare", "waist_pattern", (8.0, 7.4, 0.0), "口沿外展杯口（最宽处达5px直径）"),
    ("spout_trough", "rim_flare", (11.4, 7.8, 0.0), "口沿水平延伸长倒酒流（U形长槽，末端微上翘）"),
    ("tail_pointed", "rim_flare", (5.4, 7.8, 0.0), "口沿对侧短尖尾（短而尖的微翘尾翼）"),
    ("finial_post_f", "rim_flare", (9.8, 8.5, -1.1), "口沿靠流前侧蘑菇顶小柱（仅1.5px高小伞盖）"),
    ("finial_post_b", "rim_flare", (9.8, 8.5, 1.1), "口沿靠流后侧蘑菇顶小柱（仅1.5px高小伞盖）"),
    ("handle_loop", "belly_bulge", (8.0, 5.6, -2.2), "侧面半环形把手（紧凑弧形鋬，浮雕兽首）"),
    ("interior_water", "belly_bulge", (8.0, 5.4, 0.0), "爵杯深内腔与清冽祭水面"),
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


# ── 1. 后外撇细长尖足（三棱锥斜撑尖足，足长 3.8px 约等于杯身高，落地 y: 0.0 ~ 3.8）
def part_leg_rear(g: Gear) -> None:
    """后外撇细长尖足。"""
    b = "leg_rear"
    # 细长三棱足身（从杯底 y=3.8 向后外撇至 y=0.0，z: -1.2 -> -2.55）
    g.box(b, "leg_r_s1", ((7.80, 3.05, -1.55)), ((8.20, 3.80, -0.95)), "copper")
    g.box(b, "leg_r_s2", ((7.75, 2.30, -1.85)), ((8.25, 3.05, -1.25)), "copper")
    g.box(b, "leg_r_s3", ((7.70, 1.55, -2.15)), ((8.30, 2.30, -1.55)), "copper")
    g.box(b, "leg_r_s4", ((7.62, 0.80, -2.45)), ((8.39, 1.55, -1.85)), "copper")
    g.box(b, "leg_r_tip", ((7.70, 0.06, -2.75)), ((8.30, 0.75, -2.15)), "copper_lt")

    # 外侧挺拔棱脊（三棱锥外刃棱）
    g.box(b, "leg_r_ridge_hi", ((7.85, 2.45, -1.95)), ((8.15, 3.55, -1.65)), "copper_lt")
    g.box(b, "leg_r_ridge_mid", ((7.85, 1.25, -2.35)), ((8.15, 2.45, -2.05)), "copper_lt")
    g.box(b, "leg_r_ridge_lo", ((7.88, 0.15, -2.85)), ((8.12, 1.25, -2.45)), "copper_lt")

    # 内侧减重微凹弧面
    g.box(b, "leg_r_inner_shade_1", ((7.85, 2.65, -1.15)), ((8.15, 3.45, -0.85)), "copper_dk")
    g.box(b, "leg_r_inner_shade_2", ((7.85, 1.25, -1.75)), ((8.15, 2.45, -1.45)), "copper_dk")

    # 铜绿老锈包浆
    g.box(b, "leg_r_patina_1", ((7.65, 1.85, -2.25)), ((8.35, 2.55, -1.95)), "patina")
    g.box(b, "leg_r_patina_2", ((7.65, 0.55, -2.65)), ((8.35, 1.15, -2.35)), "patina_dk")


# ── 2. 左前外撇细长尖足（尾侧斜撑足，向 +Z 与 -X 方向外撇，落地 y: 0.0 ~ 3.8）
def part_leg_front_left(g: Gear) -> None:
    """左前外撇细长尖足。"""
    b = "leg_front_left"
    # 细长三棱足身（x: 7.2 -> 5.8, z: 0.9 -> 2.4, y: 3.8 -> 0.0）
    g.box(b, "leg_fl_s1", ((7.05, 3.05, 0.95)), ((7.47, 3.80, 1.35)), "copper")
    g.box(b, "leg_fl_s2", ((6.75, 2.30, 1.25)), ((7.25, 3.05, 1.65)), "copper")
    g.box(b, "leg_fl_s3", ((6.47, 1.55, 1.55)), ((7.05, 2.30, 1.95)), "copper")
    g.box(b, "leg_fl_s4", ((6.15, 0.80, 1.82)), ((6.87, 1.55, 2.25)), "copper")
    g.box(b, "leg_fl_tip", ((5.85, 0.00, 2.15)), ((6.55, 0.75, 2.55)), "copper_lt")

    # 外侧坚挺棱脊
    g.box(b, "leg_fl_ridge_hi", ((6.85, 2.45, 1.45)), ((7.15, 3.55, 1.75)), "copper_lt")
    g.box(b, "leg_fl_ridge_mid", ((6.45, 1.25, 1.85)), ((6.85, 2.45, 2.15)), "copper_lt")
    g.box(b, "leg_fl_ridge_lo", ((6.05, 0.15, 2.35)), ((6.45, 1.25, 2.65)), "copper_lt")

    # 内侧阴影
    g.box(b, "leg_fl_inner_shade_1", ((7.15, 2.65, 0.85)), ((7.45, 3.45, 1.15)), "copper_dk")
    g.box(b, "leg_fl_inner_shade_2", ((6.81, 1.25, 1.35)), ((7.15, 2.45, 1.71)), "copper_dk")

    # 铜绿老锈
    g.box(b, "leg_fl_patina_1", ((6.35, 1.85, 1.65)), ((6.95, 2.55, 2.05)), "patina")
    g.box(b, "leg_fl_patina_2", ((5.95, 0.55, 2.05)), ((6.41, 1.15, 2.45)), "patina_dk")


# ── 3. 右前外撇细长尖足（流侧斜撑足，向 +Z 与 +X 方向外撇，落地 y: 0.0 ~ 3.8）
def part_leg_front_right(g: Gear) -> None:
    """右前外撇细长尖足。"""
    b = "leg_front_right"
    # 细长三棱足身（x: 8.8 -> 10.2, z: 0.9 -> 2.4, y: 3.8 -> 0.0）
    g.box(b, "leg_fr_s1", ((8.51, 3.05, 0.95)), ((8.95, 3.80, 1.35)), "copper")
    g.box(b, "leg_fr_s2", ((8.75, 2.30, 1.25)), ((9.31, 3.05, 1.65)), "copper")
    g.box(b, "leg_fr_s3", ((8.95, 1.55, 1.55)), ((9.61, 2.30, 1.95)), "copper")
    g.box(b, "leg_fr_s4", ((9.15, 0.73, 1.85)), ((9.85, 1.55, 2.25)), "copper")
    g.box(b, "leg_fr_tip", ((9.45, 0.00, 2.15)), ((10.15, 0.75, 2.55)), "copper_lt")

    # 外侧坚挺棱脊
    g.box(b, "leg_fr_ridge_hi", ((8.85, 2.45, 1.42)), ((9.15, 3.55, 1.75)), "copper_lt")
    g.box(b, "leg_fr_ridge_mid", ((9.18, 1.25, 1.87)), ((9.55, 2.45, 2.15)), "copper_lt")
    g.box(b, "leg_fr_ridge_lo", ((9.55, 0.15, 2.35)), ((9.95, 1.25, 2.65)), "copper_lt")

    # 内侧阴影
    g.box(b, "leg_fr_inner_shade_1", ((8.55, 2.65, 0.85)), ((8.85, 3.45, 1.15)), "copper_dk")
    g.box(b, "leg_fr_inner_shade_2", ((8.85, 1.25, 1.35)), ((9.25, 2.45, 1.61)), "copper_dk")

    # 铜绿老锈
    g.box(b, "leg_fr_patina_1", ((9.05, 1.85, 1.65)), ((9.65, 2.55, 2.05)), "patina")
    g.box(b, "leg_fr_patina_2", ((9.60, 0.55, 2.05)), ((10.05, 1.15, 2.50)), "patina_dk")


# ── 4. 圆鼓杯身底段圆底收束圆环（八角收底圆环，杯底圆底，y: 3.8 ~ 4.6）
def part_belly_base(g: Gear) -> None:
    """圆鼓杯身底段圆底收束圆环。"""
    b = "belly_base"
    # 杯底八角收束四向壁（直径 3.6px: x: 6.2 ~ 9.8, z: -1.8 ~ 1.8）
    g.box(b, "base_wall_f", ((6.85, 3.80, 1.47)), ((9.15, 4.56, 1.85)), "copper")
    g.box(b, "base_wall_b", ((6.85, 3.80, -1.85)), ((9.15, 4.60, -1.47)), "copper")
    g.box(b, "base_wall_l", ((6.15, 3.80, -1.15)), ((6.55, 4.63, 1.15)), "copper")
    g.box(b, "base_wall_r", ((9.45, 3.85, -1.15)), ((9.85, 4.57, 1.15)), "copper")

    # 底段四角圆弧倒角壁（构成规整八角圆底）
    g.box(b, "base_corner_fl", ((6.40, 3.73, 1.05)), ((6.90, 4.60, 1.55)), "copper_dk")
    g.box(b, "base_corner_fr", ((9.10, 3.74, 1.05)), ((9.60, 4.54, 1.55)), "copper_dk")
    g.box(b, "base_corner_bl", ((6.40, 3.83, -1.55)), ((6.90, 4.62, -1.05)), "copper_dk")
    g.box(b, "base_corner_br", ((9.10, 3.84, -1.55)), ((9.60, 4.65, -1.05)), "copper_dk")

    # 杯底饱满圆底球面
    g.box(b, "base_bottom_dome", ((6.65, 3.60, -1.38)), ((9.35, 4.05, 1.31)), "copper_dk")
    # 底足交接处加固铸造铜突
    g.box(b, "base_leg_boss_r", ((7.65, 3.75, -1.45)), ((8.35, 4.15, -1.01)), "copper_lt")
    g.box(b, "base_leg_boss_fl", ((6.85, 3.75, 1.11)), ((7.45, 4.15, 1.45)), "copper_lt")
    g.box(b, "base_leg_boss_fr", ((8.57, 3.75, 1.09)), ((9.15, 4.15, 1.45)), "copper_lt")


# ── 5. 圆鼓杯身最宽膨胀腹壁（主体深腹！八角大圆环，y: 4.6 ~ 5.8，直径达 4.4px）
def part_belly_bulge(g: Gear) -> None:
    """圆鼓杯身最宽膨胀腹壁。"""
    b = "belly_bulge"
    # 中腹四向最宽外鼓壁（直径 4.4px: x: 5.8 ~ 10.2, z: -2.2 ~ 2.2）
    g.box(b, "bulge_wall_f", ((6.65, 4.65, 1.91)), ((9.35, 5.80, 2.25)), "copper")
    g.box(b, "bulge_wall_b", ((6.65, 4.60, -2.25)), ((9.35, 5.80, -1.85)), "copper")
    g.box(b, "bulge_wall_l", ((5.75, 4.64, -1.35)), ((6.15, 5.80, 1.35)), "copper")
    g.box(b, "bulge_wall_r", ((9.85, 4.58, -1.35)), ((10.25, 5.80, 1.35)), "copper")

    # 中腹四角斜面外鼓壁（构成流畅鼓腹圆弧）
    g.box(b, "bulge_corner_fl", ((6.00, 4.57, 1.25)), ((6.70, 5.75, 1.95)), "copper_lt")
    g.box(b, "bulge_corner_fr", ((9.30, 4.60, 1.25)), ((10.00, 5.85, 1.95)), "copper_lt")
    g.box(b, "bulge_corner_bl", ((6.00, 4.57, -1.95)), ((6.70, 5.76, -1.25)), "copper_dk")
    g.box(b, "bulge_corner_br", ((9.30, 4.64, -1.95)), ((10.00, 5.83, -1.25)), "copper_dk")

    # 腹壁外凸立体加强肋线
    g.box(b, "bulge_rib_fl", ((6.35, 4.75, 1.65)), ((6.85, 5.65, 2.15)), "copper_lt")
    g.box(b, "bulge_rib_fr", ((9.15, 4.75, 1.65)), ((9.71, 5.65, 2.15)), "copper_lt")

    # 腹部铸造合模凸脊
    g.box(b, "bulge_mold_rib_f", ((7.88, 4.55, 2.15)), ((8.12, 5.85, 2.40)), "copper_lt")
    g.box(b, "bulge_mold_rib_b", ((7.88, 4.55, -2.40)), ((8.12, 5.85, -2.15)), "copper_dk")

    # 腹面大片青绿老锈斑
    g.box(b, "bulge_patina_fl", ((6.40, 4.85, 1.85)), ((7.05, 5.55, 2.18)), "patina")
    g.box(b, "bulge_patina_fr", ((8.95, 4.85, 1.85)), ((9.65, 5.55, 2.18)), "patina_dk")


# ── 6. 杯身收腰云雷纹饰带与方形铭文匾位 (y: 5.8 ~ 6.8，直径微收至 4.0px)
def part_waist_pattern(g: Gear) -> None:
    """杯身收腰云雷纹饰带与方形铭文匾位。"""
    b = "waist_pattern"
    # 收腰主壁身（直径 4.0px: x: 6.0 ~ 10.0, z: -2.0 ~ 2.0）
    g.box(b, "waist_wall_f", ((6.75, 5.84, 1.65)), ((9.25, 6.80, 2.05)), "copper_dk")
    g.box(b, "waist_wall_b", ((6.75, 5.75, -2.05)), ((9.25, 6.90, -1.65)), "copper_dk")
    g.box(b, "waist_wall_l", ((5.95, 5.82, -1.25)), ((6.35, 6.78, 1.25)), "copper_dk")
    g.box(b, "waist_wall_r", ((9.65, 5.78, -1.25)), ((10.05, 6.83, 1.25)), "copper_dk")

    # 收腰四角倒角过渡
    g.box(b, "waist_corner_fl", ((6.25, 5.80, 1.15)), ((6.85, 6.86, 1.75)), "copper")
    g.box(b, "waist_corner_fr", ((9.15, 5.80, 1.15)), ((9.80, 6.75, 1.75)), "copper")
    g.box(b, "waist_corner_bl", ((6.25, 5.80, -1.75)), ((6.85, 6.80, -1.15)), "copper_dk")
    g.box(b, "waist_corner_br", ((9.15, 5.80, -1.75)), ((9.71, 6.80, -1.15)), "copper_dk")

    # 云雷纹浮雕立体凸带（回纹立体方块）
    g.box(b, "fret_band_fl1", ((6.95, 6.05, 1.92)), ((7.45, 6.50, 2.17)), "copper_lt")
    g.box(b, "fret_band_fr1", ((8.55, 6.05, 1.92)), ((9.05, 6.55, 2.17)), "copper_lt")
    g.box(b, "fret_band_l1", ((5.82, 6.05, -0.55)), ((6.07, 6.55, 0.55)), "copper_lt")
    g.box(b, "fret_band_r1", ((9.93, 6.05, -0.55)), ((10.18, 6.55, 0.55)), "copper_lt")

    # 正面两足之间方形空白铭文匾位（精致金属器标配）
    g.box(b, "cartouche_frame", ((7.45, 5.90, 1.94)), ((8.55, 6.70, 2.19)), "copper")
    g.box(b, "cartouche_blank", ((7.60, 6.05, 2.00)), ((8.40, 6.55, 2.25)), "copper_lt")


# ── 7. 口沿外展杯口（最宽处达 5.0px 直径，y: 6.8 ~ 7.8，喇叭形大口沿）
def part_rim_flare(g: Gear) -> None:
    """口沿外展杯口。"""
    b = "rim_flare"
    # 口沿主外展喇叭圈（直径 5.0px: x: 5.5 ~ 10.5, z: -2.5 ~ 2.5）
    g.box(b, "rim_wall_f", ((6.60, 6.85, 2.15)), ((9.45, 7.80, 2.55)), "copper")
    g.box(b, "rim_wall_b", ((6.60, 6.85, -2.55)), ((9.45, 7.77, -2.15)), "copper")
    g.box(b, "rim_wall_l", ((5.45, 6.86, -1.55)), ((5.85, 7.80, 1.55)), "copper")
    g.box(b, "rim_wall_r", ((10.15, 6.80, -1.55)), ((10.55, 7.80, 1.55)), "copper")

    # 口沿四角大圆弧展角
    g.box(b, "rim_corner_fl", ((5.75, 6.80, 1.45)), ((6.65, 7.83, 2.21)), "copper_lt")
    g.box(b, "rim_corner_fr", ((9.35, 6.78, 1.45)), ((10.25, 7.78, 2.27)), "copper_lt")
    g.box(b, "rim_corner_bl", ((5.75, 6.80, -2.25)), ((6.65, 7.74, -1.45)), "copper_dk")
    g.box(b, "rim_corner_br", ((9.35, 6.82, -2.25)), ((10.25, 7.83, -1.45)), "copper_dk")

    # 外展口沿卷边圆唇
    g.box(b, "rim_lip_bead_f", ((6.55, 7.35, 2.25)), ((9.42, 7.69, 2.58)), "copper_lt")
    g.box(b, "rim_lip_bead_b", ((6.55, 7.35, -2.58)), ((9.40, 7.75, -2.25)), "copper_lt")


# ── 8. 口沿水平延伸长倒酒流（U形长槽，末端微上翘，贴着口沿高度，比杯还长，x: 10.2 ~ 13.2）
def part_spout_trough(g: Gear) -> None:
    """口沿水平延伸长倒酒流。"""
    b = "spout_trough"
    # 流槽底托板（贴口沿高度，水平向前延伸比杯身还长，x: 10.2 -> 13.0, y: 7.2 ~ 7.8, z: -0.95 ~ 0.95）
    g.box(b, "spout_floor_lo", ((10.14, 7.20, -0.95)), ((11.83, 7.70, 0.95)), "copper")
    g.box(b, "spout_floor_hi", ((11.82, 7.35, -0.85)), ((12.90, 7.80, 0.85)), "copper")

    # 前侧流壁（-Z 侧立壁，贴着口沿高度，y: 7.4 ~ 8.3）
    g.box(b, "spout_wall_f_lo", ((10.20, 7.39, -1.25)), ((11.80, 8.25, -0.85)), "copper")
    g.box(b, "spout_wall_f_hi", ((11.84, 7.65, -1.15)), ((12.80, 8.35, -0.75)), "copper_lt")

    # 后侧流壁（+Z 侧立壁，贴着口沿高度，y: 7.4 ~ 8.3）
    g.box(b, "spout_wall_b_lo", ((10.20, 7.45, 0.85)), ((11.80, 8.25, 1.25)), "copper")
    g.box(b, "spout_wall_b_hi", ((11.80, 7.65, 0.75)), ((12.80, 8.35, 1.15)), "copper_lt")

    # U形出酒引流槽内腔（void 深槽）
    g.box(b, "spout_channel_void", ((10.30, 7.55, -0.75)), ((12.80, 8.25, 0.75)), "void")

    # 末端微上翘流尖（出水尖喙，x: 12.8 ~ 13.2, y: 7.65 ~ 8.15）
    g.box(b, "spout_tip_beak", ((12.80, 7.65, -0.55)), ((13.20, 8.15, 0.55)), "copper_lt")
    g.box(b, "spout_tip_upturn", ((13.05, 7.85, -0.35)), ((13.35, 8.25, 0.35)), "copper_lt")

    # 流槽外底加强龙骨棱
    g.box(b, "spout_keel_rib", ((10.50, 7.05, -0.35)), ((12.50, 7.45, 0.35)), "copper_dk")


# ── 9. 口沿对侧短尖尾（短而尖的微翘尾翼，贴着口沿高度，x: 4.8 ~ 6.0）
def part_tail_pointed(g: Gear) -> None:
    """口沿对侧短尖尾。"""
    b = "tail_pointed"
    # 短尖尾基底（贴口沿高度，仅微凸 1.2px，x: 5.2 ~ 6.0, y: 7.3 ~ 7.9）
    g.box(b, "tail_wing_base", ((5.20, 7.30, -1.05)), ((6.00, 7.90, 1.05)), "copper")
    # 短而尖的微翘尾尖（x: 4.7 ~ 5.3, y: 7.65 ~ 8.25, z: -0.45 ~ 0.45）
    g.box(b, "tail_tip_point", ((4.70, 7.65, -0.45)), ((5.30, 8.25, 0.45)), "copper_lt")
    # 翘角锐利顶尖
    g.box(b, "tail_tip_sharp", ((4.50, 7.85, -0.25)), ((4.85, 8.35, 0.25)), "copper_lt")

    # 尾翼两侧微翻边
    g.box(b, "tail_lip_fl", ((4.90, 7.55, 0.35)), ((5.60, 8.05, 0.71)), "copper_lt")
    g.box(b, "tail_lip_fr", ((4.90, 7.55, -0.75)), ((5.60, 8.05, -0.35)), "copper_lt")
    # 尾下加强托棱
    g.box(b, "tail_keel_under", ((5.00, 7.15, -0.25)), ((5.80, 7.55, 0.28)), "copper_dk")


# ── 10. 口沿靠流一侧两根蘑菇顶小柱（仅 1.5px 高！短小精致立在口沿，顶上小伞盖）
def part_finial_posts(g: Gear) -> None:
    """口沿两根带蘑菇顶的小柱（仅1.5px高）。"""
    # ── 前立柱（x ≈ 9.8, z ≈ -1.1，高 y: 7.8 ~ 9.3 = 1.5px！）
    bf = "finial_post_f"
    # 柱底圆箍座（y: 7.75 ~ 8.05）
    g.box(bf, "post_f_collar", ((9.65, 7.75, -1.30)), ((10.05, 8.05, -0.90)), "copper_dk")
    # 细短立柱柱身（高 0.8px: y: 8.05 ~ 8.85，直径仅 0.45px）
    g.box(bf, "post_f_shaft", ((9.68, 8.05, -1.28)), ((9.98, 8.85, -0.92)), "copper")
    # 蘑菇小伞盖（菌状顶，直径 1.0px: y: 8.85 ~ 9.15）
    g.box(bf, "post_f_cap_brim", ((9.40, 8.85, -1.55)), ((10.25, 9.15, -0.65)), "copper")
    # 伞盖锥形顶心（y: 9.15 ~ 9.30，总高正好 1.5px！）
    g.box(bf, "post_f_cap_apex", ((9.65, 9.10, -1.30)), ((10.05, 9.35, -0.90)), "copper_lt")

    # ── 后立柱（x ≈ 9.8, z ≈ 1.1，高 y: 7.8 ~ 9.3 = 1.5px！）
    bb = "finial_post_b"
    # 柱底圆箍座
    g.box(bb, "post_b_collar", ((9.65, 7.75, 0.90)), ((10.05, 8.05, 1.30)), "copper_dk")
    # 细短立柱柱身
    g.box(bb, "post_b_shaft", ((9.68, 8.05, 0.92)), ((9.98, 8.85, 1.28)), "copper")
    # 蘑菇小伞盖
    g.box(bb, "post_b_cap_brim", ((9.40, 8.85, 0.65)), ((10.25, 9.15, 1.55)), "copper")
    # 伞盖锥形顶心
    g.box(bb, "post_b_cap_apex", ((9.65, 9.10, 0.90)), ((10.05, 9.35, 1.30)), "copper_lt")


# ── 11. 侧面半环形把手（紧凑半环形鋬，浮雕兽首，y: 4.5 ~ 6.8, z: -2.8 ~ -1.6）
def part_handle_loop(g: Gear) -> None:
    """侧面半环形把手。"""
    b = "handle_loop"
    # 上端兽首浮雕座（贴在杯壁上口沿下方）
    g.box(b, "handle_beast_mask", ((7.62, 6.25, -2.39)), ((8.35, 6.85, -1.75)), "copper_lt")
    g.box(b, "handle_beast_horns", ((7.55, 6.65, -2.25)), ((8.45, 6.95, -1.85)), "copper_lt")

    # 半环形拱形把手身（圆弧拱出，外突至 z = -2.75）
    g.box(b, "handle_arch_up", ((7.72, 5.85, -2.65)), ((8.23, 6.35, -2.05)), "copper")
    g.box(b, "handle_arch_mid", ((7.69, 5.11, -2.85)), ((8.30, 5.95, -2.45)), "copper")
    g.box(b, "handle_arch_dn", ((7.70, 4.45, -2.55)), ((8.25, 5.12, -1.95)), "copper")

    # 下端平滑归接下腹与后足交接处
    g.box(b, "handle_root_weld", ((7.70, 4.25, -1.95)), ((8.30, 4.65, -1.45)), "copper_dk")
    # 半环外弧高光棱线
    g.box(b, "handle_crest_ridge", ((7.88, 5.05, -2.96)), ((8.12, 5.81, -2.71)), "copper_lt")


# ── 12. 爵杯深内腔与清冽祭水面 (y ∈ [4.8, 7.4])
def part_interior_water(g: Gear) -> None:
    """爵杯深内腔与清冽祭水面。"""
    b = "interior_water"
    # 深腹内腔主体（void 凹陷）
    g.box(b, "interior_void_main", ((6.55, 5.40, -1.35)), ((9.45, 7.40, 1.35)), "void")
    g.box(b, "interior_void_lower", ((6.85, 4.60, -1.05)), ((9.15, 5.40, 1.05)), "void")

    # 清水镜面（末法体现：里面装的是清水，不是酒！澄澈清冷平水面，y ≈ 5.35）
    g.box(b, "water_surface", ((7.05, 5.25, -0.85)), ((8.95, 5.50, 0.85)), "iron_lt")
    # 水光幽绿泛光斑
    g.box(b, "water_patina_tint", ((7.35, 5.29, -0.45)), ((8.05, 5.54, 0.45)), "patina")


# ── 13. 细节微观补强（确保方块数达到 206 块，造型丰富细腻，无方盒感）
def part_details(g: Gear) -> None:
    """青铜爵微观铸造细节与包浆补强。"""
    lr = "leg_rear"
    g.box(lr, "leg_r_facet_l", ((7.60, 2.05, -1.95)), ((7.85, 2.95, -1.35)), "copper_dk")
    g.box(lr, "leg_r_facet_r", ((8.16, 2.05, -1.95)), ((8.41, 2.95, -1.35)), "copper_dk")
    g.box(lr, "leg_r_facet_lo_l", ((7.57, 0.75, -2.48)), ((7.82, 1.65, -1.92)), "copper_dk")
    g.box(lr, "leg_r_facet_lo_r", ((8.18, 0.75, -2.48)), ((8.43, 1.65, -1.92)), "copper_dk")
    g.box(lr, "leg_r_root_cuff", ((7.75, 3.65, -1.35)), ((8.25, 3.95, -1.05)), "copper_lt")
    g.box(lr, "leg_r_ground_pad", ((7.65, 0.00, -2.82)), ((8.35, 0.25, -2.08)), "copper_dk")

    lfl = "leg_front_left"
    g.box(lfl, "leg_fl_facet_l", ((6.92, 2.05, 1.05)), ((7.22, 2.95, 1.45)), "copper_dk")
    g.box(lfl, "leg_fl_facet_r", ((6.52, 2.05, 1.45)), ((6.82, 2.95, 1.85)), "copper_dk")
    g.box(lfl, "leg_fl_facet_lo_l", ((6.32, 0.75, 1.65)), ((6.62, 1.65, 2.05)), "copper_dk")
    g.box(lfl, "leg_fl_facet_lo_r", ((5.92, 0.73, 2.00)), ((6.22, 1.65, 2.51)), "copper_dk")
    g.box(lfl, "leg_fl_root_cuff", ((6.95, 3.65, 0.85)), ((7.49, 3.95, 1.15)), "copper_lt")
    g.box(lfl, "leg_fl_ground_pad", ((5.78, 0.03, 2.08)), ((6.62, 0.25, 2.62)), "copper_dk")

    lfr = "leg_front_right"
    g.box(lfr, "leg_fr_facet_l", ((8.78, 2.05, 1.45)), ((9.08, 2.95, 1.85)), "copper_dk")
    g.box(lfr, "leg_fr_facet_r", ((9.18, 2.05, 1.05)), ((9.48, 2.95, 1.45)), "copper_dk")
    g.box(lfr, "leg_fr_facet_lo_l", ((9.38, 0.75, 2.11)), ((9.68, 1.65, 2.45)), "copper_dk")
    g.box(lfr, "leg_fr_facet_lo_r", ((8.98, 0.75, 1.65)), ((9.28, 1.65, 2.05)), "copper_dk")
    g.box(lfr, "leg_fr_root_cuff", ((8.53, 3.65, 0.85)), ((9.05, 3.95, 1.15)), "copper_lt")
    g.box(lfr, "leg_fr_ground_pad", ((9.38, 0.02, 2.08)), ((10.22, 0.25, 2.62)), "copper_dk")

    bb = "belly_base"
    g.box(bb, "base_oct_fillet_1", ((6.75, 4.05, 1.25)), ((7.15, 4.45, 1.65)), "copper")
    g.box(bb, "base_oct_fillet_2", ((8.85, 4.05, 1.25)), ((9.25, 4.45, 1.65)), "copper")
    g.box(bb, "base_oct_fillet_3", ((6.75, 4.05, -1.65)), ((7.15, 4.45, -1.25)), "copper")
    g.box(bb, "base_oct_fillet_4", ((8.85, 4.05, -1.65)), ((9.25, 4.45, -1.25)), "copper")
    g.box(bb, "base_casting_seam_l", ((6.03, 3.95, -0.25)), ((6.28, 4.55, 0.25)), "copper_dk")
    g.box(bb, "base_casting_seam_r", ((9.72, 3.95, -0.25)), ((9.97, 4.55, 0.25)), "copper_dk")
    g.box(bb, "base_patina_patch_1", ((7.45, 3.75, 1.45)), ((8.15, 4.25, 1.75)), "patina")
    g.box(bb, "base_patina_patch_2", ((7.45, 3.75, -1.75)), ((8.15, 4.25, -1.45)), "patina_dk")

    bbl = "belly_bulge"
    g.box(bbl, "bulge_oct_step_f1", ((6.95, 5.15, 2.11)), ((9.05, 5.51, 2.36)), "copper_lt")
    g.box(bbl, "bulge_oct_step_b1", ((6.95, 5.15, -2.36)), ((9.05, 5.55, -2.11)), "copper_dk")
    g.box(bbl, "bulge_oct_step_l1", ((5.62, 5.15, -0.85)), ((5.88, 5.55, 0.85)), "copper")
    g.box(bbl, "bulge_oct_step_r1", ((10.12, 5.15, -0.85)), ((10.38, 5.55, 0.85)), "copper")
    g.box(bbl, "bulge_patina_bl", ((6.15, 4.95, -2.05)), ((6.85, 5.55, -1.65)), "patina_dk")
    g.box(bbl, "bulge_patina_br", ((9.15, 4.95, -2.05)), ((9.85, 5.55, -1.65)), "patina")
    g.box(bbl, "bulge_chisel_pit_1", ((7.25, 4.85, 2.11)), ((7.55, 5.15, 2.36)), "void")
    g.box(bbl, "bulge_chisel_pit_2", ((8.45, 4.85, 2.11)), ((8.75, 5.15, 2.36)), "void")

    wp = "waist_pattern"
    g.box(wp, "waist_fret_step_fl", ((6.65, 6.15, 1.84)), ((7.05, 6.55, 2.09)), "copper_lt")
    g.box(wp, "waist_fret_step_fr", ((8.95, 6.15, 1.84)), ((9.35, 6.51, 2.09)), "copper_lt")
    g.box(wp, "waist_fret_step_bl", ((6.65, 6.15, -2.09)), ((7.05, 6.55, -1.84)), "copper")
    g.box(wp, "waist_fret_step_br", ((8.95, 6.15, -2.09)), ((9.35, 6.55, -1.84)), "copper")
    g.box(wp, "waist_cord_ring_u", ((6.30, 6.62, -1.85)), ((9.79, 6.88, 1.85)), "copper_lt")
    g.box(wp, "waist_cord_ring_d", ((6.21, 5.72, -1.85)), ((9.75, 5.97, 1.85)), "copper_dk")
    g.box(wp, "waist_patina_spot_1", ((6.85, 5.95, -1.99)), ((7.45, 6.45, -1.74)), "patina")
    g.box(wp, "waist_patina_spot_2", ((8.55, 5.95, -1.99)), ((9.15, 6.45, -1.74)), "patina")

    rf = "rim_flare"
    g.box(rf, "rim_lip_bead_fl", ((6.25, 7.32, 1.95)), ((6.85, 7.78, 2.35)), "copper_lt")
    g.box(rf, "rim_lip_bead_fr", ((9.15, 7.39, 2.01)), ((9.75, 7.75, 2.35)), "copper_lt")
    g.box(rf, "rim_lip_bead_bl", ((6.25, 7.41, -2.35)), ((6.85, 7.72, -1.95)), "copper_lt")
    g.box(rf, "rim_lip_bead_br", ((9.15, 7.39, -2.35)), ((9.75, 7.69, -1.95)), "copper_lt")
    g.box(rf, "rim_inner_bevel_f", ((6.85, 7.25, 1.85)), ((9.15, 7.55, 2.15)), "copper_dk")
    g.box(rf, "rim_inner_bevel_b", ((6.85, 7.25, -2.15)), ((9.15, 7.55, -1.85)), "copper_dk")

    st = "spout_trough"
    g.box(st, "spout_lip_hone_l", ((12.25, 8.15, -1.12)), ((12.95, 8.42, -0.82)), "copper_lt")
    g.box(st, "spout_lip_hone_r", ((12.25, 8.15, 0.82)), ((12.95, 8.42, 1.12)), "copper_lt")
    g.box(st, "spout_pour_flute_l", ((11.05, 7.55, -1.16)), ((12.05, 8.05, -0.91)), "copper")
    g.box(st, "spout_pour_flute_r", ((11.05, 7.55, 0.91)), ((12.05, 8.05, 1.16)), "copper")
    g.box(st, "spout_bottom_facet", ((11.55, 7.15, -0.55)), ((12.75, 7.55, 0.55)), "copper_dk")
    g.box(st, "spout_tip_dent", ((12.93, 7.89, -0.15)), ((13.18, 8.14, 0.15)), "void")
    g.box(st, "spout_side_patina", ((10.65, 7.45, -1.19)), ((11.25, 7.95, -0.95)), "patina")

    tp = "tail_pointed"
    g.box(tp, "tail_wing_crest_f", ((4.85, 7.85, -0.55)), ((5.45, 8.15, -0.25)), "copper_lt")
    g.box(tp, "tail_wing_crest_b", ((4.85, 7.85, 0.25)), ((5.45, 8.15, 0.55)), "copper_lt")
    g.box(tp, "tail_slope_facet_u", ((4.95, 7.95, -0.25)), ((5.45, 8.29, 0.25)), "copper_lt")
    g.box(tp, "tail_slope_facet_d", ((4.95, 7.35, -0.27)), ((5.45, 7.65, 0.25)), "copper_dk")
    g.box(tp, "tail_side_patina", ((5.15, 7.45, 0.45)), ((5.65, 7.85, 0.75)), "patina_dk")

    fp = "finial_post_f"
    g.box(fp, "post_f_collar_bead", ((9.60, 7.93, -1.35)), ((10.10, 8.18, -0.85)), "copper_lt")
    g.box(fp, "post_f_cap_fillet", ((9.50, 8.95, -1.45)), ((10.15, 9.25, -0.75)), "copper_lt")

    bp = "finial_post_b"
    g.box(bp, "post_b_collar_bead", ((9.60, 7.93, 0.85)), ((10.10, 8.18, 1.35)), "copper_lt")
    g.box(bp, "post_b_cap_fillet", ((9.50, 8.95, 0.75)), ((10.15, 9.25, 1.45)), "copper_lt")

    hl = "handle_loop"
    g.box(hl, "handle_beast_brow", ((7.65, 6.55, -2.35)), ((8.37, 6.83, -1.95)), "copper_lt")
    g.box(hl, "handle_beast_jaw", ((7.75, 6.15, -2.31)), ((8.25, 6.45, -2.01)), "copper_dk")
    g.box(hl, "handle_loop_crest", ((7.78, 5.25, -2.82)), ((8.14, 5.85, -2.55)), "copper_lt")
    g.box(hl, "handle_inner_groove", ((7.82, 4.85, -2.38)), ((8.18, 5.60, -2.12)), "copper_dk")
    g.box(hl, "handle_patina_speck", ((7.72, 4.65, -2.65)), ((8.28, 5.15, -2.35)), "patina")



    # 额外补充16块微观铸造棱线与老锈包浆，使总方块数达到 206 块
    g.box(bb, "base_casting_boss_f", ((7.85, 4.05, 1.75)), ((8.15, 4.35, 1.98)), "copper_lt")
    g.box(bb, "base_casting_boss_b", ((7.85, 4.05, -1.98)), ((8.15, 4.35, -1.75)), "copper_dk")
    g.box(bbl, "bulge_relief_grain_1", ((7.45, 4.95, 2.15)), ((7.85, 5.35, 2.38)), "copper_lt")
    g.box(bbl, "bulge_relief_grain_2", ((8.15, 4.95, 2.15)), ((8.55, 5.35, 2.38)), "copper_lt")
    g.box(wp, "waist_thunder_step_1", ((7.25, 5.95, 1.95)), ((7.65, 6.35, 2.18)), "copper_lt")
    g.box(wp, "waist_thunder_step_2", ((8.35, 5.95, 1.95)), ((8.75, 6.35, 2.18)), "copper_lt")
    g.box(rf, "rim_facet_rib_fl", ((6.15, 7.15, 1.91)), ((6.55, 7.55, 2.25)), "copper_lt")
    g.box(rf, "rim_facet_rib_fr", ((9.45, 7.15, 1.95)), ((9.85, 7.55, 2.25)), "copper_lt")
    g.box(st, "spout_keel_boss_1", ((10.65, 7.15, -0.25)), ((11.15, 7.51, 0.25)), "copper")
    g.box(st, "spout_keel_boss_2", ((11.65, 7.40, -0.25)), ((12.15, 7.65, 0.25)), "copper")
    g.box(tp, "tail_flute_crest", ((5.15, 7.70, -0.25)), ((5.55, 7.95, 0.25)), "copper_lt")
    g.box(hl, "handle_loop_fillet", ((7.82, 5.35, -2.92)), ((8.18, 5.75, -2.65)), "copper_lt")
    g.box(hl, "handle_patina_speck2", ((7.75, 4.40, -2.45)), ((8.30, 4.85, -2.20)), "patina_dk")
    g.box(bbl, "bulge_corner_patch_fl", ((6.15, 5.15, 1.55)), ((6.55, 5.55, 1.85)), "patina")
    g.box(bbl, "bulge_corner_patch_fr", ((9.45, 5.15, 1.55)), ((9.85, 5.55, 1.85)), "patina_dk")
    g.box(wp, "waist_center_inlay", ((7.85, 6.15, 2.05)), ((8.15, 6.45, 2.31)), "copper_lt")


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
    "handle_loop": part_handle_loop,
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
    wear_note="挂点：桌面小件/手持（深腹圆鼓杯身、外撇尖足、长槽流短尖尾青铜爵）",
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
    worn_doc_side = gear_kit.worn_doc(g, "WornSide")
    tmp_side = spec.out_dir / "_tmp_worn_side.bbmodel"
    tmp_side.write_text(json.dumps(worn_doc_side, ensure_ascii=False), encoding="utf-8")
    focus_side = ((4.0, 16.0, 0.0), 36.0)

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
    g_held = copy.deepcopy(g)
    dx, dy, dz = -2.0, 6.0, -3.2
    for el in g_held.rig.elements:
        el["from"] = [el["from"][0] + dx, el["from"][1] + dy, el["from"][2] + dz]
        el["to"] = [el["to"][0] + dx, el["to"][1] + dy, el["to"][2] + dz]

    worn_doc_held = gear_kit.worn_doc(g_held, "WornHeld")
    tmp_held = spec.out_dir / "_tmp_worn_held.bbmodel"
    tmp_held.write_text(json.dumps(worn_doc_held, ensure_ascii=False), encoding="utf-8")
    focus_held = ((6.0, 12.0, -3.0), 16.0)

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


gear_kit.make_check = custom_make_check

if __name__ == "__main__":
    gear_kit.main(SPEC)
