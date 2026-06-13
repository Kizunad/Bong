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


# The dual-pin fixture below is the dan_zong / wangyintai authored-structure
# contract (see module docstring). The P6 `decorations/` asset family is a
# separate, much larger palette that has its own blocks.rs-resolution contract
# (see DecorationNbtPaletteTest below and worldgen/tests/test_decoration_contract
# .py), so the dual-pin walk is scoped to its own subdirs — folding decorations
# into AUTHORED_STRUCTURE_BLOCKS would make the dual-pin meaningless.
DUAL_PIN_SUBDIRS = ("dan_zong", "wangyintai")


def _load_all_nbt_blocks(subdirs: tuple[str, ...] = DUAL_PIN_SUBDIRS) -> dict[str, set[str]]:
    """Return {nbt_path: {bare_block_name, ...}} for every .nbt under the given subdirs."""
    from nbt_builder import load_structure

    result: dict[str, set[str]] = {}
    roots = [SERVER_STRUCTURES_DIR / sub for sub in subdirs]
    for base in roots:
        if not base.exists():
            continue
        for root, _, files in os.walk(base):
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


class DecorationNbtPaletteTest(unittest.TestCase):
    """worldgen-v4 P6 — every `decorations/**/*.nbt` block resolves in blocks.rs.

    The decoration asset family is far larger than the dan_zong / wangyintai
    dual-pin fixture, so it gets its own contract: each block the runtime
    `DecorationNbtRegistry` will stamp must be a name `block_from_name` knows, or
    the stamp would silently drop it (a hole in the decoration).  This is the
    cross-boundary guard for the ecology-split bush pools and the structure
    fixes — a regenerated asset using an unknown block trips here.
    """

    BLOCKS_RS = REPO_ROOT / "server" / "src" / "world" / "terrain" / "blocks.rs"
    DECORATIONS_DIR = SERVER_STRUCTURES_DIR / "decorations"

    @classmethod
    def setUpClass(cls) -> None:
        cls.deco_blocks_by_file = _load_all_nbt_blocks(("decorations",))

    def _resolved_block_names(self) -> set[str]:
        import re

        source = self.BLOCKS_RS.read_text(encoding="utf-8")
        return set(re.findall(r'"([a-z0-9_]+)"\s*=>\s*BlockState::', source))

    def test_decoration_assets_present(self) -> None:
        self.assertTrue(
            self.deco_blocks_by_file,
            "no decorations/**/*.nbt assets found; run "
            "scripts/nbt/decorations/gen_decorations.py",
        )

    def test_every_decoration_nbt_block_resolves_in_blocks_rs(self) -> None:
        resolved = self._resolved_block_names()
        missing: dict[str, set[str]] = {}
        for path, names in self.deco_blocks_by_file.items():
            unknown = names - resolved
            if unknown:
                missing[Path(path).name] = unknown
        self.assertFalse(
            missing,
            "These decoration NBT files use blocks NOT resolvable by "
            "block_from_name in server/src/world/terrain/blocks.rs (the runtime "
            "registry would stamp a hole):\n"
            + "\n".join(f"  {f}: {sorted(s)}" for f, s in sorted(missing.items())),
        )

    def test_bush_ecology_subdirs_present(self) -> None:
        # The ecology split must ship all four bush_<ecology>/ pools so a shrub
        # never resolves a cross-biome variant (worldgen-v4 P6 ecology fix).
        for eco in ("bush_temperate", "bush_cold", "bush_marsh", "bush_nether"):
            nbts = list((self.DECORATIONS_DIR / eco).glob("*.nbt"))
            self.assertGreaterEqual(
                len(nbts),
                3,
                f"decorations/{eco}/ must ship >=3 variants (found {len(nbts)})",
            )

    def test_old_unsplit_bush_dir_is_gone(self) -> None:
        # The pre-split shared bush/ pool must not coexist with the ecology
        # subdirs, or the runtime would still load its cross-biome variants.
        self.assertFalse(
            (self.DECORATIONS_DIR / "bush").exists(),
            "the old shared decorations/bush/ pool must be removed after the "
            "ecology split; its cross-biome variants would still be stamped",
        )


if __name__ == "__main__":
    unittest.main()
