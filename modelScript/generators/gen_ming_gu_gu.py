"""冥骨菇：骨白菌伞、暗色菌褶与坍缩渊残骨基床。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "bone_white": (218, 213, 198),
    "bone_shadow": (124, 119, 108),
    "soot": (36, 32, 30),
    "cap_high": (238, 235, 220),
    "spore_gray": (171, 163, 148),
}


def part_bone_bed(rig):
    rig.bone("bone_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bone_floor", 0.0, 0.0, 7.1, 0.9, 5.5, 0.1, "soot"),
        ("bone_left", -2.0, 0.15, 2.4, 1.7, 3.1, 0.45, "bone_shadow"),
        ("bone_back", 0.2, -1.85, 4.8, 1.45, 1.1, 0.5, "bone_white"),
        ("bone_front", 0.15, 1.85, 4.0, 1.2, 0.9, 0.35, "soot"),
    ):
        pad(rig, "bone_bed", name, (x, y, z), (w, h, d), mat)


def part_stalks(rig):
    rig.bone("stalks", (0.0, 0.0, 0.0))
    for name, start, end, radius in (
        ("stalk_main", (0.0, 0.0, 0.0), (0.0, 6.0, -0.15), 0.46),
        ("stalk_left", (-1.75, 1.0, 0.15), (-1.95, 5.0, 0.2), 0.36),
        ("stalk_right", (1.55, 0.9, -0.2), (1.8, 4.55, -0.3), 0.34),
    ):
        strand(rig, "stalks", name, start, end, radius, "bone_white")


def part_caps_and_gills(rig):
    rig.bone("caps_and_gills", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y in (
        ("cap_main_gill", 0.0, -0.15, 4.0, 0.55, 2.9, 5.95),
        ("cap_main", 0.0, -0.15, 4.7, 0.95, 3.4, 6.45),
        ("cap_main_high", 0.0, -0.15, 3.5, 0.55, 2.45, 7.25),
        ("cap_left_gill", -1.95, 0.2, 2.65, 0.45, 1.9, 4.85),
        ("cap_left", -1.95, 0.2, 3.15, 0.75, 2.25, 5.25),
        ("cap_right_gill", 1.8, -0.25, 2.45, 0.4, 1.65, 4.35),
        ("cap_right", 1.8, -0.25, 2.9, 0.7, 2.0, 4.72),
    ):
        mat = "bone_shadow" if "gill" in name else "bone_white"
        if name == "cap_main_high":
            mat = "cap_high"
        pad(rig, "caps_and_gills", name, (x, y, z), (w, h, d), mat)


def part_spores(rig):
    rig.bone("spores", (0.0, 0.0, 0.0))
    for name, x, z, y in (
        ("spore_main_l", -1.25, 0.65, 7.95),
        ("spore_main_r", 1.15, -0.35, 7.85),
        ("spore_left", -2.65, 0.25, 6.05),
        ("spore_right", 2.55, -0.35, 5.5),
    ):
        pad(rig, "spores", name, (x, y, z), (0.32, 0.28, 0.32), "spore_gray")


def build():
    return build_rig(MATS, (part_bone_bed, part_stalks, part_caps_and_gills, part_spores))


GATES = PlantGates("冥骨菇 / ming_gu_gu")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("MingGuGu", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
