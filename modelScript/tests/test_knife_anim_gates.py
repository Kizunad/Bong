#!/usr/bin/env python3
"""`knife_anim_gates` 的两层锁：门自身的鉴别力，和四条匕首动画的实际成绩。

分两层的理由和 `gatekit` / `animgate` 一样，而且这套门自己就现场撞过一次：
`gate_flip` 的第一版量的是「刃相对**前臂**的朝向」，听着天经地义 —— 结果那是个
**恒等于零的量**（手持物被 display 变换焊死在前臂上），门永远报 0°，干净动画都过不了。
如果只有「跑一遍看红不红」这一层，这条门会以「它一直在报警」的姿态活很久。差分自证
把它当场判死：干净动画上就没过 = 失效。

第二层才是动画本身的成绩。两层都在，才既防「动画退化」也防「门退化」。
"""

from __future__ import annotations

import math
import sys
import unittest
from pathlib import Path

import numpy as np

LIB_DIR = Path(__file__).resolve().parents[1]
for _d in (LIB_DIR / "tools", LIB_DIR / "generators", LIB_DIR.parent / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import knife_anim_gates as KG  # noqa: E402


class GateDiscriminationTest(unittest.TestCase):
    """每道门：干净动画上必须过，注入它该抓的缺陷后必须报。"""

    def test_every_gate_discriminates_on_every_animation(self):
        broken = {name: KG.build(name).self_test(verbose=False) for name in sorted(KG.SUITE)}
        broken = {k: v for k, v in broken.items() if v}
        self.assertFalse(
            broken,
            f"这些动画上有门失效：{broken}。失效有两种：干净动画上就报警（门限定错了，"
            f"或动画真的坏了），以及注入缺陷后仍然过（门没有鉴别力，等于没写）。"
            f"跑 `python3 modelScript/tools/knife_anim_gates.py --self-test` 看逐条明细")

    def test_the_suite_covers_every_dagger_animation_on_disk(self):
        """磁盘上有的匕首动画都得挂在门里 —— 漏挂一条就等于没门。"""
        on_disk = {p.stem for p in KG.ANIM_DIR.glob("dagger_*.json")}
        self.assertEqual(
            on_disk, set(KG.SUITE),
            f"门的清单 {sorted(KG.SUITE)} 和磁盘上的 {sorted(on_disk)} 对不上 —— "
            f"新增匕首动画必须同时在 SUITE 里登记")


class DaggerAnimationGateTest(unittest.TestCase):
    """四条动画的实际成绩。红了就是动画退化，不是门的问题（上一类已经证过门有效）。"""

    def _fails(self, name):
        return [(g.label, g.detail) for g in KG.build(name).run_all() if not g.ok]

    def test_dagger_stab(self):
        self.assertFalse(self._fails("dagger_stab"))

    def test_dagger_slash(self):
        self.assertFalse(self._fails("dagger_slash"))

    def test_dagger_reverse_slash(self):
        self.assertFalse(self._fails("dagger_reverse_slash"))

    def test_dagger_grip_switch(self):
        self.assertFalse(self._fails("dagger_grip_switch"))

    def test_dagger_reverse_grip_switch(self):
        self.assertFalse(self._fails("dagger_reverse_grip_switch"))


class GripIsAMeasurableQuantityTest(unittest.TestCase):
    """v4 的核心回归：**把 `rightItem` 骨头拿掉，新门必须当场报红**。

    这不是造一个假缺陷 —— 「没有手持物骨头」正是 v1~v3 三个版本的真实形状。当时
    `dagger_grip_switch` 全程握法角 0°（刀在手里一动没动）却被判合格，
    `dagger_reverse_slash` 号称反握实际全程正握。两条都必须被下面的用例钉死。
    """

    @staticmethod
    def _strip_item_bone(name):
        """还原成 v3：同一条动画，但手持物骨头不存在。"""
        take = KG.KnifeTake(name, KG.DAGGER_MODEL)
        take.kfs = {k: v for k, v in take.kfs.items() if k != "rightItem"}
        return take

    def test_a_grip_switch_without_the_item_bone_fails_the_flip_gate(self):
        take = self._strip_item_bone("dagger_grip_switch")
        res = KG.gate_flip(take.grip_angle_at, take.ticks, item_at=take.item_at)
        self.assertFalse(res.ok,
                         f"手臂照抬照转、刀却一度没动，也必须判红。实际：{res.detail}")
        self.assertLess(res.worst, 1.0,
                        f"没有手持物骨头时握法角必须恒为 0，实测 {res.worst}")

    def test_the_shipped_grip_switch_passes_the_same_gate(self):
        take = KG.KnifeTake("dagger_grip_switch", KG.DAGGER_MODEL)
        res = KG.gate_flip(take.grip_angle_at, take.ticks, item_at=take.item_at)
        self.assertTrue(res.ok, f"出料版必须过这道门。实际：{res.detail}")

    def test_a_reverse_slash_without_the_item_bone_fails_the_hold_gate(self):
        take = self._strip_item_bone("dagger_reverse_slash")
        res = KG.gate_grip_hold(take.grip_angle_at, take.ticks, 180.0)
        self.assertFalse(res.ok,
                         f"号称反握、实际全程正握，必须判红。实际：{res.detail}")
        self.assertGreater(res.worst, 170.0,
                           f"偏差应接近整整 180°，实测 {res.worst}")

    def test_the_shipped_reverse_slash_holds_the_reverse_grip_all_the_way(self):
        take = KG.KnifeTake("dagger_reverse_slash", KG.DAGGER_MODEL)
        res = KG.gate_grip_hold(take.grip_angle_at, take.ticks, 180.0)
        self.assertTrue(res.ok, f"出料版必须全程反握。实际：{res.detail}")

    def test_forward_grip_animations_report_exactly_zero(self):
        """正握两招不该带任何手持物旋转 —— 带了就是误加，也得报。"""
        for name in ("dagger_stab", "dagger_slash"):
            take = KG.KnifeTake(name, KG.DAGGER_MODEL)
            worst = max(abs(take.grip_angle_at(t)[0]) for t in take.ticks)
            self.assertLess(worst, 1e-6, f"{name} 不该有握法旋转，实测最大 {worst}")

    def test_the_grip_angle_is_independent_of_the_arm_pose(self):
        """握法角量的是「刀相对手」，手臂怎么摆都不该影响它。

        这一条把 v3 那个错误判断反过来钉住：当时的结论是「刃相对前臂的朝向恒等于零，
        所以这个量没法用」——**前半句对，后半句错**。剥掉手臂差异之后剩下的正是
        手持物骨头，它恰恰是唯一该管这件事的东西。
        """
        take = KG.KnifeTake("dagger_reverse_slash", KG.DAGGER_MODEL)
        angles = [take.grip_angle_at(t)[0] for t in take.ticks]
        # 这条动画手臂从 pitch −151 扫到 −26、bend 从 10 折到 51，握法角必须纹丝不动
        self.assertLess(max(angles) - min(angles), 1e-6,
                        f"手臂扫过 125°，握法角却应恒定，实测跨度 {max(angles) - min(angles)}")

    def test_unwrap_keeps_a_half_turn_from_reading_as_a_reversal(self):
        """主值序列跨 ±180 会跳；不连续化就会把一次干净翻转读成「反向晃了 180°」。"""
        self.assertEqual([0.0, -90.0, -170.0, -190.0],
                         KG._unwrap([0.0, -90.0, -170.0, 170.0]))
        self.assertEqual([10.0, 20.0], KG._unwrap([10.0, 20.0]), "不跨界时不该改动")


class GripSwitchPairTest(unittest.TestCase):
    """换握去/回两条必须是**焊死的一对**。

    这对存在的理由是个系统层的账：one-shot emote 的 stopTick 混出会把 `rightItem`
    拉回 0，"换完握就一直反握"单条 emote 表达不了。回程做成一招之后，握法状态由
    "当前播的是哪一条"承载 —— 前提是两端严丝合缝，否则来回切会在衔接处跳，而这种跳
    只有连着播才看得见，接触表那种静态图完全看不出来。
    """

    PAIR = ("dagger_grip_switch", "dagger_reverse_grip_switch")

    def test_the_two_ends_are_axis_for_axis_identical(self):
        a, b = (KG.KnifeTake(n, KG.DAGGER_MODEL) for n in self.PAIR)
        pairs = ({(p, ax) for p, axes in a.kfs.items() for ax in axes}
                 | {(p, ax) for p, axes in b.kfs.items() for ax in axes})
        for my, other, my_t, other_t in ((a, b, 0.0, 8.0), (a, b, 8.0, 0.0)):
            for part, axis in sorted(pairs):
                with self.subTest(part=part, axis=axis, tick=my_t):
                    self.assertAlmostEqual(
                        my.sample(part, axis, my_t), other.sample(part, axis, other_t),
                        places=6,
                        msg=f"{self.PAIR[0]}@t{my_t:g} 与 {self.PAIR[1]}@t{other_t:g} 的 "
                            f"{part}.{axis} 对不上 —— 来回切会跳")

    def test_they_sweep_the_grip_in_opposite_directions(self):
        angs = {}
        for name in self.PAIR:
            take = KG.KnifeTake(name, KG.DAGGER_MODEL)
            series = KG._unwrap([take.grip_angle_at(t)[0] for t in take.ticks])
            angs[name] = series[-1] - series[0]
        self.assertLess(angs[self.PAIR[0]] * angs[self.PAIR[1]], 0.0,
                        f"一去一回必须反向，实测扫过 {angs}")
        for name, swept in angs.items():
            self.assertAlmostEqual(abs(swept), 180.0, delta=KG.FLIP_TOL,
                                   msg=f"{name} 扫过 {swept:.1f}°，不是半圈")

    def test_both_pass_through_the_same_outward_corridor(self):
        """半程时刃都该指向玩家右外侧 —— 反过来会横穿身体前方往躯干/头上蹭。"""
        for name in self.PAIR:
            take = KG.KnifeTake(name, KG.DAGGER_MODEL)
            mid = take.grip_angle_at(4.0)[0]
            self.assertLess(mid, 0.0,
                            f"{name} 半程握法角 {mid:+.0f}°，走廊反了")


class GateGeometryUnitTest(unittest.TestCase):
    """门内部那几个几何小工具的正反用例 —— 它们错了，上面两层一起假绿。"""

    def test_segment_box_hit_is_inclusive_of_the_margin(self):
        box = ((-1.0, 1.0), (-1.0, 1.0), (-1.0, 1.0))
        inside = KG._seg_hits_box(np.array([-5.0, 0.0, 0.0]), np.array([5.0, 0.0, 0.0]),
                                  box, 0.0)
        self.assertTrue(inside, "横穿盒心的线段必须判命中")
        outside = KG._seg_hits_box(np.array([-5.0, 3.0, 0.0]), np.array([5.0, 3.0, 0.0]),
                                   box, 0.0)
        self.assertFalse(outside, "盒外 2 单位平行掠过不该判命中")
        grazing = KG._seg_hits_box(np.array([-5.0, 1.4, 0.0]), np.array([5.0, 1.4, 0.0]),
                                   box, 0.5)
        self.assertTrue(grazing, "margin 要真的把盒子外扩：1.4 在 1.0+0.5 之内")

    def test_negative_margin_shrinks_the_box(self):
        """自穿模那道门用负 margin 表示「擦着皮不算」，别把它当成 0 处理。"""
        box = ((-2.0, 2.0), (-2.0, 2.0), (-2.0, 2.0))
        a, b = np.array([-5.0, 1.5, 0.0]), np.array([5.0, 1.5, 0.0])
        self.assertTrue(KG._seg_hits_box(a, b, box, 0.0))
        self.assertFalse(KG._seg_hits_box(a, b, box, -1.0),
                         "margin=-1 应把盒子缩到 ±1，y=1.5 就该落在外面")

    def test_held_item_probe_reads_the_blade_span_from_the_model(self):
        grip, tip, butt = KG.held_item_probe(KG.DAGGER_MODEL)
        self.assertGreater(tip[1], grip[1], "刃尖必须在握把之上（模型沿 +Y 出刃）")
        self.assertLess(butt[1], grip[1], "柄尾必须在握把之下")
        self.assertAlmostEqual(8.0, float(grip[0]), places=6)

    def test_display_of_refuses_a_model_without_a_hand_transform(self):
        import json
        import tempfile
        with tempfile.NamedTemporaryFile("w", suffix=".bbmodel", delete=False) as f:
            json.dump({"elements": [{"from": [0, 0, 0], "to": [1, 1, 1]}]}, f)
            path = Path(f.name)
        with self.assertRaises(ValueError):
            KG.display_of(path)


class GateThresholdProvenanceTest(unittest.TestCase):
    """门限得是**量出来的分界**，不是随手填的。这里锁住几条最容易被改松的。"""

    def test_torch_ceiling_would_have_caught_the_old_poses(self):
        """返工前四条动画的刀尖常年在 y=29~32；门限必须低于那个区间。"""
        self.assertLess(KG.TIP_CEIL, 29.0,
                        "刀尖上限放到 29 以上，就放行了返工前那批「举火把」姿态")

    def test_elevation_ceiling_would_have_caught_the_old_poses(self):
        self.assertLess(KG.ELEV_CEIL, 39.0,
                        "返工前最坏 +48°、次坏 +41°；上限必须卡在它们之下")

    def test_flip_target_is_a_real_half_turn_not_the_v3_stand_in(self):
        """v4：换握量的是「刀在手里转过多少」，门限回到 180±12。

        **v3 的 55° 是个教训，不是历史细节**：当时链上没有 `rightItem` 骨头，握法这个
        量在数据里根本不存在，判据只好退而量世界刃向 —— 而那个量对一次普通挥砍也有
        87°。门限于是被从 180 一路调到 55，去迁就一条**根本没在换握**的动画。
        新门限的两侧都有实测：用户手摆的两帧是 177.5° 与 180.5°（容差必须盖住），
        「完全没换握」是 0°（必须被拒）。
        """
        self.assertEqual(180.0, KG.FLIP_TARGET, "换握就是把刃倒转半圈，没有第二个数")
        self.assertGreaterEqual(KG.FLIP_TOL, 3.0,
                                "容差要盖得住用户手摆的 177.5° / 180.5°")
        self.assertLess(KG.FLIP_TOL, 90.0,
                        "容差大到 90 就把「转了一半」也放行了")
        self.assertGreater(KG.FLIP_TARGET - KG.FLIP_TOL, 90.0,
                           "下沿必须高过半圈的一半，否则「刃只侧过来」也算换握")
        self.assertGreater(KG.FLIP_TRAVEL_MAX, 11.3,
                           "上限低于本设计的握把行程，等于门限贴着地板")
        self.assertLess(KG.FLIP_TRAVEL_MAX, 15.1,
                        "上限放到「一次完整挥砍」的行程以上，转刀退化成挥刀也报不出来")

    def test_off_axis_ceiling_separates_interpolation_residue_from_a_wrong_axis_flip(self):
        """偏轴门限夹在两族实测之间：插值残差 9.8°，绕错轴 180°。

        emote 逐轴线性插值一段 60°/tick 的自转，中间帧不会精确落在纯 X 旋转的单参
        子群上 —— 这是格式本身的代价，不是缺陷。而绕刀面法线翻会**全程**报大偏轴。
        """
        self.assertGreater(KG.GRIP_OFF_AXIS_MAX, 9.8,
                           "门限低于插值残差 = 干净动画永远报警")
        self.assertLess(KG.GRIP_OFF_AXIS_MAX, 90.0,
                        "门限放到 90 以上，绕刀面法线翻也报不出来")

    def test_decel_ratio_sits_between_a_real_finish_and_a_hard_stop(self):
        """收势减速：四条实测末格占峰速 0~1%；被掐断的极端是 100%。"""
        self.assertGreater(KG.DECEL_RATIO, 0.05,
                           "比值定得比正常收势还低，等于永远报警")
        self.assertLess(KG.DECEL_RATIO, 1.0,
                        "1.0 = 末格就是峰速也放行，等于没锁")

    def test_blendout_minimum_matches_what_the_animations_declare(self):
        """四条招都写 endTick 8 / stopTick 10。下限高过 2 会把它们全判死。"""
        self.assertLessEqual(KG.BLENDOUT_MIN, 2.0)
        self.assertGreater(KG.BLENDOUT_MIN, 0.0)


class DirOffArcTest(unittest.TestCase):
    """刃向走廊的判据换成了「到大圆弧的垂距」—— 这个几何函数错了，整条门假绿。"""

    @staticmethod
    def _unit(v):
        v = np.array(v, float)
        return v / np.linalg.norm(v)

    def test_a_point_on_the_arc_reads_zero_regardless_of_how_fast_it_got_there(self):
        """**这条是换判据的全部理由**：带缓动的一段不是匀速的，旧判据（比 slerp(u)）
        会把「走得慢」读成「偏得远」——实测反握上撕因此凭空报出 25°，而刃根本没离开弧。
        """
        a, b = self._unit([0, 0, -1]), self._unit([0, 1, 0])
        for frac in (0.05, 0.25, 0.5, 0.75, 0.95):
            ang = math.radians(90.0 * frac)
            on_arc = self._unit([0, math.sin(ang), -math.cos(ang)])
            self.assertAlmostEqual(
                0.0, KG.dir_off_arc(on_arc, a, b), places=6,
                msg=f"弧上的点（走了 {frac:.0%}）必须读 0，与它走得快慢无关")

    def test_out_of_plane_deviation_is_measured(self):
        a, b = self._unit([0, 0, -1]), self._unit([0, 1, 0])
        tilted = self._unit([math.sin(math.radians(20)), 0, -math.cos(math.radians(20))])
        self.assertAlmostEqual(20.0, KG.dir_off_arc(tilted, a, b), places=4,
                               msg="离弧所在平面 20° 就该读 20°")

    def test_overshooting_the_endpoint_is_measured_as_a_detour(self):
        """刃冲过端点再折回来（历史上那版转刀就是这样）必须报出来。"""
        a, b = self._unit([0, 0, -1]), self._unit([0, 1, 0])
        past = self._unit([0, -math.sin(math.radians(30)), -math.cos(math.radians(30))])
        self.assertAlmostEqual(30.0, KG.dir_off_arc(past, a, b), places=4,
                               msg="投影落在弧段外时，判据取到最近端点的角距")

    def test_degenerate_arc_falls_back_to_the_endpoint_angle(self):
        a = self._unit([0, 0, -1])
        off = self._unit([0, math.sin(math.radians(15)), -math.cos(math.radians(15))])
        self.assertAlmostEqual(15.0, KG.dir_off_arc(off, a, a), places=4,
                               msg="两端重合时弧退化成一个点，只能量到那个点的角距")


class GateWindowTest(unittest.TestCase):
    """窗口参数（`since` / `until`）必须真的把窗口外的帧排除掉，否则等于没配。"""

    def test_torch_window_ignores_frames_before_since(self):
        def item_at(t):
            y = 40.0 if t < 5.0 else 10.0
            g = np.array([0.0, y, 0.0])
            return g, g + np.array([0.0, 1.0, 0.0]), g - np.array([0.0, 1.0, 0.0])
        ticks = [i / 2 for i in range(17)]
        self.assertFalse(KG.gate_torch(item_at, ticks, 26.0).ok,
                         "不设窗口时，前半段 y=41 必须报")
        self.assertTrue(KG.gate_torch(item_at, ticks, 26.0, since=5.0).ok,
                        "窗口从 t≥5 起算时，前半段不该再报")

    def test_elbow_window_ignores_frames_after_until(self):
        def bend_at(t):
            return 60.0 if t <= 6.0 else 2.0
        ticks = [i / 2 for i in range(17)]
        self.assertFalse(KG.gate_elbow(bend_at, ticks, 15.0).ok,
                         "不设窗口时，收势段 bend=2 必须报")
        self.assertTrue(KG.gate_elbow(bend_at, ticks, 15.0, until=6.0).ok,
                        "窗口收在 t≤6 时，收势段不该再报")

    def test_behind_window_ignores_frames_after_until(self):
        def item_at(t):
            z = -10.0 if t <= 4.0 else 13.0
            g = np.array([0.0, 18.0, z])
            return g, g + np.array([0.0, 0.0, 1.0]), g - np.array([0.0, 0.0, 1.0])
        ticks = [i / 2 for i in range(17)]
        ident = lambda _t: np.eye(4)  # noqa: E731
        self.assertFalse(KG.gate_behind(item_at, ident, ticks, 3.0).ok,
                         "不设窗口时，收势段 z=+14 必须报")
        self.assertTrue(KG.gate_behind(item_at, ident, ticks, 3.0, until=4.0).ok,
                        "窗口收在 t≤4 时，收势段不该再报")


if __name__ == "__main__":
    unittest.main()
