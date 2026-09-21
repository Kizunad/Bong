"""生物导出的格式契约：坐标、骨绑定、图集完整性和安装前整批校验。"""
import base64
import copy
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

EXPORTERS = Path(__file__).resolve().parents[1] / "exporters"
sys.path.insert(0, str(EXPORTERS))
import export_creature_assets as exporter
from import_creature_geo import import_geo


def fixture():
    png = io.BytesIO()
    Image.new("RGBA", (16, 16), (100, 70, 60, 255)).save(png, format="PNG")
    return {
        "meta": {"box_uv": False}, "resolution": {"width": 16, "height": 16},
        "elements": [{"uuid": "cube", "name": "body", "type": "cube",
                      "from": [2, 3, 4], "to": [6, 8, 10], "origin": [3, 4, 5],
                      "rotation": [10, 20, 30],
                      "faces": {"north": {"texture": 0, "uv": [1, 2, 5, 7]},
                                "up": {"texture": 0, "uv": [3, 4, 7, 10]}}}],
        "groups": [{"uuid": "root", "name": "root", "origin": [1, 2, 3]},
                   {"uuid": "child", "name": "jaw", "origin": [2, 4, 6], "rotation": [4, 5, 6]}],
        "outliner": [{"uuid": "root", "children": [{"uuid": "child", "children": ["cube"]}]}],
        "textures": [{"source": "data:image/png;base64," + base64.b64encode(png.getvalue()).decode()}],
        "animations": [{"name": "idle", "loop": "loop", "length": 1,
                        "animators": {"child": {"type": "bone", "keyframes": [
                            {"channel": channel, "time": time, "interpolation": "linear",
                             "data_points": [{"x": "10", "y": "20", "z": "30"}]}
                            for channel in ("rotation", "position", "scale") for time in (0, 1)]}}}],
    }


def convert(doc, check=True):
    script = "const {convert}=require(process.argv[1]);process.stdout.write(JSON.stringify(convert(JSON.parse(require('fs').readFileSync(0,'utf8')),'sample')));"
    return subprocess.run(["node", "-e", script, str(EXPORTERS / "creature_codec.cjs")],
                          input=json.dumps(doc), text=True, capture_output=True, check=check)


class CreatureCodecTest(unittest.TestCase):
    def test_format5_nested_groups_uv_and_animation_axes(self):
        result = json.loads(convert(fixture()).stdout)
        bones = result["geometry"]["minecraft:geometry"][0]["bones"]
        self.assertEqual(["root", "jaw"], [bone["name"] for bone in bones])
        self.assertEqual("root", bones[1]["parent"])
        self.assertEqual([-2, 4, 6], bones[1]["pivot"])
        cube = bones[1]["cubes"][0]
        self.assertEqual([-6, 3, 4], cube["origin"])
        self.assertEqual([4, 5, 6], cube["size"])
        self.assertEqual([-10, -20, 30], cube["rotation"])
        self.assertEqual({"uv": [7, 10], "uv_size": [-4, -6]}, cube["uv"]["up"])
        tracks = result["animation"]["animations"]["animation.bong.sample.idle"]["bones"]["jaw"]
        self.assertEqual([-10, -20, 30], tracks["rotation"]["0"])
        self.assertEqual([-10, 20, 30], tracks["position"]["0"])
        self.assertEqual([10, 20, 30], tracks["scale"]["0"])
        # 导入后再导出，应保持静态几何的可观察坐标/UV，不能重复翻轴。
        original = result["geometry"]
        imported = import_geo(original, b"unused", "sample")
        self.assertEqual(original, json.loads(convert(imported).stdout)["geometry"])

    def test_rejects_unbound_geometry_and_unbaked_motion(self):
        original = fixture()
        variants = []
        unbound = copy.deepcopy(original)
        unbound["outliner"][0]["children"][0]["children"] = []
        variants.append(unbound)
        duplicate = copy.deepcopy(original)
        duplicate["groups"][1]["name"] = "root"
        variants.append(duplicate)
        molang = copy.deepcopy(original)
        molang["animations"][0]["animators"]["child"]["keyframes"][0]["data_points"][0]["x"] = "query.anim_time"
        variants.append(molang)
        for doc in variants:
            with self.subTest(doc=doc):
                self.assertNotEqual(0, convert(doc, check=False).returncode, "不支持的数据不能静默丢失")

    def test_failed_batch_validation_preserves_installed_assets(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "models").mkdir()
            stage = root / "stage"
            stage.mkdir()
            client = root / "client"
            target = client / "geo" / "first.geo.json"
            target.parent.mkdir(parents=True)
            target.write_text("previous model")
            assets = [exporter.Asset("sample", name, f"{name}.bbmodel", "idle") for name in ("first", "second")]
            with patch.object(exporter, "ROOT", root), patch.object(exporter, "MODELS", root / "models"):
                for asset in assets:
                    asset.source.write_text(json.dumps(fixture()))
                    exporter.export(asset, stage, offline=True)
                corrupted = stage / "second.animation.json"
                bad = json.loads(corrupted.read_text())
                bad["animations"]["animation.bong.second.idle"]["bones"]["missing_bone"] = {}
                corrupted.write_text(json.dumps(bad))
                with self.assertRaisesRegex(ValueError, "不存在的骨"):
                    exporter.install(assets, stage, client)
            self.assertEqual("previous model", target.read_text(), "后半批校验失败不能提前覆盖前半批")


if __name__ == "__main__":
    unittest.main()
