"""第一批苔藓 / 藻资产的可观察契约测试。"""

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
    "hui_jin_tai": "HuiJinTai",
    "lie_yuan_tai": "LieYuanTai",
    "shi_ling_xian": "ShiLingXian",
    "xuan_rong_tai": "XuanRongTai",
    "yang_jing_tai": "YangJingTai",
    "jing_xin_zao": "JingXinZao",
}


class PlantBatch1ContractTest(unittest.TestCase):
    def test_generators_are_clean_and_differentially_self_checked(self) -> None:
        """每株作者稿必须通过结构门，且门的注入器不能假绿。"""

        for plant_id in BATCH:
            module = importlib.import_module(f"gen_{plant_id}")
            rig = module.build()
            self.assertEqual(
                module.GATES.report(rig),
                0,
                f"{plant_id} 的作者稿存在结构违例，不能进入 Round 2",
            )
            self.assertEqual(
                module.GATES.self_test(rig),
                0,
                f"{plant_id} 的差分门禁无法命中注入缺陷",
            )

    def test_exported_geometry_and_texture_are_non_empty(self) -> None:
        """运行时 geo 与 PNG 必须是同一株的非空资源。"""

        for plant_id, model_name in BATCH.items():
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
                self.assertEqual(texture.size, (64, 64), f"{plant_id} 的作者图集必须是 64×64")

    def test_catalog_wires_only_mature_stage_to_geo(self) -> None:
        """成熟阶段走 geo，幼苗和生长阶段仍保持 billboard。"""

        catalog = json.loads((ROOT / "shared/botany/plants.json").read_text(encoding="utf-8"))
        plants = {plant["id"]: plant for plant in catalog["plants"]}
        for plant_id in BATCH:
            stages = plants[plant_id]["visual"]["stages"]
            self.assertEqual(stages["mature"]["kind"], "geo", plant_id)
            if "growing" in stages:
                self.assertEqual(stages["growing"]["kind"], "billboard", plant_id)
            if "seedling" in stages:
                self.assertEqual(stages["seedling"]["kind"], "billboard", plant_id)
            self.assertTrue(stages["mature"]["geometry"].endswith(f"{plant_id}.geo.json"))
            self.assertEqual(stages["mature"]["offset"], [0.0, 0.0, 0.0], plant_id)


if __name__ == "__main__":
    unittest.main()
