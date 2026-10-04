"""蜉蝣花：看似清丽，食一瓣即发狂。层叠半透明宽薄花瓣如蜉蝣薄翼，花心隐现暗紫毒蕊。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "soil_decay": (36, 42, 48),
    "stalk_translucent": (75, 95, 115),
    "petal_pale_cyan": (145, 175, 205),
    "petal_edge_white": (220, 235, 248),
    "toxic_stamen": (80, 50, 110),
}


def part_decay_base(rig):
    rig.bone("decay_base", (0.0, 0.0, 0.0))
    pad(rig, "decay_base", "base_soil", (0.0, 0.12, 0.0), (4.5, 0.85, 4.5), "soil_decay")
    # 细弱微晶透明花茎
    strand(rig, "decay_base", "base_stem", (0.0, 0.85, 0.0), (0.0, 4.20, 0.0), 0.45, "stalk_translucent")


def part_translucent_petals(rig):
    rig.bone("translucent_petals", (0.0, 0.0, 0.0))
    # 6 片向外层叠舒展的大型薄翼花瓣（展宽达 10.5px，高 8.5px，薄翼成团）
    angles = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.75
        ortho_z = cos_a * 0.75
        
        # 花瓣向外拱曲舒展 (基部 y=4.2 -> 中部拱起 y=6.2 -> 边缘微下垂 y=5.8)
        p_base = (cos_a * 0.8, 4.2, sin_a * 0.8)
        p_mid = (cos_a * 3.4, 6.2, sin_a * 3.4)
        p_tip = (cos_a * 5.2, 5.8, sin_a * 5.2)
        
        # 宽薄花瓣（双条并合成面，带白色透光亮边）
        strand(rig, "translucent_petals", f"petal_wing_l_{idx}",
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.9, p_mid[1], p_mid[2] - ortho_z * 0.9), 0.48, "petal_pale_cyan")
        strand(rig, "translucent_petals", f"petal_wing_r_{idx}",
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.9, p_mid[1], p_mid[2] + ortho_z * 0.9), 0.48, "petal_pale_cyan")

        # 外缘薄翼包边
        strand(rig, "translucent_petals", f"petal_edge_l_{idx}",
               (p_mid[0] - ortho_x * 0.9, p_mid[1], p_mid[2] - ortho_z * 0.9),
               (p_tip[0], p_tip[1], p_tip[2]), 0.38, "petal_edge_white")
        strand(rig, "translucent_petals", f"petal_edge_r_{idx}",
               (p_mid[0] + ortho_x * 0.9, p_mid[1], p_mid[2] + ortho_z * 0.9),
               (p_tip[0], p_tip[1], p_tip[2]), 0.38, "petal_edge_white")


def part_toxic_pistil(rig):
    rig.bone("toxic_pistil", (0.0, 0.0, 0.0))
    # 花心隐秘暗紫发狂毒蕊 (聚集在中心花托，向上挺立至 y=7.8)
    pad(rig, "toxic_pistil", "stamen_core_receptacle", (0.0, 4.4, 0.0), (2.0, 1.8, 2.0), "toxic_stamen")
    strand(rig, "toxic_pistil", "stamen_point_c", (0.0, 5.8, 0.0), (0.0, 7.8, 0.0), 0.30, "toxic_stamen")
    strand(rig, "toxic_pistil", "stamen_point_l", (-0.6, 5.6, 0.2), (-0.9, 7.2, 0.4), 0.24, "toxic_stamen")
    strand(rig, "toxic_pistil", "stamen_point_r", (0.6, 5.6, -0.2), (0.9, 7.2, -0.4), 0.24, "toxic_stamen")


def build():
    return build_rig(MATS, (part_decay_base, part_translucent_petals, part_toxic_pistil))


GATES = PlantGates("蜉蝣花 / fu_you_hua")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("FuYouHua", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
