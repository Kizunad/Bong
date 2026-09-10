#!/usr/bin/env python3
"""把 gimage 黑墨身法图转成可染色透明遮罩，保持主体比例。"""

import argparse
from pathlib import Path

import numpy as np
from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--inset", type=int, default=12)
    args = parser.parse_args()
    pixels = np.asarray(Image.open(args.source).convert("RGBA"), dtype=np.float32)
    luminance = pixels[:, :, :3] @ np.array([0.299, 0.587, 0.114])
    alpha = np.clip(pixels[:, :, 3] * (1 - luminance / 255), 0, 255).astype(np.uint8)
    mask = Image.fromarray(alpha)
    bounds = mask.point(lambda value: 255 if value > 12 else 0).getbbox()
    if bounds is None:
        raise ValueError("生图没有可用的墨迹主体")
    mask = mask.crop(bounds)
    mask.thumbnail((256 - args.inset * 2, 256 - args.inset * 2), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (256, 256), (255, 255, 255, 0))
    padded_alpha = Image.new("L", canvas.size)
    padded_alpha.paste(mask, ((256 - mask.width) // 2, (256 - mask.height) // 2))
    canvas.putalpha(padded_alpha)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(args.output)
    print(f"{args.output}: RGBA 256x256; source bounds={bounds}; silhouette={mask.size}")


if __name__ == "__main__":
    main()
