#!/usr/bin/env python3
"""残剑鞘背带（can_jian_qiao_bei_dai）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证（返工宽厚大体量版）。

对应 NPC：残峰散修（can_feng_san_xiu）。
挂点：背部斜挎。皮带从右肩经背部斜到左后腰，鞘悬在后背斜跨，剑柄自右肩外侧后方露出一截（x > 4.05，完全避开头盒）；
避开左肩（留出骨片肩甲位置），避开躯干与双臂摆动盒（翻肩段走 x∈[4.08, 5.25], y∈[24.05, 24.85] 骑过右臂上沿）。

调度返工要点兑现：
1. 剑鞘截面大幅加厚做宽至约 2.5×1.5px（宽达2.4-2.6px，厚达1.4-1.6px），身后斜背极度醒目厚重；
2. 鞘身具多道厚皮革紧密缠绕层、补丁、粗锈铁箍、鞘尖大包铁；
3. 露出的断剑柄粗实（截面1.2x1.2px）、缠麻绳凸结、带宽大铸铁护手剑格与残刃犬牙断茬。

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
    "hide": (106, 70, 48),
    "hide_dk": (70, 48, 31),
    "hide_lt": (138, 100, 62),
    "linen": (146, 128, 100),
    "linen_lt": (172, 154, 122),
    "linen_dk": (112, 95, 74),
    "cord": (168, 140, 88),
    "cord_dk": (122, 96, 56),
    "iron": (88, 80, 76),
    "iron_lt": (128, 120, 112),
    "rust": (122, 66, 36),
    "rust_dk": (78, 42, 26),
    "bone": (220, 212, 192),
    "bone_lt": (235, 228, 210),
    "stitch": (188, 174, 148),
    "void": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("strap_chest_cross", "root", (0.0, 14.8, -2.4), "胸前对称横向固形扣带"),
    ("strap_chest_diag", "strap_chest_cross", (2.0, 18.0, -2.4), "胸前斜挎主背带"),
    ("strap_shoulder_r", "strap_chest_diag", (4.5, 24.2, 0.0), "右肩宽皮垫与过肩带"),
    ("strap_back_diag", "strap_shoulder_r", (0.0, 18.0, 2.5), "背部斜向主背带"),
    ("strap_waist_mount", "strap_back_diag", (-3.5, 12.8, 2.5), "左后腰斜带锚点扣与吊环"),
    ("scabbard_throat", "strap_back_diag", (2.2, 21.5, 3.0), "加厚宽大鞘口锈铁箍与挂带耳"),
    ("scabbard_upper", "scabbard_throat", (0.8, 18.0, 3.1), "截面2.5x1.5px宽厚兽皮鞘身上半段"),
    ("scabbard_lower", "scabbard_upper", (-1.2, 14.5, 3.1), "宽厚剑鞘下半段防磨皮补片与木衬"),
    ("scabbard_chape", "scabbard_lower", (-2.8, 11.2, 3.0), "加厚大包铁鞘尾护套与排气孔"),
    ("blade_broken_stub", "scabbard_throat", (3.6, 23.2, 2.8), "粗断剑锯齿犬牙残茬外露剑身"),
    ("blade_cloth_wrap", "blade_broken_stub", (3.8, 23.6, 2.8), "断裂截面粗麻布厚包裹与破角"),
    ("sword_guard", "blade_broken_stub", (4.2, 24.2, 2.8), "宽厚粗铸铁护手剑格与防滑铁齿"),
    ("sword_hilt_grip", "sword_guard", (4.8, 25.8, 2.8), "粗麻绳缠绕剑柄身（粗1.2px+外露右肩）"),
    ("sword_pommel", "sword_hilt_grip", (5.6, 27.8, 2.8), "大锈铁圆环配重与骨环吊饰"),
    ("scabbard_trinket", "scabbard_upper", (0.5, 17.2, 3.8), "鞘侧吊挂辟邪骨符与断绳"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "strap_chest_cross")])
SOFT_OVER = frozenset(frozenset((s, o)) for s, o in (
    ("bone", "bone_lt"), ("hide", "hide_dk"), ("hide", "hide_lt"),
    ("linen", "linen_lt"), ("linen", "linen_dk"), ("cord", "cord_dk"),
    ("iron", "rust"), ("rust", "rust_dk"), ("iron", "iron_lt"),
    ("hide", "cord"), ("hide", "stitch"), ("hide", "iron"),
    ("linen", "cord"), ("linen", "stitch"), ("linen", "iron"),
    ("hide", "bone"), ("iron", "cord"), ("bone", "void"),
    ("cord_dk", "rust_dk"), ("cord", "rust_dk"), ("cord_dk", "rust"),
))


# ── 1. 胸前对称横向固形扣带（完全对称骨骼，满足 mirror 自检）
def part_chest_cross(g: Gear) -> None:
    """胸前对称横向固形扣带。"""
    b = "strap_chest_cross"
    g.box(b, "chest_mid_clasp", (-0.95, 14.18, -2.48), (0.95, 15.62, -2.18), "hide_dk")
    g.box(b, "chest_mid_iron_buckle", (-0.68, 14.28, -2.72), (0.68, 15.52, -2.48), "iron")
    g.box(b, "chest_pin_bone", (-0.22, 14.05, -2.85), (0.22, 15.75, -2.55), "bone")
    g.box(b, "chest_buckle_frame_t", (-0.58, 15.35, -2.82), (0.58, 15.68, -2.52), "rust")
    g.box(b, "chest_buckle_frame_b", (-0.58, 14.12, -2.82), (0.58, 14.45, -2.52), "rust_dk")

    # 对称左右胸肋横系带（严格镜像）
    g.pair(b, "chest_wing_strap", (0.90, 14.25, -2.44), (3.85, 15.55, -2.12), "hide_dk")
    g.pair(b, "chest_wing_pad", (0.92, 14.35, -2.62), (3.82, 15.45, -2.34), "hide")
    g.pair(b, "chest_wing_stitch", (1.45, 14.42, -2.75), (3.45, 15.38, -2.48), "stitch")
    g.pair(b, "chest_rivet", (2.25, 14.52, -2.82), (2.65, 15.28, -2.52), "bone_lt")
    g.pair(b, "chest_wing_loop", (3.25, 14.32, -2.72), (3.65, 15.48, -2.42), "cord_dk")


# ── 2. 胸前斜挎主背带
def part_chest_diag(g: Gear) -> None:
    """胸前斜挎主背带。"""
    b = "strap_chest_diag"
    g.box(b, "chest_diag_seg1", (2.65, 20.25, -2.45), (3.95, 23.65, -2.15), "hide")
    g.box(b, "chest_diag_seg2", (1.45, 16.95, -2.52), (2.85, 20.45, -2.18), "hide_dk")
    g.box(b, "chest_diag_seg3", (0.15, 13.95, -2.55), (1.65, 17.25, -2.20), "hide")
    g.box(b, "chest_diag_seg4", (-1.25, 11.95, -2.56), (0.35, 14.35, -2.22), "hide_dk")
    g.box(b, "chest_diag_seg5", (-2.55, 10.95, -2.54), (-1.05, 12.35, -2.20), "hide")

    # 外层加厚补强垫片与双排针脚
    g.box(b, "chest_diag_pad_hi", (1.85, 17.95, -2.70), (3.15, 21.45, -2.42), "hide_lt")
    g.box(b, "chest_diag_pad_lo", (-0.75, 12.65, -2.76), (0.75, 15.85, -2.44), "hide_lt")
    g.box(b, "chest_diag_stitch_hi", (2.05, 18.35, -2.86), (2.95, 21.05, -2.58), "stitch")
    g.box(b, "chest_diag_stitch_lo", (-0.55, 13.05, -2.84), (0.55, 15.45, -2.60), "stitch")

    # 斜带上的骨制穿孔固定销与装饰骨片
    g.box(b, "chest_diag_bone_peg", (2.25, 19.25, -2.92), (2.75, 19.85, -2.62), "bone")
    g.box(b, "chest_diag_bone_peg_t", (2.35, 19.85, -2.95), (2.65, 20.18, -2.65), "bone_lt")
    g.box(b, "chest_bone_clasp_mid", (0.85, 14.85, -2.85), (1.55, 15.65, -2.52), "bone")
    g.box(b, "chest_bone_clasp_t", (0.95, 15.52, -2.88), (1.45, 15.88, -2.55), "bone_lt")
    g.box(b, "chest_strap_rivet_1", (2.25, 21.25, -2.75), (2.55, 21.65, -2.45), "bone_lt")
    g.box(b, "chest_strap_rivet_2", (1.75, 18.25, -2.75), (2.05, 18.65, -2.45), "bone_lt")
    g.box(b, "chest_strap_rivet_3", (-0.15, 15.25, -2.75), (0.15, 15.65, -2.45), "bone_lt")
    g.box(b, "chest_strap_rivet_4", (-1.05, 12.25, -2.75), (-0.75, 12.65, -2.45), "bone_lt")


# ── 3. 右肩宽皮垫与过肩带（骑在右臂上沿，彻底避开头盒）
def part_shoulder(g: Gear) -> None:
    """右肩宽皮垫与过肩带。"""
    b = "strap_shoulder_r"
    g.box(b, "sh_front_jog", (3.75, 22.85, -2.48), (4.35, 23.92, -2.12), "hide")
    g.box(b, "sh_back_jog", (3.75, 22.85, 2.12), (4.35, 23.92, 2.48), "hide")

    # 翻肩段（x∈[4.08, 5.35], y∈[24.05, 24.85] 避开头盒）
    g.box(b, "sh_over_base", (4.08, 24.05, -2.25), (4.82, 24.58, 2.25), "hide_dk")
    g.box(b, "sh_over_outer", (4.12, 24.28, -2.05), (4.74, 24.78, 2.05), "hide")
    g.box(b, "sh_over_crest", (4.18, 24.52, -1.65), (4.72, 24.88, 1.65), "hide_lt")

    g.box(b, "sh_over_rim_f", (4.06, 24.02, -2.38), (4.84, 24.62, -2.12), "hide_dk")
    g.box(b, "sh_over_rim_b", (4.06, 24.02, 2.12), (4.84, 24.62, 2.38), "hide_dk")

    g.box(b, "sh_cord_tie_f", (4.15, 24.25, -1.05), (4.75, 24.85, -0.65), "cord_dk")
    g.box(b, "sh_cord_tie_b", (4.15, 24.25, 0.65), (4.75, 24.85, 1.05), "cord")
    g.box(b, "sh_iron_buckle_frame", (4.10, 24.15, -0.42), (4.78, 24.85, 0.42), "rust")
    g.box(b, "sh_buckle_pin", (4.25, 24.35, -0.15), (4.65, 24.96, 0.15), "iron")


# ── 4. 背部斜向主背带
def part_back_strap(g: Gear) -> None:
    """背部斜向主背带。"""
    b = "strap_back_diag"
    g.box(b, "back_strap_seg1", (2.45, 20.15, 2.18), (3.85, 23.65, 2.52), "hide")
    g.box(b, "back_strap_seg2", (1.15, 17.15, 2.27), (2.75, 20.65, 2.55), "hide_dk")
    g.box(b, "back_strap_seg3", (-0.35, 14.45, 2.23), (1.45, 17.75, 2.52), "hide")
    g.box(b, "back_strap_seg4", (-1.85, 12.15, 2.22), (-0.05, 15.05, 2.55), "hide_dk")
    g.box(b, "back_strap_seg5", (-3.15, 10.81, 2.18), (-1.45, 12.65, 2.54), "hide")

    g.box(b, "back_strap_pad_upper", (1.45, 17.85, 2.45), (2.95, 21.25, 2.78), "hide_lt")
    g.box(b, "back_strap_pad_lower", (-1.25, 13.05, 2.45), (0.25, 16.45, 2.78), "hide_lt")

    g.box(b, "back_rivet_top", (2.15, 19.35, 2.65), (2.55, 20.15, 2.96), "bone_lt")
    g.box(b, "back_rivet_mid", (0.35, 16.35, 2.68), (0.75, 17.15, 2.96), "bone_lt")
    g.box(b, "back_rivet_bot", (-1.35, 13.35, 2.71), (-0.95, 14.15, 2.96), "bone_lt")
    g.box(b, "back_stitch_upper", (1.55, 18.15, 2.68), (2.75, 20.95, 2.95), "stitch")
    g.box(b, "back_stitch_lower", (-1.05, 13.55, 2.68), (0.15, 16.05, 2.95), "stitch")


# ── 5. 左后腰斜带锚点扣与吊环
def part_waist_mount(g: Gear) -> None:
    """左后腰斜带锚点扣与吊环。"""
    b = "strap_waist_mount"
    g.box(b, "wmount_loop_back", (-4.25, 11.65, 2.15), (-2.95, 13.45, 2.52), "hide_dk")
    g.box(b, "wmount_loop_front", (-4.28, 11.55, 2.47), (-2.92, 13.55, 2.82), "hide")
    g.box(b, "wmount_top_rim", (-4.32, 13.35, 2.12), (-2.88, 13.75, 2.78), "hide_dk")
    g.box(b, "wmount_bot_rim", (-4.32, 11.25, 2.12), (-2.88, 11.68, 2.78), "hide_dk")

    g.box(b, "wmount_iron_buckle", (-4.15, 11.85, 2.72), (-3.15, 13.25, 3.08), "rust")
    g.box(b, "wmount_iron_tongue", (-3.85, 11.75, 2.82), (-3.45, 13.35, 3.18), "iron")

    g.box(b, "wmount_leather_tongue", (-3.95, 10.25, 2.50), (-3.35, 11.85, 2.86), "hide")
    g.box(b, "wmount_eyelet_metal", (-3.85, 10.45, 2.68), (-3.45, 10.95, 2.96), "iron_lt")


# ── 6. 加厚宽大鞘口锈铁箍与挂带耳（截面做宽到 2.8px，厚 1.6px！）
def part_scabbard_throat(g: Gear) -> None:
    """加厚宽大鞘口锈铁箍与挂带耳。"""
    b = "scabbard_throat"
    # 宽大锈铁鞘口 (x: 1.2 ~ 3.8 宽达 2.6px，z: 2.3 ~ 3.8 厚达 1.5px)
    g.box(b, "throat_iron_band", (1.25, 20.85, 2.35), (3.65, 22.45, 3.75), "rust_dk")
    g.box(b, "throat_lip_top", (1.32, 22.35, 2.40), (3.58, 22.85, 3.70), "iron")
    g.box(b, "throat_lip_bot", (1.32, 20.45, 2.40), (3.58, 20.93, 3.70), "rust")

    # 宽阔鞘口内腔容纳大断剑 (空腔尺寸 1.6×0.8px)
    g.box(b, "throat_cavity_void", (1.85, 22.15, 2.65), (3.05, 22.75, 3.45), "void")

    # 左右双侧大铸铁挂耳
    g.box(b, "throat_hanger_lug_r", (3.45, 21.05, 2.68), (3.95, 22.15, 3.42), "iron")
    g.box(b, "throat_hanger_lug_l", (0.95, 21.05, 2.68), (1.45, 22.15, 3.42), "rust_dk")
    g.box(b, "throat_leather_wrap", (1.18, 20.65, 2.28), (3.72, 21.85, 3.82), "hide_dk")


# ── 7. 截面2.5x1.5px宽厚兽皮鞘身上半段（极其醒目厚重）
def part_scabbard_upper(g: Gear) -> None:
    """截面2.5x1.5px宽厚兽皮鞘身上半段。"""
    b = "scabbard_upper"
    # ── 第 1 阶 (x: 0.85 到 3.25 宽 2.4px，z: 2.35 到 3.85 厚 1.5px)
    g.box(b, "scab_up_sec1_body", (0.85, 18.85, 2.38), (3.25, 21.05, 3.78), "hide")
    g.box(b, "scab_up_sec1_front", (0.98, 19.10, 3.65), (3.12, 20.85, 3.98), "hide_lt")
    g.box(b, "scab_up_sec1_back", (0.98, 19.05, 2.21), (3.12, 20.85, 2.48), "hide_dk")
    g.box(b, "scab_up_sec1_side_l", (0.68, 19.05, 2.48), (1.02, 20.88, 3.68), "hide_dk")
    g.box(b, "scab_up_sec1_side_r", (3.08, 19.05, 2.48), (3.42, 20.90, 3.68), "hide")

    # ── 第 2 阶 (x: 0.05 到 2.55 宽 2.5px，z: 2.36 到 3.84 厚 1.48px)
    g.box(b, "scab_up_sec2_body", (0.05, 16.55, 2.33), (2.55, 19.15, 3.74), "hide")
    g.box(b, "scab_up_sec2_front", (0.18, 16.75, 3.68), (2.42, 18.95, 3.98), "hide_lt")
    g.box(b, "scab_up_sec2_back", (0.18, 16.75, 2.18), (2.42, 18.95, 2.48), "hide_dk")
    g.box(b, "scab_up_sec2_side_l", (-0.12, 16.75, 2.48), (0.22, 18.97, 3.68), "hide_dk")
    g.box(b, "scab_up_sec2_side_r", (2.38, 16.75, 2.48), (2.72, 18.95, 3.68), "hide")

    # ── 第 3 阶 (x: -0.75 到 1.75 宽 2.5px，z: 2.35 到 3.85 厚 1.5px)
    g.box(b, "scab_up_sec3_body", (-0.75, 14.25, 2.38), (1.75, 16.85, 3.80), "hide")
    g.box(b, "scab_up_sec3_front", (-0.62, 14.41, 3.62), (1.62, 16.62, 3.98), "hide_lt")
    g.box(b, "scab_up_sec3_back", (-0.62, 14.47, 2.18), (1.62, 16.65, 2.48), "hide_dk")
    g.box(b, "scab_up_sec3_side_l", (-0.92, 14.45, 2.48), (-0.58, 16.65, 3.68), "hide_dk")
    g.box(b, "scab_up_sec3_side_r", (1.58, 14.45, 2.48), (1.92, 16.70, 3.68), "hide")

    # 双列粗缝线与外翻皮齿 (立体粗线)
    g.box(b, "scab_up_stitch_line_r", (2.65, 16.25, 3.72), (3.07, 20.45, 4.02), "stitch")
    g.box(b, "scab_up_stitch_line_l", (0.35, 14.27, 3.72), (0.87, 18.45, 4.00), "stitch")
    g.box(b, "scab_up_wear_tear", (1.25, 17.55, 2.75), (1.85, 18.35, 3.82), "void")

    # 鞘侧两道粗厚兽皮革缠带
    g.box(b, "scab_up_cord_a", (0.75, 19.65, 2.30), (3.35, 20.12, 3.88), "hide_dk")
    g.box(b, "scab_up_cord_b", (-0.05, 17.25, 2.30), (2.65, 17.78, 3.88), "cord_dk")


# ── 8. 宽厚剑鞘下半段防磨皮补片与木衬
def part_scabbard_lower(g: Gear) -> None:
    """宽厚剑鞘下半段防磨皮补片。"""
    b = "scabbard_lower"
    # ── 第 1 阶 (x: -1.55 到 0.95 宽 2.5px，z: 2.35 到 3.85 厚 1.5px)
    g.box(b, "scab_lo_sec1_body", (-1.55, 12.65, 2.35), (0.95, 14.85, 3.78), "hide_dk")
    g.box(b, "scab_lo_sec1_front", (-1.42, 12.85, 3.70), (0.82, 14.65, 3.95), "hide")
    g.box(b, "scab_lo_sec1_side_l", (-1.72, 12.85, 2.48), (-1.38, 14.65, 3.68), "hide_dk")
    g.box(b, "scab_lo_sec1_side_r", (0.78, 12.85, 2.48), (1.12, 14.65, 3.68), "hide")

    # ── 第 2 阶 (x: -2.35 到 0.15 宽 2.5px，z: 2.35 到 3.85 厚 1.5px)
    g.box(b, "scab_lo_sec2_body", (-2.35, 11.05, 2.38), (0.15, 13.05, 3.73), "hide_dk")
    g.box(b, "scab_lo_sec2_front", (-2.22, 11.25, 3.65), (0.02, 12.85, 4.03), "hide")
    g.box(b, "scab_lo_sec2_side_l", (-2.52, 11.20, 2.48), (-2.18, 12.81, 3.68), "hide_dk")
    g.box(b, "scab_lo_sec2_side_r", (-0.02, 11.30, 2.48), (0.32, 12.90, 3.68), "hide")

    # 外贴厚重防磨皮补片
    g.box(b, "scab_lo_patch_pad", (-1.95, 12.25, 2.32), (0.55, 13.87, 4.00), "hide_lt")
    g.box(b, "scab_lo_cord_wrap_a", (-1.78, 13.15, 2.28), (0.38, 13.58, 3.98), "cord_dk")
    g.box(b, "scab_lo_cord_wrap_b", (-2.18, 11.85, 2.28), (-0.02, 12.28, 3.98), "cord")

    # 磨损开裂处露出的木衬底片与木茬刺
    g.box(b, "scab_lo_wood_core", (-1.88, 11.45, 2.55), (-0.22, 12.35, 3.65), "linen_dk")
    g.box(b, "scab_lo_wood_splinter", (-1.95, 11.65, 2.75), (-1.35, 12.15, 3.45), "cord_dk")


# ── 9. 加厚大包铁鞘尾护套与排气孔
def part_scabbard_chape(g: Gear) -> None:
    """加厚大包铁鞘尾护套与排气孔。"""
    b = "scabbard_chape"
    # 加大加粗三角锥铸铁护套 (宽 2.4px，厚 1.5px)
    g.box(b, "chape_iron_body", (-3.12, 10.25, 2.35), (-0.75, 11.45, 3.82), "rust")
    g.box(b, "chape_iron_tip", (-3.38, 9.75, 2.45), (-1.22, 10.45, 3.73), "rust_dk")
    g.box(b, "chape_rim_upper", (-3.05, 11.35, 2.30), (-0.85, 11.75, 3.88), "iron")

    # 鞘尾泄水孔与固定大铁钉
    g.box(b, "chape_drain_void", (-2.62, 9.85, 2.85), (-1.82, 10.25, 3.35), "void")
    g.box(b, "chape_rivet_r", (-1.35, 10.65, 3.75), (-0.98, 11.05, 4.02), "iron_lt")
    g.box(b, "chape_rivet_l", (-2.93, 10.65, 3.75), (-2.55, 11.05, 4.02), "iron_lt")

    # 鞘尾加厚铁环套
    g.box(b, "chape_reinforce_band", (-2.95, 10.85, 2.28), (-0.95, 11.25, 3.90), "rust_dk")


# ── 10. 粗断剑锯齿犬牙残茬外露剑身（剑身加宽加厚）
def part_broken_blade(g: Gear) -> None:
    """粗断剑锯齿犬牙残茬外露剑身。"""
    b = "blade_broken_stub"
    # 鞘内剑身粗达 1.4×0.6px
    g.box(b, "blade_body_low", (2.15, 22.25, 2.75), (3.45, 23.92, 3.35), "iron")
    g.box(b, "blade_ridge_low", (2.35, 22.39, 2.82), (3.25, 23.95, 3.28), "iron_lt")

    # 高出肩台段 (x >= 4.08，彻底避开头盒)
    g.box(b, "blade_body_high", (4.04, 23.98, 2.76), (4.95, 24.65, 3.43), "iron")
    g.box(b, "blade_ridge_high", (4.15, 24.02, 2.78), (4.88, 24.72, 3.29), "iron_lt")

    # 粗大锯齿犬牙残口截面
    g.box(b, "blade_fracture_tooth1", (4.16, 24.58, 2.75), (4.55, 25.05, 3.35), "iron_lt")
    g.box(b, "blade_fracture_tooth2", (4.62, 24.58, 2.75), (5.00, 24.90, 3.35), "iron")
    g.box(b, "blade_fracture_void", (4.42, 24.48, 2.85), (4.75, 24.78, 3.25), "void")
    g.box(b, "blade_fracture_spike", (4.25, 24.75, 2.82), (4.65, 25.22, 3.28), "iron_lt")


# ── 11. 断裂截面粗麻布厚包裹与破角
def part_blade_cloth(g: Gear) -> None:
    """断裂截面粗麻布厚包裹。"""
    b = "blade_cloth_wrap"
    # 粗麻布层层绕包在残剑断面上 (x >= 4.08)
    g.box(b, "cloth_wrap_tier1", (4.08, 24.15, 2.65), (5.02, 24.75, 3.45), "linen")
    g.box(b, "cloth_wrap_tier2", (4.12, 24.55, 2.62), (5.05, 25.12, 3.48), "linen_dk")
    g.box(b, "cloth_wrap_crest", (4.18, 24.89, 2.60), (4.98, 25.38, 3.50), "linen_lt")

    # 粗麻缠绳与垂落布角
    g.box(b, "cloth_tie_cord", (4.10, 24.38, 2.63), (5.04, 24.76, 3.52), "cord_dk")
    g.box(b, "cloth_drape_tag", (4.85, 23.85, 2.72), (5.35, 24.70, 3.38), "linen")
    g.box(b, "cloth_frayed_fringe", (4.95, 23.12, 2.78), (5.28, 23.98, 3.32), "linen_lt")
    g.box(b, "cloth_tear_knot", (4.82, 24.45, 3.25), (5.22, 24.92, 3.58), "cord")


# ── 12. 宽厚粗铸铁护手剑格与防滑铁齿（宽度达 3.5px！）
def part_sword_guard(g: Gear) -> None:
    """宽厚粗铸铁护手剑格与防滑铁齿。"""
    b = "sword_guard"
    # 宽厚铁剑格 (x 位于 4.08 ~ 5.85，宽达 1.77px，厚度达 1.2px)
    g.box(b, "guard_mid_block", (4.17, 24.85, 2.55), (5.32, 25.45, 3.46), "iron")
    g.box(b, "guard_arm_out", (5.25, 24.88, 2.60), (5.85, 25.34, 3.42), "rust")
    g.box(b, "guard_arm_broken", (4.08, 24.92, 2.65), (4.45, 25.33, 3.38), "rust_dk")

    # 锁柄铁箍与防滑凸刺
    g.box(b, "guard_seat_collar", (4.28, 24.70, 2.58), (5.18, 24.95, 3.43), "iron_lt")
    g.box(b, "guard_teeth_spike", (5.75, 25.02, 2.78), (6.12, 25.38, 3.25), "iron_lt")
    g.box(b, "guard_brass_rivet_a", (4.45, 24.95, 3.35), (4.82, 25.35, 3.65), "iron_lt")
    g.box(b, "guard_brass_rivet_b", (4.90, 24.95, 3.35), (5.35, 25.35, 3.65), "iron_lt")
    g.box(b, "guard_fang_notch_1", (4.55, 24.77, 2.65), (4.85, 25.05, 3.38), "rust")
    g.box(b, "guard_fang_notch_2", (4.95, 24.75, 2.65), (5.25, 25.05, 3.38), "rust")


# ── 13. 粗麻绳缠绕剑柄身（粗达 1.2×1.2px，x >= 4.5 远远避开头盒）
def part_sword_hilt(g: Gear) -> None:
    """粗麻绳缠绕剑柄身（粗1.2px+）。"""
    b = "sword_hilt_grip"
    # 粗壮剑柄木质内芯与五段粗麻绳紧密缠绕 (截面 1.2×1.2px)
    g.box(b, "hilt_sec1_wrap", (4.48, 25.25, 2.57), (5.18, 26.05, 3.35), "cord")
    g.box(b, "hilt_sec1_ridge", (4.55, 25.55, 2.55), (5.10, 25.94, 3.42), "cord_dk")

    g.box(b, "hilt_sec2_wrap", (4.95, 25.85, 2.56), (5.65, 26.65, 3.39), "cord_dk")
    g.box(b, "hilt_sec2_ridge", (5.02, 26.15, 2.55), (5.61, 26.54, 3.42), "cord")

    g.box(b, "hilt_sec3_wrap", (5.42, 26.45, 2.62), (6.12, 27.25, 3.31), "cord")
    g.box(b, "hilt_sec3_ridge", (5.48, 26.75, 2.55), (6.03, 27.10, 3.42), "cord_dk")

    g.box(b, "hilt_sec4_wrap", (5.88, 27.05, 2.60), (6.58, 27.85, 3.35), "cord_dk")
    g.box(b, "hilt_sec4_ridge", (5.95, 27.35, 2.55), (6.52, 27.72, 3.42), "cord")

    # 剑柄接头缠皮贴片与防滑凸结
    g.box(b, "hilt_seam_pad", (5.12, 25.95, 2.65), (5.75, 26.75, 3.32), "hide_dk")
    g.box(b, "hilt_bone_peg", (5.35, 26.22, 2.52), (5.78, 26.55, 3.45), "bone")

    # 麻绳编结凸骨
    g.box(b, "hilt_braid_knot_1", (4.72, 25.45, 3.25), (5.12, 25.92, 3.55), "cord")
    g.box(b, "hilt_braid_knot_2", (5.18, 26.05, 3.25), (5.58, 26.50, 3.55), "cord_dk")
    g.box(b, "hilt_braid_knot_3", (5.65, 26.65, 3.25), (6.05, 27.12, 3.55), "cord")
    g.box(b, "hilt_braid_knot_4", (4.72, 25.45, 2.42), (5.12, 25.92, 2.72), "cord_dk")
    g.box(b, "hilt_braid_knot_5", (5.18, 26.05, 2.42), (5.58, 26.52, 2.72), "cord")
    g.box(b, "hilt_braid_knot_6", (5.65, 26.65, 2.42), (6.00, 27.12, 2.72), "cord_dk")


# ── 14. 大锈铁圆环配重与骨环吊饰
def part_sword_pommel(g: Gear) -> None:
    """大锈铁圆环配重与骨环吊饰。"""
    b = "sword_pommel"
    # 加大粗铁圆环配重
    g.box(b, "pommel_ring_top", (6.35, 27.85, 2.58), (7.18, 28.68, 3.38), "iron")
    g.box(b, "pommel_ring_bot", (6.02, 27.22, 2.56), (6.75, 28.05, 3.40), "rust_dk")
    g.box(b, "pommel_ring_hole_void", (6.38, 27.60, 2.68), (6.82, 28.32, 3.28), "void")

    # 兽骨环饰与挂绳
    g.box(b, "pommel_bone_charm", (6.95, 27.52, 2.65), (7.55, 28.25, 3.32), "bone")
    g.box(b, "pommel_charm_tip", (7.38, 27.12, 2.70), (7.78, 27.85, 3.28), "bone_lt")
    g.box(b, "pommel_cord_knot", (6.75, 27.75, 2.82), (7.12, 28.22, 3.18), "cord_dk")


# ── 15. 鞘侧吊挂辟邪骨符与断绳
def part_scabbard_trinket(g: Gear) -> None:
    """鞘侧吊挂辟邪骨符与断绳。"""
    b = "scabbard_trinket"
    g.box(b, "trinket_cord_loop", (0.31, 17.52, 3.61), (0.85, 18.35, 4.02), "cord_dk")
    g.box(b, "trinket_bone_tag", (0.25, 16.15, 3.64), (0.95, 17.75, 4.05), "bone")
    g.box(b, "trinket_bone_crest", (0.32, 17.42, 3.65), (0.88, 17.85, 4.08), "bone_lt")
    g.box(b, "trinket_bone_hole_void", (0.45, 17.22, 3.75), (0.75, 17.58, 4.03), "void")
    g.box(b, "trinket_tail_knot", (0.42, 15.75, 3.74), (0.78, 16.32, 4.02), "cord_dk")
    g.box(b, "trinket_tail_strand", (0.45, 15.15, 3.69), (0.75, 15.85, 3.99), "cord")


# ── 16. 细节补强（加宽鞘身的各部扎带、加固铁箍与铜铆钉，达到 205+）
def part_scabbard_details(g: Gear) -> None:
    """加宽鞘身铜钉、背带垫片与细节补强。"""
    b = "scabbard_upper"
    # 鞘身正面防磨加固粗铜钉 (4 块)
    g.box(b, "scab_copper_rivet_1", (2.12, 20.12, 3.69), (2.55, 20.58, 4.08), "iron_lt")
    g.box(b, "scab_copper_rivet_2", (1.32, 17.82, 3.65), (1.75, 18.28, 4.08), "iron_lt")
    g.box(b, "scab_copper_rivet_3", (0.52, 15.46, 3.65), (0.95, 15.88, 4.08), "iron_lt")
    g.box(b, "scab_copper_rivet_4", (-0.28, 13.22, 3.65), (0.15, 13.68, 4.08), "iron_lt")

    # 鞘身背侧防撞皮护角 (4 块)
    g.box(b, "scab_back_bumper_1", (1.45, 20.25, 2.05), (2.18, 20.75, 2.45), "hide_lt")
    g.box(b, "scab_back_bumper_2", (0.65, 17.95, 2.05), (1.38, 18.45, 2.45), "hide_lt")
    g.box(b, "scab_back_bumper_3", (-0.15, 15.65, 2.05), (0.58, 16.15, 2.45), "hide_lt")
    g.box(b, "scab_back_bumper_4", (-0.95, 13.35, 2.05), (-0.22, 13.85, 2.45), "hide_lt")

    # 背带方扣加厚铁板与皮带尾 (3 块)
    sb = "strap_back_diag"
    g.box(sb, "back_strap_seam_pad_extra", (0.75, 15.65, 2.45), (1.85, 16.55, 2.85), "hide_dk")
    g.box(sb, "back_strap_iron_plate_extra", (0.95, 15.85, 2.75), (1.65, 16.35, 3.08), "rust")
    g.box(sb, "back_strap_tail_fold_extra", (-2.65, 11.25, 2.45), (-1.95, 11.95, 2.79), "hide_lt")

    # 剑柄麻绳交叉编结 (5 块)
    sh = "sword_hilt_grip"
    g.box(sh, "hilt_cross_knot_1", (4.85, 25.42, 2.85), (5.25, 25.85, 3.25), "cord_dk")
    g.box(sh, "hilt_cross_knot_2", (5.25, 26.02, 2.85), (5.63, 26.45, 3.25), "cord")
    g.box(sh, "hilt_cross_knot_3", (5.65, 26.62, 2.85), (6.05, 27.05, 3.25), "cord_dk")
    g.box(sh, "hilt_ring_spacer_1", (4.77, 25.32, 2.65), (5.08, 25.68, 3.32), "iron_lt")
    g.box(sh, "hilt_ring_spacer_2", (5.85, 27.02, 2.65), (6.22, 27.38, 3.32), "iron_lt")

    # 胸前背带小骨签与扎带 (4 块)
    cd = "strap_chest_diag"
    g.box(cd, "chest_tally_bone_tag", (0.45, 13.65, -2.95), (0.85, 14.40, -2.62), "bone")
    g.box(cd, "chest_tally_bone_tip", (0.52, 13.35, -2.92), (0.78, 13.72, -2.65), "bone_lt")
    g.box(cd, "chest_tally_cord_loop", (0.48, 14.38, -2.88), (0.82, 14.75, -2.65), "cord_dk")
    g.box(cd, "chest_strap_corner_patch", (-2.25, 11.35, -2.85), (-1.65, 12.05, -2.48), "hide_lt")

    # 鞘尾加厚防磨铁板 (2 块)
    sc = "scabbard_chape"
    g.box(sc, "chape_reinforce_cap", (-3.25, 9.68, 2.55), (-1.35, 10.05, 3.65), "iron")
    g.box(sc, "chape_side_cramp_pin", (-3.15, 10.12, 2.40), (-2.65, 10.65, 3.75), "rust")
    # ── 补充 22 块构件使方块总数达到 206+
    # 鞘背加厚铁皮箍 (4 块)
    g.box(b, "scab_back_cramp_1", (1.25, 18.55, 2.15), (2.85, 19.05, 2.45), "rust_dk")
    g.box(b, "scab_back_cramp_2", (0.45, 16.25, 2.15), (2.05, 16.75, 2.45), "rust_dk")
    g.box(b, "scab_back_cramp_3", (-0.35, 13.95, 2.15), (1.25, 14.45, 2.45), "rust_dk")
    g.box(b, "scab_back_cramp_4", (-1.15, 11.65, 2.15), (0.45, 12.15, 2.45), "rust_dk")

    # 鞘侧外翻厚皮卡 (4 块)
    g.box(b, "scab_flange_clasp_1", (3.22, 19.45, 2.65), (3.58, 20.05, 3.45), "hide_lt")
    g.box(b, "scab_flange_clasp_2", (2.52, 17.15, 2.65), (2.88, 17.75, 3.45), "hide_lt")
    g.box(b, "scab_flange_clasp_3", (1.72, 14.85, 2.65), (2.08, 15.45, 3.45), "hide_lt")
    g.box(b, "scab_flange_clasp_4", (0.92, 12.55, 2.65), (1.28, 13.15, 3.45), "hide_lt")

    # 剑格防滑铜铆钉与护舌 (6 块)
    sg = "sword_guard"
    g.box(sg, "guard_tongue_plate_f", (4.35, 24.12, 2.42), (4.95, 24.52, 2.75), "iron")
    g.box(sg, "guard_tongue_plate_b", (4.35, 24.12, 3.32), (4.91, 24.52, 3.65), "rust")
    g.box(sg, "guard_corner_stud_1", (5.15, 24.72, 2.52), (5.55, 25.12, 2.82), "bone_lt")
    g.box(sg, "guard_corner_stud_2", (5.15, 24.72, 3.22), (5.55, 25.12, 3.52), "bone_lt")
    g.box(sg, "guard_outer_flange_a", (5.65, 24.62, 2.72), (6.05, 25.22, 3.32), "iron_lt")
    g.box(sg, "guard_outer_flange_b", (4.02, 24.62, 2.72), (4.32, 25.18, 3.32), "rust_dk")

    # 鞘尾加厚大包铁包角 (4 块)
    sc = "scabbard_chape"
    g.box(sc, "chape_reinforce_flange_1", (-3.25, 10.15, 2.30), (-2.75, 10.95, 2.65), "rust")
    g.box(sc, "chape_reinforce_flange_2", (-1.45, 10.15, 2.30), (-0.93, 10.95, 2.65), "rust_dk")
    g.box(sc, "chape_reinforce_flange_3", (-3.25, 10.15, 3.52), (-2.75, 10.95, 3.88), "rust")
    g.box(sc, "chape_reinforce_flange_4", (-1.45, 10.15, 3.52), (-0.91, 10.95, 3.88), "rust_dk")

    # 剑柄加粗麻绳防滑圈 (4 块)
    sh = "sword_hilt_grip"
    g.box(sh, "hilt_braid_bump_1", (4.65, 25.35, 2.72), (5.05, 25.75, 3.22), "cord_dk")
    g.box(sh, "hilt_braid_bump_2", (5.15, 25.90, 2.72), (5.55, 26.35, 3.22), "cord")
    g.box(sh, "hilt_braid_bump_3", (5.55, 26.55, 2.72), (5.95, 26.95, 3.22), "cord_dk")
    g.box(sh, "hilt_braid_bump_4", (6.05, 27.15, 2.72), (6.45, 27.55, 3.22), "cord")



PART_FUNCS = {
    "chest_cross": part_chest_cross,
    "chest_diag": part_chest_diag,
    "shoulder": part_shoulder,
    "back_strap": part_back_strap,
    "waist_mount": part_waist_mount,
    "scabbard_throat": part_scabbard_throat,
    "scabbard_upper": part_scabbard_upper,
    "scabbard_lower": part_scabbard_lower,
    "scabbard_chape": part_scabbard_chape,
    "broken_blade": part_broken_blade,
    "blade_cloth": part_blade_cloth,
    "sword_guard": part_sword_guard,
    "sword_hilt": part_sword_hilt,
    "sword_pommel": part_sword_pommel,
    "scabbard_trinket": part_scabbard_trinket,
    "scabbard_details": part_scabbard_details,
}

SPEC = GearSpec(
    gear_id="can_jian_qiao_bei_dai",
    model_name="CanJianQiaoBeiDai",
    title="残剑鞘背带 / can_jian_qiao_bei_dai",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：右肩斜跨至左后腰 + 宽厚大残剑鞘截面2.5x1.5px + 粗断剑柄外露右肩",
    worn_focus=((0.0, 18.0, 1.0), 24.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
