from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.terrain_gen.blueprint import BlueprintZone, BoundarySpec, ZoneWorldgenConfig  # noqa: E402
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile  # noqa: E402
from scripts.terrain_gen.profiles.ash_dead_zone import (  # noqa: E402
    ASH_DEAD_ZONE_DECORATIONS,
    AshDeadZoneGenerator,
    fill_ash_dead_zone_tile,
)


def ash_zone() -> BlueprintZone:
    return BlueprintZone(
        name="south_ash_dead_zone",
        display_name="南荒余烬",
        bounds_xz=Bounds2D(min_x=-2200, max_x=-200, min_z=7000, max_z=9000),
        center_xz=(-1200, 8000),
        size_xz=(2000, 2000),
        spirit_qi=0.0,
        danger_level=5,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="ash_dead_zone",
            shape="irregular_blob",
            boundary=BoundarySpec(mode="hard", width=64),
            height_model={"base": [70, 82], "peak": 88},
            surface_palette=("coarse_dirt", "gravel", "sand", "smooth_stone", "stone"),
            landmarks=("silent_obelisk", "vanished_path_marker"),
        ),
    )


class AshDeadZoneProfileTests(unittest.TestCase):
    def test_ecology_declares_six_decorations_and_no_ambient_effects(self):
        names = [spec.name for spec in ASH_DEAD_ZONE_DECORATIONS]

        self.assertEqual(
            names,
            [
                "cantan_block_drift",
                "dried_corpse_mound",
                "petrified_tree_stump",
                "ash_spider_lair",
                "silent_obelisk",
                "vanished_path_marker",
            ],
        )
        self.assertEqual(AshDeadZoneGenerator.ecology.ambient_effects, ())

    def test_center_tile_is_true_zero_qi_and_no_vein_flow(self):
        zone = ash_zone()
        tile = WorldTile(tile_x=-6, tile_z=31, min_x=-1456, max_x=-1201, min_z=7936, max_z=8191)
        buffer = fill_ash_dead_zone_tile(zone, tile, 256, SurfacePalette())

        self.assertLessEqual(buffer.layers["qi_density"].max(), 0.001)
        self.assertEqual(buffer.layers["qi_vein_flow"].max(), 0.0)
        self.assertGreaterEqual(buffer.layers["mofa_decay"].min(), 0.95)

    def test_profile_hits_all_six_local_flora_variants_across_zone(self):
        zone = ash_zone()
        variants: set[int] = set()
        palette = SurfacePalette()
        tile_size = 256
        for min_x in range(zone.bounds_xz.min_x, zone.bounds_xz.max_x + 1, tile_size):
            for min_z in range(zone.bounds_xz.min_z, zone.bounds_xz.max_z + 1, tile_size):
                tile = WorldTile(
                    tile_x=min_x // tile_size,
                    tile_z=min_z // tile_size,
                    min_x=min_x,
                    max_x=min_x + tile_size - 1,
                    min_z=min_z,
                    max_z=min_z + tile_size - 1,
                )
                buffer = fill_ash_dead_zone_tile(zone, tile, tile_size, palette)
                variants.update(int(value) for value in set(buffer.layers["flora_variant_id"].tolist()) if value)

        self.assertEqual(variants, {1, 2, 3, 4, 5, 6})


if __name__ == "__main__":
    unittest.main()
