#!/usr/bin/env python3
"""render_structure.py — isometric PNG previews of decoration NBT structures.

Reads an MC 1.20.1 structure `.nbt` via `nbt_builder.load_structure`, then
draws an oblique (cabinet-style) pseudo-3D projection so the proportions of a
decoration (tree height vs. canopy, bridge span, mound shape) are reviewable
without launching the game. Painter's-algorithm sorting (back-to-front) gives
solid-looking voxels.

Usage:
    python3 scripts/nbt/render_structure.py <file.nbt> [out.png]
    python3 scripts/nbt/render_structure.py --all          # all decorations
"""

from __future__ import annotations

import os
import sys

from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(__file__))
from nbt_builder import load_structure  # noqa: E402

# ── block -> base RGB colour ────────────────────────────────────────────────
# Approximate top-face tints; good enough to read silhouette + material.
_COLORS: dict[str, tuple[int, int, int]] = {
    "oak_log": (123, 94, 53),
    "stripped_oak_log": (188, 152, 98),
    "birch_log": (215, 207, 187),
    "spruce_log": (94, 70, 41),
    "oak_leaves": (52, 110, 38),
    "birch_leaves": (110, 150, 70),
    "spruce_leaves": (46, 76, 46),
    "flowering_azalea_leaves": (120, 150, 70),
    "moss_block": (89, 109, 45),
    "moss_carpet": (95, 116, 50),
    "vine": (60, 100, 40),
    "sweet_berry_bush": (90, 110, 50),
    "fern": (80, 120, 55),
    "dead_bush": (130, 100, 55),
    "sugar_cane": (130, 175, 95),
    "crimson_nylium": (130, 40, 45),
    "crimson_roots": (140, 40, 60),
    "crimson_hyphae": (90, 40, 55),
    "weeping_vines": (150, 40, 50),
    "packed_ice": (160, 195, 235),
    "blue_ice": (120, 165, 220),
    "snow_block": (240, 248, 252),
    "pointed_dripstone": (140, 110, 95),
    "mossy_cobblestone": (95, 110, 80),
    "cobblestone": (130, 130, 130),
    "stone": (140, 140, 140),
    "stone_bricks": (128, 128, 122),
    "chiseled_stone_bricks": (118, 118, 112),
    "cracked_stone_bricks": (110, 108, 100),
    "mossy_stone_bricks": (100, 118, 92),
    "andesite": (136, 136, 138),
    "deepslate": (70, 70, 74),
    "cobbled_deepslate": (78, 78, 82),
    "smooth_basalt": (74, 72, 80),
    "basalt": (82, 80, 86),
    "polished_basalt": (88, 86, 92),
    "blackstone": (44, 40, 48),
    "polished_blackstone": (52, 48, 58),
    "polished_blackstone_bricks": (56, 52, 62),
    "chiseled_polished_blackstone": (60, 55, 66),
    "polished_diorite": (210, 210, 212),
    "prismarine": (95, 150, 140),
    "calcite": (224, 224, 220),
    "bone_block": (224, 220, 196),
    "gravel": (130, 124, 118),
    "soul_sand": (84, 64, 52),
    "soul_soil": (90, 68, 56),
    "coarse_dirt": (118, 88, 60),
    "amethyst_block": (140, 100, 200),
    "amethyst_cluster": (170, 130, 220),
    "budding_amethyst": (150, 110, 205),
    "small_amethyst_bud": (160, 120, 210),
    "purple_stained_glass": (150, 80, 200),
    "obsidian": (30, 24, 46),
    "crying_obsidian": (50, 24, 80),
    "lodestone": (110, 112, 120),
    "end_rod": (235, 235, 230),
    "soul_lantern": (120, 200, 215),
    "emerald_ore": (110, 150, 110),
    "deepslate_emerald_ore": (70, 100, 80),
    "shroomlight": (245, 170, 80),
    "red_mushroom_block": (200, 60, 55),
    "brown_mushroom_block": (150, 110, 80),
    "mushroom_stem": (220, 215, 205),
    "red_mushroom": (200, 60, 55),
    "brown_mushroom": (150, 110, 80),
    "iron_bars": (160, 160, 165),
    "spruce_planks": (110, 82, 50),
    "dark_oak_planks": (66, 44, 24),
    "oak_sign": (160, 130, 80),
    "skeleton_skull": (220, 218, 210),
}
_DEFAULT_COLOR = (200, 0, 200)  # magenta = unmapped, so gaps are obvious.


def _shade(rgb: tuple[int, int, int], factor: float) -> tuple[int, int, int]:
    return tuple(max(0, min(255, int(c * factor))) for c in rgb)


def render(path: str, out_png: str, *, cell: int = 14) -> dict:
    blocks = load_structure(path)
    if not blocks:
        raise ValueError(f"{path} has no blocks to render")

    xs = [b.pos[0] for b in blocks]
    ys = [b.pos[1] for b in blocks]
    zs = [b.pos[2] for b in blocks]
    minx, maxx = min(xs), max(xs)
    miny, maxy = min(ys), max(ys)
    minz, maxz = min(zs), max(zs)
    sx = maxx - minx + 1
    sy = maxy - miny + 1
    sz = maxz - minz + 1

    # Oblique projection: screen_x = (x - z) basis, screen_y = up - depth.
    half = cell // 2
    pad = cell * 2
    width = (sx + sz) * cell + pad * 2
    height = (sy * cell) + (sx + sz) * half + pad * 2
    img = Image.new("RGB", (width, height), (245, 245, 248))
    draw = ImageDraw.Draw(img)

    # Painter's order: far (high z, low x, low y) first; near & high last.
    def sort_key(b):
        x, y, z = b.pos
        return (z - minz) + (x - minx) - (y - miny) * 0.001, (y - miny)

    ordered = sorted(blocks, key=lambda b: ((b.pos[2] - minz) + (b.pos[0] - minx), b.pos[1] - miny))

    origin_x = pad + sz * cell
    origin_y = height - pad - half

    for b in ordered:
        x, y, z = b.pos
        lx, ly, lz = x - minx, y - miny, z - minz
        # Screen position of the block's top-left.
        scr_x = origin_x + (lx - lz) * cell - half
        scr_y = origin_y - ly * cell - (lx + lz) * half
        name = b.block_name.replace("minecraft:", "")
        base = _COLORS.get(name, _DEFAULT_COLOR)
        # Three faces: top (bright), left (mid), right (dark) for a cube look.
        top = _shade(base, 1.0)
        left = _shade(base, 0.72)
        right = _shade(base, 0.52)
        # Top rhombus.
        draw.polygon(
            [
                (scr_x, scr_y),
                (scr_x + cell, scr_y - half),
                (scr_x + 2 * cell, scr_y),
                (scr_x + cell, scr_y + half),
            ],
            fill=top, outline=(30, 30, 30),
        )
        # Left face.
        draw.polygon(
            [
                (scr_x, scr_y),
                (scr_x + cell, scr_y + half),
                (scr_x + cell, scr_y + half + cell),
                (scr_x, scr_y + cell),
            ],
            fill=left, outline=(25, 25, 25),
        )
        # Right face.
        draw.polygon(
            [
                (scr_x + cell, scr_y + half),
                (scr_x + 2 * cell, scr_y),
                (scr_x + 2 * cell, scr_y + cell),
                (scr_x + cell, scr_y + half + cell),
            ],
            fill=right, outline=(25, 25, 25),
        )

    label = f"{os.path.basename(path)}  {sx}x{sy}x{sz}  {len(blocks)} blocks"
    draw.text((6, 6), label, fill=(20, 20, 20))
    os.makedirs(os.path.dirname(out_png), exist_ok=True)
    img.save(out_png)
    return {"path": path, "out": out_png, "size": (sx, sy, sz), "blocks": len(blocks)}


def _all() -> None:
    repo = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    deco_root = os.path.join(repo, "server", "structures", "decorations")
    out_root = os.path.join(repo, "docs", "p6_renders")
    rendered = []
    for kind in sorted(os.listdir(deco_root)):
        kind_dir = os.path.join(deco_root, kind)
        if not os.path.isdir(kind_dir):
            continue
        for fn in sorted(os.listdir(kind_dir)):
            if not fn.endswith(".nbt"):
                continue
            src = os.path.join(kind_dir, fn)
            out = os.path.join(out_root, kind, fn.replace(".nbt", ".png"))
            r = render(src, out)
            rendered.append(r)
            print(f"  {kind}/{fn} -> {os.path.relpath(out, repo)}  size={r['size']}")
    print(f"\nRendered {len(rendered)} previews into docs/p6_renders/")


if __name__ == "__main__":
    args = sys.argv[1:]
    if args and args[0] == "--all":
        _all()
    elif args:
        src = args[0]
        out = args[1] if len(args) > 1 else src.replace(".nbt", ".png")
        r = render(src, out)
        print(r)
    else:
        print(__doc__)
        sys.exit(1)
