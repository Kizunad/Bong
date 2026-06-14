#!/usr/bin/env python3
"""Crystal clusters — replaces flora.rs `place_crystal`.

Retired recipe (flora.rs:673): vertical column h (blocks[0]) + tip (blocks[1],
2 blocks when h>4) + 4-dir short base stubs (blocks[2], 3/8 chance).

DecorationSpec.blocks reference (kind="crystal"):
  tian_mai_crystal amethyst_cluster / amethyst_block / budding_amethyst (sky_isle)
  xuan_jing_pillar obsidian / amethyst_block / crying_obsidian (abyssal_maze)
  phantom_qi_pillar amethyst_cluster / purple_stained_glass / soul_lantern (pseudo_vein)
  frost_qi_cluster packed_ice / blue_ice / amethyst_cluster (rift_mouth)

Variants (>=3): amethyst spire (purple), obsidian pillar (tall dark),
frost cluster (icy), phantom qi pillar (glowing). Anchor: Ground.
"""

from __future__ import annotations

import random

from _helpers import asset_path, save_and_report
from nbt_builder import StructureBuilder

KIND = "crystal"


def _crystal(seed: int, h: int, body: str, tip: str, accent: str,
             *, body_props=None, tip_props=None) -> StructureBuilder:
    """A *cluster* (not a lone pole): a tall central spire ringed by shorter
    satellite shards so the silhouette reads as a crystal growth, and the
    footprint stays a symmetric 3x3 instead of collapsing to a 1-wide sliver.

    The retired `place_crystal` used a single column + 3/8-chance base stubs,
    which with a fixed seed routinely landed lopsided (one stub, off to one
    side). We replace the coin-flip stubs with a *deterministic* satellite ring
    whose heights step down with distance, giving every variant the same
    legible, balanced cluster shape regardless of seed.
    """
    random.seed(seed)
    sb = StructureBuilder(5, h + 3, 5)
    cx = cz = 2
    # Central spire.
    for y in range(h):
        sb.set_block(cx, y, cz, body, body_props)
    sb.set_block(cx, h, cz, tip, tip_props)
    if h > 4:
        sb.set_block(cx, h + 1, cz, tip, tip_props)
    # Satellite shards: 4 cardinal companions (always present) at ~half the
    # spire's height, each tipped with a cluster, plus 4 diagonal stubs for a
    # rocky base so the cluster sits in a little crystal bed.
    sat_h = max(2, h // 2)
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        for sy in range(sat_h):
            sb.set_block(cx + dx, sy, cz + dz, body, body_props)
        sb.set_block(cx + dx, sat_h, cz + dz, tip, tip_props)
    # Diagonal base stubs (1 tall) — the accent material, framing the bed.
    for dx, dz in [(1, 1), (-1, 1), (1, -1), (-1, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, accent, tip_props if accent == tip else None)
    return sb


def build_amethyst_spire() -> StructureBuilder:
    """Sky-isle amethyst spire — amethyst block column, cluster tip."""
    return _crystal(63_001, 5, "amethyst_block", "amethyst_cluster", "budding_amethyst",
                    tip_props={"facing": "up"})


def build_obsidian_pillar() -> StructureBuilder:
    """Abyssal xuan-jing pillar — tall obsidian with crying-obsidian veins."""
    random.seed(63_002)
    h = 9
    sb = StructureBuilder(5, h + 3, 5)
    cx = cz = 2
    for y in range(h):
        # Crying-obsidian veins woven into the obsidian column.
        block = "crying_obsidian" if random.random() < 0.25 else "obsidian"
        sb.set_block(cx, y, cz, block)
    sb.set_block(cx, h, cz, "amethyst_block")
    sb.set_block(cx, h + 1, cz, "amethyst_cluster", {"facing": "up"})
    # Satellite obsidian shards (cardinal, ~1/3 height) tipped with amethyst, so
    # the xuan-jing pillar reads as a dark crystal cluster, not a single needle.
    sat_h = max(2, h // 3)
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        for sy in range(sat_h):
            block = "crying_obsidian" if random.random() < 0.2 else "obsidian"
            sb.set_block(cx + dx, sy, cz + dz, block)
        sb.set_block(cx + dx, sat_h, cz + dz, "amethyst_cluster", {"facing": "up"})
    return sb


def build_frost_cluster() -> StructureBuilder:
    """Rift-mouth frost cluster — packed/blue ice with amethyst tips."""
    return _crystal(63_003, 4, "packed_ice", "blue_ice", "amethyst_cluster",
                    tip_props={"facing": "up"})


def build_phantom_qi() -> StructureBuilder:
    """Pseudo-vein phantom qi pillar — glowing amethyst + soul lantern accents."""
    random.seed(63_004)
    h = 6
    sb = StructureBuilder(5, h + 3, 5)
    cx = cz = 2
    for y in range(h):
        block = "purple_stained_glass" if y % 2 == 1 else "amethyst_cluster"
        props = {"facing": "up"} if block == "amethyst_cluster" else None
        sb.set_block(cx, y, cz, block, props)
    sb.set_block(cx, h, cz, "amethyst_cluster", {"facing": "up"})
    sb.set_block(cx, h + 1, cz, "amethyst_cluster", {"facing": "up"})
    # Glowing satellite shards (cardinal) + soul-lantern braziers on the
    # diagonals, so the phantom-qi pillar sits in a ring of pseudo-vein light.
    sat_h = max(2, h // 2)
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        for sy in range(sat_h):
            block = "purple_stained_glass" if sy % 2 == 1 else "amethyst_block"
            sb.set_block(cx + dx, sy, cz + dz, block)
        sb.set_block(cx + dx, sat_h, cz + dz, "amethyst_cluster", {"facing": "up"})
    for dx, dz in [(1, 1), (-1, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "soul_lantern", {"hanging": "false"})
    return sb


VARIANTS = {
    "amethyst_spire_v1": build_amethyst_spire,
    "obsidian_pillar_v2": build_obsidian_pillar,
    "frost_cluster_v3": build_frost_cluster,
    "phantom_qi_v4": build_phantom_qi,
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
