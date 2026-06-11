from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "worldgen"))

from scripts.terrain_gen.profiles import GLOBAL_DECORATION_PALETTE  # noqa: E402

BLOCKS_RS = REPO_ROOT / "server" / "src" / "world" / "terrain" / "blocks.rs"

HIGH_RISK_DECORATION_NAMES = {
    "bone_mountain",
    "thatched_hermitage",
    "daily_artifact_cache",
    "broken_pillar",
    "ruined_bell_tower",
    "moss_lain_statue",
    "forgotten_stele_garden",
    "scripture_pile",
    "sect_stele",
    "whalefall_rib_tree",
    "glass_fulgurite",
}

NON_PLACEABLE_ITEM_NAMES = {
    "armor_stand",
    "bell",
    "glass_bottle",
    "iron_ingot",
    "shulker_box",
    "cobwebs",
}

ALLOWED_KINDS = {
    "tree",
    "shrub",
    "boulder",
    "crystal",
    "mushroom",
    "flower",
    "coral",
    "fallen_log",
    "grave_mound",
}

MANUAL_LARGE_DECORATION_ALLOWLIST = {
    "bone_pile",
    "cantan_block_drift",
    "charred_obelisk_shard",
    "duo_yan_boulder",
    "dust_thorn",
    "fire_vein_cactus",
    "forbidden_pillar",
    "glow_lichen_column",
    "gu_teng_creeper",
    "ice_thorn",
    "lush_grass_overlay",
    "nether_nylium_patch",
    "reed_thicket",
    "red_vine_curtain",
    "ridge_monolith",
    "scorched_earth_ring",
    "silent_rubble",
    "sinkhole_boulder",
    "small_rubble",
    "tea_herb_patch",
    "toppled_pillar",
    "wild_herb_clump",
    "ash_dead_trunk",
    "ashen_mega_tree",
    "bone_spire",
    "broken_spear_tree",
    "charred_husk_tree",
    "crystal_fang",
    "dark_bamboo_grove",
    "ghost_light_reed",
    "impact_crater_rim",
    "lightning_basalt_pit",
    "meteor_core_wreckage",
    "moon_silver_tree",
    "null_pressure_rock",
    "phantom_qi_pillar",
    "qi_crystal_pillar",
    "scarlet_crystal_spike",
    "sky_birch_tree",
    "sky_rose_tree",
    "spirit_pine",
    "twisted_pine",
    "war_banner_post",
    "white_crystal_tree",
    "xun_guang_mushroom",
    "xuan_jing_pillar",
}


def _resolved_block_names() -> set[str]:
    source = BLOCKS_RS.read_text(encoding="utf-8")
    return set(re.findall(r'"([a-z0-9_]+)"\s*=>\s*BlockState::', source))


class DecorationContractTests(unittest.TestCase):
    def test_all_decoration_blocks_resolve_in_server_palette(self) -> None:
        resolved = _resolved_block_names()
        missing = sorted(
            (deco["profile"], deco["name"], block)
            for deco in GLOBAL_DECORATION_PALETTE
            for block in deco["blocks"]
            if block not in resolved
        )

        self.assertEqual(missing, [])

    def test_high_risk_semantic_lazy_decorations_are_absent(self) -> None:
        names = {str(deco["name"]) for deco in GLOBAL_DECORATION_PALETTE}
        self.assertEqual(names & HIGH_RISK_DECORATION_NAMES, set())

    def test_non_placeable_items_are_not_authored_as_decor_blocks(self) -> None:
        offenders = sorted(
            (deco["profile"], deco["name"], block)
            for deco in GLOBAL_DECORATION_PALETTE
            for block in deco["blocks"]
            if block in NON_PLACEABLE_ITEM_NAMES
        )

        self.assertEqual(offenders, [])

    def test_decoration_kind_block_palettes_are_constrained(self) -> None:
        offenders = sorted(
            (deco["profile"], deco["name"], deco["kind"])
            for deco in GLOBAL_DECORATION_PALETTE
            if deco["kind"] not in ALLOWED_KINDS
        )

        self.assertEqual(offenders, [])

    def test_large_or_frequent_non_plant_decorations_are_review_gated(self) -> None:
        offenders = sorted(
            (deco["profile"], deco["name"], deco["kind"], deco["size_range"], deco["rarity"])
            for deco in GLOBAL_DECORATION_PALETTE
            if deco["kind"] not in {"tree", "flower"}
            and (int(deco["size_range"][1]) >= 8 or float(deco["rarity"]) >= 0.45)
            and deco["name"] not in MANUAL_LARGE_DECORATION_ALLOWLIST
        )

        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
