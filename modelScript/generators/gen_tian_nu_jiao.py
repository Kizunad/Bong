"""天怒椒：伪灵脉消散焦土所生的怒火灵椒，焦土基座拔起焦黑棘茎，结出三根如沸腾烈火般的粗壮弯角怒椒（#b92418 / #f05f1e），椒尖窜动炽烈火苗（#ffc337）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "scorched_earth": (40, 26, 26),       # 焦土碎岩座
    "charred_stalk": (48, 32, 28),        # 焦黑木茎
    "fiery_pepper_body": (185, 36, 24),   # #b92418 怒火红椒身
    "fiery_pepper_tip": (240, 95, 30),    # #f05f1e 沸腾橙红椒尖
    "flame_sparks": (255, 195, 55),       # #ffc337 炽烈火苗
}


def part_scorched_base(rig):
    rig.bone("scorched_base", (0.0, 0.0, 0.0))
    # 焦土碎石地基 (高 2.4px，宽 6.4px)
    pad(rig, "scorched_base", "base_scorched_slab", (0.0, 0.12, 0.0), (6.2, 1.8, 6.2), "scorched_earth")
    pad(rig, "scorched_base", "base_charred_rock", (-1.4, 0.12, 0.6), (2.8, 2.2, 2.6), "scorched_earth")


def part_charred_stalks(rig):
    rig.bone("charred_stalks", (0.0, 0.0, 0.0))
    # 焦黑刚劲的主分枝 (y: 1.5 -> 6.8)
    strand(rig, "charred_stalks", "stalk_main_root", (0.0, 1.6, 0.0), (0.0, 4.4, 0.0), 0.48, "charred_stalk")
    strand(rig, "charred_stalks", "stalk_branch_l", (0.0, 3.8, 0.0), (-1.8, 6.2, 0.6), 0.38, "charred_stalk")
    strand(rig, "charred_stalks", "stalk_branch_r", (0.0, 3.8, 0.0), (1.8, 6.2, -0.6), 0.38, "charred_stalk")


def part_fiery_peppers(rig):
    rig.bone("fiery_peppers", (0.0, 0.0, 0.0))
    # 三根粗壮霸道、向前向上弯拱的怒火灵椒（成团簇拥，展宽达 10.2px，高至 9.6px）
    
    # 1. 中央主椒 (最大，向前挺立微上弯)
    # 椒蒂与椒肩
    pad(rig, "fiery_peppers", "pepper_c_cap", (0.0, 4.8, 0.2), (2.4, 1.2, 2.4), "charred_stalk")
    pad(rig, "fiery_peppers", "pepper_c_body_mid", (0.0, 5.6, 0.8), (3.0, 3.2, 2.8), "fiery_pepper_body")
    strand(rig, "fiery_peppers", "pepper_c_upper", (0.0, 6.8, 1.2), (0.0, 8.4, 1.8), 0.44, "fiery_pepper_body")
    strand(rig, "fiery_peppers", "pepper_c_tip", (0.0, 8.4, 1.8), (0.0, 9.6, 2.4), 0.30, "fiery_pepper_tip")

    # 2. 左侧斜挺怒椒
    pad(rig, "fiery_peppers", "pepper_l_cap", (-1.8, 6.2, 0.6), (1.8, 1.2, 1.8), "charred_stalk")
    strand(rig, "fiery_peppers", "pepper_l_body", (-1.8, 6.4, 0.6), (-3.2, 7.8, 1.4), 0.48, "fiery_pepper_body")
    strand(rig, "fiery_peppers", "pepper_l_tip", (-3.2, 7.8, 1.4), (-4.2, 8.8, 1.8), 0.32, "fiery_pepper_tip")

    # 3. 右侧斜挺怒椒
    pad(rig, "fiery_peppers", "pepper_r_cap", (1.8, 6.2, -0.6), (1.8, 1.2, 1.8), "charred_stalk")
    strand(rig, "fiery_peppers", "pepper_r_body", (1.8, 6.4, -0.6), (3.2, 7.8, -1.4), 0.48, "fiery_pepper_body")
    strand(rig, "fiery_peppers", "pepper_r_tip", (3.2, 7.8, -1.4), (4.2, 8.8, -1.8), 0.32, "fiery_pepper_tip")


def part_boiling_sparks(rig):
    rig.bone("boiling_sparks", (0.0, 0.0, 0.0))
    # 椒尖凝聚的自爆沸腾真元火苗与热烈火星 (#ffc337)
    pad(rig, "boiling_sparks", "spark_apex_c", (0.0, 9.8, 2.5), (0.9, 0.9, 0.9), "flame_sparks")
    pad(rig, "boiling_sparks", "spark_apex_l", (-4.4, 9.0, 1.9), (0.8, 0.8, 0.8), "flame_sparks")
    pad(rig, "boiling_sparks", "spark_apex_r", (4.4, 9.0, -1.9), (0.8, 0.8, 0.8), "flame_sparks")
    # 浮动火星
    pad(rig, "boiling_sparks", "spark_mote_top", (0.0, 10.4, 1.8), (0.45, 0.45, 0.45), "flame_sparks")


def build():
    return build_rig(MATS, (part_scorched_base, part_charred_stalks, part_fiery_peppers, part_boiling_sparks))


GATES = PlantGates("天怒椒 / tian_nu_jiao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("TianNuJiao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
