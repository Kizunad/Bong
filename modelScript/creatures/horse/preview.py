#!/usr/bin/env python3
"""马 —— 一键预览：重生成三层模型 + 渲染全部视图。

目录分工与产物一致：**最终交付的皮层图留在本目录**，骨架/肌肉这类过程图落 stages/。

本目录（交付物预览，对应 modelScript/models/horse/ 的 9 份皮层）：
  pelt_matrix.png            3 毛色 × 3 体型 同尺侧视矩阵 —— 一张看全 9 份
  pelt_three_view_<coat>.png 每种毛色（常马）三视图
  pelt_head.png              三种毛色头部特写

stages/（过程图，对应 modelScript/models/horse/stages/）：
  render_skeleton_*.png · render_compare.png · render_head.png · render_limbs.png
  parts_atlas.png · render_muscle_*.png · render_muscle_bare.png · render_explode.png
  muscle_atlas.png

用法:
  python3 modelScript/creatures/horse/preview.py               # 全部
  python3 modelScript/creatures/horse/preview.py --skip-gen     # 不重生成，只渲染
  python3 modelScript/creatures/horse/preview.py --only matrix
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
# 交付物（最终 9 份皮层）与中间产物（骨架 / 肌肉 / 各类预览）分开放，别混。
FINAL = Path(__file__).resolve().parents[2] / "models" / "horse"
MODELS = FINAL / "stages"
OUT = HERE
STAGE_OUT = HERE / "stages"

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))
from bbmodel_maker.render.render_bbmodel import load_bbmodel, render, render_three_view  # noqa: E402

from gen_pelt import COATS  # noqa: E402
from gen_skeleton import PROFILES  # noqa: E402
from PIL import Image, ImageDraw  # noqa: E402

BG = (14, 15, 17)
ORDER = ("small", "medium", "large")
COAT_ORDER = ("rust", "dun", "roan")
PARTS = ("spine", "ribcage", "skull", "jaw", "pelvis", "foreleg", "hindleg", "tail")
MGROUPS = (
    ("head", "Head: masseter / temporalis"),
    ("neck", "Neck: nuchal ligament / splenius / brachiocephalicus"),
    ("torso", "Torso: longissimus / latissimus / serratus sling / abdominals"),
    ("foreleg", "Foreleg: triceps / spinati / forearm + digital tendons"),
    ("hindleg", "Hindleg: gluteus / TFL / biceps femoris / gastrocnemius"),
    ("tail", "Tail: sacrocaudalis"),
)


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


def _shared_focus(paths: list[Path]) -> tuple[float, list[tuple[float, float, float]]]:
    """一批模型共用一套取景：各自自动取景会把大小不同的马缩到一样大，
    体型差就全被抹平了 —— 而那正是这些图要看的东西。"""
    spans = []
    for fp in paths:
        tris, _, _, _ = load_bbmodel(fp)
        vs = [v for tri in tris for v in tri[0]]
        lo = [min(v[i] for v in vs) for i in range(3)]
        hi = [max(v[i] for v in vs) for i in range(3)]
        spans.append(([(a + b) / 2 for a, b in zip(lo, hi)], max(h - lo_ for h, lo_ in zip(hi, lo))))
    span = max(s for _, s in spans) * 1.04
    centers = [(c[0], span * 0.42, c[2]) for c, _ in spans]
    return span, centers


# ---------------------------------------------------------------- 交付物预览
def pelt_matrix(size: int = 430) -> None:
    """3 毛色 × 3 体型，全部同尺同角度 —— 交付物的总览图。"""
    paths = [FINAL / f"HorsePelt_{c}_{k}.bbmodel" for c in COAT_ORDER for k in ORDER]
    span, centers = _shared_focus(paths)
    tiles = []
    for fp, center in zip(paths, centers):
        ck, pk = fp.stem.split("_")[1], fp.stem.split("_")[2]
        im, _ = render(fp, yaw=90.0, pitch=0.0, size=size, focus=(center, span))
        tiles.append((f"{COATS[ck].label} · {PROFILES[pk].label}  {PROFILES[pk].wither / 16:.2f} m", im))
    _grid(tiles, cols=3).save(OUT / "pelt_matrix.png")
    print("  → pelt_matrix.png")


def pelt_three_view(size: int = 460) -> None:
    for ck in COAT_ORDER:
        im, _ = render_three_view(FINAL / f"HorsePelt_{ck}_medium.bbmodel", size=size)
        im.save(OUT / f"pelt_three_view_{ck}.png")
        print(f"  → pelt_three_view_{ck}.png")


def pelt_head(size: int = 460) -> None:
    """头部特写：毛色差别（面罩 / 白章 / 深头）主要落在脸上。"""
    tiles = []
    P = PROFILES["medium"]
    c = (0.0, P.y_occiput - P.H * 0.26, P.z_occiput - P.H * 0.42)
    for ck in COAT_ORDER:
        fp = FINAL / f"HorsePelt_{ck}_medium.bbmodel"
        for tag, yaw, pitch in (("side", 90.0, 0.0), ("3/4", 142.0, 10.0)):
            im, _ = render(fp, yaw=yaw, pitch=pitch, size=size, focus=(c, P.H * 1.75))
            tiles.append((f"{COATS[ck].label} {tag}", im))
    _grid(tiles, cols=2).save(OUT / "pelt_head.png")
    print("  → pelt_head.png")


# ---------------------------------------------------------------- 过程图
def three_views() -> None:
    for key in ORDER:
        im, _ = render_three_view(MODELS / f"HorseSkeleton_{key}.bbmodel", size=470)
        im.save(STAGE_OUT / f"render_skeleton_{key}.png")
        print(f"  → stages/render_skeleton_{key}.png")


def compare(size: int = 520) -> None:
    paths = [MODELS / f"HorseSkeleton_{k}.bbmodel" for k in ORDER]
    span, centers = _shared_focus(paths)
    tiles = []
    for fp, center, key in zip(paths, centers, ORDER):
        P = PROFILES[key]
        im, _ = render(fp, yaw=90.0, pitch=0.0, size=size, focus=(center, span))
        tiles.append((f"{P.label} ({key})  鬐甲 {P.wither / 16:.2f} m", im))
    _grid(tiles, cols=3).save(STAGE_OUT / "render_compare.png")
    print("  → stages/render_compare.png")


def head(size: int = 460) -> None:
    tiles = []
    for key in ORDER:
        P = PROFILES[key]
        tris, _, _, _ = load_bbmodel(MODELS / f"HorseSkeleton_{key}.bbmodel")
        vs = [v for tri in tris for v in tri[0]]
        z_nose = min(v[2] for v in vs)
        c = (0.0, P.y_occiput - P.H * 0.30, z_nose + P.H * 0.55)
        for tag, yaw, pitch in (("side", 90.0, 0.0), ("3/4", 140.0, 12.0)):
            im, _ = render(MODELS / f"HorseSkeleton_{key}.bbmodel", yaw=yaw, pitch=pitch, size=size, focus=(c, P.H * 1.45))
            tiles.append((f"{P.label} {tag}", im))
    _grid(tiles, cols=2).save(STAGE_OUT / "render_head.png")
    print("  → stages/render_head.png")


def limbs(size: int = 460) -> None:
    """蹄行下肢特写：腕/跗高悬、管骨独存、系冠 52° 前倾插进蹄匣。"""
    tiles = []
    P = PROFILES["medium"]
    for tag, c, span, yaw in (
        ("前肢 侧", (0.0, P.y_carpus * 0.62, P.z_carpus), P.y_elbow * 1.45, 90.0),
        ("前肢 前", (0.0, P.y_carpus * 0.62, P.z_carpus), P.y_elbow * 1.45, 180.0),
        ("后肢 侧", (0.0, P.y_hock * 0.72, P.z_hock - P.u(0.02)), P.y_stifle * 1.5, 90.0),
        ("后肢 后", (0.0, P.y_hock * 0.72, P.z_hock - P.u(0.02)), P.y_stifle * 1.5, 0.0),
    ):
        im, _ = render(MODELS / "HorseSkeleton_medium.bbmodel", yaw=yaw, pitch=6.0, size=size, focus=(c, span))
        tiles.append((f"常马 {tag}", im))
    _grid(tiles, cols=2).save(STAGE_OUT / "render_limbs.png")
    print("  → stages/render_limbs.png")


def parts_atlas(size: int = 400) -> None:
    tiles = []
    for part in PARTS:
        _run("gen_skeleton.py", "--profile", "medium", "--part", part)
        im, _ = render(MODELS / f"Horse_{part}_medium.bbmodel", yaw=138.0, pitch=14.0, size=size)
        tiles.append((part, im))
    _grid(tiles, cols=4).save(STAGE_OUT / "parts_atlas.png")
    print("  → stages/parts_atlas.png")


def muscle_views() -> None:
    for key in ORDER:
        im, _ = render_three_view(MODELS / f"HorseMuscle_{key}.bbmodel", size=470)
        im.save(STAGE_OUT / f"render_muscle_{key}.png")
        print(f"  → stages/render_muscle_{key}.png")


def muscle_bare(size: int = 520) -> None:
    paths = [MODELS / f"HorseMuscle_{k}_bare.bbmodel" for k in ORDER]
    span, centers = _shared_focus(paths)
    tiles = []
    for fp, center, key in zip(paths, centers, ORDER):
        P = PROFILES[key]
        im, _ = render(fp, yaw=90.0, pitch=0.0, size=size, focus=(center, span))
        tiles.append((f"{P.label} ({key})  鬐甲 {P.wither / 16:.2f} m", im))
    _grid(tiles, cols=3).save(STAGE_OUT / "render_muscle_bare.png")
    print("  → stages/render_muscle_bare.png")


def muscle_explode(size: int = 520) -> None:
    im, _ = render_three_view(MODELS / "HorseMuscle_medium_explode.bbmodel", size=size)
    im.save(STAGE_OUT / "render_explode.png")
    print("  → stages/render_explode.png")


def muscle_atlas(size: int = 430) -> None:
    tiles = []
    for key, label in MGROUPS:
        _run("gen_muscle.py", "--profile", "medium", "--group", key)
        im, _ = render(MODELS / f"HorseMuscle_{key}_medium.bbmodel", yaw=142.0, pitch=14.0, size=size)
        tiles.append((label, im))
    _grid(tiles, cols=3).save(STAGE_OUT / "muscle_atlas.png")
    print("  → stages/muscle_atlas.png")


def main() -> int:
    ap = argparse.ArgumentParser(description="马 三层预览渲染")
    ap.add_argument("--skip-gen", action="store_true", help="不重生成 bbmodel，只渲染现有的")
    ap.add_argument(
        "--only",
        choices=("matrix", "pelt3", "pelthead", "three", "compare", "head", "limbs", "atlas", "muscle", "bare", "explode", "matlas"),
        help="只出其中一张",
    )
    args = ap.parse_args()

    STAGE_OUT.mkdir(parents=True, exist_ok=True)
    if not args.skip_gen:
        print("生成模型…")
        _run("gen_skeleton.py")
        _run("gen_muscle.py")
        _run("gen_muscle.py", "--only-muscle")
        _run("gen_muscle.py", "--profile", "medium", "--explode", "5")
        _run("gen_pelt.py")

    print("渲染…")
    jobs = {
        "matrix": pelt_matrix,
        "pelt3": pelt_three_view,
        "pelthead": pelt_head,
        "three": three_views,
        "compare": compare,
        "head": head,
        "limbs": limbs,
        "atlas": parts_atlas,
        "muscle": muscle_views,
        "bare": muscle_bare,
        "explode": muscle_explode,
        "matlas": muscle_atlas,
    }
    for key, fn in jobs.items():
        if args.only in (None, key):
            fn()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
