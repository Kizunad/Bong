from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import ProfileContext, TerrainProfileGenerator


class WastePlateauGenerator(TerrainProfileGenerator):
    profile_name = "waste_plateau"
    extra_layers = ("neg_pressure", "ruin_density")

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Prioritize emptiness and exposure over intricate landforms.",
            "Negative-pressure patches should be represented as field-level masks, not block-level hacks.",
        )


def fill_waste_plateau_tile(
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
            "neg_pressure",
            "ruin_density",
        ),
    )
    stone_id = palette.ensure("stone")
    gravel_id = palette.ensure("gravel")
    soul_sand_id = palette.ensure("soul_sand")
    bone_block_id = palette.ensure("bone_block")
    plateau_biome_id = 6

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)
    patches = zone.worldgen.extras.get("negative_pressure_patches", [])

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    plateau = np.maximum(0.0, 1.0 - radial**1.15)
    crown = np.maximum(0.0, 1.0 - radial**2.3)

    # Shelf undulation — broad, windswept feel
    shelf = fbm_2d(wx, wz, scale=400.0, octaves=4, seed=500) * 5.0
    # Fracture lines — warped for organic cracks
    fracture = warped_fbm_2d(
        wx, wz, scale=140.0, octaves=4, warp_scale=220.0, warp_strength=50.0, seed=510
    )
    # Scarp edges
    scarp = fbm_2d(wx, wz, scale=200.0, octaves=3, seed=520)

    height = 84.0 + plateau * 11.5 + crown * 4.2 + shelf
    # Fracture cutting
    fracture_cut = np.maximum(0.0, -fracture - 0.2) * 18.0
    height = height - fracture_cut

    # Negative pressure patches
    neg_pressure = np.zeros_like(height)
    for patch in patches:
        px, pz = patch["center_xz"]
        radius = max(float(patch["radius"]), 1.0)
        strength = abs(float(patch["strength"]))
        dist = np.sqrt((wx - px) ** 2 + (wz - pz) ** 2)
        falloff = np.maximum(0.0, 1.0 - dist / radius)
        neg_pressure = np.maximum(neg_pressure, strength * falloff * falloff)

    height = height - neg_pressure * 16.0

    ruin_density = np.minimum(
        1.0,
        0.12 + neg_pressure * 1.2 + (1.0 - (scarp * 0.5 + 0.5)) * 0.18 + crown * 0.08,
    )

    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(neg_pressure > 0.46, soul_sand_id, surface_id)
    surface_id = np.where(
        (fracture < -0.35) & (surface_id == stone_id),
        gravel_id,
        surface_id,
    )
    surface_id = np.where(
        (scarp < -0.45) & (surface_id == stone_id),
        soul_sand_id,
        surface_id,
    )
    surface_id = np.where(
        (scarp > 0.45) & (surface_id == stone_id),
        bone_block_id,
        surface_id,
    )

    feature_mask = np.minimum(
        1.0,
        plateau * 0.28 + neg_pressure * 0.95 + np.maximum(0.0, -fracture) * 0.4,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, plateau_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["neg_pressure"] = np.round(neg_pressure, 3).ravel()
    buffer.layers["ruin_density"] = np.round(ruin_density, 3).ravel()

    buffer.contributing_zones.append(zone.name)
    return buffer
