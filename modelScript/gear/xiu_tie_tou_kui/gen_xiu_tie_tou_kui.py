#!/usr/bin/env python3
"""锈铁头盔（xiu_tie_tou_kui）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土定制 NPC 专属装备 #8（sui_kuang_gong 碎矿工）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

挂点与避让说明：
- 挂在头顶与头周，严禁穿入头盒（x∈[-4,4], y∈[24,32], z∈[-4,4]）。
- 壳层整体落在头盒之外：顶部 y ≥ 32.08，前眉 z ≤ -4.10，后颈 z ≥ 4.10，左右耳侧 |x| ≥ 4.10。
- 盔沿略高于眉（y ≥ 28.80），眼部不遮挡；下颌系带在 cheek 处 y ≥ 24.12，向下前伸到 z ≤ -2.15 避开手臂盒。
- 额前特别设计：高对比度碎琉璃矿灯反光镜座（带铜座、亮白高光晶块、折射多面与铁丝防护栅）。
- 盔顶左侧带有磕碰劈裂的大凹口与补焊铁疤。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/gear/xiu_tie_tou_kui/gen_xiu_tie_tou_kui.py --self-test
    python3 modelScript/gear/xiu_tie_tou_kui/gen_xiu_tie_tou_kui.py --gates
    python3 modelScript/gear/xiu_tie_tou_kui/gen_xiu_tie_tou_kui.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "iron":         (82, 78, 76),      # 粗锻生铁盔壳底色
    "iron_lt":      (120, 116, 118),   # 铁铆钉与磨损亮边
    "iron_dk":      (56, 52, 54),      # 接缝暗铁与深凹
    "rust":         (152, 86, 52),     # 粗糙红铁锈斑
    "rust_dk":      (106, 58, 36),     # 浓重铁锈蚀坑
    "glass":        (168, 208, 196),   # 浑浊绿琉璃反射底
    "glass_hi":     (242, 252, 248),   # 矿灯极高对比高光亮白
    "glass_dk":     (88, 122, 114),    # 矿灯暗边与折射深色
    "bronze":       (142, 108, 64),    # 矿灯粗铜座与箍带
    "cloth":        (152, 138, 110),   # 外翻衬布
    "cloth_dk":     (110, 96, 76),     # 衬布阴影折痕
    "strap":        (116, 76, 52),     # 下颌皮带
    "strap_dk":     (82, 54, 38),      # 磨损皮带与边缘
    "cord":         (146, 126, 82),    # 麻绳扎线
    "cord_dk":      (104, 88, 56),     # 麻绳深色结
    "bone":         (216, 206, 178),   # 矿工工号骨签
    "bone_lt":      (238, 230, 208),   # 骨签高光
    "bone_dk":      (160, 146, 116),   # 骨刻字刻槽
    "stitch":       (58, 44, 32),      # 粗麻缝线
    "void":         (22, 18, 16),      # 磕碰裂孔与眼窝暗底
}

SOFT = ("cloth", "cloth_dk", "strap", "strap_dk", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("helmet_crown", "root", (0.0, 32.8, 0.0), "盔顶半球穹顶：顶部铁板、中脊梁与环向拼接铁条（对称）"),
    ("crown_dent", "helmet_crown", (-2.2, 33.2, -1.8), "盔顶劈裂凹坑：左前被天雷碎石磕出的凹口与补焊铁疤"),
    ("brow_plate", "helmet_crown", (0.0, 30.5, -4.3), "额前眉护铁板：前额加厚防砸铁板与凸出遮檐"),
    ("lamp_mount", "brow_plate", (0.0, 31.0, -4.6), "矿灯青铜基座：额前中心铆接的方口灯座与加固支架"),
    ("lamp_crystal", "lamp_mount", (0.0, 31.0, -4.9), "矿灯反射晶片：极高对比度抛光碎琉璃透镜（核心高光挂件）"),
    ("rim_flange", "helmet_crown", (0.0, 29.8, 0.0), "盔下缘加强卷边：环绕盔圈一周的铆接加厚铁条"),
    ("liner_roll", "rim_flange", (0.0, 29.2, 0.0), "外翻减震粗布衬：从盔下缘翻出的一圈厚麻布软垫"),
    ("side_guards", "rim_flange", (0.0, 27.5, 0.0), "左右耳部护铁片：护住太阳穴与耳上的两侧下延铁板"),
    ("chin_strap_r", "side_guards", (4.25, 24.5, -2.4), "右侧下颌皮扣带：向前斜伸至下颏的扣环皮带（可动带）"),
    ("chin_strap_l", "side_guards", (-4.25, 24.5, -2.4), "左侧磨断残皮带：左侧垂落的粗糙磨断破皮带端（可动带）"),
    ("back_neck_guard", "rim_flange", (0.0, 28.5, 4.3), "后脑护颈铁叶：后颈垂下的叠压防碎石生铁片（可微动）"),
    ("hang_tally", "side_guards", (-4.35, 26.5, -1.0), "工号兽骨签：系在左颊铆钉上的打孔矿工身份骨牌（挂件独立骨）"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "helmet_crown"))


# ───────────────────────────────────────────────────────────── 分部件实现
def part_helmet_crown(g: Gear) -> None:
    """盔顶半球穹顶：顶部铁板、中脊梁与环向拼接铁条（严格对称，用于镜像门禁自证）。"""
    b = "helmet_crown"
    # 顶部整体位于 Y ≥ 32.08，彻底在头盒 (y ≤ 32.0) 之上！
    # 顶层主板
    g.box(b, "crown_top_main", (-3.85, 32.88, -3.85), (3.85, 33.48, 3.85), "iron")
    g.box(b, "crown_top_crest", (-3.25, 33.45, -3.25), (3.25, 33.78, 3.25), "iron_lt")
    g.box(b, "crown_top_finial", (-0.85, 33.75, -0.85), (0.85, 34.18, 0.85), "iron")

    # 前后纵贯加厚中脊梁（防砸大加强筋，X∈[-0.55, 0.55]）
    g.box(b, "crown_spine_ridge", (-0.55, 32.95, -4.25), (0.55, 33.85, 4.25), "iron_lt")
    g.box(b, "crown_spine_rivet_f", (-0.18, 33.80, -2.85), (0.18, 34.05, -2.45), "iron_lt")
    g.box(b, "crown_spine_rivet_b", (-0.18, 33.80, 2.45), (0.18, 34.05, 2.85), "iron_lt")
    g.box(b, "crown_spine_mid_stud", (-0.22, 33.95, -0.22), (0.22, 34.25, 0.22), "iron_lt")

    # 左右外侧穹顶斜过渡阶梯（成对对称，错开 Y 避免共面）
    g.pair(b, "crown_step_side", (3.75, 32.38, -3.95), (4.35, 33.08, 3.95), "iron")
    g.pair(b, "crown_step_side_rim", (4.25, 32.18, -4.15), (4.55, 32.88, 4.15), "iron_dk")
    g.pair(b, "crown_step_flange", (4.38, 32.28, -3.75), (4.65, 32.78, 3.75), "iron_lt")

    # 前后穹顶斜过渡阶梯（错开 Y 避免共面）
    g.box(b, "crown_step_front", (-3.95, 32.32, -4.35), (3.95, 33.02, -3.75), "iron")
    g.box(b, "crown_step_back", (-3.95, 32.32, 3.75), (3.95, 33.02, 4.35), "iron")
    g.box(b, "crown_step_front_ridge", (-3.45, 32.42, -4.48), (3.45, 32.82, -4.22), "iron_lt")
    g.box(b, "crown_step_back_ridge", (-3.45, 32.42, 4.22), (3.45, 32.82, 4.48), "iron_lt")

    # 四切角加强三角铁片（成对对称，错开 X/Z 避免共面）
    g.pair(b, "crown_corner_f", (3.25, 32.55, -4.25), (4.25, 33.15, -3.25), "rust")
    g.pair(b, "crown_corner_b", (3.25, 32.55, 3.25), (4.25, 33.15, 4.25), "rust")
    g.pair(b, "crown_corner_rib_f", (3.35, 32.65, -4.42), (4.42, 33.05, -3.35), "iron_lt")
    g.pair(b, "crown_corner_rib_b", (3.35, 32.65, 3.35), (4.42, 33.05, 4.42), "iron_lt")
    g.pair(b, "crown_corner_stud_f", (3.65, 32.85, -4.38), (4.15, 33.25, -3.88), "iron")
    g.pair(b, "crown_corner_stud_b", (3.65, 32.85, 3.88), (4.15, 33.25, 4.38), "iron")

    # 对称生铁铆钉对
    for k in range(3):
        z = -2.25 + k * 2.25
        g.pair(b, f"crown_rivet_side_{k}", (3.95, 33.05, z - 0.22), (4.25, 33.28, z + 0.22), "iron_lt")


def part_crown_dent(g: Gear) -> None:
    """盔顶劈裂凹坑：左前被天雷碎石磕出的凹口与补焊铁疤（不对称）。"""
    b = "crown_dent"
    # 凹坑中心在 X=-2.2, Z=-1.8, Y∈[32.5, 33.6]
    g.box(b, "dent_pit_void", (-2.65, 32.75, -2.25), (-1.75, 33.35, -1.35), "void")
    g.box(b, "dent_pit_bed", (-2.85, 32.55, -2.45), (-1.55, 32.85, -1.15), "rust_dk")

    # 磕裂外翻的扭曲铁边（Y 厚度 ≥ 0.58，确保穿模门禁有鉴别力）
    g.box(b, "dent_gash_lip1", (-3.15, 33.10, -2.15), (-2.55, 33.68, -1.45), "iron_lt")
    g.box(b, "dent_gash_lip2", (-1.85, 33.10, -2.65), (-1.25, 33.68, -1.95), "iron_lt")
    g.box(b, "dent_gash_back", (-2.45, 33.15, -1.25), (-1.75, 33.55, -0.75), "rust")
    g.box(b, "dent_flake1", (-2.95, 33.45, -1.85), (-2.65, 33.75, -1.45), "iron_lt")
    g.box(b, "dent_flake2", (-1.65, 33.45, -2.25), (-1.35, 33.75, -1.85), "iron_lt")

    # 抢修粗焊渣焊肉条
    g.box(b, "dent_weld_lump1", (-2.95, 33.25, -2.55), (-2.25, 33.65, -2.15), "rust")
    g.box(b, "dent_weld_lump2", (-2.15, 33.25, -1.45), (-1.45, 33.65, -0.95), "rust_dk")
    g.box(b, "dent_weld_seam", (-2.75, 33.35, -2.35), (-1.65, 33.55, -1.25), "iron")
    g.box(b, "dent_weld_blob", (-2.35, 33.48, -1.95), (-1.95, 33.75, -1.55), "iron_dk")

    # 加固打入的粗铁铆钉
    g.box(b, "dent_patch_rivet1", (-3.35, 33.25, -1.85), (-2.95, 33.65, -1.45), "iron_lt")
    g.box(b, "dent_patch_rivet2", (-1.65, 33.25, -2.75), (-1.25, 33.65, -2.35), "iron_lt")
    g.box(b, "dent_patch_staple", (-2.85, 33.35, -2.65), (-2.05, 33.58, -2.35), "iron")


def part_brow_plate(g: Gear) -> None:
    """额前眉护铁板：前额加厚防砸铁板与凸出遮檐（Z ≤ -4.10，彻底在头盒前）。"""
    b = "brow_plate"
    # 前眉主铁板（X∈[-4.35, 4.35], Y∈[29.85, 32.25], Z∈[-4.65, -4.10]）
    g.box(b, "brow_plate_main", (-4.35, 29.85, -4.45), (4.35, 32.25, -4.10), "iron")
    g.box(b, "brow_plate_upper", (-4.15, 31.65, -4.58), (4.15, 32.35, -4.22), "iron_lt")
    g.box(b, "brow_plate_bevel_l", (-4.45, 30.55, -4.55), (-3.65, 31.85, -4.18), "iron_dk")
    g.box(b, "brow_plate_bevel_r", (3.65, 30.55, -4.55), (4.45, 31.85, -4.18), "iron_dk")

    # 凸出向前的前倾遮檐（挡飞石檐部，Z 最远伸至 -4.85）
    g.box(b, "brow_visor_lip", (-4.25, 29.75, -4.85), (4.25, 30.25, -4.38), "iron_lt")
    g.box(b, "brow_visor_slope", (-4.05, 30.20, -4.75), (4.05, 30.65, -4.42), "iron")
    g.box(b, "brow_visor_rib_l", (-3.65, 29.85, -4.92), (-2.85, 30.35, -4.52), "iron_lt")
    g.box(b, "brow_visor_rib_r", (2.85, 29.85, -4.92), (3.65, 30.35, -4.52), "iron_lt")

    # 左右眉角向下斜向护板
    g.box(b, "brow_wing_l", (-4.45, 29.45, -4.52), (-3.75, 30.45, -4.12), "rust")
    g.box(b, "brow_wing_r", (3.75, 29.45, -4.52), (4.45, 30.45, -4.12), "rust_dk")
    g.box(b, "brow_visor_tab_l", (-4.32, 29.85, -4.92), (-3.75, 30.25, -4.38), "iron_dk")
    g.box(b, "brow_visor_tab_r", (3.75, 29.85, -4.92), (4.32, 30.25, -4.38), "iron_dk")
    g.box(b, "brow_ridge_rib1", (-2.45, 30.75, -4.72), (-1.65, 31.15, -4.42), "iron_lt")
    g.box(b, "brow_ridge_rib2", (1.65, 30.75, -4.72), (2.45, 31.15, -4.42), "iron_lt")
    g.box(b, "brow_rust_blot1", (-3.65, 30.15, -4.68), (-3.05, 30.55, -4.38), "rust")
    g.box(b, "brow_rust_blot2", (3.05, 30.15, -4.68), (3.65, 30.55, -4.38), "rust_dk")

    # 前额横向铆接加固铁带与五枚铆钉
    g.box(b, "brow_strap_bar", (-3.85, 30.65, -4.62), (3.85, 31.25, -4.42), "iron_dk")
    for k in range(5):
        x = -3.2 + k * 1.6
        g.box(b, f"brow_strap_rivet_{k}", (x - 0.18, 30.75, -4.75), (x + 0.18, 31.15, -4.45), "iron_lt")


def part_lamp_mount(g: Gear) -> None:
    """矿灯青铜基座：额前中心铆接的方口灯座与加固支架。"""
    b = "lamp_mount"
    # 中心位于 X=0.0, Y=30.85, Z∈[-4.95, -4.45]
    # 青铜铸造底盘
    g.box(b, "lamp_base_plate", (-1.35, 29.95, -4.75), (1.35, 32.15, -4.45), "bronze")
    g.box(b, "lamp_base_rim_t", (-1.45, 32.05, -4.82), (1.45, 32.35, -4.42), "bronze")
    g.box(b, "lamp_base_rim_b", (-1.45, 29.75, -4.82), (1.45, 30.05, -4.42), "bronze")

    # 左右侧爪支架
    g.box(b, "lamp_bracket_l", (-1.45, 30.25, -5.05), (-1.05, 31.85, -4.55), "bronze")
    g.box(b, "lamp_bracket_r", (1.05, 30.25, -5.05), (1.45, 31.85, -4.55), "bronze")
    g.box(b, "lamp_bracket_top", (-0.95, 31.75, -5.12), (0.95, 32.15, -4.65), "bronze")
    g.box(b, "lamp_bracket_bot", (-0.95, 29.95, -5.12), (0.95, 30.35, -4.65), "bronze")
    g.box(b, "lamp_bracket_top_rivet", (-0.25, 31.85, -5.22), (0.25, 32.15, -4.85), "iron_lt")
    g.box(b, "lamp_bracket_bot_rivet", (-0.25, 29.95, -5.22), (0.25, 30.25, -4.85), "iron_lt")
    g.box(b, "lamp_side_reinforce_l", (-1.52, 30.85, -4.92), (-1.22, 31.45, -4.55), "bronze")
    g.box(b, "lamp_side_reinforce_r", (1.22, 30.85, -4.92), (1.52, 31.45, -4.55), "bronze")
    g.box(b, "lamp_reflector_backing", (-0.75, 30.45, -4.95), (0.75, 31.65, -4.72), "iron_lt")
    g.box(b, "lamp_mount_screw", (-0.15, 31.75, -5.32), (0.15, 32.05, -5.02), "bronze")

    # 矿灯防护铁丝格栅（防止飞石砸碎反光镜）
    g.box(b, "lamp_grill_h1", (-0.85, 31.15, -5.28), (0.85, 31.35, -4.95), "iron_lt")
    g.box(b, "lamp_grill_v1", (-0.12, 30.25, -5.28), (0.12, 31.85, -4.95), "iron_lt")
    g.box(b, "lamp_oil_cup_tube", (-0.35, 29.65, -4.85), (0.35, 30.15, -4.45), "bronze")

    # 四角铆钉
    g.box(b, "lamp_rivet_tl", (-1.25, 31.85, -4.95), (-0.95, 32.15, -4.65), "iron_lt")
    g.box(b, "lamp_rivet_tr", (0.95, 31.85, -4.95), (1.25, 32.15, -4.65), "iron_lt")
    g.box(b, "lamp_rivet_bl", (-1.25, 30.05, -4.95), (-0.95, 30.35, -4.65), "iron_lt")
    g.box(b, "lamp_rivet_br", (0.95, 30.05, -4.95), (1.25, 30.35, -4.65), "iron_lt")

    g.box(b, "lamp_cavity_void", (-0.85, 30.35, -4.88), (0.85, 31.75, -4.55), "void")


def part_lamp_crystal(g: Gear) -> None:
    """矿灯反射晶片：极高对比度抛光碎琉璃透镜（核心高光挂件）。"""
    b = "lamp_crystal"
    # 琉璃核心反光晶块
    g.box(b, "crystal_main_body", (-0.85, 30.35, -5.05), (0.85, 31.75, -4.68), "glass")

    # 矿灯中心耀眼白高光块（高对比度）
    g.box(b, "crystal_glint_core", (-0.45, 30.75, -5.25), (0.45, 31.45, -4.95), "glass_hi")
    g.box(b, "crystal_glint_top", (-0.35, 31.35, -5.18), (0.35, 31.65, -4.88), "glass_hi")
    g.box(b, "crystal_glint_facet", (0.15, 30.55, -5.18), (0.55, 30.95, -4.88), "glass_hi")
    g.box(b, "crystal_glint_dot", (-0.35, 30.55, -5.18), (-0.12, 30.82, -4.88), "glass_hi")

    # 晶片折射暗边与破损裂隙
    g.box(b, "crystal_crack_dark", (-0.75, 30.85, -5.12), (0.15, 31.15, -4.82), "glass_dk")
    g.box(b, "crystal_edge_shade", (-0.88, 30.25, -5.00), (-0.55, 31.85, -4.72), "glass_dk")
    g.box(b, "crystal_bevel_r", (0.55, 30.25, -5.00), (0.88, 31.85, -4.72), "glass")
    g.box(b, "crystal_bottom_recess", (-0.65, 30.25, -5.10), (0.65, 30.55, -4.78), "glass_dk")
    g.box(b, "crystal_facet_l", (-0.65, 31.15, -5.15), (-0.35, 31.55, -4.85), "glass_hi")
    g.box(b, "crystal_facet_b", (-0.35, 30.45, -5.15), (0.35, 30.75, -4.85), "glass")
    g.box(b, "crystal_gash1", (-0.15, 30.85, -5.22), (0.35, 31.25, -4.92), "glass_dk")
    g.box(b, "crystal_chip", (0.45, 31.15, -5.12), (0.75, 31.45, -4.82), "glass_hi")


def part_rim_flange(g: Gear) -> None:
    """盔下缘加强卷边：环绕盔圈一周的铆接加厚铁条（Y∈[29.35, 30.15]）。"""
    b = "rim_flange"
    # 左右侧下缘条（|x| ≥ 4.10）
    g.box(b, "rim_bar_l", (-4.45, 29.35, -4.25), (-4.10, 30.05, 4.25), "iron")
    g.box(b, "rim_bar_r", (4.10, 29.35, -4.25), (4.45, 30.05, 4.25), "iron")
    g.box(b, "rim_rib_l", (-4.55, 29.45, -4.15), (-4.25, 29.85, 4.15), "iron_lt")
    g.box(b, "rim_rib_r", (4.25, 29.45, -4.15), (4.55, 29.85, 4.15), "iron_lt")

    # 后颈下缘条（z ≥ 4.10）
    g.box(b, "rim_bar_back", (-4.25, 29.35, 4.10), (4.25, 30.05, 4.45), "iron")
    g.box(b, "rim_rib_back", (-4.15, 29.45, 4.25), (4.15, 29.85, 4.55), "iron_lt")

    # 左右侧下缘铆钉
    for k in range(5):
        z = -2.85 + k * 1.45
        g.box(b, f"rim_rivet_l_{k}", (-4.65, 29.55, z - 0.18), (-4.35, 29.85, z + 0.18), "iron_lt")
        g.box(b, f"rim_rivet_r_{k}", (4.35, 29.55, z - 0.18), (4.65, 29.85, z + 0.18), "iron_lt")


def part_liner_roll(g: Gear) -> None:
    """外翻减震粗布衬：从盔下缘翻出的一圈厚麻布软垫。"""
    b = "liner_roll"
    # 衬布翻出在铁圈下沿（Y∈[28.85, 29.45]）
    g.box(b, "liner_roll_l", (-4.48, 28.85, -4.20), (-4.10, 29.45, 4.20), "cloth")
    g.box(b, "liner_roll_r", (4.10, 28.85, -4.20), (4.48, 29.45, 4.20), "cloth")
    g.box(b, "liner_crease_l", (-4.55, 28.95, -3.95), (-4.22, 29.25, 3.95), "cloth_dk")
    g.box(b, "liner_crease_r", (4.22, 28.95, -3.95), (4.55, 29.25, 3.95), "cloth_dk")

    g.box(b, "liner_roll_b", (-4.20, 28.85, 4.10), (4.20, 29.45, 4.48), "cloth")
    g.box(b, "liner_crease_b", (-3.95, 28.95, 4.22), (3.95, 29.25, 4.55), "cloth_dk")
    g.box(b, "liner_front_lip", (-4.10, 29.45, -4.48), (4.10, 29.85, -4.10), "cloth_dk")

    for k in range(5):
        z = -2.65 + k * 1.35
        g.box(b, f"liner_stitch_l_{k}", (-4.58, 29.15, z), (-4.18, 29.38, z + 0.28), "stitch")
        g.box(b, f"liner_stitch_r_{k}", (4.18, 29.15, z), (4.58, 29.38, z + 0.28), "stitch")


def part_side_guards(g: Gear) -> None:
    """左右耳部护铁片：护住太阳穴与耳上的两侧下延铁板（|x| ≥ 4.10）。"""
    b = "side_guards"
    # 左耳护板（X∈[-4.45, -4.10], Y∈[26.25, 29.45], Z∈[-2.05, 1.25]）
    g.box(b, "ear_plate_l_main", (-4.45, 26.25, -2.05), (-4.10, 29.45, 1.25), "iron")
    g.box(b, "ear_plate_l_flange", (-4.55, 26.15, -1.95), (-4.22, 29.55, 1.15), "iron_lt")
    g.box(b, "ear_plate_l_low", (-4.42, 25.45, -1.65), (-4.12, 26.35, 0.85), "iron")
    g.box(b, "ear_slit_l_void", (-4.62, 27.25, -0.65), (-4.25, 28.45, -0.15), "void")
    g.box(b, "ear_hinge_l", (-4.52, 28.95, -0.65), (-4.18, 29.35, -0.15), "rust")

    # 右耳护板
    g.box(b, "ear_plate_r_main", (4.10, 26.25, -2.05), (4.45, 29.45, 1.25), "iron")
    g.box(b, "ear_plate_r_flange", (4.22, 26.15, -1.95), (4.55, 29.55, 1.15), "iron_lt")
    g.box(b, "ear_plate_r_low", (4.12, 25.45, -1.65), (4.42, 26.35, 0.85), "iron")
    g.box(b, "ear_slit_r_void", (4.25, 27.25, -0.65), (4.62, 28.45, -0.15), "void")
    g.box(b, "ear_hinge_r", (4.18, 28.95, -0.65), (4.52, 29.35, -0.15), "rust")

    # 左右护板防砍凸筋与边缘折角
    g.box(b, "ear_rib_l", (-4.52, 26.45, -1.85), (-4.28, 29.25, -1.55), "iron_dk")
    g.box(b, "ear_rib_r", (4.28, 26.45, -1.85), (4.52, 29.25, -1.55), "iron_dk")
    g.box(b, "ear_stud_l", (-4.62, 26.65, -0.45), (-4.32, 26.95, -0.15), "iron_lt")
    g.box(b, "ear_stud_r", (4.32, 26.65, -0.45), (4.62, 26.95, -0.15), "iron_lt")


def part_chin_strap_r(g: Gear) -> None:
    """右侧下颌皮扣带：从颊侧斜向前下方延伸至下颏（避开手臂盒）。"""
    b = "chin_strap_r"
    # 上部在臂盒之上 y ≥ 24.12
    g.box(b, "strap_r_upper", (4.12, 24.12, -1.85), (4.42, 25.85, -1.15), "strap")
    g.box(b, "strap_r_rim_u", (4.15, 24.12, -1.95), (4.48, 25.85, -1.65), "strap_dk")

    # 下部向前延伸至下颏前方 z ≤ -2.15，彻底避开右臂盒 z∈[-2,2]！
    g.box(b, "strap_r_slope", (4.12, 23.25, -3.15), (4.42, 24.25, -2.15), "strap")
    g.box(b, "strap_r_slope_rim", (4.15, 23.25, -3.25), (4.48, 24.25, -2.25), "strap_dk")

    # 生铁日字带扣（在下颏前方 Z∈[-3.15, -2.35]）
    g.box(b, "buckle_r_frame_t", (4.15, 23.85, -3.15), (4.55, 24.15, -2.35), "iron")
    g.box(b, "buckle_r_frame_b", (4.15, 22.95, -3.15), (4.55, 23.25, -2.35), "iron")
    g.box(b, "buckle_r_frame_f", (4.15, 23.15, -3.25), (4.55, 23.95, -2.95), "iron_lt")
    g.box(b, "buckle_r_frame_r", (4.15, 23.15, -2.55), (4.55, 23.95, -2.25), "iron")
    g.box(b, "buckle_r_pin", (4.25, 23.05, -2.85), (4.58, 24.05, -2.75), "iron_lt")

    # 垂落打孔皮带尾
    g.box(b, "strap_r_tail1", (4.12, 22.15, -3.05), (4.42, 23.05, -2.45), "strap")
    g.box(b, "strap_r_tail2", (4.12, 21.25, -2.95), (4.40, 22.25, -2.48), "strap_dk")
    g.box(b, "strap_r_hole1", (4.22, 22.45, -2.82), (4.50, 22.75, -2.66), "void")


def part_chin_strap_l(g: Gear) -> None:
    """左侧磨断残皮带：左侧垂落的粗糙磨断破皮带端。"""
    b = "chin_strap_l"
    # 上部在臂盒之上 y ≥ 24.12
    g.box(b, "strap_l_upper", (-4.42, 24.12, -1.85), (-4.12, 25.85, -1.15), "strap")
    g.box(b, "strap_l_rim_u", (-4.48, 24.12, -1.95), (-4.15, 25.85, -1.65), "strap_dk")

    # 下部向前延伸至 z ≤ -2.15，彻底避开左臂盒 z∈[-2,2]！
    g.box(b, "strap_l_slope", (-4.42, 23.25, -3.15), (-4.12, 24.25, -2.15), "strap_dk")
    g.box(b, "strap_l_fray1", (-4.45, 22.65, -3.12), (-4.15, 23.35, -2.35), "strap")
    g.box(b, "strap_l_fray2", (-4.42, 22.05, -3.05), (-4.18, 22.75, -2.45), "strap_dk")
    g.box(b, "strap_l_rip_knot", (-4.48, 23.45, -3.05), (-4.12, 23.85, -2.35), "cord")


def part_back_neck_guard(g: Gear) -> None:
    """后脑护颈铁叶：后颈垂下的叠压防碎石生铁片（Z ≥ 4.10）。"""
    b = "back_neck_guard"
    # 第一层铁叶（紧接下缘圈，宽 7.2px）
    g.box(b, "neck_plate_1", (-3.60, 28.05, 4.12), (3.60, 29.45, 4.42), "iron")
    g.box(b, "neck_plate_1_rim", (-3.75, 27.95, 4.15), (3.75, 28.35, 4.48), "iron_lt")
    g.box(b, "neck_rib1_l", (-2.65, 28.15, 4.38), (-1.85, 29.35, 4.58), "iron_dk")
    g.box(b, "neck_rib1_r", (1.85, 28.15, 4.38), (2.65, 29.35, 4.58), "iron_dk")

    # 第二层叠压窄铁叶（Y∈[26.85, 28.25]）
    g.box(b, "neck_plate_2", (-3.15, 26.85, 4.15), (3.15, 28.25, 4.45), "iron")
    g.box(b, "neck_plate_2_rim", (-3.28, 26.75, 4.18), (3.28, 27.15, 4.52), "iron_dk")
    g.box(b, "neck_rib2_l", (-2.25, 26.95, 4.42), (-1.55, 28.15, 4.62), "iron_lt")
    g.box(b, "neck_rib2_r", (1.55, 26.95, 4.42), (2.25, 28.15, 4.62), "iron_lt")

    # 第三层中心尾盾小铁片
    g.box(b, "neck_plate_3", (-1.85, 25.85, 4.18), (1.85, 27.05, 4.48), "rust")
    g.box(b, "neck_plate_3_tip", (-1.15, 25.25, 4.22), (1.15, 25.95, 4.45), "rust_dk")
    g.box(b, "neck_plate_3_rib", (-0.45, 25.35, 4.45), (0.45, 26.95, 4.65), "iron_lt")

    # 铰接铆钉与锁链挂环
    for k in range(3):
        x = -2.2 + k * 2.2
        g.box(b, f"neck_rivet_row1_{k}", (x - 0.18, 28.85, 4.38), (x + 0.18, 29.15, 4.62), "iron_lt")
        g.box(b, f"neck_rivet_row2_{k}", (x - 0.18, 27.65, 4.42), (x + 0.18, 27.95, 4.65), "iron_lt")
    g.box(b, "neck_plate_2_stud_l", (-2.85, 27.25, 4.45), (-2.55, 27.55, 4.68), "iron_lt")
    g.box(b, "neck_plate_2_stud_r", (2.55, 27.25, 4.45), (2.85, 27.55, 4.68), "iron_lt")
    g.box(b, "neck_chain_link_l", (-3.35, 28.55, 4.38), (-3.05, 29.15, 4.65), "iron")
    g.box(b, "neck_chain_link_r", (3.05, 28.55, 4.38), (3.35, 29.15, 4.65), "iron")


def part_hang_tally(g: Gear) -> None:
    """工号兽骨签：系在左颊铆钉上的打孔矿工身份骨牌（Y ≥ 24.12，严格在左臂盒之上）。"""
    b = "hang_tally"
    # 悬挂于左颊外侧，Y 保持 ≥ 24.12，彻底避开左臂盒（y∈[12,24], x∈[-8,-4]）！
    g.box(b, "tally_cord_upper", (-4.45, 26.65, -1.05), (-4.22, 27.55, -0.75), "cord")
    g.box(b, "tally_cord_knot", (-4.52, 26.35, -1.15), (-4.18, 26.75, -0.65), "cord_dk")
    g.box(b, "tally_cord_lower", (-4.45, 25.85, -1.05), (-4.22, 26.45, -0.75), "cord")

    # 兽骨牌身（Y∈[24.15, 25.95]）
    g.box(b, "tally_bone_body", (-4.55, 24.15, -1.25), (-4.20, 25.85, -0.55), "bone")
    g.box(b, "tally_bone_rim_t", (-4.62, 25.65, -1.30), (-4.15, 25.98, -0.50), "bone_lt")
    g.box(b, "tally_bone_rim_b", (-4.62, 24.12, -1.30), (-4.15, 24.42, -0.50), "bone_lt")

    # 骨签正面的刻槽工痕与穿孔
    g.box(b, "tally_notch1", (-4.62, 25.15, -1.15), (-4.35, 25.38, -0.65), "bone_dk")
    g.box(b, "tally_notch2", (-4.62, 24.75, -1.15), (-4.35, 24.98, -0.65), "bone_dk")
    g.box(b, "tally_notch3", (-4.62, 24.35, -1.15), (-4.35, 24.58, -0.65), "bone_dk")
    g.box(b, "tally_hole_void", (-4.65, 25.62, -0.98), (-4.32, 25.88, -0.82), "void")
    g.box(b, "tally_bone_stud", (-4.65, 24.95, -0.95), (-4.35, 25.25, -0.75), "bone_lt")


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


class XiuTieTouKuiSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "helmet_crown": part_helmet_crown,
    "crown_dent": part_crown_dent,
    "brow_plate": part_brow_plate,
    "lamp_mount": part_lamp_mount,
    "lamp_crystal": part_lamp_crystal,
    "rim_flange": part_rim_flange,
    "liner_roll": part_liner_roll,
    "side_guards": part_side_guards,
    "chin_strap_r": part_chin_strap_r,
    "chin_strap_l": part_chin_strap_l,
    "back_neck_guard": part_back_neck_guard,
    "hang_tally": part_hang_tally,
}

SPEC = XiuTieTouKuiSpec(
    gear_id="xiu_tie_tou_kui",
    model_name="XiuTieTouKui",
    title="锈铁头盔 / xiu_tie_tou_kui",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：头 · 额前高对比碎琉璃矿灯座+头顶磕碰劈裂凹口+外翻衬布+下颌皮扣带",
    worn_focus=((0.0, 29.5, 0.0), 18.0),
    textured=False,
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
