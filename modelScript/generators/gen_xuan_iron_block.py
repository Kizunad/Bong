#!/usr/bin/env python3
"""生成 16×16 致密玄铁方块（Block of Xuan Iron / 玄铁重金属块）方块贴图。

【世界观与设计】：
- 对应 `xuan_iron`：纯度极高、极其致密沉重的玄铁方块。
- 视觉构成：
  1. 致密深黑金属基底。
  2. 表面几道粗粝锐利的锻击折面与冷银微反光边棱（XUAN_SPEC / XUAN_LIT）。
  3. 四周微倒角与冷硬重金属质感。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_xuaniron import (
    XUAN_VOID, XUAN_DARK, XUAN_MID, XUAN_LIT, XUAN_SPEC, XUAN_RUST
)

def generate_xuaniron_block_texture() -> Image.Image:
    """生成 16×16 致密玄铁块方块贴图。"""
    np.random.seed(999)
    img = Image.new("RGBA", (16, 16), XUAN_DARK + (255,))
    pixels = img.load()

    # 1. 铺设致密深黑玄铁金属质感（冷黑微噪点）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.7 - y * 0.6) + math.cos(x * 0.5 + y * 0.8)) * 0.5
            r = np.random.rand() + noise * 0.2
            if r < 0.25:
                col = XUAN_VOID
            elif r < 0.72:
                col = XUAN_DARK
            elif r < 0.90:
                col = XUAN_MID
            else:
                col = XUAN_RUST
            pixels[x, y] = col + (255,)

    # 2. 方块四周重金属倒角棱线（冷硬工业锻造感）
    # 顶边与左边向光
    for x in range(16):
        pixels[x, 0] = XUAN_LIT + (255,)
    for y in range(16):
        pixels[0, y] = XUAN_LIT + (255,)
    # 左上角高光点
    pixels[0, 0] = XUAN_SPEC + (255,)
    pixels[1, 1] = XUAN_SPEC + (255,)

    # 底边与右边深色阴影边框
    for x in range(16):
        pixels[x, 15] = XUAN_VOID + (255,)
    for y in range(16):
        pixels[15, y] = XUAN_VOID + (255,)

    # 3. 表面斜向锻击金属折面（斜划过方块中央的大金属折痕）
    # 斜切受光带 (x: 4~11, y: 3~10)
    facet_strip = [
        (4, 3, XUAN_SPEC), (5, 3, XUAN_LIT), (6, 4, XUAN_LIT), (7, 4, XUAN_SPEC),
        (5, 4, XUAN_LIT), (6, 5, XUAN_MID), (7, 5, XUAN_MID), (8, 5, XUAN_LIT),
        (7, 6, XUAN_LIT), (8, 6, XUAN_SPEC), (9, 6, XUAN_LIT),
        (8, 7, XUAN_MID), (9, 7, XUAN_MID), (10, 7, XUAN_LIT),
        (9, 8, XUAN_LIT), (10, 8, XUAN_SPEC), (11, 8, XUAN_LIT),
        (10, 9, XUAN_MID), (11, 9, XUAN_MID),
        (11, 10, XUAN_LIT), (12, 10, XUAN_SPEC)
    ]
    for fx, fy, col in facet_strip:
        pixels[fx, fy] = col + (255,)

    # 折痕背光阴影凹槽（紧贴受光带下侧）
    shadow_groove = [
        (4, 4), (5, 5), (6, 6), (7, 7), (8, 8), (9, 9), (10, 10), (11, 11)
    ]
    for sx, sy in shadow_groove:
        pixels[sx, sy] = XUAN_VOID + (255,)

    return img

if __name__ == "__main__":
    tex = generate_xuaniron_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "block_xuan_iron_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "block_xuan_iron_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "xuan_iron_block.png")
    print("XuanIron Block texture generated successfully!")
