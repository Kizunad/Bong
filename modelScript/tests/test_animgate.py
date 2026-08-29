#!/usr/bin/env python3
"""animgate —— 动画后验的共享判据 + 差分自证。

和 gatekit 一样，这里的重点不是「干净动作全绿」，而是**每道门都能被自己的注入器打红**。
其中质心平衡那条最典型：把横移压扁到几乎为零但符号全保留，同侧率仍是 100% —— 只查
符号的门在这里全绿，而那正是「在冰面上平移」的样子。
"""

from __future__ import annotations

import io
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]

from bbmodel_maker.gates import animgate as ag  # noqa: E402


class Ch:
    def __init__(self, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0), scale=(1.0, 1.0, 1.0)):
        self.rot, self.pos, self.scale = list(rot), list(pos), list(scale)


class FakePose(dict):
    def __missing__(self, key):
        self[key] = Ch()
        return self[key]


class FakeRig:
    """够 animgate 的适配器用的最小 rig：order / world / bone_points / bones / elements。"""

    class _B:
        def __init__(self, elements):
            self.elements = elements

    def __init__(self, boxes, offsets=None):
        # boxes: {骨名: (from, to)}
        self.order = list(boxes)
        self.elements = {f"e-{n}": {"from": list(f), "to": list(t)}
                         for n, (f, t) in boxes.items()}
        self.bones = {n: self._B([f"e-{n}"]) for n in boxes}
        self._pts = {n: np.array([[x, y, z] for x in (f[0], t[0])
                                  for y in (f[1], t[1]) for z in (f[2], t[2])], float)
                     for n, (f, t) in boxes.items()}
        self._off = offsets or {}

    def bone_points(self, name):
        return self._pts.get(name, np.zeros((0, 3)))

    def world(self, pose=None):
        out = {}
        for n in self.order:
            M = np.eye(4)
            M[:3, 3] = np.array(self._off.get(n, (0.0, 0.0, 0.0)), float)
            out[n] = M
        return out


class PoseFromTracksTest(unittest.TestCase):
    TRACKS = {"leg": {"rotation": [(0.0, [0.0, 0.0, 0.0]), (1.0, [10.0, 20.0, 30.0])],
                      "position": [(0.5, [1.0, 1.0, 1.0])]}}

    def _at(self, tt):
        return ag.pose_from_tracks(self.TRACKS, tt, FakePose)

    def test_interpolates_linearly_between_keyframes(self) -> None:
        self.assertEqual([5.0, 10.0, 15.0], self._at(0.5)["leg"].rot,
                         "引擎播的就是关键帧之间的线性插值 —— 后验必须查同一个东西")

    def test_hits_the_keyframes_exactly(self) -> None:
        self.assertEqual([0.0, 0.0, 0.0], self._at(0.0)["leg"].rot)
        self.assertEqual([10.0, 20.0, 30.0], self._at(1.0)["leg"].rot)

    def test_clamps_outside_the_track_range(self) -> None:
        self.assertEqual([10.0, 20.0, 30.0], self._at(9.0)["leg"].rot, "末帧之后保持末值")
        self.assertEqual([0.0, 0.0, 0.0], self._at(-1.0)["leg"].rot,
                         "首帧之前保持首值 —— 不许因为「落不进任何区间」就跳到动作结尾")

    def test_single_keyframe_channel_is_constant(self) -> None:
        for tt in (0.0, 0.5, 1.0):
            self.assertEqual([1.0, 1.0, 1.0], self._at(tt)["leg"].pos)

    def test_untouched_bones_keep_their_defaults(self) -> None:
        self.assertEqual([1.0, 1.0, 1.0], self._at(0.5)["other"].scale)


class AdapterTest(unittest.TestCase):
    def setUp(self) -> None:
        self.rig = FakeRig({"body": ((-2, 4, -2), (2, 8, 2)),
                            "foot": ((-1, 0, -1), (1, 1, 1))})

    def test_bone_boxes_keeps_one_box_per_bone_by_default(self) -> None:
        """合并成一个大 AABB 会把「组里最近的那一对」冲淡，链断裂就永远量不出来。"""
        boxes = ag.bone_boxes(self.rig, FakePose(), {"all": ("body", "foot")})
        self.assertEqual(2, len(boxes["all"]))

    def test_bone_boxes_can_merge_on_request(self) -> None:
        lo, hi = ag.bone_boxes(self.rig, FakePose(), {"all": ("body", "foot")},
                               merge=True)["all"]
        self.assertTrue(np.allclose((-2, 0, -2), lo))
        self.assertTrue(np.allclose((2, 8, 2), hi))

    def test_bone_boxes_skips_groups_with_no_geometry(self) -> None:
        boxes = ag.bone_boxes(self.rig, FakePose(), {"ghost": ("nope",)})
        self.assertNotIn("ghost", boxes, "没有几何的组不该产出一个假盒子")

    def test_bone_boxes_follows_the_world_transform(self) -> None:
        moved = FakeRig({"foot": ((-1, 0, -1), (1, 1, 1))}, offsets={"foot": (0, 10, 0)})
        lo, hi = ag.bone_boxes(moved, FakePose(), {"f": ("foot",)}, merge=True)["f"]
        self.assertTrue(np.allclose((-1, 10, -1), lo))

    def test_min_gap_takes_the_nearest_pair_across_two_groups(self) -> None:
        a = [(np.zeros(3), np.ones(3))]
        b = [(np.array([9.0, 0, 0]), np.array([10.0, 1, 1])),
             (np.array([1.5, 0, 0]), np.array([2.5, 1, 1]))]
        self.assertAlmostEqual(0.5, ag.min_gap(a, b), places=12,
                               msg="要的是最近的那一对，不是合并盒之间的距离")

    def test_min_gap_accepts_a_bare_box_as_a_one_element_group(self) -> None:
        a = (np.zeros(3), np.ones(3))
        b = (np.array([2.0, 0, 0]), np.array([3.0, 1, 1]))
        self.assertAlmostEqual(1.0, ag.min_gap(a, b), places=12)
        self.assertAlmostEqual(ag.aabb_gap(a, b), ag.min_gap([a], [b]), places=12)

    def test_lowest_bone_names_the_culprit(self) -> None:
        y, who = ag.lowest_bone(self.rig, FakePose())
        self.assertEqual(0.0, y)
        self.assertEqual("foot", who,
                         "只报一个数字的话，「穿地 0.43」不知道该去调什么")

    def test_lowest_bone_on_an_empty_rig(self) -> None:
        y, who = ag.lowest_bone(FakeRig({}), FakePose())
        self.assertEqual("-", who)
        self.assertGreater(y, 1e8, "没有骨就没有最低点，不该假装有")

    def test_volume_center_is_volume_weighted(self) -> None:
        rig = FakeRig({"big": ((0, 0, 0), (4, 4, 4)), "small": ((10, 0, 0), (11, 1, 1))})
        c = ag.volume_center(rig, FakePose())
        self.assertLess(c[0], 3.0, "64 体积的大块必须把形心拽在自己这边")
        self.assertGreater(c[0], 2.0)

    def test_aabb_gap_signs(self) -> None:
        a = (np.array([0.0, 0, 0]), np.array([1.0, 1, 1]))
        far = (np.array([3.0, 0, 0]), np.array([4.0, 1, 1]))
        touch = (np.array([1.0, 0, 0]), np.array([2.0, 1, 1]))
        deep = (np.array([0.4, 0, 0]), np.array([1.4, 1, 1]))
        self.assertAlmostEqual(2.0, ag.aabb_gap(a, far), places=12, msg=">0 = 分开多远")
        self.assertAlmostEqual(0.0, ag.aabb_gap(a, touch), places=12, msg="0 = 刚好贴合")
        self.assertAlmostEqual(-0.6, ag.aabb_gap(a, deep), places=12, msg="<0 = 重叠多深")

    def test_aabb_gap_takes_the_separating_axis(self) -> None:
        """只要有一根轴分开了，两个盒子就没接触 —— 取三轴的最大间隙。"""
        a = (np.array([0.0, 0, 0]), np.array([1.0, 1, 1]))
        b = (np.array([0.5, 0.5, 9.0]), np.array([1.5, 1.5, 10.0]))
        self.assertAlmostEqual(8.0, ag.aabb_gap(a, b), places=12)


class GroundGateTest(unittest.TestCase):
    def test_a_clean_walk_passes_and_names_the_lowest_bone(self) -> None:
        r = ag.gate_ground(lambda t: (0.05, "toes_l"), n=20)
        self.assertTrue(r.ok)
        self.assertEqual("toes_l", r.extra["bone"])
        self.assertIn("toes_l", r.detail)

    def test_tolerance_boundary(self) -> None:
        self.assertTrue(ag.gate_ground(lambda t: (-0.5, "x"), n=4).ok,
                        "正好等于容差不算穿地（1 单位 = 1 纹理像素，0.5 以下看不见）")
        self.assertFalse(ag.gate_ground(lambda t: (-0.51, "x"), n=4).ok)

    def test_it_finds_the_worst_frame_not_the_first(self) -> None:
        r = ag.gate_ground(lambda t: (-2.0 if 0.4 < t < 0.6 else 0.5, "wing"), n=20)
        self.assertFalse(r.ok)
        self.assertAlmostEqual(-2.0, r.worst)
        self.assertGreater(r.extra["t"], 0.4)
        self.assertLess(r.extra["t"], 0.6)


class SlipGateTest(unittest.TestCase):
    def test_a_perfectly_uniform_stance_has_zero_residual(self) -> None:
        us = np.linspace(0, 1, 30)
        slope, res = ag.slip_residual(us, -6.0 * us + 2.0)
        self.assertAlmostEqual(-6.0, slope, places=9)
        self.assertAlmostEqual(0.0, res, places=9)

    def test_fewer_than_two_points_fails_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "支撑相采样少于两点"):
            ag.slip_residual([0.3], [1.0])

    def test_gate_reports_the_worst_foot(self) -> None:
        us = np.linspace(0, 1, 30)
        clean = -6.0 * us
        r = ag.gate_slip({"l": (us, clean), "r": (us, clean + 2.0 * np.sin(us * np.pi))})
        self.assertFalse(r.ok)
        self.assertEqual("r", r.extra["foot"], "报的必须是最差那只脚")

    def test_tolerance_boundary(self) -> None:
        us = np.array([0.0, 0.5, 1.0])
        # 首末点在拟合直线上、中点抬起 h：最小二乘线穿过三点均值，残差恰好是 2h/3。
        # 取 h = 0.45 → 残差 0.30 = 容差；h = 0.48 → 0.32 越线。
        at_tol = ag.gate_slip({"l": (us, np.array([0.0, 0.45, 0.0]))})
        self.assertAlmostEqual(ag.SKATE_TOL, at_tol.worst, places=9)
        self.assertTrue(at_tol.ok, "残差正好等于容差不算超标")
        over = ag.gate_slip({"l": (us, np.array([0.0, 0.48, 0.0]))})
        self.assertFalse(over.ok)
        self.assertIn("超标", over.detail)

    def test_no_samples_fails_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "没有支撑相样本"):
            ag.gate_slip({})


class SeamGateTest(unittest.TestCase):
    def _pose_at(self, drift=0.0):
        """闭合的循环姿态；drift 只加在末帧上 —— 那正是「接缝没接上」的形状。"""
        def at(t):
            p = FakePose()
            p["neck"] = Ch(rot=(float(np.sin(2 * np.pi * t)) * 5.0
                                + (drift if t >= 1.0 else 0.0), 0, 0))
            return p
        return at

    def test_a_closed_loop_passes(self) -> None:
        self.assertTrue(ag.gate_seam(self._pose_at(), ("neck",)).ok)

    def test_an_open_loop_is_caught_and_names_the_channel(self) -> None:
        r = ag.gate_seam(self._pose_at(drift=1.0), ("neck",))
        self.assertFalse(r.ok)
        self.assertEqual("neck", r.extra["bone"])
        self.assertEqual("rot", r.extra["channel"])

    def test_one_shot_animations_are_not_checked(self) -> None:
        r = ag.gate_seam(self._pose_at(drift=99.0), ("neck",), loop=False)
        self.assertTrue(r.ok, "单次动作本来就该停在别处")
        self.assertIn("单次动作", r.detail)

    def test_tolerance_boundary(self) -> None:
        self.assertTrue(ag.gate_seam(self._pose_at(drift=0.02), ("neck",)).ok)
        self.assertFalse(ag.gate_seam(self._pose_at(drift=0.03), ("neck",)).ok)

    def test_bones_absent_from_the_pose_are_skipped(self) -> None:
        self.assertTrue(ag.gate_seam(self._pose_at(), ("neck", "ghost")).ok)


class OverlapGateTest(unittest.TestCase):
    def _boxes(self, dx):
        def at(t):
            return {"leg": (np.array([dx(t), 0.0, 0.0]), np.array([dx(t) + 2.0, 2.0, 2.0])),
                    "core": (np.array([5.0, 0.0, 0.0]), np.array([9.0, 2.0, 2.0]))}
        return at

    def test_clear_of_each_other_passes(self) -> None:
        r = ag.gate_overlap(self._boxes(lambda t: 0.0), [("leg", "core")], n=10)
        self.assertTrue(r.ok)

    def test_a_mid_animation_collision_is_caught(self) -> None:
        """静止姿不穿模不代表走起来不穿 —— 这道门就是为这个存在的。"""
        r = ag.gate_overlap(self._boxes(lambda t: 5.0 if 0.4 < t < 0.6 else 0.0),
                            [("leg", "core")], n=20)
        self.assertFalse(r.ok)
        self.assertEqual("leg × core", r.extra["pair"])
        self.assertGreater(r.worst, ag.OVERLAP_TOL)

    def test_tolerance_boundary(self) -> None:
        self.assertTrue(ag.gate_overlap(self._boxes(lambda t: 5.0 - 2.0 + 0.75),
                                        [("leg", "core")], n=4).ok,
                        "重叠正好 0.75 = 模型自己的分辨率下限，不判")
        self.assertFalse(ag.gate_overlap(self._boxes(lambda t: 5.0 - 2.0 + 0.80),
                                         [("leg", "core")], n=4).ok)

    def test_pairs_with_a_missing_group_are_skipped(self) -> None:
        r = ag.gate_overlap(self._boxes(lambda t: 0.0), [("leg", "ghost")], n=4)
        self.assertTrue(r.ok)


class ChainBreakGateTest(unittest.TestCase):
    def _boxes(self, gap_at):
        def at(t):
            g = gap_at(t)
            return {"a": (np.array([0.0, 0.0, 0.0]), np.array([1.0, 1.0, 1.0])),
                    "b": (np.array([1.0 + g, 0.0, 0.0]), np.array([2.0 + g, 1.0, 1.0]))}
        return at

    CHAIN = {"颈": ["a", "b"]}

    def test_a_seated_chain_passes(self) -> None:
        self.assertTrue(ag.gate_chain_break(self._boxes(lambda t: 0.0), self.CHAIN, n=8).ok)

    def test_a_gap_that_opens_mid_animation_is_caught(self) -> None:
        r = ag.gate_chain_break(self._boxes(lambda t: 2.0 if t > 0.5 else 0.0),
                                self.CHAIN, n=20)
        self.assertFalse(r.ok)
        self.assertEqual("颈", r.extra["chain"])
        self.assertGreater(r.extra["t"], 0.5)

    def test_tolerance_boundary(self) -> None:
        # 间隙直接按坐标造，别用 (1.0 + g) − 1.0 那种算法 —— 0.05 在浮点上会算成
        # 0.050000000000000044，恰好把「等于容差」这条边界推到线外，测的就不是判据了。
        def at(gap):
            return lambda t: {"a": (np.zeros(3), np.array([0.0, 1.0, 1.0])),
                              "b": (np.array([gap, 0.0, 0.0]), np.array([gap + 1, 1.0, 1.0]))}

        exact = ag.gate_chain_break(at(ag.LINK_BREAK_TOL), self.CHAIN, n=4)
        self.assertAlmostEqual(ag.LINK_BREAK_TOL, exact.worst, places=15)
        self.assertTrue(exact.ok, "0.05 是留给抗锯齿的余量，等于容差不算断开")
        self.assertFalse(ag.gate_chain_break(at(0.06), self.CHAIN, n=4).ok)

    def test_overlapping_links_are_fine(self) -> None:
        self.assertTrue(ag.gate_chain_break(self._boxes(lambda t: -0.5), self.CHAIN, n=4).ok,
                        "链节相互咬着不是断裂")


class BalanceGateTest(unittest.TestCase):
    def test_a_proper_weight_shift_passes(self) -> None:
        r = ag.gate_balance(lambda t: 0.8, n=20)
        self.assertTrue(r.ok)
        self.assertEqual(20, r.extra["frames"])
        self.assertAlmostEqual(1.0, r.extra["same_side"])

    def test_wrong_side_is_caught(self) -> None:
        r = ag.gate_balance(lambda t: -0.3, n=20)
        self.assertFalse(r.ok)
        self.assertIn("质心没压过去", r.detail)

    def test_flat_shift_with_a_perfect_same_side_ratio_is_still_caught(self) -> None:
        """**这条是整道门的存在理由。**

        把横移整个删掉之后，光靠骨盆侧倾也能让质心偏出零点几个百分点，同侧率仍是
        100% —— 而那正是在冰面上平移的样子。只查符号的门在这里会全绿。
        """
        r = ag.gate_balance(lambda t: 0.01, n=20)
        self.assertAlmostEqual(1.0, r.extra["same_side"], msg="同侧率确实是 100%")
        self.assertFalse(r.ok, "峰值判据必须把它抓住")
        self.assertIn("冰面", r.detail)

    def test_peak_boundary(self) -> None:
        self.assertTrue(ag.gate_balance(lambda t: 0.45, n=8).ok, "峰值等于门限算过")
        self.assertFalse(ag.gate_balance(lambda t: 0.44, n=8).ok)

    def test_double_support_gaits_are_reported_but_not_judged(self) -> None:
        r = ag.gate_balance(lambda t: None, n=8)
        self.assertTrue(r.ok)
        self.assertIn("不判", r.detail)


class InjectorTest(unittest.TestCase):
    def test_sink_by_pushes_the_model_under_ground(self) -> None:
        base = lambda t: (0.1, "toes")            # noqa: E731
        self.assertTrue(ag.gate_ground(base, n=4).ok)
        self.assertFalse(ag.gate_ground(ag.sink_by(base), n=4).ok)

    def test_skate_by_breaks_the_uniform_stance(self) -> None:
        us = np.linspace(0, 1, 30)
        clean = {"l": (us, -6.0 * us)}
        self.assertTrue(ag.gate_slip(clean).ok)
        self.assertFalse(ag.gate_slip(ag.skate_by(clean)).ok)

    def test_break_seam_by_opens_the_loop(self) -> None:
        def at(t):
            p = FakePose()
            p["neck"] = Ch(rot=(np.sin(2 * np.pi * t), 0, 0))
            return p
        self.assertTrue(ag.gate_seam(at, ("neck",)).ok)
        self.assertFalse(ag.gate_seam(ag.break_seam_by(at, "neck"), ("neck",)).ok)

    def test_overlap_by_drives_one_group_into_another(self) -> None:
        def boxes(t):
            return {"a": (np.zeros(3), np.ones(3) * 2),
                    "b": (np.array([9.0, 0, 0]), np.array([11.0, 2, 2]))}
        self.assertTrue(ag.gate_overlap(boxes, [("a", "b")], n=4).ok)
        self.assertFalse(ag.gate_overlap(ag.overlap_by(boxes, "a", "b"), [("a", "b")], n=4).ok)

    def test_overlap_by_refuses_when_a_group_is_missing(self) -> None:
        with self.assertRaises(ag.AnimInjectionImpossible):
            ag.overlap_by(lambda t: {"a": (np.zeros(3), np.ones(3))}, "a", "ghost")(0.0)

    def test_snap_chain_by_opens_a_link(self) -> None:
        def boxes(t):
            return {"a": (np.zeros(3), np.ones(3)),
                    "b": (np.array([1.0, 0, 0]), np.array([2.0, 1, 1]))}
        chain = {"c": ["a", "b"]}
        self.assertTrue(ag.gate_chain_break(boxes, chain, n=4).ok)
        self.assertFalse(ag.gate_chain_break(ag.snap_chain_by(boxes, "b"), chain, n=4).ok)

    def test_snap_chain_by_refuses_when_the_group_is_missing(self) -> None:
        with self.assertRaises(ag.AnimInjectionImpossible):
            ag.snap_chain_by(lambda t: {"a": (np.zeros(3), np.ones(3))}, "ghost")(0.0)

    def test_injectors_accept_the_list_form_that_bone_boxes_actually_returns(self) -> None:
        """回归（Kody #2104 抓到的）：`bone_boxes()` **默认逐件返回一串盒**，而两个
        盒子类注入器早先写成 `alo, ahi = boxes[a]`，组里不止一件时抛
        `ValueError: too many values to unpack` —— 于是 `self_test()` 当场崩掉，
        而 self_test 正是用来证明门有鉴别力的那一步，它自己崩了就什么都证明不了。
        """
        def boxes(t):
            return {"a": [(np.zeros(3), np.ones(3)),
                          (np.array([1.0, 0, 0]), np.array([2.0, 1, 1])),
                          (np.array([2.0, 0, 0]), np.array([3.0, 1, 1]))],
                    "b": [(np.array([9.0, 0, 0]), np.array([10.0, 1, 1]))]}

        moved = ag.overlap_by(boxes, "a", "b")(0.0)["a"]
        self.assertEqual(3, len(moved), "整组三件都要跟着挪，不能只挪其中一件")
        deltas = {tuple(np.round(m[0] - o[0], 9)) for m, o in zip(moved, boxes(0.0)["a"])}
        self.assertEqual(1, len(deltas), "整组必须**同一个**位移，不是各挪各的")
        self.assertFalse(ag.gate_overlap(ag.overlap_by(boxes, "a", "b"),
                                         [("a", "b")], n=2).ok,
                         "注入之后互穿门必须真的报出来")

        lifted = ag.snap_chain_by(boxes, "a", gap=3.0)(0.0)["a"]
        self.assertEqual(3, len(lifted))
        lift = {tuple(np.round(m[0] - o[0], 9)) for m, o in zip(lifted, boxes(0.0)["a"])}
        self.assertEqual(1, len(lift))
        self.assertFalse(ag.gate_chain_break(ag.snap_chain_by(boxes, "a"),
                                             {"c": ["a", "b"]}, n=2).ok,
                         "注入之后链断裂门必须真的报出来")

    def test_overlap_injection_works_on_a_group_whose_own_centre_is_empty(self) -> None:
        """回归（Kody #2104 二轮）：组里的盒是离散的，整体 AABB 中心可能落在空处。

        「两条腿」这种组，按两组的**整体中心**对齐会让 b 正好坐进 a 的缝里，一对实体盒
        都没真重叠 —— 注入等于没注入，`self_test` 于是报「没有鉴别力」**冤枉一道好门**。
        假红和假绿一样有害：它会逼下一个人去把门限调松。
        """
        def boxes(t):
            return {"a": [(np.array([-4.0, 0, 0]), np.array([-2.0, 2, 1])),
                          (np.array([2.0, 0, 0]), np.array([4.0, 2, 1]))],
                    "b": [(np.array([-0.5, 0, 0]), np.array([0.5, 2, 1]))]}

        self.assertTrue(ag.gate_overlap(boxes, [("a", "b")], n=2).ok, "干净时不该报")
        hit = ag.gate_overlap(ag.overlap_by(boxes, "a", "b"), [("a", "b")], n=2)
        self.assertFalse(hit.ok, f"离散组也必须注得进去，实测 worst={hit.worst:.2f}")
        self.assertGreater(hit.worst, ag.OVERLAP_TOL)

    def test_overlap_injection_refuses_when_the_geometry_is_too_thin(self) -> None:
        """「这份几何造不出该门要抓的缺陷」和「门没有鉴别力」是两回事，不许混成一个结论。"""
        def thin(t):
            return {"a": (np.zeros(3), np.full(3, 0.3)),
                    "b": (np.array([9.0, 0, 0]), np.array([9.3, 0.3, 0.3]))}

        with self.assertRaises(ag.AnimInjectionImpossible) as ctx:
            ag.overlap_by(thin, "a", "b")(0.0)
        self.assertIn("0.30", str(ctx.exception), "报错要把实测到的最深咬合写出来")

    def test_chain_injection_clears_boxes_taller_than_the_nominal_gap(self) -> None:
        """回归（Kody #2104 二轮）：`aabb_gap` 取三轴分离的最大值。

        平移量吃不掉盒自身高度时两件仍在 y 上重叠、x/z 又贴着，量出来的间隙是 0 ——
        注入失效，照样冤枉这道门。实测 y 跨度 10 的链节，固定 +3.0 完全无效。
        """
        def tall(t):
            return {"a": (np.zeros(3), np.array([1.0, 10.0, 1.0])),
                    "b": (np.array([1.0, 0, 0]), np.array([2.0, 10.0, 1.0]))}

        chain = {"c": ["a", "b"]}
        self.assertTrue(ag.gate_chain_break(tall, chain, n=2).ok, "干净时不该报")
        hit = ag.gate_chain_break(ag.snap_chain_by(tall, "b"), chain, n=2)
        self.assertFalse(hit.ok, f"高链节也必须注得开，实测 worst={hit.worst:.2f}")
        self.assertGreater(hit.worst, ag.LINK_BREAK_TOL)

    def test_chain_injection_rejects_a_gap_smaller_than_the_gate_tolerance(self) -> None:
        with self.assertRaisesRegex(ValueError, "必须大于门限"):
            ag.snap_chain_by(lambda t: {}, "a", gap=0.01, tol=ag.LINK_BREAK_TOL)

    def test_flatten_balance_keeps_the_sign_and_kills_the_peak(self) -> None:
        base = lambda t: 0.9 if t < 0.5 else None      # noqa: E731
        flat = ag.flatten_balance_by(base)
        self.assertIsNone(flat(0.9), "非单支撑相仍然是 None")
        self.assertGreater(flat(0.1), 0.0, "符号保留 —— 同侧率还是 100%")
        self.assertLess(flat(0.1), ag.BALANCE_MIN)
        self.assertTrue(ag.gate_balance(base, n=20).ok)
        self.assertFalse(ag.gate_balance(flat, n=20).ok)


class RealAdapterSelfTestTest(unittest.TestCase):
    """端到端回归：用**真的** `bone_boxes` 适配器（默认逐件形式）驱动 self_test。

    单元测试里各写各的 `boxes_at` 闭包返回单个盒，正好绕开了真实用法 —— 这个类专门
    走文档推荐的那条路，把注入器和门接在一起跑一遍。
    """

    def setUp(self) -> None:
        self.rig = FakeRig({
            "leg1": ((0, 0, 0), (1, 2, 1)), "leg2": ((1, 0, 0), (2, 2, 1)),
            "core1": ((2, 0, 0), (3, 2, 1)), "core2": ((3, 0, 0), (4, 2, 1)),
            "head1": ((4, 0, 0), (5, 2, 1)), "head2": ((5, 0, 0), (6, 2, 1)),
        })
        groups = {"leg": ("leg1", "leg2"), "core": ("core1", "core2"),
                  "head": ("head1", "head2")}
        self.gates = ag.AnimGates(
            "真适配器 / real",
            boxes_at=lambda t: ag.bone_boxes(self.rig, FakePose(), groups),
            overlap_pairs=(("leg", "head"),),
            chains={"躯干": ["leg", "core", "head"]},
            chain_probe="head",
            frames=8,
        )

    def test_bone_boxes_really_hands_back_a_list_per_group(self) -> None:
        boxes = ag.bone_boxes(self.rig, FakePose(), {"leg": ("leg1", "leg2")})
        self.assertIsInstance(boxes["leg"], list)
        self.assertEqual(2, len(boxes["leg"]))

    def test_both_box_gates_are_clean_on_the_laid_out_rig(self) -> None:
        self.assertEqual(["逐帧互穿", "链断裂"], [g.label for g in self.gates.run_all()])
        for g in self.gates.run_all():
            self.assertTrue(g.ok, f"{g.label} 不该报：{g.detail}")

    def test_self_test_runs_to_completion_and_finds_both_gates_discriminating(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = self.gates.self_test()
        self.assertEqual(0, broken, buf.getvalue())
        self.assertIn("2/2 道门有鉴别力", buf.getvalue())


class AnimGatesTest(unittest.TestCase):
    def _clean(self, **over):
        us = np.linspace(0, 1, 30)

        def pose_at(t):
            p = FakePose()
            p["neck"] = Ch(rot=(float(np.sin(2 * np.pi * t)) * 4.0, 0, 0))
            return p

        def boxes_at(t):
            return {"leg": (np.zeros(3), np.ones(3) * 2.0),
                    "core": (np.array([2.0, 0, 0]), np.array([6.0, 2, 2])),
                    "head": (np.array([6.0, 0, 0]), np.array([8.0, 2, 2]))}

        cfg = dict(
            title="探针 / probe",
            lowest_at=lambda t: (0.05, "toes_l"),
            stance_samples={"l": (us, -6.0 * us)},
            pose_at=pose_at, bones=("neck",),
            boxes_at=boxes_at, overlap_pairs=(("leg", "head"),),
            chains={"躯干": ["leg", "core", "head"]},
            frac_at=lambda t: 0.8 if t < 0.5 else None,
            frames=20,
        )
        cfg.update(over)
        return ag.AnimGates(**cfg)

    def test_all_six_gates_are_wired_when_every_closure_is_given(self) -> None:
        gates = self._clean()
        self.assertEqual(6, len(gates.specs()))
        self.assertEqual(["穿地", "滑步", "循环接缝", "逐帧互穿", "链断裂", "质心平衡"],
                         [g.label for g in gates.run_all()])

    def test_only_the_declared_gates_run(self) -> None:
        gates = ag.AnimGates("最小", lowest_at=lambda t: (0.0, "x"))
        self.assertEqual(1, len(gates.specs()))

    def test_declaring_nothing_fails_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "一个测量闭包都没给"):
            ag.AnimGates("空").specs()

    def test_pose_without_bones_fails_loud(self) -> None:
        with self.assertRaisesRegex(ValueError, "没给 bones"):
            ag.AnimGates("缺骨", pose_at=lambda t: FakePose()).specs()

    def test_one_shot_animation_drops_the_seam_gate(self) -> None:
        gates = self._clean(loop=False)
        self.assertNotIn("循环接缝", [g.label for g in gates.run_all()])

    def test_report_is_all_green_on_a_clean_action(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            bad = self._clean().report()
        self.assertEqual(0, bad, buf.getvalue())
        self.assertIn("→ 0 道门未过", buf.getvalue())
        self.assertIn("姿态本身对不对", buf.getvalue(),
                      "报告必须自己交代查不到的那一层，别让人以为全绿就是好看")

    def test_self_test_finds_every_gate_discriminating(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = self._clean().self_test()
        self.assertEqual(0, broken, buf.getvalue())
        self.assertIn("6/6 道门有鉴别力", buf.getvalue())

    def test_self_test_catches_a_tolerance_loosened_into_uselessness(self) -> None:
        """门限抬到天上，注入缺陷后照样过 —— 这道门就没有鉴别力了。"""
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = self._clean(sink_tol=999.0).self_test()
        self.assertEqual(1, broken, buf.getvalue())
        self.assertIn("没有鉴别力", buf.getvalue())
        self.assertIn("穿地", buf.getvalue())

    def test_self_test_catches_a_gate_that_already_fails_on_the_clean_action(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = self._clean(lowest_at=lambda t: (-9.0, "wing")).self_test()
        self.assertGreaterEqual(broken, 1)
        self.assertIn("干净动作上就没过", buf.getvalue())

    def test_self_test_reports_an_impossible_injection(self) -> None:
        gates = self._clean(overlap_probe=("leg", "ghost"))
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = gates.self_test()
        self.assertGreaterEqual(broken, 1)
        self.assertIn("造不出缺陷", buf.getvalue())

    def test_self_test_can_run_quietly(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            self._clean().self_test(verbose=False)
        self.assertEqual("", buf.getvalue())


if __name__ == "__main__":
    unittest.main()
