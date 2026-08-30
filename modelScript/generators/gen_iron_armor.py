#!/usr/bin/env python3
"""生成粗铁甲四件 bbmodel、64x64 UV 贴图与真实三视图预览。"""

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
        "iron_helmet",
        "IRON HELMET",
        (
            c("HEAD", "crown_left", (-4.4, 31.4, -4.4), (4.1, 1.0, 8.8)),
            c("HEAD", "crown_right", (0.2, 31.4, -4.4), (4.2, 1.0, 7.8)),
            c("HEAD", "brow_plate", (-4.4, 29.1, -4.7), (8.8, 2.3, 0.9)),
            c("HEAD", "back_plate", (-4.4, 24.4, 3.8), (8.8, 7.0, 0.9)),
            c("HEAD", "left_temple", (-4.7, 25.0, -3.7), (0.9, 6.4, 7.5)),
            c("HEAD", "right_temple", (3.8, 25.0, -3.7), (0.9, 6.4, 7.5)),
            c("HEAD", "left_cheek", (-4.75, 24.3, -4.3), (1.2, 3.8, 1.6), (32, 0)),
            c("HEAD", "right_cheek", (3.55, 24.3, -4.3), (1.2, 3.8, 1.6), (32, 0)),
            c("HEAD", "nose_guard", (-0.4, 24.7, -4.95), (0.8, 4.0, 0.6), (48, 0)),
            c("HEAD", "crown_ridge", (-0.45, 31.9, -3.8), (0.9, 1.1, 7.6), (32, 0)),
            c("HEAD", "left_brow_rivet", (-3.2, 29.9, -4.9), (0.6, 0.6, 0.35), (48, 0)),
            c("HEAD", "right_brow_rivet", (2.6, 29.9, -4.9), (0.6, 0.6, 0.35), (48, 0)),
            c("HEAD", "left_temple_rivet", (-4.9, 29.5, -0.3), (0.35, 0.6, 0.6), (48, 0)),
            c("HEAD", "right_temple_rivet", (4.55, 29.5, -0.3), (0.35, 0.6, 0.6), (48, 0)),
            c("HEAD", "rear_neck_flare", (-3.8, 23.7, 3.9), (7.6, 0.9, 1.4), (32, 0)),
        ),
    )


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "iron_chestplate",
        "IRON CHESTPLATE",
        (
            c("BODY", "upper_front_left", (-4.25, 18.0, -2.75), (4.0, 6.0, 0.8)),
            c("BODY", "upper_front_right", (0.2, 18.4, -2.75), (4.05, 5.6, 0.8)),
            c("BODY", "lower_front_plate", (-3.55, 13.0, -2.65), (7.1, 5.2, 0.75)),
            c("BODY", "upper_back_plate", (-4.1, 18.0, 1.95), (8.2, 6.0, 0.8)),
            c("BODY", "lower_back_plate", (-3.5, 13.0, 1.95), (7.0, 5.1, 0.75)),
            c("BODY", "left_side_rail", (-4.55, 14.5, -1.7), (0.65, 8.7, 3.4)),
            c("BODY", "right_side_rail", (3.9, 14.5, -1.7), (0.65, 8.7, 3.4)),
            c("BODY", "left_collar", (-4.4, 22.5, -3.0), (3.5, 1.3, 1.0), (32, 0)),
            c("BODY", "right_collar", (0.9, 22.5, -3.0), (3.5, 1.3, 1.0), (32, 0)),
            c("BODY", "center_seam", (-0.35, 13.0, -3.05), (0.7, 9.5, 0.45), (32, 0)),
            c("BODY", "waist_band", (-3.8, 12.3, -2.75), (7.6, 1.0, 5.5), (32, 0)),
            c("BODY", "left_shoulder", (-5.8, 21.2, -2.55), (1.8, 2.5, 5.1)),
            c("BODY", "right_shoulder", (4.0, 21.2, -2.55), (1.8, 2.5, 5.1)),
            c("BODY", "left_shoulder_lip", (-6.15, 22.6, -2.7), (2.2, 0.7, 5.4), (32, 0)),
            c("BODY", "right_shoulder_lip", (3.95, 22.6, -2.7), (2.2, 0.7, 5.4), (32, 0)),
            c("BODY", "left_front_strap", (-3.6, 18.0, -3.05), (0.8, 4.5, 0.35), (32, 0)),
            c("BODY", "right_front_strap", (2.8, 18.0, -3.05), (0.8, 4.5, 0.35), (32, 0)),
            c("BODY", "upper_left_rivet", (-3.5, 21.0, -3.15), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "upper_right_rivet", (2.85, 21.0, -3.15), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "lower_left_rivet", (-2.8, 14.0, -3.0), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "lower_right_rivet", (2.15, 14.0, -3.0), (0.65, 0.65, 0.35), (48, 0)),
            c("BODY", "center_boss", (-0.55, 18.2, -3.3), (1.1, 1.1, 0.5), (48, 0)),
            c("BODY", "right_repair_patch", (2.2, 15.2, -2.95), (1.0, 2.2, 0.35), (32, 0)),
        ),
    )


def _leg_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        c(mount, f"{prefix}_upper_thigh_plate", (-2.05, 7.0, -2.65), (4.1, 5.2, 0.75)),
        c(mount, f"{prefix}_lower_thigh_plate", (-1.85, 5.1, -2.6), (3.7, 2.1, 0.7)),
        c(mount, f"{prefix}_hip_fauld", (-2.35, 10.8, -2.85), (4.7, 1.4, 0.85)),
        c(mount, f"{prefix}_knee_cap", (-2.15, 3.2, -2.95), (4.3, 1.8, 1.0)),
        c(mount, f"{prefix}_outer_rail", (outer_x, 4.9, -2.0), (0.65, 6.6, 1.2), (32, 0)),
        c(mount, f"{prefix}_rear_strap", (-2.05, 6.2, 2.0), (4.1, 0.65, 0.45), (32, 0)),
        c(mount, f"{prefix}_knee_rivet", (-0.35, 3.7, -3.2), (0.7, 0.7, 0.35), (48, 0)),
        c(mount, f"{prefix}_hip_rivet", (-0.3, 11.15, -3.05), (0.6, 0.6, 0.3), (48, 0)),
        c(mount, f"{prefix}_knee_flange", (outer_x, 3.4, -2.7), (0.75, 1.5, 2.0), (32, 0)),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "iron_leggings",
        "IRON LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.65) + _leg_cubes("RIGHT_LEG", -2.3),
    )


def _boot_cubes(mount: str, outer_x: float) -> tuple[Cube, ...]:
    prefix = mount.lower()
    return (
        c(mount, f"{prefix}_shin_plate", (-2.1, 2.0, -2.7), (4.2, 3.8, 0.75)),
        c(mount, f"{prefix}_toe_cap", (-2.3, 0.0, -3.1), (4.6, 1.8, 1.2)),
        c(mount, f"{prefix}_outer_rail", (outer_x, 0.7, -1.9), (0.65, 4.3, 1.2), (32, 0)),
        c(mount, f"{prefix}_ankle_band", (-2.15, 4.0, -2.75), (4.3, 0.75, 5.5), (32, 0)),
        c(mount, f"{prefix}_toe_rivet", (-0.35, 0.7, -3.35), (0.7, 0.7, 0.35), (48, 0)),
        c(mount, f"{prefix}_heel_plate", (-2.1, 0.4, 2.0), (4.2, 2.3, 0.75)),
        c(mount, f"{prefix}_sole_edge", (-2.35, -0.25, -3.2), (4.7, 0.5, 5.5), (32, 0)),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "iron_boots",
        "IRON BOOTS",
        _boot_cubes("LEFT_FOOT", 1.65) + _boot_cubes("RIGHT_FOOT", -2.3),
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
                base = (85, 85, 85)
            elif x < 48 and y < 32:
                base = (82, 59, 48)
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
    for x, y in ((34, 3), (41, 8), (37, 15), (44, 24), (33, 28)):
        draw.rectangle((x, y, x + 2, y + 1), fill=(125, 69, 43))
    for x, y in ((3, 3), (12, 7), (24, 4), (5, 19), (20, 22), (29, 14)):
        draw.point((x, y), fill=(132, 75, 46))
        if x + 1 < 32:
            draw.point((x + 1, y), fill=(101, 61, 43))
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
