"""井心藻：灵泉眼中心的翠青藻环和向水心舒展的叶片。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, leaf, pad, strand, write_model  # noqa: E402

MATS = {
    "well_water": (38, 92, 83),
    "algae_deep": (29, 76, 59),
    "algae_green": (68, 130, 76),
    "algae_lit": (125, 168, 87),
    "well_stone": (71, 82, 70),
}


def part_well_stone(rig):
    rig.bone("well_stone", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("well_ring_front", 0.0, 2.2, 5.3, 1.0),
        ("well_ring_back", 0.0, -2.2, 5.3, 1.0),
        ("well_ring_l", -2.25, 0.0, 1.0, 3.5),
        ("well_ring_r", 2.25, 0.0, 1.0, 3.5),
    ):
        pad(rig, "well_stone", name, (x, 0.16, z), (w, 0.34, d), "well_stone")


def part_water_heart(rig):
    rig.bone("water_heart", (0.0, 0.0, 0.0))
    pad(rig, "water_heart", "water_core", (0.0, 0.18, 0.0), (3.7, 0.28, 3.7), "well_water")
    pad(rig, "water_heart", "water_deep", (0.0, 0.42, 0.0), (2.5, 0.24, 2.5), "algae_deep")


def part_algae_fronds(rig):
    rig.bone("algae_fronds", (0.0, 0.0, 0.0))
    fronds = (
        ("front", 0.0, 1.45, 0.0, 0.0, 0.42),
        ("back", 0.0, -1.45, 0.0, 180.0, -0.42),
        ("left", -1.45, 0.0, -90.0, 0.0, 0.0),
        ("right", 1.45, 0.0, 90.0, 0.0, 0.0),
    )
    for name, x, z, yaw, _unused, dz in fronds:
        start = (x, 0.45, z)
        end = (x * 0.68, 1.65, z + dz)
        strand(rig, "algae_fronds", f"frond_{name}_stem", start, end, 0.16, "algae_green")
        leaf(rig, "algae_fronds", f"frond_{name}_blade", (end[0], 1.45, end[2]), 0.58, 1.25, 0.34, "algae_lit", tilt=(0.0, yaw, 0.0))
    for index, (x, z) in enumerate(((-0.8, 0.85), (0.8, 0.85), (-0.8, -0.85), (0.8, -0.85))):
        strand(rig, "algae_fronds", f"inner_stem_{index}", (x, 0.42, z), (x * 0.55, 1.2, z * 0.55), 0.12, "algae_deep")


def part_algae_lit(rig):
    rig.bone("algae_lit", (0.0, 0.0, 0.0))
    for name, x, z in (("lit_front", 0.0, 1.85), ("lit_left", -1.85, 0.0), ("lit_right", 1.85, 0.0)):
        pad(rig, "algae_lit", name, (x, 1.05, z), (0.32, 0.3, 0.62), "algae_lit")


def build():
    return build_rig(MATS, (part_well_stone, part_water_heart, part_algae_fronds, part_algae_lit))


GATES = PlantGates("井心藻 / jing_xin_zao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("JingXinZao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
