#!/usr/bin/env python3
"""枯草裹脚（ku_cao_guo_jiao）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工：厚草鞋底露草茎、脚趾前段露出、脚面多层交叉裹布与脚踝双圈皮绳打结垂穗。

任务卡：末土装备 #17（.task-wasteland-gear.md）。
挂点：双脚（脚底 y∈[-0.85, 0.02]，脚趾与脚面 y∈[0.0, 2.8]，脚踝 y∈[2.2, 4.2]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  玩家双腿：右腿 x∈[0,4], 左腿 x∈[-4,0], y∈[0,12], z∈[-2,2]
  门禁 clearance：任何方块不得扎进双腿（咬入 ≤ 0.05px；在 y∈[0,12] 且 z∈[-2,2] 内不可穿透）。

外形：
  打破方盒感：
  - 底下是厚枯草编织鞋底，鞋底边缘参差露出 8 根粗草茎与编织翘头；
  - 前端草底托住脚掌，露出前段灰白脚趾头（大趾、二趾、三趾与外侧趾群）；
  - 脚面由多层粗布条斜向与 X 形交叉缠裹，有厚度与层次感，带翘起散开布角；
  - 脚踝周围由两道粗皮绳环绕扎紧，侧面打死结，垂下两截带流苏的绳穗；
  - 右脚背带穿有一枚梭形兽骨扣，左鞋外侧草耳下挂一枚磨损骨牙坠与多颗骨珠。
  总方块数达到 207 块，按挂点分骨 15 骨。
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "straw":        (168, 142, 96),    # 枯草编织主色
    "straw_lt":     (194, 172, 118),   # 草编亮面/凸起编草
    "straw_dk":     (118, 96, 62),     # 草编阴面/底缝
    "stem":         (138, 114, 72),    # 边缘露出的粗草茎
    "linen":        (142, 126, 98),    # 裹脚粗布
    "linen_lt":     (172, 154, 122),   # 粗布亮面/折痕
    "linen_dk":     (108, 92, 70),     # 粗布暗面
    "toe":          (136, 128, 122),   # 露出的脚趾灰肤色
    "toe_dk":       (112, 104, 98),    # 趾缝阴影
    "patch":        (120, 88, 64),     # 兽皮耐磨补片
    "cord":         (150, 126, 84),    # 扎踝皮绳
    "cord_dk":      (104, 84, 54),     # 绳结深色
    "stitch":       (188, 176, 148),   # 粗缝线
    "bone":         (206, 194, 164),   # 骨鼻扣与骨坠
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨阴影
    "void":         (28, 24, 22),      # 草耳穿绳暗孔
}

SOFT = ("straw", "straw_lt", "straw_dk", "stem", "linen", "linen_lt", "linen_dk", "patch", "cord", "cord_dk", "stitch", "toe", "toe_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("foot_r_mount", "root", (2.0, 0.0, 0.0), "右脚安装基准骨"),
    ("sole_main_r", "foot_r_mount", (2.0, -0.4, -0.6), "右脚草鞋底与草茎"),
    ("toes_r", "foot_r_mount", (2.0, 0.3, -3.2), "右脚露出的脚趾头"),
    ("heel_guard_r", "foot_r_mount", (2.0, 1.2, 2.1), "右脚后跟草编包根"),
    ("instep_wrap_r", "foot_r_mount", (2.0, 1.4, -1.8), "右脚背粗布交叉裹脚层"),
    ("ankle_strap_r", "foot_r_mount", (2.0, 2.8, 0.0), "右脚踝双圈皮绳扎带"),
    ("shoe_bone_r", "instep_wrap_r", (2.0, 1.6, -2.6), "右脚背兽骨鞋扣"),
    ("foot_l_mount", "root", (-2.0, 0.0, 0.0), "左脚安装基准骨"),
    ("sole_main_l", "foot_l_mount", (-2.0, -0.4, -0.6), "左脚草鞋底与草茎"),
    ("toes_l", "foot_l_mount", (-2.0, 0.3, -3.2), "左脚露出的脚趾头"),
    ("heel_guard_l", "foot_l_mount", (-2.0, 1.2, 2.1), "左脚后跟草编包根"),
    ("instep_wrap_l", "foot_l_mount", (-2.0, 1.4, -1.8), "左脚背粗布交叉裹脚层"),
    ("ankle_strap_l", "foot_l_mount", (-2.0, 2.8, 0.0), "左脚踝双圈皮绳扎带"),
    ("hang_tooth_l", "foot_l_mount", (-4.2, 1.8, 0.0), "左脚外草耳挂骨牙坠"),
]

ASYM_BONES = frozenset([
    "shoe_bone_r", "hang_tooth_l",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_sandal_sole(g: Gear) -> None:
    """厚草鞋底与边缘露出的参差粗草茎：
    底板在 y ∈ [-0.85, 0.02]，外伸前掌在 z ∈ [-3.6, -1.95]。
    """
    b = "sole_main"
    # 后半掌与中掌草底板 (y: -0.78 .. 0.02, z: -1.95 .. 2.05)
    g.pair(b, "sole_base_rear", (0.08, -0.78, -1.95), (4.02, 0.02, 2.05), "straw", sided=True)
    g.pair(b, "sole_bottom_strip", (0.35, -0.88, -1.85), (3.75, -0.62, 1.95), "straw_dk", sided=True)
    g.pair(b, "sole_braid_1", (0.5, -0.82, -1.75), (1.5, -0.55, 1.85), "straw_lt", sided=True)
    g.pair(b, "sole_braid_2", (2.6, -0.82, -1.75), (3.6, -0.55, 1.85), "straw_lt", sided=True)
    g.pair(b, "sole_braid_mid", (1.65, -0.82, -1.65), (2.45, -0.58, 1.75), "straw_dk", sided=True)

    # 鞋底编织格
    g.pair(b, "sole_grid_1", (0.45, -0.85, -1.5), (1.45, -0.58, -0.9), "straw_dk", sided=True)
    g.pair(b, "sole_grid_2", (0.45, -0.85, -0.5), (1.45, -0.58, 0.1), "straw", sided=True)
    g.pair(b, "sole_grid_3", (0.45, -0.85, 0.5), (1.45, -0.58, 1.1), "straw_dk", sided=True)
    g.pair(b, "sole_grid_4", (2.65, -0.85, -1.5), (3.65, -0.58, -0.9), "straw_dk", sided=True)
    g.pair(b, "sole_grid_5", (2.65, -0.85, -0.5), (3.65, -0.58, 0.1), "straw", sided=True)
    g.pair(b, "sole_grid_6", (2.65, -0.85, 0.5), (3.65, -0.58, 1.1), "straw_dk", sided=True)
    g.pair(b, "sole_pad_heel", (0.85, -0.72, 1.35), (3.25, -0.25, 1.98), "straw", sided=True)
    g.pair(b, "sole_pad_front", (0.85, -0.72, -3.15), (3.25, -0.25, -2.55), "straw_dk", sided=True)
    g.pair(b, "instep_lace_1", (0.68, 1.78, -2.86), (1.58, 2.12, -2.40), "cord", sided=True)
    g.pair(b, "instep_lace_2", (2.42, 1.78, -2.86), (3.32, 2.12, -2.40), "cord_dk", sided=True)
    g.pair(b, "heel_stay_lo", (0.65, 0.85, 2.25), (3.42, 1.25, 2.48), "cord_dk", sided=True)
    g.pair(b, "heel_stay_hi", (0.65, 1.52, 2.25), (3.42, 1.92, 2.48), "cord", sided=True)
    g.pair(b, "toe_wrap_tooth1", (0.48, 0.68, -3.42), (1.18, 0.98, -2.96), "linen_dk", sided=True)
    g.pair(b, "toe_wrap_tooth2", (2.85, 0.68, -3.42), (3.55, 0.98, -2.96), "linen_lt", sided=True)

    # 外侧鞋底边缘编包边
    g.pair(b, "sole_side_rim", (4.04, -0.75, -1.88), (4.42, 0.35, 1.88), "straw_dk", sided=True)
    g.pair(b, "sole_side_cord", (4.12, -0.45, -1.65), (4.48, 0.18, 1.65), "cord", sided=True)
    g.pair(b, "sole_side_rib", (4.16, -0.68, -1.25), (4.52, -0.06, 1.25), "straw_lt", sided=True)

    # 前脚掌伸出部底板 (z: -3.52 .. -1.98, y: -0.78 .. 0.28)
    g.pair(b, "sole_front_slab", (0.08, -0.78, -3.52), (4.02, 0.28, -1.98), "straw", sided=True)
    g.pair(b, "sole_front_strip", (0.4, -0.86, -3.48), (3.7, -0.56, -2.02), "straw_dk", sided=True)
    # 前端上翻草包尖
    g.pair(b, "sole_toe_up", (0.45, 0.18, -3.64), (3.65, 0.85, -3.32), "straw_dk", sided=True)
    g.pair(b, "sole_toe_crest", (0.8, 0.38, -3.68), (3.3, 0.78, -3.36), "straw_lt", sided=True)

    # ── 边缘露出的参差粗草茎
    g.pair(b, "stem_out_1", (4.38, -0.42, -1.62), (4.70, -0.16, -1.15), "stem", sided=True)
    g.pair(b, "stem_out_2", (4.38, -0.36, -0.75), (4.72, -0.10, -0.15), "stem", sided=True)
    g.pair(b, "stem_out_3", (4.38, -0.42, 0.25), (4.70, -0.16, 0.85), "stem", sided=True)
    g.pair(b, "stem_out_4", (4.38, -0.36, 1.15), (4.72, -0.10, 1.68), "stem", sided=True)
    g.pair(b, "stem_out_5", (4.36, -0.42, -1.12), (4.74, -0.14, -0.58), "stem", sided=True)
    g.pair(b, "stem_out_6", (4.36, -0.40, 0.58), (4.74, -0.12, 1.12), "stem", sided=True)

    g.pair(b, "stem_front_1", (0.55, 0.12, -3.78), (1.05, 0.42, -3.45), "stem", sided=True)
    g.pair(b, "stem_front_2", (1.45, 0.12, -3.78), (1.95, 0.42, -3.45), "stem", sided=True)
    g.pair(b, "stem_front_3", (2.35, 0.12, -3.78), (2.85, 0.42, -3.45), "stem", sided=True)
    g.pair(b, "stem_front_4", (3.05, 0.12, -3.78), (3.55, 0.42, -3.45), "stem", sided=True)


def part_exposed_toes(g: Gear) -> None:
    """露出的脚趾前段：位于前端 z ∈ [-3.45, -3.02]，y ∈ [0.28, 0.72]。"""
    b = "toes"
    # 大脚趾
    g.pair(b, "toe_big", (0.55, 0.28, -3.45), (1.45, 0.68, -3.02), "toe", sided=True)
    g.pair(b, "toe_big_nail", (0.75, 0.48, -3.48), (1.25, 0.72, -3.22), "toe_dk", sided=True)
    g.pair(b, "toe_big_joint", (0.68, 0.32, -3.26), (1.32, 0.62, -3.05), "toe_dk", sided=True)

    # 二脚趾
    g.pair(b, "toe_second", (1.55, 0.28, -3.42), (2.25, 0.65, -3.05), "toe", sided=True)
    g.pair(b, "toe_second_nail", (1.7, 0.45, -3.45), (2.1, 0.68, -3.25), "toe_dk", sided=True)
    g.pair(b, "toe_second_joint", (1.62, 0.32, -3.32), (2.18, 0.58, -3.08), "toe_dk", sided=True)

    # 三脚趾
    g.pair(b, "toe_third", (2.35, 0.28, -3.38), (2.88, 0.62, -3.05), "toe", sided=True)
    g.pair(b, "toe_third_nail", (2.44, 0.42, -3.45), (2.78, 0.65, -3.21), "toe_dk", sided=True)

    # 四趾与小趾群
    g.pair(b, "toe_outer_main", (2.95, 0.28, -3.32), (3.65, 0.58, -3.05), "toe", sided=True)
    g.pair(b, "toe_outer_flank", (3.08, 0.38, -3.36), (3.52, 0.62, -3.15), "toe_dk", sided=True)


def part_heel_guard(g: Gear) -> None:
    """脚后跟草编包根与后跟加固带：紧贴脚后部 z ∈ [2.08, 2.45]，y ∈ [0.05, 2.4]。"""
    b = "heel_guard"
    g.pair(b, "heel_back_main", (0.08, 0.05, 2.08), (4.02, 2.05, 2.38), "straw", sided=True)
    g.pair(b, "heel_back_crest", (0.6, 0.35, 2.22), (3.5, 1.82, 2.46), "straw_lt", sided=True)
    g.pair(b, "heel_rim_top", (0.3, 1.95, 2.12), (3.8, 2.38, 2.42), "straw_dk", sided=True)
    g.pair(b, "heel_pleat_v1", (1.0, 0.45, 2.15), (1.6, 1.75, 2.42), "straw", sided=True)
    g.pair(b, "heel_pleat_v2", (2.4, 0.45, 2.15), (3.0, 1.75, 2.42), "straw_dk", sided=True)

    g.pair(b, "heel_side_out", (4.05, 0.35, 0.8), (4.38, 2.05, 2.15), "straw_dk", sided=True)
    g.pair(b, "heel_side_cord", (4.12, 0.75, 1.0), (4.44, 1.65, 2.05), "cord", sided=True)
    g.pair(b, "heel_side_pleat", (4.08, 0.55, 1.3), (4.42, 1.85, 1.85), "straw_lt", sided=True)
    g.pair(b, "heel_cord_rib1", (0.55, 0.65, 2.18), (1.15, 1.88, 2.44), "straw_lt", sided=True)
    g.pair(b, "heel_cord_rib2", (2.85, 0.65, 2.18), (3.45, 1.88, 2.44), "straw_dk", sided=True)


def part_instep_wraps(g: Gear) -> None:
    """脚面粗布多层交叉裹裹：各层在 Z、Y 轴均有独立层级，避免共面。"""
    b = "instep_wrap"
    # ── 第 1 交叉布层 (y: 0.65 .. 1.42, z: -3.02 .. -2.08)
    g.pair(b, "wrap_cross1_base", (0.12, 0.65, -3.02), (3.98, 1.42, -2.08), "linen", sided=True)
    g.pair(b, "wrap_cross1_fold", (0.45, 0.78, -2.94), (3.65, 1.38, -2.46), "linen_lt", sided=True)
    g.pair(b, "wrap_cross1_seam", (1.2, 0.88, -2.93), (2.8, 1.12, -2.35), "stitch", sided=True)

    # ── 第 2 交叉布层 (y: 1.25 .. 2.05, z: -2.85 .. -2.15)
    g.pair(b, "wrap_cross2_base", (0.15, 1.22, -2.83), (3.95, 2.05, -2.15), "linen_dk", sided=True)
    g.pair(b, "wrap_cross2_crest", (0.6, 1.42, -2.88), (3.5, 1.96, -2.32), "linen", sided=True)
    g.pair(b, "wrap_cross2_pleat", (0.8, 1.58, -2.92), (2.2, 2.15, -2.42), "linen_lt", sided=True)
    g.pair(b, "wrap_cross2_accent", (2.35, 1.40, -2.82), (3.35, 1.92, -2.36), "linen_dk", sided=True)

    # ── 第 3 脚背延伸层 (y: 1.95 .. 2.65, z: -2.78 .. -2.16)
    g.pair(b, "wrap_instep_slope", (0.18, 1.92, -2.78), (3.92, 2.58, -2.16), "linen", sided=True)
    g.pair(b, "wrap_instep_crest", (0.65, 2.02, -2.82), (3.45, 2.55, -2.28), "linen_lt", sided=True)
    g.pair(b, "wrap_instep_shade", (0.28, 1.98, -2.72), (3.82, 2.32, -2.18), "linen_dk", sided=True)
    g.pair(b, "wrap_instep_tie", (1.2, 2.15, -2.86), (2.8, 2.48, -2.32), "cord", sided=True)

    # ── 第 4 层斜向交错布与布舌
    g.pair(b, "wrap_cross3_band", (0.45, 1.46, -2.94), (3.65, 1.85, -2.44), "linen", sided=True)
    g.pair(b, "wrap_cross3_seam", (0.85, 1.50, -2.96), (3.25, 1.80, -2.48), "stitch", sided=True)
    g.pair(b, "wrap_flap_hi1", (0.45, 2.25, -2.85), (1.35, 2.75, -2.35), "linen_lt", sided=True)
    g.pair(b, "wrap_flap_hi2", (2.75, 2.25, -2.85), (3.65, 2.75, -2.35), "linen_dk", sided=True)
    g.pair(b, "wrap_cross4_diag", (0.88, 0.95, -2.86), (2.82, 1.52, -2.40), "linen_lt", sided=True)

    # ── 散开翘起的粗布角
    g.pair(b, "wrap_flap_out", (3.25, 1.35, -2.90), (4.15, 1.86, -2.35), "linen_lt", sided=True)
    g.pair(b, "wrap_flap_in", (0.32, 1.45, -2.82), (1.18, 2.02, -2.30), "linen_dk", sided=True)
    g.pair(b, "wrap_patch_toe", (2.65, 0.72, -2.92), (3.62, 1.32, -2.42), "patch", sided=True)
    g.pair(b, "wrap_patch_s1", (2.75, 0.82, -2.96), (3.55, 1.15, -2.48), "stitch", sided=True)
    g.pair(b, "wrap_tongue_out", (4.05, 0.55, -2.4), (4.38, 1.75, -0.6), "linen_dk", sided=True)


def part_ankle_straps(g: Gear) -> None:
    """脚踝双圈皮绳扎带与打结垂穗（前后与外侧环抱相连，绝不悬空）。"""
    b = "ankle_strap"
    # 下圈皮绳扎带
    g.pair(b, "ankle_cord_lo_f", (0.05, 2.65, -2.45), (4.25, 3.15, -2.16), "cord", sided=True)
    g.pair(b, "ankle_cord_lo_out", (4.05, 2.62, -2.28), (4.44, 3.18, 2.28), "cord", sided=True)
    g.pair(b, "ankle_cord_lo_b", (0.05, 2.65, 2.16), (4.25, 3.15, 2.45), "cord", sided=True)
    g.pair(b, "ankle_cord_knot_b", (1.65, 2.62, 2.22), (2.35, 3.22, 2.58), "cord_dk", sided=True)

    # 上圈皮绳扎带
    g.pair(b, "ankle_cord_hi_f", (0.05, 3.55, -2.42), (4.25, 3.95, -2.16), "cord_dk", sided=True)
    g.pair(b, "ankle_cord_hi_out", (4.05, 3.52, -2.28), (4.44, 3.98, 2.28), "cord_dk", sided=True)
    g.pair(b, "ankle_cord_hi_b", (0.05, 3.55, 2.16), (4.25, 3.95, 2.42), "cord_dk", sided=True)

    # 外侧草编提耳圈
    g.pair(b, "sandal_ear_loop", (4.12, 1.65, -0.42), (4.56, 2.95, 0.42), "straw_dk", sided=True)
    g.pair(b, "sandal_ear_hole", (4.24, 1.95, -0.24), (4.58, 2.65, 0.24), "void", sided=True)
    g.pair(b, "ankle_stay_cord1", (4.15, 2.45, -1.5), (4.48, 3.75, -1.1), "cord_dk", sided=True)
    g.pair(b, "ankle_stay_cord2", (4.15, 2.45, 1.1), (4.48, 3.75, 1.5), "cord_dk", sided=True)

    # 绳结与垂穗
    g.pair(b, "ankle_knot_main", (1.6, 2.58, -2.62), (2.4, 3.38, -2.24), "cord_dk", sided=True)
    g.pair(b, "ankle_knot_core", (1.85, 2.75, -2.68), (2.25, 3.25, -2.26), "cord", sided=True)
    g.pair(b, "ankle_tail_1", (1.75, 1.55, -2.56), (2.15, 2.62, -2.25), "cord", sided=True)
    g.pair(b, "ankle_tail_2", (2.22, 1.65, -2.56), (2.58, 2.62, -2.25), "cord_dk", sided=True)
    g.pair(b, "ankle_tail_tassel1", (1.8, 1.05, -2.54), (2.12, 1.58, -2.28), "cord_dk", sided=True)
    g.pair(b, "ankle_tail_tassel2", (2.25, 1.15, -2.54), (2.55, 1.68, -2.28), "cord", sided=True)
    g.pair(b, "instep_tie_cross", (0.85, 2.35, -2.80), (2.45, 2.72, -2.34), "cord_dk", sided=True)


def part_shoe_details(g: Gear) -> None:
    """细部特征：右脚背上的梭形兽骨扣与左脚外耳挂小骨牙+多颗骨珠。"""
    sb = "shoe_bone_r"
    g.box(sb, "bone_buckle_main", (1.52, 1.26, -2.99), (2.48, 1.86, -2.24), "bone")
    g.box(sb, "bone_buckle_crest", (1.72, 1.45, -3.05), (2.28, 1.74, -2.22), "bone_lt")
    g.box(sb, "bone_buckle_pin", (1.82, 1.15, -2.90), (2.18, 2.02, -2.34), "bone_dk")
    g.box(sb, "bone_stud_1", (1.15, 1.35, -2.85), (1.45, 1.75, -2.45), "bone_lt")
    g.box(sb, "bone_stud_2", (2.55, 1.35, -2.85), (2.85, 1.75, -2.45), "bone")
    g.box(sb, "bone_stud_3", (1.85, 0.85, -2.85), (2.15, 1.18, -2.48), "bone_dk")
    g.box(sb, "bone_stud_4", (1.82, 2.05, -2.88), (2.12, 2.38, -2.48), "bone")

    ht = "hang_tooth_l"
    g.box(ht, "tooth_cord", (-4.65, 2.82, -0.18), (-4.28, 3.55, 0.18), "cord_dk")
    g.box(ht, "tooth_bead_3", (-4.82, 2.85, -0.32), (-4.22, 3.52, 0.32), "bone")
    g.box(ht, "tooth_bead_1", (-4.86, 2.15, -0.36), (-4.20, 2.82, 0.36), "bone_lt")
    g.box(ht, "tooth_bead_2", (-4.86, 1.45, -0.36), (-4.20, 2.12, 0.36), "bone")
    g.box(ht, "tooth_head", (-4.88, 0.95, -0.38), (-4.22, 1.42, 0.38), "bone_dk")
    g.box(ht, "tooth_body", (-4.85, 0.35, -0.35), (-4.25, 0.95, 0.35), "bone")
    g.box(ht, "tooth_tip", (-4.78, -0.08, -0.32), (-4.32, 0.32, 0.32), "bone_lt")
    g.box(ht, "tooth_knot_end", (-4.65, -0.35, -0.15), (-4.35, -0.09, 0.15), "cord_dk")


PART_FUNCS = {
    "sandal_sole": part_sandal_sole,
    "exposed_toes": part_exposed_toes,
    "heel_guard": part_heel_guard,
    "instep_wraps": part_instep_wraps,
    "ankle_straps": part_ankle_straps,
    "shoe_details": part_shoe_details,
}

SPEC = GearSpec(
    gear_id="ku_cao_guo_jiao",
    model_name="KuCaoGuoJiao",
    title="枯草裹脚 / ku_cao_guo_jiao",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：双脚（y∈[-0.85, 4.2]，右脚 x∈[0,4], 左脚 x∈[-4,0]）",
    worn_focus=((0.0, 1.5, 0.0), 16.0),
    texture_overrides={"stitch": "gear_stitch_lt"},  # 画笔库里 gear_stitch 已是腰挂件的深线，这里用浅粗线
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
