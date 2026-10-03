"""渊泥红玉返工版：底部黑色泥岩底座（#1c181a），红色玉质羽叶向外舒展成团，叶尖平滑收细为暗红（#6a1010）。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "black_mud_rock": (28, 24, 26),     # #1c181a 渊泥黑岩
    "black_mud_crust": (42, 36, 40),    # 泥岩外层
    "ruby_deep": (135, 24, 28),         # 深玉红 #87181c
    "ruby_bright": (215, 45, 52),       # 鲜玉红 #d72d34
    "ruby_tip_dark": (106, 16, 16),     # #6a1010 暗红收尖
}


def part_mud_rock_base(rig):
    rig.bone("mud_rock_base", (0.0, 0.0, 0.0))
    # 底部厚重黑色泥岩底座 (高 2.8px，宽 6.6px)
    pad(rig, "mud_rock_base", "mud_rock_main", (0.0, 0.12, 0.0), (6.6, 2.6, 6.2), "black_mud_rock")
    pad(rig, "mud_rock_base", "mud_rock_ledge_l", (-1.8, 0.12, 0.6), (3.0, 3.2, 3.0), "black_mud_crust")
    pad(rig, "mud_rock_base", "mud_rock_ledge_r", (1.6, 0.12, -0.6), (3.0, 2.2, 3.2), "black_mud_rock")
    # 泥岩正中向上拔起的深红粗短茎托 (y: 2.2 -> 3.8)
    strand(rig, "mud_rock_base", "mud_stalk_collar", (0.0, 2.2, 0.0), (0.0, 3.8, 0.0), 0.55, "ruby_deep")


def part_ruby_fronds(rig):
    rig.bone("ruby_fronds", (0.0, 0.0, 0.0))
    # 6 束宽大向外大弧度舒展拱垂的玉红羽片（成团成簇，宽达 10.8px，高 7.6px）
    angles = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.72
        ortho_z = cos_a * 0.72
        
        p_base = (cos_a * 0.8, 3.2, sin_a * 0.8)
        p_mid = (cos_a * 3.2, 5.2, sin_a * 3.2)
        p_outer = (cos_a * 4.6, 5.0, sin_a * 4.6)
        
        # 宽厚双排玉红羽片
        strand(rig, "ruby_fronds", f"frond_in_l_{idx}",
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8), 0.52, "ruby_deep")
        strand(rig, "ruby_fronds", f"frond_in_r_{idx}",
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8), 0.52, "ruby_deep")

        strand(rig, "ruby_fronds", f"frond_out_l_{idx}",
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8),
               (p_outer[0], p_outer[1], p_outer[2]), 0.42, "ruby_bright")
        strand(rig, "ruby_fronds", f"frond_out_r_{idx}",
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8),
               (p_outer[0], p_outer[1], p_outer[2]), 0.42, "ruby_bright")


def part_tapered_tips(rig):
    rig.bone("tapered_tips", (0.0, 0.0, 0.0))
    # 6 束羽叶末端平滑收细的暗红锐利叶尖（#6a1010，自然下垂收尖，无粉色块）
    angles = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_outer = (cos_a * 4.6, 5.0, sin_a * 4.6)
        p_tip = (cos_a * 5.4, 4.4, sin_a * 5.4)
        strand(rig, "tapered_tips", f"tip_taper_{idx}", p_outer, p_tip, 0.28, "ruby_tip_dark")

    # 中央直立玉红心芽收尖 (y: 3.8 -> 7.4)
    pad(rig, "tapered_tips", "tip_crown_mid", (0.0, 4.2, 0.0), (1.8, 2.2, 1.8), "ruby_deep")
    strand(rig, "tapered_tips", "tip_crown_apex", (0.0, 5.6, 0.0), (0.0, 7.4, 0.0), 0.32, "ruby_tip_dark")


def build():
    return build_rig(MATS, (part_mud_rock_base, part_ruby_fronds, part_tapered_tips))


GATES = PlantGates("渊泥红玉 / yuan_ni_hong_yu")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YuanNiHongYu", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
