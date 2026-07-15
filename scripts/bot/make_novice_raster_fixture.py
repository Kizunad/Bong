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


def _write_flat_tile(output_dir: Path, tile_x: int, tile_z: int, biome_id: int) -> None:
    tile_dir = output_dir / f"tile_{tile_x}_{tile_z}"
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
    _write_repeated(tile_dir / "biome_id.bin", bytes([biome_id]), area)
    _write_repeated(tile_dir / "water_level.bin", struct.pack("<f", -1.0), area)
    _write_repeated(tile_dir / "feature_mask.bin", struct.pack("<f", 0.0), area)
    _write_repeated(tile_dir / "boundary_weight.bin", struct.pack("<f", 0.0), area)


def _tile_manifest(tile_x: int, tile_z: int) -> dict:
    return {
        "tile_x": tile_x,
        "tile_z": tile_z,
        "dir": f"tile_{tile_x}_{tile_z}",
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


def generate(output_dir: Path) -> Path:
    _write_flat_tile(output_dir, 0, 0, biome_id=0)
    # SpiritWood seed cell (0,0) resolves to (1292, 1519). Its production crown/root
    # bounds cross both the x=1280 and z=1536 tile edges, so the fixture must cover
    # all four touched tiles with one flat spawn biome. Otherwise the real outer
    # trunk can border fallback terrain tens of blocks higher than the tree base.
    spiritwood_tiles = [(4, 5), (5, 5), (4, 6), (5, 6)]
    for tile_x, tile_z in spiritwood_tiles:
        _write_flat_tile(output_dir, tile_x, tile_z, biome_id=4)

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
        "world_bounds": {"min_x": 0, "max_x": 1535, "min_z": 0, "max_z": 1791},
        "surface_palette": ["grass_block", "stone"],
        "biome_palette": ["plains"],
        "tiles": [_tile_manifest(0, 0)]
        + [_tile_manifest(tile_x, tile_z) for tile_x, tile_z in spiritwood_tiles],
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
