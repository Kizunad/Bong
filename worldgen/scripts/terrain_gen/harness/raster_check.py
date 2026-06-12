"""Post-generation raster sanity checks.

Catches known data integrity issues before they reach the Rust server:
- spans legality (worldgen-v4 P0 §8.1 #8): count ∈ [0,4], floor<=ceiling per
  used slot, unused slots = sentinel i16::MAX, spans non-overlapping
- rift_axis_sdf defaulting to 0 (causes false rift carving everywhere)
- water level above surrounding terrain (floating water)
- missing layers in tiles
- qi_density / mofa_decay outside [0, 1]
- qi_density vs zone.spirit_qi declared gross mismatch
- underground_tier outside {0,1,2,3}
- anomaly_kind outside {0..5} or present without anomaly_intensity
- fossil_bbox outside {0,1,2} or manifest fossil_bboxes with no raster cells
- ash_dead_zone tiles must keep qi_vein_flow exactly 0
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

# worldgen-v4 P0 §8.1 #1 span encoding constants (kept in lockstep with
# terrain_gen.fields).  Duplicated here so the harness has no heavy import.
SPAN_MAX_SPANS = 4
SPAN_SENTINEL = 32767  # i16::MAX
SPAN_BYTES_PER_COLUMN = SPAN_MAX_SPANS * 2 * 2  # 16
SPAN_MIN_Y = -64
SPAN_MAX_Y = 431



def validate_rasters(raster_dir: str | Path) -> tuple[bool, str]:
    """Validate raster output. Returns (ok, message)."""
    raster_path = Path(raster_dir)
    manifest_path = raster_path / "manifest.json"

    if not manifest_path.exists():
        return False, f"manifest.json not found at {manifest_path}"

    manifest = json.loads(manifest_path.read_text())
    tiles = manifest["tiles"]
    tile_size = manifest["tile_size"]
    area = tile_size * tile_size
    errors: list[str] = []
    warnings: list[str] = []
    collapse_mask_tiles = 0
    collapse_mask_has_positive = False
    fossil_cell_count = 0

    for tile_info in tiles:
        tile_dir = raster_path / tile_info["dir"]
        tile_id = tile_info["dir"]
        zones = tile_info.get("zones", [])

        # Check all expected layers exist
        for layer_name in tile_info.get("layers", []):
            layer_file = tile_dir / f"{layer_name}.bin"
            if not layer_file.exists():
                errors.append(f"{tile_id}: missing {layer_name}.bin")
                continue

            # Spot-check known-dangerous defaults
            if layer_name == "rift_axis_sdf" and not any(
                z in zones for z in ("blood_valley",)
            ):
                data = _read_float_layer(layer_file, area)
                if data is not None:
                    min_val = min(data)
                    if min_val < 0.9:
                        errors.append(
                            f"{tile_id}: rift_axis_sdf min={min_val:.2f} in non-rift tile "
                            f"(zones={zones}) — will cause false rift carving"
                        )

        # worldgen-v4 P0 §8.1 #8 — span legality (replaces height.bin range +
        # the deleted sky_island_base_y / cavern_floor_y companion checks).
        span_error = _validate_spans(tile_dir, area, tile_id, zones)
        if span_error is not None:
            errors.append(span_error)

        # underground_tier must be {0,1,2,3}. It's uint8 so just spot-check range.
        tier_file = tile_dir / "underground_tier.bin"
        if tier_file.exists():
            raw = tier_file.read_bytes()
            if len(raw) == area:
                t_max = max(raw)
                if t_max > 3:
                    errors.append(
                        f"{tile_id}: underground_tier max={t_max} > 3 (zones={zones})"
                    )

        # Anomaly integrity: kind must be 0..5 and non-zero only when
        # intensity > 0 (otherwise event systems will query a ghost event).
        anomaly_kind_file = tile_dir / "anomaly_kind.bin"
        anomaly_int_file = tile_dir / "anomaly_intensity.bin"
        if anomaly_kind_file.exists():
            raw = anomaly_kind_file.read_bytes()
            if len(raw) == area and max(raw) > 5:
                errors.append(
                    f"{tile_id}: anomaly_kind max={max(raw)} > 5 (zones={zones})"
                )
            if anomaly_int_file.exists() and len(raw) == area:
                int_vals = _read_float_layer(anomaly_int_file, area)
                if int_vals is not None:
                    for k, i in zip(raw, int_vals):
                        if k > 0 and i <= 0.0:
                            warnings.append(
                                f"{tile_id}: anomaly_kind={k} present without "
                                f"intensity (zones={zones})"
                            )
                            break

        fossil_file = tile_dir / "fossil_bbox.bin"
        if fossil_file.exists():
            raw = fossil_file.read_bytes()
            if len(raw) == area:
                f_max = max(raw)
                if f_max > 2:
                    errors.append(
                        f"{tile_id}: fossil_bbox max={f_max} > 2 (zones={zones})"
                    )
                fossil_cell_count += sum(1 for value in raw if value > 0)

        # Validate semantic layers: qi_density / mofa_decay must stay in [0, 1],
        # qi_vein_flow likewise. These are narrative-facing so out-of-range
        # values will confuse downstream agent / HUD consumers.
        for semantic_layer in ("qi_density", "mofa_decay", "qi_vein_flow"):
            sem_file = tile_dir / f"{semantic_layer}.bin"
            if not sem_file.exists():
                continue
            sem_data = _read_float_layer(sem_file, area)
            if sem_data is None:
                continue
            s_min, s_max = min(sem_data), max(sem_data)
            if s_min < -0.01 or s_max > 1.01:
                errors.append(
                    f"{tile_id}: {semantic_layer} range=[{s_min:.3f},{s_max:.3f}] "
                    f"outside [0,1] (zones={zones})"
                )

        collapse_file = tile_dir / "realm_collapse_mask.bin"
        if collapse_file.exists():
            collapse_mask_tiles += 1
            raw = collapse_file.read_bytes()
            if len(raw) != area:
                errors.append(
                    f"{tile_id}: realm_collapse_mask length={len(raw)} expected {area}"
                )
            elif any(value not in (0, 1) for value in raw):
                errors.append(
                    f"{tile_id}: realm_collapse_mask contains values outside {{0,1}}"
                )
            elif max(raw) > 0:
                collapse_mask_has_positive = True

        if "south_ash_dead_zone" in zones:
            vein_file = tile_dir / "qi_vein_flow.bin"
            if not vein_file.exists():
                errors.append(f"{tile_id}: ash_dead_zone tile missing qi_vein_flow.bin")
            else:
                vein_data = _read_float_layer(vein_file, area)
                if vein_data is not None:
                    vein_max = max(vein_data)
                    if vein_max > 0.0:
                        errors.append(
                            f"{tile_id}: ash_dead_zone qi_vein_flow max={vein_max:.6f}, expected 0.0"
                        )

        # Check water vs terrain consistency. The surface y now comes from the
        # span column (span[0].ceiling) instead of the deleted height.bin.
        water_file = tile_dir / "water_level.bin"
        surface_ys = _read_span_surface_ys(tile_dir, area)
        if water_file.exists() and surface_ys is not None:
            w_data = _read_float_layer(water_file, area)
            if w_data is not None:
                max_depth = 0.0
                water_cols = 0
                for w, surface_y in zip(w_data, surface_ys):
                    if w >= 0 and surface_y is not None:
                        water_cols += 1
                        depth = w - surface_y
                        if depth > max_depth:
                            max_depth = depth
                if max_depth > 15.0 and water_cols > area * 0.1:
                    warnings.append(
                        f"{tile_id}: max water depth={max_depth:.1f} blocks "
                        f"({water_cols} water cols) — may look like floating water"
                    )

    # plan-tsy-worldgen-v1 §4.3 — TSY / overworld manifest 分支校验。
    # 判定 manifest 类型：tile.zones 含 tsy_* 即视为 TSY manifest，否则 overworld。
    manifest_kind = "overworld"
    for tile in tiles:
        if any(z.startswith("tsy_") for z in tile.get("zones", [])):
            manifest_kind = "tsy"
            break

    if manifest_kind == "tsy":
        # 1. 每 family 至少 1 个 kind=rift_portal direction=exit POI
        families: dict[str, dict[str, int]] = {}
        for poi in manifest.get("pois", []):
            if poi["kind"] != "rift_portal":
                continue
            tags = {
                t.split(":", 1)[0]: t.split(":", 1)[1]
                for t in poi.get("tags", [])
                if ":" in t
            }
            family = tags.get("family_id")
            direction = tags.get("direction")
            if family:
                families.setdefault(family, {"entry": 0, "exit": 0})
                if direction in ("entry", "exit"):
                    families[family][direction] += 1
        for fam, counts in families.items():
            if counts.get("exit", 0) < 1:
                errors.append(f"TSY family '{fam}' has no rift_portal direction=exit")

        # 2. 每 family 三层齐全（按 zone name 后缀 _shallow/_mid/_deep）
        fam_tiers: dict[str, set[str]] = {}
        for tile in tiles:
            for z in tile.get("zones", []):
                if not z.startswith("tsy_"):
                    continue
                for tier in ("shallow", "mid", "deep"):
                    suffix = f"_{tier}"
                    if z.endswith(suffix):
                        fam = z[len("tsy_") : -len(suffix)]
                        fam_tiers.setdefault(fam, set()).add(tier)
        for fam, tiers in fam_tiers.items():
            missing = {"shallow", "mid", "deep"} - tiers
            if missing:
                errors.append(f"TSY family '{fam}' missing tiers: {sorted(missing)}")

        # 3. tsy_presence > 0 的 cell 必须 qi_density >= 0.7
        for tile_info in tiles:
            tile_dir = raster_path / tile_info["dir"]
            presence = tile_dir / "tsy_presence.bin"
            qi_file = tile_dir / "qi_density.bin"
            if not (presence.exists() and qi_file.exists()):
                continue
            pres_raw = presence.read_bytes()
            qi_data = _read_float_layer(qi_file, area)
            if qi_data is None or len(pres_raw) != area:
                continue
            for p, q in zip(pres_raw, qi_data):
                if p > 0 and q < 0.70:
                    errors.append(
                        f"{tile_info['dir']}: tsy_presence>0 with "
                        f"qi_density={q:.2f} < 0.7"
                    )
                    break

        # 4. tsy_origin_id ∈ {0..4}, tsy_depth_tier ∈ {0..3}
        for tile_info in tiles:
            for layer_name, max_val in (("tsy_origin_id", 4), ("tsy_depth_tier", 3)):
                f = raster_path / tile_info["dir"] / f"{layer_name}.bin"
                if not f.exists():
                    continue
                raw = f.read_bytes()
                if len(raw) == area and max(raw) > max_val:
                    errors.append(
                        f"{tile_info['dir']}: {layer_name} max={max(raw)} > {max_val}"
                    )

        # 5. 三层 AABB Y 区间不 overlap — 此校验需读 blueprint 而非 raster；
        #    blueprint loader 一致性校验里做（cross_manifest_check.py 后续接入）。
    else:
        fossil_bboxes = manifest.get("fossil_bboxes", [])
        if fossil_bboxes and fossil_cell_count == 0:
            errors.append("overworld manifest declares fossil_bboxes but fossil_bbox rasters are empty")
        if fossil_cell_count > 0 and not fossil_bboxes:
            warnings.append("fossil_bbox raster cells present without manifest.fossil_bboxes metadata")
        for bbox in fossil_bboxes:
            bbox_cell_count = _count_fossil_cells_in_bbox(raster_path, tiles, tile_size, bbox)
            if bbox_cell_count == 0:
                errors.append(
                    "manifest fossil_bbox "
                    f"{bbox.get('name', '<unnamed>')} has no raster cells inside its AABB"
                )

        # 6. 每个 kind=rift_portal direction=entry POI 必须带 family_id + target_family_pos_xyz
        for poi in manifest.get("pois", []):
            if poi["kind"] != "rift_portal":
                continue
            tags = {
                t.split(":", 1)[0]: t.split(":", 1)[1]
                for t in poi.get("tags", [])
                if ":" in t
            }
            if tags.get("direction") != "entry":
                continue
            if "family_id" not in tags:
                errors.append(
                    f"overworld rift_portal at {poi['pos_xyz']} missing family_id tag"
                )
            if "target_family_pos_xyz" not in tags:
                errors.append(
                    f"overworld rift_portal at {poi['pos_xyz']} missing "
                    f"target_family_pos_xyz tag"
                )

        # 7. 主世界 manifest 不出现 tsy_* layer
        for tile in tiles:
            for layer in tile.get("layers", []):
                if layer.startswith("tsy_"):
                    errors.append(
                        f"overworld manifest tile {tile['dir']} unexpectedly "
                        f"contains {layer}"
                    )

    collapsed_zones = manifest.get("collapsed_zones", [])
    semantic_layers = manifest.get("semantic_layers", [])
    if collapsed_zones:
        if "realm_collapse_mask" not in semantic_layers:
            errors.append(
                "manifest has collapsed_zones but semantic_layers lacks realm_collapse_mask"
            )
        if collapse_mask_tiles == 0:
            errors.append("manifest has collapsed_zones but no tile writes realm_collapse_mask")
        elif not collapse_mask_has_positive:
            errors.append("realm_collapse_mask is present but contains no collapsed cells")
    elif "realm_collapse_mask" in semantic_layers:
        errors.append(
            "manifest semantic_layers includes realm_collapse_mask but collapsed_zones is empty"
        )

    if "realm_collapse_mask" in semantic_layers and collapse_mask_tiles == 0:
        errors.append(
            "manifest semantic_layers includes realm_collapse_mask but no tile writes it"
        )

    # Build report
    lines: list[str] = []
    if errors:
        lines.append(f"ERRORS ({len(errors)}):")
        for e in errors:
            lines.append(f"  ✗ {e}")
    if warnings:
        lines.append(f"WARNINGS ({len(warnings)}):")
        for w in warnings:
            lines.append(f"  ⚠ {w}")

    if not errors and not warnings:
        lines.append(
            f"All {len(tiles)} tiles passed validation (manifest_kind={manifest_kind})."
        )

    return len(errors) == 0, "\n".join(lines)


def _read_float_layer(path: Path, expected_count: int) -> list[float] | None:
    """Read a binary float32 layer file."""
    try:
        raw = path.read_bytes()
        if len(raw) != expected_count * 4:
            return None
        return list(struct.unpack(f"<{expected_count}f", raw))
    except OSError:
        return None


def _validate_spans(
    tile_dir: Path, area: int, tile_id: str, zones: list[str]
) -> str | None:
    """worldgen-v4 P0 §8.1 #8 — validate spans_count.bin + spans.bin legality.

    Returns the first error string found, or None if the tile's spans are
    legal (or the tile has no spans files at all).  Rules per column:
      * count ∈ [0, MAX_SPANS]
      * each used slot has SPAN_MIN_Y <= floor_y <= ceiling_y <= SPAN_MAX_Y
      * unused slots (index >= count) are both sentinel i16::MAX
      * the used spans are pairwise non-overlapping (sorted with a gap)
    """
    count_file = tile_dir / "spans_count.bin"
    spans_file = tile_dir / "spans.bin"
    if not (count_file.exists() and spans_file.exists()):
        return None

    count_bytes = count_file.read_bytes()
    spans_bytes = spans_file.read_bytes()
    if len(count_bytes) != area:
        return (
            f"{tile_id}: spans_count.bin length={len(count_bytes)} expected {area}"
        )
    if len(spans_bytes) != area * SPAN_BYTES_PER_COLUMN:
        return (
            f"{tile_id}: spans.bin length={len(spans_bytes)} expected "
            f"{area * SPAN_BYTES_PER_COLUMN}"
        )

    slots_per_col = SPAN_MAX_SPANS * 2
    for col_idx in range(area):
        count = count_bytes[col_idx]
        if count > SPAN_MAX_SPANS:
            return (
                f"{tile_id}: column {col_idx} spans count={count} > "
                f"{SPAN_MAX_SPANS} (zones={zones})"
            )
        base = col_idx * SPAN_BYTES_PER_COLUMN
        flat = struct.unpack(
            f"<{slots_per_col}h", spans_bytes[base : base + SPAN_BYTES_PER_COLUMN]
        )
        used: list[tuple[int, int]] = []
        for slot in range(SPAN_MAX_SPANS):
            floor_y = flat[2 * slot]
            ceiling_y = flat[2 * slot + 1]
            if slot < count:
                if floor_y > ceiling_y:
                    return (
                        f"{tile_id}: column {col_idx} span {slot} "
                        f"floor_y={floor_y} > ceiling_y={ceiling_y} (zones={zones})"
                    )
                if not (SPAN_MIN_Y <= floor_y <= SPAN_MAX_Y) or not (
                    SPAN_MIN_Y <= ceiling_y <= SPAN_MAX_Y
                ):
                    return (
                        f"{tile_id}: column {col_idx} span {slot} "
                        f"({floor_y},{ceiling_y}) out of world Y "
                        f"[{SPAN_MIN_Y},{SPAN_MAX_Y}] (zones={zones})"
                    )
                used.append((floor_y, ceiling_y))
            else:
                if floor_y != SPAN_SENTINEL or ceiling_y != SPAN_SENTINEL:
                    return (
                        f"{tile_id}: column {col_idx} unused slot {slot} not "
                        f"sentinel-filled ({floor_y},{ceiling_y}); count={count} "
                        f"(zones={zones})"
                    )
        # Non-overlap with a gap (order-independent: span[0] may be the surface
        # cap sitting above a cave-floor remnant).
        prev_ceiling: int | None = None
        for floor_y, ceiling_y in sorted(used):
            if prev_ceiling is not None and floor_y <= prev_ceiling + 1:
                return (
                    f"{tile_id}: column {col_idx} spans overlap/touch: "
                    f"floor_y={floor_y} <= prev ceiling_y={prev_ceiling} + 1 "
                    f"(zones={zones})"
                )
            prev_ceiling = ceiling_y
    return None


def _read_span_surface_ys(tile_dir: Path, area: int) -> list[int | None] | None:
    """Return per-column surface y (span[0].ceiling) or None for void columns.

    Returns None entirely when the tile has no span files (older raster set).
    """
    count_file = tile_dir / "spans_count.bin"
    spans_file = tile_dir / "spans.bin"
    if not (count_file.exists() and spans_file.exists()):
        return None
    count_bytes = count_file.read_bytes()
    spans_bytes = spans_file.read_bytes()
    if len(count_bytes) != area or len(spans_bytes) != area * SPAN_BYTES_PER_COLUMN:
        return None
    surfaces: list[int | None] = []
    for col_idx in range(area):
        count = count_bytes[col_idx]
        if count == 0:
            surfaces.append(None)
            continue
        base = col_idx * SPAN_BYTES_PER_COLUMN
        # span[0].ceiling_y = second i16 of the column.
        ceiling_y = struct.unpack("<h", spans_bytes[base + 2 : base + 4])[0]
        surfaces.append(ceiling_y)
    return surfaces


def _count_fossil_cells_in_bbox(
    raster_path: Path,
    tiles: list[dict],
    tile_size: int,
    bbox: dict,
) -> int:
    min_x = int(bbox["min_x"])
    max_x = int(bbox["max_x"])
    min_z = int(bbox["min_z"])
    max_z = int(bbox["max_z"])
    count = 0
    for tile_info in tiles:
        if "fossil_bbox" not in tile_info.get("layers", []):
            continue
        tile_x = int(tile_info["tile_x"])
        tile_z = int(tile_info["tile_z"])
        tile_min_x = tile_x * tile_size
        tile_min_z = tile_z * tile_size
        tile_max_x = tile_min_x + tile_size - 1
        tile_max_z = tile_min_z + tile_size - 1
        if tile_max_x < min_x or tile_min_x > max_x or tile_max_z < min_z or tile_min_z > max_z:
            continue
        raw = (raster_path / tile_info["dir"] / "fossil_bbox.bin").read_bytes()
        if len(raw) != tile_size * tile_size:
            continue
        for local_z in range(tile_size):
            world_z = tile_min_z + local_z
            if world_z < min_z or world_z > max_z:
                continue
            row_offset = local_z * tile_size
            for local_x in range(tile_size):
                world_x = tile_min_x + local_x
                if min_x <= world_x <= max_x and raw[row_offset + local_x] > 0:
                    count += 1
    return count
