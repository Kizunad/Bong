#!/usr/bin/env python3
"""皮绳护腕（pi_sheng_hu_wan）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工：兽皮翻边、立体X形交叉缠绳、草绳扎圈与多针插鞘。

任务卡：末土装备 #14（.task-wasteland-gear.md）。
挂点：双前臂（双前臂 y∈[12.08, 17.5], 右臂 x∈[4,8], 左臂 x∈[-8,-4], z∈[-2,2]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  躯干 x∈[-4,4], y∈[12,24], z∈[-2,2]
  右臂 x∈[4,8], y∈[12,24], z∈[-2,2]
  左臂 x∈[-8,-4], y∈[12,24], z∈[-2,2]
  门禁 clearance：任何方块不得扎进手臂或躯干内部（咬入 ≤ 0.05px）。

外形：
  打破方框感：
  - 双小臂包裹硬质粗兽皮，本体分下、中、上三段错落交叠，带有不规则翻卷边缘；
  - 上下两端翻出粗毛与糙毛领边，带翘起的毛绺；
  - 上下两端各加一圈粗草绳扎圈，系死结；
  - 中段由双层凸起的交叉 X 形皮绳前后缠绕紧箍，绳结凸出；
  - 右臂外侧专配多层兽皮针鞘，斜插 3 根长短不一的磨尖骨针（非暗器，带穿线孔与刻纹）；
  - 左臂外侧带有破碎旧皮大补丁、粗麻缝线，以及腕底悬挂的小骨计数签。
  总方块数达到 210+，按挂点分骨 17 骨。
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "hide":         (118, 78, 56),     # 兽皮主色
    "hide_dk":      (84, 56, 42),      # 兽皮深色暗面
    "hide_lt":      (150, 108, 78),    # 兽皮亮面/边缘磨白
    "fur":          (186, 172, 148),   # 衬垫粗毛/糙毛
    "fur_dk":       (136, 122, 102),   # 毛皮阴影
    "cord":         (150, 126, 84),    # 捆绑皮绳/草绳
    "cord_dk":      (104, 84, 54),     # 绳结/阴影
    "bone":         (206, 194, 164),   # 骨扣与骨针
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨针暗槽/阴面
    "rust":         (146, 84, 50),     # 锈铁小孔钉
    "stitch":       (188, 176, 148),   # 粗缝线
    "cord_lt":      (178, 152, 104),   # 编绳高光
    "void":         (28, 24, 22),      # 针孔穿透暗部
}

SOFT = ("hide", "hide_dk", "hide_lt", "fur", "fur_dk", "cord", "cord_dk", "cord_lt", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨 ≥ 10）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("arm_r_mount", "root", (6.0, 14.0, 0.0), "右前臂安装基准骨"),
    ("bracer_cuff_r", "arm_r_mount", (6.0, 14.5, 0.0), "右护腕主皮筒"),
    ("bracer_fur_r", "arm_r_mount", (6.0, 14.5, 0.0), "右护腕毛皮衬边"),
    ("bracer_strap_r", "arm_r_mount", (6.0, 14.5, 0.0), "右护腕缠绕皮绳"),
    ("bracer_buckle_r", "arm_r_mount", (6.0, 14.5, 0.0), "右护腕骨扣固定件"),
    ("needle_sheath_r", "arm_r_mount", (8.3, 14.8, 0.0), "右护腕插针皮鞘"),
    ("bone_needle_a", "needle_sheath_r", (8.4, 15.2, -0.4), "右护腕小骨针A"),
    ("bone_needle_b", "needle_sheath_r", (8.4, 14.6, 0.4), "右护腕小骨针B"),
    ("bone_needle_c", "needle_sheath_r", (8.4, 14.2, 0.0), "右护腕小骨针C"),
    ("arm_l_mount", "root", (-6.0, 14.0, 0.0), "左前臂安装基准骨"),
    ("bracer_cuff_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕主皮筒"),
    ("bracer_fur_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕毛皮衬边"),
    ("bracer_strap_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕缠绕皮绳"),
    ("bracer_buckle_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕骨扣固定件"),
    ("patch_strap_l", "arm_l_mount", (-8.3, 14.5, 0.0), "左护腕修补皮片与缝线"),
    ("hang_bone_tally", "arm_l_mount", (-6.0, 12.5, 2.2), "左腕下挂小骨签"),
]

ASYM_BONES = frozenset([
    "needle_sheath_r", "bone_needle_a", "bone_needle_b", "bone_needle_c",
    "patch_strap_l", "hang_bone_tally",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_cuff_base(g: Gear) -> None:
    """左右护腕主皮筒：三层阶梯包裹，厚度错落。
    前后与外侧包覆小臂。前侧 z ∈ [-2.38, -2.08]，后侧 z ∈ [2.08, 2.38]，外侧 x ∈ [8.08, 8.42]。
    """
    b = "bracer_cuff"
    # 下段 (y: 12.55 .. 14.05)
    g.pair(b, "cuff_lo_f", (4.08, 12.55, -2.35), (8.02, 14.02, -2.08), "hide", sided=True)
    g.pair(b, "cuff_lo_f_crest", (4.75, 12.82, -2.48), (7.35, 13.82, -2.24), "hide_lt", sided=True)
    g.pair(b, "cuff_lo_b", (4.08, 12.55, 2.08), (8.02, 14.02, 2.35), "hide", sided=True)
    g.pair(b, "cuff_lo_b_crest", (4.75, 12.82, 2.24), (7.35, 13.82, 2.48), "hide_lt", sided=True)
    g.pair(b, "cuff_lo_out", (8.08, 12.55, -2.04), (8.38, 14.02, 2.04), "hide_dk", sided=True)

    # 中段 (y: 14.05 .. 15.55，稍厚)
    g.pair(b, "cuff_mid_f", (4.08, 14.06, -2.38), (8.02, 15.52, -2.08), "hide_dk", sided=True)
    g.pair(b, "cuff_mid_f_crest", (4.65, 14.32, -2.46), (7.55, 15.22, -2.24), "hide", sided=True)
    g.pair(b, "cuff_mid_b", (4.08, 14.06, 2.08), (8.02, 15.52, 2.38), "hide_dk", sided=True)
    g.pair(b, "cuff_mid_b_crest", (4.65, 14.32, 2.24), (7.55, 15.22, 2.46), "hide", sided=True)
    g.pair(b, "cuff_mid_out", (8.08, 14.06, -2.04), (8.42, 15.52, 2.04), "hide", sided=True)

    # 上段 (y: 15.55 .. 16.75)
    g.pair(b, "cuff_hi_f", (4.08, 15.56, -2.35), (8.02, 16.72, -2.08), "hide", sided=True)
    g.pair(b, "cuff_hi_f_crest", (4.75, 15.82, -2.44), (7.35, 16.52, -2.22), "hide_lt", sided=True)
    g.pair(b, "cuff_hi_b", (4.08, 15.56, 2.08), (8.02, 16.72, 2.35), "hide", sided=True)
    g.pair(b, "cuff_hi_b_crest", (4.75, 15.82, 2.22), (7.35, 16.52, 2.44), "hide_lt", sided=True)
    g.pair(b, "cuff_hi_out", (8.08, 15.56, -2.04), (8.38, 16.72, 2.04), "hide_dk", sided=True)

    # 外前角与外后角斜切过渡
    g.pair(b, "cuff_corner_fa", (8.09, 12.65, -2.42), (8.36, 16.65, -2.08), "hide", sided=True)
    g.pair(b, "cuff_corner_ba", (8.09, 12.65, 2.08), (8.36, 16.65, 2.42), "hide", sided=True)
    # 边缘翻卷皮牙
    g.pair(b, "cuff_flap_top", (6.4, 16.76, -2.44), (7.8, 17.06, -2.16), "hide_lt", sided=True)
    g.pair(b, "cuff_flap_bot", (4.5, 12.28, -2.44), (5.8, 12.50, -2.16), "hide_dk", sided=True)
    g.pair(b, "cuff_flap_back", (6.2, 16.76, 2.16), (7.6, 17.06, 2.44), "hide_lt", sided=True)
    g.pair(b, "cuff_fringe_f1", (4.15, 12.32, -2.48), (4.48, 12.58, -2.20), "hide", sided=True)
    g.pair(b, "cuff_fringe_f2", (7.1, 12.32, -2.44), (7.7, 12.58, -2.18), "hide_lt", sided=True)
    g.pair(b, "cuff_fringe_b1", (4.15, 12.32, 2.20), (4.48, 12.58, 2.48), "hide", sided=True)
    g.pair(b, "cuff_fringe_b2", (7.1, 12.32, 2.18), (7.7, 12.58, 2.44), "hide_lt", sided=True)
    g.pair(b, "cuff_seam_f1", (4.8, 14.04, -2.42), (5.5, 14.32, -2.12), "stitch", sided=True)
    g.pair(b, "cuff_seam_f2", (6.8, 14.04, -2.42), (7.5, 14.32, -2.12), "stitch", sided=True)
    g.pair(b, "cuff_seam_b1", (4.8, 14.04, 2.12), (5.5, 14.32, 2.42), "stitch", sided=True)
    g.pair(b, "cuff_seam_b2", (6.8, 14.04, 2.12), (7.5, 14.32, 2.42), "stitch", sided=True)
    g.pair(b, "cuff_strap_knot_f", (6.2, 13.75, -2.72), (6.8, 14.15, -2.25), "cord", sided=True)
    g.pair(b, "cuff_strap_knot_b", (6.2, 13.75, 2.25), (6.8, 14.15, 2.72), "cord", sided=True)


def part_cuff_fur(g: Gear) -> None:
    """上下端翻出的粗毛与糙毛衬垫滚边，带参差毛绺。"""
    b = "bracer_fur"
    # 上毛边：前、外、后
    g.pair(b, "fur_top_f", (4.06, 16.74, -2.48), (8.04, 17.22, -2.12), "fur", sided=True)
    g.pair(b, "fur_top_f_crest", (4.65, 16.94, -2.56), (7.55, 17.32, -2.25), "fur_dk", sided=True)
    g.pair(b, "fur_top_out", (8.06, 16.74, -2.22), (8.52, 17.22, 2.22), "fur", sided=True)
    g.pair(b, "fur_top_out_tuft", (8.15, 17.15, -1.45), (8.58, 17.48, 1.45), "fur_dk", sided=True)
    g.pair(b, "fur_top_b", (4.06, 16.74, 2.12), (8.04, 17.22, 2.48), "fur", sided=True)
    g.pair(b, "fur_top_b_crest", (4.65, 16.94, 2.25), (7.55, 17.32, 2.56), "fur_dk", sided=True)

    # 下毛边：前、外、后（手腕端）
    g.pair(b, "fur_bot_f", (4.06, 12.08, -2.46), (8.04, 12.54, -2.12), "fur", sided=True)
    g.pair(b, "fur_bot_f_tuft1", (4.45, 11.75, -2.52), (5.75, 12.18, -2.22), "fur_dk", sided=True)
    g.pair(b, "fur_bot_f_tuft2", (6.45, 11.75, -2.52), (7.75, 12.18, -2.22), "fur", sided=True)
    g.pair(b, "fur_bot_out", (8.06, 12.08, -2.22), (8.48, 12.54, 2.22), "fur", sided=True)
    g.pair(b, "fur_bot_out_tuft", (8.15, 11.72, -1.35), (8.54, 12.12, 1.35), "fur_dk", sided=True)
    g.pair(b, "fur_bot_b", (4.06, 12.08, 2.12), (8.04, 12.54, 2.46), "fur", sided=True)
    g.pair(b, "fur_bot_b_tuft1", (4.45, 11.75, 2.22), (5.75, 12.18, 2.52), "fur_dk", sided=True)
    g.pair(b, "fur_bot_b_tuft2", (6.45, 11.75, 2.22), (7.75, 12.18, 2.52), "fur", sided=True)


def part_cuff_straps(g: Gear) -> None:
    """两端各一圈粗草绳扎圈 + 中段前后凸起 X 形交叉缠绕皮绳。"""
    b = "bracer_strap"
    # ── 上草绳扎圈 (y: 16.12 .. 16.62)
    g.pair(b, "straw_cord_top_f", (4.05, 16.12, -2.52), (8.05, 16.62, -2.20), "cord", sided=True)
    g.pair(b, "straw_cord_top_out", (8.15, 16.10, -2.28), (8.56, 16.60, 2.28), "cord", sided=True)
    g.pair(b, "straw_cord_top_b", (4.05, 16.12, 2.20), (8.05, 16.62, 2.52), "cord", sided=True)
    g.pair(b, "straw_cord_top_knot", (5.85, 16.02, -2.64), (6.55, 16.68, -2.25), "cord_dk", sided=True)
    g.pair(b, "straw_cord_top_tail", (6.05, 15.55, -2.66), (6.35, 16.08, -2.26), "cord", sided=True)

    # ── 下草绳扎圈 (y: 12.65 .. 13.15)
    g.pair(b, "straw_cord_bot_f", (4.05, 12.65, -2.52), (8.05, 13.15, -2.20), "cord", sided=True)
    g.pair(b, "straw_cord_bot_out", (8.15, 12.68, -2.28), (8.56, 13.18, 2.28), "cord", sided=True)
    g.pair(b, "straw_cord_bot_b", (4.05, 12.65, 2.20), (8.05, 13.15, 2.52), "cord", sided=True)
    g.pair(b, "straw_cord_bot_knot", (5.85, 12.52, -2.64), (6.55, 13.22, -2.25), "cord_dk", sided=True)
    g.pair(b, "straw_cord_bot_tail", (6.05, 12.05, -2.62), (6.35, 12.58, -2.26), "cord", sided=True)

    # ── 前面凸起 X 形交叉皮绳 (中段 y: 13.25 .. 16.02)
    # 斜向条 1（左下到右上）
    g.pair(b, "cross_f_x1_lo", (4.55, 13.28, -2.56), (5.85, 14.54, -2.22), "cord", sided=True)
    g.pair(b, "cross_f_x1_hi", (6.45, 14.86, -2.56), (7.65, 16.02, -2.26), "cord", sided=True)
    # 斜向条 2（右下到左上，压在上面）
    g.pair(b, "cross_f_x2_full", (4.58, 13.26, -2.62), (7.62, 16.04, -2.34), "cord_dk", sided=True)
    # 交叉中心锁扣与凸节
    g.pair(b, "cross_f_hub", (5.75, 14.36, -2.70), (6.45, 14.94, -2.28), "cord", sided=True)
    g.pair(b, "cross_f_tie", (5.92, 14.16, -2.68), (6.28, 15.14, -2.32), "cord_dk", sided=True)
    # 前向交叉附加压筋
    g.pair(b, "cross_f_reinforce", (5.15, 13.85, -2.66), (7.05, 15.45, -2.36), "cord_dk", sided=True)
    g.pair(b, "cross_f_accent1", (4.82, 14.75, -2.64), (5.62, 15.65, -2.32), "cord_lt", sided=True)
    g.pair(b, "cross_f_accent2", (6.55, 13.55, -2.64), (7.38, 14.45, -2.32), "cord_lt", sided=True)
    g.pair(b, "cross_b_accent1", (4.82, 14.75, 2.32), (5.62, 15.65, 2.64), "cord_lt", sided=True)
    g.pair(b, "cross_b_accent2", (6.55, 13.55, 2.32), (7.38, 14.45, 2.64), "cord_lt", sided=True)

    # ── 后面凸起 X 形交叉皮绳 (中段 y: 13.25 .. 16.02)
    g.pair(b, "cross_b_x1_lo", (4.55, 13.28, 2.22), (5.85, 14.54, 2.56), "cord", sided=True)
    g.pair(b, "cross_b_x1_hi", (6.45, 14.86, 2.26), (7.65, 16.02, 2.56), "cord", sided=True)
    g.pair(b, "cross_b_x2_full", (4.58, 13.26, 2.34), (7.62, 16.04, 2.62), "cord_dk", sided=True)
    g.pair(b, "cross_b_hub", (5.75, 14.36, 2.28), (6.45, 14.94, 2.70), "cord", sided=True)
    g.pair(b, "cross_b_tie", (5.92, 14.16, 2.32), (6.28, 15.14, 2.68), "cord_dk", sided=True)
    g.pair(b, "cross_b_reinforce", (5.15, 13.85, 2.36), (7.05, 15.45, 2.66), "cord_dk", sided=True)

    # 外侧纵向拉紧皮带
    g.pair(b, "outer_strap_stay", (8.22, 13.18, -0.40), (8.58, 16.05, 0.40), "cord", sided=True)
    g.pair(b, "outer_stay_rib", (8.34, 13.48, -0.20), (8.65, 15.78, 0.20), "cord_dk", sided=True)


def part_cuff_buckles(g: Gear) -> None:
    """前后与外侧的固定兽骨扣。"""
    b = "bracer_buckle"
    # 前端上骨扣
    g.pair(b, "buckle_f_top", (4.85, 15.85, -2.66), (5.75, 16.22, -2.26), "bone", sided=True)
    g.pair(b, "buckle_f_top_pin", (5.18, 15.65, -2.70), (5.42, 16.38, -2.28), "bone_dk", sided=True)

    # 前端下骨扣
    g.pair(b, "buckle_f_bot", (4.85, 13.12, -2.66), (5.75, 13.52, -2.28), "bone", sided=True)
    g.pair(b, "buckle_f_bot_pin", (5.18, 12.95, -2.70), (5.42, 13.68, -2.30), "bone_dk", sided=True)

    # 后端上骨扣
    g.pair(b, "buckle_b_top", (4.85, 15.85, 2.26), (5.75, 16.22, 2.66), "bone", sided=True)
    g.pair(b, "buckle_b_top_pin", (5.18, 15.65, 2.28), (5.42, 16.38, 2.70), "bone_dk", sided=True)

    # 后端下骨扣
    g.pair(b, "buckle_b_bot", (4.85, 13.12, 2.28), (5.75, 13.52, 2.66), "bone", sided=True)
    g.pair(b, "buckle_b_bot_pin", (5.18, 12.95, 2.30), (5.42, 13.68, 2.70), "bone_dk", sided=True)

    # 外侧中心大菱形兽骨扣（长宽高 ≥ 0.65px，供穿模差分自证）
    g.pair(b, "buckle_out_base", (8.18, 14.18, -0.66), (8.85, 14.98, 0.66), "bone", sided=True)
    g.pair(b, "buckle_out_core", (8.32, 14.38, -0.42), (8.88, 14.78, 0.42), "bone_lt", sided=True)
    g.pair(b, "buckle_out_peg", (8.28, 14.48, -0.16), (8.92, 14.72, 0.16), "bone_dk", sided=True)


def part_cuff_needles(g: Gear) -> None:
    """右护腕专有：外侧缝补插针皮鞘 + 3 根磨尖兽骨针（不对称挂件）。"""
    sh = "needle_sheath_r"
    # 插鞘基座
    g.box(sh, "sheath_base", (8.24, 13.28, -1.35), (8.60, 15.85, 1.35), "hide_dk")
    g.box(sh, "sheath_strap_top", (8.28, 15.12, -1.45), (8.68, 15.58, 1.45), "cord")
    g.box(sh, "sheath_strap_bot", (8.28, 13.62, -1.45), (8.68, 14.08, 1.45), "cord")
    g.box(sh, "sheath_slot_a", (8.48, 13.72, -0.85), (8.72, 15.42, -0.48), "void")
    g.box(sh, "sheath_slot_b", (8.48, 13.68, 0.38), (8.75, 15.38, 0.78), "void")
    g.box(sh, "sheath_slot_c", (8.48, 13.82, -0.22), (8.72, 15.22, 0.12), "void")

    # 骨针 A（前针，长针，厚度尺寸 ≥ 0.65px）
    na = "bone_needle_a"
    g.box(na, "needle_a_head", (8.42, 16.08, -0.72), (8.85, 16.78, -0.35), "bone")
    g.box(na, "needle_a_eye", (8.52, 16.32, -0.66), (8.88, 16.58, -0.42), "void")
    g.box(na, "needle_a_body", (8.44, 13.48, -0.68), (8.78, 16.08, -0.38), "bone")
    g.box(na, "needle_a_tip", (8.46, 12.65, -0.65), (8.74, 13.48, -0.42), "bone_lt")
    g.box(na, "needle_a_shade", (8.36, 13.88, -0.70), (8.62, 15.72, -0.45), "bone_dk")

    # 骨针 B（后针，中针）
    nb = "bone_needle_b"
    g.box(nb, "needle_b_head", (8.42, 15.72, 0.42), (8.82, 16.38, 0.75), "bone")
    g.box(nb, "needle_b_eye", (8.52, 15.92, 0.48), (8.86, 16.18, 0.68), "void")
    g.box(nb, "needle_b_body", (8.44, 13.35, 0.45), (8.78, 15.72, 0.72), "bone")
    g.box(nb, "needle_b_tip", (8.46, 12.62, 0.48), (8.74, 13.35, 0.68), "bone_lt")
    g.box(nb, "needle_b_shade", (8.36, 13.75, 0.42), (8.62, 15.42, 0.68), "bone_dk")

    # 骨针 C（中针，短针，位于 bone_needle_c）
    nc = "bone_needle_c"
    g.box(nc, "needle_c_head", (8.42, 15.35, -0.15), (8.78, 15.88, 0.12), "bone_lt")
    g.box(nc, "needle_c_body", (8.44, 13.75, -0.12), (8.75, 15.35, 0.08), "bone")
    g.box(nc, "needle_c_tip", (8.46, 13.15, -0.11), (8.72, 13.75, 0.12), "bone_lt")


def part_cuff_details(g: Gear) -> None:
    """左护腕专有：深色旧皮大补丁、密集缝线迹与腕底悬挂刻槽骨签。"""
    pl = "patch_strap_l"
    # 左臂外侧深色旧皮补丁
    g.box(pl, "patch_l_base", (-8.68, 13.45, -1.25), (-8.30, 15.75, 1.25), "hide_dk")
    g.box(pl, "patch_l_worn", (-8.74, 13.85, -0.85), (-8.40, 15.35, 0.85), "hide_lt")
    g.box(pl, "patch_l_corner_curl", (-8.78, 15.45, 0.85), (-8.45, 15.85, 1.28), "hide")
    # 密集缝线迹
    for k, z in enumerate((-1.05, -0.45, 0.15, 0.75)):
        g.box(pl, f"stitch_l_top_{k}", (-8.72, 15.58, z), (-8.36, 15.88, z + 0.3), "stitch")
        g.box(pl, f"stitch_l_bot_{k}", (-8.72, 13.35, z), (-8.36, 13.65, z + 0.3), "stitch")
    g.box(pl, "stitch_l_v1", (-8.72, 14.15, -1.18), (-8.36, 15.05, -0.88), "stitch")
    g.box(pl, "stitch_l_v2", (-8.72, 14.15, 0.82), (-8.36, 15.05, 1.12), "stitch")
    g.box(pl, "stitch_l_cross1", (-8.82, 14.42, -0.35), (-8.44, 14.82, -0.05), "stitch")
    g.box(pl, "stitch_l_cross2", (-8.82, 14.42, 0.05), (-8.44, 14.82, 0.35), "stitch")

    # 左腕后侧下挂小皮条骨记号签（尺寸厚度 ≥ 0.65px）
    ht = "hang_bone_tally"
    g.box(ht, "tally_loop", (-6.8, 12.45, 2.18), (-6.2, 13.05, 2.85), "cord_dk")
    g.box(ht, "tally_tag_main", (-6.9, 10.75, 2.18), (-6.1, 12.42, 2.85), "bone_dk")
    g.box(ht, "tally_tag_hole", (-6.65, 12.05, 2.15), (-6.35, 12.35, 2.88), "void")
    g.box(ht, "tally_tag_notch1", (-6.95, 11.55, 2.25), (-6.72, 11.85, 2.78), "bone")
    g.box(ht, "tally_tag_notch2", (-6.95, 11.15, 2.25), (-6.72, 11.45, 2.78), "bone")


PART_FUNCS = {
    "cuff_base": part_cuff_base,
    "cuff_fur": part_cuff_fur,
    "cuff_straps": part_cuff_straps,
    "cuff_buckles": part_cuff_buckles,
    "cuff_needles": part_cuff_needles,
    "cuff_details": part_cuff_details,
}

SPEC = GearSpec(
    gear_id="pi_sheng_hu_wan",
    model_name="PiShengHuWan",
    title="皮绳护腕 / pi_sheng_hu_wan",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：双前臂（双前臂 y∈[12.08, 17.5], 右臂 x∈[4,8], 左臂 x∈[-8,-4]）",
    worn_focus=((0.0, 14.5, 0.0), 20.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
