#!/usr/bin/env python3
"""灵布衫四件套生成器的几何、贴图、门禁与输出回归。"""

from __future__ import annotations

import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
sys.path.insert(0, str(LIB_DIR / "generators"))

import gen_spirit_cloth_armor as spirit  # noqa: E402
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, build_bbmodel, validate_part, write_material_assets  # noqa: E402


def _part_with(cubes: tuple[Cube, ...]) -> ArmorPart:
    return ArmorPart("probe", "PROBE", cubes)


class SpiritClothArmorGeneratorTest(unittest.TestCase):
    def test_exposes_four_distinct_parts_and_valid_mounts(self) -> None:
        all_parts = spirit.parts()
        self.assertEqual(
            [
                "spirit_cloth_helmet",
                "spirit_cloth_chestplate",
                "spirit_cloth_leggings",
                "spirit_cloth_boots",
            ],
            [part.key for part in all_parts],
        )
        for part in all_parts:
            validate_part(part)

    def test_each_piece_has_a_material_specific_shape_contract(self) -> None:
        helmet, chest, legs, boots = spirit.parts()
        self.assertGreaterEqual(len(helmet.cubes), 17, "头巾必须有颅盖、眉箍、护耳、后脑搭接和系带")
        self.assertGreaterEqual(len(chest.cubes), 28, "胸甲必须有衣身、交领、腰缠和袖片")
        self.assertGreaterEqual(len(legs.cubes), 28, "护腿必须有双腿布片与三道缠带")
        self.assertGreaterEqual(len(boots.cubes), 40, "双靴必须有鞋底、脚面、鞋筒与踝部缠带")
        self.assertEqual({"HEAD"}, {cube.mount for cube in helmet.cubes})
        self.assertEqual({"BODY"}, {cube.mount for cube in chest.cubes})
        self.assertEqual({"LEFT_LEG", "RIGHT_LEG"}, {cube.mount for cube in legs.cubes})
        self.assertEqual({"LEFT_FOOT", "RIGHT_FOOT"}, {cube.mount for cube in boots.cubes})
        names = {cube.name for cube in boots.cubes}
        self.assertTrue(any(name.startswith("toe_") for name in names))
        self.assertTrue(any(name.startswith("heel_") for name in names))
        self.assertTrue(any(name.startswith("shaft_") for name in names))

    def test_texture_is_deterministic_64_square_and_four_tone(self) -> None:
        first = spirit.make_texture()
        second = spirit.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual("RGB", first.mode)
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertGreater(len(set(first.getdata())), 100, "灵布纹理不能退化为四块纯色")
        quadrants = [
            first.crop((0, 0, 32, 32)),
            first.crop((32, 0, 64, 32)),
            first.crop((0, 32, 32, 64)),
            first.crop((32, 32, 64, 64)),
        ]
        means = [sum(sum(pixel) for pixel in image.getdata()) / (32 * 32 * 3) for image in quadrants]
        self.assertGreater(means[0], means[1] + 20.0, "阴影蓝青象限必须显著深于主布")
        self.assertGreater(means[2], means[0] + 15.0, "月白缠带必须显著亮于主布")

    def test_geometry_guards_pass_and_calculate_connectivity(self) -> None:
        all_parts = spirit.parts()
        spirit._assert_no_coplanar_faces(all_parts)
        spirit._assert_uv_tiles(all_parts)
        spirit._assert_mirror_symmetry(all_parts)
        spirit._assert_no_isolated_cubes(all_parts)
        spirit._assert_helmet_front_projection(all_parts)
        spirit._assert_shape_dimensions(all_parts)

    def test_shape_dimension_guard_catches_a_front_projection_regression(self) -> None:
        helmet = spirit.part_helmet()
        brow_index = next(index for index, cube in enumerate(helmet.cubes) if cube.name == "brow_wrap")
        brow = helmet.cubes[brow_index]
        broken = replace(brow, origin=(brow.origin[0], brow.origin[1], brow.origin[2] - 1.0))
        cubes = helmet.cubes[:brow_index] + (broken,) + helmet.cubes[brow_index + 1:]
        with self.assertRaisesRegex(ValueError, "前缘"):
            spirit._assert_shape_dimensions((replace(helmet, cubes=cubes),))

    def test_connectivity_guard_catches_a_moved_cube(self) -> None:
        part = spirit.part_helmet()
        moved = replace(part.cubes[0], origin=(part.cubes[0].origin[0] + 12.0, *part.cubes[0].origin[1:]))
        broken = _part_with((moved,) + part.cubes[1:])
        with self.assertRaisesRegex(ValueError, "孤立 cube"):
            spirit._assert_no_isolated_cubes((broken,))

    def test_coplanar_guard_catches_a_real_face_collision(self) -> None:
        bad = _part_with(
            (
                Cube("BODY", "front", (-4.0, 12.0, -2.5), (8.0, 10.0, 1.0), spirit.UV_CLOTH_MAIN),
                Cube("BODY", "inner", (-4.0, 12.0, -2.5), (4.0, 5.0, 1.0), spirit.UV_CLOTH_SHADE),
            )
        )
        with self.assertRaisesRegex(ValueError, "共面"):
            spirit._assert_no_coplanar_faces((bad,))

    def test_uv_guard_catches_an_out_of_tile_cube(self) -> None:
        bad = replace(spirit.part_chestplate().cubes[0], uv=(64, 64))
        with self.assertRaisesRegex(ValueError, "未知 uv|超出"):
            spirit._assert_uv_tiles((_part_with((bad,)),))

    def test_gatekit_differential_self_test_passes(self) -> None:
        self.assertEqual(0, spirit.GATES.self_test(spirit.build(), verbose=False))

    def test_emit_java_and_digest_are_stable(self) -> None:
        all_parts = spirit.parts()
        java = spirit.emit_java(all_parts)
        self.assertEqual(sum(len(part.cubes) for part in all_parts), java.count("new ArmorCube("))
        for part in all_parts:
            self.assertEqual(16, len(spirit.cube_digest(part)))
            self.assertEqual(spirit.cube_digest(part), spirit.cube_digest(part))

    def test_bbmodel_preserves_cubes_mounts_and_namespace(self) -> None:
        texture = spirit.make_texture()
        for part in spirit.parts():
            model = build_bbmodel(spirit.MATERIAL, part, texture)
            self.assertEqual(len(part.cubes), len(model["elements"]))
            self.assertEqual(f"geometry.bong.{part.key}", model["model_identifier"])
            self.assertEqual({cube.mount for cube in part.cubes}, {group["name"].upper() for group in model["outliner"]})

    def test_writer_emits_four_models_four_textures_four_views_and_combined(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outputs = write_material_assets(
                spirit.MATERIAL,
                spirit.parts(),
                spirit.make_texture(),
                root / "models",
                root / "textures",
                root / "previews",
                render_previews=True,
            )
            self.assertEqual(13, len(outputs))
            self.assertEqual(4, len(list((root / "models/armor/spirit_cloth").glob("*.bbmodel"))))
            self.assertEqual(4, len(list((root / "textures").glob("spirit_cloth_*/0.png"))))
            for key in (
                "preview:spirit_cloth_helmet",
                "preview:spirit_cloth_chestplate",
                "preview:spirit_cloth_leggings",
                "preview:spirit_cloth_boots",
                "preview:all",
            ):
                self.assertTrue(outputs[key].is_file(), key)
                with Image.open(outputs[key]) as image:
                    self.assertGreater(image.width, 0)
                    self.assertGreater(image.height, 0)


if __name__ == "__main__":
    unittest.main()
