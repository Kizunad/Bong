"""养经苔：灰白阶梯岩块上的锈绿苔垫与锈橙斑点。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "stone_base": (177, 181, 178),
    "stone_shadow": (123, 128, 126),
    "stone_high": (208, 211, 205),
    "moss": (107, 122, 42),
    "rust_spot": (160, 96, 42),
}


def part_stone_steps(rig):
    rig.bone("stone_steps", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("stone_base", 0.0, 0.0, 7.3, 1.25, 5.4, 0.12, "stone_base"),
        ("stone_front", 0.0, 1.8, 5.8, 1.35, 1.25, 0.65, "stone_shadow"),
        ("stone_left", -2.0, 0.1, 3.1, 2.45, 3.25, 0.8, "stone_high"),
        ("stone_right", 2.15, -0.2, 2.55, 2.2, 3.0, 0.9, "stone_shadow"),
        ("stone_mid", 0.0, -0.35, 4.7, 2.4, 3.0, 2.0, "stone_base"),
        ("stone_top_left", -1.0, -0.55, 2.8, 1.55, 2.35, 4.25, "stone_high"),
        ("stone_top_right", 1.25, -0.45, 2.25, 1.3, 2.0, 4.2, "stone_shadow"),
    ):
        pad(rig, "stone_steps", name, (x, y, z), (w, h, d), mat)


def part_moss_pads(rig):
    rig.bone("moss_pads", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y in (
        ("moss_main", 0.0, -0.35, 4.8, 1.0, 2.9, 5.8),
        ("moss_left", -1.85, 0.1, 1.55, 0.8, 1.55, 5.25),
        ("moss_right", 1.9, -0.1, 1.45, 0.75, 1.6, 5.15),
        ("moss_front", 0.0, 1.25, 3.3, 0.65, 0.65, 5.05),
    ):
        pad(rig, "moss_pads", name, (x, y, z), (w, h, d), "moss")


def part_moss_strands(rig):
    rig.bone("moss_strands", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("sprout_left", (-1.9, 5.9, 0.0), (-2.25, 7.0, 0.2)),
        ("sprout_right", (1.5, 5.85, -0.2), (1.85, 6.85, -0.4)),
        ("sprout_center", (0.0, 6.55, -0.35), (0.15, 7.35, -0.25)),
    ):
        strand(rig, "moss_strands", name, start, end, 0.2, "moss")


def part_rust_spots(rig):
    rig.bone("rust_spots", (0.0, 0.0, 0.0))
    for name, x, z, y, w, h, d in (
        ("spot_front", -0.85, 1.86, 4.1, 0.42, 0.32, 0.22),
        ("spot_left", -2.0, 1.55, 4.0, 0.3, 0.38, 0.28),
        ("spot_top", 1.05, -0.55, 6.88, 0.48, 0.22, 0.42),
        ("spot_back", 0.15, -1.8, 5.7, 0.32, 0.25, 0.26),
    ):
        pad(rig, "rust_spots", name, (x, y, z), (w, h, d), "rust_spot")


def build():
    return build_rig(MATS, (part_stone_steps, part_moss_pads, part_moss_strands, part_rust_spots))


GATES = PlantGates("养经苔 / yang_jing_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YangJingTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
