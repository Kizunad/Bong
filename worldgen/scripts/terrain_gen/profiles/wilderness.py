from __future__ import annotations

import math

import numpy as np

from ..fields import (
    DEFAULT_FIELD_LAYERS,
    Bounds2D,
    SurfacePalette,
    TileFieldBuffer,
    WildernessFieldPlan,
    WorldTile,
)
from ..noise import _tile_coords


def build_wilderness_base_plan(bounds_xz: Bounds2D) -> WildernessFieldPlan:
    return WildernessFieldPlan(
        profile_name="wilderness",
        bounds_xz=bounds_xz,
        required_layers=DEFAULT_FIELD_LAYERS,
        notes=(
            "Acts as the global fallback outside named zones.",
            "First-pass stitching targets zone-to-wilderness blending only.",
        ),
    )


def sample_wilderness_point(world_x: int, world_z: int) -> dict[str, float | int | str]:
    """Single-point sampler — kept for Rust parity tests. Do not change the math."""
    continental = (
        math.sin(world_x / 2400.0) * 8.5
        + math.cos(world_z / 2700.0) * 7.2
        + math.sin((world_x + world_z) / 3600.0) * 5.8
    )
    ridge = (
        math.sin(world_x / 680.0) * 4.2
        + math.cos(world_z / 760.0) * 3.6
        + math.sin((world_x - world_z) / 940.0) * 2.9
    )
    mountain = (
        math.sin(world_x / 1200.0) * math.cos(world_z / 1400.0) * 3.8
        + math.sin((world_x + world_z) / 1800.0) * 2.4
    )
    drainage = (
        0.5
        + math.sin(world_x / 520.0) * math.cos(world_z / 610.0) * 0.22
        + math.sin((world_x - world_z) / 870.0) * 0.16
        + math.cos((world_x + world_z) / 1040.0) * 0.12
    )
    scar = (
        0.5
        + math.sin((world_x + world_z) / 760.0)
        * math.cos((world_x - world_z) / 690.0)
        * 0.2
        + math.sin(world_x / 430.0) * math.cos(world_z / 470.0) * 0.14
    )

    height = 70.0 + continental * 4.0 + ridge * 3.5 + mountain * 6.0
    if drainage < 0.12:
        height -= (0.12 - drainage) * 8.0
    if scar > 0.82:
        height += (scar - 0.82) * 9.5

    roughness = abs(ridge) * 0.13 + abs(scar - 0.5) * 0.08
    feature_mask = min(1.0, 0.09 + abs(continental) * 0.08 + roughness * 0.58)

    if height < 76.0 and drainage > 0.18 and scar < 0.72:
        surface_name = "grass_block"
    elif drainage < 0.06 or scar > 0.84:
        surface_name = "gravel"
    elif roughness < 0.07:
        surface_name = "coarse_dirt"
    else:
        surface_name = "stone"

    return {
        "height": round(height, 3),
        "surface_name": surface_name,
        "subsurface_name": "stone",
        "water_level": -1.0,
        "biome_id": 8 if drainage < 0.09 else (7 if feature_mask > 0.2 else 0),
        "feature_mask": round(feature_mask, 3),
        "boundary_weight": 0.0,
    }


def fill_wilderness_tile(
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
    required_layers: tuple[str, ...],
) -> TileFieldBuffer:
    """Vectorized wilderness fill — same math as sample_wilderness_point."""
    buffer = TileFieldBuffer.create(tile, tile_size, required_layers)
    stone_id = palette.ensure("stone")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    grass_id = palette.ensure("grass_block")

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)

    continental = (
        np.sin(wx / 2400.0) * 8.5
        + np.cos(wz / 2700.0) * 7.2
        + np.sin((wx + wz) / 3600.0) * 5.8
    )
    ridge = (
        np.sin(wx / 680.0) * 4.2
        + np.cos(wz / 760.0) * 3.6
        + np.sin((wx - wz) / 940.0) * 2.9
    )
    mountain = (
        np.sin(wx / 1200.0) * np.cos(wz / 1400.0) * 3.8
        + np.sin((wx + wz) / 1800.0) * 2.4
    )
    drainage = (
        0.5
        + np.sin(wx / 520.0) * np.cos(wz / 610.0) * 0.22
        + np.sin((wx - wz) / 870.0) * 0.16
        + np.cos((wx + wz) / 1040.0) * 0.12
    )
    scar = (
        0.5
        + np.sin((wx + wz) / 760.0) * np.cos((wx - wz) / 690.0) * 0.2
        + np.sin(wx / 430.0) * np.cos(wz / 470.0) * 0.14
    )

    height = 70.0 + continental * 4.0 + ridge * 3.5 + mountain * 6.0
    height = np.where(drainage < 0.12, height - (0.12 - drainage) * 8.0, height)
    height = np.where(scar > 0.82, height + (scar - 0.82) * 9.5, height)

    roughness = np.abs(ridge) * 0.13 + np.abs(scar - 0.5) * 0.08
    feature_mask = np.minimum(1.0, 0.09 + np.abs(continental) * 0.08 + roughness * 0.58)

    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(
        (height < 76.0) & (drainage > 0.18) & (scar < 0.72), grass_id, surface_id
    )
    surface_id = np.where(roughness < 0.07, coarse_dirt_id, surface_id)
    surface_id = np.where((drainage < 0.06) | (scar > 0.84), gravel_id, surface_id)

    biome_id = np.full_like(height, 0, dtype=np.int32)
    biome_id = np.where(feature_mask > 0.2, 7, biome_id)
    biome_id = np.where(drainage < 0.09, 8, biome_id)

    buffer.layers["height"] = np.round(height, 3).ravel().tolist()
    buffer.layers["surface_id"] = surface_id.ravel().tolist()
    buffer.layers["subsurface_id"] = [stone_id] * (tile_size * tile_size)
    buffer.layers["water_level"] = [-1.0] * (tile_size * tile_size)
    buffer.layers["biome_id"] = biome_id.ravel().tolist()
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel().tolist()
    buffer.layers["boundary_weight"] = [0.0] * (tile_size * tile_size)

    # Zone-specific layers (rift_axis_sdf, cave_mask, etc.) are already
    # initialized to their safe defaults by TileFieldBuffer.create() via
    # LAYER_REGISTRY — no per-layer patching needed here.

    return buffer
