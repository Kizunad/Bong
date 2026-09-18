#!/usr/bin/env python3
"""生成 16×16 深板岩粗铁矿（CuTie Ore / Deepslate Iron Ore Block）方块贴图。

【世界观与设计】：
- 对应 `cu_tie`：深板岩层中嵌生的粗铁结核矿石。
- 视觉构成：
  1. 深灰深板岩片状层理基底。
  2. 嵌生的大块泥褐风化结核包裹体。
  3. 结核崩裂露出的暖焦生铁多面体晶斑与金橙地热火痕。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_cutie import (
    ORE_VOID, ORE_DARK, ORE_MUD_MID, ORE_MUD_LIT,
    ORE_IRON_MID, ORE_IRON_LIT, ORE_IRON_SPEC,
    FIRE_DEEP, FIRE_MID, FIRE_HIGH
)

def generate_cutie_ore_block_texture() -> Image.Image:
    """生成 16×16 深板岩粗铁矿石方块贴图。"""
    np.random.seed(414)
    img = Image.new("RGBA", (16, 16), ORE_DARK + (255,))
    pixels = img.load()

    # 1. 铺设深板岩岩体基底（深灰黑水平片理）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(y * 1.2) + math.cos(x * 0.4 + y * 0.8)) * 0.5
            r = np.random.rand() + noise * 0.3
            if r < 0.28:
                col = ORE_VOID
            elif r < 0.72:
                col = ORE_DARK
            else:
                col = ORE_IRON_MID
            pixels[x, y] = col + (255,)

    # 2. 嵌生的粗铁风化泥褐结核大斑（中左核心区 3~11, 3~12）
    mud_nodule = [
        (6, 3, ORE_MUD_LIT), (7, 3, ORE_MUD_LIT), (8, 3, ORE_MUD_MID),
        (5, 4, ORE_MUD_LIT), (6, 4, ORE_MUD_MID), (7, 4, ORE_MUD_MID), (8, 4, ORE_MUD_MID), (9, 4, ORE_DARK),
        (4, 5, ORE_MUD_LIT), (9, 5, ORE_DARK), (10, 5, ORE_DARK),
        (3, 6, ORE_MUD_LIT), (10, 6, ORE_DARK), (11, 6, ORE_VOID),
        (3, 7, ORE_MUD_LIT), (10, 7, ORE_DARK),
        (3, 8, ORE_MUD_MID), (9, 8, ORE_DARK), (10, 8, ORE_VOID),
        (4, 9, ORE_MUD_MID), (9, 9, ORE_DARK),
        (4, 10, ORE_DARK), (5, 10, ORE_DARK), (8, 10, ORE_DARK), (9, 10, ORE_VOID),
        (5, 11, ORE_DARK), (6, 11, ORE_VOID), (7, 11, ORE_VOID), (8, 11, ORE_VOID)
    ]
    for mx, my, col in mud_nodule:
        pixels[mx, my] = col + (255,)

    # 3. 结核崩裂露出的生铁多面体金属块（核内金属 5~8, 5~9）
    metallic_facets = [
        (5, 5, ORE_IRON_LIT), (6, 5, ORE_IRON_SPEC), (7, 5, ORE_IRON_LIT), (8, 5, ORE_IRON_MID),
        (4, 6, ORE_IRON_LIT), (5, 6, ORE_IRON_SPEC), (6, 6, ORE_IRON_LIT), (7, 6, ORE_IRON_MID), (8, 6, ORE_DARK),
        (4, 7, ORE_IRON_MID), (5, 7, ORE_IRON_LIT), (6, 7, ORE_IRON_MID), (7, 7, ORE_DARK), (8, 7, ORE_VOID),
        (5, 8, ORE_IRON_MID), (6, 8, ORE_DARK), (7, 8, ORE_VOID),
        (5, 9, ORE_DARK), (6, 9, ORE_VOID)
    ]
    for ox, oy, col in metallic_facets:
        pixels[ox, oy] = col + (255,)

    # 4. 金属块与泥壳开裂接缝中的地热金橙火痕（FIRE）
    fire_cracks = [
        (7, 5, FIRE_HIGH), (8, 6, FIRE_MID), (7, 7, FIRE_HIGH), (8, 7, FIRE_DEEP),
        (6, 8, FIRE_MID), (7, 8, FIRE_DEEP), (9, 7, FIRE_DEEP), (4, 8, FIRE_MID)
    ]
    for fx, fy, col in fire_cracks:
        pixels[fx, fy] = col + (255,)

    return img

if __name__ == "__main__":
    tex = generate_cutie_ore_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_cu_tie_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_cu_tie_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_cu_tie.png")
    print("CuTie Ore Block texture generated successfully!")
