"""灵果返工版：一根短枝配两片绿叶（#345f2d），挂着一个圆润绿果（#6ab04a），表面一圈淡青流光（#aaf0d0）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_stone": (45, 48, 44),           # 山间碎石座
    "wood_branch": (65, 52, 42),          # 木质短枝
    "mountain_leaf": (52, 95, 45),        # 两片翠叶
    "fruit_green": (106, 176, 74),        # #6ab04a 圆润绿果
    "cyan_streamer": (170, 240, 208),     # #aaf0d0 淡青流光
}


def part_branch_and_leaves(rig):
    rig.bone("branch_and_leaves", (0.0, 0.0, 0.0))
    # 山间碎石底座 (高 1.6px，宽 5.8px)
    pad(rig, "branch_and_leaves", "branch_stone_base", (0.0, 0.12, 0.0), (5.8, 1.4, 5.8), "soil_stone")
    # 木质短枝 (y: 1.2 -> 6.8)
    strand(rig, "branch_and_leaves", "branch_stalk_main", (-0.4, 1.2, -0.4), (0.2, 6.2, 0.2), 0.44, "wood_branch")
    strand(rig, "branch_and_leaves", "branch_pedicel", (0.2, 6.2, 0.2), (0.2, 7.8, 0.8), 0.32, "wood_branch")

    # 左右两片舒展的翠绿阔叶 (向外大弧展出，展宽 8.6px)
    strand(rig, "branch_and_leaves", "leaf_left", (-0.4, 2.4, -0.2), (-3.8, 3.8, 0.8), 0.52, "mountain_leaf")
    strand(rig, "branch_and_leaves", "leaf_right", (0.2, 3.2, 0.0), (3.6, 4.4, -1.2), 0.50, "mountain_leaf")


def part_stepped_spirit_fruit(rig):
    rig.bone("stepped_spirit_fruit", (0.0, 0.0, 0.0))
    # 挂在短枝上的圆润饱满绿果（阶梯收缩逼近球形，y: 2.8 -> 9.0，直径 5.4px）
    # 1. 下层底部层 (3.6x3.6 截面，y: 2.8 -> 4.2)
    pad(rig, "stepped_spirit_fruit", "fruit_base_layer", (0.2, 2.8, 0.8), (3.6, 1.4, 3.6), "fruit_green")

    # 2. 中层赤道膨大层 (5.2x5.2 截面多块组合，y: 4.2 -> 6.8)
    pad(rig, "stepped_spirit_fruit", "fruit_equator_core", (0.2, 4.2, 0.8), (5.0, 2.6, 5.0), "fruit_green")
    pad(rig, "stepped_spirit_fruit", "fruit_equator_x", (0.2, 4.6, 0.8), (5.6, 1.8, 4.2), "fruit_green")
    pad(rig, "stepped_spirit_fruit", "fruit_equator_z", (0.2, 4.6, 0.8), (4.2, 1.8, 5.6), "fruit_green")

    # 3. 上层穹顶收敛层 (3.8x3.8 截面，y: 6.8 -> 8.2)
    pad(rig, "stepped_spirit_fruit", "fruit_dome_layer", (0.2, 6.8, 0.8), (3.8, 1.4, 3.8), "fruit_green")

    # 4. 顶端圆球冠顶 (2.2x2.2 截面，y: 8.2 -> 9.0)
    pad(rig, "stepped_spirit_fruit", "fruit_top_cap", (0.2, 8.2, 0.8), (2.2, 0.8, 2.2), "fruit_green")


def part_cyan_glowing_ring(rig):
    rig.bone("cyan_glowing_ring", (0.0, 0.0, 0.0))
    # 灵果表面环绕的一圈淡青色流光仙纹 (#aaf0d0)
    # 前面斜向流光环
    strand(rig, "cyan_glowing_ring", "glow_band_front", (-1.8, 4.6, 3.45), (2.2, 6.2, 3.45), 0.24, "cyan_streamer")
    # 右侧斜向流光环
    strand(rig, "cyan_glowing_ring", "glow_band_right", (3.05, 5.4, 1.8), (3.05, 7.0, -1.4), 0.24, "cyan_streamer")
    # 顶端流光珠
    pad(rig, "cyan_glowing_ring", "glow_top_pip", (0.2, 8.7, 0.8), (1.1, 0.6, 1.1), "cyan_streamer")


def build():
    return build_rig(MATS, (part_branch_and_leaves, part_stepped_spirit_fruit, part_cyan_glowing_ring))


GATES = PlantGates("灵果 / ling_guo")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("LingGuo", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
