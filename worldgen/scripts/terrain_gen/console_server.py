"""Dev-only FastAPI console server for the worldgen-v4 3D preview (§8.1 #7).

This is a **localhost-only, unauthenticated** developer tool.  It serves the
raster manifest + per-tile span/semantic binaries to the vite + three.js
viewer in ``worldgen/console/`` and triggers per-zone incremental regenerations.

It is *not* part of the production chunk-synthesis path: the Rust server reads
the same rasters directly via mmap (see ``server/src/world/terrain/raster.rs``);
this server only exists so a human can fly around the generated world in a
browser and re-bake a single zone after editing blueprint parameters.

Run (dev):

    cd worldgen
    .venv/bin/python -m scripts.terrain_gen.console_server \
        --rasters generated/terrain-gen/rasters

Dependencies (fastapi + uvicorn) live in ``worldgen/requirements-dev.txt`` and
are deliberately kept out of the production / CI numpy-only environment.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Optional

from fastapi import FastAPI, HTTPException, Response
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

from .blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    load_blueprint,
    load_profile_catalog,
    load_zone_overlays,
)
from .bakers.raster_export import (
    SPANS_COUNT_FILE,
    SPANS_FILE,
    build_raster_bake_plan,
    regen_zone,
)
from .fields import LAYER_REGISTRY, SPAN_BYTES_PER_COLUMN
from .stitcher import build_generation_plan, synthesize_fields

# The console is served from a vite dev server; allow its default + a couple of
# common alternates.  Dev-only, localhost-only — no credentials, no wildcard
# origins beyond local dev ports.
_DEV_ORIGINS = [
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "http://localhost:4173",  # vite preview
    "http://127.0.0.1:4173",
]

# Concrete binaries the tile endpoint will stream.  spans_* are always present
# (P0 §8.1 #1); semantic / vertical layers are present only when the tile wrote
# them — the endpoint 404s for an absent layer name and serves a safe-default
# zero buffer is *not* done (the viewer queries the manifest first).
_SPAN_FILES = (SPANS_COUNT_FILE, SPANS_FILE)


class RegenRequest(BaseModel):
    """POST /api/regen body."""

    zone_name: str


def _read_manifest(rasters_dir: Path) -> dict[str, Any]:
    manifest_path = rasters_dir / "manifest.json"
    if not manifest_path.exists():
        raise HTTPException(
            status_code=503,
            detail=(
                f"manifest not found at {manifest_path}; run "
                "`python -m scripts.terrain_gen --backend raster` first"
            ),
        )
    with manifest_path.open(encoding="utf-8") as handle:
        return json.load(handle)


def _safe_tile_dir(rasters_dir: Path, x: int, z: int) -> Path:
    """Resolve ``tile_{x}_{z}`` and guard against path escape."""
    tile_id = f"tile_{x}_{z}"
    tile_dir = (rasters_dir / tile_id).resolve()
    # Path-traversal guard: the resolved dir must stay under rasters_dir.
    if rasters_dir.resolve() not in tile_dir.parents and tile_dir != rasters_dir.resolve():
        raise HTTPException(status_code=400, detail="invalid tile coordinates")
    return tile_dir


def create_app(
    rasters_dir: Path,
    *,
    blueprint_path: Path = DEFAULT_BLUEPRINT_PATH,
    profiles_path: Path = DEFAULT_PROFILES_PATH,
    tile_size: int = 512,
    output_dir: Optional[Path] = None,
    overworld_only: bool = True,
) -> FastAPI:
    """Build the FastAPI app bound to a concrete rasters directory.

    ``rasters_dir``     — directory holding manifest.json + tile_*/ subdirs.
    ``output_dir``      — the terrain_gen ``--output-dir`` whose ``rasters``
                          child equals ``rasters_dir`` (defaults to its parent),
                          used to rebuild the bake plan for /api/regen.
    ``overworld_only``  — drop TSY-only layers from the regen whitelist (matches
                          the main pipeline's overworld pass).
    """
    rasters_dir = rasters_dir.resolve()
    if output_dir is None:
        output_dir = rasters_dir.parent

    app = FastAPI(
        title="Bong worldgen-v4 console (dev-only)",
        description="Localhost 3D terrain preview + per-zone regen. Not for prod.",
    )
    app.add_middleware(
        CORSMiddleware,
        allow_origins=_DEV_ORIGINS,
        allow_credentials=False,
        allow_methods=["GET", "POST", "OPTIONS"],
        allow_headers=["*"],
    )

    # plan-tsy-worldgen-v1 §4.1 — overworld manifest drops TSY-only layers.
    tsy_only = frozenset({"tsy_presence", "tsy_origin_id", "tsy_depth_tier"})
    regen_whitelist: Optional[set[str]] = (
        (set(LAYER_REGISTRY.keys()) - tsy_only) if overworld_only else None
    )

    @app.get("/api/manifest")
    def get_manifest() -> dict[str, Any]:
        """Return the raster manifest (v2 with spans_encoding) verbatim."""
        return _read_manifest(rasters_dir)

    @app.get("/api/tile/{x}/{z}/{layer}")
    def get_tile_layer(x: int, z: int, layer: str) -> Response:
        """Stream one tile binary (spans_count / spans / a semantic .bin).

        Content-Type is application/octet-stream; the response carries
        ``X-Span-Stride`` so the viewer can compute ``offset = col_idx * 16``
        for the spans buffer without re-reading the manifest.
        """
        tile_dir = _safe_tile_dir(rasters_dir, x, z)
        # Whitelist the requested artifact: span files verbatim, otherwise a
        # registered layer's ``<layer>.bin``.  Reject anything else so the URL
        # can never name an arbitrary file.
        if layer in _SPAN_FILES:
            file_name = layer
        elif layer.endswith(".bin") and layer[:-4] in LAYER_REGISTRY:
            file_name = layer
        elif layer in LAYER_REGISTRY:
            file_name = f"{layer}.bin"
        else:
            raise HTTPException(status_code=400, detail=f"unknown layer '{layer}'")

        bin_path = tile_dir / file_name
        if not bin_path.exists():
            raise HTTPException(
                status_code=404,
                detail=f"tile_{x}_{z} has no '{file_name}' (absent layer)",
            )
        data = bin_path.read_bytes()
        headers = {
            "Cache-Control": "no-cache",
            "X-Tile-Size": str(tile_size),
            "X-Span-Stride": str(SPAN_BYTES_PER_COLUMN),
        }
        return Response(
            content=data,
            media_type="application/octet-stream",
            headers=headers,
        )

    @app.post("/api/regen")
    def post_regen(req: RegenRequest) -> dict[str, Any]:
        """Re-bake the tiles touched by ``zone_name`` in place (incremental).

        Real regeneration — runs the terrain_gen synthesis for the zone's
        tiles and overwrites their span/semantic binaries + the manifest
        entries, leaving the rest of the world untouched.  Returns the list of
        rewritten tile ids so the viewer knows which tiles to refetch.
        """
        blueprint = load_blueprint(blueprint_path)
        zone_names = {zone.name for zone in blueprint.zones}
        if req.zone_name not in zone_names:
            raise HTTPException(
                status_code=400,
                detail=(
                    f"unknown zone '{req.zone_name}'; "
                    f"known zones: {sorted(zone_names)}"
                ),
            )

        profile_catalog = load_profile_catalog(profiles_path)
        zone_overlays = load_zone_overlays(None)
        plan = build_generation_plan(
            blueprint=blueprint,
            profile_catalog=profile_catalog,
            blueprint_path=blueprint_path,
            profiles_path=profiles_path,
            output_dir=output_dir,
            tile_size=tile_size,
            zone_overlays=zone_overlays,
        )
        plan.bake_plan = build_raster_bake_plan(plan, output_dir)
        fields = synthesize_fields(plan, zone_filter={req.zone_name})
        rewritten = regen_zone(
            plan,
            fields,
            req.zone_name,
            layer_whitelist=regen_whitelist,
        )
        return {
            "zone_name": req.zone_name,
            "rewritten_tiles": rewritten,
            "tile_count": len(rewritten),
        }

    return app


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Dev-only worldgen-v4 console server (FastAPI + uvicorn)"
    )
    parser.add_argument(
        "--rasters",
        type=Path,
        default=Path("generated/terrain-gen/rasters"),
        help="Path to the rasters directory (holds manifest.json + tile_*/)",
    )
    parser.add_argument(
        "--blueprint",
        type=Path,
        default=DEFAULT_BLUEPRINT_PATH,
        help="Blueprint JSON used by POST /api/regen",
    )
    parser.add_argument(
        "--profiles",
        type=Path,
        default=DEFAULT_PROFILES_PATH,
        help="Profile catalog JSON used by POST /api/regen",
    )
    parser.add_argument("--tile-size", type=int, default=512)
    parser.add_argument("--host", default="127.0.0.1", help="bind host (localhost only)")
    parser.add_argument("--port", type=int, default=8765)
    return parser.parse_args()


def main() -> None:
    import uvicorn

    args = parse_args()
    app = create_app(
        args.rasters,
        blueprint_path=args.blueprint,
        profiles_path=args.profiles,
        tile_size=args.tile_size,
    )
    print(
        f"[console] serving rasters from {args.rasters.resolve()} "
        f"on http://{args.host}:{args.port}  (dev-only, no auth)"
    )
    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
