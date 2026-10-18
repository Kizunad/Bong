#!/usr/bin/env python3
"""青铜行铃（qing_tong_xing_ling）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #5（佩挂 / 吊挂，也可提着）。凡物青铜铃：合瓦形（横截面是两头尖的杏仁形）、口缘内凹成弧、
两角下垂，顶有方颈半环纽，内悬一枚铁舌。末法体现：原本能「镇驱」的灵纹不起作用——所以腹面只有云纹带和乳钉带，
没有发光的灵纹；它仍是一只响的铃，唯一的功能是报信。

造型与比例以参考图（``qing_tong_xing_ling_ref.png``：左 = 宽面正视，右 = 窄面侧视）为准：
环顶到口角 = 7px（参考图 813px），所以口宽约 7.4px、中部最厚约 1.9px（任务卡写「口径约 5」，参考图
口宽 / 环顶到口角 = 1.06，取 7px 高时口宽 7.4px，按参考图走）。钟身自肩到口逐渐外撇，口缘中间高、两角下垂；
腹面自上而下：双线 + 云纹带 + 双线 → 空白 → 双线夹一排乳钉 → 口缘加厚边。

坐标（px，1 方块 = 16px）：铁舌尖落在 y=0（站在地上看比例，实际是吊着的），钟心 x=z=0，宽面朝 ±z，
窄面朝 ±x；整件左右、前后对称。手持：握住纽上的丝绳，铃垂在拳下（见 check.png 的「持握姿势」栏）。

用法：
    python3 modelScript/metalware/qing_tong_xing_ling/gen_qing_tong_xing_ling.py --table | --gates | (全量)
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "metalware" / "_kit"))
import metal_review  # noqa: E402
from gear_kit import Gear  # noqa: E402
from metal_review import MetalSpec  # noqa: E402

MATS = {
    "bz":        (150, 92, 58),     # 红褐铜底
    "bz_relief": (168, 112, 74),    # 浮雕带 / 乳钉
    "bz_pat":    (84, 130, 110),    # 青绿锈
    "bz_worn":   (206, 152, 98),    # 手磨亮铜（环、口缘）
    "bz_dk":     (76, 54, 42),      # 凹处暗铜 / 磕缺
    "iron":      (72, 66, 62),      # 铁舌
    "iron_lt":   (128, 118, 108),   # 铁舌磨亮的锤头
    "cord":      (110, 92, 70),     # 丝绳
    "void":      (18, 16, 14),
}

# 生成器材质名 → 画笔库（handheld_texture.py 的 metal_bz_* 一组，六件青铜器共用）
BZ_TEXTURES = {
    "bz": "metal_bz", "bz_relief": "metal_bz_relief", "bz_pat": "metal_bz_pat", "bz_worn": "metal_bz_worn",
    "bz_dk": "metal_bz_dk", "iron": "metal_bz_iron", "iron_lt": "metal_bz_iron_lt", "cord": "metal_bz_cord",
    "void": "metal_bz_soot",
}

# ───────────────────────────────────────────────────────────── 尺寸：参考图 px → 模型 px
REF_BOTTOM = 995.0          # 铁舌尖（模型 y = 0）
S = 7.0 / 813.0             # 环顶（ref y 82）到口角（ref y 895）= 7px


def Y(ref_y: float) -> float:
    return (REF_BOTTOM - ref_y) * S


def X(ref_px: float) -> float:
    return ref_px * S


# 钟身剖面：(参考图 y, 半宽 px, 半厚 px 模型)，自上而下；中部最厚 1.9px（半厚 0.95）
PROFILE = ((258, 228, 0.50), (300, 240, 0.54), (350, 256, 0.60), (410, 270, 0.65), (470, 280, 0.70),
           (535, 292, 0.75), (600, 305, 0.80), (665, 326, 0.85), (735, 358, 0.90), (812, 392, 0.94))
HOLLOW_FROM = 600           # 此线以下的钟腹中间镂空（口下可见内壁与铁舌）
WALL_T = 0.28
BODY_BOTTOM = 812.0         # 口缘中间的 ref y（中间高、两角下垂）
# 口缘裙：(半宽起 px, 半宽止 px, 底边 ref y)，两角比中间低；越靠外越薄（杏仁形）
SKIRT = ((200, 300, 872), (300, 392, 893), (392, 432, 850))
RELIEF_D = 0.22

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根；铁舌尖落在 y=0"),
    ("body", "root", (0.0, Y(535), 0.0), "钟身：合瓦形（杏仁形截面）逐层外撇，中腹镂空"),
    ("skirt", "body", (0.0, Y(812), 0.0), "口缘裙：中间高、两角下垂的内凹弧口 + 加厚缘边"),
    ("band", "body", (0.0, Y(430), 0.0), "腹面纹带：上下双线 + 云纹带 + 乳钉带（灵纹不起作用，只剩装饰）"),
    ("top", "body", (0.0, Y(250), 0.0), "肩顶 + 方颈 + 半环纽"),
    ("cord", "top", (0.0, Y(60), 0.0), "穿环的丝绳 + 绳结"),
    ("clapper", "body", (0.0, Y(650), 0.0), "铁舌：悬桥 + 细杆 + 梨形锤头（磨亮）"),
    ("chip", "skirt", (X(240), Y(850), 0.0), "口缘一处磕缺（不对称）"),
]
ASYM_BONES = frozenset({"chip"})


# ───────────────────────────────────────────────────────────── 剖面插值
def profile_at(ref_y: float) -> tuple[float, float]:
    """某 ref y 处的 (半宽 px→模型, 半厚)，线性插值。"""
    pts = PROFILE
    if ref_y <= pts[0][0]:
        return X(pts[0][1]), pts[0][2]
    for (ya, ha, ta), (yb, hb, tb) in zip(pts, pts[1:]):
        if ya <= ref_y <= yb:
            f = (ref_y - ya) / (yb - ya)
            return X(ha + (hb - ha) * f), ta + (tb - ta) * f
    return X(pts[-1][1]), pts[-1][2]


def slabs(hx: float, tz: float):
    """杏仁形截面的三段近似：(x 起, x 止, 半厚)。中心最厚，向两侧收薄。"""
    return ((0.0, 0.5 * hx, tz), (0.5 * hx, 0.8 * hx, 0.86 * tz), (0.8 * hx, hx, 0.58 * tz))


# ───────────────────────────────────────────────────────────── 零件
def part_body(g: Gear) -> None:
    """钟身：每相邻两个剖面点叠一层（中心 + 两对侧段，构成杏仁截面）；HOLLOW_FROM 以下中心镂空，只留前后壁。"""
    b = "body"
    for k, ((ya, ha, ta), (yb, hb, tb)) in enumerate(zip(PROFILE, PROFILE[1:])):
        hx, tz = X((ha + hb) / 2), (ta + tb) / 2
        y0, y1 = Y(yb), Y(ya)
        (c0, c1, tc), (m0, m1, tm), (e0, e1, te) = slabs(hx, tz)
        if ya >= HOLLOW_FROM:
            g.box(b, f"body_t{k}_wf", (-c1, y0, -tc), (c1, y1, -tc + WALL_T), "bz")
            g.box(b, f"body_t{k}_wb", (-c1, y0, tc - WALL_T), (c1, y1, tc), "bz")
        else:
            g.box(b, f"body_t{k}_c", (-c1, y0, -tc), (c1, y1, tc), "bz")
        g.pair(b, f"body_t{k}_m", (m0, y0, -tm), (m1, y1, tm), "bz")
        g.pair(b, f"body_t{k}_e", (e0, y0, -te), (e1, y1, te), "bz")


def part_skirt(g: Gear) -> None:
    """口缘裙：身下按到中线的距离分三列往下接（越靠外越长越薄，外角收成下垂的尖角）+ 缘边加厚的亮铜棱。

    中间一列（|x|<200px）的缘边直接挂在 body 最下一层下面；B / C 列的壁到「缘边顶」为止，缘边单独一块在壁下。
    """
    b, lip = "skirt", 0.26
    top = Y(BODY_BOTTOM)
    tz0 = PROFILE[-1][2]
    g.box(b, "skirt_lip_mid_f", (-X(200), top - lip, -tz0 - 0.12), (X(200), top, -tz0 + WALL_T), "bz_worn")
    g.box(b, "skirt_lip_mid_b", (-X(200), top - lip, tz0 - WALL_T), (X(200), top, tz0 + 0.12), "bz_worn")
    x0, x1, yb = SKIRT[0]                       # B 列：薄壁（中空）
    tz = max(0.3, 0.97 * math.sqrt(max(0.05, 1 - (((x0 + x1) / 2) / 430.0) ** 2)))
    g.pair(b, "skirt_c0_f", (X(x0), Y(yb) + lip, -tz), (X(x1), top, -tz + WALL_T), "bz")
    g.pair(b, "skirt_c0_b", (X(x0), Y(yb) + lip, tz - WALL_T), (X(x1), top, tz), "bz")
    g.pair(b, "skirt_lip0_f", (X(x0), Y(yb), -tz - 0.12), (X(x1), Y(yb) + lip, -tz + WALL_T), "bz_worn")
    g.pair(b, "skirt_lip0_b", (X(x0), Y(yb), tz - WALL_T), (X(x1), Y(yb) + lip, tz + 0.12), "bz_worn")
    x0, x1, yb = SKIRT[1]                       # C 列：已经很薄，实心；底边一道亮铜棱
    tz = max(0.3, 0.97 * math.sqrt(max(0.05, 1 - (((x0 + x1) / 2) / 430.0) ** 2)))
    g.pair(b, "skirt_c1", (X(x0), Y(yb) + lip, -tz), (X(x1), top, tz), "bz")
    g.pair(b, "skirt_lip1", (X(x0), Y(yb), -tz - 0.1), (X(x1), Y(yb) + lip, tz + 0.1), "bz_worn")
    x0, x1, yb = SKIRT[2]                       # 外角：杏仁截面的尖端，薄而下垂，整块亮铜
    tz = max(0.3, 0.97 * math.sqrt(max(0.05, 1 - (((x0 + x1) / 2) / 430.0) ** 2)))
    g.pair(b, "skirt_horn", (X(x0), Y(yb), -tz), (X(x1), top, tz), "bz_worn")


def _face_z(ref_y: float, x: float) -> float:
    """某 ref y、模型 x 处宽面（杏仁截面）的半厚。"""
    hx, tz = profile_at(ref_y)
    for x0, x1, t in slabs(hx, tz):
        if abs(x) < x1 + 1e-9:
            return t
    return slabs(hx, tz)[-1][2]


def _row(g: Gear, bone: str, name: str, ref_y0: float, ref_y1: float, mat: str, x_range: tuple | None = None) -> None:
    """横贯宽面的一条浮雕带（前后各一）：沿杏仁截面的三段各一块，贴面 0.22 凸。"""
    ymid = (ref_y0 + ref_y1) / 2
    hx, tz = profile_at(ymid)
    y0, y1 = Y(ref_y1), Y(ref_y0)
    for k, (x0, x1, t) in enumerate(slabs(hx, tz)):
        if k == 0:
            g.box(bone, f"{name}_f{k}", (-x1, y0, -t - RELIEF_D), (x1, y1, -t), mat)
            g.box(bone, f"{name}_b{k}", (-x1, y0, t), (x1, y1, t + RELIEF_D), mat)
        else:
            g.pair(bone, f"{name}_f{k}", (x0, y0, -t - RELIEF_D), (x1, y1, -t), mat)
            g.pair(bone, f"{name}_b{k}", (x0, y0, t), (x1, y1, t + RELIEF_D), mat)


def part_band(g: Gear) -> None:
    """腹面纹带：云纹带（上下双线夹 5 朵如意云头）+ 下部双线夹一排乳钉。云头 = 云冠 + 云身 + 云心卷。"""
    b = "band"
    _row(g, b, "band_a", 345, 372, "bz_relief")
    _row(g, b, "band_b", 462, 490, "bz_relief")
    _row(g, b, "band_c", 572, 600, "bz_relief")
    _row(g, b, "band_d", 640, 668, "bz_relief")
    ymid = 417.0
    # 云头：五朵（中心一朵 + 左右各两朵），朵距 1.0px；z 随所在截面段的厚度
    for i, cx in enumerate((0.0, 1.0, 2.0)):
        t = _face_z(ymid, cx)
        for sz, face in ((-1, "f"), (1, "b")):
            z0, z1 = (-t - RELIEF_D, -t) if sz < 0 else (t, t + RELIEF_D)
            z2 = (z0 - 0.0, z1 + 0.0)
            zc = (-t - RELIEF_D - 0.0, -t) if sz < 0 else (t, t + RELIEF_D)
            outer0, outer1 = (zc[0] - 0.22, zc[0]) if sz < 0 else (zc[1], zc[1] + 0.22)
            parts = (
                (f"cloud{i}_body_{face}", (-0.34, Y(452), zc[0]), (0.34, Y(400), zc[1])),
                (f"cloud{i}_crown_{face}", (-0.22, Y(400), zc[0]), (0.22, Y(372), zc[1])),
                (f"cloud{i}_curl_{face}", (-0.11, Y(441), outer0), (0.11, Y(415), outer1)),
            )
            for nm, f, tt in parts:
                if cx == 0.0:
                    g.box(b, nm, f, tt, "bz_relief")
                else:
                    g.pair(b, nm, (f[0] + cx, f[1], f[2]), (tt[0] + cx, tt[1], tt[2]), "bz_relief")
    # 乳钉：双线之间 5 颗（中心一颗 + 左右各两颗），0.3 见方、凸 0.3
    ys = (Y(632), Y(606))
    for i, cx in enumerate((0.0, 1.05, 2.1)):
        t = _face_z((606 + 632) / 2, cx)
        for sz, face in ((-1, "f"), (1, "b")):
            zf = (-t - 0.3, -t) if sz < 0 else (t, t + 0.3)
            if cx == 0.0:
                g.box(b, f"stud{i}_{face}", (-0.15, ys[0], zf[0]), (0.15, ys[1], zf[1]), "bz_worn")
            else:
                g.pair(b, f"stud{i}_{face}", (cx - 0.15, ys[0], zf[0]), (cx + 0.15, ys[1], zf[1]), "bz_worn")
    # 窄面（±x 棱线处）一条竖棱，读作合瓦的接缝
    hx = profile_at(430)[0]
    g.pair(b, "seam", (hx - 0.05, Y(786), -0.12), (hx + 0.17, Y(284), 0.12), "bz_relief")


def part_top(g: Gear) -> None:
    """肩顶盖 + 方颈 + 半环纽（两竖 + 两级拱顶），环内留孔穿绳。"""
    b = "top"
    hx, tz = profile_at(258)
    g.box(b, "top_cap", (-hx, Y(258), -tz), (hx, Y(258) + 0.22, tz), "bz_worn")
    stem0, stem1 = Y(258) + 0.22, Y(175)
    g.box(b, "stem_neck", (-X(85), stem0, -0.3), (X(85), stem1 - 0.02, 0.3), "bz")
    g.box(b, "stem_collar", (-X(95), stem1 - 0.24, -0.34), (X(95), stem1, 0.34), "bz_relief")
    leg_top = stem1 + 0.44
    g.pair(b, "ring_leg", (0.45, stem1, -0.28), (0.73, leg_top, 0.28), "bz_worn")
    g.box(b, "ring_arch_lo", (-0.73, leg_top, -0.28), (0.73, leg_top + 0.24, 0.28), "bz_worn")
    g.box(b, "ring_arch_hi", (-0.5, leg_top + 0.24, -0.28), (0.5, leg_top + 0.48, 0.28), "bz_worn")


def part_cord(g: Gear) -> None:
    """丝绳：从环孔穿出的双股向上收成绳结，绳结上再一小段绳头（手持时握这里）。"""
    b = "cord"
    g.pair(b, "cord_strand", (0.04, Y(150), -0.115), (0.27, Y(60), 0.115), "cord")
    g.box(b, "cord_knot", (-0.36, Y(60), -0.3), (0.36, Y(20), 0.3), "cord")
    g.pair(b, "cord_tail", (0.1, Y(20), -0.115), (0.33, Y(20) + 0.45, 0.115), "cord")


def part_clapper(g: Gear) -> None:
    """铁舌：横在钟腹内壁间的悬桥 + 细杆（穿过口缘中间）+ 梨形锤头（磨亮）。"""
    b = "clapper"
    tz = 0.94 - WALL_T
    g.box(b, "in_bridge", (-0.2, Y(668), -tz), (0.2, Y(642), tz), "iron")
    g.box(b, "in_rod", (-0.15, Y(812) + 0.0, -0.15), (0.15, Y(668), 0.15), "iron")
    g.box(b, "clapper_neck", (-0.15, Y(930), -0.15), (0.15, Y(812), 0.15), "iron")
    g.box(b, "clapper_head_a", (-0.24, Y(965), -0.24), (0.24, Y(930), 0.24), "iron_lt")
    g.box(b, "clapper_head_b", (-0.17, Y(995), -0.17), (0.17, Y(965), 0.17), "iron_lt")


def part_chip(g: Gear) -> None:
    """口缘一处使用磕缺（不对称）：缘边上一小块暗铜缺口 + 缺口旁的亮铜磨痕。"""
    b = "chip"
    tz = max(0.3, 0.97 * math.sqrt(1 - (335 / 430.0) ** 2))
    g.box(b, "chip_notch", (X(310), Y(893) + 0.02, -tz - 0.2), (X(342), Y(893) + 0.3, -tz + 0.1), "bz_dk")
    g.box(b, "chip_wear", (X(342), Y(893) - 0.02, -tz - 0.14), (X(372) - 0.0, Y(893) + 0.22, -tz + 0.1), "bz_worn")


PARTS = {
    "body": part_body,
    "skirt": part_skirt,
    "band": part_band,
    "top": part_top,
    "cord": part_cord,
    "clapper": part_clapper,
    "chip": part_chip,
}

# 参考图 qing_tong_xing_ling_ref.png（1536×1024）：左 = 宽面正视，右 = 窄面侧视
SPEC = MetalSpec(
    gear_id="qing_tong_xing_ling",
    model_name="QingTongXingLing",
    title="青铜行铃 / qing_tong_xing_ling",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=HERE,
    asym=ASYM_BONES,
    min_bones=7,
    round_label="round 3/3",
    texture_overrides=BZ_TEXTURES,
    interpen_bite=0.3,
    ref_front_box=(75, 20, 935, 995),
    ref_side_box=(1100, 20, 1332, 995),
    height_parts=(
        ("绳 + 环", 20, 175, ("ring_", "cord_")),
        ("方颈", 175, 258, ("stem_",)),
        ("钟身（含口缘裙）", 258, 893, ("body_", "skirt_")),
        ("铁舌（口下露出）", 812, 995, ("clapper_",)),
    ),
    width_parts=(
        ("front", "口宽", 75, 935, ("skirt_", "body_")),
        ("front", "肩宽", 277, 733, ("top_cap",)),
        ("front", "环宽", 430, 600, ("ring_",)),
        ("side", "口厚", 1100, 1325, ("body_", "skirt_")),
        ("side", "纽颈厚", 1175, 1245, ("stem_",)),
    ),
    # 手持：右臂前抬 90°，握住纽上的丝绳（局部 y 取绳结附近），绳在拳心沿臂轴往前 1px；铃垂在拳下，不碰手臂
    hold={"arm_deg": 90.0, "anchor": (0.0, Y(30) + 0.2, 0.0), "lead": 1.0, "skip_arm": ("ring_", "stem_", "cord_", "top_")},
)

if __name__ == "__main__":
    metal_review.main(SPEC)
