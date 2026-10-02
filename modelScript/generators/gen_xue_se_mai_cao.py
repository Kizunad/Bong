"""血色脉草：古战场暗红血土中生出的刀形立叶，贯穿凸起动脉血脉与脉节。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "blood_earth": (48, 32, 30),
    "stem_knot": (56, 44, 42),
    "blade_graygreen": (62, 70, 60),
    "blood_vein_main": (168, 38, 32),
    "blood_vein_sub": (215, 68, 55),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    # 浸血暗褐古战场土台
    pad(rig, "soil_base", "soil_mound", (0.0, 0.12, 0.0), (4.2, 0.75, 4.2), "blood_earth")
    pad(rig, "soil_base", "soil_clot_l", (-1.4, 0.12, 0.3), (1.5, 0.55, 1.8), "blood_earth")
    pad(rig, "soil_base", "soil_clot_r", (1.4, 0.12, -0.2), (1.5, 0.55, 1.8), "blood_earth")


def part_pulse_stem(rig):
    rig.bone("pulse_stem", (0.0, 0.0, 0.0))
    # 粗短脉管主茎与脉冲结节
    strand(rig, "pulse_stem", "stem_lower", (0.0, 0.45, 0.0), (0.0, 2.80, 0.0), 0.38, "stem_knot")
    pad(rig, "pulse_stem", "stem_pulse_node", (0.0, 2.60, 0.0), (1.1, 0.85, 1.1), "blood_vein_main")


def part_blade_leaves(rig):
    rig.bone("blade_leaves", (0.0, 0.0, 0.0))
    # 直挺向上展开的 3 束刀形灰绿草叶
    # 1. 主叶 (中央挺拔微向后弯, y: 3.20 -> 10.80)
    strand(rig, "blade_leaves", "blade_c_low", (0.0, 3.20, 0.0), (0.1, 6.80, -0.3), 0.42, "blade_graygreen")
    strand(rig, "blade_leaves", "blade_c_high", (0.1, 6.80, -0.3), (0.15, 10.20, -0.5), 0.32, "blade_graygreen")
    strand(rig, "blade_leaves", "blade_c_tip", (0.15, 10.20, -0.5), (0.18, 11.60, -0.55), 0.20, "blade_graygreen")

    # 2. 左侧刀叶 (向左前方展开, y: 3.00 -> 9.40)
    strand(rig, "blade_leaves", "blade_l_low", (-0.3, 3.00, 0.1), (-1.6, 6.20, 0.6), 0.36, "blade_graygreen")
    strand(rig, "blade_leaves", "blade_l_high", (-1.6, 6.20, 0.6), (-2.5, 8.80, 1.1), 0.26, "blade_graygreen")
    strand(rig, "blade_leaves", "blade_l_tip", (-2.5, 8.80, 1.1), (-3.1, 10.10, 1.4), 0.18, "blade_graygreen")

    # 3. 右侧刀叶 (向右后方展开, y: 3.00 -> 9.20)
    strand(rig, "blade_leaves", "blade_r_low", (0.3, 3.00, -0.1), (1.6, 6.10, -0.6), 0.36, "blade_graygreen")
    strand(rig, "blade_leaves", "blade_r_high", (1.6, 6.10, -0.6), (2.5, 8.60, -1.0), 0.26, "blade_graygreen")
    strand(rig, "blade_leaves", "blade_r_tip", (2.5, 8.60, -1.0), (3.0, 9.80, -1.3), 0.18, "blade_graygreen")


def part_blood_veins(rig):
    rig.bone("blood_veins", (0.0, 0.0, 0.0))
    # 贯穿叶面凸起的深红/亮红动脉血脉 (贴附在各叶脊正面上)
    # 中央主脉
    strand(rig, "blood_veins", "vein_c_main", (0.0, 3.35, 0.22), (0.1, 7.00, -0.08), 0.24, "blood_vein_main")
    strand(rig, "blood_veins", "vein_c_upper", (0.1, 7.00, -0.08), (0.15, 10.40, -0.32), 0.18, "blood_vein_sub")

    # 左侧主脉与分支
    strand(rig, "blood_veins", "vein_l_main", (-0.3, 3.15, 0.32), (-1.6, 6.35, 0.82), 0.22, "blood_vein_main")
    strand(rig, "blood_veins", "vein_l_upper", (-1.6, 6.35, 0.82), (-2.5, 8.95, 1.30), 0.16, "blood_vein_sub")

    # 右侧主脉与分支
    strand(rig, "blood_veins", "vein_r_main", (0.3, 3.15, 0.12), (1.6, 6.25, -0.42), 0.22, "blood_vein_main")
    strand(rig, "blood_veins", "vein_r_upper", (1.6, 6.25, -0.42), (2.5, 8.75, -0.80), 0.16, "blood_vein_sub")


def build():
    return build_rig(MATS, (part_soil_base, part_pulse_stem, part_blade_leaves, part_blood_veins))


GATES = PlantGates("血色脉草 / xue_se_mai_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XueSeMaiCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
