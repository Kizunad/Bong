from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from typing import TYPE_CHECKING, Iterable

import numpy as np

if TYPE_CHECKING:
    from .terrain_gen.fields import GeneratedFieldSet


class PoiType(StrEnum):
    FORGE_STATION = "forge_station"
    ALCHEMY_FURNACE = "alchemy_furnace"
    ROGUE_VILLAGE = "rogue_village"
    MUTANT_NEST = "mutant_nest"
    SCROLL_HIDDEN = "scroll_hidden"
    SPIRIT_HERB_VALLEY = "spirit_herb_valley"


@dataclass(frozen=True)
class QiRange:
    min: float
    max: float

    def relaxed(self, margin: float) -> "QiRange":
        return QiRange(max(0.0, self.min - margin), min(1.0, self.max + margin))


@dataclass(frozen=True)
class Vec3:
    x: float
    y: float
    z: float

    def as_tuple(self) -> tuple[float, float, float]:
        return (self.x, self.y, self.z)


@dataclass(frozen=True)
class TerrainField:
    height: np.ndarray
    water_mask: np.ndarray
    slope: np.ndarray
    origin_x: int = 0
    origin_z: int = 0
    cell_size: int = 1

    @classmethod
    def from_height(
        cls,
        height: np.ndarray,
        *,
        water_mask: np.ndarray | None = None,
        origin_x: int = 0,
        origin_z: int = 0,
        cell_size: int = 1,
    ) -> "TerrainField":
        height = np.asarray(height, dtype=np.float64)
        if height.ndim != 2:
            raise ValueError("height must be a 2D array")
        if water_mask is None:
            water_mask = np.zeros_like(height, dtype=bool)
        else:
            water_mask = np.asarray(water_mask, dtype=bool)
        if water_mask.shape != height.shape:
            raise ValueError("water_mask shape must match height")
        grad_z, grad_x = np.gradient(height, float(cell_size), float(cell_size))
        slope = np.sqrt(grad_x * grad_x + grad_z * grad_z)
        return cls(
            height=height,
            water_mask=water_mask,
            slope=slope,
            origin_x=origin_x,
            origin_z=origin_z,
            cell_size=cell_size,
        )


@dataclass(frozen=True)
class PoiSelection:
    poi_type: PoiType
    pos: Vec3
    qi_value: float
    strategy: str


POI_QI_REQUIREMENTS: dict[PoiType, QiRange] = {
    PoiType.FORGE_STATION: QiRange(0.4, 0.6),
    PoiType.ALCHEMY_FURNACE: QiRange(0.3, 0.5),
    PoiType.ROGUE_VILLAGE: QiRange(0.4, 0.6),
    PoiType.MUTANT_NEST: QiRange(0.5, 0.7),
    PoiType.SCROLL_HIDDEN: QiRange(0.3, 0.5),
    PoiType.SPIRIT_HERB_VALLEY: QiRange(0.4, 0.7),
}

FALLBACK_LOCATIONS: dict[PoiType, Vec3] = {
    PoiType.FORGE_STATION: Vec3(300.0, 70.0, 200.0),
    PoiType.ALCHEMY_FURNACE: Vec3(-400.0, 70.0, 100.0),
    PoiType.ROGUE_VILLAGE: Vec3(500.0, 70.0, -300.0),
    PoiType.MUTANT_NEST: Vec3(1200.0, 70.0, 800.0),
    PoiType.SCROLL_HIDDEN: Vec3(-800.0, 70.0, -1200.0),
    PoiType.SPIRIT_HERB_VALLEY: Vec3(-300.0, 70.0, 600.0),
}

POI_BLUEPRINTS: dict[PoiType, dict[str, object]] = {
    PoiType.FORGE_STATION: {
        "name": "破败炼器台",
        "gameplay": "forge",
        "entity": "forge_station",
        "loot": "none",
        "refresh": "permanent",
        "blocks": ("campfire", "anvil", "mossy_cobblestone", "bone_block"),
        "qi_affinity": 0.15,
        "danger_bias": 0,
        "unlock": "引气期可用；醒灵期仅可见",
    },
    PoiType.ALCHEMY_FURNACE: {
        "name": "凡铁丹炉",
        "gameplay": "alchemy",
        "entity": "alchemy_furnace",
        "loot": "iron_pot",
        "refresh": "permanent",
        "blocks": ("cauldron", "campfire", "coarse_dirt", "bone_block"),
        "qi_affinity": 0.10,
        "danger_bias": 0,
        "unlock": "引气期可用；醒灵期仅可见",
    },
    PoiType.ROGUE_VILLAGE: {
        "name": "散修聚居点",
        "gameplay": "social",
        "entity": "rogue_npc_cluster",
        "loot": "dead_letter_mailbox",
        "refresh": "npc_24h_server_time",
        "blocks": ("oak_planks", "stripped_oak_log", "barrel", "bone_block"),
        "qi_affinity": 0.12,
        "danger_bias": 1,
        "unlock": "引气期可交易；屠村后 1 周拒绝交易",
    },
    PoiType.MUTANT_NEST: {
        "name": "异变兽巢",
        "gameplay": "combat",
        "entity": "zombie_mutant_placeholder",
        "loot": "beast_core_stub",
        "refresh": "mutant_24h_server_time",
        "blocks": ("cobweb", "bone_block", "soul_sand", "mossy_cobblestone"),
        "qi_affinity": 0.20,
        "danger_bias": 3,
        "unlock": "高难战斗点；凝脉期才可稳定单杀",
    },
    PoiType.SCROLL_HIDDEN: {
        "name": "残卷藏匿点",
        "gameplay": "knowledge",
        "entity": "cave_network_entrance",
        "loot": "skill_scroll_cache",
        "refresh": "scroll_7d_real_time",
        "blocks": ("stone", "mossy_cobblestone", "chiseled_stone_bricks", "bone_block"),
        "qi_affinity": 0.08,
        "danger_bias": 1,
        "unlock": "可拾取 1-2 张随机残卷",
    },
    PoiType.SPIRIT_HERB_VALLEY: {
        "name": "灵草谷",
        "gameplay": "gather",
        "entity": "botany_static_points",
        "loot": "ningmai_cao,yinqi_cao,jiegu_rui,anshen_guo,qingzhuo_cao",
        "refresh": "botany_natural_growth",
        "blocks": ("grass_block", "moss_block", "fern", "bone_block"),
        "qi_affinity": 0.18,
        "danger_bias": 0,
        "unlock": "基础灵草集中采集点",
    },
}

_RELAXATION_STEPS: tuple[tuple[int, float, str], ...] = (
    (1500, 0.0, "strict_radius_1500"),
    (2000, 0.0, "relaxed_radius_2000"),
    (2000, 0.1, "relaxed_radius_2000_qi_margin_0_1"),
)


def select_poi_locations(
    spawn_center: Vec3,
    radius: int,
    spirit_qi_field: np.ndarray,
    terrain_field: TerrainField,
    *,
    min_spawn_distance: int = 200,
    min_same_type_distance: int = 1000,
    min_cross_type_distance: int = 64,
    poi_types: Iterable[PoiType] = tuple(PoiType),
) -> dict[PoiType, list[Vec3]]:
    selections = select_poi_location_records(
        spawn_center,
        radius,
        spirit_qi_field,
        terrain_field,
        min_spawn_distance=min_spawn_distance,
        min_same_type_distance=min_same_type_distance,
        min_cross_type_distance=min_cross_type_distance,
        poi_types=poi_types,
    )
    return {poi_type: [selection.pos for selection in rows] for poi_type, rows in selections.items()}


def select_poi_location_records(
    spawn_center: Vec3,
    radius: int,
    spirit_qi_field: np.ndarray,
    terrain_field: TerrainField,
    *,
    min_spawn_distance: int = 200,
    min_same_type_distance: int = 1000,
    min_cross_type_distance: int = 64,
    poi_types: Iterable[PoiType] = tuple(PoiType),
) -> dict[PoiType, list[PoiSelection]]:
    qi = np.asarray(spirit_qi_field, dtype=np.float64)
    if qi.shape != terrain_field.height.shape:
        raise ValueError("spirit_qi_field shape must match terrain field height")

    selected: dict[PoiType, list[PoiSelection]] = {}
    occupied_positions: list[Vec3] = []
    for poi_type in poi_types:
        record = _select_one(
            poi_type,
            spawn_center,
            radius,
            qi,
            terrain_field,
            min_spawn_distance,
            min_same_type_distance,
            min_cross_type_distance,
            occupied_positions,
        )
        selected[poi_type] = [record]
        occupied_positions.append(record.pos)
    return selected


def build_novice_poi_manifest_payload(
    fields: "GeneratedFieldSet",
    *,
    spawn_center: Vec3 = Vec3(0.0, 70.0, 0.0),
    radius: int = 1500,
    sample_stride: int = 8,
) -> list[dict[str, object]]:
    qi, terrain = _field_set_to_selector_inputs(fields, sample_stride=sample_stride)
    records = select_poi_location_records(
        spawn_center,
        radius,
        qi,
        terrain,
        min_spawn_distance=200,
        min_same_type_distance=1000,
    )

    payload: list[dict[str, object]] = []
    for poi_type in PoiType:
        record = records[poi_type][0]
        blueprint = POI_BLUEPRINTS[poi_type]
        qi_range = POI_QI_REQUIREMENTS[poi_type]
        tags = [
            "poi_novice",
            f"poi_type:{poi_type.value}",
            f"gameplay:{blueprint['gameplay']}",
            f"entity:{blueprint['entity']}",
            f"loot:{blueprint['loot']}",
            f"refresh:{blueprint['refresh']}",
            f"selection:{record.strategy}",
            f"qi_range:{qi_range.min:.1f}-{qi_range.max:.1f}",
            "visual:ash_ruin_bone",
            "silent_guidance",
        ]
        payload.append(
            {
                "zone": "spawn",
                "kind": f"novice_{poi_type.value}",
                "name": str(blueprint["name"]),
                "pos_xyz": [record.pos.x, record.pos.y, record.pos.z],
                "tags": tags,
                "unlock": str(blueprint["unlock"]),
                "qi_affinity": float(blueprint["qi_affinity"]),
                "danger_bias": int(blueprint["danger_bias"]),
            }
        )
    return payload


def _select_one(
    poi_type: PoiType,
    spawn_center: Vec3,
    radius: int,
    qi: np.ndarray,
    terrain: TerrainField,
    min_spawn_distance: int,
    min_same_type_distance: int,
    min_cross_type_distance: int,
    occupied_positions: Iterable[Vec3],
) -> PoiSelection:
    for step_radius, qi_margin, label in _RELAXATION_STEPS:
        effective_radius = max(radius, step_radius)
        candidates = _candidate_indices(
            spawn_center,
            effective_radius,
            POI_QI_REQUIREMENTS[poi_type].relaxed(qi_margin),
            qi,
            terrain,
            min_spawn_distance,
            min_cross_type_distance,
            occupied_positions,
        )
        if candidates.size == 0:
            continue
        row, col = _pick_best_candidate(spawn_center, candidates, qi, terrain)
        pos = _pos_from_index(row, col, terrain)
        return PoiSelection(
            poi_type=poi_type,
            pos=pos,
            qi_value=float(qi[row, col]),
            strategy=label,
        )

    fallback = FALLBACK_LOCATIONS[poi_type]
    return PoiSelection(
        poi_type=poi_type,
        pos=fallback,
        qi_value=float("nan"),
        strategy=f"fallback_fixed_after_{len(_RELAXATION_STEPS)}_attempts",
    )


def _candidate_indices(
    spawn_center: Vec3,
    radius: int,
    qi_range: QiRange,
    qi: np.ndarray,
    terrain: TerrainField,
    min_spawn_distance: int,
    min_cross_type_distance: int,
    occupied_positions: Iterable[Vec3],
) -> np.ndarray:
    rows, cols = np.indices(qi.shape)
    world_x = terrain.origin_x + cols * terrain.cell_size
    world_z = terrain.origin_z + rows * terrain.cell_size
    dist = np.sqrt((world_x - spawn_center.x) ** 2 + (world_z - spawn_center.z) ** 2)
    valid = (
        (qi >= qi_range.min)
        & (qi <= qi_range.max)
        & (dist <= radius)
        & (dist >= min_spawn_distance)
        & (terrain.slope < 0.3)
        & (~terrain.water_mask)
    )
    for occupied in occupied_positions:
        occupied_dist = np.sqrt((world_x - occupied.x) ** 2 + (world_z - occupied.z) ** 2)
        valid &= occupied_dist >= min_cross_type_distance
    return np.argwhere(valid)


def _pick_best_candidate(
    spawn_center: Vec3,
    candidates: np.ndarray,
    qi: np.ndarray,
    terrain: TerrainField,
) -> tuple[int, int]:
    rows = candidates[:, 0]
    cols = candidates[:, 1]
    world_x = terrain.origin_x + cols * terrain.cell_size
    world_z = terrain.origin_z + rows * terrain.cell_size
    dist = np.sqrt((world_x - spawn_center.x) ** 2 + (world_z - spawn_center.z) ** 2)
    slope = terrain.slope[rows, cols]
    qi_value = qi[rows, cols]
    score = dist * 0.002 + slope * 6.0 - qi_value * 0.5
    best = int(np.argmin(score))
    return int(rows[best]), int(cols[best])


def _pos_from_index(row: int, col: int, terrain: TerrainField) -> Vec3:
    return Vec3(
        x=float(terrain.origin_x + col * terrain.cell_size),
        y=float(round(terrain.height[row, col]) + 1.0),
        z=float(terrain.origin_z + row * terrain.cell_size),
    )


def _field_set_to_selector_inputs(
    fields: "GeneratedFieldSet", *, sample_stride: int
) -> tuple[np.ndarray, TerrainField]:
    if sample_stride <= 0:
        raise ValueError("sample_stride must be positive")
    if "qi_density" not in fields.layers:
        raise ValueError("GeneratedFieldSet must include qi_density for novice POI selection")

    tiles = fields.tiles
    if not tiles:
        raise ValueError("GeneratedFieldSet has no tiles")

    min_x = min(tile.tile.min_x for tile in tiles)
    max_x = max(tile.tile.max_x for tile in tiles)
    min_z = min(tile.tile.min_z for tile in tiles)
    max_z = max(tile.tile.max_z for tile in tiles)
    width = ((max_x - min_x) // sample_stride) + 1
    depth = ((max_z - min_z) // sample_stride) + 1

    qi = np.full((depth, width), np.nan, dtype=np.float64)
    height = np.full((depth, width), 70.0, dtype=np.float64)
    water_mask = np.ones((depth, width), dtype=bool)

    for tile in tiles:
        tile_size = tile.tile_size
        height_tile = tile.layers["height"].reshape((tile_size, tile_size))
        qi_tile = tile.layers["qi_density"].reshape((tile_size, tile_size))
        water_tile = tile.layers["water_level"].reshape((tile_size, tile_size))
        for local_z in range(0, tile_size, sample_stride):
            world_z = tile.tile.min_z + local_z
            row = (world_z - min_z) // sample_stride
            for local_x in range(0, tile_size, sample_stride):
                world_x = tile.tile.min_x + local_x
                col = (world_x - min_x) // sample_stride
                h = float(height_tile[local_z, local_x])
                water = float(water_tile[local_z, local_x])
                qi[row, col] = float(qi_tile[local_z, local_x])
                height[row, col] = h
                water_mask[row, col] = water >= 0.0 and h < water + 0.75

    terrain = TerrainField.from_height(
        height,
        water_mask=water_mask,
        origin_x=min_x,
        origin_z=min_z,
        cell_size=sample_stride,
    )
    return qi, terrain
