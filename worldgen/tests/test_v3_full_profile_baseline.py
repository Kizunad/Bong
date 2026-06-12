"""worldgen-v4 P2 全 profile 行为等价对拍 — 迁移前的景观锁.

P2 rewrites every hand-written numpy profile into the declarative DSL.  The hard
constraint is **换表示不换景观** (change the representation, not the landscape):
a migrated DSL profile must reproduce the same observable terrain it produced as
numpy — surface_y / water_y / biome_id and the full span shape (cave voids,
sky-isle second spans).  Plan §P2 验收 is容差等价 (NOT byte-exact like P0).

This test freezes that contract for ALL 20 registered profiles in
``worldgen/fixtures/v3_full_profile_baseline.json``.  Three guarantees:

1. Re-running the current (pre-migration) generators + the span shim today still
   yields the exact baseline numbers — catches accidental landscape drift on any
   profile before a single DSL line is written (the equivalence对拍物 itself must
   be stable, or every later migration check is built on sand).
2. The golden actually exercises the structural folds it claims to lock: at least
   one cave void column (2 spans) and the sky-isle floating second span are
   present, so the matrix isn't a degenerate "every column is one flat span"
   self-pin.
3. Coverage: the golden covers every profile in ``_GENERATORS`` — no profile can
   be silently migrated without an equivalence pin.

When a profile is later migrated to the DSL and stops folding through the shim,
its DSL-built ColumnSpans must reproduce these numbers within tolerance.  If a
profile legitimately changes its landscape, regenerate with
``python3 tests/regenerate_v3_full_profile_baseline.py`` and review the diff —
never hand-edit the JSON to silence this test.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import numpy as np

from scripts.terrain_gen.profiles import list_profile_generators
from scripts.terrain_gen.spans_shim import spans_for_tile
from v3_full_profile_baseline import (
    FULL_PROFILE_BASELINE,
    build_full_profile_buffer,
    col_index,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = REPO_ROOT / "worldgen/fixtures/v3_full_profile_baseline.json"


class V3FullProfileBaselineTest(unittest.TestCase):
    def setUp(self) -> None:
        self.baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
        self.assertGreater(len(self.baseline), 0, "baseline fixture is empty")
        self.by_profile: dict[str, list[dict]] = {}
        for row in self.baseline:
            self.by_profile.setdefault(row["profile"], []).append(row)

    def test_baseline_covers_every_registered_profile(self) -> None:
        # Every profile in the global registry (minus the wilderness fallback,
        # which is not a zone profile) must have an equivalence pin — otherwise a
        # DSL migration could land unverified.
        registered = set(list_profile_generators())
        baselined = set(self.by_profile)
        registry_keys = {key for key, _fn, _zone in FULL_PROFILE_BASELINE}
        self.assertEqual(
            registry_keys,
            registered,
            "the full-profile baseline registry must list exactly the registered "
            f"profiles; missing={registered - registry_keys}, "
            f"extra={registry_keys - registered}",
        )
        self.assertEqual(
            baselined,
            registered,
            "every registered terrain profile must have a frozen equivalence "
            f"baseline; missing={registered - baselined}, "
            f"extra={baselined - registered}",
        )

    def test_baseline_exercises_structural_folds(self) -> None:
        # The golden must lock the multi-span structures (cave void, sky-isle
        # second span) — if every column were a single flat span the matrix would
        # be a no-op that no carve regression could trip.
        cave_multi = [
            r
            for r in self.by_profile.get("cave_network", [])
            if len(r["spans"]) > 1
        ]
        self.assertGreater(
            len(cave_multi),
            0,
            "cave_network baseline has no carved (2-span) column — the cave-void "
            "fold is not being pinned; the golden can't catch a cave regression",
        )
        for row in cave_multi:
            # span[0] is the walkable surface cap; span[1] is the floor remnant
            # BELOW the void (lower floor).  Lock that ordering so a migration
            # that swaps surface/floor (drops query_surface off the walkable top)
            #撞红.
            surface_span, floor_span = row["spans"][0], row["spans"][1]
            self.assertGreater(
                surface_span[0],
                floor_span[1] + 1,
                f"cave_network {(row['local_x'], row['local_z'])}: surface span "
                f"{surface_span} must sit above the floor remnant {floor_span} "
                "with an air gap (the carved void)",
            )

        isle_multi = [
            r for r in self.by_profile.get("sky_isle", []) if len(r["spans"]) > 1
        ]
        self.assertGreater(
            len(isle_multi),
            0,
            "sky_isle baseline has no floating (2-span) column — the sky-island "
            "→ second span fold is not pinned",
        )
        for row in isle_multi:
            ground_span, isle_span = row["spans"][0], row["spans"][1]
            self.assertGreater(
                isle_span[0],
                ground_span[1] + 1,
                f"sky_isle {(row['local_x'], row['local_z'])}: isle span "
                f"{isle_span} must float above the ground span {ground_span}",
            )

    def test_baseline_includes_real_water_columns(self) -> None:
        water_rows = [
            r
            for r in self.by_profile.get("spring_marsh", [])
            if r["water_y"] is not None
        ]
        self.assertGreater(
            len(water_rows),
            0,
            "spring_marsh baseline must contain at least one flooded column so a "
            "migration that drops the water surface is caught",
        )

    def test_generators_still_match_frozen_baseline(self) -> None:
        # Run each generator once, fold through the shim, and compare every
        # sampled column to the golden.  This is BOTH a pre-migration drift guard
        # AND, post-migration, the contract the DSL profile must reproduce.
        for profile_key, rows in self.by_profile.items():
            buffer = build_full_profile_buffer(profile_key)
            columns = spans_for_tile(buffer)
            water = np.asarray(buffer.layers["water_level"])
            biome = np.asarray(buffer.layers["biome_id"])
            for row in rows:
                idx = col_index(row["local_x"], row["local_z"])
                column = columns[idx]
                col = (row["local_x"], row["local_z"])

                self.assertEqual(
                    column.surface_ceiling_y,
                    row["surface_y"],
                    f"{profile_key}{col}: carved surface_y drifted from baseline "
                    f"(expected {row['surface_y']}, got "
                    f"{column.surface_ceiling_y}); if the profile changed on "
                    "purpose regenerate the golden",
                )

                got_spans = [[int(f), int(c)] for f, c in column.spans]
                self.assertEqual(
                    got_spans,
                    row["spans"],
                    f"{profile_key}{col}: span shape drifted from baseline "
                    f"(expected {row['spans']}, got {got_spans}); the vertical "
                    "structure (cave void / sky-isle) changed",
                )

                water_level = float(water[idx])
                got_water = round(water_level) if water_level >= 0.0 else None
                self.assertEqual(
                    got_water,
                    row["water_y"],
                    f"{profile_key}{col}: water_y drifted from baseline "
                    f"(expected {row['water_y']}, got {got_water})",
                )

                self.assertEqual(
                    int(biome[idx]),
                    row["biome_id"],
                    f"{profile_key}{col}: biome_id drifted from baseline "
                    f"(expected {row['biome_id']}, got {int(biome[idx])})",
                )


if __name__ == "__main__":
    unittest.main()
