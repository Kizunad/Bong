#!/usr/bin/env python3
"""Generate 16x16 workbench block textures for plan-workbench-recipes-v1 P0.2.

Outputs:
  client/src/main/resources/assets/bong/textures/block/workbench_top.png
  client/src/main/resources/assets/bong/textures/block/workbench_side.png
  client/src/main/resources/assets/bong/textures/block/workbench_front.png
  client/src/main/resources/assets/bong/textures/block/workbench_bottom.png
"""

import os
import random
from PIL import Image, ImageDraw

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.dirname(SCRIPT_DIR)
OUT_DIR = os.path.join(
    PROJECT_ROOT,
    "client", "src", "main", "resources", "assets", "bong", "textures", "block",
)

# Colors from plan P0.2 spec
BONE_WHITE = (0xE8, 0xDC, 0xC8)       # #E8DCC8 - top background
GRID_BROWN = (0x5C, 0x4A, 0x3A)       # #5C4A3A - grid lines
CINNABAR_RED = (0x8B, 0x3A, 0x3A)     # #8B3A3A - center array mark
BONE_PIN = (0xD4, 0xC8, 0xB0)         # #D4C8B0 - corner bone pins
WOOD_DARK = (0x3E, 0x2E, 0x1E)        # #3E2E1E - side wood base
VEIN_BLUE = (0x6B, 0x7B, 0x8A)        # #6B7B8A - spirit veins
IRON_GREY = (0x6E, 0x6E, 0x6E)        # #6E6E6E - iron handle
STONE_GREY = (0x7A, 0x7A, 0x7A)       # #7A7A7A - bottom stone base

# Subtle variations for natural feel
BONE_LIGHT = (0xED, 0xE2, 0xD0)
BONE_SHADOW = (0xD8, 0xCC, 0xB4)
WOOD_MID = (0x4A, 0x38, 0x28)
WOOD_LIGHT = (0x54, 0x42, 0x30)
STONE_DARK = (0x6E, 0x6E, 0x6E)
STONE_LIGHT = (0x86, 0x86, 0x86)
STONE_MID = (0x72, 0x72, 0x72)


def gen_top():
    """Workbench top: bone white base + 3x3 grid + center array mark + corner bone pins."""
    img = Image.new("RGBA", (16, 16), BONE_WHITE + (255,))
    draw = ImageDraw.Draw(img)

    # Subtle texture variation
    rng = random.Random(42)
    for y in range(16):
        for x in range(16):
            if rng.random() < 0.15:
                shade = rng.choice([BONE_LIGHT, BONE_SHADOW])
                img.putpixel((x, y), shade + (255,))

    # 3x3 grid lines (at positions 5 and 10, dividing into ~5px cells)
    for i in [5, 10]:
        for j in range(1, 15):
            draw.point((i, j), fill=GRID_BROWN + (255,))
            draw.point((j, i), fill=GRID_BROWN + (255,))

    # Border edge (1px outline)
    for i in range(16):
        draw.point((i, 0), fill=GRID_BROWN + (200,))
        draw.point((i, 15), fill=GRID_BROWN + (200,))
        draw.point((0, i), fill=GRID_BROWN + (200,))
        draw.point((15, i), fill=GRID_BROWN + (200,))

    # Center 4x4 array mark (cinnabar red) at pixels 6-9
    for y in range(6, 10):
        for x in range(6, 10):
            img.putpixel((x, y), CINNABAR_RED + (255,))
    # Inner accent - brighter center
    img.putpixel((7, 7), (0x9B, 0x4A, 0x4A, 255))
    img.putpixel((8, 8), (0x9B, 0x4A, 0x4A, 255))

    # Four corner bone pins (2x2 at each corner, offset 1px from edge)
    for cx, cy in [(1, 1), (13, 1), (1, 13), (13, 13)]:
        for dx in range(2):
            for dy in range(2):
                img.putpixel((cx + dx, cy + dy), BONE_PIN + (255,))

    return img


def gen_side():
    """Workbench side: dark wood grain with subtle spirit veins."""
    img = Image.new("RGBA", (16, 16), WOOD_DARK + (255,))
    draw = ImageDraw.Draw(img)

    # Wood grain horizontal streaks
    rng = random.Random(101)
    for y in range(16):
        for x in range(16):
            r = rng.random()
            if r < 0.20:
                img.putpixel((x, y), WOOD_MID + (255,))
            elif r < 0.30:
                img.putpixel((x, y), WOOD_LIGHT + (255,))

    # Horizontal grain lines (every 3-4 rows)
    for y in [3, 7, 11, 14]:
        for x in range(16):
            if rng.random() < 0.7:
                c = (0x35, 0x25, 0x15)
                img.putpixel((x, y), c + (255,))

    # Spirit veins (subtle blue-grey, semi-random paths, blended into wood)
    vein_starts = [(2, 4), (10, 9), (6, 13)]
    for sx, sy in vein_starts:
        x, y = sx, sy
        for _ in range(5):
            if 0 <= x < 16 and 0 <= y < 16:
                base = img.getpixel((x, y))
                # Blend 30% vein color into base wood
                blended = tuple(
                    int(base[c] * 0.7 + VEIN_BLUE[c] * 0.3) for c in range(3)
                ) + (255,)
                img.putpixel((x, y), blended)
            x += rng.choice([-1, 0, 1])
            y += rng.choice([0, 1])
            x = max(0, min(15, x))
            y = max(0, min(15, y))

    # Top and bottom edge darkening
    for x in range(16):
        px = img.getpixel((x, 0))
        img.putpixel((x, 0), (max(0, px[0] - 20), max(0, px[1] - 20), max(0, px[2] - 15), 255))
        px = img.getpixel((x, 15))
        img.putpixel((x, 15), (max(0, px[0] - 20), max(0, px[1] - 20), max(0, px[2] - 15), 255))

    return img


def gen_front():
    """Workbench front: same as side but with iron handle in center."""
    img = gen_side()

    # Iron handle: 2x3 px recess (spec) + 1x1 ring below
    # Recess area (darker, 2 wide x 3 tall) at x=7-8, y=5-7
    recess_color = (0x2E, 0x1E, 0x12, 255)
    for y in range(5, 8):
        for x in range(7, 9):
            img.putpixel((x, y), recess_color)

    # Iron ring (1x1 at bottom center of recess)
    img.putpixel((7, 7), IRON_GREY + (255,))
    img.putpixel((8, 7), (0x7A, 0x7A, 0x7A, 255))  # ring highlight
    # Ring shadow below
    img.putpixel((7, 8), (0x55, 0x55, 0x55, 255))
    img.putpixel((8, 8), (0x55, 0x55, 0x55, 255))

    return img


def gen_bottom():
    """Workbench bottom: grey stone base texture."""
    img = Image.new("RGBA", (16, 16), STONE_GREY + (255,))

    # Stone texture variation
    rng = random.Random(77)
    for y in range(16):
        for x in range(16):
            r = rng.random()
            if r < 0.20:
                img.putpixel((x, y), STONE_DARK + (255,))
            elif r < 0.35:
                img.putpixel((x, y), STONE_LIGHT + (255,))
            elif r < 0.42:
                img.putpixel((x, y), STONE_MID + (255,))

    # Subtle crack lines
    cracks = [(3, 2, 5, 5), (11, 8, 13, 12), (7, 10, 9, 14)]
    for x1, y1, x2, y2 in cracks:
        cx, cy = x1, y1
        while cx <= x2 and cy <= y2:
            if 0 <= cx < 16 and 0 <= cy < 16:
                img.putpixel((cx, cy), (0x60, 0x60, 0x60, 255))
            cx += rng.choice([0, 1])
            cy += rng.choice([0, 1])

    # Edge darkening
    for i in range(16):
        px = img.getpixel((i, 0))
        img.putpixel((i, 0), (max(0, px[0] - 15), max(0, px[1] - 15), max(0, px[2] - 15), 255))
        px = img.getpixel((i, 15))
        img.putpixel((i, 15), (max(0, px[0] - 15), max(0, px[1] - 15), max(0, px[2] - 15), 255))

    return img


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    textures = {
        "workbench_top.png": gen_top(),
        "workbench_side.png": gen_side(),
        "workbench_front.png": gen_front(),
        "workbench_bottom.png": gen_bottom(),
    }
    for name, img in textures.items():
        path = os.path.join(OUT_DIR, name)
        img.save(path)
        print(f"  wrote {path} ({img.size[0]}x{img.size[1]})")
    print(f"Generated {len(textures)} workbench textures.")


if __name__ == "__main__":
    main()
