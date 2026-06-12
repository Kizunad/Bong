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

    def test_budget_never_lifts_surface_ceiling_by_merging_upward(self) -> None:
        # worldgen-v4 P3 §8.1 #2 — surface-anchoring regression guard.
        # An overhang-dense column (>MAX_SPANS runs) with the *thinnest* air gap
        # sitting just ABOVE the surface run.  The naive "merge thinnest gap"
        # budget would swallow the surface run upward, lifting span[0].ceiling
        # off the real ground (e.g. surface 100 → 110, or worse, drop the merged
        # run below the surface candidate set so query_surface lands at 20).
        # The surface-aware budget must protect that gap and keep query_surface
        # pinned to surface_y.
        runs = [
            (0, 10),      # deep floor remnant
            (12, 20),     # gap=1 below surface (safe to merge — keeps ceiling)
            (95, 100),    # SURFACE run (ceiling == surface_y)
            (102, 110),   # gap=1 ABOVE surface (must NOT merge into surface)
            (200, 210),   # floating isle
            (300, 310),   # floating isle
        ]
        spans = _runs_to_spans(runs, surface_y=100)
        self.assertLessEqual(spans.count, MAX_SPANS)
        assert_legal_spans(self, spans, "overhang-dense >4-run column")
        self.assertEqual(
            spans.surface_ceiling_y, 100,
            "query_surface must stay on the real ground (100): the budget must "
            "not merge the surface run upward across the thin gap above it; "
            f"got {spans.surface_ceiling_y} from spans {spans.spans}",
        )

    def test_budget_may_merge_surface_run_downward_keeping_ceiling(self) -> None:
        # The complement: when the thin gap is *below* the surface run, merging
        # is allowed because it preserves the surface ceiling — only an air gap
        # underground is lost.  query_surface stays pinned.
        runs = [
            (80, 88),     # floor remnant just below surface
            (90, 100),    # SURFACE run (ceiling == surface_y), gap=1 below
            (200, 210),
            (300, 310),
            (400, 410),
        ]
        spans = _runs_to_spans(runs, surface_y=100)
        self.assertLessEqual(spans.count, MAX_SPANS)
        assert_legal_spans(self, spans, "merge-below-surface column")
        self.assertEqual(
            spans.surface_ceiling_y, 100,
            "merging the surface run with a run *below* it keeps the ceiling at "
            f"surface_y=100; got {spans.surface_ceiling_y} from {spans.spans}",
        )

    def test_budget_surface_anchored_when_runs_all_above_lowest_surface(self) -> None:
        # Degenerate: surface run is the lowest and every other run floats above
        # it, more than MAX_SPANS total.  The gap above the surface is the *only*
        # one touching it; the budget merges among the upper runs instead, so the
        # surface run survives intact and query_surface stays pinned.
        runs = [
            (0, 10),      # SURFACE (lowest), surface_y == 10
            (50, 60),
            (100, 110),
            (150, 160),
            (200, 210),
            (250, 260),
        ]
        spans = _runs_to_spans(runs, surface_y=10)
        self.assertLessEqual(spans.count, MAX_SPANS)
        assert_legal_spans(self, spans, "surface-lowest dense column")
        self.assertEqual(
            spans.surface_ceiling_y, 10,
            "surface run is lowest; the budget must merge upper runs and keep "
            f"the ground span (0,10) intact; got {spans.spans}",
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


class CanyonWallDepthBandTest(unittest.TestCase):
    """worldgen-v4 P3 §6.1 round 3 — overhangs gate on the valley wall band.

    rift_valley sets ``wall_depth_band`` so the overhang shelf only appears on
    the valley *wall* (a surface between floor and rim), never the flat plateau
    top or the valley floor.  These pin that gate: a column whose surface sits
    outside the band is left solid no matter how strongly the wall noise fires.
    """

    def test_surface_above_band_is_left_solid(self) -> None:
        # A high plateau column (surface 200) with a band capping at 100 must not
        # overhang — the whole point of the round-3 fix.
        carver = CanyonCarver(wall_threshold=0.0, wall_depth_band=(40, 100))
        high = ColumnSpans(((SPAN_MIN_Y, 200),))
        for wx in range(0, 80):
            out = carver.carve_column(high, wx, 17, 4242)
            self.assertEqual(
                out.count, 1,
                f"plateau column (surface 200 > band hi 100) must stay solid at "
                f"wx={wx}; got {out.spans}",
            )

    def test_surface_in_band_can_overhang(self) -> None:
        # A wall column (surface 70, inside [40,100]) with permissive noise must
        # be able to overhang somewhere across the line.
        carver = CanyonCarver(wall_threshold=0.0, wall_depth_band=(40, 100))
        wall = ColumnSpans(((SPAN_MIN_Y, 70),))
        overhung = any(
            carver.carve_column(wall, wx, 17, 4242).count >= 2
            for wx in range(0, 300)
        )
        self.assertTrue(
            overhung,
            "a wall column inside the depth band must overhang somewhere "
            "(threshold=0 lets noise fire freely)",
        )

    def test_band_none_is_unrestricted(self) -> None:
        # Default (no band) keeps the original standalone behavior: overhangs the
        # high plateau column too (so segment-1 tests stay valid).
        carver = CanyonCarver(wall_threshold=0.0, wall_depth_band=None)
        high = ColumnSpans(((SPAN_MIN_Y, 200),))
        overhung = any(
            carver.carve_column(high, wx, 17, 4242).count >= 2
            for wx in range(0, 300)
        )
        self.assertTrue(
            overhung,
            "with no depth band the carver overhangs any surface (unrestricted)",
        )

    def test_band_order_independent(self) -> None:
        # (100, 40) is normalized to (40, 100): same gate either way.
        a = CanyonCarver(wall_depth_band=(40, 100)).wall_depth_band
        b = CanyonCarver(wall_depth_band=(100, 40)).wall_depth_band
        self.assertEqual(a, b, "wall_depth_band must normalize to (lo, hi)")


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


class FloatingIslandDripTest(unittest.TestCase):
    """worldgen-v4 P3 §6.1 round 2/3 — the stalactite-underside drip params.

    sky_isle gives the carver a tight ``drip_scale`` + a ``drip_sharpness`` > 1
    so the island bottom hangs as irregular 钟乳 points.  These pin that the
    drip params actually change the underside (a regression that ignored them
    would re-flatten the bottom).
    """

    def test_drip_sharpness_lowers_average_drip(self) -> None:
        # A sharpness > 1 on the [0,1] drip field pushes most columns toward
        # *less* drip (small values shrink under a power > 1), so the average
        # island floor sits HIGHER than with linear drip.  Compare mean isle
        # floor across a field for sharpness 1.0 vs 2.5.
        def mean_floor(sharpness: float) -> float:
            carver = FloatingIslandCarver(
                altitude=250, silhouette_threshold=0.0, drip_depth=20,
                drip_scale=20.0, drip_sharpness=sharpness,
            )
            floors = []
            for wx in range(0, 120):
                col = carver.carve_column(GROUND, wx, 9, 4242)
                isles = [s for s in col.spans if s[0] > 150]
                floors.extend(s[0] for s in isles)
            return sum(floors) / len(floors) if floors else 0.0

        linear = mean_floor(1.0)
        sharp = mean_floor(2.5)
        self.assertGreater(
            sharp, linear,
            f"sharpness 2.5 must raise the mean isle floor vs linear "
            f"({sharp:.1f} should be > {linear:.1f}): a >1 exponent biases most "
            f"columns to hang less, so only spiky drips reach deep",
        )

    def test_drip_params_keep_columns_legal(self) -> None:
        carver = FloatingIslandCarver(
            altitude=260, silhouette_threshold=0.0, drip_depth=24,
            drip_scale=14.0, drip_sharpness=2.2,
        )
        for wx in range(0, 200, 3):
            for wz in range(0, 80, 7):
                out = carver.carve_column(GROUND, wx, wz, 4242)
                assert_legal_spans(self, out, f"drip@({wx},{wz})")

    def test_drip_is_deterministic(self) -> None:
        carver = FloatingIslandCarver(drip_scale=18.0, drip_sharpness=1.9)
        for wx in range(0, 100, 5):
            a = carver.carve_column(GROUND, wx, 11, 7).spans
            b = carver.carve_column(GROUND, wx, 11, 7).spans
            self.assertEqual(a, b, f"drip carve must be deterministic at wx={wx}")


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
        # 2x2 square tile (the shape contract holds even for an empty chain).
        columns = [GROUND, GROUND, GROUND, GROUND]
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

    def test_non_square_columns_rejected(self) -> None:
        # CodeRabbit base.py:381 — a column count that is not tile_size**2 would
        # mislabel every (wx, wz) via idx % / // tile_size, sampling the carve
        # noise at the wrong world coordinate.  Fail fast instead.
        columns = [GROUND for _ in range(15)]  # 15 != 4*4
        with self.assertRaises(ValueError) as ctx:
            apply_carver_chain(
                columns, [CanyonCarver()],
                origin_x=0, origin_z=0, tile_size=4, seed=1,
            )
        self.assertIn("16", str(ctx.exception), "error must state the expected count")

    def test_non_square_rejected_even_with_empty_chain(self) -> None:
        # The shape contract is independent of whether any carver runs.
        with self.assertRaises(ValueError):
            apply_carver_chain(
                [GROUND, GROUND, GROUND],  # 3 columns, not 2*2
                [], origin_x=0, origin_z=0, tile_size=2, seed=1,
            )

    def test_nonpositive_tile_size_rejected(self) -> None:
        with self.assertRaises(ValueError):
            apply_carver_chain(
                [], [CanyonCarver()], origin_x=0, origin_z=0, tile_size=0, seed=1,
            )

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


class CarveTileEquivalenceTest(unittest.TestCase):
    """worldgen-v4 P3 — vectorized carve_tile MUST equal the scalar carve_column.

    ``apply_carver_chain`` uses each carver's vectorized ``carve_tile`` for speed
    (gate/noise computed once over the tile).  This is only valid if it produces
    *byte-identical* output to looping ``carve_column`` per column — otherwise the
    fast export would diverge from the carve contract the rest of P3 tests lock.
    These pin that equivalence over a real-ish field for every production carver,
    including a multi-span ground column (cave / canyon split inputs).
    """

    GROUND_LO = ColumnSpans(((SPAN_MIN_Y, 70),))  # wall-band surface for canyon
    GROUND_HI = ColumnSpans(((SPAN_MIN_Y, 250),))  # high plateau

    def _assert_chain_equiv(self, carver, base_columns, tile_size, origin):
        ox, oz = origin
        fast = apply_carver_chain(
            list(base_columns), [carver],
            origin_x=ox, origin_z=oz, tile_size=tile_size, seed=4242,
        )
        for i, base in enumerate(base_columns):
            wx = ox + (i % tile_size)
            wz = oz + (i // tile_size)
            ref = carver.carve_column(base, wx, wz, 4242)
            self.assertEqual(
                ref.spans, fast[i].spans,
                f"{carver.name} carve_tile diverged from carve_column at "
                f"col {i} (wx={wx},wz={wz}): scalar {ref.spans} != "
                f"vectorized {fast[i].spans}",
            )

    def test_canyon_tile_equals_scalar(self) -> None:
        ts = 24
        cols = [self.GROUND_LO for _ in range(ts * ts)]
        self._assert_chain_equiv(
            CanyonCarver(wall_threshold=0.3, wall_depth_band=(50, 90)),
            cols, ts, (1000, 2000),
        )

    def test_floating_island_tile_equals_scalar(self) -> None:
        ts = 24
        cols = [self.GROUND_HI for _ in range(ts * ts)]
        self._assert_chain_equiv(
            FloatingIslandCarver(silhouette_threshold=0.45),
            cols, ts, (500, 700),
        )

    def test_cave_network_tile_equals_scalar(self) -> None:
        ts = 20
        cols = [self.GROUND_HI for _ in range(ts * ts)]
        self._assert_chain_equiv(
            CaveNetworkCarver(layers=3, octaves=2),
            cols, ts, (333, 888),
        )

    def test_cave_network_coarse_lattice_mixed_surface_equals_scalar(self) -> None:
        # worldgen-v4 P3 perf: the cave carver samples density on a coarse
        # absolute world lattice — y_step strides Y, xz_step strides X/Z — and
        # trilinearly interpolates.  A REAL cave zone has an undulating surface,
        # so columns have DIFFERENT carve tops: the vectorized path builds the
        # grid to the GLOBAL top and slices each column, while the scalar path
        # anchors on each column's OWN top and on its own single (x, z) lattice
        # bracket.  If any axis lattice were keyed on a tile-relative index (or a
        # per-column top) instead of absolute world coords, the interpolation
        # would diverge.  This pins the absolute-anchor equivalence across a
        # staggered surface for every production (y_step, xz_step) combination.
        ts = 18
        rng = __import__("random").Random(20240613)
        cols = [
            ColumnSpans(((SPAN_MIN_Y, 60 + rng.randint(0, 40)),))
            for _ in range(ts * ts)
        ]
        for y_step, xz_step in [(1, 1), (4, 1), (1, 3), (4, 3), (3, 3), (5, 2)]:
            self._assert_chain_equiv(
                CaveNetworkCarver(
                    layers=3, octaves=1, y_step=y_step, xz_step=xz_step
                ),
                cols, ts, (333, 888),
            )

    def test_chain_of_all_three_equals_scalar(self) -> None:
        # Composed chain: each carver's tile path must equal scalar even when
        # the previous carver already split the column (the divergence-prone case).
        ts = 16
        cols = [self.GROUND_HI for _ in range(ts * ts)]
        chain = [
            CanyonCarver(wall_threshold=0.3),
            FloatingIslandCarver(silhouette_threshold=0.5),
            CaveNetworkCarver(layers=2, octaves=2),
        ]
        ox, oz = 1234, 5678
        fast = apply_carver_chain(
            list(cols), chain, origin_x=ox, origin_z=oz, tile_size=ts, seed=99,
        )
        for i, base in enumerate(cols):
            wx = ox + (i % ts)
            wz = oz + (i // ts)
            ref = base
            for c in chain:
                ref = c.carve_column(ref, wx, wz, 99)
            self.assertEqual(
                ref.spans, fast[i].spans,
                f"3-carver chain diverged at col {i}: {ref.spans} != {fast[i].spans}",
            )

    def test_empty_void_columns_tile_equals_scalar(self) -> None:
        # CodeRabbit test_carvers.py:709 — the equivalence suite only used solid
        # GROUND columns, so a void column ``ColumnSpans(())`` (surface_ceiling_y
        # is None) never exercised the vectorized gate's None-surface branch.  A
        # void column must carve to a void column on BOTH paths (no surface ⇒
        # nothing to overhang, no ground ⇒ isle-air-gap guard skips it, cave has
        # no solid to remove).  Pin that the tile path agrees with the scalar
        # path over an all-void tile for every production carver.
        ts = 12
        void = ColumnSpans(())
        cols = [void for _ in range(ts * ts)]
        for carver in (
            CanyonCarver(wall_threshold=0.3),
            FloatingIslandCarver(silhouette_threshold=0.45),
            CaveNetworkCarver(layers=2, octaves=2),
        ):
            with self.subTest(carver=carver.name):
                self._assert_chain_equiv(carver, cols, ts, (700, 900))

    def test_mixed_void_and_solid_columns_tile_equals_scalar(self) -> None:
        # The realistic case: a tile where some columns are void (sky over an
        # abyss) interleaved with solid ground.  The vectorized gate must skip
        # void columns exactly as the scalar loop's per-column guards do, with
        # no index drift between the two interleaved kinds.
        ts = 14
        void = ColumnSpans(())
        cols = [
            (void if (i % 3 == 0) else self.GROUND_HI)
            for i in range(ts * ts)
        ]
        for carver in (
            CanyonCarver(wall_threshold=0.3),
            FloatingIslandCarver(silhouette_threshold=0.45),
            CaveNetworkCarver(layers=2, octaves=2),
        ):
            with self.subTest(carver=carver.name):
                self._assert_chain_equiv(carver, cols, ts, (123, 456))


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
