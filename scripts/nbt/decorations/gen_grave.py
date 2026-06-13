#!/usr/bin/env python3
"""Grave mounds — replaces flora.rs `place_grave_mound`.

Retired recipe (flora.rs:836): a sunken cobble dome (body blocks[0] inside,
mossy crust blocks[1] on top/outer rim), radius 2-5, sunk 1 block into the
ground, with a sign (blocks[2]) staked on top.

DecorationSpec.blocks reference (kind="grave_mound"):
  wayfarer_grave  cobblestone / mossy_cobblestone / oak_sign (spawn_plain)

Anchor: Embedded — the registry stamps template y=0 *at* the surface block
(sinking one block), so the dome's lowest row replaces the top soil and the
mound reads as half-buried, not stacked on top. The retired geometry used an
unconditional overwrite for exactly this; an Embedded NBT stamp reproduces it.

Variants (>=3): small mossy grave (radius 2), large grave (radius 4),
twin/double grave (two small mounds). All carry an oak_sign headstone.
"""

from __future__ import annotations

import random

from _helpers import asset_path, disc_offsets, save_and_report
from nbt_builder import StructureBuilder

KIND = "grave"


def _dome(sb: StructureBuilder, cx: int, cz: int, radius: int) -> int:
    """Stack shrinking discs into a half-buried dome; return the top y."""
    mound_h = radius - 1
    for dy in range(mound_h + 1):
        layer_r = radius - dy
        for dx, dz in disc_offsets(layer_r):
            x, z = cx + dx, cz + dz
            d2 = dx * dx + dz * dz
            # Top layer + outer rim -> mossy crust; interior -> plain cobble.
            block = "mossy_cobblestone" if (dy == mound_h or d2 == layer_r * layer_r) else "cobblestone"
            sb.set_block(x, dy, z, block)
    return mound_h


def build_small_grave() -> StructureBuilder:
    """Small mossy grave, radius 2, oak sign headstone."""
    random.seed(66_001)
    radius = 2
    span = 2 * radius + 1
    sb = StructureBuilder(span, radius + 3, span)
    cx = cz = radius
    top = _dome(sb, cx, cz, radius)
    sb.set_block(cx, top + 1, cz, "oak_sign", {"rotation": "8"})
    return sb


def build_large_grave() -> StructureBuilder:
    """Large grave, radius 4, weathered with a couple dead bushes + sign."""
    random.seed(66_002)
    radius = 4
    span = 2 * radius + 1
    sb = StructureBuilder(span, radius + 3, span)
    cx = cz = radius
    top = _dome(sb, cx, cz, radius)
    sb.set_block(cx, top + 1, cz, "oak_sign", {"rotation": "4"})
    # Dead bushes clinging to the mound flank.
    sb.set_block(cx + radius - 1, 1, cz, "dead_bush")
    sb.set_block(cx, 1, cz + radius - 1, "dead_bush")
    return sb


def build_twin_grave() -> StructureBuilder:
    """Twin grave — two small mounds side by side, each with a headstone."""
    random.seed(66_003)
    radius = 2
    # Two domes offset along X.
    span_x = 2 * (radius + 2) + 3
    span_z = 2 * radius + 1
    sb = StructureBuilder(span_x, radius + 3, span_z)
    cz = radius
    for cx in (radius, radius + 2 * radius + 1):
        if cx + radius >= span_x:
            continue
        top = _dome(sb, cx, cz, radius)
        sb.set_block(cx, top + 1, cz, "oak_sign", {"rotation": "8"})
    return sb


VARIANTS = {
    "small_v1": build_small_grave,
    "large_v2": build_large_grave,
    "twin_v3": build_twin_grave,
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
