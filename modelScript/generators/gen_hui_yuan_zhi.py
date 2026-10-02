"""回元枝：温润枝液包裹的浅色枝条与回元露珠。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "warm_soil": (82, 73, 61),
    "stone_seat": (58, 58, 62),   # 深灰石墩 #3a3a3e
    "stone_edge": (92, 92, 96),   # 石墩棱边
    "qi_cyan": (74, 216, 208),    # 石墩正面青光 #4ad8d0
    "branch_bark": (102, 69, 49),
    "branch_high": (153, 111, 69),
    "leaf_sage": (116, 158, 92),
    "leaf_fresh": (175, 205, 112),
    "lantern_fruit": (218, 210, 154),
    "lantern_glint": (255, 244, 198),
}


def part_warm_soil(rig):
    """底层温润薄土垫台。"""
    rig.bone("warm_soil", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("soil_floor", 0.0, 0.0, 7.6, 0.6, 7.2, 0.1, "warm_soil"),
        ("soil_left", -2.6, 0.1, 2.2, 0.5, 3.2, 0.2, "warm_soil"),
        ("soil_right", 2.6, -0.2, 2.0, 0.5, 3.0, 0.2, "warm_soil"),
    ):
        pad(rig, "warm_soil", name, (x, y, z), (w, h, d), mat)


def part_stone_seat(rig):
    """明显的深灰方形石墩（高约 4px，比树干宽一圈），石墩正面嵌显眼的青光 #4ad8d0。"""
    rig.bone("stone_seat", (0.0, 0.0, 0.0))
    # 方形石墩主台身 (高 4.0px: y=0.5..4.5, 宽深 6.2x6.2px)
    pad(rig, "stone_seat", "seat_main", (0.0, 0.5, 0.0), (6.2, 4.0, 6.2), "stone_seat")
    # 顶部与侧角边缘微凸石棱
    pad(rig, "stone_seat", "seat_top_rim", (0.0, 4.25, 0.0), (6.5, 0.35, 6.5), "stone_edge")
    pad(rig, "stone_seat", "seat_corner_l", (-3.05, 0.55, 0.0), (0.45, 3.8, 3.6), "stone_edge")
    pad(rig, "stone_seat", "seat_corner_r", (3.05, 0.55, 0.0), (0.45, 3.8, 3.6), "stone_edge")
    # 石墩正面正中嵌入的显眼青光 #4ad8d0
    pad(rig, "stone_seat", "qi_front", (0.0, 2.3, 3.16), (1.6, 1.6, 0.24), "qi_cyan")


def part_upright_branches(rig):
    """枝干从方形石墩顶面中心长出，自然伸展分叉。"""
    rig.bone("upright_branches", (0.0, 0.0, 0.0))
    branches = (
        ("trunk", (0.0, 4.4, 0.0), (0.1, 8.8, 0.0), 0.55),
        ("branch_left", (-0.1, 5.8, 0.05), (-2.2, 7.8, 0.25), 0.32),
        ("branch_left_tip", (-2.2, 7.8, 0.25), (-2.95, 9.2, 0.45), 0.22),
        ("branch_right", (0.2, 6.2, -0.1), (2.1, 7.9, -0.35), 0.32),
        ("branch_right_tip", (2.1, 7.9, -0.35), (2.85, 9.2, -0.55), 0.22),
        ("branch_high", (0.05, 7.6, 0.0), (-0.8, 9.8, 0.45), 0.25),
    )
    for name, start, end, radius in branches:
        strand(rig, "upright_branches", name, start, end, radius, "branch_bark")


def part_sage_leaves(rig):
    rig.bone("sage_leaves", (0.0, 0.0, 0.0))
    leaves = (
        ("left_low", (-2.2, 7.8, 0.25), (-3.1, 8.4, 0.55), "leaf_sage"),
        ("left_high", (-0.8, 9.8, 0.45), (-1.75, 10.4, 0.8), "leaf_fresh"),
        ("right_low", (2.1, 7.9, -0.35), (3.1, 8.4, -0.65), "leaf_fresh"),
        ("right_high", (0.1, 8.8, 0.0), (1.1, 9.8, 0.5), "leaf_sage"),
        ("front", (0.1, 6.9, 0.2), (0.4, 7.7, 1.7), "leaf_fresh"),
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


def part_lantern_fruits(rig):
    rig.bone("lantern_fruits", (0.0, 0.0, 0.0))
    for index, (x, y, z) in enumerate(
        (
            (0.1, 7.4, 0.45),
            (-1.7, 8.2, 0.55),
            (1.75, 8.3, -0.55),
            (-0.95, 9.4, 0.55),
            (0.85, 9.2, 0.25),
        )
    ):
        strand(
            rig,
            "lantern_fruits",
            f"lantern_stem_{index}",
            (x, y - 0.35, z),
            (x, y, z),
            0.14,
            "branch_high",
        )
        pad(
            rig,
            "lantern_fruits",
            f"lantern_body_{index}",
            (x, y + 0.22, z),
            (0.75, 0.85, 0.75),
            "lantern_fruit",
        )
        pad(
            rig,
            "lantern_fruits",
            f"lantern_glint_{index}",
            (x + 0.12, y + 0.35, z + 0.38),
            (0.35, 0.35, 0.22),
            "lantern_glint",
        )


def build():
    return build_rig(
        MATS,
        (part_warm_soil, part_stone_seat, part_upright_branches, part_sage_leaves, part_lantern_fruits),
    )


GATES = PlantGates("回元枝 / hui_yuan_zhi")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("HuiYuanZhi", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
