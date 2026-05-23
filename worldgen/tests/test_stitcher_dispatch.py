"""Tests for _build_zone_overlay_tile dispatch and _remap_flora_variant_to_global.

Covers the newly wired dan_zong_yi_yuan and wangyintai profiles to ensure they
no longer fall through to the ``else: return None`` branch, and that
flora_variant_id remapping produces correct global ids.
"""

from __future__ import annotations

import unittest

import numpy as np

from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile
from scripts.terrain_gen.profiles import PROFILE_DECORATION_OFFSETS
from scripts.terrain_gen.stitcher import (
    _build_zone_overlay_tile,
    _remap_flora_variant_to_global,
)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_zone(profile: str) -> BlueprintZone:
    """Create a minimal BlueprintZone for the given profile name."""
    configs = {
        "dan_zong_yi_yuan": dict(
            name="dan_zong_yi_yuan",
            display_name="丹宗遗园",
            bounds_xz=Bounds2D(min_x=-2400, max_x=-800, min_z=3200, max_z=4800),
            center_xz=(-1600, 4000),
            size_xz=(1600, 1600),
            spirit_qi=0.40,
            danger_level=4,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="dan_zong_yi_yuan",
                shape="ellipse",
                boundary=BoundarySpec(mode="soft", width=96),
                height_model={
                    "base": [62, 78],
                    "peak": 92,
                    "compound_flatten_radius": 96,
                },
                surface_palette=(
                    "podzol", "coarse_dirt", "mud",
                    "purple_terracotta", "mossy_cobblestone",
                ),
            ),
        ),
        "wangyintai": dict(
            name="wangyintai",
            display_name="忘音台",
            bounds_xz=Bounds2D(min_x=3200, max_x=4199, min_z=-2800, max_z=-1801),
            center_xz=(3700, -2300),
            size_xz=(1000, 1000),
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
        ),
    }
    return BlueprintZone(**configs[profile])


def _make_tile_for(profile: str, tile_size: int = 64) -> WorldTile:
    """Create a tile at the centre of the zone for the given profile."""
    centres = {
        "dan_zong_yi_yuan": (-1600, 4000),
        "wangyintai": (3700, -2300),
    }
    cx, cz = centres[profile]
    min_x = cx - tile_size // 2
    min_z = cz - tile_size // 2
    return WorldTile(
        tile_x=min_x // tile_size,
        tile_z=min_z // tile_size,
        min_x=min_x,
        max_x=min_x + tile_size - 1,
        min_z=min_z,
        max_z=min_z + tile_size - 1,
    )


# ---------------------------------------------------------------------------
# Dispatch tests — profiles no longer return None
# ---------------------------------------------------------------------------


class StitcherDispatchDanZongTests(unittest.TestCase):
    """Verify _build_zone_overlay_tile dispatches dan_zong_yi_yuan correctly."""

    def setUp(self) -> None:
        self.zone = _make_zone("dan_zong_yi_yuan")
        self.tile = _make_tile_for("dan_zong_yi_yuan", tile_size=64)
        self.palette = SurfacePalette()

    def test_dispatch_returns_non_none(self) -> None:
        result = _build_zone_overlay_tile(
            self.zone, self.tile, 64, self.palette,
        )
        self.assertIsNotNone(
            result,
            "expected _build_zone_overlay_tile to dispatch dan_zong_yi_yuan "
            "and return a TileFieldBuffer, but got None — the profile likely "
            "fell through to the else branch",
        )

    def test_overlay_contains_expected_ecology_layers(self) -> None:
        result = _build_zone_overlay_tile(
            self.zone, self.tile, 64, self.palette,
        )
        assert result is not None
        self.assertIn(
            "flora_density", result.layers,
            "expected flora_density layer in dan_zong_yi_yuan overlay "
            "because the profile declares it in extra_layers",
        )
        self.assertIn(
            "flora_variant_id", result.layers,
            "expected flora_variant_id layer in dan_zong_yi_yuan overlay "
            "because the profile declares it in extra_layers",
        )
        self.assertIn(
            "qi_density", result.layers,
            "expected qi_density layer in dan_zong_yi_yuan overlay",
        )

    def test_flora_variant_id_remapped_to_global(self) -> None:
        """After _build_zone_overlay_tile, flora_variant_id should already
        contain global ids (the function calls _remap_flora_variant_to_global
        internally)."""
        result = _build_zone_overlay_tile(
            self.zone, self.tile, 64, self.palette,
        )
        assert result is not None
        fv = result.layers["flora_variant_id"]
        offset = PROFILE_DECORATION_OFFSETS.get("dan_zong_yi_yuan", 0)

        # Zero must stay zero (no decoration).
        zero_mask = fv == 0
        if zero_mask.any():
            # 0 pixels are correct.
            pass

        nonzero = fv[fv > 0]
        if len(nonzero) > 0:
            min_global = int(nonzero.min())
            # Global ids for this profile start at `offset`.
            self.assertGreaterEqual(
                min_global, offset,
                f"expected non-zero flora_variant_id >= {offset} "
                f"(dan_zong_yi_yuan global offset) after remap, "
                f"actual min={min_global}",
            )


class StitcherDispatchWangyintaiTests(unittest.TestCase):
    """Verify _build_zone_overlay_tile dispatches wangyintai correctly."""

    def setUp(self) -> None:
        self.zone = _make_zone("wangyintai")
        self.tile = _make_tile_for("wangyintai", tile_size=64)
        self.palette = SurfacePalette()

    def test_dispatch_returns_non_none(self) -> None:
        result = _build_zone_overlay_tile(
            self.zone, self.tile, 64, self.palette,
        )
        self.assertIsNotNone(
            result,
            "expected _build_zone_overlay_tile to dispatch wangyintai "
            "and return a TileFieldBuffer, but got None — the profile likely "
            "fell through to the else branch",
        )

    def test_overlay_contains_expected_ecology_layers(self) -> None:
        result = _build_zone_overlay_tile(
            self.zone, self.tile, 64, self.palette,
        )
        assert result is not None
        self.assertIn(
            "flora_density", result.layers,
            "expected flora_density layer in wangyintai overlay "
            "because the profile declares it in extra_layers",
        )
        self.assertIn(
            "flora_variant_id", result.layers,
            "expected flora_variant_id layer in wangyintai overlay "
            "because the profile declares it in extra_layers",
        )
        self.assertIn(
            "qi_density", result.layers,
            "expected qi_density layer in wangyintai overlay",
        )
        self.assertIn(
            "mofa_decay", result.layers,
            "expected mofa_decay layer in wangyintai overlay",
        )

    def test_flora_variant_id_remapped_to_global(self) -> None:
        result = _build_zone_overlay_tile(
            self.zone, self.tile, 64, self.palette,
        )
        assert result is not None
        fv = result.layers["flora_variant_id"]
        offset = PROFILE_DECORATION_OFFSETS.get("wangyintai", 0)

        zero_mask = fv == 0
        if zero_mask.any():
            pass  # 0 pixels are correct.

        nonzero = fv[fv > 0]
        if len(nonzero) > 0:
            min_global = int(nonzero.min())
            self.assertGreaterEqual(
                min_global, offset,
                f"expected non-zero flora_variant_id >= {offset} "
                f"(wangyintai global offset) after remap, "
                f"actual min={min_global}",
            )


# ---------------------------------------------------------------------------
# _remap_flora_variant_to_global unit tests
# ---------------------------------------------------------------------------


class RemapFloraVariantToGlobalTests(unittest.TestCase):
    """Directly test _remap_flora_variant_to_global behaviour."""

    def _make_buffer_with_variants(
        self, variant_values: list[int],
    ):
        from scripts.terrain_gen.fields import TileFieldBuffer, WorldTile

        n = len(variant_values)
        tile = WorldTile(
            tile_x=0, tile_z=0,
            min_x=0, max_x=n - 1,
            min_z=0, max_z=0,
        )
        buf = TileFieldBuffer.create(tile, n, ("flora_variant_id",))
        buf.layers["flora_variant_id"] = np.array(
            variant_values, dtype=np.uint8,
        )
        return buf

    def test_zero_stays_zero(self) -> None:
        buf = self._make_buffer_with_variants([0, 0, 0])
        _remap_flora_variant_to_global(buf, "wangyintai")
        result = buf.layers["flora_variant_id"]
        self.assertTrue(
            (result == 0).all(),
            f"expected all-zero flora_variant_id to remain all-zero after "
            f"remap, actual={list(result)}",
        )

    def test_nonzero_shifted_by_offset(self) -> None:
        offset = PROFILE_DECORATION_OFFSETS["wangyintai"]
        buf = self._make_buffer_with_variants([0, 1, 2])
        _remap_flora_variant_to_global(buf, "wangyintai")
        result = list(int(v) for v in buf.layers["flora_variant_id"])
        expected = [0, offset, offset + 1]
        self.assertEqual(
            result, expected,
            f"expected remap [0,1,2] -> {expected} for wangyintai "
            f"(offset={offset}), actual={result}",
        )

    def test_first_profile_no_shift(self) -> None:
        """The first profile in registration order gets offset 1, so local
        ids 1..N already equal global 1..N and _remap returns early."""
        # Find the first profile with offset <= 1.
        first_profiles = [
            p for p, o in PROFILE_DECORATION_OFFSETS.items() if o <= 1
        ]
        if not first_profiles:
            self.skipTest("no profile with offset <= 1 found")
        profile = first_profiles[0]
        buf = self._make_buffer_with_variants([0, 1, 2])
        original = buf.layers["flora_variant_id"].copy()
        _remap_flora_variant_to_global(buf, profile)
        np.testing.assert_array_equal(
            buf.layers["flora_variant_id"], original,
            err_msg=(
                f"expected no shift for profile '{profile}' (offset <= 1), "
                f"but values changed"
            ),
        )

    def test_missing_layer_is_noop(self) -> None:
        """If the buffer has no flora_variant_id layer, remap is a no-op."""
        from scripts.terrain_gen.fields import TileFieldBuffer, WorldTile

        tile = WorldTile(
            tile_x=0, tile_z=0,
            min_x=0, max_x=3,
            min_z=0, max_z=0,
        )
        buf = TileFieldBuffer.create(tile, 4, ("height",))
        # Should not raise.
        _remap_flora_variant_to_global(buf, "wangyintai")
        self.assertNotIn(
            "flora_variant_id", buf.layers,
            "expected no flora_variant_id layer to be created when it "
            "was not present originally",
        )


if __name__ == "__main__":
    unittest.main()
