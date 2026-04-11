from __future__ import annotations

import binascii
import json
import math
import struct
import zlib
from pathlib import Path

from ..fields import BakePlan, GeneratedFieldSet, TerrainGenerationPlan, TileFieldBuffer
from ..profiles.wilderness import sample_wilderness_point


def _png_chunk(chunk_type: bytes, payload: bytes) -> bytes:
    crc = binascii.crc32(chunk_type)
    crc = binascii.crc32(payload, crc) & 0xFFFFFFFF
    return (
        struct.pack(">I", len(payload)) + chunk_type + payload + struct.pack(">I", crc)
    )


def _write_png_rows(path: Path, width: int, height: int, rows: list[bytes]) -> Path:
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    idat = zlib.compress(b"".join(rows), level=9)
    with path.open("wb") as handle:
        handle.write(b"\x89PNG\r\n\x1a\n")
        handle.write(_png_chunk(b"IHDR", ihdr))
        handle.write(_png_chunk(b"IDAT", idat))
        handle.write(_png_chunk(b"IEND", b""))
    return path


def _surface_color(surface_name: str) -> tuple[int, int, int]:
    palette = {
        "stone": (111, 114, 118),
        "dirt": (111, 90, 70),
        "coarse_dirt": (120, 106, 90),
        "gravel": (132, 128, 122),
        "sand": (182, 170, 128),
        "red_sandstone": (173, 103, 79),
        "terracotta": (154, 110, 92),
        "mud": (91, 81, 72),
        "clay": (126, 138, 146),
        "grass_block": (93, 136, 82),
        "moss_block": (78, 121, 74),
        "andesite": (126, 130, 133),
        "deepslate": (84, 88, 96),
        "dead_bush": (140, 121, 79),
    }
    return palette.get(surface_name, (255, 0, 255))


def _sample_axis(min_value: int, max_value: int, stride: int) -> list[int]:
    values = list(range(min_value, max_value + 1, stride))
    if not values or values[-1] != max_value:
        values.append(max_value)
    return values


def _tile_lookup(fields: GeneratedFieldSet) -> dict[tuple[int, int], TileFieldBuffer]:
    return {(tile.tile.tile_x, tile.tile.tile_z): tile for tile in fields.tiles}


def _sample_point(
    fields: GeneratedFieldSet,
    tile_map: dict[tuple[int, int], TileFieldBuffer],
    world_x: int,
    world_z: int,
) -> tuple[float, str, float, float]:
    tile_x = world_x // fields.tile_size
    tile_z = world_z // fields.tile_size
    tile = tile_map.get((tile_x, tile_z))
    if tile is None:
        sample = sample_wilderness_point(world_x, world_z)
        return (
            float(sample["height"]),
            str(sample["surface_name"]),
            float(sample["water_level"]),
            float(sample["feature_mask"]),
        )

    local_x = world_x - tile.tile.min_x
    local_z = world_z - tile.tile.min_z
    index = tile.index(local_x, local_z)
    surface_id = int(tile.get_index_value("surface_id", index))
    return (
        float(tile.get_index_value("height", index)),
        str(fields.surface_palette.names[surface_id]),
        float(tile.get_index_value("water_level", index)),
        float(tile.get_index_value("feature_mask", index)),
    )


def _choose_stride(plan: TerrainGenerationPlan, target_max_dim: int = 1024) -> int:
    max_dim = max(plan.world_bounds.width, plan.world_bounds.depth)
    return max(1, math.ceil(max_dim / target_max_dim))


def export_worldpainter_rasters(
    plan: TerrainGenerationPlan,
    fields: GeneratedFieldSet,
) -> dict[str, Path]:
    if plan.bake_plan is None:
        raise ValueError("worldpainter bake plan is required before export")

    output_dir = plan.bake_plan.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    stride = _choose_stride(plan)
    sample_xs = _sample_axis(plan.world_bounds.min_x, plan.world_bounds.max_x, stride)
    sample_zs = _sample_axis(plan.world_bounds.min_z, plan.world_bounds.max_z, stride)
    width = len(sample_xs)
    height = len(sample_zs)
    tile_map = _tile_lookup(fields)

    min_height = float("inf")
    max_height = float("-inf")
    for world_z in sample_zs:
        for world_x in sample_xs:
            height_value, _, _, _ = _sample_point(fields, tile_map, world_x, world_z)
            min_height = min(min_height, height_value)
            max_height = max(max_height, height_value)
    height_span = max(max_height - min_height, 1.0)

    height_rows: list[bytes] = []
    material_rows: list[bytes] = []
    water_rows: list[bytes] = []
    feature_rows: list[bytes] = []

    for world_z in sample_zs:
        height_row = bytearray([0])
        material_row = bytearray([0])
        water_row = bytearray([0])
        feature_row = bytearray([0])
        for world_x in sample_xs:
            height_value, surface_name, water_level, feature_mask = _sample_point(
                fields, tile_map, world_x, world_z
            )
            height_gray = int(
                max(0.0, min(255.0, (height_value - min_height) / height_span * 255.0))
            )
            height_row.extend((height_gray, height_gray, height_gray))

            material_row.extend(_surface_color(surface_name))

            if water_level >= 0.0 and height_value < water_level + 0.75:
                water_intensity = int(
                    max(96.0, min(255.0, 160.0 + (water_level - height_value) * 28.0))
                )
                water_row.extend((0, water_intensity // 2, water_intensity))
            else:
                water_row.extend((0, 0, 0))

            feature_intensity = int(max(0.0, min(255.0, feature_mask * 255.0)))
            feature_row.extend(
                (feature_intensity, feature_intensity, feature_intensity)
            )

        height_rows.append(bytes(height_row))
        material_rows.append(bytes(material_row))
        water_rows.append(bytes(water_row))
        feature_rows.append(bytes(feature_row))

    artifact_paths = plan.bake_plan.artifacts
    _write_png_rows(artifact_paths["heightmap"], width, height, height_rows)
    _write_png_rows(artifact_paths["material_map"], width, height, material_rows)
    _write_png_rows(artifact_paths["water_map"], width, height, water_rows)
    _write_png_rows(artifact_paths["feature_map"], width, height, feature_rows)

    manifest_path = artifact_paths["manifest"]
    manifest = {
        "backend": "worldpainter",
        "world_name": plan.world_name,
        "world_bounds": {
            "min_x": plan.world_bounds.min_x,
            "max_x": plan.world_bounds.max_x,
            "min_z": plan.world_bounds.min_z,
            "max_z": plan.world_bounds.max_z,
        },
        "sample_stride_blocks": stride,
        "image_width": width,
        "image_height": height,
        "height_range": {
            "min": round(min_height, 3),
            "max": round(max_height, 3),
        },
        "artifacts": {key: str(path) for key, path in artifact_paths.items()},
        "notes": [
            "heightmap uses 8-bit grayscale normalized to the sampled height range",
            "material_map is a terrain surface color reference raster",
            "water_map highlights sampled water presence",
            "feature_map stores feature_mask intensity",
        ],
    }
    with manifest_path.open("w", encoding="utf-8") as handle:
        json.dump(manifest, handle, ensure_ascii=False, indent=2)
        handle.write("\n")

    return artifact_paths


def build_worldpainter_bake_plan(
    plan: TerrainGenerationPlan, output_root: Path
) -> BakePlan:
    output_dir = output_root / "worldpainter"
    world_stem = plan.world_name
    return BakePlan(
        backend="worldpainter",
        output_dir=output_dir,
        artifacts={
            "heightmap": output_dir / f"{world_stem}.heightmap.png",
            "material_map": output_dir / f"{world_stem}.surface.png",
            "water_map": output_dir / f"{world_stem}.water.png",
            "feature_map": output_dir / f"{world_stem}.features.png",
            "manifest": output_dir / f"{world_stem}.manifest.json",
        },
        notes=(
            "Use this backend during rapid iteration to validate macro terrain silhouettes.",
            "Exports normalized height/material/water/feature rasters for downstream bake tooling.",
        ),
    )
