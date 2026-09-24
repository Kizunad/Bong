#!/usr/bin/env python3
"""异变缝合兽 —— 核心阶段动画预览：连拍图 + GIF。

固定取景：逐帧自动包围盒会让整只在画面里抖，姿态问题全被抖动盖住。画地平线，
一眼看有没有陷地 / 浮空。

蠕动**必须开 --world 看**：导出的是循环动画（每周期净位移已减掉），不加回位移的话
渲染出来是原地伸缩，看不出它在前进，也看不出锚段有没有蹭地。

用法:
  python3 modelScript/creatures/stitched_beast/render_core_anim.py
  python3 modelScript/creatures/stitched_beast/render_core_anim.py --only core_crawl --world --gif
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

import beast_anim as BA  # noqa: E402
import core_anim as A  # noqa: E402
import head_anim as HA  # noqa: E402
import fragment_anim as FA  # noqa: E402
from bbmodel_maker.rig.anim_rig import Pose, Rig, rotmat  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402

VIEWS = {"side": (90.0, 6.0), "34": (145.0, 18.0), "front": (180.0, 6.0), "top": (90.0, 78.0)}
OUT = HERE / "renders" / "3_core_anim"
OUT_SHARD = HERE / "renders" / "5_shard_anim"

# 渲哪一套：核心（母体）还是碎片。两边接口一样（MODEL / ANIMS / sample），
# 所以整个渲染流程一行不用改，只换模块。
MOD = A


def focus_box(rig: Rig, pad: float = 1.12):
    """固定取景框。

    **排除芽骨**：芽的几何按满尺寸建（growth=1），但每条动画都把它压到
    BUD_DORMANT=0.10。把满尺寸的芽算进取景，整只会被缩到画面中央一小团，
    蠕动的伸缩根本看不出来（实测）。
    """
    W = rig.world(Pose())
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for n in rig.order:
        if n.startswith("bud_"):
            continue
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


def frame(rig: Rig, name: str, t: float, view: str, size: int, focus,
          world: bool = False) -> Image.Image:
    pose = MOD.sample(rig, name, t)
    if world and name.endswith("crawl"):
        # 把每周期净位移加回去：验的是真实轨迹，不是循环曲线
        pose["root"].pos = list(pose["root"].pos)
        pose["root"].pos[2] -= MOD.CRAWL_D * t
    im, _ = render(MOD.MODEL, yaw=VIEWS[view][0], pitch=VIEWS[view][1], size=size,
                   xform=rig.element_xform(pose), focus=focus)
    gy = screen_row((0.0, 0.0, 0.0), view, size, focus)
    d = ImageDraw.Draw(im)
    d.line([(0, gy), (size, gy)], fill=(72, 66, 58), width=1)
    top = int(max(0, screen_row((0.0, 42.0, 0.0), view, size, focus) - 12))
    return im.crop((0, top, size, min(size, int(gy) + 26)))


def contact_sheet(rig: Rig, name: str, view: str, cols: int, size: int, focus,
                  world: bool) -> Image.Image:
    loop = MOD.ANIMS[name][1]
    tiles = [frame(rig, name, i / cols if loop else i / (cols - 1), view, size, focus, world)
             for i in range(cols)]
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
    ap.add_argument("--world", action="store_true", help="蠕动加回每周期净位移（看真实前进）")
    ap.add_argument("--shard", action="store_true", help="改渲碎片那一套动画")
    ap.add_argument("--head", default="", help="改渲某个供体的头颅动画")
    ap.add_argument("--beast", type=int, default=0, help="改渲整兽（带腿走路）动画")
    args = ap.parse_args()

    global MOD, OUT
    if args.shard:
        MOD, OUT = FA, OUT_SHARD
    if args.beast:
        BA.use(args.beast)
        MOD, OUT = BA, HERE / "renders" / "beast_anim" / f"seed{args.beast}"
        OUT.mkdir(parents=True, exist_ok=True)
    if args.head:
        HA.use(args.head)
        # 一个供体一个子目录：不然渲第二个供体会把第一个的同名连拍覆盖掉
        MOD, OUT = HA, HERE / "renders" / "head" / args.head
        OUT.mkdir(parents=True, exist_ok=True)

    if not MOD.MODEL.exists():
        print(f"缺 {MOD.MODEL}，先跑 gen_core.py")
        return 1
    rig = Rig(MOD.MODEL)
    focus = focus_box(rig)
    OUT.mkdir(parents=True, exist_ok=True)
    tag = "_world" if args.world else ""
    for name in (args.only or list(MOD.ANIMS)):
        contact_sheet(rig, name, args.view, args.cols, args.size, focus,
                      args.world).save(OUT / f"{name}_{args.view}{tag}.png")
        print(f"→ {name}_{args.view}{tag}.png")
        if args.gif:
            n = 24
            fr = [frame(rig, name, i / n, args.view, 300, focus, args.world) for i in range(n)]
            h = max(f.height for f in fr)
            fr = [f.crop((0, 0, f.width, h)) for f in fr]
            fr[0].save(OUT / f"{name}_{args.view}{tag}.gif", save_all=True, append_images=fr[1:],
                       duration=int(1000 * MOD.ANIMS[name][0] / n), loop=0)
            print(f"→ {name}_{args.view}{tag}.gif")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
