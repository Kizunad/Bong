"""无言果：形如安神果然无棱，灰蓝哑光果皮（#5f6673 / #96a0af）浑圆无痕，表皮隐缀无言暗斑。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "decay_soil": (38, 38, 45),          # 毒蚀泥台
    "toxic_leaf": (52, 58, 65),          # 哑光枯灰叶
    "fruit_slate_dark": (95, 102, 115),  # #5f6673 灰蓝哑光果皮
    "fruit_slate_light": (150, 160, 175),# #96a0af 顶端亮灰
    "mute_venom_speck": (62, 54, 82),    # #3e3652 无言毒斑
}


def part_toxic_base(rig):
    rig.bone("toxic_base", (0.0, 0.0, 0.0))
    # 底部毒化灰土底座 (高 2.2px，宽 6.4px)
    pad(rig, "toxic_base", "base_decay_slab", (0.0, 0.12, 0.0), (6.2, 1.8, 6.2), "decay_soil")
    pad(rig, "toxic_base", "base_stalk_collar", (0.0, 1.5, 0.0), (2.8, 1.2, 2.8), "toxic_leaf")


def part_mute_leaves(rig):
    rig.bone("mute_leaves", (0.0, 0.0, 0.0))
    # 底部承托的 4 片哑光枯灰平叶（向四方舒展，展宽 9.0px）
    leaves = (
        ("leaf_mute_f", (0.0, 1.6, 0.8), (0.0, 2.6, 3.8), 0.46),
        ("leaf_mute_b", (0.0, 1.6, -0.8), (0.0, 2.6, -3.8), 0.46),
        ("leaf_mute_l", (-0.8, 1.6, 0.0), (-3.8, 2.6, 0.0), 0.46),
        ("leaf_mute_r", (0.8, 1.6, 0.0), (3.8, 2.6, 0.0), 0.46),
    )
    for name, start, end, r in leaves:
        strand(rig, "mute_leaves", name, start, end, r, "toxic_leaf")


def part_smooth_fruit(rig):
    rig.bone("smooth_fruit", (0.0, 0.0, 0.0))
    # 浑圆平滑无棱的桃卵形果实（y: 2.6 -> 9.4，整块平滑过渡，无棱无凹槽）
    # 1. 主卵形圆润果肉 (圆柱倒角多层叠加，四周完全光滑对称)
    pad(rig, "smooth_fruit", "fruit_core_body", (0.0, 2.6, 0.0), (5.2, 4.4, 5.2), "fruit_slate_dark")
    pad(rig, "smooth_fruit", "fruit_bulge_x", (0.0, 3.0, 0.0), (5.8, 3.6, 4.4), "fruit_slate_dark")
    pad(rig, "smooth_fruit", "fruit_bulge_z", (0.0, 3.0, 0.0), (4.4, 3.6, 5.8), "fruit_slate_dark")
    # 2. 顶端平滑收敛穹顶与圆滑顶部 (y: 6.8 -> 9.4)
    pad(rig, "smooth_fruit", "fruit_top_dome", (0.0, 6.8, 0.0), (4.0, 1.6, 4.0), "fruit_slate_light")
    pad(rig, "smooth_fruit", "fruit_apex_smooth", (0.0, 8.2, 0.0), (2.4, 1.2, 2.4), "fruit_slate_light")


def part_mute_spots(rig):
    rig.bone("mute_spots", (0.0, 0.0, 0.0))
    # 果皮表面的无言哑毒暗斑（#3e3652，贴于前后左右表皮上）
    specks = (
        ("spot_front_l", (-1.2, 4.8, 2.65), (0.7, 0.7, 0.25), "mute_venom_speck"),
        ("spot_front_r", (1.4, 5.6, 2.65), (0.8, 0.8, 0.25), "mute_venom_speck"),
        ("spot_side_r", (2.95, 4.5, -0.6), (0.25, 0.7, 0.7), "mute_venom_speck"),
        ("spot_side_l", (-2.95, 5.2, 0.8), (0.25, 0.7, 0.7), "mute_venom_speck"),
        ("spot_top_cap", (0.4, 8.8, 0.3), (0.7, 0.7, 0.7), "mute_venom_speck"),
    )
    for name, pos, size, mat in specks:
        pad(rig, "mute_spots", name, pos, size, mat)


def build():
    return build_rig(MATS, (part_toxic_base, part_mute_leaves, part_smooth_fruit, part_mute_spots))


GATES = PlantGates("无言果 / wu_yan_guo")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("WuYanGuo", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
