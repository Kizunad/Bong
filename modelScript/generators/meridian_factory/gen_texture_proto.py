#!/usr/bin/env python3
"""贴图工序样板：用 b01 meridian_straight 对比「平色版」和「程序化像素贴图版」。

产出 model-review/img/meridian_factory/texture_proto/compare.png：
左 = 单件参考图（refs/b01_meridian_straight.png 的 3/4 半幅），
中 = 现在的平色版，右 = 贴图版；中、右同一 3/4 取景。

几何只做小改（贴图版）：去掉 32 个斜交筋丝立方体（改画进贴图）；肉箍加 0.8px 鼓边；
管子两端加暖粉封面薄板；骨环端口加一级台阶倒角。
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


COLLAR_CENTERS = (-6.0, 6.0)    # 两个肉箍的中心 z（箍宽 3px：cz ± 1.5）
BULGE_OUT = 0.8                 # 鼓边外凸量
BULGE_HALF = 1.0                # 鼓边沿 z 的半宽（箍宽 3px，鼓中段 2px，两侧各收 0.5px）


def _cube(name: str, lo, hi, material: str, group: str) -> dict:
    return {"name": name, "from": list(lo), "to": list(hi), "group": group, "material": material}


def collar_bulges() -> list:
    """肉箍鼓边：中段外凸 0.8px、两侧收。两侧面各一块，顶 / 底面各两块（避开骨环）。"""
    cubes = []
    for cz in COLLAR_CENTERS:
        tag = "rear" if cz < 0 else "front"
        z0, z1 = cz - BULGE_HALF, cz + BULGE_HALF
        for side, (x0, x1) in (("l", (-5.0 - BULGE_OUT, -5.0)), ("r", (5.0, 5.0 + BULGE_OUT))):
            cubes.append(_cube(f"bulge_{tag}_side_{side}", (x0, 2.0, z0), (x1, 8.0, z1), "flesh_main", "flesh_collars"))
        for side, (x0, x1) in (("l", (-5.0, -2.0)), ("r", (2.0, 5.0))):
            cubes.append(_cube(f"bulge_{tag}_top_{side}", (x0, 9.0, z0), (x1, 9.0 + BULGE_OUT, z1), "flesh_main", "flesh_collars"))
            cubes.append(_cube(f"bulge_{tag}_bot_{side}", (x0, 1.0 - BULGE_OUT, z0), (x1, 1.0, z1), "flesh_main", "flesh_collars"))
    return cubes


def end_caps() -> list:
    """管子两端封面：暖粉薄板，缩进管口 0.1px，8×6 开口位置不变（板在管腔里，管口截面不动）。"""
    return [
        _cube("end_cap_rear", (-3.2, 2.8, -7.9), (3.2, 7.2, -7.4), "tendon_tube", "meridian_tube"),
        _cube("end_cap_front", (-3.2, 2.8, 7.4), (3.2, 7.2, 7.9), "tendon_tube", "meridian_tube"),
    ]


def port_chamfers() -> list:
    """骨环端口倒角：环外加一级 5×5 / 内 3×3、厚 0.25px 的台阶底座（顶、底面各一圈）。"""
    cubes = []
    for cz in COLLAR_CENTERS:
        tag = "rear" if cz < 0 else "front"
        for face, (y0, y1) in (("top", (9.0, 9.25)), ("bot", (0.75, 1.0))):
            frame = (
                ("n", (-2.5, cz - 2.5), (2.5, cz - 1.5)),
                ("s", (-2.5, cz + 1.5), (2.5, cz + 2.5)),
                ("w", (-2.5, cz - 1.5), (-1.5, cz + 1.5)),
                ("e", (1.5, cz - 1.5), (2.5, cz + 1.5)),
            )
            for edge, (xa, za), (xb, zb) in frame:
                cubes.append(_cube(f"chamfer_{tag}_{face}_{edge}", (xa, y0, za), (xb, y1, zb), "bone_main", "ports"))
    return cubes


def textured_cubes():
    """贴图版几何：去掉用几何假装的筋丝；加肉箍鼓边、两端封面、骨环倒角。"""
    base = [c for c in b01.all_cubes() if c["material"] != "tendon_fiber"]
    return base + collar_bulges() + end_caps() + port_chamfers()


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
