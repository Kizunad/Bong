"""暴龙王装饰不能损坏原作者资产，重生成不能覆盖手工修改。"""

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from modelScript.creatures.baolongwang import gates
from modelScript.creatures.baolongwang import gen_decorations as gen


class BaolongwangDecorationsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.built = gen.build(3)

    def test_additions_preserve_original_asset_and_bind_to_body(self):
        for result in gates.GATES.run_all(self.built.doc):
            with self.subTest(contract=result.key):
                self.assertTrue(result.ok, result.violations)

    def test_gates_detect_real_asset_corruption(self):
        self.assertEqual(gates.GATES.self_test(self.built.doc), 0,
                         "缺陷注入后必须检出原几何、动画、UV、绑定、尺寸或嵌入错误")

    def test_regeneration_refuses_to_overwrite_artist_edits(self):
        for artifact in ("model", "texture"):
            with self.subTest(artifact=artifact), tempfile.TemporaryDirectory() as temp:
                destination = Path(temp) / "artist.bbmodel"
                texture_path = destination.with_suffix(".png")
                gen.write_model(self.built, destination)
                initial = (destination.read_bytes(), texture_path.read_bytes())
                gen.write_model(gen.build(3), destination)
                self.assertEqual(
                    (destination.read_bytes(), texture_path.read_bytes()), initial,
                    "相同输入应生成相同资产",
                )
                if artifact == "model":
                    edited = copy.deepcopy(self.built.doc)
                    edited["elements"][-1]["from"][0] += .25
                    destination.write_text(json.dumps(edited))
                else:
                    edited = self.built.atlas.copy()
                    edited.putpixel((0, 0), (255, 0, 255, 255))
                    edited.save(texture_path)
                artist_bytes = (destination.read_bytes(), texture_path.read_bytes())
                with self.assertRaises(FileExistsError):
                    gen.write_model(self.built, destination)
                self.assertEqual(
                    (destination.read_bytes(), texture_path.read_bytes()), artist_bytes,
                    "手工修改的模型与贴图均不得被重生成覆盖",
                )


if __name__ == "__main__":
    unittest.main()
