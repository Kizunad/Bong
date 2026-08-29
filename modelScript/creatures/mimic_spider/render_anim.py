#!/usr/bin/env python3
"""拟态灰烬蛛 —— 动画预览：连拍图 + GIF。

固定取景（focus）：逐帧自动包围盒会让整只在画面里抖，姿态问题全被抖动盖住。
画地平线，一眼看脚有没有陷地 / 浮空。

用法:
  python3 render_anim.py                       # 全部动画，侧视连拍
  python3 render_anim.py --only ambush_burst bite --view 34
  python3 render_anim.py --only walk --gif --fps 24
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
from spider_rig import SHELL, Pose, SpiderRig, rotmat  # noqa: E402

VIEWS = {"side": (90.0, 6.0), "34": (145.0, 18.0), "front": (180.0, 6.0), "top": (90.0, 78.0)}


def focus_box(rig: SpiderRig, pad: float = 1.30):
    W = rig.world(Pose())
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for n in rig.order:
        pts = rig.bone_points(n)
        if len(pts):
            wp = pts @ W[n][:3, :3].T + W[n][:3, 3]
            lo = np.minimum(lo, wp.min(axis=0))
            hi = np.maximum(hi, wp.max(axis=0))
    center = (lo + hi) / 2
    center[1] = (hi[1] + 0.0) / 2
    return center, float((hi - lo).max()) * pad


def screen_row(pt, view: str, size: int, focus) -> float:
    yaw, pitch = VIEWS[view]
    center, span = focus
    R = rotmat(pitch, 0) @ rotmat(yaw, 1)
    v = R @ (np.asarray(pt, float) - np.asarray(center, float))
    return size / 2 - v[1] * ((size - 60) / span)


def frame(rig: SpiderRig, name: str, t: float, view: str, size: int, focus) -> Image.Image:
    pose = G.sample(rig, name, t)
    im, _ = render(SHELL, yaw=VIEWS[view][0], pitch=VIEWS[view][1], size=size,
                   xform=rig.element_xform(pose), focus=focus)
    gy = screen_row((0.0, 0.0, 0.0), view, size, focus)
    d = ImageDraw.Draw(im)
    d.line([(0, gy), (size, gy)], fill=(70, 65, 58), width=1)
    top = int(max(0, screen_row((0.0, 24.0, 0.0), view, size, focus) - 20))
    return im.crop((0, top, size, min(size, int(gy) + 24)))


def contact_sheet(rig: SpiderRig, name: str, view: str, cols: int, size: int, focus) -> Image.Image:
    tiles = [frame(rig, name, i / (cols - 1) if not G.ANIMS[name][1] else i / cols,
                   view, size, focus) for i in range(cols)]
    gap = 4
    th = max(t.height for t in tiles)
    sheet = Image.new("RGB", (cols * size + gap * (cols + 1), th + 2 * gap), (14, 15, 17))
    for i, t in enumerate(tiles):
        sheet.paste(t, (gap + i * (size + gap), gap))
    return sheet


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--view", default="side", choices=sorted(VIEWS))
    ap.add_argument("--cols", type=int, default=8)
    ap.add_argument("--size", type=int, default=240)
    ap.add_argument("--gif", action="store_true")
    ap.add_argument("--fps", type=int, default=20)
    args = ap.parse_args()

    rig = SpiderRig()
    focus = focus_box(rig)
    names = args.only or list(G.ANIMS)
    for name in names:
        sheet = contact_sheet(rig, name, args.view, args.cols, args.size, focus)
        out = HERE / f"anim_{name}_{args.view}.png"
        sheet.save(out)
        print(f"→ {out.name}")
        if args.gif:
            n = 24
            frames = [frame(rig, name, i / n, args.view, 300, focus) for i in range(n)]
            h = max(f.height for f in frames)
            frames = [f.crop((0, 0, f.width, h)) for f in frames]
            gout = HERE / f"anim_{name}_{args.view}.gif"
            frames[0].save(gout, save_all=True, append_images=frames[1:],
                           duration=int(1000 * G.ANIMS[name][0] / n), loop=0)
            print(f"→ {gout.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
