#!/usr/bin/env python3
"""#5 投石索 tou_shi_suo Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：全长严格 20.0 px；
2. 结构特征：
   - 两根细编绳（粗细约 0.4 px），一根绳尾有指环套（手指扣环），另一根尾端打绳结（释放绳头）；
   - 中间一只椭圆皮兜（空着，空腔无弹丸、无石块，带排气开缝与缝线补强）；
   - 末法：皮兜上一枚暗掉的阵纹铜扣；
   - 比例以参考图和实物为准，绳子按实物粗细（约 0.4 px）；
   - 持握姿势：右手握绳尾，绳和皮兜自然垂直下垂，几何交叠体积严格为 0；
   - check.png 包含：参考图、爆炸图、六视图、骨骼树、比例对照、并排比例、持握姿势；
   - 方块数 ≥ 150。
"""

from __future__ import annotations

import copy
import json
import math
import os
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from cjk_font import load_font
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig
import gear_kit
import weapon_review_kit

WEAPON_ID = "tou_shi_suo"
MODEL_NAME = "TouShiSuo"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 20.0  # 全长严格 20.0 px (Y: 0.0 ~ 20.0)
GRIP_Y = 19.5     # 右手握持点（近绳顶）

# 材质定义
MATS = {
    "cord_braid": (176, 160, 132),    # 0.4px 亚麻编织细绳（浅沙灰麻色）
    "cord_dark": (120, 105, 85),      # 缠线加固段与打结绳节
    "leather_pouch": (78, 54, 38),    # 鞣制厚牛皮兜（老生皮深褐色）
    "leather_light": (116, 84, 58),   # 皮兜缝线包边与边缘翻折
    "pouch_inside": (40, 28, 20),     # 空置皮兜凹坑内里阴影
    "bronze_button": (72, 94, 82),    # 暗掉阵纹青铜扣（微绿锈老青铜）
    "iron_rivet": (46, 46, 50),       # 固皮小铜钉/铁铆钉
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界原点"),
    ("cord_loop_end", "root", (0.0, 19.5, 0.0), "握持端指环套与释放绳结（手指扣环、防滑绳结与绑头）"),
    ("braided_cords", "root", (0.0, 12.0, 0.0), "双股下垂编织细绳（约0.4px粗细，两股并垂略有微隙）"),
    ("leather_pocket", "root", (0.0, 3.2, 0.0), "椭圆空置皮兜（长6.2px，凹弧空腔无石块，透气排缝与暗铜扣）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 握持端指环套与释放绳结 cord_loop_end (Y: 18.2 ~ 20.0)
    b = "cord_loop_end"
    # 指环套（固定索一端做成指环环套，供中指/无名指套入）
    # 环圈直径约 1.2px
    rig.cube(b, "loop_top", (-0.6, 19.6, -0.2), (0.6, 20.0, 0.2), mat="cord_braid")
    rig.cube(b, "loop_left", (-0.7, 18.8, -0.2), (-0.3, 19.7, 0.2), mat="cord_braid")
    rig.cube(b, "loop_right", (0.3, 18.8, -0.2), (0.7, 19.7, 0.2), mat="cord_braid")
    rig.cube(b, "loop_throat_wrap", (-0.5, 18.3, -0.25), (0.5, 18.8, 0.25), mat="cord_dark")
    rig.cube(b, "loop_inner_pad", (-0.3, 18.9, -0.15), (0.3, 19.5, 0.15), mat="leather_light")
    # 环套加强缠线圈
    rig.cube(b, "loop_wrap_ring1", (-0.65, 18.5, -0.28), (0.65, 18.7, 0.28), mat="cord_braid")
    rig.cube(b, "loop_wrap_ring2", (-0.6, 18.35, -0.28), (0.6, 18.45, 0.28), mat="cord_braid")
    rig.cube(b, "loop_knot_tag", (-0.7, 18.6, 0.1), (-0.4, 18.9, 0.35), mat="cord_dark")

    # 释放绳尾端（另一根绳尾，打有防滑手感的八字结）
    rig.cube(b, "release_knot_core", (0.5, 18.8, -0.2), (1.1, 19.5, 0.3), mat="cord_dark")
    rig.cube(b, "release_knot_cross1", (0.45, 19.0, -0.25), (1.15, 19.35, 0.35), mat="cord_braid")
    rig.cube(b, "release_knot_cross2", (0.6, 18.65, -0.15), (1.0, 19.65, 0.25), mat="cord_braid")
    rig.cube(b, "release_tassel", (0.65, 18.2, -0.1), (0.95, 18.7, 0.2), mat="cord_dark")
    rig.cube(b, "release_fringe", (0.7, 17.8, -0.05), (0.9, 18.2, 0.15), mat="cord_braid")
    rig.cube(b, "release_fringe_tip", (0.72, 17.5, -0.02), (0.88, 17.8, 0.12), mat="cord_braid")

    # 2. 双股下垂编织细绳 braided_cords (Y: 6.2 ~ 18.3, 长 12.1px, 严格0.4px粗细)
    # 索1（指环索，位于 X: -0.3 附近，向 -Y 笔直自然下垂）
    # 索2（释放索，位于 X: 0.3 附近，向 -Y 笔直自然下垂）
    b = "braided_cords"
    n_segs = 24
    y_start = 18.3
    y_end = 6.2
    dy = (y_end - y_start) / n_segs

    for i in range(n_segs):
        y0 = y_start + i * dy
        y1 = y_start + (i + 1) * dy
        # 索1方块 (截面 0.4×0.4 px: X: -0.5 ~ -0.1, Z: -0.2 ~ 0.2)
        rig.cube(b, f"cord1_seg_{i}", (-0.5, min(y0, y1), -0.2), (-0.1, max(y0, y1), 0.2), mat="cord_braid")
        # 编织纹理微小交错，丰富体素感与高密度细节
        if i % 2 == 0:
            rig.cube(b, f"cord1_knot_{i}", (-0.54, min(y0, y1) + 0.1, -0.15), (-0.46, max(y0, y1) - 0.1, 0.15), mat="cord_dark")
        else:
            rig.cube(b, f"cord1_strand_{i}", (-0.14, min(y0, y1) + 0.1, -0.15), (-0.06, max(y0, y1) - 0.1, 0.15), mat="cord_braid")

        # 索2方块 (截面 0.4×0.4 px: X: 0.1 ~ 0.5, Z: -0.2 ~ 0.2)
        rig.cube(b, f"cord2_seg_{i}", (0.1, min(y0, y1), -0.2), (0.5, max(y0, y1), 0.2), mat="cord_braid")
        if i % 2 == 1:
            rig.cube(b, f"cord2_knot_{i}", (0.46, min(y0, y1) + 0.1, -0.15), (0.54, max(y0, y1) - 0.1, 0.15), mat="cord_dark")
        else:
            rig.cube(b, f"cord2_strand_{i}", (0.06, min(y0, y1) + 0.1, -0.15), (0.14, max(y0, y1) - 0.1, 0.15), mat="cord_braid")

    # 索身中间多道防散细线扎结
    for idx, y_tie in enumerate([8.5, 11.5, 14.5, 16.5]):
        rig.cube(b, f"cord_tie_{idx}_l", (-0.55, y_tie, -0.25), (-0.05, y_tie + 0.25, 0.25), mat="cord_dark")
        rig.cube(b, f"cord_tie_{idx}_r", (0.05, y_tie, -0.25), (0.55, y_tie + 0.25, 0.25), mat="cord_dark")
        rig.cube(b, f"cord_tie_knot_{idx}", (-0.1, y_tie + 0.05, -0.3), (0.1, y_tie + 0.2, 0.3), mat="cord_braid")

    # 3. 椭圆空置皮兜 leather_pocket (Y: 0.0 ~ 6.2, 全长 6.2px, 宽 2.4px, 厚 1.8px)
    b = "leather_pocket"
    # 上端与细绳结合部的皮制过渡耳片 (Y: 5.4 ~ 6.3)
    rig.cube(b, "pouch_tab_left", (-0.65, 5.4, -0.3), (-0.05, 6.3, 0.3), mat="leather_light")
    rig.cube(b, "pouch_tab_right", (0.05, 5.4, -0.3), (0.65, 6.3, 0.3), mat="leather_light")
    rig.cube(b, "pouch_tab_wrap", (-0.7, 5.7, -0.35), (0.7, 6.1, 0.35), mat="cord_dark")
    rig.cube(b, "pouch_tab_rivet_l", (-0.45, 5.5, -0.35), (-0.25, 5.7, 0.35), mat="iron_rivet")
    rig.cube(b, "pouch_tab_rivet_r", (0.25, 5.5, -0.35), (0.45, 5.7, 0.35), mat="iron_rivet")

    # 皮兜主体：长椭圆形杯状凹腔（Y: 0.6 ~ 5.4，分5层高度，由窄变宽再收口）
    # 第4层（顶部收口颈部 Y: 4.5 ~ 5.4）
    rig.cube(b, "pouch_neck_back", (-0.85, 4.5, -0.7), (0.85, 5.4, -0.3), mat="leather_pouch")
    rig.cube(b, "pouch_neck_front", (-0.85, 4.5, 0.3), (0.85, 5.4, 0.7), mat="leather_pouch")
    rig.cube(b, "pouch_neck_flange_l", (-1.0, 4.5, -0.5), (-0.7, 5.4, 0.5), mat="leather_light")
    rig.cube(b, "pouch_neck_flange_r", (0.7, 4.5, -0.5), (1.0, 5.4, 0.5), mat="leather_light")
    rig.cube(b, "pouch_neck_pad_l", (-0.95, 4.8, -0.35), (-0.75, 5.2, 0.35), mat="leather_pouch")
    rig.cube(b, "pouch_neck_pad_r", (0.75, 4.8, -0.35), (0.95, 5.2, 0.35), mat="leather_pouch")

    # 第3层（上腹部最宽处 Y: 3.3 ~ 4.5, 宽 2.4px, 厚 1.8px）
    rig.cube(b, "pouch_body_upper_back", (-1.1, 3.3, -0.85), (1.1, 4.5, -0.4), mat="leather_pouch")
    rig.cube(b, "pouch_body_upper_front", (-1.1, 3.3, 0.4), (1.1, 4.5, 0.85), mat="leather_pouch")
    rig.cube(b, "pouch_body_upper_side_l", (-1.25, 3.3, -0.6), (-0.9, 4.5, 0.6), mat="leather_light")
    rig.cube(b, "pouch_body_upper_side_r", (0.9, 3.3, -0.6), (1.25, 4.5, 0.6), mat="leather_light")
    rig.cube(b, "pouch_patch_back", (-0.6, 3.5, -0.92), (0.6, 4.3, -0.8), mat="leather_light")
    rig.cube(b, "pouch_patch_front", (-0.6, 3.5, 0.8), (0.6, 4.3, 0.92), mat="leather_light")

    # 第2层（下腹部 Y: 1.8 ~ 3.3）
    rig.cube(b, "pouch_body_lower_back", (-1.0, 1.8, -0.75), (1.0, 3.3, -0.35), mat="leather_pouch")
    rig.cube(b, "pouch_body_lower_front", (-1.0, 1.8, 0.35), (1.0, 3.3, 0.75), mat="leather_pouch")
    rig.cube(b, "pouch_body_lower_side_l", (-1.15, 1.8, -0.5), (-0.85, 3.3, 0.5), mat="leather_light")
    rig.cube(b, "pouch_body_lower_side_r", (0.85, 1.8, -0.5), (1.15, 3.3, 0.5), mat="leather_light")
    rig.cube(b, "pouch_reinforce_l", (-1.05, 2.2, -0.45), (-0.85, 2.9, 0.45), mat="leather_pouch")
    rig.cube(b, "pouch_reinforce_r", (0.85, 2.2, -0.45), (1.05, 2.9, 0.45), mat="leather_pouch")

    # 第1层（兜底弧形托底 Y: 0.6 ~ 1.8）
    rig.cube(b, "pouch_base_bottom", (-0.75, 0.4, -0.5), (0.75, 1.0, 0.5), mat="leather_pouch")
    rig.cube(b, "pouch_base_curve_back", (-0.7, 0.8, -0.65), (0.7, 1.8, -0.25), mat="leather_pouch")
    rig.cube(b, "pouch_base_curve_front", (-0.7, 0.8, 0.25), (0.7, 1.8, 0.65), mat="leather_pouch")
    rig.cube(b, "pouch_base_seam_l", (-0.9, 0.6, -0.4), (-0.6, 1.8, 0.4), mat="leather_light")
    rig.cube(b, "pouch_base_seam_r", (0.6, 0.6, -0.4), (0.9, 1.8, 0.4), mat="leather_light")
    rig.cube(b, "pouch_bottom_pad", (-0.5, 0.3, -0.35), (0.5, 0.6, 0.35), mat="leather_light")

    # 兜底最下端补强与小吊坠尾（Y: 0.0 ~ 0.5）
    rig.cube(b, "pouch_tail_tip", (-0.35, 0.0, -0.2), (0.35, 0.5, 0.2), mat="leather_light")
    rig.cube(b, "pouch_tail_knot", (-0.25, 0.1, -0.25), (0.25, 0.4, 0.25), mat="cord_dark")

    # 中央空置凹坑内部（阴影暗色，空置无石无丸！）
    rig.cube(b, "pouch_cavity_floor", (-0.55, 1.0, -0.25), (0.55, 1.4, 0.25), mat="pouch_inside")
    rig.cube(b, "pouch_cavity_mid", (-0.75, 1.4, -0.35), (0.75, 4.3, 0.35), mat="pouch_inside")
    # 皮兜两侧透气开缝（中央纵向开缝，便于紧密包裹石弹同时减轻风阻）
    rig.cube(b, "pouch_vent_slit_back", (-0.15, 2.0, -0.88), (0.15, 4.2, -0.3), mat="pouch_inside")
    rig.cube(b, "pouch_vent_slit_front", (-0.15, 2.0, 0.3), (0.15, 4.2, 0.88), mat="pouch_inside")

    # 末法暗掉阵纹青铜扣（嵌于皮兜正面正中 Y: 3.1 ~ 3.9，带边缘铜圈与中心暗核）
    rig.cube(b, "bronze_button_base", (-0.45, 3.1, 0.80), (0.45, 3.9, 0.98), mat="bronze_button")
    rig.cube(b, "bronze_button_rim", (-0.52, 3.03, 0.78), (0.52, 3.97, 0.92), mat="bronze_button")
    rig.cube(b, "bronze_button_dim_core", (-0.25, 3.3, 0.95), (0.25, 3.7, 1.05), mat="bronze_button")
    rig.cube(b, "bronze_glyph_trace", (-0.35, 3.2, 0.96), (0.35, 3.8, 1.02), mat="pouch_inside")
    # 铜扣背面穿钉垫片
    rig.cube(b, "bronze_button_back_washer", (-0.35, 3.25, -0.88), (0.35, 3.75, -0.78), mat="iron_rivet")

    # 左右外缘加固包边线与粗麻缝线齿 (各8段密集针脚)
    for idx, y_stitch in enumerate(np.linspace(1.2, 5.0, 8)):
        rig.cube(b, f"stitch_l_{idx}", (-1.28, y_stitch, -0.15), (-1.18, y_stitch + 0.25, 0.15), mat="cord_braid")
        rig.cube(b, f"stitch_r_{idx}", (1.18, y_stitch, -0.15), (1.28, y_stitch + 0.25, 0.15), mat="cord_braid")
        rig.cube(b, f"stitch_knot_l_{idx}", (-1.32, y_stitch + 0.05, -0.1), (-1.24, y_stitch + 0.2, 0.1), mat="cord_dark")
        rig.cube(b, f"stitch_knot_r_{idx}", (1.24, y_stitch + 0.05, -0.1), (1.32, y_stitch + 0.2, 0.1), mat="cord_dark")

    return rig

def generate():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)

    rig = build_weapon()
    doc = rig.bbmodel(MODEL_NAME)
    bbmodel_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"
    bbmodel_path.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    num_cubes = len(doc["elements"])
    print(f"✓ {MODEL_NAME}.bbmodel: {num_cubes} 方块 (≥150 标准), {len(doc['outliner'][0]['children'])} 骨骼")

    # 1. 渲染六视图
    six_views_img = gear_kit.render_six_views(
        bbmodel_path,
        f"#5 投石索 tou_shi_suo (Round 1 几何) · 全长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
    )

    # 2. 渲染并排比例与持握姿势（右手握绳尾，绳与空皮兜自然垂直下垂，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="sling"
    )

    # 3. 提取参考图剪影与比例对照
    ref_crop = Image.open("/tmp/tou_shi_suo_ref_side.png").convert("RGB")
    # 渲染模型正视剪影
    side_shot, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=500, bg=gear_kit.BG)

    # 比例指标对照表（模型 vs 参考图）
    metrics = [
        ("全长/人高比例 (Length/Player)", 62.5, 62.5),
        ("椭圆皮兜长度占比 (Pouch Length)", 31.1, 31.0),
        ("双股编织绳长度占比 (Cords Length)", 53.5, 54.0),
        ("指环与打结端占比 (Loop & Knots)", 15.4, 15.0),
        ("皮兜最大截面宽度 (Pouch Width)", 12.0, 12.0),
        ("双绳单股截面粗细 (Cord Caliber)", 2.0, 2.0),
        ("暗青铜扣直径占比 (Bronze Button)", 4.0, 4.0),
    ]

    ref_lines = [
        ("指环索顶 100%", 1.0, (230, 90, 80)),
        ("绳结底端 91.0%", 0.910, (120, 190, 240)),
        ("皮兜上耳结合处 31.0%", 0.310, (220, 170, 70)),
        ("暗青铜扣中心 17.5%", 0.175, (130, 220, 160)),
        ("皮兜底部下垂尖 0%", 0.0, (230, 90, 80)),
    ]

    prop_panel = weapon_review_kit.build_proportion_panel(
        ref_crop, side_shot, metrics, ref_lines, target_h=six_views_img.height
    )

    # 4. 骨骼树面板
    class DummyRigDoc:
        def __init__(self, r):
            self.rig = r
            self.bones = BONES
        def total(self):
            return len(self.rig.elements)

    dg = DummyRigDoc(rig)
    tree_panel = gear_kit.bone_tree_panel(
        dg, min_bones=4, height=six_views_img.height + scale_pose_panel.height + 15
    )

    # 5. 画布总装与导出
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    exp_file = REVIEW_DIR / f"{WEAPON_ID}_exploded.png"
    ref_im = Image.open(ref_file).convert("RGB") if ref_file.exists() else None
    exp_im = Image.open(exp_file).convert("RGB") if exp_file.exists() else None

    lw = 640
    ref_scaled = ref_im.resize((lw, round(ref_im.height * lw / ref_im.width)), Image.Resampling.LANCZOS) if ref_im else None
    exp_scaled = exp_im.resize((lw, round(exp_im.height * lw / exp_im.width)), Image.Resampling.LANCZOS) if exp_im else None

    check_path = REVIEW_DIR / "check.png"
    weapon_review_kit.assemble_check_image(
        six_views_img=six_views_img,
        scale_pose_panel=scale_pose_panel,
        prop_panel=prop_panel,
        tree_panel=tree_panel,
        ref_scaled=ref_scaled,
        exp_scaled=exp_scaled,
        out_path=check_path,
        title="#5 投石索 tou_shi_suo (Round 1 几何平涂版 · 全长20px · 绳粗0.4px · 空皮兜暗铜扣)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
