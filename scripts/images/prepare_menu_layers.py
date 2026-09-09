#!/usr/bin/env python3
"""从同一母图和图生图遮罩提取主菜单层，保留原始近景像素。"""

import argparse
import shutil
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("resources", type=Path)
    args = parser.parse_args()
    source, resources = args.source, args.resources
    master = Image.open(source / "rift-master-r1.png").convert("RGB")
    background = Image.open(source / "rift-background-r1.png").convert("RGB")
    matte = Image.open(source / "rift-foreground-mask-r1.png").convert("L")
    if master.size != (1536, 1024) or background.size != master.size or matte.size != master.size:
        raise ValueError("母图、背景与遮罩必须同为 1536x1024")
    mask = np.asarray(matte) > 128
    occupancy = float(mask.mean())
    if not 0.05 < occupancy < 0.7:
        raise ValueError(f"前景遮罩占比异常: {occupancy:.3f}")
    destination = resources / "assets/bong/textures/gui/main_menu"
    destination.mkdir(parents=True, exist_ok=True)
    background.save(destination / "valley.png", optimize=True)
    for name, side in (("rock-left", "left"), ("rock-right", "right")):
        selected = mask.copy()
        if side == "left":
            selected[:, master.width // 2:] = False
        else:
            selected[:, :master.width // 2] = False
        alpha = Image.fromarray(selected.astype(np.uint8) * 255).filter(ImageFilter.GaussianBlur(0.55))
        layer = master.convert("RGBA")
        layer.putalpha(alpha)
        layer.save(destination / f"{name}.png", optimize=True)
    font_dir = resources / "assets/bong/font"
    font_dir.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source / "MaShanZheng-Regular.ttf", font_dir / "menu-title.ttf")
    shutil.copyfile(source / "OFL.txt", font_dir / "menu-title-ofl.txt")
    print(f"layers={destination} size={master.size} foreground_coverage={occupancy:.3f}")


if __name__ == "__main__":
    main()
