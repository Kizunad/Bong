from __future__ import annotations

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
from scripts.terrain_gen.fields import Bounds2D, LAYER_REGISTRY, SurfacePalette, WorldTile
from scripts.terrain_gen.profiles import get_profile_generator
from scripts.terrain_gen.profiles.jiu_zong_ruin import (
    COMMON_DECORATION_COUNT,
    JIU_ZONG_ORIGIN_SPECIFIC,
    JIU_ZONG_RUIN_DECORATIONS_COMMON,
    fill_jiu_zong_ruin_tile,
)
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields


class JiuzongRuinProfileTest(unittest.TestCase):
    def build_zone(self, origin_id: int = 1) -> BlueprintZone:
        return BlueprintZone(
            name=f"jiuzong_origin_{origin_id}",
            display_name="九宗故地",
            bounds_xz=Bounds2D(min_x=-400, max_x=399, min_z=-400, max_z=399),
            center_xz=(0, 0),
            size_xz=(800, 800),
            spirit_qi=0.40,
            danger_level=6,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="jiu_zong_ruin",
                shape="irregular_blob",
                boundary=BoundarySpec(mode="semi_hard", width=96),
                height_model={"base": [72, 90], "peak": 100},
                surface_palette=(
                    "mossy_cobblestone",
                    "stone_bricks",
                    "cracked_stone_bricks",
                    "coarse_dirt",
                    "gravel",
                ),
                landmarks=("formation_core_stub", "forgotten_stele_garden"),
                extras={"zongmen_origin_id": origin_id},
            ),
        )

    def test_profile_generator_registers_origin_layer_and_decorations(self) -> None:
        generator = get_profile_generator("jiu_zong_ruin")

        self.assertEqual(generator.__class__.__name__, "JiuzongRuinGenerator")
        self.assertIn("zongmen_origin_id", LAYER_REGISTRY)
        self.assertIn("zongmen_origin_id", generator.extra_layers)
        self.assertEqual(len(JIU_ZONG_RUIN_DECORATIONS_COMMON), 5)
        self.assertEqual(len(JIU_ZONG_ORIGIN_SPECIFIC), 7)
        self.assertEqual(JIU_ZONG_ORIGIN_SPECIFIC[1].name, "bloodstream_altar")

    def test_fill_tile_pins_origin_qi_turbulence_and_origin_decoration(self) -> None:
        zone = self.build_zone(origin_id=1)
        tile = WorldTile(tile_x=0, tile_z=0, min_x=-400, max_x=399, min_z=-400, max_z=399)

        buffer = fill_jiu_zong_ruin_tile(zone, tile, 800, SurfacePalette())

        origin_values = set(int(value) for value in buffer.layers["zongmen_origin_id"])
        self.assertEqual(origin_values, {1})
        self.assertGreaterEqual(buffer.layers["qi_density"].min(), 0.10)
        self.assertLessEqual(buffer.layers["qi_density"].max(), 0.70)
        self.assertGreater(buffer.layers["ruin_density"].max(), 0.6)
        self.assertIn(5, set(int(value) for value in buffer.layers["anomaly_kind"]))
        self.assertIn(
            COMMON_DECORATION_COUNT + 1,
            set(int(value) for value in buffer.layers["flora_variant_id"]),
        )

    def test_synthesize_fields_exports_zongmen_origin_id(self) -> None:
        zone = self.build_zone(origin_id=7)
        profile_catalog = TerrainProfileCatalog(
            version=1,
            profiles={
                "jiu_zong_ruin": TerrainProfileSpec(
                    name="jiu_zong_ruin",
                    boundary=BoundarySpec(mode="semi_hard", width=96),
                    height={"base": [72, 90], "peak": 100},
                    surface=(
                        "mossy_cobblestone",
                        "stone_bricks",
                        "cracked_stone_bricks",
                        "coarse_dirt",
                        "gravel",
                    ),
                    water={"level": "very_low", "coverage": 0.03},
                    passability="medium",
                    extras={"origin_field": "zongmen_origin_id"},
                )
            },
        )
        blueprint = WorldBlueprint(
            version=1,
            world_name="test_world",
            spawn_zone=zone.name,
            bounds_xz=zone.bounds_xz,
            notes=(),
            zones=(zone,),
        )

        plan = build_generation_plan(
            blueprint=blueprint,
            profile_catalog=profile_catalog,
            blueprint_path=Path("blueprint.json"),
            profiles_path=Path("profiles.json"),
            output_dir=Path("."),
            tile_size=800,
        )
        fields = synthesize_fields(plan)

        self.assertIn("zongmen_origin_id", fields.layers)
        self.assertEqual(fields.tiles[0].layers["zongmen_origin_id"].max(), 7)


if __name__ == "__main__":
    unittest.main()
