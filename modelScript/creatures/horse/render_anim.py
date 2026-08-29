#!/usr/bin/env python3
"""马 —— 动画预览：连拍图 + GIF。

取景固定（focus 参数），不用逐帧自动包围盒——后者每帧取景不同，整只会在画面里持续
抖动，任何姿态问题都被这层抖动盖住。地平线画出来，方便一眼看蹄有没有陷地。

用法:
  python3 modelScript/creatures/horse/render_anim.py                    # 全部动画（常马）侧视连拍
  python3 modelScript/creatures/horse/render_anim.py --only walk trot --gif
  python3 modelScript/creatures/horse/render_anim.py --profile large --view 34
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
from gen_skeleton import PROFILES  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from rig import FINAL, Rig, rotmat  # noqa: E402

OUT = HERE / "anim"
VIEWS = {"side": (90.0, 4.0), "34": (140.0, 12.0), "front": (180.0, 4.0), "back": (0.0, 8.0)}


def focus_box(rig: Rig, pad: float = 1.20) -> tuple[np.ndarray, float]:
    """静止姿包围盒 → 固定取景中心与跨度（留余量给腾空 / 人立 / 侧倒）。"""
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
    center[1] = (hi[1] + 0.0) / 2  # 纵向以地面为下界取景
    return center, float((hi - lo).max()) * pad


def screen_row(pt, view: str, size: int, focus) -> float:
    """世界点 → 屏幕行号。与 render 的投影逐字一致（正交，R = Rx(pitch)·Ry(yaw)）。

    别用"地平线 ≈ 画面某比例"这类估算：取景跨度随体型改、俯角一变行号就偏，裁切会把
    蹄切掉，然后看图的人会把切掉的蹄当成穿地。
    """
    yaw, pitch = VIEWS[view]
    center, span = focus
    R = rotmat(pitch, 0) @ rotmat(yaw, 1)
    v = R @ (np.asarray(pt, float) - np.asarray(center, float))
    return size / 2 - v[1] * ((size - 60) / span)


def anim_top(rig: Rig, P, name: str, n: int = 10, L: G.Load = G.BARE) -> float:
    """整段动画里模型的最高世界 y。

    裁切上沿不能按静止姿定：人立那条把头抬到静止高度的 1.7 倍，按静止姿裁会把头切掉，
    然后看图的人会以为"人立没抬起来"。逐帧量一遍最高点再裁。
    """
    top = 0.0
    for i in range(n):
        pose = G.sample(rig, P, name, i / n, L)
        W = rig.world(pose)
        for bn in rig.order:
            pts = rig.bone_points(bn)
            if len(pts):
                top = max(top, float((pts @ W[bn][:3, :3].T + W[bn][:3, 3])[:, 1].max()))
    return top


def frame(rig: Rig, P, src: Path, name: str, t: float, view: str, size: int, focus, pad: int = 26,
          top_y: float | None = None, L: G.Load = G.BARE) -> Image.Image:
    pose = G.sample(rig, P, name, t, L)
    # 背景刻意提亮：马的"黑点"（下肢/鬃/尾）在暗背景上直接隐形，而下肢正是这套
    # 预览唯一要看的东西——蹄有没有贴地、有没有滑步，全在那四条腿上。
    im, _ = render(src, yaw=VIEWS[view][0], pitch=VIEWS[view][1], size=size,
                   xform=rig.element_xform(pose), focus=focus, bg=(96, 99, 104))
    gy = screen_row((0.0, 0.0, 0.0), view, size, focus)
    d = ImageDraw.Draw(im)
    d.line([(0, gy), (size, gy)], fill=(196, 188, 172), width=1)  # 地平线，对照蹄是否贴地
    ty = P.wither * 1.30 if top_y is None else top_y + P.wither * 0.05
    top = int(max(0, screen_row((0.0, ty, 0.0), view, size, focus) - pad))
    bot = int(min(size, gy + pad))
    return im.crop((0, top, size, bot))


def _row(rig: Rig, P, src: Path, name: str, view: str, cols: int, size: int, focus, L: G.Load):
    ty = anim_top(rig, P, name, L=L)
    return [(i / cols, frame(rig, P, src, name, i / cols, view, size, focus, top_y=ty, L=L))
            for i in range(cols)]


def _canvas(rows: list[tuple[str, list]], cols: int, size: int, title: str) -> Image.Image:
    gap, hdr, lab = 6, 16, 13
    W = cols * size + gap * (cols + 1)
    th = rows[0][1][0][1].height
    rh = th + lab + gap
    canvas = Image.new("RGB", (W, hdr + gap + rh * len(rows)), (13, 14, 16))
    d = ImageDraw.Draw(canvas)
    d.text((gap, 3), title, fill=(226, 208, 182))
    for r, (tag, tiles) in enumerate(rows):
        y0 = hdr + gap + r * rh
        d.text((gap, y0), tag, fill=(196, 176, 130))
        for i, (t, im) in enumerate(tiles):
            canvas.paste(im, (gap + i * (size + gap), y0 + lab))
            if r == 0:
                d.text((gap + i * (size + gap) + 3, y0 + lab + 2), f"{t:.2f}", fill=(150, 143, 132))
    return canvas


def contact_sheet(rig: Rig, P, src: Path, name: str, view: str, cols: int, size: int, focus,
                  loads: list[G.Load]) -> Image.Image:
    """连拍。给多个负载档时**逐档一行叠起来**——负载改的是幅度不是姿势，只有同一
    时刻上下并排才看得出差别；分成三张图各看各的，人眼记不住那点差。"""
    length, loop, _n, _fn = G.ANIMS[name]
    rows = [(f"{L.label}（{L.ratio[P.key] * 100:.1f}% 自重）" if L.key else "空载",
             _row(rig, P, src, name, view, cols, size, focus, L)) for L in loads]
    return _canvas(rows, cols, size,
                   f"{name}  {length:.2f}s  {'loop' if loop else 'once'}  [{view}]  {P.label}")


def main() -> int:
    ap = argparse.ArgumentParser(description="马动画预览")
    ap.add_argument("--profile", choices=sorted(PROFILES), default="medium")
    ap.add_argument("--coat", default="rust")
    ap.add_argument("--only", nargs="*", help="只渲染这些动画")
    ap.add_argument("--view", default="side", choices=list(VIEWS))
    ap.add_argument("--cols", type=int, default=8, help="连拍帧数")
    ap.add_argument("--size", type=int, default=300)
    ap.add_argument("--gif", action="store_true", help="同时出 GIF")
    ap.add_argument("--gif-frames", type=int, default=24)
    ap.add_argument("--load", nargs="*", choices=list(G.LOADS),
                    help="负载档（缺省只出空载；给多档则逐档一行叠在同一张里）")
    args = ap.parse_args()

    P = PROFILES[args.profile]
    src = FINAL / f"HorsePelt_{args.coat}_{args.profile}.bbmodel"
    rig = Rig(src)
    focus = focus_box(rig)
    OUT.mkdir(parents=True, exist_ok=True)
    names = args.only or list(G.ANIMS)
    loads = [G.LOADS[k] for k in (args.load if args.load is not None else [""])]
    sfx = "_loads" if len(loads) > 1 else ("" if not loads[0].key else f"_{loads[0].key}")
    for name in names:
        use = [L for L in loads if not L.key or name not in G.NO_LOAD] or [G.BARE]
        sheet = contact_sheet(rig, P, src, name, args.view, args.cols, args.size, focus, use)
        out = OUT / f"anim_{name}_{args.view}{sfx if len(use) > 1 or use[0].key else ''}.png"
        sheet.save(out)
        print(f"  → anim/{out.name}")
        if args.gif:
            length = G.ANIMS[name][0]
            n = args.gif_frames
            L = use[-1]
            ty = anim_top(rig, P, name, L=L)
            frames = [frame(rig, P, src, name, i / n, args.view, args.size, focus, top_y=ty, L=L)
                      for i in range(n)]
            gp = OUT / f"anim_{name}_{args.view}{'' if not L.key else '_' + L.key}.gif"
            frames[0].save(gp, save_all=True, append_images=frames[1:], duration=int(length * 1000 / n), loop=0)
            print(f"  → anim/{gp.name}  ({n} 帧 / {length:.2f}s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
