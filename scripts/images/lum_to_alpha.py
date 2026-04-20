#!/usr/bin/env python3
"""
粒子贴图后处理:亮度 → alpha。

黑底生成的粒子图,RGB 亮度越高 = 越不透明。
这个脚本把亮度通道转成 alpha,RGB 可选保留原色或置白(染色型)。

用法:
    python lum_to_alpha.py <输入图>... [--no-tint] [--out DIR]

    --no-tint   保留原色(符文、固定色粒子用)
    --out DIR   输出目录(默认原地 *_alpha.png)
"""

from PIL import Image
from pathlib import Path
import argparse
import sys


def luminance_to_alpha(src: Path, tint_white: bool, out_dir: Path | None) -> Path:
    img = Image.open(src).convert("RGB")
    # ITU-R 601 luma 转亮度
    alpha = img.convert("L")

    if tint_white:
        white = Image.new("L", img.size, 255)
        rgba = Image.merge("RGBA", (white, white, white, alpha))
    else:
        r, g, b = img.split()
        rgba = Image.merge("RGBA", (r, g, b, alpha))

    if out_dir:
        out_dir.mkdir(parents=True, exist_ok=True)
        out = out_dir / (src.stem + "_alpha.png")
    else:
        out = src.with_stem(src.stem + "_alpha").with_suffix(".png")
    rgba.save(out)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("inputs", nargs="+", type=Path)
    ap.add_argument("--no-tint", action="store_true", help="保留原色(默认染色型置白)")
    ap.add_argument("--out", type=Path, help="输出目录")
    args = ap.parse_args()

    for src in args.inputs:
        if not src.exists():
            print(f"[skip] {src} 不存在", file=sys.stderr)
            continue
        out = luminance_to_alpha(src, tint_white=not args.no_tint, out_dir=args.out)
        print(f"{src.name} → {out}")


if __name__ == "__main__":
    main()
