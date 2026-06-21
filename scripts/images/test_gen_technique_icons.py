from __future__ import annotations

import unittest
from pathlib import Path

from PIL import Image

import gen_technique_icons as gen

# NPC 专用技能不该出现在玩家功法图标清单里（玩家不可绑定、不进 HUD）。
NPC_IDS = {"npc.heal_basic", "npc.buff_speed", "npc.buff_defense"}


class SafeIdTest(unittest.TestCase):
    def test_dots_colons_slashes_become_underscores(self) -> None:
        self.assertEqual("woliu_heart", gen.safe_id("woliu.heart"))
        self.assertEqual("a_b_c_d", gen.safe_id("a:b/c.d"))

    def test_plain_id_unchanged(self) -> None:
        self.assertEqual("shield_block", gen.safe_id("shield_block"))


class TechniqueListTest(unittest.TestCase):
    def test_no_duplicate_ids(self) -> None:
        ids = [tid for tid, _, _ in gen.TECHNIQUES]
        dupes = {i for i in ids if ids.count(i) > 1}
        self.assertEqual(set(), dupes, f"清单存在重复功法 id: {dupes}")

    def test_npc_skills_excluded(self) -> None:
        ids = {tid for tid, _, _ in gen.TECHNIQUES}
        leaked = ids & NPC_IDS
        self.assertEqual(set(), leaked, f"NPC 专用技能不应在玩家图标清单: {leaked}")

    def test_every_entry_has_name_and_prompt(self) -> None:
        for tid, name, prompt in gen.TECHNIQUES:
            self.assertTrue(name.strip(), f"{tid} 缺中文名")
            self.assertIn("solid black background", prompt,
                          f"{tid} 的 prompt 应统一 solid black background 底以便风格一致")


class CommittedIconAssetsTest(unittest.TestCase):
    """锁死「清单里每个功法 → 一张 128×128 RGBA 图标」契约。

    改技能 id / 误删图标 / 缩放规格漂移都会在此撞红，
    避免静默退回 #643 的文字标签兜底而无人察觉。
    """

    def test_every_technique_has_committed_icon(self) -> None:
        missing = []
        for tid, _, _ in gen.TECHNIQUES:
            target = gen.OUT_DIR / f"skill_scroll_{gen.safe_id(tid)}.png"
            if not target.exists():
                missing.append(tid)
        self.assertEqual([], missing,
                         f"以下功法缺图标(应在 {gen.OUT_DIR}): {missing}")

    def test_all_icons_are_128_rgba(self) -> None:
        bad = []
        for tid, _, _ in gen.TECHNIQUES:
            target = gen.OUT_DIR / f"skill_scroll_{gen.safe_id(tid)}.png"
            if not target.exists():
                continue
            with Image.open(target) as im:
                if im.size != (gen.ICON_SIZE, gen.ICON_SIZE) or im.mode != "RGBA":
                    bad.append((tid, im.size, im.mode))
        self.assertEqual([], bad,
                         f"以下图标规格不是 128×128 RGBA: {bad}")


if __name__ == "__main__":
    unittest.main()
