"""worldgen-v4 P0 — raster export spans binary-layout + manifest v2 pin.

End-to-end export of a real profile, asserting the on-disk contract:
  * spans_count.bin (u8/col) + spans.bin (SPAN_BYTES_PER_COLUMN/col) written;
  * height.bin and the six deleted vertical patch layers are NOT written;
  * sky_island_mask + underground_tier remain as semantic rasters;
  * manifest version == 2, carries spans_encoding; vertical_layers lists only the
    semantic vertical rasters a given export actually wrote (gated on disk, not on
    registry membership — Major #3);
  * decoding column 0 from the raw files via the i16-LE/sentinel layout matches
    the surface folded by the shim (offset = col_idx * stride round-trip).
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.bakers.raster_export import (
    SPANS_COUNT_FILE,
    SPANS_FILE,
    build_raster_bake_plan,
    export_rasters,
)
from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    TerrainProfileCatalog,
    TerrainProfileSpec,
    WorldBlueprint,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import (
    SPAN_BYTES_PER_COLUMN,
    SPAN_SENTINEL,
    Bounds2D,
    decode_spans_column,
)
from scripts.terrain_gen.spans_shim import spans_for_tile
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields

TILE_SIZE = 200

DELETED_VERTICAL_LAYERS = (
    "cave_mask",
    "ceiling_height",
    "entrance_mask",
    "sky_island_base_y",
    "sky_island_thickness",
    "cavern_floor_y",
)


def _build_zone(profile: str) -> BlueprintZone:
    return BlueprintZone(
        name=f"{profile}_export",
        display_name=profile,
        bounds_xz=Bounds2D(min_x=-100, max_x=99, min_z=-100, max_z=99),
        center_xz=(0, 0),
        size_xz=(200, 200),
        spirit_qi=0.6,
        danger_level=2,
        worldgen=ZoneWorldgenConfig(
            terrain_profile=profile,
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=24),
            height_model={"base": [60, 80], "peak": 96},
            surface_palette=("grass_block", "stone", "gravel"),
        ),
    )


def _export(profile: str, temp_dir: str):
    zone = _build_zone(profile)
    blueprint = WorldBlueprint(
        version=1,
        world_name="export_world",
        spawn_zone=zone.name,
        bounds_xz=zone.bounds_xz,
        notes=(),
        zones=(zone,),
    )
    catalog = TerrainProfileCatalog(
        version=1,
        profiles={
            profile: TerrainProfileSpec(
                name=profile,
                boundary=BoundarySpec(mode="soft", width=24),
                height={"base": [60, 80], "peak": 96},
                surface=("grass_block", "stone", "gravel"),
                water={"level": "none", "coverage": 0.0},
                passability="medium",
                extras={},
            )
        },
    )
    out = Path(temp_dir)
    plan = build_generation_plan(
        blueprint=blueprint,
        profile_catalog=catalog,
        blueprint_path=Path("b.json"),
        profiles_path=Path("p.json"),
        output_dir=out,
        tile_size=TILE_SIZE,
    )
    plan.bake_plan = build_raster_bake_plan(plan, out)
    fields = synthesize_fields(plan)
    artifacts = export_rasters(plan, fields)
    manifest = json.loads(artifacts["manifest"].read_text(encoding="utf-8"))
    return manifest, artifacts["raster_dir"], fields


class SpansExportLayoutTest(unittest.TestCase):
    def test_manifest_is_version_2_with_spans_encoding(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            manifest, _raster_dir, _fields = _export("sky_isle", td)
        self.assertEqual(manifest["version"], 2)
        enc = manifest["spans_encoding"]
        self.assertEqual(enc["max_spans"], 4)
        self.assertEqual(enc["bytes_per_column"], SPAN_BYTES_PER_COLUMN)
        self.assertEqual(enc["sentinel"], SPAN_SENTINEL)
        self.assertEqual(enc["count_file"], SPANS_COUNT_FILE)
        self.assertEqual(enc["spans_file"], SPANS_FILE)

    def test_vertical_layers_reflect_actually_written_per_profile(self) -> None:
        # worldgen-v4 P0 Major #3: vertical_layers is gated on written_layer_names,
        # so it lists ONLY the semantic vertical layers a given export actually
        # wrote — never every registered name.  The old registry-only gate declared
        # BOTH layers in every single-profile export (phantom layers the Rust
        # reader would fail to mmap).  Pin the corrected per-profile truth:
        #   sky_isle      → only sky_island_mask  (no abyssal tiers ⇒ no underground_tier)
        #   abyssal_maze  → only underground_tier (no isle ⇒ no sky_island_mask)
        #   cave_network  → neither               (uses neither semantic layer)
        expected = {
            "sky_isle": ["sky_island_mask"],
            "abyssal_maze": ["underground_tier"],
            "cave_network": [],
        }
        for profile, want in expected.items():
            with self.subTest(profile=profile):
                with tempfile.TemporaryDirectory() as td:
                    manifest, raster_dir, _fields = _export(profile, td)
                self.assertEqual(
                    manifest["vertical_layers"],
                    want,
                    f"{profile}: vertical_layers must list exactly the semantic "
                    f"vertical rasters this export wrote ({want}); the geometric "
                    f"vertical layers are folded into spans and the unwritten "
                    f"semantic one must NOT be declared",
                )

    def test_span_files_have_exact_byte_lengths(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, _fields = _export("cave_network", td)
            area = TILE_SIZE * TILE_SIZE
            for tile in manifest["tiles"]:
                self.assertTrue(tile.get("spans"), "tile must flag spans=True")
                tile_dir = raster_dir / tile["dir"]
                count_bytes = (tile_dir / SPANS_COUNT_FILE).read_bytes()
                spans_bytes = (tile_dir / SPANS_FILE).read_bytes()
                self.assertEqual(
                    len(count_bytes), area,
                    f"{tile['dir']}: spans_count.bin must be {area} bytes (u8/col)",
                )
                self.assertEqual(
                    len(spans_bytes), area * SPAN_BYTES_PER_COLUMN,
                    f"{tile['dir']}: spans.bin must be {area * SPAN_BYTES_PER_COLUMN}"
                    " bytes (16/col)",
                )

    def test_height_and_deleted_layers_not_written(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, _fields = _export("sky_isle", td)
            for tile in manifest["tiles"]:
                tile_dir = raster_dir / tile["dir"]
                self.assertFalse(
                    (tile_dir / "height.bin").exists(),
                    "height.bin must be folded into spans, never written",
                )
                self.assertNotIn("height", tile["layers"])
                for layer in DELETED_VERTICAL_LAYERS:
                    self.assertFalse(
                        (tile_dir / f"{layer}.bin").exists(),
                        f"{layer}.bin must not be written (folded into spans)",
                    )
                    self.assertNotIn(layer, tile["layers"])

    def test_semantic_vertical_rasters_still_written(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, _fields = _export("sky_isle", td)
            tile = manifest["tiles"][0]
            tile_dir = raster_dir / tile["dir"]
            self.assertTrue((tile_dir / "sky_island_mask.bin").exists())
            self.assertIn("sky_island_mask", tile["layers"])

    def test_declared_vertical_layers_all_exist_on_disk(self) -> None:
        # worldgen-v4 P0 Major #3: manifest.vertical_layers must be gated on what
        # was actually written (written_layer_names), NOT on registry membership.
        # Pin that EVERY declared vertical layer has a real .bin in EVERY tile —
        # otherwise the Rust reader mmaps a missing file.  A registry-only gate
        # would silently re-introduce the mismatch this test guards against.
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, _fields = _export("sky_isle", td)
            declared = manifest["vertical_layers"]
            self.assertTrue(
                declared,
                "sky_isle profile must yield at least sky_island_mask in "
                "vertical_layers — empty means the gate over-trimmed",
            )
            for tile in manifest["tiles"]:
                tile_dir = raster_dir / tile["dir"]
                for layer in declared:
                    self.assertTrue(
                        (tile_dir / f"{layer}.bin").exists(),
                        f"manifest declares vertical layer '{layer}' but "
                        f"{tile['dir']}/{layer}.bin is absent — manifest↔disk "
                        f"mismatch (Rust load would fail to mmap)",
                    )
                    self.assertIn(
                        layer, tile["layers"],
                        f"'{layer}' declared in vertical_layers but missing from "
                        f"tile['layers'] of {tile['dir']}",
                    )

    def test_decoded_column_surface_matches_shim_fold(self) -> None:
        # The bytes on disk must decode (offset = col_idx * stride) to exactly
        # what the shim folded — the binary path is the contract, not an
        # internal re-fold.
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, fields = _export("sky_isle", td)
            tile_meta = manifest["tiles"][0]
            tile_dir = raster_dir / tile_meta["dir"]
            count_bytes = (tile_dir / SPANS_COUNT_FILE).read_bytes()
            spans_bytes = (tile_dir / SPANS_FILE).read_bytes()

            # Find the matching in-memory tile buffer and re-fold it.
            buf = next(
                t for t in fields.tiles if t.tile.tile_id == tile_meta["dir"]
            )
            folded = spans_for_tile(buf)
            for col_idx in (0, 100, len(folded) // 2, len(folded) - 1):
                decoded = decode_spans_column(count_bytes, spans_bytes, col_idx)
                self.assertEqual(
                    decoded.spans,
                    folded[col_idx].spans,
                    f"column {col_idx} on disk != shim fold (offset "
                    f"{col_idx * SPAN_BYTES_PER_COLUMN})",
                )


if __name__ == "__main__":
    unittest.main()
