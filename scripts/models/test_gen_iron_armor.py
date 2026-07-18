#!/usr/bin/env python3

from __future__ import annotations

import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from PIL import Image

MODEL_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(MODEL_DIR))

import gen_iron_armor as iron
from armor_model_common import ArmorPart, Cube, build_bbmodel, validate_part, write_material_assets


class IronArmorGeneratorTest(unittest.TestCase):
    def test_four_part_functions_have_distinct_dense_silhouettes(self) -> None:
        parts = iron.parts()
        self.assertEqual(
            ["iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots"],
            [part.key for part in parts],
        )
        self.assertEqual([15, 23, 18, 14], [len(part.cubes) for part in parts])
        for part in parts:
            validate_part(part)

    def test_texture_is_deterministic_64_square_and_not_flat(self) -> None:
        first = iron.make_texture()
        second = iron.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertGreater(len(set(first.getdata())), 100, "锤纹/锈蚀贴图不应退化为纯色")

    def test_bbmodel_preserves_every_cube_and_mount_group(self) -> None:
        part = iron.part_leggings()
        model = build_bbmodel("iron", part, iron.make_texture())
        self.assertEqual(len(part.cubes), len(model["elements"]))
        self.assertEqual({"left_leg", "right_leg"}, {group["name"] for group in model["outliner"]})
        self.assertTrue(str(model["textures"][0]["source"]).startswith("data:image/png;base64,"))

    def test_invalid_duplicate_and_nonpositive_cubes_fail_loud(self) -> None:
        valid = iron.part_helmet()
        duplicate = replace(valid, cubes=valid.cubes + (valid.cubes[0],))
        with self.assertRaisesRegex(ValueError, "duplicate cube name"):
            validate_part(duplicate)

        bad_cube = replace(valid.cubes[0], size=(0.0, 1.0, 1.0))
        invalid = ArmorPart("bad", "bad", (bad_cube,))
        with self.assertRaisesRegex(ValueError, "size must be positive"):
            validate_part(invalid)

    def test_writer_emits_four_models_and_four_runtime_textures(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outputs = write_material_assets(
                "iron",
                iron.parts(),
                iron.make_texture(),
                root / "models",
                root / "textures",
                root / "previews",
                render_previews=False,
            )
            self.assertEqual(8, len(outputs))
            self.assertEqual(4, len(list((root / "models/armor/iron").glob("*.bbmodel"))))
            textures = list((root / "textures").glob("iron_*/0.png"))
            self.assertEqual(4, len(textures))
            for path in textures:
                with Image.open(path) as texture:
                    self.assertEqual((64, 64), texture.size)

    def test_writer_rejects_duplicate_part_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            part = iron.part_helmet()
            with self.assertRaisesRegex(ValueError, "duplicate armor part key"):
                write_material_assets(
                    "iron",
                    (part, part),
                    iron.make_texture(),
                    root / "models",
                    root / "textures",
                    root / "previews",
                    render_previews=False,
                )


if __name__ == "__main__":
    unittest.main()
