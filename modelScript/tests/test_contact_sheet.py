#!/usr/bin/env python3
"""contact_sheet —— round 2 的人工闸门产物。

工具本身**不做判断**（那是人的活），所以这里测的是「该看的东西有没有被整理到一张图
上、该点的名有没有点齐」：六个视角、上一轮共用同一取景、点名与自证结果落在图上、
以及图上的文字必须是 ASCII（PIL 默认位图字体画不了中文）。
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
for _d in ("generators", "tools"):
    sys.path.insert(0, str(LIB_DIR / _d))

from bbmodel_maker.workbench import contact_sheet as cs  # noqa: E402
from bbmodel_maker.render import framing  # noqa: E402
from bbmodel_maker.gates import manifest as mfmod  # noqa: E402

MODEL = LIB_DIR / "models" / "GrassPouch.bbmodel"
BG = (22, 23, 26)


def _scaled_copy(dst: Path, factor: float) -> Path:
    doc = json.loads(MODEL.read_text())
    for el in doc["elements"]:
        for key in ("from", "to", "origin"):
            el[key] = [8.0 + (v - 8.0) * factor if i != 1 else v * factor
                       for i, v in enumerate(el[key])]
    dst.write_text(json.dumps(doc, ensure_ascii=False))
    return dst


class BuildSheetTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = mfmod.manifest_for(MODEL)

    def test_six_views_in_three_columns_without_a_previous_round(self) -> None:
        sheet = cs.build_sheet(MODEL, manifest=self.manifest, size=80)
        self.assertEqual(3 * 80 + 4 * 12, sheet.width, "六视角排三列")

    def test_previous_round_pairs_up_side_by_side(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            prev = _scaled_copy(Path(tmp) / "Prev.bbmodel", 1.0)
            sheet = cs.build_sheet(MODEL, manifest=self.manifest, prev=prev, size=80)
        self.assertEqual(2 * 80 + 3 * 12, sheet.width,
                         "有上一轮时排两列，NOW / PREV 左右并排才比得动")

    def test_previous_round_shares_the_current_rounds_framing(self) -> None:
        """上一轮各算各的取景 = 这张对比图毫无意义。"""
        with tempfile.TemporaryDirectory() as tmp:
            small = _scaled_copy(Path(tmp) / "Small.bbmodel", 0.5)
            sheet = np.asarray(cs.build_sheet(MODEL, manifest=self.manifest, prev=small,
                                              size=160), int)
        # 切片必须严丝合缝贴住 contact_sheet 的版面（gap=12，label=16，有标题时另加
        # 一条 label+gap 的标题带）：多切一行就把拼版底色圈进来，它离图块底色够远，
        # 整行会被当成前景，两边都量成满宽 —— 测试假绿。
        gap, lab, size = 12, 16, 160
        y = (lab + gap) + gap + lab
        now = sheet[y:y + size, gap:gap + size]
        prev = sheet[y:y + size, gap + size + gap:gap + size + gap + size]

        def width(tile):
            fg = np.abs(tile - np.array(BG)).sum(2) > 12
            xs = np.nonzero(fg.any(0))[0]
            return int(xs.max() - xs.min() + 1) if len(xs) else 0

        w_now, w_prev = width(now), width(prev)
        self.assertGreater(w_now, 0)
        self.assertGreater(w_prev, 0)
        self.assertLess(w_prev, w_now * 0.75,
                        f"缩到一半的上一轮必须**看起来**也小一半（实测 {w_prev} vs {w_now}）"
                        "—— 如果两张一样宽，说明各自被拉满了，对比是假的")

    def test_view_labels_are_honest_about_the_axis_they_show(self) -> None:
        views = framing.views_for(self.manifest.facing)
        self.assertEqual("+z", views[0].shows)
        self.assertIn("FRONT (+z)", views[0].label)

    def test_facing_defaults_to_the_legacy_assumption_without_a_manifest(self) -> None:
        sheet = cs.build_sheet(MODEL, manifest=None, size=64)
        self.assertEqual(3 * 64 + 4 * 12, sheet.width)


class NotesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = mfmod.manifest_for(MODEL)

    def _rollcall(self, mutate=None):
        if mutate is None:
            return mfmod.roll_call(MODEL, self.manifest)
        doc = json.loads(MODEL.read_text())
        mutate(doc)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "Probe.bbmodel"
            path.write_text(json.dumps(doc, ensure_ascii=False))
            return mfmod.roll_call(path, self.manifest)

    def test_clean_manifest_notes_are_a_single_summary_pair(self) -> None:
        lines = cs._manifest_notes(self._rollcall())
        self.assertTrue(lines[0].startswith("MANIFEST |"))
        self.assertIn("9/9", lines[0])
        self.assertEqual([], [ln for ln in lines if ln.startswith("!")])
        self.assertTrue(any(ln.startswith("MATERIALS |") for ln in lines))

    def test_a_missing_feature_gets_its_own_red_line_naming_it(self) -> None:
        rc = self._rollcall(lambda d: d.__setitem__(
            "elements", [e for e in d["elements"] if not e["name"].startswith("strap_")]))
        lines = cs._manifest_notes(rc)
        red = [ln for ln in lines if ln.startswith("!")]
        self.assertTrue(any("shoulder_strap" in ln and "MISSING" in ln for ln in red), red)

    def test_an_unseen_material_gets_a_red_line(self) -> None:
        rc = self._rollcall(lambda d: d.__setitem__(
            "elements", [e for e in d["elements"] if not e["name"].startswith("bone_peg")]))
        self.assertTrue(any("never on camera" in ln and "bone" in ln
                            for ln in cs._manifest_notes(rc)))

    def test_gate_notes_summarise_both_cleanliness_and_discriminating_power(self) -> None:
        import gen_grass_pouch as gp

        buf = _Silence()
        with buf:
            lines = cs._gate_notes(gp.GATES, gp.build())
        self.assertTrue(lines[0].startswith("GATES |"))
        self.assertIn("7/7 clean", lines[0])
        self.assertIn("SELF-TEST 7/7", lines[0],
                      "自证结果必须和「干净」并列写出来 —— 干净但没有鉴别力等于没查")
        self.assertEqual([], [ln for ln in lines if ln.startswith("!")])

    def test_gate_notes_flag_a_gate_without_discriminating_power(self) -> None:
        from bbmodel_maker.gates import gatekit
        import gen_grass_pouch as gp

        blind = gatekit.AssetGates("盲门", gp.MATS, asym=gp.ASYM,
                                   free_floating=frozenset({"sprig_a", "sprig_b"}),
                                   soft_over=gp.SOFT_OVER, seats=gp.SEATS,
                                   seat_materials=frozenset({"seam", "stitch", "bone"}),
                                   interpen_bite=9999.0)
        with _Silence():
            lines = cs._gate_notes(blind, gp.build())
        self.assertTrue(any("FAILED their own injection" in ln for ln in lines), lines)

    def test_every_note_line_is_ascii(self) -> None:
        """图上的字走 PIL 默认字体，非 ASCII 会画成空白 —— 中文表格留给终端。"""
        import gen_grass_pouch as gp

        with _Silence():
            lines = cs._manifest_notes(self._rollcall()) + cs._gate_notes(gp.GATES, gp.build())
        for ln in lines:
            self.assertTrue(ln.isascii(), f"图上的文字必须是 ASCII，这行不是：{ln!r}")


class _Silence:
    """门禁/点名的中文全文是给终端看的，测试里不需要它刷屏。"""

    def __enter__(self):
        import io

        self._old = sys.stdout
        sys.stdout = io.StringIO()
        return self

    def __exit__(self, *exc):
        sys.stdout = self._old
        return False


class CliTest(unittest.TestCase):
    def test_end_to_end_writes_a_png_and_returns_zero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "sheet.png"
            argv = sys.argv
            sys.argv = ["contact_sheet.py", str(MODEL), "--gates", "gen_grass_pouch",
                        "--size", "72", "--out", str(out)]
            try:
                with _Silence():
                    rc = cs.main()
            finally:
                sys.argv = argv
            self.assertEqual(0, rc)
            self.assertTrue(out.is_file())

    def test_no_manifest_flag_skips_the_roll_call(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "sheet.png"
            argv = sys.argv
            sys.argv = ["contact_sheet.py", str(MODEL), "--no-manifest",
                        "--size", "64", "--out", str(out)]
            try:
                with _Silence():
                    rc = cs.main()
            finally:
                sys.argv = argv
            self.assertEqual(0, rc)
            self.assertTrue(out.is_file())

    def test_missing_manifest_fails_loud_instead_of_silently_skipping(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            model = _scaled_copy(Path(tmp) / "NoSheetAsset.bbmodel", 1.0)
            argv = sys.argv
            sys.argv = ["contact_sheet.py", str(model)]
            try:
                with self.assertRaises(FileNotFoundError):
                    with _Silence():
                        cs.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
