#!/usr/bin/env python3
"""兽皮披肩（shou_pi_pi_jian）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土定制 NPC 专属装备 #6（bei_huang_lie_hu 北荒猎户）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，+x = 穿戴者右侧。

挂点与避让说明：
- 挂在双肩与上背（y∈[13.5, 28.5]）。
- 枯皮狼头搭在右肩（x ≥ 4.15，颅骨 y ≥ 24.12，吻部与下颌在右肩前 z ≤ -2.15），严格避开头盒（x∈[-4,4], y∈[24,32]）与右臂盒（y≤24，x≥4.0）。
- 上背披肩主体位于 y∈[13.5, 23.9]，z ≥ 2.15，覆于肩胛骨下方，下缘带密集参差毛齿与缝嵌六根枯兽肋骨排。
- 胸前两只狼爪挂在胸口两侧偏上（y∈[16.5, 22.8]），中间留出宽阔缝隙，为 NPC 复用装备皮囊胸挂（pi_nang_xiong_gua）与兽牙项链（shou_ya_xiang_lian）留出充足空间，绝不干涉。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/gear/shou_pi_pi_jian/gen_shou_pi_pi_jian.py --self-test
    python3 modelScript/gear/shou_pi_pi_jian/gen_shou_pi_pi_jian.py --gates
    python3 modelScript/gear/shou_pi_pi_jian/gen_shou_pi_pi_jian.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "fur":          (118, 98, 76),     # 枯皮狼毛褐灰底色
    "fur_dk":       (82, 66, 52),      # 深色毛簇与背阴
    "fur_lt":       (156, 140, 116),   # 浅色枯白毛尖
    "hide_bald":    (136, 82, 64),     # 脱毛露皮与风干肌膜
    "bone":         (216, 206, 178),   # 狼头颅骨与獠牙
    "bone_lt":      (238, 230, 208),   # 尖牙尖端
    "bone_dk":      (158, 144, 114),   # 骨槽与骨缝
    "claw":         (42, 36, 34),      # 尖锐黑角质狼爪
    "cord":         (146, 126, 82),    # 皮绳扎带
    "cord_dk":      (104, 88, 56),     # 粗绳结
    "stitch":       (58, 44, 32),      # 粗麻缝线
    "iron":         (84, 80, 82),      # 加固生铁箍
    "iron_lt":      (120, 116, 118),   # 铁铆钉
    "void":         (22, 18, 16),      # 空眼眶与鼻孔深处
}

SOFT = ("fur", "fur_dk", "fur_lt", "hide_bald", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("cape_back", "root", (0.0, 20.0, 2.45), "后背狼皮披风：覆于背部的多层厚毛皮披身（对称）"),
    ("back_scars", "cape_back", (0.0, 19.5, 2.50), "背部风化磨蚀残斑与补丁粗线（不对称）"),
    ("fur_fringe_b", "cape_back", (0.0, 15.5, 2.45), "后摆毛须撕边：下垂至肩胛骨下方的参差撕裂毛皮（可晃动）"),
    ("rib_plates", "cape_back", (0.0, 19.5, 2.55), "缝嵌狼肋骨片：缝在披肩背部的弧形枯兽肋骨排"),
    ("cape_l", "root", (-5.4, 24.5, 0.0), "左肩皮披垫：跨越左肩顶端与左前胸的覆皮"),
    ("cape_r", "root", (5.4, 24.5, 0.0), "右肩底皮托：右肩底垫、右胸垂布与固定带"),
    ("wolf_head", "cape_r", (5.6, 24.5, 0.0), "右肩狼头颅骨：覆在右肩上的枯皮狼颅骨、吻部、獠牙与空眼眶"),
    ("wolf_jaw", "wolf_head", (5.5, 22.8, -2.6), "下颌骨：悬挂在吻部下方的张开狼下颌与下排尖齿（可微动）"),
    ("wolf_crest", "wolf_head", (5.6, 26.5, 0.2), "狼耳与背脊残毛：立起的干瘪狼耳与头顶鬃毛（可晃动）"),
    ("paw_l", "cape_l", (-2.6, 19.5, -2.45), "左垂狼爪：垂在前胸左侧的枯狼前爪与黑角质勾爪（挂件独立骨）"),
    ("paw_r", "cape_r", (2.6, 19.5, -2.45), "右垂狼爪：垂在前胸右侧的枯狼前爪与勾爪（挂件独立骨）"),
    ("chest_tie", "root", (0.0, 22.0, -2.45), "前胸束带骨栓：连接左右前爪的交叉皮绳与束紧兽骨卡扣"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "cape_back"))


# ───────────────────────────────────────────────────────────── 分部件实现
def part_cape_back(g: Gear) -> None:
    """后背狼皮披风：覆于背部的多层厚毛皮披身（严格对称）。"""
    b = "cape_back"
    # 后背主披身（X∈[-4.4, 4.4], Y∈[16.2, 23.9], Z∈[2.15, 2.58]）
    g.box(b, "back_skin_t", (-4.35, 20.85, 2.18), (4.35, 23.88, 2.48), "fur")
    g.box(b, "back_skin_m", (-4.15, 18.25, 2.22), (4.15, 20.95, 2.52), "fur")
    g.box(b, "back_skin_b", (-3.85, 16.25, 2.25), (3.85, 18.35, 2.55), "fur_dk")

    # 左右外缘隆起毛皮厚边与折叠层
    g.box(b, "back_rim_l", (-4.45, 17.55, 2.15), (-4.05, 23.92, 2.55), "fur_lt")
    g.box(b, "back_rim_r", (4.05, 17.55, 2.15), (4.45, 23.92, 2.55), "fur_lt")
    g.box(b, "back_rim_fold_l", (-4.38, 18.55, 2.45), (-3.98, 23.55, 2.68), "fur_dk")
    g.box(b, "back_rim_fold_r", (3.98, 18.55, 2.45), (4.38, 23.55, 2.68), "fur_dk")

    # 狼皮背脊深色鬃毛中线
    g.box(b, "back_spine_fur1", (-0.45, 20.25, 2.45), (0.45, 23.85, 2.68), "fur_dk")
    g.box(b, "back_spine_fur2", (-0.42, 17.55, 2.48), (0.42, 20.35, 2.70), "fur_dk")
    g.box(b, "back_spine_ridge", (-0.22, 16.85, 2.52), (0.22, 23.65, 2.78), "fur_lt")
    g.box(b, "back_spine_tuft1", (-0.32, 21.85, 2.62), (0.32, 22.85, 2.86), "fur_lt")
    g.box(b, "back_spine_tuft2", (-0.32, 19.15, 2.62), (0.32, 20.15, 2.86), "fur_lt")

    g.box(b, "back_skin_fold_t_l", (-3.65, 22.05, 2.38), (-2.55, 23.45, 2.65), "fur_lt")
    g.box(b, "back_skin_fold_t_r", (2.55, 22.05, 2.38), (3.65, 23.45, 2.65), "fur_lt")
    g.box(b, "back_skin_fold_b_l", (-3.45, 17.05, 2.38), (-2.45, 18.25, 2.65), "fur_dk")
    g.box(b, "back_skin_fold_b_r", (2.45, 17.05, 2.38), (3.45, 18.25, 2.65), "fur_dk")
    g.box(b, "back_spine_knot1", (-0.35, 20.95, 2.68), (0.35, 21.45, 2.92), "cord_dk")
    g.box(b, "back_spine_knot2", (-0.35, 18.25, 2.68), (0.35, 18.75, 2.92), "cord_dk")


def part_back_scars(g: Gear) -> None:
    """背部风化磨蚀残斑与补丁粗线（不对称细节）。"""
    b = "back_scars"
    g.box(b, "back_bald_patch1", (-2.85, 19.45, 2.35), (-1.45, 21.65, 2.56), "hide_bald")
    g.box(b, "back_bald_patch2", (1.65, 18.05, 2.38), (2.95, 20.15, 2.58), "hide_bald")
    g.box(b, "back_bald_scar1", (-2.55, 19.95, 2.52), (-1.75, 20.55, 2.66), "fur_dk")
    g.box(b, "back_bald_scar2", (1.95, 18.55, 2.52), (2.65, 19.15, 2.66), "fur_dk")

    for k in range(5):
        y = 19.55 + k * 0.48
        g.box(b, f"back_patch_st1_{k}", (-2.95, y, 2.42), (-2.62, y + 0.28, 2.66), "stitch")
    for k in range(4):
        y = 18.15 + k * 0.52
        g.box(b, f"back_patch_st2_{k}", (2.82, y, 2.45), (3.15, y + 0.28, 2.68), "stitch")


def part_fur_fringe_b(g: Gear) -> None:
    """后摆毛须撕边：下垂至肩胛骨下方的参差撕裂毛皮（可晃动独立骨）。"""
    b = "fur_fringe_b"
    # 下垂撕条毛束（Y∈[13.5, 16.5]）
    fringes = (
        ("fr_b1", -3.65, -3.05, 14.85, 16.35, 2.22, 2.55, "fur"),
        ("fr_b1_tip", -3.55, -3.15, 14.15, 14.95, 2.25, 2.52, "fur_lt"),
        ("fr_b1_sub", -3.45, -3.22, 13.55, 14.25, 2.28, 2.50, "fur_dk"),
        ("fr_b2", -2.85, -2.25, 14.45, 16.35, 2.22, 2.55, "fur_dk"),
        ("fr_b2_tip", -2.75, -2.35, 13.85, 14.55, 2.25, 2.52, "fur_lt"),
        ("fr_b3", -2.05, -1.45, 14.95, 16.35, 2.22, 2.55, "fur"),
        ("fr_b3_tip", -1.95, -1.55, 14.25, 15.05, 2.25, 2.52, "fur_dk"),
        ("fr_b3_sub", -1.85, -1.62, 13.65, 14.35, 2.28, 2.50, "fur_lt"),
        ("fr_b4", -1.25, -0.65, 14.55, 16.35, 2.22, 2.55, "fur_lt"),
        ("fr_b4_tip", -1.15, -0.75, 13.95, 14.65, 2.25, 2.52, "fur_dk"),
        ("fr_b5", -0.45, 0.45, 14.25, 16.35, 2.25, 2.62, "fur_dk"),
        ("fr_b5_tip", -0.35, 0.35, 13.55, 14.35, 2.28, 2.58, "fur_lt"),
        ("fr_b5_sub", -0.25, 0.25, 12.85, 13.65, 2.30, 2.56, "fur_dk"),
        ("fr_b6", 0.65, 1.25, 14.65, 16.35, 2.22, 2.55, "fur"),
        ("fr_b6_tip", 0.75, 1.15, 14.05, 14.75, 2.25, 2.52, "fur_lt"),
        ("fr_b7", 1.45, 2.05, 14.45, 16.35, 2.22, 2.55, "fur_dk"),
        ("fr_b7_tip", 1.55, 1.95, 13.85, 14.55, 2.25, 2.52, "fur_lt"),
        ("fr_b7_sub", 1.65, 1.88, 13.35, 13.95, 2.28, 2.50, "fur"),
        ("fr_b8", 2.25, 2.85, 14.95, 16.35, 2.22, 2.55, "fur"),
        ("fr_b8_tip", 2.35, 2.75, 14.25, 15.05, 2.25, 2.52, "fur_dk"),
        ("fr_b9", 3.05, 3.65, 14.75, 16.35, 2.22, 2.55, "fur_lt"),
        ("fr_b9_tip", 3.15, 3.55, 14.05, 14.85, 2.25, 2.52, "fur_dk"),
        ("fr_b9_sub", 3.22, 3.48, 13.45, 14.15, 2.28, 2.50, "fur_lt"),
    )
    for name, x0, x1, y0, y1, z0, z1, mat in fringes:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), mat)


def part_rib_plates(g: Gear) -> None:
    """缝嵌狼肋骨片：缝在披肩背部的弧形枯兽肋骨排与生铁紧固铆件。"""
    b = "rib_plates"
    # 三对对称缝嵌在背中线两侧的弧形兽肋骨
    rib_coords = (
        ("rib_l1", -3.65, -1.85, 21.45, 22.25, 2.45, 2.68, "bone"),
        ("rib_l1_tip", -1.95, -1.15, 21.65, 22.05, 2.48, 2.72, "bone_lt"),
        ("rib_l2", -3.45, -1.65, 19.85, 20.65, 2.48, 2.70, "bone"),
        ("rib_l2_tip", -1.75, -0.95, 20.05, 20.45, 2.50, 2.74, "bone_lt"),
        ("rib_l3", -3.15, -1.55, 18.25, 19.05, 2.50, 2.72, "bone"),
        ("rib_l3_tip", -1.65, -0.85, 18.45, 18.85, 2.52, 2.76, "bone_lt"),

        ("rib_r1", 1.85, 3.65, 21.45, 22.25, 2.45, 2.68, "bone"),
        ("rib_r1_tip", 1.15, 1.95, 21.65, 22.05, 2.48, 2.72, "bone_lt"),
        ("rib_r2", 1.65, 3.45, 19.85, 20.65, 2.48, 2.70, "bone"),
        ("rib_r2_tip", 0.95, 1.75, 20.05, 20.45, 2.50, 2.74, "bone_lt"),
        ("rib_r3", 1.55, 3.15, 18.25, 19.05, 2.50, 2.72, "bone"),
        ("rib_r3_tip", 0.85, 1.65, 18.45, 18.85, 2.52, 2.76, "bone_lt"),
    )
    for name, x0, x1, y0, y1, z0, z1, mat in rib_coords:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), mat)

    # 肋骨两端粗皮绳锁紧扎箍与铁垫片
    for k in range(3):
        y = 18.65 + k * 1.60
        g.box(b, f"rib_tie_l_{k}", (-3.55, y - 0.22, 2.42), (-3.25, y + 0.22, 2.75), "cord_dk")
        g.box(b, f"rib_tie_r_{k}", (3.25, y - 0.22, 2.42), (3.55, y + 0.22, 2.75), "cord_dk")
        g.box(b, f"rib_rivet_l_{k}", (-3.48, y - 0.12, 2.72), (-3.32, y + 0.12, 2.85), "iron_lt")
        g.box(b, f"rib_rivet_r_{k}", (3.32, y - 0.12, 2.72), (3.48, y + 0.12, 2.85), "iron_lt")
        g.box(b, f"rib_cord_x_l_{k}", (-2.65, y - 0.15, 2.55), (-2.15, y + 0.15, 2.78), "cord")
        g.box(b, f"rib_cord_x_r_{k}", (2.15, y - 0.15, 2.55), (2.65, y + 0.15, 2.78), "cord")


def part_cape_l(g: Gear) -> None:
    """左肩皮披垫：跨越左肩顶端（骑过左臂上沿）与左前胸下垂的兽皮。"""
    b = "cape_l"
    # 左肩顶（X∈[-6.85, -4.12]，Y∈[24.12, 25.45]，Z∈[-2.55, 2.45]；严格高于臂盒 y=24.0）
    g.box(b, "cape_l_top_main", (-6.75, 24.12, -2.45), (-4.12, 25.35, 2.35), "fur")
    g.box(b, "cape_l_top_rim_o", (-6.88, 24.12, -2.52), (-6.55, 25.45, 2.42), "fur_dk")
    g.box(b, "cape_l_top_rim_f", (-6.65, 24.12, -2.62), (-4.12, 25.42, -2.32), "fur_lt")
    g.box(b, "cape_l_top_rim_b", (-6.65, 24.12, 2.22), (-4.12, 25.42, 2.52), "fur_dk")
    g.box(b, "cape_l_ridge1", (-6.25, 24.45, -2.35), (-5.15, 25.55, 2.25), "fur_lt")
    g.box(b, "cape_l_ridge2", (-5.45, 24.35, -2.25), (-4.35, 25.45, 2.15), "fur_dk")

    # 左肩前胸垂皮（从肩顶向前下方垂落，X∈[-4.15, -1.85]，Y∈[19.2, 23.9]，Z∈[-2.56, -2.18]）
    g.box(b, "cape_l_front_p1", (-4.15, 21.65, -2.54), (-1.85, 23.90, -2.22), "fur")
    g.box(b, "cape_l_front_p2", (-3.95, 19.45, -2.52), (-1.95, 21.75, -2.24), "fur_dk")
    g.box(b, "cape_l_front_rim", (-2.15, 19.25, -2.58), (-1.78, 23.90, -2.18), "fur_lt")

    # 垂皮下摆撕裂毛牙
    g.box(b, "cape_l_front_fray1", (-3.85, 18.65, -2.50), (-3.25, 19.55, -2.25), "fur_lt")
    g.box(b, "cape_l_front_fray2", (-3.15, 18.45, -2.50), (-2.55, 19.55, -2.25), "fur")
    g.box(b, "cape_l_front_fray3", (-2.45, 18.85, -2.50), (-1.85, 19.55, -2.25), "fur_dk")
    g.box(b, "cape_l_front_sub1", (-3.65, 18.05, -2.48), (-3.35, 18.75, -2.26), "fur_dk")
    g.box(b, "cape_l_front_sub2", (-2.95, 17.85, -2.48), (-2.65, 18.55, -2.26), "fur_lt")

    # 肩部粗线补疤与加固厚皮
    g.box(b, "cape_l_patch", (-5.85, 24.25, -1.45), (-4.55, 25.45, 0.45), "hide_bald")
    for k in range(3):
        z = -1.05 + k * 0.55
        g.box(b, f"cape_l_stitch_{k}", (-5.95, 24.85, z), (-4.45, 25.15, z + 0.28), "stitch")


def part_cape_r(g: Gear) -> None:
    """右肩底皮托：右肩底垫、右胸垂布与固定带。"""
    b = "cape_r"
    # 右肩顶垫（X∈[4.12, 6.85]，Y∈[24.12, 25.25]，Z∈[-2.55, 2.45]）
    g.box(b, "cape_r_top_main", (4.12, 24.12, -2.45), (6.75, 25.25, 2.35), "fur_dk")
    g.box(b, "cape_r_top_rim_o", (6.55, 24.12, -2.52), (6.88, 25.35, 2.42), "fur_dk")
    g.box(b, "cape_r_top_fold", (4.35, 24.25, -2.25), (6.45, 25.35, 2.15), "fur")

    # 右前胸垂皮（X∈[1.85, 4.15]，Y∈[19.2, 23.9]，Z∈[-2.56, -2.18]）
    g.box(b, "cape_r_front_p1", (1.85, 21.65, -2.54), (4.15, 23.90, -2.22), "fur")
    g.box(b, "cape_r_front_p2", (1.95, 19.45, -2.52), (3.95, 21.75, -2.24), "fur_dk")
    g.box(b, "cape_r_front_rim", (1.78, 19.25, -2.58), (2.15, 23.90, -2.18), "fur_lt")

    # 垂皮下摆撕裂毛牙
    g.box(b, "cape_r_front_fray1", (1.85, 18.85, -2.50), (2.45, 19.55, -2.25), "fur_dk")
    g.box(b, "cape_r_front_fray2", (2.55, 18.45, -2.50), (3.15, 19.55, -2.25), "fur")
    g.box(b, "cape_r_front_fray3", (3.25, 18.65, -2.50), (3.85, 19.55, -2.25), "fur_lt")
    g.box(b, "cape_r_front_sub1", (2.15, 18.05, -2.48), (2.45, 18.95, -2.26), "fur_lt")
    g.box(b, "cape_r_front_sub2", (2.85, 17.85, -2.48), (3.15, 18.55, -2.26), "fur_dk")


def part_wolf_head(g: Gear) -> None:
    """右肩狼头颅骨：覆在右肩上的枯皮狼颅骨、吻部、獠牙与空眼眶（三轴均大，满足自证）。"""
    b = "wolf_head"
    # 严格位于 X ≥ 4.15，颅骨 y ≥ 24.12 避开右臂与头盒！吻部位于右肩前方 z ≤ -2.15！
    # 1. 颅骨脑壳（位于肩顶上方，X∈[4.25, 7.35], Y∈[24.12, 26.85], Z∈[-1.95, 2.45]）
    g.box(b, "wolf_cranium", (4.25, 24.12, -1.95), (7.35, 26.85, 2.45), "bone")
    g.box(b, "wolf_cranium_crest", (4.45, 26.65, -1.75), (7.15, 27.45, 2.25), "bone_lt")
    g.box(b, "wolf_temple_l", (4.18, 24.35, -1.25), (4.45, 26.35, 2.35), "bone_dk")
    g.box(b, "wolf_temple_r", (7.15, 24.35, -1.25), (7.45, 26.35, 2.35), "bone_dk")
    g.box(b, "wolf_cheek_plate_l", (4.15, 24.25, -1.85), (4.65, 25.85, -0.25), "bone_lt")
    g.box(b, "wolf_cheek_plate_r", (6.95, 24.25, -1.85), (7.45, 25.85, -0.25), "bone_lt")

    # 2. 狼吻部前伸（向前方下探，位于右肩前方 z ≤ -2.15，X∈[4.55, 7.05], Y∈[22.85, 25.55], Z∈[-4.25, -2.15]）
    g.box(b, "wolf_snout_bridge", (4.55, 23.45, -4.25), (7.05, 25.55, -2.15), "bone")
    g.box(b, "wolf_snout_top", (4.85, 25.35, -4.05), (6.75, 25.85, -2.25), "bone_lt")
    g.box(b, "wolf_snout_ridge", (5.15, 25.65, -3.85), (6.45, 26.05, -2.35), "bone_lt")
    g.box(b, "wolf_nose_tip", (5.15, 23.65, -4.45), (6.45, 24.85, -4.15), "bone_dk")
    g.box(b, "wolf_snout_side_l", (4.38, 23.65, -3.85), (4.72, 25.05, -2.18), "bone_dk")
    g.box(b, "wolf_snout_side_r", (6.88, 23.65, -3.85), (7.22, 25.05, -2.18), "bone_dk")

    # 3. 左右深陷空眼眶（黑洞 void 材质，位于吻部与脑壳交界处，Z∈[-2.35, -1.65]，Y∈[24.8, 26.2]）
    g.box(b, "wolf_orbit_r", (6.75, 24.75, -2.35), (7.38, 25.95, -1.65), "void")
    g.box(b, "wolf_orbit_l", (4.22, 24.75, -2.35), (4.85, 25.95, -1.65), "void")
    g.box(b, "wolf_brow_r", (6.65, 25.85, -2.45), (7.42, 26.35, -1.55), "bone_lt")
    g.box(b, "wolf_brow_l", (4.18, 25.85, -2.45), (4.95, 26.35, -1.55), "bone_lt")

    # 鼻孔前凹
    g.box(b, "wolf_nostril_void", (5.35, 23.75, -4.52), (6.25, 24.45, -4.35), "void")

    # 4. 上颌两枚巨大獠牙与排尖齿
    g.box(b, "fang_upper_r", (6.55, 22.15, -4.05), (6.98, 23.65, -3.55), "bone_lt")
    g.box(b, "fang_upper_l", (4.62, 22.15, -4.05), (5.05, 23.65, -3.55), "bone_lt")
    g.box(b, "fang_tip_r", (6.62, 21.65, -3.95), (6.92, 22.25, -3.65), "bone_lt")
    g.box(b, "fang_tip_l", (4.68, 21.65, -3.95), (4.98, 22.25, -3.65), "bone_lt")
    g.box(b, "teeth_incisor", (5.05, 23.15, -4.22), (6.55, 23.65, -3.85), "bone_lt")

    # 颅骨上的裂痕与生铁加固钉
    g.box(b, "wolf_skull_crack", (5.45, 25.85, 0.45), (5.85, 27.05, 1.85), "bone_dk")
    g.box(b, "wolf_skull_rivet", (5.15, 26.75, 0.85), (5.55, 27.15, 1.25), "iron_lt")
    g.box(b, "wolf_skull_suture1", (5.75, 26.45, -0.15), (6.15, 27.15, 0.85), "bone_dk")
    g.box(b, "wolf_skull_suture2", (4.85, 26.45, 1.15), (5.35, 27.05, 2.05), "bone_dk")
    g.box(b, "wolf_nasal_flange_l", (4.45, 24.85, -3.45), (4.85, 25.45, -2.45), "bone_lt")
    g.box(b, "wolf_nasal_flange_r", (6.75, 24.85, -3.45), (7.15, 25.45, -2.45), "bone_lt")
    g.box(b, "wolf_premolar_r", (6.85, 23.05, -3.25), (7.18, 23.65, -2.75), "bone_lt")
    g.box(b, "wolf_premolar_l", (4.42, 23.05, -3.25), (4.75, 23.65, -2.75), "bone_lt")


def part_wolf_jaw(g: Gear) -> None:
    """下颌骨：悬挂在吻部下方的张开狼下颌与下排尖齿（完全在右肩前 z ≤ -2.15）。"""
    b = "wolf_jaw"
    # 下颌主体（Y∈[21.6, 23.4]，Z∈[-4.1, -2.2]）
    g.box(b, "jaw_chin", (5.05, 21.65, -4.05), (6.55, 22.85, -3.45), "bone")
    g.box(b, "jaw_ramus_r", (6.35, 21.85, -3.55), (6.95, 23.35, -2.25), "bone_dk")
    g.box(b, "jaw_ramus_l", (4.65, 21.85, -3.55), (5.25, 23.35, -2.25), "bone_dk")
    g.box(b, "jaw_bottom_ridge", (5.25, 21.45, -3.95), (6.35, 21.75, -2.55), "bone_lt")

    # 下排尖齿与小獠牙
    g.box(b, "jaw_fang_r", (6.45, 22.65, -3.85), (6.85, 23.65, -3.45), "bone_lt")
    g.box(b, "jaw_fang_l", (4.75, 22.65, -3.85), (5.15, 23.65, -3.45), "bone_lt")
    g.box(b, "jaw_teeth_front", (5.15, 22.65, -4.00), (6.45, 23.15, -3.65), "bone_lt")
    g.box(b, "jaw_tubercle_l", (4.85, 21.85, -3.65), (5.25, 22.35, -3.05), "bone_lt")
    g.box(b, "jaw_tubercle_r", (6.35, 21.85, -3.65), (6.75, 22.35, -3.05), "bone_lt")
    g.box(b, "jaw_angle_keel", (5.35, 21.35, -3.25), (6.25, 21.75, -2.15), "bone_dk")

    # 下颌连接皮绳与骨栓
    g.box(b, "jaw_strap_r", (6.82, 22.65, -2.45), (7.08, 23.65, -2.15), "cord_dk")
    g.box(b, "jaw_strap_l", (4.52, 22.65, -2.45), (4.78, 23.65, -2.15), "cord_dk")


def part_wolf_crest(g: Gear) -> None:
    """狼耳与背脊残毛：立起的干瘪狼耳与头顶鬃毛（可晃动独立骨）。"""
    b = "wolf_crest"
    # 右耳（直立三角形，X∈[6.35, 7.35], Y∈[26.5, 28.5]）
    g.box(b, "ear_r_base", (6.35, 26.55, 0.45), (7.25, 27.65, 1.35), "fur_dk")
    g.box(b, "ear_r_tip", (6.55, 27.45, 0.65), (7.15, 28.45, 1.15), "fur")
    g.box(b, "ear_r_inner", (6.45, 26.65, 0.35), (7.05, 27.75, 0.65), "hide_bald")

    # 左耳（X∈[4.35, 5.25]）
    g.box(b, "ear_l_base", (4.35, 26.55, 0.45), (5.25, 27.65, 1.35), "fur_dk")
    g.box(b, "ear_l_tip", (4.45, 27.45, 0.65), (5.05, 28.45, 1.15), "fur")
    g.box(b, "ear_l_inner", (4.55, 26.65, 0.35), (5.15, 27.75, 0.65), "hide_bald")

    # 狼头顶至后脑的耸立鬃毛簇
    g.box(b, "crest_tuft1", (5.25, 26.85, 0.15), (6.35, 27.65, 0.95), "fur_lt")
    g.box(b, "crest_tuft2", (5.15, 26.65, 1.05), (6.45, 27.45, 1.95), "fur_dk")
    g.box(b, "crest_tuft3", (5.35, 25.85, 1.85), (6.25, 26.75, 2.55), "fur")


def part_paw_l(g: Gear) -> None:
    """左垂狼爪：垂在前胸左侧的枯狼前爪与黑角质勾爪（挂件独立骨）。"""
    b = "paw_l"
    g.box(b, "paw_l_wrist", (-3.15, 19.45, -2.58), (-2.05, 20.75, -2.28), "fur")
    g.box(b, "paw_l_cuff", (-3.25, 19.85, -2.62), (-1.95, 20.35, -2.24), "fur_lt")
    g.box(b, "paw_l_pad", (-3.05, 18.25, -2.55), (-2.15, 19.55, -2.30), "hide_bald")

    g.box(b, "toe_l1", (-3.12, 17.45, -2.52), (-2.72, 18.35, -2.32), "fur_dk")
    g.box(b, "claw_l1", (-3.05, 16.75, -2.60), (-2.78, 17.55, -2.28), "claw")

    g.box(b, "toe_l2", (-2.68, 17.25, -2.54), (-2.32, 18.25, -2.30), "fur_dk")
    g.box(b, "claw_l2", (-2.62, 16.55, -2.62), (-2.38, 17.35, -2.26), "claw")

    g.box(b, "toe_l3", (-2.28, 17.45, -2.52), (-1.88, 18.35, -2.32), "fur_dk")
    g.box(b, "claw_l3", (-2.22, 16.75, -2.60), (-1.95, 17.55, -2.28), "claw")

    g.box(b, "paw_l_cord", (-3.18, 19.35, -2.62), (-2.02, 19.68, -2.24), "cord_dk")
    g.box(b, "paw_l_cord_knot", (-2.75, 19.20, -2.68), (-2.45, 19.52, -2.20), "cord")
    g.box(b, "paw_l_knuckle1", (-3.02, 17.85, -2.62), (-2.82, 18.25, -2.25), "bone_lt")
    g.box(b, "paw_l_knuckle2", (-2.58, 17.75, -2.64), (-2.38, 18.15, -2.25), "bone_lt")
    g.box(b, "paw_l_knuckle3", (-2.18, 17.85, -2.62), (-1.98, 18.25, -2.25), "bone_lt")


def part_paw_r(g: Gear) -> None:
    """右垂狼爪：垂在前胸右侧的枯狼前爪与勾爪（挂件独立骨）。"""
    b = "paw_r"
    g.box(b, "paw_r_wrist", (2.05, 19.45, -2.58), (3.15, 20.75, -2.28), "fur")
    g.box(b, "paw_r_cuff", (1.95, 19.85, -2.62), (3.25, 20.35, -2.24), "fur_lt")
    g.box(b, "paw_r_pad", (2.15, 18.25, -2.55), (3.05, 19.55, -2.30), "hide_bald")

    g.box(b, "toe_r1", (1.88, 17.45, -2.52), (2.28, 18.35, -2.32), "fur_dk")
    g.box(b, "claw_r1", (1.95, 16.75, -2.60), (2.22, 17.55, -2.28), "claw")

    g.box(b, "toe_r2", (2.32, 17.25, -2.54), (2.68, 18.25, -2.30), "fur_dk")
    g.box(b, "claw_r2", (2.38, 16.55, -2.62), (2.62, 17.35, -2.26), "claw")

    g.box(b, "toe_r3", (2.72, 17.45, -2.52), (3.12, 18.35, -2.32), "fur_dk")
    g.box(b, "claw_r3", (2.78, 16.75, -2.60), (3.05, 17.55, -2.28), "claw")

    g.box(b, "paw_r_cord", (2.02, 19.35, -2.62), (3.18, 19.68, -2.24), "cord_dk")
    g.box(b, "paw_r_cord_knot", (2.45, 19.20, -2.68), (2.75, 19.52, -2.20), "cord")
    g.box(b, "paw_r_knuckle1", (1.98, 17.85, -2.62), (2.18, 18.25, -2.25), "bone_lt")
    g.box(b, "paw_r_knuckle2", (2.38, 17.75, -2.64), (2.58, 18.15, -2.25), "bone_lt")
    g.box(b, "paw_r_knuckle3", (2.82, 17.85, -2.62), (3.02, 18.25, -2.25), "bone_lt")


def part_chest_tie(g: Gear) -> None:
    """前胸束带骨栓：连接左右前爪的交叉皮绳与束紧兽骨卡扣。"""
    b = "chest_tie"
    g.box(b, "tie_strap_l", (-1.95, 21.65, -2.62), (-0.45, 22.25, -2.38), "cord")
    g.box(b, "tie_strap_r", (0.45, 21.65, -2.62), (1.95, 22.25, -2.38), "cord_dk")

    g.box(b, "tie_center_pad", (-0.75, 21.35, -2.68), (0.75, 22.55, -2.35), "hide_bald")

    g.box(b, "tie_bone_toggle", (-0.55, 21.65, -2.85), (0.55, 22.25, -2.48), "bone")
    g.box(b, "tie_bone_crest", (-0.22, 21.45, -2.92), (0.22, 22.45, -2.52), "bone_lt")
    g.box(b, "tie_bone_slit", (-0.45, 21.82, -2.88), (0.45, 22.08, -2.45), "bone_dk")

    g.box(b, "tie_dangle_a", (-0.35, 20.45, -2.72), (-0.05, 21.55, -2.48), "cord")
    g.box(b, "tie_dangle_b", (0.05, 20.25, -2.72), (0.35, 21.45, -2.48), "cord_dk")
    g.box(b, "tie_dangle_knot", (-0.25, 20.15, -2.76), (0.25, 20.55, -2.44), "cord_dk")
    g.box(b, "tie_bone_bead", (-0.18, 19.82, -2.75), (0.18, 20.22, -2.45), "bone_lt")


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


class ShouPiPiJianSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "cape_back": part_cape_back,
    "back_scars": part_back_scars,
    "fur_fringe_b": part_fur_fringe_b,
    "rib_plates": part_rib_plates,
    "cape_l": part_cape_l,
    "cape_r": part_cape_r,
    "wolf_head": part_wolf_head,
    "wolf_jaw": part_wolf_jaw,
    "wolf_crest": part_wolf_crest,
    "paw_l": part_paw_l,
    "paw_r": part_paw_r,
    "chest_tie": part_chest_tie,
}

SPEC = ShouPiPiJianSpec(
    gear_id="shou_pi_pi_jian",
    model_name="ShouPiPiJian",
    title="兽皮披肩 / shou_pi_pi_jian",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：双肩/上背 · 枯皮狼头搭右肩+前胸垂双爪与骨扣+背部嵌肋骨撕裂披肩",
    worn_focus=((0.0, 21.5, 0.0), 22.0),
    tex_views=(
        ("BACK 背面（+z）", 0.0, 0.0),
        ("3/4 背右斜", 28.0, 18.0),
        ("FRONT 正面（-z）", 180.0, 0.0),
    ),
    texture_overrides={
        "fur": "bear_fur",
        "fur_dk": "bear_fur_dark",
        "fur_lt": "gear_fur_gr",
        "hide_bald": "gear_hide_bald",
        "claw": "gear_claw",
    },
    textured=True,
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
