"""玄绒苔：深渊温差带的漆黑绒面与银色近手高光。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, leaf, pad, strand, write_model  # noqa: E402

MATS = {
    "velvet_bed": (18, 19, 24),
    "velvet_mid": (29, 31, 39),
    "velvet_edge": (47, 43, 51),
    "silver_glint": (159, 165, 151),
    "cold_shadow": (12, 13, 18),
}


def part_velvet_bed(rig):
    rig.bone("velvet_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, d, h in (
        ("velvet_center", 0.0, 0.0, 6.0, 4.8, 0.42),
        ("velvet_front", 0.0, 2.2, 4.8, 1.3, 0.35),
        ("velvet_back", 0.0, -2.1, 4.7, 1.15, 0.38),
        ("velvet_l", -2.55, 0.1, 1.2, 2.9, 0.4),
        ("velvet_r", 2.55, -0.15, 1.2, 2.9, 0.4),
    ):
        pad(rig, "velvet_bed", name, (x, 0.16, z), (w, h, d), "velvet_bed")


def part_velvet_lobes(rig):
    rig.bone("velvet_lobes", (0.0, 0.0, 0.0))
    for suffix, x, z, tilt in (
        ("c", 0.0, 0.0, (0.0, 0.0, 0.0)),
        ("l", -1.55, 0.25, (0.0, 0.0, -8.0)),
        ("r", 1.55, -0.25, (0.0, 0.0, 8.0)),
    ):
        leaf(rig, "velvet_lobes", f"lobe_{suffix}_base", (x, 0.52, z), 2.0, 1.55, 0.44, "velvet_mid", tilt=tilt)
        leaf(rig, "velvet_lobes", f"lobe_{suffix}_edge", (x, 0.83, z + 0.1), 1.45, 1.25, 0.36, "velvet_edge", tilt=tilt)


def part_silver_hairs(rig):
    rig.bone("silver_hairs", (0.0, 0.0, 0.0))
    for side, x in (("l", -1.35), ("r", 1.35)):
        for index, z in enumerate((-0.72, 0.0, 0.72)):
            strand(rig, "silver_hairs", f"silver_{side}_{index}", (x, 0.72, z), (x * 0.75, 1.62, z + 0.08), 0.12, "silver_glint")
    strand(rig, "silver_hairs", "silver_center", (0.0, 0.84, -0.15), (0.0, 1.75, 0.15), 0.12, "silver_glint")


def part_cold_shadow(rig):
    rig.bone("cold_shadow", (0.0, 0.0, 0.0))
    for name, x, z in (("shadow_l", -2.0, 1.15), ("shadow_r", 2.0, -1.1)):
        pad(rig, "cold_shadow", name, (x, 0.66, z), (0.46, 0.32, 0.8), "cold_shadow")


def build():
    return build_rig(MATS, (part_velvet_bed, part_velvet_lobes, part_silver_hairs, part_cold_shadow))


GATES = PlantGates("玄绒苔 / xuan_rong_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XuanRongTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
