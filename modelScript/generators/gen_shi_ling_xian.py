"""噬灵藓：黑色苔垫上向外放射、带暗红叶脉的尖叶莲座。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "root_bed": (26, 20, 20),
    "root_mid": (48, 29, 31),
    "leaf_black": (26, 20, 20),
    "leaf_shadow": (14, 11, 13),
    "vein_red": (138, 28, 28),
}


def part_root_bed(rig):
    rig.bone("root_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("root_center", 0.0, 0.0, 5.6, 1.55, 4.8, 0.16, "root_bed"),
        ("root_front", 0.0, 1.9, 4.5, 1.05, 1.35, 0.35, "root_mid"),
        ("root_left", -2.35, 0.2, 1.45, 1.2, 2.6, 0.28, "root_mid"),
        ("root_right", 2.3, -0.25, 1.4, 1.3, 2.55, 0.25, "root_bed"),
        ("root_back", 0.15, -1.8, 3.8, 1.1, 1.15, 0.38, "root_shadow"),
    ):
        if mat == "root_shadow":
            mat = "leaf_shadow"
        pad(rig, "root_bed", name, (x, y, z), (w, h, d), mat)


def _leaf_points(index: int) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
    angle = math.radians(index * 36.0 + 9.0)
    dx, dz = math.cos(angle), math.sin(angle)
    return (dx * 0.35, 1.15, dz * 0.35), (dx * 3.05, 5.25, dz * 3.05)


def part_pointed_rosette(rig):
    rig.bone("pointed_rosette", (0.0, 0.0, 0.0))
    for index in range(10):
        start, tip = _leaf_points(index)
        end = (tip[0] * 0.88, 4.48, tip[2] * 0.88)
        strand(rig, "pointed_rosette", f"leaf_{index}", start, end, 0.46, "leaf_black")
        strand(rig, "pointed_rosette", f"leaf_tip_{index}", end, tip, 0.24, "leaf_shadow")


def part_leaf_veins(rig):
    rig.bone("leaf_veins", (0.0, 0.0, 0.0))
    for index in range(10):
        start, tip = _leaf_points(index)
        vein_start = (start[0], start[1] + 0.46, start[2])
        vein_end = (tip[0] * 0.86, tip[1] - 0.3, tip[2] * 0.86)
        strand(rig, "leaf_veins", f"vein_{index}", vein_start, vein_end, 0.11, "vein_red")


def part_root_shadows(rig):
    rig.bone("root_shadows", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("shadow_front", 0.0, 2.35, 1.0, 0.45),
        ("shadow_left", -2.5, -0.35, 0.55, 1.25),
        ("shadow_right", 2.45, 0.5, 0.55, 1.2),
    ):
        pad(rig, "root_shadows", name, (x, 1.0, z), (w, 0.32, d), "leaf_shadow")


def build():
    return build_rig(MATS, (part_root_bed, part_pointed_rosette, part_leaf_veins, part_root_shadows))


GATES = PlantGates("噬灵藓 / shi_ling_xian")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ShiLingXian", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
