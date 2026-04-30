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
    ZoneOverlaySpec,
    ZoneWorldgenConfig,
    load_zone_overlays,
)
from scripts.terrain_gen.bakers.raster_export import (
    build_raster_bake_plan,
    export_rasters,
)
from scripts.terrain_gen.fields import Bounds2D
from scripts.terrain_gen.stitcher import build_generation_plan, synthesize_fields


class TerrainGenZoneOverlayTest(unittest.TestCase):
    def test_collapsed_overlay_exports_realm_collapse_mask(self) -> None:
        zone = BlueprintZone(
            name="spawn",
            display_name="初醒原",
            bounds_xz=Bounds2D(min_x=0, max_x=15, min_z=0, max_z=15),
            center_xz=(8, 8),
            size_xz=(16, 16),
            spirit_qi=0.3,
            danger_level=1,
            worldgen=ZoneWorldgenConfig(
                terrain_profile="spawn_plain",
                shape="ellipse",
                boundary=BoundarySpec(mode="soft", width=2),
                height_model={"base": [66, 78], "peak": 84},
                surface_palette=("grass_block", "dirt", "coarse_dirt", "gravel"),
            ),
        )
        blueprint = WorldBlueprint(
            version=1,
            world_name="test_world",
            spawn_zone="spawn",
            bounds_xz=Bounds2D(min_x=0, max_x=15, min_z=0, max_z=15),
            notes=(),
            zones=(zone,),
        )
        profile_catalog = TerrainProfileCatalog(
            version=1,
            profiles={
                "spawn_plain": TerrainProfileSpec(
                    name="spawn_plain",
                    boundary=BoundarySpec(mode="soft", width=2),
                    height={"base": [66, 78], "peak": 84},
                    surface=("grass_block", "dirt", "coarse_dirt", "gravel"),
                    water={"level": "low", "coverage": 0.05},
                    passability="high",
                )
            },
        )
        overlays = (
            ZoneOverlaySpec(
                zone_id="spawn",
                overlay_kind="collapsed",
                payload={
                    "zone_status": "collapsed",
                    "danger_level": 4,
                    "active_events": ["realm_collapse"],
                },
                payload_version=1,
                since_wall=123,
            ),
        )

        with tempfile.TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir)
            plan = build_generation_plan(
                blueprint=blueprint,
                profile_catalog=profile_catalog,
                blueprint_path=Path("blueprint.json"),
                profiles_path=Path("profiles.json"),
                output_dir=output_dir,
                tile_size=16,
                zone_overlays=overlays,
            )
            plan.bake_plan = build_raster_bake_plan(plan, output_dir)
            fields = synthesize_fields(plan)
            artifacts = export_rasters(plan, fields)

            self.assertIn("realm_collapse_mask", fields.layers)
            self.assertGreater(
                int(fields.tiles[0].layers["realm_collapse_mask"].max()), 0
            )
            manifest = json.loads(artifacts["manifest"].read_text(encoding="utf-8"))
            self.assertIn("realm_collapse_mask", manifest["semantic_layers"])
            self.assertEqual(
                manifest["collapsed_zones"],
                [
                    {
                        "zone_id": "spawn",
                        "zone_status": "collapsed",
                        "payload_version": 1,
                        "since_wall": 123,
                        "active_events": ["realm_collapse"],
                        "display_name": "初醒原",
                        "dimension": "overworld",
                        "bounds_xz": {
                            "min_x": 0,
                            "max_x": 15,
                            "min_z": 0,
                            "max_z": 15,
                        },
                    }
                ],
            )

    def test_load_zone_overlays_consumes_server_export_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            overlay_path = Path(temp_dir) / "zones-export.json"
            overlay_path.write_text(
                json.dumps(
                    {
                        "schema_version": 10,
                        "kind": "zones_export_v1",
                        "zones_runtime": [],
                        "zone_overlays": [
                            {
                                "zone_id": "spawn",
                                "overlay_kind": "collapsed",
                                "payload_json": json.dumps(
                                    {
                                        "zone_status": "collapsed",
                                        "active_events": ["realm_collapse"],
                                    }
                                ),
                                "payload_version": 1,
                                "since_wall": 456,
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            overlays = load_zone_overlays(overlay_path)

        self.assertEqual(len(overlays), 1)
        self.assertEqual(overlays[0].zone_id, "spawn")
        self.assertEqual(overlays[0].overlay_kind, "collapsed")
        self.assertEqual(overlays[0].payload["zone_status"], "collapsed")
        self.assertEqual(overlays[0].since_wall, 456)


if __name__ == "__main__":
    unittest.main()
