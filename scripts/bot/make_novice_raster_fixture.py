#!/usr/bin/env python3
"""Generate a tiny flat raster world with six real novice POIs for Bot e2e."""

from __future__ import annotations

import argparse
import json
import math
import struct
from pathlib import Path

TILE_SIZE = 256
SURFACE_Y = 72
SPAN_SENTINEL = 32767
DEFAULT_ZONES_PATH = Path(__file__).resolve().parents[2] / "server" / "zones.json"
SPIRITWOOD_TILES = {(4, 5), (5, 5), (4, 6), (5, 6)}

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


def _spawn_distribution(zones_path: Path = DEFAULT_ZONES_PATH) -> list[dict]:
    config = json.loads(zones_path.read_text(encoding="utf-8"))
    spawn_zone = next(
        (zone for zone in config["zones"] if zone.get("name") == "spawn"), None
    )
    if spawn_zone is None:
        raise ValueError(f"{zones_path} is missing the spawn zone")
    distribution = spawn_zone.get("spawn_distribution", [])
    if not distribution:
        raise ValueError(f"{zones_path} spawn zone has no spawn_distribution")
    return distribution


def spawn_fixture_tiles(zones_path: Path = DEFAULT_ZONES_PATH) -> set[tuple[int, int]]:
    tiles = set()
    for index, cluster in enumerate(_spawn_distribution(zones_path)):
        anchor = cluster.get("anchor")
        radius = cluster.get("radius")
        if (
            not isinstance(anchor, list)
            or len(anchor) != 3
            or not all(isinstance(value, (int, float)) for value in anchor)
            or not all(math.isfinite(float(value)) for value in anchor)
            or not isinstance(radius, (int, float))
            or not math.isfinite(float(radius))
            or radius < 0
            or cluster.get("safe_y") != SURFACE_Y
        ):
            raise ValueError(f"invalid spawn_distribution[{index}] in {zones_path}")
        min_tile_x = math.floor((anchor[0] - radius) / TILE_SIZE)
        max_tile_x = math.floor((anchor[0] + radius) / TILE_SIZE)
        min_tile_z = math.floor((anchor[2] - radius) / TILE_SIZE)
        max_tile_z = math.floor((anchor[2] + radius) / TILE_SIZE)
        tiles.update(
            (tile_x, tile_z)
            for tile_x in range(min_tile_x, max_tile_x + 1)
            for tile_z in range(min_tile_z, max_tile_z + 1)
        )
    return tiles


def _world_bounds(tiles: set[tuple[int, int]]) -> dict[str, int]:
    return {
        "min_x": min(tile_x for tile_x, _ in tiles) * TILE_SIZE,
        "max_x": (max(tile_x for tile_x, _ in tiles) + 1) * TILE_SIZE - 1,
        "min_z": min(tile_z for _, tile_z in tiles) * TILE_SIZE,
        "max_z": (max(tile_z for _, tile_z in tiles) + 1) * TILE_SIZE - 1,
    }


def generate(output_dir: Path, fixture_token: str) -> Path:
    if not fixture_token or fixture_token != fixture_token.strip():
        raise ValueError("fixture_token must be non-empty and contain no surrounding whitespace")

    # Generic Bot users retain production spawn selection. Cover every tile touched
    # by the authoritative spawn_distribution so username hashes cannot fall through
    # to the raster loader's stone fallback.
    spawn_tiles = spawn_fixture_tiles()
    # SpiritWood seed cell (0,0) resolves to (1292, 1519). Its production crown/root
    # bounds cross both the x=1280 and z=1536 tile edges, so the fixture must cover
    # all four touched tiles with one flat meadow biome.
    all_tiles = spawn_tiles | SPIRITWOOD_TILES
    for tile_x, tile_z in sorted(all_tiles):
        biome_id = 4 if (tile_x, tile_z) in SPIRITWOOD_TILES else 0
        _write_flat_tile(output_dir, tile_x, tile_z, biome_id=biome_id)

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
        "bot_fixture": {
            "kind": "ambient-surface-v1",
            "token": fixture_token,
            "surface_y": SURFACE_Y,
            "support": "grass_block",
            "feet_y": SURFACE_Y + 1,
            "head_y": SURFACE_Y + 2,
        },
        "world_bounds": _world_bounds(all_tiles),
        "surface_palette": ["grass_block", "stone"],
        "biome_palette": [
            "minecraft:plains",
            "minecraft:stony_peaks",
            "minecraft:swamp",
            "minecraft:badlands",
            "minecraft:meadow",
        ],
        "tiles": [_tile_manifest(tile_x, tile_z) for tile_x, tile_z in sorted(all_tiles)],
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
    parser.add_argument("--fixture-token", required=True)
    args = parser.parse_args()
    print(generate(args.output_dir, args.fixture_token))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
