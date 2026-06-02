"""Dual-pin tests: NBT structure block palette ⊆ server block_from_name known list.

FIX A (plan-terrain-wiring-v1 opus verify) — cross-boundary contract:

  1. Every block name in dan_zong / wangyintai NBT files must be present in
     AUTHORED_STRUCTURE_BLOCKS (the fixture list kept in blocks.rs tests).
  2. Every block name in AUTHORED_STRUCTURE_BLOCKS must appear in the NBT palette
     OR have an explicit justification (e.g. iron_nugget → AIR alias).

If the NBT palette gains a new block that is not added to blocks.rs, the Python
side test fires first (CI catches it before the Rust side silently drops blocks).
"""

from __future__ import annotations

import os
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_NBT_DIR = REPO_ROOT / "scripts" / "nbt"
SERVER_STRUCTURES_DIR = REPO_ROOT / "server" / "structures"

sys.path.insert(0, str(SCRIPTS_NBT_DIR))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

# ---------------------------------------------------------------------------
# The canonical "authored block" list — must match
# server/src/world/terrain/blocks.rs tests::AUTHORED_STRUCTURE_BLOCKS exactly.
# ---------------------------------------------------------------------------
AUTHORED_STRUCTURE_BLOCKS: frozenset[str] = frozenset(
    {
        "amethyst_block",
        "amethyst_cluster",
        "andesite",
        "birch_pressure_plate",
        "blackstone",
        "bone_block",
        "bookshelf",
        "calcite",
        "campfire",
        "candle",
        "cauldron",
        "chain",
        "chiseled_deepslate",
        "chiseled_polished_blackstone",
        "chiseled_stone_bricks",
        "coal_block",
        "coal_ore",
        "coarse_dirt",
        "cobblestone",
        "cobblestone_wall",
        "cobweb",
        "cracked_deepslate_bricks",
        "cracked_polished_blackstone_bricks",
        "cracked_stone_bricks",
        "dark_oak_planks",
        "dark_oak_slab",
        "dark_oak_stairs",
        "dead_bush",
        "deepslate_brick_slab",
        "deepslate_bricks",
        "flower_pot",
        "gravel",
        "iron_nugget",
        "lectern",
        "mossy_cobblestone",
        "mossy_stone_bricks",
        "oak_fence",
        "oak_log",
        "podzol",
        "polished_blackstone",
        "polished_blackstone_bricks",
        "polished_blackstone_slab",
        "polished_blackstone_stairs",
        "polished_blackstone_wall",
        "polished_deepslate",
        "polished_deepslate_slab",
        "purple_glazed_terracotta",
        "purple_stained_glass",
        "purple_stained_glass_pane",
        "purple_terracotta",
        "red_mushroom",
        "skeleton_skull",
        "smooth_basalt",
        "soul_campfire",
        "soul_lantern",
        "soul_sand",
        "soul_soil",
        "stone_brick_slab",
        "stone_brick_stairs",
        "stone_bricks",
        "vine",
        "water",
        "white_banner",
    }
)


def _load_all_nbt_blocks() -> dict[str, set[str]]:
    """Return {nbt_path: {bare_block_name, ...}} for every .nbt under server/structures/."""
    from nbt_builder import load_structure

    result: dict[str, set[str]] = {}
    for root, _, files in os.walk(SERVER_STRUCTURES_DIR):
        for fname in files:
            if not fname.endswith(".nbt"):
                continue
            path = str(Path(root) / fname)
            try:
                blocks = load_structure(path)
            except Exception as exc:
                raise RuntimeError(f"Failed to parse {path}: {exc}") from exc
            names: set[str] = set()
            for sb in blocks:
                name = sb.block_name
                if name.startswith("minecraft:"):
                    name = name[len("minecraft:"):]
                names.add(name)
            result[path] = names
    return result


class NbtPaletteSubsetTest(unittest.TestCase):
    """NBT palette ⊆ AUTHORED_STRUCTURE_BLOCKS (Python-side half of the dual-pin)."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.nbt_blocks_by_file = _load_all_nbt_blocks()

    def test_every_nbt_block_is_in_authored_list(self) -> None:
        """Each block name in every NBT file must be in AUTHORED_STRUCTURE_BLOCKS.

        If this fails: add the new block name to AUTHORED_STRUCTURE_BLOCKS here
        AND to block_from_name in server/src/world/terrain/blocks.rs.
        """
        missing: dict[str, set[str]] = {}
        for path, names in self.nbt_blocks_by_file.items():
            not_authored = names - AUTHORED_STRUCTURE_BLOCKS
            if not_authored:
                missing[Path(path).name] = not_authored

        self.assertFalse(
            missing,
            f"These NBT files contain block names NOT in AUTHORED_STRUCTURE_BLOCKS:\n"
            + "\n".join(f"  {f}: {sorted(s)}" for f, s in sorted(missing.items()))
            + "\n\nAdd each missing name to:\n"
            "  1. AUTHORED_STRUCTURE_BLOCKS in worldgen/tests/test_nbt_block_palette.py\n"
            "  2. block_from_name in server/src/world/terrain/blocks.rs",
        )

    def test_all_nbt_files_are_non_empty(self) -> None:
        """Every .nbt file must contain at least one block (guards against empty artifacts)."""
        empty: list[str] = [
            Path(p).name
            for p, names in self.nbt_blocks_by_file.items()
            if not names
        ]
        self.assertFalse(
            empty,
            f"These NBT files appear to be empty (0 blocks): {empty}",
        )

    def test_dan_zong_and_wangyintai_nbt_files_present(self) -> None:
        """Expected NBT structure subdirs must have files (guards against accidental rm)."""
        for subdir in ("dan_zong", "wangyintai"):
            nbt_dir = SERVER_STRUCTURES_DIR / subdir
            nbt_files = list(nbt_dir.glob("*.nbt")) if nbt_dir.exists() else []
            self.assertGreater(
                len(nbt_files),
                0,
                f"Expected at least one .nbt file in server/structures/{subdir}/",
            )

    def test_union_nbt_palette_equals_authored_list(self) -> None:
        """Union of all NBT block names must equal AUTHORED_STRUCTURE_BLOCKS exactly.

        If AUTHORED_STRUCTURE_BLOCKS contains names not in any NBT, they are stale
        entries that should be removed (unless kept as forward-compatibility stubs).
        If the NBT palette contains names not in AUTHORED_STRUCTURE_BLOCKS, the
        test above would have caught them.  Both directions are pinned here.
        """
        all_nbt_names: set[str] = set()
        for names in self.nbt_blocks_by_file.values():
            all_nbt_names.update(names)

        extra_in_authored = AUTHORED_STRUCTURE_BLOCKS - all_nbt_names
        # iron_nugget is kept for AIR-alias disambiguation even though it is not
        # a real MC block; allow it as an explicit exception.
        _allowed_extras = frozenset({"iron_nugget"})
        unexpected_extras = extra_in_authored - _allowed_extras

        self.assertFalse(
            unexpected_extras,
            f"AUTHORED_STRUCTURE_BLOCKS contains names not found in any NBT file: "
            f"{sorted(unexpected_extras)}. Remove stale entries.",
        )


if __name__ == "__main__":
    unittest.main()
