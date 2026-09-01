#!/usr/bin/env python3
"""凡铁采药刀 (HerbKnifeIron) 生成器单元测试。"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
MODEL_SCRIPT = REPO / "modelScript"
MODELS_DIR = MODEL_SCRIPT / "models"
HERB_KNIFE_BBMODEL = MODELS_DIR / "HerbKnifeIron.bbmodel"


class TestGenHerbKnifeIron(unittest.TestCase):
    """验证 HerbKnifeIron.bbmodel 几何与贴图规范"""

    def setUp(self):
        self.assertTrue(HERB_KNIFE_BBMODEL.exists(), f"文件不存在: {HERB_KNIFE_BBMODEL}")
        self.doc = json.loads(HERB_KNIFE_BBMODEL.read_text(encoding="utf-8"))

    def test_bbmodel_format(self):
        """格式版本与元数据"""
        self.assertEqual(self.doc.get("meta", {}).get("format_version"), "4.10")
        self.assertEqual(self.doc.get("name"), "HerbKnifeIron")
        self.assertEqual(self.doc.get("model_identifier"), "herb_knife_iron")

    def test_bone_groups(self):
        """骨骼 Group 层级完整性"""
        outliner = self.doc.get("outliner", [])
        group_names = [g.get("name") for g in outliner]
        for expected in ["pivot", "handle", "blade_spine", "blade_edge", "tail_cord"]:
            self.assertIn(expected, group_names, f"缺失骨骼 Group: {expected}")

    def test_element_count_and_boxes(self):
        """元素数量合理且包围盒无退化"""
        elements = self.doc.get("elements", [])
        self.assertGreaterEqual(len(elements), 35, "采药刀模型部件过少")
        for el in elements:
            frm = el.get("from", [0, 0, 0])
            to = el.get("to", [0, 0, 0])
            for i in range(3):
                self.assertLess(frm[i], to[i], f"元素 {el.get('name')} 轴 {i} 出现退化盒")

    def test_texture_resolution(self):
        """贴图分辨率为 64x64"""
        res = self.doc.get("resolution", {})
        self.assertEqual(res.get("width"), 64)
        self.assertEqual(res.get("height"), 64)
        textures = self.doc.get("textures", [])
        self.assertEqual(len(textures), 1)
        self.assertTrue(textures[0].get("source", "").startswith("data:image/png;base64,"))


class TestGenHerbKnifeIronPlayerAnim(unittest.TestCase):
    """验证玩家 + 凡铁采药刀动画 HerbKnifeIronPlayerAnim.bbmodel"""

    def setUp(self):
        self.player_anim_path = MODELS_DIR / "HerbKnifeIronPlayerAnim.bbmodel"
        self.assertTrue(self.player_anim_path.exists(), f"文件不存在: {self.player_anim_path}")
        self.doc = json.loads(self.player_anim_path.read_text(encoding="utf-8"))

    def test_player_anim_structure(self):
        """结构与贴图包含 1 张 Atlas 图集"""
        textures = self.doc.get("textures", [])
        self.assertEqual(len(textures), 1)
        self.assertEqual(self.doc.get("name"), "HerbKnifeIronPlayerAnim")

    def test_animations_baked(self):
        """烘焙的采药与折刀动画数量"""
        anims = self.doc.get("animations", [])
        anim_names = [a.get("name") for a in anims]
        for expected in ["herb_harvest", "herb_knife_slash", "herb_knife_unfold"]:
            self.assertIn(expected, anim_names, f"缺失烘焙动画: {expected}")

    def test_player_animation_json_files(self):
        """验证生成的 emotecraft v3 JSON 动画文件"""
        anim_dir = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"
        for anim_file in ["herb_harvest.json", "herb_knife_slash.json", "herb_knife_unfold.json"]:
            path = anim_dir / anim_file
            self.assertTrue(path.exists(), f"缺少动画 JSON 文件: {path}")
            doc = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(doc.get("version"), 3)
            self.assertIn("moves", doc.get("emote", {}))
            self.assertGreater(len(doc["emote"]["moves"]), 0)


if __name__ == "__main__":
    unittest.main()
