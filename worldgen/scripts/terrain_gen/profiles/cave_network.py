from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import ProfileContext, TerrainProfileGenerator


class CaveNetworkGenerator(TerrainProfileGenerator):
    profile_name = "cave_network"
    extra_layers = ("cave_mask", "ceiling_height", "entrance_mask")

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Surface should advertise underground space through sinkholes and entrances.",
            "Macro underground void layout is deferred to dedicated cave field generation.",
        )


def fill_cave_network_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    buffer = TileFieldBuffer.create(
        tile, tile_size,
        ("height", "surface_id", "subsurface_id", "water_level",
         "biome_id", "feature_mask", "boundary_weight",
         "cave_mask", "ceiling_height", "entrance_mask"),
    )
    stone_id = palette.ensure("stone")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    deepslate_id = palette.ensure("deepslate")
    cave_biome_id = 5

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    cluster = np.maximum(0.0, 1.0 - radial ** 1.55)

    # Sinkhole noise — warped for organic, not circular, shapes
    sinkhole = warped_fbm_2d(wx, wz, scale=130.0, octaves=4,
                             warp_scale=200.0, warp_strength=55.0, seed=400)
    # Cave connectivity — large-scale tunneling pattern
    crack = warped_fbm_2d(wx, wz, scale=100.0, octaves=5,
                          warp_scale=160.0, warp_strength=40.0, seed=410)
    # Surface undulation
    surface_fbm = fbm_2d(wx, wz, scale=200.0, octaves=3, seed=420)

    cave_mask = np.minimum(1.0, cluster * 0.78 + (crack * 0.5 + 0.5) * 0.34)
    entrance_mask = np.maximum(
        0.0,
        np.minimum(1.0, cluster * 0.75 - np.abs(sinkhole - 0.18) * 2.0),
    )

    sink_depth = entrance_mask * 14.0 + np.maximum(0.0, cave_mask - 0.68) * 9.0
    height = 75.0 + surface_fbm * 3.0 - sink_depth
    ceiling_height = np.maximum(10.0, 24.0 + cave_mask * 18.0 - entrance_mask * 7.0)

    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(entrance_mask > 0.52, gravel_id, surface_id)
    surface_id = np.where(
        (cave_mask > 0.78) & (surface_id == stone_id), deepslate_id, surface_id,
    )
    surface_id = np.where(
        (sinkhole < -0.3) & (surface_id == stone_id), coarse_dirt_id, surface_id,
    )

    feature_mask = np.minimum(1.0, cave_mask * 0.8 + entrance_mask * 0.35)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel().tolist()
    buffer.layers["surface_id"] = surface_id.ravel().tolist()
    buffer.layers["subsurface_id"] = [deepslate_id] * area
    buffer.layers["water_level"] = [-1.0] * area
    buffer.layers["biome_id"] = [cave_biome_id] * area
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel().tolist()
    buffer.layers["boundary_weight"] = [0.0] * area
    buffer.layers["cave_mask"] = np.round(cave_mask, 3).ravel().tolist()
    buffer.layers["ceiling_height"] = np.round(ceiling_height, 3).ravel().tolist()
    buffer.layers["entrance_mask"] = np.round(entrance_mask, 3).ravel().tolist()

    buffer.contributing_zones.append(zone.name)
    return buffer
