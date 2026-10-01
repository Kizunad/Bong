"""灰烬苔：残灰方块上的低矮灰黑苔层。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, leaf, pad, write_model  # noqa: E402

MATS = {
    "ash_bed": (62, 61, 54),
    "ash_edge": (103, 97, 78),
    "soot": (31, 32, 30),
    "lichen": (78, 87, 60),
    "dry_highlight": (138, 126, 84),
}


def part_ash_bed(rig):
    rig.bone("ash_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("ash_center", 0.0, 0.0, 5.8, 4.8),
        ("ash_front", 0.0, 2.25, 4.6, 1.55),
        ("ash_back", 0.0, -2.15, 4.9, 1.35),
        ("ash_left", -2.65, 0.15, 1.45, 3.1),
        ("ash_right", 2.6, -0.25, 1.35, 2.8),
    ):
        pad(rig, "ash_bed", name, (x, 0.15, z), (w, 0.42, d), "ash_bed")
    for name, x, z, w, d in (
        ("ash_edge_front", 0.0, 2.45, 4.0, 0.24),
        ("ash_edge_left", -2.85, 0.05, 0.24, 2.2),
        ("ash_edge_right", 2.8, -0.15, 0.24, 2.0),
    ):
        pad(rig, "ash_bed", name, (x, 0.58, z), (w, 0.24, d), "ash_edge")


def part_soot_crust(rig):
    rig.bone("soot_crust", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("soot_core", 0.0, 0.0, 3.8, 2.7),
        ("soot_l", -1.65, 0.65, 1.4, 1.3),
        ("soot_r", 1.6, -0.55, 1.25, 1.2),
    ):
        pad(rig, "soot_crust", name, (x, 0.52, z), (w, 0.38, d), "soot")


def part_lichen_rosettes(rig):
    rig.bone("lichen_rosettes", (0.0, 0.0, 0.0))
    for suffix, x, z in (("c", 0.0, 0.0), ("l", -2.2, 0.45), ("r", 2.15, -0.25)):
        for index, (dx, dz, tilt) in enumerate(
            ((-0.7, 0.0, (-8.0, 0.0, -10.0)), (0.0, 0.2, (0.0, 0.0, 0.0)),
             (0.7, -0.05, (8.0, 0.0, 10.0)))
        ):
            leaf(
                rig,
                "lichen_rosettes",
                f"lichen_{suffix}_{index}",
                (x + dx, 0.7, z + dz),
                0.72,
                1.35,
                0.32,
                "lichen",
                tilt=tilt,
            )


def part_dry_highlights(rig):
    rig.bone("dry_highlights", (0.0, 0.0, 0.0))
    for name, x, z in (("dry_l", -1.25, 1.1), ("dry_r", 1.35, -1.0), ("dry_front", 0.1, 2.0)):
        pad(rig, "dry_highlights", name, (x, 0.98, z), (0.38, 0.26, 0.7), "dry_highlight")


def build():
    return build_rig(MATS, (part_ash_bed, part_soot_crust, part_lichen_rosettes, part_dry_highlights))


GATES = PlantGates("灰烬苔 / hui_jin_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("HuiJinTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
