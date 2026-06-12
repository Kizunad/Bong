"""Regenerate worldgen/fixtures/v3_full_profile_baseline.json (worldgen-v4 P2).

Run from the worldgen/ directory:

    python3 -m tests.regenerate_v3_full_profile_baseline

This captures the full-profile equivalence golden for the P2 DSL migration: for
EVERY registered terrain profile, it folds the profile's 2D output through the
SAME ``spans_fold.spans_for_tile`` the raster export uses and records, per
sampled column, the observable contract a migrated DSL profile must reproduce:

  surface_y   = span[0].ceiling_y     (the carved walkable surface)
  water_y     = round(water_level) or None
  biome_id    = uint8 biome index
  spans       = [[floor, ceiling], ...]  (the full span shape — cave voids /
                sky-isle second spans locked, not just the surface)

It must NOT be re-run casually.  Regenerate only when a profile legitimately
changes its landscape (and never to silence a migration regression — that is the
whole point of the pin); review the JSON diff by hand afterwards.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

# Allow a direct `python3 tests/regenerate_v3_full_profile_baseline.py` run.
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.terrain_gen.spans_fold import spans_for_tile  # noqa: E402
from v3_full_profile_baseline import (  # noqa: E402
    FULL_PROFILE_BASELINE,
    build_full_profile_buffer,
    col_index,
    sample_columns_for,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = REPO_ROOT / "worldgen/fixtures/v3_full_profile_baseline.json"


def capture_baseline() -> list[dict]:
    rows: list[dict] = []
    for profile_key, _fill_fn, _zone in FULL_PROFILE_BASELINE:
        buffer = build_full_profile_buffer(profile_key)
        columns = spans_for_tile(buffer)
        water = np.asarray(buffer.layers["water_level"])
        biome = np.asarray(buffer.layers["biome_id"])
        for local_x, local_z in sample_columns_for(profile_key):
            idx = col_index(local_x, local_z)
            column = columns[idx]
            water_level = float(water[idx])
            surface_y = column.surface_ceiling_y
            rows.append(
                {
                    "profile": profile_key,
                    "local_x": local_x,
                    "local_z": local_z,
                    "surface_y": surface_y,
                    "water_y": (round(water_level) if water_level >= 0.0 else None),
                    "biome_id": int(biome[idx]),
                    "spans": [[int(f), int(c)] for f, c in column.spans],
                }
            )
    return rows


def main() -> None:
    rows = capture_baseline()
    BASELINE_PATH.parent.mkdir(parents=True, exist_ok=True)
    BASELINE_PATH.write_text(
        json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    profiles = sorted({row["profile"] for row in rows})
    print(
        f"wrote {len(rows)} baseline columns across {len(profiles)} profiles "
        f"-> {BASELINE_PATH}"
    )


if __name__ == "__main__":
    main()
