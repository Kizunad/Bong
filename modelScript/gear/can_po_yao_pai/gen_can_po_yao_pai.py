#!/usr/bin/env python3
"""残破宗门腰牌（can_po_yao_pai）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土定制 NPC 专属装备 #5（zong_men_can_tu 宗门残徒）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双腿 x∈[0,4] / [-4,0] y∈[0,12] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

挂点与避让说明：
- 挂在右前胯部（x∈[0.2, 3.9]，y∈[1.2, 13.6]，z∈[-3.5, -2.08]）。
- 完全在躯干和右腿前面（z ≤ -2.08），咬入深度 ≤ 0.05px，严格不穿入身体。
- x 位于中右侧（x ≤ 3.9），严格避开手臂摆动盒（x ≥ 4.0，y∈[12,24]）。
- 左侧完全留空，为 NPC 复用装备水囊（water_skin，挂于左胯 x∈[-7.9, -4.1]）留出充分空域，绝不干涉。
- 体量醒目：厚重兽骨大牌身、断裂斜角、多层阶梯残茬、立体宗门卷云纹、铁箍铆钉、红丝缠绳、粗大流苏与串结骨珠。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/gear/can_po_yao_pai/gen_can_po_yao_pai.py --self-test
    python3 modelScript/gear/can_po_yao_pai/gen_can_po_yao_pai.py --gates
    python3 modelScript/gear/can_po_yao_pai/gen_can_po_yao_pai.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "bone":         (216, 206, 178),   # 象牙白老兽骨（腰牌基底）
    "bone_lt":      (236, 228, 204),   # 浮雕凸起与骨棱亮光
    "bone_dk":      (164, 150, 118),   # 骨刻深槽与背阴面
    "bone_stain":   (132, 114, 86),    # 断面风化陈年旧垢
    "sash_red":     (134, 42, 38),     # 褪色宗门暗红布带
    "sash_red_dk":  (92, 28, 26),      # 暗红绳结与扎带背阴
    "cord":         (146, 126, 82),    # 枯黄麻绳 / 悬绳
    "cord_dk":      (104, 88, 56),     # 粗绳阴影与绳结
    "leather":      (106, 72, 50),     # 减震底托厚皮
    "leather_dk":   (74, 50, 36),      # 磨损深色皮边
    "iron":         (84, 80, 82),      # 生铁悬环与加固箍
    "iron_lt":      (120, 116, 118),   # 铁铆钉与磨亮棱角
    "rust":         (144, 80, 46),     # 锈蚀凹斑
    "bronze":       (128, 102, 62),    # 残存青铜小铃
    "stitch":       (58, 44, 32),      # 粗麻针脚
    "void":         (24, 20, 18),      # 穿绳孔与贯通裂隙暗底
}

SOFT = ("sash_red", "sash_red_dk", "cord", "cord_dk", "leather", "leather_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("waist_sash", "root", (0.0, 12.8, 0.0), "腰带布绳圈：前后对称暗红粗布腰带"),
    ("sash_knot", "waist_sash", (2.2, 12.8, -2.45), "右前系结：双层交叉活结与打结绳舌"),
    ("hang_loop", "sash_knot", (2.3, 11.8, -2.55), "悬挂索环：双股粗麻吊绳、八角生铁环与穿索"),
    ("token_pad", "hang_loop", (2.2, 9.2, -2.38), "减震皮托：腰牌后方厚牛皮垫板、包边与粗针脚"),
    ("token_main", "hang_loop", (2.2, 9.2, -2.62), "兽骨腰牌身：拱首厚骨牌身、凸棱边框与减薄底槽"),
    ("token_fracture", "token_main", (3.0, 7.8, -2.62), "残破断角：右下斜向大裂口、多层阶梯残茬与横贯劈裂缝"),
    ("token_crest", "token_main", (2.0, 9.2, -2.76), "宗门流云残纹：正面浅浮雕断弧卷云纹饰"),
    ("token_wrap", "token_main", (2.2, 11.0, -2.62), "牌首加固铁箍：夹片铁板、贯通铆钉与多道红丝缠绳"),
    ("hang_tassel", "token_main", (1.8, 6.2, -2.65), "悬垂流苏索：从下耳穿出的盘长结与粗股红丝绳"),
    ("tassel_beads", "hang_tassel", (1.8, 5.0, -2.65), "串结骨珠：多棱兽骨管、隔片与残破青铜铃"),
    ("tassel_tails", "tassel_beads", (1.8, 3.2, -2.65), "散开羽须：十数条参差下垂的粗细丝绒尾"),
    ("side_toggle", "hang_loop", (3.3, 11.0, -2.55), "侧挂骨签：辅助穿扣的雕槽细兽骨签与麻绳结"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "waist_sash"))


# ───────────────────────────────────────────────────────────── 零件建模实现
def part_waist_sash(g: Gear) -> None:
    """腰带布绳圈：前后对称的暗红粗布束带，上下暗色滚边，中线皮质扣垫与前后双行针脚。"""
    b = "waist_sash"
    # 前后对称横跨布带（厚度 ≥ 0.28）
    g.box(b, "sash_f_main", (-3.95, 12.20, -2.48), (3.95, 13.40, -2.18), "sash_red")
    g.box(b, "sash_f_rim_t", (-3.95, 13.40, -2.50), (3.95, 13.68, -2.16), "sash_red_dk")
    g.box(b, "sash_f_rim_b", (-3.95, 11.92, -2.50), (3.95, 12.20, -2.16), "sash_red_dk")

    g.box(b, "sash_b_main", (-3.95, 12.20, 2.18), (3.95, 13.40, 2.48), "sash_red")
    g.box(b, "sash_b_rim_t", (-3.95, 13.40, 2.16), (3.95, 13.68, 2.50), "sash_red_dk")
    g.box(b, "sash_b_rim_b", (-3.95, 11.92, 2.16), (3.95, 12.20, 2.50), "sash_red_dk")

    # 前腰对称粗麻针脚行（左右成对）
    for k in range(3):
        x = 0.65 + k * 1.05
        g.pair(b, f"sash_f_st_t_{k}", (x, 13.25, -2.62), (x + 0.42, 13.55, -2.32), "stitch")
        g.pair(b, f"sash_f_st_b_{k}", (x, 12.05, -2.62), (x + 0.42, 12.35, -2.32), "stitch")
        g.pair(b, f"sash_b_st_t_{k}", (x, 13.25, 2.32), (x + 0.42, 13.55, 2.62), "stitch")
        g.pair(b, f"sash_b_st_b_{k}", (x, 12.05, 2.32), (x + 0.42, 12.35, 2.62), "stitch")

    # 后腰正中收扣骨卡（背中对称骨带扣）
    g.box(b, "sash_back_buckle", (-0.75, 12.12, 2.40), (0.75, 13.48, 2.76), "bone_dk")
    g.box(b, "sash_back_pin", (-0.22, 12.32, 2.70), (0.22, 13.28, 2.96), "iron")


def part_sash_knot(g: Gear) -> None:
    """右前系结：粗布带打结的大纽扣、交叉带舌与垂坠绳结。"""
    b = "sash_knot"
    g.box(b, "knot_underpad", (1.45, 11.95, -2.58), (3.15, 13.65, -2.25), "leather_dk")
    g.box(b, "knot_core", (1.68, 12.15, -2.82), (2.92, 13.45, -2.35), "sash_red_dk")
    g.box(b, "knot_cross_a", (1.55, 12.40, -2.92), (2.75, 13.20, -2.48), "sash_red")
    g.box(b, "knot_cross_b", (1.85, 12.30, -2.98), (3.05, 13.30, -2.55), "sash_red_dk")
    g.box(b, "knot_eye", (2.12, 12.60, -3.08), (2.58, 13.00, -2.82), "sash_red")

    g.box(b, "knot_pad_st1", (1.55, 13.42, -2.68), (1.92, 13.72, -2.38), "stitch")
    g.box(b, "knot_pad_st2", (2.68, 13.42, -2.68), (3.05, 13.72, -2.38), "stitch")
    g.box(b, "knot_rivet", (1.62, 12.10, -2.72), (1.92, 12.42, -2.42), "iron_lt")

    # 垂坠带舌
    g.box(b, "tongue_l_seg1", (1.48, 11.45, -2.82), (2.05, 12.30, -2.48), "sash_red")
    g.box(b, "tongue_l_seg2", (1.38, 10.65, -2.76), (1.92, 11.55, -2.45), "sash_red_dk")
    g.box(b, "tongue_l_tip", (1.42, 10.15, -2.72), (1.85, 10.75, -2.46), "sash_red")

    g.box(b, "tongue_r_seg1", (2.55, 11.35, -2.85), (3.12, 12.25, -2.50), "sash_red_dk")
    g.box(b, "tongue_r_seg2", (2.65, 10.55, -2.80), (3.18, 11.45, -2.52), "sash_red")
    g.box(b, "tongue_r_tip", (2.72, 9.95, -2.75), (3.15, 10.65, -2.52), "sash_red_dk")


def part_hang_loop(g: Gear) -> None:
    """悬挂索环：双股粗麻吊绳、八角生铁环与穿索。"""
    b = "hang_loop"
    g.box(b, "hang_rope_l", (1.88, 11.65, -2.72), (2.22, 12.32, -2.45), "cord")
    g.box(b, "hang_rope_r", (2.32, 11.65, -2.72), (2.68, 12.32, -2.45), "cord_dk")
    g.box(b, "hang_rope_knot", (1.82, 11.82, -2.82), (2.75, 12.18, -2.38), "cord_dk")

    g.box(b, "ring_bar_t", (1.88, 11.75, -2.85), (2.72, 12.08, -2.48), "iron")
    g.box(b, "ring_bar_b", (1.88, 10.82, -2.85), (2.72, 11.15, -2.48), "iron")
    g.box(b, "ring_bar_l", (1.65, 11.05, -2.85), (1.98, 11.85, -2.48), "iron_lt")
    g.box(b, "ring_bar_r", (2.62, 11.05, -2.85), (2.95, 11.85, -2.48), "iron")

    g.box(b, "ring_corn_lt", (1.72, 11.58, -2.82), (2.08, 11.95, -2.52), "rust")
    g.box(b, "ring_corn_rt", (2.52, 11.58, -2.82), (2.88, 11.95, -2.52), "iron")
    g.box(b, "ring_corn_lb", (1.72, 10.95, -2.82), (2.08, 11.32, -2.52), "iron")
    g.box(b, "ring_corn_rb", (2.52, 10.95, -2.82), (2.88, 11.32, -2.52), "rust")
    g.box(b, "ring_center_void", (1.98, 11.15, -2.90), (2.62, 11.75, -2.42), "void")

    g.box(b, "ring_drop_cord_a", (2.10, 10.65, -2.78), (2.48, 11.10, -2.48), "sash_red")
    g.box(b, "ring_drop_knot", (2.02, 10.58, -2.86), (2.56, 10.92, -2.42), "sash_red_dk")


def part_token_pad(g: Gear) -> None:
    """减震皮托：腰牌后方厚牛皮垫板、包边与粗针脚。"""
    b = "token_pad"
    g.box(b, "pad_main_t", (0.55, 8.25, -2.52), (3.85, 11.35, -2.25), "leather")
    g.box(b, "pad_main_b_l", (0.55, 5.85, -2.52), (2.65, 8.35, -2.25), "leather")
    g.box(b, "pad_main_b_r", (2.55, 7.15, -2.52), (3.65, 8.35, -2.25), "leather_dk")

    g.box(b, "pad_edge_top", (0.48, 11.28, -2.56), (3.92, 11.58, -2.22), "leather_dk")
    g.box(b, "pad_edge_l", (0.42, 5.75, -2.56), (0.72, 11.45, -2.22), "leather_dk")
    g.box(b, "pad_edge_b_l", (0.48, 5.65, -2.56), (2.68, 5.95, -2.22), "leather_dk")
    g.box(b, "pad_edge_frac_r", (2.55, 6.95, -2.56), (3.72, 7.25, -2.22), "leather_dk")

    for k in range(6):
        y = 6.05 + k * 0.88
        g.box(b, f"pad_stitch_l_{k}", (0.55, y, -2.60), (0.85, y + 0.35, -2.28), "stitch")
    for k in range(4):
        x = 0.85 + k * 0.48
        g.box(b, f"pad_stitch_b_{k}", (x, 5.75, -2.60), (x + 0.32, 6.05, -2.28), "stitch")

    # 丰富皮垫边缘磨损撕裂小角
    g.box(b, "pad_tear_flap1", (2.42, 6.65, -2.54), (2.75, 7.15, -2.24), "leather_dk")
    g.box(b, "pad_tear_flap2", (3.45, 7.10, -2.54), (3.82, 7.55, -2.24), "leather")
    g.box(b, "pad_top_notch", (1.65, 11.38, -2.58), (2.05, 11.65, -2.25), "leather")


def part_token_main(g: Gear) -> None:
    """兽骨腰牌身：拱首厚骨牌身、凸棱边框与减薄底槽。"""
    b = "token_main"
    # 尺寸厚度饱满，Z 跨度达 0.62，三轴尺寸均 ≥ 0.62，用于硬件互穿自证
    g.box(b, "token_arch_base", (0.95, 10.35, -2.98), (3.45, 11.15, -2.36), "bone")
    g.box(b, "token_arch_crown", (1.35, 10.85, -2.98), (3.05, 11.32, -2.36), "bone_lt")
    g.box(b, "token_top_hole", (2.05, 10.45, -3.02), (2.55, 10.88, -2.32), "void")

    g.box(b, "token_plate_upper", (0.75, 8.15, -2.92), (3.65, 10.45, -2.38), "bone")
    g.box(b, "token_plate_lower_l", (0.75, 6.05, -2.92), (2.45, 8.25, -2.38), "bone")

    g.box(b, "token_rim_l", (0.68, 5.95, -2.98), (1.02, 11.05, -2.42), "bone_lt")
    g.box(b, "token_rim_r_top", (3.38, 8.15, -2.98), (3.72, 11.05, -2.42), "bone_lt")
    g.box(b, "token_rim_bot_l", (0.68, 5.95, -2.98), (2.35, 6.35, -2.42), "bone_lt")
    g.box(b, "token_rim_top_arch", (1.15, 10.95, -3.02), (3.25, 11.38, -2.42), "bone_lt")

    g.box(b, "token_dish_upper", (1.08, 8.25, -2.96), (3.32, 10.35, -2.72), "bone_dk")
    g.box(b, "token_dish_lower", (1.08, 6.35, -2.96), (2.28, 8.35, -2.72), "bone_dk")

    g.box(b, "token_foot_l_boss", (0.65, 5.85, -3.02), (1.05, 6.45, -2.40), "bone")
    g.box(b, "token_foot_l_crest", (0.60, 6.00, -3.08), (0.88, 6.35, -2.50), "bone_lt")

    g.box(b, "tassel_lug_core", (1.55, 5.65, -2.92), (2.15, 6.25, -2.44), "bone")
    g.box(b, "tassel_lug_hole", (1.72, 5.75, -3.02), (1.98, 6.12, -2.38), "void")
    g.box(b, "tassel_lug_ring", (1.48, 5.82, -2.98), (2.22, 6.08, -2.42), "bone_lt")


def part_token_fracture(g: Gear) -> None:
    """残破断角：右下斜向大裂口、多层阶梯残茬与横贯劈裂缝。"""
    b = "token_fracture"
    g.box(b, "frac_step1_body", (2.42, 7.75, -2.92), (3.52, 8.18, -2.50), "bone_stain")
    g.box(b, "frac_step1_spur", (2.82, 7.62, -2.95), (3.38, 7.95, -2.55), "bone_dk")
    g.box(b, "frac_step1_chip", (3.25, 7.92, -2.98), (3.62, 8.28, -2.60), "bone_lt")

    g.box(b, "frac_step2_body", (2.35, 7.22, -2.90), (3.05, 7.82, -2.52), "bone_stain")
    g.box(b, "frac_step2_spur", (2.65, 7.08, -2.94), (3.12, 7.42, -2.56), "bone_dk")
    g.box(b, "frac_step2_tooth", (2.85, 7.28, -2.98), (3.15, 7.62, -2.58), "bone_lt")

    g.box(b, "frac_step3_body", (2.28, 6.55, -2.88), (2.75, 7.28, -2.54), "bone_stain")
    g.box(b, "frac_step3_spur", (2.42, 6.38, -2.92), (2.82, 6.78, -2.56), "bone_dk")

    g.box(b, "crack_trunk_1", (2.25, 8.05, -3.02), (2.62, 8.85, -2.70), "bone_dk")
    g.box(b, "crack_trunk_2", (1.98, 8.75, -3.02), (2.35, 9.55, -2.70), "bone_stain")
    g.box(b, "crack_trunk_3", (1.68, 9.45, -3.00), (2.08, 10.15, -2.72), "bone_dk")
    g.box(b, "crack_branch_r", (2.28, 8.95, -3.00), (2.82, 9.35, -2.72), "bone_stain")
    g.box(b, "crack_branch_l", (1.55, 8.55, -2.98), (1.95, 8.88, -2.72), "bone_dk")

    g.box(b, "crack_pit_1", (2.12, 8.45, -3.08), (2.36, 8.75, -2.68), "void")
    g.box(b, "crack_pit_2", (1.85, 9.65, -3.08), (2.08, 9.95, -2.68), "void")

    g.box(b, "splinter_high", (2.55, 8.25, -2.98), (2.88, 8.65, -2.68), "bone_lt")
    g.box(b, "splinter_mid", (3.08, 8.28, -2.98), (3.42, 8.58, -2.68), "bone_lt")
    g.box(b, "splinter_low", (2.18, 6.25, -2.95), (2.48, 6.62, -2.68), "bone_lt")
    g.box(b, "splinter_flake1", (2.75, 7.45, -3.02), (3.05, 7.75, -2.72), "bone_lt")
    g.box(b, "splinter_flake2", (2.35, 6.85, -2.98), (2.65, 7.15, -2.70), "bone_stain")


def part_token_crest(g: Gear) -> None:
    """正面浅浮雕宗门流云残纹：云纹断弧、旋涡中心与起凸残棱。"""
    b = "token_crest"
    g.box(b, "crest_eye_outer", (1.68, 8.88, -3.05), (2.22, 9.42, -2.75), "bone_lt")
    g.box(b, "crest_eye_pit", (1.82, 9.02, -3.12), (2.08, 9.28, -2.82), "bone_dk")

    g.box(b, "crest_wing_tl", (1.25, 9.25, -3.02), (1.88, 9.68, -2.76), "bone_lt")
    g.box(b, "crest_wing_tr", (2.02, 9.32, -3.02), (2.68, 9.75, -2.76), "bone_lt")
    g.box(b, "crest_curl_top", (1.12, 9.52, -3.02), (1.52, 9.98, -2.76), "bone_lt")
    g.box(b, "crest_wing_bl", (1.32, 8.55, -3.02), (1.95, 8.95, -2.76), "bone_lt")
    g.box(b, "crest_curl_bot", (1.18, 8.18, -3.02), (1.58, 8.65, -2.76), "bone_lt")

    g.box(b, "crest_stream_a", (1.45, 7.75, -3.00), (2.18, 8.08, -2.76), "bone_lt")
    g.box(b, "crest_stream_b", (1.55, 7.35, -3.00), (2.28, 7.68, -2.76), "bone_lt")
    g.box(b, "crest_stream_c", (1.65, 6.95, -3.00), (2.22, 7.28, -2.76), "bone_lt")

    g.box(b, "crest_shade_t", (1.75, 9.68, -2.98), (2.55, 9.92, -2.74), "bone_dk")
    g.box(b, "crest_shade_m", (1.22, 7.95, -2.98), (2.02, 8.22, -2.74), "bone_dk")
    g.box(b, "crest_ribbon_1", (1.35, 8.95, -3.02), (1.75, 9.28, -2.78), "bone_lt")
    g.box(b, "crest_ribbon_2", (2.15, 8.65, -3.02), (2.55, 8.98, -2.78), "bone_lt")


def part_token_wrap(g: Gear) -> None:
    """牌首加固铁箍：夹片铁板、贯通铆钉与多道红丝缠绳。"""
    b = "token_wrap"
    g.box(b, "clamp_plate_f", (1.12, 10.18, -3.05), (3.28, 10.68, -2.78), "iron")
    g.box(b, "clamp_plate_b", (1.12, 10.18, -2.52), (3.28, 10.68, -2.26), "iron_lt")
    g.box(b, "clamp_plate_t", (1.12, 10.58, -3.05), (3.28, 10.82, -2.26), "iron")

    g.box(b, "clamp_rivet_l", (1.28, 10.28, -3.15), (1.62, 10.58, -2.85), "iron_lt")
    g.box(b, "clamp_rivet_r", (2.78, 10.28, -3.15), (3.12, 10.58, -2.85), "iron_lt")
    g.box(b, "clamp_rust_spot", (1.72, 10.22, -3.08), (2.18, 10.62, -2.76), "rust")

    for k in range(4):
        y = 9.42 + k * 0.18
        mat = "sash_red" if k % 2 == 0 else "sash_red_dk"
        g.box(b, f"wrap_cord_f_{k}", (0.95, y, -3.00), (3.45, y + 0.16, -2.74), mat)
        g.box(b, f"wrap_cord_b_{k}", (0.95, y, -2.56), (3.45, y + 0.16, -2.30), mat)
        g.box(b, f"wrap_cord_l_{k}", (0.82, y, -3.00), (1.08, y + 0.16, -2.30), mat)
        g.box(b, f"wrap_cord_r_{k}", (3.32, y, -3.00), (3.58, y + 0.16, -2.30), mat)

    g.box(b, "wrap_center_knot", (1.92, 9.45, -3.12), (2.48, 10.05, -2.70), "sash_red_dk")
    g.box(b, "wrap_bone_pin", (1.65, 9.62, -3.20), (2.75, 9.88, -2.85), "bone_lt")
    g.box(b, "wrap_iron_notch", (2.28, 10.15, -3.10), (2.58, 10.45, -2.80), "rust")


def part_hang_tassel(g: Gear) -> None:
    """悬垂流苏索：从腰牌下穿孔穿出的多股粗麻红丝悬绳结。"""
    b = "hang_tassel"
    g.box(b, "tassel_neck_l", (1.68, 5.35, -2.85), (1.92, 5.95, -2.52), "sash_red")
    g.box(b, "tassel_neck_r", (1.90, 5.35, -2.85), (2.12, 5.95, -2.52), "sash_red_dk")

    g.box(b, "tassel_knot_main", (1.52, 5.02, -2.96), (2.28, 5.58, -2.40), "sash_red_dk")
    g.box(b, "tassel_knot_cross_a", (1.42, 5.12, -3.02), (2.38, 5.48, -2.34), "sash_red")
    g.box(b, "tassel_knot_cross_b", (1.62, 4.95, -3.08), (2.18, 5.65, -2.28), "sash_red_dk")
    g.box(b, "tassel_knot_eye", (1.75, 5.18, -3.12), (2.05, 5.42, -2.82), "cord_dk")

    g.box(b, "tassel_stem_l", (1.72, 4.45, -2.82), (1.95, 5.15, -2.50), "sash_red")
    g.box(b, "tassel_stem_r", (1.92, 4.45, -2.82), (2.15, 5.15, -2.50), "sash_red_dk")
    g.box(b, "tassel_tie_wrap", (1.65, 4.75, -2.88), (2.22, 4.98, -2.45), "cord_dk")


def part_tassel_beads(g: Gear) -> None:
    """串结骨珠：流苏中段穿缀的多棱兽骨管、隔片与微型残青铜铃。"""
    b = "tassel_beads"
    # 三轴尺寸均 ≥ 0.62，用于穿模门禁自证
    g.box(b, "bead_body", (1.55, 4.62, -2.98), (2.32, 5.25, -2.36), "bone")
    g.box(b, "bead_rim_t", (1.52, 5.08, -3.02), (2.35, 5.28, -2.32), "bone_lt")
    g.box(b, "bead_rim_b", (1.52, 4.58, -3.02), (2.35, 4.78, -2.32), "bone_lt")
    g.box(b, "bead_groove_m", (1.55, 4.85, -3.05), (2.32, 5.02, -2.30), "bone_dk")

    g.box(b, "bead_spacer", (1.68, 4.42, -2.88), (2.18, 4.65, -2.48), "cord_dk")

    # 残破小青铜铃（四方体，三轴尺寸均 ≥ 0.62）
    g.box(b, "bell_main", (1.52, 3.88, -2.98), (2.35, 4.52, -2.36), "bronze")
    g.box(b, "bell_rim", (1.45, 3.82, -3.02), (2.42, 4.10, -2.32), "bronze")
    g.box(b, "bell_crack_rust", (1.50, 4.02, -3.05), (1.82, 4.38, -2.58), "rust")
    g.box(b, "bell_void_hole", (1.75, 3.75, -2.90), (2.12, 4.02, -2.42), "void")


def part_tassel_tails(g: Gear) -> None:
    """散开羽须：十数条参差下垂的粗细丝绒尾。"""
    b = "tassel_tails"
    g.box(b, "tail_core_a", (1.75, 3.05, -2.85), (2.12, 3.95, -2.50), "sash_red_dk")
    g.box(b, "tail_core_b", (1.78, 2.15, -2.82), (2.08, 3.15, -2.52), "sash_red")

    strands = (
        ("l1", 1.48, 1.76, 2.65, 3.75, -2.88, -2.52, "sash_red"),
        ("l2", 1.38, 1.68, 2.05, 3.05, -2.85, -2.55, "sash_red_dk"),
        ("l3", 1.28, 1.58, 1.55, 2.35, -2.82, -2.56, "cord"),
        ("l4", 1.18, 1.48, 1.25, 1.95, -2.80, -2.58, "cord_dk"),
        ("r1", 2.12, 2.40, 2.55, 3.75, -2.88, -2.52, "sash_red"),
        ("r2", 2.22, 2.52, 1.95, 2.95, -2.85, -2.55, "sash_red_dk"),
        ("r3", 2.32, 2.62, 1.45, 2.25, -2.82, -2.56, "cord_dk"),
        ("r4", 2.42, 2.72, 1.15, 1.85, -2.80, -2.58, "cord"),
        ("f1", 1.65, 1.98, 2.40, 3.55, -3.02, -2.70, "sash_red_dk"),
        ("f2", 1.88, 2.22, 2.20, 3.35, -3.02, -2.70, "sash_red"),
        ("f3", 1.75, 2.05, 1.65, 2.45, -2.98, -2.72, "sash_red_dk"),
        ("f4", 1.82, 2.15, 1.15, 1.95, -2.96, -2.72, "sash_red"),
        ("b1", 1.68, 2.02, 2.30, 3.45, -2.60, -2.32, "sash_red"),
        ("b2", 1.85, 2.18, 2.50, 3.65, -2.60, -2.32, "sash_red_dk"),
        ("b3", 1.72, 2.08, 1.55, 2.55, -2.58, -2.32, "sash_red"),
        ("tip_l", 1.35, 1.62, 0.95, 1.65, -2.80, -2.58, "cord"),
        ("tip_r", 2.25, 2.55, 0.85, 1.55, -2.80, -2.58, "cord_dk"),
        ("tip_m", 1.82, 2.05, 0.75, 1.65, -2.78, -2.56, "sash_red_dk"),
    )
    for name, x0, x1, y0, y1, z0, z1, mat in strands:
        g.box(b, f"strand_{name}", (x0, y0, z0), (x1, y1, z1), mat)


def part_side_toggle(g: Gear) -> None:
    """侧挂骨签：辅助穿扣的雕槽细兽骨签与麻绳结。"""
    b = "side_toggle"
    g.box(b, "toggle_cord_drop1", (2.75, 11.25, -2.78), (3.15, 11.75, -2.46), "cord")
    g.box(b, "toggle_cord_knot", (3.02, 10.92, -2.86), (3.42, 11.38, -2.40), "cord_dk")
    g.box(b, "toggle_cord_drop2", (3.12, 10.42, -2.78), (3.38, 11.05, -2.46), "cord")

    g.box(b, "toggle_bone_shaft", (3.08, 9.42, -2.80), (3.42, 10.55, -2.44), "bone")
    g.box(b, "toggle_bone_head", (2.98, 10.35, -2.88), (3.52, 10.75, -2.38), "bone_lt")
    g.box(b, "toggle_bone_tail", (2.98, 9.15, -2.88), (3.52, 9.55, -2.38), "bone_stain")
    g.box(b, "toggle_bone_slit", (2.95, 9.82, -2.92), (3.55, 10.08, -2.35), "bone_dk")
    g.box(b, "toggle_bead", (3.05, 10.15, -2.86), (3.45, 10.35, -2.40), "bone_lt")
    g.box(b, "toggle_ring_tie", (3.02, 10.58, -2.82), (3.48, 10.78, -2.42), "cord_dk")


# ───────────────────────────────────────────────────────────── 门禁与共面自动错开
def deconflict_and_fix(g: Gear) -> None:
    """消除不对称部件内的 Z-fighting 共面，并确保无薄片。"""
    all_els = [e for e in g.rig.elements if not any(e.get("rotation", [0, 0, 0]))]
    asym_uuids = set()
    for b_name in ASYM_BONES:
        if b_name in g.rig.bones:
            asym_uuids.update(g.rig.bones[b_name]["children"])

    # 1. 修正尺寸 < 0.22 的薄片
    for e in all_els:
        for ax in range(3):
            s = e["to"][ax] - e["from"][ax]
            if s < 0.22:
                d = 0.22 - s
                e["from"][ax] = round(e["from"][ax] - d / 2, 4)
                e["to"][ax] = round(e["to"][ax] + d / 2, 4)

    # 2. 消除不对称件与其它件的共面冲突
    asym_els = [e for e in all_els if e["uuid"] in asym_uuids]
    for i, e in enumerate(asym_els):
        idx = all_els.index(e)
        for _ in range(25):
            moved = False
            for f in all_els[:idx]:
                for ax in range(3):
                    o1, o2 = [a for a in range(3) if a != ax]
                    if min(e["to"][o1], f["to"][o1]) - max(e["from"][o1], f["from"][o1]) <= 1e-4:
                        continue
                    if min(e["to"][o2], f["to"][o2]) - max(e["from"][o2], f["from"][o2]) <= 1e-4:
                        continue
                    for side, sign in (("from", -1), ("to", 1)):
                        if abs(e[side][ax] - f[side][ax]) < 1e-4:
                            e[side][ax] = round(e[side][ax] + sign * 0.003 * (1 + (i % 5)), 4)
                            moved = True
            if not moved:
                break


class CanPoYaoPaiSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "waist_sash": part_waist_sash,
    "sash_knot": part_sash_knot,
    "hang_loop": part_hang_loop,
    "token_pad": part_token_pad,
    "token_main": part_token_main,
    "token_fracture": part_token_fracture,
    "token_crest": part_token_crest,
    "token_wrap": part_token_wrap,
    "hang_tassel": part_hang_tassel,
    "tassel_beads": part_tassel_beads,
    "tassel_tails": part_tassel_tails,
    "side_toggle": part_side_toggle,
}

SPEC = CanPoYaoPaiSpec(
    gear_id="can_po_yao_pai",
    model_name="CanPoYaoPai",
    title="残破宗门腰牌 / can_po_yao_pai",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：右前胯 · 避开手臂摆动盒 · 裂角兽骨腰牌+流云残纹+红绳流苏",
    worn_focus=((2.0, 8.5, -1.0), 18.0),
    textured=False,
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
