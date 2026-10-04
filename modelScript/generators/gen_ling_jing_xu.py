"""灵晶须：裂晶柱周围缠绕的紫晶须与细碎晶刺。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "crater_stone": (51, 49, 56),
    "crystal_dark": (87, 42, 112),
    "crystal_purple": (145, 74, 189),
    "crystal_light": (202, 132, 233),
    "crystal_glow": (142, 78, 255),
}


def part_crater_stone(rig):
    rig.bone("crater_stone", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("crater_floor", 0.0, 0.0, 7.1, 0.9, 5.7, 0.1, "crater_stone"),
        ("crater_left", -2.2, 0.15, 2.1, 1.55, 3.2, 0.42, "crater_stone"),
        ("crater_back", 0.2, -1.8, 4.6, 1.3, 1.2, 0.5, "crater_stone"),
        ("crater_right", 2.25, -0.2, 1.5, 1.2, 2.55, 0.4, "crater_stone"),
    ):
        pad(rig, "crater_stone", name, (x, y, z), (w, h, d), mat)


def part_cracked_pillar(rig):
    rig.bone("cracked_pillar", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("pillar_dark", 0.0, -0.15, 2.2, 3.0, 2.0, 0.75, "crystal_dark"),
        ("pillar_mid", 0.15, -0.1, 1.55, 2.8, 1.45, 3.55, "crystal_purple"),
        ("pillar_cap", 0.18, -0.1, 1.0, 1.2, 0.95, 6.2, "crystal_light"),
    ):
        pad(rig, "cracked_pillar", name, (x, y, z), (w, h, d), mat)


def part_crystal_whiskers(rig):
    rig.bone("crystal_whiskers", (0.0, 0.0, 0.0))
    for index in range(6):
        angle = math.radians(index * 60.0 + 15.0)
        dx, dz = math.cos(angle), math.sin(angle)
        start = (dx * 0.45, 1.8 + (index % 2) * 0.25, -0.15 + dz * 0.45)
        mid = (dx * 1.8, 4.0 + (index % 3) * 0.25, -0.15 + dz * 1.8)
        end = (dx * 3.0, 6.1 + (index % 2) * 0.5, -0.15 + dz * 3.0)
        strand(rig, "crystal_whiskers", f"whisker_{index}_lower", start, mid, 0.3, "crystal_purple")
        strand(rig, "crystal_whiskers", f"whisker_{index}_tip", mid, end, 0.22, "crystal_light")


def part_crystal_glow(rig):
    rig.bone("crystal_glow", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("glow_crack_left", (-0.72, 3.3, 0.0), (-0.15, 6.2, 0.0)),
        ("glow_crack_right", (0.65, 3.5, -0.15), (0.3, 6.8, -0.1)),
        ("glow_whisker", (-1.25, 4.35, 1.0), (-2.5, 6.2, 2.0)),
    ):
        strand(rig, "crystal_glow", name, start, end, 0.22, "crystal_glow")


def build():
    return build_rig(MATS, (part_crater_stone, part_cracked_pillar, part_crystal_whiskers, part_crystal_glow))


GATES = PlantGates("灵晶须 / ling_jing_xu")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("LingJingXu", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
