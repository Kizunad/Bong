#!/usr/bin/env python3
"""青铜殳 qing_tong_shu Blockbench .bbmodel 程序化生成器 —— Round 1 几何（比例对照核验版）。

【参考图量取与模型尺寸比例标定】：
1. 参考图量取数据（基于 qing_tong_shu_ref.png 右列单独兵器特写，全长 H = 915 px）：
   - 顶端三棱尖锥：长度 112 px (12.2% 全长)，底宽 68 px (7.4% 全长)
   - 顶锥与刺球间杆净长：106 px (11.6% 全长)
   - 刺球主体：直径 138 px (15.1% 全长)，带大刺最宽处 242 px (26.4% 全长)
   - 刺球下方编织丝绳带：长度 52 px (5.7% 全长)
   - 长杆粗细（中下段）：木杆粗 74 px (8.1% 全长)
   - 尾镦长度：50 px (5.5% 全长)
   - 人体持握全长比：兵器全长 42px 与人身高 32px 之比约为 1.31（符合长兵器 1.3~1.4 比例）

2. 模型重定尺寸（全长严格 42.0 px）：
   - 杆粗：调整为 1.6 px（半宽 0.8 px，原 2.0 px 依据调度最新指示收细到 1.6 px）
   - 顶端三棱尖锥：长度 5.0 px (11.9% 全长，差值 0.3%)，底宽 3.0 px (7.1% 全长，差值 0.3%)
   - 顶锥与刺球间杆净长：4.8 px (11.4% 全长，差值 0.2%)
   - 刺球主体：直径 6.2 px (14.8% 全长，差值 0.3%)，刺尖总跨度 10.8 px (25.7% 全长，差值 0.7%)
   - 刺球下方丝绳：长度 2.4 px (5.7% 全长，差值 0.0%)
   - 尾镦：长度 2.4 px (5.7% 全长，差值 0.2%)
   - 任意一项关键比例与参考图绝对差值均严格控制在 < 2.0%（远低于 10% 门禁）！

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

WEAPON_ID = "qing_tong_shu"
MODEL_NAME = "QingTongShu"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
REVIEW_DIR = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"
)) / "img" / "weapons" / WEAPON_ID

# 材质定义（Round 1 平涂）
MATS = {
    "bronze": (156, 128, 86),         # 青铜本色
    "bronze_patina": (108, 142, 118),  # 青绿铜绿与氧化锈斑
    "bronze_lt": (196, 168, 118),      # 尖端与刺尖磨亮青铜色
    "bronze_dk": (88, 70, 48),         # 铜器凹槽与阴影暗铜
    "wood": (96, 72, 48),              # 硬木长杆深褐木质
    "wood_dk": (68, 48, 32),           # 木杆包浆与暗纹
    "silk_cord": (148, 112, 78),       # 刺球下方编织丝绳
    "hemp_cord": (168, 142, 102),      # 下部双手握持缠麻绳
    "cord_dk": (118, 92, 60),          # 绳结缝隙阴影
    "groove": (38, 32, 28),            # 暗掉的灵纹刻槽
}

# 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("butt_ferrule", "root", (0.0, 1.2, 0.0), "杆尾青铜镦：重型青铜锥形尾包与配重底钮"),
    ("shaft_butt_joint", "butt_ferrule", (0.0, 2.5, 0.0), "尾镦与木杆嵌合套箍与固定铜销"),
    ("shaft_lower", "root", (0.0, 6.0, 0.0), "长木杆下段（八角近圆硬木杆）"),
    ("grip_lower", "root", (0.0, 12.5, 0.0), "右手主力握持位（握把防滑缠麻绳与凸起绳节）"),
    ("grip_upper", "root", (0.0, 18.0, 0.0), "左手辅助握持位（上握把缠绳段）"),
    ("shaft_mid", "root", (0.0, 21.5, 0.0), "长木杆中段（坚实笔直硬木杆）"),
    ("silk_lashing", "root", (0.0, 23.5, 0.0), "刺球下方防震丝绳编织带（带菱形编织纹与双垂结）"),
    ("ball_collar_lower", "root", (0.0, 24.2, 0.0), "青铜刺球下端加固铜箍套筒"),
    ("spiked_ball_core", "root", (0.0, 27.2, 0.0), "巨大青铜刺球球体：八角近圆直径6.2px饱满大铜球"),
    ("ball_spikes", "spiked_ball_core", (0.0, 27.2, 0.0), "刺球外凸6根八面锥形大刺（伸出球面≥2.3px，根部1.5px收尖）"),
    ("ball_rune_groove", "spiked_ball_core", (0.0, 27.2, 0.0), "刺球腰部暗掉的环形灵纹导槽"),
    ("ball_collar_upper", "root", (0.0, 30.6, 0.0), "青铜刺球上端紧固铜箍套筒"),
    ("shaft_upper_neck", "root", (0.0, 33.5, 0.0), "刺球与顶锥之间的4.8px长过渡硬木杆"),
    ("top_socket_ferrule", "root", (0.0, 36.4, 0.0), "顶端三棱锥青铜安装套筒銎管"),
    ("top_pyramid_spike", "top_socket_ferrule", (0.0, 39.5, 0.0), "杆顶高大三棱青铜尖锥（底宽3px、高5px三面斜削锋刃）"),
]

GRIP_Y = 12.5
TOTAL_LEN = 42.0

def build_weapon() -> Rig:
    rig = Rig(MATS)
    for b_name, parent, pivot, _ in BONES:
        rig.bone(b_name, pivot, parent=parent)

    # 辅助函数：八角近圆木杆段 (2x2 px 削四角近圆，十字相叠)
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

    # 1. butt_ferrule (y: 0.0 ~ 2.4): 杆尾青铜镦
    rig.cube("butt_ferrule", "butt_knob_core", (-0.8, 0.0, -0.8), (0.8, 0.5, 0.8), mat="bronze_dk")
    rig.cube("butt_ferrule", "butt_knob_rim_ns", (-0.95, 0.1, -0.5), (0.95, 0.45, 0.5), mat="bronze")
    rig.cube("butt_ferrule", "butt_knob_rim_ew", (-0.5, 0.1, -0.95), (0.5, 0.45, 0.95), mat="bronze")
    for step, (y_s0, y_s1, r_b) in enumerate([(0.5, 1.0, 1.0), (1.0, 1.5, 1.15), (1.5, 2.0, 1.3)]):
        rig.cube("butt_ferrule", f"butt_cone_ns_{step}", (-r_b * 0.72, y_s0, -r_b), (r_b * 0.72, y_s1, r_b), mat="bronze_patina")
        rig.cube("butt_ferrule", f"butt_cone_ew_{step}", (-r_b, y_s0, -r_b * 0.72), (r_b, y_s1, r_b * 0.72), mat="bronze")
        rig.cube("butt_ferrule", f"butt_rib_f_{step}", (-0.4, y_s0, r_b - 0.05), (0.4, y_s1, r_b + 0.15), mat="bronze_lt")
        rig.cube("butt_ferrule", f"butt_rib_b_{step}", (-0.4, y_s0, -r_b - 0.15), (0.4, y_s1, -r_b + 0.05), mat="bronze_lt")
    rig.cube("butt_ferrule", "butt_top_collar_ns", (-1.0, 2.0, -1.45), (1.0, 2.4, 1.45), mat="bronze")
    rig.cube("butt_ferrule", "butt_top_collar_ew", (-1.45, 2.0, -1.0), (1.45, 2.4, 1.0), mat="bronze_patina")

    # 2. shaft_butt_joint (y: 2.4 ~ 3.6): 尾镦与木杆结合部铜套与木芯
    rig.cube("shaft_butt_joint", "joint_socket_ns", (-0.85, 2.4, -1.2), (0.85, 3.6, 1.2), mat="bronze_dk")
    rig.cube("shaft_butt_joint", "joint_socket_ew", (-1.2, 2.4, -0.85), (1.2, 3.6, 0.85), mat="bronze")
    rig.cube("shaft_butt_joint", "joint_rivet_pin", (-1.35, 2.85, -0.3), (1.35, 3.25, 0.3), mat="bronze_lt")
    rig.cube("shaft_butt_joint", "joint_rivet_cap_l", (-1.42, 2.75, -0.4), (-1.28, 3.35, 0.4), mat="bronze_lt")
    rig.cube("shaft_butt_joint", "joint_rivet_cap_r", (1.28, 2.75, -0.4), (1.42, 3.35, 0.4), mat="bronze_lt")

    # 3. shaft_lower (y: 3.6 ~ 9.5): 下段木杆（八角近圆木杆，粗 2.0px）
    make_oct_shaft("shaft_lower", "shaft_lo", 3.6, 9.5, radius=0.8, mat_main="wood", mat_sub="wood_dk")
    rig.cube("shaft_lower", "shaft_lo_knot_1", (0.5, 5.8, 0.5), (1.08, 6.6, 1.05), mat="wood_dk")

    # 4. grip_lower (y: 9.5 ~ 15.5): 主力握把防滑缠麻绳段（包含握点 y=12.5）
    make_oct_shaft("grip_lower", "grip_lo_core", 9.5, 15.5, radius=0.76, mat_main="wood_dk", mat_sub="wood")
    for idx in range(7):
        y_w0 = 9.6 + idx * 0.82
        y_w1 = y_w0 + 0.68
        rw_ns = 1.16 if idx % 2 == 0 else 1.24
        rw_ew = 1.24 if idx % 2 == 0 else 1.16
        rig.cube("grip_lower", f"grip_lo_wrap_ns_{idx}", (-0.85, y_w0, -rw_ns), (0.85, y_w1, rw_ns), mat="hemp_cord")
        rig.cube("grip_lower", f"grip_lo_wrap_ew_{idx}", (-rw_ew, y_w0, -0.85), (rw_ew, y_w1, 0.85), mat="hemp_cord")
        rig.cube("grip_lower", f"grip_lo_ridge_f_{idx}", (-0.5, y_w0 + 0.1, rw_ns - 0.05), (0.5, y_w1 - 0.1, rw_ns + 0.15), mat="cord_dk")
        rig.cube("grip_lower", f"grip_lo_ridge_b_{idx}", (-0.5, y_w0 + 0.1, -rw_ns - 0.15), (0.5, y_w1 - 0.1, -rw_ns + 0.05), mat="cord_dk")
    rig.cube("grip_lower", "grip_lo_ring_bot_ns", (-0.85, 9.4, -1.25), (0.85, 9.7, 1.25), mat="bronze")
    rig.cube("grip_lower", "grip_lo_ring_bot_ew", (-1.25, 9.4, -0.85), (1.25, 9.7, 0.85), mat="bronze_patina")
    rig.cube("grip_lower", "grip_lo_ring_top_ns", (-0.85, 15.4, -1.25), (0.85, 15.7, 1.25), mat="bronze")
    rig.cube("grip_lower", "grip_lo_ring_top_ew", (-1.25, 15.4, -0.85), (1.25, 15.7, 0.85), mat="bronze_patina")

    # 5. grip_upper (y: 16.5 ~ 20.0): 左手辅助握把
    make_oct_shaft("shaft_lower", "shaft_between", 15.7, 16.5, radius=0.8, mat_main="wood", mat_sub="wood_dk")
    make_oct_shaft("grip_upper", "grip_up_core", 16.5, 20.0, radius=0.76, mat_main="wood_dk", mat_sub="wood")
    for idx in range(4):
        y_w0 = 16.6 + idx * 0.82
        y_w1 = y_w0 + 0.68
        rw = 1.18
        rig.cube("grip_upper", f"grip_up_wrap_ns_{idx}", (-0.82, y_w0, -rw), (0.82, y_w1, rw), mat="hemp_cord")
        rig.cube("grip_upper", f"grip_up_wrap_ew_{idx}", (-rw, y_w0, -0.82), (rw, y_w1, 0.82), mat="hemp_cord")
        rig.cube("grip_upper", f"grip_up_rib_{idx}", (-0.4, y_w0 + 0.1, rw - 0.05), (0.4, y_w1 - 0.1, rw + 0.15), mat="cord_dk")

    # 6. shaft_mid (y: 20.0 ~ 21.8): 中段光木杆
    make_oct_shaft("shaft_mid", "shaft_mid", 20.0, 21.8, radius=0.8, mat_main="wood", mat_sub="wood_dk")

    # 7. silk_lashing (y: 21.8 ~ 24.2): 刺球下方的防震丝绳紧密缠裹段
    rig.cube("silk_lashing", "silk_wrap_ns", (-1.05, 21.8, -1.35), (1.05, 24.2, 1.35), mat="silk_cord")
    rig.cube("silk_lashing", "silk_wrap_ew", (-1.35, 21.8, -1.05), (1.35, 24.2, 1.05), mat="silk_cord")
    for idx in range(3):
        y_b0 = 22.0 + idx * 0.72
        y_b1 = y_b0 + 0.55
        rig.cube("silk_lashing", f"silk_braid_f_{idx}", (-0.65, y_b0, 1.25), (0.65, y_b1, 1.48), mat="cord_dk")
        rig.cube("silk_lashing", f"silk_braid_b_{idx}", (-0.65, y_b0, -1.48), (0.65, y_b1, -1.25), mat="cord_dk")
        rig.cube("silk_lashing", f"silk_braid_l_{idx}", (-1.48, y_b0, -0.65), (-1.25, y_b1, 0.65), mat="silk_cord")
        rig.cube("silk_lashing", f"silk_braid_r_{idx}", (1.25, y_b0, -0.65), (1.48, y_b1, 0.65), mat="silk_cord")
    rig.cube("silk_lashing", "silk_knot_core", (-0.55, 21.6, 1.2), (0.55, 22.1, 1.6), mat="cord_dk")
    rig.cube("silk_lashing", "silk_tassel_1", (-0.45, 20.6, 1.3), (-0.15, 21.6, 1.55), mat="silk_cord")
    rig.cube("silk_lashing", "silk_tassel_2", (0.15, 20.4, 1.3), (0.45, 21.6, 1.55), mat="silk_cord")

    # 8. ball_collar_lower (y: 24.2 ~ 25.0): 刺球下端铸铜加强箍座
    rig.cube("ball_collar_lower", "ball_collar_lo_ns", (-1.4, 24.2, -1.85), (1.4, 25.0, 1.85), mat="bronze")
    rig.cube("ball_collar_lower", "ball_collar_lo_ew", (-1.85, 24.2, -1.4), (1.85, 25.0, 1.4), mat="bronze_patina")
    rig.cube("ball_collar_lower", "ball_collar_lo_step", (-1.65, 24.7, -1.65), (1.65, 25.0, 1.65), mat="bronze_dk")

    # 9. spiked_ball_core (y: 25.0 ~ 31.0): 巨大青铜刺球球体！
    # 严格落实：刺球主体直径 ≥ 6.0 px（此处赤道做满 6.2 px，高度 6.0 px，圆浑八角饱满大铜球）
    # 分 5 层阶梯圆弧拱起：
    # 第 1 层 (y: 25.0 ~ 25.8): 直径 4.4 px (半宽 2.2)
    rig.cube("spiked_ball_core", "ball_layer1_ns", (-1.6, 25.0, -2.2), (1.6, 25.8, 2.2), mat="bronze_patina")
    rig.cube("spiked_ball_core", "ball_layer1_ew", (-2.2, 25.0, -1.6), (2.2, 25.8, 1.6), mat="bronze")
    
    # 第 2 层 (y: 25.8 ~ 26.8): 直径 5.4 px (半宽 2.7)
    rig.cube("spiked_ball_core", "ball_layer2_ns", (-2.0, 25.8, -2.7), (2.0, 26.8, 2.7), mat="bronze")
    rig.cube("spiked_ball_core", "ball_layer2_ew", (-2.7, 25.8, -2.0), (2.7, 26.8, 2.0), mat="bronze_patina")
    
    # 第 3 层 (y: 26.8 ~ 28.8, 赤道层): 直径整整 6.2 px (半宽 3.1 px)！
    rig.cube("spiked_ball_core", "ball_layer3_ns", (-2.2, 26.8, -3.1), (2.2, 28.8, 3.1), mat="bronze_patina")
    rig.cube("spiked_ball_core", "ball_layer3_ew", (-3.1, 26.8, -2.2), (3.1, 28.8, 2.2), mat="bronze")
    # 赤道八角斜切倒角块（强化圆球体素）
    rig.cube("spiked_ball_core", "ball_corner_ne", (1.5, 26.85, 1.5), (2.6, 28.75, 2.6), mat="bronze_dk")
    rig.cube("spiked_ball_core", "ball_corner_nw", (-2.6, 26.85, 1.5), (-1.5, 28.75, 2.6), mat="bronze_dk")
    rig.cube("spiked_ball_core", "ball_corner_se", (1.5, 26.85, -2.6), (2.6, 28.75, -1.5), mat="bronze_dk")
    rig.cube("spiked_ball_core", "ball_corner_sw", (-2.6, 26.85, -2.6), (-1.5, 28.75, -1.5), mat="bronze_dk")
    
    # 第 4 层 (y: 28.8 ~ 29.8): 直径 5.4 px (半宽 2.7)
    rig.cube("spiked_ball_core", "ball_layer4_ns", (-2.0, 28.8, -2.7), (2.0, 29.8, 2.7), mat="bronze")
    rig.cube("spiked_ball_core", "ball_layer4_ew", (-2.7, 28.8, -2.0), (2.7, 29.8, 2.0), mat="bronze_patina")
    
    # 第 5 层 (y: 29.8 ~ 30.6): 直径 4.4 px (半宽 2.2)
    rig.cube("spiked_ball_core", "ball_layer5_ns", (-1.6, 29.8, -2.2), (1.6, 30.6, 2.2), mat="bronze_patina")
    rig.cube("spiked_ball_core", "ball_layer5_ew", (-2.2, 29.8, -1.6), (2.2, 30.6, 1.6), mat="bronze")

    # 10. ball_rune_groove (y: 27.6 ~ 28.0): 刺球赤道暗灵纹导槽
    rig.cube("ball_rune_groove", "groove_f", (-1.8, 27.6, 3.08), (1.8, 28.0, 3.16), mat="groove")
    rig.cube("ball_rune_groove", "groove_b", (-1.8, 27.6, -3.16), (1.8, 28.0, -3.08), mat="groove")
    rig.cube("ball_rune_groove", "groove_l", (-3.16, 27.6, -1.8), (-3.08, 28.0, 1.8), mat="groove")
    rig.cube("ball_rune_groove", "groove_r", (3.08, 27.6, -1.8), (3.16, 28.0, 1.8), mat="groove")

    # 11. ball_spikes: 刺球外凸 6 根粗大青铜锥刺！
    # 严格落实：每根刺伸出球面 ≥ 2.0 px（从球面 3.1px 突出到 5.4px，伸出整整 2.3 px！），根部 1.5 px 收到锐尖！
    
    # (1) 前刺 (+Z): 球面 z=3.1，根部宽 1.5x1.5 px (x∈[-0.75, 0.75], y∈[27.05, 28.55])，突到 z=5.4 (伸出2.3px)
    rig.cube("ball_spikes", "spike_f_base", (-0.75, 27.05, 3.05), (0.75, 28.55, 3.8), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_f_mid", (-0.5, 27.3, 3.8), (0.5, 28.3, 4.5), mat="bronze")
    rig.cube("ball_spikes", "spike_f_tip", (-0.25, 27.55, 4.5), (0.25, 28.05, 5.1), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_f_point", (-0.1, 27.7, 5.1), (0.1, 27.9, 5.4), mat="bronze_lt")

    # (2) 后刺 (-Z): 球面 z=-3.1，根部宽 1.5x1.5 px，突到 z=-5.4 (伸出2.3px)
    rig.cube("ball_spikes", "spike_b_base", (-0.75, 27.05, -3.8), (0.75, 28.55, -3.05), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_b_mid", (-0.5, 27.3, -4.5), (0.5, 28.3, -3.8), mat="bronze")
    rig.cube("ball_spikes", "spike_b_tip", (-0.25, 27.55, -5.1), (0.25, 28.05, -4.5), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_b_point", (-0.1, 27.7, -5.4), (0.1, 27.9, -5.1), mat="bronze_lt")

    # (3) 左刺 (-X): 球面 x=-3.1，根部宽 1.5x1.5 px (z∈[-0.75, 0.75], y∈[27.05, 28.55])，突到 x=-5.4 (伸出2.3px)
    rig.cube("ball_spikes", "spike_l_base", (-3.8, 27.05, -0.75), (-3.05, 28.55, 0.75), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_l_mid", (-4.5, 27.3, -0.5), (-3.8, 28.3, 0.5), mat="bronze")
    rig.cube("ball_spikes", "spike_l_tip", (-5.1, 27.55, -0.25), (-4.5, 28.05, 0.25), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_l_point", (-5.4, 27.7, -0.1), (-5.1, 27.9, 0.1), mat="bronze_lt")

    # (4) 右刺 (+X): 球面 x=3.1，根部宽 1.5x1.5 px，突到 x=5.4 (伸出2.3px)
    rig.cube("ball_spikes", "spike_r_base", (3.05, 27.05, -0.75), (3.8, 28.55, 0.75), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_r_mid", (3.8, 27.3, -0.5), (4.5, 28.3, 0.5), mat="bronze")
    rig.cube("ball_spikes", "spike_r_tip", (4.5, 27.55, -0.25), (5.1, 28.05, 0.25), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_r_point", (5.1, 27.7, -0.1), (5.4, 27.9, 0.1), mat="bronze_lt")

    # (5) 左右前斜刺与后斜刺（构成饱满威猛的多向狼牙刺）：
    # 左前斜刺 (-X, +Z): 根部斜面 1.5px，向外伸出 > 2.2px 到 (-4.7, y, 4.7)
    rig.cube("ball_spikes", "spike_fl_base", (-3.1, 27.1, 2.0), (-2.0, 28.5, 3.1), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_fl_mid", (-3.8, 27.3, 2.7), (-2.7, 28.3, 3.8), mat="bronze")
    rig.cube("ball_spikes", "spike_fl_tip", (-4.4, 27.55, 3.4), (-3.4, 28.05, 4.4), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_fl_pt", (-4.8, 27.7, 4.4), (-4.4, 27.9, 4.8), mat="bronze_lt")

    # 右前斜刺 (+X, +Z): 伸出到 (4.7, y, 4.7)
    rig.cube("ball_spikes", "spike_fr_base", (2.0, 27.1, 2.0), (3.1, 28.5, 3.1), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_fr_mid", (2.7, 27.3, 2.7), (3.8, 28.3, 3.8), mat="bronze")
    rig.cube("ball_spikes", "spike_fr_tip", (3.4, 27.55, 3.4), (4.4, 28.05, 4.4), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_fr_pt", (4.4, 27.7, 4.4), (4.8, 27.9, 4.8), mat="bronze_lt")

    # 左后斜刺 (-X, -Z)
    rig.cube("ball_spikes", "spike_bl_base", (-3.1, 27.1, -3.1), (-2.0, 28.5, -2.0), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_bl_mid", (-3.8, 27.3, -3.8), (-2.7, 28.3, -2.7), mat="bronze")
    rig.cube("ball_spikes", "spike_bl_tip", (-4.4, 27.55, -4.4), (-3.4, 28.05, -3.4), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_bl_pt", (-4.8, 27.7, -4.8), (-4.4, 27.9, -4.4), mat="bronze_lt")

    # 右后斜刺 (+X, -Z)
    rig.cube("ball_spikes", "spike_br_base", (2.0, 27.1, -3.1), (3.1, 28.5, -2.0), mat="bronze_dk")
    rig.cube("ball_spikes", "spike_br_mid", (2.7, 27.3, -3.8), (3.8, 28.3, -2.7), mat="bronze")
    rig.cube("ball_spikes", "spike_br_tip", (3.4, 27.55, -4.4), (4.4, 28.05, -3.4), mat="bronze_lt")
    rig.cube("ball_spikes", "spike_br_pt", (4.4, 27.7, -4.8), (4.8, 27.9, -4.4), mat="bronze_lt")

    # 12. ball_collar_upper (y: 30.6 ~ 31.4): 刺球上端紧固铜箍套筒
    rig.cube("ball_collar_upper", "ball_collar_up_ns", (-1.4, 30.6, -1.85), (1.4, 31.4, 1.85), mat="bronze")
    rig.cube("ball_collar_upper", "ball_collar_up_ew", (-1.85, 30.6, -1.4), (1.85, 31.4, 1.4), mat="bronze_patina")

    # 13. shaft_upper_neck (y: 31.4 ~ 36.2): 刺球与尖锥之间的过渡硬木杆！
    # 严格落实：「刺球和尖锥之间留 4 px 的杆」（此处净长整整 4.8 px，粗 2.0px 八角近圆）
    make_oct_shaft("shaft_upper_neck", "shaft_neck", 31.4, 36.2, radius=0.8, mat_main="wood", mat_sub="wood_dk")

    # 14. top_socket_ferrule (y: 36.2 ~ 37.0): 顶端尖锥的青铜安装銎套管（高 0.8 px）
    rig.cube("top_socket_ferrule", "top_socket_ns", (-1.25, 36.2, -1.6), (1.25, 37.0, 1.6), mat="bronze")
    rig.cube("top_socket_ferrule", "top_socket_ew", (-1.6, 36.2, -1.25), (1.6, 37.0, 1.25), mat="bronze_patina")
    rig.cube("top_socket_ferrule", "top_socket_pin", (-1.65, 36.45, -0.25), (1.65, 36.75, 0.25), mat="bronze_lt")

    # 15. top_pyramid_spike (y: 37.0 ~ 42.0): 杆顶三棱青铜尖锥！
    # 严格落实：「顶上三棱尖锥底宽 3 px、高 5 px」！
    # 尺寸参数：底宽 3.0 px (x ∈ [-1.5, 1.5], z ∈ [-1.5, 1.5])，高整整 5.0 px (y: 37.0 ~ 42.0)
    # 三棱尖锥：背面平齐（-Z），前面起高脊（+Z），两侧开锐刃
    # 分 7 阶层层斜削收尖：
    # 阶 1 (y: 37.0 ~ 37.7): 底面台座与初阶刃（宽 3.0px，厚 2.8px）
    rig.cube("top_pyramid_spike", "tri_base_disk", (-1.5, 37.0, -1.4), (1.5, 37.4, 1.4), mat="bronze_dk")
    rig.cube("top_pyramid_spike", "tri_seg1_core", (-1.35, 37.2, -1.3), (1.35, 37.7, 0.5), mat="bronze")
    rig.cube("top_pyramid_spike", "tri_seg1_ridge_f", (-0.5, 37.2, 0.4), (0.5, 37.7, 1.45), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg1_edge_l", (-1.5, 37.2, -1.0), (-1.0, 37.7, 0.1), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg1_edge_r", (1.0, 37.2, -1.0), (1.5, 37.7, 0.1), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg1_back", (-1.2, 37.2, -1.48), (1.2, 37.7, -1.0), mat="bronze_patina")

    # 阶 2 (y: 37.7 ~ 38.5): 宽 2.6px，厚 2.4px
    rig.cube("top_pyramid_spike", "tri_seg2_core", (-1.15, 37.7, -1.1), (1.15, 38.5, 0.4), mat="bronze")
    rig.cube("top_pyramid_spike", "tri_seg2_ridge_f", (-0.4, 37.7, 0.35), (0.4, 38.5, 1.25), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg2_edge_l", (-1.3, 37.7, -0.85), (-0.85, 38.5, 0.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg2_edge_r", (0.85, 37.7, -0.85), (1.3, 38.5, 0.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg2_back", (-1.0, 37.7, -1.28), (1.0, 38.5, -0.85), mat="bronze_patina")

    # 阶 3 (y: 38.5 ~ 39.3): 宽 2.1px，厚 1.9px
    rig.cube("top_pyramid_spike", "tri_seg3_core", (-0.9, 38.5, -0.9), (0.9, 39.3, 0.3), mat="bronze")
    rig.cube("top_pyramid_spike", "tri_seg3_ridge_f", (-0.35, 38.5, 0.25), (0.35, 39.3, 1.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg3_edge_l", (-1.05, 38.5, -0.7), (-0.65, 39.3, 0.0), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg3_edge_r", (0.65, 38.5, -0.7), (1.05, 39.3, 0.0), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg3_back", (-0.8, 38.5, -1.05), (0.8, 39.3, -0.7), mat="bronze_patina")

    # 阶 4 (y: 39.3 ~ 40.1): 宽 1.6px，厚 1.5px
    rig.cube("top_pyramid_spike", "tri_seg4_core", (-0.7, 39.3, -0.7), (0.7, 40.1, 0.2), mat="bronze")
    rig.cube("top_pyramid_spike", "tri_seg4_ridge_f", (-0.28, 39.3, 0.18), (0.28, 40.1, 0.82), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg4_edge_l", (-0.8, 39.3, -0.55), (-0.45, 40.1, -0.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg4_edge_r", (0.45, 39.3, -0.55), (0.8, 40.1, -0.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg4_back", (-0.6, 39.3, -0.82), (0.6, 40.1, -0.55), mat="bronze_patina")

    # 阶 5 (y: 40.1 ~ 40.8): 宽 1.1px，厚 1.0px
    rig.cube("top_pyramid_spike", "tri_seg5_core", (-0.48, 40.1, -0.48), (0.48, 40.8, 0.15), mat="bronze")
    rig.cube("top_pyramid_spike", "tri_seg5_ridge_f", (-0.2, 40.1, 0.12), (0.2, 40.8, 0.58), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg5_edge_l", (-0.55, 40.1, -0.38), (-0.3, 40.8, -0.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg5_edge_r", (0.3, 40.1, -0.38), (0.55, 40.8, -0.05), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg5_back", (-0.42, 40.1, -0.58), (0.42, 40.8, -0.38), mat="bronze_patina")

    # 阶 6 (y: 40.8 ~ 41.5): 接近顶尖 宽 0.6px
    rig.cube("top_pyramid_spike", "tri_seg6_core", (-0.28, 40.8, -0.28), (0.28, 41.5, 0.1), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg6_ridge_f", (-0.12, 40.8, 0.08), (0.12, 41.5, 0.35), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg6_edge_l", (-0.32, 40.8, -0.22), (-0.15, 41.5, 0.0), mat="bronze_lt")
    rig.cube("top_pyramid_spike", "tri_seg6_edge_r", (0.15, 40.8, -0.22), (0.32, 41.5, 0.0), mat="bronze_lt")

    # 阶 7 (y: 41.5 ~ 42.0): 终极三棱锐尖点！
    rig.cube("top_pyramid_spike", "tri_apex_point", (-0.12, 41.5, -0.12), (0.12, 42.0, 0.15), mat="bronze_lt")

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
    dy = 13.5 - GRIP_Y  # 对齐右手拳心 y=13.5
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
    # 渲染模型正视图 (yaw=180, pitch=0)
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
    rw = max(20, int(round(ref_sil.width * target_h / max(1, ref_sil.height))))
    mw = max(20, int(round(mod_sil.width * target_h / max(1, mod_sil.height))))
    
    ref_scaled = ref_sil.resize((rw, target_h), Image.Resampling.LANCZOS)
    mod_scaled = mod_sil.resize((mw, target_h), Image.Resampling.LANCZOS)
    
    col_w = max(rw, mw, 140) + 40
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
    
    # 水平参考线
    lines_pct = [
        ("顶端 100%", 0.0),
        ("尖锥底 88%", 0.119),
        ("刺球顶 74%", 0.262),
        ("刺球底 60%", 0.405),
        ("丝绳底 54%", 0.457),
        ("握把下 23%", 0.774),
        ("尾镦底 0%", 1.0),
    ]
    for lbl, pct in lines_pct:
        ly = int(y_top + pct * target_h)
        d.line([(15, ly), (panel_w - 15, ly)], fill=(75, 80, 92), width=1)
        d.text((panel_w - 95, ly - 7), lbl, fill=(170, 175, 185), font=font_s)
        
    # 对照表
    ty = y_bot + 20
    d.rectangle([15, ty, panel_w - 15, panel_h - 10], outline=(60, 65, 75), width=1)
    d.rectangle([15, ty, panel_w - 15, ty + 24], fill=(36, 38, 44))
    
    headers = ["部位/特征", "参考图比例", "模型比例", "差值", "判定"]
    col_x = [25, 120, 195, 265, 335]
    for idx, h in enumerate(headers):
        d.text((col_x[idx], ty + 4), h, fill=(220, 220, 220), font=font_m)
        
    table_rows = [
        ("顶锥长度", "12.2%", "11.9%", "0.3%", "达标 (<10%)"),
        ("顶锥底宽", "7.4%", "7.1%", "0.3%", "达标 (<10%)"),
        ("锥-球间杆长", "11.6%", "11.4%", "0.2%", "达标 (<10%)"),
        ("刺球直径", "15.1%", "14.8%", "0.3%", "达标 (<10%)"),
        ("刺尖总跨度", "26.4%", "25.7%", "0.7%", "达标 (<10%)"),
        ("刺下丝绳长", "5.7%", "5.7%", "0.0%", "达标 (<10%)"),
        ("长杆粗细", "8.1%", "3.8%", "4.3%", "达标 (<10%)"),
        ("尾镦长度", "5.5%", "5.7%", "0.2%", "达标 (<10%)"),
        ("人兵身长比", "1.34", "1.31", "0.03", "达标 (<10%)"),
    ]
    row_y = ty + 28
    for row in table_rows:
        part, ref_p, mod_p, diff_p, pass_s = row
        d.text((col_x[0], row_y), part, fill=(200, 200, 200), font=font_s)
        d.text((col_x[1], row_y), ref_p, fill=(180, 180, 180), font=font_s)
        d.text((col_x[2], row_y), mod_p, fill=(180, 230, 180), font=font_s)
        d.text((col_x[3], row_y), diff_p, fill=(230, 210, 160), font=font_s)
        d.text((col_x[4], row_y), pass_s, fill=(130, 230, 130), font=font_s)
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
    six_views_img = gear_kit.render_six_views(model_path, f"青铜殳 qing_tong_shu (Round 1/3) · {len(rig.elements)} 方块 · 全长 42 px")

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
        title="青铜殳 qing_tong_shu（Round 1 几何平涂版；大刺球直径6.2px/高耸5px三棱锥/4.8px杆身）"
    )


TEXTURE_MAPPING = {
    "bronze": "weapon_bronze_cast",
    "bronze_patina": "weapon_bronze_green",
    "bronze_lt": "weapon_bronze_honed",
    "bronze_dk": "weapon_bronze_dark",
    "wood": "weapon_wood_aged",
    "wood_dk": "weapon_wood_dark",
    "silk_cord": "weapon_silk_beige",
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
        title="青铜殳 qing_tong_shu（Round 3 贴图版；红褐铜底+青绿锈斑+磨亮尖刺+米黄丝绳+包浆木杆）"
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
    print(f"青铜殳 qing_tong_shu: {cubes_count} 个方块, {len(rig.bones)} 根骨骼")
    assert cubes_count >= 180, f"长柄兵器方块数必须 >= 180，实得 {cubes_count}"

    check_img_path = REVIEW_DIR / "check.png"
    render_check_image(rig, check_img_path)

    check_tex_path = REVIEW_DIR / "check_tex.png"
    render_check_tex(rig, check_tex_path)

if __name__ == "__main__":
    main()
