"""item bbmodel 导出器的黑盒回归测试。

三套 #2301 已验收资产是字节级 oracle：如果 OBJ/MTL/model JSON/atlas 任一字节
漂移，说明导出器和历史产物的转换契约不一致，不能把主线产物改成迁就新工具。
丹药只检查映射、四类文件和内嵌 atlas 原字节，避免测试偷偷依赖运行时注册表。
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
sys.path.insert(0, str(LIB_DIR / "exporters"))

import export_item_assets as export  # noqa: E402


REFERENCE_ASSETS = {
    "bamboo_jian": "BambooJianSingle.bbmodel",
    "beast_spine_sword": "BeastSpineSword.bbmodel",
    "herb_sickle": "HerbSickle.bbmodel",
}


class ItemAssetExporterTest(unittest.TestCase):
    def test_empty_or_missing_elements_are_rejected_at_export_boundary(self) -> None:
        source = export.MODELS / "BambooJianSingle.bbmodel"
        original = json.loads(source.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory(prefix="bong-invalid-item-export-") as temp:
            temp_root = Path(temp)
            for case_name, elements in (("empty", []), ("missing", None)):
                malformed = dict(original)
                if elements is None:
                    malformed.pop("elements", None)
                else:
                    malformed["elements"] = elements
                malformed_path = temp_root / f"{case_name}.bbmodel"
                malformed_path.write_text(
                    json.dumps(malformed),
                    encoding="utf-8",
                )
                with self.subTest(case=case_name), self.assertRaisesRegex(
                    ValueError, "elements"
                ):
                    export.export_asset(
                        malformed_path,
                        f"invalid_{case_name}",
                        output_root=temp_root / "output",
                    )

    def test_replays_all_accepted_item_assets_byte_for_byte(self) -> None:
        committed = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong"
        with tempfile.TemporaryDirectory(prefix="bong-item-export-") as temp:
            output_root = Path(temp)
            for identifier, filename in REFERENCE_ASSETS.items():
                export.export_asset(
                    export.MODELS / filename,
                    identifier,
                    output_root=output_root,
                )
                for relative in (
                    Path("models/item") / identifier / f"{identifier}.json",
                    Path("models/item") / identifier / f"{identifier}.mtl",
                    Path("models/item") / identifier / f"{identifier}.obj",
                    Path("textures/item") / identifier / "atlas.png",
                ):
                    expected = committed / relative
                    actual = output_root / relative
                    self.assertEqual(
                        expected.read_bytes(),
                        actual.read_bytes(),
                        f"{identifier}/{relative.name} 必须逐字节复现已验收产物",
                    )

    def test_single_edged_axes_turn_blade_side_to_the_opposite_face_in_hand(self) -> None:
        """v2 斧的斧刃建在 -X 侧；标准手持 display 下它朝上（用户判「上下反了」）。

        契约：导出的手持 display 等于标准 display 再绕握柄轴（模型 Y）转半圈，
        于是 -X 侧在手里落到与标准 display 相反的一面；几何不动。
        """

        import numpy as np

        def rotation(degrees: list[float]) -> np.ndarray:
            x, y, z = np.radians(degrees)
            rx = np.array([[1, 0, 0], [0, np.cos(x), -np.sin(x)], [0, np.sin(x), np.cos(x)]])
            ry = np.array([[np.cos(y), 0, np.sin(y)], [0, 1, 0], [-np.sin(y), 0, np.cos(y)]])
            rz = np.array([[np.cos(z), -np.sin(z), 0], [np.sin(z), np.cos(z), 0], [0, 0, 1]])
            return rx @ ry @ rz  # MC: Quaternionf.rotationXYZ

        standard = export.build_display(export.ExportOptions(offset=(0.0, 0.0, 0.0)))
        blade_side = np.array([-1.0, 0.0, 0.0])
        committed = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "models" / "item"
        for identifier in export.HAFT_TURN_DEG:
            display = json.loads((committed / identifier / f"{identifier}.json").read_text(encoding="utf-8"))["display"]
            for mode in ("thirdperson_righthand", "thirdperson_lefthand",
                         "firstperson_righthand", "firstperson_lefthand"):
                with self.subTest(item=identifier, mode=mode):
                    turned = rotation(display[mode]["rotation"]) @ blade_side
                    plain = rotation(standard[mode]["rotation"]) @ blade_side
                    np.testing.assert_allclose(
                        turned, -plain, atol=1e-9,
                        err_msg=f"{identifier} {mode}: 斧刃侧应落到标准 display 的反面（刃口朝下）",
                    )

    def test_pill_mapping_names_are_explicit_and_assets_are_complete(self) -> None:
        self.assertEqual(8, len(export.PILL_ASSETS))
        for identifier, filename in export.PILL_ASSETS.items():
            source = export.MODELS / filename
            self.assertTrue(source.is_file(), f"丹药源模型缺失: {filename}")
            with tempfile.TemporaryDirectory(prefix="bong-pill-export-") as temp:
                output_root = Path(temp)
                paths = export.export_asset(source, identifier, output_root=output_root)
                self.assertEqual(
                    {"json", "mtl", "obj", "atlas"},
                    set(paths),
                    f"{identifier} 应落下四类 item 资产",
                )
                self.assertTrue(all(path.is_file() for path in paths.values()))
                self.assertIn(
                    f"usemtl {identifier}_atlas",
                    paths["obj"].read_text(encoding="utf-8"),
                )
                self.assertIn(
                    f"newmtl {identifier}_atlas",
                    paths["mtl"].read_text(encoding="utf-8"),
                )


if __name__ == "__main__":
    unittest.main()
