"""第六批果实 / 根资产的可观察契约测试。"""

from __future__ import annotations

import importlib
import json
import sys
import unittest
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
GEN = ROOT / "modelScript" / "generators"
sys.path.insert(0, str(GEN))

BATCH = {
    "an_shen_guo": ("AnShenGuo", "gen_an_shen_guo"),
    "wu_yan_guo": ("WuYanGuo", "gen_wu_yan_guo"),
    "food.spirit_fruit.ling_guo": ("LingGuo", "gen_ling_guo"),
    "tian_nu_jiao": ("TianNuJiao", "gen_tian_nu_jiao"),
    "shi_mai_gen": ("ShiMaiGen", "gen_shi_mai_gen"),
}


class PlantBatch6ContractTest(unittest.TestCase):
    def test_generators_are_clean_and_differentially_self_checked(self) -> None:
        """每株作者稿必须通过结构门，且每道门都能命中注入缺陷。"""

        for plant_id, (_, mod_name) in BATCH.items():
            module = importlib.import_module(mod_name)
            rig = module.build()
            self.assertEqual(module.GATES.report(rig), 0, plant_id)
            self.assertEqual(module.GATES.self_test(rig), 0, plant_id)

    def test_exported_geometry_and_texture_are_non_empty(self) -> None:
        """运行时 geo 与 PNG 必须是同一株的非空资源。"""

        for plant_id, (model_name, _) in BATCH.items():
            geometry_path = ROOT / "client/src/main/resources/assets/bong/geo/plants" / f"{plant_id}.geo.json"
            texture_path = ROOT / "client/src/main/resources/assets/bong/textures/entity/plants" / f"{plant_id}.png"
            geometry = json.loads(geometry_path.read_text(encoding="utf-8"))
            model = geometry["minecraft:geometry"][0]
            self.assertEqual(model["description"]["identifier"], f"geometry.bong.{plant_id}")
            self.assertGreater(
                sum(len(bone.get("cubes", [])) for bone in model["bones"]),
                0,
                f"{model_name} 导出后不能是空几何",
            )
            with Image.open(texture_path) as texture:
                self.assertEqual(texture.size, (64, 64), plant_id)

    def test_catalog_wires_only_mature_stage_to_geo(self) -> None:
        """成熟阶段走本批 geo，幼苗和生长阶段保持 billboard。"""

        catalog = json.loads((ROOT / "shared/botany/plants.json").read_text(encoding="utf-8"))
        plants = {plant["id"]: plant for plant in catalog["plants"]}
        for plant_id in BATCH:
            stages = plants[plant_id]["visual"]["stages"]
            mature = stages["mature"]
            self.assertEqual(mature["kind"], "geo", plant_id)
            self.assertTrue(mature["geometry"].endswith(f"{plant_id}.geo.json"), plant_id)
            self.assertEqual(mature["offset"], [0.0, 0.0, 0.0], plant_id)
            for stage in ("growing", "seedling"):
                if stage in stages:
                    self.assertEqual(stages[stage]["kind"], "billboard", f"{plant_id}:{stage}")


if __name__ == "__main__":
    unittest.main()
