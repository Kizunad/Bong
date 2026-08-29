#!/usr/bin/env python3
"""`held_item_pose` 求解台的**策略**锁：哪些约束是闸门、哪些是权重。

这两类混起来是这个模块最容易被"顺手修正"的地方——两者看起来都叫"容差"，改错一个不会
报错，只会让求解台开始说谎：

- `tol`（左腕离棍身轴线的 px）是**闸门**。够不着就是够不着，够不着的候选留下来只会让
  "副手握着棍"变成一句空话。淘汰干净，整帧无解就报 `None`。
- `near_tol`（帧间最大转角）是**权重**。一次真的抡击本来就要求手臂在两 tick 内转过大
  角度；把它当闸门用，撞击帧会直接判成无解，工具于是报"这一招做不出来"——而那一招已经
  出料了（`club_sweep` 的 t6/t7 实测正是这么被判死的，见
  `test_a_hard_gate_on_the_swing_would_declare_the_shipped_impact_frame_unsolvable`）。

所以：贴棍距离和帧间转角**一起进目标函数**，转角原样报出来交人判断。本文件把这条策略
钉死，以及把两个 `near_tol` 入口的非正值挡在门外——它一个是分母、一个是区间半径，非正
值在两处都是静默错误，不是异常。
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
REPO = LIB_DIR.parent
for _d in (LIB_DIR / "generators", LIB_DIR / "tools", REPO / "client" / "tools"):
    sys.path.insert(0, str(_d))

import held_item_pose as HIP  # noqa: E402
import gen_club_sweep as SWEEP  # noqa: E402

# 出料时用的就是这个默认值；下面几条都拿它说事，写死在这里免得读的人以为是随手挑的。
PRODUCTION_NEAR_TOL = 34.0
# 副手网格步长：12° 够粗，跑得快，且和 `solve --two-hand` 里那一档一致。
STEP = 12


def _sweep_right_arm_track() -> list:
    """club_sweep 出料的右臂逐帧姿态——副手就是照着这条轨迹解出来的。"""
    return [dict(pitch=pose["rightArm"]["pitch"], yaw=pose["rightArm"]["yaw"],
                 roll=pose["rightArm"]["roll"], bend=pose["rightArm"]["bend"],
                 axis=180)
            for _tick, pose in sorted(SWEEP.POSE.items())]


class OffHandChainPolicyTest(unittest.TestCase):
    """`solve_off_hand_chain`：闸门 vs 权重。"""

    @classmethod
    def setUpClass(cls) -> None:
        cls.item, cls.display, _tip, _grip = HIP.load_item("wooden_club")
        cls.rights = _sweep_right_arm_track()

    def _chain(self, **kw):
        params = dict(span=(0.20, 0.65), near_tol=PRODUCTION_NEAR_TOL,
                      step=STEP, tol=3.0)
        params.update(kw)
        return HIP.solve_off_hand_chain(self.item, self.display, self.rights, **params)

    def test_the_shipped_sweep_solves_on_every_frame(self) -> None:
        """基准：出料参数下每一帧都解得出副手。后面几条都以这个为对照。"""
        rows = self._chain()
        self.assertEqual(len(self.rights), len(rows))
        unsolved = [i for i, r in enumerate(rows) if r is None]
        self.assertEqual([], unsolved,
                         f"第 {unsolved} 帧解不出副手——club_sweep 是照这条链出料的，"
                         "解不出就说明求解台和出料资产已经对不上了")

    def test_the_swing_is_reported_not_hidden(self) -> None:
        """每帧把"本帧最大转角"原样返回（第 7 项），首帧按定义是 0。

        这是"不硬过滤"能成立的前提：不替人拍板，但必须把判断需要的数字给他。
        """
        rows = self._chain()
        self.assertEqual(0.0, rows[0][6], "首帧没有'上一帧'，转角按定义是 0")
        for i, (prev, row) in enumerate(zip(rows, rows[1:]), start=1):
            want = max(abs(row[j + 1] - prev[j + 1]) for j in range(4))
            self.assertAlmostEqual(
                want, row[6], places=6,
                msg=f"第 {i} 帧报的转角 {row[6]} 和逐轴算出来的 {want} 对不上")

    def test_a_swing_larger_than_near_tol_is_allowed(self) -> None:
        """**这条就是策略本身**：转角超过 `near_tol` 的解照样可以被选中。

        哪天有人把加权取舍改成"评分前按 near_tol 过滤"，这条立刻撞红。
        """
        rows = self._chain()
        over = [(i, r[6]) for i, r in enumerate(rows)
                if r is not None and r[6] > PRODUCTION_NEAR_TOL]
        self.assertTrue(
            over,
            f"出料参数（near_tol={PRODUCTION_NEAR_TOL}）下竟然没有任何一帧超容差——"
            "要么求解台被改成硬过滤了，要么 club_sweep 的右臂轨迹变了")

    def test_a_hard_gate_on_the_swing_would_declare_the_shipped_impact_frame_unsolvable(
            self) -> None:
        """把建议里的硬过滤照做一遍，看它对**已经出料的动画**做了什么。

        这条不测生产代码，它测的是"为什么不那么写"：同一条右臂轨迹、同一组参数，硬过滤
        之后有帧被判成无解。求解台于是报"这一招做不出来"——而这一招已经在
        `player_animation/club_sweep.json` 里了。
        """
        soft = self._chain()
        hard, prev = [], None
        for right in self.rights:
            rows = [r for r in HIP.solve_off_hand(self.item, self.display, right,
                                                  span=(0.20, 0.65), top=4000, step=STEP)
                    if r[0] <= 3.0]
            if prev is not None:
                rows = [r for r in rows
                        if max(abs(r[i + 1] - prev[i + 1]) for i in range(4))
                        <= PRODUCTION_NEAR_TOL]
            if not rows:
                hard.append(None)
                continue
            best = min(rows, key=lambda r: r[0])
            hard.append(best)
            prev = best

        self.assertEqual([], [i for i, r in enumerate(soft) if r is None],
                         "对照组：加权取舍下不该有无解帧")
        self.assertTrue(
            [i for i, r in enumerate(hard) if r is None],
            "硬过滤竟然也全解得出——那这条'为什么不硬过滤'的论据已经过期，"
            "回去重新量，别照抄结论")

    # ── near_tol 的入口校验 ─────────────────────────────────────────────────

    def test_the_chain_refuses_a_non_positive_near_tol(self) -> None:
        """它是分母：0 当场除零，负数把"转得越多分越高"，静默解出乱抽的副手。"""
        for bad in (0.0, -1.0, -34.0):
            with self.assertRaises(ValueError, msg=f"near_tol={bad} 竟然被接受了") as cm:
                HIP.solve_off_hand_chain(self.item, self.display, self.rights[:2],
                                         near_tol=bad, step=STEP)
            self.assertIn("near_tol", str(cm.exception))

    def test_the_right_arm_solver_refuses_a_non_positive_near_tol(self) -> None:
        """另一个入口：那里 `near_tol` 是 `--near` 盒子的半径，负数让区间首尾颠倒，
        扫描空转，读起来却像"这个目标够不到"。"""
        for bad in (0.0, -5.0):
            with self.assertRaises(ValueError) as cm:
                HIP.solve(self.display, None, None, up=(0, 1), right=(0, 1),
                          forward=(-1e9, 1e9), elev=(-90, 90), bend_range=(8, 20),
                          hand_forward=-1e9, near={"pitch": 0.0}, near_tol=bad)
            self.assertIn("near_tol", str(cm.exception))

    def test_a_positive_near_tol_is_still_accepted_by_the_right_arm_solver(self) -> None:
        """边界另一侧：正值不许被误伤（哪怕小到只剩一个格子）。"""
        rows = HIP.solve(self.display, *HIP.load_item("wooden_club")[2:],
                         up=(-1e9, 1e9), right=(-1e9, 1e9), forward=(-1e9, 1e9),
                         elev=(-90, 90), bend_range=(8, 20), hand_forward=-1e9,
                         near={"pitch": 0.0, "yaw": 0.0, "roll": 0.0}, near_tol=1.0,
                         top=3)
        self.assertTrue(rows, "near_tol=1 的一格盒子应当仍能解出姿态")


class OffHandDistanceGateTest(unittest.TestCase):
    """对照组：`tol` 是**闸门**，够不着就该报无解，不许"尽量选个近的"糊过去。"""

    @classmethod
    def setUpClass(cls) -> None:
        cls.item, cls.display, _tip, _grip = HIP.load_item("wooden_club")
        cls.rights = _sweep_right_arm_track()

    def test_an_unreachable_tolerance_comes_back_as_none(self) -> None:
        rows = HIP.solve_off_hand_chain(self.item, self.display, self.rights,
                                        span=(0.20, 0.65),
                                        near_tol=PRODUCTION_NEAR_TOL, step=STEP,
                                        tol=0.0001)
        self.assertTrue(any(r is None for r in rows),
                        "离轴容差压到 0.0001px 还帧帧有解，说明 tol 没在闸")

    def test_loosening_the_gate_brings_the_frames_back(self) -> None:
        """闸门是单调的：放宽只会让解更多，不会更少。"""
        tight = HIP.solve_off_hand_chain(self.item, self.display, self.rights,
                                         span=(0.20, 0.65),
                                         near_tol=PRODUCTION_NEAR_TOL, step=STEP,
                                         tol=0.5)
        loose = HIP.solve_off_hand_chain(self.item, self.display, self.rights,
                                         span=(0.20, 0.65),
                                         near_tol=PRODUCTION_NEAR_TOL, step=STEP,
                                         tol=3.0)
        self.assertLessEqual(sum(r is None for r in loose),
                             sum(r is None for r in tight),
                             "放宽 tol 之后无解帧反而变多了——闸门不单调")


if __name__ == "__main__":
    unittest.main()
