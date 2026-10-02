"""云顶兰：浮岛顶面开放的银白兰草，飘逸多姿的银白兰花瓣（#cbdbe4 / #fcfcff）与向下兜翻的大唇瓣。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "floating_rock": (48, 52, 58),
    "orchid_leaf": (75, 92, 105),       # 苍灰青叶基
    "silver_petal": (205, 218, 228),    # 银白主花瓣
    "pure_white_lip": (252, 252, 255),  # 纯白唇瓣
    "blue_vein": (115, 175, 235),       # 冰蓝冷纹
}


def part_floating_base(rig):
    rig.bone("floating_base", (0.0, 0.0, 0.0))
    # 浮岛浅根微岩台
    pad(rig, "floating_base", "base_mound", (0.0, 0.12, 0.0), (4.5, 0.75, 4.5), "floating_rock")
    strand(rig, "floating_base", "base_flower_stalk", (0.0, 0.75, 0.0), (0.0, 4.20, 0.0), 0.42, "orchid_leaf")


def part_orchid_leaves(rig):
    rig.bone("orchid_leaves", (0.0, 0.0, 0.0))
    # 4 片基生狭长微弧兰草宽叶（两侧大弧度弓垂）
    leaves = (
        ("leaf_l_low", (-0.4, 0.8, 0.2), (-2.8, 2.8, 0.6), 0.44),
        ("leaf_l_arch", (-2.8, 2.8, 0.6), (-4.6, 2.1, 0.9), 0.34),
        ("leaf_r_low", (0.4, 0.8, -0.2), (2.8, 2.8, -0.6), 0.44),
        ("leaf_r_arch", (2.8, 2.8, -0.6), (4.6, 2.1, -0.9), 0.34),
        ("leaf_b_low", (0.0, 0.8, -0.5), (0.0, 2.9, -3.2), 0.42),
        ("leaf_b_arch", (0.0, 2.9, -3.2), (0.0, 2.2, -4.8), 0.32),
    )
    for name, start, end, r in leaves:
        strand(rig, "orchid_leaves", name, start, end, r, "orchid_leaf")


def part_silver_orchid_flower(rig):
    rig.bone("silver_orchid_flower", (0.0, 0.0, 0.0))
    # 典型兰花结构：上方背萼片（直挺如旗，y: 4.2 -> 9.4）、两侧舒展侧瓣（向左右侧平展）
    # 1. 直挺上位大主萼片 (dorsal sepal)
    strand(rig, "silver_orchid_flower", "petal_dorsal_base", (0.0, 4.2, 0.0), (0.0, 7.2, -0.2), 0.52, "silver_petal")
    strand(rig, "silver_orchid_flower", "petal_dorsal_apex", (0.0, 7.2, -0.2), (0.0, 9.6, -0.3), 0.38, "pure_white_lip")
    # 背部冰蓝细纹
    strand(rig, "silver_orchid_flower", "petal_dorsal_vein", (0.0, 4.4, -0.1), (0.0, 8.8, -0.25), 0.16, "blue_vein")

    # 2. 左右横向飞展的飘逸双侧瓣 (petals, 向侧上展开，宽达 10.2px)
    strand(rig, "silver_orchid_flower", "petal_wing_l_in", (-0.4, 4.8, 0.1), (-2.8, 6.2, 0.3), 0.48, "silver_petal")
    strand(rig, "silver_orchid_flower", "petal_wing_l_out", (-2.8, 6.2, 0.3), (-4.8, 7.2, 0.5), 0.36, "pure_white_lip")

    strand(rig, "silver_orchid_flower", "petal_wing_r_in", (0.4, 4.8, -0.1), (2.8, 6.2, -0.3), 0.48, "silver_petal")
    strand(rig, "silver_orchid_flower", "petal_wing_r_out", (2.8, 6.2, -0.3), (4.8, 7.2, -0.5), 0.36, "pure_white_lip")

    # 3. 左右下侧萼片 (lateral sepals)
    strand(rig, "silver_orchid_flower", "petal_sepal_l", (-0.4, 4.2, -0.1), (-3.2, 3.8, -0.4), 0.42, "silver_petal")
    strand(rig, "silver_orchid_flower", "petal_sepal_r", (0.4, 4.2, 0.1), (3.2, 3.8, 0.4), 0.42, "silver_petal")


def part_orchid_lip(rig):
    rig.bone("orchid_lip", (0.0, 0.0, 0.0))
    # 兰花核心标志：向前下方兜状大翻卷的纯白大唇瓣（labellum，带冰蓝蕊柱）
    pad(rig, "orchid_lip", "lip_column_core", (0.0, 4.6, 0.4), (1.4, 1.4, 1.4), "blue_vein")
    pad(rig, "orchid_lip", "lip_pouch_body", (0.0, 4.1, 1.6), (2.8, 1.6, 2.4), "pure_white_lip")
    pad(rig, "orchid_lip", "lip_pouch_rim_l", (-1.2, 4.2, 1.6), (1.1, 1.2, 1.8), "silver_petal")
    pad(rig, "orchid_lip", "lip_pouch_rim_r", (1.2, 4.2, 1.6), (1.1, 1.2, 1.8), "silver_petal")
    pad(rig, "orchid_lip", "lip_front_lobe", (0.0, 3.5, 2.8), (2.2, 0.7, 1.6), "pure_white_lip")


def build():
    return build_rig(MATS, (part_floating_base, part_orchid_leaves, part_silver_orchid_flower, part_orchid_lip))


GATES = PlantGates("云顶兰 / yun_ding_lan")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YunDingLan", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
