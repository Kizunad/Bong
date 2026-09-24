#!/usr/bin/env python3
"""怠怒之狮 —— 一键预览：重生成模型 + 渲染全部视图到本目录。

产出（均落在 modelScript/creatures/dainu_lion/）：
  render_skeleton.png        骨架三视图
  render_muscle.png          骨 + 肌三视图
  render_muscle_bare.png     只肌肉三视图
  render_explode.png         延展视图（部件沿径向散开）
  render_pelt.png            皮毛/外观层三视图（最终外观）
  render_pelt_anatomy.png    皮层叠在骨+肌上，看包裹关系
  render_head_front.png      头部正面特写 · render_head_34.png 3/4 特写
  muscle_atlas.png           逐肌群图集（每群单独显示在骨架上）

用法:
  python3 modelScript/creatures/dainu_lion/preview.py           # 全部
  python3 modelScript/creatures/dainu_lion/preview.py --atlas   # 只出图集
  python3 modelScript/creatures/dainu_lion/preview.py --skip-gen  # 不重生成，只渲染现有 bbmodel
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
MODELS = Path(__file__).resolve().parents[2] / "models" / "dainu_lion"

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
from bbmodel_maker.render.render_bbmodel import render, render_three_view  # noqa: E402

from PIL import Image, ImageDraw  # noqa: E402

GROUPS = [
    ("head", "Head: temporalis / masseter"),
    ("neck", "Neck: nuchal / sternocephalicus"),
    ("torso", "Torso: longissimus / latissimus / obliquus / pectoralis"),
    ("foreleg", "Foreleg: deltoid / supra-infraspinatus / triceps / biceps"),
    ("hindleg", "Hindleg: gluteus / biceps femoris / quadriceps / gastrocnemius"),
    ("tail", "Tail: caudal muscles"),
]


def _run(script: str, *args: str) -> None:
    subprocess.run([sys.executable, str(HERE / script), *args], check=True, capture_output=True)


def three_view(bbmodel: str, out: str, size: int = 470) -> None:
    im, _ = render_three_view(MODELS / bbmodel, size=size)
    im.save(HERE / out)
    print(f"  → {out}")


def atlas(size: int = 430) -> None:
    """逐肌群图集：每群单独显示在完整骨架上，形状与附着点一眼可辨。"""
    tiles = []
    for key, label in GROUPS:
        _run("gen_muscle.py", "--group", key)
        im, _ = render(MODELS / f"DainuLionMuscle_{key}.bbmodel", yaw=145.0, pitch=15.0, size=size)
        tiles.append((label, im))

    cols, gap, hdr = 3, 10, 20
    tw, th = tiles[0][1].size
    rows = (len(tiles) + cols - 1) // cols
    canvas = Image.new("RGB", (cols * tw + gap * (cols + 1), rows * (th + hdr) + gap * (rows + 1)), (14, 15, 17))
    draw = ImageDraw.Draw(canvas)
    for i, (label, im) in enumerate(tiles):
        r, c = divmod(i, cols)
        x = gap + c * (tw + gap)
        y = gap + r * (th + hdr + gap)
        draw.text((x + 3, y + 4), label, fill=(224, 206, 180))
        canvas.paste(im, (x, y + hdr))
    canvas.save(HERE / "muscle_atlas.png")
    print("  → muscle_atlas.png")


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮预览渲染")
    ap.add_argument("--atlas", action="store_true", help="只出逐肌群图集")
    ap.add_argument("--skip-gen", action="store_true", help="不重生成 bbmodel，只渲染现有的")
    args = ap.parse_args()

    if not args.skip_gen:
        print("生成模型…")
        _run("gen_skeleton.py")
        _run("gen_muscle.py")
        _run("gen_muscle.py", "--only-muscle")
        _run("gen_muscle.py", "--explode", "5")
        _run("gen_pelt.py")
        _run("gen_pelt.py", "--with-anatomy")

    print("渲染…")
    if not args.atlas:
        three_view("DainuLionSkeleton.bbmodel", "render_skeleton.png")
        three_view("DainuLionMuscle.bbmodel", "render_muscle.png")
        three_view("DainuLionMuscle_bare.bbmodel", "render_muscle_bare.png")
        three_view("DainuLionMuscle_explode.bbmodel", "render_explode.png", size=500)
        three_view("DainuLionPelt.bbmodel", "render_pelt.png")
        three_view("DainuLionPelt_anatomy.bbmodel", "render_pelt_anatomy.png")
        for tag, yaw, pitch in (("front", 178.0, 6.0), ("34", 138.0, 14.0)):
            im, _ = render(MODELS / "DainuLionPelt.bbmodel", yaw=yaw, pitch=pitch, size=560)
            im.save(HERE / f"render_head_{tag}.png")
            print(f"  → render_head_{tag}.png")
    atlas()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
