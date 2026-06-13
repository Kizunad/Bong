#!/usr/bin/env python3
"""Hanging crystals — replaces flora.rs `place_hanging_crystal`.

Retired recipe (flora.rs:713): an inverted crystal hanging *down* from a
sky-isle underside — body column (blocks[1]) growing downward, tip (blocks[0])
at the very bottom, 4-dir accent stubs (blocks[2]) near the top.

DecorationSpec.blocks reference (sky_isle tian_mai_crystal, bottom anchor):
  tian_mai_crystal amethyst_cluster / amethyst_block / budding_amethyst

Anchor: Hanging — the registry aligns the template's *top* row (y = size_y-1)
to the underside block (`surface_pos.y - 1`) and the rest hangs below. So we
author the template with the attachment/body at the TOP (high y) and the tip
at the BOTTOM (y=0), matching how it should read once hung.

Variants (>=3): amethyst stalactite (purple), icy dripstone (frost),
deep amethyst pendant (long). Anchor: Hanging.
"""

from __future__ import annotations

import random

from _helpers import asset_path, save_and_report
from nbt_builder import StructureBuilder

KIND = "hanging_crystal"


def _hanging(seed: int, h: int, body: str, tip: str, accent: str,
             *, body_props=None, tip_props=None, accent_props=None) -> StructureBuilder:
    """Author top-attached, tip-at-bottom. Template top (y=h-1) is the anchor row."""
    random.seed(seed)
    sb = StructureBuilder(5, h, 5)
    cx = cz = 2
    # Body fills from just below the top down toward the tip.
    for y in range(1, h):
        sb.set_block(cx, y, cz, body, body_props)
    # Tip at the very bottom (y=0).
    sb.set_block(cx, 0, cz, tip, tip_props)
    # Accent stubs clinging near the top attachment row.
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        if random.random() < 2 / 8:
            sb.set_block(cx + dx, h - 1, cz + dz, accent, accent_props)
    return sb


def build_amethyst_stalactite() -> StructureBuilder:
    """Sky-isle amethyst stalactite — amethyst block body, cluster tip."""
    return _hanging(67_001, 5, "amethyst_block", "amethyst_cluster", "budding_amethyst",
                    tip_props={"facing": "down"})


def build_icy_dripstone() -> StructureBuilder:
    """Frost dripstone pendant — packed-ice body, pointed-dripstone tip down."""
    return _hanging(67_002, 4, "packed_ice", "pointed_dripstone", "blue_ice",
                    tip_props={"vertical_direction": "down", "thickness": "tip"})


def build_deep_pendant() -> StructureBuilder:
    """Long amethyst pendant — taller, with a 2-block budding tip."""
    random.seed(67_003)
    h = 7
    sb = StructureBuilder(5, h, 5)
    cx = cz = 2
    for y in range(2, h):
        sb.set_block(cx, y, cz, "amethyst_block")
    # Two-block tip.
    sb.set_block(cx, 1, cz, "budding_amethyst")
    sb.set_block(cx, 0, cz, "amethyst_cluster", {"facing": "down"})
    for dx, dz in [(1, 0), (0, -1)]:
        sb.set_block(cx + dx, h - 1, cz + dz, "amethyst_block")
    return sb


VARIANTS = {
    "amethyst_stalactite_v1": build_amethyst_stalactite,
    "icy_dripstone_v2": build_icy_dripstone,
    "deep_pendant_v3": build_deep_pendant,
}


def generate() -> list[dict]:
    reports = []
    for variant, builder in VARIANTS.items():
        sb = builder()
        reports.append(save_and_report(sb, asset_path(KIND, variant), preview_y=0))
    return reports


if __name__ == "__main__":
    for r in generate():
        print(f"\n=== {r['path']}")
        print(f"  size={r['size']} blocks={r['total_blocks']} palette={r['palette_size']} bytes={r['file_bytes']}")
        print(f"  counts={r['block_counts']}")
