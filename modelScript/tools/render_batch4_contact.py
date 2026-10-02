import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

from bbmodel_maker.workbench.contact_sheet import build_sheet

ROOT = Path(__file__).resolve().parents[2]
MODELS = ROOT / "modelScript" / "models"
REVIEWS = ROOT / "model-review" / "img" / "plants"
OUT = ROOT / "model-review" / "img" / "plants" / "batch4_contact.png"

PLANTS = [
    ("spirit_grass", "SpiritGrass", "灵草"),
    ("ning_mai_cao", "NingMaiCao", "凝脉草"),
    ("ci_she_hao", "CiSheHao", "刺舌蒿"),
    ("qing_zhuo_cao", "QingZhuoCao", "清浊草"),
    ("xue_se_mai_cao", "XueSeMaiCao", "血色脉草"),
    ("fu_chen_cao", "FuChenCao", "浮尘草"),
    ("fu_yuan_jue", "FuYuanJue", "负元蕨"),
    ("zhen_jie_zi", "ZhenJieZi", "针芥子"),
]

def main():
    rows = []
    
    # 渲染每株植物的 6 视角接触图
    for pid, model_name, cname in PLANTS:
        print(f"Rendering contact row for {cname} ({pid})...")
        bbmodel_path = MODELS / f"{model_name}.bbmodel"
        
        # 1. 渲染 6 视角 (NOW 3x2 格)
        sheet_img = build_sheet(bbmodel_path, size=150, shading="lambert")
        
        # 2. 读取 GUI 原画与 CPA 参考图
        gui_path = REVIEWS / pid / "gui_original.png"
        ref_path = REVIEWS / pid / "three_view.png"
        
        gui_img = Image.open(gui_path).convert("RGBA") if gui_path.exists() else Image.new("RGBA", (128, 128), (0,0,0,0))
        ref_img = Image.open(ref_path).convert("RGBA") if ref_path.exists() else Image.new("RGBA", (128, 128), (0,0,0,0))
        
        rows.append({
            "pid": pid,
            "cname": cname,
            "gui": gui_img,
            "ref": ref_img,
            "sheet": sheet_img,
        })

    total_w = 1338
    row_h = 340
    header_h = 60
    total_h = header_h + row_h * len(PLANTS)
    
    canvas = Image.new("RGB", (total_w, total_h), (122, 122, 122)) # 中灰背景
    draw = ImageDraw.Draw(canvas)
    
    # 标题栏
    draw.rectangle([(0, 0), (total_w, header_h)], fill=(45, 45, 48))
    draw.text((30, 20), "Bong 灵植第 4 批：草本 / 蕨 (8 种) Round 1 接触表", fill=(240, 240, 240))
    draw.text((750, 20), "NOW 六视角 (3x2: FRONT, SIDE_R, BACK / SIDE_L, 3/4, TOP)", fill=(200, 200, 200))
    
    y = header_h
    for idx, r in enumerate(rows):
        bg_col = (118, 118, 118) if idx % 2 == 1 else (126, 126, 126)
        draw.rectangle([(0, y), (total_w, y + row_h)], fill=bg_col)
        
        # 标号与名称
        draw.text((25, y + 20), f"#{idx+1} {r['cname']}", fill=(255, 255, 255))
        draw.text((25, y + 45), f"{r['pid']}", fill=(210, 210, 210))
        
        # 贴 GUI 原画 (缩放至 110x110)
        gui_thumb = r['gui'].copy()
        gui_thumb.thumbnail((110, 110), Image.Resampling.LANCZOS)
        canvas.paste(gui_thumb, (25, y + 80), gui_thumb if gui_thumb.mode == 'RGBA' else None)
        draw.text((25, y + 200), "GUI 原画", fill=(230, 230, 230))
        
        # 贴 CPA 参考 (缩放至 180x180)
        ref_thumb = r['ref'].copy()
        ref_thumb.thumbnail((180, 180), Image.Resampling.LANCZOS)
        canvas.paste(ref_thumb, (155, y + 60), ref_thumb if ref_thumb.mode == 'RGBA' else None)
        draw.text((155, y + 250), "CPA 三视图", fill=(230, 230, 230))
        
        # 贴 6 视角渲染图
        sheet = r['sheet'].convert("RGBA")
        canvas.paste(sheet, (370, y + 15), sheet)
        
        # 画分割线
        draw.line([(0, y + row_h - 1), (total_w, y + row_h - 1)], fill=(80, 80, 80), width=1)
        y += row_h

    OUT.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(OUT)
    print(f"All done! Saved contact sheet to: {OUT} ({total_w}x{total_h})")

if __name__ == "__main__":
    main()
