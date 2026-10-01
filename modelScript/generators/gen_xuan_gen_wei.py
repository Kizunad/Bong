"""悬根薇：浮岛底面垂落的翠青藤薇与虹吸锐晶根尖。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "island_rock": (68, 78, 70),
    "rock_high": (104, 113, 94),
    "root_green": (42, 112, 62),
    "leaf_green": (88, 168, 78),
    "leaf_high": (142, 204, 98),
    "siphon_crystal": (92, 205, 214),
    "crystal_glint": (178, 246, 236),
}


def part_island_underside(rig):
    rig.bone("island_underside", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("island_floor", 0.0, 0.0, 7.2, 0.85, 5.8, 0.1, "island_rock"),
        ("island_left", -2.45, 0.1, 2.2, 1.5, 3.2, 0.4, "rock_high"),
        ("island_back", 0.1, -1.75, 4.8, 1.25, 1.25, 0.45, "island_rock"),
        ("island_right", 2.35, -0.2, 1.7, 1.15, 2.6, 0.35, "island_rock"),
    ):
        pad(rig, "island_underside", name, (x, y, z), (w, h, d), mat)


def part_hanging_roots(rig):
    rig.bone("hanging_roots", (0.0, 0.0, 0.0))
    roots = (
        ("root_center", (0.0, 1.0, 0.0), (0.15, 5.9, 0.1), (0.38, 0.3)),
        ("root_left", (-1.25, 0.9, 0.25), (-2.3, 5.45, 0.55), (0.32, 0.24)),
        ("root_right", (1.25, 0.85, -0.25), (2.35, 5.3, -0.5), (0.34, 0.24)),
        ("root_back", (0.4, 0.8, -1.0), (1.05, 4.9, -2.35), (0.3, 0.22)),
        ("root_front", (-0.45, 0.75, 0.8), (-1.05, 4.7, 2.25), (0.3, 0.22)),
    )
    for name, start, end, (base_radius, tip_radius) in roots:
        strand(rig, "hanging_roots", f"{name}_lower", start, end, base_radius, "root_green")
        tip = (end[0] * 1.06, end[1] + 0.55, end[2] * 1.06)
        strand(rig, "hanging_roots", f"{name}_tip", end, tip, tip_radius, "leaf_green")


def part_root_leaves(rig):
    rig.bone("root_leaves", (0.0, 0.0, 0.0))
    for index, (x, y, z, dx, dz) in enumerate(
        (
            (-1.15, 3.0, 0.45, -0.95, 0.2),
            (1.1, 3.15, -0.45, 0.9, -0.25),
            (-0.35, 4.0, 0.3, -0.7, 0.65),
            (0.75, 4.15, -0.25, 0.8, 0.5),
            (-1.85, 4.25, 0.65, -0.55, 0.65),
            (1.85, 4.35, -0.7, 0.55, -0.65),
        )
    ):
        strand(
            rig,
            "root_leaves",
            f"leaf_stem_{index}",
            (x, y, z),
            (x + dx * 0.9, y + 0.7, z + dz * 0.9),
            0.18,
            "leaf_green",
        )
        pad(
            rig,
            "root_leaves",
            f"leaf_blade_{index}",
            (x + dx, y + 1.0, z + dz),
            (0.75, 0.28, 0.48),
            "leaf_high",
        )


def part_siphon_crystals(rig):
    rig.bone("siphon_crystals", (0.0, 0.0, 0.0))
    for index, (x, y, z, dx, dz) in enumerate(
        (
            (0.2, 0.85, 0.1, 0.25, 0.15),
            (-2.3, 0.9, 0.55, -0.3, 0.15),
            (2.35, 0.85, -0.5, 0.3, -0.2),
            (1.05, 0.8, -2.35, 0.2, -0.3),
            (-1.05, 0.75, 2.25, -0.15, 0.3),
        )
    ):
        strand(
            rig,
            "siphon_crystals",
            f"crystal_shard_{index}",
            (x, y, z),
            (x + dx, y + 1.2, z + dz),
            0.22,
            "siphon_crystal",
        )
        pad(
            rig,
            "siphon_crystals",
            f"crystal_tip_{index}",
            (x + dx, y + 1.45, z + dz),
            (0.42, 0.7, 0.42),
            "crystal_glint",
        )


def build():
    return build_rig(
        MATS,
        (part_island_underside, part_hanging_roots, part_root_leaves, part_siphon_crystals),
    )


GATES = PlantGates("悬根薇 / xuan_gen_wei")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XuanGenWei", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
