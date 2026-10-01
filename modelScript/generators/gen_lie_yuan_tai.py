"""裂渊苔：黑灰碎石堆上的紫苔与亮紫裂纹。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "rift_stone": (43, 43, 46),
    "rift_stone_light": (82, 80, 84),
    "purple_moss": (106, 47, 160),
    "dark_purple": (58, 26, 90),
    "rift_glow": (192, 112, 255),
}


def part_rift_stones(rig):
    rig.bone("rift_stones", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, mat in (
        ("stone_floor", 0.0, 0.0, 7.4, 0.75, 5.7, "rift_stone"),
        ("stone_left", -2.0, 0.2, 3.4, 2.8, 3.5, "rift_stone_light"),
        ("stone_center", -0.15, -0.55, 3.8, 3.7, 3.2, "rift_stone"),
        ("stone_right", 2.15, -0.1, 2.7, 2.5, 3.45, "rift_stone_light"),
        ("stone_back", 0.05, -1.95, 4.8, 2.3, 1.45, "rift_stone"),
        ("stone_front", 0.05, 1.9, 4.9, 1.5, 1.1, "rift_stone_light"),
        ("stone_cap", 0.45, -0.45, 2.4, 1.3, 2.0, "rift_stone_light"),
    ):
        bottom = 0.12 if name == "stone_floor" else 0.55
        pad(rig, "rift_stones", name, (x, bottom, z), (w, h, d), mat)


def part_purple_moss(rig):
    rig.bone("purple_moss", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, mat, y in (
        ("moss_center", -0.05, -0.5, 4.5, 0.9, 3.0, "purple_moss", 4.15),
        ("moss_left", -2.0, 0.3, 1.65, 0.75, 2.0, "purple_moss", 3.35),
        ("moss_right", 2.0, -0.15, 1.5, 0.7, 2.0, "dark_purple", 3.25),
        ("moss_back", 0.0, -1.85, 3.6, 0.55, 0.9, "dark_purple", 3.0),
        ("moss_front", 0.0, 1.75, 3.3, 0.5, 0.75, "purple_moss", 2.55),
    ):
        pad(rig, "purple_moss", name, (x, y, z), (w, h, d), mat)


def part_rift_cracks(rig):
    rig.bone("rift_cracks", (0.0, 0.0, 0.0))
    for name, start, end, radius in (
        ("crack_main", (-1.8, 5.1, 1.05), (1.75, 5.25, -1.0), 0.18),
        ("crack_left", (-0.9, 5.17, 0.55), (-2.35, 4.15, -0.15), 0.13),
        ("crack_right", (0.25, 5.2, 0.15), (1.65, 4.0, 0.6), 0.13),
        ("crack_front", (-0.55, 3.15, 1.68), (0.7, 3.15, 1.68), 0.12),
    ):
        strand(rig, "rift_cracks", name, start, end, radius, "rift_glow")


def part_dark_purple(rig):
    rig.bone("dark_purple", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y in (
        ("dark_gap_left", -1.55, -1.3, 0.62, 0.8, 1.2, 2.35),
        ("dark_gap_right", 1.35, 1.05, 0.62, 0.9, 1.0, 2.65),
        ("dark_gap_front", 0.0, 2.08, 1.1, 0.5, 0.34, 2.2),
    ):
        pad(rig, "dark_purple", name, (x, y, z), (w, h, d), "dark_purple")


def build():
    return build_rig(MATS, (part_rift_stones, part_purple_moss, part_rift_cracks, part_dark_purple))


GATES = PlantGates("裂渊苔 / lie_yuan_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("LieYuanTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
