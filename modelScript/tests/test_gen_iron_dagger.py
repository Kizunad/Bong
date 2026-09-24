import json
import sys
import tempfile
import unittest
from pathlib import Path
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
LIB = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB / "generators"))

from gen_iron_dagger import (
    BBMODEL_OUT,
    BONE_ORDER,
    EMIT_OFFSET,
    GUARD_Y,
    GRIP_Y,
    POMMEL_Y,
    BLADE_LEN,
    build_bbmodel_dict,
    build_cubes,
    generate_texture_atlas,
)


class TestGenIronDagger(unittest.TestCase):
    def test_cubes_and_groups_structure(self):
        cubes = build_cubes()
        self.assertGreater(len(cubes), 20, "凡铁匕首应包含足够细节的 Cubes (≥20)")
        
        # 检查所有 Group 都在 BONE_ORDER 中
        bones_found = set(c[0] for c in cubes)
        for b in bones_found:
            self.assertIn(b, BONE_ORDER)

    def test_texture_atlas_dimensions_and_quadrants(self):
        atlas = generate_texture_atlas()
        self.assertEqual(atlas.size, (64, 64), "Atlas 尺寸应为 64x64")
        self.assertEqual(atlas.mode, "RGBA")

    def test_bbmodel_generation_and_emit_offset(self):
        cubes = build_cubes()
        atlas = generate_texture_atlas()
        doc = build_bbmodel_dict(cubes, atlas)
        
        self.assertEqual(doc["meta"]["format_version"], "4.10")
        self.assertEqual(doc["name"], "IronDagger")
        self.assertEqual(len(doc["elements"]), len(cubes))
        
        # 验证 display 设置完备性
        displays = doc.get("display", {})
        self.assertIn("thirdperson_righthand", displays)
        self.assertIn("firstperson_righthand", displays)
        self.assertIn("gui", displays)

    def test_no_empty_elements(self):
        cubes = build_cubes()
        for bone, mat, name, f_xyz, t_xyz, rot in cubes:
            for i in range(3):
                self.assertLess(f_xyz[i], t_xyz[i], f"Cube {name} 在轴 {i} 上的 min 必须小于 max")


if __name__ == "__main__":
    unittest.main()
