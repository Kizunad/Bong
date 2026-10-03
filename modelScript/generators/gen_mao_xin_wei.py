"""茅心薇返工版：金黄干草编成的环形草窝底座（#b08a3a / #7a5a2a），中间点缀小巧温暖的黄色小花（#eeb22d）。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "thatch_soil": (52, 42, 34),
    "straw_nest_gold": (176, 138, 58),   # #b08a3a 金黄干草
    "straw_nest_dark": (122, 90, 42),    # #7a5a2a 暗金干草
    "petal_warm_yellow": (238, 178, 45), # #eeb22d 暖黄花瓣
    "golden_core": (252, 222, 110),      # 亮黄心蕊
}


def part_thatch_nest(rig):
    rig.bone("thatch_nest", (0.0, 0.0, 0.0))
    # 底部微泥台
    pad(rig, "thatch_nest", "nest_soil_base", (0.0, 0.12, 0.0), (5.2, 0.65, 5.2), "thatch_soil")
    # 环形草窝底座（外径 9.2px，内凹窝心 4.0px，高 2.4px）
    # 8 段环状加厚草窝外缘
    for i in range(8):
        deg = i * 45.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        mat = "straw_nest_gold" if i % 2 == 0 else "straw_nest_dark"
        pad(rig, "thatch_nest", f"nest_rim_{i}", (cos_a * 3.6, 0.65, sin_a * 3.6), (2.8, 1.8, 2.8), mat, rotation=(0.0, deg, 0.0))

    # 交错编织的横向干草茎段
    strands = (
        ("nest_strand_1", (-3.6, 2.1, -1.2), (1.4, 2.4, -3.4), 0.28, "straw_nest_gold"),
        ("nest_strand_2", (1.4, 2.4, -3.4), (3.8, 1.9, 0.8), 0.28, "straw_nest_dark"),
        ("nest_strand_3", (3.8, 1.9, 0.8), (-0.8, 2.3, 3.6), 0.28, "straw_nest_gold"),
        ("nest_strand_4", (-0.8, 2.3, 3.6), (-3.6, 2.1, -1.2), 0.28, "straw_nest_dark"),
    )
    for name, start, end, r, mat in strands:
        strand(rig, "thatch_nest", name, start, end, r, mat)


def part_warm_yellow_flower(rig):
    rig.bone("warm_yellow_flower", (0.0, 0.0, 0.0))
    # 缩小的黄色小花（宽 6.0px，高至 6.8px，点缀在草窝中间）
    # 5 片向斜上方微敞开的暖黄花瓣
    angles = [0.0, 72.0, 144.0, 216.0, 288.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.45
        ortho_z = cos_a * 0.45
        
        # 花瓣从窝心拔起
        p_base = (cos_a * 0.5, 1.8, sin_a * 0.5)
        p_mid = (cos_a * 2.0, 3.8, sin_a * 2.0)
        p_tip = (cos_a * 3.0, 5.2, sin_a * 3.0)
        
        strand(rig, "warm_yellow_flower", f"petal_in_l_{idx}",
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.6, p_mid[1], p_mid[2] - ortho_z * 0.6), 0.40, "petal_warm_yellow")
        strand(rig, "warm_yellow_flower", f"petal_in_r_{idx}",
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.6, p_mid[1], p_mid[2] + ortho_z * 0.6), 0.40, "petal_warm_yellow")

        strand(rig, "warm_yellow_flower", f"petal_out_{idx}",
               (p_mid[0], p_mid[1], p_mid[2]), (p_tip[0], p_tip[1], p_tip[2]), 0.32, "golden_core")


def part_golden_core(rig):
    rig.bone("golden_core", (0.0, 0.0, 0.0))
    # 草窝花心中的温润金黄小蕊 (y: 2.2 -> 5.8)
    pad(rig, "golden_core", "core_receptacle", (0.0, 1.8, 0.0), (1.6, 1.4, 1.6), "straw_nest_dark")
    pad(rig, "golden_core", "core_stamen", (0.0, 3.2, 0.0), (1.3, 1.4, 1.3), "golden_core")
    strand(rig, "golden_core", "core_tuft", (0.0, 4.4, 0.0), (0.0, 6.2, 0.0), 0.28, "golden_core")


def build():
    return build_rig(MATS, (part_thatch_nest, part_warm_yellow_flower, part_golden_core))


GATES = PlantGates("茅心薇 / mao_xin_wei")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("MaoXinWei", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
