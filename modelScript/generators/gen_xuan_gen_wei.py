"""悬根薇：浮岛底面垂落的翠青藤薇与虹吸锐晶根尖 (Round 3 终轮)。"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import (  # noqa: E402
    PlantGates,
    build_rig,
    curved_vine_chain,
    pad,
    strand,
    write_model,
)

MATS = {
    "island_rock": (68, 78, 70),
    "rock_high": (104, 113, 94),
    "root_green": (42, 112, 62),
    "leaf_green": (88, 168, 78),
    "leaf_high": (142, 204, 98),
    "siphon_crystal": (106, 224, 106),  # 亮绿尖晶 #6ae06a
    "crystal_glint": (178, 246, 140),   # 晶体高光
}


def part_island_underside(rig):
    """浮岛底面顶部岩台（模型顶部挂点 y: 11.2..12.5）。"""
    rig.bone("island_underside", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("island_floor", 0.0, 0.0, 7.4, 0.9, 5.6, 11.4, "island_rock"),
        ("island_ledge_l", -2.4, 0.1, 2.2, 0.8, 3.2, 11.5, "rock_high"),
        ("island_ledge_r", 2.3, -0.2, 2.0, 0.75, 2.8, 11.5, "rock_high"),
        ("island_back", 0.0, -1.8, 4.8, 0.85, 1.4, 11.45, "island_rock"),
    ):
        pad(rig, "island_underside", name, (x, y, z), (w, h, d), mat)


def part_hanging_roots(rig):
    """几根翠绿细藤在竖直方向绕成中空圆环（像倒挂的花环/圆圈，内部中空）。"""
    rig.bone("hanging_roots", (0.0, 0.0, 0.0))
    for root_index in range(5):
        phase = math.tau * root_index / 5.0
        points = []
        for step in range(16):
            t = step / 15.0
            angle = phase + math.tau * t
            # 竖直环面：x 与 y 构成大圆，z 仅做微小起伏
            rx = 3.6 + 0.32 * math.cos(3.0 * angle + phase)
            ry = 3.9 + 0.32 * math.sin(3.0 * angle + phase)
            rz = 0.55 * math.sin(2.0 * angle + phase)
            x = rx * math.cos(angle)
            y = 7.3 + ry * math.sin(angle)  # y: 3.1..11.5，顶部紧贴岩台，底部在 y~3.2
            z = rz
            points.append((x, y, z))
        curved_vine_chain(
            rig,
            "hanging_roots",
            f"root_loop_{root_index}",
            points,
            0.42,
            0.28,
            "root_green",
        )


def part_root_leaves(rig):
    """环身外缘侧生的小片翠绿藤叶。"""
    rig.bone("root_leaves", (0.0, 0.0, 0.0))
    leaves_data = (
        ("leaf_left_hi", (-3.6, 8.8, 0.4), (-4.4, 8.4, 0.8), "leaf_high"),
        ("leaf_left_mid", (-3.9, 6.8, -0.3), (-4.7, 6.2, -0.6), "leaf_green"),
        ("leaf_right_hi", (3.5, 8.6, -0.4), (4.3, 8.2, -0.8), "leaf_high"),
        ("leaf_right_mid", (3.8, 6.9, 0.3), (4.6, 6.3, 0.7), "leaf_green"),
        ("leaf_top_front", (0.2, 10.8, 1.2), (0.4, 10.3, 1.8), "leaf_green"),
    )
    for index, (tag, start, end, mat) in enumerate(leaves_data):
        strand(
            rig,
            "root_leaves",
            f"leaf_stem_{tag}",
            start,
            end,
            0.18,
            "leaf_green",
        )
        pad(
            rig,
            "root_leaves",
            f"leaf_blade_{tag}",
            end,
            (0.85, 0.28, 0.55),
            mat,
        )


def part_siphon_crystals(rig):
    """竖直圆环下端垂挂的 5 枚亮绿尖晶 #6ae06a，尖头朝下。"""
    rig.bone("siphon_crystals", (0.0, 0.0, 0.0))
    crystals_data = (
        ("crystal_center", (0.0, 3.4, 0.1), (0.0, 1.2, 0.1), (0.0, 0.6, 0.1), 0.32),
        ("crystal_left", (-1.6, 3.8, -0.3), (-1.6, 1.8, -0.3), (-1.6, 1.1, -0.3), 0.26),
        ("crystal_right", (1.6, 3.7, 0.25), (1.6, 1.7, 0.25), (1.6, 1.0, 0.25), 0.26),
        ("crystal_front", (-0.7, 3.5, 0.8), (-0.7, 2.0, 0.8), (-0.7, 1.4, 0.8), 0.22),
        ("crystal_back", (0.8, 3.5, -0.7), (0.8, 2.1, -0.7), (0.8, 1.5, -0.7), 0.22),
    )
    for name, start, mid, end, rad in crystals_data:
        # 上段主晶体
        strand(
            rig,
            "siphon_crystals",
            f"{name}_body",
            start,
            mid,
            rad,
            "siphon_crystal",
        )
        # 下段尖头朝下收尖锥体
        strand(
            rig,
            "siphon_crystals",
            f"{name}_tip",
            mid,
            end,
            rad * 0.55,
            "crystal_glint",
        )
        # 尖尖刺端点
        spire_w = max(rad * 0.9, 0.25)
        pad(
            rig,
            "siphon_crystals",
            f"{name}_spire",
            end,
            (spire_w, 0.45, spire_w),
            "crystal_glint",
        )


def build():
    return build_rig(
        MATS,
        (part_island_underside, part_hanging_roots, part_root_leaves, part_siphon_crystals),
    )


GATES = PlantGates("悬根薇 / xuan_gen_wei")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XuanGenWei", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
