#!/usr/bin/env python3
"""灰鳞穴蜥（hui_lin_xue_xi）四足生物模型程序化生成器 —— Round 1/3（只做几何）。

依据：docs/worldview.md §十三 幽暗地穴。任务卡第五批 #19：没有眼睛的地穴大蜥蜴，灰白鳞片、
头骨上一排感振的骨瘤，舌头分叉很长，爪子能抓岩壁。

蜥蜴的伏地姿态：上臂 / 大腿向身体两侧平伸，肘膝外翻，前臂 / 小腿向下踩地，五趾张开成扇。
  1. 头      长楔形头 + 眼位上一块光滑的苍白瘢痕（它没有眼）+ 瘢痕周围一圈骨瘤 + 颌线骨瘤 +
             张口 24° 的下颌 + 两排锥齿 + 鼻孔裂
  2. 舌      从口里伸出很远的细长舌，末端分叉
  3. 颈      两节粗颈，颈下垂着苍白的喉囊
  4. 躯干    13 片鳞桶（背 / 腰 / 腹三层）+ 两侧每片两枚交错的鳞板 + 背脊骨刺
  5. 四肢    上臂 / 前臂 / 掌 / 五趾（内三趾一组、外二趾一组）+ 每趾两节趾骨一枚钩爪；
             后肢大腿 / 小腿 / 跗 / 五趾；肘膝外侧各两枚骨刺
  6. 尾      6 节渐细的尾，每节一圈暗色环 + 背刺，尾腹苍白

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X。

用法：
  python3 modelScript/creatures/hui_lin_xue_xi/gen_hui_lin_xue_xi.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/hui_lin_xue_xi/gen_hui_lin_xue_xi.py --self-test # 门禁差分自证
  python3 modelScript/creatures/hui_lin_xue_xi/gen_hui_lin_xue_xi.py --list
  python3 modelScript/creatures/hui_lin_xue_xi/gen_hui_lin_xue_xi.py --part head
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "hui_lin_xue_xi"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/hui_lin_xue_xi")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "HuiLinXueXi.bbmodel"
BBMODEL_COPY = MODELS_DIR / "HuiLinXueXi.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质必须 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "scale_grey": (138, 140, 138),   # 灰鳞主色
    "scale_pale": (196, 196, 186),   # 浅色鳞片
    "scale_dark": (88, 90, 94),      # 暗色鳞片、环带
    "belly": (214, 208, 192),        # 腹、喉囊
    "nodule": (232, 224, 200),       # 骨瘤、齿、背刺
    "claw_dark": (52, 46, 48),       # 爪、鼻孔
    "mouth": (166, 82, 90),          # 口腔、舌
    "scar": (176, 140, 136),         # 眼位的光滑瘢痕
}

# ---------------------------------------------------------------------------
# 骨骼：(名称, pivot(左侧 x 为负), 父骨)。右侧由 define_bones 生成。
# ---------------------------------------------------------------------------
CENTER_BONES = [
    ("root", (0.0, 0.0, 0.0), None),
    ("hips", (0.0, 6.0, 5.2), "root"),
    ("spine_lumbar", (0.0, 6.2, 1.5), "hips"),
    ("thorax", (0.0, 6.4, -3.0), "spine_lumbar"),
    ("neck_base", (0.0, 6.9, -6.0), "thorax"),
    ("neck_upper", (0.0, 6.8, -8.2), "neck_base"),
    ("head", (0.0, 6.3, -10.2), "neck_upper"),
    ("jaw", (0.0, 4.8, -11.0), "head"),
    ("tongue_1", (0.0, 4.3, -12.4), "jaw"),
    ("tongue_2", (0.0, 3.9, -21.0), "tongue_1"),
    ("tail_1", (0.0, 6.0, 7.0), "hips"),
    ("tail_2", (0.0, 5.7, 10.0), "tail_1"),
    ("tail_3", (0.0, 5.2, 13.0), "tail_2"),
    ("tail_4", (0.0, 4.5, 16.0), "tail_3"),
    ("tail_5", (0.0, 3.6, 19.0), "tail_4"),
    ("tail_6", (0.0, 2.7, 22.0), "tail_5"),
]

SIDE_BONES = [
    ("nodule", (2.3, 7.5, -12.0), "head"),
    ("tongue_fork", (0.3, 3.0, -26.0), "tongue_2"),
    ("arm", (3.6, 6.2, -4.6), "thorax"),
    ("forearm", (7.4, 4.8, -5.8), "arm"),
    ("hand", (6.6, 1.2, -8.2), "forearm"),
    ("toe_f_in", (5.4, 1.0, -9.0), "hand"),
    ("toe_f_out", (8.2, 1.0, -9.0), "hand"),
    ("thigh", (3.6, 6.0, 5.2), "hips"),
    ("shin", (7.6, 4.4, 3.6), "thigh"),
    ("foot", (6.8, 1.1, 7.4), "shin"),
    ("toe_b_in", (5.6, 1.0, 5.6), "foot"),
    ("toe_b_out", (8.0, 1.0, 5.6), "foot"),
]

MIN_MOVING_BONES = 32


def define_bones(rig: Rig) -> None:
    for name, pivot, parent in CENTER_BONES:
        rig.bone(name, pivot, parent)
    side_names = {b for b, _, _ in SIDE_BONES}
    for suffix, sx in (("_l", -1.0), ("_r", 1.0)):
        for base, (x, y, z), parent in SIDE_BONES:
            par = parent + suffix if parent in side_names else parent
            rig.bone(base + suffix, (x * sx, y, z), par)


def build_rig() -> Rig:
    rig = Rig(MATS)
    define_bones(rig)
    return rig


# ---------------------------------------------------------------------------
# 几何小工具
# ---------------------------------------------------------------------------
Vec = tuple[float, float, float]


def bar(rig: Rig, bone: str, name: str, a: Vec, b: Vec, rx: float, rz: float, *,
        mat: str, up: Vec | None = None, extend: float = 0.0) -> None:
    """端点精确落在 a、b 的定向柱。up 给定时局部 +Z（半厚 rz）指向 up；局部 x（半宽 rx）= 轴 × up。"""
    frm, to, rot, org = shaft_box(a, b, rx, rz, extend, up)
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


def box(rig: Rig, bone: str, name: str, lo: Vec, hi: Vec, *, mat: str) -> None:
    rig.cube(bone, name, lo, hi, mat=mat)


def mx(x: float, s: float) -> float:
    return x * s


def side_name(base: str, s: float) -> str:
    return base + ("_l" if s < 0 else "_r")


def lerp3(a: Vec, b: Vec, t: float) -> Vec:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def mirror_pt(p: Vec, s: float) -> Vec:
    return (p[0] * s, p[1], p[2])


def spike(rig: Rig, bone: str, name: str, base: Vec, tip: Vec, width: float, mat: str = "nodule") -> None:
    """两段骨刺：粗根 + 细尖。"""
    mid = lerp3(base, tip, 0.5)
    up_ref = (0.0, 1.0, 0.0) if abs(tip[1] - base[1]) < 0.7 * math.dist(base, tip) else (1.0, 0.0, 0.0)
    bar(rig, bone, f"{name}_a", base, mid, width, width, mat=mat, up=up_ref)
    bar(rig, bone, f"{name}_b", mid, tip, max(width * 0.5, 0.12), max(width * 0.5, 0.12), mat=mat, up=up_ref)


# ===========================================================================
# 1. 头
# ===========================================================================
JAW_HINGE = (0.0, 4.8, -11.0)
JAW_OPEN_DEG = 24.0


def jaw_pt(x: float, y: float, z: float) -> Vec:
    """闭口姿态下的下颌点绕铰链（x 轴）转开 JAW_OPEN_DEG。"""
    a = math.radians(JAW_OPEN_DEG)
    dy, dz = y - JAW_HINGE[1], z - JAW_HINGE[2]
    return (x, JAW_HINGE[1] + dy * math.cos(a) + dz * math.sin(a),
            JAW_HINGE[2] - dy * math.sin(a) + dz * math.cos(a))


def jaw_dir(v: Vec) -> Vec:
    a = math.radians(JAW_OPEN_DEG)
    return (v[0], v[1] * math.cos(a) + v[2] * math.sin(a), -v[1] * math.sin(a) + v[2] * math.cos(a))


def part_head(rig: Rig) -> None:
    """长楔形头：颅 + 瘢痕 + 骨瘤环 + 三段吻 + 两排锥齿 + 张口下颌。"""
    # ── 颅：厚实的方颅，顶面中间一块光滑瘢痕（眼的位置），两侧是鳞 ──
    box(rig, "head", "cranium", (-2.3, 4.6, -14.0), (2.3, 7.3, -10.4), mat="scale_grey")
    box(rig, "head", "occiput", (-2.0, 4.8, -10.4), (2.0, 7.0, -9.4), mat="scale_dark")
    for s in (-1.0, 1.0):
        box(rig, "head", side_name("scar_patch", s), (mx(0.25, s), 7.3, -13.6), (mx(2.05, s), 7.62, -11.2), mat="scar")
        box(rig, "head", side_name("scar_dimple", s), (mx(0.8, s), 7.62, -12.8), (mx(1.5, s), 7.85, -12.0), mat="scale_dark")
        box(rig, "head", side_name("cheek_plate", s), (mx(2.3, s), 4.8, -13.6), (mx(2.65, s), 6.8, -10.6), mat="scale_pale")
        box(rig, "head", side_name("temple_scale", s), (mx(2.3, s), 6.8, -13.0), (mx(2.65, s), 7.3, -10.8), mat="scale_dark")
    # 瘢痕周围一圈骨瘤：两侧边各 4 粒，前缘 3 粒，后缘 3 粒；左右成对，中线上的各一
    for s in (-1.0, 1.0):
        bone = side_name("nodule", s)
        for k, z in enumerate((-13.3, -12.4, -11.5, -10.7)):
            box(rig, bone, side_name(f"eye_nod_edge{k}", s), (mx(2.0, s), 7.3, z - 0.45), (mx(2.8, s), 8.1, z + 0.45), mat="nodule")
        for k, (xo, zo) in enumerate(((0.6, -14.0), (1.7, -13.9))):
            box(rig, bone, side_name(f"eye_nod_front{k}", s), (mx(xo - 0.45, s), 7.3, zo - 0.45), (mx(xo + 0.45, s), 8.0, zo + 0.45), mat="nodule")
        for k, (xo, zo) in enumerate(((0.6, -10.95), (1.6, -11.0))):
            box(rig, bone, side_name(f"eye_nod_back{k}", s), (mx(xo - 0.45, s), 7.3, zo - 0.45), (mx(xo + 0.45, s), 7.95, zo + 0.45), mat="nodule")
    box(rig, "head", "eye_nod_mid_front", (-0.45, 7.3, -14.35), (0.45, 8.1, -13.5), mat="nodule")
    box(rig, "head", "eye_nod_mid_back", (-0.45, 7.3, -11.5), (0.45, 8.1, -10.65), mat="nodule")

    # ── 吻：三段，渐窄渐低；顶面一列骨瘤，鼻孔裂在最前 ──
    pts = [(0.0, 6.0, -14.0), (0.0, 5.7, -16.2), (0.0, 5.45, -18.0), (0.0, 5.3, -19.6)]
    hw = [2.15, 1.85, 1.55, 1.25]
    hh = [1.4, 1.2, 1.05, 0.9]
    for i in range(3):
        bar(rig, "head", f"snout_{i}", pts[i], pts[i + 1], (hw[i] + hw[i + 1]) / 2, (hh[i] + hh[i + 1]) / 2,
            mat="scale_grey" if i != 1 else "scale_dark", up=(0, 1, 0))
        yy = (pts[i][1] + pts[i + 1][1]) / 2 + (hh[i] + hh[i + 1]) / 2
        zz = (pts[i][2] + pts[i + 1][2]) / 2
        box(rig, "head", f"snout_nod{i}", (-0.5, yy, zz - 0.5), (0.5, yy + 0.7, zz + 0.5), mat="nodule")
    for s in (-1.0, 1.0):
        box(rig, "head", side_name("nostril", s), (mx(0.35, s), 6.15, -19.4), (mx(0.85, s), 6.45, -18.7), mat="claw_dark")
        # 颌线骨瘤：沿上颌侧缘三粒
        for k, z in enumerate((-14.8, -16.0, -17.2)):
            box(rig, side_name("nodule", s), side_name(f"jaw_nod{k}", s), (mx(1.95 - 0.2 * k, s), 5.25 - 0.1 * k, z - 0.45),
                (mx(2.75 - 0.2 * k, s), 6.0 - 0.1 * k, z + 0.45), mat="nodule")
        # 上颌牙龈 + 一排锥齿（每颗两段）
        bar(rig, "head", side_name("gum_upper", s), (mx(1.9, s), 4.65, -14.0), (mx(1.1, s), 4.5, -19.4), 0.28, 0.32, mat="mouth", up=(0, 1, 0))
        for k in range(7):
            z = -14.4 - 0.78 * k
            xo = 1.85 - 0.1 * k
            bar(rig, "head", side_name(f"tooth_up{k}", s), (mx(xo, s), 4.65, z), (mx(xo, s), 3.95, z - 0.05), 0.25, 0.28, mat="nodule", up=(1, 0, 0))
            bar(rig, "head", side_name(f"tooth_up_tip{k}", s), (mx(xo, s), 3.95, z - 0.05), (mx(xo, s), 3.4, z - 0.1), 0.16, 0.18, mat="nodule", up=(1, 0, 0))
    box(rig, "head", "palate", (-1.2, 4.5, -19.0), (1.2, 4.75, -13.6), mat="mouth")

    # ── 下颌：绕铰链张开 24°：下颌骨两支 + 一排锥齿 + 口底 ──
    box(rig, "jaw", "jaw_hinge_block", (-2.0, 4.3, -11.6), (2.0, 5.3, -10.4), mat="scale_dark")
    for s in (-1.0, 1.0):
        bar(rig, "jaw", side_name("mandible_back", s), jaw_pt(mx(1.9, s), 4.5, -11.4), jaw_pt(mx(1.6, s), 4.1, -14.4), 0.45, 0.65,
            mat="scale_pale", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("mandible_front", s), jaw_pt(mx(1.6, s), 4.1, -14.4), jaw_pt(mx(1.1, s), 3.9, -19.0), 0.4, 0.55,
            mat="scale_pale", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("jaw_gum", s), jaw_pt(mx(1.5, s), 4.75, -12.6), jaw_pt(mx(1.0, s), 4.55, -18.8), 0.24, 0.3, mat="mouth",
            up=jaw_dir((0, 1, 0)))
        for k in range(6):
            z = -13.0 - 0.95 * k
            xo = 1.55 - 0.08 * k
            bar(rig, "jaw", side_name(f"tooth_dn{k}", s), jaw_pt(mx(xo, s), 4.7, z), jaw_pt(mx(xo, s), 5.4, z - 0.05), 0.25, 0.28, mat="nodule",
                up=jaw_dir((1, 0, 0)))
            bar(rig, "jaw", side_name(f"tooth_dn_tip{k}", s), jaw_pt(mx(xo, s), 5.4, z - 0.05), jaw_pt(mx(xo, s), 5.95, z - 0.1), 0.16, 0.18,
                mat="nodule", up=jaw_dir((1, 0, 0)))
        # 下颌骨瘤两粒
        for k, z in enumerate((-14.2, -16.6)):
            p = jaw_pt(mx(2.75 - 0.3 * k, s), 3.9, z)
            box(rig, side_name("nodule", s), side_name(f"jaw_nod_dn{k}", s), (p[0] - 0.4, p[1] - 0.4, p[2] - 0.4), (p[0] + 0.4, p[1] + 0.4, p[2] + 0.4), mat="nodule")
    bar(rig, "jaw", "mouth_floor", jaw_pt(0.0, 4.5, -12.2), jaw_pt(0.0, 4.4, -18.2), 0.95, 0.18, mat="mouth", up=jaw_dir((0, 1, 0)))
    bar(rig, "jaw", "chin_plate", jaw_pt(0.0, 3.6, -19.2), jaw_pt(0.0, 3.8, -17.0), 0.9, 0.3, mat="belly", up=jaw_dir((0, 1, 0)))


# ===========================================================================
# 2. 舌
# ===========================================================================
def part_tongue(rig: Rig) -> None:
    """从口里伸出很远的细长舌：根在口里（随下颌），出口后一节，再一节，末端分叉成两股。"""
    bar(rig, "tongue_1", "tongue_root", jaw_pt(0.0, 4.35, -12.4), jaw_pt(0.0, 4.2, -16.4), 0.62, 0.28, mat="mouth", up=jaw_dir((0, 1, 0)))
    bar(rig, "tongue_1", "tongue_out", jaw_pt(0.0, 4.2, -16.4), (0.0, 3.9, -21.0), 0.46, 0.24, mat="mouth", up=(1, 0, 0))
    bar(rig, "tongue_2", "tongue_mid", (0.0, 3.9, -21.0), (0.0, 3.0, -26.0), 0.36, 0.2, mat="mouth", up=(1, 0, 0))
    for s in (-1.0, 1.0):
        bone = side_name("tongue_fork", s)
        bar(rig, bone, side_name("tongue_fork_a", s), (mx(0.3, s), 3.0, -26.0), (mx(1.0, s), 2.5, -28.0), 0.26, 0.17, mat="mouth", up=(1, 0, 0))
        bar(rig, bone, side_name("tongue_fork_b", s), (mx(1.0, s), 2.5, -28.0), (mx(1.7, s), 2.0, -29.8), 0.2, 0.14, mat="mouth", up=(1, 0, 0))


# ===========================================================================
# 3. 颈
# ===========================================================================
NECK_PATH = [(0.0, 6.9, -6.0), (0.0, 6.9, -8.2), (0.0, 6.7, -10.4)]


def part_neck(rig: Rig) -> None:
    """两节粗颈：鳞套 + 背面暗鳞 + 颈背骨刺 + 颈下苍白喉囊。"""
    for i in range(2):
        a, b = NECK_PATH[i], NECK_PATH[i + 1]
        bone = "neck_base" if i == 0 else "neck_upper"
        r = 2.6 - 0.25 * i
        bar(rig, bone, f"neck_core_{i}", a, b, r * 0.78, r, mat="scale_grey", up=(1, 0, 0))
        bar(rig, bone, f"neck_top_{i}", (a[0], a[1] + r * 0.78, a[2]), (b[0], b[1] + r * 0.78, b[2]), 0.3, r * 0.8, mat="scale_dark", up=(1, 0, 0))
        bar(rig, bone, f"neck_throat_{i}", (a[0], a[1] - r * 0.78 - 0.1, a[2]), (b[0], b[1] - r * 0.78 - 0.1, b[2]), 0.3, r * 0.7, mat="belly", up=(1, 0, 0))
        zc = (a[2] + b[2]) / 2
        spike(rig, bone, f"neck_spike{i}", (0.0, a[1] + r * 0.78 + 0.3, zc), (0.0, a[1] + r * 0.78 + 1.8, zc + 0.35), 0.38)
        for s in (-1.0, 1.0):
            spike(rig, bone, side_name(f"neck_side_spike{i}", s), (mx(1.7, s), a[1] + r * 0.5 + 0.2, zc), (mx(2.1, s), a[1] + r * 0.5 + 1.3, zc + 0.3), 0.3)
            for k in range(2):
                box(rig, bone, side_name(f"neck_plate{i}_{k}", s), (mx(r, s), a[1] - 0.8 + 1.2 * k, zc - 0.9 + 0.2 * k), (mx(r + 0.3, s), a[1] + 0.2 + 1.2 * k, zc + 0.1 + 0.2 * k),
                    mat="scale_pale" if (i + k) % 2 else "scale_dark")
    box(rig, "neck_upper", "dewlap", (-1.2, 4.5, -10.0), (1.2, 5.5, -7.4), mat="belly")


# ===========================================================================
# 4. 躯干：鳞桶
# ===========================================================================
SLICES = 13
SLICE_Z0 = -6.0
# (z, 半宽, 脊线 y, 腹线 y)：胸宽，腰略收，臀再宽
PROFILE = [
    (-6.0, 3.0, 8.4, 3.0),
    (-4.0, 4.0, 9.2, 2.8),
    (-1.0, 4.3, 9.6, 2.6),
    (2.5, 4.1, 9.4, 2.7),
    (5.5, 3.9, 9.0, 2.9),
    (7.5, 3.0, 8.0, 3.4),
]
SLICE_JITTER = [0.10, -0.15, 0.20, -0.05, 0.15, -0.10, 0.20, -0.05, 0.15, -0.15, 0.10, -0.05, 0.15]


def torso_profile(z: float) -> tuple[float, float, float]:
    for (z0, *a), (z1, *b) in zip(PROFILE, PROFILE[1:]):
        if z0 <= z <= z1:
            t = (z - z0) / (z1 - z0)
            return tuple(a[k] + (b[k] - a[k]) * t for k in range(3))  # type: ignore[return-value]
    return tuple(PROFILE[-1][1:]) if z > PROFILE[-1][0] else tuple(PROFILE[0][1:])  # type: ignore[return-value]


def slice_geom(i: int) -> tuple[float, float, float, float, float, float]:
    """第 i 片：(z0, z1, 半宽, 脊线 y, 腹线 y, 腹层顶 y)。"""
    z0 = SLICE_Z0 + i
    w, ytop, ybot = torso_profile(z0 + 0.475)
    ytop += SLICE_JITTER[i]
    return z0, z0 + 0.95, w, ytop, ybot, ybot + 0.32 * (ytop - ybot)


def slice_bone(i: int) -> str:
    z = SLICE_Z0 + i + 0.475
    if z < -1.0:
        return "thorax"
    if z < 3.5:
        return "spine_lumbar"
    return "hips"


def part_torso(rig: Rig) -> None:
    """13 片鳞桶：背 / 腰 / 腹三层；两侧每片两枚交错的鳞板；背脊一列骨刺 + 外侧一列小刺。"""
    for i in range(SLICES):
        z0, z1, w, ytop, ybot, ylow = slice_geom(i)
        bone = slice_bone(i)
        ymid = ytop - 2.0
        zc = (z0 + z1) / 2
        box(rig, bone, f"slice{i}_top", (-w * 0.8, ymid, z0), (w * 0.8, ytop, z1), mat="scale_dark" if i % 2 == 0 else "scale_grey")
        box(rig, bone, f"slice{i}_mid", (-w, ylow, z0), (w, ymid, z1), mat="scale_pale" if i % 3 == 0 else "scale_grey")
        box(rig, bone, f"slice{i}_low", (-w * 0.84, ybot, z0), (w * 0.84, ylow, z1), mat="belly")
        spike(rig, bone, f"dorsal_spike{i}", (0.0, ytop, zc), (0.0, ytop + 1.5 + 0.3 * (i % 3), zc + 0.35), 0.4)
        for s in (-1.0, 1.0):
            # 交错鳞板：上半 / 下半各一枚，错开半片
            yh = (ymid + ylow) / 2
            box(rig, bone, side_name(f"plate_hi{i}", s), (mx(w - 0.05, s), yh, z0), (mx(w + 0.32, s), ymid, z0 + 0.55),
                mat="scale_pale" if (i + (s > 0)) % 2 else "scale_dark")
            box(rig, bone, side_name(f"plate_lo{i}", s), (mx(w - 0.05, s), ylow, z0 + 0.45), (mx(w + 0.32, s), yh, z1),
                mat="scale_dark" if (i + (s > 0)) % 2 else "scale_pale")
            # 肩背斜面上再加一枚鳞板：与下面两枚错开，三枚叠成瓦片
            box(rig, bone, side_name(f"plate_top{i}", s), (mx(w * 0.8 - 0.05, s), ymid, z0 + 0.25), (mx(w * 0.8 + 0.3, s), ytop - 0.1, z0 + 0.8),
                mat="scale_dark" if (i + (s > 0)) % 2 else "scale_pale")
            if i % 2 == 1:
                spike(rig, bone, side_name(f"scute{i}", s), (mx(w * 0.62, s), ytop, zc), (mx(w * 0.62 + 0.3, s), ytop + 1.0, zc + 0.3), 0.3)


# ===========================================================================
# 5. 四肢
# ===========================================================================
def toe_fan(rig: Rig, bone_in: str, bone_out: str, s: float, tag: str, base: Vec, forward: float) -> None:
    """五趾张开成扇：每趾 2 节趾骨 + 一枚弯向地面的钩爪。内三趾挂 bone_in，外二趾挂 bone_out。

    base 是掌前缘中点（右侧坐标，x 已带 s）；forward 取 -1（朝 -Z 前）。
    趾偏角：中间一趾正前，两侧各外撇。趾尖落在 y≈0.2。
    """
    offs = (-2.5, -1.25, 0.0, 1.25, 2.5)
    lens = (2.4, 3.1, 3.4, 3.0, 2.3)
    for k, (o, ln) in enumerate(zip(offs, lens)):
        bone = bone_in if k < 3 else bone_out
        sx = o * s
        a = (base[0] + sx, 0.95, base[2])
        b = (base[0] + sx * 1.35, 0.7, base[2] + forward * ln * 0.55)
        c = (base[0] + sx * 1.7, 0.55, base[2] + forward * ln)
        d = (base[0] + sx * 1.85, 0.2, base[2] + forward * (ln + 1.35))
        bar(rig, bone, side_name(f"toe_{tag}{k}_a", s), a, b, 0.5, 0.45, mat="scale_grey")
        bar(rig, bone, side_name(f"toe_{tag}{k}_b", s), b, c, 0.42, 0.38, mat="scale_dark")
        bar(rig, bone, side_name(f"claw_{tag}{k}", s), c, d, 0.24, 0.22, mat="claw_dark")


def limb_scales(rig: Rig, bone: str, name: str, a: Vec, b: Vec, r: float, s: float, n: int) -> None:
    """沿一段肢干上表面排 n 枚鳞板，浅深交替，前后错位。"""
    for k in range(n):
        t = (k + 0.5) / n
        p = lerp3(a, b, t)
        half = 0.5 * math.dist(a, b) / n
        box(rig, bone, side_name(f"{name}{k}", s), (p[0] - r * 0.7, p[1] + r * 0.55, p[2] - half + 0.05 * (k % 2)),
            (p[0] + r * 0.7, p[1] + r * 0.55 + 0.35, p[2] + half - 0.05 * ((k + 1) % 2)), mat="scale_pale" if k % 2 else "scale_dark")


def front_leg(rig: Rig, s: float) -> None:
    """上臂平伸（肘外翻） → 前臂向前下踩 → 掌 → 五趾。"""
    ar, fo, hd = side_name("arm", s), side_name("forearm", s), side_name("hand", s)
    shoulder, elbow, wrist = (mx(3.6, s), 6.2, -4.6), (mx(7.4, s), 4.8, -5.8), (mx(6.6, s), 1.2, -8.2)
    bar(rig, ar, side_name("humerus", s), shoulder, elbow, 1.45, 1.3, mat="scale_grey", up=(0, 1, 0))
    limb_scales(rig, ar, "arm_scale", shoulder, elbow, 1.3, s, 4)
    box(rig, ar, side_name("shoulder_mass", s), (mx(3.5, s), 4.6, -6.0), (mx(5.2, s), 7.4, -3.3), mat="scale_dark")
    bar(rig, fo, side_name("radius", s), elbow, wrist, 1.15, 1.0, mat="scale_dark", up=(1, 0, 0))
    limb_scales(rig, fo, "forearm_scale", (elbow[0], elbow[1] + 0.2, elbow[2]), (wrist[0], wrist[1] + 1.0, wrist[2]), 1.0, s, 3)
    for k in range(2):
        spike(rig, fo, side_name(f"elbow_spike{k}", s), (mx(8.15, s), 5.4 - 0.9 * k, -5.8 + 0.7 * k), (mx(9.6, s), 5.9 - 0.9 * k, -5.2 + 0.7 * k), 0.34)
    # 掌：厚垫 + 腕鳞
    box(rig, hd, side_name("palm", s), (mx(5.0, s), 0.0, -9.0), (mx(8.2, s), 1.5, -7.0), mat="scale_grey")
    box(rig, hd, side_name("palm_pad", s), (mx(5.4, s), 0.0, -9.0), (mx(7.8, s), 0.35, -7.4), mat="belly")
    box(rig, hd, side_name("wrist_scale", s), (mx(5.6, s), 1.5, -8.6), (mx(7.6, s), 1.95, -7.6), mat="scale_pale")
    toe_fan(rig, side_name("toe_f_in", s), side_name("toe_f_out", s), s, "f", (mx(6.6, s), 0.95, -9.0), -1.0)


def hind_leg(rig: Rig, s: float) -> None:
    """大腿外翻向前 → 小腿向后下 → 跗 → 五趾朝前。"""
    th, sn, ft = side_name("thigh", s), side_name("shin", s), side_name("foot", s)
    hip, knee, ankle = (mx(3.6, s), 6.0, 5.2), (mx(7.6, s), 4.4, 3.6), (mx(6.8, s), 1.1, 7.4)
    bar(rig, th, side_name("femur", s), hip, knee, 1.7, 1.5, mat="scale_grey", up=(0, 1, 0))
    limb_scales(rig, th, "thigh_scale", hip, knee, 1.5, s, 4)
    box(rig, th, side_name("haunch_mass", s), (mx(3.4, s), 4.4, 3.8), (mx(5.6, s), 7.6, 6.8), mat="scale_dark")
    bar(rig, sn, side_name("tibia", s), knee, ankle, 1.2, 1.05, mat="scale_dark", up=(1, 0, 0))
    limb_scales(rig, sn, "shin_scale", (knee[0], knee[1] + 0.2, knee[2]), (ankle[0], ankle[1] + 1.0, ankle[2]), 1.05, s, 3)
    for k in range(2):
        spike(rig, sn, side_name(f"knee_spike{k}", s), (mx(8.0, s), 2.6 - 0.8 * k, 4.6 + 0.9 * k), (mx(9.4, s), 3.1 - 0.8 * k, 4.4 + 0.9 * k), 0.34)
    box(rig, ft, side_name("heel", s), (mx(5.6, s), 0.0, 6.6), (mx(8.0, s), 1.8, 8.4), mat="scale_grey")
    box(rig, ft, side_name("sole_pad", s), (mx(5.8, s), 0.0, 5.6), (mx(7.8, s), 0.35, 8.2), mat="belly")
    box(rig, ft, side_name("ankle_scale", s), (mx(5.9, s), 1.8, 6.8), (mx(7.7, s), 2.25, 8.0), mat="scale_pale")
    toe_fan(rig, side_name("toe_b_in", s), side_name("toe_b_out", s), s, "b", (mx(6.8, s), 0.95, 5.6), -1.0)


def part_limbs(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        front_leg(rig, s)
        hind_leg(rig, s)


# ===========================================================================
# 6. 尾
# ===========================================================================
TAIL_PTS = [(6.0, 7.0), (5.7, 10.0), (5.2, 13.0), (4.5, 16.0), (3.6, 19.0), (2.7, 22.0), (1.9, 25.0)]
TAIL_R = [3.4, 2.9, 2.4, 1.9, 1.45, 1.05, 0.7]


def part_tail(rig: Rig) -> None:
    """6 节尾：每节 = 鳞芯 + 苍白腹鳞 + 一圈暗环 + 一枚背刺；渐细，尾尖贴近地面。"""
    for i in range(6):
        (y0, z0), (y1, z1) = TAIL_PTS[i], TAIL_PTS[i + 1]
        r0, r1 = TAIL_R[i], TAIL_R[i + 1]
        r = (r0 + r1) / 2
        bone = f"tail_{i + 1}"
        a, b = (0.0, y0, z0), (0.0, y1, z1)
        bar(rig, bone, f"tail_core{i}", a, b, r * 0.72, r, mat="scale_grey", up=(1, 0, 0))
        bar(rig, bone, f"tail_belly{i}", (0.0, y0 - r * 0.72 - 0.12, z0), (0.0, y1 - r * 0.72 - 0.12, z1), 0.22, r * 0.78, mat="belly", up=(1, 0, 0))
        ring_a, ring_b = lerp3(a, b, 0.42), lerp3(a, b, 0.62)
        bar(rig, bone, f"tail_ring{i}", ring_a, ring_b, r0 * 0.78, r0 * 1.12, mat="scale_dark", up=(1, 0, 0))
        for s in (-1.0, 1.0):
            for k, t0 in enumerate((0.05, 0.62)):
                p0 = lerp3(a, b, t0)
                p1 = lerp3(a, b, t0 + 0.33)
                box(rig, bone, side_name(f"tail_plate{i}_{k}", s), (mx(r * 1.0 - 0.05, s), min(p0[1], p1[1]) - r * 0.35, p0[2]),
                    (mx(r * 1.0 + 0.28, s), max(p0[1], p1[1]) + r * 0.35, p1[2]), mat="scale_pale" if (i + k) % 2 else "scale_dark")
        mid = lerp3(a, b, 0.25)
        spike(rig, bone, f"tail_spike{i}", (0.0, mid[1] + r * 0.72, mid[2]), (0.0, mid[1] + r * 0.72 + 1.3 * (r / 3.0) + 0.4, mid[2] + 0.35), max(r * 0.16, 0.2))
    bar(rig, "tail_6", "tail_tip", (0.0, 1.9, 25.0), (0.0, 1.35, 26.6), 0.38, 0.5, mat="scale_dark", up=(1, 0, 0))


PARTS = {
    "head": ("1. 长楔形头：瘢痕 + 骨瘤环 + 锥齿 + 张口下颌", part_head),
    "tongue": ("2. 细长分叉舌", part_tongue),
    "neck": ("3. 两节粗颈 + 喉囊 + 颈刺", part_neck),
    "torso": ("4. 13 片鳞桶 + 交错鳞板 + 背刺", part_torso),
    "limbs": ("5. 四肢：外翻肘膝 + 五趾钩爪", part_limbs),
    "tail": ("6. 6 节渐细尾", part_tail),
}


def build_full() -> Rig:
    rig = build_rig()
    for _, func in PARTS.values():
        func(rig)
    return rig


def moving_bones(rig: Rig) -> list[str]:
    """带几何的骨：只做分组不挂几何的不算（root 本身不动）。"""
    return [n for n in rig.bone_order if n != "root" and rig.bones[n]["children"]]


# ===========================================================================
# 门禁
# ===========================================================================
# 软覆盖：各色鳞片在肢根、肩髋、鳞板叠压是造型意图；骨瘤 / 刺 / 爪 / 口腔与鳞的互穿仍要报。
_SCALES = ("scale_grey", "scale_pale", "scale_dark", "belly")
SOFT_OVER = frozenset(frozenset((a, b)) for i, a in enumerate(_SCALES) for b in _SCALES[i + 1:])


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制，去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates(
    "灰鳞穴蜥 / hui_lin_xue_xi",
    tuple(MATS),
    asym=(),
    soft_over=SOFT_OVER,
)


def self_test() -> None:
    print("运行 gen_hui_lin_xue_xi.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    cubes = len(rig.elements)
    moving = moving_bones(rig)

    assert abs(lo[1]) < 0.12, f"贴地失败：最低点 y={lo[1]:.3f}，掌垫 / 爪尖应落在 0 附近"
    assert 8.0 <= hi[1] <= 14.0, f"体高 {hi[1]:.1f}px 超出 8..14（伏地蜥蜴）"
    length = hi[2] - lo[2]
    assert 40.0 <= length <= 62.0, f"含舌纵长 {length:.1f}px 超出 40..62"
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert cubes >= 450, f"部件数 {cubes} < 450（对标枯皮狼 Round 3 的 568）"
    print(f"  [OK] 贴地 y={lo[1]:.2f} 体高 {hi[1]:.1f}px 纵长 {length:.1f}px 部件 {cubes} 可动骨 {len(moving)}")

    broken = GATES.self_test(rig)
    assert broken == 0, f"{broken} 道门没有鉴别力（注入缺陷未被报出）"
    print("✓ 差分自证全绿")


# ===========================================================================
# 渲染：逐件预览 + 同取景接触表
# ===========================================================================
BG = (119, 119, 119)


def _tmp_model(rig: Rig, name: str) -> Path:
    path = CREATURE_DIR / f".{name}.tmp.bbmodel"
    path.write_text(json.dumps(rig.bbmodel(name, namespace="bong"), ensure_ascii=False), encoding="utf-8")
    return path


def focus_of(rig: Rig) -> tuple[tuple[float, float, float], float]:
    lo, hi = rig.bounds()
    center = tuple((lo[i] + hi[i]) / 2 for i in range(3))
    span = max(hi[i] - lo[i] for i in range(3)) * 1.1
    return center, span


def render_part_previews() -> None:
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(16), load_font(13)
    print("渲染逐件预览 ...")
    for i, (key, (title, func)) in enumerate(PARTS.items(), start=1):
        rig = build_rig()
        func(rig)
        path = _tmp_model(rig, f"part_{key}")
        focus = focus_of(rig)
        views = [
            ("头吻面 FRONT：相机在 -Z", 180.0, 0.0),
            ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0),
            ("左肋面 LEFT：相机在 -X", 90.0, 0.0),
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


# 六视角：标签写实际照到的面。render() 的 yaw 方向用「+X / -Z 标记块」实测过：
#   yaw=0   相机在 +Z（照到尾与臀，+X 在画面右）   yaw=180 相机在 -Z（照到头吻，+X 在画面左）
#   yaw=-90 相机在 +X（照到右肋，头朝画面右）      yaw=90  相机在 -X（照到左肋，头朝画面左）
SIX_VIEWS = [
    ("头吻面 FRONT：相机在 -Z，照到 -Z 面", 180.0, 0.0),
    ("尾臀面 REAR：相机在 +Z，照到 +Z 面", 0.0, 0.0),
    ("左肋面 LEFT：相机在 -X，照到 -X 面", 90.0, 0.0),
    ("右肋面 RIGHT：相机在 +X，照到 +X 面", -90.0, 0.0),
    ("脊背面 TOP：相机在 +Y，照到 +Y 面", 0.0, 89.9),
    ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0),
]


def view_spans(rig: Rig) -> list[float]:
    """六个视角各自的取景尺寸：伏地的长蜥蜴若全用最长边，正 / 背面只占画面一小条，看不清。"""
    lo, hi = rig.bounds()
    dx, dy, dz = (hi[i] - lo[i] for i in range(3))
    return [max(dx, dy) * 1.15, max(dx, dy) * 1.15, max(dy, dz) * 1.05, max(dy, dz) * 1.05, max(dx, dz) * 1.05, max(dx, dy, dz) * 0.8]


def six_view_sheet(path: Path, header: str, focus, cell: int = 460, spans: list[float] | None = None) -> Image.Image:
    font_h, font_l = load_font(18), load_font(13)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 56), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), header, fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(SIX_VIEWS):
        px, py = 5 + (i % 3) * (cell + 5), 34 + (i // 3) * (cell + 10)
        view_focus = (focus[0], spans[i]) if spans else focus
        im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=view_focus)
        sheet.paste(im, (px, py))
        d.rectangle([px, py, px + 300, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


# 骨骼树的中文标签：每根骨一个人话名字，读树的人不必猜 neck_upper 是什么
BONE_LABELS = {
    "root": "根", "hips": "臀", "spine_lumbar": "腰脊", "thorax": "胸脊", "neck_base": "颈根", "neck_upper": "颈上",
    "head": "头", "jaw": "下颌", "tongue_1": "舌根", "tongue_2": "舌中",
    "tail_1": "尾 1", "tail_2": "尾 2", "tail_3": "尾 3", "tail_4": "尾 4", "tail_5": "尾 5", "tail_6": "尾 6",
    "nodule": "骨瘤", "tongue_fork": "舌叉", "arm": "上臂", "forearm": "前臂", "hand": "前掌",
    "toe_f_in": "前内趾", "toe_f_out": "前外趾", "thigh": "大腿", "shin": "小腿", "foot": "后跗",
    "toe_b_in": "后内趾", "toe_b_out": "后外趾",
}


def bone_label(name: str) -> str:
    for suffix, tag in (("_l", "左"), ("_r", "右")):
        if name.endswith(suffix) and name[: -len(suffix)] in BONE_LABELS:
            return f"{tag}{BONE_LABELS[name[: -len(suffix)]]}"
    return BONE_LABELS.get(name, name)


def bone_tree_lines(rig: Rig) -> list[str]:
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines: list[str] = []

    def walk(name: str, depth: int) -> None:
        cnt = len(rig.bones[name]["children"])
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{bone_label(name)} {name} ({cnt})")
        for kid in children.get(name, []):
            walk(kid, depth + 1)

    walk("root", 0)
    return lines


def bone_tree_panel(rig: Rig, height: int) -> Image.Image:
    lines = bone_tree_lines(rig)
    font_h, font_s, font_t = load_font(16), load_font(13), load_font(12)
    step = 15
    panel = Image.new("RGB", (380, max(height, 90 + len(lines) * step)), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.text((12, 10), "骨骼层级树（括号内为挂载 cube 数）", fill=(180, 235, 200), font=font_h)
    d.text((12, 34), f"骨骼 {len(rig.bones)}（可动 {len(moving_bones(rig))}，要求 ≥ {MIN_MOVING_BONES}）", fill=(240, 210, 150), font=font_s)
    d.text((12, 54), f"cube {len(rig.elements)}", fill=(190, 210, 235), font=font_s)
    y = 76
    for ln in lines:
        d.text((12, y), ln, fill=(230, 230, 235), font=font_t)
        y += step
    return panel


def render_check(rig: Rig, now_path: Path) -> None:
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    font_h = load_font(18)
    focus = focus_of(rig)
    sheet_now = six_view_sheet(now_path, f"NOW {ROUND_TAG}｜{len(rig.bones)} 骨 / {len(rig.elements)} cube", focus, spans=view_spans(rig))
    sheet_now.save(REVIEW_DIR / "render.png")
    sheet_now.save(REVIEW_DIR / "contact_sheet.png")
    tree = bone_tree_panel(rig, sheet_now.height)
    tree.save(REVIEW_DIR / "bones_tree.png")

    row_h = max(sheet_now.height, tree.height)
    ref = Image.open(EXPLODED_REF).convert("RGB") if EXPLODED_REF.exists() else Image.new("RGB", (400, row_h), (50, 50, 50))
    ref = ref.resize((int(ref.width * row_h / ref.height), row_h), Image.Resampling.LANCZOS)

    cols = [("参考图 / 爆炸图 EXPLODED REF（同一只蜥）", ref),
            (f"NOW：{ROUND_TAG}（{len(rig.elements)} cube / {len(rig.bones)} 骨）", sheet_now),
            (f"骨骼树（{len(rig.bones)} 骨）", tree)]

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
    ap = argparse.ArgumentParser(description="灰鳞穴蜥 (hui_lin_xue_xi) 生成器 Round 1/3")
    ap.add_argument("--self-test", action="store_true", help="门禁差分自证")
    ap.add_argument("--list", action="store_true", help="列出编号部件")
    ap.add_argument("--part", choices=sorted(PARTS), help="只输出单个部件的 bbmodel")
    ap.add_argument("--no-render", action="store_true", help="只写 bbmodel，不出预览图")
    args = ap.parse_args()

    if args.list:
        for key, (title, _) in PARTS.items():
            print(f"  {key:8s} {title}")
        return
    if args.self_test:
        self_test()
        return

    self_test()
    if args.part:
        rig = build_rig()
        PARTS[args.part][1](rig)
        out = CREATURE_DIR / f"HuiLinXueXi_{args.part}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(f"HuiLinXueXi_{args.part}", namespace="bong"), ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"✓ {out}（{len(rig.elements)} cube）")
        return

    rig = build_full()
    doc = rig.bbmodel("HuiLinXueXi", namespace="bong")
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
