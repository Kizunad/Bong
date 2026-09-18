#!/usr/bin/env python3
"""生成 16×16 盐蓬晶岩（Salt Crystal Ore / Halite Rock Block）方块贴图。

【世界观与设计】：
- 对应 `salt_crystal`：白盐蓬析出凝聚形成的深蓝半透明晶盐岩块。
- 视觉构成：
  1. 深靛黑与深海蓝交错的结晶盐基底。
  2. 斜向贯穿的冰蓝晶体解理面与晶粒。
  3. 表面天然析出的白霜闪电状裂痕（Frost Lightning），与 3D 斜插主晶 100% 同构。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_saltcrystal import (
    SALT_VOID, SALT_DARK, SALT_INDIGO,
    CRYST_MID, CRYST_LIT, CRYST_HIGH,
    FROST_DEEP, FROST_WHITE
)

def generate_salt_crystal_block_texture() -> Image.Image:
    """生成 16×16 盐蓬结晶岩方块贴图。"""
    np.random.seed(333)
    img = Image.new("RGBA", (16, 16), SALT_DARK + (255,))
    pixels = img.load()

    # 1. 铺设深靛蓝/黑蓝盐晶基质（带天然斜向解理纹路）
    for y in range(16):
        for x in range(16):
            # 斜向 45 度晶体纹理
            diag = math.sin((x + y) * 0.7) + math.cos((x - y) * 0.4)
            r = np.random.rand() + diag * 0.3
            if r < 0.28:
                col = SALT_VOID
            elif r < 0.70:
                col = SALT_DARK
            elif r < 0.90:
                col = SALT_INDIGO
            else:
                col = CRYST_MID
            pixels[x, y] = col + (255,)

    # 2. 斜插的主晶矿脉带（自左下至右上，斜贯方块）
    main_vein = [
        (3, 14, CRYST_MID), (4, 13, CRYST_MID), (5, 12, CRYST_LIT),
        (5, 11, CRYST_MID), (6, 10, CRYST_LIT), (7, 9, CRYST_HIGH),
        (7, 8, CRYST_LIT), (8, 7, CRYST_HIGH), (9, 6, CRYST_LIT),
        (10, 5, CRYST_HIGH), (11, 4, CRYST_LIT), (12, 3, CRYST_MID),
        (13, 2, CRYST_MID),
        # 旁侧伴生厚度
        (6, 11, CRYST_MID), (7, 10, CRYST_LIT), (8, 8, CRYST_MID),
        (9, 7, CRYST_LIT), (10, 6, CRYST_MID), (11, 5, CRYST_MID)
    ]
    for vx, vy, col in main_vein:
        pixels[vx, vy] = col + (255,)

    # 3. 晶体内部的白霜闪电状解理纹（Lightning Frost，极细冰白电光弧）
    frost_lightning = [
        (4, 12, FROST_DEEP), (5, 12, FROST_WHITE),
        (6, 10, FROST_WHITE), (7, 10, FROST_DEEP),
        (7, 9, FROST_WHITE), (8, 8, FROST_WHITE),
        (8, 7, FROST_DEEP), (9, 6, FROST_WHITE),
        (10, 5, FROST_WHITE), (11, 4, FROST_DEEP)
    ]
    for fx, fy, col in frost_lightning:
        pixels[fx, fy] = col + (255,)

    # 4. 次生散落白霜结晶细斑（伴生碎屑）
    frost_spots = [
        (2, 6, FROST_WHITE), (3, 7, CRYST_LIT),
        (12, 10, FROST_WHITE), (13, 11, CRYST_LIT),
        (10, 13, FROST_DEEP), (14, 4, FROST_DEEP)
    ]
    for sx, sy, col in frost_spots:
        pixels[sx, sy] = col + (255,)

    return img

if __name__ == "__main__":
    tex = generate_salt_crystal_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_salt_crystal_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_salt_crystal_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_salt_crystal.png")
    print("Salt Crystal block texture generated successfully!")
