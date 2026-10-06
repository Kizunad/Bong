#!/usr/bin/env python3
"""把两段预览 GIF 逐帧左右并排，标题用 CJK 字体，输出一段新 GIF（vs_ref 用）。

两段 GIF 的帧数要一致（同一时间表：同样的 tick 与 gif_timing 产出的 duration）。
帧时间逐帧取左边那段的 duration（含 --end-hold-ms 的收势停留帧）。

    python3 modelScript/tools/compose_side_by_side.py \\
        bone_sword_slash/use.gif iron_sword_v2_use/use.gif \\
        --titles "骨剑 bone_sword_slash@UANIM4（新）" "铁剑 iron_sword_v2_use（已通过，同 display）" \\
        --out bone_sword_slash/vs_ref.gif
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageSequence

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cjk_font import load_font  # noqa: E402

TITLE_FONT = load_font(14)
TITLE_H = 26
GAP = 8
BG = (20, 22, 26)
TEXT = (230, 230, 230)


def _frames(path: Path) -> tuple[list[Image.Image], list[int]]:
    """逐帧读出图像与各自的显示时长（毫秒）。预览工具的停留帧时长与其余帧不同，不能只取首帧。"""
    im = Image.open(path)
    frames: list[Image.Image] = []
    durations: list[int] = []
    fallback = int(im.info.get("duration", 48))
    for frame in ImageSequence.Iterator(im):
        frames.append(frame.convert("RGB").copy())
        durations.append(int(frame.info.get("duration", fallback)))
    return frames, durations


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("left", type=Path)
    ap.add_argument("right", type=Path)
    ap.add_argument("--titles", nargs=2, required=True, metavar=("LEFT", "RIGHT"))
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()

    left, durations = _frames(args.left)
    right, _ = _frames(args.right)
    if len(left) != len(right):
        raise SystemExit(f"帧数不一致：{args.left} {len(left)} 帧，{args.right} {len(right)} 帧")

    w, h = left[0].size
    canvas_w = w * 2 + GAP
    canvas_h = h + TITLE_H
    out_frames = []
    for lf, rf in zip(left, right):
        canvas = Image.new("RGB", (canvas_w, canvas_h), BG)
        canvas.paste(lf, (0, TITLE_H))
        canvas.paste(rf, (w + GAP, TITLE_H))
        draw = ImageDraw.Draw(canvas)
        draw.text((8, 6), args.titles[0], fill=TEXT, font=TITLE_FONT)
        draw.text((w + GAP + 8, 6), args.titles[1], fill=TEXT, font=TITLE_FONT)
        out_frames.append(canvas.convert("P", palette=Image.ADAPTIVE, colors=192))

    args.out.parent.mkdir(parents=True, exist_ok=True)
    out_frames[0].save(
        args.out,
        save_all=True,
        append_images=out_frames[1:],
        duration=durations,
        loop=0,
        disposal=2,
    )
    print(f"{args.out}  {len(out_frames)} 帧 / {canvas_w}x{canvas_h}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
