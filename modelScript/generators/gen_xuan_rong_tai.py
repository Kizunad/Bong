"""玄绒苔：漆黑隆起绒垫中央斜插冰蓝晶体。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "velvet_black": (20, 20, 22),
    "velvet_mid": (35, 35, 39),
    "velvet_edge": (49, 47, 53),
    "ice_crystal": (159, 216, 255),
    "blue_glow": (74, 184, 255),
}


def part_velvet_pad(rig):
    rig.bone("velvet_pad", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("pad_floor", 0.0, 0.0, 7.0, 0.9, 5.7, 0.12, "velvet_black"),
        ("pad_front", 0.0, 2.05, 5.5, 1.35, 1.45, 0.38, "velvet_mid"),
        ("pad_back", 0.0, -1.95, 5.5, 1.45, 1.35, 0.45, "velvet_mid"),
        ("pad_left", -2.45, 0.05, 1.55, 1.5, 3.4, 0.42, "velvet_edge"),
        ("pad_right", 2.45, -0.08, 1.55, 1.6, 3.35, 0.4, "velvet_edge"),
        ("pad_center", 0.0, -0.05, 4.8, 2.55, 3.9, 1.15, "velvet_mid"),
        ("pad_top", 0.05, -0.12, 3.7, 1.2, 3.0, 3.45, "velvet_black"),
    ):
        pad(rig, "velvet_pad", name, (x, y, z), (w, h, d), mat)


def part_velvet_tufts(rig):
    rig.bone("velvet_tufts", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("tuft_front", (0.0, 2.0, 1.0), (0.0, 4.8, 1.35)),
        ("tuft_left", (-1.5, 2.4, 0.2), (-2.15, 4.6, 0.35)),
        ("tuft_right", (1.45, 2.35, -0.25), (2.1, 4.5, -0.4)),
        ("tuft_back", (0.0, 2.4, -1.0), (0.0, 4.95, -1.35)),
    ):
        strand(rig, "velvet_tufts", name, start, end, 0.25, "velvet_black")


def part_ice_crystals(rig):
    rig.bone("ice_crystals", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("crystal_center", (0.0, 3.9, 0.0), (-0.45, 8.0, 0.25)),
        ("crystal_left", (-0.55, 3.85, -0.1), (-1.55, 7.0, -0.4)),
        ("crystal_right", (0.55, 3.82, 0.05), (1.65, 6.8, 0.15)),
        ("crystal_back", (0.1, 3.8, -0.6), (0.45, 6.55, -1.75)),
    ):
        strand(rig, "ice_crystals", name, start, end, 0.34, "ice_crystal")


def part_blue_cracks(rig):
    rig.bone("blue_cracks", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("glow_center", (-0.05, 4.15, 0.15), (-0.35, 6.65, 0.3)),
        ("glow_left", (-0.7, 3.85, -0.05), (-1.3, 5.85, -0.3)),
        ("glow_right", (0.6, 3.95, 0.05), (1.35, 5.7, 0.12)),
    ):
        strand(rig, "blue_cracks", name, start, end, 0.22, "blue_glow")


def build():
    return build_rig(MATS, (part_velvet_pad, part_velvet_tufts, part_ice_crystals, part_blue_cracks))


GATES = PlantGates("玄绒苔 / xuan_rong_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XuanRongTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
