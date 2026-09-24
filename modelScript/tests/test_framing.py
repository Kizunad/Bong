#!/usr/bin/env python3
"""framing —— 诚实视角命名 + 固定取景。

这一组测试盯死两个实测踩过的坑：
  1. 视角标签和实际照到的轴面必须对得上（yaw=180 照的是 −z，不是「正面」这个词本身）；
  2. 同一批图必须共用取景，否则跨图/跨轮的一切比较都是噪声。
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]

from bbmodel_maker.render import framing  # noqa: E402
from bbmodel_maker.render import render_bbmodel  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

BG = (22, 23, 26)


def _model(tmp: Path, name: str, boxes) -> Path:
    """写一个最小 bbmodel：boxes = [(件名, from, to, 材质)]。"""
    rig = Rig({"red": (200, 60, 60), "blue": (60, 90, 200)})
    rig.bone("root", (0.0, 0.0, 0.0))
    for n, frm, to, mat in boxes:
        rig.cube("root", n, frm, to, mat=mat)
    path = tmp / f"{name}.bbmodel"
    path.write_text(json.dumps(rig.bbmodel(name), ensure_ascii=False))
    return path


def _silhouette(img):
    """前景像素的 (宽, 高)，单位像素。背景是纯 BG。"""
    a = np.asarray(img, int)
    fg = np.abs(a - np.array(BG)).sum(2) > 12
    if not fg.any():
        return (0, 0)
    ys, xs = np.nonzero(fg)
    return (int(xs.max() - xs.min() + 1), int(ys.max() - ys.min() + 1))


class YawTableTest(unittest.TestCase):
    """yaw_for_normal 是整个模块的地基，记反一个符号左右两侧就全镜像了。"""

    def test_axis_normals_map_to_the_yaw_that_actually_shows_them(self) -> None:
        table = {(0, 0, 1): 0.0, (0, 0, -1): 180.0, (1, 0, 0): 270.0, (-1, 0, 0): 90.0}
        for n, expect in table.items():
            self.assertAlmostEqual(
                expect, framing.yaw_for_normal(n), places=6,
                msg=f"法线 {n} 应在 yaw={expect} 正对镜头，因为渲染器只画 R@n 的 z>0.02 的面",
            )

    def test_plus_x_is_270_not_90_because_that_pair_is_the_classic_mixup(self) -> None:
        self.assertNotAlmostEqual(
            90.0, framing.yaw_for_normal((1, 0, 0)),
            msg="+x 面在 yaw=90 时 z' = -sin(90) = -1，被背面剔除；记成 90 会让 SIDE_L/R 整体镜像",
        )

    def test_every_named_view_actually_survives_backface_culling(self) -> None:
        """核心诚实性断言：View.shows 声称的那个面必须真的可见，反面必须真的被剔除。"""
        for facing in framing.FACING_NORMALS:
            for view in framing.views_for(facing, ("FRONT", "BACK", "SIDE_L", "SIDE_R", "TOP")):
                n = np.array(framing.FACING_NORMALS.get(view.shows, (0.0, 1.0, 0.0)), float)
                z = float((framing.view_matrix(view) @ n)[2])
                self.assertGreater(
                    z, 0.02,
                    f"facing={facing} 的 {view.name} 声称照到 {view.shows}，"
                    f"但该面法线转到相机空间后 z={z:.3f} ≤ 0.02（会被背面剔除）",
                )
                back = float((framing.view_matrix(view) @ (-n))[2])
                self.assertLessEqual(
                    back, 0.02,
                    f"facing={facing} 的 {view.name} 不应同时看见 {view.shows} 的反面（z={back:.3f}）",
                )

    def test_non_unit_input_is_normalised(self) -> None:
        self.assertAlmostEqual(180.0, framing.yaw_for_normal((0, 0, -7.5)), places=6)

    def test_zero_vector_and_tilted_normals_fail_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "零向量"):
            framing.yaw_for_normal((0, 0, 0))
        with self.assertRaisesRegex(ValueError, "y 分量"):
            framing.yaw_for_normal((0, 1, 0))
        with self.assertRaisesRegex(ValueError, "三元组"):
            framing.yaw_for_normal((0, 1))


class FacingTest(unittest.TestCase):
    def test_four_axis_facings_round_trip(self) -> None:
        for key, normal in framing.FACING_NORMALS.items():
            self.assertEqual(normal, framing.parse_facing(key))
            self.assertEqual(normal, framing.parse_facing(f"  {key} "), "两侧空白应被容忍")

    def test_unknown_facing_names_the_legal_set(self) -> None:
        with self.assertRaises(ValueError) as ctx:
            framing.parse_facing("front")
        self.assertIn("+z", str(ctx.exception), "报错必须把合法取值列出来，否则得去翻源码")

    def test_right_is_up_cross_facing_matching_the_screen_right_convention(self) -> None:
        # facing=+z 时 FRONT 是 yaw=0，屏幕右方向 = R⁻¹·(1,0,0) = (cos0,0,sin0) = +x。
        # 小草包的侧插袋写在 +x、作者称「右侧」，这条约定就是照它定的。
        self.assertEqual((1.0, 0.0, 0.0), framing.right_normal("+z"))
        self.assertEqual((-1.0, 0.0, 0.0), framing.right_normal("-z"))
        self.assertEqual((0.0, 0.0, -1.0), framing.right_normal("+x"))
        self.assertEqual((0.0, 0.0, 1.0), framing.right_normal("-x"))


class ViewsForTest(unittest.TestCase):
    def test_legacy_minus_z_reproduces_the_historical_three_view_angles(self) -> None:
        """默认 facing 换掉 = 悄悄把所有历史预览图的语义翻面。角度必须一个不动。"""
        views = framing.views_for(framing.LEGACY_FACING, ("FRONT", "SIDE_R", "3/4"))
        self.assertEqual(
            [("FRONT", 180.0, 0.0), ("SIDE_R", 90.0, 0.0), ("3/4", 145.0, 15.0)],
            [(v.name, v.yaw, v.pitch) for v in views],
            "framing 派生的三视图角度必须与历史 THREE_VIEW_ANGLES 完全一致",
        )
        self.assertEqual(
            [(v.name, v.yaw, v.pitch) for v in views],
            [tuple(a) for a in render_bbmodel.THREE_VIEW_ANGLES],
            "render_bbmodel 的三视图表必须就是从 framing 派生的这一份",
        )

    def test_front_and_back_are_opposite_for_every_facing(self) -> None:
        for facing in framing.FACING_NORMALS:
            f, b = framing.views_for(facing, ("FRONT", "BACK"))
            self.assertAlmostEqual((f.yaw + 180.0) % 360.0, b.yaw, places=6,
                                   msg=f"{facing}: BACK 必须正好在 FRONT 对面")
            self.assertEqual(framing.FACING_NORMALS[facing],
                             framing.parse_facing(f.shows),
                             f"{facing}: FRONT 照到的必须就是声明的那个面")

    def test_side_l_and_side_r_are_opposite_and_track_the_facing(self) -> None:
        for facing in framing.FACING_NORMALS:
            r, l_ = framing.views_for(facing, ("SIDE_R", "SIDE_L"))
            self.assertAlmostEqual((r.yaw + 180.0) % 360.0, l_.yaw, places=6)
            self.assertEqual(framing.right_normal(facing), framing.parse_facing(r.shows))

    def test_top_is_a_true_overhead_and_three_quarter_is_front_minus_35(self) -> None:
        for facing in framing.FACING_NORMALS:
            front, top, tq = framing.views_for(facing, ("FRONT", "TOP", "3/4"))
            self.assertEqual(90.0, top.pitch, "TOP 必须是真俯视（正 pitch 俯视，90° 正对 +y）")
            self.assertEqual("+y", top.shows)
            self.assertAlmostEqual((front.yaw - 35.0) % 360.0, tq.yaw, places=6,
                                   msg="3/4 是相对 FRONT 的偏移，不是硬编绝对角")
            self.assertEqual(15.0, tq.pitch)
            self.assertEqual("", tq.shows, "斜视角没有单一轴面，shows 必须留空而不是瞎猜一个")

    def test_six_views_default_order_is_stable(self) -> None:
        self.assertEqual(list(framing.SIX_VIEWS),
                         [v.name for v in framing.views_for("+z")])

    def test_empty_name_list_yields_no_views(self) -> None:
        self.assertEqual((), framing.views_for("+z", ()))

    def test_unknown_view_name_fails_loud(self) -> None:
        with self.assertRaises(KeyError) as ctx:
            framing.views_for("+z", ("SIDE",))
        self.assertIn("SIDE_R", str(ctx.exception), "报错要把可选名字列出来")

    def test_view_by_name_is_the_single_view_shortcut(self) -> None:
        self.assertEqual(framing.views_for("+z", ("SIDE_R",))[0],
                         framing.view_by_name("+z", "SIDE_R"))

    def test_label_carries_both_the_semantic_name_and_the_real_axis(self) -> None:
        v = framing.view_by_name("+z", "FRONT")
        self.assertEqual("FRONT (+z) yaw=0 pitch=0", v.label)
        self.assertEqual("3/4 yaw=325 pitch=15", framing.view_by_name("+z", "3/4").label,
                         "斜视角没有轴面可报，就不要凭空括一个上去")
        self.assertTrue(v.label.isascii(), "标签走 PIL 默认位图字体，非 ASCII 会画成空白")


class ViewMatrixTest(unittest.TestCase):
    def test_matches_the_matrix_render_actually_uses(self) -> None:
        """两处各写一遍视图矩阵迟早会漂，这条断言就是那根绳子。"""
        for yaw, pitch in ((0.0, 0.0), (145.0, 15.0), (270.0, 90.0), (-35.0, 22.0)):
            expect = render_bbmodel._rotmat(pitch, 0) @ render_bbmodel._rotmat(yaw, 1)
            got = framing.view_matrix(framing.View("X", yaw, pitch, ""))
            self.assertTrue(np.allclose(expect, got, atol=1e-12),
                            f"yaw={yaw} pitch={pitch} 的视图矩阵与 render() 内部不一致")


class FocusTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        tmp = Path(self.tmp.name)
        # 8 宽 × 2 高 × 2 深的扁盒：自动取景下正视/侧视会被各自拉满，固定取景下不会。
        self.slab = _model(tmp, "Slab", [("slab", (-4, 0, -1), (4, 2, 1), "red")])

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_bounds_are_the_real_geometry(self) -> None:
        lo, hi = framing.model_bounds(self.slab)
        self.assertTrue(np.allclose(lo, (-4, 0, -1)))
        self.assertTrue(np.allclose(hi, (4, 2, 1)))

    def test_focus_centres_on_the_bbox_and_covers_every_requested_view(self) -> None:
        views = framing.views_for("+z")
        center, span = framing.focus_for(self.slab, views, margin=1.0)
        self.assertTrue(np.allclose(center, (0.0, 1.0, 0.0)),
                        "取景中心必须是包围盒中心，否则各图的模型位置对不齐")
        lo, hi = framing.model_bounds(self.slab)
        corners = np.array([[lo[0] if i & 1 else hi[0], lo[1] if i & 2 else hi[1],
                             lo[2] if i & 4 else hi[2]] for i in range(8)]) - center
        for v in views:
            p = (framing.view_matrix(v) @ corners.T).T[:, :2]
            self.assertLessEqual(float((p.max(0) - p.min(0)).max()), span + 1e-9,
                                 f"{v.name} 的投影跨度超过公共 span，会被裁掉")

    def test_span_is_the_widest_view_not_the_diagonal(self) -> None:
        views = framing.views_for("+z", ("FRONT",))
        _, span = framing.focus_for(self.slab, views, margin=1.0)
        self.assertAlmostEqual(8.0, span, places=6,
                               msg="只要 FRONT 时 span 就该是 8（x 跨度），取对角线会白白浪费一半画布")

    def test_margin_scales_the_span_and_defaults_to_a_little_slack(self) -> None:
        views = framing.views_for("+z", ("FRONT",))
        _, tight = framing.focus_for(self.slab, views, margin=1.0)
        _, loose = framing.focus_for(self.slab, views, margin=2.0)
        self.assertAlmostEqual(2.0, loose / tight, places=6)
        _, default = framing.focus_for(self.slab, views)
        self.assertGreater(default, tight, "默认应留一点白，贴边图在缩略图里会被误读成顶出去了")

    def test_focus_is_deterministic(self) -> None:
        views = framing.views_for("-z")
        self.assertEqual(framing.focus_for(self.slab, views),
                         framing.focus_for(self.slab, views))

    def test_no_views_fails_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "没有视角"):
            framing.focus_for(self.slab, ())

    def test_model_without_faces_fails_loud(self) -> None:
        with tempfile.TemporaryDirectory() as t:
            path = Path(t) / "Empty.bbmodel"
            doc = json.loads(self.slab.read_text())
            doc["elements"] = []
            path.write_text(json.dumps(doc))
            with self.assertRaisesRegex(ValueError, "没有可渲染的面"):
                framing.model_bounds(path)


class RenderViewsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.slab = _model(Path(self.tmp.name), "Slab",
                           [("slab", (-4, 0, -1), (4, 2, 1), "red")])

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_fixed_focus_keeps_scale_identical_across_views(self) -> None:
        """这条是整个模块的存在理由：换个角度看，尺子不许跟着换。"""
        views = framing.views_for("+z", ("FRONT", "SIDE_R"))
        focus = framing.focus_for(self.slab, views, margin=1.0)
        tiles = dict(framing.render_views(self.slab, views, focus=focus, size=240))
        front_w = _silhouette(tiles[views[0]])[0]
        side_w = _silhouette(tiles[views[1]])[0]
        self.assertAlmostEqual(
            2.0 / 8.0, side_w / front_w, delta=0.03,
            msg=f"固定取景下侧视宽应是正视的 2/8（几何真实比例），实测 {side_w}/{front_w}",
        )

    def test_auto_framing_would_have_lied_which_is_why_focus_is_mandatory(self) -> None:
        """反证：不固定取景时两张图会被各自拉满，宽度比退化成 ~1。"""
        from bbmodel_maker.render.render_bbmodel import render

        views = framing.views_for("+z", ("FRONT", "SIDE_R"))
        widths = [_silhouette(render(self.slab, yaw=v.yaw, pitch=v.pitch, size=240)[0])[0]
                  for v in views]
        self.assertAlmostEqual(1.0, widths[1] / widths[0], delta=0.05,
                               msg="自动取景确实把两张图都拉满了 —— 所以它不能用来做跨图比较")

    def test_render_views_defaults_to_the_shared_focus(self) -> None:
        views = framing.views_for("-z", ("FRONT", "TOP"))
        auto = framing.render_views(self.slab, views, size=160)
        explicit = framing.render_views(self.slab, views, size=160,
                                        focus=framing.focus_for(self.slab, views))
        for (va, ia), (vb, ib) in zip(auto, explicit):
            self.assertEqual(va, vb)
            self.assertEqual(ia.tobytes(), ib.tobytes(),
                             "缺省 focus 必须等于 focus_for 的结果，绝不能退回逐图自动取景")

    def test_one_image_per_view_at_the_requested_size(self) -> None:
        views = framing.views_for("+x")
        out = framing.render_views(self.slab, views, size=96)
        self.assertEqual(len(views), len(out))
        for v, im in out:
            self.assertEqual((96, 96), im.size)
            self.assertIn(v, views)

    def test_zero_views_renders_nothing(self) -> None:
        self.assertEqual([], framing.render_views(self.slab, (), focus=((0, 0, 0), 10.0)))


class ContactSheetTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.slab = _model(Path(self.tmp.name), "Slab",
                           [("slab", (-4, 0, -1), (4, 2, 1), "red")])
        self.tiles = [(v.label, im) for v, im in
                      framing.render_views(self.slab, framing.views_for("+z"), size=64)]

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_grid_geometry_follows_columns(self) -> None:
        two = framing.contact_sheet(self.tiles, columns=2)
        three = framing.contact_sheet(self.tiles, columns=3)
        self.assertEqual(2 * 64 + 3 * 12, two.width)
        self.assertEqual(3 * 64 + 4 * 12, three.width)
        self.assertGreater(two.height, three.height, "列少了行就多，高度必须跟着长")

    def test_more_columns_than_tiles_does_not_leave_empty_columns(self) -> None:
        one = framing.contact_sheet(self.tiles[:1], columns=6)
        self.assertEqual(64 + 2 * 12, one.width, "只有一张图时不该按 6 列撑出一片空画布")

    def test_title_and_notes_add_bands(self) -> None:
        bare = framing.contact_sheet(self.tiles, columns=3)
        titled = framing.contact_sheet(self.tiles, columns=3, title="GrassPouch round 2")
        noted = framing.contact_sheet(self.tiles, columns=3, notes=["a", "b", "c"])
        self.assertGreater(titled.height, bare.height)
        self.assertGreater(noted.height, bare.height)

    def test_bang_prefixed_notes_are_painted_in_the_warn_colour(self) -> None:
        # PIL 新版默认字体是抗锯齿 TTF，落盘像素是 warn 与底色的混合，不会有一个像素
        # 严格等于 warn。判据取「离 warn 足够近」，并且只在**注记带**里量 —— 图块本身
        # 就是红方块，拿整张图找红色会永远为真。
        warn = (232, 120, 96)
        band = 26
        hot = framing.contact_sheet(self.tiles[:1], notes=["! shoulder_strap 缺席"], warn=warn)
        calm = framing.contact_sheet(self.tiles[:1], notes=["shoulder_strap ok"], warn=warn)
        self.assertEqual(hot.size, calm.size, "同样一行字，标不标红不该改变版面")

        def closest(img):
            a = np.asarray(img, int)[-band:]
            return int(np.abs(a - np.array(warn)).sum(2).min())

        self.assertLess(closest(hot), 40,
                        "以 ! 开头的注记必须真的画成警示色 —— 三十秒扫一眼靠的就是颜色")
        self.assertGreater(closest(calm), 120,
                           "普通注记不许染成警示色，否则红色就失去信息量了")

    def test_empty_and_bad_columns_fail_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "没有图"):
            framing.contact_sheet([])
        with self.assertRaisesRegex(ValueError, "columns"):
            framing.contact_sheet(self.tiles, columns=0)


class ThreeViewCompatTest(unittest.TestCase):
    """6 个既有调用点都是 `render_three_view(path, size=...)`，签名不许动。"""

    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.slab = _model(Path(self.tmp.name), "Slab",
                           [("slab", (-4, 0, -1), (4, 2, 1), "red")])

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def test_positional_path_and_size_keyword_still_work(self) -> None:
        img, name = render_bbmodel.render_three_view(self.slab, size=100)
        self.assertEqual("Slab", name)
        self.assertEqual(3 * 100 + 4 * 12, img.width)

    def test_facing_switch_changes_the_angles_not_the_signature(self) -> None:
        default = framing.views_for(framing.LEGACY_FACING, render_bbmodel.THREE_VIEW_NAMES)
        plus_z = framing.views_for("+z", render_bbmodel.THREE_VIEW_NAMES)
        self.assertNotEqual([v.yaw for v in default], [v.yaw for v in plus_z])
        img, _ = render_bbmodel.render_three_view(self.slab, size=64, facing="+z")
        self.assertEqual(3 * 64 + 4 * 12, img.width)

    def test_three_view_tiles_share_one_focus(self) -> None:
        """三视图内部也不许各拉各的 —— 侧视的 2/8 比例必须在拼版里保住。"""
        img = np.asarray(render_bbmodel.render_three_view(self.slab, size=200, facing="+z")[0], int)
        # 切片必须**严丝合缝**贴住 render_three_view 的拼版（gap=12，label_height=18）：
        # 多切一行就把拼版底色 (14,15,17) 圈进来，它离 tile 底色 (22,23,26) 的距离
        # 大于阈值，于是整行被当成前景 —— 三条宽度全变成 200，测试假红。
        gap, lab = 12, 18
        widths = []
        for i in range(3):
            x0 = gap + i * (200 + gap)
            tile = img[gap + lab:gap + lab + 200, x0:x0 + 200]
            fg = np.abs(tile - np.array(BG)).sum(2) > 12
            xs = np.nonzero(fg.any(0))[0]
            widths.append(int(xs.max() - xs.min() + 1))
        self.assertAlmostEqual(2.0 / 8.0, widths[1] / widths[0], delta=0.04,
                               msg=f"FRONT/SIDE_R 宽度比失真，三张图没共用取景：{widths}")


if __name__ == "__main__":
    unittest.main()
