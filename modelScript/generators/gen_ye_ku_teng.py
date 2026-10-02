"""夜枯藤：幽穴里吸取真元的干枯暗藤。"""

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
    "cave_bed": (36, 34, 38),
    "dry_vine": (58, 42, 74),      # 暗紫 #3a2a4a
    "vine_high": (74, 70, 80),     # 灰 #4a4650
    "siphon_void": (102, 68, 138), # 虹吸吸元低饱和紫
    "dead_leaf": (54, 50, 56),     # 枯叶
}


def part_cave_bed(rig):
    rig.bone("cave_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bed_floor", 0.0, 0.0, 7.8, 0.75, 7.4, 0.1, "cave_bed"),
        ("bed_left", -2.6, 0.1, 2.2, 0.95, 3.4, 0.3, "cave_bed"),
        ("bed_right", 2.5, -0.2, 2.0, 0.9, 3.2, 0.3, "cave_bed"),
        ("bed_back", 0.0, -2.4, 5.2, 0.9, 1.6, 0.3, "vine_high"),
    ):
        pad(rig, "cave_bed", name, (x, y, z), (w, h, d), mat)


def part_dry_vines(rig):
    """6 根 1px 细藤绕成横躺的环/甜甜圈状卷须团（俯视呈收束漩涡），暗紫 #3a2a4a 与灰 #4a4650 交错。"""
    rig.bone("dry_vines", (0.0, 0.0, 0.0))
    for vine_index in range(6):
        phase_torus = math.tau * vine_index / 6.0
        mat = "dry_vine" if vine_index % 2 == 0 else "vine_high"
        points = []
        for step in range(14):
            t = step / 13.0
            # 沿环形大圆绕 1.3 圈
            phi = phase_torus + math.tau * 1.3 * t
            # 沿环管截面绕 2.0 圈
            theta = math.tau * 2.0 * t
            # 环形大半径 R=4.2，截面小半径 r=1.5，中心中空（半径约 2.7px）
            rad = 4.2 + 1.45 * math.cos(theta)
            x = rad * math.cos(phi)
            z = rad * math.sin(phi)
            y = 1.35 + 1.35 * (1.0 + math.sin(theta))  # y: 1.35..4.05
            points.append((x, y, z))
        curved_vine_chain(
            rig,
            "dry_vines",
            f"vine_torus_{vine_index}",
            points,
            0.44,
            0.30,
            mat,
        )


def part_siphon_tendrils(rig):
    """从环体外缘向外蔓延垂落的虹吸触须。"""
    rig.bone("siphon_tendrils", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("siphon_left", (-4.6, 2.3, 1.2), (-5.6, 1.1, 1.6)),
        ("siphon_front", (0.8, 2.2, 4.4), (1.4, 0.9, 5.5)),
        ("siphon_right", (4.5, 2.3, -1.0), (5.5, 1.0, -1.4)),
        ("siphon_back", (-1.2, 2.4, -4.3), (-1.8, 1.2, -5.3)),
    ):
        strand(rig, "siphon_tendrils", name, start, end, 0.28, "siphon_void")
    for name, x, y, z in (
        ("siphon_tip_left", -5.6, 0.95, 1.6),
        ("siphon_tip_front", 1.4, 0.75, 5.5),
        ("siphon_tip_right", 5.5, 0.85, -1.4),
        ("siphon_tip_back", -1.8, 1.05, -5.3),
    ):
        pad(rig, "siphon_tendrils", name, (x, y, z), (0.44, 0.42, 0.44), "siphon_void")


def part_dead_leaves(rig):
    """卷须团外圈稀疏散布的枯叶。"""
    rig.bone("dead_leaves", (0.0, 0.0, 0.0))
    for name, center, size in (
        ("leaf_north", (1.6, 2.8, -4.2), (1.1, 0.32, 0.65)),
        ("leaf_south", (-2.2, 2.7, 4.0), (1.0, 0.3, 0.6)),
        ("leaf_east", (4.4, 2.9, 1.8), (0.7, 0.28, 0.95)),
    ):
        pad(rig, "dead_leaves", name, center, size, "dead_leaf")


def build():
    return build_rig(MATS, (part_cave_bed, part_dry_vines, part_siphon_tendrils, part_dead_leaves))


GATES = PlantGates("夜枯藤 / ye_ku_teng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YeKuTeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
