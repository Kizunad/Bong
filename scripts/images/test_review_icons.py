"""保护离线快照与审图标记的资源归属契约。"""

import json
import tempfile
import unittest
from pathlib import Path

import review_icons as review
from PIL import Image


class IconReviewTest(unittest.TestCase):
    def test_snapshot_survives_source_replacement_and_content_changes_isolate_marks(
        self,
    ):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / "source.png"
            Image.new("RGBA", (12, 24), (255, 255, 255, 80)).save(source)
            original = source.read_bytes()
            rows = [{"id": "I001", "path": "items/source.png", "icon": "source.png"}]
            first = review.build_review(rows, root, root / "first", "第一轮")
            second = review.build_review(rows, root, root / "second", "另一个标题")
            self.assertEqual(first["datasetId"], second["datasetId"])
            self.assertEqual("transparent", first["images"][0]["alpha"])
            Image.new("RGBA", (12, 24), (0, 0, 0, 255)).save(source)
            third = review.build_review(rows, root, root / "third", "下一轮")
            self.assertNotEqual(first["datasetId"], third["datasetId"])
            moved = root / "moved"
            (root / "first").rename(moved)
            for field in ("icon", "iconThumb"):
                self.assertTrue((moved / first["images"][0][field]).is_file())
            self.assertEqual(
                original, (moved / first["images"][0]["icon"]).read_bytes()
            )
            with self.assertRaises(FileExistsError):
                review.build_review(rows, root, moved, "禁止覆盖")
            self.assertEqual(
                first, json.loads((moved / "review-data.json").read_text())
            )

    def test_duplicate_resource_identity_cannot_mix_review_marks(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            Image.new("RGB", (8, 8)).save(root / "icon.png")
            row = {"id": "I001", "path": "items/icon.png", "icon": "icon.png"}
            for duplicate in ({**row, "id": "I002"}, {**row, "path": "other/icon.png"}):
                with self.subTest(duplicate=duplicate), self.assertRaises(ValueError):
                    review.build_review([row, duplicate], root, root / "review", "审图")
            self.assertFalse((root / "review").exists())

    def test_manifest_text_cannot_close_embedded_json_script(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            Image.new("RGB", (8, 8)).save(root / "icon.png")
            text = "</script><script>window.injected=true</script>"
            rows = [
                {
                    "id": "I001",
                    "path": "items/icon.png",
                    "icon": "icon.png",
                    "name": text,
                }
            ]
            output = root / "review"
            review.build_review(rows, root, output, text)
            html = (output / "index.html").read_text()
            embedded = html.split('<script id="data" type="application/json">', 1)[
                1
            ].split("</script>", 1)[0]
            self.assertEqual(text, json.loads(embedded)["images"][0]["name"])
            self.assertNotIn(text, html)


if __name__ == "__main__":
    unittest.main()
