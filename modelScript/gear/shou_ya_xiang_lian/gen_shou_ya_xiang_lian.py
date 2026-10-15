#!/usr/bin/env python3
"""兽牙项链（shou_ya_xiang_lian）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #15（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形（参考图 shou_ya_xiang_lian_ref.png / shou_ya_xiang_lian_exploded.png）：
皮绳串的兽牙、骨珠、碎陶片，中间一块大的异兽獠牙。
- 项链整体呈深 U 形悬挂在胸前：y∈[14.5, 24.6]，前索 z ≤ -2.10（完全在躯干前壁 -2.0 之外）。
- 过肩部分走肩峰外侧骑跨线路：|x| ≥ 4.10，y ≥ 24.10（严格避开头盒与手臂盒）。
- 后颈部分横跨背后：z ≥ 2.10，y∈[22.8, 23.85]，配有粗兽骨横栓扣与系结。
- 中部核心：巨大弯曲异兽獠牙（fang_center，长约 4.5px，包铁箍、刻槽风化，独立可动骨）。
- 左右对称排列副兽牙（fangs_inner，弯曲尖锐，独立骨）与外侧小兽齿（fangs_outer，独立骨）。
- 穿插串珠：粗磨六角骨珠、髓环骨节（bone_beads，独立骨）。
- 不对称残土点缀：两枚古陶碎片（clay_shards，钻孔挂系在不同高位）、左侧悬垂小异兽勾爪（hang_claw，独立骨）、
  右侧悬垂皮绳散丝与小骨粒流苏（hang_tassel，独立骨）。

骨骼数 ≥ 10，挂件独立骨，全绿门禁（含 clearance、coplanar、self-test）。
用法：
    python3 modelScript/gear/shou_ya_xiang_lian/gen_shou_ya_xiang_lian.py --self-test
    python3 modelScript/gear/shou_ya_xiang_lian/gen_shou_ya_xiang_lian.py --gates
    python3 modelScript/gear/shou_ya_xiang_lian/gen_shou_ya_xiang_lian.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "bone":       (214, 204, 176),    # 异兽骨主色
    "bone_lt":    (236, 226, 200),    # 獠牙尖高光 / 新鲜骨亮面
    "bone_dk":    (168, 154, 126),    # 骨裂纹 / 髓腔阴影
    "bone_stain": (130, 110, 82),     # 老化老骨斑 / 齿根黄色
    "clay":       (162, 94, 62),      # 碎陶片红赭色
    "clay_dk":    (116, 62, 40),      # 陶片粗糙断茬
    "strap":      (112, 72, 50),      # 鞣制牛皮绳
    "strap_dk":   (78, 48, 34),       # 浸油深色皮绳
    "cord":       (144, 128, 78),     # 麻绳扎结
    "cord_dk":    (102, 88, 56),      # 脏草绳深色
    "iron":       (82, 78, 76),       # 獠牙加固粗生铁箍
    "iron_lt":    (116, 112, 110),    # 铁光泽 / 铆钉头
    "rust":       (152, 86, 52),      # 生锈铁屑
    "void":       (28, 24, 22),       # 穿孔暗影
}

# 软件材质与骨陶硬物允许贴合咬合
SOFT = ("strap", "strap_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("bone", "bone_dk", "bone_stain", "clay", "clay_dk", "iron"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("cord_main", "root", (0.0, 21.0, -2.2), "主皮索圈：后颈横跨索、肩峰骑越索、胸前悬挂主索弧"),
    ("fang_center", "cord_main", (0.0, 16.5, -2.4), "中央大獠牙：粗大弯曲獠牙、齿根铁箍、风化纵槽（核心独立骨）"),
    ("fangs_inner", "cord_main", (0.0, 18.0, -2.35), "内侧副兽牙对：左右对称次级獠牙、根部扎绳"),
    ("fangs_outer", "cord_main", (0.0, 19.8, -2.3), "外侧小兽齿对：左右对称小排尖齿、齿槽骨粒"),
    ("bone_beads", "cord_main", (0.0, 20.5, -2.3), "骨珠骨节组：串联在各牙齿之间的六角骨珠与粗骨管"),
    ("clay_shards", "cord_main", (1.6, 20.0, -2.35), "穿孔碎陶片：带红赭胎质与彩陶断茬的两枚吊挂残陶片"),
    ("cord_ties", "cord_main", (0.0, 19.0, -2.35), "固定扎结件：铜丝缠箍、齿根麻绳打结锁死短绳"),
    ("hang_claw", "cord_main", (-2.6, 17.5, -2.35), "左侧挂勾爪：细皮绳悬挂的尖锐异兽弯爪（独立可动骨）"),
    ("hang_tassel", "cord_main", (2.8, 17.5, -2.35), "右侧皮须流苏：细散皮绳须垂挂的小骨珠与绳结（独立可动骨）"),
    ("back_toggle", "cord_main", (0.0, 23.4, 2.25), "后颈固定骨栓：后颈皮绳束紧横骨栓与悬垂绳头"),
]

# 不对称骨骼：主獠牙刻槽、陶片、扎绳须条、勾爪、流苏、后颈骨栓打结
ASYM_BONES = frozenset({"fang_center", "clay_shards", "cord_ties", "hang_claw", "hang_tassel", "back_toggle"})


# ───────────────────────────────────────────────────────────── 几何部件构建
def part_cord_main(g: Gear) -> None:
    """主皮索圈：后颈横跨索、肩峰骑越索、胸前悬挂主索弧。"""
    b = "cord_main"
    # 后颈段（z ≥ 2.10，y∈[23.15, 23.65]）
    g.pair(b, "cord_back_run", (0.45, 23.20, 2.10), (3.65, 23.55, 2.35), "strap")
    # 后肩转接段（y ≤ 23.85，z 严格 ≥ 2.10，避开头部与手臂）
    g.pair(b, "cord_back_jog", (3.55, 23.15, 2.12), (4.12, 23.85, 2.38), "strap_dk")
    # 后立升段（x ≥ 4.10，z 严格 ≥ 2.10 避开手臂）
    g.pair(b, "cord_back_riser", (4.10, 23.80, 2.10), (4.48, 24.45, 2.42), "strap")
    # 肩峰骑越段（|x| ≥ 4.12，y ≥ 24.10 避开手臂顶部）
    g.pair(b, "cord_over_shoulder", (4.12, 24.10, -2.15), (4.52, 24.48, 2.15), "strap_dk")
    # 前下沉段（x ≥ 4.10，z 严格 ≤ -2.10 避开手臂）
    g.pair(b, "cord_front_riser", (4.10, 23.80, -2.45), (4.48, 24.45, -2.10), "strap")
    # 前肩斜下转接段（y ≤ 23.85，z 严格 ≤ -2.10 避开头部与手臂）
    g.pair(b, "cord_front_jog", (3.55, 23.15, -2.40), (4.12, 23.85, -2.12), "strap_dk")

    # 胸前下垂深 U 形皮绳弧：
    # 第一段：锁骨段（x∈[2.85, 3.85]，y∈[22.25, 23.55]，z∈[-2.36, -2.08]）
    g.pair(b, "cord_f_drop1", (2.85, 22.25, -2.36), (3.85, 23.55, -2.08), "strap")
    # 第二段：中胸段（x∈[1.65, 2.95]，y∈[20.15, 22.40]，z∈[-2.40, -2.12]）
    g.pair(b, "cord_f_drop2", (1.65, 20.15, -2.40), (2.95, 22.40, -2.12), "strap_dk")
    # 第三段：心口段（x∈[0.65, 1.85]，y∈[18.15, 20.35]，z∈[-2.44, -2.16]）
    g.pair(b, "cord_f_drop3", (0.65, 18.15, -2.44), (1.85, 20.35, -2.16), "strap")
    # 第四段：中央交汇横跨底弧（x∈[-0.75, 0.75]，y∈[17.85, 18.35]，z∈[-2.48, -2.20]）
    g.box(b, "cord_f_center_loop", (-0.75, 17.85, -2.48), (0.75, 18.35, -2.20), "strap_dk")


def part_fang_center(g: Gear) -> None:
    """中央大獠牙：粗大弯曲獠牙、齿根铁箍、风化纵槽（核心独立骨）。"""
    b = "fang_center"
    # 中央主牙挂在心口底弧 (x=0.0, y∈[13.5, 18.2], z∈[-2.75, -2.30])
    # 齿冠包生铁加固箍
    g.box(b, "fang_c_band_t", (-0.56, 17.55, -2.90), (0.56, 18.25, -2.26), "iron")
    g.box(b, "fang_c_band_rivet", (-0.18, 17.70, -3.02), (0.18, 18.00, -2.75), "iron_lt")
    g.box(b, "fang_c_rust", (-0.45, 17.40, -2.85), (0.45, 17.65, -2.32), "rust")

    # 齿根基座（宽厚微黄老骨色，厚度饱满）
    g.box(b, "fang_c_root", (-0.52, 16.65, -2.88), (0.52, 17.55, -2.28), "bone_stain")
    g.box(b, "fang_c_crest_t", (-0.48, 17.25, -2.95), (0.48, 17.75, -2.22), "bone_lt")
    # 牙身第一节（主躯干，微向前隆起弧度）
    g.box(b, "fang_c_body1", (-0.45, 15.65, -2.62), (0.45, 16.75, -2.30), "bone")
    # 牙身第二节（微向右侧微弧弯曲，渐缩）
    g.box(b, "fang_c_body2", (-0.38, 14.65, -2.60), (0.38, 15.75, -2.32), "bone")
    # 牙身第三节（尖峰收束）
    g.box(b, "fang_c_tip1", (-0.30, 13.85, -2.65), (0.30, 14.75, -2.36), "bone_lt")
    # 极锐獠牙尖端
    g.box(b, "fang_c_tip2", (-0.18, 13.15, -2.62), (0.18, 13.95, -2.32), "bone_lt")

    # 獠牙正中磨损凹槽与陈年暗裂（厚度 ≥ 0.26）
    g.box(b, "fang_c_groove", (-0.12, 14.85, -2.76), (0.12, 16.85, -2.48), "bone_dk")
    g.box(b, "fang_c_crack", (-0.35, 15.85, -2.58), (0.15, 16.15, -2.26), "bone_stain")
    # 獠牙边缘粗糙侧齿与刻槽（厚度 ≥ 0.26）
    g.box(b, "fang_c_flange_l", (-0.65, 15.25, -2.58), (-0.38, 16.25, -2.32), "bone_stain")
    g.box(b, "fang_c_flange_r", (0.38, 15.25, -2.58), (0.65, 16.25, -2.32), "bone_lt")
    g.box(b, "fang_c_flange_l2", (-0.55, 14.35, -2.56), (-0.32, 15.05, -2.34), "bone_stain")
    g.box(b, "fang_c_flange_r2", (0.32, 14.35, -2.56), (0.55, 15.05, -2.34), "bone")
    # 齿根下部横向加固草绳圈
    g.box(b, "fang_c_cord_loop", (-0.48, 16.35, -2.72), (0.48, 16.70, -2.25), "cord_dk")


def part_fangs_inner(g: Gear) -> None:
    """内侧副兽牙对：左右对称次级獠牙、根部扎绳。"""
    b = "fangs_inner"
    # 悬挂于 x ≈ ±1.25，y∈[15.2, 18.8]，长约 3.2px
    # 牙根部小铁环与皮套
    g.pair(b, "fang_in_band", (0.95, 18.18, -2.55), (1.55, 18.68, -2.25), "iron")
    g.pair(b, "fang_in_knot", (1.02, 18.45, -2.65), (1.48, 18.85, -2.35), "cord_dk")
    g.pair(b, "fang_in_cap", (0.92, 17.85, -2.58), (1.58, 18.22, -2.22), "bone_stain")
    # 齿根
    g.pair(b, "fang_in_root", (0.98, 17.45, -2.52), (1.52, 18.25, -2.28), "bone_stain")
    # 牙身（微向外展微弯）
    g.pair(b, "fang_in_body1", (1.05, 16.45, -2.56), (1.55, 17.55, -2.30), "bone")
    g.pair(b, "fang_in_body2", (1.12, 15.45, -2.52), (1.52, 16.55, -2.32), "bone_lt")
    # 牙尖
    g.pair(b, "fang_in_tip", (1.20, 14.65, -2.58), (1.48, 15.70, -2.30), "bone_lt")
    # 牙侧纵槽（厚度 ≥ 0.25）
    g.pair(b, "fang_in_slit", (1.20, 16.15, -2.68), (1.45, 17.35, -2.40), "bone_dk")


def part_fangs_outer(g: Gear) -> None:
    """外侧小兽齿对：左右对称小排尖齿、齿槽骨粒。"""
    b = "fangs_outer"
    # 第一对外排牙齿
    g.pair(b, "fang_out1_band", (2.05, 19.35, -2.55), (2.65, 19.85, -2.25), "cord")
    g.pair(b, "fang_out1_root", (2.08, 18.65, -2.50), (2.62, 19.45, -2.22), "bone_stain")
    g.pair(b, "fang_out1_body", (2.12, 17.65, -2.52), (2.58, 18.75, -2.24), "bone")
    g.pair(b, "fang_out1_tip", (2.16, 16.95, -2.54), (2.54, 17.75, -2.26), "bone_lt")

    # 第二对外排侧向短尖齿
    g.pair(b, "fang_out2_band", (3.05, 21.25, -2.52), (3.65, 21.75, -2.22), "cord_dk")
    g.pair(b, "fang_out2_root", (3.08, 20.55, -2.48), (3.62, 21.35, -2.20), "bone_stain")
    g.pair(b, "fang_out2_body", (3.12, 19.75, -2.50), (3.58, 20.65, -2.22), "bone")
    g.pair(b, "fang_out2_tip", (3.16, 19.05, -2.52), (3.54, 19.85, -2.24), "bone_lt")


def part_bone_beads(g: Gear) -> None:
    """骨珠骨节组：串联在各牙齿之间的六角骨珠与粗骨管。"""
    b = "bone_beads"
    # 中央两牙之间的高位管状骨节（x ≈ ±0.60，y∈[18.6, 19.7]）
    g.pair(b, "bead_tube_inner", (0.35, 18.55, -2.52), (0.85, 19.65, -2.20), "bone")
    g.pair(b, "bead_ring_inner", (0.30, 18.95, -2.58), (0.90, 19.35, -2.15), "bone_dk")
    # 内外段隔开的扁垫骨片
    g.pair(b, "bead_spacer_inner", (0.32, 18.25, -2.50), (0.88, 18.55, -2.22), "bone_lt")

    # 第一对与第二对副牙之间的六角扁骨珠（x ≈ ±1.80，y∈[19.8, 20.8]）
    g.pair(b, "bead_hex_mid", (1.52, 19.85, -2.50), (2.08, 20.75, -2.20), "bone_lt")
    g.pair(b, "bead_rim_mid", (1.48, 20.18, -2.55), (2.12, 20.48, -2.15), "bone_stain")
    g.pair(b, "bead_spacer_mid", (1.55, 20.75, -2.48), (2.05, 21.05, -2.20), "bone_dk")

    # 上肩锁骨处的大型磨圆古骨圆环（x ≈ ±2.85，y∈[21.6, 22.7]，体量饱满）
    g.pair(b, "bead_ring_upper", (2.55, 21.65, -2.80), (3.18, 22.65, -2.18), "bone")
    g.pair(b, "bead_knot_upper", (2.50, 22.05, -2.55), (3.20, 22.35, -2.09), "cord_dk")
    g.pair(b, "bead_cap_upper", (2.52, 22.65, -2.52), (3.18, 22.95, -2.15), "bone_stain")


def part_clay_shards(g: Gear) -> None:
    """穿孔碎陶片：带红赭胎质与彩陶断茬的两枚吊挂残陶片（不对称）。"""
    b = "clay_shards"
    # 第一枚陶片：挂在左侧副牙外旁（x ≈ -1.85，y∈[17.2, 19.2]，z∈[-2.65, -2.32]）
    # 悬挂细麻绳圈
    g.box(b, "shard1_cord", (-2.02, 18.85, -2.55), (-1.68, 19.65, -2.28), "cord_dk")
    # 陶片主体（梯形破片，红赭色陶胎）
    g.box(b, "shard1_body", (-2.25, 17.62, -2.62), (-1.45, 18.95, -2.32), "clay")
    # 彩陶黑纹绘带残存
    g.box(b, "shard1_stripe", (-2.18, 18.22, -2.68), (-1.52, 18.52, -2.35), "clay_dk")
    # 锯齿断口残茬
    g.box(b, "shard1_fracture", (-2.05, 17.15, -2.58), (-1.65, 17.75, -2.36), "clay_dk")
    # 陶片穿孔
    g.box(b, "shard1_hole", (-1.98, 18.62, -2.72), (-1.72, 18.88, -2.25), "void")
    g.box(b, "shard1_flange", (-2.32, 17.95, -2.66), (-2.05, 18.55, -2.38), "clay")

    # 第二枚陶片：挂在右侧高位（x ≈ 2.55，y∈[18.4, 20.3]，z∈[-2.62, -2.30]）
    g.box(b, "shard2_cord", (2.38, 19.85, -2.52), (2.72, 20.55, -2.25), "cord")
    g.box(b, "shard2_body", (2.22, 18.75, -2.58), (2.88, 19.95, -2.28), "clay")
    g.box(b, "shard2_fracture", (2.32, 18.15, -2.55), (2.78, 18.85, -2.32), "clay_dk")
    g.box(b, "shard2_hole", (2.35, 19.55, -2.66), (2.61, 19.81, -2.22), "void")
    g.box(b, "shard2_stripe", (2.28, 18.95, -2.62), (2.82, 19.25, -2.25), "clay_dk")


def part_cord_ties(g: Gear) -> None:
    """固定扎结件：铜丝缠箍、齿根麻绳打结锁死短绳。"""
    b = "cord_ties"
    # 主獠牙左右两侧各一道捆扎绳圈
    g.pair(b, "tie_center_flank", (0.45, 17.75, -2.56), (0.85, 18.38, -2.24), "cord_dk")
    # 麻绳垂落的小须头（左右各一小撮，刻意错开高低）
    g.box(b, "tie_fray_l", (-0.78, 17.05, -2.54), (-0.52, 17.85, -2.25), "cord")
    g.box(b, "tie_fray_r", (0.55, 17.25, -2.54), (0.78, 17.95, -2.28), "cord_dk")


def part_hang_claw(g: Gear) -> None:
    """左侧挂勾爪：细皮绳悬挂的尖锐异兽弯爪（挂件独立可动骨）。"""
    b = "hang_claw"
    # 悬挂于左侧胸前锁骨下方（x ≈ -2.75，y∈[15.0, 18.5]）
    # 挂绳与皮结
    g.box(b, "claw_cord", (-2.88, 17.65, -2.52), (-2.62, 18.65, -2.25), "strap_dk")
    g.box(b, "claw_knot", (-2.92, 17.35, -2.58), (-2.58, 17.75, -2.20), "cord_dk")
    # 勾爪基座骨柄（粗糙暗骨）
    g.box(b, "claw_base", (-2.95, 16.55, -2.56), (-2.55, 17.45, -2.22), "bone_stain")
    # 弯爪主弧节（向前向下大幅度弯钩）
    g.box(b, "claw_arch", (-2.92, 15.65, -2.65), (-2.58, 16.65, -2.28), "bone")
    # 弯爪锐利爪尖
    g.box(b, "claw_tip", (-2.88, 14.85, -2.75), (-2.62, 15.75, -2.35), "bone_lt")
    # 爪根包扎粗麻绳
    g.box(b, "claw_lashing", (-2.98, 16.92, -2.62), (-2.52, 17.30, -2.18), "cord")


def part_hang_tassel(g: Gear) -> None:
    """右侧皮须流苏：细散皮绳须垂挂的小骨珠与绳结（挂件独立可动骨）。"""
    b = "hang_tassel"
    # 悬挂于右侧胸前（x ≈ 2.95，y∈[14.8, 18.5]）
    # 主皮绳索
    g.box(b, "tassel_strap", (2.82, 17.45, -2.50), (3.08, 18.45, -2.24), "strap")
    g.box(b, "tassel_knot", (2.75, 17.15, -2.56), (3.15, 17.55, -2.18), "cord")
    # 串在流苏上的小骨珠
    g.box(b, "tassel_bead1", (2.78, 16.45, -2.55), (3.12, 17.20, -2.20), "bone_lt")
    # 分叉皮须 1
    g.box(b, "tassel_strand1", (2.72, 15.35, -2.52), (2.96, 16.55, -2.26), "strap_dk")
    # 分叉皮须 2
    g.box(b, "tassel_strand2", (2.95, 15.05, -2.48), (3.18, 16.50, -2.24), "strap")
    # 末端串联极小兽齿
    g.box(b, "tassel_tooth", (2.68, 14.45, -2.55), (2.98, 15.40, -2.25), "bone")
    g.box(b, "tassel_tooth_tip", (2.72, 13.85, -2.52), (2.94, 14.50, -2.28), "bone_lt")


def part_back_toggle(g: Gear) -> None:
    """后颈固定骨栓：后颈皮绳束紧横骨栓与悬垂绳头。"""
    b = "back_toggle"
    # 后脑下方后颈中线处（z ≥ 2.10，y∈[22.8, 24.2]）
    # 横穿在左右后绳端部的带孔兽骨栓
    g.box(b, "toggle_bone_pin", (-0.75, 23.25, 2.25), (0.75, 23.65, 2.58), "bone_stain")
    g.box(b, "toggle_bone_crest", (-0.35, 23.65, 2.28), (0.35, 23.95, 2.55), "bone_lt")
    # 缠绕在骨栓上的皮绳套结
    g.box(b, "toggle_loop", (-0.52, 23.10, 2.20), (0.52, 23.75, 2.65), "strap_dk")
    # 向后右侧垂下的一截备用收紧皮绳尾
    g.box(b, "toggle_tail_root", (0.48, 22.85, 2.28), (0.95, 23.35, 2.56), "strap")
    g.box(b, "toggle_tail_drop", (0.65, 21.85, 2.26), (0.98, 22.95, 2.54), "strap_dk")
    g.box(b, "toggle_tail_knot", (0.58, 21.45, 2.24), (1.02, 21.90, 2.56), "cord_dk")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "cord_main": part_cord_main,
    "fang_center": part_fang_center,
    "fangs_inner": part_fangs_inner,
    "fangs_outer": part_fangs_outer,
    "bone_beads": part_bone_beads,
    "clay_shards": part_clay_shards,
    "cord_ties": part_cord_ties,
    "hang_claw": part_hang_claw,
    "hang_tassel": part_hang_tassel,
    "back_toggle": part_back_toggle,
}

SPEC = GearSpec(
    gear_id="shou_ya_xiang_lian",
    model_name="ShouYaXiangLian",
    title="兽牙项链 / shou_ya_xiang_lian",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：颈/胸 · 巨型异兽主獠牙+群齿骨珠+碎陶片+后颈骨栓",
    worn_focus=((0.0, 18.5, -2.0), 18.0),
    tex_views=(
        ("FRONT 正面（-z）", 180.0, 0.0),
        ("3/4 正右斜", 152.0, 18.0),
        ("RIGHT 右侧（+x）", -90.0, 0.0),
    ),
    texture_overrides={
        "strap": "gear_hide",
        "strap_dk": "gear_hide_dk",
    },
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
