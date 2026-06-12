from __future__ import annotations

import argparse
import logging
from pathlib import Path
from typing import Optional

from .blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    REPO_ROOT,
    WORLDGEN_ROOT,
    WorldBlueprint,
    load_blueprint,
    load_profile_catalog,
    load_zone_overlays,
)
from .bakers.raster_export import build_raster_bake_plan, export_rasters
from .bakers.worldpainter import (
    build_worldpainter_bake_plan,
    export_worldpainter_rasters,
)
from .exporters import (
    format_field_summary,
    format_summary,
    write_field_summary_json,
    write_plan_json,
    write_preview_images,
)
from .fields import LAYER_REGISTRY
from .layouts import COMPOUND_LAYOUT_REGISTRY, export_placement_manifest, run_layout
from .stitcher import build_generation_plan, synthesize_fields

logger = logging.getLogger(__name__)

DEFAULT_OUTPUT_DIR = WORLDGEN_ROOT / "generated" / "terrain-gen"
DEFAULT_TSY_OUTPUT_DIR = WORLDGEN_ROOT / "generated" / "terrain-gen-tsy"

# plan-tsy-worldgen-v1 §4.1 — 主世界 manifest 不写 TSY 专用 layer。
TSY_ONLY_LAYERS = frozenset({"tsy_presence", "tsy_origin_id", "tsy_depth_tier"})


_STRUCTURES_DIR = REPO_ROOT / "server" / "structures"

# Map from layout name to NBT structures subdirectory under server/structures/.
# Derived from the layout naming convention: "dan_zong_compound" → "dan_zong".
# Add new entries here when new compound layouts are registered.
_LAYOUT_NBT_SUBDIR: dict[str, str] = {
    "dan_zong_compound": "dan_zong",
    "wangyintai_compound": "wangyintai",
}


def run_layout_pass(blueprint: WorldBlueprint, output_dir: Path) -> Path | None:
    """Export pass: for each zone with an architectural_layout, run the layout
    and write a sidecar ``placement_manifest.json``.

    plan-terrain-wiring-v1 P0 #1/M1:
    - Zones with ``architectural_layout`` set are looked up in
      ``COMPOUND_LAYOUT_REGISTRY``.
    - Zones whose required POI kind is missing from zone.pois are
      warned about and skipped (M1 guard — prevents ValueError crashing the
      whole pass; the pipeline continues for other zones).
    - Output: ``<output_dir>/rasters/placement_manifest.json``.

    plan-terrain-wiring-v1 P2: NBT bare filenames (e.g. ``"dan_zong_great_hall.nbt"``)
    are resolved against ``server/structures/<subdir>/`` using ``_LAYOUT_NBT_SUBDIR``.

    Returns the output path on success, or None if no layouts were processed.
    """
    rasters_dir = output_dir / "rasters"
    rasters_dir.mkdir(parents=True, exist_ok=True)
    output_path = rasters_dir / "placement_manifest.json"

    from .layouts.runner import NbtPasteResult

    all_paste_results: list[NbtPasteResult] = []
    processed = 0

    for zone in blueprint.zones:
        layout_name = zone.architectural_layout
        if not layout_name:
            continue

        if layout_name not in COMPOUND_LAYOUT_REGISTRY:
            logger.warning(
                "Zone '%s': architectural_layout '%s' not found in "
                "COMPOUND_LAYOUT_REGISTRY — skipping",
                zone.name, layout_name,
            )
            continue

        spec = COMPOUND_LAYOUT_REGISTRY[layout_name]
        # M1 guard: warn+skip if required POI kind is absent
        poi_kinds = {p.kind for p in zone.pois}
        if spec.poi_kind not in poi_kinds:
            logger.warning(
                "Zone '%s': layout '%s' requires POI kind '%s' but zone.pois "
                "has %s — skipping (M1 guard)",
                zone.name, layout_name, spec.poi_kind, sorted(poi_kinds),
            )
            continue

        # Resolve NBT base directory for this layout (P2 path wiring).
        nbt_subdir = _LAYOUT_NBT_SUBDIR.get(layout_name)
        nbt_base_dir: str | None = None
        if nbt_subdir:
            candidate = _STRUCTURES_DIR / nbt_subdir
            if candidate.is_dir():
                nbt_base_dir = str(candidate)
            else:
                logger.warning(
                    "Zone '%s': NBT structures dir '%s' not found — "
                    "NBT placements will produce empty blocks",
                    zone.name, candidate,
                )

        try:
            layout_result = run_layout(spec, zone, nbt_base_dir=nbt_base_dir)
        except ValueError as exc:
            logger.warning(
                "Zone '%s': run_layout failed: %s — skipping",
                zone.name, exc,
            )
            continue

        paste_results = list(layout_result.paste_results)
        total_blocks = sum(pr.block_count for pr in paste_results)
        logger.info(
            "Zone '%s' layout '%s': %d placements, %d total blocks",
            zone.name, layout_name, len(paste_results), total_blocks,
        )
        all_paste_results.extend(paste_results)
        processed += 1

    if processed == 0:
        logger.info("run_layout_pass: no zones with architectural_layout found — no manifest written")
        return None

    export_placement_manifest([r for r in all_paste_results], str(output_path))
    logger.info("run_layout_pass: wrote %s", output_path)
    return output_path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Scaffold blueprint-driven terrain generation plan"
    )
    parser.add_argument(
        "--blueprint",
        type=Path,
        default=DEFAULT_BLUEPRINT_PATH,
        help="Path to the world blueprint JSON",
    )
    parser.add_argument(
        "--profiles",
        type=Path,
        default=DEFAULT_PROFILES_PATH,
        help="Path to the terrain profile catalog JSON",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help="Directory to write the generated plan metadata",
    )
    parser.add_argument(
        "--tile-size",
        type=int,
        default=512,
        help="Tile size for field planning",
    )
    parser.add_argument(
        "--backend",
        choices=("raster", "worldpainter"),
        default="raster",
        help="Bake backend to prepare metadata for",
    )
    parser.add_argument(
        "--tsy-blueprint",
        type=Path,
        default=None,
        help="Optional TSY-dim blueprint JSON; runs a second export pass into --tsy-output-dir",
    )
    parser.add_argument(
        "--tsy-output-dir",
        type=Path,
        default=DEFAULT_TSY_OUTPUT_DIR,
        help="Directory for the TSY-dim raster export (only used when --tsy-blueprint is given)",
    )
    parser.add_argument(
        "--zone-overlays",
        type=Path,
        default=None,
        help="Optional zones_export_v1 JSON carrying persisted zone_overlays from the server",
    )
    parser.add_argument(
        "--tsy-zone-overlays",
        type=Path,
        default=None,
        help="Optional TSY-dim zones_export_v1 JSON carrying persisted zone_overlays",
    )
    parser.add_argument(
        "--zone-filter",
        type=str,
        default=None,
        help=(
            "Comma-separated zone names to synthesize. Intended for preview/"
            "incremental runs; omitted means full active worldgen."
        ),
    )
    return parser.parse_args()


def _parse_zone_filter(raw: str | None) -> set[str] | None:
    if raw is None:
        return None
    zones = {part.strip() for part in raw.split(",") if part.strip()}
    return zones or None


def _run_pipeline(
    blueprint_path: Path,
    profiles_path: Path,
    output_dir: Path,
    tile_size: int,
    backend: str,
    *,
    layer_whitelist: Optional[set[str]] = None,
    label: str = "",
    zone_overlays_path: Optional[Path] = None,
    zone_filter: Optional[set[str]] = None,
) -> None:
    """Single export pass; mirrors original `main()` body."""
    if label:
        print(f"\n=== {label} ===")
    blueprint = load_blueprint(blueprint_path)
    profile_catalog = load_profile_catalog(profiles_path)
    zone_overlays = load_zone_overlays(zone_overlays_path)
    plan = build_generation_plan(
        blueprint=blueprint,
        profile_catalog=profile_catalog,
        blueprint_path=blueprint_path,
        profiles_path=profiles_path,
        output_dir=output_dir,
        tile_size=tile_size,
        zone_overlays=zone_overlays,
    )
    if backend == "worldpainter":
        plan.bake_plan = build_worldpainter_bake_plan(plan, output_dir)
    else:
        plan.bake_plan = build_raster_bake_plan(plan, output_dir)

    plan_path = write_plan_json(plan, output_dir)
    field_set = synthesize_fields(plan, zone_filter=zone_filter)
    field_summary_path = write_field_summary_json(field_set, output_dir)
    preview_paths = write_preview_images(plan, field_set, output_dir)
    bake_artifacts: dict[str, Path] = {}
    if backend == "worldpainter":
        bake_artifacts = export_worldpainter_rasters(plan, field_set)
    elif backend == "raster":
        bake_artifacts = export_rasters(plan, field_set, layer_whitelist=layer_whitelist)
    print(format_summary(plan, plan_path))
    print(format_field_summary(field_set, field_summary_path))
    for preview_label, preview_path in preview_paths.items():
        print(f"  {preview_label}: {preview_path}")
    for artifact_label, artifact_path in bake_artifacts.items():
        print(f"  bake_{artifact_label}: {artifact_path}")

    # plan-terrain-wiring-v1 P0 #1/M1 — layout export pass (runs after rasters).
    manifest_path = run_layout_pass(blueprint, output_dir)
    if manifest_path is not None:
        print(f"  placement_manifest: {manifest_path}")


def main() -> None:
    args = parse_args()
    zone_filter = _parse_zone_filter(args.zone_filter)
    overworld_whitelist = set(LAYER_REGISTRY.keys()) - TSY_ONLY_LAYERS
    _run_pipeline(
        args.blueprint,
        args.profiles,
        args.output_dir,
        args.tile_size,
        args.backend,
        layer_whitelist=overworld_whitelist if args.backend == "raster" else None,
        label="overworld" if args.tsy_blueprint else "",
        zone_overlays_path=args.zone_overlays,
        zone_filter=zone_filter,
    )
    if args.tsy_blueprint is not None:
        _run_pipeline(
            args.tsy_blueprint,
            args.profiles,
            args.tsy_output_dir,
            args.tile_size,
            args.backend,
            layer_whitelist=None,
            label="tsy",
            zone_overlays_path=args.tsy_zone_overlays,
            zone_filter=zone_filter,
        )


if __name__ == "__main__":
    main()
