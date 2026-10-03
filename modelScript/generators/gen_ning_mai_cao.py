"""凝脉草返工版：四片宽菱形蓝叶十字展开（#2a6aa8 / #5aa8e0），中脉一道亮绿（#6ae04a）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_mound": (45, 38, 32),
    "blue_leaf_dark": (42, 106, 168),   # #2a6aa8
    "blue_leaf_light": (90, 168, 224),  # #5aa8e0
    "green_midvein": (106, 224, 74),    # #6ae04a 亮绿中脉
    "center_bud": (140, 240, 100),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    pad(rig, "soil_base", "soil_center", (0.0, 0.12, 0.0), (4.5, 0.85, 4.5), "soil_mound")
    pad(rig, "soil_base", "soil_stem_base", (0.0, 0.75, 0.0), (2.8, 1.2, 2.8), "blue_leaf_dark")


def part_rhombus_leaves(rig):
    rig.bone("rhombus_leaves", (0.0, 0.0, 0.0))
    # 四片宽大宽菱形叶十字对称向四方舒展（E, W, S, N）
    # 每片菱形叶由基部狭窄 -> 中部菱角大幅展宽 -> 尖端平滑收窄构成
    # 宽度达 4.2px，长度达 5.2px，整体铺开达 11.5px 阔度，高度 7.5px
    
    # 1. 东向叶 (+X)
    pad(rig, "rhombus_leaves", "leaf_e_neck", (1.2, 1.8, 0.0), (1.6, 0.55, 2.6), "blue_leaf_dark", rotation=(0.0, 0.0, -12.0))
    pad(rig, "rhombus_leaves", "leaf_e_rhombus_body", (2.8, 2.6, 0.0), (2.4, 0.50, 4.6), "blue_leaf_light", rotation=(0.0, 0.0, -8.0))
    pad(rig, "rhombus_leaves", "leaf_e_rhombus_wing_f", (2.8, 2.7, 1.6), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(0.0, 24.0, -8.0))
    pad(rig, "rhombus_leaves", "leaf_e_rhombus_wing_b", (2.8, 2.7, -1.6), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(0.0, -24.0, -8.0))
    pad(rig, "rhombus_leaves", "leaf_e_tip", (4.4, 3.1, 0.0), (1.8, 0.42, 2.0), "blue_leaf_light", rotation=(0.0, 0.0, -4.0))

    # 2. 西向叶 (-X)
    pad(rig, "rhombus_leaves", "leaf_w_neck", (-1.2, 1.8, 0.0), (1.6, 0.55, 2.6), "blue_leaf_dark", rotation=(0.0, 0.0, 12.0))
    pad(rig, "rhombus_leaves", "leaf_w_rhombus_body", (-2.8, 2.6, 0.0), (2.4, 0.50, 4.6), "blue_leaf_light", rotation=(0.0, 0.0, 8.0))
    pad(rig, "rhombus_leaves", "leaf_w_rhombus_wing_f", (-2.8, 2.7, 1.6), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(0.0, -24.0, 8.0))
    pad(rig, "rhombus_leaves", "leaf_w_rhombus_wing_b", (-2.8, 2.7, -1.6), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(0.0, 24.0, 8.0))
    pad(rig, "rhombus_leaves", "leaf_w_tip", (-4.4, 3.1, 0.0), (1.8, 0.42, 2.0), "blue_leaf_light", rotation=(0.0, 0.0, 4.0))

    # 3. 南向叶 (+Z)
    pad(rig, "rhombus_leaves", "leaf_s_neck", (0.0, 1.8, 1.2), (2.6, 0.55, 1.6), "blue_leaf_dark", rotation=(12.0, 0.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_s_rhombus_body", (0.0, 2.6, 2.8), (4.6, 0.50, 2.4), "blue_leaf_light", rotation=(8.0, 0.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_s_rhombus_wing_l", (-1.6, 2.7, 2.8), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(8.0, -24.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_s_rhombus_wing_r", (1.6, 2.7, 2.8), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(8.0, 24.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_s_tip", (0.0, 3.1, 4.4), (2.0, 0.42, 1.8), "blue_leaf_light", rotation=(4.0, 0.0, 0.0))

    # 4. 北向叶 (-Z)
    pad(rig, "rhombus_leaves", "leaf_n_neck", (0.0, 1.8, -1.2), (2.6, 0.55, 1.6), "blue_leaf_dark", rotation=(-12.0, 0.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_n_rhombus_body", (0.0, 2.6, -2.8), (4.6, 0.50, 2.4), "blue_leaf_light", rotation=(-8.0, 0.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_n_rhombus_wing_l", (-1.6, 2.7, -2.8), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(-8.0, 24.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_n_rhombus_wing_r", (1.6, 2.7, -2.8), (1.8, 0.42, 1.8), "blue_leaf_dark", rotation=(-8.0, -24.0, 0.0))
    pad(rig, "rhombus_leaves", "leaf_n_tip", (0.0, 3.1, -4.4), (2.0, 0.42, 1.8), "blue_leaf_light", rotation=(-4.0, 0.0, 0.0))

    # 中央收拢的聚气心芽 (直挺至 6.8px)
    pad(rig, "rhombus_leaves", "leaf_core_bud", (0.0, 2.2, 0.0), (1.8, 4.5, 1.8), "blue_leaf_light")


def part_bright_veins(rig):
    rig.bone("bright_veins", (0.0, 0.0, 0.0))
    # 四片宽菱形叶中脉各一道醒目凸起的亮绿中脉 (#6ae04a)
    strand(rig, "bright_veins", "vein_e", (0.4, 2.4, 0.0), (5.1, 3.3, 0.0), 0.28, "green_midvein")
    strand(rig, "bright_veins", "vein_w", (-0.4, 2.4, 0.0), (-5.1, 3.3, 0.0), 0.28, "green_midvein")
    strand(rig, "bright_veins", "vein_s", (0.0, 2.4, 0.4), (0.0, 3.3, 5.1), 0.28, "green_midvein")
    strand(rig, "bright_veins", "vein_n", (0.0, 2.4, -0.4), (0.0, 3.3, -5.1), 0.28, "green_midvein")
    # 顶心汇聚点
    pad(rig, "bright_veins", "vein_center_eye", (0.0, 6.2, 0.0), (0.85, 0.85, 0.85), "center_bud")


def build():
    return build_rig(MATS, (part_soil_base, part_rhombus_leaves, part_bright_veins))


GATES = PlantGates("凝脉草 / ning_mai_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("NingMaiCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
