"""Carver owner 与 manifest provenance 必须是两套独立语义。"""

from __future__ import annotations

import unittest
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np

from scripts.terrain_gen.bakers.raster_export import _tile_carver_chain
from scripts.terrain_gen.fields import TileFieldBuffer, WorldTile
from scripts.terrain_gen.stitcher import _blend_tile_layers


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

        self.assertEqual(base.contributing_zones, ["positive_owner"])
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


if __name__ == "__main__":
    unittest.main()
