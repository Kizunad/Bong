#!/usr/bin/env python3
"""皮囊胸挂（pi_nang_xiong_gua）Blockbench .bbmodel 程序化生成器 —— Round 1/3 返工重构。

任务卡：末土装备 #19（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

调度审返工要求：
- 胸前挂带加宽到几乎整个躯干正面宽度（x 从 -3.95 到 +3.95）；
- 横贯胸前的一整排 5 个饱满鼓肚皮囊（每个皮囊有厚度、双层翻盖、骨扣/骨栓、缝线）、
  3 个精细小陶瓶（左中右各异，有陶瓶塞、细绳绑圈、挂带）；
- 加上毛边粗麻破布与下摆衬垫；
- 背后大幅度 X 形立体交叉背带（带中心菱形加固皮垫、铆钉、防磨边）；
- 下方横置硬皮鞘与骨锥刀，侧腰环带与垂尾；
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
    "hide":       (120, 78, 56),      # 主皮囊生皮
    "hide_dk":    (84, 54, 40),       # 暗色厚皮/封底
    "hide_lt":    (152, 108, 78),     # 翻盖亮皮
    "cloth":      (156, 140, 110),    # 毛边粗麻布
    "cloth_dk":   (112, 98, 76),      # 脏粗麻阴影
    "strap":      (112, 72, 50),      # X背带
    "strap_dk":   (78, 48, 36),       # 深色背带
    "clay":       (160, 94, 62),      # 陶瓶陶色
    "clay_dk":    (112, 60, 40),      # 粗陶阴影/彩陶纹
    "cord":       (144, 128, 78),     # 草绳绑扎
    "cord_dk":    (102, 88, 56),      # 深色麻绳
    "bone":       (214, 204, 176),    # 骨栓/骨塞
    "bone_lt":    (236, 226, 200),    # 骨塞头/骨锥尖
    "bone_stain": (132, 112, 84),     # 老骨斑
    "iron":       (82, 78, 76),       # 粗锻生铁扣
    "iron_lt":    (116, 112, 110),    # 铁光泽 / 铆钉
    "rust":       (152, 86, 52),      # 铁锈斑
    "void":       (28, 24, 22),       # 穿孔暗影
}

# 软件材质与硬物允许贴合咬合
SOFT = ("hide", "hide_dk", "hide_lt", "cloth", "cloth_dk", "strap", "strap_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("bone", "bone_lt", "bone_stain", "clay", "clay_dk", "iron", "hide_dk"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("harness_front", "root", (0.0, 18.0, -2.25), "胸前主挂载大宽带：横贯整个躯干正面的承重大带、吊带与下摆衬布"),
    ("harness_back_x", "root", (0.0, 18.0, 2.25), "背后 X 形立体交叉背带：双向斜跨背带、中心交叉菱形牛皮垫、定位铆钉"),
    ("harness_waist", "root", (0.0, 11.6, 0.0), "左右环腰连接带：低位过侧腰皮带、粗铁日字带扣、咬合舌片"),
    ("pouch_row1", "harness_front", (-2.6, 16.5, -2.6), "左外侧大药囊（#1）：弧面加厚生皮药袋、多层翻盖、角扣与粗缝线"),
    ("pouch_row2", "harness_front", (-1.3, 16.5, -2.65), "左中方皮囊（#2）：方形储物皮仓、立柱骨栓、横向扎紧皮带"),
    ("pouch_row3", "harness_front", (0.0, 16.5, -2.7), "中央宽厚双格主囊（#3）：胸口正中大容积双联皮仓、双骨扣、皮搭扣"),
    ("pouch_row4", "harness_front", (1.3, 16.5, -2.65), "右中多插孔工具袋（#4）：带开孔皮护套、三插槽、骨针插件"),
    ("pouch_row5", "harness_front", (2.6, 16.5, -2.6), "右外侧扎口皮袋（#5）：束口式柔皮软囊、环绕皮绳结与皮盖"),
    ("jar_l", "harness_front", (-3.25, 14.5, -2.75), "左侧小陶药瓶：圆肚短颈粗陶罐、带孔兽骨塞、麻绳环扣（独立骨）"),
    ("jar_m", "harness_front", (-0.65, 14.2, -2.8), "中央扁壶小彩陶瓶：扁腹彩绘陶水罐、软木塞、下垂双股麻绳（独立骨）"),
    ("jar_r", "harness_front", (3.25, 14.5, -2.75), "右侧细长陶丹瓶：细颈广口彩陶瓶、兽皮封口软塞、扎紧绳圈（独立骨）"),
    ("hang_knife", "harness_front", (0.0, 13.5, -2.55), "胸板下方横插骨锥：横置打孔硬皮鞘、小骨锥柄与锋利骨尖（独立骨）"),
    ("hang_waist_tails", "harness_waist", (4.25, 10.0, 0.0), "腰带垂落皮舌与骨饰：穿出带扣自然下垂的双侧打孔皮带头与兽骨坠（独立骨）"),
]

# 不对称骨骼（五个各异皮囊、三个不同陶瓶、横插骨锥、垂尾）
ASYM_BONES = frozenset({
    "harness_back_x", "pouch_row1", "pouch_row2", "pouch_row3", "pouch_row4", "pouch_row5",
    "jar_l", "jar_m", "jar_r", "hang_knife", "hang_waist_tails"
})


# ───────────────────────────────────────────────────────────── 几何部件构建
def part_harness_front(g: Gear) -> None:
    """胸前主挂载大宽带：横贯整个躯干正面的承重大带、吊带与下摆衬布。"""
    b = "harness_front"
    # 躯干宽度为 x∈[-4.0, 4.0]，挂带横贯整个躯干宽度：x∈[-3.95, 3.95]！
    # 主横承重带（y∈[15.80, 17.85]，厚度充分，z∈[-2.36, -2.08]）
    g.box(b, "f_band_main", (-3.95, 15.80, -2.36), (3.95, 17.85, -2.08), "strap")
    g.box(b, "f_band_top_lip", (-3.88, 17.85, -2.44), (3.88, 18.25, -2.12), "strap_dk")
    g.box(b, "f_band_bot_lip", (-3.88, 15.32, -2.44), (3.88, 15.80, -2.12), "strap_dk")

    # 底层破损粗麻衬布与下摆（厚度 ≥ 0.22）
    g.box(b, "f_cloth_underlay", (-3.85, 14.65, -2.32), (3.85, 18.45, -2.06), "cloth")
    g.box(b, "f_cloth_fringe_l", (-3.75, 13.85, -2.34), (-1.25, 14.68, -2.08), "cloth_dk")
    g.box(b, "f_cloth_fringe_r", (1.25, 13.85, -2.34), (3.75, 14.68, -2.08), "cloth_dk")
    g.box(b, "f_cloth_tear", (-0.85, 14.05, -2.36), (0.85, 14.75, -2.08), "hide_dk")

    # 胸前中轴双铆钉加强皮盘（中心对称）
    g.box(b, "f_center_boss", (-1.15, 15.35, -2.50), (1.15, 18.30, -2.16), "hide_dk")
    g.box(b, "f_center_rivet_t", (-0.18, 17.92, -2.68), (0.18, 18.22, -2.35), "iron_lt")
    g.box(b, "f_center_rivet_b", (-0.18, 15.42, -2.68), (0.18, 15.72, -2.35), "iron_lt")
    # 增加双层挂载背带与缝线（成对镜像 pair）
    g.box(b, "f_band_sub_strap1", (-3.88, 16.35, -2.40), (3.88, 16.75, -2.10), "strap_dk")
    g.box(b, "f_band_sub_strap2", (-3.88, 17.15, -2.40), (3.88, 17.55, -2.10), "strap_dk")
    for idx, sx in enumerate((0.5, 1.8, 2.8, 3.5)):
        g.pair(b, f"f_band_rivet_row_{idx}", (sx - 0.12, 17.92, -2.52), (sx + 0.12, 18.18, -2.25), "iron_lt")
        g.pair(b, f"f_band_rivet_bot_{idx}", (sx - 0.12, 15.42, -2.52), (sx + 0.12, 15.68, -2.25), "iron_lt")
        g.pair(b, f"f_cloth_stitch_row_{idx}", (sx - 0.15, 14.25, -2.38), (sx + 0.15, 14.65, -2.12), "cord_dk")
        g.pair(b, f"f_cloth_stitch_top_{idx}", (sx - 0.15, 18.25, -2.38), (sx + 0.15, 18.55, -2.12), "cord")
    # 额外在两端加一对角骨片与铁挂扣（避开共面）
    g.pair(b, "f_band_horn_tag", (3.62, 15.15, -2.52), (3.92, 15.75, -2.15), "bone_stain")
    g.pair(b, "f_band_ring_tag", (3.68, 17.68, -2.52), (3.92, 18.08, -2.15), "iron_lt")

    # 前肩斜下转接段（从 y=23.15 上升至 y=23.85，x 向外展至 4.12）
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        jx0, jx1 = min(sx * 2.85, sx * 3.82), max(sx * 2.85, sx * 3.82)
        g.box(b, f"f_suspender_{side}", (jx0, 17.88, -2.36), (jx1, 23.15, -2.08), "strap")
        kx0, kx1 = min(sx * 3.45, sx * 4.12), max(sx * 3.45, sx * 4.12)
        g.box(b, f"f_shoulder_jog_{side}", (kx0, 23.15, -2.38), (kx1, 23.85, -2.08), "strap_dk")
        rx0, rx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"f_shoulder_riser_{side}", (rx0, 23.80, -2.42), (rx1, 24.45, -2.10), "strap")
        ox0, ox1 = min(sx * 4.12, sx * 4.52), max(sx * 4.12, sx * 4.52)
        g.box(b, f"f_over_shoulder_{side}", (ox0, 24.10, -2.15), (ox1, 24.48, 2.15), "strap_dk")
        px0, px1 = min(sx * 4.16, sx * 4.56), max(sx * 4.16, sx * 4.56)
        g.box(b, f"f_shoulder_pad_{side}", (px0, 24.48, -1.65), (px1, 24.88, 1.65), "hide_lt")
        # 前胸悬挂双排针脚（严格镜像！）
        for idx, sy in enumerate((18.5, 19.5, 20.5, 21.5, 22.5)):
            sx0, sx1 = min(sx * 3.10, sx * 3.55), max(sx * 3.10, sx * 3.55)
            g.box(b, f"f_susp_stitch_{side}_{idx}", (sx0, sy, -2.46), (sx1, sy + 0.40, -2.15), "cord")


def part_harness_back_x(g: Gear) -> None:
    """背后 X 形立体交叉背带：双向斜跨背带、中心交叉菱形牛皮垫、定位铆钉。"""
    b = "harness_back_x"
    # 背部后表面位于 z = 2.0，所有背后部件严格位于 z ≥ 2.10！
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        rx0, rx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"b_shoulder_riser_{side}", (rx0, 23.80, 2.08), (rx1, 24.45, 2.40), "strap")
        jx0, jx1 = min(sx * 3.45, sx * 4.30), max(sx * 3.45, sx * 4.30)
        g.box(b, f"b_shoulder_jog_{side}", (jx0, 23.15, 2.12), (jx1, 23.85, 2.42), "strap_dk")

    # X 形两条交叉斜跨带（内层与外层错开 Z 深度，避免共面）
    # 第 1 条（内层）：左肩至右后腰
    g.box(b, "b_x1_seg1", (-3.75, 20.65, 2.10), (-2.05, 23.25, 2.34), "strap")
    g.box(b, "b_x1_seg2", (-2.25, 17.85, 2.12), (-0.55, 20.85, 2.36), "strap_dk")
    g.box(b, "b_x1_seg3", (-0.75, 15.05, 2.10), (0.95, 18.05, 2.34), "strap")
    g.box(b, "b_x1_seg4", (0.75, 12.25, 2.12), (2.45, 15.25, 2.36), "strap_dk")
    g.box(b, "b_x1_seg5", (2.25, 11.25, 2.10), (3.75, 12.65, 2.34), "strap")

    # 第 2 条（外层）：右肩至左后腰
    g.box(b, "b_x2_seg1", (2.05, 20.65, 2.38), (3.75, 23.25, 2.62), "strap")
    g.box(b, "b_x2_seg2", (0.55, 17.85, 2.40), (2.25, 20.85, 2.64), "strap_dk")
    g.box(b, "b_x2_seg3", (-0.95, 15.05, 2.38), (0.75, 18.05, 2.62), "strap")
    g.box(b, "b_x2_seg4", (-2.45, 12.25, 2.40), (-0.75, 15.25, 2.64), "strap_dk")
    g.box(b, "b_x2_seg5", (-3.75, 11.25, 2.38), (-2.25, 12.65, 2.62), "strap")

    # 背后 X 背带双排针脚（附着在各背带段上）
    # 段1 针脚 (y∈[20.65, 23.25])
    g.box(b, "b_stitch_x1_1", (-3.15, 21.25, 2.22), (-2.65, 21.65, 2.45), "cord_dk")
    g.box(b, "b_stitch_x1_2", (-2.75, 22.25, 2.22), (-2.25, 22.65, 2.45), "cord_dk")
    # 段2 针脚 (y∈[17.85, 20.85])
    g.box(b, "b_stitch_x1_3", (-1.95, 18.45, 2.22), (-1.45, 18.85, 2.45), "cord_dk")
    g.box(b, "b_stitch_x1_4", (-1.55, 19.65, 2.22), (-1.05, 20.05, 2.45), "cord_dk")
    # 段4 针脚 (y∈[12.25, 15.25])
    g.box(b, "b_stitch_x1_5", (1.05, 13.05, 2.22), (1.55, 13.45, 2.45), "cord_dk")
    g.box(b, "b_stitch_x1_6", (1.45, 14.15, 2.22), (1.95, 14.55, 2.45), "cord_dk")

    # X2 段1 针脚
    g.box(b, "b_stitch_x2_1", (2.65, 21.25, 2.45), (3.15, 21.65, 2.68), "cord_dk")
    g.box(b, "b_stitch_x2_2", (2.25, 22.25, 2.45), (2.75, 22.65, 2.68), "cord_dk")
    # X2 段2 针脚
    g.box(b, "b_stitch_x2_3", (1.45, 18.45, 2.45), (1.95, 18.85, 2.68), "cord_dk")
    g.box(b, "b_stitch_x2_4", (1.05, 19.65, 2.45), (1.55, 20.05, 2.68), "cord_dk")
    # X2 段4 针脚
    g.box(b, "b_stitch_x2_5", (-1.55, 13.05, 2.45), (-1.05, 13.45, 2.68), "cord_dk")
    g.box(b, "b_stitch_x2_6", (-1.95, 14.15, 2.45), (-1.45, 14.55, 2.68), "cord_dk")

    # 中心交叉处的菱形厚牛皮加固垫（居中，x 严格关于 0 对称）
    g.box(b, "b_cross_pad", (-1.35, 16.85, 2.65), (1.35, 19.25, 2.92), "hide_dk")
    g.box(b, "b_cross_plate", (-0.95, 17.20, 2.92), (0.95, 18.88, 3.20), "hide_lt")
    g.box(b, "b_cross_rivet_t", (-0.18, 18.42, 3.20), (0.18, 18.72, 3.48), "iron_lt")
    g.box(b, "b_cross_rivet_b", (-0.18, 17.28, 3.20), (0.18, 17.58, 3.48), "iron_lt")


def part_harness_waist(g: Gear) -> None:
    """左右环腰连接带：低位过侧腰皮带、粗铁日字带扣、咬合舌片。"""
    b = "harness_waist"
    # 低位于 y∈[11.25, 11.90]，x 位于 ±(4.10..4.38)
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        wx0, wx1 = min(sx * 4.10, sx * 4.38), max(sx * 4.10, sx * 4.38)
        g.box(b, f"waist_strap_{side}", (wx0, 11.25, -2.15), (wx1, 11.90, 2.15), "strap")
        px0, px1 = min(sx * 4.12, sx * 4.45), max(sx * 4.12, sx * 4.45)
        g.box(b, f"waist_pad_{side}", (px0, 11.35, -1.65), (px1, 11.80, 1.65), "strap_dk")

        # 腰间生铁日字带扣
        bx0, bx1 = min(sx * 3.25, sx * 4.35), max(sx * 3.25, sx * 4.35)
        g.box(b, f"buckle_frame_t_{side}", (bx0, 11.82, -2.62), (bx1, 12.12, -2.32), "iron")
        g.box(b, f"buckle_frame_b_{side}", (bx0, 11.02, -2.62), (bx1, 11.32, -2.32), "iron")
        lx0, lx1 = min(sx * 3.25, sx * 3.55), max(sx * 3.25, sx * 3.55)
        g.box(b, f"buckle_frame_in_{side}", (lx0, 11.32, -2.62), (lx1, 11.82, -2.32), "iron_lt")
        rx0, rx1 = min(sx * 4.05, sx * 4.35), max(sx * 4.05, sx * 4.35)
        g.box(b, f"buckle_frame_out_{side}", (rx0, 11.32, -2.62), (rx1, 11.82, -2.32), "iron_lt")
        mx0, mx1 = min(sx * 3.70, sx * 3.92), max(sx * 3.70, sx * 3.92)
        g.box(b, f"buckle_frame_mid_{side}", (mx0, 11.28, -2.58), (mx1, 11.88, -2.30), "rust")
        tx0, tx1 = min(sx * 3.68, sx * 3.94), max(sx * 3.68, sx * 3.94)
        g.box(b, f"buckle_pin_{side}", (tx0, 11.45, -2.82), (tx1, 11.75, -2.48), "rust")


def part_pouch_row1(g: Gear) -> None:
    """左外侧大药囊（#1）：弧面加厚生皮药袋、多层翻盖、角扣与粗缝线。"""
    b = "pouch_row1"
    # 位于最左侧：x∈[-3.85, -2.45]，y∈[14.45, 17.85]，z∈[-3.18, -2.36]
    # 前倾皮袋身与加固底托
    g.box(b, "pouch1_body", (-3.85, 14.55, -3.12), (-2.45, 17.65, -2.36), "hide")
    g.box(b, "pouch1_bot", (-3.78, 14.22, -3.05), (-2.52, 14.65, -2.42), "hide_dk")
    g.box(b, "pouch1_crest", (-3.72, 15.05, -3.24), (-2.58, 17.15, -2.95), "hide_lt")
    g.box(b, "pouch1_side_ridge", (-3.92, 14.82, -3.08), (-3.68, 17.38, -2.55), "hide_dk")
    g.box(b, "pouch1_pocket_lip", (-3.65, 15.42, -3.28), (-2.65, 15.82, -3.02), "hide_lt")
    g.box(b, "pouch1_patch", (-3.55, 16.52, -3.26), (-2.75, 17.12, -2.92), "cloth")
    g.box(b, "pouch1_patch_stitch", (-3.48, 16.72, -3.32), (-2.82, 16.98, -2.98), "cord_dk")

    # 翻盖
    g.box(b, "pouch1_flap_top", (-3.90, 17.35, -3.18), (-2.40, 18.05, -2.32), "hide_lt")
    g.box(b, "pouch1_flap_tongue", (-3.35, 15.85, -3.28), (-2.95, 17.45, -3.00), "hide_lt")

    # 骨栓扣与系绳（厚度 ≥ 0.25）
    g.box(b, "pouch1_bone_toggle", (-3.42, 16.15, -3.42), (-2.88, 16.55, -3.14), "bone")
    g.box(b, "pouch1_toggle_cord", (-3.25, 15.65, -3.36), (-3.05, 16.35, -3.05), "cord_dk")

    # 缝线
    g.box(b, "pouch1_stitch1", (-3.75, 14.85, -3.18), (-3.45, 15.25, -2.92), "cord")
    g.box(b, "pouch1_stitch2", (-2.85, 14.85, -3.18), (-2.55, 15.25, -2.92), "cord")


def part_pouch_row2(g: Gear) -> None:
    """左中方皮囊（#2）：方形储物皮仓、立柱骨栓、横向扎紧皮带。"""
    b = "pouch_row2"
    # 位于左中部：x∈[-2.35, -1.05]，y∈[14.55, 17.85]，z∈[-3.15, -2.36]
    g.box(b, "pouch2_body", (-2.35, 14.65, -3.08), (-1.05, 17.65, -2.36), "hide_dk")
    g.box(b, "pouch2_bot", (-2.28, 14.32, -3.00), (-1.12, 14.75, -2.42), "hide")
    g.box(b, "pouch2_flap", (-2.40, 17.25, -3.15), (-1.00, 17.95, -2.32), "hide")
    g.box(b, "pouch2_side_fold1", (-2.46, 15.15, -3.02), (-2.22, 17.15, -2.48), "hide_lt")
    g.box(b, "pouch2_side_fold2", (-1.18, 15.15, -3.02), (-0.94, 17.15, -2.48), "hide_lt")
    g.box(b, "pouch2_front_patch", (-2.15, 15.85, -3.20), (-1.25, 16.85, -2.92), "cloth_dk")

    # 垂直锁扣皮舌与长骨销
    g.box(b, "pouch2_strap", (-1.82, 15.65, -3.22), (-1.58, 17.45, -2.95), "strap_dk")
    g.box(b, "pouch2_toggle", (-1.95, 16.15, -3.38), (-1.45, 16.65, -3.10), "bone_lt")
    g.box(b, "pouch2_cord", (-1.85, 15.60, -3.32), (-1.55, 16.38, -3.00), "cord")
    # 底部横扎带与骨片角饰
    g.box(b, "pouch2_girth", (-2.42, 15.05, -3.15), (-0.98, 15.45, -2.92), "strap")
    g.box(b, "pouch2_corner_bone1", (-2.38, 14.55, -3.18), (-2.05, 14.95, -2.88), "bone_stain")
    g.box(b, "pouch2_corner_bone2", (-1.35, 14.55, -3.18), (-1.02, 14.95, -2.88), "bone_stain")
    g.box(b, "pouch2_rivet", (-1.82, 14.75, -3.25), (-1.58, 15.05, -2.95), "iron_lt")


def part_pouch_row3(g: Gear) -> None:
    """中央宽厚双格主囊（#3）：胸口正中大容积双联皮仓、双骨扣、皮搭扣。"""
    b = "pouch_row3"
    # 位于正中：x∈[-0.95, 0.95]，y∈[14.45, 17.95]，z∈[-3.25, -2.36]
    # 双格主仓（左半格与右半格）
    g.box(b, "pouch3_l_body", (-0.95, 14.65, -3.15), (-0.08, 17.65, -2.36), "hide")
    g.box(b, "pouch3_r_body", (0.08, 14.65, -3.15), (0.95, 17.65, -2.36), "hide")
    g.box(b, "pouch3_bot", (-0.88, 14.28, -3.08), (0.88, 14.72, -2.42), "hide_dk")

    # 双格翻盖
    g.box(b, "pouch3_l_flap", (-1.00, 17.25, -3.18), (-0.05, 17.95, -2.32), "hide_lt")
    g.box(b, "pouch3_r_flap", (0.05, 17.25, -3.18), (1.00, 17.95, -2.32), "hide_lt")

    # 双骨扣
    g.box(b, "pouch3_l_toggle", (-0.68, 16.15, -3.38), (-0.38, 16.65, -3.12), "bone_lt")
    g.box(b, "pouch3_r_toggle", (0.38, 16.15, -3.38), (0.68, 16.65, -3.12), "bone_lt")
    g.box(b, "pouch3_l_cord", (-0.65, 15.65, -3.32), (-0.42, 16.38, -3.02), "cord_dk")
    g.box(b, "pouch3_r_cord", (0.42, 15.65, -3.32), (0.65, 16.38, -3.02), "cord_dk")

    # 双格中央紧束皮封带
    g.box(b, "pouch3_center_iron", (-0.18, 14.55, -3.22), (0.18, 17.85, -2.95), "iron")
    g.box(b, "pouch3_center_rivet", (-0.12, 16.25, -3.42), (0.12, 16.55, -3.12), "iron_lt")
    g.box(b, "pouch3_l_ridge", (-0.85, 15.28, -3.25), (-0.25, 17.05, -2.98), "hide_lt")
    g.box(b, "pouch3_r_ridge", (0.25, 15.28, -3.25), (0.85, 17.05, -2.98), "hide_lt")
    g.box(b, "pouch3_bot_band", (-0.98, 14.85, -3.25), (0.98, 15.25, -2.88), "strap_dk")
    g.box(b, "pouch3_stitch_l1", (-0.75, 14.45, -3.18), (-0.45, 14.85, -2.92), "cord")
    g.box(b, "pouch3_stitch_r1", (0.45, 14.45, -3.18), (0.75, 14.85, -2.92), "cord")
    g.box(b, "pouch3_mid_tie", (-0.55, 16.85, -3.32), (0.55, 17.15, -2.92), "cord_dk")


def part_pouch_row4(g: Gear) -> None:
    """右中多插孔工具袋（#4）：带开孔皮护套、三插槽、骨针插件。"""
    b = "pouch_row4"
    # 位于右中部：x∈[1.05, 2.35]，y∈[14.55, 17.85]，z∈[-3.15, -2.36]
    g.box(b, "pouch4_body", (1.05, 14.65, -3.08), (2.35, 17.65, -2.36), "hide_dk")
    g.box(b, "pouch4_bot", (1.12, 14.32, -3.00), (2.28, 14.75, -2.42), "hide")
    g.box(b, "pouch4_rim", (1.00, 17.35, -3.15), (2.40, 17.82, -2.32), "hide_lt")
    g.box(b, "pouch4_pocket_b", (1.15, 14.62, -3.25), (2.25, 15.15, -2.95), "hide_dk")
    g.box(b, "pouch4_side_lace", (2.25, 15.25, -3.18), (2.48, 16.85, -2.85), "cord")

    # 外侧三孔皮革工具插槽
    g.box(b, "pouch4_slot_main", (1.18, 15.05, -3.22), (2.22, 17.15, -2.96), "hide")
    g.box(b, "pouch4_slot_div1", (1.42, 15.00, -3.30), (1.64, 17.20, -2.92), "strap_dk")
    g.box(b, "pouch4_slot_div2", (1.76, 15.00, -3.30), (1.98, 17.20, -2.92), "strap_dk")

    # 插孔内露出的三根磨尖兽骨针/锥（宽度 ≥ 0.22）
    g.box(b, "pouch4_bone1", (1.20, 16.85, -3.18), (1.44, 18.55, -2.88), "bone_lt")
    g.box(b, "pouch4_bone2", (1.58, 16.85, -3.18), (1.82, 18.35, -2.88), "bone")
    g.box(b, "pouch4_bone3", (1.94, 16.85, -3.18), (2.18, 18.15, -2.88), "bone_stain")


def part_pouch_row5(g: Gear) -> None:
    """右外侧扎口皮袋（#5）：束口式柔皮软囊、环绕皮绳结与皮盖。"""
    b = "pouch_row5"
    # 位于最右侧：x∈[2.45, 3.85]，y∈[14.45, 17.85]，z∈[-3.18, -2.36]
    g.box(b, "pouch5_body", (2.45, 14.55, -3.12), (3.85, 17.65, -2.36), "hide")
    g.box(b, "pouch5_bot", (2.52, 14.22, -3.05), (3.78, 14.65, -2.42), "hide_dk")
    g.box(b, "pouch5_crest", (2.58, 15.05, -3.24), (3.72, 17.15, -2.95), "hide_lt")
    g.box(b, "pouch5_side_ridge", (3.68, 14.82, -3.08), (3.92, 17.38, -2.55), "hide_dk")
    g.box(b, "pouch5_pocket_lip", (2.65, 15.42, -3.28), (3.65, 15.82, -3.02), "hide_lt")
    g.box(b, "pouch5_patch", (2.75, 16.52, -3.26), (3.55, 17.12, -2.92), "cloth")
    g.box(b, "pouch5_patch_stitch", (2.82, 16.72, -3.36), (3.48, 16.98, -3.02), "cord_dk")

    # 束口环扎绳与垂落绳头
    g.box(b, "pouch5_neck_band", (2.40, 16.85, -3.18), (3.90, 17.35, -2.85), "cord_dk")
    g.box(b, "pouch5_neck_knot", (3.05, 16.75, -3.32), (3.35, 17.45, -2.98), "cord")
    g.box(b, "pouch5_neck_tail", (3.08, 15.85, -3.28), (3.32, 16.85, -3.00), "cord_dk")

    # 囊顶褶边
    g.box(b, "pouch5_lip_top", (2.42, 17.35, -3.15), (3.78, 17.95, -2.32), "hide_lt")
    g.box(b, "pouch5_corner_bone1", (2.48, 14.48, -3.18), (2.82, 14.95, -2.88), "bone_stain")
    g.box(b, "pouch5_corner_bone2", (3.48, 14.48, -3.18), (3.82, 14.95, -2.88), "bone_stain")


def part_jar_l(g: Gear) -> None:
    """左侧小陶药瓶：圆肚短颈粗陶罐、带孔兽骨塞、麻绳环扣（独立骨）。"""
    b = "jar_l"
    # 悬挂于最左侧皮囊下方外侧（x ≈ -3.35，y∈[11.5, 14.8]，z∈[-3.05, -2.35]）
    g.box(b, "jar_l_cord_top", (-3.48, 14.25, -2.75), (-3.12, 14.85, -2.45), "cord_dk")
    g.box(b, "jar_l_knot", (-3.55, 13.95, -2.82), (-3.05, 14.35, -2.38), "cord")

    # 兽骨瓶塞（带孔）
    g.box(b, "jar_l_stopper", (-3.48, 13.55, -2.75), (-3.12, 14.05, -2.45), "bone_lt")
    g.box(b, "jar_l_stopper_void", (-3.42, 13.78, -2.72), (-3.18, 14.00, -2.48), "void")

    # 粗陶药瓶（圆鼓肚）
    g.box(b, "jar_l_neck", (-3.55, 13.25, -2.82), (-3.05, 13.65, -2.38), "clay_dk")
    g.box(b, "jar_l_belly", (-3.75, 11.95, -2.98), (-2.85, 13.28, -2.22), "clay")
    g.box(b, "jar_l_belly_ridge1", (-3.82, 12.35, -2.92), (-2.78, 12.85, -2.28), "clay_dk")
    g.box(b, "jar_l_base", (-3.62, 11.45, -2.96), (-2.98, 12.07, -2.34), "clay_dk")
    g.box(b, "jar_l_cord_ring", (-3.62, 13.15, -2.88), (-2.98, 13.48, -2.32), "cord_dk")
    g.box(b, "jar_l_dangle1", (-3.58, 12.45, -2.85), (-3.32, 13.18, -2.55), "cord")
    g.box(b, "jar_l_dangle2", (-3.55, 11.85, -2.80), (-3.28, 12.55, -2.50), "cord_dk")
    # 增加小皮护套
    g.box(b, "jar_l_leather_cage", (-3.78, 11.75, -2.95), (-2.82, 12.35, -2.25), "hide_dk")
    g.box(b, "jar_l_leather_strap", (-3.65, 12.25, -3.02), (-2.95, 12.82, -2.18), "strap")


def part_jar_m(g: Gear) -> None:
    """中央扁壶小彩陶瓶：扁腹彩绘陶水罐、软木塞、下垂双股麻绳（独立骨）。"""
    b = "jar_m"
    # 悬挂在中央偏左下方（x ≈ -0.75，y∈[11.6, 14.8]，z∈[-3.12, -2.38]）
    # 悬挂皮扣
    g.box(b, "jar_m_strap", (-0.92, 14.25, -2.82), (-0.58, 14.85, -2.52), "strap")
    g.box(b, "jar_m_knot", (-0.98, 13.95, -2.92), (-0.52, 14.35, -2.46), "cord")

    # 软木塞与小骨塞盖
    g.box(b, "jar_m_plug", (-0.90, 13.58, -2.84), (-0.60, 14.05, -2.52), "hide_dk")
    g.box(b, "jar_m_bone_cap", (-0.95, 13.88, -2.86), (-0.55, 14.15, -2.48), "bone_lt")

    # 扁壶陶瓶身（彩陶黑褐纹带）
    g.box(b, "jar_m_neck", (-0.95, 13.25, -2.85), (-0.55, 13.65, -2.48), "clay_dk")
    g.box(b, "jar_m_belly", (-1.25, 11.95, -3.02), (-0.25, 13.28, -2.32), "clay")
    g.box(b, "jar_m_stripe", (-1.28, 12.45, -3.05), (-0.22, 12.78, -2.28), "clay_dk")
    g.box(b, "jar_m_stripe2", (-1.18, 12.05, -2.98), (-0.32, 12.35, -2.35), "clay_dk")
    g.box(b, "jar_m_base", (-1.12, 11.55, -2.92), (-0.38, 12.05, -2.42), "clay_dk")
    g.box(b, "jar_m_ring", (-1.02, 13.15, -2.88), (-0.48, 13.48, -2.42), "cord_dk")
    # 下垂双股麻绳与挂坠小骨珠（厚度 ≥ 0.22）
    g.box(b, "jar_m_dangle1", (-0.90, 10.85, -2.88), (-0.66, 11.85, -2.48), "cord")
    g.box(b, "jar_m_dangle2", (-0.68, 10.65, -2.86), (-0.44, 11.75, -2.52), "cord_dk")
    g.box(b, "jar_m_bead", (-0.82, 11.05, -2.95), (-0.55, 11.55, -2.42), "bone_lt")


def part_jar_r(g: Gear) -> None:
    """右侧细长陶丹瓶：细颈广口彩陶瓶、兽皮封口软塞、扎紧绳圈（独立骨）。"""
    b = "jar_r"
    # 悬挂于最右侧皮套下方外侧（x ≈ 3.35，y∈[11.5, 14.8]，z∈[-3.05, -2.35]）
    g.box(b, "jar_r_cord_top", (3.12, 14.25, -2.75), (3.48, 14.85, -2.45), "cord")
    g.box(b, "jar_r_knot", (3.05, 13.95, -2.82), (3.55, 14.35, -2.38), "cord_dk")

    # 兽皮包扎封口
    g.box(b, "jar_r_plug", (3.10, 13.65, -2.78), (3.50, 14.15, -2.42), "hide_lt")
    g.box(b, "jar_r_neck", (3.05, 13.25, -2.82), (3.55, 13.75, -2.38), "clay_dk")
    # 六角长瓶身
    g.box(b, "jar_r_belly", (2.85, 11.95, -2.98), (3.75, 13.28, -2.22), "clay")
    g.box(b, "jar_r_base", (2.98, 11.45, -2.96), (3.62, 12.07, -2.34), "clay_dk")
    # 陶瓶饰带
    g.box(b, "jar_r_band1", (2.82, 12.45, -3.02), (3.78, 12.85, -2.18), "clay_dk")
    g.box(b, "jar_r_band2", (2.82, 12.02, -3.02), (3.78, 12.35, -2.18), "clay_dk")
    g.box(b, "jar_r_net1", (2.75, 12.15, -2.95), (3.85, 12.45, -2.25), "cord")
    g.box(b, "jar_r_net2", (2.75, 12.65, -2.95), (3.85, 12.95, -2.25), "cord_dk")
    g.box(b, "jar_r_dangle", (3.12, 11.15, -2.80), (3.45, 11.85, -2.40), "cord")
    g.box(b, "jar_r_cord_ring", (2.95, 13.15, -2.88), (3.65, 13.48, -2.32), "cord")
    # 右侧陶瓶加挂小饰物（错开共面）
    g.box(b, "jar_r_tag_bone", (3.08, 10.65, -2.88), (3.48, 11.25, -2.38), "bone")
    g.box(b, "jar_r_tag_tip", (3.15, 10.15, -2.85), (3.42, 10.75, -2.40), "bone_lt")
    g.box(b, "jar_r_tag_knot", (3.15, 11.18, -2.84), (3.42, 11.48, -2.36), "cord_dk")
    g.box(b, "jar_r_rim_fold", (2.85, 13.52, -2.90), (3.75, 13.82, -2.25), "hide_dk")


def part_hang_knife(g: Gear) -> None:
    """胸板下方横插骨锥：横置打孔硬皮鞘、小骨锥柄与锋利骨尖（独立骨）。"""
    b = "hang_knife"
    # 横置于胸板底缘下方（x∈[-2.45, 2.45]，y∈[13.45, 14.45]，z∈[-2.78, -2.42]）
    # 皮鞘
    g.box(b, "sheath_body", (-2.25, 13.55, -2.75), (0.85, 14.45, -2.45), "hide_dk")
    g.box(b, "sheath_mouth", (0.55, 13.45, -2.82), (0.95, 14.55, -2.38), "strap_dk")
    # 鞘身金属包角与铆钉
    # 鞘身金属包角与铆钉
    g.box(b, "sheath_tip_iron", (-2.45, 13.52, -2.82), (-2.15, 14.48, -2.36), "iron")
    g.box(b, "sheath_rivet", (-2.05, 13.72, -2.90), (-1.82, 14.12, -2.40), "iron_lt")
    g.box(b, "sheath_band1", (-1.25, 13.48, -2.84), (-0.95, 14.52, -2.36), "strap")
    g.box(b, "sheath_band2", (-0.15, 13.48, -2.84), (0.15, 14.52, -2.36), "strap")
    g.box(b, "sheath_ridge", (-2.12, 13.82, -2.76), (0.70, 14.10, -2.42), "hide_lt")

    # 皮鞘固定双扣环（挂在主横带底唇上）
    g.box(b, "sheath_loop1", (-1.85, 14.25, -2.85), (-1.45, 15.45, -2.38), "cord_dk")
    g.box(b, "sheath_loop2", (-0.35, 14.25, -2.85), (0.05, 15.45, -2.38), "cord_dk")

    # 拔出一半的小骨锥
    g.box(b, "bone_knife_hilt", (0.85, 13.65, -2.68), (1.95, 14.35, -2.48), "bone")
    g.box(b, "bone_knife_pommel", (1.85, 13.55, -2.72), (2.35, 14.45, -2.44), "bone_lt")
    g.box(b, "bone_knife_pommel_ring", (2.25, 13.72, -2.76), (2.55, 14.28, -2.40), "iron_lt")
    g.box(b, "bone_knife_guard", (0.72, 13.42, -2.78), (0.98, 14.58, -2.40), "bone_stain")
    # 锥柄缠绳防滑
    g.box(b, "bone_knife_wrap", (1.15, 13.58, -2.75), (1.65, 14.42, -2.41), "cord")
    g.box(b, "bone_knife_knot", (1.32, 13.48, -2.82), (1.58, 14.52, -2.38), "cord_dk")


def part_hang_waist_tails(g: Gear) -> None:
    """腰带垂落皮舌与骨饰：穿出带扣自然下垂的双侧打孔皮带头与兽骨坠（独立骨）。"""
    b = "hang_waist_tails"
    # 右腰垂舌
    g.box(b, "waist_tail_r_top", (3.48, 10.35, -2.52), (3.98, 11.25, -2.25), "strap")
    g.box(b, "waist_tail_r_bot", (3.42, 9.15, -2.56), (3.92, 10.45, -2.28), "strap_dk")
    g.box(b, "waist_tail_r_tip", (3.48, 8.45, -2.52), (3.88, 9.25, -2.25), "strap")
    g.box(b, "waist_tail_r_hole", (3.56, 9.65, -2.62), (3.82, 9.98, -2.22), "void")
    g.box(b, "waist_tail_r_ring", (3.45, 8.75, -2.58), (3.92, 9.05, -2.22), "iron_lt")
    # 右侧垂绳与小骨坠
    g.box(b, "waist_tail_r_cord", (3.54, 7.85, -2.50), (3.82, 8.55, -2.22), "cord")
    g.box(b, "waist_tail_r_bone", (3.45, 6.75, -2.55), (3.92, 7.95, -2.18), "bone_lt")
    g.box(b, "waist_tail_r_knot", (3.52, 7.65, -2.58), (3.85, 8.05, -2.16), "cord_dk")

    # 左腰垂舌
    g.box(b, "waist_tail_l_top", (-3.98, 10.35, -2.52), (-3.48, 11.25, -2.25), "strap")
    g.box(b, "waist_tail_l_bot", (-3.92, 9.45, -2.56), (-3.42, 10.45, -2.28), "strap_dk")
    g.box(b, "waist_tail_l_tip", (-3.88, 8.85, -2.52), (-3.48, 9.55, -2.25), "strap")
    g.box(b, "waist_tail_l_hole", (-3.82, 9.85, -2.62), (-3.56, 10.18, -2.22), "void")
    g.box(b, "waist_tail_l_bone", (-3.85, 7.85, -2.55), (-3.52, 8.85, -2.18), "bone")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "harness_front": part_harness_front,
    "harness_back_x": part_harness_back_x,
    "harness_waist": part_harness_waist,
    "pouch_row1": part_pouch_row1,
    "pouch_row2": part_pouch_row2,
    "pouch_row3": part_pouch_row3,
    "pouch_row4": part_pouch_row4,
    "pouch_row5": part_pouch_row5,
    "jar_l": part_jar_l,
    "jar_m": part_jar_m,
    "jar_r": part_jar_r,
    "hang_knife": part_hang_knife,
    "hang_waist_tails": part_hang_waist_tails,
}

SPEC = GearSpec(
    gear_id="pi_nang_xiong_gua",
    model_name="PiNangXiongGua",
    title="皮囊胸挂 / pi_nang_xiong_gua",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：胸前 · 横贯胸前整排五联皮囊+三挂陶瓶+X形立体背跨带+骨锥鞘",
    textured=False,            # Round 1 纯几何
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
