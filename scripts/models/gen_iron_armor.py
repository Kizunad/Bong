#!/usr/bin/env python3
"""生成粗铁甲四件 bbmodel、64x64 UV 贴图与真实三视图预览。"""

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
        "iron_helmet",
        "粗铁盔",
        (
            c("HEAD", "crown_top", (-4.4, 31.4, -4.4), (8.8, 1.0, 8.8)),
            c("HEAD", "brow_plate", (-4.4, 28.3, -4.7), (8.8, 3.3, 0.9)),
            c("HEAD", "back_plate", (-4.4, 24.2, 3.8), (8.8, 7.2, 0.9)),
            c("HEAD", "left_temple", (-4.7, 24.2, -3.9), (0.9, 7.2, 7.7)),
            c("HEAD", "right_temple", (3.8, 24.2, -3.9), (0.9, 7.2, 7.7)),
            c("HEAD", "left_cheek", (-4.75, 24.0, -4.35), (1.4, 4.5, 2.0), (32, 0)),
            c("HEAD", "right_cheek", (3.35, 24.0, -4.35), (1.4, 4.5, 2.0), (32, 0)),
            c("HEAD", "nose_guard", (-0.45, 24.2, -4.95), (0.9, 4.4, 0.7), (48, 0)),
            c("HEAD", "crown_ridge", (-0.5, 31.8, -4.0), (1.0, 1.2, 8.0), (32, 0)),
            c("HEAD", "left_brow_rivet", (-3.2, 29.3, -4.9), (0.6, 0.6, 0.35), (48, 0)),
            c("HEAD", "right_brow_rivet", (2.6, 29.3, -4.9), (0.6, 0.6, 0.35), (48, 0)),
            c("HEAD", "left_temple_rivet", (-4.9, 29.3, -0.3), (0.35, 0.6, 0.6), (48, 0)),
            c("HEAD", "right_temple_rivet", (4.55, 29.3, -0.3), (0.35, 0.6, 0.6), (48, 0)),
        ),
    )


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "iron_chestplate",
        "粗铁胸甲",
        (
            c("BODY", "upper_front_plate", (-4.5, 17.8, -2.8), (9.0, 6.2, 0.9)),
            c("BODY", "lower_front_plate", (-3.9, 12.8, -2.7), (7.8, 5.3, 0.8)),
            c("BODY", "back_plate", (-4.3, 13.0, 1.9), (8.6, 11.0, 0.9)),
            c("BODY", "left_side_rail", (-4.7, 13.5, -2.0), (0.8, 10.0, 4.0)),
            c("BODY", "right_side_rail", (3.9, 13.5, -2.0), (0.8, 10.0, 4.0)),
            c("BODY", "left_collar", (-4.4, 22.5, -3.0), (3.5, 1.3, 1.0), (32, 0)),
            c("BODY", "right_collar", (0.9, 22.5, -3.0), (3.5, 1.3, 1.0), (32, 0)),
            c("BODY", "center_seam", (-0.35, 13.0, -3.05), (0.7, 9.5, 0.45), (32, 0)),
            c("BODY", "waist_band", (-4.1, 12.3, -2.9), (8.2, 1.0, 5.8), (32, 0)),
            c("BODY", "left_shoulder", (-6.0, 21.0, -2.7), (2.0, 2.8, 5.4)),
            c("BODY", "right_shoulder", (4.0, 21.0, -2.7), (2.0, 2.8, 5.4)),
            c("BODY", "left_front_strap", (-3.6, 18.0, -3.05), (0.8, 4.5, 0.35), (32, 0)),
            c("BODY", "right_front_strap", (2.8, 18.0, -3.05), (0.8, 4.5, 0.35), (32, 0)),
            c("BODY", "upper_left_rivet", (-3.5, 21.0, -3.15), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "upper_right_rivet", (2.85, 21.0, -3.15), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "lower_left_rivet", (-2.8, 14.0, -3.0), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "lower_right_rivet", (2.15, 14.0, -3.0), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "center_boss", (-0.55, 18.2, -3.3), (1.1, 1.1, 0.5), (48, 0)),
        ),
    )


def _leg_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        c(mount, f"{prefix}_thigh_plate", (-2.3, 5.0, -2.7), (4.6, 7.2, 0.8)),
        c(mount, f"{prefix}_hip_fauld", (-2.5, 10.7, -2.9), (5.0, 1.6, 1.0)),
        c(mount, f"{prefix}_knee_cap", (-2.4, 3.0, -3.0), (4.8, 2.3, 1.2)),
        c(mount, f"{prefix}_outer_rail", (outer_x, 4.5, -2.3), (0.8, 7.3, 4.6), (32, 0)),
        c(mount, f"{prefix}_rear_strap", (-2.2, 5.6, 2.0), (4.4, 0.8, 0.55), (32, 0)),
        c(mount, f"{prefix}_knee_rivet", (-0.35, 3.7, -3.25), (0.7, 0.7, 0.35), (48, 0)),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "iron_leggings",
        "粗铁腿甲",
        _leg_cubes("LEFT_LEG", 1.7) + _leg_cubes("RIGHT_LEG", -2.5),
    )


def _boot_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        c(mount, f"{prefix}_shin_plate", (-2.4, 1.8, -2.8), (4.8, 4.2, 0.9)),
        c(mount, f"{prefix}_toe_cap", (-2.5, -0.1, -3.2), (5.0, 2.2, 1.5)),
        c(mount, f"{prefix}_outer_rail", (outer_x, 0.4, -2.5), (0.8, 4.8, 5.0), (32, 0)),
        c(mount, f"{prefix}_ankle_band", (-2.4, 3.8, -2.9), (4.8, 1.0, 5.8), (32, 0)),
        c(mount, f"{prefix}_toe_rivet", (-0.35, 0.8, -3.45), (0.7, 0.7, 0.35), (48, 0)),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "iron_boots",
        "粗铁靴",
        _boot_cubes("LEFT_FOOT", 1.7) + _boot_cubes("RIGHT_FOOT", -2.5),
    )


def parts() -> tuple[ArmorPart, ...]:
    return part_helmet(), part_chestplate(), part_leggings(), part_boots()


def make_texture() -> Image.Image:
    rng = random.Random(0x1A0A)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (82, 82, 78))
    pixels = image.load()
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                base = (82, 82, 78)
            elif x < 48 and y < 32:
                base = (112, 65, 42)
            elif y < 32:
                base = (174, 168, 144)
            elif x < 32:
                base = (58, 39, 30)
            else:
                base = (38, 39, 38)
            jitter = rng.randint(-10, 10)
            pixels[x, y] = tuple(max(0, min(255, channel + jitter)) for channel in base)

    draw = ImageDraw.Draw(image)
    for x, y, length in ((4, 5, 9), (18, 11, 7), (7, 24, 13), (35, 6, 8), (36, 20, 10)):
        draw.line((x, y, x + length, y + 1), fill=(45, 46, 44), width=1)
    for x, y in ((51, 5), (58, 12), (52, 22), (60, 27)):
        draw.rectangle((x, y, x + 2, y + 2), fill=(205, 198, 168))
        draw.point((x, y), fill=(236, 230, 202))
    return image


def generate(render_previews: bool = True) -> dict[str, Path]:
    return write_material_assets(
        "iron",
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
