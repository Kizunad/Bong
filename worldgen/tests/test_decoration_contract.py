from __future__ import annotations

import re
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "worldgen"))

from scripts.terrain_gen.profiles import GLOBAL_DECORATION_PALETTE  # noqa: E402
from scripts.terrain_gen.profiles.base import (  # noqa: E402
    DECORATION_ANCHORS,
    DecorationSpec,
    decoration_payload,
)

BLOCKS_RS = REPO_ROOT / "server" / "src" / "world" / "terrain" / "blocks.rs"

# worldgen-v4 P6 §8.1 — the anchor literal must stay in lockstep with the Rust
# `DecorationAnchor` variants in nbt_registry.rs (from_manifest parses these
# exact lowercase strings; an out-of-sync rename would silently mis-place stamps).
RUST_NBT_REGISTRY_RS = (
    REPO_ROOT / "server" / "src" / "world" / "terrain" / "nbt_registry.rs"
)

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


class DecorationNbtSchemaTests(unittest.TestCase):
    """worldgen-v4 P6 §8.1 — DecorationSpec NBT-pipeline field contract."""

    def test_every_palette_entry_carries_nbt_fields(self) -> None:
        # The manifest the Rust ManifestDecoration deserializes MUST always
        # include both new keys for every entry (with safe defaults), so the
        # serde #[serde(default)] path and the Python exporter never disagree.
        for deco in GLOBAL_DECORATION_PALETTE:
            self.assertIn(
                "nbt_templates",
                deco,
                f"decoration {deco['profile']}/{deco['name']} is missing "
                f"'nbt_templates' in the global palette manifest dict",
            )
            self.assertIsInstance(
                deco["nbt_templates"],
                list,
                f"{deco['name']} nbt_templates must serialize as a JSON list",
            )
            self.assertIn(
                "anchor",
                deco,
                f"decoration {deco['profile']}/{deco['name']} is missing "
                f"'anchor' in the global palette manifest dict",
            )
            self.assertIn(
                deco["anchor"],
                DECORATION_ANCHORS,
                f"{deco['name']} anchor {deco['anchor']!r} is not a known anchor "
                f"{DECORATION_ANCHORS}",
            )

    def test_existing_decorations_default_to_procedural_ground(self) -> None:
        # No existing spec has opted into the NBT path yet (Stage 2 authors the
        # assets). So every current palette entry must default to empty
        # templates + ground anchor — i.e. stay on the procedural placement path.
        for deco in GLOBAL_DECORATION_PALETTE:
            self.assertEqual(
                deco["nbt_templates"],
                [],
                f"{deco['name']} unexpectedly declares NBT templates before "
                f"Stage 2 authored any assets: {deco['nbt_templates']}",
            )
            self.assertEqual(
                deco["anchor"],
                "ground",
                f"{deco['name']} anchor should default to 'ground' until an "
                f"NBT-driven variant opts into a different anchor",
            )

    def test_decoration_payload_round_trips_nbt_fields(self) -> None:
        spec = DecorationSpec(
            name="ling_yu_tree",
            kind="tree",
            blocks=("oak_log", "oak_leaves"),
            nbt_templates=(
                "decorations/tree/ling_yu_tree_v1.nbt",
                "decorations/tree/ling_yu_tree_v2.nbt",
            ),
            anchor="hanging",
        )
        payload = decoration_payload(spec)
        self.assertEqual(
            payload["nbt_templates"],
            [
                "decorations/tree/ling_yu_tree_v1.nbt",
                "decorations/tree/ling_yu_tree_v2.nbt",
            ],
            "decoration_payload must emit the template list in authored order",
        )
        self.assertEqual(
            payload["anchor"],
            "hanging",
            "decoration_payload must emit the anchor string verbatim",
        )

    def test_anchor_default_is_ground_and_empty_templates(self) -> None:
        spec = DecorationSpec(name="x", kind="boulder", blocks=("stone",))
        self.assertEqual(spec.anchor, "ground")
        self.assertEqual(spec.nbt_templates, ())

    def test_invalid_anchor_is_rejected_at_construction(self) -> None:
        # A bad anchor must trip at spec construction (loud) rather than ship a
        # manifest the Rust enum would silently coerce to Ground.
        for bad in ["floating", "sky_isle_top", "GROUND", ""]:
            with self.assertRaises(
                ValueError,
                msg=f"anchor {bad!r} should be rejected but was accepted",
            ):
                DecorationSpec(name="bad", kind="tree", blocks=("a",), anchor=bad)

    def test_all_three_anchors_construct(self) -> None:
        for anchor in DECORATION_ANCHORS:
            spec = DecorationSpec(
                name=f"deco_{anchor}", kind="crystal", blocks=("amethyst_block",), anchor=anchor
            )
            self.assertEqual(spec.anchor, anchor)

    def test_python_anchors_match_rust_from_manifest_arms(self) -> None:
        # Cross-language pin: each DECORATION_ANCHORS string must be handled by a
        # `"<anchor>" =>` arm in the Rust DecorationAnchor::from_manifest match,
        # so the two enums can never silently drift apart.
        source = RUST_NBT_REGISTRY_RS.read_text(encoding="utf-8")
        # "ground" is the catch-all default arm (`_ =>`), not a literal arm.
        for anchor in DECORATION_ANCHORS:
            if anchor == "ground":
                continue
            self.assertIn(
                f'"{anchor}" =>',
                source,
                f"Rust DecorationAnchor::from_manifest has no arm for "
                f"Python anchor {anchor!r}; the enums have drifted",
            )


if __name__ == "__main__":
    unittest.main()
