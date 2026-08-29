#!/usr/bin/env python3
"""怠怒之狮 —— 动画预览：连拍图 + GIF。

取景固定（focus 参数），不用逐帧自动包围盒——后者每帧取景不同，整只会在画面里
持续抖动，任何姿态问题都被这层抖动盖住。地平线画出来，方便一眼看脚有没有陷地。

用法:
  python3 render_anim.py                 # 全部动画，侧视连拍 + GIF
  python3 render_anim.py --only walk run --view 34
  python3 render_anim.py --only run --gif-only --fps 30
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import gen_anim as G  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from rig import PELT, Rig, rotmat  # noqa: E402

VIEWS = {"side": (90.0, 4.0), "34": (140.0, 12.0), "front": (180.0, 4.0), "back": (0.0, 8.0)}


def focus_box(rig: Rig, pad: float = 1.22) -> tuple[np.ndarray, float]:
    """静止姿包围盒 → 固定取景中心与跨度（留余量给腾空/侧翻）。"""
    W = rig.world()
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for n in rig.order:
        pts = rig.bone_points(n)
        if len(pts):
            wp = pts @ W[n][:3, :3].T + W[n][:3, 3]
            lo = np.minimum(lo, wp.min(axis=0))
            hi = np.maximum(hi, wp.max(axis=0))
    center = (lo + hi) / 2
    center[1] = (hi[1] + 0.0) / 2          # 纵向以地面为下界取景
    return center, float((hi - lo).max()) * pad


def screen_row(pt, view: str, size: int, focus) -> float:
    """世界点 → 屏幕行号。与 render 的投影逐字一致（正交，R = Rx(pitch)·Ry(yaw)）。

    别用"地平线 ≈ 画面某比例"这类估算：取景跨度随模型改、俯角一变行号就偏，裁切
    会把爪子切掉，然后看图的人（我）会把切掉的爪子当成穿地。
    """
    yaw, pitch = VIEWS[view]
    center, span = focus
    R = rotmat(pitch, 0) @ rotmat(yaw, 1)
    v = R @ (np.asarray(pt, float) - np.asarray(center, float))
    return size / 2 - v[1] * ((size - 60) / span)


def frame(rig: Rig, name: str, t: float, view: str, size: int, focus, pad: int = 26) -> Image.Image:
    """渲一帧并按投影后的实际内容裁掉上下空白（兽身横长，方画布里只占中间一条）。"""
    pose = G.sample(rig, name, t)
    im, _ = render(PELT, yaw=VIEWS[view][0], pitch=VIEWS[view][1], size=size,
                   xform=rig.element_xform(pose), focus=focus)
    gy = screen_row((0.0, 0.0, 0.0), view, size, focus)
    d = ImageDraw.Draw(im)
    d.line([(0, gy), (size, gy)], fill=(70, 65, 58), width=1)   # 地平线，对照脚是否贴地
    top = int(max(0, screen_row((0.0, 37.0, 0.0), view, size, focus) - pad))
    bot = int(min(size, gy + pad))
    return im.crop((0, top, size, bot))


def contact_sheet(rig: Rig, name: str, view: str, cols: int, size: int, focus) -> Image.Image:
    length, loop, _n, _fn = G.ANIMS[name]
    tiles = [(i / cols, frame(rig, name, i / cols, view, size, focus)) for i in range(cols)]
    gap, hdr = 6, 16
    W = cols * size + gap * (cols + 1)
    th = tiles[0][1].height
    canvas = Image.new("RGB", (W, th + hdr + gap * 2), (13, 14, 16))
    d = ImageDraw.Draw(canvas)
    d.text((gap, 3), f"{name}  {length:.2f}s  {'loop' if loop else 'once'}  [{view}]", fill=(226, 208, 182))
    for i, (t, im) in enumerate(tiles):
        x = gap + i * (size + gap)
        canvas.paste(im, (x, hdr + gap))
        d.text((x + 3, hdr + gap + 2), f"{t * length:.2f}s", fill=(150, 143, 132))
    return canvas


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮动画预览")
    ap.add_argument("--only", nargs="*", help="只渲染这些动画")
    ap.add_argument("--view", default="side", choices=list(VIEWS))
    ap.add_argument("--cols", type=int, default=8, help="连拍帧数")
    ap.add_argument("--size", type=int, default=300)
    ap.add_argument("--gif", action="store_true", help="同时出 GIF")
    ap.add_argument("--gif-only", action="store_true")
    ap.add_argument("--gif-frames", type=int, default=24)
    args = ap.parse_args()

    rig = Rig()
    focus = focus_box(rig)
    names = args.only or list(G.ANIMS)
    for name in names:
        if not args.gif_only:
            sheet = contact_sheet(rig, name, args.view, args.cols, args.size, focus)
            out = HERE / f"anim_{name}_{args.view}.png"
            sheet.save(out)
            print(f"  → {out.name}")
        if args.gif or args.gif_only:
            length = G.ANIMS[name][0]
            n = args.gif_frames
            frames = [frame(rig, name, i / n, args.view, args.size, focus) for i in range(n)]
            out = HERE / f"anim_{name}_{args.view}.gif"
            frames[0].save(out, save_all=True, append_images=frames[1:],
                           duration=int(length * 1000 / n), loop=0)
            print(f"  → {out.name}  ({n} 帧 / {length:.2f}s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
