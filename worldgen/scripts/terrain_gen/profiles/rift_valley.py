from __future__ import annotations

import math

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, ridge_2d, warped_fbm_2d
from .base import ProfileContext, TerrainProfileGenerator


class RiftValleyGenerator(TerrainProfileGenerator):
    profile_name = "rift_valley"
    extra_layers = ("rift_axis_sdf", "rim_edge_mask", "fracture_mask")

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Primary macro landform is a long rift with hard boundary falloff.",
            "Later postprocess may deepen local chokes, but the main valley belongs to field generation.",
        )


def fill_rift_valley_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    layer_names = (
        "height",
        "surface_id",
        "subsurface_id",
        "water_level",
        "biome_id",
        "feature_mask",
        "boundary_weight",
        "rift_axis_sdf",
        "rim_edge_mask",
        "fracture_mask",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    blackstone_id = palette.ensure("blackstone")
    basalt_id = palette.ensure("basalt")
    magma_id = palette.ensure("magma_block")
    nylium_id = palette.ensure("crimson_nylium")
    stone_id = palette.ensure("stone")
    gravel_id = palette.ensure("gravel")
    rift_biome_id = 3

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_l = max(zone.size_xz[1] * 0.5, 1.0)
    angle = math.radians(-20.0)
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    fdx = wx - center_x
    fdz = wz - center_z
    along = fdx * cos_a - fdz * sin_a
    cross = fdx * sin_a + fdz * cos_a

    along_ratio = np.clip(along / half_l, -1.0, 1.0)
    cross_ratio = cross / half_w

    # Width variation along rift — FBM for organic wobble
    width_warp = fbm_2d(along, cross, scale=200.0, octaves=3, seed=300)
    width_noise = 1.0 + width_warp * 0.4
    normalized_cross = np.abs(cross_ratio) / np.maximum(width_noise, 0.35)

    axial_profile = np.maximum(0.0, 1.0 - np.abs(along_ratio) ** 1.25)

    # Branch canyons — warped noise for organic branching
    branch = warped_fbm_2d(
        wx, wz, scale=110.0, octaves=4, warp_scale=180.0, warp_strength=45.0, seed=310
    )
    valley_strength = np.maximum(0.0, 1.0 - normalized_cross**1.42) * axial_profile
    valley_strength = np.minimum(
        1.0, valley_strength + np.maximum(0.0, branch - 0.35) * 0.3
    )

    # Rim detail
    rim_fbm = fbm_2d(wx, wz, scale=130.0, octaves=4, seed=320)
    rim_height = 108.0 + 13.0 * axial_profile + rim_fbm * 5.0

    # Valley floor detail
    floor_fbm = fbm_2d(along, cross, scale=160.0, octaves=4, seed=330)
    floor_height = 40.0 + floor_fbm * 8.0

    height = rim_height - (rim_height - floor_height) * valley_strength
    # Branch canyon carving
    branch_cut = (
        np.maximum(0.0, branch - 0.5)
        * 22.0
        * np.where(normalized_cross < 1.2, 1.0, 0.0)
    )
    height = height - branch_cut

    # Surface
    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(valley_strength > 0.88, magma_id, surface_id)
    surface_id = np.where(
        (valley_strength > 0.72) & (valley_strength <= 0.88),
        blackstone_id,
        surface_id,
    )
    surface_id = np.where(
        (valley_strength > 0.52) & (valley_strength <= 0.72),
        basalt_id,
        surface_id,
    )
    surface_id = np.where(
        (normalized_cross > 0.98) & (surface_id == stone_id),
        gravel_id,
        surface_id,
    )
    surface_id = np.where(
        (rim_fbm > 0.55) & (surface_id == stone_id),
        gravel_id,
        surface_id,
    )

    rim_edge_mask = np.maximum(0.0, 1.0 - np.abs(normalized_cross - 1.0) * 3.6)
    fracture_noise = ridge_2d(wx, wz, scale=60.0, octaves=5, seed=340)
    fracture_mask = np.where(
        valley_strength > 0.4,
        np.maximum(0.0, fracture_noise) * valley_strength,
        0.0,
    )
    surface_id = np.where(
        (fracture_mask > 0.7) & (surface_id == stone_id), nylium_id, surface_id
    )
    feature_mask = np.maximum(valley_strength, rim_edge_mask * 0.72)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, rift_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["rift_axis_sdf"] = np.round(normalized_cross, 3).ravel()
    buffer.layers["rim_edge_mask"] = np.round(rim_edge_mask, 3).ravel()
    buffer.layers["fracture_mask"] = np.round(fracture_mask, 3).ravel()

    buffer.contributing_zones.append(zone.name)
    return buffer
