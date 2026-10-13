"""手持器物共用的 16² 通用材质样本：旧木、凡铁、亮边铁、麻绳、皮垫。

OBJ 链每个面整张铺满，所以这里画的都是「材质样本」而不是某个面的具体图案
（见 `held_item_common` 的 UV 约定）。每个函数收一个 seed，同一件器物里不同部位用
不同 seed，同类材质才不会逐像素重复。

`gen_bao_chu` / `gen_array_flag_held` 里各有一份自己的贴图函数（已过审，输出不动）；
刮刀、骨骸钳、钝气夹从这里取，避免再抄第四、五、六份。
"""

from __future__ import annotations

import random

from bbmodel_maker.render.held_item_common import blotch, noise_fill
from PIL import Image


def old_wood(seed: int, base: tuple[int, int, int] = (136, 99, 60)) -> Image.Image:
    """旧木：浅褐，汗渍压暗、磨亮各几块，纵向纤维只画两条（多了读成条纹布）。"""
    rng = random.Random(seed)
    img = Image.new("RGBA", (16, 16), base + (255,))
    noise_fill(img, rng, base, 7, warm=3)
    dark = tuple(max(0, c - 24) for c in base)
    light = tuple(min(255, c + 22) for c in base)
    blotch(img, rng, 3, dark, (2.0, 4.0))
    blotch(img, rng, 3, light, (1.8, 3.4))
    pixels = img.load()
    fibre = tuple(max(0, c - 20) for c in base) + (255,)
    for x in (4, 11):
        for y in range(16):
            if (x * 5 + y * 3) % 4:
                pixels[x, y] = fibre
    return img


def pitted_iron(seed: int, base: tuple[int, int, int] = (92, 88, 84)) -> Image.Image:
    """凡铁：暗哑灰铁，锈斑和凹坑，没有反光（凡铁不是灵铁）。"""
    rng = random.Random(seed)
    img = Image.new("RGBA", (16, 16), base + (255,))
    noise_fill(img, rng, base, 9)
    blotch(img, rng, 5, (128, 78, 46), (1.6, 3.4))
    blotch(img, rng, 4, tuple(max(0, c - 30) for c in base), (1.4, 2.8))
    blotch(img, rng, 2, tuple(min(255, c + 26) for c in base), (1.6, 3.0))
    return img


def worn_edge(seed: int, rust_blotches: int = 3) -> Image.Image:
    """磨亮的刃口 / 夹口：比铁身亮一档，`rust_blotches` 控制残锈多少。"""
    rng = random.Random(seed)
    base = (138, 134, 126)
    img = Image.new("RGBA", (16, 16), base + (255,))
    noise_fill(img, rng, base, 8)
    blotch(img, rng, 3, (166, 162, 154), (1.8, 3.2))
    blotch(img, rng, rust_blotches, (132, 74, 40), (1.6, 3.0))
    return img


def hemp_cord(seed: int) -> Image.Image:
    """麻绳：搓过的黄麻，斜向拧纹（斜纹的方向和密度就是「绳」的全部识别特征）。"""
    rng = random.Random(seed)
    base = (146, 128, 94)
    img = Image.new("RGBA", (16, 16), base + (255,))
    noise_fill(img, rng, base, 7, warm=3)
    pixels = img.load()
    for start, colour in ((range(-8, 16, 3), (116, 100, 72, 255)), (range(-6, 16, 6), (170, 152, 116, 255))):
        for offset in start:
            for step in range(16):
                x, y = offset + step, 15 - step
                if 0 <= x < 16 and 0 <= y < 16:
                    pixels[x, y] = colour
    return img


def hide_pad(seed: int) -> Image.Image:
    """皮垫：深褐旧皮，磨亮的边和细小的裂纹。"""
    rng = random.Random(seed)
    base = (96, 68, 48)
    img = Image.new("RGBA", (16, 16), base + (255,))
    noise_fill(img, rng, base, 7, warm=3)
    blotch(img, rng, 4, (72, 48, 32), (2.0, 4.0))
    blotch(img, rng, 3, (124, 92, 66), (1.8, 3.2))
    pixels = img.load()
    for _ in range(3):                                   # 细裂纹：短、暗、顺着一个方向
        x, y = rng.randint(1, 13), rng.randint(0, 12)
        for step in range(rng.randint(2, 4)):
            pixels[x + step % 2, y + step] = (62, 42, 28, 255)
    return img


def pale_bone(seed: int) -> Image.Image:
    """老骨：泛黄米白，带暗黄斑和一两处更白的磨亮处（骨套 / 骨饰通用）。"""
    rng = random.Random(seed)
    base = (214, 200, 168)
    img = Image.new("RGBA", (16, 16), base + (255,))
    noise_fill(img, rng, base, 7, warm=2)
    blotch(img, rng, 4, (178, 160, 124), (1.6, 3.2))
    blotch(img, rng, 2, (236, 226, 198), (1.4, 2.6))
    return img
