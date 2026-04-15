from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .fields import Bounds2D

WORLDGEN_ROOT = Path(__file__).resolve().parents[2]
REPO_ROOT = WORLDGEN_ROOT.parent
DEFAULT_BLUEPRINT_PATH = REPO_ROOT / "server" / "zones.worldview.example.json"
DEFAULT_PROFILES_PATH = WORLDGEN_ROOT / "terrain-profiles.example.json"


@dataclass(frozen=True)
class BoundarySpec:
    mode: str
    width: int


@dataclass(frozen=True)
class ZoneWorldgenConfig:
    terrain_profile: str
    shape: str
    boundary: BoundarySpec
    height_model: dict[str, Any]
    surface_palette: tuple[str, ...]
    biome_mix: tuple[str, ...] = ()
    landmarks: tuple[str, ...] = ()
    extras: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class PoiSpec:
    """Narrative point-of-interest inside a zone.

    POIs are first-class references for the 天道 Agent / NPC AI / HUD to anchor
    stories on (洞府 / 碑铭 / 灵泉眼 / 血月祭坛 / 宗门废墟 ...). They are
    serialized into the raster manifest so the Rust server can surface them to
    downstream consumers without re-parsing the blueprint.
    """

    kind: str                           # cave_mouth | ruin | spirit_font | stele | altar | tomb | shrine | ...
    pos_xyz: tuple[float, float, float]
    name: str = ""
    tags: tuple[str, ...] = ()
    unlock: str = ""                    # free-form unlock condition text for agent
    qi_affinity: float = 0.0            # [-1, 1] local qi bias (negative = sink)
    danger_bias: int = 0                # delta to zone.danger_level when nearby


@dataclass(frozen=True)
class BlueprintZone:
    name: str
    display_name: str
    bounds_xz: Bounds2D
    center_xz: tuple[int, int]
    size_xz: tuple[int, int]
    spirit_qi: float
    danger_level: int
    worldgen: ZoneWorldgenConfig
    pois: tuple[PoiSpec, ...] = ()


@dataclass(frozen=True)
class WorldBlueprint:
    version: int
    world_name: str
    spawn_zone: str
    bounds_xz: Bounds2D
    notes: tuple[str, ...]
    zones: tuple[BlueprintZone, ...]


@dataclass(frozen=True)
class TerrainProfileSpec:
    name: str
    boundary: BoundarySpec
    height: dict[str, Any]
    surface: tuple[str, ...]
    water: dict[str, Any]
    passability: str
    extras: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class TerrainProfileCatalog:
    version: int
    profiles: dict[str, TerrainProfileSpec]


def _bounds_from_aabb(aabb: dict[str, Any]) -> Bounds2D:
    min_x, _, min_z = aabb["min"]
    max_x, _, max_z = aabb["max"]
    return Bounds2D(
        min_x=int(round(min_x)),
        max_x=int(round(max_x)),
        min_z=int(round(min_z)),
        max_z=int(round(max_z)),
    )


def _bounds_from_world(world_bounds: dict[str, Any]) -> Bounds2D:
    min_x, min_z = world_bounds["min"]
    max_x, max_z = world_bounds["max"]
    return Bounds2D(
        min_x=int(round(min_x)),
        max_x=int(round(max_x)),
        min_z=int(round(min_z)),
        max_z=int(round(max_z)),
    )


def _parse_boundary(raw: dict[str, Any]) -> BoundarySpec:
    return BoundarySpec(mode=str(raw["mode"]), width=int(raw["width"]))


def _pop_known(raw: dict[str, Any], keys: tuple[str, ...]) -> dict[str, Any]:
    return {key: value for key, value in raw.items() if key not in keys}


def load_blueprint(path: Path) -> WorldBlueprint:
    with path.open(encoding="utf-8") as handle:
        raw = json.load(handle)

    world_raw = raw["world"]
    zones: list[BlueprintZone] = []
    for zone_raw in raw["zones"]:
        worldgen_raw = zone_raw["worldgen"]
        worldgen = ZoneWorldgenConfig(
            terrain_profile=str(worldgen_raw["terrain_profile"]),
            shape=str(worldgen_raw.get("shape", "unknown")),
            boundary=_parse_boundary(worldgen_raw["boundary"]),
            height_model=dict(worldgen_raw.get("height_model", {})),
            surface_palette=tuple(
                str(item) for item in worldgen_raw.get("surface_palette", [])
            ),
            biome_mix=tuple(str(item) for item in worldgen_raw.get("biome_mix", [])),
            landmarks=tuple(str(item) for item in worldgen_raw.get("landmarks", [])),
            extras=_pop_known(
                worldgen_raw,
                (
                    "terrain_profile",
                    "shape",
                    "boundary",
                    "height_model",
                    "surface_palette",
                    "biome_mix",
                    "landmarks",
                ),
            ),
        )
        center_x, center_z = zone_raw.get("center_xz", [0, 0])
        size_x, size_z = zone_raw.get("size_xz", [0, 0])
        pois: list[PoiSpec] = []
        for poi_raw in zone_raw.get("pois", []):
            pos = poi_raw.get("pos_xyz", [0.0, 0.0, 0.0])
            pois.append(
                PoiSpec(
                    kind=str(poi_raw["kind"]),
                    pos_xyz=(
                        float(pos[0]),
                        float(pos[1]) if len(pos) > 1 else 0.0,
                        float(pos[2]) if len(pos) > 2 else 0.0,
                    ),
                    name=str(poi_raw.get("name", "")),
                    tags=tuple(str(item) for item in poi_raw.get("tags", [])),
                    unlock=str(poi_raw.get("unlock", "")),
                    qi_affinity=float(poi_raw.get("qi_affinity", 0.0)),
                    danger_bias=int(poi_raw.get("danger_bias", 0)),
                )
            )
        zones.append(
            BlueprintZone(
                name=str(zone_raw["name"]),
                display_name=str(zone_raw.get("display_name", zone_raw["name"])),
                bounds_xz=_bounds_from_aabb(zone_raw["aabb"]),
                center_xz=(int(round(center_x)), int(round(center_z))),
                size_xz=(int(round(size_x)), int(round(size_z))),
                spirit_qi=float(zone_raw["spirit_qi"]),
                danger_level=int(zone_raw["danger_level"]),
                worldgen=worldgen,
                pois=tuple(pois),
            )
        )

    return WorldBlueprint(
        version=int(raw.get("version", 1)),
        world_name=str(world_raw["name"]),
        spawn_zone=str(world_raw["spawn_zone"]),
        bounds_xz=_bounds_from_world(world_raw["bounds_xz"]),
        notes=tuple(str(item) for item in world_raw.get("notes", [])),
        zones=tuple(zones),
    )


def load_profile_catalog(path: Path) -> TerrainProfileCatalog:
    with path.open(encoding="utf-8") as handle:
        raw = json.load(handle)

    profiles: dict[str, TerrainProfileSpec] = {}
    for profile_name, profile_raw in raw["profiles"].items():
        profiles[str(profile_name)] = TerrainProfileSpec(
            name=str(profile_name),
            boundary=_parse_boundary(profile_raw["boundary"]),
            height=dict(profile_raw.get("height", {})),
            surface=tuple(str(item) for item in profile_raw.get("surface", [])),
            water=dict(profile_raw.get("water", {})),
            passability=str(profile_raw.get("passability", "unknown")),
            extras=_pop_known(
                profile_raw,
                ("boundary", "height", "surface", "water", "passability"),
            ),
        )

    return TerrainProfileCatalog(version=int(raw.get("version", 1)), profiles=profiles)
