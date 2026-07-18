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
        self.assertEqual([21, 37, 30, 28], [len(part.cubes) for part in parts])
        for part in parts:
            validate_part(part)
        self.assertTrue(any("rib" in cube.name for cube in bone.part_chestplate().cubes))
        self.assertTrue(any("horn" in cube.name for cube in bone.part_helmet().cubes))
        helmet_names = {cube.name for cube in bone.part_helmet().cubes}
        self.assertNotIn("left_crown_rail", helmet_names)
        self.assertEqual(3, len([name for name in helmet_names if name.startswith("left_horn_")]))
        self.assertEqual(2, len([name for name in helmet_names if name.startswith("right_horn_")]))
        self.assertEqual(
            3,
            len([cube for cube in bone.part_chestplate().cubes if "spine_knob" in cube.name]),
        )

    def test_rope_loops_are_hollow_strips_not_solid_platforms(self) -> None:
        rope_cubes = [cube for part in bone.parts() for cube in part.cubes if "rope" in cube.name]
        self.assertGreaterEqual(len(rope_cubes), 25)
        for cube in rope_cubes:
            sx, _, sz = cube.size
            self.assertFalse(
                sx > 1.0 and sz > 1.0,
                f"{cube.name} 同时横跨 x/z，会把绑绳烘焙成实心平台",
            )

    def test_texture_is_deterministic_mottled_and_bone_colored(self) -> None:
        first = bone.make_texture()
        second = bone.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertGreater(len(set(first.getdata())), 100)
        r, g, b = first.getpixel((4, 4))
        self.assertGreater(r, b, "骨色应偏暖灰白而非冷铁灰")
        self.assertGreater(g, b)
        self.assertEqual((118, 79, 50), first.getpixel((0, 37)), "绑绳亮纤维色应被固定")

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
