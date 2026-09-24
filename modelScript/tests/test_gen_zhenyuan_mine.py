#!/usr/bin/env python3
"""真元诡雷（zhenyuan_mine）生成器测试与回归锁。"""

from __future__ import annotations

import json
import math
import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "generators"))

import gen_zhenyuan_mine as mine


class ZhenyuanMineGeneratorTest(unittest.TestCase):
    def test_model_build_and_structure(self):
        model, cubes, tex = mine.build_bbmodel()
        self.assertIsInstance(model, dict)
        self.assertEqual(model["name"], "zhenyuan_mine")
        self.assertEqual(model["model_identifier"], "geometry.bong.zhenyuan_mine")
        self.assertEqual(model["meta"]["format_version"], "4.10")
        
        # 检验各部分组件均已生成
        cubes_names = [c[2] for c in cubes]
        # 1. 检验底座
        self.assertTrue(any("base_slab" in n for n in cubes_names))
        # 2. 检验四大主岩板
        self.assertTrue(any("plate_nw_main" in n for n in cubes_names))
        self.assertTrue(any("plate_ne_main" in n for n in cubes_names))
        self.assertTrue(any("plate_sw_main" in n for n in cubes_names))
        self.assertTrue(any("plate_se_main" in n for n in cubes_names))
        # 3. 检验真元核心突刺
        self.assertTrue(any("core_burst_center" in n for n in cubes_names))
        self.assertTrue(any("core_spike_tip" in n for n in cubes_names))
        # 4. 检验四角锁灵骨桩
        self.assertTrue(any("bone_post_shaft_nw" in n for n in cubes_names))
        self.assertTrue(any("bone_post_cap_se" in n for n in cubes_names))
        # 5. 检验四边金属锁扣
        self.assertTrue(any("clasp_n" in n for n in cubes_names))
        self.assertTrue(any("clasp_e" in n for n in cubes_names))

    def test_texture_resolution(self):
        _, _, tex = mine.build_bbmodel()
        self.assertEqual(tex.size, (mine.RES, mine.RES))
        self.assertEqual(tex.mode, "RGBA")


if __name__ == "__main__":
    unittest.main()
