"""井心藻：暗青外圈与翠青内圈组成的发光莲座。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "deep_teal": (12, 50, 52),
    "outer_teal": (26, 126, 120),
    "inner_teal": (47, 214, 200),
    "center_glow": (116, 255, 240),
    "shadow_teal": (18, 70, 68),
}


def part_lotus_base(rig):
    rig.bone("lotus_base", (0.0, 0.0, 0.0))
    pad(rig, "lotus_base", "deep_center", (0.0, 0.18, 0.0), (4.8, 1.15, 4.8), "deep_teal")
    for name, x, z, w, d in (
        ("outer_base_front", 0.0, 1.95, 4.9, 1.45),
        ("outer_base_back", 0.0, -1.95, 4.9, 1.45),
        ("outer_base_left", -1.95, 0.0, 1.45, 3.8),
        ("outer_base_right", 1.95, 0.0, 1.45, 3.8),
    ):
        pad(rig, "lotus_base", name, (x, 0.48, z), (w, 0.72, d), "shadow_teal")


def _ring_point(index: int, radius: float, height: float) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
    angle = math.radians(index * 45.0)
    dx, dz = math.cos(angle), math.sin(angle)
    start = (dx * 0.6, height, dz * 0.6)
    end = (dx * radius, height + 2.55, dz * radius)
    return start, end


def part_outer_leaves(rig):
    rig.bone("outer_leaves", (0.0, 0.0, 0.0))
    for index in range(8):
        start, end = _ring_point(index, 3.05, 0.95)
        strand(rig, "outer_leaves", f"outer_stem_{index}", start, end, 0.39, "outer_teal")
        pad(rig, "outer_leaves", f"outer_blade_{index}", end, (0.72, 0.72, 1.15), "outer_teal")


def part_inner_leaves(rig):
    rig.bone("inner_leaves", (0.0, 0.0, 0.0))
    for index in range(8):
        angle = math.radians(index * 45.0 + 22.5)
        dx, dz = math.cos(angle), math.sin(angle)
        start = (dx * 0.18, 1.65, dz * 0.18)
        end = (dx * 1.8, 5.55, dz * 1.8)
        strand(rig, "inner_leaves", f"inner_stem_{index}", start, end, 0.36, "inner_teal")
        pad(rig, "inner_leaves", f"inner_blade_{index}", end, (0.65, 0.8, 1.1), "inner_teal")


def part_center_glow(rig):
    rig.bone("center_glow", (0.0, 0.0, 0.0))
    pad(rig, "center_glow", "glow_base", (0.0, 2.25, 0.0), (2.0, 1.3, 2.0), "center_glow")
    pad(rig, "center_glow", "glow_core", (0.0, 3.5, 0.0), (1.25, 1.1, 1.25), "center_glow")
    strand(rig, "center_glow", "glow_spire", (0.0, 3.9, 0.0), (0.0, 5.15, 0.0), 0.23, "center_glow")


def build():
    return build_rig(MATS, (part_lotus_base, part_outer_leaves, part_inner_leaves, part_center_glow))


GATES = PlantGates("井心藻 / jing_xin_zao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("JingXinZao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
