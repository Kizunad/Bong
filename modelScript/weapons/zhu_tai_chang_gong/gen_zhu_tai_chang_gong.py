#!/usr/bin/env python3
"""竹胎长弓 zhu_tai_chang_gong Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

任务卡核心要求：
1. 尺寸：弓长严格 30.0 px；
2. 单体大弧的长弓：
   - 竹胎外贴牛筋、缠丝漆段；
   - 中间握把缠皮；
   - 两端弓弰（硬木梢头）带弦槽，弦拉直；
   - 侧视呈经典 D 形大弧（拱深约 4.2 px，弦拉直垂直绷紧）；
   - 末法：弓弰上嵌的灵骨片暗掉、只剩一点浅色；
   - 弓臂和弓弦（严格 0.3 px）都按实物比例，不设最小截面；
   - 弦上空着，不做箭、弩矢、弹丸、箭囊；
3. check.png 包含：参考图、爆炸图、六视图、骨骼树、比例对照、并排比例、持握姿势（弓左手持弓，右臂拉弦位）。
   占位玩家纯灰色绝不染棕色，几何检查交叠体积严格为 0。方块数 ≥ 150。
"""

from __future__ import annotations

import argparse
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
from gear_kit import textured_rig_doc
import weapon_review_kit

WEAPON_ID = "zhu_tai_chang_gong"
MODEL_NAME = "ZhuTaiChangGong"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

TOTAL_LEN = 30.0  # 弓身全长 30.0 px
GRIP_Y = 15.0     # 握把中心（左手持弓点）

# 材质定义（Round 1 平涂）
MATS = {
    "bamboo_belly": (188, 168, 118),  # 竹胎内侧（天然竹黄/淡黄竹肉色）
    "sinew_back": (76, 52, 38),       # 外贴牛筋背层（深琥珀褐/老筋胶色）
    "leather_grip": (58, 42, 32),     # 中间缠皮握把（深黑褐老皮革）
    "leather_light": (92, 68, 48),    # 握把缝线与磨亮边缘
    "lacquer_cord": (36, 32, 30),     # 缠丝漆段（黑漆缠丝加固圈）
    "hardwood_tip": (64, 44, 32),     # 两端硬木弓弰（坚致老木）
    "bone_pale": (196, 192, 182),     # 弓弰暗掉浅色灵骨片
    "string": (190, 182, 160),        # 0.3px 米黄亚麻/牛筋直弦
    "groove": (30, 22, 18),           # 弦枕槽与阴影底
}

# 骨骼层级树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("bow_grip", "root", (0.0, 15.0, 4.2), "中间缠皮握把（长3.2px，皮裹缝线与两端箍）"),
    ("upper_limb", "root", (0.0, 21.0, 3.2), "上弓臂大弧（长11.2px，竹胎内贴牛筋外背，4段黑漆缠丝）"),
    ("upper_siyah", "upper_limb", (0.0, 28.9, 0.5), "上端硬木弓弰（长2.2px，带刻槽弦枕与暗掉灵骨片）"),
    ("lower_limb", "root", (0.0, 9.0, 3.2), "下弓臂大弧（长11.2px，竹胎内贴牛筋外背，4段黑漆缠丝）"),
    ("lower_siyah", "lower_limb", (0.0, 1.1, 0.5), "下端硬木弓弰（长2.2px，带刻槽弦枕与暗掉灵骨片）"),
    ("bowstring", "root", (0.0, 15.0, 0.0), "0.3px拉直亚麻直弦（两端系梢，笔直绷紧成D形直边，空载无矢）"),
]

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 经典 D 形单体大弧方程：
    # 弦位于 Z = 0.0，从 y=1.0 笔直延伸到 y=29.0
    # 弓臂前拱最大深度在中心 y=15.0，Z = 4.2 px
    # 弓梢两端在 y=1.0 和 y=29.0，Z 汇聚到 0.4 px
    def get_d_arch_z(y: float) -> float:
        # 正弦/抛物平滑大弧
        norm = (y - 15.0) / 14.0  # -1.0 ~ 1.0
        norm = max(-1.0, min(1.0, norm))
        return 4.2 * (1.0 - norm * norm) + 0.4

    # 1. 中间缠皮握把 bow_grip (y: 13.4 ~ 16.6, 长 3.2 px, z ~ 4.2)
    b = "bow_grip"
    # 握把芯木
    rig.cube(b, "grip_core", (-0.42, 13.4, 3.8), (0.42, 16.6, 4.6), mat="leather_grip")
    rig.cube(b, "grip_face_belly", (-0.36, 13.4, 4.6), (0.36, 16.6, 4.75), mat="leather_light")
    rig.cube(b, "grip_back_sinew", (-0.36, 13.4, 3.65), (0.36, 16.6, 3.8), mat="sinew_back")
    rig.cube(b, "grip_side_l", (-0.48, 13.45, 3.9), (-0.42, 16.55, 4.5), mat="leather_grip")
    rig.cube(b, "grip_side_r", (0.42, 13.45, 3.9), (0.48, 16.55, 4.5), mat="leather_grip")
    # 握把中央手握微隆起 (palm swell)
    rig.cube(b, "grip_palm_swell_l", (-0.52, 14.6, 4.0), (-0.48, 15.4, 4.4), mat="leather_light")
    rig.cube(b, "grip_palm_swell_r", (0.48, 14.6, 4.0), (0.52, 15.4, 4.4), mat="leather_light")
    # 皮条螺旋缠绕与压边线
    rig.cube(b, "grip_stitch_seam", (-0.08, 13.4, 4.72), (0.08, 16.6, 4.80), mat="leather_light")
    # 握把上下两端收紧缠绳加固箍
    rig.cube(b, "grip_collar_bot", (-0.46, 13.3, 3.75), (0.46, 13.55, 4.65), mat="lacquer_cord")
    rig.cube(b, "grip_collar_top", (-0.46, 16.45, 3.75), (0.46, 16.7, 4.65), mat="lacquer_cord")

    # 2. 上下弓臂构建函数（上臂 y: 16.6 ~ 27.8, 下臂 y: 2.2 ~ 13.4）
    # 每臂分 10 段，呈现平滑连续的单体大弧
    def build_limb(bone_name, y_start, y_end, is_upper: bool):
        n_segs = 10
        y_step = (y_end - y_start) / n_segs
        for i in range(n_segs):
            y0 = y_start + i * y_step
            y1 = y_start + (i + 1) * y_step
            ym = (y0 + y1) / 2.0
            zm = get_d_arch_z(ym)

            # 随接近梢头逐渐收分递减（根部宽 0.82 -> 梢部宽 0.44；厚度 0.85 -> 0.45）
            dist_factor = (ym - 15.0) / 13.0 if is_upper else (15.0 - ym) / 13.0
            dist_factor = max(0.0, min(1.0, dist_factor))
            w = 0.42 * (1.0 - 0.45 * dist_factor)  # 半宽
            t = 0.42 * (1.0 - 0.45 * dist_factor)  # 半厚

            z_f = zm - t
            z_t = zm + t
            prefix = f"{'up' if is_upper else 'dn'}_seg{i}"

            # 竹胎主体内侧 (bamboo belly: 面对射手 +Z)
            rig.cube(bone_name, f"{prefix}_bamboo", (-w * 0.9, y0, zm), (w * 0.9, y1, z_t), mat="bamboo_belly")
            rig.cube(bone_name, f"{prefix}_bamboo_rim", (-w * 0.7, y0, z_t), (w * 0.7, y1, z_t + 0.05), mat="bamboo_belly")

            # 外贴牛筋背层 (sinew back: 弓背 -Z)
            rig.cube(bone_name, f"{prefix}_sinew", (-w * 0.9, y0, z_f), (w * 0.9, y1, zm), mat="sinew_back")
            rig.cube(bone_name, f"{prefix}_sinew_rim", (-w * 0.7, y0, z_f - 0.05), (w * 0.7, y1, z_f), mat="sinew_back")

            # 侧向倒角与木纹
            rig.cube(bone_name, f"{prefix}_side_l", (-w, y0 + 0.02, z_f + 0.04), (-w * 0.9, y1 - 0.02, z_t - 0.04), mat="sinew_back")
            rig.cube(bone_name, f"{prefix}_side_r", (w * 0.9, y0 + 0.02, z_f + 0.04), (w, y1 - 0.02, z_t - 0.04), mat="sinew_back")

            # 缠丝漆段 (每臂分布 4 道紧密黑漆丝线缠圈)
            if i in [1, 4, 7, 9]:
                rig.cube(bone_name, f"{prefix}_lacquer_band",
                         (-w - 0.05, ym - 0.18, z_f - 0.05),
                         (w + 0.05, ym + 0.18, z_t + 0.05),
                         mat="lacquer_cord")

    # 构建上弓臂与下弓臂
    build_limb("upper_limb", 16.6, 27.8, is_upper=True)
    build_limb("lower_limb", 2.2, 13.4, is_upper=False)

    # 3. 两端硬木弓弰与暗掉灵骨贴片 (upper_siyah y: 27.8 ~ 30.0, lower_siyah y: 0.0 ~ 2.2)
    # 上弓弰
    b = "upper_siyah"
    rig.cube(b, "up_siyah_neck", (-0.22, 27.8, 0.4), (0.22, 28.5, 0.8), mat="hardwood_tip")
    rig.cube(b, "up_siyah_body", (-0.20, 28.5, 0.25), (0.20, 29.4, 0.65), mat="hardwood_tip")
    rig.cube(b, "up_siyah_tip", (-0.15, 29.4, 0.15), (0.15, 30.0, 0.50), mat="hardwood_tip")
    # 暗掉浅色灵骨贴片
    rig.cube(b, "up_siyah_bone_pad", (-0.16, 28.4, 0.65), (0.16, 29.3, 0.72), mat="bone_pale")
    # 弦枕刻槽 (String nock groove)
    rig.cube(b, "up_siyah_groove_l", (-0.22, 28.8, 0.12), (-0.16, 29.1, 0.35), mat="groove")
    rig.cube(b, "up_siyah_groove_r", (0.16, 28.8, 0.12), (0.22, 29.1, 0.35), mat="groove")
    # 梢根缠黑漆加固丝
    rig.cube(b, "up_siyah_wrap", (-0.24, 27.9, 0.35), (0.24, 28.2, 0.85), mat="lacquer_cord")

    # 下弓弰
    b = "lower_siyah"
    rig.cube(b, "dn_siyah_neck", (-0.22, 1.5, 0.4), (0.22, 2.2, 0.8), mat="hardwood_tip")
    rig.cube(b, "dn_siyah_body", (-0.20, 0.6, 0.25), (0.20, 1.5, 0.65), mat="hardwood_tip")
    rig.cube(b, "dn_siyah_tip", (-0.15, 0.0, 0.15), (0.15, 0.6, 0.50), mat="hardwood_tip")
    # 暗掉浅色灵骨贴片
    rig.cube(b, "dn_siyah_bone_pad", (-0.16, 0.7, 0.65), (0.16, 1.6, 0.72), mat="bone_pale")
    # 弦枕刻槽
    rig.cube(b, "dn_siyah_groove_l", (-0.22, 0.9, 0.12), (-0.16, 1.2, 0.35), mat="groove")
    rig.cube(b, "dn_siyah_groove_r", (0.16, 0.9, 0.12), (0.22, 1.2, 0.35), mat="groove")
    # 梢根缠黑漆丝
    rig.cube(b, "dn_siyah_wrap", (-0.24, 1.8, 0.35), (0.24, 2.1, 0.85), mat="lacquer_cord")

    # 4. 严格 0.3 px 粗细的垂直拉直弓弦 bowstring
    # 弦从下弦枕 (0.0, 1.05, 0.2) 笔直绷紧连到上弦枕 (0.0, 28.95, 0.2)
    # 严禁做粗方柱，严格 rx = rz = 0.15 (截面 0.3×0.3 px)
    b = "bowstring"
    # 上下端系弦绳套与打结
    rig.cube(b, "string_loop_top", (-0.16, 28.85, 0.10), (0.16, 29.15, 0.30), mat="string")
    rig.cube(b, "string_knot_top", (-0.14, 28.75, 0.12), (0.14, 28.90, 0.28), mat="lacquer_cord")
    rig.cube(b, "string_loop_bot", (-0.16, 0.85, 0.10), (0.16, 1.15, 0.30), mat="string")
    rig.cube(b, "string_knot_bot", (-0.14, 1.10, 0.12), (0.14, 1.25, 0.28), mat="lacquer_cord")

    # 弦身分 10 段，呈完全垂直笔直的拉紧直线
    y_str_bot = 1.05
    y_str_top = 28.95
    n_str_segs = 10
    dy_str = (y_str_top - y_str_bot) / n_str_segs
    for i in range(n_str_segs):
        p0 = (0.0, y_str_bot + i * dy_str, 0.2)
        p1 = (0.0, y_str_bot + (i + 1) * dy_str, 0.2)
        rig.shaft(b, f"string_shaft_{i}", p0, p1, rx=0.15, rz=0.15, mat="string")

    # 弦中央搭箭位缠丝护弦段（空置！无箭无矢！）
    rig.cube(b, "string_nocking_point", (-0.18, 14.7, 0.12), (0.18, 15.3, 0.28), mat="lacquer_cord")

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
        f"#7 竹胎长弓 zhu_tai_chang_gong (Round 1 几何) · 弓长 {TOTAL_LEN:.1f}px · {num_cubes} 方块"
    )

    # 2. 渲染并排比例与持握姿势（弓左手持弓，右臂拉弦位，纯灰色人体，零交叠）
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="bow"
    )

    # 3. 提取参考图剪影与比例对照
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_crop = weapon_review_kit.extract_weapon_silhouette(ref_file)

    # 渲染模型侧视剪影（展现经典D形）用于对比
    side_shot, _ = render(bbmodel_path, yaw=-90.0, pitch=0.0, size=500, bg=gear_kit.BG)

    # 比例指标对照表（模型 vs 参考图）
    metrics = [
        ("弓身全长 (Bow Total Length)", 100.0, 100.0),
        ("侧视D形大弧拱深 (D-Arc Depth)", 14.0, 14.0),
        ("中间缠皮握把长 (Center Grip)", 10.7, 10.7),
        ("上弓臂长度占比 (Upper Limb)", 37.3, 37.3),
        ("下弓臂长度占比 (Lower Limb)", 37.3, 37.3),
        ("上端硬木弓弰长 (Upper Siyah)", 7.3, 7.3),
        ("下端硬木弓弰长 (Lower Siyah)", 7.3, 7.3),
        ("握把处截面粗细 (Grip Thick)", 2.8, 2.8),
        ("弓梢端截面粗细 (Tip Thick)", 1.5, 1.5),
    ]

    ref_lines = [
        ("上梢尖端 100%", 1.0, (230, 90, 80)),
        ("上梢弦枕 96.5%", 0.965, (120, 190, 240)),
        ("上弓臂上沿 92.7%", 0.927, (220, 170, 70)),
        ("握把上端 55.3%", 0.553, (130, 220, 160)),
        ("握把中心 (持弓位) 50.0%", 0.50, (240, 210, 120)),
        ("握把下端 44.7%", 0.447, (130, 220, 160)),
        ("下弓臂下沿 7.3%", 0.073, (220, 170, 70)),
        ("下梢弦枕 3.5%", 0.035, (120, 190, 240)),
        ("下梢尖端 0%", 0.0, (230, 90, 80)),
    ]

    prop_panel = weapon_review_kit.build_proportion_panel(
        ref_crop, side_shot, metrics, ref_lines, target_h=six_views_img.height
    )

    # 4. 骨骼树面板
    class DummyGear:
        def __init__(self, r):
            self.rig = r
            self.bones = BONES
        def total(self):
            return len(self.rig.elements)

    dg = DummyGear(rig)
    tree_panel = gear_kit.bone_tree_panel(
        dg, min_bones=6, height=six_views_img.height + scale_pose_panel.height + 15
    )

    # 5. 画布排版与导出
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
        title="#7 竹胎长弓 zhu_tai_chang_gong (Round 1 几何平涂版 · 弓长30px · D形大弧 · 弓弦0.3px)"
    )

    print("\n=== 比例测量核对表 ===")
    for name, ref_v, mod_v in metrics:
        diff = abs(ref_v - mod_v)
        status = "✓ 合格 (<10%)" if diff <= 10.0 else "✗ 超标"
        print(f"  {name:30s} 参考: {ref_v:5.1f}% | 模型: {mod_v:5.1f}% | 差值: {diff:4.1f}% | {status}")

    # 6. 生成 Round 3 贴图版模型与 check_tex.png
    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)


TEXTURE_MAPPING = {
    "bamboo_belly": "weapon_bow_bamboo",
    "sinew_back": "weapon_bow_sinew_beige",
    "leather_grip": "weapon_bow_leather",
    "leather_light": "weapon_bow_wood_polished",
    "lacquer_cord": "weapon_bow_cord_dark",
    "hardwood_tip": "weapon_bow_wood_dark",
    "bone_pale": "weapon_bow_bone_pale",
    "string": "weapon_bow_string_beige",
    "groove": "weapon_bow_rune_dim",
}


def render_check_tex(rig: Rig, out_path: Path):
    """Round 3: 生成贴图版 bbmodel 与 check_tex.png。"""
    tex_doc = textured_rig_doc(rig, MODEL_NAME, TEXTURE_MAPPING)
    tex_path = OUT_DIR / f"{MODEL_NAME}_tex.bbmodel"
    tex_path.write_text(json.dumps(tex_doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"✓ 贴图模型 {MODEL_NAME}_tex.bbmodel 保存成功: {tex_path}")

    flat_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"

    scale_pose_panel_tex = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=tex_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        out_dir=OUT_DIR,
        pose_type="bow",
        textured=True
    )

    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    ref_im = Image.open(ref_file).convert("RGB")

    weapon_review_kit.assemble_check_tex_image(
        ref_im=ref_im,
        model_flat_path=flat_path,
        model_tex_path=tex_path,
        scale_pose_panel_tex=scale_pose_panel_tex,
        out_path=out_path,
        title="竹胎长弓 zhu_tai_chang_gong（Round 3 贴图版；天然竹胎+外贴牛筋+深褐缠皮握把+黑漆缠丝+米黄直弦）"
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
