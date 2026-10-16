#!/usr/bin/env python3
"""半截袍摆（ban_jie_pao_bai）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土定制 NPC 专属装备 #5（zong_men_can_tu 宗门残徒）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双腿 x∈[0,4] / [-4,0] y∈[0,12] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

挂点与避让说明：
- 挂在腰腹围一圈，下垂至大腿中段（y∈[3.2, 13.6]）。
- 前开衩（x≈0.0 处开口微交叠），前片 z ≤ -2.12，后片 z ≥ 2.12，完全在腿盒前后，咬入 ≤ 0.05px。
- 重要避让：NPC 复用装备水囊（water_skin）位于左胯外侧（x∈[-7.9, -4.1]，y∈[3.2, 11.4]）；
  因此半截袍摆左侧（x ≤ -3.6）在 y∈[4, 12] 完全留空开口，绝不形成整圈封闭，严格避开水囊位置。
- 右侧（x∈[3.8, 4.35]）设侧系带与骨扣，且在 y ≤ 12 之下避开右臂盒。
- 外形特征：撕裂下摆（犬牙差互撕条）、烧灼黑洞露内衬、暗红粗布腰带束结、右前开衩、多褶皱阶梯。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/gear/ban_jie_pao_bai/gen_ban_jie_pao_bai.py --self-test
    python3 modelScript/gear/ban_jie_pao_bai/gen_ban_jie_pao_bai.py --gates
    python3 modelScript/gear/ban_jie_pao_bai/gen_ban_jie_pao_bai.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "cloth_teal":    (106, 122, 128),  # 旧宗门青灰残袍布
    "cloth_teal_lt": (138, 154, 158),  # 磨损褪色浅青灰
    "cloth_teal_dk": (72, 86, 92),     # 褶皱背阴暗青灰
    "cloth_lining":  (186, 178, 158),  # 泛黄内衬粗布
    "sash_red":      (134, 42, 38),    # 宗门暗红布腰带
    "sash_red_dk":   (92, 28, 26),     # 暗红扎结与带边
    "cord":          (146, 126, 82),   # 细麻绳 / 侧系带
    "cord_dk":       (104, 88, 56),    # 粗麻结与阴影
    "bone":          (216, 206, 178),  # 骨栓 / 骨扣
    "bone_lt":       (236, 228, 204),  # 骨棱亮色
    "bone_dk":       (164, 150, 118),  # 骨槽暗色
    "char":          (36, 32, 30),     # 烧灼焦黑炭边
    "stitch":        (58, 44, 32),     # 粗麻补缝线
    "void":          (22, 18, 16),     # 烧穿破洞暗底
}

SOFT = ("cloth_teal", "cloth_teal_lt", "cloth_teal_dk", "cloth_lining",
        "sash_red", "sash_red_dk", "cord", "cord_dk", "stitch")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("waist_sash", "root", (0.0, 12.8, 0.0), "腰带布绳圈：前后对称暗红腰束带"),
    ("sash_knot", "waist_sash", (1.6, 12.8, -2.45), "右侧腰结：打结花与下垂绑带尾（可微动）"),
    ("skirt_front_r", "waist_sash", (2.0, 9.0, -2.45), "前右袍摆大片：多层折叠青灰布片、开衩右缘"),
    ("skirt_front_l", "waist_sash", (-1.8, 9.5, -2.45), "前左袍摆：前开衩左侧斜切下摆（左胯外侧避让留空）"),
    ("skirt_back_m", "waist_sash", (0.0, 8.5, 2.45), "后中袍摆主片：后背中长摆、内衬翻边"),
    ("skirt_back_r", "waist_sash", (2.4, 8.5, 2.45), "后右袍摆外片：右臀后侧下摆与侧缝折角"),
    ("skirt_back_l", "waist_sash", (-2.2, 9.0, 2.45), "后左袍摆短片：后左侧收拢下摆（避让水囊空间）"),
    ("skirt_pleats", "waist_sash", (0.0, 11.2, 0.0), "腰际捏褶：前胸下与后腰上的密集叠层打褶条"),
    ("tear_strips_f", "skirt_front_r", (1.5, 5.5, -2.45), "前缘撕条：前下摆犬牙差互的悬空撕裂布缕（可摆动）"),
    ("tear_strips_b", "skirt_back_m", (0.0, 4.2, 2.45), "后摆长撕条：后下摆参差垂坠的长布碎条（可摆动）"),
    ("burn_patch", "skirt_front_r", (2.2, 7.8, -2.45), "火灼残洞：前右摆被雷火烧黑的穿透洞、焦炭边与内衬粗针脚"),
    ("side_ties", "waist_sash", (3.8, 10.5, 0.0), "右胯侧系绳：连接前后袍摆的粗麻扎绳与兽骨扣（挂件独立骨）"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "waist_sash"))


# ───────────────────────────────────────────────────────────── 分部件实现
def part_waist_sash(g: Gear) -> None:
    """腰带布绳圈：前后对称暗红腰束带，带边滚线与针脚。"""
    b = "waist_sash"
    # 前腰横束带（厚度 ≥ 0.28）
    g.box(b, "sash_f_base", (-3.95, 12.18, -2.48), (3.95, 13.42, -2.18), "sash_red")
    g.box(b, "sash_f_rim_t", (-3.95, 13.42, -2.50), (3.95, 13.68, -2.16), "sash_red_dk")
    g.box(b, "sash_f_rim_b", (-3.95, 11.92, -2.50), (3.95, 12.18, -2.16), "sash_red_dk")

    # 后腰横束带（对称）
    g.box(b, "sash_b_base", (-3.95, 12.18, 2.18), (3.95, 13.42, 2.48), "sash_red")
    g.box(b, "sash_b_rim_t", (-3.95, 13.42, 2.16), (3.95, 13.68, 2.50), "sash_red_dk")
    g.box(b, "sash_b_rim_b", (-3.95, 11.92, 2.16), (3.95, 12.18, 2.50), "sash_red_dk")

    # 前腰与后腰对称双行固定粗麻针脚
    for k in range(4):
        x = 0.55 + k * 0.95
        g.pair(b, f"sash_f_st_t_{k}", (x, 13.25, -2.62), (x + 0.38, 13.55, -2.32), "stitch")
        g.pair(b, f"sash_f_st_b_{k}", (x, 12.05, -2.62), (x + 0.38, 12.35, -2.32), "stitch")
        g.pair(b, f"sash_b_st_t_{k}", (x, 13.25, 2.32), (x + 0.42, 13.55, 2.62), "stitch")
        g.pair(b, f"sash_b_st_b_{k}", (x, 12.05, 2.32), (x + 0.42, 12.35, 2.62), "stitch")

    # 后腰正中大兽骨卡扣（三轴尺寸均 ≥ 0.62，用于穿模门禁自证）
    g.box(b, "sash_back_clasp", (-0.85, 12.02, 2.38), (0.85, 13.58, 3.02), "bone")
    g.box(b, "sash_back_clasp_rim", (-0.95, 12.12, 2.42), (0.95, 13.48, 2.95), "bone_lt")
    g.box(b, "sash_back_pin", (-0.22, 12.28, 2.65), (0.22, 13.32, 3.12), "bone_dk")


def part_sash_knot(g: Gear) -> None:
    """右前腰带系结：斜系的大布结与垂坠双股破损带尾。"""
    b = "sash_knot"
    # 右前系结点（X≈1.6，Y∈[12.1, 13.5]）
    g.box(b, "knot_body_core", (1.15, 12.15, -2.85), (2.35, 13.45, -2.35), "sash_red_dk")
    g.box(b, "knot_wrap_a", (1.05, 12.35, -2.95), (2.15, 13.15, -2.48), "sash_red")
    g.box(b, "knot_wrap_b", (1.35, 12.25, -3.02), (2.45, 13.25, -2.55), "sash_red_dk")
    g.box(b, "knot_center_loop", (1.55, 12.55, -3.12), (1.95, 12.98, -2.85), "sash_red")

    # 垂落带舌 1（左短带）
    g.box(b, "knot_tail1_a", (1.05, 11.45, -2.86), (1.55, 12.25, -2.52), "sash_red")
    g.box(b, "knot_tail1_b", (0.95, 10.65, -2.80), (1.45, 11.55, -2.50), "sash_red_dk")
    g.box(b, "knot_tail1_c", (1.02, 10.05, -2.76), (1.38, 10.75, -2.52), "sash_red")

    # 垂落带舌 2（右长带，带开线毛边）
    g.box(b, "knot_tail2_a", (1.85, 11.35, -2.88), (2.35, 12.25, -2.52), "sash_red_dk")
    g.box(b, "knot_tail2_b", (1.95, 10.45, -2.82), (2.42, 11.45, -2.52), "sash_red")
    g.box(b, "knot_tail2_c", (2.05, 9.55, -2.78), (2.45, 10.55, -2.54), "sash_red_dk")
    g.box(b, "knot_tail2_fray", (2.12, 9.15, -2.74), (2.40, 9.65, -2.55), "sash_red")


def part_skirt_front_r(g: Gear) -> None:
    """前右袍摆大片：从腰部垂下的大幅右前摆，层叠青灰破布与开衩斜边。"""
    b = "skirt_front_r"
    # 主袍身（从 X=0.15 延伸到 X=3.85，高度 Y∈[6.25, 12.25]，Z∈[-2.58, -2.25]）
    # 第一层：上主片
    g.box(b, "front_r_p1", (0.15, 10.25, -2.54), (3.82, 12.25, -2.25), "cloth_teal")
    # 第二层：中段错层（向外微出）
    g.box(b, "front_r_p2", (0.25, 8.25, -2.58), (3.85, 10.35, -2.28), "cloth_teal")
    # 第三层：下段收束片
    g.box(b, "front_r_p3", (0.35, 6.25, -2.56), (3.75, 8.35, -2.26), "cloth_teal_dk")

    # 开衩内侧翻边与外缘滚折边（读出旧宗门长袍的镶边工艺）
    g.box(b, "front_r_slit_rim", (0.10, 6.25, -2.62), (0.42, 12.25, -2.22), "cloth_teal_lt")
    g.box(b, "front_r_bottom_rim", (0.35, 6.15, -2.60), (3.75, 6.45, -2.24), "cloth_teal_lt")

    # 表面竖向走线褶皱条（起棱暗褶与磨白边）
    g.box(b, "front_r_fold1", (0.85, 7.15, -2.65), (1.18, 11.85, -2.48), "cloth_teal_dk")
    g.box(b, "front_r_fold2", (1.85, 7.55, -2.66), (2.18, 11.65, -2.48), "cloth_teal_lt")
    g.box(b, "front_r_fold3", (2.85, 6.85, -2.65), (3.18, 11.25, -2.48), "cloth_teal_dk")
    g.box(b, "front_r_fold4", (1.35, 6.45, -2.64), (1.68, 10.85, -2.48), "cloth_teal_lt")
    g.box(b, "front_r_fold5", (2.35, 6.35, -2.64), (2.68, 10.55, -2.48), "cloth_teal_dk")
    g.box(b, "front_r_fold6", (3.35, 7.15, -2.64), (3.65, 11.55, -2.48), "cloth_teal_lt")

    # 翻出的浅黄内衬与补丁粗线
    g.box(b, "front_r_lining1", (0.45, 6.05, -2.48), (1.45, 6.35, -2.20), "cloth_lining")
    g.box(b, "front_r_lining2", (2.65, 6.05, -2.48), (3.65, 6.35, -2.20), "cloth_lining")
    g.box(b, "front_r_patch1", (0.95, 9.15, -2.62), (1.75, 9.85, -2.35), "cloth_lining")
    for k in range(3):
        x = 1.05 + k * 0.28
        g.box(b, f"front_r_patch_st_{k}", (x, 9.68, -2.68), (x + 0.22, 9.95, -2.32), "stitch")


def part_skirt_front_l(g: Gear) -> None:
    """前左袍摆：前开衩左侧斜切下摆（左胯外侧严格留空避让水囊）。"""
    b = "skirt_front_l"
    # 左侧袍身从 X=-0.15 向左延伸到 X=-3.50，严格不到 -3.8，左胯留空！
    # 高度斜向抬高（从 X=-0.15 处 y=6.8 到 X=-3.40 处 y=9.2，斜切上去）
    g.box(b, "front_l_p1", (-3.50, 10.45, -2.54), (-0.15, 12.25, -2.25), "cloth_teal")
    g.box(b, "front_l_p2", (-3.25, 8.85, -2.56), (-0.18, 10.55, -2.28), "cloth_teal")
    g.box(b, "front_l_p3", (-2.25, 7.25, -2.54), (-0.25, 8.95, -2.26), "cloth_teal_dk")

    # 开衩左边缘滚边
    g.box(b, "front_l_slit_rim", (-0.45, 7.15, -2.62), (-0.12, 12.25, -2.22), "cloth_teal_lt")
    # 斜切下摆滚边
    g.box(b, "front_l_bot_rim", (-3.35, 8.75, -2.60), (-2.15, 9.05, -2.24), "cloth_teal_lt")

    # 竖向暗褶与开衩细节
    g.box(b, "front_l_fold1", (-1.15, 7.85, -2.64), (-0.82, 11.65, -2.46), "cloth_teal_dk")
    g.box(b, "front_l_fold2", (-2.15, 8.95, -2.64), (-1.82, 11.85, -2.46), "cloth_teal_lt")
    g.box(b, "front_l_fold3", (-2.85, 9.55, -2.64), (-2.52, 11.75, -2.46), "cloth_teal_dk")
    g.box(b, "front_l_slit_notch", (-0.52, 9.25, -2.66), (-0.18, 9.65, -2.38), "cloth_teal_dk")

    # 边缘磨破露出内衬布与斜补丁
    g.box(b, "front_l_lining", (-2.65, 8.65, -2.48), (-1.65, 8.95, -2.20), "cloth_lining")
    g.box(b, "front_l_patch", (-1.95, 9.85, -2.62), (-1.25, 10.55, -2.35), "cloth_lining")
    for k in range(2):
        y = 10.05 + k * 0.32
        g.box(b, f"front_l_patch_st_{k}", (-1.88, y, -2.68), (-1.35, y + 0.25, -2.32), "stitch")


def part_skirt_back_m(g: Gear) -> None:
    """后中袍摆主片：后背中长下摆（下垂至 y≈4.2，体量饱满宽阔）。"""
    b = "skirt_back_m"
    # 后中主板（跨度 X∈[-2.2, 2.2]，高度 Y∈[4.25, 12.25]，Z∈[2.25, 2.58]）
    g.box(b, "back_m_p1", (-2.20, 9.85, 2.25), (2.20, 12.25, 2.54), "cloth_teal")
    g.box(b, "back_m_p2", (-2.10, 7.25, 2.28), (2.10, 9.95, 2.58), "cloth_teal")
    g.box(b, "back_m_p3", (-1.95, 4.85, 2.26), (1.95, 7.35, 2.56), "cloth_teal_dk")

    # 后摆中缝暗线（旧道袍背中缝）
    g.box(b, "back_m_seam", (-0.18, 4.75, 2.48), (0.18, 12.25, 2.65), "cloth_teal_dk")
    # 左右两道大外凸脊褶与副褶
    g.box(b, "back_m_ridge_l", (-1.25, 5.65, 2.48), (-0.88, 11.55, 2.66), "cloth_teal_lt")
    g.box(b, "back_m_ridge_r", (0.88, 5.65, 2.48), (1.25, 11.55, 2.66), "cloth_teal_lt")
    g.box(b, "back_m_subridge_l", (-1.85, 6.25, 2.46), (-1.52, 10.85, 2.64), "cloth_teal_dk")
    g.box(b, "back_m_subridge_r", (1.52, 6.25, 2.46), (1.85, 10.85, 2.64), "cloth_teal_dk")
    g.box(b, "back_m_patch1", (-0.95, 8.45, 2.48), (-0.25, 9.15, 2.70), "cloth_lining")
    g.box(b, "back_m_patch1_st", (-0.88, 8.65, 2.55), (-0.32, 8.95, 2.78), "stitch")

    # 后摆底边缘整齐翻出的泛黄内衬
    g.box(b, "back_m_lining", (-1.85, 4.65, 2.20), (1.85, 4.95, 2.48), "cloth_lining")
    # 衬布粗针脚行
    for k in range(5):
        x = -1.45 + k * 0.72
        g.box(b, f"back_m_st_{k}", (x, 4.85, 2.48), (x + 0.35, 5.15, 2.68), "stitch")


def part_skirt_back_r(g: Gear) -> None:
    """后右袍摆外片：右侧臀后延伸的大幅摆面。"""
    b = "skirt_back_r"
    # 跨度 X∈[1.95, 3.85]，高度 Y∈[5.45, 12.25]
    g.box(b, "back_r_p1", (1.95, 9.65, 2.25), (3.85, 12.25, 2.54), "cloth_teal")
    g.box(b, "back_r_p2", (1.85, 7.25, 2.28), (3.82, 9.75, 2.58), "cloth_teal")
    g.box(b, "back_r_p3", (1.80, 5.45, 2.26), (3.65, 7.35, 2.56), "cloth_teal_dk")

    # 右外侧翻折边与侧褶
    g.box(b, "back_r_outer_fold", (3.55, 5.35, 2.22), (3.88, 12.25, 2.62), "cloth_teal_lt")
    g.box(b, "back_r_diag_fold", (2.65, 6.25, 2.48), (3.05, 11.45, 2.66), "cloth_teal_dk")
    g.box(b, "back_r_lining", (2.15, 5.25, 2.20), (3.55, 5.55, 2.48), "cloth_lining")


def part_skirt_back_l(g: Gear) -> None:
    """后左袍摆短片：后左侧收拢下摆（左胯留出充分空间，避让水囊）。"""
    b = "skirt_back_l"
    # X∈[-3.50, -1.95]，严格不超过 -3.60，留出水囊区！
    # 下沿到 y≈7.5 结束，比右侧短一大截
    g.box(b, "back_l_p1", (-3.50, 10.25, 2.25), (-1.95, 12.25, 2.54), "cloth_teal")
    g.box(b, "back_l_p2", (-3.35, 8.45, 2.28), (-1.85, 10.35, 2.58), "cloth_teal")
    g.box(b, "back_l_p3", (-2.95, 7.25, 2.26), (-1.80, 8.55, 2.56), "cloth_teal_dk")

    # 左边缘开线粗糙撕茬
    g.box(b, "back_l_tear_edge", (-3.55, 7.15, 2.22), (-3.22, 12.25, 2.62), "cloth_teal_lt")
    g.box(b, "back_l_lining", (-3.15, 7.05, 2.20), (-1.95, 7.35, 2.48), "cloth_lining")


def part_skirt_pleats(g: Gear) -> None:
    """腰际捏褶：前腹下与后腰上的密集叠层打褶条。"""
    b = "skirt_pleats"
    # 前腰沿线打褶（Y∈[11.2, 12.3]）
    for k in range(5):
        x = -3.2 + k * 1.45
        g.box(b, f"pleat_f_{k}", (x, 11.15, -2.66), (x + 0.55, 12.35, -2.42), "cloth_teal_dk")
        g.box(b, f"pleat_f_rib_{k}", (x + 0.12, 11.35, -2.72), (x + 0.42, 12.25, -2.52), "cloth_teal_lt")

    # 后腰沿线打褶（Y∈[11.2, 12.3]）
    for k in range(5):
        x = -3.0 + k * 1.45
        g.box(b, f"pleat_b_{k}", (x, 11.15, 2.42), (x + 0.55, 12.35, 2.66), "cloth_teal_dk")
        g.box(b, f"pleat_b_rib_{k}", (x + 0.12, 11.35, 2.52), (x + 0.42, 12.25, 2.72), "cloth_teal_lt")


def part_tear_strips_f(g: Gear) -> None:
    """前缘撕条：前下摆犬牙差互的悬空撕裂布缕（可摆动骨）。"""
    b = "tear_strips_f"
    # 下垂撕条（Y∈[3.2, 6.4]，X 分布于前右摆下缘）
    strips = (
        ("strip_f1", 0.45, 0.85, 4.45, 6.35, -2.58, -2.32, "cloth_teal"),
        ("strip_f1_tip", 0.52, 0.78, 3.85, 4.55, -2.56, -2.34, "cloth_teal_dk"),
        ("strip_f2", 1.05, 1.48, 4.05, 6.25, -2.58, -2.32, "cloth_teal_dk"),
        ("strip_f2_tip", 1.12, 1.42, 3.45, 4.15, -2.56, -2.34, "cloth_teal_lt"),
        ("strip_f3", 1.65, 2.12, 4.85, 6.35, -2.58, -2.32, "cloth_teal"),
        ("strip_f3_tip", 1.75, 2.02, 4.15, 4.95, -2.56, -2.34, "cloth_teal_dk"),
        ("strip_f4", 2.28, 2.75, 3.95, 6.25, -2.58, -2.32, "cloth_teal_lt"),
        ("strip_f4_tip", 2.38, 2.65, 3.25, 4.05, -2.56, -2.34, "cloth_teal_dk"),
        ("strip_f5", 2.95, 3.42, 4.55, 6.25, -2.58, -2.32, "cloth_teal"),
        ("strip_f5_tip", 3.05, 3.32, 3.95, 4.65, -2.56, -2.34, "cloth_teal_lt"),
        ("strip_fl_1", -1.85, -1.45, 5.85, 7.35, -2.58, -2.32, "cloth_teal_dk"),
        ("strip_fl_1_tip", -1.78, -1.52, 5.25, 5.95, -2.56, -2.34, "cloth_teal"),
        ("strip_fl_2", -1.25, -0.85, 5.65, 7.35, -2.58, -2.32, "cloth_teal"),
        ("strip_fl_2_tip", -1.18, -0.92, 4.95, 5.75, -2.56, -2.34, "cloth_teal_lt"),
        ("strip_fl_3", -0.75, -0.35, 5.25, 6.95, -2.58, -2.32, "cloth_teal_dk"),
        ("strip_fl_3_tip", -0.68, -0.42, 4.55, 5.35, -2.56, -2.34, "cloth_teal_lt"),
        ("strip_f_fray1", 0.85, 1.15, 3.25, 4.25, -2.54, -2.36, "cloth_teal_dk"),
        ("strip_f_fray2", 2.05, 2.35, 3.15, 4.15, -2.54, -2.36, "cloth_teal_lt"),
        ("strip_f_fray3", 0.25, 0.52, 3.85, 4.65, -2.54, -2.36, "cloth_teal_dk"),
        ("strip_f_fray4", 1.45, 1.72, 3.55, 4.45, -2.54, -2.36, "cloth_teal_lt"),
        ("strip_f_fray5", 2.72, 2.98, 3.35, 4.25, -2.54, -2.36, "cloth_teal"),
    )
    for name, x0, x1, y0, y1, z0, z1, mat in strips:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), mat)


def part_tear_strips_b(g: Gear) -> None:
    """后摆长撕条：后下摆参差垂坠的长布碎条（可摆动骨）。"""
    b = "tear_strips_b"
    # 后摆撕条下垂至 y≈2.5–3.8
    strips = (
        ("strip_b1", -1.85, -1.35, 3.25, 4.95, 2.32, 2.58, "cloth_teal"),
        ("strip_b1_tip", -1.75, -1.45, 2.65, 3.35, 2.34, 2.56, "cloth_teal_dk"),
        ("strip_b2", -1.15, -0.65, 2.85, 4.85, 2.32, 2.58, "cloth_teal_dk"),
        ("strip_b2_tip", -1.05, -0.75, 2.15, 2.95, 2.34, 2.56, "cloth_teal_lt"),
        ("strip_b3", -0.45, 0.15, 3.45, 4.95, 2.32, 2.58, "cloth_teal"),
        ("strip_b3_tip", -0.35, 0.05, 2.75, 3.55, 2.34, 2.56, "cloth_teal_dk"),
        ("strip_b4", 0.35, 0.85, 2.95, 4.85, 2.32, 2.58, "cloth_teal_lt"),
        ("strip_b4_tip", 0.45, 0.75, 2.25, 3.05, 2.34, 2.56, "cloth_teal_dk"),
        ("strip_b5", 1.05, 1.55, 3.35, 4.95, 2.32, 2.58, "cloth_teal"),
        ("strip_b5_tip", 1.15, 1.45, 2.65, 3.45, 2.34, 2.56, "cloth_teal_lt"),
        ("strip_b6", 1.75, 2.25, 3.65, 5.55, 2.32, 2.58, "cloth_teal_dk"),
        ("strip_b6_tip", 1.85, 2.15, 2.95, 3.75, 2.34, 2.56, "cloth_teal"),
        ("strip_b7", 2.45, 2.95, 3.85, 5.55, 2.32, 2.58, "cloth_teal"),
        ("strip_b7_tip", 2.55, 2.85, 3.15, 3.95, 2.34, 2.56, "cloth_teal_lt"),
        ("strip_b8", 3.15, 3.65, 4.25, 5.55, 2.32, 2.58, "cloth_teal_dk"),
        ("strip_b8_tip", 3.25, 3.55, 3.55, 4.35, 2.34, 2.56, "cloth_teal"),
        ("strip_b_fray1", -0.85, -0.55, 1.85, 2.85, 2.35, 2.55, "cloth_teal"),
        ("strip_b_fray2", 0.65, 0.95, 1.75, 2.75, 2.35, 2.55, "cloth_teal_dk"),
        ("strip_b_fray3", 2.15, 2.45, 2.25, 3.25, 2.35, 2.55, "cloth_teal_lt"),
    )
    for name, x0, x1, y0, y1, z0, z1, mat in strips:
        g.box(b, name, (x0, y0, z0), (x1, y1, z1), mat)


def part_burn_patch(g: Gear) -> None:
    """火灼残洞：前右摆被雷火烧黑的穿透洞、焦炭边与内衬粗针脚。"""
    b = "burn_patch"
    # 中心位于 X=2.30, Y=7.80
    # 穿透黑洞（void 材质，厚度 ≥ 0.28）
    g.box(b, "burn_void_core", (1.95, 7.45, -2.75), (2.65, 8.15, -2.42), "void")
    g.box(b, "burn_void_lip", (2.02, 7.25, -2.72), (2.48, 7.55, -2.44), "void")

    # 焦黑炭化边缘层（围绕破洞的焦边圈）
    g.box(b, "char_ring_t", (1.75, 8.12, -2.68), (2.85, 8.42, -2.38), "char")
    g.box(b, "char_ring_b", (1.82, 7.05, -2.68), (2.75, 7.35, -2.38), "char")
    g.box(b, "char_ring_l", (1.72, 7.32, -2.68), (2.02, 8.18, -2.38), "char")
    g.box(b, "char_ring_r", (2.58, 7.32, -2.68), (2.88, 8.18, -2.38), "char")

    # 洞内衬托出的泛黄内衬补片
    g.box(b, "lining_patch_back", (1.65, 6.95, -2.42), (2.95, 8.55, -2.20), "cloth_lining")
    g.box(b, "char_spurt1", (1.55, 7.85, -2.72), (1.85, 8.25, -2.36), "char")
    g.box(b, "char_spurt2", (2.75, 7.15, -2.72), (3.05, 7.55, -2.36), "char")
    g.box(b, "char_soot_smudge", (1.92, 8.35, -2.64), (2.55, 8.65, -2.36), "cloth_teal_dk")

    # 补丁四周粗麻粗针脚（用粗线补缝残破洞口）
    for k in range(3):
        x = 1.75 + k * 0.45
        g.box(b, f"burn_stitch_t_{k}", (x, 8.40, -2.66), (x + 0.32, 8.68, -2.32), "stitch")
        g.box(b, f"burn_stitch_b_{k}", (x, 6.82, -2.66), (x + 0.32, 7.10, -2.32), "stitch")
    for k in range(2):
        y = 7.42 + k * 0.55
        g.box(b, f"burn_stitch_l_{k}", (1.52, y, -2.66), (1.78, y + 0.38, -2.32), "stitch")
        g.box(b, f"burn_stitch_r_{k}", (2.82, y, -2.66), (3.08, y + 0.38, -2.32), "stitch")


def part_side_ties(g: Gear) -> None:
    """右胯侧系绳：连接前后袍摆的粗麻扎绳与兽骨扣（挂件独立骨）。"""
    b = "side_ties"
    # 右侧跨接前后摆的绳圈与结（X ≥ 4.08，Y∈[9.2, 11.5]，严格在右腿与手臂盒之外）
    g.box(b, "tie_cord_wrap1", (4.08, 10.85, -1.85), (4.38, 11.15, 1.85), "cord")
    g.box(b, "tie_cord_wrap2", (4.08, 9.85, -1.75), (4.38, 10.15, 1.75), "cord_dk")

    # 侧边绳结与兽骨插扣（三轴尺寸均 ≥ 0.58，确保穿模门禁自证有鉴别力）
    g.box(b, "tie_knot_high", (4.18, 10.75, -0.45), (4.52, 11.25, 0.45), "cord_dk")
    g.box(b, "tie_bone_toggle1", (4.15, 10.45, -0.32), (4.75, 11.55, 0.32), "bone")
    g.box(b, "tie_bone_crest1", (4.12, 10.72, -0.35), (4.78, 11.38, 0.35), "bone_lt")

    g.box(b, "tie_knot_low", (4.18, 9.75, -0.45), (4.52, 10.25, 0.45), "cord_dk")
    g.box(b, "tie_bone_toggle2", (4.25, 9.45, -0.22), (4.55, 10.55, 0.22), "bone_dk")

    # 下垂细绳头与坠饰
    g.box(b, "tie_dangle_cord1", (4.22, 8.85, -0.25), (4.48, 9.78, 0.05), "cord")
    g.box(b, "tie_dangle_cord2", (4.22, 8.45, -0.05), (4.48, 9.55, 0.25), "cord_dk")
    g.box(b, "tie_dangle_bead", (4.18, 8.25, -0.18), (4.52, 8.65, 0.18), "bone_lt")
    g.box(b, "tie_dangle_fray1", (4.25, 7.85, -0.22), (4.45, 8.35, -0.02), "cord")
    g.box(b, "tie_dangle_fray2", (4.25, 7.75, 0.02), (4.45, 8.25, 0.22), "cord_dk")
    g.box(b, "tie_bead_slit", (4.15, 8.38, -0.22), (4.55, 8.52, 0.22), "bone_dk")
    g.box(b, "tie_cross_loop", (4.12, 10.15, -0.32), (4.45, 10.45, 0.32), "cord_dk")


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


class BanJiePaoBaiSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "waist_sash": part_waist_sash,
    "sash_knot": part_sash_knot,
    "skirt_front_r": part_skirt_front_r,
    "skirt_front_l": part_skirt_front_l,
    "skirt_back_m": part_skirt_back_m,
    "skirt_back_r": part_skirt_back_r,
    "skirt_back_l": part_skirt_back_l,
    "skirt_pleats": part_skirt_pleats,
    "tear_strips_f": part_tear_strips_f,
    "tear_strips_b": part_tear_strips_b,
    "burn_patch": part_burn_patch,
    "side_ties": part_side_ties,
}

SPEC = BanJiePaoBaiSpec(
    gear_id="ban_jie_pao_bai",
    model_name="BanJiePaoBai",
    title="半截袍摆 / ban_jie_pao_bai",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：腰腹/大腿 · 前开衩撕裂袍摆+后长撕条+火灼残洞+避让左胯水囊",
    worn_focus=((0.0, 8.5, 0.0), 20.0),
    tex_views=(
        ("FRONT 正面（-z）", 180.0, 0.0),
        ("3/4 正右斜", 152.0, 18.0),
        ("BACK 背面（+z）", 0.0, 0.0),
    ),
    texture_overrides={"cloth_lining": "gear_linen_lt"},
    textured=True,
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
