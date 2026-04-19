"""把 rune_char_<code>_<font>.png 排成 5字×4体 grid,深背景预览。"""
from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

PART = Path(__file__).parent / "particles"
OUT = Path(__file__).parent / "runes_preview.png"

CHARS = "敕令封破道"
FONTS = [("kai", "楷·马善政"), ("xing", "行·智莽行"),
         ("cao", "草·柳建毛草"), ("cang", "草·龙藏")]

CELL = 180           # 单元格像素
SCALE_UP = 2         # 贴图 ×2 显示
PAD_TOP = 40         # 顶部字体名行高
PAD_LEFT = 60        # 左侧字符列宽
BG = (24, 28, 36)

cols = len(FONTS)
rows = len(CHARS)
W = PAD_LEFT + CELL * cols
H = PAD_TOP + CELL * rows

canvas = Image.new("RGBA", (W, H), BG + (255,))
draw = ImageDraw.Draw(canvas)

try:
    label_font = ImageFont.truetype("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 14)
    char_font = ImageFont.truetype("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 28)
except Exception:
    label_font = ImageFont.load_default()
    char_font = label_font

# 顶部字体名
for c, (fkey, label) in enumerate(FONTS):
    x = PAD_LEFT + c * CELL + CELL // 2
    draw.text((x, PAD_TOP // 2), label, fill=(220, 220, 220), font=label_font, anchor="mm")

# 左侧字符
for r, ch in enumerate(CHARS):
    y = PAD_TOP + r * CELL + CELL // 2
    draw.text((PAD_LEFT // 2, y), ch, fill=(180, 180, 180), font=char_font, anchor="mm")

# 贴图 grid
for r, ch in enumerate(CHARS):
    for c, (fkey, _) in enumerate(FONTS):
        src = PART / f"rune_char_{ord(ch):04x}_{fkey}.png"
        if not src.exists():
            continue
        img = Image.open(src).convert("RGBA")
        if SCALE_UP > 1:
            img = img.resize((img.width * SCALE_UP, img.height * SCALE_UP), Image.NEAREST)
        x = PAD_LEFT + c * CELL + (CELL - img.width) // 2
        y = PAD_TOP + r * CELL + (CELL - img.height) // 2
        canvas.alpha_composite(img, (x, y))

canvas.save(OUT)
print(f"preview → {OUT}  ({W}×{H})")
