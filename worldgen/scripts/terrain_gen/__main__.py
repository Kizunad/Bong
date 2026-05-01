from __future__ import annotations

import argparse
from pathlib import Path
from typing import Optional

from .blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    WORLDGEN_ROOT,
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
from .stitcher import build_generation_plan, synthesize_fields

DEFAULT_OUTPUT_DIR = WORLDGEN_ROOT / "generated" / "terrain-gen"
DEFAULT_TSY_OUTPUT_DIR = WORLDGEN_ROOT / "generated" / "terrain-gen-tsy"

# plan-tsy-worldgen-v1 §4.1 — 主世界 manifest 不写 TSY 专用 layer。
TSY_ONLY_LAYERS = frozenset({"tsy_presence", "tsy_origin_id", "tsy_depth_tier"})


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
    return parser.parse_args()


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
    field_set = synthesize_fields(plan)
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


def main() -> None:
    args = parse_args()
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
        )


if __name__ == "__main__":
    main()
