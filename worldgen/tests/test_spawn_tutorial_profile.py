from __future__ import annotations

import unittest

import numpy as np

from scripts.terrain_gen.blueprint import (
    BoundarySpec,
    BlueprintZone,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile
from scripts.terrain_gen.profiles.spawn_plain import (
    dynamic_lingquan_selector,
    fill_spawn_plain_tile,
    spawn_tutorial_pois_for_zone,
)


class SpawnTutorialProfileTest(unittest.TestCase):
    def build_zone(self) -> BlueprintZone:
        return BlueprintZone(
            name="spawn",
            display_name="初醒原",
            bounds_xz=Bounds2D(min_x=-150, max_x=149, min_z=-150, max_z=149),
            center_xz=(0, 0),
            size_xz=(300, 300),
            spirit_qi=0.30,
            danger_level=1,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="spawn_plain",
                shape="ellipse",
                boundary=BoundarySpec(mode="soft", width=32),
                height_model={"base": [66, 78], "peak": 84},
                surface_palette=("grass_block", "dirt", "coarse_dirt", "gravel"),
            ),
        )

    def test_dynamic_lingquan_selector_prefers_high_qi_cells(self) -> None:
        coords = np.array([-100.0, -50.0, 0.0, 50.0, 100.0])
        wx, wz = np.meshgrid(coords, coords)
        qi = np.zeros((5, 5), dtype=np.float64)
        height = np.full((5, 5), 70.0, dtype=np.float64)
        qi[2, 3] = 0.70
        qi[0, 2] = 0.60

        selected = dynamic_lingquan_selector((0.0, 0.0), qi, height, wx, wz)

        self.assertEqual(selected[0], (50.0, 71.0, 0.0))
        self.assertEqual(selected[1], (0.0, 71.0, -100.0))

    def test_fill_spawn_plain_tile_bumps_fallback_lingquans_to_high_qi(self) -> None:
        zone = self.build_zone()
        tile = WorldTile(
            tile_x=0,
            tile_z=0,
            min_x=-150,
            max_x=149,
            min_z=-150,
            max_z=149,
        )
        buffer = fill_spawn_plain_tile(zone, tile, 300, SurfacePalette())

        first_idx = ((100 - tile.min_z) * 300) + (50 - tile.min_x)
        second_idx = ((-80 - tile.min_z) * 300) + (-30 - tile.min_x)

        self.assertGreaterEqual(buffer.layers["qi_density"][first_idx], 0.5)
        self.assertGreaterEqual(buffer.layers["qi_density"][second_idx], 0.5)

    def test_spawn_tutorial_pois_include_coffin_lingquans_and_kaimai_chest(self) -> None:
        kinds = {poi.kind: poi for poi in spawn_tutorial_pois_for_zone(self.build_zone())}

        self.assertIn("spawn_tutorial_coffin", kinds)
        self.assertIn("tutorial_chest", kinds)
        self.assertIn("tutorial_rogue_anchor", kinds)
        self.assertIn("tutorial_rat_path", kinds)
        lingquans = [
            poi
            for poi in spawn_tutorial_pois_for_zone(self.build_zone())
            if poi.kind == "tutorial_lingquan"
        ]
        self.assertEqual(len(lingquans), 2)
        self.assertIn("loot:kaimai_dan", kinds["tutorial_chest"].tags)


if __name__ == "__main__":
    unittest.main()
