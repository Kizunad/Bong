from __future__ import annotations

import math
from pathlib import Path

import numpy as np

from .blueprint import (
    BlueprintZone,
    TerrainProfileCatalog,
    WorldBlueprint,
    ZoneOverlaySpec,
)
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
from .profiles import PROFILE_DECORATION_OFFSETS, ProfileContext, get_profile_generator
from .profiles.ash_dead_zone import fill_ash_dead_zone_tile
from .profiles.abyssal_maze import fill_abyssal_maze_tile
from .profiles.ancient_battlefield import fill_ancient_battlefield_tile
from .profiles.broken_peaks import fill_broken_peaks_tile
from .profiles.cave_network import fill_cave_network_tile
from .profiles.dan_zong_yi_yuan import fill_dan_zong_yi_yuan_tile
from .profiles.jiu_zong_ruin import fill_jiu_zong_ruin_tile
from .profiles.spawn_plain import fill_spawn_plain_tile
from .profiles.pseudo_vein_oasis import fill_pseudo_vein_oasis_tile
from .profiles.rift_mouth_barrens import fill_rift_mouth_barrens_tile
from .profiles.rift_valley import fill_rift_valley_tile
from .profiles.sky_isle import fill_sky_isle_tile
from .profiles.spring_marsh import fill_spring_marsh_tile
from .profiles.tribulation_scorch import fill_tribulation_scorch_tile
from .profiles.tsy_daneng_crater import fill_tsy_daneng_crater_tile
from .profiles.tsy_gaoshou_hermitage import fill_tsy_gaoshou_hermitage_tile
from .profiles.tsy_zhanchang import fill_tsy_zhanchang_tile
from .profiles.tsy_zongmen_ruin import fill_tsy_zongmen_ruin_tile
from .profiles.wangyintai import fill_wangyintai_tile
from .profiles.waste_plateau import fill_waste_plateau_tile
from .profiles.wilderness import build_wilderness_base_plan, fill_wilderness_tile


def build_generation_plan(
    blueprint: WorldBlueprint,
    profile_catalog: TerrainProfileCatalog,
    blueprint_path: Path,
    profiles_path: Path,
    output_dir: Path,
    tile_size: int,
    zone_overlays: tuple[ZoneOverlaySpec, ...] = (),
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
        zone_overlays=list(zone_overlays),
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

    if shape in {"ellipse", "circular", "massif", "basin", "plateau", "subterranean_cluster", "irregular_blob"}:
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


# worldgen-v4 P0 §8.1 #1: blend modes for the legacy vertical patch layers that
# were removed from LAYER_REGISTRY (folded into spans) but are still emitted by
# the unrewritten profiles and consumed by the shim.  Mirrors the modes those
# layers carried in v3 so the span fold reproduces the v3 landscape verbatim.
SHIM_PATCH_BLEND_MODES: dict[str, str] = {
    "cave_mask": "maximum",
    "ceiling_height": "maximum",
    "entrance_mask": "maximum",
    "sky_island_base_y": "minimum",       # coordinate sentinel 9999 preserved
    "sky_island_thickness": "maximum",
    "cavern_floor_y": "minimum",          # coordinate sentinel 9999 preserved
}


def blend_spans(
    base_spans: tuple[tuple[int, int], ...],
    overlay_spans: tuple[tuple[int, int], ...],
    weight: float,
) -> tuple[tuple[int, int], ...]:
    """Merge a wilderness *base* column with a zone *overlay* column by weight.

    The span representation's blend semantic (§8.1 #1 落点): the overlay's
    ground span端点 is lerped toward the base's by ``1 - weight`` (so the zone
    fully owns its interior at weight 1 and fades to wilderness at weight 0);
    overlay-only features (floating isles / extra cave-floor remnants) appear
    once weight crosses 0.5 — the same dithered cut the discrete layers use —
    and disappear below it.  The result is always a legal, non-overlapping span
    list with span[0] as the surface.

    This is a pure helper so the span-blend rule is unit-pinnable independently
    of the numpy tile machinery; the per-column 2D layer blend in
    ``_blend_tile_layers`` feeds the same landscape into the export-time fold.
    """
    from .fields import ColumnSpans

    weight = max(0.0, min(1.0, weight))
    if not overlay_spans:
        return base_spans
    if not base_spans:
        # No wilderness ground here — only adopt the overlay once we are mostly
        # inside the zone, else keep the void.
        return overlay_spans if weight >= 0.5 else ()

    # The column's vertical STRUCTURE (caves below / isles above) belongs to the
    # dominant side — crossing the 0.5 dither line hands the structure to the
    # zone overlay.  Only the surface ceiling lerps continuously so the ground
    # height transitions smoothly across the boundary instead of stepping.
    structural = overlay_spans if weight >= 0.5 else base_spans
    base_ceiling = base_spans[0][1]
    ov_ceiling = overlay_spans[0][1]
    blended_ceiling = round(base_ceiling + (ov_ceiling - base_ceiling) * weight)

    struct_floor = structural[0][0]
    # Guard: the lerped surface must stay above the structural span's floor.
    blended_ceiling = max(blended_ceiling, struct_floor)
    merged: list[tuple[int, int]] = [(struct_floor, blended_ceiling)]

    # Keep the dominant side's extra spans (cave floor remnant, floating isle),
    # dropping any that would collide with the lerped surface span.
    for span in structural[1:]:
        if len(merged) >= 4:
            break
        floor_y, ceiling_y = span
        if floor_y <= blended_ceiling + 1 and ceiling_y >= struct_floor - 1:
            continue
        merged.append(span)

    return ColumnSpans(tuple(merged)).spans


def _blend_tile_layers(
    base_tile: TileFieldBuffer,
    overlay_tile: TileFieldBuffer,
    zone: BlueprintZone,
) -> None:
    tile_size = base_tile.tile_size
    wx, wz = _tile_coords(base_tile.tile.min_x, base_tile.tile.min_z, tile_size)
    weight = _compute_boundary_weight_array(zone, wx, wz).ravel()

    if not np.any(weight > 0.0):
        if zone.name not in base_tile.contributing_zones:
            base_tile.contributing_zones.append(zone.name)
        return

    transition_noise = _coherent_noise_2d_array(wx, wz, scale=84.0, seed=71).ravel()
    transition_band = np.clip(1.0 - np.abs(weight - 0.5) * 2.0, 0.0, 1.0)
    height_weight = np.clip(
        weight + transition_noise * 0.12 * transition_band, 0.0, 1.0
    )

    # Layers are stored as ndarrays already; operate in place where possible.
    base_height = base_tile.layers["height"]
    overlay_height = overlay_tile.layers["height"]
    blended_height = base_height + (overlay_height - base_height) * height_weight
    np.round(blended_height, 3, out=blended_height)
    base_tile.layers["height"] = blended_height

    # Discrete layers: dither the transition instead of cutting at a fixed threshold.
    swap_threshold = np.clip(0.5 + transition_noise * 0.18 * transition_band, 0.2, 0.8)
    swap = weight >= swap_threshold
    for layer_name in ("surface_id", "subsurface_id"):
        if layer_name in overlay_tile.layers:
            base_tile.layers[layer_name] = np.where(
                swap, overlay_tile.layers[layer_name], base_tile.layers[layer_name]
            )

    if "biome_id" in overlay_tile.layers:
        biome_swap = weight >= np.maximum(0.55, swap_threshold)
        base_tile.layers["biome_id"] = np.where(
            biome_swap,
            overlay_tile.layers["biome_id"],
            base_tile.layers["biome_id"],
        )

    # Water level
    base_water = base_tile.layers["water_level"]
    overlay_water = overlay_tile.layers["water_level"]
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
    blended_water = np.where(
        (blended_water >= 0) & (base_tile.layers["height"] >= blended_water),
        -1.0,
        blended_water,
    )
    np.round(blended_water, 3, out=blended_water)
    base_tile.layers["water_level"] = blended_water

    # Feature mask
    base_feature = base_tile.layers["feature_mask"]
    overlay_feature = overlay_tile.layers["feature_mask"]
    blended_feature = np.maximum(base_feature, overlay_feature * weight)
    np.round(blended_feature, 3, out=blended_feature)
    base_tile.layers["feature_mask"] = blended_feature

    # Boundary weight
    base_bw = base_tile.layers["boundary_weight"]
    blended_bw = np.maximum(base_bw, weight)
    np.round(blended_bw, 3, out=blended_bw)
    base_tile.layers["boundary_weight"] = blended_bw

    # Extra layers
    core_layers = {
        "height",
        "surface_id",
        "subsurface_id",
        "biome_id",
        "water_level",
        "feature_mask",
        "boundary_weight",
    }
    for extra_layer, overlay_arr in overlay_tile.layers.items():
        if extra_layer in core_layers or extra_layer not in base_tile.layers:
            continue
        base_arr = base_tile.layers[extra_layer]
        spec = LAYER_REGISTRY.get(extra_layer)
        # worldgen-v4 P0: cave_mask / ceiling_height / sky_island_base_y etc.
        # are no longer in LAYER_REGISTRY (folded into spans at export) but the
        # shim still reads them per column.  Blend them with their ORIGINAL v3
        # modes so the folded spans reproduce the v3 landscape across zone
        # boundaries — a coordinate layer like sky_island_base_y must keep its
        # `minimum` blend (sentinel 9999 preserved), not default to maximum.
        if spec is not None:
            blend = spec.blend_mode
        else:
            blend = SHIM_PATCH_BLEND_MODES.get(extra_layer, "maximum")
        if blend == "minimum":
            blended = np.minimum(base_arr, overlay_arr)
        elif blend == "lerp":
            # Smooth interpolation — overlay can raise OR lower base by `weight`.
            # Use float ops even if arrays came in as other dtypes.
            blended = base_arr + (overlay_arr - base_arr) * weight
        elif blend == "swap":
            # Discrete-id layers (flora_variant_id / ground_cover_id /
            # mineral_kind / anomaly_kind / *_origin_id 等). Hard pick overlay
            # vs base via the same dithered `swap` mask used for surface_id —
            # never multiply or maximum integer ids (would corrupt the global
            # palette index by mixing zones together).
            blended = np.where(swap, overlay_arr, base_arr).astype(base_arr.dtype)
        else:  # "maximum" (default for extra layers)
            blended = np.maximum(base_arr, overlay_arr * weight)
        if blend != "swap":
            np.round(blended, 3, out=blended)
        base_tile.layers[extra_layer] = blended

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

    if shape in {"ellipse", "circular", "massif", "basin", "plateau", "subterranean_cluster", "irregular_blob"}:
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


def _collapsed_zone_names(overlays: list[ZoneOverlaySpec]) -> set[str]:
    collapsed = set()
    for overlay in overlays:
        if overlay.overlay_kind != "collapsed":
            continue
        if overlay.payload.get("zone_status") == "collapsed":
            collapsed.add(overlay.zone_id)
    return collapsed


def _apply_realm_collapse_mask(
    buffer: TileFieldBuffer, zone: BlueprintZone
) -> None:
    if "realm_collapse_mask" not in buffer.layers:
        return
    wx, wz = _tile_coords(buffer.tile.min_x, buffer.tile.min_z, buffer.tile_size)
    weight = _compute_boundary_weight_array(zone, wx, wz).ravel()
    buffer.layers["realm_collapse_mask"] = np.maximum(
        buffer.layers["realm_collapse_mask"],
        (weight > 0.0).astype(np.uint8),
    )


def _compute_circular_mask(
    buffer: TileFieldBuffer,
    poi_center_xz: tuple[int, int],
    radius: int,
) -> np.ndarray:
    """Return a 1-D boolean mask marking columns within *radius* of a POI."""
    tile = buffer.tile
    tile_size = buffer.tile_size
    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    cx, cz = poi_center_xz
    dist_sq = (wx - cx) ** 2 + (wz - cz) ** 2
    return (dist_sq < radius * radius).ravel()


def apply_compound_flatten(
    buffer: TileFieldBuffer,
    poi_center_xz: tuple[int, int],
    radius: int,
    target_height: float,
) -> None:
    """Flatten the height field to *target_height* within *radius* of a POI.

    Provides a level platform for deterministic building layouts.
    Outside the radius the height field is untouched.
    """
    if radius <= 0:
        return
    inside = _compute_circular_mask(buffer, poi_center_xz, radius)

    if "height" in buffer.layers:
        buffer.layers["height"] = np.where(
            inside, target_height, buffer.layers["height"]
        )


# Layers zeroed inside layout-flatten radius, derived from LAYER_REGISTRY.
_DENSITY_MASKABLE_LAYERS: tuple[str, ...] = tuple(
    name for name, spec in LAYER_REGISTRY.items() if spec.density_maskable
)


def compute_layout_density_mask(
    buffer: TileFieldBuffer,
    poi_center_xz: tuple[int, int],
    radius: int,
) -> None:
    """Zero out density-spawned decoration layers within *radius* of a POI.

    This prevents density-spawned vegetation from growing on top of
    deterministic building layouts.  Affected layers are derived from
    ``LAYER_REGISTRY`` entries with ``density_maskable=True``.

    Uses a circular SDF: distance(column, POI center) < radius => masked.
    """
    if radius <= 0:
        return
    inside = _compute_circular_mask(buffer, poi_center_xz, radius)

    for layer_name in _DENSITY_MASKABLE_LAYERS:
        if layer_name in buffer.layers:
            buffer.layers[layer_name] = np.where(
                inside, 0, buffer.layers[layer_name]
            )


def _remap_flora_variant_to_global(
    buffer: TileFieldBuffer, profile_name: str
) -> None:
    """Remap a profile's local flora_variant_id values to the global palette.

    Profile fill_* functions write **local** variant ids (1..N) for simplicity;
    we then shift them into the global id space so the Rust runtime can
    dereference decorations without needing per-tile profile context.
    """
    if "flora_variant_id" not in buffer.layers:
        return
    offset = PROFILE_DECORATION_OFFSETS.get(profile_name, 0)
    if offset <= 1:
        return  # first profile gets offset 1 → local 1..N already = global 1..N
    arr = buffer.layers["flora_variant_id"]
    remapped = np.where(
        arr > 0,
        arr.astype(np.int32) + (offset - 1),
        0,
    )
    buffer.layers["flora_variant_id"] = remapped.astype(np.uint8)


def _build_zone_overlay_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer | None:
    profile = zone.worldgen.terrain_profile
    if profile == "spawn_plain":
        buffer = fill_spawn_plain_tile(zone, tile, tile_size, palette)
    elif profile == "broken_peaks":
        buffer = fill_broken_peaks_tile(zone, tile, tile_size, palette)
    elif profile == "spring_marsh":
        buffer = fill_spring_marsh_tile(zone, tile, tile_size, palette)
    elif profile == "rift_valley":
        buffer = fill_rift_valley_tile(zone, tile, tile_size, palette)
    elif profile == "cave_network":
        buffer = fill_cave_network_tile(zone, tile, tile_size, palette)
    elif profile == "jiu_zong_ruin":
        buffer = fill_jiu_zong_ruin_tile(zone, tile, tile_size, palette)
    elif profile == "waste_plateau":
        buffer = fill_waste_plateau_tile(zone, tile, tile_size, palette)
    elif profile == "pseudo_vein_oasis":
        buffer = fill_pseudo_vein_oasis_tile(zone, tile, tile_size, palette)
    elif profile == "rift_mouth_barrens":
        buffer = fill_rift_mouth_barrens_tile(zone, tile, tile_size, palette)
    elif profile == "ash_dead_zone":
        buffer = fill_ash_dead_zone_tile(zone, tile, tile_size, palette)
    elif profile == "sky_isle":
        buffer = fill_sky_isle_tile(zone, tile, tile_size, palette)
    elif profile == "abyssal_maze":
        buffer = fill_abyssal_maze_tile(zone, tile, tile_size, palette)
    elif profile == "ancient_battlefield":
        buffer = fill_ancient_battlefield_tile(zone, tile, tile_size, palette)
    elif profile == "tribulation_scorch":
        buffer = fill_tribulation_scorch_tile(zone, tile, tile_size, palette)
    elif profile == "tsy_zongmen_ruin":
        buffer = fill_tsy_zongmen_ruin_tile(zone, tile, tile_size, palette)
    elif profile == "tsy_daneng_crater":
        buffer = fill_tsy_daneng_crater_tile(zone, tile, tile_size, palette)
    elif profile == "tsy_zhanchang":
        buffer = fill_tsy_zhanchang_tile(zone, tile, tile_size, palette)
    elif profile == "tsy_gaoshou_hermitage":
        buffer = fill_tsy_gaoshou_hermitage_tile(zone, tile, tile_size, palette)
    elif profile == "dan_zong_yi_yuan":
        buffer = fill_dan_zong_yi_yuan_tile(zone, tile, tile_size, palette)
    elif profile == "wangyintai":
        buffer = fill_wangyintai_tile(zone, tile, tile_size, palette)
    else:
        return None
    if buffer is not None:
        _remap_flora_variant_to_global(buffer, profile)
    return buffer


def _resolve_layout_poi_center_xz(zone: BlueprintZone) -> tuple[int, int]:
    """Return the (x, z) center for compound flatten/mask of *zone*.

    Strategy (plan-terrain-wiring-v1 §11 M2):
    1. Look up the layout in COMPOUND_LAYOUT_REGISTRY to get poi_kind.
    2. Find the first POI in zone.pois matching that poi_kind.
    3. Fall back to zone center_xz if no POI matches (warn).
    """
    from .layouts import COMPOUND_LAYOUT_REGISTRY

    layout_name = zone.architectural_layout
    if layout_name and layout_name in COMPOUND_LAYOUT_REGISTRY:
        spec = COMPOUND_LAYOUT_REGISTRY[layout_name]
        for poi in zone.pois:
            if poi.kind == spec.poi_kind:
                return (int(round(poi.pos_xyz[0])), int(round(poi.pos_xyz[2])))

    # Fallback: zone center
    import logging as _logging
    _logging.getLogger(__name__).warning(
        "Zone '%s': could not resolve POI center for compound flatten "
        "(layout=%s), falling back to zone center_xz %s",
        zone.name, zone.architectural_layout, zone.center_xz,
    )
    return zone.center_xz


def _resolve_layout_target_height(zone: BlueprintZone) -> float:
    """Return the target height for compound flatten of *zone*.

    Strategy (plan-terrain-wiring-v1 §11 M2):
    1. Look up poi_kind via COMPOUND_LAYOUT_REGISTRY.
    2. Return POI.pos_xyz.y of the first matching POI.
    3. Fall back to zone height_model base mid-point.
    """
    from .layouts import COMPOUND_LAYOUT_REGISTRY

    layout_name = zone.architectural_layout
    if layout_name and layout_name in COMPOUND_LAYOUT_REGISTRY:
        spec = COMPOUND_LAYOUT_REGISTRY[layout_name]
        for poi in zone.pois:
            if poi.kind == spec.poi_kind:
                return float(poi.pos_xyz[1])

    # Fallback: mid-point of base height range
    base = zone.worldgen.height_model.get("base", [64, 80])
    if isinstance(base, list) and len(base) >= 2:
        return float((base[0] + base[1]) / 2)
    return 64.0


def synthesize_fields(plan: TerrainGenerationPlan) -> GeneratedFieldSet:
    palette = SurfacePalette()
    palette.extend(("stone", "coarse_dirt", "gravel"))

    all_layers = list(plan.wilderness.required_layers)
    for zone_plan in plan.zone_plans:
        for layer_name in zone_plan.required_layers:
            if layer_name not in all_layers:
                all_layers.append(layer_name)
    collapsed_zones = _collapsed_zone_names(plan.zone_overlays)
    if collapsed_zones and "realm_collapse_mask" not in all_layers:
        all_layers.append("realm_collapse_mask")

    generated_tiles: list[TileFieldBuffer] = []
    active_tiles = [
        tile
        for tile in plan.tiles
        if any(_zone_intersects_tile(zone, tile) for zone in plan.blueprint_zones)
    ]

    # plan-terrain-wiring-v1 P0 M2/#8: build a name→zone lookup for flatten/mask.
    _zone_by_name: dict[str, BlueprintZone] = {z.name: z for z in plan.blueprint_zones}

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
            if zone.name in collapsed_zones:
                _apply_realm_collapse_mask(base_tile, zone)

            # plan-terrain-wiring-v1 P0 M2/#8 — compound flatten + density mask.
            # Applied after blending, before compact, so layout platform is
            # authoritative over any zone height variation.
            if zone.compound_flatten_radius:
                # Resolve POI center: use the first POI of the layout's poi_kind.
                # We need the layout to find poi_kind; fall back to zone center_xz
                # + average height if no POI matches.
                poi_center_xz = _resolve_layout_poi_center_xz(zone)
                target_height = _resolve_layout_target_height(zone)
                apply_compound_flatten(
                    base_tile,
                    poi_center_xz=poi_center_xz,
                    radius=zone.compound_flatten_radius,
                    target_height=target_height,
                )
                compute_layout_density_mask(
                    base_tile,
                    poi_center_xz=poi_center_xz,
                    radius=zone.compound_flatten_radius,
                )

        base_tile.compact_layers()
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
            "Implemented: pseudo_vein_oasis overlay (false qi oasis + hungry ring ecology).",
            "Implemented: rift_mouth_barrens overlay (portal_anchor_sdf + negative pressure scar).",
            "Implemented: ash_dead_zone overlay (zero qi core + ash ecology).",
            "Implemented: sky_isle overlay (sky_island_* vertical layers).",
            "Implemented: abyssal_maze overlay (underground_tier/cavern_floor_y).",
            "Implemented: ancient_battlefield overlay (anomaly_intensity/anomaly_kind).",
            "Implemented: tribulation_scorch overlay (glass, lightning pits, surface minerals).",
            "Implemented: realm_collapse_mask from server zone_overlays collapsed records.",
            "Only active tiles intersecting named zones are synthesized in this scaffold stage.",
        ),
    )
