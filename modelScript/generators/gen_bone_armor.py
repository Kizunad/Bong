#!/usr/bin/env python3
"""生成兽骨拼扎甲四件 bbmodel、64x64 UV 贴图与真实三视图预览。"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

from PIL import Image, ImageDraw

# --- modelScript 路径引导：共用底座在 core/ ---
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, TEXTURE_SIZE, write_material_assets

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
TEXTURE_ROOT = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
PREVIEW_ROOT = Path(__file__).resolve().parents[1] / "out"


def c(mount, name, origin, size, uv=(0, 0)) -> Cube:
    return Cube(mount, name, origin, size, uv)


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "bone_helmet",
        "BONE HELMET",
        (
            c("HEAD", "left_brow_bridge", (-3.7, 28.7, -4.75), (3.2, 1.0, 0.75)),
            c("HEAD", "right_brow_bridge", (0.4, 28.9, -4.75), (3.0, 0.9, 0.75), (32, 0)),
            c("HEAD", "forehead_keel", (-1.05, 29.6, -4.7), (2.1, 2.5, 0.65)),
            c("HEAD", "left_temple_rail", (-4.45, 27.1, -3.5), (0.85, 4.9, 2.65)),
            c("HEAD", "left_rear_rail", (-4.4, 30.45, -0.8), (0.8, 0.8, 4.7), (32, 0)),
            c("HEAD", "right_temple_rail", (3.6, 27.6, -3.6), (0.85, 4.3, 2.55), (32, 0)),
            c("HEAD", "right_rear_rail", (3.65, 30.7, -0.95), (0.75, 0.75, 4.85)),
            c("HEAD", "rear_crossbar", (-3.6, 30.4, 3.5), (7.1, 0.85, 0.75)),
            c("HEAD", "left_cheek", (-4.55, 24.9, -4.25), (1.0, 3.8, 1.3), (32, 0)),
            c("HEAD", "right_cheek", (3.55, 24.5, -4.25), (1.0, 4.2, 1.3), (32, 0)),
            c("HEAD", "nose_bone", (-0.35, 25.0, -4.9), (0.7, 3.7, 0.55)),
            c("HEAD", "left_jaw_tusk", (-4.15, 23.7, -4.35), (0.75, 2.2, 0.75), (32, 0)),
            c("HEAD", "left_jaw_tip", (-4.05, 23.15, -4.25), (0.55, 0.8, 0.55)),
            c("HEAD", "right_jaw_tusk", (3.4, 24.0, -4.35), (0.75, 1.9, 0.75), (32, 0)),
            c("HEAD", "left_horn_base", (-3.0, 31.5, -1.0), (1.1, 1.8, 1.1)),
            c("HEAD", "left_horn_mid", (-3.35, 33.0, -0.85), (0.85, 1.6, 0.85)),
            c("HEAD", "left_horn_tip", (-3.65, 34.25, -0.7), (0.55, 1.15, 0.55), (32, 0)),
            c("HEAD", "right_horn_base", (2.0, 31.5, -1.0), (1.0, 1.45, 1.0)),
            c("HEAD", "right_horn_tip", (2.25, 32.65, -0.85), (0.65, 1.0, 0.65), (32, 0)),
            c("HEAD", "rear_rope", (-3.6, 27.1, 4.0), (7.2, 0.55, 0.45), (0, 32)),
            c("HEAD", "left_temple_knot", (-4.75, 27.0, -0.2), (0.4, 0.8, 0.8), (0, 32)),
        ),
    )


def part_chestplate() -> ArmorPart:
    cubes = [
        c("BODY", "sternum_upper", (-0.7, 19.0, -3.0), (1.4, 4.8, 0.75)),
        c("BODY", "sternum_lower", (-0.55, 13.0, -3.0), (1.1, 5.7, 0.75), (32, 0)),
        c("BODY", "sternum_boss", (-1.0, 18.2, -3.25), (2.0, 1.3, 0.45), (32, 0)),
        c("BODY", "left_scapula", (-5.3, 21.4, -2.7), (1.8, 1.8, 1.0), (32, 0)),
        c("BODY", "right_scapula", (3.5, 21.8, -2.7), (2.0, 1.6, 1.0)),
        c("BODY", "left_scapula_prong", (-6.0, 22.2, -1.3), (1.2, 0.9, 3.2), (32, 0)),
        c("BODY", "right_scapula_prong", (5.1, 22.0, -0.8), (1.0, 0.8, 2.6), (32, 0)),
        c("BODY", "left_collar_bone", (-4.1, 22.8, -3.0), (3.2, 0.7, 0.75)),
        c("BODY", "right_collar_bone", (0.9, 22.6, -3.0), (3.5, 0.75, 0.75)),
        c("BODY", "back_spine_upper", (-0.6, 18.6, 2.05), (1.2, 5.2, 0.7), (32, 0)),
        c("BODY", "back_spine_lower", (-0.45, 13.0, 2.05), (0.9, 5.3, 0.7)),
    ]
    left_front_ribs = ((21.0, 3.9), (18.8, 3.6), (16.5, 3.2), (14.2, 2.7))
    right_front_ribs = ((21.2, 3.8), (19.0, 3.3), (16.7, 2.9), (14.5, 2.1))
    for index, (y, span) in enumerate(left_front_ribs):
        cubes.append(c("BODY", f"left_front_rib_{index}", (-span - 0.45, y, -3.0), (span, 0.65, 0.7)))
    for index, (y, span) in enumerate(right_front_ribs):
        cubes.append(c("BODY", f"right_front_rib_{index}", (0.45, y, -3.0), (span, 0.65, 0.7), (32, 0)))
    left_back_ribs = ((20.6, 3.7), (17.5, 3.25), (14.4, 2.8))
    right_back_ribs = ((20.4, 3.5), (17.7, 3.0), (14.7, 2.35))
    for index, (y, span) in enumerate(left_back_ribs):
        cubes.append(c("BODY", f"left_back_rib_{index}", (-span - 0.45, y, 2.0), (span, 0.65, 0.7)))
    for index, (y, span) in enumerate(right_back_ribs):
        cubes.append(c("BODY", f"right_back_rib_{index}", (0.45, y, 2.0), (span, 0.65, 0.7), (32, 0)))
    cubes.extend(
        (
            c("BODY", "left_rope_strap", (-3.45, 18.0, -3.1), (0.5, 4.8, 0.3), (0, 32)),
            c("BODY", "right_rope_strap", (2.95, 18.6, -3.1), (0.5, 3.7, 0.3), (0, 32)),
            c("BODY", "waist_rope_front", (-4.1, 12.55, -2.95), (8.2, 0.55, 0.45), (0, 32)),
            c("BODY", "waist_rope_back", (-4.0, 12.55, 2.5), (8.0, 0.55, 0.45), (0, 32)),
            c("BODY", "waist_rope_left", (-4.1, 12.55, -2.5), (0.45, 0.55, 5.0), (0, 32)),
            c("BODY", "waist_rope_right", (3.65, 12.55, -2.5), (0.45, 0.55, 5.0), (0, 32)),
            c("BODY", "broken_right_rib_tip", (2.5, 14.15, -3.0), (1.35, 0.5, 0.65), (32, 0)),
            c("BODY", "broken_rib_binding", (2.55, 13.95, -3.15), (0.35, 1.25, 0.3), (0, 32)),
            c("BODY", "upper_spine_knob", (-0.75, 21.0, 2.65), (1.5, 1.0, 0.45), (32, 0)),
            c("BODY", "middle_spine_knob", (-0.65, 17.4, 2.65), (1.3, 0.9, 0.45)),
            c("BODY", "lower_spine_knob", (-0.55, 14.1, 2.65), (1.1, 0.8, 0.45), (32, 0)),
            c("BODY", "waist_rope_knot", (3.65, 12.25, -3.15), (0.85, 1.0, 0.55), (0, 32)),
        )
    )
    return ArmorPart("bone_chestplate", "BONE CHESTPLATE", tuple(cubes))


def _rope_loop(mount: str, prefix: str, y: float, label: str) -> tuple[Cube, ...]:
    return (
        c(mount, f"{prefix}_{label}_rope_front", (-2.05, y, -3.0), (4.1, 0.45, 0.35), (0, 32)),
        c(mount, f"{prefix}_{label}_rope_back", (-2.05, y, 2.65), (4.1, 0.45, 0.35), (0, 32)),
        c(mount, f"{prefix}_{label}_rope_left", (-2.1, y, -2.65), (0.35, 0.45, 5.3), (0, 32)),
        c(mount, f"{prefix}_{label}_rope_right", (1.75, y, -2.65), (0.35, 0.45, 5.3), (0, 32)),
    )


def _leg_cubes(mount: str, outer_x: float, outer_span: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    bones = (
        c(mount, f"{prefix}_femur_splint", (-0.5, 5.1, -2.8), (1.0, 6.8, 0.75)),
        c(mount, f"{prefix}_outer_splint", (outer_x, 5.5, -2.55), (0.9, outer_span, 0.9), (32, 0)),
        c(mount, f"{prefix}_hip_inner", (-2.15, 10.75, -2.9), (1.65, 1.25, 0.8)),
        c(mount, f"{prefix}_hip_outer", (0.15, 10.9, -2.9), (2.0, 1.1, 0.8), (32, 0)),
        c(mount, f"{prefix}_knee_bone", (-1.2, 3.25, -3.0), (2.4, 1.65, 0.9)),
        c(mount, f"{prefix}_side_tooth", (outer_x, 3.9, -2.55), (0.65, 1.6, 1.7), (32, 0)),
    )
    return bones + _rope_loop(mount, prefix, 9.1, "upper") + _rope_loop(mount, prefix, 5.85, "lower")


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "bone_leggings",
        "BONE LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.2, 6.1)
        + (c("LEFT_LEG", "left_leg_knee_hook", (1.0, 3.55, -3.1), (0.7, 1.0, 0.6), (32, 0)),)
        + _leg_cubes("RIGHT_LEG", -2.1, 5.5)
        + (c("RIGHT_LEG", "right_leg_knee_chip", (-1.7, 3.5, -3.05), (0.55, 0.85, 0.55)),),
    )


def _boot_cubes(mount: str, outer_x: float, inner_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    bones = (
        c(mount, f"{prefix}_shin_splint", (-0.55, 1.8, -2.8), (1.1, 4.2, 0.8)),
        c(mount, f"{prefix}_inner_splint", (inner_x, 2.0, -2.6), (0.7, 3.7, 0.65), (32, 0)),
        c(mount, f"{prefix}_outer_splint", (outer_x, 2.2, -2.6), (0.7, 3.4, 0.65), (32, 0)),
        c(mount, f"{prefix}_ankle_front", (-2.0, 3.75, -2.9), (4.0, 0.8, 0.75)),
        c(mount, f"{prefix}_ankle_back", (-1.8, 3.8, 2.15), (3.6, 0.7, 0.65), (32, 0)),
        c(mount, f"{prefix}_center_claw", (-0.45, -0.1, -3.55), (0.9, 1.45, 1.75)),
        c(mount, f"{prefix}_outer_claw", (outer_x, 0.0, -3.4), (0.7, 1.25, 1.45), (32, 0)),
        c(mount, f"{prefix}_inner_claw", (inner_x, 0.15, -3.3), (0.6, 1.05, 1.3)),
        c(mount, f"{prefix}_heel_bone", (-1.7, 0.25, 2.0), (3.4, 1.8, 0.65), (32, 0)),
        c(mount, f"{prefix}_ankle_knot", (outer_x, 2.55, -3.15), (0.75, 0.8, 0.45), (0, 32)),
    )
    return bones + _rope_loop(mount, prefix, 2.75, "ankle")


def part_boots() -> ArmorPart:
    return ArmorPart(
        "bone_boots",
        "BONE BOOTS",
        _boot_cubes("LEFT_FOOT", 1.1, -1.9) + _boot_cubes("RIGHT_FOOT", -1.9, 1.1),
    )


def parts() -> tuple[ArmorPart, ...]:
    return part_helmet(), part_chestplate(), part_leggings(), part_boots()


def make_texture() -> Image.Image:
    rng = random.Random(0xB0AE)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (208, 200, 184))
    pixels = image.load()
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                base = (208, 200, 184)
            elif x < 48 and y < 32:
                base = (166, 150, 126)
            elif y < 32:
                base = (66, 60, 52)
            elif x < 32:
                base = (91, 62, 42)
            else:
                base = (119, 105, 86)
            value_jitter = rng.randint(-9, 9)
            warm_jitter = rng.randint(-3, 3)
            offsets = (value_jitter + warm_jitter, value_jitter, value_jitter - warm_jitter)
            pixels[x, y] = tuple(
                max(0, min(255, channel + offset)) for channel, offset in zip(base, offsets)
            )

    draw = ImageDraw.Draw(image)
    for points in (
        ((3, 4), (9, 9), (7, 16)),
        ((18, 3), (15, 11), (22, 18)),
        ((26, 5), (23, 13), (29, 20)),
        ((35, 5), (42, 11), (38, 23)),
    ):
        draw.line(points, fill=(105, 96, 82), width=1)
    for branch in (((9, 9), (13, 12)), ((15, 11), (12, 15)), ((42, 11), (45, 14))):
        draw.line(branch, fill=(91, 81, 68), width=1)
    for x, y in ((5, 22), (11, 3), (16, 25), (21, 7), (27, 27), (34, 18), (39, 4), (44, 26)):
        draw.point((x, y), fill=(126, 113, 94))
        if x + 1 < 48:
            draw.point((x + 1, y), fill=(226, 218, 199))
    for y in (36, 43, 50, 57):
        draw.line((0, y, 31, y), fill=(55, 38, 28), width=1)
        draw.line((0, y + 1, 31, y + 1), fill=(118, 79, 50), width=1)
    for x, y in ((4, 34), (13, 41), (23, 48), (8, 55), (27, 60)):
        draw.rectangle((x, y, x + 2, y + 1), fill=(149, 101, 61))
    return image


def generate(render_previews: bool = True) -> dict[str, Path]:
    return write_material_assets(
        "bone",
        parts(),
        make_texture(),
        LOCAL_MODELS,
        TEXTURE_ROOT,
        PREVIEW_ROOT,
        render_previews,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel/texture")
    args = parser.parse_args()
    outputs = generate(render_previews=not args.no_preview)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
