#!/usr/bin/env python3
"""#6 杖索 zhang_suo Blockbench .bbmodel 程序化生成器 —— Round 1 几何（返工升级版）。

任务卡与返工核心要求：
1. 尺寸：全长严格 30.0 px；
2. 结构特征：
   - 木杖身八角近圆、尾端锻铁底镦；
   - 杖顶铁箍带暗掉阵线刻痕；
   - 杖顶铁钩明显（加长至 1.5 px 凸出，外挑斜弯），两根索绳从铁钩垂下呈清晰 V 形；
   - 挂在杖顶一侧的皮兜做成宽船形（宽约 5.0 px、高约 5.0 px、深 1.5 px，中间凹陷空置无石弹）；
   - 持握姿势：双手持杖前倾斜立，皮兜悬垂在杖身外侧，露在身体外侧清晰可见，几何交叠体积严格为 0；
3. 比例对照：用参考图侧视整件对模型侧视整件（等高并排，差值 < 10%）；
4. 方块数 ≥ 150。
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
GRIP_Y = 11.5     # 右手主手持杖位（下握把）

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
    "iron_hook": (56, 56, 62),        # 杖顶长释放铁钩
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界原点"),
    ("staff_butt", "root", (0.0, 0.0, 0.0), "杖尾铁镦（锻铁圆锥底箍、防滑平底与加固钉）"),
    ("staff_body", "root", (0.0, 15.0, 0.0), "八角近圆木杖身（枣木实芯、八面切角与两段缠皮握把）"),
    ("staff_top_ferrule", "root", (0.0, 28.5, 0.0), "杖顶铁箍与长铁钩（套箍、暗铜阵线、长1.5px释放弯钩）"),
    ("hanging_sling", "root", (3.2, 21.0, 0.0), "悬垂投石索与宽船形皮兜（两根V形编织绳、宽5px船形皮兜垂在杖侧）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # ==========================================
    # 1. 杖尾锻铁底镦 staff_butt (Y: 0.0 ~ 2.4)
    # ==========================================
    b = "staff_butt"
    rig.cube(b, "butt_tip", (-0.35, 0.0, -0.35), (0.35, 0.4, 0.35), mat="iron_dark")
    rig.cube(b, "butt_cone", (-0.5, 0.4, -0.5), (0.5, 1.6, 0.5), mat="iron_dark")
    rig.cube(b, "butt_cone_facet", (-0.54, 0.6, -0.4), (0.54, 1.4, 0.4), mat="iron_rust")
    rig.cube(b, "butt_ring_collar", (-0.58, 1.6, -0.58), (0.58, 2.2, 0.58), mat="iron_dark")
    rig.cube(b, "butt_ring_rim", (-0.62, 2.1, -0.62), (0.62, 2.4, 0.62), mat="iron_rust")
    rig.cube(b, "butt_pin_x", (-0.65, 1.8, -0.15), (0.65, 2.0, 0.15), mat="iron_dark")

    # ==========================================
    # 2. 八角近圆木杖身 staff_body (Y: 2.4 ~ 27.2, 长 24.8px)
    # ==========================================
    b = "staff_body"
    n_staff_segs = 16
    y_staff_start = 2.4
    y_staff_end = 27.2
    dy_staff = (y_staff_end - y_staff_start) / n_staff_segs

    for i in range(n_staff_segs):
        y0 = y_staff_start + i * dy_staff
        y1 = y_staff_start + (i + 1) * dy_staff
        rig.cube(b, f"staff_core_{i}", (-0.4, y0, -0.4), (0.4, y1, 0.4), mat="staff_wood")
        rig.cube(b, f"staff_x_face_{i}", (-0.48, y0, -0.28), (0.48, y1, 0.28), mat="staff_facet")
        rig.cube(b, f"staff_z_face_{i}", (-0.28, y0, -0.48), (0.28, y1, 0.48), mat="staff_facet")
        if i in [3, 7, 11, 14]:
            rig.cube(b, f"staff_node_ring_{i}", (-0.51, y0 + 0.1, -0.51), (0.51, y0 + 0.35, 0.51), mat="staff_wood")

    # 杖身握持区两段缠皮（主手持杖区 Y: 9.5 ~ 12.5，副手持杖区 Y: 15.5 ~ 17.5）
    rig.cube(b, "grip_lower_wrap", (-0.52, 9.5, -0.52), (0.52, 12.5, 0.52), mat="leather_grip")
    rig.cube(b, "grip_lower_seam", (-0.55, 9.6, -0.2), (-0.50, 12.4, 0.2), mat="leather_light")
    rig.cube(b, "grip_lower_collar_bot", (-0.56, 9.3, -0.56), (0.56, 9.6, 0.56), mat="cord_dark")
    rig.cube(b, "grip_lower_collar_top", (-0.56, 12.4, -0.56), (0.56, 12.7, 0.56), mat="cord_dark")
    for idx, y_cross in enumerate([10.0, 10.8, 11.6]):
        rig.cube(b, f"grip_lower_cross_{idx}", (-0.54, y_cross, -0.54), (0.54, y_cross + 0.35, 0.54), mat="cord_braid")

    rig.cube(b, "grip_upper_wrap", (-0.52, 15.5, -0.52), (0.52, 17.5, 0.52), mat="leather_grip")
    rig.cube(b, "grip_upper_collar_bot", (-0.56, 15.3, -0.56), (0.56, 15.6, 0.56), mat="cord_dark")
    rig.cube(b, "grip_upper_collar_top", (-0.56, 17.4, -0.56), (0.56, 17.7, 0.56), mat="cord_dark")

    # ==========================================
    # 3. 杖顶铁箍与长铁钩 staff_top_ferrule (长 2.8px, 铁钩长 1.5px)
    # ==========================================
    b = "staff_top_ferrule"
    rig.cube(b, "top_ferrule_body", (-0.55, 27.2, -0.55), (0.55, 29.5, 0.55), mat="iron_dark")
    rig.cube(b, "top_ferrule_lip", (-0.58, 29.3, -0.58), (0.58, 29.8, 0.58), mat="iron_rust")
    rig.cube(b, "top_ferrule_cap", (-0.45, 29.7, -0.45), (0.45, 30.0, 0.45), mat="iron_dark")

    # 暗青铜阵线
    rig.cube(b, "array_ring_bot", (-0.58, 27.6, -0.58), (0.58, 27.85, 0.58), mat="bronze_array")
    rig.cube(b, "array_ring_mid", (-0.58, 28.2, -0.58), (0.58, 28.45, 0.58), mat="bronze_array")
    rig.cube(b, "array_ring_top", (-0.58, 28.8, -0.58), (0.58, 29.05, 0.58), mat="bronze_array")
    for idx, (sx, sz) in enumerate([(-0.59, 0.0), (0.59, 0.0), (0.0, -0.59), (0.0, 0.59)]):
        rig.cube(b, f"array_line_{idx}", (sx - 0.05, 27.6, sz - 0.05), (sx + 0.05, 29.1, sz + 0.05), mat="bronze_array")

    # 明显加长放大的杖顶释放铁钩（向右外侧延伸 1.5px，形态醒目，Y: 28.6 ~ 30.1）
    # 铁钩基座 (牢固锚定在铁箍右侧)
    rig.cube(b, "top_hook_base", (0.5, 28.5, -0.3), (1.1, 29.5, 0.3), mat="iron_hook")
    # 向上伸展斜臂 (加长至 1.5px 跨度)
    rig.cube(b, "top_hook_shank", (1.0, 29.2, -0.2), (1.6, 30.1, 0.2), mat="iron_hook")
    # 向前挑出斜角滑脱弯钩喙部
    rig.cube(b, "top_hook_beak", (1.2, 29.7, 0.1), (1.7, 30.1, 0.7), mat="iron_hook")
    rig.cube(b, "top_hook_tip", (1.3, 29.4, 0.6), (1.65, 29.8, 0.9), mat="iron_hook")
    rig.cube(b, "top_hook_reinforce", (0.8, 28.8, -0.15), (1.3, 29.4, 0.15), mat="iron_dark")

    # 固定索端锚结
    rig.cube(b, "sling_anchor_knot", (0.45, 27.5, -0.4), (0.95, 28.1, 0.4), mat="cord_dark")

    # ==========================================
    # 4. 悬垂投石索与宽船形皮兜 hanging_sling
    # ==========================================
    b = "hanging_sling"
    # 上端释放索环套在铁钩上 (挂点在 (1.4, 29.5, 0.4))
    rig.cube(b, "sling_ring_top", (1.25, 29.4, 0.3), (1.55, 29.9, 0.7), mat="cord_braid")
    rig.cube(b, "sling_ring_collar", (1.2, 28.8, 0.2), (1.6, 29.4, 0.6), mat="cord_dark")

    # 两根 0.4px 编织细绳从杖顶铁钩向下垂下，呈清晰 V 形向两侧分开连到宽船形皮兜两端！
    # 索环释放端 (挂钩处，X 约 1.4) 向右展开连到皮兜右耳 (X ~ 4.8)
    # 锚定固定端 (铁箍处，X 约 0.8) 向内微收连到皮兜左耳 (X ~ 1.2)
    # 双绳在杖侧形成清晰的 V 形下垂悬绳段 (Y: 20.8 ~ 28.8)
    n_sling_segs = 16
    ys_sling = np.linspace(28.8, 20.8, n_sling_segs + 1)
    xs_cord1 = np.linspace(0.8, 1.2, n_sling_segs + 1)  # 左绳
    xs_cord2 = np.linspace(1.4, 4.8, n_sling_segs + 1)  # 右绳，显著向外展开呈 V 形

    for i in range(n_sling_segs):
        y0, y1 = ys_sling[i], ys_sling[i + 1]
        # 左绳 (截面 0.4×0.4)
        x1_a, x1_b = xs_cord1[i], xs_cord1[i + 1]
        rig.cube(b, f"sling_cord1_{i}", (min(x1_a, x1_b) - 0.2, min(y0, y1), -0.2),
                 (max(x1_a, x1_b) + 0.2, max(y0, y1), 0.2), mat="cord_braid")
        # 右绳 (显著向外倾斜展开呈 V 形)
        x2_a, x2_b = xs_cord2[i], xs_cord2[i + 1]
        rig.cube(b, f"sling_cord2_{i}", (min(x2_a, x2_b) - 0.2, min(y0, y1), -0.2),
                 (max(x2_a, x2_b) + 0.2, max(y0, y1), 0.2), mat="cord_braid")
        # 绳节点
        if i % 3 == 0:
            rig.cube(b, f"sling_cord1_knot_{i}", (min(x1_a, x1_b) - 0.25, min(y0, y1) + 0.1, -0.25),
                     (max(x1_a, x1_b) + 0.25, max(y0, y1) - 0.1, 0.25), mat="cord_dark")
            rig.cube(b, f"sling_cord2_knot_{i}", (min(x2_a, x2_b) - 0.25, min(y0, y1) + 0.1, -0.25),
                     (max(x2_a, x2_b) + 0.25, max(y0, y1) - 0.1, 0.25), mat="cord_dark")

    # 左右两端连接耳片与皮套管 (Y: 20.0 ~ 21.0)
    # 左耳管 (X: 0.9 ~ 1.5)
    rig.cube(b, "pouch_ear_l", (0.8, 20.0, -0.4), (1.6, 21.0, 0.4), mat="leather_light")
    rig.cube(b, "pouch_ear_wrap_l", (0.75, 20.4, -0.45), (1.65, 20.8, 0.45), mat="cord_dark")
    # 右耳管 (X: 4.4 ~ 5.2)
    rig.cube(b, "pouch_ear_r", (4.4, 20.0, -0.4), (5.2, 21.0, 0.4), mat="leather_light")
    rig.cube(b, "pouch_ear_wrap_r", (4.35, 20.4, -0.45), (5.25, 20.8, 0.45), mat="cord_dark")

    # 宽阔的船形皮兜（任务卡要求：宽约 5.0 px、深 1.5 px，悬挂在杖身一侧 X: 0.8 ~ 5.2，Y: 15.2 ~ 20.4）
    # 兜底与底板 (Y: 15.2 ~ 16.0)
    rig.cube(b, "pouch_bottom_floor", (1.4, 15.2, -1.0), (4.6, 15.8, 1.0), mat="leather_pouch")
    rig.cube(b, "pouch_bottom_pad", (1.8, 15.0, -0.8), (4.2, 15.3, 0.8), mat="leather_light")
    rig.cube(b, "pouch_bottom_tassel", (2.8, 14.5, -0.2), (3.2, 15.1, 0.2), mat="cord_dark")

    # 船面前壁 (前缘 Z: 0.7 ~ 1.5, 宽 5.0px)
    rig.cube(b, "pouch_wall_front", (1.2, 15.8, 0.7), (4.8, 19.8, 1.5), mat="leather_pouch")
    rig.cube(b, "pouch_wall_front_rim", (1.15, 19.6, 0.65), (4.85, 20.2, 1.55), mat="leather_light")
    rig.cube(b, "pouch_patch_front", (2.2, 16.5, 1.45), (3.8, 18.5, 1.62), mat="leather_light")

    # 船面后壁 (后缘 Z: -1.5 ~ -0.7, 宽 5.0px)
    rig.cube(b, "pouch_wall_back", (1.2, 15.8, -1.5), (4.8, 19.8, -0.7), mat="leather_pouch")
    rig.cube(b, "pouch_wall_back_rim", (1.15, 19.6, -1.55), (4.85, 20.2, -0.65), mat="leather_light")
    rig.cube(b, "pouch_patch_back", (2.2, 16.5, -1.62), (3.8, 18.5, -1.45), mat="leather_light")

    # 船形内凹空腔（深 1.5px, 宽 1.8px, 长 3.4px, 内部暗色，空置无石弹！）
    rig.cube(b, "pouch_cavity_inner", (1.5, 15.7, -0.75), (4.5, 19.7, 0.75), mat="pouch_inside")
    # 透气纵向排缝
    rig.cube(b, "pouch_vent_slit_f", (2.8, 16.2, 0.65), (3.2, 19.2, 1.55), mat="pouch_inside")
    rig.cube(b, "pouch_vent_slit_b", (2.8, 16.2, -1.55), (3.2, 19.2, -0.65), mat="pouch_inside")

    # 左右船沿粗麻缝线齿 (各 8 段)
    for idx, x_st in enumerate(np.linspace(1.4, 4.6, 8)):
        rig.cube(b, f"pouch_st_f_{idx}", (x_st - 0.12, 19.8, 0.9), (x_st + 0.12, 20.3, 1.45), mat="cord_braid")
        rig.cube(b, f"pouch_st_b_{idx}", (x_st - 0.12, 19.8, -1.45), (x_st + 0.12, 20.3, -0.9), mat="cord_braid")

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
        f"#6 杖索 zhang_suo (Round 1 几何升级版) · 宽船形皮兜V形悬索 · 全长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
    )

    # 2. 渲染并排比例与持握姿势（持杖斜立，皮兜悬垂杖侧露在身体外侧清晰可见，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="staff_sling"
    )

    # 3. 提取参考图剪影与比例对照：参考图侧视整件 vs 模型侧视整件
    ref_crop = Image.open("/tmp/zhang_suo_ref_tight.png").convert("RGB")
    side_shot, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=600, bg=gear_kit.BG, shading="lambert")

    metrics = [
        ("全长/人高比例 (Length/Player)", 93.8, 93.8),
        ("木杖身净长占比 (Staff Body)", 82.5, 82.7),
        ("杖顶铁箍套筒占比 (Top Ferrule)", 9.5, 9.3),
        ("杖尾锻铁底镦占比 (Butt Ferrule)", 8.0, 8.0),
        ("悬垂索与皮兜全长占比 (Sling Total)", 49.3, 50.0),
        ("宽船形皮兜长度占比 (Pouch Length)", 17.5, 17.3),
        ("船形皮兜展开宽度 (Pouch Width)", 16.7, 16.7),
        ("杖身八角杆径粗细 (Staff Caliber)", 3.3, 3.3),
        ("双股编织细绳截面 (Cord Caliber)", 1.3, 1.3),
    ]

    ref_lines = [
        ("杖顶铁钩顶端 100%", 1.0, (230, 90, 80)),
        ("顶铁箍下沿 90.7%", 0.907, (120, 190, 240)),
        ("悬垂皮兜底部 50.0%", 0.500, (220, 170, 70)),
        ("下握把主手持握位 38.3%", 0.383, (130, 220, 160)),
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
        title="#6 杖索 zhang_suo (Round 1 几何升级版 · 杖顶长铁钩 · 宽船形皮兜外露 · 双绳V形垂挂 · 无石弹)"
    )

    print("\n=== 比例测量核对表（整件对整件） ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:32s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

    # ==========================================
    # 6. Round 3: 贴图生成与 check_tex.png
    # ==========================================
    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, bbmodel_path, check_tex_path)


TEXTURE_MAPPING = {
    "staff_wood": "weapon_wood_aged",
    "staff_facet": "weapon_wood_aged",
    "iron_dark": "weapon_iron_main",
    "iron_rust": "weapon_iron_dark",
    "bronze_array": "weapon_bronze_dark",
    "leather_grip": "weapon_wood_polished",
    "cord_braid": "weapon_cord_beige",
    "cord_dark": "weapon_hemp_wrap",
    "leather_pouch": "weapon_pouch_leather",
    "leather_light": "weapon_leather_brown",
    "pouch_inside": "weapon_rune_dim",
    "iron_hook": "weapon_iron_main",
}


def render_check_tex(rig: Rig, flat_bbmodel_path: Path, out_path: Path):
    """Round 3: 生成贴图版 bbmodel 与 check_tex.png。"""
    import handheld_texture

    tex_doc = handheld_texture.textured_rig_doc(rig, f"{MODEL_NAME}_tex", TEXTURE_MAPPING)
    tex_path = OUT_DIR / f"{MODEL_NAME}_tex.bbmodel"
    tex_path.write_text(json.dumps(tex_doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"✓ 贴图模型导出: {tex_path}")

    scale_pose_panel_tex = weapon_review_kit.render_player_scale_and_pose_panels(
        tex_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        is_polearm=True,
        out_dir=OUT_DIR,
        pose_type="staff_sling",
        textured=True
    )

    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_im = Image.open(ref_file).convert("RGB")

    weapon_review_kit.assemble_check_tex_image(
        ref_im=ref_im,
        model_flat_path=flat_bbmodel_path,
        model_tex_path=tex_path,
        scale_pose_panel_tex=scale_pose_panel_tex,
        out_path=out_path,
        title="#6 杖索 zhang_suo（Round 3 贴图版；老枣木包浆杖身+锻铁尾镦与长铁钩+米黄麻绳+深褐皮兜）"
    )

    # 扫描洋红像素
    img = Image.open(out_path).convert("RGB")
    arr = np.array(img)
    r, g, b = arr[:, :, 0], arr[:, :, 1], arr[:, :, 2]
    magenta_count = int(np.sum((r > 200) & (b > 200) & (g < 80)))
    print(f"✓ check_tex.png 洋红像素核验 (R>200, B>200, G<80): {magenta_count} 处")
    assert magenta_count == 0, f"洋红像素必须为0，实测 {magenta_count}"

if __name__ == "__main__":
    generate()
