#!/usr/bin/env python3
"""Small/mid trees — replaces flora.rs `place_tree`.

Retired procedural recipe (flora.rs:391): trunk column (blocks[0]) + canopy
sphere radius ~2-4 (blocks[1]) + sparse accent (blocks[2]) + oak vine drape.
Block palettes mirror real DecorationSpec.blocks tuples:

  elder_oak       oak_log / oak_leaves / moss_block   (spawn_plain)
  memory_birch    birch_log / birch_leaves            (spawn_plain)
  qing_yun_pine   spruce_log / spruce_leaves          (broken_peaks)
  petrified_stump polished_diorite / stripped_oak_log (ash_dead_zone)

Variants (>=3 required): oak (round), birch (tall slim), pine (conical),
petrified stump (short dead). Anchor: Ground — trunk base at template y=0.
A player is 2 blocks tall; every living variant clears 6+ blocks so trees
never look shorter than the player.
"""

from __future__ import annotations

import random

from _helpers import asset_path, disc_offsets, save_and_report, sphere_offsets
from nbt_builder import StructureBuilder

KIND = "small_tree"


def _trunk(sb: StructureBuilder, cx: int, cz: int, h: int, log: str, *, axis: str = "y") -> None:
    for y in range(h):
        sb.set_block(cx, y, cz, log, {"axis": axis})


def build_oak() -> StructureBuilder:
    """Round-canopy oak, ~8 tall. Moss accent flecks + a couple vine strands."""
    random.seed(60_001)
    w = d = 9
    cx = cz = w // 2
    trunk_h = 6
    sb = StructureBuilder(w, 8 + trunk_h, d)
    _trunk(sb, cx, cz, trunk_h, "oak_log")

    canopy_y = trunk_h
    radius = 3
    leaf_cells: set[tuple[int, int, int]] = set()
    for dx, dy, dz in sphere_offsets(radius, y_min=-1, y_max=radius):
        x, y, z = cx + dx, canopy_y + dy, cz + dz
        if not (0 <= x < w and 0 <= z < d):
            continue
        d2 = dx * dx + dy * dy + dz * dz
        # Ragged rim: cull ~1/3 of outer-shell leaves.
        if d2 > (radius - 1) ** 2 and random.random() < 0.35:
            continue
        if (x, y, z) == (cx, canopy_y, cz):
            continue  # leave trunk top for the log
        sb.set_block(x, y, z, "oak_leaves", {"persistent": "true"})
        leaf_cells.add((x, y, z))

    # Moss accent flecks — only where a leaf already exists, so moss never
    # floats in a gap the ragged rim cull opened up.
    moss_candidates = [c for c in leaf_cells if c[1] >= canopy_y]
    random.shuffle(moss_candidates)
    for ax, ay, az in moss_candidates[:4]:
        sb.set_block(ax, ay, az, "moss_block")

    # Vine strands drape only from canopy-bottom leaf cells (so each vine hangs
    # from a real leaf, never from empty air where the rim was culled).
    bottom_leaves = sorted(c for c in leaf_cells if c[1] == canopy_y - 1)
    for vx, vy0, vz in bottom_leaves[:4]:
        for vy in range(vy0 - 1, max(vy0 - 3, 0), -1):
            sb.set_block(vx, vy, vz, "vine", {"south": "true"})
    return sb


def build_birch() -> StructureBuilder:
    """Tall, slim birch — ~10 tall, narrow ellipsoid canopy."""
    random.seed(60_002)
    w = d = 7
    cx = cz = w // 2
    trunk_h = 8
    sb = StructureBuilder(w, 6 + trunk_h, d)
    _trunk(sb, cx, cz, trunk_h, "birch_log")

    canopy_y = trunk_h - 1
    # Narrow vertical canopy: small radius, taller band.
    for dy in range(-2, 4):
        radius = 2 if -1 <= dy <= 1 else 1
        for dx, dz in disc_offsets(radius):
            x, y, z = cx + dx, canopy_y + dy, cz + dz
            if not (0 <= x < w and 0 <= z < d):
                continue
            if dx * dx + dz * dz == radius * radius and random.random() < 0.3:
                continue
            if (x, z) == (cx, cz) and dy < 2:
                continue  # trunk core
            sb.set_block(x, y, z, "birch_leaves", {"persistent": "true"})
    return sb


def build_pine() -> StructureBuilder:
    """Conical spruce pine — ~11 tall, stacked shrinking discs of needles."""
    random.seed(60_003)
    w = d = 9
    cx = cz = w // 2
    trunk_h = 11
    sb = StructureBuilder(w, trunk_h + 2, d)
    _trunk(sb, cx, cz, trunk_h, "spruce_log")

    # Conical: wide rings low, narrowing to a tip. Tiers every ~2 blocks.
    tiers = [(2, 3), (4, 3), (5, 2), (7, 2), (8, 1), (10, 1)]
    for ring_y, radius in tiers:
        for dx, dz in disc_offsets(radius):
            x, y, z = cx + dx, ring_y, cz + dz
            if not (0 <= x < w and 0 <= z < d):
                continue
            if dx * dx + dz * dz == radius * radius and random.random() < 0.25:
                continue
            if (x, z) == (cx, cz):
                continue
            sb.set_block(x, y, z, "spruce_leaves", {"persistent": "true"})
    sb.set_block(cx, trunk_h, cz, "spruce_leaves", {"persistent": "true"})  # tip
    # A couple mossy-cobble roots at the base (the spec's blocks[2]).
    for dx, dz in [(1, 0), (-1, 0), (0, 1)]:
        sb.set_block(cx + dx, 0, cz + dz, "mossy_cobblestone")
    return sb


def build_petrified_stump() -> StructureBuilder:
    """Petrified ancient-tree stump — a thick, snapped-off fossil trunk with
    flared roots, weathered stone 'bark', and a hollow rotted heartwood core.
    Ash dead zone; long dead, so no canopy."""
    random.seed(60_004)
    w = d = 7
    cx = cz = w // 2
    sb = StructureBuilder(w, 6, d)

    palette = ["polished_diorite", "andesite", "stone", "cobblestone"]

    def petrified() -> str:
        r = random.random()
        if r < 0.12:
            return "cracked_stone_bricks"
        if r < 0.24:
            return "mossy_cobblestone"
        return random.choice(palette)

    # Flared roots gripping the ground (y=0): a 3×3 pad + cardinal root tips
    # radiating out, plus a few longer diagonal roots, so the stump reads as
    # anchored into the earth rather than a block sitting on top of it.
    root_cells = {(dx, dz) for dx in (-1, 0, 1) for dz in (-1, 0, 1)}
    root_cells |= {(2, 0), (-2, 0), (0, 2), (0, -2)}
    for cell in [(2, 1), (-2, -1), (1, 2), (-1, -2)]:
        if random.random() < 0.6:
            root_cells.add(cell)
    for dx, dz in root_cells:
        sb.set_block(cx + dx, 0, cz + dz, petrified())

    # Thick 3×3 trunk: the weathered 'bark' shell snaps off at jagged heights
    # while the heartwood core rotted out hollow (only the root pad beneath it).
    for dx in (-1, 0, 1):
        for dz in (-1, 0, 1):
            if dx == 0 and dz == 0:
                continue  # hollow rotted core
            top = random.randint(3, 5)
            for y in range(1, top):
                sb.set_block(cx + dx, y, cz + dz, petrified())

    # A snapped-off branch stub jutting from one side, mid-height.
    sb.set_block(cx + 2, 2, cz, petrified())
    # Dead brush clinging to the exposed roots.
    for dx, dz in [(-2, 0), (0, 2)]:
        sb.set_block(cx + dx, 1, cz + dz, "dead_bush")
    return sb


VARIANTS = {
    "oak_round_v1": build_oak,
    "birch_tall_v2": build_birch,
    "pine_conical_v3": build_pine,
    "petrified_stump_v4": build_petrified_stump,
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
