#!/usr/bin/env python3
"""冻髓鹿（dong_sui_lu）四足生物模型程序化生成器 —— Round 1/3（只做几何）。

依据：docs/worldview.md §十七 冬季「灵气因低温结晶，耐寒结晶物种盛放」。
外形（任务卡第四批 #10）：瘦长灰白鹿；鹿角是结晶灵气长成的冰晶枝；背脊和肩胛有结晶簇；
蹄冻裂；呼出白雾（几块浅色方块）。

做法对标枯皮狼 / 游商傀儡的密度：按部件拆 part_*()，每个部件单独出预览，最后拼接。
  1. 头      颅骨 + 三段收细的长吻 + 张口下颌 + 颊齿 + 大耳（三段）+ 鼻头 + 白雾
  2. 颈      两节颈椎 + 颈肌 + 霜鬃 + 喉下冰凌
  3. 鹿角    每侧：角座 → 主干四段 → 眉叉 → 冠叉，每根枝再分杈，枝尖是冰晶
  4. 躯干    14 节脊椎（棘突 + 横突）+ 8 对四段透空肋 + 胸骨 + 暗色胸腔衬 + 骨盆
  5. 残毛    背脊毛带、肋侧披毛（前三肋与后两肋有毛，中段肋裸露）、胸垂、腹毛、臀腿毛团
  6. 结晶簇  肩隆 / 背脊两处 / 每侧肩胛，每簇 5~8 根高低不一的晶柱
  7. 四肢    前：肩胛 - 肱 - 桡 - 掌 - 蹄；后：股 - 胫 - 跗 - 跖 - 蹄；蹄裂成两瓣，带悬蹄和冻裂缝
  8. 尾      4 节短尾 + 霜毛

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X。

用法：
  python3 modelScript/creatures/dong_sui_lu/gen_dong_sui_lu.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/dong_sui_lu/gen_dong_sui_lu.py --self-test # 门禁差分自证
  python3 modelScript/creatures/dong_sui_lu/gen_dong_sui_lu.py --list
  python3 modelScript/creatures/dong_sui_lu/gen_dong_sui_lu.py --part head
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "dong_sui_lu"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/dong_sui_lu")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "DongSuiLu.bbmodel"
BBMODEL_COPY = MODELS_DIR / "DongSuiLu.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质必须 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "fur_ash": (150, 144, 135),     # 灰白的枯毛，躯干主色
    "fur_frost": (206, 206, 200),   # 挂霜的毛尖
    "fur_dark": (92, 84, 77),       # 底毛、腹下、小腿下半
    "bone_grey": (186, 180, 168),   # 外露骨
    "hoof_dark": (58, 54, 53),      # 蹄、鼻头、眼窝、胸腔暗衬
    "ice_pale": (198, 224, 236),    # 浅色结晶
    "ice_deep": (116, 168, 206),    # 深色结晶、眼
    "mist_white": (238, 243, 246),  # 呼出的白雾
}

# ---------------------------------------------------------------------------
# 骨骼：(名称, pivot(左侧 x 为负), 父骨)。右侧由 define_bones 生成。
# ---------------------------------------------------------------------------
CENTER_BONES = [
    ("root", (0.0, 0.0, 0.0), None),
    ("hips", (0.0, 21.0, 7.0), "root"),
    ("spine_lumbar", (0.0, 21.8, 2.0), "hips"),
    ("thorax", (0.0, 22.5, -3.0), "spine_lumbar"),
    ("neck_base", (0.0, 23.0, -6.0), "thorax"),
    ("neck_upper", (0.0, 25.8, -9.0), "neck_base"),
    ("head", (0.0, 28.0, -10.8), "neck_upper"),
    ("snout", (0.0, 27.4, -13.2), "head"),
    ("jaw", (0.0, 26.4, -11.8), "head"),
    ("breath", (0.0, 25.8, -18.4), "head"),
    ("tail_1", (0.0, 21.0, 9.6), "hips"),
    ("tail_2", (0.0, 20.0, 11.0), "tail_1"),
    ("tail_3", (0.0, 18.8, 12.3), "tail_2"),
    ("tail_4", (0.0, 17.5, 13.5), "tail_3"),
    ("crystal_withers", (0.0, 23.6, -4.6), "thorax"),
    ("crystal_back_a", (0.0, 22.6, 0.0), "spine_lumbar"),
    ("crystal_back_b", (0.0, 22.0, 4.2), "spine_lumbar"),
]

SIDE_BONES = [
    ("ear", (1.5, 29.6, -10.4), "head"),
    ("antler_base", (1.3, 29.8, -11.2), "head"),
    ("antler_beam", (2.2, 33.0, -10.6), "antler_base"),
    ("antler_brow", (2.0, 31.6, -11.0), "antler_base"),
    ("antler_crown", (4.6, 40.0, -10.6), "antler_beam"),
    ("shoulder", (3.0, 19.0, -5.0), "thorax"),
    ("crystal_shoulder", (3.6, 19.5, -4.2), "shoulder"),
    ("arm", (3.1, 19.0, -5.0), "shoulder"),
    ("forearm", (3.2, 14.5, -4.0), "arm"),
    ("cannon_f", (3.0, 8.2, -4.7), "forearm"),
    ("hoof_f", (3.0, 2.6, -5.0), "cannon_f"),
    ("thigh", (3.0, 19.5, 7.0), "hips"),
    ("shin", (3.4, 13.6, 4.6), "thigh"),
    ("hock", (3.0, 8.0, 8.6), "shin"),
    ("cannon_b", (3.0, 6.0, 8.2), "hock"),
    ("hoof_b", (3.0, 2.6, 7.8), "cannon_b"),
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


def sub3(a: Vec, b: Vec) -> Vec:
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def scale3(a: Vec, k: float) -> Vec:
    return (a[0] * k, a[1] * k, a[2] * k)


def norm3(a: Vec) -> Vec:
    n = math.sqrt(a[0] ** 2 + a[1] ** 2 + a[2] ** 2) or 1.0
    return (a[0] / n, a[1] / n, a[2] / n)


def cross3(a: Vec, b: Vec) -> Vec:
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def mirror_pt(p: Vec, s: float) -> Vec:
    return (p[0] * s, p[1], p[2])


# ---------------------------------------------------------------------------
# 结晶：两段收细的晶柱，尖端再顶一粒小晶尖。全身所有冰晶都走这个函数。
# ---------------------------------------------------------------------------
def shard(rig: Rig, bone: str, name: str, base: Vec, tip: Vec, width: float, mat: str) -> None:
    """一根晶柱：base → 中点（宽 width）→ 近尖（宽 0.62 width）→ 晶尖（宽 0.3 width）。"""
    axis = sub3(tip, base)
    up_ref = (0.0, 1.0, 0.0) if abs(norm3(axis)[1]) < 0.9 else (1.0, 0.0, 0.0)
    length = math.sqrt(sum(c * c for c in axis))
    p1 = lerp3(base, tip, 0.55)
    bar(rig, bone, f"{name}_a", base, p1, width, width, mat=mat, up=up_ref)
    if length < 2.4:
        # 太短的晶柱没有地方再收一级尖，两段就够；多收一级会做出薄于 0.2px 的碎片
        bar(rig, bone, f"{name}_b", p1, tip, max(width * 0.55, 0.12), max(width * 0.55, 0.12), mat=mat, up=up_ref)
        return
    p2 = lerp3(base, tip, 0.88)
    bar(rig, bone, f"{name}_b", p1, p2, width * 0.62, width * 0.62, mat=mat, up=up_ref)
    bar(rig, bone, f"{name}_c", p2, tip, max(width * 0.3, 0.11), max(width * 0.3, 0.11), mat=mat, up=up_ref)


def crystal_cluster(rig: Rig, bone: str, name: str, base: Vec, plan: list[tuple[float, float, float, float, float]]) -> None:
    """plan 每项 (dx, dz, 高度, 宽, 倾斜量)；dx≠0 的项左右各长一根，倾斜量是尖端相对底端朝外的 x 偏移。"""
    for k, (dx, dz, h, w, lean) in enumerate(plan):
        mat = "ice_pale" if k % 2 == 0 else "ice_deep"
        if dx == 0.0:
            assert lean == 0.0, "中线上的晶柱不能倾斜，否则左右不镜像"
            shard(rig, bone, f"{name}{k}", (base[0], base[1], base[2] + dz), (base[0], base[1] + h, base[2] + dz), w, mat)
            continue
        # 不在中线的晶柱左右各一根，名字带 _l / _r，门禁按镜像对检查
        for sd in (-1.0, 1.0):
            b = (base[0] + dx * sd, base[1], base[2] + dz)
            tip = (b[0] + lean * sd, b[1] + h, b[2] + 0.15 * lean)
            shard(rig, bone, side_name(f"{name}{k}", sd), b, tip, w, mat)


def icicle(rig: Rig, bone: str, name: str, top: Vec, length: float, width: float, mat: str = "ice_pale") -> None:
    """垂挂的冰凌，从毛边 / 关节往下长。"""
    shard(rig, bone, name, top, (top[0], top[1] - length, top[2]), width, mat)


# ===========================================================================
# 1. 头
# ===========================================================================
JAW_HINGE = (0.0, 26.4, -11.8)
JAW_OPEN_DEG = 12.0


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
    """长脸鹿头：后颅 + 额 + 三段长吻 + 下颌 + 眼 + 大耳 + 白雾。"""
    # ── 颅骨：后颅毛覆、额骨裸露、眶上脊 ──
    box(rig, "head", "cranium_back", (-1.9, 27.0, -11.4), (1.9, 29.6, -9.6), mat="fur_ash")
    box(rig, "head", "cranium_frost", (-1.5, 29.6, -11.0), (1.5, 30.0, -9.8), mat="fur_frost")
    box(rig, "head", "forehead_bone", (-1.6, 27.4, -12.6), (1.6, 29.6, -11.4), mat="bone_grey")
    box(rig, "head", "poll_fur", (-1.2, 27.4, -9.6), (1.2, 29.2, -8.8), mat="fur_dark")
    for s in (-1.0, 1.0):
        # 眼窝 + 冰蓝眼 + 眶上脊 + 颧突
        box(rig, "head", side_name("eye_socket", s), (mx(1.5, s), 27.8, -12.7), (mx(2.15, s), 29.0, -11.5), mat="hoof_dark")
        box(rig, "head", side_name("eye_glint", s), (mx(1.65, s), 28.1, -12.85), (mx(2.1, s), 28.7, -12.2), mat="ice_deep")
        bar(rig, "head", side_name("brow_ridge", s), (mx(1.0, s), 29.2, -13.0), (mx(2.4, s), 29.2, -11.0), 0.3, 0.3,
            mat="bone_grey", up=(0, 1, 0))
        bar(rig, "head", side_name("zygoma", s), (mx(1.9, s), 27.2, -13.0), (mx(2.5, s), 27.4, -10.2), 0.28, 0.28,
            mat="bone_grey")
        box(rig, "head", side_name("temple_fur", s), (mx(1.9, s), 27.2, -11.2), (mx(2.4, s), 28.8, -9.7), mat="fur_ash")
        # 眼下泪腺沟：一道暗线
        box(rig, "head", side_name("tear_groove", s), (mx(1.75, s), 26.7, -13.4), (mx(2.05, s), 27.7, -12.9), mat="fur_dark")

    # ── 长吻：三段逐渐收窄压低，鼻梁是裸骨，两侧覆毛，鼻头皮革黑 ──
    pts = [(0.0, 27.6, -12.4), (0.0, 26.9, -14.6), (0.0, 26.2, -16.4), (0.0, 25.7, -17.6)]
    hw = [1.45, 1.2, 0.98, 0.85]
    hh = [1.2, 1.0, 0.85, 0.72]
    for i in range(3):
        bar(rig, "snout", f"muzzle_bone_{i}", pts[i], pts[i + 1], (hw[i] + hw[i + 1]) / 2, (hh[i] + hh[i + 1]) / 2,
            mat="bone_grey", up=(0, 1, 0))
    for s in (-1.0, 1.0):
        bar(rig, "snout", side_name("muzzle_fur", s), (mx(1.25, s), 27.2, -12.6), (mx(0.95, s), 25.9, -17.0), 0.4, 0.7,
            mat="fur_ash", up=(s, 0, 0))
        bar(rig, "snout", side_name("lip_frost", s), (mx(1.15, s), 26.0, -13.6), (mx(0.9, s), 25.3, -17.3), 0.22, 0.32,
            mat="fur_frost", up=(s, 0, 0))
    bar(rig, "snout", "nose_bridge_fur", (0.0, 28.0, -12.8), (0.0, 27.3, -14.9), 0.6, 0.22, mat="fur_dark", up=(0, 1, 0))
    box(rig, "snout", "nose_pad", (-0.95, 24.8, -18.5), (0.95, 26.4, -17.6), mat="hoof_dark")
    box(rig, "snout", "nostril_l", (-0.75, 25.35, -18.7), (-0.2, 25.95, -18.4), mat="bone_grey")
    box(rig, "snout", "nostril_r", (0.2, 25.35, -18.7), (0.75, 25.95, -18.4), mat="bone_grey")
    box(rig, "snout", "upper_lip", (-0.8, 24.7, -17.9), (0.8, 25.1, -16.6), mat="fur_dark")

    # ── 下颌：绕铰链张开 12°，下颌骨两支 + 颊齿 + 下唇毛 ──
    box(rig, "jaw", "jaw_hinge_block", (-1.4, 26.0, -11.9), (1.4, 26.9, -10.9), mat="hoof_dark")
    for s in (-1.0, 1.0):
        bar(rig, "jaw", side_name("mandible_back", s), jaw_pt(mx(1.25, s), 26.5, -11.4), jaw_pt(mx(1.1, s), 25.7, -13.4),
            0.28, 0.45, mat="bone_grey", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("mandible_front", s), jaw_pt(mx(1.1, s), 25.7, -13.4), jaw_pt(mx(0.8, s), 25.2, -16.6),
            0.26, 0.4, mat="bone_grey", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("coronoid", s), jaw_pt(mx(1.25, s), 26.5, -11.6), jaw_pt(mx(1.2, s), 27.6, -11.8),
            0.28, 0.2, mat="bone_grey", up=(1, 0, 0))
        for k, zc in enumerate((-12.9, -13.8, -14.7, -15.6)):
            bar(rig, "jaw", side_name(f"molar_down{k}", s), jaw_pt(mx(1.0, s), 26.0, zc), jaw_pt(mx(1.0, s), 26.8 + 0.05 * k, zc - 0.1),
                0.2, 0.28, mat="fur_frost", up=(1, 0, 0))
        bar(rig, "jaw", side_name("jaw_fur", s), jaw_pt(mx(1.35, s), 25.4, -12.8), jaw_pt(mx(1.0, s), 24.7, -16.2), 0.3, 0.5,
            mat="fur_ash", up=(s, 0, 0))
    bar(rig, "jaw", "chin_cap", jaw_pt(0.0, 25.0, -16.9), jaw_pt(0.0, 25.4, -16.2), 0.55, 0.35, mat="fur_dark", up=(1, 0, 0))
    bar(rig, "jaw", "chin_frost", jaw_pt(0.0, 24.7, -16.4), jaw_pt(0.0, 23.7, -16.3), 0.3, 0.3, mat="ice_pale", up=(1, 0, 0))

    # ── 耳：宽大的叶形，三段，里面暗色；耳缘挂霜 ──
    for s in (-1.0, 1.0):
        n = (s * 0.5, 0.0, -0.87)
        bar(rig, side_name("ear", s), side_name("ear_base", s), (mx(2.0, s), 29.4, -9.2), (mx(3.0, s), 30.3, -9.3), 1.1, 0.3,
            mat="fur_ash", up=n)
        bar(rig, side_name("ear", s), side_name("ear_mid", s), (mx(3.0, s), 30.3, -9.3), (mx(4.4, s), 31.5, -9.5), 1.0, 0.26,
            mat="fur_ash", up=n)
        bar(rig, side_name("ear", s), side_name("ear_tip", s), (mx(4.4, s), 31.5, -9.5), (mx(5.5, s), 32.5, -9.6), 0.55, 0.22,
            mat="fur_dark", up=n)
        bar(rig, side_name("ear", s), side_name("ear_inner", s), (mx(2.3, s), 29.6, -9.5), (mx(4.8, s), 31.8, -9.8), 0.48, 0.12,
            mat="hoof_dark", up=n)
        bar(rig, side_name("ear", s), side_name("ear_frost", s), (mx(3.6, s), 31.2, -9.0), (mx(5.3, s), 32.8, -9.2), 0.28, 0.2,
            mat="fur_frost", up=n)

    # ── 白雾：呼出的白气，几块大小不一的浅色方块飘向前方；各块互不相连 ──
    for k, (lo, hi) in enumerate((
        ((-0.7, 25.2, -20.2), (0.5, 26.4, -19.0)),
        ((-1.6, 25.5, -22.0), (-0.1, 26.8, -20.7)),
        ((-2.6, 25.8, -24.0), (-1.0, 27.2, -22.5)),
        ((-3.8, 26.1, -26.0), (-2.0, 27.8, -24.3)),
        ((-5.2, 26.4, -27.9), (-3.6, 28.0, -26.4)),
    )):
        box(rig, "breath", f"mist_{k}", lo, hi, mat="mist_white")


# ===========================================================================
# 2. 颈
# ===========================================================================
NECK_PATH = [(0.0, 22.8, -5.4), (0.0, 24.4, -7.4), (0.0, 26.0, -9.0), (0.0, 27.6, -10.4)]


def part_neck(rig: Rig) -> None:
    """两节颈：每节两枚颈椎、粗壮的颈肌、颈背的毛鬃、喉下的毛垂和冰凌。"""
    for i in range(3):
        a, b = NECK_PATH[i], NECK_PATH[i + 1]
        bone = "neck_base" if i < 2 else "neck_upper"
        r = 2.0 - 0.2 * i
        bar(rig, bone, f"neck_muscle_{i}", a, b, r, r - 0.1, mat="fur_ash", up=(1, 0, 0))
        bar(rig, bone, f"neck_core_{i}", lerp3(a, b, 0.1), lerp3(a, b, 0.9), 0.55, 0.55, mat="bone_grey")
    for i, (z, y) in enumerate(((-5.6, 23.0), (-6.4, 24.0), (-7.4, 25.0), (-8.2, 25.9), (-9.0, 26.8), (-9.9, 27.6))):
        bone = "neck_base" if i < 3 else "neck_upper"
        box(rig, bone, f"cervical_spur_{i}", (-0.35, y + 1.3, z - 0.35), (0.35, y + 2.0 - 0.1 * i, z + 0.35), mat="bone_grey")
    # 颈背鬃：沿颈背一排向后倾的霜毛刺
    for k, (z, y, h) in enumerate(((-5.8, 24.4, 1.7), (-6.8, 25.4, 1.9), (-7.8, 26.4, 1.8), (-8.7, 27.3, 1.6), (-9.5, 28.1, 1.3))):
        bone = "neck_base" if k < 3 else "neck_upper"
        for s in (-1.0, 1.0):
            tuft(rig, bone, side_name(f"mane_tuft{k}", s), (mx(0.6, s), y, z), h, 0.7, "fur_frost" if k % 2 else "fur_ash")
    # 颈侧垂下的毛片（三层），喉下冰凌
    for s in (-1.0, 1.0):
        rng = random.Random(7001)
        for k, (z, y, ln) in enumerate(((-6.0, 24.0, 3.8), (-7.0, 25.0, 3.4), (-8.0, 26.0, 2.8), (-8.8, 26.9, 2.0))):
            bone = "neck_base" if k < 3 else "neck_upper"
            rag(rig, bone, side_name(f"neck_fringe{k}", s), (mx(1.55 - 0.1 * k, s), y, z), ln, 0.95, s, rng,
                "fur_frost" if k % 2 == 0 else "fur_ash", flare_deg=9.0)
    for k, (z, y, ln) in enumerate(((-6.8, 22.4, 1.8), (-7.8, 23.6, 2.3), (-8.6, 24.6, 1.5))):
        bone = "neck_base" if k < 2 else "neck_upper"
        icicle(rig, bone, f"throat_ice{k}", (0.0, y, z), ln, 0.4, "ice_pale" if k % 2 == 0 else "ice_deep")
    # 喉下毛垂
    bar(rig, "neck_base", "dewlap", (0.0, 23.2, -6.4), (0.0, 22.0, -7.4), 0.55, 0.9, mat="fur_dark", up=(1, 0, 0))


# ===========================================================================
# 3. 鹿角 —— 结晶灵气长成的冰晶枝
# ===========================================================================
# 主干折线（右侧；左侧镜像）。顺序 = 角座 → 主干四段 → 冠尖。
BEAM = [(1.3, 29.8, -11.2), (2.2, 33.0, -10.6), (3.4, 36.4, -10.0), (4.6, 39.6, -10.5), (5.6, 42.2, -11.5)]
BEAM_R = [0.95, 0.85, 0.72, 0.58, 0.4]
# 每一根枝：(挂靠主干节点序号, 方向(dx,dy,dz), 长度, 起始半宽, 所属骨, 分杈[(取枝上位置 t, 方向, 长度)])
TINES = [
    # 眉叉：贴着角座前上方，向前上长
    (0, "antler_brow", (0.25, 0.55, -0.8), 5.2, 0.6, [(0.6, (0.3, 0.9, -0.2), 2.4)]),
    (1, "antler_brow", (0.4, 0.35, -0.85), 5.0, 0.55, [(0.55, (0.2, 1.0, -0.2), 2.6), (0.85, (0.5, 0.7, -0.4), 1.6)]),
    # 主干中部：向外后上的侧枝
    (2, "antler_beam", (0.9, 0.55, 0.15), 5.6, 0.55, [(0.5, (0.3, 1.0, 0.0), 3.0), (0.8, (0.7, 0.7, 0.2), 2.2)]),
    (2, "antler_beam", (0.15, 0.4, -0.9), 4.6, 0.5, [(0.6, (0.1, 1.0, -0.2), 2.4)]),
    # 冠叉：向外、向前、向后各一把
    (3, "antler_crown", (0.95, 0.35, 0.2), 5.0, 0.5, [(0.55, (0.3, 1.0, 0.0), 2.6), (0.85, (0.8, 0.5, 0.2), 1.7)]),
    (3, "antler_crown", (0.1, 0.5, 0.85), 4.6, 0.46, [(0.6, (0.1, 1.0, 0.1), 2.4)]),
    (3, "antler_crown", (0.2, 0.3, -0.92), 4.2, 0.44, [(0.6, (0.1, 1.0, -0.1), 2.0)]),
    (4, "antler_crown", (0.8, 0.8, 0.0), 3.6, 0.38, []),
]


def part_antlers(rig: Rig) -> None:
    """每侧一副：角座 + 4 段主干 + 8 根侧枝 + 12 根分杈，枝尖全是冰晶。"""
    for s in (-1.0, 1.0):
        base_b, beam_b, brow_b, crown_b = (side_name(n, s) for n in ("antler_base", "antler_beam", "antler_brow", "antler_crown"))
        pts = [mirror_pt(p, s) for p in BEAM]
        # 角座：一圈骨质的玫瑰盘 + 角柄
        box(rig, base_b, side_name("burr_ring", s), (mx(0.55, s), 29.4, -11.9), (mx(2.1, s), 30.4, -10.5), mat="bone_grey")
        box(rig, base_b, side_name("burr_frost", s), (mx(0.75, s), 30.4, -11.7), (mx(1.9, s), 30.7, -10.7), mat="fur_frost")
        # 主干四段：越往上越细，下段是骨质灰、上段转冰晶
        seg_bone = [base_b, beam_b, beam_b, crown_b]
        seg_mat = ["bone_grey", "bone_grey", "ice_pale", "ice_pale"]
        for i in range(4):
            r = (BEAM_R[i] + BEAM_R[i + 1]) / 2
            bar(rig, seg_bone[i], side_name(f"beam_{i}", s), pts[i], pts[i + 1], r, r, mat=seg_mat[i], up=(0, 0, 1))
        # 骨 → 晶的过渡：主干第二节外裹一层深色晶壳
        bar(rig, beam_b, side_name("beam_shell", s), lerp3(pts[1], pts[2], 0.4), lerp3(pts[1], pts[2], 0.95), 0.5, 0.95, mat="ice_deep",
            up=(s, 0, 0))
        bar(rig, crown_b, side_name("beam_core_top", s), lerp3(pts[3], pts[4], 0.05), lerp3(pts[3], pts[4], 0.95), 0.3, 0.3,
            mat="ice_deep", up=(0, 0, 1))
        shard(rig, crown_b, side_name("beam_tip", s), pts[4], (pts[4][0] + 0.2 * s, pts[4][1] + 2.4, pts[4][2] - 0.3), 0.34, "ice_pale")
        # 侧枝与分杈
        for ti, (node, bone_base, d, ln, w, subs) in enumerate(TINES):
            bone = side_name(bone_base, s)
            root = pts[node]
            dv = norm3((d[0] * s, d[1], d[2]))
            tip = add3(root, scale3(dv, ln))
            mid = lerp3(root, tip, 0.52)
            mat_a = "ice_pale" if node >= 2 else "bone_grey"
            bar(rig, bone, side_name(f"tine{ti}_a", s), root, mid, w, w, mat=mat_a, up=(0, 0, 1) if abs(dv[2]) < 0.8 else (0, 1, 0))
            shard(rig, bone, side_name(f"tine{ti}_b", s), mid, tip, w * 0.85, "ice_deep" if ti % 2 else "ice_pale")
            for si, (t, sd, sl) in enumerate(subs):
                sroot = lerp3(root, tip, t)
                sdv = norm3((sd[0] * s, sd[1], sd[2]))
                shard(rig, bone, side_name(f"tine{ti}_s{si}", s), sroot, add3(sroot, scale3(sdv, sl)), w * 0.55,
                      "ice_pale" if (ti + si) % 2 else "ice_deep")
        # 角面上的碎晶颗粒：贴着主干的小晶粒，让枝面不光滑
        for k in range(5):
            t = 0.2 + 0.17 * k
            seg = 1 + (k // 2)
            p = lerp3(pts[seg], pts[seg + 1], t % 1.0)
            off = 0.85 * BEAM_R[seg]
            box(rig, beam_b if seg < 3 else crown_b, side_name(f"grain{k}", s),
                (p[0] + s * off - 0.25, p[1] - 0.25, p[2] - 0.25 - off * 0.2), (p[0] + s * off + 0.25, p[1] + 0.35, p[2] + 0.25 - off * 0.2),
                mat="ice_pale")


# ===========================================================================
# 4. 躯干：脊椎 / 肋骨 / 骨盆
# ===========================================================================
SPINE_N = 14
SPINE_Z0 = -5.4


def spine_y(i: int) -> float:
    """脊椎椎体中心高度：肩隆最高，腰略凹，臀微抬。"""
    return 23.2 - 0.07 * i - 0.5 * math.sin(math.pi * min(i, 11) / 11.0) + (0.2 if i >= 11 else 0.0)


def spine_bone_for(z: float) -> str:
    if z < -0.6:
        return "thorax"
    if z < 4.6:
        return "spine_lumbar"
    return "hips"


def part_spine(rig: Rig) -> None:
    """14 节椎体：椎体 + 两段棘突 + 横突；节间留缝。尾根与骨盆接在末端。"""
    for i in range(SPINE_N):
        z0 = SPINE_Z0 + i * 1.0
        z1 = z0 + 0.82
        zc = (z0 + z1) / 2
        y = spine_y(i)
        bone = spine_bone_for(zc)
        box(rig, bone, f"vertebra_body_{i}", (-0.7, y - 0.5, z0), (0.7, y + 0.5, z1), mat="bone_grey")
        sh = 1.6 - 0.07 * i if i < 8 else 0.9
        bar(rig, bone, f"spinous_base_{i}", (0.0, y + 0.5, zc), (0.0, y + 0.5 + sh * 0.55, zc + 0.1), 0.32, 0.34, mat="bone_grey", up=(1, 0, 0))
        bar(rig, bone, f"spinous_tip_{i}", (0.0, y + 0.5 + sh * 0.55, zc + 0.1), (0.0, y + 0.5 + sh, zc + 0.25), 0.2, 0.22,
            mat="fur_frost", up=(1, 0, 0))
        for s in (-1.0, 1.0):
            bar(rig, bone, side_name(f"transverse_{i}", s), (mx(0.7, s), y - 0.2, zc), (mx(1.5, s), y - 0.3, zc), 0.2, 0.3,
                mat="bone_grey", up=(0, 1, 0))
    # 骨盆：髂骨翼 + 坐骨 + 荐部肉垫
    for s in (-1.0, 1.0):
        bar(rig, "hips", side_name("ilium_wing", s), (mx(1.0, s), 22.0, 5.4), (mx(3.0, s), 20.6, 7.2), 0.6, 0.22, mat="bone_grey", up=(0, 1, 0))
        bar(rig, "hips", side_name("ischium", s), (mx(2.4, s), 20.4, 8.4), (mx(1.8, s), 18.8, 10.2), 0.4, 0.3, mat="bone_grey", up=(1, 0, 0))
        box(rig, "hips", side_name("pelvis_pad", s), (mx(1.0, s), 19.0, 6.2), (mx(1.8, s), 20.4, 8.6), mat="fur_dark")


# (z 中心, 赤道半宽, 椎侧 y, 骨骼, 弓深)
RIBS = [
    (-5.0, 2.9, 23.2, "thorax", 8.4),
    (-4.0, 3.2, 23.3, "thorax", 9.2),
    (-3.0, 3.3, 23.0, "thorax", 9.2),
    (-2.0, 3.3, 22.7, "thorax", 8.8),
    (-1.0, 3.2, 22.4, "thorax", 8.2),
    (0.0, 3.1, 22.2, "spine_lumbar", 7.6),
    (1.0, 2.9, 22.0, "spine_lumbar", 7.0),
    (2.0, 2.6, 21.8, "spine_lumbar", 6.2),
]


def rib_points(i: int, s: float) -> list[Vec]:
    """第 i 根肋中心线的 5 个折点：椎侧 → 肋背 → 赤道 → 下弯 → 肋端。披毛贴肋也靠这个。"""
    zc, w, yt, _, depth = RIBS[i]
    return [
        (mx(0.7, s), yt - 0.2, zc),
        (mx(w * 0.78, s), yt - 0.75, zc),
        (mx(w, s), yt - depth * 0.45, zc),
        (mx(w * 0.86, s), yt - depth * 0.78, zc),
        (mx(w * 0.5, s), yt - depth, zc),
    ]


def part_ribcage(rig: Rig) -> None:
    """每根肋 4 段，肋间透空；暗色胸腔衬在里面；前五对接软骨与胸骨。"""
    for i, (zc, w, yt, bone, depth) in enumerate(RIBS):
        for s in (-1.0, 1.0):
            pts = rib_points(i, s)
            width = 0.3 - 0.015 * max(0, i - 3)
            for k in range(4):
                bar(rig, bone, side_name(f"rib{i}_{k}", s), pts[k], pts[k + 1], width, 0.34, mat="bone_grey", up=(0, 0, 1), extend=0.1)
    for s in (-1.0, 1.0):
        for k in range(5):
            zc, _, yt, _, depth = RIBS[k]
            bot = yt - depth
            box(rig, "thorax", side_name(f"costal_cartilage{k}", s), (mx(0.3, s), bot - 0.1, zc - 0.3), (mx(1.6, s), bot + 0.5, zc + 0.3),
                mat="fur_dark")
    for k in range(5):
        z = -5.0 + k * 1.0
        yb = RIBS[k][2] - RIBS[k][4]
        box(rig, "thorax", f"sternebra_{k}", (-0.45, yb - 0.5, z - 0.4), (0.45, yb + 0.3, z + 0.4), mat="bone_grey")
    box(rig, "thorax", "xiphoid", (-0.3, 13.9, -0.7), (0.3, 14.5, 1.0), mat="bone_grey")
    # 胸腔暗衬：肋间透出的只能是黑暗，肋才读得出来
    box(rig, "thorax", "cavity_lining", (-1.8, 16.4, -5.6), (1.8, 21.2, 0.9), mat="hoof_dark")
    box(rig, "spine_lumbar", "cavity_lining_rear", (-1.7, 17.2, 0.9), (1.7, 21.2, 2.6), mat="hoof_dark")
    box(rig, "thorax", "lung_remnant", (-1.1, 18.0, -4.2), (1.1, 20.2, -1.4), mat="fur_dark")


# ===========================================================================
# 5. 残毛：披毛 / 毛带 / 胸垂 / 臀腿毛团
# ===========================================================================
def skin_strip(rig: Rig, bone: str, name: str, pts: list[Vec], widths: list[float], s: float, mat: str,
               thickness: float = 0.2) -> None:
    """沿 XY 平面折线铺一条薄毛片；每段的外法线取朝 (s, +y) 一侧，薄的一维朝外。"""
    for k in range(len(pts) - 1):
        a, b = pts[k], pts[k + 1]
        dx, dy = b[0] - a[0], b[1] - a[1]
        n = (-dy, dx, 0.0)
        if n[0] * s + n[1] < 0:
            n = (dy, -dx, 0.0)
        bar(rig, bone, f"{name}_{k}", a, b, (widths[k] + widths[k + 1]) / 4, thickness, mat=mat, up=n)


def rag(rig: Rig, bone: str, name: str, top: Vec, length: float, width: float, s: float,
        rng: random.Random | None, mat: str, flare_deg: float = 12.0) -> None:
    """一片锯齿下垂的毛片：三段逐渐收窄，向外微翻；rng 给定时带抖动与第二齿。"""
    flare = math.tan(math.radians(flare_deg)) * s

    def jit(amount: float) -> float:
        return rng.uniform(-amount, amount) if rng else 0.0

    pts = [top]
    for t in (0.42, 0.78, 1.0):
        # x 方向的抖动要乘 s：左右两侧用同一串随机数，才能严格镜像
        pts.append((top[0] + length * t * flare + jit(0.12) * s, top[1] - length * t, top[2] + jit(0.2)))
    skin_strip(rig, bone, name, pts, [width, width * 0.8, width * 0.6, width * 0.35], s, mat)
    if rng and rng.random() < 0.6:
        a = lerp3(pts[1], pts[2], 0.3)
        dz = rng.choice((-1, 1)) * width * 0.55
        b = (a[0] + length * 0.2 * flare, a[1] - length * rng.uniform(0.35, 0.5), a[2] + dz)
        skin_strip(rig, bone, f"{name}_prong", [a, b], [width * 0.4, width * 0.15], s, mat, thickness=0.17)


def tuft(rig: Rig, bone: str, name: str, base: Vec, height: float, width: float, mat: str) -> None:
    """一根向后（+Z）倾的毛刺：两段，下粗上细。"""
    mid = (base[0], base[1] + height * 0.55, base[2] + height * 0.12)
    tip = (base[0], base[1] + height, base[2] + height * 0.38)
    bar(rig, bone, f"{name}_a", base, mid, width * 0.5, width * 0.4, mat=mat, up=(1, 0, 0))
    bar(rig, bone, f"{name}_b", mid, tip, width * 0.3, width * 0.25, mat=mat, up=(1, 0, 0))


def rib_outer_x(i: int, y: float) -> float:
    pts = rib_points(i, 1.0)
    if y >= pts[0][1]:
        return pts[0][0]
    for a, b in zip(pts, pts[1:]):
        if b[1] <= y <= a[1]:
            return a[0] + (b[0] - a[0]) * (a[1] - y) / (a[1] - b[1])
    return pts[-1][0]


def drape(rig: Rig, bone: str, name: str, i: int, s: float, hang: float, mat: str, rng: random.Random,
          width: float = 1.0) -> Vec:
    """一片贴着第 i 根肋外侧铺下来的披毛：椎侧 → 肋背 → 赤道，再自由垂下 hang 个单位，末端锯齿；返回披毛末端的点，供挂冰凌。"""
    zc, w, yt, _, depth = RIBS[i]
    y_eq = yt - 0.45 * depth
    off = 0.43
    path = [
        (mx(1.1, s), yt + 0.2, zc),
        (mx(rib_outer_x(i, yt - 0.75) + off, s), yt - 0.75, zc),
        (mx(w + off, s), y_eq, zc),
    ]
    flare = 0.26 * s
    for t in (0.45, 0.8, 1.0):
        path.append((path[2][0] + hang * t * flare + rng.uniform(-0.1, 0.1) * s, y_eq - hang * t, zc + rng.uniform(-0.15, 0.15)))
    widths = [width, width, width * 0.95, width * 0.8, width * 0.6, width * 0.32]
    skin_strip(rig, bone, name, path, widths, s, mat)
    return path[-1]


def part_pelt(rig: Rig) -> None:
    """披毛盖住前三肋与后两肋，中段肋裸露；背脊两侧一条毛带；胸垂、腹毛、臀腿毛团；毛边挂冰凌。"""
    mats_cycle = ["fur_ash", "fur_frost", "fur_ash", "fur_dark", "fur_frost"]
    for s in (-1.0, 1.0):
        rng = random.Random(3105)
        # 肋侧披毛：前三肋（0、1、2）与后两肋（6、7）有毛，中段 3~5 裸露
        plan = ((0, 3.0), (1, 4.2), (2, 3.4), (6, 2.4), (7, 1.8))
        for i, hang in plan:
            bone = RIBS[i][3]
            hem = drape(rig, bone, side_name(f"drape{i}", s), i, s, hang * rng.uniform(0.9, 1.1), mats_cycle[i % 5], rng, width=0.82)
            if i in (1, 7):
                # 毛边挂冰凌：冰凌顶端就是披毛末端，不另算位置（披毛的垂长带随机，另算会脱开）
                icicle(rig, bone, side_name(f"drape_ice{i}", s), (hem[0], hem[1] + 0.15, hem[2]), 1.6, 0.34, "ice_pale")
        # 背脊毛带：沿脊两侧，向外略翻，肩隆处最厚
        for k in range(6):
            z0 = SPINE_Z0 + k * 2.4
            y = spine_y(int(k * 2.4)) - 0.2
            bone = spine_bone_for(z0 + 1.0)
            bar(rig, bone, side_name(f"back_band{k}", s), (mx(0.85, s), y + 0.5, z0), (mx(2.0, s), y - 0.35, z0 + 2.2), 0.6, 0.35,
                mat="fur_frost" if k % 2 == 0 else "fur_ash", up=(s * 0.3, 1, 0))
        # 胸垂：前胸下方一大块毛，边缘锯齿
        bar(rig, "thorax", side_name("brisket_fur", s), (mx(1.5, s), 17.8, -6.4), (mx(2.2, s), 14.6, -5.0), 0.9, 1.2,
            mat="fur_ash", up=(s, 0, 0))
        for k, z in enumerate((-6.2, -5.2, -4.2)):
            rag(rig, "thorax", side_name(f"brisket_rag{k}", s), (mx(2.15, s), 15.0, z), 2.4 - 0.3 * k, 0.9, s, rng,
                "fur_frost" if k % 2 == 0 else "fur_dark", flare_deg=8.0)
        # 腹下：干瘪的毛膜 + 碎毛
        bar(rig, "spine_lumbar", side_name("belly_membrane", s), (mx(1.9, s), 16.6, 0.6), (mx(1.5, s), 15.6, 5.4), 0.25, 1.7,
            mat="fur_dark", up=(0, 1, 0))
        for k, z in enumerate((2.0, 3.4, 4.6)):
            rag(rig, "spine_lumbar", side_name(f"belly_rag{k}", s), (mx(1.7, s), 15.9, z), rng.uniform(1.4, 2.2), 0.85, s, rng,
                "fur_ash", flare_deg=6.0)
        # 肩腿毛团：肩上一大团披毛（盖住肩胛）、臀上一大团（盖住大腿根）
        bar(rig, side_name("shoulder", s), side_name("shoulder_fur", s), (mx(3.5, s), 21.4, -5.2), (mx(3.9, s), 17.4, -4.6), 1.8, 1.3,
            mat="fur_ash", up=(s, 0, 0))
        bar(rig, side_name("shoulder", s), side_name("shoulder_fur_frost", s), (mx(3.9, s), 21.0, -5.0), (mx(4.1, s), 18.2, -4.6), 0.9, 0.5,
            mat="fur_frost", up=(s, 0, 0))
        bar(rig, side_name("thigh", s), side_name("thigh_fur", s), (mx(3.7, s), 21.4, 7.0), (mx(3.9, s), 15.6, 5.6), 1.8, 1.4,
            mat="fur_ash", up=(s, 0, 0))
        bar(rig, side_name("thigh", s), side_name("thigh_fur_frost", s), (mx(4.3, s), 21.4, 6.6), (mx(4.5, s), 18.0, 5.8), 1.1, 0.4,
            mat="fur_frost", up=(s, 0, 0))
        # 臀部的毛堆：两层台阶 + 顶上一层霜毛，把腰到大腿根盖成一整块
        box(rig, "hips", side_name("rump_coat_top", s), (mx(0.9, s), 20.2, 3.4), (mx(3.5, s), 22.0, 9.6), mat="fur_ash")
        box(rig, "hips", side_name("rump_coat_low", s), (mx(0.9, s), 17.8, 4.8), (mx(3.8, s), 20.2, 9.8), mat="fur_ash")
        box(rig, "hips", side_name("rump_frost", s), (mx(0.9, s), 22.0, 4.2), (mx(3.1, s), 22.5, 9.0), mat="fur_frost")
        # 肋上披毛的「鞍」：盖住第 3~5 肋的上半，肋骨的下半仍然露着
        zc4 = RIBS[4][0]
        pts4 = rib_points(4, s)
        saddle = [(mx(1.0, s), pts4[0][1] + 0.5, zc4), (mx(abs(pts4[1][0]) + 0.45, s), pts4[1][1] + 0.1, zc4),
                  (mx(abs(pts4[2][0]) + 0.45, s), pts4[2][1] + 1.6, zc4)]
        skin_strip(rig, "thorax", side_name("saddle_coat", s), saddle, [3.4, 3.4, 3.0], s, "fur_ash", thickness=0.28)
        for k, z in enumerate((7.4, 8.4, 9.4)):
            rag(rig, side_name("thigh", s), side_name(f"breech_rag{k}", s), (mx(4.1, s), 18.8, z), rng.uniform(2.6, 3.6), 1.0, s, rng,
                "fur_frost" if k % 2 else "fur_ash", flare_deg=8.0)
        # 毛边冰凌
        for k, z in enumerate((-6.0, -4.6)):
            icicle(rig, "thorax", side_name(f"brisket_ice{k}", s), (mx(2.5, s), 14.6, z), 1.9 + 0.5 * k, 0.4, "ice_pale" if k else "ice_deep")


# ===========================================================================
# 6. 结晶簇：肩隆 / 背脊两处 / 每侧肩胛
# ===========================================================================
def part_crystals(rig: Rig) -> None:
    """每簇 5~8 根高低不一的晶柱：从骨上长出来，所以底座垫一块骨灰色的结晶根。"""
    # 肩隆：全身最大一簇，8 根
    box(rig, "crystal_withers", "withers_root", (-1.4, 23.2, -5.6), (1.4, 24.0, -3.4), mat="ice_deep")
    crystal_cluster(rig, "crystal_withers", "withers_shard", (0.0, 24.0, -4.5), [
        (0.0, 0.0, 7.4, 0.6, 0.0), (0.9, 0.5, 5.8, 0.5, 0.5), (-0.9, 0.4, 5.6, 0.5, -0.5),
        (0.4, -0.8, 4.6, 0.45, 0.2), (-0.5, -0.9, 4.2, 0.45, -0.3), (1.2, -0.3, 3.2, 0.38, 0.9),
        (-1.2, -0.4, 3.0, 0.38, -0.9), (0.0, 1.0, 3.6, 0.4, 0.0),
    ])
    # 背脊两处，各 6 根，越往后越矮
    for key, base_z, bone, h0 in (("a", 0.0, "crystal_back_a", 5.0), ("b", 4.2, "crystal_back_b", 3.8)):
        by = spine_y(int((base_z - SPINE_Z0) / 1.0)) + 1.5
        box(rig, bone, f"back_{key}_root", (-1.1, by - 0.4, base_z - 0.9), (1.1, by + 0.2, base_z + 0.9), mat="ice_deep")
        crystal_cluster(rig, bone, f"back_{key}_shard", (0.0, by + 0.2, base_z), [
            (0.0, 0.0, h0, 0.5, 0.0), (0.7, 0.5, h0 * 0.75, 0.42, 0.45), (-0.7, 0.3, h0 * 0.7, 0.42, -0.45),
            (0.3, -0.7, h0 * 0.55, 0.38, 0.2), (-0.4, -0.7, h0 * 0.5, 0.38, -0.2), (0.9, -0.2, h0 * 0.4, 0.32, 0.7),
        ])
    # 肩胛：每侧一簇，朝外上方斜长，6 根
    for s in (-1.0, 1.0):
        bone = side_name("crystal_shoulder", s)
        box(rig, bone, side_name("shoulder_root", s), (mx(3.9, s), 20.4, -5.4), (mx(5.0, s), 21.6, -3.6), mat="ice_deep")
        base = (mx(4.4, s), 21.0, -4.5)
        for k, (dz, h, w, lean) in enumerate(((0.0, 5.2, 0.5, 1.4), (0.8, 4.0, 0.42, 1.0), (-0.8, 4.2, 0.42, 1.0),
                                              (0.0, 3.0, 0.36, 2.0), (1.1, 2.4, 0.3, 1.6), (-1.1, 2.6, 0.3, 1.6))):
            b = (base[0], base[1] - 0.3 * abs(dz), base[2] + dz)
            shard(rig, bone, side_name(f"shoulder_shard{k}", s), b, (b[0] + lean * s, b[1] + h, b[2] + dz * 0.2), w,
                  "ice_pale" if k % 2 == 0 else "ice_deep")


# ===========================================================================
# 7. 四肢
# ===========================================================================
def hoof(rig: Rig, bone: str, s: float, tag: str, x: float, z0: float) -> None:
    """一只蹄裂成两瓣：每瓣 = 蹄壁（前倾斜柱）+ 蹄底垫；蹄壁上有暗色冻裂缝；脚踝后面两粒悬蹄；冠带挂霜。"""
    for k, off in enumerate((-0.62, 0.62)):
        c = x + s * off
        bar(rig, bone, side_name(f"hoof_wall_{tag}{k}", s), (c, 2.0, z0 + 0.35), (c, 0.6, z0 - 1.65), 0.5, 0.55, mat="hoof_dark", up=(0, 1, 0))
        box(rig, bone, side_name(f"hoof_sole_{tag}{k}", s), (c - 0.5, 0.0, z0 - 1.95), (c + 0.5, 0.5, z0 + 0.35), mat="fur_dark")
        # 冻裂缝：蹄壁正面一道斜裂，浅色骨从里面透出来
        bar(rig, bone, side_name(f"hoof_crack_{tag}{k}", s), (c, 1.9, z0 - 0.75), (c + s * (0.1 if k else -0.1), 0.7, z0 - 1.25), 0.1, 0.12,
            mat="bone_grey", up=(1, 0, 0))
    # 缺口：外侧瓣前缘崩掉一角，露出骨
    box(rig, bone, side_name(f"hoof_chip_{tag}", s), (mx(abs(x) + 0.8, s), 0.5, z0 - 1.95), (mx(abs(x) + 1.15, s), 1.0, z0 - 1.4), mat="bone_grey")
    # 悬蹄
    for k, off in enumerate((-0.6, 0.6)):
        box(rig, bone, side_name(f"dewclaw_{tag}{k}", s), (x + s * off - 0.25, 1.9, z0 + 0.5), (x + s * off + 0.25, 2.6, z0 + 1.0), mat="hoof_dark")
    # 冠带：一圈霜毛和下垂的小冰凌
    bar(rig, bone, side_name(f"coronet_fur_{tag}", s), (x - 0.7, 2.35, z0 + 0.1), (x + 0.7, 2.35, z0 + 0.1), 0.4, 0.8, mat="fur_frost", up=(0, 1, 0))
    for k, off in enumerate((-0.7, 0.7)):
        icicle(rig, bone, side_name(f"coronet_ice_{tag}{k}", s), (x + s * off * 1.2, 2.4, z0 - 0.4), 1.5, 0.28, "ice_pale" if k else "ice_deep")


def front_leg(rig: Rig, s: float) -> None:
    """肩胛 → 肱骨 → 桡骨 → 腕 / 掌骨 → 球节 + 蹄。"""
    sh, ar, fo = side_name("shoulder", s), side_name("arm", s), side_name("forearm", s)
    cn, hf = side_name("cannon_f", s), side_name("hoof_f", s)
    shoulder, elbow = (mx(3.0, s), 19.0, -5.0), (mx(3.2, s), 14.5, -4.0)
    knee, fetlock = (mx(3.0, s), 8.2, -4.7), (mx(3.0, s), 2.6, -5.0)
    # 肩胛：扁板贴肋外侧 + 肩胛冈
    bar(rig, sh, side_name("scapula", s), (mx(3.1, s), 23.0, -3.8), (mx(3.6, s), 18.8, -5.2), 0.9, 0.22, mat="bone_grey", up=(s, 0, 0.1))
    bar(rig, sh, side_name("scapula_spine", s), (mx(3.3, s), 22.4, -4.1), (mx(4.0, s), 19.8, -4.9), 0.18, 0.3, mat="fur_frost", up=(0, 0, 1))
    # 肱骨：骨干 + 肌肉残余；肘突后翘
    bar(rig, ar, side_name("humerus", s), shoulder, elbow, 0.85, 0.85, mat="bone_grey")
    bar(rig, ar, side_name("humerus_muscle", s), (mx(3.9, s), 18.6, -5.0), (mx(4.0, s), 15.4, -4.2), 0.8, 0.55, mat="fur_ash", up=(s, 0, 0))
    box(rig, ar, side_name("shoulder_knob", s), (mx(2.5, s), 18.4, -5.6), (mx(3.6, s), 19.6, -4.4), mat="bone_grey")
    bar(rig, fo, side_name("olecranon", s), (mx(3.2, s), 14.7, -3.9), (mx(3.2, s), 15.4, -2.6), 0.45, 0.45, mat="bone_grey")
    # 桡骨 + 尺骨并行，之间露缝；上半截裹毛袖
    bar(rig, fo, side_name("radius", s), (mx(3.5, s), 14.2, -4.1), (mx(3.1, s), 8.4, -4.7), 0.52, 0.52, mat="bone_grey")
    bar(rig, fo, side_name("ulna", s), (mx(2.7, s), 14.2, -3.8), (mx(2.7, s), 9.0, -4.3), 0.3, 0.3, mat="bone_grey")
    bar(rig, fo, side_name("forearm_fur", s), (mx(3.6, s), 14.2, -4.0), (mx(3.4, s), 11.0, -4.5), 0.75, 0.6, mat="fur_ash", up=(s, 0, 0))
    bar(rig, fo, side_name("forearm_tuft", s), (mx(3.7, s), 11.2, -4.3), (mx(3.5, s), 9.6, -4.6), 0.4, 0.4, mat="fur_dark")
    # 腕：腕骨块 + 副腕垫 + 腕关节的结晶
    box(rig, cn, side_name("carpus", s), (mx(2.4, s), 7.6, -5.4), (mx(3.6, s), 9.0, -4.0), mat="bone_grey")
    box(rig, cn, side_name("carpal_pad", s), (mx(2.6, s), 7.9, -3.7), (mx(3.4, s), 8.9, -3.3), mat="hoof_dark")
    shard(rig, cn, side_name("knee_ice", s), (mx(3.6, s), 7.7, -4.7), (mx(5.2, s), 9.0, -4.9), 0.3, "ice_pale")
    # 掌骨（炮骨）：细长，两侧的筋；下端接球节
    bar(rig, cn, side_name("metacarpal", s), knee, fetlock, 0.42, 0.42, mat="bone_grey")
    bar(rig, cn, side_name("metacarpal_tendon", s), (mx(3.0, s), 7.6, -3.9), (mx(3.0, s), 3.2, -4.3), 0.2, 0.2, mat="fur_dark")
    bar(rig, cn, side_name("metacarpal_frost", s), (mx(3.5, s), 7.4, -4.8), (mx(3.45, s), 4.6, -5.0), 0.24, 0.3, mat="fur_frost")
    box(rig, cn, side_name("fetlock_knob", s), (mx(2.5, s), 2.2, -5.5), (mx(3.5, s), 3.4, -4.5), mat="bone_grey")
    hoof(rig, hf, s, "f", mx(3.0, s), -5.4)


def hind_leg(rig: Rig, s: float) -> None:
    """股骨 → 胫骨 → 跗（跟骨后折）→ 跖骨 → 球节 + 蹄。后腿 Z 字：膝向前、跗向后。"""
    th, sn, hk = side_name("thigh", s), side_name("shin", s), side_name("hock", s)
    cb, hb = side_name("cannon_b", s), side_name("hoof_b", s)
    hip, stifle, hock, fetlock = (mx(3.0, s), 19.5, 7.0), (mx(3.4, s), 13.6, 4.6), (mx(3.0, s), 8.0, 8.6), (mx(3.0, s), 2.6, 7.8)
    bar(rig, th, side_name("femur", s), hip, stifle, 0.95, 0.95, mat="bone_grey")
    box(rig, th, side_name("trochanter", s), (mx(3.5, s), 19.0, 6.3), (mx(4.0, s), 20.4, 7.6), mat="bone_grey")
    box(rig, th, side_name("patella", s), (mx(2.8, s), 13.2, 3.6), (mx(3.9, s), 14.4, 4.4), mat="fur_frost")
    # 胫骨 + 腓骨，向后下斜；上半裹毛
    bar(rig, sn, side_name("tibia", s), stifle, hock, 0.68, 0.68, mat="bone_grey")
    bar(rig, sn, side_name("fibula", s), (mx(2.5, s), 13.4, 5.2), (mx(2.4, s), 8.6, 8.4), 0.25, 0.25, mat="bone_grey")
    bar(rig, sn, side_name("shin_fur", s), (mx(3.8, s), 13.6, 4.8), (mx(3.6, s), 10.0, 7.4), 0.85, 0.55, mat="fur_ash", up=(s, 0, 0))
    bar(rig, sn, side_name("shin_tuft", s), (mx(3.9, s), 10.2, 7.2), (mx(3.7, s), 8.6, 8.4), 0.4, 0.4, mat="fur_dark")
    # 跗：跟骨向后上翘，跟腱从胫骨绷到跟骨尖，跗关节外挂冰
    bar(rig, hk, side_name("calcaneus", s), hock, (mx(3.0, s), 10.0, 10.0), 0.6, 0.6, mat="bone_grey")
    bar(rig, hk, side_name("achilles", s), (mx(3.4, s), 11.6, 6.6), (mx(3.4, s), 10.0, 9.9), 0.18, 0.2, mat="fur_dark")
    box(rig, hk, side_name("hock_joint", s), (mx(2.4, s), 6.8, 7.8), (mx(3.7, s), 8.8, 9.2), mat="bone_grey")
    shard(rig, hk, side_name("hock_ice", s), (mx(3.7, s), 6.9, 8.8), (mx(5.6, s), 8.1, 9.3), 0.32, "ice_deep")
    # 跖骨：细长，近乎竖直
    bar(rig, cb, side_name("metatarsal", s), (mx(3.0, s), 7.2, 8.4), fetlock, 0.42, 0.42, mat="bone_grey")
    bar(rig, cb, side_name("metatarsal_tendon", s), (mx(3.0, s), 7.0, 9.1), (mx(3.0, s), 3.2, 8.4), 0.2, 0.2, mat="fur_dark")
    bar(rig, cb, side_name("metatarsal_frost", s), (mx(3.5, s), 7.0, 8.4), (mx(3.45, s), 4.4, 8.1), 0.24, 0.3, mat="fur_frost")
    box(rig, cb, side_name("fetlock_knob", s), (mx(2.5, s), 2.2, 7.3), (mx(3.5, s), 3.4, 8.3), mat="bone_grey")
    hoof(rig, hb, s, "b", mx(3.0, s), 7.6)


def part_limbs(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        front_leg(rig, s)
        hind_leg(rig, s)


# ===========================================================================
# 8. 尾
# ===========================================================================
TAIL_NODES = [
    ((21.0, 9.6), (20.0, 11.0), 0.8, "tail_1"),
    ((20.0, 11.0), (18.8, 12.3), 0.72, "tail_2"),
    ((18.8, 12.3), (17.5, 13.5), 0.62, "tail_3"),
    ((17.5, 13.5), (16.0, 14.4), 0.5, "tail_4"),
]


def part_tail(rig: Rig) -> None:
    """4 节短尾：每节一根尾椎 + 毛套，尾尖一撮霜毛。"""
    for i, ((y0, z0), (y1, z1), r, bone) in enumerate(TAIL_NODES):
        a, b = (0.0, y0, z0), (0.0, y1, z1)
        bar(rig, bone, f"tail_vertebra_{i}", lerp3(a, b, 0.06), lerp3(a, b, 0.94), r * 0.75, r * 0.75, mat="bone_grey")
        bar(rig, bone, f"tail_fur_{i}", lerp3(a, b, 0.0), lerp3(a, b, 1.0), r * 1.2, r * 1.0, mat="fur_ash" if i % 2 == 0 else "fur_frost",
            up=(1, 0, 0))
    mid = (0.0, 15.2, 14.4)
    bar(rig, "tail_4", "tail_tuft_a", (0.0, 16.0, 14.4), (0.0, 13.8, 15.2), 0.7, 0.55, mat="fur_frost", up=(1, 0, 0))
    bar(rig, "tail_4", "tail_tuft_b", (0.0, 14.2, 15.1), (0.0, 12.4, 15.7), 0.4, 0.35, mat="fur_ash", up=(1, 0, 0))
    icicle(rig, "tail_4", "tail_ice", (0.0, 12.6, 15.7), 1.8, 0.3, "ice_pale")
    del mid


PARTS = {
    "head": ("1. 长脸鹿头 + 张口下颌 + 大耳 + 白雾", part_head),
    "neck": ("2. 两节颈 + 霜鬃 + 喉下冰凌", part_neck),
    "antlers": ("3. 结晶鹿角：角座 / 主干 / 眉叉 / 冠叉 / 分杈", part_antlers),
    "spine": ("4a. 14 节脊椎与骨盆", part_spine),
    "ribcage": ("4b. 8 对 4 段透空肋骨架", part_ribcage),
    "pelt": ("5. 残毛：披毛 / 毛带 / 胸垂 / 臀腿毛团", part_pelt),
    "crystals": ("6. 结晶簇：肩隆 / 背脊 / 肩胛", part_crystals),
    "limbs": ("7. 四肢：四段腿 + 裂蹄 + 悬蹄", part_limbs),
    "tail": ("8. 4 节短尾", part_tail),
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
# 白雾是一团彼此分离的气，有意悬空
MIST = frozenset(f"mist_{k}" for k in range(5))

# 软覆盖：毛 / 冰在毛边相叠是造型意图；骨与毛、冰与骨的互穿仍然要报
_SOFT_MATS = ("fur_ash", "fur_frost", "fur_dark")
SOFT_OVER = frozenset(frozenset((a, b)) for i, a in enumerate(_SOFT_MATS) for b in _SOFT_MATS[i + 1:])
SOFT_OVER |= frozenset({frozenset(("ice_pale", "ice_deep"))})
# 毛是包在骨 / 冰 / 蹄上的：毛袖、毛垂、颈肌裹着骨头本来就会盖住它。骨×骨、冰×骨仍要报。
SOFT_OVER |= frozenset(frozenset((f, o)) for f in _SOFT_MATS for o in ("bone_grey", "hoof_dark", "ice_pale", "ice_deep"))


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制（冻髓鹿带角高近 3 格），去掉越界门。"""

    def specs(self):
        out = []
        for key, label, fn, injector in super().specs():
            if key == "overflow":
                continue
            out.append((key, label, fn, injector))
        return tuple(out)


GATES = CreatureGates(
    "冻髓鹿 / dong_sui_lu",
    tuple(MATS),
    asym=("breath",),
    free_floating=MIST,
    soft_over=SOFT_OVER,
)


def self_test() -> None:
    print("运行 gen_dong_sui_lu.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    cubes = len(rig.elements)
    moving = moving_bones(rig)

    assert abs(lo[1]) < 0.12, f"贴地失败：最低点 y={lo[1]:.3f}，蹄底应落在 0 附近"
    assert 36.0 <= hi[1] <= 48.0, f"带角体高 {hi[1]:.1f}px 超出 36..48"
    length = hi[2] - lo[2]
    assert 34.0 <= length <= 52.0, f"纵长 {length:.1f}px 超出 34..52"
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


def six_view_sheet(path: Path, header: str, focus, cell: int = 460) -> Image.Image:
    font_h, font_l = load_font(18), load_font(13)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 56), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), header, fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(SIX_VIEWS):
        px, py = 5 + (i % 3) * (cell + 5), 34 + (i // 3) * (cell + 10)
        im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)
        sheet.paste(im, (px, py))
        d.rectangle([px, py, px + 300, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


# 骨骼树的中文标签：每根骨一个人话名字，读树的人不必猜 neck_upper 是什么
BONE_LABELS = {
    "root": "根", "hips": "臀", "spine_lumbar": "腰脊", "thorax": "胸脊", "neck_base": "颈根", "neck_upper": "颈上",
    "head": "头", "snout": "吻", "jaw": "下颌", "breath": "白雾",
    "tail_1": "尾 1", "tail_2": "尾 2", "tail_3": "尾 3", "tail_4": "尾 4",
    "crystal_withers": "肩隆晶簇", "crystal_back_a": "背晶簇 A", "crystal_back_b": "背晶簇 B",
    "ear": "耳", "antler_base": "角座", "antler_beam": "角主干", "antler_brow": "眉叉", "antler_crown": "冠叉",
    "shoulder": "肩胛", "crystal_shoulder": "肩胛晶簇", "arm": "肱", "forearm": "桡", "cannon_f": "前掌",
    "hoof_f": "前蹄", "thigh": "股", "shin": "胫", "hock": "跗", "cannon_b": "后跖", "hoof_b": "后蹄",
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
    sheet_now = six_view_sheet(now_path, f"NOW {ROUND_TAG}｜{len(rig.bones)} 骨 / {len(rig.elements)} cube", focus)
    sheet_now.save(REVIEW_DIR / "render.png")
    sheet_now.save(REVIEW_DIR / "contact_sheet.png")
    tree = bone_tree_panel(rig, sheet_now.height)
    tree.save(REVIEW_DIR / "bones_tree.png")

    row_h = max(sheet_now.height, tree.height)
    ref = Image.open(EXPLODED_REF).convert("RGB") if EXPLODED_REF.exists() else Image.new("RGB", (400, row_h), (50, 50, 50))
    ref = ref.resize((int(ref.width * row_h / ref.height), row_h), Image.Resampling.LANCZOS)

    cols = [("参考图 / 爆炸图 EXPLODED REF（同一只鹿）", ref),
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
    ap = argparse.ArgumentParser(description="冻髓鹿 (dong_sui_lu) 生成器 Round 1/3")
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
        out = CREATURE_DIR / f"DongSuiLu_{args.part}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(f"DongSuiLu_{args.part}", namespace="bong"), ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"✓ {out}（{len(rig.elements)} cube）")
        return

    rig = build_full()
    doc = rig.bbmodel("DongSuiLu", namespace="bong")
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
