"""Saturated tests for plan-terrain-wiring-v1 P0 deliverables.

Covers:
- B2: run_layout -> LayoutResult.paste_results -> export_placement_manifest (bridge)
- B3: stamp_radial / block_grid produce real non-empty blocks
- M3: facing / axis / rotation property rotation in _paste_nbt / _rotate_properties
- Registry: COMPOUND_LAYOUT_REGISTRY contains dan_zong_compound / wangyintai_compound
- Passthrough: BlueprintZone.architectural_layout / compound_flatten_radius
- ZoneFieldPlan new fields via profiles/base.py
- stitcher unit: apply_compound_flatten / compute_layout_density_mask
- __main__.run_layout_pass: zone with layout writes placement_manifest.json;
  zone missing poi_kind -> warn+skip, no crash
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

# Ensure worldgen root is on path
_worldgen_root = str(Path(__file__).resolve().parents[1])
_repo_root = str(Path(__file__).resolve().parents[2])
sys.path.insert(0, _worldgen_root)
sys.path.insert(0, str(Path(_repo_root) / "scripts" / "nbt"))


from scripts.terrain_gen.blueprint import (  # noqa: E402
    BlueprintZone,
    BoundarySpec,
    PoiSpec,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import (  # noqa: E402
    Bounds2D,
    TileFieldBuffer,
    WorldTile,
    ZoneFieldPlan,
)
from scripts.terrain_gen.layouts import (  # noqa: E402
    COMPOUND_LAYOUT_REGISTRY,
    LayoutResult,
    LayoutSpec,
    NbtPasteResult,
    Placement,
    export_placement_manifest,
    run_layout,
)
from scripts.terrain_gen.layouts.runner import (  # noqa: E402
    BlockPlacement,
    _block_grid,
    _paste_nbt,
    _rotate_properties,
    _stamp_radial,
)
from scripts.terrain_gen.stitcher import (  # noqa: E402
    apply_compound_flatten,
    compute_layout_density_mask,
)


# ---------------------------------------------------------------------------
# Test fixtures
# ---------------------------------------------------------------------------


def _make_poi(kind: str = "ruin", y: float = 82.0) -> PoiSpec:
    return PoiSpec(kind=kind, pos_xyz=(100.0, y, 200.0))


def _make_zone(
    pois: tuple[PoiSpec, ...] = (),
    center: tuple[int, int] = (100, 200),
    architectural_layout: str | None = None,
    compound_flatten_radius: int | None = None,
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
            surface_palette=("podzol",),
        ),
        pois=pois,
        architectural_layout=architectural_layout,
        compound_flatten_radius=compound_flatten_radius,
    )


def _make_simple_nbt() -> str:
    """Create a simple 2-block NBT file and return its path."""
    from nbt_builder import StructureBuilder  # type: ignore[import]

    fd, path = tempfile.mkstemp(suffix=".nbt")
    os.close(fd)
    sb = StructureBuilder(3, 3, 3)
    sb.set_block(0, 0, 0, "minecraft:stone_bricks")
    sb.set_block(1, 0, 0, "minecraft:oak_stairs", {"facing": "north", "half": "bottom"})
    sb.save(path)
    return path


def _make_buffer(
    tile: WorldTile, tile_size: int = 64, height_val: float = 76.0
) -> TileFieldBuffer:
    buffer = TileFieldBuffer.create(
        tile,
        tile_size,
        ("height", "flora_density", "flora_variant_id", "ground_cover_density", "ground_cover_id"),
    )
    buffer.layers["height"][:] = height_val
    buffer.layers["flora_density"][:] = 0.5
    buffer.layers["flora_variant_id"][:] = 3
    buffer.layers["ground_cover_density"][:] = 0.3
    buffer.layers["ground_cover_id"][:] = 1
    return buffer


def _make_tile(min_x: int = 80, min_z: int = 180, tile_size: int = 64) -> WorldTile:
    return WorldTile(
        tile_x=min_x // tile_size,
        tile_z=min_z // tile_size,
        min_x=min_x,
        max_x=min_x + tile_size - 1,
        min_z=min_z,
        max_z=min_z + tile_size - 1,
    )


# ---------------------------------------------------------------------------
# B2: Bridge run_layout -> paste_results -> export_placement_manifest
# ---------------------------------------------------------------------------


class B2BridgeTests(unittest.TestCase):
    """LayoutResult.paste_results is populated; export manifest has non-empty blocks."""

    def _make_layout_with_nbt(self, nbt_path: str) -> tuple[LayoutSpec, BlueprintZone]:
        spec = LayoutSpec(
            name="bridge_test",
            poi_kind="ruin",
            radius=20,
            placements=(
                Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload=nbt_path),
            ),
        )
        zone = _make_zone(pois=(_make_poi("ruin"),))
        return spec, zone

    def test_run_layout_populates_paste_results(self):
        """B2: run_layout returns LayoutResult with paste_results non-empty."""
        nbt_path = _make_simple_nbt()
        try:
            spec, zone = self._make_layout_with_nbt(nbt_path)
            result = run_layout(spec, zone)
            self.assertIsInstance(result, LayoutResult)
            self.assertEqual(len(result.paste_results), 1,
                             "paste_results should have one entry for the nbt placement")
            pr = result.paste_results[0]
            self.assertIsInstance(pr, NbtPasteResult)
            self.assertGreater(pr.block_count, 0,
                               "NbtPasteResult.block_count should be > 0 after reading real NBT")
            self.assertGreater(len(pr.blocks), 0,
                               "NbtPasteResult.blocks should be non-empty")
        finally:
            os.unlink(nbt_path)

    def test_paste_results_feeds_export_manifest(self):
        """B2: run_layout paste_results fed into export_placement_manifest yields valid JSON."""
        nbt_path = _make_simple_nbt()
        fd, manifest_path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        try:
            spec, zone = self._make_layout_with_nbt(nbt_path)
            result = run_layout(spec, zone)
            export_placement_manifest(list(result.paste_results), manifest_path)

            with open(manifest_path) as f:
                manifest = json.load(f)

            self.assertEqual(manifest["version"], 1)
            self.assertGreater(len(manifest["structures"]), 0,
                               "Manifest structures should be non-empty")
            first_struct = manifest["structures"][0]
            self.assertGreater(
                len(first_struct["blocks"]), 0,
                "structures[0].blocks should be non-empty (B2 data path complete)",
            )
        finally:
            os.unlink(nbt_path)
            os.unlink(manifest_path)

    def test_multiple_placements_all_in_paste_results(self):
        """B2: Multiple placements -> paste_results length matches placement count."""
        nbt_path = _make_simple_nbt()
        try:
            spec = LayoutSpec(
                name="multi",
                poi_kind="ruin",
                radius=50,
                placements=(
                    Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload=nbt_path),
                    Placement(offset=(10, 0, 0), rotation=0, kind="stamp_radial", payload="herb_garden_pen_6x6"),
                    Placement(offset=(0, 0, 20), rotation=0, kind="block_grid", payload="path_6x10"),
                ),
            )
            zone = _make_zone(pois=(_make_poi("ruin"),))
            result = run_layout(spec, zone)
            self.assertEqual(len(result.paste_results), 3,
                             "Each placement kind should produce a paste result")
        finally:
            os.unlink(nbt_path)

    def test_empty_placements_gives_empty_paste_results(self):
        """B2: Empty placement list -> empty paste_results tuple."""
        spec = LayoutSpec(name="empty", poi_kind="ruin", radius=10, placements=())
        zone = _make_zone(pois=(_make_poi("ruin"),))
        result = run_layout(spec, zone)
        self.assertEqual(len(result.paste_results), 0)

    def test_missing_poi_raises_value_error(self):
        """B2: Missing POI -> ValueError (not silent)."""
        spec = LayoutSpec(name="bad", poi_kind="nonexistent", radius=10, placements=())
        zone = _make_zone(pois=(_make_poi("ruin"),))
        with self.assertRaises(ValueError):
            run_layout(spec, zone)

    def test_missing_nbt_file_paste_result_has_zero_blocks(self):
        """B2: Missing NBT file -> paste_result has block_count=0, no crash."""
        spec = LayoutSpec(
            name="missing",
            poi_kind="ruin",
            radius=10,
            placements=(
                Placement(offset=(0, 0, 0), rotation=0, kind="nbt", payload="/nonexistent/file.nbt"),
            ),
        )
        zone = _make_zone(pois=(_make_poi("ruin"),))
        result = run_layout(spec, zone)
        self.assertEqual(len(result.paste_results), 1)
        pr = result.paste_results[0]
        self.assertEqual(pr.block_count, 0, "Missing NBT -> block_count=0")
        self.assertEqual(len(pr.blocks), 0)


# ---------------------------------------------------------------------------
# B3: stamp_radial / block_grid produce real blocks
# ---------------------------------------------------------------------------


class B3StampRadialTests(unittest.TestCase):
    """stamp_radial produces non-empty block list (B3 implementation)."""

    def test_6x6_produces_blocks(self):
        result = _stamp_radial((100, 64, 200), 0, "herb_garden_pen_6x6")
        self.assertGreater(result.block_count, 0,
                           "stamp_radial 6x6 should produce blocks (B3)")
        self.assertGreater(len(result.blocks), 0)

    def test_8x8_produces_blocks(self):
        result = _stamp_radial((100, 64, 200), 0, "herb_garden_pen_8x8")
        self.assertGreater(result.block_count, 0,
                           "stamp_radial 8x8 should produce blocks (B3)")

    def test_6x6_block_count_farmland_plus_wheat(self):
        """6x6 grid = 36 farmland + 36 wheat = 72 blocks."""
        result = _stamp_radial((100, 64, 200), 0, "herb_garden_pen_6x6")
        self.assertEqual(result.block_count, 72,
                         "6x6 pen: 36 farmland + 36 wheat = 72 blocks")

    def test_8x8_block_count_farmland_plus_wheat(self):
        """8x8 grid = 64 farmland + 64 wheat = 128 blocks."""
        result = _stamp_radial((100, 64, 200), 0, "herb_garden_pen_8x8")
        self.assertEqual(result.block_count, 128,
                         "8x8 pen: 64 farmland + 64 wheat = 128 blocks")

    def test_blocks_contain_farmland_and_wheat(self):
        result = _stamp_radial((100, 64, 200), 0, "herb_garden_pen_6x6")
        block_names = {b.block_name for b in result.blocks}
        self.assertIn("minecraft:farmland", block_names,
                      "stamp_radial should include farmland blocks")
        self.assertIn("minecraft:wheat", block_names,
                      "stamp_radial should include wheat blocks")

    def test_world_pos_is_used_as_origin(self):
        result = _stamp_radial((500, 80, 300), 0, "herb_garden_pen_6x6")
        xs = [b.world_pos[0] for b in result.blocks]
        zs = [b.world_pos[2] for b in result.blocks]
        # Center at 500, half=3, so X range is [497..502]
        self.assertIn(500, xs, "Origin X should be included in block positions")
        self.assertIn(300, zs, "Origin Z should be included in block positions")

    def test_determinism(self):
        """Same inputs -> same block list."""
        r1 = _stamp_radial((50, 70, 150), 180, "herb_garden_pen_6x6")
        r2 = _stamp_radial((50, 70, 150), 180, "herb_garden_pen_6x6")
        self.assertEqual(
            [b.world_pos for b in r1.blocks],
            [b.world_pos for b in r2.blocks],
            "stamp_radial must be deterministic",
        )

    def test_all_rotations_produce_same_count(self):
        """All 4 rotations produce the same number of blocks."""
        counts = [
            _stamp_radial((0, 0, 0), rot, "herb_garden_pen_6x6").block_count
            for rot in (0, 90, 180, 270)
        ]
        self.assertEqual(len(set(counts)), 1,
                         "All rotations should produce the same block count")


class B3BlockGridTests(unittest.TestCase):
    """block_grid produces non-empty block list (B3 implementation)."""

    def test_produces_blocks(self):
        result = _block_grid((100, 64, 200), 0, "central_path_6x152")
        self.assertGreater(result.block_count, 0,
                           "block_grid should produce blocks (B3)")
        self.assertGreater(len(result.blocks), 0)

    def test_6x152_block_count(self):
        """6 wide x 152 long = 912 blocks."""
        result = _block_grid((100, 64, 200), 0, "central_path_6x152")
        self.assertEqual(result.block_count, 912,
                         "6x152 path: 6*152=912 blocks")

    def test_custom_dimension_4x50(self):
        """Payload dimension parsing: 4x50 = 200 blocks."""
        result = _block_grid((0, 0, 0), 0, "stone_path_4x50")
        self.assertEqual(result.block_count, 200, "4*50=200 blocks")

    def test_blocks_are_mossy_cobblestone(self):
        result = _block_grid((100, 64, 200), 0, "central_path_6x10")
        block_names = {b.block_name for b in result.blocks}
        self.assertIn("minecraft:mossy_cobblestone", block_names,
                      "block_grid should produce mossy_cobblestone")

    def test_rotation_90_swaps_extension_axis(self):
        """Rotating 90 swaps which axis the path extends along."""
        r0 = _block_grid((0, 64, 0), 0, "path_6x20")
        r90 = _block_grid((0, 64, 0), 90, "path_6x20")
        z_range_0 = max(b.world_pos[2] for b in r0.blocks) - min(b.world_pos[2] for b in r0.blocks)
        z_range_90 = max(b.world_pos[2] for b in r90.blocks) - min(b.world_pos[2] for b in r90.blocks)
        # At rotation=0 path extends along Z (large z range); at 90 extends along X
        self.assertGreater(z_range_0, z_range_90,
                           "rotation=0 path should have greater Z range than rotation=90")

    def test_determinism(self):
        r1 = _block_grid((10, 64, 20), 180, "path_6x10")
        r2 = _block_grid((10, 64, 20), 180, "path_6x10")
        self.assertEqual(
            [b.world_pos for b in r1.blocks],
            [b.world_pos for b in r2.blocks],
            "block_grid must be deterministic",
        )

    def test_all_rotations_produce_same_count(self):
        counts = [
            _block_grid((0, 0, 0), rot, "path_6x10").block_count
            for rot in (0, 90, 180, 270)
        ]
        self.assertEqual(len(set(counts)), 1,
                         "All rotations should produce same block count")


# ---------------------------------------------------------------------------
# M3: Facing / axis / rotation property rotation
# ---------------------------------------------------------------------------


class M3RotatePropertiesTests(unittest.TestCase):
    """_rotate_properties applies M3 directional blockstate rotation."""

    # --- facing ---
    def test_facing_north_rot90_becomes_west(self):
        """Stair facing north, rotated 90 CW -> faces west."""
        result = _rotate_properties({"facing": "north", "half": "bottom"}, 90)
        self.assertEqual(result["facing"], "west",
                         "north + 90 CW should become west")

    def test_facing_north_rot180_becomes_south(self):
        result = _rotate_properties({"facing": "north"}, 180)
        self.assertEqual(result["facing"], "south")

    def test_facing_north_rot270_becomes_east(self):
        result = _rotate_properties({"facing": "north"}, 270)
        self.assertEqual(result["facing"], "east")

    def test_facing_east_rot90_becomes_north(self):
        result = _rotate_properties({"facing": "east"}, 90)
        self.assertEqual(result["facing"], "north")

    def test_facing_east_rot180_becomes_west(self):
        result = _rotate_properties({"facing": "east"}, 180)
        self.assertEqual(result["facing"], "west")

    def test_facing_east_rot270_becomes_south(self):
        result = _rotate_properties({"facing": "east"}, 270)
        self.assertEqual(result["facing"], "south")

    def test_facing_south_rot90_becomes_east(self):
        result = _rotate_properties({"facing": "south"}, 90)
        self.assertEqual(result["facing"], "east")

    def test_facing_west_rot90_becomes_south(self):
        result = _rotate_properties({"facing": "west"}, 90)
        self.assertEqual(result["facing"], "south")

    def test_facing_up_invariant(self):
        result = _rotate_properties({"facing": "up"}, 90)
        self.assertEqual(result["facing"], "up",
                         "up facing is rotation-invariant")

    def test_facing_down_invariant(self):
        result = _rotate_properties({"facing": "down"}, 180)
        self.assertEqual(result["facing"], "down")

    def test_facing_rotation_0_is_identity(self):
        props = {"facing": "north", "half": "bottom"}
        result = _rotate_properties(props, 0)
        self.assertEqual(result, props,
                         "rotation=0 must be identity")

    def test_facing_full_360_returns_to_origin(self):
        """Applying 90 rotation 4 times returns to original facing."""
        current = {"facing": "north"}
        for _ in range(4):
            current = _rotate_properties(current, 90)
        self.assertEqual(current["facing"], "north",
                         "4x90 rotation must return to original facing")

    def test_non_directional_properties_pass_through(self):
        props = {"facing": "north", "half": "bottom", "open": "false", "powered": "true"}
        result = _rotate_properties(props, 90)
        self.assertEqual(result["half"], "bottom")
        self.assertEqual(result["open"], "false")
        self.assertEqual(result["powered"], "true")

    # --- axis ---
    def test_axis_x_rot90_becomes_z(self):
        result = _rotate_properties({"axis": "x"}, 90)
        self.assertEqual(result["axis"], "z",
                         "axis x rotated 90 should become z")

    def test_axis_z_rot90_becomes_x(self):
        result = _rotate_properties({"axis": "z"}, 90)
        self.assertEqual(result["axis"], "x")

    def test_axis_y_invariant(self):
        result = _rotate_properties({"axis": "y"}, 90)
        self.assertEqual(result["axis"], "y",
                         "Y axis is rotation-invariant")

    def test_axis_x_rot180_unchanged(self):
        result = _rotate_properties({"axis": "x"}, 180)
        self.assertEqual(result["axis"], "x",
                         "180 rotation of X axis stays X (symmetry)")

    # --- rotation (banner/sign) ---
    def test_banner_rot0_plus_90_gives_step4(self):
        """Banner rotation 0 + 90 CW = step 4 (4 steps of 22.5)."""
        result = _rotate_properties({"rotation": "0"}, 90)
        self.assertEqual(result["rotation"], "4",
                         "rotation=0 + 90 = 4 steps")

    def test_banner_rot12_plus_90_wraps_to_0(self):
        """Banner rotation 12 + 90 = 0 (mod 16)."""
        result = _rotate_properties({"rotation": "12"}, 90)
        self.assertEqual(result["rotation"], "0",
                         "rotation=12 + 4 steps = 16 mod 16 = 0")

    def test_banner_rot2_plus_180_gives_10(self):
        result = _rotate_properties({"rotation": "2"}, 180)
        self.assertEqual(result["rotation"], "10",
                         "rotation=2 + 8 steps (180) = 10")

    def test_empty_properties_noop(self):
        result = _rotate_properties({}, 90)
        self.assertEqual(result, {})

    def test_result_is_new_dict(self):
        """_rotate_properties must not mutate the input dict."""
        props = {"facing": "north"}
        result = _rotate_properties(props, 90)
        self.assertIsNot(result, props,
                         "_rotate_properties must return a new dict")
        self.assertEqual(props["facing"], "north",
                         "Original props must not be mutated")


class M3PasteNbtFacingRotationTests(unittest.TestCase):
    """Integration: _paste_nbt applies facing rotation (M3 — server stamps as-is)."""

    def test_stair_facing_north_rot90_gives_west(self):
        """Stair facing north pasted at rotation=90 should have facing=west in manifest."""
        from nbt_builder import StructureBuilder  # type: ignore[import]

        fd, path = tempfile.mkstemp(suffix=".nbt")
        os.close(fd)
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.set_block(0, 0, 0, "minecraft:oak_stairs",
                         {"facing": "north", "half": "bottom"})
            sb.save(path)

            result = _paste_nbt((100, 64, 200), 90, path)
            self.assertEqual(len(result.blocks), 1)
            self.assertEqual(
                result.blocks[0].properties.get("facing"), "west",
                "facing=north at rotation=90 must become west in manifest (M3)",
            )
            self.assertEqual(result.blocks[0].properties.get("half"), "bottom",
                             "non-directional property must pass through unchanged")
        finally:
            os.unlink(path)

    def test_stair_facing_east_rot0_preserves_facing(self):
        """rotation=0 -> facing is unchanged."""
        from nbt_builder import StructureBuilder  # type: ignore[import]

        fd, path = tempfile.mkstemp(suffix=".nbt")
        os.close(fd)
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.set_block(0, 0, 0, "minecraft:oak_stairs", {"facing": "east", "half": "bottom"})
            sb.save(path)

            result = _paste_nbt((0, 0, 0), 0, path)
            self.assertEqual(result.blocks[0].properties.get("facing"), "east")
        finally:
            os.unlink(path)


# ---------------------------------------------------------------------------
# Registry
# ---------------------------------------------------------------------------


class RegistryTests(unittest.TestCase):
    """COMPOUND_LAYOUT_REGISTRY is populated with expected entries."""

    def test_registry_contains_dan_zong_compound(self):
        self.assertIn("dan_zong_compound", COMPOUND_LAYOUT_REGISTRY,
                      "COMPOUND_LAYOUT_REGISTRY must contain dan_zong_compound")

    def test_registry_contains_wangyintai_compound(self):
        self.assertIn("wangyintai_compound", COMPOUND_LAYOUT_REGISTRY,
                      "COMPOUND_LAYOUT_REGISTRY must contain wangyintai_compound")

    def test_registry_values_are_layout_specs(self):
        from scripts.terrain_gen.layouts.base import LayoutSpec

        for name, spec in COMPOUND_LAYOUT_REGISTRY.items():
            self.assertIsInstance(spec, LayoutSpec,
                                  f"COMPOUND_LAYOUT_REGISTRY['{name}'] must be a LayoutSpec")

    def test_dan_zong_poi_kind_is_ruin(self):
        spec = COMPOUND_LAYOUT_REGISTRY["dan_zong_compound"]
        self.assertEqual(spec.poi_kind, "ruin",
                         "dan_zong_compound poi_kind must be 'ruin'")

    def test_wangyintai_poi_kind_is_guantiantai(self):
        spec = COMPOUND_LAYOUT_REGISTRY["wangyintai_compound"]
        self.assertEqual(spec.poi_kind, "guantiantai",
                         "wangyintai_compound poi_kind must be 'guantiantai'")

    def test_dan_zong_has_placements(self):
        spec = COMPOUND_LAYOUT_REGISTRY["dan_zong_compound"]
        self.assertGreater(len(spec.placements), 0,
                           "dan_zong_compound must have placements")

    def test_wangyintai_has_placements(self):
        spec = COMPOUND_LAYOUT_REGISTRY["wangyintai_compound"]
        self.assertGreater(len(spec.placements), 0,
                           "wangyintai_compound must have placements")

    def test_registry_is_dict_of_layout_specs(self):
        self.assertIsInstance(COMPOUND_LAYOUT_REGISTRY, dict)


# ---------------------------------------------------------------------------
# Passthrough: BlueprintZone and ZoneFieldPlan new fields
# ---------------------------------------------------------------------------


class PassthroughFieldsTests(unittest.TestCase):
    """BlueprintZone and ZoneFieldPlan carry architectural_layout and compound_flatten_radius."""

    def test_blueprint_zone_defaults_to_none(self):
        zone = _make_zone()
        self.assertIsNone(zone.architectural_layout,
                          "BlueprintZone.architectural_layout must default to None")
        self.assertIsNone(zone.compound_flatten_radius,
                          "BlueprintZone.compound_flatten_radius must default to None")

    def test_blueprint_zone_explicit_values(self):
        zone = _make_zone(
            architectural_layout="dan_zong_compound",
            compound_flatten_radius=96,
        )
        self.assertEqual(zone.architectural_layout, "dan_zong_compound")
        self.assertEqual(zone.compound_flatten_radius, 96)

    def test_zone_field_plan_defaults_to_none(self):
        plan = ZoneFieldPlan(
            zone_name="z",
            display_name="Z",
            profile_name="p",
            generator_name="g",
            shape="ellipse",
            bounds_xz=Bounds2D(min_x=0, max_x=100, min_z=0, max_z=100),
            boundary_mode="soft",
            boundary_width=64,
            required_layers=("height",),
        )
        self.assertIsNone(plan.architectural_layout,
                          "ZoneFieldPlan.architectural_layout must default to None")
        self.assertIsNone(plan.compound_flatten_radius,
                          "ZoneFieldPlan.compound_flatten_radius must default to None")

    def test_zone_field_plan_explicit_values(self):
        plan = ZoneFieldPlan(
            zone_name="z",
            display_name="Z",
            profile_name="p",
            generator_name="g",
            shape="ellipse",
            bounds_xz=Bounds2D(min_x=0, max_x=100, min_z=0, max_z=100),
            boundary_mode="soft",
            boundary_width=64,
            required_layers=("height",),
            architectural_layout="dan_zong_compound",
            compound_flatten_radius=96,
        )
        self.assertEqual(plan.architectural_layout, "dan_zong_compound")
        self.assertEqual(plan.compound_flatten_radius, 96)

    def test_profiles_base_passthrough_with_layout(self):
        """profiles/base.py plan() passes BlueprintZone layout fields to ZoneFieldPlan."""
        from scripts.terrain_gen.blueprint import TerrainProfileSpec
        from scripts.terrain_gen.profiles.base import ProfileContext, TerrainProfileGenerator

        class FakeGenerator(TerrainProfileGenerator):
            profile_name = "fake"

            def build_notes(self, context):  # type: ignore[override]
                return ()

        zone = _make_zone(
            architectural_layout="dan_zong_compound",
            compound_flatten_radius=96,
        )
        profile_spec = TerrainProfileSpec(
            name="fake",
            boundary=BoundarySpec(mode="soft", width=64),
            height={},
            surface=(),
            water={},
            passability="unknown",
        )
        ctx = ProfileContext(zone=zone, profile_spec=profile_spec)
        zone_plan = FakeGenerator().plan(ctx)

        self.assertEqual(zone_plan.architectural_layout, "dan_zong_compound",
                         "ZoneFieldPlan.architectural_layout must be passed from BlueprintZone")
        self.assertEqual(zone_plan.compound_flatten_radius, 96,
                         "ZoneFieldPlan.compound_flatten_radius must be passed from BlueprintZone")

    def test_profiles_base_passthrough_none(self):
        """Zones without layout fields propagate None."""
        from scripts.terrain_gen.blueprint import TerrainProfileSpec
        from scripts.terrain_gen.profiles.base import ProfileContext, TerrainProfileGenerator

        class FakeGenerator(TerrainProfileGenerator):
            profile_name = "fake"

            def build_notes(self, context):  # type: ignore[override]
                return ()

        zone = _make_zone()
        profile_spec = TerrainProfileSpec(
            name="fake",
            boundary=BoundarySpec(mode="soft", width=64),
            height={},
            surface=(),
            water={},
            passability="unknown",
        )
        ctx = ProfileContext(zone=zone, profile_spec=profile_spec)
        zone_plan = FakeGenerator().plan(ctx)

        self.assertIsNone(zone_plan.architectural_layout)
        self.assertIsNone(zone_plan.compound_flatten_radius)


class BlueprintLoadPassthroughTests(unittest.TestCase):
    """load_blueprint propagates layout fields into BlueprintZone."""

    def _make_bp_json(
        self,
        tmpdir: str,
        arch_layout: str | None,
        flatten_radius: int | None = 96,
    ) -> Path:
        worldgen: dict = {
            "terrain_profile": "test_profile",
            "shape": "ellipse",
            "boundary": {"mode": "soft", "width": 64},
            "height_model": {"base": [62, 78]},
            "surface_palette": ["podzol"],
        }
        if arch_layout is not None:
            worldgen["architectural_layout"] = arch_layout
        if flatten_radius is not None:
            worldgen["height_model"]["compound_flatten_radius"] = flatten_radius

        data = {
            "version": 1,
            "world": {
                "name": "w",
                "spawn_zone": "s",
                "bounds_xz": {"min": [-100, -100], "max": [100, 100]},
                "notes": [],
            },
            "zones": [
                {
                    "name": "test_zone",
                    "display_name": "Test Zone",
                    "aabb": {"min": [-50.0, 0.0, -50.0], "max": [50.0, 200.0, 50.0]},
                    "center_xz": [0.0, 0.0],
                    "size_xz": [100.0, 100.0],
                    "spirit_qi": 0.4,
                    "danger_level": 3,
                    "worldgen": worldgen,
                    "pois": [{"kind": "ruin", "pos_xyz": [0.0, 82.0, 0.0]}],
                }
            ],
        }
        path = Path(tmpdir) / "zones.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def test_reads_architectural_layout(self):
        from scripts.terrain_gen.blueprint import load_blueprint

        with tempfile.TemporaryDirectory() as tmpdir:
            path = self._make_bp_json(tmpdir, arch_layout="dan_zong_compound")
            bp = load_blueprint(path)

        zone = bp.zones[0]
        self.assertEqual(zone.architectural_layout, "dan_zong_compound",
                         "load_blueprint must set BlueprintZone.architectural_layout")

    def test_reads_flatten_radius_from_height_model(self):
        from scripts.terrain_gen.blueprint import load_blueprint

        with tempfile.TemporaryDirectory() as tmpdir:
            path = self._make_bp_json(tmpdir, arch_layout=None, flatten_radius=96)
            bp = load_blueprint(path)

        zone = bp.zones[0]
        self.assertEqual(zone.compound_flatten_radius, 96,
                         "load_blueprint must read compound_flatten_radius from height_model")

    def test_no_layout_fields_gives_none(self):
        from scripts.terrain_gen.blueprint import load_blueprint

        with tempfile.TemporaryDirectory() as tmpdir:
            path = self._make_bp_json(tmpdir, arch_layout=None, flatten_radius=None)
            bp = load_blueprint(path)

        zone = bp.zones[0]
        self.assertIsNone(zone.architectural_layout)
        self.assertIsNone(zone.compound_flatten_radius)


# ---------------------------------------------------------------------------
# Stitcher unit: apply_compound_flatten / compute_layout_density_mask
# ---------------------------------------------------------------------------


class StitcherFlattenTests(unittest.TestCase):
    """Unit: apply_compound_flatten and compute_layout_density_mask behavior."""

    def test_flatten_inside_radius_equals_target(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)
        buffer.layers["height"][:] = np.linspace(60, 90, 64 * 64)

        target = 82.0
        apply_compound_flatten(buffer, poi_center_xz=(100, 200), radius=20, target_height=target)

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
                idx = lz * 64 + lx
                if dist_sq < 20 * 20:
                    self.assertAlmostEqual(
                        float(buffer.layers["height"][idx]),
                        target,
                        places=2,
                        msg=f"Height at ({wx},{wz}) inside radius must be {target}, "
                            f"dist={dist_sq**0.5:.1f}",
                    )

    def test_flatten_preserves_outside_radius(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)
        original = buffer.layers["height"].copy()

        # POI far from tile -> nothing should be flattened
        apply_compound_flatten(buffer, poi_center_xz=(500, 500), radius=5, target_height=82.0)

        np.testing.assert_array_almost_equal(
            buffer.layers["height"],
            original,
            decimal=5,
            err_msg="Height must be untouched when POI is outside tile",
        )

    def test_flatten_radius_zero_noop(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)
        original = buffer.layers["height"].copy()

        apply_compound_flatten(buffer, poi_center_xz=(100, 200), radius=0, target_height=82.0)

        np.testing.assert_array_equal(
            buffer.layers["height"],
            original,
            err_msg="radius=0 must not modify any heights",
        )

    def test_large_radius_flattens_entire_tile(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)
        buffer.layers["height"][:] = np.random.default_rng(42).uniform(50, 100, 64 * 64)

        target = 82.0
        apply_compound_flatten(buffer, poi_center_xz=(100, 200), radius=500, target_height=target)

        np.testing.assert_array_almost_equal(
            buffer.layers["height"],
            np.full(64 * 64, target),
            decimal=2,
            err_msg="Large radius should flatten ALL heights to target",
        )

    def test_density_mask_zeroes_flora_inside_radius(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)

        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=20)

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - 100) ** 2 + (wz - 200) ** 2
                idx = lz * 64 + lx
                if dist_sq < 20 * 20:
                    self.assertEqual(
                        float(buffer.layers["flora_density"][idx]),
                        0.0,
                        f"flora_density at ({wx},{wz}) inside mask must be 0, "
                        f"dist={dist_sq**0.5:.1f}",
                    )

    def test_density_mask_preserves_flora_outside_radius(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)

        # POI far away
        compute_layout_density_mask(buffer, poi_center_xz=(500, 500), radius=5)

        self.assertAlmostEqual(
            float(np.min(buffer.layers["flora_density"])),
            0.5,
            places=3,
            msg="flora_density must be preserved when POI is outside tile",
        )

    def test_density_mask_radius_zero_noop(self):
        tile = _make_tile(min_x=80, min_z=180)
        buffer = _make_buffer(tile)
        original = buffer.layers["flora_density"].copy()

        compute_layout_density_mask(buffer, poi_center_xz=(100, 200), radius=0)

        np.testing.assert_array_equal(
            buffer.layers["flora_density"],
            original,
            err_msg="radius=0 must not modify flora_density",
        )


# ---------------------------------------------------------------------------
# Export pass: run_layout_pass
# ---------------------------------------------------------------------------


class RunLayoutPassTests(unittest.TestCase):
    """__main__.run_layout_pass writes placement_manifest.json for layout zones."""

    def _write_blueprint(
        self,
        tmpdir: str,
        poi_kinds: list[str],
        arch_layout: str = "dan_zong_compound",
    ) -> Path:
        pois = [{"kind": k, "pos_xyz": [100.0, 82.0, 200.0]} for k in poi_kinds]
        data = {
            "version": 1,
            "world": {
                "name": "w",
                "spawn_zone": "s",
                "bounds_xz": {"min": [-100, -100], "max": [100, 100]},
                "notes": [],
            },
            "zones": [
                {
                    "name": "layout_zone",
                    "display_name": "Layout Zone",
                    "aabb": {"min": [-50.0, 0.0, -50.0], "max": [50.0, 200.0, 50.0]},
                    "center_xz": [0.0, 0.0],
                    "size_xz": [100.0, 100.0],
                    "spirit_qi": 0.4,
                    "danger_level": 3,
                    "worldgen": {
                        "terrain_profile": "test",
                        "shape": "ellipse",
                        "boundary": {"mode": "soft", "width": 64},
                        "height_model": {"base": [62, 78]},
                        "surface_palette": ["podzol"],
                        "architectural_layout": arch_layout,
                    },
                    "pois": pois,
                }
            ],
        }
        path = Path(tmpdir) / "zones.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def _write_plain_blueprint(self, tmpdir: str) -> Path:
        data = {
            "version": 1,
            "world": {
                "name": "w",
                "spawn_zone": "s",
                "bounds_xz": {"min": [-100, -100], "max": [100, 100]},
                "notes": [],
            },
            "zones": [
                {
                    "name": "plain",
                    "display_name": "Plain",
                    "aabb": {"min": [0.0, 0.0, 0.0], "max": [100.0, 200.0, 100.0]},
                    "center_xz": [50.0, 50.0],
                    "size_xz": [100.0, 100.0],
                    "spirit_qi": 0.2,
                    "danger_level": 1,
                    "worldgen": {
                        "terrain_profile": "plain",
                        "shape": "square",
                        "boundary": {"mode": "soft", "width": 32},
                        "height_model": {"base": [64, 72]},
                        "surface_palette": ["grass_block"],
                    },
                    "pois": [],
                }
            ],
        }
        path = Path(tmpdir) / "zones.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def test_layout_zone_with_poi_writes_manifest(self):
        """Zone with architectural_layout and matching POI -> manifest written."""
        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        poi_kind = COMPOUND_LAYOUT_REGISTRY["dan_zong_compound"].poi_kind

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_blueprint(tmpdir, poi_kinds=[poi_kind],
                                            arch_layout="dan_zong_compound")
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            manifest_path = run_layout_pass(bp, output_dir)

            # All assertions inside the with block (tmpdir still alive)
            self.assertIsNotNone(manifest_path,
                                 "run_layout_pass should return a Path when layouts processed")
            self.assertTrue(manifest_path.exists(),
                            "placement_manifest.json must exist on disk")

            with open(manifest_path) as f:
                manifest = json.load(f)
            self.assertEqual(manifest["version"], 1,
                             "manifest version must be 1")
            self.assertIn("structures", manifest,
                          "manifest must have 'structures' key")
            self.assertIsInstance(manifest["structures"], list)

    def test_manifest_structures_are_non_empty(self):
        """Manifest structures list must be non-empty for a valid layout zone."""
        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        poi_kind = COMPOUND_LAYOUT_REGISTRY["dan_zong_compound"].poi_kind

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_blueprint(tmpdir, poi_kinds=[poi_kind],
                                            arch_layout="dan_zong_compound")
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            manifest_path = run_layout_pass(bp, output_dir)

            # Check inside with block (tmpdir still alive)
            self.assertIsNotNone(manifest_path,
                                 "run_layout_pass must return a path")
            with open(manifest_path) as f:
                manifest = json.load(f)
        self.assertGreater(len(manifest["structures"]), 0,
                           "structures must be non-empty for a layout zone with placements")

    def test_missing_poi_kind_warns_skips_returns_none(self):
        """Zone with wrong poi_kind -> warn+skip, no crash, None returned (M1 guard)."""
        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_blueprint(
                tmpdir,
                poi_kinds=["wrong_poi_kind"],
                arch_layout="dan_zong_compound",
            )
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            with self.assertLogs("scripts.terrain_gen.__main__", level="WARNING") as cm:
                result = run_layout_pass(bp, output_dir)

            # Check inside with block (tmpdir still alive)
            self.assertIsNone(result,
                              "Missing POI -> no manifest written -> None returned")
            warning_text = " ".join(cm.output)
            self.assertIn("layout_zone", warning_text,
                          "Warning should mention the zone name")
            # no manifest file written
            manifest_file = output_dir / "rasters" / "placement_manifest.json"
            self.assertFalse(manifest_file.exists(),
                             "No manifest file should be written when all zones are skipped")

    def test_no_layout_zones_returns_none(self):
        """Blueprint with no architectural_layout zones -> None, no manifest."""
        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_plain_blueprint(tmpdir)
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            result = run_layout_pass(bp, output_dir)

            # Check inside with block
            self.assertIsNone(result,
                              "No layout zones -> None returned")
            manifest_file = output_dir / "rasters" / "placement_manifest.json"
            self.assertFalse(manifest_file.exists(),
                             "No manifest file when no layout zones exist")

    def test_unknown_layout_name_warns_skips(self):
        """Zone referencing unknown layout name -> warn+skip, no crash."""
        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        data = {
            "version": 1,
            "world": {
                "name": "w",
                "spawn_zone": "s",
                "bounds_xz": {"min": [-100, -100], "max": [100, 100]},
                "notes": [],
            },
            "zones": [
                {
                    "name": "bad_layout_zone",
                    "display_name": "Bad",
                    "aabb": {"min": [0.0, 0.0, 0.0], "max": [100.0, 200.0, 100.0]},
                    "center_xz": [50.0, 50.0],
                    "size_xz": [100.0, 100.0],
                    "spirit_qi": 0.2,
                    "danger_level": 1,
                    "worldgen": {
                        "terrain_profile": "plain",
                        "shape": "square",
                        "boundary": {"mode": "soft", "width": 32},
                        "height_model": {"base": [64, 72]},
                        "surface_palette": ["grass_block"],
                        "architectural_layout": "nonexistent_compound_xyz",
                    },
                    "pois": [{"kind": "ruin", "pos_xyz": [50.0, 80.0, 50.0]}],
                }
            ],
        }
        with tempfile.TemporaryDirectory() as tmpdir:
            path = Path(tmpdir) / "zones.json"
            path.write_text(json.dumps(data), encoding="utf-8")
            bp = load_blueprint(path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            with self.assertLogs("scripts.terrain_gen.__main__", level="WARNING") as cm:
                result = run_layout_pass(bp, output_dir)

            # Inside with block
            self.assertIsNone(result)
            warning_text = " ".join(cm.output)
            self.assertIn("nonexistent_compound_xyz", warning_text,
                          "Warning should mention the unknown layout name")

    def test_manifest_location_is_rasters_subdir(self):
        """Manifest is written to <output_dir>/rasters/placement_manifest.json."""
        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        poi_kind = COMPOUND_LAYOUT_REGISTRY["dan_zong_compound"].poi_kind

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_blueprint(tmpdir, poi_kinds=[poi_kind],
                                            arch_layout="dan_zong_compound")
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            manifest_path = run_layout_pass(bp, output_dir)
            expected = output_dir / "rasters" / "placement_manifest.json"

            # Compare inside with block (tmpdir still alive)
            self.assertEqual(manifest_path, expected,
                             "Manifest must be at <output_dir>/rasters/placement_manifest.json")
            self.assertTrue(manifest_path.exists(),
                            "placement_manifest.json must exist at expected path")


if __name__ == "__main__":
    unittest.main()
