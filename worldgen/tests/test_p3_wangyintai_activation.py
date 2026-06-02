"""Saturated tests for plan-terrain-wiring-v1 P3 — 王印台端到端激活.

Covers (#4 NBT assets, #6 blueprint/POI, #10 biome, reusability):

NBT assets (#4):
  - 4 NBT files exist in server/structures/wangyintai/ and are non-empty
  - All 4 filenames match what wangyintai_compound.py references
  - Each NBT is non-zero in both file size and block count (via run_layout)

Blueprint / POI (#6):
  - zones.worldview.example.json contains a wangyintai zone entry
  - The zone has terrain_profile="wangyintai" and architectural_layout="wangyintai_compound"
  - The zone has compound_flatten_radius in its height_model
  - The zone has a POI with kind="guantiantai" (matches wangyintai_compound.py poi_kind)
  - The guantiantai POI center is within the wangyintai AABB from zones.json

Layout resolution (17 placements):
  - run_layout with wangyintai zone and guantiantai POI succeeds (no ValueError)
  - run_layout produces exactly 17 placed structures (spec count)
  - All 17 paste_results have block_count > 0
  - The guantiantai_ruins.nbt (central POI structure) is the largest by block count
  - layout is deterministic (two consecutive runs produce identical placed positions)

Biome (#10 append-only, FIX B):
  - BIOME_PALETTE has at least 18 entries (covers wangyintai biome_id=17)
  - Frozen indices 0-11 are unchanged
  - BIOME_PALETTE[12] = plains (rift_mouth_barrens / pseudo_vein_oasis hungry_ring)
  - BIOME_PALETTE[13] = old_growth_pine_taiga (dan_zong_yi_yuan)
  - BIOME_PALETTE[14-16] = plains placeholders (reserved)
  - BIOME_PALETTE[17] = windswept_hills (wangyintai)
  - biome_id=17 does NOT fall back to plains
  - Zone-specific indices (1-11, 13, 17) are unique; plains placeholders may repeat
  - All indices 12-17 are valid minecraft: biome strings

Reusability (P0/P1 zero-new-server-code contract):
  - COMPOUND_LAYOUT_REGISTRY["wangyintai_compound"] exists (P0 infrastructure)
  - wangyintai_compound poi_kind == "guantiantai"
  - wangyintai_compound has exactly 17 placements
  - run_layout for wangyintai uses the same COMPOUND_LAYOUT_REGISTRY entry (no
    special-casing — zone-agnostic, P1 contract)

Export pass (M1 guard):
  - run_layout_pass with wangyintai zone + correct POI kind writes manifest
  - run_layout_pass with wangyintai zone + wrong POI kind warns+skips (no crash)
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts" / "nbt"))

from scripts.terrain_gen.bakers.raster_export import BIOME_PALETTE
from scripts.terrain_gen.blueprint import BlueprintZone, BoundarySpec, PoiSpec, ZoneWorldgenConfig
from scripts.terrain_gen.fields import Bounds2D
from scripts.terrain_gen.layouts import COMPOUND_LAYOUT_REGISTRY, run_layout
from scripts.terrain_gen.layouts.wangyintai_compound import WANGYINTAI_COMPOUND_LAYOUT

REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_DIR = REPO_ROOT / "server"
WANGYINTAI_NBT_DIR = SERVER_DIR / "structures" / "wangyintai"
WANGYINTAI_NBT_BASE = str(WANGYINTAI_NBT_DIR)
ZONES_WORLDVIEW_PATH = SERVER_DIR / "zones.worldview.example.json"


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def _make_wangyintai_zone() -> BlueprintZone:
    """Standard wangyintai zone fixture matching zones.worldview.example.json."""
    return BlueprintZone(
        name="wangyintai",
        display_name="王印台",
        bounds_xz=Bounds2D(min_x=3500, max_x=4500, min_z=-2150, max_z=-1150),
        center_xz=(4000, -1650),
        size_xz=(1000, 1000),
        spirit_qi=-0.15,
        danger_level=3,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="wangyintai",
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=64),
            height_model={"base": [68, 86], "peak": 98, "compound_flatten_radius": 64},
            surface_palette=(
                "smooth_basalt", "deepslate", "calcite", "gray_concrete",
            ),
        ),
        pois=(
            PoiSpec(
                kind="guantiantai",
                pos_xyz=(4000.0, 92.0, -1650.0),
                name="观天台",
                tags=("wangyintai", "vortex_formation", "woliu_path"),
                unlock="found_by_exploration",
                qi_affinity=-0.15,
                danger_bias=1,
            ),
        ),
    )


# ---------------------------------------------------------------------------
# NBT asset existence tests (#4)
# ---------------------------------------------------------------------------


class NbtAssetExistenceTests(unittest.TestCase):
    """4 NBT files must exist in server/structures/wangyintai/ and be non-empty."""

    _EXPECTED_FILES = {
        "jingxuguan_side_hall.nbt",
        "corridor_fragment.nbt",
        "fallen_vortex_disc.nbt",
        "guantiantai_ruins.nbt",
    }

    def test_nbt_directory_exists(self):
        """server/structures/wangyintai/ directory must exist."""
        self.assertTrue(
            WANGYINTAI_NBT_DIR.exists(),
            f"NBT directory {WANGYINTAI_NBT_DIR} must exist (P3 deliverable)",
        )

    def test_all_4_nbt_files_present(self):
        """All 4 expected NBT files must be present on disk."""
        on_disk = {f.name for f in WANGYINTAI_NBT_DIR.glob("*.nbt")}
        for fname in self._EXPECTED_FILES:
            self.assertIn(
                fname,
                on_disk,
                f"{fname} must exist in {WANGYINTAI_NBT_DIR} (P3 NBT deliverable)",
            )

    def test_all_4_nbt_files_nonzero_size(self):
        """Each NBT file must have a positive file size (not an empty placeholder)."""
        for fname in self._EXPECTED_FILES:
            path = WANGYINTAI_NBT_DIR / fname
            if path.exists():
                size = path.stat().st_size
                self.assertGreater(
                    size,
                    0,
                    f"{fname} has file size=0; must contain NBT data",
                )

    def test_layout_references_all_4_nbt_filenames(self):
        """Every .nbt payload in wangyintai_compound.py matches an existing file."""
        for p in WANGYINTAI_COMPOUND_LAYOUT.placements:
            if p.kind == "nbt":
                fname = Path(p.payload).name
                path = WANGYINTAI_NBT_DIR / fname
                self.assertTrue(
                    path.exists(),
                    f"Layout references '{fname}' but file not found at {path}",
                )

    def test_no_dead_nbt_files_on_disk(self):
        """Every .nbt file on disk must be referenced by the layout (no orphans)."""
        on_disk = {f.name for f in WANGYINTAI_NBT_DIR.glob("*.nbt")}
        in_layout = {Path(p.payload).name for p in WANGYINTAI_COMPOUND_LAYOUT.placements
                     if p.kind == "nbt"}
        dead = on_disk - in_layout
        self.assertEqual(
            dead,
            set(),
            f"Dead NBT files not referenced by wangyintai layout: {dead}",
        )


# ---------------------------------------------------------------------------
# Blueprint / POI tests (#6)
# ---------------------------------------------------------------------------


class BlueprintPoiTests(unittest.TestCase):
    """zones.worldview.example.json must contain a valid wangyintai worldgen zone."""

    def _load_zones(self) -> list[dict]:
        with open(ZONES_WORLDVIEW_PATH, encoding="utf-8") as f:
            data = json.load(f)
        return data.get("zones", [])

    def _find_wangyintai(self) -> dict | None:
        for z in self._load_zones():
            if z.get("name") == "wangyintai":
                return z
        return None

    def test_wangyintai_zone_exists_in_worldview(self):
        """zones.worldview.example.json must have a zone named 'wangyintai'."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(
            zone,
            "wangyintai zone is missing from zones.worldview.example.json (#6 blocker)",
        )

    def test_wangyintai_has_worldgen_block(self):
        """wangyintai zone must have a 'worldgen' block."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone, "wangyintai zone must exist")
        self.assertIn(
            "worldgen",
            zone,
            "wangyintai zone must have a 'worldgen' block",
        )

    def test_terrain_profile_is_wangyintai(self):
        """worldgen.terrain_profile must be 'wangyintai'."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone)
        self.assertEqual(
            zone["worldgen"]["terrain_profile"],
            "wangyintai",
            "worldgen.terrain_profile must be 'wangyintai'",
        )

    def test_architectural_layout_is_wangyintai_compound(self):
        """worldgen.architectural_layout must be 'wangyintai_compound'."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone)
        self.assertEqual(
            zone["worldgen"].get("architectural_layout"),
            "wangyintai_compound",
            "worldgen.architectural_layout must be 'wangyintai_compound' (#6)",
        )

    def test_height_model_has_compound_flatten_radius(self):
        """height_model must include compound_flatten_radius."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone)
        height = zone["worldgen"].get("height_model", {})
        self.assertIn(
            "compound_flatten_radius",
            height,
            "height_model must include compound_flatten_radius for layout flatten",
        )
        self.assertGreater(
            height["compound_flatten_radius"],
            0,
            "compound_flatten_radius must be > 0",
        )

    def test_poi_kind_guantiantai_present(self):
        """pois must contain a POI with kind='guantiantai' (#6 / M1 contract)."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone)
        pois = zone.get("pois", [])
        kinds = [p.get("kind") for p in pois]
        self.assertIn(
            "guantiantai",
            kinds,
            "wangyintai pois must contain kind='guantiantai' to satisfy "
            "wangyintai_compound.py poi_kind='guantiantai' (runner.py ValueError guard)",
        )

    def test_guantiantai_poi_within_aabb(self):
        """guantiantai POI position must be inside the wangyintai AABB."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone)
        aabb = zone.get("aabb", {})
        aabb_min = aabb.get("min", [])
        aabb_max = aabb.get("max", [])
        pois = zone.get("pois", [])
        for poi in pois:
            if poi.get("kind") == "guantiantai":
                pos = poi["pos_xyz"]
                for axis, name in enumerate(("x", "y", "z")):
                    self.assertGreaterEqual(
                        pos[axis],
                        aabb_min[axis],
                        f"guantiantai POI {name}={pos[axis]} must be >= aabb.min[{axis}]={aabb_min[axis]}",
                    )
                    self.assertLessEqual(
                        pos[axis],
                        aabb_max[axis],
                        f"guantiantai POI {name}={pos[axis]} must be <= aabb.max[{axis}]={aabb_max[axis]}",
                    )
                break

    def test_guantiantai_poi_y_matches_patrol_anchor(self):
        """guantiantai POI Y should be near patrol anchor Y (consistency check)."""
        zone = self._find_wangyintai()
        self.assertIsNotNone(zone)
        pois = zone.get("pois", [])
        patrol_anchors = zone.get("patrol_anchors", [])
        for poi in pois:
            if poi.get("kind") == "guantiantai":
                poi_y = poi["pos_xyz"][1]
                # Patrol anchors have Y 88-92; POI should be within 10 blocks
                if patrol_anchors:
                    anchor_ys = [a[1] for a in patrol_anchors]
                    min_anchor_y = min(anchor_ys)
                    max_anchor_y = max(anchor_ys)
                    self.assertGreaterEqual(
                        poi_y,
                        min_anchor_y - 10,
                        f"guantiantai POI Y={poi_y} should be near patrol anchors (min_y={min_anchor_y})",
                    )
                break


# ---------------------------------------------------------------------------
# Layout resolution tests (17 placements)
# ---------------------------------------------------------------------------


class LayoutResolutionTests(unittest.TestCase):
    """run_layout with wangyintai zone + guantiantai POI produces 17 placements."""

    def setUp(self):
        self.zone = _make_wangyintai_zone()
        self.result = run_layout(
            WANGYINTAI_COMPOUND_LAYOUT, self.zone, nbt_base_dir=WANGYINTAI_NBT_BASE
        )

    def test_run_layout_succeeds_no_valueerror(self):
        """run_layout must NOT raise ValueError for wangyintai + guantiantai POI."""
        # setUp already called run_layout; reaching here means no ValueError
        self.assertIsNotNone(self.result, "run_layout should return a LayoutResult")

    def test_exactly_17_placed_structures(self):
        """run_layout must produce exactly 17 placed structures."""
        self.assertEqual(
            len(self.result.placed),
            17,
            f"Expected 17 placed structures (1 central + 4 side halls + "
            f"4 corridors + 8 fallen discs), got {len(self.result.placed)}",
        )

    def test_exactly_17_paste_results(self):
        """paste_results must have exactly 17 entries (one per placement)."""
        self.assertEqual(
            len(self.result.paste_results),
            17,
            f"Expected 17 paste_results, got {len(self.result.paste_results)}",
        )

    def test_all_paste_results_nonzero_blocks(self):
        """Every paste_result must produce > 0 blocks (all NBT files exist and non-empty)."""
        for i, (pr, pl) in enumerate(
            zip(self.result.paste_results, self.result.placed)
        ):
            self.assertGreater(
                pr.block_count,
                0,
                f"Placement {i} '{pl.payload}' has block_count=0; "
                f"NBT file must exist and be non-empty",
            )

    def test_guantiantai_ruins_is_largest(self):
        """guantiantai_ruins.nbt (central POI anchor) must have the most blocks."""
        ruins_results = [
            pr for pr, pl in zip(self.result.paste_results, self.result.placed)
            if "guantiantai_ruins" in pl.payload
        ]
        self.assertEqual(
            len(ruins_results),
            1,
            "There must be exactly 1 guantiantai_ruins placement",
        )
        ruins_count = ruins_results[0].block_count
        all_counts = [pr.block_count for pr in self.result.paste_results]
        self.assertEqual(
            max(all_counts),
            ruins_count,
            f"guantiantai_ruins must be the largest structure; "
            f"ruins={ruins_count}, max={max(all_counts)}",
        )

    def test_poi_center_used_as_anchor(self):
        """run_layout must use the guantiantai POI as the anchor point."""
        self.assertEqual(
            self.result.poi_world_pos,
            (4000, 92, -1650),
            f"POI anchor should be (4000, 92, -1650), got {self.result.poi_world_pos}",
        )

    def test_guantiantai_ruins_at_poi_center(self):
        """guantiantai_ruins (offset 0,0,0) must be placed exactly at POI center."""
        for pr, pl in zip(self.result.paste_results, self.result.placed):
            if "guantiantai_ruins" in pl.payload:
                self.assertEqual(
                    pl.world_pos,
                    (4000, 92, -1650),
                    f"guantiantai_ruins world_pos should equal POI center, "
                    f"got {pl.world_pos}",
                )
                break

    def test_total_block_count_nonzero(self):
        """Total block count across all 17 placements must be > 0."""
        total = sum(pr.block_count for pr in self.result.paste_results)
        self.assertGreater(
            total,
            0,
            "Total block count across all wangyintai placements must be > 0",
        )

    def test_placement_count_matches_layout_spec(self):
        """Layout spec must have exactly 17 placements."""
        self.assertEqual(
            len(WANGYINTAI_COMPOUND_LAYOUT.placements),
            17,
            f"WANGYINTAI_COMPOUND_LAYOUT must have 17 placements, "
            f"got {len(WANGYINTAI_COMPOUND_LAYOUT.placements)}",
        )


# ---------------------------------------------------------------------------
# Determinism tests
# ---------------------------------------------------------------------------


class DeterminismTests(unittest.TestCase):
    """run_layout for wangyintai must be deterministic."""

    def test_two_runs_produce_identical_world_positions(self):
        """Same zone + layout → same placed structure world positions."""
        zone = _make_wangyintai_zone()
        r1 = run_layout(WANGYINTAI_COMPOUND_LAYOUT, zone,
                        nbt_base_dir=WANGYINTAI_NBT_BASE)
        r2 = run_layout(WANGYINTAI_COMPOUND_LAYOUT, zone,
                        nbt_base_dir=WANGYINTAI_NBT_BASE)
        positions1 = [p.world_pos for p in r1.placed]
        positions2 = [p.world_pos for p in r2.placed]
        self.assertEqual(
            positions1,
            positions2,
            "Wangyintai layout must be deterministic (same input → same output)",
        )

    def test_two_runs_produce_identical_paste_block_counts(self):
        """Same zone + layout → same block counts per placement."""
        zone = _make_wangyintai_zone()
        r1 = run_layout(WANGYINTAI_COMPOUND_LAYOUT, zone,
                        nbt_base_dir=WANGYINTAI_NBT_BASE)
        r2 = run_layout(WANGYINTAI_COMPOUND_LAYOUT, zone,
                        nbt_base_dir=WANGYINTAI_NBT_BASE)
        counts1 = [pr.block_count for pr in r1.paste_results]
        counts2 = [pr.block_count for pr in r2.paste_results]
        self.assertEqual(
            counts1,
            counts2,
            "Block counts must be identical across two runs (determinism)",
        )


# ---------------------------------------------------------------------------
# Biome palette tests (#10)
# ---------------------------------------------------------------------------


class BiomePaletteP3Tests(unittest.TestCase):
    """BIOME_PALETTE covers wangyintai biome_id=17; frozen indices 0-11 unchanged."""

    def test_palette_length_covers_index_17(self):
        """BIOME_PALETTE must have at least 18 entries to cover wangyintai biome_id=17."""
        self.assertGreaterEqual(
            len(BIOME_PALETTE),
            18,
            f"BIOME_PALETTE length={len(BIOME_PALETTE)} must be >= 18 "
            f"to cover wangyintai biome_id=17",
        )

    def test_index_17_is_valid_minecraft_biome(self):
        """BIOME_PALETTE[17] must be a non-empty minecraft: biome name."""
        biome_17 = BIOME_PALETTE[17]
        self.assertTrue(
            biome_17.startswith("minecraft:"),
            f"BIOME_PALETTE[17]={biome_17!r} must start with 'minecraft:'",
        )
        self.assertGreater(
            len(biome_17),
            len("minecraft:"),
            f"BIOME_PALETTE[17] must have a non-empty biome name after 'minecraft:'",
        )

    def test_biome_id_17_does_not_fall_back_to_plains(self):
        """biome_id=17 must NOT silently degrade to plains."""
        biome_id = 17
        biome = BIOME_PALETTE[biome_id] if biome_id < len(BIOME_PALETTE) else BIOME_PALETTE[0]
        self.assertNotEqual(
            biome,
            "minecraft:plains",
            "biome_id=17 must NOT fall back to 'minecraft:plains'; "
            "BIOME_PALETTE must cover index 17 with a distinct biome",
        )

    def test_frozen_indices_0_to_11_unchanged(self):
        """Indices 0-11 must match the original frozen palette (P2 regression)."""
        expected_frozen = (
            "minecraft:plains",
            "minecraft:stony_peaks",
            "minecraft:swamp",
            "minecraft:badlands",
            "minecraft:meadow",
            "minecraft:dripstone_caves",
            "minecraft:desert",
            "minecraft:forest",           # index 7 — forest_wilderness_biome
            "minecraft:river",            # index 8 — river_wilderness_biome
            "minecraft:frozen_peaks",
            "minecraft:mangrove_swamp",
            "minecraft:flower_forest",    # index 11
        )
        for i, expected in enumerate(expected_frozen):
            self.assertEqual(
                BIOME_PALETTE[i],
                expected,
                f"BIOME_PALETTE[{i}] changed from {expected!r} to {BIOME_PALETTE[i]!r}; "
                f"indices 0-11 are frozen (raster.rs hardcodes forest=7, river=8)",
            )

    def test_dan_zong_index_13_still_present(self):
        """Index 13 must map to dan_zong biome (FIX B: dan_zong moved from 12 → 13)."""
        self.assertGreaterEqual(len(BIOME_PALETTE), 14)
        self.assertEqual(
            BIOME_PALETTE[13],
            "minecraft:old_growth_pine_taiga",
            "BIOME_PALETTE[13] must be minecraft:old_growth_pine_taiga (dan_zong FIX B: moved from 12→13)",
        )

    def test_index_12_is_plains_for_rift_mouth(self):
        """Index 12 must be plains (rift_mouth_barrens / pseudo_vein_oasis hungry_ring contract).

        FIX B: prior versions had dan_zong at 12, causing those zones to get pine_taiga.
        """
        self.assertGreaterEqual(len(BIOME_PALETTE), 13)
        self.assertEqual(
            BIOME_PALETTE[12],
            "minecraft:plains",
            "BIOME_PALETTE[12] must be 'minecraft:plains' (rift_mouth_barrens / hungry_ring — FIX B)",
        )

    def test_all_new_indices_13_to_17_are_valid_biomes(self):
        """Indices 13-17 must all be non-empty minecraft: biome strings."""
        for i in range(13, 18):
            self.assertGreater(
                len(BIOME_PALETTE),
                i,
                f"BIOME_PALETTE must have index {i} (needed for P3 wangyintai biome)",
            )
            entry = BIOME_PALETTE[i]
            self.assertTrue(
                entry.startswith("minecraft:"),
                f"BIOME_PALETTE[{i}]={entry!r} must be a 'minecraft:' biome string",
            )

    def test_no_duplicate_entries_among_zone_biomes(self):
        """Zone-specific biomes must be unique; plains placeholders (0, 12, 14-16) may repeat.

        FIX B: indices 12, 14-16 are intentional plains placeholders.  Only zone-specific
        indices (1-11, 13, 17) must be unique.
        """
        _PLAINS_PLACEHOLDER_INDICES = frozenset({0, 12, 14, 15, 16})
        seen: dict[str, int] = {}
        for i, entry in enumerate(BIOME_PALETTE):
            if i in _PLAINS_PLACEHOLDER_INDICES:
                self.assertEqual(
                    entry,
                    "minecraft:plains",
                    f"BIOME_PALETTE[{i}] is a plains-placeholder index but contains {entry!r}",
                )
                continue
            self.assertNotIn(
                entry,
                seen,
                f"BIOME_PALETTE[{i}]={entry!r} duplicates index {seen.get(entry)}: "
                f"zone-specific indices must be unique",
            )
            seen[entry] = i

    def test_bake_assertion_passes_for_wangyintai_biome_id(self):
        """biome_id=17 (wangyintai) must pass the bake assertion."""
        palette_len = len(BIOME_PALETTE)
        wangyintai_biome_id = 17
        assert wangyintai_biome_id < palette_len, (
            f"wangyintai biome_id=17 must be < len(BIOME_PALETTE)={palette_len}; "
            f"BIOME_PALETTE is too short"
        )


# ---------------------------------------------------------------------------
# Reusability tests (P0/P1 zero-new-server-code contract)
# ---------------------------------------------------------------------------


class ReusabilityTests(unittest.TestCase):
    """Wang Yintai layout reuses the same P0/P1 infrastructure; zero new server code."""

    def test_registry_contains_wangyintai_compound(self):
        """COMPOUND_LAYOUT_REGISTRY must include 'wangyintai_compound' (P0 contract)."""
        self.assertIn(
            "wangyintai_compound",
            COMPOUND_LAYOUT_REGISTRY,
            "COMPOUND_LAYOUT_REGISTRY must contain 'wangyintai_compound' (P0 pass-through)",
        )

    def test_wangyintai_poi_kind_is_guantiantai(self):
        """WANGYINTAI_COMPOUND_LAYOUT.poi_kind must be 'guantiantai'."""
        self.assertEqual(
            WANGYINTAI_COMPOUND_LAYOUT.poi_kind,
            "guantiantai",
            "WANGYINTAI_COMPOUND_LAYOUT.poi_kind must be 'guantiantai'",
        )

    def test_wangyintai_layout_has_17_placements(self):
        """WANGYINTAI_COMPOUND_LAYOUT must have exactly 17 placements."""
        self.assertEqual(
            len(WANGYINTAI_COMPOUND_LAYOUT.placements),
            17,
            f"WANGYINTAI_COMPOUND_LAYOUT must have 17 placements "
            f"(1 central + 4 halls + 4 corridors + 8 discs), "
            f"got {len(WANGYINTAI_COMPOUND_LAYOUT.placements)}",
        )

    def test_registry_entry_is_same_object_as_module_const(self):
        """COMPOUND_LAYOUT_REGISTRY['wangyintai_compound'] must be WANGYINTAI_COMPOUND_LAYOUT."""
        self.assertIs(
            COMPOUND_LAYOUT_REGISTRY["wangyintai_compound"],
            WANGYINTAI_COMPOUND_LAYOUT,
            "Registry must reference the canonical WANGYINTAI_COMPOUND_LAYOUT constant",
        )

    def test_run_layout_uses_registry_not_special_case(self):
        """Calling run_layout via COMPOUND_LAYOUT_REGISTRY lookup produces same result."""
        zone = _make_wangyintai_zone()
        # Via direct const
        r1 = run_layout(WANGYINTAI_COMPOUND_LAYOUT, zone, nbt_base_dir=WANGYINTAI_NBT_BASE)
        # Via registry lookup
        spec_from_registry = COMPOUND_LAYOUT_REGISTRY["wangyintai_compound"]
        r2 = run_layout(spec_from_registry, zone, nbt_base_dir=WANGYINTAI_NBT_BASE)
        self.assertEqual(
            len(r1.placed),
            len(r2.placed),
            "Direct const and registry lookup must produce same placement count",
        )
        for p1, p2 in zip(r1.placed, r2.placed):
            self.assertEqual(
                p1.world_pos,
                p2.world_pos,
                "Placement positions must be identical via const vs registry",
            )

    def test_missing_poi_kind_raises_value_error(self):
        """Zone without guantiantai POI raises ValueError (M1 guard — not silent)."""
        zone_no_poi = BlueprintZone(
            name="wangyintai",
            display_name="王印台",
            bounds_xz=Bounds2D(min_x=3500, max_x=4500, min_z=-2150, max_z=-1150),
            center_xz=(4000, -1650),
            size_xz=(1000, 1000),
            spirit_qi=-0.15,
            danger_level=3,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="wangyintai",
                shape="ellipse",
                boundary=BoundarySpec(mode="soft", width=64),
                height_model={"base": [68, 86], "peak": 98},
                surface_palette=("smooth_basalt",),
            ),
            pois=(),  # no guantiantai POI
        )
        with self.assertRaises(ValueError) as ctx:
            run_layout(WANGYINTAI_COMPOUND_LAYOUT, zone_no_poi)
        self.assertIn(
            "guantiantai",
            str(ctx.exception),
            "ValueError must mention the missing poi_kind 'guantiantai'",
        )


# ---------------------------------------------------------------------------
# Export pass tests (M1 guard)
# ---------------------------------------------------------------------------


class ExportPassWangyintaiTests(unittest.TestCase):
    """run_layout_pass writes manifest for wangyintai zone; warns+skips without POI."""

    def _write_blueprint(self, tmpdir: str, poi_kinds: list[str]) -> Path:
        """Write a minimal blueprint JSON with wangyintai zone."""
        import tempfile

        pois = [{"kind": k, "pos_xyz": [4000.0, 92.0, -1650.0]} for k in poi_kinds]
        data = {
            "version": 1,
            "world": {
                "name": "w",
                "spawn_zone": "s",
                "bounds_xz": {"min": [3000, -2500], "max": [5000, -800]},
                "notes": [],
            },
            "zones": [
                {
                    "name": "wangyintai",
                    "display_name": "王印台",
                    "aabb": {"min": [3500.0, 40.0, -2150.0], "max": [4500.0, 200.0, -1150.0]},
                    "center_xz": [4000.0, -1650.0],
                    "size_xz": [1000.0, 1000.0],
                    "spirit_qi": -0.15,
                    "danger_level": 3,
                    "worldgen": {
                        "terrain_profile": "wangyintai",
                        "shape": "ellipse",
                        "boundary": {"mode": "soft", "width": 64},
                        "height_model": {"base": [68, 86], "peak": 98},
                        "surface_palette": ["smooth_basalt"],
                        "architectural_layout": "wangyintai_compound",
                    },
                    "pois": pois,
                }
            ],
        }
        path = Path(tmpdir) / "zones.json"
        path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def test_wangyintai_zone_with_poi_writes_manifest(self):
        """Zone with architectural_layout=wangyintai_compound and guantiantai POI → manifest."""
        import tempfile

        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_blueprint(tmpdir, poi_kinds=["guantiantai"])
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            manifest_path = run_layout_pass(bp, output_dir)

            self.assertIsNotNone(
                manifest_path,
                "run_layout_pass must return a Path when wangyintai layout is processed",
            )
            self.assertTrue(
                manifest_path.exists(),
                "placement_manifest.json must be written to disk",
            )
            with open(manifest_path) as f:
                manifest = json.load(f)
            self.assertEqual(manifest["version"], 1)
            self.assertGreater(
                len(manifest["structures"]),
                0,
                "Manifest must have structures for wangyintai layout zone",
            )

    def test_wangyintai_zone_without_poi_warns_skips(self):
        """Zone with wrong POI kind → warn+skip, no crash, None returned (M1 guard)."""
        import tempfile

        from scripts.terrain_gen.blueprint import load_blueprint
        from scripts.terrain_gen.__main__ import run_layout_pass

        with tempfile.TemporaryDirectory() as tmpdir:
            bp_path = self._write_blueprint(tmpdir, poi_kinds=["wrong_kind"])
            bp = load_blueprint(bp_path)
            output_dir = Path(tmpdir) / "output"
            output_dir.mkdir()

            with self.assertLogs("scripts.terrain_gen.__main__", level="WARNING") as cm:
                result = run_layout_pass(bp, output_dir)

            self.assertIsNone(
                result,
                "Missing POI → no manifest written → None returned (M1 guard)",
            )
            warning_text = " ".join(cm.output)
            self.assertIn(
                "wangyintai",
                warning_text,
                "Warning must mention the zone name 'wangyintai'",
            )


if __name__ == "__main__":
    unittest.main()
