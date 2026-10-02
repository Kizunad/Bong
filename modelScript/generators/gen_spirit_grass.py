"""灵草：末法残土常见的狭长微弧灵草，深青蓝渐变到浅青灵光草尖。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_base": (45, 38, 32),
    "grass_deep": (36, 58, 62),
    "grass_mid": (48, 85, 92),
    "spirit_light": (82, 142, 155),
    "spirit_glow": (125, 205, 222),
}


def part_root_base(rig):
    rig.bone("root_base", (0.0, 0.0, 0.0))
    # 根部湿润土丘与须根
    pad(rig, "root_base", "root_soil_center", (0.0, 0.12, 0.0), (3.8, 0.75, 3.8), "soil_base")
    pad(rig, "root_base", "root_soil_f", (0.2, 0.12, 1.45), (2.2, 0.55, 1.2), "soil_base")
    pad(rig, "root_base", "root_soil_b", (-0.15, 0.12, -1.35), (2.4, 0.55, 1.3), "soil_base")
    pad(rig, "root_base", "root_soil_l", (-1.4, 0.12, 0.15), (1.1, 0.45, 2.2), "soil_base")
    pad(rig, "root_base", "root_soil_r", (1.45, 0.12, -0.1), (1.2, 0.45, 2.1), "soil_base")


def part_stem_cluster(rig):
    rig.bone("stem_cluster", (0.0, 0.0, 0.0))
    # 4 根主茎从根部汇聚向上拔起
    stems = (
        ("stem_center", (0.0, 0.45, 0.0), (0.1, 3.6, -0.2), 0.32, "grass_deep"),
        ("stem_left", (-0.5, 0.42, 0.2), (-1.4, 3.2, 0.6), 0.28, "grass_deep"),
        ("stem_right", (0.5, 0.42, -0.1), (1.5, 3.1, -0.5), 0.28, "grass_deep"),
        ("stem_front", (-0.1, 0.45, 0.4), (0.3, 2.8, 1.2), 0.26, "grass_deep"),
    )
    for name, start, end, r, mat in stems:
        strand(rig, "stem_cluster", name, start, end, r, mat)


def part_grass_blades(rig):
    rig.bone("grass_blades", (0.0, 0.0, 0.0))
    # 向上舒展的外展狭长灵草叶片 (分中段与高段展开)
    blades = (
        # 中央主叶 (高挺微后倾)
        ("blade_c_mid", (0.1, 3.6, -0.2), (0.2, 6.8, -0.45), 0.30, "grass_mid"),
        ("blade_c_high", (0.2, 6.8, -0.45), (0.15, 9.4, -0.2), 0.26, "spirit_light"),
        # 左侧展开叶 (大弧外展)
        ("blade_l_mid", (-1.4, 3.2, 0.6), (-2.6, 5.8, 1.1), 0.28, "grass_mid"),
        ("blade_l_high", (-2.6, 5.8, 1.1), (-3.2, 8.2, 1.45), 0.24, "spirit_light"),
        # 右侧展开叶
        ("blade_r_mid", (1.5, 3.1, -0.5), (2.8, 5.6, -0.9), 0.28, "grass_mid"),
        ("blade_r_high", (2.8, 5.6, -0.9), (3.4, 7.8, -1.2), 0.24, "spirit_light"),
        # 前伸次叶
        ("blade_f_mid", (0.3, 2.8, 1.2), (0.6, 5.2, 2.1), 0.26, "grass_mid"),
        ("blade_f_high", (0.6, 5.2, 2.1), (0.8, 7.2, 2.6), 0.22, "spirit_light"),
    )
    for name, start, end, r, mat in blades:
        strand(rig, "grass_blades", name, start, end, r, mat)


def part_spirit_tips(rig):
    rig.bone("spirit_tips", (0.0, 0.0, 0.0))
    # 浅青灵光尖芒
    tips = (
        ("tip_center", (0.15, 9.4, -0.2), (0.1, 10.8, 0.0), 0.20),
        ("tip_left", (-3.2, 8.2, 1.45), (-3.6, 9.5, 1.7), 0.18),
        ("tip_right", (3.4, 7.8, -1.2), (3.9, 9.1, -1.45), 0.18),
        ("tip_front", (0.8, 7.2, 2.6), (0.95, 8.3, 2.95), 0.16),
    )
    for name, start, end, r in tips:
        strand(rig, "spirit_tips", name, start, end, r, "spirit_glow")


def build():
    return build_rig(MATS, (part_root_base, part_stem_cluster, part_grass_blades, part_spirit_tips))


GATES = PlantGates("灵草 / spirit_grass")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("SpiritGrass", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
