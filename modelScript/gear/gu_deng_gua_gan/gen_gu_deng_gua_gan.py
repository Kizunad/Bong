#!/usr/bin/env python3
"""骨灯挂杆（gu_deng_gua_gan）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #20（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形（参考图 gu_deng_gua_gan_ref.png / gu_deng_gua_gan_exploded.png）：
背上斜插一根骨杆，顶端挂兽骨灯笼（灯芯处提高对比度，不做真发光）。
- 背板与背带：厚兽皮背托板贴紧后背（z∈[2.10, 2.50]，y∈[12.8, 23.5]），肩带过肩（|x| ≥ 4.10，y ≥ 24.10）。
- 骨杆底座：背板下方双层加固生铁箍与麻绳穿插绑扎，斜向稳固承插骨杆基座。
- 异兽脊骨长杆：粗壮带骨节的分节长骨杆从后背左下方斜向右上拔起，经右肩外侧高高耸立（y∈[13.5, 32.5]）。
  在后背上升时 z ≥ 4.15（避开头盒），在右肩外侧保持 x ≥ 4.15（避开头盒），在右臂上方保持 y ≥ 24.50（避开手臂盒）。
- 杆顶悬挂兽骨灯笼：顶端弯钩挂绳垂下一只六角镂空雕琢的异兽头骨灯笼（x∈[4.4, 5.8]，y∈[24.5, 30.5]）。
  骨笼内部嵌有一团高对比度的金白烈焰灯芯（高对比度不透明体素，不做真发光）。
- 挂件点缀：骨杆中段绑有辟邪骨币串与风化符片（独立骨），灯笼底座下垂兽骨坠与碎皮须（独立骨，y ≥ 24.20 避开手臂）。

骨骼数 ≥ 10，挂件独立骨，全绿门禁（含 clearance、coplanar、self-test）。
用法：
    python3 modelScript/gear/gu_deng_gua_gan/gen_gu_deng_gua_gan.py --self-test
    python3 modelScript/gear/gu_deng_gua_gan/gen_gu_deng_gua_gan.py --gates
    python3 modelScript/gear/gu_deng_gua_gan/gen_gu_deng_gua_gan.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "bone":        (214, 204, 176),   # 异兽骨主色
    "bone_lt":     (236, 226, 200),   # 骨棱高光 / 灯笼骨架
    "bone_dk":     (168, 154, 126),   # 骨槽暗色
    "bone_stain":  (130, 110, 82),    # 老化老骨斑
    "glow_core":   (255, 242, 170),   # 高对比度明亮灯芯核
    "glow_flame":  (245, 175, 60),    # 灯芯烈焰外缘
    "hide":        (120, 78, 56),     # 背板生皮
    "hide_dk":     (84, 54, 40),      # 背板深皮
    "hide_lt":     (152, 108, 78),    # 浅色垫皮
    "strap":       (112, 72, 50),     # 背带
    "strap_dk":    (78, 48, 36),      # 深色背带
    "cord":        (144, 128, 78),    # 吊绳 / 扎绑麻绳
    "cord_dk":     (102, 88, 56),     # 深色麻绳
    "iron":        (82, 78, 76),      # 铁箍 / 吊钩
    "iron_lt":     (116, 112, 110),   # 铁光泽
    "rust":        (152, 86, 52),     # 铁锈斑
    "void":        (28, 24, 22),      # 灯笼内部暗空隙
}

# 软件材质与骨铁硬物允许贴合咬合
SOFT = ("hide", "hide_dk", "hide_lt", "strap", "strap_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void", "glow_core", "glow_flame"), ("bone", "bone_lt", "bone_dk", "bone_stain", "iron"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("harness_back", "root", (0.0, 18.0, 2.25), "背板总成：厚生皮背垫板、中线护脊带、定位铁铆片"),
    ("harness_straps", "root", (0.0, 18.0, -2.25), "双肩背带：过肩宽皮带、胸前固定斜索、带扣与针脚"),
    ("pole_socket", "harness_back", (-1.2, 14.5, 2.6), "骨杆底座：斜插承托套筒、双层加固生铁箍、粗麻紧固缠绳"),
    ("pole_lower", "pole_socket", (0.5, 18.5, 2.9), "骨杆下段：斜跨背部的下部粗壮兽脊骨、分叉骨突"),
    ("pole_upper", "pole_lower", (3.6, 26.5, 3.4), "骨杆上段：高高耸立在右肩上方的弧形大骨杆、骨节与骨裂纹"),
    ("pole_tip", "pole_upper", (4.8, 31.8, 1.2), "杆顶悬挂尖端：弯曲向前的骨突尖端、粗锻生铁吊钩"),
    ("lantern_hanger", "pole_tip", (5.1, 29.8, 0.4), "灯笼吊绳：粗麻悬绳、骨环扣、兽骨灯笼顶盖与提梁"),
    ("lantern_cage", "lantern_hanger", (5.1, 27.5, 0.4), "兽骨灯笼笼身：六角镂空兽骨灯罩、侧棱柱、挡风骨翼"),
    ("lantern_core", "lantern_cage", (5.1, 27.3, 0.4), "灯芯高对比明焰：高对比金白烈焰核、炽热火舌体素（独立骨）"),
    ("lantern_tassel", "lantern_cage", (5.1, 25.0, 0.4), "灯底垂饰流苏：灯笼下底悬垂的小兽骨锥与碎皮须（独立骨）"),
    ("pole_trinkets", "pole_lower", (1.6, 20.5, 3.3), "骨杆中段饰物：悬绑在骨杆中部的辟邪骨币串与符片（独立骨）"),
]

# 不对称骨骼（斜插骨杆、灯笼总成与饰物）
ASYM_BONES = frozenset({
    "pole_socket", "pole_lower", "pole_upper", "pole_tip",
    "lantern_hanger", "lantern_cage", "lantern_core", "lantern_tassel", "pole_trinkets"
})


# ───────────────────────────────────────────────────────────── 几何部件构建
def part_harness_back(g: Gear) -> None:
    """背板总成：厚生皮背垫板、中线护脊带、定位铁铆片。"""
    b = "harness_back"
    # 后背表面位于 z = 2.0，所有背后部件严格位于 z ≥ 2.10！
    # 厚兽皮背垫板（x∈[-3.50, 3.50]，y∈[12.80, 23.20]，z∈[2.10, 2.36]）
    g.box(b, "back_pad_main", (-3.50, 12.80, 2.10), (3.50, 23.20, 2.36), "hide_dk")
    g.box(b, "back_pad_top", (-3.10, 22.80, 2.36), (3.10, 23.55, 2.62), "hide")
    g.box(b, "back_pad_bot", (-3.10, 12.45, 2.36), (3.10, 13.20, 2.62), "hide")

    # 中轴护脊加厚皮条（严格关于 x=0 对称）
    g.box(b, "back_spine_strip", (-0.95, 13.50, 2.36), (0.95, 22.50, 2.58), "strap_dk")
    g.box(b, "back_center_iron", (-0.65, 17.50, 2.58), (0.65, 18.50, 2.86), "iron")
    g.box(b, "back_center_rivet", (-0.18, 17.82, 2.86), (0.18, 18.18, 3.12), "iron_lt")

    # 左右边缘粗针脚加固缝线
    for k, y in enumerate((14.5, 16.5, 19.5, 21.5)):
        g.pair(b, f"back_stitch_{k}", (2.85, y, 2.36), (3.25, y + 0.60, 2.58), "cord_dk")


def part_harness_straps(g: Gear) -> None:
    """双肩背带：过肩宽皮带、胸前固定斜索、带扣与针脚。"""
    b = "harness_straps"
    # 双肩对称走线：
    # 后肩下沉段（x∈±[2.85, 3.75]，y∈[23.15, 23.85]，z∈[2.12, 2.38] 避开背垫板共面）
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        jx0, jx1 = min(sx * 2.85, sx * 3.75), max(sx * 2.85, sx * 3.75)
        g.box(b, f"b_strap_jog_{side}", (jx0, 23.15, 2.12), (jx1, 23.85, 2.38), "strap")
        # 肩部后立升段（x ≥ 4.10，避开头部）
        rx0, rx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"b_strap_riser_{side}", (rx0, 23.80, 2.08), (rx1, 24.45, 2.40), "strap_dk")
        # 肩峰骑越段（|x| ≥ 4.12，y ≥ 24.10 避开头部与手臂）
        ox0, ox1 = min(sx * 4.12, sx * 4.52), max(sx * 4.12, sx * 4.52)
        g.box(b, f"over_shoulder_{side}", (ox0, 24.10, -2.15), (ox1, 24.48, 2.15), "strap")
        # 肩垫护皮
        px0, px1 = min(sx * 4.16, sx * 4.56), max(sx * 4.16, sx * 4.56)
        g.box(b, f"shoulder_pad_{side}", (px0, 24.48, -1.65), (px1, 24.88, 1.65), "hide_lt")
        # 前肩下沉段（x ≥ 4.10）
        fx0, fx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"f_strap_riser_{side}", (fx0, 23.80, -2.42), (fx1, 24.45, -2.10), "strap_dk")
        # 前胸斜下段（从 y=23.85 下降到 y=14.50，z 严格 ≤ -2.10）
        g.box(b, f"f_strap_drop_{side}", (jx0, 14.50, -2.36), (jx1, 23.85, -2.08), "strap")
        # 胸前金属调节扣（左右严格镜像！）
        bx0, bx1 = (jx0 - 0.10, jx1 + 0.10) if sx > 0 else (jx0 - 0.10, jx1 + 0.10)
        g.box(b, f"f_buckle_frame_{side}", (min(bx0, bx1), 18.25, -2.52), (max(bx0, bx1), 19.15, -2.18), "iron")
        px_min, px_max = min(bx0 + 0.20, bx1 - 0.20), max(bx0 + 0.20, bx1 - 0.20)
        g.box(b, f"f_buckle_pin_{side}", (px_min, 18.52, -2.68), (px_max, 18.88, -2.40), "rust")

    # 胸前横向束带（连通左右肩带，对称中心）
    g.box(b, "chest_cross_band", (-2.85, 18.35, -2.34), (2.85, 18.95, -2.08), "strap_dk")
    g.box(b, "chest_bone_toggle", (-0.45, 18.15, -2.55), (0.45, 19.15, -2.25), "bone")


def part_pole_socket(g: Gear) -> None:
    """骨杆底座：斜插承托套筒、双层加固生铁箍、粗麻紧固缠绳。"""
    b = "pole_socket"
    # 底座位于背板左下偏中处（x∈[-2.35, -0.45]，y∈[13.2, 16.5]，z∈[2.36, 3.25]）
    # 斜插骨承座外壳（厚生铁包皮斗）
    g.box(b, "socket_body", (-2.25, 13.45, 2.38), (-0.55, 16.35, 3.12), "iron")
    # 顶部加固铁箍：上沿到 16.48，与内部骨杆重叠仅 0.28 (<0.55)，避免穿模违例
    g.box(b, "socket_rim_top", (-2.38, 16.12, 2.32), (-0.42, 16.48, 3.22), "iron_lt")
    g.box(b, "socket_rim_bot", (-2.38, 13.25, 2.32), (-0.42, 13.85, 3.22), "rust")
    # 底部斜向支撑骨角（加固底座）
    g.box(b, "socket_brace_l", (-2.52, 13.65, 2.45), (-2.18, 15.65, 3.05), "bone_stain")
    g.box(b, "socket_brace_r", (-0.62, 13.65, 2.45), (-0.28, 15.65, 3.05), "bone_stain")
    # 紧固铆钉
    g.box(b, "socket_rivet1", (-2.15, 16.18, 3.22), (-1.85, 16.42, 3.48), "iron_lt")
    g.box(b, "socket_rivet2", (-1.05, 16.18, 3.22), (-0.75, 16.42, 3.48), "iron_lt")

    # 缠绕在底座上的三圈粗麻绳
    g.box(b, "socket_cord_band", (-2.45, 14.45, 2.30), (-0.35, 15.35, 3.28), "cord_dk")
    g.box(b, "socket_cord_knot", (-1.65, 14.65, 3.28), (-1.15, 15.15, 3.65), "cord")


def part_pole_lower(g: Gear) -> None:
    """骨杆下段：斜跨背部的下部粗壮兽脊骨、分叉骨突。"""
    b = "pole_lower"
    # 从底座 (-1.4, 16.2, 2.7) 拔出，斜向右上跨越背部至 (2.5, 23.5, 3.5)
    # 第 1 节：基部骨节（y 从 16.20 开始，与底座箍重叠仅 0.28px，不触发穿模）
    g.box(b, "pole_seg1_core", (-1.85, 16.20, 2.52), (-0.75, 18.80, 3.15), "bone_stain")
    g.box(b, "pole_seg1_flange", (-2.05, 17.15, 2.45), (-0.55, 17.85, 3.25), "bone")
    # 第 2 节：中部过渡节
    g.box(b, "pole_seg2_core", (-0.95, 18.40, 2.65), (0.55, 21.20, 3.28), "bone")
    g.box(b, "pole_seg2_node", (-1.15, 19.45, 2.58), (0.75, 20.15, 3.38), "bone_lt")
    # 第 3 节：背部上截（y∈[20.8, 23.6]，z∈[2.78, 3.42]）
    g.box(b, "pole_seg3_core", (0.35, 20.80, 2.78), (1.85, 23.60, 3.42), "bone")
    g.box(b, "pole_seg3_spine", (0.75, 21.85, 3.42), (1.45, 23.25, 3.82), "bone_lt")

    # 骨杆接合处铁箍
    g.box(b, "pole_iron_collar1", (-1.10, 18.55, 2.58), (0.65, 19.05, 3.32), "iron")
    g.box(b, "pole_iron_collar2", (0.25, 20.85, 2.72), (1.95, 21.35, 3.48), "rust")


def part_pole_upper(g: Gear) -> None:
    """骨杆上段：高高耸立在右肩上方的大弧度弯曲骨杆。"""
    b = "pole_upper"
    # 第 4 节过渡：在 y ≤ 23.88 时后倾至 z ≥ 4.15！
    # 低位过渡块（y ≤ 23.88，完全在头盒 y=24.0 之下）
    g.box(b, "pole_seg4_low", (1.65, 23.20, 3.20), (3.45, 23.88, 4.35), "bone")
    # 高位背截（y ≥ 23.88，z 严格 ≥ 4.15，完全在头盒后表面 z=4.0 之后！）
    g.box(b, "pole_seg4_core", (2.45, 23.88, 4.15), (3.65, 26.40, 4.75), "bone")
    g.box(b, "pole_seg4_jog", (3.25, 25.80, 4.18), (4.55, 27.60, 4.78), "bone")

    # 第 5 节：耸立骨杆柱身（位于 x ≥ 4.15 且 z ≥ 4.15，彻底避开头盒！）
    g.box(b, "pole_seg5_riser", (4.25, 27.20, 4.15), (5.15, 30.80, 4.75), "bone")
    g.box(b, "pole_seg5_crest", (4.35, 28.45, 4.85), (5.05, 30.15, 5.25), "bone_lt")
    # 第 6 节：顶部前弯弧钩（从 y=30.4 向前上方伸出至右肩外上方 x ≥ 4.15）
    g.box(b, "pole_seg6_arch", (4.22, 30.40, 2.25), (5.18, 32.60, 4.35), "bone_lt")
    g.box(b, "pole_seg6_forward", (4.25, 31.80, 0.75), (5.15, 33.10, 2.45), "bone")

    # 上截防裂铜丝缠束
    g.box(b, "pole_wire_wrap1", (4.18, 28.05, 4.10), (5.22, 28.45, 4.90), "cord")
    g.box(b, "pole_wire_wrap2", (4.18, 31.05, 2.20), (5.22, 31.45, 4.40), "cord_dk")


def part_pole_tip(g: Gear) -> None:
    """杆顶悬挂尖端：弯曲向前的骨突尖端、粗锻生铁吊钩。"""
    b = "pole_tip"
    # 悬挑于右肩前上方（x∈[4.25, 5.15]，y∈[31.8, 33.2]，z∈[0.05, 1.0]）
    g.box(b, "tip_bone_beak", (4.32, 32.15, 0.15), (4.98, 32.95, 0.85), "bone_stain")
    # 锻打生铁吊环与下悬钩
    g.box(b, "tip_hook_socket", (4.25, 32.25, 0.05), (5.05, 32.75, 0.45), "iron")
    g.box(b, "tip_hook_loop", (4.45, 31.45, -0.05), (4.85, 32.35, 0.35), "iron_lt")
    g.box(b, "tip_hook_claw", (4.48, 31.22, 0.12), (4.82, 31.75, 0.55), "rust")


def part_lantern_hanger(g: Gear) -> None:
    """灯笼吊绳：粗麻悬绳、骨环扣、兽骨灯笼顶盖与提梁。"""
    b = "lantern_hanger"
    # 从吊钩 (4.65, 31.35, 0.15) 垂直下垂至灯笼顶盖 (y∈[29.2, 31.35])
    # 悬索位于 x ≈ 4.65，z ≈ 0.25（严格在 x ≥ 4.40，y ≥ 24.50）
    g.box(b, "hang_rope_top", (4.50, 30.55, 0.10), (4.80, 31.45, 0.40), "cord")
    g.box(b, "hang_rope_bot", (4.54, 29.85, 0.14), (4.76, 30.65, 0.36), "cord_dk")
    # 粗骨调节环珠
    g.box(b, "hang_bone_ring", (4.42, 30.32, 0.02), (4.88, 30.88, 0.48), "bone_lt")

    # 灯笼顶盖：六角形削角骨板（顶端锥形，y∈[28.85, 29.85]，x∈[4.35, 5.85]，z∈[-0.45, 1.05]）
    g.box(b, "lantern_roof_core", (4.45, 29.05, -0.35), (5.75, 29.75, 0.95), "bone")
    g.box(b, "lantern_roof_crest", (4.75, 29.65, -0.05), (5.45, 30.15, 0.65), "bone_stain")
    # 飞檐四角骨牙角
    g.box(b, "roof_corner_f", (4.35, 28.85, -0.55), (4.85, 29.25, -0.15), "bone_lt")
    g.box(b, "roof_corner_b", (4.35, 28.85, 0.75), (4.85, 29.25, 1.15), "bone_lt")
    g.box(b, "roof_corner_r", (5.35, 28.85, 0.10), (5.85, 29.25, 0.50), "bone_lt")


def part_lantern_cage(g: Gear) -> None:
    """兽骨灯笼笼身：六角镂空兽骨灯罩、侧棱柱、挡风骨翼。"""
    b = "lantern_cage"
    # 笼身尺寸：x∈[4.42, 5.78]，y∈[26.15, 29.05]，z∈[-0.38, 0.98]
    # 笼身四角立柱（镂空结构，四根兽骨角柱立在四周）
    g.box(b, "cage_post_fl", (4.42, 26.35, -0.38), (4.78, 29.05, -0.02), "bone")
    g.box(b, "cage_post_fr", (5.42, 26.35, -0.38), (5.78, 29.05, -0.02), "bone")
    g.box(b, "cage_post_bl", (4.42, 26.35, 0.62), (4.78, 29.05, 0.98), "bone_dk")
    g.box(b, "cage_post_br", (5.42, 26.35, 0.62), (5.78, 29.05, 0.98), "bone_dk")

    # 镂空侧壁骨条格栅
    g.box(b, "cage_cross_f", (4.72, 27.45, -0.35), (5.48, 27.85, -0.10), "bone_lt")
    g.box(b, "cage_cross_b", (4.72, 27.45, 0.70), (5.48, 27.85, 0.95), "bone_stain")
    g.box(b, "cage_cross_l", (4.45, 27.45, -0.08), (4.72, 27.85, 0.68), "bone_lt")
    g.box(b, "cage_cross_r", (5.48, 27.40, -0.08), (5.75, 27.80, 0.68), "bone_stain")

    # 灯笼底托板
    g.box(b, "lantern_floor_main", (4.45, 25.85, -0.35), (5.75, 26.45, 0.95), "bone_stain")
    g.box(b, "lantern_floor_lip", (4.35, 26.15, -0.45), (5.85, 26.55, 1.05), "bone")
    # 底托四角向下倒挂的兽骨短刺
    g.box(b, "floor_spike_fl", (4.38, 25.45, -0.42), (4.68, 26.15, -0.12), "bone_lt")
    g.box(b, "floor_spike_fr", (5.52, 25.45, -0.42), (5.82, 26.15, -0.12), "bone_lt")
    g.box(b, "floor_spike_bl", (4.38, 25.45, 0.72), (4.68, 26.15, 1.02), "bone_stain")
    g.box(b, "floor_spike_br", (5.52, 25.45, 0.72), (5.82, 26.15, 1.02), "bone_stain")
    g.box(b, "cage_inner_void", (4.75, 26.42, -0.05), (5.45, 28.95, 0.65), "void")


def part_lantern_core(g: Gear) -> None:
    """灯芯高对比明焰：高对比金白烈焰核、炽热火舌体素（独立骨）。"""
    b = "lantern_core"
    # 位于笼身中央镂空空间 (x ≈ 5.10, y ≈ 27.40, z ≈ 0.30)
    g.box(b, "flame_core_hot", (4.92, 26.95, 0.12), (5.28, 27.75, 0.48), "glow_core")
    g.box(b, "flame_flange_bot", (4.85, 26.65, 0.05), (5.35, 27.15, 0.55), "glow_flame")
    g.box(b, "flame_tongue_top", (4.95, 27.65, 0.15), (5.25, 28.35, 0.45), "glow_core")
    # 侧向飘逸火星尖
    g.box(b, "flame_flicker_r", (5.25, 27.25, 0.20), (5.55, 27.88, 0.42), "glow_flame")
    g.box(b, "flame_flicker_l", (4.65, 27.15, 0.18), (4.95, 27.65, 0.40), "glow_flame")


def part_lantern_tassel(g: Gear) -> None:
    """灯底垂饰流苏：灯笼下底悬垂的小兽骨锥与碎皮须（独立骨，严格 y ≥ 24.15 避开手臂）。"""
    b = "lantern_tassel"
    # 从灯底中央 (5.10, 25.85, 0.30) 向下悬垂至 y=24.20（严格高于手臂 y=24.00）
    g.box(b, "tassel_cord", (4.96, 25.40, 0.16), (5.24, 25.95, 0.44), "cord_dk")
    g.box(b, "tassel_knot", (4.90, 25.08, 0.10), (5.30, 25.48, 0.50), "cord")

    # 悬垂的镇风兽骨小吊锥（长约 1.2px，底部停在 24.20）
    g.box(b, "tassel_bone_root", (4.94, 24.65, 0.14), (5.26, 25.12, 0.46), "bone_stain")
    g.box(b, "tassel_bone_body", (4.96, 24.35, 0.16), (5.24, 24.75, 0.44), "bone")
    g.box(b, "tassel_bone_tip", (4.98, 24.15, 0.18), (5.22, 24.45, 0.42), "bone_lt")

    # 骨锥两侧垂落的两根细皮条须（厚度 ≥ 0.25，Z 错开）
    g.box(b, "tassel_strand_a", (4.78, 24.22, 0.12), (5.04, 25.05, 0.40), "strap_dk")
    g.box(b, "tassel_strand_b", (5.16, 24.22, 0.20), (5.42, 25.05, 0.48), "strap")


def part_pole_trinkets(g: Gear) -> None:
    """骨杆中段饰物：悬绑在骨杆中部的辟邪骨币串与符片（独立骨）。"""
    b = "pole_trinkets"
    # 捆绑在杆身的麻绳圈（错开共面）
    g.box(b, "trinket_cord_ring", (0.82, 21.00, 2.80), (1.68, 21.40, 3.60), "cord")
    g.box(b, "trinket_cord_drop", (1.32, 20.15, 3.32), (1.62, 21.15, 3.62), "cord_dk")

    # 穿孔兽骨币（方孔圆币，厚 0.28px）
    g.box(b, "bone_coin_body", (1.15, 19.25, 3.38), (1.85, 20.25, 3.68), "bone_stain")
    g.box(b, "bone_coin_void", (1.38, 19.58, 3.35), (1.64, 19.92, 3.72), "void")

    # 骨币下挂的一片削尖残符骨签
    g.box(b, "charm_tag_body", (1.25, 18.05, 3.36), (1.75, 19.35, 3.66), "bone")
    g.box(b, "charm_tag_tip", (1.35, 17.45, 3.38), (1.65, 18.15, 3.64), "bone_lt")
    # 副骨币（并排错位悬挂）
    g.box(b, "bone_coin2_body", (0.85, 18.65, 3.35), (1.35, 19.45, 3.62), "bone_lt")
    g.box(b, "bone_coin2_void", (0.97, 18.92, 3.32), (1.23, 19.18, 3.65), "void")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "harness_back": part_harness_back,
    "harness_straps": part_harness_straps,
    "pole_socket": part_pole_socket,
    "pole_lower": part_pole_lower,
    "pole_upper": part_pole_upper,
    "pole_tip": part_pole_tip,
    "lantern_hanger": part_lantern_hanger,
    "lantern_cage": part_lantern_cage,
    "lantern_core": part_lantern_core,
    "lantern_tassel": part_lantern_tassel,
    "pole_trinkets": part_pole_trinkets,
}

SPEC = GearSpec(
    gear_id="gu_deng_gua_gan",
    model_name="GuDengGuaGan",
    title="骨灯挂杆 / gu_deng_gua_gan",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：背（高出肩） · 粗兽骨斜插挂杆+六角镂空骨灯笼+明焰灯芯",
    textured=False,            # Round 1 纯几何
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
