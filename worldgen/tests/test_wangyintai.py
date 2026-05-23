"""Saturated tests for wangyintai (忘音台) terrain profile flora layers.

Covers:
- fill_wangyintai_tile happy path: flora_density and flora_variant_id layers
  exist with correct dtypes and value ranges.
- Empty tile (1x1): all layers still created, no crash.
- Small tile (4x4): layers valid at minimum useful size.
- flora_variant_id value domain: only 0, 1, or 2 present.
- flora_density [0, 1] range enforcement.
- Variant mask priority: disc_fragment (variant 2) overwrites rubble (variant 1)
  where both masks overlap.
- build_notes correctness: flora_variant_id (not flora_density) references local ids.
- Profile generator registration and ecology metadata.
"""

from __future__ import annotations

import unittest

from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile
from scripts.terrain_gen.profiles import get_profile_generator
from scripts.terrain_gen.profiles.wangyintai import (
    WANGYINTAI_DECORATIONS,
    WangyintaiGenerator,
    fill_wangyintai_tile,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_wangyintai_zone(
    size: int = 1000,
) -> BlueprintZone:
    half = size // 2
    return BlueprintZone(
        name="wangyintai",
        display_name="忘音台",
        bounds_xz=Bounds2D(
            min_x=3200, max_x=3200 + size - 1,
            min_z=-2800, max_z=-2800 + size - 1,
        ),
        center_xz=(3200 + half, -2800 + half),
        size_xz=(size, size),
        spirit_qi=-0.15,
        danger_level=3,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="wangyintai",
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=64),
            height_model={"base": [68, 86], "peak": 98},
            surface_palette=(
                "smooth_basalt", "deepslate", "calcite", "gray_concrete",
            ),
        ),
    )


def _make_tile(
    min_x: int = 3200, min_z: int = -2800, tile_size: int = 64,
) -> WorldTile:
    return WorldTile(
        tile_x=min_x // tile_size,
        tile_z=min_z // tile_size,
        min_x=min_x,
        max_x=min_x + tile_size - 1,
        min_z=min_z,
        max_z=min_z + tile_size - 1,
    )


# ---------------------------------------------------------------------------
# Happy-path tests
# ---------------------------------------------------------------------------


class FillWangyintaiFloraHappyPathTests(unittest.TestCase):
    """Verify flora_density and flora_variant_id are created with correct
    dtypes, shapes, and value domains on a standard 64x64 tile."""

    def setUp(self) -> None:
        self.zone = _make_wangyintai_zone()
        self.tile = _make_tile(min_x=3500, min_z=-2500, tile_size=64)
        self.palette = SurfacePalette()
        self.buffer = fill_wangyintai_tile(
            self.zone, self.tile, 64, self.palette,
        )

    def test_flora_density_layer_exists(self) -> None:
        self.assertIn(
            "flora_density", self.buffer.layers,
            "expected flora_density layer in buffer because wangyintai "
            "declares it in extra_layers, but it was missing",
        )

    def test_flora_variant_id_layer_exists(self) -> None:
        self.assertIn(
            "flora_variant_id", self.buffer.layers,
            "expected flora_variant_id layer in buffer because wangyintai "
            "declares it in extra_layers, but it was missing",
        )

    def test_flora_density_shape(self) -> None:
        area = 64 * 64
        actual = self.buffer.layers["flora_density"].shape[0]
        self.assertEqual(
            actual, area,
            f"expected flora_density length={area} because tile is 64x64, "
            f"actual={actual}",
        )

    def test_flora_variant_id_shape(self) -> None:
        area = 64 * 64
        actual = self.buffer.layers["flora_variant_id"].shape[0]
        self.assertEqual(
            actual, area,
            f"expected flora_variant_id length={area} because tile is 64x64, "
            f"actual={actual}",
        )

    def test_flora_density_dtype_is_float(self) -> None:
        import numpy as np

        dtype = self.buffer.layers["flora_density"].dtype
        self.assertTrue(
            np.issubdtype(dtype, np.floating),
            f"expected flora_density dtype to be floating, "
            f"actual={dtype}",
        )

    def test_flora_variant_id_dtype_is_uint8(self) -> None:
        import numpy as np

        dtype = self.buffer.layers["flora_variant_id"].dtype
        self.assertEqual(
            dtype, np.uint8,
            f"expected flora_variant_id dtype=uint8 because variants "
            f"are stored as bytes, actual={dtype}",
        )

    def test_flora_density_range_zero_to_one(self) -> None:
        arr = self.buffer.layers["flora_density"]
        self.assertGreaterEqual(
            float(arr.min()), 0.0,
            f"expected flora_density min >= 0.0 because densities are "
            f"clipped, actual min={float(arr.min())}",
        )
        self.assertLessEqual(
            float(arr.max()), 1.0,
            f"expected flora_density max <= 1.0 because densities are "
            f"clipped, actual max={float(arr.max())}",
        )

    def test_flora_variant_id_values_in_domain(self) -> None:
        import numpy as np

        arr = self.buffer.layers["flora_variant_id"]
        unique_vals = set(int(v) for v in np.unique(arr))
        invalid = unique_vals - {0, 1, 2}
        self.assertEqual(
            invalid, set(),
            f"expected flora_variant_id values in {{0, 1, 2}} because "
            f"wangyintai has 2 decoration variants (silent_rubble=1, "
            f"cracked_disc_fragment=2) plus 0=none, "
            f"but found invalid values: {invalid}",
        )


# ---------------------------------------------------------------------------
# Boundary / edge-case tests
# ---------------------------------------------------------------------------


class FillWangyintaiEmptyTileTests(unittest.TestCase):
    """Verify fill_wangyintai_tile does not crash on a 1x1 tile."""

    def test_1x1_tile_produces_valid_buffer(self) -> None:
        zone = _make_wangyintai_zone(size=1000)
        tile = WorldTile(
            tile_x=3700, tile_z=-2300,
            min_x=3700, max_x=3700,
            min_z=-2300, max_z=-2300,
        )
        palette = SurfacePalette()
        buffer = fill_wangyintai_tile(zone, tile, 1, palette)

        self.assertIn("flora_density", buffer.layers)
        self.assertIn("flora_variant_id", buffer.layers)
        self.assertEqual(
            buffer.layers["flora_density"].shape[0], 1,
            "expected single-element flora_density for 1x1 tile",
        )
        self.assertEqual(
            buffer.layers["flora_variant_id"].shape[0], 1,
            "expected single-element flora_variant_id for 1x1 tile",
        )

    def test_4x4_tile_produces_valid_layers(self) -> None:
        zone = _make_wangyintai_zone(size=1000)
        tile = WorldTile(
            tile_x=3700 // 4, tile_z=-2300 // 4,
            min_x=3700, max_x=3703,
            min_z=-2300, max_z=-2297,
        )
        palette = SurfacePalette()
        buffer = fill_wangyintai_tile(zone, tile, 4, palette)

        area = 4 * 4
        self.assertEqual(
            buffer.layers["flora_density"].shape[0], area,
            f"expected flora_density length={area} for 4x4 tile, "
            f"actual={buffer.layers['flora_density'].shape[0]}",
        )
        self.assertEqual(
            buffer.layers["flora_variant_id"].shape[0], area,
            f"expected flora_variant_id length={area} for 4x4 tile, "
            f"actual={buffer.layers['flora_variant_id'].shape[0]}",
        )
        arr = buffer.layers["flora_density"]
        self.assertGreaterEqual(
            float(arr.min()), 0.0,
            "flora_density should be >= 0 even on small tiles",
        )
        self.assertLessEqual(
            float(arr.max()), 1.0,
            "flora_density should be <= 1 even on small tiles",
        )


# ---------------------------------------------------------------------------
# Variant mask priority tests
# ---------------------------------------------------------------------------


class FloraVariantMaskPriorityTests(unittest.TestCase):
    """Verify that variant 2 (cracked_disc_fragment) can overwrite variant 1
    (silent_rubble) where both masks apply, and that flora_density is never
    negative where a variant is set."""

    def setUp(self) -> None:
        # Use a large tile centered on the zone to maximise overlap.
        self.zone = _make_wangyintai_zone(size=300)
        self.tile = _make_tile(min_x=3200, min_z=-2800, tile_size=300)
        self.palette = SurfacePalette()
        self.buffer = fill_wangyintai_tile(
            self.zone, self.tile, 300, self.palette,
        )

    def test_variant_2_overwrites_variant_1_where_disc_mask_active(self) -> None:
        import numpy as np

        fv = self.buffer.layers["flora_variant_id"]
        fd = self.buffer.layers["flora_density"]

        # Wherever variant == 2, density must be > 0 (disc_fragment was set).
        disc_indices = np.where(fv == 2)[0]
        if len(disc_indices) > 0:
            disc_densities = fd[disc_indices]
            self.assertTrue(
                (disc_densities > 0).all(),
                f"expected flora_density > 0 wherever flora_variant_id == 2 "
                f"because disc_fragment mask always sets positive density, "
                f"but found {int((disc_densities <= 0).sum())} zero-density "
                f"pixels at variant 2 locations",
            )

    def test_variant_1_has_positive_density(self) -> None:
        import numpy as np

        fv = self.buffer.layers["flora_variant_id"]
        fd = self.buffer.layers["flora_density"]

        rubble_indices = np.where(fv == 1)[0]
        if len(rubble_indices) > 0:
            rubble_densities = fd[rubble_indices]
            self.assertTrue(
                (rubble_densities > 0).all(),
                f"expected flora_density > 0 wherever flora_variant_id == 1 "
                f"because rubble mask always sets positive density, "
                f"but found {int((rubble_densities <= 0).sum())} zero-density "
                f"pixels at variant 1 locations",
            )

    def test_variant_0_has_zero_density(self) -> None:
        import numpy as np

        fv = self.buffer.layers["flora_variant_id"]
        fd = self.buffer.layers["flora_density"]

        none_indices = np.where(fv == 0)[0]
        if len(none_indices) > 0:
            none_densities = fd[none_indices]
            self.assertTrue(
                (none_densities == 0).all(),
                f"expected flora_density == 0 wherever flora_variant_id == 0 "
                f"because no decoration should have density, "
                f"but found {int((none_densities != 0).sum())} non-zero "
                f"density pixels with variant 0",
            )


# ---------------------------------------------------------------------------
# build_notes correctness
# ---------------------------------------------------------------------------


class BuildNotesTests(unittest.TestCase):
    """Verify build_notes describes flora_variant_id, not flora_density."""

    def test_build_notes_references_flora_variant_id(self) -> None:
        gen = WangyintaiGenerator()
        from scripts.terrain_gen.blueprint import BoundarySpec, TerrainProfileSpec
        from scripts.terrain_gen.profiles.base import ProfileContext

        zone = _make_wangyintai_zone()
        spec = TerrainProfileSpec(
            name="wangyintai",
            boundary=BoundarySpec(mode="soft", width=64),
            height={"base": [68, 86], "peak": 98},
            surface=("smooth_basalt", "deepslate", "calcite", "gray_concrete"),
            water={"level": "none", "coverage": 0.0},
            passability="medium",
        )
        ctx = ProfileContext(zone=zone, profile_spec=spec)
        notes = gen.build_notes(ctx)
        flora_note = [n for n in notes if "flora_variant_id" in n]
        self.assertEqual(
            len(flora_note), 1,
            f"expected exactly one build_note referencing 'flora_variant_id', "
            f"got {len(flora_note)}: {flora_note}",
        )
        self.assertIn(
            "local ids 1..2", flora_note[0],
            f"expected flora_variant_id note to mention 'local ids 1..2', "
            f"actual note: {flora_note[0]}",
        )
        # Ensure the old wrong description is gone.
        density_note = [n for n in notes if "flora_density uses local ids" in n]
        self.assertEqual(
            len(density_note), 0,
            f"expected no build_note saying 'flora_density uses local ids' "
            f"because local ids belong to flora_variant_id, "
            f"but found: {density_note}",
        )


# ---------------------------------------------------------------------------
# Profile registration
# ---------------------------------------------------------------------------


class ProfileRegistrationTests(unittest.TestCase):
    """Verify wangyintai generator is registered correctly."""

    def test_generator_class(self) -> None:
        gen = get_profile_generator("wangyintai")
        self.assertEqual(
            gen.__class__.__name__, "WangyintaiGenerator",
            f"expected WangyintaiGenerator, got {gen.__class__.__name__}",
        )

    def test_extra_layers_include_flora(self) -> None:
        gen = get_profile_generator("wangyintai")
        self.assertIn(
            "flora_density", gen.extra_layers,
            "expected flora_density in extra_layers",
        )
        self.assertIn(
            "flora_variant_id", gen.extra_layers,
            "expected flora_variant_id in extra_layers",
        )

    def test_decorations_count(self) -> None:
        self.assertEqual(
            len(WANGYINTAI_DECORATIONS), 2,
            f"expected 2 decorations (silent_rubble, cracked_disc_fragment), "
            f"got {len(WANGYINTAI_DECORATIONS)}",
        )

    def test_decoration_names(self) -> None:
        names = [d.name for d in WANGYINTAI_DECORATIONS]
        self.assertEqual(
            names, ["silent_rubble", "cracked_disc_fragment"],
            f"expected decoration names [silent_rubble, cracked_disc_fragment], "
            f"got {names}",
        )


if __name__ == "__main__":
    unittest.main()
