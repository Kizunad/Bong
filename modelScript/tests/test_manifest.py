#!/usr/bin/env python3
"""manifest —— 人写的特征清单 + 点名器。

重点不在 happy path。这一组测试大半是**差分自证**：故意把 manifest 点名的东西弄坏，
点名器必须红。只跑「干净模型全绿」的测试，信息量是零 —— 小草包漏背带那两轮，
七道数值门也是全绿的。
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "generators"))

from bbmodel_maker.render import framing  # noqa: E402
from bbmodel_maker.gates import manifest as mf  # noqa: E402

MODEL = LIB_DIR / "models" / "GrassPouch.bbmodel"
SHEET = LIB_DIR / "manifests" / "GrassPouch.manifest.toml"

MIN_DOC = {
    "facing": "+z",
    "features": {"rim": {"must_show_in": ["FRONT"]}},
}


def _doc(**over):
    d = json.loads(json.dumps(MIN_DOC))
    d.update(over)
    return d


def _feature(**over):
    spec = {"must_show_in": ["FRONT"]}
    spec.update(over)
    return _doc(features={"rim": spec})


class ParseTest(unittest.TestCase):
    def test_minimal_manifest_fills_the_documented_defaults(self) -> None:
        m = mf.parse_manifest(MIN_DOC)
        self.assertEqual("+z", m.facing)
        self.assertEqual(0.0, m.mirror_x, "中轴默认在 0（居中建模空间）")
        self.assertEqual(260, m.size)
        self.assertEqual({}, m.materials)
        self.assertEqual(1, len(m.features))
        f = m.features[0]
        self.assertEqual(("FRONT",), f.must_show_in)
        self.assertEqual({"FRONT": 1}, f.min_px)
        self.assertFalse(f.mirror)
        self.assertIsNone(f.asym)

    def test_selector_defaults_to_the_feature_name_as_element_prefix(self) -> None:
        f = mf.parse_manifest(MIN_DOC).features[0]
        self.assertEqual((), f.elements)
        self.assertEqual("elements rim*", f.selector_text(),
                         "不给选件规则时，特征名本身就是件名前缀 —— 这条要写在报告里")

    def test_selector_text_lists_every_rule_in_play(self) -> None:
        f = mf.parse_manifest(_feature(material="cord", elements=["strap_"],
                                       names=["peg_tie"])).features[0]
        self.assertEqual("material cord; elements strap_*; names peg_tie", f.selector_text())

    def test_string_and_list_selectors_both_normalise_to_tuples(self) -> None:
        one = mf.parse_manifest(_feature(elements="strap_")).features[0]
        many = mf.parse_manifest(_feature(elements=["strap_", "knot_"])).features[0]
        self.assertEqual(("strap_",), one.elements)
        self.assertEqual(("strap_", "knot_"), many.elements)

    def test_int_min_px_applies_to_every_listed_view(self) -> None:
        f = mf.parse_manifest(_feature(must_show_in=["FRONT", "SIDE_R"], min_px=300)).features[0]
        self.assertEqual({"FRONT": 300, "SIDE_R": 300}, f.min_px)

    def test_table_min_px_is_per_view(self) -> None:
        f = mf.parse_manifest(_feature(must_show_in=["FRONT", "SIDE_R"],
                                       min_px={"FRONT": 2200, "SIDE_R": 280})).features[0]
        self.assertEqual({"FRONT": 2200, "SIDE_R": 280}, f.min_px)

    def test_table_min_px_missing_a_view_falls_back_to_one(self) -> None:
        f = mf.parse_manifest(_feature(must_show_in=["FRONT", "SIDE_R"],
                                       min_px={"FRONT": 900})).features[0]
        self.assertEqual({"FRONT": 900, "SIDE_R": 1}, f.min_px,
                         "没给门限的视角退到 1 = 只查「露过面」，不是跳过不查")

    def test_view_names_come_back_in_the_six_view_order(self) -> None:
        m = mf.parse_manifest(_doc(features={
            "a": {"must_show_in": ["TOP", "FRONT"]},
            "b": {"must_show_in": ["SIDE_R"]},
        }))
        self.assertEqual(("FRONT", "SIDE_R", "TOP"), m.view_names(),
                         "视角顺序必须稳定，否则报告的列顺序每次都变")

    def test_every_malformed_field_fails_loud(self) -> None:
        cases = [
            ("顶层声明 facing", {"features": MIN_DOC["features"]}),
            ("顶层声明 facing", _doc(facing=3)),
            ("未知朝向", _doc(facing="front")),
            ("mirror_x", _doc(mirror_x="八")),
            ("mirror_x", _doc(mirror_x=True)),
            ("size", _doc(size=16)),
            ("size", _doc(size=True)),
            ("materials", _doc(materials=["weave"])),
            ("颜色必须是三个", _doc(materials={"weave": [1, 2]})),
            ("颜色必须是三个", _doc(materials={"weave": [1, 2, 300]})),
            ("颜色必须是三个", _doc(materials={"weave": [1, 2, "x"]})),
            ("features", {"facing": "+z", "features": []}),
            ("一个特征都没有", {"facing": "+z", "features": {}}),
            ("必须是一张表", _doc(features={"rim": "braid"})),
            ("未知字段", _feature(min_pixels=5)),
            ("must_show_in", _doc(features={"rim": {}})),
            ("min_px", _feature(min_px=0)),
            ("min_px", _feature(min_px=True)),
            ("min_px", _feature(min_px="200")),
            ("must_show_in 里没有这些视角", _feature(min_px={"BACK": 10})),
            ("min_px", _feature(min_px={"FRONT": 0})),
            ("asym", _feature(asym="middle")),
            ("mirror", _feature(mirror="yes")),
            ("自相矛盾", _feature(mirror=True, asym="right")),
            ("must_show_in", _feature(must_show_in=7)),
        ]
        for needle, doc in cases:
            with self.subTest(needle=needle, doc=doc):
                with self.assertRaises(ValueError) as ctx:
                    mf.parse_manifest(doc)
                self.assertIn(needle, str(ctx.exception))

    def test_unknown_view_name_is_caught_at_parse_time_not_after_rendering(self) -> None:
        with self.assertRaises(KeyError):
            mf.parse_manifest(_feature(must_show_in=["SIDE"]))

    def test_missing_file_says_the_clist_must_be_human_written(self) -> None:
        with self.assertRaises(FileNotFoundError) as ctx:
            mf.load_manifest(LIB_DIR / "manifests" / "NoSuchAsset.manifest.toml")
        self.assertIn("清单必须人写", str(ctx.exception),
                      "报错要顶住「那就自动生成一份吧」这个念头")

    def test_manifest_for_resolves_by_model_stem(self) -> None:
        m = mf.manifest_for(MODEL)
        self.assertEqual(SHEET, m.source)


class HueTest(unittest.TestCase):
    PALETTE = {"weave": (152, 136, 102), "stitch": (72, 56, 42), "bone": (206, 198, 170)}

    def test_exact_colour_matches_itself(self) -> None:
        for name, rgb in self.PALETTE.items():
            self.assertEqual(name, mf.hue_match(rgb, self.PALETTE))

    def test_classification_survives_a_three_times_brightness_swing(self) -> None:
        """这就是用色相不用绝对亮度的全部理由：lambert 把背向面夹到 0.32、顶面给到 1.0。"""
        for name, rgb in self.PALETTE.items():
            for shade in (0.32, 0.68, 1.0):
                dim = tuple(c * shade for c in rgb)
                self.assertEqual(name, mf.hue_match(dim, self.PALETTE),
                                 f"{name} 在 {shade:.2f} 档亮度下应仍判为自己")

    def test_far_hue_matches_nothing(self) -> None:
        self.assertIsNone(mf.hue_match((30, 200, 40), self.PALETTE),
                          "绿色不该被归进任何一种枯草黄")

    def test_tolerance_is_the_knob(self) -> None:
        near = (152, 150, 102)         # 比 weave 绿一点
        self.assertIsNone(mf.hue_match(near, self.PALETTE, tol=0.001))
        self.assertEqual("weave", mf.hue_match(near, self.PALETTE, tol=0.5))

    def test_empty_palette_matches_nothing_instead_of_crashing(self) -> None:
        self.assertIsNone(mf.hue_match((10, 10, 10), {}))
        self.assertEqual({}, mf.hue_counts(np.zeros((4, 4, 3), np.uint8), {}))

    def test_counts_partition_the_image(self) -> None:
        img = np.zeros((2, 3, 3), np.uint8)
        img[0, :] = self.PALETTE["weave"]
        img[1, :2] = self.PALETTE["bone"]
        img[1, 2] = (30, 200, 40)
        counts = mf.hue_counts(img, self.PALETTE)
        self.assertEqual({"weave": 3, "stitch": 0, "bone": 2}, counts)
        self.assertEqual(5, sum(counts.values()), "第 6 个像素是绿的，不属于任何材质")


class ElementMaterialTest(unittest.TestCase):
    def test_uv_sampled_materials_agree_with_the_generator_colour_index(self) -> None:
        """UV 采样判材质必须和生成器自己的 color 索引逐件对上，否则选件规则就是瞎的。"""
        import gen_grass_pouch as gp

        rig = gp.build()
        expect = {e["name"]: gp.MATS_BY_COLOR[e["color"]] for e in rig.elements}
        got = mf.element_materials(MODEL, dict(gp.MATS))
        self.assertEqual(set(expect), set(got), "件名集合必须一致")
        wrong = {n: (expect[n], got[n]) for n in expect if expect[n] != got[n]}
        self.assertEqual({}, wrong, f"这些件的材质判错了（期望, 实际）：{wrong}")

    def test_manifest_palette_matches_the_generator_source_of_truth(self) -> None:
        import gen_grass_pouch as gp

        self.assertEqual({k: tuple(v) for k, v in gp.MATS.items()},
                         mf.manifest_for(MODEL).materials,
                         "清单里的 [materials] 与 gen_grass_pouch.MATS 漂了 —— 改一边就得改另一边")


class SelectTest(unittest.TestCase):
    def setUp(self) -> None:
        self.doc = json.loads(MODEL.read_text())
        import gen_grass_pouch as gp

        self.mat_of = mf.element_materials(MODEL, dict(gp.MATS))

    def _names(self, **spec):
        feat = mf.parse_manifest(_feature(**spec)).features[0]
        return sorted(e["name"] for e in mf.select_elements(self.doc, feat, self.mat_of))

    def test_prefix_selector(self) -> None:
        self.assertEqual(["bone_peg_01", "bone_peg_02", "bone_peg_03"],
                         self._names(elements=["bone_peg"]))

    def test_exact_name_selector(self) -> None:
        self.assertEqual(["peg_tie"], self._names(names=["peg_tie"]))

    def test_material_selector(self) -> None:
        self.assertEqual(["bone_peg_01", "bone_peg_02", "bone_peg_03"],
                         self._names(material="bone"))

    def test_rules_union_without_duplicates(self) -> None:
        got = self._names(elements=["bone_peg"], names=["peg_tie"], material="bone")
        self.assertEqual(["bone_peg_01", "bone_peg_02", "bone_peg_03", "peg_tie"], got)

    def test_unmatched_selector_selects_nothing_rather_than_everything(self) -> None:
        self.assertEqual([], self._names(elements=["no_such_prefix_"]))


class RollCallTest(unittest.TestCase):
    """点名器的真实回归 + 差分自证。"""

    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = mf.manifest_for(MODEL)
        cls.doc = json.loads(MODEL.read_text())

    def _variant(self, mutate, manifest=None):
        """把改过的模型写进临时文件再点名。mutate 收 doc，就地改。"""
        doc = json.loads(json.dumps(self.doc))
        mutate(doc)
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "GrassPouch.bbmodel"
            path.write_text(json.dumps(doc, ensure_ascii=False))
            return mf.roll_call(path, manifest or self.manifest)

    def _verdict(self, rc, key):
        return next(v for v in rc.verdicts if v.key == key)

    def test_the_shipped_model_passes_its_own_manifest(self) -> None:
        rc = mf.roll_call(MODEL, self.manifest)
        self.assertTrue(rc.ok, f"小草包应通过自己的清单，实际缺项：{rc.missing}")
        self.assertEqual((), rc.missing)
        self.assertEqual(("FRONT", "SIDE_R"), rc.views)
        self.assertEqual(len(self.manifest.features), len(rc.verdicts))

    def test_every_material_is_on_camera(self) -> None:
        rc = mf.roll_call(MODEL, self.manifest)
        self.assertEqual((), rc.unseen_materials(),
                         "七种材质都该在某张图上露过面")
        for view in rc.views:
            self.assertGreater(sum(rc.census[view].values()), 0)

    def test_deleting_the_straps_is_caught_as_a_whole_missing_feature(self) -> None:
        """差分自证之一：整件删掉 —— 正是小草包前两轮真实犯过的那个错。"""
        rc = self._variant(lambda d: d.__setitem__(
            "elements", [e for e in d["elements"] if not e["name"].startswith("strap_")]))
        v = self._verdict(rc, "shoulder_strap")
        self.assertFalse(rc.ok)
        self.assertIn("shoulder_strap", rc.missing)
        self.assertEqual(0, v.count)
        self.assertTrue(any("整件缺席" in p for p in v.problems),
                        f"选不中任何件时必须报「整件缺席」，实际：{v.problems}")

    def test_deleting_only_the_left_straps_is_caught_by_mirror(self) -> None:
        """半边缺失比整件缺失更阴险：像素数还很可观，全靠 mirror 这一条抓。"""
        rc = self._variant(lambda d: d.__setitem__(
            "elements", [e for e in d["elements"]
                         if not (e["name"].startswith("strap_") and "_l" in e["name"])]))
        v = self._verdict(rc, "shoulder_strap")
        self.assertGreater(v.count, 0, "右侧带子还在，件数不为零")
        floor = next(f for f in self.manifest.features
                     if f.key == "shoulder_strap").min_px["FRONT"]
        self.assertGreater(v.pixels["FRONT"], floor // 2,
                           "像素门限拦不住半边缺失 —— 这正是需要 mirror 的理由")
        self.assertTrue(any("左右成对" in p for p in v.problems), v.problems)

    def test_side_pocket_moved_to_the_left_is_caught_by_asym(self) -> None:
        def flip(d):
            for e in d["elements"]:
                if e["name"].startswith(("pocket_", "sprig_")):
                    for k in ("from", "to", "origin"):
                        e[k][0] = 16.0 - e[k][0]
                    e["from"][0], e["to"][0] = min(e["from"][0], e["to"][0]), max(e["from"][0], e["to"][0])

        rc = self._variant(flip)
        v = self._verdict(rc, "side_pocket")
        self.assertTrue(any("只在右侧" in p for p in v.problems),
                        f"侧插袋镜像到左边必须撞红（右 = FRONT 视观者右手边 = +x），实际：{v.problems}")

    def test_a_feature_buried_behind_another_part_trips_min_px(self) -> None:
        """骨扣被前檐吞掉那次的复现：件都在、位置也合法，就是看不见。"""
        def bury(d):
            for e in d["elements"]:
                if e["name"].startswith("bone_peg"):
                    for k in ("from", "to", "origin"):
                        e[k][2] -= 1.6           # 整颗骨扣缩回檐后

        rc = self._variant(bury)
        v = self._verdict(rc, "bone_toggle")
        self.assertEqual(3, v.count, "件一个没少")
        self.assertTrue(any("只露" in p for p in v.problems),
                        f"藏进檐后必须撞 min_px，实际 {v.pixels} / {v.problems}")

    def test_min_px_floor_is_what_decides_not_the_pixel_count_itself(self) -> None:
        """把门限抬到天上，同一个干净模型必须红 —— 证明门限真的在起作用。"""
        doc = json.loads(json.dumps({
            "facing": "+z", "mirror_x": 8.0,
            "features": {"bone_toggle": {"elements": ["bone_peg"],
                                         "must_show_in": ["FRONT"],
                                         "min_px": 99999}},
        }))
        rc = mf.roll_call(MODEL, mf.parse_manifest(doc))
        self.assertFalse(rc.ok)
        self.assertIn("99999", "".join(self._verdict(rc, "bone_toggle").problems))

    def test_removing_a_material_shows_up_in_the_census(self) -> None:
        rc = self._variant(lambda d: d.__setitem__(
            "elements", [e for e in d["elements"] if not e["name"].startswith("bone_peg")]))
        self.assertIn("bone", rc.unseen_materials(),
                      "骨扣是唯一的 bone 件，删掉后材质普查必须报 bone 一次都没上镜")

    def test_a_material_that_never_shows_counts_as_a_missing_item(self) -> None:
        """回归（Kody #2104 抓到的）：未上镜的材质早就被 `lines()` 打成 `!` 红行，
        但 `ok` / `report()` 不算它 —— 于是「报告是红的、退出码是 0」，
        正是这套工具要治的那种假绿。

        这里所有特征都过，只有一种清单里声明、模型里根本没有的材质缺席。
        """
        doc = json.loads(json.dumps({
            "facing": "+z", "mirror_x": 8.0,
            "materials": {**{k: list(v) for k, v in self.manifest.materials.items()},
                          "ghost": [10, 250, 10]},
            "features": {"bone_toggle": {"elements": ["bone_peg"],
                                         "must_show_in": ["FRONT"], "min_px": 100}},
        }))
        rc = mf.roll_call(MODEL, mf.parse_manifest(doc))
        self.assertEqual((), rc.missing, "特征一条不缺")
        self.assertEqual(("ghost",), rc.unseen_materials())
        self.assertFalse(rc.ok, "未上镜的材质必须算缺项，否则报告红而退出码绿")
        self.assertEqual(1, rc.report(), "report() 的返回值就是 CLI 的退出码依据")
        red = [ln for ln in rc.lines() if ln.startswith("!")]
        self.assertTrue(any("ghost" in ln for ln in red))
        self.assertTrue(any("材质没上镜" in ln for ln in red),
                        f"总结行必须把材质缺席也算进去并标红：{red}")

    def test_a_feature_covering_the_whole_model_reports_the_silhouette(self) -> None:
        """边界：抽光了没法渲，得退回整幅剪影而不是崩掉。"""
        doc = {"facing": "+z", "features": {"everything": {"elements": [""],
                                                           "must_show_in": ["FRONT"]}}}
        rc = mf.roll_call(MODEL, mf.parse_manifest(doc))
        v = self._verdict(rc, "everything")
        self.assertEqual(len(self.doc["elements"]), v.count)
        self.assertGreater(v.pixels["FRONT"], 1000)
        self.assertTrue(rc.ok)

    def test_keep_images_hands_back_one_render_per_view(self) -> None:
        keep: dict = {}
        mf.roll_call(MODEL, self.manifest, keep_images=keep)
        self.assertEqual({"FRONT", "SIDE_R"}, set(keep))
        for img in keep.values():
            self.assertEqual((self.manifest.size, self.manifest.size), img.size)

    def test_size_override_changes_the_pixel_scale(self) -> None:
        small = mf.roll_call(MODEL, self.manifest, size=120)
        big = mf.roll_call(MODEL, self.manifest, size=260)
        s = self._verdict(small, "woven_body").pixels["FRONT"]
        b = self._verdict(big, "woven_body").pixels["FRONT"]
        self.assertLess(s, b, "像素数按面积缩放 —— 所以门限必须和 size 锁在一起")

    def test_report_lines_flag_failures_for_the_contact_sheet(self) -> None:
        rc = self._variant(lambda d: d.__setitem__(
            "elements", [e for e in d["elements"] if not e["name"].startswith("strap_")]))
        lines = rc.lines()
        self.assertTrue(lines[0].startswith("点名 "))
        bad = [ln for ln in lines if ln.startswith("!")]
        self.assertTrue(any("shoulder_strap" in ln for ln in bad))
        self.assertTrue(any("处缺项" in ln for ln in bad),
                        "总结行在有缺项时也必须标红")

        clean = mf.roll_call(MODEL, self.manifest).lines()
        self.assertEqual([], [ln for ln in clean if ln.startswith("!")],
                         "干净模型一行红都不该有")

    def test_verdicts_carry_the_selector_so_a_miss_is_debuggable(self) -> None:
        rc = mf.roll_call(MODEL, self.manifest)
        self.assertEqual("elements strap_*", self._verdict(rc, "shoulder_strap").selector)


class CliTest(unittest.TestCase):
    def test_cli_returns_one_when_only_a_material_is_off_camera(self) -> None:
        import tomllib

        doc = tomllib.loads(SHEET.read_bytes().decode())
        doc["materials"]["ghost"] = [10, 250, 10]
        with tempfile.TemporaryDirectory() as tmp:
            sheet = Path(tmp) / "Probe.manifest.toml"
            sheet.write_text("\n".join(
                ['facing = "+z"', "mirror_x = 8.0", f"size = {doc['size']}", "", "[materials]"]
                + [f"{k} = {list(v)}" for k, v in doc["materials"].items()]
                + ["", "[features]",
                   'bone_toggle = { elements = ["bone_peg"], must_show_in = ["FRONT"], min_px = 100 }']
            ))
            argv = sys.argv
            sys.argv = ["manifest.py", str(MODEL), "--manifest", str(sheet)]
            try:
                self.assertEqual(1, mf.main(),
                                 "只有材质缺席时 CLI 也必须非零退出 —— 报告是红的，"
                                 "退出码不能是绿的")
            finally:
                sys.argv = argv

    def test_cli_returns_zero_on_a_clean_asset(self) -> None:
        argv = sys.argv
        sys.argv = ["manifest.py", str(MODEL)]
        try:
            self.assertEqual(0, mf.main())
        finally:
            sys.argv = argv


if __name__ == "__main__":
    unittest.main()
