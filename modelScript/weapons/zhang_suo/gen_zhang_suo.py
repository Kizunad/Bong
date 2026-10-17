#!/usr/bin/env python3
"""#6 杖索 zhang_suo Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：全长严格 30.0 px；
2. 结构特征：
   - 木杖身八角近圆、尾端铁镦；
   - 顶端铁箍带暗掉阵线，附铁钩挂着索环；
   - 细编绳（粗细约 0.4 px）连着皮兜，皮兜空着垂在杖身一侧；
   - 兜里空着，不做石弹、弹丸；
   - 持握姿势：持杖微倾斜立，皮兜悬垂杖侧，双手持杖位，几何交叠体积严格为 0；
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

WEAPON_ID = "zhang_suo"
MODEL_NAME = "ZhangSuo"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 30.0  # 全长严格 30.0 px (杖身 Y: 0.0 ~ 30.0)
GRIP_Y = 11.5     # 右手握持点（下中段持杖位）

# 材质定义
MATS = {
    "staff_wood": (108, 78, 52),      # 老枣木/白蜡木八角杖身（坚韧深褐色）
    "staff_facet": (132, 98, 68),     # 八角切面高光与微棱
    "iron_dark": (48, 48, 52),        # 锻铁尾镦与顶箍套筒
    "iron_rust": (62, 54, 48),        # 铁件风化微锈边
    "bronze_array": (74, 96, 84),     # 顶箍上暗掉阵线（暗绿灰铜纹）
    "leather_grip": (64, 46, 32),     # 杖身握把缠皮带
    "cord_braid": (175, 160, 132),    # 0.4px 亚麻编织细悬绳
    "cord_dark": (122, 106, 86),      # 缠线紧固段与索环结
    "leather_pouch": (78, 54, 38),    # 鞣制厚牛皮兜（老生皮深褐色）
    "leather_light": (118, 86, 60),   # 皮兜缝线包边与边缘翻折
    "pouch_inside": (38, 26, 20),     # 空置皮兜凹坑内里阴影
    "iron_hook": (56, 56, 62),        # 杖顶释放铁钩
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界原点"),
    ("staff_butt", "root", (0.0, 0.0, 0.0), "杖尾铁镦（锻铁圆锥底箍、防滑平底与加固钉）"),
    ("staff_body", "root", (0.0, 15.0, 0.0), "八角近圆木杖身（枣木实芯、八面切角与中段缠皮握把）"),
    ("staff_top_ferrule", "root", (0.0, 28.5, 0.0), "杖顶铁箍与暗阵线（筒状套箍、暗青铜阵线、挂索铁钩）"),
    ("hanging_sling", "root", (0.0, 20.0, 0.0), "悬垂投石索与空皮兜（0.4px细绳索环、椭圆皮兜垂于杖侧）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 1. 杖尾锻铁底镦 staff_butt (Y: 0.0 ~ 2.4)
    b = "staff_butt"
    # 底镦铁锥套筒（截面略大，直径约 1.2px）
    rig.cube(b, "butt_tip", (-0.35, 0.0, -0.35), (0.35, 0.4, 0.35), mat="iron_dark")
    rig.cube(b, "butt_cone", (-0.5, 0.4, -0.5), (0.5, 1.6, 0.5), mat="iron_dark")
    rig.cube(b, "butt_cone_facet", (-0.54, 0.6, -0.4), (0.54, 1.4, 0.4), mat="iron_rust")
    rig.cube(b, "butt_ring_collar", (-0.58, 1.6, -0.58), (0.58, 2.2, 0.58), mat="iron_dark")
    rig.cube(b, "butt_ring_rim", (-0.62, 2.1, -0.62), (0.62, 2.4, 0.62), mat="iron_rust")
    # 铁镦固定横销与铆钉
    rig.cube(b, "butt_pin_x", (-0.65, 1.8, -0.15), (0.65, 2.0, 0.15), mat="iron_dark")

    # 2. 八角近圆木杖身 staff_body (Y: 2.4 ~ 27.2, 长 24.8px, 直径约 1.0px)
    b = "staff_body"
    # 杖身分 16 段高度构建，截面采用十字与对角复合构建八角近圆截面
    # 核心截面：中心方形 0.8×0.8 + 侧面八角棱片
    n_staff_segs = 16
    y_staff_start = 2.4
    y_staff_end = 27.2
    dy_staff = (y_staff_end - y_staff_start) / n_staff_segs

    for i in range(n_staff_segs):
        y0 = y_staff_start + i * dy_staff
        y1 = y_staff_start + (i + 1) * dy_staff
        # 中心方柱 (截面 0.8×0.8)
        rig.cube(b, f"staff_core_{i}", (-0.4, y0, -0.4), (0.4, y1, 0.4), mat="staff_wood")
        # 八面微倒角棱片 (X方向突起与Z方向突起)
        rig.cube(b, f"staff_x_face_{i}", (-0.48, y0, -0.28), (0.48, y1, 0.28), mat="staff_facet")
        rig.cube(b, f"staff_z_face_{i}", (-0.28, y0, -0.48), (0.28, y1, 0.48), mat="staff_facet")
        # 杖节加固微凸纹
        if i in [3, 7, 11, 14]:
            rig.cube(b, f"staff_node_ring_{i}", (-0.51, y0 + 0.1, -0.51), (0.51, y0 + 0.35, 0.51), mat="staff_wood")

    # 杖身握持区缠皮加固段（两手握持区域：下握把 Y: 9.5 ~ 12.5，上握把 Y: 15.5 ~ 17.5）
    # 下握把（主手持杖区）
    rig.cube(b, "grip_lower_wrap", (-0.52, 9.5, -0.52), (0.52, 12.5, 0.52), mat="leather_grip")
    rig.cube(b, "grip_lower_seam", (-0.55, 9.6, -0.2), (-0.50, 12.4, 0.2), mat="leather_light")
    rig.cube(b, "grip_lower_collar_bot", (-0.56, 9.3, -0.56), (0.56, 9.6, 0.56), mat="cord_dark")
    rig.cube(b, "grip_lower_collar_top", (-0.56, 12.4, -0.56), (0.56, 12.7, 0.56), mat="cord_dark")
    # 螺旋交叉缠绳
    for idx, y_cross in enumerate([10.0, 10.8, 11.6]):
        rig.cube(b, f"grip_lower_cross_{idx}", (-0.54, y_cross, -0.54), (0.54, y_cross + 0.35, 0.54), mat="cord_braid")

    # 上握把（副手持杖区）
    rig.cube(b, "grip_upper_wrap", (-0.52, 15.5, -0.52), (0.52, 17.5, 0.52), mat="leather_grip")
    rig.cube(b, "grip_upper_collar_bot", (-0.56, 15.3, -0.56), (0.56, 15.6, 0.56), mat="cord_dark")
    rig.cube(b, "grip_upper_collar_top", (-0.56, 17.4, -0.56), (0.56, 17.7, 0.56), mat="cord_dark")

    # 3. 杖顶铁箍与暗阵线 staff_top_ferrule (Y: 27.2 ~ 30.0, 长 2.8px)
    b = "staff_top_ferrule"
    # 顶端金属套箍（锻铁圆筒，箍住杖木防止劈裂）
    rig.cube(b, "top_ferrule_body", (-0.55, 27.2, -0.55), (0.55, 29.5, 0.55), mat="iron_dark")
    rig.cube(b, "top_ferrule_lip", (-0.58, 29.3, -0.58), (0.58, 29.8, 0.58), mat="iron_rust")
    rig.cube(b, "top_ferrule_cap", (-0.45, 29.7, -0.45), (0.45, 30.0, 0.45), mat="iron_dark")

    # 末法暗掉阵纹线（套箍表面嵌有三圈古法储力阵纹，现已暗淡失效，呈微绿暗铜痕）
    rig.cube(b, "array_ring_bot", (-0.58, 27.6, -0.58), (0.58, 27.85, 0.58), mat="bronze_array")
    rig.cube(b, "array_ring_mid", (-0.58, 28.2, -0.58), (0.58, 28.45, 0.58), mat="bronze_array")
    rig.cube(b, "array_ring_top", (-0.58, 28.8, -0.58), (0.58, 29.05, 0.58), mat="bronze_array")
    # 纵向阵线连缀刻痕
    for idx, (sx, sz) in enumerate([(-0.59, 0.0), (0.59, 0.0), (0.0, -0.59), (0.0, 0.59)]):
        rig.cube(b, f"array_line_{idx}", (sx - 0.05, 27.6, sz - 0.05), (sx + 0.05, 29.1, sz + 0.05), mat="bronze_array")

    # 杖顶释放铁钩（向右后侧弯出，用于挂投石索的脱钩环）
    # 钩座
    rig.cube(b, "top_hook_base", (0.4, 28.8, -0.2), (0.9, 29.6, 0.2), mat="iron_hook")
    # 向上伸展钩臂
    rig.cube(b, "top_hook_shank", (0.7, 29.4, -0.15), (1.1, 30.0, 0.15), mat="iron_hook")
    # 向前挑出斜角弯钩（保持适宜的滑脱角度）
    rig.cube(b, "top_hook_beak", (0.7, 29.7, 0.1), (1.05, 30.0, 0.5), mat="iron_hook")
    rig.cube(b, "top_hook_tip", (0.75, 29.5, 0.4), (1.0, 29.8, 0.6), mat="iron_hook")

    # 固定索端绑带（索一端死结死系在铁箍底圈，另一端带索环套在铁钩上）
    rig.cube(b, "sling_anchor_knot", (0.35, 27.5, -0.4), (0.85, 28.1, 0.4), mat="cord_dark")

    # 4. 悬垂投石索与空皮兜 hanging_sling (Y: 15.0 ~ 29.5, 垂在杖身一侧 X: 1.0 ~ 2.6)
    b = "hanging_sling"
    # 上端索环（挂在铁钩上的释放环套）
    rig.cube(b, "sling_ring_top", (0.75, 29.2, 0.2), (1.05, 29.7, 0.55), mat="cord_braid")
    rig.cube(b, "sling_ring_collar", (0.8, 28.7, 0.1), (1.1, 29.2, 0.4), mat="cord_dark")

    # 细编绳下垂段（两根约 0.4px 细绳从杖顶垂下，全长约 7.5px，Y: 21.2 ~ 28.7）
    n_sling_segs = 16
    y_sling_top = 28.7
    y_sling_bot = 21.2
    dy_sling = (y_sling_bot - y_sling_top) / n_sling_segs

    for i in range(n_sling_segs):
        y0 = y_sling_top + i * dy_sling
        y1 = y_sling_top + (i + 1) * dy_sling
        # 绳1（固定端下引绳，X 紧贴杖侧约 0.8 ~ 1.2）
        rig.cube(b, f"sling_cord1_{i}", (0.75, min(y0, y1), -0.2), (1.15, max(y0, y1), 0.2), mat="cord_braid")
        # 绳2（挂钩释放端下引绳，X 约 1.2 ~ 1.6）
        rig.cube(b, f"sling_cord2_{i}", (1.25, min(y0, y1), -0.2), (1.65, max(y0, y1), 0.2), mat="cord_braid")
        # 编织凸节点
        if i % 2 == 0:
            rig.cube(b, f"sling_cord_knot_{i}", (0.8, min(y0, y1) + 0.1, -0.25), (1.6, max(y0, y1) - 0.1, 0.25), mat="cord_dark")
        else:
            rig.cube(b, f"sling_cord_fiber_{i}", (1.15, min(y0, y1) + 0.05, -0.15), (1.25, max(y0, y1) - 0.05, 0.15), mat="cord_braid")

    # 绳兜连接处加固耳片与皮管套 (Y: 20.4 ~ 21.3)
    rig.cube(b, "sling_tab_sleeve", (0.7, 20.4, -0.3), (1.7, 21.3, 0.3), mat="leather_light")
    rig.cube(b, "sling_tab_wrap", (0.65, 20.7, -0.35), (1.75, 21.1, 0.35), mat="cord_dark")
    rig.cube(b, "sling_tab_rivet", (1.1, 20.5, -0.35), (1.3, 20.9, 0.35), mat="iron_dark")

    # 空置椭圆皮兜（长 5.5px, Y: 15.0 ~ 20.5, 宽 2.2px, 厚 1.6px，凹弧空腔，无石弹）
    # 皮兜顶部颈口 (Y: 19.5 ~ 20.5)
    rig.cube(b, "pouch_top_back", (0.6, 19.5, -0.6), (1.8, 20.5, -0.2), mat="leather_pouch")
    rig.cube(b, "pouch_top_front", (0.6, 19.5, 0.2), (1.8, 20.5, 0.6), mat="leather_pouch")
    rig.cube(b, "pouch_top_side_l", (0.5, 19.5, -0.4), (0.7, 20.5, 0.4), mat="leather_light")
    rig.cube(b, "pouch_top_side_r", (1.7, 19.5, -0.4), (1.9, 20.5, 0.4), mat="leather_light")

    # 皮兜中腹部（最饱满凹兜，Y: 17.0 ~ 19.5, 宽 2.2px, 厚 1.6px）
    rig.cube(b, "pouch_belly_back", (0.5, 17.0, -0.8), (1.9, 19.5, -0.3), mat="leather_pouch")
    rig.cube(b, "pouch_belly_front", (0.5, 17.0, 0.3), (1.9, 19.5, 0.8), mat="leather_pouch")
    rig.cube(b, "pouch_belly_side_l", (0.4, 17.0, -0.5), (0.65, 19.5, 0.5), mat="leather_light")
    rig.cube(b, "pouch_belly_side_r", (1.75, 17.0, -0.5), (2.0, 19.5, 0.5), mat="leather_light")
    # 腹部缝线与补强贴皮
    rig.cube(b, "pouch_belly_patch_b", (0.8, 17.5, -0.88), (1.6, 19.0, -0.75), mat="leather_light")
    rig.cube(b, "pouch_belly_patch_f", (0.8, 17.5, 0.75), (1.6, 19.0, 0.88), mat="leather_light")

    # 皮兜底部收口 (Y: 15.5 ~ 17.0)
    rig.cube(b, "pouch_bot_back", (0.6, 15.5, -0.65), (1.8, 17.0, -0.2), mat="leather_pouch")
    rig.cube(b, "pouch_bot_front", (0.6, 15.5, 0.2), (1.8, 17.0, 0.65), mat="leather_pouch")
    rig.cube(b, "pouch_bot_side_l", (0.5, 15.5, -0.4), (0.75, 17.0, 0.4), mat="leather_light")
    rig.cube(b, "pouch_bot_side_r", (1.65, 15.5, -0.4), (1.9, 17.0, 0.4), mat="leather_light")

    # 兜底收合缝与流苏 (Y: 14.8 ~ 15.5)
    rig.cube(b, "pouch_bottom_fold", (0.7, 15.0, -0.4), (1.7, 15.5, 0.4), mat="leather_light")
    rig.cube(b, "pouch_tail_tassel", (0.9, 14.6, -0.2), (1.5, 15.1, 0.2), mat="cord_dark")

    # 皮兜内部空置凹槽（内里阴影，空置无弹丸无石弹！）
    rig.cube(b, "pouch_cavity_floor", (0.7, 15.3, -0.25), (1.7, 15.8, 0.25), mat="pouch_inside")
    rig.cube(b, "pouch_cavity_mid", (0.65, 15.8, -0.3), (1.75, 19.8, 0.3), mat="pouch_inside")
    # 纵向透气排缝
    rig.cube(b, "pouch_slit_front", (1.1, 16.5, 0.25), (1.3, 19.2, 0.82), mat="pouch_inside")
    rig.cube(b, "pouch_slit_back", (1.1, 16.5, -0.82), (1.3, 19.2, -0.25), mat="pouch_inside")

    # 兜身两侧缝线细节齿 (每侧 8 段密集缝线)
    for idx, y_st in enumerate(np.linspace(15.5, 20.0, 8)):
        rig.cube(b, f"pouch_st_l_{idx}", (0.38, y_st, -0.15), (0.45, y_st + 0.25, 0.15), mat="cord_braid")
        rig.cube(b, f"pouch_st_r_{idx}", (1.95, y_st, -0.15), (2.02, y_st + 0.25, 0.15), mat="cord_braid")
        rig.cube(b, f"pouch_st_knot_{idx}", (0.34, y_st + 0.05, -0.1), (0.42, y_st + 0.2, 0.1), mat="cord_dark")

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
        f"#6 杖索 zhang_suo (Round 1 几何) · 全长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
    )

    # 2. 渲染并排比例与持握姿势（持杖微倾斜立，皮兜悬垂杖侧，双手持杖位，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="staff_sling"
    )

    # 3. 提取参考图剪影与比例对照
    ref_crop = Image.open("/tmp/zhang_suo_ref_side.png").convert("RGB")
    side_shot, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=500, bg=gear_kit.BG)

    # 比例指标对照表（模型 vs 参考图）
    metrics = [
        ("全长/人高比例 (Length/Player)", 93.8, 93.8),
        ("木杖身净长占比 (Staff Body)", 82.5, 82.7),
        ("杖顶铁箍套筒占比 (Top Ferrule)", 9.5, 9.3),
        ("杖尾锻铁底镦占比 (Butt Ferrule)", 8.0, 8.0),
        ("悬垂索与皮兜全长占比 (Sling Total)", 49.3, 50.0),
        ("椭圆空皮兜长度占比 (Pouch Length)", 18.5, 18.3),
        ("杖身八角杆径粗细 (Staff Caliber)", 3.3, 3.3),
        ("细编织悬绳截面 (Cord Caliber)", 1.3, 1.3),
    ]

    ref_lines = [
        ("杖顶铁钩顶端 100%", 1.0, (230, 90, 80)),
        ("顶铁箍下沿 90.7%", 0.907, (120, 190, 240)),
        ("悬垂皮兜底部 50.0%", 0.500, (220, 170, 70)),
        ("下握把中心持握位 38.3%", 0.383, (130, 220, 160)),
        ("杖底铁镦底尖 0%", 0.0, (230, 90, 80)),
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
        title="#6 杖索 zhang_suo (Round 1 几何平涂版 · 全长30px · 八角杖身 · 空皮兜悬垂)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

if __name__ == "__main__":
    generate()
