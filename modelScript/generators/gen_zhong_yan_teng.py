"""终焉藤：毒蛊师的焦黑终极藤材。"""

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
    "ash_bed": (48, 42, 40),
    "charcoal_vine": (27, 24, 25),
    "bark_rust": (92, 45, 38),
    "toxin_red": (160, 36, 42),
    "ember_line": (224, 88, 42),   # 橙色余烬 #e0582a
    "bone_thorn": (106, 26, 16),   # 暗红毒刺 #6a1a10
}


def part_ash_bed(rig):
    rig.bone("ash_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bed_floor", 0.0, 0.0, 7.8, 0.85, 6.8, 0.1, "ash_bed"),
        ("bed_left", -2.6, 0.1, 2.2, 1.25, 3.2, 0.35, "ash_bed"),
        ("bed_right", 2.5, -0.2, 2.1, 1.1, 2.8, 0.35, "charcoal_vine"),
        ("bed_back", 0.0, -2.1, 5.0, 1.15, 1.4, 0.35, "bark_rust"),
    ):
        pad(rig, "ash_bed", name, (x, y, z), (w, h, d), mat)


def _tower_path(vine_index: int, count: int = 12):
    phase = math.tau * vine_index / 6.0
    points = []
    for step in range(count):
        t = step / (count - 1)
        angle = phase + math.tau * 1.35 * t
        # 稀疏尖塔状：底部半径 5.2px，顶部收敛至 1.1px，总高达到 11.8px
        radius = 1.1 + 4.1 * (1.0 - t) ** 0.85
        points.append(
            (
                radius * math.cos(angle),
                0.8 + 10.8 * t,
                radius * math.sin(angle),
            )
        )
    return points


def part_terminal_vines(rig):
    """6 根 1px 细黑藤螺旋上升成高约 12px 的尖塔状藤团，藤与藤之间留出 1~2px 空隙。"""
    rig.bone("terminal_vines", (0.0, 0.0, 0.0))
    for vine_index in range(6):
        curved_vine_chain(
            rig,
            "terminal_vines",
            f"vine_tower_{vine_index}",
            _tower_path(vine_index),
            0.45,
            0.28,
            "charcoal_vine",
        )


def part_toxin_veins(rig):
    """藤身缝隙间穿插的暗红毒脉与点缀的橙色余烬。"""
    rig.bone("toxin_veins", (0.0, 0.0, 0.0))
    for name, start, end, mat in (
        ("vein_center", (-0.9, 3.2, 0.5), (0.1, 5.5, 0.4), "toxin_red"),
        ("vein_left", (-2.4, 3.8, 1.1), (-1.4, 6.8, 1.0), "ember_line"),
        ("vein_right", (2.1, 4.0, 0.2), (2.2, 7.2, 0.3), "toxin_red"),
        ("vein_reach", (0.2, 6.2, 0.4), (0.9, 9.4, 0.5), "ember_line"),
    ):
        strand(rig, "toxin_veins", name, start, end, 0.28, mat)
    # 缝隙间点缀的橙色余烬火点
    for name, center in (
        ("ember_speck_1", (0.4, 2.5, -0.6)),
        ("ember_speck_2", (-1.2, 5.2, -0.8)),
        ("ember_speck_3", (1.1, 7.6, 0.2)),
        ("ember_speck_4", (-0.3, 9.2, 0.6)),
    ):
        pad(rig, "toxin_veins", name, center, (0.55, 0.55, 0.55), "ember_line")


def part_poison_thorns(rig):
    """沿黑藤向外斜挑的暗红尖刺 #6a1a10。"""
    rig.bone("poison_thorns", (0.0, 0.0, 0.0))
    thorn_index = 0
    for vine_index in range(6):
        points = _tower_path(vine_index)
        for point_index in (3, 7):
            x, y, z = points[point_index]
            radial = math.hypot(x, z) or 1.0
            dx, dz = x / radial * 0.85, z / radial * 0.85
            strand(
                rig,
                "poison_thorns",
                f"thorn_{thorn_index}",
                (x, y, z),
                (x + dx, y + 0.45, z + dz),
                0.22,
                "bone_thorn",
            )
            pad(
                rig,
                "poison_thorns",
                f"thorn_tip_{thorn_index}",
                (x + dx * 1.15, y + 0.55, z + dz * 1.15),
                (0.35, 0.35, 0.35),
                "bone_thorn",
            )
            thorn_index += 1


def build():
    return build_rig(
        MATS,
        (part_ash_bed, part_terminal_vines, part_toxin_veins, part_poison_thorns),
    )


GATES = PlantGates("终焉藤 / zhong_yan_teng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ZhongYanTeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
