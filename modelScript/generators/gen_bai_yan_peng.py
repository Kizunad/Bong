"""白盐蓬：灰白细叶与附着在叶尖的灵盐晶粒。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "dry_soil": (90, 83, 73),
    "stem_dark": (63, 69, 65),
    "leaf_gray": (216, 220, 224),
    "leaf_white": (240, 243, 244),
    "salt_crystal": (255, 255, 255),
}


def part_dry_soil(rig):
    rig.bone("dry_soil", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("soil_floor", 0.0, 0.0, 7.0, 0.8, 5.5, 0.1, "dry_soil"),
        ("soil_left", -2.0, 0.2, 2.4, 1.35, 2.8, 0.35, "dry_soil"),
        ("soil_back", 0.1, -1.75, 4.5, 1.1, 1.1, 0.4, "stem_dark"),
        ("soil_right", 2.2, -0.15, 1.55, 1.1, 2.4, 0.35, "dry_soil"),
    ):
        pad(rig, "dry_soil", name, (x, y, z), (w, h, d), mat)


def _leaf_end(index: int) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
    angle = math.radians(index * 45.0 + 12.0)
    dx, dz = math.cos(angle), math.sin(angle)
    start = (dx * 0.35, 1.05 + (index % 3) * 0.2, dz * 0.35)
    end = (dx * (2.4 + (index % 2) * 0.55), 5.5 + (index % 4) * 0.55, dz * (2.4 + (index % 2) * 0.55))
    return start, end


def part_fine_leaves(rig):
    rig.bone("fine_leaves", (0.0, 0.0, 0.0))
    for index in range(8):
        start, end = _leaf_end(index)
        strand(rig, "fine_leaves", f"leaf_{index}", start, end, 0.28, "leaf_gray")
        tip = (end[0] * 1.06, end[1] + 0.5, end[2] * 1.06)
        strand(rig, "fine_leaves", f"leaf_tip_{index}", end, tip, 0.22, "leaf_white")


def _branch_point(index: int, ratio: float) -> tuple[float, float, float]:
    start, end = _leaf_end(index)
    return tuple(start[axis] + (end[axis] - start[axis]) * ratio for axis in range(3))


def part_feather_leaves(rig):
    rig.bone("feather_leaves", (0.0, 0.0, 0.0))
    for index in range(8):
        start, end = _leaf_end(index)
        horizontal = math.hypot(end[0] - start[0], end[2] - start[2])
        perp_x = -(end[2] - start[2]) / horizontal
        perp_z = (end[0] - start[0]) / horizontal
        for level, ratio in enumerate((0.3, 0.48, 0.66, 0.82)):
            point = _branch_point(index, ratio)
            length = 0.52 - level * 0.07
            for side, label in ((-1.0, "l"), (1.0, "r")):
                leaf_start = (point[0], point[1] + 0.04, point[2])
                leaf_end = (
                    point[0] + perp_x * length * side,
                    point[1] + 0.18 + (level % 2) * 0.04,
                    point[2] + perp_z * length * side,
                )
                strand(
                    rig,
                    "feather_leaves",
                    f"feather_{index}_{level}_{label}",
                    leaf_start,
                    leaf_end,
                    0.14,
                    "leaf_gray" if level % 2 else "leaf_white",
                )


def part_leaf_highlights(rig):
    rig.bone("leaf_highlights", (0.0, 0.0, 0.0))
    for index in (0, 2, 4, 6):
        start, end = _leaf_end(index)
        highlight_start = (start[0], start[1] + 0.3, start[2])
        highlight_end = (end[0] * 0.92, end[1] + 0.3, end[2] * 0.92)
        strand(rig, "leaf_highlights", f"highlight_{index}", highlight_start, highlight_end, 0.2, "leaf_white")


def part_salt_crystals(rig):
    rig.bone("salt_crystals", (0.0, 0.0, 0.0))
    for index in (0, 1, 3, 5, 7):
        _start, end = _leaf_end(index)
        pad(rig, "salt_crystals", f"salt_{index}", (end[0], end[1] + 0.55, end[2]), (0.38, 0.48, 0.38), "salt_crystal")
    for name, x, z in (("salt_center", 0.0, 0.0), ("salt_left", -1.35, 0.2), ("salt_right", 1.35, -0.3)):
        pad(rig, "salt_crystals", name, (x, 2.0, z), (0.3, 0.36, 0.3), "salt_crystal")


def build():
    return build_rig(MATS, (part_dry_soil, part_fine_leaves, part_feather_leaves, part_leaf_highlights, part_salt_crystals))


GATES = PlantGates("白盐蓬 / bai_yan_peng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("BaiYanPeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
