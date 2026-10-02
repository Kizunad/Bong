"""负元蕨：北荒负灵域暗紫蕨叶，中央螺旋蜷曲拳卷幼叶，舒展羽叶带倒吸紫脉。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "negative_qi_soil": (38, 28, 45),
    "fiddlehead_stalk": (52, 38, 65),
    "fern_frond_deep": (65, 48, 85),
    "negative_vein": (125, 75, 175),
    "toxic_spores": (175, 115, 235),
}


def part_rhizome_base(rig):
    rig.bone("rhizome_base", (0.0, 0.0, 0.0))
    # 负灵域侵蚀暗紫碎石土台
    pad(rig, "rhizome_base", "rhizome_center", (0.0, 0.12, 0.0), (4.2, 0.75, 4.2), "negative_qi_soil")
    pad(rig, "rhizome_base", "rhizome_scale_l", (-1.4, 0.12, 0.2), (1.4, 0.55, 1.8), "negative_qi_soil")
    pad(rig, "rhizome_base", "rhizome_scale_r", (1.4, 0.12, -0.2), (1.4, 0.55, 1.8), "negative_qi_soil")


def part_curled_fiddlehead(rig):
    rig.bone("curled_fiddlehead", (0.0, 0.0, 0.0))
    # 典型蕨类中央螺旋拳卷幼芽 (crozier / fiddlehead, y: 0.50 -> 6.80)
    strand(rig, "curled_fiddlehead", "fiddlehead_stalk_low", (0.0, 0.50, 0.0), (0.05, 3.80, 0.1), 0.38, "fiddlehead_stalk")
    strand(rig, "curled_fiddlehead", "fiddlehead_stalk_mid", (0.05, 3.80, 0.1), (0.1, 5.80, 0.25), 0.32, "fiddlehead_stalk")
    # 向内卷回的拳头
    pad(rig, "curled_fiddlehead", "fiddlehead_coil_back", (0.1, 6.20, -0.2), (0.75, 0.75, 0.6), "negative_vein")
    pad(rig, "curled_fiddlehead", "fiddlehead_coil_tip", (0.1, 5.65, -0.45), (0.55, 0.55, 0.5), "toxic_spores")


def part_spread_fronds(rig):
    rig.bone("spread_fronds", (0.0, 0.0, 0.0))
    # 左右与后方大幅舒展弯拱的羽状大蕨叶 (fronds)
    # 1. 左羽叶 (大弧弓弯外展)
    strand(rig, "spread_fronds", "frond_l_mid", (-0.3, 1.80, 0.1), (-1.9, 4.20, 0.5), 0.34, "fern_frond_deep")
    strand(rig, "spread_fronds", "frond_l_high", (-1.9, 4.20, 0.5), (-3.2, 5.80, 0.8), 0.26, "fern_frond_deep")
    strand(rig, "spread_fronds", "frond_l_arch", (-3.2, 5.80, 0.8), (-4.2, 5.20, 1.0), 0.18, "fern_frond_deep")

    # 2. 右羽叶
    strand(rig, "spread_fronds", "frond_r_mid", (0.3, 1.80, -0.1), (1.9, 4.10, -0.5), 0.34, "fern_frond_deep")
    strand(rig, "spread_fronds", "frond_r_high", (1.9, 4.10, -0.5), (3.2, 5.70, -0.8), 0.26, "fern_frond_deep")
    strand(rig, "spread_fronds", "frond_r_arch", (3.2, 5.70, -0.8), (4.1, 5.10, -1.0), 0.18, "fern_frond_deep")

    # 3. 后向羽叶
    strand(rig, "spread_fronds", "frond_b_mid", (0.0, 1.60, -0.3), (0.0, 3.80, -1.9), 0.32, "fern_frond_deep")
    strand(rig, "spread_fronds", "frond_b_high", (0.0, 3.80, -1.9), (0.0, 5.20, -3.1), 0.24, "fern_frond_deep")
    strand(rig, "spread_fronds", "frond_b_arch", (0.0, 5.20, -3.1), (0.0, 4.60, -3.9), 0.16, "fern_frond_deep")


def part_negative_veins(rig):
    rig.bone("negative_veins", (0.0, 0.0, 0.0))
    # 倒吸天地灵气的荧紫脉线 (贴附在各蕨叶上表面)
    strand(rig, "negative_veins", "vein_l_main", (-0.3, 2.00, 0.15), (-1.9, 4.35, 0.55), 0.18, "negative_vein")
    strand(rig, "negative_veins", "vein_l_tip", (-1.9, 4.35, 0.55), (-3.2, 5.95, 0.85), 0.14, "negative_vein")

    strand(rig, "negative_veins", "vein_r_main", (0.3, 2.00, -0.05), (1.9, 4.25, -0.45), 0.18, "negative_vein")
    strand(rig, "negative_veins", "vein_r_tip", (1.9, 4.25, -0.45), (3.2, 5.85, -0.75), 0.14, "negative_vein")

    strand(rig, "negative_veins", "vein_b_main", (0.0, 1.80, -0.3), (0.0, 3.95, -1.9), 0.16, "negative_vein")
    strand(rig, "negative_veins", "vein_b_tip", (0.0, 3.95, -1.9), (0.0, 5.35, -3.1), 0.12, "negative_vein")


def build():
    return build_rig(MATS, (part_rhizome_base, part_curled_fiddlehead, part_spread_fronds, part_negative_veins))


GATES = PlantGates("负元蕨 / fu_yuan_jue")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("FuYuanJue", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
