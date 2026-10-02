"""解蛊蕊：幽暗地穴湿处所生的紫色小花，花瓣深紫聚成钟杯状，中央探出解毒亮金花蕊。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "wet_soil": (30, 26, 36),
    "stem_dark_violet": (58, 48, 85),
    "petal_violet": (115, 82, 168),       # #7352a8 幽暗紫花瓣
    "pistil_yellow_glow": (225, 210, 110), # 核心解毒金黄花蕊
    "pistil_spark": (250, 245, 180),
}


def part_wet_ground(rig):
    rig.bone("wet_ground", (0.0, 0.0, 0.0))
    pad(rig, "wet_ground", "ground_mound", (0.0, 0.12, 0.0), (5.8, 0.85, 5.8), "wet_soil")
    pad(rig, "wet_ground", "ground_lip_f", (0.0, 0.15, 2.8), (3.2, 0.55, 1.6), "wet_soil")
    # 暗紫粗壮短花茎
    strand(rig, "wet_ground", "ground_stem", (0.0, 0.85, 0.0), (0.0, 3.60, 0.0), 0.52, "stem_dark_violet")


def part_violet_bell_petals(rig):
    rig.bone("violet_bell_petals", (0.0, 0.0, 0.0))
    # 5 片厚实幽紫宽瓣，聚合成向上微敞开的钟杯花冠（bell/cup，高 7.5px，阔 9.8px）
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.75
        ortho_z = cos_a * 0.75
        
        # 杯底向上逐渐敞开
        p_base = (cos_a * 0.9, 3.2, sin_a * 0.9)
        p_mid = (cos_a * 2.8, 5.5, sin_a * 2.8)
        p_rim = (cos_a * 4.6, 7.2, sin_a * 4.6)
        p_lip = (cos_a * 5.1, 6.8, sin_a * 5.1)
        
        # 宽厚花瓣面
        strand(rig, "violet_bell_petals", f"petal_cup_l_{idx}",
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8), 0.55, "petal_violet")
        strand(rig, "violet_bell_petals", f"petal_cup_r_{idx}",
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8), 0.55, "petal_violet")

        # 花瓣外翻上缘
        strand(rig, "violet_bell_petals", f"petal_rim_l_{idx}",
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8),
               (p_rim[0] - ortho_x * 0.4, p_rim[1], p_rim[2] - ortho_z * 0.4), 0.46, "petal_violet")
        strand(rig, "violet_bell_petals", f"petal_rim_r_{idx}",
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8),
               (p_rim[0] + ortho_x * 0.4, p_rim[1], p_rim[2] + ortho_z * 0.4), 0.46, "petal_violet")

        strand(rig, "violet_bell_petals", f"petal_lip_{idx}",
               (p_rim[0], p_rim[1], p_rim[2]), (p_lip[0], p_lip[1], p_lip[2]), 0.38, "petal_violet")


def part_antidote_stamen(rig):
    rig.bone("antidote_stamen", (0.0, 0.0, 0.0))
    # 花杯中央耸立的高亮金黄解毒花蕊（药性所在，y: 3.5 -> 8.8）
    pad(rig, "antidote_stamen", "stamen_receptacle", (0.0, 3.6, 0.0), (2.2, 1.8, 2.2), "stem_dark_violet")
    # 中央主金蕊
    strand(rig, "antidote_stamen", "stamen_gold_pillar", (0.0, 4.8, 0.0), (0.0, 8.4, 0.0), 0.38, "pistil_yellow_glow")
    pad(rig, "antidote_stamen", "stamen_gold_knob", (0.0, 8.5, 0.0), (1.1, 1.1, 1.1), "pistil_spark")
    
    # 环绕三支副金蕊头
    pad(rig, "antidote_stamen", "stamen_side_a", (0.75, 7.8, 0.0), (0.6, 0.8, 0.6), "pistil_yellow_glow")
    pad(rig, "antidote_stamen", "stamen_side_b", (-0.4, 7.8, 0.65), (0.6, 0.8, 0.6), "pistil_yellow_glow")
    pad(rig, "antidote_stamen", "stamen_side_c", (-0.4, 7.8, -0.65), (0.6, 0.8, 0.6), "pistil_yellow_glow")


def build():
    return build_rig(MATS, (part_wet_ground, part_violet_bell_petals, part_antidote_stamen))


GATES = PlantGates("解蛊蕊 / jie_gu_rui")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("JieGuRui", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
