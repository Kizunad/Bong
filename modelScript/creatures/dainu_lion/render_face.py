#!/usr/bin/env python3
"""脸部特写：把头/鬃区单独切出来渲染。

整只渲染时脸只占百来像素，眼睛这种 0.3 单位的细节根本看不出来（竖瞳到底有没有
渲出来、眉脊有没有压住眼睛，在全身图上判断不了）。这里按 z 切一刀只留头区，
让 render() 的自动取景把脸放满画面。
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
MODELS = Path(__file__).resolve().parents[2] / "models" / "dainu_lion"
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

from gen_pelt import _corners  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮脸部特写")
    ap.add_argument("--zcut", type=float, default=-13.0, help="只保留 z 中心在此之前的件")
    ap.add_argument("--no-mane", action="store_true", help="剥掉鬃毛看裸脸")
    ap.add_argument("--size", type=int, default=620)
    args = ap.parse_args()

    src = MODELS / "DainuLionPelt.bbmodel"
    data = json.loads(src.read_text())
    keep = []
    for e in data["elements"]:
        pts = _corners(e)
        zc = sum(p[2] for p in pts) / len(pts)
        if zc > args.zcut:
            continue
        if args.no_mane and e["name"].startswith("mane_"):
            continue
        keep.append(e)
    kept = {e["uuid"] for e in keep}
    data["elements"] = keep

    def prune(node):
        if isinstance(node, str):
            return node in kept
        node["children"] = [c for c in node.get("children", []) if prune(c)]
        return True

    for root in data["outliner"]:
        prune(root)

    tmp = MODELS / "_face_tmp.bbmodel"
    tmp.write_text(json.dumps(data, ensure_ascii=False))
    tag = "_bare" if args.no_mane else ""
    for name, yaw, pitch in (("front", 180.0, 0.0), ("34", 143.0, 10.0), ("side", 90.0, 0.0)):
        im, _ = render(tmp, yaw=yaw, pitch=pitch, size=args.size)
        im.save(HERE / f"face_{name}{tag}.png")
        print(f"  → face_{name}{tag}.png")
    tmp.unlink()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
