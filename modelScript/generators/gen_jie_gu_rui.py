"""解蛊蕊返工版：底部暗灰晶岩块（#2a2e3a，带蓝白细纹 #7ab8ff），上面堆 14 颗紫色圆珠小花苞（#6a3ab0 / #9a6ae0）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "crystal_rock": (42, 46, 58),         # #2a2e3a 暗灰晶岩
    "rock_vein": (122, 184, 255),         # #7ab8ff 蓝白细纹
    "purple_bud_dark": (75, 42, 125),     # 暗紫底色
    "purple_bud_mid": (106, 58, 176),     # #6a3ab0 主紫小苞
    "purple_bud_light": (154, 106, 224),  # #9a6ae0 亮紫小苞
}


def part_crystal_rock_base(rig):
    rig.bone("crystal_rock_base", (0.0, 0.0, 0.0))
    # 底部厚重暗灰晶岩块 (高 2.8px，宽 6.6px)
    pad(rig, "crystal_rock_base", "rock_base_main", (0.0, 0.12, 0.0), (6.4, 2.6, 6.2), "crystal_rock")
    pad(rig, "crystal_rock_base", "rock_step_l", (-1.8, 0.12, -0.6), (3.0, 3.2, 3.2), "crystal_rock")
    pad(rig, "crystal_rock_base", "rock_step_r", (1.8, 0.12, 0.8), (3.2, 2.2, 3.4), "crystal_rock")


def part_rock_blue_veins(rig):
    rig.bone("rock_blue_veins", (0.0, 0.0, 0.0))
    # 晶岩表面的蓝白发光细纹 (#7ab8ff)，贴在岩石外侧面上
    # 前外壁细纹 (z ~ 3.15)
    strand(rig, "rock_blue_veins", "vein_front", (-2.2, 0.8, 3.15), (0.8, 2.2, 3.15), 0.22, "rock_vein")
    # 右外壁细纹 (x ~ 3.25)
    strand(rig, "rock_blue_veins", "vein_side_r", (3.25, 0.8, -1.8), (3.25, 2.0, 1.2), 0.22, "rock_vein")
    # 前突岩台斜裂纹
    strand(rig, "rock_blue_veins", "vein_diagonal", (-1.2, 2.65, 2.4), (1.4, 2.65, 2.8), 0.20, "rock_vein")


def part_purple_bead_cluster(rig):
    rig.bone("purple_bead_cluster", (0.0, 0.0, 0.0))
    # 堆叠在岩块上的 14 颗紫色圆珠小苞（像一串饱满紧密的紫葡萄簇，高至 9.2px）
    # 下层基座小球 (7 颗，y: 2.8 ~ 4.8)
    lower_beads = (
        ("bead_low_c", (0.0, 2.8, 0.0), (2.4, 2.4, 2.4), "purple_bud_dark"),
        ("bead_low_f", (0.2, 2.6, 1.6), (2.2, 2.2, 2.2), "purple_bud_mid"),
        ("bead_low_b", (-0.2, 2.6, -1.6), (2.2, 2.2, 2.2), "purple_bud_dark"),
        ("bead_low_l", (-1.6, 3.0, 0.2), (2.2, 2.2, 2.2), "purple_bud_mid"),
        ("bead_low_r", (1.6, 2.8, -0.2), (2.2, 2.2, 2.2), "purple_bud_mid"),
        ("bead_low_fl", (-1.2, 2.7, 1.4), (2.0, 2.0, 2.0), "purple_bud_mid"),
        ("bead_low_br", (1.2, 2.9, -1.4), (2.0, 2.0, 2.0), "purple_bud_dark"),
    )
    for name, pos, size, mat in lower_beads:
        pad(rig, "purple_bead_cluster", name, pos, size, mat)

    # 中层聚拢小球 (5 颗，y: 4.8 ~ 7.0)
    mid_beads = (
        ("bead_mid_c", (0.1, 4.8, 0.1), (2.2, 2.2, 2.2), "purple_bud_mid"),
        ("bead_mid_f", (0.3, 4.9, 1.2), (2.0, 2.0, 2.0), "purple_bud_light"),
        ("bead_mid_b", (-0.3, 5.0, -1.1), (2.0, 2.0, 2.0), "purple_bud_mid"),
        ("bead_mid_l", (-1.1, 5.1, 0.1), (1.9, 1.9, 1.9), "purple_bud_light"),
        ("bead_mid_r", (1.2, 4.8, -0.1), (1.9, 1.9, 1.9), "purple_bud_light"),
    )
    for name, pos, size, mat in mid_beads:
        pad(rig, "purple_bead_cluster", name, pos, size, mat)

    # 顶层簇尖小球 (3 颗收尖，y: 6.8 ~ 8.8)
    top_beads = (
        ("bead_top_apex", (0.0, 6.8, 0.0), (1.8, 1.8, 1.8), "purple_bud_light"),
        ("bead_top_side_a", (0.6, 6.5, 0.5), (1.5, 1.5, 1.5), "purple_bud_light"),
        ("bead_top_side_b", (-0.5, 6.6, -0.4), (1.5, 1.5, 1.5), "purple_bud_light"),
    )
    for name, pos, size, mat in top_beads:
        pad(rig, "purple_bead_cluster", name, pos, size, mat)


def build():
    return build_rig(MATS, (part_crystal_rock_base, part_rock_blue_veins, part_purple_bead_cluster))


GATES = PlantGates("解蛊蕊 / jie_gu_rui")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("JieGuRui", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
