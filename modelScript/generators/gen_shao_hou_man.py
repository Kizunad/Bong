"""烧喉蔓：洞壁上的发光垂挂藤蔓。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import (  # noqa: E402
    PlantGates,
    build_rig,
    curved_vine_chain,
    pad,
    strand,
    write_model,
)

MATS = {
    "cave_wall": (38, 35, 34),
    "wall_high": (67, 57, 52),
    "throat_vine": (48, 27, 27),
    "ember_orange": (235, 82, 28),
    "glow_hot": (255, 146, 45),
    "burn_leaf": (105, 38, 35),
}


def part_cave_wall(rig):
    rig.bone("cave_wall", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("wall_anchor", 0.0, -0.45, 7.0, 0.85, 1.6, 6.35, "wall_high"),
        ("wall_back", 0.0, -1.8, 5.8, 1.1, 0.9, 0.35, "cave_wall"),
        ("wall_left", -3.0, -0.95, 1.0, 4.4, 1.0, 0.35, "wall_high"),
        ("wall_right", 3.0, -1.0, 1.0, 4.4, 1.0, 0.35, "cave_wall"),
        ("wall_floor", 0.0, 0.0, 7.2, 0.8, 5.6, 0.1, "cave_wall"),
    ):
        pad(rig, "cave_wall", name, (x, y, z), (w, h, d), mat)


def _hanging_path(vine_index: int, count: int = 10):
    anchors = (-2.55, -1.55, -0.55, 0.45, 1.45, 2.45)
    anchor_x = anchors[vine_index]
    points = []
    for step in range(count):
        t = step / (count - 1)
        points.append(
            (
                anchor_x + 0.42 * math.sin(math.pi * t + vine_index * 0.55),
                6.55 - (5.55 - 0.35 * math.sin(math.pi * t)) * t,
                0.15 * math.cos(math.pi * t + vine_index),
            )
        )
    return points


def part_hanging_vines(rig):
    rig.bone("hanging_vines", (0.0, 0.0, 0.0))
    for vine_index in range(6):
        curved_vine_chain(
            rig,
            "hanging_vines",
            f"vine_hang_{vine_index}",
            _hanging_path(vine_index),
            0.52,
            0.28,
            "throat_vine",
        )


def part_ember_segments(rig):
    rig.bone("ember_segments", (0.0, 0.0, 0.0))
    for vine_index in range(6):
        points = _hanging_path(vine_index)
        for leaf_index in (2, 5, 8):
            x, y, z = points[leaf_index]
            pad(
                rig,
                "ember_segments",
                f"ember_leaf_{vine_index}_{leaf_index}",
                (x + 0.18, y, z + 0.06),
                (0.62, 0.46, 0.28),
                "glow_hot" if (vine_index + leaf_index) % 2 else "ember_orange",
            )


def part_burn_leaves(rig):
    rig.bone("burn_leaves", (0.0, 0.0, 0.0))
    for vine_index in range(6):
        x, y, z = _hanging_path(vine_index)[-1]
        pad(
            rig,
            "burn_leaves",
            f"leaf_tail_{vine_index}",
            (x, y + 0.18, z + 0.08),
            (0.78, 0.32, 0.42),
            "burn_leaf",
        )


def build():
    return build_rig(MATS, (part_cave_wall, part_hanging_vines, part_ember_segments, part_burn_leaves))


GATES = PlantGates("烧喉蔓 / shao_hou_man")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ShaoHouMan", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
