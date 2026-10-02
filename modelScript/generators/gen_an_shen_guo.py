"""安神果返工版：短枝挂着墨绿近黑的阶梯桃形果（#1e2a22），表面一道道浅色纵纹（#c8c0a0），底部透微暖光，配 3 片深绿叶（#2a3a28）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_base": (42, 40, 48),
    "leaf_deep_green": (42, 65, 38),      # #2a3a28 深绿叶
    "peach_dark_green": (22, 38, 28),     # #1e2a22 墨绿近黑桃形果
    "light_stripes": (205, 195, 155),     # #c8c0a0 浅色纵纹
    "bottom_warm": (225, 165, 75),        # 底部透暖光
}


def part_branch_and_leaves(rig):
    rig.bone("branch_and_leaves", (0.0, 0.0, 0.0))
    # 梯田泥土座 (高 1.6px，宽 5.8px)
    pad(rig, "branch_and_leaves", "branch_soil_base", (0.0, 0.12, 0.0), (5.8, 1.4, 5.8), "soil_base")
    # 挺立支撑木短枝 (y: 1.2 -> 6.8)
    strand(rig, "branch_and_leaves", "branch_main_stalk", (-0.4, 1.2, -0.4), (0.2, 6.4, 0.2), 0.44, "leaf_deep_green")
    strand(rig, "branch_and_leaves", "branch_pedicel", (0.2, 6.4, 0.2), (0.2, 7.8, 0.8), 0.32, "leaf_deep_green")
    
    # 3 片深绿宽阔承托叶片 (展开阔达 9.0px)
    strand(rig, "branch_and_leaves", "leaf_left", (-0.4, 2.2, 0.0), (-3.8, 3.6, 1.2), 0.52, "leaf_deep_green")
    strand(rig, "branch_and_leaves", "leaf_right", (0.2, 2.8, -0.2), (3.6, 4.2, -1.6), 0.50, "leaf_deep_green")
    strand(rig, "branch_and_leaves", "leaf_back", (0.0, 2.4, -0.6), (-0.5, 3.8, -3.8), 0.48, "leaf_deep_green")


def part_stepped_peach_fruit(rig):
    rig.bone("stepped_peach_fruit", (0.0, 0.0, 0.0))
    # 挂在短枝上的墨绿近黑桃形果（阶梯逐层递变逼近桃形，y: 2.8 -> 9.6，尖顶）
    # 1. 下层底部层 (3x3 截面，y: 2.8 -> 4.2)
    pad(rig, "stepped_peach_fruit", "peach_base_layer", (0.2, 2.8, 0.8), (3.6, 1.4, 3.6), "peach_dark_green")
    
    # 2. 中层饱满桃腹层 (5x5 截面多块组合，带左右桃心瓣凹槽，y: 4.2 -> 6.8)
    pad(rig, "stepped_peach_fruit", "peach_belly_core", (0.2, 4.2, 0.8), (5.0, 2.6, 5.0), "peach_dark_green")
    pad(rig, "stepped_peach_fruit", "peach_lobe_l", (-1.1, 4.2, 0.8), (3.2, 2.6, 4.6), "peach_dark_green")
    pad(rig, "stepped_peach_fruit", "peach_lobe_r", (1.5, 4.2, 0.8), (3.2, 2.6, 4.6), "peach_dark_green")

    # 3. 上层桃肩收拢层 (3.8x3.8 截面，y: 6.8 -> 8.2)
    pad(rig, "stepped_peach_fruit", "peach_shoulder_layer", (0.2, 6.8, 0.8), (3.8, 1.4, 3.8), "peach_dark_green")

    # 4. 尖顶小喙 (收尖，y: 8.2 -> 9.6)
    pad(rig, "stepped_peach_fruit", "peach_apex_taper", (0.2, 8.2, 0.8), (2.2, 1.0, 2.2), "peach_dark_green")
    strand(rig, "stepped_peach_fruit", "peach_sharp_tip", (0.2, 8.8, 0.8), (0.2, 9.6, 0.9), 0.28, "peach_dark_green")


def part_light_stripes(rig):
    rig.bone("light_stripes", (0.0, 0.0, 0.0))
    # 桃身表面清晰立体的浅色纵纹 (#c8c0a0)
    # 前面纵纹
    strand(rig, "light_stripes", "stripe_front_main", (0.2, 3.2, 3.4), (0.2, 8.4, 2.8), 0.22, "light_stripes")
    # 左侧纵纹
    strand(rig, "light_stripes", "stripe_side_l", (-2.4, 3.6, 0.8), (-1.8, 7.8, 0.8), 0.22, "light_stripes")
    # 右侧纵纹
    strand(rig, "light_stripes", "stripe_side_r", (2.8, 3.6, 0.8), (2.2, 7.8, 0.8), 0.22, "light_stripes")
    # 背面纵纹
    strand(rig, "light_stripes", "stripe_back", (0.2, 3.2, -1.8), (0.2, 8.0, -1.2), 0.22, "light_stripes")


def part_warm_glow(rig):
    rig.bone("warm_glow", (0.0, 0.0, 0.0))
    # 桃果底部透出的一抹温润暖光 (#e1a54b)
    pad(rig, "warm_glow", "glow_bottom_pad", (0.2, 2.2, 0.8), (2.8, 0.8, 2.8), "bottom_warm")


def build():
    return build_rig(MATS, (part_branch_and_leaves, part_stepped_peach_fruit, part_light_stripes, part_warm_glow))


GATES = PlantGates("安神果 / an_shen_guo")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("AnShenGuo", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
