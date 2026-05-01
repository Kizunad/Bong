from __future__ import annotations

import binascii
import json
import math
import struct
import zlib
from dataclasses import fields as dataclass_fields
from dataclasses import is_dataclass
from pathlib import Path
from typing import Any

from .blueprint import BlueprintZone
from .fields import Bounds2D, GeneratedFieldSet, TerrainGenerationPlan, TileFieldBuffer
from .noise import coherent_noise_2d
from .profiles.wilderness import sample_wilderness_point


def _json_ready(value: Any) -> Any:
    if isinstance(value, Path):
        return str(value)
    if is_dataclass(value):
        return {
            field.name: _json_ready(getattr(value, field.name))
            for field in dataclass_fields(value)
        }
    if isinstance(value, dict):
        return {str(key): _json_ready(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_json_ready(item) for item in value]
    return value


def write_plan_json(plan: TerrainGenerationPlan, output_dir: Path) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    plan_path = output_dir / "terrain-plan.json"
    with plan_path.open("w", encoding="utf-8") as handle:
        json.dump(_json_ready(plan), handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    return plan_path


def write_field_summary_json(fields: GeneratedFieldSet, output_dir: Path) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    summary_path = output_dir / "terrain-fields-summary.json"
    payload = {
        "tile_size": fields.tile_size,
        "surface_palette": fields.surface_palette.names,
        "layers": list(fields.layers),
        "notes": list(fields.notes),
        "tile_summaries": _json_ready(fields.summaries()),
    }
    with summary_path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    return summary_path


def _surface_color(surface_name: str) -> tuple[int, int, int]:
    palette = {
        "stone": (111, 114, 118),
        "smooth_stone": (171, 174, 174),
        "dirt": (111, 90, 70),
        "coarse_dirt": (120, 106, 90),
        "gravel": (132, 128, 122),
        "sand": (182, 170, 128),
        "red_sandstone": (173, 103, 79),
        "terracotta": (154, 110, 92),
        "blackstone": (54, 52, 58),
        "basalt": (78, 80, 84),
        "magma_block": (182, 86, 28),
        "crimson_nylium": (118, 36, 54),
        "calcite": (229, 231, 228),
        "snow_block": (246, 248, 252),
        "packed_ice": (164, 198, 235),
        "podzol": (103, 77, 57),
        "rooted_dirt": (97, 88, 68),
        "soul_sand": (88, 70, 58),
        "bone_block": (224, 219, 201),
        "mud": (91, 81, 72),
        "clay": (126, 138, 146),
        "grass_block": (93, 136, 82),
        "moss_block": (78, 121, 74),
        "andesite": (126, 130, 133),
        "deepslate": (84, 88, 96),
        "dead_bush": (140, 121, 79),
    }
    return palette.get(surface_name, (255, 0, 255))


def _png_chunk(chunk_type: bytes, payload: bytes) -> bytes:
    crc = binascii.crc32(chunk_type)
    crc = binascii.crc32(payload, crc) & 0xFFFFFFFF
    return (
        struct.pack(">I", len(payload)) + chunk_type + payload + struct.pack(">I", crc)
    )


def _write_png(
    path: Path, width: int, height: int, pixels: list[tuple[int, int, int]]
) -> Path:
    rows = []
    for row_start in range(0, len(pixels), width):
        row = bytearray([0])
        for red, green, blue in pixels[row_start : row_start + width]:
            row.extend((red, green, blue))
        rows.append(bytes(row))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    idat = zlib.compress(b"".join(rows), level=9)

    with path.open("wb") as handle:
        handle.write(b"\x89PNG\r\n\x1a\n")
        handle.write(_png_chunk(b"IHDR", ihdr))
        handle.write(_png_chunk(b"IDAT", idat))
        handle.write(_png_chunk(b"IEND", b""))
    return path


def _sample_axis(min_value: int, max_value: int, stride: int) -> list[int]:
    step = max(stride, 1)
    values = list(range(min_value, max_value + 1, step))
    if not values or values[-1] != max_value:
        values.append(max_value)
    return values


def _contains_point(bounds: Bounds2D, world_x: int, world_z: int) -> bool:
    return (
        bounds.min_x <= world_x <= bounds.max_x
        and bounds.min_z <= world_z <= bounds.max_z
    )


def _point_in_zone_core(zone: BlueprintZone, world_x: int, world_z: int) -> bool:
    shape = zone.worldgen.shape
    center_x, center_z = zone.center_xz
    half_width = max(zone.size_xz[0] * 0.5, 1.0)
    half_depth = max(zone.size_xz[1] * 0.5, 1.0)
    edge_noise = coherent_noise_2d(world_x, world_z, scale=420.0, seed=17)
    edge_warp = 1.0 + edge_noise * 0.12

    if shape in {"ellipse", "massif", "basin", "plateau", "subterranean_cluster", "irregular_blob"}:
        dx = (world_x - center_x) / (half_width * edge_warp)
        dz = (world_z - center_z) / (half_depth * (1.0 - edge_noise * 0.08))
        return dx * dx + dz * dz <= 1.0

    if shape == "rotated_rift":
        angle = math.radians(-20.0)
        cos_angle = math.cos(angle)
        sin_angle = math.sin(angle)
        dx = world_x - center_x
        dz = world_z - center_z
        along = dx * cos_angle - dz * sin_angle
        cross = dx * sin_angle + dz * cos_angle
        return abs(along) <= half_depth * (1.0 - edge_noise * 0.06) and abs(
            cross
        ) <= half_width * (1.0 + edge_noise * 0.16)

    return _contains_point(zone.bounds_xz, world_x, world_z)


def _point_in_zone_transition(zone: BlueprintZone, world_x: int, world_z: int) -> bool:
    if _point_in_zone_core(zone, world_x, world_z):
        return False
    return _contains_point(
        zone.bounds_xz.expanded(zone.worldgen.boundary.width), world_x, world_z
    )


def _zone_preview_color(profile_name: str) -> tuple[int, int, int]:
    colors = {
        "spawn_plain": (122, 171, 104),
        "broken_peaks": (160, 168, 182),
        "spring_marsh": (82, 146, 122),
        "rift_valley": (191, 96, 74),
        "cave_network": (114, 102, 158),
        "waste_plateau": (171, 144, 96),
        "ash_dead_zone": (128, 126, 118),
    }
    return colors.get(profile_name, (214, 88, 185))


def _zone_preview_weight(zone: BlueprintZone, world_x: int, world_z: int) -> float:
    shape = zone.worldgen.shape
    center_x, center_z = zone.center_xz
    half_width = max(zone.size_xz[0] * 0.5, 1.0)
    half_depth = max(zone.size_xz[1] * 0.5, 1.0)
    boundary = max(float(zone.worldgen.boundary.width), 1.0)
    edge_noise = coherent_noise_2d(world_x, world_z, scale=420.0, seed=17)
    edge_warp = 1.0 + edge_noise * 0.12

    if shape in {"ellipse", "massif", "basin", "plateau", "subterranean_cluster", "irregular_blob"}:
        dx = (world_x - center_x) / (half_width * edge_warp)
        dz = (world_z - center_z) / (half_depth * (1.0 - edge_noise * 0.08))
        core_ratio = math.sqrt(dx * dx + dz * dz)

        ex = (world_x - center_x) / ((half_width + boundary) * edge_warp)
        ez = (world_z - center_z) / (
            (half_depth + boundary) * (1.0 - edge_noise * 0.08)
        )
        expanded_ratio = math.sqrt(ex * ex + ez * ez)
    elif shape == "rotated_rift":
        angle = math.radians(-20.0)
        cos_angle = math.cos(angle)
        sin_angle = math.sin(angle)
        dx = world_x - center_x
        dz = world_z - center_z
        along = dx * cos_angle - dz * sin_angle
        cross = dx * sin_angle + dz * cos_angle
        along_warp = 1.0 - edge_noise * 0.06
        cross_warp = 1.0 + edge_noise * 0.16
        core_ratio = max(
            abs(along) / (half_depth * along_warp),
            abs(cross) / (half_width * cross_warp),
        )
        expanded_ratio = max(
            abs(along) / ((half_depth + boundary) * along_warp),
            abs(cross) / ((half_width + boundary) * cross_warp),
        )
    else:
        if _contains_point(zone.bounds_xz, world_x, world_z):
            return 0.75
        if _contains_point(
            zone.bounds_xz.expanded(zone.worldgen.boundary.width), world_x, world_z
        ):
            return 0.22
        return 0.0

    if core_ratio <= 1.0:
        center_bias = max(0.0, 1.0 - core_ratio)
        return 0.52 + center_bias * 0.32
    if expanded_ratio <= 1.0:
        transition = max(0.0, 1.0 - expanded_ratio)
        smooth = transition * transition * (3.0 - 2.0 * transition)
        return 0.12 + smooth * 0.2
    return 0.0


def _dominant_zone_preview(
    plan: TerrainGenerationPlan, world_x: int, world_z: int
) -> tuple[BlueprintZone | None, float]:
    best_zone: BlueprintZone | None = None
    best_weight = 0.0
    for zone in plan.blueprint_zones:
        weight = _zone_preview_weight(zone, world_x, world_z)
        if weight > best_weight:
            best_zone = zone
            best_weight = weight
    return best_zone, best_weight


def _wilderness_preview_color(
    surface_name: str, height_value: float
) -> tuple[int, int, int]:
    base = {
        "stone": (92, 94, 98),
        "gravel": (102, 100, 96),
        "coarse_dirt": (98, 90, 84),
        "dirt": (102, 91, 80),
        "mud": (92, 84, 80),
        "sand": (116, 109, 92),
        "clay": (99, 107, 112),
        "grass_block": (88, 98, 84),
        "moss_block": (82, 92, 80),
        "andesite": (97, 99, 102),
        "smooth_stone": (126, 128, 128),
        "deepslate": (78, 80, 86),
        "dead_bush": (108, 100, 82),
    }.get(surface_name, (94, 92, 90))
    relief = max(0.0, min(1.0, (height_value - 62.0) / 28.0))
    return _blend_color((72, 74, 78), base, 0.28 + relief * 0.18)


def _surface_preview_color(
    surface_name: str,
    height_value: float,
    water_level: float,
    dominant_zone: BlueprintZone | None,
    zone_weight: float,
) -> tuple[int, int, int]:
    if water_level >= 0.0 and height_value < water_level + 0.75:
        water_color = (74, 112, 168)
        if (
            dominant_zone is not None
            and dominant_zone.worldgen.terrain_profile == "spring_marsh"
        ):
            return _blend_color(water_color, (88, 146, 138), 0.28)
        return water_color

    if dominant_zone is None or zone_weight < 0.08:
        return _wilderness_preview_color(surface_name, height_value)

    local_surface = _blend_color(
        _wilderness_preview_color(surface_name, height_value),
        _surface_color(surface_name),
        0.34,
    )
    zone_tint = _zone_preview_color(dominant_zone.worldgen.terrain_profile)
    return _blend_color(local_surface, zone_tint, min(0.74, 0.34 + zone_weight * 0.42))


def _blend_color(
    base: tuple[int, int, int], overlay: tuple[int, int, int], alpha: float
) -> tuple[int, int, int]:
    return (
        int(round(base[0] + (overlay[0] - base[0]) * alpha)),
        int(round(base[1] + (overlay[1] - base[1]) * alpha)),
        int(round(base[2] + (overlay[2] - base[2]) * alpha)),
    )


def _tile_lookup(fields: GeneratedFieldSet) -> dict[tuple[int, int], TileFieldBuffer]:
    return {(tile.tile.tile_x, tile.tile.tile_z): tile for tile in fields.tiles}


def _sample_preview_point(
    fields: GeneratedFieldSet,
    tile_map: dict[tuple[int, int], TileFieldBuffer],
    world_x: int,
    world_z: int,
) -> tuple[float, str, float]:
    tile_x = world_x // fields.tile_size
    tile_z = world_z // fields.tile_size
    tile = tile_map.get((tile_x, tile_z))

    if tile is None:
        sample = sample_wilderness_point(world_x, world_z)
        return (
            float(sample["height"]),
            str(sample["surface_name"]),
            float(sample["water_level"]),
        )

    local_x = world_x - tile.tile.min_x
    local_z = world_z - tile.tile.min_z
    index = tile.index(local_x, local_z)
    surface_id = int(tile.get_index_value("surface_id", index))
    surface_name = fields.surface_palette.names[surface_id]
    return (
        float(tile.get_index_value("height", index)),
        str(surface_name),
        float(tile.get_index_value("water_level", index)),
    )


def _focus_bounds(plan: TerrainGenerationPlan, margin: int = 1400) -> Bounds2D:
    min_x = min(zone.bounds_xz.min_x for zone in plan.blueprint_zones) - margin
    max_x = max(zone.bounds_xz.max_x for zone in plan.blueprint_zones) + margin
    min_z = min(zone.bounds_xz.min_z for zone in plan.blueprint_zones) - margin
    max_z = max(zone.bounds_xz.max_z for zone in plan.blueprint_zones) + margin
    return Bounds2D(
        min_x=max(plan.world_bounds.min_x, min_x),
        max_x=min(plan.world_bounds.max_x, max_x),
        min_z=max(plan.world_bounds.min_z, min_z),
        max_z=min(plan.world_bounds.max_z, max_z),
    )


def _zone_preview_bounds(
    plan: TerrainGenerationPlan, zone: BlueprintZone, margin: int = 640
) -> Bounds2D:
    return Bounds2D(
        min_x=max(plan.world_bounds.min_x, zone.bounds_xz.min_x - margin),
        max_x=min(plan.world_bounds.max_x, zone.bounds_xz.max_x + margin),
        min_z=max(plan.world_bounds.min_z, zone.bounds_xz.min_z - margin),
        max_z=min(plan.world_bounds.max_z, zone.bounds_xz.max_z + margin),
    )


def _apply_light(
    color: tuple[int, int, int], brightness: float, ambient: float = 0.72
) -> tuple[int, int, int]:
    factor = max(0.0, min(1.6, ambient + brightness * 0.55))
    return (
        max(0, min(255, int(round(color[0] * factor)))),
        max(0, min(255, int(round(color[1] * factor)))),
        max(0, min(255, int(round(color[2] * factor)))),
    )


def _hillshade(heights: list[list[float]]) -> list[list[float]]:
    if not heights or not heights[0]:
        return []

    height_count = len(heights)
    width = len(heights[0])
    result = [[0.0 for _ in range(width)] for _ in range(height_count)]
    light_x = -0.65
    light_y = -0.45
    light_z = 0.62

    for z in range(height_count):
        z0 = max(0, z - 1)
        z1 = min(height_count - 1, z + 1)
        for x in range(width):
            x0 = max(0, x - 1)
            x1 = min(width - 1, x + 1)
            dzdx = (heights[z][x1] - heights[z][x0]) * 0.5
            dzdy = (heights[z1][x] - heights[z0][x]) * 0.5
            nx = -dzdx * 0.18
            ny = -dzdy * 0.18
            nz = 1.0
            length = math.sqrt(nx * nx + ny * ny + nz * nz)
            nx /= length
            ny /= length
            nz /= length
            result[z][x] = max(0.0, nx * light_x + ny * light_y + nz * light_z)

    return result


def _contour_strength(
    heights: list[list[float]], z: int, x: int, interval: float = 6.0
) -> float:
    height_value = heights[z][x]
    nearest = round(height_value / interval) * interval
    distance = abs(height_value - nearest)
    if distance > 0.38:
        return 0.0

    height_count = len(heights)
    width = len(heights[0])
    z0 = max(0, z - 1)
    z1 = min(height_count - 1, z + 1)
    x0 = max(0, x - 1)
    x1 = min(width - 1, x + 1)
    local_values = (
        heights[z1][x],
        heights[z0][x],
        heights[z][x1],
        heights[z][x0],
    )
    local_span = max(local_values) - min(local_values)
    if local_span < 0.45:
        return 0.0

    return max(0.0, 1.0 - distance / 0.38)


def _render_preview_set(
    plan: TerrainGenerationPlan,
    fields: GeneratedFieldSet,
    output_dir: Path,
    sample_bounds: Bounds2D,
    stride: int,
    name_prefix: str,
) -> dict[str, Path]:
    sample_xs = _sample_axis(sample_bounds.min_x, sample_bounds.max_x, stride)
    sample_zs = _sample_axis(sample_bounds.min_z, sample_bounds.max_z, stride)
    if not sample_xs or not sample_zs:
        return {}

    width = len(sample_xs)
    height = len(sample_zs)
    tile_map = _tile_lookup(fields)

    height_grid: list[list[float]] = []
    surface_grid: list[list[tuple[int, int, int]]] = []
    layout_grid: list[list[tuple[int, int, int]]] = []

    for world_z in sample_zs:
        row_heights: list[float] = []
        row_surface: list[tuple[int, int, int]] = []
        row_layout: list[tuple[int, int, int]] = []
        for world_x in sample_xs:
            height_value, surface_name, water_level = _sample_preview_point(
                fields, tile_map, world_x, world_z
            )
            row_heights.append(height_value)
            dominant_zone, zone_weight = _dominant_zone_preview(plan, world_x, world_z)

            surface_color = _surface_preview_color(
                surface_name,
                height_value,
                water_level,
                dominant_zone,
                zone_weight,
            )
            row_surface.append(surface_color)

            shade = int(max(0.0, min(255.0, (height_value - 35.0) / 175.0 * 255.0)))
            base_layout = (26 + shade // 6, 26 + shade // 6, 30 + shade // 7)
            layout_color = base_layout
            if dominant_zone is not None:
                zone_color = _zone_preview_color(dominant_zone.worldgen.terrain_profile)
                layout_color = _blend_color(
                    base_layout,
                    zone_color,
                    min(0.9, 0.18 + zone_weight * 0.82),
                )

            for zone in plan.blueprint_zones:
                center_x, center_z = zone.center_xz
                if (
                    abs(world_x - center_x) <= stride
                    and abs(world_z - center_z) <= stride
                ):
                    layout_color = (244, 244, 244)
                    break

            row_layout.append(layout_color)

        height_grid.append(row_heights)
        surface_grid.append(row_surface)
        layout_grid.append(row_layout)

    shade_grid = _hillshade(height_grid)
    height_pixels: list[tuple[int, int, int]] = []
    surface_pixels: list[tuple[int, int, int]] = []
    layout_pixels: list[tuple[int, int, int]] = []
    min_height = min(min(row) for row in height_grid)
    max_height = max(max(row) for row in height_grid)
    height_span = max(max_height - min_height, 1.0)

    for z in range(height):
        for x in range(width):
            height_value = height_grid[z][x]
            base_gray = int(
                max(
                    0.0,
                    min(
                        255.0, 36.0 + (height_value - min_height) / height_span * 196.0
                    ),
                )
            )
            lit_gray = _apply_light(
                (base_gray, base_gray, base_gray), shade_grid[z][x], 0.52
            )
            contour = _contour_strength(height_grid, z, x)
            if contour > 0.0:
                lit_gray = _blend_color(lit_gray, (24, 26, 30), contour * 0.36)
            height_pixels.append(lit_gray)
            lit_surface = _apply_light(surface_grid[z][x], shade_grid[z][x], 0.66)
            if contour > 0.0:
                lit_surface = _blend_color(lit_surface, (28, 30, 34), contour * 0.22)
            surface_pixels.append(lit_surface)
            layout_pixels.append(
                _apply_light(layout_grid[z][x], shade_grid[z][x], 0.58)
            )

    height_path = _write_png(
        output_dir / f"{name_prefix}height-preview.png", width, height, height_pixels
    )
    surface_path = _write_png(
        output_dir / f"{name_prefix}surface-preview.png", width, height, surface_pixels
    )
    layout_path = _write_png(
        output_dir / f"{name_prefix}layout-preview.png", width, height, layout_pixels
    )
    return {
        f"{name_prefix}height_preview": height_path,
        f"{name_prefix}surface_preview": surface_path,
        f"{name_prefix}layout_preview": layout_path,
    }


def write_preview_images(
    plan: TerrainGenerationPlan,
    fields: GeneratedFieldSet,
    output_dir: Path,
    stride: int = 64,
) -> dict[str, Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    previews = _render_preview_set(
        plan,
        fields,
        output_dir,
        plan.world_bounds,
        stride,
        name_prefix="",
    )
    focus_stride = max(24, stride // 2)
    previews.update(
        _render_preview_set(
            plan,
            fields,
            output_dir,
            _focus_bounds(plan),
            focus_stride,
            name_prefix="focus-",
        )
    )
    zone_stride = max(12, stride // 3)
    for zone in plan.blueprint_zones:
        previews.update(
            _render_preview_set(
                plan,
                fields,
                output_dir,
                _zone_preview_bounds(plan, zone),
                zone_stride,
                name_prefix=f"zone-{zone.name}-",
            )
        )
    return previews


def format_summary(plan: TerrainGenerationPlan, plan_path: Path) -> str:
    lines = [
        "terrain_gen scaffold ready",
        f"  world: {plan.world_name}",
        f"  blueprint: {plan.blueprint_path}",
        f"  profiles: {plan.profiles_path}",
        f"  zones: {len(plan.zone_plans)}",
        f"  tiles: {plan.tile_count} @ {plan.tile_size} blocks",
        f"  stitch: {plan.stitch_strategy}",
        f"  plan: {plan_path}",
    ]
    if plan.bake_plan is not None:
        lines.append(f"  bake backend: {plan.bake_plan.backend}")
        lines.append(f"  bake output: {plan.bake_plan.output_dir}")
    return "\n".join(lines)


def format_field_summary(fields: GeneratedFieldSet, summary_path: Path) -> str:
    lines = [
        "terrain_gen field synthesis ready",
        f"  tiles synthesized: {len(fields.tiles)}",
        f"  layers: {', '.join(fields.layers)}",
        f"  palette entries: {len(fields.surface_palette.names)}",
        f"  summary: {summary_path}",
    ]
    return "\n".join(lines)
