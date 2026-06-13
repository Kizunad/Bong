"""worldgen-v4 P6 — decoration NBT geometry pins (the round-3 blocker fixes).

These lock three runtime-physics / ecology hazards that the earlier rounds
shipped, so a regenerated asset that re-introduces them trips the suite:

  Blocker 1 — `kind="shrub"` ecology routing. A shrub used to resolve a single
    shared `bush/` pool whose four variants spanned four biomes, so the runtime
    `hash % len` pick could stamp a frozen ice thorn or a nether crimson patch
    into the spawn starter meadow. Fixed by splitting into `bush_<ecology>/`
    pools routed per shrub name in `profiles/base.py`. Pinned: every authored
    shrub name resolves a non-empty pool, and the four ecology pools are
    mutually exclusive (no cross-biome leak).

  Blocker 2 — `broken_urn` floating corner posts. The posts sat on (±r, ±r)
    which is OUTSIDE the round dais footprint, leaving them hanging over AIR.
    Pinned: every non-floor block in every urn variant has a block directly
    below it.

  Blocker 3 — `spirit_ore_vein` unsupported `amethyst_cluster`. The upward
    shards were scattered over the top with AIR below; a `facing=up`
    AmethystClusterBlock needs a solid support face below in MC 1.20.1 or it
    pops into a dropped item the instant the chunk loads. Pinned: every
    `facing=up` amethyst_cluster sits directly on a solid outcrop block.

The tests load the SHIPPED `.nbt` assets under
`server/structures/decorations/`, so they pin exactly what the Rust
`DecorationNbtRegistry` stamps at runtime — not just the builder output.
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
DECORATIONS_ROOT = REPO_ROOT / "server" / "structures" / "decorations"

sys.path.insert(0, str(REPO_ROOT / "worldgen"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "nbt"))

from nbt_builder import load_structure  # noqa: E402

from scripts.terrain_gen.profiles import GLOBAL_DECORATION_PALETTE  # noqa: E402
from scripts.terrain_gen.profiles.base import (  # noqa: E402
    _SHRUB_DEFAULT_ECOLOGY,
    _SHRUB_ECOLOGIES,
    _SHRUB_ECOLOGY,
    nbt_placement_for,
    shrub_ecology_for,
)


def _bare(name: str) -> str:
    return name[len("minecraft:"):] if name.startswith("minecraft:") else name


def _block_positions(nbt_path: Path) -> dict[tuple[int, int, int], tuple[str, dict]]:
    """{(x, y, z): (bare_block_name, properties)} for a shipped .nbt asset."""
    out: dict[tuple[int, int, int], tuple[str, dict]] = {}
    for sb in load_structure(str(nbt_path)):
        out[sb.pos] = (_bare(sb.block_name), sb.properties)
    return out


# ---------------------------------------------------------------------------
# Blocker 1 — shrub ecology routing.
# ---------------------------------------------------------------------------
class ShrubEcologyRoutingTests(unittest.TestCase):
    """Each shrub name resolves an ecology-pure pool; the pools never overlap."""

    def _shrub_names(self) -> list[str]:
        names = sorted(
            {
                str(d["name"])
                for d in GLOBAL_DECORATION_PALETTE
                if d["kind"] == "shrub"
            }
        )
        self.assertTrue(
            names,
            "expected at least one kind='shrub' decoration in the global palette; "
            "the ecology routing test is vacuous otherwise",
        )
        return names

    def test_each_ecology_pool_exists_with_three_distinct_variants(self) -> None:
        # Every ecology label must back a real bush_<ecology>/ dir with >=3
        # authored variants (the §6.1 variant contract, per ecology).
        for eco in _SHRUB_ECOLOGIES:
            dir_path = DECORATIONS_ROOT / f"bush_{eco}"
            self.assertTrue(
                dir_path.is_dir(),
                f"ecology '{eco}' has no decorations/bush_{eco}/ asset dir",
            )
            variants = sorted(dir_path.glob("*.nbt"))
            self.assertGreaterEqual(
                len(variants),
                3,
                f"bush_{eco}/ ships only {len(variants)} variant(s); the §6.1 "
                f"contract requires >=3 per ecology pool",
            )

    def test_default_ecology_is_a_known_ecology(self) -> None:
        self.assertIn(
            _SHRUB_DEFAULT_ECOLOGY,
            _SHRUB_ECOLOGIES,
            f"the shrub fallback ecology {_SHRUB_DEFAULT_ECOLOGY!r} must be one of "
            f"{_SHRUB_ECOLOGIES} so its bush_<ecology>/ dir exists",
        )

    def test_every_shrub_resolves_a_non_empty_pool(self) -> None:
        # Each authored shrub must resolve a non-empty template pool whose ids
        # all live under the shrub's own ecology dir.
        for name in self._shrub_names():
            templates, anchor = nbt_placement_for(name, "shrub")
            eco = shrub_ecology_for(name)
            self.assertEqual(
                anchor,
                "ground",
                f"shrub {name!r} must use the ground anchor (got {anchor!r})",
            )
            self.assertTrue(
                templates,
                f"shrub {name!r} (ecology {eco}) resolved an EMPTY template pool — "
                f"the runtime would silently fall back to procedural geometry",
            )
            expected_prefix = f"decorations/bush_{eco}/"
            for tid in templates:
                self.assertTrue(
                    tid.startswith(expected_prefix),
                    f"shrub {name!r} (ecology {eco}) resolved {tid!r} which is NOT "
                    f"under its own pool {expected_prefix} — cross-biome leak",
                )

    def test_ecology_pools_are_mutually_exclusive(self) -> None:
        # The four ecology pools must be pairwise disjoint, so a temperate shrub
        # can NEVER resolve a cold/nether/marsh variant under any hash. Resolve
        # each pool from a name pinned to that ecology (or the dir glob for the
        # default ecology, which has no guaranteed mapped name).
        pools: dict[str, set[str]] = {}
        for eco in _SHRUB_ECOLOGIES:
            # A name guaranteed to map to `eco`: take one from _SHRUB_ECOLOGY,
            # else fall back to the dir glob (true for the default ecology).
            name = next((n for n, e in _SHRUB_ECOLOGY.items() if e == eco), None)
            if name is not None:
                pools[eco] = set(nbt_placement_for(name, "shrub")[0])
            else:
                pools[eco] = {
                    f"decorations/bush_{eco}/{p.name}"
                    for p in (DECORATIONS_ROOT / f"bush_{eco}").glob("*.nbt")
                }
            self.assertTrue(
                pools[eco],
                f"ecology pool {eco} is empty; cannot verify exclusivity",
            )
        ecos = list(_SHRUB_ECOLOGIES)
        for i in range(len(ecos)):
            for j in range(i + 1, len(ecos)):
                a, b = ecos[i], ecos[j]
                overlap = pools[a] & pools[b]
                self.assertEqual(
                    overlap,
                    set(),
                    f"ecology pools {a} and {b} share variants {sorted(overlap)}; "
                    f"a {a} shrub could stamp a {b} variant — cross-biome leak",
                )

    def test_starter_shrub_never_resolves_nether_or_cold(self) -> None:
        # The headline regression: the spawn starter meadow must not stamp a
        # nether crimson patch or a frozen ice thorn.
        templates, _ = nbt_placement_for("starter_shrub", "shrub")
        joined = " ".join(templates)
        self.assertNotIn(
            "bush_nether", joined,
            "starter_shrub (spawn meadow) must never resolve a nether variant",
        )
        self.assertNotIn(
            "bush_cold", joined,
            "starter_shrub (spawn meadow) must never resolve a frozen variant",
        )
        self.assertTrue(
            all(t.startswith("decorations/bush_temperate/") for t in templates),
            f"starter_shrub must resolve only temperate variants, got {templates}",
        )

    def test_unmapped_shrub_falls_back_to_default_temperate_pool(self) -> None:
        # A profile that authors a new shrub name without registering its ecology
        # must get the safe overworld default, never a jarring biome.
        templates, anchor = nbt_placement_for("a_brand_new_unmapped_shrub", "shrub")
        self.assertEqual(shrub_ecology_for("a_brand_new_unmapped_shrub"),
                         _SHRUB_DEFAULT_ECOLOGY)
        self.assertTrue(
            all(
                t.startswith(f"decorations/bush_{_SHRUB_DEFAULT_ECOLOGY}/")
                for t in templates
            ),
            f"unmapped shrub must resolve the default pool "
            f"bush_{_SHRUB_DEFAULT_ECOLOGY}/, got {templates}",
        )


# ---------------------------------------------------------------------------
# Blockers 2 & 3 — no floating / unsupported decoration blocks.
# ---------------------------------------------------------------------------
# Blocks that do NOT provide a sturdy top support face for a `facing=up`
# amethyst cluster (clusters can't stack on each other, air is nothing).
_NON_SUPPORT_BLOCKS = {"amethyst_cluster"}


class StructureSupportTests(unittest.TestCase):
    """Shipped structure assets must not float / leave clusters unsupported."""

    def _variants(self, kind: str) -> list[Path]:
        variants = sorted((DECORATIONS_ROOT / kind).glob("*.nbt"))
        self.assertTrue(
            variants,
            f"no shipped variants under decorations/{kind}/ — run "
            f"scripts/nbt/decorations/gen_structures.py",
        )
        return variants

    def test_broken_urn_has_no_floating_blocks(self) -> None:
        # Every block above y=0 must have a block directly beneath it; the
        # earlier off-disc corner posts hung over AIR (Blocker 2).
        for path in self._variants("broken_urn"):
            blocks = _block_positions(path)
            floaters = [
                (x, y, z, name)
                for (x, y, z), (name, _) in blocks.items()
                if y > 0 and (x, y - 1, z) not in blocks
            ]
            self.assertEqual(
                floaters,
                [],
                f"{path.name}: these blocks float over AIR (no block below): "
                f"{floaters}. Urn posts must stand on the dais footprint, not the "
                f"off-disc corners.",
            )

    def test_spirit_ore_vein_amethyst_clusters_are_supported(self) -> None:
        # Every upward amethyst cluster must sit on a solid outcrop block; a
        # `facing=up` cluster with AIR/cluster below pops off on chunk load
        # (Blocker 3).
        for path in self._variants("spirit_ore_vein"):
            blocks = _block_positions(path)
            unsupported = []
            saw_cluster = False
            for (x, y, z), (name, props) in blocks.items():
                if name != "amethyst_cluster":
                    continue
                saw_cluster = True
                self.assertEqual(
                    props.get("facing"),
                    "up",
                    f"{path.name}: amethyst_cluster at {(x, y, z)} must face up "
                    f"(got {props.get('facing')!r})",
                )
                below = blocks.get((x, y - 1, z))
                if below is None or below[0] in _NON_SUPPORT_BLOCKS:
                    unsupported.append(
                        (x, y, z, "below=" + (below[0] if below else "AIR"))
                    )
            self.assertTrue(
                saw_cluster,
                f"{path.name}: expected at least one amethyst_cluster shard",
            )
            self.assertEqual(
                unsupported,
                [],
                f"{path.name}: these facing=up amethyst clusters lack a solid "
                f"support block below and would pop on chunk load: {unsupported}",
            )

    def test_bone_pile_skulls_rest_on_the_mound(self) -> None:
        # Bone piles perch skulls on top of the mound; each skull must sit on a
        # bone-pile block, never hang over AIR. (Same support hazard class as the
        # urn posts, on the bone-pile kind.)
        for path in self._variants("bone_pile"):
            blocks = _block_positions(path)
            floaters = [
                (x, y, z, name)
                for (x, y, z), (name, _) in blocks.items()
                if name == "skeleton_skull" and y > 0 and (x, y - 1, z) not in blocks
            ]
            self.assertEqual(
                floaters,
                [],
                f"{path.name}: skulls floating over AIR (no mound block below): "
                f"{floaters}",
            )


if __name__ == "__main__":
    unittest.main()
