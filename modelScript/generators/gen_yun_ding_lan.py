"""云顶兰返工版：底部灰黑碎岩座（#2a2a2e / #4a4a50），上面开放大宽瓣飘逸银白兰花（#cbdbe4 / #fcfcff）。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "rock_dark": (42, 42, 46),           # #2a2a2e 灰黑碎岩底
    "rock_mid": (74, 74, 80),            # #4a4a50 灰岩石块
    "orchid_leaf": (65, 82, 90),         # 苍灰青基叶
    "silver_petal": (205, 218, 228),     # #cbdbe4 银白大宽瓣
    "pure_white_lip": (252, 252, 255),   # #fcfcff 纯白兜唇
}


def part_rock_base(rig):
    rig.bone("rock_base", (0.0, 0.0, 0.0))
    # 底部厚重灰黑碎岩底座 (高 3.0px，宽 6.8px)
    pad(rig, "rock_base", "rock_base_core", (0.0, 0.12, 0.0), (6.6, 2.6, 6.2), "rock_dark")
    pad(rig, "rock_base", "rock_step_l", (-1.8, 0.12, 0.6), (3.2, 3.2, 3.0), "rock_mid")
    pad(rig, "rock_base", "rock_step_r", (1.6, 0.12, -0.6), (3.0, 2.2, 3.4), "rock_dark")
    # 兰花主茎直接扎入岩座中拔出 (y: 2.4 -> 4.5)
    strand(rig, "rock_base", "rock_stem_stalk", (0.0, 2.2, 0.0), (0.0, 4.4, 0.0), 0.48, "orchid_leaf")


def part_orchid_leaves(rig):
    rig.bone("orchid_leaves", (0.0, 0.0, 0.0))
    # 从岩石缝中垂下的 4 片宽大狭长兰草叶
    leaves = (
        ("leaf_l_low", (-0.4, 2.4, 0.2), (-2.8, 3.8, 0.6), 0.46),
        ("leaf_l_arch", (-2.8, 3.8, 0.6), (-4.8, 2.8, 0.9), 0.36),
        ("leaf_r_low", (0.4, 2.4, -0.2), (2.8, 3.8, -0.6), 0.46),
        ("leaf_r_arch", (2.8, 3.8, -0.6), (4.8, 2.8, -0.9), 0.36),
        ("leaf_b_low", (0.0, 2.4, -0.5), (0.0, 3.9, -3.2), 0.44),
        ("leaf_b_arch", (0.0, 3.9, -3.2), (0.0, 2.8, -4.9), 0.34),
    )
    for name, start, end, r in leaves:
        strand(rig, "orchid_leaves", name, start, end, r, "orchid_leaf")


def part_silver_orchid_flower(rig):
    rig.bone("silver_orchid_flower", (0.0, 0.0, 0.0))
    # 大幅加宽的银白大兰花瓣（横向宽达 10.6px，高至 9.8px，宽瓣有面）
    # 1. 挺拔上位大主萼片 (高耸且宽厚，双排铺面)
    pad(rig, "silver_orchid_flower", "petal_dorsal_broad", (0.0, 5.8, -0.2), (2.6, 3.4, 0.6), "silver_petal")
    pad(rig, "silver_orchid_flower", "petal_dorsal_apex", (0.0, 8.4, -0.3), (1.8, 1.8, 0.5), "pure_white_lip")

    # 2. 左右横向飞展的宽大双侧翼花瓣 (向侧上展开，各宽 2.4px，展宽 10.6px)
    pad(rig, "silver_orchid_flower", "petal_wing_l_in", (-2.4, 6.2, 0.2), (2.8, 1.8, 0.6), "silver_petal", rotation=(0.0, -15.0, 16.0))
    pad(rig, "silver_orchid_flower", "petal_wing_l_out", (-4.4, 7.2, 0.4), (2.2, 1.4, 0.5), "pure_white_lip", rotation=(0.0, -15.0, 24.0))

    pad(rig, "silver_orchid_flower", "petal_wing_r_in", (2.4, 6.2, -0.2), (2.8, 1.8, 0.6), "silver_petal", rotation=(0.0, 15.0, -16.0))
    pad(rig, "silver_orchid_flower", "petal_wing_r_out", (4.4, 7.2, -0.4), (2.2, 1.4, 0.5), "pure_white_lip", rotation=(0.0, 15.0, -24.0))

    # 3. 左右下侧斜向展开的宽萼片
    pad(rig, "silver_orchid_flower", "petal_sepal_l", (-2.6, 4.4, -0.2), (2.6, 1.4, 0.6), "silver_petal", rotation=(0.0, 18.0, -18.0))
    pad(rig, "silver_orchid_flower", "petal_sepal_r", (2.6, 4.4, 0.2), (2.6, 1.4, 0.6), "silver_petal", rotation=(0.0, -18.0, 18.0))


def part_orchid_lip(rig):
    rig.bone("orchid_lip", (0.0, 0.0, 0.0))
    # 兰花大兜唇瓣 (向前突出，宽大饱满，y: 3.8 -> 5.8)
    pad(rig, "orchid_lip", "lip_pouch_core", (0.0, 4.2, 1.6), (3.2, 1.8, 2.6), "pure_white_lip")
    pad(rig, "orchid_lip", "lip_pouch_rim_l", (-1.4, 4.4, 1.6), (1.1, 1.4, 2.0), "silver_petal")
    pad(rig, "orchid_lip", "lip_pouch_rim_r", (1.4, 4.4, 1.6), (1.1, 1.4, 2.0), "silver_petal")
    pad(rig, "orchid_lip", "lip_front_apron", (0.0, 3.6, 2.8), (2.4, 0.8, 1.8), "pure_white_lip")


def build():
    return build_rig(MATS, (part_rock_base, part_orchid_leaves, part_silver_orchid_flower, part_orchid_lip))


GATES = PlantGates("云顶兰 / yun_ding_lan")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("YunDingLan", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
