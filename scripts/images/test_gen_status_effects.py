from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import gen_status_effects as gse


class GenStatusEffectsTest(unittest.TestCase):
    VALID_CATS = {"dot", "control", "buff", "debuff", "unknown"}

    def test_effects_manifest_unique_nonempty_valid(self) -> None:
        # 母题清单是渲染契约：id 必须唯一、非空、类别合法——重复/空会让某些
        # 效果静默丢图标。
        ids = [e[0] for e in gse.EFFECTS]
        self.assertEqual(len(ids), len(set(ids)),
                         f"EFFECTS 有重复 id: {sorted(x for x in ids if ids.count(x) > 1)}")
        for eid, cat, motif in gse.EFFECTS:
            self.assertTrue(eid and eid.strip(), f"空 id: {eid!r}")
            self.assertIn(cat, self.VALID_CATS, f"{eid} 类别非法: {cat}")
            self.assertGreater(len(motif), 10, f"{eid} 母题过短，难以生成可辨图标")

    def test_build_cmd_uses_gen_py_transparent_emblem_style(self) -> None:
        # 命令构造是纯函数：必须走 gen.py 的 none 画风 + 透明底 + 1024 高清，
        # 并携带 eid 与母题文本。
        cmd = gse.build_cmd("bleeding", "three crimson blood droplets")
        self.assertIn(str(gse.GEN), cmd)
        self.assertIn("--style", cmd)
        self.assertIn("none", cmd)
        self.assertIn("--transparent", cmd)
        self.assertIn("1024x1024", cmd)
        self.assertIn("bleeding", cmd)
        joined = " ".join(cmd)
        self.assertIn("three crimson blood droplets", joined)
        self.assertIn("emblem", joined, "STYLE 前缀应被拼进 prompt")

    def test_gen_one_skips_existing_without_force(self) -> None:
        # 已存在且非 force：返回 skip，且不触发任何子进程（网络）调用。
        with tempfile.TemporaryDirectory() as tmp:
            orig = gse.OUT_DIR
            try:
                gse.OUT_DIR = Path(tmp)
                (gse.OUT_DIR / "bleeding.png").write_bytes(b"existing")
                eid, ok, msg = gse.gen_one("bleeding", "motif", force=False)
            finally:
                gse.OUT_DIR = orig
        self.assertEqual("bleeding", eid)
        self.assertTrue(ok)
        self.assertIn("skip", msg, "已存在文件应返回 skip 而非重生成")

    def test_install_resizes_to_asset_size_and_tracks_missing(self) -> None:
        from PIL import Image

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src, dst = root / "src", root / "dst"
            src.mkdir()
            present = gse.EFFECTS[0][0]
            missing_id = gse.EFFECTS[1][0]
            Image.new("RGBA", (1024, 1024), (10, 20, 30, 255)).save(src / f"{present}.png")

            orig_out, orig_asset = gse.OUT_DIR, gse.ASSET_DIR
            try:
                gse.OUT_DIR, gse.ASSET_DIR = src, dst
                n, missing = gse.install()
            finally:
                gse.OUT_DIR, gse.ASSET_DIR = orig_out, orig_asset

            # Assert inside the with-block: the temp dir must still exist.
            self.assertGreaterEqual(n, 1, "存在的源图应被装入")
            self.assertIn(missing_id, missing, "缺源图的 id 应进 missing 列表")
            out_png = dst / f"{present}.png"
            self.assertTrue(out_png.exists(), "装入的图标文件应存在")
            with Image.open(out_png) as installed:
                self.assertEqual((gse.ASSET_SIZE, gse.ASSET_SIZE), installed.size,
                                 f"资产应缩到 {gse.ASSET_SIZE}²")


if __name__ == "__main__":
    unittest.main()
