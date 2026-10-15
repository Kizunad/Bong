#!/usr/bin/env python3
"""皮囊胸挂（pi_nang_xiong_gua）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #19（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形（参考图 pi_nang_xiong_gua_ref.png / pi_nang_xiong_gua_exploded.png）：
胸前一排小皮囊和小陶瓶的挂带，X 形背带交叉到背后。
- 前胸挂载平台位于 y∈[14.2, 21.5]，z ≤ -2.10（紧贴躯干前壁但绝不穿入）。
- 肩带从胸前上沿延伸，骑越肩峰外侧（|x| ≥ 4.10，y ≥ 24.10，避开头盒与手臂盒），延伸至背后。
- 背后背带呈大幅度 X 形交叉：从左右肩斜向交叉至对侧后腰，交叉中心设有菱形牛皮加固垫与固定铆钉。
- 侧腰环带位于 y∈[11.25, 11.90]（低于双臂盒 y=12.0），连接前后胸腹带，左右配粗生铁带扣。
- 胸前挂载丰富实用求生构件：
  1. 左侧粗缝兽皮药袋（带翻盖、骨扣）
  2. 中央双联小方皮囊（主储物仓、双骨栓）
  3. 右侧工具皮套（插孔、粗针脚）
  4. 左侧悬挂的小陶药瓶（束口肚陶瓶、骨塞、草绳环扣，独立骨）
  5. 右侧悬挂的小陶丹瓶（粗陶罐、软木/兽皮塞、绑扎绳，独立骨）
  6. 胸板下方横插的骨锥皮鞘与小骨刀（挂件独立骨）
  7. 左右腰侧调节带垂落的皮带尾与骨饰（挂件独立骨）

骨骼数 ≥ 10，挂件独立骨，全绿门禁（含 clearance、coplanar、self-test）。
用法：
    python3 modelScript/gear/pi_nang_xiong_gua/gen_pi_nang_xiong_gua.py --self-test
    python3 modelScript/gear/pi_nang_xiong_gua/gen_pi_nang_xiong_gua.py --gates
    python3 modelScript/gear/pi_nang_xiong_gua/gen_pi_nang_xiong_gua.py
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
    "strap":      (112, 72, 50),      # X背带
    "strap_dk":   (78, 48, 36),       # 深色背带
    "clay":       (160, 94, 62),      # 陶瓶陶色
    "clay_dk":    (112, 60, 40),      # 粗陶阴影/断茬
    "cord":       (144, 128, 78),     # 草绳绑扎
    "cord_dk":    (102, 88, 56),      # 深色麻绳
    "bone":       (214, 204, 176),    # 骨栓/骨塞
    "bone_lt":    (236, 226, 200),    # 骨塞头/骨锥尖
    "bone_stain": (132, 112, 84),     # 老骨斑
    "iron":       (82, 78, 76),       # 粗锻生铁扣
    "iron_lt":    (116, 112, 110),    # 铁光泽
    "rust":       (152, 86, 52),      # 铁锈斑
    "void":       (28, 24, 22),       # 穿孔暗影
}

# 软件材质与硬物允许贴合咬合
SOFT = ("hide", "hide_dk", "hide_lt", "strap", "strap_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("bone", "bone_lt", "bone_stain", "clay", "clay_dk", "iron", "hide_dk"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("harness_front", "root", (0.0, 18.0, -2.25), "胸前主承载带板：双层横向承重皮带、上吊耳、前胸基盘"),
    ("harness_back_x", "root", (0.0, 18.0, 2.25), "背后 X 形交叉背带：双向斜跨背带、中心交叉菱形牛皮垫、定位铆钉"),
    ("harness_waist", "root", (0.0, 11.6, 0.0), "左右环腰连接带：低位过侧腰皮带、粗铁日字带扣、咬合舌片"),
    ("pouch_l", "harness_front", (-2.6, 16.5, -2.6), "左侧皮药囊：翻盖粗缝皮袋、小骨扣、下缘缝线"),
    ("pouch_c", "harness_front", (0.0, 16.5, -2.65), "中央双联小方皮囊：并排两格储物皮仓、双骨栓、束紧横带"),
    ("pouch_r", "harness_front", (2.6, 16.5, -2.6), "右侧工具皮套：插孔皮护袋、侧边插槽、粗针脚"),
    ("jar_l", "harness_front", (-3.1, 14.8, -2.7), "左侧悬挂小陶药瓶：圆肚短颈粗陶罐、带孔兽骨塞、麻绳环扣（独立骨）"),
    ("jar_r", "harness_front", (3.1, 14.8, -2.7), "右侧悬挂小陶丹瓶：细颈广口彩陶瓶、兽皮封口软塞、扎紧绳圈（独立骨）"),
    ("hang_knife", "harness_front", (0.0, 13.8, -2.55), "胸板下方横插骨锥：横置打孔硬皮鞘、小骨锥柄与锋利骨尖（独立骨）"),
    ("hang_waist_tails", "harness_waist", (4.25, 10.0, 0.0), "腰带垂落皮舌与骨饰：穿出带扣自然下垂的双侧打孔皮带头与兽骨坠（独立骨）"),
]

# 不对称骨骼（X背带、各口袋、挂瓶、挂刀、垂尾）
ASYM_BONES = frozenset({"harness_back_x", "pouch_l", "pouch_c", "pouch_r", "jar_l", "jar_r", "hang_knife", "hang_waist_tails"})


# ───────────────────────────────────────────────────────────── 几何部件构建
def part_harness_front(g: Gear) -> None:
    """胸前主承载带板：双层横向承重皮带、上吊耳、前胸基盘。"""
    b = "harness_front"
    # 胸前主横承重带（跨度 x∈[-3.80, 3.80]，y∈[16.20, 17.65]，z∈[-2.36, -2.08]）
    g.box(b, "f_band_main", (-3.80, 16.20, -2.36), (3.80, 17.65, -2.08), "strap")
    g.box(b, "f_band_top_lip", (-3.70, 17.65, -2.42), (3.70, 18.05, -2.12), "strap_dk")
    g.box(b, "f_band_bot_lip", (-3.70, 15.80, -2.42), (3.70, 16.20, -2.12), "strap_dk")

    # 胸前中线加强固定皮方块（中线对称，Y 和 Z 错开）
    g.box(b, "f_center_boss", (-0.95, 15.75, -2.50), (0.95, 18.12, -2.16), "hide_dk")
    g.box(b, "f_center_rivet_t", (-0.18, 17.72, -2.65), (0.18, 18.02, -2.35), "iron_lt")
    g.box(b, "f_center_rivet_b", (-0.18, 15.85, -2.65), (0.18, 16.15, -2.35), "iron_lt")

    # 两侧悬挂转接吊耳（向上延伸接肩带，x ≈ ±(2.80..3.80)）
    # 左右成对上升带（从 y=18.05 上升到 y=23.85，避开头部）
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        x0, x1 = min(sx * 2.85, sx * 3.75), max(sx * 2.85, sx * 3.75)
        g.box(b, f"f_suspender_{side}", (x0, 18.05, -2.36), (x1, 23.15, -2.08), "strap")
        # 前肩斜下转接段（从 y=23.15 上升至 y=23.85，x 向外微展至 4.12）
        jx0, jx1 = min(sx * 3.45, sx * 4.12), max(sx * 3.45, sx * 4.12)
        g.box(b, f"f_shoulder_jog_{side}", (jx0, 23.15, -2.38), (jx1, 23.85, -2.08), "strap_dk")
        # 肩部前侧立升段（x ≥ 4.10，避开头部）
        rx0, rx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"f_shoulder_riser_{side}", (rx0, 23.80, -2.42), (rx1, 24.45, -2.10), "strap")
        # 肩峰骑越段（|x| ≥ 4.12，y ≥ 24.10 避开手臂与头盒）
        ox0, ox1 = min(sx * 4.12, sx * 4.52), max(sx * 4.12, sx * 4.52)
        g.box(b, f"f_over_shoulder_{side}", (ox0, 24.10, -2.15), (ox1, 24.48, 2.15), "strap_dk")
        # 肩垫护皮（缝在肩带上方）
        px0, px1 = min(sx * 4.16, sx * 4.56), max(sx * 4.16, sx * 4.56)
        g.box(b, f"f_shoulder_pad_{side}", (px0, 24.48, -1.65), (px1, 24.88, 1.65), "hide_lt")
        # 前胸背带上的调节扣片
        kx0, kx1 = min(sx * 2.80, sx * 3.80), max(sx * 2.80, sx * 3.80)
        g.box(b, f"f_buckle_slide_{side}", (kx0, 20.85, -2.52), (kx1, 21.45, -2.18), "iron_lt")
        g.box(b, f"f_slide_pin_{side}", (kx0 + 0.20, 21.02, -2.68), (kx1 - 0.20, 21.28, -2.40), "rust")


def part_harness_back_x(g: Gear) -> None:
    """背后 X 形交叉背带：双向斜跨背带、中心交叉菱形牛皮垫、定位铆钉。"""
    b = "harness_back_x"
    # 背部后表面位于 z = 2.0，所有背后部件必须严格位于 z ≥ 2.10！
    # 左右肩后下沉段（x ≥ 4.10，y∈[23.80, 24.45]，z ≥ 2.10）
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        rx0, rx1 = min(sx * 4.10, sx * 4.48), max(sx * 4.10, sx * 4.48)
        g.box(b, f"b_shoulder_riser_{side}", (rx0, 23.80, 2.08), (rx1, 24.45, 2.40), "strap")
        jx0, jx1 = min(sx * 3.45, sx * 4.30), max(sx * 3.45, sx * 4.30)
        g.box(b, f"b_shoulder_jog_{side}", (jx0, 23.15, 2.12), (jx1, 23.85, 2.42), "strap_dk")

    # X 形两条交叉斜跨带（内层带错开 Z，外层带错开 Z）
    # 第一条（内层）：从左肩斜下至右腰
    g.box(b, "b_x1_seg1", (-3.75, 20.65, 2.10), (-2.05, 23.25, 2.34), "strap")
    g.box(b, "b_x1_seg2", (-2.25, 17.85, 2.12), (-0.55, 20.85, 2.36), "strap_dk")
    g.box(b, "b_x1_seg3", (-0.75, 15.05, 2.10), (0.95, 18.05, 2.34), "strap")
    g.box(b, "b_x1_seg4", (0.75, 12.25, 2.12), (2.45, 15.25, 2.36), "strap_dk")
    g.box(b, "b_x1_seg5", (2.25, 11.25, 2.10), (3.75, 12.65, 2.34), "strap")

    # 第二条（外层）：从右肩斜下至左腰
    g.box(b, "b_x2_seg1", (2.05, 20.65, 2.38), (3.75, 23.25, 2.62), "strap")
    g.box(b, "b_x2_seg2", (0.55, 17.85, 2.40), (2.25, 20.85, 2.64), "strap_dk")
    g.box(b, "b_x2_seg3", (-0.95, 15.05, 2.38), (0.75, 18.05, 2.62), "strap")
    g.box(b, "b_x2_seg4", (-2.45, 12.25, 2.40), (-0.75, 15.25, 2.64), "strap_dk")
    g.box(b, "b_x2_seg5", (-3.75, 11.25, 2.38), (-2.25, 12.65, 2.62), "strap")

    # 中心交叉处的菱形厚牛皮加固垫（压在两条外侧，z∈[2.62, 2.88]）
    g.box(b, "b_cross_pad", (-1.25, 16.85, 2.62), (1.25, 19.15, 2.88), "hide_dk")
    g.box(b, "b_cross_plate", (-0.85, 17.20, 2.88), (0.85, 18.80, 3.14), "hide_lt")
    g.box(b, "b_cross_rivet_t", (-0.18, 18.42, 3.14), (0.18, 18.72, 3.40), "iron_lt")
    g.box(b, "b_cross_rivet_b", (-0.18, 17.28, 3.14), (0.18, 17.58, 3.40), "iron_lt")


def part_harness_waist(g: Gear) -> None:
    """左右环腰连接带：低位过侧腰皮带、粗铁日字带扣、咬合舌片。"""
    b = "harness_waist"
    # 低位于 y∈[11.25, 11.90]（低于双臂盒 y=12.0），x 位于 ±(4.10..4.38)
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        wx0, wx1 = min(sx * 4.10, sx * 4.38), max(sx * 4.10, sx * 4.38)
        # 侧腰横带（连接前后背带）
        g.box(b, f"waist_strap_{side}", (wx0, 11.25, -2.15), (wx1, 11.90, 2.15), "strap")
        px0, px1 = min(sx * 4.12, sx * 4.45), max(sx * 4.12, sx * 4.45)
        g.box(b, f"waist_pad_{side}", (px0, 11.35, -1.65), (px1, 11.80, 1.65), "strap_dk")

        # 腰间生铁日字带扣（位于 x ≈ ±(3.20..4.35), y∈[11.02, 12.12], z∈[-2.62, -2.32]）
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


def part_pouch_l(g: Gear) -> None:
    """左侧皮药囊：翻盖粗缝皮袋、小骨扣、下缘缝线。"""
    b = "pouch_l"
    # 左侧皮囊挂在 x∈[-3.65, -1.85]，y∈[14.65, 17.85]，z∈[-3.15, -2.36]
    # 主袋身（深皮饱满鼓肚）
    g.box(b, "pouch_l_body", (-3.65, 14.75, -3.05), (-1.85, 17.65, -2.36), "hide")
    g.box(b, "pouch_l_bot", (-3.55, 14.35, -2.95), (-1.95, 14.75, -2.42), "hide_dk")
    # 翻盖（浅色生皮，前凸搭扣）
    g.box(b, "pouch_l_flap_top", (-3.72, 17.25, -3.12), (-1.78, 17.95, -2.32), "hide_lt")
    g.box(b, "pouch_l_flap_tongue", (-3.05, 15.85, -3.20), (-2.45, 17.25, -2.95), "hide_lt")
    # 骨栓扣（打孔别在盖舌上的小兽骨）
    g.box(b, "pouch_l_bone_toggle", (-2.95, 16.15, -3.38), (-2.55, 16.55, -3.10), "bone")
    g.box(b, "pouch_l_toggle_cord", (-2.85, 15.65, -3.34), (-2.65, 16.35, -3.08), "cord_dk")
    # 边缘粗针脚
    g.box(b, "pouch_l_stitch1", (-3.55, 14.95, -3.18), (-3.25, 15.35, -2.90), "cord")
    g.box(b, "pouch_l_stitch2", (-2.25, 14.95, -3.18), (-1.95, 15.35, -2.90), "cord")


def part_pouch_c(g: Gear) -> None:
    """中央双联小方皮囊：并排两格储物皮仓、双骨栓、束紧横带。"""
    b = "pouch_c"
    # 中央双格皮袋：x∈[-1.55, 1.55]，y∈[14.55, 17.85]，z∈[-3.15, -2.36]
    # 左格
    g.box(b, "pouch_c1_body", (-1.55, 14.65, -3.05), (-0.15, 17.55, -2.36), "hide_dk")
    g.box(b, "pouch_c1_flap", (-1.60, 17.15, -3.15), (-0.10, 17.85, -2.32), "hide")
    g.box(b, "pouch_c1_toggle", (-1.02, 16.15, -3.36), (-0.68, 16.65, -3.08), "bone_lt")
    g.box(b, "pouch_c1_cord", (-0.98, 15.65, -3.28), (-0.72, 16.38, -3.00), "cord_dk")

    # 右格
    g.box(b, "pouch_c2_body", (0.15, 14.65, -3.05), (1.55, 17.55, -2.36), "hide_dk")
    g.box(b, "pouch_c2_flap", (0.10, 17.15, -3.15), (1.60, 17.85, -2.32), "hide")
    g.box(b, "pouch_c2_toggle", (0.68, 16.15, -3.36), (1.02, 16.65, -3.08), "bone_lt")
    g.box(b, "pouch_c2_cord", (0.72, 15.65, -3.28), (0.98, 16.38, -3.00), "cord_dk")

    # 双格中央紧束皮封带
    g.box(b, "pouch_c_band", (-1.65, 15.82, -3.12), (1.65, 16.32, -2.85), "strap_dk")


def part_pouch_r(g: Gear) -> None:
    """右侧工具皮套：插孔皮护袋、侧边插槽、粗针脚。"""
    b = "pouch_r"
    # 右侧挂袋：x∈[1.85, 3.65]，y∈[14.65, 17.85]，z∈[-3.05, -2.36]
    g.box(b, "pouch_r_body", (1.85, 14.75, -3.05), (3.65, 17.65, -2.36), "hide")
    g.box(b, "pouch_r_bot", (1.95, 14.35, -2.95), (3.55, 14.75, -2.42), "hide_dk")
    # 顶部开口卷边与侧边加固套
    g.box(b, "pouch_r_rim", (1.80, 17.35, -3.12), (3.68, 17.85, -2.32), "hide_lt")
    g.box(b, "pouch_r_slot", (2.15, 15.25, -3.20), (3.35, 17.15, -2.95), "hide_dk")
    # 插槽内露出的两根兽骨签/锥
    g.box(b, "bone_pick1", (2.35, 16.85, -3.15), (2.65, 18.45, -2.85), "bone_lt")
    g.box(b, "bone_pick2", (2.85, 16.85, -3.15), (3.15, 18.25, -2.85), "bone")


def part_jar_l(g: Gear) -> None:
    """左侧悬挂小陶药瓶：圆肚短颈粗陶罐、带孔兽骨塞、麻绳环扣（独立骨）。"""
    b = "jar_l"
    # 悬挂于左侧皮囊下方外侧（x ≈ -3.15，y∈[11.5, 14.8]，z∈[-3.05, -2.35]）
    # 悬挂麻绳与吊扣
    g.box(b, "jar_l_cord_top", (-3.28, 14.25, -2.75), (-2.92, 14.85, -2.45), "cord_dk")
    g.box(b, "jar_l_knot", (-3.35, 13.95, -2.82), (-2.85, 14.35, -2.38), "cord")

    # 兽骨瓶塞（带孔）
    g.box(b, "jar_l_stopper", (-3.28, 13.55, -2.75), (-2.92, 14.05, -2.45), "bone_lt")
    g.box(b, "jar_l_stopper_void", (-3.22, 13.78, -2.72), (-2.95, 14.00, -2.46), "void")

    # 粗陶药瓶（束颈、圆鼓肚）
    g.box(b, "jar_l_neck", (-3.35, 13.25, -2.82), (-2.85, 13.65, -2.38), "clay_dk")
    g.box(b, "jar_l_belly", (-3.55, 11.95, -2.98), (-2.65, 13.28, -2.22), "clay")
    g.box(b, "jar_l_base", (-3.42, 11.45, -2.96), (-2.78, 12.07, -2.34), "clay_dk")
    # 瓶颈捆扎草绳圈
    g.box(b, "jar_l_cord_ring", (-3.42, 13.15, -2.88), (-2.78, 13.48, -2.32), "cord_dk")
    # 瓶身垂落的小麻绳结与绳头
    g.box(b, "jar_l_dangle1", (-3.48, 12.45, -2.85), (-3.22, 13.18, -2.55), "cord")
    g.box(b, "jar_l_dangle2", (-3.45, 11.85, -2.80), (-3.18, 12.55, -2.50), "cord_dk")


def part_jar_r(g: Gear) -> None:
    """右侧悬挂小陶丹瓶：细颈广口彩陶瓶、兽皮封口软塞、扎紧绳圈（独立骨）。"""
    b = "jar_r"
    # 悬挂于右侧皮套下方外侧（x ≈ 3.15，y∈[11.5, 14.8]，z∈[-3.05, -2.35]）
    g.box(b, "jar_r_cord_top", (2.92, 14.25, -2.75), (3.28, 14.85, -2.45), "cord")
    g.box(b, "jar_r_knot", (2.85, 13.95, -2.82), (3.35, 14.35, -2.38), "cord_dk")

    # 兽皮包扎封口
    g.box(b, "jar_r_plug", (2.90, 13.65, -2.78), (3.30, 14.15, -2.42), "hide_lt")
    g.box(b, "jar_r_neck", (2.85, 13.25, -2.82), (3.35, 13.75, -2.38), "clay_dk")
    # 六角长瓶身
    g.box(b, "jar_r_belly", (2.65, 11.95, -2.98), (3.55, 13.28, -2.22), "clay")
    g.box(b, "jar_r_base", (2.78, 11.45, -2.96), (3.42, 12.07, -2.34), "clay_dk")
    # 陶瓶红黑相间饰带
    g.box(b, "jar_r_band", (2.62, 12.45, -3.02), (3.58, 12.85, -2.18), "clay_dk")


def part_hang_knife(g: Gear) -> None:
    """胸板下方横插骨锥：横置打孔硬皮鞘、小骨锥柄与锋利骨尖（独立骨）。"""
    b = "hang_knife"
    # 横置于胸板底缘下方（x∈[-2.45, 2.45]，y∈[13.65, 14.45]，z∈[-2.78, -2.42]）
    # 皮鞘（x∈[-2.25, 0.85]）
    g.box(b, "sheath_body", (-2.25, 13.65, -2.75), (0.85, 14.45, -2.45), "hide_dk")
    g.box(b, "sheath_mouth", (0.55, 13.55, -2.82), (0.95, 14.55, -2.38), "strap_dk")
    # 皮鞘固定扣环（挂在主横带底唇上）
    g.box(b, "sheath_loop1", (-1.85, 14.25, -2.85), (-1.45, 15.95, -2.38), "cord_dk")
    g.box(b, "sheath_loop2", (-0.35, 14.25, -2.85), (0.05, 15.95, -2.38), "cord_dk")

    # 拔出一半的小骨锥（锥柄在右侧 x∈[0.95, 2.35]）
    g.box(b, "bone_knife_hilt", (0.85, 13.75, -2.68), (1.95, 14.35, -2.48), "bone")
    g.box(b, "bone_knife_pommel", (1.85, 13.65, -2.72), (2.35, 14.45, -2.44), "bone_lt")
    # 锥柄缠绳防滑
    g.box(b, "bone_knife_wrap", (1.15, 13.68, -2.75), (1.65, 14.42, -2.41), "cord")
    # 鞘身末端金属包角
    g.box(b, "sheath_tip_iron", (-2.45, 13.62, -2.78), (-2.15, 14.48, -2.38), "iron")
    g.box(b, "sheath_rivet", (-2.05, 13.85, -2.85), (-1.82, 14.25, -2.48), "iron_lt")


def part_hang_waist_tails(g: Gear) -> None:
    """腰带垂落皮舌与骨饰：穿出带扣自然下垂的双侧打孔皮带头与兽骨坠（独立骨）。"""
    b = "hang_waist_tails"
    # 右腰垂舌（x∈[3.45, 4.05]，y∈[8.2, 11.25]，z∈[-2.58, -2.25]）
    g.box(b, "waist_tail_r_top", (3.48, 10.35, -2.52), (3.98, 11.25, -2.25), "strap")
    g.box(b, "waist_tail_r_bot", (3.42, 9.15, -2.56), (3.92, 10.45, -2.28), "strap_dk")
    g.box(b, "waist_tail_r_tip", (3.48, 8.45, -2.52), (3.88, 9.25, -2.25), "strap")
    g.box(b, "waist_tail_r_hole", (3.56, 9.65, -2.62), (3.82, 9.98, -2.22), "void")
    # 右侧垂绳与小骨坠
    g.box(b, "waist_tail_r_cord", (3.54, 7.85, -2.50), (3.82, 8.55, -2.22), "cord")
    g.box(b, "waist_tail_r_bone", (3.45, 6.75, -2.55), (3.92, 7.95, -2.18), "bone_lt")

    # 左腰垂舌（小一段备用皮舌，x∈[-4.02, -3.48]）
    g.box(b, "waist_tail_l_top", (-3.98, 10.35, -2.52), (-3.48, 11.25, -2.25), "strap")
    g.box(b, "waist_tail_l_bot", (-3.92, 9.45, -2.56), (-3.42, 10.45, -2.28), "strap_dk")
    g.box(b, "waist_tail_l_tip", (-3.88, 8.85, -2.52), (-3.48, 9.55, -2.25), "strap")
    g.box(b, "waist_tail_l_hole", (-3.82, 9.85, -2.62), (-3.56, 10.18, -2.22), "void")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "harness_front": part_harness_front,
    "harness_back_x": part_harness_back_x,
    "harness_waist": part_harness_waist,
    "pouch_l": part_pouch_l,
    "pouch_c": part_pouch_c,
    "pouch_r": part_pouch_r,
    "jar_l": part_jar_l,
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
    wear_note="挂点：胸前 · 前排实用皮囊+双陶瓶+X形背跨带+横插骨锥",
    textured=False,            # Round 1 纯几何
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
