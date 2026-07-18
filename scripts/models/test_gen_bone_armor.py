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

import gen_bone_armor as bone
from armor_model_common import ArmorPart, build_bbmodel, validate_part, write_material_assets


class BoneArmorGeneratorTest(unittest.TestCase):
    def test_four_parts_use_open_rib_and_splint_silhouettes(self) -> None:
        parts = bone.parts()
        self.assertEqual(
            ["bone_helmet", "bone_chestplate", "bone_leggings", "bone_boots"],
            [part.key for part in parts],
        )
        self.assertEqual([16, 25, 14, 16], [len(part.cubes) for part in parts])
        for part in parts:
            validate_part(part)
        self.assertTrue(any("rib" in cube.name for cube in bone.part_chestplate().cubes))
        self.assertTrue(any("horn" in cube.name for cube in bone.part_helmet().cubes))

    def test_texture_is_deterministic_mottled_and_bone_colored(self) -> None:
        first = bone.make_texture()
        second = bone.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertGreater(len(set(first.getdata())), 100)
        r, g, b = first.getpixel((4, 4))
        self.assertGreater(r, b, "骨色应偏暖灰白而非冷铁灰")
        self.assertGreater(g, b)

    def test_bbmodel_preserves_left_right_foot_mounts(self) -> None:
        part = bone.part_boots()
        model = build_bbmodel("bone", part, bone.make_texture())
        self.assertEqual(len(part.cubes), len(model["elements"]))
        self.assertEqual({"left_foot", "right_foot"}, {group["name"] for group in model["outliner"]})

    def test_invalid_duplicate_cube_fails_loud(self) -> None:
        valid = bone.part_helmet()
        duplicate = replace(valid, cubes=valid.cubes + (valid.cubes[0],))
        with self.assertRaisesRegex(ValueError, "duplicate cube name"):
            validate_part(duplicate)

        invalid = ArmorPart("empty", "empty", ())
        with self.assertRaisesRegex(ValueError, "must have key and cubes"):
            validate_part(invalid)

    def test_writer_emits_four_models_and_runtime_textures(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outputs = write_material_assets(
                "bone",
                bone.parts(),
                bone.make_texture(),
                root / "models",
                root / "textures",
                root / "previews",
                render_previews=False,
            )
            self.assertEqual(8, len(outputs))
            self.assertEqual(4, len(list((root / "models/armor/bone").glob("*.bbmodel"))))
            textures = list((root / "textures").glob("bone_*/0.png"))
            self.assertEqual(4, len(textures))
            for path in textures:
                with Image.open(path) as texture:
                    self.assertEqual((64, 64), texture.size)

    def test_writer_rejects_duplicate_part_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            part = bone.part_helmet()
            with self.assertRaisesRegex(ValueError, "duplicate armor part key"):
                write_material_assets(
                    "bone",
                    (part, part),
                    bone.make_texture(),
                    root / "models",
                    root / "textures",
                    root / "previews",
                    render_previews=False,
                )


if __name__ == "__main__":
    unittest.main()
