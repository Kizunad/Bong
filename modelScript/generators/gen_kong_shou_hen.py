"""空兽痕：负灵域残灰里留下的兽骨状爪痕遗物。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "ash_bed": (55, 54, 50),
    "bone_dark": (73, 72, 68),
    "bone_pale": (125, 120, 108),
    "edge_high": (177, 169, 148),
    "void_scar": (83, 43, 51),
}


def part_ash_bed(rig):
    rig.bone("ash_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("ash_floor", 0.0, 0.0, 7.4, 0.95, 5.8, 0.1, "ash_bed"),
        ("ash_left", -2.15, 0.25, 2.2, 1.7, 3.4, 0.45, "bone_dark"),
        ("ash_right", 2.15, -0.2, 2.1, 1.35, 3.0, 0.4, "ash_bed"),
        ("ash_back", 0.1, -1.8, 4.8, 1.35, 1.25, 0.5, "ash_bed"),
    ):
        pad(rig, "ash_bed", name, (x, y, z), (w, h, d), mat)


def part_bone_arc(rig):
    rig.bone("bone_arc", (0.0, 0.0, 0.0))
    segments = (
        ("arc_base", (-2.25, 1.2, 0.15), (0.0, 2.35, -0.05), 0.55, "bone_dark"),
        ("arc_main", (0.0, 2.35, -0.05), (2.45, 3.45, 0.2), 0.52, "bone_pale"),
        ("arc_tip", (2.45, 3.45, 0.2), (3.0, 4.65, 0.25), 0.34, "edge_high"),
        ("claw_left", (-0.4, 2.1, 0.0), (-1.75, 4.35, 0.75), 0.35, "bone_pale"),
        ("claw_mid", (0.25, 2.55, 0.0), (0.1, 5.45, 0.15), 0.34, "edge_high"),
        ("claw_right", (0.85, 2.8, 0.05), (1.7, 5.05, -0.55), 0.34, "bone_pale"),
    )
    for name, start, end, radius, mat in segments:
        strand(rig, "bone_arc", name, start, end, radius, mat)


def part_bone_fragments(rig):
    rig.bone("bone_fragments", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("fragment_front", -1.55, 1.9, 1.0, 0.65, 0.5, 1.05, "bone_pale"),
        ("fragment_left", -2.55, -0.85, 0.6, 0.45, 0.9, 1.1, "edge_high"),
        ("fragment_right", 2.5, 0.9, 0.55, 0.55, 0.75, 1.0, "bone_dark"),
    ):
        pad(rig, "bone_fragments", name, (x, y, z), (w, h, d), mat)


def part_void_scars(rig):
    rig.bone("void_scars", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("scar_main", (-1.65, 1.62, 0.3), (0.0, 2.7, 0.1)),
        ("scar_branch", (0.0, 2.72, 0.1), (1.5, 3.45, 0.2)),
        ("scar_claw", (0.15, 2.7, 0.1), (0.15, 4.4, 0.15)),
    ):
        strand(rig, "void_scars", name, start, end, 0.2, "void_scar")


def build():
    return build_rig(MATS, (part_ash_bed, part_bone_arc, part_bone_fragments, part_void_scars))


GATES = PlantGates("空兽痕 / kong_shou_hen")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("KongShouHen", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
