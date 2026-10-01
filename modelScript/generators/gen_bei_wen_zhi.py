"""碑文芝：灰青灵芝伞面与紫色断纹，生在残阵石旁。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "ruin_stone": (62, 72, 72),
    "stone_high": (114, 132, 130),
    "cap_gray": (132, 151, 148),
    "purple_vein": (108, 54, 132),
    "purple_glint": (184, 105, 190),
}


def part_ruin_stone(rig):
    rig.bone("ruin_stone", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("stone_floor", 0.0, 0.0, 7.2, 0.9, 5.8, 0.1, "ruin_stone"),
        ("stone_left", -2.25, 0.25, 2.2, 1.8, 3.4, 0.45, "stone_high"),
        ("stone_back", 0.15, -1.8, 4.8, 1.5, 1.25, 0.55, "ruin_stone"),
        ("stone_right", 2.45, -0.2, 1.5, 1.3, 2.5, 0.4, "ruin_stone"),
    ):
        pad(rig, "ruin_stone", name, (x, y, z), (w, h, d), mat)


def part_stems(rig):
    rig.bone("stems", (0.0, 0.0, 0.0))
    for name, start, end, radius in (
        ("stem_main", (-0.35, 1.3, -0.1), (-0.25, 6.2, -0.15), 0.46),
        ("stem_left", (-1.8, 1.45, 0.2), (-1.95, 4.9, 0.15), 0.34),
        ("stem_right", (1.25, 1.25, -0.15), (1.65, 4.55, -0.25), 0.32),
    ):
        strand(rig, "stems", name, start, end, radius, "stone_high")


def part_caps(rig):
    rig.bone("caps", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("cap_main_lower", -0.25, -0.15, 5.2, 0.95, 2.45, 6.05, "cap_gray"),
        ("cap_main_top", -0.25, -0.15, 4.35, 0.7, 2.0, 6.85, "stone_high"),
        ("cap_left_lower", -1.95, 0.15, 3.0, 0.65, 1.75, 4.75, "cap_gray"),
        ("cap_left_top", -1.95, 0.15, 2.35, 0.5, 1.4, 5.3, "stone_high"),
        ("cap_right_lower", 1.65, -0.25, 2.55, 0.6, 1.6, 4.45, "cap_gray"),
    ):
        pad(rig, "caps", name, (x, y, z), (w, h, d), mat)


def part_inscription_veins(rig):
    rig.bone("inscription_veins", (0.0, 0.0, 0.0))
    for name, start, end, radius, mat in (
        ("inscription_main", (-2.1, 7.72, 0.45), (1.65, 7.72, -0.35), 0.2, "purple_vein"),
        ("inscription_branch_l", (-0.8, 7.74, 0.2), (-1.9, 7.75, -0.85), 0.18, "purple_vein"),
        ("inscription_branch_r", (0.35, 7.74, 0.0), (1.45, 7.75, 0.7), 0.18, "purple_glint"),
        ("inscription_left", (-2.5, 5.48, 0.15), (-1.2, 5.55, 0.15), 0.2, "purple_glint"),
    ):
        strand(rig, "inscription_veins", name, start, end, radius, mat)


def build():
    return build_rig(MATS, (part_ruin_stone, part_stems, part_caps, part_inscription_veins))


GATES = PlantGates("碑文芝 / bei_wen_zhi")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("BeiWenZhi", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
