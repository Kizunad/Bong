"""血色脉草返工版：扇形一大簇尖叶（10片以上扇形展开），一侧赤红（#c02a1a）一侧青绿（#2aa8a0），成团占满格子。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_bloody": (45, 32, 34),
    "stem_base": (55, 45, 48),
    "leaf_red": (192, 42, 26),      # #c02a1a 鲜赤红
    "leaf_cyan": (42, 168, 160),    # #2aa8a0 亮青绿
    "vein_bright": (240, 85, 65),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    pad(rig, "soil_base", "soil_center", (0.0, 0.12, 0.0), (5.2, 0.85, 4.6), "soil_bloody")
    pad(rig, "soil_base", "soil_fan_root", (0.0, 0.85, 0.0), (3.8, 1.2, 2.6), "stem_base")


def part_red_fan_leaves(rig):
    rig.bone("red_fan_leaves", (0.0, 0.0, 0.0))
    # 左侧扇形排布的 6 片长尖刀叶（赤红色 #c02a1a）
    # 扇形展开角度覆盖 -65° 到 -6° (在 X-Y 平面内扇形展开，cos=X, sin=Z)
    red_angles = [-62.0, -50.0, -38.0, -26.0, -16.0, -6.0]
    lengths = [7.2, 8.2, 9.0, 9.4, 8.8, 8.0]
    for i, (deg, l) in enumerate(zip(red_angles, lengths)):
        rad = math.radians(deg)
        # 让扇形左右展开在 X 轴上：x = sin(rad), z = cos(rad) 或反过来
        # 这里 X 应该左右分：左为 -X, 右为 +X
        # rad 为负时，sin(rad) 为负 (-X)
        sin_x = math.sin(rad)
        cos_z = math.cos(rad) * 0.35
        
        p_base = (sin_x * 0.8, 1.8, cos_z * 0.8)
        p_mid = (sin_x * (l * 0.6), 1.8 + l * 0.5, cos_z * 1.2)
        p_tip = (sin_x * l, 1.8 + l * 0.82, cos_z * 1.5)
        
        strand(rig, "red_fan_leaves", f"red_leaf_low_{i}", p_base, p_mid, 0.40, "leaf_red")
        strand(rig, "red_fan_leaves", f"red_leaf_high_{i}", p_mid, p_tip, 0.30, "leaf_red")


def part_cyan_fan_leaves(rig):
    rig.bone("cyan_fan_leaves", (0.0, 0.0, 0.0))
    # 右侧扇形排布的 6 片长尖刀叶（青绿色 #2aa8a0）
    # 扇形展开角度覆盖 +6° 到 +62°
    cyan_angles = [6.0, 16.0, 26.0, 38.0, 50.0, 62.0]
    lengths = [8.0, 8.8, 9.4, 9.0, 8.2, 7.2]
    for i, (deg, l) in enumerate(zip(cyan_angles, lengths)):
        rad = math.radians(deg)
        sin_x = math.sin(rad)
        cos_z = math.cos(rad) * 0.35
        
        p_base = (sin_x * 0.8, 1.8, cos_z * 0.8)
        p_mid = (sin_x * (l * 0.6), 1.8 + l * 0.5, cos_z * 1.2)
        p_tip = (sin_x * l, 1.8 + l * 0.82, cos_z * 1.5)
        
        strand(rig, "cyan_fan_leaves", f"cyan_leaf_low_{i}", p_base, p_mid, 0.40, "leaf_cyan")
        strand(rig, "cyan_fan_leaves", f"cyan_leaf_high_{i}", p_mid, p_tip, 0.30, "leaf_cyan")


def part_central_ridge(rig):
    rig.bone("central_ridge", (0.0, 0.0, 0.0))
    # 红青两相交汇处的直挺血色主脊脉线与中央大叶
    strand(rig, "central_ridge", "ridge_main_stalk", (0.0, 1.8, 0.0), (0.0, 10.2, 0.0), 0.35, "vein_bright")
    pad(rig, "central_ridge", "ridge_core_collar", (0.0, 2.2, 0.0), (1.6, 1.8, 1.6), "leaf_red")


def build():
    return build_rig(MATS, (part_soil_base, part_red_fan_leaves, part_cyan_fan_leaves, part_central_ridge))


GATES = PlantGates("血色脉草 / xue_se_mai_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XueSeMaiCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
