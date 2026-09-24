#!/usr/bin/env python3
"""匕首动画的 Round 2 **人工闸门**：出一张给人看的接触表，然后停下等人一句话。

和 `bbmodel_maker.workbench.contact_sheet` 同一套规矩，只是被测对象从静态模型换成
一条动画，所以多了一个「时间」维度：每一行是一个关键 tick，每一列是一个视角。

表里必须有四样（缺一样这张表就不配叫闸门）：

1. **诚实命名的视角**。列头写的是实际照到的轴面，不是"FRONT"这种口头约定 ——
   `render_bbmodel` 历史上把 yaw=180 叫 FRONT，等价于假定所有资产都朝 −z；玩家模型
   确实朝 −z，所以这里 facing="-z"，但**标签仍然把轴面写出来**，让人能自己核。
   `preview_player_anim.VIEWS` 只有三个视角、且那个叫 "SIDE" 的（yaw=96）没写明照的
   是哪一面 —— 它实际是 SIDE_R(−x)，也就是持刀的右侧；左侧（刀被身体挡住的那一面）
   在它那里根本看不到。接触表两侧都出，并且把轴面写在标签上。
2. **上一轮的同一取景**。取景在两轮之间必须相同，否则并排看到的全是取景噪声。
3. **人写的特征清单点名结果**（`modelScript/manifests/<anim>.anim.toml`）——
   「这条动作应该看得出什么」只能人来写，工具只负责核对；让模型自己出题自己判卷
   是这套纪律明令禁止的。
4. **门禁的差分自证结果** —— 报不出自己该抓的缺陷的门算失效。

用法:
    python3 modelScript/tools/knife_contact_sheet.py dagger_stab \\
        --prev <上一轮 JSON 目录> --out modelScript/out/
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

_HERE = Path(__file__).resolve().parent
_REPO = _HERE.parents[1]
for _d in (_HERE, _REPO / "client" / "tools"):
    if str(_d) not in sys.path:
        sys.path.insert(0, str(_d))

import knife_anim_gates as KG  # noqa: E402
import preview_player_anim as P  # noqa: E402
import render_animation as RA  # noqa: E402
from bbmodel_maker.render import framing  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402

VIEW_NAMES = ("FRONT", "SIDE_R", "SIDE_L", "3/4")
MANIFEST_DIR = _REPO / "modelScript" / "manifests"


def key_ticks(emote) -> list[float]:
    return sorted({float(m["tick"]) for m in emote["moves"]})


def _load(path: Path):
    doc = json.loads(Path(path).read_text(encoding="utf-8"))
    emote = doc.get("emote", doc)
    return emote, P.collect_keyframes(emote)


def _union_focus(scene, ids, held_ids, disp, kfs_list, ends):
    """两轮共用一个取景 —— 各自自动取景的话，并排看到的差异大半是取景造成的。"""
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for kfs, end in zip(kfs_list, ends):
        c, r = P._fit_focus(kfs, disp, scene, ids, held_ids, end)
        lo = np.minimum(lo, np.array(c) - r)
        hi = np.maximum(hi, np.array(c) + r)
    centre = (lo + hi) / 2.0
    return tuple(centre), float(np.max(hi - lo) / 2.0)


def _tiles(kfs, disp, scene, ids, held_ids, tick, views, focus, size):
    seg = P.segment_transforms(kfs, tick)
    xform = {ids[n]: m for n, m in seg.items()}
    if held_ids:
        hm = P.hand_transform(kfs, tick, disp)
        for hid in held_ids:
            xform[hid] = hm
    out = []
    for v in views:
        img, _ = render(scene, yaw=v.yaw, pitch=v.pitch, size=size,
                        xform=xform, focus=focus, shading="mc")
        out.append(img)
    return out


def manifest_check(name: str, take: KG.KnifeTake) -> tuple[list[str], int]:
    """人写的特征清单点名。清单缺席本身就是一条红 —— 没人写过「这动作该看出什么」，
    就没有任何东西能判断它像不像那个动作。"""
    path = MANIFEST_DIR / f"{name}.anim.toml"
    if not path.exists():
        return [f"! MANIFEST MISSING: {path.relative_to(_REPO)}"], 1
    import tomllib
    spec = tomllib.loads(path.read_text(encoding="utf-8"))
    lines, bad = [], 0
    for feat in spec.get("feature", []):
        key, kind = feat["key"], feat["check"]
        want = feat.get("value")
        tick = float(feat.get("tick", 0.0))
        if kind == "grip_at":
            got = take.item_at(tick)[0]
            d = float(np.linalg.norm(got - np.array(want, float)))
            ok = d <= float(feat.get("tol", 2.0))
            detail = f"grip@t{tick:g}={np.round(got,1).tolist()} d={d:.1f}"
        elif kind == "tip_below":
            got = max(take.item_at(t)[1][1] for t in take.ticks)
            ok = got <= float(want)
            detail = f"max tip y={got:.1f} <= {want}"
        elif kind == "blade_elev_at":
            g, tip, _b = take.item_at(tick)
            v = tip - g
            got = math.degrees(math.asin(v[1] / np.linalg.norm(v)))
            ok = abs(got - float(want)) <= float(feat.get("tol", 15.0))
            detail = f"elev@t{tick:g}={got:+.0f} want {want:+g}"
        elif kind == "grip_angle_at":
            got, off = take.grip_angle_at(tick)
            ok = (abs(abs(got) - abs(float(want))) <= float(feat.get("tol", 12.0))
                  and off <= KG.GRIP_OFF_AXIS_MAX)
            detail = f"grip@t{tick:g}={got:+.0f}deg want {float(want):+g} off-axis {off:.1f}"
        elif kind == "grip_travel_min":
            pts = np.array([take.item_at(t)[0] for t in take.ticks])
            got = float(np.linalg.norm(pts.max(0) - pts.min(0)))
            ok = got >= float(want)
            detail = f"grip span={got:.1f} >= {want}"
        elif kind == "grip_travel_max":
            pts = np.array([take.item_at(t)[0] for t in take.ticks])
            got = float(np.linalg.norm(pts.max(0) - pts.min(0)))
            ok = got <= float(want)
            detail = f"grip span={got:.1f} <= {want}"
        elif kind == "tip_travel_min":
            pts = np.array([take.item_at(t)[1] for t in take.ticks])
            got = float(np.linalg.norm(pts.max(0) - pts.min(0)))
            ok = got >= float(want)
            detail = f"tip span={got:.1f} >= {want}"
        else:
            ok, detail = False, f"unknown check {kind!r}"
        bad += 0 if ok else 1
        lines.append(f"{'ok' if ok else '!!'} {key}: {detail}")
    head = f"MANIFEST | {len(lines) - bad}/{len(lines)} features ok"
    return [head] + [ln for ln in lines if ln.startswith('!!')], bad


def sheet(name: str, model: Path, prev: Path | None, out: Path, size: int = 190):
    take = KG.KnifeTake(name, model)
    views = [v for v in framing.views_for("-z", VIEW_NAMES)]
    disp = take.disp
    scene, ids, held_ids = P.build_scene(_REPO / "modelScript" / "out" / "_knife_sheet.bbmodel",
                                         model)
    cur_emote, cur_kfs = _load(KG.ANIM_DIR / f"{name}.json")
    rows = [("now", cur_kfs, key_ticks(cur_emote))]
    kfs_list, ends = [cur_kfs], [float(cur_emote.get("endTick", 8))]
    if prev and (prev / f"{name}.json").exists():
        pe, pk = _load(prev / f"{name}.json")
        rows.append(("prev", pk, key_ticks(pe)))
        kfs_list.append(pk)
        ends.append(float(pe.get("endTick", 8)))
    focus = _union_focus(scene, ids, held_ids, disp, kfs_list, ends)

    # 终端出中文全文，图上只留 ASCII（PIL 默认位图字体画不了中文）
    gates = KG.build(name, model)
    print(f"\n===== {name} =====")
    bad = gates.report()
    print()
    broken = gates.self_test()
    notes, mbad = manifest_check(name, take)
    print("\n".join(notes))

    ticks = sorted(set(rows[0][2]) | (set(rows[1][2]) if len(rows) > 1 else set()))
    lab, gap = 14, 6
    cols = len(views) * len(rows)
    w = cols * size + gap * (cols + 1) + 44
    hdr = 16 + 12 * (len(notes) + 3)
    h = hdr + len(ticks) * (size + lab + gap) + gap
    canvas = Image.new("RGB", (w, h), (16, 17, 20))
    d = ImageDraw.Draw(canvas)
    d.text((6, 6), f"{name}  |  gates {len(gates.run_all()) - bad}/{len(gates.run_all())} ok"
                   f"  |  broken gates {broken}  |  manifest fails {mbad}",
           fill=(240, 240, 232))
    y = 22
    for ln in notes:
        d.text((6, y), ln, fill=(255, 140, 140) if ln.startswith("!") else (190, 190, 184))
        y += 12
    d.text((6, y), "columns: " + " | ".join(
        f"{tag} {v.label}" for tag, _k, _t in rows for v in views), fill=(150, 150, 146))
    y = hdr
    for t in ticks:
        d.text((4, y + size // 2), f"t{t:g}", fill=(232, 232, 224))
        x = 44
        for tag, kfs, _tk in rows:
            for v, img in zip(views, _tiles(kfs, disp, scene, ids, held_ids, t,
                                            views, focus, size)):
                d.text((x + 2, y), f"{tag} {v.label}", fill=(198, 198, 190))
                canvas.paste(img, (x, y + lab))
                x += size + gap
        y += size + lab + gap
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    print(f"接触表 → {out}")
    return bad + broken + mbad


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("anims", nargs="*", default=sorted(KG.SUITE))
    ap.add_argument("--model", type=Path, default=KG.DAGGER_MODEL)
    ap.add_argument("--prev", type=Path, default=None, help="上一轮 JSON 所在目录")
    ap.add_argument("--out-dir", type=Path,
                    default=_REPO / "modelScript" / "out" / "knife_contact")
    ap.add_argument("--size", type=int, default=190)
    args = ap.parse_args(argv)
    bad = 0
    for name in (args.anims or sorted(KG.SUITE)):
        bad += sheet(name, args.model, args.prev,
                     args.out_dir / f"{name}_contact.png", args.size)
    print("\n—— Round 2 是人工闸门：看完这几张表再决定 round 3 改什么，不要自评通过 ——")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
