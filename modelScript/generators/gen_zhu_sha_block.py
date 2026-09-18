#!/usr/bin/env python3
"""生成与朱砂 3D 原矿模型完全同构色盘的 16×16 矿石方块贴图。

世界观背景：
  火山 / 血谷深层产，如凝固血髓与火山黑岩交织。

输出：
  - `modelScript/out/ore_zhu_sha_16x.png`（预览）
  - `client/src/main/resources/assets/bong/textures/block/ore_zhu_sha.png`
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_zhusha import (
    CRUST_DARK, CRUST_BASE, CRUST_MID,
    BLOOD_DEEP, BLOOD_BODY, BLOOD_LIT, BLOOD_HIGH,
    MAGMA_GLOW
)

def generate_zhusha_block_texture() -> Image.Image:
    """生成 16×16 火山血谷朱砂矿石纹理。"""
    np.random.seed(88) # 固定种子
    img = Image.new("RGBA", (16, 16), CRUST_BASE + (255,))
    pixels = img.load()
    
    # 1. 铺设火山黑曜/玄武岩围岩基底（深黑焦褐）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.9 + y * 0.6) + math.cos(x * 0.4 - y * 0.8)) * 0.5
            r = np.random.rand() + noise * 0.3
            if r < 0.35:
                col = CRUST_DARK
            elif r < 0.75:
                col = CRUST_BASE
            else:
                col = CRUST_MID
            pixels[x, y] = col + (255,)
            
    # 2. 火山岩层开裂熔隙（深熔沟）
    chasm = [
        (3, 2), (4, 3), (5, 4), (6, 5), (7, 6), (7, 7), (8, 8), (9, 9), (10, 10),
        (11, 11), (12, 12), (13, 13),
        (5, 8), (6, 9), (7, 10), (8, 11), (9, 12),
        (8, 4), (9, 5), (10, 5), (11, 6)
    ]
    for cx, cy in chasm:
        pixels[cx, cy] = CRUST_DARK + (255,)
        
    # 3. 巨幅血髓晶矿露出（多面体血玉质感，侵蚀贯穿）
    # 大晶体 A（中左主矿核 4~9, 4~9）
    ore_core = [
        (6, 6, BLOOD_HIGH),
        (5, 5, BLOOD_LIT), (6, 5, BLOOD_LIT), (7, 5, BLOOD_BODY),
        (5, 6, BLOOD_LIT), (7, 6, BLOOD_BODY), (8, 6, BLOOD_DEEP),
        (4, 7, BLOOD_LIT), (5, 7, BLOOD_BODY), (6, 7, BLOOD_BODY), (7, 7, BLOOD_DEEP),
        (5, 8, BLOOD_BODY), (6, 8, BLOOD_DEEP), (7, 8, BLOOD_DEEP),
        (6, 9, BLOOD_DEEP),
    ]
    for ox, oy, col in ore_core:
        pixels[ox, oy] = col + (255,)
        
    # 侧伴生矿带 B（右上小晶斑 10~13, 4~7）
    ore_flank_1 = [
        (11, 4, BLOOD_HIGH),
        (10, 5, BLOOD_LIT), (11, 5, BLOOD_BODY), (12, 5, BLOOD_BODY),
        (11, 6, BLOOD_BODY), (12, 6, BLOOD_DEEP),
        (12, 7, BLOOD_DEEP),
    ]
    for ox, oy, col in ore_flank_1:
        pixels[ox, oy] = col + (255,)
        
    # 下方伴生矿带 C（左下 2~4, 11~13）
    ore_flank_2 = [
        (3, 11, BLOOD_LIT),
        (2, 12, BLOOD_BODY), (3, 12, BLOOD_BODY), (4, 12, BLOOD_DEEP),
        (3, 13, BLOOD_DEEP),
    ]
    for ox, oy, col in ore_flank_2:
        pixels[ox, oy] = col + (255,)
        
    # 4. 炽火内生脉络（MAGMA_GLOW，血髓晶体裂隙中的熔岩火光）
    magma_sparks = [
        (7, 4), (4, 6), (8, 5), (6, 8), (10, 4), (12, 4), (13, 6), (2, 11), (4, 13)
    ]
    for mx, my in magma_sparks:
        if 0 <= mx < 16 and 0 <= my < 16:
            pixels[mx, my] = MAGMA_GLOW + (255,)
            
    return img

if __name__ == "__main__":
    tex = generate_zhusha_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_zhu_sha_16x.png")
    
    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_zhu_sha_preview.png")
    
    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_zhu_sha.png")
    print("ZhuSha block texture generated successfully!")
