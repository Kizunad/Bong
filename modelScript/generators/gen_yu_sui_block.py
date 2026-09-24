#!/usr/bin/env python3
"""生成与玉髓 3D 模型完全同构色盘的 16×16 矿石方块贴图。

世界观背景：
  深渊寒岩 / 鲸落化石缝隙产，墨黑寒岩层层包裹，裂缝中透出青白温润的冷翠玉髓灵光。

输出：
  - `modelScript/out/ore_yu_sui_16x.png`（预览）
  - `client/src/main/resources/assets/bong/textures/block/ore_yu_sui.png`
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_yusui import (
    SHALE_DARK, SHALE_BASE, SHALE_MID,
    JADE_DEEP, JADE_BODY, JADE_LIT, JADE_HIGH,
    GLOW_SEAM
)

def generate_yusui_block_texture() -> Image.Image:
    """生成 16×16 深渊寒岩玉髓矿石纹理。"""
    np.random.seed(99) # 固定种子
    img = Image.new("RGBA", (16, 16), SHALE_BASE + (255,))
    pixels = img.load()
    
    # 1. 铺设墨黑寒页岩底色（层状片理）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.9 - y * 0.4) + math.cos(x * 0.3 + y * 0.8)) * 0.5
            r = np.random.rand() + noise * 0.3
            if r < 0.35:
                col = SHALE_DARK
            elif r < 0.78:
                col = SHALE_BASE
            else:
                col = SHALE_MID
            pixels[x, y] = col + (255,)
            
    # 2. 雕琢页岩层叠碎裂缝
    cracks = [
        (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 10), (10, 11),
        (8, 4), (8, 5), (8, 6), (7, 7), (6, 8), (5, 9),
        (11, 4), (12, 5), (13, 6), (14, 7)
    ]
    for cx, cy in cracks:
        pixels[cx, cy] = SHALE_DARK + (255,)
        
    # 3. 嵌入中心十字开裂的温润青白玉髓（与 3D 模型的十字裂纹与深层玉核呼应）
    # 中央主裂缝玉核（7~9, 6~8）
    core_jade = [
        (7, 7, JADE_HIGH), (8, 7, JADE_LIT),
        (7, 6, JADE_LIT), (8, 6, JADE_BODY),
        (6, 7, JADE_BODY), (9, 7, JADE_BODY),
        (7, 8, JADE_BODY), (8, 8, JADE_DEEP),
        (6, 6, JADE_DEEP), (9, 8, JADE_DEEP),
    ]
    for ox, oy, col in core_jade:
        pixels[ox, oy] = col + (255,)
        
    # 沿裂隙延展的温润冷玉晶带（左上 3~5, 4~6 & 右下 10~13, 10~12）
    arm_nw = [
        (4, 5, JADE_HIGH),
        (3, 4, JADE_LIT), (4, 4, JADE_BODY),
        (5, 5, JADE_BODY), (5, 6, JADE_DEEP),
    ]
    for ox, oy, col in arm_nw:
        pixels[ox, oy] = col + (255,)
        
    arm_se = [
        (11, 10, JADE_HIGH),
        (10, 10, JADE_LIT), (11, 11, JADE_BODY),
        (12, 11, JADE_BODY), (12, 12, JADE_DEEP),
    ]
    for ox, oy, col in arm_se:
        pixels[ox, oy] = col + (255,)
        
    # 4. 裂口冷翠微光（GLOW_SEAM）
    glows = [
        (8, 6), (6, 7), (8, 8), (4, 4), (5, 6), (10, 11), (12, 10), (13, 6)
    ]
    for gx, gy in glows:
        if 0 <= gx < 16 and 0 <= gy < 16:
            pixels[gx, gy] = GLOW_SEAM + (255,)
            
    return img

if __name__ == "__main__":
    tex = generate_yusui_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_yu_sui_16x.png")
    
    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_yu_sui_preview.png")
    
    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_yu_sui.png")
    print("YuSui block texture generated successfully!")
