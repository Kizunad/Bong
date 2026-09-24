#!/usr/bin/env python3
"""生成上古遗迹崩塌金属青铜残石（Ancient Bronze Debris Block）16×16 方块贴图。

【世界观与设计】：
- 对应 `gu_tong_pian` 出产地 —— 坍缩渊深处上古遗迹废墟。
- 方块不是挖矿石块，而是上古青铜构件崩塌断裂、深埋在碎石与灰烬中的残垣方块。
- 视觉呈现：
  1. 斑驳深青铜基面与粗糙铸造砂眼。
  2. 表面断裂的饕餮雷纹与云纹残迹（深刻槽）。
  3. 斜向断裂的金属崩茬与微弱铜绿。
"""

from __future__ import annotations

import math
from pathlib import Path
from PIL import Image
import numpy as np

import sys
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "core"))
from palette_gutongpian import (
    BRONZE_VOID, BRONZE_DARK, BRONZE_MID, BRONZE_LIT, BRONZE_HIGH,
    FRACTURE_GRAIN, PATINA_MOSS
)

def generate_bronze_debris_block_texture() -> Image.Image:
    """生成 16×16 上古青铜残垣方块贴图。"""
    np.random.seed(314)
    img = Image.new("RGBA", (16, 16), BRONZE_MID + (255,))
    pixels = img.load()

    # 1. 铺设斑驳古青铜铸造基底（带有铸件沙眼与氧化斑驳）
    for y in range(16):
        for x in range(16):
            noise = (math.sin(x * 0.8 + y * 0.5) + math.cos(x * 0.4 - y * 0.9)) * 0.5
            r = np.random.rand() + noise * 0.3
            if r < 0.28:
                col = BRONZE_DARK
            elif r < 0.72:
                col = BRONZE_MID
            else:
                col = BRONZE_LIT
            pixels[x, y] = col + (255,)

    # 2. 铸面断裂崩缺区（左上方为崩裂的金属断茬口，不规则断裂）
    fracture_zone = [
        (0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0),
        (0, 1), (1, 1), (2, 1), (3, 1),
        (0, 2), (1, 2), (2, 2),
        (0, 3), (1, 3)
    ]
    for fx, fy in fracture_zone:
        pixels[fx, fy] = FRACTURE_GRAIN + (255,)
    # 断茬边缘阴影
    pixels[5, 0] = BRONZE_VOID + (255,)
    pixels[4, 1] = BRONZE_VOID + (255,)
    pixels[3, 2] = BRONZE_VOID + (255,)
    pixels[2, 3] = BRONZE_VOID + (255,)

    # 3. 雕琢中央残存的饕餮兽纹与回形云雷纹（深阴刻沟）
    # 中央饕餮兽眼与眉脊
    relief_high = [
        (6, 4, BRONZE_HIGH), (7, 4, BRONZE_HIGH), (8, 4, BRONZE_HIGH), (9, 4, BRONZE_HIGH),
        (5, 5, BRONZE_LIT), (6, 5, BRONZE_HIGH), (9, 5, BRONZE_HIGH), (10, 5, BRONZE_LIT),
        (7, 7, BRONZE_HIGH), (8, 7, BRONZE_HIGH), # 兽鼻
        (6, 9, BRONZE_HIGH), (7, 9, BRONZE_LIT), (8, 9, BRONZE_LIT), (9, 9, BRONZE_HIGH), # 兽口凸棱
        (7, 11, BRONZE_HIGH), (8, 11, BRONZE_HIGH), (7, 12, BRONZE_LIT), (8, 12, BRONZE_LIT) # 衔环凸起
    ]
    for rx, ry, col in relief_high:
        pixels[rx, ry] = col + (255,)

    # 阴线回纹槽（深邃暗纹）
    grooves = [
        (7, 5), (8, 5), (6, 6), (9, 6), (6, 8), (9, 8), (7, 10), (8, 10),
        (12, 7), (13, 7), (13, 8), (14, 8), (12, 12), (13, 12)
    ]
    for gx, gy in grooves:
        pixels[gx, gy] = BRONZE_VOID + (255,)

    # 4. 微量青绿薄锈（角隅沉积）
    patina = [
        (14, 1), (15, 2), (1, 14), (2, 15), (13, 14), (14, 13)
    ]
    for px, py in patina:
        pixels[px, py] = PATINA_MOSS + (255,)

    return img

if __name__ == "__main__":
    tex = generate_bronze_debris_block_texture()
    out_dir = Path(__file__).resolve().parent.parent / "out"
    out_dir.mkdir(parents=True, exist_ok=True)
    tex.save(out_dir / "block_ancient_bronze_debris_16x.png")

    tex_8x = tex.resize((128, 128), Image.NEAREST)
    tex_8x.save(out_dir / "block_ancient_bronze_debris_preview.png")

    client_dir = Path(__file__).resolve().parents[2] / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "block"
    client_dir.mkdir(parents=True, exist_ok=True)
    tex.save(client_dir / "ancient_bronze_debris.png")
    print("Ancient Bronze Debris block texture generated successfully!")
