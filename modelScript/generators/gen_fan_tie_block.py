#!/usr/bin/env python3
"""生成 16×16 地表凡铁矿石（FanTie Ore / Surface Iron Ore Block）方块贴图。

【世界观与设计】：
- 对应 `fan_tie`：地表至 y=0 浅层常见矿石，普通石灰岩/花岗岩中嵌有平整立方断口的凡铁原矿。
- 视觉构成：
  1. 普通浅灰岩石基底。
  2. 嵌生的凡铁矿斜方解理大断块（IRON_DARK / IRON_MID / IRON_LIT）。
  3. 斜向贯穿的金色/炽金地火裂纹（GOLD_DEEP / GOLD_MID / GOLD_HIGH）。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_fantie import (
    IRON_VOID, IRON_DARK, IRON_MID, IRON_LIT, IRON_SPEC,
    GOLD_DEEP, GOLD_MID, GOLD_HIGH
)

def generate_fantie_ore_block_texture() -> Image.Image:
    """生成 16×16 地表浅层凡铁矿石方块贴图。"""
    np.random.seed(520)
    # 岩石基底浅灰色
    ROCK_BASE = (112, 114, 116)
    ROCK_DARK = (78, 80, 84)
    ROCK_LIT  = (144, 146, 148)

    img = Image.new("RGBA", (16, 16), ROCK_BASE + (255,))
    pixels = img.load()

    # 1. 铺设地表浅层石灰岩/花岗岩基底
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.8 + y * 0.6) + math.cos(x * 0.4 - y * 0.7)) * 0.5
            r = np.random.rand() + noise * 0.25
            if r < 0.25:
                col = ROCK_DARK
            elif r < 0.78:
                col = ROCK_BASE
            else:
                col = ROCK_LIT
            pixels[x, y] = col + (255,)

    # 2. 嵌生的大块阶梯立方凡铁矿斑（中左核心区 3~12, 3~12）
    iron_facets = [
        # 主立方断块 (4~11, 4~11)
        (5, 4, IRON_LIT), (6, 4, IRON_SPEC), (7, 4, IRON_LIT), (8, 4, IRON_LIT),
        (4, 5, IRON_LIT), (5, 5, IRON_MID), (6, 5, IRON_LIT), (7, 5, IRON_MID), (8, 5, IRON_MID), (9, 5, IRON_DARK),
        (4, 6, IRON_LIT), (5, 6, IRON_MID), (6, 6, IRON_MID), (7, 6, IRON_DARK), (8, 6, IRON_DARK), (9, 6, IRON_VOID),
        (4, 7, IRON_MID), (5, 7, IRON_MID), (6, 7, IRON_DARK), (7, 7, IRON_VOID), (8, 7, IRON_DARK), (9, 7, IRON_VOID),
        (5, 8, IRON_MID), (6, 8, IRON_DARK), (7, 8, IRON_DARK), (8, 8, IRON_DARK), (9, 8, IRON_VOID), (10, 8, IRON_VOID),
        (5, 9, IRON_DARK), (6, 9, IRON_DARK), (7, 9, IRON_VOID), (8, 9, IRON_VOID), (9, 9, IRON_VOID),
        (6, 10, IRON_VOID), (7, 10, IRON_VOID), (8, 10, IRON_VOID)
    ]
    for ox, oy, col in iron_facets:
        pixels[ox, oy] = col + (255,)

    # 3. 伴生外围小立方铁块 (右下 11~13, 10~12)
    flank_iron = [
        (11, 10, IRON_LIT), (12, 10, IRON_SPEC),
        (11, 11, IRON_MID), (12, 11, IRON_DARK), (13, 11, IRON_VOID),
        (12, 12, IRON_VOID)
    ]
    for ox, oy, col in flank_iron:
        pixels[ox, oy] = col + (255,)

    # 4. 对角贯穿金色火裂缝（自左下 5,9 斜穿至右上 8,4）
    gold_crack = [
        (5, 8, GOLD_DEEP), (6, 7, GOLD_MID), (7, 6, GOLD_HIGH),
        (7, 5, GOLD_HIGH), (8, 5, GOLD_MID), (8, 4, GOLD_DEEP),
        (6, 8, GOLD_MID), (7, 7, GOLD_MID), (12, 11, GOLD_MID)
    ]
    for gx, gy, col in gold_crack:
        pixels[gx, gy] = col + (255,)

    return img

if __name__ == "__main__":
    tex = generate_fantie_ore_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_fan_tie_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_fan_tie_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_fan_tie.png")
    print("FanTie Ore Block texture generated successfully!")
