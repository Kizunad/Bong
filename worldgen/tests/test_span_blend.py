"""worldgen-v4 P0 — stitcher span-blend semantic pin (§8.1 #1 落点).

``blend_spans`` merges a wilderness base column with a zone overlay column by
boundary weight.  These cases lock the contract:
  * surface ceiling lerps between base and overlay by weight (0 / 0.5 / 1);
  * overlay-only features (isles / cave floors) appear only at weight >= 0.5;
  * void base adopts the overlay only once mostly inside the zone;
  * the result is always a legal ColumnSpans (delegated to ColumnSpans ctor).
Also verifies the FOLD patch-layer blend modes still match the v3 modes so the
fold across boundaries stays landscape-faithful.
"""

from __future__ import annotations

import unittest

from scripts.terrain_gen.fields import ColumnSpans
from scripts.terrain_gen.stitcher import FOLD_PATCH_BLEND_MODES, blend_spans


class BlendSpansTest(unittest.TestCase):
    def test_surface_lerps_with_weight(self) -> None:
        base = ((-64, 60),)
        overlay = ((-64, 80),)
        self.assertEqual(blend_spans(base, overlay, 0.0), ((-64, 60),))
        self.assertEqual(blend_spans(base, overlay, 1.0), ((-64, 80),))
        self.assertEqual(
            blend_spans(base, overlay, 0.5),
            ((-64, 70),),
            "weight 0.5 must land the surface exactly halfway (60..80 → 70)",
        )

    def test_weight_clamped_outside_unit_range(self) -> None:
        base = ((-64, 60),)
        overlay = ((-64, 80),)
        self.assertEqual(blend_spans(base, overlay, 5.0), ((-64, 80),))
        self.assertEqual(blend_spans(base, overlay, -5.0), ((-64, 60),))

    def test_overlay_isle_appears_only_above_half_weight(self) -> None:
        base = ((-64, 70),)
        overlay = ((-64, 72), (300, 320))
        high = blend_spans(base, overlay, 0.8)
        self.assertEqual(high, ((-64, 72), (300, 320)), "isle present inside zone")
        low = blend_spans(base, overlay, 0.3)
        self.assertEqual(
            len(low), 1, "isle must fade out near the wilderness boundary"
        )

    def test_void_base_adopts_overlay_only_when_mostly_inside(self) -> None:
        overlay = ((-64, 72),)
        self.assertEqual(blend_spans((), overlay, 0.6), ((-64, 72),))
        self.assertEqual(
            blend_spans((), overlay, 0.4),
            (),
            "below 0.5 a void wilderness column stays void",
        )

    def test_empty_overlay_returns_base_untouched(self) -> None:
        base = ((-64, 65),)
        self.assertEqual(blend_spans(base, (), 0.7), base)

    def test_result_is_always_legal_column_spans(self) -> None:
        # A pathological overlay extra span that collides with the blended
        # ground must be dropped, not yield an illegal (overlapping) column.
        base = ((-64, 70),)
        overlay = ((-64, 90), (40, 60))  # 40..60 overlaps a ground up to ~90
        result = blend_spans(base, overlay, 1.0)
        # Constructing ColumnSpans must not raise (legality guaranteed).
        ColumnSpans(result)
        self.assertEqual(result[0][1], 90)

    def test_cave_floor_remnant_from_overlay_survives_inside_zone(self) -> None:
        # overlay is a carved cave: span[0] surface cap, span[1] floor remnant.
        base = ((-64, 70),)
        overlay = ((68, 72), (-64, 40))
        result = blend_spans(base, overlay, 1.0)
        self.assertIn((-64, 40), result, "deep cave floor remnant kept inside zone")
        ColumnSpans(result)


class FoldPatchBlendModeTest(unittest.TestCase):
    def test_coordinate_layers_keep_minimum_blend(self) -> None:
        # sky_island_base_y / cavern_floor_y are coordinate-valued with sentinel
        # 9999; a maximum blend would corrupt them across boundaries.
        self.assertEqual(FOLD_PATCH_BLEND_MODES["sky_island_base_y"], "minimum")
        self.assertEqual(FOLD_PATCH_BLEND_MODES["cavern_floor_y"], "minimum")

    def test_mask_layers_keep_maximum_blend(self) -> None:
        for layer in ("cave_mask", "ceiling_height", "entrance_mask",
                      "sky_island_thickness"):
            self.assertEqual(
                FOLD_PATCH_BLEND_MODES[layer],
                "maximum",
                f"{layer} must blend by maximum (v3 behaviour)",
            )


if __name__ == "__main__":
    unittest.main()
