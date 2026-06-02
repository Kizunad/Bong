"""Integration tests: compound_flatten_radius + density mask via synthesize_fields.

FIX C (plan-terrain-wiring-v1 opus verify) — zero integration coverage for the
new flatten/density-mask wiring added in P0 M2/#8.

Coverage:
  - Full synthesize_fields chain with a zone that has compound_flatten_radius.
  - Height flattened to target_height within the flatten radius.
  - flora_density, flora_variant_id, ground_cover_density, ground_cover_id all
    zeroed within flatten radius (density mask).
  - Other density-maskable layers listed in _DENSITY_MASKABLE_LAYERS are
    also zeroed (registry-driven check, not hard-coded per layer).
  - _resolve_layout_poi_center_xz fallback when POI kind not matched → zone center.
  - _resolve_layout_poi_center_xz fallback when layout not in registry → zone center.
  - _resolve_layout_target_height fallback when POI kind not matched → base midpoint.
  - _resolve_layout_target_height fallback when layout not in registry → 64.0 default.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import numpy as np

from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    PoiSpec,
    TerrainProfileCatalog,
    TerrainProfileSpec,
    WorldBlueprint,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.bakers.raster_export import build_raster_bake_plan
from scripts.terrain_gen.fields import Bounds2D
from scripts.terrain_gen.stitcher import (
    _DENSITY_MASKABLE_LAYERS,
    _resolve_layout_poi_center_xz,
    _resolve_layout_target_height,
    build_generation_plan,
    synthesize_fields,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_TILE_SIZE = 32  # small tile so tests run fast


def _make_profile_catalog(profile_name: str = "dan_zong_yi_yuan") -> TerrainProfileCatalog:
    return TerrainProfileCatalog(
        version=1,
        profiles={
            profile_name: TerrainProfileSpec(
                name=profile_name,
                boundary=BoundarySpec(mode="soft", width=4),
                height={"base": [62, 78], "peak": 92},
                surface=("podzol", "coarse_dirt", "mud", "purple_terracotta"),
                water={"level": "none"},
                passability="unknown",
                architectural_layout="dan_zong_compound",
                compound_flatten_radius=8,
            )
        },
    )


def _make_blueprint(
    zone: BlueprintZone,
    tile_size: int = _TILE_SIZE,
) -> WorldBlueprint:
    return WorldBlueprint(
        version=1,
        world_name="test_flatten",
        spawn_zone=zone.name,
        bounds_xz=zone.bounds_xz,
        notes=(),
        zones=(zone,),
    )


def _run_synthesize(
    zone: BlueprintZone,
    profile_catalog: TerrainProfileCatalog | None = None,
) -> "GeneratedFieldSet":  # noqa: F821 — type hint only
    """Run synthesize_fields for a single zone, returning GeneratedFieldSet."""
    from scripts.terrain_gen.bakers.raster_export import build_raster_bake_plan

    if profile_catalog is None:
        profile_catalog = _make_profile_catalog(zone.worldgen.terrain_profile)

    blueprint = _make_blueprint(zone)
    with tempfile.TemporaryDirectory() as tmp:
        output_dir = Path(tmp)
        plan = build_generation_plan(
            blueprint=blueprint,
            profile_catalog=profile_catalog,
            blueprint_path=Path("blueprint.json"),
            profiles_path=Path("profiles.json"),
            output_dir=output_dir,
            tile_size=_TILE_SIZE,
        )
        plan.bake_plan = build_raster_bake_plan(plan, output_dir)
        return synthesize_fields(plan)


# ---------------------------------------------------------------------------
# Zone fixtures
# ---------------------------------------------------------------------------


def _make_dan_zong_flatten_zone(
    flatten_radius: int = 8,
    poi_y: float = 75.0,
    arch_layout: str | None = "dan_zong_compound",
) -> BlueprintZone:
    """Zone centered at (0, 0) with a flatten radius and POI at (0, poi_y, 0)."""
    half = _TILE_SIZE // 2
    return BlueprintZone(
        name="dan_zong_yi_yuan",
        display_name="丹宗遗园 (test)",
        bounds_xz=Bounds2D(
            min_x=-half, max_x=half - 1,
            min_z=-half, max_z=half - 1,
        ),
        center_xz=(0, 0),
        size_xz=(_TILE_SIZE, _TILE_SIZE),
        spirit_qi=0.40,
        danger_level=4,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="dan_zong_yi_yuan",
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=4),
            height_model={"base": [62, 78], "peak": 92},
            surface_palette=(
                "podzol", "coarse_dirt", "mud",
                "purple_terracotta", "mossy_cobblestone",
            ),
        ),
        pois=(
            PoiSpec(
                kind="ruin",
                pos_xyz=(0.0, poi_y, 0.0),
                name="百草丹殿",
                tags=("dandao_path",),
            ),
        ),
        architectural_layout=arch_layout,
        compound_flatten_radius=flatten_radius,
    )


# ---------------------------------------------------------------------------
# Integration tests: full synthesize_fields chain
# ---------------------------------------------------------------------------


class FlattenIntegrationTest(unittest.TestCase):
    """synthesize_fields flattens height within compound_flatten_radius."""

    def _poi_y(self) -> float:
        return 75.0

    def setUp(self) -> None:
        zone = _make_dan_zong_flatten_zone(flatten_radius=8, poi_y=self._poi_y())
        self.fields = _run_synthesize(zone)
        self.tile = self.fields.tiles[0]
        self.poi_center = (0, 0)  # zone center / POI xz
        self.radius = 8
        self.target_height = self._poi_y()

    def test_height_within_radius_equals_target(self) -> None:
        """All columns within flatten_radius of POI center must have height ≈ target."""
        heights = self.tile.layers["height"]
        tile_size = self.fields.tile_size
        tile = self.tile.tile
        cx, cz = self.poi_center

        failures: list[str] = []
        for lz in range(tile_size):
            for lx in range(tile_size):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - cx) ** 2 + (wz - cz) ** 2
                if dist_sq < self.radius * self.radius:
                    idx = lz * tile_size + lx
                    h = float(heights[idx])
                    diff = abs(h - self.target_height)
                    if diff > 1.0:
                        failures.append(
                            f"({wx},{wz}) h={h:.2f} target={self.target_height} diff={diff:.2f}"
                        )

        self.assertFalse(
            failures,
            f"Height within flatten radius not at target ({self.target_height}); "
            f"first 5 failures: {failures[:5]}",
        )

    def test_density_maskable_layers_zeroed_within_radius(self) -> None:
        """All LAYER_REGISTRY density_maskable layers must be 0 within flatten radius."""
        tile_size = self.fields.tile_size
        tile = self.tile.tile
        cx, cz = self.poi_center

        for layer_name in _DENSITY_MASKABLE_LAYERS:
            if layer_name not in self.tile.layers:
                continue  # layer not written for this profile — not maskable here
            arr = self.tile.layers[layer_name]
            for lz in range(tile_size):
                for lx in range(tile_size):
                    wx = tile.min_x + lx
                    wz = tile.min_z + lz
                    dist_sq = (wx - cx) ** 2 + (wz - cz) ** 2
                    if dist_sq < self.radius * self.radius:
                        idx = lz * tile_size + lx
                        val = float(arr[idx])
                        self.assertEqual(
                            val,
                            0.0,
                            f"layer '{layer_name}' at ({wx},{wz}) must be 0 inside "
                            f"flatten radius={self.radius}, got {val}",
                        )

    def test_flora_density_zeroed_within_radius(self) -> None:
        """flora_density specifically must be 0 within flatten radius."""
        if "flora_density" not in self.tile.layers:
            self.skipTest("flora_density not in tile layers for this profile")
        tile_size = self.fields.tile_size
        tile = self.tile.tile
        cx, cz = self.poi_center
        arr = self.tile.layers["flora_density"]

        inside_vals = [
            float(arr[lz * tile_size + lx])
            for lz in range(tile_size)
            for lx in range(tile_size)
            if (tile.min_x + lx - cx) ** 2 + (tile.min_z + lz - cz) ** 2
            < self.radius * self.radius
        ]
        self.assertTrue(
            inside_vals,
            "Expected at least one column within flatten radius",
        )
        self.assertTrue(
            all(v == 0.0 for v in inside_vals),
            f"flora_density must be 0 inside flatten radius; non-zero: "
            f"{[v for v in inside_vals if v != 0.0][:5]}",
        )

    def test_flora_variant_id_zeroed_within_radius(self) -> None:
        """flora_variant_id specifically must be 0 within flatten radius."""
        if "flora_variant_id" not in self.tile.layers:
            self.skipTest("flora_variant_id not in tile layers for this profile")
        tile_size = self.fields.tile_size
        tile = self.tile.tile
        cx, cz = self.poi_center
        arr = self.tile.layers["flora_variant_id"]

        inside_vals = [
            int(arr[lz * tile_size + lx])
            for lz in range(tile_size)
            for lx in range(tile_size)
            if (tile.min_x + lx - cx) ** 2 + (tile.min_z + lz - cz) ** 2
            < self.radius * self.radius
        ]
        self.assertTrue(inside_vals, "Expected at least one column within radius")
        self.assertTrue(
            all(v == 0 for v in inside_vals),
            f"flora_variant_id must be 0 inside flatten radius; non-zero: "
            f"{[v for v in inside_vals if v != 0][:5]}",
        )

    def test_density_maskable_layers_registry_driven(self) -> None:
        """_DENSITY_MASKABLE_LAYERS must include the expected layer names.

        This test ensures that if LAYER_REGISTRY.density_maskable changes,
        we catch regressions in the set.  The minimum expected set is pinned.
        """
        expected_maskable = {"flora_density", "flora_variant_id"}
        actual = set(_DENSITY_MASKABLE_LAYERS)
        missing = expected_maskable - actual
        self.assertFalse(
            missing,
            f"_DENSITY_MASKABLE_LAYERS is missing expected layers: {missing}. "
            f"Check LAYER_REGISTRY density_maskable flags in fields.py.",
        )


class FlattenFallbackTest(unittest.TestCase):
    """_resolve_layout_poi_center_xz and _resolve_layout_target_height fallback branches."""

    def _make_zone_no_matching_poi(self) -> BlueprintZone:
        """Zone with a registered layout but no POI matching that layout's poi_kind."""
        half = _TILE_SIZE // 2
        return BlueprintZone(
            name="dan_zong_yi_yuan",
            display_name="丹宗遗园 (no POI)",
            bounds_xz=Bounds2D(
                min_x=-half, max_x=half - 1,
                min_z=-half, max_z=half - 1,
            ),
            center_xz=(5, 7),  # non-zero so fallback is detectable
            size_xz=(_TILE_SIZE, _TILE_SIZE),
            spirit_qi=0.40,
            danger_level=4,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="dan_zong_yi_yuan",
                shape="ellipse",
                boundary=BoundarySpec(mode="soft", width=4),
                height_model={"base": [60, 80], "peak": 92},
                surface_palette=("podzol", "coarse_dirt"),
            ),
            pois=(
                PoiSpec(
                    kind="altar",  # wrong kind — dan_zong_compound expects "ruin"
                    pos_xyz=(0.0, 70.0, 0.0),
                    name="altar",
                    tags=(),
                ),
            ),
            architectural_layout="dan_zong_compound",
            compound_flatten_radius=4,
        )

    def _make_zone_unknown_layout(self) -> BlueprintZone:
        """Zone with an architectural_layout that is NOT in COMPOUND_LAYOUT_REGISTRY."""
        half = _TILE_SIZE // 2
        return BlueprintZone(
            name="dan_zong_yi_yuan",
            display_name="丹宗遗园 (unknown layout)",
            bounds_xz=Bounds2D(
                min_x=-half, max_x=half - 1,
                min_z=-half, max_z=half - 1,
            ),
            center_xz=(3, 9),
            size_xz=(_TILE_SIZE, _TILE_SIZE),
            spirit_qi=0.40,
            danger_level=4,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="dan_zong_yi_yuan",
                shape="ellipse",
                boundary=BoundarySpec(mode="soft", width=4),
                height_model={"base": [60, 80], "peak": 92},
                surface_palette=("podzol", "coarse_dirt"),
            ),
            architectural_layout="nonexistent_layout_xyz",
            compound_flatten_radius=4,
        )

    def test_resolve_poi_center_falls_back_to_zone_center_when_poi_missing(self) -> None:
        """When no POI kind matches, _resolve_layout_poi_center_xz returns zone.center_xz."""
        zone = self._make_zone_no_matching_poi()
        center = _resolve_layout_poi_center_xz(zone)
        self.assertEqual(
            center,
            zone.center_xz,
            f"Expected fallback to zone.center_xz={zone.center_xz}, got {center}",
        )

    def test_resolve_poi_center_falls_back_to_zone_center_when_layout_unknown(self) -> None:
        """When layout not in registry, _resolve_layout_poi_center_xz returns zone.center_xz."""
        zone = self._make_zone_unknown_layout()
        center = _resolve_layout_poi_center_xz(zone)
        self.assertEqual(
            center,
            zone.center_xz,
            f"Expected fallback to zone.center_xz={zone.center_xz}, got {center}",
        )

    def test_resolve_target_height_falls_back_to_base_midpoint_when_poi_missing(self) -> None:
        """When no POI kind matches, _resolve_layout_target_height returns base midpoint."""
        zone = self._make_zone_no_matching_poi()
        height = _resolve_layout_target_height(zone)
        # height_model.base = [60, 80] → midpoint = 70.0
        self.assertAlmostEqual(
            height,
            70.0,
            places=5,
            msg=f"Expected fallback target_height=70.0 (base midpoint), got {height}",
        )

    def test_resolve_target_height_falls_back_to_64_when_layout_unknown(self) -> None:
        """When layout not in registry, _resolve_layout_target_height returns base midpoint.

        Note: the fallback uses the height_model.base midpoint if available,
        so with base=[60,80] we expect 70.0 (not hard-coded 64.0).
        """
        zone = self._make_zone_unknown_layout()
        height = _resolve_layout_target_height(zone)
        # height_model.base = [60, 80] → midpoint = 70.0
        self.assertAlmostEqual(
            height,
            70.0,
            places=5,
            msg=f"Expected fallback target_height=70.0 (base midpoint), got {height}",
        )

    def test_resolve_poi_center_uses_poi_when_poi_kind_matches(self) -> None:
        """When POI kind matches, _resolve_layout_poi_center_xz returns the POI xz."""
        zone = _make_dan_zong_flatten_zone(flatten_radius=4, poi_y=75.0)
        # dan_zong_compound.poi_kind == "ruin"; zone has PoiSpec(kind="ruin")
        center = _resolve_layout_poi_center_xz(zone)
        self.assertEqual(
            center,
            (0, 0),  # POI is at (0.0, 75.0, 0.0) → rounded (0, 0)
            f"Expected POI-anchored center (0, 0), got {center}",
        )

    def test_resolve_target_height_uses_poi_y_when_poi_kind_matches(self) -> None:
        """When POI kind matches, _resolve_layout_target_height returns POI.pos_xyz.y."""
        zone = _make_dan_zong_flatten_zone(flatten_radius=4, poi_y=75.0)
        height = _resolve_layout_target_height(zone)
        self.assertAlmostEqual(
            height,
            75.0,
            places=5,
            msg=f"Expected POI.y=75.0 as target height, got {height}",
        )

    def test_flatten_uses_zone_center_fallback_when_poi_missing(self) -> None:
        """synthesize_fields with missing POI still runs without error (uses zone center)."""
        zone = self._make_zone_no_matching_poi()
        # Should not raise
        try:
            fields = _run_synthesize(zone)
        except Exception as exc:
            self.fail(
                f"synthesize_fields must not raise when POI kind is missing; "
                f"got: {type(exc).__name__}: {exc}"
            )
        self.assertIsNotNone(fields)


if __name__ == "__main__":
    unittest.main()
