"""Regenerate worldgen/fixtures/v3_surface_baseline.json from the v3 pipeline.

Run from the worldgen/ directory:

    python3 -m tests.regenerate_v3_baseline

The output is the frozen behaviour-equivalence golden for worldgen-v4 P0.
It is captured from the untouched v3 generators (height + carve masks) fed
through ``v3_surface_top_y`` — the Python mirror of the v3 Rust carve — so the
golden is the REAL v3 walkable surface, not ``round(height)``.  It must NOT be
regenerated unless a profile legitimately changes, because the whole point is to
pin v3 landscape behaviour.

CRITICAL (worldgen-v4 P0 BLOCKER): the previous golden recorded
``surface_y = round(height)`` and never applied the v3 rift/fracture/neg/
entrance sculpting, so it pinned ``round(height)`` against ``round(height)`` —
a circular no-op that could never catch the surface regression.  Now the golden
anchors on the carved surface and the profile matrix spans every carve branch.

Each entry records, per sampled column, the observable contract:
  surface_y = v3 carved top_y          (rift→fracture→neg→entrance, clamped)
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

from scripts.terrain_gen.spans_fold import v3_surface_top_y  # noqa: E402
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


def _layer(buffer, name: str, area: int, default: float) -> np.ndarray:
    if name in buffer.layers:
        return np.asarray(buffer.layers[name], dtype=np.float64).reshape(area)
    return np.full(area, default, dtype=np.float64)


def v3_carved_surface(buffer, idx: int) -> int:
    """The frozen golden surface = v3 carved top_y at one sampled column."""
    area = buffer.tile_size * buffer.tile_size
    height = np.asarray(buffer.layers["height"], dtype=np.float64).reshape(area)
    # rift_axis_sdf defaults to 99 (no rift); everything else to 0 (no effect).
    rift = _layer(buffer, "rift_axis_sdf", area, 99.0)
    rim = _layer(buffer, "rim_edge_mask", area, 0.0)
    frac = _layer(buffer, "fracture_mask", area, 0.0)
    neg = _layer(buffer, "neg_pressure", area, 0.0)
    ent = _layer(buffer, "entrance_mask", area, 0.0)
    return v3_surface_top_y(
        height=float(height[idx]),
        rift_axis_sdf=float(rift[idx]),
        rim_edge_mask=float(rim[idx]),
        fracture_mask=float(frac[idx]),
        neg_pressure=float(neg[idx]),
        entrance_mask=float(ent[idx]),
    )


def capture_baseline() -> list[dict]:
    rows: list[dict] = []
    for profile_key, _fill_fn, _zone in BASELINE_PROFILES:
        buffer = build_baseline_buffer(profile_key)
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
                    "surface_y": v3_carved_surface(buffer, idx),
                    "water_y": (
                        round(water_level) if water_level >= 0.0 else None
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
