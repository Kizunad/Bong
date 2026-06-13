#!/usr/bin/env python3
"""Big mushrooms — replaces flora.rs `place_mushroom`.

Retired recipe (flora.rs:745): 2-4 tall stem (blocks[1]) + radius-2 disc cap
(blocks[0], jagged rim) + center accent (blocks[2]).

DecorationSpec.blocks reference (kind="mushroom"):
  xun_guang_mushroom shroomlight / crimson_hyphae / red_mushroom_block (abyssal_maze)

Variants (>=3): glowing shroomlight (xianxia abyssal), brown wild mushroom,
red wild mushroom. Anchor: Ground.
"""

from __future__ import annotations

import random

from _helpers import asset_path, disc_offsets, save_and_report
from nbt_builder import StructureBuilder

KIND = "big_mushroom"


def _mushroom(seed: int, stem_h: int, cap: str, stem: str, accent: str,
              *, cap_radius: int = 2, cap_props=None) -> StructureBuilder:
    random.seed(seed)
    span = 2 * cap_radius + 1
    sb = StructureBuilder(span, stem_h + 3, span)
    cx = cz = cap_radius
    for y in range(stem_h):
        sb.set_block(cx, y, cz, stem)
    cap_y = stem_h
    for dx, dz in disc_offsets(cap_radius):
        x, z = cx + dx, cz + dz
        # Jagged rim cull.
        if dx * dx + dz * dz == cap_radius * cap_radius and random.random() < 1 / 3:
            continue
        sb.set_block(x, cap_y, z, cap, cap_props)
    # Center accent above the cap.
    sb.set_block(cx, cap_y + 1, cz, accent, {"facing": "up"} if "amethyst" in accent else None)
    return sb


def build_shroomlight() -> StructureBuilder:
    """Abyssal xun-guang mushroom — glowing shroomlight cap, crimson stem."""
    return _mushroom(64_001, 4, "shroomlight", "crimson_hyphae", "red_mushroom_block",
                     cap_radius=2)


def build_brown_wild() -> StructureBuilder:
    """Brown wild mushroom — classic brown cap, short stem."""
    return _mushroom(64_002, 2, "brown_mushroom_block",
                     "mushroom_stem", "brown_mushroom", cap_radius=2,
                     cap_props={"north": "true", "south": "true", "east": "true", "west": "true"})


def build_red_wild() -> StructureBuilder:
    """Red wild mushroom — taller red cap with a wider crown, mushroom stem."""
    return _mushroom(64_003, 3, "red_mushroom_block",
                     "mushroom_stem", "red_mushroom", cap_radius=3,
                     cap_props={"north": "true", "south": "true", "east": "true", "west": "true"})


VARIANTS = {
    "shroomlight_v1": build_shroomlight,
    "brown_wild_v2": build_brown_wild,
    "red_wild_v3": build_red_wild,
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
