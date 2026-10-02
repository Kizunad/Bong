"""刺舌蒿：辛苦微毒，干绿羽裂叶带尖锐刺舌小齿，顶端缀聚苦黄微毒穗蕾。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "dry_ground": (52, 46, 38),
    "wormwood_stem": (48, 54, 40),
    "serrated_leaf": (68, 75, 52),
    "spiky_tongue": (110, 118, 65),
    "toxic_buds": (168, 142, 58),
}


def part_stem_base(rig):
    rig.bone("stem_base", (0.0, 0.0, 0.0))
    # 干燥沙土基台
    pad(rig, "stem_base", "stem_ground", (0.0, 0.12, 0.0), (3.6, 0.65, 3.6), "dry_ground")
    # 主茎直立稍带微曲 (分 3 段上升)
    strand(rig, "stem_base", "stem_seg_0", (0.0, 0.45, 0.0), (0.1, 4.0, 0.15), 0.35, "wormwood_stem")
    strand(rig, "stem_base", "stem_seg_1", (0.1, 4.0, 0.15), (-0.1, 7.8, -0.1), 0.30, "wormwood_stem")
    strand(rig, "stem_base", "stem_seg_2", (-0.1, 7.8, -0.1), (0.0, 10.6, 0.0), 0.24, "wormwood_stem")


def part_feather_branches(rig):
    rig.bone("feather_branches", (0.0, 0.0, 0.0))
    # 侧生羽状互生分枝
    branches = (
        ("branch_l1", (0.05, 3.2, 0.1), (-1.8, 5.2, 0.6), 0.22),
        ("branch_r1", (0.08, 4.5, 0.12), (1.9, 6.4, -0.5), 0.22),
        ("branch_l2", (-0.05, 6.2, 0.0), (-1.6, 8.2, -0.4), 0.20),
        ("branch_r2", (-0.08, 7.5, -0.08), (1.5, 9.4, 0.5), 0.18),
    )
    for name, start, end, r in branches:
        strand(rig, "feather_branches", name, start, end, r, "wormwood_stem")


def part_spiky_tongues(rig):
    rig.bone("spiky_tongues", (0.0, 0.0, 0.0))
    # 侧生尖利刺舌小齿叶片
    leaves = (
        # 左下分枝裂片
        ("tongue_l1_a", (-1.8, 5.2, 0.6), (-2.8, 5.9, 1.0), 0.22, "serrated_leaf"),
        ("tongue_l1_spike", (-2.8, 5.9, 1.0), (-3.6, 6.4, 1.3), 0.14, "spiky_tongue"),
        ("tongue_l1_side", (-1.2, 4.6, 0.4), (-2.1, 4.8, 1.2), 0.15, "spiky_tongue"),
        # 右下分枝裂片
        ("tongue_r1_a", (1.9, 6.4, -0.5), (2.9, 7.1, -0.9), 0.22, "serrated_leaf"),
        ("tongue_r1_spike", (2.9, 7.1, -0.9), (3.7, 7.5, -1.2), 0.14, "spiky_tongue"),
        ("tongue_r1_side", (1.3, 5.8, -0.3), (2.2, 6.1, -1.1), 0.15, "spiky_tongue"),
        # 左上分枝裂片
        ("tongue_l2_a", (-1.6, 8.2, -0.4), (-2.4, 9.1, -0.7), 0.20, "serrated_leaf"),
        ("tongue_l2_spike", (-2.4, 9.1, -0.7), (-3.1, 9.6, -0.9), 0.13, "spiky_tongue"),
        # 右上分枝裂片
        ("tongue_r2_a", (1.5, 9.4, 0.5), (2.3, 10.2, 0.8), 0.18, "serrated_leaf"),
        ("tongue_r2_spike", (2.3, 10.2, 0.8), (2.9, 10.6, 1.0), 0.12, "spiky_tongue"),
        # 靠近地面的基部刺舌叶
        ("tongue_base_f", (0.0, 1.8, 1.2), (0.0, 2.4, 2.2), 0.16, "spiky_tongue"),
        ("tongue_base_b", (0.0, 1.8, -1.2), (0.0, 2.4, -2.2), 0.16, "spiky_tongue"),
    )
    for name, start, end, r, mat in leaves:
        strand(rig, "spiky_tongues", name, start, end, r, mat)


def part_toxic_flower(rig):
    rig.bone("toxic_flower", (0.0, 0.0, 0.0))
    # 茎顶聚生的细碎苦黄小穗蕾
    pad(rig, "toxic_flower", "flower_top_core", (0.0, 10.6, 0.0), (1.1, 1.2, 1.1), "toxic_buds")
    pad(rig, "toxic_flower", "flower_top_l", (-0.45, 10.8, 0.15), (0.65, 0.8, 0.65), "toxic_buds")
    pad(rig, "toxic_flower", "flower_top_r", (0.45, 10.9, -0.1), (0.65, 0.75, 0.65), "toxic_buds")
    pad(rig, "toxic_flower", "flower_top_apex", (0.0, 11.6, 0.0), (0.6, 0.7, 0.6), "toxic_buds")


def build():
    return build_rig(MATS, (part_stem_base, part_feather_branches, part_spiky_tongues, part_toxic_flower))


GATES = PlantGates("刺舌蒿 / ci_she_hao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("CiSheHao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
