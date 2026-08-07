#!/usr/bin/env python3
"""马具预览：分档对照 + 蹄部特写。

马具件小，套在整只马上按整体取景渲出来只有几个像素——什么都看不出来。所以取景一律
**锁在装备本身的包围盒**上（蹄铁就锁蹄），并且三档并排同尺，好一眼比出分档差别。

产物（本目录 tack/）：
  shoe_rows.png     三档 × 四视角，常马，蹄部特写（带马）
  shoe_alone.png    三档单出（无马），看 U 形轮廓与钉/夹/灵纹
  shoe_profiles.png 三档 × 三体型，侧视特写

用法:
  python3 scripts/models/horse/preview_tack.py
  python3 scripts/models/horse/preview_tack.py --skip-gen
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
FINAL = REPO / "local_models" / "horse"
STAGES = FINAL / "stages"
TACK = FINAL / "tack"
OUT = HERE / "tack"

sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))
from gen_skeleton import PROFILES  # noqa: E402
from gen_tack import SHOES  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402
from render_bbmodel import render  # noqa: E402

BG = (14, 15, 17)
# 四个角度各查一件事：侧视看铁条厚与钉排、蹄尖视看趾夹、斜下视看 U 形、正下视看开口。
# yaw 的朝向实测过（render 的 yaw=178 是马脸朝镜头，0 是马尾），别照猜。
VIEWS = (("侧 side", 90.0, 6.0), ("蹄尖 toe", 178.0, 8.0), ("斜下 3/4-low", 150.0, -25.0), ("底 below", 92.0, -74.0))


def tack_focus(path: Path, pad: float = 1.45) -> tuple[np.ndarray, float]:
    """取景锁在**马具件**的包围盒上（带马的那份里皮件占绝大多数，按整体取景就没了）。"""
    d = json.loads(path.read_text())
    els = [e for e in d["elements"] if e.get("_tack")]
    if not els:
        els = d["elements"]
    lo = np.array([min(e["from"][i] for e in els) for i in range(3)], float)
    hi = np.array([max(e["to"][i] for e in els) for i in range(3)], float)
    return (lo + hi) / 2, float((hi - lo).max()) * pad


def foot_focus(path: Path, foot: str = "f_l", pad: float = 2.6) -> tuple[np.ndarray, float]:
    """单只蹄的取景（四只蹄分散在四角，按整只马具取景一样看不清）。"""
    d = json.loads(path.read_text())
    els = [e for e in d["elements"] if e.get("_tack") and e["name"].startswith(f"shoe_{foot}_")]
    lo = np.array([min(e["from"][i] for e in els) for i in range(3)], float)
    hi = np.array([max(e["to"][i] for e in els) for i in range(3)], float)
    c = (lo + hi) / 2
    return c, float((hi - lo).max()) * pad


def grid(rows: list[tuple[str, list[tuple[str, Image.Image]]]], cell: int, title: str) -> Image.Image:
    gap, lab, hdr = 10, 16, 26
    cols = max(len(r[1]) for r in rows)
    W = gap + cols * (cell + gap)
    H = hdr + gap + len(rows) * (cell + lab + gap)
    im = Image.new("RGB", (W, H), BG)
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-gen", action="store_true")
    ap.add_argument("--profile", default="medium", choices=sorted(PROFILES))
    args = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)

    if not args.skip_gen:
        subprocess.run([sys.executable, str(HERE / "gen_tack.py"), "--skip-anim"], check=True, capture_output=True)
        subprocess.run([sys.executable, str(HERE / "gen_tack.py"), "--with-horse", "--skip-anim"],
                       check=True, capture_output=True)

    pk = args.profile
    cell = 300

    # 1) 三档 × 四视角，带马、蹄部特写
    rows = []
    for tk, spec in SHOES.items():
        p = STAGES / f"HorseShoe_{tk}_{pk}_on_horse.bbmodel"
        f = foot_focus(p)
        tiles = [(v, render(p, yaw=y, pitch=pi, size=cell, bg=BG, focus=f)[0]) for v, y, pi in VIEWS]
        rows.append((f"{spec.label}", tiles))
    grid(rows, cell, f"蹄铁三档 · {PROFILES[pk].label} · 左前蹄特写（带马）").save(OUT / "shoe_rows.png")

    # 2) 三档单出：只有铁，看清件与件的关系
    tiles_all = []
    for tk, spec in SHOES.items():
        p = TACK / f"HorseShoe_{tk}_{pk}.bbmodel"
        f = foot_focus(p, pad=1.30)
        tiles_all.append((spec.label, [(v, render(p, yaw=y, pitch=pi, size=cell, bg=BG, focus=f)[0])
                                       for v, y, pi in VIEWS]))
    grid(tiles_all, cell, f"蹄铁三档 · {PROFILES[pk].label} · 单出（无马）").save(OUT / "shoe_alone.png")

    # 3) 三档 × 三体型：侧视，看尺寸是否跟着蹄走
    rows = []
    for tk, spec in SHOES.items():
        tiles = []
        for p2 in ("small", "medium", "large"):
            p = STAGES / f"HorseShoe_{tk}_{p2}_on_horse.bbmodel"
            tiles.append((PROFILES[p2].label, render(p, yaw=90.0, pitch=6.0, size=cell, bg=BG,
                                                     focus=foot_focus(p))[0]))
        rows.append((spec.label, tiles))
    grid(rows, cell, "蹄铁三档 × 三体型 · 左前蹄侧视").save(OUT / "shoe_profiles.png")

    # 4) 整只马：装备做出来最容易犯的错是"只在特写里成立"。玩家实际是站着看整只马的，
    #    所以必须有一张按整只取景的图，确认远处还看得出蹄上有铁、且三档分得开。
    #    第一行故意放**赤脚**：判"穿没穿看得出来"要有对照组，不然只能凭印象说"好像有"。
    rows = []
    for lab_row, path_of in (("赤脚（对照）", lambda tk: FINAL / f"HorsePelt_rust_{pk}.bbmodel"),):
        tiles = [(lab, render(path_of(None), yaw=yaw, pitch=pit, size=cell, bg=BG)[0])
                 for lab, yaw, pit in (("侧 side", 90.0, 8.0), ("斜 3/4", 140.0, 12.0))]
        rows.append((lab_row, tiles))
    for tk, spec in SHOES.items():
        tiles = []
        for lab, yaw, pit in (("侧 side", 90.0, 8.0), ("斜 3/4", 140.0, 12.0)):
            p = STAGES / f"HorseShoe_{tk}_{pk}_on_horse.bbmodel"
            tiles.append((lab, render(p, yaw=yaw, pitch=pit, size=cell, bg=BG)[0]))
        rows.append((spec.label, tiles))
    grid(rows, cell, f"蹄铁三档 · {PROFILES[pk].label} · 整只（正常观看距离）").save(OUT / "shoe_on_horse.png")

    # 5) 动画连拍：静止姿贴合不代表跑起来还贴合。铁刚性挂在蹄骨上理论上不会掉，但
    #    "理论上"正是这套流水线一路踩坑的地方——袭步腾空与倒毙侧翻各来一条，眼见为实。
    import render_anim as RA
    from rig import Rig

    for name in ("gallop", "death"):
        p = STAGES / f"HorseShoe_lingtie_{pk}_on_horse.bbmodel"
        rig = Rig(p)
        RA.contact_sheet(rig, PROFILES[pk], p, name, "side", 8, 300, RA.focus_box(rig)).save(
            OUT / f"shoe_anim_{name}.png")
        print(f"→ {(OUT / f'shoe_anim_{name}.png').relative_to(REPO)}")

    for f in ("shoe_rows.png", "shoe_alone.png", "shoe_profiles.png", "shoe_on_horse.png"):
        print(f"→ {(OUT / f).relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
