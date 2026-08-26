#!/usr/bin/env python3
"""`anim_common.integer_retime` / `retime` —— 把一条设计好的动画整体拉长 / 压缩。

这两个函数存在的理由是一条运行时硬约束：**PlayerAnimator 的 tick 是整数**
（`AnimationJson.java:123` 的 `getAsInt()`、`KeyframeAnimation.java:451/469` 的
`findAtTick(int)` / `addKeyFrame(int, ...)`）。写小数进 JSON 不会报错——会被截断，然后
和相邻整数帧**撞成同一帧**，静默丢关键帧。于是"拉长 1.2 倍"这件事没有精确解，只能求
误差最小的整数落位，而"误差"该怎么定义、以及**拉长是搬帧不是重采样**，就是这里锁的
东西。

三条设计意图：

1. **误差按累计位置算，不按段长算。** 逐段四舍五入再累加，误差会一路攒下去；对累计
   位置取整则保证任何一帧的时间误差 ≤ 0.5 tick。
2. **`keep_gap` 是给"必须紧跟"的段用的**（overshoot 贴着 impact 后一 tick，
   conventions §2.6），被拉成 2 tick 就不再是弹性过冲。
3. **搬帧，姿态一个数都不改。** 每一段走过的姿态集合与段长无关，所以贴棍距离、挡不挡
   脸、包围盒这些几何判据在拉长后逐字成立——这是相对"重采样"的全部好处，必须有测试
   钉住，否则以后有人图省事换成重采样，几何判据会静默失去意义（重采样会把落在两个新
   整数 tick 之间的 LOAD / IMPACT 极值插值削掉）。
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (REPO / "client" / "tools",):
    sys.path.insert(0, str(_d))

import anim_common as A       # noqa: E402
import gen_club_sweep as SWEEP  # noqa: E402
import render_animation as RA  # noqa: E402

ANIM = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "player_animation"

# 一条形态和 club_sweep 一样的骨架：首段 2 tick，其余逐 tick。
SKELETON = [0, 2, 3, 4, 5, 6, 7, 8]


class IntegerRetimeTest(unittest.TestCase):
    """落位求解本身。"""

    def test_the_club_sweep_case_lands_where_the_generator_says(self) -> None:
        """真实用例：8 tick 骨架 × 1.25，overshoot（设计 tick 6）保持紧跟。"""
        got = A.integer_retime(SKELETON, 1.25, keep_gap={6})
        self.assertEqual({0: 0, 2: 3, 3: 4, 4: 5, 5: 6, 6: 7, 7: 9, 8: 10}, got)

    def test_scale_one_is_the_identity(self) -> None:
        """×1.0 不许动任何一帧——重定时管道空转时必须是恒等，否则"没拉长"也会漂。"""
        self.assertEqual({t: t for t in SKELETON},
                         A.integer_retime(SKELETON, 1.0))

    def test_every_frame_lands_within_half_a_tick_of_ideal(self) -> None:
        """核心保证：累计取整 ⇒ 任何一帧的时间误差 ≤ 0.5 tick。

        这条是"逐段取整再累加"做不到的——那种做法误差会累积，末帧能偏出好几 tick。
        """
        for scale in (1.1, 1.2, 1.25, 1.4, 1.5, 1.75, 2.0, 2.5, 3.0):
            got = A.integer_retime(SKELETON, scale)
            for src, out in got.items():
                self.assertLessEqual(
                    abs(out - src * scale), 0.5 + 1e-9,
                    f"scale={scale}: 设计 tick {src} 的理想位置 {src * scale:.2f}，"
                    f"落到 {out} —— 偏了 {abs(out - src * scale):.2f} tick")

    def test_the_result_is_strictly_increasing(self) -> None:
        """帧序不许乱，也不许两帧并成一帧（并了就是静默丢关键帧）。"""
        for scale in (1.1, 1.2, 1.25, 1.5, 2.0, 3.0):
            out = [A.integer_retime(SKELETON, scale)[t] for t in SKELETON]
            self.assertEqual(sorted(set(out)), out,
                             f"scale={scale} 的落位 {out} 不是严格递增")

    def test_halfway_rounds_up_not_to_even(self) -> None:
        """恰好落在半 tick 上时向上取整。

        Python 内建 `round` 是**banker's rounding**（`round(2.5) == 2`），拿它做重定时
        会让"半数"这一档随奇偶跳来跳去：同一个 scale 下 2.5→2 而 3.5→4，段长分布毫无
        道理可言。这里显式用 floor(x+0.5)。
        """
        # 2×1.25 = 2.5 → 3（banker's 会给 2）
        self.assertEqual(3, A.integer_retime([0, 2, 4], 1.25)[2])
        # 6×1.25 = 7.5 → 8（banker's 恰好也给 8，两种实现在这一档不可区分，
        # 所以真正能证伪 banker's 的是上面那条）
        self.assertEqual(8, A.integer_retime([0, 6, 8], 1.25)[6])

    def test_keep_gap_preserves_the_original_gap(self) -> None:
        """`keep_gap` 里的帧与上一帧的间隔保持原长，不受 scale 影响。"""
        for scale in (1.25, 1.5, 2.0):
            got = A.integer_retime(SKELETON, scale, keep_gap={6})
            self.assertEqual(
                1, got[6] - got[5],
                f"scale={scale}: overshoot 与 impact 之间被拉成 {got[6] - got[5]} tick")

    def test_keep_gap_on_a_multi_tick_gap_keeps_that_length(self) -> None:
        """原间隔不是 1 时也保持原长（2 tick 的段就保持 2 tick）。"""
        got = A.integer_retime([0, 2, 4, 6], 2.0, keep_gap={4})
        self.assertEqual(2, got[4] - got[2])

    def test_keep_gap_on_every_frame_degenerates_to_the_identity(self) -> None:
        """全部锁死 ⇒ 完全不拉长。边界情形要有确定行为，不能崩。"""
        self.assertEqual({t: t for t in SKELETON},
                         A.integer_retime(SKELETON, 3.0, keep_gap=SKELETON[1:]))

    def test_a_single_frame_animation_is_allowed(self) -> None:
        """只有 tick 0 的退化输入不该炸。"""
        self.assertEqual({0: 0}, A.integer_retime([0], 1.25))

    # ── 错误分支 ────────────────────────────────────────────────────────────

    def test_it_refuses_a_skeleton_that_does_not_start_at_zero(self) -> None:
        """首帧必须是 0：整条动画的时间原点，偏了之后 endTick / 落位全部对不上。"""
        with self.assertRaises(ValueError) as cm:
            A.integer_retime([1, 3, 5], 1.25)
        self.assertIn("tick 0", str(cm.exception))

    def test_it_refuses_an_empty_skeleton(self) -> None:
        with self.assertRaises(ValueError):
            A.integer_retime([], 1.25)

    def test_it_refuses_keep_gap_pointing_at_a_frame_that_does_not_exist(self) -> None:
        """写错帧号必须响——静默忽略的话，"我锁了 overshoot"就是个错觉。"""
        with self.assertRaises(ValueError) as cm:
            A.integer_retime(SKELETON, 1.25, keep_gap={99})
        self.assertIn("99", str(cm.exception))

    def test_it_refuses_a_scale_that_collapses_two_frames(self) -> None:
        """压缩到两帧撞在同一 tick 上必须报错，不能默默丢帧。"""
        with self.assertRaises(ValueError) as cm:
            A.integer_retime([0, 1, 2, 3], 0.25)
        self.assertIn("同一帧", str(cm.exception))


class RetimeTableTest(unittest.TestCase):
    """POSE 表的搬迁。"""

    def setUp(self) -> None:
        self.table = {
            0: dict(easing="OUTSINE", rightArm=dict(pitch=-30, bend=20, axis=180)),
            2: dict(easing="INSINE", rightArm=dict(pitch=+10, bend=60, axis=180)),
            4: dict(easing="LINEAR", rightArm=dict(pitch=-30, bend=20, axis=180)),
        }

    def test_it_moves_the_frames_without_touching_the_poses(self) -> None:
        """姿态原样搬——这是"拉长不改设计"的全部含义。"""
        out = A.retime(self.table, {0: 0, 2: 3, 4: 5})
        self.assertEqual([0, 3, 5], sorted(out))
        for src, dst in ((0, 0), (2, 3), (4, 5)):
            self.assertEqual(self.table[src], out[dst])

    def test_it_refuses_a_mapping_with_a_hole(self) -> None:
        """漏掉一帧就是丢一帧，必须响。"""
        with self.assertRaises(ValueError) as cm:
            A.retime(self.table, {0: 0, 2: 3})
        self.assertIn("4", str(cm.exception))

    def test_it_refuses_a_mapping_that_collides(self) -> None:
        with self.assertRaises(ValueError) as cm:
            A.retime(self.table, {0: 0, 2: 3, 4: 3})
        self.assertIn("同一个 tick", str(cm.exception))

    def test_a_mapping_with_extra_entries_is_harmless(self) -> None:
        """多给几条映射（比如整套骨架的表）不该报错——只用得上的那几条。"""
        out = A.retime(self.table, {0: 0, 1: 1, 2: 3, 3: 4, 4: 5})
        self.assertEqual([0, 3, 5], sorted(out))

    def test_the_traversed_poses_are_identical_after_retiming(self) -> None:
        """**搬帧 ≠ 重采样**：每一段走过的姿态集合与段长无关。

        判据：原动画在段内 α 处的取值，必须与重定时后同一段 α 处逐位相等。成立就意味着
        任何**几何**判据（贴棍距离、挡不挡脸、棍头包围盒）在拉长后逐字成立，变的只有
        速度。哪天有人把 `retime` 换成"在新网格上按原曲线重采样"，这条会立刻撞红——
        重采样在非整数倍率下会把落在两个新整数 tick 之间的极值帧插值削掉。
        """
        mapping = {0: 0, 2: 3, 4: 5}
        src_doc = A.build_doc(self.table, name="a", description="",
                              end_tick=4, stop_tick=6)
        dst_doc = A.build_doc(A.retime(self.table, mapping), name="a", description="",
                              end_tick=5, stop_tick=7)
        src_kfs = RA.collect_keyframes(src_doc["emote"])
        dst_kfs = RA.collect_keyframes(dst_doc["emote"])
        segments = [(0, 2), (2, 4)]
        for a, b in segments:
            A2, B2 = mapping[a], mapping[b]
            for i in range(21):
                alpha = i / 20.0
                for axis in ("pitch", "bend"):
                    want = RA.sample_axis(src_kfs, "rightArm", axis,
                                          a + (b - a) * alpha)
                    got = RA.sample_axis(dst_kfs, "rightArm", axis,
                                         A2 + (B2 - A2) * alpha)
                    self.assertAlmostEqual(
                        want, got, places=9,
                        msg=f"段 {a}→{b} 的 α={alpha:.2f} 处 rightArm.{axis} "
                            f"拉长前 {want:.6f} ≠ 拉长后 {got:.6f}")


class ClubSweepRetimingTest(unittest.TestCase):
    """出料的 club_sweep.json 必须和生成器里的设计落位一致。"""

    def test_the_emitted_ticks_match_the_declared_timing(self) -> None:
        emote = json.loads(
            (ANIM / "club_sweep.json").read_text(encoding="utf-8"))["emote"]
        ticks = sorted({int(m["tick"]) for m in emote["moves"]})
        self.assertEqual(sorted(SWEEP.TIMING.values()), ticks,
                         "JSON 里的 tick 和 gen_club_sweep.TIMING 对不上 —— 生成器没重跑")
        self.assertEqual(SWEEP.END_TICK, int(emote["endTick"]))

    def test_the_design_skeleton_itself_is_untouched(self) -> None:
        """POSE 表仍按 8 tick 骨架写——拉长发生在出料一步，不许回写进设计表。

        回写了就再也说不清"这条动画的设计节奏是什么"，下次要改倍率只能凭猜。
        """
        self.assertEqual(SKELETON, sorted(SWEEP.POSE))


if __name__ == "__main__":
    unittest.main()
