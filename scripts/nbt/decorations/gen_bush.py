#!/usr/bin/env python3
"""Shrubs / bushes — replaces flora.rs `place_shrub`.

Retired recipe (flora.rs:583): 1-3 tall primary column (blocks[0]) + 4-dir
neighbour accent (blocks[1]) + 50% crown accent (blocks[2]).

worldgen-v4 P6 ecology fix
==========================
A `kind="shrub"` decoration is authored by MANY profiles spanning wildly
different biomes (spawn meadow, ice peaks, nether rift, wetland marsh, ...).
The first cut shipped a single ``bush/`` pool whose four variants were one per
biome, so the runtime ``hash % len`` pick would happily stamp a nether
``crimson_patch`` or a frozen ``ice_thorn`` into the spawn starter meadow.

The fix splits the pool into **ecology subdirs** — one per biome family — each
with its own internally-consistent variant set:

    bush_temperate/   lush overworld foliage (berry / fern / azalea)
    bush_cold/        frozen alpine thorns (packed ice / snow / dripstone)
    bush_marsh/       wetland reeds (sugar cane / tall grass / lily pad)
    bush_nether/      nether growth (crimson nylium / roots / vines)

``profiles/base.py`` maps each shrub *name* to exactly one ecology, so the
variant pool a name resolves into is mutually exclusive with the other
ecologies' pools — a temperate shrub can never resolve a nether/cold variant.

DecorationSpec.blocks reference (kind="shrub"):
  starter_shrub   sweet_berry_bush / grass / fern   (spawn_plain → temperate)
  ice_thorn       packed_ice / snow_block / pointed_dripstone (broken_peaks → cold)
  reed_thicket    sugar_cane / tall_grass / fern    (spring_marsh → marsh)
  red_vine_curtain weeping_vines / crimson_roots    (cave_network → nether)

Every ecology subdir ships >=3 variants (height / density / species). Anchor:
Ground.
"""

from __future__ import annotations

import random

from _helpers import asset_path, save_and_report
from nbt_builder import StructureBuilder

# Ecology subdir labels — mirrors the keys in profiles/base.py `_SHRUB_ECOLOGY`
# values and the `bush_<ecology>` dir layout under decorations/.
ECO_TEMPERATE = "bush_temperate"
ECO_COLD = "bush_cold"
ECO_MARSH = "bush_marsh"
ECO_NETHER = "bush_nether"


# ---------------------------------------------------------------------------
# Temperate — lush overworld foliage.
# ---------------------------------------------------------------------------
def build_temperate_leafy() -> StructureBuilder:
    """Lush leafy bush — azalea leaves clump with berry + fern accents."""
    random.seed(61_001)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    for y in range(2):
        sb.set_block(cx, y, cz, "oak_leaves", {"persistent": "true"})
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "flowering_azalea_leaves", {"persistent": "true"})
    sb.set_block(cx, 2, cz, "sweet_berry_bush", {"age": "3"})
    sb.set_block(cx + 1, 0, cz + 1, "fern")
    sb.set_block(cx - 1, 0, cz - 1, "fern")
    return sb


def build_temperate_fern_clump() -> StructureBuilder:
    """Low grass + fern ground clump — a flatter meadow tuft."""
    random.seed(61_011)
    sb = StructureBuilder(5, 3, 5)
    cx = cz = 2
    # Soft ground tuft: grass core, ferns radiating, a couple taller ferns.
    sb.set_block(cx, 0, cz, "fern")
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)]:
        block = "grass" if (dx + dz) % 2 == 0 else "fern"
        sb.set_block(cx + dx, 0, cz + dz, block)
    # Two tall_grass blades standing one block taller for vertical relief
    # (tall_grass resolves in the server palette; large_fern does not).
    sb.set_block(cx, 1, cz, "tall_grass", {"half": "lower"})
    sb.set_block(cx + 1, 1, cz, "tall_grass", {"half": "lower"})
    return sb


def build_temperate_azalea_mound() -> StructureBuilder:
    """Domed azalea bush — leaf mound with a flowering crown."""
    random.seed(61_012)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # 3x3 base ring of azalea leaves.
    for dx in (-1, 0, 1):
        for dz in (-1, 0, 1):
            sb.set_block(cx + dx, 0, cz + dz, "azalea_leaves", {"persistent": "true"})
    # Cardinal second tier + flowering crown.
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 1, cz + dz, "azalea_leaves", {"persistent": "true"})
    sb.set_block(cx, 1, cz, "flowering_azalea_leaves", {"persistent": "true"})
    sb.set_block(cx, 2, cz, "flowering_azalea_leaves", {"persistent": "true"})
    return sb


# ---------------------------------------------------------------------------
# Cold — frozen alpine thorns.
# ---------------------------------------------------------------------------
def build_cold_ice_thorn() -> StructureBuilder:
    """Jagged ice thorn — packed ice spire with dripstone tips."""
    random.seed(61_002)
    sb = StructureBuilder(5, 5, 5)
    cx = cz = 2
    for y in range(3):
        sb.set_block(cx, y, cz, "packed_ice")
    sb.set_block(cx, 3, cz, "blue_ice")
    for i, (dx, dz) in enumerate([(1, 0), (-1, 0), (0, 1), (0, -1)]):
        sb.set_block(cx + dx, 0, cz + dz, "snow_block")
        thickness = "tip" if i % 2 == 0 else "frustum"
        sb.set_block(cx + dx, 1, cz + dz, "pointed_dripstone",
                     {"vertical_direction": "up", "thickness": thickness})
    return sb


def build_cold_snow_crust() -> StructureBuilder:
    """Low snow-crusted shrub — snow mound with frosted dead bramble."""
    random.seed(61_021)
    sb = StructureBuilder(5, 3, 5)
    cx = cz = 2
    # Snow-block crust over a 3x3 footprint.
    for dx in (-1, 0, 1):
        for dz in (-1, 0, 1):
            sb.set_block(cx + dx, 0, cz + dz, "snow_block")
    # A frosted, leafless bramble poking up from the crust + a snow nub
    # (powder_snow is not in the server palette, so use snow_block for the nub).
    sb.set_block(cx, 1, cz, "dead_bush")
    sb.set_block(cx + 1, 1, cz, "dead_bush")
    sb.set_block(cx, 1, cz - 1, "snow_block")
    return sb


def build_cold_frost_bramble() -> StructureBuilder:
    """Frost bramble — blue-ice nub with twin dripstone thorns."""
    random.seed(61_022)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # Twin short ice nubs with dripstone thorns on top, on a snow skirt.
    for dx, dz in [(0, 0), (1, 0)]:
        sb.set_block(cx + dx, 0, cz + dz, "packed_ice")
        sb.set_block(cx + dx, 1, cz + dz, "blue_ice")
        sb.set_block(cx + dx, 2, cz + dz, "pointed_dripstone",
                     {"vertical_direction": "up", "thickness": "tip"})
    for dx, dz in [(-1, 0), (0, 1), (0, -1), (2, 0)]:
        sb.set_block(cx + dx, 0, cz + dz, "snow_block")
    return sb


# ---------------------------------------------------------------------------
# Marsh — wetland reeds.
# ---------------------------------------------------------------------------
def build_marsh_reed_thicket() -> StructureBuilder:
    """Tall wetland reeds — sugar cane stalks of varied height + grass."""
    random.seed(61_003)
    sb = StructureBuilder(5, 5, 5)
    cx = cz = 2
    for dx in range(-1, 2):
        for dz in range(-1, 2):
            if random.random() < 0.55:
                h = random.randint(2, 4)
                for y in range(h):
                    sb.set_block(cx + dx, y, cz + dz, "sugar_cane")
    for dx, dz in [(2, 2), (0, 0), (3, 1)]:
        x, z = dx, dz
        if 0 <= x < 5 and 0 <= z < 5:
            sb.set_block(x, 0, z, "fern")
    return sb


def build_marsh_cattail_clump() -> StructureBuilder:
    """Cattail clump — tall-grass tufts with a few short cane spikes."""
    random.seed(61_031)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # Tall-grass ring with cane spikes at the cardinal points.
    for dx, dz in [(0, 0), (1, 1), (-1, -1), (1, -1), (-1, 1)]:
        sb.set_block(cx + dx, 0, cz + dz, "tall_grass", {"half": "lower"})
    for dx, dz in [(1, 0), (-1, 0), (0, 1)]:
        sb.set_block(cx + dx, 0, cz + dz, "sugar_cane")
        sb.set_block(cx + dx, 1, cz + dz, "sugar_cane")
    return sb


def build_marsh_lily_reed() -> StructureBuilder:
    """Lily-fringed reeds — lily pads round a low reed/fern stand."""
    random.seed(61_032)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # Central reed stand.
    for y in range(3):
        sb.set_block(cx, y, cz, "sugar_cane")
    # Fern + lily-pad fringe.
    for dx, dz in [(1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "lily_pad")
    sb.set_block(cx + 1, 0, cz + 1, "fern")
    sb.set_block(cx - 1, 0, cz - 1, "fern")
    return sb


# ---------------------------------------------------------------------------
# Nether — crimson / warped growth.
# ---------------------------------------------------------------------------
def build_nether_crimson_patch() -> StructureBuilder:
    """Nether nylium patch — crimson nylium mound with roots + weeping vines."""
    random.seed(61_004)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    for dx, dz in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "crimson_nylium")
    for dx, dz in [(0, 0), (1, 0), (-1, 1)]:
        sb.set_block(cx + dx, 1, cz + dz, "crimson_roots")
    sb.set_block(cx, 2, cz, "weeping_vines")
    return sb


def build_nether_warped_patch() -> StructureBuilder:
    """Warped growth — nylium mound with warped roots + twisting vines."""
    random.seed(61_041)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # Crimson nylium base (warped_nylium isn't in the server palette), warped
    # flora on top so the patch still reads as the cyan warped variant.
    for dx, dz in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]:
        sb.set_block(cx + dx, 0, cz + dz, "crimson_nylium")
    for dx, dz in [(1, 0), (-1, 0), (0, 1)]:
        sb.set_block(cx + dx, 1, cz + dz, "warped_roots")
    # Twisting vines grow UP, so they tower over the centre nylium.
    for y in (1, 2):
        sb.set_block(cx, y, cz, "twisting_vines")
    return sb


def build_nether_vine_curtain() -> StructureBuilder:
    """Weeping-vine curtain — nylium ledge dripping crimson roots + vines."""
    random.seed(61_042)
    sb = StructureBuilder(5, 4, 5)
    cx = cz = 2
    # A 3-wide nylium ledge with weeping vines hanging from beneath it... but
    # ground-anchored, so we instead stand them: nylium pad + crimson roots
    # crown + weeping vines as the tall centrepiece.
    for dx in (-1, 0, 1):
        sb.set_block(cx + dx, 0, cz, "crimson_nylium")
    sb.set_block(cx, 0, cz + 1, "crimson_nylium")
    sb.set_block(cx, 0, cz - 1, "crimson_nylium")
    for dx in (-1, 0, 1):
        sb.set_block(cx + dx, 1, cz, "crimson_roots")
    for y in (1, 2):
        sb.set_block(cx, y, cz + 1, "weeping_vines")
    return sb


# ---------------------------------------------------------------------------
# Ecology → variant builder map. The subdir name is the asset `kind` so the
# resulting template ids are `decorations/bush_<ecology>/<variant>.nbt`.
# ---------------------------------------------------------------------------
ECOLOGIES: dict[str, dict] = {
    ECO_TEMPERATE: {
        "leafy_v1": build_temperate_leafy,
        "fern_clump_v2": build_temperate_fern_clump,
        "azalea_mound_v3": build_temperate_azalea_mound,
    },
    ECO_COLD: {
        "ice_thorn_v1": build_cold_ice_thorn,
        "snow_crust_v2": build_cold_snow_crust,
        "frost_bramble_v3": build_cold_frost_bramble,
    },
    ECO_MARSH: {
        "reed_thicket_v1": build_marsh_reed_thicket,
        "cattail_clump_v2": build_marsh_cattail_clump,
        "lily_reed_v3": build_marsh_lily_reed,
    },
    ECO_NETHER: {
        "crimson_patch_v1": build_nether_crimson_patch,
        "warped_patch_v2": build_nether_warped_patch,
        "vine_curtain_v3": build_nether_vine_curtain,
    },
}


def generate() -> list[dict]:
    reports = []
    for eco_dir, variants in ECOLOGIES.items():
        for variant, builder in variants.items():
            sb = builder()
            reports.append(save_and_report(sb, asset_path(eco_dir, variant), preview_y=0))
    return reports


if __name__ == "__main__":
    for r in generate():
        print(f"\n=== {r['path']}")
        print(f"  size={r['size']} blocks={r['total_blocks']} palette={r['palette_size']} bytes={r['file_bytes']}")
        print(f"  counts={r['block_counts']}")
