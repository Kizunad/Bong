#!/usr/bin/env python3
"""残卷缠甲 (scroll_wrap) 建模生成器单元测试。"""

import unittest
from pathlib import Path

from PIL import Image

import sys as _sys
_sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
_sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "generators"))

from gen_scroll_wrap_armor import (
    MATERIAL,
    generate,
    make_texture,
    parts,
    emit_java,
)


class TestGenScrollWrapArmor(unittest.TestCase):
    def test_parts_count_and_keys(self) -> None:
        all_parts = parts()
        self.assertEqual(len(all_parts), 4)
        keys = {p.key for p in all_parts}
        expected = {
            "scroll_wrap_helmet",
            "scroll_wrap_chestplate",
            "scroll_wrap_leggings",
            "scroll_wrap_boots",
        }
        self.assertEqual(keys, expected)

    def test_cubes_count_and_mounts(self) -> None:
        all_parts = parts()
        for p in all_parts:
            self.assertGreater(len(p.cubes), 8, f"{p.key} 立方体数量过少")
            for c in p.cubes:
                self.assertIn(
                    c.mount,
                    {"HEAD", "BODY", "LEFT_LEG", "RIGHT_LEG", "LEFT_FOOT", "RIGHT_FOOT"},
                )

    def test_make_texture(self) -> None:
        tex = make_texture()
        self.assertEqual(tex.size, (64, 64))
        self.assertEqual(tex.mode, "RGB")
        # 确保包含丰富颜色阶梯与噪点
        colors = len(set(tex.get_flattened_data() if hasattr(tex, "get_flattened_data") else tex.getdata()))
        self.assertGreater(colors, 100)

    def test_emit_java(self) -> None:
        java_code = emit_java(parts())
        self.assertIn("private static List<ArmorCube> scrollWrapHelmet()", java_code)
        self.assertIn("private static List<ArmorCube> scrollWrapChestplate()", java_code)
        self.assertIn("private static List<ArmorCube> scrollWrapLeggings()", java_code)
        self.assertIn("private static List<ArmorCube> scrollWrapBoots()", java_code)

    def test_generate_outputs(self) -> None:
        outputs = generate(render_previews=False)
        self.assertIn("model:scroll_wrap_helmet", outputs)
        self.assertIn("model:scroll_wrap_chestplate", outputs)
        self.assertIn("model:scroll_wrap_leggings", outputs)
        self.assertIn("model:scroll_wrap_boots", outputs)
        self.assertIn("texture:scroll_wrap_helmet", outputs)


if __name__ == "__main__":
    unittest.main()
