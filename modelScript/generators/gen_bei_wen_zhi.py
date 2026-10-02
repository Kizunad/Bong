"""碑文芝：灰青灵芝伞面与紫色断纹，生在残阵石旁。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "ruin_stone": (62, 72, 72),
    "stone_high": (114, 132, 130),
    "cap_gray": (90, 96, 112),
    "cap_shadow": (66, 72, 86),
    "purple_vein": (138, 90, 200),
    "purple_glint": (198, 140, 232),
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
        ("stem_main", (-0.15, 1.35, -0.1), (-0.05, 4.55, -0.1), 0.58),
        ("stem_left", (-1.1, 1.45, 0.2), (-1.05, 4.35, 0.15), 0.42),
        ("stem_right", (1.0, 1.4, -0.15), (1.05, 4.25, -0.2), 0.4),
    ):
        strand(rig, "stems", name, start, end, radius, "stone_high")


def part_caps(rig):
    rig.bone("caps", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("cap_cloud_lower", 0.0, -0.1, 6.5, 0.7, 3.7, 4.35, "cap_shadow"),
        ("cap_cloud", 0.0, -0.1, 6.2, 0.65, 3.4, 4.95, "cap_gray"),
        ("cap_cloud_top", 0.0, -0.1, 5.4, 0.45, 2.9, 5.6, "stone_high"),
        ("cap_left_drop", -2.65, 0.0, 1.5, 0.45, 2.8, 4.55, "cap_gray"),
        ("cap_right_drop", 2.65, 0.0, 1.5, 0.45, 2.8, 4.55, "cap_gray"),
    ):
        pad(rig, "caps", name, (x, y, z), (w, h, d), mat)


def part_inscription_veins(rig):
    rig.bone("inscription_veins", (0.0, 0.0, 0.0))
    for name, start, end, radius, mat in (
        ("inscription_main", (-2.5, 6.08, 0.95), (2.5, 6.08, 0.95), 0.2, "purple_vein"),
        ("inscription_mid", (-2.35, 6.1, -0.55), (2.25, 6.1, -0.55), 0.18, "purple_glint"),
        ("inscription_branch_l", (-1.65, 6.1, 0.95), (-2.2, 6.1, -0.45), 0.18, "purple_vein"),
        ("inscription_branch_r", (-0.25, 6.1, -0.55), (0.65, 6.1, 0.95), 0.18, "purple_glint"),
        ("inscription_front_l", (-2.7, 4.72, 1.82), (-1.0, 4.92, 1.82), 0.2, "purple_glint"),
        ("inscription_front_r", (0.35, 4.92, 1.82), (2.6, 4.72, 1.82), 0.18, "purple_vein"),
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
