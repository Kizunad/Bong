"""终焉藤：毒蛊师的焦黑终极藤材。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

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


def part_terminal_vines(rig):
    rig.bone("terminal_vines", (0.0, 0.0, 0.0))
    segments = (
        ("vine_center_0", (-0.5, 0.9, 0.0), (-1.0, 2.65, 0.05), 0.46),
        ("vine_center_1", (-1.0, 2.65, 0.05), (0.15, 4.15, 0.0), 0.42),
        ("vine_center_2", (0.15, 4.15, 0.0), (-0.55, 5.75, 0.2), 0.38),
        ("vine_left_0", (-1.55, 1.15, 0.4), (-2.35, 2.95, 0.5), 0.34),
        ("vine_left_1", (-2.35, 2.95, 0.5), (-1.75, 4.85, 0.65), 0.3),
        ("vine_right_0", (0.7, 1.3, -0.35), (2.0, 3.05, -0.35), 0.36),
        ("vine_right_1", (2.0, 3.05, -0.35), (2.45, 5.15, -0.1), 0.3),
        ("vine_reach", (0.05, 3.9, 0.0), (1.45, 5.7, 0.3), 0.28),
    )
    for name, start, end, radius in segments:
        strand(rig, "terminal_vines", name, start, end, radius, "charcoal_vine")


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
    for index, (x, y, z, dx, dz) in enumerate(
        (
            (-1.25, 2.3, 0.35, -0.7, 0.2),
            (-2.05, 3.75, 0.65, -0.55, 0.35),
            (-1.0, 4.95, 0.8, -0.65, 0.15),
            (1.2, 2.45, -0.3, 0.7, -0.2),
            (2.15, 4.05, -0.05, 0.65, -0.3),
            (1.0, 5.1, 0.45, 0.7, 0.2),
        )
    ):
        strand(rig, "poison_thorns", f"thorn_{index}", (x, y, z), (x + dx, y + 0.35, z + dz), 0.22, "bone_thorn")


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
