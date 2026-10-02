"""凝脉草：性平而凝，墨蓝与深青色泽的稳固对生厚叶，中肋汇聚收束凝结。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "earth_mound": (48, 42, 38),
    "deep_blue_stem": (22, 45, 62),
    "leaf_indigo": (32, 68, 88),
    "leaf_aquablue": (48, 102, 128),
    "vein_cyan": (75, 155, 185),
}


def part_root_node(rig):
    rig.bone("root_node", (0.0, 0.0, 0.0))
    # 稳固扎根的泥台基底
    pad(rig, "root_node", "root_base_mound", (0.0, 0.12, 0.0), (4.2, 0.85, 4.2), "earth_mound")
    pad(rig, "root_node", "root_collar", (0.0, 0.75, 0.0), (2.4, 0.65, 2.4), "deep_blue_stem")


def part_main_stem(rig):
    rig.bone("main_stem", (0.0, 0.0, 0.0))
    # 笔直坚实圆柱粗茎 (分节段上升)
    strand(rig, "main_stem", "stem_lower", (0.0, 1.2, 0.0), (0.0, 4.5, 0.0), 0.42, "deep_blue_stem")
    strand(rig, "main_stem", "stem_mid", (0.0, 4.5, 0.0), (0.05, 7.8, -0.05), 0.36, "deep_blue_stem")
    strand(rig, "main_stem", "stem_top", (0.05, 7.8, -0.05), (0.08, 9.6, -0.08), 0.28, "deep_blue_stem")


def part_paired_leaves(rig):
    rig.bone("paired_leaves", (0.0, 0.0, 0.0))
    # 3 轮厚实收拢的肉质青蓝叶片
    # 1. 下轮大叶 (y~3.2, 十字舒展)
    for name, cx, cz, w, d, rot in (
        ("leaf_low_e", 2.2, 0.0, 2.8, 1.8, (0.0, 0.0, -18.0)),
        ("leaf_low_w", -2.2, 0.0, 2.8, 1.8, (0.0, 0.0, 18.0)),
        ("leaf_low_s", 0.0, 2.1, 1.8, 2.7, (18.0, 0.0, 0.0)),
        ("leaf_low_n", 0.0, -2.1, 1.8, 2.7, (-18.0, 0.0, 0.0)),
    ):
        pad(rig, "paired_leaves", name, (cx, 3.2, cz), (w, 0.32, d), "leaf_indigo", rotation=rot)

    # 2. 中轮叶 (y~5.8, 45度交错, 上扬)
    for name, cx, cz, w, d, rot in (
        ("leaf_mid_ne", 1.6, -1.6, 2.2, 1.5, (-14.0, 45.0, -14.0)),
        ("leaf_mid_sw", -1.6, 1.6, 2.2, 1.5, (14.0, 45.0, 14.0)),
        ("leaf_mid_se", 1.6, 1.6, 2.2, 1.5, (14.0, -45.0, -14.0)),
        ("leaf_mid_nw", -1.6, -1.6, 2.2, 1.5, (-14.0, -45.0, 14.0)),
    ):
        pad(rig, "paired_leaves", name, (cx, 5.8, cz), (w, 0.30, d), "leaf_aquablue", rotation=rot)

    # 3. 顶轮幼叶 (y~8.2, 向上抱合收束)
    pad(rig, "paired_leaves", "leaf_top_e", (0.9, 8.2, 0.0), (1.4, 0.28, 1.1), "leaf_aquablue", rotation=(0.0, 0.0, -32.0))
    pad(rig, "paired_leaves", "leaf_top_w", (-0.8, 8.2, 0.0), (1.4, 0.28, 1.1), "leaf_aquablue", rotation=(0.0, 0.0, 32.0))


def part_condense_veins(rig):
    rig.bone("condense_veins", (0.0, 0.0, 0.0))
    # 叶片表面凝敛气机的发光青脉 (贯穿至顶心)
    veins = (
        ("vein_low_e", (0.4, 3.35, 0.0), (2.8, 2.8, 0.0), 0.16),
        ("vein_low_w", (-0.4, 3.35, 0.0), (-2.8, 2.8, 0.0), 0.16),
        ("vein_low_s", (0.0, 3.35, 0.4), (0.0, 2.8, 2.8), 0.16),
        ("vein_low_n", (0.0, 3.35, -0.4), (0.0, 2.8, -2.8), 0.16),
        # 顶端聚气气孔核心
        ("vein_crown_core", (0.08, 9.6, -0.08), (0.09, 10.2, -0.09), 0.22),
    )
    for name, start, end, r in veins:
        strand(rig, "condense_veins", name, start, end, r, "vein_cyan")


def build():
    return build_rig(MATS, (part_root_node, part_main_stem, part_paired_leaves, part_condense_veins))


GATES = PlantGates("凝脉草 / ning_mai_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("NingMaiCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
