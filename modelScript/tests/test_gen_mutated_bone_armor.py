#!/usr/bin/env python3
"""测试异兽刺骨甲（mutated_bone_armor）模型生成器、贴图与无冲突断言。"""

from __future__ import annotations

import unittest
from pathlib import Path
import sys

_REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_REPO / "modelScript" / "generators"))

from gen_mutated_bone_armor import (
    MATERIAL,
    make_texture,
    parts,
    _assert_no_coplanar_faces,
    generate,
    emit_java,
)


class TestMutatedBoneArmor(unittest.TestCase):
    def test_parts_structure(self) -> None:
        all_parts = parts()
        self.assertEqual(len(all_parts), 2)
        keys = [p.key for p in all_parts]
        self.assertIn("mutated_bone_chestplate", keys)
        self.assertIn("mutated_bone_leggings", keys)

    def test_chestplate_cubes_and_features(self) -> None:
        all_parts = parts()
        chest = next(p for p in all_parts if p.key == "mutated_bone_chestplate")
        cube_names = [c.name for c in chest.cubes]

        # 验证胸骨中脊与肋骨
        self.assertIn("chest_sternum_core", cube_names)
        self.assertIn("chest_rib_top_l", cube_names)
        self.assertIn("chest_rib_top_r", cube_names)

        # 验证左肩单侧兽颅肩甲
        self.assertIn("skull_cranium_main", cube_names)
        self.assertIn("skull_fang_front", cube_names)
        self.assertIn("skull_orbit_front", cube_names)

        # 验证小臂刺骨护腕
        self.assertIn("armguard_plate_outer_l", cube_names)
        self.assertIn("armguard_plate_outer_r", cube_names)
        self.assertIn("armguard_wrap_top_l", cube_names)
        self.assertIn("armguard_wrap_top_r", cube_names)

        # 全部挂在 BODY
        self.assertTrue(all(c.mount == "BODY" for c in chest.cubes))

    def test_leggings_cubes_and_mounts(self) -> None:
        all_parts = parts()
        legs = next(p for p in all_parts if p.key == "mutated_bone_leggings")
        cube_names = [c.name for c in legs.cubes]

        # 验证左右腿独立胫骨与绑腿
        self.assertIn("left_leg_shin_plate_main", cube_names)
        self.assertIn("right_leg_shin_plate_main", cube_names)
        self.assertIn("left_leg_shin_strap_top", cube_names)
        self.assertIn("right_leg_shin_strap_top", cube_names)

        # 挂载点在左右腿
        mounts = {c.mount for c in legs.cubes}
        self.assertEqual(mounts, {"LEFT_LEG", "RIGHT_LEG"})

    def test_no_coplanar_faces(self) -> None:
        all_parts = parts()
        # 不抛异常即为通过
        _assert_no_coplanar_faces(all_parts)

    def test_texture_generation(self) -> None:
        img = make_texture()
        self.assertEqual(img.size, (64, 64))
        self.assertEqual(img.mode, "RGB")
        # 确保色彩丰富度高，非纯色
        colors = len(set(img.getdata()))
        self.assertGreater(colors, 100)

    def test_emit_java(self) -> None:
        code = emit_java(parts())
        self.assertIn("mutatedBoneChestplate", code)
        self.assertIn("mutatedBoneLeggings", code)
        self.assertIn("new ArmorCube", code)

    def test_generate_pipeline(self) -> None:
        outputs = generate(render_previews=False)
        self.assertIn("model:mutated_bone_chestplate", outputs)
        self.assertIn("model:mutated_bone_leggings", outputs)
        self.assertIn("model_on_player:mutated_bone_chestplate", outputs)
        self.assertIn("model_on_player:mutated_bone_leggings", outputs)
        self.assertTrue(outputs["model:mutated_bone_chestplate"].exists())
        self.assertTrue(outputs["model:mutated_bone_leggings"].exists())
        self.assertTrue(outputs["model_on_player:mutated_bone_chestplate"].exists())
        self.assertTrue(outputs["model_on_player:mutated_bone_leggings"].exists())


if __name__ == "__main__":
    unittest.main()
