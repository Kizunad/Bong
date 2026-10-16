"""末土 NPC 皮肤：8 张 64×64 classic 皮肤的格式与可复现性。

只守三件事：尺寸 / 通道正确、基础层六个盒子的每个像素都不透明（否则游戏里会露出洞）、
生成器重跑得到的 PNG 与仓库里提交的逐字节一致（皮肤只由脚本产生，不手改 PNG）。
"""

from __future__ import annotations

import importlib.util
import io
import sys
import unittest
from pathlib import Path

import numpy as np
from PIL import Image

NPC_DIR = Path(__file__).resolve().parents[1] / "npcs"
sys.path.insert(0, str(NPC_DIR / "_kit"))
from skin_kit import BOXES, face_rect  # noqa: E402

NPC_IDS = ("gu_bi_you_shang", "can_feng_san_xiu", "shi_gu_ren", "cai_yao_ren",
           "zong_men_can_tu", "bei_huang_lie_hu", "shou_ye_deng_ke", "sui_kuang_gong")
BASE_PARTS = ("head", "body", "rarm", "larm", "rleg", "lleg")
FACES = ("right", "front", "left", "back", "top", "bottom")


def _load_generator(npc_id: str):
    path = NPC_DIR / npc_id / f"gen_{npc_id}_skin.py"
    spec = importlib.util.spec_from_file_location(f"gen_{npc_id}_skin", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class NpcSkinTest(unittest.TestCase):
    def test_skins_are_valid_and_reproducible(self) -> None:
        for npc_id in NPC_IDS:
            with self.subTest(npc=npc_id):
                committed = (NPC_DIR / npc_id / f"{npc_id}_skin.png").read_bytes()
                img = Image.open(io.BytesIO(committed))
                self.assertEqual(img.size, (64, 64))
                self.assertEqual(img.mode, "RGBA")
                pixels = np.asarray(img)
                for part in BASE_PARTS:
                    for face in FACES:
                        x, y, w, h = face_rect(part, face)
                        alpha = pixels[y:y + h, x:x + w, 3]
                        self.assertTrue((alpha == 255).all(), f"{part}.{face} 有透明像素")
                self.assertTrue((pixels[..., 3] < 255).any(), "第二层应有透明区域")
                buf = io.BytesIO()
                _load_generator(npc_id).build().image().save(buf, format="PNG")
                self.assertEqual(buf.getvalue(), committed, "重跑生成器与已提交的 PNG 不一致")

    def test_box_layout_is_vanilla_classic(self) -> None:
        """classic 4px 手臂：两只手臂和两条腿都是 4×12×4，躯干 8×12×4，头 8×8×8。"""
        self.assertEqual(BOXES["rarm"][2:], (4, 12, 4))
        self.assertEqual(BOXES["larm"][2:], (4, 12, 4))
        self.assertEqual(BOXES["body"][2:], (8, 12, 4))
        self.assertEqual(BOXES["head"][2:], (8, 8, 8))


if __name__ == "__main__":
    unittest.main()
