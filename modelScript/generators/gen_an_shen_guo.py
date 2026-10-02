"""安神果：生于青云残峰梯田的淡黄色桃形果（#e1c373 / #f8e8a5），带清晰桃棱中凹，托在梯田叶座上。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "terrace_soil": (48, 42, 34),       # 梯田泥石台
    "terrace_leaf": (58, 72, 45),       # 梯田基叶
    "peach_yellow": (225, 195, 115),    # #e1c373 淡黄桃果
    "peach_highlight": (248, 232, 165), # #f8e8a5 桃尖亮黄
    "peach_groove": (175, 138, 75),     # 桃身脊棱
}


def part_terrace_base(rig):
    rig.bone("terrace_base", (0.0, 0.0, 0.0))
    # 底部青云残峰梯田碎石泥土座 (高 2.2px，宽 6.4px)
    pad(rig, "terrace_base", "base_terrace_slab", (0.0, 0.12, 0.0), (6.2, 1.8, 6.2), "terrace_soil")
    pad(rig, "terrace_base", "base_stalk_collar", (0.0, 1.5, 0.0), (2.8, 1.2, 2.8), "terrace_leaf")


def part_support_leaves(rig):
    rig.bone("support_leaves", (0.0, 0.0, 0.0))
    # 底部托着桃果的 4 片宽阔承托绿叶（四方外展拱托，展宽 9.2px）
    leaves = (
        ("leaf_support_f", (0.0, 1.6, 0.8), (0.0, 2.8, 3.8), 0.46),
        ("leaf_support_b", (0.0, 1.6, -0.8), (0.0, 2.8, -3.8), 0.46),
        ("leaf_support_l", (-0.8, 1.6, 0.0), (-3.8, 2.8, 0.0), 0.46),
        ("leaf_support_r", (0.8, 1.6, 0.0), (3.8, 2.8, 0.0), 0.46),
    )
    for name, start, end, r in leaves:
        strand(rig, "support_leaves", name, start, end, r, "terrace_leaf")


def part_peach_fruit(rig):
    rig.bone("peach_fruit", (0.0, 0.0, 0.0))
    # 饱满淡黄色桃形果实（y: 2.4 -> 9.4，下部圆润膨大、上部逐渐收尖）
    # 1. 桃身左右两瓣饱满果肉 (形成典型桃心外轮廓)
    pad(rig, "peach_fruit", "peach_lobe_l", (-1.3, 2.6, 0.0), (3.2, 4.4, 4.8), "peach_yellow")
    pad(rig, "peach_fruit", "peach_lobe_r", (1.3, 2.6, 0.0), (3.2, 4.4, 4.8), "peach_yellow")
    # 2. 桃腹前后外凸层
    pad(rig, "peach_fruit", "peach_belly_f", (0.0, 3.0, 1.2), (4.4, 3.6, 2.8), "peach_yellow")
    pad(rig, "peach_fruit", "peach_belly_b", (0.0, 3.0, -1.2), (4.4, 3.6, 2.8), "peach_yellow")
    # 3. 桃肩部聚拢层 (y: 6.8 -> 8.2)
    pad(rig, "peach_fruit", "peach_shoulder", (0.0, 6.8, 0.0), (3.6, 1.6, 3.6), "peach_yellow")
    # 4. 桃尖微翘顶喙 (y: 8.2 -> 9.6)
    pad(rig, "peach_fruit", "peach_apex_point", (0.0, 8.2, 0.2), (1.8, 1.4, 1.8), "peach_highlight")


def part_peach_groove(rig):
    rig.bone("peach_groove", (0.0, 0.0, 0.0))
    # 安神果标志性特征：桃身正面的中凹棱线（有棱！从桃底一直贯通至桃尖）
    strand(rig, "peach_groove", "groove_front_main", (0.0, 2.8, 2.5), (0.0, 7.8, 2.0), 0.24, "peach_groove")
    strand(rig, "peach_groove", "groove_front_top", (0.0, 7.8, 2.0), (0.0, 8.8, 1.1), 0.20, "peach_groove")
    pad(rig, "peach_groove", "groove_dent_bottom", (0.0, 2.6, 1.8), (0.8, 0.8, 1.4), "peach_groove")


def build():
    return build_rig(MATS, (part_terrace_base, part_support_leaves, part_peach_fruit, part_peach_groove))


GATES = PlantGates("安神果 / an_shen_guo")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("AnShenGuo", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
