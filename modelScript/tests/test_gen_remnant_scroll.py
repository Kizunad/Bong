#!/usr/bin/env python3
"""残卷（RemnantScroll）生成器与模型回归测试。"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LIB_DIR = Path(__file__).resolve().parents[1]
for _d in ("core", "generators"):
    sys.path.insert(0, str(LIB_DIR / _d))

import gen_remnant_scroll as scroll


class TestRemnantScrollModel(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.model, cls.cubes, cls.tex = scroll.build_bbmodel()

    def test_element_count_and_richness(self):
        """确保模型拥有足够细腻的体素分件（>= 30 个），拒绝偷懒单板。"""
        self.assertGreaterEqual(len(self.cubes), 30, f"实际仅 {len(self.cubes)} 个部件")
        self.assertEqual(len(self.model["elements"]), len(self.cubes))

    def test_bounding_box_proportions(self):
        """校验包围盒与参考图对齐：宽 11~17，高 15~20，厚度 1.2~5.0。"""
        min_x = min(c.frm[0] for c in self.cubes)
        max_x = max(c.to[0] for c in self.cubes)
        min_y = min(c.frm[1] for c in self.cubes)
        max_y = max(c.to[1] for c in self.cubes)
        min_z = min(c.frm[2] for c in self.cubes)
        max_z = max(c.to[2] for c in self.cubes)

        width = max_x - min_x
        height = max_y - min_y
        depth = max_z - min_z

        self.assertTrue(11.0 <= width <= 17.0, f"宽度 {width} 偏离设计范围 [11, 17]")
        self.assertTrue(15.0 <= height <= 20.0, f"高度 {height} 偏离设计范围 [15, 20]")
        self.assertTrue(1.2 <= depth <= 5.0, f"厚度 {depth} 偏离设计范围 [1.2, 5.0]")

    def test_iconic_features_present(self):
        """校验左下角放射巨晶刺、侧缘晶脊、上下毛边断茬等标志性特征。"""
        has_giant_spike = any("giant_spike" in c.name and "crys_l" in c.name for c in self.cubes)
        self.assertTrue(has_giant_spike, "缺失左侧标志性紫曜巨晶刺")

        fringes = [c for c in self.cubes if c.bone == "fringes"]
        self.assertGreaterEqual(len(fringes), 10, f"撕裂毛边数量不足 ({len(fringes)} < 10)")

        left_crystals = [c for c in self.cubes if c.bone == "crystals_left"]
        right_crystals = [c for c in self.cubes if c.bone == "crystals_right"]
        self.assertGreaterEqual(len(left_crystals), 6, "左侧晶簇部件过少")
        self.assertGreaterEqual(len(right_crystals), 6, "右侧晶簇部件过少")

    def test_bone_outliner_integrity(self):
        """校验骨骼层级与 outliner 包含全部声明骨骼，且元素挂载无遗漏。"""
        outliner_names = [b["name"] for b in self.model["outliner"]]
        for bone in scroll.BONE_ORDER:
            self.assertIn(bone, outliner_names)

        total_children = sum(len(b["children"]) for b in self.model["outliner"])
        self.assertEqual(total_children, len(self.model["elements"]))

    def test_texture_resolution_and_format(self):
        """校验贴图尺寸 256x256 RGBA 及内嵌 data URI 有效性。"""
        self.assertEqual(self.tex.size, (256, 256))
        self.assertEqual(self.tex.mode, "RGBA")

        tex_entry = self.model["textures"][0]
        self.assertEqual(tex_entry["width"], 256)
        self.assertEqual(tex_entry["height"], 256)
        self.assertTrue(tex_entry["source"].startswith("data:image/png;base64,"))

    def test_uv_ranges_valid(self):
        """确保所有面的 UV 坐标都落在 [0, 256] 合法贴图区间内。"""
        for elem in self.model["elements"]:
            for face_name, face_data in elem["faces"].items():
                u0, v0, u1, v1 = face_data["uv"]
                self.assertTrue(0.0 <= u0 <= 256.0, f"{elem['name']} {face_name} u0={u0} 越界")
                self.assertTrue(0.0 <= u1 <= 256.0, f"{elem['name']} {face_name} u1={u1} 越界")
                self.assertTrue(0.0 <= v0 <= 256.0, f"{elem['name']} {face_name} v0={v0} 越界")
                self.assertTrue(0.0 <= v1 <= 256.0, f"{elem['name']} {face_name} v1={v1} 越界")

    def test_format_version_compatibility(self):
        """确认 Blockbench 版本为 4.10 兼容格式。"""
        self.assertEqual(self.model["meta"]["format_version"], "4.10")
        self.assertEqual(self.model["meta"]["model_format"], "free")


if __name__ == "__main__":
    unittest.main()
