"""萤渊菇：深穴浅层的暖橙发光菌伞与暗色菌丝。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "cave_stone": (42, 38, 36),
    "stalk_rust": (106, 52, 30),
    "amber_cap": (191, 83, 27),
    "orange_glow": (255, 144, 45),
    "glow_hot": (255, 160, 64),
}


def part_cave_bed(rig):
    rig.bone("cave_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("cave_floor", 0.0, 0.0, 7.2, 0.95, 5.7, 0.1, "cave_stone"),
        ("cave_left", -2.45, 0.15, 2.35, 2.8, 3.6, 0.35, "cave_stone"),
        ("cave_back", 0.0, -1.85, 5.1, 2.55, 1.35, 0.35, "cave_stone"),
        ("cave_right", 2.45, -0.2, 2.3, 2.5, 3.4, 0.35, "cave_stone"),
        ("cave_front_lip", 0.0, 1.7, 2.0, 1.45, 1.15, 0.25, "cave_stone"),
    ):
        pad(rig, "cave_bed", name, (x, y, z), (w, h, d), mat)


def part_warm_stalks(rig):
    rig.bone("warm_stalks", (0.0, 0.0, 0.0))
    for name, start, end, radius in (
        ("stalk_main", (0.0, 1.35, -0.1), (0.05, 6.35, -0.1), 0.48),
        ("stalk_left", (-1.7, 1.45, 0.2), (-1.95, 5.1, 0.2), 0.34),
        ("stalk_right", (1.55, 1.35, -0.25), (1.7, 4.75, -0.3), 0.34),
    ):
        strand(rig, "warm_stalks", name, start, end, radius, "stalk_rust")


def part_amber_caps(rig):
    rig.bone("amber_caps", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("cap_main_gill", 0.05, -0.1, 4.0, 0.55, 2.9, 6.15, "orange_glow"),
        ("cap_main", 0.05, -0.1, 4.8, 1.0, 3.45, 6.62, "amber_cap"),
        ("cap_main_hot", 0.05, -0.1, 3.25, 0.45, 2.25, 7.48, "glow_hot"),
        ("cap_left_gill", -1.95, 0.2, 2.55, 0.42, 1.8, 4.95, "orange_glow"),
        ("cap_left", -1.95, 0.2, 3.1, 0.75, 2.15, 5.32, "amber_cap"),
        ("cap_right_gill", 1.7, -0.25, 2.4, 0.4, 1.7, 4.6, "orange_glow"),
        ("cap_right", 1.7, -0.25, 2.85, 0.7, 2.0, 4.95, "amber_cap"),
    ):
        pad(rig, "amber_caps", name, (x, y, z), (w, h, d), mat)


def part_glow_filaments(rig):
    rig.bone("glow_filaments", (0.0, 0.0, 0.0))
    for name, start, end, radius in (
        ("filament_main", (-1.55, 7.72, 0.2), (1.6, 7.72, -0.25), 0.22),
        ("filament_left", (-2.2, 5.72, 0.25), (-1.1, 5.78, 0.25), 0.18),
        ("filament_right", (1.15, 5.35, -0.25), (2.05, 5.38, -0.25), 0.18),
        ("filament_rock_left", (-3.05, 1.35, 1.98), (-1.85, 2.55, 1.98), 0.2),
        ("filament_rock_right", (3.05, 1.4, 1.72), (1.95, 2.45, 1.72), 0.2),
        ("filament_rock_front", (-0.8, 0.95, 2.3), (0.95, 1.45, 2.3), 0.16),
    ):
        strand(rig, "glow_filaments", name, start, end, radius, "orange_glow")
    for name, x, z in (("hot_main", 0.0, -0.1), ("hot_left", -1.95, 0.2), ("hot_right", 1.7, -0.25)):
        pad(rig, "glow_filaments", name, (x, 7.96 if name == "hot_main" else 5.75, z), (0.3, 0.25, 0.3), "glow_hot")


def build():
    return build_rig(MATS, (part_cave_bed, part_warm_stalks, part_amber_caps, part_glow_filaments))


GATES = PlantGates("萤渊菇 / ying_yuan_gu")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YingYuanGu", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
