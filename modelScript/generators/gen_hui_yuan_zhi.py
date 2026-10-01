"""回元枝：温润枝液包裹的浅色枝条与回元露珠。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "warm_soil": (82, 73, 61),
    "branch_bark": (102, 69, 49),
    "branch_high": (153, 111, 69),
    "leaf_sage": (116, 158, 92),
    "leaf_fresh": (175, 205, 112),
    "sap_gold": (244, 199, 90),
    "sap_glint": (255, 235, 164),
}


def part_warm_soil(rig):
    rig.bone("warm_soil", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("soil_floor", 0.0, 0.0, 7.0, 0.8, 5.6, 0.1, "warm_soil"),
        ("soil_left", -2.2, 0.1, 2.1, 1.3, 3.1, 0.35, "warm_soil"),
        ("soil_back", 0.2, -1.8, 4.7, 1.1, 1.2, 0.4, "branch_bark"),
        ("soil_right", 2.25, -0.2, 1.6, 1.05, 2.55, 0.35, "warm_soil"),
    ):
        pad(rig, "warm_soil", name, (x, y, z), (w, h, d), mat)


def part_upright_branches(rig):
    rig.bone("upright_branches", (0.0, 0.0, 0.0))
    branches = (
        ("trunk", (0.0, 0.8, 0.0), (0.1, 5.5, 0.0), 0.5),
        ("branch_left", (-0.1, 2.3, 0.05), (-2.15, 4.35, 0.25), 0.3),
        ("branch_left_tip", (-2.15, 4.35, 0.25), (-2.9, 5.8, 0.45), 0.22),
        ("branch_right", (0.2, 2.9, -0.1), (2.05, 4.55, -0.35), 0.3),
        ("branch_right_tip", (2.05, 4.55, -0.35), (2.8, 5.75, -0.55), 0.22),
        ("branch_high", (0.05, 4.25, 0.0), (-0.8, 6.25, 0.45), 0.25),
    )
    for name, start, end, radius in branches:
        strand(rig, "upright_branches", name, start, end, radius, "branch_bark")


def part_sage_leaves(rig):
    rig.bone("sage_leaves", (0.0, 0.0, 0.0))
    leaves = (
        ("left_low", (-2.15, 4.35, 0.25), (-3.05, 4.95, 0.55), "leaf_sage"),
        ("left_high", (-0.8, 6.25, 0.45), (-1.75, 6.85, 0.8), "leaf_fresh"),
        ("right_low", (2.05, 4.55, -0.35), (3.05, 5.0, -0.65), "leaf_fresh"),
        ("right_high", (0.1, 5.5, 0.0), (1.05, 6.5, 0.5), "leaf_sage"),
        ("front", (0.1, 3.55, 0.2), (0.4, 4.4, 1.7), "leaf_fresh"),
    )
    for name, start, end, mat in leaves:
        strand(rig, "sage_leaves", f"leaf_stem_{name}", start, end, 0.17, "leaf_sage")
        pad(
            rig,
            "sage_leaves",
            f"leaf_blade_{name}",
            (end[0], end[1] + 0.35, end[2]),
            (0.95, 0.3, 0.55),
            mat,
        )


def part_sap_drops(rig):
    rig.bone("sap_drops", (0.0, 0.0, 0.0))
    for index, (x, y, z) in enumerate(
        (
            (0.1, 4.0, 0.45),
            (-1.7, 4.85, 0.55),
            (1.75, 4.95, -0.55),
            (-0.95, 6.0, 0.55),
            (0.85, 5.8, 0.25),
        )
    ):
        pad(rig, "sap_drops", f"sap_drop_{index}", (x, y, z), (0.34, 0.52, 0.34), "sap_gold")
        pad(
            rig,
            "sap_drops",
            f"sap_glint_{index}",
            (x - 0.08, y + 0.12, z + 0.08),
            (0.24, 0.28, 0.24),
            "sap_glint",
        )


def build():
    return build_rig(MATS, (part_warm_soil, part_upright_branches, part_sage_leaves, part_sap_drops))


GATES = PlantGates("回元枝 / hui_yuan_zhi")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("HuiYuanZhi", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
