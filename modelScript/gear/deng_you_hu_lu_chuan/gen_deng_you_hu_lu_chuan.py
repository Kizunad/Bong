#!/usr/bin/env python3
"""灯油葫芦串（deng_you_hu_lu_chuan）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土定制 NPC 专属装备 #7（shou_ye_deng_ke 守夜灯客）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，+x = 穿戴者右侧。

挂点与避让说明：
- 挂点：从左肩（x≈-4.8..-2.4，y≈24.2）跨肩，经前胸斜跨至右腰（x≈2.6，y≈12.2），并系于腰周细皮绳圈。
- 与水囊（water_skin）方向相反（水囊是右肩翻过斜向左胯），守夜灯客斜挎方向为左肩至右腰，完全不抢占同一挂位通道。
- 4 只拳头大、鼓肚的干燥油葫芦（最大的约 4×5×4px，双球葫芦腰身收窄、骨塞、裂纹补绳）：
  1. gourd_a：上胸小双球葫芦（宽 2.4×高 4.0×深 2.4px），微向左倾斜，骨塞封口，附双圈麻绳结。
  2. gourd_b：中上胸饱满圆葫芦（宽 3.0×高 4.6×深 3.0px），双球腰身收窄，横向皮绳扎圈，带刻槽兽骨塞。
  3. gourd_c：中下胸大双肚葫芦（宽 3.8×高 5.2×深 3.8px，约 4×5×4px！），硕大圆鼓下肚、深凹收腰、纵向劈裂缝与粗麻补裂缝。
  4. gourd_d：右腰长颈微弯葫芦（宽 3.2×高 5.0×深 3.2px），弯颈斜塞，塞下穿绳垂挂一卷浸油焦黑备用灯芯。
- 挂件独立骨：备用灯芯卷（hang_wick）、骨质量油勺（hang_spoon）、带扣调节舌（strap_buckle）。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/gear/deng_you_hu_lu_chuan/gen_deng_you_hu_lu_chuan.py --self-test
    python3 modelScript/gear/deng_you_hu_lu_chuan/gen_deng_you_hu_lu_chuan.py --gates
    python3 modelScript/gear/deng_you_hu_lu_chuan/gen_deng_you_hu_lu_chuan.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "gourd":        (168, 128, 72),    # 干葫芦赭黄
    "gourd_lt":     (198, 158, 98),    # 葫芦反光亮面
    "gourd_dk":     (124, 92, 50),     # 葫芦腰暗凹与阴影
    "gourd_spot":   (96, 70, 42),      # 葫芦皮陈年斑点
    "leather":      (106, 72, 50),     # 挂带厚牛皮
    "leather_dk":   (74, 50, 36),      # 磨损深色皮
    "cord":         (146, 126, 82),    # 绑绳 / 悬绳
    "cord_dk":      (104, 88, 56),     # 粗绳结与缠线
    "bone":         (216, 206, 178),   # 骨塞 / 骨勺
    "bone_lt":      (238, 230, 208),   # 骨塞顶端与高光
    "bone_dk":      (158, 144, 114),   # 骨槽与骨孔
    "wick":         (52, 46, 42),      # 浸油灯芯粗织带
    "wick_char":    (28, 24, 22),      # 烧焦发黑灯头
    "iron":         (84, 80, 82),      # 生铁带扣
    "iron_lt":      (120, 116, 118),   # 铁铆钉
    "stitch":       (58, 44, 32),      # 粗麻缝线
    "void":         (22, 18, 16),      # 葫芦口暗孔
}

SOFT = ("gourd", "gourd_lt", "gourd_dk", "gourd_spot", "leather", "leather_dk",
        "cord", "cord_dk", "wick", "wick_char", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("waist_cord", "root", (0.0, 12.5, 0.0), "腰际定位绳带：前后对称细皮腰带（对称）"),
    ("shoulder_pad", "root", (-5.4, 24.5, 0.0), "左肩减震皮垫：骑跨左肩顶端的厚皮肩托"),
    ("strap_front", "shoulder_pad", (-0.5, 18.0, -2.45), "前胸斜跨皮带：从左肩斜贯至右腰的主承重带"),
    ("strap_back", "shoulder_pad", (-0.5, 18.0, 2.45), "后背斜跨皮带：后背斜向贯穿的背带与滑扣"),
    ("strap_buckle", "waist_cord", (2.8, 12.5, -2.45), "右腰生铁调节带扣与垂带舌（挂件独立骨）"),
    ("gourd_a", "strap_front", (-2.2, 20.8, -3.45), "高位双球小葫芦：上胸小油葫芦与骨塞（独立摆动骨）"),
    ("gourd_b", "strap_front", (-0.9, 17.8, -3.75), "中位饱满圆葫芦：胸心饱满油葫芦与扎圈（独立摆动骨）"),
    ("gourd_c", "strap_front", (0.6, 14.8, -4.10), "中低大双肚葫芦：4x5x4大双肚油葫芦与补裂缝（独立摆动骨）"),
    ("gourd_d", "strap_front", (2.4, 12.0, -3.75), "低位长颈弯葫芦：右腰长颈油葫芦与悬挂索（独立摆动骨）"),
    ("hang_wick", "gourd_d", (2.8, 9.5, -3.75), "备用灯芯卷：栓在低位葫芦下的盘卷浸油焦黑灯芯（挂件独立骨）"),
    ("hang_spoon", "strap_front", (-1.8, 15.8, -2.75), "刻骨量油勺：悬在胸前的小长柄雕花兽骨量勺（挂件独立骨）"),
    ("strap_knots", "strap_front", (0.0, 17.5, -2.55), "葫芦固定扎绳组：四处牢固绑死在主皮带上的麻绳结扣"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "waist_cord"))


# ───────────────────────────────────────────────────────────── 分部件实现
def part_waist_cord(g: Gear) -> None:
    """腰际定位绳带：前后对称细皮腰带，四枚对称固定铁钉与前后双行针脚（严格对称）。"""
    b = "waist_cord"
    # 前腰横束细皮带（厚度 ≥ 0.28）
    g.box(b, "belt_f_main", (-3.95, 12.22, -2.46), (3.95, 12.78, -2.18), "leather")
    g.box(b, "belt_f_rim_t", (-3.95, 12.78, -2.48), (3.95, 13.02, -2.16), "leather_dk")
    g.box(b, "belt_f_rim_b", (-3.95, 11.98, -2.48), (3.95, 12.22, -2.16), "leather_dk")

    # 后腰横束细皮带（对称）
    g.box(b, "belt_b_main", (-3.95, 12.22, 2.18), (3.95, 12.78, 2.46), "leather")
    g.box(b, "belt_b_rim_t", (-3.95, 12.78, 2.16), (3.95, 13.02, 2.48), "leather_dk")
    g.box(b, "belt_b_rim_b", (-3.95, 11.98, 2.16), (3.95, 12.22, 2.48), "leather_dk")

    # 对称前后铁铆钉
    for k in range(2):
        x = 1.15 + k * 1.85
        g.pair(b, f"belt_f_rivet_{k}", (x - 0.18, 12.35, -2.62), (x + 0.18, 12.65, -2.32), "iron_lt")
        g.pair(b, f"belt_b_rivet_{k}", (x - 0.18, 12.35, 2.32), (x + 0.18, 12.65, 2.62), "iron_lt")

    # 对称针脚行
    for k in range(3):
        x = 0.55 + k * 1.15
        g.pair(b, f"belt_f_st_{k}", (x, 12.82, -2.55), (x + 0.38, 13.05, -2.32), "stitch")
        g.pair(b, f"belt_b_st_{k}", (x, 12.82, 2.32), (x + 0.38, 13.05, 2.55), "stitch")


def part_shoulder_pad(g: Gear) -> None:
    """左肩减震皮垫：骑跨左肩顶端的厚皮肩托，严格位于 x ≤ -4.12 避开头部。"""
    b = "shoulder_pad"
    # 左肩顶覆皮（X∈[-6.75, -4.12]，Y∈[24.12, 25.35]，Z∈[-2.52, 2.42]）
    g.box(b, "pad_base", (-6.75, 24.12, -2.42), (-4.12, 25.15, 2.32), "leather")
    g.box(b, "pad_rim_l", (-6.88, 24.12, -2.52), (-6.55, 25.28, 2.42), "leather_dk")
    g.box(b, "pad_rim_r", (-4.45, 24.12, -2.52), (-4.12, 25.28, 2.42), "leather_dk")
    g.box(b, "pad_ridge_t", (-6.45, 24.85, -2.25), (-4.35, 25.35, 2.15), "leather_dk")
    g.box(b, "pad_ridge_b", (-6.25, 24.95, -2.05), (-4.55, 25.45, 1.95), "leather")

    # 左右双排加固粗针脚与铁扣环
    for k in range(4):
        z = -1.65 + k * 1.05
        g.box(b, f"pad_stitch_l_{k}", (-6.65, 24.85, z), (-6.15, 25.18, z + 0.35), "stitch")
        g.box(b, f"pad_stitch_r_{k}", (-4.75, 24.85, z), (-4.25, 25.18, z + 0.35), "stitch")
    g.box(b, "pad_iron_ring_f", (-5.65, 24.15, -2.75), (-4.75, 24.95, -2.40), "iron")
    g.box(b, "pad_iron_ring_b", (-5.65, 24.15, 2.40), (-4.75, 24.95, 2.75), "iron")


def part_strap_front(g: Gear) -> None:
    """前胸斜跨皮带：从左肩斜贯至右腰的主承重带。"""
    b = "strap_front"
    segments = (
        ("front_seg1", -4.15, -2.45, 20.85, 23.85, -2.50, -2.22),
        ("front_seg2", -2.55, -0.95, 17.85, 21.05, -2.50, -2.22),
        ("front_seg3", -1.05, 0.85, 14.85, 18.05, -2.50, -2.22),
        ("front_seg4", 0.75, 2.65, 12.25, 15.05, -2.50, -2.22),
    )
    for name, x0, x1, y0, y1, z0, z1 in segments:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), "leather")

    edges = (
        ("edge_seg1_t", -4.25, -2.55, 21.05, 23.95, -2.56, -2.28),
        ("edge_seg1_b", -4.05, -2.35, 20.65, 23.75, -2.56, -2.28),
        ("edge_seg2_t", -2.65, -1.05, 18.05, 21.15, -2.56, -2.28),
        ("edge_seg2_b", -2.45, -0.85, 17.65, 20.75, -2.56, -2.28),
        ("edge_seg3_t", -1.15, 0.75, 15.05, 18.15, -2.56, -2.28),
        ("edge_seg3_b", -0.95, 0.95, 14.65, 17.75, -2.56, -2.28),
        ("edge_seg4_t", 0.65, 2.55, 12.45, 15.15, -2.56, -2.28),
        ("edge_seg4_b", 0.85, 2.75, 12.05, 14.75, -2.56, -2.28),
    )
    for name, x0, x1, y0, y1, z0, z1 in edges:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), "leather_dk")

    for k in range(5):
        t = k / 4.0
        x = -3.5 + t * 5.2
        y = 22.8 - t * 9.8
        g.box(b, f"front_st_l_{k}", (x - 0.22, y, -2.62), (x + 0.15, y + 0.32, -2.32), "stitch")
        g.box(b, f"front_st_r_{k}", (x + 0.35, y - 0.22, -2.62), (x + 0.72, y + 0.10, -2.32), "stitch")

    rivet_pos = ((-2.6, 22.2), (-1.4, 19.4), (0.1, 16.5), (1.6, 13.7))
    for k, (x, y) in enumerate(rivet_pos):
        g.box(b, f"front_rivet_{k}", (x - 0.18, y - 0.18, -2.65), (x + 0.18, y + 0.18, -2.35), "iron_lt")
        g.box(b, f"front_punch_{k}", (x + 0.35, y - 0.14, -2.62), (x + 0.65, y + 0.14, -2.32), "void")


def part_strap_back(g: Gear) -> None:
    """后背斜跨皮带：后背斜向贯穿的背带与滑扣。"""
    b = "strap_back"
    segments = (
        ("back_seg1", -4.15, -2.45, 20.85, 23.85, 2.22, 2.50),
        ("back_seg2", -2.55, -0.95, 17.85, 21.05, 2.22, 2.50),
        ("back_seg3", -1.05, 0.85, 14.85, 18.05, 2.22, 2.50),
        ("back_seg4", 0.75, 2.65, 12.25, 15.05, 2.22, 2.50),
    )
    for name, x0, x1, y0, y1, z0, z1 in segments:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), "leather")

    for k, (name, x0, x1, y0, y1, z0, z1) in enumerate(segments):
        g.box(b, f"back_edge_{k}", (x0 - 0.1, y0, 2.28), (x1 + 0.1, y1, 2.56), "leather_dk")

    for k in range(5):
        t = k / 4.0
        x = -3.5 + t * 5.2
        y = 22.8 - t * 9.8
        g.box(b, f"back_st_l_{k}", (x - 0.22, y, 2.32), (x + 0.15, y + 0.32, 2.62), "stitch")
        g.box(b, f"back_st_r_{k}", (x + 0.35, y - 0.22, 2.32), (x + 0.72, y + 0.10, 2.62), "stitch")

    # 后背滑动生铁日字调节扣（三轴尺寸均 ≥ 0.62，用于穿模自证）
    g.box(b, "back_slider_frame", (-0.95, 16.55, 2.38), (0.75, 18.25, 3.02), "iron")
    g.box(b, "back_slider_bar_m", (-0.22, 16.65, 2.45), (0.22, 18.15, 3.12), "iron_lt")
    g.box(b, "back_slider_pin", (-0.15, 17.15, 3.02), (0.15, 17.85, 3.25), "bone_dk")


def part_strap_buckle(g: Gear) -> None:
    """右腰生铁调节带扣与垂带舌。"""
    b = "strap_buckle"
    g.box(b, "buckle_frame_t", (2.15, 12.85, -2.78), (3.35, 13.15, -2.42), "iron")
    g.box(b, "buckle_frame_b", (2.15, 11.85, -2.78), (3.35, 12.15, -2.42), "iron")
    g.box(b, "buckle_frame_l", (2.15, 12.05, -2.78), (2.45, 12.95, -2.42), "iron_lt")
    g.box(b, "buckle_frame_r", (3.05, 12.05, -2.78), (3.35, 12.95, -2.42), "iron")

    g.box(b, "buckle_tongue", (2.65, 11.95, -2.92), (2.88, 12.95, -2.55), "bone_dk")
    g.box(b, "buckle_underpad", (2.05, 11.75, -2.52), (3.45, 13.25, -2.25), "leather_dk")

    g.box(b, "buckle_tail_a", (2.45, 10.85, -2.72), (2.95, 11.95, -2.45), "leather")
    g.box(b, "buckle_tail_b", (2.42, 9.95, -2.68), (2.88, 10.95, -2.45), "leather_dk")
    g.box(b, "buckle_tail_tip", (2.48, 9.45, -2.65), (2.82, 10.05, -2.46), "leather")
    g.box(b, "buckle_keeper1", (2.35, 11.25, -2.78), (3.05, 11.55, -2.38), "leather_dk")
    g.box(b, "buckle_keeper2", (2.35, 10.45, -2.76), (2.95, 10.75, -2.40), "leather")
    g.box(b, "buckle_rivet1", (2.55, 12.05, -2.85), (2.85, 12.35, -2.52), "iron_lt")


def part_gourd_a(g: Gear) -> None:
    """高位小葫芦：上胸小双球油葫芦（宽 2.4×高 4.0×深 2.4px），腰身收窄与骨塞（Y顶端 ≤ 23.90 避开头部）。"""
    b = "gourd_a"
    # 中心 X=-2.20, Y=20.20, Z=-3.45 (顶端骨塞 20.20+3.65=23.85 严格低于头盒 y=24.0)
    cx, cy, cz = -2.20, 20.20, -3.45

    # 1. 下肚球体（分 3 层体素，宽度 2.4, 深度 2.4, 高度 1.8）
    g.box(b, "gourd_a_base", (cx - 0.85, cy - 1.15, cz - 0.85), (cx + 0.85, cy - 0.75, cz + 0.85), "gourd_dk")
    g.box(b, "gourd_a_belly_b", (cx - 1.20, cy - 0.75, cz - 1.20), (cx + 1.20, cy + 0.45, cz + 1.20), "gourd")
    g.box(b, "gourd_a_belly_ring", (cx - 1.25, cy - 0.35, cz - 1.25), (cx + 1.25, cy + 0.15, cz + 1.25), "gourd_lt")
    g.box(b, "gourd_a_belly_shoulder", (cx - 1.05, cy + 0.45, cz - 1.05), (cx + 1.05, cy + 0.75, cz + 1.05), "gourd")

    # 2. 腰身明显收窄（腰细至 1.5×1.5）
    g.box(b, "gourd_a_waist", (cx - 0.75, cy + 0.75, cz - 0.75), (cx + 0.75, cy + 1.25, cz + 0.75), "gourd_dk")
    # 缠绕在腰部的细麻绳圈与绳结
    g.box(b, "gourd_a_cord_w", (cx - 0.82, cy + 0.85, cz - 0.82), (cx + 0.82, cy + 1.15, cz + 0.82), "cord_dk")
    g.box(b, "gourd_a_cord_knot", (cx + 0.65, cy + 0.75, cz - 0.65), (cx + 1.05, cy + 1.25, cz - 0.25), "cord")

    # 3. 上肚球体（宽度 1.9, 深度 1.9, 高度 1.4）
    g.box(b, "gourd_a_belly_t", (cx - 0.95, cy + 1.25, cz - 0.95), (cx + 0.95, cy + 2.05, cz + 0.95), "gourd")
    g.box(b, "gourd_a_belly_t_crest", (cx - 1.00, cy + 1.45, cz - 1.00), (cx + 1.00, cy + 1.85, cz + 1.00), "gourd_lt")
    g.box(b, "gourd_a_top_cap", (cx - 0.65, cy + 2.05, cz - 0.65), (cx + 0.65, cy + 2.35, cz + 0.65), "gourd_dk")

    # 4. 瓶口与兽骨塞
    g.box(b, "gourd_a_neck", (cx - 0.45, cy + 2.35, cz - 0.45), (cx + 0.45, cy + 2.65, cz + 0.45), "gourd")
    g.box(b, "gourd_a_lip", (cx - 0.52, cy + 2.65, cz - 0.52), (cx + 0.52, cy + 2.85, cz + 0.52), "gourd_dk")
    g.box(b, "gourd_a_mouth_void", (cx - 0.22, cy + 2.75, cz - 0.22), (cx + 0.22, cy + 2.95, cz + 0.22), "void")

    g.box(b, "gourd_a_plug", (cx - 0.28, cy + 2.80, cz - 0.28), (cx + 0.28, cy + 3.45, cz + 0.28), "bone")
    g.box(b, "gourd_a_plug_cap", (cx - 0.38, cy + 3.35, cz - 0.38), (cx + 0.38, cy + 3.65, cz + 0.38), "bone_lt")

    # 表面风化暗斑
    g.box(b, "gourd_a_spot", (cx - 0.95, cy - 0.45, cz - 1.28), (cx - 0.45, cy + 0.15, cz - 1.05), "gourd_spot")


def part_gourd_b(g: Gear) -> None:
    """中位圆腹葫芦：胸心饱满油葫芦（宽 3.0×高 4.6×深 3.0px），深腰身与刻槽骨塞。"""
    b = "gourd_b"
    cx, cy, cz = -0.90, 17.80, -3.75

    # 1. 硕大下肚（宽 3.0, 深 3.0, 高 2.2）
    g.box(b, "gourd_b_base", (cx - 1.10, cy - 1.35, cz - 1.10), (cx + 1.10, cy - 0.85, cz + 1.10), "gourd_dk")
    g.box(b, "gourd_b_belly_b", (cx - 1.50, cy - 0.85, cz - 1.50), (cx + 1.50, cy + 0.55, cz + 1.50), "gourd")
    g.box(b, "gourd_b_belly_rib1", (cx - 1.58, cy - 0.45, cz - 1.58), (cx + 1.58, cy + 0.25, cz + 1.58), "gourd_lt")
    g.box(b, "gourd_b_belly_shoulder", (cx - 1.30, cy + 0.55, cz - 1.30), (cx + 1.30, cy + 0.95, cz + 1.30), "gourd")

    # 2. 深凹腰身（细至 1.8×1.8）
    g.box(b, "gourd_b_waist", (cx - 0.90, cy + 0.95, cz - 0.90), (cx + 0.90, cy + 1.45, cz + 0.90), "gourd_dk")
    g.box(b, "gourd_b_cord_w", (cx - 0.98, cy + 1.05, cz - 0.98), (cx + 0.98, cy + 1.35, cz + 0.98), "cord_dk")
    g.box(b, "gourd_b_cord_knot", (cx - 1.25, cy + 0.95, cz - 0.65), (cx - 0.75, cy + 1.45, cz - 0.25), "cord")

    # 3. 丰满上肚（宽 2.4, 深 2.4, 高 1.6）
    g.box(b, "gourd_b_belly_t", (cx - 1.20, cy + 1.45, cz - 1.20), (cx + 1.20, cy + 2.35, cz + 1.20), "gourd")
    g.box(b, "gourd_b_belly_t_crest", (cx - 1.25, cy + 1.65, cz - 1.25), (cx + 1.25, cy + 2.15, cz + 1.25), "gourd_lt")
    g.box(b, "gourd_b_top_cap", (cx - 0.85, cy + 2.35, cz - 0.85), (cx + 0.85, cy + 2.65, cz + 0.85), "gourd_dk")

    # 4. 瓶口与带刻槽兽骨塞
    g.box(b, "gourd_b_neck", (cx - 0.55, cy + 2.65, cz - 0.55), (cx + 0.55, cy + 2.95, cz + 0.55), "gourd")
    g.box(b, "gourd_b_lip", (cx - 0.62, cy + 2.95, cz - 0.62), (cx + 0.62, cy + 3.15, cz + 0.62), "gourd_dk")
    g.box(b, "gourd_b_void", (cx - 0.25, cy + 3.05, cz - 0.25), (cx + 0.25, cy + 3.25, cz + 0.25), "void")

    g.box(b, "gourd_b_plug", (cx - 0.32, cy + 3.10, cz - 0.32), (cx + 0.32, cy + 3.85, cz + 0.32), "bone")
    g.box(b, "gourd_b_plug_top", (cx - 0.42, cy + 3.75, cz - 0.42), (cx + 0.42, cy + 4.15, cz + 0.42), "bone_lt")
    g.box(b, "gourd_b_plug_slit", (cx - 0.35, cy + 3.45, cz - 0.35), (cx + 0.35, cy + 3.65, cz + 0.35), "bone_dk")

    # 斑点与裂纹皮条
    g.box(b, "gourd_b_spot1", (cx - 1.15, cy - 0.65, cz - 1.62), (cx - 0.65, cy - 0.15, cz - 1.35), "gourd_spot")
    g.box(b, "gourd_b_spot2", (cx + 0.55, cy - 0.45, cz - 1.62), (cx + 1.05, cy + 0.05, cz - 1.35), "gourd_spot")


def part_gourd_c(g: Gear) -> None:
    """中低大双肚葫芦：4x5x4大双肚油葫芦与补裂缝（宽 3.8×高 5.2×深 3.8px，胸前最醒目核心！）。"""
    b = "gourd_c"
    # 中心 X=0.60, Y=14.80, Z=-4.10 (后部 z=-2.20 绝对不穿胸)
    cx, cy, cz = 0.60, 14.80, -4.10

    # 1. 巨大下肚（近 4px 宽深，壮硕饱满！）
    g.box(b, "gourd_c_base", (cx - 1.40, cy - 1.85, cz - 1.40), (cx + 1.40, cy - 1.25, cz + 1.40), "gourd_dk")
    g.box(b, "gourd_c_belly_b", (cx - 1.90, cy - 1.25, cz - 1.90), (cx + 1.90, cy + 0.65, cz + 1.90), "gourd")
    g.box(b, "gourd_c_belly_crest", (cx - 1.98, cy - 0.75, cz - 1.98), (cx + 1.98, cy + 0.35, cz + 1.98), "gourd_lt")
    g.box(b, "gourd_c_belly_shoulder", (cx - 1.65, cy + 0.65, cz - 1.65), (cx + 1.65, cy + 1.15, cz + 1.65), "gourd")

    # 腹部大纵向裂纹与补合缝线（末土破损修补细节）
    g.box(b, "gourd_c_crack", (cx - 1.45, cy - 0.95, cz - 2.05), (cx - 0.85, cy + 0.55, cz - 1.65), "gourd_dk")
    g.box(b, "gourd_c_patch_st1", (cx - 1.65, cy - 0.45, cz - 2.12), (cx - 0.65, cy - 0.15, cz - 1.75), "stitch")
    g.box(b, "gourd_c_patch_st2", (cx - 1.65, cy + 0.15, cz - 2.12), (cx - 0.65, cy + 0.45, cz - 1.75), "stitch")

    # 2. 深凹腰身（急剧收窄至 2.4×2.4，体现传统双球葫芦标志性腰身）
    g.box(b, "gourd_c_waist", (cx - 1.20, cy + 1.15, cz - 1.20), (cx + 1.20, cy + 1.75, cz + 1.20), "gourd_dk")
    # 腰部双圈粗麻捆扎绳与大悬挂结扣
    g.box(b, "gourd_c_cord_w1", (cx - 1.28, cy + 1.25, cz - 1.28), (cx + 1.28, cy + 1.45, cz + 1.28), "cord_dk")
    g.box(b, "gourd_c_cord_w2", (cx - 1.28, cy + 1.48, cz - 1.28), (cx + 1.28, cy + 1.68, cz + 1.28), "cord")
    g.box(b, "gourd_c_cord_knot", (cx + 0.95, cy + 1.20, cz - 0.95), (cx + 1.55, cy + 1.75, cz - 0.45), "cord_dk")

    # 3. 丰厚上肚（宽 3.0, 深 3.0, 高 1.8）
    g.box(b, "gourd_c_belly_t", (cx - 1.50, cy + 1.75, cz - 1.50), (cx + 1.50, cy + 2.75, cz + 1.50), "gourd")
    g.box(b, "gourd_c_belly_t_crest", (cx - 1.58, cy + 1.95, cz - 1.58), (cx + 1.58, cy + 2.55, cz + 1.58), "gourd_lt")
    g.box(b, "gourd_c_top_rim", (cx - 1.10, cy + 2.75, cz - 1.10), (cx + 1.10, cy + 3.05, cz + 1.10), "gourd_dk")

    # 4. 粗大出油瓶口与兽骨大塞
    g.box(b, "gourd_c_neck", (cx - 0.75, cy + 3.05, cz - 0.75), (cx + 0.75, cy + 3.45, cz + 0.75), "gourd")
    g.box(b, "gourd_c_lip", (cx - 0.85, cy + 3.45, cz - 0.85), (cx + 0.85, cy + 3.65, cz + 0.85), "gourd_dk")
    g.box(b, "gourd_c_void", (cx - 0.35, cy + 3.55, cz - 0.35), (cx + 0.35, cy + 3.75, cz + 0.35), "void")

    g.box(b, "gourd_c_plug", (cx - 0.45, cy + 3.60, cz - 0.45), (cx + 0.45, cy + 4.55, cz + 0.45), "bone")
    g.box(b, "gourd_c_plug_cap", (cx - 0.60, cy + 4.45, cz - 0.60), (cx + 0.60, cy + 4.85, cz + 0.60), "bone_lt")

    # 风化油斑
    g.box(b, "gourd_c_belly_spot", (cx + 0.85, cy - 0.85, cz - 2.05), (cx + 1.45, cy - 0.25, cz - 1.75), "gourd_spot")


def part_gourd_d(g: Gear) -> None:
    """低位长颈弯葫芦：右腰长颈油葫芦（宽 3.2×高 5.0×深 3.2px），弯颈斜塞与悬芯细绳。"""
    b = "gourd_d"
    cx, cy, cz = 2.40, 12.00, -3.75

    # 1. 硕大下肚（宽 3.2, 深 3.2, 高 2.2）
    g.box(b, "gourd_d_base", (cx - 1.20, cy - 1.35, cz - 1.15), (cx + 1.20, cy - 0.85, cz + 1.15), "gourd_dk")
    g.box(b, "gourd_d_belly_b", (cx - 1.60, cy - 0.85, cz - 1.55), (cx + 1.60, cy + 0.65, cz + 1.55), "gourd")
    g.box(b, "gourd_d_belly_rib", (cx - 1.68, cy - 0.45, cz - 1.62), (cx + 1.68, cy + 0.25, cz + 1.62), "gourd_lt")
    g.box(b, "gourd_d_belly_shoulder", (cx - 1.35, cy + 0.65, cz - 1.30), (cx + 1.35, cy + 1.05, cz + 1.30), "gourd")

    # 2. 收窄腰身
    g.box(b, "gourd_d_waist", (cx - 0.95, cy + 1.05, cz - 0.95), (cx + 0.95, cy + 1.55, cz + 0.95), "gourd_dk")
    g.box(b, "gourd_d_cord_w", (cx - 1.02, cy + 1.15, cz - 1.02), (cx + 1.02, cy + 1.45, cz + 1.02), "cord_dk")
    g.box(b, "gourd_d_cord_knot", (cx + 0.75, cy + 1.05, cz - 0.65), (cx + 1.15, cy + 1.55, cz - 0.25), "cord_dk")

    # 3. 弯曲长颈与小上肚（向右微偏，形成自然歪颈葫芦）
    g.box(b, "gourd_d_neck1", (cx - 0.85, cy + 1.55, cz - 0.85), (cx + 0.95, cy + 2.35, cz + 0.85), "gourd")
    g.box(b, "gourd_d_neck2", (cx - 0.65, cy + 2.35, cz - 0.75), (cx + 1.15, cy + 3.05, cz + 0.75), "gourd_lt")
    g.box(b, "gourd_d_lip", (cx - 0.45, cy + 3.05, cz - 0.65), (cx + 1.05, cy + 3.35, cz + 0.65), "gourd_dk")
    g.box(b, "gourd_d_void", (cx - 0.05, cy + 3.15, cz - 0.25), (cx + 0.55, cy + 3.45, cz + 0.25), "void")

    # 4. 骨塞与系灯芯细绳孔
    g.box(b, "gourd_d_plug", (cx - 0.15, cy + 3.30, cz - 0.35), (cx + 0.75, cy + 4.15, cz + 0.35), "bone")
    g.box(b, "gourd_d_plug_eye", (cx + 0.05, cy + 4.05, cz - 0.22), (cx + 0.55, cy + 4.45, cz + 0.22), "bone_lt")

    # 垂落细绳（悬挂下方备用灯芯卷）
    g.box(b, "gourd_d_wick_line1", (cx + 0.85, cy + 0.65, cz - 0.35), (cx + 1.15, cy + 3.45, cz - 0.05), "cord")
    g.box(b, "gourd_d_wick_line2", (cx + 0.85, cy - 1.45, cz - 0.35), (cx + 1.15, cy + 0.75, cz - 0.05), "cord_dk")


def part_hang_wick(g: Gear) -> None:
    """备用灯芯卷：栓在低位葫芦下的盘卷浸油焦黑灯芯（挂件独立骨）。"""
    b = "hang_wick"
    cx, cy, cz = 3.25, 9.50, -3.75
    # 盘卷主轮圈（宽 1.6, 高 1.6, 深 0.8）
    g.box(b, "wick_coil_core", (cx - 0.75, cy - 0.75, cz - 0.40), (cx + 0.75, cy + 0.75, cz + 0.40), "wick")
    g.box(b, "wick_coil_rim_t", (cx - 0.65, cy + 0.65, cz - 0.45), (cx + 0.65, cy + 0.92, cz + 0.45), "wick_char")
    g.box(b, "wick_coil_rim_b", (cx - 0.65, cy - 0.92, cz - 0.45), (cx + 0.65, cy - 0.65, cz + 0.45), "wick_char")
    g.box(b, "wick_coil_rim_l", (cx - 0.92, cy - 0.65, cz - 0.45), (cx - 0.65, cy + 0.65, cz + 0.45), "wick")
    g.box(b, "wick_coil_rim_r", (cx + 0.65, cy - 0.65, cz - 0.45), (cx + 0.92, cy + 0.65, cz + 0.45), "wick_char")

    # 束紧油芯卷的细草绳圈
    g.box(b, "wick_tie_w", (cx - 0.28, cy - 0.85, cz - 0.48), (cx + 0.28, cy + 0.85, cz + 0.48), "cord_dk")
    g.box(b, "wick_tie_knot", (cx - 0.22, cy + 0.75, cz - 0.55), (cx + 0.22, cy + 1.15, cz + 0.25), "cord")

    # 伸出卷外的毛糙焦黑灯芯头（带烧焦发硬尖角）
    g.box(b, "wick_tail_char1", (cx + 0.55, cy - 1.35, cz - 0.32), (cx + 0.88, cy - 0.65, cz + 0.32), "wick_char")
    g.box(b, "wick_tail_char2", (cx + 0.65, cy - 1.85, cz - 0.28), (cx + 0.98, cy - 1.25, cz + 0.28), "wick_char")
    g.box(b, "wick_tail_char3", (cx + 0.48, cy - 2.15, cz - 0.22), (cx + 0.82, cy - 1.65, cz + 0.22), "wick_char")
    g.box(b, "wick_grease_spot1", (cx - 0.55, cy - 0.45, cz - 0.50), (cx - 0.15, cy + 0.15, cz - 0.22), "wick_char")
    g.box(b, "wick_grease_spot2", (cx + 0.15, cy + 0.15, cz - 0.50), (cx + 0.55, cy + 0.55, cz - 0.22), "wick_char")


def part_hang_spoon(g: Gear) -> None:
    """刻骨量油勺：悬在胸前的小长柄雕花兽骨量勺（挂件独立骨）。"""
    b = "hang_spoon"
    cx, cy, cz = -1.85, 15.80, -2.75
    # 悬挂细麻绳与绳结
    g.box(b, "spoon_cord", (cx - 0.15, cy + 1.25, cz - 0.15), (cx + 0.15, cy + 2.35, cz + 0.15), "cord")
    g.box(b, "spoon_knot", (cx - 0.22, cy + 1.05, cz - 0.22), (cx + 0.22, cy + 1.35, cz + 0.22), "cord_dk")

    # 长骨柄（带多段刻槽）
    g.box(b, "spoon_handle", (cx - 0.18, cy - 1.15, cz - 0.18), (cx + 0.18, cy + 1.15, cz + 0.18), "bone")
    g.box(b, "spoon_crest_t", (cx - 0.25, cy + 0.85, cz - 0.25), (cx + 0.25, cy + 1.15, cz + 0.25), "bone_lt")
    g.box(b, "spoon_notch1", (cx - 0.22, cy + 0.25, cz - 0.22), (cx + 0.22, cy + 0.45, cz + 0.22), "bone_dk")
    g.box(b, "spoon_notch2", (cx - 0.22, cy - 0.45, cz - 0.22), (cx + 0.22, cy - 0.25, cz + 0.22), "bone_dk")
    g.box(b, "spoon_cord_bead", (cx - 0.22, cy + 1.85, cz - 0.22), (cx + 0.22, cy + 2.15, cz + 0.22), "bone_lt")

    # 勺斗（盛油小凹斗）
    g.box(b, "spoon_bowl_body", (cx - 0.45, cy - 1.95, cz - 0.42), (cx + 0.45, cy - 1.05, cz + 0.42), "bone")
    g.box(b, "spoon_bowl_rim", (cx - 0.52, cy - 1.45, cz - 0.48), (cx + 0.52, cy - 1.05, cz + 0.48), "bone_lt")
    g.box(b, "spoon_bowl_void", (cx - 0.25, cy - 1.55, cz - 0.25), (cx + 0.25, cy - 1.05, cz + 0.25), "void")
    g.box(b, "spoon_bowl_keel", (cx - 0.15, cy - 2.15, cz - 0.35), (cx + 0.15, cy - 1.35, cz + 0.35), "bone_dk")


def part_strap_knots(g: Gear) -> None:
    """葫芦固定扎绳组：四处牢固绑死在主皮带上的麻绳结扣与连接带环。"""
    b = "strap_knots"
    knot_specs = (
        ("knot_a", -2.35, 20.65, -2.62, "cord_dk"),
        ("knot_b", -0.95, 18.05, -2.65, "cord_dk"),
        ("knot_c", 0.65, 15.45, -2.68, "cord_dk"),
        ("knot_d", 2.05, 12.85, -2.65, "cord_dk"),
    )
    for name, x, y, z, mat in knot_specs:
        g.box(b, f"{name}_wrap", (x - 0.35, y - 0.25, z - 0.25), (x + 0.35, y + 0.25, z + 0.25), mat)
        g.box(b, f"{name}_boss", (x - 0.22, y - 0.18, z - 0.35), (x + 0.22, y + 0.18, z - 0.15), "cord")
        g.box(b, f"{name}_rivet", (x - 0.15, y + 0.22, z - 0.28), (x + 0.15, y + 0.45, z - 0.12), "iron_lt")
        g.box(b, f"{name}_dangle", (x - 0.12, y - 0.55, z - 0.28), (x + 0.12, y - 0.22, z - 0.12), "cord_dk")


# ───────────────────────────────────────────────────────────── 门禁与共面自动错开
def deconflict_and_fix(g: Gear) -> None:
    """消除不对称部件内的 Z-fighting 共面，并确保无薄片。"""
    all_els = [e for e in g.rig.elements if not any(e.get("rotation", [0, 0, 0]))]
    asym_uuids = set()
    for b_name in ASYM_BONES:
        if b_name in g.rig.bones:
            asym_uuids.update(g.rig.bones[b_name]["children"])

    # 1. 修正尺寸 < 0.22 的薄片
    for e in all_els:
        for ax in range(3):
            s = e["to"][ax] - e["from"][ax]
            if s < 0.22:
                d = 0.22 - s
                e["from"][ax] = round(e["from"][ax] - d / 2, 4)
                e["to"][ax] = round(e["to"][ax] + d / 2, 4)

    # 2. 消除不对称件与其它件的共面冲突
    asym_els = [e for e in all_els if e["uuid"] in asym_uuids]
    for i, e in enumerate(asym_els):
        idx = all_els.index(e)
        for _ in range(25):
            moved = False
            for f in all_els[:idx]:
                for ax in range(3):
                    o1, o2 = [a for a in range(3) if a != ax]
                    if min(e["to"][o1], f["to"][o1]) - max(e["from"][o1], f["from"][o1]) <= 1e-4:
                        continue
                    if min(e["to"][o2], f["to"][o2]) - max(e["from"][o2], f["from"][o2]) <= 1e-4:
                        continue
                    for side, sign in (("from", -1), ("to", 1)):
                        if abs(e[side][ax] - f[side][ax]) < 1e-4:
                            e[side][ax] = round(e[side][ax] + sign * 0.003 * (1 + (i % 5)), 4)
                            moved = True
            if not moved:
                break


class DengYouHuLuChuanSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "waist_cord": part_waist_cord,
    "shoulder_pad": part_shoulder_pad,
    "strap_front": part_strap_front,
    "strap_back": part_strap_back,
    "strap_buckle": part_strap_buckle,
    "gourd_a": part_gourd_a,
    "gourd_b": part_gourd_b,
    "gourd_c": part_gourd_c,
    "gourd_d": part_gourd_d,
    "hang_wick": part_hang_wick,
    "hang_spoon": part_hang_spoon,
    "strap_knots": part_strap_knots,
}

SPEC = DengYouHuLuChuanSpec(
    gear_id="deng_you_hu_lu_chuan",
    model_name="DengYouHuLuChuan",
    title="灯油葫芦串 / deng_you_hu_lu_chuan",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：左肩至右腰斜跨 · 四只拳头大鼓肚油葫芦+焦黑备用灯芯卷+骨量油勺",
    worn_focus=((0.0, 16.5, -1.0), 22.0),
    tex_views=(
        ("FRONT 正面（-z）", 180.0, 0.0),
        ("3/4 前左斜", -28.0, 18.0),
        ("RIGHT 右侧（+x）", -90.0, 0.0),
    ),
    textured=True,
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
