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
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile
from scripts.terrain_gen.profiles import get_profile_generator
from scripts.terrain_gen.profiles.pseudo_vein_oasis import (
    PSEUDO_VEIN_DECORATIONS,
    fill_pseudo_vein_oasis_tile,
)
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields


class PseudoVeinOasisProfileTest(unittest.TestCase):
    def build_zone(self) -> BlueprintZone:
        return BlueprintZone(
            name="pseudo_vein_unit",
            display_name="伪灵脉",
            bounds_xz=Bounds2D(min_x=-150, max_x=149, min_z=-150, max_z=149),
            center_xz=(0, 0),
            size_xz=(300, 300),
            spirit_qi=0.60,
            danger_level=4,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="pseudo_vein_oasis",
                shape="circular",
                boundary=BoundarySpec(mode="soft", width=32),
                height_model={"base": [68, 76], "peak": 80},
                surface_palette=(
                    "grass_block",
                    "moss_block",
                    "flowering_azalea_leaves",
                    "warped_wart_block",
                ),
                landmarks=("phantom_qi_pillar", "tiandao_seal_stele"),
                extras={"core_radius": 60, "rim_radius": 120},
            ),
        )

    def test_profile_generator_is_registered_with_expected_ecology(self) -> None:
        generator = get_profile_generator("pseudo_vein_oasis")

        self.assertEqual(generator.__class__.__name__, "PseudoVeinOasisGenerator")
        self.assertEqual(len(PSEUDO_VEIN_DECORATIONS), 5)
        self.assertEqual(
            [deco.name for deco in PSEUDO_VEIN_DECORATIONS],
            [
                "false_spirit_lotus",
                "phantom_qi_pillar",
                "lush_grass_overlay",
                "tiandao_seal_stele",
                "false_vein_well",
            ],
        )
        self.assertIn("qi_density", generator.extra_layers)
        self.assertIn("neg_pressure", generator.extra_layers)
        self.assertIn("anomaly_kind", generator.extra_layers)

    def test_fill_pseudo_vein_oasis_tile_pins_qi_gradient_and_decorations(self) -> None:
        zone = self.build_zone()
        tile = WorldTile(
            tile_x=0,
            tile_z=0,
            min_x=-150,
            max_x=149,
            min_z=-150,
            max_z=149,
        )
        palette = SurfacePalette()

        buffer = fill_pseudo_vein_oasis_tile(zone, tile, 300, palette)
        center_idx = (150 * 300) + 150
        body_idx = (150 * 300) + 186
        edge_idx = (150 * 300) + 204
        hungry_idx = (150 * 300) + 240

        self.assertEqual(buffer.layers["qi_density"][center_idx], 0.8)
        self.assertEqual(buffer.layers["qi_density"][body_idx], 0.6)
        self.assertEqual(buffer.layers["qi_density"][edge_idx], 0.25)
        self.assertEqual(buffer.layers["qi_density"][hungry_idx], 0.08)
        self.assertEqual(buffer.layers["flora_density"][hungry_idx], 0.0)
        self.assertEqual(buffer.layers["anomaly_kind"][center_idx], 2)
        self.assertEqual(buffer.layers["neg_pressure"][center_idx], 0.0)

        variants = set(int(value) for value in buffer.layers["flora_variant_id"] if value > 0)
        self.assertEqual(variants, {1, 2, 3, 4, 5})

    def test_synthesize_fields_exports_pseudo_vein_layers(self) -> None:
        zone = self.build_zone()
        blueprint = WorldBlueprint(
            version=1,
            world_name="test_world",
            spawn_zone="pseudo_vein_unit",
            bounds_xz=zone.bounds_xz,
            notes=(),
            zones=(zone,),
        )
        profile_catalog = TerrainProfileCatalog(
            version=1,
            profiles={
                "pseudo_vein_oasis": TerrainProfileSpec(
                    name="pseudo_vein_oasis",
                    boundary=BoundarySpec(mode="soft", width=32),
                    height={"base": [68, 76], "peak": 80},
                    surface=(
                        "grass_block",
                        "moss_block",
                        "flowering_azalea_leaves",
                        "warped_wart_block",
                    ),
                    water={"level": "low", "coverage": 0.04},
                    passability="high",
                    extras={"core_radius": 60, "rim_radius": 120},
                )
            },
        )

        plan = build_generation_plan(
            blueprint=blueprint,
            profile_catalog=profile_catalog,
            blueprint_path=Path("blueprint.json"),
            profiles_path=Path("profiles.json"),
            output_dir=Path("."),
            tile_size=300,
        )
        fields = synthesize_fields(plan)

        self.assertIn("qi_density", fields.layers)
        self.assertIn("flora_variant_id", fields.layers)
        self.assertIn("anomaly_kind", fields.layers)
        self.assertGreater(fields.tiles[0].layers["qi_density"].max(), 0.55)


if __name__ == "__main__":
    unittest.main()
