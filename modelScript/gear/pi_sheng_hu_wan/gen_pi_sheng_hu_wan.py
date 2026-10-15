#!/usr/bin/env python3
"""皮绳护腕（pi_sheng_hu_wan）Blockbench .bbmodel 程序化生成器 —— Round 1/3：几何骨架与部件。

任务卡：末土装备 #14（.task-wasteland-gear.md）。
挂点：双前臂（双前臂 y∈[12.08, 17.2], 右臂 x∈[4,8], 左臂 x∈[-8,-4], z∈[-2,2]）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
  脚底 y=0，玩家面朝 -z，+z 是背面，+x = 穿戴者右侧。
  躯干 x∈[-4,4], y∈[12,24], z∈[-2,2]
  右臂 x∈[4,8], y∈[12,24], z∈[-2,2]
  左臂 x∈[-8,-4], y∈[12,24], z∈[-2,2]
  门禁 clearance：任何方块不得扎进手臂或躯干内部（咬入 ≤ 0.05px）。

外形：
  双小臂包裹硬质兽皮衬垫与软熟兽皮，上下两端翻出泛白羊毛/糙毛衬边；
  外侧与前后侧缠绕多道交叉皮绳，配打磨兽骨扣固定；
  内侧留出微隙避免穿入身体，以交错皮绳系合；
  右臂外侧斜插两根磨尖小骨针（穿线缝补用，带穿孔与针槽，非暗器）；
  左臂带有皮革补片与粗麻加固缝线，腕下挂有一枚小骨片计数签。

分部件：
  - cuff_base: 左右护腕基础皮筒（前后外侧三面包裹）
  - cuff_fur: 上下端露出的粗毛糙皮滚边
  - cuff_straps: 交叉缠绕的捆扎皮绳与穿绳孔
  - cuff_buckles: 兽骨雕制的小梭形骨扣与系带结
  - cuff_needles: 右护腕专有的插针皮槽与双骨针（不对称）
  - cuff_details: 左护腕修补皮片、粗线迹与下挂小骨签（不对称）

用法：
    python3 modelScript/gear/pi_sheng_hu_wan/gen_pi_sheng_hu_wan.py --self-test   # 门禁差分自证
    python3 modelScript/gear/pi_sheng_hu_wan/gen_pi_sheng_hu_wan.py --part cuff_base # 单件预览
    python3 modelScript/gear/pi_sheng_hu_wan/gen_pi_sheng_hu_wan.py --worn        # 只出穿戴三面
    python3 modelScript/gear/pi_sheng_hu_wan/gen_pi_sheng_hu_wan.py               # 全量：模型 + check.png
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
    "cord":         (150, 126, 84),    # 捆绑皮绳/麻绳
    "cord_dk":      (104, 84, 54),     # 绳结/阴影
    "bone":         (206, 194, 164),   # 骨扣与骨针
    "bone_lt":      (224, 214, 186),   # 骨尖高光
    "bone_dk":      (156, 142, 112),   # 骨针暗槽/阴面
    "rust":         (146, 84, 50),     # 锈铁小孔钉
    "stitch":       (188, 176, 148),   # 粗缝线
    "void":         (28, 24, 22),      # 针孔穿透暗部
}

SOFT = ("hide", "hide_dk", "hide_lt", "fur", "fur_dk", "cord", "cord_dk", "stitch")
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
    ("arm_l_mount", "root", (-6.0, 14.0, 0.0), "左前臂安装基准骨"),
    ("bracer_cuff_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕主皮筒"),
    ("bracer_fur_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕毛皮衬边"),
    ("bracer_strap_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕缠绕皮绳"),
    ("bracer_buckle_l", "arm_l_mount", (-6.0, 14.5, 0.0), "左护腕骨扣固定件"),
    ("patch_strap_l", "arm_l_mount", (-8.3, 14.5, 0.0), "左护腕修补皮片与缝线"),
    ("hang_bone_tally", "arm_l_mount", (-6.0, 12.5, 2.2), "左腕下挂小骨签"),
]

ASYM_BONES = frozenset([
    "needle_sheath_r", "bone_needle_a", "bone_needle_b",
    "patch_strap_l", "hang_bone_tally",
])


# ───────────────────────────────────────────────────────────── 分部件实现
def part_cuff_base(g: Gear) -> None:
    """左右护腕主皮筒：前后与外侧包覆小臂。
    右臂：x ∈ [4.0, 8.0], y ∈ [12.0, 24.0], z ∈ [-2.0, 2.0]。
    护腕主体位于 y ∈ [12.5, 16.8]。
    为避开手臂内部 (咬入≤0.05px)，采用 C 形分面包裹：
      - 前面：z ∈ [-2.42, -2.08], x ∈ [4.05, 8.05]
      - 后面：z ∈ [2.08, 2.42], x ∈ [4.05, 8.05]
      - 外侧：x ∈ [8.08, 8.44], z ∈ [-2.18, 2.18]
      - 外前斜角过渡与外后斜角过渡
    全部使用 g.pair 镜像到左护腕（sided=True）。
    """
    b = "bracer_cuff"
    # 前主面 (z: -2.38 .. -2.08, y: 12.6 .. 16.7)
    g.pair(b, "cuff_front_main", (4.08, 12.6, -2.38), (8.02, 16.7, -2.08), "hide", sided=True)
    g.pair(b, "cuff_front_raised", (4.8, 13.0, -2.46), (7.4, 16.3, -2.25), "hide_lt", sided=True)
    g.pair(b, "cuff_front_dk", (4.2, 12.65, -2.42), (4.75, 16.65, -2.12), "hide_dk", sided=True)

    # 后主面 (z: 2.08 .. 2.38, y: 12.6 .. 16.7)
    g.pair(b, "cuff_back_main", (4.08, 12.6, 2.08), (8.02, 16.7, 2.38), "hide", sided=True)
    g.pair(b, "cuff_back_raised", (4.8, 13.0, 2.25), (7.4, 16.3, 2.46), "hide_lt", sided=True)
    g.pair(b, "cuff_back_dk", (4.2, 12.65, 2.12), (4.75, 16.65, 2.42), "hide_dk", sided=True)

    # 外侧主面 (x: 8.08 .. 8.42, y: 12.6 .. 16.7)
    g.pair(b, "cuff_outer_main", (8.08, 12.6, -2.06), (8.42, 16.7, 2.06), "hide", sided=True)
    g.pair(b, "cuff_outer_thick", (8.18, 13.0, -1.6), (8.48, 16.3, 1.6), "hide_dk", sided=True)

    # 外前角过渡包裹 (斜切圆角，x从8.09开始，z在-2.44..-2.08)
    g.pair(b, "cuff_corner_fa", (8.09, 12.62, -2.44), (8.38, 16.68, -2.08), "hide", sided=True)
    # 外后角过渡包裹
    g.pair(b, "cuff_corner_ba", (8.09, 12.62, 2.08), (8.38, 16.68, 2.44), "hide", sided=True)


def part_cuff_fur(g: Gear) -> None:
    """上下端翻出的粗毛与糙毛衬垫滚边。
    上端：y ∈ [16.65, 17.2]；下端：y ∈ [12.08, 12.65]。
    """
    b = "bracer_fur"
    # 上毛边：前、外、后
    g.pair(b, "fur_top_f", (4.05, 16.65, -2.46), (8.05, 17.15, -2.12), "fur", sided=True)
    g.pair(b, "fur_top_f_crest", (4.6, 16.85, -2.52), (7.6, 17.2, -2.25), "fur_dk", sided=True)
    g.pair(b, "fur_top_out", (8.05, 16.65, -2.22), (8.48, 17.15, 2.22), "fur", sided=True)
    g.pair(b, "fur_top_b", (4.05, 16.65, 2.12), (8.05, 17.15, 2.46), "fur", sided=True)
    g.pair(b, "fur_top_b_crest", (4.6, 16.85, 2.25), (7.6, 17.2, 2.52), "fur_dk", sided=True)

    # 下毛边：前、外、后（手腕端）
    g.pair(b, "fur_bot_f", (4.05, 12.12, -2.44), (8.05, 12.62, -2.12), "fur", sided=True)
    g.pair(b, "fur_bot_f_tuft", (4.8, 12.08, -2.5), (7.2, 12.45, -2.25), "fur_dk", sided=True)
    g.pair(b, "fur_bot_out", (8.05, 12.12, -2.2), (8.46, 12.62, 2.2), "fur", sided=True)
    g.pair(b, "fur_bot_b", (4.05, 12.12, 2.12), (8.05, 12.62, 2.44), "fur", sided=True)
    g.pair(b, "fur_bot_b_tuft", (4.8, 12.08, 2.25), (7.2, 12.45, 2.5), "fur_dk", sided=True)


def part_cuff_straps(g: Gear) -> None:
    """交叉缠绕的皮绳与固定带。"""
    b = "bracer_strap"
    # 上下两圈环绕紧箍皮带（前、外、后）
    # 上带 (y: 15.65 .. 16.25)
    g.pair(b, "band_top_f", (4.05, 15.65, -2.48), (8.05, 16.25, -2.18), "cord", sided=True)
    g.pair(b, "band_top_out", (8.15, 15.65, -2.25), (8.52, 16.25, 2.25), "cord", sided=True)
    g.pair(b, "band_top_b", (4.05, 15.65, 2.18), (8.05, 16.25, 2.48), "cord", sided=True)

    # 下带 (y: 13.05 .. 13.65)
    g.pair(b, "band_bot_f", (4.05, 13.05, -2.48), (8.05, 13.65, -2.18), "cord", sided=True)
    g.pair(b, "band_bot_out", (8.15, 13.05, -2.25), (8.52, 13.65, 2.25), "cord", sided=True)
    g.pair(b, "band_bot_b", (4.05, 13.05, 2.18), (8.05, 13.65, 2.48), "cord", sided=True)

    # 前面斜向交叉皮绳（十字交错）
    g.pair(b, "cross_f_diag1", (4.55, 13.35, -2.52), (7.65, 15.95, -2.24), "cord", sided=True)
    g.pair(b, "cross_f_diag2", (4.58, 13.38, -2.58), (7.62, 15.92, -2.28), "cord_dk", sided=True)
    g.pair(b, "cross_f_knot", (5.8, 14.38, -2.64), (6.4, 14.92, -2.22), "cord_dk", sided=True)

    # 后面斜向交叉皮绳
    g.pair(b, "cross_b_diag1", (4.55, 13.35, 2.24), (7.65, 15.95, 2.52), "cord", sided=True)
    g.pair(b, "cross_b_diag2", (4.58, 13.38, 2.28), (7.62, 15.92, 2.58), "cord_dk", sided=True)
    g.pair(b, "cross_b_knot", (5.8, 14.38, 2.22), (6.4, 14.92, 2.64), "cord_dk", sided=True)

    # 外侧垂直系拉筋绳
    g.pair(b, "outer_stay_cord", (8.22, 13.25, -0.38), (8.56, 16.02, 0.38), "cord", sided=True)


def part_cuff_buckles(g: Gear) -> None:
    """外侧与前后端固定的兽骨小扣件。"""
    b = "bracer_buckle"
    # 前端上骨扣
    g.pair(b, "buckle_f_top", (4.9, 15.75, -2.6), (5.7, 16.15, -2.2), "bone", sided=True)
    g.pair(b, "buckle_f_top_pin", (5.2, 15.55, -2.64), (5.4, 16.35, -2.22), "bone_dk", sided=True)

    # 前端下骨扣
    g.pair(b, "buckle_f_bot", (4.9, 13.15, -2.6), (5.7, 13.55, -2.2), "bone", sided=True)
    g.pair(b, "buckle_f_bot_pin", (5.2, 12.95, -2.64), (5.4, 13.75, -2.22), "bone_dk", sided=True)

    # 外侧中心大骨梭扣（菱形扣合块，厚度与高度均 ≥ 0.65px，供穿模门差分自证）
    g.pair(b, "buckle_out_center", (8.15, 14.15, -0.65), (8.82, 14.95, 0.65), "bone", sided=True)
    g.pair(b, "buckle_out_core", (8.32, 14.35, -0.42), (8.78, 14.75, 0.42), "bone_lt", sided=True)
    g.pair(b, "buckle_out_peg", (8.28, 14.42, -0.15), (8.75, 14.72, 0.15), "bone_dk", sided=True)


def part_cuff_needles(g: Gear) -> None:
    """右护腕专有：外侧缝补用小皮插鞘 + 两根磨尖兽骨针（不对称挂件）。"""
    # 插鞘基座位于 needle_sheath_r (x: 8.24 .. 8.7)
    sh = "needle_sheath_r"
    g.box(sh, "sheath_base", (8.24, 13.32, -1.25), (8.62, 15.85, 1.25), "hide_dk")
    g.box(sh, "sheath_strap_top", (8.28, 15.1, -1.35), (8.68, 15.55, 1.35), "cord")
    g.box(sh, "sheath_strap_bot", (8.28, 13.65, -1.35), (8.68, 14.1, 1.35), "cord")
    g.box(sh, "sheath_slot_a", (8.48, 13.75, -0.68), (8.75, 15.45, -0.18), "void")
    g.box(sh, "sheath_slot_b", (8.48, 13.72, 0.18), (8.75, 15.48, 0.68), "void")

    # 骨针 A（前针，稍微偏长）位于 bone_needle_a
    na = "bone_needle_a"
    g.box(na, "needle_a_head", (8.42, 16.05, -0.55), (8.78, 16.7, -0.25), "bone")
    g.box(na, "needle_a_eye", (8.52, 16.3, -0.52), (8.82, 16.55, -0.28), "void")
    g.box(na, "needle_a_body", (8.44, 13.55, -0.52), (8.74, 16.05, -0.28), "bone")
    g.box(na, "needle_a_tip", (8.46, 12.7, -0.5), (8.72, 13.55, -0.26), "bone_lt")
    g.box(na, "needle_a_shade", (8.36, 13.9, -0.54), (8.58, 15.75, -0.3), "bone_dk")

    # 骨针 B（后针，稍微偏短，倾斜插着）位于 bone_needle_b
    nb = "bone_needle_b"
    g.box(nb, "needle_b_head", (8.42, 15.75, 0.25), (8.78, 16.35, 0.55), "bone")
    g.box(nb, "needle_b_eye", (8.52, 15.95, 0.28), (8.82, 16.2, 0.52), "void")
    g.box(nb, "needle_b_body", (8.44, 13.38, 0.28), (8.74, 15.75, 0.52), "bone")
    g.box(nb, "needle_b_tip", (8.46, 12.6, 0.3), (8.72, 13.38, 0.54), "bone_lt")
    g.box(nb, "needle_b_shade", (8.36, 13.78, 0.3), (8.58, 15.42, 0.54), "bone_dk")


def part_cuff_details(g: Gear) -> None:
    """左护腕专有：外侧破皮补丁、粗麻缝线与腕底挂小骨签（不对称挂件）。"""
    pl = "patch_strap_l"
    # 左臂外侧深色旧皮补丁 (x ∈ [-8.72, -8.32], y ∈ [13.3, 15.7])
    g.box(pl, "patch_l_base", (-8.68, 13.5, -1.2), (-8.34, 15.7, 1.2), "hide_dk")
    g.box(pl, "patch_l_worn", (-8.74, 13.9, -0.8), (-8.4, 15.3, 0.8), "hide_lt")
    # 补丁四周粗缝线迹
    for k, z in enumerate((-1.0, -0.4, 0.2, 0.8)):
        g.box(pl, f"stitch_l_top_{k}", (-8.72, 15.55, z), (-8.36, 15.85, z + 0.3), "stitch")
        g.box(pl, f"stitch_l_bot_{k}", (-8.72, 13.38, z), (-8.36, 13.68, z + 0.3), "stitch")
    g.box(pl, "stitch_l_v1", (-8.72, 14.2, -1.15), (-8.36, 15.0, -0.85), "stitch")
    g.box(pl, "stitch_l_v2", (-8.72, 14.2, 0.85), (-8.36, 15.0, 1.15), "stitch")

    # 左腕后侧下挂的小皮条骨记号签 (厚度与宽度尺寸 ≥ 0.65px，供穿模门差分自证)
    ht = "hang_bone_tally"
    g.box(ht, "tally_loop", (-6.8, 12.45, 2.18), (-6.2, 13.05, 2.82), "cord_dk")
    g.box(ht, "tally_tag_main", (-6.9, 10.8, 2.18), (-6.1, 12.42, 2.82), "bone_dk")
    g.box(ht, "tally_tag_hole", (-6.65, 12.1, 2.15), (-6.35, 12.35, 2.85), "void")
    g.box(ht, "tally_tag_notch1", (-6.95, 11.6, 2.25), (-6.72, 11.85, 2.75), "bone")
    g.box(ht, "tally_tag_notch2", (-6.95, 11.2, 2.25), (-6.72, 11.45, 2.75), "bone")


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
    wear_note="挂点：双前臂（双前臂 y∈[12.08, 17.2], 右臂 x∈[4,8], 左臂 x∈[-8,-4]）",
    worn_focus=((0.0, 14.5, 0.0), 20.0),
    round_label="round 1/3",
    textured=False,
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
