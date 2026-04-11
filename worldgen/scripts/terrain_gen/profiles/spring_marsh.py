from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import ProfileContext, TerrainProfileGenerator


class SpringMarshGenerator(TerrainProfileGenerator):
    profile_name = "spring_marsh"

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Low basin terrain with shallow water coverage.",
            "Traversal should feel fragmented by channels and islets.",
        )


def fill_spring_marsh_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    buffer = TileFieldBuffer.create(
        tile,
        tile_size,
        (
            "height",
            "surface_id",
            "subsurface_id",
            "water_level",
            "biome_id",
            "feature_mask",
            "boundary_weight",
        ),
    )
    mud_id = palette.ensure("mud")
    clay_id = palette.ensure("clay")
    grass_id = palette.ensure("grass_block")
    moss_id = palette.ensure("moss_block")
    rooted_dirt_id = palette.ensure("rooted_dirt")
    stone_id = palette.ensure("stone")
    dirt_id = palette.ensure("dirt")
    marsh_biome_id = 2

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    basin = np.maximum(0.0, 1.0 - radial**1.7)

    large_islands = warped_fbm_2d(
        wx, wz, scale=280.0, octaves=4, warp_scale=360.0, warp_strength=90.0, seed=200
    )
    medium_islands = warped_fbm_2d(
        wx, wz, scale=120.0, octaves=4, warp_scale=180.0, warp_strength=55.0, seed=210
    )
    small_islands = fbm_2d(wx, wz, scale=50.0, octaves=3, seed=220)
    channels = warped_fbm_2d(
        wx, wz, scale=90.0, octaves=5, warp_scale=250.0, warp_strength=70.0, seed=230
    )
    pools = warped_fbm_2d(
        wx, wz, scale=120.0, octaves=3, warp_scale=200.0, warp_strength=40.0, seed=240
    )
    rim = fbm_2d(wx, wz, scale=180.0, octaves=3, seed=250)

    island_noise = large_islands * 0.55 + medium_islands * 0.3 + small_islands * 0.15
    pool_depth = np.maximum(0.0, pools - 0.1) * 2.5
    in_basin = basin > 0.16

    # Water level must be well BELOW surrounding wilderness terrain (min ~54 at boundary).
    # Keeping a 10-block margin ensures stitching never pushes terrain below water.
    water_surface = 44.0

    # Noise > 0 → island (above water), noise < 0 → channel (below water)
    shore_t = np.clip(island_noise * 5.0 + 0.5, 0.0, 1.0)

    # Islands: 4-19 blocks above water — tall enough to be visible landmarks
    island_h = water_surface + 4.0 + np.abs(island_noise) * 14.0 + rim * 1.0
    # Water floor: 4-8 blocks below water — deep enough to feel like a lake
    floor_h = water_surface - 5.0 + channels * 2.0 - pool_depth

    # Smooth transition at shoreline
    height_basin = island_h * shore_t + floor_h * (1.0 - shore_t)
    # Rim sits above water, below wilderness — creates a natural bowl
    height_rim = 52.0 + rim * 1.5

    # Smooth basin-to-rim transition instead of hard cutoff
    basin_blend = np.clip((basin - 0.10) / 0.10, 0.0, 1.0)
    height = height_rim + (height_basin - height_rim) * basin_blend

    # Water only in basin interior where terrain dips below surface
    waterline = np.where((basin_blend > 0.25) & (height < water_surface), water_surface, -1.0)

    # Surface selection — relative to waterline
    submerged = (waterline >= 0) & (height < waterline)
    shoreline = (waterline >= 0) & (height >= waterline) & (height < waterline + 1.5)
    island_low = (waterline >= 0) & (height >= waterline + 1.5) & (height < waterline + 4.0)
    island_high = (waterline >= 0) & (height >= waterline + 4.0)

    surface_id = np.full_like(height, grass_id, dtype=np.int32)
    # Deep water bottom
    surface_id = np.where(submerged & (height < waterline - 2.0), clay_id, surface_id)
    # Shallow water bottom
    surface_id = np.where(
        submerged & (height >= waterline - 2.0), mud_id, surface_id
    )
    # Shoreline band
    surface_id = np.where(shoreline, mud_id, surface_id)
    # Low island: moss or rooted dirt
    surface_id = np.where(
        island_low & (channels > 0.0), moss_id, surface_id
    )
    surface_id = np.where(
        island_low & (channels <= 0.0), rooted_dirt_id, surface_id
    )
    # High island: grass
    surface_id = np.where(island_high, grass_id, surface_id)
    # Basin interior channels that are dry
    surface_id = np.where(
        (basin > 0.5) & (channels < -0.2) & (surface_id == grass_id),
        mud_id,
        surface_id,
    )

    feature_mask = np.maximum(
        np.maximum(basin, np.abs(channels) * 0.45),
        np.maximum(0.0, island_noise) * 0.8,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel().tolist()
    buffer.layers["surface_id"] = surface_id.ravel().tolist()
    buffer.layers["subsurface_id"] = [stone_id] * area
    buffer.layers["water_level"] = np.round(waterline, 3).ravel().tolist()
    buffer.layers["biome_id"] = [marsh_biome_id] * area
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel().tolist()
    buffer.layers["boundary_weight"] = [0.0] * area

    buffer.contributing_zones.append(zone.name)
    return buffer
