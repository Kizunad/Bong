"""终焉藤：毒蛊师的焦黑终极藤材。"""

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
    "ash_bed": (48, 42, 40),
    "charcoal_vine": (27, 24, 25),
    "bark_rust": (92, 45, 38),
    "toxin_red": (160, 36, 42),
    "ember_line": (236, 82, 34),
    "bone_thorn": (170, 154, 126),
}


def part_ash_bed(rig):
    rig.bone("ash_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bed_floor", 0.0, 0.0, 7.2, 0.85, 5.7, 0.1, "ash_bed"),
        ("bed_left", -2.3, 0.1, 2.1, 1.3, 3.0, 0.35, "ash_bed"),
        ("bed_right", 2.2, -0.2, 2.0, 1.1, 2.6, 0.35, "charcoal_vine"),
        ("bed_back", 0.0, -1.8, 4.8, 1.15, 1.2, 0.35, "bark_rust"),
    ):
        pad(rig, "ash_bed", name, (x, y, z), (w, h, d), mat)


def _tower_path(vine_index: int, count: int = 9):
    phase = math.tau * vine_index / 12.0
    points = []
    for step in range(count):
        t = step / (count - 1)
        angle = phase + math.tau * 1.05 * t
        radius = 0.55 + 2.25 * (1.0 - t) ** 0.62
        points.append(
            (
                radius * math.cos(angle),
                0.82 + 5.65 * t,
                radius * math.sin(angle),
            )
        )
    return points


def part_terminal_vines(rig):
    rig.bone("terminal_vines", (0.0, 0.0, 0.0))
    for vine_index in range(12):
        curved_vine_chain(
            rig,
            "terminal_vines",
            f"vine_tower_{vine_index}",
            _tower_path(vine_index),
            0.56,
            0.34,
            "charcoal_vine",
        )


def part_toxin_veins(rig):
    rig.bone("toxin_veins", (0.0, 0.0, 0.0))
    for name, start, end, mat in (
        ("vein_center", (-0.9, 2.7, 0.45), (0.1, 4.1, 0.4), "toxin_red"),
        ("vein_left", (-2.25, 3.0, 0.9), (-1.8, 4.75, 0.95), "ember_line"),
        ("vein_right", (1.95, 3.1, 0.1), (2.4, 5.05, 0.25), "toxin_red"),
        ("vein_reach", (0.15, 4.2, 0.4), (1.35, 5.65, 0.65), "ember_line"),
    ):
        strand(rig, "toxin_veins", name, start, end, 0.25, mat)


def part_poison_thorns(rig):
    rig.bone("poison_thorns", (0.0, 0.0, 0.0))
    thorn_index = 0
    for vine_index in range(12):
        points = _tower_path(vine_index)
        for point_index in (2, 5, 7):
            x, y, z = points[point_index]
            radial = math.hypot(x, z) or 1.0
            dx, dz = x / radial * 0.62, z / radial * 0.62
            strand(
                rig,
                "poison_thorns",
                f"thorn_{thorn_index}",
                (x, y, z),
                (x + dx, y + 0.32, z + dz),
                0.18,
                "bone_thorn",
            )
            thorn_index += 1


def build():
    return build_rig(MATS, (part_ash_bed, part_terminal_vines, part_toxin_veins, part_poison_thorns))


GATES = PlantGates("终焉藤 / zhong_yan_teng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ZhongYanTeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
