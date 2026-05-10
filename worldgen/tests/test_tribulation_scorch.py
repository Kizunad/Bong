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
    load_blueprint,
    load_profile_catalog,
)
from scripts.terrain_gen.bakers.raster_export import build_raster_bake_plan, export_rasters
from scripts.terrain_gen.fields import Bounds2D, SurfacePalette, WorldTile
from scripts.terrain_gen.harness.raster_check import validate_rasters
from scripts.terrain_gen.profiles import get_profile_generator
from scripts.terrain_gen.profiles.tribulation_scorch import (
    ANOMALY_KIND_CURSED_ECHO,
    MINERAL_KIND_LODESTONE,
    TRIBULATION_SCORCH_DECORATIONS,
    fill_tribulation_scorch_tile,
)
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields


class TribulationScorchProfileTest(unittest.TestCase):
    def build_zone(self, with_pit: bool = True) -> BlueprintZone:
        extras = {"ascension_pit_xz": [0.0, 0.0], "ascension_pit_radius": 24.0} if with_pit else {}
        return BlueprintZone(
            name="north_waste_east_scorch" if with_pit else "drift_scorch_001",
            display_name="北荒东陲焦土",
            bounds_xz=Bounds2D(min_x=-150, max_x=149, min_z=-150, max_z=149),
            center_xz=(0, 0),
            size_xz=(300, 300),
            spirit_qi=0.28,
            danger_level=7,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="tribulation_scorch",
                shape="irregular_blob",
                boundary=BoundarySpec(mode="hard", width=80),
                height_model={"base": [70, 88], "peak": 96},
                surface_palette=("coarse_dirt", "gravel", "sand", "blackstone", "basalt", "glass"),
                landmarks=("charred_husk_tree", "tianjie_ascension_pit"),
                extras=extras,
            ),
        )

    def catalog(self) -> TerrainProfileCatalog:
        return TerrainProfileCatalog(
            version=1,
            profiles={
                "tribulation_scorch": TerrainProfileSpec(
                    name="tribulation_scorch",
                    boundary=BoundarySpec(mode="hard", width=80),
                    height={"base": [70, 88], "peak": 96},
                    surface=("coarse_dirt", "gravel", "sand", "blackstone", "basalt", "glass"),
                    water={"level": "very_low", "coverage": 0.01},
                    passability="high",
                    extras={"ambient_hint": {"thunder_feel": "frequent"}},
                )
            },
        )

    def test_profile_generator_is_registered_with_expected_ecology(self) -> None:
        generator = get_profile_generator("tribulation_scorch")

        self.assertEqual(generator.__class__.__name__, "TribulationScorchGenerator")
        self.assertEqual(len(TRIBULATION_SCORCH_DECORATIONS), 7)
        self.assertEqual(
            [deco.name for deco in TRIBULATION_SCORCH_DECORATIONS],
            [
                "glass_fulgurite",
                "charred_husk_tree",
                "lightning_basalt_pit",
                "lodestone_vortex",
                "iron_lattice_slag",
                "blue_lightning_glass",
                "magnetized_copper_slag",
            ],
        )
        self.assertIn("mineral_density", generator.extra_layers)
        self.assertIn("anomaly_kind", generator.extra_layers)
        self.assertIn("static_crackle", generator.ecology.ambient_effects)

    def test_fill_tile_pins_ascension_pit_and_surface_minerals(self) -> None:
        zone = self.build_zone(with_pit=True)
        tile = WorldTile(tile_x=0, tile_z=0, min_x=-150, max_x=149, min_z=-150, max_z=149)

        buffer = fill_tribulation_scorch_tile(zone, tile, 300, SurfacePalette())
        center_idx = (150 * 300) + 150

        self.assertEqual(buffer.layers["qi_density"][center_idx], 0.0)
        self.assertGreaterEqual(buffer.layers["anomaly_intensity"][center_idx], 0.6)
        self.assertEqual(buffer.layers["anomaly_kind"][center_idx], ANOMALY_KIND_CURSED_ECHO)
        self.assertEqual(buffer.layers["mineral_kind"][center_idx], MINERAL_KIND_LODESTONE)
        self.assertGreater(buffer.layers["mineral_density"].max(), 0.35)

        variants = set(int(value) for value in buffer.layers["flora_variant_id"] if value > 0)
        self.assertEqual(variants, {1, 2, 3, 4, 5, 6, 7})

    def test_synthesize_and_export_manifest_include_ascension_pit_once(self) -> None:
        zones = (self.build_zone(with_pit=True), self.build_zone(with_pit=False))
        blueprint = WorldBlueprint(
            version=1,
            world_name="test_world",
            spawn_zone=zones[0].name,
            bounds_xz=Bounds2D(min_x=-200, max_x=200, min_z=-200, max_z=200),
            notes=(),
            zones=zones,
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)
            plan = build_generation_plan(
                blueprint=blueprint,
                profile_catalog=self.catalog(),
                blueprint_path=Path("blueprint.json"),
                profiles_path=Path("profiles.json"),
                output_dir=output_dir,
                tile_size=300,
            )
            plan.bake_plan = build_raster_bake_plan(plan, output_dir)
            fields = synthesize_fields(plan)
            artifacts = export_rasters(plan, fields)

            self.assertIn("mineral_density", fields.layers)
            self.assertIn("mineral_kind", fields.layers)
            manifest = json.loads(artifacts["manifest"].read_text(encoding="utf-8"))
            self.assertEqual(len(manifest["ascension_pits"]), 1)
            self.assertEqual(manifest["ascension_pits"][0]["zone"], "north_waste_east_scorch")
            self.assertEqual(
                manifest["ascension_pits"][0]["loot"][0]["template_id"],
                "xujie_canxie",
            )
            ok, message = validate_rasters(artifacts["raster_dir"])
            self.assertTrue(ok, message)

    def test_repo_blueprint_contains_three_scorch_zones(self) -> None:
        root = Path(__file__).resolve().parents[2]
        blueprint = load_blueprint(root / "server" / "zones.worldview.example.json")
        catalog = load_profile_catalog(root / "worldgen" / "terrain-profiles.example.json")

        scorch_zones = [
            zone for zone in blueprint.zones if zone.worldgen.terrain_profile == "tribulation_scorch"
        ]
        self.assertEqual(
            [zone.name for zone in scorch_zones],
            ["blood_valley_east_scorch", "north_waste_east_scorch", "drift_scorch_001"],
        )
        self.assertIn("tribulation_scorch", catalog.profiles)
        pits = [
            zone
            for zone in scorch_zones
            if "ascension_pit_xz" in zone.worldgen.extras
        ]
        self.assertEqual([zone.name for zone in pits], ["north_waste_east_scorch"])


if __name__ == "__main__":
    unittest.main()
