"""无言果返工版：珍珠白圆球（#e8e4d8，高光 #ffffff），被一圈 6 片暗银色尖叶（#4a4e58）向上紧密包托。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_base": (45, 38, 32),
    "silver_leaf": (65, 80, 105),        # #4a4e58 暗银尖叶
    "pedicel_green": (48, 75, 42),        # 果柄叶心
    "pearl_body": (235, 225, 205),        # #e8e4d8 珍珠白圆球
    "pearl_glow": (255, 255, 255),        # #ffffff 纯白高光
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    # 底部基座 (高 1.6px，宽 5.8px)
    pad(rig, "soil_base", "base_soil_pad", (0.0, 0.12, 0.0), (5.8, 1.4, 5.8), "soil_base")
    # 果座叶柄短环
    pad(rig, "soil_base", "base_pedicel", (0.0, 1.2, 0.0), (2.6, 1.2, 2.6), "pedicel_green")


def part_silver_leaves(rig):
    rig.bone("silver_leaves", (0.0, 0.0, 0.0))
    # 6 片暗银色尖叶向上包托着珍珠白圆球（展宽 8.8px，高 6.4px，尖叶成托）
    angles = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        # 叶片自基部外拱后向上收紧抱住珍珠圆球
        p_base = (cos_a * 1.2, 1.4, sin_a * 1.2)
        p_mid = (cos_a * 3.6, 3.2, sin_a * 3.6)
        p_tip = (cos_a * 3.1, 6.2, sin_a * 3.1)
        
        strand(rig, "silver_leaves", f"leaf_cup_low_{idx}", p_base, p_mid, 0.44, "silver_leaf")
        strand(rig, "silver_leaves", f"leaf_cup_high_{idx}", p_mid, p_tip, 0.32, "silver_leaf")


def part_pearl_sphere(rig):
    rig.bone("pearl_sphere", (0.0, 0.0, 0.0))
    # 阶梯逐层递变逼近的饱满珍珠白圆球（y: 2.8 -> 9.0，直径 5.4px，浑圆如珠）
    # 1. 下层底部层 (3.6x3.6 截面，y: 2.8 -> 4.2)
    pad(rig, "pearl_sphere", "pearl_base_layer", (0.0, 2.8, 0.0), (3.6, 1.4, 3.6), "pearl_body")

    # 2. 中层赤道膨大层 (5.2x5.2 截面，带四方圆弧凸出，y: 4.2 -> 6.8)
    pad(rig, "pearl_sphere", "pearl_equator_core", (0.0, 4.2, 0.0), (5.0, 2.6, 5.0), "pearl_body")
    pad(rig, "pearl_sphere", "pearl_equator_x", (0.0, 4.6, 0.0), (5.6, 1.8, 4.2), "pearl_body")
    pad(rig, "pearl_sphere", "pearl_equator_z", (0.0, 4.6, 0.0), (4.2, 1.8, 5.6), "pearl_body")

    # 3. 上层穹顶收敛层 (3.8x3.8 截面，y: 6.8 -> 8.2)
    pad(rig, "pearl_sphere", "pearl_dome_layer", (0.0, 6.8, 0.0), (3.8, 1.4, 3.8), "pearl_body")

    # 4. 顶端圆球冠顶 (2.2x2.2 截面，y: 8.2 -> 9.0)
    pad(rig, "pearl_sphere", "pearl_top_cap", (0.0, 8.2, 0.0), (2.2, 0.8, 2.2), "pearl_body")


def part_white_highlight(rig):
    rig.bone("white_highlight", (0.0, 0.0, 0.0))
    # 珍珠白圆球表面的纯白发光点与顶尖高光 (#ffffff)
    pad(rig, "white_highlight", "glow_front_highlight", (0.6, 7.2, 1.8), (1.1, 1.1, 0.6), "pearl_glow")
    pad(rig, "white_highlight", "glow_top_pip", (0.0, 8.7, 0.0), (1.2, 0.5, 1.2), "pearl_glow")


def build():
    return build_rig(MATS, (part_soil_base, part_silver_leaves, part_pearl_sphere, part_white_highlight))


GATES = PlantGates("无言果 / wu_yan_guo")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("WuYanGuo", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
