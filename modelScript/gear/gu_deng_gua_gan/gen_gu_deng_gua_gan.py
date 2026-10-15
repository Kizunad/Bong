#!/usr/bin/env python3
"""骨灯挂杆（gu_deng_gua_gan）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工重构。

任务卡：末土装备 #20（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

调度审返工要求：
- 骨灯笼放大到约 6×8×6（大概有头那么大），做出完整的骨条笼架、飞檐角、雕花挡风骨板和多层高对比明亮灯芯（灯芯用高对比金白烈焰）；
- 骨杆粗壮加厚，分节清晰带明显骨节、粗麻捆扎绳、生铁紧固箍、斜撑支架；
- 背后增加完整的兽皮背包底座（包含鼓起皮囊、翻盖、挂扣、捆扎卷垫）；
- 骨杆中段绑辟邪骨币串与长符片，灯笼底座下垂镇风兽骨小吊锥与长皮须；
- 方块数做到 250 以上，门禁与差分自证全绿。

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
    "cord":        (144, 128, 78),     # 吊绳 / 扎绑麻绳
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
    ("pack_base", "harness_back", (0.0, 16.5, 3.2), "背部兽皮背包底座：鼓肚储物大皮囊、双层翻盖、捆扎卷毯"),
    ("harness_straps", "root", (0.0, 18.0, -2.25), "双肩背带：过肩宽皮带、胸前固定斜索、带扣与双排针脚"),
    ("pole_socket", "harness_back", (-1.2, 14.5, 3.5), "骨杆底座：斜插承托套筒、双层加固生铁箍、斜撑骨角与粗麻紧固绳"),
    ("pole_lower", "pole_socket", (0.5, 18.5, 3.9), "骨杆下段：斜跨背部的粗壮大兽脊骨、分节粗骨节、分叉骨突"),
    ("pole_upper", "pole_lower", (3.8, 26.5, 4.4), "骨杆上段：高高耸立在右肩上方的大弧度粗骨杆、骨节与骨裂纹"),
    ("pole_tip", "pole_upper", (5.1, 32.8, 1.2), "杆顶悬挂尖端：弯曲向前的尖锐骨嘴、粗锻生铁重吊钩"),
    ("lantern_hanger", "pole_tip", (5.2, 30.5, 0.4), "大灯笼吊绳：粗麻双悬绳、雕花骨环扣、六角兽骨顶盖与飞檐"),
    ("lantern_cage", "lantern_hanger", (5.2, 27.2, 0.4), "6x8x6大骨灯笼笼身：宽大镂空兽骨灯罩、六角立柱、骨条格栅、厚底板"),
    ("lantern_core", "lantern_cage", (5.2, 27.0, 0.4), "高对比明焰灯芯：高对比金白烈焰核、多层跃动火舌体素（独立骨）"),
    ("lantern_tassel", "lantern_cage", (5.2, 24.8, 0.4), "灯底垂饰流苏：灯笼下底悬垂的长兽骨吊锥与碎皮须（独立骨）"),
    ("pole_trinkets", "pole_lower", (1.6, 20.5, 4.2), "骨杆中段饰物：悬绑在骨杆中部的辟邪双骨币串与长符片（独立骨）"),
]

# 不对称骨骼（斜插骨杆、灯笼总成与饰物）
ASYM_BONES = frozenset({
    "pack_base", "pole_socket", "pole_lower", "pole_upper", "pole_tip",
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
    g.box(b, "back_center_iron", (-0.65, 17.50, 2.52), (0.65, 18.50, 2.82), "iron")
    g.box(b, "back_center_rivet", (-0.18, 17.82, 2.82), (0.18, 18.18, 3.12), "iron_lt")

    # 背垫板边缘针脚与包角铜皮（成对镜像）
    for idx, (cx, cy) in enumerate(((3.25, 13.15), (3.25, 22.85))):
        g.pair(b, f"back_corner_iron_{idx}", (cx - 0.22, cy - 0.22, 2.32), (cx + 0.22, cy + 0.22, 2.65), "iron_lt")
        g.pair(b, f"back_corner_rivet_{idx}", (cx - 0.12, cy - 0.12, 2.65), (cx + 0.12, cy + 0.12, 2.85), "rust")
    for k, y in enumerate((14.5, 16.5, 19.5, 21.5)):
        g.pair(b, f"back_stitch_{k}", (2.85, y, 2.36), (3.25, y + 0.60, 2.58), "cord_dk")


def part_pack_base(g: Gear) -> None:
    """背部兽皮背包底座：鼓肚储物大皮囊、双层翻盖、捆扎卷毯。"""
    b = "pack_base"
    # 增加在背托板上的饱满兽皮背包（x∈[-3.10, 2.45]，y∈[13.40, 21.20]，z∈[2.58, 3.85]）
    # 增加在背托板上的饱满兽皮背包（强化大容量与多挂扣）
    # 大皮包主仓
    g.box(b, "pack_body_main", (-3.05, 14.15, 2.62), (2.25, 20.45, 3.75), "hide")
    g.box(b, "pack_body_bot", (-2.92, 13.48, 2.65), (2.12, 14.25, 3.65), "hide_dk")
    g.box(b, "pack_body_front", (-2.85, 14.48, 3.75), (2.05, 19.85, 3.98), "hide_lt")
    g.box(b, "pack_body_flange1", (-3.12, 15.25, 2.75), (-2.88, 19.25, 3.65), "hide_dk")
    g.box(b, "pack_body_flange2", (2.08, 15.25, 2.75), (2.32, 19.25, 3.65), "hide_dk")

    # 大翻盖
    g.box(b, "pack_flap_top", (-3.15, 19.82, 2.55), (2.35, 20.85, 3.88), "hide_dk")
    g.box(b, "pack_flap_lip", (-2.75, 18.45, 3.85), (1.95, 20.15, 4.08), "hide_lt")
    g.box(b, "pack_flap_ridge", (-2.65, 19.15, 4.08), (1.85, 19.55, 4.28), "hide")

    # 双锁紧皮带与骨栓扣
    g.box(b, "pack_strap1", (-2.25, 14.05, 3.72), (-1.85, 20.35, 4.12), "strap_dk")
    g.box(b, "pack_strap2", (0.55, 14.05, 3.72), (0.95, 20.35, 4.12), "strap_dk")
    g.box(b, "pack_toggle1", (-2.35, 17.85, 4.12), (-1.75, 18.35, 4.35), "bone_lt")
    g.box(b, "pack_toggle2", (0.45, 17.85, 4.12), (1.05, 18.35, 4.35), "bone_lt")

    # 背包侧挂小皮袋与小水囊
    g.box(b, "pack_side_pouch", (-3.35, 15.55, 2.85), (-2.95, 18.25, 3.55), "hide")
    g.box(b, "pack_side_flap", (-3.42, 17.85, 2.80), (-2.90, 18.55, 3.60), "hide_lt")
    g.box(b, "pack_side_toggle", (-3.52, 16.95, 3.10), (-3.30, 17.45, 3.35), "bone_stain")

    # 背包底部横向捆扎的卷毯/兽皮卷
    g.box(b, "roll_bedroll", (-2.95, 12.25, 2.75), (2.15, 13.55, 3.85), "hide_dk")
    g.box(b, "roll_strap1", (-2.15, 12.15, 2.68), (-1.85, 13.65, 3.92), "strap")
    g.box(b, "roll_strap2", (0.85, 12.15, 2.68), (1.15, 13.65, 3.92), "strap")
    g.box(b, "roll_buckle1", (-2.22, 12.65, 3.92), (-1.78, 13.15, 4.15), "iron_lt")
    g.box(b, "roll_buckle2", (0.78, 12.65, 3.92), (1.22, 13.15, 4.15), "iron_lt")
    # 卷毯两端包皮与绳结（厚度 ≥ 0.25）
    g.box(b, "roll_cap_l", (-3.22, 12.35, 2.82), (-2.92, 13.45, 3.78), "hide_lt")
    g.box(b, "roll_cap_r", (2.12, 12.35, 2.82), (2.42, 13.45, 3.78), "hide_lt")
    g.box(b, "roll_knot1", (-2.22, 13.35, 3.82), (-1.78, 13.75, 4.18), "cord")
    g.box(b, "roll_knot2", (0.78, 13.35, 3.82), (1.22, 13.75, 4.15), "cord")

    # 翻盖两侧骨护角
    g.box(b, "pack_flap_corner1", (-3.22, 19.45, 3.78), (-2.85, 20.25, 4.12), "bone_stain")
    g.box(b, "pack_flap_corner2", (1.95, 19.45, 3.78), (2.32, 20.25, 4.12), "bone_stain")

    # 背包皮带缝线（厚度 ≥ 0.24，避开共面）
    g.box(b, "pack_stitch1_a", (-2.18, 15.05, 4.12), (-1.92, 15.45, 4.36), "cord_dk")
    g.box(b, "pack_stitch1_b", (-2.18, 16.55, 4.12), (-1.92, 16.95, 4.36), "cord_dk")
    g.box(b, "pack_stitch1_c", (-2.18, 18.55, 4.12), (-1.92, 18.95, 4.36), "cord_dk")
    g.box(b, "pack_stitch2_a", (0.58, 15.05, 4.12), (0.84, 15.45, 4.36), "cord_dk")
    g.box(b, "pack_stitch2_b", (0.58, 16.45, 4.12), (0.84, 16.85, 4.36), "cord_dk")
    g.box(b, "pack_stitch2_c", (0.58, 18.55, 4.12), (0.84, 18.95, 4.36), "cord_dk")

    # 背包右侧挂的一捆细麻绳
    g.box(b, "pack_rope_coil_main", (2.35, 15.65, 2.92), (2.75, 17.85, 3.70), "cord")
    g.box(b, "pack_rope_coil_band", (2.30, 16.55, 2.85), (2.80, 16.95, 3.76), "strap_dk")
    g.box(b, "pack_rope_coil_drop", (2.42, 14.85, 3.12), (2.68, 15.75, 3.48), "cord_dk")


def part_harness_straps(g: Gear) -> None:
    """双肩背带：过肩宽皮带、胸前固定斜索、带扣与双排针脚。"""
    b = "harness_straps"
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        jx0, jx1 = min(sx * 2.85, sx * 3.75), max(sx * 2.85, sx * 3.75)
        g.box(b, f"b_strap_jog_{side}", (jx0, 23.15, 2.12), (jx1, 23.85, 2.38), "strap")
        rx0, rx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"b_strap_riser_{side}", (rx0, 23.80, 2.08), (rx1, 24.45, 2.40), "strap_dk")
        ox0, ox1 = min(sx * 4.12, sx * 4.52), max(sx * 4.12, sx * 4.52)
        g.box(b, f"over_shoulder_{side}", (ox0, 24.10, -2.15), (ox1, 24.48, 2.15), "strap")
        px0, px1 = min(sx * 4.16, sx * 4.56), max(sx * 4.16, sx * 4.56)
        g.box(b, f"shoulder_pad_{side}", (px0, 24.48, -1.65), (px1, 24.88, 1.65), "hide_lt")
        fx0, fx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"f_strap_riser_{side}", (fx0, 23.80, -2.42), (fx1, 24.45, -2.10), "strap_dk")
        g.box(b, f"f_strap_drop_{side}", (jx0, 14.50, -2.36), (jx1, 23.85, -2.08), "strap")
        # 前胸金属调节扣
        bx0, bx1 = (jx0 - 0.10, jx1 + 0.10) if sx > 0 else (jx0 - 0.10, jx1 + 0.10)
        g.box(b, f"f_buckle_frame_{side}", (min(bx0, bx1), 18.25, -2.52), (max(bx0, bx1), 19.15, -2.18), "iron")
        px_min, px_max = min(bx0 + 0.20, bx1 - 0.20), max(bx0 + 0.20, bx1 - 0.20)
        g.box(b, f"f_buckle_pin_{side}", (px_min, 18.52, -2.68), (px_max, 18.88, -2.40), "rust")

    # 双肩背带针脚与前后皮环（严格镜像成对）
    for k, y in enumerate((15.5, 17.0, 19.8, 21.2, 22.5)):
        g.pair(b, f"f_strap_stitch_{k}", (3.05, y, -2.44), (3.55, y + 0.35, -2.15), "cord")
    g.pair(b, "strap_collar_f", (3.05, 23.25, -2.48), (3.55, 23.65, -2.05), "hide_dk")
    g.pair(b, "strap_collar_b", (3.05, 23.25, 2.05), (3.55, 23.65, 2.48), "hide_dk")
    g.pair(b, "strap_tongue_drop", (3.15, 16.55, -2.48), (3.45, 18.25, -2.20), "strap_dk")
    g.pair(b, "strap_tongue_tip", (3.18, 16.05, -2.46), (3.42, 16.65, -2.22), "strap")

    # 胸前横向束带与骨栓扣
    g.box(b, "chest_cross_band", (-2.85, 18.35, -2.34), (2.85, 18.95, -2.08), "strap_dk")
    g.box(b, "chest_bone_toggle", (-0.45, 18.15, -2.55), (0.45, 19.15, -2.25), "bone")
    g.box(b, "chest_band_knot", (-0.35, 18.42, -2.62), (0.35, 18.88, -2.32), "cord_dk")
    g.pair(b, "chest_band_rivet", (1.45, 18.45, -2.45), (1.75, 18.85, -2.15), "iron_lt")


def part_pole_socket(g: Gear) -> None:
    """骨杆底座：斜插承托套筒、双层加固生铁箍、斜撑骨角与粗麻紧固绳。"""
    b = "pole_socket"
    # 底座位于背包后侧左偏位置（x∈[-2.55, -0.65]，y∈[13.4, 17.2]，z∈[3.25, 4.35]）
    g.box(b, "socket_body", (-2.45, 13.55, 3.32), (-0.75, 16.55, 4.25), "iron")
    g.box(b, "socket_rim_top", (-2.58, 16.12, 3.25), (-0.62, 16.68, 4.32), "iron_lt")
    g.box(b, "socket_rim_bot", (-2.58, 13.25, 3.25), (-0.62, 13.85, 4.32), "rust")
    # 斜撑坚硬兽骨角与额外加固筋
    g.box(b, "socket_brace_l", (-2.75, 13.65, 3.35), (-2.38, 15.85, 4.05), "bone_stain")
    g.box(b, "socket_brace_r", (-0.82, 13.65, 3.35), (-0.45, 15.85, 4.05), "bone_stain")
    g.box(b, "socket_wedge_t", (-2.15, 15.82, 3.28), (-1.05, 16.32, 4.15), "bone_lt")
    g.box(b, "socket_wedge_b", (-2.10, 13.62, 3.28), (-1.05, 14.12, 4.15), "bone")
    # 紧固铆钉
    g.box(b, "socket_rivet1", (-2.25, 16.18, 4.28), (-1.95, 16.45, 4.52), "iron_lt")
    g.box(b, "socket_rivet2", (-1.25, 16.18, 4.28), (-0.95, 16.45, 4.52), "iron_lt")

    # 缠绕在底座上的多圈粗麻绳
    g.box(b, "socket_cord_band", (-2.65, 14.45, 3.22), (-0.55, 15.35, 4.38), "cord_dk")
    g.box(b, "socket_cord_knot", (-1.85, 14.65, 4.38), (-1.35, 15.15, 4.75), "cord")


def part_pole_lower(g: Gear) -> None:
    """骨杆下段：斜跨背部的粗壮大兽脊骨、分节粗骨节、分叉骨突。"""
    b = "pole_lower"
    # 加粗的粗壮兽脊骨（骨宽达 1.4px！），斜跨背包后侧
    # 粗壮分节大骨杆（大幅增加骨节与外附骨刺细节）
    # 第 1 节：基部骨节（y∈[16.4, 19.2]）
    g.box(b, "pole_seg1_core", (-2.05, 16.45, 3.42), (-0.85, 19.20, 4.22), "bone_stain")
    g.box(b, "pole_seg1_flange", (-2.28, 17.35, 3.35), (-0.62, 18.25, 4.32), "bone")
    g.box(b, "pole_seg1_thorn", (-2.45, 17.55, 3.65), (-2.15, 18.20, 4.05), "bone_lt")
    g.box(b, "pole_seg1_wrap", (-2.12, 16.65, 3.38), (-0.78, 17.05, 4.28), "cord_dk")

    # 第 2 节：中部大脊骨节（y∈[18.8, 21.8]）
    g.box(b, "pole_seg2_core", (-1.15, 18.80, 3.55), (0.75, 21.80, 4.35), "bone")
    g.box(b, "pole_seg2_node", (-1.38, 19.85, 3.45), (0.98, 20.75, 4.45), "bone_lt")
    g.box(b, "pole_seg2_flange_l", (-1.55, 20.15, 3.75), (-1.25, 20.80, 4.15), "bone_stain")
    g.box(b, "pole_seg2_flange_r", (0.85, 20.15, 3.75), (1.15, 20.80, 4.15), "bone_stain")
    g.box(b, "pole_seg2_wrap", (-1.22, 19.35, 3.50), (0.82, 19.75, 4.40), "cord")

    # 第 3 节：背部上截粗骨
    g.box(b, "pole_seg3_core", (0.45, 21.40, 3.68), (2.25, 23.82, 4.48), "bone")
    g.box(b, "pole_seg3_spine", (0.95, 22.45, 4.48), (1.75, 23.82, 5.08), "bone_lt")
    g.box(b, "pole_seg3_thorn_b", (0.65, 22.85, 3.45), (1.15, 23.45, 3.75), "bone_lt")
    g.box(b, "pole_seg3_wrap", (0.38, 21.85, 3.62), (2.32, 22.25, 4.54), "cord_dk")

    # 粗生铁加固箍与麻绳缠束
    g.box(b, "pole_iron_collar1", (-1.32, 18.95, 3.45), (0.88, 19.52, 4.42), "iron")
    g.box(b, "pole_iron_collar2", (0.35, 21.45, 3.58), (2.35, 22.05, 4.58), "rust")


def part_pole_upper(g: Gear) -> None:
    """骨杆上段：高高耸立在右肩上方的大弧度粗骨杆、骨节与骨裂纹。"""
    b = "pole_upper"
    # 第 4 节：肩峰上行段（在 y ≤ 23.85 时，z 保持 ≥ 4.18，y ≥ 23.85 时 z ≥ 4.25 彻底避开头盒）
    g.box(b, "pole_seg4_low", (1.95, 23.40, 4.18), (3.65, 23.85, 4.85), "bone")
    g.box(b, "pole_seg4_core", (2.65, 23.85, 4.25), (4.15, 26.80, 5.05), "bone")
    g.box(b, "pole_seg4_jog", (3.65, 26.20, 4.28), (4.95, 28.20, 5.08), "bone")

    # 第 5 节：高高耸立粗柱骨杆（增加骨节凸起与双侧骨刺）
    g.box(b, "pole_seg5_riser", (4.35, 27.80, 4.25), (5.55, 31.80, 4.75), "bone")
    g.box(b, "pole_seg5_crest", (4.55, 29.15, 4.75), (5.35, 30.95, 5.35), "bone_lt")
    g.box(b, "pole_seg5_node1", (4.22, 28.55, 4.18), (5.68, 29.35, 4.82), "bone_stain")
    g.box(b, "pole_seg5_node2", (4.22, 30.25, 4.18), (5.68, 31.05, 4.82), "bone_stain")
    g.box(b, "pole_seg5_spike1", (5.55, 28.85, 4.35), (5.95, 29.55, 4.65), "bone_lt")
    g.box(b, "pole_seg5_spike2", (5.55, 30.45, 4.35), (5.95, 31.15, 4.65), "bone_lt")
    g.box(b, "pole_seg5_spike_back", (4.75, 29.85, 5.35), (5.15, 30.45, 5.85), "bone_lt")

    # 第 6 节：顶部前倾巨大弧钩（从 y=31.2 向前上方挑出，延伸至 z ≈ 0.2）
    g.box(b, "pole_seg6_arch", (4.38, 31.20, 2.15), (5.52, 33.40, 4.45), "bone_lt")
    g.box(b, "pole_seg6_forward", (4.42, 32.60, 0.45), (5.48, 34.10, 2.35), "bone")
    g.box(b, "pole_seg6_crest_f", (4.65, 33.65, 0.85), (5.25, 34.45, 1.85), "bone_lt")
    g.box(b, "pole_seg6_thorn1", (4.32, 32.85, 1.25), (4.55, 33.55, 1.75), "bone_stain")
    g.box(b, "pole_seg6_thorn2", (5.35, 32.85, 1.25), (5.58, 33.55, 1.75), "bone_stain")

    # 防裂铜丝与生铁夹箍
    g.box(b, "pole_wire_wrap1", (4.25, 28.62, 4.15), (5.65, 29.18, 5.25), "cord")
    g.box(b, "pole_wire_wrap2", (4.28, 31.85, 2.08), (5.62, 32.35, 4.52), "cord_dk")


def part_pole_tip(g: Gear) -> None:
    """杆顶悬挂尖端：弯曲向前的尖锐骨嘴、粗锻生铁重吊钩。"""
    b = "pole_tip"
    # 挑于右肩外前方（x∈[4.45, 5.45]，y∈[32.8, 34.2]，z∈[-0.15, 0.85]）
    g.box(b, "tip_bone_beak", (4.55, 33.15, -0.05), (5.35, 33.95, 0.75), "bone_stain")
    # 粗锻生铁重吊环与大下悬钩
    g.box(b, "tip_hook_socket", (4.45, 33.25, -0.15), (5.45, 33.85, 0.35), "iron")
    g.box(b, "tip_hook_loop", (4.72, 32.35, -0.28), (5.18, 33.45, 0.22), "iron_lt")
    g.box(b, "tip_hook_claw", (4.75, 32.05, -0.05), (5.15, 32.65, 0.45), "rust")


def part_lantern_hanger(g: Gear) -> None:
    """大灯笼吊绳：粗麻双悬绳、雕花骨环扣、六角兽骨顶盖与飞檐。"""
    b = "lantern_hanger"
    # 从吊钩 (4.95, 32.15, 0.05) 垂直下垂至大灯笼顶盖 (y∈[30.2, 32.15])
    g.box(b, "hang_rope_top", (4.82, 31.25, -0.12), (5.18, 32.25, 0.22), "cord")
    g.box(b, "hang_rope_bot", (4.84, 30.45, -0.10), (5.16, 31.35, 0.20), "cord_dk")
    # 雕花大骨环扣
    g.box(b, "hang_bone_ring", (4.72, 30.95, -0.22), (5.28, 31.55, 0.32), "bone_lt")

    # 6×8×6 大骨灯笼顶盖（宽达 6.2px，进深 6.2px！y∈[29.4, 30.8]）
    # (中心位于 x=5.00, z=0.05)
    g.box(b, "lantern_roof_core", (4.15, 29.55, -0.80), (5.85, 30.45, 0.90), "bone")
    g.box(b, "lantern_roof_cap", (4.45, 30.35, -0.50), (5.55, 30.85, 0.60), "bone_stain")
    g.box(b, "lantern_roof_ridge1", (4.08, 29.48, -0.88), (5.92, 29.80, 0.98), "bone_lt")
    g.box(b, "lantern_roof_ridge2", (4.32, 29.95, -0.62), (5.68, 30.42, 0.72), "bone_lt")
    # 顶部托骨细齿圈
    for idx, fx in enumerate((4.32, 4.62, 4.92, 5.22, 5.52)):
        g.box(b, f"roof_tooth_t_{idx}", (fx - 0.12, 30.75, -0.42), (fx + 0.12, 31.15, -0.15), "bone_lt")
        g.box(b, f"roof_tooth_b_{idx}", (fx - 0.12, 30.75, 0.15), (fx + 0.12, 31.15, 0.42), "bone_lt")
        g.box(b, f"floor_spike_f2_{idx}", (fx - 0.12, 24.95, -0.74), (fx + 0.12, 25.35, -0.45), "bone_stain")
        g.box(b, f"floor_spike_b2_{idx}", (fx - 0.12, 24.95, 0.45), (fx + 0.12, 25.35, 0.74), "bone_stain")

    # 飞檐四角大兽牙翘角
    g.box(b, "roof_horn_fl", (4.12, 29.35, -1.05), (4.65, 29.85, -0.55), "bone_lt")
    g.box(b, "roof_horn_fr", (5.35, 29.35, -1.05), (5.88, 29.85, -0.55), "bone_lt")
    g.box(b, "roof_horn_bl", (4.12, 29.35, 0.65), (4.65, 29.85, 1.15), "bone_lt")
    g.box(b, "roof_horn_br", (5.35, 29.35, 0.65), (5.88, 29.85, 1.15), "bone_lt")
    # 翘角尖端
    g.box(b, "roof_horn_tip_fl", (4.02, 29.65, -1.22), (4.45, 30.15, -0.75), "bone_lt")
    g.box(b, "roof_horn_tip_fr", (5.55, 29.65, -1.22), (5.98, 30.15, -0.75), "bone_lt")
    g.box(b, "roof_horn_tip_bl", (4.02, 29.65, 0.75), (4.45, 30.15, 1.22), "bone_stain")
    g.box(b, "roof_horn_tip_br", (5.55, 29.65, 0.75), (5.98, 30.15, 1.22), "bone_stain")


def part_lantern_cage(g: Gear) -> None:
    """6x8x6大骨灯笼笼身：宽大镂空兽骨灯罩、六角立柱、骨条格栅、厚底板。"""
    b = "lantern_cage"
    # 大灯笼笼身尺寸：x∈[4.18, 5.82]，y∈[25.80, 29.55]，z∈[-0.78, 0.88]（体量巨大如头盒！）
    # 四根角骨立柱
    g.box(b, "cage_post_fl", (4.18, 26.15, -0.78), (4.58, 29.55, -0.38), "bone")
    g.box(b, "cage_post_fr", (5.42, 26.15, -0.78), (5.82, 29.55, -0.38), "bone")
    g.box(b, "cage_post_bl", (4.18, 26.15, 0.48), (4.58, 29.55, 0.88), "bone_dk")
    g.box(b, "cage_post_br", (5.42, 26.15, 0.48), (5.82, 29.55, 0.88), "bone_dk")

    # 中部八方骨栏格栅（上下两道骨圈箍，Y 错开）
    g.box(b, "cage_hoop_top", (4.12, 28.52, -0.84), (5.88, 29.02, 0.94), "bone_stain")
    g.box(b, "cage_hoop_bot", (4.12, 26.42, -0.84), (5.88, 26.92, 0.94), "bone_stain")

    # 镂空挡风侧板与骨排竖栅
    g.box(b, "cage_slat_f1", (4.82, 26.95, -0.78), (5.18, 28.52, -0.48), "bone_lt")
    g.box(b, "cage_slat_b1", (4.82, 26.95, 0.58), (5.18, 28.52, 0.88), "bone_lt")
    g.box(b, "cage_slat_l1", (4.22, 26.95, -0.18), (4.52, 28.52, 0.28), "bone_lt")
    g.box(b, "cage_slat_r1", (5.48, 26.95, -0.18), (5.78, 28.52, 0.28), "bone_lt")

    # 细密斜向兽肋护骨格栅（增加细节）
    for idx, dy in enumerate((26.9, 27.5, 28.1)):
        g.box(b, f"cage_rib_fl_{idx}", (4.20, dy, -0.68), (4.50, dy + 0.35, -0.32), "bone_lt")
        g.box(b, f"cage_rib_fr_{idx}", (5.50, dy, -0.68), (5.80, dy + 0.35, -0.32), "bone_lt")
        g.box(b, f"cage_rib_bl_{idx}", (4.20, dy, 0.42), (4.50, dy + 0.35, 0.78), "bone_stain")
        g.box(b, f"cage_rib_br_{idx}", (5.50, dy, 0.42), (5.80, dy + 0.35, 0.78), "bone_stain")

    # 飞檐四角骨爪倒勾（x 严格 ≥ 4.14 避开头盒与共面！）
    for idx, (hx, hz) in enumerate(((4.30, -0.85), (5.90, -0.85), (4.30, 0.95), (5.90, 0.95))):
        g.box(b, f"cage_fang_{idx}", (hx - 0.10, 28.95, hz - 0.18), (hx + 0.18, 29.42, hz + 0.18), "bone_lt")
        g.box(b, f"cage_fang_tip_{idx}", (hx - 0.10, 29.45, hz - 0.22), (hx + 0.22, 29.82, hz + 0.22), "bone_stain")

    # 宽大六角老骨厚底盘
    g.box(b, "lantern_floor_main", (4.15, 25.45, -0.80), (5.85, 26.15, 0.90), "bone_stain")
    g.box(b, "lantern_floor_rim", (4.05, 25.85, -0.90), (5.95, 26.25, 1.00), "bone")
    g.box(b, "lantern_floor_skirt", (4.12, 25.22, -0.75), (5.88, 25.65, 0.80), "bone_dk")
    # 底部托骨牙圈
    for idx, fx in enumerate((4.35, 4.75, 5.25, 5.65)):
        g.box(b, f"floor_slat_f_{idx}", (fx - 0.12, 25.25, -0.85), (fx + 0.12, 25.75, -0.55), "bone_lt")
        g.box(b, f"floor_slat_b_{idx}", (fx - 0.12, 25.25, 0.55), (fx + 0.12, 25.75, 0.85), "bone_lt")

    # 内部透光镂空空隙
    g.box(b, "cage_inner_void", (4.62, 26.25, -0.32), (5.38, 28.95, 0.42), "void")


def part_lantern_core(g: Gear) -> None:
    """高对比明焰灯芯：高对比金白烈焰核、多层跃动火舌体素（独立骨）。"""
    b = "lantern_core"
    # 位于大笼身正中央 (x ≈ 5.00, y ≈ 27.50, z ≈ 0.05)
    # 灯芯底座骨托与定位铁铆
    g.box(b, "flame_base_socket", (4.78, 26.15, -0.12), (5.22, 26.52, 0.22), "bone_stain")
    g.box(b, "flame_base_rivet", (4.88, 26.26, 0.14), (5.12, 26.54, 0.38), "iron_lt")

    # 中心极亮金白灯芯烈火核心
    g.box(b, "flame_core_hot", (4.72, 26.85, -0.22), (5.28, 27.95, 0.32), "glow_core")
    # 外围炽热烈焰外壳（尺寸控制在 0.52 以内，避免互穿）
    g.box(b, "flame_flange_bot", (4.75, 26.48, -0.18), (5.25, 27.25, 0.28), "glow_flame")
    g.box(b, "flame_tongue_top1", (4.82, 27.85, -0.12), (5.18, 28.65, 0.22), "glow_core")
    g.box(b, "flame_tongue_top2", (4.88, 28.58, -0.06), (5.12, 29.15, 0.16), "glow_flame")
    # 四向飘逸飞溅火舌
    g.box(b, "flame_flicker_f", (4.82, 27.25, -0.42), (5.18, 27.75, -0.18), "glow_flame")
    g.box(b, "flame_flicker_b", (4.82, 27.25, 0.28), (5.18, 27.75, 0.52), "glow_flame")
    g.box(b, "flame_flicker_r", (5.25, 27.25, -0.12), (5.55, 27.85, 0.22), "glow_flame")
    g.box(b, "flame_flicker_l", (4.45, 27.25, -0.12), (4.75, 27.85, 0.22), "glow_flame")


def part_lantern_tassel(g: Gear) -> None:
    """灯底垂饰流苏：灯笼下底悬垂的长兽骨吊锥与碎皮须（独立骨，严格 y ≥ 24.15 避开手臂）。"""
    b = "lantern_tassel"
    # 下垂大骨锥挂坠（从 25.45 下垂至 24.18，厚度 ≥ 0.25）
    g.box(b, "tassel_cord", (4.86, 25.02, -0.08), (5.14, 25.55, 0.18), "cord_dk")
    g.box(b, "tassel_knot", (4.80, 24.72, -0.14), (5.20, 25.12, 0.24), "cord")

    # 镇风巨兽吊齿锥（厚度 ≥ 0.25）
    g.box(b, "tassel_bone_root", (4.82, 24.52, -0.12), (5.18, 24.85, 0.22), "bone_stain")
    g.box(b, "tassel_bone_body", (4.85, 24.28, -0.09), (5.15, 24.62, 0.19), "bone")
    g.box(b, "tassel_bone_tip", (4.88, 24.12, -0.06), (5.12, 24.36, 0.16), "bone_lt")

    # 两侧分叉长皮条须
    g.box(b, "tassel_strand_l", (4.65, 24.22, -0.10), (4.90, 25.02, 0.20), "strap_dk")
    g.box(b, "tassel_strand_r", (5.10, 24.22, -0.10), (5.35, 25.02, 0.20), "strap")


def part_pole_trinkets(g: Gear) -> None:
    """骨杆中段饰物：悬绑在骨杆中部的辟邪双骨币串与长符片（独立骨）。"""
    b = "pole_trinkets"
    # 悬绑在骨杆中部 (x ≈ 1.35, y ≈ 20.8, z ≈ 4.1)
    g.box(b, "trinket_cord_ring", (0.92, 21.00, 3.55), (1.78, 21.40, 4.45), "cord")
    g.box(b, "trinket_cord_drop1", (1.42, 20.15, 4.02), (1.72, 21.15, 4.40), "cord_dk")
    g.box(b, "trinket_cord_drop2", (1.02, 19.85, 4.02), (1.32, 21.05, 4.40), "cord")

    # 穿孔兽骨币 1
    g.box(b, "bone_coin1_body", (1.25, 19.25, 4.10), (1.95, 20.25, 4.44), "bone_stain")
    g.box(b, "bone_coin1_void", (1.48, 19.58, 4.06), (1.74, 19.92, 4.48), "void")
    g.box(b, "bone_coin1_flange", (1.15, 19.55, 4.12), (2.05, 19.95, 4.42), "bone_lt")

    # 穿孔兽骨币 2（错开 Y 与 Z 坐标）
    g.box(b, "bone_coin2_body", (0.85, 18.58, 4.18), (1.45, 19.48, 4.48), "bone_lt")
    g.box(b, "bone_coin2_void", (1.02, 18.92, 4.12), (1.28, 19.22, 4.52), "void")
    g.box(b, "bone_coin2_flange", (0.75, 18.88, 4.20), (1.55, 19.28, 4.46), "bone_stain")

    # 辟邪第3骨币（厚度 ≥ 0.25）
    g.box(b, "bone_coin3_body", (1.15, 17.75, 4.22), (1.65, 18.55, 4.52), "bone")
    g.box(b, "bone_coin3_void", (1.28, 18.00, 4.18), (1.52, 18.30, 4.56), "void")

    # 下挂长符骨签
    g.box(b, "charm_tag_body", (1.32, 16.95, 4.15), (1.82, 18.15, 4.45), "bone")
    g.box(b, "charm_tag_tip", (1.42, 16.35, 4.18), (1.72, 17.05, 4.42), "bone_lt")
    g.box(b, "charm_tag_knot", (1.45, 17.95, 4.12), (1.68, 18.35, 4.48), "cord_dk")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "harness_back": part_harness_back,
    "pack_base": part_pack_base,
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
    wear_note="挂点：背（高出肩） · 粗兽骨斜插挂杆+6x8x6大骨灯笼+明焰灯芯+背部兽皮背包",
    textured=False,            # Round 1 纯几何
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
