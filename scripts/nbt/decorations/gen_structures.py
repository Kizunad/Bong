#!/usr/bin/env python3
"""Mid structures — replaces the 6 scatter structures in structures.rs.

These are the density-scatter feature structures (NOT the large deterministic
sect layouts): ruin pillar / broken altar (urn) / bone pile / spirit ore vein /
rift bridge / spawn portal. Block palettes + proportions mirror the retired
`rasterize_*` recipes in `server/src/world/terrain/structures.rs`.

Each kind ships >=3 variants (height / completeness / orientation). Orientation
for the rift bridge is baked as explicit X / Z axis variants (the runtime may
further rotate via `Rotation`). All anchor: Ground unless noted.
"""

from __future__ import annotations

import math
import random

from _helpers import asset_path, disc_offsets, save_and_report
from nbt_builder import StructureBuilder

# ---------------------------------------------------------------------------
# Ruin pillar — foundation + tapering column + overhanging crown.
# Palette: chiseled / cracked / mossy stone bricks, polished blackstone.
# ---------------------------------------------------------------------------
RUINS_PILLAR = "ruins_pillar"


def _ruin_pillar(seed: int, column_h: int, footprint_r: int, crown_overhang: int,
                 *, broken_top: bool) -> StructureBuilder:
    random.seed(seed)
    extent = footprint_r + crown_overhang + 1
    span = 2 * extent + 1
    sb = StructureBuilder(span, column_h + 4, span)
    cx = cz = extent

    # Foundation: a 2-tall plinth footprint_r wide.
    for fy in range(2):
        for dx, dz in disc_offsets(footprint_r):
            ring = max(abs(dx), abs(dz))
            block = "mossy_stone_bricks" if ring == footprint_r else "stone_bricks"
            sb.set_block(cx + dx, fy, cz + dz, block)

    # Column: square 1x1 (sometimes 2x2 at the base), brick mix, growing up.
    for cy in range(2, 2 + column_h):
        # Taper: occasional missing/cracked block for a ruined look.
        if broken_top and cy >= 2 + column_h - 2 and random.random() < 0.5:
            continue
        roll = random.random()
        block = ("cracked_stone_bricks" if roll < 0.35
                 else "mossy_stone_bricks" if roll < 0.6
                 else "chiseled_stone_bricks")
        sb.set_block(cx, cy, cz, block)

    # Crown: overhanging cap if not broken off.
    if not broken_top:
        crown_y = 2 + column_h
        for dx in range(-crown_overhang, crown_overhang + 1):
            for dz in range(-crown_overhang, crown_overhang + 1):
                sb.set_block(cx + dx, crown_y, cz + dz, "polished_blackstone")
    return sb


def build_pillar_intact():
    return _ruin_pillar(70_001, 9, 2, 1, broken_top=False)


def build_pillar_tall_broken():
    return _ruin_pillar(70_002, 13, 2, 2, broken_top=True)


def build_pillar_short_stub():
    return _ruin_pillar(70_003, 5, 2, 1, broken_top=True)


# ---------------------------------------------------------------------------
# Broken urn / altar — platform + dais + corner posts + core.
# Palette: polished blackstone, crying obsidian, amethyst, blackstone slabs.
# ---------------------------------------------------------------------------
BROKEN_URN = "broken_urn"


def _altar(seed: int, platform_r: int, dais_r: int, *, with_posts: bool) -> StructureBuilder:
    random.seed(seed)
    extent = platform_r + 1
    span = 2 * extent + 1
    sb = StructureBuilder(span, 6, span)
    cx = cz = extent

    # Platform floor (ring-tiered blackstone).
    for dx, dz in disc_offsets(platform_r):
        ring = max(abs(dx), abs(dz))
        roll = random.random()
        if ring == platform_r and roll < 0.25:
            continue  # broken edge
        block = ("polished_blackstone_bricks" if ring == platform_r
                 else "polished_blackstone")
        sb.set_block(cx + dx, 0, cz + dz, block)

    # Dais: raised central tier.
    for dx, dz in disc_offsets(dais_r):
        sb.set_block(cx + dx, 1, cz + dz, "polished_blackstone")

    # Core: crying-obsidian + amethyst centrepiece (the "urn").
    sb.set_block(cx, 2, cz, "crying_obsidian")
    sb.set_block(cx, 3, cz, "amethyst_block")

    # Rim posts. The posts must stand on the platform, so they go on the four
    # cardinal RIM points (±platform_r, 0)/(0, ±platform_r) — those lie exactly
    # on the disc (dist² == platform_r²), unlike the (±r, ±r) corners (dist² ==
    # 2·r²) which fall OUTSIDE the disc footprint and would float over AIR. We
    # also stamp a guaranteed footing at y=0 under each post so the broken-edge
    # roll above (which can delete the rim ring block) never leaves a post
    # hanging in the air.
    if with_posts:
        rim_points = [(platform_r, 0), (-platform_r, 0), (0, platform_r), (0, -platform_r)]
        for sx, sz in rim_points:
            if random.random() < 0.7:
                sb.set_block(cx + sx, 0, cz + sz, "polished_blackstone_bricks")
                for py in range(1, 4):
                    sb.set_block(cx + sx, py, cz + sz, "polished_blackstone")
    return sb


def build_urn_full():
    return _altar(71_001, 3, 2, with_posts=True)


def build_urn_small():
    return _altar(71_002, 2, 1, with_posts=False)


def build_urn_collapsed():
    sb = _altar(71_003, 3, 1, with_posts=True)
    return sb


# ---------------------------------------------------------------------------
# Bone pile — ellipsoid bone mound + skulls + scatter.
# Palette: bone_block, soul_sand, soul_soil, gravel, skeleton_skull.
# ---------------------------------------------------------------------------
BONE_PILE = "bone_pile"


def _bone_pile(seed: int, radius: int, height: int) -> StructureBuilder:
    random.seed(seed)
    span = 2 * radius + 1
    sb = StructureBuilder(span, height + 2, span)
    cx = cz = radius
    # Track the absolute (x, z) of the TOP mound layer so skulls perch on real
    # mound blocks — a floor `skeleton_skull` needs a solid block below it or it
    # pops into a dropped item the instant the chunk loads.
    top_cells: list[tuple[int, int]] = []
    top_dy = height - 1
    for dy in range(height):
        layer_r = max(1, int(radius * (1 - dy / max(height, 1))))
        for dx, dz in disc_offsets(layer_r):
            roll = random.random()
            if roll < 0.6:
                block = "bone_block"
            elif roll < 0.75:
                block = "soul_sand"
            elif roll < 0.85:
                block = "soul_soil"
            else:
                block = "gravel"
            props = {"axis": random.choice(["x", "y", "z"])} if block == "bone_block" else None
            sb.set_block(cx + dx, dy, cz + dz, block, props)
            if dy == top_dy:
                top_cells.append((cx + dx, cz + dz))
    # Skulls perched on top — seated ONLY on top-layer mound cells (guaranteed
    # solid support directly below), so no skull ever hangs over AIR.
    n_skulls = min(max(1, radius - 1), len(top_cells))
    for sx, sz in random.sample(top_cells, n_skulls):
        sb.set_block(sx, height, sz, "skeleton_skull", {"rotation": str(random.randint(0, 15))})
    return sb


def build_bones_small():
    return _bone_pile(72_001, 3, 2)


def build_bones_large():
    return _bone_pile(72_002, 4, 3)


def build_bones_scattered():
    """A flatter, more spread-out scatter (lower mound, wider)."""
    return _bone_pile(72_003, 5, 2)


# ---------------------------------------------------------------------------
# Spirit ore vein — calcite/basalt matrix outcrop + amethyst/emerald nodes.
# Palette: calcite, smooth_basalt, deepslate matrix; amethyst/budding/emerald.
# ---------------------------------------------------------------------------
SPIRIT_ORE = "spirit_ore_vein"


def _ore_vein(seed: int, radius: int, height: int, rich: bool) -> StructureBuilder:
    random.seed(seed)
    span = 2 * radius + 1
    sb = StructureBuilder(span, height + 2, span)
    cx = cz = radius
    # Outcrop: rocky matrix hemisphere. Remember the absolute (x, z) of every
    # cell on the TOP layer so the upward amethyst clusters can be seated on a
    # real solid block — a `facing=up` AmethystClusterBlock needs a solid
    # support face below it in MC 1.20.1, or it pops off into a dropped item the
    # instant the chunk loads.
    top_cells: list[tuple[int, int]] = []
    top_dy = height - 1
    for dy in range(height):
        layer_r = max(1, radius - dy)
        for dx, dz in disc_offsets(layer_r):
            roll = random.random()
            node_chance = 0.35 if rich else 0.2
            if roll < node_chance:
                block = random.choice(["amethyst_block", "budding_amethyst",
                                       "deepslate_emerald_ore", "emerald_ore"])
                props = None
            else:
                block = random.choice(["calcite", "smooth_basalt", "deepslate", "stone"])
                props = None
            sb.set_block(cx + dx, dy, cz + dz, block, props)
            if dy == top_dy:
                top_cells.append((cx + dx, cz + dz))
    # Amethyst cluster shards poking from the top — seated ONLY on top-layer
    # outcrop cells (guaranteed solid support directly below), so no shard ever
    # hangs over AIR. Pick a deterministic subset so each variant differs.
    n_shards = min(max(2, radius), len(top_cells))
    for sx, sz in random.sample(top_cells, n_shards):
        sb.set_block(sx, height, sz, "amethyst_cluster", {"facing": "up"})
    return sb


def build_ore_small():
    return _ore_vein(73_001, 2, 2, rich=False)


def build_ore_rich():
    return _ore_vein(73_002, 3, 3, rich=True)


def build_ore_flat():
    return _ore_vein(73_003, 4, 2, rich=False)


# ---------------------------------------------------------------------------
# Rift bridge remnant — pylons + catenary plank deck + chain rails.
# Palette: polished_basalt/blackstone/basalt pylons; planks deck; chains/iron_bars.
# ---------------------------------------------------------------------------
RIFT_BRIDGE = "rift_bridge"


def _rift_bridge(seed: int, span_half: int, deck_half_w: int, pylon_h: int,
                 *, axis: str, broken: bool) -> StructureBuilder:
    """A remnant rope/plank span between two stone pylons.

    The deck reads as *one* coherent plank material (a real walkway), not a
    per-block material lottery. Pylons are a single banded stone so they read
    as masonry towers. The catenary deck dips toward the middle and the
    iron-bar railings line both edges along the whole span. Broken variants
    drop the middle plank section out, leaving the gap a rift bridge should
    have once the ropes rotted.
    """
    random.seed(seed)
    if axis == "x":
        sx_span = 2 * span_half + 1
        sz_span = 2 * deck_half_w + 1
    else:
        sx_span = 2 * deck_half_w + 1
        sz_span = 2 * span_half + 1
    sb = StructureBuilder(sx_span, pylon_h + 4, sz_span)
    along_ctr = span_half

    def place(along, cross, y, block, props=None):
        if axis == "x":
            sb.set_block(along, y, cross, block, props)
        else:
            sb.set_block(cross, y, along, block, props)

    # One deck plank species + one pylon stone per bridge, chosen by seed so
    # each instance differs but is internally consistent.
    deck_plank = random.choice(["spruce_planks", "dark_oak_planks"])
    pylon_stone = random.choice(["polished_basalt", "blackstone", "stone_bricks"])
    pylon_cap = "basalt"

    # Pylons at both ends — solid stone towers, banded with a darker cap row.
    for end in (0, 2 * span_half):
        for cross in range(0, 2 * deck_half_w + 1):
            for py in range(pylon_h):
                block = pylon_cap if py == pylon_h - 1 else pylon_stone
                place(end, cross, py, block)

    # Railing prop: bars connect along the span axis so they read as a continuous
    # rail. The span runs along `along`, which maps to world-x for axis="x"
    # (→ east/west connections) and world-z for axis="z" (→ north/south).
    rail_props = ({"east": "true", "west": "true"} if axis == "x"
                  else {"north": "true", "south": "true"})

    # Catenary deck: dips toward the middle (sag).
    max_sag = max(1, span_half // 3)
    for along in range(1, 2 * span_half):  # skip the pylon columns themselves
        rel = along - along_ctr
        sag = int(max_sag * (1 - (rel / max(span_half, 1)) ** 2))
        deck_y = max(0, pylon_h - 1 - sag)
        # Broken bridges drop a middle plank section out.
        if broken and abs(rel) <= 1:
            continue
        for cross in range(2 * deck_half_w + 1):
            place(along, cross, deck_y, deck_plank)
            # Iron-bar railing on both deck edges, one block above the planks.
            if cross in (0, 2 * deck_half_w):
                place(along, cross, deck_y + 1, "iron_bars", rail_props)
    return sb


def build_bridge_x():
    return _rift_bridge(74_001, 6, 1, 4, axis="x", broken=False)


def build_bridge_z():
    return _rift_bridge(74_002, 6, 1, 4, axis="z", broken=False)


def build_bridge_broken():
    return _rift_bridge(74_003, 7, 2, 5, axis="x", broken=True)


# ---------------------------------------------------------------------------
# Spawn portal — circular platform + end pillars + lodestone + end_rod + core.
# Palette: polished blackstone, crying obsidian, lodestone, end_rod, amethyst.
# ---------------------------------------------------------------------------
SPAWN_PORTAL = "spawn_portal"


def _spawn_portal(seed: int, platform_r: int, pillar_h: int) -> StructureBuilder:
    random.seed(seed)
    extent = platform_r + 1
    span = 2 * extent + 1
    sb = StructureBuilder(span, pillar_h + 4, span)
    cx = cz = extent

    # Circular platform.
    for dx, dz in disc_offsets(platform_r):
        roll = random.random()
        block = ("crying_obsidian" if (dx == 0 and dz == 0)
                 else "obsidian" if roll < 0.2
                 else "amethyst_block" if roll < 0.3
                 else "polished_blackstone")
        sb.set_block(cx + dx, 0, cz + dz, block)

    # Lodestone core on the platform centre.
    sb.set_block(cx, 1, cz, "lodestone")
    sb.set_block(cx, 2, cz, "end_rod", {"facing": "up"})

    # End pillars around the rim (cardinal points).
    rim = platform_r
    for dx, dz in [(rim, 0), (-rim, 0), (0, rim), (0, -rim)]:
        for py in range(1, pillar_h + 1):
            block = random.choice(["polished_blackstone", "chiseled_polished_blackstone",
                                   "crying_obsidian", "polished_blackstone_bricks"])
            sb.set_block(cx + dx, py, cz + dz, block)
        sb.set_block(cx + dx, pillar_h + 1, cz + dz, "end_rod", {"facing": "up"})
    return sb


def build_portal_main():
    return _spawn_portal(75_001, 4, 3)


def build_portal_small():
    return _spawn_portal(75_002, 3, 2)


def build_portal_grand():
    return _spawn_portal(75_003, 5, 4)


VARIANT_GROUPS = {
    RUINS_PILLAR: {
        "intact_v1": build_pillar_intact,
        "tall_broken_v2": build_pillar_tall_broken,
        "short_stub_v3": build_pillar_short_stub,
    },
    BROKEN_URN: {
        "full_v1": build_urn_full,
        "small_v2": build_urn_small,
        "collapsed_v3": build_urn_collapsed,
    },
    BONE_PILE: {
        "small_v1": build_bones_small,
        "large_v2": build_bones_large,
        "scattered_v3": build_bones_scattered,
    },
    SPIRIT_ORE: {
        "small_v1": build_ore_small,
        "rich_v2": build_ore_rich,
        "flat_v3": build_ore_flat,
    },
    RIFT_BRIDGE: {
        "x_v1": build_bridge_x,
        "z_v2": build_bridge_z,
        "broken_v3": build_bridge_broken,
    },
    SPAWN_PORTAL: {
        "main_v1": build_portal_main,
        "small_v2": build_portal_small,
        "grand_v3": build_portal_grand,
    },
}


def generate() -> list[dict]:
    reports = []
    for kind, variants in VARIANT_GROUPS.items():
        for variant, builder in variants.items():
            sb = builder()
            reports.append(save_and_report(sb, asset_path(kind, variant), preview_y=0))
    return reports


if __name__ == "__main__":
    for r in generate():
        print(f"\n=== {r['path']}")
        print(f"  size={r['size']} blocks={r['total_blocks']} palette={r['palette_size']} bytes={r['file_bytes']}")
        print(f"  counts={r['block_counts']}")
