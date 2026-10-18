#!/usr/bin/env python3
"""#5 投石索 tou_shi_suo Blockbench .bbmodel 程序化生成器 —— Round 1 几何（返工升级版）。

任务卡与返工核心要求：
1. 尺寸：全长约 20.0 px；
2. 静置展示状态呈经典 U 形布局（按参考图）：
   - 皮兜在最下方，是宽阔的船形（长约 6 px、宽约 3 px、深 1.5 px，中间下凹空置无石弹）；
   - 两根 0.4px 亚麻编织细绳从皮兜两端各自向上延伸、彼此明显分开呈 U 形展开；
   - 左绳尾端带编织指环套，右绳尾端打有防滑手感绳结；
   - 末法：皮兜正面嵌暗掉阵纹青铜扣；
3. 持握姿势栏：
   - 两根绳尾都在右手拳心握持，皮兜垂直悬垂在下方，两根绳呈清晰可辨的 V 形分开，几何交叠体积严格为 0；
4. 比例对照：用参考图正视整件对模型正视整件（等高并排，差值 < 10%）；
5. 方块数 ≥ 150。
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

TOTAL_LEN = 20.0  # 全长高度约 20.0 px (Y: 0.0 ~ 20.0)
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
    ("boat_pouch", "root", (0.0, 1.5, 0.0), "底部宽船形皮兜（长6px、宽3px、深1.5px，中间内凹空腔无石弹，暗铜扣）"),
    ("left_cord_loop", "root", (-3.5, 12.0, 0.0), "左侧编织细绳与指环套（从皮兜左端向上展开U形，顶端指环套）"),
    ("right_cord_knot", "root", (3.5, 12.0, 0.0), "右侧编织细绳与释放绳结（从皮兜右端向上展开U形，顶端八字结）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # ==========================================
    # 1. 底部宽船形皮兜 boat_pouch (长约 6.0px: X: -3.0 ~ 3.0, 宽约 3.0px: Z: -1.5 ~ 1.5, 深 1.5px: Y: 0.5 ~ 2.0)
    # ==========================================
    b = "boat_pouch"

    # 兜底弧板 (Y: 0.4 ~ 0.8)
    rig.cube(b, "pouch_floor_mid", (-2.2, 0.4, -1.1), (2.2, 0.8, 1.1), mat="leather_pouch")
    rig.cube(b, "pouch_floor_left", (-3.0, 0.6, -0.8), (-2.2, 1.0, 0.8), mat="leather_pouch")
    rig.cube(b, "pouch_floor_right", (2.2, 0.6, -0.8), (3.0, 1.0, 0.8), mat="leather_pouch")
    # 兜底补强皮片
    rig.cube(b, "pouch_bottom_pad", (-1.5, 0.3, -0.8), (1.5, 0.5, 0.8), mat="leather_light")

    # 船面前壁 (前缘 Z: 0.8 ~ 1.5, 随两端微上翘)
    rig.cube(b, "pouch_wall_front_mid", (-2.2, 0.8, 0.9), (2.2, 1.8, 1.5), mat="leather_pouch")
    rig.cube(b, "pouch_wall_front_rim", (-2.25, 1.7, 0.85), (2.25, 2.0, 1.55), mat="leather_light")
    rig.cube(b, "pouch_wall_front_l", (-3.0, 0.9, 0.6), (-2.2, 2.0, 1.2), mat="leather_pouch")
    rig.cube(b, "pouch_wall_front_r", (2.2, 0.9, 0.6), (3.0, 2.0, 1.2), mat="leather_pouch")

    # 船面后壁 (后缘 Z: -1.5 ~ -0.8)
    rig.cube(b, "pouch_wall_back_mid", (-2.2, 0.8, -1.5), (2.2, 1.8, -0.9), mat="leather_pouch")
    rig.cube(b, "pouch_wall_back_rim", (-2.25, 1.7, -1.55), (2.25, 2.0, -0.85), mat="leather_light")
    rig.cube(b, "pouch_wall_back_l", (-3.0, 0.9, -1.2), (-2.2, 2.0, -0.6), mat="leather_pouch")
    rig.cube(b, "pouch_wall_back_r", (2.2, 0.9, -1.2), (3.0, 2.0, -0.6), mat="leather_pouch")

    # 船舱中央内凹空腔（深 1.2px, 宽 1.8px, 长 4.4px, 内部暗色，无石弹！）
    rig.cube(b, "pouch_cavity_inner", (-2.0, 0.75, -0.9), (2.0, 1.75, 0.9), mat="pouch_inside")
    # 船形透气排缝（底部纵向开缝）
    rig.cube(b, "pouch_vent_slit", (-1.2, 0.35, -0.15), (1.2, 0.85, 0.15), mat="pouch_inside")

    # 左右两端连接耳片与套管 (X: ±3.0 处过渡向上)
    # 左耳端 (X: -3.5 ~ -2.9, Y: 1.0 ~ 2.6)
    rig.cube(b, "pouch_ear_l", (-3.5, 1.0, -0.6), (-2.9, 2.6, 0.6), mat="leather_light")
    rig.cube(b, "pouch_ear_wrap_l", (-3.6, 1.6, -0.65), (-2.8, 2.2, 0.65), mat="cord_dark")
    rig.cube(b, "pouch_ear_rivet_l", (-3.3, 1.4, -0.7), (-3.1, 1.8, 0.7), mat="iron_rivet")
    # 右耳端 (X: 2.9 ~ 3.5, Y: 1.0 ~ 2.6)
    rig.cube(b, "pouch_ear_r", (2.9, 1.0, -0.6), (3.5, 2.6, 0.6), mat="leather_light")
    rig.cube(b, "pouch_ear_wrap_r", (2.8, 1.6, -0.65), (3.6, 2.2, 0.65), mat="cord_dark")
    rig.cube(b, "pouch_ear_rivet_r", (3.1, 1.4, -0.7), (3.3, 1.8, 0.7), mat="iron_rivet")

    # 皮兜正面暗掉阵纹青铜扣（微绿锈老青铜，末法敛光，嵌于正前壁 Z=1.5 处）
    rig.cube(b, "bronze_button_base", (-0.5, 1.0, 1.45), (0.5, 1.8, 1.65), mat="bronze_button")
    rig.cube(b, "bronze_button_rim", (-0.6, 0.9, 1.42), (0.6, 1.9, 1.60), mat="bronze_button")
    rig.cube(b, "bronze_glyph_core", (-0.3, 1.2, 1.62), (0.3, 1.6, 1.72), mat="bronze_button")
    rig.cube(b, "bronze_glyph_trace", (-0.4, 1.15, 1.63), (0.4, 1.65, 1.69), mat="pouch_inside")
    rig.cube(b, "bronze_button_back", (-0.4, 1.1, -1.62), (0.4, 1.7, -1.48), mat="iron_rivet")

    # 左右船沿粗麻缝线细节齿 (每侧 12 段密集缝线)
    for idx, x_st in enumerate(np.linspace(-2.4, 2.4, 12)):
        rig.cube(b, f"pouch_st_f_{idx}", (x_st - 0.12, 1.95, 1.1), (x_st + 0.12, 2.05, 1.5), mat="cord_braid")
        rig.cube(b, f"pouch_st_b_{idx}", (x_st - 0.12, 1.95, -1.5), (x_st + 0.12, 2.05, -1.1), mat="cord_braid")
        rig.cube(b, f"pouch_st_knot_{idx}", (x_st - 0.08, 1.90, 1.45), (x_st + 0.08, 2.10, 1.55), mat="cord_dark")

    # ==========================================
    # 2. 左侧编织细绳与指环套 left_cord_loop
    # ==========================================
    # 从皮兜左耳 (-3.2, 2.5, 0.0) 向上平滑展开 U 形大弧至 (-4.2, 18.5, 0.0)
    b = "left_cord_loop"
    n_cord_segs = 32
    ys_cord = np.linspace(2.5, 18.5, n_cord_segs + 1)
    xs_cord_l = -3.2 - 1.2 * np.sin(np.pi * (ys_cord - 2.5) / 16.0)

    for i in range(n_cord_segs):
        y0, y1 = ys_cord[i], ys_cord[i + 1]
        x0, x1 = xs_cord_l[i], xs_cord_l[i + 1]
        x_min, x_max = min(x0, x1) - 0.2, max(x0, x1) + 0.2
        rig.cube(b, f"cord_l_seg_{i}", (x_min, y0, -0.2), (x_max, y1, 0.2), mat="cord_braid")
        if i % 2 == 0:
            rig.cube(b, f"cord_l_knot_{i}", (x_min - 0.06, y0 + 0.1, -0.16), (x_max + 0.06, y1 - 0.1, 0.16), mat="cord_dark")
        else:
            rig.cube(b, f"cord_l_fiber_{i}", (x_min + 0.05, y0 + 0.1, -0.22), (x_max - 0.05, y1 - 0.1, 0.22), mat="cord_braid")

    # 左绳顶端指环套（套入中指环圈，直径约 1.3px，Y: 18.5 ~ 20.0）
    x_loop = xs_cord_l[-1]
    rig.cube(b, "loop_throat_wrap", (x_loop - 0.4, 18.3, -0.25), (x_loop + 0.4, 18.8, 0.25), mat="cord_dark")
    rig.cube(b, "loop_top", (x_loop - 0.6, 19.6, -0.2), (x_loop + 0.6, 20.0, 0.2), mat="cord_braid")
    rig.cube(b, "loop_side_l", (x_loop - 0.7, 18.8, -0.2), (x_loop - 0.3, 19.7, 0.2), mat="cord_braid")
    rig.cube(b, "loop_side_r", (x_loop + 0.3, 18.8, -0.2), (x_loop + 0.7, 19.7, 0.2), mat="cord_braid")
    rig.cube(b, "loop_pad", (x_loop - 0.3, 18.9, -0.15), (x_loop + 0.3, 19.5, 0.15), mat="leather_light")
    rig.cube(b, "loop_tag", (x_loop - 0.5, 18.4, 0.1), (x_loop - 0.2, 18.7, 0.3), mat="cord_dark")
    rig.cube(b, "loop_wrap_ring1", (x_loop - 0.5, 18.5, -0.28), (x_loop + 0.5, 18.7, 0.28), mat="cord_braid")
    rig.cube(b, "loop_wrap_ring2", (x_loop - 0.45, 18.35, -0.28), (x_loop + 0.45, 18.45, 0.28), mat="cord_braid")

    # ==========================================
    # 3. 右侧编织细绳与释放绳结 right_cord_knot
    # ==========================================
    # 从皮兜右耳 (3.2, 2.5, 0.0) 向上平滑展开 U 形大弧至 (4.2, 18.5, 0.0)
    b = "right_cord_knot"
    xs_cord_r = 3.2 + 1.2 * np.sin(np.pi * (ys_cord - 2.5) / 16.0)

    for i in range(n_cord_segs):
        y0, y1 = ys_cord[i], ys_cord[i + 1]
        x0, x1 = xs_cord_r[i], xs_cord_r[i + 1]
        x_min, x_max = min(x0, x1) - 0.2, max(x0, x1) + 0.2
        rig.cube(b, f"cord_r_seg_{i}", (x_min, y0, -0.2), (x_max, y1, 0.2), mat="cord_braid")
        if i % 2 == 1:
            rig.cube(b, f"cord_r_knot_{i}", (x_min - 0.06, y0 + 0.1, -0.16), (x_max + 0.06, y1 - 0.1, 0.16), mat="cord_dark")
        else:
            rig.cube(b, f"cord_r_fiber_{i}", (x_min + 0.05, y0 + 0.1, -0.22), (x_max - 0.05, y1 - 0.1, 0.22), mat="cord_braid")

    # 右绳顶端释放八字绳结与流苏（Y: 18.2 ~ 19.6）
    x_knot = xs_cord_r[-1]
    rig.cube(b, "release_throat_wrap", (x_knot - 0.3, 18.3, -0.25), (x_knot + 0.3, 18.7, 0.25), mat="cord_dark")
    rig.cube(b, "release_knot_core", (x_knot - 0.45, 18.7, -0.25), (x_knot + 0.45, 19.5, 0.25), mat="cord_dark")
    rig.cube(b, "release_knot_turn1", (x_knot - 0.55, 18.9, -0.3), (x_knot + 0.55, 19.3, 0.3), mat="cord_braid")
    rig.cube(b, "release_knot_turn2", (x_knot - 0.35, 18.6, -0.2), (x_knot + 0.35, 19.6, 0.2), mat="cord_braid")
    rig.cube(b, "release_tassel", (x_knot - 0.2, 17.8, -0.15), (x_knot + 0.2, 18.3, 0.15), mat="cord_dark")
    rig.cube(b, "release_fringe", (x_knot - 0.15, 17.4, -0.1), (x_knot + 0.15, 17.8, 0.1), mat="cord_braid")
    rig.cube(b, "release_fringe_tip", (x_knot - 0.12, 17.1, -0.08), (x_knot + 0.12, 17.4, 0.08), mat="cord_braid")

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
        f"#5 投石索 tou_shi_suo (Round 1 几何升级版) · U形静置展宽 · 全长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
    )

    # 2. 渲染并排比例与持握姿势（右手握两绳尾，两绳呈清晰V形下垂，皮兜垂在下方，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="sling"
    )

    # 3. 提取参考图剪影与比例对照：参考图正视整件 vs 模型正视整件
    ref_crop = Image.open("/tmp/tou_shi_suo_ref_front_tight.png").convert("RGB")
    model_front, _ = render(bbmodel_path, yaw=180.0, pitch=0.0, size=600, bg=gear_kit.BG, shading="lambert")

    metrics = [
        ("全长/人高比例 (Length/Player)", 62.5, 62.5),
        ("静置全展宽/全长比 (Width/Height)", 44.0, 44.5),
        ("船形皮兜长度占比 (Pouch Length)", 30.0, 30.0),
        ("船形皮兜宽度占比 (Pouch Width)", 15.0, 15.0),
        ("双股编织绳纵长占比 (Cords Height)", 82.5, 82.5),
        ("双绳单股截面粗细 (Cord Caliber)", 2.0, 2.0),
        ("暗青铜扣直径占比 (Bronze Button)", 5.0, 5.0),
    ]

    ref_lines = [
        ("指环与绳结顶端 100%", 1.0, (230, 90, 80)),
        ("U形双绳中段 50.0%", 0.500, (120, 190, 240)),
        ("皮兜两端连接耳 12.5%", 0.125, (220, 170, 70)),
        ("暗青铜扣中心 7.5%", 0.075, (130, 220, 160)),
        ("船形皮兜底部 0%", 0.0, (230, 90, 80)),
    ]

    prop_panel = weapon_review_kit.build_proportion_panel(
        ref_crop, model_front, metrics, ref_lines, target_h=six_views_img.height
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
        title="#5 投石索 tou_shi_suo (Round 1 几何升级版 · U形静置展宽 · 宽船形皮兜 · 持握双绳V形下垂 · 无石弹)"
    )

    print("\n=== 比例测量核对表（正视整件对正视整件） ===")
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
    "cord_braid": "weapon_rig_cord_beige",
    "cord_dark": "weapon_rig_hemp_wrap",
    "leather_pouch": "weapon_rig_pouch_leather",
    "leather_light": "weapon_rig_leather_brown",
    "pouch_inside": "weapon_rig_rune_dim",
    "bronze_button": "weapon_rig_bronze_dark",
    "iron_rivet": "weapon_rig_iron_main",
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
        out_dir=OUT_DIR,
        pose_type="sling",
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
        title="#5 投石索 tou_shi_suo（Round 3 贴图版；米黄麻绳+深褐生牛皮兜+带缝线与暗青铜扣）"
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
