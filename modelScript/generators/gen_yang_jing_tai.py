"""养经苔：死域边缘的锈绿苔面和向外分叉的经脉纹。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, leaf, pad, strand, write_model  # noqa: E402

MATS = {
    "rust_bed": (82, 57, 37),
    "rust_edge": (125, 77, 42),
    "meridian_green": (83, 111, 50),
    "meridian_light": (132, 145, 61),
    "scar_dark": (44, 42, 30),
}


def part_rust_bed(rig):
    rig.bone("rust_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, d, h in (
        ("rust_core", 0.0, 0.0, 5.8, 4.5, 0.42),
        ("rust_front", 0.0, 2.05, 4.9, 1.25, 0.34),
        ("rust_back", 0.0, -2.0, 4.6, 1.15, 0.36),
        ("rust_l", -2.45, 0.05, 1.3, 2.6, 0.38),
        ("rust_r", 2.45, -0.05, 1.3, 2.6, 0.38),
    ):
        pad(rig, "rust_bed", name, (x, 0.15, z), (w, h, d), "rust_bed")
    for name, x, z, w, d in (
        ("rust_edge_front", 0.0, 2.3, 4.2, 0.24),
        ("rust_edge_left", -2.6, 0.0, 0.24, 2.1),
        ("rust_edge_right", 2.6, 0.0, 0.24, 2.1),
    ):
        pad(rig, "rust_bed", name, (x, 0.56, z), (w, 0.24, d), "rust_edge")


def part_meridian_mats(rig):
    rig.bone("meridian_mats", (0.0, 0.0, 0.0))
    for side, sign in (("l", -1.0), ("r", 1.0)):
        strand(rig, "meridian_mats", f"vein_{side}_front", (0.0, 0.55, 0.45), (sign * 2.28, 0.96, 1.7), 0.16, "meridian_green")
        strand(rig, "meridian_mats", f"vein_{side}_back", (0.0, 0.53, -0.35), (sign * 2.2, 0.9, -1.65), 0.15, "meridian_green")
        strand(rig, "meridian_mats", f"vein_{side}_side", (0.0, 0.5, 0.0), (sign * 2.65, 0.8, 0.0), 0.18, "meridian_light")
    strand(rig, "meridian_mats", "vein_spine", (-0.15, 0.54, 0.0), (0.15, 1.08, 0.0), 0.19, "meridian_light")


def part_meridian_leaves(rig):
    rig.bone("meridian_leaves", (0.0, 0.0, 0.0))
    for side, sign in (("l", -1.0), ("r", 1.0)):
        for index, z in enumerate((-0.95, 0.0, 0.95)):
            leaf(rig, "meridian_leaves", f"leaf_{side}_{index}", (sign * 1.65, 0.78, z), 0.55, 1.0, 0.3, "meridian_green", tilt=(0.0, sign * 12.0, sign * 5.0))


def part_scar_dark(rig):
    rig.bone("scar_dark", (0.0, 0.0, 0.0))
    for name, x, z in (("scar_l", -1.0, 1.75), ("scar_r", 1.1, -1.75), ("scar_front", 0.0, 2.0)):
        pad(rig, "scar_dark", name, (x, 0.75, z), (0.28, 0.25, 0.7), "scar_dark")


def build():
    return build_rig(MATS, (part_rust_bed, part_meridian_mats, part_meridian_leaves, part_scar_dark))


GATES = PlantGates("养经苔 / yang_jing_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YangJingTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
