#!/usr/bin/env python3
"""生成与丹砂 3D 模型完全同构色盘的 16×16 矿石方块贴图。

输出：
- `modelScript/out/ore_dan_sha_16x.png`（预览与对照）
- `client/src/main/resources/assets/bong/textures/block/ore_dan_sha.png`
"""

from __future__ import annotations

import math
import os
import sys
from pathlib import Path
from PIL import Image
import numpy as np

# 导入共享色板
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_dansha import (
    ROCK_DARK, ROCK_BASE, ROCK_MID, ROCK_LIT,
    CRYSTAL_DEEP, CRYSTAL_BODY, CRYSTAL_LIT, CRYSTAL_HIGHLIGHT,
    VEIN_GLOW
)

def generate_dansha_block_texture() -> Image.Image:
    """生成 16×16 的原生红岩丹砂矿石纹理。"""
    np.random.seed(42) # 固定随机种子保持确定性
    img = Image.new("RGBA", (16, 16), ROCK_BASE + (255,))
    pixels = img.load()
    
    # 1. 填充红岩基底（红岩风化层次）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.8 + y * 0.5) + math.cos(x * 0.3 - y * 0.7)) * 0.5
            r_val = np.random.rand() + noise * 0.4
            if r_val < 0.25:
                col = ROCK_DARK
            elif r_val < 0.65:
                col = ROCK_BASE
            elif r_val < 0.88:
                col = ROCK_MID
            else:
                col = ROCK_LIT
            pixels[x, y] = col + (255,)
            
    # 2. 雕琢裂缝与岩层剥落（裂缝用最深岩色）
    cracks = [
        (4, 3), (5, 4), (6, 5), (7, 5), (8, 6), (9, 7),
        (3, 10), (4, 11), (5, 12), (6, 12), (7, 13),
        (10, 2), (11, 3), (12, 4)
    ]
    for cx, cy in cracks:
        pixels[cx, cy] = ROCK_DARK + (255,)
        
    # 3. 嵌入 3 处丹砂结晶簇（与 3D 模型的正中晶柱、侧伴生晶相互呼应）
    # 晶簇 A（主晶簇，位于中上方 6~9, 4~8）
    cluster_a = [
        (7, 4, CRYSTAL_HIGHLIGHT),
        (6, 5, CRYSTAL_LIT), (7, 5, CRYSTAL_BODY), (8, 5, CRYSTAL_LIT),
        (6, 6, CRYSTAL_BODY), (7, 6, CRYSTAL_BODY), (8, 6, CRYSTAL_DEEP),
        (5, 7, CRYSTAL_LIT), (6, 7, CRYSTAL_BODY), (7, 7, CRYSTAL_DEEP),
        (6, 8, CRYSTAL_DEEP), (7, 8, CRYSTAL_DEEP),
    ]
    for cx, cy, col in cluster_a:
        pixels[cx, cy] = col + (255,)
        
    # 晶簇 B（左下小伴生晶 2~4, 10~12）
    cluster_b = [
        (3, 10, CRYSTAL_LIT),
        (2, 11, CRYSTAL_LIT), (3, 11, CRYSTAL_BODY), (4, 11, CRYSTAL_DEEP),
        (3, 12, CRYSTAL_BODY), (4, 12, CRYSTAL_DEEP),
    ]
    for cx, cy, col in cluster_b:
        pixels[cx, cy] = col + (255,)
        
    # 晶簇 C（右侧微细晶 12~13, 8~10）
    cluster_c = [
        (12, 8, CRYSTAL_HIGHLIGHT),
        (12, 9, CRYSTAL_BODY), (13, 9, CRYSTAL_LIT),
        (13, 10, CRYSTAL_DEEP)
    ]
    for cx, cy, col in cluster_c:
        pixels[cx, cy] = col + (255,)
        
    # 4. 晶簇与岩缝间的渗出微光脉络（VEIN_GLOW）
    veins = [
        (8, 4), (5, 6), (8, 7), (4, 10), (5, 11), (11, 8), (12, 10)
    ]
    for vx, vy in veins:
        if 0 <= vx < 16 and 0 <= vy < 16:
            pixels[vx, vy] = VEIN_GLOW + (255,)
            
    return img

if __name__ == "__main__":
    tex = generate_dansha_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    preview_path = out_dir / "ore_dan_sha_16x.png"
    tex.save(preview_path)
    
    # 放大 8 倍存一张清晰无损预览
    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_dan_sha_preview.png")
    
    # 存入客户端 block 纹理目录
    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_dan_sha.png")
    print("Block texture generated and saved successfully!")
