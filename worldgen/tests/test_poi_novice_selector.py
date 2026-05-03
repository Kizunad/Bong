from __future__ import annotations

import math

import numpy as np

from scripts.poi_novice_selector import (
    PoiType,
    TerrainField,
    Vec3,
    build_novice_poi_manifest_payload,
    select_poi_location_records,
)
from scripts.terrain_gen.fields import GeneratedFieldSet, SurfacePalette, TileFieldBuffer, WorldTile


def test_selector_picks_valid_qi_and_avoids_spawn_core_water_and_slope() -> None:
    height = np.full((80, 80), 70.0)
    qi = np.full((80, 80), 0.45)
    water = np.zeros((80, 80), dtype=bool)
    water[30:40, 30:40] = True
    height[10:20, 10:20] = np.linspace(70.0, 120.0, 10).reshape(10, 1)
    terrain = TerrainField.from_height(
        height,
        water_mask=water,
        origin_x=-320,
        origin_z=-320,
        cell_size=8,
    )

    records = select_poi_location_records(Vec3(0.0, 70.0, 0.0), 1500, qi, terrain)

    for poi_type in (
        PoiType.FORGE_STATION,
        PoiType.ALCHEMY_FURNACE,
        PoiType.ROGUE_VILLAGE,
        PoiType.SCROLL_HIDDEN,
        PoiType.SPIRIT_HERB_VALLEY,
    ):
        record = records[poi_type][0]
        assert record.strategy == "strict_radius_1500"
        assert 200 <= math.hypot(record.pos.x, record.pos.z) <= 1500
        assert not (-80 <= record.pos.x <= 0 and -80 <= record.pos.z <= 0)


def test_selector_falls_back_after_radius_and_qi_relaxation_are_exhausted() -> None:
    height = np.full((20, 20), 70.0)
    qi = np.full((20, 20), 0.05)
    terrain = TerrainField.from_height(height, origin_x=-80, origin_z=-80, cell_size=8)

    records = select_poi_location_records(
        Vec3(0.0, 70.0, 0.0),
        1500,
        qi,
        terrain,
        poi_types=(PoiType.MUTANT_NEST,),
    )

    record = records[PoiType.MUTANT_NEST][0]
    assert record.strategy == "fallback_fixed_after_3_attempts"
    assert record.pos.as_tuple() == (1200.0, 70.0, 800.0)


def test_manifest_payload_exports_six_novice_pois_with_selection_tags() -> None:
    tile_size = 32
    tile = WorldTile(tile_x=0, tile_z=0, min_x=0, max_x=31, min_z=0, max_z=31)
    buffer = TileFieldBuffer.create(
        tile,
        tile_size,
        ("height", "water_level", "qi_density"),
    )
    buffer.layers["height"] = np.full(tile_size * tile_size, 70.0)
    buffer.layers["water_level"] = np.full(tile_size * tile_size, -1.0)
    buffer.layers["qi_density"] = np.full(tile_size * tile_size, 0.55)
    fields = GeneratedFieldSet(
        tile_size=tile_size,
        surface_palette=SurfacePalette(),
        layers=("height", "water_level", "qi_density"),
        tiles=[buffer],
    )

    payload = build_novice_poi_manifest_payload(
        fields,
        spawn_center=Vec3(-300.0, 70.0, -300.0),
        radius=1500,
        sample_stride=8,
    )

    assert [entry["kind"] for entry in payload] == [
        "novice_forge_station",
        "novice_alchemy_furnace",
        "novice_rogue_village",
        "novice_mutant_nest",
        "novice_scroll_hidden",
        "novice_spirit_herb_valley",
    ]
    assert all("poi_novice" in entry["tags"] for entry in payload)
    assert all(any(str(tag).startswith("selection:") for tag in entry["tags"]) for entry in payload)
