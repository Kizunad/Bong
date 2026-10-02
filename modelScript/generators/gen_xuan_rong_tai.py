"""玄绒苔：漆黑隆起绒垫中央斜插冰蓝晶体。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "velvet_black": (20, 20, 22),
    "velvet_mid": (35, 35, 39),
    "velvet_edge": (49, 47, 53),
    "ice_crystal": (159, 216, 255),
    "blue_glow": (74, 184, 255),
}


def part_velvet_pad(rig):
    """宽扁圆饼状漆黑绒垫 (Round 3 小修：改宽扁圆饼)。"""
    rig.bone("velvet_pad", (0.0, 0.0, 0.0))
    # 采用圆饼交叉阶梯构建宽扁圆饼垫（总宽 8.2px，总高约 2.5px，形态扁圆）
    for name, x, z, w, h, d, y, mat in (
        # 底层宽圆底 (y: 0.12 -> 0.87, 展宽 8.2px)
        ("pad_floor_cross_x", 0.0, 0.0, 8.2, 0.75, 5.8, 0.12, "velvet_edge"),
        ("pad_floor_cross_z", 0.0, 0.0, 5.8, 0.75, 8.2, 0.12, "velvet_edge"),
        ("pad_floor_center", 0.0, 0.0, 7.2, 0.85, 7.2, 0.15, "velvet_black"),
        # 中层圆饼腰身 (y: 0.85 -> 1.80)
        ("pad_mid_cross_x", 0.0, 0.0, 7.6, 0.95, 5.2, 0.85, "velvet_mid"),
        ("pad_mid_cross_z", 0.0, 0.0, 5.2, 0.95, 7.6, 0.85, "velvet_mid"),
        ("pad_mid_center", 0.0, 0.0, 6.4, 0.95, 6.4, 0.85, "velvet_mid"),
        # 顶层平坦圆饼面 (y: 1.75 -> 2.50)
        ("pad_top_cross_x", 0.0, 0.0, 6.2, 0.75, 4.4, 1.75, "velvet_black"),
        ("pad_top_cross_z", 0.0, 0.0, 4.4, 0.75, 6.2, 1.75, "velvet_black"),
        ("pad_top_center", 0.0, 0.0, 5.2, 0.75, 5.2, 1.75, "velvet_black"),
    ):
        pad(rig, "velvet_pad", name, (x, y, z), (w, h, d), mat)


def part_velvet_tufts(rig):
    rig.bone("velvet_tufts", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("tuft_front", (0.0, 1.8, 1.8), (0.0, 2.8, 2.4)),
        ("tuft_left", (-1.8, 1.8, 0.0), (-2.4, 2.8, 0.0)),
        ("tuft_right", (1.8, 1.8, 0.0), (2.4, 2.8, 0.0)),
        ("tuft_back", (0.0, 1.8, -1.8), (0.0, 2.8, -2.4)),
    ):
        strand(rig, "velvet_tufts", name, start, end, 0.25, "velvet_black")


def part_ice_crystals(rig):
    rig.bone("ice_crystals", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("crystal_center", (0.0, 2.2, 0.0), (-0.45, 7.2, 0.25)),
        ("crystal_left", (-0.55, 2.2, -0.1), (-1.65, 6.2, -0.4)),
        ("crystal_right", (0.55, 2.2, 0.05), (1.75, 6.0, 0.15)),
        ("crystal_back", (0.1, 2.2, -0.6), (0.45, 5.8, -1.75)),
    ):
        strand(rig, "ice_crystals", name, start, end, 0.34, "ice_crystal")


def part_blue_cracks(rig):
    rig.bone("blue_cracks", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("glow_center", (-0.05, 2.4, 0.15), (-0.35, 5.8, 0.3)),
        ("glow_left", (-0.7, 2.3, -0.05), (-1.4, 4.9, -0.3)),
        ("glow_right", (0.6, 2.3, 0.25), (1.55, 4.8, 0.35)),
    ):
        strand(rig, "blue_cracks", name, start, end, 0.22, "blue_glow")


def build():
    return build_rig(MATS, (part_velvet_pad, part_velvet_tufts, part_ice_crystals, part_blue_cracks))


GATES = PlantGates("玄绒苔 / xuan_rong_tai")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("XuanRongTai", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
