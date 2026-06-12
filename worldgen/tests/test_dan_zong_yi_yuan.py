"""Saturated tests for dan_zong_yi_yuan terrain + layout (plan-dandao-path-v1 PR-3).

Covers:
- Zone schema parsing + AABB non-overlap with existing zones
- Terrain profile JSON parsing (dan_zong_yi_yuan entry)
- Layout determinism: same seed twice -> identical coordinates
- Layout-region density mask: no density spawn inside layout radius
- compound_flatten_radius: height field flattened inside radius
- Herb garden planting rule: inner N/NE/E/SE = tuigu_teng,
  inner S/SW/W/NW = shouxin_cao, outer cardinal = longlin_tai,
  outer diagonal = xuyuan_rui
- Layout structure counts and symmetry
- Ambient recipe JSON structure
- Profile generator registration
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from math import cos, radians, sin
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import numpy as np

from scripts.terrain_gen.blueprint import (
    BlueprintZone,
    BoundarySpec,
    PoiSpec,
    TerrainProfileSpec,
    ZoneWorldgenConfig,
    load_profile_catalog,
)
from scripts.terrain_gen.fields import (
    Bounds2D,
    SurfacePalette,
    TileFieldBuffer,
    WorldTile,
)
from scripts.terrain_gen.layouts.base import (
    LayoutResult,
    LayoutSpec,
    Placement,
)
from scripts.terrain_gen.layouts.dan_zong_compound import (
    DAN_ZONG_COMPOUND_LAYOUT,
    INNER_RING_HERB_ASSIGNMENT,
    OUTER_RING_HERB_ASSIGNMENT,
    _BONE_POSITIONS,
    _FURNACE_X_OFFSET,
    _FURNACE_Z_POSITIONS,
    _INNER_ANGLES,
    _INNER_RING_RADIUS,
    _OUTER_ANGLES,
    _OUTER_RING_RADIUS,
    _STELE_ANGLES,
    _STELE_RADIUS,
)
from scripts.terrain_gen.layouts.runner import run_layout
from scripts.terrain_gen.profiles import get_profile_generator, list_profile_generators
from scripts.terrain_gen.stitcher import (
    apply_compound_flatten,
    compute_layout_density_mask,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_DIR = REPO_ROOT / "server"
WORLDGEN_DIR = REPO_ROOT / "worldgen"


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_dan_zong_zone(
    poi_pos: tuple[float, float, float] = (-1600.0, 82.0, 4000.0),
) -> BlueprintZone:
    return BlueprintZone(
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
            height_model={"base": [62, 78], "peak": 92, "compound_flatten_radius": 96},
            surface_palette=(
                "podzol", "coarse_dirt", "mud",
                "purple_terracotta", "mossy_cobblestone",
            ),
        ),
        pois=(
            PoiSpec(
                kind="ruin",
                pos_xyz=poi_pos,
                name="百草丹殿",
                tags=("dandao_path", "alchemy", "boss_lore", "baolongwang_prequel"),
                unlock="found_by_exploration",
                qi_affinity=-0.10,
                danger_bias=2,
            ),
        ),
    )


def _make_tile(min_x: int = -1632, min_z: int = 3968, tile_size: int = 64) -> WorldTile:
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
# Zone schema tests
# ---------------------------------------------------------------------------


class ZoneSchemaTests(unittest.TestCase):
    """Verify dan_zong_yi_yuan and baolongwang_cavern_deep parse correctly."""

    def setUp(self):
        zones_path = SERVER_DIR / "zones.json"
        with zones_path.open(encoding="utf-8") as f:
            self.zones_data = json.load(f)
        self.zones = self.zones_data["zones"]

    def test_dan_zong_zone_present(self):
        names = [z["name"] for z in self.zones]
        self.assertIn(
            "dan_zong_yi_yuan", names,
            f"dan_zong_yi_yuan zone not found in zones.json, got: {names}",
        )

    def test_baolongwang_zone_present(self):
        names = [z["name"] for z in self.zones]
        self.assertIn(
            "baolongwang_cavern_deep", names,
            f"baolongwang_cavern_deep zone not found in zones.json, got: {names}",
        )

    def test_dan_zong_spirit_qi(self):
        # worldgen-v4 P4 §8.1 #4/#11 — spirit_qi 从历史手填值（0.40）重构为统一灵气
        # 场的面积加权均值（派生值 ≈0.39）。P4 后稳定的契约是 **qi_grade 档位**
        # （dan_zong 仍是 common 档 [0.15, 0.45)），精确标量由场派生决定（要锁精确值
        # 须在 blueprint 声明 qi_override）。校验落档 + 接近原值，而非旧定值 0.40。
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertTrue(
            0.15 <= zone["spirit_qi"] < 0.45,
            f"dan_zong_yi_yuan spirit_qi should stay in COMMON grade [0.15, 0.45) "
            f"after P4 field derivation (plan S9.2 中阶丹宗); got {zone['spirit_qi']}",
        )
        self.assertAlmostEqual(
            zone["spirit_qi"], 0.40, delta=0.10,
            msg="dan_zong_yi_yuan derived spirit_qi should stay near the historical "
            "0.40 anchor (P4 target-preserving kernel); large drift = bug",
        )

    def test_dan_zong_danger_level(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertEqual(
            zone["danger_level"], 4,
            "dan_zong_yi_yuan danger_level should be 4 (plan S9.2)",
        )

    def test_baolongwang_negative_spirit_qi(self):
        # worldgen-v4 P4 §8.1 #4/#11 — spirit_qi 从历史手填值（-0.80）重构为统一灵气
        # 场的面积加权均值。baolongwang 的 AABB 与正灵 zhanhun_plain(0.4) 重叠，max-blend
        # 让相邻灵气填补部分负压 → 派生值约 -0.43（仍负灵域，天道盲点意图保留）。P4 后
        # 稳定契约是 **spirit_qi < 0（负灵域，吞噬真元）**，不再是精确 -0.80（要锁精确
        # 深负值须在 blueprint 声明 qi_override 并避开正灵邻接）。
        zone = next(z for z in self.zones if z["name"] == "baolongwang_cavern_deep")
        self.assertLess(
            zone["spirit_qi"], -0.1,
            f"baolongwang_cavern_deep spirit_qi must stay in NEGATIVE grade (< -0.1, "
            f"tiandao blindspot that devours qi, plan S8.1 #3); got {zone['spirit_qi']}",
        )

    def test_aabb_no_overlap_dan_zong(self):
        """dan_zong_yi_yuan AABB must not overlap any other zone."""
        dz = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        dz_aabb = dz["aabb"]
        dz_min = dz_aabb["min"]
        dz_max = dz_aabb["max"]

        for other in self.zones:
            if other["name"] == "dan_zong_yi_yuan":
                continue
            o_min = other["aabb"]["min"]
            o_max = other["aabb"]["max"]
            overlap = (
                dz_min[0] < o_max[0] and dz_max[0] > o_min[0]
                and dz_min[1] < o_max[1] and dz_max[1] > o_min[1]
                and dz_min[2] < o_max[2] and dz_max[2] > o_min[2]
            )
            self.assertFalse(
                overlap,
                f"dan_zong_yi_yuan AABB overlaps with {other['name']}: "
                f"dan_zong=[{dz_min}, {dz_max}] vs [{o_min}, {o_max}]",
            )

    def test_aabb_no_overlap_baolongwang(self):
        """baolongwang_cavern_deep AABB must not overlap any other zone."""
        bz = next(z for z in self.zones if z["name"] == "baolongwang_cavern_deep")
        bz_aabb = bz["aabb"]
        bz_min = bz_aabb["min"]
        bz_max = bz_aabb["max"]

        for other in self.zones:
            if other["name"] == "baolongwang_cavern_deep":
                continue
            o_min = other["aabb"]["min"]
            o_max = other["aabb"]["max"]
            overlap = (
                bz_min[0] < o_max[0] and bz_max[0] > o_min[0]
                and bz_min[1] < o_max[1] and bz_max[1] > o_min[1]
                and bz_min[2] < o_max[2] and bz_max[2] > o_min[2]
            )
            self.assertFalse(
                overlap,
                f"baolongwang_cavern_deep AABB overlaps with {other['name']}: "
                f"baolongwang=[{bz_min}, {bz_max}] vs [{o_min}, {o_max}]",
            )

    def test_dan_zong_ambient_recipe_id(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertEqual(
            zone["ambient_recipe_id"], "ambient_dan_zong",
            "dan_zong_yi_yuan ambient_recipe_id should be 'ambient_dan_zong'",
        )

    def test_dan_zong_patrol_anchors_count(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertEqual(
            len(zone["patrol_anchors"]), 3,
            "dan_zong_yi_yuan should have 3 patrol anchors (plan S9.2)",
        )

    def test_all_zones_valid_structure(self):
        """Every zone in zones.json has required fields."""
        required_fields = {"name", "aabb", "spirit_qi", "danger_level"}
        for zone in self.zones:
            for field in required_fields:
                self.assertIn(
                    field, zone,
                    f"Zone '{zone.get('name', '?')}' missing required field '{field}'",
                )


# ---------------------------------------------------------------------------
# Terrain profile JSON parsing tests
# ---------------------------------------------------------------------------


class TerrainProfileParsingTests(unittest.TestCase):
    def setUp(self):
        catalog_path = WORLDGEN_DIR / "terrain-profiles.example.json"
        self.catalog = load_profile_catalog(catalog_path)

    def test_dan_zong_profile_present(self):
        self.assertIn(
            "dan_zong_yi_yuan", self.catalog.profiles,
            "dan_zong_yi_yuan profile not found in terrain-profiles.example.json",
        )

    def test_dan_zong_architectural_layout(self):
        profile = self.catalog.profiles["dan_zong_yi_yuan"]
        self.assertEqual(
            profile.architectural_layout, "dan_zong_compound",
            "architectural_layout should be 'dan_zong_compound'",
        )

    def test_dan_zong_compound_flatten_radius(self):
        profile = self.catalog.profiles["dan_zong_yi_yuan"]
        self.assertEqual(
            profile.compound_flatten_radius, 96,
            "compound_flatten_radius should be 96",
        )

    def test_dan_zong_height_base(self):
        profile = self.catalog.profiles["dan_zong_yi_yuan"]
        self.assertEqual(
            profile.height.get("base"), [62, 78],
            "height.base should be [62, 78]",
        )

    def test_dan_zong_height_peak(self):
        profile = self.catalog.profiles["dan_zong_yi_yuan"]
        self.assertEqual(
            profile.height.get("peak"), 92,
            "height.peak should be 92",
        )

    def test_dan_zong_surface_palette(self):
        profile = self.catalog.profiles["dan_zong_yi_yuan"]
        expected = ("podzol", "coarse_dirt", "mud", "purple_terracotta", "mossy_cobblestone")
        self.assertEqual(
            profile.surface, expected,
            f"Surface palette mismatch: expected {expected}, got {profile.surface}",
        )

    def test_dan_zong_structure_density_keys(self):
        profile = self.catalog.profiles["dan_zong_yi_yuan"]
        expected_keys = {"wild_herb_clump", "scattered_bone_fragment", "small_rubble"}
        actual_keys = set(profile.extras.get("structure_density", {}).keys())
        self.assertEqual(
            actual_keys, expected_keys,
            f"structure_density keys mismatch: expected {expected_keys}, got {actual_keys}",
        )


# ---------------------------------------------------------------------------
# Profile generator registration tests
# ---------------------------------------------------------------------------


class ProfileGeneratorRegistrationTests(unittest.TestCase):
    def test_dan_zong_generator_registered(self):
        names = list_profile_generators()
        self.assertIn(
            "dan_zong_yi_yuan", names,
            f"dan_zong_yi_yuan generator not registered, available: {names}",
        )

    def test_dan_zong_generator_profile_name(self):
        gen = get_profile_generator("dan_zong_yi_yuan")
        self.assertEqual(gen.profile_name, "dan_zong_yi_yuan")

    def test_dan_zong_generator_has_decorations(self):
        gen = get_profile_generator("dan_zong_yi_yuan")
        self.assertEqual(
            len(gen.ecology.decorations), 3,
            "DanZongYiYuanGenerator should have 3 decorations (wild_herb_clump, "
            "scattered_bone_fragment, small_rubble)",
        )

    def test_dan_zong_decoration_names(self):
        gen = get_profile_generator("dan_zong_yi_yuan")
        names = {d.name for d in gen.ecology.decorations}
        expected = {"wild_herb_clump", "scattered_bone_fragment", "small_rubble"}
        self.assertEqual(
            names, expected,
            f"Decoration names mismatch: expected {expected}, got {names}",
        )


# ---------------------------------------------------------------------------
# Layout definition tests
# ---------------------------------------------------------------------------


class LayoutDefinitionTests(unittest.TestCase):
    """Verify the DAN_ZONG_COMPOUND_LAYOUT structure."""

    def test_layout_name(self):
        self.assertEqual(DAN_ZONG_COMPOUND_LAYOUT.name, "dan_zong_compound")

    def test_layout_poi_kind(self):
        self.assertEqual(DAN_ZONG_COMPOUND_LAYOUT.poi_kind, "ruin")

    def test_layout_radius(self):
        self.assertEqual(
            DAN_ZONG_COMPOUND_LAYOUT.radius, 96,
            "Layout radius should be 96, matching compound_flatten_radius",
        )

    def test_total_placement_count(self):
        """1 hall + 1 sarcophagus + 16 gardens + 16 furnaces + 1 path + 3 springs + 4 steles + 8 bones = 50"""
        self.assertEqual(
            len(DAN_ZONG_COMPOUND_LAYOUT.placements), 50,
            f"Expected 50 placements (49 original + master_sarcophagus P2 #3), "
            f"got {len(DAN_ZONG_COMPOUND_LAYOUT.placements)}",
        )

    def test_great_hall_at_origin(self):
        hall = DAN_ZONG_COMPOUND_LAYOUT.placements[0]
        self.assertEqual(hall.offset, (0, 0, 0), "Great hall should be at origin")
        self.assertEqual(hall.kind, "nbt")
        self.assertIn("great_hall", hall.payload)

    def test_inner_herb_gardens_count(self):
        """8 inner ring herb gardens (placements 1-8)."""
        inner_gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_6x6"
        ]
        self.assertEqual(
            len(inner_gardens), 8,
            f"Expected 8 inner herb gardens (6x6), got {len(inner_gardens)}",
        )

    def test_outer_herb_gardens_count(self):
        """8 outer ring herb gardens (placements 9-16)."""
        outer_gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_8x8"
        ]
        self.assertEqual(
            len(outer_gardens), 8,
            f"Expected 8 outer herb gardens (8x8), got {len(outer_gardens)}",
        )

    def test_furnace_count(self):
        furnaces = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "ruined_open_furnace.nbt"
        ]
        self.assertEqual(
            len(furnaces), 16,
            f"Expected 16 furnaces (8 pairs), got {len(furnaces)}",
        )

    def test_furnace_symmetry(self):
        """Furnaces should be mirrored about x=0 axis."""
        furnaces = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "ruined_open_furnace.nbt"
        ]
        left_xs = sorted(p.offset[0] for p in furnaces if p.offset[0] < 0)
        right_xs = sorted(p.offset[0] for p in furnaces if p.offset[0] > 0)
        self.assertEqual(
            len(left_xs), len(right_xs),
            "Furnaces should have equal left/right count for symmetry",
        )
        for lx, rx in zip(left_xs, right_xs):
            self.assertEqual(
                lx, -rx,
                f"Furnace symmetry broken: left x={lx}, right x={rx} (should be mirrors)",
            )

    def test_poison_springs_count(self):
        springs = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if "poison_spring" in p.payload
        ]
        self.assertEqual(
            len(springs), 3,
            f"Expected 3 poison springs, got {len(springs)}",
        )

    def test_stele_count(self):
        steles = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "fallen_recipe_stele.nbt"
        ]
        self.assertEqual(
            len(steles), 4,
            f"Expected 4 fallen recipe steles, got {len(steles)}",
        )

    def test_bone_count(self):
        bones = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "fallen_alchemist_bone.nbt"
        ]
        self.assertEqual(
            len(bones), 8,
            f"Expected 8 fallen alchemist bones, got {len(bones)}",
        )

    def test_bones_along_central_path(self):
        """Bones should be placed at small x offsets (path edges) with varying z."""
        bones = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "fallen_alchemist_bone.nbt"
        ]
        for bone in bones:
            self.assertIn(
                abs(bone.offset[0]), (4,),
                f"Bone x offset should be +/-4 (path edge), got {bone.offset[0]}",
            )
            self.assertGreater(
                bone.offset[2], 0,
                f"Bone z offset should be positive (along central path), got {bone.offset[2]}",
            )

    def test_central_path_present(self):
        paths = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.kind == "block_grid"
        ]
        self.assertEqual(
            len(paths), 1,
            f"Expected exactly 1 central path (block_grid), got {len(paths)}",
        )
        self.assertIn("central_path", paths[0].payload)

    def test_all_rotations_valid(self):
        """Every placement rotation is 0/90/180/270."""
        for p in DAN_ZONG_COMPOUND_LAYOUT.placements:
            self.assertIn(
                p.rotation, (0, 90, 180, 270),
                f"Invalid rotation {p.rotation} for placement '{p.payload}'",
            )

    def test_inner_ring_positions_at_correct_radius(self):
        """Inner herb gardens should be at approximately r=24 from origin."""
        gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_6x6"
        ]
        for garden in gardens:
            dx, dz = garden.offset[0], garden.offset[2]
            dist = (dx * dx + dz * dz) ** 0.5
            self.assertAlmostEqual(
                dist, _INNER_RING_RADIUS, delta=2,
                msg=(
                    f"Inner garden at ({dx},{dz}) distance={dist:.1f} "
                    f"should be ~{_INNER_RING_RADIUS} (inner ring radius)"
                ),
            )

    def test_outer_ring_positions_at_correct_radius(self):
        """Outer herb gardens should be at approximately r=48 from origin."""
        gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_8x8"
        ]
        for garden in gardens:
            dx, dz = garden.offset[0], garden.offset[2]
            dist = (dx * dx + dz * dz) ** 0.5
            self.assertAlmostEqual(
                dist, _OUTER_RING_RADIUS, delta=2,
                msg=(
                    f"Outer garden at ({dx},{dz}) distance={dist:.1f} "
                    f"should be ~{_OUTER_RING_RADIUS} (outer ring radius)"
                ),
            )


# ---------------------------------------------------------------------------
# Layout determinism tests
# ---------------------------------------------------------------------------


class LayoutDeterminismTests(unittest.TestCase):
    def test_same_input_same_output(self):
        """Core determinism guarantee: identical input -> identical placements."""
        zone = _make_dan_zong_zone()

        result_a = run_layout(DAN_ZONG_COMPOUND_LAYOUT, zone)
        result_b = run_layout(DAN_ZONG_COMPOUND_LAYOUT, zone)

        self.assertEqual(
            len(result_a.placed), len(result_b.placed),
            f"Placement count differs: {len(result_a.placed)} vs {len(result_b.placed)}",
        )
        for i, (pa, pb) in enumerate(zip(result_a.placed, result_b.placed)):
            self.assertEqual(
                pa.world_pos, pb.world_pos,
                f"Determinism violated at index {i}: "
                f"{pa.world_pos} != {pb.world_pos} for '{pa.payload}'",
            )
            self.assertEqual(pa.rotation, pb.rotation)
            self.assertEqual(pa.kind, pb.kind)
            self.assertEqual(pa.payload, pb.payload)

    def test_all_50_placements_resolved(self):
        zone = _make_dan_zong_zone()
        result = run_layout(DAN_ZONG_COMPOUND_LAYOUT, zone)
        self.assertEqual(
            len(result.placed), 50,
            f"Expected 50 placed structures (49 original + master_sarcophagus P2 #3), "
            f"got {len(result.placed)}",
        )

    def test_poi_center_correct(self):
        zone = _make_dan_zong_zone()
        result = run_layout(DAN_ZONG_COMPOUND_LAYOUT, zone)
        self.assertEqual(
            result.poi_world_pos, (-1600, 82, 4000),
            f"POI center should be (-1600, 82, 4000), got {result.poi_world_pos}",
        )

    def test_great_hall_at_poi_center(self):
        zone = _make_dan_zong_zone()
        result = run_layout(DAN_ZONG_COMPOUND_LAYOUT, zone)
        hall = result.placed[0]
        self.assertEqual(
            hall.world_pos, (-1600, 82, 4000),
            f"Great hall should be at POI center, got {hall.world_pos}",
        )


# ---------------------------------------------------------------------------
# Layout-region density mask tests
# ---------------------------------------------------------------------------


class LayoutDensityMaskTests(unittest.TestCase):
    def test_density_zeroed_inside_layout_radius(self):
        """No wild_herb_clump / scattered_bone / small_rubble inside radius 96."""
        poi_center = (-1600, 4000)
        tile = _make_tile(min_x=-1632, min_z=3968, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)

        compute_layout_density_mask(buffer, poi_center_xz=poi_center, radius=96)

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - poi_center[0]) ** 2 + (wz - poi_center[1]) ** 2
                idx = lz * 64 + lx
                if dist_sq < 96 * 96:
                    self.assertEqual(
                        buffer.layers["flora_density"][idx], 0.0,
                        f"flora_density should be 0 at ({wx},{wz}), "
                        f"inside layout radius 96 (dist={dist_sq**0.5:.1f})",
                    )

    def test_density_preserved_outside_radius(self):
        """Density spawn preserved outside the 96 block radius."""
        poi_center = (-1600, 4000)
        # Tile far from POI center
        tile = _make_tile(min_x=-2300, min_z=3300, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        original = buffer.layers["flora_density"].copy()

        compute_layout_density_mask(buffer, poi_center_xz=poi_center, radius=96)

        # This tile is ~700 blocks from center, well outside radius
        np.testing.assert_array_equal(
            buffer.layers["flora_density"], original,
            err_msg="Density outside layout radius 96 should be preserved",
        )


# ---------------------------------------------------------------------------
# compound_flatten_radius tests
# ---------------------------------------------------------------------------


class CompoundFlattenTests(unittest.TestCase):
    def test_height_flattened_inside_radius(self):
        """POI +/- 96 blocks height = 76 (plan S9.9 requirement)."""
        poi_center = (-1600, 4000)
        tile = _make_tile(min_x=-1632, min_z=3968, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        buffer.layers["height"][:] = np.random.default_rng(42).uniform(60, 95, 64 * 64)

        target_height = 76.0
        apply_compound_flatten(
            buffer, poi_center_xz=poi_center, radius=96, target_height=target_height
        )

        for lx in range(64):
            for lz in range(64):
                wx = tile.min_x + lx
                wz = tile.min_z + lz
                dist_sq = (wx - poi_center[0]) ** 2 + (wz - poi_center[1]) ** 2
                idx = lz * 64 + lx
                if dist_sq < 96 * 96:
                    diff = abs(float(buffer.layers["height"][idx]) - target_height)
                    self.assertLessEqual(
                        diff, 1.0,
                        f"Height at ({wx},{wz}) deviates {diff:.2f} from target {target_height}, "
                        f"exceeds +/-1 tolerance (plan S9.9)",
                    )

    def test_height_preserved_outside_radius(self):
        poi_center = (-1600, 4000)
        tile = _make_tile(min_x=-2300, min_z=3300, tile_size=64)
        buffer = _make_buffer_with_density(tile, tile_size=64)
        original = np.random.default_rng(123).uniform(60, 95, 64 * 64)
        buffer.layers["height"][:] = original.copy()

        apply_compound_flatten(
            buffer, poi_center_xz=poi_center, radius=96, target_height=76.0
        )

        np.testing.assert_array_almost_equal(
            buffer.layers["height"], original, decimal=5,
            err_msg="Height outside flatten radius should be preserved",
        )


# ---------------------------------------------------------------------------
# Herb garden planting rule tests
# ---------------------------------------------------------------------------


class HerbGardenPlantingTests(unittest.TestCase):
    """Verify herb assignment matches plan S9.7 exactly."""

    def test_inner_ring_north_group_is_tuigu_teng(self):
        """Inner ring N/NE/E/SE should be tuigu_teng (蜕骨藤)."""
        for angle in (0, 45, 90, 135):
            self.assertEqual(
                INNER_RING_HERB_ASSIGNMENT[angle], "tuigu_teng",
                f"Inner ring angle {angle} (N/NE/E/SE) should be tuigu_teng",
            )

    def test_inner_ring_south_group_is_shouxin_cao(self):
        """Inner ring S/SW/W/NW should be shouxin_cao (兽心草)."""
        for angle in (180, 225, 270, 315):
            self.assertEqual(
                INNER_RING_HERB_ASSIGNMENT[angle], "shouxin_cao",
                f"Inner ring angle {angle} (S/SW/W/NW) should be shouxin_cao",
            )

    def test_inner_ring_complete(self):
        """Inner ring should have exactly 8 assignments."""
        self.assertEqual(
            len(INNER_RING_HERB_ASSIGNMENT), 8,
            "Inner ring should have 8 herb assignments (one per direction)",
        )

    def test_outer_ring_diagonal_is_xuyuan_rui(self):
        """Outer ring diagonal positions should be xuyuan_rui (续元蕊)."""
        diagonal_angles = (22.5, 112.5, 202.5, 292.5)
        for angle in diagonal_angles:
            self.assertEqual(
                OUTER_RING_HERB_ASSIGNMENT[angle], "xuyuan_rui",
                f"Outer ring diagonal angle {angle} should be xuyuan_rui",
            )

    def test_outer_ring_cardinal_is_longlin_tai(self):
        """Outer ring cardinal-adjacent positions should be longlin_tai (龙鳞苔)."""
        cardinal_angles = (67.5, 157.5, 247.5, 337.5)
        for angle in cardinal_angles:
            self.assertEqual(
                OUTER_RING_HERB_ASSIGNMENT[angle], "longlin_tai",
                f"Outer ring cardinal-adjacent angle {angle} should be longlin_tai",
            )

    def test_outer_ring_complete(self):
        """Outer ring should have exactly 8 assignments."""
        self.assertEqual(
            len(OUTER_RING_HERB_ASSIGNMENT), 8,
            "Outer ring should have 8 herb assignments",
        )

    def test_all_four_herb_types_present(self):
        """All 4 herb types used across inner + outer rings."""
        all_herbs = set(INNER_RING_HERB_ASSIGNMENT.values()) | set(OUTER_RING_HERB_ASSIGNMENT.values())
        expected = {"tuigu_teng", "shouxin_cao", "longlin_tai", "xuyuan_rui"}
        self.assertEqual(
            all_herbs, expected,
            f"Expected all 4 herb types: {expected}, got: {all_herbs}",
        )


# ---------------------------------------------------------------------------
# Layout bagua symmetry tests (Round 2 visual verification)
# ---------------------------------------------------------------------------


class BaguaSymmetryTests(unittest.TestCase):
    """Verify 8-fold rotational symmetry of herb garden rings."""

    def test_inner_ring_8_fold_angular_spacing(self):
        """Inner ring gardens at 0/45/90/.../315 degrees = 45-degree spacing."""
        gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_6x6"
        ]
        import math
        angles = sorted(
            math.degrees(math.atan2(g.offset[2], g.offset[0])) % 360
            for g in gardens
        )
        # Should have 8 angles roughly 45 degrees apart
        self.assertEqual(len(angles), 8)
        for i in range(len(angles)):
            next_i = (i + 1) % len(angles)
            gap = (angles[next_i] - angles[i]) % 360
            self.assertAlmostEqual(
                gap, 45.0, delta=5.0,
                msg=f"Inner ring angular gap {gap:.1f} between {angles[i]:.1f} and {angles[next_i]:.1f} should be ~45deg",
            )

    def test_outer_ring_offset_from_inner(self):
        """Outer ring should be offset ~22.5 degrees from inner ring."""
        import math
        inner_gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_6x6"
        ]
        outer_gardens = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "herb_garden_pen_8x8"
        ]
        inner_angles = sorted(
            math.degrees(math.atan2(g.offset[2], g.offset[0])) % 360
            for g in inner_gardens
        )
        outer_angles = sorted(
            math.degrees(math.atan2(g.offset[2], g.offset[0])) % 360
            for g in outer_gardens
        )
        # Each outer angle should be ~22.5 degrees from nearest inner angle
        for oa in outer_angles:
            min_diff = min(abs((oa - ia) % 360) for ia in inner_angles)
            min_diff = min(min_diff, 360 - min_diff)
            self.assertAlmostEqual(
                min_diff, 22.5, delta=5.0,
                msg=f"Outer angle {oa:.1f} should be ~22.5deg from nearest inner angle",
            )

    def test_furnace_pairs_x_axis_mirror(self):
        """Each furnace pair should be exactly mirrored about x=0."""
        furnaces = [
            p for p in DAN_ZONG_COMPOUND_LAYOUT.placements
            if p.payload == "ruined_open_furnace.nbt"
        ]
        by_z: dict[int, list[int]] = {}
        for f in furnaces:
            z = f.offset[2]
            by_z.setdefault(z, []).append(f.offset[0])
        for z, xs in by_z.items():
            self.assertEqual(
                len(xs), 2,
                f"Furnace pair at z={z} should have exactly 2 furnaces, got {len(xs)}",
            )
            xs.sort()
            self.assertEqual(
                xs[0], -xs[1],
                f"Furnace pair at z={z} should be mirrored: x={xs[0]} and x={xs[1]}",
            )


# ---------------------------------------------------------------------------
# Ambient recipe tests
# ---------------------------------------------------------------------------


class AmbientRecipeTests(unittest.TestCase):
    def setUp(self):
        recipe_path = SERVER_DIR / "assets" / "audio" / "recipes" / "ambient_dan_zong.json"
        with recipe_path.open(encoding="utf-8") as f:
            self.recipe = json.load(f)

    def test_recipe_id(self):
        self.assertEqual(
            self.recipe["id"], "ambient_dan_zong",
            "Ambient recipe id should be 'ambient_dan_zong'",
        )

    def test_recipe_has_layers(self):
        self.assertIn("layers", self.recipe)
        self.assertEqual(
            len(self.recipe["layers"]), 4,
            "Ambient recipe should have 4 layers (plan S9.6)",
        )

    def test_layer_structure(self):
        required_keys = {"sound", "volume", "pitch", "delay_ticks"}
        for i, layer in enumerate(self.recipe["layers"]):
            for key in required_keys:
                self.assertIn(
                    key, layer,
                    f"Layer {i} missing required key '{key}'",
                )

    def test_layer_sounds_are_minecraft(self):
        for layer in self.recipe["layers"]:
            self.assertTrue(
                layer["sound"].startswith("minecraft:"),
                f"Sound '{layer['sound']}' should start with 'minecraft:'",
            )

    def test_layer_volumes_positive(self):
        for layer in self.recipe["layers"]:
            self.assertGreater(
                layer["volume"], 0.0,
                f"Volume for '{layer['sound']}' should be positive",
            )

    def test_layer_delays_ascending(self):
        delays = [layer["delay_ticks"] for layer in self.recipe["layers"]]
        self.assertEqual(
            delays, sorted(delays),
            f"Layer delay_ticks should be in ascending order, got {delays}",
        )

    def test_recipe_has_loop(self):
        self.assertIn("loop", self.recipe)
        self.assertIn("interval_ticks", self.recipe["loop"])

    def test_recipe_category(self):
        self.assertEqual(self.recipe.get("category"), "AMBIENT")


# ---------------------------------------------------------------------------
# Worldview example JSON tests
# ---------------------------------------------------------------------------


class WorldviewExampleZoneTests(unittest.TestCase):
    def setUp(self):
        wv_path = SERVER_DIR / "zones.worldview.example.json"
        with wv_path.open(encoding="utf-8") as f:
            self.data = json.load(f)
        self.zones = self.data["zones"]

    def test_dan_zong_in_worldview(self):
        names = [z["name"] for z in self.zones]
        self.assertIn("dan_zong_yi_yuan", names)

    def test_baolongwang_in_worldview(self):
        names = [z["name"] for z in self.zones]
        self.assertIn("baolongwang_cavern_deep", names)

    def test_dan_zong_has_pois(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertGreater(
            len(zone.get("pois", [])), 0,
            "dan_zong_yi_yuan should have at least 1 POI (百草丹殿)",
        )

    def test_dan_zong_poi_kind_is_ruin(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        poi = zone["pois"][0]
        self.assertEqual(
            poi["kind"], "ruin",
            "百草丹殿 POI kind should be 'ruin'",
        )

    def test_dan_zong_has_worldgen(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertIn("worldgen", zone)
        self.assertEqual(zone["worldgen"]["terrain_profile"], "dan_zong_yi_yuan")

    def test_dan_zong_worldgen_has_architectural_layout(self):
        zone = next(z for z in self.zones if z["name"] == "dan_zong_yi_yuan")
        self.assertEqual(
            zone["worldgen"].get("architectural_layout"), "dan_zong_compound",
        )

    def test_baolongwang_has_2_pois(self):
        zone = next(z for z in self.zones if z["name"] == "baolongwang_cavern_deep")
        self.assertEqual(
            len(zone.get("pois", [])), 2,
            "baolongwang_cavern_deep should have 2 POIs (furnace + spawn)",
        )

    def test_baolongwang_poi_kinds(self):
        zone = next(z for z in self.zones if z["name"] == "baolongwang_cavern_deep")
        kinds = {p["kind"] for p in zone["pois"]}
        self.assertIn("boss_arena", kinds)
        self.assertIn("boss_spawn", kinds)

    def test_baolongwang_negative_qi(self):
        zone = next(z for z in self.zones if z["name"] == "baolongwang_cavern_deep")
        self.assertLess(
            zone["spirit_qi"], 0.0,
            "baolongwang_cavern_deep spirit_qi should be negative (plan S8.1 #3)",
        )


# ---------------------------------------------------------------------------
# Client atmosphere profile tests
# ---------------------------------------------------------------------------


class AtmosphereProfileTests(unittest.TestCase):
    def setUp(self):
        atmo_path = (
            REPO_ROOT / "client" / "src" / "main" / "resources"
            / "assets" / "bong" / "atmosphere" / "dan_zong_yi_yuan.json"
        )
        with atmo_path.open(encoding="utf-8") as f:
            self.profile = json.load(f)

    def test_zone_id(self):
        self.assertEqual(self.profile["zone_id"], "dan_zong_yi_yuan")

    def test_fog_color(self):
        self.assertEqual(
            self.profile["fog_color"], "#5A4060",
            "Fog color should be #5A4060 (dark purple-grey, plan S9.6)",
        )

    def test_fog_density(self):
        self.assertAlmostEqual(self.profile["fog_density"], 0.30, places=2)

    def test_has_ambient_particles(self):
        self.assertGreater(
            len(self.profile["ambient_particles"]), 0,
            "Should have at least one ambient particle config",
        )

    def test_pill_haze_particle(self):
        pill_haze = [
            p for p in self.profile["ambient_particles"]
            if p["type"] == "pill_haze"
        ]
        self.assertEqual(
            len(pill_haze), 1,
            "Should have exactly one pill_haze particle config",
        )
        self.assertEqual(pill_haze[0]["tint"], "#8B6FA5")

    def test_ambient_recipe_id(self):
        self.assertEqual(
            self.profile["ambient_recipe_id"], "ambient_dan_zong",
        )


if __name__ == "__main__":
    unittest.main()
