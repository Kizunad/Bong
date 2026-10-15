#!/usr/bin/env python3
"""骨片肩甲（gu_pian_jian_jia）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #13（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，
左臂在 -x 侧（x∈[-8,-4]）。

外形（参考图 gu_pian_jian_jia_ref.png / gu_pian_jian_jia_exploded.png）：
兽骨片叠成的单边肩甲（左肩），皮带斜挎固定，骨片间绑草绳、挂兽牙。
- 左肩甲覆盖左肩峰与上臂外侧：y∈[18.5, 26.8]，x∈[-9.0, -4.10]，z∈[-2.65, 2.65]。
  严禁扎入头盒（x ≤ -4.10 或 y ≤ 23.90）、躯干（y ≥ 24.10 或 z 离开躯干）与左臂盒（外侧 x ≤ -8.10，前 z ≤ -2.10，后 z ≥ 2.10）。
- 骨甲由三层重叠弯曲的粗糙异兽肩胛骨板构成：顶层阔骨、中层覆片、下层护臂板。
- 骨甲上凸起断裂的兽角棱脊与加固骨栓。
- 骨板之间钻孔，用深色草绳穿孔交叉打结固定，绳尾外露。
- 斜挎固定皮带从左肩胛出发，斜穿胸前（z ≤ -2.10）与背后（z ≥ 2.10），在右腰下侧（y ≤ 11.90，避开右臂盒）由粗铁日字扣紧固。
- 骨甲下缘悬挂一串磨损的异兽尖牙（挂件独立骨），右腰带扣下方垂挂备用皮舌与骨片（挂件独立骨）。

骨骼数 ≥ 10，挂件独立骨，全绿门禁（含 clearance、coplanar、self-test）。
用法：
    python3 modelScript/gear/gu_pian_jian_jia/gen_gu_pian_jian_jia.py --self-test
    python3 modelScript/gear/gu_pian_jian_jia/gen_gu_pian_jian_jia.py --gates
    python3 modelScript/gear/gu_pian_jian_jia/gen_gu_pian_jian_jia.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "bone":       (214, 204, 176),    # 异兽骨主色
    "bone_lt":    (234, 224, 198),    # 骨棱亮面 / 獠牙尖端
    "bone_dk":    (168, 154, 126),    # 骨阴影 / 裂纹内壁
    "bone_stain": (132, 112, 84),     # 陈年风化斑 / 老骨深斑
    "hide":       (120, 78, 56),      # 底衬生兽皮
    "hide_dk":    (86, 54, 40),       # 暗皮衬垫
    "strap":      (112, 72, 50),      # 斜挎鞣制皮带
    "strap_dk":   (78, 50, 36),       # 磨损厚皮
    "cord":       (144, 128, 78),     # 扎结麻绳 / 草绳
    "cord_dk":    (104, 90, 56),      # 脏草绳深色
    "iron":       (82, 78, 76),       # 粗锻生铁带扣
    "iron_lt":    (116, 112, 110),    # 铁光泽 / 铆钉
    "rust":       (152, 86, 52),      # 铁扣红锈
    "void":       (28, 24, 22),       # 穿孔暗影
}

# 软件材质与硬质骨铁允许贴合咬合
SOFT = ("hide", "hide_dk", "strap", "strap_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("bone", "bone_dk", "bone_stain", "strap"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("chest_anchor", "root", (0.0, 18.5, -2.45), "胸前固定骨扣：胸骨正中的兽骨定位圆盘与皮垫（中线对称件）"),
    ("pauldron_pad", "root", (-6.0, 24.5, 0.0), "左肩底衬垫：粗兽皮衬层、护肩皮坎、防磨翻边"),
    ("plate_top", "root", (-6.2, 25.5, 0.0), "顶层主骨板：宽厚异兽肩胛骨拱弧、肩峰隆起、顶侧包边"),
    ("plate_mid", "root", (-7.5, 23.2, 0.0), "中层叠压骨片：顺肩斜向外展开的第二层弧形骨甲板"),
    ("plate_low", "root", (-8.3, 20.8, 0.0), "底层护臂骨片：垂挂在外侧上臂的第三层阶梯式弧形坚骨"),
    ("bone_ridge", "plate_top", (-6.0, 26.2, 0.0), "肩峰骨刺脊：断裂异兽突角骨脊、骨刺粗节、骨裂加固栓"),
    ("lash_cords", "root", (-6.8, 23.5, 0.0), "穿骨孔草绳：各层骨甲穿孔扎结草绳、前后十字锁线、悬垂绳结"),
    ("strap_front", "root", (-1.0, 18.5, -2.25), "胸前斜挎皮带：从左肩斜下横跨胸膛到右腰前部、缝线铆钉"),
    ("strap_back", "root", (-1.0, 18.5, 2.25), "背后斜跨皮带：从左肩斜下横跨后背到右腰后部、加强护垫"),
    ("strap_buckle", "root", (4.2, 11.5, 0.0), "右腰带扣总成：下部环腰皮带、生铁日字带扣、咬合舌片"),
    ("hang_teeth", "plate_low", (-8.35, 18.6, -0.2), "骨甲悬挂兽牙：骨板下缘皮绳悬垂的三枚异兽尖牙串（独立可动骨）"),
    ("hang_strap_tail", "strap_buckle", (4.25, 9.8, -0.4), "右腰皮带垂尾：穿出扣环自然下垂的备用打孔皮带头与兽骨坠（独立可动骨）"),
]

# 单肩甲除胸骨中心定位件外整体不对称，其它骨骼进入 asym 白名单
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "chest_anchor"))


# ───────────────────────────────────────────────────────────── 几何部件构建
def part_pad(g: Gear) -> None:
    """左肩底衬垫：粗兽皮衬层、护肩皮坎、防磨翻边。"""
    b = "pauldron_pad"
    # 肩顶皮衬垫：骑在左肩 y ≥ 24.10，x∈[-8.30, -4.12]，z∈[-2.45, 2.45]
    g.box(b, "pad_top_main", (-8.30, 24.10, -2.45), (-4.12, 24.45, 2.45), "hide_dk")
    g.box(b, "pad_top_lip_in", (-4.35, 24.45, -2.25), (-4.10, 24.85, 2.25), "hide")
    # 前胸与后背过渡护皮片（前 z ≤ -2.10，后 z ≥ 2.10）
    g.box(b, "pad_front_flap", (-7.85, 22.80, -2.42), (-4.15, 24.10, -2.10), "hide")
    g.box(b, "pad_back_flap", (-7.85, 22.80, 2.10), (-4.15, 24.10, 2.42), "hide_dk")
    # 外侧下垂包边垫皮（x ≤ -8.14，避免与 mid_outer_plate 共面）
    g.box(b, "pad_arm_outer", (-8.45, 21.85, -2.35), (-8.14, 24.10, 2.35), "hide_dk")
    g.box(b, "pad_stitch_f", (-7.50, 23.10, -2.58), (-4.50, 23.40, -2.32), "cord_dk")
    g.box(b, "pad_stitch_b", (-7.50, 23.10, 2.32), (-4.50, 23.40, 2.58), "cord_dk")


def part_plate_top(g: Gear) -> None:
    """顶层主骨板：宽厚异兽肩胛骨拱弧、肩峰隆起、顶侧包边。"""
    b = "plate_top"
    # 顶部主拱骨板：y∈[24.48, 25.45]，x∈[-8.45, -4.15]，z∈[-2.62, 2.62]
    g.box(b, "top_arch_core", (-8.25, 24.48, -2.35), (-4.25, 25.25, 2.35), "bone")
    g.box(b, "top_arch_ridge", (-7.85, 25.25, -1.85), (-4.55, 25.68, 1.85), "bone_lt")
    # 骨板前倾弧面与后倾弧面
    g.box(b, "top_front_slope", (-8.15, 23.95, -2.62), (-4.35, 24.85, -2.25), "bone")
    g.box(b, "top_back_slope", (-8.15, 23.95, 2.25), (-4.35, 24.85, 2.62), "bone_dk")
    # 内侧贴颈加厚防砍骨坎（x ≤ -4.10，高出肩峰）
    g.box(b, "top_neck_guard", (-4.65, 25.18, -2.15), (-4.18, 26.25, 2.15), "bone_stain")
    g.box(b, "top_neck_crest", (-4.52, 26.25, -1.65), (-4.20, 26.65, 1.65), "bone_lt")
    # 顶板层叠小骨片与粗粝碎骨叠片
    g.box(b, "top_scale_f1", (-7.60, 24.75, -2.52), (-6.15, 25.12, -2.20), "bone_lt")
    g.box(b, "top_scale_f2", (-6.05, 24.80, -2.48), (-4.75, 25.18, -2.18), "bone")
    g.box(b, "top_scale_b1", (-7.60, 24.75, 2.20), (-6.15, 25.12, 2.52), "bone_dk")
    g.box(b, "top_scale_b2", (-6.05, 24.80, 2.18), (-4.70, 25.18, 2.48), "bone_stain")
    # 前后小骨角（肩角凸起）
    g.box(b, "top_horn_f", (-8.25, 24.65, -2.82), (-7.55, 25.35, -2.45), "bone_lt")
    g.box(b, "top_horn_b", (-8.25, 24.65, 2.45), (-7.55, 25.35, 2.82), "bone_stain")
    # 骨板上的风化斑块与断层
    g.box(b, "top_stain_spot1", (-7.25, 25.26, -1.25), (-6.05, 25.52, 0.45), "bone_stain")
    g.box(b, "top_stain_spot2", (-5.85, 24.95, 1.35), (-4.75, 25.32, 2.25), "bone_stain")


def part_plate_mid(g: Gear) -> None:
    """中层叠压骨片：顺肩斜向外展开的第二层弧形骨甲板。"""
    b = "plate_mid"
    # 外侧主体（x ≤ -8.10，y∈[22.35, 24.35]）
    g.box(b, "mid_outer_plate", (-8.75, 22.35, -2.25), (-8.10, 24.35, 2.25), "bone")
    g.box(b, "mid_outer_ridge", (-8.98, 22.85, -1.65), (-8.65, 24.15, 1.65), "bone_lt")
    # 外侧覆叠副骨鳞片
    g.box(b, "mid_flake_top", (-8.92, 23.45, -1.45), (-8.68, 24.25, 1.45), "bone_stain")
    g.box(b, "mid_flake_bot", (-8.92, 22.45, -1.28), (-8.68, 23.25, 1.28), "bone")
    # 前包角（z ≤ -2.10，y 起始 22.25 避开 22.35 共面）
    g.box(b, "mid_front_wrap", (-8.68, 22.25, -2.55), (-6.85, 24.15, -2.12), "bone")
    g.box(b, "mid_front_lip", (-8.35, 22.02, -2.62), (-7.15, 22.55, -2.32), "bone_dk")
    g.box(b, "mid_front_scale", (-8.25, 23.15, -2.72), (-7.25, 23.85, -2.46), "bone_lt")
    # 后包角（z ≥ 2.10，y 起始 22.25 避开 22.35 共面）
    g.box(b, "mid_back_wrap", (-8.68, 22.25, 2.12), (-6.85, 24.15, 2.55), "bone_dk")
    g.box(b, "mid_back_lip", (-8.35, 22.02, 2.32), (-7.15, 22.55, 2.62), "bone_stain")
    g.box(b, "mid_back_scale", (-8.25, 23.15, 2.46), (-7.25, 23.85, 2.72), "bone_stain")
    # 边缘厚度加强凸棱
    g.box(b, "mid_rim_reinforce", (-8.85, 22.15, -2.10), (-8.45, 22.55, 2.10), "bone_stain")


def part_plate_low(g: Gear) -> None:
    """底层护臂骨片：垂挂在外侧上臂的第三层阶梯式弧形坚骨。"""
    b = "plate_low"
    # 第三层护臂板：x ≤ -8.10，y∈[19.35, 22.25]
    g.box(b, "low_outer_plate", (-8.65, 19.35, -2.05), (-8.12, 22.25, 2.05), "bone")
    g.box(b, "low_outer_crest", (-8.88, 19.85, -1.45), (-8.55, 21.85, 1.45), "bone_lt")
    # 外侧纵向加固条肋
    g.box(b, "low_rib_f", (-8.78, 19.55, -1.82), (-8.52, 22.05, -1.42), "bone_stain")
    g.box(b, "low_rib_b", (-8.78, 19.55, 1.42), (-8.52, 22.05, 1.82), "bone_stain")
    # 前下角弧面（z ≤ -2.10）
    g.box(b, "low_front_wrap", (-8.55, 19.45, -2.48), (-7.45, 22.15, -2.10), "bone")
    g.box(b, "low_front_boss", (-8.35, 20.25, -2.58), (-7.65, 21.25, -2.35), "bone_lt")
    # 后下角弧面（z ≥ 2.10）
    g.box(b, "low_back_wrap", (-8.55, 19.45, 2.10), (-7.45, 22.15, 2.48), "bone_dk")
    g.box(b, "low_back_boss", (-8.35, 20.25, 2.35), (-7.65, 21.25, 2.58), "bone_stain")
    # 底端锯齿状残缺收尾（五个骨茬齿）
    g.box(b, "low_tooth1", (-8.52, 18.75, -1.95), (-8.18, 19.35, -1.45), "bone_lt")
    g.box(b, "low_tooth2", (-8.62, 18.55, -1.35), (-8.18, 19.35, -0.75), "bone")
    g.box(b, "low_tooth3", (-8.65, 18.42, -0.65), (-8.18, 19.35, 0.65), "bone_stain")
    g.box(b, "low_tooth4", (-8.62, 18.55, 0.75), (-8.18, 19.35, 1.35), "bone")
    g.box(b, "low_tooth5", (-8.52, 18.75, 1.45), (-8.18, 19.35, 1.95), "bone_dk")
    # 底板上的横向穿孔暗孔（供下方系挂兽牙）
    g.box(b, "low_hole_void1", (-8.68, 19.48, -1.68), (-8.40, 19.78, -1.38), "void")
    g.box(b, "low_hole_void2", (-8.68, 19.48, -0.95), (-8.40, 19.78, -0.65), "void")
    g.box(b, "low_hole_void3", (-8.68, 19.48, -0.15), (-8.40, 19.78, 0.15), "void")
    g.box(b, "low_hole_void4", (-8.68, 19.48, 0.65), (-8.40, 19.78, 0.95), "void")
    g.box(b, "low_hole_void5", (-8.68, 19.48, 1.38), (-8.40, 19.78, 1.68), "void")


def part_bone_ridge(g: Gear) -> None:
    """肩峰骨刺脊：断裂异兽突角骨脊、骨刺粗节、骨裂加固栓。"""
    b = "bone_ridge"
    g.box(b, "spine_base", (-7.10, 25.55, -1.15), (-5.45, 26.25, 1.15), "bone_stain")
    g.box(b, "spine_mid", (-6.95, 26.25, -0.85), (-5.65, 26.85, 0.85), "bone")
    g.box(b, "spine_horn_tip", (-6.82, 26.85, -0.55), (-5.85, 27.35, 0.55), "bone_lt")
    # 骨角断裂面破裂骨髓暗槽（厚度 ≥ 0.25）
    g.box(b, "spine_fracture_void", (-6.55, 27.18, -0.32), (-6.05, 27.45, 0.32), "void")
    # 刺根部的加固打孔小横栓
    g.box(b, "spine_peg_pin", (-7.25, 25.75, -0.22), (-5.30, 26.05, 0.22), "bone_dk")
    g.box(b, "spine_peg_knot", (-7.45, 25.65, -0.35), (-7.15, 26.15, 0.35), "cord")


def part_lash_cords(g: Gear) -> None:
    """穿骨孔草绳：各层骨甲穿孔扎结草绳、前后十字锁线、悬垂绳结。"""
    b = "lash_cords"
    # 前侧 X 交叉绑线（z ≤ -2.25）
    g.box(b, "cord_x_front1", (-7.65, 23.65, -2.68), (-6.35, 24.25, -2.42), "cord")
    g.box(b, "cord_x_front2", (-7.65, 24.25, -2.68), (-6.35, 24.88, -2.42), "cord_dk")
    g.box(b, "cord_front_knot", (-7.15, 23.92, -2.82), (-6.75, 24.55, -2.48), "cord")
    # 后侧加固横绑线（z ≥ 2.25）
    g.box(b, "cord_band_back1", (-7.75, 23.75, 2.42), (-6.25, 24.25, 2.68), "cord_dk")
    g.box(b, "cord_band_back2", (-7.75, 24.45, 2.42), (-6.25, 24.95, 2.68), "cord")
    g.box(b, "cord_back_knot", (-7.15, 24.15, 2.48), (-6.75, 24.75, 2.82), "cord")

    # 连接中板与底板的外侧粗草绳箍（y 起始 22.02 避开 22.05 共面）
    g.box(b, "cord_loop_outer", (-9.05, 22.02, -1.35), (-8.72, 22.65, 1.35), "cord")
    g.box(b, "cord_knot_outer", (-9.18, 22.08, -0.35), (-8.82, 22.75, 0.35), "cord_dk")
    g.box(b, "cord_dangle1", (-9.08, 21.15, -0.25), (-8.80, 22.15, 0.15), "cord")
    g.box(b, "cord_dangle2", (-9.05, 20.35, -0.15), (-8.82, 21.25, 0.20), "cord_dk")


def part_chest_anchor(g: Gear) -> None:
    """胸前固定骨扣：胸骨正中的兽骨定位圆盘与皮垫（中线对称件）。"""
    b = "chest_anchor"
    # 胸前中线上的定位皮衬垫与兽骨扣（x 严格对称于 0.0）
    g.box(b, "chest_disc_pad", (-0.80, 17.85, -2.52), (0.80, 19.15, -2.20), "hide_dk")
    g.box(b, "chest_disc_bone", (-0.60, 18.05, -2.68), (0.60, 18.95, -2.36), "bone")
    g.box(b, "chest_disc_boss", (-0.30, 18.25, -2.82), (0.30, 18.75, -2.52), "bone_lt")
    g.box(b, "chest_disc_rivet", (-0.15, 18.35, -2.95), (0.15, 18.65, -2.65), "iron_lt")


def part_strap_front(g: Gear) -> None:
    """胸前斜挎皮带：从左肩斜下横跨胸膛到右腰前部、缝线铆钉。"""
    b = "strap_front"
    # 注意：在 y ≥ 23.90 时，x 必须严格 ≤ -4.10 避开头部！
    # 一旦 y ≤ 23.85，头部不存在，可以内倾跨向胸膛中央！
    # 各段之间 Z 范围轻微错开 0.03px，防止两端共面冲突
    # 肩峰端部垂直下垂段（x ≤ -4.15，高至 24.50 错开垫皮）
    g.box(b, "strap_f_drop", (-4.85, 23.85, -2.48), (-4.11, 24.50, -2.07), "strap")
    # 胸前斜下各段（y ≤ 23.85，z 严格 ≤ -2.10）
    g.box(b, "strap_f_seg1", (-4.85, 22.60, -2.36), (-3.25, 23.85, -2.08), "strap")
    g.box(b, "strap_f_seg2", (-3.55, 20.20, -2.42), (-1.85, 22.80, -2.12), "strap_dk")
    g.box(b, "strap_f_seg3", (-2.15, 17.60, -2.37), (-0.45, 20.40, -2.09), "strap")
    g.box(b, "strap_f_seg4", (-0.75, 15.00, -2.43), (0.95, 17.80, -2.13), "strap_dk")
    g.box(b, "strap_f_seg5", (0.65, 12.40, -2.38), (2.45, 15.20, -2.10), "strap")
    g.box(b, "strap_f_seg6", (2.15, 11.25, -2.42), (3.85, 12.80, -2.12), "strap_dk")

    # 皮带胸口正中央加厚垫片与铁铆钉
    g.box(b, "strap_f_pad", (-1.85, 17.45, -2.50), (-0.75, 19.85, -2.18), "hide_dk")
    g.box(b, "strap_f_rivet1", (-1.55, 19.05, -2.65), (-1.15, 19.45, -2.35), "iron_lt")
    g.box(b, "strap_f_rivet2", (-1.35, 17.80, -2.65), (-0.95, 18.25, -2.35), "iron_lt")

    # 沿带身的粗针脚缝线
    g.box(b, "strap_f_stitch1", (-3.25, 21.65, -2.48), (-2.85, 22.25, -2.22), "cord")
    g.box(b, "strap_f_stitch2", (0.05, 16.25, -2.48), (0.45, 16.85, -2.22), "cord")


def part_strap_back(g: Gear) -> None:
    """背后斜跨皮带：从左肩斜下横跨后背到右腰后部、加强护垫。"""
    b = "strap_back"
    # 在 y ≥ 23.90 时，x 必须严格 ≤ -4.10 避开头部！
    # 肩峰端部垂直下垂段（x ≤ -4.15，高至 24.50 错开垫皮）
    g.box(b, "strap_b_drop", (-4.85, 23.85, 2.07), (-4.11, 24.50, 2.48), "strap")
    # 后背斜下各段（y ≤ 23.85，z 严格 ≥ 2.10）
    g.box(b, "strap_b_seg1", (-4.85, 22.60, 2.08), (-3.25, 23.85, 2.36), "strap")
    g.box(b, "strap_b_seg2", (-3.55, 20.20, 2.12), (-1.85, 22.80, 2.42), "strap_dk")
    g.box(b, "strap_b_seg3", (-2.15, 17.60, 2.09), (-0.45, 20.40, 2.37), "strap")
    g.box(b, "strap_b_seg4", (-0.75, 15.00, 2.13), (0.95, 17.80, 2.43), "strap_dk")
    g.box(b, "strap_b_seg5", (0.65, 12.40, 2.10), (2.45, 15.20, 2.38), "strap")
    g.box(b, "strap_b_seg6", (2.15, 11.25, 2.12), (3.85, 12.80, 2.42), "strap_dk")

    # 背后交叉抗拉力强化宽皮垫
    g.box(b, "strap_b_pad", (-1.85, 17.45, 2.18), (-0.75, 19.85, 2.50), "hide")
    g.box(b, "strap_b_rivet1", (-1.55, 19.05, 2.35), (-1.15, 19.45, 2.65), "iron_lt")
    g.box(b, "strap_b_rivet2", (-1.35, 17.80, 2.35), (-0.95, 18.25, 2.65), "iron_lt")


def part_strap_buckle(g: Gear) -> None:
    """右腰带扣总成：下部环腰皮带、生铁日字带扣、咬合舌片。"""
    b = "strap_buckle"
    # 右侧腰带交汇处位于 y∈[11.25, 11.90]（低于右臂盒 y=12.0），x ≥ 4.10（外于右腿盒 x=4.0）
    g.box(b, "waist_side_strap", (4.10, 11.25, -2.15), (4.38, 11.90, 2.15), "strap")
    g.box(b, "waist_side_reinforce", (4.20, 11.35, -1.65), (4.45, 11.80, 1.65), "strap_dk")

    # 右腰前方的粗生铁日字扣
    g.box(b, "buckle_frame_t", (3.25, 11.82, -2.62), (4.35, 12.12, -2.32), "iron")
    g.box(b, "buckle_frame_b", (3.25, 11.02, -2.62), (4.35, 11.32, -2.32), "iron")
    g.box(b, "buckle_frame_l", (3.25, 11.32, -2.62), (3.55, 11.82, -2.32), "iron_lt")
    g.box(b, "buckle_frame_r", (4.05, 11.32, -2.62), (4.35, 11.82, -2.32), "iron_lt")
    g.box(b, "buckle_frame_mid", (3.70, 11.28, -2.58), (3.92, 11.88, -2.30), "rust")

    # 穿扣咬合销舌
    g.box(b, "buckle_tongue_pin", (3.68, 11.45, -2.82), (3.94, 11.75, -2.48), "rust")


def part_hang_teeth(g: Gear) -> None:
    """骨甲悬挂兽牙：骨板下缘皮绳悬垂的五枚异兽尖牙串（独立可动骨）。"""
    b = "hang_teeth"
    # 五枚异兽尖牙串，沿下缘弧形分布
    teeth_data = [
        ("tooth1", -1.55, 18.55, 17.65, 16.85, 16.25, "bone_dk"),
        ("tooth2", -0.80, 18.35, 17.45, 16.65, 15.95, "bone"),
        ("tooth3", 0.00, 18.15, 17.15, 16.05, 15.25, "bone_lt"),
        ("tooth4", 0.80, 18.35, 17.45, 16.65, 15.95, "bone"),
        ("tooth5", 1.55, 18.55, 17.65, 16.85, 16.25, "bone_dk"),
    ]
    for tag, z_c, y_cord, y_root, y_body, y_tip, mat_body in teeth_data:
        g.box(b, f"cord_{tag}", (-8.58, y_cord, z_c - 0.12), (-8.34, 19.55, z_c + 0.12), "cord_dk")
        g.box(b, f"knot_{tag}", (-8.68, y_cord - 0.25, z_c - 0.20), (-8.28, y_cord + 0.12, z_c + 0.20), "cord")
        g.box(b, f"{tag}_root", (-8.66, y_root, z_c - 0.18), (-8.30, y_cord - 0.15, z_c + 0.18), "bone_stain")
        g.box(b, f"{tag}_body", (-8.64, y_body, z_c - 0.15), (-8.32, y_root + 0.12, z_c + 0.15), mat_body)
        g.box(b, f"{tag}_tip", (-8.60, y_tip, z_c - 0.12), (-8.36, y_body + 0.10, z_c + 0.12), "bone_lt")


def part_hang_strap_tail(g: Gear) -> None:
    """右腰皮带垂尾：穿出扣环自然下垂的备用打孔皮带头与兽骨坠（独立可动骨）。"""
    b = "hang_strap_tail"
    # 上段皮舌
    g.box(b, "tail_strap_top", (3.48, 10.35, -2.52), (3.98, 11.25, -2.25), "strap")
    # 下段打孔皮舌（Z 错开 0.03）
    g.box(b, "tail_strap_bot", (3.42, 9.15, -2.56), (3.92, 10.45, -2.28), "strap_dk")
    # 皮舌剪角尖端
    g.box(b, "tail_strap_tip", (3.48, 8.45, -2.52), (3.88, 9.25, -2.25), "strap")
    # 调节穿孔（嵌在下段皮舌内部）
    g.box(b, "tail_hole_void1", (3.56, 9.85, -2.62), (3.82, 10.18, -2.22), "void")
    g.box(b, "tail_hole_void2", (3.54, 9.25, -2.62), (3.80, 9.58, -2.22), "void")

    # 皮舌尾端绑着的细骨饰碎片（随身体晃动）
    g.box(b, "tail_bone_cord", (3.54, 7.85, -2.50), (3.82, 8.55, -2.22), "cord")
    g.box(b, "tail_bone_amulet", (3.45, 6.75, -2.55), (3.92, 7.95, -2.18), "bone")
    g.box(b, "tail_bone_tip", (3.52, 6.25, -2.50), (3.85, 6.85, -2.22), "bone_dk")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "pad": part_pad,
    "plate_top": part_plate_top,
    "plate_mid": part_plate_mid,
    "plate_low": part_plate_low,
    "bone_ridge": part_bone_ridge,
    "lash_cords": part_lash_cords,
    "chest_anchor": part_chest_anchor,
    "strap_front": part_strap_front,
    "strap_back": part_strap_back,
    "strap_buckle": part_strap_buckle,
    "hang_teeth": part_hang_teeth,
    "hang_strap_tail": part_hang_strap_tail,
}

SPEC = GearSpec(
    gear_id="gu_pian_jian_jia",
    model_name="GuPianJianJia",
    title="骨片肩甲 / gu_pian_jian_jia",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：单肩（左） · 叠层兽骨肩甲+斜挎固定皮带+下垂兽牙",
    textured=False,            # Round 1 纯几何
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
