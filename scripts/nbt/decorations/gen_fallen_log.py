#!/usr/bin/env python3
"""Fallen logs — replaces flora.rs `place_fallen_log`.

Retired recipe (flora.rs:803): a single horizontal log run (axis X or Z by
hash) of length 3-6, blocks[0] = oak_log with the axis property set.

DecorationSpec.blocks reference (kind="fallen_log"):
  fallen_oak_log  oak_log  (spawn_plain)

Orientation handling — fallen logs are directional. We author the log lying
along +X (axis=x). The Rust registry can place it in 4 orientations via
`Rotation::{None,Cw90,Cw180,Cw270}`; for the 90/270 turns the runtime wiring
(Stage 3) is responsible for swapping the log's `axis` property to `z`, since
position-only rotation keeps the baked `axis=x`. To make the asset set
self-sufficient regardless, we also ship a pre-baked axis=z variant and a
mossy/decayed variant so the look reads as a rotted log, not a clean beam.

Variants (>=3): oak along X (clean), spruce along Z (pre-rotated), mossy
decayed oak along X (with mushrooms + moss). Anchor: Ground.
"""

from __future__ import annotations

import random

from _helpers import asset_path, save_and_report
from nbt_builder import StructureBuilder

KIND = "fallen_log"


def build_oak_x() -> StructureBuilder:
    """Clean oak log lying along +X, length 5."""
    random.seed(65_001)
    length = 5
    sb = StructureBuilder(length, 2, 3)
    cz = 1
    for x in range(length):
        sb.set_block(x, 0, cz, "oak_log", {"axis": "x"})
    return sb


def build_spruce_z() -> StructureBuilder:
    """Pre-rotated spruce log lying along +Z, length 6."""
    random.seed(65_002)
    length = 6
    sb = StructureBuilder(3, 2, length)
    cx = 1
    for z in range(length):
        sb.set_block(cx, 0, z, "spruce_log", {"axis": "z"})
    return sb


def build_mossy_decayed() -> StructureBuilder:
    """Rotted mossy oak log along +X with moss carpet + clinging mushrooms."""
    random.seed(65_003)
    length = 5
    sb = StructureBuilder(length, 3, 3)
    cz = 1
    for x in range(length):
        # Mix in stripped/mossy sections to read as decayed.
        block = "stripped_oak_log" if random.random() < 0.4 else "oak_log"
        sb.set_block(x, 0, cz, block, {"axis": "x"})
        # Moss carpet draped on top of some segments...
        if random.random() < 0.5:
            sb.set_block(x, 1, cz, "moss_carpet")
        # ...and a clinging mushroom sprouting at the log's *side* (offset z),
        # so it sits beside the moss instead of overwriting it.
        elif random.random() < 0.4:
            side = cz + (1 if random.random() < 0.5 else -1)
            sb.set_block(x, 1, side, "brown_mushroom")
    return sb


VARIANTS = {
    "oak_x_v1": build_oak_x,
    "spruce_z_v2": build_spruce_z,
    "mossy_decayed_v3": build_mossy_decayed,
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
