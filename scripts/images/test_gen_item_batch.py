from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import gen_item_batch as batch


class GenItemBatchTest(unittest.TestCase):
    def test_load_items_recurses_toml_and_builds_prompt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            nested = root / "nested"
            nested.mkdir()
            source = nested / "items.toml"
            source.write_text(
                """
[[item]]
id = "bone_coin_5"
name = "封灵骨币·五"
category = "bone_coin"
rarity = "common"
""",
                encoding="utf-8",
            )

            items = batch.load_items(root)

        item = items["bone_coin_5"]
        self.assertEqual("封灵骨币·五", item.name)
        self.assertEqual("bone_coin", item.category)
        self.assertIn("透明背景", batch.prompt_for(item))
        self.assertIn("128×128 icon", batch.prompt_for(item))

    def test_selected_items_filters_existing_textures_without_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "bone_coin_5.png").write_bytes(b"png")
            items = {
                "bone_coin_5": batch.ItemSpec("bone_coin_5", "封灵骨币·五", "bone_coin", Path("a.toml"), "common"),
                "shu_gu": batch.ItemSpec("shu_gu", "噬元鼠骨", "misc", Path("b.toml"), "common"),
            }

            selected = batch.selected_items(items, out, ["bone_coin_5", "shu_gu"], False, False)

        self.assertEqual(["shu_gu"], [item.item_id for item in selected])

    def test_load_items_rejects_duplicate_ids(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first = root / "first.toml"
            second = root / "second.toml"
            first.write_text(
                """
[[item]]
id = "bone_coin_5"
name = "封灵骨币·五"
category = "bone_coin"
""",
                encoding="utf-8",
            )
            second.write_text(
                """
[[item]]
id = "bone_coin_5"
name = "重复骨币"
category = "bone_coin"
""",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "duplicate item id"):
                batch.load_items(root)

    def test_parse_ids_deduplicates_while_preserving_order(self) -> None:
        ids = batch.parse_ids(["bone_coin_5, shu_gu", "bone_coin_5", "zhu_gu"])

        self.assertEqual(["bone_coin_5", "shu_gu", "zhu_gu"], ids)

    def test_generation_command_calls_project_gen_py_with_item_style(self) -> None:
        item = batch.ItemSpec("shu_gu", "噬元鼠骨", "misc", Path("fauna.toml"), "common")
        command = batch.gen_command(item, Path("out"), "cliproxy")

        self.assertIn("--style", command)
        self.assertIn("item", command)
        self.assertIn("--transparent", command)
        self.assertIn("噬元鼠骨", " ".join(command))


if __name__ == "__main__":
    unittest.main()
