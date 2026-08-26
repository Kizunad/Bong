#!/usr/bin/env python3
"""`anim_common.integer_retime` —— 把一条设计好的动画整体拉长 / 压缩时，帧该落在哪儿。

这个函数存在的理由是一条运行时硬约束：**PlayerAnimator 的 tick 是整数**
（`AnimationJson.java:123` 的 `getAsInt()`、`KeyframeAnimation.java:451/469` 的
`findAtTick(int)` / `addKeyFrame(int, ...)`）。写小数进 JSON 不会报错——会被截断，然后
和相邻整数帧**撞成同一帧**，静默丢关键帧。于是"拉长 1.2 倍"这件事没有精确解，只能求
误差最小的整数落位，而"误差"该怎么定义就是这里锁的东西。

三条设计意图：

1. **误差按累计位置算，不按段长算。** 逐段四舍五入再累加，误差会一路攒下去；对累计
   位置取整则保证任何一帧的时间误差 ≤ 0.5 tick。
2. **`keep_gap` 是给"必须紧跟"的段用的**（overshoot 贴着 impact 后一 tick，
   conventions §2.6），被拉成 2 tick 就不再是弹性过冲。
3. **落位解完写进生成器的 POSE 键，不在出料一步搬帧。** 全仓生成器都满足「POSE 的键 ==
   出料 JSON 的 tick」，`bbmodel_to_pose` 的回程靠的就是这条等式（不变量本身由
   `PoseTickContractTest` 对全部生成器钉住）。所以这里只提供**求落位**，`ClubSweepRetimingTest`
   反过来核验 `gen_club_sweep` 里那组 tick 确实是这个解。

还有一条不在函数里、但必须有测试守住的：**拉长是搬帧，不是重采样。** 姿态一个数都不
改，所以每一段走过的姿态集合与段长无关，贴棍距离、挡不挡脸、包围盒这些几何判据在拉长
后逐字成立，变的只有速度。这条直接拿**出料的 club_sweep.json** 和它 8 tick 草稿逐段
对拍（见 `test_stretching_changed_only_the_speed_not_a_single_pose`）——哪天有人改成
"在新网格上按原曲线重采样"，那条会立刻撞红：重采样在非整数倍率下会把落在两个新整数
tick 之间的 LOAD / IMPACT 极值插值削掉。
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


class ClubSweepRetimingTest(unittest.TestCase):
    """`gen_club_sweep` 的那次拉长：落位、出料、以及"只改了速度"。"""

    def _mapping(self) -> dict:
        """生成器自报的那次求解，原样跑一遍。"""
        return A.integer_retime(SWEEP.DESIGN_TICKS, SWEEP.TIME_SCALE,
                                keep_gap=SWEEP.KEEP_GAP)

    def test_the_declared_stretch_reproduces_the_pose_ticks(self) -> None:
        """POSE 的键必须正好是「设计骨架 × TIME_SCALE」的解。

        POSE 键改了却没重新解、或者倍率改了却没重排帧，都在这里撞红——这是"设计节奏"
        这件事在代码里唯一的锚点（拉长已经写进键里，光看 POSE 是看不出原骨架的）。
        """
        self.assertEqual(sorted(self._mapping().values()), sorted(SWEEP.POSE))

    def test_ten_ticks_is_the_nearest_integer_to_the_requested_1_2x(self) -> None:
        """为什么是 10 不是 9.6：整数网格上离 1.2× 最近的一档。"""
        ideal = max(SWEEP.DESIGN_TICKS) * 1.2
        self.assertEqual(10, SWEEP.END_TICK)
        self.assertLessEqual(abs(SWEEP.END_TICK - ideal), 0.5,
                             f"末帧 {SWEEP.END_TICK} 离 1.2× 的理想位置 {ideal} 太远")

    def test_the_emitted_json_matches_the_pose_table(self) -> None:
        emote = json.loads(
            (ANIM / "club_sweep.json").read_text(encoding="utf-8"))["emote"]
        ticks = sorted({int(m["tick"]) for m in emote["moves"]})
        self.assertEqual(sorted(SWEEP.POSE), ticks,
                         "JSON 里的 tick 和 gen_club_sweep.POSE 对不上 —— 生成器没重跑")
        self.assertEqual(SWEEP.END_TICK, int(emote["endTick"]))

    def test_stretching_changed_only_the_speed_not_a_single_pose(self) -> None:
        """**搬帧 ≠ 重采样**所依赖的那条前提，拿出料资产验。

        前提是：段内插值**只看段内进度 α**，不看段有多长——于是一段走过的姿态集合
        `{lerp(v0, v1, ease(α)) : α ∈ [0,1]}` 与段长无关。判据：把出料的 10 tick 表按
        落位搬回 8 tick 草稿，两条在同一段的同一 α 处必须**逐轴逐位**相等。

        这条成立，"拉长只改了速度"才是真的，这条动画在 8 tick 上做过的几何量测（副手
        贴棍 ≤1.38px、不挡脸、棍头包围盒 25.4×10.9×8.6）才能不重测就搬到 10 tick 上用。
        哪天有人把 easing 改成带绝对时长的（比如按 tick 数而不是按 α 插值），或者把
        拉长改成"在新网格上按原曲线重采样"，这条立刻撞红。

        （"POSE 键必须就是出料 tick"由上面那两条管；这里只管插值本身的性质。）
        """
        mapping = self._mapping()
        inverse = {dst: src for src, dst in mapping.items()}
        stray = sorted(set(SWEEP.POSE) - set(inverse))
        self.assertEqual([], stray,
                         f"POSE 里的 tick {stray} 不在落位解的像里——先看上面那条"
                         "test_the_declared_stretch_reproduces_the_pose_ticks")
        draft = {inverse[tick]: pose for tick, pose in SWEEP.POSE.items()}

        shipped_doc = A.build_doc(SWEEP.POSE, name="x", description="",
                                  end_tick=SWEEP.END_TICK, stop_tick=SWEEP.END_TICK + 2)
        draft_doc = A.build_doc(draft, name="x", description="",
                                end_tick=max(draft), stop_tick=max(draft) + 2)
        shipped = RA.collect_keyframes(shipped_doc["emote"])
        drafted = RA.collect_keyframes(draft_doc["emote"])

        self.assertEqual(sorted(drafted), sorted(shipped),
                         "搬回草稿之后 part 集合都不一样了，后面的逐轴对拍没有意义")
        design = list(SWEEP.DESIGN_TICKS)
        checked = 0
        for a, b in zip(design, design[1:]):
            dst_a, dst_b = mapping[a], mapping[b]
            for part, axes in drafted.items():
                for axis in axes:
                    for i in range(21):
                        alpha = i / 20.0
                        want = RA.sample_axis(drafted, part, axis, a + (b - a) * alpha)
                        got = RA.sample_axis(shipped, part, axis,
                                             dst_a + (dst_b - dst_a) * alpha)
                        self.assertAlmostEqual(
                            want, got, places=9,
                            msg=f"段 {a}→{b}（出料 {dst_a}→{dst_b}）的 α={alpha:.2f} 处 "
                                f"{part}.{axis}：草稿 {want:.6f} ≠ 出料 {got:.6f}")
                        checked += 1
        self.assertGreater(checked, 1000,
                           f"只对拍了 {checked} 个采样点——采样循环八成是空转的")


if __name__ == "__main__":
    unittest.main()
