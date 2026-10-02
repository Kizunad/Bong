"""针芥子：烈辛草本，残峰湿地裂隙生，长出多条细长直立锐利如针的直立角果（siliques）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "crag_stone": (45, 48, 44),
    "sinewy_stem": (52, 65, 48),
    "mustard_leaf": (68, 88, 55),
    "pungent_pods": (155, 172, 62),
    "needle_spines": (210, 225, 115),
}


def part_stone_base(rig):
    rig.bone("stone_base", (0.0, 0.0, 0.0))
    # 坚硬残峰石隙基座与基生叶环
    pad(rig, "stone_base", "stone_crag", (0.0, 0.12, 0.0), (3.8, 0.75, 3.8), "crag_stone")
    pad(rig, "stone_base", "stone_side_l", (-1.3, 0.12, 0.3), (1.4, 0.55, 1.6), "crag_stone")
    pad(rig, "stone_base", "stone_side_r", (1.3, 0.12, -0.3), (1.4, 0.55, 1.6), "crag_stone")


def part_sinewy_stalk(rig):
    rig.bone("sinewy_stalk", (0.0, 0.0, 0.0))
    # 劲健直立的主茎与上部分生花轴 (y: 0.50 -> 7.80)
    strand(rig, "sinewy_stalk", "stalk_base", (0.0, 0.50, 0.0), (0.05, 3.60, 0.05), 0.36, "sinewy_stem")
    strand(rig, "sinewy_stalk", "stalk_mid", (0.05, 3.60, 0.05), (0.0, 6.80, -0.05), 0.28, "sinewy_stem")
    strand(rig, "sinewy_stalk", "stalk_apex", (0.0, 6.80, -0.05), (0.0, 8.40, 0.0), 0.22, "sinewy_stem")


def part_pinnate_leaves(rig):
    rig.bone("pinnate_leaves", (0.0, 0.0, 0.0))
    # 基部与下部琴状羽裂叶片
    leaves = (
        ("leaf_base_f", (0.0, 0.65, 0.5), (0.0, 1.60, 2.2), 0.24, "mustard_leaf"),
        ("leaf_base_b", (0.0, 0.65, -0.5), (0.0, 1.60, -2.2), 0.24, "mustard_leaf"),
        ("leaf_base_l", (-0.5, 0.65, 0.0), (-2.1, 1.50, 0.1), 0.24, "mustard_leaf"),
        ("leaf_base_r", (0.5, 0.65, 0.0), (2.1, 1.50, -0.1), 0.24, "mustard_leaf"),
        # 中茎部披针叶
        ("leaf_stem_fl", (-0.1, 3.20, 0.2), (-1.2, 4.40, 1.1), 0.20, "mustard_leaf"),
        ("leaf_stem_br", (0.1, 4.20, -0.2), (1.1, 5.20, -1.1), 0.18, "mustard_leaf"),
    )
    for name, start, end, r, mat in leaves:
        strand(rig, "pinnate_leaves", name, start, end, r, mat)


def part_needle_siliques(rig):
    rig.bone("needle_siliques", (0.0, 0.0, 0.0))
    # 针状直立开裂的细长角果丛 (针芥子标志特征, 直指天空)
    pods = (
        # 顶心主针角果
        ("pod_center_main", (0.0, 8.40, 0.0), (0.0, 11.60, 0.0), 0.22, "pungent_pods"),
        ("pod_center_spine", (0.0, 11.60, 0.0), (0.0, 12.80, 0.0), 0.16, "needle_spines"),
        # 前斜刺角果
        ("pod_f_main", (0.0, 7.20, 0.15), (0.2, 10.20, 0.9), 0.20, "pungent_pods"),
        ("pod_f_spine", (0.2, 10.20, 0.9), (0.3, 11.40, 1.25), 0.15, "needle_spines"),
        # 后斜刺角果
        ("pod_b_main", (0.0, 7.20, -0.15), (-0.2, 10.10, -0.85), 0.20, "pungent_pods"),
        ("pod_b_spine", (-0.2, 10.10, -0.85), (-0.3, 11.30, -1.2), 0.15, "needle_spines"),
        # 左斜刺角果
        ("pod_l_main", (-0.15, 6.40, 0.0), (-0.95, 9.60, 0.15), 0.20, "pungent_pods"),
        ("pod_l_spine", (-0.95, 9.60, 0.15), (-1.35, 10.80, 0.2), 0.15, "needle_spines"),
        # 右斜刺角果
        ("pod_r_main", (0.15, 6.50, 0.0), (0.95, 9.50, -0.15), 0.20, "pungent_pods"),
        ("pod_r_spine", (0.95, 9.50, -0.15), (1.35, 10.70, -0.2), 0.15, "needle_spines"),
    )
    for name, start, end, r, mat in pods:
        strand(rig, "needle_siliques", name, start, end, r, mat)


def build():
    return build_rig(MATS, (part_stone_base, part_sinewy_stalk, part_pinnate_leaves, part_needle_siliques))


GATES = PlantGates("针芥子 / zhen_jie_zi")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ZhenJieZi", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
