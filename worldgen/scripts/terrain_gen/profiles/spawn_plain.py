from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import ProfileContext, TerrainProfileGenerator


class SpawnPlainGenerator(TerrainProfileGenerator):
    profile_name = "spawn_plain"

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Low-relief onboarding terrain.",
            "Keep traversal readable and avoid major obstacles.",
        )


def fill_spawn_plain_tile(
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
    grass_id = palette.ensure("grass_block")
    podzol_id = palette.ensure("podzol")
    dirt_id = palette.ensure("dirt")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    stone_id = palette.ensure("stone")
    spawn_biome_id = 4
    flower_forest_biome_id = 11

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    heartland = np.maximum(0.0, 1.0 - radial**1.9)
    inner_meadow = np.maximum(0.0, 1.0 - radial**2.8)

    # Gentle rolling hills — large-scale FBM
    rolling = fbm_2d(wx, wz, scale=320.0, octaves=4, seed=10) * 2.3
    # Organic swale depressions — domain-warped for natural curves
    swale = warped_fbm_2d(
        wx, wz, scale=180.0, octaves=3, warp_scale=350.0, warp_strength=60.0, seed=20
    )
    # Path-like ridges
    path = fbm_2d(wx, wz, scale=220.0, octaves=3, seed=30)

    height = 69.0 + heartland * 3.8 + rolling * 0.8 - inner_meadow * 1.2
    # Occasional ponds in swale depressions
    pond_mask = (heartland > 0.14) & (swale < -0.55)
    water_level = np.where(pond_mask, 66.8, -1.0)
    height = np.where(pond_mask, height - (-0.55 - swale) * 4.0, height)

    # Surface
    surface_id = np.full_like(height, dirt_id, dtype=np.int32)
    surface_id = np.where(inner_meadow > 0.5, grass_id, surface_id)
    surface_id = np.where(
        (heartland > 0.34) & (np.abs(rolling) < 1.6), grass_id, surface_id
    )
    surface_id = np.where(swale < -0.6, coarse_dirt_id, surface_id)
    surface_id = np.where(np.abs(rolling) > 1.8, gravel_id, surface_id)
    surface_id = np.where(
        (water_level >= 0.0) & (height < water_level - 0.45), dirt_id, surface_id
    )
    surface_id = np.where((heartland > 0.56) & (path > 0.36), podzol_id, surface_id)

    feature_mask = np.minimum(
        1.0, 0.05 + (1.0 - inner_meadow) * 0.14 + np.abs(rolling) * 0.04
    )

    biome_id = np.where(feature_mask > 0.12, flower_forest_biome_id, spawn_biome_id)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.round(water_level, 3).ravel()
    buffer.layers["biome_id"] = biome_id.ravel().astype(np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)

    buffer.contributing_zones.append(zone.name)
    return buffer
