"""并排预览 GIF 的逐帧时长契约：输出每一帧的时长取左边那段的对应帧（PR #2402 Kody review）。

旧实现只读首帧时长，--end-hold-ms 的收势停留帧会按普通帧时长播放，并排预览的时间轴与
左侧原动画错位。这里用一段带停留帧的 GIF 锁住逐帧时长。
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image, ImageSequence

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "tools"))

import compose_side_by_side  # noqa: E402

# 两帧动画 + 一帧收势停留。每帧颜色不同：Pillow 会合并相同的相邻帧，时长会被叠加。
DURATIONS = [50, 50, 500]


def _write_gif(path: Path, durations: list[int]) -> None:
    frames = [Image.new("RGB", (8, 8), (40 * (i + 1), 0, 0)) for i in range(len(durations))]
    frames[0].save(path, save_all=True, append_images=frames[1:], duration=durations, loop=0)


class ComposeSideBySideDurationTest(unittest.TestCase):
    def test_output_keeps_each_frame_duration_of_left_clip(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            left = Path(tmp) / "left.gif"
            right = Path(tmp) / "right.gif"
            out = Path(tmp) / "out.gif"
            _write_gif(left, DURATIONS)
            _write_gif(right, DURATIONS)
            argv = [
                "compose_side_by_side.py",
                str(left),
                str(right),
                "--titles",
                "左",
                "右",
                "--out",
                str(out),
            ]
            with mock.patch.object(sys, "argv", argv):
                self.assertEqual(compose_side_by_side.main(), 0)
            with Image.open(out) as im:
                got = [frame.info["duration"] for frame in ImageSequence.Iterator(im)]
            self.assertEqual(
                got,
                DURATIONS,
                "并排 GIF 的逐帧时长应与左侧原动画一致（含收势停留帧）",
            )


if __name__ == "__main__":
    unittest.main()
