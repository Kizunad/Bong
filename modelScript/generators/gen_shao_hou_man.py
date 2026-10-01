"""烧喉蔓：洞壁上的发光垂挂藤蔓。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "cave_wall": (38, 35, 34),
    "wall_high": (67, 57, 52),
    "throat_vine": (48, 27, 27),
    "ember_orange": (235, 82, 28),
    "glow_hot": (255, 146, 45),
    "burn_leaf": (105, 38, 35),
}


def part_cave_wall(rig):
    rig.bone("cave_wall", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("wall_main", 0.0, -1.55, 6.8, 6.3, 1.35, 0.35, "cave_wall"),
        ("wall_left", -3.0, -0.95, 1.1, 4.3, 1.0, 0.35, "wall_high"),
        ("wall_right", 3.0, -1.0, 1.1, 4.8, 1.0, 0.35, "cave_wall"),
        ("wall_floor", 0.0, 0.0, 7.2, 0.8, 5.6, 0.1, "cave_wall"),
    ):
        pad(rig, "cave_wall", name, (x, y, z), (w, h, d), mat)


def part_hanging_vines(rig):
    rig.bone("hanging_vines", (0.0, 0.0, 0.0))
    segments = (
        ("vine_left_0", (-2.2, 6.25, -0.25), (-2.0, 4.7, 0.2), 0.34),
        ("vine_left_1", (-2.0, 4.7, 0.2), (-2.55, 3.0, 0.15), 0.3),
        ("vine_left_2", (-2.55, 3.0, 0.15), (-2.1, 1.15, 0.35), 0.27),
        ("vine_mid_0", (-0.55, 6.45, 0.1), (-0.35, 4.85, 0.55), 0.38),
        ("vine_mid_1", (-0.35, 4.85, 0.55), (0.4, 3.3, 0.5), 0.32),
        ("vine_mid_2", (0.4, 3.3, 0.5), (0.0, 1.0, 0.75), 0.27),
        ("vine_right_0", (1.45, 6.1, -0.15), (1.35, 4.55, 0.35), 0.32),
        ("vine_right_1", (1.35, 4.55, 0.35), (2.05, 2.9, 0.25), 0.28),
        ("vine_right_2", (2.05, 2.9, 0.25), (1.65, 1.35, 0.55), 0.24),
    )
    for name, start, end, radius in segments:
        strand(rig, "hanging_vines", name, start, end, radius, "throat_vine")


def part_ember_segments(rig):
    rig.bone("ember_segments", (0.0, 0.0, 0.0))
    for name, center, size, mat in (
        ("ember_left", (-2.2, 4.2, 0.48), (0.35, 0.8, 0.3), "ember_orange"),
        ("ember_left_low", (-2.3, 2.25, 0.45), (0.3, 0.65, 0.28), "glow_hot"),
        ("ember_mid", (-0.1, 4.25, 0.85), (0.38, 0.85, 0.32), "glow_hot"),
        ("ember_mid_low", (0.18, 2.05, 1.05), (0.3, 0.68, 0.28), "ember_orange"),
        ("ember_right", (1.4, 4.1, 0.62), (0.32, 0.75, 0.3), "ember_orange"),
        ("ember_right_low", (1.9, 2.2, 0.52), (0.3, 0.62, 0.28), "glow_hot"),
    ):
        pad(rig, "ember_segments", name, center, size, mat)


def part_burn_leaves(rig):
    rig.bone("burn_leaves", (0.0, 0.0, 0.0))
    for name, center, size in (
        ("leaf_left", (-2.45, 3.45, 0.3), (0.85, 0.3, 0.45)),
        ("leaf_mid", (0.45, 3.05, 0.7), (0.8, 0.28, 0.42)),
        ("leaf_right", (2.0, 3.0, 0.45), (0.8, 0.3, 0.45)),
    ):
        pad(rig, "burn_leaves", name, center, size, "burn_leaf")


def build():
    return build_rig(MATS, (part_cave_wall, part_hanging_vines, part_ember_segments, part_burn_leaves))


GATES = PlantGates("烧喉蔓 / shao_hou_man")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ShaoHouMan", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
