"""浮尘草返工版：底部暗色晶质叶丛底座（#2a2e34），顶上 3~5 个灰白孢子小球（#c8c8c8）悬浮成串。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "base_leaf_dark": (42, 46, 52),    # #2a2e34
    "base_leaf_mid": (65, 72, 80),
    "spore_ball": (200, 200, 200),     # #c8c8c8
    "spore_halo": (225, 235, 230),
    "stalk_pale": (85, 95, 100),
}


def part_crystal_leaves(rig):
    rig.bone("crystal_leaves", (0.0, 0.0, 0.0))
    # 底部暗灰晶质尖叶丛厚重底座（放射丛生，占据底面大部分，高 3.5px，展宽 8.6px）
    pad(rig, "crystal_leaves", "leaf_base_soil", (0.0, 0.12, 0.0), (4.4, 0.85, 4.4), "base_leaf_dark")
    
    # 放射晶质尖叶（E, W, S, N, NE, NW, SE, SW）
    leaves = (
        ("leaf_e", (0.0, 0.6, 0.0), (3.8, 2.2, 0.0), 0.42, "base_leaf_dark"),
        ("leaf_w", (0.0, 0.6, 0.0), (-3.8, 2.2, 0.0), 0.42, "base_leaf_dark"),
        ("leaf_s", (0.0, 0.6, 0.0), (0.0, 2.2, 3.8), 0.42, "base_leaf_dark"),
        ("leaf_n", (0.0, 0.6, 0.0), (0.0, 2.2, -3.8), 0.42, "base_leaf_dark"),
        ("leaf_ne", (0.0, 0.8, 0.0), (2.8, 3.2, -2.8), 0.36, "base_leaf_mid"),
        ("leaf_nw", (0.0, 0.8, 0.0), (-2.8, 3.2, -2.8), 0.36, "base_leaf_mid"),
        ("leaf_se", (0.0, 0.8, 0.0), (2.8, 3.2, 2.8), 0.36, "base_leaf_mid"),
        ("leaf_sw", (0.0, 0.8, 0.0), (-2.8, 3.2, 2.8), 0.36, "base_leaf_mid"),
    )
    for name, start, end, r, mat in leaves:
        strand(rig, "crystal_leaves", name, start, end, r, mat)


def part_connecting_stalks(rig):
    rig.bone("connecting_stalks", (0.0, 0.0, 0.0))
    # 支撑孢子球串的苍白微茎 (细如丝线，引导孢子上升)
    strand(rig, "connecting_stalks", "stalk_main", (0.0, 1.8, 0.0), (0.1, 5.6, 0.1), 0.26, "stalk_pale")
    strand(rig, "connecting_stalks", "stalk_branch_l", (0.1, 5.6, 0.1), (-1.4, 7.8, -0.6), 0.20, "stalk_pale")
    strand(rig, "connecting_stalks", "stalk_branch_r", (0.1, 5.6, 0.1), (1.5, 8.4, 0.8), 0.20, "stalk_pale")


def part_spore_chain(rig):
    rig.bone("spore_chain", (0.0, 0.0, 0.0))
    # 上方 4 个灰白孢子小球悬浮成串，有大有小（#c8c8c8 / #e1e1e1）
    # 1. 主孢子大球 (居中偏上，直径 2.4px，y: 6.2)
    pad(rig, "spore_chain", "spore_main_core", (0.1, 5.8, 0.1), (2.2, 2.2, 2.2), "spore_ball")
    pad(rig, "spore_chain", "spore_main_halo", (0.1, 5.6, 0.1), (1.6, 2.6, 1.6), "spore_halo")

    # 2. 顶端小球 (高位，直径 1.6px，y: 9.2)
    pad(rig, "spore_chain", "spore_top_ball", (0.3, 8.8, 0.2), (1.6, 1.6, 1.6), "spore_ball")

    # 3. 左侧悬浮球 (中高位，直径 1.8px，y: 7.6)
    pad(rig, "spore_chain", "spore_left_ball", (-1.6, 7.4, -0.7), (1.8, 1.8, 1.8), "spore_ball")

    # 4. 右侧悬浮球 (中位，直径 1.5px，y: 8.2)
    pad(rig, "spore_chain", "spore_right_ball", (1.6, 8.0, 0.9), (1.5, 1.5, 1.5), "spore_ball")


def build():
    return build_rig(MATS, (part_crystal_leaves, part_connecting_stalks, part_spore_chain))


GATES = PlantGates("浮尘草 / fu_chen_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("FuChenCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
