"""Regenerate worldgen/fixtures/v3_surface_baseline.json from the v3 pipeline.

Run from the worldgen/ directory:

    python3 -m tests.regenerate_v3_baseline

The output is the frozen behaviour-equivalence golden for worldgen-v4 P0.
It is captured ONCE from the untouched v3 generators (height + 2D patch
layers) and must NOT be regenerated after the span shim lands — doing so would
defeat the purpose of pinning v3 landscape behaviour.

Each entry records, per sampled column, the observable 2D contract:
  surface_y = round(height)              (top solid block y)
  water_y   = round(water_level) or null (-1 sentinel → no water)
  biome_id  = uint8 biome index
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

# Allow a direct `python3 tests/regenerate_v3_baseline.py` run: put both the
# tests/ dir (for v3_baseline_zones) and the worldgen/ root (for scripts.*) on
# the import path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from v3_baseline_zones import (  # noqa: E402
    BASELINE_PROFILES,
    SAMPLE_COLUMNS,
    TILE_SIZE,
    build_baseline_buffer,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = REPO_ROOT / "worldgen/fixtures/v3_surface_baseline.json"


def _col_index(local_x: int, local_z: int) -> int:
    return local_z * TILE_SIZE + local_x


def capture_baseline() -> list[dict]:
    rows: list[dict] = []
    for profile_key, _fill_fn, _zone in BASELINE_PROFILES:
        buffer = build_baseline_buffer(profile_key)
        height = np.asarray(buffer.layers["height"])
        water = np.asarray(buffer.layers["water_level"])
        biome = np.asarray(buffer.layers["biome_id"])
        for local_x, local_z in SAMPLE_COLUMNS:
            idx = _col_index(local_x, local_z)
            water_level = float(water[idx])
            rows.append(
                {
                    "profile": profile_key,
                    "local_x": local_x,
                    "local_z": local_z,
                    "surface_y": int(round(float(height[idx]))),
                    "water_y": (
                        int(round(water_level)) if water_level >= 0.0 else None
                    ),
                    "biome_id": int(biome[idx]),
                }
            )
    return rows


def main() -> None:
    rows = capture_baseline()
    BASELINE_PATH.parent.mkdir(parents=True, exist_ok=True)
    BASELINE_PATH.write_text(
        json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {len(rows)} baseline columns -> {BASELINE_PATH}")


if __name__ == "__main__":
    main()
