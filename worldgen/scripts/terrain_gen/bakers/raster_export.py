from __future__ import annotations

import json
import shutil
from pathlib import Path
from typing import Optional

import numpy as np

from ..blueprint import BlueprintZone, PoiSpec
from ..fields import (
    LAYER_REGISTRY,
    MAX_SPANS,
    SPAN_BYTES_PER_COLUMN,
    SPAN_SENTINEL,
    BakePlan,
    GeneratedFieldSet,
    TerrainGenerationPlan,
    encode_spans_arrays,
)
from ..spans_shim import spans_for_tile
from ..profiles import (
    GLOBAL_DECORATION_PALETTE,
    PROFILE_DECORATION_OFFSETS,
    get_profile_generator,
    list_profile_generators,
)
from ..profiles.base import DecorationSpec, EcologySpec
from ..profiles.spawn_plain import spawn_tutorial_pois_for_zone
from ..structures.ascension_pit import ascension_pits_for_zone
from ..structures.corpse_mound import corpse_mounds_for_zone
from ..structures.whale_fossil import fossil_bboxes_for_zone
from ...poi_novice_selector import build_novice_poi_manifest_payload

BIOME_PALETTE = (
    # ---- indices 0-11 (frozen — raster.rs hardcodes forest=7, river=8) ----
    "minecraft:plains",           # 0  default wilderness fallback
    "minecraft:stony_peaks",      # 1
    "minecraft:swamp",            # 2
    "minecraft:badlands",         # 3
    "minecraft:meadow",           # 4
    "minecraft:dripstone_caves",  # 5
    "minecraft:desert",           # 6
    "minecraft:forest",           # 7  (hardcoded in raster.rs forest_wilderness_biome)
    "minecraft:river",            # 8  (hardcoded in raster.rs river_wilderness_biome)
    "minecraft:frozen_peaks",     # 9
    "minecraft:mangrove_swamp",   # 10
    "minecraft:flower_forest",    # 11
    # ---- indices 12+ (append-only, plan-terrain-wiring-v1 P2/P3 #10) ----
    # index 12: restored to plains — rift_mouth_barrens and pseudo_vein_oasis
    # hungry_ring both hardcode biome_id=12 and expect a barren/plains fallback.
    "minecraft:plains",                 # 12  rift_mouth_barrens / pseudo_vein_oasis hungry_ring
    "minecraft:old_growth_pine_taiga",  # 13  dan_zong_yi_yuan (丹宗遗园 alchemy toxin terrain)
    # indices 14-16: plains placeholders (reserved for future zone biomes)
    "minecraft:plains",                 # 14  reserved placeholder
    "minecraft:plains",                 # 15  reserved placeholder
    "minecraft:plains",                 # 16  reserved placeholder
    "minecraft:windswept_hills",        # 17  wangyintai (忘音台 windswept highland ruins)
)

# Derived from the central LAYER_REGISTRY — no need to maintain separate lists.
FLOAT_LAYERS = {
    name for name, spec in LAYER_REGISTRY.items() if spec.export_type == "float32"
}
UINT8_LAYERS = {
    name for name, spec in LAYER_REGISTRY.items() if spec.export_type == "uint8"
}


# worldgen-v4 P0 §8.1 #1: the spans replace height.bin + the six vertical patch
# layers.  height stays in LAYER_REGISTRY for the stitcher's internal blend math,
# and the six patch layers are still emitted by the unrewritten profiles into the
# tile buffer for the shim to read — but NONE of them are written as standalone
# rasters; they are all folded into spans_count.bin + spans.bin at export.
SPANS_COUNT_FILE = "spans_count.bin"
SPANS_FILE = "spans.bin"
_SPAN_FOLDED_LAYERS = frozenset(
    {
        "height",
        "cave_mask",
        "ceiling_height",
        "entrance_mask",
        "sky_island_base_y",
        "sky_island_thickness",
        "cavern_floor_y",
    }
)


def _layer_file_name(layer_name: str) -> str:
    return f"{layer_name}.bin"


def _write_float_layer(path: Path, values: np.ndarray) -> None:
    arr = np.ascontiguousarray(values, dtype=np.float32)
    path.write_bytes(arr.tobytes())


def _write_u8_layer(path: Path, values: np.ndarray) -> None:
    arr = np.ascontiguousarray(values, dtype=np.uint8)
    path.write_bytes(arr.tobytes())


def _write_spans(tile_dir: Path, buffer) -> None:
    """Fold the tile's blended 2D layers into spans and write both files.

    spans_count.bin : u8 per column (0..=MAX_SPANS)
    spans.bin       : SPAN_BYTES_PER_COLUMN bytes per column, little-endian i16
                      pairs, sentinel-padded; Rust mmaps at col_idx * stride.
    """
    columns = spans_for_tile(buffer)
    count_arr, spans_arr = encode_spans_arrays(columns)
    (tile_dir / SPANS_COUNT_FILE).write_bytes(count_arr.tobytes())
    (tile_dir / SPANS_FILE).write_bytes(spans_arr.tobytes())


def export_rasters(
    plan: TerrainGenerationPlan,
    fields: GeneratedFieldSet,
    layer_whitelist: Optional[set[str]] = None,
) -> dict[str, Path]:
    """Export rasters; if layer_whitelist is given, only those layers are written.

    plan-tsy-worldgen-v1 §2.1 — 主世界 manifest 调用传 whitelist 过滤掉 tsy_*；
    TSY manifest 调用传 None 导全部。
    """
    if plan.bake_plan is None:
        raise ValueError("raster bake plan is required before export")

    output_dir = plan.bake_plan.output_dir
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    manifest_tiles: list[dict[str, object]] = []
    written_layer_names: set[str] = set()
    for tile in fields.tiles:
        tile_dir = output_dir / tile.tile.tile_id
        tile_dir.mkdir(parents=True, exist_ok=True)

        # worldgen-v4 P0: spans replace height + the deleted vertical layers.
        _write_spans(tile_dir, tile)

        written_layers: list[str] = []
        for layer_name in fields.layers:
            if layer_name not in tile.layers:
                continue
            if layer_name in _SPAN_FOLDED_LAYERS:
                # height is folded into spans.bin — never written standalone.
                continue
            if layer_whitelist is not None and layer_name not in layer_whitelist:
                continue
            layer_path = tile_dir / _layer_file_name(layer_name)
            values = tile.layers[layer_name]
            if layer_name in FLOAT_LAYERS:
                _write_float_layer(layer_path, values)
            elif layer_name in UINT8_LAYERS:
                # plan-terrain-wiring-v1 P2 #10: bake-time guard — any biome_id
                # that exceeds the palette bounds would silently degrade to
                # default_wilderness_biome at runtime (raster.rs :775).  Fail fast.
                if layer_name == "biome_id":
                    max_biome_id = int(values.max()) if len(values) > 0 else 0
                    palette_len = len(BIOME_PALETTE)
                    assert max_biome_id < palette_len, (
                        f"biome_id max={max_biome_id} >= len(BIOME_PALETTE)={palette_len} "
                        f"in tile {tile.tile.tile_id}; "
                        f"extend BIOME_PALETTE (append-only) to cover this id"
                    )
                _write_u8_layer(layer_path, values)
            else:
                raise ValueError(f"unsupported raster layer '{layer_name}'")
            written_layers.append(layer_name)
            written_layer_names.add(layer_name)

        manifest_tiles.append(
            {
                "tile_x": tile.tile.tile_x,
                "tile_z": tile.tile.tile_z,
                "dir": tile.tile.tile_id,
                "zones": list(tile.contributing_zones),
                "layers": written_layers,
                # worldgen-v4 P0: every tile carries the two span files; the
                # Rust reader mmaps spans_count.bin + spans.bin per tile.
                "spans": True,
            }
        )

    pois_payload = _collect_poi_payload(plan.blueprint_zones)
    pois_payload.extend(build_novice_poi_manifest_payload(fields))
    ecology_payload = _collect_profile_ecology()
    global_decoration_palette = _collect_global_decoration_palette()
    collapsed_zones_payload = _collect_collapsed_zone_payload(plan)
    fossil_bboxes = _collect_fossil_bboxes(plan.blueprint_zones)
    corpse_mounds = _collect_corpse_mounds(plan.blueprint_zones)
    ascension_pits = _collect_ascension_pits(plan.blueprint_zones)

    manifest = {
        "version": 2,
        "backend": "raster",
        "world_name": plan.world_name,
        "tile_size": fields.tile_size,
        # worldgen-v4 P0 §8.1 #1 — span column encoding (replaces height.bin +
        # the deleted vertical patch layers).  The Rust reader uses these
        # constants to mmap spans.bin at offset = col_idx * bytes_per_column.
        "spans_encoding": {
            "max_spans": MAX_SPANS,
            "bytes_per_column": SPAN_BYTES_PER_COLUMN,
            "sentinel": SPAN_SENTINEL,
            "count_file": SPANS_COUNT_FILE,
            "spans_file": SPANS_FILE,
            "slot_layout": "i16_le(floor_y, ceiling_y) × max_spans, "
            "unused slots = sentinel; surface = span[0].ceiling_y",
        },
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
            for name in (
                "qi_density",
                "mofa_decay",
                "qi_vein_flow",
                "neg_pressure",
                "portal_anchor_sdf",
                "realm_collapse_mask",
            )
            if name in written_layer_names
        ],
        "collapsed_zones": collapsed_zones_payload,
        # worldgen-v4 P0 §8.1 #1/#12: the geometric vertical patch layers
        # (sky_island_base_y/thickness, cave_mask, ceiling_height, entrance_mask,
        # cavern_floor_y) are folded into spans.  Only the SEMANTIC vertical
        # layers (sky_island_mask, underground_tier) remain as rasters because
        # the 灵草 environment locks key off them directly.
        "vertical_layers": [
            name
            for name in (
                "sky_island_mask",
                "underground_tier",
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
        "global_decoration_palette": global_decoration_palette,
        "structure_layers": [name for name in ("fossil_bbox",) if name in LAYER_REGISTRY],
        "fossil_bboxes": fossil_bboxes,
        "corpse_mounds": corpse_mounds,
        "ascension_pits": ascension_pits,
        "notes": [
            "Python exports 2D terrain fields only; block and biome realization happens in Rust.",
            "All tile layer payloads are little-endian raw binaries for mmap-friendly loading.",
            "worldgen-v4 P0: column vertical structure lives in spans_count.bin + spans.bin",
            "  (per-column solid (floor_y, ceiling_y) ranges, see manifest.spans_encoding).",
            "  These replace height.bin AND the old sky_island_base_y/thickness, cave_mask,",
            "  ceiling_height, entrance_mask, cavern_floor_y patch layers (folded by the shim).",
            "  surface_y = span[0].ceiling_y (walkable ground; caves carved below, isles above).",
            "Semantic layers (qi_density / mofa_decay / qi_vein_flow) carry the xianxia world model.",
            "Vertical SEMANTIC layers retained: sky_island_mask (gate isle flora) +",
            "  underground_tier (0..3) for the 5 灵草 environment locks (§8.1 #12).",
            "Ecology: flora_density (0..1) + flora_variant_id (uint8 index into ",
            "  profiles_ecology[zone_profile].decorations; 0 = no flora / wilderness fallback).",
            "Anomaly: anomaly_intensity (0..1) + anomaly_kind (uint8 from anomaly_kinds map).",
            "  Event systems trigger themed spawns / FX when intensity > 0.3.",
            "Rift mouth: portal_anchor_sdf stores meter-distance to the nearest entry anchor;",
            "  it is independent from rift_axis_sdf and gates portal/negative-pressure runtime logic.",
            "Structure: fossil_bbox (0 none, 1 outer, 2 core) marks whalefall fossils;",
            "  manifest.fossil_bboxes carries AABB metadata for mineral anchor materialization.",
            "Ash dead zone corpse_mounds carry dried surface loot: fan tie, rotten bone coin, dried herb.",
            "Tribulation scorch ascension_pits carry single-point basalt/obsidian pit metadata and xujie_canxie loot.",
            "POIs are zone-scoped narrative anchors for agent / NPC / HUD consumers.",
            "Realm collapse: realm_collapse_mask=1 marks persisted collapsed zones; keep blocks",
            "  but disable qi-dependent structures / shrines / furnaces in those columns.",
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
        seen = {(poi.kind, poi.name) for poi in zone.pois}
        for poi in zone.pois:
            payload.append(_poi_dict(zone.name, poi))
        if zone.worldgen.terrain_profile == "spawn_plain":
            for poi in spawn_tutorial_pois_for_zone(zone):
                if (poi.kind, poi.name) in seen:
                    continue
                payload.append(_poi_dict(zone.name, poi))
                seen.add((poi.kind, poi.name))
    return payload


def _collect_fossil_bboxes(zones: list[BlueprintZone]) -> list[dict[str, object]]:
    payload: list[dict[str, object]] = []
    for zone in zones:
        payload.extend(fossil_bboxes_for_zone(zone))
    return payload


def _collect_corpse_mounds(zones: list[BlueprintZone]) -> list[dict[str, object]]:
    payload: list[dict[str, object]] = []
    for zone in zones:
        payload.extend(corpse_mounds_for_zone(zone))
    return payload


def _collect_ascension_pits(zones: list[BlueprintZone]) -> list[dict[str, object]]:
    payload: list[dict[str, object]] = []
    for zone in zones:
        payload.extend(ascension_pits_for_zone(zone))
    return payload


def _decoration_dict(
    profile_name: str, local_idx: int, deco: DecorationSpec
) -> dict[str, object]:
    """Per-profile decoration view that carries both local and global ids."""
    local_id = local_idx + 1
    offset = PROFILE_DECORATION_OFFSETS.get(profile_name, 0)
    global_id = offset + local_idx if offset > 0 else 0
    return {
        "local_id": local_id,
        "global_id": global_id,
        "name": deco.name,
        "kind": deco.kind,
        "blocks": list(deco.blocks),
        "size_range": list(deco.size_range),
        "rarity": deco.rarity,
        "notes": deco.notes,
    }


def _ecology_dict(profile_name: str, spec: EcologySpec) -> dict[str, object]:
    # variant_id 0 is reserved for "no flora"; actual palette starts at 1.
    return {
        "decorations": [
            _decoration_dict(profile_name, i, d)
            for i, d in enumerate(spec.decorations)
        ],
        "ambient_effects": list(spec.ambient_effects),
        "notes": spec.notes,
    }


def _collect_profile_ecology() -> dict[str, object]:
    payload: dict[str, object] = {}
    for profile_name in list_profile_generators():
        gen = get_profile_generator(profile_name)
        payload[profile_name] = _ecology_dict(profile_name, gen.ecology)
    return payload


def _collect_global_decoration_palette() -> list[dict[str, object]]:
    """Return the flat global palette for the raster manifest.

    flora_variant_id rasters are written in global-id space (see stitcher
    _remap_flora_variant_to_global). Consumers index this list directly.
    """
    return [dict(entry) for entry in GLOBAL_DECORATION_PALETTE]


def _collect_collapsed_zone_payload(
    plan: TerrainGenerationPlan,
) -> list[dict[str, object]]:
    zones_by_name = {zone.name: zone for zone in plan.blueprint_zones}
    payload: list[dict[str, object]] = []
    seen: set[str] = set()
    for overlay in plan.zone_overlays:
        if overlay.overlay_kind != "collapsed":
            continue
        if overlay.payload.get("zone_status") != "collapsed":
            continue
        if overlay.zone_id in seen:
            continue
        seen.add(overlay.zone_id)
        zone = zones_by_name.get(overlay.zone_id)
        entry: dict[str, object] = {
            "zone_id": overlay.zone_id,
            "zone_status": "collapsed",
            "payload_version": overlay.payload_version,
            "since_wall": overlay.since_wall,
            "active_events": list(overlay.payload.get("active_events", [])),
        }
        if zone is not None:
            entry["display_name"] = zone.display_name
            entry["dimension"] = zone.dimension
            entry["bounds_xz"] = {
                "min_x": zone.bounds_xz.min_x,
                "max_x": zone.bounds_xz.max_x,
                "min_z": zone.bounds_xz.min_z,
                "max_z": zone.bounds_xz.max_z,
            }
        payload.append(entry)
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
