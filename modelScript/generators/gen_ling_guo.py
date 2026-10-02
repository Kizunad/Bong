"""灵果：山间孤生的圆润灵果，碧翠果身（#407030 / #648c3c）带隐隐流光仙纹（#a0e060，正典 tint 10543200）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "mountain_stone": (45, 48, 44),       # 山间碎石座
    "bush_twig": (52, 65, 48),            # 灌木短枝
    "jade_fruit_deep": (64, 112, 48),     # #407030 深翠灵果
    "jade_fruit_mid": (100, 140, 60),     # #648c3c 翠绿果皮
    "spirit_glow_band": (130, 235, 140),  # #82eb8c 灵气流光纹
}


def part_stone_base(rig):
    rig.bone("stone_base", (0.0, 0.0, 0.0))
    # 底部山石台座 (高 2.2px，宽 6.4px)
    pad(rig, "stone_base", "stone_mound", (0.0, 0.12, 0.0), (6.4, 1.8, 6.4), "mountain_stone")
    pad(rig, "stone_base", "stone_step_l", (-1.6, 0.12, 0.5), (2.8, 2.2, 2.4), "mountain_stone")


def part_shrub_twigs(rig):
    rig.bone("shrub_twigs", (0.0, 0.0, 0.0))
    # 山间低矮灌木托枝与 4 片托叶（y: 1.6 -> 3.8，展宽 8.6px）
    strand(rig, "shrub_twigs", "twig_stalk_c", (0.0, 1.6, 0.0), (0.0, 3.2, 0.0), 0.52, "bush_twig")
    leaves = (
        ("twig_leaf_f", (0.0, 1.8, 0.6), (0.0, 2.8, 3.6), 0.44),
        ("twig_leaf_b", (0.0, 1.8, -0.6), (0.0, 2.8, -3.6), 0.44),
        ("twig_leaf_l", (-0.6, 1.8, 0.0), (-3.6, 2.8, 0.0), 0.44),
        ("twig_leaf_r", (0.6, 1.8, 0.0), (3.6, 2.8, 0.0), 0.44),
    )
    for name, start, end, r in leaves:
        strand(rig, "shrub_twigs", name, start, end, r, "bush_twig")


def part_round_spirit_fruit(rig):
    rig.bone("round_spirit_fruit", (0.0, 0.0, 0.0))
    # 孤生圆润晶莹灵果（大号浑圆球形果体，y: 2.8 -> 8.8，直径 5.6px）
    pad(rig, "round_spirit_fruit", "fruit_core_sphere", (0.0, 2.8, 0.0), (5.2, 5.0, 5.2), "jade_fruit_deep")
    pad(rig, "round_spirit_fruit", "fruit_equator_x", (0.0, 3.3, 0.0), (5.8, 4.0, 4.6), "jade_fruit_mid")
    pad(rig, "round_spirit_fruit", "fruit_equator_z", (0.0, 3.3, 0.0), (4.6, 4.0, 5.8), "jade_fruit_mid")
    pad(rig, "round_spirit_fruit", "fruit_top_crown", (0.0, 7.6, 0.0), (3.4, 1.2, 3.4), "jade_fruit_mid")


def part_glowing_bands(rig):
    rig.bone("glowing_bands", (0.0, 0.0, 0.0))
    # 灵果表皮隐隐流转的亮绿仙光环纹 (#a0e060)
    # 环状流光微凸环
    strand(rig, "glowing_bands", "glow_ring_f", (-2.2, 4.8, 2.8), (2.2, 5.4, 2.8), 0.24, "spirit_glow_band")
    strand(rig, "glowing_bands", "glow_ring_r", (2.8, 5.4, 2.2), (2.8, 5.8, -2.2), 0.24, "spirit_glow_band")
    strand(rig, "glowing_bands", "glow_ring_diag", (-1.8, 6.8, -1.8), (1.8, 7.2, 1.8), 0.22, "spirit_glow_band")
    pad(rig, "glowing_bands", "glow_top_pip", (0.0, 8.7, 0.0), (0.9, 0.9, 0.9), "spirit_glow_band")


def build():
    return build_rig(MATS, (part_stone_base, part_shrub_twigs, part_round_spirit_fruit, part_glowing_bands))


GATES = PlantGates("灵果 / ling_guo")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("LingGuo", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
