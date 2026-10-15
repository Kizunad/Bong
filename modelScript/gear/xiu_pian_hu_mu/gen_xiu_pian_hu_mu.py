#!/usr/bin/env python3
"""锈片护目（xiu_pian_hu_mu）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #11（.task-wasteland-gear.md）。
穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，
头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]，
躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，
双臂 x=±4..±8 y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形（参考图 xiu_pian_hu_mu_ref.png / xiu_pian_hu_mu_exploded.png）：
锈铁片敲成的护目镜，两片浑浊碎琉璃、皮条绑头，镜框铆钉和缠布。
- 护目镜贴在面部眼前：y∈[26.2, 29.2]，z ≤ -4.12（完全在头盒 -z 表面之外，严禁穿入）。
- 左右眼框由两圈粗厚锈铁片敲打成八角圆框，内嵌双层破损浑浊琉璃（裂缝暗斑）。
- 鼻梁处以拱起铁桥跨接，附带内衬皮垫和双孔铆接片。
- 侧翼延伸出铁质铰耳与皮带转接板，两侧粗皮带紧扣头部两侧（|x| ≥ 4.10），后脑勺后扣（z ≥ 4.12）。
- 右侧镜框斜裹一层脏麻布缠条，左侧镜框带三角铆接加固片与铁锈凸起。
- 侧带下方垂下一小串兽骨护符碎片（挂件独立骨，y ≥ 24.20，避开手臂），后脑勺扣带垂下残余皮舌与系绳（挂件独立骨）。

骨骼数 ≥ 10，挂件独立骨，全绿门禁（含 clearance、coplanar、self-test）。
用法：
    python3 modelScript/gear/xiu_pian_hu_mu/gen_xiu_pian_hu_mu.py --self-test
    python3 modelScript/gear/xiu_pian_hu_mu/gen_xiu_pian_hu_mu.py --gates
    python3 modelScript/gear/xiu_pian_hu_mu/gen_xiu_pian_hu_mu.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质定义
MATS = {
    "iron":       (82, 78, 76),       # 锻打深铁
    "iron_lt":    (115, 110, 108),    # 铁缘光泽 / 铆钉头
    "rust":       (152, 86, 52),      # 粗糙红锈
    "rust_dk":    (106, 58, 36),      # 浓重铁锈斑
    "glass":      (156, 182, 172),    # 浑浊绿琉璃
    "glass_dk":   (108, 134, 126),    # 琉璃暗纹/破损夹层
    "strap":      (116, 76, 52),      # 硬皮绑带
    "strap_dk":   (82, 54, 38),       # 磨损深皮
    "cloth":      (152, 138, 110),    # 粗麻缠布
    "cloth_dk":   (110, 96, 76),      # 脏布阴影
    "cord":       (142, 126, 80),     # 麻绳/细皮绳
    "cord_dk":    (102, 88, 56),
    "bone":       (212, 202, 174),    # 骨片/兽骨扣
    "bone_dk":    (162, 148, 118),    # 骨阴影
    "void":       (28, 24, 22),       # 暗缝隙
}

# 软件（皮带、布、绳、骨吊饰）与各部件允许轻微咬合
SOFT = ("strap", "strap_dk", "cloth", "cloth_dk", "cord", "cord_dk", "bone", "bone_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("glass", "glass_dk"), ("iron", "rust", "rust_dk"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("goggle_bridge", "root", (0.0, 27.7, -4.5), "鼻梁拱桥：双层铁拱梁、鼻垫护皮、中央铆件"),
    ("frame_r", "root", (2.35, 27.7, -4.6), "右镜框：八角铁圈、内外斜框、厚边与前凸"),
    ("frame_l", "root", (-2.35, 27.7, -4.6), "左镜框：八角铁圈、内外斜框、厚边与前凸"),
    ("lens_r", "frame_r", (2.35, 27.7, -4.5), "右琉璃镜片：浑浊主镜面、碎角补片、裂纹横条"),
    ("lens_l", "frame_l", (-2.35, 27.7, -4.5), "左琉璃镜片：浑浊主镜面、碎角补片、裂纹斜条"),
    ("strap_sides", "root", (0.0, 27.7, 0.0), "两侧头部绑带：太阳穴铰耳铆接板、双层横向主皮带、紧固卡线"),
    ("strap_back", "root", (0.0, 27.7, 4.25), "后脑勺绑带：后脑主横带、铁日字扣环、带舌插孔"),
    ("cloth_wrap", "frame_r", (3.2, 27.7, -4.3), "右框与右侧带缠布：斜缠防磨粗布条、卷边、垂落线头"),
    ("rust_patches", "frame_l", (-3.1, 27.7, -4.7), "左框加固铁补丁：三角补片、厚铆钉头、破损焊缝"),
    ("hang_bone", "strap_sides", (-4.3, 25.5, -2.0), "左侧挂骨符：细皮绳悬垂的小磨损兽骨板（可晃动）"),
    ("hang_tail", "strap_back", (1.2, 26.0, 4.35), "后脑绑带垂尾：穿出扣环自然下垂的打孔皮舌与细绳（可晃动）"),
]

# 不对称骨骼（两眼镜片裂纹不同，缠布在右，加固片在左，骨符在左，垂尾在后右）
ASYM_BONES = frozenset({"lens_r", "lens_l", "cloth_wrap", "rust_patches", "hang_bone", "hang_tail"})

# ───────────────────────────────────────────────────────────── 几何构建
EYE_X = 2.35
EYE_Y = 27.7
Z_FACE = -4.12       # 贴脸内缘（离头盒 -4.0 留出 0.12px 余量，严禁穿入）


def part_bridge(g: Gear) -> None:
    """鼻梁拱桥：双层铁拱梁、鼻垫护皮、中央铆件。"""
    b = "goggle_bridge"
    # 拱桥主铁板（跨度 x∈[-0.78, 0.78]，z∈[-4.90, -4.60]）
    g.box(b, "bridge_arch_mid", (-0.78, 27.40, -4.90), (0.78, 28.05, -4.60), "iron")
    g.box(b, "bridge_arch_top", (-0.70, 28.05, -4.85), (0.70, 28.45, -4.55), "iron_lt")
    g.box(b, "bridge_arch_bot", (-0.70, 27.00, -4.85), (0.70, 27.40, -4.55), "rust")

    # 中央加强筋与两枚加固小铆钉
    g.box(b, "bridge_rib", (-0.22, 26.90, -5.12), (0.22, 28.55, -4.82), "rust_dk")
    g.box(b, "bridge_rivet_t", (-0.16, 28.18, -5.30), (0.16, 28.50, -4.98), "iron_lt")
    g.box(b, "bridge_rivet_b", (-0.16, 26.95, -5.30), (0.16, 27.27, -4.98), "iron_lt")

    # 贴脸侧皮垫（防止铁片磨破鼻梁）
    g.box(b, "bridge_pad_main", (-0.82, 27.10, -4.45), (0.82, 28.35, -4.15), "strap_dk")
    g.box(b, "bridge_pad_lip", (-0.60, 26.75, -4.38), (0.60, 27.10, -4.12), "strap")
    g.box(b, "bridge_pad_top", (-0.60, 28.35, -4.38), (0.60, 28.70, -4.12), "strap")

    # 鼻梁与左右框过渡铁角片（注意成对且带 _r/_l）
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        x_min, x_max = min(sx * 0.76, sx * 1.15), max(sx * 0.76, sx * 1.15)
        g.box(b, f"bridge_wing_{side}", (x_min, 27.25, -4.75), (x_max, 28.20, -4.45), "iron")
        rx_min, rx_max = min(sx * 0.82, sx * 1.08), max(sx * 0.82, sx * 1.08)
        g.box(b, f"bridge_wing_rivet_{side}", (rx_min, 27.55, -4.98), (rx_max, 27.85, -4.68), "rust")


def _build_single_frame(g: Gear, b: str, sx: float, side: str) -> None:
    """构建单侧镜框的八角厚铁圈及外缘棱台。"""
    cx = sx * EYE_X
    cy = EYE_Y

    # 四边主框：
    # 上下边横跨 x 从 cx - 1.05 到 cx + 1.05
    x_in = cx - sx * 1.05
    x_out = cx + sx * 1.05
    x_min, x_max = min(x_in, x_out), max(x_in, x_out)
    g.box(b, f"frame_top_{side}", (x_min, cy + 1.05, -5.05), (x_max, cy + 1.65, -4.25), "iron")
    g.box(b, f"frame_bot_{side}", (x_min, cy - 1.65, -5.05), (x_max, cy - 1.05, -4.25), "iron")

    # 左右立框：y 从 cy - 1.05 到 cy + 1.05（与上下边垂直交错，不共享 Y 面）
    in_x0, in_x1 = min(cx - sx * 1.65, cx - sx * 1.05), max(cx - sx * 1.65, cx - sx * 1.05)
    out_x0, out_x1 = min(cx + sx * 1.05, cx + sx * 1.65), max(cx + sx * 1.05, cx + sx * 1.65)
    g.box(b, f"frame_in_{side}", (in_x0, cy - 1.05, -5.05), (in_x1, cy + 1.05, -4.25), "iron")
    g.box(b, f"frame_out_{side}", (out_x0, cy - 1.05, -5.05), (out_x1, cy + 1.05, -4.25), "rust")

    # 四个切角（右上、右下、左上、左下）
    corners = (
        ("rt", 0.95, 1.55, 0.90, 1.50),
        ("rb", 0.95, 1.55, -1.50, -0.90),
        ("lt", -1.55, -0.95, 0.90, 1.50),
        ("lb", -1.55, -0.95, -1.50, -0.90),
    )
    for cname, x0, x1, y0, y1 in corners:
        cx_min, cx_max = min(cx + sx * x0, cx + sx * x1), max(cx + sx * x0, cx + sx * x1)
        mat = "rust_dk" if "r" in cname else "iron"
        g.box(b, f"frame_corner_{side}_{cname}", (cx_min, cy + y0, -4.95), (cx_max, cy + y1, -4.30), mat)

    # 镜圈外凸压檐（外缘突出的挡沙铁檐）
    g.box(b, f"frame_visor_{side}", (x_min - 0.20, cy + 1.45, -5.25), (x_max + 0.20, cy + 1.80, -4.85), "iron_lt")
    g.box(b, f"frame_visor_lip_{side}", (x_min + 0.20, cy + 1.70, -5.38), (x_max - 0.20, cy + 1.98, -4.98), "iron")
    g.box(b, f"frame_skirt_{side}", (x_min - 0.20, cy - 1.85, -5.18), (x_max + 0.20, cy - 1.45, -4.65), "rust")

    # 镜筒筒身向面部的深筒延伸壁（从 z=-4.65 到 z=-4.35，厚度 0.30）
    bx_min, bx_max = min(cx - 1.45, cx + 1.45), max(cx - 1.45, cx + 1.45)
    g.box(b, f"frame_barrel_t_{side}", (bx_min, cy + 1.15, -4.65), (bx_max, cy + 1.55, -4.35), "iron")
    g.box(b, f"frame_barrel_b_{side}", (bx_min, cy - 1.55, -4.65), (bx_max, cy - 1.15, -4.35), "iron")
    bin_x0, bin_x1 = min(cx - sx * 1.48, cx - sx * 1.15), max(cx - sx * 1.48, cx - sx * 1.15)
    bout_x0, bout_x1 = min(cx + sx * 1.15, cx + sx * 1.48), max(cx + sx * 1.15, cx + sx * 1.48)
    g.box(b, f"frame_barrel_in_{side}", (bin_x0, cy - 1.15, -4.65), (bin_x1, cy + 1.15, -4.35), "iron")
    g.box(b, f"frame_barrel_out_{side}", (bout_x0, cy - 1.15, -4.65), (bout_x1, cy + 1.15, -4.35), "rust_dk")

    # 紧贴面部的减震软皮圈（遮光贴皮，从 z=-4.35 到 Z_FACE=-4.10，厚度 0.25）
    gx_min, gx_max = min(cx - 1.52, cx + 1.52), max(cx - 1.52, cx + 1.52)
    g.box(b, f"frame_gasket_t_{side}", (gx_min, cy + 1.25, -4.35), (gx_max, cy + 1.62, Z_FACE), "strap_dk")
    g.box(b, f"frame_gasket_b_{side}", (gx_min, cy - 1.62, -4.35), (gx_max, cy - 1.25, Z_FACE), "strap_dk")
    gin_x0, gin_x1 = min(cx - sx * 1.62, cx - sx * 1.25), max(cx - sx * 1.62, cx - sx * 1.25)
    gout_x0, gout_x1 = min(cx + sx * 1.25, cx + sx * 1.62), max(cx + sx * 1.25, cx + sx * 1.62)
    g.box(b, f"frame_gasket_in_{side}", (gin_x0, cy - 1.25, -4.35), (gin_x1, cy + 1.25, Z_FACE), "strap")
    g.box(b, f"frame_gasket_out_{side}", (gout_x0, cy - 1.25, -4.35), (gout_x1, cy + 1.25, Z_FACE), "strap")

    # 镜框边沿散落的小铁铆钉（凸出于前表面）
    g.box(b, f"rim_rivet_t_{side}", (cx - 0.16, cy + 1.48, -5.28), (cx + 0.16, cy + 1.78, -4.95), "iron_lt")
    g.box(b, f"rim_rivet_b_{side}", (cx - 0.16, cy - 1.78, -5.28), (cx + 0.16, cy - 1.48, -4.95), "iron_lt")


def part_frames(g: Gear) -> None:
    """右左主镜框：八角铁圈、内外斜框、厚边与前凸筒身。"""
    _build_single_frame(g, "frame_r", 1.0, "r")
    _build_single_frame(g, "frame_l", -1.0, "l")


def _build_single_lens(g: Gear, b: str, sx: float, side: str) -> None:
    """构建单侧镜框内的浑浊琉璃镜片及裂纹补片。"""
    cx = sx * EYE_X
    cy = EYE_Y
    # 主琉璃平片：嵌在内部 (x∈[cx-1.02, cx+1.02], y∈[cy-0.92, cy+0.92], z 约 -4.75 到 -4.50)
    lx0, lx1 = min(cx - 1.02, cx + 1.02), max(cx - 1.02, cx + 1.02)
    g.box(b, f"lens_main_{side}", (lx0, cy - 0.92, -4.72), (lx1, cy + 0.92, -4.48), "glass")
    # 边缘厚度层与深色透光层
    bx0, bx1 = min(cx - 0.92, cx + 0.92), max(cx - 0.92, cx + 0.92)
    g.box(b, f"lens_back_{side}", (bx0, cy - 0.82, -4.52), (bx1, cy + 0.82, -4.26), "glass_dk")
    # 琉璃中央凸透微拱
    cx0, cx1 = min(cx - 0.52, cx + 0.52), max(cx - 0.52, cx + 0.52)
    g.box(b, f"lens_core_{side}", (cx0, cy - 0.52, -4.92), (cx1, cy + 0.52, -4.66), "glass")

    # 左右不对称裂纹与暗纹加固条
    if side == "r":
        g.box(b, "lens_crack_r1", (cx - 0.95, cy + 0.10, -4.86), (cx + 0.75, cy + 0.38, -4.58), "glass_dk")
        g.box(b, "lens_crack_r2", (cx + 0.05, cy - 0.42, -4.80), (cx + 0.85, cy + 0.18, -4.52), "glass_dk")
        g.box(b, "lens_clip_r_top", (cx - 0.30, cy + 0.82, -5.02), (cx + 0.30, cy + 1.15, -4.42), "rust")
        g.box(b, "lens_clip_r_bot", (cx + 0.22, cy - 1.15, -5.02), (cx + 0.72, cy - 0.82, -4.42), "iron")
    else:
        g.box(b, "lens_crack_l1", (cx - 0.78, cy - 0.68, -4.86), (cx + 0.68, cy + 0.58, -4.58), "glass_dk")
        g.box(b, "lens_crack_l2", (cx - 0.68, cy + 0.18, -4.80), (cx + 0.18, cy + 0.72, -4.52), "glass_dk")
        g.box(b, "lens_clip_l_outer", (cx - 1.15, cy - 0.28, -5.02), (cx - 0.82, cy + 0.28, -4.42), "rust_dk")


def part_lenses(g: Gear) -> None:
    """双眼浑浊琉璃镜片：透光面、磨损瑕疵暗纹、内嵌加固铁夹。"""
    _build_single_lens(g, "lens_r", 1.0, "r")
    _build_single_lens(g, "lens_l", -1.0, "l")


def part_straps_sides(g: Gear) -> None:
    """两侧头部绑带：太阳穴铰耳铆接板、双层横向主皮带。"""
    b = "strap_sides"
    for sx, side in ((1.0, "r"), (-1.0, "l")):
        # 外伸铰链耳（前面过渡部分 z∈[-4.45, -4.08]）
        ex0, ex1 = min(sx * 3.45, sx * 4.08), max(sx * 3.45, sx * 4.08)
        g.box(b, f"hinge_ear_{side}", (ex0, 27.15, -4.52), (ex1, 28.25, -4.08), "iron")
        px0, px1 = min(sx * 3.75, sx * 4.05), max(sx * 3.75, sx * 4.05)
        g.box(b, f"hinge_pin_{side}", (px0, 26.85, -4.42), (px1, 28.55, -4.15), "rust")

        # 头部侧面外壁转接板：必须严格在 |x| ≥ 4.10！
        fx0, fx1 = min(sx * 4.10, sx * 4.38), max(sx * 4.10, sx * 4.38)
        g.box(b, f"hinge_flange_{side}", (fx0, 27.05, -4.08), (fx1, 28.35, -3.52), "iron")
        rx0, rx1 = min(sx * 4.25, sx * 4.52), max(sx * 4.25, sx * 4.52)
        g.box(b, f"hinge_rivet1_{side}", (rx0, 27.85, -3.88), (rx1, 28.18, -3.58), "iron_lt")
        g.box(b, f"hinge_rivet2_{side}", (rx0, 27.22, -3.88), (rx1, 27.55, -3.58), "iron_lt")

        # 头部侧面贯通主皮带（从 z = -3.52 到 z = +4.10）
        mx0, mx1 = min(sx * 4.10, sx * 4.35), max(sx * 4.10, sx * 4.35)
        g.box(b, f"side_strap_main_{side}", (mx0, 27.10, -3.52), (mx1, 28.30, 1.20), "strap")
        g.box(b, f"side_strap_rear_{side}", (mx0, 27.10, 1.20), (mx1, 28.30, 4.10), "strap_dk")

        # 侧带加强脊条（压在主带上）
        gx0, gx1 = min(sx * 4.18, sx * 4.43), max(sx * 4.18, sx * 4.43)
        g.box(b, f"side_strap_ridge_t_{side}", (gx0, 28.20, -3.40), (gx1, 28.52, 4.00), "strap_dk")
        g.box(b, f"side_strap_ridge_b_{side}", (gx0, 26.88, -3.40), (gx1, 27.20, 4.00), "strap_dk")

        # 侧边调节扣环（z ≈ 0.0）
        bx0, bx1 = min(sx * 4.22, sx * 4.48), max(sx * 4.22, sx * 4.48)
        g.box(b, f"side_buckle_frame_{side}", (bx0, 26.78, -0.42), (bx1, 28.62, 0.42), "iron")
        kx0, kx1 = min(sx * 4.33, sx * 4.58), max(sx * 4.33, sx * 4.58)
        g.box(b, f"side_buckle_pin_{side}", (kx0, 27.55, -0.22), (kx1, 27.85, 0.22), "rust_dk")


def part_strap_back(g: Gear) -> None:
    """后脑勺绑带：后脑主横带、铁日字扣环、带舌插孔。"""
    b = "strap_back"
    # 后脑部表面为 z = +4.0，皮带必须严格位于 z ≥ 4.10！
    # 左右成对后带
    g.box(b, "back_strap_r", (0.50, 27.10, 4.10), (4.10, 28.30, 4.35), "strap")
    g.box(b, "back_strap_l", (-4.10, 27.10, 4.10), (-0.50, 28.30, 4.35), "strap")

    # 后脑主带上下厚边（中心对称）
    g.box(b, "back_strap_top", (-3.80, 28.20, 4.18), (3.80, 28.52, 4.44), "strap_dk")
    g.box(b, "back_strap_bot", (-3.80, 26.88, 4.18), (3.80, 27.20, 4.44), "strap_dk")

    # 中央日字铁带扣：左右立框配对成 _l / _r，上下横梁与中柱对称
    g.box(b, "back_buckle_top", (-0.85, 28.40, 4.28), (0.85, 28.75, 4.58), "iron")
    g.box(b, "back_buckle_bot", (-0.85, 26.65, 4.28), (0.85, 27.00, 4.58), "iron")
    g.box(b, "back_buckle_l", (-0.85, 27.00, 4.28), (-0.55, 28.40, 4.58), "iron_lt")
    g.box(b, "back_buckle_r", (0.55, 27.00, 4.28), (0.85, 28.40, 4.58), "iron_lt")
    g.box(b, "back_buckle_mid", (-0.15, 27.00, 4.34), (0.15, 28.40, 4.64), "iron")
    g.box(b, "back_buckle_tongue", (-0.18, 27.58, 4.64), (0.18, 27.88, 4.90), "rust_dk")

    # 垫在日字扣下方的皮垫
    g.box(b, "back_strap_underpad", (-0.95, 26.70, 4.08), (0.95, 28.70, 4.36), "strap_dk")


def part_cloth_wrap(g: Gear) -> None:
    """右框与右侧带缠布：斜缠防磨粗布条、卷边、垂落线头。"""
    b = "cloth_wrap"
    # 右镜框外侧斜向缠绕的数层粗麻布片
    g.box(b, "wrap_band1", (2.95, 28.15, -5.12), (3.85, 28.78, -4.32), "cloth")
    g.box(b, "wrap_band2", (2.85, 27.20, -5.20), (3.85, 27.82, -4.40), "cloth_dk")
    g.box(b, "wrap_band3", (3.10, 26.52, -5.10), (3.92, 27.18, -4.32), "cloth")

    # 缠布重叠结与布褶棱起
    g.box(b, "wrap_knot_main", (3.62, 27.68, -4.92), (4.15, 28.32, -4.42), "cloth")
    g.box(b, "wrap_knot_layer", (3.72, 27.78, -5.15), (4.08, 28.22, -4.72), "cloth_dk")

    # 缠布向下压入侧皮带连接处
    g.box(b, "wrap_tail_top", (4.12, 27.32, -3.85), (4.42, 28.08, -3.20), "cloth")
    g.box(b, "wrap_tail_bot", (4.14, 26.82, -3.62), (4.40, 27.48, -2.98), "cloth_dk")

    # 麻绳扎结与垂落的小草绳头（确保 overlap > 0.18）
    g.box(b, "wrap_cord_loop", (3.42, 27.42, -5.25), (3.78, 28.28, -4.85), "cord")
    g.box(b, "wrap_cord_drop1", (3.68, 26.18, -4.75), (4.02, 27.00, -4.40), "cord_dk")
    g.box(b, "wrap_cord_drop2", (3.75, 25.65, -4.68), (4.08, 26.35, -4.32), "cord")


def part_rust_patches(g: Gear) -> None:
    """左框加固铁补丁：三角补片、厚铆钉头、破损焊缝。"""
    b = "rust_patches"
    # 左镜框上侧加固铁补片
    g.box(b, "patch_plate_top", (-3.75, 28.22, -5.20), (-2.72, 28.88, -4.42), "rust")
    g.box(b, "patch_plate_mid", (-3.88, 27.52, -5.28), (-3.22, 28.38, -4.50), "rust_dk")
    g.box(b, "patch_lip", (-3.68, 28.62, -5.38), (-3.02, 28.98, -4.62), "iron")

    # 补片上的凸出大铆钉
    g.box(b, "patch_rivet1", (-3.62, 28.42, -5.48), (-3.30, 28.78, -5.08), "iron_lt")
    g.box(b, "patch_rivet2", (-3.02, 28.42, -5.38), (-2.70, 28.78, -4.98), "iron_lt")
    g.box(b, "patch_rivet3", (-3.78, 27.72, -5.48), (-3.42, 28.08, -5.08), "iron_lt")

    # 焊渣粗糙面
    g.box(b, "weld_lump1", (-2.62, 28.72, -5.08), (-2.28, 29.08, -4.62), "rust_dk")
    g.box(b, "weld_lump2", (-3.88, 27.12, -4.98), (-3.52, 27.48, -4.52), "rust")
    g.box(b, "weld_lump3", (-2.18, 26.22, -5.08), (-1.82, 26.58, -4.62), "rust_dk")


def part_hang_bone(g: Gear) -> None:
    """左侧挂骨符：细皮绳悬垂的小磨损兽骨板（挂件独立骨，y ≥ 24.20，避开手臂）。"""
    b = "hang_bone"
    # 悬挂起点位于左侧调节扣下方（x∈[-4.42, -4.12], z∈[-2.25, -1.85]）
    # 绳段 1
    g.box(b, "bone_cord_t", (-4.42, 26.50, -2.18), (-4.14, 27.15, -1.82), "cord")
    # 绳段 2（与绳段 1 在 y∈[26.50, 26.75] 有 0.25 重叠）
    g.box(b, "bone_cord_m", (-4.36, 25.85, -2.22), (-4.10, 26.70, -1.85), "cord_dk")
    # 绳结（在 y∈[25.85, 26.15] 有 0.30 重叠）
    g.box(b, "bone_cord_knot", (-4.42, 25.60, -2.32), (-4.08, 26.12, -1.80), "cord")

    # 兽骨牌头部（与绳结在 y∈[25.60, 25.85] 有 0.25 重叠）
    g.box(b, "bone_charm_top", (-4.38, 25.10, -2.35), (-4.12, 25.75, -1.78), "bone")
    # 兽骨牌身（与骨牌头部在 y∈[25.10, 25.35] 有 0.25 重叠，y 下沿到 24.25，严格 > 24.10 避开手臂）
    g.box(b, "bone_charm_body", (-4.42, 24.25, -2.42), (-4.10, 25.30, -1.72), "bone")
    # 骨牌下部收尖（嵌在骨牌身内）
    g.box(b, "bone_charm_tip", (-4.38, 24.20, -2.28), (-4.14, 24.58, -1.85), "bone_dk")
    # 骨片刻纹与孔洞暗槽
    g.box(b, "bone_charm_notch", (-4.45, 24.60, -2.25), (-4.18, 24.95, -1.95), "bone_dk")
    g.box(b, "bone_hole_void", (-4.44, 25.32, -2.18), (-4.18, 25.58, -1.92), "void")


def part_hang_tail(g: Gear) -> None:
    """后脑绑带垂尾：穿出扣环自然下垂的打孔皮舌与细绳（挂件独立骨，可晃动）。"""
    b = "hang_tail"
    # 后脑扣右侧皮舌
    g.box(b, "tail_root", (0.82, 27.22, 4.35), (1.48, 28.18, 4.68), "strap")
    # 第一段下垂皮舌（与根部在 y∈[27.22, 27.45] 充分重叠）
    g.box(b, "tail_drop1", (1.02, 26.40, 4.32), (1.58, 27.42, 4.62), "strap")
    # 第二段下垂皮舌（与第一段在 y∈[26.40, 26.65] 充分重叠）
    g.box(b, "tail_drop2", (1.12, 25.40, 4.28), (1.62, 26.55, 4.55), "strap_dk")
    # 尾尖（与第二段在 y∈[25.40, 25.65] 充分重叠）
    g.box(b, "tail_tip", (1.20, 24.80, 4.25), (1.56, 25.55, 4.48), "strap_dk")

    # 皮舌上的调节圆孔（嵌在皮舌中）
    g.box(b, "tail_hole1", (1.25, 26.75, 4.45), (1.50, 27.10, 4.70), "void")
    g.box(b, "tail_hole2", (1.27, 25.85, 4.40), (1.52, 26.20, 4.65), "void")

    # 扣环旁束紧细皮绳结与下垂碎绳头
    g.box(b, "tail_cord_knot", (0.62, 26.82, 4.40), (1.08, 27.28, 4.70), "cord")
    g.box(b, "tail_cord_strand1", (0.72, 25.80, 4.35), (0.98, 26.90, 4.62), "cord_dk")
    g.box(b, "tail_cord_strand2", (0.78, 25.10, 4.28), (1.04, 25.95, 4.54), "cord")


# ───────────────────────────────────────────────────────────── 零件清单与规格
PARTS = {
    "bridge": part_bridge,
    "frames": part_frames,
    "lenses": part_lenses,
    "straps_sides": part_straps_sides,
    "strap_back": part_strap_back,
    "cloth_wrap": part_cloth_wrap,
    "rust_patches": part_rust_patches,
    "hang_bone": part_hang_bone,
    "hang_tail": part_hang_tail,
}

SPEC = GearSpec(
    gear_id="xiu_pian_hu_mu",
    model_name="XiuPianHuMu",
    title="锈片护目 / xiu_pian_hu_mu",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：眼/头 · 锈片护目镜+碎琉璃+皮条绑头",
    textured=False,            # Round 1 纯几何
    round_label="round 1/3",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
