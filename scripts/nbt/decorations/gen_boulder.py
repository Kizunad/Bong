#!/usr/bin/env python3
"""Boulders — replaces flora.rs `place_boulder`.

Retired recipe (flora.rs:619): upper hemisphere radius 2-5, blocks[0] primary
with 1/7 secondary + 1/19 tertiary flecks, upper rim broken by hash cull.

DecorationSpec.blocks reference (kind="boulder"):
  wayfarer_rock   mossy_cobblestone / cobblestone / stone (spawn_plain)
  ridge_monolith  deepslate / andesite / cobbled_deepslate (broken_peaks)
  jade_moss_rock  moss_block / mossy_cobblestone / prismarine (spring_marsh)
  bleached_bone_rubble bone_block / calcite / gravel (waste_plateau)

Variants (>=3): mossy cobble boulder (small), deepslate monolith (tall),
jade moss rock (green), bone rubble (wastes). Anchor: Ground.
"""

from __future__ import annotations

import random

from _helpers import asset_path, hemisphere_offsets, save_and_report
from nbt_builder import StructureBuilder

KIND = "boulder"


def _boulder(seed: int, radius: int, primary: str, secondary: str, tertiary: str,
             *, height_scale: float = 1.0) -> StructureBuilder:
    random.seed(seed)
    span = 2 * radius + 1
    height = int(radius * height_scale) + 1
    sb = StructureBuilder(span, height + 1, span)
    cx = cz = radius
    top = int(radius * height_scale)
    for dx, dy, dz in hemisphere_offsets(radius):
        y = int(dy * height_scale)
        if y > top:
            continue
        x, z = cx + dx, cz + dz
        # Break the very top rim so it isn't a clean dome.
        if y == top and random.random() < 0.25:
            continue
        roll = random.random()
        if roll < 1 / 7:
            block = secondary
        elif roll < 1 / 7 + 1 / 19:
            block = tertiary
        else:
            block = primary
        sb.set_block(x, y, z, block)
    return sb


def build_mossy_cobble() -> StructureBuilder:
    """Wayfarer rock — small mossy cobble boulder, radius 3."""
    return _boulder(62_001, 3, "mossy_cobblestone", "cobblestone", "stone")


def build_deepslate_monolith() -> StructureBuilder:
    """Ridge monolith — tall deepslate slab, radius 4, stretched vertically."""
    return _boulder(62_002, 4, "deepslate", "andesite", "cobbled_deepslate",
                    height_scale=1.6)


def build_jade_moss() -> StructureBuilder:
    """Jade moss rock — green mossy boulder with prismarine flecks, radius 3."""
    return _boulder(62_003, 3, "moss_block", "mossy_cobblestone", "prismarine")


def build_bone_rubble() -> StructureBuilder:
    """Bleached bone rubble — wastes bone/calcite scatter mound, radius 4."""
    return _boulder(62_004, 4, "bone_block", "calcite", "gravel", height_scale=0.7)


VARIANTS = {
    "mossy_cobble_v1": build_mossy_cobble,
    "deepslate_monolith_v2": build_deepslate_monolith,
    "jade_moss_v3": build_jade_moss,
    "bone_rubble_v4": build_bone_rubble,
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
