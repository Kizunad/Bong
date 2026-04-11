from __future__ import annotations

import argparse
from pathlib import Path

from .blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    WORLDGEN_ROOT,
    load_blueprint,
    load_profile_catalog,
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
from .stitcher import build_generation_plan, synthesize_fields

DEFAULT_OUTPUT_DIR = WORLDGEN_ROOT / "generated" / "terrain-gen"


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
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    blueprint = load_blueprint(args.blueprint)
    profile_catalog = load_profile_catalog(args.profiles)
    plan = build_generation_plan(
        blueprint=blueprint,
        profile_catalog=profile_catalog,
        blueprint_path=args.blueprint,
        profiles_path=args.profiles,
        output_dir=args.output_dir,
        tile_size=args.tile_size,
    )
    if args.backend == "worldpainter":
        plan.bake_plan = build_worldpainter_bake_plan(plan, args.output_dir)
    else:
        plan.bake_plan = build_raster_bake_plan(plan, args.output_dir)

    plan_path = write_plan_json(plan, args.output_dir)
    field_set = synthesize_fields(plan)
    field_summary_path = write_field_summary_json(field_set, args.output_dir)
    preview_paths = write_preview_images(plan, field_set, args.output_dir)
    bake_artifacts: dict[str, Path] = {}
    if args.backend == "worldpainter":
        bake_artifacts = export_worldpainter_rasters(plan, field_set)
    elif args.backend == "raster":
        bake_artifacts = export_rasters(plan, field_set)
    print(format_summary(plan, plan_path))
    print(format_field_summary(field_set, field_summary_path))
    for label, preview_path in preview_paths.items():
        print(f"  {label}: {preview_path}")
    for label, artifact_path in bake_artifacts.items():
        print(f"  bake_{label}: {artifact_path}")


if __name__ == "__main__":
    main()
