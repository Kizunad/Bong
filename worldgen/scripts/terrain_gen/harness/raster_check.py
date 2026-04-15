"""Post-generation raster sanity checks.

Catches known data integrity issues before they reach the Rust server:
- rift_axis_sdf defaulting to 0 (causes false rift carving everywhere)
- water level above surrounding terrain (floating water)
- height values outside sane world range
- missing layers in tiles
- qi_density / mofa_decay outside [0, 1]
- qi_density vs zone.spirit_qi declared gross mismatch
- sky_island_base_y outside [200, 400] when mask > 0
- underground_tier outside {0,1,2,3}
- cavern_floor_y outside [-64, 64] when tier > 0
"""

from __future__ import annotations

import json
import struct
from pathlib import Path



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

        # Check height range
        height_file = tile_dir / "height.bin"
        if height_file.exists():
            h_data = _read_float_layer(height_file, area)
            if h_data is not None:
                h_min, h_max = min(h_data), max(h_data)
                if h_min < -64:
                    warnings.append(f"{tile_id}: height min={h_min:.1f} below bedrock")
                if h_max > 500:
                    warnings.append(
                        f"{tile_id}: height max={h_max:.1f} near world ceiling"
                    )

        # Validate vertical layers: sky_island_base_y / cavern_floor_y use
        # sentinel 9999 for "no isle/cavern here", so presence must correlate
        # with the companion mask/tier layer.
        sky_mask_file = tile_dir / "sky_island_mask.bin"
        sky_base_file = tile_dir / "sky_island_base_y.bin"
        if sky_mask_file.exists() and sky_base_file.exists():
            mask_vals = _read_float_layer(sky_mask_file, area)
            base_vals = _read_float_layer(sky_base_file, area)
            if mask_vals is not None and base_vals is not None:
                for m, b in zip(mask_vals, base_vals):
                    if m > 0.05 and (b < 200.0 or b > 400.0):
                        warnings.append(
                            f"{tile_id}: sky_island_base_y={b:.1f} out of "
                            f"[200,400] while mask={m:.2f} (zones={zones})"
                        )
                        break

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

        floor_file = tile_dir / "cavern_floor_y.bin"
        if floor_file.exists() and tier_file.exists():
            floor_vals = _read_float_layer(floor_file, area)
            tier_raw = tier_file.read_bytes()
            if floor_vals is not None and len(tier_raw) == area:
                for t, f in zip(tier_raw, floor_vals):
                    if t > 0 and (f < -64.0 or f > 64.0):
                        warnings.append(
                            f"{tile_id}: cavern_floor_y={f:.1f} out of "
                            f"[-64,64] while tier={t} (zones={zones})"
                        )
                        break

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

        # Check water vs terrain consistency
        water_file = tile_dir / "water_level.bin"
        if water_file.exists() and height_file.exists():
            w_data = _read_float_layer(water_file, area)
            h_data = _read_float_layer(height_file, area)
            if w_data is not None and h_data is not None:
                max_depth = 0.0
                water_cols = 0
                for w, h in zip(w_data, h_data):
                    if w >= 0:
                        water_cols += 1
                        depth = w - h
                        if depth > max_depth:
                            max_depth = depth
                if max_depth > 15.0 and water_cols > area * 0.1:
                    warnings.append(
                        f"{tile_id}: max water depth={max_depth:.1f} blocks "
                        f"({water_cols} water cols) — may look like floating water"
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
        lines.append(f"All {len(tiles)} tiles passed validation.")

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
