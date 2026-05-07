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
from ..noise import _tile_coords, fbm_2d


WILDERNESS_DECORATIONS_INFO = (
    "Generic wilderness flora fallback: oak/birch trees (variant 0 → Rust defaults), "
    "cobblestone boulders, grass & fern shrubs. Density encoded per-column in "
    "flora_density raster; variant_id stays 0 so Rust picks wilderness fallbacks."
)


def build_wilderness_base_plan(bounds_xz: Bounds2D) -> WildernessFieldPlan:
    return WildernessFieldPlan(
        profile_name="wilderness",
        bounds_xz=bounds_xz,
        required_layers=DEFAULT_FIELD_LAYERS
        + (
            "flora_density",
            "flora_variant_id",
            "ground_cover_density",
            "ground_cover_id",
        ),
        notes=(
            "Acts as the global fallback outside named zones.",
            "First-pass stitching targets zone-to-wilderness blending only.",
            f"Ecology: {WILDERNESS_DECORATIONS_INFO}",
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

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = biome_id.ravel().astype(np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)

    # Zone-specific layers (rift_axis_sdf, cave_mask, etc.) are already
    # initialized to their safe defaults by TileFieldBuffer.create() via
    # LAYER_REGISTRY — no per-layer patching needed here.

    # --- xianxia semantic baseline -------------------------------------
    # 末法残土 = 灵气普遍稀薄且大地普遍腐朽。用低频噪声做宏观"灵气云图"，
    # 让荒野自带起伏（避免全世界是一张死板的常量灵气图），zone overlay
    # 再在其上 lerp/maximum 叠加。
    if "qi_density" in buffer.layers:
        qi_cloud = fbm_2d(wx, wz, scale=2200.0, octaves=3, seed=901)  # [-1, 1]
        qi_density = np.clip(0.12 + qi_cloud * 0.08, 0.0, 1.0)
        buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()

    if "mofa_decay" in buffer.layers:
        decay_cloud = fbm_2d(wx, wz, scale=2600.0, octaves=3, seed=902)
        # 末法基线 ≈ 0.4，南北/东西方向有轻微梯度（北更腐朽）
        lat_gradient = np.clip(-wz / 12000.0, -0.25, 0.25)
        mofa_decay = np.clip(0.40 + decay_cloud * 0.12 + lat_gradient, 0.0, 1.0)
        buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()

    # qi_vein_flow 默认 0，荒野无灵脉，由 zone profile 显式生成。

    # --- Wilderness flora baseline ---
    # Sparse, generic flora — concrete variant id is 0 (zone profiles override
    # with their own palettes). Only flora_density carries information here;
    # Rust can fall back to generic oak/stone-boulder placement when it sees
    # variant_id == 0 within wilderness bounds.
    if "flora_density" in buffer.layers:
        flora_cloud = fbm_2d(wx, wz, scale=180.0, octaves=3, seed=903)
        # Density 0.05..0.45 across wilderness, biased toward grass/coarse surfaces.
        flora_density = np.clip(0.20 + flora_cloud * 0.18, 0.0, 0.55)
        # Thin out in gravel/scar zones.
        flora_density = np.where(scar > 0.82, flora_density * 0.3, flora_density)
        flora_density = np.where(drainage < 0.09, flora_density * 0.5, flora_density)
        buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()

    # --- Wilderness ground cover (草/蕨/蒲公英) ---
    # 与 flora 平行的地表植被层。density 高（0.4–0.75）让草甸"看着是
    # 草甸"，但在 gravel/scar 上压到 0。variant_id 引用 GLOBAL palette 的
    # wilderness 段：1=wild_grass, 2=wild_fern, 3=wild_dandelion（见
    # profiles/__init__.py WILDERNESS_GROUND_COVER 顺序）。
    if "ground_cover_density" in buffer.layers:
        from . import global_decoration_id

        gc_grass = global_decoration_id("wilderness", 1)
        gc_fern = global_decoration_id("wilderness", 2)
        gc_dandelion = global_decoration_id("wilderness", 3)

        gc_cloud = fbm_2d(wx, wz, scale=140.0, octaves=3, seed=911)
        gc_density = np.clip(0.55 + gc_cloud * 0.20, 0.0, 0.85)
        gc_density = np.where(scar > 0.82, 0.0, gc_density)
        gc_density = np.where(drainage < 0.09, gc_density * 0.4, gc_density)
        # 只在 grass/coarse 表面铺，gravel/stone 表面不长（surface_id 已经定）
        on_soft = (surface_id == grass_id) | (surface_id == coarse_dirt_id)
        gc_density = np.where(on_soft, gc_density, 0.0)
        buffer.layers["ground_cover_density"] = np.round(gc_density, 3).ravel()

        # Variant 选取：默认 grass，feature 高的地方掺 fern，零星 dandelion
        variant_cloud = fbm_2d(wx, wz, scale=80.0, octaves=2, seed=917)
        gc_variant = np.full_like(height, gc_grass, dtype=np.int32)
        gc_variant = np.where((feature_mask > 0.18) & (variant_cloud > 0.05), gc_fern, gc_variant)
        gc_variant = np.where(variant_cloud > 0.55, gc_dandelion, gc_variant)
        gc_variant = np.where(gc_density <= 0.0, 0, gc_variant)
        buffer.layers["ground_cover_id"] = gc_variant.ravel().astype(np.uint8)

    return buffer
