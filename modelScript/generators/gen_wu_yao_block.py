#!/usr/bin/env python3
"""生成 16×16 原生态聚阴黑曜石矿方块贴图。

【世界观与原画深度契合】：
- 贴图展现深黑火山玻璃基质（带天然斜向冷黑流纹与贝壳状同心断口）。
- 正中天然崩裂开一道十字交叉的聚阴金芒细裂，深处隐隐透出炽亮金核。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_wuyao import (
    OBS_VOID, OBS_DARK, OBS_MID, OBS_EDGE, OBS_SPEC,
    GOLD_CORE, GOLD_MID, GOLD_DEEP
)

def generate_wuyao_ore_block_texture() -> Image.Image:
    """生成 16×16 天然黑曜石矿石贴图。"""
    np.random.seed(520)
    img = Image.new("RGBA", (16, 16), OBS_DARK + (255,))
    pixels = img.load()

    # 1. 铺设天然火山玻璃岩基底（同心贝壳流纹）
    cx, cy = 7.5, 7.5
    for y in range(16):
        for x in range(16):
            r = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)
            wave = math.sin(r * 1.6 + (x * 0.2 - y * 0.3))
            if wave < -0.4:
                col = OBS_VOID
            elif wave < 0.2:
                col = OBS_DARK
            elif wave < 0.7:
                col = OBS_MID
            else:
                col = OBS_EDGE
            pixels[x, y] = col + (255,)

    # 2. 贝壳状玻璃断口冷光弧线
    arc = [
        (3, 3, OBS_SPEC), (4, 4, OBS_EDGE), (5, 4, OBS_SPEC),
        (4, 11, OBS_SPEC), (5, 12, OBS_EDGE), (11, 4, OBS_SPEC), (12, 5, OBS_EDGE),
        (11, 11, OBS_EDGE), (12, 11, OBS_SPEC)
    ]
    for ax, ay, col in arc:
        pixels[ax, ay] = col + (255,)

    # 3. 中心十字聚阴金裂（透出欺天暗金）
    core_fissure = [
        (7, 7, GOLD_CORE), (8, 7, GOLD_CORE), (7, 8, GOLD_CORE), (8, 8, GOLD_CORE),
        (6, 7, GOLD_MID), (9, 7, GOLD_MID), (7, 6, GOLD_MID), (8, 9, GOLD_MID),
        (5, 7, GOLD_DEEP), (10, 7, GOLD_DEEP), (7, 5, GOLD_DEEP), (8, 10, GOLD_DEEP),
        (6, 6, GOLD_DEEP), (9, 8, GOLD_DEEP), (8, 6, GOLD_MID), (7, 9, GOLD_MID)
    ]
    for fx, fy, col in core_fissure:
        pixels[fx, fy] = col + (255,)

    return img

if __name__ == "__main__":
    tex = generate_wuyao_ore_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "ore_wu_yao_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "ore_wu_yao_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ore_wu_yao.png")
    print("WuYao ore block texture generated successfully!")
