"""焦脉藤：焦黑藤蔓与未熄的橙红炭线。"""

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
    "scorch_bed": (52, 45, 40),
    "char_black": (30, 27, 26),
    "char_high": (68, 57, 50),
    "ember_red": (184, 53, 25),
    "ember_hot": (255, 106, 26),  # #ff6a1a
    "ash_leaf": (78, 67, 58),
}


def part_scorched_bed(rig):
    rig.bone("scorched_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("bed_floor", 0.0, 0.0, 7.6, 0.8, 6.4, 0.1, "scorch_bed"),
        ("bed_left", -2.4, 0.2, 2.4, 1.25, 3.4, 0.35, "char_black"),
        ("bed_right", 2.4, -0.2, 2.2, 1.05, 3.0, 0.35, "scorch_bed"),
        ("bed_back", 0.0, -2.0, 5.0, 1.1, 1.4, 0.35, "char_high"),
    ):
        pad(rig, "scorched_bed", name, (x, y, z), (w, h, d), mat)


def part_charred_vines(rig):
    """5 根 1px 细黑藤螺旋缠绕上升成纺锤形团，藤与藤之间留出充分空隙，展现螺旋交错剪影。"""
    rig.bone("charred_vines", (0.0, 0.0, 0.0))
    for vine_index in range(5):
        phase = math.tau * vine_index / 5.0
        points = []
        for step in range(12):
            t = step / 11.0
            angle = phase + math.tau * 1.35 * t
            # 纺锤形螺旋曲线：中段最大半径 ~5.8px，外缘逼近 6.3px
            radius = 3.2 + 2.6 * math.sin(math.pi * t)
            points.append(
                (
                    radius * math.cos(angle),
                    0.8 + 9.2 * t,
                    radius * math.sin(angle),
                )
            )
        curved_vine_chain(
            rig,
            "charred_vines",
            f"vine_spiral_{vine_index}",
            points,
            0.45,
            0.32,
            "char_black",
        )


def part_ember_core(rig):
    """中心留一根竖直的橙红发光芯 #ff6a1a (2px 粗)，透过藤蔓空隙清晰可见。"""
    rig.bone("ember_core", (0.0, 0.0, 0.0))
    # 竖直 2px 粗 (半径 1.0) 橙红发光炭线芯
    strand(
        rig,
        "ember_core",
        "ember_vertical_core",
        (0.0, 0.6, 0.0),
        (0.0, 9.8, 0.0),
        1.0,
        "ember_hot",
    )
    # 两道斜向透出的余烬暗红火脉
    for name, start, end, mat in (
        ("ember_branch_1", (0.0, 3.2, 0.0), (1.8, 4.4, 1.2), "ember_red"),
        ("ember_branch_2", (0.0, 6.2, 0.0), (-1.6, 7.3, -1.0), "ember_red"),
    ):
        strand(rig, "ember_core", name, start, end, 0.3, mat)


def part_ash_leaves(rig):
    """外缘中高位稀疏点缀少量灰叶。"""
    rig.bone("ash_leaves", (0.0, 0.0, 0.0))
    for name, center, size in (
        ("leaf_left", (-2.6, 6.8, 1.4), (1.1, 0.35, 0.7)),
        ("leaf_mid", (1.2, 7.8, -2.1), (0.9, 0.32, 0.6)),
        ("leaf_right", (2.8, 8.6, 0.8), (0.9, 0.3, 0.55)),
    ):
        pad(rig, "ash_leaves", name, center, size, "ash_leaf")


def build():
    return build_rig(MATS, (part_scorched_bed, part_charred_vines, part_ember_core, part_ash_leaves))


GATES = PlantGates("焦脉藤 / jiao_mai_teng")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("JiaoMaiTeng", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
