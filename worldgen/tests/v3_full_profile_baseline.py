"""worldgen-v4 P2 全 profile 行为等价基线 (shared fixtures).

P0 froze a 7-shape representative baseline (``v3_baseline_zones.py``) — one
profile per structural fold (normal / water / sky_isle / cave / abyssal / rift /
waste).  P2 rewrites every profile into the declarative DSL, so the
equivalence对拍物 must widen to **every** registered profile: when a profile is
migrated, its DSL output must still produce the same observable landscape
(surface_y / water_y / biome_id / span shape) it produced as hand-written numpy.

This module is the shared registry: it builds a deterministic tile per profile
(fixed seed via the profile's own ``np.random.RandomState`` perm cache + a fixed
origin tile) and exposes the column observables the golden pins:

  surface_y   = span[0].ceiling_y          (= the v3 carved walkable surface)
  water_y     = round(water_level) or None (-1 sentinel → no water)
  biome_id    = uint8 biome index
  span_shape  = the full ColumnSpans tuple as a list of [floor, ceiling] pairs
                (locks cave voids / sky-isle second spans, not just the surface)

The span shape is produced by folding the profile's 2D buffer through
``spans_fold.spans_for_tile`` — the SAME canonical fold the raster export uses —
so the golden pins exactly what ships to the Rust runtime.  Every profile is now
DSL-driven; its DSL-built layers fold here and must reproduce this tuple within
tolerance; that is the "换表示不换景观" gate.

The baseline is captured ONCE by ``regenerate_v3_full_profile_baseline.py`` and
frozen in ``worldgen/fixtures/v3_full_profile_baseline.json``.  It must NOT be
regenerated casually — only when a profile legitimately changes its landscape,
and then the JSON diff has to be reviewed by hand.
"""

from __future__ import annotations

from collections.abc import Callable

from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import (
    Bounds2D,
    SurfacePalette,
    TileFieldBuffer,
    WorldTile,
)
from scripts.terrain_gen.profiles.abyssal_maze import fill_abyssal_maze_tile
from scripts.terrain_gen.profiles.ancient_battlefield import (
    fill_ancient_battlefield_tile,
)
from scripts.terrain_gen.profiles.ash_dead_zone import fill_ash_dead_zone_tile
from scripts.terrain_gen.profiles.broken_peaks import fill_broken_peaks_tile
from scripts.terrain_gen.profiles.cave_network import fill_cave_network_tile
from scripts.terrain_gen.profiles.dan_zong_yi_yuan import fill_dan_zong_yi_yuan_tile
from scripts.terrain_gen.profiles.jiu_zong_ruin import fill_jiu_zong_ruin_tile
from scripts.terrain_gen.profiles.pseudo_vein_oasis import fill_pseudo_vein_oasis_tile
from scripts.terrain_gen.profiles.rift_mouth_barrens import fill_rift_mouth_barrens_tile
from scripts.terrain_gen.profiles.rift_valley import fill_rift_valley_tile
from scripts.terrain_gen.profiles.sky_isle import fill_sky_isle_tile
from scripts.terrain_gen.profiles.spawn_plain import fill_spawn_plain_tile
from scripts.terrain_gen.profiles.spring_marsh import fill_spring_marsh_tile
from scripts.terrain_gen.profiles.tribulation_scorch import fill_tribulation_scorch_tile
from scripts.terrain_gen.profiles.tsy_daneng_crater import fill_tsy_daneng_crater_tile
from scripts.terrain_gen.profiles.tsy_gaoshou_hermitage import (
    fill_tsy_gaoshou_hermitage_tile,
)
from scripts.terrain_gen.profiles.tsy_zhanchang import fill_tsy_zhanchang_tile
from scripts.terrain_gen.profiles.tsy_zongmen_ruin import fill_tsy_zongmen_ruin_tile
from scripts.terrain_gen.profiles.wangyintai import fill_wangyintai_tile
from scripts.terrain_gen.profiles.waste_plateau import fill_waste_plateau_tile

# 200x200 tile centred on the origin — same geometry as the P0 baseline so the
# heartland / radial math lands identical sampled columns and the two baselines
# stay comparable.
TILE_SIZE = 200
TILE = WorldTile(tile_x=0, tile_z=0, min_x=-100, max_x=99, min_z=-100, max_z=99)

# Sampled local (x, z) columns — center + cardinal + diagonal offsets so each
# profile exercises a representative spread (heartland / rim / cave interior /
# isle / water basin) rather than a single column.
SAMPLE_COLUMNS = (
    (100, 100),  # center
    (60, 100),
    (140, 100),
    (100, 60),
    (100, 140),
    (40, 40),
    (160, 160),
    (160, 40),
    (40, 160),
)

# Per-profile EXTRA sample columns chosen to land on that profile's signature
# vertical fold (the sky-isle floating second span, a cave void column) so the
# golden pins the multi-span shape, not just the surface.  These were located
# from the deterministic v3 output (nearest multi-span column to the tile
# center) and are stable as long as the noise/perm cache is unchanged.
EXTRA_SAMPLE_COLUMNS: dict[str, tuple[tuple[int, int], ...]] = {
    # (75,132): mask>=0.2 → folds to spans[1] = the floating isle block above
    # the ground span — the exact "sky_island_mask → second span" equivalence
    # the DSL carve op must reproduce.
    "sky_isle": ((75, 132),),
    # (100,100): cave_mask>0.58 → ground span carved into surface cap + floor
    # remnant (2 spans). Pins the cave-void fold.
    "cave_network": ((100, 100),),
}


def sample_columns_for(profile_key: str) -> tuple[tuple[int, int], ...]:
    """Base sample grid + this profile's structural extra columns (deduped)."""
    extras = EXTRA_SAMPLE_COLUMNS.get(profile_key, ())
    seen: set[tuple[int, int]] = set()
    out: list[tuple[int, int]] = []
    for col in (*SAMPLE_COLUMNS, *extras):
        if col not in seen:
            seen.add(col)
            out.append(col)
    return tuple(out)

# Extras each profile needs so its center column actually exercises its signature
# carve branch (neg_pressure patch / ascension pit / portal anchor / origin id).
_WASTE_EXTRAS = {
    "negative_pressure_patches": [
        {"center_xz": [0, 0], "radius": 50, "strength": 0.9},
    ]
}
_RIFT_MOUTH_EXTRAS = {
    "portal_anchor_xz": [0, 0],
    "core_radius": 30.0,
    "outer_radius": 150.0,
}
_TRIBULATION_EXTRAS = {
    "ascension_pit_xz": [0, 0],
    "ascension_pit_radius": 40.0,
}
_OASIS_EXTRAS = {"core_radius": 60.0, "rim_radius": 120.0}
_JIUZONG_EXTRAS = {"zongmen_origin_id": 7}


def _zone(
    profile: str,
    shape: str,
    boundary_mode: str,
    spirit_qi: float,
    extras: dict | None = None,
) -> BlueprintZone:
    return BlueprintZone(
        name=f"{profile}_baseline",
        display_name=profile,
        bounds_xz=Bounds2D(min_x=-100, max_x=99, min_z=-100, max_z=99),
        center_xz=(0, 0),
        size_xz=(200, 200),
        spirit_qi=spirit_qi,
        danger_level=2,
        worldgen=ZoneWorldgenConfig(
            terrain_profile=profile,
            shape=shape,
            boundary=BoundarySpec(mode=boundary_mode, width=32),
            height_model={"base": [60, 80], "peak": 96},
            surface_palette=(
                "grass_block",
                "stone",
                "gravel",
                "mud",
                "water",
            ),
            extras=dict(extras) if extras else {},
        ),
    )


FillFn = Callable[[BlueprintZone, WorldTile, int, SurfacePalette], TileFieldBuffer]

# (profile_key, fill_fn, zone) for EVERY registered terrain profile.  profile_key
# == the canonical profile name so it lines up with _GENERATORS and the DSL
# migration table.  TSY profiles default to depth_tier "shallow" (their numpy
# default) so the baseline pins the shallow branch; deeper tiers are exercised by
# their own activation tests.
FULL_PROFILE_BASELINE: tuple[tuple[str, FillFn, BlueprintZone], ...] = (
    (
        "spawn_plain",
        fill_spawn_plain_tile,
        _zone("spawn_plain", "blob", "soft", 0.30),
    ),
    (
        "broken_peaks",
        fill_broken_peaks_tile,
        _zone("broken_peaks", "massif", "soft", 0.40),
    ),
    (
        "spring_marsh",
        fill_spring_marsh_tile,
        _zone("spring_marsh", "basin", "soft", 0.60),
    ),
    (
        "rift_valley",
        fill_rift_valley_tile,
        _zone("rift_valley", "massif", "semi_hard", 0.40),
    ),
    (
        "cave_network",
        fill_cave_network_tile,
        _zone("cave_network", "subterranean_cluster", "semi_hard", 0.40),
    ),
    (
        "jiu_zong_ruin",
        fill_jiu_zong_ruin_tile,
        _zone("jiu_zong_ruin", "massif", "soft", 0.40, extras=_JIUZONG_EXTRAS),
    ),
    (
        "waste_plateau",
        fill_waste_plateau_tile,
        _zone("waste_plateau", "massif", "soft", 0.05, extras=_WASTE_EXTRAS),
    ),
    (
        "pseudo_vein_oasis",
        fill_pseudo_vein_oasis_tile,
        _zone("pseudo_vein_oasis", "blob", "soft", 0.32, extras=_OASIS_EXTRAS),
    ),
    (
        "rift_mouth_barrens",
        fill_rift_mouth_barrens_tile,
        _zone(
            "rift_mouth_barrens", "blob", "soft", 0.05, extras=_RIFT_MOUTH_EXTRAS
        ),
    ),
    ("sky_isle", fill_sky_isle_tile, _zone("sky_isle", "ellipse", "soft", 0.85)),
    (
        "ash_dead_zone",
        fill_ash_dead_zone_tile,
        _zone("ash_dead_zone", "blob", "soft", 0.0),
    ),
    (
        "dan_zong_yi_yuan",
        fill_dan_zong_yi_yuan_tile,
        _zone("dan_zong_yi_yuan", "blob", "soft", 0.40),
    ),
    (
        "abyssal_maze",
        fill_abyssal_maze_tile,
        _zone("abyssal_maze", "subterranean_cluster", "semi_hard", 0.55),
    ),
    (
        "ancient_battlefield",
        fill_ancient_battlefield_tile,
        _zone("ancient_battlefield", "massif", "soft", 0.40),
    ),
    (
        "tribulation_scorch",
        fill_tribulation_scorch_tile,
        _zone(
            "tribulation_scorch", "blob", "soft", 0.30, extras=_TRIBULATION_EXTRAS
        ),
    ),
    (
        "tsy_zongmen_ruin",
        fill_tsy_zongmen_ruin_tile,
        _zone("tsy_zongmen_ruin", "massif", "semi_hard", 0.40),
    ),
    (
        "tsy_daneng_crater",
        fill_tsy_daneng_crater_tile,
        _zone("tsy_daneng_crater", "massif", "semi_hard", 0.45),
    ),
    (
        "tsy_zhanchang",
        fill_tsy_zhanchang_tile,
        _zone("tsy_zhanchang", "massif", "semi_hard", 0.40),
    ),
    (
        "tsy_gaoshou_hermitage",
        fill_tsy_gaoshou_hermitage_tile,
        _zone("tsy_gaoshou_hermitage", "blob", "soft", 0.40),
    ),
    (
        "wangyintai",
        fill_wangyintai_tile,
        _zone("wangyintai", "blob", "soft", -0.15),
    ),
)

# Profiles whose ground surface is sculpted (rift / fracture / neg / entrance) or
# carved (cave / abyssal) — for these the golden surface_y can legitimately drop
# well below round(height) and the span shape may have >1 span.  Pure heightfield
# profiles always fold to exactly one span.
CARVED_PROFILES = frozenset(
    {
        "rift_valley",
        "rift_mouth_barrens",
        "waste_plateau",
        "ash_dead_zone",
        "cave_network",
        "abyssal_maze",
        "tribulation_scorch",
    }
)


def build_full_profile_buffer(profile_key: str) -> TileFieldBuffer:
    """Fill the canonical tile for *profile_key* and return its TileFieldBuffer."""
    for key, fill_fn, zone in FULL_PROFILE_BASELINE:
        if key == profile_key:
            return fill_fn(zone, TILE, TILE_SIZE, SurfacePalette())
    raise KeyError(f"unknown full-profile baseline key {profile_key!r}")


def col_index(local_x: int, local_z: int) -> int:
    return local_z * TILE_SIZE + local_x
