"""实体植物的材质面不能因图集分辨率变更而采样到透明空白。"""

import base64
import importlib
import io
import json
from pathlib import Path
import sys
import unittest

from PIL import Image

MODEL_SCRIPT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(MODEL_SCRIPT / "generators"))

SOLID_PLANTS = {
    "LongLinTai": "gen_long_lin_tai",
    "TuiGuTeng": "gen_tui_gu_teng",
    "HeiGuJun": "gen_hei_gu_jun",
    "XuYuanRui": "gen_xu_yuan_rui",
}


class PlantTextureTest(unittest.TestCase):
    def test_solid_material_faces_have_opaque_texture_coverage(self):
        # 同时核对生成器和交付的作者稿，避免只修代码、漏更新内嵌 PNG。
        for name, module_name in SOLID_PLANTS.items():
            generated = importlib.import_module(module_name).build_bbmodel_dict()
            authored = json.loads((MODEL_SCRIPT / "models" / f"{name}.bbmodel").read_text())
            for source_name, model in (("generated", generated), ("authored", authored)):
                with self.subTest(plant=name, source=source_name):
                    png = base64.b64decode(model["textures"][0]["source"].split(",", 1)[1])
                    texture = Image.open(io.BytesIO(png)).convert("RGBA")
                    resolution = model["resolution"]
                    self.assertEqual(
                        (resolution["width"], resolution["height"]), texture.size,
                        "模型 UV 分辨率必须与内嵌 PNG 一致",
                    )
                    material_regions = {
                        tuple(face["uv"])
                        for element in model["elements"]
                        for face in element["faces"].values()
                        if face["texture"] is not None
                    }
                    for u0, v0, u1, v1 in sorted(material_regions):
                        region = (min(u0, u1), min(v0, v1), max(u0, u1), max(v0, v1))
                        alpha = texture.crop(region).getchannel("A")
                        self.assertEqual(
                            (255, 255), alpha.getextrema(),
                            f"{name} 材质区 {region} 采样到透明像素；检查绘图尺寸与 UV 缩放",
                        )


if __name__ == "__main__":
    unittest.main()
