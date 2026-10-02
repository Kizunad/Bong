"""浮尘草：看似无害，苍白纤细直茎顶着毛茸茸的膨大浮尘孢子球，四散飘零微尘。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "dusty_mound": (48, 50, 52),
    "pale_stem": (55, 68, 65),
    "spore_core": (175, 185, 180),
    "fluff_spore": (215, 225, 220),
    "toxic_mote": (110, 168, 155),
}


def part_soil_base(rig):
    rig.bone("soil_base", (0.0, 0.0, 0.0))
    # 灰白沙土台与基叶
    pad(rig, "soil_base", "soil_center", (0.0, 0.12, 0.0), (3.8, 0.65, 3.8), "dusty_mound")
    pad(rig, "soil_base", "soil_leaf_f", (0.0, 0.45, 1.4), (1.8, 0.28, 1.6), "pale_stem")
    pad(rig, "soil_base", "soil_leaf_b", (0.0, 0.45, -1.4), (1.8, 0.28, 1.6), "pale_stem")


def part_slender_stem(rig):
    rig.bone("slender_stem", (0.0, 0.0, 0.0))
    # 极细长直立轻微拱弯的灰绿草茎 (y: 0.50 -> 8.20)
    strand(rig, "slender_stem", "stem_low", (0.0, 0.50, 0.0), (0.1, 4.20, 0.1), 0.28, "pale_stem")
    strand(rig, "slender_stem", "stem_high", (0.1, 4.20, 0.1), (0.05, 8.20, -0.05), 0.24, "pale_stem")


def part_spore_head(rig):
    rig.bone("spore_head", (0.0, 0.0, 0.0))
    # 顶端膨大的毛茸球形孢子囊 (y: 8.20 -> 11.80, 直径约 3.4px)
    yc = 10.0
    pad(rig, "spore_head", "spore_core_cube", (0.05, yc - 1.1, -0.05), (2.8, 2.2, 2.8), "spore_core")
    # 交叉蓬松绒毛外壳 (fluff cross)
    pad(rig, "spore_head", "spore_fluff_x", (0.05, yc - 0.8, -0.05), (3.5, 1.6, 2.2), "fluff_spore")
    pad(rig, "spore_head", "spore_fluff_z", (0.05, yc - 0.8, -0.05), (2.2, 1.6, 3.5), "fluff_spore")
    pad(rig, "spore_head", "spore_fluff_y", (0.05, yc - 1.5, -0.05), (2.2, 3.0, 2.2), "fluff_spore")


def part_dust_motes(rig):
    rig.bone("dust_motes", (0.0, 0.0, 0.0))
    # 四周漂浮飘散的微粒毒孢子 (motes, y: 9.0..13.2)
    motes = (
        ("mote_top_l", (-0.95, 12.2, 0.4), (0.35, 0.35, 0.35), "toxic_mote"),
        ("mote_top_r", (1.1, 12.5, -0.3), (0.32, 0.32, 0.32), "fluff_spore"),
        ("mote_side_r", (1.95, 10.2, 0.8), (0.30, 0.30, 0.30), "toxic_mote"),
        ("mote_side_l", (-1.85, 9.8, -0.9), (0.32, 0.32, 0.32), "fluff_spore"),
        ("mote_front", (0.2, 11.2, 1.95), (0.28, 0.28, 0.28), "toxic_mote"),
    )
    for name, pos, size, mat in motes:
        pad(rig, "dust_motes", name, pos, size, mat)


def build():
    return build_rig(MATS, (part_soil_base, part_slender_stem, part_spore_head, part_dust_motes))


GATES = PlantGates("浮尘草 / fu_chen_cao")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("FuChenCao", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
