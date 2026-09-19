"""item bbmodel 导出器的黑盒回归测试。

三套 #2301 已验收资产是字节级 oracle：如果 OBJ/MTL/model JSON/atlas 任一字节
漂移，说明导出器和历史产物的转换契约不一致，不能把主线产物改成迁就新工具。
丹药只检查映射、四类文件和内嵌 atlas 原字节，避免测试偷偷依赖运行时注册表。
"""

from __future__ import annotations

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
