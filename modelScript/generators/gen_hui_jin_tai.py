"""灰烬苔：灰黑碎岩隆起团块上的橙红余烬裂纹。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "ash_bed": (43, 42, 40),
    "ash_edge": (85, 82, 78),
    "soot": (27, 26, 24),
    "ember_crack": (224, 88, 42),
    "ember_hot": (255, 150, 64),
}


def part_ash_bed(rig):
    rig.bone("ash_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("ash_floor", 0.0, 0.0, 6.8, 5.2),
        ("ash_lump_left", -2.0, 0.25, 2.7, 3.3),
        ("ash_lump_center", 0.0, -0.1, 3.8, 3.4),
        ("ash_lump_right", 2.05, -0.15, 2.6, 3.1),
        ("ash_lump_back", 0.0, -1.85, 3.5, 1.25),
        ("ash_lump_front", -0.65, 1.65, 2.4, 1.05),
    ):
        height = {
            "ash_floor": 0.8,
            "ash_lump_left": 3.4,
            "ash_lump_center": 4.6,
            "ash_lump_right": 3.8,
            "ash_lump_back": 5.4,
            "ash_lump_front": 2.2,
        }[name]
        bottom = 0.1 if name == "ash_floor" else 0.65
        pad(rig, "ash_bed", name, (x, bottom, z), (w, height, d), "ash_bed")
    for name, x, z, w, d in (
        ("ash_edge_front", 0.0, 2.35, 4.3, 0.32),
        ("ash_edge_left", -2.55, 0.2, 0.32, 2.2),
        ("ash_edge_right", 2.55, -0.1, 0.32, 2.2),
    ):
        pad(rig, "ash_bed", name, (x, 1.0, z), (w, 0.55, d), "ash_edge")


def part_soot_crust(rig):
    rig.bone("soot_crust", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("soot_core", 0.0, 0.0, 3.4, 2.5),
        ("soot_l", -1.55, 0.6, 1.3, 1.2),
        ("soot_r", 1.55, -0.55, 1.2, 1.1),
        ("soot_front", 0.15, 1.35, 1.9, 0.72),
    ):
        pad(rig, "soot_crust", name, (x, 4.95, z), (w, 0.36, d), "soot")


def part_ember_cracks(rig):
    rig.bone("ember_cracks", (0.0, 0.0, 0.0))
    cracks = (
        ("ember_main", (-1.8, 5.55, 0.15), (0.0, 5.85, 0.45), 0.18),
        ("ember_branch_l", (-1.0, 5.7, 0.3), (-1.9, 5.85, 1.25), 0.14),
        ("ember_branch_r", (0.0, 5.85, 0.45), (1.75, 5.35, -0.25), 0.16),
        ("ember_front", (-0.5, 5.35, 1.45), (1.2, 5.1, 1.45), 0.13),
        # 侧面余烬裂纹 (Round 3 小修：灰烬苔侧面也加橙红余烬裂纹)
        ("ember_side_left", (-1.9, 5.2, 0.25), (-3.1, 1.6, 0.35), 0.14),
        ("ember_side_right", (1.75, 5.1, -0.25), (3.0, 1.5, -0.2), 0.14),
        ("ember_side_front", (-0.3, 4.8, 1.65), (-0.5, 1.8, 2.3), 0.13),
        ("ember_side_back", (0.2, 5.2, -1.8), (0.3, 2.2, -2.4), 0.13),
    )
    for name, start, end, radius in cracks:
        strand(rig, "ember_cracks", name, start, end, radius, "ember_crack")


def part_ember_hot(rig):
    rig.bone("ember_hot", (0.0, 0.0, 0.0))
    for name, x, z in (("ember_hot_l", -1.55, 0.3), ("ember_hot_r", 0.85, -0.1), ("ember_hot_front", 0.25, 1.55)):
        pad(rig, "ember_hot", name, (x, 5.82, z), (0.28, 0.22, 0.42), "ember_hot")


def build():
    return build_rig(MATS, (part_ash_bed, part_soot_crust, part_ember_cracks, part_ember_hot))


GATES = PlantGates("灰烬苔 / hui_jin_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("HuiJinTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
