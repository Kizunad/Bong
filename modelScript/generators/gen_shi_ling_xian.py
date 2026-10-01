"""噬灵藓：负灵域的黑藓垫与向上吸附的短刺。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, leaf, pad, strand, write_model  # noqa: E402

MATS = {
    "null_bed": (19, 24, 22),
    "null_moss": (26, 42, 37),
    "siphon_black": (12, 17, 16),
    "wet_teal": (47, 85, 74),
    "dead_tip": (70, 67, 48),
}


def part_null_bed(rig):
    rig.bone("null_bed", (0.0, 0.0, 0.0))
    for name, x, z, w, d in (
        ("null_center", 0.0, 0.0, 5.8, 4.9),
        ("null_l", -2.55, 0.2, 1.35, 2.5),
        ("null_r", 2.48, -0.2, 1.3, 2.7),
        ("null_front", 0.0, 2.2, 4.4, 1.0),
    ):
        pad(rig, "null_bed", name, (x, 0.14, z), (w, 0.36, d), "null_bed")


def part_siphon_tufts(rig):
    rig.bone("siphon_tufts", (0.0, 0.0, 0.0))
    for suffix, x, z in (("c", 0.0, 0.0), ("l", -1.9, 0.3), ("r", 1.9, -0.25)):
        for index, dx in enumerate((-0.52, 0.0, 0.52)):
            leaf(
                rig,
                "siphon_tufts",
                f"tuft_{suffix}_{index}",
                (x + dx, 0.47, z + (index - 1) * 0.12),
                0.64,
                0.9,
                0.34,
                "null_moss",
                tilt=(0.0, (index - 1) * 9.0, (index - 1) * 8.0),
            )


def part_siphon_spines(rig):
    rig.bone("siphon_spines", (0.0, 0.0, 0.0))
    for side, x in (("l", -1.85), ("r", 1.85)):
        for index, z in enumerate((-0.9, 0.0, 0.9)):
            strand(rig, "siphon_spines", f"siphon_{side}_{index}", (x, 0.38, z), (x * 0.84, 1.28, z + 0.12), 0.14, "siphon_black")
    strand(rig, "siphon_spines", "siphon_center", (0.0, 0.4, 0.0), (0.0, 1.42, 0.0), 0.17, "siphon_black")


def part_wet_tips(rig):
    rig.bone("wet_tips", (0.0, 0.0, 0.0))
    for name, x, z in (("wet_l", -2.0, -0.75), ("wet_r", 2.0, 0.75), ("wet_front", 0.0, 2.02)):
        pad(rig, "wet_tips", name, (x, 0.72, z), (0.34, 0.26, 0.48), "wet_teal")
        pad(rig, "wet_tips", f"{name}_dead", (x, 0.97, z), (0.22, 0.24, 0.3), "dead_tip")


def build():
    return build_rig(MATS, (part_null_bed, part_siphon_tufts, part_siphon_spines, part_wet_tips))


GATES = PlantGates("噬灵藓 / shi_ling_xian")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("ShiLingXian", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
