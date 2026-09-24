#!/usr/bin/env python3
"""gatekit —— 七道几何门 + 自带的缺陷注入器。

这一组测试有两层：
  1. 每道门自己的判据（边界、白名单、豁免、符号方向）；
  2. **自证机制本身也要能红** —— 故意配一个白名单反了的穿模门，`self_test()` 必须
     指出这道门没有鉴别力。历史上正是这种「坏版本和修好的版本都报 17 处」的假绿，
     让人以为门在工作。
"""

from __future__ import annotations

import io
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "generators"))

from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.gates.gatekit import AssetGates, InjectionImpossible, Seat  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

MATS = {"hard": (200, 200, 200), "soft": (100, 90, 80), "trim": (150, 140, 120)}


def _rig(cubes, mats=MATS) -> Rig:
    """cubes = [(骨名, 件名, from, to, 材质)]，骨骼按首次出现建。"""
    rig = Rig(mats)
    for bone, name, frm, to, mat in cubes:
        if bone not in rig.bones:
            rig.bone(bone, (0.0, 0.0, 0.0))
        rig.cube(bone, name, frm, to, mat=mat)
    return rig


COLORS = gatekit.mats_by_color(MATS)


class HelperTest(unittest.TestCase):
    def test_mats_by_color_reproduces_rigkit_modulo_eight(self) -> None:
        many = {f"m{i}": (i, i, i) for i in range(10)}
        table = gatekit.mats_by_color(many)
        self.assertEqual("m8", table[0], "第 9 种材质的 color 绕回 0 —— 这是 rigkit 的既有行为")
        self.assertEqual(8, len(table), "color 只有 8 个槽")

    def test_bone_of_finds_the_owner_and_marks_orphans(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (1, 1, 1), "hard")])
        self.assertEqual("body", gatekit.bone_of(rig, rig.elements[0]["uuid"]))
        self.assertEqual("?", gatekit.bone_of(rig, "no-such-uuid"))

    def test_seat_surface_accepts_a_constant_or_a_function_of_height(self) -> None:
        flat = Seat("x", 0, 1, 3.5, 0.1)
        taper = Seat("x", 0, 1, lambda y: 3.5 - y * 0.5, 0.1)
        self.assertEqual(3.5, flat.surface_at(99.0))
        self.assertEqual(2.5, taper.surface_at(2.0), "壁随高度收口时宿主面得跟着走")


class OrphanTest(unittest.TestCase):
    def test_owned_elements_are_clean(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (1, 1, 1), "hard")])
        self.assertEqual([], gatekit.gate_orphans(rig))

    def test_element_dropped_from_its_bone_is_reported_by_name(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (1, 1, 1), "hard"),
                    ("body", "b", (2, 0, 0), (3, 1, 1), "hard")])
        rig.bones["body"]["children"].remove(rig.elements[1]["uuid"])
        self.assertEqual(["b"], gatekit.gate_orphans(rig))

    def test_empty_rig_has_no_orphans(self) -> None:
        self.assertEqual([], gatekit.gate_orphans(Rig(MATS)))


class OverflowTest(unittest.TestCase):
    def _names(self, frm, to, shift=(8.0, 0.0, 8.0)):
        rig = _rig([("body", "a", frm, to, "hard")])
        return [v.split(":")[0] for v in gatekit.gate_overflow(rig, shift)]

    def test_a_piece_inside_the_block_is_clean(self) -> None:
        self.assertEqual([], self._names((-4, 0, -4), (4, 16, 4)))

    def test_each_axis_and_each_direction_is_checked(self) -> None:
        self.assertEqual(["a"], self._names((-9, 0, 0), (-8.5, 1, 1)), "x 负向越界")
        self.assertEqual(["a"], self._names((8.5, 0, 0), (9, 1, 1)), "x 正向越界")
        self.assertEqual(["a"], self._names((0, -1, 0), (1, -0.5, 1)), "y 负向越界（y 不平移）")
        self.assertEqual(["a"], self._names((0, 16.5, 0), (1, 17, 1)), "y 正向越界")
        self.assertEqual(["a"], self._names((0, 0, -9), (1, 1, -8.5)), "z 负向越界")
        self.assertEqual(["a"], self._names((0, 0, 8.5), (1, 1, 9)), "z 正向越界")

    def test_tolerance_boundary(self) -> None:
        self.assertEqual([], self._names((-8, 0, -8), (8.005, 1, 8)),
                         "超出 0.005px 在容差 0.01 之内，不该报")
        self.assertEqual(["a"], self._names((-8, 0, -8), (8.02, 1, 8)),
                         "超出 0.02px 已越过容差，必须报")

    def test_violation_text_carries_both_corners(self) -> None:
        rig = _rig([("body", "a", (20, 0, 0), (21, 1, 1), "hard")])
        self.assertEqual(["a: (20, 0, 0)→(21, 1, 1)"], gatekit.gate_overflow(rig))

    def test_shift_is_a_parameter_not_a_constant(self) -> None:
        self.assertEqual([], self._names((0, 0, 0), (16, 16, 16), shift=(0.0, 0.0, 0.0)),
                         "已经在方块空间里建模的资产用零平移")


class DegenerateTest(unittest.TestCase):
    def _bad(self, to, thr=gatekit.MIN_THICKNESS):
        rig = _rig([("body", "a", (0, 0, 0), to, "hard")])
        return gatekit.gate_degenerate(rig, thr)

    def test_fat_piece_is_clean(self) -> None:
        self.assertEqual([], self._bad((1, 1, 1)))

    def test_exactly_at_the_threshold_is_allowed(self) -> None:
        self.assertEqual([], self._bad((1, 0.2, 1)), "判据是 < 门限，等于门限不算退化")

    def test_just_under_the_threshold_is_flagged_on_any_axis(self) -> None:
        self.assertEqual(["a: (1, 0.19, 1)"], self._bad((1, 0.19, 1)))
        self.assertEqual(1, len(self._bad((0.19, 1, 1))))
        self.assertEqual(1, len(self._bad((1, 1, 0.19))))

    def test_threshold_is_a_parameter(self) -> None:
        self.assertEqual([], self._bad((1, 0.19, 1), thr=0.1))


class FloatingTest(unittest.TestCase):
    def test_two_face_touching_pieces_are_clean(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                    ("body", "b", (2, 0, 0), (4, 2, 2), "hard")])
        self.assertEqual([], gatekit.gate_floating(rig))

    def test_a_piece_beside_another_with_a_third_axis_gap_is_caught(self) -> None:
        """三轴判据存在的全部理由。

        a 和 b 在 x/y 上大面积重叠，但 z 上差着 0.5px 的缝 —— 早先的两轴判据把这判成
        「搭上了」，骨扣离前檐 0.06px 分离、侧袋针脚整排飘在兜外都是这么漏过去的。
        """
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                    ("body", "b", (0, 0, 2.5), (2, 2, 4.5), "hard")])
        # 把第三轴容差放到无穷大 = 退回旧的两轴判据：它会说这两件搭上了。
        self.assertEqual([], gatekit.gate_floating(rig, contact_tol=float("inf")),
                         "两轴判据确实会放过这一对 —— 这就是它当年漏掉骨扣分离的原因")
        self.assertEqual(["a", "b"], sorted(gatekit.gate_floating(rig)),
                         "三轴判据下，第三轴上 0.5px 的缝必须让两件都判为悬空")

    def test_gap_within_contact_tolerance_still_counts_as_touching(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                    ("body", "b", (0, 0, 2.05), (2, 2, 4), "hard")])
        self.assertEqual([], gatekit.gate_floating(rig),
                         "0.05px < 0.12 容差，渲染上看不见这道缝")

    def test_overlap_must_exceed_the_face_threshold_on_two_axes(self) -> None:
        # 只在 y 上有实打实的重叠面，x 上只搭 0.1px（< 0.15）→ 不算接触
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                    ("body", "b", (1.9, 0, 2), (4, 2, 4), "hard")])
        self.assertEqual(["a", "b"], sorted(gatekit.gate_floating(rig)))

    def test_free_whitelist_exempts_deliberate_floaters(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                    ("body", "b", (0, 0, 2), (2, 2, 4), "hard"),
                    ("body", "sprig", (0, 40, 0), (1, 41, 1), "hard")])
        self.assertEqual(["sprig"], gatekit.gate_floating(rig))
        self.assertEqual([], gatekit.gate_floating(rig, frozenset({"sprig"})))

    def test_a_lone_piece_has_nothing_to_touch(self) -> None:
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard")])
        self.assertEqual(["a"], gatekit.gate_floating(rig))


class InterpenetratingTest(unittest.TestCase):
    def _pair(self, m1, m2, bone2="lid", offset=(0.0, 0.0, 0.0), **kw):
        rig = _rig([("body", "a", (0, 0, 0), (2, 2, 2), m1),
                    (bone2, "b", (offset[0], offset[1], offset[2]),
                     (offset[0] + 2, offset[1] + 2, offset[2] + 2), m2)])
        return gatekit.gate_interpenetrating(rig, COLORS, **kw)

    def test_deep_cross_bone_overlap_of_hard_materials_is_flagged(self) -> None:
        out = self._pair("hard", "soft", offset=(1.0, 0.0, 0.0))
        self.assertEqual(["a(hard) × b(soft) 互穿 1.00px"], out)

    def test_same_bone_pieces_are_never_compared(self) -> None:
        self.assertEqual([], self._pair("hard", "soft", bone2="body", offset=(1.0, 0, 0)),
                         "同 bone 内是同一构件的分段（taper/shaft 相邻段），本来就该重叠")

    def test_same_material_is_exempt(self) -> None:
        self.assertEqual([], self._pair("hard", "hard", offset=(1.0, 0, 0)))

    def test_soft_over_whitelist_exempts_the_designed_overlaps(self) -> None:
        soft = frozenset({frozenset(("hard", "soft"))})
        self.assertEqual([], self._pair("hard", "soft", offset=(1.0, 0, 0), soft_over=soft))

    def test_bite_threshold_boundary(self) -> None:
        self.assertEqual([], self._pair("hard", "soft", offset=(1.45, 0, 0)),
                         "重叠 0.55 == 门限，判据是 > 门限，薄贴不该报")
        self.assertEqual(1, len(self._pair("hard", "soft", offset=(1.44, 0, 0))),
                         "重叠 0.56 越过门限，真扎进去了")

    def test_hard_override_forces_a_check_that_the_whitelist_would_have_skipped(self) -> None:
        """背篓的背带就是这条：材质是 cord，会顺着「绳捆皮盖」被整体放行。"""
        soft = frozenset({frozenset(("hard", "soft"))})
        forced = self._pair("hard", "soft", offset=(1.0, 0, 0), soft_over=soft,
                            hard_override=lambda n1, m1, n2, m2: True)
        self.assertEqual(1, len(forced))

    def test_hard_override_still_respects_the_same_material_exemption(self) -> None:
        self.assertEqual([], self._pair("hard", "hard", offset=(1.0, 0, 0),
                                        hard_override=lambda *a: True),
                         "同材质是同一构件的延续，强制查也不该报")

    def test_no_overlap_is_clean(self) -> None:
        self.assertEqual([], self._pair("hard", "soft", offset=(5.0, 0, 0)))


class SeatingTest(unittest.TestCase):
    WALL = 0.9

    def _run(self, frm, to, seats, materials=None, mat="trim"):
        rig = _rig([("body", "band_f_0", frm, to, mat)])
        return gatekit.gate_seating(rig, seats, COLORS, materials)

    def _seat(self, **kw):
        base = dict(match="band_f_", axis=2, outward=+1, surface=2.0,
                    min_bite=0.10, max_bite=self.WALL, host="前壁")
        base.update(kw)
        return (Seat(**base),)

    def test_a_properly_seated_piece_is_clean(self) -> None:
        # 宿主面 z=2.0，件从 1.78 到 2.6 → 咬入 0.22，在 [0.10, 0.9] 内
        self.assertEqual([], self._run((0, 0, 1.78), (1, 1, 2.6), self._seat()))

    def test_too_shallow_reads_as_floating_off_the_host(self) -> None:
        out = self._run((0, 0, 1.95), (1, 1, 2.6), self._seat())
        self.assertEqual(["band_f_0 没咬住前壁（bite=0.05 < 0.1）"], out)

    def test_negative_bite_means_the_piece_left_the_host_entirely(self) -> None:
        out = self._run((0, 0, 2.5), (1, 1, 3.2), self._seat())
        self.assertIn("没咬住前壁", out[0])
        self.assertIn("-0.50", out[0], "分离要报成负咬入，别把符号吃掉")

    def test_too_deep_reads_as_punching_through_the_inner_wall(self) -> None:
        out = self._run((0, 0, 0.8), (1, 1, 2.6), self._seat())
        self.assertEqual(["band_f_0 扎穿前壁 0.30px（可咬深度 0.9）"], out)

    def test_boundary_values_are_inclusive_on_both_ends(self) -> None:
        self.assertEqual([], self._run((0, 0, 1.9), (1, 1, 2.6), self._seat()),
                         "咬入正好 0.10 = 下界，不该报")
        self.assertEqual([], self._run((0, 0, 1.1), (1, 1, 2.6), self._seat()),
                         "咬入正好 0.90 = 壁厚，不该报")

    def test_max_bite_none_skips_the_punch_through_check(self) -> None:
        """栓在檐外的硬件没有「内壁」可穿 —— 骨扣就是这一档。"""
        self.assertEqual([], self._run((0, 0, -5), (1, 1, 2.6),
                                       self._seat(max_bite=None)))

    def test_outward_minus_one_measures_from_the_other_side(self) -> None:
        # 宿主面 z=-2.0，件在它的负方向外侧：咬入 = hi[2] − (−2.0)
        seats = self._seat(surface=-2.0, outward=-1, host="后壁")
        self.assertEqual([], self._run((0, 0, -2.6), (1, 1, -1.78), seats))
        self.assertIn("没咬住后壁", self._run((0, 0, -3.2), (1, 1, -2.05), seats)[0])

    def test_callable_surface_tracks_a_tapering_wall(self) -> None:
        seats = (Seat("band_f_", 0, +1, lambda y: 3.5 - y, 0.10, self.WALL, host="侧壁"),)
        # 件的 y 中心 = 1.0 → 宿主面 x = 2.5；件从 2.28 起 → 咬入 0.22
        self.assertEqual([], self._run((2.28, 0.5, 0), (3.1, 1.5, 1), seats))
        # 同一个件挪到 y 中心 = 3.0 → 宿主面退到 x = 0.5，件整个飘在外面
        self.assertIn("没咬住侧壁", self._run((2.28, 2.5, 0), (3.1, 3.5, 1), seats)[0])

    def test_unmatched_elements_are_not_checked(self) -> None:
        rig = _rig([("body", "flap_crease", (0, 0, 9), (1, 1, 10), "trim")])
        self.assertEqual([], gatekit.gate_seating(rig, self._seat(), COLORS),
                         "贴自己那件的横缝/折痕匹配不到 Seat，就不查")

    def test_exclude_list_skips_named_pieces(self) -> None:
        seats = self._seat(match="band_", exclude=("band_f_0",))
        self.assertEqual([], self._run((0, 0, 9), (1, 1, 10), seats))

    def test_material_filter_narrows_what_the_gate_looks_at(self) -> None:
        bad = (0, 0, 9), (1, 1, 10)
        self.assertEqual(1, len(self._run(*bad, self._seat(), materials=frozenset({"trim"}))))
        self.assertEqual([], self._run(*bad, self._seat(), materials=frozenset({"hard"})),
                         "不在材质白名单里的件整件跳过")

    def test_first_matching_seat_wins(self) -> None:
        seats = (Seat("band_f_", 2, +1, 2.0, 0.10, None), Seat("band_", 2, +1, 99.0, 0.10, None))
        self.assertEqual([], self._run((0, 0, 1.78), (1, 1, 2.6), seats),
                         "更具体的前缀写在前面就该先命中")


class MirrorTest(unittest.TestCase):
    def test_paired_pieces_must_mirror_and_asym_bones_are_exempt(self) -> None:
        rig = _rig([("body", "wall_l", (-3, 0, 0), (-1, 2, 2), "hard"),
                    ("body", "wall_r", (1, 0, 0), (3, 2, 2), "hard"),
                    ("lid", "flap", (0.5, 3, 0), (2, 4, 2), "soft")])
        self.assertEqual(1, len(gatekit.gate_mirror(rig)),
                         "盖压歪的件不排除时会被中线判据抓住")
        self.assertEqual([], gatekit.gate_mirror(rig, asym=("lid",)))

    def test_single_side_translation_is_caught(self) -> None:
        rig = _rig([("body", "wall_l", (-3, 0, 0), (-1, 2, 2), "hard"),
                    ("body", "wall_r", (1.5, 0, 0), (3.5, 2, 2), "hard")])
        self.assertTrue(any("x 未镜像" in v for v in gatekit.gate_mirror(rig)))


class InjectorTest(unittest.TestCase):
    def setUp(self) -> None:
        self.rig = _rig([("body", "wall_l", (-3, 0, 0), (-1, 2, 2), "hard"),
                         ("body", "wall_r", (1, 0, 0), (3, 2, 2), "hard"),
                         ("lid", "flap", (-1, 2, 0), (1, 4, 2), "soft")])

    def test_injectors_never_touch_the_original_rig(self) -> None:
        before = [dict(e) for e in self.rig.elements]
        for fn in (gatekit.inject_orphan, gatekit.inject_overflow, gatekit.inject_degenerate,
                   gatekit.inject_floating, gatekit.inject_mirror):
            fn(self.rig, colors=COLORS, asym=())
        self.assertEqual([e["from"] for e in before],
                         [e["from"] for e in self.rig.elements],
                         "注入器必须在副本上动手，原 rig 一个字节都不许改")

    def test_each_injector_reports_who_it_broke(self) -> None:
        for fn in (gatekit.inject_orphan, gatekit.inject_overflow, gatekit.inject_degenerate,
                   gatekit.inject_floating, gatekit.inject_mirror):
            rig, victim, what = fn(self.rig, colors=COLORS, asym=())
            self.assertIn(victim, what, f"{fn.__name__} 的说明里得点出受害件名")
            self.assertIsInstance(rig, Rig)

    def test_interpenetration_injector_needs_a_checkable_pair(self) -> None:
        same = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                     ("lid", "b", (5, 0, 0), (7, 2, 2), "hard")])
        with self.assertRaises(InjectionImpossible):
            gatekit.inject_interpenetrating(same, colors=COLORS)

    def test_seating_injector_needs_a_seat(self) -> None:
        with self.assertRaises(InjectionImpossible):
            gatekit.inject_seating(self.rig, seats=(), colors=COLORS)

    def test_mirror_injector_falls_back_to_a_midline_piece(self) -> None:
        mid = _rig([("body", "spine", (-1, 0, 0), (1, 2, 2), "hard")])
        _, victim, what = gatekit.inject_mirror(mid, asym=())
        self.assertEqual("spine", victim)
        self.assertIn("中线件", what)

    def test_mirror_injector_gives_up_when_everything_is_exempt(self) -> None:
        with self.assertRaises(InjectionImpossible):
            gatekit.inject_mirror(self.rig, asym=("body", "lid"))


class AssetGatesTest(unittest.TestCase):
    def setUp(self) -> None:
        # 一个规规矩矩的小资产：底板 + 左右壁（镜像成对）+ 盖（不对称，走 asym）
        # + 一颗骨扣（第三种材质 —— 没有它，穿模注入器找不到一对该被检查的异材质件）。
        self.rig = _rig([("body", "floor", (-3.0, 0.0, 0.0), (3.0, 1.0, 2.0), "hard"),
                         ("body", "wall_l", (-3.0, 1.0, 0.0), (-1.0, 3.0, 2.0), "hard"),
                         ("body", "wall_r", (1.0, 1.0, 0.0), (3.0, 3.0, 2.0), "hard"),
                         ("lid", "flap", (-3.0, 3.0, 0.0), (3.0, 4.0, 2.0), "soft"),
                         ("lid", "peg", (-0.5, 4.0, 0.4), (0.5, 5.0, 1.6), "trim")])
        self.gates = AssetGates("测试件 / probe", MATS, asym=("lid",),
                                soft_over=frozenset({frozenset(("hard", "soft"))}))

    def test_six_gates_without_seats_seven_with(self) -> None:
        self.assertEqual(
            ["orphans", "overflow", "degenerate", "floating", "interpenetrating", "mirror"],
            [k for k, _, _, _ in self.gates.specs()],
        )
        seated = AssetGates("x", MATS, seats=(Seat("wall_", 0, 1, 3.0, 0.1),))
        self.assertEqual("seating", [k for k, _, _, _ in seated.specs()][-2],
                         "贴面就位门排在镜像门之前")
        self.assertEqual(7, len(seated.specs()))

    def test_degenerate_label_tracks_its_threshold(self) -> None:
        loose = AssetGates("x", MATS, min_thickness=0.5)
        self.assertIn("退化薄片 (<0.5px)", [lab for _, lab, _, _ in loose.specs()])

    def test_clean_rig_passes_every_gate(self) -> None:
        self.assertEqual(0, self.gates.total(self.rig))
        for g in self.gates.run_all(self.rig):
            self.assertTrue(g.ok, f"{g.label} 不该报：{g.violations}")

    def test_report_prints_the_canonical_block(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            total = self.gates.report(self.rig, px=16.0, note="注：人眼定夺。")
        lines = buf.getvalue().splitlines()
        self.assertEqual(0, total)
        self.assertEqual("测试件 / probe 自检:", lines[0])
        self.assertEqual("  bbox   : 6.0×5.0×2.0px = 0.38W × 0.31H × 0.12D 格", lines[1])
        self.assertEqual("  cubes  : 5  bones: 2", lines[2])
        self.assertEqual("  材质   : 3/3 种在用 — hard:3, soft:1, trim:1", lines[3])
        self.assertEqual("  ✓ 孤儿 element: 0", lines[4])
        self.assertEqual("  → 共 0 处违例", lines[-2])
        self.assertEqual("  注：人眼定夺。", lines[-1])

    def test_report_shows_at_most_six_examples_per_gate(self) -> None:
        """违例多了只列前六条 —— 报告是给人扫一眼的，不是日志。"""
        rig = _rig([("body", f"a{i}", (0.0, 40.0 + i * 3, 0.0), (1.0, 41.0 + i * 3, 1.0), "hard")
                    for i in range(8)])
        buf = io.StringIO()
        with redirect_stdout(buf):
            gatekit.AssetGates("x", MATS).report(rig)
        lines = buf.getvalue().splitlines()
        head = next(i for i, ln in enumerate(lines) if ln.startswith("  ✗ 悬空无接触:"))
        self.assertEqual("  ✗ 悬空无接触: 8", lines[head])
        detail = 0
        for ln in lines[head + 1:]:
            if not ln.startswith("      - "):
                break
            detail += 1
        self.assertEqual(6, detail, "八处违例只该列出前六条")

    def test_self_test_passes_on_a_well_formed_asset(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = self.gates.self_test(self.rig)
        self.assertEqual(0, broken, buf.getvalue())
        self.assertIn("6/6 道门有鉴别力", buf.getvalue())

    def test_self_test_catches_a_gate_whose_threshold_is_too_loose(self) -> None:
        """核心元测试：**自证机制本身必须能红。**

        把穿模门的咬入门限抬到 999px —— 注入器照样能挑出一对该查的件并把它们叠成同心，
        但门永远报 0。这就是「判据在，输出也在，就是没有鉴别力」那一档假绿，
        self_test 必须点名这道门。
        """
        loose = AssetGates("松门 / loose", MATS, asym=("lid",),
                           soft_over=frozenset({frozenset(("hard", "soft"))}),
                           interpen_bite=999.0)
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = loose.self_test(self.rig)
        out = buf.getvalue()
        self.assertEqual(1, broken, out)
        self.assertIn("没有鉴别力", out)
        self.assertIn("硬件互穿", out)
        self.assertIn("5/6 道门有鉴别力", out)

    def test_self_test_flags_a_whitelist_that_exempts_everything(self) -> None:
        """白名单放行到「什么都不查」时，连缺陷都造不出来 —— 同样算这道门失效。"""
        blind = AssetGates("盲门 / blind", MATS, asym=("lid",),
                           soft_over=frozenset({frozenset(("hard", "soft")),
                                                frozenset(("hard", "trim")),
                                                frozenset(("soft", "trim"))}))
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = blind.self_test(self.rig)
        self.assertEqual(1, broken, buf.getvalue())
        self.assertIn("造不出缺陷", buf.getvalue())

    def test_self_test_catches_a_gate_that_already_fires_on_the_clean_rig(self) -> None:
        dirty = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                      ("body", "lonely", (0, 40, 0), (1, 41, 1), "hard")])
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = AssetGates("脏底 / dirty", MATS).self_test(dirty)
        self.assertGreaterEqual(broken, 1)
        self.assertIn("干净模型上就报了", buf.getvalue())

    def test_self_test_reports_an_impossible_injection_as_a_failure(self) -> None:
        """造不出缺陷 = 这道门在这份资产上无从验证，只能算失效，不许静默算过。"""
        flat = _rig([("body", "a", (0, 0, 0), (2, 2, 2), "hard"),
                     ("body", "b", (2, 0, 0), (4, 2, 2), "hard")])
        buf = io.StringIO()
        with redirect_stdout(buf):
            broken = AssetGates("单骨 / one-bone", MATS).self_test(flat)
        self.assertGreaterEqual(broken, 1)
        self.assertIn("造不出缺陷", buf.getvalue())

    def test_self_test_can_run_quietly(self) -> None:
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.gates.self_test(self.rig, verbose=False)
        self.assertEqual("", buf.getvalue())


class MigratedGeneratorsTest(unittest.TestCase):
    """迁过来的两个生成器：门数、门序、干净、且每道门都有鉴别力。"""

    def test_grass_pouch_keeps_its_seven_gates_and_stays_clean(self) -> None:
        import gen_grass_pouch as gp

        rig = gp.build()
        labels = [g.label for g in gp.GATES.run_all(rig)]
        self.assertEqual(
            ["孤儿 element", "越出 0..16 方块空间", "退化薄片 (<0.2px)", "悬空无接触",
             "硬件互穿（穿模）", "贴面件未就位/扎穿", "对称件左右不镜像"], labels)
        self.assertEqual(0, gp.GATES.total(rig))

    def test_back_basket_keeps_its_six_gates_and_stays_clean(self) -> None:
        import gen_back_basket as bb

        rig = bb.build()
        labels = [g.label for g in bb.GATES.run_all(rig)]
        self.assertEqual(
            ["孤儿 element", "越出 0..16 方块空间", "退化薄片 (<0.2px)", "悬空无接触",
             "硬件互穿（穿模）", "对称件左右不镜像"], labels)
        self.assertEqual(0, bb.GATES.total(rig))

    def test_both_assets_pass_their_own_differential_self_test(self) -> None:
        import gen_back_basket as bb
        import gen_grass_pouch as gp

        for mod in (gp, bb):
            buf = io.StringIO()
            with redirect_stdout(buf):
                broken = mod.GATES.self_test(mod.build())
            self.assertEqual(0, broken,
                             f"{mod.__name__} 有门报不出自己该抓的缺陷:\n{buf.getvalue()}")

    def test_back_basket_strap_override_is_wired(self) -> None:
        import gen_back_basket as bb

        self.assertTrue(bb._strap_vs_hard("strap_l_1", "cord", "post_r_f", "bamboo"))
        self.assertTrue(bb._strap_vs_hard("lid_flap", "hide", "strap_r_1", "cord"))
        self.assertFalse(bb._strap_vs_hard("cord_a", "cord", "lid_flap", "hide"),
                         "普通捆绳压皮盖是构造，不该被强制查")


if __name__ == "__main__":
    unittest.main()
