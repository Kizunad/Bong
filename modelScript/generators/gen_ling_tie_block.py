#!/usr/bin/env python3
"""生成 16×16 血谷灵铁矿（LingTie Ore Block）方块贴图。

【世界观与设计】：
- 对应 `ling_tie`：血谷深层矿脉中的致密冷铁方块。
- 视觉构成：
  1. 深邃的青黑色冷金属与玄岩共生基底。
  2. 表面纵横密布如闪电/冰裂般的青碧色储灵裂纹（Teal Qi Veins）。
  3. 裂隙交汇处透出极亮青白冰光（QI_HIGH），与 3D 水滴梭形矿核完全同构。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_lingtie import (
    IRON_VOID, IRON_DARK, IRON_MID, IRON_LIT, IRON_SPEC,
    QI_DEEP, QI_BODY, QI_HIGH
)

def generate_lingtie_ore_block_texture() -> Image.Image:
    """生成 16×16 血谷灵铁矿石方块贴图。"""
    np.random.seed(909)
    img = Image.new("RGBA", (16, 16), IRON_DARK + (255,))
    pixels = img.load()

    # 1. 铺设致密青黑冷铁金属基底（漫射光泽与深黑金属晶粒）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.7 + y * 0.5) + math.cos(x * 0.4 - y * 0.8)) * 0.5
            r = np.random.rand() + noise * 0.25
            if r < 0.25:
                col = IRON_VOID
            elif r < 0.72:
                col = IRON_DARK
            elif r < 0.92:
                col = IRON_MID
            else:
                col = IRON_LIT
            pixels[x, y] = col + (255,)

    # 冷金属断口高光点
    glints = [(3, 4), (11, 2), (5, 12), (13, 11), (2, 9)]
    for gx, gy in glints:
        pixels[gx, gy] = IRON_SPEC + (255,)

    # 2. 纵横交错的青碧储灵裂纹网（与原画标志性的冰裂纹网一致）
    # 主储灵脉 A（自左下延伸至右上）
    vein_main = [
        (2, 13, QI_DEEP), (3, 12, QI_BODY), (4, 11, QI_BODY), (5, 11, QI_HIGH),
        (6, 10, QI_BODY), (7, 9, QI_BODY), (8, 9, QI_HIGH), (8, 8, QI_BODY),
        (9, 7, QI_BODY), (10, 6, QI_HIGH), (11, 5, QI_BODY), (12, 4, QI_BODY),
        (13, 4, QI_DEEP)
    ]
    for vx, vy, col in vein_main:
        pixels[vx, vy] = col + (255,)

    # 次级分支脉 B（自中部分叉延伸至左上）
    vein_branch_nw = [
        (6, 9, QI_DEEP), (5, 8, QI_BODY), (5, 7, QI_HIGH),
        (4, 6, QI_BODY), (3, 5, QI_BODY), (2, 5, QI_DEEP)
    ]
    for vx, vy, col in vein_branch_nw:
        pixels[vx, vy] = col + (255,)

    # 次级分支脉 C（自右下延伸汇聚）
    vein_branch_se = [
        (9, 13, QI_DEEP), (9, 12, QI_BODY), (9, 11, QI_HIGH),
        (8, 10, QI_BODY), (10, 10, QI_BODY), (11, 9, QI_DEEP)
    ]
    for vx, vy, col in vein_branch_se:
        pixels[vx, vy] = col + (255,)

    # 裂口深处伴生暗青晕光
    halos = [
        (4, 12), (6, 11), (7, 10), (9, 8), (11, 6), (5, 9), (9, 10)
    ]
    for hx, hy in halos:
        if pixels[hx, hy][:3] == IRON_DARK or pixels[hx, hy][:3] == IRON_MID:
            pixels[hx, hy] = QI_DEEP + (255,)

    return img

if __name__ == "__main__":
    tex = generate_lingtie_ore_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_ling_tie_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_ling_tie_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_ling_tie.png")
    print("LingTie Ore Block texture generated successfully!")
