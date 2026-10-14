#!/usr/bin/env python3
"""浮灵母（fu_ling_mu）水母状生物模型程序化生成器 —— Round 1/3。

依据 worldview.md §十三 灵泉湿地 / 灵眼：漂浮在灵眼附近的水母状生物。

外形（任务卡第五批 + 参考图 exploded.png）：
  · 伞盖四层阶梯叠成：顶盘（十二根放射脉纹像一朵花）→ 三层逐级放大的环壁；每层外壁十二条深色竖纹，
    台面上十二道放射深纹；整体浅奶白 / 浅灰蓝，靠深色纹路读轮廓
  · 伞缘垂十二片裙瓣，每瓣中线一道深纹
  · 伞下一根细柱从顶盘内面垂到中心，柱端是一团发光灵核（青白，唯一的亮处）
  · 灵核下接口腕管，四条褶边口腕两节下垂
  · 六束触须挂在伞下沿，每束三节、长短不一，带深色中线和关节珠，末梢微卷
  · 几粒漂浮的光点（刻意悬空）

骨骼：伞盖 4 层 + 十二片裙瓣 + 灵核 + 口腕管 + 口腕 4×2 + 触须 6×3 + 光点，总数远超 ≥ 20。

每个编号部件是一个 part_*()，可单独出预览。

用法：
  python3 modelScript/creatures/fu_ling_mu/gen_fu_ling_mu.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/fu_ling_mu/gen_fu_ling_mu.py --self-test # 门禁差分自证
  python3 modelScript/creatures/fu_ling_mu/gen_fu_ling_mu.py --list

坐标约定：16 单位 = 1 格，地面 y=0（触须梢刚好点地），头 / 正面朝 -Z，左侧是 -X（与髓蛀蜈同）。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig, shaft_box  # noqa: E402

CREATURE_DIR = REPO / "modelScript" / "creatures" / "fu_ling_mu"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/fu_ling_mu")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "FuLingMu.bbmodel"
BBMODEL_COPY = MODELS_DIR / "FuLingMu.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "jelly_cream": (226, 224, 208),   # 浅奶白伞肉
    "jelly_grey": (176, 188, 190),    # 浅灰蓝阴面
    "vein_dark": (46, 54, 68),        # 深色脉纹
    "vein_slate": (98, 110, 126),     # 次级纹、关节珠
    "frill_tan": (208, 196, 170),     # 口腕褶边
    "glow_teal": (140, 236, 220),     # 灵核
    "glow_white": (240, 255, 250),    # 灵核亮点、光点
}
JELLY_MATS = ("jelly_cream", "jelly_grey", "frill_tan")

# ---------------------------------------------------------------------------
# 体型参数（px；16 = 1 格）
# ---------------------------------------------------------------------------
MIN_MOVING_BONES = 20
TAN15 = math.tan(math.radians(15.0))

# 伞盖六层：(骨名, 外切圆半径 A, 径向厚 t, y0, y1)，自下而上收成一个圆鼓的穹顶；
# 上层的内径必须落在下层环壁覆盖范围内，台阶才咬得住
TIERS = [
    ("bell_5", 20.0, 6.0, 29.0, 31.2),
    ("bell_4", 18.4, 5.6, 31.2, 33.4),
    ("bell_3", 16.0, 5.2, 33.4, 35.6),
    ("bell_2", 13.2, 4.8, 35.6, 37.8),
    ("bell_1", 9.8, 4.4, 37.8, 40.0),
    ("bell_0", 6.0, 6.0, 40.0, 42.5),     # 顶盘：t = A，中心合拢
]
BELL_LOW = TIERS[0][0]                    # 最下一层：裙瓣、触须、光点都挂它
SIDED_STEMS = (0, 30, 60, 300, 330)       # 右半边的边心角（°，x = cos，z = sin）；左半边是镜像
CENTER_STEMS = (90, 270)                  # 落在中线上的边心角（z = ±）

ROD_Y0, ROD_Y1 = 30.0, 40.0               # 灵核柱：自顶盘内面（y = 40）垂到灵核顶
CORE_Y0, CORE_Y1 = 24.0, 30.0
MANU_Y0, MANU_Y1 = 19.0, 24.0
TENT_R = 15.5                             # 触须挂点半径
TENT_Y = 28.0
TENT_SEGS = 5                             # 每束触须的节数
TENT_LEN = (6.0, 6.2, 6.0, 5.4, 4.2)     # 各节垂长，合计 27.8（触须梢刚好点地）
TENT_WAVE = 1.5                           # 触须左右蜿蜒的幅度（px）
TENT_SCALE = {30: 0.9, 330: 0.8, 90: 1.0, 270: 0.72}   # 各束长度系数（按词干配对，左右一致）
TENT_BUNDLES = ((30, False), (330, False), (90, True), (270, True))
ORAL_STEMS = (45, 315)


def side_tag(s: float) -> str:
    return "_l" if s < 0 else "_r"


def lerp3(a, b, t: float):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def fold(ang: int) -> tuple[int, str]:
    """边心角 → (词干, 侧后缀)。左右镜像的两条边共用同一个词干；中线上的没有后缀。"""
    c = math.cos(math.radians(ang))
    if abs(c) < 1e-9:
        return ang % 360, ""
    return (ang if c > 0 else (180 - ang) % 360), ("_r" if c > 0 else "_l")


def frame(stem: int, s: float):
    """右半边词干 stem 在 s 侧的径向 u、切向 t（水平单位向量）。左侧 = 右侧 x 取反。"""
    r = math.radians(stem)
    return (s * math.cos(r), 0.0, math.sin(r)), (-s * math.sin(r), 0.0, math.cos(r))


def placed(rig_stems: tuple[int, ...], center: tuple[int, ...]):
    """遍历 (词干, s, 后缀)：右半边词干各出左右两份，中线词干只出一份（s = +1，无后缀）。"""
    for stem in rig_stems:
        for s in (-1.0, 1.0):
            yield stem, s, side_tag(s)
    for stem in center:
        yield stem, 1.0, ""


def pt(u, r: float, y: float, t=None, off: float = 0.0):
    p = (u[0] * r, y, u[2] * r)
    if t is not None:
        p = (p[0] + t[0] * off, p[1], p[2] + t[2] * off)
    return p


# ---------------------------------------------------------------------------
# 骨骼
# ---------------------------------------------------------------------------
def tent_points(stem: int, s: float, wavy: bool):
    """触须 TENT_SEGS+1 个折点：挂点 → 各节末端。每节略向外飘，wavy 的束再按正弦左右蜿蜒，末梢卷回。"""
    u, t = frame(stem, s)
    mult = TENT_SCALE[stem]
    amp = TENT_WAVE if wavy else 0.5
    pts = [(u[0] * TENT_R, TENT_Y, u[2] * TENT_R)]
    for k in range(TENT_SEGS):
        side = amp * (math.sin(math.pi * (k + 1) / 2.0) if wavy else 0.0) * (1.0 if k < TENT_SEGS - 1 else -0.6)
        prev = pts[-1]
        pts.append((prev[0] + u[0] * 0.6 + t[0] * side, prev[1] - TENT_LEN[k] * mult, prev[2] + u[2] * 0.6 + t[2] * side))
    return pts, u, t


def oral_points(stem: int, s: float):
    u, t = frame(stem, s)
    return [pt(u, 1.8, 21.5), pt(u, 5.5, 16.5), pt(u, 8.0, 11.5)], u, t


def lappet_points(stem: int, s: float):
    u, t = frame(stem, s)
    a = pt(u, 19.0, 29.0)
    b = pt(u, 22.0, 24.6)
    return a, b, u, t


def define_bones(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0), None)
    prev = "root"
    for name, _, _, y0, _ in TIERS:
        prev = rig.bone(name, (0.0, y0, 0.0), prev)
    rig.bone("core", (0.0, (CORE_Y0 + CORE_Y1) / 2, 0.0), TIERS[-1][0])
    rig.bone("manu", (0.0, MANU_Y1, 0.0), "core")
    for stem, s, tag in placed(ORAL_STEMS, ()):
        pts, _, _ = oral_points(stem, s)
        rig.bone(f"oral_{stem}{tag}_a", pts[0], "manu")
        rig.bone(f"oral_{stem}{tag}_b", pts[1], f"oral_{stem}{tag}_a")
    for stem, s, tag in placed(SIDED_STEMS, CENTER_STEMS):
        a, _, _, _ = lappet_points(stem, s)
        rig.bone(f"lappet_{stem}{tag}", a, BELL_LOW)
    for stem, centre in TENT_BUNDLES:
        for s in ((1.0,) if centre else (-1.0, 1.0)):
            tag = "" if centre else side_tag(s)
            pts, _, _ = tent_points(stem, s, not centre)
            prev = BELL_LOW
            for k in range(TENT_SEGS):
                prev = rig.bone(f"tent_{stem}{tag}_{k}", pts[k], prev)
    rig.bone("motes", (0.0, 20.0, 0.0), BELL_LOW)


def build_rig() -> Rig:
    rig = Rig(MATS)
    define_bones(rig)
    return rig


# ---------------------------------------------------------------------------
# 几何小工具
# ---------------------------------------------------------------------------
Vec = tuple[float, float, float]


def bar(rig: Rig, bone: str, name: str, a: Vec, b: Vec, rx: float, rz: float, *,
        mat: str, up: Vec | None = None) -> None:
    """端点精确落在 a、b 的定向柱。up 给定时局部 +Z（半厚 rz）指向 up。"""
    frm, to, rot, org = shaft_box(a, b, rx, rz, 0.0, up)
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


def box(rig: Rig, bone: str, name: str, lo: Vec, hi: Vec, *, mat: str) -> None:
    rig.cube(bone, name, lo, hi, mat=mat)


def ring_h(rig: Rig, bone: str, prefix: str, y0: float, y1: float, a: float, t: float, mat_for) -> None:
    """水平十二边形环壁：外切圆半径 a、径向厚 t、高 y0..y1；边心角 0°,30°,…（x = cos，z = sin）。"""
    centre_apothem = a - t / 2
    vertex_r = centre_apothem / math.cos(math.radians(15.0))
    yc = (y0 + y1) / 2
    for k in range(12):
        ang = 30 * k
        pa = (vertex_r * math.cos(math.radians(ang - 15)), yc, vertex_r * math.sin(math.radians(ang - 15)))
        pb = (vertex_r * math.cos(math.radians(ang + 15)), yc, vertex_r * math.sin(math.radians(ang + 15)))
        stem, tag = fold(ang)
        frm, to, rot, org = shaft_box(pa, pb, t / 2, (y1 - y0) / 2, t / 2 * TAN15, (0.0, 1.0, 0.0))
        rig.cube(bone, f"{prefix}_a{stem}{tag}", frm, to, rot=rot, org=org, mat=mat_for(ang))


# ===========================================================================
# 1. 伞盖：四层阶梯环壁
# ===========================================================================
def part_bell(rig: Rig) -> None:
    for n, (bone, a, t, y0, y1) in enumerate(TIERS):
        # 同层十二边交替奶白 / 灰蓝，读出伞肉的明暗；各层再错开一档
        ring_h(rig, bone, f"tier{n}", y0, y1, a, t, lambda ang, n=n: "jelly_grey" if (ang // 30 + n) % 2 else "jelly_cream")


# ===========================================================================
# 2. 脉纹：外壁十二条竖纹 + 台面放射纹 + 顶盘花瓣
# ===========================================================================
def part_veins(rig: Rig) -> None:
    for n, (bone, a, t, y0, y1) in enumerate(TIERS):
        for stem, s, tag in placed(SIDED_STEMS, CENTER_STEMS):
            u, tv = frame(stem, s)
            # 外壁：中线一条深纹（贴着外面，厚 0.5）+ 两侧各一条细板岩纹
            bar(rig, bone, f"wall{n}_{stem}{tag}", pt(u, a + 0.25, y0 + 0.3), pt(u, a + 0.25, y1 - 0.3), 0.7, 0.25, mat="vein_dark", up=u)
            off = a * TAN15 * 0.55
            for d in (-1.0, 1.0):
                p0, p1 = pt(u, a + 0.2, y0 + 0.6, tv, d * off), pt(u, a + 0.2, y1 - 0.6, tv, d * off)
                if tag == "":
                    k_tag = "_r" if p0[0] > 0 else "_l"
                    name = f"wallsec{n}_{stem}{k_tag}"
                else:
                    name = f"wallsec{n}_{stem}_{'ab'[d > 0]}{tag}"
                bar(rig, bone, name, p0, p1, 0.35, 0.2, mat="vein_slate", up=u)
            # 台面：自内向外的放射深纹，落在本层顶面外露的那一圈上（或顶盘整个顶面）
            r_in = TIERS[n - 1][1] + 0.2 if n > 0 else None
            r_in = (TIERS[n + 1][1] + 0.2) if n < len(TIERS) - 1 else 0.8
            r_out = a - 0.3 if n < len(TIERS) - 1 else a - 0.4
            bar(rig, bone, f"spoke{n}_{stem}{tag}", pt(u, r_in, y1 + 0.25), pt(u, r_out, y1 + 0.25), 0.7, 0.25, mat="vein_dark", up=(0.0, 1.0, 0.0))
    box(rig, TIERS[-1][0], "hub", (-1.2, TIERS[-1][4], -1.2), (1.2, TIERS[-1][4] + 0.9, 1.2), mat="vein_slate")


# ===========================================================================
# 3. 裙瓣：伞缘垂下的十二片
# ===========================================================================
def part_lappets(rig: Rig) -> None:
    for stem, s, tag in placed(SIDED_STEMS, CENTER_STEMS):
        a, b, u, t = lappet_points(stem, s)
        hw = (20.0 * TAN15 - 0.7) * 0.72
        bone = f"lappet_{stem}{tag}"
        mid = lerp3(a, b, 0.5)
        bar(rig, bone, f"lap_top_{stem}{tag}", a, mid, hw, 0.4, mat="jelly_cream" if stem % 60 == 0 else "jelly_grey", up=u)
        low = (b[0] + u[0] * 0.0, b[1], b[2])
        bar(rig, bone, f"lap_low_{stem}{tag}", mid, low, hw * 0.82, 0.4, mat="jelly_grey" if stem % 60 == 0 else "jelly_cream", up=u)
        # 瓣中线深纹，贴在外面
        o = 0.6
        bar(rig, bone, f"lap_vein_{stem}{tag}", (a[0] + u[0] * o, a[1] - 0.3, a[2] + u[2] * o), (low[0] + u[0] * o, low[1] + 0.4, low[2] + u[2] * o),
            0.45, 0.2, mat="vein_dark", up=u)


# ===========================================================================
# 4. 灵核 + 吊柱
# ===========================================================================
def part_core(rig: Rig) -> None:
    box(rig, TIERS[-1][0], "rod", (-0.7, ROD_Y0, -0.7), (0.7, ROD_Y1, 0.7), mat="vein_slate")
    box(rig, "core", "core_main", (-3.0, CORE_Y0, -3.0), (3.0, CORE_Y1, 3.0), mat="glow_teal")
    # 四面各一枚亮点凸起（贴在面上，不嵌进去；底面被口腕管接住，不放）
    cy = (CORE_Y0 + CORE_Y1) / 2
    box(rig, "core", "core_nubx_r", (3.0, cy - 0.9, -0.9), (4.6, cy + 0.9, 0.9), mat="glow_white")
    box(rig, "core", "core_nubx_l", (-4.6, cy - 0.9, -0.9), (-3.0, cy + 0.9, 0.9), mat="glow_white")
    box(rig, "core", "core_nubz0", (-0.9, cy - 0.9, 3.0), (0.9, cy + 0.9, 4.6), mat="glow_white")
    box(rig, "core", "core_nubz1", (-0.9, cy - 0.9, -4.6), (0.9, cy + 0.9, -3.0), mat="glow_white")


# ===========================================================================
# 5. 口腕管 + 四条口腕
# ===========================================================================
def part_oral(rig: Rig) -> None:
    box(rig, "manu", "manu_tube", (-2.2, MANU_Y0, -2.2), (2.2, MANU_Y1, 2.2), mat="frill_tan")
    for stem, s, tag in placed(ORAL_STEMS, ()):
        pts, u, t = oral_points(stem, s)
        for k in range(2):
            bone = f"oral_{stem}{tag}_{'ab'[k]}"
            a, b = pts[k], pts[k + 1]
            hw = 2.0 - 0.4 * k
            bar(rig, bone, f"oral_main{k}_{stem}{tag}", a, b, hw, 0.5, mat="frill_tan", up=u)
            # 外面一条深中线
            o = 0.7
            bar(rig, bone, f"oral_vein{k}_{stem}{tag}", (a[0] + u[0] * o, a[1], a[2] + u[2] * o), (b[0] + u[0] * o, b[1], b[2] + u[2] * o),
                0.4, 0.2, mat="vein_slate", up=u)
            # 两边的褶边瘤：沿臂三粒，贴在两侧边上
            for n, tt in enumerate((0.2, 0.5, 0.8)):
                c = lerp3(a, b, tt)
                for d in (-1.0, 1.0):
                    cx = (c[0] + t[0] * d * (hw + 0.5), c[1], c[2] + t[2] * d * (hw + 0.5))
                    name = f"oral_frill{k}_{stem}{n}{tag}" if d < 0 else f"oral_frill{k}_{stem}{n}b{tag}"
                    box(rig, bone, name, (cx[0] - 0.5, cx[1] - 0.6, cx[2] - 0.5), (cx[0] + 0.5, cx[1] + 0.6, cx[2] + 0.5), mat="jelly_cream")


# ===========================================================================
# 6. 触须：六束，每束三节 + 深中线 + 关节珠
# ===========================================================================
def part_tentacles(rig: Rig) -> None:
    for stem, centre in TENT_BUNDLES:
        for s in ((1.0,) if centre else (-1.0, 1.0)):
            tag = "" if centre else side_tag(s)
            pts, u, t = tent_points(stem, s, not centre)
            # 挂座：贴在伞下沿底面（y = 29）的深色小块
            box(rig, f"tent_{stem}{tag}_0", f"tent_root_{stem}{tag}",
                (pts[0][0] - 1.2, 28.0, pts[0][2] - 1.2), (pts[0][0] + 1.2, 29.0, pts[0][2] + 1.2), mat="vein_dark")
            for k in range(TENT_SEGS):
                bone = f"tent_{stem}{tag}_{k}"
                a, b = pts[k], pts[k + 1]
                hw = 1.5 - 0.17 * k
                bar(rig, bone, f"tent_seg{k}_{stem}{tag}", a, b, hw, 0.55, mat="jelly_cream" if k % 2 == 0 else "jelly_grey", up=u)
                o = 0.75
                bar(rig, bone, f"tent_vein{k}_{stem}{tag}", (a[0] + u[0] * o, a[1], a[2] + u[2] * o), (b[0] + u[0] * o, b[1], b[2] + u[2] * o),
                    0.4, 0.2, mat="vein_dark", up=u)
                if k < TENT_SEGS - 1:
                    # 关节珠：挂下一节骨，落在本节末端（贴着不嵌）
                    nb = f"tent_{stem}{tag}_{k + 1}"
                    box(rig, nb, f"tent_bead{k}_{stem}{tag}", (b[0] - 0.9, b[1] - 0.9, b[2] - 0.9), (b[0] + 0.9, b[1] + 0.9, b[2] + 0.9), mat="vein_slate")


# ===========================================================================
# 7. 漂浮光点（刻意悬空）
# ===========================================================================
MOTES = ((-11.0, 21.0, -6.0), (9.5, 17.5, 7.0), (-5.0, 12.0, 11.5), (13.0, 9.5, -9.0), (3.0, 7.0, -12.5), (-14.0, 15.0, 6.0))


def part_motes(rig: Rig) -> None:
    for k, (x, y, z) in enumerate(MOTES):
        h = 0.55 + 0.1 * (k % 3)
        box(rig, "motes", f"mote{k}", (x - h, y - h, z - h), (x + h, y + h, z + h), mat="glow_white")


PARTS = {
    "bell": ("1. 伞盖：六层逐级收边的穹顶环壁", part_bell),
    "veins": ("2. 脉纹：外壁竖纹 / 台面放射纹 / 顶盘花瓣", part_veins),
    "lappets": ("3. 十二片裙瓣", part_lappets),
    "core": ("4. 灵核 + 吊柱", part_core),
    "oral": ("5. 口腕管 + 四条褶边口腕", part_oral),
    "tentacles": ("6. 六束五节蜿蜒触须", part_tentacles),
    "motes": ("7. 漂浮光点", part_motes),
}


def build_full() -> Rig:
    rig = build_rig()
    for _, func in PARTS.values():
        func(rig)
    return rig


def moving_bones(rig: Rig) -> list[str]:
    """带几何的骨（root 本身不动，不算）。"""
    return [n for n in rig.bone_order if n != "root" and rig.bones[n]["children"]]


# ===========================================================================
# 门禁
# ===========================================================================
# 光点是刻意悬空的；motes 骨（随机散落）不对称，镜像门豁免。
ASYM = ("motes",)
FREE = frozenset(f"mote{k}" for k in range(len(MOTES)))

# 穿模门按 AABB 判：十二边形斜边、斜垂的触须 / 口腕的 AABB 很大，会和同层的深纹 / 关节珠虚报互穿。
# 伞肉 × 脉纹、伞肉 × 伞肉放行；灵核（青 / 白）与其余件的穿模仍照常查。
_VEIN = ("vein_dark", "vein_slate")
SOFT_OVER = frozenset(
    {frozenset((a, b)) for a in JELLY_MATS for b in JELLY_MATS if a != b}
    | {frozenset((a, v)) for a in JELLY_MATS for v in _VEIN}
    | {frozenset(_VEIN)}
)


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制，去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates("浮灵母 / fu_ling_mu", tuple(MATS), asym=ASYM, soft_over=SOFT_OVER, free_floating=FREE)


def self_test() -> None:
    print("运行 gen_fu_ling_mu.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    moving = moving_bones(rig)
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert -0.15 < lo[1] < 1.5, f"触须梢应刚好点地：最低点 y={lo[1]:.3f}"
    print(f"  [OK] 可动骨 {len(moving)} cube {len(rig.elements)} 宽 {hi[0] - lo[0]:.1f}px 深 {hi[2] - lo[2]:.1f}px 高 {hi[1]:.1f}px 最低 y={lo[1]:.2f}")
    total = GATES.report(rig)
    assert total == 0, f"{total} 处门禁违例"
    broken = GATES.self_test(rig)
    assert broken == 0, f"{broken} 道门没有鉴别力（注入缺陷未被报出）"
    print("✓ 差分自证全绿")


# ===========================================================================
# 渲染：逐件预览 + 六视角接触表
# ===========================================================================
BG = (119, 119, 119)


def _tmp_model(rig: Rig, name: str) -> Path:
    path = CREATURE_DIR / f".{name}.tmp.bbmodel"
    path.write_text(json.dumps(rig.bbmodel(name, namespace="bong"), ensure_ascii=False), encoding="utf-8")
    return path


def focus_of(rig: Rig, margin: float = 1.1) -> tuple[tuple[float, float, float], float]:
    lo, hi = rig.bounds()
    center = tuple((lo[i] + hi[i]) / 2 for i in range(3))
    span = max(hi[i] - lo[i] for i in range(3)) * margin
    return center, span


# yaw 方向：yaw=0 相机在 +Z（照到背面）；yaw=180 相机在 -Z（照到正面）；
# yaw=90 相机在 -X（照到左侧）；yaw=-90 相机在 +X（照到右侧）；pitch<0 相机在下方。
def six_views(rig: Rig):
    whole = focus_of(rig)
    return [
        ("正面 FRONT：相机在 -Z", 180.0, 0.0, whole),
        ("背面 REAR：相机在 +Z", 0.0, 0.0, whole),
        ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0, whole),
        ("左侧面 LEFT：相机在 -X", 90.0, 0.0, whole),
        ("右侧面 RIGHT：相机在 +X", -90.0, 0.0, whole),
        ("仰视 BOTTOM：相机在 -Y，看口腕与灵核", 180.0, -60.0, whole),
    ]


FOCUS = {
    "bell": ((0.0, 36.0, 0.0), 50.0),
    "veins": ((0.0, 36.0, 0.0), 50.0),
    "lappets": ((0.0, 28.0, 0.0), 50.0),
    "core": ((0.0, 30.0, 0.0), 26.0),
    "oral": ((0.0, 21.0, 0.0), 32.0),
    "tentacles": ((0.0, 14.0, 0.0), 44.0),
    "motes": ((0.0, 14.0, 0.0), 40.0),
}


def render_part_previews() -> None:
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(16), load_font(13)
    print("渲染逐件预览 ...")
    for i, (key, (title, func)) in enumerate(PARTS.items(), start=1):
        rig = build_rig()
        func(rig)
        path = _tmp_model(rig, f"part_{key}")
        focus = FOCUS[key]
        views = [
            ("正面 FRONT：相机在 -Z", 180.0, 0.0),
            ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0),
            ("仰视：相机在 -Y 侧下方", 180.0, -60.0),
        ]
        cell = 440
        sheet = Image.new("RGB", (cell * 3 + 20, cell + 56), (32, 33, 36))
        d = ImageDraw.Draw(sheet)
        d.text((10, 6), f"部件 {i}: {title}（{len(rig.elements)} cubes）", fill=(235, 235, 235), font=font_t)
        for j, (label, yaw, pitch) in enumerate(views):
            im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)
            sheet.paste(im, (5 + j * (cell + 5), 32))
            d.text((10 + j * (cell + 5), cell + 36), label, fill=(210, 210, 210), font=font_s)
        sheet.save(PARTS_DIR / f"part_{i}_{key}.png")
        path.unlink(missing_ok=True)
        print(f"  ✓ part_{i}_{key}.png")


def six_view_sheet(rig: Rig, path: Path, header: str, cell: int = 520) -> Image.Image:
    """2 行 × 3 列方格：上排 正 / 背 / 3/4，下排 左 / 右 / 仰视。"""
    font_h, font_l = load_font(18), load_font(13)
    sheet = Image.new("RGB", (cell * 3 + 20, 34 + (cell + 5) * 2 + 5), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), header, fill=(235, 235, 235), font=font_h)
    for n, (label, yaw, pitch, focus) in enumerate(six_views(rig)):
        px, py = 5 + (n % 3) * (cell + 5), 34 + (n // 3) * (cell + 5)
        im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)
        sheet.paste(im, (px, py))
        d.rectangle([px, py, px + 400, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def bone_tree_lines(rig: Rig) -> list[str]:
    """骨骼树：十二片裙瓣同构，只展开前两片，其余一行带过。"""
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines: list[str] = []
    skipped_lappets = 0

    def walk(name: str, depth: int) -> None:
        nonlocal skipped_lappets
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{name} ({len(rig.bones[name]['children'])})")
        for kid in children.get(name, []):
            walk(kid, depth + 1)

    walk("root", 0)
    return lines


def bone_tree_panel(rig: Rig, height: int) -> Image.Image:
    lines = bone_tree_lines(rig)
    font_h, font_s, font_t = load_font(16), load_font(13), load_font(11)
    step = 13
    panel = Image.new("RGB", (400, max(height, 90 + len(lines) * step)), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.text((12, 10), "骨骼层级树（括号内为挂载 cube 数）", fill=(180, 235, 200), font=font_h)
    d.text((12, 34), f"骨骼 {len(rig.bones)}（可动 {len(moving_bones(rig))}，下限 ≥ {MIN_MOVING_BONES}）", fill=(240, 210, 150), font=font_s)
    d.text((12, 54), f"cube {len(rig.elements)}", fill=(190, 210, 235), font=font_s)
    y = 76
    for ln in lines:
        d.text((12, y), ln, fill=(230, 230, 235), font=font_t)
        y += step
    return panel


def render_check(rig: Rig, now_path: Path) -> None:
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    font_h = load_font(18)
    sheet_now = six_view_sheet(rig, now_path, f"NOW {ROUND_TAG}｜{len(rig.bones)} 骨 / {len(rig.elements)} cube")
    sheet_now.save(REVIEW_DIR / "render.png")
    sheet_now.save(REVIEW_DIR / "contact_sheet.png")
    tree = bone_tree_panel(rig, sheet_now.height)
    tree.save(REVIEW_DIR / "bones_tree.png")

    row_h = max(sheet_now.height, tree.height)
    ref = Image.open(EXPLODED_REF).convert("RGB") if EXPLODED_REF.exists() else Image.new("RGB", (400, row_h), (50, 50, 50))
    ref = ref.resize((int(ref.width * row_h / ref.height), row_h), Image.Resampling.LANCZOS)
    cols = [
        ("爆炸参考图 EXPLODED REF", ref),
        (f"NOW：{ROUND_TAG}（{len(rig.elements)} cube / {len(rig.bones)} 骨）", sheet_now),
        (f"骨骼树（{len(rig.bones)} 骨）", tree),
    ]
    total_w = sum(c.width for _, c in cols) + 10 * (len(cols) + 1)
    canvas = Image.new("RGB", (total_w, row_h + 48), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    x = 10
    for title, im in cols:
        canvas.paste(im, (x, 38))
        d.text((x + 4, 8), title, fill=(215, 205, 180), font=font_h)
        x += im.width + 10
    canvas.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png: {REVIEW_DIR / 'check.png'}")


def main() -> None:
    ap = argparse.ArgumentParser(description="浮灵母 (fu_ling_mu) 生成器 Round 1/3")
    ap.add_argument("--self-test", action="store_true", help="门禁差分自证")
    ap.add_argument("--list", action="store_true", help="列出编号部件")
    ap.add_argument("--no-render", action="store_true", help="只写 bbmodel，不出预览图")
    args = ap.parse_args()

    if args.list:
        for key, (title, _) in PARTS.items():
            print(f"  {key:10s} {title}")
        return
    if args.self_test:
        self_test()
        return

    self_test()
    rig = build_full()
    doc = rig.bbmodel("FuLingMu", namespace="bong")
    for out in (BBMODEL_OUT, BBMODEL_COPY):
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"✓ 全身模型 {BBMODEL_OUT}（{len(rig.elements)} cube / {len(rig.bones)} 骨）")
    if args.no_render:
        return
    render_part_previews()
    render_check(rig, BBMODEL_OUT)


if __name__ == "__main__":
    main()
