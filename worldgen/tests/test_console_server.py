"""worldgen-v4 P1 — console_server.py dev API contract pins (§8.1 #7).

These lock the *observable HTTP behavior* of the dev-only 3D preview backend so
the vite + three.js viewer (and any future consumer) can rely on:

  * GET  /api/manifest        — manifest v2 shape: version, tile_size,
                                spans_encoding (constants the viewer needs to
                                decode spans.bin), tiles[], surface_palette.
  * GET  /api/tile/{x}/{z}/{layer}
                                — raw little-endian binaries with the exact byte
                                lengths the P0 span layout dictates
                                (spans_count = cols, spans = cols * 16) plus the
                                semantic float32 layers; correct content-type +
                                X-Span-Stride header; absent layer → 404; unknown
                                layer name → 400; path-traversal → rejected.
  * POST /api/regen           — REAL incremental regeneration: the zone's tiles
                                are re-baked in place (spans.bin mtime bumps),
                                unaffected tiles are left untouched, the manifest
                                stays complete, and the rewritten tiles are
                                byte-identical to a full bake (seam-free).
                                Unknown zone → 400.

The fixtures build a *tiny* world (128-block tiles, two zones) so a full bake
plus an incremental regen run in a few seconds — a full ±10000 world bake is
minutes-long and unsuitable for CI.
"""

from __future__ import annotations

import json
import os
import struct
import time
from pathlib import Path

import numpy as np
import pytest

# fastapi/httpx are dev-only (requirements-dev.txt). Skip cleanly if a numpy-only
# environment (CI raster pipeline) imports this file.
pytest.importorskip("fastapi")
pytest.importorskip("httpx")

from fastapi.testclient import TestClient  # noqa: E402

from scripts.terrain_gen.bakers.raster_export import (  # noqa: E402
    SPANS_COUNT_FILE,
    SPANS_FILE,
    build_raster_bake_plan,
    export_rasters,
)
from scripts.terrain_gen.blueprint import (  # noqa: E402
    load_blueprint,
    load_profile_catalog,
    load_zone_overlays,
)
from scripts.terrain_gen.console_server import create_app  # noqa: E402
from scripts.terrain_gen.fields import (  # noqa: E402
    LAYER_REGISTRY,
    MAX_SPANS,
    SPAN_BYTES_PER_COLUMN,
    SPAN_SENTINEL,
)
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields  # noqa: E402

TILE_SIZE = 128
PROFILES_PATH = Path(__file__).resolve().parents[1] / "terrain-profiles.example.json"

# Two zones so regen has a real "affected vs untouched" split. spawn_plain and
# broken_peaks are both fully-implemented profiles in the example catalog.
_TINY_BLUEPRINT = {
    "version": 1,
    "world": {
        "name": "console_test_world",
        "spawn_zone": "spawn",
        "bounds_xz": {"min": [-256, -256], "max": [255, 255]},
        "notes": [],
    },
    "zones": [
        {
            "name": "spawn",
            "display_name": "Tiny Spawn",
            "aabb": {"min": [-200, -64, -200], "max": [0, 320, 0]},
            "center_xz": [-100, -100],
            "size_xz": [200, 200],
            "spirit_qi": 0.3,
            "danger_level": 1,
            "worldgen": {
                "terrain_profile": "spawn_plain",
                "shape": "ellipse",
                "boundary": {"mode": "soft", "width": 32},
                "height_model": {"base": [66, 78], "peak": 84},
            },
            "pois": [],
        },
        {
            "name": "peaks",
            "display_name": "Tiny Peaks",
            "aabb": {"min": [16, -64, 16], "max": [200, 320, 200]},
            "center_xz": [108, 108],
            "size_xz": [184, 184],
            "spirit_qi": 0.5,
            "danger_level": 2,
            "worldgen": {
                "terrain_profile": "broken_peaks",
                "shape": "ellipse",
                "boundary": {"mode": "soft", "width": 32},
                "height_model": {"base": [90, 120], "peak": 140},
            },
            "pois": [],
        },
    ],
    "paths": [],
}

_OVERWORLD_WHITELIST = set(LAYER_REGISTRY.keys()) - {
    "tsy_presence",
    "tsy_origin_id",
    "tsy_depth_tier",
}


def _full_bake(output_dir: Path, blueprint_path: Path) -> None:
    blueprint = load_blueprint(blueprint_path)
    catalog = load_profile_catalog(PROFILES_PATH)
    plan = build_generation_plan(
        blueprint=blueprint,
        profile_catalog=catalog,
        blueprint_path=blueprint_path,
        profiles_path=PROFILES_PATH,
        output_dir=output_dir,
        tile_size=TILE_SIZE,
        zone_overlays=load_zone_overlays(None),
    )
    plan.bake_plan = build_raster_bake_plan(plan, output_dir)
    fields = synthesize_fields(plan)
    export_rasters(plan, fields, layer_whitelist=_OVERWORLD_WHITELIST)


@pytest.fixture(scope="module")
def baked_world(tmp_path_factory) -> dict:
    """Write the tiny blueprint + run one full raster bake; return paths."""
    root = tmp_path_factory.mktemp("console_world")
    blueprint_path = root / "blueprint.json"
    blueprint_path.write_text(json.dumps(_TINY_BLUEPRINT), encoding="utf-8")
    output_dir = root / "out"
    _full_bake(output_dir, blueprint_path)
    return {
        "root": root,
        "blueprint_path": blueprint_path,
        "output_dir": output_dir,
        "rasters_dir": output_dir / "rasters",
    }


@pytest.fixture
def client(baked_world) -> TestClient:
    app = create_app(
        baked_world["rasters_dir"],
        blueprint_path=baked_world["blueprint_path"],
        profiles_path=PROFILES_PATH,
        tile_size=TILE_SIZE,
        output_dir=baked_world["output_dir"],
    )
    return TestClient(app)


def _first_solid_tile(manifest: dict) -> tuple[int, int]:
    """Pick a tile that intersects the spawn zone center (guaranteed solid)."""
    for tile in manifest["tiles"]:
        if "spawn" in tile["zones"]:
            return tile["tile_x"], tile["tile_z"]
    # Fallback: any tile.
    return manifest["tiles"][0]["tile_x"], manifest["tiles"][0]["tile_z"]


# ---------------------------------------------------------------------------
# GET /api/manifest
# ---------------------------------------------------------------------------


def test_manifest_returns_v2_shape(client: TestClient) -> None:
    resp = client.get("/api/manifest")
    assert resp.status_code == 200, f"manifest must serve 200, got {resp.status_code}"
    m = resp.json()
    assert m["version"] == 2, "console only speaks manifest v2 (P0 spans); got " + str(
        m["version"]
    )
    assert m["tile_size"] == TILE_SIZE
    assert m["backend"] == "raster"
    assert isinstance(m["surface_palette"], list) and m["surface_palette"], (
        "surface_palette must be a non-empty list — the viewer maps surface_id "
        "indices into it for vertex colors"
    )
    assert isinstance(m["tiles"], list) and m["tiles"], "tiles[] must be populated"


def test_manifest_carries_spans_encoding_constants(client: TestClient) -> None:
    enc = client.get("/api/manifest").json()["spans_encoding"]
    # The viewer needs these exact constants to mmap-decode spans.bin client-side.
    assert enc["max_spans"] == MAX_SPANS, (
        f"viewer reads {MAX_SPANS} slots/col; manifest says {enc['max_spans']}"
    )
    assert enc["bytes_per_column"] == SPAN_BYTES_PER_COLUMN, (
        "stride mismatch would offset every column read"
    )
    assert enc["sentinel"] == SPAN_SENTINEL
    assert enc["count_file"] == SPANS_COUNT_FILE
    assert enc["spans_file"] == SPANS_FILE


def test_manifest_tile_entries_declare_spans(client: TestClient) -> None:
    m = client.get("/api/manifest").json()
    for tile in m["tiles"]:
        assert tile["spans"] is True, (
            f"tile {tile['dir']} must declare spans=True (every tile carries the "
            "two span files in P0)"
        )
        assert {"tile_x", "tile_z", "dir", "zones", "layers"} <= set(tile.keys())


def test_manifest_exposes_per_zone_params_with_profile(client: TestClient) -> None:
    """manifest.zones carries the editable blueprint subset per zone, including
    terrain_profile (so the console swatch can color by profile, not magenta)."""
    m = client.get("/api/manifest").json()
    zones = m["zones"]
    assert isinstance(zones, list) and zones, "manifest.zones must be a non-empty list"
    by_name = {z["name"]: z for z in zones}
    assert {"spawn", "peaks"} <= set(by_name), (
        f"both tiny-world zones must appear in manifest.zones; got {sorted(by_name)}"
    )
    spawn = by_name["spawn"]
    peaks = by_name["peaks"]
    # terrain_profile is surfaced top-level for the swatch + inside worldgen.
    assert spawn["terrain_profile"] == "spawn_plain", (
        f"spawn must carry its profile for swatch coloring; got {spawn.get('terrain_profile')}"
    )
    assert peaks["terrain_profile"] == "broken_peaks"
    assert spawn["terrain_profile"] != peaks["terrain_profile"], (
        "distinct zones must have distinguishable profiles (else all-magenta swatch bug)"
    )
    # Editable numeric fields the param panel POSTs back as overrides.
    assert spawn["spirit_qi"] == 0.3 and peaks["spirit_qi"] == 0.5
    assert spawn["worldgen"]["terrain_profile"] == "spawn_plain"
    assert {"spirit_qi", "danger_level", "display_name", "worldgen"} <= set(spawn)


# ---------------------------------------------------------------------------
# GET /api/tile/{x}/{z}/{layer}
# ---------------------------------------------------------------------------


def test_tile_spans_count_length_is_one_byte_per_column(client: TestClient) -> None:
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    resp = client.get(f"/api/tile/{x}/{z}/{SPANS_COUNT_FILE}")
    assert resp.status_code == 200
    assert resp.headers["content-type"] == "application/octet-stream"
    expected = TILE_SIZE * TILE_SIZE  # 1 byte/col
    assert len(resp.content) == expected, (
        f"spans_count must be {expected} bytes (1 u8/col for {TILE_SIZE}^2 cols), "
        f"got {len(resp.content)}"
    )


def test_tile_spans_length_is_stride_per_column(client: TestClient) -> None:
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    resp = client.get(f"/api/tile/{x}/{z}/{SPANS_FILE}")
    assert resp.status_code == 200
    expected = TILE_SIZE * TILE_SIZE * SPAN_BYTES_PER_COLUMN
    assert len(resp.content) == expected, (
        f"spans.bin must be cols * {SPAN_BYTES_PER_COLUMN} bytes = {expected}, "
        f"got {len(resp.content)}"
    )
    assert resp.headers.get("x-span-stride") == str(SPAN_BYTES_PER_COLUMN), (
        "X-Span-Stride header lets the viewer compute offset = col * stride "
        "without re-reading the manifest"
    )
    assert resp.headers.get("x-tile-size") == str(TILE_SIZE)


def test_tile_spans_offset_decodes_to_valid_surface(client: TestClient) -> None:
    """The P0 offset law: col_idx * 16 indexes a (floor_y, ceiling_y) i16-LE pair.

    Decode the same column from both the count and spans buffers and assert the
    surface span (span[0].ceiling_y) is a real world Y, and that unused slots are
    sentinel-filled — exactly what the face-culled voxel mesher relies on.
    """
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    counts = client.get(f"/api/tile/{x}/{z}/{SPANS_COUNT_FILE}").content
    spans = client.get(f"/api/tile/{x}/{z}/{SPANS_FILE}").content

    counts_arr = np.frombuffer(counts, dtype=np.uint8)
    solid_cols = np.nonzero(counts_arr > 0)[0]
    assert solid_cols.size > 0, "a spawn-zone tile must have at least one solid column"

    col = int(solid_cols[0])
    cnt = int(counts_arr[col])
    assert 1 <= cnt <= MAX_SPANS

    # Decode floor/ceiling of span[0] straight from the byte offset (the law the
    # viewer + Rust reader both implement).
    base = col * SPAN_BYTES_PER_COLUMN
    floor_y, ceiling_y = struct.unpack_from("<hh", spans, base)
    assert -64 <= floor_y <= 431, f"span[0].floor_y={floor_y} out of world Y range"
    assert -64 <= ceiling_y <= 431, f"span[0].ceiling_y={ceiling_y} out of world Y range"
    assert floor_y <= ceiling_y, "floor must be <= ceiling (surface above its base)"

    # Slots beyond the count byte must be sentinel-filled (i16::MAX) so the
    # decoder stops scanning — guards the "skip trailing sentinels" loop.
    for slot in range(cnt, MAX_SPANS):
        s_floor, s_ceiling = struct.unpack_from("<hh", spans, base + slot * 4)
        assert s_floor == SPAN_SENTINEL and s_ceiling == SPAN_SENTINEL, (
            f"slot {slot} beyond count={cnt} must be sentinel {SPAN_SENTINEL}, "
            f"got ({s_floor}, {s_ceiling})"
        )


def test_tile_semantic_layer_is_float32(client: TestClient) -> None:
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    resp = client.get(f"/api/tile/{x}/{z}/qi_density.bin")
    assert resp.status_code == 200, "qi_density is a semantic layer every tile writes"
    expected = TILE_SIZE * TILE_SIZE * 4  # float32
    assert len(resp.content) == expected, (
        f"qi_density.bin must be cols * 4 bytes = {expected}, got {len(resp.content)}"
    )
    # All values clamp to [0, 1] (heatmap input).
    vals = np.frombuffer(resp.content, dtype="<f4")
    assert float(vals.min()) >= -1e-6 and float(vals.max()) <= 1.0 + 1e-6


def test_tile_layer_name_without_suffix_resolves(client: TestClient) -> None:
    """Both 'qi_density' and 'qi_density.bin' must address the same file."""
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    a = client.get(f"/api/tile/{x}/{z}/qi_density")
    b = client.get(f"/api/tile/{x}/{z}/qi_density.bin")
    assert a.status_code == 200 and b.status_code == 200
    assert a.content == b.content


def test_tile_absent_layer_returns_404(client: TestClient) -> None:
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    # tsy_presence is a registered layer but never written for an overworld bake.
    resp = client.get(f"/api/tile/{x}/{z}/tsy_presence.bin")
    assert resp.status_code == 404, (
        "a registered-but-unwritten layer must 404 (the file genuinely isn't "
        "there) so the viewer falls back to safe_default"
    )


def test_tile_unknown_layer_returns_400(client: TestClient) -> None:
    m = client.get("/api/manifest").json()
    x, z = _first_solid_tile(m)
    resp = client.get(f"/api/tile/{x}/{z}/not_a_real_layer")
    assert resp.status_code == 400, (
        "an unregistered layer name must 400 (bad request) — never let the URL "
        "address an arbitrary file"
    )


def test_tile_path_traversal_is_rejected(client: TestClient) -> None:
    # A traversal attempt in the layer slug must not escape the rasters dir.
    resp = client.get("/api/tile/0/0/..%2f..%2fmanifest.json")
    assert resp.status_code in (400, 404), (
        "path traversal in the layer slug must be refused, never serve files "
        "outside the tile dir"
    )


def test_missing_tile_dir_returns_404(client: TestClient) -> None:
    # Coordinates far outside the baked world: tile dir does not exist.
    resp = client.get(f"/api/tile/999/999/{SPANS_FILE}")
    assert resp.status_code == 404


# ---------------------------------------------------------------------------
# POST /api/regen
# ---------------------------------------------------------------------------


def _tile_sha(path: Path) -> dict[str, bytes]:
    return {f.name: f.read_bytes() for f in sorted(path.iterdir()) if f.is_file()}


def test_regen_rewrites_only_affected_tiles(client: TestClient, baked_world) -> None:
    rasters = baked_world["rasters_dir"]
    manifest_before = client.get("/api/manifest").json()
    tiles_before = {t["dir"] for t in manifest_before["tiles"]}

    # An affected tile (intersects peaks) and an unaffected one (spawn-only).
    affected_id = None
    untouched_id = None
    for tile in manifest_before["tiles"]:
        if "peaks" in tile["zones"]:
            affected_id = affected_id or tile["dir"]
        elif "peaks" not in tile["zones"] and "spawn" in tile["zones"]:
            untouched_id = untouched_id or tile["dir"]
    assert affected_id and untouched_id, (
        "tiny world must yield both a peaks-touching tile and a spawn-only tile"
    )

    affected_spans = rasters / affected_id / SPANS_FILE
    untouched_spans = rasters / untouched_id / SPANS_FILE
    mtime_affected_before = os.path.getmtime(affected_spans)
    mtime_untouched_before = os.path.getmtime(untouched_spans)
    # Filesystem mtime resolution can be coarse; nudge past it.
    time.sleep(1.05)

    resp = client.post("/api/regen", json={"zone_name": "peaks"})
    assert resp.status_code == 200, f"regen must succeed, got {resp.status_code}"
    body = resp.json()
    assert body["zone_name"] == "peaks"
    assert affected_id in body["rewritten_tiles"], (
        "the peaks-touching tile must be in the rewritten set"
    )
    assert body["tile_count"] == len(body["rewritten_tiles"])

    # Real regen: the affected tile's spans.bin is freshly written.
    assert os.path.getmtime(affected_spans) > mtime_affected_before, (
        "regen must truly rewrite the affected tile (mtime bumps), not stub"
    )
    # The unaffected tile is left exactly as-is (no rmtree-then-rewrite).
    assert os.path.getmtime(untouched_spans) == mtime_untouched_before, (
        "a spawn-only tile must NOT be touched by a peaks regen"
    )

    # Manifest stays complete after the incremental rewrite.
    manifest_after = client.get("/api/manifest").json()
    tiles_after = {t["dir"] for t in manifest_after["tiles"]}
    assert tiles_after == tiles_before, (
        "incremental regen must not drop or add tiles in the manifest"
    )


def test_regen_unknown_zone_returns_400(client: TestClient) -> None:
    resp = client.post("/api/regen", json={"zone_name": "no_such_zone"})
    assert resp.status_code == 400, "unknown zone name must be a 400, not a 500"
    assert "no_such_zone" in resp.json()["detail"]


def test_regen_missing_zone_name_returns_422(client: TestClient) -> None:
    # The pydantic body model requires zone_name; omitting it is a request schema
    # error → FastAPI 422 (validation), never a 500.
    resp = client.post("/api/regen", json={})
    assert resp.status_code == 422, (
        "POST /api/regen without zone_name must be a 422 (missing required field), "
        f"got {resp.status_code}"
    )


# ---------------------------------------------------------------------------
# No-bake-yet: manifest.json absent (first run, before any pipeline bake)
# ---------------------------------------------------------------------------


@pytest.fixture
def unbaked_client(tmp_path) -> TestClient:
    """A console app pointed at an empty rasters dir (no manifest.json yet)."""
    blueprint_path = tmp_path / "bp.json"
    blueprint_path.write_text(json.dumps(_TINY_BLUEPRINT), encoding="utf-8")
    output_dir = tmp_path / "out"
    rasters_dir = output_dir / "rasters"
    rasters_dir.mkdir(parents=True)  # exists but holds no manifest.json
    app = create_app(
        rasters_dir,
        blueprint_path=blueprint_path,
        profiles_path=PROFILES_PATH,
        tile_size=TILE_SIZE,
        output_dir=output_dir,
    )
    return TestClient(app)


def test_manifest_missing_returns_503(unbaked_client: TestClient) -> None:
    resp = unbaked_client.get("/api/manifest")
    assert resp.status_code == 503, (
        "GET /api/manifest before any bake must be a 503 (run pipeline first), "
        f"not a 500/404; got {resp.status_code}"
    )
    assert "manifest not found" in resp.json()["detail"]


def test_regen_before_any_bake_returns_503(unbaked_client: TestClient) -> None:
    # regen_zone raises FileNotFoundError when manifest.json is absent (no prior
    # full bake). post_regen must translate that into a 503, never an uncaught
    # 500 traceback.
    resp = unbaked_client.post("/api/regen", json={"zone_name": "peaks"})
    assert resp.status_code == 503, (
        "POST /api/regen before any full bake must be a 503 (manifest missing), "
        f"not a 500 traceback; got {resp.status_code}"
    )
    detail = resp.json()["detail"]
    assert "no baked world yet" in detail or "manifest" in detail, (
        f"503 detail must point the operator at running the pipeline; got: {detail}"
    )


# ---------------------------------------------------------------------------
# POST /api/regen — live blueprint parameter overrides
# ---------------------------------------------------------------------------


def _peaks_qi_density_after_regen(
    tmp_path: Path,
    overrides: dict | None,
) -> dict[str, bytes]:
    """Full-bake the tiny world, regen 'peaks' (optionally with overrides) via
    the API, and return {tile_id: qi_density.bin bytes} for every peaks tile.

    Used to prove an override actually changes the generated output: two runs
    differing only in ``overrides`` must produce different qi_density bytes.
    """
    tmp_path.mkdir(parents=True, exist_ok=True)
    blueprint_path = tmp_path / "bp.json"
    blueprint_path.write_text(json.dumps(_TINY_BLUEPRINT), encoding="utf-8")
    out_dir = tmp_path / "out"
    _full_bake(out_dir, blueprint_path)
    rasters = out_dir / "rasters"

    app = create_app(
        rasters,
        blueprint_path=blueprint_path,
        profiles_path=PROFILES_PATH,
        tile_size=TILE_SIZE,
        output_dir=out_dir,
    )
    c = TestClient(app)
    body: dict = {"zone_name": "peaks"}
    if overrides is not None:
        body["overrides"] = overrides
    resp = c.post("/api/regen", json=body)
    assert resp.status_code == 200, (
        f"regen must succeed, got {resp.status_code}: {resp.text}"
    )
    rewritten = resp.json()["rewritten_tiles"]
    assert rewritten, "regen must rewrite at least one peaks tile"
    return {
        tile_id: (rasters / tile_id / "qi_density.bin").read_bytes()
        for tile_id in rewritten
        if (rasters / tile_id / "qi_density.bin").exists()
    }


def test_regen_overrides_change_generated_output(tmp_path) -> None:
    """An override on spirit_qi must produce DIFFERENT tiles than no override.

    qi_density scales by (0.5 + spirit_qi) in every profile, so bumping
    spirit_qi from 0.5 → 0.95 must change qi_density.bin bytes for the regen'd
    peaks tiles. This is the end-to-end proof that the param panel is live, not
    a placeholder that the server ignores.
    """
    baseline = _peaks_qi_density_after_regen(tmp_path / "base", overrides=None)
    bumped = _peaks_qi_density_after_regen(
        tmp_path / "bump", overrides={"spirit_qi": 0.95}
    )

    assert set(baseline) == set(bumped), (
        "the same peaks tiles must be rewritten regardless of override value"
    )
    assert baseline, "must have at least one qi_density tile to compare"
    differing = [tid for tid in baseline if baseline[tid] != bumped[tid]]
    assert differing, (
        "raising spirit_qi must change qi_density.bin for at least one peaks "
        "tile — if bytes are identical the override was silently ignored "
        "(zombie param panel)"
    )


def test_regen_overrides_do_not_rewrite_disk_blueprint(tmp_path) -> None:
    """Overrides are in-memory only: the on-disk blueprint source is untouched."""
    blueprint_path = tmp_path / "bp.json"
    original_text = json.dumps(_TINY_BLUEPRINT)
    blueprint_path.write_text(original_text, encoding="utf-8")
    out_dir = tmp_path / "out"
    _full_bake(out_dir, blueprint_path)

    app = create_app(
        out_dir / "rasters",
        blueprint_path=blueprint_path,
        profiles_path=PROFILES_PATH,
        tile_size=TILE_SIZE,
        output_dir=out_dir,
    )
    c = TestClient(app)
    resp = c.post(
        "/api/regen", json={"zone_name": "peaks", "overrides": {"spirit_qi": 0.95}}
    )
    assert resp.status_code == 200, resp.text

    # The source blueprint file must be byte-identical: overrides never persist.
    on_disk = json.loads(blueprint_path.read_text(encoding="utf-8"))
    peaks = next(z for z in on_disk["zones"] if z["name"] == "peaks")
    assert peaks["spirit_qi"] == 0.5, (
        "disk blueprint spirit_qi must remain 0.5 — overrides are in-memory only, "
        f"got {peaks['spirit_qi']}"
    )


def test_regen_worldgen_override_changes_terrain(tmp_path) -> None:
    """A nested worldgen override (terrain_profile swap) re-bakes with the new
    profile, changing the span bytes — proves worldgen sub-dict merges live."""
    blueprint_path = tmp_path / "bp.json"
    blueprint_path.write_text(json.dumps(_TINY_BLUEPRINT), encoding="utf-8")
    out_dir = tmp_path / "out"
    _full_bake(out_dir, blueprint_path)
    rasters = out_dir / "rasters"

    app = create_app(
        rasters,
        blueprint_path=blueprint_path,
        profiles_path=PROFILES_PATH,
        tile_size=TILE_SIZE,
        output_dir=out_dir,
    )
    c = TestClient(app)
    # Capture the affected tiles' spans before the override regen.
    affected = c.post("/api/regen", json={"zone_name": "peaks"}).json()[
        "rewritten_tiles"
    ]
    before = {
        tid: (rasters / tid / SPANS_FILE).read_bytes() for tid in affected
    }
    # Now override the peaks profile to spawn_plain (much lower terrain).
    resp = c.post(
        "/api/regen",
        json={
            "zone_name": "peaks",
            "overrides": {"worldgen": {"terrain_profile": "spawn_plain"}},
        },
    )
    assert resp.status_code == 200, resp.text
    after = {tid: (rasters / tid / SPANS_FILE).read_bytes() for tid in affected}
    differing = [t for t in before if before[t] != after[t]]
    assert differing, (
        "swapping terrain_profile via a worldgen override must change spans.bin "
        "for the affected tiles — worldgen sub-dict merge is live"
    )


def test_regen_unknown_override_field_returns_400(client: TestClient) -> None:
    resp = client.post(
        "/api/regen",
        json={"zone_name": "peaks", "overrides": {"not_a_field": 1}},
    )
    assert resp.status_code == 400, (
        "an unknown override field must be a 400 (bad request), never silently "
        f"ignored or a 500; got {resp.status_code}"
    )
    assert "not_a_field" in resp.json()["detail"]


def test_regen_typed_override_error_returns_400(client: TestClient) -> None:
    # spirit_qi must be a float; a string makes parse_blueprint raise → 400.
    resp = client.post(
        "/api/regen",
        json={"zone_name": "peaks", "overrides": {"spirit_qi": "very high"}},
    )
    assert resp.status_code == 400, (
        "a type-invalid override (spirit_qi as a non-numeric string) must be a "
        f"400, not a 500 traceback; got {resp.status_code}"
    )


def test_regen_empty_overrides_behaves_like_no_overrides(tmp_path) -> None:
    """An empty/absent overrides object must NOT change the bake (falsy → skip)."""
    none_run = _peaks_qi_density_after_regen(tmp_path / "none", overrides=None)
    empty_run = _peaks_qi_density_after_regen(tmp_path / "empty", overrides={})
    assert none_run == empty_run, (
        "an empty overrides dict must produce byte-identical tiles to no "
        "overrides at all (it is a no-op, not an error)"
    )


def test_regen_is_byte_identical_to_full_bake(tmp_path) -> None:
    """Incremental regen of a zone must produce the same bytes as a full bake.

    This is the seam-free guarantee: re-baking only 'peaks' tiles (which still
    blend in their neighbor 'spawn') yields tiles byte-identical to a full run,
    so no visible stitching seam appears in the viewer after a regen.
    """
    blueprint_path = tmp_path / "bp.json"
    blueprint_path.write_text(json.dumps(_TINY_BLUEPRINT), encoding="utf-8")

    # Reference: a clean full bake.
    full_dir = tmp_path / "full"
    _full_bake(full_dir, blueprint_path)

    # Subject: full bake, then incremental regen of 'peaks' over it via the API.
    inc_dir = tmp_path / "inc"
    _full_bake(inc_dir, blueprint_path)
    app = create_app(
        inc_dir / "rasters",
        blueprint_path=blueprint_path,
        profiles_path=PROFILES_PATH,
        tile_size=TILE_SIZE,
        output_dir=inc_dir,
    )
    c = TestClient(app)
    assert c.post("/api/regen", json={"zone_name": "peaks"}).status_code == 200

    full_tiles = {p.name for p in (full_dir / "rasters").iterdir() if p.is_dir()}
    inc_tiles = {p.name for p in (inc_dir / "rasters").iterdir() if p.is_dir()}
    assert full_tiles == inc_tiles, "tile set must match the full bake"

    for tile_id in full_tiles:
        full_files = _tile_sha(full_dir / "rasters" / tile_id)
        inc_files = _tile_sha(inc_dir / "rasters" / tile_id)
        assert full_files == inc_files, (
            f"tile {tile_id} differs after incremental regen — a seam would show"
        )
