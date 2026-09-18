#!/usr/bin/env python3
"""生成工坊粗铁方块（Block of Crude Iron / 粗铁垛）16×16 方块贴图。

【世界观与设计】：
- 对应 `iron_ingot` 的方块形态：9 块粗铁锭在工坊熔炼打造成的重型粗铁方块。
- 视觉构成（满铺交错铸铁砖面，自然平铺 tiling）：
  1. 四块大梯形生铁坯砖紧实咬合（交错砖砌结构，两块在上、两块在下）。
  2. 表面具有极具辨识度的生铁锻打凹坑、氧化铁皮与边缘锻击倒角。
  3. 铁块拼合缝隙间透出深沉的氧化铁渣与微弱暗红余火。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_ironingot import (
    SCALE_VOID, SCALE_DARK, SCALE_MID, SCALE_LIT, SCALE_HIGH,
    EMBER_DARK, EMBER_GLOW
)

def generate_crude_iron_block_texture() -> Image.Image:
    """生成 16×16 工坊生铁砖方块贴图。"""
    np.random.seed(888)
    img = Image.new("RGBA", (16, 16), SCALE_MID + (255,))
    pixels = img.load()

    # 1. 铺设生铁金属晶粒与氧化皮基底（粗糙锻打漫射面）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.9 + y * 0.7) + math.cos(x * 0.5 - y * 0.8)) * 0.5
            r = np.random.rand() + noise * 0.25
            if r < 0.22:
                col = SCALE_DARK
            elif r < 0.75:
                col = SCALE_MID
            else:
                col = SCALE_LIT
            pixels[x, y] = col + (255,)

    # 2. 砖缝（交错铁锭咬合拼缝）
    # 水平主拼缝 y=7, 8
    for x in range(16):
        pixels[x, 7] = SCALE_VOID + (255,)
        pixels[x, 8] = SCALE_DARK + (255,)
    # 上半区纵向拼缝 x=7, 8 (y: 0~7)
    for y in range(0, 8):
        pixels[7, y] = SCALE_VOID + (255,)
        pixels[8, y] = SCALE_DARK + (255,)
    # 下半区纵向拼缝 x=11, 12 与 x=3, 4 (交错结构)
    for y in range(8, 16):
        pixels[11, y] = SCALE_VOID + (255,)
        pixels[12, y] = SCALE_DARK + (255,)

    # 3. 铁砖边缘锤打倒角与高光倒角（Beveled Ingot Edges）
    # 上半区左砖 (0~7, 0~7) 顶边与左边受光
    for x in range(1, 7):
        pixels[x, 0] = SCALE_HIGH + (255,)
        pixels[x, 1] = SCALE_LIT + (255,)
    for y in range(1, 7):
        pixels[0, y] = SCALE_LIT + (255,)
    # 上半区右砖 (8~15, 0~7) 顶边受光
    for x in range(9, 15):
        pixels[x, 0] = SCALE_HIGH + (255,)
        pixels[x, 1] = SCALE_LIT + (255,)

    # 下半区砖块 (0~11, 8~15) 顶边受光
    for x in range(1, 11):
        pixels[x, 9] = SCALE_LIT + (255,)
        pixels[x, 8] = SCALE_DARK + (255,)
    for x in range(13, 16):
        pixels[x, 9] = SCALE_LIT + (255,)

    # 4. 铁锭表面锻打凹坑与铸造砂眼（Hammer Dimples & Scale Flakes）
    dimples = [
        (3, 3, SCALE_VOID), (4, 3, SCALE_DARK), (2, 4, SCALE_DARK),
        (11, 3, SCALE_VOID), (12, 3, SCALE_DARK), (10, 4, SCALE_DARK),
        (5, 12, SCALE_VOID), (6, 12, SCALE_DARK), (4, 13, SCALE_DARK),
        (13, 12, SCALE_VOID), (14, 12, SCALE_DARK)
    ]
    for dx, dy, col in dimples:
        pixels[dx, dy] = col + (255,)

    # 5. 接缝深处冷却中的暗红余火微痕（Ember Seams）
    embers = [
        (7, 4, EMBER_GLOW), (8, 4, EMBER_DARK),
        (4, 7, EMBER_GLOW), (5, 7, EMBER_DARK),
        (11, 7, EMBER_GLOW), (12, 7, EMBER_DARK),
        (11, 11, EMBER_GLOW), (12, 11, EMBER_DARK)
    ]
    for ex, ey, col in embers:
        pixels[ex, ey] = col + (255,)

    return img

if __name__ == "__main__":
    tex = generate_crude_iron_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "block_iron_pile_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "block_iron_pile_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "raw_iron_block.png")
    print("Crude Iron Block texture generated successfully!")
