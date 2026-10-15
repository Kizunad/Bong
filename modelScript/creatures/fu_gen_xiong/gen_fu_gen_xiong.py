#!/usr/bin/env python3
"""腐根熊（fu_gen_xiong）四足生物模型程序化生成器 —— Round 3/3（几何）。

依据：docs/worldview.md §十三 青云残峰。任务卡第五批 #23：身上长出腐烂树根和菌褶的大熊，
半边毛皮被根须顶穿，一只眼被菌盖盖住，肩上冒出枯枝。

  1. 头      宽颅 + 短厚吻 + 张口 24° 的下颌与獠牙 + 圆耳；左眼被一顶菌盖盖住，菌褶垂到颊上；右眼是小黑窝
  2. 颈      短粗两节颈 + 颈鬃
  3. 躯干    24 片毛桶（Round 1 为 16 片，用户：「太短了身体」→ 加长一半）（背 / 腰 / 腹三层），肩隆最高；两侧交错毛板、背上毛刺
  4. 根      两侧肋上不对称的树根：主根斜跨肋侧并分杈；左侧一个腐洞（暗洞 + 菌褶衬）
  5. 菌      肩隆、背脊两处、两侧肋上各一簇托架菌：菌盖（两层）+ 菌褶 + 菌柄
  6. 枯枝    每侧肩上 4 根分杈的枯枝，三级分叉
  7. 四肢    粗壮四肢：肩 - 臂 - 前臂 - 掌 - 趾；后肢大腿 - 小腿 - 跟 - 掌 - 趾；下半截长苔，每脚五趾五爪
  8. 尾      两节短尾

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X。

用法：
  python3 modelScript/creatures/fu_gen_xiong/gen_fu_gen_xiong.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/fu_gen_xiong/gen_fu_gen_xiong.py --self-test # 门禁差分自证
  python3 modelScript/creatures/fu_gen_xiong/gen_fu_gen_xiong.py --list
  python3 modelScript/creatures/fu_gen_xiong/gen_fu_gen_xiong.py --part head
"""

from __future__ import annotations

import argparse
import json
import math
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig, shaft_box  # noqa: E402

CREATURE_DIR = REPO / "modelScript" / "creatures" / "fu_gen_xiong"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/fu_gen_xiong")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "FuGenXiong.bbmodel"
BBMODEL_COPY = MODELS_DIR / "FuGenXiong.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质必须 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "fur_brown": (98, 66, 44),      # 棕毛主色
    "fur_dark": (58, 40, 30),       # 深褐底毛
    "moss": (92, 96, 54),           # 肢下半截的苔
    "root_pale": (206, 192, 160),   # 根须、爪、齿
    "wood": (124, 94, 64),          # 枯枝、菌柄、根节
    "cap": (184, 96, 50),           # 菌盖
    "gill": (222, 184, 128),        # 菌褶、吻部浅毛
    "hole": (30, 22, 18),           # 腐洞、鼻、眼窝
}

# ---------------------------------------------------------------------------
# 骨骼：(名称, pivot(左侧 x 为负), 父骨)。右侧由 define_bones 生成。
# ---------------------------------------------------------------------------
CENTER_BONES = [
    ("root", (0.0, 0.0, 0.0), None),
    # 脊线加长到 24 片：胸 / 腰 / 中段 / 臀 四节，后半身（中段与臀）整体比 Round 1 后移 8 单位
    ("spine_lumbar", (0.0, 16.0, 3.0), "root"),
    ("spine_mid", (0.0, 16.4, 9.5), "spine_lumbar"),
    ("hips", (0.0, 15.0, 16.0), "spine_mid"),
    ("thorax", (0.0, 17.0, -3.5), "spine_lumbar"),
    ("neck_base", (0.0, 16.5, -7.5), "thorax"),
    ("neck_upper", (0.0, 15.6, -10.0), "neck_base"),
    ("head", (0.0, 14.2, -12.6), "neck_upper"),
    ("snout", (0.0, 13.0, -16.2), "head"),
    ("jaw", (0.0, 11.0, -13.4), "head"),
    ("eye_cap", (-3.0, 16.4, -15.6), "head"),
    ("fungus_hump", (0.0, 21.0, -4.0), "thorax"),
    ("fungus_back_a", (0.0, 19.4, 1.0), "spine_lumbar"),
    ("fungus_back_b", (0.0, 18.4, 6.0), "spine_mid"),
    ("fungus_back_c", (0.0, 18.2, 12.0), "spine_mid"),
    ("tail_1", (0.0, 15.6, 17.6), "hips"),
    ("tail_2", (0.0, 14.8, 19.8), "tail_1"),
    # 跨背细根（Round 3）：左右落点不同，各自一根骨，挂在所跨的那一段脊骨下
    ("tendril_a", (0.0, 18.0, 4.2), "spine_lumbar"),
    ("tendril_b", (0.0, 17.6, 8.9), "spine_mid"),
    ("tendril_c", (0.0, 16.0, 16.8), "hips"),
]

SIDE_BONES = [
    ("ear", (3.0, 17.0, -12.6), "head"),
    ("shoulder", (4.4, 16.0, -4.4), "thorax"),
    ("branch", (3.0, 21.0, -5.0), "shoulder"),
    ("root_flank", (5.4, 15.0, -2.0), "thorax"),
    ("fungus_flank", (5.4, 16.0, 2.0), "spine_lumbar"),
    ("root_rear", (5.4, 15.0, 9.0), "spine_mid"),
    ("fungus_rear", (5.4, 16.0, 11.0), "spine_mid"),
    ("arm", (4.6, 14.0, -4.4), "shoulder"),
    ("forearm", (5.0, 9.5, -3.6), "arm"),
    ("paw_f", (4.4, 3.8, -5.2), "forearm"),
    ("toes_f", (4.4, 0.9, -6.8), "paw_f"),
    ("thigh", (4.4, 15.0, 16.0), "hips"),
    ("shin", (5.0, 9.8, 14.0), "thigh"),
    ("hock", (4.4, 4.0, 18.0), "shin"),
    ("paw_b", (4.4, 2.6, 18.2), "hock"),
    ("toes_b", (4.4, 0.9, 16.4), "paw_b"),
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


def add3(a: Vec, b: Vec) -> Vec:
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def scale3(a: Vec, k: float) -> Vec:
    return (a[0] * k, a[1] * k, a[2] * k)


def norm3(a: Vec) -> Vec:
    n = math.sqrt(a[0] ** 2 + a[1] ** 2 + a[2] ** 2) or 1.0
    return (a[0] / n, a[1] / n, a[2] / n)


def tuft(rig: Rig, bone: str, name: str, base: Vec, height: float, width: float, mat: str) -> None:
    """一根向后（+Z）倾的毛刺：两段，下粗上细。"""
    mid = (base[0], base[1] + height * 0.55, base[2] + height * 0.12)
    tip = (base[0], base[1] + height, base[2] + height * 0.38)
    bar(rig, bone, f"{name}_a", base, mid, width * 0.5, width * 0.4, mat=mat, up=(1, 0, 0))
    bar(rig, bone, f"{name}_b", mid, tip, width * 0.3, width * 0.25, mat=mat, up=(1, 0, 0))


def shelf(rig: Rig, bone: str, name: str, center: Vec, radius: float, base_y: float) -> None:
    """一朵托架菌：下层宽盖 + 上层窄盖 + 盖下的菌褶 + 一根菌柄落到 base_y（体表）。"""
    cx, y, cz = center
    r = radius
    box(rig, bone, f"{name}_cap_lo", (cx - r, y, cz - r * 0.85), (cx + r, y + 0.45, cz + r * 0.85), mat="cap")
    box(rig, bone, f"{name}_cap_hi", (cx - r * 0.62, y + 0.45, cz - r * 0.52), (cx + r * 0.62, y + 0.85, cz + r * 0.52), mat="cap")
    box(rig, bone, f"{name}_gill", (cx - r * 0.78, y - 0.35, cz - r * 0.66), (cx + r * 0.78, y, cz + r * 0.66), mat="gill")
    if y - 0.35 - base_y > 0.3:
        bar(rig, bone, f"{name}_stalk", (cx, base_y, cz), (cx, y - 0.35, cz), 0.26, 0.26, mat="wood")


def twig(rig: Rig, bone: str, name: str, a: Vec, b: Vec, r: float) -> None:
    """一段枯枝：木色方柱，尖端略细。"""
    bar(rig, bone, name, a, b, r, r, mat="wood", up=(0, 0, 1) if abs(b[2] - a[2]) < 0.8 * math.dist(a, b) else (1, 0, 0))


# ===========================================================================
# 1. 头
# ===========================================================================
JAW_HINGE = (0.0, 11.0, -13.4)
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
    """宽颅 + 短厚吻 + 张口下颌 + 獠牙 + 圆耳；左眼盖着菌盖，右眼只剩小黑窝。"""
    box(rig, "head", "cranium", (-3.7, 11.8, -16.2), (3.7, 17.0, -11.0), mat="fur_brown")
    box(rig, "head", "forehead", (-3.0, 17.0, -15.6), (3.0, 17.6, -11.6), mat="fur_dark")
    box(rig, "head", "occiput", (-3.3, 12.2, -11.0), (3.3, 16.6, -9.8), mat="fur_dark")
    box(rig, "head", "brow_bar", (-3.3, 16.3, -16.7), (3.3, 17.0, -16.2), mat="fur_dark")
    for s in (-1.0, 1.0):
        box(rig, "head", side_name("cheek_ruff", s), (mx(3.7, s), 11.6, -15.6), (mx(4.5, s), 15.4, -11.2), mat="fur_brown")
        box(rig, "head", side_name("cheek_ruff_low", s), (mx(3.5, s), 10.4, -15.0), (mx(4.3, s), 11.8, -11.6), mat="fur_dark")
        # 耳：圆耳，外层毛 + 内层菌褶色
        box(rig, side_name("ear", s), side_name("ear_outer", s), (mx(2.5, s), 17.0, -13.4), (mx(4.5, s), 19.0, -11.8), mat="fur_dark")
        box(rig, side_name("ear", s), side_name("ear_inner", s), (mx(2.8, s), 17.5, -13.7), (mx(4.0, s), 18.6, -13.4), mat="gill")
    # 右眼：小黑窝
    box(rig, "head", "eye_socket_r", (2.2, 14.8, -16.5), (3.2, 15.8, -16.1), mat="hole")
    box(rig, "head", "eye_glint_r", (2.5, 15.1, -16.75), (2.9, 15.5, -16.5), mat="gill")

    # ── 左眼被菌盖盖住：盖面两层 + 菌褶垂到颊上 + 三缕垂下的褶丝 ──
    box(rig, "eye_cap", "eye_cap_lo", (-4.2, 15.9, -17.2), (-1.7, 16.45, -14.0), mat="cap")
    box(rig, "eye_cap", "eye_cap_hi", (-3.6, 16.45, -16.6), (-2.3, 16.85, -14.8), mat="cap")
    box(rig, "eye_cap", "eye_cap_gill", (-3.9, 15.2, -17.0), (-2.0, 15.9, -16.25), mat="gill")
    for k, (x, z, ln) in enumerate(((-3.5, -16.6, 1.8), (-2.8, -16.4, 2.4), (-2.2, -16.7, 1.5))):
        bar(rig, "eye_cap", f"eye_cap_strand{k}", (x, 15.2, z), (x, 15.2 - ln, z), 0.2, 0.2, mat="gill", up=(1, 0, 0))
    bar(rig, "eye_cap", "eye_cap_root", (-2.8, 14.8, -16.3), (-2.8, 15.2, -16.3), 0.5, 0.4, mat="wood")

    # ── 吻：短而厚，三段收窄；吻部是浅色毛 ──
    pts = [(0.0, 13.2, -16.2), (0.0, 12.9, -18.2), (0.0, 12.6, -20.0), (0.0, 12.4, -21.2)]
    hw = [2.6, 2.3, 2.0, 1.7]
    hh = [2.2, 1.95, 1.7, 1.45]
    for i in range(3):
        bar(rig, "snout", f"muzzle_{i}", pts[i], pts[i + 1], (hw[i] + hw[i + 1]) / 2, (hh[i] + hh[i + 1]) / 2, mat="gill" if i != 0 else "fur_brown",
            up=(0, 1, 0))
    box(rig, "snout", "nose_pad", (-1.4, 12.9, -22.1), (1.4, 14.2, -21.0), mat="hole")
    for s in (-1.0, 1.0):
        bar(rig, "snout", side_name("fang_up_a", s), (mx(1.4, s), 10.9, -19.6), (mx(1.4, s), 9.6, -19.7), 0.32, 0.32, mat="root_pale", up=(1, 0, 0))
        bar(rig, "snout", side_name("fang_up_b", s), (mx(1.4, s), 9.6, -19.7), (mx(1.4, s), 8.6, -19.8), 0.2, 0.2, mat="root_pale", up=(1, 0, 0))
        for k, zc in enumerate((-17.0, -17.9, -18.8)):
            bar(rig, "snout", side_name(f"molar_up{k}", s), (mx(2.0, s), 10.9, zc), (mx(2.0, s), 9.9, zc), 0.28, 0.3, mat="root_pale", up=(1, 0, 0))
    box(rig, "snout", "upper_lip", (-1.9, 10.6, -21.0), (1.9, 11.2, -16.8), mat="fur_dark")

    # ── 下颌：绕铰链张开 24° ──
    box(rig, "jaw", "jaw_hinge_block", (-3.2, 10.4, -14.0), (3.2, 11.6, -12.6), mat="hole")
    for s in (-1.0, 1.0):
        bar(rig, "jaw", side_name("mandible_back", s), jaw_pt(mx(3.0, s), 10.8, -13.6), jaw_pt(mx(2.3, s), 10.2, -17.0), 0.55, 0.75, mat="fur_dark",
            up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("mandible_front", s), jaw_pt(mx(2.3, s), 10.2, -17.0), jaw_pt(mx(1.5, s), 9.9, -20.6), 0.5, 0.65, mat="gill",
            up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("fang_dn_a", s), jaw_pt(mx(1.3, s), 10.5, -19.6), jaw_pt(mx(1.3, s), 11.7, -19.65), 0.3, 0.3, mat="root_pale",
            up=jaw_dir((1, 0, 0)))
        bar(rig, "jaw", side_name("fang_dn_b", s), jaw_pt(mx(1.3, s), 11.7, -19.65), jaw_pt(mx(1.3, s), 12.6, -19.7), 0.18, 0.18, mat="root_pale",
            up=jaw_dir((1, 0, 0)))
        for k, zc in enumerate((-16.2, -17.1, -18.0)):
            bar(rig, "jaw", side_name(f"molar_dn{k}", s), jaw_pt(mx(2.0, s), 10.8, zc), jaw_pt(mx(2.0, s), 11.7, zc), 0.28, 0.3, mat="root_pale",
                up=jaw_dir((1, 0, 0)))
    bar(rig, "jaw", "chin_ruff", jaw_pt(0.0, 9.4, -20.8), jaw_pt(0.0, 9.9, -18.6), 1.3, 0.5, mat="fur_dark", up=(1, 0, 0))
    bar(rig, "jaw", "mouth_floor", jaw_pt(0.0, 10.7, -14.6), jaw_pt(0.0, 10.5, -20.2), 1.5, 0.2, mat="hole", up=jaw_dir((0, 1, 0)))


# ===========================================================================
# 2. 颈
# ===========================================================================
NECK_PATH = [(0.0, 16.9, -6.4), (0.0, 16.2, -8.8), (0.0, 15.0, -11.2)]


def part_neck(rig: Rig) -> None:
    """短粗两节颈：毛套 + 两排颈鬃 + 颈下垂毛。"""
    for i in range(2):
        a, b = NECK_PATH[i], NECK_PATH[i + 1]
        bone = "neck_base" if i == 0 else "neck_upper"
        r = 4.3 - 0.4 * i
        bar(rig, bone, f"neck_fur_{i}", a, b, r * 0.88, r, mat="fur_brown" if i == 0 else "fur_dark", up=(1, 0, 0))
        zc = (a[2] + b[2]) / 2
        for k, z in enumerate((zc - 0.8, zc + 0.8)):
            for s in (-1.0, 1.0):
                for row, xo in enumerate((0.9, 2.3)):
                    tuft(rig, bone, side_name(f"neck_tuft{i}_{k}_{row}", s), (mx(xo, s), a[1] + r * 0.88 - 0.35 * row - 0.2, z), 2.0 - 0.4 * row, 0.9,
                         "fur_dark" if (k + row) % 2 else "fur_brown")
        bar(rig, bone, f"neck_ruff_low_{i}", (a[0], a[1] - r * 0.88 + 0.1, a[2]), (b[0], b[1] - r * 0.88 + 0.1, b[2]), 0.5, r * 0.8, mat="fur_dark", up=(1, 0, 0))


# ===========================================================================
# 3. 躯干：毛桶
# ===========================================================================
SLICES = 24
SLICE_Z0 = -7.0
# (z, 半宽, 脊线 y, 腹线 y)：熊的长背——肩隆最高，背线顺着长长的脊缓缓落下，到臀再微微抬起、收圆
PROFILE = [
    (-7.0, 4.6, 18.0, 7.6),
    (-4.0, 5.4, 21.0, 6.8),
    (0.0, 5.4, 19.8, 7.6),
    (4.0, 5.2, 18.8, 8.2),
    (8.0, 5.0, 18.0, 8.4),
    (12.0, 5.0, 17.4, 8.4),
    (15.0, 5.0, 17.4, 8.2),
    (18.0, 3.8, 16.4, 8.8),
]
SLICE_JITTER = [0.10, -0.20, 0.25, -0.10, 0.20, -0.15, 0.10, 0.25, -0.15, 0.20, -0.05, 0.15, -0.20, 0.10, 0.20, -0.10,
                0.15, -0.10, 0.20, -0.05, 0.10, -0.15, 0.20, -0.10]


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
    return z0, z0 + 0.95, w, ytop, ybot, ybot + 0.3 * (ytop - ybot)


def slice_bone(i: int) -> str:
    z = SLICE_Z0 + i + 0.475
    if z < -0.8:
        return "thorax"
    if z < 6.0:
        return "spine_lumbar"
    if z < 13.0:
        return "spine_mid"
    return "hips"


def flank_x(z: float, s: float, off: float = 0.0) -> float:
    """躯干在 z 处的外缘 x（带符号）；off 向外再退多少。"""
    return mx(torso_profile(z)[0] + off, s)


def part_torso(rig: Rig) -> None:
    """16 片毛桶：背 / 腰 / 腹三层；两侧每片两枚交错毛板；背上交替的毛刺。"""
    for i in range(SLICES):
        z0, z1, w, ytop, ybot, ylow = slice_geom(i)
        bone = slice_bone(i)
        ymid = ytop - 2.2
        zc = (z0 + z1) / 2
        box(rig, bone, f"slice{i}_top", (-w * 0.82, ymid, z0), (w * 0.82, ytop, z1), mat="fur_brown" if i % 2 == 0 else "fur_dark")
        box(rig, bone, f"slice{i}_mid", (-w, ylow, z0), (w, ymid, z1), mat="fur_dark" if i % 3 == 0 else "fur_brown")
        box(rig, bone, f"slice{i}_low", (-w * 0.82, ybot, z0), (w * 0.82, ylow, z1), mat="fur_dark")
        for s in (-1.0, 1.0):
            yh = (ymid + ylow) / 2
            box(rig, bone, side_name(f"plate_hi{i}", s), (mx(w - 0.05, s), yh, z0), (mx(w + 0.4, s), ymid, z0 + 0.55),
                mat="fur_dark" if (i + (s > 0)) % 2 else "fur_brown")
            box(rig, bone, side_name(f"plate_lo{i}", s), (mx(w - 0.05, s), ylow, z0 + 0.45), (mx(w + 0.4, s), yh, z1),
                mat="fur_brown" if (i + (s > 0)) % 2 else "fur_dark")
            if i % 2 == 0:
                tuft(rig, bone, side_name(f"back_tuft{i}", s), (mx(w * 0.55, s), ytop, zc), 1.8 + 0.4 * (i % 3), 0.9, "fur_dark" if i % 4 == 0 else "fur_brown")
        # 肋下垂毛：一片向下伸的深色毛，错落参差
        for s in (-1.0, 1.0):
            if i % 3 == 0:
                bar(rig, bone, side_name(f"belly_fringe{i}", s), (mx(w * 0.8, s), ylow + 0.2, zc), (mx(w * 0.8 + 0.4, s), ybot - 1.6 - 0.3 * (i % 3), zc + 0.2), 0.45, 0.2,
                    mat="fur_dark", up=(s, 0, 0))


# ===========================================================================
# 4. 根：不对称的树根、腐洞
# ===========================================================================
# 每条根：沿 z 给出 (z, y) 折点，外缘 x 取躯干外缘外 0.5 左右；分杈：(取主线折点序号, 向后的 z 偏移, 向下的 y 偏移, 长度折点数)
ROOT_PLANS = {
    -1.0: {
        "trunk": [(-6.0, 19.4), (-4.4, 17.2), (-2.8, 14.8), (-1.0, 12.6), (0.6, 10.6)],
        "branches": [(1, [(-3.4, 18.4), (-1.6, 17.0), (0.2, 16.4)]), (2, [(-1.8, 13.2), (0.0, 13.4), (1.4, 12.2)]),
                     (3, [(0.6, 11.4), (2.2, 9.6)])],
    },
    1.0: {
        "trunk": [(-5.0, 19.8), (-3.0, 17.6), (-1.2, 15.0), (0.8, 12.6), (2.6, 10.4)],
        "branches": [(1, [(-1.8, 18.8), (0.2, 18.0), (2.2, 17.6)]), (2, [(0.0, 15.4), (1.6, 14.0)]),
                     (3, [(2.6, 12.0), (4.2, 10.4), (5.4, 9.6)])],
    },
}


def root_line(rig: Rig, bone: str, name: str, pts: list[tuple[float, float]], s: float, r0: float, off: float = 0.55) -> list[Vec]:
    """沿躯干侧面贴一条根：每段是 square 截面，越到末端越细；返回 3D 折点供分杈用。"""
    pts3 = [(flank_x(z, s, off), y, z) for z, y in pts]
    for k in range(len(pts3) - 1):
        r = r0 * (1.0 - 0.55 * k / max(len(pts3) - 1, 1))
        bar(rig, bone, side_name(f"{name}_{k}", s), pts3[k], pts3[k + 1], r, r, mat="root_pale" if k % 2 == 0 else "wood")
    return pts3


# 加长后背的后半段树根（挂中段脊的 root_rear）：同样左右不对称，左侧一条长根跨到臀，右侧两股短根
REAR_ROOT_PLANS = {
    -1.0: {
        "trunk": [(7.0, 18.0), (9.2, 16.2), (11.4, 14.0), (13.6, 12.2), (15.6, 10.4)],
        "branches": [(1, [(10.6, 17.4), (12.4, 17.0), (14.4, 16.2)]), (2, [(13.0, 14.4), (15.0, 13.4)]),
                     (3, [(14.8, 11.2), (16.4, 9.8)])],
    },
    1.0: {
        "trunk": [(8.4, 18.2), (10.2, 16.0), (12.6, 14.4), (14.4, 12.0), (16.4, 10.6)],
        "branches": [(1, [(11.8, 17.2), (13.6, 17.0)]), (2, [(14.2, 15.0), (16.0, 14.2), (17.0, 13.0)]),
                     (3, [(15.8, 11.0), (17.0, 9.8)])],
    },
}


def lay_roots(rig: Rig, bone: str, plan: dict, s: float, tag: str) -> None:
    """主根 + 分杈 + 根尖 + 根节；tag 区分前后两组，免得 cube 重名。"""
    trunk = root_line(rig, bone, f"{tag}trunk", plan["trunk"], s, 0.5)
    for bi, (node, pts) in enumerate(plan["branches"]):
        start = trunk[node]
        start_zy = (start[2], start[1])
        root_line(rig, bone, f"{tag}br{bi}", [start_zy] + pts, s, 0.32, off=0.55)
        # 根尖：钻进毛里的细根（朝躯干方向伸一小截，表示顶穿毛皮）
        tip = flank_x(pts[-1][0], s, 0.55), pts[-1][1], pts[-1][0]
        bar(rig, bone, side_name(f"{tag}tip{bi}", s), tip, (mx(abs(tip[0]) - 0.6, s), tip[1] - 0.5, tip[2] + 0.3), 0.18, 0.18, mat="root_pale")
    # 根节：主根折点处一粒木色瘤
    for k in (1, 3):
        p = trunk[k]
        box(rig, bone, side_name(f"{tag}knot{k}", s), (p[0] - 0.55 * (1 if s > 0 else -1) - 0.2, p[1] - 0.45, p[2] - 0.45),
            (p[0] + 0.55 * (1 if s > 0 else -1) + 0.2, p[1] + 0.45, p[2] + 0.45), mat="wood")


def part_roots(rig: Rig) -> None:
    """两侧不对称的树根：前半身主根斜跨肋侧并分三杈，后半身（加长的部分）再一组；左侧肋中部一个腐洞。"""
    for s, plan in ROOT_PLANS.items():
        lay_roots(rig, side_name("root_flank", s), plan, s, "root_")
        lay_roots(rig, side_name("root_rear", s), REAR_ROOT_PLANS[s], s, "rroot_")
    # 左侧腐洞：暗洞 + 菌褶衬边 + 洞口上沿的垂褶
    bone = side_name("root_flank", -1.0)
    zc, yc = 1.8, 12.0
    x0 = flank_x(zc, -1.0, 0.0)
    box(rig, bone, "rot_hole_l", (x0 - 0.5, yc - 1.2, zc - 1.1), (x0 + 0.3, yc + 1.2, zc + 1.1), mat="hole")
    for tag, (dy, dz, hy, hz) in {"top": (1.2, 0.0, 0.45, 1.5), "bot": (-1.65, 0.0, 0.45, 1.5), "fro": (0.0, -1.325, 1.6, 0.45), "bak": (0.0, 1.325, 1.6, 0.45)}.items():
        xr = flank_x(zc + dz, -1.0)
        box(rig, bone, f"rot_rim_{tag}_l", (xr - 0.75, yc + dy - hy * 0.5, zc + dz - hz * 0.5), (xr - 0.35, yc + dy + hy * 0.5, zc + dz + hz * 0.5), mat="gill")


# ===========================================================================
# 5. 菌：托架菌簇
# ===========================================================================
def part_fungus(rig: Rig) -> None:
    """肩隆一大簇（6 朵）、背脊两簇（各 5 朵）、两侧肋上各一簇（4 朵），大小高低错落。"""
    hump = [(-1.6, 22.6, -5.6, 2.2), (1.2, 23.4, -4.6, 2.0), (-0.4, 24.2, -3.0, 1.6), (2.4, 22.4, -2.8, 1.5), (-2.6, 23.2, -3.4, 1.3), (0.4, 21.9, -6.8, 1.2)]
    for k, (x, y, z, r) in enumerate(hump):
        base = torso_profile(z)[1] - 0.3
        shelf(rig, "fungus_hump", f"hump_shelf{k}", (x, y, z), r, base)
    back_a = [(0.6, 21.0, 0.2, 1.9), (-1.8, 21.4, 1.6, 1.5), (2.2, 20.4, 2.0, 1.4), (-0.6, 22.0, 2.6, 1.1), (1.4, 21.8, -0.8, 1.0)]
    for k, (x, y, z, r) in enumerate(back_a):
        shelf(rig, "fungus_back_a", f"backa_shelf{k}", (x, y, z), r, torso_profile(z)[1] - 0.3)
    back_b = [(-1.0, 19.4, 6.4, 1.7), (1.8, 19.0, 7.6, 1.4), (-2.4, 18.6, 8.0, 1.2), (0.4, 19.8, 8.8, 1.0), (2.6, 19.2, 6.0, 1.1)]
    for k, (x, y, z, r) in enumerate(back_b):
        shelf(rig, "fungus_back_b", f"backb_shelf{k}", (x, y, z), r, torso_profile(z)[1] - 0.3)
    # 加长的后背再立一簇：沿脊一路到臀，菌盖由大渐小
    back_c = [(0.8, 18.8, 11.4, 1.6), (-1.8, 18.6, 12.8, 1.3), (2.0, 18.4, 13.6, 1.2), (-0.4, 18.9, 14.8, 0.9), (-2.4, 18.2, 15.6, 0.8)]
    for k, (x, y, z, r) in enumerate(back_c):
        shelf(rig, "fungus_back_c", f"backc_shelf{k}", (x, y, z), r, torso_profile(z)[1] - 0.3)
    # 侧面的菌：长在肋侧，盖口朝外（盖是平放的，所以用横向的小台）
    # 前 4 朵挂腰脊，后 3 朵（加长的后半身）挂中段脊
    for s in (-1.0, 1.0):
        for k, (z, y, r) in enumerate(((1.0, 15.6, 1.3), (3.4, 14.4, 1.1), (5.2, 16.0, 1.0), (7.0, 14.2, 0.9),
                                       (9.8, 15.4, 1.2), (12.2, 14.0, 1.0), (14.4, 15.2, 0.9))):
            bone = side_name("fungus_flank" if z < 8.5 else "fungus_rear", s)
            if s < 0 and k == 1:
                continue  # 左肋这一处是腐洞，不长菌
            w = torso_profile(z)[0]
            cx = mx(w + 0.5 + r, s)
            box(rig, bone, side_name(f"flank_cap_lo{k}", s), (cx - r, y, z - r * 0.85), (cx + r, y + 0.45, z + r * 0.85), mat="cap")
            box(rig, bone, side_name(f"flank_cap_hi{k}", s), (cx - r * 0.62, y + 0.45, z - r * 0.52), (cx + r * 0.62, y + 0.85, z + r * 0.52), mat="cap")
            box(rig, bone, side_name(f"flank_gill{k}", s), (cx - r * 0.78, y - 0.35, z - r * 0.66), (cx + r * 0.78, y, z + r * 0.66), mat="gill")
            bar(rig, bone, side_name(f"flank_stalk{k}", s), (mx(w - 0.05, s), y - 0.15, z), (cx, y - 0.15, z), 0.24, 0.24, mat="wood")


# ===========================================================================
# 6. 枯枝：肩上冒出的分杈枯枝
# ===========================================================================
# 每根枯枝：根（相对肩骨右侧坐标）、主方向、长度、半径、三级分杈 [(取主干位置 t, 方向, 长度)]
BRANCHES = [
    ((3.0, 21.0, -5.4), (0.25, 1.0, -0.2), 7.0, 0.55, [(0.45, (0.9, 0.8, -0.1), 3.4), (0.7, (-0.5, 0.9, 0.3), 3.0), (0.9, (0.3, 1.0, 0.4), 2.0)]),
    ((3.8, 20.6, -3.8), (0.55, 0.9, 0.2), 6.0, 0.5, [(0.5, (1.0, 0.5, 0.2), 3.0), (0.75, (0.1, 1.0, -0.4), 2.6)]),
    ((2.2, 21.2, -3.0), (-0.1, 1.0, 0.3), 6.4, 0.5, [(0.55, (0.7, 0.8, 0.4), 3.0), (0.8, (-0.6, 0.9, 0.0), 2.2)]),
    ((4.6, 19.8, -5.6), (0.8, 0.7, -0.2), 5.0, 0.45, [(0.6, (0.6, 1.0, 0.0), 2.4)]),
]


def part_branches(rig: Rig) -> None:
    """每侧肩上 4 根枯枝，三级分杈；左右两侧的枝形是同一份计划的镜像。"""
    for s in (-1.0, 1.0):
        bone = side_name("branch", s)
        for bi, (root, d, ln, r, subs) in enumerate(BRANCHES):
            root_s = (mx(root[0], s), root[1], root[2])
            dv = norm3((d[0] * s, d[1], d[2]))
            mid = add3(root_s, scale3(dv, ln * 0.5))
            tip = add3(root_s, scale3(dv, ln))
            twig(rig, bone, side_name(f"br{bi}_a", s), root_s, mid, r)
            twig(rig, bone, side_name(f"br{bi}_b", s), mid, tip, r * 0.72)
            for si, (t, sd, sl) in enumerate(subs):
                sroot = add3(root_s, scale3(dv, ln * t))
                sdv = norm3((sd[0] * s, sd[1], sd[2]))
                smid = add3(sroot, scale3(sdv, sl * 0.55))
                stip = add3(sroot, scale3(sdv, sl))
                twig(rig, bone, side_name(f"br{bi}_s{si}_a", s), sroot, smid, r * 0.55)
                twig(rig, bone, side_name(f"br{bi}_s{si}_b", s), smid, stip, r * 0.4)
        # 枝根处的木质瘤：枯枝是从肩肉里顶出来的
        box(rig, bone, side_name("branch_burl", s), (mx(2.0, s), 19.6, -6.0), (mx(5.6, s), 21.4, -2.4), mat="wood")


# ===========================================================================
# 7. 四肢
# ===========================================================================
def paw(rig: Rig, bone: str, s: float, tag: str, x0: float, z_back: float, z_front: float) -> None:
    """一只熊掌：掌垫 + 五趾（粗短毛趾）+ 五枚长爪；趾从 z_back 向 z_front（更负）张开。"""
    box(rig, bone, side_name(f"pad_{tag}", s), (x0 - 2.2, 0.0, z_front), (x0 + 2.2, 0.45, z_back), mat="hole")
    offs = (-2.0, -1.0, 0.0, 1.0, 2.0)
    for k, o in enumerate(offs):
        xo = x0 + s * o
        ln = 1.6 + 0.3 * (1 - abs(o) / 2.0)
        a = (xo, 1.3, z_front + 0.4)
        b = (xo + s * o * 0.12, 0.8, z_front - ln * 0.6)
        c = (xo + s * o * 0.2, 0.2, z_front - ln * 0.6 - 1.7)
        bar(rig, bone, side_name(f"toe_{tag}{k}", s), a, b, 0.5, 0.5, mat="fur_dark")
        bar(rig, bone, side_name(f"claw_{tag}{k}", s), b, c, 0.22, 0.22, mat="root_pale")


def front_leg(rig: Rig, s: float) -> None:
    """肩毛块 → 粗臂 → 前臂 → 掌（下半截长苔）→ 五趾五爪。"""
    sh, ar, fo = side_name("shoulder", s), side_name("arm", s), side_name("forearm", s)
    pf, tf = side_name("paw_f", s), side_name("toes_f", s)
    shoulder, elbow, wrist = (mx(4.6, s), 15.5, -4.4), (mx(5.0, s), 9.5, -3.6), (mx(4.4, s), 3.8, -5.2)
    bar(rig, sh, side_name("shoulder_mass", s), (mx(4.8, s), 19.2, -5.0), (mx(5.4, s), 14.4, -4.4), 2.4, 1.2, mat="fur_brown", up=(s, 0, 0))
    bar(rig, sh, side_name("shoulder_fringe", s), (mx(5.8, s), 17.8, -5.0), (mx(6.0, s), 13.8, -4.4), 1.6, 0.4, mat="fur_dark", up=(s, 0, 0))
    bar(rig, ar, side_name("arm_fur", s), shoulder, elbow, 2.5, 2.3, mat="fur_brown")
    bar(rig, ar, side_name("arm_ruff", s), (mx(6.0, s), 14.4, -4.0), (mx(6.3, s), 10.6, -3.4), 1.2, 0.35, mat="fur_dark", up=(s, 0, 0))
    bar(rig, fo, side_name("forearm_fur", s), elbow, wrist, 2.2, 2.0, mat="fur_dark")
    bar(rig, fo, side_name("forearm_moss", s), (mx(6.0, s), 8.6, -3.6), (mx(5.6, s), 5.0, -4.6), 1.1, 0.35, mat="moss", up=(s, 0, 0))
    box(rig, pf, side_name("wrist_block", s), (mx(2.4, s), 1.3, -7.6), (mx(6.4, s), 4.4, -3.4), mat="fur_dark")
    box(rig, pf, side_name("wrist_moss", s), (mx(2.6, s), 2.2, -7.9), (mx(6.2, s), 3.6, -7.5), mat="moss")
    paw(rig, tf, s, "f", mx(4.4, s), -5.2, -9.4)
    box(rig, tf, side_name("palm_block", s), (mx(2.4, s), 0.45, -9.4), (mx(6.4, s), 1.3, -4.2), mat="fur_dark")


HIND_Z = 8.0  # 后半身相对 Round 1 整体后移的距离（躯干加长的量）


def hind_leg(rig: Rig, s: float) -> None:
    """大腿毛块 → 小腿 → 跟（后翘）→ 掌 → 五趾五爪。"""
    th, sn, hk = side_name("thigh", s), side_name("shin", s), side_name("hock", s)
    pb, tb = side_name("paw_b", s), side_name("toes_b", s)
    hip, stifle, hock = (mx(4.4, s), 15.0, HIND_Z + 8.0), (mx(5.0, s), 9.8, HIND_Z + 6.0), (mx(4.4, s), 4.0, HIND_Z + 10.0)
    bar(rig, th, side_name("thigh_mass", s), hip, stifle, 3.0, 2.6, mat="fur_brown")
    bar(rig, th, side_name("thigh_fringe", s), (mx(6.8, s), 15.4, HIND_Z + 8.0), (mx(7.0, s), 10.6, HIND_Z + 6.4), 1.8, 0.4, mat="fur_dark", up=(s, 0, 0))
    bar(rig, sn, side_name("shin_fur", s), stifle, hock, 2.2, 2.0, mat="fur_dark")
    bar(rig, sn, side_name("shin_moss", s), (mx(6.4, s), 8.6, HIND_Z + 6.4), (mx(6.0, s), 5.0, HIND_Z + 9.0), 1.1, 0.35, mat="moss", up=(s, 0, 0))
    box(rig, hk, side_name("heel_block", s), (mx(2.4, s), 1.4, HIND_Z + 9.0), (mx(6.4, s), 5.0, HIND_Z + 11.8), mat="fur_dark")
    box(rig, hk, side_name("heel_moss", s), (mx(2.6, s), 2.4, HIND_Z + 11.8), (mx(6.2, s), 4.0, HIND_Z + 12.2), mat="moss")
    paw(rig, tb, s, "b", mx(4.4, s), HIND_Z + 9.0, HIND_Z + 6.0)
    box(rig, pb, side_name("palm_block", s), (mx(2.4, s), 0.45, HIND_Z + 6.4), (mx(6.4, s), 1.4, HIND_Z + 9.2), mat="fur_dark")


def part_limbs(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        front_leg(rig, s)
        hind_leg(rig, s)


# ===========================================================================
# 8. 尾
# ===========================================================================
def part_tail(rig: Rig) -> None:
    """两节短尾：毛团 + 一撮苔。"""
    bar(rig, "tail_1", "tail_1_fur", (0.0, 15.8, 17.0), (0.0, 14.8, 19.6), 1.7, 1.7, mat="fur_brown", up=(1, 0, 0))
    bar(rig, "tail_2", "tail_2_fur", (0.0, 14.8, 19.6), (0.0, 13.6, 20.8), 1.4, 1.4, mat="fur_dark", up=(1, 0, 0))
    bar(rig, "tail_2", "tail_moss", (0.0, 15.6, 19.2), (0.0, 14.6, 20.6), 0.6, 0.9, mat="moss", up=(1, 0, 0))
    tuft(rig, "tail_2", "tail_tuft", (0.0, 13.6, 20.6), 1.6, 0.9, "fur_brown")


# ===========================================================================
# 9. Round 3 收尾：垂毛 / 垂苔 / 跨背细根 / 菌盖旁的小菌
# ===========================================================================
def shag(rig: Rig, bone: str, name: str, top: Vec, length: float, width: float, s: float, mat: str, kink: float = 0.0) -> None:
    """一绺垂毛：两段向下垂、末端略向外翻，朝外一侧薄（s 是左右符号），宽度沿 z。"""
    mid = (top[0] + s * length * 0.06, top[1] - length * 0.5, top[2] + kink * 0.4)
    end = (top[0] + s * length * 0.16, top[1] - length, top[2] + kink)
    bar(rig, bone, f"{name}_a", top, mid, width * 0.5, 0.22, mat=mat, up=(s, 0, 0))
    bar(rig, bone, f"{name}_b", mid, end, width * 0.34, 0.2, mat=mat, up=(s, 0, 0))


def part_fringe(rig: Rig) -> None:
    """垂毛与垂苔：参考图里熊身上的毛是一绺绺往下垂的，不是平整的毛桶。
    肋侧每隔一片垂一绺（长短交错、深浅相间）、大腿与肩外缘各垂几绺；肢体下半截和菌盖下挂几缕垂苔。"""
    rng = random.Random(2331)
    # 长短随机但左右同一份（毛的参差不是刻意不对称，镜像门禁要求成对）
    flank_len = [(rng.uniform(2.6, 4.4), rng.uniform(-0.3, 0.3)) for _ in range(SLICES)]
    thigh_len = [rng.uniform(3.0, 4.4) for _ in range(3)]
    shoulder_len = [rng.uniform(2.8, 4.0) for _ in range(3)]
    for s in (-1.0, 1.0):
        for i in range(0, SLICES, 2):                                       # 每隔一片垂一绺，长短交错
            z0, z1, w, ytop, ybot, ylow = slice_geom(i)
            length, kink = flank_len[i]
            shag(rig, slice_bone(i), side_name(f"flank_shag{i}", s), (mx(w + 0.45, s), ylow + 1.4, (z0 + z1) / 2), length, 1.0, s,
                 "fur_dark" if i % 3 else "fur_brown", kink=kink)
        # 大腿外缘与肩外缘各垂一排
        for k, dz in enumerate((-1.4, 0.0, 1.4)):
            shag(rig, side_name("thigh", s), side_name(f"thigh_shag{k}", s), (mx(7.1, s), 14.6 - 0.3 * (k % 2), HIND_Z + 7.4 + dz), thigh_len[k], 1.0, s,
                 "fur_brown" if k % 2 else "fur_dark")
            shag(rig, side_name("shoulder", s), side_name(f"shoulder_shag{k}", s), (mx(6.1, s), 17.2 - 0.3 * (k % 2), -5.8 + 1.1 * k), shoulder_len[k], 0.9, s,
                 "fur_dark" if k % 2 else "fur_brown")
        # 垂苔：前臂 / 小腿下半截的苔、肋侧菌柄下各挂细长的苔缕
        for stem, bone, pts in (("fmoss", side_name("forearm", s), ((6.2, 7.4, -3.9), (6.2, 6.6, -4.6))),
                                ("hmoss", side_name("shin", s), ((6.6, 7.2, HIND_Z + 6.8), (6.4, 6.4, HIND_Z + 8.4)))):
            for k, p in enumerate(pts):
                shag(rig, bone, side_name(f"{stem}{k}", s), (mx(p[0], s), p[1], p[2]), 2.0 + 0.5 * k, 0.5, s, "moss")


def part_tendrils(rig: Rig) -> None:
    """跨背的细根与菌盖旁的小菌：
    细根各从一侧肋下起，翻过脊背，落到另一侧（左右落点不同），避开已有的菌簇和主根；
    每朵肩隆 / 背脊的大菌盖旁再冒一两朵小菌。"""
    plans = (
        ("tend_a", "tendril_a", 4.2, [(-1, 12.6), (-0.55, 17.0), (0.0, 19.9), (0.5, 17.4), (1, 13.6)]),
        ("tend_b", "tendril_b", 8.9, [(1, 12.4), (0.6, 16.6), (-0.1, 18.7), (-0.6, 15.8), (-1, 10.8)]),
        ("tend_c", "tendril_c", 16.8, [(-1, 11.4), (-0.5, 15.0), (0.0, 17.2), (0.5, 15.2), (1, 11.0)]),
    )
    for name, bone, z, pts in plans:
        w = torso_profile(z)[0]
        pts3 = [(f * (w + 0.5) if abs(f) == 1 else f * w * 0.9, y, z + 0.35 * k) for k, (f, y) in enumerate(pts)]
        for k in range(len(pts3) - 1):
            r = 0.3 if k in (1, 2) else 0.34
            bar(rig, bone, f"{name}_{k}", pts3[k], pts3[k + 1], r, r, mat="root_pale" if k % 2 == 0 else "wood")
    satellites = (
        ("fungus_hump", (2.2, 22.6, -7.4, 0.8)), ("fungus_hump", (-3.8, 22.4, -4.6, 0.7)), ("fungus_back_a", (-3.6, 20.2, 0.6, 0.7)),
        ("fungus_back_a", (3.8, 20.0, 1.2, 0.6)), ("fungus_back_b", (3.6, 18.4, 8.2, 0.6)), ("fungus_back_c", (-3.4, 18.0, 13.4, 0.6)),
    )
    for k, (bone, (x, y, z, r)) in enumerate(satellites):
        shelf(rig, bone, f"sat_shelf{k}", (x, y, z), r, torso_profile(z)[1] - 0.3)


PARTS = {
    "head": ("1. 宽颅短吻 + 张口獠牙 + 圆耳 + 菌盖遮左眼", part_head),
    "neck": ("2. 短粗两节颈 + 颈鬃", part_neck),
    "torso": ("3. 24 片毛桶 + 毛板 + 毛刺", part_torso),
    "roots": ("4. 不对称树根 + 左肋腐洞", part_roots),
    "fungus": ("5. 托架菌簇：肩隆 / 背脊 / 肋侧", part_fungus),
    "branches": ("6. 肩上枯枝：每侧 4 根三级分杈", part_branches),
    "limbs": ("7. 四肢：粗肢 + 苔 + 五趾五爪", part_limbs),
    "tail": ("8. 两节短尾", part_tail),
    "fringe": ("9. 垂毛与垂苔：肋侧 / 大腿 / 肩外缘 / 肢下半截", part_fringe),
    "tendrils": ("10. 跨背细根与菌盖旁的小菌", part_tendrils),
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
# 软覆盖：毛层层相叠、毛包着肢体与根、苔长在毛上，都是造型意图；菌 / 枝 / 爪 / 洞与别件的互穿仍要报。
_LAYERS = ("fur_brown", "fur_dark", "moss")
SOFT_OVER = frozenset(frozenset((a, b)) for i, a in enumerate(_LAYERS) for b in _LAYERS[i + 1:])
SOFT_OVER |= frozenset(frozenset((f, o)) for f in _LAYERS for o in ("root_pale", "wood"))


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制，去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates(
    "腐根熊 / fu_gen_xiong",
    tuple(MATS),
    # 有意不对称：根（左右走向不同）、腐洞、遮眼菌盖、散生的菌簇、左肋少一朵菌
    asym=("root_flank_l", "root_flank_r", "root_rear_l", "root_rear_r", "fungus_flank_l", "fungus_flank_r", "fungus_rear_l", "fungus_rear_r",
          "eye_cap", "fungus_hump", "fungus_back_a", "fungus_back_b", "fungus_back_c", "tendril_a", "tendril_b", "tendril_c"),
    soft_over=SOFT_OVER,
)


def self_test() -> None:
    print("运行 gen_fu_gen_xiong.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    cubes = len(rig.elements)
    moving = moving_bones(rig)

    assert abs(lo[1]) < 0.12, f"贴地失败：最低点 y={lo[1]:.3f}，掌垫 / 爪尖应落在 0 附近"
    assert 22.0 <= hi[1] <= 34.0, f"含枯枝体高 {hi[1]:.1f}px 超出 22..34"
    length = hi[2] - lo[2]
    assert 40.0 <= length <= 64.0, f"吻尾纵长 {length:.1f}px 超出 40..64（躯干 16 → 24 片，Round 1 约 36）"
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
    "head": "头", "snout": "吻", "jaw": "下颌", "eye_cap": "遮眼菌盖", "fungus_hump": "肩隆菌簇",
    "fungus_back_a": "背菌簇 A", "fungus_back_b": "背菌簇 B", "fungus_back_c": "背菌簇 C", "spine_mid": "中脊", "root_rear": "后肋树根", "fungus_rear": "后肋菌", "tail_1": "尾 1", "tail_2": "尾 2",
    "ear": "耳", "shoulder": "肩毛块", "branch": "肩枯枝", "root_flank": "肋侧树根", "fungus_flank": "肋侧菌",
    "arm": "臂", "forearm": "前臂", "paw_f": "前掌", "toes_f": "前趾", "thigh": "大腿", "shin": "小腿",
    "hock": "跟", "paw_b": "后掌", "toes_b": "后趾",
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

    cols = [("参考图 / 爆炸图 EXPLODED REF（同一只熊）", ref),
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


# ---------------------------------------------------------------------------
# 像素贴图（画笔库 handheld_texture：bear_* / shelf_* 是本只补的）
# ---------------------------------------------------------------------------
TEXTURE_MATERIAL = {
    "fur_brown": "bear_fur", "fur_dark": "bear_fur_dark", "moss": "moss", "root_pale": "bear_root",
    "wood": "bear_stalk", "cap": "shelf_cap", "gill": "shelf_gill", "hole": "void",
}
TEX_OUT = CREATURE_DIR / "FuGenXiong_tex.bbmodel"


def textured_run() -> None:
    """平色 bbmodel（本脚本全量产出）→ 贴图版 FuGenXiong_tex.bbmodel（骨骼层级原样保留，动画靠它）+ check_tex.png。"""
    from creature_tex import texture_bbmodel, write_check_tex

    rig = build_full()
    focus = focus_of(rig)
    texture_bbmodel(BBMODEL_OUT, TEX_OUT, tuple(MATS), TEXTURE_MATERIAL)
    views = (("3/4 左前上", 135.0, 25.0, focus), ("正面 头吻", 180.0, 0.0, (focus[0], focus[1] * 0.8)), ("右肋面", -90.0, 0.0, focus))
    out = write_check_tex(BBMODEL_OUT, TEX_OUT, REVIEW_DIR / "check_tex.png", views, REVIEW_DIR / "exploded.png")
    print(f"✓ {TEX_OUT.name}；check_tex.png → {out}")


def main() -> None:
    ap = argparse.ArgumentParser(description="腐根熊 (fu_gen_xiong) 生成器 Round 3/3")
    ap.add_argument("--self-test", action="store_true", help="门禁差分自证")
    ap.add_argument("--list", action="store_true", help="列出编号部件")
    ap.add_argument("--part", choices=sorted(PARTS), help="只输出单个部件的 bbmodel")
    ap.add_argument("--no-render", action="store_true", help="只写 bbmodel，不出预览图")
    ap.add_argument("--tex", action="store_true", help="只出像素贴图版 FuGenXiong_tex.bbmodel 与 check_tex.png（需先跑过全量）")
    args = ap.parse_args()

    if args.tex:
        textured_run()
        return
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
        out = CREATURE_DIR / f"FuGenXiong_{args.part}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(f"FuGenXiong_{args.part}", namespace="bong"), ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"✓ {out}（{len(rig.elements)} cube）")
        return

    rig = build_full()
    doc = rig.bbmodel("FuGenXiong", namespace="bong")
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
