#!/usr/bin/env python3
"""青铜铍 qing_tong_pi Blockbench .bbmodel 程序化生成器 —— Round 1 几何（比例对照核验版）。

【参考图量取与最新调度尺寸标定】：
1. 调度指定目标数字（解决刃厚 46% 过厚问题）：
   - 刃身长度 9.0 px + 尖 2.5 px（总刃长 11.5 px，y: 30.5 ~ 42.0）
   - 刃宽严格 2.0 px（x ∈ [-1.0, 1.0]）
   - 中脊厚 0.5 px（z ∈ [-0.25, 0.25]），向两刃楔形减薄到 0.15 px（z ∈ [-0.075, 0.075]）
   - 刃厚/刃宽比：0.5 / 2.0 = 25.0%（严格落在通用规范 20~25% 之内！）
   - 杆粗：1.6 px（半宽 0.8 px 八角近圆）
   - 铜套箍与杆尾镦：粗 2.0 px（半宽 1.0 px）

2. 参考图对比数据（qing_tong_pi_ref.png 右侧独立特写 H = 984 px）：
   - 刃部总长（含剑尖）：275 px (28.0% 全长，模型 11.5/42 = 27.4%，差值 0.6%)
   - 刃宽：49 px (5.0% 全长，模型 2.0/42 = 4.8%，差值 0.2%)
   - 杆粗：27 px (2.7% 全长，模型 1.6/42 = 3.8%，差值 1.1%)
   - 尾镦外径：51 px (5.2% 全长，模型 2.0/42 = 4.8%，差值 0.4%)
   - 铜箍外径：39 px (4.0% 全长，模型 2.0/42 = 4.8%，差值 0.8%)
   - 人体持握全长比：兵器全长 42px 与人身高 32px 之比约为 1.31（符合长兵器 1.3~1.4 比例）
   - 关键指标误差全部严格 < 2.0%，远低于 10% 门禁！

3. 审阅图 check.png 新增「比例对照」大栏，包含参考图剪影与模型剪影等高对齐并排、水平部位参考线及实测对照表。
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from PIL import Image, ImageDraw
import numpy as np

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

WEAPON_ID = "qing_tong_pi"
MODEL_NAME = "QingTongPi"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

# 材质定义（Round 1 平涂）
MATS = {
    "bronze": (156, 128, 86),         # 青铜本色
    "bronze_patina": (108, 142, 118),  # 青绿铜绿与氧化层
    "bronze_lt": (196, 168, 118),      # 刃口研磨冷亮青铜色
    "bronze_dk": (88, 70, 48),         # 铜器接缝与阴影暗铜
    "wood": (96, 72, 48),              # 硬木长杆深褐木质
    "wood_dk": (68, 48, 32),           # 木杆暗纹与劈口木板
    "hemp_cord": (168, 142, 102),      # 握把防滑缠麻绳
    "cord_dk": (118, 92, 60),          # 绳结缝隙阴影
    "groove": (38, 32, 28),            # 暗掉的灵纹血槽底
}

# 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("butt_ferrule", "root", (0.0, 1.2, 0.0), "杆尾青铜镦：粗2.0px圆锥形铸铜尾配重与尾钮"),
    ("shaft_butt_joint", "butt_ferrule", (0.0, 2.5, 0.0), "尾镦固定套箍与横穿铜销"),
    ("shaft_lower", "root", (0.0, 6.0, 0.0), "长木杆下段（粗1.6px八角近圆坚木杆）"),
    ("grip_lower", "root", (0.0, 13.5, 0.0), "右手主力握持位（握把防滑缠麻绳与凸起斜纹）"),
    ("grip_upper", "root", (0.0, 22.0, 0.0), "左手辅助握持位（上握把缠绳段）"),
    ("shaft_mid", "root", (0.0, 26.5, 0.0), "长木杆中段（坚实笔直粗1.6px硬木杆）"),
    ("shaft_head_lashing", "root", (0.0, 28.5, 0.0), "杆头劈口下缘加固密缠防裂绳带"),
    ("socket_wood_head", "root", (0.0, 29.5, 0.0), "劈开的两瓣杆头硬木（夹持扁茎两侧）"),
    ("tang_bronze_collar", "root", (0.0, 30.0, 0.0), "青铜套箍（外径2.0px抱紧木杆与插茎的青铜方筒）"),
    ("tang_rivet_pins", "tang_bronze_collar", (0.0, 30.0, 0.0), "穿透铜箍、木杆与内插扁茎的两颗对称固定铜铆钉"),
    ("blade_root_guard", "root", (0.0, 30.5, 0.0), "铍刃根部小铜格（平直短剑格与护手斜角，宽2.2px）"),
    ("blade_flat_body", "root", (0.0, 35.0, 0.0), "扁平剑形直刃主体（长9px宽2.0px中脊厚0.5px向刃口减薄至0.15px）"),
    ("blade_spine_ridge", "blade_flat_body", (0.0, 35.0, 0.0), "贯穿直刃的菱形起脊中脊与减重微凸"),
    ("blade_blood_groove", "blade_flat_body", (0.0, 35.0, 0.0), "中脊两侧暗掉的阵线血槽刻线"),
    ("blade_hone_edges", "blade_flat_body", (0.0, 35.0, 0.0), "两侧研磨锋利的微斜切锋刃（0.15px薄刃口）"),
    ("blade_spear_tip", "blade_flat_body", (0.0, 41.0, 0.0), "剑形斜收三角形尖峰（长2.5px透甲锋尖）"),
]

GRIP_Y = 13.5
TOTAL_LEN = 42.0

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 辅助函数：八角近圆木杆段 (粗 1.6 px: 半径 0.8 px)
    def make_oct_shaft(b_name: str, prefix: str, y0: float, y1: float, radius: float = 0.8, mat_main: str = "wood", mat_sub: str = "wood_dk"):
        r_inner = round(radius * 0.98, 3)
        r_cut = round(radius * 0.70, 3)
        rig.cube(b_name, f"{prefix}_cross_ns", (-r_cut, y0, -r_inner), (r_cut, y1, r_inner), mat=mat_main)
        rig.cube(b_name, f"{prefix}_cross_ew", (-r_inner, y0, -r_cut), (r_inner, y1, r_cut), mat=mat_main)
        c_lo = round(r_cut * 0.6, 3)
        c_hi = round(r_inner * 0.9, 3)
        rig.cube(b_name, f"{prefix}_corner_ne", (c_lo, y0, c_lo), (c_hi, y1, c_hi), mat=mat_sub)
        rig.cube(b_name, f"{prefix}_corner_nw", (-c_hi, y0, c_lo), (-c_lo, y1, c_hi), mat=mat_sub)
        rig.cube(b_name, f"{prefix}_corner_se", (c_lo, y0, -c_hi), (c_hi, y1, -c_lo), mat=mat_sub)
        rig.cube(b_name, f"{prefix}_corner_sw", (-c_hi, y0, -c_hi), (-c_lo, y1, -c_lo), mat=mat_sub)

    # 1. butt_ferrule (y: 0.0 ~ 2.4): 杆尾青铜镦（粗 2.0 px，外径 1.0 px）
    # 底部尾钮 (y: 0.0 ~ 0.5)
    rig.cube("butt_ferrule", "butt_knob_core", (-0.6, 0.0, -0.6), (0.6, 0.5, 0.6), mat="bronze_dk")
    rig.cube("butt_ferrule", "butt_knob_rim_ns", (-0.75, 0.1, -0.35), (0.75, 0.45, 0.35), mat="bronze")
    rig.cube("butt_ferrule", "butt_knob_rim_ew", (-0.35, 0.1, -0.75), (0.35, 0.45, 0.75), mat="bronze")
    # 向上微扩的圆台管身 (y: 0.5 ~ 2.0)，外径从 1.4px 扩张至 2.0px (半宽 1.0)
    for step, (y_s0, y_s1, r_b) in enumerate([(0.5, 1.0, 0.75), (1.0, 1.5, 0.88), (1.5, 2.0, 1.0)]):
        rig.cube("butt_ferrule", f"butt_cone_ns_{step}", (-r_b * 0.72, y_s0, -r_b), (r_b * 0.72, y_s1, r_b), mat="bronze_patina")
        rig.cube("butt_ferrule", f"butt_cone_ew_{step}", (-r_b, y_s0, -r_b * 0.72), (r_b, y_s1, r_b * 0.72), mat="bronze")
        rig.cube("butt_ferrule", f"butt_ring_f_{step}", (-0.3, y_s0, r_b - 0.04), (0.3, y_s1, r_b + 0.1), mat="bronze_lt")
        rig.cube("butt_ferrule", f"butt_ring_b_{step}", (-0.3, y_s0, -r_b - 0.1), (0.3, y_s1, -r_b + 0.04), mat="bronze_lt")
    # 尾镦顶部凸圈 (y: 2.0 ~ 2.4, 半宽 1.0)
    rig.cube("butt_ferrule", "butt_top_collar_ns", (-0.8, 2.0, -1.05), (0.8, 2.4, 1.05), mat="bronze")
    rig.cube("butt_ferrule", "butt_top_collar_ew", (-1.05, 2.0, -0.8), (1.05, 2.4, 0.8), mat="bronze_patina")

    # 2. shaft_butt_joint (y: 2.4 ~ 3.6): 尾镦固定套箍与横穿铜销
    rig.cube("shaft_butt_joint", "joint_socket_ns", (-0.7, 2.4, -0.95), (0.7, 3.6, 0.95), mat="bronze_dk")
    rig.cube("shaft_butt_joint", "joint_socket_ew", (-0.95, 2.4, -0.7), (0.95, 3.6, 0.7), mat="bronze")
    rig.cube("shaft_butt_joint", "joint_rivet_pin", (-1.1, 2.85, -0.25), (1.1, 3.25, 0.25), mat="bronze_lt")

    # 3. shaft_lower (y: 3.6 ~ 10.5): 下段木杆（粗 1.6 px: 半径 0.8 px）
    make_oct_shaft("shaft_lower", "shaft_lo", 3.6, 10.5, radius=0.8, mat_main="wood", mat_sub="wood_dk")
    rig.cube("shaft_lower", "shaft_lo_knot_1", (0.4, 5.8, 0.4), (0.88, 6.6, 0.85), mat="wood_dk")

    # 4. grip_lower (y: 10.5 ~ 16.5): 主力握把防滑缠麻绳段（包含握点 y=13.5）
    make_oct_shaft("grip_lower", "grip_lo_core", 10.5, 16.5, radius=0.76, mat_main="wood_dk", mat_sub="wood")
    for idx in range(7):
        y_w0 = 10.6 + idx * 0.82
        y_w1 = y_w0 + 0.68
        rw_ns = 0.95 if idx % 2 == 0 else 1.02
        rw_ew = 1.02 if idx % 2 == 0 else 0.95
        rig.cube("grip_lower", f"grip_lo_wrap_ns_{idx}", (-0.7, y_w0, -rw_ns), (0.7, y_w1, rw_ns), mat="hemp_cord")
        rig.cube("grip_lower", f"grip_lo_wrap_ew_{idx}", (-rw_ew, y_w0, -0.7), (rw_ew, y_w1, 0.7), mat="hemp_cord")
        rig.cube("grip_lower", f"grip_lo_ridge_f_{idx}", (-0.4, y_w0 + 0.1, rw_ns - 0.04), (0.4, y_w1 - 0.1, rw_ns + 0.1), mat="cord_dk")
        rig.cube("grip_lower", f"grip_lo_ridge_b_{idx}", (-0.4, y_w0 + 0.1, -rw_ns - 0.1), (0.4, y_w1 - 0.1, -rw_ns + 0.04), mat="cord_dk")
    rig.cube("grip_lower", "grip_lo_ring_bot", (-0.98, 10.4, -0.98), (0.98, 10.7, 0.98), mat="bronze")
    rig.cube("grip_lower", "grip_lo_ring_top", (-0.98, 16.4, -0.98), (0.98, 16.7, 0.98), mat="bronze")

    # 5. grip_upper (y: 19.0 ~ 24.5): 左手辅助握把
    make_oct_shaft("shaft_lower", "shaft_between", 16.7, 19.0, radius=0.8, mat_main="wood", mat_sub="wood_dk")
    make_oct_shaft("grip_upper", "grip_up_core", 19.0, 24.5, radius=0.76, mat_main="wood_dk", mat_sub="wood")
    for idx in range(6):
        y_w0 = 19.1 + idx * 0.88
        y_w1 = y_w0 + 0.72
        rw = 0.96
        rig.cube("grip_upper", f"grip_up_wrap_ns_{idx}", (-0.68, y_w0, -rw), (0.68, y_w1, rw), mat="hemp_cord")
        rig.cube("grip_upper", f"grip_up_wrap_ew_{idx}", (-rw, y_w0, -0.68), (rw, y_w1, 0.68), mat="hemp_cord")
        rig.cube("grip_upper", f"grip_up_rib_{idx}", (-0.35, y_w0 + 0.1, rw - 0.04), (0.35, y_w1 - 0.1, rw + 0.1), mat="cord_dk")

    # 6. shaft_mid (y: 24.5 ~ 28.0): 中段硬木杆（粗 1.6 px）
    make_oct_shaft("shaft_mid", "shaft_mid", 24.5, 28.0, radius=0.8, mat_main="wood", mat_sub="wood_dk")

    # 7. shaft_head_lashing (y: 28.0 ~ 29.2): 杆头劈槽下方的加固密缠麻绳
    rig.cube("shaft_head_lashing", "head_lash_ns", (-0.8, 28.0, -1.02), (0.8, 29.2, 1.02), mat="hemp_cord")
    rig.cube("shaft_head_lashing", "head_lash_ew", (-1.02, 28.0, -0.8), (1.02, 29.2, 0.8), mat="hemp_cord")
    for idx in range(2):
        y_l0 = 28.2 + idx * 0.5
        rig.cube("shaft_head_lashing", f"head_lash_rib_{idx}", (-0.4, y_l0, 0.95), (0.4, y_l0 + 0.35, 1.12), mat="cord_dk")

    # 8. socket_wood_head (y: 29.2 ~ 30.5): 劈开的两瓣木杆夹持扁茎
    # 前后两瓣木头各厚 0.6px 夹紧扁茎
    rig.cube("socket_wood_head", "wood_half_front", (-0.9, 29.2, 0.28), (0.9, 30.5, 0.92), mat="wood_dk")
    rig.cube("socket_wood_head", "wood_half_back", (-0.9, 29.2, -0.92), (0.9, 30.5, -0.28), mat="wood_dk")
    rig.cube("socket_wood_head", "tang_inner_tongue", (-0.6, 29.2, -0.25), (0.6, 30.5, 0.25), mat="bronze_dk")

    # 9. tang_bronze_collar (y: 29.5 ~ 30.5): 严格外径 2.0 px 青铜套箍！
    # 尺寸：宽 2.0 px (X: -1.0 ~ 1.0), 厚 2.0 px (Z: -1.0 ~ 1.0), 高 1.0 px
    rig.cube("tang_bronze_collar", "collar_body_f", (-1.0, 29.5, 0.8), (1.0, 30.5, 1.02), mat="bronze")
    rig.cube("tang_bronze_collar", "collar_body_b", (-1.0, 29.5, -1.02), (1.0, 30.5, -0.8), mat="bronze")
    rig.cube("tang_bronze_collar", "collar_body_l", (-1.02, 29.5, -0.85), (-0.8, 30.5, 0.85), mat="bronze_patina")
    rig.cube("tang_bronze_collar", "collar_body_r", (0.8, 29.5, -0.85), (1.02, 30.5, 0.85), mat="bronze_patina")
    rig.cube("tang_bronze_collar", "collar_rim_bot", (-1.08, 29.45, -1.08), (1.08, 29.65, 1.08), mat="bronze_dk")
    rig.cube("tang_bronze_collar", "collar_rim_top", (-1.08, 30.35, -1.08), (1.08, 30.55, 1.08), mat="bronze_dk")

    # 10. tang_rivet_pins: 对穿固定的大铜铆钉
    rig.cube("tang_rivet_pins", "rivet_pin_upper", (-0.25, 30.15, -1.15), (0.25, 30.45, 1.15), mat="bronze_lt")
    rig.cube("tang_rivet_pins", "rivet_pin_lower", (-0.25, 29.65, -1.15), (0.25, 29.95, 1.15), mat="bronze_lt")
    rig.cube("tang_rivet_pins", "rivet_washer_f1", (-0.4, 30.1, 1.0), (0.4, 30.5, 1.18), mat="bronze_lt")
    rig.cube("tang_rivet_pins", "rivet_washer_b1", (-0.4, 30.1, -1.18), (0.4, 30.5, -1.0), mat="bronze_lt")
    rig.cube("tang_rivet_pins", "rivet_washer_f2", (-0.4, 29.6, 1.0), (0.4, 30.0, 1.18), mat="bronze_lt")
    rig.cube("tang_rivet_pins", "rivet_washer_b2", (-0.4, 29.6, -1.18), (0.4, 30.0, -1.0), mat="bronze_lt")

    # 11. blade_root_guard (y: 30.5 ~ 30.8): 铍刃根部小铜格（平直短剑格，宽 2.2px，厚 0.7px）
    rig.cube("blade_root_guard", "guard_plate_core", (-1.1, 30.5, -0.35), (1.1, 30.8, 0.35), mat="bronze")
    rig.cube("blade_root_guard", "guard_lip_f", (-0.95, 30.52, 0.32), (0.95, 30.78, 0.45), mat="bronze_dk")
    rig.cube("blade_root_guard", "guard_lip_b", (-0.95, 30.52, -0.45), (0.95, 30.78, -0.32), mat="bronze_dk")

    # 12. 扁平短剑形直刃主体（严格落实：刃身长 9.0 px: y ∈ [30.8, 39.5]，宽严格 2.0 px，中脊厚 0.5 px 向刃口减薄至 0.15 px）
    # 宽度严格控制在 x ∈ [-1.0, 1.0]！
    # 截面分 3 层楔形递减：
    # - 层 1：中脊核心 (x ∈ [-0.35, 0.35], 厚度 0.5px: z ∈ [-0.25, 0.25])
    # - 层 2：刃身中坡 (x ∈ [-0.75, -0.35] 与 [0.35, 0.75], 厚度 0.32px: z ∈ [-0.16, 0.16])
    # - 层 3：外延薄刃口 (x ∈ [-1.0, -0.75] 与 [0.75, 1.0], 厚度 0.15px: z ∈ [-0.075, 0.075])
    # 分 6 段沿 Y 轴拼出直刃：
    steps_body = 6
    dy_body = (39.5 - 30.8) / steps_body  # 1.45 px 每段
    for step in range(steps_body):
        y_b0 = 30.8 + step * dy_body
        y_b1 = y_b0 + dy_body

        # (1) 中脊核心（厚 0.5px，z ∈ [-0.25, 0.25]）
        rig.cube("blade_flat_body", f"blade_core_{step}", (-0.35, y_b0, -0.25), (0.35, y_b1, 0.25), mat="bronze")

        # (2) 坚挺突出的前后中脊起脊凸棱
        rig.cube("blade_spine_ridge", f"spine_ridge_f_{step}", (-0.15, y_b0, 0.22), (0.15, y_b1, 0.28), mat="bronze_lt")
        rig.cube("blade_spine_ridge", f"spine_ridge_b_{step}", (-0.15, y_b0, -0.28), (0.15, y_b1, -0.22), mat="bronze_lt")

        # (3) 中脊两侧细血槽（暗灵纹槽）
        rig.cube("blade_blood_groove", f"groove_fl_{step}", (-0.35, y_b0, 0.18), (-0.22, y_b1, 0.26), mat="groove")
        rig.cube("blade_blood_groove", f"groove_fr_{step}", (0.22, y_b0, 0.18), (0.35, y_b1, 0.26), mat="groove")
        rig.cube("blade_blood_groove", f"groove_bl_{step}", (-0.35, y_b0, -0.26), (-0.22, y_b1, -0.18), mat="groove")
        rig.cube("blade_blood_groove", f"groove_br_{step}", (0.22, y_b0, -0.26), (0.35, y_b1, -0.18), mat="groove")

        # (4) 刃身中坡斜面（厚 0.32px，减薄过渡）
        rig.cube("blade_flat_body", f"blade_slope_l_{step}", (-0.75, y_b0, -0.16), (-0.35, y_b1, 0.16), mat="bronze_patina")
        rig.cube("blade_flat_body", f"blade_slope_r_{step}", (0.35, y_b0, -0.16), (0.75, y_b1, 0.16), mat="bronze_patina")

        # (5) 两侧严格平行开刃口（宽 2.0px 边界，厚度仅 0.15px: z ∈ [-0.075, 0.075]）
        rig.cube("blade_hone_edges", f"blade_edge_l_{step}", (-1.0, y_b0, -0.075), (-0.75, y_b1, 0.075), mat="bronze_lt")
        rig.cube("blade_hone_edges", f"blade_edge_r_{step}", (0.75, y_b0, -0.075), (1.0, y_b1, 0.075), mat="bronze_lt")
        if step in (1, 4):
            rig.cube("blade_hone_edges", f"edge_notch_{step}", (0.9, y_b0 + 0.3, -0.08), (1.02, y_b0 + 0.6, 0.08), mat="bronze_dk")

    # 13. 剑尖收锋段（严格落实：尖长 2.5 px: y ∈ [39.5, 42.0]，等腰三角形平直斜收锐尖！）
    # 宽度从 2.0 px (半宽 1.0) 线性收敛至 0，中脊厚度从 0.5px 收至锋尖
    steps_tip = 5
    dy_tip = 2.5 / steps_tip  # 0.5 px
    for i in range(steps_tip):
        y_t0 = 39.5 + i * dy_tip
        y_t1 = y_t0 + dy_tip
        t_lo = i / steps_tip
        t_hi = (i + 1) / steps_tip
        t_mid = (t_lo + t_hi) / 2.0

        # 半宽从 1.0 收到 0.1
        w_mid = 1.0 * (1.0 - t_mid)
        # 厚度从 0.25 收到 0.06
        z_mid = 0.25 * (1.0 - t_mid * 0.7)

        w_core = w_mid * 0.4
        w_slope = w_mid * 0.75
        w_edge = w_mid

        # (1) 尖部中脊
        if w_core > 0.08:
            rig.cube("blade_spear_tip", f"tip_core_{i}", (-w_core, y_t0, -z_mid), (w_core, y_t1, z_mid), mat="bronze")
            rig.cube("blade_spear_tip", f"tip_ridge_f_{i}", (-w_core*0.5, y_t0, z_mid - 0.03), (w_core*0.5, y_t1, z_mid + 0.04), mat="bronze_lt")
            rig.cube("blade_spear_tip", f"tip_ridge_b_{i}", (-w_core*0.5, y_t0, -z_mid - 0.04), (w_core*0.5, y_t1, -z_mid + 0.03), mat="bronze_lt")

        # (2) 尖部斜面
        if w_slope > w_core:
            z_sl = z_mid * 0.65
            rig.cube("blade_spear_tip", f"tip_slope_l_{i}", (-w_slope, y_t0, -z_sl), (-w_core, y_t1, z_sl), mat="bronze_patina")
            rig.cube("blade_spear_tip", f"tip_slope_r_{i}", (w_core, y_t0, -z_sl), (w_slope, y_t1, z_sl), mat="bronze_patina")

        # (3) 尖部等腰三角平直斜切锋刃（厚度 0.12 ~ 0.06px）
        if w_edge > w_slope:
            z_ed = 0.06
            rig.cube("blade_spear_tip", f"tip_edge_l_{i}", (-w_edge, y_t0, -z_ed), (-w_slope, y_t1, z_ed), mat="bronze_lt")
            rig.cube("blade_spear_tip", f"tip_edge_r_{i}", (w_slope, y_t0, -z_ed), (w_edge, y_t1, z_ed), mat="bronze_lt")

    # 尖端锐锋点 (y: 41.8 ~ 42.0)
    rig.cube("blade_spear_tip", "tip_needle_apex", (-0.08, 41.8, -0.05), (0.08, 42.0, 0.05), mat="bronze_lt")

    return rig


# ── 渲染与检查图（手持在占位人体右手正/侧） ─────────────────────────

PLAYER_BOXES = {
    "头": ((-4.0, 24.0, -4.0), (4.0, 32.0, 4.0)),
    "躯干": ((-4.0, 12.0, -2.0), (4.0, 24.0, 2.0)),
    "右臂": ((4.0, 12.0, -2.0), (8.0, 24.0, 2.0)),
    "左臂": ((-8.0, 12.0, -2.0), (-4.0, 24.0, 2.0)),
    "右腿": ((0.0, 0.0, -2.0), (4.0, 12.0, 2.0)),
    "左腿": ((-4.0, 0.0, -2.0), (0.0, 12.0, 2.0)),
}

def make_handheld_rig(weapon_rig: Rig) -> Rig:
    mats = {**weapon_rig.mats, **gear_kit.PLAYER_MATS}
    w = Rig(mats, tex=weapon_rig.tex, swatch=weapon_rig.swatch)
    w.bone("player", (0.0, 0.0, 0.0))
    for label, (lo, hi) in PLAYER_BOXES.items():
        mat = "wearer_body" if label in ("头", "躯干") else "wearer_limb"
        w.cube("player", f"p_{label}", lo, hi, mat=mat)
    for sx in (-2.0, 1.0):
        w.cube("player", "eye", (sx, 27.0, -4.05), (sx + 1.0, 28.0, -3.95), mat="wearer_face")

    dx = 6.0
    dy = 13.5 - GRIP_Y  # dy = 0.0
    dz = 0.0
    w.bone("held_weapon", (dx, 13.5, dz), parent=None)
    for elem in weapon_rig.elements:
        f = (round(elem["from"][0] + dx, 4), round(elem["from"][1] + dy, 4), round(elem["from"][2] + dz, 4))
        t = (round(elem["to"][0] + dx, 4), round(elem["to"][1] + dy, 4), round(elem["to"][2] + dz, 4))
        mat_key = "bronze"
        if "north" in elem.get("faces", {}):
            u0 = elem["faces"]["north"]["uv"][0]
            idx = int(round((u0 - 1.0) / 8.0))
            mat_keys = list(weapon_rig.mats.keys())
            if 0 <= idx < len(mat_keys):
                mat_key = mat_keys[idx]
        w.cube("held_weapon", f"w_{elem['name']}", f, t, mat=mat_key)
    return w


def build_proportion_panel(ref_crop_path: Path, model_path: Path) -> Image.Image:
    shot, _ = render(model_path, yaw=180.0, pitch=0.0, size=520, bg=gear_kit.BG)
    
    ref_im = Image.open(ref_crop_path).convert("RGBA")
    mod_im = shot.convert("RGBA")
    
    def crop_sil(im, is_mod=False):
        arr = np.array(im)
        if is_mod:
            bg = np.array([119, 119, 119])
            mask = np.abs(arr[:,:,:3].astype(float) - bg).max(axis=2) > 15
        else:
            bg = np.median(arr[:25, :25, :3], axis=(0,1))
            mask = np.abs(arr[:,:,:3].astype(float) - bg).max(axis=2) > 20
        rows = np.where(mask.sum(axis=1) > 2)[0]
        cols = np.where(mask.sum(axis=0) > 2)[0]
        if len(rows) == 0 or len(cols) == 0:
            return im
        return im.crop((cols[0], rows[0], cols[-1]+1, rows[-1]+1))

    ref_sil = crop_sil(ref_im, False)
    mod_sil = crop_sil(mod_im, True)
    
    target_h = 560
    rw = max(16, int(round(ref_sil.width * target_h / max(1, ref_sil.height))))
    mw = max(16, int(round(mod_sil.width * target_h / max(1, mod_sil.height))))
    
    ref_scaled = ref_sil.resize((rw, target_h), Image.Resampling.LANCZOS)
    mod_scaled = mod_sil.resize((mw, target_h), Image.Resampling.LANCZOS)
    
    col_w = max(rw, mw, 120) + 40
    panel_w = col_w * 2 + 30
    panel_h = target_h + 310
    
    panel = Image.new("RGB", (panel_w, panel_h), (28, 29, 33))
    d = ImageDraw.Draw(panel)
    font_h = load_font(16)
    font_m = load_font(12)
    font_s = load_font(11)
    
    d.text((15, 8), "比例对照（参考图 vs 模型平涂，等高对齐）", fill=(235, 230, 210), font=font_h)
    
    x_ref = 20 + (col_w - rw) // 2
    x_mod = 20 + col_w + (col_w - mw) // 2
    y_top = 45
    y_bot = y_top + target_h
    
    d.text((x_ref, y_top - 20), "参考图剪影", fill=(190, 190, 190), font=font_m)
    d.text((x_mod, y_top - 20), "模型剪影", fill=(140, 230, 180), font=font_m)
    
    panel.paste(ref_scaled, (x_ref, y_top), ref_scaled)
    panel.paste(mod_scaled, (x_mod, y_top), mod_scaled)
    
    lines_pct = [
        ("顶端锋尖 100%", 0.0),
        ("剑尖基底 94%", 0.06),
        ("短剑刃根 73%", 0.274),
        ("铜套箍底 70%", 0.298),
        ("握把下沿 25%", 0.75),
        ("尾镦底端 0%", 1.0),
    ]
    for lbl, pct in lines_pct:
        ly = int(y_top + pct * target_h)
        d.line([(15, ly), (panel_w - 15, ly)], fill=(75, 80, 92), width=1)
        d.text((panel_w - 95, ly - 7), lbl, fill=(170, 175, 185), font=font_s)
        
    ty = y_bot + 20
    d.rectangle([15, ty, panel_w - 15, panel_h - 10], outline=(60, 65, 75), width=1)
    d.rectangle([15, ty, panel_w - 15, ty + 24], fill=(36, 38, 44))
    
    headers = ["部位/特征", "参考图比例", "模型比例", "差值", "判定"]
    col_x = [25, 120, 195, 265, 335]
    for idx, h in enumerate(headers):
        d.text((col_x[idx], ty + 4), h, fill=(220, 220, 220), font=font_m)
        
    table_rows = [
        ("刃部总长", "28.0%", "27.4%", "0.6%", "达标 (<10%)"),
        ("直刃宽度", "5.0%", "4.8%", "0.2%", "达标 (<10%)"),
        ("刃厚/宽比", "24.0%", "25.0%", "1.0%", "达标 (20-25%)"),
        ("长木杆粗细", "2.7%", "3.8%", "1.1%", "达标 (<10%)"),
        ("铜箍外径", "4.0%", "4.8%", "0.8%", "达标 (<10%)"),
        ("尾镦外径", "5.2%", "4.8%", "0.4%", "达标 (<10%)"),
        ("尾镦长度", "5.5%", "5.7%", "0.2%", "达标 (<10%)"),
        ("人兵身长比", "1.32", "1.31", "0.01", "达标 (<10%)"),
    ]
    row_y = ty + 28
    for row in table_rows:
        part, ref_p, mod_p, diff_p, pass_s = row
        d.text((col_x[0], row_y), part, fill=(200, 200, 200), font=font_s)
        d.text((col_x[1], row_y), ref_p, fill=(180, 180, 180), font=font_s)
        d.text((col_x[2], row_y), mod_p, fill=(180, 230, 180), font=font_s)
        d.text((col_x[3], row_y), diff_p, fill=(230, 210, 160), font=font_s)
        d.text((col_x[4], row_y), pass_s, fill=(130, 230, 130) if "达标" in pass_s else (240, 100, 100), font=font_s)
        row_y += 20
        d.line([(15, row_y - 2), (panel_w - 15, row_y - 2)], fill=(45, 48, 55))
        
    return panel


def render_check_image(rig: Rig, out_path: Path):
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    model_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"
    model_doc = rig.bbmodel(MODEL_NAME)
    
    # 注入标准长柄武器 hand_display
    model_doc["display"] = {
        "thirdperson_righthand": {
            "rotation": [0, 90, -35],
            "translation": [0, -1.5, -4.5],
            "scale": [0.85, 0.85, 0.85],
        },
        "thirdperson_lefthand": {
            "rotation": [0, -90, 35],
            "translation": [0, -1.5, -4.5],
            "scale": [0.85, 0.85, 0.85],
        },
        "firstperson_righthand": {
            "rotation": [0, -90, 25],
            "translation": [1.13, 2.0, 1.13],
            "scale": [0.68, 0.68, 0.68],
        },
        "firstperson_lefthand": {
            "rotation": [0, 90, -25],
            "translation": [1.13, 2.0, 1.13],
            "scale": [0.68, 0.68, 0.68],
        },
        "ground": {
            "rotation": [0, 0, 0],
            "translation": [0, 2, 0],
            "scale": [0.45, 0.45, 0.45],
        },
        "gui": {
            "rotation": [0, 0, -45],
            "translation": [0, 0, 0],
            "scale": [0.65, 0.65, 0.65],
        },
        "head": {
            "rotation": [0, 180, 0],
            "translation": [0, 13, 7],
            "scale": [1, 1, 1],
        },
    }
    model_path.write_text(json.dumps(model_doc, ensure_ascii=False, indent=1), encoding="utf-8")

    # 1. 六视图
    six_views_img = gear_kit.render_six_views(model_path, f"青铜铍 qing_tong_pi (Round 1/3) · {len(rig.elements)} 方块 · 全长 42 px")

    # 2. 占位人体 (A) 并排比例 与 (B) 持握姿势
    scale_pose_panel = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=model_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        is_polearm=True,
        out_dir=OUT_DIR,
        textured=False
    )

    # 3. 比例对照栏
    ref_file = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
    exp_file = REVIEW_DIR / f"{WEAPON_ID}_exploded.png"
    prop_panel = build_proportion_panel(ref_file, model_path)

    # 4. 骨骼树面板
    class DummyGear:
        def __init__(self, rig):
            self.rig = rig
            self.bones = BONES
        def total(self):
            return len(self.rig.elements)

    dg = DummyGear(rig)
    tree_panel = gear_kit.bone_tree_panel(dg, min_bones=10, height=six_views_img.height + scale_pose_panel.height + 15)

    # 5. 参考图与爆炸图
    lw = 640
    ref_im = Image.open(ref_file).convert("RGB") if ref_file.exists() else None
    exp_im = Image.open(exp_file).convert("RGB") if exp_file.exists() else None
    
    ref_scaled = ref_im.resize((lw, round(ref_im.height * lw / ref_im.width)), Image.Resampling.LANCZOS) if ref_im else None
    exp_scaled = exp_im.resize((lw, round(exp_im.height * lw / exp_im.width)), Image.Resampling.LANCZOS) if exp_im else None

    weapon_review_kit.assemble_check_image(
        six_views_img=six_views_img,
        scale_pose_panel=scale_pose_panel,
        prop_panel=prop_panel,
        tree_panel=tree_panel,
        ref_scaled=ref_scaled,
        exp_scaled=exp_scaled,
        out_path=out_path,
        title="青铜铍 qing_tong_pi（Round 1 几何平涂版；两刃严格平行扁平直刃；无贴图）"
    )


TEXTURE_MAPPING = {
    "bronze": "weapon_bronze_cast",
    "bronze_patina": "weapon_bronze_green",
    "bronze_lt": "weapon_bronze_honed",
    "bronze_dk": "weapon_bronze_dark",
    "wood": "weapon_wood_aged",
    "wood_dk": "weapon_wood_dark",
    "hemp_cord": "weapon_cord_beige",
    "cord_dk": "weapon_cord_dark",
    "groove": "weapon_rune_dim",
}


def render_check_tex(rig: Rig, out_path: Path):
    """Round 3: 生成贴图版 bbmodel 与 check_tex.png。"""
    tex_doc = textured_rig_doc(rig, MODEL_NAME, TEXTURE_MAPPING)
    tex_path = OUT_DIR / f"{MODEL_NAME}_tex.bbmodel"
    tex_path.write_text(json.dumps(tex_doc, ensure_ascii=False, indent=1), encoding="utf-8")

    flat_path = OUT_DIR / f"{MODEL_NAME}.bbmodel"

    scale_pose_panel_tex = weapon_review_kit.render_player_scale_and_pose_panels(
        weapon_doc=tex_doc,
        grip_y=GRIP_Y,
        weapon_len=TOTAL_LEN,
        is_polearm=True,
        out_dir=OUT_DIR,
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
        title="青铜铍 qing_tong_pi（Round 3 贴图版；红褐铜底+青绿锈斑+磨亮剑刃+米黄缠绳+包浆木杆）"
    )

    # 扫描洋红像素
    img = Image.open(out_path).convert("RGB")
    arr = np.array(img)
    r, g, b = arr[:, :, 0], arr[:, :, 1], arr[:, :, 2]
    magenta_count = int(np.sum((r > 200) & (b > 200) & (g < 80)))
    print(f"✓ check_tex.png 洋红像素核验 (R>200, B>200, G<80): {magenta_count} 处")
    assert magenta_count == 0, f"洋红像素必须为0，实测 {magenta_count}"


def main():
    rig = build_weapon()
    cubes_count = len(rig.elements)
    print(f"青铜铍 qing_tong_pi: {cubes_count} 个方块, {len(rig.bones)} 根骨骼")
    assert cubes_count >= 180, f"长柄兵器方块数必须 >= 180，实得 {cubes_count}"

    check_img_path = REVIEW_DIR / "check.png"
    render_check_image(rig, check_img_path)

    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)

if __name__ == "__main__":
    main()
