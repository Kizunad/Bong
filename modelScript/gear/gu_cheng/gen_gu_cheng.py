#!/usr/bin/env python3
"""骨秤（gu_cheng）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

对应 NPC：骨币游商（gu_bi_you_shang）。
挂点：枯藤腰带左后腰（x∈[-5.5, -3.2], y∈[6.0, 13.8], z∈[2.08, 3.8]），
避开腰前偏右的骨币匣，避开双臂摆动盒（x∈[-8,-4], y∈[12,24], z∈[-2,2]）并在侧后留出安全间隙。

外形：打磨泛黄的微弧兽骨秤杆，刻 8 道浅凹秤星；草绳双提纽，上方皮套挂环；
前头细麻绳吊着一块碎骨凿制的浅弧秤盘；长臂悬吊一枚八面粗铸的锈铁秤砣；
秤尾吊一串配重骨珠与麻绳流苏。

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
    "rust": (122, 66, 36),
    "rust_dk": (78, 42, 26),
    "hide": (106, 70, 48),
    "hide_dk": (70, 48, 31),
    "stitch": (188, 174, 148),
    "void": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_strap", "root", (0.0, 12.8, 2.4), "腰后皮带片与带扣"),
    ("mount_hook", "belt_strap", (-4.0, 12.0, 2.5), "左后腰皮挂套与骨插销"),
    ("iron_ring", "mount_hook", (-4.0, 11.2, 2.5), "悬挂铸铁环与防磨垫"),
    ("cords_pivot", "iron_ring", (-4.0, 10.4, 2.5), "草绳双提纽与抱箍"),
    ("beam_main", "cords_pivot", (-4.0, 9.8, 2.5), "微弧兽骨秤杆主轴"),
    ("beam_marks", "beam_main", (-5.0, 9.7, 2.5), "不等距浅凹秤星"),
    ("beam_head", "beam_main", (-2.6, 9.9, 2.5), "秤杆前段骨节与挂钩"),
    ("beam_tail", "beam_main", (-6.3, 9.3, 2.8), "秤杆尾部骨节与穿孔"),
    ("pan_cords", "beam_head", (-2.6, 8.2, 2.5), "秤盘三向吊绳"),
    ("pan_plate", "pan_cords", (-2.5, 6.8, 2.5), "碎骨浅弧秤盘本体"),
    ("weight_loop", "beam_main", (-5.1, 9.4, 2.6), "秤砣滑动绳套"),
    ("weight_iron", "weight_loop", (-5.1, 7.5, 2.6), "八角铸铁秤砣本体"),
    ("tail_tassel", "beam_tail", (-6.4, 8.2, 2.8), "尾端骨珠与流苏"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "belt_strap")])
SOFT_OVER = (
    ("bone", "bone_lt"), ("bone", "bone_dk"), ("bone", "bone_stain"),
    ("cord", "cord_dk"), ("iron", "rust"), ("rust", "rust_dk"),
    ("hide", "hide_dk"), ("bone", "void"), ("cord", "bone"),
    ("hide", "cord"), ("iron", "cord"), ("hide", "stitch"),
    ("iron", "bone"), ("hide", "iron"),
)


def part_belt(g: Gear) -> None:
    """腰后皮带片与带扣。"""
    b = "belt_strap"
    # 中央背扣底板与外盖 (z: 2.08 .. 2.82)
    g.box(b, "belt_center_core", (-1.25, 12.15, 2.08), (1.25, 13.65, 2.45), "hide_dk")
    g.box(b, "belt_center_cover", (-0.95, 12.22, 2.42), (0.95, 13.58, 2.72), "hide")
    g.box(b, "belt_center_bone_pin", (-0.35, 12.02, 2.68), (0.35, 13.78, 2.98), "bone")
    g.box(b, "belt_pin_head", (-0.28, 13.74, 2.71), (0.28, 14.05, 2.96), "bone_lt")
    g.box(b, "belt_pin_tail", (-0.25, 11.75, 2.71), (0.25, 12.06, 2.96), "bone_dk")

    # 左右延伸皮带片
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.10), (4.48, 13.62, 2.44), "hide_dk")
    g.pair(b, "belt_wing_outer", (1.24, 12.25, 2.41), (4.44, 13.55, 2.68), "hide")
    g.pair(b, "belt_edge_top", (1.22, 13.58, 2.13), (4.46, 13.88, 2.52), "hide_dk")
    g.pair(b, "belt_edge_bot", (1.22, 11.92, 2.13), (4.46, 12.22, 2.52), "hide_dk")

    # 对称骨铆钉与粗缝线 (尺寸均 >= 0.26)
    g.pair(b, "belt_rivet_mid", (1.82, 12.70, 2.65), (2.18, 13.10, 2.95), "bone")
    g.pair(b, "belt_rivet_lat", (3.25, 12.70, 2.65), (3.58, 13.10, 2.95), "bone")
    g.pair(b, "belt_stitch_1", (1.32, 13.22, 2.65), (1.68, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_2", (2.52, 13.22, 2.65), (2.88, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_3", (3.82, 13.22, 2.65), (4.18, 13.54, 2.92), "stitch")
    g.pair(b, "belt_stitch_b1", (1.32, 12.26, 2.65), (1.68, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.65), (2.88, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")

    # 左右防磨加固垫与外延挂鼻
    g.pair(b, "belt_pad_reinforce", (2.15, 12.35, 2.44), (2.95, 13.45, 2.74), "hide_dk")
    g.pair(b, "belt_side_loop", (4.22, 12.35, 2.45), (4.56, 13.45, 2.82), "cord_dk")


def part_mount(g: Gear) -> None:
    """左后腰皮挂套与骨插销。"""
    b = "mount_hook"
    # 厚兽皮挂套
    g.box(b, "mount_back_wall", (-4.42, 12.12, 2.06), (-3.52, 13.68, 2.34), "hide_dk")
    g.box(b, "mount_front_wall", (-4.46, 12.06, 2.66), (-3.48, 13.74, 2.96), "hide")
    g.box(b, "mount_top_lip", (-4.50, 13.60, 2.16), (-3.44, 13.94, 2.78), "hide")
    g.box(b, "mount_bot_loop", (-4.36, 11.56, 2.22), (-3.58, 12.18, 2.84), "hide_dk")

    # 挂套下垂悬挂舌
    g.box(b, "mount_tongue_pad", (-4.26, 11.18, 2.35), (-3.68, 11.74, 2.70), "hide")
    g.box(b, "mount_tab_front", (-4.30, 11.40, 2.72), (-3.64, 12.04, 3.02), "hide_dk")

    # 粗骨插销横锁
    g.box(b, "mount_bone_pin_body", (-4.68, 12.40, 2.76), (-3.22, 12.86, 3.08), "bone")
    g.box(b, "mount_bone_pin_flange", (-4.86, 12.32, 2.73), (-4.62, 12.94, 3.12), "bone_lt")
    g.box(b, "mount_bone_pin_tip", (-3.28, 12.46, 2.80), (-3.04, 12.82, 3.04), "bone_dk")

    # 固定麻绳缠绑
    g.box(b, "mount_cord_wrap_l", (-4.55, 12.26, 2.62), (-4.20, 13.00, 2.94), "cord_dk")
    g.box(b, "mount_cord_wrap_r", (-3.75, 12.26, 2.62), (-3.40, 13.00, 2.94), "cord")


def part_ring(g: Gear) -> None:
    """悬挂铸铁环与防磨软垫。"""
    r = "iron_ring"
    # 挂环顶端抱卡
    g.box(r, "iron_lug_top", (-4.25, 11.10, 2.38), (-3.70, 11.46, 2.74), "iron")
    # 铁环四段加厚铸造
    g.box(r, "iron_ring_upper", (-4.35, 10.75, 2.40), (-3.60, 11.18, 2.72), "rust")
    g.box(r, "iron_ring_left", (-4.52, 10.20, 2.39), (-4.06, 10.90, 2.71), "iron")
    g.box(r, "iron_ring_right", (-3.88, 10.20, 2.41), (-3.44, 10.90, 2.73), "rust_dk")
    g.box(r, "iron_ring_bottom", (-4.36, 9.95, 2.40), (-3.58, 10.34, 2.72), "iron")

    # 防磨软兽皮衬片与铁锈凸斑
    g.box(r, "iron_ring_hide_pad", (-4.28, 9.92, 2.36), (-3.66, 10.19, 2.76), "hide_dk")
    g.box(r, "iron_ring_rust_scab", (-4.45, 10.40, 2.70), (-4.02, 10.84, 2.96), "rust")


def part_pivot(g: Gear) -> None:
    """草绳双提纽与抱箍。"""
    c = "cords_pivot"
    # 主提纽顶结与双股下挂
    g.box(c, "piv1_top_knot", (-4.24, 9.82, 2.38), (-3.70, 10.20, 2.74), "cord_dk")
    g.box(c, "piv1_strand_l", (-4.26, 9.18, 2.39), (-3.92, 9.96, 2.67), "cord")
    g.box(c, "piv1_strand_r", (-3.90, 9.19, 2.47), (-3.56, 9.92, 2.73), "cord_dk")
    # 主纽在秤杆上的粗麻绳抱箍环
    g.box(c, "piv1_collar_core", (-4.35, 8.90, 2.34), (-3.60, 9.36, 2.74), "cord_dk")
    g.box(c, "piv1_collar_ridge", (-4.28, 9.24, 2.36), (-3.66, 9.50, 2.76), "cord")

    # 副提纽
    g.box(c, "piv2_top_knot", (-3.68, 9.85, 2.44), (-3.15, 10.24, 2.80), "cord_dk")
    g.box(c, "piv2_strand", (-3.64, 9.22, 2.46), (-3.20, 9.98, 2.78), "cord")
    g.box(c, "piv2_collar", (-3.68, 8.92, 2.38), (-3.18, 9.40, 2.84), "cord_dk")

    # 提纽悬挂的小骨栓
    g.box(c, "piv_bone_toggle", (-4.45, 9.35, 2.46), (-4.10, 9.88, 2.78), "bone_lt")


def part_beam(g: Gear) -> None:
    """微弧兽骨秤杆主轴。"""
    bm = "beam_main"
    # ── 支点中央段
    g.box(bm, "beam_sec_mid_c", (-4.44, 8.96, 2.42), (-3.36, 9.35, 2.72), "bone")
    g.box(bm, "beam_sec_mid_t", (-4.38, 9.26, 2.44), (-3.42, 9.55, 2.70), "bone_lt")
    g.box(bm, "beam_sec_mid_b", (-4.38, 8.74, 2.44), (-3.42, 9.02, 2.70), "bone_dk")
    g.box(bm, "beam_sec_mid_f", (-4.42, 9.00, 2.20), (-3.38, 9.28, 2.46), "bone_stain")
    g.box(bm, "beam_sec_mid_k", (-4.42, 9.00, 2.68), (-3.38, 9.28, 2.94), "bone")

    # ── 支点向前伸展段
    g.box(bm, "beam_sec_fwd_c", (-3.48, 9.06, 2.36), (-2.65, 9.42, 2.66), "bone")
    g.box(bm, "beam_sec_fwd_t", (-3.44, 9.36, 2.43), (-2.69, 9.64, 2.69), "bone_lt")
    g.box(bm, "beam_sec_fwd_b", (-3.44, 8.84, 2.43), (-2.69, 9.12, 2.69), "bone_dk")
    g.box(bm, "beam_sec_fwd_f", (-3.46, 9.10, 2.19), (-2.67, 9.38, 2.45), "bone_stain")

    # ── 支点向后第 1 段
    g.box(bm, "beam_sec_rear1_c", (-5.15, 8.86, 2.45), (-4.35, 9.22, 2.75), "bone")
    g.box(bm, "beam_sec_rear1_t", (-5.11, 9.16, 2.47), (-4.39, 9.45, 2.73), "bone_lt")
    g.box(bm, "beam_sec_rear1_b", (-5.11, 8.64, 2.47), (-4.39, 8.92, 2.73), "bone_dk")
    g.box(bm, "beam_sec_rear1_f", (-5.13, 8.92, 2.23), (-4.37, 9.18, 2.49), "bone_stain")
    g.box(bm, "beam_sec_rear1_k", (-5.13, 8.90, 2.71), (-4.37, 9.18, 2.97), "bone")

    # ── 支点向后第 2 段
    g.box(bm, "beam_sec_rear2_c", (-5.85, 8.75, 2.50), (-5.05, 9.06, 2.85), "bone")
    g.box(bm, "beam_sec_rear2_t", (-5.81, 8.96, 2.52), (-5.09, 9.28, 2.78), "bone_lt")
    g.box(bm, "beam_sec_rear2_b", (-5.81, 8.48, 2.52), (-5.09, 8.76, 2.78), "bone_dk")
    g.box(bm, "beam_sec_rear2_f", (-5.83, 8.74, 2.28), (-5.07, 9.02, 2.54), "bone_stain")

    # ── 支点向后第 3 段
    g.box(bm, "beam_sec_rear3_c", (-6.55, 8.55, 2.58), (-5.75, 8.86, 2.84), "bone")
    g.box(bm, "beam_sec_rear3_t", (-6.51, 8.76, 2.60), (-5.79, 9.08, 2.86), "bone_lt")
    g.box(bm, "beam_sec_rear3_b", (-6.51, 8.28, 2.60), (-5.79, 8.56, 2.86), "bone_dk")
    g.box(bm, "beam_sec_rear3_f", (-6.53, 8.54, 2.36), (-5.77, 8.82, 2.62), "bone_stain")

    # 秤杆两处粗麻绳缠绕修补箍
    g.box(bm, "beam_cord_wrap1_a", (-3.94, 8.86, 2.32), (-3.52, 9.42, 2.81), "cord_dk")
    g.box(bm, "beam_cord_wrap1_b", (-3.54, 8.90, 2.35), (-3.16, 9.33, 2.74), "cord")
    g.box(bm, "beam_cord_wrap2_a", (-6.00, 8.52, 2.44), (-5.58, 9.05, 2.93), "cord_dk")
    g.box(bm, "beam_cord_wrap2_b", (-5.60, 8.56, 2.48), (-5.22, 9.04, 2.84), "cord")


def part_marks(g: Gear) -> None:
    """不等距浅凹秤星刻度。"""
    bm = "beam_marks"
    marks = [
        (-4.48, 9.20, 2.45),
        (-4.72, 9.12, 2.47),
        (-4.98, 9.05, 2.50),
        (-5.32, 8.95, 2.54),
        (-5.62, 8.85, 2.58),
        (-5.92, 8.75, 2.62),
        (-6.18, 8.65, 2.66),
        (-6.42, 8.55, 2.70),
    ]
    for i, (mx, my, mz) in enumerate(marks):
        g.box(bm, f"mark_groove_{i}", (mx - 0.12, my + 0.18, mz - 0.05), (mx + 0.14, my + 0.45, mz + 0.22), "cord_dk")
        g.box(bm, f"mark_dot_{i}", (mx - 0.12, my - 0.15, mz - 0.28), (mx + 0.14, my + 0.12, mz - 0.02), "bone_stain")


def part_beam_head(g: Gear) -> None:
    """秤杆前段骨节与挂钩。"""
    bh = "beam_head"
    # 前端骨节球形膨大髁部
    g.box(bh, "head_knob_core", (-2.75, 9.05, 2.38), (-2.15, 9.55, 2.74), "bone")
    g.box(bh, "head_knob_top", (-2.70, 9.50, 2.41), (-2.20, 9.78, 2.71), "bone_lt")
    g.box(bh, "head_knob_bot", (-2.70, 8.82, 2.41), (-2.20, 9.10, 2.71), "bone_dk")
    g.box(bh, "head_knob_end", (-2.20, 9.10, 2.42), (-1.90, 9.50, 2.70), "bone_stain")

    # 前段骨孔与加固铁箍
    g.box(bh, "head_iron_collar", (-2.68, 9.00, 2.32), (-2.35, 9.60, 2.80), "rust_dk")
    g.box(bh, "head_collar_clasp", (-2.65, 9.54, 2.40), (-2.38, 9.82, 2.72), "iron")

    # 下方悬吊锻打铁钩
    g.box(bh, "head_hook_shank", (-2.62, 8.48, 2.42), (-2.32, 9.12, 2.70), "iron")
    g.box(bh, "head_hook_bend", (-2.65, 8.18, 2.38), (-2.28, 8.58, 2.74), "rust")
    g.box(bh, "head_hook_point", (-2.36, 8.25, 2.41), (-2.06, 8.70, 2.71), "iron")


def part_beam_tail(g: Gear) -> None:
    """秤杆尾部骨节与穿孔。"""
    bt = "beam_tail"
    # 尾部膨大骨节
    g.box(bt, "tail_knob_core", (-7.05, 8.35, 2.62), (-6.42, 8.88, 3.00), "bone")
    g.box(bt, "tail_knob_top", (-6.98, 8.84, 2.61), (-6.48, 9.12, 2.97), "bone_lt")
    g.box(bt, "tail_knob_bot", (-6.98, 8.12, 2.68), (-6.43, 8.40, 2.99), "bone_dk")
    g.box(bt, "tail_knob_tip", (-7.28, 8.42, 2.67), (-6.95, 8.82, 2.96), "bone_stain")
    # 穿绳暗孔与麻绳加固圈 (dz >= 0.25)
    g.box(bt, "tail_hole_recess", (-6.95, 8.48, 2.69), (-6.52, 8.78, 2.95), "void")
    g.box(bt, "tail_cord_banding", (-6.58, 8.32, 2.53), (-6.22, 8.92, 3.04), "cord_dk")


def part_pan_cords(g: Gear) -> None:
    """秤盘三向吊绳。"""
    pc = "pan_cords"
    # 吊钩下顶结
    g.box(pc, "pan_cords_apex", (-2.62, 8.00, 2.40), (-2.23, 8.42, 2.72), "cord_dk")

    # 绳 1（向前延伸，良好搭接）
    g.box(pc, "pan_cord1_seg1", (-2.48, 7.48, 2.31), (-2.16, 8.12, 2.59), "cord")
    g.box(pc, "pan_cord1_seg2", (-2.35, 6.95, 2.22), (-2.02, 7.65, 2.52), "cord_dk")
    g.box(pc, "pan_cord1_seg3", (-2.12, 6.42, 2.10), (-1.78, 7.10, 2.40), "cord")

    # 绳 2（向后延伸，良好搭接）
    g.box(pc, "pan_cord2_seg1", (-2.46, 7.52, 2.55), (-2.20, 8.18, 2.83), "cord")
    g.box(pc, "pan_cord2_seg2", (-2.32, 6.95, 2.65), (-2.00, 7.65, 2.95), "cord_dk")
    g.box(pc, "pan_cord2_seg3", (-2.10, 6.42, 2.78), (-1.78, 7.10, 3.08), "cord")

    # 绳 3（向外侧延伸，良好搭接）
    g.box(pc, "pan_cord3_seg1", (-2.88, 7.46, 2.43), (-2.42, 8.10, 2.69), "cord")
    g.box(pc, "pan_cord3_seg2", (-3.12, 6.95, 2.40), (-2.72, 7.65, 2.72), "cord_dk")
    g.box(pc, "pan_cord3_seg3", (-3.40, 6.42, 2.45), (-3.00, 7.10, 2.70), "cord")


def part_pan_plate(g: Gear) -> None:
    """碎骨浅弧秤盘本体。"""
    pp = "pan_plate"
    # 秤盘中心底板
    g.box(pp, "pan_base_core", (-2.92, 6.22, 2.20), (-1.75, 6.58, 3.10), "bone")
    g.box(pp, "pan_base_dish", (-2.82, 6.32, 2.30), (-1.88, 6.65, 3.00), "bone_stain")
    g.box(pp, "pan_base_bot_rib", (-2.78, 6.02, 2.32), (-1.92, 6.35, 2.98), "bone_dk")

    # 八角浅卷边
    g.box(pp, "pan_rim_n", (-2.84, 6.40, 2.02), (-1.90, 6.85, 2.34), "bone")
    g.box(pp, "pan_rim_n_lip", (-2.75, 6.80, 2.05), (-1.95, 7.08, 2.32), "bone_lt")
    g.box(pp, "pan_rim_s", (-2.84, 6.40, 2.96), (-1.90, 6.85, 3.28), "bone")
    g.box(pp, "pan_rim_s_lip", (-2.75, 6.80, 2.98), (-1.95, 7.08, 3.25), "bone_lt")
    g.box(pp, "pan_rim_e", (-1.85, 6.40, 2.26), (-1.58, 6.85, 3.04), "bone")
    g.box(pp, "pan_rim_e_lip", (-1.82, 6.80, 2.32), (-1.55, 7.08, 2.98), "bone_lt")
    g.box(pp, "pan_rim_w", (-3.12, 6.40, 2.26), (-2.85, 6.85, 3.04), "bone")
    g.box(pp, "pan_rim_w_lip", (-3.15, 6.80, 2.32), (-2.88, 7.08, 2.98), "bone_lt")

    # 四切角斜边
    g.box(pp, "pan_corner_ne", (-2.08, 6.36, 2.08), (-1.70, 6.82, 2.45), "bone_stain")
    g.box(pp, "pan_corner_nw", (-3.00, 6.44, 2.10), (-2.62, 6.82, 2.45), "bone_stain")
    g.box(pp, "pan_corner_se", (-2.08, 6.36, 2.85), (-1.70, 6.82, 3.20), "bone_stain")
    g.box(pp, "pan_corner_sw", (-3.00, 6.36, 2.85), (-2.62, 6.82, 3.20), "bone_stain")

    # 三个吊绳穿孔系结
    g.box(pp, "pan_knot_1", (-1.95, 6.52, 2.12), (-1.65, 6.95, 2.41), "cord_dk")
    g.box(pp, "pan_knot_2", (-1.96, 6.52, 2.82), (-1.68, 6.95, 3.16), "cord_dk")
    g.box(pp, "pan_knot_3", (-3.22, 6.52, 2.38), (-2.86, 6.95, 2.74), "cord_dk")

    # 碎裂修补铁锔钉
    g.box(pp, "pan_staple_1", (-2.55, 6.15, 2.45), (-2.15, 6.52, 2.82), "rust")
    g.box(pp, "pan_staple_2", (-2.65, 6.42, 2.47), (-2.25, 6.75, 2.78), "rust_dk")


def part_weight_loop(g: Gear) -> None:
    """秤砣滑动绳套。"""
    wl = "weight_loop"
    g.box(wl, "wloop_collar_t", (-5.37, 9.08, 2.41), (-4.83, 9.47, 2.83), "cord_dk")
    g.box(wl, "wloop_collar_b", (-5.37, 8.50, 2.41), (-4.83, 8.86, 2.83), "cord_dk")
    g.box(wl, "wloop_collar_f", (-5.33, 8.78, 2.30), (-4.87, 9.19, 2.55), "cord")
    g.box(wl, "wloop_collar_k", (-5.33, 8.78, 2.69), (-4.87, 9.19, 2.95), "cord")

    # 下垂悬吊双股麻绳
    g.box(wl, "wloop_drop_a", (-5.30, 7.95, 2.49), (-4.94, 8.72, 2.80), "cord")
    g.box(wl, "wloop_drop_b", (-5.28, 7.45, 2.45), (-4.92, 8.12, 2.78), "cord_dk")

    # 滑环限位小骨签
    g.box(wl, "wloop_bone_wedge", (-5.42, 9.18, 2.50), (-4.98, 9.55, 2.80), "bone_lt")


def part_weight_iron(g: Gear) -> None:
    """八角铸铁秤砣本体。"""
    wi = "weight_iron"
    # 砣顶吊耳
    g.box(wi, "weight_lug_arch", (-5.32, 7.25, 2.42), (-4.88, 7.70, 2.80), "iron")
    g.box(wi, "weight_lug_hole", (-5.25, 7.30, 2.50), (-4.95, 7.62, 2.74), "void")
    g.box(wi, "weight_lug_base", (-5.38, 7.06, 2.38), (-4.82, 7.38, 2.84), "rust_dk")

    # 砣身上层
    g.box(wi, "weight_upper_core", (-5.43, 6.68, 2.34), (-4.77, 7.16, 2.90), "iron")
    g.box(wi, "weight_upper_f", (-5.42, 6.70, 2.22), (-4.78, 7.15, 2.48), "rust")
    g.box(wi, "weight_upper_b", (-5.42, 6.70, 2.76), (-4.78, 7.15, 3.02), "rust_dk")
    g.box(wi, "weight_upper_l", (-5.60, 6.72, 2.33), (-5.32, 7.17, 2.91), "rust")
    g.box(wi, "weight_upper_r", (-4.88, 6.65, 2.33), (-4.60, 7.17, 2.91), "iron")

    # 砣身中层
    g.box(wi, "weight_mid_core", (-5.62, 5.96, 2.16), (-4.58, 6.76, 3.08), "iron")
    g.box(wi, "weight_mid_rim_t", (-5.67, 6.53, 2.13), (-4.53, 6.87, 3.11), "rust")
    g.box(wi, "weight_mid_rim_b", (-5.67, 5.93, 2.13), (-4.53, 6.27, 3.11), "rust_dk")
    # 八角切棱
    g.box(wi, "weight_mid_nw", (-5.66, 6.06, 2.18), (-5.30, 6.74, 2.50), "rust_dk")
    g.box(wi, "weight_mid_ne", (-4.90, 6.06, 2.18), (-4.62, 6.74, 2.50), "rust")
    g.box(wi, "weight_mid_sw", (-5.66, 6.06, 2.74), (-5.30, 6.74, 3.06), "rust")
    g.box(wi, "weight_mid_se", (-4.90, 6.06, 2.74), (-4.56, 6.74, 3.06), "rust_dk")

    # 砣底垫块
    g.box(wi, "weight_bot_base", (-5.50, 5.65, 2.28), (-4.70, 6.12, 2.96), "rust_dk")
    g.box(wi, "weight_bot_pad", (-5.38, 5.38, 2.38), (-4.82, 5.78, 2.86), "iron")


def part_tail_tassel(g: Gear) -> None:
    """尾端骨珠配重与麻绳流苏。"""
    tt = "tail_tassel"
    g.box(tt, "tassel_neck_cord", (-6.92, 8.08, 2.65), (-6.48, 8.60, 2.97), "cord_dk")
    g.box(tt, "bead1_main", (-7.02, 7.55, 2.58), (-6.38, 8.28, 3.04), "bone")
    g.box(tt, "bead1_ridge_t", (-6.96, 8.10, 2.61), (-6.44, 8.38, 3.01), "bone_lt")
    g.box(tt, "bead1_ridge_b", (-7.00, 7.45, 2.66), (-6.41, 7.75, 3.03), "bone_dk")
    g.box(tt, "bead_knot_mid", (-6.92, 7.28, 2.65), (-6.48, 7.68, 2.97), "cord_dk")
    g.box(tt, "bead2_main", (-6.96, 6.85, 2.61), (-6.44, 7.48, 3.01), "bone_stain")

    # 散开的流苏绳穗 (3 股)
    g.box(tt, "fringe_strand_1", (-6.98, 6.15, 2.58), (-6.65, 7.08, 2.90), "cord")
    g.box(tt, "fringe_strand_2", (-6.75, 5.92, 2.68), (-6.42, 7.07, 3.00), "cord_dk")
    g.box(tt, "fringe_strand_3", (-6.92, 6.28, 2.70), (-6.55, 7.05, 3.05), "cord")


def part_extra_details(g: Gear) -> None:
    """骨脊加固、防滑箍与十字骨托细节。"""
    bm = "beam_main"
    # 秤杆骨脊加固骨突 (dz >= 0.28)
    g.box(bm, "beam_crest_spine_1", (-4.05, 9.40, 2.42), (-3.69, 9.70, 2.74), "bone_lt")
    g.box(bm, "beam_crest_spine_2", (-4.85, 9.27, 2.48), (-4.49, 9.60, 2.75), "bone_lt")
    g.box(bm, "beam_crest_spine_3", (-5.55, 9.15, 2.48), (-5.19, 9.46, 2.74), "bone_lt")

    # 秤杆腹底磨损骨托 (dz >= 0.28)
    g.box(bm, "beam_belly_pad_1", (-4.05, 8.60, 2.39), (-3.69, 8.90, 2.74), "bone_dk")
    g.box(bm, "beam_belly_pad_2", (-4.85, 8.54, 2.45), (-4.49, 8.80, 2.75), "bone_dk")
    g.box(bm, "beam_belly_pad_3", (-5.55, 8.35, 2.44), (-5.19, 8.66, 2.73), "bone_dk")

    # 秤盘边缘挂吊骨牙与麻绳结
    pp = "pan_plate"
    g.box(pp, "pan_trinket_bone1", (-2.88, 5.78, 2.42), (-2.55, 6.25, 2.75), "bone_stain")
    g.box(pp, "pan_trinket_bone2", (-2.18, 5.78, 2.42), (-1.85, 6.25, 2.75), "bone_stain")
    g.box(pp, "pan_rim_stitch_1", (-2.85, 6.77, 2.72), (-2.48, 7.12, 3.05), "stitch")
    g.box(pp, "pan_rim_stitch_2", (-2.22, 6.85, 2.72), (-1.85, 7.12, 3.05), "stitch")

    # 秤砣防滑横箍与铁钉
    wi = "weight_iron"
    g.box(wi, "weight_flaw_boss_1", (-5.48, 6.30, 2.98), (-5.15, 6.62, 3.30), "rust")
    g.box(wi, "weight_flaw_boss_2", (-5.08, 6.30, 2.98), (-4.75, 6.62, 3.30), "rust_dk")
    g.box(wi, "weight_flaw_boss_3", (-5.48, 6.30, 1.92), (-5.15, 6.62, 2.25), "rust_dk")
    g.box(wi, "weight_flaw_boss_4", (-5.08, 6.30, 1.92), (-4.75, 6.62, 2.25), "rust")

    # 尾流苏上的木珠与线结
    tt = "tail_tassel"
    g.box(tt, "tassel_wood_bead", (-6.85, 6.50, 2.65), (-6.52, 6.85, 2.96), "cord_dk")

    # 附加细节 1：腰带外侧双层皮扣
    b = "belt_strap"
    g.pair(b, "belt_sub_clasp", (2.05, 12.60, 2.72), (2.42, 13.20, 3.02), "hide_dk")
    g.pair(b, "belt_sub_ring", (3.05, 12.60, 2.72), (3.42, 13.20, 3.02), "rust")

    # 附加细节 2：秤杆尾段加强骨环
    bt = "beam_tail"
    g.box(bt, "tail_ring_bone_a", (-6.75, 8.37, 2.55), (-6.38, 8.90, 3.05), "bone_lt")
    g.box(bt, "tail_ring_bone_b", (-6.38, 8.35, 2.55), (-6.05, 8.88, 3.05), "bone")

    # 附加细节 3：秤盘底座星纹十字骨托
    g.box(pp, "pan_bot_cross_x", (-3.15, 6.08, 2.48), (-1.55, 6.28, 2.79), "bone_dk")
    g.box(pp, "pan_bot_cross_z", (-2.52, 6.03, 2.05), (-2.18, 6.31, 3.25), "bone_dk")


PART_FUNCS = {
    "belt": part_belt,
    "mount": part_mount,
    "ring": part_ring,
    "pivot": part_pivot,
    "beam": part_beam,
    "marks": part_marks,
    "beam_head": part_beam_head,
    "beam_tail": part_beam_tail,
    "pan_cords": part_pan_cords,
    "pan_plate": part_pan_plate,
    "weight_loop": part_weight_loop,
    "weight_iron": part_weight_iron,
    "tail_tassel": part_tail_tassel,
    "extra_details": part_extra_details,
}

SPEC = GearSpec(
    gear_id="gu_cheng",
    model_name="GuCheng",
    title="骨秤 / gu_cheng",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：左后腰（x∈[-5.5, -3.2], y∈[6.0, 13.8], z∈[2.08, 3.8]）",
    worn_focus=((-4.0, 9.5, 2.5), 18.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
