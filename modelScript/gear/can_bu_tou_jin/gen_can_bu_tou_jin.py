#!/usr/bin/env python3
"""残布头巾（can_bu_tou_jin）Blockbench .bbmodel 程序化生成器 —— Round 1/3：几何骨架与部件。

任务卡：末土装备 #18（.task-wasteland-gear.md）。
挂点：头（头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]
  躯干 x∈[-4,4], y∈[12,24], z∈[-2,2]
  门禁 clearance：任何方块不得扎进头盒（咬入 ≤ 0.05px；在 x∈[-4,4], y∈[24,32], z∈[-4,4] 内绝对不可穿透）。

外形：
  几层残布重重叠叠缠裹在头颅四周（额头、两鬓、后脑 y≈27.0..32.4）；
  头顶呈层叠螺旋收拢圆盘，顶端露出残破布结与毛边（y≈32.08..33.2）；
  额前（z≈-4.8..-4.08, y≈27.4..29.8）斜别一根打磨光滑的兽骨簪（带穿孔与暗纹刻槽）；
  右后耳侧（x≈3.8..4.6, z≈3.2..4.6）垂下一截参差布尾，一直垂至肩背（y≈20.0..28.0）；
  布层上带有深色旧皮补丁、粗麻缝线迹。

分部件：
  - turban_crown: 头顶螺旋缠裹圆顶与顶结
  - turban_brow: 额前横缠加固布层与眉间折痕
  - turban_sides: 两鬓与后脑多层交叠布带
  - bone_hairpin: 额前斜插的雕工兽骨簪（不对称挂件）
  - cloth_tail: 右后耳侧垂落至肩的破损布尾（不对称挂件）
  - turban_details: 破布补丁、边缘缝线与麻绳紧箍圈

用法：
    python3 modelScript/gear/can_bu_tou_jin/gen_can_bu_tou_jin.py --self-test   # 门禁差分自证
    python3 modelScript/gear/can_bu_tou_jin/gen_can_bu_tou_jin.py --part turban_crown # 单件预览
    python3 modelScript/gear/can_bu_tou_jin/gen_can_bu_tou_jin.py --worn        # 只出穿戴三面
    python3 modelScript/gear/can_bu_tou_jin/gen_can_bu_tou_jin.py               # 全量：模型 + check.png
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
    "linen_dk":     (108, 92, 70),     # 粗布暗面/阴影
    "patch":        (120, 88, 64),     # 废皮补丁
    "patch_dk":     (86, 62, 44),      # 补丁暗面
    "cord":         (150, 126, 84),    # 束头细绳
    "cord_dk":      (104, 84, 54),     # 绳结深色
    "stitch":       (188, 176, 148),   # 粗缝线
    "bone":         (206, 194, 164),   # 兽骨簪
    "bone_lt":      (224, 214, 186),   # 骨簪高光
    "bone_dk":      (156, 142, 112),   # 骨簪暗刻槽
    "rust":         (146, 84, 50),     # 束带小铁扣/铆钉
    "void":         (28, 24, 22),      # 簪孔与折缝暗处
}

SOFT = ("linen", "linen_lt", "linen_dk", "patch", "patch_dk", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("head_mount", "root", (0.0, 24.0, 0.0), "头戴基准骨"),
    ("turban_crown", "head_mount", (0.0, 32.0, 0.0), "头巾顶盖螺旋层"),
    ("turban_brow", "head_mount", (0.0, 28.5, -4.2), "额前横缠布圈"),
    ("turban_side_r", "head_mount", (4.2, 28.5, 0.0), "右侧缠裹层"),
    ("turban_side_l", "head_mount", (-4.2, 28.5, 0.0), "左侧缠裹层"),
    ("turban_back", "head_mount", (0.0, 28.5, 4.2), "后脑缠裹层"),
    ("bone_pin_mount", "head_mount", (0.0, 28.8, -4.5), "额前骨簪安装骨"),
    ("cloth_tail_root", "head_mount", (4.2, 27.5, 3.8), "右后耳侧垂布根部"),
    ("cloth_tail_mid", "cloth_tail_root", (4.3, 24.0, 3.7), "垂布中段（摆动骨）"),
    ("cloth_tail_tip", "cloth_tail_mid", (4.3, 20.5, 3.6), "垂布末端毛边"),
    ("turban_patch", "head_mount", (-2.5, 29.5, -4.3), "额角兽皮补丁与线迹"),
    ("hang_iron_clasp", "cloth_tail_root", (4.5, 26.5, 3.9), "垂带根部锈铁扣"),
]

ASYM_BONES = frozenset([
    "bone_pin_mount", "cloth_tail_root", "cloth_tail_mid", "cloth_tail_tip",
    "turban_patch", "hang_iron_clasp",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_turban_crown(g: Gear) -> None:
    """头顶螺旋缠裹圆顶与顶结：
    头盒顶面在 y = 32.0。顶盖完全位于 y ∈ [32.08, 33.35]，且 x ∈ [-4.4, 4.4], z ∈ [-4.4, 4.4]。
    严格高于头盒顶 y=32.08，绝不进入头盒。
    """
    b = "turban_crown"
    # 头顶底层大盖板 (y: 32.08 .. 32.48)
    g.box(b, "crown_base_plate", (-4.18, 32.08, -4.18), (4.18, 32.48, 4.18), "linen")
    # 底层螺旋交错带 (前后盖过左右，端面错开避免共面)
    g.box(b, "crown_band_f", (-4.22, 32.22, -4.35), (4.22, 32.58, -3.75), "linen_dk")
    g.box(b, "crown_band_b", (-4.22, 32.22, 3.75), (4.22, 32.58, 4.35), "linen_lt")
    g.pair("turban_side", "crown_band_side", (3.78, 32.25, -3.72), (4.35, 32.55, 3.72), "linen", sided=True)

    # 头顶中层阶梯收边 (y: 32.48 .. 32.88)
    g.box(b, "crown_mid_tier", (-3.6, 32.48, -3.6), (3.6, 32.88, 3.6), "linen_dk")
    g.box(b, "crown_mid_crest", (-3.1, 32.55, -3.1), (3.1, 32.92, 3.1), "linen_lt")
    # 头顶上层螺旋台阶 (y: 32.88 .. 33.22)
    g.box(b, "crown_top_spiral", (-2.2, 32.88, -2.2), (2.2, 33.22, 2.2), "linen")
    g.box(b, "crown_top_ring", (-1.6, 33.15, -1.6), (1.6, 33.35, 1.6), "linen_lt")
    g.box(b, "crown_top_knot", (-0.9, 33.22, -0.9), (0.9, 33.55, 0.9), "linen_dk")
    g.box(b, "crown_knot_tuft", (-0.45, 33.55, -0.45), (0.45, 33.82, 0.45), "linen_lt")


def part_turban_brow(g: Gear) -> None:
    """额前横缠布圈与眉间折痕：
    贴在头盒正前方 z ∈ [-4.85, -4.08]，y ∈ [27.0, 31.8]。
    中线件严格对称于 x=0。
    """
    b = "turban_brow"
    # 额前主缠绕层 (y: 27.5 .. 31.5)
    g.box(b, "brow_main_band", (-4.15, 27.8, -4.48), (4.15, 31.4, -4.08), "linen")
    # 眉心上凸起斜褶皱 (中线件)
    g.box(b, "brow_fold_c1", (-2.8, 29.4, -4.68), (2.8, 30.15, -4.32), "linen_lt")
    g.box(b, "brow_fold_c2", (-2.78, 28.65, -4.68), (2.78, 29.35, -4.32), "linen_dk")
    g.box(b, "brow_fold_c3", (-3.2, 30.18, -4.62), (3.2, 30.78, -4.28), "linen_lt")
    g.box(b, "brow_top_hem", (-3.8, 31.2, -4.55), (3.8, 31.8, -4.12), "linen_dk")

    # 额前下边缘卷边 (y: 27.2 .. 27.8)
    g.box(b, "brow_bot_roll", (-3.6, 27.2, -4.55), (3.6, 27.78, -4.15), "linen_lt")


def part_turban_sides(g: Gear) -> None:
    """两鬓与后脑多层交叠布带：
    头盒侧面 x ∈ [±4.08, ±4.55], y ∈ [27.0, 31.8], z ∈ [-4.1, 4.1]。
    后脑 z ∈ [4.08, 4.58], y ∈ [27.0, 31.8], x ∈ [-4.15, 4.15]。
    """
    # 左右两鬓（使用 g.pair 镜像配对）
    br = "turban_side"
    g.pair(br, "side_main_band", (4.08, 27.42, -4.12), (4.45, 31.58, 4.12), "linen", sided=True)
    g.pair(br, "side_fold_top", (4.12, 30.2, -3.8), (4.52, 31.2, 3.8), "linen_lt", sided=True)
    g.pair(br, "side_fold_mid", (4.12, 28.8, -3.8), (4.52, 29.8, 3.8), "linen_dk", sided=True)
    g.pair(br, "side_bot_roll", (4.12, 27.38, -3.5), (4.48, 28.18, 3.5), "linen_lt", sided=True)

    # 后脑包裹层（中线件）
    bk = "turban_back"
    g.box(bk, "back_main_band", (-4.15, 27.5, 4.08), (4.15, 31.62, 4.45), "linen")
    g.box(bk, "back_fold_upper", (-3.6, 30.0, 4.15), (3.6, 31.2, 4.52), "linen_dk")
    g.box(bk, "back_fold_lower", (-3.6, 28.25, 4.15), (3.6, 29.6, 4.52), "linen_lt")
    g.box(bk, "back_bot_roll", (-3.8, 27.2, 4.12), (3.8, 28.0, 4.48), "linen_dk")

    # 束紧细皮绳一圈（前、侧、后）
    g.box("turban_brow", "brow_cord_f", (-4.18, 28.2, -4.72), (4.18, 28.6, -4.45), "cord")
    g.pair(br, "side_cord", (4.16, 28.2, -4.05), (4.56, 28.6, 4.05), "cord", sided=True)
    g.box(bk, "back_cord_b", (-4.18, 28.2, 4.45), (4.18, 28.6, 4.72), "cord")


def part_bone_hairpin(g: Gear) -> None:
    """额前斜插的雕工兽骨簪（不对称挂件，位于 bone_pin_mount 骨骼）。
    斜横在额头前：x ∈ [-3.2, 3.2], y ∈ [28.6, 30.2], z ∈ [-4.88, -4.42]。
    长宽厚 ≥ 0.65px，供穿模差分自证抓取。
    """
    bp = "bone_hairpin"  # 挂在 bone_pin_mount 骨
    # 簪头（左侧粗端，带簪首雕花圆环）
    g.box("bone_pin_mount", "pin_head_bulb", (-3.35, 29.2, -4.85), (-2.55, 30.05, -4.18), "bone")
    g.box("bone_pin_mount", "pin_head_eye", (-3.12, 29.45, -4.88), (-2.78, 29.8, -4.15), "void")
    g.box("bone_pin_mount", "pin_head_crest", (-3.42, 29.35, -4.86), (-2.48, 29.9, -4.25), "bone_lt")

    # 簪身（横贯额前）
    g.box("bone_pin_mount", "pin_shaft_main", (-2.55, 29.1, -4.78), (2.2, 29.75, -4.22), "bone")
    g.box("bone_pin_mount", "pin_shaft_groove", (-2.0, 29.3, -4.82), (1.5, 29.55, -4.18), "bone_dk")

    # 簪尾尖（右侧磨尖锥形出梢）
    g.box("bone_pin_mount", "pin_tip_taper", (2.2, 29.2, -4.72), (3.15, 29.65, -4.28), "bone_lt")
    g.box("bone_pin_mount", "pin_tip_point", (3.15, 29.3, -4.65), (3.65, 29.55, -4.35), "bone_lt")


def part_cloth_tail(g: Gear) -> None:
    """右后耳侧垂落至肩背的破损布尾（三节级联骨骼）。
    位置在 x ∈ [4.15, 4.58], z ∈ [3.2, 4.55]，向下从 y=28.0 垂落至 y=18.5。
    根部带有紧固用的锈铁扣（hang_iron_clasp 骨骼，尺寸厚度 ≥ 0.65px，供穿模差分自证）。
    """
    # 根部（y: 25.5 .. 28.5）
    tr = "cloth_tail_root"
    g.box(tr, "tail_root_main", (4.18, 25.5, 3.3), (4.85, 28.5, 4.3), "linen")
    g.box(tr, "tail_root_pleat", (4.25, 25.8, 3.45), (4.92, 28.2, 4.15), "linen_dk")
    g.box(tr, "tail_root_hem", (4.22, 27.2, 3.32), (4.88, 28.45, 4.28), "linen_lt")

    # 锈铁束扣（hang_iron_clasp 骨骼，尺寸长宽厚 ≥ 0.65px，用于穿模门差分自证）
    ic = "hang_iron_clasp"
    g.box(ic, "iron_clasp_base", (4.22, 26.15, 3.4), (4.95, 26.85, 4.22), "rust")
    g.box(ic, "iron_clasp_core", (4.32, 26.25, 3.5), (4.98, 26.75, 4.12), "rust")

    # 中段（y: 22.0 .. 25.5）
    tm = "cloth_tail_mid"
    g.box(tm, "tail_mid_main", (4.18, 22.0, 3.35), (4.85, 25.5, 4.25), "linen_dk")
    g.box(tm, "tail_mid_pleat", (4.25, 22.4, 3.5), (4.92, 25.2, 4.1), "linen")
    g.box(tm, "tail_mid_rip", (4.28, 23.2, 3.65), (4.95, 24.2, 3.95), "void")

    # 末梢毛边（y: 18.5 .. 22.0）
    tt = "cloth_tail_tip"
    g.box(tt, "tail_tip_main", (4.18, 19.5, 3.4), (4.85, 22.0, 4.2), "linen")
    g.box(tt, "tail_tip_fray1", (4.22, 18.5, 3.45), (4.82, 19.5, 3.8), "linen_lt")
    g.box(tt, "tail_tip_fray2", (4.22, 18.8, 3.85), (4.82, 19.5, 4.15), "linen_dk")


def part_turban_details(g: Gear) -> None:
    """额角旧兽皮补丁、缝补线迹与装饰。"""
    tp = "turban_patch"
    # 左额角废皮补丁 (x ∈ [-3.8, -1.8], y ∈ [28.5, 31.2], z ∈ [-4.78, -4.38])
    g.box(tp, "patch_brow_base", (-3.65, 28.6, -4.75), (-1.88, 31.1, -4.35), "patch")
    g.box(tp, "patch_brow_worn", (-3.45, 28.85, -4.82), (-2.05, 30.82, -4.42), "patch_dk")
    # 补丁边缘粗缝线迹
    for k, x in enumerate((-3.4, -2.8, -2.25)):
        g.box(tp, f"stitch_patch_top_{k}", (x, 30.95, -4.85), (x + 0.35, 31.25, -4.48), "stitch")
        g.box(tp, f"stitch_patch_bot_{k}", (x, 28.45, -4.85), (x + 0.35, 28.75, -4.48), "stitch")
    g.box(tp, "stitch_patch_l", (-3.75, 29.5, -4.85), (-3.45, 30.2, -4.48), "stitch")
    g.box(tp, "stitch_patch_r", (-2.05, 29.5, -4.85), (-1.75, 30.2, -4.48), "stitch")


PART_FUNCS = {
    "turban_crown": part_turban_crown,
    "turban_brow": part_turban_brow,
    "turban_sides": part_turban_sides,
    "bone_hairpin": part_bone_hairpin,
    "cloth_tail": part_cloth_tail,
    "turban_details": part_turban_details,
}

SPEC = GearSpec(
    gear_id="can_bu_tou_jin",
    model_name="CanBuTouJin",
    title="残布头巾 / can_bu_tou_jin",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：头（头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]）+ 额前骨簪 + 耳侧垂布",
    worn_focus=((0.0, 27.5, 0.0), 20.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
