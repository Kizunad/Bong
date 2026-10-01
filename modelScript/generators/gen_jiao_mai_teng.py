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
    for vine_index in range(10):
        phase = math.tau * vine_index / 10.0
        points = []
        for step in range(10):
            t = step / 9.0
            angle = phase + math.tau * 1.35 * t
            radius = 0.65 + 0.72 * math.sin(math.pi * t)
            points.append(
                (
                    radius * math.cos(angle),
                    0.78 + 5.2 * t,
                    radius * math.sin(angle),
                )
            )
        curved_vine_chain(
            rig,
            "charred_vines",
            f"vine_spiral_{vine_index}",
            points,
            0.62,
            0.38,
            "char_black",
        )


def part_ember_core(rig):
    rig.bone("ember_core", (0.0, 0.0, 0.0))
    for name, start, end, mat in (
        ("ember_main_0", (-1.95, 1.0, 0.35), (-1.2, 2.3, 0.38), "ember_red"),
        ("ember_main_1", (-1.1, 2.42, 0.38), (0.15, 3.55, 0.2), "ember_hot"),
        ("ember_main_2", (0.3, 3.7, 0.2), (1.65, 4.72, 0.5), "ember_red"),
        ("ember_side", (-2.25, 3.2, 0.88), (-1.85, 4.75, 0.95), "ember_hot"),
        ("ember_right", (2.4, 3.62, -0.35), (2.6, 5.4, -0.15), "ember_red"),
    ):
        strand(rig, "ember_core", name, start, end, 0.26, mat)


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
