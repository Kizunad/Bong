"""预览 GIF 时间表的契约：一轮总时长 = 动画时长 + 收势停留（PR #2332 Kody review）。

旧实现给一次性动画补的收势帧也算了一个整帧时长，再叠加停留，于是每段一次性动画都
凭空多播一帧；--end-hold-ms 0 时也会多停一帧。两个预览工具共用 gif_schedule，这里锁它。
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "tools"))

from gif_timing import gif_schedule  # noqa: E402

FRAME_MS = 50
COUNT = 12  # 一段 0.6 秒的动画，20fps
ANIMATION_MS = COUNT * FRAME_MS


class GifScheduleTest(unittest.TestCase):
    def test_total_duration_is_animation_length_plus_hold(self) -> None:
        for looped, hold, expected in (
            (False, 500, ANIMATION_MS + 500),
            (False, 0, ANIMATION_MS),
            (True, 500, ANIMATION_MS),  # 循环动画不停留
        ):
            with self.subTest(looped=looped, hold=hold):
                schedule = gif_schedule(0.6, COUNT, FRAME_MS, looped, hold)
                self.assertEqual(expected, sum(duration for _, duration in schedule),
                                 "一轮 GIF 总时长必须等于动画时长 + 收势停留，不能多出一帧")

    def test_hold_frame_shows_the_final_pose_only_when_holding(self) -> None:
        with_hold = gif_schedule(0.6, COUNT, FRAME_MS, False, 500)
        self.assertEqual((0.6, 500), with_hold[-1], "收势帧应落在动画末尾，只承担停留时长")
        without_hold = gif_schedule(0.6, COUNT, FRAME_MS, False, 0)
        self.assertEqual(COUNT, len(without_hold), "没有停留时不应追加收势帧")


if __name__ == "__main__":
    unittest.main()
