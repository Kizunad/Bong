"""Compose honest runtime screenshots and shipped status PNGs for visual review."""

from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont, ImageStat

ROOT = Path(__file__).resolve().parent
ASSETS = ROOT.parents[2] / "src/main/resources/assets/bong-client/textures/hud/effects"
FONT = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 18)
SMALL = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 14)
STAGES = ("entry", "travel", "settled", "warning", "exit")


def shot(directory, stage):
    return Image.open(ROOT / directory / f"preview-hud-status-effects-{stage}.png").convert("RGB")


def main():
    sheet = Image.new("RGB", (1440, 1210), "#101614")
    draw = ImageDraw.Draw(sheet)
    draw.text((24, 16), "STATUS EFFECTS / ROUND 3 / MINECRAFT FRAMEBUFFER", font=FONT, fill="#e4e8dc")
    draw.text((24, 47), "Top-center crops at equal logical scale. The wide view uses GUI scale 3; compact uses scale 2.", font=SMALL, fill="#acb9b0")
    for column, (directory, title) in enumerate((("round-2-wide", "Round 2 / 1280 x 720"),
                                               ("round-3-wide", "Round 3 / 1280 x 720"),
                                               ("round-3-compact", "Round 3 / 640 x 480"))):
        x = 24 + column * 472
        draw.text((x, 79), title, font=SMALL, fill="#b6c994")
        for row, stage in enumerate(STAGES):
            source = shot(directory, stage)
            scale = 3 if directory.endswith("wide") else 2
            crop = source.crop((source.width // 2 - 155 * scale, 0,
                                source.width // 2 + 155 * scale, 145 * scale))
            crop.thumbnail((448, 178), Image.Resampling.LANCZOS)
            y = 111 + row * 199
            draw.text((x, y), stage, font=SMALL, fill="#d5dbd1")
            sheet.paste(crop, (x, y + 20))
            assert max(ImageStat.Stat(crop).stddev) > 3, f"Blank framebuffer: {directory}/{stage}"
    draw.text((24, 1128), "Loaded SVG meshes: socket 44 / rim 21 / blood 22 / taint 36 triangles.", font=SMALL, fill="#c3d6b3")
    draw.text((24, 1155), "Snapshots use the production planner and renderer with explicit local fixtures, not server-applied effects.", font=SMALL, fill="#a6b5ad")
    sheet.save(ROOT / "final-contact-sheet.png")

    board = Image.new("RGB", (1280, 520), "#171e1b")
    draw = ImageDraw.Draw(board)
    draw.text((24, 20), "STATUS PNG / GIMAGE2 / NEW ICONS", font=FONT, fill="#e4e8dc")
    for i, name in enumerate(("immobilized", "shieldblocking", "health_regen_boost", "exhausted")):
        x = 24 + i * 315
        draw.text((x, 62), name, font=SMALL, fill="#b6c994")
        path = ASSETS / f"{name}.png"
        if not path.exists():
            draw.text((x, 150), "PENDING / provider 502", font=SMALL, fill="#e9a68d")
            continue
        source = Image.open(path).convert("RGBA")
        for tile, bg in enumerate(("#171e1b", "#cad1c6")):
            target = Image.new("RGBA", (256, 170), bg)
            icon = source.resize((154, 154), Image.Resampling.LANCZOS)
            target.alpha_composite(icon, (51, 8))
            board.paste(target.convert("RGB"), (x, 94 + tile * 180))
        icon = source.resize((24, 24), Image.Resampling.LANCZOS)
        board.paste(icon, (x + 116, 460), icon)
    board.save(ROOT / "png-contact-sheet.png")
    # A static fixture must produce different frames along the animation path.
    a, b = shot("round-3-wide", "entry"), shot("round-3-wide", "settled")
    diff = ImageChops.difference(a.crop((400, 0, 880, 400)), b.crop((400, 0, 880, 400)))
    assert diff.getbbox() is not None, "Entry and settled frames must differ"
    print(ROOT / "final-contact-sheet.png")
    print(ROOT / "png-contact-sheet.png")


if __name__ == "__main__":
    main()
