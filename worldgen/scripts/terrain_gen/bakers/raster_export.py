from __future__ import annotations

import json
import shutil
from pathlib import Path

import numpy as np

from ..blueprint import BlueprintZone, PoiSpec
from ..fields import LAYER_REGISTRY, BakePlan, GeneratedFieldSet, TerrainGenerationPlan
from ..profiles import list_profile_generators
from ..profiles import get_profile_generator
from ..profiles.base import DecorationSpec, EcologySpec

BIOME_PALETTE = (
    "minecraft:plains",
    "minecraft:stony_peaks",
    "minecraft:swamp",
    "minecraft:badlands",
    "minecraft:meadow",
    "minecraft:dripstone_caves",
    "minecraft:desert",
    "minecraft:forest",
    "minecraft:river",
    "minecraft:frozen_peaks",
    "minecraft:mangrove_swamp",
    "minecraft:flower_forest",
)

# Derived from the central LAYER_REGISTRY — no need to maintain separate lists.
FLOAT_LAYERS = {
    name for name, spec in LAYER_REGISTRY.items() if spec.export_type == "float32"
}
UINT8_LAYERS = {
    name for name, spec in LAYER_REGISTRY.items() if spec.export_type == "uint8"
}


def _layer_file_name(layer_name: str) -> str:
    return f"{layer_name}.bin"


def _write_float_layer(path: Path, values: np.ndarray) -> None:
    arr = np.ascontiguousarray(values, dtype=np.float32)
    path.write_bytes(arr.tobytes())


def _write_u8_layer(path: Path, values: np.ndarray) -> None:
    arr = np.ascontiguousarray(values, dtype=np.uint8)
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

    pois_payload = _collect_poi_payload(plan.blueprint_zones)
    ecology_payload = _collect_profile_ecology()

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
        "pois": pois_payload,
        "semantic_layers": [
            name
            for name in ("qi_density", "mofa_decay", "qi_vein_flow")
            if name in LAYER_REGISTRY
        ],
        "vertical_layers": [
            name
            for name in (
                "sky_island_mask",
                "sky_island_base_y",
                "sky_island_thickness",
                "underground_tier",
                "cavern_floor_y",
                "ceiling_height",
            )
            if name in LAYER_REGISTRY
        ],
        "abyssal_tier_floor_y": {"1": 28.0, "2": -4.0, "3": -36.0},
        "anomaly_kinds": {
            "0": "none",
            "1": "spacetime_rift",
            "2": "qi_turbulence",
            "3": "blood_moon_anchor",
            "4": "cursed_echo",
            "5": "wild_formation",
        },
        "profiles_ecology": ecology_payload,
        "notes": [
            "Python exports 2D terrain fields only; block and biome realization happens in Rust.",
            "All tile layer payloads are little-endian raw binaries for mmap-friendly loading.",
            "Semantic layers (qi_density / mofa_decay / qi_vein_flow) carry the xianxia world model.",
            "Vertical layers encode 3D world from 2D rasters: sky_island_* for floating isles above",
            "  (Rust should gate on mask>=0.2), underground_tier+cavern_floor_y for stacked caves below",
            "  (tier 1/2/3 floors per abyssal_tier_floor_y). Sentinel 9999 = 'no isle/cavern here'.",
            "Ecology: flora_density (0..1) + flora_variant_id (uint8 index into ",
            "  profiles_ecology[zone_profile].decorations; 0 = no flora / wilderness fallback).",
            "Anomaly: anomaly_intensity (0..1) + anomaly_kind (uint8 from anomaly_kinds map).",
            "  Event systems trigger themed spawns / FX when intensity > 0.3.",
            "POIs are zone-scoped narrative anchors for agent / NPC / HUD consumers.",
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


def _poi_dict(zone_name: str, poi: PoiSpec) -> dict[str, object]:
    return {
        "zone": zone_name,
        "kind": poi.kind,
        "name": poi.name,
        "pos_xyz": [poi.pos_xyz[0], poi.pos_xyz[1], poi.pos_xyz[2]],
        "tags": list(poi.tags),
        "unlock": poi.unlock,
        "qi_affinity": poi.qi_affinity,
        "danger_bias": poi.danger_bias,
    }


def _collect_poi_payload(zones: list[BlueprintZone]) -> list[dict[str, object]]:
    payload: list[dict[str, object]] = []
    for zone in zones:
        for poi in zone.pois:
            payload.append(_poi_dict(zone.name, poi))
    return payload


def _decoration_dict(idx: int, deco: DecorationSpec) -> dict[str, object]:
    return {
        "variant_id": idx,
        "name": deco.name,
        "kind": deco.kind,
        "blocks": list(deco.blocks),
        "size_range": list(deco.size_range),
        "rarity": deco.rarity,
        "notes": deco.notes,
    }


def _ecology_dict(spec: EcologySpec) -> dict[str, object]:
    # variant_id 0 is reserved for "no flora"; actual palette starts at 1.
    return {
        "decorations": [
            _decoration_dict(i + 1, d) for i, d in enumerate(spec.decorations)
        ],
        "ambient_effects": list(spec.ambient_effects),
        "notes": spec.notes,
    }


def _collect_profile_ecology() -> dict[str, object]:
    payload: dict[str, object] = {}
    for profile_name in list_profile_generators():
        gen = get_profile_generator(profile_name)
        payload[profile_name] = _ecology_dict(gen.ecology)
    return payload


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
