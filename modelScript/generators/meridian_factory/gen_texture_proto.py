#!/usr/bin/env python3
"""贴图工序样板：用 b01 meridian_straight 对比「平色版」和「程序化像素贴图版」。

产出 model-review/img/meridian_factory/texture_proto/compare.png：
左 = 单件参考图（refs/b01_meridian_straight.png 的 3/4 半幅），
中 = 现在的平色版，右 = 贴图版；中、右同一 3/4 取景。

几何结论不改，只做一处替换：贴图版把 32 个斜交筋丝立方体的**几何**去掉，
筋丝改画进贴图（立方体筋丝是用几何假装线条，贴图线条才是正路）。
"""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageDraw

import gen_meridian_straight as b01
from bbmodel_maker.render.render_bbmodel import render
from pixel_texture import write_textured_bbmodel

REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")
OUT_DIR = REVIEW_DIR / "texture_proto"
REF = REVIEW_DIR / "refs" / "b01_meridian_straight.png"

BG = (119, 119, 119)
VIEW_SIZE = 640
# 参考图右半幅（3/4 图）在原图里的裁切框
REF_CROP = (800, 200, 1500, 780)


def textured_cubes():
    """贴图版几何：去掉用几何假装的筋丝和光斑，其余与平色版完全一致。"""
    return [c for c in b01.all_cubes() if c["material"] != "tendon_fiber"]


def render_pair(yaw: float, pitch: float):
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    flat_path = b01.generate_bbmodel(OUT_DIR / "_flat.bbmodel")
    tex_path = OUT_DIR / "_textured.bbmodel"
    atlas = write_textured_bbmodel(textured_cubes(), tex_path, "meridian_straight_textured")
    atlas.save(OUT_DIR / "atlas.png")
    flat, _ = render(flat_path, yaw=yaw, pitch=pitch, size=VIEW_SIZE, bg=BG, shading="mc")
    textured, _ = render(tex_path, yaw=yaw, pitch=pitch, size=VIEW_SIZE, bg=BG, shading="mc")
    return flat, textured


def labeled(img: Image.Image, text: str) -> Image.Image:
    out = Image.new("RGB", (img.width, img.height + 26), (24, 25, 28))
    out.paste(img, (0, 26))
    ImageDraw.Draw(out).text((8, 8), text, fill=(230, 230, 230))
    return out


def main():
    parser = argparse.ArgumentParser(description="b01 贴图工序样板")
    parser.add_argument("--yaw", type=float, default=35.0)
    parser.add_argument("--pitch", type=float, default=25.0)
    args = parser.parse_args()

    flat, textured = render_pair(args.yaw, args.pitch)
    ref = Image.open(REF).convert("RGB").crop(REF_CROP)
    ref = ref.resize((int(ref.width * VIEW_SIZE / ref.height), VIEW_SIZE), Image.Resampling.LANCZOS)

    tiles = [
        labeled(ref, "REFERENCE (target)"),
        labeled(flat, "FLAT (current)"),
        labeled(textured, "TEXTURED (pixel atlas)"),
    ]
    canvas = Image.new("RGB", (sum(t.width for t in tiles) + 16, tiles[0].height), (28, 29, 33))
    x = 0
    for tile in tiles:
        canvas.paste(tile, (x, 0))
        x += tile.width + 8
    canvas.save(OUT_DIR / "compare.png")
    print(f"✓ compare.png: {OUT_DIR / 'compare.png'}")


if __name__ == "__main__":
    main()
