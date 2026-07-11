#!/usr/bin/env python3
"""Generate a tiny flat raster world with six real novice POIs for Bot e2e."""

from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path

TILE_SIZE = 256
SURFACE_Y = 72
SPAN_SENTINEL = 32767

POIS = [
    ("forge_station", "破败炼器台", [224.0, 73.0, 240.0], "strict_radius_1500"),
    ("alchemy_furnace", "凡铁丹炉", [32.0, 73.0, 200.0], "strict_radius_1500"),
    ("rogue_village", "散修聚居点", [208.0, 73.0, 48.0], "strict_radius_1500"),
    ("mutant_nest", "异变兽巢", [240.0, 73.0, 240.0], "relaxed_radius_2000"),
    ("scroll_hidden", "残卷藏匿点", [176.0, 73.0, 96.0], "strict_radius_1500"),
    (
        "spirit_herb_valley",
        "灵草谷",
        [48.0, 73.0, 224.0],
        "relaxed_radius_2000_qi_margin_0_1",
    ),
]


def _write_repeated(path: Path, value: bytes, count: int) -> None:
    path.write_bytes(value * count)


def generate(output_dir: Path) -> Path:
    tile_dir = output_dir / "tile_0_0"
    tile_dir.mkdir(parents=True, exist_ok=True)
    area = TILE_SIZE * TILE_SIZE

    _write_repeated(tile_dir / "spans_count.bin", b"\x01", area)
    span = struct.pack(
        "<8h",
        -64,
        SURFACE_Y,
        SPAN_SENTINEL,
        SPAN_SENTINEL,
        SPAN_SENTINEL,
        SPAN_SENTINEL,
        SPAN_SENTINEL,
        SPAN_SENTINEL,
    )
    _write_repeated(tile_dir / "spans.bin", span, area)
    _write_repeated(tile_dir / "surface_id.bin", b"\x00", area)
    _write_repeated(tile_dir / "subsurface_id.bin", b"\x01", area)
    _write_repeated(tile_dir / "biome_id.bin", b"\x00", area)
    _write_repeated(tile_dir / "water_level.bin", struct.pack("<f", -1.0), area)
    _write_repeated(tile_dir / "feature_mask.bin", struct.pack("<f", 0.0), area)
    _write_repeated(tile_dir / "boundary_weight.bin", struct.pack("<f", 0.0), area)

    pois = []
    for index, (kind, name, pos_xyz, strategy) in enumerate(POIS, start=1):
        pois.append(
            {
                "zone": "spawn",
                "kind": f"novice_{kind}",
                "name": name,
                "pos_xyz": pos_xyz,
                "tags": [
                    "poi_novice",
                    f"poi_type:{kind}",
                    f"selection:{strategy}",
                    "fixture:bot_e2e",
                ],
                "unlock": f"bot_fixture_unlock_{index}",
                "qi_affinity": round(index / 10.0, 1),
                "danger_bias": index,
            }
        )

    manifest = {
        "version": 2,
        "tile_size": TILE_SIZE,
        "world_bounds": {"min_x": 0, "max_x": 255, "min_z": 0, "max_z": 255},
        "surface_palette": ["grass_block", "stone"],
        "biome_palette": ["plains"],
        "tiles": [
            {
                "tile_x": 0,
                "tile_z": 0,
                "dir": "tile_0_0",
                "zones": ["spawn"],
                "layers": [
                    "surface_id",
                    "subsurface_id",
                    "biome_id",
                    "water_level",
                    "feature_mask",
                    "boundary_weight",
                ],
                "spans": True,
            }
        ],
        "pois": pois,
    }
    manifest_path = output_dir / "manifest.json"
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return manifest_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output_dir", type=Path)
    args = parser.parse_args()
    print(generate(args.output_dir))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
