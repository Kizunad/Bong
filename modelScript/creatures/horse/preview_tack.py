#!/usr/bin/env python3
"""马具预览：分档对照 + 局部特写 + 整只 + 动画连拍。

四张图各回答一个问题，缺一张就有一类问题看不出来：
  · `<kind>_alone`    —— 件与件的关系对不对（单出，无马）
  · `<kind>_rows`     —— 装在马上贴不贴合（局部特写，带马）
  · `<kind>_on_horse` —— **正常观看距离**下看不看得出、三档分不分得开
    （第一行固定放**没装备**的对照——判"装没装看得出来"总得有对照组）
  · `<kind>_anim`     —— 跑起来还在不在原处

取景一律锁在**装备自身**的包围盒上：马具件小，按整只马取景渲出来只有几个像素。

用法:
  python3 modelScript/creatures/horse/preview_tack.py
  python3 modelScript/creatures/horse/preview_tack.py --kind saddle --skip-gen
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
FINAL = Path(__file__).resolve().parents[2] / "models" / "horse"
STAGES = FINAL / "stages"
TACK = FINAL / "tack"
OUT = HERE / "tack"

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))
from gen_skeleton import PROFILES  # noqa: E402
from gen_tack import KINDS  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402

BG = (14, 15, 17)
# yaw 的朝向实测过（render 的 yaw=178 是马脸朝镜头，0 是马尾），别照猜。
# 每种马具看四个角度，各查一件事。
VIEWS = {
    "shoe": (("侧 side", 90.0, 6.0), ("蹄尖 toe", 178.0, 8.0),
             ("斜下 3/4-low", 150.0, -25.0), ("底 below", 92.0, -74.0)),
    "saddle": (("侧 side", 90.0, 4.0), ("斜 3/4", 142.0, 16.0),
               ("后 rear", 4.0, 18.0), ("底（看肚带）", 96.0, -60.0)),
    "rein": (("侧 side", 90.0, 2.0), ("斜前 3/4", 150.0, 10.0),
             ("正面（看鼻革绕圈）", 178.0, 4.0), ("下（看咽革）", 120.0, -46.0)),
    "bard": (("侧 side", 90.0, 6.0), ("斜前 3/4", 142.0, 14.0),
             ("后（看搭后）", 6.0, 22.0), ("前（看当胸）", 178.0, 8.0)),
}
# 局部特写取哪一簇件（蹄铁四只分散在四角，按整体取景一样看不清）
FOCUS_PREFIX = {"shoe": "shoe_f_l_", "saddle": "saddle_", "rein": "rein_", "bard": "bard_"}


def _els(path: Path, prefix: str | None = None) -> list[dict]:
    d = json.loads(path.read_text())
    els = [e for e in d["elements"] if e.get("_tack")]
    if prefix:
        els = [e for e in els if e["name"].startswith(prefix)] or els
    return els


def focus_of(path: Path, prefix: str | None = None, pad: float = 1.45):
    els = _els(path, prefix)
    lo = np.array([min(e["from"][i] for e in els) for i in range(3)], float)
    hi = np.array([max(e["to"][i] for e in els) for i in range(3)], float)
    return (lo + hi) / 2, float((hi - lo).max()) * pad


def grid(rows: list[tuple[str, list[tuple[str, Image.Image]]]], cell: int, title: str) -> Image.Image:
    gap, lab, hdr = 10, 16, 26
    cols = max(len(r[1]) for r in rows)
    im = Image.new("RGB", (gap + cols * (cell + gap), hdr + gap + len(rows) * (cell + lab + gap)), BG)
    d = ImageDraw.Draw(im)
    d.text((gap, 6), title, fill=(226, 222, 210))
    for r, (rname, tiles) in enumerate(rows):
        y = hdr + gap + r * (cell + lab + gap)
        d.text((gap + 2, y), rname, fill=(210, 198, 168))
        for c, (cname, img) in enumerate(tiles):
            x = gap + c * (cell + gap)
            im.paste(img, (x, y + lab))
            d.text((x + 4, y + lab + 2), cname, fill=(150, 150, 146))
    return im


def one_kind(kind: str, pk: str, cell: int = 300) -> None:
    K = KINDS[kind]
    views = VIEWS[kind]
    pre = FOCUS_PREFIX[kind]
    cap = kind.capitalize()

    def on_horse(tk: str) -> Path:
        return STAGES / f"Horse{cap}_{tk}_{pk}_on_horse.bbmodel"

    rows = [(spec.label, [(v, render(on_horse(tk), yaw=y, pitch=p, size=cell, bg=BG,
                                     focus=focus_of(on_horse(tk), pre, 2.0))[0]) for v, y, p in views])
            for tk, spec in K.table.items()]
    grid(rows, cell, f"{K.label}三档 · {PROFILES[pk].label} · 局部特写（带马）").save(OUT / f"{kind}_rows.png")

    rows = [(spec.label, [(v, render(TACK / f"Horse{cap}_{tk}_{pk}.bbmodel", yaw=y, pitch=p, size=cell, bg=BG,
                                     focus=focus_of(TACK / f"Horse{cap}_{tk}_{pk}.bbmodel", pre, 1.30))[0])
                          for v, y, p in views])
            for tk, spec in K.table.items()]
    grid(rows, cell, f"{K.label}三档 · {PROFILES[pk].label} · 单出（无马）").save(OUT / f"{kind}_alone.png")

    far = (("侧 side", 90.0, 8.0), ("斜 3/4", 140.0, 12.0))
    bare = FINAL / f"HorsePelt_rust_{pk}.bbmodel"
    rows = [("未装备（对照）", [(v, render(bare, yaw=y, pitch=p, size=cell, bg=BG)[0]) for v, y, p in far])]
    rows += [(spec.label, [(v, render(on_horse(tk), yaw=y, pitch=p, size=cell, bg=BG)[0]) for v, y, p in far])
             for tk, spec in K.table.items()]
    grid(rows, cell, f"{K.label}三档 · {PROFILES[pk].label} · 整只（正常观看距离）").save(OUT / f"{kind}_on_horse.png")

    # 整套穿戴：判据只回答"甲与鞍没撞上"。撞没撞上和**穿全了好不好看**是两件事，
    # 后者只有把四种马具装在同一匹马上才看得见——甲让开鞍位留下的那条空档，究竟是
    # "给鞍留的位置"还是"一个洞"，就看这张图。
    if kind == "bard":
        from gen_tack import SUITS

        def suit(tk: str) -> Path:
            return STAGES / f"HorseSuit_{tk}_{pk}_on_horse.bbmodel"

        if suit(next(iter(SUITS))).is_file():
            rows = [("未装备（对照）", [(v, render(FINAL / f"HorsePelt_rust_{pk}.bbmodel",
                                                  yaw=y, pitch=p, size=cell, bg=BG)[0])
                                        for v, y, p in far])]
            rows += [(f"{K.table[tk].label} 全套", [(v, render(suit(tk), yaw=y, pitch=p,
                                                              size=cell, bg=BG)[0])
                                                    for v, y, p in far]) for tk in SUITS]
            grid(rows, cell, f"整套穿戴 · {PROFILES[pk].label}（甲 + 鞍 + 缰 + 蹄铁）").save(
                OUT / "bard_suit.png")
            print(f"→ {(OUT / 'bard_suit.png').relative_to(REPO)}")

    # 动画连拍：静止姿贴合不代表跑起来还贴合。取用料最全的那一档，袭步 + 倒毙各一条。
    # 这里只出空载那一档：这几张看的是**马具贴不贴身**，负载改的是步态，逐档叠上来
    # 反而把要看的东西挤小了（负载的对拍另有 anim/anim_*_side_loads.png）。
    import gen_anim as GA
    import render_anim as RA
    from rig import Rig

    top = list(K.table)[-1]
    for name in ("gallop", "death"):
        p = on_horse(top)
        rig = Rig(p)
        RA.contact_sheet(rig, PROFILES[pk], p, name, "side", 8, 300, RA.focus_box(rig),
                         [GA.BARE]).save(OUT / f"{kind}_anim_{name}.png")

    for f in (f"{kind}_alone.png", f"{kind}_rows.png", f"{kind}_on_horse.png",
              f"{kind}_anim_gallop.png", f"{kind}_anim_death.png"):
        print(f"→ {(OUT / f).relative_to(REPO)}")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-gen", action="store_true")
    ap.add_argument("--kind", choices=[*sorted(KINDS), "all"], default="all")
    ap.add_argument("--profile", default="medium", choices=sorted(PROFILES))
    args = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)

    if not args.skip_gen:
        subprocess.run([sys.executable, str(HERE / "gen_tack.py"), "--with-horse", "--skip-anim"],
                       check=True, capture_output=True)

    for kind in (sorted(KINDS) if args.kind == "all" else [args.kind]):
        one_kind(kind, args.profile)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
