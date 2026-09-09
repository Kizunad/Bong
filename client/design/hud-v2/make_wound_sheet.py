"""把生产网格离屏图拼成枚举对照表，包含原比例缩小和同部位放大。"""

import argparse
import csv
import shutil
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

from make_contact_sheet import FONT
from make_wound_assets import WOUNDS


def build(renders, output):
    sheet = Image.new("RGB", (1320, 1140), "#111714")
    pen = ImageDraw.Draw(sheet)

    def text(x, y, value, size=18, color="#c6cbbf"):
        font = ImageFont.truetype(FONT, size)
        bounds = pen.textbbox((x, y), value, font=font)
        if bounds[2] > sheet.width or bounds[3] > sheet.height:
            raise ValueError(f"对照表文字超出画布: {value}")
        pen.text((x, y), value, font=font, fill=color)

    def paste(picture, x, y, size):
        picture = picture.resize(size, Image.Resampling.LANCZOS)
        sheet.paste(picture, (x, y), picture)

    pictures = {
        wound: Image.open(renders / "round-2-wounds" / f"mini-body-{wound.lower()}.png").convert("RGBA")
        for wound, _, _ in WOUNDS
    }
    text(36, 30, "MiniBody / 黄色剪影与伤势", 32, "#eceee5")
    text(36, 86, "WoundLevel 全枚举 / 同一左前臂（画面右侧）", 20)
    text(36, 122, "黄色人体无内部结构线；真元与体力刻度沿用第二轮。", 17, "#8f9d91")
    text(36, 154, "生产网格离屏预览，尚未接入 Minecraft。", 17, "#8f9d91")
    old = Image.open(renders / "round-2" / "mini-body.png").convert("RGBA")
    paste(old, 999, 32, (84, 111))
    paste(pictures["INTACT"], 1171, 32, (84, 111))
    text(996, 153, "上一稿", 17)
    text(1168, 153, "本次修订", 17, "#e8c64d")

    for index, (wound, label, note) in enumerate(WOUNDS):
        x, y = 36 + (index % 3) * 424, 218 + (index // 3) * 422
        pen.rectangle((x, y, x + 399, y + 397), fill="#1d2520")
        text(x + 18, y + 16, label, 27, "#eceee5")
        text(x + 18, y + 56, wound, 15, "#93a294")
        text(x + 237, y + 23, note, 18)
        p = pictures[wound]
        paste(p, x + 17, y + 93, (168, 222))
        # 所有伤势都裁同一块区域；SEVERED 的空白远端也是判别内容。
        detail = p.crop((58 * 4, 59 * 4, 76 * 4, 89 * 4))
        paste(detail, x + 241, y + 85, (108, 180))
        text(x + 241, y + 268, "同部位放大", 15, "#93a294")
        paste(p, x + 291, y + 307, (56, 74))
        text(x + 18, y + 343, "56 × 74 px", 17, "#93a294")
        text(x + 18, y + 369, "等比例缩小检查", 14, "#93a294")

    with (renders / "asset-check.csv").open(newline="", encoding="utf-8") as source:
        checks = [row for row in csv.DictReader(source)
                  if row["asset"].startswith("round-2-wounds/")]
    expected = {f"round-2-wounds/mini-body-{wound.lower()}.svg" for wound, _, _ in WOUNDS}
    if {row["asset"] for row in checks} != expected or any(row["bounds"] != "OK" for row in checks):
        raise ValueError("六种伤势的网格检查记录不完整")
    maximum = max(int(row["triangles"]) for row in checks)
    text(36, 1075, f"6 / 6 SVG 通过生产解析及三角化 · 无越界 · 最大 {maximum} 三角形", 18)
    text(36, 1110, "出血、愈合、夹板为独立字段，本表未叠加。断肢直接缺失远端；其他 HUD 造型保持现稿。", 16, "#8f9d91")
    output.mkdir(parents=True, exist_ok=True)
    sheet.save(output / "wound-contact-sheet.png")
    shutil.copyfile(renders / "asset-check.csv", output / "asset-check.csv")
    print(output / "wound-contact-sheet.png")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("renders", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    build(args.renders, args.output)
