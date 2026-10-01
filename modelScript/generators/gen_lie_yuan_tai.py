"""裂渊苔：紫黑裂缝口的两侧苔脊和压差微光。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "rift_bed": (28, 20, 31),
    "rift_moss": (56, 29, 63),
    "rift_edge": (91, 40, 96),
    "pressure_glow": (126, 73, 139),
    "dry_black": (20, 18, 24),
}


def part_rift_bed(rig):
    rig.bone("rift_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, d, h in (
        ("rift_floor", 0.0, 0.0, 6.8, 4.5, 0.34),
        ("rift_front", 0.0, 2.2, 5.7, 1.3, 0.26),
        ("rift_back", 0.0, -2.1, 5.9, 1.1, 0.28),
    ):
        pad(rig, "rift_bed", name, (x, 0.12, z), (w, h, d), "rift_bed")


def part_rift_ridges(rig):
    rig.bone("rift_ridges", (0.0, 0.0, 0.0))
    for side, x in (("l", -2.15), ("r", 2.15)):
        pad(rig, "rift_ridges", f"ridge_{side}_base", (x, 0.42, 0.0), (1.2, 0.58, 3.9), "rift_moss")
        pad(rig, "rift_ridges", f"ridge_{side}_tip", (x * 0.83, 0.9, -0.1), (0.82, 0.7, 2.6), "rift_edge")
    pad(rig, "rift_ridges", "ridge_front_lock", (0.0, 0.43, 1.72), (3.5, 0.52, 0.62), "rift_moss")


def part_pressure_filaments(rig):
    rig.bone("pressure_filaments", (0.0, 0.0, 0.0))
    for side, x in (("l", -2.08), ("r", 2.08)):
        strand(rig, "pressure_filaments", f"pressure_{side}_a", (x, 0.72, -1.15), (x * 0.74, 1.75, -0.55), 0.16, "pressure_glow")
        strand(rig, "pressure_filaments", f"pressure_{side}_b", (x, 0.7, 0.72), (x * 0.68, 1.45, 0.92), 0.14, "pressure_glow")
    strand(rig, "pressure_filaments", "pressure_core", (-0.65, 0.72, 0.0), (0.65, 0.72, 0.0), 0.12, "pressure_glow")


def part_dry_black(rig):
    rig.bone("dry_black", (0.0, 0.0, 0.0))
    for name, x, z in (("black_l", -1.25, -1.55), ("black_r", 1.3, 1.4), ("black_front", 0.0, 2.05)):
        pad(rig, "dry_black", name, (x, 0.68, z), (0.7, 0.35, 0.9), "dry_black")


def build():
    return build_rig(MATS, (part_rift_bed, part_rift_ridges, part_pressure_filaments, part_dry_black))


GATES = PlantGates("裂渊苔 / lie_yuan_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("LieYuanTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
