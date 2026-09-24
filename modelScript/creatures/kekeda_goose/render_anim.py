#!/usr/bin/env python3
"""珂珂达 —— 动画预览：连拍图 + GIF，落在 renders/5_anim/ 下。

取景固定（focus 参数），不用逐帧自动包围盒 —— 后者每帧取景不同，整只会在画面里持续
抖动，任何姿态问题都被这层抖动盖住。地平线画出来，一眼看脚有没有陷地。

渲的是**导出的关键帧插值**，不是解析采样器 —— 和 check_anim 一个口径，看到的就是
引擎会播的那条曲线。

面亮度用 MC 原版口径（shading="mc"）：默认的 lambert 光让相邻朝向差 2.5 倍，会把
体素的阶梯面照成一身条纹，判姿态时容易误判成模型有问题。

用法:
  python3 modelScript/creatures/kekeda_goose/render_anim.py                 # 全部，侧视连拍
  python3 modelScript/creatures/kekeda_goose/render_anim.py --only poop lay_egg --gif
  python3 modelScript/creatures/kekeda_goose/render_anim.py --only walk --view 34
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
from bbmodel_maker.rig.anim_rig import rotmat  # noqa: E402
from check_anim import Exported  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from rig import PLUME, Goose  # noqa: E402

OUT = HERE / "renders" / "5_anim"
VIEWS = {"side": (90.0, 6.0), "34": (138.0, 14.0), "front": (180.0, 6.0), "back": (0.0, 10.0)}
BG = (13, 14, 16)
INK = (226, 208, 182)
DIM = (150, 143, 132)
MARK = (206, 138, 74)      # 释放帧的标记色


def focus_box(g: Goose, pad: float = 1.30) -> tuple[np.ndarray, float]:
    """静止姿包围盒 → 固定取景中心与跨度（留余量给蹲下 / 张翼 / 侧翻）。"""
    W = g.world()
    lo, hi = np.array([1e9] * 3), np.array([-1e9] * 3)
    for n in g.order:
        pts = g.bone_points(n)
        if len(pts):
            wp = pts @ W[n][:3, :3].T + W[n][:3, 3]
            lo, hi = np.minimum(lo, wp.min(axis=0)), np.maximum(hi, wp.max(axis=0))
    center = (lo + hi) / 2
    center[1] = hi[1] / 2                      # 纵向以地面为下界取景
    return center, float((hi - lo).max()) * pad


def screen_row(pt, view: str, size: int, focus) -> float:
    """世界点 → 屏幕行号。与 render 的投影逐字一致（正交，R = Rx(pitch)·Ry(yaw)）。

    别用"地平线 ≈ 画面某比例"这类估算：取景跨度随模型改、俯角一变行号就偏，裁切会把
    脚切掉，然后看图的人（我）会把切掉的脚当成穿地。
    """
    yaw, pitch = VIEWS[view]
    center, span = focus
    v = rotmat(pitch, 0) @ rotmat(yaw, 1) @ (np.asarray(pt, float) - np.asarray(center, float))
    return size / 2 - v[1] * ((size - 60) / span)


def frame(g: Goose, ex: Exported, t: float, view: str, size: int, focus, pad: int = 24):
    im, _ = render(PLUME, yaw=VIEWS[view][0], pitch=VIEWS[view][1], size=size,
                   xform=g.element_xform(ex.at(t)), focus=focus, shading="mc")
    gy = screen_row((0.0, 0.0, 0.0), view, size, focus)
    ImageDraw.Draw(im).line([(0, gy), (size, gy)], fill=(70, 65, 58), width=1)
    top = int(max(0, screen_row((0.0, 24.0, 0.0), view, size, focus) - pad))
    return im.crop((0, top, size, int(min(size, gy + pad))))


def contact_sheet(g: Goose, name: str, view: str, cols: int, size: int, focus) -> Image.Image:
    length, loop, _n, _fn = G.ANIMS[name]
    ex = Exported(g, name)
    rel = G.RELEASE.get(name)
    tiles = [(i / cols, frame(g, ex, i / cols, view, size, focus)) for i in range(cols)]
    # 释放帧单独插一格：它多半不落在均匀采样点上，而那一帧正是这段动画的意义所在
    if rel is not None:
        tiles.append((rel, frame(g, ex, rel, view, size, focus)))
        tiles.sort(key=lambda kv: kv[0])
    gap, hdr = 6, 16
    th = tiles[0][1].height
    canvas = Image.new("RGB", (len(tiles) * (size + gap) + gap, th + hdr + gap * 2), BG)
    d = ImageDraw.Draw(canvas)
    d.text((gap, 3), f"{name}  {length:.2f}s  {'loop' if loop else 'once'}  [{view}]", fill=INK)
    for i, (t, im) in enumerate(tiles):
        x = gap + i * (size + gap)
        canvas.paste(im, (x, hdr + gap))
        hit = rel is not None and abs(t - rel) < 1e-6
        d.text((x + 3, hdr + gap + 2), f"{t * length:.2f}s" + (" <RELEASE" if hit else ""),
               fill=MARK if hit else DIM)
        if hit:
            d.rectangle([x, hdr + gap, x + size - 1, hdr + gap + th - 1], outline=MARK)
    return canvas


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达动画预览")
    ap.add_argument("--only", nargs="*", help="只渲染这些动画")
    ap.add_argument("--view", default="side", choices=list(VIEWS))
    ap.add_argument("--cols", type=int, default=9, help="连拍帧数")
    ap.add_argument("--size", type=int, default=270)
    ap.add_argument("--gif", action="store_true", help="同时出 GIF")
    ap.add_argument("--gif-frames", type=int, default=28)
    args = ap.parse_args()

    g = Goose()
    focus = focus_box(g)
    OUT.mkdir(parents=True, exist_ok=True)
    for name in args.only or list(G.ANIMS):
        sheet = contact_sheet(g, name, args.view, args.cols, args.size, focus)
        out = OUT / f"{name}_{args.view}.png"
        sheet.save(out)
        print(f"  → renders/5_anim/{out.name}")
        if args.gif:
            ex, n = Exported(g, name), args.gif_frames
            length = G.ANIMS[name][0]
            fr = [frame(g, ex, i / n, args.view, args.size, focus) for i in range(n)]
            out = OUT / f"{name}_{args.view}.gif"
            fr[0].save(out, save_all=True, append_images=fr[1:],
                       duration=int(length * 1000 / n), loop=0)
            print(f"  → renders/5_anim/{out.name}  ({n} 帧 / {length:.2f}s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
