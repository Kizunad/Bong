#!/usr/bin/env python3
"""铁炉座（tie_lu_zuo）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #10（地上摆件 / 器座）。
世界观基调：
- 精致规整的古代铸铁器座：高约 11.6 px、宽深约 16 px 的方形三层宏大台座。
- 造型严格遵从调度意见返工（彻底杜绝矮扁方砖）：
  * 整体增高至 11.6 px（下层宽座 2.2px、中层收腰 4.0px、上层外扩承盘 3.2px、四角兽足 4.2px）；
  * 中层收腰显著加高（高 4.0px），并明显向内缩进 2.4px（从下座 14.6px 缩至 9.8px，缩进达 2.4px，远超 2px 门槛）；
  * 四角铸铁兽足更加高大威猛（高度达 4.2px），做出具象饕餮兽面膝盖顶座（兽角、兽目、阔吻）与粗壮外撇三爪（爪尖踏地 y=0.0）；
  * 上层承盘显著向外扩展（宽达 15.2px，多级托拱向外挑出）；
  * 束腰四面环绕高大舒展的如意云头纹浮雕（如意顶拱、云心宝珠、卷云耳与舒展云尾）；
  * 承盘四周高起防溢挡灰立沿与环形排灰槽，中央深凹双同心死寂阵环凹线（深暗无光 void，不发光）；
  * 底座四角与承盘四角铆接生铁包角铁箍与凸起铆钉排。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/metalware/tie_lu_zuo/gen_tie_lu_zuo.py --self-test
    python3 modelScript/metalware/tie_lu_zuo/gen_tie_lu_zuo.py --gates
    python3 modelScript/metalware/tie_lu_zuo/gen_tie_lu_zuo.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "gear" / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "iron":         (72, 68, 66),      # 铸铁黑褐氧化皮（台座主体底色）
    "iron_lt":      (126, 122, 120),   # 凸起棱角、铆钉与磨光高光
    "iron_dk":      (46, 42, 42),      # 阴刻槽底、排灰槽与阴影接缝暗铁
    "ruyi":         (88, 82, 80),      # 如意云头纹浮雕（比底色稍亮一档，沉稳微亮）
    "rust":         (146, 76, 46),     # 火燎红褐铁锈斑
    "rust_dk":      (96, 48, 30),      # 浓重氧化蚀斑
    "bronze_pat":   (92, 116, 98),     # 局部青铜锈斑（旧时代残留）
    "ash":          (166, 164, 158),   # 承盘死角积存的浅冷灰
    "ash_dk":       (112, 108, 104),   # 炭灰深色垢层
    "void":         (18, 16, 14),      # 灵气失效的导灵凹槽与排灰暗孔（死寂深黑）
}

SOFT = ("ash", "ash_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS)) | soft_pairs(("ruyi",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("base_plinth", "root", (0.0, 3.2, 0.0), "下层宽座：宽厚方形底台、上阶收折与底面防变形暗槽（对称）"),
    ("beast_legs_f", "base_plinth", (0.0, 2.0, -6.2), "前部高大兽足：高大兽首膝盖座与前探外撇兽爪（对称，踏地 y=0）"),
    ("beast_legs_b", "base_plinth", (0.0, 2.0, 6.2), "后部高大兽足：高大兽首膝盖座与后探外撇兽爪（对称，踏地 y=0）"),
    ("waist_tier", "base_plinth", (0.0, 6.4, 0.0), "中层高收腰体：高度达4px且缩进2.4px的高耸束腰方台（对称）"),
    ("ruyi_relief", "waist_tier", (0.0, 6.4, 0.0), "收腰如意云纹：束腰四面环绕的舒展高大如意云头浮雕饰带（对称）"),
    ("upper_platter", "waist_tier", (0.0, 9.6, 0.0), "上层外扩承盘：多级斜展托拱与向外显著扩展的大承盘（对称）"),
    ("platter_rim", "upper_platter", (0.0, 11.0, 0.0), "承盘立沿唇边：四周高起1px挡灰围沿与环形排灰凹槽（对称）"),
    ("dead_array_rings", "upper_platter", (0.0, 10.4, 0.0), "死寂同心阵环：台面中央灵气失效的同心阵环凹线与导灵十字槽（对称）"),
    ("corner_irons_base", "base_plinth", (0.0, 3.2, 0.0), "下座四角铁箍：下层底台四角铆接生铁包角与加固铆钉（对称）"),
    ("corner_irons_top", "upper_platter", (0.0, 10.6, 0.0), "承盘四角铁箍：上层盘角铆接生铁包角与加固铆钉（对称）"),
    ("ash_sediment", "upper_platter", (3.2, 10.5, -3.2), "角落残存冷灰：排灰槽死角薄薄积存的陈年炭灰（不对称）"),
    ("wear_scars", "base_plinth", (-6.5, 3.2, 6.5), "旧蚀氧化痕：底座边角火燎红斑与磨蚀铁茬（不对称）"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in (
    "root", "base_plinth", "beast_legs_f", "beast_legs_b", "waist_tier", "ruyi_relief",
    "upper_platter", "platter_rim", "dead_array_rings", "corner_irons_base", "corner_irons_top"
))


# ───────────────────────────────────────────────────────────── 零件建模实现
def part_base_plinth(g: Gear) -> None:
    """下层宽座：宽厚方形底台、上阶收折与底面防变形暗槽（对称）。"""
    b = "base_plinth"
    # 1. 底层最宽主方台（15.6 × 15.6 px，Y∈[2.25, 3.35]）
    g.box(b, "plinth_base_main", (-7.75, 2.25, -7.75), (7.75, 3.35, 7.75), "iron")

    # 底板下缘收角包边（前后横条两端内缩至 ±7.55，侧边条内缩至 ±7.65，消除与底板主板端面共面）
    g.box(b, "plinth_base_rim_f", (-7.55, 2.22, -7.82), (7.55, 2.65, -7.62), "iron_dk")
    g.box(b, "plinth_base_rim_b", (-7.55, 2.22, 7.62), (7.55, 2.65, 7.82), "iron_dk")
    g.pair(b, "plinth_base_rim_side", (7.62, 2.22, -7.65), (7.82, 2.65, 7.65), "iron_dk")

    # 底面中心暗凹防变形槽（三轴尺寸均 ≥ 0.62，用于自证穿模）
    g.box(b, "plinth_cavity_void", (-1.25, 2.05, -1.25), (1.25, 2.45, 1.25), "void")
    g.box(b, "plinth_cavity_lip", (-1.45, 2.12, -1.45), (1.45, 2.35, 1.45), "rust")

    # 2. 上阶收折层（14.6 × 14.6 px，Y∈[3.35, 4.45]）
    g.box(b, "plinth_tier2_main", (-7.25, 3.35, -7.25), (7.25, 4.45, 7.25), "iron")
    # 上阶挑沿出唇（前后横条两端内缩至 ±7.05，侧条通长至 ±7.35）
    g.box(b, "plinth_tier2_lip_f", (-7.05, 4.15, -7.42), (7.05, 4.48, -7.15), "iron_lt")
    g.box(b, "plinth_tier2_lip_b", (-7.05, 4.15, 7.15), (7.05, 4.48, 7.42), "iron_lt")
    g.pair(b, "plinth_tier2_lip_side", (7.15, 4.15, -7.35), (7.42, 4.48, 7.35), "iron_lt")

    # 底座四边中部生铁加固立筋条（前后左右各一对加固条）
    g.pair(b, "plinth_side_brace_f", (2.15, 2.55, -7.85), (3.15, 3.85, -7.65), "iron_lt")
    g.pair(b, "plinth_side_brace_b", (2.15, 2.55, 7.65), (3.15, 3.85, 7.85), "iron_lt")
    g.pair(b, "plinth_side_brace_side", (7.65, 2.55, -0.50), (7.85, 3.85, 0.50), "iron_lt")


def part_beast_legs_f(g: Gear) -> None:
    """前部高大兽足：高大兽首膝盖座与前探外撇兽爪（成对镜像对称，高度达4.2px，踏地 y=0）。"""
    b = "beast_legs_f"
    cx, cz = 6.25, -6.25
    # 1. 具象饕餮兽面膝盖顶座（Y∈[2.15, 4.25]，底面下探错开底台底面）
    g.pair(b, "leg_f_knee_block", (cx - 1.25, 2.15, cz - 1.25), (cx + 1.25, 3.85, cz + 1.25), "iron")
    # 兽角（高高耸起）
    g.pair(b, "leg_f_horn_crest", (cx - 0.45, 3.65, cz - 1.55), (cx + 0.45, 4.25, cz - 0.75), "iron_lt")
    g.pair(b, "leg_f_horn_tip", (cx - 0.35, 4.05, cz - 1.75), (cx + 0.35, 4.45, cz - 1.35), "iron_lt")
    # 粗大怒目眉骨与兽眼
    g.pair(b, "leg_f_knee_brow", (cx - 1.05, 3.15, cz - 1.45), (cx + 1.05, 3.68, cz - 0.85), "iron_lt")
    g.pair(b, "leg_f_knee_eye", (cx + 0.45, 2.75, cz - 1.38), (cx + 0.95, 3.25, cz - 0.95), "iron_dk")
    g.pair(b, "leg_f_knee_eye_in", (cx - 0.95, 2.75, cz - 1.38), (cx - 0.45, 3.25, cz - 0.95), "iron_dk")
    # 阔鼻吻部
    g.pair(b, "leg_f_knee_snout", (cx - 0.55, 2.45, cz - 1.48), (cx + 0.55, 2.95, cz - 1.05), "iron")

    # 2. 粗壮曲腿胫骨（向下向斜外前方大角度外撇！）
    g.pair(b, "leg_f_shank_main", (cx - 1.05, 0.85, cz - 1.45), (cx + 1.05, 2.45, cz + 0.35), "iron")
    g.pair(b, "leg_f_shank_ridge", (cx - 0.35, 0.95, cz - 1.75), (cx + 0.35, 2.68, cz - 1.25), "iron_lt")
    g.pair(b, "leg_f_shank_flange_o", (cx + 0.85, 1.15, cz - 1.35), (cx + 1.35, 2.25, cz - 0.25), "iron_dk")
    g.pair(b, "leg_f_shank_flange_i", (cx - 1.35, 1.15, cz - 1.35), (cx - 0.85, 2.25, cz - 0.25), "iron_dk")

    # 3. 踏地三爪足（三爪彻底分离，爪尖牢固着地 y=0.0）
    # 中爪（强力前探）
    g.pair(b, "leg_f_toe_mid", (cx - 0.52, 0.00, cz - 2.35), (cx + 0.52, 0.95, cz - 1.45), "iron")
    g.pair(b, "leg_f_claw_mid", (cx - 0.38, 0.02, cz - 2.68), (cx + 0.38, 0.55, cz - 2.15), "iron_lt")
    # 内侧爪（偏内侧，X 严格在 [cx-1.65, cx-0.58]）
    g.pair(b, "leg_f_toe_in", (cx - 1.65, 0.00, cz - 1.45), (cx - 0.58, 0.85, cz - 0.55), "iron")
    g.pair(b, "leg_f_claw_in", (cx - 1.95, 0.02, cz - 1.65), (cx - 1.35, 0.45, cz - 1.15), "iron_lt")
    # 外侧爪（偏外侧，X 严格在 [cx+0.58, cx+1.65]）
    g.pair(b, "leg_f_toe_out", (cx + 0.58, 0.00, cz - 1.45), (cx + 1.65, 0.85, cz - 0.55), "iron")
    g.pair(b, "leg_f_claw_out", (cx + 1.35, 0.02, cz - 1.65), (cx + 1.95, 0.45, cz - 1.15), "iron_lt")
    # 脚跟掌垫（Z 严格在 [cz-0.45, cz+0.65]）
    g.pair(b, "leg_f_heel_pad", (cx - 0.85, 0.00, cz - 0.45), (cx + 0.85, 0.75, cz + 0.65), "iron_dk")


def part_beast_legs_b(g: Gear) -> None:
    """后部高大兽足：高大兽首膝盖座与后探外撇兽爪（成对镜像对称，高度达4.2px，踏地 y=0）。"""
    b = "beast_legs_b"
    cx, cz = 6.25, 6.25
    # 1. 具象饕餮兽面膝盖顶座（Y∈[2.15, 4.25]）
    g.pair(b, "leg_b_knee_block", (cx - 1.25, 2.15, cz - 1.25), (cx + 1.25, 3.85, cz + 1.25), "iron")
    # 兽角（高高耸起）
    g.pair(b, "leg_b_horn_crest", (cx - 0.45, 3.65, cz + 0.75), (cx + 0.45, 4.25, cz + 1.55), "iron_lt")
    g.pair(b, "leg_b_horn_tip", (cx - 0.35, 4.05, cz + 1.35), (cx + 0.35, 4.45, cz + 1.75), "iron_lt")
    # 粗大怒目眉骨与兽眼
    g.pair(b, "leg_b_knee_brow", (cx - 1.05, 3.15, cz + 0.85), (cx + 1.05, 3.68, cz + 1.45), "iron_lt")
    g.pair(b, "leg_b_knee_eye", (cx + 0.45, 2.75, cz + 0.95), (cx + 0.95, 3.25, cz + 1.38), "iron_dk")
    g.pair(b, "leg_b_knee_eye_in", (cx - 0.95, 2.75, cz + 0.95), (cx - 0.45, 3.25, cz + 1.38), "iron_dk")
    # 阔鼻吻部
    g.pair(b, "leg_b_knee_snout", (cx - 0.55, 2.45, cz + 1.05), (cx + 0.55, 2.95, cz + 1.48), "iron")

    # 2. 粗壮曲腿胫骨（向下向斜外后方大角度外撇！）
    g.pair(b, "leg_b_shank_main", (cx - 1.05, 0.85, cz - 0.35), (cx + 1.05, 2.45, cz + 1.45), "iron")
    g.pair(b, "leg_b_shank_ridge", (cx - 0.35, 0.95, cz + 1.25), (cx + 0.35, 2.68, cz + 1.75), "iron_lt")
    g.pair(b, "leg_b_shank_flange_o", (cx + 0.85, 1.15, cz + 0.25), (cx + 1.35, 2.25, cz + 1.35), "iron_dk")
    g.pair(b, "leg_b_shank_flange_i", (cx - 1.35, 1.15, cz + 0.25), (cx - 0.85, 2.25, cz + 1.38), "iron_dk")

    # 3. 踏地三爪足（爪尖牢固着地 y=0.0）
    # 中爪（强力后探）
    g.pair(b, "leg_b_toe_mid", (cx - 0.52, 0.00, cz + 1.45), (cx + 0.52, 0.95, cz + 2.35), "iron")
    g.pair(b, "leg_b_claw_mid", (cx - 0.38, 0.02, cz + 2.15), (cx + 0.38, 0.55, cz + 2.68), "iron_lt")
    # 内侧爪
    g.pair(b, "leg_b_toe_in", (cx - 1.65, 0.00, cz + 0.55), (cx - 0.58, 0.85, cz + 1.45), "iron")
    g.pair(b, "leg_b_claw_in", (cx - 1.95, 0.02, cz + 1.15), (cx - 1.35, 0.45, cz + 1.65), "iron_lt")
    # 外侧爪
    g.pair(b, "leg_b_toe_out", (cx + 0.58, 0.00, cz + 0.55), (cx + 1.65, 0.85, cz + 1.45), "iron")
    g.pair(b, "leg_b_claw_out", (cx + 1.35, 0.02, cz + 1.15), (cx + 1.95, 0.45, cz + 1.65), "iron_lt")
    # 脚跟掌垫
    g.pair(b, "leg_b_heel_pad", (cx - 0.85, 0.00, cz - 0.65), (cx + 0.85, 0.75, cz + 0.45), "iron_dk")


def part_waist_tier(g: Gear) -> None:
    """中层高收腰体：高度达4.0px且缩进达2.4px的高耸束腰方台（绝对杜绝矮扁！对称）。"""
    b = "waist_tier"
    # 核心高收腰方柱：收缩到 9.8 × 9.8 px（缩进达 2.4px！高整整 4.0px，Y∈[4.42, 8.48]）
    g.box(b, "waist_core_pillar", (-4.90, 4.42, -4.90), (4.90, 8.48, 4.90), "iron")

    # 束腰上下过渡的内敛倒角弦线
    # 下束腰倒角台唇（前后横条两端内缩至 ±4.85，侧条通长，消除角部共面）
    g.box(b, "waist_bottom_lip_f", (-4.85, 4.45, -5.25), (4.85, 4.95, -4.92), "iron_dk")
    g.box(b, "waist_bottom_lip_b", (-4.85, 4.45, 4.92), (4.85, 4.95, 5.25), "iron_dk")
    g.pair(b, "waist_bottom_lip_side", (4.92, 4.45, -5.15), (5.25, 4.95, 5.15), "iron_dk")

    # 上束腰倒角托台（前后横条两端内缩至 ±4.85，侧条通长，消除角部共面）
    g.box(b, "waist_top_lip_f", (-4.85, 7.95, -5.35), (4.85, 8.45, -4.92), "iron_dk")
    g.box(b, "waist_top_lip_b", (-4.85, 7.95, 4.92), (4.85, 8.45, 5.35), "iron_dk")
    g.pair(b, "waist_top_lip_side", (4.92, 7.95, -5.25), (5.35, 8.45, 5.25), "iron_dk")

    # 束腰四角内凹立柱角槽（高度 3.2px，增添多折面阴影深度）
    g.pair(b, "waist_corner_notch_f", (4.55, 4.65, -4.95), (4.95, 8.25, -4.55), "iron_dk")
    g.pair(b, "waist_corner_notch_b", (4.55, 4.65, 4.55), (4.95, 8.25, 4.95), "iron_dk")


def part_ruyi_relief(g: Gear) -> None:
    """收腰如意云纹：束腰四面环绕的高大舒展如意云头浮雕（高2.8px，生动多层深度，严格对称）。"""
    b = "ruyi_relief"
    # 在 4.0px 高的束腰外壁（Z=±4.90, X=±4.90，Y∈[5.0, 7.8]）向外凸出多层次如意浮雕
    # 1. 前后两面高大如意云头浮雕
    for side, z_face, sign in (("f", -4.90, -1), ("b", 4.90, 1)):
        # 主云头厚度 0.38px
        z_t_0, z_t_1 = (z_face - 0.38, z_face) if sign == -1 else (z_face, z_face + 0.38)
        # 云心宝珠厚度 0.42px
        z_p_0, z_p_1 = (z_face - 0.42, z_face) if sign == -1 else (z_face, z_face + 0.42)
        # 云耳厚度 0.32px
        z_e_0, z_e_1 = (z_face - 0.32, z_face) if sign == -1 else (z_face, z_face + 0.32)
        # 云尾厚度 0.26px
        z_w_0, z_w_1 = (z_face - 0.26, z_face) if sign == -1 else (z_face, z_face + 0.26)
        # 侧云蕾厚度 0.20px
        z_c_0, z_c_1 = (z_face - 0.20, z_face) if sign == -1 else (z_face, z_face + 0.20)

        # 中央如意云头顶拱（自身对称，高耸主拱，X ∈ [-0.85, 0.85]，Y∈[6.05, 7.65]）
        g.box(b, f"ruyi_mid_crown_{side}", (-0.85, 6.05, z_t_0), (0.85, 7.65, z_t_1), "ruyi")
        # 云心宝珠（Y∈[5.25, 5.95]，与主云头在 Y 轴完全分离无交集）
        g.box(b, f"ruyi_mid_pearl_{side}", (-0.45, 5.25, z_p_0), (0.45, 5.95, z_p_1), "ruyi")

        # 左右展开的卷云耳与舒展云尾（X 区间严格错开对接，无重叠）
        g.pair(b, f"ruyi_wing_ear_{side}", (0.88, 5.95, z_e_0), (1.95, 7.55, z_e_1), "ruyi")
        g.pair(b, f"ruyi_wing_curl_{side}", (1.35, 5.25, z_e_0), (2.35, 5.85, z_e_1), "ruyi")
        g.pair(b, f"ruyi_wing_tail_{side}", (2.38, 5.45, z_w_0), (3.65, 6.65, z_w_1), "ruyi")
        g.pair(b, f"ruyi_side_cloud_{side}", (3.68, 5.85, z_c_0), (4.75, 7.25, z_c_1), "ruyi")

    # 2. 左右两侧外壁高大如意云头浮雕（成对对称）
    # 中央主云头厚度 0.38px
    g.pair(b, "ruyi_side_crown", (4.90, 6.05, -0.85), (5.28, 7.65, 0.85), "ruyi")
    # 云心宝珠（Z 严格在 [-0.45, 0.45]，Y∈[5.25, 5.95]）
    g.pair(b, "ruyi_side_pearl", (4.90, 5.25, -0.45), (5.32, 5.95, 0.45), "ruyi")
    # 侧壁卷云耳（厚度 0.32px，Z 严格错开对接）
    g.pair(b, "ruyi_side_ear_f", (4.90, 5.95, -1.95), (5.22, 7.55, -0.88), "ruyi")
    g.pair(b, "ruyi_side_ear_b", (4.90, 5.95, 0.88), (5.22, 7.55, 1.95), "ruyi")
    g.pair(b, "ruyi_side_curl_f", (4.90, 5.25, -2.35), (5.22, 5.85, -1.35), "ruyi")
    g.pair(b, "ruyi_side_curl_b", (4.90, 5.25, 1.35), (5.22, 5.85, 2.35), "ruyi")
    # 侧壁云尾（厚度 0.26px）
    g.pair(b, "ruyi_side_tail_f", (4.90, 5.45, -3.65), (5.16, 6.65, -2.38), "ruyi")
    g.pair(b, "ruyi_side_tail_b", (4.90, 5.45, 2.38), (5.16, 6.65, 3.65), "ruyi")
    # 侧壁小云蕾（厚度 0.20px）
    g.pair(b, "ruyi_side_flank_f", (4.90, 5.85, -4.75), (5.10, 7.25, -3.68), "iron_lt")
    g.pair(b, "ruyi_side_flank_b", (4.90, 5.85, 3.68), (5.10, 7.25, 4.75), "iron_lt")


def part_upper_platter(g: Gear) -> None:
    """上层外扩承盘：多级斜展托拱与向外显著扩展的大承盘（宽达15.2px，对称）。"""
    b = "upper_platter"
    # 1. 下层斜展托拱阶梯（从束腰 9.8 逐层外扩至 11.6、13.2 px，Y∈[8.48, 9.85]）
    g.box(b, "bracket_arch_tier1", (-5.80, 8.48, -5.80), (5.80, 9.15, 5.80), "iron")
    g.box(b, "bracket_arch_tier2", (-6.60, 9.15, -6.60), (6.60, 9.85, 6.60), "iron")

    # 托拱下缘承重装饰生铁斜撑牛腿（成对对称）
    g.pair(b, "platter_corbel_f", (2.85, 8.55, -6.68), (4.35, 9.75, -5.95), "iron_lt")
    g.pair(b, "platter_corbel_b", (2.85, 8.55, 5.95), (4.35, 9.75, 6.68), "iron_lt")
    g.pair(b, "platter_corbel_side", (5.95, 8.55, -0.75), (6.68, 9.75, 0.75), "iron_lt")

    # 2. 承盘主托板（宽 15.2 × 深 15.2 px，厚度收于 Y=10.25，作为承盘面与排灰槽底的承重基底！）
    g.box(b, "platter_floor_plate", (-7.55, 9.85, -7.55), (7.55, 10.25, 7.55), "iron")
    # 托板底面斜削过渡倒角（Y∈[9.82, 10.15]，前后两端内缩至 ±7.25，侧条内缩至 ±7.45）
    g.box(b, "platter_chamfer_f", (-7.25, 9.82, -7.65), (7.25, 10.15, -7.25), "iron_dk")
    g.box(b, "platter_chamfer_b", (-7.25, 9.82, 7.25), (7.25, 10.15, 7.65), "iron_dk")
    g.pair(b, "platter_chamfer_side", (7.25, 9.82, -7.45), (7.65, 10.15, 7.45), "iron_dk")


def part_platter_rim(g: Gear) -> None:
    """承盘立沿唇边：四周高起1px挡灰围沿与环形排灰凹槽（严格对称）。"""
    b = "platter_rim"
    # 四周高起立沿（外宽 15.2px，高起至 Y=11.62，高出台面 1.0px，前后通长，左右侧条内缩避开角部共面）
    g.box(b, "platter_wall_f", (-7.60, 10.55, -7.62), (7.60, 11.55, -6.85), "iron")
    g.box(b, "platter_wall_b", (-7.60, 10.55, 6.85), (7.60, 11.55, 7.62), "iron")
    g.pair(b, "platter_wall_side", (6.85, 10.55, -6.85), (7.62, 11.55, 6.85), "iron")

    # 沿顶磨光防磕扣边唇（前后通长，侧条内缩）
    g.box(b, "platter_lip_f", (-7.55, 11.35, -7.68), (7.55, 11.68, -6.75), "iron_lt")
    g.box(b, "platter_lip_b", (-7.55, 11.35, 6.75), (7.55, 11.68, 7.68), "iron_lt")
    g.pair(b, "platter_lip_side", (6.75, 11.35, -6.75), (7.68, 11.68, 6.75), "iron_lt")

    # 承盘四边环形「排灰浅槽」（沿立沿内侧凹陷，Y∈[10.25, 10.55]，宽度 1.2px，各边端面完全错开无重叠）
    g.box(b, "ash_trough_f", (-5.62, 10.25, -6.85), (5.62, 10.55, -5.65), "iron_dk")
    g.box(b, "ash_trough_b", (-5.62, 10.25, 5.65), (5.62, 10.55, 6.85), "iron_dk")
    g.pair(b, "ash_trough_side", (5.65, 10.25, -5.62), (6.85, 10.55, 5.62), "iron_dk")

    # 排灰槽四角汇灰沉淀暗孔（材质 void，X∈[5.75, 6.65], Z∈[5.75, 6.65]，与槽体无重叠）
    g.pair(b, "ash_drain_hole_f", (5.75, 10.20, -6.65), (6.65, 10.55, -5.75), "void")
    g.pair(b, "ash_drain_hole_b", (5.75, 10.20, 5.75), (6.65, 10.55, 6.65), "void")


def part_dead_array_rings(g: Gear) -> None:
    """死寂同心阵环：台面中央灵气失效的同心阵环凹线与导灵十字槽（末法核心主题，深黑死寂，对称）。"""
    b = "dead_array_rings"
    # 台面中央阵盘（凹于四周立沿内，Y∈[10.30, 10.52]，X∈[-5.6, 5.6], Z∈[-5.6, 5.6]）
    g.box(b, "array_stone_field", (-5.60, 10.30, -5.60), (5.60, 10.52, 5.60), "iron")

    # 1. 外同心阵环暗槽（R≈3.8..4.4，比周围更暗更灰的凹线，绝不发光！）
    g.box(b, "dead_ring_out_f", (-3.65, 10.38, -4.35), (3.65, 10.58, -3.85), "void")
    g.box(b, "dead_ring_out_b", (-3.65, 10.38, 3.85), (3.65, 10.58, 4.35), "void")
    g.pair(b, "dead_ring_out_side", (3.85, 10.38, -3.65), (4.35, 10.58, 3.65), "void")
    # 外环四斜角弧槽
    g.pair(b, "dead_ring_out_diag_f", (2.55, 10.36, -3.75), (3.75, 10.56, -2.55), "iron_dk")
    g.pair(b, "dead_ring_out_diag_b", (2.55, 10.36, 2.55), (3.75, 10.56, 3.75), "iron_dk")

    # 2. 内同心阵环暗槽（R≈1.8..2.4，深暗无光）
    g.box(b, "dead_ring_in_f", (-1.85, 10.38, -2.35), (1.85, 10.58, -1.85), "void")
    g.box(b, "dead_ring_in_b", (-1.85, 10.38, 1.85), (1.85, 10.58, 2.35), "void")
    g.pair(b, "dead_ring_in_side", (1.85, 10.38, -1.85), (2.35, 10.58, 1.85), "void")
    # 内环斜角转接
    g.pair(b, "dead_ring_in_diag_f", (1.15, 10.36, -1.95), (1.95, 10.56, -1.15), "iron_dk")
    g.pair(b, "dead_ring_in_diag_b", (1.15, 10.36, 1.15), (1.95, 10.56, 1.95), "iron_dk")

    # 3. 承灵中心暗孔（圆孔/方孔导灵枢，早已干涸失去响应）
    g.box(b, "dead_array_core_void", (-0.85, 10.25, -0.85), (0.85, 10.58, 0.85), "void")
    g.box(b, "dead_array_core_rim", (-1.05, 10.32, -1.05), (1.05, 10.55, 1.05), "iron_dk")

    # 4. 十字导灵暗槽线（分段式贯通，在环槽交汇处精确断开，消除共面）
    # 南北十字槽分段
    g.box(b, "dead_cross_n_in", (-0.22, 10.38, -1.85), (0.22, 10.58, -0.85), "void")
    g.box(b, "dead_cross_n_mid", (-0.22, 10.38, -3.85), (0.22, 10.58, -2.35), "void")
    g.box(b, "dead_cross_n_out", (-0.22, 10.38, -4.85), (0.22, 10.58, -4.35), "void")

    g.box(b, "dead_cross_s_in", (-0.22, 10.38, 0.85), (0.22, 10.58, 1.85), "void")
    g.box(b, "dead_cross_s_mid", (-0.22, 10.38, 2.35), (0.22, 10.58, 3.85), "void")
    g.box(b, "dead_cross_s_out", (-0.22, 10.38, 4.35), (0.22, 10.58, 4.85), "void")

    # 东西十字槽分段
    g.pair(b, "dead_cross_ew_in", (0.85, 10.38, -0.22), (1.85, 10.58, 0.22), "void")
    g.pair(b, "dead_cross_ew_mid", (2.35, 10.38, -0.22), (3.85, 10.58, 0.22), "void")
    g.pair(b, "dead_cross_ew_out", (4.35, 10.38, -0.22), (4.85, 10.58, 0.22), "void")


def part_corner_irons_base(g: Gear) -> None:
    """下座四角铁箍：下层底台四角铆接生铁包角与加固铆钉（成对严格对称）。"""
    b = "corner_irons_base"
    # 四角 L 形铁箍包角：主翼通到角顶，侧翼从主翼后缘接出，转折处无重叠
    # 1. 前左/前右包角（成对对称）
    g.pair(b, "iron_base_wing_f", (5.85, 2.15, -7.88), (7.88, 3.45, -7.55), "iron_lt")
    g.pair(b, "iron_base_wing_fs", (7.55, 2.15, -7.55), (7.88, 3.45, -5.85), "iron_lt")
    g.pair(b, "iron_base_stud_f1", (6.45, 2.45, -7.95), (6.95, 2.95, -7.75), "iron_lt")
    g.pair(b, "iron_base_stud_f2", (7.75, 2.45, -6.95), (7.95, 2.95, -6.45), "iron_lt")

    # 2. 后左/后右包角（成对对称）
    g.pair(b, "iron_base_wing_b", (5.85, 2.15, 7.55), (7.88, 3.45, 7.88), "iron_lt")
    g.pair(b, "iron_base_wing_bs", (7.55, 2.15, 5.85), (7.88, 3.45, 7.55), "iron_lt")
    g.pair(b, "iron_base_stud_b1", (6.45, 2.45, 7.75), (6.95, 2.95, 7.95), "iron_lt")
    g.pair(b, "iron_base_stud_b2", (7.75, 2.45, 6.45), (7.95, 2.95, 6.95), "iron_lt")


def part_corner_irons_top(g: Gear) -> None:
    """承盘四角铁箍：上层盘角铆接生铁包角与加固铆钉（成对严格对称）。"""
    b = "corner_irons_top"
    # 四角 L 形铁箍包在盘角 (±7.60, ±7.60)，主翼通角顶，侧翼后接无重叠
    # 1. 前方两角
    g.pair(b, "iron_top_wing_f", (6.05, 10.15, -7.78), (7.78, 11.75, -7.42), "iron_lt")
    g.pair(b, "iron_top_wing_fs", (7.42, 10.15, -7.42), (7.78, 11.75, -6.05), "iron_lt")
    g.pair(b, "iron_top_stud_f1", (6.55, 10.75, -7.85), (7.05, 11.25, -7.62), "iron_lt")
    g.pair(b, "iron_top_stud_f2", (7.62, 10.75, -7.05), (7.85, 11.25, -6.55), "iron_lt")

    # 2. 后方两角
    g.pair(b, "iron_top_wing_b", (6.05, 10.15, 7.42), (7.78, 11.75, 7.78), "iron_lt")
    g.pair(b, "iron_top_wing_bs", (7.42, 10.15, 6.05), (7.78, 11.75, 7.42), "iron_lt")
    g.pair(b, "iron_top_stud_b1", (6.55, 10.75, 7.62), (7.05, 11.25, 7.85), "iron_lt")
    g.pair(b, "iron_top_stud_b2", (7.62, 10.75, 6.55), (7.85, 11.25, 7.05), "iron_lt")


def part_ash_sediment(g: Gear) -> None:
    """角落残存冷灰：排灰槽死角薄薄积存的陈年炭灰（不对称挂件骨）。"""
    b = "ash_sediment"
    # 西北角积灰（X≈-5.8, Z≈-5.8）
    g.box(b, "ash_corner_crust1", (-6.45, 10.48, -6.45), (-5.55, 10.62, -5.55), "ash_dk")
    g.box(b, "ash_corner_flake1", (-6.25, 10.58, -6.25), (-5.75, 10.68, -5.75), "ash")
    g.box(b, "ash_trough_drift_n", (-4.85, 10.48, -6.45), (-3.25, 10.60, -5.85), "ash_dk")

    # 东南角零星灰粒与黑垢
    g.box(b, "ash_corner_crust2", (5.55, 10.48, 5.55), (6.25, 10.60, 6.25), "ash_dk")
    g.box(b, "ash_speck_e", (5.85, 10.50, 3.25), (6.35, 10.60, 4.45), "ash")
    g.box(b, "ash_array_drift", (1.25, 10.52, -3.95), (2.45, 10.62, -3.55), "ash_dk")


def part_wear_scars(g: Gear) -> None:
    """旧蚀氧化痕：底座边角火燎红斑与磨蚀铁茬（不对称挂件骨）。"""
    b = "wear_scars"
    # 东南台座角落火蚀红斑（贴在下座包角外侧，不深扎入底板内部）
    g.box(b, "scar_heat_blotch", (5.65, 3.46, 6.85), (7.25, 3.65, 7.65), "rust")
    g.box(b, "scar_deep_pit", (6.45, 3.46, 7.15), (7.05, 3.72, 7.55), "rust_dk")

    # 西南承盘角轻微磕损暗茬
    g.box(b, "scar_chip_sw", (-7.68, 10.75, 6.85), (-7.32, 11.25, 7.35), "void")
    g.box(b, "scar_chip_burr", (-7.62, 10.65, 6.75), (-7.25, 10.95, 7.15), "iron_dk")
    g.box(b, "scar_patina_drop", (-5.65, 3.55, -6.85), (-5.15, 3.95, -6.35), "bronze_pat")


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


class TieLuZuoSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "base_plinth": part_base_plinth,
    "beast_legs_f": part_beast_legs_f,
    "beast_legs_b": part_beast_legs_b,
    "waist_tier": part_waist_tier,
    "ruyi_relief": part_ruyi_relief,
    "upper_platter": part_upper_platter,
    "platter_rim": part_platter_rim,
    "dead_array_rings": part_dead_array_rings,
    "corner_irons_base": part_corner_irons_base,
    "corner_irons_top": part_corner_irons_top,
    "ash_sediment": part_ash_sediment,
    "wear_scars": part_wear_scars,
}

TEXTURE_OVERRIDES = {
    "iron": "metal_cast_iron",
    "iron_lt": "metal_cast_edge",
    "iron_dk": "metal_cast_dk",
    "ruyi": "metal_cast_relief",
    "rust": "metal_cast_rust",
    "rust_dk": "metal_cast_rust_dk",
    "bronze_pat": "metal_patina",
    "ash": "metal_cast_ash",
    "ash_dk": "metal_cast_ash_dk",
    "void": "metal_cast_soot",
}

SPEC = TieLuZuoSpec(
    gear_id="tie_lu_zuo",
    model_name="TieLuZuo",
    title="铁炉座 / tie_lu_zuo",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="地上摆件 · 高11.6px三层台座+收腰高4px缩进2.4px+高大饕餮兽足+如意云纹+外扩承盘死寂阵环",
    worn_focus=((8.0, 14.0, 0.0), 42.0),
    category="metalware",
    scale_mode="placed",
    player_offset=(16.0, 0.0, 0.0),
    texture_overrides=TEXTURE_OVERRIDES,
    textured=True,
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
