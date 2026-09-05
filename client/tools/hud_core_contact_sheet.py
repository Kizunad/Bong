#!/usr/bin/env python3
"""检查真实 HUD 截图的动态差异，拼接人工审阅图；不生成或修饰 HUD 本体。"""

import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def require(condition, message):
    if not condition:
        raise ValueError(message)


def anchor_y(width, height):
    # 这是截图 fixture 的布局预期，Java 几何契约另有单测。
    return height - (152 if (width - 196) // 2 - 28 < 82 else 86)


def verify_group(images):
    full, low, recovered = images
    width, height = full.size
    require(all(im.size == full.size for im in images), "三态必须使用同一视口")
    top = anchor_y(width, height)
    for im in images:
        require(im.getpixel((width // 2, height // 2)) == (23, 33, 41), "加载遮罩或错误画布")
        head = im.getpixel((24, top + 7))
        require(min(head) > 70, "人体 SVG 缺失")
    for x, is_fill in (
        (47, lambda c: c[2] > c[1] > c[0] + 40),
        (57, lambda c: c[0] > c[2] and c[1] > c[2] + 30),
    ):
        heights = [sum(is_fill(im.getpixel((x, y))) for y in range(top + 9, top + 74)) for im in images]
        require(0 < heights[1] < heights[2] < heights[0], f"动态条未反映低值/恢复/满值: {heights}")
    wound = [im.getpixel((24, top + 20)) for im in images]
    require(wound[1][0] > 200 and wound[1][1] < 100, "受伤帧缺少胸部伤势标记")
    require(wound[0] == wound[2] != wound[1], "恢复后伤势标记残留")
    left = (width - 196) // 2
    lower = height - 42
    for index, im in enumerate(images):
        require(min(im.getpixel((left + index * 22 + 10, lower))) > 230, "选中边框未跟随当前槽位")
    # 同一图标、同一像素：低值态仅下半部被冷却遮罩压暗，恢复时重新露出图标。
    upper_pixel = (left + 22 + 10, lower + 5)
    lower_pixel = (left + 22 + 10, lower + 14)
    require(full.getpixel(upper_pixel) == low.getpixel(upper_pixel), "冷却遮罩错误覆盖整个图标")
    require(full.getpixel(lower_pixel) == recovered.getpixel(lower_pixel) != low.getpixel(lower_pixel),
            "冷却遮罩没有覆盖图标，或到期后仍然残留")
    return "bars / wounds / selection / cooldown: PASS"


def self_test(images):
    # 从真实图注入失败，证明 gate 可以识别空绘制、冻结帧和状态残留。
    full, low, recovered = images
    cases = {
        "blank-frame": [Image.new("RGB", full.size, (23, 33, 41)), low, recovered],
        "frozen-state": [full, full, full],
        "stale-recovery": [full, low, low],
    }
    for name, altered in cases.items():
        try:
            verify_group(altered)
        except ValueError:
            continue
        raise ValueError(f"门禁无法识别注入缺陷: {name}")
    return "injected blank / frozen / stale: REJECTED"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--shots", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--previous", type=Path, help="可选的上一轮截图目录，仅用于同取景比较")
    args = parser.parse_args()
    shots = json.loads(args.config.read_text())["screenshots"]
    require("status=passed" in (args.shots / "ui-preview-result.txt").read_text(), "截图会话尚未成功")
    groups = []
    for offset in range(0, len(shots), 3):
        batch = shots[offset:offset + 3]
        require(len(batch) == 3, "每组必须有满值、低值、恢复三态")
        images = []
        for shot in batch:
            with Image.open(args.shots / f"ui-{shot['name']}.png") as image:
                require(image.size == (shot["framebuffer_width"], shot["framebuffer_height"]), "framebuffer 尺寸错误")
                images.append(image.convert("RGB").resize(
                    (shot["expected_logical_width"], shot["expected_logical_height"]), Image.Resampling.NEAREST))
        result = verify_group(images)
        injected = self_test(images)
        print(f"{batch[0]['name']}: {result}; {injected}")
        groups.append((batch, images))

    # 三态同取景：只裁去无内容的上半画布，不缩放 HUD，统一放大两倍供人工辨认。
    font = ImageFont.truetype("DejaVuSans.ttf", 16)
    width = max(im.width for _, images in groups for im in images) * 2
    cell_height = 704 if args.previous else 352
    sheet = Image.new("RGB", (width * 3, cell_height * len(groups) + 64), (15, 20, 25))
    draw = ImageDraw.Draw(sheet)
    draw.text((12, 8), "Minecraft GUI screenshots | FULL / LOW + WOUNDED / RECOVERED", font=font, fill="white")
    draw.text((12, 32), "Bottom 160 logical px, 2x nearest | pixel gates PASS; injected defects rejected | human review pending", font=font, fill="#A9B6C0")
    for row, (batch, images) in enumerate(groups):
        for col, (shot, im) in enumerate(zip(batch, images)):
            x, y = col * width, row * cell_height + 64
            draw.text((x + 8, y + 4), f"{shot['name']} | GUI {im.width}x{im.height} | scale {shot['gui_scale']}",
                      font=font, fill="white")
            crop = im.crop((0, im.height - 160, im.width, im.height))
            sheet.paste(crop.resize((im.width * 2, 320), Image.Resampling.NEAREST), (x, y + 30))
            if args.previous:
                with Image.open(args.previous / f"ui-{shot['name']}.png") as source:
                    previous = source.convert("RGB").resize(im.size, Image.Resampling.NEAREST)
                prior_crop = previous.crop((0, im.height - 160, im.width, im.height))
                difference = sum(a != b for a, b in zip(crop.get_flattened_data(), prior_crop.get_flattened_data()))
                draw.text((x + 8, y + 352), f"Previous round | changed pixels in same crop: {difference}", font=font, fill="#A9B6C0")
                sheet.paste(prior_crop.resize((im.width * 2, 320), Image.Resampling.NEAREST), (x, y + 376))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(args.output)
    print(args.output)


if __name__ == "__main__":
    main()
