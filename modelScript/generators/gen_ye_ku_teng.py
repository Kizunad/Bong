"""夜枯藤：幽穴里吸取真元的干枯暗藤。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "cave_bed": (36, 34, 38),
    "dry_vine": (22, 20, 28),
    "vine_high": (55, 43, 67),
    "siphon_void": (78, 54, 108),
    "dead_leaf": (48, 44, 50),
}


def part_cave_bed(rig):
    rig.bone("cave_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bed_floor", 0.0, 0.0, 7.0, 0.8, 5.6, 0.1, "cave_bed"),
        ("bed_left", -2.35, 0.0, 2.0, 1.4, 3.2, 0.35, "cave_bed"),
        ("bed_right", 2.3, -0.25, 2.2, 1.15, 2.8, 0.35, "cave_bed"),
        ("bed_back", 0.0, -1.75, 4.8, 1.05, 1.2, 0.35, "vine_high"),
    ):
        pad(rig, "cave_bed", name, (x, y, z), (w, h, d), mat)


def part_dry_vines(rig):
    rig.bone("dry_vines", (0.0, 0.0, 0.0))
    segments = (
        ("vine_main_0", (-2.0, 0.85, 0.15), (-1.15, 2.45, 0.2), 0.4),
        ("vine_main_1", (-1.15, 2.45, 0.2), (0.2, 4.05, 0.0), 0.36),
        ("vine_main_2", (0.2, 4.05, 0.0), (1.6, 5.15, 0.35), 0.32),
        ("vine_drop_0", (-0.95, 2.2, 0.55), (-2.25, 3.65, 0.7), 0.3),
        ("vine_drop_1", (-2.25, 3.65, 0.7), (-2.75, 5.1, 0.5), 0.26),
        ("vine_drop_2", (-2.75, 5.1, 0.5), (-2.35, 6.45, 0.7), 0.25),
        ("vine_right_0", (0.7, 3.0, -0.45), (2.35, 3.9, -0.65), 0.3),
        ("vine_right_1", (2.35, 3.9, -0.65), (2.65, 5.55, -0.45), 0.26),
    )
    for name, start, end, radius in segments:
        strand(rig, "dry_vines", name, start, end, radius, "dry_vine")


def part_siphon_tendrils(rig):
    rig.bone("siphon_tendrils", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("siphon_left", (-2.45, 3.9, 0.85), (-3.0, 2.1, 0.95)),
        ("siphon_low", (-1.2, 2.55, 0.9), (0.0, 1.05, 1.05)),
        ("siphon_right", (2.25, 4.15, -0.5), (3.05, 2.45, -0.55)),
        ("siphon_high", (1.25, 5.0, 0.45), (2.45, 6.25, 0.55)),
    ):
        strand(rig, "siphon_tendrils", name, start, end, 0.26, "siphon_void")
    for name, x, y, z in (
        ("siphon_tip_left", -3.0, 1.95, 0.95),
        ("siphon_tip_low", 0.0, 0.9, 1.05),
        ("siphon_tip_right", 3.05, 2.3, -0.55),
        ("siphon_tip_high", 2.45, 6.15, 0.55),
    ):
        pad(rig, "siphon_tendrils", name, (x, y, z), (0.42, 0.38, 0.42), "siphon_void")


def part_dead_leaves(rig):
    rig.bone("dead_leaves", (0.0, 0.0, 0.0))
    for name, center, size in (
        ("leaf_main", (0.95, 4.55, 0.2), (0.95, 0.3, 0.55)),
        ("leaf_left", (-2.45, 5.95, 0.6), (0.8, 0.28, 0.5)),
        ("leaf_right", (2.45, 5.2, -0.4), (0.75, 0.28, 0.46)),
    ):
        pad(rig, "dead_leaves", name, center, size, "dead_leaf")


def build():
    return build_rig(MATS, (part_cave_bed, part_dry_vines, part_siphon_tendrils, part_dead_leaves))


GATES = PlantGates("夜枯藤 / ye_ku_teng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YeKuTeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
