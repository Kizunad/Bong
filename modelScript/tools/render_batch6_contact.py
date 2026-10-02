"""生成灵植第 6 批 Round 1 评审接触表。

版式严格复用灵植第一至五批的标准规范：
- 尺寸：总宽 1338px，每株占一行（总高 5 * 430 = 2150px）；
- 统一背景：中灰 (122, 122, 122)；
- 左栏 (0..250px)：植物名称与代号、GUI 原画 (居中等比缩放)、CPA 三视图参考；
- 中栏 (250..790px)：NOW 当前作者稿的六视角 (3x2: FRONT, SIDE_R, BACK / SIDE_L, 3/4, TOP)；
- 右栏 (790..1330px)：PREV first-cut 基线对照的六视角 (并排同取景对比)；
- 视角图块：每个视角 170x170px，底层 300px 渲染，采用正面补光，标签白字。
"""

from __future__ import annotations

from pathlib import Path
from PIL import Image, ImageDraw

import bbmodel_maker.render.framing as framing

ROOT = Path(__file__).resolve().parents[2]
MODELS = ROOT / "modelScript" / "models"
PREV_DIR = ROOT / "modelScript" / "out" / "prev_batch6"
REVIEWS = ROOT / "model-review" / "img" / "plants"
OUT = ROOT / "model-review" / "img" / "plants" / "batch6_contact.png"

PLANTS = [
    ("an_shen_guo", "AnShenGuo", "安神果"),
    ("wu_yan_guo", "WuYanGuo", "无言果"),
    ("ling_guo", "LingGuo", "灵果"),
    ("tian_nu_jiao", "TianNuJiao", "天怒椒"),
    ("shi_mai_gen", "ShiMaiGen", "噬脉根"),
]

VIEW_ORDER = [
    ("FRONT (+z)", "FRONT"),
    ("SIDE_R (+x)", "SIDE_R"),
    ("BACK (-z)", "BACK"),
    ("SIDE_L (-x)", "SIDE_L"),
    ("3/4", "3/4"),
    ("TOP (+y)", "TOP"),
]


def render_six_views(model_path: Path, focus: object, size: int = 300) -> dict[str, Image.Image]:
    views = framing.views_for("+z")
    rendered = framing.render_views(model_path, views, focus=focus, size=size, shading="lambert")
    return {v.name: im for v, im in rendered}


def build_tile_grid(views_dict: dict[str, Image.Image], tile_size: int = 170) -> Image.Image:
    grid_w = 540
    grid_h = 390
    grid = Image.new("RGB", (grid_w, grid_h), (122, 122, 122))
    draw = ImageDraw.Draw(grid)

    col_w = 170
    col_gap = 12
    row_h = 170
    label_h = 16
    gap_y = 10

    for i, (label_text, view_key) in enumerate(VIEW_ORDER):
        c = i % 3
        r = i // 3
        x = c * (col_w + col_gap)
        y = r * (row_h + label_h + gap_y)

        draw.text((x + 2, y), label_text, fill=(240, 240, 240))
        im = views_dict[view_key].resize((col_w, row_h), Image.Resampling.LANCZOS)
        grid.paste(im, (x, y + label_h + 2))

    return grid


def main():
    total_w = 1338
    row_h = 430
    total_h = row_h * len(PLANTS)
    canvas = Image.new("RGB", (total_w, total_h), (122, 122, 122))
    draw = ImageDraw.Draw(canvas)

    for idx, (pid, model_name, cname) in enumerate(PLANTS):
        y_offset = idx * row_h
        print(f"[{idx+1}/{len(PLANTS)}] Processing {cname} ({pid})...")

        now_model_path = MODELS / f"{model_name}.bbmodel"
        prev_model_path = PREV_DIR / f"{model_name}.bbmodel"

        views = framing.views_for("+z")
        focus = framing.focus_for(now_model_path, views, margin=1.02)

        # 1. 渲染当前作者稿 (NOW)
        now_views = render_six_views(now_model_path, focus, size=300)
        now_grid = build_tile_grid(now_views, tile_size=170)

        # 2. 渲染首轮基线 (PREV)
        if prev_model_path.exists():
            prev_views = render_six_views(prev_model_path, focus, size=300)
            prev_grid = build_tile_grid(prev_views, tile_size=170)
        else:
            prev_grid = Image.new("RGB", (540, 390), (122, 122, 122))

        # 3. 左栏参考图与文字
        draw.text((15, y_offset + 15), f"#{idx+1} {cname}", fill=(255, 255, 255))
        draw.text((15, y_offset + 35), f"{pid}", fill=(210, 210, 210))

        # GUI 原画
        gui_path = REVIEWS / pid / "gui_original.png"
        if gui_path.exists():
            gui_im = Image.open(gui_path).convert("RGBA")
            gui_thumb = gui_im.resize((100, 100), Image.Resampling.NEAREST)
            canvas.paste(gui_thumb, (15, y_offset + 70), gui_thumb)
            draw.text((15, y_offset + 175), "GUI 原画", fill=(230, 230, 230))

        # CPA 三视图参考
        ref_path = REVIEWS / pid / "three_view.png"
        if ref_path.exists():
            ref_im = Image.open(ref_path).convert("RGBA")
            ref_thumb = ref_im.copy()
            ref_thumb.thumbnail((110, 160), Image.Resampling.LANCZOS)
            canvas.paste(ref_thumb, (125, y_offset + 70), ref_thumb if ref_thumb.mode == "RGBA" else None)
            draw.text((125, y_offset + 235), "CPA 三视图", fill=(230, 230, 230))

        # 栏目标题
        draw.text((250, y_offset + 15), "NOW (当前作者稿: 丰满果根/原画配色)", fill=(255, 240, 160))
        draw.text((790, y_offset + 15), "PREV (Round 1 first-cut 基线对照)", fill=(210, 210, 210))

        canvas.paste(now_grid, (250, y_offset + 35))
        canvas.paste(prev_grid, (790, y_offset + 35))
        draw.line([(0, y_offset + row_h - 1), (total_w, y_offset + row_h - 1)], fill=(75, 75, 75), width=2)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(OUT)
    print(f"All done! Rendered batch 6 contact sheet to: {OUT} ({total_w}x{total_h})")


if __name__ == "__main__":
    main()
