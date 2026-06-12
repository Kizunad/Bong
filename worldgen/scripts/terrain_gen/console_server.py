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
    parse_blueprint,
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


# Zone blueprint fields the console may override on a regen. These map 1:1 onto
# the on-disk blueprint zone entry (server/zones.worldview.example.json). Editing
# anything outside this set is rejected (400) so a typo / unknown field never
# silently no-ops — the param panel is a *live* control, not a scratch pad.
_OVERRIDABLE_ZONE_FIELDS = frozenset(
    {
        "spirit_qi",
        "danger_level",
        "display_name",
        "worldgen",
    }
)


class RegenRequest(BaseModel):
    """POST /api/regen body.

    ``overrides`` is an optional partial zone-blueprint patch. When non-empty it
    is merged onto the named zone's on-disk blueprint entry (in memory only — the
    source file is never rewritten) and the zone is re-baked from the override
    blueprint, so editing e.g. ``spirit_qi`` truly changes the generated tiles.
    """

    zone_name: str
    overrides: Optional[dict[str, Any]] = None


class OverrideError(Exception):
    """Raised when ``overrides`` names an unknown field or fails type parsing.

    Translated to an HTTP 400 by ``post_regen`` (bad request, not a 500): the
    operator edited the param panel into an invalid blueprint.
    """


def _deep_merge(base: dict[str, Any], patch: dict[str, Any]) -> dict[str, Any]:
    """Recursively merge ``patch`` into a copy of ``base``.

    Nested dicts are merged key-by-key (so a partial sub-section keeps the
    sibling keys it didn't mention); every other value type — including lists —
    replaces the base value wholesale. The inputs are never mutated.

    A *shallow* ``dict.update`` here would drop un-submitted nested keys: e.g.
    posting ``{"worldgen": {"height_model": {"amplitude": 12}}}`` would replace
    the entire ``height_model`` sub-dict (losing ``base``, ``octaves``, …) and
    silently bake garbage. Deep-merge keeps the rest of ``height_model``.
    """
    merged = dict(base)
    for key, value in patch.items():
        existing = merged.get(key)
        if isinstance(existing, dict) and isinstance(value, dict):
            merged[key] = _deep_merge(existing, value)
        else:
            merged[key] = value
    return merged


def _apply_zone_overrides(
    blueprint_raw: dict[str, Any],
    zone_name: str,
    overrides: dict[str, Any],
) -> dict[str, Any]:
    """Return a deep copy of ``blueprint_raw`` with ``overrides`` merged into the
    ``zone_name`` zone entry.

    Only keys in ``_OVERRIDABLE_ZONE_FIELDS`` are accepted at the zone top level;
    ``worldgen`` (itself a dict) is deep-merged so a partial worldgen patch (e.g.
    just ``terrain_profile`` or a single ``height_model`` knob) keeps every
    sibling key it didn't mention. Unknown keys raise ``OverrideError`` (→ 400).
    The source dict is never mutated.
    """
    import copy

    if not isinstance(overrides, dict):
        raise OverrideError(
            f"overrides must be a JSON object, got {type(overrides).__name__}"
        )
    unknown = set(overrides) - _OVERRIDABLE_ZONE_FIELDS
    if unknown:
        raise OverrideError(
            f"unknown override field(s) {sorted(unknown)}; "
            f"editable: {sorted(_OVERRIDABLE_ZONE_FIELDS)}"
        )

    merged = copy.deepcopy(blueprint_raw)
    zone_entry = None
    for entry in merged.get("zones", []):
        if entry.get("name") == zone_name:
            zone_entry = entry
            break
    if zone_entry is None:
        # Should be unreachable (caller validates the zone exists first), but
        # keep it a 400 rather than an AttributeError.
        raise OverrideError(f"zone '{zone_name}' not found in blueprint")

    for key, value in overrides.items():
        if key == "worldgen" and isinstance(value, dict):
            base_wg = zone_entry.get("worldgen", {})
            base_wg = base_wg if isinstance(base_wg, dict) else {}
            zone_entry["worldgen"] = _deep_merge(base_wg, value)
        else:
            zone_entry[key] = value
    return merged


def _load_blueprint_with_overrides(
    blueprint_path: Path,
    zone_name: str,
    overrides: Optional[dict[str, Any]],
):
    """Load the blueprint and, when ``overrides`` is non-empty, merge them onto
    the named zone in memory.

    Returns the parsed ``WorldBlueprint``. Type errors from re-parsing an
    overridden zone (e.g. ``spirit_qi: "high"``) surface as ``OverrideError`` so
    the caller can answer 400 instead of leaking a 500.
    """
    if not overrides:
        return load_blueprint(blueprint_path)

    with blueprint_path.open(encoding="utf-8") as handle:
        blueprint_raw = json.load(handle)
    merged_raw = _apply_zone_overrides(blueprint_raw, zone_name, overrides)
    try:
        return parse_blueprint(merged_raw)
    except (ValueError, TypeError, KeyError) as exc:
        raise OverrideError(
            f"overrides produced an invalid blueprint zone: {exc}"
        ) from exc


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
    zone_overlays_path: Optional[Path] = None,
) -> FastAPI:
    """Build the FastAPI app bound to a concrete rasters directory.

    ``rasters_dir``        — directory holding manifest.json + tile_*/ subdirs.
    ``output_dir``         — the terrain_gen ``--output-dir`` whose ``rasters``
                             child equals ``rasters_dir`` (defaults to its
                             parent), used to rebuild the bake plan for
                             /api/regen.
    ``overworld_only``     — drop TSY-only layers from the regen whitelist
                             (matches the main pipeline's overworld pass).
    ``zone_overlays_path`` — same ``zones_export_v1`` JSON the full pipeline
                             takes via ``--zone-overlays`` (persisted server
                             zone_overlays, e.g. collapse records). Loaded once
                             and reused by every /api/regen so an incremental
                             re-bake honors the SAME overlays the full bake did.
                             ``None`` (default) → no overlays, matching the
                             pipeline's own default.
    """
    rasters_dir = rasters_dir.resolve()
    if output_dir is None:
        output_dir = rasters_dir.parent

    # Load the persisted zone_overlays ONCE, the same way the full pipeline does
    # (terrain_gen --zone-overlays). Reusing them on every regen keeps an
    # incremental re-bake byte-consistent with a full bake that used the same
    # overlays; passing None here (the old behavior) silently dropped collapse /
    # overlay records on regen.
    zone_overlays = load_zone_overlays(zone_overlays_path)

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

        ``req.overrides`` (optional) patches the named zone's blueprint entry in
        memory before baking, so editing parameters in the console truly changes
        the generated tiles. The on-disk blueprint source is never rewritten.
        """
        # Validate the zone name against the *unmodified* blueprint first so an
        # unknown zone is a clean 400 before we try to apply overrides to it.
        base_blueprint = load_blueprint(blueprint_path)
        zone_names = {zone.name for zone in base_blueprint.zones}
        if req.zone_name not in zone_names:
            raise HTTPException(
                status_code=400,
                detail=(
                    f"unknown zone '{req.zone_name}'; "
                    f"known zones: {sorted(zone_names)}"
                ),
            )

        try:
            blueprint = _load_blueprint_with_overrides(
                blueprint_path, req.zone_name, req.overrides
            )
        except OverrideError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc

        profile_catalog = load_profile_catalog(profiles_path)
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
        try:
            rewritten = regen_zone(
                plan,
                fields,
                req.zone_name,
                layer_whitelist=regen_whitelist,
            )
        except FileNotFoundError as exc:
            # No prior full bake → manifest.json is absent. Incremental regen has
            # nothing to patch; the operator must run the pipeline once first.
            # Surface this as 503 (service not yet ready), never a 500 traceback.
            raise HTTPException(
                status_code=503,
                detail=(
                    "no baked world yet — run "
                    "`python -m scripts.terrain_gen --backend raster` once before "
                    f"regenerating a zone ({exc})"
                ),
            ) from exc
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
    parser.add_argument(
        "--zone-overlays",
        type=Path,
        default=None,
        help=(
            "Optional zones_export_v1 JSON carrying persisted zone_overlays "
            "(collapse records etc.); same file the full pipeline takes via "
            "--zone-overlays. Regen honors these so it matches the full bake."
        ),
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
        zone_overlays_path=args.zone_overlays,
    )
    print(
        f"[console] serving rasters from {args.rasters.resolve()} "
        f"on http://{args.host}:{args.port}  (dev-only, no auth)"
    )
    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
