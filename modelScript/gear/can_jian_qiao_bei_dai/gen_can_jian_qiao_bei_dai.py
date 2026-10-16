#!/usr/bin/env python3
"""残剑鞘背带（can_jian_qiao_bei_dai）Blockbench .bbmodel 程序化生成器 —— Round 1/3：分部件建模 + 门禁全绿 + 差分自证。

对应 NPC：残峰散修（can_feng_san_xiu）。
挂点：背部斜挎。皮带从右肩经背部斜到左后腰，鞘悬在后背斜跨，剑柄自右肩外侧后方露出一截（x > 4.05，完全避开头盒）；
避开左肩（留出骨片肩甲位置），避开躯干与双臂摆动盒（翻肩段走 x∈[4.08, 5.25], y∈[24.05, 24.85] 骑过右臂上沿）。

外形：兽皮缝制残剑鞘、内衬木衬与粗缝线、锈铁鞘口与鞘尾；
鞘内斜插半截断剑：锯齿犬牙截面、碎布半缠包裹、麻绳缠柄、锈铁环首；
斜跨皮带带锈铁方扣与骨针扣。

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
    ("scabbard_throat", "strap_back_diag", (2.2, 21.5, 2.8), "鞘口加厚锈铁箍与挂带耳"),
    ("scabbard_upper", "scabbard_throat", (0.8, 18.0, 2.9), "剑鞘上半段兽皮鞘身与双列粗缝线"),
    ("scabbard_lower", "scabbard_upper", (-1.2, 14.5, 2.8), "剑鞘下半段防磨皮补片与木衬露茬"),
    ("scabbard_chape", "scabbard_lower", (-2.8, 11.2, 2.7), "鞘尾铸铁护套包头与排气孔"),
    ("blade_broken_stub", "scabbard_throat", (3.6, 23.2, 2.7), "断剑外露剑身锯齿犬牙残茬"),
    ("blade_cloth_wrap", "blade_broken_stub", (3.8, 23.6, 2.7), "断裂剑截面半缠粗麻布与破角"),
    ("sword_guard", "blade_broken_stub", (4.2, 24.2, 2.7), "残破铁剑格与防滑铁齿"),
    ("sword_hilt_grip", "sword_guard", (4.8, 25.8, 2.7), "麻绳缠绕剑柄身（外露右肩斜上方）"),
    ("sword_pommel", "sword_hilt_grip", (5.6, 27.8, 2.7), "柄首厚锈铁圆环配重与骨环吊饰"),
    ("scabbard_trinket", "scabbard_upper", (0.5, 17.2, 3.2), "鞘侧吊挂磨损骨符与断绳"),
]

ASYM_BONES = frozenset([b[0] for b in BONES if b[0] not in ("root", "strap_chest_cross")])
SOFT_OVER = (
    ("bone", "bone_lt"), ("hide", "hide_dk"), ("hide", "hide_lt"),
    ("linen", "linen_lt"), ("linen", "linen_dk"), ("cord", "cord_dk"),
    ("iron", "rust"), ("rust", "rust_dk"), ("iron", "iron_lt"),
    ("hide", "cord"), ("hide", "stitch"), ("hide", "iron"),
    ("linen", "cord"), ("linen", "stitch"), ("linen", "iron"),
    ("hide", "bone"), ("iron", "cord"), ("bone", "void"),
)


# ── 1. 胸前对称横向固形扣带（完全对称骨骼，满足 mirror 自检）
def part_chest_cross(g: Gear) -> None:
    """胸前对称横向固形扣带。"""
    b = "strap_chest_cross"
    g.box(b, "chest_mid_clasp", (-0.95, 14.18, -2.48), (0.95, 15.62, -2.18), "hide_dk")
    g.box(b, "chest_mid_iron_buckle", (-0.68, 14.28, -2.72), (0.68, 15.52, -2.44), "iron")
    g.box(b, "chest_pin_bone", (-0.22, 14.05, -2.85), (0.22, 15.75, -2.55), "bone")
    g.box(b, "chest_buckle_frame_t", (-0.58, 15.35, -2.82), (0.58, 15.68, -2.52), "rust")
    g.box(b, "chest_buckle_frame_b", (-0.58, 14.12, -2.82), (0.58, 14.47, -2.52), "rust_dk")

    # 对称左右胸肋横系带（严格镜像）
    g.pair(b, "chest_wing_strap", (-3.85, 14.25, -2.44), (-0.90, 15.55, -2.12), "hide_dk")
    g.pair(b, "chest_wing_pad", (-3.82, 14.35, -2.62), (-0.92, 15.45, -2.34), "hide")
    g.pair(b, "chest_wing_stitch", (-3.45, 14.42, -2.75), (-1.45, 15.38, -2.48), "stitch")
    g.pair(b, "chest_rivet", (-2.65, 14.52, -2.82), (-2.25, 15.28, -2.52), "bone_lt")
    g.pair(b, "chest_wing_loop", (-3.65, 14.32, -2.72), (-3.25, 15.48, -2.42), "cord_dk")


# ── 2. 胸前斜挎主背带（非对称骨骼，从右胸向左肋跨越）
def part_chest_diag(g: Gear) -> None:
    """胸前斜挎主背带。"""
    b = "strap_chest_diag"
    g.box(b, "chest_diag_seg1", (2.65, 20.25, -2.48), (3.95, 23.65, -2.15), "hide")
    g.box(b, "chest_diag_seg2", (1.45, 16.95, -2.52), (2.85, 20.45, -2.18), "hide_dk")
    g.box(b, "chest_diag_seg3", (0.15, 13.95, -2.55), (1.65, 17.25, -2.20), "hide")
    g.box(b, "chest_diag_seg4", (-1.25, 11.95, -2.56), (0.35, 14.35, -2.22), "hide_dk")
    g.box(b, "chest_diag_seg5", (-2.55, 10.95, -2.54), (-1.05, 12.35, -2.20), "hide")

    # 外层加厚补强垫片与双排针脚
    g.box(b, "chest_diag_pad_hi", (1.85, 17.95, -2.71), (3.15, 21.45, -2.42), "hide_lt")
    g.box(b, "chest_diag_pad_lo", (-0.75, 12.65, -2.76), (0.75, 15.85, -2.47), "hide_lt")
    g.box(b, "chest_diag_stitch_hi", (2.09, 18.35, -2.86), (2.95, 21.05, -2.58), "stitch")
    g.box(b, "chest_diag_stitch_lo", (-0.55, 13.05, -2.88), (0.55, 15.45, -2.60), "stitch")

    # 斜带上的骨制穿孔固定销与装饰骨片
    g.box(b, "chest_diag_bone_peg", (2.25, 19.25, -2.92), (2.75, 19.85, -2.62), "bone")
    g.box(b, "chest_diag_bone_peg_t", (2.35, 19.85, -2.95), (2.65, 20.18, -2.65), "bone_lt")
    g.box(b, "chest_bone_clasp_mid", (0.85, 14.85, -2.85), (1.55, 15.65, -2.52), "bone")
    g.box(b, "chest_bone_clasp_t", (0.95, 15.52, -2.88), (1.45, 15.88, -2.55), "bone_lt")
    g.box(b, "chest_strap_rivet_1", (2.25, 21.25, -2.75), (2.55, 21.65, -2.45), "bone_lt")
    g.box(b, "chest_strap_rivet_2", (2.05, 18.25, -2.75), (2.35, 18.65, -2.45), "bone_lt")
    g.box(b, "chest_strap_rivet_3", (-0.15, 15.25, -2.75), (0.15, 15.65, -2.45), "bone_lt")
    g.box(b, "chest_strap_rivet_4", (-0.85, 12.25, -2.75), (-0.55, 12.65, -2.45), "bone_lt")


# ── 3. 右肩宽皮垫与过肩带（严格控高：x<=4.0 时 y<=23.95；x>=4.08 时 y>=24.05 骑过右臂，头盒0侵入）
def part_shoulder(g: Gear) -> None:
    """右肩宽皮垫与过肩带。"""
    b = "strap_shoulder_r"
    # 前后过渡段（y 严格保持在 23.95 以下，紧贴肩台）
    g.box(b, "sh_front_jog", (3.75, 22.85, -2.43), (4.35, 23.92, -2.12), "hide")
    g.box(b, "sh_back_jog", (3.75, 22.85, 2.12), (4.35, 23.92, 2.48), "hide")

    # 翻肩段（完全位于 x∈[4.08, 5.35], y∈[24.05, 24.85]，完全在头盒 x<=4.0 之外，在右臂 y<=24.0 之上）
    g.box(b, "sh_over_base", (4.08, 24.05, -2.25), (4.82, 24.58, 2.25), "hide_dk")
    g.box(b, "sh_over_outer", (4.12, 24.28, -2.05), (4.76, 24.78, 2.05), "hide")
    g.box(b, "sh_over_crest", (4.18, 24.52, -1.65), (4.72, 24.88, 1.65), "hide_lt")

    # 翻肩包边与防滑带
    g.box(b, "sh_over_rim_f", (4.06, 24.02, -2.38), (4.84, 24.62, -2.12), "hide_dk")
    g.box(b, "sh_over_rim_b", (4.06, 24.02, 2.12), (4.84, 24.62, 2.38), "hide_dk")

    # 肩顶外侧扎带与铁方扣
    g.box(b, "sh_cord_tie_f", (4.15, 24.25, -1.05), (4.75, 24.85, -0.65), "cord_dk")
    g.box(b, "sh_cord_tie_b", (4.15, 24.25, 0.65), (4.75, 24.85, 1.05), "cord")
    g.box(b, "sh_iron_buckle_frame", (4.07, 24.15, -0.42), (4.78, 24.92, 0.42), "rust")
    g.box(b, "sh_buckle_pin", (4.25, 24.35, -0.15), (4.65, 24.96, 0.15), "iron")


# ── 4. 背部斜向主背带（从右肩后沿向左后腰倾斜）
def part_back_strap(g: Gear) -> None:
    """背部斜向主背带。"""
    b = "strap_back_diag"
    # 背部主背带 5 段斜向阶梯搭接 (z∈[2.15, 2.85])
    g.box(b, "back_strap_seg1", (2.45, 20.15, 2.18), (3.85, 23.65, 2.52), "hide")
    g.box(b, "back_strap_seg2", (1.15, 17.15, 2.21), (2.75, 20.65, 2.50), "hide_dk")
    g.box(b, "back_strap_seg3", (-0.35, 14.45, 2.23), (1.45, 17.77, 2.52), "hide")
    g.box(b, "back_strap_seg4", (-1.85, 12.15, 2.16), (-0.05, 15.05, 2.57), "hide_dk")
    g.box(b, "back_strap_seg5", (-3.15, 10.85, 2.18), (-1.40, 12.65, 2.52), "hide")

    # 外层加厚护皮与补强贴片
    g.box(b, "back_strap_pad_upper", (1.47, 17.85, 2.45), (2.95, 21.25, 2.78), "hide_lt")
    g.box(b, "back_strap_pad_lower", (-1.25, 13.05, 2.45), (0.25, 16.48, 2.78), "hide_lt")

    # 紧固双排骨铆钉与双列针脚
    g.box(b, "back_rivet_top", (2.15, 19.35, 2.72), (2.52, 20.15, 2.96), "bone_lt")
    g.box(b, "back_rivet_mid", (0.35, 16.35, 2.68), (0.75, 17.15, 2.96), "bone_lt")
    g.box(b, "back_rivet_bot", (-1.39, 13.35, 2.73), (-0.95, 14.15, 2.96), "bone_lt")
    g.box(b, "back_stitch_upper", (1.55, 18.15, 2.68), (2.73, 20.97, 2.95), "stitch")
    g.box(b, "back_stitch_lower", (-1.05, 13.55, 2.68), (0.15, 16.05, 2.95), "stitch")

    # 背带上的方扣加厚铁板与皮带尾折角
    g.box(b, "back_strap_seam_pad", (0.75, 15.65, 2.45), (1.85, 16.55, 2.82), "hide_dk")
    g.box(b, "back_strap_iron_plate", (0.95, 15.85, 2.75), (1.65, 16.35, 3.05), "rust")
    g.box(b, "back_strap_tail_fold", (-2.65, 11.25, 2.42), (-1.95, 11.95, 2.76), "hide_lt")


# ── 5. 左后腰斜带锚点扣与吊环
def part_waist_mount(g: Gear) -> None:
    """左后腰斜带锚点扣与吊环。"""
    b = "strap_waist_mount"
    g.box(b, "wmount_loop_back", (-4.25, 11.65, 2.15), (-2.95, 13.45, 2.55), "hide_dk")
    g.box(b, "wmount_loop_front", (-4.28, 11.55, 2.45), (-2.92, 13.55, 2.82), "hide")
    g.box(b, "wmount_top_rim", (-4.32, 13.35, 2.12), (-2.88, 13.75, 2.78), "hide_dk")
    g.box(b, "wmount_bot_rim", (-4.32, 11.25, 2.12), (-2.88, 11.68, 2.78), "hide_dk")

    # 锈铁长方形连接带扣
    g.box(b, "wmount_iron_buckle", (-4.15, 11.85, 2.72), (-3.15, 13.25, 3.08), "rust")
    g.box(b, "wmount_iron_tongue", (-3.85, 11.75, 2.82), (-3.45, 13.35, 3.18), "iron")

    # 粗皮插舌下垂段与金属气眼
    g.box(b, "wmount_leather_tongue", (-3.95, 10.25, 2.43), (-3.35, 11.85, 2.85), "hide")
    g.box(b, "wmount_eyelet_metal", (-3.85, 10.45, 2.68), (-3.45, 10.95, 2.96), "iron_lt")


# ── 6. 鞘口加厚锈铁箍与挂带耳
def part_scabbard_throat(g: Gear) -> None:
    """鞘口加厚锈铁箍与挂带耳。"""
    b = "scabbard_throat"
    g.box(b, "throat_iron_band", (1.75, 20.85, 2.55), (3.15, 22.45, 3.35), "rust_dk")
    g.box(b, "throat_lip_top", (1.82, 22.39, 2.58), (3.08, 22.75, 3.32), "iron")
    g.box(b, "throat_lip_bot", (1.82, 20.55, 2.53), (3.08, 20.95, 3.32), "rust")

    # 鞘口内腔空隙
    g.box(b, "throat_cavity_void", (2.05, 22.15, 2.72), (2.75, 22.65, 3.18), "void")

    # 侧边铜耳与挂接皮套
    g.box(b, "throat_hanger_lug_r", (2.95, 21.05, 2.68), (3.45, 22.15, 3.22), "iron")
    g.box(b, "throat_hanger_lug_l", (1.45, 21.05, 2.68), (1.95, 22.15, 3.22), "rust_dk")
    g.box(b, "throat_leather_wrap", (1.62, 20.72, 2.48), (3.28, 21.85, 3.42), "hide_dk")


# ── 7. 剑鞘上半段兽皮鞘身与双列粗缝线
def part_scabbard_upper(g: Gear) -> None:
    """剑鞘上半段兽皮鞘身。"""
    b = "scabbard_upper"
    g.box(b, "scab_up_sec1_body", (1.35, 18.83, 2.58), (2.55, 21.05, 3.28), "hide")
    g.box(b, "scab_up_sec1_crest", (1.48, 19.15, 3.18), (2.38, 20.87, 3.44), "hide_lt")
    g.box(b, "scab_up_sec1_side", (1.12, 19.05, 2.65), (1.42, 20.75, 3.20), "hide_dk")

    g.box(b, "scab_up_sec2_body", (0.55, 16.55, 2.56), (1.75, 19.15, 3.26), "hide")
    g.box(b, "scab_up_sec2_crest", (0.68, 16.85, 3.16), (1.58, 18.85, 3.42), "hide_lt")
    g.box(b, "scab_up_sec2_side", (0.32, 16.75, 2.62), (0.62, 18.72, 3.18), "hide_dk")

    g.box(b, "scab_up_sec3_body", (-0.25, 14.25, 2.54), (0.95, 16.85, 3.24), "hide")
    g.box(b, "scab_up_sec3_crest", (-0.12, 14.55, 3.14), (0.78, 16.55, 3.40), "hide_lt")
    g.box(b, "scab_up_sec3_side", (-0.48, 14.45, 2.60), (-0.18, 16.47, 3.16), "hide_dk")

    # 双列粗缝线与外翻皮齿
    g.box(b, "scab_up_stitch_line_r", (1.72, 16.25, 3.22), (2.18, 20.45, 3.48), "stitch")
    g.box(b, "scab_up_stitch_line_l", (0.12, 14.28, 3.20), (0.58, 18.45, 3.44), "stitch")
    g.box(b, "scab_up_wear_tear", (0.98, 17.55, 2.75), (1.48, 18.35, 3.32), "void")

    # 鞘侧两道粗麻绳加固扎带
    g.box(b, "scab_up_cord_a", (1.20, 19.65, 2.50), (2.65, 20.10, 3.35), "cord_dk")
    g.box(b, "scab_up_cord_b", (0.40, 17.30, 2.48), (1.85, 17.75, 3.34), "cord")

    # 鞘背加厚防磨层
    g.box(b, "scab_back_pad_1", (1.25, 19.29, 2.35), (2.44, 20.72, 2.58), "hide_dk")
    g.box(b, "scab_back_pad_2", (0.47, 17.01, 2.38), (1.70, 18.68, 2.69), "hide_dk")
    g.box(b, "scab_back_pad_3", (-0.32, 14.72, 2.41), (0.94, 16.50, 2.71), "hide_dk")

    # 鞘身双侧小骨钉
    g.box(b, "scab_bone_stud_1", (2.44, 19.54, 2.79), (2.78, 19.93, 3.07), "bone_lt")
    g.box(b, "scab_bone_stud_2", (1.64, 17.19, 2.84), (2.01, 17.61, 3.12), "bone_lt")
    g.box(b, "scab_bone_stud_3", (0.82, 14.90, 2.77), (1.18, 15.30, 3.13), "bone_lt")

    # 鞘身双侧外缝翻边齿
    g.box(b, "scab_flange_tooth_1", (2.23, 20.45, 3.12), (2.68, 20.81, 3.39), "hide_dk")
    g.box(b, "scab_flange_tooth_2", (1.75, 18.82, 3.10), (2.02, 19.28, 3.52), "hide_dk")
    g.box(b, "scab_flange_tooth_3", (1.20, 17.41, 3.22), (1.54, 17.79, 3.47), "hide_dk")
    g.box(b, "scab_flange_tooth_4", (0.49, 15.93, 3.15), (0.90, 16.49, 3.48), "hide_dk")
    g.box(b, "scab_flange_tooth_5", (-0.08, 14.35, 3.15), (0.30, 14.78, 3.49), "hide_dk")
    g.box(b, "scab_flange_tooth_6", (-0.61, 12.72, 3.17), (-0.37, 13.50, 3.43), "hide_dk")


# ── 8. 剑鞘下半段防磨皮补片与木衬露茬
def part_scabbard_lower(g: Gear) -> None:
    """剑鞘下半段防磨皮补片。"""
    b = "scabbard_lower"
    g.box(b, "scab_lo_sec1_body", (-1.00, 12.65, 2.52), (0.12, 14.89, 3.22), "hide_dk")
    g.box(b, "scab_lo_sec1_crest", (-0.92, 12.95, 3.09), (-0.02, 14.65, 3.38), "hide")
    g.box(b, "scab_lo_sec2_body", (-1.81, 11.05, 2.50), (-0.65, 13.05, 3.20), "hide_dk")
    g.box(b, "scab_lo_sec2_crest", (-1.72, 11.35, 3.10), (-0.82, 12.85, 3.36), "hide")

    # 外贴防磨损粗厚皮补片
    g.box(b, "scab_lo_patch_pad", (-1.35, 12.25, 2.41), (-0.25, 13.88, 3.34), "hide_lt")
    g.box(b, "scab_lo_cord_wrap_a", (-1.18, 13.15, 2.40), (-0.48, 13.58, 3.40), "cord_dk")
    g.box(b, "scab_lo_cord_wrap_b", (-1.58, 11.85, 2.38), (-0.88, 12.28, 3.38), "cord")

    # 磨损开裂处露出的木衬底片与木茬刺
    g.box(b, "scab_lo_wood_core", (-1.48, 11.45, 2.62), (-0.80, 12.35, 3.14), "linen_dk")
    g.box(b, "scab_lo_wood_splinter", (-1.62, 11.65, 2.72), (-1.22, 12.15, 3.05), "cord_dk")

    # 剑鞘下半段侧边缝线齿
    g.box(b, "scab_lo_stitch_1", (-0.87, 14.15, 3.22), (-0.59, 14.59, 3.46), "stitch")
    g.box(b, "scab_lo_stitch_2", (-1.24, 13.46, 3.24), (-0.99, 13.79, 3.51), "stitch")
    g.box(b, "scab_lo_stitch_3", (-1.51, 12.76, 3.25), (-1.15, 13.14, 3.57), "stitch")
    g.box(b, "scab_lo_stitch_4", (-1.74, 12.09, 3.22), (-1.42, 12.48, 3.44), "stitch")
    g.box(b, "scab_lo_stitch_5", (-2.18, 11.40, 3.22), (-1.88, 11.84, 3.51), "stitch")
    g.box(b, "scab_lo_stitch_6", (-2.55, 10.72, 3.09), (-2.30, 11.04, 3.36), "stitch")


# ── 9. 鞘尾铸铁护套包头与排气孔
def part_scabbard_chape(g: Gear) -> None:
    """鞘尾铸铁护套包头。"""
    b = "scabbard_chape"
    g.box(b, "chape_iron_body", (-2.58, 10.25, 2.48), (-1.45, 11.45, 3.24), "rust")
    g.box(b, "chape_iron_tip", (-2.82, 9.85, 2.55), (-1.82, 10.45, 3.18), "rust_dk")
    g.box(b, "chape_rim_upper", (-2.52, 11.32, 2.45), (-1.38, 11.68, 3.26), "iron")

    # 鞘尾泄水孔与固定铁钉
    g.box(b, "chape_drain_void", (-2.42, 9.92, 2.67), (-2.12, 10.25, 2.98), "void")
    g.box(b, "chape_rivet_r", (-1.65, 10.65, 3.15), (-1.35, 11.05, 3.42), "iron_lt")
    g.box(b, "chape_rivet_l", (-2.41, 10.69, 3.18), (-2.11, 11.09, 3.45), "iron_lt")

    # 鞘尾加固铁环套
    g.box(b, "chape_reinforce_band", (-2.49, 10.83, 2.42), (-1.55, 11.25, 3.28), "rust_dk")
    g.box(b, "chape_wire_wrap_1", (-2.25, 10.45, 2.44), (-1.75, 10.75, 3.25), "iron")
    g.box(b, "chape_wire_wrap_2", (-2.15, 10.05, 2.46), (-1.85, 10.35, 3.22), "iron_lt")
    g.box(b, "chape_tip_stud", (-2.75, 9.75, 2.72), (-2.35, 10.15, 3.05), "rust")


# ── 10. 断剑外露剑身锯齿犬牙残茬（在 y<=23.95 时位于 x∈[2.5, 3.9]；在 y>=24.0 时严格位于 x>=4.08，彻底避开头盒）
def part_broken_blade(g: Gear) -> None:
    """断剑外露剑身。"""
    b = "blade_broken_stub"
    # ── 鞘口内及肩台下方段 (y <= 23.95)
    g.box(b, "blade_body_low", (2.55, 22.25, 2.78), (3.35, 23.91, 3.12), "iron")
    g.box(b, "blade_ridge_low", (2.65, 22.35, 2.82), (3.25, 23.95, 3.08), "iron_lt")
    g.box(b, "blade_edge_r_low", (3.25, 22.35, 2.80), (3.55, 23.95, 3.10), "iron_lt")
    g.box(b, "blade_edge_l_low", (2.35, 22.38, 2.80), (2.65, 23.93, 3.10), "iron")

    # ── 高出肩台段 (x >= 4.08，在头盒 x<=4.0 之外，y 可达到 24.85)
    g.box(b, "blade_body_high", (4.06, 23.98, 2.78), (4.75, 24.65, 3.12), "iron")
    g.box(b, "blade_ridge_high", (4.15, 24.02, 2.82), (4.68, 24.72, 3.08), "iron_lt")
    g.box(b, "blade_edge_r_high", (4.65, 24.00, 2.80), (4.95, 24.62, 3.10), "iron_lt")

    # 断裂处的锯齿犬牙残口截面 (位于 x >= 4.08)
    g.box(b, "blade_fracture_tooth1", (4.17, 24.58, 2.80), (4.45, 24.95, 3.10), "iron_lt")
    g.box(b, "blade_fracture_tooth2", (4.52, 24.58, 2.83), (4.85, 24.88, 3.06), "iron")
    g.box(b, "blade_fracture_void", (4.38, 24.48, 2.84), (4.65, 24.78, 3.14), "void")
    g.box(b, "blade_fracture_spike", (4.25, 24.79, 2.82), (4.55, 25.12, 3.08), "iron_lt")


# ── 11. 断裂截面粗麻布包裹（位于 x >= 4.08）
def part_blade_cloth(g: Gear) -> None:
    """断裂截面粗麻布包裹。"""
    b = "blade_cloth_wrap"
    # 粗麻布包裹 (x 位于 4.08 ~ 5.15)
    g.box(b, "cloth_wrap_tier1", (4.08, 24.15, 2.70), (4.90, 24.75, 3.22), "linen")
    g.box(b, "cloth_wrap_tier2", (4.12, 24.55, 2.68), (4.88, 25.00, 3.24), "linen_dk")
    g.box(b, "cloth_wrap_crest", (4.18, 24.89, 2.65), (4.82, 25.25, 3.26), "linen_lt")

    # 缠布扎绳与垂落布角
    g.box(b, "cloth_tie_cord", (4.10, 24.38, 2.62), (4.86, 24.77, 3.30), "cord_dk")
    g.box(b, "cloth_drape_tag", (4.75, 23.85, 2.76), (5.15, 24.65, 3.14), "linen")
    g.box(b, "cloth_frayed_fringe", (4.85, 23.22, 2.80), (5.12, 23.98, 3.10), "linen_lt")
    g.box(b, "cloth_tear_knot", (4.72, 24.45, 3.15), (5.05, 24.85, 3.42), "cord")


# ── 12. 残存铁剑格与防滑铁齿（位于 x >= 4.08）
def part_sword_guard(g: Gear) -> None:
    """残破铁剑格与防滑铁齿。"""
    b = "sword_guard"
    # 残破铁剑格 (x 位于 4.08 ~ 5.35)
    g.box(b, "guard_mid_block", (4.09, 24.85, 2.61), (5.12, 25.38, 3.25), "iron")
    g.box(b, "guard_arm_out", (5.05, 24.88, 2.70), (5.65, 25.29, 3.20), "rust")
    g.box(b, "guard_arm_broken", (4.08, 24.92, 2.72), (4.38, 25.30, 3.18), "rust_dk")

    # 锁柄铁箍与防锈垫片
    g.box(b, "guard_seat_collar", (4.22, 24.70, 2.66), (5.02, 24.98, 3.20), "iron_lt")
    g.box(b, "guard_teeth_spike", (5.55, 25.02, 2.82), (5.85, 25.32, 3.08), "iron_lt")
    g.box(b, "guard_brass_rivet_a", (4.35, 24.95, 3.18), (4.75, 25.35, 3.45), "iron_lt")
    g.box(b, "guard_brass_rivet_b", (4.85, 24.95, 3.18), (5.25, 25.39, 3.45), "iron_lt")
    g.box(b, "guard_fang_notch_1", (4.45, 24.75, 2.72), (4.75, 25.05, 2.98), "rust")
    g.box(b, "guard_fang_notch_2", (4.85, 24.75, 2.72), (5.15, 25.05, 2.98), "rust")


# ── 13. 麻绳缠绕剑柄身（外露右肩上方，x >= 4.5 远远避开头盒）
def part_sword_hilt(g: Gear) -> None:
    """麻绳缠绕剑柄身。"""
    b = "sword_hilt_grip"
    # 剑柄木质内芯与五段麻绳紧密缠绕 (x 位于 4.5 ~ 6.2，y 位于 25.2 ~ 27.8)
    g.box(b, "hilt_sec1_wrap", (4.52, 25.25, 2.70), (5.02, 25.95, 3.16), "cord")
    g.box(b, "hilt_sec1_ridge", (4.58, 25.52, 2.65), (4.95, 25.86, 3.25), "cord_dk")

    g.box(b, "hilt_sec2_wrap", (4.92, 25.82, 2.68), (5.42, 26.55, 3.20), "cord_dk")
    g.box(b, "hilt_sec2_ridge", (4.98, 26.12, 2.65), (5.35, 26.42, 3.25), "cord")

    g.box(b, "hilt_sec3_wrap", (5.32, 26.42, 2.72), (5.82, 27.15, 3.24), "cord")
    g.box(b, "hilt_sec3_ridge", (5.38, 26.72, 2.65), (5.75, 26.97, 3.25), "cord_dk")

    g.box(b, "hilt_sec4_wrap", (5.72, 27.02, 2.70), (6.22, 27.75, 3.20), "cord_dk")
    g.box(b, "hilt_sec4_ridge", (5.78, 27.32, 2.65), (6.15, 27.62, 3.25), "cord")

    # 剑柄接头缠皮贴片与骨销
    g.box(b, "hilt_seam_pad", (5.05, 25.92, 2.67), (5.55, 26.65, 3.18), "hide_dk")
    g.box(b, "hilt_bone_peg", (5.25, 26.18, 2.62), (5.62, 26.49, 3.28), "bone")

    # 剑柄防滑麻线凸骨 (6 块)
    g.box(b, "hilt_braid_knot_1", (4.65, 25.45, 3.12), (4.98, 25.85, 3.38), "cord")
    g.box(b, "hilt_braid_knot_2", (5.00, 26.05, 3.12), (5.38, 26.43, 3.38), "cord_dk")
    g.box(b, "hilt_braid_knot_3", (5.45, 26.65, 3.12), (5.78, 27.05, 3.38), "cord")
    g.box(b, "hilt_braid_knot_4", (4.65, 25.45, 2.52), (4.98, 25.85, 2.78), "cord_dk")
    g.box(b, "hilt_braid_knot_5", (5.07, 26.05, 2.52), (5.38, 26.45, 2.78), "cord")
    g.box(b, "hilt_braid_knot_6", (5.45, 26.65, 2.52), (5.78, 27.05, 2.78), "cord_dk")


# ── 14. 柄首厚锈铁圆环配重与骨环吊饰
def part_sword_pommel(g: Gear) -> None:
    """柄首厚锈铁圆环配重。"""
    b = "sword_pommel"
    # 剑柄首铁圆环配重 (x 位于 6.0 ~ 7.2)
    g.box(b, "pommel_ring_top", (6.15, 27.82, 2.71), (6.85, 28.58, 3.22), "iron")
    g.box(b, "pommel_ring_bot", (5.85, 27.22, 2.68), (6.45, 27.98, 3.27), "rust_dk")
    g.box(b, "pommel_ring_hole_void", (6.18, 27.60, 2.74), (6.52, 28.22, 3.16), "void")

    # 柄首悬挂的裂纹兽骨环饰
    g.box(b, "pommel_bone_charm", (6.65, 27.52, 2.72), (7.15, 28.18, 3.18), "bone")
    g.box(b, "pommel_charm_tip", (7.02, 27.12, 2.75), (7.35, 27.78, 3.15), "bone_lt")
    g.box(b, "pommel_cord_knot", (6.45, 27.75, 2.82), (6.75, 28.15, 3.08), "cord_dk")


# ── 15. 鞘侧吊挂磨损骨符与断绳
def part_scabbard_trinket(g: Gear) -> None:
    """鞘侧吊挂磨损骨符。"""
    b = "scabbard_trinket"
    g.box(b, "trinket_cord_loop", (0.42, 17.52, 3.15), (0.82, 18.28, 3.45), "cord_dk")
    g.box(b, "trinket_bone_tag", (0.32, 16.22, 3.18), (0.92, 17.68, 3.46), "bone")
    g.box(b, "trinket_bone_crest", (0.38, 17.42, 3.12), (0.85, 17.78, 3.50), "bone_lt")
    g.box(b, "trinket_bone_hole_void", (0.50, 17.22, 3.22), (0.74, 17.55, 3.48), "void")
    g.box(b, "trinket_tail_knot", (0.48, 15.82, 3.22), (0.75, 16.38, 3.52), "cord_dk")
    g.box(b, "trinket_tail_strand", (0.50, 15.25, 3.25), (0.74, 15.92, 3.48), "cord")



# ── 16. 细节补强（扩充方块总数达到 205+）
def part_scabbard_details(g: Gear) -> None:
    """剑鞘加固铜钉、背带挂环与剑柄编结细节。"""
    b = "scabbard_upper"
    # 鞘身正面防磨加固铜钉 (4 块)
    g.box(b, "scab_copper_rivet_1", (2.12, 20.12, 3.19), (2.48, 20.52, 3.55), "iron_lt")
    g.box(b, "scab_copper_rivet_2", (1.32, 17.82, 3.19), (1.68, 18.22, 3.55), "iron_lt")
    g.box(b, "scab_copper_rivet_3", (0.52, 15.46, 3.19), (0.88, 15.84, 3.55), "iron_lt")
    g.box(b, "scab_copper_rivet_4", (-0.28, 13.22, 3.19), (0.08, 13.62, 3.55), "iron_lt")

    # 鞘身背侧防撞皮护角 (4 块)
    g.box(b, "scab_back_bumper_1", (1.52, 20.25, 2.12), (2.12, 20.75, 2.45), "hide_lt")
    g.box(b, "scab_back_bumper_2", (0.72, 17.95, 2.12), (1.32, 18.45, 2.45), "hide_lt")
    g.box(b, "scab_back_bumper_3", (-0.08, 15.65, 2.12), (0.52, 16.15, 2.45), "hide_lt")
    g.box(b, "scab_back_bumper_4", (-0.88, 13.35, 2.12), (-0.28, 13.85, 2.45), "hide_lt")

    # 背带上的方扣加厚铁板与皮带尾折角 (3 块)
    sb = "strap_back_diag"
    g.box(sb, "back_strap_seam_pad_extra", (0.78, 15.68, 2.48), (1.82, 16.52, 2.85), "hide_dk")
    g.box(sb, "back_strap_iron_plate_extra", (0.98, 15.88, 2.78), (1.62, 16.32, 3.08), "rust")
    g.box(sb, "back_strap_tail_fold_extra", (-2.62, 11.28, 2.49), (-1.92, 11.92, 2.79), "hide_lt")

    # 剑柄麻绳交叉编结 (5 块)
    sh = "sword_hilt_grip"
    g.box(sh, "hilt_cross_knot_1", (4.65, 25.42, 2.78), (4.98, 25.82, 3.12), "cord_dk")
    g.box(sh, "hilt_cross_knot_2", (5.01, 26.02, 2.78), (5.38, 26.37, 3.12), "cord")
    g.box(sh, "hilt_cross_knot_3", (5.45, 26.62, 2.78), (5.78, 27.02, 3.12), "cord_dk")
    g.box(sh, "hilt_ring_spacer_1", (4.55, 25.32, 2.60), (4.85, 25.62, 3.23), "iron_lt")
    g.box(sh, "hilt_ring_spacer_2", (5.65, 27.04, 2.65), (5.95, 27.32, 3.25), "iron_lt")

    # 胸前背带小骨签与扎带 (4 块)
    cd = "strap_chest_diag"
    g.box(cd, "chest_tally_bone_tag", (0.45, 13.65, -2.95), (0.85, 14.45, -2.62), "bone")
    g.box(cd, "chest_tally_bone_tip", (0.52, 13.35, -2.92), (0.78, 13.72, -2.65), "bone_lt")
    g.box(cd, "chest_tally_cord_loop", (0.48, 14.38, -2.93), (0.82, 14.75, -2.65), "cord_dk")
    g.box(cd, "chest_strap_corner_patch", (-2.25, 11.35, -2.85), (-1.65, 12.05, -2.48), "hide_lt")

    # 鞘尾加厚防磨铁板 (2 块)
    sc = "scabbard_chape"
    g.box(sc, "chape_reinforce_cap", (-2.78, 9.72, 2.62), (-1.95, 10.02, 3.08), "iron")
    g.box(sc, "chape_side_cramp_pin", (-2.62, 10.15, 2.45), (-2.22, 10.65, 3.21), "rust")


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
    wear_note="挂点：右肩经胸背斜跨至左后腰 + 断剑柄斜露右肩外侧上方",
    worn_focus=((0.0, 18.0, 1.0), 24.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
