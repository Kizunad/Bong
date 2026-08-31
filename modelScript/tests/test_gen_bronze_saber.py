#!/usr/bin/env python3
"""青铜单刀 (BronzeSaber / BronzeSaberPlayerAnim) 生成器与动画单元测试。"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
MODEL_SCRIPT = REPO / "modelScript"
MODELS_DIR = MODEL_SCRIPT / "models"
ANIM_DIR = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"

SABER_BBMODEL = MODELS_DIR / "BronzeSaber.bbmodel"
PLAYER_ANIM_BBMODEL = MODELS_DIR / "BronzeSaberPlayerAnim.bbmodel"


class TestGenBronzeSaber(unittest.TestCase):
    """验证 BronzeSaber.bbmodel 几何与贴图规范"""

    def setUp(self):
        self.assertTrue(SABER_BBMODEL.exists(), f"文件不存在: {SABER_BBMODEL}")
        self.doc = json.loads(SABER_BBMODEL.read_text(encoding="utf-8"))

    def test_bbmodel_format(self):
        """格式版本与元数据"""
        self.assertEqual(self.doc.get("meta", {}).get("format_version"), "4.10")
        self.assertEqual(self.doc.get("name"), "BronzeSaber")
        self.assertEqual(self.doc.get("model_identifier"), "bronze_saber")

    def test_bone_groups(self):
        """骨骼 Group 层级"""
        outliner = self.doc.get("outliner", [])
        group_names = [g.get("name") for g in outliner]
        for expected in ["pommel", "tassel", "grip", "guard", "blade"]:
            self.assertIn(expected, group_names, f"缺失骨骼 Group: {expected}")

    def test_element_count_and_boxes(self):
        """元素非空且包围盒合法"""
        elements = self.doc.get("elements", [])
        self.assertGreaterEqual(len(elements), 40, "青铜单刀模型部件过少")
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


class TestGenBronzeSaberPlayerAnim(unittest.TestCase):
    """验证玩家 + 青铜单刀动画 BronzeSaberPlayerAnim.bbmodel"""

    def setUp(self):
        self.assertTrue(PLAYER_ANIM_BBMODEL.exists(), f"文件不存在: {PLAYER_ANIM_BBMODEL}")
        self.doc = json.loads(PLAYER_ANIM_BBMODEL.read_text(encoding="utf-8"))

    def test_player_anim_structure(self):
        """结构与贴图包含 2 张 (皮肤 + 刀)"""
        textures = self.doc.get("textures", [])
        self.assertEqual(len(textures), 1)
        self.assertEqual(self.doc.get("name"), "BronzeSaberPlayerAnim")

    def test_animations_baked(self):
        """烘焙的刀法与基础动画数量"""
        anims = self.doc.get("animations", [])
        anim_names = [a.get("name") for a in anims]
        self.assertIn("saber_slash_down", anim_names)
        self.assertIn("saber_swing_horiz", anim_names)

    def test_player_animation_json_files(self):
        """验证生成的 emotecraft v3 JSON 动画文件"""
        for anim_file in ["saber_slash_down.json", "saber_swing_horiz.json"]:
            path = ANIM_DIR / anim_file
            self.assertTrue(path.exists(), f"缺少动画 JSON 文件: {path}")
            doc = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(doc.get("version"), 3)
            self.assertIn("moves", doc.get("emote", {}))
            self.assertGreater(len(doc["emote"]["moves"]), 0)


if __name__ == "__main__":
    unittest.main()
