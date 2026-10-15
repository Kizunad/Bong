#!/usr/bin/env python3
"""残布头巾（can_bu_tou_jin）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工：多层错位缠绕、鼓起不平褶皱顶部、两条长短垂尾与额前斜骨簪。

任务卡：末土装备 #18（.task-wasteland-gear.md）。
挂点：头（头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]
  躯干 x∈[-4,4], y∈[12,24], z∈[-2,2]
  门禁 clearance：任何方块不得扎进头盒（咬入 ≤ 0.05px；在 x∈[-4,4], y∈[24,32], z∈[-4,4] 内绝对不可穿透）。

外形：
  打破方帽感：
  - 4-5 层残布粗条错位重叠缠绕，呈现鼓起、厚实、松垮的头巾轮廓；
  - 顶部不平，呈多重褶皱起伏的圆丘顶包，顶端带破布扭结与散开毛头；
  - 额前斜插一根精工雕花长兽骨簪（带镂空穿孔与刻纹，横跨额前）；
  - 右后耳侧垂下两条长短不一的参差布尾，长尾垂至肩头，短尾飘散在颈侧；
  - 布层覆有废皮补丁、粗麻粗线迹与散开的破布齿。
  总方块数达到 206 块，按挂点分骨 15 骨。
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
    ("turban_crown", "head_mount", (0.0, 32.0, 0.0), "头巾顶盖鼓起褶皱层"),
    ("turban_brow", "head_mount", (0.0, 28.5, -4.2), "额前4-5层错位横缠布圈"),
    ("turban_side_r", "head_mount", (4.2, 28.5, 0.0), "右侧缠裹层"),
    ("turban_side_l", "head_mount", (-4.2, 28.5, 0.0), "左侧缠裹层"),
    ("turban_back", "head_mount", (0.0, 28.5, 4.2), "后脑缠裹层"),
    ("bone_pin_mount", "head_mount", (0.0, 28.8, -4.5), "额前骨簪安装骨"),
    ("cloth_tail_long", "head_mount", (4.2, 27.5, 3.8), "右后长垂布尾（挂点骨）"),
    ("tail_long_mid", "cloth_tail_long", (4.3, 23.5, 3.7), "长垂布中段（摆动骨）"),
    ("tail_long_tip", "tail_long_mid", (4.3, 19.5, 3.6), "长垂布末端毛边"),
    ("cloth_tail_short", "head_mount", (4.1, 26.5, 3.5), "右侧短垂布尾"),
    ("turban_patch", "head_mount", (-2.5, 29.5, -4.3), "额角兽皮补丁与线迹"),
    ("hang_iron_clasp", "cloth_tail_long", (4.5, 26.5, 3.9), "垂带根部锈铁扣"),
]

ASYM_BONES = frozenset([
    "bone_pin_mount", "cloth_tail_long", "tail_long_mid", "tail_long_tip",
    "cloth_tail_short", "turban_patch", "hang_iron_clasp",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_turban_crown(g: Gear) -> None:
    """头顶螺旋缠裹圆顶：由外到内 4 圈螺旋布环盘起，不平褶皱圆包。
    严格高于头盒顶 y = 32.08。
    """
    b = "turban_crown"
    # ── 底层大基座板 (y: 32.08 .. 32.42)
    g.box(b, "crown_base", (-4.22, 32.08, -4.22), (4.22, 32.42, 4.22), "linen")

    # ── 第 1 盘（外圈矩形环 y: 32.18 .. 32.55）
    g.box(b, "ring1_f", (-4.35, 32.18, -4.42), (4.35, 32.55, -3.85), "linen_dk")
    g.box(b, "ring1_b", (-4.35, 32.18, 3.85), (4.35, 32.55, 4.42), "linen_lt")
    g.pair("turban_side", "ring1_side", (3.88, 32.15, -3.80), (4.44, 32.52, 3.80), "linen", sided=True)
    g.pair("turban_side", "ring1_corner_f", (3.75, 32.22, -4.38), (4.38, 32.65, -3.75), "linen_lt", sided=True)
    g.pair("turban_side", "ring1_corner_b", (3.75, 32.22, 3.75), (4.38, 32.65, 4.38), "linen_dk", sided=True)

    # ── 第 2 盘（次外圈圆丘环 y: 32.58 .. 32.96）
    g.box(b, "ring2_f", (-3.65, 32.58, -3.72), (3.65, 32.96, -2.85), "linen_lt")
    g.box(b, "ring2_b", (-3.65, 32.58, 2.85), (3.65, 32.96, 3.72), "linen_dk")
    g.pair("turban_side", "ring2_side", (2.88, 32.60, -2.80), (3.72, 32.98, 2.80), "linen", sided=True)
    g.box(b, "ring2_diag_1", (-2.85, 32.68, -2.82), (2.85, 33.05, -0.65), "linen_dk")
    g.box(b, "ring2_diag_2", (-2.85, 32.68, 0.65), (2.85, 33.05, 2.82), "linen_lt")
    g.pair("turban_side", "ring2_accent_1", (1.85, 32.72, -3.45), (3.25, 33.08, -2.65), "linen", sided=True)
    g.pair("turban_side", "ring2_accent_2", (1.85, 32.72, 2.65), (3.25, 33.08, 3.45), "linen_lt", sided=True)

    # ── 第 3 盘（内圈隆起圆包 y: 33.05 .. 33.52）
    g.box(b, "ring3_core", (-2.25, 33.05, -2.25), (2.25, 33.48, 2.25), "linen")
    g.box(b, "ring3_crest_f", (-1.95, 33.15, -2.35), (1.95, 33.56, -1.15), "linen_lt")
    g.box(b, "ring3_crest_b", (-1.95, 33.15, 1.15), (1.95, 33.56, 2.35), "linen_dk")
    g.pair("turban_side", "ring3_crest_side", (1.25, 33.18, -1.82), (2.35, 33.58, 1.82), "linen", sided=True)
    g.pair("turban_side", "ring3_flank_puff", (0.85, 33.25, -2.15), (2.15, 33.68, -1.25), "linen_dk", sided=True)

    # ── 第 4 盘（顶端布结与散乱碎毛 y: 33.58 .. 34.42）
    g.box(b, "crown_knot_bun", (-1.15, 33.58, -1.15), (1.15, 34.02, 1.15), "linen_dk")
    g.box(b, "crown_knot_tie", (-1.35, 33.65, -0.62), (1.35, 33.92, 0.62), "cord")
    g.box(b, "crown_knot_wrap", (-0.62, 33.72, -1.35), (0.62, 33.98, 1.35), "cord_dk")
    g.box(b, "crown_tuft_mid", (-0.45, 34.02, -0.45), (0.45, 34.42, 0.45), "linen_lt")
    g.pair("turban_crown", "crown_tuft_pair", (0.35, 33.92, -0.25), (0.75, 34.32, 0.25), "linen")
    g.pair("turban_side", "crown_diag_accent1", (1.45, 32.72, -1.8), (2.65, 33.15, -1.1), "linen", sided=True)
    g.pair("turban_side", "crown_diag_accent2", (1.45, 32.72, 1.1), (2.65, 33.15, 1.8), "linen_lt", sided=True)
    g.pair("turban_crown", "crown_corner_shred", (2.45, 32.65, -2.45), (3.15, 33.08, -1.75), "linen_dk")


def part_turban_brow(g: Gear) -> None:
    """额前 5 层错位横缠布圈与眉间立体折痕：
    y ∈ [26.8, 32.08], z ∈ [-4.88, -4.08]。
    """
    b = "turban_brow"
    # ── Layer 1（眉弓下沿 y: 26.82 .. 27.80）
    g.box(b, "brow_l1_c", (-2.65, 26.82, -4.56), (2.65, 27.78, -4.12), "linen_lt")
    g.pair(b, "brow_l1_wing", (2.68, 26.85, -4.54), (3.85, 27.78, -4.12), "linen_dk")
    g.box(b, "brow_l1_crest", (-1.45, 27.05, -4.72), (1.45, 27.65, -4.35), "linen")
    g.pair(b, "brow_l1_tooth", (1.65, 26.55, -4.65), (2.35, 27.12, -4.25), "linen_lt")

    # ── Layer 2（主额下带 y: 27.84 .. 28.82）
    g.box(b, "brow_l2_c", (-2.85, 27.84, -4.60), (2.85, 28.82, -4.12), "linen")
    g.pair(b, "brow_l2_wing", (2.88, 27.86, -4.58), (4.05, 28.82, -4.12), "linen_lt")
    g.box(b, "brow_l2_crest", (-1.65, 28.12, -4.75), (1.65, 28.62, -4.38), "linen_lt")
    g.pair(b, "brow_l2_pleat", (2.15, 28.02, -4.68), (3.45, 28.68, -4.28), "linen_dk")
    g.pair(b, "brow_l2_accent", (1.25, 28.25, -4.76), (2.45, 28.66, -4.32), "linen")

    # ── Layer 3（中额隆起带 y: 28.86 .. 29.84）
    g.box(b, "brow_l3_c", (-2.75, 28.86, -4.64), (2.75, 29.84, -4.12), "linen_dk")
    g.pair(b, "brow_l3_wing", (2.84, 28.88, -4.62), (3.95, 29.84, -4.12), "linen")
    g.box(b, "brow_l3_crest", (-1.55, 29.08, -4.78), (1.55, 29.62, -4.42), "linen")
    g.pair(b, "brow_l3_accent", (2.25, 29.05, -4.70), (3.55, 29.68, -4.36), "linen_lt")
    g.pair(b, "brow_l3_flank_fold", (2.85, 29.28, -4.66), (3.85, 29.75, -4.24), "linen_dk")

    # ── Layer 4（上额厚卷带 y: 29.88 .. 30.88）
    g.box(b, "brow_l4_c", (-2.85, 29.88, -4.58), (2.85, 30.88, -4.12), "linen")
    g.pair(b, "brow_l4_wing", (2.88, 29.90, -4.56), (3.85, 30.88, -4.12), "linen_dk")
    g.box(b, "brow_l4_crest", (-1.65, 30.12, -4.75), (1.65, 30.68, -4.35), "linen_lt")
    g.pair(b, "brow_l4_pleat", (2.15, 30.05, -4.68), (3.35, 30.72, -4.26), "linen")

    # ── Layer 5（顶边卷边 y: 30.92 .. 32.05）
    g.box(b, "brow_l5_c", (-2.45, 30.92, -4.54), (2.45, 32.05, -4.14), "linen_dk")
    g.pair(b, "brow_l5_wing", (2.48, 30.94, -4.52), (3.75, 32.05, -4.14), "linen_lt")
    g.box(b, "brow_l5_crest", (-1.35, 31.22, -4.68), (1.35, 31.82, -4.32), "linen_lt")

    # 眉心中央纵向折波
    g.box(b, "brow_mid_ridge", (-0.55, 27.55, -4.80), (0.55, 30.85, -4.46), "linen_lt")
    g.pair(b, "brow_rag_tooth1", (0.65, 26.62, -4.68), (1.35, 27.18, -4.30), "linen_dk")
    g.pair(b, "brow_rag_tooth2", (2.85, 27.65, -4.70), (3.65, 28.22, -4.30), "linen_lt")
    g.pair(b, "brow_rag_tooth3", (0.85, 28.65, -4.74), (1.65, 29.22, -4.34), "linen")
    g.pair(b, "brow_rag_tooth4", (2.65, 30.65, -4.66), (3.42, 31.22, -4.32), "linen_lt")


def part_turban_sides(g: Gear) -> None:
    """两鬓与后脑 5 层错位缠裹层：
    两侧 x ∈ [±4.08, ±4.56]，后脑 z ∈ [4.08, 4.65]。
    """
    br = "turban_side"
    # ── 两侧 5 段斜向缠绕布条
    g.pair(br, "side_t1_main", (4.08, 26.85, -3.95), (4.45, 27.78, 3.95), "linen_lt", sided=True)
    g.pair(br, "side_t1_crest", (4.16, 27.05, -3.65), (4.52, 27.65, 3.65), "linen", sided=True)

    g.pair(br, "side_t2_main", (4.12, 27.86, -4.04), (4.48, 28.84, 4.04), "linen", sided=True)
    g.pair(br, "side_t2_crest", (4.20, 28.06, -3.75), (4.54, 28.66, 3.75), "linen_lt", sided=True)

    g.pair(br, "side_t3_main", (4.12, 28.88, -4.04), (4.48, 29.86, 4.04), "linen_dk", sided=True)
    g.pair(br, "side_t3_crest", (4.20, 29.10, -3.75), (4.54, 29.70, 3.75), "linen", sided=True)

    g.pair(br, "side_t4_main", (4.12, 29.90, -4.04), (4.48, 30.90, 4.04), "linen_lt", sided=True)
    g.pair(br, "side_t4_crest", (4.20, 30.12, -3.75), (4.54, 30.72, 3.75), "linen_dk", sided=True)

    g.pair(br, "side_t5_main", (4.08, 30.94, -3.95), (4.45, 32.05, 3.95), "linen_dk", sided=True)
    g.pair(br, "side_t5_crest", (4.16, 31.16, -3.65), (4.52, 31.85, 3.65), "linen_lt", sided=True)

    # 两鬓散落布角
    g.pair(br, "side_rag_f1", (4.24, 27.55, -3.85), (4.55, 28.35, -2.85), "linen_lt", sided=True)
    g.pair(br, "side_rag_f2", (4.22, 29.55, -3.85), (4.55, 30.35, -2.85), "linen", sided=True)
    g.pair(br, "side_rag_b1", (4.25, 28.25, 2.55), (4.55, 29.05, 3.65), "linen_dk", sided=True)
    g.pair(br, "side_rag_b2", (4.22, 30.25, 2.55), (4.55, 31.05, 3.65), "linen_lt", sided=True)
    g.pair(br, "side_accent_strap", (4.25, 28.35, -1.25), (4.56, 29.45, 1.25), "linen_dk", sided=True)
    g.pair(br, "side_extra_pleat1", (4.24, 27.15, -2.5), (4.54, 27.85, -1.5), "linen", sided=True)
    g.pair(br, "side_extra_pleat2", (4.26, 29.15, -2.48), (4.52, 29.85, -1.5), "linen_lt", sided=True)
    g.pair(br, "side_extra_pleat3", (4.25, 27.15, 1.5), (4.54, 27.85, 2.5), "linen_dk", sided=True)
    g.pair(br, "side_extra_pleat4", (4.26, 29.15, 1.5), (4.52, 29.85, 2.5), "linen", sided=True)
    g.pair(br, "side_ear_rag", (4.26, 26.55, -0.6), (4.58, 27.25, 0.6), "linen_lt", sided=True)

    # ── 后脑 5 层横向布带
    bk = "turban_back"
    g.box(bk, "back_t1_main", (-4.12, 26.85, 4.08), (4.12, 27.82, 4.45), "linen")
    g.box(bk, "back_t1_crest", (-3.55, 27.05, 4.22), (3.55, 27.65, 4.56), "linen_lt")

    g.box(bk, "back_t2_main", (-4.15, 27.86, 4.08), (4.15, 28.84, 4.48), "linen_dk")
    g.box(bk, "back_t2_crest", (-3.65, 28.06, 4.25), (3.65, 28.66, 4.60), "linen")

    g.box(bk, "back_t3_main", (-4.15, 28.88, 4.08), (4.15, 29.86, 4.48), "linen")
    g.box(bk, "back_t3_crest", (-3.65, 29.10, 4.25), (3.65, 29.70, 4.60), "linen_lt")

    g.box(bk, "back_t4_main", (-4.15, 29.90, 4.08), (4.15, 30.90, 4.48), "linen_dk")
    g.box(bk, "back_t4_crest", (-3.65, 30.12, 4.25), (3.65, 30.72, 4.60), "linen")

    g.box(bk, "back_t5_main", (-4.12, 30.94, 4.08), (4.12, 32.05, 4.45), "linen")
    g.box(bk, "back_t5_crest", (-3.55, 31.16, 4.22), (3.55, 31.85, 4.56), "linen_lt")

    # 束紧细皮绳一圈
    g.box("turban_brow", "brow_cord_f", (-4.16, 27.42, -4.75), (4.16, 27.74, -4.50), "cord")
    g.pair(br, "side_cord", (4.28, 27.40, -4.02), (4.58, 27.76, 4.02), "cord", sided=True)
    g.box(bk, "back_cord_b", (-4.16, 27.42, 4.48), (4.16, 27.78, 4.75), "cord")


def part_bone_hairpin(g: Gear) -> None:
    """额前斜插的雕工兽骨簪（不对称挂件，位于 bone_pin_mount 骨骼）。
    斜横在额头前：x ∈ [-3.4, 3.7], y ∈ [28.95, 30.05], z ∈ [-4.94, -4.24]。
    长宽厚 ≥ 0.65px，供穿模差分自证抓取。
    """
    bp = "bone_pin_mount"
    # 簪头（左侧粗端，带簪首雕花圆环）
    g.box(bp, "pin_head_bulb", (-3.35, 29.22, -4.88), (-2.55, 30.02, -4.22), "bone")
    g.box(bp, "pin_head_eye", (-3.12, 29.44, -4.92), (-2.78, 29.80, -4.18), "void")
    g.box(bp, "pin_head_crest", (-3.42, 29.35, -4.90), (-2.48, 29.85, -4.26), "bone_lt")
    g.box(bp, "pin_head_ring1", (-3.46, 29.15, -4.76), (-3.22, 30.10, -4.34), "bone_dk")
    g.box(bp, "pin_head_ring2", (-2.68, 29.15, -4.76), (-2.44, 30.10, -4.34), "bone_dk")

    # 簪身（横贯额前）
    g.box(bp, "pin_shaft_main", (-2.55, 29.22, -4.82), (2.2, 29.72, -4.24), "bone")
    g.box(bp, "pin_shaft_groove", (-2.0, 29.36, -4.86), (1.5, 29.58, -4.20), "bone_dk")
    g.box(bp, "pin_shaft_ridge", (-1.2, 29.32, -4.90), (0.8, 29.64, -4.18), "bone_lt")

    # 簪尾尖（右侧磨尖锥形出梢）
    g.box(bp, "pin_tip_taper", (2.2, 29.25, -4.76), (3.15, 29.62, -4.30), "bone_lt")
    g.box(bp, "pin_tip_point", (3.15, 29.30, -4.68), (3.65, 29.56, -4.38), "bone_lt")


def part_cloth_tail(g: Gear) -> None:
    """右后耳侧垂落的两条长短不一的参差布尾（长尾垂至肩头，短尾飘散在颈侧）。"""
    # ── 1. 长垂布尾（骨骼 cloth_tail_long 及其子骨骼，垂至 y ≈ 18.0）
    tl = "cloth_tail_long"
    g.box(tl, "tail_long_root", (4.18, 25.55, 3.32), (4.85, 28.52, 4.32), "linen")
    g.box(tl, "tail_long_r_pleat", (4.25, 25.85, 3.48), (4.92, 28.22, 4.16), "linen_dk")
    g.box(tl, "tail_long_r_hem", (4.22, 27.22, 3.35), (4.88, 28.45, 4.28), "linen_lt")

    # 锈铁束扣（hang_iron_clasp 骨骼，尺寸长宽厚 ≥ 0.65px，用于穿模门差分自证）
    ic = "hang_iron_clasp"
    g.box(ic, "iron_clasp_base", (4.22, 26.15, 3.42), (4.95, 26.85, 4.22), "rust")
    g.box(ic, "iron_clasp_core", (4.32, 26.25, 3.52), (4.98, 26.75, 4.12), "rust")

    # 长尾中段（骨骼 tail_long_mid, y: 22.0 .. 25.55）
    tm = "tail_long_mid"
    g.box(tm, "tail_long_mid_body", (4.18, 22.05, 3.35), (4.85, 25.55, 4.25), "linen_dk")
    g.box(tm, "tail_long_mid_pleat", (4.25, 22.42, 3.5), (4.92, 25.22, 4.1), "linen")
    g.box(tm, "tail_long_mid_crest", (4.30, 22.82, 3.58), (4.95, 24.82, 4.05), "linen_lt")
    g.box(tm, "tail_long_mid_rip", (4.28, 23.22, 3.65), (4.88, 24.22, 3.95), "void")

    # 长尾末段毛边（骨骼 tail_long_tip, y: 18.2 .. 22.05）
    tt = "tail_long_tip"
    g.box(tt, "tail_long_tip_body", (4.18, 19.55, 3.4), (4.85, 22.05, 4.2), "linen")
    g.box(tt, "tail_long_tip_pleat", (4.24, 19.82, 3.55), (4.90, 21.62, 4.08), "linen_dk")
    g.box(tt, "tail_long_tip_fray1", (4.22, 18.52, 3.45), (4.82, 19.55, 3.8), "linen_lt")
    g.box(tt, "tail_long_tip_fray2", (4.22, 18.82, 3.85), (4.82, 19.55, 4.15), "linen_dk")
    g.box(tt, "tail_long_tip_fringe", (4.25, 18.18, 3.58), (4.78, 18.82, 3.98), "linen_lt")

    # ── 2. 短垂布尾（骨骼 cloth_tail_short，垂至 y ≈ 21.5）
    ts = "cloth_tail_short"
    g.box(ts, "tail_short_root", (4.20, 25.22, 2.35), (4.78, 27.82, 3.25), "linen_dk")
    g.box(ts, "tail_short_pleat", (4.22, 25.52, 2.45), (4.84, 27.52, 3.12), "linen_lt")
    g.box(ts, "tail_short_mid", (4.16, 22.82, 2.38), (4.76, 25.22, 3.18), "linen")
    g.box(ts, "tail_short_fray1", (4.20, 21.82, 2.42), (4.72, 22.82, 2.82), "linen_lt")
    g.box(ts, "tail_short_fray2", (4.20, 22.12, 2.85), (4.72, 22.82, 3.15), "linen_dk")
    g.box(ts, "tail_short_tassel", (4.22, 21.25, 2.45), (4.68, 21.85, 3.05), "linen_lt")
    g.box(tt, "tail_long_fringe_sub", (4.26, 17.65, 3.48), (4.74, 18.22, 3.88), "linen_dk")
    g.box(tt, "tail_long_fray3", (4.28, 18.45, 3.88), (4.76, 19.15, 4.18), "linen")
    g.box(ts, "tail_short_fray3", (4.24, 21.45, 2.85), (4.66, 22.05, 3.15), "linen")
    g.pair("turban_side", "side_buckle_bone", (4.25, 27.25, -0.45), (4.65, 27.95, 0.45), "bone", sided=True)
    g.pair("turban_side", "side_buckle_pin", (4.30, 27.44, -0.15), (4.70, 27.80, 0.15), "bone_dk", sided=True)
    g.pair("turban_brow", "brow_stitch_accent1", (0.85, 27.45, -4.78), (1.65, 27.75, -4.42), "stitch")
    g.pair("turban_brow", "brow_cross_accent1", (0.92, 29.35, -4.74), (1.52, 29.65, -4.45), "stitch")
    g.pair("turban_brow", "brow_cross_accent2", (2.72, 29.38, -4.74), (3.32, 29.65, -4.48), "stitch")
    g.pair("turban_crown", "crown_rib_cross1", (0.85, 32.75, -2.15), (1.55, 33.12, -1.45), "linen_lt")
    g.pair("turban_crown", "crown_rib_cross2", (0.85, 32.75, 1.45), (1.55, 33.12, 2.15), "linen_dk")
    g.pair("turban_side", "side_rib_tie", (4.26, 30.45, -1.15), (4.56, 30.78, 1.15), "cord_dk", sided=True)
    g.pair("turban_brow", "brow_stitch_accent2", (2.82, 28.45, -4.78), (3.62, 28.75, -4.40), "stitch")
    g.pair("turban_brow", "brow_stitch_accent3", (0.88, 30.45, -4.78), (1.62, 30.75, -4.42), "stitch")
    g.pair("turban_crown", "crown_twist_accent1", (1.28, 33.35, -1.25), (1.88, 33.75, -0.65), "linen_lt")
    g.pair("turban_crown", "crown_twist_accent2", (1.28, 33.35, 0.65), (1.88, 33.75, 1.25), "linen_dk")


def part_turban_details(g: Gear) -> None:
    """额角废皮补丁、粗麻粗线迹与加固结构。"""
    tp = "turban_patch"
    # 左额角废皮补丁 (x ∈ [-3.65, -1.88], y ∈ [28.62, 31.12])
    g.box(tp, "patch_brow_base", (-3.65, 28.62, -4.72), (-1.88, 31.12, -4.38), "patch")
    g.box(tp, "patch_brow_worn", (-3.45, 28.85, -4.80), (-2.05, 30.78, -4.45), "patch_dk")
    # 补丁四周粗缝线迹
    for k, x in enumerate((-3.4, -2.8, -2.25)):
        g.box(tp, f"stitch_patch_top_{k}", (x, 30.98, -4.85), (x + 0.35, 31.28, -4.50), "stitch")
        g.box(tp, f"stitch_patch_bot_{k}", (x, 28.42, -4.85), (x + 0.35, 28.72, -4.50), "stitch")
    g.box(tp, "stitch_patch_l", (-3.75, 29.52, -4.85), (-3.45, 30.22, -4.50), "stitch")
    g.box(tp, "stitch_patch_r", (-2.05, 29.52, -4.85), (-1.75, 30.22, -4.50), "stitch")

    # 表面额外横纵交叉粗缝线
    for k, y in enumerate((28.52, 29.52, 30.52)):
        g.box(tp, f"turban_cross_stitch_{k}", (2.18, y, -4.75), (3.12, y + 0.35, -4.42), "stitch")
    for k, z in enumerate((-2.5, 0.0, 2.5)):
        g.box(tp, f"side_cross_stitch_{k}", (-4.55, 29.8, z), (-4.22, 30.15, z + 0.6), "stitch")


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
    wear_note="挂点：头（头盒 x∈[-4,4], y∈[24,32], z∈[-4,4]）+ 额前骨簪 + 侧后双垂布尾",
    worn_focus=((0.0, 27.5, 0.0), 20.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
