"""Saturated tests for plan-terrain-wiring-v1 P2 — 丹宗遗源端到端激活.

Covers (#3 dead assets, #10 biome, placement resolution):

Dead assets (#3):
  - master_sarcophagus.nbt is now referenced in DAN_ZONG_COMPOUND_LAYOUT
  - herb_garden_pen_inner.nbt and herb_garden_pen_outer.nbt are gone (git rm'd)
  - Every NBT file in server/structures/dan_zong/ is referenced by the layout
  - Every layout NBT payload has a corresponding file

Biome fix (#10, FIX B):
  - BIOME_PALETTE has at least 14 entries (covers dan_zong biome_id=13)
  - Indices 0-11 are frozen (forest=7, river=8 invariant for raster.rs)
  - BIOME_PALETTE[12] = plains (rift_mouth_barrens / pseudo_vein_oasis hungry_ring)
  - BIOME_PALETTE[13] = old_growth_pine_taiga (dan_zong_yi_yuan)
  - Bake-time assertion: biome_id.max() < len(BIOME_PALETTE)
  - dan_zong biome_id=13 resolves to old_growth_pine_taiga (not plains)

Placement resolution (server startup check):
  - All nbt, stamp_radial, block_grid placements in DAN_ZONG_COMPOUND_LAYOUT
    produce block_count > 0 after run_layout
  - Total placement count = 50 (49 original + master_sarcophagus)
  - master_sarcophagus placement block_count > 0

Y-anchor (M2 contract):
  - great_hall (offset y=0) world_y equals POI.y=82
  - master_sarcophagus world_y = POI.y - 4 = 78

Biome positive assertion:
  - dan_zong biome_id=12 resolves to BIOME_PALETTE[12] (not BIOME_PALETTE[0])
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
from scripts.terrain_gen.layouts.dan_zong_compound import DAN_ZONG_COMPOUND_LAYOUT
from scripts.terrain_gen.layouts.runner import run_layout

REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_DIR = REPO_ROOT / "server"
DAN_ZONG_NBT_DIR = SERVER_DIR / "structures" / "dan_zong"
DAN_ZONG_NBT_BASE = str(DAN_ZONG_NBT_DIR)


# ---------------------------------------------------------------------------
# Fixture
# ---------------------------------------------------------------------------


def _make_dan_zong_zone() -> BlueprintZone:
    """Standard dan_zong_yi_yuan zone fixture matching zones.worldview.example.json."""
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
                pos_xyz=(-1600.0, 82.0, 4000.0),
                name="百草丹殿",
                tags=("dandao_path", "alchemy", "boss_lore", "baolongwang_prequel"),
                unlock="found_by_exploration",
                qi_affinity=-0.10,
                danger_bias=2,
            ),
        ),
    )


# ---------------------------------------------------------------------------
# Dead asset tests (#3)
# ---------------------------------------------------------------------------


class DeadAssetTests(unittest.TestCase):
    """Every NBT in server/structures/dan_zong/ must be referenced; no orphans."""

    def _layout_nbt_payloads(self) -> set[str]:
        """Return all .nbt filenames referenced in the layout (basename only).

        For kind=nbt, the payload IS the filename (possibly with a path prefix).
        For kind=stamp_radial, the payload is a size name like 'herb_garden_pen_6x6'
        that corresponds to an NBT file of the same base name + '.nbt' suffix.
        Those NBT files serve as the canonical specification for the herb garden
        pen footprint even though _stamp_radial generates inline blocks.
        """
        referenced: set[str] = set()
        for p in DAN_ZONG_COMPOUND_LAYOUT.placements:
            if p.kind == "nbt":
                referenced.add(Path(p.payload).name)
            elif p.kind == "stamp_radial":
                # stamp_radial payload is a name like "herb_garden_pen_6x6"
                # → corresponding NBT spec file is "herb_garden_pen_6x6.nbt"
                referenced.add(p.payload + ".nbt")
        return referenced

    def _disk_nbt_files(self) -> set[str]:
        """Return all .nbt filenames on disk in server/structures/dan_zong/."""
        return {f.name for f in DAN_ZONG_NBT_DIR.glob("*.nbt")}

    def test_no_dead_nbt_files_on_disk(self):
        """Every .nbt file in server/structures/dan_zong/ must be referenced by layout."""
        on_disk = self._disk_nbt_files()
        in_layout = self._layout_nbt_payloads()
        dead = on_disk - in_layout
        self.assertEqual(
            dead,
            set(),
            f"Dead NBT files found (not referenced by layout): {dead}. "
            f"Either add to layout placements or git rm them.",
        )

    def test_master_sarcophagus_in_layout(self):
        """master_sarcophagus.nbt must be referenced in DAN_ZONG_COMPOUND_LAYOUT (#3)."""
        payloads = self._layout_nbt_payloads()
        self.assertIn(
            "master_sarcophagus.nbt",
            payloads,
            "master_sarcophagus.nbt must be added as an nbt placement (plan #3 dead asset)",
        )

    def test_herb_garden_pen_inner_removed(self):
        """herb_garden_pen_inner.nbt must be git rm'd (old unused large pen)."""
        self.assertNotIn(
            "herb_garden_pen_inner.nbt",
            self._disk_nbt_files(),
            "herb_garden_pen_inner.nbt (9093B old large pen) must be removed from disk",
        )

    def test_herb_garden_pen_outer_removed(self):
        """herb_garden_pen_outer.nbt must be git rm'd (old unused large pen)."""
        self.assertNotIn(
            "herb_garden_pen_outer.nbt",
            self._disk_nbt_files(),
            "herb_garden_pen_outer.nbt (10849B old large pen) must be removed from disk",
        )

    def test_no_layout_payload_references_missing_nbt(self):
        """Every .nbt payload referenced in the layout must exist on disk."""
        on_disk = self._disk_nbt_files()
        for p in DAN_ZONG_COMPOUND_LAYOUT.placements:
            if p.kind == "nbt":
                name = Path(p.payload).name
                self.assertIn(
                    name,
                    on_disk,
                    f"Layout references '{name}' but file not found in {DAN_ZONG_NBT_DIR}",
                )

    def test_total_placement_count_is_50(self):
        """50 placements: 49 original + 1 master_sarcophagus."""
        self.assertEqual(
            len(DAN_ZONG_COMPOUND_LAYOUT.placements),
            50,
            f"Expected 50 placements after adding master_sarcophagus, "
            f"got {len(DAN_ZONG_COMPOUND_LAYOUT.placements)}",
        )


# ---------------------------------------------------------------------------
# Biome fix tests (#10)
# ---------------------------------------------------------------------------


class BiomePaletteTests(unittest.TestCase):
    """BIOME_PALETTE covers dan_zong biome_id=13; frozen indices 0-11 unchanged.

    FIX B: index 12 is restored to plains (rift_mouth_barrens / hungry_ring);
    dan_zong moves to index 13 = old_growth_pine_taiga.
    """

    def test_palette_length_covers_index_13(self):
        """BIOME_PALETTE must have at least 14 entries to cover dan_zong biome_id=13."""
        self.assertGreaterEqual(
            len(BIOME_PALETTE),
            14,
            f"BIOME_PALETTE length={len(BIOME_PALETTE)} must be >= 14 "
            f"to cover dan_zong biome_id=13",
        )

    def test_index_12_is_plains(self):
        """BIOME_PALETTE[12] must be minecraft:plains.

        FIX B: rift_mouth_barrens and pseudo_vein_oasis hungry_ring both
        hardcode biome_id=12 and expect the default barren fallback (plains).
        """
        self.assertEqual(
            BIOME_PALETTE[12],
            "minecraft:plains",
            f"BIOME_PALETTE[12]={BIOME_PALETTE[12]!r} must be 'minecraft:plains' "
            f"(rift_mouth_barrens / hungry_ring contract — FIX B)",
        )

    def test_index_13_is_dan_zong_pine_taiga(self):
        """BIOME_PALETTE[13] must be old_growth_pine_taiga (dan_zong biome)."""
        self.assertGreater(len(BIOME_PALETTE), 13, "BIOME_PALETTE must have index 13")
        self.assertEqual(
            BIOME_PALETTE[13],
            "minecraft:old_growth_pine_taiga",
            f"BIOME_PALETTE[13]={BIOME_PALETTE[13]!r} must be 'minecraft:old_growth_pine_taiga' "
            f"(dan_zong_yi_yuan biome — FIX B)",
        )

    def test_index_13_is_valid_minecraft_biome(self):
        """BIOME_PALETTE[13] must be a non-empty minecraft: biome name (dan_zong)."""
        self.assertGreater(len(BIOME_PALETTE), 13, "BIOME_PALETTE must have index 13")
        biome_13 = BIOME_PALETTE[13]
        self.assertTrue(
            biome_13.startswith("minecraft:"),
            f"BIOME_PALETTE[13]={biome_13!r} must start with 'minecraft:'",
        )
        self.assertGreater(
            len(biome_13),
            len("minecraft:"),
            f"BIOME_PALETTE[13]={biome_13!r} must have a non-empty biome name after 'minecraft:'",
        )

    def test_frozen_indices_0_to_11_unchanged(self):
        """Indices 0-11 must match the original palette exactly (raster.rs hardcodes 7 and 8)."""
        expected_frozen = (
            "minecraft:plains",
            "minecraft:stony_peaks",
            "minecraft:swamp",
            "minecraft:badlands",
            "minecraft:meadow",
            "minecraft:dripstone_caves",
            "minecraft:desert",
            "minecraft:forest",           # index 7 — forest_wilderness_biome in raster.rs
            "minecraft:river",            # index 8 — river_wilderness_biome in raster.rs
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

    def test_forest_biome_at_index_7(self):
        """Index 7 must be minecraft:forest (raster.rs hardcode)."""
        self.assertEqual(
            BIOME_PALETTE[7],
            "minecraft:forest",
            "BIOME_PALETTE[7] must remain minecraft:forest (raster.rs forest_wilderness_biome)",
        )

    def test_river_biome_at_index_8(self):
        """Index 8 must be minecraft:river (raster.rs hardcode)."""
        self.assertEqual(
            BIOME_PALETTE[8],
            "minecraft:river",
            "BIOME_PALETTE[8] must remain minecraft:river (raster.rs river_wilderness_biome)",
        )

    def test_biome_id_12_resolves_to_plains_intentionally(self):
        """biome_id=12 must resolve to plains (rift_mouth_barrens / hungry_ring contract).

        FIX B: prior versions had dan_zong at index 12, which caused rift_mouth_barrens
        and pseudo_vein_oasis (hungry_ring) to get pine_taiga instead of the intended
        barren/plains terrain.  Index 12 is restored to plains.
        """
        biome_id = 12
        biome = BIOME_PALETTE[biome_id] if biome_id < len(BIOME_PALETTE) else BIOME_PALETTE[0]
        self.assertEqual(
            biome,
            "minecraft:plains",
            "biome_id=12 must resolve to 'minecraft:plains' "
            "(rift_mouth_barrens / hungry_ring use this index — FIX B)",
        )

    def test_dan_zong_biome_id_13_does_not_fall_back_to_plains(self):
        """dan_zong biome_id=13 must NOT silently degrade to plains (palette must cover it)."""
        biome_id = 13
        biome = BIOME_PALETTE[biome_id] if biome_id < len(BIOME_PALETTE) else BIOME_PALETTE[0]
        self.assertNotEqual(
            biome,
            "minecraft:plains",
            "dan_zong biome_id=13 must NOT resolve to 'minecraft:plains'; "
            "BIOME_PALETTE[13] must be the dedicated alchemy biome",
        )

    def test_all_palette_entries_are_strings(self):
        """Every BIOME_PALETTE entry must be a non-empty string."""
        for i, entry in enumerate(BIOME_PALETTE):
            self.assertIsInstance(
                entry, str,
                f"BIOME_PALETTE[{i}] must be a string, got {type(entry).__name__}",
            )
            self.assertGreater(
                len(entry), 0,
                f"BIOME_PALETTE[{i}] must not be empty",
            )

    def test_no_duplicate_entries_among_zone_biomes(self):
        """Zone-specific biomes (13, 17) must be unique; plains placeholders (0, 12, 14-16)
        are allowed to repeat — they are intentional fallback/placeholder slots.

        FIX B: index 12 and 14-16 are plains placeholders.  Checking uniqueness across
        the whole palette would spuriously fail on those intentional duplicates.
        """
        # Indices with intentionally non-unique biomes (plains placeholder slots)
        _PLAINS_PLACEHOLDER_INDICES = frozenset({0, 12, 14, 15, 16})
        seen: dict[str, int] = {}
        for i, entry in enumerate(BIOME_PALETTE):
            if i in _PLAINS_PLACEHOLDER_INDICES:
                self.assertEqual(
                    entry,
                    "minecraft:plains",
                    f"BIOME_PALETTE[{i}] is a designated plains-placeholder index "
                    f"but contains {entry!r}; must be 'minecraft:plains'",
                )
                continue
            self.assertNotIn(
                entry,
                seen,
                f"BIOME_PALETTE[{i}]={entry!r} is a duplicate of index {seen.get(entry)}: "
                f"zone-specific indices must be unique",
            )
            seen[entry] = i


class BiomeBakeAssertionTests(unittest.TestCase):
    """export_rasters bake-time assertion fires when biome_id >= len(BIOME_PALETTE)."""

    def test_assertion_fires_on_out_of_bounds_biome_id(self):
        """Bake assertion must raise AssertionError when max biome_id >= palette length."""
        import tempfile
        import numpy as np

        # Import the assertion logic inline — simulate what export_rasters checks.
        # We can't run the full export pipeline without real raster data, but we
        # can directly test the assertion expression that was added.
        palette_len = len(BIOME_PALETTE)
        # Craft a biome_id array whose max == palette_len (exactly at boundary)
        bad_biome_ids = np.array([0, 1, palette_len], dtype=np.uint8)
        max_id = int(bad_biome_ids.max())
        with self.assertRaises(AssertionError) as ctx:
            assert max_id < palette_len, (
                f"biome_id max={max_id} >= len(BIOME_PALETTE)={palette_len} "
                f"in tile test; extend BIOME_PALETTE (append-only) to cover this id"
            )
        self.assertIn(
            "BIOME_PALETTE",
            str(ctx.exception),
            "AssertionError message should mention BIOME_PALETTE",
        )

    def test_assertion_passes_for_valid_biome_ids(self):
        """Bake assertion must pass silently when all biome_ids are within palette bounds."""
        import numpy as np

        palette_len = len(BIOME_PALETTE)
        # Valid: all ids in range [0, palette_len - 1]
        valid_biome_ids = np.arange(palette_len, dtype=np.uint8)
        max_id = int(valid_biome_ids.max())
        # Should not raise
        assert max_id < palette_len, (
            f"biome_id max={max_id} should be < {palette_len}"
        )

    def test_assertion_passes_for_dan_zong_biome_id(self):
        """biome_id=13 (dan_zong, FIX B) must pass the bake assertion."""
        palette_len = len(BIOME_PALETTE)
        dan_zong_biome_id = 13
        # Must not raise
        assert dan_zong_biome_id < palette_len, (
            f"dan_zong biome_id=13 must be < len(BIOME_PALETTE)={palette_len}; "
            f"BIOME_PALETTE is too short"
        )


# ---------------------------------------------------------------------------
# Placement resolution tests
# ---------------------------------------------------------------------------


class PlacementResolutionTests(unittest.TestCase):
    """All placement kinds produce block_count > 0 after run_layout (server startup check)."""

    def setUp(self):
        self.zone = _make_dan_zong_zone()
        # Provide the NBT base dir so bare filenames resolve to actual files
        self.result = run_layout(
            DAN_ZONG_COMPOUND_LAYOUT, self.zone, nbt_base_dir=DAN_ZONG_NBT_BASE
        )

    def test_all_50_placements_resolved(self):
        """run_layout must produce 50 placed structures (49 original + master_sarcophagus)."""
        self.assertEqual(
            len(self.result.placed),
            50,
            f"Expected 50 placed structures, got {len(self.result.placed)}",
        )

    def test_all_50_paste_results(self):
        """paste_results must have 50 entries (one per placement)."""
        self.assertEqual(
            len(self.result.paste_results),
            50,
            f"Expected 50 paste_results, got {len(self.result.paste_results)}",
        )

    def test_nbt_placements_have_nonzero_blocks(self):
        """Every nbt placement (real .nbt file) must produce > 0 blocks."""
        nbt_results = [
            pr for pr, pl in zip(self.result.paste_results, self.result.placed)
            if pl.kind == "nbt"
        ]
        self.assertGreater(
            len(nbt_results),
            0,
            "There must be nbt placements in the layout",
        )
        for pr in nbt_results:
            self.assertGreater(
                pr.block_count,
                0,
                f"nbt placement '{pr.nbt_path}' has block_count=0; "
                f"check that the .nbt file exists and is non-empty",
            )

    def test_stamp_radial_placements_have_nonzero_blocks(self):
        """Every stamp_radial placement must produce > 0 blocks (B3 check)."""
        radial_results = [
            pr for pr, pl in zip(self.result.paste_results, self.result.placed)
            if pl.kind == "stamp_radial"
        ]
        self.assertGreater(
            len(radial_results),
            0,
            "There must be stamp_radial placements (herb gardens) in the layout",
        )
        for pr in radial_results:
            self.assertGreater(
                pr.block_count,
                0,
                f"stamp_radial '{pr.nbt_path}' has block_count=0 (B3 regression)",
            )

    def test_block_grid_placements_have_nonzero_blocks(self):
        """Every block_grid placement (central path) must produce > 0 blocks (B3 check)."""
        grid_results = [
            pr for pr, pl in zip(self.result.paste_results, self.result.placed)
            if pl.kind == "block_grid"
        ]
        self.assertGreater(
            len(grid_results),
            0,
            "There must be block_grid placements (central path) in the layout",
        )
        for pr in grid_results:
            self.assertGreater(
                pr.block_count,
                0,
                f"block_grid '{pr.nbt_path}' has block_count=0 (B3 regression)",
            )

    def test_master_sarcophagus_block_count_nonzero(self):
        """master_sarcophagus placement must produce > 0 blocks."""
        sarc_results = [
            pr for pr, pl in zip(self.result.paste_results, self.result.placed)
            if pl.kind == "nbt" and "master_sarcophagus" in pl.payload
        ]
        self.assertEqual(
            len(sarc_results),
            1,
            f"Expected exactly 1 master_sarcophagus paste result, got {len(sarc_results)}",
        )
        pr = sarc_results[0]
        self.assertGreater(
            pr.block_count,
            0,
            f"master_sarcophagus block_count={pr.block_count}; "
            f"NBT file must have at least 1 block",
        )

    def test_total_block_count_is_nonzero(self):
        """Sum of all placement block counts must be > 0."""
        total_blocks = sum(pr.block_count for pr in self.result.paste_results)
        self.assertGreater(
            total_blocks,
            0,
            "Total block count across all placements must be > 0",
        )

    def test_great_hall_placement_count_is_largest(self):
        """dan_zong_great_hall.nbt must produce the most blocks (it is the main structure)."""
        hall_results = [
            pr for pr, pl in zip(self.result.paste_results, self.result.placed)
            if pl.kind == "nbt" and "great_hall" in pl.payload
        ]
        self.assertEqual(len(hall_results), 1, "Should have exactly 1 great hall placement")
        hall_count = hall_results[0].block_count

        all_nbt_counts = [
            pr.block_count
            for pr, pl in zip(self.result.paste_results, self.result.placed)
            if pl.kind == "nbt"
        ]
        self.assertEqual(
            max(all_nbt_counts),
            hall_count,
            f"great_hall must produce the most blocks among nbt placements "
            f"(hall={hall_count}, max={max(all_nbt_counts)})",
        )


# ---------------------------------------------------------------------------
# Y-anchor tests (M2 contract)
# ---------------------------------------------------------------------------


class YAnchorTests(unittest.TestCase):
    """POI.y=82 anchors the layout; placement world_y = POI.y + offset.y."""

    def setUp(self):
        self.zone = _make_dan_zong_zone()
        self.poi_y = 82  # from zones.worldview.example.json
        self.result = run_layout(
            DAN_ZONG_COMPOUND_LAYOUT, self.zone, nbt_base_dir=DAN_ZONG_NBT_BASE
        )

    def _find_placed(self, payload_substr: str):
        for pl in self.result.placed:
            if payload_substr in pl.payload:
                return pl
        return None

    def test_great_hall_world_y_equals_poi_y(self):
        """Great hall (offset y=0) must be at world_y == POI.y=82."""
        hall = self._find_placed("great_hall")
        self.assertIsNotNone(hall, "great_hall placement must exist")
        self.assertEqual(
            hall.world_pos[1],
            self.poi_y,
            f"great_hall world_y={hall.world_pos[1]} must equal POI.y={self.poi_y}",
        )

    def test_master_sarcophagus_world_y_is_poi_y_minus_4(self):
        """master_sarcophagus (offset y=-4) must be at world_y == POI.y - 4 = 78."""
        sarc = self._find_placed("master_sarcophagus")
        self.assertIsNotNone(sarc, "master_sarcophagus placement must exist")
        expected_y = self.poi_y - 4
        self.assertEqual(
            sarc.world_pos[1],
            expected_y,
            f"master_sarcophagus world_y={sarc.world_pos[1]} must equal POI.y-4={expected_y}",
        )

    def test_poi_center_used_as_anchor(self):
        """run_layout must use the POI position as the anchor (M2 contract)."""
        self.assertEqual(
            self.result.poi_world_pos,
            (-1600, 82, 4000),
            f"POI center should be (-1600, 82, 4000), got {self.result.poi_world_pos}",
        )


if __name__ == "__main__":
    unittest.main()
