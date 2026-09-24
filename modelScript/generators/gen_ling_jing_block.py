#!/usr/bin/env python3
"""生成灵晶深岩晶洞（Spirit Crystal Geode Block）16×16 方块贴图。

【世界观与设计】：
- 对应 `ling_jing`：高纯度天地真元在深层玄岩中孕育的天然晶洞方块（Geode）。
- 视觉构成：
  1. 外围为致密的深灰玄岩硬壳。
  2. 中心向内深凹破裂，露出内部剔透凹陷的幽蓝、天蓝天然多面晶簇。
  3. 晶洞最深处微泛蓝白天道灵光（与 3D 模型的双锥晶核完全呼应）。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_lingjing import (
    CRYSTAL_VOID, CRYSTAL_DEEP, CRYSTAL_MID, CRYSTAL_LIT, CRYSTAL_HIGH,
    ROCK_DARK, ROCK_BASE, ROCK_LIT
)

def generate_lingjing_geode_block_texture() -> Image.Image:
    """生成 16×16 灵晶深岩晶洞方块贴图。"""
    np.random.seed(618)
    img = Image.new("RGBA", (16, 16), ROCK_BASE + (255,))
    pixels = img.load()

    # 1. 铺设外围深灰玄岩基底（颗粒石质）
    cx, cy = 7.5, 7.5
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.8 + y * 0.6) + math.cos(x * 0.5 - y * 0.7)) * 0.5
            r = np.random.rand() + noise * 0.25
            if r < 0.28:
                col = ROCK_DARK
            elif r < 0.72:
                col = ROCK_BASE
            else:
                col = ROCK_LIT
            pixels[x, y] = col + (255,)

    # 2. 晶洞凹陷破裂圈（向内凹陷，边缘深色阴影环）
    for y in range(16):
        for x in range(16):
            dist = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
            if dist < 5.8:
                pixels[x, y] = ROCK_DARK + (255,)
            if dist < 5.0:
                pixels[x, y] = CRYSTAL_VOID + (255,)

    # 3. 晶洞内部幽蓝剔透晶簇面（凹陷内壁生长出晶尖）
    # 中心晶核与棱面（半径 < 4.2）
    for y in range(16):
        for x in range(16):
            dist = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
            if dist < 4.2:
                if dist < 1.4:
                    col = CRYSTAL_HIGH # 晶洞深处天道灵光
                elif dist < 2.6:
                    col = CRYSTAL_LIT if (x + y) % 2 == 0 else CRYSTAL_MID
                elif dist < 3.8:
                    col = CRYSTAL_MID if (x * y) % 3 == 0 else CRYSTAL_DEEP
                else:
                    col = CRYSTAL_DEEP
                pixels[x, y] = col + (255,)

    # 晶洞内突出锐利向心晶尖（尖刺点缀）
    crystal_tips = [
        (7, 5, CRYSTAL_HIGH), (8, 5, CRYSTAL_LIT),
        (5, 7, CRYSTAL_LIT), (5, 8, CRYSTAL_HIGH),
        (10, 7, CRYSTAL_HIGH), (10, 8, CRYSTAL_LIT),
        (7, 10, CRYSTAL_LIT), (8, 10, CRYSTAL_HIGH),
        (6, 6, CRYSTAL_HIGH), (9, 9, CRYSTAL_HIGH)
    ]
    for tx, ty, col in crystal_tips:
        pixels[tx, ty] = col + (255,)

    return img

if __name__ == "__main__":
    tex = generate_lingjing_geode_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "block_ling_jing_geode_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "block_ling_jing_geode_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_ling_jing.png")
    print("LingJing Geode Block texture generated successfully!")
