#!/usr/bin/env python3
"""珂珂达 —— 一键预览：重生成三层模型 + 渲染全部视图到本目录。

产出（均落在 scripts/models/kekeda_goose/）：
  render_skeleton.png         骨架三视图
  render_skeleton_detail.png  骨架特写：头喙栉板 / 蹼足 / 收翼俯视 / 龙骨
  render_muscle.png           骨 + 肌三视图
  render_muscle_bare.png      只软组织三视图
  render_explode.png          延展视图（各件沿离轴方向散开）
  render_plume.png            绒羽/外观层三视图 —— **最终成品**
  render_plume_anatomy.png    绒羽叠在骨+肌上，看包裹关系与羽簇长短
  render_face.png             脸部特写（正 / 3-4 / 侧）
  render_layers.png           三层并排对照
  muscle_atlas.png            逐肌群图集

图上标签一律用 ASCII —— PIL 默认位图字体没有 CJK 字形，中文会渲成一排方框。

用法:
  python3 scripts/models/kekeda_goose/preview.py            # 全部
  python3 scripts/models/kekeda_goose/preview.py --atlas    # 只出肌群图集
  python3 scripts/models/kekeda_goose/preview.py --skip-gen # 不重生成，只渲染
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
MODELS = REPO / "local_models" / "kekeda_goose"

sys.path.insert(0, str(HERE.parent))
from render_bbmodel import render, render_three_view  # noqa: E402

from PIL import Image, ImageDraw  # noqa: E402

BG = (13, 14, 16)
INK = (226, 208, 182)

MUSCLE_GROUPS = [
    ("breast", "Breast: pectoralis / supracoracoideus / tendons"),
    ("trunk", "Trunk: abdominal / spinal / crop / fat pad"),
    ("neck", "Neck: complexus / longus colli"),
    ("wing", "Wing: triceps / biceps / propatagium"),
    ("leg", "Leg: iliotibialis / gastrocnemius / tendons"),
    ("tail", "Tail: levator / depressor / uropygial gland"),
]

SKELETON_SHOTS = [
    ("head / bill / lamellae", 122, 10, (0.0, 14.2, -5.2), 9.0),
    ("head front", 176, 6, (0.0, 14.2, -4.8), 8.0),
    ("webbed feet", 130, 40, (0.0, 1.2, -1.0), 9.5),
    ("folded wing (top)", 20, 46, (0.0, 9.6, 0.0), 12.0),
    ("ribcage / keel", 96, 12, (0.0, 8.0, -1.0), 12.0),
]
FACE_SHOTS = [
    ("face front", 180, 3, (0.0, 14.2, -4.4), 8.0),
    ("face 3/4", 140, 10, (0.0, 14.2, -4.2), 8.5),
    ("face side", 96, 4, (0.0, 14.2, -4.6), 8.5),
]


def _run(script: str, *args: str) -> None:
    subprocess.run([sys.executable, str(HERE / script), *args], check=True, capture_output=True)


def strip(shots, model: str, out: str, size: int = 300) -> None:
    """一排特写拼图。"""
    tiles = [(lab, render(MODELS / model, yaw=y, pitch=p, size=size, focus=(c, s))[0])
             for lab, y, p, c, s in shots]
    gap, hdr = 8, 16
    canvas = Image.new("RGB", (len(tiles) * (size + gap) + gap, size + hdr + gap * 2), BG)
    d = ImageDraw.Draw(canvas)
    for i, (lab, im) in enumerate(tiles):
        x = gap + i * (size + gap)
        canvas.paste(im, (x, hdr + gap))
        d.text((x + 2, 4), lab, fill=INK)
    canvas.save(HERE / out)
    print(f"  → {out}")


def three_view(model: str, out: str, size: int = 460) -> None:
    im, _ = render_three_view(MODELS / model, size=size)
    im.save(HERE / out)
    print(f"  → {out}")


def layers(size: int = 340) -> None:
    """三层并排：一眼看清"球是绒羽撑的，不是身体大"。"""
    tiles = [(lab, render(MODELS / m, yaw=132, pitch=12, size=size, focus=((0.0, 8.2, -1.0), 19.0))[0])
             for lab, m in (("1. skeleton", "KekedaSkeleton.bbmodel"),
                            ("2. + muscle & fat", "KekedaMuscle.bbmodel"),
                            ("3. + down (final)", "KekedaPlume.bbmodel"))]
    gap, hdr = 8, 16
    canvas = Image.new("RGB", (len(tiles) * (size + gap) + gap, size + hdr + gap * 2), BG)
    d = ImageDraw.Draw(canvas)
    for i, (lab, im) in enumerate(tiles):
        x = gap + i * (size + gap)
        canvas.paste(im, (x, hdr + gap))
        d.text((x + 2, 4), lab, fill=INK)
    canvas.save(HERE / "render_layers.png")
    print("  → render_layers.png")


def atlas(size: int = 400) -> None:
    tiles = []
    for key, label in MUSCLE_GROUPS:
        _run("gen_muscle.py", "--group", key)
        im, _ = render(MODELS / f"KekedaMuscle_{key}.bbmodel", yaw=138, pitch=14, size=size,
                       focus=((0.0, 8.2, -1.0), 19.0))
        tiles.append((label, im))
    cols, gap, hdr = 3, 10, 18
    rows = (len(tiles) + cols - 1) // cols
    canvas = Image.new("RGB", (cols * size + gap * (cols + 1), rows * (size + hdr) + gap * (rows + 1)), BG)
    d = ImageDraw.Draw(canvas)
    for i, (label, im) in enumerate(tiles):
        r, c = divmod(i, cols)
        x, y = gap + c * (size + gap), gap + r * (size + hdr + gap)
        d.text((x + 2, y + 3), label, fill=INK)
        canvas.paste(im, (x, y + hdr))
    canvas.save(HERE / "muscle_atlas.png")
    print("  → muscle_atlas.png")


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达预览渲染")
    ap.add_argument("--atlas", action="store_true", help="只出逐肌群图集")
    ap.add_argument("--skip-gen", action="store_true", help="不重生成 bbmodel，只渲染现有的")
    args = ap.parse_args()

    if not args.skip_gen:
        print("生成模型…")
        _run("gen_skeleton.py")
        _run("gen_muscle.py")
        _run("gen_muscle.py", "--only-muscle")
        _run("gen_muscle.py", "--explode", "4")
        _run("gen_plume.py")
        _run("gen_plume.py", "--with-anatomy")

    print("渲染…")
    if not args.atlas:
        three_view("KekedaSkeleton.bbmodel", "render_skeleton.png")
        three_view("KekedaMuscle.bbmodel", "render_muscle.png")
        three_view("KekedaMuscle_bare.bbmodel", "render_muscle_bare.png")
        three_view("KekedaMuscle_explode.bbmodel", "render_explode.png", size=500)
        three_view("KekedaPlume.bbmodel", "render_plume.png")
        three_view("KekedaPlume_anatomy.bbmodel", "render_plume_anatomy.png")
        strip(SKELETON_SHOTS, "KekedaSkeleton.bbmodel", "render_skeleton_detail.png", size=250)
        strip(FACE_SHOTS, "KekedaPlume.bbmodel", "render_face.png", size=330)
        layers()
    atlas()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
