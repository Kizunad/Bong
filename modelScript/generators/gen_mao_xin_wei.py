"""茅心薇：隐居茅舍残痕里生出的暖黄小薇，多层宽厚金黄暖瓣（#eeb22d / #fcde6e），残存温润灵光。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "thatch_debris": (58, 48, 36),
    "leaf_moss_green": (52, 68, 42),
    "petal_warm_yellow": (238, 178, 45), # #eeb22d 暖黄花瓣
    "petal_amber_glow": (252, 222, 110), # 亮黄边缘光晕
    "pistil_residual": (165, 95, 30),
}


def part_thatch_base(rig):
    rig.bone("thatch_base", (0.0, 0.0, 0.0))
    # 茅舍残木与枯草碎屑台座
    pad(rig, "thatch_base", "thatch_ground", (0.0, 0.12, 0.0), (4.8, 0.85, 4.8), "thatch_debris")
    pad(rig, "thatch_base", "thatch_rotten_timber", (-0.8, 0.45, 0.6), (3.6, 0.95, 1.8), "thatch_debris", rotation=(0.0, 18.0, 0.0))


def part_green_bracts(rig):
    rig.bone("green_bracts", (0.0, 0.0, 0.0))
    # 底部托着花朵的一圈苍绿苞片与基生嫩叶（5 片放射）
    angles = [36.0, 108.0, 180.0, 252.0, 324.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (0.0, 1.2, 0.0)
        p_tip = (cos_a * 4.6, 2.2, sin_a * 4.6)
        strand(rig, "green_bracts", f"bract_leaf_{idx}", p_base, p_tip, 0.42, "leaf_moss_green")


def part_warm_yellow_flower(rig):
    rig.bone("warm_yellow_flower", (0.0, 0.0, 0.0))
    # 8 片饱满厚实大宽瓣多层放射展开（外轮 5 片大瓣，内轮 3 片立瓣，金黄暖色成团，高 7.8px，阔 10.2px）
    # 1. 外轮 5 片大宽瓣
    angles_out = [0.0, 72.0, 144.0, 216.0, 288.0]
    for idx, deg in enumerate(angles_out):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.72
        ortho_z = cos_a * 0.72
        
        p_base = (cos_a * 0.8, 2.2, sin_a * 0.8)
        p_mid = (cos_a * 3.2, 4.8, sin_a * 3.2)
        p_tip = (cos_a * 4.9, 5.8, sin_a * 4.9)
        
        # 宽厚双排金黄瓣面
        strand(rig, "warm_yellow_flower", f"petal_out_l_{idx}",
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8), 0.52, "petal_warm_yellow")
        strand(rig, "warm_yellow_flower", f"petal_out_r_{idx}",
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8), 0.52, "petal_warm_yellow")

        # 瓣端暖光边
        strand(rig, "warm_yellow_flower", f"petal_tip_{idx}",
               (p_mid[0], p_mid[1], p_mid[2]), (p_tip[0], p_tip[1], p_tip[2]), 0.42, "petal_amber_glow")

    # 2. 内轮 3 片半抱合立瓣
    for i in range(3):
        deg = i * 120.0 + 36.0
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_base = (cos_a * 0.6, 3.2, sin_a * 0.6)
        p_tip = (cos_a * 2.4, 6.8, sin_a * 2.4)
        strand(rig, "warm_yellow_flower", f"petal_in_up_{i}", p_base, p_tip, 0.44, "petal_warm_yellow")


def part_golden_core(rig):
    rig.bone("golden_core", (0.0, 0.0, 0.0))
    # 花心温润残灵金黄花蕊 (高 7.2px)
    pad(rig, "golden_core", "core_receptacle", (0.0, 3.6, 0.0), (1.8, 1.8, 1.8), "pistil_residual")
    pad(rig, "golden_core", "core_eye", (0.0, 5.2, 0.0), (1.4, 1.4, 1.4), "petal_amber_glow")
    strand(rig, "golden_core", "core_tuft", (0.0, 5.2, 0.0), (0.0, 7.4, 0.0), 0.32, "petal_amber_glow")


def build():
    return build_rig(MATS, (part_thatch_base, part_green_bracts, part_warm_yellow_flower, part_golden_core))


GATES = PlantGates("茅心薇 / mao_xin_wei")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("MaoXinWei", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
