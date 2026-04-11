from __future__ import annotations

import json
import shutil
from pathlib import Path

import numpy as np

from ..fields import BakePlan, GeneratedFieldSet, TerrainGenerationPlan

BIOME_PALETTE = (
    "minecraft:plains",
    "minecraft:stony_peaks",
    "minecraft:swamp",
    "minecraft:badlands",
    "minecraft:meadow",
    "minecraft:dripstone_caves",
    "minecraft:desert",
)

FLOAT_LAYERS = {
    "height",
    "water_level",
    "feature_mask",
    "boundary_weight",
    "rift_axis_sdf",
    "rim_edge_mask",
    "fracture_mask",
    "cave_mask",
    "ceiling_height",
    "entrance_mask",
    "neg_pressure",
    "ruin_density",
}

UINT8_LAYERS = {
    "surface_id",
    "subsurface_id",
    "biome_id",
}


def _layer_file_name(layer_name: str) -> str:
    return f"{layer_name}.bin"


def _write_float_layer(path: Path, values: list[float | int]) -> None:
    arr = np.array(values, dtype=np.float32)
    path.write_bytes(arr.tobytes())


def _write_u8_layer(path: Path, values: list[float | int]) -> None:
    arr = np.array(values, dtype=np.uint8)
    path.write_bytes(arr.tobytes())


def export_rasters(
    plan: TerrainGenerationPlan, fields: GeneratedFieldSet
) -> dict[str, Path]:
    if plan.bake_plan is None:
        raise ValueError("raster bake plan is required before export")

    output_dir = plan.bake_plan.output_dir
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    manifest_tiles: list[dict[str, object]] = []
    for tile in fields.tiles:
        tile_dir = output_dir / tile.tile.tile_id
        tile_dir.mkdir(parents=True, exist_ok=True)

        written_layers: list[str] = []
        for layer_name in fields.layers:
            if layer_name not in tile.layers:
                continue
            layer_path = tile_dir / _layer_file_name(layer_name)
            values = tile.layers[layer_name]
            if layer_name in FLOAT_LAYERS:
                _write_float_layer(layer_path, values)
            elif layer_name in UINT8_LAYERS:
                _write_u8_layer(layer_path, values)
            else:
                raise ValueError(f"unsupported raster layer '{layer_name}'")
            written_layers.append(layer_name)

        manifest_tiles.append(
            {
                "tile_x": tile.tile.tile_x,
                "tile_z": tile.tile.tile_z,
                "dir": tile.tile.tile_id,
                "zones": list(tile.contributing_zones),
                "layers": written_layers,
            }
        )

    manifest = {
        "version": 1,
        "backend": "raster",
        "world_name": plan.world_name,
        "tile_size": fields.tile_size,
        "world_bounds": {
            "min_x": plan.world_bounds.min_x,
            "max_x": plan.world_bounds.max_x,
            "min_z": plan.world_bounds.min_z,
            "max_z": plan.world_bounds.max_z,
        },
        "surface_palette": list(fields.surface_palette.names),
        "biome_palette": list(BIOME_PALETTE),
        "tiles": manifest_tiles,
        "notes": [
            "Python exports 2D terrain fields only; block and biome realization happens in Rust.",
            "All tile layer payloads are little-endian raw binaries for mmap-friendly loading.",
        ],
    }

    manifest_path = plan.bake_plan.artifacts["manifest"]
    with manifest_path.open("w", encoding="utf-8") as handle:
        json.dump(manifest, handle, ensure_ascii=False, indent=2)
        handle.write("\n")

    return {
        "manifest": manifest_path,
        "raster_dir": output_dir,
    }


def build_raster_bake_plan(plan: TerrainGenerationPlan, output_root: Path) -> BakePlan:
    output_dir = output_root / "rasters"
    return BakePlan(
        backend="raster",
        output_dir=output_dir,
        artifacts={
            "manifest": output_dir / "manifest.json",
        },
        notes=(
            "Exports terrain fields as raw binary rasters for runtime chunk synthesis.",
        ),
    )
