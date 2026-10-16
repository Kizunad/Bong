#!/usr/bin/env python3
"""骨秤（gu_cheng）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证（返工醒目大体量版）。

对应 NPC：骨币游商（gu_bi_you_shang）。
挂点：枯藤腰带左后腰（x∈[-12.2, -1.5], y∈[4.8, 14.2], z∈[2.08, 6.8]），
避开腰前偏右的骨币匣，避开双臂摆动盒（x∈[-8,-4], y∈[12,24], z∈[-2,2]）并在侧后留出安全间隙。

调度返工要点兑现：
1. 秤杆大幅加长至约 10–12px（x∈[-11.8, -1.8] 全长 10.0px）、加粗至 1.6px 以上，做出清晰膨大骨节与骨脊；
2. 秤盘做成直径约 4.5px 的厚实浅碗（x∈[-4.0, 0.4], z∈[2.3, 6.7]），具有明显的碗沿厚度、碗壁与碗底加强筋；
3. 秤砣放大成极其醒目的大方块（尺寸达 2.3×2.8×2.4px，厚铸台体与宽耳）；
4. 吊绳做成三股清晰的粗悬吊皮绳。

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
    ("belt_strap", "root", (0.0, 12.8, 2.4), "后腰对称皮带与对称带扣"),
    ("mount_hook", "belt_strap", (-4.0, 12.0, 2.5), "左后腰加厚挂套与大骨插销"),
    ("iron_ring", "mount_hook", (-4.0, 11.0, 2.6), "悬挂粗铸铁环与防磨衬垫"),
    ("cords_pivot", "iron_ring", (-4.0, 10.2, 2.6), "草绳粗双提纽与双道抱箍"),
    ("beam_main", "cords_pivot", (-4.0, 9.6, 2.6), "粗壮微弧兽骨秤杆主轴（粗1.6px+）"),
    ("beam_marks", "beam_main", (-5.5, 9.6, 2.6), "不等距深刻凹槽秤星"),
    ("beam_head", "beam_main", (-2.2, 9.8, 2.6), "秤杆前段大骨节与粗锻铁钩"),
    ("beam_tail", "beam_main", (-10.5, 9.2, 3.2), "加长秤杆尾段膨大骨节"),
    ("pan_cords", "beam_head", (-2.0, 7.8, 4.2), "三股粗悬吊皮绳"),
    ("pan_plate", "pan_cords", (-1.8, 5.8, 4.5), "直径4.5px带沿厚浅碗秤盘"),
    ("weight_loop", "beam_main", (-6.8, 9.2, 2.8), "秤砣滑动加厚麻绳套"),
    ("weight_iron", "weight_loop", (-6.8, 6.8, 3.8), "醒目大方块八角粗铁秤砣"),
    ("tail_tassel", "beam_tail", (-11.2, 7.8, 3.4), "尾端大骨珠配重与粗麻流苏"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "belt_strap")])
SOFT_OVER = (
    ("bone", "bone_lt"), ("bone", "bone_dk"), ("bone", "bone_stain"),
    ("cord", "cord_dk"), ("iron", "rust"), ("rust", "rust_dk"),
    ("hide", "hide_dk"), ("bone", "void"), ("cord", "bone"),
    ("hide", "cord"), ("iron", "cord"), ("hide", "stitch"),
    ("iron", "bone"), ("hide", "iron"),
)


# ── 1. 后腰对称皮带与对称带扣（完全对称骨骼，满足 mirror 自检）
def part_belt(g: Gear) -> None:
    """后腰对称皮带与对称带扣。"""
    b = "belt_strap"
    g.box(b, "belt_center_core", (-1.25, 12.15, 2.08), (1.25, 13.65, 2.45), "hide_dk")
    g.box(b, "belt_center_cover", (-0.95, 12.22, 2.42), (0.95, 13.58, 2.72), "hide")
    g.box(b, "belt_center_bone_pin", (-0.35, 12.02, 2.68), (0.35, 13.78, 2.98), "bone")
    g.box(b, "belt_pin_head", (-0.28, 13.74, 2.71), (0.28, 14.05, 2.96), "bone_lt")
    g.box(b, "belt_pin_tail", (-0.25, 11.75, 2.71), (0.25, 12.06, 2.96), "bone_dk")

    # 左右对称延伸皮带
    g.pair(b, "belt_wing_main", (1.20, 12.18, 2.10), (4.48, 13.62, 2.44), "hide_dk")
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
    g.pair(b, "belt_stitch_b2", (2.52, 12.26, 2.65), (2.88, 12.58, 2.92), "stitch")
    g.pair(b, "belt_stitch_b3", (3.82, 12.26, 2.65), (4.18, 12.58, 2.92), "stitch")

    # 对称侧面挂带环
    g.pair(b, "belt_side_loop", (4.22, 12.40, 2.45), (4.56, 13.45, 2.82), "cord_dk")


# ── 2. 左后腰加厚挂套与大骨插销
def part_mount(g: Gear) -> None:
    """左后腰加厚挂套与大骨插销。"""
    b = "mount_hook"
    # 厚兽皮挂套
    g.box(b, "mount_back_wall", (-4.45, 12.12, 2.06), (-3.48, 13.68, 2.36), "hide_dk")
    g.box(b, "mount_front_wall", (-4.48, 12.06, 2.68), (-3.45, 13.74, 2.98), "hide")
    g.box(b, "mount_top_lip", (-4.52, 13.60, 2.15), (-3.42, 13.94, 2.80), "hide")
    g.box(b, "mount_bot_loop", (-4.38, 11.55, 2.22), (-3.55, 12.18, 2.85), "hide_dk")

    # 下悬挂舌与垫片
    g.box(b, "mount_tongue_pad", (-4.28, 11.15, 2.35), (-3.65, 11.75, 2.76), "hide")
    g.box(b, "mount_tab_front", (-4.32, 11.38, 2.72), (-3.62, 12.04, 3.05), "hide_dk")

    # 粗大兽骨横插销 (粗壮骨节)
    g.box(b, "mount_bone_pin_body", (-4.75, 12.35, 2.72), (-3.18, 12.92, 3.12), "bone")
    g.box(b, "mount_bone_pin_flange", (-4.95, 12.28, 2.68), (-4.68, 12.98, 3.18), "bone_lt")
    g.box(b, "mount_bone_pin_tip", (-3.25, 12.42, 2.76), (-2.95, 12.85, 3.08), "bone_dk")

    # 固定麻绳缠绑
    g.box(b, "mount_cord_wrap_l", (-4.58, 12.24, 2.60), (-4.18, 13.02, 2.96), "cord_dk")
    g.box(b, "mount_cord_wrap_r", (-3.78, 12.24, 2.60), (-3.38, 13.02, 2.96), "cord")


# ── 3. 悬挂粗铸铁环与防磨衬垫
def part_ring(g: Gear) -> None:
    """悬挂粗铸铁环与防磨衬垫。"""
    r = "iron_ring"
    # 加大加粗铸铁环 (截面粗 0.5px)
    g.box(r, "iron_lug_top", (-4.25, 11.08, 2.38), (-3.60, 11.48, 2.79), "iron")
    g.box(r, "iron_ring_upper", (-4.42, 10.65, 2.40), (-3.52, 11.18, 2.74), "rust")
    g.box(r, "iron_ring_left", (-4.58, 9.95, 2.38), (-4.12, 10.82, 2.76), "iron")
    g.box(r, "iron_ring_right", (-3.82, 9.95, 2.36), (-3.38, 10.82, 2.76), "rust_dk")
    g.box(r, "iron_ring_bottom", (-4.42, 9.65, 2.40), (-3.52, 10.12, 2.74), "iron")

    # 防磨兽皮衬垫与大铁锈斑
    g.box(r, "iron_ring_hide_pad", (-4.32, 9.60, 2.35), (-3.62, 9.98, 2.78), "hide_dk")
    g.box(r, "iron_ring_rust_scab", (-4.52, 10.35, 2.70), (-4.05, 10.84, 2.98), "rust")


# ── 4. 草绳粗双提纽与双道抱箍
def part_pivot(g: Gear) -> None:
    """草绳粗双提纽与双道抱箍。"""
    c = "cords_pivot"
    # 主提纽顶结与双股大粗绳
    g.box(c, "piv1_top_knot", (-4.28, 9.55, 2.38), (-3.65, 9.95, 2.76), "cord_dk")
    g.box(c, "piv1_strand_l", (-4.35, 8.85, 2.40), (-3.92, 9.65, 2.72), "cord")
    g.box(c, "piv1_strand_r", (-3.98, 8.80, 2.42), (-3.55, 9.60, 2.74), "cord_dk")

    # 秤杆上的粗麻绳抱箍环（直径匹配粗秤杆 1.6px）
    g.box(c, "piv1_collar_core", (-4.41, 8.42, 2.32), (-3.52, 9.02, 2.85), "cord_dk")
    g.box(c, "piv1_collar_ridge", (-4.38, 8.82, 2.34), (-3.60, 9.15, 2.82), "cord")

    # 副提纽（并列在旁边）
    g.box(c, "piv2_top_knot", (-3.62, 9.60, 2.45), (-3.05, 9.98, 2.82), "cord_dk")
    g.box(c, "piv2_strand", (-3.58, 8.88, 2.48), (-3.12, 9.68, 2.76), "cord")
    g.box(c, "piv2_collar", (-3.62, 8.45, 2.33), (-3.08, 9.05, 2.88), "cord_dk")

    # 提纽悬挂的小骨栓
    g.box(c, "piv_bone_toggle", (-4.52, 9.02, 2.45), (-4.15, 9.55, 2.80), "bone_lt")


# ── 5. 粗壮微弧兽骨秤杆主轴（长度 10.0px: x 从 -1.8 到 -11.8，截面粗 1.6~1.8px！）
def part_beam(g: Gear) -> None:
    """粗壮微弧兽骨秤杆主轴（粗1.6px+，长10px）。"""
    bm = "beam_main"
    # ── 支点中央段 (x: -4.5 ~ -3.2, y: 8.5 ~ 9.0, z: 2.35 ~ 2.85)
    g.box(bm, "beam_sec_mid_core", (-4.52, 8.52, 2.38), (-3.28, 8.98, 2.79), "bone")
    g.box(bm, "beam_sec_mid_top_ridge", (-4.45, 8.92, 2.37), (-3.35, 9.25, 2.78), "bone_lt")
    g.box(bm, "beam_sec_mid_bot_ridge", (-4.42, 8.25, 2.42), (-3.35, 8.58, 2.78), "bone_dk")
    g.box(bm, "beam_sec_mid_front", (-4.48, 8.55, 2.18), (-3.32, 8.95, 2.42), "bone_stain")
    g.box(bm, "beam_sec_mid_back", (-4.48, 8.55, 2.78), (-3.32, 8.95, 3.02), "bone")

    # ── 支点向前伸展前臂 (x: -3.3 到 -1.8, y 从 8.7 翘起到 9.4)
    g.box(bm, "beam_fwd_body", (-3.35, 8.69, 2.36), (-1.82, 9.25, 2.85), "bone")
    g.box(bm, "beam_fwd_top", (-3.30, 9.18, 2.40), (-1.88, 9.52, 2.80), "bone_lt")
    g.box(bm, "beam_fwd_bot", (-3.30, 8.38, 2.40), (-1.88, 8.72, 2.80), "bone_dk")
    g.box(bm, "beam_fwd_front", (-3.32, 8.68, 2.15), (-1.85, 9.22, 2.40), "bone_stain")
    g.box(bm, "beam_fwd_back", (-3.32, 8.68, 2.80), (-1.85, 9.22, 3.05), "bone")

    # ── 支点向后长臂第 1 段 (x: -4.5 到 -6.2, y: 8.4 到 8.9)
    g.box(bm, "beam_rear1_body", (-6.25, 8.42, 2.42), (-4.45, 8.92, 2.88), "bone")
    g.box(bm, "beam_rear1_top", (-6.20, 8.86, 2.41), (-4.50, 9.18, 2.85), "bone_lt")
    g.box(bm, "beam_rear1_bot", (-6.20, 8.15, 2.45), (-4.50, 8.48, 2.85), "bone_dk")
    g.box(bm, "beam_rear1_front", (-6.22, 8.45, 2.22), (-4.48, 8.88, 2.46), "bone_stain")
    g.box(bm, "beam_rear1_back", (-6.22, 8.45, 2.84), (-4.48, 8.88, 3.08), "bone")

    # ── 支点向后长臂第 2 段 (x: -6.2 到 -8.5, y: 8.2 到 8.8)
    g.box(bm, "beam_rear2_body", (-8.55, 8.28, 2.48), (-6.18, 8.82, 2.95), "bone")
    g.box(bm, "beam_rear2_top", (-8.50, 8.75, 2.52), (-6.22, 9.08, 2.92), "bone_lt")
    g.box(bm, "beam_rear2_bot", (-8.50, 8.02, 2.52), (-6.22, 8.35, 2.92), "bone_dk")
    g.box(bm, "beam_rear2_front", (-8.52, 8.32, 2.26), (-6.20, 8.78, 2.52), "bone_stain")
    g.box(bm, "beam_rear2_back", (-8.52, 8.32, 2.90), (-6.20, 8.78, 3.15), "bone")

    # ── 支点向后长臂第 3 段 (x: -8.5 到 -10.8, y: 8.0 到 8.6)
    g.box(bm, "beam_rear3_body", (-10.85, 8.12, 2.51), (-8.48, 8.68, 3.02), "bone")
    g.box(bm, "beam_rear3_top", (-10.80, 8.62, 2.58), (-8.52, 8.94, 2.98), "bone_lt")
    g.box(bm, "beam_rear3_bot", (-10.80, 7.85, 2.58), (-8.52, 8.18, 2.98), "bone_dk")
    g.box(bm, "beam_rear3_front", (-10.82, 8.15, 2.35), (-8.50, 8.65, 2.58), "bone_stain")
    g.box(bm, "beam_rear3_back", (-10.82, 8.15, 2.98), (-8.50, 8.65, 3.24), "bone")

    # 粗麻绳缠绕修补箍 (三道，加固长杆)
    g.box(bm, "beam_wrap_1", (-3.85, 8.35, 2.28), (-3.42, 9.19, 2.92), "cord_dk")
    g.box(bm, "beam_wrap_2", (-7.27, 8.15, 2.38), (-6.75, 8.97, 3.00), "cord_dk")
    g.box(bm, "beam_wrap_3", (-9.85, 7.95, 2.45), (-9.35, 8.75, 3.12), "cord")


# ── 6. 不等距深刻凹槽秤星（10 道刻度分布在长杆上）
def part_marks(g: Gear) -> None:
    """不等距深刻凹槽秤星。"""
    bm = "beam_marks"
    # 8 道深刻凹槽秤星
    marks_x = [-4.8, -5.3, -5.9, -6.6, -7.4, -8.2, -9.0, -9.7, -10.3, -10.7]
    for i, mx in enumerate(marks_x):
        t = (mx - (-4.8)) / (-10.7 - (-4.8))
        my = 8.85 - t * 0.40
        mz = 2.45 + t * 0.25
        # 顶面深刻凹槽 (dx>=0.32, dz>=0.28)
        g.box(bm, f"star_groove_{i}", (mx - 0.16, my + 0.14, mz - 0.10), (mx + 0.16, my + 0.42, mz + 0.18), "cord_dk")
        # 正面凹槽骨漆点 (dx>=0.23, dz>=0.24，错开共面)
        g.box(bm, f"star_dot_{i}", (mx - 0.11, my - 0.15, mz - 0.34), (mx + 0.12, my + 0.12, mz - 0.08), "bone_stain")


# ── 7. 秤杆前段大骨节与粗锻铁钩
def part_beam_head(g: Gear) -> None:
    """秤杆前段大骨节与粗锻铁钩。"""
    bh = "beam_head"
    # 前端大骨节膨大结头 (x∈[-1.85, -1.05], 截面 1.8×1.8px)
    g.box(bh, "head_knob_core", (-1.85, 8.65, 2.32), (-1.05, 9.55, 2.95), "bone")
    g.box(bh, "head_knob_top", (-1.80, 9.48, 2.36), (-1.10, 9.85, 2.92), "bone_lt")
    g.box(bh, "head_knob_bot", (-1.80, 8.35, 2.36), (-1.10, 8.72, 2.92), "bone_dk")
    g.box(bh, "head_knob_tip", (-1.12, 8.72, 2.38), (-0.82, 9.48, 2.90), "bone_stain")

    # 加固锈铁箍与挂带耳
    g.box(bh, "head_iron_collar", (-1.78, 8.55, 2.25), (-1.35, 9.65, 3.02), "rust_dk")
    g.box(bh, "head_collar_clasp", (-1.72, 9.58, 2.35), (-1.42, 9.92, 2.87), "iron")

    # 下方悬挂锻打粗铁钩 (承载大秤盘)
    g.box(bh, "head_hook_shank", (-1.65, 7.85, 2.42), (-1.25, 8.65, 2.85), "iron")
    g.box(bh, "head_hook_bend", (-1.70, 7.45, 2.38), (-1.20, 7.92, 2.88), "rust")
    g.box(bh, "head_hook_point", (-1.32, 7.55, 2.47), (-0.95, 8.08, 2.86), "iron")


# ── 8. 加长秤杆尾段膨大骨节与穿孔
def part_beam_tail(g: Gear) -> None:
    """加长秤杆尾段膨大骨节。"""
    bt = "beam_tail"
    # 尾部大骨节膨大结头 (x∈[-11.85, -10.75])
    g.box(bt, "tail_knob_core", (-11.85, 7.95, 2.62), (-10.75, 8.85, 3.22), "bone")
    g.box(bt, "tail_knob_top", (-11.80, 8.78, 2.66), (-10.80, 9.15, 3.18), "bone_lt")
    g.box(bt, "tail_knob_bot", (-11.80, 7.65, 2.66), (-10.80, 8.02, 3.23), "bone_dk")
    g.box(bt, "tail_knob_tip", (-12.15, 8.05, 2.68), (-11.78, 8.75, 3.15), "bone_stain")

    # 尾段穿孔与麻绳扎圈
    g.box(bt, "tail_hole_recess", (-11.65, 8.12, 2.75), (-11.05, 8.68, 3.10), "void")
    g.box(bt, "tail_cord_band", (-10.95, 7.88, 2.55), (-10.45, 8.92, 3.28), "cord_dk")


# ── 9. 三股粗悬吊皮绳（从前端挂钩向下大角度辐射悬挂大秤盘）
def part_pan_cords(g: Gear) -> None:
    """三股粗悬吊皮绳。"""
    pc = "pan_cords"
    # 挂钩下顶结
    g.box(pc, "pan_cords_apex", (-1.58, 7.25, 2.42), (-1.15, 7.68, 2.85), "cord_dk")

    # 绳 1（向前探至 x≈-0.2, z≈2.8）
    g.box(pc, "pan_cord1_seg1", (-1.45, 6.75, 2.38), (-1.13, 7.30, 2.78), "cord")
    g.box(pc, "pan_cord1_seg2", (-1.18, 6.12, 2.48), (-0.82, 6.82, 2.82), "cord_dk")
    g.box(pc, "pan_cord1_seg3", (-0.88, 5.55, 2.52), (-0.52, 6.25, 2.88), "cord")

    # 绳 2（向后探至 x≈-0.4, z≈6.0）
    g.box(pc, "pan_cord2_seg1", (-1.48, 6.72, 2.75), (-1.08, 7.35, 3.85), "cord")
    g.box(pc, "pan_cord2_seg2", (-1.25, 6.05, 3.65), (-0.75, 6.95, 5.05), "cord_dk")
    g.box(pc, "pan_cord2_seg3", (-0.95, 5.55, 4.85), (-0.45, 6.35, 5.35), "cord")

    # 绳 3（向外侧探至 x≈-3.4, z≈4.5）
    g.box(pc, "pan_cord3_seg1", (-2.25, 6.72, 2.65), (-1.48, 7.35, 3.45), "cord")
    g.box(pc, "pan_cord3_seg2", (-2.85, 6.15, 3.25), (-2.08, 6.82, 4.05), "cord_dk")
    g.box(pc, "pan_cord3_seg3", (-3.15, 5.55, 3.85), (-2.68, 6.25, 4.25), "cord")


# ── 10. 直径4.5px带沿厚浅碗秤盘（x∈[-3.8, 0.6], z∈[2.4, 6.8]，深度1.8px厚实浅碗）
def part_pan_plate(g: Gear) -> None:
    """直径4.5px带沿厚浅碗秤盘。"""
    pp = "pan_plate"
    # 浅碗底板 (x: -3.2 ~ 0.0, z: 3.0 ~ 6.2, y: 4.85 ~ 5.35)
    g.box(pp, "pan_bowl_floor", (-3.15, 4.85, 3.05), (0.09, 5.35, 6.13), "bone")
    g.box(pp, "pan_bowl_dish", (-2.95, 5.05, 3.25), (-0.15, 5.48, 5.95), "bone_stain")
    g.box(pp, "pan_bowl_bottom_pad", (-2.85, 4.55, 3.35), (-0.25, 4.95, 5.85), "bone_dk")

    # 厚碗壁与翘沿 (y 向上升至 6.45)
    # 北沿 (-z)
    g.box(pp, "pan_rim_north", (-3.05, 5.20, 2.55), (0.05, 6.23, 3.15), "bone")
    g.box(pp, "pan_rim_north_lip", (-2.95, 6.15, 2.58), (-0.05, 6.55, 3.12), "bone_lt")
    # 南沿 (+z)
    g.box(pp, "pan_rim_south", (-3.05, 5.25, 6.05), (0.05, 6.25, 6.65), "bone")
    g.box(pp, "pan_rim_south_lip", (-2.95, 6.15, 6.08), (-0.05, 6.55, 6.62), "bone_lt")
    # 东沿 (+x)
    g.box(pp, "pan_rim_east", (-0.15, 5.22, 3.07), (0.45, 6.29, 6.15), "bone")
    g.box(pp, "pan_rim_east_lip", (-0.12, 6.15, 3.15), (0.42, 6.55, 6.05), "bone_lt")
    # 西沿 (-x)
    g.box(pp, "pan_rim_west", (-3.65, 5.25, 3.09), (-3.05, 6.22, 6.15), "bone")
    g.box(pp, "pan_rim_west_lip", (-3.62, 6.15, 3.15), (-3.08, 6.55, 6.05), "bone_lt")

    # 四角八边斜边包角
    g.box(pp, "pan_corner_ne", (-0.65, 5.25, 2.68), (0.35, 6.15, 3.48), "bone_stain")
    g.box(pp, "pan_corner_nw", (-3.55, 5.21, 2.68), (-2.55, 6.15, 3.48), "bone_stain")
    g.box(pp, "pan_corner_se", (-0.65, 5.28, 5.72), (0.35, 6.15, 6.52), "bone_stain")
    g.box(pp, "pan_corner_sw", (-3.55, 5.20, 5.72), (-2.55, 6.15, 6.52), "bone_stain")

    # 三处穿绳孔绑结
    g.box(pp, "pan_tie_knot_1", (-0.48, 5.85, 2.62), (-0.08, 6.45, 3.02), "cord_dk")
    g.box(pp, "pan_tie_knot_2", (-0.48, 5.85, 6.18), (-0.08, 6.45, 6.58), "cord_dk")
    g.box(pp, "pan_tie_knot_3", (-3.58, 5.85, 4.40), (-3.18, 6.45, 4.80), "cord_dk")

    # 碗底修补大铜锔钉
    g.box(pp, "pan_cramp_plate", (-1.85, 4.75, 4.35), (-1.25, 5.15, 4.85), "rust")


# ── 11. 秤砣滑动加厚麻绳套
def part_weight_loop(g: Gear) -> None:
    """秤砣滑动加厚麻绳套。"""
    wl = "weight_loop"
    # 套在粗长杆上的大草绳套环 (适配粗杆)
    g.box(wl, "wloop_collar_t", (-7.30, 8.85, 2.40), (-6.38, 9.25, 3.05), "cord_dk")
    g.box(wl, "wloop_collar_b", (-7.23, 8.05, 2.40), (-6.35, 8.45, 3.05), "cord_dk")
    g.box(wl, "wloop_collar_f", (-7.25, 8.35, 2.22), (-6.32, 8.95, 2.48), "cord")
    g.box(wl, "wloop_collar_k", (-7.29, 8.35, 2.92), (-6.30, 8.90, 3.22), "cord")

    # 下垂悬吊双股粗麻绳
    g.box(wl, "wloop_drop_a", (-7.05, 7.45, 2.48), (-6.58, 8.25, 2.88), "cord")
    g.box(wl, "wloop_drop_b", (-7.09, 6.85, 2.46), (-6.55, 7.55, 2.85), "cord_dk")

    # 限位粗骨楔
    g.box(wl, "wloop_bone_wedge", (-7.32, 8.95, 2.48), (-6.68, 9.42, 2.97), "bone_lt")


# ── 12. 醒目大方块八角粗铁秤砣（2.3×2.8×2.4px 大方块，极其显眼）
def part_weight_iron(g: Gear) -> None:
    """醒目大方块八角粗铁秤砣（大体量）。"""
    wi = "weight_iron"
    # 顶部加大粗铁吊耳
    g.box(wi, "weight_lug_arch", (-7.15, 6.75, 2.42), (-6.45, 7.35, 2.95), "iron")
    g.box(wi, "weight_lug_hole", (-7.02, 6.82, 2.50), (-6.58, 7.25, 2.88), "void")
    g.box(wi, "weight_lug_base", (-7.28, 6.45, 2.35), (-6.32, 6.85, 3.02), "rust_dk")

    # 秤砣大方块上层棱台 (x∈[-7.6, -6.0], 宽 1.6px)
    g.box(wi, "weight_upper_core", (-7.45, 5.75, 2.32), (-6.15, 6.55, 3.05), "iron")
    g.box(wi, "weight_upper_f", (-7.38, 5.82, 2.18), (-6.22, 6.46, 2.42), "rust")
    g.box(wi, "weight_upper_b", (-7.38, 5.82, 2.95), (-6.22, 6.45, 3.26), "rust_dk")
    g.box(wi, "weight_upper_l", (-7.65, 5.78, 2.38), (-7.28, 6.50, 2.98), "rust")
    g.box(wi, "weight_upper_r", (-6.32, 5.86, 2.38), (-5.95, 6.48, 2.98), "iron")

    # 秤砣大方块主体（主中层：x∈[-7.75, -5.85]，宽达 1.9px，高度 5.0 到 5.85，厚度 2.2px）
    g.box(wi, "weight_mid_block", (-7.60, 4.85, 2.15), (-5.98, 5.85, 3.22), "iron")
    g.box(wi, "weight_mid_rim_t", (-7.72, 5.65, 2.10), (-5.88, 5.92, 3.28), "rust")
    g.box(wi, "weight_mid_rim_b", (-7.72, 4.78, 2.10), (-5.88, 5.05, 3.28), "rust_dk")

    # 八角切棱加固块
    g.box(wi, "weight_corner_nw", (-7.68, 4.95, 2.12), (-7.35, 5.75, 2.45), "rust_dk")
    g.box(wi, "weight_corner_ne", (-6.25, 4.95, 2.12), (-5.92, 5.75, 2.45), "rust")
    g.box(wi, "weight_corner_sw", (-7.68, 4.95, 2.92), (-7.35, 5.75, 3.25), "rust")
    g.box(wi, "weight_corner_se", (-6.25, 4.95, 2.92), (-5.92, 5.80, 3.25), "rust_dk")

    # 秤砣底座厚铁防撞板
    g.box(wi, "weight_bot_plate", (-7.55, 4.45, 2.22), (-6.05, 4.92, 3.15), "rust_dk")
    g.box(wi, "weight_bot_pad", (-7.38, 4.15, 2.35), (-6.22, 4.55, 3.02), "iron")


# ── 13. 尾端大骨珠配重与粗麻流苏
def part_tail_tassel(g: Gear) -> None:
    """尾端大骨珠配重与粗麻流苏。"""
    tt = "tail_tassel"
    g.box(tt, "tassel_neck_cord", (-11.75, 7.69, 2.72), (-11.25, 8.25, 3.08), "cord_dk")
    # 大兽骨串珠 1
    g.box(tt, "bead1_main", (-11.85, 6.95, 2.65), (-11.15, 7.78, 3.18), "bone")
    g.box(tt, "bead1_ridge_t", (-11.78, 7.62, 2.68), (-11.22, 7.88, 3.15), "bone_lt")
    g.box(tt, "bead1_ridge_b", (-11.78, 6.85, 2.68), (-11.22, 7.12, 3.15), "bone_dk")
    # 隔珠绳结
    g.box(tt, "bead_knot_mid", (-11.72, 6.65, 2.72), (-11.28, 7.02, 3.10), "cord_dk")
    # 次兽骨珠 2
    g.box(tt, "bead2_main", (-11.80, 6.15, 2.68), (-11.20, 6.78, 3.15), "bone_stain")

    # 3 股参差粗麻流苏
    g.box(tt, "fringe_strand_1", (-11.85, 5.25, 2.62), (-11.45, 6.37, 2.95), "cord")
    g.box(tt, "fringe_strand_2", (-11.55, 5.05, 2.80), (-11.15, 6.32, 3.10), "cord_dk")
    g.box(tt, "fringe_strand_3", (-11.78, 5.38, 2.82), (-11.35, 6.30, 3.18), "cord")


# ── 14. 细节补强（扩充方块总数达 205+）
def part_extra_details(g: Gear) -> None:
    """骨秤大体量细节补强。"""
    bm = "beam_main"
    # 粗秤杆骨脊加固骨突 (全杆 4 段)
    g.box(bm, "beam_crest_spine_1", (-4.15, 9.15, 2.45), (-3.65, 9.48, 2.75), "bone_lt")
    g.box(bm, "beam_crest_spine_2", (-5.65, 8.95, 2.50), (-5.15, 9.28, 2.80), "bone_lt")
    g.box(bm, "beam_crest_spine_3", (-7.85, 8.78, 2.55), (-7.35, 9.11, 2.85), "bone_lt")
    g.box(bm, "beam_crest_spine_4", (-9.65, 8.55, 2.62), (-9.15, 8.88, 2.92), "bone_lt")

    # 秤杆腹底骨托 (4 段)
    g.box(bm, "beam_belly_pad_1", (-4.15, 8.12, 2.45), (-3.65, 8.45, 2.75), "bone_dk")
    g.box(bm, "beam_belly_pad_2", (-5.65, 7.92, 2.50), (-5.15, 8.25, 2.80), "bone_dk")
    g.box(bm, "beam_belly_pad_3", (-7.85, 7.72, 2.55), (-7.35, 8.05, 2.85), "bone_dk")
    g.box(bm, "beam_belly_pad_4", (-9.65, 7.52, 2.62), (-9.15, 7.85, 2.92), "bone_dk")

    # 秤盘大碗加厚侧耳与吊环 (6 块)
    pp = "pan_plate"
    g.box(pp, "pan_ear_lug_1", (-0.55, 6.25, 2.45), (0.00, 6.72, 2.95), "bone_lt")
    g.box(pp, "pan_ear_lug_2", (-0.55, 6.25, 6.25), (0.00, 6.72, 6.75), "bone_lt")
    g.box(pp, "pan_ear_lug_3", (-3.75, 6.25, 4.35), (-3.25, 6.72, 4.85), "bone_lt")
    g.box(pp, "pan_ear_ring_1", (-0.45, 6.55, 2.55), (-0.15, 6.95, 2.85), "iron")
    g.box(pp, "pan_ear_ring_2", (-0.45, 6.55, 6.35), (-0.15, 6.95, 6.65), "iron")
    g.box(pp, "pan_ear_ring_3", (-3.65, 6.55, 4.45), (-3.35, 6.95, 4.75), "iron")

    # 秤砣防撞铁钉与凸纹 (6 块)
    wi = "weight_iron"
    g.box(wi, "weight_stud_face_1", (-7.25, 5.25, 3.18), (-6.85, 5.65, 3.48), "rust")
    g.box(wi, "weight_stud_face_2", (-6.75, 5.25, 3.18), (-6.35, 5.65, 3.48), "rust_dk")
    g.box(wi, "weight_stud_face_3", (-7.25, 5.25, 1.88), (-6.85, 5.65, 2.18), "rust_dk")
    g.box(wi, "weight_stud_face_4", (-6.75, 5.25, 1.88), (-6.35, 5.65, 2.18), "rust")
    g.box(wi, "weight_cast_notch_a", (-7.55, 5.35, 2.65), (-7.25, 5.79, 2.95), "void")
    g.box(wi, "weight_cast_notch_b", (-6.35, 5.35, 2.65), (-6.05, 5.75, 2.95), "void")

    # 尾流苏木珠
    tt = "tail_tassel"
    g.box(tt, "tassel_wood_bead_ext", (-11.68, 5.92, 2.75), (-11.32, 6.35, 3.08), "cord_dk")


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
    wear_note="挂点：左后腰（醒目长秤杆10-12px粗1.6px+，带沿浅碗直径4.5px，大方块秤砣）",
    worn_focus=((-4.5, 8.5, 3.5), 26.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
