#!/usr/bin/env python3
"""劫雷鸦（jie_lei_ya）飞行类生物模型程序化生成器 —— Round 1/3。

依据 worldview.md §八 天劫与劫气标记：天道精准定点清除，成群跟着身负劫气的修士盘旋的焦黑大乌鸦。

外形（任务卡第五批 + 参考图 exploded.png）：
  · 大乌鸦，展翼约 120px（7 格半），身长约 30px；焦黑羽毛，每片羽毛尖端一截焦褐、卷边；
  · 羽毛全是逐片的薄刃：躯干七条羽列叠成鳞，颈有羽领，头顶一撮冠羽，翅尖初级羽分四束炸开、尖端劈成两叉
  · 每侧翅：上臂 / 前臂 / 手 / 小翼羽 / 内侧次级羽 / 外侧次级羽 / 四束初级羽，共 11 骨
  · 雷痕：蓝白色细折线贴在羽面和胸侧，一枚白点的眼
  · 腿：羽裤 + 裸露的黑角质跗，三前趾一后趾，每趾带钩爪

骨骼：脊 2 + 颈 2 + 头 / 上喙 / 下喙 / 冠 + 双翼 22 + 尾 5 + 双腿 10，总数远超飞行类下限 24。

每个编号部件是一个 part_*()，可单独出预览。

用法：
  python3 modelScript/creatures/jie_lei_ya/gen_jie_lei_ya.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/jie_lei_ya/gen_jie_lei_ya.py --self-test # 门禁差分自证
  python3 modelScript/creatures/jie_lei_ya/gen_jie_lei_ya.py --list

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X（与髓蛀蜈同）。静置姿态：双翼向两侧平展，脚爪踩地。
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "jie_lei_ya"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/jie_lei_ya")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "JieLeiYa.bbmodel"
BBMODEL_COPY = MODELS_DIR / "JieLeiYa.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "feather_black": (30, 28, 34),    # 焦黑主羽
    "feather_dark": (50, 46, 56),     # 稍浅的覆羽 / 躯干芯
    "char_brown": (96, 64, 44),       # 羽尖焦褐
    "ash_grey": (116, 114, 118),      # 灰烬色
    "bolt_blue": (160, 208, 255),     # 雷痕
    "eye_white": (250, 250, 255),     # 眼里一点白光
    "horn": (78, 64, 56),             # 喙、跗
    "horn_dark": (36, 32, 34),        # 爪尖、眼窝、跗鳞
}
FEATHER_MATS = ("feather_black", "feather_dark", "char_brown", "ash_grey")

# ---------------------------------------------------------------------------
# 体型参数（px；16 = 1 格）
# ---------------------------------------------------------------------------
BODY_Y = 14.0                       # 躯干体轴高度
CHEST_Z, BELLY_Z, RUMP_Z = -8.5, -0.5, 6.0
RUMP_END_Z = 12.0
MIN_MOVING_BONES = 24

# 右翼关节点（左翼取 x 反号）：肩 → 肘 → 腕 → 手尖
WING_S = (5.5, 17.0, -4.0)
WING_E = (17.5, 18.2, 0.5)
WING_W = (30.0, 20.0, -3.5)
WING_T = (40.0, 20.8, -5.5)

TAIL_Z0 = RUMP_END_Z
HIP = (3.4, 10.2, 2.5)
KNEE = (3.9, 7.4, 0.0)
ANKLE = (3.6, 4.4, 5.0)
FOOT = (3.6, 0.9, 3.2)

TOE_OUT_ANGLES = (-22.0, 0.0, 22.0)  # 三前趾向外偏角（°），负为内侧


def side_tag(s: float) -> str:
    return "_l" if s < 0 else "_r"


def sx(p: tuple[float, float, float], s: float) -> tuple[float, float, float]:
    return (p[0] * s, p[1], p[2])


def lerp3(a, b, t: float):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def along(a, b, d: float):
    """从 a 朝 b 走 d 个单位的点。"""
    return lerp3(a, b, d / math.dist(a, b))


# ---------------------------------------------------------------------------
# 骨骼
# ---------------------------------------------------------------------------
PRIMARY_T = (0.12, 0.38, 0.62, 0.86)       # 初级羽束根部沿手骨的位置
PRIMARY_ANGLE = (62.0, 40.0, 22.0, 4.0)    # 各束羽向后偏离正外侧的角度（°）
PRIMARY_LEN = (19.0, 22.0, 25.0, 24.0)


def define_bones(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0), None)
    rig.bone("spine_0", (0.0, BODY_Y, -3.0), "root")
    rig.bone("spine_1", (0.0, BODY_Y, 5.0), "spine_0")
    rig.bone("neck_0", (0.0, 17.0, -9.0), "spine_0")
    rig.bone("neck_1", (0.0, 19.0, -11.5), "neck_0")
    rig.bone("head", (0.0, 21.5, -15.0), "neck_1")
    rig.bone("beak_up", (0.0, 21.4, -18.0), "head")
    rig.bone("beak_low", (0.0, 19.0, -18.0), "head")
    rig.bone("crest", (0.0, 24.8, -13.0), "head")
    for s in (-1.0, 1.0):
        t = side_tag(s)
        rig.bone(f"wing_arm{t}", sx(WING_S, s), "spine_0")
        rig.bone(f"wing_fore{t}", sx(WING_E, s), f"wing_arm{t}")
        rig.bone(f"wing_hand{t}", sx(WING_W, s), f"wing_fore{t}")
        rig.bone(f"alula{t}", sx(WING_W, s), f"wing_hand{t}")
        rig.bone(f"sec_in{t}", sx(lerp3(WING_S, WING_E, 0.5), s), f"wing_arm{t}")
        rig.bone(f"sec_out{t}", sx(lerp3(WING_E, WING_W, 0.5), s), f"wing_fore{t}")
        for g, tt in enumerate(PRIMARY_T):
            rig.bone(f"prim_{g}{t}", sx(lerp3(WING_W, WING_T, tt), s), f"wing_hand{t}")
        rig.bone(f"leg_thigh{t}", sx(HIP, s), "spine_1")
        rig.bone(f"leg_shin{t}", sx(KNEE, s), f"leg_thigh{t}")
        rig.bone(f"leg_tarsus{t}", sx(ANKLE, s), f"leg_shin{t}")
        rig.bone(f"toes_front{t}", sx(FOOT, s), f"leg_tarsus{t}")
        rig.bone(f"toe_back{t}", sx(FOOT, s), f"leg_tarsus{t}")
    rig.bone("tail_0", (0.0, 14.2, TAIL_Z0), "spine_1")
    rig.bone("tail_1", (0.0, 14.0, TAIL_Z0 + 4.5), "tail_0")
    rig.bone("tail_c", (0.0, 14.0, TAIL_Z0 + 7.0), "tail_1")
    rig.bone("tail_l", (0.0, 14.0, TAIL_Z0 + 7.0), "tail_1")
    rig.bone("tail_r", (0.0, 14.0, TAIL_Z0 + 7.0), "tail_1")


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


def mbox(rig: Rig, bone: str, stem: str, s: float, x0: float, x1: float, y0: float, y1: float, z0: float, z1: float, *,
         mat: str) -> None:
    """镜像盒：右侧 x 取 x0..x1（≥0），左侧自动取反。"""
    lo, hi = (x0, x1) if s > 0 else (-x1, -x0)
    box(rig, bone, f"{stem}{side_tag(s)}", (lo, y0, z0), (hi, y1, z1), mat=mat)


def rotate_about_y(v: Vec, deg: float) -> Vec:
    """绕 +Y 轴转 deg°（俯视逆时针）。"""
    r = math.radians(deg)
    return (v[0] * math.cos(r) + v[2] * math.sin(r), v[1], -v[0] * math.sin(r) + v[2] * math.cos(r))


def feather(rig: Rig, bone: str, stem: str, tag: str, a: Vec, b: Vec, hw: float, ht: float, *, mat: str, up: Vec,
            tip_len: float = 0.22, tip_split: float = 0.0) -> None:
    """一片羽毛：主羽片（a → 近端）+ 焦褐尖（近端 → b）。

    tip_split > 0 时尖端劈成两叉，向两侧各偏 tip_split°（炸开的初级羽）。
    名字 = stem + 部位 + tag，tag 是 "" / "_l" / "_r"，保证镜像门按词干配对。
    """
    m = lerp3(a, b, 1.0 - tip_len)
    bar(rig, bone, f"{stem}{tag}", a, m, hw, ht, mat=mat, up=up)
    if tip_split <= 0.0:
        bar(rig, bone, f"{stem}t{tag}", m, b, hw * 0.8, ht, mat="char_brown", up=up)
        return
    d = (b[0] - m[0], b[1] - m[1], b[2] - m[2])
    mir = -1.0 if tag == "_l" else 1.0       # 左翼是右翼的镜像：绕 Y 的偏角和 x 向错位都要反号
    for k, sign in enumerate((-1.0, 1.0)):
        e = rotate_about_y(d, sign * mir * tip_split)
        # 两叉各取半宽，从近端错开半个叉宽，避免两叉重叠
        off = (hw * 0.5) * sign
        if abs(d[0]) < abs(d[2]):
            start = (m[0] + off * mir, m[1], m[2])
        else:
            start = (m[0], m[1], m[2] + off)
        end = (start[0] + e[0], start[1] + e[1], start[2] + e[2])
        bar(rig, bone, f"{stem}t{'ab'[k]}{tag}", start, end, hw * 0.5, ht, mat="char_brown", up=up)


def bolt_line(rig: Rig, bone: str, stem: str, tag: str, a: Vec, b: Vec, *, lateral: Vec, rise: float, zig: float,
              pieces: int = 5) -> None:
    """沿 a → b 的雷痕折线：每段向 lateral 方向交错偏 zig，抬高 rise 贴在羽面上。"""
    pts = []
    for k in range(pieces + 1):
        p = lerp3(a, b, k / pieces)
        off = zig * (1 if k % 2 else -1) * (0.0 if k in (0, pieces) else 1.0)
        pts.append((p[0] + lateral[0] * off, p[1] + rise + lateral[1] * off, p[2] + lateral[2] * off))
    for k in range(pieces):
        bar(rig, bone, f"{stem}{k}{tag}", pts[k], pts[k + 1], 0.25, 0.25, mat="bolt_blue")


# ===========================================================================
# 1. 头 + 喙 + 冠羽 + 眼
# ===========================================================================
SKULL_Z0, SKULL_Z1 = -18.0, -12.5
SKULL_Y0, SKULL_Y1 = 19.0, 24.4


def part_head(rig: Rig) -> None:
    box(rig, "head", "skull", (-3.2, SKULL_Y0, SKULL_Z0), (3.2, SKULL_Y1, SKULL_Z1), mat="feather_dark")
    box(rig, "head", "crown", (-2.6, SKULL_Y1, SKULL_Z0 + 0.8), (2.6, SKULL_Y1 + 1.0, SKULL_Z1 - 0.6), mat="feather_black")
    for s in (-1.0, 1.0):
        mbox(rig, "head", "cheek", s, 3.2, 3.9, 19.8, 23.2, -17.4, -13.0, mat="feather_black")
        # 眼窝 + 一点白光：眼窝贴在颊外面，白点再贴在眼窝外面
        mbox(rig, "head", "socket", s, 3.9, 4.4, 21.4, 23.2, -16.6, -14.8, mat="horn_dark")
        mbox(rig, "head", "eye", s, 4.4, 4.9, 21.9, 22.7, -15.9, -15.1, mat="eye_white")
        # 眉棱：焦褐一条压在眼上
        mbox(rig, "head", "brow", s, 1.0, 3.2, SKULL_Y1 - 0.1, SKULL_Y1 + 0.5, -17.6, -15.6, mat="char_brown")
        # 颊羽：从眼后往后下方披的两片
        for k, (z, y) in enumerate(((-13.6, 22.2), (-13.2, 20.6))):
            feather(rig, "head", f"cheekf{k}", side_tag(s), (s * 4.0, y, z), (s * 4.7, y - 0.8, z + 3.0), 1.0, 0.3,
                    mat="feather_black" if k == 0 else "feather_dark", up=(s, 0.0, 0.0))
    # 喙：上喙四段向下弯，下喙三段，鼻沟一条暗线
    up_pts = [(0.0, 21.6, -18.0), (0.0, 21.3, -20.6), (0.0, 20.5, -22.8), (0.0, 19.0, -24.2)]
    up_sz = [(1.5, 1.6), (1.3, 1.4), (1.0, 1.1), (0.55, 0.65)]
    for k in range(3):
        a, b = up_pts[k], up_pts[k + 1]
        rx = (up_sz[k][0] + up_sz[k + 1][0]) / 2
        rz = (up_sz[k][1] + up_sz[k + 1][1]) / 2
        bar(rig, "beak_up", f"beak_up{k}", a, b, rx, rz, mat="horn")
    bar(rig, "beak_up", "beak_hook", up_pts[3], (0.0, 18.0, -24.5), 0.4, 0.4, mat="horn_dark")
    box(rig, "beak_up", "nostril_l", (-1.5, 22.0, -19.8), (-1.0, 22.6, -19.0), mat="horn_dark")
    box(rig, "beak_up", "nostril_r", (1.0, 22.0, -19.8), (1.5, 22.6, -19.0), mat="horn_dark")
    low_pts = [(0.0, 19.0, -18.0), (0.0, 18.8, -20.4), (0.0, 18.4, -22.6), (0.0, 18.2, -23.6)]
    low_sz = [(1.0, 0.6), (0.8, 0.5), (0.5, 0.4), (0.25, 0.3)]
    for k in range(3):
        rx = (low_sz[k][0] + low_sz[k + 1][0]) / 2
        rz = (low_sz[k][1] + low_sz[k + 1][1]) / 2
        bar(rig, "beak_low", f"beak_low{k}", low_pts[k], low_pts[k + 1], rx, rz, mat="horn")
    # 冠羽：头顶一撮三片，中间一片更长向后翘，焦褐尖
    for k, (x, ln) in enumerate(((-1.5, 4.5), (0.0, 6.0), (1.5, 4.5))):
        y0 = SKULL_Y1 + 1.0
        a, b = (x, y0, -13.2), (x * 1.6, y0 + 2.4, -13.2 + ln)
        feather(rig, "crest", f"crestf{abs(k - 1) and 0 or 1}", "" if x == 0.0 else side_tag(x), a, b, 0.9, 0.3,
                mat="feather_black", up=(1.0, 0.0, 0.0))


# ===========================================================================
# 2. 颈：两节 + 羽领
# ===========================================================================
def feather_ring(rig: Rig, bone: str, stem: str, y0: float, z0: float, r: float, length: float, hw: float,
                 flare: float, phis=(0.0, 45.0, -45.0, 90.0, -90.0), mats=("feather_black", "feather_dark")) -> None:
    """绕 z 轴一圈羽（颈领）：每片从半径 r 处起，向后 length、向外 flare。"""
    for n, phi in enumerate(phis):
        ph = math.radians(phi)
        for s in ((1.0,) if abs(abs(phi) - 90.0) < 1e-6 else (-1.0, 1.0)):
            nrm = (s * math.cos(ph), math.sin(ph), 0.0)
            a = (nrm[0] * r, y0 + nrm[1] * r, z0)
            b = (a[0] + nrm[0] * flare, a[1] + nrm[1] * flare, z0 + length)
            tag = "" if abs(abs(phi) - 90.0) < 1e-6 else side_tag(s)
            feather(rig, bone, f"{stem}{n}", tag, a, b, hw, 0.3, mat=mats[n % 2], up=nrm)


def part_neck(rig: Rig) -> None:
    box(rig, "neck_0", "neck0_core", (-3.0, 15.6, -10.5), (3.0, 20.4, -8.5), mat="feather_dark")
    box(rig, "neck_1", "neck1_core", (-2.8, 17.2, -12.5), (2.8, 22.4, -10.5), mat="feather_dark")
    feather_ring(rig, "neck_0", "ruff0_", 18.0, -10.4, 2.8, 3.6, 1.3, 0.9)
    feather_ring(rig, "neck_1", "ruff1_", 19.8, -12.4, 2.6, 3.0, 1.2, 0.8, phis=(0.0, 50.0, -50.0, 90.0))


# ===========================================================================
# 3. 躯干：八棱芯 + 七条鳞状羽列
# ===========================================================================
# 躯干半径沿 z 的折线：胸口接颈处收窄，胸腹最鼓，臀往尾收细成一颗蛋
CORE_PROFILE = ((CHEST_Z, 4.2), (-5.5, 5.2), (-1.0, 5.7), (3.0, 5.5), (7.0, 4.6), (RUMP_END_Z, 3.0))
CORE_SLICES = 8
CORE_RING_T = 2.0                  # 外壳环径向厚；里面再填一个八棱芯，不透光
CORE_SIDES = 16


def core_radius(z: float) -> float:
    for (z0, r0), (z1, r1) in zip(CORE_PROFILE, CORE_PROFILE[1:]):
        if z <= z1:
            t = min(1.0, max(0.0, (z - z0) / (z1 - z0)))
            return r0 + (r1 - r0) * (t * t * (3.0 - 2.0 * t))
    return CORE_PROFILE[-1][1]


def core_y(z: float) -> float:
    """躯干中心高度：臀部略低 0.4，和原稿一致。"""
    t = min(1.0, max(0.0, (z - 3.0) / 6.0))
    return BODY_Y - 0.4 * (t * t * (3.0 - 2.0 * t))


def octa_core(rig: Rig, bone: str, stem: str, yc: float, z0: float, z1: float, r: float) -> None:
    """八棱柱芯：十字两盒 + 转 45° 的一盒。"""
    box(rig, bone, f"{stem}_h", (-r, yc - 0.414 * r, z0), (r, yc + 0.414 * r, z1), mat="feather_dark")
    box(rig, bone, f"{stem}_v", (-0.414 * r, yc - r, z0), (0.414 * r, yc + r, z1), mat="feather_dark")
    s45 = 0.72 * r
    rig.cube(bone, f"{stem}_d", (-s45, yc - s45, z0), (s45, yc + s45, z1), rot=(0.0, 0.0, 45.0), org=(0.0, yc, (z0 + z1) / 2),
             mat="feather_dark")


def poly_ring(rig: Rig, bone: str, stem: str, yc: float, z0: float, z1: float, apothem: float, t: float) -> None:
    """CORE_SIDES 边形环：边心角 0°, 22.5°, …（0° = +X）。左右镜像边共用词干（折到 x≥0 一侧），才对得上镜像门。"""
    step = 360.0 / CORE_SIDES
    half = math.radians(step / 2)
    vertex_r = (apothem - t / 2) / math.cos(half)
    zc, zlen = (z0 + z1) / 2, z1 - z0
    for k in range(CORE_SIDES):
        ang = step * k
        pts = [(vertex_r * math.cos(math.radians(ang) + d), yc + vertex_r * math.sin(math.radians(ang) + d), zc) for d in (-half, half)]
        cx = math.cos(math.radians(ang))
        if abs(cx) < 1e-6:
            name = f"{stem}_a{k}"
        else:
            name = f"{stem}_a{k if cx > 0 else (CORE_SIDES // 2 - k) % CORE_SIDES}{'_r' if cx > 0 else '_l'}"
        frm, to, rot, org = shaft_box(pts[0], pts[1], t / 2, zlen / 2, t / 2 * math.tan(half), (0.0, 0.0, 1.0))
        rig.cube(bone, name, frm, to, rot=rot, org=org, mat="feather_dark" if k % 2 else "feather_black")


ROW_PHIS = (0.0, 35.0, 65.0, -35.0, -65.0)        # 两侧羽列的环向角（°，自水平向上为正）
BLADE_LEN = 4.6
BLADE_STRIDE = 3.4


def part_torso(rig: Rig) -> None:
    """躯干 = 沿 z 切 8 片的十六边形环（半径跟着 CORE_PROFILE 走成一颗蛋）+ 每片里一个八棱芯。"""
    dz = (RUMP_END_Z - CHEST_Z) / CORE_SLICES
    for n in range(CORE_SLICES):
        z0, z1 = CHEST_Z + dz * n, CHEST_Z + dz * (n + 1)
        zm = (z0 + z1) / 2
        r, yc = core_radius(zm), core_y(zm)
        bone = "spine_0" if zm < CHEST_Z + 8.0 else "spine_1"
        poly_ring(rig, bone, f"torso{n}", yc, z0, z1, r, CORE_RING_T)
        octa_core(rig, bone, f"core{n}", yc, z0, z1, r - CORE_RING_T)
    rows = [(phi, True) for phi in ROW_PHIS] + [(90.0, False), (-90.0, False)]
    for ri, (phi, sided) in enumerate(rows):
        ph = math.radians(phi)
        for s in ((-1.0, 1.0) if sided else (1.0,)):
            nrm = (s * math.cos(ph) if sided else 0.0, math.sin(ph), 0.0)
            tag = side_tag(s) if sided else ""
            z = CHEST_Z + (1.7 if ri % 2 else 0.0)
            k = 0
            while z + BLADE_LEN <= RUMP_END_Z + 0.2:
                r = core_radius(z)
                yc = core_y(z)
                a = (nrm[0] * (r + 0.35), yc + nrm[1] * (r + 0.35), z)
                b = (a[0] + nrm[0] * 0.8, a[1] + nrm[1] * 0.8, z + BLADE_LEN)
                bone = "spine_0" if z < CHEST_Z + 8.0 else "spine_1"
                feather(rig, bone, f"body{ri}_{k}", tag, a, b, 1.55, 0.35, mat="feather_black" if (k + ri) % 2 else "feather_dark",
                        up=nrm, tip_len=0.25)
                z += BLADE_STRIDE
                k += 1


# ===========================================================================
# 4. 双翼：臂骨 + 覆羽 + 次级羽 + 小翼羽 + 四束初级羽
# ===========================================================================
ARM_HW, ARM_HT = 1.9, 1.2


def part_wings(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        t = side_tag(s)
        S, E, W, T = (sx(p, s) for p in (WING_S, WING_E, WING_W, WING_T))
        up = (0.0, 1.0, 0.0)
        # 臂骨：每段在关节处缩进，关节块才不咬进上一段
        bar(rig, f"wing_arm{t}", f"arm{t}", S, along(E, S, 1.6), ARM_HW, ARM_HT, mat="feather_black", up=up)
        box(rig, f"wing_fore{t}", f"elbow{t}", (E[0] - 1.8, E[1] - 1.5, E[2] - 1.8), (E[0] + 1.8, E[1] + 1.5, E[2] + 1.8), mat="char_brown")
        bar(rig, f"wing_fore{t}", f"fore{t}", along(E, W, 1.8), along(W, E, 1.6), ARM_HW - 0.2, ARM_HT - 0.1, mat="feather_black", up=up)
        box(rig, f"wing_hand{t}", f"wrist{t}", (W[0] - 1.6, W[1] - 1.3, W[2] - 1.6), (W[0] + 1.6, W[1] + 1.3, W[2] + 1.6), mat="char_brown")
        bar(rig, f"wing_hand{t}", f"hand{t}", along(W, T, 1.6), T, ARM_HW - 0.7, ARM_HT - 0.3, mat="feather_black", up=up)
        # 臂上覆羽：沿臂前缘一排短羽向后倒，压在次级羽根上
        for k in range(5):
            tt = 0.1 + 0.2 * k
            for seg, (p0, p1, bone) in enumerate(((S, E, f"wing_arm{t}"), (E, W, f"wing_fore{t}"))):
                a = lerp3(p0, p1, tt)
                a = (a[0], a[1] + ARM_HT + 0.35, a[2] - 0.2)
                b = (a[0] + s * 0.8, a[1] - 1.1, a[2] + 5.4)
                feather(rig, bone, f"covert{seg}_{k}", t, a, b, 1.25, 0.35, mat="feather_dark" if k % 2 else "feather_black",
                        up=(0.0, 1.0, 0.0), tip_len=0.3)
        for k in range(3):
            a = lerp3(W, T, 0.15 + 0.3 * k)
            a = (a[0], a[1] + ARM_HT - 0.3 + 0.35, a[2])
            b = (a[0] + s * 1.2, a[1] - 0.6, a[2] + 5.0)
            feather(rig, f"wing_hand{t}", f"covert_h{k}", t, a, b, 1.1, 0.35, mat="feather_dark", up=(0.0, 1.0, 0.0), tip_len=0.3)
        # 内侧次级羽（挂臂骨）：5 片，略向内偏
        for j in range(5):
            tt = 0.1 + 0.8 * j / 4
            a = lerp3(S, E, tt)
            a = (a[0], a[1] - 0.4, a[2] + 1.0)
            ln = 13.0 + 0.6 * j
            ang = math.radians(-6.0 + 2.0 * j)
            b = (a[0] + s * ln * math.sin(ang), a[1] - 0.6, a[2] + ln * math.cos(ang))
            feather(rig, f"sec_in{t}", f"sec_in{j}", t, a, b, 1.3, 0.35, mat="feather_black" if j % 2 == 0 else "feather_dark",
                    up=up, tip_len=0.24)
        # 外侧次级羽（挂前臂骨）：6 片
        for j in range(6):
            tt = 0.1 + 0.8 * j / 5
            a = lerp3(E, W, tt)
            a = (a[0], a[1] - 0.4, a[2] + 1.0)
            ln = 14.5 + 0.5 * j
            ang = math.radians(8.0 + 1.2 * j)
            b = (a[0] + s * ln * math.sin(ang), a[1] - 0.6, a[2] + ln * math.cos(ang))
            feather(rig, f"sec_out{t}", f"sec_out{j}", t, a, b, 1.3, 0.35, mat="feather_black" if j % 2 == 0 else "feather_dark",
                    up=up, tip_len=0.24)
        # 小翼羽：腕前缘两片小羽，向前外
        for k in range(2):
            a = (W[0] + s * (0.5 + k), W[1] + 0.2, W[2] - 1.4)
            b = (a[0] + s * (2.0 + 1.2 * k), a[1] + 0.2, a[2] - 3.4 - k)
            feather(rig, f"alula{t}", f"alula{k}", t, a, b, 0.8, 0.3, mat="feather_black", up=up, tip_len=0.3)
        # 四束初级羽：每束三片扇开，越往外越炸，尖端劈叉
        for g in range(4):
            base = lerp3(W, T, PRIMARY_T[g])
            base = (base[0], base[1] - 0.2, base[2] + 0.4)
            spread = 7.0 + 2.0 * g
            for i in (-1, 0, 1):
                ang = math.radians(PRIMARY_ANGLE[g] + spread * i)
                ln = PRIMARY_LEN[g] - 1.5 * abs(i)
                d = (s * math.cos(ang), 0.06, math.sin(ang))
                b = (base[0] + d[0] * ln, base[1] + d[1] * ln, base[2] + d[2] * ln)
                feather(rig, f"prim_{g}{t}", f"prim{g}_{i + 1}", t, base, b, 1.25 - (0.12 if g == 3 else 0.0), 0.35,
                        mat="feather_black" if (g + i) % 2 else "feather_dark", up=up, tip_len=0.26, tip_split=8.0 + 2.0 * g)


# ===========================================================================
# 5. 尾：两节 + 七片尾羽扇 + 上下覆羽
# ===========================================================================
TAIL_FAN_ANGLES = (0.0, 11.0, 22.0, 33.0)
TAIL_LEN = 22.0


def part_tail(rig: Rig) -> None:
    box(rig, "tail_0", "tail0_core", (-2.8, 12.6, TAIL_Z0), (2.8, 16.0, TAIL_Z0 + 4.5), mat="feather_dark")
    box(rig, "tail_1", "tail1_core", (-3.2, 12.8, TAIL_Z0 + 4.5), (3.2, 15.2, TAIL_Z0 + 7.0), mat="feather_dark")
    up = (0.0, 1.0, 0.0)
    z0 = TAIL_Z0 + 5.0
    for k, deg in enumerate(TAIL_FAN_ANGLES):
        for s in ((1.0,) if k == 0 else (-1.0, 1.0)):
            ang = math.radians(deg)
            ln = TAIL_LEN - 1.2 * k
            a = (s * 0.9 * k, 14.0, z0)
            b = (a[0] + s * math.sin(ang) * ln, a[1] - 1.5, a[2] + math.cos(ang) * ln)
            bone = "tail_c" if k == 0 else ("tail_l" if s < 0 else "tail_r")
            tag = "" if k == 0 else side_tag(s)
            feather(rig, bone, f"tailf{k}", tag, a, b, 1.5, 0.35, mat="feather_black" if k % 2 == 0 else "feather_dark",
                    up=up, tip_len=0.24, tip_split=7.0 if k else 0.0)   # 中片尖端不劈叉：劈叉的两叉不自对称
    # 上 / 下尾覆羽：根部几片短羽
    for k in range(3):
        for s in ((1.0,) if k == 0 else (-1.0, 1.0)):
            tag = "" if k == 0 else side_tag(s)
            x = s * 1.1 * k
            for layer, (dy, mat) in enumerate(((1.0, "feather_dark"), (-1.0, "feather_black"))):
                a = (x, 14.0 + dy * 0.9, z0 - 1.0)
                b = (x + s * 0.5 * k, 14.0 + dy * 0.9 - dy * 0.0 - 0.8, z0 + 8.0 - k)
                bone = "tail_c" if k == 0 else ("tail_l" if s < 0 else "tail_r")
                feather(rig, bone, f"tailcov{layer}_{k}", tag, a, b, 1.3, 0.3, mat=mat, up=up, tip_len=0.3)


# ===========================================================================
# 6. 腿：羽裤 + 跗 + 三前趾一后趾 + 钩爪
# ===========================================================================
def part_legs(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        t = side_tag(s)
        H, K, A, F = (sx(p, s) for p in (HIP, KNEE, ANKLE, FOOT))
        bar(rig, f"leg_thigh{t}", f"thigh{t}", H, along(K, H, 0.8), 1.7, 1.7, mat="feather_dark")
        box(rig, f"leg_shin{t}", f"knee{t}", (K[0] - 1.4, K[1] - 1.4, K[2] - 1.4), (K[0] + 1.4, K[1] + 1.4, K[2] + 1.4), mat="feather_dark")
        bar(rig, f"leg_shin{t}", f"shin{t}", along(K, A, 1.2), along(A, K, 1.3), 1.15, 1.15, mat="feather_dark")
        # 羽裤：从大腿外侧往下披三片，小腿外侧两片
        for k in range(3):
            a = lerp3(H, K, 0.15 + 0.3 * k)
            a = (a[0] + s * 1.7, a[1], a[2])
            feather(rig, f"leg_thigh{t}", f"pants{k}", t, a, (a[0] + s * 0.6, a[1] - 3.2, a[2] + 0.8), 1.1, 0.3,
                    mat="feather_black" if k % 2 == 0 else "feather_dark", up=(s, 0.0, 0.0), tip_len=0.3)
        for k in range(2):
            a = lerp3(K, A, 0.2 + 0.3 * k)
            a = (a[0] + s * 1.15, a[1], a[2])
            feather(rig, f"leg_shin{t}", f"shinf{k}", t, a, (a[0] + s * 0.5, a[1] - 2.6, a[2] + 1.0), 0.9, 0.3,
                    mat="feather_black", up=(s, 0.0, 0.0), tip_len=0.3)
        box(rig, f"leg_tarsus{t}", f"ankle{t}", (A[0] - 1.0, A[1] - 1.0, A[2] - 1.0), (A[0] + 1.0, A[1] + 1.0, A[2] + 1.0), mat="horn_dark")
        bar(rig, f"leg_tarsus{t}", f"tarsus{t}", along(A, F, 1.0), F, 0.8, 0.8, mat="horn")
        for k, tt in enumerate((0.3, 0.55, 0.8)):
            c = lerp3(A, F, tt)
            box(rig, f"leg_tarsus{t}", f"scute{k}{t}", (c[0] - 0.95, c[1] - 0.45, c[2] - 0.95), (c[0] + 0.95, c[1] + 0.45, c[2] + 0.95), mat="horn_dark")
        # 前趾：三根，每趾两节 + 钩爪；后趾一根
        for k, beta in enumerate(TOE_OUT_ANGLES):
            b = math.radians(beta)
            d = (s * math.sin(b), 0.0, -math.cos(b))
            p0 = F
            p1 = (p0[0] + d[0] * 3.6, 1.1, p0[2] + d[2] * 3.6)
            p2 = (p1[0] + d[0] * 2.6, 0.9, p1[2] + d[2] * 2.6)
            p3 = (p2[0] + d[0] * 1.7, 0.45, p2[2] + d[2] * 1.7)
            bar(rig, f"toes_front{t}", f"toe{k}a{t}", p0, p1, 0.6, 0.6, mat="horn")
            bar(rig, f"toes_front{t}", f"toe{k}b{t}", p1, p2, 0.55, 0.55, mat="horn")
            bar(rig, f"toes_front{t}", f"toe{k}c{t}", p2, p3, 0.4, 0.4, mat="horn_dark")
        q1 = (F[0], 1.1, F[2] + 3.2)
        q2 = (F[0], 0.45, F[2] + 4.6)
        bar(rig, f"toe_back{t}", f"hallux_a{t}", F, q1, 0.6, 0.6, mat="horn")
        bar(rig, f"toe_back{t}", f"hallux_b{t}", q1, q2, 0.4, 0.4, mat="horn_dark")


# ===========================================================================
# 7. 雷痕：胸侧、翅面、尾羽上的蓝白折线
# ===========================================================================
def part_bolts(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        t = side_tag(s)
        up = (0.0, 1.0, 0.0)
        # 翅：初级羽两束中片、外侧次级羽一片，沿羽中线折线，贴在羽面上
        for g, bone in ((1, f"prim_1{t}"), (2, f"prim_2{t}")):
            base = lerp3(WING_W, WING_T, PRIMARY_T[g])
            base = (s * base[0], base[1] - 0.2, base[2] + 0.4)
            ang = math.radians(PRIMARY_ANGLE[g])
            d = (s * math.cos(ang), 0.16, math.sin(ang))
            ln = PRIMARY_LEN[g]
            a = (base[0] + d[0] * ln * 0.2, base[1] + d[1] * ln * 0.2, base[2] + d[2] * ln * 0.2)
            b = (base[0] + d[0] * ln * 0.78, base[1] + d[1] * ln * 0.78, base[2] + d[2] * ln * 0.78)
            bolt_line(rig, bone, f"bolt_p{g}_", t, a, b, lateral=(d[2] * s, 0.0, -d[0] * s), rise=0.7, zig=0.7)
        E, W = sx(WING_E, s), sx(WING_W, s)
        a = lerp3(E, W, 0.5)
        a = (a[0], a[1] - 0.4, a[2] + 3.0)
        b = (a[0] + s * 2.4, a[1], a[2] + 11.0)
        bolt_line(rig, f"sec_out{t}", "bolt_s_", t, a, b, lateral=(s, 0.0, 0.0), rise=0.65, zig=0.7)
        # 胸侧：沿躯干侧面贴着八棱芯外那圈羽（半径 core_radius + 1.5）折线下行
        c0 = (s * (core_radius(CHEST_Z + 1.2) + 1.5), BODY_Y + 2.4, CHEST_Z + 1.2)
        c1 = (s * (core_radius(CHEST_Z + 7.2) + 1.5), BODY_Y - 1.6, CHEST_Z + 7.2)
        bolt_line(rig, "spine_0", "bolt_c_", t, c0, c1, lateral=(0.0, 0.0, 1.0), rise=0.0, zig=0.8)
    # 尾羽中片上一条
    a, b = (0.0, 14.0 + 0.35 + 0.25, TAIL_Z0 + 8.0), (0.0, 14.0 - 1.5 + 0.35 + 0.25 - 0.3, TAIL_Z0 + 22.0)
    bolt_line(rig, "tail_c", "bolt_t_", "", a, b, lateral=(1.0, 0.0, 0.0), rise=0.0, zig=0.0, pieces=4)


PARTS = {
    "head": ("1. 头 + 喙 + 冠羽 + 白点眼", part_head),
    "neck": ("2. 颈两节 + 羽领", part_neck),
    "torso": ("3. 躯干八棱芯 + 七条鳞状羽列", part_torso),
    "wings": ("4. 双翼：臂骨 / 覆羽 / 次级羽 / 小翼羽 / 初级羽", part_wings),
    "tail": ("5. 尾两节 + 七片尾羽扇", part_tail),
    "legs": ("6. 羽裤 + 跗 + 钩爪", part_legs),
    "bolts": ("7. 雷痕折线", part_bolts),
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
# 穿模门按 AABB 判：斜置羽片的 AABB 很大，同一翼里的羽片 / 雷痕 / 覆羽会互相虚报互穿。
# 羽毛各色之间、雷痕 × 羽毛放行；喙 / 爪 / 眼等其他材质仍照常查。
SOFT_OVER = frozenset(
    {frozenset((a, b)) for a in FEATHER_MATS for b in FEATHER_MATS if a != b}
    | {frozenset(("bolt_blue", m)) for m in FEATHER_MATS}
)


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制（劫雷鸦展翼 120px），去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates("劫雷鸦 / jie_lei_ya", tuple(MATS), soft_over=SOFT_OVER)


def self_test() -> None:
    print("运行 gen_jie_lei_ya.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    moving = moving_bones(rig)
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert abs(lo[1]) < 0.15, f"贴地失败：最低点 y={lo[1]:.3f}，爪尖应落在 0 附近"
    span = hi[0] - lo[0]
    assert 80.0 <= span <= 130.0, f"展翼 {span:.1f}px 超出 80..130"
    print(f"  [OK] 可动骨 {len(moving)} cube {len(rig.elements)} 展翼 {span:.1f}px 身长 {hi[2] - lo[2]:.1f}px 高 {hi[1]:.1f}px")
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


# yaw 方向：yaw=0 相机在 +Z（照到尾）；yaw=180 相机在 -Z（照到头）；
# yaw=90 相机在 -X（照到左侧，头朝画面左）；yaw=-90 相机在 +X（照到右侧，头朝画面右）。
def six_views(rig: Rig):
    whole = focus_of(rig)
    body = ((0.0, 17.0, -2.0), 52.0)
    side = ((0.0, 15.0, 6.0), 80.0)
    return [
        ("头吻面 FRONT：相机在 -Z，对准头与躯干（翼尖出框）", 180.0, 0.0, body),
        ("尾臀面 REAR：相机在 +Z，对准尾与躯干（翼尖出框）", 0.0, 0.0, body),
        ("左前上 3/4：相机在 -X -Z +Y，全身", 135.0, 25.0, whole),
        ("左侧面 LEFT：相机在 -X，头朝左", 90.0, 0.0, side),
        ("右侧面 RIGHT：相机在 +X，头朝右", -90.0, 0.0, side),
        ("俯视 TOP：相机在 +Y，头朝上，全身", 0.0, 89.9, whole),
    ]


FOCUS = {
    "head": ((0.0, 21.5, -17.0), 22.0),
    "neck": ((0.0, 18.5, -10.5), 20.0),
    "torso": ((0.0, BODY_Y, 2.0), 32.0),
    "wings": ((26.0, 19.0, -2.0), 62.0),
    "tail": ((0.0, 14.0, TAIL_Z0 + 14.0), 42.0),
    "legs": ((3.6, 5.0, 2.0), 20.0),
    "bolts": ((20.0, 17.0, 0.0), 64.0),
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
            ("头吻面 FRONT：相机在 -Z", 180.0, 0.0),
            ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0),
            ("俯视 TOP：相机在 +Y" if key in ("wings", "bolts", "tail") else "左侧面 LEFT：相机在 -X",
             0.0 if key in ("wings", "bolts", "tail") else 90.0, 89.9 if key in ("wings", "bolts", "tail") else 0.0),
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
    """2 行 × 3 列方格：上排 前 / 后 / 3/4，下排 左 / 右 / 俯视。"""
    font_h, font_l = load_font(18), load_font(13)
    sheet = Image.new("RGB", (cell * 3 + 20, 34 + (cell + 5) * 2 + 5), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), header, fill=(235, 235, 235), font=font_h)
    for n, (label, yaw, pitch, focus) in enumerate(six_views(rig)):
        px, py = 5 + (n % 3) * (cell + 5), 34 + (n // 3) * (cell + 5)
        im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)
        sheet.paste(im, (px, py))
        d.rectangle([px, py, px + 430, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def bone_tree_lines(rig: Rig) -> list[str]:
    """骨骼树：右翼、右腿完整展开，左侧同构的只写一行。"""
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines: list[str] = []

    def count_desc(name: str) -> int:
        return sum(1 + count_desc(k) for k in children.get(name, []))

    def walk(name: str, depth: int) -> None:
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{name} ({len(rig.bones[name]['children'])})")
        if name.endswith("_l") and name.startswith(("wing_arm", "leg_thigh")):
            lines.append(f"{'  ' * (depth + 1)}（另 {count_desc(name)} 根子骨，与右侧镜像同构）")
            return
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
    ap = argparse.ArgumentParser(description="劫雷鸦 (jie_lei_ya) 生成器 Round 1/3")
    ap.add_argument("--self-test", action="store_true", help="门禁差分自证")
    ap.add_argument("--list", action="store_true", help="列出编号部件")
    ap.add_argument("--no-render", action="store_true", help="只写 bbmodel，不出预览图")
    args = ap.parse_args()

    if args.list:
        for key, (title, _) in PARTS.items():
            print(f"  {key:9s} {title}")
        return
    if args.self_test:
        self_test()
        return

    self_test()
    rig = build_full()
    doc = rig.bbmodel("JieLeiYa", namespace="bong")
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
