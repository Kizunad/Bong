"""worldgen-v4 P0 行为等价对拍 — v3 baseline pin.

This test locks the frozen v3 surface contract captured in
``worldgen/fixtures/v3_surface_baseline.json``.  Two guarantees:

1. Re-running the v3 generators today still yields the exact baseline numbers
   (catches accidental landscape drift in any of the five representative
   profiles before the span refactor changes anything).
2. (after P0 shim lands) the spans folded from those same 2D outputs reproduce
   the baseline: ``surface_y == lowest-solid-span ceiling`` and
   ``water_y`` unchanged — see ``test_spans_shim.py``.

If a profile legitimately changes its landscape, regenerate the golden with
``python3 tests/regenerate_v3_baseline.py`` and review the diff — never edit the
JSON by hand to silence this test.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

import numpy as np

from v3_baseline_zones import TILE_SIZE, build_baseline_buffer

REPO_ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = REPO_ROOT / "worldgen/fixtures/v3_surface_baseline.json"


def _col_index(local_x: int, local_z: int) -> int:
    return local_z * TILE_SIZE + local_x


class V3SurfaceBaselineTest(unittest.TestCase):
    def setUp(self) -> None:
        self.baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
        self.assertGreater(len(self.baseline), 0, "baseline fixture is empty")

    def test_baseline_covers_all_representative_shapes(self) -> None:
        profiles = {row["profile"] for row in self.baseline}
        self.assertEqual(
            profiles,
            {"normal", "water", "sky_isle", "cave", "abyssal"},
            "baseline must cover all five representative column shapes; "
            f"got {sorted(profiles)}",
        )

    def test_baseline_includes_real_water_columns(self) -> None:
        water_rows = [
            row
            for row in self.baseline
            if row["profile"] == "water" and row["water_y"] is not None
        ]
        self.assertGreater(
            len(water_rows),
            0,
            "water profile baseline must contain at least one flooded column so "
            "the shim's water_y equivalence is actually exercised",
        )

    def test_v3_generators_still_match_frozen_baseline(self) -> None:
        # Group rows by profile so each generator only runs once.
        rows_by_profile: dict[str, list[dict]] = {}
        for row in self.baseline:
            rows_by_profile.setdefault(row["profile"], []).append(row)

        for profile_key, rows in rows_by_profile.items():
            buffer = build_baseline_buffer(profile_key)
            height = np.asarray(buffer.layers["height"])
            water = np.asarray(buffer.layers["water_level"])
            biome = np.asarray(buffer.layers["biome_id"])
            for row in rows:
                idx = _col_index(row["local_x"], row["local_z"])
                got_surface = int(round(float(height[idx])))
                water_level = float(water[idx])
                got_water = (
                    int(round(water_level)) if water_level >= 0.0 else None
                )
                got_biome = int(biome[idx])
                col = (row["local_x"], row["local_z"])
                self.assertEqual(
                    got_surface,
                    row["surface_y"],
                    f"{profile_key}{col}: surface drifted from v3 baseline "
                    f"(expected {row['surface_y']}, got {got_surface}); if the "
                    f"profile changed on purpose regenerate the golden",
                )
                self.assertEqual(
                    got_water,
                    row["water_y"],
                    f"{profile_key}{col}: water_y drifted from v3 baseline "
                    f"(expected {row['water_y']}, got {got_water})",
                )
                self.assertEqual(
                    got_biome,
                    row["biome_id"],
                    f"{profile_key}{col}: biome_id drifted from v3 baseline "
                    f"(expected {row['biome_id']}, got {got_biome})",
                )


if __name__ == "__main__":
    unittest.main()
