from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, ridge_2d, warped_fbm_2d
from .base import ProfileContext, TerrainProfileGenerator


class BrokenPeaksGenerator(TerrainProfileGenerator):
    profile_name = "broken_peaks"

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "High-relief massif with visible ridge lines.",
            "Favor rock exposure and narrow traversal corridors.",
        )


def fill_broken_peaks_tile(
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
    stone_id = palette.ensure("stone")
    andesite_id = palette.ensure("andesite")
    deepslate_id = palette.ensure("deepslate")
    calcite_id = palette.ensure("calcite")
    snow_id = palette.ensure("snow_block")
    ice_id = palette.ensure("packed_ice")
    gravel_id = palette.ensure("gravel")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    peaks_biome_id = 1

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    massif = np.maximum(0.0, 1.0 - radial**1.45)

    # --- Primary ridges: sharp, dramatic ---
    ridges = ridge_2d(
        wx, wz, scale=140.0, octaves=5, lacunarity=2.1, gain=0.48, seed=100
    )
    # Secondary ridge system at different orientation for complexity
    ridges2 = ridge_2d(wx, wz, scale=220.0, octaves=4, seed=110)
    # Large-scale base — broad doming
    base_fbm = fbm_2d(wx, wz, scale=500.0, octaves=3, seed=120)
    # Fine crag detail
    detail = fbm_2d(wx, wz, scale=45.0, octaves=3, seed=130)
    # Erosion channels — warped for organic gullies
    erosion = warped_fbm_2d(
        wx, wz, scale=80.0, octaves=4, warp_scale=150.0, warp_strength=35.0, seed=140
    )

    # --- Compose height with dramatic relief ---
    # Stronger massif uplift so the skyline actually reaches the new 512-height world.
    base_height = 82.0 + massif * 110.0 + base_fbm * 18.0

    # Primary ridges: drive the skyline.
    ridge_lift = np.maximum(0.0, ridges + 0.08) ** 1.18 * 185.0 * massif
    # Secondary ridges: broaden the crest field.
    ridge2_lift = np.maximum(0.0, ridges2 + 0.05) ** 1.08 * 82.0 * massif
    # Crag detail on peaks
    crag = np.maximum(0.0, detail) * 18.0 * massif

    # Deep valleys between ridges — the key to dramatic relief
    valley_cut = np.minimum(0.0, ridges) * 60.0 * massif
    valley_cut2 = np.minimum(0.0, ridges2) * 22.0 * massif

    # Erosion gullies on slopes
    erosion_cut = np.maximum(0.0, erosion - 0.1) * 32.0 * massif

    height = (
        base_height
        + ridge_lift
        + ridge2_lift
        + crag
        + valley_cut
        + valley_cut2
        - erosion_cut
    )

    # Clamp to stay within world bounds (min_y=-64, max_y=319)
    height = np.clip(height, -50.0, 460.0)

    # --- Surface: more variety based on height and slope ---
    ruggedness = np.abs(ridges) + np.abs(detail) * 0.7
    is_peak = height > 300.0
    is_high = height > 220.0
    is_valley = ridges < -0.3

    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(height > 285.0, snow_id, surface_id)
    surface_id = np.where((height > 270.0) & (detail > 0.42), ice_id, surface_id)
    surface_id = np.where(
        (height > 235.0) & (surface_id == stone_id), calcite_id, surface_id
    )
    # High exposed peaks: deepslate
    surface_id = np.where(
        is_peak & (ruggedness > 0.6) & (surface_id == stone_id),
        deepslate_id,
        surface_id,
    )
    # High slopes: andesite
    surface_id = np.where(
        is_high & (ruggedness > 0.8) & ~is_peak, andesite_id, surface_id
    )
    # Valley floors: gravel from erosion
    surface_id = np.where(is_valley, gravel_id, surface_id)
    # Eroded gullies: coarse dirt
    surface_id = np.where(
        (erosion > 0.4) & ~is_peak & ~is_valley,
        coarse_dirt_id,
        surface_id,
    )

    feature_mask = np.minimum(1.0, massif * 0.6 + ruggedness * 0.28)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel().tolist()
    buffer.layers["surface_id"] = surface_id.ravel().tolist()
    buffer.layers["subsurface_id"] = [stone_id] * area
    buffer.layers["water_level"] = [-1.0] * area
    buffer.layers["biome_id"] = [peaks_biome_id] * area
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel().tolist()
    buffer.layers["boundary_weight"] = [0.0] * area

    buffer.contributing_zones.append(zone.name)
    return buffer
