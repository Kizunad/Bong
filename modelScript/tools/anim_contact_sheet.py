#!/usr/bin/env python3
"""玩家动画的 **Round 2 人工闸门**：出一张给人看的接触表，然后停下等人一句话。

`bbmodel-contact-sheet` 管的是静态模型；动画这边缺的是同一件东西——一张**跨轮同取景**
的对照图，加上门禁和它的差分自证结果。四样缺一不可（modelScript/README「Round 2 是人工
闸门，不是模型自评」）：

1. **诚实命名的视角**：标签写出这一张实际照到的轴面（FRONT 就是 -z 面），三视角共用
   一个固定取景；
2. **上一轮的同一批 tick、同一个取景** —— 自动取景下每张图各算各的包围盒，跨轮对比
   全是噪声，所以这里扫两轮的**联合**包围盒，两边共用；
3. **门禁结果**（`player_anim_gates`）—— 人写的判据核对姿态，缺一项就红；
4. **门禁的差分自证** —— 报不出自己该抓的缺陷的门算失效，这一栏是给判据本身发的
   体检报告。

工具**不替人做那个判断**。数值门只能回答"有没有"，回答不了"像不像、好不好看、是不是
那个动作"。任何"让模型自己判断像不像"的设计都是自己出题自己判卷。

    python3 modelScript/tools/anim_contact_sheet.py \\
        client/src/main/resources/assets/bong/player_animation/herb_harvest.json \\
        --prev /tmp/prev/herb_harvest.json --profile harvest \\
        --ticks 0,3,6,8,11 --prev-ticks 0,4,8,12
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

LIB = Path(__file__).resolve().parents[1]
from bbmodel_maker import workspace  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
for _d in (LIB / "tools", REPO / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import player_anim_gates as GATES  # noqa: E402
import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402
from bbmodel_maker.render.held_item_render import label_font  # noqa: E402
from bbmodel_maker.render.render_bbmodel import load_bbmodel, render  # noqa: E402

#: 视角标签写的是**实际照到的轴面**。yaw=180 照的是 -z 面，而玩家的脸就在 -z，所以它
#: 确实是 FRONT —— 但标签必须把轴写出来，因为"yaw=180 名义叫 FRONT 实渲 -z 面"这件事
#: 在小草包那轮害人在错的视角上找了三轮不存在的缺陷。
VIEWS = (("FRONT (-z 面)", 180.0, 6.0), ("SIDE_R (-x 面)", 96.0, 6.0), ("3/4", 146.0, 14.0))

#: 表里全是中文，PIL 的内置点阵字体没有 CJK 字形——不换字体，整张接触表的门禁栏会渲成
#: 一排方框，而这张图存在的唯一理由就是给人读。库里的 `label_font` 会去找系统 CJK 字体。
FONT = label_font(13)
FONT_S = label_font(11)
FONT_H = label_font(15)

BG = (16, 17, 20)
FG = (232, 232, 224)
DIM = (150, 152, 158)
OK_C = (120, 200, 130)
BAD_C = (226, 110, 100)


def _kfs(path: Path):
    doc = json.loads(path.read_text(encoding="utf-8"))
    emote = doc.get("emote", doc)
    return RA.collect_keyframes(emote), emote


def _union_focus(scene, ids, held_ids, display, runs, samples=13, margin=1.10):
    """扫**两轮全部帧**取联合包围盒。跨轮对比必须共用取景，否则全是噪声。"""
    lo = np.full(3, np.inf)
    hi = np.full(3, -np.inf)
    for kfs, emote in runs:
        end = float(emote["endTick"])
        for i in range(samples):
            tick = end * i / (samples - 1)
            xform = {ids[n]: m for n, m in P.segment_transforms(kfs, tick).items()}
            hm = P.hand_transform(kfs, tick, display)
            for hid in held_ids:
                xform[hid] = hm
            tris, _, _, _ = load_bbmodel(scene, xform=xform)
            v = np.array([p for vs, _, _ in tris for p in vs])
            lo, hi = np.minimum(lo, v.min(0)), np.maximum(hi, v.max(0))
    centre = (lo + hi) / 2
    span = float(max(hi[0] - lo[0], hi[1] - lo[1])) * margin
    return centre, span


def _tile(scene, ids, held_ids, display, kfs, tick, focus, size):
    xform = {ids[n]: m for n, m in P.segment_transforms(kfs, tick).items()}
    hm = P.hand_transform(kfs, tick, display)
    for hid in held_ids:
        xform[hid] = hm
    return [render(scene, yaw=yaw, pitch=pitch, size=size, xform=xform,
                   focus=focus, shading="mc")[0] for _label, yaw, pitch in VIEWS]


def build(anim: Path, prev: Path | None, hold: Path, profile: str,
          ticks: list[float], prev_ticks: list[float], size: int, out: Path) -> Path:
    kfs, emote = _kfs(anim)
    runs = [(kfs, emote)]
    prev_kfs = None
    if prev is not None:
        prev_kfs, prev_emote = _kfs(prev)
        runs.append((prev_kfs, prev_emote))

    knife_doc = json.loads(hold.read_text(encoding="utf-8"))
    display = knife_doc["display"]["thirdperson_righthand"]
    scene, ids, held_ids = P.build_scene(LIB / "out" / "_anim_sheet_scene.bbmodel", hold)
    focus = _union_focus(scene, ids, held_ids, display, runs)

    rows = []
    for i, tick in enumerate(ticks):
        new_tiles = _tile(scene, ids, held_ids, display, kfs, tick, focus, size)
        old_tiles = ([None] * 3 if prev_kfs is None else
                     _tile(scene, ids, held_ids, display, prev_kfs,
                           prev_ticks[min(i, len(prev_ticks) - 1)], focus, size))
        rows.append((tick, prev_ticks[min(i, len(prev_ticks) - 1)] if prev_kfs else None,
                     new_tiles, old_tiles))

    gates = GATES.run_gates(emote, knife_doc, profile)
    prev_gates = (GATES.run_gates(prev_emote, knife_doc, profile)
                  if prev_kfs is not None else [])

    # ── 排版 ───────────────────────────────────────────────────────────
    gap, head_h, lab_h = 10, 58, 18
    cols = 6
    table_h = 30 + 20 * (len(gates) + 3) + (20 * 10)
    w = gap + cols * (size + gap)
    h = head_h + len(rows) * (size + lab_h + gap) + table_h
    canvas = Image.new("RGB", (w, h), BG)
    d = ImageDraw.Draw(canvas)

    d.text((gap, 10), f"{anim.stem}  ·  Round 2 人工闸门接触表  ·  profile={profile}",
           fill=FG, font=FONT_H)
    d.text((gap, 28), "左=本轮  右=上一轮（同一取景、同一相机）。"
                      "工具只核对数值，像不像那件事请人来判。", fill=DIM, font=FONT)

    x0 = gap
    for c, (label, _yaw, _pitch) in enumerate(VIEWS):
        d.text((x0 + c * 2 * (size + gap) + 4, head_h - 16), label, fill=FG, font=FONT)

    y = head_h
    for tick, ptick, new_tiles, old_tiles in rows:
        for c in range(3):
            x = x0 + c * 2 * (size + gap)
            canvas.paste(new_tiles[c], (x, y + lab_h))
            d.text((x + 4, y), f"本轮 t{tick:g}", fill=OK_C, font=FONT)
            if old_tiles[c] is not None:
                canvas.paste(old_tiles[c], (x + size + gap, y + lab_h))
                d.text((x + size + gap + 4, y), f"上一轮 t{ptick:g}", fill=DIM, font=FONT)
        y += size + lab_h + gap

    y += 6
    d.text((gap, y), "门禁（本轮 → 上一轮同判据）", fill=FG, font=FONT_H)
    y += 22
    for i, r in enumerate(gates):
        mark = "√" if r.ok else "×"   # WQY 没有 ✓/✗ 字形，渲出来是方框
        d.text((gap, y), f"{mark} {r.key:<10s} {r.label:<12s}"
                         f" 实测 {r.worst:8.2f} / 门限 {r.limit:6.2f}   {r.detail}",
               fill=OK_C if r.ok else BAD_C, font=FONT)
        if i < len(prev_gates):
            p = prev_gates[i]
            d.text((w - 320, y), f"上一轮 {p.worst:8.2f} "
                                 f"{'过' if p.ok else '不过'}", fill=DIM, font=FONT)
        y += 20
    y += 10
    d.text((gap, y), "门禁的差分自证（注入缺陷 → 门必须报违例；报不出来的门算失效）", fill=FG, font=FONT_H)
    y += 22
    for line in _self_test_lines(knife_doc):
        d.text((gap, y), line, fill=OK_C if line.lstrip().startswith("√") else BAD_C,
               font=FONT_S)
        y += 20

    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    return out


def _self_test_lines(knife_doc: dict) -> list[str]:
    import contextlib
    import io as _io
    buf = _io.StringIO()
    with contextlib.redirect_stdout(buf):
        GATES.self_test(knife_doc)
    return [ln.replace("✓", "√").replace("✗", "×")
            for ln in buf.getvalue().splitlines() if ln.startswith("  ")]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("json", type=Path)
    ap.add_argument("--prev", type=Path, default=None, help="上一轮的同名动画 JSON")
    ap.add_argument("--hold", type=Path, default=LIB / "models" / "HerbKnifeIron.bbmodel")
    ap.add_argument("--profile", choices=sorted(GATES.PROFILES), default="harvest")
    ap.add_argument("--ticks", default="0,3,6,8,11")
    ap.add_argument("--prev-ticks", default=None,
                    help="上一轮取哪几 tick（缺省沿用 --ticks）")
    ap.add_argument("--size", type=int, default=190)
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()

    ticks = [float(t) for t in args.ticks.split(",")]
    prev_ticks = ([float(t) for t in args.prev_ticks.split(",")]
                  if args.prev_ticks else ticks)
    out = args.out or (LIB / "out" / f"contact_{args.json.stem}_r2.png")
    print(build(args.json, args.prev, args.hold, args.profile,
                ticks, prev_ticks, args.size, out))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
