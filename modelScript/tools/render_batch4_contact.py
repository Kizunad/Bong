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
    
    # 渲染每株植物的 6 视角接触图 (相机拉近，size=210)
    for pid, model_name, cname in PLANTS:
        print(f"Rendering contact row for {cname} ({pid})...")
        bbmodel_path = MODELS / f"{model_name}.bbmodel"
        
        # 1. 渲染 6 视角 (NOW 3x2 格，每个格子 210x210，细节纤毫毕现)
        sheet_img = build_sheet(bbmodel_path, size=210, shading="lambert")
        
        # 2. 读取 GUI 原画
        gui_path = REVIEWS / pid / "gui_original.png"
        gui_img = Image.open(gui_path).convert("RGBA") if gui_path.exists() else Image.new("RGBA", (128, 128), (0,0,0,0))
        
        rows.append({
            "pid": pid,
            "cname": cname,
            "gui": gui_img,
            "sheet": sheet_img,
        })

    # 标准大版式：宽度 1338px
    # 左栏：标号与名称 + 大图 GUI 原画（200x200，与渲染格同尺寸）
    # 右栏：NOW 六视角并排 (3x2 约 950px 宽)
    total_w = 1338
    row_h = 390
    header_h = 60
    total_h = header_h + row_h * len(PLANTS)
    
    canvas = Image.new("RGB", (total_w, total_h), (122, 122, 122)) # 统一中灰背景
    draw = ImageDraw.Draw(canvas)
    
    # 标题栏
    draw.rectangle([(0, 0), (total_w, header_h)], fill=(45, 45, 48))
    draw.text((30, 20), "Bong 灵植第 4 批：草本 / 蕨 (8 种) Round 1 接触表 (拉近大格)", fill=(240, 240, 240))
    draw.text((700, 20), "NOW 六视角 (3x2: FRONT, SIDE_R, BACK / SIDE_L, 3/4, TOP)", fill=(200, 200, 200))
    
    y = header_h
    for idx, r in enumerate(rows):
        bg_col = (118, 118, 118) if idx % 2 == 1 else (126, 126, 126)
        draw.rectangle([(0, y), (total_w, y + row_h)], fill=bg_col)
        
        # 标号与名称
        draw.text((30, y + 20), f"#{idx+1} {r['cname']}", fill=(255, 255, 255))
        draw.text((30, y + 45), f"{r['pid']}", fill=(220, 220, 220))
        
        # 贴大尺寸 GUI 原画 (200x200，与 3D 渲染单格等大同尺寸可读！)
        gui_thumb = r['gui'].copy()
        gui_thumb = gui_thumb.resize((200, 200), Image.Resampling.NEAREST)
        canvas.paste(gui_thumb, (30, y + 80), gui_thumb if gui_thumb.mode == 'RGBA' else None)
        draw.text((30, y + 295), "GUI 原画 (200px 等大对照)", fill=(240, 240, 240))
        
        # 贴 6 视角渲染大图
        sheet = r['sheet'].convert("RGBA")
        canvas.paste(sheet, (270, y + 10), sheet)
        
        # 行底分割线
        draw.line([(0, y + row_h - 1), (total_w, y + row_h - 1)], fill=(75, 75, 75), width=2)
        y += row_h

    OUT.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(OUT)
    print(f"Contact sheet re-rendered with 210px close-up cells: {OUT} ({total_w}x{total_h})")

if __name__ == "__main__":
    main()
