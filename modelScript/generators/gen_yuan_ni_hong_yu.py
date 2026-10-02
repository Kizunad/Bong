"""渊泥红玉：共生于渊泥黑树伞下的玉红蕨体，深红晶透羽叶放射簇生，叶尖带晶润红玉珠。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "black_mud": (32, 28, 30),
    "ruby_deep": (135, 24, 28),        # 深玉红 #87181c
    "ruby_bright": (215, 45, 52),      # 鲜玉红 #d72d34
    "ruby_gem_tip": (252, 175, 185),   # 晶透红玉尖 #fcafb9
    "fern_stalk_dark": (68, 20, 22),
}


def part_mud_base(rig):
    rig.bone("mud_base", (0.0, 0.0, 0.0))
    # 渊泥黑树下湿润泥沼基座
    pad(rig, "mud_base", "mud_mound", (0.0, 0.12, 0.0), (5.2, 0.85, 5.2), "black_mud")
    pad(rig, "mud_base", "mud_stalk_collar", (0.0, 0.85, 0.0), (3.4, 1.2, 3.4), "fern_stalk_dark")


def part_ruby_fronds(rig):
    rig.bone("ruby_fronds", (0.0, 0.0, 0.0))
    # 6 束饱满向外大弧度舒展的厚重玉红蕨叶（红玉质感，全幅展开达 10.4px，高 7.8px）
    angles = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        
        ortho_x = -sin_a * 0.72
        ortho_z = cos_a * 0.72
        
        p_base = (cos_a * 0.8, 1.8, sin_a * 0.8)
        p_mid = (cos_a * 3.2, 4.8, sin_a * 3.2)
        p_tip = (cos_a * 5.1, 5.6, sin_a * 5.1)
        
        # 宽厚双排玉红羽片
        strand(rig, "ruby_fronds", f"frond_in_l_{idx}",
               (p_base[0] - ortho_x, p_base[1], p_base[2] - ortho_z),
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8), 0.52, "ruby_deep")
        strand(rig, "ruby_fronds", f"frond_in_r_{idx}",
               (p_base[0] + ortho_x, p_base[1], p_base[2] + ortho_z),
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8), 0.52, "ruby_deep")

        strand(rig, "ruby_fronds", f"frond_out_l_{idx}",
               (p_mid[0] - ortho_x * 0.8, p_mid[1], p_mid[2] - ortho_z * 0.8),
               (p_tip[0], p_tip[1], p_tip[2]), 0.42, "ruby_bright")
        strand(rig, "ruby_fronds", f"frond_out_r_{idx}",
               (p_mid[0] + ortho_x * 0.8, p_mid[1], p_mid[2] + ortho_z * 0.8),
               (p_tip[0], p_tip[1], p_tip[2]), 0.42, "ruby_bright")


def part_gem_tips(rig):
    rig.bone("gem_tips", (0.0, 0.0, 0.0))
    # 6 束蕨叶末端凝聚结出的晶莹剔透红玉宝珠与顶心红玉核 (极高灵质)
    angles = [0.0, 60.0, 120.0, 180.0, 240.0, 300.0]
    for idx, deg in enumerate(angles):
        rad = math.radians(deg)
        cos_a = math.cos(rad)
        sin_a = math.sin(rad)
        p_tip = (cos_a * 5.2, 5.8, sin_a * 5.2)
        pad(rig, "gem_tips", f"gem_bead_{idx}", (p_tip[0], p_tip[1], p_tip[2]), (1.1, 1.1, 1.1), "ruby_gem_tip")

    # 中央红玉结晶直立主核 (高 7.6px)
    pad(rig, "gem_tips", "gem_crown_core", (0.0, 4.2, 0.0), (2.2, 2.4, 2.2), "ruby_deep")
    pad(rig, "gem_tips", "gem_crown_apex", (0.0, 6.4, 0.0), (1.4, 1.6, 1.4), "ruby_gem_tip")


def build():
    return build_rig(MATS, (part_mud_base, part_ruby_fronds, part_gem_tips))


GATES = PlantGates("渊泥红玉 / yuan_ni_hong_yu")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YuanNiHongYu", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
