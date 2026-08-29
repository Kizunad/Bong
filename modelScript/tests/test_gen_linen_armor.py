#!/usr/bin/env python3
"""麻布僧袍生成器的单元测试与回归锁。"""

from __future__ import annotations

import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from PIL import Image

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in ("generators", "exporters", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_linen_armor as linen
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, MOUNT_X, build_bbmodel, validate_part, write_material_assets


def _part_with(cubes: tuple[Cube, ...]) -> ArmorPart:
    return ArmorPart("probe", "PROBE", cubes)


class LinenArmorGeneratorTest(unittest.TestCase):
    def test_exposes_chestplate_and_leggings(self) -> None:
        parts = linen.parts()
        self.assertEqual(["linen_chestplate", "linen_leggings"], [part.key for part in parts])
        for part in parts:
            validate_part(part)

    def test_texture_is_deterministic_64_square_and_not_flat(self) -> None:
        first = linen.make_texture()
        second = linen.make_texture()
        self.assertEqual((64, 64), first.size)
        self.assertEqual(first.tobytes(), second.tobytes())
        self.assertGreater(len(set(first.getdata())), 100, "贴图噪点与经纬织物不应退化为纯色")

    def test_texture_quadrants_have_distinct_values(self) -> None:
        """验证四个象限的明度和色调显著分离：护手/绑脚(Q3)显著亮于主布(Q1)与深色外披(Q2)。"""
        img = linen.make_texture()
        # 采样四个区域的平均亮度
        def avg_v(box: tuple[int, int, int, int]) -> float:
            x0, y0, x1, y1 = box
            vals = []
            for y in range(y0, y1):
                for x in range(x0, x1):
                    r, g, b = img.getpixel((x, y))
                    vals.append(0.299 * r + 0.587 * g + 0.114 * b)
            return sum(vals) / len(vals)

        v_q1_main = avg_v((4, 4, 28, 28))
        v_q2_dark = avg_v((36, 4, 60, 28))
        v_q3_wrap = avg_v((4, 36, 28, 60))
        v_q4_rope = avg_v((36, 36, 60, 60))

        self.assertGreater(v_q3_wrap, v_q1_main + 25.0, "白麻护手绑脚必须显著亮于主粗麻布")
        self.assertGreater(v_q1_main, v_q2_dark + 25.0, "内裤主粗麻布必须显著亮于深茶色外袍裙摆")

    def test_bbmodel_preserves_every_cube_and_mount_group(self) -> None:
        texture = linen.make_texture()
        for part in linen.parts():
            model = build_bbmodel(linen.MATERIAL, part, texture)
            self.assertEqual(len(part.cubes), len(model["elements"]))
            self.assertEqual(f"geometry.bong.{part.key}", model["model_identifier"])
            mounts = {c.mount for c in part.cubes}
            self.assertEqual(mounts, {g["name"].upper() for g in model["outliner"]})

    def test_element_origins_carry_the_mount_offset(self) -> None:
        texture = linen.make_texture()
        part = linen.part_leggings()
        model = build_bbmodel(linen.MATERIAL, part, texture)
        by_name = {e["name"]: e for e in model["elements"]}
        for cube in part.cubes:
            self.assertAlmostEqual(
                cube.origin[0] + MOUNT_X[cube.mount],
                by_name[cube.name]["from"][0],
                places=3,
            )

    def test_no_coplanar_faces_passes_on_real_parts(self) -> None:
        parts = linen.parts()
        linen._assert_no_coplanar_faces(parts)

    def test_coplanar_guard_catches_mutation(self) -> None:
        bad = _part_with((
            Cube("BODY", "c1", (-4.0, 12.0, -2.5), (8.0, 10.0, 1.0), linen.UV_LINEN_MAIN),
            Cube("BODY", "c2", (-4.0, 12.0, -2.5), (4.0, 5.0, 1.0), linen.UV_LINEN_MAIN),
        ))
        with self.assertRaisesRegex(ValueError, "共面"):
            linen._assert_no_coplanar_faces((bad,))

    def test_emit_java_and_digest_are_stable(self) -> None:
        for part in linen.parts():
            java = linen.emit_java(part)
            self.assertEqual(len(part.cubes), java.count("new ArmorCube("))
            digest = linen.cube_digest(part)
            self.assertEqual(16, len(digest))
            self.assertEqual(digest, linen.cube_digest(part))

    def test_digest_changes_when_geometry_changes(self) -> None:
        part = linen.part_chestplate()
        moved = replace(part, cubes=(replace(part.cubes[0], origin=(-4.3, 12.8, -2.52)),) + part.cubes[1:])
        self.assertNotEqual(linen.cube_digest(part), linen.cube_digest(moved))

    def test_writer_emits_two_models_two_textures_and_three_previews(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outputs = write_material_assets(
                linen.MATERIAL,
                linen.parts(),
                linen.make_texture(),
                root / "models",
                root / "textures",
                root / "previews",
                render_previews=True,
            )
            self.assertEqual(7, len(outputs), "2 model + 2 texture + 2 three-view + 1 combined 应全部产出")
            self.assertEqual(2, len(list((root / "models/armor/linen").glob("*.bbmodel"))))
            textures = list((root / "textures").glob("linen_*/0.png"))
            self.assertEqual(2, len(textures))
            for path in textures:
                with Image.open(path) as tex:
                    self.assertEqual((64, 64), tex.size)

            for key in ("preview:linen_chestplate", "preview:linen_leggings", "preview:all"):
                path = outputs[key]
                self.assertTrue(path.is_file(), f"预览输出缺失: {path}")
                with Image.open(path) as prev:
                    self.assertGreater(prev.width, 0)
                    self.assertGreater(prev.height, 0)


if __name__ == "__main__":
    unittest.main()
