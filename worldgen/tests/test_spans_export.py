"""worldgen-v4 P0 — raster export spans binary-layout + manifest v2 pin.

End-to-end export of a real profile, asserting the on-disk contract:
  * spans_count.bin (u8/col) + spans.bin (SPAN_BYTES_PER_COLUMN/col) written;
  * height.bin and the six deleted vertical patch layers are NOT written;
  * sky_island_mask + underground_tier remain as semantic rasters;
  * manifest version == 2, carries spans_encoding; vertical_layers lists only the
    semantic vertical rasters a given export actually wrote (gated on disk, not on
    registry membership — Major #3);
  * decoding column 0 from the raw files via the i16-LE/sentinel layout matches
    the surface folded by spans_fold (offset = col_idx * stride round-trip).
"""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.bakers.raster_export import (
    CARVE_SEED,
    SPANS_COUNT_FILE,
    SPANS_FILE,
    _tile_carver_chain,
    _zone_carver_chains,
    build_raster_bake_plan,
    export_rasters,
)
from scripts.terrain_gen.carvers import apply_carver_chain
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
from scripts.terrain_gen.spans_fold import spans_for_tile
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
    return manifest, artifacts["raster_dir"], fields, plan


class SpansExportLayoutTest(unittest.TestCase):
    def test_manifest_is_version_2_with_spans_encoding(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            manifest, _raster_dir, _fields, _plan = _export("sky_isle", td)
        self.assertEqual(manifest["version"], 2)
        enc = manifest["spans_encoding"]
        self.assertEqual(enc["max_spans"], 4)
        self.assertEqual(enc["bytes_per_column"], SPAN_BYTES_PER_COLUMN)
        self.assertEqual(enc["sentinel"], SPAN_SENTINEL)
        self.assertEqual(enc["count_file"], SPANS_COUNT_FILE)
        self.assertEqual(enc["spans_file"], SPANS_FILE)
        self.assertEqual(manifest["qi_density_source"], "qi_field")
        self.assertIn("qi_density", manifest["semantic_layers"])

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
                    manifest, raster_dir, _fields, _plan = _export(profile, td)
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
            manifest, raster_dir, _fields, _plan = _export("cave_network", td)
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
            manifest, raster_dir, _fields, _plan = _export("sky_isle", td)
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
            manifest, raster_dir, _fields, _plan = _export("sky_isle", td)
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
            manifest, raster_dir, _fields, _plan = _export("sky_isle", td)
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

    def test_structure_layers_gated_on_written_not_registry(self) -> None:
        # worldgen-v4 P0 (CodeRabbit #520): structure_layers (fossil_bbox) must be
        # gated on written_layer_names too — a registry-only check declared
        # fossil_bbox for every export even when no tile wrote it.  sky_isle has no
        # fossil POIs, so fossil_bbox must NOT appear; and any layer that IS declared
        # must have a real .bin on disk (else the Rust reader mmaps a missing file).
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, _fields, _plan = _export("sky_isle", td)
            self.assertNotIn(
                "fossil_bbox", manifest["structure_layers"],
                "sky_isle writes no fossils, yet structure_layers still declares "
                "fossil_bbox — registry-only phantom regression",
            )
            for tile in manifest["tiles"]:
                tile_dir = raster_dir / tile["dir"]
                for layer in manifest["structure_layers"]:
                    self.assertTrue(
                        (tile_dir / f"{layer}.bin").exists(),
                        f"manifest declares structure layer '{layer}' but "
                        f"{tile['dir']}/{layer}.bin is absent — manifest↔disk mismatch",
                    )

    def test_decoded_column_surface_matches_span_fold_and_carve(self) -> None:
        # The bytes on disk must decode (offset = col_idx * stride) to exactly
        # what the export wrote: spans_fold THEN the zone's P3 carver chain
        # (sky_isle = floating_island).  The binary path is the contract — we
        # reproduce fold+carve here and assert byte-for-byte, so a regression in
        # the carve hook or the encoding offset turns this red.
        with tempfile.TemporaryDirectory() as td:
            manifest, raster_dir, fields, plan = _export("sky_isle", td)
            tile_meta = manifest["tiles"][0]
            tile_dir = raster_dir / tile_meta["dir"]
            count_bytes = (tile_dir / SPANS_COUNT_FILE).read_bytes()
            spans_bytes = (tile_dir / SPANS_FILE).read_bytes()

            # Find the matching in-memory tile buffer, fold, then carve exactly
            # as the export does (same chain lookup + CARVE_SEED).
            buf = next(
                t for t in fields.tiles if t.tile.tile_id == tile_meta["dir"]
            )
            zone_chains = _zone_carver_chains(plan)
            chain = _tile_carver_chain(buf, zone_chains)
            self.assertTrue(
                chain,
                "sky_isle tile must resolve a floating_island carver chain — "
                "the export carves it, so the round-trip reference must too",
            )
            # worldgen-v4 P3 §6.1 双源收口: the export folds with
            # suppress_fold_isle=True whenever the chain owns the isle, so the
            # round-trip baseline MUST mirror that flag or it would silently
            # diverge from the on-disk bytes when the 2D fold also has isle data.
            suppress_fold_isle = any(c.name == "floating_island" for c in chain)
            folded = spans_for_tile(buf, suppress_fold_isle=suppress_fold_isle)
            carved = apply_carver_chain(
                folded,
                chain,
                origin_x=buf.tile.min_x,
                origin_z=buf.tile.min_z,
                tile_size=buf.tile_size,
                seed=CARVE_SEED,
            )
            # The chain actually changed at least one column (a real isle span),
            # else this test would pass even if carving silently no-op'd.
            self.assertNotEqual(
                [c.spans for c in carved],
                [c.spans for c in folded],
                "sky_isle carve must add isle spans to some columns",
            )
            for col_idx in (0, 100, len(carved) // 2, len(carved) - 1):
                decoded = decode_spans_column(count_bytes, spans_bytes, col_idx)
                self.assertEqual(
                    decoded.spans,
                    carved[col_idx].spans,
                    f"column {col_idx} on disk != fold+carve (offset "
                    f"{col_idx * SPAN_BYTES_PER_COLUMN})",
                )


class _StubZonePlan:
    def __init__(self, zone_name: str, profile_name: str) -> None:
        self.zone_name = zone_name
        self.profile_name = profile_name


class _StubPlan:
    def __init__(self, zone_plans) -> None:
        self.zone_plans = zone_plans


class ZoneCarverChainResolutionTest(unittest.TestCase):
    """_zone_carver_chains must fail fast on an unregistered profile."""

    def test_none_plan_yields_empty_map(self) -> None:
        # The incremental console path has no plan — no carving, no crash.
        self.assertEqual(_zone_carver_chains(None), {})

    def test_known_profile_resolves_its_chain(self) -> None:
        # sky_isle declares a floating_island chain; it must materialize.
        plan = _StubPlan([_StubZonePlan("z", "sky_isle")])
        chains = _zone_carver_chains(plan)
        self.assertIn("z", chains)
        self.assertTrue(
            chains["z"],
            "sky_isle must resolve a non-empty floating_island carver chain",
        )

    def test_unregistered_profile_raises_not_silently_skipped(self) -> None:
        # A zone referencing a profile with no generator is a misconfig — the
        # export must fail loudly, not ship that zone flat/un-carved.
        plan = _StubPlan([_StubZonePlan("ghost_zone", "no_such_profile_xyz")])
        with self.assertRaises(KeyError) as ctx:
            _zone_carver_chains(plan)
        msg = str(ctx.exception)
        self.assertIn("ghost_zone", msg)
        self.assertIn("no_such_profile_xyz", msg)


class ZoneFilterValidationTest(unittest.TestCase):
    """synthesize_fields must fail fast on an unknown zone_filter name."""

    def _plan(self, td: str):
        zone = _build_zone("cave_network")
        blueprint = WorldBlueprint(
            version=1,
            world_name="filter_world",
            spawn_zone=zone.name,
            bounds_xz=zone.bounds_xz,
            notes=(),
            zones=(zone,),
        )
        catalog = TerrainProfileCatalog(
            version=1,
            profiles={
                "cave_network": TerrainProfileSpec(
                    name="cave_network",
                    boundary=BoundarySpec(mode="soft", width=24),
                    height={"base": [60, 80], "peak": 96},
                    surface=("grass_block", "stone", "gravel"),
                    water={"level": "none", "coverage": 0.0},
                    passability="medium",
                    extras={},
                )
            },
        )
        return build_generation_plan(
            blueprint=blueprint,
            profile_catalog=catalog,
            blueprint_path=Path("b.json"),
            profiles_path=Path("p.json"),
            output_dir=Path(td),
            tile_size=TILE_SIZE,
        ), zone.name

    def test_unknown_zone_filter_raises_with_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            plan, known_name = self._plan(td)
            with self.assertRaises(ValueError) as ctx:
                synthesize_fields(plan, zone_filter={"does_not_exist"})
        msg = str(ctx.exception)
        self.assertIn(
            "does_not_exist", msg,
            "the error must name the offending unknown zone so the typo is "
            f"obvious; got: {msg!r}",
        )
        self.assertIn(
            known_name, msg,
            "the error must list the available zone names as candidates; got: "
            f"{msg!r}",
        )

    def test_known_zone_filter_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            plan, known_name = self._plan(td)
            fields = synthesize_fields(plan, zone_filter={known_name})
        self.assertTrue(
            fields.tiles,
            "a valid zone_filter must still synthesize that zone's tiles",
        )

    def test_none_zone_filter_skips_validation(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            plan, _ = self._plan(td)
            fields = synthesize_fields(plan, zone_filter=None)
        self.assertTrue(fields.tiles, "no filter must export the full world")


if __name__ == "__main__":
    unittest.main()
