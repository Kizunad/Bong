#!/usr/bin/env python3
"""一次性处理 tmp/ 下的 8 张粒子原图,按 plan-particle-system-v1 §4.1 规格输出到 particles/。

映射表见 PARTICLES。全部生成 <id>_alpha.png 供 client 使用,<id>_src.png 留存。
"""

from PIL import Image
from pathlib import Path

TMP = Path(__file__).parent / "tmp"
OUT = Path(__file__).parent / "particles"
OUT.mkdir(exist_ok=True)

# (src_filename, particle_id, target_size, rotate_cw, seamless_horiz, tint_white, already_rgba)
PARTICLES = [
    ("OpenAI Playground 2026-04-17 at 12.55.43.png", "sword_qi_trail",     (128, 32),  0,   False, True,  False),
    ("OpenAI Playground 2026-04-17 at 12.56.47.png", "sword_slash_arc",    (256, 32),  0,   False, True,  False),
    ("OpenAI Playground 2026-04-17 at 13.01.13.png", "lingqi_ripple",      (256, 256), 0,   False, True,  False),
    ("OpenAI Playground 2026-04-17 at 13.01.47.png", "breakthrough_pillar",(128, 32),  90,  False, True,  False),
    ("OpenAI Playground 2026-04-17 at 13.02.59.png", "enlightenment_dust", (32, 32),   0,   False, True,  False),
    ("OpenAI Playground 2026-04-17 at 13.03.26.png", "tribulation_spark",  (256, 32),  90,  False, True,  False),
    ("OpenAI Playground 2026-04-17 at 13.04.02.png", "flying_sword_trail", (256, 32),  0,   True,  True,  True),
    ("OpenAI Playground 2026-04-17 at 13.19.43.png", "qi_aura",            (64, 64),   0,   False, False, False),  # 保留青色原色,黑底走 lum→α
]


def lum_to_alpha(img: Image.Image, tint_white: bool) -> Image.Image:
    rgb = img.convert("RGB")
    alpha = rgb.convert("L")
    if tint_white:
        white = Image.new("L", rgb.size, 255)
        return Image.merge("RGBA", (white, white, white, alpha))
    r, g, b = rgb.split()
    return Image.merge("RGBA", (r, g, b, alpha))


def seamless_horiz(img: Image.Image, blend_frac: float = 0.1) -> Image.Image:
    """左右各取 blend 宽度的镜像平均,使 tile 无缝。"""
    w, h = img.size
    blend = max(1, int(w * blend_frac))
    arr = img.copy()
    left = arr.crop((0, 0, blend, h))
    right = arr.crop((w - blend, 0, w, h))
    mixed = Image.blend(left, right.transpose(Image.FLIP_LEFT_RIGHT), 0.5)
    # 左端贴 mixed,右端贴 mixed 镜像
    arr.paste(mixed, (0, 0))
    arr.paste(mixed.transpose(Image.FLIP_LEFT_RIGHT), (w - blend, 0))
    return arr


def process_one(entry):
    src_name, pid, size, rotate_cw, seam, tint, already_rgba = entry
    src = TMP / src_name
    img = Image.open(src)

    # 1. 若已 RGBA 保持,否则亮度转 alpha
    if already_rgba:
        rgba = img.convert("RGBA")
    else:
        rgba = lum_to_alpha(img, tint_white=tint)

    # 2. 旋转(PIL 顺时针为负)
    if rotate_cw:
        rgba = rgba.rotate(-rotate_cw, expand=True, resample=Image.BICUBIC)

    # 3. 下采样到目标(LANCZOS 最佳)
    resized = rgba.resize(size, Image.LANCZOS)

    # 4. 水平 seamless(ribbon)
    if seam:
        resized = seamless_horiz(resized, blend_frac=0.05)

    # 5. 输出 _alpha.png(可用) + _src.png(留存便于重处理)
    alpha_out = OUT / f"{pid}.png"
    resized.save(alpha_out)
    # 留原尺寸 _alpha 供大图查看
    full_out = OUT / f"{pid}_full.png"
    rgba.save(full_out)

    print(f"{pid:24s} {src_name[-20:]:20s} → {size[0]:3d}×{size[1]:<3d} alpha mean={sum(resized.split()[3].getdata())/(size[0]*size[1]):.0f}")


if __name__ == "__main__":
    for e in PARTICLES:
        process_one(e)
    print(f"\n输出目录: {OUT}")
