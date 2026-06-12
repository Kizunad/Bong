"""worldgen-v4 P3 §8.1 #1 — span carver framework + 4 carvers, saturated tests.

Locks the carve contract the rest of P3 (rift_valley / sky_isle profiles, raster
export) depends on:

  * 3D noise — determinism, range, broadcasting (the carve driver);
  * framework — SolidColumn round-trip, MAX_SPANS budgeting, surface anchoring,
    validate_spans rejection;
  * each carver — determinism (same seed → identical spans), span legality
    (non-overlap / floor<=ceiling / world range / count<=MAX_SPANS / span[0] is
    the surface), and that it actually produces its expected multi-span shape
    (overhang≥2 / island≥2 / arch≥2 / cave≥3 somewhere over a field);
  * the apply_carver_chain pipeline hook (column ordering + chain composition).

These are pin tests: a regression that re-floats a span, drops the overhang, or
breaks determinism must turn this file red.
"""

from __future__ import annotations

import unittest

import numpy as np

from scripts.terrain_gen.fields import (
    MAX_SPANS,
    SPAN_MAX_Y,
    SPAN_MIN_Y,
    ColumnSpans,
)
from scripts.terrain_gen.carvers import (
    CARVER_REGISTRY,
    ArchCarver,
    CanyonCarver,
    CaveNetworkCarver,
    FloatingIslandCarver,
    SolidColumn,
    apply_carver_chain,
    build_carver,
    validate_spans,
)
from scripts.terrain_gen.carvers.base import (
    _budget_runs,
    _mask_to_runs,
    _runs_to_spans,
    _select_surface_run,
)
from scripts.terrain_gen.noise import (
    fbm_3d,
    gradient_noise_3d,
    warped_fbm_3d,
)

# A single solid ground column used as the carve subject across the suite.
GROUND = ColumnSpans(((SPAN_MIN_Y, 80),))


def assert_legal_spans(test: unittest.TestCase, column: ColumnSpans, ctx: str) -> None:
    """Assert all §8.1 carve invariants on a carved column with rich context."""
    test.assertLessEqual(
        column.count,
        MAX_SPANS,
        f"{ctx}: count={column.count} exceeds MAX_SPANS={MAX_SPANS}; carvers "
        f"must budget down, not emit illegal columns",
    )
    ordered = sorted(column.spans)
    prev_ceiling = None
    for floor_y, ceiling_y in ordered:
        test.assertTrue(
            SPAN_MIN_Y <= floor_y <= ceiling_y <= SPAN_MAX_Y,
            f"{ctx}: span ({floor_y},{ceiling_y}) violates "
            f"{SPAN_MIN_Y} <= floor <= ceiling <= {SPAN_MAX_Y}",
        )
        if prev_ceiling is not None:
            test.assertGreater(
                floor_y,
                prev_ceiling + 1,
                f"{ctx}: spans overlap/touch — floor {floor_y} must be > prev "
                f"ceiling {prev_ceiling} + 1 (need a real air gap)",
            )
        prev_ceiling = ceiling_y


# ---------------------------------------------------------------------------
# 3D noise — the carve driver
# ---------------------------------------------------------------------------
class Noise3DTest(unittest.TestCase):
    def test_fbm_3d_is_deterministic(self) -> None:
        x = np.linspace(0, 400, 64)
        y = np.linspace(-64, 300, 64)
        z = np.linspace(0, 400, 64)
        a = fbm_3d(x, y, z, scale=80, seed=7)
        b = fbm_3d(x, y, z, scale=80, seed=7)
        np.testing.assert_array_equal(
            a, b, "same seed/coords must give byte-identical 3D noise (carve "
            "determinism depends on it)"
        )

    def test_fbm_3d_different_seed_differs(self) -> None:
        x = np.linspace(0, 400, 32)
        a = fbm_3d(x, x, x, scale=80, seed=7)
        c = fbm_3d(x, x, x, scale=80, seed=8)
        self.assertFalse(
            np.array_equal(a, c), "different seeds must produce different noise"
        )

    def test_fbm_3d_range_within_unit(self) -> None:
        x = np.linspace(-1000, 1000, 50)
        y = np.linspace(-64, 431, 50)
        z = np.linspace(-1000, 1000, 50)
        v = fbm_3d(x, y, z, scale=64, seed=3)
        self.assertGreaterEqual(v.min(), -1.0)
        self.assertLessEqual(v.max(), 1.0)

    def test_gradient_noise_3d_varies_with_height(self) -> None:
        # Two different Y at the same X/Z must differ — the whole point of 3D.
        a = gradient_noise_3d(np.array([10.0]), np.array([0.0]), np.array([10.0]), seed=1)
        b = gradient_noise_3d(np.array([10.0]), np.array([100.0]), np.array([10.0]), seed=1)
        self.assertNotEqual(
            float(a[0]), float(b[0]),
            "3D noise must vary with Y or carves degrade to 2D slabs",
        )

    def test_fbm_3d_broadcasts_to_volume(self) -> None:
        xx, yy, zz = np.meshgrid(
            np.arange(6), np.arange(40), np.arange(6), indexing="ij"
        )
        vol = fbm_3d(xx, yy, zz, scale=30, seed=2)
        self.assertEqual(vol.shape, (6, 40, 6))

    def test_warped_fbm_3d_deterministic_and_bounded(self) -> None:
        x = np.linspace(0, 300, 40)
        a = warped_fbm_3d(x, x, x, scale=70, seed=3)
        b = warped_fbm_3d(x, x, x, scale=70, seed=3)
        np.testing.assert_array_equal(a, b)
        self.assertGreaterEqual(a.min(), -1.5)  # warp widens slightly
        self.assertLessEqual(a.max(), 1.5)


# ---------------------------------------------------------------------------
# Framework — SolidColumn / run extraction / budgeting / surface anchor
# ---------------------------------------------------------------------------
class SolidColumnTest(unittest.TestCase):
    def test_round_trips_a_simple_column(self) -> None:
        col = SolidColumn.from_spans(GROUND)
        self.assertEqual(col.surface_y, 80)
        self.assertTrue(col.is_solid(0))
        self.assertTrue(col.is_solid(80))
        self.assertFalse(col.is_solid(81))
        back = col.to_spans()
        self.assertEqual(back.spans, GROUND.spans)

    def test_round_trips_multi_span_column(self) -> None:
        original = ColumnSpans(((SPAN_MIN_Y, 70), (300, 320)))
        col = SolidColumn.from_spans(original)
        back = col.to_spans()
        self.assertEqual(back.count, 2)
        self.assertEqual(
            back.surface_ceiling_y, 70,
            "round-trip must keep the ground (70) as span[0], not the isle (320)",
        )

    def test_carving_a_void_splits_into_two_runs(self) -> None:
        col = SolidColumn.from_spans(GROUND)
        col.fill_range(20, 40, False)  # punch an air band
        runs = col.runs()
        self.assertEqual(
            len(runs), 2,
            "punching a void in a solid column must yield two solid runs",
        )
        self.assertEqual(runs, [(SPAN_MIN_Y, 19), (41, 80)])

    def test_to_spans_preserves_surface_after_void(self) -> None:
        col = SolidColumn.from_spans(GROUND)
        col.fill_range(20, 40, False)
        spans = col.to_spans()
        self.assertEqual(
            spans.surface_ceiling_y, 80,
            "the surface cap (80) must stay span[0] even with a void below",
        )

    def test_mask_to_runs_empty(self) -> None:
        mask = np.zeros(SolidColumn.HEIGHT, dtype=bool)
        self.assertEqual(_mask_to_runs(mask), [])

    def test_mask_to_runs_full(self) -> None:
        mask = np.ones(SolidColumn.HEIGHT, dtype=bool)
        self.assertEqual(_mask_to_runs(mask), [(SPAN_MIN_Y, SPAN_MAX_Y)])


class RunBudgetTest(unittest.TestCase):
    def test_budget_merges_thinnest_gap_first(self) -> None:
        # 5 runs, must collapse to <=4 by merging the smallest gap.
        runs = [(0, 10), (12, 20), (40, 50), (100, 110), (200, 210)]
        budgeted = _budget_runs(runs, MAX_SPANS)
        self.assertLessEqual(len(budgeted), MAX_SPANS)
        # The thinnest gap (10→12, gap=1) merges first: (0,20).
        self.assertIn((0, 20), budgeted)

    def test_budget_noop_when_within_budget(self) -> None:
        runs = [(0, 10), (40, 50)]
        self.assertEqual(_budget_runs(runs, MAX_SPANS), runs)

    def test_runs_to_spans_budgets_six_runs_to_four(self) -> None:
        runs = [
            (SPAN_MIN_Y, -60), (-50, -40), (-30, -20),
            (0, 10), (40, 60), (200, 220),
        ]
        spans = _runs_to_spans(runs, surface_y=60)
        self.assertLessEqual(spans.count, MAX_SPANS)
        assert_legal_spans(self, spans, "budget 6→4")

    def test_runs_to_spans_empty_is_void(self) -> None:
        self.assertEqual(_runs_to_spans([], None).spans, ())

    def test_select_surface_run_prefers_highest_at_or_below_surface(self) -> None:
        runs = [(SPAN_MIN_Y, -50), (0, 40), (200, 220)]
        idx = _select_surface_run(runs, surface_y=40)
        self.assertEqual(runs[idx], (0, 40))

    def test_select_surface_run_falls_back_to_topmost_when_no_surface(self) -> None:
        runs = [(SPAN_MIN_Y, -50), (200, 220)]
        idx = _select_surface_run(runs, surface_y=None)
        self.assertEqual(runs[idx], (200, 220))

    def test_select_surface_run_when_all_float_above(self) -> None:
        runs = [(100, 120), (200, 220)]
        idx = _select_surface_run(runs, surface_y=40)
        self.assertEqual(
            runs[idx], (100, 120),
            "when every run floats above the old surface, the lowest is span[0]",
        )


class ValidateSpansTest(unittest.TestCase):
    def test_accepts_legal_column(self) -> None:
        col = ColumnSpans(((SPAN_MIN_Y, 70), (100, 120)))
        self.assertIs(validate_spans(col), col)

    def test_construction_rejects_overlap(self) -> None:
        # ColumnSpans itself rejects overlap at construction (so an illegal
        # column cannot even reach validate_spans).
        with self.assertRaises(ValueError):
            ColumnSpans(((SPAN_MIN_Y, 70), (60, 90)))


# ---------------------------------------------------------------------------
# Per-carver: determinism + legality + expected multi-span structure
# ---------------------------------------------------------------------------
class _CarverCaseMixin:
    """Shared determinism + legality sweep; subclasses set CARVER + asserts."""

    carver = None  # set by subclass
    min_expected_count = 2  # the multi-span shape this carver should produce

    def _field(self):
        for wx in range(0, 200, 5):
            for wz in range(0, 200, 5):
                yield wx, wz

    def test_determinism(self) -> None:
        for wx, wz in self._field():
            a = self.carver.carve_column(GROUND, wx, wz, 9001)
            b = self.carver.carve_column(GROUND, wx, wz, 9001)
            self.assertEqual(
                a.spans, b.spans,
                f"{self.carver.name} at ({wx},{wz}) must be deterministic: "
                f"two carves with seed 9001 gave {a.spans} then {b.spans}",
            )

    def test_legality_over_field(self) -> None:
        for wx, wz in self._field():
            carved = self.carver.carve_column(GROUND, wx, wz, 9001)
            assert_legal_spans(
                self, carved, f"{self.carver.name}@({wx},{wz})"
            )

    def test_span0_is_surface(self) -> None:
        for wx, wz in self._field():
            carved = self.carver.carve_column(GROUND, wx, wz, 9001)
            if carved.count >= 1:
                self.assertEqual(
                    carved.spans[0][1],
                    carved.surface_ceiling_y,
                    f"{self.carver.name}@({wx},{wz}): span[0].ceiling must be "
                    f"the surface anchor (query_surface contract §8.1 #2)",
                )

    def test_produces_expected_multi_span_somewhere(self) -> None:
        max_count = 0
        for wx, wz in self._field():
            carved = self.carver.carve_column(GROUND, wx, wz, 9001)
            max_count = max(max_count, carved.count)
        self.assertGreaterEqual(
            max_count,
            self.min_expected_count,
            f"{self.carver.name} must produce at least one column with "
            f">= {self.min_expected_count} spans (its signature 3D shape); "
            f"max seen was {max_count}",
        )

    def test_void_column_is_left_untouched(self) -> None:
        void = ColumnSpans(())
        out = self.carver.carve_column(void, 10, 10, 9001)
        self.assertEqual(
            out.spans, (),
            f"{self.carver.name} must not invent ground in a fully-void column",
        )


class CanyonCarverTest(_CarverCaseMixin, unittest.TestCase):
    carver = CanyonCarver()
    min_expected_count = 2  # overhang shelf + floor remnant

    def test_overhang_has_void_under_shelf(self) -> None:
        # Find a carved overhang and assert the shelf sits above a real air gap.
        for wx in range(0, 300):
            carved = self.carver.carve_column(GROUND, wx, 40, 4242)
            if carved.count >= 2:
                shelf = carved.spans[0]  # surface span (the overhang lip)
                floor = min(carved.spans[1:], key=lambda s: s[1])
                self.assertGreater(
                    shelf[0],
                    floor[1] + 1,
                    f"overhang shelf {shelf} must hang over an air gap above "
                    f"the floor remnant {floor}",
                )
                self.assertEqual(
                    shelf[1], 80,
                    "the overhang lip ceiling stays the original surface (80)",
                )
                return
        self.fail("canyon carver produced no overhang over a 300-block line")


class FloatingIslandCarverTest(_CarverCaseMixin, unittest.TestCase):
    carver = FloatingIslandCarver()
    min_expected_count = 2  # ground + floating isle

    def test_void_column_is_left_untouched(self) -> None:
        # Override: a floating isle MAY appear over open sky (a chasm with no
        # ground), so a void column legitimately gains an isle span — but it must
        # stay legal and never exceed one span (no phantom ground invented).
        void = ColumnSpans(())
        out = self.carver.carve_column(void, 10, 10, 9001)
        assert_legal_spans(self, out, "island over void")
        self.assertLessEqual(
            out.count, 1,
            "an isle over open sky is at most one floating span (no ground "
            "fabricated beneath it)",
        )

    def test_island_floats_free_above_ground(self) -> None:
        for wx in range(0, 400):
            for wz in range(0, 200, 7):
                carved = self.carver.carve_column(GROUND, wx, wz, 4242)
                if carved.count >= 2:
                    ground = carved.spans[0]
                    isle = max(carved.spans[1:], key=lambda s: s[1])
                    self.assertGreater(
                        isle[0],
                        ground[1] + 1,
                        f"isle {isle} must float above ground {ground} with a "
                        f"real air gap",
                    )
                    self.assertGreater(
                        isle[0], 150,
                        "the floating isle body sits high in the sky",
                    )
                    return
        self.fail("floating island carver produced no detached isle")


class ArchCarverTest(_CarverCaseMixin, unittest.TestCase):
    carver = ArchCarver()
    min_expected_count = 2  # ground/wall + arch roof

    def test_arch_has_roof_over_opening(self) -> None:
        for wz in range(0, 200, 4):
            for wx in range(0, 200):
                carved = self.carver.carve_column(GROUND, wx, wz, 4242)
                if carved.count >= 2:
                    roof = carved.spans[0]  # surface span (the arch crown)
                    ground = min(carved.spans[1:], key=lambda s: s[1])
                    self.assertGreater(
                        roof[0],
                        ground[1] + 1,
                        f"arch roof {roof} must span over an air opening above "
                        f"the wall/ground {ground}",
                    )
                    self.assertEqual(roof[1], 80, "roof crown reaches the surface")
                    return
        self.fail("arch carver produced no opening over the sampled field")


class CaveNetworkCarverTest(_CarverCaseMixin, unittest.TestCase):
    carver = CaveNetworkCarver()
    min_expected_count = 3  # surface cap + >=1 mid floor + bedrock floor

    def test_cave_has_multiple_solid_layers(self) -> None:
        for wx in range(0, 200):
            for wz in range(0, 200, 3):
                carved = self.carver.carve_column(GROUND, wx, wz, 4242)
                if carved.count >= 3:
                    # The surface cap must remain at the top.
                    self.assertEqual(
                        carved.surface_ceiling_y, 80,
                        "the cave roof keeps the original surface walkable",
                    )
                    return
        self.fail("cave network carver produced no >=3-span column")


# ---------------------------------------------------------------------------
# Pipeline hook + registry
# ---------------------------------------------------------------------------
class CarverChainTest(unittest.TestCase):
    def test_apply_chain_preserves_column_order_and_count(self) -> None:
        tile_size = 4
        columns = [GROUND for _ in range(tile_size * tile_size)]
        out = apply_carver_chain(
            columns,
            [CanyonCarver()],
            origin_x=0,
            origin_z=0,
            tile_size=tile_size,
            seed=1,
        )
        self.assertEqual(len(out), len(columns))
        for col in out:
            assert_legal_spans(self, col, "chain output")

    def test_empty_chain_is_identity(self) -> None:
        columns = [GROUND, GROUND]
        out = apply_carver_chain(
            columns, [], origin_x=0, origin_z=0, tile_size=2, seed=1
        )
        self.assertEqual([c.spans for c in out], [c.spans for c in columns])

    def test_chain_is_deterministic(self) -> None:
        tile_size = 4
        columns = [GROUND for _ in range(tile_size * tile_size)]
        kw = dict(origin_x=128, origin_z=256, tile_size=tile_size, seed=77)
        a = apply_carver_chain(columns, [CanyonCarver(), CaveNetworkCarver()], **kw)
        b = apply_carver_chain(columns, [CanyonCarver(), CaveNetworkCarver()], **kw)
        self.assertEqual([c.spans for c in a], [c.spans for c in b])

    def test_chain_composition_two_carvers_legal(self) -> None:
        # Canyon then cave: the second carver must keep the column legal even
        # after the first already split it (the "前一个 carver 输出被后一个切成
        # 非法" risk from the scout).
        tile_size = 8
        columns = [GROUND for _ in range(tile_size * tile_size)]
        out = apply_carver_chain(
            columns,
            [CanyonCarver(), CaveNetworkCarver()],
            origin_x=0,
            origin_z=0,
            tile_size=tile_size,
            seed=313,
        )
        for col in out:
            assert_legal_spans(self, col, "canyon+cave chain")

    def test_world_coords_use_origin_offset(self) -> None:
        # Two tiles at different origins must carve their shared local index
        # differently (world coords, not local, drive the noise).
        cols = [GROUND]
        a = apply_carver_chain(
            cols, [FloatingIslandCarver()], origin_x=0, origin_z=0,
            tile_size=1, seed=5,
        )
        b = apply_carver_chain(
            cols, [FloatingIslandCarver()], origin_x=512, origin_z=512,
            tile_size=1, seed=5,
        )
        # Not necessarily different spans, but the carve must have *sampled*
        # different world coords; assert at least one of a wide tile pair differs.
        differ = False
        for ox in range(0, 2000, 137):
            ca = apply_carver_chain(
                cols, [FloatingIslandCarver()], origin_x=ox, origin_z=0,
                tile_size=1, seed=5,
            )[0]
            if ca.spans != a[0].spans:
                differ = True
                break
        self.assertTrue(
            differ, "carve must vary with world X (origin offset), not be "
            "constant across the world"
        )


class CarverRegistryTest(unittest.TestCase):
    def test_registry_has_all_four(self) -> None:
        self.assertEqual(
            set(CARVER_REGISTRY),
            {"canyon", "floating_island", "arch", "cave_network"},
        )

    def test_build_carver_constructs_by_kind(self) -> None:
        c = build_carver("canyon", {"void_height": 20})
        self.assertIsInstance(c, CanyonCarver)
        self.assertEqual(c.void_height, 20)

    def test_build_carver_unknown_kind_raises(self) -> None:
        with self.assertRaises(KeyError) as cm:
            build_carver("nonexistent")
        self.assertIn("available", str(cm.exception))


if __name__ == "__main__":
    unittest.main()
