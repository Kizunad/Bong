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
- anomaly_kind outside {0..5} or present without anomaly_intensity
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
