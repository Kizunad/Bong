#!/usr/bin/env python3
"""
符文字符贴图渲染(rune_char 粒子)— 多字体矩阵。

四种毛笔字体 × N 个字符 = 每字多个变体,给 VfxPlayer 随机挑。
命名:rune_char_<unicode>_<font_key>.png(如 rune_char_6555_kai.png)

用法:
    python render_rune_chars.py [--out DIR] [--chars "敕令封破道"] [--fonts kai xing cao cang]
"""

from PIL import Image, ImageDraw, ImageFont, ImageFilter
from pathlib import Path
import argparse

DEFAULT_CHARS = "敕令封破道"
FONT_DIR = Path(__file__).parent / "fonts"
FONTS = {
    "kai":  (FONT_DIR / "MaShanZheng-Regular.ttf",   "楷 · 马善政"),
    "xing": (FONT_DIR / "ZhiMangXing-Regular.ttf",   "行 · 智莽行"),
    "cao":  (FONT_DIR / "LiuJianMaoCao-Regular.ttf", "草 · 柳建毛草"),
    "cang": (FONT_DIR / "LongCang-Regular.ttf",      "草 · 龙藏"),
}

GOLD = (255, 210, 100)          # 字体主色
BLOOM_INNER_TINT = (255, 220, 120)  # 内层 bloom(紧贴字)
BLOOM_OUTER_TINT = (255, 180, 60)   # 外层 bloom(发散)
SIZE = 64
TARGET_FILL_RATIO = 0.82        # 字形占 canvas 的比例(0.82 给 bloom 留边)
SCALE = 4                       # oversample 倍数(抗锯齿)


def fit_font_size(font_path: Path, char: str, target_px: int) -> int:
    """二分搜索最大字号使 char 的 bbox 长边 ≈ target_px。"""
    lo, hi = 8, 512
    while lo < hi:
        mid = (lo + hi + 1) // 2
        f = ImageFont.truetype(str(font_path), mid)
        l, t, r, b = f.getbbox(char)
        longest = max(r - l, b - t)
        if longest <= target_px:
            lo = mid
        else:
            hi = mid - 1
    return lo


def render_rune(char: str, font_path: Path, out_path: Path) -> None:
    big = SIZE * SCALE
    target_px = int(big * TARGET_FILL_RATIO)
    font_size = fit_font_size(font_path, char, target_px)
    font = ImageFont.truetype(str(font_path), font_size)

    # 1. 纯字层(金色,无描边)
    glyph = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    dg = ImageDraw.Draw(glyph)
    dg.text((big // 2, big // 2), char, fill=GOLD + (255,), font=font, anchor="mm")

    # 2. 两层 bloom
    alpha = glyph.split()[3]
    inner_blur = alpha.filter(ImageFilter.GaussianBlur(SCALE * 1))  # 紧贴
    outer_blur = alpha.filter(ImageFilter.GaussianBlur(SCALE * 4))  # 外扩
    # 外层衰减些以免盖过字
    outer_blur = outer_blur.point(lambda v: int(v * 0.7))

    inner_bloom = Image.new("RGBA", glyph.size, BLOOM_INNER_TINT + (0,))
    inner_bloom.putalpha(inner_blur)
    outer_bloom = Image.new("RGBA", glyph.size, BLOOM_OUTER_TINT + (0,))
    outer_bloom.putalpha(outer_blur)

    # 3. 堆叠:外 bloom → 内 bloom → 字
    composed = Image.new("RGBA", glyph.size, (0, 0, 0, 0))
    composed = Image.alpha_composite(composed, outer_bloom)
    composed = Image.alpha_composite(composed, inner_bloom)
    composed = Image.alpha_composite(composed, glyph)

    # 4. 下采样
    composed.resize((SIZE, SIZE), Image.LANCZOS).save(out_path)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--chars", default=DEFAULT_CHARS)
    ap.add_argument("--out", type=Path, default=Path("."))
    ap.add_argument("--fonts", nargs="+", default=list(FONTS.keys()),
                    choices=list(FONTS.keys()),
                    help=f"font keys, available: {', '.join(FONTS.keys())}")
    args = ap.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)

    for ch in args.chars:
        for fkey in args.fonts:
            font_path, label = FONTS[fkey]
            if not font_path.exists():
                print(f"[skip] {ch} × {fkey}: {font_path} 不存在")
                continue
            out = args.out / f"rune_char_{ord(ch):04x}_{fkey}.png"
            render_rune(ch, font_path, out)
            print(f"{ch} × {fkey:5s} ({label}) → {out.name}")


if __name__ == "__main__":
    main()
