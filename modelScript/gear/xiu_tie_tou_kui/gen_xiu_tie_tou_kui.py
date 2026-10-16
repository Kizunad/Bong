#!/usr/bin/env python3
"""锈铁头盔（xiu_tie_tou_kui）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土定制 NPC 专属装备 #8（sui_kuang_gong 碎矿工）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

返工要点（调度审阅意见）：
- 坚决杜绝方盒子感：用逐层收小的多面环体素把盔顶做成圆鼓的半球圆穹（顶部鼓起、四周弧形下收）；
- 下沿设明显外翻一圈的矿工大帽檐（flanged brim）；
- 前额矿灯座做大（宽深显著、青铜卡槽支架与铜耳），灯片极高对比度（亮白高光晶块+暗绿折射底+防碎铁栅）；
- 盔壳两侧增加凹坑（天雷砸痕与蚀坑）与凸起生铁铆钉排；
- 壳层整体落在头盒之外，严禁穿入头盒（x∈[-4,4], y∈[24,32], z∈[-4,4]）；
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
    "glass_hi":     (244, 254, 250),   # 矿灯极高对比高光亮白
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
    ("helmet_crown", "root", (0.0, 32.8, 0.0), "半球圆穹顶：逐层收小的多面铁环圆穹与防砸脊梁（对称）"),
    ("crown_dent", "helmet_crown", (-2.2, 33.5, -1.8), "盔顶劈裂凹坑：左前天雷崩裂口与粗焊铁疤"),
    ("helmet_brim", "helmet_crown", (0.0, 29.8, 0.0), "外翻大帽檐：环绕盔圈一周的倾斜外展挡石帽檐"),
    ("brow_plate", "helmet_crown", (0.0, 30.8, -4.4), "额前眉护铁板：前额加厚防砸铁板与鼻梁上护檐"),
    ("lamp_mount", "brow_plate", (0.0, 31.5, -4.8), "大型矿灯青铜基座：额前宽厚青铜灯座、支架与油管"),
    ("lamp_crystal", "lamp_mount", (0.0, 31.5, -5.2), "高对比矿灯反射晶片：大幅多面亮白反光琉璃与防护铁栅（核心高光）"),
    ("side_dents", "helmet_crown", (0.0, 31.0, 0.0), "侧盔凹坑与铆钉排：左右盔侧被碎石砸出的凹坑与密集铆钉"),
    ("liner_roll", "helmet_brim", (0.0, 29.2, 0.0), "外翻减震粗布衬：帽檐下方翻出的一圈粗麻软垫"),
    ("side_guards", "helmet_brim", (0.0, 27.5, 0.0), "左右耳部护铁片：太阳穴与耳侧下延护板"),
    ("chin_strap_r", "side_guards", (4.25, 24.5, -2.4), "右侧下颌皮扣带：斜向前伸至下颏的扣环皮带（可动带）"),
    ("chin_strap_l", "side_guards", (-4.25, 24.5, -2.4), "左侧磨断残皮带：左侧垂落的粗糙磨断破皮带端（可动带）"),
    ("back_neck_guard", "helmet_brim", (0.0, 28.5, 4.3), "后脑护颈铁叶：后颈垂下的三阶叠压防碎石铁叶（可微动）"),
    ("hang_tally", "side_guards", (-4.35, 26.5, -1.0), "工号兽骨签：系在左颊铆钉上的打孔矿工身份骨牌（挂件独立骨）"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "helmet_crown"))


# ───────────────────────────────────────────────────────────── 分部件实现
def part_helmet_crown(g: Gear) -> None:
    """半球圆穹顶：逐层收小的多面铁环圆穹，形成圆鼓饱满的半球形头盔（严格对称，用于镜像门禁自证）。"""
    b = "helmet_crown"
    # 圆穹分 5 个高度梯级收小，从 y=32.08 逐层收至 y=35.45
    # 第 1 级（低层大环，R=4.38，y∈[32.10, 32.78]）
    g.pair(b, "crown_t1_side", (3.75, 32.10, -2.85), (4.38, 32.78, 2.85), "iron")
    g.box(b, "crown_t1_front", (-2.85, 32.10, -4.38), (2.85, 32.78, -3.75), "iron")
    g.box(b, "crown_t1_back", (-2.85, 32.10, 3.75), (2.85, 32.78, 4.38), "iron")
    g.pair(b, "crown_t1_corn_f", (2.85, 32.13, -3.95), (3.95, 32.75, -2.85), "rust")
    g.pair(b, "crown_t1_corn_b", (2.85, 32.13, 2.85), (3.95, 32.75, 3.95), "rust")

    # 第 2 级（中低层环，R=3.85，y∈[32.70, 33.48]，充分搭接，错开 Y 避免共面）
    g.pair(b, "crown_t2_side", (3.15, 32.70, -2.45), (3.95, 33.48, 2.45), "iron")
    g.box(b, "crown_t2_front", (-2.45, 32.70, -3.92), (2.45, 33.48, -3.15), "iron")
    g.box(b, "crown_t2_back", (-2.45, 32.70, 3.15), (2.45, 33.48, 3.92), "iron")
    g.pair(b, "crown_t2_corn_f", (2.35, 32.73, -3.55), (3.55, 33.45, -2.35), "iron_lt")
    g.pair(b, "crown_t2_corn_b", (2.35, 32.73, 2.35), (3.55, 33.45, 3.55), "iron_lt")

    # 第 3 级（中高层环，R=3.05，y∈[33.40, 34.18]，错开 Y 避免共面）
    g.pair(b, "crown_t3_side", (2.35, 33.40, -1.95), (3.35, 34.18, 1.95), "iron_lt")
    g.box(b, "crown_t3_front", (-1.95, 33.40, -3.35), (1.95, 34.18, -2.35), "iron_lt")
    g.box(b, "crown_t3_back", (-1.95, 33.40, 2.35), (1.95, 34.18, 3.35), "iron_lt")
    g.pair(b, "crown_t3_corn_f", (1.85, 33.43, -2.85), (2.85, 34.15, -1.85), "iron")
    g.pair(b, "crown_t3_corn_b", (1.85, 33.43, 1.85), (2.85, 34.15, 2.85), "iron")

    # 第 4 级（高层球顶收束，R=2.15，y∈[34.10, 34.78]，错开 Y 避免共面）
    g.pair(b, "crown_t4_side", (1.45, 34.10, -1.35), (2.45, 34.78, 1.35), "iron")
    g.box(b, "crown_t4_front", (-1.35, 34.10, -2.45), (1.35, 34.78, -1.45), "iron")
    g.box(b, "crown_t4_back", (-1.35, 34.10, 1.45), (1.35, 34.78, 2.45), "iron")
    g.pair(b, "crown_t4_corn_f", (1.25, 34.13, -1.95), (1.95, 34.75, -1.25), "rust")
    g.pair(b, "crown_t4_corn_b", (1.25, 34.13, 1.25), (1.95, 34.75, 1.95), "rust")

    # 第 5 级（顶心球盖与铆合顶钮，y∈[34.75, 35.45]）
    g.box(b, "crown_dome_cap", (-1.35, 34.75, -1.35), (1.35, 35.15, 1.35), "iron_lt")
    g.box(b, "crown_dome_crest", (-0.85, 35.12, -0.85), (0.85, 35.45, 0.85), "iron")

    # 纵向纵贯防砸粗铁脊梁（加强筋，弧线顺应半球隆起）
    g.box(b, "crown_spine_t1_f", (-0.45, 32.35, -4.52), (0.45, 33.15, -3.65), "iron_lt")
    g.box(b, "crown_spine_t1_b", (-0.45, 32.35, 3.65), (0.45, 33.15, 4.52), "iron_lt")
    g.box(b, "crown_spine_t2_f", (-0.42, 33.05, -3.98), (0.42, 33.85, -2.85), "iron_lt")
    g.box(b, "crown_spine_t2_b", (-0.42, 33.05, 2.85), (0.42, 33.85, 3.98), "iron_lt")
    g.box(b, "crown_spine_t3_f", (-0.38, 33.75, -3.15), (0.38, 34.55, -1.85), "iron_lt")
    g.box(b, "crown_spine_t3_b", (-0.38, 33.75, 1.85), (0.38, 34.55, 3.15), "iron_lt")
    g.box(b, "crown_spine_top", (-0.35, 34.45, -1.85), (0.35, 35.25, 1.85), "iron_lt")

    # 脊梁上的对称加固铆钉（突出脊梁表面，错开共面）
    g.box(b, "crown_spine_stud_f1", (-0.18, 33.75, -3.55), (0.18, 34.15, -3.15), "iron_lt")
    g.box(b, "crown_spine_stud_b1", (-0.18, 33.75, 3.15), (0.18, 34.15, 3.55), "iron_lt")
    g.box(b, "crown_spine_stud_f2", (-0.18, 34.45, -2.35), (0.18, 34.85, -1.95), "iron_lt")
    g.box(b, "crown_spine_stud_b2", (-0.18, 34.45, 1.95), (0.18, 34.85, 2.35), "iron_lt")


def part_crown_dent(g: Gear) -> None:
    """盔顶劈裂凹坑：左前天雷崩裂口与粗焊铁疤（不对称）。"""
    b = "crown_dent"
    # 凹坑中心在 X=-2.15, Z=-2.05, Y∈[32.8, 34.2]
    g.box(b, "dent_pit_void", (-2.65, 32.95, -2.55), (-1.65, 33.85, -1.55), "void")
    g.box(b, "dent_pit_bed", (-2.85, 32.75, -2.75), (-1.45, 33.05, -1.35), "rust_dk")

    # 磕裂外翻的扭曲铁边（Y 厚度 ≥ 0.58，确保穿模门禁自证有鉴别力）
    g.box(b, "dent_gash_lip1", (-3.15, 33.10, -2.45), (-2.55, 33.72, -1.75), "iron_lt")
    g.box(b, "dent_gash_lip2", (-1.75, 33.10, -2.95), (-1.15, 33.72, -2.25), "iron_lt")
    g.box(b, "dent_gash_back", (-2.45, 33.25, -1.45), (-1.75, 33.85, -0.85), "rust")

    # 粗焊条补焊焊瘤
    g.box(b, "dent_weld_lump1", (-2.95, 33.45, -2.85), (-2.25, 33.85, -2.45), "rust")
    g.box(b, "dent_weld_lump2", (-2.15, 33.45, -1.55), (-1.45, 33.85, -1.05), "rust_dk")
    g.box(b, "dent_weld_seam", (-2.75, 33.55, -2.55), (-1.65, 33.78, -1.45), "iron")

    # 铆接加固铁片
    g.box(b, "dent_patch_rivet1", (-3.35, 33.35, -2.15), (-2.95, 33.75, -1.75), "iron_lt")
    g.box(b, "dent_patch_rivet2", (-1.55, 33.35, -3.05), (-1.15, 33.75, -2.65), "iron_lt")
    g.box(b, "dent_flake_a", (-2.85, 33.65, -2.15), (-2.55, 33.95, -1.75), "iron_lt")


def part_helmet_brim(g: Gear) -> None:
    """外翻大帽檐：环绕盔圈一周的倾斜外展挡石帽檐（Y∈[29.45, 30.55]，显著向外挑出！）。"""
    b = "helmet_brim"
    # 外翻檐口向外延展至 R=4.95，形成一圈清晰可见的矿工大翻檐！
    # 左右外翻侧檐（|x| 在 4.10 到 4.95）
    g.box(b, "brim_side_l", (-4.95, 29.55, -4.35), (-4.10, 30.15, 4.35), "iron")
    g.box(b, "brim_side_r", (4.10, 29.55, -4.35), (4.95, 30.15, 4.35), "iron")
    g.box(b, "brim_flare_l", (-5.15, 29.45, -3.95), (-4.75, 29.85, 3.95), "iron_lt")
    g.box(b, "brim_flare_r", (4.75, 29.45, -3.95), (5.15, 29.85, 3.95), "iron_lt")

    # 后颈外翻后檐（z 在 4.10 到 4.95）
    g.box(b, "brim_back", (-4.35, 29.55, 4.10), (4.35, 30.15, 4.95), "iron")
    g.box(b, "brim_back_flare", (-3.95, 29.45, 4.75), (3.95, 29.85, 5.15), "iron_lt")

    # 前额向前大挑檐（矿工防落石大檐板，向 -z 挑出至 -5.15！）
    g.box(b, "brim_front_visor", (-4.35, 29.65, -5.15), (4.35, 30.25, -4.10), "iron")
    g.box(b, "brim_front_lip", (-4.15, 29.55, -5.35), (4.15, 29.95, -4.85), "iron_lt")
    g.box(b, "brim_front_slope", (-3.85, 30.15, -4.95), (3.85, 30.65, -4.25), "iron_dk")

    # 檐口四切角过渡
    corners = (
        ("brim_corn_fl", -4.95, -3.95, -4.95, -3.95),
        ("brim_corn_fr", 3.95, 4.95, -4.95, -3.95),
        ("brim_corn_bl", -4.95, -3.95, 3.95, 4.95),
        ("brim_corn_br", 3.95, 4.95, 3.95, 4.95),
    )
    for name, x0, x1, z0, z1 in corners:
        g.box(b, name, (x0, 29.55, z0), (x1, 30.05, z1), "rust")

    # 帽檐一周密集生铁铆钉
    for k in range(5):
        z = -3.2 + k * 1.6
        g.box(b, f"brim_rivet_l_{k}", (-5.05, 29.85, z - 0.18), (-4.75, 30.15, z + 0.18), "iron_lt")
        g.box(b, f"brim_rivet_r_{k}", (4.75, 29.85, z - 0.18), (5.05, 30.15, z + 0.18), "iron_lt")


def part_brow_plate(g: Gear) -> None:
    """额前眉护铁板：前额加厚防砸铁板与加固筋带。"""
    b = "brow_plate"
    # 前眉板覆盖额前（X∈[-4.2, 4.2], Y∈[30.2, 32.3], Z∈[-4.65, -4.10]）
    g.box(b, "brow_plate_main", (-4.25, 30.25, -4.55), (4.25, 32.35, -4.10), "iron")
    g.box(b, "brow_plate_bevel_l", (-4.45, 30.55, -4.55), (-3.65, 31.85, -4.18), "iron_dk")
    g.box(b, "brow_plate_bevel_r", (3.65, 30.55, -4.55), (4.45, 31.85, -4.18), "iron_dk")

    # 前额横向加固铁带与五枚粗铁铆钉
    g.box(b, "brow_strap_bar", (-3.85, 30.85, -4.72), (3.85, 31.45, -4.45), "iron_dk")
    for k in range(5):
        x = -3.2 + k * 1.6
        g.box(b, f"brow_strap_rivet_{k}", (x - 0.18, 30.95, -4.85), (x + 0.18, 31.35, -4.55), "iron_lt")

    # 左右眉角向下斜向护板
    g.box(b, "brow_wing_l", (-4.45, 29.75, -4.62), (-3.75, 30.65, -4.12), "rust")
    g.box(b, "brow_wing_r", (3.75, 29.75, -4.62), (4.45, 30.65, -4.12), "rust_dk")


def part_lamp_mount(g: Gear) -> None:
    """大型矿灯青铜基座：额前宽厚青铜灯座、支架与油管（显著放大，形成视觉焦点！）。"""
    b = "lamp_mount"
    # 尺寸放大至 3.4 宽 × 3.2 高 × 2.0 深！
    # 中心 X=0.0, Y=31.45, Z∈[-5.55, -4.45]
    # 粗厚青铜方底盘（宽 3.4px）
    g.box(b, "lamp_base_plate", (-1.70, 30.25, -4.85), (1.70, 32.65, -4.45), "bronze")
    g.box(b, "lamp_base_rim_t", (-1.80, 32.55, -4.95), (1.80, 32.85, -4.42), "bronze")
    g.box(b, "lamp_base_rim_b", (-1.80, 30.05, -4.95), (1.80, 30.35, -4.42), "bronze")

    # 粗壮青铜支架侧爪（向前突出夹持灯片）
    g.box(b, "lamp_bracket_l", (-1.80, 30.45, -5.35), (-1.35, 32.45, -4.65), "bronze")
    g.box(b, "lamp_bracket_r", (1.35, 30.45, -5.35), (1.80, 32.45, -4.65), "bronze")
    g.box(b, "lamp_bracket_top", (-1.40, 32.25, -5.45), (1.40, 32.65, -4.75), "bronze")
    g.box(b, "lamp_bracket_bot", (-1.40, 30.25, -5.45), (1.40, 30.65, -4.75), "bronze")

    # 青铜加强筋与固定螺栓
    g.box(b, "lamp_rib_l", (-1.85, 31.15, -5.25), (-1.55, 31.75, -4.75), "iron_lt")
    g.box(b, "lamp_rib_r", (1.55, 31.15, -5.25), (1.85, 31.75, -4.75), "iron_lt")
    g.box(b, "lamp_mount_screw_t", (-0.25, 32.35, -5.52), (0.25, 32.65, -5.15), "iron_lt")
    g.box(b, "lamp_mount_screw_b", (-0.25, 30.25, -5.52), (0.25, 30.55, -5.15), "iron_lt")

    # 四角粗生铁固定大铆钉
    g.box(b, "lamp_rivet_tl", (-1.65, 32.35, -5.15), (-1.25, 32.75, -4.65), "iron_lt")
    g.box(b, "lamp_rivet_tr", (1.25, 32.35, -5.15), (1.65, 32.75, -4.65), "iron_lt")
    g.box(b, "lamp_rivet_bl", (-1.65, 30.15, -5.15), (-1.25, 30.55, -4.65), "iron_lt")
    g.box(b, "lamp_rivet_br", (1.25, 30.15, -5.15), (1.65, 30.55, -4.65), "iron_lt")

    # 矿灯下方进油铜管与油盒底托
    g.box(b, "lamp_oil_cup", (-0.55, 29.55, -5.05), (0.55, 30.25, -4.55), "bronze")
    g.box(b, "lamp_oil_pipe", (-0.22, 29.25, -4.95), (0.22, 29.65, -4.65), "bronze")


def part_lamp_crystal(g: Gear) -> None:
    """高对比矿灯反射晶片：极高对比度大块抛光碎琉璃透镜与防碎铁栅（前额核心焦点！）。"""
    b = "lamp_crystal"
    # 中心 X=0.0, Y=31.45, Z∈[-5.55, -4.85]
    # 琉璃核心反光晶块（大块高透绿琉璃底座，宽 2.4×高 1.8px）
    g.box(b, "crystal_main_body", (-1.20, 30.55, -5.25), (1.20, 32.35, -4.75), "glass")

    # 矿灯中心极高对比耀眼亮白高光块（多阶梯反射棱面，强化对比度）
    g.box(b, "crystal_glint_core", (-0.75, 30.95, -5.55), (0.75, 31.95, -5.15), "glass_hi")
    g.box(b, "crystal_glint_top", (-0.55, 31.85, -5.48), (0.55, 32.25, -5.05), "glass_hi")
    g.box(b, "crystal_glint_bot", (-0.55, 30.65, -5.48), (0.55, 31.05, -5.05), "glass_hi")
    g.box(b, "crystal_glint_l", (-1.05, 31.15, -5.45), (-0.65, 31.75, -5.05), "glass_hi")
    g.box(b, "crystal_glint_r", (0.65, 31.15, -5.45), (1.05, 31.75, -5.05), "glass_hi")

    # 晶片折射暗边与破损深裂隙（产生强烈明暗反差）
    g.box(b, "crystal_crack_dark", (-0.95, 31.25, -5.38), (0.25, 31.65, -4.95), "glass_dk")
    g.box(b, "crystal_crack_branch", (-0.15, 30.85, -5.38), (0.55, 31.35, -4.95), "glass_dk")
    g.box(b, "crystal_edge_shade_l", (-1.25, 30.55, -5.18), (-0.85, 32.35, -4.78), "glass_dk")
    g.box(b, "crystal_edge_shade_r", (0.85, 30.55, -5.18), (1.25, 32.35, -4.78), "glass_dk")

    # 前护镜十字防砸生铁防护细条
    g.box(b, "crystal_grill_h", (-1.35, 31.35, -5.62), (1.35, 31.55, -5.25), "iron_lt")
    g.box(b, "crystal_grill_v", (-0.12, 30.45, -5.62), (0.12, 32.45, -5.25), "iron_lt")


def part_side_dents(g: Gear) -> None:
    """侧盔凹坑与铆钉排：左右盔侧被碎石砸出的凹坑与密集铆钉。"""
    b = "side_dents"
    # 左侧深砸凹痕与裂纹（X∈[-4.45, -3.95], Y∈[31.2, 32.8], Z∈[-1.5, 1.5]）
    g.box(b, "dent_side_l1_void", (-4.45, 31.65, -0.65), (-4.10, 32.35, 0.15), "void")
    g.box(b, "dent_side_l1_pit", (-4.38, 31.45, -0.85), (-4.12, 32.55, 0.35), "rust_dk")
    g.box(b, "dent_side_l1_rim", (-4.48, 31.35, -0.95), (-4.15, 32.65, 0.45), "rust")

    # 右侧受击凹坑
    g.box(b, "dent_side_r1_pit", (4.12, 31.55, -0.25), (4.45, 32.45, 0.65), "rust_dk")
    g.box(b, "dent_side_r1_rim", (4.15, 31.45, -0.35), (4.48, 32.55, 0.75), "rust")

    # 左右侧面密集铆接铁条与铆钉排
    g.box(b, "side_seam_plate_l", (-4.42, 30.65, -1.85), (-4.12, 32.85, -1.35), "iron_dk")
    g.box(b, "side_seam_plate_r", (4.12, 30.65, -1.85), (4.42, 32.85, -1.35), "iron_dk")
    for k in range(3):
        y = 30.85 + k * 0.85
        g.box(b, f"side_rivet_l_{k}", (-4.55, y, -1.72), (-4.22, y + 0.28, -1.48), "iron_lt")
        g.box(b, f"side_rivet_r_{k}", (4.22, y, -1.72), (4.55, y + 0.28, -1.48), "iron_lt")

    for k in range(3):
        z = -0.55 + k * 0.95
        g.box(b, f"side_mid_rivet_l_{k}", (-4.52, 32.55, z), (-4.25, 32.85, z + 0.28), "iron_lt")
        g.box(b, f"side_mid_rivet_r_{k}", (4.25, 32.55, z), (4.52, 32.85, z + 0.28), "iron_lt")


def part_liner_roll(g: Gear) -> None:
    """外翻减震粗布衬：帽檐下方翻出的一圈粗麻软垫。"""
    b = "liner_roll"
    # 左右侧外翻粗布卷（位于帽檐下缘 Y∈[28.85, 29.55]）
    g.box(b, "liner_roll_l", (-4.58, 28.85, -4.20), (-4.10, 29.55, 4.20), "cloth")
    g.box(b, "liner_roll_r", (4.10, 28.85, -4.20), (4.58, 29.55, 4.20), "cloth")
    g.box(b, "liner_crease_l", (-4.65, 28.95, -3.95), (-4.22, 29.35, 3.95), "cloth_dk")
    g.box(b, "liner_crease_r", (4.22, 28.95, -3.95), (4.65, 29.35, 3.95), "cloth_dk")

    # 后颈外翻粗布卷
    g.box(b, "liner_roll_b", (-4.20, 28.85, 4.10), (4.20, 29.55, 4.58), "cloth")
    g.box(b, "liner_crease_b", (-3.95, 28.95, 4.22), (3.95, 29.35, 4.65), "cloth_dk")

    # 衬布翻边锁线粗针脚
    for k in range(5):
        z = -2.65 + k * 1.35
        g.box(b, f"liner_stitch_l_{k}", (-4.68, 29.15, z), (-4.18, 29.38, z + 0.28), "stitch")
        g.box(b, f"liner_stitch_r_{k}", (4.18, 29.15, z), (4.68, 29.38, z + 0.28), "stitch")


def part_side_guards(g: Gear) -> None:
    """左右耳部护铁片：太阳穴与耳侧下延护板（|x| ≥ 4.10）。"""
    b = "side_guards"
    # 左耳护板
    g.box(b, "ear_plate_l_main", (-4.45, 26.25, -2.05), (-4.10, 29.55, 1.25), "iron")
    g.box(b, "ear_plate_l_flange", (-4.55, 26.15, -1.95), (-4.22, 29.65, 1.15), "iron_lt")
    g.box(b, "ear_plate_l_low", (-4.42, 25.45, -1.65), (-4.12, 26.35, 0.85), "iron")
    g.box(b, "ear_slit_l_void", (-4.62, 27.25, -0.65), (-4.25, 28.45, -0.15), "void")
    g.box(b, "ear_hinge_l", (-4.52, 28.95, -0.65), (-4.18, 29.45, -0.15), "rust")

    # 右耳护板
    g.box(b, "ear_plate_r_main", (4.10, 26.25, -2.05), (4.45, 29.55, 1.25), "iron")
    g.box(b, "ear_plate_r_flange", (4.22, 26.15, -1.95), (4.55, 29.65, 1.15), "iron_lt")
    g.box(b, "ear_plate_r_low", (4.12, 25.45, -1.65), (4.42, 26.35, 0.85), "iron")
    g.box(b, "ear_slit_r_void", (4.25, 27.25, -0.65), (4.62, 28.45, -0.15), "void")
    g.box(b, "ear_hinge_r", (4.18, 28.95, -0.65), (4.52, 29.45, -0.15), "rust")

    # 左右护板防砍凸筋与边缘铆钉
    g.box(b, "ear_rib_l", (-4.52, 26.45, -1.85), (-4.28, 29.35, -1.55), "iron_dk")
    g.box(b, "ear_rib_r", (4.28, 26.45, -1.85), (4.52, 29.35, -1.55), "iron_dk")
    g.box(b, "ear_stud_l", (-4.62, 26.65, -0.45), (-4.32, 26.95, -0.15), "iron_lt")
    g.box(b, "ear_stud_r", (4.32, 26.65, -0.45), (4.62, 26.95, -0.15), "iron_lt")


def part_chin_strap_r(g: Gear) -> None:
    """右侧下颌皮扣带：斜向前伸至下颏的扣环皮带（严格在右臂盒前 z ≤ -2.15，彻底避开手臂盒）。"""
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

    # 下部向前延伸至 z ≤ -2.15
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
    g.box(b, "neck_plate_2_stud_l", (-2.85, 27.25, 4.45), (-2.55, 27.55, 4.68), "iron_lt")
    g.box(b, "neck_plate_2_stud_r", (2.55, 27.25, 4.45), (2.85, 27.55, 4.68), "iron_lt")

    # 第三层中心尾盾小铁片
    g.box(b, "neck_plate_3", (-1.85, 25.85, 4.18), (1.85, 27.05, 4.48), "rust")
    g.box(b, "neck_plate_3_tip", (-1.15, 25.25, 4.22), (1.15, 25.95, 4.45), "rust_dk")
    g.box(b, "neck_plate_3_rib", (-0.45, 25.35, 4.45), (0.45, 26.95, 4.65), "iron_lt")

    # 铰接铆钉
    for k in range(3):
        x = -2.2 + k * 2.2
        g.box(b, f"neck_rivet_row1_{k}", (x - 0.18, 28.85, 4.38), (x + 0.18, 29.15, 4.62), "iron_lt")
        g.box(b, f"neck_rivet_row2_{k}", (x - 0.18, 27.65, 4.42), (x + 0.18, 27.95, 4.65), "iron_lt")


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
    "helmet_brim": part_helmet_brim,
    "brow_plate": part_brow_plate,
    "lamp_mount": part_lamp_mount,
    "lamp_crystal": part_lamp_crystal,
    "side_dents": part_side_dents,
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
    wear_note="挂点：头 · 圆穹半球盔顶+外翻大帽檐+额前大矿灯座高对比反射镜+侧面凹坑铆钉",
    worn_focus=((0.0, 29.5, 0.0), 18.0),
    textured=False,
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
