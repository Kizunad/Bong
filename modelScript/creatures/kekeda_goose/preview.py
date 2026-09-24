#!/usr/bin/env python3
"""珂珂达 —— 一键预览：重生成三层模型 + 渲染全部视图到本目录。

产出全部落在 modelScript/creatures/kekeda_goose/renders/ 下，按**看的目的**分四个目录
（编号前缀让它们按重要性排序，成品永远在最上面）：

  1_final/     成品 —— 平时只需要看这个
      three_view.png      正 / 侧 / 3-4
      detail.png          脸部 · 收翼 · 尾 · 游戏观看距离
  2_layers/    分层对照 —— 讲"球是外层撑的，不是身体大"
      three_layers.png    骨架 → +肌肉脂肪 → +外观 并排
      anatomy_cutaway.png 半剖：左半骨+肌，右半外观
  3_skeleton/  骨架层
      three_view.png · detail.png（头喙栉板 / 蹼足 / 收翼俯视 / 龙骨）
  4_muscle/    肌肉层
      three_view.png（骨+肌）· bare.png（纯软组织）
      explode.png（各件散开）· atlas.png（逐肌群图集）

图上标签一律用 ASCII —— PIL 默认位图字体没有 CJK 字形，中文会渲成一排方框。

用法:
  python3 modelScript/creatures/kekeda_goose/preview.py            # 全部
  python3 modelScript/creatures/kekeda_goose/preview.py --atlas    # 只出肌群图集
  python3 modelScript/creatures/kekeda_goose/preview.py --skip-gen # 不重生成，只渲染
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
MODELS = Path(__file__).resolve().parents[2] / "models" / "kekeda_goose"
OUT = HERE / "renders"

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
from bbmodel_maker.render.render_bbmodel import render, render_three_view  # noqa: E402

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
FINAL_SHOTS = [
    ("face 3/4", 148, 10, (0.0, 14.1, -4.6), 9.0),
    ("folded wing", 128, 16, (0.0, 8.4, 0.8), 15.0),
    ("rear 3/4", 325, 14, (0.0, 8.2, -0.6), 16.0),
    ("game distance", 145, 15, (0.0, 8.0, -0.6), 26.0),
]


def _run(script: str, *args: str) -> None:
    subprocess.run([sys.executable, str(HERE / script), *args], check=True, capture_output=True)


# 一律用 MC 口径的面亮度渲外观层：本模块默认的 lambert 光相邻朝向差 2.5 倍，
# 会把体素球的阶梯面照成一身条纹，看着比进游戏丑得多（判"圆不圆滑"时会误导）
SHADE = "mc"


def strip(shots, model: str, out: str, size: int = 300) -> None:
    """一排特写拼图。"""
    tiles = [(lab, render(MODELS / model, yaw=y, pitch=p, size=size, focus=(c, s), shading=SHADE)[0])
             for lab, y, p, c, s in shots]
    gap, hdr = 8, 16
    canvas = Image.new("RGB", (len(tiles) * (size + gap) + gap, size + hdr + gap * 2), BG)
    d = ImageDraw.Draw(canvas)
    for i, (lab, im) in enumerate(tiles):
        x = gap + i * (size + gap)
        canvas.paste(im, (x, hdr + gap))
        d.text((x + 2, 4), lab, fill=INK)
    save(canvas, out)


def save(im, rel: str) -> None:
    path = OUT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path)
    print(f"  → renders/{rel}")


def three_view(model: str, out: str, size: int = 460, shading: str = "lambert") -> None:
    im, _ = render_three_view(MODELS / model, size=size, shading=shading)
    save(im, out)


def layers(size: int = 340) -> None:
    """三层并排：一眼看清"球是绒羽撑的，不是身体大"。"""
    tiles = [(lab, render(MODELS / m, yaw=132, pitch=12, size=size,
                          focus=((0.0, 8.2, -1.0), 19.0), shading=SHADE)[0])
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
    save(canvas, "2_layers/three_layers.png")


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
    save(canvas, "4_muscle/atlas.png")


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
        three_view("KekedaPlume.bbmodel", "1_final/three_view.png", shading=SHADE)
        strip(FINAL_SHOTS, "KekedaPlume.bbmodel", "1_final/detail.png", size=300)
        three_view("KekedaPlume_anatomy.bbmodel", "2_layers/anatomy_cutaway.png")
        layers()
        three_view("KekedaSkeleton.bbmodel", "3_skeleton/three_view.png")
        strip(SKELETON_SHOTS, "KekedaSkeleton.bbmodel", "3_skeleton/detail.png", size=250)
        three_view("KekedaMuscle.bbmodel", "4_muscle/three_view.png")
        three_view("KekedaMuscle_bare.bbmodel", "4_muscle/bare.png")
        three_view("KekedaMuscle_explode.bbmodel", "4_muscle/explode.png", size=500)
    atlas()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
