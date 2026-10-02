"""焦脉藤：焦黑藤蔓与未熄的橙红炭线。"""

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
    "scorch_bed": (52, 45, 40),
    "char_black": (30, 27, 26),
    "char_high": (68, 57, 50),
    "ember_red": (184, 53, 25),
    "ember_hot": (255, 113, 45),
    "ash_leaf": (78, 67, 58),
}


def part_scorched_bed(rig):
    rig.bone("scorched_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bed_floor", 0.0, 0.0, 7.1, 0.8, 5.6, 0.1, "scorch_bed"),
        ("bed_left", -2.25, 0.2, 2.2, 1.25, 3.2, 0.35, "char_black"),
        ("bed_right", 2.25, -0.2, 2.1, 1.05, 2.8, 0.35, "scorch_bed"),
        ("bed_back", 0.0, -1.8, 4.8, 1.1, 1.15, 0.35, "char_high"),
    ):
        pad(rig, "scorched_bed", name, (x, y, z), (w, h, d), mat)


def part_charred_vines(rig):
    rig.bone("charred_vines", (0.0, 0.0, 0.0))
    for vine_index in range(6):
        phase = math.tau * vine_index / 6.0
        points = []
        for step in range(10):
            t = step / 9.0
            angle = phase + math.tau * 1.2 * t
            radius = 2.75 + 0.45 * math.sin(math.pi * t)
            points.append(
                (
                    radius * math.cos(angle),
                    0.85 + 5.05 * t,
                    radius * math.sin(angle),
                )
            )
        curved_vine_chain(
            rig,
            "charred_vines",
            f"vine_spiral_{vine_index}",
            points,
            0.48,
            0.30,
            "char_black",
        )


def part_ember_core(rig):
    rig.bone("ember_core", (0.0, 0.0, 0.0))
    strand(
        rig,
        "ember_core",
        "ember_vertical_core",
        (0.0, 0.95, 0.0),
        (0.0, 6.1, 0.0),
        0.9,
        "ember_hot",
    )
    for name, start, end, mat in (
        ("ember_side_left", (-2.4, 1.3, 0.5), (-2.55, 2.65, 0.6), "ember_red"),
        ("ember_side_right", (2.3, 3.1, -0.35), (2.5, 4.7, -0.2), "ember_red"),
        ("ember_upper", (-1.7, 4.65, -0.25), (-0.8, 5.75, -0.05), "ember_hot"),
    ):
        strand(rig, "ember_core", name, start, end, 0.2, mat)


def part_ash_leaves(rig):
    rig.bone("ash_leaves", (0.0, 0.0, 0.0))
    for name, center, size in (
        ("leaf_left", (-2.05, 4.78, 0.72), (0.9, 0.3, 0.55)),
        ("leaf_mid", (0.85, 4.05, -0.28), (0.85, 0.28, 0.5)),
        ("leaf_right", (2.52, 5.35, -0.25), (0.75, 0.28, 0.45)),
    ):
        pad(rig, "ash_leaves", name, center, size, "ash_leaf")


def build():
    return build_rig(MATS, (part_scorched_bed, part_charred_vines, part_ember_core, part_ash_leaves))


GATES = PlantGates("焦脉藤 / jiao_mai_teng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("JiaoMaiTeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
