#!/usr/bin/env python3
"""Shrubs / bushes — replaces flora.rs `place_shrub`.

Retired recipe (flora.rs:583): 1-3 tall primary column (blocks[0]) + 4-dir
neighbour accent (blocks[1]) + 50% crown accent (blocks[2]).

DecorationSpec.blocks reference (kind="shrub"):
  starter_shrub   sweet_berry_bush / grass / fern   (spawn_plain)
  ice_thorn       packed_ice / snow_block / pointed_dripstone (broken_peaks)
  reed_thicket    sugar_cane / tall_grass / fern    (spring_marsh)
  red_vine_curtain weeping_vines / crimson_roots    (cave_network)

Variants (>=3): leafy berry bush (lush), ice thorn (jagged), reed thicket
(tall wetland), crimson nylium patch (nether). Anchor: Ground.
"""

from __future__ import annotations

import random

from _helpers import asset_path, save_and_report
from nbt_builder import StructureBuilder

KIND = "bush"


def build_leafy() -> StructureBuilder:
    """Lush leafy bush — azalea leaves clump with berry + fern accents."""
    random.seed(61_001)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # 2-tall core of oak leaves.
    for y in range(2):
        sb.set_block(cx, y, cz, "oak_leaves", {"persistent": "true"})
    # 4-dir flowering azalea accents.
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        if random.random() < 0.75:
            sb.set_block(cx + dx, 0, cz + dz, "flowering_azalea_leaves", {"persistent": "true"})
    # Fern + berry crown.
    sb.set_block(cx, 2, cz, "sweet_berry_bush", {"age": "3"})
    sb.set_block(cx + 1, 0, cz + 1, "fern")
    return sb


def build_ice_thorn() -> StructureBuilder:
    """Jagged ice thorn — packed ice spire with dripstone tips. Peaks."""
    random.seed(61_002)
    sb = StructureBuilder(5, 5, 5)
    cx = cz = 2
    # Central 3-tall packed-ice spire.
    for y in range(3):
        sb.set_block(cx, y, cz, "packed_ice")
    sb.set_block(cx, 3, cz, "blue_ice")
    # Snow-block skirt + dripstone thorns radiating out.
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "snow_block")
        if random.random() < 0.6:
            sb.set_block(cx + dx, 1, cz + dz, "pointed_dripstone",
                         {"vertical_direction": "up", "thickness": "tip"})
    return sb


def build_reed_thicket() -> StructureBuilder:
    """Tall wetland reeds — sugar cane stalks of varied height + grass."""
    random.seed(61_003)
    sb = StructureBuilder(5, 5, 5)
    cx = cz = 2
    # Several cane stalks scattered in a 3x3, heights 2-4.
    for dx in range(-1, 2):
        for dz in range(-1, 2):
            if random.random() < 0.55:
                h = random.randint(2, 4)
                for y in range(h):
                    sb.set_block(cx + dx, y, cz + dz, "sugar_cane")
    # Ground tall grass + fern filler.
    for dx, dz in [(2, 2), (0, 0), (3, 1)]:
        x, z = dx, dz
        if 0 <= x < 5 and 0 <= z < 5:
            sb.set_block(x, 0, z, "fern")
    return sb


def build_crimson_patch() -> StructureBuilder:
    """Nether nylium patch — crimson nylium mound with roots + weeping vines."""
    random.seed(61_004)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # Low nylium mound.
    for dx, dz in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "crimson_nylium")
    # Crimson roots sprouting.
    for dx, dz in [(0, 0), (1, 0), (-1, 1)]:
        sb.set_block(cx + dx, 1, cz + dz, "crimson_roots")
    # A couple weeping vines on the mound.
    sb.set_block(cx, 2, cz, "weeping_vines")
    return sb


VARIANTS = {
    "leafy_v1": build_leafy,
    "ice_thorn_v2": build_ice_thorn,
    "reed_thicket_v3": build_reed_thicket,
    "crimson_patch_v4": build_crimson_patch,
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
