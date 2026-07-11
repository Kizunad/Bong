"""Carver owner 与 manifest provenance 必须是两套独立语义。"""

from __future__ import annotations

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np

from scripts.terrain_gen.bakers.raster_export import (
    CARVE_SEED,
    _tile_carver_chain,
    _zone_carver_chains,
)
from scripts.terrain_gen.blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    load_blueprint,
    load_profile_catalog,
)
from scripts.terrain_gen.fields import TileFieldBuffer, WorldTile
from scripts.terrain_gen.noise import _tile_coords
from scripts.terrain_gen.carvers import apply_carver_chain
from scripts.terrain_gen.spans_fold import spans_for_tile
from scripts.terrain_gen.stitcher import (
    _blend_tile_layers,
    _compute_boundary_weight_array,
    build_generation_plan,
    synthesize_fields,
)


TILE_SIZE = 2
CORE_LAYERS = (
    "height",
    "surface_id",
    "subsurface_id",
    "biome_id",
    "water_level",
    "feature_mask",
    "boundary_weight",
)


def _buffer() -> TileFieldBuffer:
    tile = WorldTile(
        tile_x=0,
        tile_z=0,
        min_x=0,
        max_x=TILE_SIZE - 1,
        min_z=0,
        max_z=TILE_SIZE - 1,
    )
    return TileFieldBuffer.create(tile, TILE_SIZE, CORE_LAYERS)


class BlendTileCarverOwnerTest(unittest.TestCase):
    def test_zero_weight_zone_stays_provenance_only(self) -> None:
        base = _buffer()
        overlay = _buffer()
        zone = SimpleNamespace(name="provenance_only")

        with patch(
            "scripts.terrain_gen.stitcher._compute_boundary_weight_array",
            return_value=np.zeros((TILE_SIZE, TILE_SIZE), dtype=np.float32),
        ):
            _blend_tile_layers(base, overlay, zone)

        self.assertEqual(
            base.contributing_zones,
            ["provenance_only"],
            "粗 AABB 命中仍应保留在 manifest provenance",
        )
        self.assertEqual(
            base.carver_owner_zones,
            [],
            "真实 boundary weight 全零时不得取得 carver 控制权",
        )

    def test_positive_weight_zone_is_recorded_as_carver_owner(self) -> None:
        base = _buffer()
        overlay = _buffer()
        zone = SimpleNamespace(name="positive_owner")

        with (
            patch(
                "scripts.terrain_gen.stitcher._compute_boundary_weight_array",
                return_value=np.ones((TILE_SIZE, TILE_SIZE), dtype=np.float32),
            ),
            patch(
                "scripts.terrain_gen.stitcher._coherent_noise_2d_array",
                return_value=np.zeros((TILE_SIZE, TILE_SIZE), dtype=np.float32),
            ),
        ):
            _blend_tile_layers(base, overlay, zone)

        self.assertEqual(
            base.contributing_zones,
            ["positive_owner"],
            "正权重 zone 应保留为 provenance；实际列表必须只含 positive_owner",
        )
        self.assertEqual(
            base.carver_owner_zones,
            ["positive_owner"],
            "实际参与过 blend 的 zone 才能成为 carver owner",
        )


class TileCarverChainOwnershipTest(unittest.TestCase):
    def test_chain_uses_positive_owner_not_first_provenance_zone(self) -> None:
        provenance_chain = [object()]
        owner_chain = [object()]
        buffer = SimpleNamespace(
            contributing_zones=["provenance_only", "positive_owner"],
            carver_owner_zones=["positive_owner"],
        )

        resolved = _tile_carver_chain(
            buffer,
            {
                "provenance_only": provenance_chain,
                "positive_owner": owner_chain,
            },
        )

        self.assertIs(
            resolved,
            owner_chain,
            "export 不得让零权重 provenance zone 抢走 carver chain",
        )

    def test_no_positive_owner_means_no_carving(self) -> None:
        buffer = SimpleNamespace(
            contributing_zones=["provenance_only"],
            carver_owner_zones=[],
        )

        self.assertEqual(
            _tile_carver_chain(buffer, {"provenance_only": [object()]}),
            [],
            "owner 为空时不得回退到 provenance zone 雕刻整块 tile",
        )


class RealBlueprintCarverOwnerTest(unittest.TestCase):
    def test_tile_6_minus_7_keeps_blood_valley_as_provenance_only(self) -> None:
        blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        catalog = load_profile_catalog(DEFAULT_PROFILES_PATH)
        tile_size = 512
        tile = WorldTile(
            tile_x=6,
            tile_z=-7,
            min_x=6 * tile_size,
            max_x=7 * tile_size - 1,
            min_z=-7 * tile_size,
            max_z=-6 * tile_size - 1,
        )
        blood_valley = next(
            zone for zone in blueprint.zones if zone.name == "blood_valley"
        )
        wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
        blood_weight = _compute_boundary_weight_array(blood_valley, wx, wz)
        self.assertEqual(
            float(np.nanmax(blood_weight)),
            0.0,
            "真实 witness 前提漂移：tile_6_-7 上 blood_valley 权重应全零",
        )

        with TemporaryDirectory() as temp_dir:
            plan = build_generation_plan(
                blueprint,
                catalog,
                DEFAULT_BLUEPRINT_PATH,
                DEFAULT_PROFILES_PATH,
                Path(temp_dir),
                tile_size,
            )
            plan.tiles = [tile]
            buffer = synthesize_fields(plan).tiles[0]
            chain = _tile_carver_chain(buffer, _zone_carver_chains(plan))

        self.assertIn(
            "blood_valley",
            buffer.contributing_zones,
            "粗 AABB provenance 仍应记录 blood_valley，避免破坏 manifest/debug 兼容",
        )
        self.assertNotIn(
            "blood_valley",
            buffer.carver_owner_zones,
            "零权重 blood_valley 不得取得 tile_6_-7 的 carver ownership",
        )
        self.assertIn(
            "zhanhun_plain",
            buffer.carver_owner_zones,
            "真实正权重的战魂平野应保留为该 tile 的几何 owner",
        )
        self.assertEqual(
            chain,
            [],
            "战魂平野边缘没有正贡献 carver owner，chain 必须精确为空",
        )
        folded = spans_for_tile(buffer, suppress_fold_isle=False)
        carved = apply_carver_chain(
            folded,
            chain,
            origin_x=buffer.tile.min_x,
            origin_z=buffer.tile.min_z,
            tile_size=buffer.tile_size,
            seed=CARVE_SEED,
        )
        self.assertEqual(
            [column.spans for column in carved],
            [column.spans for column in folded],
            "空 owner chain 必须保证 tile_6_-7 的所有 spans 差异列为 0",
        )


if __name__ == "__main__":
    unittest.main()
