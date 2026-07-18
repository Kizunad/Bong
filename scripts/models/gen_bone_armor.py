#!/usr/bin/env python3
"""生成兽骨拼扎甲四件 bbmodel、64x64 UV 贴图与真实三视图预览。"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

from PIL import Image, ImageDraw

from armor_model_common import ArmorPart, Cube, TEXTURE_SIZE, write_material_assets

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = REPO / "local_models"
TEXTURE_ROOT = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
PREVIEW_ROOT = REPO / "scripts" / "models"


def c(mount, name, origin, size, uv=(0, 0)) -> Cube:
    return Cube(mount, name, origin, size, uv)


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "bone_helmet",
        "BONE HELMET",
        (
            c("HEAD", "brow_bridge", (-3.6, 28.8, -4.7), (7.2, 1.1, 0.8)),
            c("HEAD", "forehead_plate", (-2.5, 29.8, -4.65), (5.0, 2.0, 0.7), (32, 0)),
            c("HEAD", "left_crown_rail", (-4.5, 27.3, -3.8), (0.9, 4.8, 7.6)),
            c("HEAD", "right_crown_rail", (3.6, 27.3, -3.8), (0.9, 4.8, 7.6)),
            c("HEAD", "rear_crossbar", (-3.8, 30.4, 3.7), (7.6, 1.0, 0.8)),
            c("HEAD", "left_cheek", (-4.55, 24.6, -4.3), (1.0, 4.2, 1.4), (32, 0)),
            c("HEAD", "right_cheek", (3.55, 24.6, -4.3), (1.0, 4.2, 1.4), (32, 0)),
            c("HEAD", "nose_bone", (-0.4, 24.8, -4.9), (0.8, 4.0, 0.6)),
            c("HEAD", "left_jaw_tusk", (-4.2, 23.8, -4.4), (0.8, 2.4, 0.8), (32, 0)),
            c("HEAD", "right_jaw_tusk", (3.4, 23.8, -4.4), (0.8, 2.4, 0.8), (32, 0)),
            c("HEAD", "left_horn_base", (-3.2, 31.5, -1.0), (1.1, 2.4, 1.1)),
            c("HEAD", "left_horn_tip", (-3.0, 33.5, -0.8), (0.7, 1.8, 0.7), (32, 0)),
            c("HEAD", "right_horn_base", (2.1, 31.5, -1.0), (1.0, 1.8, 1.0)),
            c("HEAD", "right_horn_tip", (2.3, 33.0, -0.8), (0.6, 1.2, 0.6), (32, 0)),
            c("HEAD", "rear_rope", (-3.8, 27.0, 4.2), (7.6, 0.7, 0.5), (0, 32)),
            c("HEAD", "forehead_crack_patch", (-0.5, 30.2, -4.9), (1.0, 1.1, 0.3), (48, 0)),
        ),
    )


def part_chestplate() -> ArmorPart:
    cubes = [
        c("BODY", "sternum", (-0.65, 13.0, -3.0), (1.3, 10.8, 0.8)),
        c("BODY", "left_scapula", (-5.6, 21.5, -2.8), (2.0, 2.2, 1.0), (32, 0)),
        c("BODY", "right_scapula", (3.6, 21.5, -2.8), (2.0, 2.2, 1.0), (32, 0)),
        c("BODY", "left_collar_bone", (-4.3, 23.0, -3.0), (3.4, 0.7, 0.8)),
        c("BODY", "right_collar_bone", (0.9, 23.0, -3.0), (3.4, 0.7, 0.8)),
        c("BODY", "back_spine", (-0.55, 13.0, 2.1), (1.1, 10.8, 0.7), (32, 0)),
    ]
    for index, (y, span) in enumerate(((21.5, 4.0), (19.2, 3.7), (16.9, 3.4), (14.6, 3.1))):
        cubes.append(c("BODY", f"left_front_rib_{index}", (-span - 0.5, y, -3.0), (span, 0.75, 0.75)))
        cubes.append(c("BODY", f"right_front_rib_{index}", (0.5, y, -3.0), (span, 0.75, 0.75)))
    for index, (y, span) in enumerate(((20.5, 3.8), (17.5, 3.4), (14.5, 3.0))):
        cubes.append(c("BODY", f"left_back_rib_{index}", (-span - 0.45, y, 2.0), (span, 0.7, 0.7), (32, 0)))
        cubes.append(c("BODY", f"right_back_rib_{index}", (0.45, y, 2.0), (span, 0.7, 0.7), (32, 0)))
    cubes.extend(
        (
            c("BODY", "left_rope_strap", (-3.5, 18.0, -3.1), (0.6, 5.0, 0.35), (0, 32)),
            c("BODY", "right_rope_strap", (2.9, 18.0, -3.1), (0.6, 5.0, 0.35), (0, 32)),
            c("BODY", "waist_rope", (-4.2, 12.6, -2.8), (8.4, 0.7, 5.6), (0, 32)),
            c("BODY", "left_scapula_prong", (-6.2, 22.0, -0.4), (1.0, 1.0, 3.0), (32, 0)),
            c("BODY", "right_scapula_prong", (5.2, 22.0, -0.4), (1.0, 1.0, 3.0), (32, 0)),
        )
    )
    return ArmorPart("bone_chestplate", "BONE CHESTPLATE", tuple(cubes))


def _leg_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        c(mount, f"{prefix}_femur_splint", (-0.55, 5.0, -2.8), (1.1, 7.2, 0.8)),
        c(mount, f"{prefix}_outer_splint", (outer_x, 5.5, -2.5), (1.1, 6.4, 1.0), (32, 0)),
        c(mount, f"{prefix}_hip_plate", (-2.3, 10.7, -2.9), (4.6, 1.5, 0.9), (32, 0)),
        c(mount, f"{prefix}_knee_bone", (-1.8, 3.2, -3.0), (3.6, 1.8, 1.0)),
        c(mount, f"{prefix}_upper_rope", (-2.1, 9.0, -2.95), (4.2, 0.6, 5.9), (0, 32)),
        c(mount, f"{prefix}_lower_rope", (-2.1, 5.8, -2.95), (4.2, 0.6, 5.9), (0, 32)),
        c(mount, f"{prefix}_side_tooth", (outer_x, 4.2, -2.5), (0.7, 1.8, 1.8), (32, 0)),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "bone_leggings",
        "BONE LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.2) + _leg_cubes("RIGHT_LEG", -2.3),
    )


def _boot_cubes(mount: str, outer_x: float, inner_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        c(mount, f"{prefix}_shin_splint", (-0.6, 1.8, -2.8), (1.2, 4.2, 0.9)),
        c(mount, f"{prefix}_inner_splint", (inner_x, 2.0, -2.6), (0.8, 3.7, 0.7), (32, 0)),
        c(mount, f"{prefix}_outer_splint", (outer_x, 2.0, -2.6), (0.8, 3.7, 0.7), (32, 0)),
        c(mount, f"{prefix}_ankle_bone", (-2.2, 3.7, -2.9), (4.4, 1.0, 5.6)),
        c(mount, f"{prefix}_center_claw", (-0.5, -0.1, -3.5), (1.0, 1.5, 1.8)),
        c(mount, f"{prefix}_outer_claw", (outer_x, 0.0, -3.35), (0.8, 1.3, 1.5), (32, 0)),
        c(mount, f"{prefix}_heel_bone", (-1.8, 0.2, 2.0), (3.6, 2.0, 0.7), (32, 0)),
        c(mount, f"{prefix}_ankle_rope", (-2.1, 2.7, -3.0), (4.2, 0.6, 6.0), (0, 32)),
    )


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
    for points in (((3, 4), (9, 9), (7, 16)), ((18, 3), (15, 11), (22, 18)), ((35, 5), (42, 11), (38, 23))):
        draw.line(points, fill=(105, 96, 82), width=1)
    for y in (36, 43, 50, 57):
        draw.line((0, y, 31, y), fill=(55, 38, 28), width=1)
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
