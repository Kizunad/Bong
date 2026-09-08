#!/usr/bin/env python3

from __future__ import annotations

import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in ("generators", "exporters", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_copper_armor as copper
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, build_bbmodel, validate_part, write_material_assets


class CopperArmorGeneratorTest(unittest.TestCase):
    def test_four_part_functions_have_distinct_dense_silhouettes(self) -> None:
        parts = copper.parts()
        self.assertEqual(
            ["copper_helmet", "copper_chestplate", "copper_leggings", "copper_boots"],
            [part.key for part in parts],
        )
        self.assertEqual([19, 25, 18, 16], [len(part.cubes) for part in parts])
        for part in parts:
            validate_part(part)

    def test_texture_is_deterministic_64_square_and_not_flat(self) -> None:
        first = copper.make_texture()
        second = copper.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual(first.tobytes(), second.tobytes())
        colors = len(set(first.getdata()))
        self.assertGreater(colors, 100, "古铜锻面与铜绿氧化贴图不应退化为纯色")

    def test_bbmodel_preserves_every_cube_and_mount_group(self) -> None:
        part = copper.part_leggings()
        model = build_bbmodel("copper", part, copper.make_texture())
        self.assertEqual(len(part.cubes), len(model["elements"]))
        self.assertEqual({"left_leg", "right_leg"}, {group["name"] for group in model["outliner"]})
        self.assertTrue(str(model["textures"][0]["source"]).startswith("data:image/png;base64,"))

    def test_writer_emits_four_models_four_runtime_textures_and_five_previews(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outputs = write_material_assets(
                "copper",
                copper.parts(),
                copper.make_texture(),
                root / "models",
                root / "textures",
                root / "previews",
                render_previews=True,
            )
            self.assertEqual(13, len(outputs), "4 model + 4 texture + 4 three-view + 1 combined 应全部产出")


if __name__ == "__main__":
    unittest.main()
