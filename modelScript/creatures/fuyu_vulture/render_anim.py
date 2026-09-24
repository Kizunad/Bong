#!/usr/bin/env python3
"""腐羽鹫 —— 动画预览：连拍图 + GIF。

取景**固定**（focus 参数），不用逐帧自动包围盒 —— 后者每帧取景不同，整只会在画面里
持续抖动，任何姿态问题都被这层抖动盖住。地平线画出来，一眼看脚有没有陷地。

收翼 / 展翼两套绑定姿的包围盒差着一倍多（展翼翼展是体长的两倍），所以两类动作各用各
的取景，不然地面动作会缩成画面中间一小坨。

用法:
  python3 modelScript/creatures/fuyu_vulture/render_anim.py                    # 全部，侧视连拍
  python3 modelScript/creatures/fuyu_vulture/render_anim.py --only walk --gif
  python3 modelScript/creatures/fuyu_vulture/render_anim.py --view 34 --size large
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
from bbmodel_maker.rig.animkit import Pose, rotmat  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from rig import VultureRig  # noqa: E402

VIEWS = {"side": (90.0, 6.0), "34": (138.0, 14.0), "front": (180.0, 6.0),
         "back": (0.0, 10.0), "top": (180.0, 86.0)}


def focus_box(rig: VultureRig, pad: float = 1.20) -> tuple[np.ndarray, float]:
    """静止姿包围盒 → 固定取景中心与跨度（留余量给腾空 / 侧翻 / 展翼）。"""
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
    center[1] = (hi[1] + 0.0) / 2          # 纵向以地面为下界取景
    return center, float((hi - lo).max()) * pad


def screen_row(pt, view: str, size: int, focus) -> float:
    """世界点 → 屏幕行号。与 render 的投影逐字一致（正交，R = Rx(pitch)·Ry(yaw)）。

    别用"地平线 ≈ 画面某比例"这类估算：取景跨度随档位改、俯角一变行号就偏，裁切会把
    爪子切掉，然后看图的人（我）会把切掉的爪子当成穿地。
    """
    yaw, pitch = VIEWS[view]
    center, span = focus
    R = rotmat(pitch, 0) @ rotmat(yaw, 1)
    v = R @ (np.asarray(pt, float) - np.asarray(center, float))
    return size / 2 - v[1] * ((size - 60) / span)


def frame(rig: VultureRig, name: str, t: float, view: str, size: int, focus,
          crop: bool = True, pad: int = 22) -> Image.Image:
    pose = G.sample(rig, name, t)
    im, _ = render(rig.path, yaw=VIEWS[view][0], pitch=VIEWS[view][1], size=size,
                   xform=rig.element_xform(pose), focus=focus)
    gy = screen_row((0.0, 0.0, 0.0), view, size, focus)
    d = ImageDraw.Draw(im)
    d.line([(0, gy), (size, gy)], fill=(74, 68, 60), width=1)   # 地平线，对照脚是否贴地
    if not crop or view == "top":
        return im
    top_y = float(rig.bones["skull"].origin[1]) * 2.1
    top = int(max(0, screen_row((0.0, top_y, 0.0), view, size, focus) - pad))
    bot = int(min(size, gy + pad))
    return im.crop((0, top, size, bot)) if bot - top > 40 else im


def contact_sheet(rig: VultureRig, name: str, view: str, cols: int, size: int,
                  focus) -> Image.Image:
    clip = G.ANIMS[name]
    tiles = [(i / cols, frame(rig, name, i / cols, view, size, focus)) for i in range(cols)]
    gap, hdr = 6, 16
    width = cols * size + gap * (cols + 1)
    th = tiles[0][1].height
    canvas = Image.new("RGB", (width, th + hdr + gap * 2), (13, 14, 16))
    d = ImageDraw.Draw(canvas)
    d.text((gap, 3), f"{name}  {clip.length:.2f}s  {'loop' if clip.loop else 'once'}"
                     f"  [{view}]  {clip.kind}", fill=(226, 208, 182))
    for i, (t, im) in enumerate(tiles):
        x = gap + i * (size + gap)
        canvas.paste(im, (x, hdr + gap))
        d.text((x + 3, hdr + gap + 2), f"{t * clip.length:.2f}s", fill=(150, 143, 132))
    return canvas


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫动画预览")
    ap.add_argument("--only", nargs="*", help="只渲染这些动画")
    ap.add_argument("--size", default="mid", choices=G.SIZES)
    ap.add_argument("--morph", default="jin")
    ap.add_argument("--view", default="side", choices=list(VIEWS))
    ap.add_argument("--cols", type=int, default=8, help="连拍帧数")
    ap.add_argument("--px", type=int, default=300)
    ap.add_argument("--gif", action="store_true", help="同时出 GIF")
    ap.add_argument("--gif-only", action="store_true")
    ap.add_argument("--gif-frames", type=int, default=24)
    args = ap.parse_args()

    rigs: dict[str, tuple[VultureRig, tuple]] = {}
    for kind in ("ground", "flight"):
        r = G.default_rig(args.size, args.morph, spread=kind == "flight")
        rigs[kind] = (r, focus_box(r))

    for name in (args.only or list(G.ANIMS)):
        rig, focus = rigs[G.ANIMS[name].kind]
        if not args.gif_only:
            sheet = contact_sheet(rig, name, args.view, args.cols, args.px, focus)
            out = HERE / f"anim_{name}_{args.view}.png"
            sheet.save(out)
            print(f"  → {out.name}")
        if args.gif or args.gif_only:
            n = args.gif_frames
            length = G.ANIMS[name].length
            frames = [frame(rig, name, i / n, args.view, args.px, focus) for i in range(n)]
            out = HERE / f"anim_{name}_{args.view}.gif"
            frames[0].save(out, save_all=True, append_images=frames[1:],
                           duration=int(length * 1000 / n), loop=0)
            print(f"  → {out.name}  ({n} 帧 / {length:.2f}s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
