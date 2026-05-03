from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.blueprint import (
    BoundarySpec,
    BlueprintZone,
    TerrainProfileCatalog,
    TerrainProfileSpec,
    WorldBlueprint,
    ZoneWorldgenConfig,
)
from scripts.terrain_gen.bakers.raster_export import build_raster_bake_plan, export_rasters
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile
from scripts.terrain_gen.harness.raster_check import validate_rasters
from scripts.terrain_gen.profiles import get_profile_generator
from scripts.terrain_gen.profiles.abyssal_maze import fill_abyssal_maze_tile
from scripts.terrain_gen.profiles.cave_network import fill_cave_network_tile
from scripts.terrain_gen.profiles.rift_mouth_barrens import (
    RIFT_MOUTH_DECORATIONS,
    fill_rift_mouth_barrens_tile,
)
from scripts.terrain_gen.profiles.rift_valley import fill_rift_valley_tile
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields


class RiftMouthBarrensProfileTest(unittest.TestCase):
    def build_zone(self, profile: str = "rift_mouth_barrens") -> BlueprintZone:
        return BlueprintZone(
            name=f"{profile}_unit",
            display_name="Rift Mouth",
            bounds_xz=Bounds2D(min_x=-150, max_x=149, min_z=-150, max_z=149),
            center_xz=(0, 0),
            size_xz=(300, 300),
            spirit_qi=0.05,
            danger_level=5,
            worldgen=ZoneWorldgenConfig(
                terrain_profile=profile,
                shape="circular" if profile == "rift_mouth_barrens" else "subterranean_cluster",
                boundary=BoundarySpec(mode="hard", width=48),
                height_model={"base": [60, 80], "peak": 88},
                surface_palette=(
                    "blackstone",
                    "obsidian",
                    "tuff",
                    "coarse_dirt",
                    "packed_ice",
                ),
                extras={
                    "portal_anchor_xz": [0, 0],
                    "core_radius": 30,
                    "outer_radius": 150,
                    "tsy_zone_link": "tsy_daneng_01_shallow",
                },
            ),
        )

    def test_profile_generator_is_registered_with_expected_ecology(self) -> None:
        generator = get_profile_generator("rift_mouth_barrens")

        self.assertEqual(generator.__class__.__name__, "RiftMouthBarrensGenerator")
        self.assertEqual(
            [deco.name for deco in RIFT_MOUTH_DECORATIONS],
            [
                "charred_obelisk_shard",
                "frost_qi_cluster",
                "ganshi_drift",
                "fresh_collapse_rubble",
                "spacetime_scar",
                "dao_zhuang_corpse_pose",
                "cracked_floor_seam",
            ],
        )
        self.assertIn("portal_anchor_sdf", generator.extra_layers)
        self.assertIn("neg_pressure", generator.extra_layers)
        self.assertIn("anomaly_kind", generator.extra_layers)

    def test_fill_tile_pins_negative_pressure_and_decorations(self) -> None:
        zone = self.build_zone()
        tile = WorldTile(tile_x=0, tile_z=0, min_x=-150, max_x=149, min_z=-150, max_z=149)
        buffer = fill_rift_mouth_barrens_tile(zone, tile, 300, SurfacePalette())
        center_idx = (150 * 300) + 150

        self.assertLess(buffer.layers["portal_anchor_sdf"][center_idx], 1.0)
        self.assertEqual(buffer.layers["neg_pressure"][center_idx], 0.8)
        self.assertEqual(buffer.layers["qi_density"][center_idx], 0.0)
        self.assertEqual(buffer.layers["anomaly_kind"][center_idx], 1)

        variants = set(int(value) for value in buffer.layers["flora_variant_id"] if value > 0)
        self.assertEqual(variants, {1, 2, 3, 4, 5, 6, 7})

    def test_synthesize_and_export_manifest_include_portal_anchor_sdf(self) -> None:
        zone = self.build_zone()
        blueprint = WorldBlueprint(
            version=1,
            world_name="test_world",
            spawn_zone=zone.name,
            bounds_xz=zone.bounds_xz,
            notes=(),
            zones=(zone,),
        )
        catalog = TerrainProfileCatalog(
            version=1,
            profiles={
                "rift_mouth_barrens": TerrainProfileSpec(
                    name="rift_mouth_barrens",
                    boundary=BoundarySpec(mode="hard", width=48),
                    height={"base": [60, 80], "peak": 88},
                    surface=(
                        "blackstone",
                        "obsidian",
                        "tuff",
                        "coarse_dirt",
                        "packed_ice",
                    ),
                    water={"level": "none", "coverage": 0.0},
                    passability="medium",
                    extras={"core_radius": 30, "outer_radius": 150},
                )
            },
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)
            plan = build_generation_plan(
                blueprint=blueprint,
                profile_catalog=catalog,
                blueprint_path=Path("blueprint.json"),
                profiles_path=Path("profiles.json"),
                output_dir=output_dir,
                tile_size=300,
            )
            plan.bake_plan = build_raster_bake_plan(plan, output_dir)
            fields = synthesize_fields(plan)
            artifacts = export_rasters(plan, fields)

            self.assertIn("portal_anchor_sdf", fields.layers)
            manifest = json.loads(artifacts["manifest"].read_text(encoding="utf-8"))
            self.assertIn("portal_anchor_sdf", manifest["semantic_layers"])
            min_anchor_sdf = min(
                float(tile.layers["portal_anchor_sdf"].min()) for tile in fields.tiles
            )
            self.assertEqual(min_anchor_sdf, 0.0)
            ok, message = validate_rasters(artifacts["raster_dir"])
            self.assertTrue(ok, message)

    def test_portal_anchor_sdf_is_independent_from_rift_axis_sdf(self) -> None:
        tile = WorldTile(tile_x=0, tile_z=0, min_x=-150, max_x=149, min_z=-150, max_z=149)
        palette = SurfacePalette()

        rift_valley = self.build_zone("rift_valley")
        rift_valley_buffer = fill_rift_valley_tile(rift_valley, tile, 300, palette)
        self.assertIn("rift_axis_sdf", rift_valley_buffer.layers)
        self.assertNotIn("portal_anchor_sdf", rift_valley_buffer.layers)

        rift_mouth = self.build_zone()
        rift_mouth_buffer = fill_rift_mouth_barrens_tile(rift_mouth, tile, 300, palette)
        self.assertIn("portal_anchor_sdf", rift_mouth_buffer.layers)
        self.assertNotIn("rift_axis_sdf", rift_mouth_buffer.layers)

    def test_cave_and_abyssal_profiles_write_internal_rift_hotspots(self) -> None:
        tile = WorldTile(tile_x=0, tile_z=0, min_x=-150, max_x=149, min_z=-150, max_z=149)
        palette = SurfacePalette()

        cave = self.build_zone("cave_network")
        cave_buffer = fill_cave_network_tile(cave, tile, 300, palette)
        self.assertEqual(int(cave_buffer.layers["anomaly_kind"].max()), 1)
        self.assertGreater(float(cave_buffer.layers["neg_pressure"].max()), 0.0)
        self.assertLess(float(cave_buffer.layers["portal_anchor_sdf"].min()), 1.0)

        abyssal = self.build_zone("abyssal_maze")
        abyssal_buffer = fill_abyssal_maze_tile(abyssal, tile, 300, palette)
        center_idx = (150 * 300) + 150
        self.assertEqual(abyssal_buffer.layers["underground_tier"][center_idx], 3)
        self.assertLess(abyssal_buffer.layers["portal_anchor_sdf"][center_idx], 1.0)
        self.assertGreater(abyssal_buffer.layers["neg_pressure"][center_idx], 0.0)
        self.assertEqual(abyssal_buffer.layers["anomaly_kind"][center_idx], 1)


if __name__ == "__main__":
    unittest.main()
