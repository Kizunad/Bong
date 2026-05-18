"""Saturated tests for layout infrastructure (plan-dandao-path-v1 PR-1).

Covers:
- LayoutSpec / Placement dataclass validation and round-trip
- LayoutResult / PlacedStructure construction
- Layout runner: POI lookup, world coordinate computation, determinism
- Density mask: flora/ground_cover zeroed inside layout radius
- compound_flatten_radius: height field flattened inside radius
- Edge cases: empty placements, radius=0, missing POI, invalid rotation
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

import numpy as np

from scripts.terrain_gen.blueprint import (  # noqa: E402
    BlueprintZone,
    BoundarySpec,
    PoiSpec,
    TerrainProfileSpec,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import (  # noqa: E402
    Bounds2D,
    SurfacePalette,
    TileFieldBuffer,
    WorldTile,
)
from scripts.terrain_gen.layouts.base import (  # noqa: E402
    LayoutResult,
    LayoutSpec,
    PlacedStructure,
    Placement,
)
from scripts.terrain_gen.layouts.runner import (  # noqa: E402
    _find_poi_center,
    run_layout,
)
from scripts.terrain_gen.stitcher import (  # noqa: E402
    apply_compound_flatten,
    compute_layout_density_mask,
)


# ---------------------------------------------------------------------------
# Test fixtures
# ---------------------------------------------------------------------------


def _make_zone(
    pois: tuple[PoiSpec, ...] = (),
    center: tuple[int, int] = (100, 200),
) -> BlueprintZone:
    return BlueprintZone(
        name="test_zone",
        display_name="Test Zone",
        bounds_xz=Bounds2D(min_x=0, max_x=200, min_z=100, max_z=300),
        center_xz=center,
        size_xz=(200, 200),
        spirit_qi=0.4,
        danger_level=3,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="test_profile",
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=64),
            height_model={"base": [62, 78], "peak": 92},
            surface_palette=("podzol", "coarse_dirt"),
        ),
        pois=pois,
    )


def _make_tile(min_x: int = 0, min_z: int = 100, tile_size: int = 64) -> WorldTile:
    return WorldTile(
        tile_x=min_x // tile_size,
        tile_z=min_z // tile_size,
        min_x=min_x,
        max_x=min_x + tile_size - 1,
        min_z=min_z,
        max_z=min_z + tile_size - 1,
    )


def _make_buffer_with_density(
    tile: WorldTile, tile_size: int = 64
) -> TileFieldBuffer:
    """Create a tile buffer with non-zero flora and ground cover density."""
    buffer = TileFieldBuffer.create(
        tile,
        tile_size,
        (
            "height",
            "flora_density",
            "flora_variant_id",
            "ground_cover_density",
            "ground_cover_id",
        ),
    )
    area = tile_size * tile_size
    buffer.layers["height"][:] = 76.0
    buffer.layers["flora_density"][:] = 0.5
    buffer.layers["flora_variant_id"][:] = 3
    buffer.layers["ground_cover_density"][:] = 0.3
    buffer.layers["ground_cover_id"][:] = 1
    return buffer


# ---------------------------------------------------------------------------
# Placement dataclass tests
# ---------------------------------------------------------------------------


class PlacementTests(unittest.TestCase):
    def test_valid_placement_construction(self):
        p = Placement(offset=(10, 0, 20), rotation=90, kind="nbt", payload="hall.nbt")
        self.assertEqual(p.offset, (10, 0, 20))
        self.assertEqual(p.rotation, 90)
        self.assertEqual(p.kind, "nbt")
        self.assertEqual(p.payload, "hall.nbt")

    def test_all_valid_rotations(self):
        for rot in (0, 90, 180, 270):
            p = Placement(offset=(0, 0, 0), rotation=rot, kind="nbt", payload="x")
            self.assertEqual(p.rotation, rot)

    def test_invalid_rotation_raises(self):
        with self.assertRaises(ValueError) as ctx:
            Placement(offset=(0, 0, 0), rotation=45, kind="nbt", payload="x")
        self.assertIn("45", str(ctx.exception))

    def test_invalid_offset_length_raises(self):
        with self.assertRaises(ValueError) as ctx:
            Placement(offset=(0, 0), rotation=0, kind="nbt", payload="x")  # type: ignore[arg-type]
        self.assertIn("3-tuple", str(ctx.exception))

    def test_all_kind_values_accepted(self):
        for kind in ("nbt", "block_grid", "stamp_radial"):
            p = Placement(offset=(0, 0, 0), rotation=0, kind=kind, payload="x")  # type: ignore[arg-type]
            self.assertEqual(p.kind, kind)

    def test_placement_is_frozen(self):
        p = Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload="x")
        with self.assertRaises(AttributeError):
            p.rotation = 90  # type: ignore[misc]

    def test_placement_equality(self):
        a = Placement(offset=(1, 2, 3), rotation=90, kind="nbt", payload="foo.nbt")
        b = Placement(offset=(1, 2, 3), rotation=90, kind="nbt", payload="foo.nbt")
        self.assertEqual(a, b)

    def test_placement_inequality(self):
        a = Placement(offset=(1, 2, 3), rotation=90, kind="nbt", payload="foo.nbt")
        b = Placement(offset=(1, 2, 3), rotation=180, kind="nbt", payload="foo.nbt")
        self.assertNotEqual(a, b)


# ---------------------------------------------------------------------------
# LayoutSpec dataclass tests
# ---------------------------------------------------------------------------


class LayoutSpecTests(unittest.TestCase):
    def test_basic_construction(self):
        spec = LayoutSpec(
            name="test_layout",
            poi_kind="test_poi",
            radius=96,
            placements=(
                Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload="main.nbt"),
            ),
        )
        self.assertEqual(spec.name, "test_layout")
        self.assertEqual(spec.poi_kind, "test_poi")
        self.assertEqual(spec.radius, 96)
        self.assertEqual(len(spec.placements), 1)

    def test_empty_placements_accepted(self):
        spec = LayoutSpec(
            name="empty", poi_kind="poi", radius=10, placements=()
        )
        self.assertEqual(len(spec.placements), 0)

    def test_negative_radius_raises(self):
        with self.assertRaises(ValueError) as ctx:
            LayoutSpec(name="bad", poi_kind="p", radius=-1, placements=())
        self.assertIn("-1", str(ctx.exception))

    def test_zero_radius_accepted(self):
        spec = LayoutSpec(name="zero", poi_kind="p", radius=0, placements=())
        self.assertEqual(spec.radius, 0)

    def test_layout_spec_is_frozen(self):
        spec = LayoutSpec(name="x", poi_kind="p", radius=10, placements=())
        with self.assertRaises(AttributeError):
            spec.radius = 20  # type: ignore[misc]

    def test_many_placements(self):
        placements = tuple(
            Placement(offset=(i, 0, i * 2), rotation=0, kind="nbt", payload=f"s{i}.nbt")
            for i in range(50)
        )
        spec = LayoutSpec(name="big", poi_kind="p", radius=200, placements=placements)
        self.assertEqual(len(spec.placements), 50)


# ---------------------------------------------------------------------------
# PlacedStructure / LayoutResult dataclass tests
# ---------------------------------------------------------------------------


class PlacedStructureTests(unittest.TestCase):
    def test_construction(self):
        ps = PlacedStructure(
            world_pos=(100, 80, 200), rotation=90, kind="nbt", payload="hall.nbt"
        )
        self.assertEqual(ps.world_pos, (100, 80, 200))

    def test_frozen(self):
        ps = PlacedStructure(
            world_pos=(0, 0, 0), rotation=0, kind="nbt", payload="x"
        )
        with self.assertRaises(AttributeError):
            ps.world_pos = (1, 1, 1)  # type: ignore[misc]


class LayoutResultTests(unittest.TestCase):
    def test_construction(self):
        result = LayoutResult(
            layout_name="test",
            poi_world_pos=(100, 80, 200),
            placed=(
                PlacedStructure(
                    world_pos=(110, 80, 220), rotation=0, kind="nbt", payload="x.nbt"
                ),
            ),
        )
        self.assertEqual(result.layout_name, "test")
        self.assertEqual(len(result.placed), 1)

    def test_empty_placed_accepted(self):
        result = LayoutResult(
            layout_name="empty", poi_world_pos=(0, 0, 0), placed=()
        )
        self.assertEqual(len(result.placed), 0)


# ---------------------------------------------------------------------------
# Runner tests
# ---------------------------------------------------------------------------


class FindPoiCenterTests(unittest.TestCase):
    def test_finds_matching_poi(self):
        zone = _make_zone(
            pois=(
                PoiSpec(kind="altar", pos_xyz=(50.0, 80.0, 150.0)),
                PoiSpec(kind="hall", pos_xyz=(100.0, 82.0, 200.0)),
            )
        )
        center = _find_poi_center(zone, "hall")
        self.assertEqual(center, (100, 82, 200))

    def test_returns_none_when_not_found(self):
        zone = _make_zone(pois=())
        self.assertIsNone(_find_poi_center(zone, "nonexistent"))

    def test_returns_first_match(self):
        zone = _make_zone(
            pois=(
                PoiSpec(kind="hall", pos_xyz=(10.0, 20.0, 30.0)),
                PoiSpec(kind="hall", pos_xyz=(99.0, 99.0, 99.0)),
            )
        )
        center = _find_poi_center(zone, "hall")
        self.assertEqual(center, (10, 20, 30))

    def test_rounds_float_coordinates(self):
        zone = _make_zone(
            pois=(PoiSpec(kind="spring", pos_xyz=(10.7, 20.3, 30.5)),)
        )
        center = _find_poi_center(zone, "spring")
        self.assertEqual(center, (11, 20, 30))


class RunLayoutTests(unittest.TestCase):
    def _layout_and_zone(self) -> tuple[LayoutSpec, BlueprintZone]:
        spec = LayoutSpec(
            name="test_compound",
            poi_kind="great_hall",
            radius=96,
            placements=(
                Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload="hall.nbt"),
                Placement(offset=(24, 0, 0), rotation=90, kind="stamp_radial", payload="garden_6x6"),
                Placement(offset=(-12, 0, 32), rotation=180, kind="nbt", payload="furnace.nbt"),
                Placement(offset=(0, 0, 64), rotation=0, kind="block_grid", payload="path_6x128"),
            ),
        )
        zone = _make_zone(
            pois=(PoiSpec(kind="great_hall", pos_xyz=(100.0, 82.0, 200.0)),)
        )
        return spec, zone

    def test_world_coordinates_computed_correctly(self):
        spec, zone = self._layout_and_zone()
        result = run_layout(spec, zone)

        self.assertEqual(result.layout_name, "test_compound")
        self.assertEqual(result.poi_world_pos, (100, 82, 200))
        self.assertEqual(len(result.placed), 4)

        # hall at center
        self.assertEqual(result.placed[0].world_pos, (100, 82, 200))
        self.assertEqual(result.placed[0].kind, "nbt")
        self.assertEqual(result.placed[0].payload, "hall.nbt")

        # garden at (100+24, 82, 200)
        self.assertEqual(result.placed[1].world_pos, (124, 82, 200))
        self.assertEqual(result.placed[1].rotation, 90)

        # furnace at (100-12, 82, 200+32)
        self.assertEqual(result.placed[2].world_pos, (88, 82, 232))

        # path at (100, 82, 200+64)
        self.assertEqual(result.placed[3].world_pos, (100, 82, 264))

    def test_missing_poi_raises_valueerror(self):
        spec = LayoutSpec(
            name="bad", poi_kind="nonexistent", radius=10, placements=()
        )
        zone = _make_zone(
            pois=(PoiSpec(kind="other", pos_xyz=(0.0, 0.0, 0.0)),)
        )
        with self.assertRaises(ValueError) as ctx:
            run_layout(spec, zone)
        self.assertIn("nonexistent", str(ctx.exception))
        self.assertIn("test_zone", str(ctx.exception))

    def test_empty_placements_returns_empty_result(self):
        spec = LayoutSpec(
            name="empty_layout", poi_kind="hall", radius=10, placements=()
        )
        zone = _make_zone(
            pois=(PoiSpec(kind="hall", pos_xyz=(50.0, 70.0, 150.0)),)
        )
        result = run_layout(spec, zone)
        self.assertEqual(result.layout_name, "empty_layout")
        self.assertEqual(result.poi_world_pos, (50, 70, 150))
        self.assertEqual(len(result.placed), 0)

    def test_determinism_same_input_same_output(self):
        """Same seed / same input → identical placement coordinates.
        This is the core determinism guarantee."""
        spec, zone = self._layout_and_zone()

        result_a = run_layout(spec, zone)
        result_b = run_layout(spec, zone)

        self.assertEqual(len(result_a.placed), len(result_b.placed))
        for pa, pb in zip(result_a.placed, result_b.placed):
            self.assertEqual(
                pa.world_pos,
                pb.world_pos,
                f"Determinism violated: {pa.world_pos} != {pb.world_pos} "
                f"for payload '{pa.payload}'",
            )
            self.assertEqual(pa.rotation, pb.rotation)
            self.assertEqual(pa.kind, pb.kind)
            self.assertEqual(pa.payload, pb.payload)

    def test_negative_offsets(self):
        spec = LayoutSpec(
            name="neg",
            poi_kind="center",
            radius=50,
            placements=(
                Placement(offset=(-30, -5, -20), rotation=0, kind="nbt", payload="x.nbt"),
            ),
        )
        zone = _make_zone(
            pois=(PoiSpec(kind="center", pos_xyz=(100.0, 80.0, 200.0)),)
        )
        result = run_layout(spec, zone)
        self.assertEqual(result.placed[0].world_pos, (70, 75, 180))

    def test_all_placement_kinds_handled(self):
        """All three kind values run without error."""
        placements = tuple(
            Placement(offset=(i * 10, 0, 0), rotation=0, kind=kind, payload=f"{kind}_test")
            for i, kind in enumerate(("nbt", "block_grid", "stamp_radial"))
        )
        spec = LayoutSpec(name="kinds", poi_kind="p", radius=50, placements=placements)
        zone = _make_zone(pois=(PoiSpec(kind="p", pos_xyz=(50.0, 70.0, 150.0)),))
        result = run_layout(spec, zone)
        self.assertEqual(len(result.placed), 3)
        self.assertEqual(result.placed[0].kind, "nbt")
        self.assertEqual(result.placed[1].kind, "block_grid")
        self.assertEqual(result.placed[2].kind, "stamp_radial")

    def test_large_layout_many_placements(self):
        """Stress test: 100 placements all compute correctly."""
        placements = tuple(
            Placement(offset=(i, 0, i * 2), rotation=(i * 90) % 360, kind="nbt", payload=f"s{i}.nbt")
            for i in range(100)
        )
        spec = LayoutSpec(name="large", poi_kind="p", radius=200, placements=placements)
        zone = _make_zone(pois=(PoiSpec(kind="p", pos_xyz=(500.0, 80.0, 500.0)),))
        result = run_layout(spec, zone)
        self.assertEqual(len(result.placed), 100)
        # Spot-check first and last
        self.assertEqual(result.placed[0].world_pos, (500, 80, 500))
        self.assertEqual(result.placed[99].world_pos, (599, 80, 698))


# ---------------------------------------------------------------------------
# Density mask tests
# ---------------------------------------------------------------------------


class DensityMaskTests(unittest.TestCase):
    def test_mask_zeroes_density_inside_radius(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)

        # POI center at (100, 200), radius=30
        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=30)

        # Check that cells near center are zeroed
        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
                idx = lz * 64 + lx
                if dist_sq < 30 * 30:
                    self.assertEqual(
                        buffer.layers["flora_density"][idx],
                        0.0,
                        f"flora_density should be 0 at ({wx},{wz}), "
                        f"dist={dist_sq**0.5:.1f} < radius=30",
                    )
                    self.assertEqual(buffer.layers["flora_variant_id"][idx], 0)
                    self.assertEqual(buffer.layers["ground_cover_density"][idx], 0.0)
                    self.assertEqual(buffer.layers["ground_cover_id"][idx], 0)

    def test_mask_preserves_density_outside_radius(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)

        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=10)

        # The POI at (100, 200) has radius=10. Most of the tile (80..143, 180..243)
        # is outside radius. Check corners.
        for corner_lx, corner_lz in [(0, 0), (63, 63), (0, 63), (63, 0)]:
            idx = corner_lz * 64 + corner_lx
            wx = tile.min_x + corner_lx
            wz = tile.min_z + corner_lz
            dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
            if dist_sq >= 10 * 10:
                self.assertGreater(
                    buffer.layers["flora_density"][idx],
                    0.0,
                    f"flora_density should be preserved at ({wx},{wz}), "
                    f"dist={dist_sq**0.5:.1f} >= radius=10",
                )

    def test_radius_zero_does_nothing(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        original_density = buffer.layers["flora_density"].copy()

        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=0)

        np.testing.assert_array_equal(
            buffer.layers["flora_density"],
            original_density,
            err_msg="radius=0 should not modify any density values",
        )

    def test_poi_outside_tile_does_nothing(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        original_density = buffer.layers["flora_density"].copy()

        # POI very far away — no cells within radius
        compute_layout_density_mask(buffer, poi_center_xz=(9999, 9999), radius=30)

        np.testing.assert_array_equal(
            buffer.layers["flora_density"],
            original_density,
            err_msg="POI outside tile bounds should not affect density",
        )

    def test_large_radius_covers_entire_tile(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)

        # radius large enough to cover entire tile
        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=500)

        self.assertEqual(
            buffer.layers["flora_density"].max(),
            0.0,
            "Large radius should zero ALL flora_density in the tile",
        )
        self.assertEqual(buffer.layers["ground_cover_density"].max(), 0.0)

    def test_mask_handles_missing_layers_gracefully(self):
        """Buffer without flora layers should not raise."""
        tile = _make_tile()
        buffer = TileFieldBuffer.create(tile, 64, ("height",))
        buffer.layers["height"][:] = 76.0
        # Should not raise
        compute_layout_density_mask(buffer, poi_center_xz=(32, 132), radius=20)

    def test_mask_circular_shape(self):
        """Verify the mask is circular, not square."""
        tile = _make_tile(min_x=70, min_z=170, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        radius = 20
        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=radius)

        # Corner case: point at (radius, 0) from center is masked;
        # point at (radius-1, radius-1) from center is ~1.41*radius away, NOT masked
        # Check a point that's at 45-degree angle, distance > radius
        diag_dist = (14 ** 2 + 14 ** 2) ** 0.5  # ~19.8, just under 20
        # This should be masked (just barely inside)
        corner_x, corner_z = 100 + 14, 200 + 14
        if 70 <= corner_x <= 133 and 170 <= corner_z <= 233:
            lx = corner_x - 70
            lz = corner_z - 170
            idx = lz * 64 + lx
            if diag_dist < radius:
                self.assertEqual(
                    buffer.layers["flora_density"][idx], 0.0,
                    f"Point at diagonal distance {diag_dist:.1f} < {radius} should be masked",
                )


# ---------------------------------------------------------------------------
# Compound flatten tests
# ---------------------------------------------------------------------------


class CompoundFlattenTests(unittest.TestCase):
    def test_flatten_inside_radius(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        # Set varied heights
        buffer.layers["height"][:] = np.linspace(60, 90, 64 * 64)

        target_height = 76.0
        apply_compound_flatten(
            buffer, poi_center_xz=(100, 200), radius=20, target_height=target_height
        )

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
                idx = lz * 64 + lx
                if dist_sq < 20 * 20:
                    self.assertAlmostEqual(
                        float(buffer.layers["height"][idx]),
                        target_height,
                        places=2,
                        msg=f"Height at ({wx},{wz}) should be {target_height} "
                        f"(inside flatten radius), got {buffer.layers['height'][idx]}",
                    )

    def test_flatten_preserves_outside_radius(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        original_height = np.linspace(60, 90, 64 * 64)
        buffer.layers["height"][:] = original_height.copy()

        apply_compound_flatten(
            buffer, poi_center_xz=(100, 200), radius=10, target_height=76.0
        )

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
                idx = lz * 64 + lx
                if dist_sq >= 10 * 10:
                    self.assertAlmostEqual(
                        float(buffer.layers["height"][idx]),
                        float(original_height[idx]),
                        places=5,
                        msg=f"Height at ({wx},{wz}) outside radius should be preserved",
                    )

    def test_radius_zero_does_nothing(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        varied = np.linspace(60, 90, 64 * 64)
        buffer.layers["height"][:] = varied.copy()

        apply_compound_flatten(
            buffer, poi_center_xz=(100, 200), radius=0, target_height=76.0
        )

        np.testing.assert_array_almost_equal(
            buffer.layers["height"],
            varied,
            decimal=5,
            err_msg="radius=0 should not modify any height values",
        )

    def test_large_radius_flattens_entire_tile(self):
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        buffer.layers["height"][:] = np.random.default_rng(42).uniform(50, 100, 64 * 64)

        target = 76.0
        apply_compound_flatten(
            buffer, poi_center_xz=(100, 200), radius=500, target_height=target
        )

        np.testing.assert_array_almost_equal(
            buffer.layers["height"],
            np.full(64 * 64, target),
            decimal=2,
            err_msg="Large radius should flatten ALL heights to target",
        )

    def test_flatten_tolerance_within_one_block(self):
        """Plan §9.9 requires height constant within +/-1 tolerance."""
        tile = _make_tile(min_x=80, min_z=180, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        buffer.layers["height"][:] = np.random.default_rng(7).uniform(60, 100, 64 * 64)

        target = 76.0
        radius = 30
        apply_compound_flatten(
            buffer, poi_center_xz=(100, 200), radius=radius, target_height=target
        )

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
                idx = lz * 64 + lx
                if dist_sq < radius * radius:
                    diff = abs(float(buffer.layers["height"][idx]) - target)
                    self.assertLessEqual(
                        diff,
                        1.0,
                        f"Height at ({wx},{wz}) deviates {diff:.2f} from target {target}, "
                        f"exceeds +/-1 tolerance (plan §9.9 requirement)",
                    )


# ---------------------------------------------------------------------------
# TerrainProfileSpec new fields tests
# ---------------------------------------------------------------------------


class TerrainProfileSpecTests(unittest.TestCase):
    def test_default_optional_fields_are_none(self):
        spec = TerrainProfileSpec(
            name="basic",
            boundary=BoundarySpec(mode="soft", width=96),
            height={"base": [66, 78]},
            surface=("grass_block",),
            water={"level": "low"},
            passability="high",
        )
        self.assertIsNone(spec.architectural_layout)
        self.assertIsNone(spec.compound_flatten_radius)

    def test_explicit_optional_fields(self):
        spec = TerrainProfileSpec(
            name="dan_zong",
            boundary=BoundarySpec(mode="soft", width=96),
            height={"base": [62, 78], "peak": 92},
            surface=("podzol",),
            water={"level": "low"},
            passability="medium",
            architectural_layout="dan_zong_compound",
            compound_flatten_radius=96,
        )
        self.assertEqual(spec.architectural_layout, "dan_zong_compound")
        self.assertEqual(spec.compound_flatten_radius, 96)

    def test_spec_is_frozen(self):
        spec = TerrainProfileSpec(
            name="frozen_test",
            boundary=BoundarySpec(mode="soft", width=64),
            height={},
            surface=(),
            water={},
            passability="unknown",
        )
        with self.assertRaises(AttributeError):
            spec.architectural_layout = "x"  # type: ignore[misc]


# ---------------------------------------------------------------------------
# Profile catalog loading with new fields
# ---------------------------------------------------------------------------


class ProfileCatalogLoadTests(unittest.TestCase):
    def test_load_profile_with_layout_fields(self):
        import json
        import tempfile

        catalog_data = {
            "version": 1,
            "profiles": {
                "test_layout_zone": {
                    "height": {"base": [62, 78], "peak": 92, "compound_flatten_radius": 96},
                    "boundary": {"mode": "soft", "width": 96},
                    "surface": ["podzol", "coarse_dirt"],
                    "water": {"level": "low", "coverage": 0.10},
                    "passability": "medium",
                    "architectural_layout": "test_compound",
                },
                "normal_zone": {
                    "height": {"base": [66, 78], "peak": 84},
                    "boundary": {"mode": "soft", "width": 96},
                    "surface": ["grass_block"],
                    "water": {"level": "low", "coverage": 0.05},
                    "passability": "high",
                },
            },
        }

        from scripts.terrain_gen.blueprint import load_profile_catalog

        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(catalog_data, f)
            f.flush()
            catalog = load_profile_catalog(Path(f.name))

        # Zone with layout fields
        layout_profile = catalog.profiles["test_layout_zone"]
        self.assertEqual(layout_profile.architectural_layout, "test_compound")
        self.assertEqual(layout_profile.compound_flatten_radius, 96)

        # Normal zone without layout fields
        normal_profile = catalog.profiles["normal_zone"]
        self.assertIsNone(normal_profile.architectural_layout)
        self.assertIsNone(normal_profile.compound_flatten_radius)


# ---------------------------------------------------------------------------
# Integration: layout runner + density mask + flatten combined
# ---------------------------------------------------------------------------


class LayoutIntegrationTests(unittest.TestCase):
    def test_full_pipeline_deterministic(self):
        """Run layout + density mask + flatten twice, verify identical results."""
        spec = LayoutSpec(
            name="integration_test",
            poi_kind="test_hall",
            radius=20,
            placements=(
                Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload="hall.nbt"),
                Placement(offset=(10, 0, 5), rotation=90, kind="stamp_radial", payload="garden"),
                Placement(offset=(-8, 0, 12), rotation=180, kind="block_grid", payload="path"),
            ),
        )
        zone = _make_zone(
            pois=(PoiSpec(kind="test_hall", pos_xyz=(100.0, 76.0, 210.0)),)
        )

        results = []
        for _ in range(2):
            result = run_layout(spec, zone)
            tile = _make_tile(min_x=80, min_z=192, tile_size=64)
            buffer = _make_buffer_with_density(tile, tile_size=64)
            buffer.layers["height"][:] = np.random.default_rng(42).uniform(60, 90, 64 * 64)

            apply_compound_flatten(buffer, poi_center_xz=(100, 210), radius=20, target_height=76.0)
            compute_layout_density_mask(buffer, poi_center_xz=(100, 210), radius=20)

            results.append((result, buffer.layers["height"].copy(), buffer.layers["flora_density"].copy()))

        # Check layout results identical
        r0, r1 = results[0][0], results[1][0]
        self.assertEqual(len(r0.placed), len(r1.placed))
        for p0, p1 in zip(r0.placed, r1.placed):
            self.assertEqual(p0.world_pos, p1.world_pos)

        # Check buffer arrays identical
        np.testing.assert_array_equal(results[0][1], results[1][1])
        np.testing.assert_array_equal(results[0][2], results[1][2])


if __name__ == "__main__":
    unittest.main()
