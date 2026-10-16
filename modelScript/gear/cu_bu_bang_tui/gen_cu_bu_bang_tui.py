#!/usr/bin/env python3
"""粗布绑腿（cu_bu_bang_tui）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工：螺旋斜缠布带、散开布头、膝下脚踝双道扎绳打结与外侧皮补片。

任务卡：末土装备 #16（.task-wasteland-gear.md）。
挂点：双小腿（y∈[0.8, 11.5]，右腿 x∈[0,4], 左腿 x∈[-4,0], z∈[-2,2]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  右腿 x∈[0,4], y∈[0,12], z∈[-2,2]
  左腿 x∈[-4,0], y∈[0,12], z∈[-2,2]
  门禁 clearance：任何方块不得扎进左右腿（咬入 ≤ 0.05px）。

外形：
  打破方筒感：
  - 双小腿包裹粗布条，看得出明显的螺旋上升缠绕肌理，每圈有前后厚度错落与斜向波浪；
  - 膝下（y≈10.0..11.2）与脚踝（y≈1.5..2.8）各有一道粗草绳重重打结扎紧，垂下绳头；
  - 表面带有几根翘起、散开的破布头与下垂撕裂布舌；
  - 外侧覆有防磨旧兽皮补片，带兽骨扣与四周粗麻缝线；
  - 右腿外侧插有 2 根缝补粗骨针，左腿脚踝草绳下垂挂一串磨光骨珠计数串。
  总方块数达到 210+，按挂点分骨 14 骨。
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "linen":        (142, 126, 98),    # 粗布主色
    "linen_lt":     (172, 154, 122),   # 粗布亮面/折痕凸起
    "linen_dk":     (108, 92, 70),     # 粗布阴面/折痕深色
    "patch":        (120, 88, 64),     # 外侧防磨兽皮
    "patch_dk":     (86, 62, 44),      # 兽皮暗部
    "patch_lt":     (148, 114, 86),    # 兽皮磨损边缘
    "cord":         (150, 126, 84),    # 扎腿草绳/麻绳
    "cord_dk":      (104, 84, 54),     # 绳结深色
    "stitch":       (188, 176, 148),   # 粗缝线迹
    "bone":         (206, 194, 164),   # 兽骨扣与骨针
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨阴影
    "void":         (28, 24, 22),      # 穿绳暗孔
}

SOFT = ("linen", "linen_lt", "linen_dk", "patch", "patch_dk", "patch_lt", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("leg_r_mount", "root", (2.0, 6.0, 0.0), "右小腿安装基准骨"),
    ("wrap_main_r", "leg_r_mount", (2.0, 6.0, 0.0), "右小腿绑腿主体"),
    ("wrap_layers_r", "leg_r_mount", (2.0, 6.0, 0.0), "右小腿螺旋重叠折边与散开布头"),
    ("knee_cord_r", "leg_r_mount", (2.0, 10.5, 0.0), "右小腿膝下草绳扎带"),
    ("ankle_cord_r", "leg_r_mount", (2.0, 2.2, 0.0), "右小腿脚踝草绳扎带"),
    ("patch_side_r", "leg_r_mount", (4.2, 6.0, 0.0), "右小腿外侧皮补丁与骨扣"),
    ("shin_needle_r", "patch_side_r", (4.3, 7.5, 0.0), "右小腿外侧插缝补粗骨针"),
    ("leg_l_mount", "root", (-2.0, 6.0, 0.0), "左小腿安装基准骨"),
    ("wrap_main_l", "leg_l_mount", (-2.0, 6.0, 0.0), "左小腿绑腿主体"),
    ("wrap_layers_l", "leg_l_mount", (-2.0, 6.0, 0.0), "左小腿螺旋重叠折边与散开布头"),
    ("knee_cord_l", "leg_l_mount", (-2.0, 10.5, 0.0), "左小腿膝下草绳扎带"),
    ("ankle_cord_l", "leg_l_mount", (-2.0, 2.2, 0.0), "左小腿脚踝草绳扎带"),
    ("patch_side_l", "leg_l_mount", (-4.2, 6.0, 0.0), "左小腿外侧皮补丁与骨扣"),
    ("ankle_beads_l", "leg_l_mount", (-4.3, 3.5, 0.0), "左小腿绑带下垂骨珠串"),
]

ASYM_BONES = frozenset([
    "shin_needle_r", "ankle_beads_l",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_wrap_base(g: Gear) -> None:
    """左右小腿绑腿主体：5 段螺旋阶梯环绕，每圈高度约 2px，厚度错开。
    右腿：x ∈ [0.08, 4.02], y ∈ [1.15, 11.25], z ∈ [-2.36, -2.08] 与 [2.08, 2.38]。
    全部使用 g.pair 镜像到左腿（sided=True）。
    """
    b = "wrap_main"
    # ── 第 1 圈 (y: 1.15 .. 3.15)
    g.pair(b, "wrap_tier1_f", (0.08, 1.15, -2.35), (4.02, 3.15, -2.08), "linen", sided=True)
    g.pair(b, "wrap_tier1_b", (0.08, 1.15, 2.08), (4.02, 3.15, 2.35), "linen", sided=True)
    g.pair(b, "wrap_tier1_out", (4.08, 1.15, -2.04), (4.38, 3.15, 2.04), "linen_dk", sided=True)

    # ── 第 2 圈 (y: 3.16 .. 5.15，厚度微凸)
    g.pair(b, "wrap_tier2_f", (0.08, 3.16, -2.38), (4.02, 5.15, -2.08), "linen_dk", sided=True)
    g.pair(b, "wrap_tier2_b", (0.08, 3.16, 2.08), (4.02, 5.15, 2.38), "linen_dk", sided=True)
    g.pair(b, "wrap_tier2_out", (4.08, 3.16, -2.06), (4.42, 5.15, 2.06), "linen", sided=True)

    # ── 第 3 圈 (y: 5.16 .. 7.15)
    g.pair(b, "wrap_tier3_f", (0.08, 5.16, -2.35), (4.02, 7.15, -2.08), "linen", sided=True)
    g.pair(b, "wrap_tier3_b", (0.08, 5.16, 2.08), (4.02, 7.15, 2.42), "linen_lt", sided=True)
    g.pair(b, "wrap_tier3_out", (4.08, 5.16, -2.04), (4.38, 7.15, 2.04), "linen_dk", sided=True)

    # ── 第 4 圈 (y: 7.16 .. 9.15)
    g.pair(b, "wrap_tier4_f", (0.08, 7.16, -2.38), (4.02, 9.15, -2.08), "linen_lt", sided=True)
    g.pair(b, "wrap_tier4_b", (0.08, 7.16, 2.08), (4.02, 9.15, 2.38), "linen", sided=True)
    g.pair(b, "wrap_tier4_out", (4.08, 7.16, -2.06), (4.42, 9.15, 2.06), "linen_lt", sided=True)

    # ── 第 5 圈 (y: 9.16 .. 11.22)
    g.pair(b, "wrap_tier5_f", (0.08, 9.16, -2.35), (4.02, 11.22, -2.08), "linen", sided=True)
    g.pair(b, "wrap_tier5_b", (0.08, 9.16, 2.08), (4.02, 11.22, 2.35), "linen_dk", sided=True)
    g.pair(b, "wrap_tier5_out", (4.08, 9.16, -2.04), (4.38, 11.22, 2.04), "linen", sided=True)


def part_wrap_layers(g: Gear) -> None:
    """表面螺旋重叠的缠布折边、斜向隆起带，与散开翘起的破布角。"""
    b = "wrap_layers"
    # ── 4 道螺旋倾斜缠绕条带（前斜、侧斜、后斜）
    # 螺旋 1: y ≈ 2.2 .. 3.2
    g.pair(b, "spiral_f_1", (0.12, 2.22, -2.48), (3.98, 3.12, -2.25), "linen_lt", sided=True)
    g.pair(b, "spiral_out_1", (4.15, 2.62, -1.8), (4.46, 3.52, 1.8), "linen_lt", sided=True)
    g.pair(b, "spiral_b_1", (0.12, 3.02, 2.25), (3.98, 3.92, 2.48), "linen_dk", sided=True)
    g.pair(b, "spiral_crest_1", (0.8, 2.48, -2.54), (3.2, 2.88, -2.28), "linen", sided=True)

    # 螺旋 2: y ≈ 4.2 .. 5.2
    g.pair(b, "spiral_f_2", (0.12, 4.22, -2.48), (3.98, 5.12, -2.25), "linen_dk", sided=True)
    g.pair(b, "spiral_out_2", (4.15, 4.62, -1.8), (4.46, 5.52, 1.8), "linen", sided=True)
    g.pair(b, "spiral_b_2", (0.12, 5.02, 2.25), (3.98, 5.92, 2.5), "linen_lt", sided=True)
    g.pair(b, "spiral_crest_2", (0.8, 4.48, -2.54), (3.2, 4.88, -2.28), "linen_lt", sided=True)

    # 螺旋 3: y ≈ 6.2 .. 7.2
    g.pair(b, "spiral_f_3", (0.12, 6.22, -2.48), (3.98, 7.12, -2.25), "linen_lt", sided=True)
    g.pair(b, "spiral_out_3", (4.15, 6.62, -1.8), (4.46, 7.52, 1.8), "linen_dk", sided=True)
    g.pair(b, "spiral_b_3", (0.12, 7.02, 2.25), (3.98, 7.92, 2.5), "linen", sided=True)
    g.pair(b, "spiral_crest_3", (0.8, 6.48, -2.54), (3.2, 6.88, -2.28), "linen", sided=True)

    # 螺旋 4: y ≈ 8.2 .. 9.2
    g.pair(b, "spiral_f_4", (0.12, 8.22, -2.48), (3.98, 9.12, -2.25), "linen", sided=True)
    g.pair(b, "spiral_out_4", (4.15, 8.62, -1.8), (4.46, 9.52, 1.8), "linen_lt", sided=True)
    g.pair(b, "spiral_b_4", (0.12, 9.02, 2.25), (3.98, 9.92, 2.48), "linen_dk", sided=True)
    g.pair(b, "spiral_crest_4", (0.8, 8.48, -2.54), (3.2, 8.88, -2.28), "linen_dk", sided=True)

    # ── 散开翘起的粗麻布头与下垂撕裂布舌（末土废土质感）
    g.pair(b, "loose_flap_f1", (1.26, 3.45, -2.66), (2.16, 4.15, -2.22), "linen_lt", sided=True)
    g.pair(b, "loose_flap_f2", (2.56, 7.45, -2.66), (3.46, 8.15, -2.22), "linen_dk", sided=True)
    g.pair(b, "loose_flap_f3", (0.5, 5.25, -2.60), (1.4, 5.85, -2.24), "linen", sided=True)
    g.pair(b, "loose_flap_f4", (2.2, 9.45, -2.58), (3.1, 10.15, -2.24), "linen_dk", sided=True)
    g.pair(b, "loose_flap_b1", (1.4, 5.45, 2.22), (2.3, 6.15, 2.62), "linen", sided=True)
    g.pair(b, "loose_flap_b2", (2.2, 8.25, 2.22), (3.1, 8.95, 2.62), "linen_lt", sided=True)
    g.pair(b, "loose_flap_b3", (0.55, 3.85, 2.24), (1.45, 4.55, 2.54), "linen_dk", sided=True)
    g.pair(b, "loose_flap_b4", (2.4, 6.65, 2.24), (3.3, 7.35, 2.54), "linen", sided=True)
    g.pair(b, "loose_tag_out1", (4.16, 6.15, -1.2), (4.52, 6.85, -0.5), "linen_dk", sided=True)
    g.pair(b, "loose_tag_out2", (4.16, 8.15, 0.4), (4.52, 8.85, 1.1), "linen", sided=True)
    g.pair(b, "loose_tag_out3", (4.16, 4.15, 0.2), (4.52, 4.85, 0.9), "linen_lt", sided=True)

    # 表面下垂与纵向压紧带 (丰富层次)
    g.pair(b, "shin_cord_stay_1", (1.18, 2.15, -2.62), (1.62, 9.75, -2.32), "cord_dk", sided=True)
    g.pair(b, "shin_cord_stay_2", (2.52, 2.15, -2.62), (2.96, 9.75, -2.32), "cord", sided=True)
    g.pair(b, "shin_cross_tie_1", (0.9, 3.85, -2.68), (1.95, 4.25, -2.35), "stitch", sided=True)
    g.pair(b, "shin_cross_tie_2", (2.2, 5.85, -2.68), (3.25, 6.25, -2.35), "stitch", sided=True)
    g.pair(b, "shin_cross_tie_3", (0.9, 7.85, -2.68), (1.95, 8.25, -2.35), "stitch", sided=True)

    # 腿肚后向加固交叉带与缝线
    g.pair(b, "calf_cord_cross1", (0.6, 4.25, 2.28), (2.2, 5.85, 2.58), "cord_dk", sided=True)
    g.pair(b, "calf_cord_cross2", (1.8, 6.25, 2.28), (3.4, 7.85, 2.58), "cord", sided=True)
    g.pair(b, "calf_stitch_1", (1.2, 3.65, 2.32), (2.4, 4.05, 2.62), "stitch", sided=True)
    g.pair(b, "calf_stitch_2", (1.2, 5.65, 2.32), (2.4, 6.05, 2.66), "stitch", sided=True)
    g.pair(b, "calf_stitch_3", (1.2, 7.65, 2.32), (2.4, 8.05, 2.62), "stitch", sided=True)
    g.pair(b, "calf_flap_extra", (2.8, 4.55, 2.26), (3.6, 5.25, 2.60), "linen_dk", sided=True)
    g.pair(b, "shin_cord_accent1", (0.35, 4.85, -2.52), (1.05, 5.65, -2.26), "cord", sided=True)
    g.pair(b, "shin_cord_accent2", (3.15, 6.85, -2.52), (3.85, 7.65, -2.26), "cord_dk", sided=True)
    g.pair(b, "shin_cord_accent3", (0.35, 8.85, -2.52), (1.05, 9.65, -2.26), "cord", sided=True)
    g.pair(b, "calf_cord_accent1", (0.35, 4.85, 2.26), (1.05, 5.65, 2.52), "cord", sided=True)
    g.pair(b, "calf_cord_accent2", (3.15, 6.85, 2.26), (3.85, 7.65, 2.52), "cord_dk", sided=True)
    g.pair(b, "calf_cord_accent3", (0.35, 8.85, 2.26), (1.05, 9.65, 2.52), "cord", sided=True)

    # ── 下脚踝磨破粗麻毛边 (厚度 ≥ 0.22px)
    g.pair(b, "fray_bot_f", (0.2, 0.85, -2.36), (3.8, 1.18, -2.12), "linen_dk", sided=True)
    g.pair(b, "fray_bot_out", (4.06, 0.92, -1.6), (4.34, 1.18, 1.6), "linen_lt", sided=True)
    g.pair(b, "fray_bot_b", (0.2, 0.85, 2.12), (3.8, 1.18, 2.36), "linen", sided=True)
    g.pair(b, "fray_fringe_1", (0.8, 0.52, -2.34), (1.5, 0.88, -2.10), "linen", sided=True)
    g.pair(b, "fray_fringe_2", (2.4, 0.52, -2.34), (3.1, 0.88, -2.10), "linen_lt", sided=True)


def part_knee_cords(g: Gear) -> None:
    """膝盖下方重重扎紧的粗草绳圈与系结垂穗。
    位于 y ∈ [9.8, 11.2]。
    """
    b = "knee_cord"
    # 上圈草绳 (y: 10.65 .. 11.15)
    g.pair(b, "knee_cord_hi_f", (0.05, 10.65, -2.48), (4.05, 11.15, -2.2), "cord", sided=True)
    g.pair(b, "knee_cord_hi_out", (4.18, 10.65, -2.12), (4.52, 11.15, 2.12), "cord", sided=True)
    g.pair(b, "knee_cord_hi_b", (0.05, 10.65, 2.2), (4.05, 11.15, 2.48), "cord", sided=True)

    # 下圈草绳 (y: 9.95 .. 10.45)
    g.pair(b, "knee_cord_lo_f", (0.05, 9.95, -2.48), (4.05, 10.45, -2.2), "cord_dk", sided=True)
    g.pair(b, "knee_cord_lo_out", (4.18, 9.95, -2.12), (4.52, 10.45, 2.12), "cord_dk", sided=True)
    g.pair(b, "knee_cord_lo_b", (0.05, 9.95, 2.2), (4.05, 10.45, 2.48), "cord_dk", sided=True)

    # 膝下绳结核心
    g.pair(b, "knee_knot_main", (1.6, 10.15, -2.62), (2.4, 10.95, -2.22), "cord_dk", sided=True)
    g.pair(b, "knee_knot_wrap", (1.8, 10.32, -2.68), (2.2, 10.78, -2.24), "cord", sided=True)
    # 膝下两根垂穗
    g.pair(b, "knee_tail_1", (1.5, 9.15, -2.56), (1.9, 10.18, -2.26), "cord", sided=True)
    g.pair(b, "knee_tail_2", (2.05, 8.85, -2.56), (2.45, 10.18, -2.26), "cord_dk", sided=True)
    g.pair(b, "knee_tail_fringe", (1.6, 8.45, -2.58), (1.95, 9.18, -2.22), "cord_dk", sided=True)


def part_ankle_cords(g: Gear) -> None:
    """脚踝上方草绳扎带与系结（用户明确要求：膝下和脚踝各一道草绳打结）。
    位于 y ∈ [1.6, 2.8]。
    """
    b = "ankle_cord"
    # 脚踝草绳全圈
    g.pair(b, "ankle_cord_f", (0.05, 1.82, -2.52), (4.05, 2.32, -2.22), "cord", sided=True)
    g.pair(b, "ankle_cord_out", (4.16, 1.82, -2.12), (4.52, 2.32, 2.12), "cord", sided=True)
    g.pair(b, "ankle_cord_b", (0.05, 1.82, 2.22), (4.05, 2.32, 2.52), "cord", sided=True)

    # 脚踝外侧绳结
    g.pair(b, "ankle_knot_main", (4.20, 1.72, -0.45), (4.62, 2.48, 0.45), "cord_dk", sided=True)
    g.pair(b, "ankle_knot_core", (4.32, 1.88, -0.22), (4.72, 2.36, 0.22), "cord", sided=True)
    # 脚踝垂绳头
    g.pair(b, "ankle_tail_1", (4.25, 0.88, -0.36), (4.58, 1.75, -0.05), "cord", sided=True)
    g.pair(b, "ankle_tail_2", (4.25, 1.05, 0.05), (4.58, 1.75, 0.36), "cord_dk", sided=True)
    g.pair(b, "ankle_knot_ear1", (4.28, 2.25, -0.65), (4.66, 2.75, -0.15), "cord_dk", sided=True)
    g.pair(b, "ankle_knot_ear2", (4.28, 2.25, 0.15), (4.66, 2.75, 0.65), "cord", sided=True)


def part_outer_patches(g: Gear) -> None:
    """左右小腿外侧的加固兽皮防磨护板与骨扣。
    y ∈ [3.8, 8.6], x ∈ [4.22, 4.95]。
    """
    b = "patch_side"
    # 外侧主皮片 (y: 3.8 .. 8.4)
    g.pair(b, "patch_main", (4.22, 3.82, -1.4), (4.54, 8.42, 1.4), "patch", sided=True)
    g.pair(b, "patch_crest", (4.36, 4.42, -0.96), (4.62, 7.78, 0.96), "patch_lt", sided=True)
    g.pair(b, "patch_shade", (4.2, 3.84, -1.46), (4.5, 8.38, -1.04), "patch_dk", sided=True)
    g.pair(b, "patch_corner_curl", (4.32, 7.85, 1.05), (4.65, 8.52, 1.48), "patch_dk", sided=True)

    # 皮片边缘粗麻加固缝线 (上、下、前后)
    g.pair(b, "patch_stitch_top", (4.28, 8.28, -1.2), (4.58, 8.55, 1.2), "stitch", sided=True)
    g.pair(b, "patch_stitch_bot", (4.28, 3.62, -1.2), (4.58, 3.92, 1.2), "stitch", sided=True)
    g.pair(b, "patch_stitch_f", (4.28, 4.22, -1.52), (4.58, 8.02, -1.25), "stitch", sided=True)
    g.pair(b, "patch_stitch_b", (4.28, 4.22, 1.25), (4.58, 8.02, 1.52), "stitch", sided=True)

    # 皮片中央圆柱形加固小骨扣（左右配对，厚度与高宽 ≥ 0.65px，供穿模自证）
    g.pair(b, "patch_bone_buckle", (4.26, 5.75, -0.45), (4.92, 6.45, 0.45), "bone", sided=True)
    g.pair(b, "patch_bone_pin", (4.34, 5.92, -0.15), (4.85, 6.28, 0.15), "bone_dk", sided=True)


def part_shin_needles(g: Gear) -> None:
    """右小腿外侧皮片上斜插的 2 根缝补粗骨针（不对称挂件，长短各一，用于穿模差分自证）。"""
    sn = "shin_needle_r"
    # 插针皮套
    g.box(sn, "needle_loop_top", (4.38, 7.22, -0.75), (4.74, 7.68, 0.25), "cord_dk")
    g.box(sn, "needle_loop_bot", (4.38, 6.12, -0.75), (4.74, 6.58, 0.25), "cord_dk")

    # 骨针 1（主针，长粗针，尺寸深度 ≥ 0.65px）
    g.box(sn, "needle_head_1", (4.42, 8.22, -0.65), (4.82, 8.95, -0.05), "bone")
    g.box(sn, "needle_eye_1", (4.52, 8.52, -0.55), (4.86, 8.78, -0.15), "void")
    g.box(sn, "needle_shaft_1", (4.45, 5.62, -0.62), (4.82, 8.22, -0.08), "bone")
    g.box(sn, "needle_tip_1", (4.48, 4.52, -0.58), (4.74, 5.62, -0.12), "bone_lt")
    g.box(sn, "needle_shade_1", (4.4, 5.82, -0.64), (4.65, 8.02, -0.12), "bone_dk")

    # 骨针 2（副针，短细针，斜靠在旁）
    g.box(sn, "needle_head_2", (4.44, 7.82, 0.05), (4.78, 8.45, 0.52), "bone_lt")
    g.box(sn, "needle_shaft_2", (4.46, 5.82, 0.08), (4.76, 7.82, 0.48), "bone")
    g.box(sn, "needle_tip_2", (4.48, 4.92, 0.12), (4.72, 5.82, 0.44), "bone_lt")


def part_ankle_trinket(g: Gear) -> None:
    """左小腿绑带下垂的磨损骨珠串（不对称挂件，3 颗雕纹骨珠 + 绳尾，用于穿模差分自证）。"""
    ab = "ankle_beads_l"
    # 悬挂麻绳段
    g.box(ab, "bead_cord", (-4.65, 3.65, -0.2), (-4.34, 4.62, 0.2), "cord")
    # 上骨珠 (尺寸深度 ≥ 0.65px)
    g.box(ab, "bead_top_main", (-4.92, 2.75, -0.42), (-4.22, 3.58, 0.42), "bone")
    g.box(ab, "bead_top_band", (-4.95, 2.98, -0.25), (-4.18, 3.35, 0.25), "bone_lt")
    # 中骨珠
    g.box(ab, "bead_mid_main", (-4.92, 1.75, -0.42), (-4.22, 2.58, 0.42), "bone_dk")
    g.box(ab, "bead_mid_crest", (-4.88, 1.95, -0.28), (-4.25, 2.38, 0.28), "bone")
    # 下骨珠
    g.box(ab, "bead_bot_main", (-4.86, 0.85, -0.38), (-4.28, 1.58, 0.38), "bone_lt")
    g.box(ab, "bead_bot_tip", (-4.78, 0.35, -0.28), (-4.32, 0.85, 0.28), "bone_dk")
    # 绳尾小结
    g.box(ab, "bead_knot", (-4.65, -0.05, -0.18), (-4.35, 0.38, 0.18), "cord_dk")


PART_FUNCS = {
    "wrap_base": part_wrap_base,
    "wrap_layers": part_wrap_layers,
    "knee_cords": part_knee_cords,
    "ankle_cords": part_ankle_cords,
    "outer_patches": part_outer_patches,
    "shin_needles": part_shin_needles,
    "ankle_trinket": part_ankle_trinket,
}

SPEC = GearSpec(
    gear_id="cu_bu_bang_tui",
    model_name="CuBuBangTui",
    title="粗布绑腿 / cu_bu_bang_tui",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：双小腿（y∈[0.8, 11.5]，右腿 x∈[0,4], 左腿 x∈[-4,0]）",
    worn_focus=((0.0, 6.0, 0.0), 18.0),
    texture_overrides={"stitch": "gear_stitch_lt"},  # 画笔库里 gear_stitch 已是腰挂件的深线，这里用浅粗线
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
