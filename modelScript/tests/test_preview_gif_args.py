"""动画预览工具 GIF 参数的边界测试。

保护的契约：--end-hold-ms 会被加到一次性动画最后一帧的 GIF 时长上，负数会让 Pillow
报错或写出坏 GIF。两个预览工具都必须在命令行解析阶段就拒绝负数（PR #2332 Kody review）。
"""

from __future__ import annotations

import contextlib
import io
import sys
import unittest
from pathlib import Path

LIB_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(LIB_DIR / "tools"))

import creature_anim_frames  # noqa: E402
import preview_player_anim  # noqa: E402

# 每个工具的最小合法参数：只有必填项，好让失败只可能来自 --end-hold-ms
TOOLS = {
    "creature_anim_frames": (creature_anim_frames.build_parser, ["--name", "fuya_v2", "--out", "/tmp/unused"]),
    "preview_player_anim": (preview_player_anim.build_parser, ["unused.json"]),
}


class EndHoldArgumentTest(unittest.TestCase):
    def test_negative_end_hold_is_rejected_and_zero_is_accepted(self) -> None:
        for tool, (build_parser, required) in TOOLS.items():
            with self.subTest(tool=tool):
                parser = build_parser()
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(
                    SystemExit, msg=f"{tool}: --end-hold-ms -1 应当在解析阶段被拒绝"
                ):
                    parser.parse_args([*required, "--end-hold-ms", "-1"])
                self.assertEqual(0, parser.parse_args([*required, "--end-hold-ms", "0"]).end_hold_ms)


if __name__ == "__main__":
    unittest.main()
