"""悬根薇：浮岛底面垂落的翠青藤薇与虹吸锐晶根尖。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import (  # noqa: E402
    PlantGates,
    build_rig,
    curved_vine_chain,
    pad,
    strand,
    write_model,
)

MATS = {
    "island_rock": (68, 78, 70),
    "rock_high": (104, 113, 94),
    "root_green": (42, 112, 62),
    "leaf_green": (88, 168, 78),
    "leaf_high": (142, 204, 98),
    "siphon_crystal": (106, 224, 106),
    "crystal_glint": (178, 246, 140),
}


def part_island_underside(rig):
    rig.bone("island_underside", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("island_floor", 0.0, 0.0, 7.2, 0.9, 5.8, 6.35, "island_rock"),
        ("island_left", -2.45, 0.1, 2.2, 1.45, 3.2, 6.4, "rock_high"),
        ("island_back", 0.1, -1.75, 4.8, 1.25, 1.25, 6.45, "island_rock"),
        ("island_right", 2.35, -0.2, 1.7, 1.15, 2.6, 6.4, "island_rock"),
    ):
        pad(rig, "island_underside", name, (x, y, z), (w, h, d), mat)


def _root_path(root_index: int, count: int = 12):
    phase = math.tau * root_index / 6.0
    points = []
    for step in range(count):
        t = step / (count - 1)
        angle = phase + math.tau * t
        radius_x = 2.75 + 0.18 * math.sin(phase)
        radius_y = 2.25 + 0.12 * math.cos(phase)
        points.append(
            (
                radius_x * math.cos(angle),
                3.45 + radius_y * math.sin(angle),
                -0.85 + 0.3 * root_index + 0.12 * math.sin(angle),
            )
        )
    return points


def part_hanging_roots(rig):
    rig.bone("hanging_roots", (0.0, 0.0, 0.0))
    for root_index in range(6):
        points = _root_path(root_index)
        curved_vine_chain(
            rig,
            "hanging_roots",
            f"root_vine_{root_index}",
            points,
            0.45,
            0.28,
            "root_green",
        )


def part_root_leaves(rig):
    rig.bone("root_leaves", (0.0, 0.0, 0.0))
    for index, (x, y, z, dx, dz) in enumerate(
        (
            (2.7, 3.5, -0.55, 1.0, 0.0),
            (-2.7, 3.35, -0.25, -1.0, 0.0),
            (0.0, 5.7, 0.1, 0.0, 1.0),
            (0.0, 1.2, 0.25, 0.0, -1.0),
        )
    ):
        strand(
            rig,
            "root_leaves",
            f"leaf_stem_{index}",
            (x, y, z),
            (x + dx * 0.75, y - 0.1, z + dz * 0.75),
            0.18,
            "leaf_green",
        )
        pad(
            rig,
            "root_leaves",
            f"leaf_blade_{index}",
            (x + dx * 0.9, y - 0.35, z + dz * 0.9),
            (0.75, 0.28, 0.48),
            "leaf_high",
        )


def part_siphon_crystals(rig):
    rig.bone("siphon_crystals", (0.0, 0.0, 0.0))
    for index, (x, y, z, dx, dz) in enumerate(
        (
            (-1.75, 1.45, -0.55, -0.1, 0.0),
            (-0.65, 1.12, -0.25, 0.0, 0.1),
            (0.7, 1.1, 0.05, 0.1, 0.0),
            (1.8, 1.45, 0.25, 0.0, -0.1),
        )
    ):
        strand(
            rig,
            "siphon_crystals",
            f"crystal_shard_{index}",
            (x, y, z),
            (x + dx, y - 0.58, z + dz),
            0.22,
            "siphon_crystal",
        )
        pad(
            rig,
            "siphon_crystals",
            f"crystal_tip_{index}",
            (x + dx, y - 0.86, z + dz),
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
