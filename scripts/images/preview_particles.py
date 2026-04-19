"""把 particles/ 下所有贴图合成到深灰背景,方便 preview。"""
from PIL import Image, ImageDraw, ImageFont
from pathlib import Path

PART = Path(__file__).parent / "particles"
OUT = Path(__file__).parent / "particles_preview.png"

# (pid, scale_up_to_show)
ITEMS = [
    ("sword_qi_trail", 2),
    ("sword_slash_arc", 2),
    ("breakthrough_pillar", 2),
    ("tribulation_spark", 2),
    ("flying_sword_trail", 2),
    ("lingqi_ripple", 1),
    ("qi_aura", 4),
    ("enlightenment_dust", 8),
    ("rune_char_6555", 4),
    ("rune_char_4ee4", 4),
    ("rune_char_5c01", 4),
    ("rune_char_7834", 4),
    ("rune_char_9053", 4),
]

CELL_W, CELL_H = 640, 140
BG = (32, 36, 44, 255)  # 深灰蓝背景

cols = 1
rows = len(ITEMS)
canvas = Image.new("RGBA", (CELL_W, CELL_H * rows), BG)
draw = ImageDraw.Draw(canvas)

try:
    font = ImageFont.truetype("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 16)
except Exception:
    font = ImageFont.load_default()

for i, (pid, scale) in enumerate(ITEMS):
    src = PART / f"{pid}.png"
    if not src.exists():
        continue
    img = Image.open(src).convert("RGBA")
    if scale > 1:
        img = img.resize((img.width * scale, img.height * scale), Image.NEAREST)

    y_cell = i * CELL_H
    # 粘贴到 cell 右侧居中
    px = 180
    py = y_cell + (CELL_H - img.height) // 2
    canvas.alpha_composite(img, (px, py))

    # 左侧写 id + 尺寸
    orig = Image.open(src)
    label = f"{pid}\n{orig.width}×{orig.height} (×{scale})"
    draw.multiline_text((10, y_cell + 40), label, fill=(220, 220, 220, 255), font=font)

canvas.save(OUT)
print(f"preview → {OUT}")
