#!/usr/bin/env python3
"""铁火盆（tie_huo_pen）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #9（地上摆件）。
世界观基调：
- 精致规整的古代铸铁器具：铸铁圆形浅盆、圆鼓腹、外翻宽平沿、沿上回纹（雷纹）凸起、两侧兽面衔环、三只外撇兽足。
- 绝非末土捡拾缝补之物，而是成熟文明造物；末法体现在灵气与燃料匮乏：盆里只剩一层冷灰与两三块残炭、奄奄一息的高对比红热余烬。
- 尺寸：口径约 14.6 px、高约 8 px，三矮足，落地摆放（y=0）。
- 造型要点（调度与用户返工意见）：
  * 杜绝方形梯台：使用逐层方块圆环近似圆（行合并算法，参考骨笼蟹颅骨壳做法）；
  * 圆鼓腹（下敛上敞、中部外凸弧度饱满，外带横向弦纹圈）；
  * 外翻宽平沿，平沿表面环绕规整回纹凸条；
  * 两侧兽面铺首衔环（八角中空下垂圆环）；
  * 三只三向外撇兽爪足（120° 排布，爪趾着地 y=0）；
  * 盆底圆形冷灰薄层与两三块高对比余烬残炭。
- 方块数 ≥ 200，分骨 ≥ 10，挂件独立骨。

用法：
    python3 modelScript/metalware/tie_huo_pen/gen_tie_huo_pen.py --self-test
    python3 modelScript/metalware/tie_huo_pen/gen_tie_huo_pen.py --gates
    python3 modelScript/metalware/tie_huo_pen/gen_tie_huo_pen.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "gear" / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "iron":         (76, 72, 70),      # 铸铁黑褐底色（温润包浆氧化皮）
    "iron_lt":      (118, 114, 112),   # 凸起回纹棱角与磨光高光
    "iron_dk":      (50, 46, 46),      # 铸铁深阴影与凹槽底
    "rust":         (146, 78, 48),     # 盆底长期受热泛出的红褐斑
    "rust_dk":      (102, 52, 34),     # 浓重铁锈蚀斑
    "bronze_pat":   (92, 116, 98),     # 局部青绿铜锈渍（与铁器共存的金属氧化）
    "ash":          (168, 166, 160),   # 冷炭灰浅灰色
    "ash_dk":       (114, 110, 106),   # 炭灰深灰层与阴影
    "charcoal":     (34, 32, 30),      # 焦黑残炭
    "ember":        (255, 78, 24),     # 高对比炽热余烬核心（鲜明红橙）
    "ember_glow":   (255, 178, 38),    # 高对比微弱金黄火星
    "void":         (20, 18, 16),      # 盆底暗隙与排灰孔暗底
}

SOFT = ("ash", "ash_dk", "charcoal", "ember", "ember_glow")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("basin_bottom", "root", (0.0, 2.4, 0.0), "盆底基板：圆形平底、底心凹印与火蚀斑块（对称）"),
    ("basin_belly", "basin_bottom", (0.0, 4.8, 0.0), "圆鼓盆腹：逐层圆环构筑圆腹、下敛中鼓与加固弦纹（对称）"),
    ("basin_rim", "basin_belly", (0.0, 7.4, 0.0), "外折宽平沿：外撇大宽沿、回纹饰带与沿口扣唇（对称）"),
    ("rim_notches", "basin_rim", (-4.5, 7.8, 2.5), "沿口磕缺残损：旧时代碰撞残缺的小崩口与暗铁茬（不对称）"),
    ("beast_leg_b", "basin_bottom", (0.0, 1.4, 4.6), "后位外撇兽足：兽面膝盖、起脊曲腿与三趾踏地爪（对称自身）"),
    ("beast_leg_fl", "basin_bottom", (-3.8, 1.4, -2.4), "前左外撇兽足：斜向外撇的雄劲兽爪足（着地 y=0）"),
    ("beast_leg_fr", "basin_bottom", (3.8, 1.4, -2.4), "前右外撇兽足：斜向外撇的雄劲兽爪足（着地 y=0）"),
    ("handle_boss_l", "basin_belly", (-6.4, 5.6, 0.0), "左侧兽面铺首：威严饕餮面浮雕与衔环挂鼻"),
    ("handle_boss_r", "basin_belly", (6.4, 5.6, 0.0), "右侧兽面铺首：威严饕餮面浮雕与衔环挂鼻"),
    ("handle_ring_l", "handle_boss_l", (-7.4, 4.2, 0.0), "左侧锻铁衔环：自然下垂的八角铁圆环（独立可动挂件骨）"),
    ("handle_ring_r", "handle_boss_r", (7.4, 4.2, 0.0), "右侧锻铁衔环：自然下垂的八角铁圆环（独立可动挂件骨）"),
    ("ash_bed", "basin_bottom", (0.0, 3.4, 0.0), "盆底冷灰层：薄覆盆底的沉寂灰白炭灰圆形薄层"),
    ("ember_coals", "ash_bed", (0.5, 3.8, -0.2), "残炭与高对比余烬：两块焦黑断炭与明亮红热余烬（核心点睛件）"),
]

ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in (
    "root", "basin_bottom", "basin_belly", "basin_rim", "beast_leg_b"
))


# ───────────────────────────────────────────────────────────── 几何与圆环生成辅助
CELL = 0.5  # 体素栅格步长（0.5px，保证每个方块各向尺寸 >= 0.5px，杜绝退化薄片）


def circle_boxes_symmetric(r_in: float, r_out: float, y0: float, y1: float, cell: float = CELL, offset_x: float = 0.0, offset_z: float = 0.0):
    """生成逐层方块圆环（行合并算法，参考骨笼蟹颅骨壳做法）。
    在 [y0, y1] 高度层内，根据内径 r_in 与外径 r_out，在体素栅格内筛选属于圆环的格子，
    并将同一 z 行上连续的 x 格子合并为单个 box。
    同时区分中线件（跨越 x=0，强制 xmin = -xmax）与镜像件对（纯右侧段，用 pair 配对）。
    """
    max_idx = int(r_out / cell) + 2
    r_in_sq = (max(0.0, r_in - 0.05)) ** 2
    r_out_sq = (r_out + 0.05) ** 2

    rows: dict[int, list[int]] = {}
    for iz in range(-max_idx, max_idx + 1):
        z = iz * cell
        for ix in range(-max_idx, max_idx + 1):
            x = ix * cell
            r_sq = x * x + z * z
            if r_in_sq <= r_sq <= r_out_sq:
                rows.setdefault(iz, []).append(ix)

    mid_boxes = []
    pair_boxes = []

    for iz, ixs in sorted(rows.items()):
        ixs.sort()
        runs = []
        start = prev = ixs[0]
        for ix in ixs[1:] + [None]:
            if ix is not None and ix == prev + 1:
                prev = ix
                continue
            runs.append((start, prev))
            if ix is not None:
                start = prev = ix

        zmin = round((iz - 0.5) * cell + offset_z, 4)
        zmax = round((iz + 0.5) * cell + offset_z, 4)

        for s, p in runs:
            if s <= 0 and p >= 0:
                assert s == -p, f"Middle run not symmetric: {s}..{p}"
                xmax = round((p + 0.5) * cell + offset_x, 4)
                xmin = -xmax
                mid_boxes.append((xmin, round(y0, 4), zmin, xmax, round(y1, 4), zmax))
            elif s > 0:
                xmin = round((s - 0.5) * cell + offset_x, 4)
                xmax = round((p + 0.5) * cell + offset_x, 4)
                pair_boxes.append((xmin, round(y0, 4), zmin, xmax, round(y1, 4), zmax))

    return mid_boxes, pair_boxes


def add_symmetric_ring(g: Gear, b: str, prefix: str, r_in: float, r_out: float, y0: float, y1: float, mat: str, offset_x: float = 0.0, offset_z: float = 0.0) -> None:
    """向骨骼 b 中追加一层严格镜像对称的圆环方块。"""
    mids, pairs = circle_boxes_symmetric(r_in, r_out, y0, y1, offset_x=offset_x, offset_z=offset_z)
    for idx, (xm, ym0, zm, xp, ym1, zp) in enumerate(mids):
        g.box(b, f"{prefix}_mid_{idx}", (xm, ym0, zm), (xp, ym1, zp), mat)
    for idx, (xm, ym0, zm, xp, ym1, zp) in enumerate(pairs):
        g.pair(b, f"{prefix}_{idx}", (xm, ym0, zm), (xp, ym1, zp), mat)


# ───────────────────────────────────────────────────────────── 零件建模实现
def part_basin_bottom(g: Gear) -> None:
    """盆底基板：圆形平底、底心凹印与火蚀斑块（对称）。"""
    b = "basin_bottom"
    # 圆形底板（R≈3.8，Y∈[2.45, 3.15]）
    add_symmetric_ring(g, b, "floor_base", 0.0, 3.8, 2.45, 3.15, "iron")

    # 底面中心同心防变形环印（暗火蚀斑，R∈[1.6, 2.6]，Y∈[2.32, 2.45]，微偏移 offset_z 消除共面）
    add_symmetric_ring(g, b, "floor_heat_ring", 1.6, 2.6, 2.32, 2.45, "rust", offset_z=0.015)

    # 底心下凹排气暗孔（用于穿模自证，尺寸 ≥ 0.62，Y∈[2.18, 2.45]）
    g.box(b, "floor_cavity_void", (-0.75, 2.18, -0.75), (0.75, 2.45, 0.75), "void")


def part_basin_belly(g: Gear) -> None:
    """圆鼓盆腹：逐层圆环构筑圆腹、下敛中鼓与加固弦纹（严格对称）。"""
    b = "basin_belly"
    # 腹部采用 4 级逐层圆环向上展宽并呈现饱满弧形外鼓：
    # 第 1 级（近底敛腹，R_in=3.6, R_out=4.8，Y∈[3.15, 4.15]）
    add_symmetric_ring(g, b, "belly_t1", 3.6, 4.8, 3.15, 4.15, "iron")

    # 第 2 级（中下腹弧形渐鼓，R_in=4.6, R_out=5.8，Y∈[4.15, 5.15]）
    add_symmetric_ring(g, b, "belly_t2", 4.6, 5.8, 4.15, 5.15, "iron")

    # 第 3 级（最鼓腹层，饱满圆弧，R_in=5.2, R_out=6.5，Y∈[5.15, 6.15]）
    add_symmetric_ring(g, b, "belly_t3", 5.2, 6.5, 5.15, 6.15, "iron")

    # 第 4 级（上腹微敛收颈，R_in=4.8, R_out=5.8，Y∈[6.15, 7.05]）
    add_symmetric_ring(g, b, "belly_t4", 4.8, 5.8, 6.15, 7.05, "iron")

    # 外壁水平环绕铸铁加固凸起弦纹（最鼓腹 Y≈5.0 处的凸出加强筋圈，R∈[6.15, 6.85]，Y∈[4.85, 5.45]，微偏移 offset_x/offset_z 消除共面）
    add_symmetric_ring(g, b, "belly_rib", 6.15, 6.85, 4.85, 5.45, "iron_lt", offset_x=0.015, offset_z=0.015)


def part_basin_rim(g: Gear) -> None:
    """外折宽平沿：外撇大宽沿与规整回纹（雷纹）浅浮雕饰带（严格对称）。"""
    b = "basin_rim"
    # 平沿向外水平挑出成大平沿（R_in=5.4, R_out=7.3，口径 14.6px，宽平沿！Y∈[7.05, 7.75]）
    add_symmetric_ring(g, b, "rim_plate", 5.4, 7.3, 7.05, 7.75, "iron")

    # 宽平沿最外缘下翻扣唇（R∈[7.05, 7.42]，Y∈[6.75, 7.05]）
    add_symmetric_ring(g, b, "rim_lip", 7.05, 7.42, 6.75, 7.05, "iron_dk")

    # 平沿上表面起棱铸造「回纹 / 雷纹」规整饰齿（Y∈[7.75, 8.05]，材质 iron_lt 高光显眼）
    # 1. 前后中轴径向方折棱（自身严格对称于 X=0）
    g.box(b, "key_fret_f_mid", (-0.35, 7.75, -7.05), (0.35, 8.05, -5.65), "iron_lt")
    g.box(b, "key_fret_b_mid", (-0.35, 7.75, 5.65), (0.35, 8.05, 7.05), "iron_lt")

    # 2. 左右两侧径向方折棱（成对对称）
    g.pair(b, "key_fret_side_mid", (5.65, 7.75, -0.35), (7.05, 8.05, 0.35), "iron_lt")

    # 3. 四角 45° 放射回纹齿（成对对称）
    g.pair(b, "key_fret_diag_f", (3.95, 7.75, -4.95), (4.95, 8.05, -3.95), "iron_lt")
    g.pair(b, "key_fret_diag_b", (3.95, 7.75, 3.95), (4.95, 8.05, 4.95), "iron_lt")

    # 4. 前后象限环形间隔回纹齿（成对对称）
    g.pair(b, "key_fret_f_in", (1.75, 7.75, -6.65), (2.45, 8.05, -5.85), "iron_lt")
    g.pair(b, "key_fret_f_out", (5.15, 7.75, -3.25), (5.95, 8.05, -2.55), "iron_lt")
    g.pair(b, "key_fret_b_in", (1.75, 7.75, 5.85), (2.45, 8.05, 6.65), "iron_lt")
    g.pair(b, "key_fret_b_out", (5.15, 7.75, 2.55), (5.95, 8.05, 3.25), "iron_lt")

    # 5. 回纹内圈折角方钩（成对对称，X/Z 严格避开相邻回纹齿）
    g.pair(b, "key_hook_f1", (2.65, 7.75, -6.05), (3.45, 8.05, -5.55), "iron_lt")
    g.pair(b, "key_hook_f2", (5.45, 7.75, -2.45), (6.15, 8.05, -1.55), "iron_lt")
    g.pair(b, "key_hook_b1", (2.65, 7.75, 5.55), (3.45, 8.05, 6.05), "iron_lt")
    g.pair(b, "key_hook_b2", (5.45, 7.75, 1.55), (6.15, 8.05, 2.45), "iron_lt")


def part_rim_notches(g: Gear) -> None:
    """沿口磕缺残损：旧时代碰撞残缺的小崩口与暗铁茬（不对称使用痕迹）。"""
    b = "rim_notches"
    # 左前沿口磕缺崩角（X=-5.4, Z=-5.4）
    g.box(b, "notch_fl_pit", (-5.75, 7.65, -5.75), (-4.85, 8.12, -4.85), "void")
    g.box(b, "notch_fl_chip1", (-5.95, 7.55, -5.55), (-5.35, 7.95, -4.95), "iron_dk")
    g.box(b, "notch_fl_chip2", (-5.55, 7.55, -5.95), (-4.95, 7.95, -5.35), "rust")

    # 右后沿口轻微磨损凹痕
    g.box(b, "notch_br_pit", (5.85, 7.75, 3.45), (6.45, 8.12, 4.05), "rust_dk")
    g.box(b, "notch_br_lip", (5.75, 7.65, 3.35), (6.55, 7.88, 4.15), "iron_dk")


def part_beast_leg_b(g: Gear) -> None:
    """后位外撇兽足：兽面膝盖、起脊曲腿与三趾踏地爪（严格对称于 X=0，下端触地 y=0）。"""
    b = "beast_leg_b"
    # 后腿中心 X=0.0，Z∈[3.8, 6.2]，高 Y∈[0.0, 3.4]
    # 1. 上段兽面膝骨（三轴均 ≥ 0.62，Z 避开 belly 切片 3.75）
    g.box(b, "leg_b_knee_main", (-1.25, 2.15, 3.72), (1.25, 3.35, 4.82), "iron")
    g.box(b, "leg_b_knee_crest", (-0.95, 2.55, 4.65), (0.95, 3.25, 5.15), "iron_lt")
    g.box(b, "leg_b_knee_brow", (-1.15, 2.85, 4.45), (1.15, 3.15, 5.05), "iron_lt")
    g.pair(b, "leg_b_knee_eye", (0.45, 2.65, 4.85), (0.85, 2.95, 5.22), "iron_dk")

    # 2. 中段起脊弯曲胫骨（向后外撇倾斜，带外凸脊线）
    g.box(b, "leg_b_shank_main", (-0.95, 0.85, 4.25), (0.95, 2.25, 5.35), "iron")
    g.box(b, "leg_b_shank_ridge", (-0.35, 0.75, 5.15), (0.35, 2.35, 5.55), "iron_lt")
    g.pair(b, "leg_b_shank_flange", (0.85, 1.05, 4.45), (1.15, 2.05, 5.15), "iron_dk")

    # 3. 下段三趾踏地爪（牢牢坐于 y=0.0）
    # 中趾（正后前伸）
    g.box(b, "leg_b_toe_mid", (-0.45, 0.00, 5.25), (0.45, 0.85, 6.25), "iron")
    g.box(b, "leg_b_claw_mid", (-0.35, 0.03, 6.05), (0.35, 0.45, 6.55), "iron_lt")
    # 左右侧趾（成对对称）
    g.pair(b, "leg_b_toe_side", (0.45, 0.00, 4.85), (1.15, 0.75, 5.75), "iron")
    g.pair(b, "leg_b_claw_side", (0.75, 0.03, 5.55), (1.35, 0.35, 6.15), "iron_lt")

    # 足跟落地掌垫
    g.box(b, "leg_b_heel_pad", (-0.75, 0.00, 4.05), (0.75, 0.65, 4.85), "iron_dk")


def part_beast_leg_fl(g: Gear) -> None:
    """前左外撇兽足：斜向外撇的雄劲兽爪足（触地 y=0）。"""
    b = "beast_leg_fl"
    cx, cz = -3.75, -2.65
    # 1. 上段膝骨
    g.box(b, "leg_fl_knee_main", (cx - 1.15, 2.15, cz - 1.15), (cx + 0.85, 3.35, cz + 0.85), "iron")
    g.box(b, "leg_fl_knee_crest", (cx - 1.35, 2.55, cz - 1.35), (cx - 0.45, 3.25, cz - 0.45), "iron_lt")
    g.box(b, "leg_fl_knee_brow", (cx - 1.25, 2.85, cz - 1.05), (cx - 0.25, 3.15, cz - 0.25), "iron_lt")
    g.box(b, "leg_fl_eye1", (cx - 1.05, 2.65, cz - 1.25), (-4.25, 2.95, -3.35), "iron_dk")

    # 2. 中段胫骨（斜向外撇）
    g.box(b, "leg_fl_shank_main", (cx - 1.45, 0.85, cz - 1.45), (cx + 0.45, 2.25, cz + 0.45), "iron")
    g.box(b, "leg_fl_shank_ridge", (cx - 1.65, 0.75, cz - 1.65), (cx - 0.95, 2.35, cz - 0.95), "iron_lt")
    g.box(b, "leg_fl_flange", (cx - 0.65, 1.05, cz - 1.55), (cx + 0.25, 2.05, cz - 0.75), "iron_dk")

    # 3. 下段爪足（触地 y=0.0）
    g.box(b, "leg_fl_toe_mid", (cx - 2.15, 0.00, cz - 2.15), (cx - 0.85, 0.85, cz - 0.85), "iron")
    g.box(b, "leg_fl_claw_mid", (cx - 2.45, 0.03, cz - 2.45), (cx - 1.65, 0.45, cz - 1.65), "iron_lt")
    g.box(b, "leg_fl_toe_l", (cx - 1.45, 0.00, cz - 2.35), (cx - 0.45, 0.75, cz - 1.35), "iron")
    g.box(b, "leg_fl_claw_l", (cx - 1.75, 0.03, cz - 2.65), (cx - 0.85, 0.35, cz - 1.95), "iron_lt")
    g.box(b, "leg_fl_toe_r", (cx - 2.35, 0.00, cz - 1.45), (cx - 1.35, 0.75, cz - 0.45), "iron")
    g.box(b, "leg_fl_claw_r", (cx - 2.65, 0.03, cz - 1.75), (cx - 1.95, 0.35, cz - 0.85), "iron_lt")
    g.box(b, "leg_fl_pad", (cx - 1.15, 0.00, cz - 1.15), (cx + 0.55, 0.95, cz + 0.55), "iron_dk")


def part_beast_leg_fr(g: Gear) -> None:
    """前右外撇兽足：斜向外撇的雄劲兽爪足（触地 y=0）。"""
    b = "beast_leg_fr"
    cx, cz = 3.75, -2.65
    # 1. 上段膝骨
    g.box(b, "leg_fr_knee_main", (cx - 0.85, 2.15, cz - 1.15), (cx + 1.15, 3.35, cz + 0.85), "iron")
    g.box(b, "leg_fr_knee_crest", (cx + 0.45, 2.55, cz - 1.35), (cx + 1.35, 3.25, cz - 0.45), "iron_lt")
    g.box(b, "leg_fr_knee_brow", (cx + 0.25, 2.85, cz - 1.05), (cx + 1.25, 3.15, cz - 0.25), "iron_lt")
    g.box(b, "leg_fr_eye1", (4.25, 2.65, -3.35), (cx + 1.05, 2.95, cz - 1.25), "iron_dk")

    # 2. 中段胫骨
    g.box(b, "leg_fr_shank_main", (cx - 0.45, 0.85, cz - 1.45), (cx + 1.45, 2.25, cz + 0.45), "iron")
    g.box(b, "leg_fr_shank_ridge", (cx + 0.95, 0.75, cz - 1.65), (cx + 1.65, 2.35, cz - 0.95), "iron_lt")
    g.box(b, "leg_fr_flange", (cx - 0.25, 1.05, cz - 1.55), (cx + 0.65, 2.05, cz - 0.75), "iron_dk")

    # 3. 下段爪足（触地 y=0.0）
    g.box(b, "leg_fr_toe_mid", (cx + 0.85, 0.00, cz - 2.15), (cx + 2.15, 0.85, cz - 0.85), "iron")
    g.box(b, "leg_fr_claw_mid", (cx + 1.65, 0.03, cz - 2.45), (cx + 2.45, 0.45, cz - 1.65), "iron_lt")
    g.box(b, "leg_fr_toe_r", (cx + 0.45, 0.00, cz - 2.35), (cx + 1.45, 0.75, cz - 1.35), "iron")
    g.box(b, "leg_fr_claw_r", (cx + 0.85, 0.03, cz - 2.65), (cx + 1.75, 0.35, cz - 1.95), "iron_lt")
    g.box(b, "leg_fr_toe_l", (cx + 1.35, 0.00, cz - 1.45), (cx + 2.35, 0.75, cz - 0.45), "iron")
    g.box(b, "leg_fr_claw_l", (cx + 1.95, 0.03, cz - 1.75), (cx + 2.65, 0.35, cz - 0.85), "iron_lt")
    g.box(b, "leg_fr_pad", (cx - 0.55, 0.00, cz - 0.95), (cx + 1.15, 0.95, cz + 0.55), "iron_dk")


def part_handle_boss_l(g: Gear) -> None:
    """左侧兽面铺首：威严饕餮面浮雕座与衔环插孔。"""
    b = "handle_boss_l"
    cx, cy, cz = -6.45, 5.65, 0.0
    # 铺首底板
    g.box(b, "boss_l_base", (cx - 0.85, cy - 1.15, cz - 1.45), (cx + 0.45, cy + 1.15, cz + 1.45), "iron")
    g.box(b, "boss_l_brow", (cx - 1.05, cy + 0.65, cz - 1.35), (cx - 0.25, cy + 1.25, cz + 1.35), "iron_lt")
    g.box(b, "boss_l_horn_t", (cx - 0.95, cy + 1.15, cz - 0.95), (cx - 0.35, cy + 1.65, cz + 0.95), "iron_lt")

    # 兽目与鼻梁
    g.box(b, "boss_l_snout", (cx - 1.25, cy - 0.35, cz - 0.85), (cx - 0.45, cy + 0.55, cz + 0.85), "iron")
    g.box(b, "boss_l_eye_f", (cx - 1.12, cy + 0.25, cz - 1.25), (cx - 0.55, cy + 0.65, cz - 0.65), "iron_lt")
    g.box(b, "boss_l_eye_b", (cx - 1.12, cy + 0.25, cz + 0.65), (cx - 0.55, cy + 0.65, cz + 1.25), "iron_lt")

    # 衔环下唇与通孔（void）
    g.box(b, "boss_l_jaw", (cx - 1.15, cy - 0.95, cz - 0.75), (cx - 0.35, cy - 0.25, cz + 0.75), "iron_dk")
    g.box(b, "boss_l_ring_hole", (cx - 1.05, cy - 0.65, cz - 0.35), (cx - 0.45, cy - 0.05, cz + 0.35), "void")
    g.box(b, "boss_l_rivet_t", (cx - 0.95, cy + 0.85, cz - 0.22), (cx - 0.55, cy + 1.25, cz + 0.22), "iron_lt")
    g.box(b, "boss_l_rivet_b", (cx - 0.95, cy - 1.05, cz - 0.22), (cx - 0.55, cy - 0.65, cz + 0.22), "iron_lt")
    g.box(b, "boss_l_patina", (cx - 0.90, cy + 0.25, cz - 0.45), (cx - 0.60, cy + 0.75, cz + 0.45), "bronze_pat")


def part_handle_boss_r(g: Gear) -> None:
    """右侧兽面铺首：威严饕餮面浮雕座与衔环插孔。"""
    b = "handle_boss_r"
    cx, cy, cz = 6.45, 5.65, 0.0
    # 铺首底板
    g.box(b, "boss_r_base", (cx - 0.45, cy - 1.15, cz - 1.45), (cx + 0.85, cy + 1.15, cz + 1.45), "iron")
    g.box(b, "boss_r_brow", (cx + 0.25, cy + 0.65, cz - 1.35), (cx + 1.05, cy + 1.25, cz + 1.35), "iron_lt")
    g.box(b, "boss_r_horn_t", (cx + 0.35, cy + 1.15, cz - 0.95), (cx + 0.95, cy + 1.65, cz + 0.95), "iron_lt")

    # 兽目与鼻梁
    g.box(b, "boss_r_snout", (cx + 0.45, cy - 0.35, cz - 0.85), (cx + 1.25, cy + 0.55, cz + 0.85), "iron")
    g.box(b, "boss_r_eye_f", (cx + 0.55, cy + 0.25, cz - 1.25), (cx + 1.12, cy + 0.65, cz - 0.65), "iron_lt")
    g.box(b, "boss_r_eye_b", (cx + 0.55, cy + 0.25, cz + 0.65), (cx + 1.12, cy + 0.65, cz + 1.25), "iron_lt")

    # 衔环下唇与通孔
    g.box(b, "boss_r_jaw", (cx + 0.35, cy - 0.95, cz - 0.75), (cx + 1.15, cy - 0.25, cz + 0.75), "iron_dk")
    g.box(b, "boss_r_ring_hole", (cx + 0.45, cy - 0.65, cz - 0.35), (cx + 1.05, cy - 0.05, cz + 0.35), "void")
    g.box(b, "boss_r_rivet_t", (cx + 0.55, cy + 0.85, cz - 0.22), (cx + 0.95, cy + 1.25, cz + 0.22), "iron_lt")
    g.box(b, "boss_r_rivet_b", (cx + 0.55, cy - 1.05, cz - 0.22), (cx + 0.95, cy - 0.65, cz + 0.22), "iron_lt")
    g.box(b, "boss_r_patina", (cx + 0.60, cy + 0.25, cz - 0.45), (cx + 0.90, cy + 0.75, cz + 0.45), "bronze_pat")


def part_handle_ring_l(g: Gear) -> None:
    """左侧锻铁衔环：自然下垂的八角铁圆环（独立可动挂件骨）。"""
    b = "handle_ring_l"
    cx, cy, cz = -7.45, 4.25, 0.0
    # 八角锻打悬环（外径 2.8px，线径 0.55px）
    g.box(b, "ring_l_bar_t", (cx - 0.28, cy + 1.05, cz - 0.85), (cx + 0.28, cy + 1.55, cz + 0.85), "iron")
    g.box(b, "ring_l_bar_b", (cx - 0.28, cy - 1.55, cz - 0.85), (cx + 0.28, cy - 1.05, cz + 0.85), "iron")
    g.box(b, "ring_l_bar_f", (cx - 0.28, cy - 0.95, cz - 1.45), (cx + 0.28, cy + 0.95, cz - 0.95), "iron_lt")
    g.box(b, "ring_l_bar_r", (cx - 0.28, cy - 0.95, cz + 0.95), (cx + 0.28, cy + 0.95, cz + 1.45), "iron")

    # 四切角加强
    g.box(b, "ring_l_corn_tf", (cx - 0.25, cy + 0.75, cz - 1.25), (cx + 0.25, cy + 1.35, cz - 0.65), "iron_lt")
    g.box(b, "ring_l_corn_tb", (cx - 0.25, cy + 0.75, cz + 0.65), (cx + 0.25, cy + 1.35, cz + 1.25), "rust")
    g.box(b, "ring_l_corn_bf", (cx - 0.25, cy - 1.35, cz - 1.25), (cx + 0.25, cy - 0.75, cz - 0.65), "rust")
    g.box(b, "ring_l_corn_bb", (cx - 0.25, cy - 1.35, cz + 0.65), (cx + 0.25, cy - 0.75, cz + 1.25), "iron")

    # 环心镂空（void）
    g.box(b, "ring_l_void", (cx - 0.35, cy - 0.95, cz - 0.85), (cx + 0.35, cy + 0.95, cz + 0.85), "void")
    g.box(b, "ring_l_wear_notch", (cx - 0.32, cy - 1.25, cz - 0.35), (cx + 0.32, cy - 0.85, cz + 0.35), "iron_lt")


def part_handle_ring_r(g: Gear) -> None:
    """右侧锻铁衔环：自然下垂的八角铁圆环（独立可动挂件骨）。"""
    b = "handle_ring_r"
    cx, cy, cz = 7.45, 4.25, 0.0
    # 八角锻打悬环
    g.box(b, "ring_r_bar_t", (cx - 0.28, cy + 1.05, cz - 0.85), (cx + 0.28, cy + 1.55, cz + 0.85), "iron")
    g.box(b, "ring_r_bar_b", (cx - 0.28, cy - 1.55, cz - 0.85), (cx + 0.28, cy - 1.05, cz + 0.85), "iron")
    g.box(b, "ring_r_bar_f", (cx - 0.28, cy - 0.95, cz - 1.45), (cx + 0.28, cy + 0.95, cz - 0.95), "iron_lt")
    g.box(b, "ring_r_bar_r", (cx - 0.28, cy - 0.95, cz + 0.95), (cx + 0.28, cy + 0.95, cz + 1.45), "iron")

    # 四切角加强
    g.box(b, "ring_r_corn_tf", (cx - 0.25, cy + 0.75, cz - 1.25), (cx + 0.25, cy + 1.35, cz - 0.65), "iron_lt")
    g.box(b, "ring_r_corn_tb", (cx - 0.25, cy + 0.75, cz + 0.65), (cx + 0.25, cy + 1.35, cz + 1.25), "rust")
    g.box(b, "ring_r_corn_bf", (cx - 0.25, cy - 1.35, cz - 1.25), (cx + 0.25, cy - 0.75, cz - 0.65), "rust")
    g.box(b, "ring_r_corn_bb", (cx - 0.25, cy - 1.35, cz + 0.65), (cx + 0.25, cy - 0.75, cz + 1.25), "iron")

    # 环心镂空
    g.box(b, "ring_r_void", (cx - 0.35, cy - 0.95, cz - 0.85), (cx + 0.35, cy + 0.95, cz + 0.85), "void")
    g.box(b, "ring_r_wear_notch", (cx - 0.32, cy - 1.25, cz - 0.35), (cx + 0.32, cy - 0.85, cz + 0.35), "iron_lt")


def part_ash_bed(g: Gear) -> None:
    """盆底冷灰层：薄覆盆底的沉寂灰白炭灰圆形薄层。"""
    b = "ash_bed"
    # 炭灰圆形薄层（R≈4.2，Y∈[3.15, 3.55]）
    add_symmetric_ring(g, b, "ash_layer", 0.0, 4.2, 3.15, 3.55, "ash")

    # 灰面中央微凹炭灰堆（R≈2.6，Y∈[3.55, 3.82]）
    add_symmetric_ring(g, b, "ash_mound", 0.0, 2.6, 3.55, 3.82, "ash_dk")


def part_ember_coals(g: Gear) -> None:
    """残炭与高对比余烬：两块焦黑断炭与明亮红热余烬（核心点睛件，末法缺燃料主题！）。"""
    b = "ember_coals"
    # 残炭 1（右中大块断炭，长 2.2×宽 1.6×高 1.2px，底面坐于 y=3.85 之上）
    g.box(b, "coal_1_main", (0.45, 3.85, -0.95), (2.25, 4.75, 0.45), "charcoal")
    g.box(b, "coal_1_top_ridge", (0.65, 4.65, -0.75), (1.95, 4.95, 0.25), "charcoal")
    g.box(b, "coal_1_ash_crust", (0.35, 3.95, -0.85), (0.75, 4.55, 0.35), "ash_dk")

    # 残炭 1 内部裂开的高对比炽热红橙余烬核心（鲜明红橙！）
    g.box(b, "ember_core_1", (0.95, 4.05, -0.65), (1.85, 4.65, 0.15), "ember")
    g.box(b, "ember_glow_spark_1", (1.25, 4.35, -0.45), (1.65, 4.85, -0.05), "ember_glow")
    g.box(b, "ember_slit_glow_1", (0.75, 4.25, -0.85), (1.15, 4.55, -0.35), "ember_glow")

    # 残炭 2（左前小块断炭，斜卧）
    g.box(b, "coal_2_main", (-1.95, 3.85, -1.65), (-0.45, 4.55, -0.45), "charcoal")
    g.box(b, "coal_2_ridge", (-1.75, 4.45, -1.45), (-0.75, 4.75, -0.65), "charcoal")
    # 残炭 2 底部微弱红光与火星
    g.box(b, "ember_core_2", (-1.65, 3.95, -1.25), (-0.85, 4.25, -0.65), "ember")
    g.box(b, "ember_glow_spark_2", (-1.45, 4.05, -1.05), (-1.05, 4.45, -0.75), "ember_glow")

    # 残炭 3（后侧小炭粒与灰烬中隐现的火星）
    g.box(b, "coal_3_small", (-0.65, 3.85, 1.15), (0.45, 4.25, 1.95), "charcoal")
    g.box(b, "ember_spark_flake", (-0.15, 3.88, 1.35), (0.25, 4.15, 1.75), "ember_glow")
    g.box(b, "ember_ash_rim", (0.85, 3.85, -1.15), (1.95, 4.05, -0.75), "ember")
    g.box(b, "ember_clinker_1", (-1.45, 3.85, 0.65), (-0.95, 4.05, 1.15), "charcoal")
    g.box(b, "ember_clinker_spark", (-1.35, 3.88, 0.75), (-1.05, 4.02, 1.05), "ember_glow")
    g.box(b, "ember_speck_l", (-2.25, 3.82, -0.65), (-1.85, 4.02, -0.25), "ember_glow")
    g.box(b, "ember_speck_r", (2.15, 3.82, 0.35), (2.55, 4.02, 0.75), "ember_glow")


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


class TieHuoPenSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict_and_fix(g)
        return g


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "basin_bottom": part_basin_bottom,
    "basin_belly": part_basin_belly,
    "basin_rim": part_basin_rim,
    "rim_notches": part_rim_notches,
    "beast_leg_b": part_beast_leg_b,
    "beast_leg_fl": part_beast_leg_fl,
    "beast_leg_fr": part_beast_leg_fr,
    "handle_boss_l": part_handle_boss_l,
    "handle_boss_r": part_handle_boss_r,
    "handle_ring_l": part_handle_ring_l,
    "handle_ring_r": part_handle_ring_r,
    "ash_bed": part_ash_bed,
    "ember_coals": part_ember_coals,
}

SPEC = TieHuoPenSpec(
    gear_id="tie_huo_pen",
    model_name="TieHuoPen",
    title="铁火盆 / tie_huo_pen",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="地上摆件 · 铸铁圆形浅盆+圆鼓腹+回纹平沿+外撇三兽足+兽面衔环+薄冷灰与高对比余烬",
    worn_focus=((7.5, 12.0, 0.0), 38.0),
    tex_views=(
        ("3/4 俯斜（+z/+x上方）", 32.0, 26.0),
        ("FRONT 正面（-z）", 180.0, 0.0),
        ("TOP 顶视（看盆内余烬）", 180.0, 75.0),
    ),
    texture_overrides={
        "iron": "metal_iron",
        "iron_lt": "metal_iron_lt",
        "iron_dk": "metal_iron_dk",
        "rust": "metal_rust",
        "rust_dk": "metal_rust_dk",
        "bronze_pat": "metal_bronze_pat",
        "ash": "metal_ash",
        "ash_dk": "metal_ash_dk",
        "charcoal": "metal_charcoal",
        "ember": "metal_ember",
        "ember_glow": "metal_ember_glow",
    },
    category="metalware",
    scale_mode="placed",
    player_offset=(15.0, 0.0, 0.0),
    textured=True,
    round_label="round 3/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
