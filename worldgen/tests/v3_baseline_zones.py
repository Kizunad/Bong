"""Shared fixtures for the worldgen-v4 P0 行为等价对拍.

These build deterministic zones + tiles for the four representative column
shapes that the span refactor must preserve byte-for-byte at the 2D level:

  * normal    — broken_peaks: single solid span (bedrock → height)
  * water     — spring_marsh: solid span with a water surface above it
  * sky_isle  — sky_isle: a second high-altitude span above the ground
  * cave      — cave_network: the ground span carved open by a void
  * abyssal   — abyssal_maze: ground span carved + kept tier semantic layer

The v3 baseline (height / water_level / biome_id at sampled columns) is frozen
in ``v3_surface_baseline.json``.  P0's shim must reproduce those exact values:
``surface_y == round(height)`` and ``water_y == round(water_level) if >= 0``.

Capturing the baseline BEFORE any generation-code change is the whole point —
once the shim lands we can only verify equivalence against numbers taken from
the untouched v3 pipeline.  See ``regenerate_v3_baseline.py``.
"""

from __future__ import annotations

from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import Bounds2D, WorldTile
from scripts.terrain_gen.profiles.abyssal_maze import fill_abyssal_maze_tile
from scripts.terrain_gen.profiles.broken_peaks import fill_broken_peaks_tile
from scripts.terrain_gen.profiles.cave_network import fill_cave_network_tile
from scripts.terrain_gen.profiles.sky_isle import fill_sky_isle_tile
from scripts.terrain_gen.profiles.spring_marsh import fill_spring_marsh_tile

# Fixed 200x200 tile centred on the origin so every profile's heartland math
# (radial falloff around zone center) lands the same sampled columns.
TILE_SIZE = 200
TILE = WorldTile(
    tile_x=0, tile_z=0, min_x=-100, max_x=99, min_z=-100, max_z=99
)

# Sampled local (x, z) columns — center + four cardinal offsets, chosen so each
# profile exercises a non-trivial column (isle / cave interior / water basin).
SAMPLE_COLUMNS = (
    (100, 100),  # center
    (60, 100),
    (140, 100),
    (100, 60),
    (100, 140),
    (40, 40),
    (160, 160),
)


def _zone(profile: str, shape: str, boundary_mode: str, spirit_qi: float) -> BlueprintZone:
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
        ),
    )


# (profile_key, fill_fn, zone_builder) — the canonical baseline matrix.
BASELINE_PROFILES = (
    (
        "normal",
        fill_broken_peaks_tile,
        _zone("broken_peaks", "massif", "soft", 0.4),
    ),
    (
        "water",
        fill_spring_marsh_tile,
        _zone("spring_marsh", "basin", "soft", 0.6),
    ),
    (
        "sky_isle",
        fill_sky_isle_tile,
        _zone("sky_isle", "ellipse", "soft", 0.8),
    ),
    (
        "cave",
        fill_cave_network_tile,
        _zone("cave_network", "subterranean_cluster", "semi_hard", 0.4),
    ),
    (
        "abyssal",
        fill_abyssal_maze_tile,
        _zone("abyssal_maze", "subterranean_cluster", "semi_hard", 0.55),
    ),
)


def build_baseline_buffer(profile_key: str):
    """Fill the canonical tile for *profile_key* and return its TileFieldBuffer."""
    from scripts.terrain_gen.fields import SurfacePalette

    for key, fill_fn, zone in BASELINE_PROFILES:
        if key == profile_key:
            return fill_fn(zone, TILE, TILE_SIZE, SurfacePalette())
    raise KeyError(f"unknown baseline profile key {profile_key!r}")
