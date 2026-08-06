#!/usr/bin/env python3
"""马骨架 —— 一键预览：重生成三档模型 + 渲染全部视图到本目录。

产出（均落在 scripts/models/horse/）：
  render_skeleton_small.png / _medium.png / _large.png   逐档三视图
  render_compare.png                                     三档并排**同尺**侧视（比例差别只有同尺才看得出来）
  render_head.png                                        头部特写（正 / 侧 / 3-4）
  render_limbs.png                                       前后肢特写（蹄行下肢是这活儿最容易做砸的地方）
  parts_atlas.png                                        逐部件图集（常马）

用法:
  python3 scripts/models/horse/preview.py              # 全部
  python3 scripts/models/horse/preview.py --skip-gen    # 不重生成，只渲染现有 bbmodel
  python3 scripts/models/horse/preview.py --only compare
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
MODELS = REPO / "local_models" / "horse"

sys.path.insert(0, str(HERE.parent))  # 复用 scripts/models/render_bbmodel.py
sys.path.insert(0, str(HERE))
from render_bbmodel import load_bbmodel, render, render_three_view  # noqa: E402

from gen_skeleton import PROFILES  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402

BG = (14, 15, 17)
ORDER = ("small", "medium", "large")
PARTS = ("spine", "ribcage", "skull", "jaw", "pelvis", "foreleg", "hindleg", "tail")


def _run(script: str, *args: str) -> None:
    subprocess.run([sys.executable, str(HERE / script), *args], check=True, capture_output=True)


def _grid(tiles: list[tuple[str, Image.Image]], cols: int, hdr: int = 20, gap: int = 10) -> Image.Image:
    tw, th = tiles[0][1].size
    rows = (len(tiles) + cols - 1) // cols
    canvas = Image.new("RGB", (cols * tw + gap * (cols + 1), rows * (th + hdr) + gap * (rows + 1)), BG)
    draw = ImageDraw.Draw(canvas)
    for i, (label, im) in enumerate(tiles):
        r, c = divmod(i, cols)
        x = gap + c * (tw + gap)
        y = gap + r * (th + hdr + gap)
        draw.text((x + 3, y + 4), label, fill=(224, 214, 190))
        canvas.paste(im, (x, y + hdr))
    return canvas


def three_views() -> None:
    for key in ORDER:
        im, _ = render_three_view(MODELS / f"HorseSkeleton_{key}.bbmodel", size=470)
        im.save(HERE / f"render_skeleton_{key}.png")
        print(f"  → render_skeleton_{key}.png")


def compare(size: int = 520) -> None:
    """三档并排，**共用一套取景**——各自自动取景会把三匹马缩放到一样大，
    体型差就全被抹平了，正是这张图要看的东西。"""
    spans = []
    for key in ORDER:
        tris, _, _, _ = load_bbmodel(MODELS / f"HorseSkeleton_{key}.bbmodel")
        vs = [v for tri in tris for v in tri[0]]
        lo = [min(v[i] for v in vs) for i in range(3)]
        hi = [max(v[i] for v in vs) for i in range(3)]
        spans.append(([(a + b) / 2 for a, b in zip(lo, hi)], max(h - lo_ for h, lo_ in zip(hi, lo))))
    span = max(s for _, s in spans) * 1.04  # 最大者定尺，三张同比例

    tiles = []
    for (center, _), key in zip(spans, ORDER):
        P = PROFILES[key]
        # 取景中心统一压到地面上方半个 span，四蹄才不会被裁
        focus = ((center[0], span * 0.42, center[2]), span)
        im, _ = render(MODELS / f"HorseSkeleton_{key}.bbmodel", yaw=90.0, pitch=0.0, size=size, focus=focus)
        tiles.append((f"{P.label} ({key})  鬐甲 {P.wither / 16:.2f} m", im))
    _grid(tiles, cols=3).save(HERE / "render_compare.png")
    print("  → render_compare.png")


def head(size: int = 460) -> None:
    tiles = []
    for key in ORDER:
        P = PROFILES[key]
        tris, _, _, _ = load_bbmodel(MODELS / f"HorseSkeleton_{key}.bbmodel")
        # 取景锁在头上：吻端最靠前，往后取 1.3 个头长
        vs = [v for tri in tris for v in tri[0]]
        z_nose = min(v[2] for v in vs)
        c = (0.0, P.y_occiput - P.H * 0.30, z_nose + P.H * 0.55)
        for tag, yaw, pitch in (("side", 90.0, 0.0), ("3/4", 140.0, 12.0)):
            im, _ = render(
                MODELS / f"HorseSkeleton_{key}.bbmodel",
                yaw=yaw,
                pitch=pitch,
                size=size,
                focus=(c, P.H * 1.45),
            )
            tiles.append((f"{P.label} {tag}", im))
    _grid(tiles, cols=2).save(HERE / "render_head.png")
    print("  → render_head.png")


def limbs(size: int = 460) -> None:
    """蹄行下肢特写：腕/跗高悬、管骨独存、系冠 52° 前倾插进蹄匣——做砸就在这儿。"""
    tiles = []
    P = PROFILES["medium"]
    for tag, c, span, yaw in (
        ("前肢 侧", (0.0, P.y_carpus * 0.62, P.z_carpus), P.y_elbow * 1.45, 90.0),
        ("前肢 前", (0.0, P.y_carpus * 0.62, P.z_carpus), P.y_elbow * 1.45, 180.0),
        ("后肢 侧", (0.0, P.y_hock * 0.72, P.z_hock - P.u(0.02)), P.y_stifle * 1.5, 90.0),
        ("后肢 后", (0.0, P.y_hock * 0.72, P.z_hock - P.u(0.02)), P.y_stifle * 1.5, 0.0),
    ):
        im, _ = render(
            MODELS / "HorseSkeleton_medium.bbmodel", yaw=yaw, pitch=6.0, size=size, focus=(c, span)
        )
        tiles.append((f"常马 {tag}", im))
    _grid(tiles, cols=2).save(HERE / "render_limbs.png")
    print("  → render_limbs.png")


def parts_atlas(size: int = 400) -> None:
    tiles = []
    for part in PARTS:
        _run("gen_skeleton.py", "--profile", "medium", "--part", part)
        im, _ = render(MODELS / f"Horse_{part}_medium.bbmodel", yaw=138.0, pitch=14.0, size=size)
        tiles.append((part, im))
    _grid(tiles, cols=4).save(HERE / "parts_atlas.png")
    print("  → parts_atlas.png")


def main() -> int:
    ap = argparse.ArgumentParser(description="马骨架预览渲染")
    ap.add_argument("--skip-gen", action="store_true", help="不重生成 bbmodel，只渲染现有的")
    ap.add_argument(
        "--only",
        choices=("three", "compare", "head", "limbs", "atlas"),
        help="只出其中一张",
    )
    args = ap.parse_args()

    if not args.skip_gen:
        print("生成模型…")
        _run("gen_skeleton.py")

    print("渲染…")
    jobs = {"three": three_views, "compare": compare, "head": head, "limbs": limbs, "atlas": parts_atlas}
    for key, fn in jobs.items():
        if args.only in (None, key):
            fn()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
