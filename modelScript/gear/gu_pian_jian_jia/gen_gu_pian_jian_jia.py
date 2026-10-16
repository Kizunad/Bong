#!/usr/bin/env python3
"""骨片肩甲（gu_pian_jian_jia）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工重构。

任务卡：末土装备 #13（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，
左臂在 -x 侧（x∈[-8,-4]）。

调度审返工要求：
- 肩甲本体必须覆盖整个左肩、垂到上臂一半，四层叠压骨片，每层多片、具厚度、边缘参差、裂纹和绑扎草绳；
- 包住肩头（盖住手臂盒 x∈[-8,-4] 的上沿并留摆动间隙），兽牙串加长，方块做到 250 以上；
- 严禁穿进玩家身体（头盒、躯干、双臂），门禁与差分自证全绿。

骨骼数 ≥ 10，挂件独立骨，全绿门禁。
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
    "hide_lt":    (152, 108, 78),     # 磨损毛边皮
    "strap":      (112, 72, 50),      # 斜挎鞣制皮带
    "strap_dk":   (78, 48, 36),       # 磨损厚皮
    "cord":       (144, 128, 78),     # 扎结麻绳 / 草绳
    "cord_dk":    (102, 88, 56),      # 脏草绳深色
    "iron":       (82, 78, 76),       # 粗锻生铁带扣
    "iron_lt":    (116, 112, 110),    # 铁光泽 / 铆钉
    "rust":       (152, 86, 52),      # 铁扣红锈
    "void":       (28, 24, 22),       # 穿孔暗影
}

# 软件材质与硬质骨铁允许贴合咬合
SOFT = ("hide", "hide_dk", "hide_lt", "strap", "strap_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("bone", "bone_dk", "bone_stain", "strap"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("chest_anchor", "root", (0.0, 18.5, -2.45), "胸前固定骨扣：胸骨正中的兽骨定位圆盘与皮垫（中线对称件）"),
    ("pauldron_pad", "root", (-6.0, 24.5, 0.0), "左肩底衬垫：多层粗厚兽皮衬层、护肩皮坎、参差毛边下摆"),
    ("plate_top", "root", (-6.2, 25.5, 0.0), "第1层主肩胛骨板：巨大厚重拱形异兽肩胛骨、肩峰隆起、顶侧包边"),
    ("plate_upper", "root", (-7.2, 24.2, 0.0), "第2层叠压副骨片：顺肩斜向外展开的多片厚骨瓦片、前覆后压"),
    ("plate_mid", "root", (-8.2, 22.4, 0.0), "第3层外侧肩护板：包裹上臂外侧的弧形坚骨板、侧骨翼、防劈骨棱"),
    ("plate_low", "root", (-8.4, 18.5, 0.0), "第4层深垂护臂板：垂至上臂中部的巨大锯齿兽骨板、肋突骨条"),
    ("bone_ridge", "plate_top", (-6.0, 26.5, 0.0), "肩峰多叉骨刺脊：断裂异兽角脊、前后分叉骨刺、骨裂加固木栓"),
    ("lash_cords", "root", (-6.8, 23.5, 0.0), "穿骨孔草绳网络：各层骨甲穿孔扎结草绳、前后双十字锁线、垂落散麻丝"),
    ("strap_front", "root", (-1.0, 18.5, -2.25), "胸前斜挎皮带：从左肩斜下横跨胸膛到右腰前部、双排针脚与铆钉"),
    ("strap_back", "root", (-1.0, 18.5, 2.25), "背后斜跨皮带：从左肩斜下横跨后背到右腰后部、加强护垫与缝线"),
    ("strap_buckle", "root", (4.2, 11.5, 0.0), "右腰带扣总成：下部环腰皮带、生铁日字带扣、咬合双舌片"),
    ("hang_teeth", "plate_low", (-8.35, 16.5, -0.2), "骨甲悬挂长兽牙串：骨板下缘悬垂的7枚交错粗长异兽尖牙（独立可动骨）"),
    ("hang_strap_tail", "strap_buckle", (4.25, 9.8, -0.4), "右腰皮带垂尾：穿出扣环自然下垂的备用打孔皮带头与兽骨坠（独立可动骨）"),
]

# 单肩甲除胸骨中心定位件外整体不对称，其它骨骼进入 asym 白名单
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "chest_anchor"))


# ───────────────────────────────────────────────────────────── 几何部件构建
def part_pad(g: Gear) -> None:
    """左肩底衬垫：多层粗厚兽皮衬层、护肩皮坎、参差毛边下摆。"""
    b = "pauldron_pad"
    # 肩顶皮衬垫：骑在左肩 y ≥ 24.10，x∈[-8.50, -4.12]，z∈[-2.65, 2.65]
    g.box(b, "pad_top_main", (-8.48, 24.08, -2.55), (-4.12, 24.45, 2.55), "hide_dk")
    g.box(b, "pad_top_layer2", (-8.25, 24.48, -2.32), (-4.18, 24.75, 2.32), "hide")
    g.box(b, "pad_top_lip_in", (-4.35, 24.45, -2.25), (-4.10, 24.95, 2.25), "hide_lt")

    # 前胸与后背过渡护皮片（前 z ≤ -2.10，后 z ≥ 2.10）
    g.box(b, "pad_front_flap", (-8.05, 22.45, -2.44), (-4.15, 24.10, -2.10), "hide")
    g.box(b, "pad_front_fringe1", (-7.85, 21.65, -2.44), (-6.45, 22.45, -2.18), "hide_dk")
    g.box(b, "pad_front_fringe2", (-6.25, 21.85, -2.44), (-4.85, 22.45, -2.18), "hide_lt")

    g.box(b, "pad_back_flap", (-8.05, 22.45, 2.10), (-4.15, 24.10, 2.44), "hide_dk")
    g.box(b, "pad_back_fringe1", (-7.85, 21.65, 2.18), (-6.45, 22.45, 2.44), "hide")
    g.box(b, "pad_back_fringe2", (-6.25, 21.85, 2.18), (-4.85, 22.45, 2.44), "hide_lt")

    # 外侧下垂包边垫皮（x ≤ -8.14，覆盖上臂顶侧）
    g.box(b, "pad_arm_outer", (-8.55, 20.85, -2.45), (-8.18, 24.10, 2.45), "hide_dk")
    g.box(b, "pad_arm_fringe", (-8.58, 20.12, -2.15), (-8.20, 20.82, 2.15), "hide_lt")

    # 衬垫上的粗缝线
    g.box(b, "pad_stitch_f1", (-7.60, 22.80, -2.58), (-6.20, 23.10, -2.32), "cord_dk")
    g.box(b, "pad_stitch_f2", (-5.90, 22.80, -2.58), (-4.50, 23.10, -2.32), "cord_dk")
    g.box(b, "pad_stitch_b1", (-7.60, 22.80, 2.32), (-6.20, 23.10, 2.58), "cord_dk")
    g.box(b, "pad_stitch_b2", (-5.90, 22.80, 2.32), (-4.50, 23.10, 2.58), "cord_dk")


def part_plate_top(g: Gear) -> None:
    """第1层主肩胛骨板：巨大厚重拱形异兽肩胛骨、肩峰隆起、顶侧包边。"""
    b = "plate_top"
    # 顶部主拱骨板：y∈[24.50, 25.85]，x∈[-8.60, -4.15]，z∈[-2.75, 2.75]
    # 中轴大拱弧主干（厚重坚韧异兽老骨）
    g.box(b, "top_arch_core", (-8.55, 24.50, -2.45), (-4.22, 25.40, 2.45), "bone")
    g.box(b, "top_arch_ridge", (-8.15, 25.40, -1.95), (-4.45, 25.95, 1.95), "bone_lt")
    g.box(b, "top_arch_crest", (-7.62, 25.95, -1.38), (-4.85, 26.42, 1.38), "bone_lt")

    # 前倾弧面与后倾弧面（大面积倾斜护甲骨檐）
    g.box(b, "top_front_slope1", (-8.45, 23.95, -2.75), (-4.35, 24.82, -2.32), "bone")
    g.box(b, "top_front_slope2", (-8.15, 24.65, -2.85), (-4.55, 25.32, -2.45), "bone_lt")
    g.box(b, "top_back_slope1", (-8.45, 23.95, 2.32), (-4.35, 24.82, 2.75), "bone_dk")
    g.box(b, "top_back_slope2", (-8.15, 24.65, 2.45), (-4.55, 25.32, 2.85), "bone_stain")

    # 内侧贴颈加厚防砍骨坎（高高耸起，保护颈部）
    g.box(b, "top_neck_guard1", (-4.65, 25.18, -2.25), (-4.18, 26.45, 2.25), "bone_stain")
    g.box(b, "top_neck_guard2", (-4.52, 26.45, -1.75), (-4.20, 27.05, 1.75), "bone_lt")
    g.box(b, "top_neck_spike", (-4.45, 27.05, -0.75), (-4.22, 27.55, 0.75), "bone_lt")

    # 前后肩角翘起的尖刺状骨檐
    g.box(b, "top_corner_f1", (-8.45, 24.85, -2.95), (-7.45, 25.65, -2.55), "bone_lt")
    g.box(b, "top_corner_f2", (-8.15, 25.45, -3.12), (-7.55, 26.15, -2.75), "bone_lt")
    g.box(b, "top_corner_b1", (-8.45, 24.85, 2.55), (-7.45, 25.65, 2.95), "bone_stain")
    g.box(b, "top_corner_b2", (-8.15, 25.45, 2.75), (-7.55, 26.15, 3.12), "bone_stain")

    # 顶板层叠多片细碎骨鳞瓦片
    g.box(b, "top_scale_f1", (-7.60, 24.72, -2.62), (-6.15, 25.12, -2.25), "bone_lt")
    g.box(b, "top_scale_f2", (-6.05, 24.80, -2.55), (-4.75, 25.18, -2.22), "bone")
    g.box(b, "top_scale_b1", (-7.60, 24.72, 2.25), (-6.15, 25.12, 2.62), "bone_dk")
    g.box(b, "top_scale_b2", (-6.05, 24.80, 2.22), (-4.70, 25.18, 2.55), "bone_stain")

    # 骨板上的风化斑块与深层骨裂槽
    g.box(b, "top_stain_spot1", (-7.25, 25.26, -1.25), (-6.05, 25.52, 0.45), "bone_stain")
    g.box(b, "top_stain_spot2", (-5.85, 24.95, 1.35), (-4.75, 25.32, 2.25), "bone_stain")
    g.box(b, "top_crack_void", (-6.85, 25.42, -0.65), (-6.35, 25.75, -0.35), "void")


def part_plate_upper(g: Gear) -> None:
    """第2层叠压副骨片：顺肩斜向外展开的多片厚骨瓦片、前覆后压。"""
    b = "plate_upper"
    # 第2层骨瓦片顺着肩头向外滑下：y∈[23.10, 25.20]，x∈[-8.75, -5.50]
    # 外侧主瓦片（在左臂盒 y∈[12,24], x∈[-8,-4] 上方或外侧）
    g.box(b, "up_outer_plate", (-8.75, 24.12, -2.35), (-8.12, 24.85, 2.35), "bone")
    g.box(b, "up_outer_ridge", (-8.95, 24.18, -1.75), (-8.62, 24.65, 1.75), "bone_lt")
    g.box(b, "up_outer_flange", (-8.85, 24.52, -1.25), (-8.15, 25.15, 1.25), "bone_stain")

    # 前侧叠压骨片（z ≤ -2.10）
    g.box(b, "up_front_wrap1", (-8.68, 23.15, -2.62), (-6.82, 24.65, -2.25), "bone")
    g.box(b, "up_front_wrap2", (-8.25, 22.85, -2.75), (-7.15, 23.85, -2.45), "bone_lt")
    g.box(b, "up_front_tip", (-8.55, 22.55, -2.82), (-7.65, 23.25, -2.55), "bone_lt")

    # 后侧叠压骨片（z ≥ 2.10）
    g.box(b, "up_back_wrap1", (-8.68, 23.15, 2.25), (-6.82, 24.65, 2.62), "bone_dk")
    g.box(b, "up_back_wrap2", (-8.25, 22.85, 2.45), (-7.15, 23.85, 2.75), "bone_stain")
    g.box(b, "up_back_tip", (-8.55, 22.55, 2.55), (-7.65, 23.25, 2.82), "bone_stain")

    # 层间加固木/骨销
    g.box(b, "up_pin_front", (-7.45, 23.85, -2.92), (-7.05, 24.25, -2.52), "bone_dk")
    g.box(b, "up_pin_back", (-7.45, 23.85, 2.52), (-7.05, 24.25, 2.92), "bone_dk")


def part_plate_mid(g: Gear) -> None:
    """第3层外侧肩护板：包裹上臂外侧的弧形坚骨板、侧骨翼、防劈骨棱。"""
    b = "plate_mid"
    # 外侧主体大骨板（x ≤ -8.10，y∈[21.20, 23.85]，z∈[-2.45, 2.45]）
    g.box(b, "mid_outer_plate", (-8.85, 21.35, -2.35), (-8.12, 23.65, 2.35), "bone")
    g.box(b, "mid_outer_ridge", (-9.12, 21.85, -1.85), (-8.65, 23.25, 1.85), "bone_lt")
    g.box(b, "mid_outer_boss", (-9.25, 22.25, -0.95), (-8.85, 22.95, 0.95), "bone_lt")

    # 纵向双加固肋
    g.box(b, "mid_flange_f", (-8.98, 21.65, -1.75), (-8.72, 23.35, -1.15), "bone_stain")
    g.box(b, "mid_flange_b", (-8.98, 21.65, 1.15), (-8.72, 23.35, 1.75), "bone_stain")

    # 前包角大弧面（z ≤ -2.10）
    g.box(b, "mid_front_wrap", (-8.72, 21.25, -2.65), (-6.88, 23.45, -2.15), "bone")
    g.box(b, "mid_front_scale1", (-8.45, 21.85, -2.78), (-7.35, 22.95, -2.42), "bone_lt")
    g.box(b, "mid_front_scale2", (-8.25, 20.95, -2.72), (-7.45, 21.85, -2.38), "bone_dk")

    # 后包角大弧面（z ≥ 2.10）
    g.box(b, "mid_back_wrap", (-8.72, 21.25, 2.15), (-6.88, 23.45, 2.65), "bone_dk")
    g.box(b, "mid_back_scale1", (-8.45, 21.85, 2.42), (-7.35, 22.95, 2.78), "bone_stain")
    g.box(b, "mid_back_scale2", (-8.25, 20.95, 2.38), (-7.45, 21.85, 2.72), "bone_dk")

    # 边缘厚度咬合棱
    g.box(b, "mid_rim_band", (-8.95, 21.05, -2.18), (-8.45, 21.52, 2.18), "bone_stain")


def part_plate_low(g: Gear) -> None:
    """第4层深垂护臂板：垂至上臂中部的巨大锯齿兽骨板、肋突骨条。"""
    b = "plate_low"
    # 第4层深垂骨甲板：x ≤ -8.10，y 从 16.50 一路延伸至 21.80！覆盖上臂大部！
    # 外侧主深垂板与纵向双加固槽
    g.box(b, "low_outer_plate1", (-8.75, 17.65, -2.22), (-8.14, 21.80, 2.22), "bone")
    g.box(b, "low_outer_plate2", (-8.65, 16.48, -1.95), (-8.16, 17.95, 1.95), "bone_stain")
    g.box(b, "low_outer_crest", (-8.98, 17.85, -1.55), (-8.55, 21.25, 1.55), "bone_lt")
    g.box(b, "low_crest_boss", (-9.15, 18.85, -0.75), (-8.80, 20.25, 0.75), "bone_lt")
    # 增加细部骨钉
    g.box(b, "low_rivet_f1", (-8.85, 20.65, -1.95), (-8.50, 21.05, -1.65), "bone_lt")
    g.box(b, "low_rivet_b1", (-8.85, 20.65, 1.65), (-8.50, 21.05, 1.95), "bone_lt")
    g.box(b, "low_rivet_f2", (-8.82, 19.25, -1.95), (-8.54, 19.65, -1.65), "bone_lt")
    g.box(b, "low_rivet_b2", (-8.82, 19.25, 1.65), (-8.54, 19.65, 1.95), "bone_lt")
    g.box(b, "low_plate_lip_t", (-8.88, 21.55, -1.78), (-8.48, 21.95, 1.78), "bone_stain")

    # 外侧两侧骨棱条（仿兽肋骨）
    g.box(b, "low_rib_f1", (-8.88, 17.88, -1.85), (-8.52, 21.45, -1.35), "bone_stain")
    g.box(b, "low_rib_f2", (-8.82, 16.88, -1.65), (-8.54, 18.25, -1.15), "bone")
    g.box(b, "low_rib_b1", (-8.88, 17.88, 1.35), (-8.52, 21.45, 1.85), "bone_stain")
    g.box(b, "low_rib_b2", (-8.82, 16.88, 1.15), (-8.54, 18.25, 1.65), "bone_dk")
    # 增加额外加固副肋条
    g.box(b, "low_rib_f3", (-8.78, 18.45, -2.12), (-8.56, 20.85, -1.82), "bone")
    g.box(b, "low_rib_b3", (-8.78, 18.45, 1.82), (-8.56, 20.85, 2.12), "bone")
    g.box(b, "low_flange_bot", (-8.70, 17.25, -1.75), (-8.32, 17.65, 1.75), "bone_stain")

    # 前下角弧面（z ≤ -2.10）
    g.box(b, "low_front_wrap", (-8.65, 17.25, -2.55), (-7.35, 21.42, -2.12), "bone")
    g.box(b, "low_front_scale", (-8.45, 16.65, -2.62), (-7.55, 17.85, -2.28), "bone_lt")
    g.box(b, "low_front_flange", (-8.52, 18.15, -2.72), (-7.82, 19.85, -2.42), "bone_stain")

    # 后下角弧面（z ≥ 2.10）
    g.box(b, "low_back_wrap", (-8.65, 17.25, 2.12), (-7.35, 21.42, 2.55), "bone_dk")
    g.box(b, "low_back_scale", (-8.45, 16.65, 2.28), (-7.55, 17.85, 2.62), "bone_stain")
    g.box(b, "low_back_flange", (-8.52, 18.15, 2.42), (-7.82, 19.85, 2.72), "bone_dk")

    # 底端参差错落的锯齿骨茬下摆（7 个骨茬尖）
    g.box(b, "low_tooth1", (-8.58, 15.85, -2.05), (-8.20, 16.65, -1.55), "bone_lt")
    g.box(b, "low_tooth2", (-8.62, 15.55, -1.55), (-8.20, 16.65, -0.95), "bone")
    g.box(b, "low_tooth3", (-8.72, 15.25, -0.95), (-8.20, 16.65, -0.35), "bone_stain")
    g.box(b, "low_tooth4", (-8.78, 14.85, -0.35), (-8.20, 16.65, 0.35), "bone_lt")
    g.box(b, "low_tooth5", (-8.72, 15.25, 0.35), (-8.20, 16.65, 0.95), "bone_stain")
    g.box(b, "low_tooth6", (-8.62, 15.55, 0.95), (-8.20, 16.65, 1.55), "bone")
    g.box(b, "low_tooth7", (-8.58, 15.85, 1.55), (-8.20, 16.65, 2.05), "bone_dk")

    # 挂兽牙穿孔暗影
    for idx, zc in enumerate((-1.70, -1.15, -0.60, 0.0, 0.60, 1.15, 1.70), start=1):
        g.box(b, f"low_hole_{idx}", (-8.72, 16.90, zc - 0.14), (-8.44, 17.18, zc + 0.14), "void")


def part_bone_ridge(g: Gear) -> None:
    """肩峰多叉骨刺脊：断裂异兽角脊、前后分叉骨刺、骨裂加固木栓。"""
    b = "bone_ridge"
    # 中央耸立主骨刺（y∈[25.55, 28.25]，x∈[-7.35, -5.15]）
    g.box(b, "spine_base", (-7.35, 25.55, -1.35), (-5.25, 26.45, 1.35), "bone_stain")
    g.box(b, "spine_mid", (-7.15, 26.45, -0.95), (-5.45, 27.35, 0.95), "bone")
    g.box(b, "spine_upper", (-6.95, 27.35, -0.65), (-5.65, 28.15, 0.65), "bone_lt")
    g.box(b, "spine_horn_tip", (-6.82, 28.15, -0.42), (-5.85, 28.75, 0.42), "bone_lt")
    g.box(b, "spine_fracture_void", (-6.55, 28.55, -0.28), (-6.05, 28.85, 0.28), "void")

    # 前向分叉小角刺（两段紧密相连）
    g.box(b, "spine_f_spike1", (-7.05, 26.25, -1.95), (-5.85, 26.95, -1.25), "bone_lt")
    g.box(b, "spine_f_spike2", (-6.95, 26.75, -2.45), (-5.95, 27.55, -1.75), "bone_lt")

    # 后向分叉小角刺（两段紧密相连）
    g.box(b, "spine_b_spike1", (-7.05, 26.25, 1.25), (-5.85, 26.95, 1.95), "bone_stain")
    g.box(b, "spine_b_spike2", (-6.95, 26.75, 1.75), (-5.95, 27.55, 2.45), "bone_stain")

    # 刺根部的加固打孔小横栓与固定骨楔
    g.box(b, "spine_peg_pin1", (-7.45, 25.85, -0.25), (-5.15, 26.15, 0.25), "bone_dk")
    g.box(b, "spine_peg_pin2", (-6.55, 26.75, -1.15), (-6.05, 27.05, 1.15), "bone_dk")
    g.box(b, "spine_peg_knot1", (-7.65, 25.75, -0.38), (-7.35, 26.25, 0.38), "cord")
    g.box(b, "spine_peg_knot2", (-5.25, 25.75, -0.38), (-4.95, 26.25, 0.38), "cord")


def part_lash_cords(g: Gear) -> None:
    """穿骨孔草绳网络：各层骨甲穿孔扎结草绳、前后双十字锁线、垂落散麻丝。"""
    b = "lash_cords"
    # 前胸侧四层骨板之间的复杂麻绳绑扎网络（z ≤ -2.25）
    # 第1组十字交叉（顶板与第2层之间）
    g.box(b, "cord_f_cross1_a", (-7.85, 24.15, -2.78), (-6.45, 24.75, -2.48), "cord")
    g.box(b, "cord_f_cross1_b", (-7.85, 24.75, -2.78), (-6.45, 25.35, -2.48), "cord_dk")
    g.box(b, "cord_f_knot1", (-7.25, 24.45, -2.92), (-6.85, 25.05, -2.52), "cord")

    # 第2组十字交叉（第2层与第3层之间）
    g.box(b, "cord_f_cross2_a", (-7.95, 22.45, -2.85), (-6.65, 23.05, -2.52), "cord_dk")
    g.box(b, "cord_f_cross2_b", (-7.95, 23.05, -2.85), (-6.65, 23.65, -2.52), "cord")
    g.box(b, "cord_f_knot2", (-7.35, 22.75, -2.98), (-6.95, 23.35, -2.58), "cord")

    # 第3组横绑带（第3层与第4层之间）
    g.box(b, "cord_f_band3", (-8.35, 20.85, -2.82), (-6.85, 21.35, -2.48), "cord")
    g.box(b, "cord_f_knot3", (-7.75, 20.65, -2.95), (-7.25, 21.45, -2.52), "cord_dk")
    g.box(b, "cord_f_dangle", (-7.65, 19.85, -2.88), (-7.35, 20.75, -2.58), "cord")

    # 后背侧双十字锁线与横向绑扎（z ≥ 2.25）
    g.box(b, "cord_b_cross1_a", (-7.85, 24.15, 2.48), (-6.45, 24.75, 2.78), "cord_dk")
    g.box(b, "cord_b_cross1_b", (-7.85, 24.75, 2.48), (-6.45, 25.35, 2.78), "cord")
    g.box(b, "cord_b_knot1", (-7.25, 24.45, 2.52), (-6.85, 25.05, 2.92), "cord")

    g.box(b, "cord_b_cross2_a", (-7.95, 22.45, 2.52), (-6.65, 23.05, 2.85), "cord")
    g.box(b, "cord_b_cross2_b", (-7.95, 23.05, 2.52), (-6.65, 23.65, 2.85), "cord_dk")
    g.box(b, "cord_b_knot2", (-7.35, 22.75, 2.58), (-6.95, 23.35, 2.98), "cord")

    g.box(b, "cord_b_band3", (-8.35, 20.85, 2.48), (-6.85, 21.35, 2.82), "cord_dk")

    # 外侧环绕大骨箍（x ≤ -9.00）
    g.box(b, "cord_loop_outer1", (-9.28, 23.15, -1.45), (-8.92, 23.75, 1.45), "cord")
    g.box(b, "cord_loop_outer2", (-9.22, 20.95, -1.35), (-8.92, 21.55, 1.35), "cord_dk")
    g.box(b, "cord_outer_knot", (-9.38, 22.15, -0.45), (-9.02, 22.85, 0.45), "cord")
    g.box(b, "cord_outer_tail1", (-9.28, 21.25, -0.32), (-9.04, 22.25, 0.18), "cord")
    g.box(b, "cord_outer_tail2", (-9.25, 20.45, -0.18), (-9.02, 21.35, 0.28), "cord_dk")


def part_chest_anchor(g: Gear) -> None:
    """胸前固定骨扣：胸骨正中的兽骨定位圆盘与皮垫（中线对称件）。"""
    b = "chest_anchor"
    g.box(b, "chest_disc_pad", (-0.80, 17.85, -2.52), (0.80, 19.15, -2.20), "hide_dk")
    g.box(b, "chest_disc_bone", (-0.60, 18.05, -2.68), (0.60, 18.95, -2.36), "bone")
    g.box(b, "chest_disc_boss", (-0.30, 18.25, -2.82), (0.30, 18.75, -2.52), "bone_lt")
    g.box(b, "chest_disc_rivet", (-0.15, 18.35, -2.95), (0.15, 18.65, -2.65), "iron_lt")


def part_strap_front(g: Gear) -> None:
    """胸前斜挎皮带：从左肩斜下横跨胸膛到右腰前部、双排针脚与铆钉。"""
    b = "strap_front"
    # 肩峰端部垂直下垂段（x ≤ -4.11，避开头部）
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

    # 沿带身的粗针脚缝线（密集排布）
    g.box(b, "strap_f_stitch1", (-3.25, 21.65, -2.48), (-2.85, 22.25, -2.22), "cord")
    g.box(b, "strap_f_stitch2", (-2.45, 19.85, -2.48), (-2.05, 20.45, -2.22), "cord")
    g.box(b, "strap_f_stitch3", (0.05, 16.25, -2.48), (0.45, 16.85, -2.22), "cord")
    g.box(b, "strap_f_stitch4", (1.25, 13.85, -2.48), (1.65, 14.45, -2.22), "cord")
    g.box(b, "strap_f_stitch5", (-1.25, 18.65, -2.52), (-0.85, 19.25, -2.24), "cord")
    g.box(b, "strap_f_stitch6", (1.85, 12.25, -2.52), (2.25, 12.85, -2.24), "cord")

    # 胸前挂附的小骨哨（斜挂在胸前皮带上）
    g.box(b, "strap_f_whistle_cord", (-2.25, 18.65, -2.62), (-1.85, 19.45, -2.32), "cord_dk")
    g.box(b, "strap_f_whistle_body", (-2.32, 17.55, -2.68), (-1.78, 18.75, -2.35), "bone_lt")
    g.box(b, "strap_f_whistle_void", (-2.18, 18.35, -2.75), (-1.92, 18.65, -2.28), "void")
    g.box(b, "strap_f_whistle_lip", (-2.28, 17.35, -2.65), (-1.82, 17.65, -2.38), "bone")


def part_strap_back(g: Gear) -> None:
    """背后斜跨皮带：从左肩斜下横跨后背到右腰后部、加强护垫与缝线。"""
    b = "strap_back"
    # 肩峰端部垂直下垂段（x ≤ -4.11，避开头部）
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

    # 背后缝线与挂坠
    g.box(b, "strap_b_stitch1", (-3.25, 21.65, 2.22), (-2.85, 22.25, 2.48), "cord_dk")
    g.box(b, "strap_b_stitch2", (0.05, 16.25, 2.22), (0.45, 16.85, 2.48), "cord_dk")
    g.box(b, "strap_b_stitch3", (-2.45, 19.85, 2.22), (-2.05, 20.45, 2.48), "cord_dk")
    g.box(b, "strap_b_stitch4", (1.25, 13.85, 2.22), (1.65, 14.45, 2.48), "cord_dk")
    g.box(b, "strap_b_stitch5", (-1.25, 18.65, 2.22), (-0.85, 19.25, 2.48), "cord_dk")
    g.box(b, "strap_b_stitch6", (1.85, 12.25, 2.22), (2.25, 12.85, 2.48), "cord_dk")

    # 背后交叉垫防磨双层外边
    g.box(b, "strap_b_pad_rim1", (-1.95, 17.35, 2.16), (-0.65, 17.65, 2.52), "hide_dk")
    g.box(b, "strap_b_pad_rim2", (-1.95, 19.65, 2.16), (-0.65, 19.95, 2.52), "hide_dk")


def part_strap_buckle(g: Gear) -> None:
    """右腰带扣总成：下部环腰皮带、生铁日字带扣、咬合双舌片。"""
    b = "strap_buckle"
    # 右侧腰带交汇处位于 y∈[11.25, 11.90]，x ≥ 4.10（外于右腿盒）
    g.box(b, "waist_side_strap", (4.10, 11.25, -2.15), (4.38, 11.90, 2.15), "strap")
    g.box(b, "waist_side_reinforce", (4.20, 11.35, -1.65), (4.45, 11.80, 1.65), "strap_dk")

    # 右腰生铁日字扣
    g.box(b, "buckle_frame_t", (3.25, 11.82, -2.62), (4.35, 12.12, -2.32), "iron")
    g.box(b, "buckle_frame_b", (3.25, 11.02, -2.62), (4.35, 11.32, -2.32), "iron")
    g.box(b, "buckle_frame_l", (3.25, 11.32, -2.62), (3.55, 11.82, -2.32), "iron_lt")
    g.box(b, "buckle_frame_r", (4.05, 11.32, -2.62), (4.35, 11.82, -2.32), "iron_lt")
    g.box(b, "buckle_frame_mid", (3.70, 11.28, -2.58), (3.92, 11.88, -2.30), "rust")

    # 咬合双针
    g.box(b, "buckle_pin1", (3.55, 11.45, -2.82), (3.78, 11.75, -2.48), "rust")
    g.box(b, "buckle_pin2", (3.84, 11.45, -2.82), (4.04, 11.75, -2.48), "rust")


def part_hang_teeth(g: Gear) -> None:
    """骨甲悬挂长兽牙串：骨板下缘悬垂的7枚交错粗长异兽尖牙（加长加密，独立可动骨）。"""
    b = "hang_teeth"
    # 7 枚异兽尖牙串，沿下缘大幅度弧形加长悬挂（牙长可达 4.5px！）
    teeth_data = [
        # (tag, z_c, y_cord_top, y_root, y_mid, y_body, y_tip, mat_body)
        ("t1", -1.70, 17.05, 16.15, 15.35, 14.55, 13.85, "bone_dk"),
        ("t2", -1.15, 16.85, 15.85, 14.95, 14.05, 13.25, "bone"),
        ("t3", -0.60, 16.65, 15.55, 14.55, 13.55, 12.65, "bone_lt"),
        ("t4", 0.00,  16.45, 15.25, 14.15, 12.95, 11.85, "bone_lt"),  # 中央超长异兽犬齿
        ("t5", 0.60,  16.65, 15.55, 14.55, 13.55, 12.65, "bone"),
        ("t6", 1.15,  16.85, 15.85, 14.95, 14.05, 13.25, "bone_stain"),
        ("t7", 1.70,  17.05, 16.15, 15.35, 14.55, 13.85, "bone_dk"),
    ]
    for tag, zc, y_cord, y_root, y_mid, y_body, y_tip, mat_b in teeth_data:
        # 悬挂麻绳与紧结
        g.box(b, f"cord_{tag}", (-8.62, y_cord, zc - 0.12), (-8.36, 17.35, zc + 0.12), "cord_dk")
        g.box(b, f"knot_{tag}", (-8.70, y_cord - 0.28, zc - 0.16), (-8.28, y_cord + 0.05, zc + 0.16), "cord")

        # 牙根部（包骨环与陈年骨斑）
        g.box(b, f"root_{tag}", (-8.68, y_root, zc - 0.18), (-8.30, y_cord - 0.18, zc + 0.18), "bone_stain")
        g.box(b, f"ring_{tag}", (-8.74, y_root + 0.15, zc - 0.22), (-8.24, y_cord - 0.25, zc + 0.22), "bone_dk")
        g.box(b, f"peg_{tag}", (-8.82, y_root + 0.32, zc - 0.12), (-8.12, y_root + 0.62, zc + 0.12), "bone_lt")

        # 牙身分三节收窄弯曲
        g.box(b, f"mid_{tag}", (-8.66, y_mid, zc - 0.16), (-8.32, y_root + 0.12, zc + 0.16), mat_b)
        g.box(b, f"body_{tag}", (-8.64, y_body, zc - 0.14), (-8.34, y_mid + 0.10, zc + 0.14), "bone")
        g.box(b, f"tip_{tag}", (-8.60, y_tip, zc - 0.12), (-8.38, y_body + 0.08, zc + 0.12), "bone_lt")
        # 牙侧小倒钩
        g.box(b, f"barb_{tag}", (-8.68, y_body + 0.25, zc - 0.08), (-8.32, y_body + 0.65, zc + 0.22), "bone_stain")


def part_hang_strap_tail(g: Gear) -> None:
    """右腰皮带垂尾：穿出扣环自然下垂的备用打孔皮带头与兽骨坠（独立可动骨）。"""
    b = "hang_strap_tail"
    # 上段皮舌
    g.box(b, "tail_strap_top", (3.48, 10.35, -2.52), (3.98, 11.25, -2.25), "strap")
    # 下段打孔皮舌（Z 错开 0.03）
    g.box(b, "tail_strap_bot", (3.42, 9.15, -2.56), (3.92, 10.45, -2.28), "strap_dk")
    # 皮舌剪角尖端
    g.box(b, "tail_strap_tip", (3.48, 8.45, -2.52), (3.88, 9.25, -2.25), "strap")
    # 调节穿孔
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
    "plate_upper": part_plate_upper,
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
    wear_note="挂点：单肩（左） · 四层宽厚叠压兽骨肩甲+加长下垂兽牙+斜挎皮带",
    worn_focus=((-6.0, 21.5, 0.0), 22.0),
    tex_views=(
        ("LEFT 穿戴者左侧（-x）", 90.0, 0.0),
        ("3/4 左前斜", -145.0, 18.0),
        ("FRONT 正面（-z）", 180.0, 0.0),
    ),
    texture_overrides={
        "strap": "gear_hide",
        "strap_dk": "gear_hide_dk",
    },
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
