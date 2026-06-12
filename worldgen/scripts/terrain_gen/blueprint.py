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
    # --- worldgen-v4 P2 DSL fields (§P2 + §8.1 #4) ---
    # The declarative terrain DSL (terrain_style / surface_palette / flora_table /
    # qi_grade / qi_override).  Populated from the blueprint zone's optional DSL
    # block; absent → safe defaults (empty style, common qi_grade, no override) so
    # legacy blueprints without these fields still parse.  ``dsl`` is None when
    # the zone declares no DSL block at all (distinguish "not yet migrated" from
    # "explicitly empty").  Typed as Any to avoid a fields.py ↔ dsl.py import
    # cycle; the concrete type is dsl.ZoneTerrainDSL.
    dsl: Any | None = None


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
    # plan-tsy-worldgen-v1 §2.2.d — TSY 架构反转后 zone 必须显式标位面。
    #   "overworld" 默认 / "tsy" 由 zones.tsy.json 写。
    #   Rust ZoneConfig 已支持 (server/src/world/zone.rs:347, 474)，
    #   实际值域对齐 DimensionKind serde rename_all = "snake_case"。
    dimension: str = "overworld"
    # plan-terrain-wiring-v1 P0 M2 — layout fields passthrough.
    # Sourced from TerrainProfileSpec (terrain-profiles.example.json) and
    # propagated here so stitcher and export pass can read them without
    # re-loading the profile catalog.
    architectural_layout: str | None = None
    compound_flatten_radius: int | None = None


@dataclass(frozen=True)
class WorldBlueprint:
    version: int
    world_name: str
    spawn_zone: str
    bounds_xz: Bounds2D
    notes: tuple[str, ...]
    zones: tuple[BlueprintZone, ...]


@dataclass(frozen=True)
class ZoneOverlaySpec:
    zone_id: str
    overlay_kind: str
    payload: dict[str, Any]
    payload_version: int
    since_wall: int


@dataclass(frozen=True)
class TerrainProfileSpec:
    name: str
    boundary: BoundarySpec
    height: dict[str, Any]
    surface: tuple[str, ...]
    water: dict[str, Any]
    passability: str
    extras: dict[str, Any] = field(default_factory=dict)
    # --- layout infrastructure (plan-dandao-path-v1 PR-1) ---
    architectural_layout: str | None = None
    compound_flatten_radius: int | None = None
    # --- worldgen-v4 P2 DSL fields (§P2 + §8.1 #4) ---
    # A profile-level declarative DSL default (terrain_style / surface_palette /
    # flora_table / qi_grade / qi_override) the catalog can declare so a zone may
    # inherit it.  None when the profile declares no DSL block.  Typed Any to
    # avoid the fields.py ↔ dsl.py import cycle (concrete: dsl.ZoneTerrainDSL).
    dsl: Any | None = None


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


# worldgen-v4 P2: inline-DSL marker keys.  ``surface_palette`` is DELIBERATELY
# excluded — it already exists as the legacy block-override list[str] on
# ZoneWorldgenConfig (a string list, not structured DSL rules).  Structured DSL
# surface rules are only read from the nested ``worldgen.dsl`` block, so the two
# representations never collide.  Plan §P2 risk "surface_palette 字段歧义" resolved
# here: top-level surface_palette = block list; dsl.surface_palette = rules.
_INLINE_DSL_KEYS = ("terrain_style", "flora_table", "qi_grade", "qi_override")


def _has_inline_dsl(worldgen_raw: dict[str, Any]) -> bool:
    return any(key in worldgen_raw for key in _INLINE_DSL_KEYS)


def load_blueprint(path: Path) -> WorldBlueprint:
    with path.open(encoding="utf-8") as handle:
        raw = json.load(handle)
    return parse_blueprint(raw)


def parse_blueprint(raw: dict[str, Any]) -> WorldBlueprint:
    """Parse an already-loaded blueprint mapping into a ``WorldBlueprint``.

    Split out from ``load_blueprint`` so callers that hold an in-memory blueprint
    dict (e.g. the dev console applying per-zone parameter overrides without
    touching the on-disk source) reuse the exact same parsing + validation.
    """
    # Deferred import — dsl.py imports only noise (no blueprint), so this is safe
    # and keeps the module top-level free of a potential future cycle.
    from .dsl import parse_zone_terrain_dsl

    world_raw = raw["world"]
    zones: list[BlueprintZone] = []
    for zone_raw in raw["zones"]:
        worldgen_raw = zone_raw["worldgen"]
        # worldgen-v4 P2: the declarative DSL block lives under worldgen.dsl, OR
        # the DSL fields (terrain_style / surface_palette / flora_table /
        # qi_grade / qi_override) sit directly in the worldgen section.  We只在
        # 至少声明了一个 DSL 字段时构造 ZoneTerrainDSL，否则 dsl=None（区分"未迁移"
        # 与"显式空"），保持旧 blueprint 向后兼容。
        dsl_obj = None
        dsl_raw = worldgen_raw.get("dsl")
        if dsl_raw is None and _has_inline_dsl(worldgen_raw):
            # Inline DSL: read only the DSL marker keys, NOT the legacy
            # surface_palette block list (which would crash the structured rule
            # parser). Structured surface rules must use the nested dsl block.
            dsl_raw = {
                key: worldgen_raw[key]
                for key in _INLINE_DSL_KEYS
                if key in worldgen_raw
            }
        if dsl_raw is not None:
            dsl_obj = parse_zone_terrain_dsl(dsl_raw)

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
                    # DSL keys are not free-form extras.
                    "dsl",
                    "terrain_style",
                    "flora_table",
                    "qi_grade",
                    "qi_override",
                ),
            ),
            dsl=dsl_obj,
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
        # plan-terrain-wiring-v1 P0 M2: read layout fields from worldgen section.
        # architectural_layout lives at worldgen top level; compound_flatten_radius
        # may be inside height_model or at worldgen top level.
        _arch_layout: str | None = worldgen_raw.get("architectural_layout")
        _flatten_radius: int | None = None
        if "compound_flatten_radius" in worldgen_raw:
            _flatten_radius = int(worldgen_raw["compound_flatten_radius"])
        elif "compound_flatten_radius" in worldgen_raw.get("height_model", {}):
            _flatten_radius = int(worldgen_raw["height_model"]["compound_flatten_radius"])

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
                dimension=str(zone_raw.get("dimension", "minecraft:overworld")),
                architectural_layout=_arch_layout,
                compound_flatten_radius=_flatten_radius,
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


def load_zone_overlays(path: Path | None) -> tuple[ZoneOverlaySpec, ...]:
    if path is None:
        return ()

    with path.open(encoding="utf-8") as handle:
        raw = json.load(handle)

    overlay_raw = raw if isinstance(raw, list) else raw.get("zone_overlays", [])
    overlays: list[ZoneOverlaySpec] = []
    for item in overlay_raw:
        payload = item.get("payload", None)
        if payload is None:
            payload_json = str(item.get("payload_json", "{}"))
            payload = json.loads(payload_json)
        overlays.append(
            ZoneOverlaySpec(
                zone_id=str(item["zone_id"]),
                overlay_kind=str(item["overlay_kind"]),
                payload=dict(payload),
                payload_version=int(item.get("payload_version", 1)),
                since_wall=int(item.get("since_wall", 0)),
            )
        )
    return tuple(overlays)


def load_profile_catalog(path: Path) -> TerrainProfileCatalog:
    from .dsl import parse_zone_terrain_dsl

    with path.open(encoding="utf-8") as handle:
        raw = json.load(handle)

    profiles: dict[str, TerrainProfileSpec] = {}
    for profile_name, profile_raw in raw["profiles"].items():
        height_raw = dict(profile_raw.get("height", {}))
        # worldgen-v4 P2: profile-level DSL default. Read from nested ``dsl`` block
        # or the inline marker keys (terrain_style / flora_table / qi_grade /
        # qi_override). The legacy ``surface`` block list never collides because
        # the structured DSL surface rules live under ``dsl.surface_palette``.
        dsl_obj = None
        profile_dsl_raw = profile_raw.get("dsl")
        if profile_dsl_raw is None and _has_inline_dsl(profile_raw):
            profile_dsl_raw = {
                key: profile_raw[key]
                for key in _INLINE_DSL_KEYS
                if key in profile_raw
            }
        if profile_dsl_raw is not None:
            dsl_obj = parse_zone_terrain_dsl(profile_dsl_raw)
        # Extract compound_flatten_radius from height sub-dict if present.
        compound_flatten_radius: int | None = None
        if "compound_flatten_radius" in height_raw:
            compound_flatten_radius = int(height_raw.pop("compound_flatten_radius"))
        # Also check top-level (some profiles may put it there).
        if "compound_flatten_radius" in profile_raw:
            compound_flatten_radius = int(profile_raw["compound_flatten_radius"])
        profiles[str(profile_name)] = TerrainProfileSpec(
            name=str(profile_name),
            boundary=_parse_boundary(profile_raw["boundary"]),
            height=height_raw,
            surface=tuple(str(item) for item in profile_raw.get("surface", [])),
            water=dict(profile_raw.get("water", {})),
            passability=str(profile_raw.get("passability", "unknown")),
            extras=_pop_known(
                profile_raw,
                (
                    "boundary", "height", "surface", "water",
                    "passability", "architectural_layout",
                    "compound_flatten_radius",
                    # DSL keys are not free-form extras.
                    "dsl", "terrain_style", "flora_table",
                    "qi_grade", "qi_override",
                ),
            ),
            architectural_layout=profile_raw.get("architectural_layout"),
            compound_flatten_radius=compound_flatten_radius,
            dsl=dsl_obj,
        )

    return TerrainProfileCatalog(version=int(raw.get("version", 1)), profiles=profiles)
