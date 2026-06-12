"""worldgen-v4 P3 §6.1 — cross-section render tool tests.

The cross-section PNG is the §6.1 三轮打磨 evidence tool, so its colour mapping
(solid / air / water / surface) and geometry (y_max at the top, one pixel column
per span column) are pinned here — a regression that flips the image upside
down or paints solid as air would silently invalidate every round's evidence.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.fields import SPAN_MIN_Y, ColumnSpans
from scripts.terrain_gen.carvers import CanyonCarver
from scripts.terrain_gen.harness.cross_section import (
    COLOR_AIR,
    COLOR_SOLID,
    COLOR_WATER,
    _column_color_stack,
    render_carved_line,
    render_cross_section,
)


class ColumnColorStackTest(unittest.TestCase):
    def test_solid_below_surface_air_above(self) -> None:
        col = ColumnSpans(((SPAN_MIN_Y, 10),))
        stack = _column_color_stack(col, y_min=0, y_max=20, water_level=None)
        self.assertEqual(stack[5], COLOR_SOLID, "y=5 is inside the solid span")
        self.assertEqual(stack[15], COLOR_AIR, "y=15 is above the surface → air")

    def test_water_fills_below_water_level_where_not_solid(self) -> None:
        col = ColumnSpans(((SPAN_MIN_Y, 5),))
        stack = _column_color_stack(col, y_min=0, y_max=20, water_level=12)
        self.assertEqual(stack[3], COLOR_SOLID, "below the surface stays solid")
        self.assertEqual(stack[10], COLOR_WATER, "air below water level → water")
        self.assertEqual(stack[15], COLOR_AIR, "above water level → air")

    def test_air_gap_in_multi_span_renders_air(self) -> None:
        col = ColumnSpans(((SPAN_MIN_Y, 5), (15, 20)))
        stack = _column_color_stack(col, y_min=0, y_max=25, water_level=None)
        self.assertEqual(stack[3], COLOR_SOLID)
        self.assertEqual(stack[10], COLOR_AIR, "the void between two spans is air")
        self.assertEqual(stack[18], COLOR_SOLID, "the upper span is solid again")


class RenderCrossSectionTest(unittest.TestCase):
    def test_renders_a_png_with_expected_dimensions(self) -> None:
        cols = [ColumnSpans(((SPAN_MIN_Y, 10),)) for _ in range(16)]
        with tempfile.TemporaryDirectory() as tmp:
            out = render_cross_section(
                cols, Path(tmp) / "slice.png",
                y_min=0, y_max=31, pixel_scale=2,
            )
            self.assertTrue(out.exists())
            from PIL import Image

            img = Image.open(out)
            self.assertEqual(
                img.size, (16 * 2, 32 * 2),
                "width = columns*scale, height = (y_max-y_min+1)*scale",
            )

    def test_y_max_is_at_the_top(self) -> None:
        # A column solid only at the bottom must paint the BOTTOM rows solid.
        cols = [ColumnSpans(((0, 2),))]
        with tempfile.TemporaryDirectory() as tmp:
            out = render_cross_section(
                cols, Path(tmp) / "s.png", y_min=0, y_max=20, pixel_scale=1,
            )
            from PIL import Image

            img = Image.open(out).convert("RGB")
            top = img.getpixel((0, 0))
            bottom = img.getpixel((0, 19))
            self.assertEqual(top, COLOR_AIR, "top of image is high Y = air")
            self.assertEqual(
                bottom, COLOR_SOLID, "bottom of image is low Y = solid ground"
            )

    def test_empty_columns_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                render_cross_section([], Path(tmp) / "x.png")

    def test_bad_y_range_rejected(self) -> None:
        cols = [ColumnSpans(((SPAN_MIN_Y, 0),))]
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                render_cross_section(cols, Path(tmp) / "x.png", y_min=10, y_max=5)

    def test_render_carved_line_end_to_end(self) -> None:
        ground = ColumnSpans(((SPAN_MIN_Y, 80),))
        with tempfile.TemporaryDirectory() as tmp:
            out = render_carved_line(
                ground, [CanyonCarver()], Path(tmp) / "canyon.png",
                wz=40, x_start=0, length=64, seed=4242,
                y_min=-10, y_max=110, pixel_scale=1,
            )
            self.assertTrue(out.exists())
            from PIL import Image

            img = Image.open(out)
            self.assertEqual(img.size, (64, 121))


if __name__ == "__main__":
    unittest.main()
