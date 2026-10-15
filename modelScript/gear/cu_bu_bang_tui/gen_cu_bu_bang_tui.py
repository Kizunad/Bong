#!/usr/bin/env python3
"""粗布绑腿（cu_bu_bang_tui）Blockbench .bbmodel 程序化生成器 —— Round 1/3：几何骨架与部件。

任务卡：末土装备 #16（.task-wasteland-gear.md）。
挂点：双小腿（y∈[0.8, 11.5]，右腿 x∈[0,4], 左腿 x∈[-4,0], z∈[-2,2]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  右腿 x∈[0,4], y∈[0,12], z∈[-2,2]
  左腿 x∈[-4,0], y∈[0,12], z∈[-2,2]
  门禁 clearance：任何方块不得扎进左右腿或躯干（咬入 ≤ 0.05px）。

外形：
  粗布条一层层、一圈圈紧紧缠裹在双小腿上，自脚踝上方（y≈1.0）直缠至膝下（y≈11.2）；
  前胸、后肚、外侧全面包裹，内侧留出微缝避免双腿摩擦穿透；
  膝下（y≈10.2..11.2）用双股粗草绳重重扎紧，打结留垂穗；
  外侧覆有防刺耐磨的旧皮补片，带骨扣与粗麻缝线；
  右腿外侧插有一根缝补粗骨针，左腿绑带下垂一串磨光骨珠计数串。

分部件：
  - wrap_base: 左右小腿粗布缠裹主片（多层螺旋阶梯交错，覆盖前后外侧）
  - wrap_layers: 表面螺旋重叠的缠布折边与粗布毛边
  - knee_cords: 膝盖下方草绳扎圈与绳结
  - outer_patches: 左右小腿外侧兽皮补片与固定骨扣
  - shin_needles: 右小腿绑带插槽中的粗骨针（不对称）
  - ankle_trinket: 左小腿绑绳下垂的一串磨损骨珠（不对称）

用法：
    python3 modelScript/gear/cu_bu_bang_tui/gen_cu_bu_bang_tui.py --self-test   # 门禁差分自证
    python3 modelScript/gear/cu_bu_bang_tui/gen_cu_bu_bang_tui.py --part wrap_base # 单件预览
    python3 modelScript/gear/cu_bu_bang_tui/gen_cu_bu_bang_tui.py --worn        # 只出穿戴三面
    python3 modelScript/gear/cu_bu_bang_tui/gen_cu_bu_bang_tui.py               # 全量：模型 + check.png
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
    ("wrap_layers_r", "leg_r_mount", (2.0, 6.0, 0.0), "右小腿螺旋重叠折边"),
    ("knee_cord_r", "leg_r_mount", (2.0, 10.5, 0.0), "右小腿膝下草绳扎带"),
    ("patch_side_r", "leg_r_mount", (4.2, 6.0, 0.0), "右小腿外侧皮补丁与骨扣"),
    ("shin_needle_r", "patch_side_r", (4.3, 7.5, 0.0), "右小腿外侧插缝补粗骨针"),
    ("leg_l_mount", "root", (-2.0, 6.0, 0.0), "左小腿安装基准骨"),
    ("wrap_main_l", "leg_l_mount", (-2.0, 6.0, 0.0), "左小腿绑腿主体"),
    ("wrap_layers_l", "leg_l_mount", (-2.0, 6.0, 0.0), "左小腿螺旋重叠折边"),
    ("knee_cord_l", "leg_l_mount", (-2.0, 10.5, 0.0), "左小腿膝下草绳扎带"),
    ("patch_side_l", "leg_l_mount", (-4.2, 6.0, 0.0), "左小腿外侧皮补丁与骨扣"),
    ("ankle_beads_l", "leg_l_mount", (-4.3, 3.5, 0.0), "左小腿绑带下垂骨珠串"),
]

ASYM_BONES = frozenset([
    "shin_needle_r", "ankle_beads_l",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_wrap_base(g: Gear) -> None:
    """左右小腿绑腿主体：前后与外侧全方位包覆。
    右腿：x ∈ [0.0, 4.0], y ∈ [0.0, 12.0], z ∈ [-2.0, 2.0]。
    绑腿高度 y ∈ [1.2, 11.2]。
    内侧 (x ≈ 0.0) 留空，避免两腿内侧穿模碰撞。
    - 前主面包覆：z ∈ [-2.36, -2.08], x ∈ [0.08, 4.02]
    - 后主面包覆：z ∈ [2.08, 2.36], x ∈ [0.08, 4.02]
    - 外侧主面包覆：x ∈ [4.08, 4.38], z ∈ [-2.06, 2.06]
    全部使用 g.pair 镜像到左腿（sided=True）。
    """
    b = "wrap_main"
    # 前面上下分段主片
    g.pair(b, "wrap_f_lower", (0.08, 1.2, -2.36), (4.02, 5.8, -2.08), "linen", sided=True)
    g.pair(b, "wrap_f_mid", (0.08, 5.82, -2.38), (4.02, 9.2, -2.08), "linen_dk", sided=True)
    g.pair(b, "wrap_f_upper", (0.08, 9.22, -2.36), (4.02, 11.2, -2.08), "linen", sided=True)

    # 后面小腿肚隆起分段主片
    g.pair(b, "wrap_b_lower", (0.08, 1.2, 2.08), (4.02, 5.2, 2.36), "linen", sided=True)
    g.pair(b, "wrap_b_calf", (0.08, 5.22, 2.08), (4.02, 9.0, 2.42), "linen_lt", sided=True)
    g.pair(b, "wrap_b_upper", (0.08, 9.02, 2.08), (4.02, 11.2, 2.36), "linen_dk", sided=True)

    # 外侧主面
    g.pair(b, "wrap_out_lower", (4.08, 1.2, -2.06), (4.38, 6.0, 2.06), "linen", sided=True)
    g.pair(b, "wrap_out_upper", (4.08, 6.02, -2.06), (4.38, 11.2, 2.06), "linen_dk", sided=True)


def part_wrap_layers(g: Gear) -> None:
    """表面螺旋重叠的缠布折边、隆起带与斜缠肌理。"""
    b = "wrap_layers"
def part_wrap_layers(g: Gear) -> None:
    """表面螺旋重叠的缠布折边、隆起带与斜缠肌理。"""
    b = "wrap_layers"
    # 斜缠螺旋布环（阶梯层叠模拟斜向绑扎，厚度 ≥ 0.22px）
    # 层 1: y ≈ 2.2 .. 3.2
    g.pair(b, "spiral_f_1", (0.12, 2.2, -2.48), (3.98, 3.1, -2.25), "linen_lt", sided=True)
    g.pair(b, "spiral_out_1", (4.15, 2.6, -1.8), (4.45, 3.5, 1.8), "linen_lt", sided=True)
    g.pair(b, "spiral_b_1", (0.12, 3.0, 2.25), (3.98, 3.9, 2.48), "linen_dk", sided=True)
    g.pair(b, "spiral_crest_1", (0.8, 2.45, -2.52), (3.2, 2.85, -2.28), "linen", sided=True)

    # 层 2: y ≈ 4.2 .. 5.2
    g.pair(b, "spiral_f_2", (0.12, 4.2, -2.48), (3.98, 5.1, -2.25), "linen_dk", sided=True)
    g.pair(b, "spiral_out_2", (4.15, 4.6, -1.8), (4.45, 5.5, 1.8), "linen", sided=True)
    g.pair(b, "spiral_b_2", (0.12, 5.0, 2.25), (3.98, 5.9, 2.5), "linen_lt", sided=True)
    g.pair(b, "spiral_crest_2", (0.8, 4.45, -2.52), (3.2, 4.85, -2.28), "linen_lt", sided=True)

    # 层 3: y ≈ 6.2 .. 7.2
    g.pair(b, "spiral_f_3", (0.12, 6.2, -2.48), (3.98, 7.1, -2.25), "linen_lt", sided=True)
    g.pair(b, "spiral_out_3", (4.15, 6.6, -1.8), (4.45, 7.5, 1.8), "linen_dk", sided=True)
    g.pair(b, "spiral_b_3", (0.12, 7.0, 2.25), (3.98, 7.9, 2.5), "linen", sided=True)
    g.pair(b, "spiral_crest_3", (0.8, 6.45, -2.52), (3.2, 6.85, -2.28), "linen", sided=True)

    # 层 4: y ≈ 8.2 .. 9.2
    g.pair(b, "spiral_f_4", (0.12, 8.2, -2.48), (3.98, 9.1, -2.25), "linen", sided=True)
    g.pair(b, "spiral_out_4", (4.15, 8.6, -1.8), (4.45, 9.5, 1.8), "linen_lt", sided=True)
    g.pair(b, "spiral_b_4", (0.12, 9.0, 2.25), (3.98, 9.9, 2.48), "linen_dk", sided=True)
    g.pair(b, "spiral_crest_4", (0.8, 8.45, -2.52), (3.2, 8.85, -2.28), "linen_dk", sided=True)

    # 胫前正中纵向束带（加固直筋）
    g.pair(b, "shin_ridge_c", (1.75, 1.6, -2.58), (2.25, 9.8, -2.3), "cord", sided=True)
    for k, y in enumerate((2.6, 4.6, 6.6, 8.6)):
        g.pair(b, f"shin_cross_{k}", (1.4, y, -2.64), (2.6, y + 0.35, -2.34), "stitch", sided=True)

    # 下脚踝磨破粗麻毛边 (各维度尺寸 ≥ 0.22px)
    g.pair(b, "fray_bot_f", (0.2, 0.85, -2.36), (3.8, 1.2, -2.12), "linen_dk", sided=True)
    g.pair(b, "fray_bot_out", (4.06, 0.85, -1.6), (4.32, 1.2, 1.6), "linen_lt", sided=True)
    g.pair(b, "fray_bot_b", (0.2, 0.85, 2.12), (3.8, 1.2, 2.36), "linen", sided=True)
    g.pair(b, "fray_fringe_1", (0.8, 0.55, -2.35), (1.5, 0.88, -2.13), "linen", sided=True)
    g.pair(b, "fray_fringe_2", (2.4, 0.55, -2.35), (3.1, 0.88, -2.13), "linen_lt", sided=True)


def part_knee_cords(g: Gear) -> None:
    """膝盖下方重重扎紧的双股草绳圈与系结垂穗。"""
    b = "knee_cord"
    # 上草绳圈 (y: 10.65 .. 11.15)
    g.pair(b, "cord_upper_f", (0.05, 10.65, -2.48), (4.05, 11.15, -2.2), "cord", sided=True)
    g.pair(b, "cord_upper_out", (4.18, 10.65, -2.12), (4.52, 11.15, 2.12), "cord", sided=True)
    g.pair(b, "cord_upper_b", (0.05, 10.65, 2.2), (4.05, 11.15, 2.48), "cord", sided=True)

    # 下草绳圈 (y: 9.95 .. 10.45)
    g.pair(b, "cord_lower_f", (0.05, 9.95, -2.48), (4.05, 10.45, -2.2), "cord_dk", sided=True)
    g.pair(b, "cord_lower_out", (4.18, 9.95, -2.12), (4.52, 10.45, 2.12), "cord_dk", sided=True)
    g.pair(b, "cord_lower_b", (0.05, 9.95, 2.2), (4.05, 10.45, 2.48), "cord_dk", sided=True)

    # 膝下前侧绳结核心
    g.pair(b, "cord_knot_main", (1.6, 10.15, -2.62), (2.4, 10.95, -2.22), "cord_dk", sided=True)
    g.pair(b, "cord_knot_wrap", (1.8, 10.32, -2.68), (2.2, 10.78, -2.24), "cord", sided=True)
    # 绳结垂穗
    g.pair(b, "cord_tail_1", (1.5, 9.15, -2.56), (1.9, 10.2, -2.26), "cord", sided=True)
    g.pair(b, "cord_tail_2", (2.05, 8.85, -2.56), (2.45, 10.2, -2.26), "cord_dk", sided=True)


def part_outer_patches(g: Gear) -> None:
    """左右小腿外侧的加固兽皮防磨护板与骨扣。"""
    b = "patch_side"
    # 外侧主皮片 (y: 3.8 .. 8.4)
    g.pair(b, "patch_main", (4.22, 3.8, -1.4), (4.54, 8.4, 1.4), "patch", sided=True)
    g.pair(b, "patch_crest", (4.32, 4.4, -1.0), (4.62, 7.8, 1.0), "patch_lt", sided=True)
    g.pair(b, "patch_shade", (4.2, 3.82, -1.45), (4.5, 8.38, -1.05), "patch_dk", sided=True)

    # 皮片边缘粗麻加固缝线 (上、下、前后)
    g.pair(b, "patch_stitch_top", (4.28, 8.25, -1.2), (4.58, 8.52, 1.2), "stitch", sided=True)
    g.pair(b, "patch_stitch_bot", (4.28, 3.65, -1.2), (4.58, 3.92, 1.2), "stitch", sided=True)
    g.pair(b, "patch_stitch_f", (4.28, 4.2, -1.52), (4.58, 8.0, -1.25), "stitch", sided=True)
    g.pair(b, "patch_stitch_b", (4.28, 4.2, 1.25), (4.58, 8.0, 1.52), "stitch", sided=True)

    # 皮片中央圆柱形加固小骨扣（左右配对，厚度与高宽 ≥ 0.65px，供穿模自证）
    g.pair(b, "patch_bone_buckle", (4.26, 5.75, -0.45), (4.92, 6.45, 0.45), "bone", sided=True)
    g.pair(b, "patch_bone_pin", (4.34, 5.92, -0.15), (4.85, 6.28, 0.15), "bone_dk", sided=True)


def part_shin_needles(g: Gear) -> None:
    """右小腿外侧皮片上斜插的粗骨针（不对称挂件，用于穿模差分自证）。"""
    sn = "shin_needle_r"
    # 插针皮套
    g.box(sn, "needle_loop_top", (4.38, 7.2, -0.65), (4.74, 7.65, -0.05), "cord_dk")
    g.box(sn, "needle_loop_bot", (4.38, 6.12, -0.65), (4.74, 6.55, -0.05), "cord_dk")
    # 骨针主体（尺寸深度 ≥ 0.65px，供差分自证抓取）
    g.box(sn, "needle_head", (4.42, 8.2, -0.62), (4.82, 8.95, 0.02), "bone")
    g.box(sn, "needle_eye", (4.52, 8.5, -0.52), (4.86, 8.78, -0.08), "void")
    g.box(sn, "needle_shaft", (4.45, 5.6, -0.58), (4.82, 8.2, -0.02), "bone")
    g.box(sn, "needle_tip", (4.48, 4.5, -0.52), (4.74, 5.6, -0.08), "bone_lt")
    g.box(sn, "needle_shade", (4.4, 5.8, -0.6), (4.65, 8.0, -0.08), "bone_dk")


def part_ankle_trinket(g: Gear) -> None:
    """左小腿绑带下垂的磨损骨珠串（不对称挂件，用于穿模差分自证）。"""
    ab = "ankle_beads_l"
    # 悬挂麻绳段
    g.box(ab, "bead_cord", (-4.65, 3.6, -0.2), (-4.34, 4.6, 0.2), "cord")
    # 上骨珠
    g.box(ab, "bead_top_main", (-4.92, 2.7, -0.42), (-4.22, 3.55, 0.42), "bone")
    g.box(ab, "bead_top_band", (-4.95, 2.95, -0.25), (-4.18, 3.3, 0.25), "bone_lt")
    # 下骨珠（尺寸深度 ≥ 0.65px，供穿模自证）
    g.box(ab, "bead_bot_main", (-4.92, 1.7, -0.42), (-4.22, 2.55, 0.42), "bone_dk")
    g.box(ab, "bead_bot_tip", (-4.78, 1.15, -0.32), (-4.28, 1.7, 0.32), "bone_lt")
    # 绳尾小结
    g.box(ab, "bead_knot", (-4.65, 0.75, -0.18), (-4.35, 1.15, 0.18), "cord_dk")


PART_FUNCS = {
    "wrap_base": part_wrap_base,
    "wrap_layers": part_wrap_layers,
    "knee_cords": part_knee_cords,
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
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
