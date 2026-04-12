from __future__ import annotations

import math
from pathlib import Path

import numpy as np

from .blueprint import BlueprintZone, TerrainProfileCatalog, WorldBlueprint
from .fields import (
    LAYER_REGISTRY,
    GeneratedFieldSet,
    SurfacePalette,
    TerrainGenerationPlan,
    TileFieldBuffer,
    WorldTile,
    build_world_tiles,
)
from .noise import coherent_noise_2d, _tile_coords
from .profiles import ProfileContext, get_profile_generator
from .profiles.broken_peaks import fill_broken_peaks_tile
from .profiles.cave_network import fill_cave_network_tile
from .profiles.spawn_plain import fill_spawn_plain_tile
from .profiles.rift_valley import fill_rift_valley_tile
from .profiles.spring_marsh import fill_spring_marsh_tile
from .profiles.waste_plateau import fill_waste_plateau_tile
from .profiles.wilderness import build_wilderness_base_plan, fill_wilderness_tile


def build_generation_plan(
    blueprint: WorldBlueprint,
    profile_catalog: TerrainProfileCatalog,
    blueprint_path: Path,
    profiles_path: Path,
    output_dir: Path,
    tile_size: int,
) -> TerrainGenerationPlan:
    zone_plans = []
    for zone in blueprint.zones:
        profile_name = zone.worldgen.terrain_profile
        if profile_name not in profile_catalog.profiles:
            raise KeyError(
                f"Blueprint zone '{zone.name}' references unknown profile '{profile_name}'"
            )

        generator = get_profile_generator(profile_name)
        zone_plans.append(
            generator.plan(
                ProfileContext(
                    zone=zone,
                    profile_spec=profile_catalog.profiles[profile_name],
                )
            )
        )

    return TerrainGenerationPlan(
        world_name=blueprint.world_name,
        blueprint_path=blueprint_path,
        profiles_path=profiles_path,
        output_dir=output_dir,
        world_bounds=blueprint.bounds_xz,
        tile_size=tile_size,
        tiles=build_world_tiles(blueprint.bounds_xz, tile_size),
        wilderness=build_wilderness_base_plan(blueprint.bounds_xz),
        blueprint_zones=list(blueprint.zones),
        zone_plans=zone_plans,
        stitch_strategy="zone_to_wilderness_distance_falloff_v1",
        notes=(
            "This scaffold builds metadata and execution order only.",
            "Actual field synthesis and blending are the next implementation step.",
        ),
    )


# ---------------------------------------------------------------------------
# Vectorized boundary weight computation
# ---------------------------------------------------------------------------


def _coherent_noise_2d_array(
    wx: np.ndarray, wz: np.ndarray, scale: float, seed: int
) -> np.ndarray:
    """Vectorized coherent_noise_2d — must match scalar version exactly."""
    sx = wx / max(scale, 1.0)
    sz = wz / max(scale, 1.0)
    sp = seed * 0.017
    return (
        np.sin(sx * 1.17 + sz * 0.83 + sp) * 0.5
        + np.cos(sx * -0.71 + sz * 1.29 - sp * 1.3) * 0.3
        + np.sin(sx * 2.03 - sz * 1.61 + sp * 0.7) * 0.2
    )


def _shape_membership_ratio_array(
    zone: BlueprintZone,
    wx: np.ndarray,
    wz: np.ndarray,
) -> np.ndarray:
    shape = zone.worldgen.shape
    center_x, center_z = zone.center_xz
    half_width = max(zone.size_xz[0] * 0.5, 1.0)
    half_depth = max(zone.size_xz[1] * 0.5, 1.0)
    edge_noise = _coherent_noise_2d_array(wx, wz, scale=420.0, seed=17)
    edge_warp = 1.0 + edge_noise * 0.12

    if shape in {"ellipse", "massif", "basin", "plateau", "subterranean_cluster"}:
        dx = (wx - center_x) / (half_width * edge_warp)
        dz = (wz - center_z) / (half_depth * (1.0 - edge_noise * 0.08))
        return np.sqrt(dx * dx + dz * dz)

    if shape == "rotated_rift":
        angle = math.radians(-20.0)
        cos_a = math.cos(angle)
        sin_a = math.sin(angle)
        fdx = wx - center_x
        fdz = wz - center_z
        along = fdx * cos_a - fdz * sin_a
        cross = fdx * sin_a + fdz * cos_a
        cross_warp = 1.0 + edge_noise * 0.16
        along_warp = 1.0 - edge_noise * 0.06
        return np.maximum(
            np.abs(along) / (half_depth * along_warp),
            np.abs(cross) / (half_width * cross_warp),
        )

    bounds = zone.bounds_xz
    in_bounds = (
        (wx >= bounds.min_x)
        & (wx <= bounds.max_x)
        & (wz >= bounds.min_z)
        & (wz <= bounds.max_z)
    )
    dist_left = wx - bounds.min_x
    dist_right = bounds.max_x - wx
    dist_top = wz - bounds.min_z
    dist_bottom = bounds.max_z - wz
    min_dist = np.minimum(
        np.minimum(dist_left, dist_right), np.minimum(dist_top, dist_bottom)
    )
    result = np.where(in_bounds, np.maximum(0.0, 1.0 - min_dist), np.inf)
    return result


def _compute_boundary_weight_array(
    zone: BlueprintZone,
    wx: np.ndarray,
    wz: np.ndarray,
) -> np.ndarray:
    def smoothstep01(value: np.ndarray) -> np.ndarray:
        clamped = np.clip(value, 0.0, 1.0)
        return clamped * clamped * (3.0 - 2.0 * clamped)

    width = max(float(zone.worldgen.boundary.width), 1.0)
    ratio = _shape_membership_ratio_array(zone, wx, wz)
    blend_ratio = width / max(min(zone.size_xz) * 0.5, 1.0)
    outer_limit = 1.0 + blend_ratio
    mode = zone.worldgen.boundary.mode

    # Interior: ratio <= 1.0
    interior_t = np.clip((1.0 - ratio) / max(blend_ratio, 0.001), 0.0, 1.0)
    smooth_t = smoothstep01(interior_t)
    if mode == "hard":
        interior_weight = 0.55 + smooth_t * 0.45
    elif mode == "semi_hard":
        interior_weight = 0.35 + smooth_t * 0.65
    else:
        interior_weight = 0.2 + smooth_t * 0.8

    # Exterior: ratio > 1.0 and <= outer_limit
    outer_t = np.clip((outer_limit - ratio) / max(blend_ratio, 0.001), 0.0, 1.0)
    smooth_outer = smoothstep01(outer_t)
    if mode == "hard":
        exterior_weight = smooth_outer * 0.6
    elif mode == "semi_hard":
        exterior_weight = smooth_outer * 0.45
    else:
        exterior_weight = smooth_outer * 0.35

    weight = np.where(ratio <= 1.0, interior_weight, exterior_weight)
    weight = np.where(ratio > outer_limit, 0.0, weight)
    return weight


# ---------------------------------------------------------------------------
# Vectorized tile blending
# ---------------------------------------------------------------------------


def _blend_tile_layers(
    base_tile: TileFieldBuffer,
    overlay_tile: TileFieldBuffer,
    zone: BlueprintZone,
) -> None:
    tile_size = base_tile.tile_size
    wx, wz = _tile_coords(base_tile.tile.min_x, base_tile.tile.min_z, tile_size)
    weight = _compute_boundary_weight_array(zone, wx, wz).ravel()

    active = weight > 0.0
    if not np.any(active):
        if zone.name not in base_tile.contributing_zones:
            base_tile.contributing_zones.append(zone.name)
        return

    # Convert layers to numpy for fast blending
    transition_noise = _coherent_noise_2d_array(wx, wz, scale=84.0, seed=71).ravel()
    transition_band = np.clip(1.0 - np.abs(weight - 0.5) * 2.0, 0.0, 1.0)
    height_weight = np.clip(
        weight + transition_noise * 0.12 * transition_band, 0.0, 1.0
    )

    base_height = np.array(base_tile.layers["height"], dtype=np.float64)
    overlay_height = np.array(overlay_tile.layers["height"], dtype=np.float64)
    blended_height = base_height + (overlay_height - base_height) * height_weight
    base_tile.layers["height"] = np.round(blended_height, 3).tolist()

    # Discrete layers: dither the transition instead of cutting at a fixed threshold.
    swap_threshold = np.clip(0.5 + transition_noise * 0.18 * transition_band, 0.2, 0.8)
    swap = weight >= swap_threshold
    for layer_name in ("surface_id", "subsurface_id"):
        if layer_name in overlay_tile.layers:
            base_arr = np.array(base_tile.layers[layer_name])
            overlay_arr = np.array(overlay_tile.layers[layer_name])
            base_tile.layers[layer_name] = np.where(
                swap, overlay_arr, base_arr
            ).tolist()

    if "biome_id" in overlay_tile.layers:
        base_arr = np.array(base_tile.layers["biome_id"])
        overlay_arr = np.array(overlay_tile.layers["biome_id"])
        biome_swap = weight >= np.maximum(0.55, swap_threshold)
        base_tile.layers["biome_id"] = np.where(
            biome_swap, overlay_arr, base_arr
        ).tolist()

    # Water level
    base_water = np.array(base_tile.layers["water_level"], dtype=np.float64)
    overlay_water = np.array(overlay_tile.layers["water_level"], dtype=np.float64)
    has_overlay_water = overlay_water >= 0.0
    no_base_water = base_water < 0.0
    blended_water = np.where(
        has_overlay_water & no_base_water,
        np.where(weight >= 0.5, overlay_water, -1.0),
        np.where(
            has_overlay_water,
            base_water + (overlay_water - base_water) * height_weight,
            base_water,
        ),
    )
    # Remove water where blended terrain is above water level (stitching raised it)
    blended_height_final = np.array(base_tile.layers["height"], dtype=np.float64)
    blended_water = np.where(
        (blended_water >= 0) & (blended_height_final >= blended_water),
        -1.0,
        blended_water,
    )
    base_tile.layers["water_level"] = np.round(blended_water, 3).tolist()

    # Feature mask
    base_feature = np.array(base_tile.layers["feature_mask"], dtype=np.float64)
    overlay_feature = np.array(overlay_tile.layers["feature_mask"], dtype=np.float64)
    base_tile.layers["feature_mask"] = np.round(
        np.maximum(base_feature, overlay_feature * weight), 3
    ).tolist()

    # Boundary weight
    base_bw = np.array(base_tile.layers["boundary_weight"], dtype=np.float64)
    base_tile.layers["boundary_weight"] = np.round(
        np.maximum(base_bw, weight), 3
    ).tolist()

    # Extra layers
    for extra_layer in overlay_tile.layers:
        if extra_layer in (
            "height",
            "surface_id",
            "subsurface_id",
            "biome_id",
            "water_level",
            "feature_mask",
            "boundary_weight",
        ):
            continue
        if extra_layer in base_tile.layers:
            base_arr = np.array(base_tile.layers[extra_layer], dtype=np.float64)
            overlay_arr = np.array(overlay_tile.layers[extra_layer], dtype=np.float64)
            spec = LAYER_REGISTRY.get(extra_layer)
            blend = spec.blend_mode if spec else "maximum"
            if blend == "minimum":
                blended = np.minimum(base_arr, overlay_arr)
            else:  # "maximum" (default for extra layers)
                blended = np.maximum(base_arr, overlay_arr * weight)
            base_tile.layers[extra_layer] = np.round(blended, 3).tolist()

    if zone.name not in base_tile.contributing_zones:
        base_tile.contributing_zones.append(zone.name)


# ---------------------------------------------------------------------------
# Zone dispatch and synthesis
# ---------------------------------------------------------------------------


def _shape_membership_ratio(zone: BlueprintZone, world_x: int, world_z: int) -> float:
    """Scalar version — kept for any remaining scalar callers."""
    shape = zone.worldgen.shape
    center_x, center_z = zone.center_xz
    half_width = max(zone.size_xz[0] * 0.5, 1.0)
    half_depth = max(zone.size_xz[1] * 0.5, 1.0)
    edge_noise = coherent_noise_2d(world_x, world_z, scale=420.0, seed=17)
    edge_warp = 1.0 + edge_noise * 0.12

    if shape in {"ellipse", "massif", "basin", "plateau", "subterranean_cluster"}:
        dx = (world_x - center_x) / (half_width * edge_warp)
        dz = (world_z - center_z) / (half_depth * (1.0 - edge_noise * 0.08))
        return math.sqrt(dx * dx + dz * dz)

    if shape == "rotated_rift":
        angle = math.radians(-20.0)
        cos_angle = math.cos(angle)
        sin_angle = math.sin(angle)
        dx = world_x - center_x
        dz = world_z - center_z
        along = dx * cos_angle - dz * sin_angle
        cross = dx * sin_angle + dz * cos_angle
        cross_warp = 1.0 + edge_noise * 0.16
        along_warp = 1.0 - edge_noise * 0.06
        return max(
            abs(along) / (half_depth * along_warp),
            abs(cross) / (half_width * cross_warp),
        )

    bounds = zone.bounds_xz
    if not (
        bounds.min_x <= world_x <= bounds.max_x
        and bounds.min_z <= world_z <= bounds.max_z
    ):
        return float("inf")

    dist_left = world_x - bounds.min_x
    dist_right = bounds.max_x - world_x
    dist_top = world_z - bounds.min_z
    dist_bottom = bounds.max_z - world_z
    return max(0.0, 1.0 - min(dist_left, dist_right, dist_top, dist_bottom))


def _zone_intersects_tile(zone: BlueprintZone, tile: WorldTile) -> bool:
    expanded_bounds = zone.bounds_xz.expanded(zone.worldgen.boundary.width)
    return expanded_bounds.intersects(tile.bounds)


def _build_zone_overlay_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer | None:
    if zone.worldgen.terrain_profile == "spawn_plain":
        return fill_spawn_plain_tile(zone, tile, tile_size, palette)
    if zone.worldgen.terrain_profile == "broken_peaks":
        return fill_broken_peaks_tile(zone, tile, tile_size, palette)
    if zone.worldgen.terrain_profile == "spring_marsh":
        return fill_spring_marsh_tile(zone, tile, tile_size, palette)
    if zone.worldgen.terrain_profile == "rift_valley":
        return fill_rift_valley_tile(zone, tile, tile_size, palette)
    if zone.worldgen.terrain_profile == "cave_network":
        return fill_cave_network_tile(zone, tile, tile_size, palette)
    if zone.worldgen.terrain_profile == "waste_plateau":
        return fill_waste_plateau_tile(zone, tile, tile_size, palette)
    return None


def synthesize_fields(plan: TerrainGenerationPlan) -> GeneratedFieldSet:
    palette = SurfacePalette()
    palette.extend(("stone", "coarse_dirt", "gravel"))

    all_layers = list(plan.wilderness.required_layers)
    for zone_plan in plan.zone_plans:
        for layer_name in zone_plan.required_layers:
            if layer_name not in all_layers:
                all_layers.append(layer_name)

    generated_tiles: list[TileFieldBuffer] = []
    active_tiles = [
        tile
        for tile in plan.tiles
        if any(_zone_intersects_tile(zone, tile) for zone in plan.blueprint_zones)
    ]

    for tile in active_tiles:
        base_tile = fill_wilderness_tile(
            tile, plan.tile_size, palette, tuple(all_layers)
        )
        for zone in plan.blueprint_zones:
            if not _zone_intersects_tile(zone, tile):
                continue
            overlay_tile = _build_zone_overlay_tile(zone, tile, plan.tile_size, palette)
            if overlay_tile is None:
                continue
            _blend_tile_layers(base_tile, overlay_tile, zone)
        generated_tiles.append(base_tile)

    return GeneratedFieldSet(
        tile_size=plan.tile_size,
        surface_palette=palette,
        layers=tuple(all_layers),
        tiles=generated_tiles,
        notes=(
            "Implemented: wilderness base synthesis.",
            "Implemented: spawn_plain overlay synthesis.",
            "Implemented: broken_peaks overlay synthesis.",
            "Implemented: spring_marsh overlay synthesis.",
            "Implemented: rift_valley overlay synthesis and zone-to-wilderness blending.",
            "Implemented: cave_network surface proxy synthesis.",
            "Implemented: waste_plateau overlay synthesis.",
            "Only active tiles intersecting named zones are synthesized in this scaffold stage.",
        ),
    )
