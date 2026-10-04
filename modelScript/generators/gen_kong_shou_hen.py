"""空兽痕：负灵域残灰里留下的兽骨状爪痕遗物。"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from plant_geo_common import PlantGates, build_rig, pad, strand, write_model  # noqa: E402

MATS = {
    "ash_bed": (55, 54, 50),
    "bone_dark": (73, 72, 68),
    "bone_pale": (190, 188, 170),
    "edge_high": (224, 216, 190),
    "socket_dark": (25, 32, 36),
    "ice_blue": (106, 216, 255),
    "ice_hot": (180, 244, 255),
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


def part_skull(rig):
    rig.bone("skull", (0.0, 0.0, 0.0))
    for name, x, z, w, h, d, y, mat in (
        ("skull_cranium", 0.0, 0.0, 5.4, 2.7, 2.5, 2.35, "bone_pale"),
        ("skull_brow", 0.0, 1.0, 5.65, 0.62, 1.45, 4.65, "edge_high"),
        ("skull_cheek_left", -1.7, 0.85, 1.45, 1.55, 1.5, 1.95, "bone_pale"),
        ("skull_cheek_right", 1.7, 0.85, 1.45, 1.55, 1.5, 1.95, "bone_pale"),
        ("skull_bridge", 0.0, 1.3, 1.35, 1.6, 0.8, 2.2, "bone_dark"),
        ("skull_muzzle", 0.0, 1.18, 2.75, 1.15, 1.45, 1.35, "bone_pale"),
        ("skull_jaw", 0.0, 0.82, 3.5, 0.65, 1.5, 0.78, "bone_dark"),
    ):
        pad(rig, "skull", name, (x, y, z), (w, h, d), mat)


def part_eye_sockets(rig):
    rig.bone("eye_sockets", (0.0, 0.0, 0.0))
    for side, x in (("left", -1.35), ("right", 1.35)):
        pad(rig, "eye_sockets", f"socket_{side}", (x, 3.45, 1.34), (1.05, 1.05, 0.24), "socket_dark")
        pad(rig, "eye_sockets", f"eye_{side}", (x, 3.7, 1.52), (0.48, 0.38, 0.28), "ice_blue")
        pad(rig, "eye_sockets", f"eye_hot_{side}", (x, 3.86, 1.66), (0.24, 0.24, 0.22), "ice_hot")


def part_teeth_ribs(rig):
    rig.bone("teeth_ribs", (0.0, 0.0, 0.0))
    for index, x in enumerate((-1.25, -0.42, 0.42, 1.25)):
        strand(rig, "teeth_ribs", f"tooth_{index}", (x, 1.42, 1.86), (x, 0.72, 1.86), 0.2, "edge_high")
    for name, start, end, radius in (
        ("rib_left_outer", (-2.35, 1.4, -0.7), (-3.35, 4.1, -0.6), 0.28),
        ("rib_left_inner", (-2.2, 1.6, -0.9), (-2.95, 4.55, -0.85), 0.24),
        ("rib_right_outer", (2.35, 1.4, -0.7), (3.35, 4.1, -0.6), 0.28),
        ("rib_right_inner", (2.2, 1.6, -0.9), (2.95, 4.55, -0.85), 0.24),
        ("spike_left", (-2.5, 2.5, -0.25), (-3.45, 3.4, 0.1), 0.2),
        ("spike_right", (2.5, 2.5, -0.25), (3.45, 3.4, 0.1), 0.2),
    ):
        strand(rig, "teeth_ribs", name, start, end, radius, "bone_dark")


def part_void_scars(rig):
    rig.bone("void_scars", (0.0, 0.0, 0.0))
    for name, start, end in (
        ("scar_main", (-0.25, 4.2, 1.58), (0.0, 2.65, 1.62)),
        ("scar_branch", (0.0, 3.2, 1.63), (0.95, 3.65, 1.63)),
        ("scar_claw", (0.0, 3.2, 1.63), (-0.95, 3.65, 1.63)),
    ):
        strand(rig, "void_scars", name, start, end, 0.2, "void_scar")


def build():
    return build_rig(MATS, (part_ash_bed, part_skull, part_eye_sockets, part_teeth_ribs, part_void_scars))


GATES = PlantGates("空兽痕 / kong_shou_hen")


def main():
    rig = build()
    GATES.report(rig)
    GATES.self_test(rig)
    path = write_model("KongShouHen", rig)
    print(f"生成作者稿：{path.relative_to(Path(__file__).resolve().parents[2])}")


if __name__ == "__main__":
    main()
