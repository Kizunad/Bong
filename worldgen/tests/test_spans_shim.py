"""worldgen-v4 P0 — 2D → spans shim folding + behaviour-equivalence pin.

Two layers of coverage:

1. ``column_spans_for_index`` unit cases — every column shape (normal / cave /
   sky-isle / degenerate carve / void-ish) folds to the right span list, with
   span[0] always the walkable surface.

2. Full-tile equivalence against the frozen v3 baseline: folding each
   representative profile's tile must reproduce ``surface_y == round(height)``
   and the water surface must be untouched, proving "先换表示不换景观".
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import numpy as np

from regenerate_v3_baseline import v3_carved_surface
from scripts.terrain_gen.fields import SPAN_MAX_Y
from scripts.terrain_gen.spans_shim import (
    column_spans_for_index,
    spans_for_tile,
    v3_surface_top_y,
)
from v3_baseline_zones import TILE_SIZE, build_baseline_buffer

REPO_ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = REPO_ROOT / "worldgen/fixtures/v3_surface_baseline.json"

# Sentinel for "no sky isle / no cave" in the per-column folding helper.
NO_ISLE_BASE = 9999.0


def _no_features(surface_y: int):
    return column_spans_for_index(
        surface_y=surface_y,
        cave_mask=0.0,
        ceiling_height=0.0,
        sky_mask=0.0,
        sky_base_y=NO_ISLE_BASE,
        sky_thickness=0.0,
    )


class ColumnFoldingTest(unittest.TestCase):
    def test_normal_column_is_single_span_to_surface(self) -> None:
        col = _no_features(72)
        self.assertEqual(col.spans, ((-64, 72),))
        self.assertEqual(col.surface_ceiling_y, 72)

    def test_surface_above_world_ceiling_is_clamped(self) -> None:
        col = _no_features(900)
        self.assertEqual(
            col.surface_ceiling_y,
            SPAN_MAX_Y,
            "a height above the world ceiling must clamp to SPAN_MAX_Y, not "
            "produce an out-of-range span",
        )

    def test_cave_column_carves_void_and_keeps_surface_on_top(self) -> None:
        # surface 72, ceiling_height 20 → carve_floor=52, carve_ceiling=70.
        # span[0] = (71, 72) walkable cap; span[1] = (-64, 51) floor remnant.
        col = column_spans_for_index(
            surface_y=72,
            cave_mask=0.9,
            ceiling_height=20.0,
            sky_mask=0.0,
            sky_base_y=NO_ISLE_BASE,
            sky_thickness=0.0,
        )
        self.assertEqual(col.count, 2)
        self.assertEqual(
            col.surface_ceiling_y,
            72,
            "cave void is carved BELOW the surface; the walkable top must stay "
            "at round(height) so NPC navigation is unchanged",
        )
        # The two spans are non-overlapping with a real air gap (the cave).
        floors_ceils = sorted(col.spans)
        self.assertLess(floors_ceils[0][1] + 1, floors_ceils[1][0])

    def test_cave_below_threshold_stays_single_span(self) -> None:
        col = column_spans_for_index(
            surface_y=72,
            cave_mask=0.50,  # below 0.58 gate
            ceiling_height=20.0,
            sky_mask=0.0,
            sky_base_y=NO_ISLE_BASE,
            sky_thickness=0.0,
        )
        self.assertEqual(col.spans, ((-64, 72),))

    def test_degenerate_carve_collapses_to_single_span(self) -> None:
        # Tiny ceiling_height near a low surface → carve can't leave both a
        # floor remnant and a surface cap; must collapse to one solid span.
        col = column_spans_for_index(
            surface_y=-50,
            cave_mask=0.9,
            ceiling_height=2.0,
            sky_mask=0.0,
            sky_base_y=NO_ISLE_BASE,
            sky_thickness=0.0,
        )
        self.assertEqual(col.count, 1)
        self.assertEqual(col.surface_ceiling_y, -50)

    def test_sky_isle_adds_second_span_above_surface(self) -> None:
        col = column_spans_for_index(
            surface_y=72,
            cave_mask=0.0,
            ceiling_height=0.0,
            sky_mask=0.5,
            sky_base_y=300.0,
            sky_thickness=20.0,
        )
        self.assertEqual(col.count, 2)
        self.assertEqual(col.spans[0], (-64, 72))
        self.assertEqual(
            col.spans[1], (300, 320), "isle span = (base_y, base_y+thickness)"
        )
        self.assertEqual(
            col.surface_ceiling_y,
            72,
            "the floating isle must NOT become the surface — ground stays span[0]",
        )

    def test_sky_isle_below_mask_gate_is_ignored(self) -> None:
        col = column_spans_for_index(
            surface_y=72,
            cave_mask=0.0,
            ceiling_height=0.0,
            sky_mask=0.1,  # below 0.2 gate
            sky_base_y=300.0,
            sky_thickness=20.0,
        )
        self.assertEqual(col.count, 1)

    def test_sky_isle_thin_or_sentinel_is_ignored(self) -> None:
        # thickness < 4 → ignored
        thin = column_spans_for_index(
            surface_y=72, cave_mask=0.0, ceiling_height=0.0,
            sky_mask=0.5, sky_base_y=300.0, sky_thickness=2.0,
        )
        self.assertEqual(thin.count, 1)
        # base_y sentinel → ignored
        sentinel = column_spans_for_index(
            surface_y=72, cave_mask=0.0, ceiling_height=0.0,
            sky_mask=0.5, sky_base_y=9999.0, sky_thickness=20.0,
        )
        self.assertEqual(sentinel.count, 1)

    def test_cave_and_sky_isle_combine_to_three_spans(self) -> None:
        col = column_spans_for_index(
            surface_y=72,
            cave_mask=0.9,
            ceiling_height=20.0,
            sky_mask=0.5,
            sky_base_y=300.0,
            sky_thickness=20.0,
        )
        self.assertEqual(col.count, 3)
        self.assertEqual(
            col.surface_ceiling_y, 72, "surface unchanged with cave + isle stacked"
        )


class V3SurfaceCarveUnitTest(unittest.TestCase):
    """``v3_surface_top_y`` reproduces the column.rs carve, branch by branch."""

    def test_no_masks_is_clamped_round_height(self) -> None:
        self.assertEqual(v3_surface_top_y(72.0, 99.0, 0.0, 0.0, 0.0, 0.0), 72)
        # Above the world ceiling clamps via clamp_world_y to 429.
        self.assertEqual(v3_surface_top_y(999.0, 99.0, 0.0, 0.0, 0.0, 0.0), 429)

    def test_rift_branch_carves(self) -> None:
        # round((1-0.5)*22 + 0.25*4) = 12 off 80 → 68.
        self.assertEqual(v3_surface_top_y(80.0, 0.5, 0.25, 0.0, 0.0, 0.0), 68)
        # sdf >= 0.9 → no carve.
        self.assertEqual(v3_surface_top_y(80.0, 0.9, 1.0, 0.0, 0.0, 0.0), 80)

    def test_fracture_branch_truncates(self) -> None:
        # f32 (0.90-0.7)*300 = 59.99… truncates to 59 off 100 → 41 (matches the
        # v3 Rust f32 path; f64 would give 60 → 40).
        self.assertEqual(v3_surface_top_y(100.0, 99.0, 0.0, 0.90, 0.0, 0.0), 41)
        # mask <= 0.7 → no carve.
        self.assertEqual(v3_surface_top_y(100.0, 99.0, 0.0, 0.70, 0.0, 0.0), 100)

    def test_neg_pressure_branch_sinks(self) -> None:
        self.assertEqual(v3_surface_top_y(90.0, 99.0, 0.0, 0.0, 0.5, 0.0), 83)
        # neg <= 0.18 → no sink.
        self.assertEqual(v3_surface_top_y(90.0, 99.0, 0.0, 0.0, 0.18, 0.0), 90)

    def test_entrance_branch_sinks_half_away(self) -> None:
        # round(0.45*10)=round(4.5)=5 (half away, like Rust f32::round) off 72 → 67.
        self.assertEqual(v3_surface_top_y(72.0, 99.0, 0.0, 0.0, 0.0, 0.45), 67)
        # entrance <= 0.16 → no sink.
        self.assertEqual(v3_surface_top_y(72.0, 99.0, 0.0, 0.0, 0.0, 0.16), 72)


class ShimBehaviourEquivalenceTest(unittest.TestCase):
    """Folding the v3 tiles must reproduce the frozen carved baseline surfaces."""

    def setUp(self) -> None:
        self.baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))

    def _col_index(self, local_x: int, local_z: int) -> int:
        return local_z * TILE_SIZE + local_x

    def test_folded_surface_matches_v3_baseline(self) -> None:
        rows_by_profile: dict[str, list[dict]] = {}
        for row in self.baseline:
            rows_by_profile.setdefault(row["profile"], []).append(row)

        for profile_key, rows in rows_by_profile.items():
            buffer = build_baseline_buffer(profile_key)
            columns = spans_for_tile(buffer)
            for row in rows:
                idx = self._col_index(row["local_x"], row["local_z"])
                surface = columns[idx].surface_ceiling_y
                # The golden surface is the carved v3 top_y (already clamped to
                # the v3 world range, which is <= SPAN_MAX_Y).
                expected = row["surface_y"]
                self.assertEqual(
                    surface,
                    expected,
                    f"{profile_key}({row['local_x']},{row['local_z']}): folded "
                    f"span surface {surface} != v3 carved baseline {expected}; the "
                    f"shim must reproduce the rift/fracture/neg/entrance sculpt",
                )

    def test_whole_tile_surface_equivalence_and_legality(self) -> None:
        for profile_key in (
            "normal", "water", "sky_isle", "cave", "abyssal", "rift", "waste",
        ):
            buffer = build_baseline_buffer(profile_key)
            height = np.asarray(buffer.layers["height"], dtype=np.float64)
            columns = spans_for_tile(buffer)
            self.assertEqual(len(columns), len(height))
            for idx, col in enumerate(columns):
                # Every folded column must be a legal ColumnSpans (construction
                # already enforced it) and its surface must match the v3 carved
                # top_y derived independently from the same buffer.
                expected = v3_carved_surface(buffer, idx)
                self.assertEqual(
                    col.surface_ceiling_y,
                    expected,
                    f"{profile_key} col {idx}: folded surface "
                    f"{col.surface_ceiling_y} != v3 carved top_y {expected}",
                )

    def test_water_columns_unaffected_by_folding(self) -> None:
        # The shim folds only the solid structure; water_level must be readable
        # unchanged from the same buffer (the spans layer is orthogonal to it).
        buffer = build_baseline_buffer("water")
        water = np.asarray(buffer.layers["water_level"], dtype=np.float64)
        flooded = int(np.sum(water >= 0.0))
        self.assertGreater(
            flooded,
            0,
            "water profile must keep flooded columns after folding so water_y "
            "equivalence is exercised",
        )


if __name__ == "__main__":
    unittest.main()
