#!/usr/bin/env python3
"""生成与雄黄 3D 模型完全同构色盘的 16×16 矿石方块贴图。

世界观背景：
  洞穴深层 / 尸骸附近产，焦黑粗糙岩石中夹杂着半透明蜜蜡金黄晶簇。

输出：
  - `modelScript/out/ore_xiong_huang_16x.png`（预览）
  - `client/src/main/resources/assets/bong/textures/block/ore_xiong_huang.png`
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_xionghuang import (
    ROCK_DARK, ROCK_BASE, ROCK_MID,
    AMBER_DEEP, AMBER_BODY, AMBER_LIT, AMBER_HIGH,
    SULFUR_SEAM
)

def generate_xionghuang_block_texture() -> Image.Image:
    """生成 16×16 洞穴焦岩雄黄矿石纹理。"""
    np.random.seed(66) # 固定种子
    img = Image.new("RGBA", (16, 16), ROCK_BASE + (255,))
    pixels = img.load()
    
    # 1. 铺设焦黑灰黑洞穴岩底
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.7 + y * 0.8) + math.cos(x * 0.5 - y * 0.6)) * 0.5
            r = np.random.rand() + noise * 0.35
            if r < 0.32:
                col = ROCK_DARK
            elif r < 0.75:
                col = ROCK_BASE
            else:
                col = ROCK_MID
            pixels[x, y] = col + (255,)
            
    # 2. 雕琢焦黑碎裂石缝
    cracks = [
        (4, 2), (5, 3), (6, 4), (7, 4), (8, 5), (9, 6),
        (3, 8), (4, 9), (5, 10), (6, 11), (7, 11),
        (10, 8), (11, 9), (12, 10), (13, 11)
    ]
    for cx, cy in cracks:
        pixels[cx, cy] = ROCK_DARK + (255,)
        
    # 3. 嵌生蜜蜡晶黄粗棱晶斑（与 3D 模型的中央粗棱主晶与侧倾晶群呼应）
    # 主晶簇 A（中上方 6~10, 3~7）
    cluster_main = [
        (7, 3, AMBER_HIGH),
        (6, 4, AMBER_LIT), (7, 4, AMBER_HIGH), (8, 4, AMBER_LIT),
        (6, 5, AMBER_BODY), (7, 5, AMBER_BODY), (8, 5, AMBER_BODY), (9, 5, AMBER_DEEP),
        (5, 6, AMBER_LIT), (6, 6, AMBER_BODY), (7, 6, AMBER_DEEP), (8, 6, AMBER_DEEP),
        (6, 7, AMBER_DEEP), (7, 7, AMBER_DEEP),
    ]
    for ox, oy, col in cluster_main:
        pixels[ox, oy] = col + (255,)
        
    # 侧生晶斑 B（左下角 2~5, 10~13）
    cluster_sub1 = [
        (3, 10, AMBER_HIGH),
        (2, 11, AMBER_LIT), (3, 11, AMBER_BODY), (4, 11, AMBER_DEEP),
        (3, 12, AMBER_BODY), (4, 12, AMBER_DEEP),
        (3, 13, AMBER_DEEP),
    ]
    for ox, oy, col in cluster_sub1:
        pixels[ox, oy] = col + (255,)
        
    # 伴生小晶点 C（右下 11~13, 9~12）
    cluster_sub2 = [
        (12, 9, AMBER_HIGH),
        (11, 10, AMBER_LIT), (12, 10, AMBER_BODY),
        (12, 11, AMBER_BODY), (13, 11, AMBER_DEEP),
        (12, 12, AMBER_DEEP),
    ]
    for ox, oy, col in cluster_sub2:
        pixels[ox, oy] = col + (255,)
        
    # 4. 晶石接缝处透出的硫磺黄晕（SULFUR_SEAM）
    seams = [
        (8, 3), (5, 5), (9, 4), (5, 7), (8, 7), (2, 10), (4, 13), (11, 9), (13, 10)
    ]
    for sx, sy in seams:
        if 0 <= sx < 16 and 0 <= sy < 16:
            pixels[sx, sy] = SULFUR_SEAM + (255,)
            
    return img

if __name__ == "__main__":
    tex = generate_xionghuang_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_xiong_huang_16x.png")
    
    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_xiong_huang_preview.png")
    
    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_xiong_huang.png")
    print("XiongHuang block texture generated successfully!")
