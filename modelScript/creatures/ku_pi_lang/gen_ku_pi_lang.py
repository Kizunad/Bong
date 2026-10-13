#!/usr/bin/env python3
"""枯皮狼（ku_pi_lang）四足生物模型程序化生成器 —— Round 3/3。

Round 2 的调度意见（四条）与本轮对应的做法：
  1. 腿是方柱     → 每条腿拆大腿 / 小腿 / 跗 / 掌，长骨用 shaft_box 定向到关节点上；
                   后腿做反关节（膝向前、跗向后折，跟骨后翘），每脚 4 趾 4 根爪尖。
  2. 肋骨是白条   → 8 对肋，每根 4 段沿 XY 平面从脊椎向外下弯再内收；肋间透空，
                   胸腔里衬暗色做底，肋骨才读得出来。
  3. 几乎没有残皮 → 肩鬃、背上断续的皮带、从腰窝和肋侧垂下的锯齿残片、后腿的残毛，
                   胸腔两侧大面积裸露。
  4. 头太细       → 加粗颅骨、长吻分三段收细、下颌绕铰链张开 26°、上下犬齿与颊齿、
                   颧弓外露、右耳尚存缺口、左耳撕成一片挂下来的皮。

每个编号部件都是一个 part_*()，可单独出预览（--part / 正式运行时输出到 parts/）。

用法：
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --self-test # 门禁差分自证
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --list
  python3 modelScript/creatures/ku_pi_lang/gen_ku_pi_lang.py --part head

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X。
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "ku_pi_lang"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/ku_pi_lang")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
PREV_BBMODEL = CREATURE_DIR / "KuPiLang_round2.bbmodel"
BBMODEL_OUT = CREATURE_DIR / "KuPiLang.bbmodel"
BBMODEL_COPY = MODELS_DIR / "KuPiLang.bbmodel"

ROUND_TAG = "Round 3/3"
PREV_TAG = "Round 2"

# 材质必须 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "pelt_dark": (62, 54, 48),     # 残存的深灰褐枯毛
    "pelt_mangy": (92, 81, 69),    # 风干脱落的杂毛
    "pelt_rust": (112, 56, 42),    # 偏红的残皮
    "flesh_scab": (92, 40, 38),    # 结痂的暗红肉、筋腱、牙龈
    "bone_white": (222, 216, 202), # 外露骨
    "bone_dark": (38, 35, 31),     # 胸腔暗衬、爪、鼻头、眼窝
    "fang_white": (244, 239, 228), # 犬齿
    "eye_amber": (219, 142, 40),   # 琥珀色狼眼
}

# ---------------------------------------------------------------------------
# 骨骼：(名称, pivot(左侧 x 为负), 父骨)。右侧由 mirrored() 生成。
# ---------------------------------------------------------------------------
CENTER_BONES = [
    ("root", (0.0, 0.0, 0.0), None),
    ("hips", (0.0, 12.6, 6.8), "root"),
    ("spine_lumbar", (0.0, 13.4, 2.6), "hips"),
    ("thorax", (0.0, 13.8, -2.2), "spine_lumbar"),
    ("neck_base", (0.0, 15.0, -5.4), "thorax"),
    ("neck_upper", (0.0, 15.3, -7.6), "neck_base"),
    ("head", (0.0, 15.2, -9.0), "neck_upper"),
    ("snout", (0.0, 14.6, -11.6), "head"),
    ("jaw", (0.0, 13.2, -10.2), "head"),
    ("tongue", (0.0, 12.6, -12.4), "jaw"),
    ("tail_1", (0.0, 12.2, 9.5), "hips"),
    ("tail_2", (0.0, 11.8, 10.7), "tail_1"),
    ("tail_3", (0.0, 11.0, 11.9), "tail_2"),
    ("tail_4", (0.0, 10.0, 13.1), "tail_3"),
    ("tail_5", (0.0, 8.8, 14.3), "tail_4"),
    ("tail_6", (0.0, 7.5, 15.5), "tail_5"),
    ("tail_7", (0.0, 6.1, 16.7), "tail_6"),
]

# 左侧骨骼（x 为正的值，生成时取负）
SIDE_BONES = [
    ("ear", (1.7, 16.4, -9.4), "head"),
    ("shoulder", (2.5, 14.8, -4.4), "thorax"),
    ("arm", (3.3, 11.0, -5.2), "shoulder"),
    ("forearm", (3.4, 7.4, -3.4), "arm"),
    ("paw_f", (3.1, 3.0, -4.6), "forearm"),
    ("toes_f", (3.1, 0.9, -5.5), "paw_f"),
    ("thigh", (3.1, 12.0, 6.8), "hips"),
    ("shin", (3.2, 7.6, 3.8), "thigh"),
    ("hock", (3.1, 4.0, 7.6), "shin"),
    ("paw_b", (3.1, 0.9, 6.7), "hock"),
    ("toes_b", (3.1, 0.9, 6.3), "paw_b"),
    ("pelt_flank", (2.8, 13.0, 3.0), "spine_lumbar"),
    ("pelt_shoulder", (3.4, 14.0, -3.0), "thorax"),
]

MIN_MOVING_BONES = 32


def define_bones(rig: Rig) -> None:
    for name, pivot, parent in CENTER_BONES:
        rig.bone(name, pivot, parent)
    for suffix, sx in (("_l", -1.0), ("_r", 1.0)):
        for base, (x, y, z), parent in SIDE_BONES:
            # 父骨若是左右成对的骨，名字带同一后缀；躯干骨不带
            par = parent + suffix if parent in {b for b, _, _ in SIDE_BONES} else parent
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
    """端点精确落在 a、b 的定向柱。

    up 给定时，局部 +Z（半厚 rz）指向 up；局部 x（半宽 rx）= 轴 × up。
    不给 up 则无滚转，只适合截面近似方形的骨。
    """
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


# ===========================================================================
# 1. 头
# ===========================================================================
JAW_HINGE = (0.0, 13.2, -10.2)
JAW_OPEN_DEG = 26.0


def jaw_pt(x: float, y: float, z: float) -> Vec:
    """把「闭口姿态」下的下颌点绕铰链（x 轴）转开 JAW_OPEN_DEG。"""
    a = math.radians(JAW_OPEN_DEG)
    dy, dz = y - JAW_HINGE[1], z - JAW_HINGE[2]
    return (x, JAW_HINGE[1] + dy * math.cos(a) + dz * math.sin(a),
            JAW_HINGE[2] - dy * math.sin(a) + dz * math.cos(a))


def jaw_dir(v: Vec) -> Vec:
    a = math.radians(JAW_OPEN_DEG)
    return (v[0], v[1] * math.cos(a) + v[2] * math.sin(a), -v[1] * math.sin(a) + v[2] * math.cos(a))


def part_head(rig: Rig) -> None:
    """长吻 + 张开下颌 + 露犬齿 + 颧骨外露 + 一只耳朵撕裂。"""
    # ── 颅骨：后颅、额、矢状嵴，左太阳穴蒙皮已脱，右太阳穴仍有皮 ──
    box(rig, "head", "cranium_back", (-2.2, 13.6, -10.0), (2.2, 16.4, -8.4), mat="pelt_dark")
    box(rig, "head", "cranium_front", (-1.9, 13.9, -12.0), (1.9, 16.2, -10.0), mat="bone_white")
    box(rig, "head", "crest_bone", (-0.4, 16.2, -11.6), (0.4, 16.9, -8.8), mat="bone_white")
    box(rig, "head", "occiput_scab", (-1.4, 14.2, -8.4), (1.4, 15.8, -8.0), mat="flesh_scab")

    for s in (-1.0, 1.0):
        # 颞部：上半残皮、下半结痂，皮缘参差
        box(rig, "head", side_name("crest_pelt_patch", s), (mx(0.4, s), 16.2, -10.2), (mx(1.9, s), 16.7, -8.6), mat="pelt_rust")
        box(rig, "head", side_name("temple_pelt", s), (mx(2.2, s), 15.0, -10.2), (mx(2.55, s), 16.0, -8.6), mat="pelt_mangy")
        box(rig, "head", side_name("temple_scab", s), (mx(2.2, s), 14.2, -10.0), (mx(2.55, s), 15.0, -8.8), mat="flesh_scab")
        # 眼窝凹陷 + 琥珀眼 + 眉骨 + 眶后突
        box(rig, "head", side_name("eye_socket", s), (mx(1.55, s), 14.7, -12.1), (mx(2.15, s), 15.7, -10.9), mat="bone_dark")
        box(rig, "head", side_name("eye_amber", s), (mx(1.7, s), 14.95, -12.25), (mx(2.1, s), 15.45, -11.55), mat="eye_amber")
        bar(rig, "head", side_name("brow_ridge", s), (mx(1.0, s), 15.9, -12.5), (mx(2.5, s), 15.9, -10.4), 0.35, 0.3,
            mat="bone_white", up=(0, 1, 0))
        box(rig, "head", side_name("postorbital", s), (mx(2.2, s), 15.2, -10.4), (mx(2.6, s), 16.1, -9.8), mat="bone_white")
        # 颧弓：向外凸出的一道骨弧，弧下透空露出暗色
        bar(rig, "head", side_name("zygoma_front", s), (mx(2.0, s), 14.0, -12.6), (mx(2.8, s), 14.3, -11.0), 0.3, 0.3,
            mat="bone_white")
        bar(rig, "head", side_name("zygoma_mid", s), (mx(2.8, s), 14.3, -11.0), (mx(2.9, s), 14.5, -9.6), 0.3, 0.3,
            mat="bone_white")
        bar(rig, "head", side_name("zygoma_back", s), (mx(2.9, s), 14.5, -9.6), (mx(2.5, s), 14.2, -8.8), 0.3, 0.3,
            mat="bone_white")
        box(rig, "head", side_name("cheek_gap", s), (mx(1.9, s), 13.6, -12.0), (mx(2.4, s), 14.0, -9.4), mat="bone_dark")

    # ── 长吻：三段逐渐收细，鼻梁蒙一层残皮，鼻头皮革黑 ──
    snout_pts = [(0.0, 14.9, -11.8), (0.0, 14.6, -14.0), (0.0, 14.3, -16.0), (0.0, 14.1, -17.4)]
    snout_hw = [1.5, 1.2, 0.95, 0.8]
    snout_hh = [1.0, 0.85, 0.72, 0.6]
    for i in range(3):
        bar(rig, "snout", f"muzzle_bone_{i}", snout_pts[i], snout_pts[i + 1], (snout_hw[i] + snout_hw[i + 1]) / 2,
            (snout_hh[i] + snout_hh[i + 1]) / 2, mat="bone_white", up=(0, 1, 0))
    bar(rig, "snout", "nose_bridge_pelt", (0.0, 15.65, -12.2), (0.0, 15.25, -14.6), 0.62, 0.22, mat="pelt_dark", up=(0, 1, 0))
    box(rig, "snout", "nose_bridge_scab", (-0.4, 15.1, -15.0), (0.4, 15.4, -16.4), mat="flesh_scab")
    box(rig, "snout", "nose_pad", (-0.8, 13.9, -18.1), (0.8, 14.9, -17.4), mat="bone_dark")
    box(rig, "snout", "nostril_l", (-0.6, 14.35, -18.3), (-0.15, 14.75, -18.05), mat="fang_white")
    box(rig, "snout", "nostril_r", (0.15, 14.35, -18.3), (0.6, 14.75, -18.05), mat="fang_white")
    # 唇缘缩起，牙龈外翻成一条暗红带
    for s in (-1.0, 1.0):
        bar(rig, "snout", side_name("gum_upper", s), (mx(1.45, s), 13.75, -12.0), (mx(0.85, s), 13.6, -17.0), 0.22, 0.3,
            mat="flesh_scab", up=(0, 1, 0))
        # 上犬齿与门齿
        bar(rig, "snout", side_name("fang_up", s), (mx(0.95, s), 13.65, -15.6), (mx(0.95, s), 11.7, -15.85), 0.3, 0.3,
            mat="fang_white", up=(1, 0, 0))
        for k, (zc, ln) in enumerate(((-12.6, 0.8), (-13.5, 1.0), (-14.4, 0.9))):
            bar(rig, "snout", side_name(f"premolar_up{k}", s), (mx(1.38 - 0.05 * k, s), 13.7, zc),
                (mx(1.38 - 0.05 * k, s), 13.7 - ln, zc - 0.1), 0.2, 0.3, mat="fang_white", up=(1, 0, 0))
        for k, xo in enumerate((0.3, 0.7)):
            bar(rig, "snout", side_name(f"incisor_up{k}", s), (mx(xo, s), 13.65, -17.05),
                (mx(xo, s), 12.7, -17.1), 0.17, 0.2, mat="fang_white", up=(1, 0, 0))

    # ── 下颌：绕铰链张开，下颌骨两支 + 下颌齿 + 悬垂的舌 ──
    box(rig, "jaw", "jaw_hinge_block", (-1.5, 12.7, -10.9), (1.5, 13.7, -9.7), mat="bone_dark")
    for s in (-1.0, 1.0):
        bar(rig, "jaw", side_name("mandible_back", s), jaw_pt(mx(1.3, s), 13.0, -10.4), jaw_pt(mx(1.15, s), 12.5, -13.2),
            0.28, 0.45, mat="bone_white", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("mandible_front", s), jaw_pt(mx(1.15, s), 12.5, -13.2), jaw_pt(mx(0.8, s), 12.3, -16.4),
            0.26, 0.4, mat="bone_white", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("coronoid", s), jaw_pt(mx(1.3, s), 13.0, -10.7), jaw_pt(mx(1.25, s), 14.3, -10.9),
            0.3, 0.2, mat="bone_white", up=(1, 0, 0))
        bar(rig, "jaw", side_name("jaw_gum", s), jaw_pt(mx(1.0, s), 13.25, -11.6), jaw_pt(mx(0.8, s), 13.0, -16.2),
            0.2, 0.3, mat="flesh_scab", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("fang_down", s), jaw_pt(mx(0.8, s), 13.1, -15.5), jaw_pt(mx(0.8, s), 14.6, -15.55),
            0.28, 0.28, mat="fang_white", up=(1, 0, 0))
        for k, zc in enumerate((-12.4, -13.3, -14.2)):
            bar(rig, "jaw", side_name(f"premolar_down{k}", s), jaw_pt(mx(1.05, s), 13.2, zc),
                jaw_pt(mx(1.05, s), 14.0 + 0.1 * k, zc - 0.1), 0.2, 0.28, mat="fang_white", up=(1, 0, 0))
        bar(rig, "jaw", side_name("incisor_down", s), jaw_pt(mx(0.35, s), 13.1, -16.5), jaw_pt(mx(0.35, s), 14.0, -16.55),
            0.17, 0.2, mat="fang_white", up=(1, 0, 0))
    box_a, box_b = jaw_pt(0.0, 12.4, -16.3), jaw_pt(0.0, 12.8, -15.6)
    bar(rig, "jaw", "chin_cap", box_a, box_b, 0.5, 0.35, mat="bone_white", up=(1, 0, 0))
    # 舌：软片垂在张开的下颌上，舌尖耷拉
    bar(rig, "tongue", "tongue_root", jaw_pt(0.0, 13.0, -12.2), jaw_pt(0.0, 13.0, -14.2), 0.45, 0.25,
        mat="flesh_scab", up=jaw_dir((0, 1, 0)))
    bar(rig, "tongue", "tongue_tip", jaw_pt(0.0, 13.0, -14.2), jaw_pt(0.0, 12.5, -15.4), 0.35, 0.2,
        mat="flesh_scab", up=jaw_dir((0, 1, 0)))

    # ── 耳：右耳尚存但有缺口，左耳撕裂，一片皮耷拉下来 ──
    # 耳面朝前外（法线 (±0.45, 0, -0.9)），从头吻方向看得到整片耳宽
    bar(rig, "ear_r", "ear_r_base", (1.5, 16.5, -9.6), (1.8, 17.9, -9.4), 1.3, 0.28, mat="pelt_dark", up=(0.45, 0.0, -0.9))
    bar(rig, "ear_r", "ear_r_mid", (1.8, 17.9, -9.4), (2.0, 19.3, -9.3), 1.0, 0.24, mat="pelt_mangy", up=(0.45, 0.0, -0.9))
    bar(rig, "ear_r", "ear_r_tip", (2.0, 19.3, -9.3), (2.1, 20.5, -9.2), 0.55, 0.2, mat="pelt_dark", up=(0.45, 0.0, -0.9))
    bar(rig, "ear_r", "ear_r_inner", (1.45, 16.7, -9.8), (1.75, 18.8, -9.65), 0.5, 0.13, mat="bone_dark", up=(0.45, 0.0, -0.9))
    box(rig, "ear_r", "ear_r_notch", (2.5, 18.3, -9.6), (2.95, 19.2, -8.8), mat="flesh_scab")

    bar(rig, "ear_l", "ear_l_stump", (-1.5, 16.5, -9.6), (-1.8, 17.6, -9.5), 1.3, 0.28, mat="pelt_dark", up=(-0.45, 0.0, -0.9))
    bar(rig, "ear_l", "ear_l_tear_edge", (-2.0, 17.5, -9.6), (-2.3, 18.2, -9.5), 0.7, 0.22, mat="flesh_scab", up=(-0.45, 0.0, -0.9))
    bar(rig, "ear_l", "ear_l_flap_a", (-2.4, 18.0, -9.3), (-3.3, 17.4, -9.0), 0.5, 0.2, mat="pelt_mangy", up=(0.0, 1.0, 0.0))
    bar(rig, "ear_l", "ear_l_flap_b", (-3.3, 17.4, -9.0), (-4.0, 16.4, -8.9), 0.34, 0.18, mat="pelt_dark", up=(0.2, 1.0, 0.0))

    # ── 颈：两节颈骨 + 喉部筋 + 颈鬃，颈骨一粒粒外露 ──
    for i, (z, y) in enumerate(((-5.3, 15.5), (-6.3, 15.55), (-7.3, 15.5), (-8.2, 15.4))):
        bone = "neck_base" if i < 2 else "neck_upper"
        box(rig, bone, f"cervical_{i}", (-0.55, y - 0.6, z - 0.45), (0.55, y + 0.5, z + 0.45), mat="bone_white")
        box(rig, bone, f"cervical_spur_{i}", (-0.25, y + 0.5, z - 0.25), (0.25, y + 1.0, z + 0.25), mat="bone_white")
    for s in (-1.0, 1.0):
        bar(rig, "neck_upper", side_name("throat_sinew", s), (mx(0.9, s), 14.5, -8.4), (mx(0.8, s), 13.2, -6.2), 0.22, 0.28,
            mat="flesh_scab", up=(1, 0, 0))
        bar(rig, "neck_base", side_name("neck_mane", s), (mx(1.5, s), 16.3, -7.8), (mx(1.9, s), 14.2, -5.4), 0.8, 0.4,
            mat="pelt_dark", up=(1, 0, 0))
        bar(rig, "neck_upper", side_name("neck_mane_tuft", s), (mx(1.6, s), 16.0, -8.4), (mx(1.8, s), 14.8, -7.6), 0.5, 0.3,
            mat="pelt_rust", up=(1, 0, 0))


# ===========================================================================
# 2. 脊椎
# ===========================================================================
SPINE_SEGMENTS = [
    # (z_min, z_max, y_body_min, y_body_max, 棘突高, 骨骼)
    (-5.2, -4.2, 14.8, 15.6, 1.4, "thorax"),
    (-4.0, -3.0, 15.0, 15.8, 1.8, "thorax"),
    (-2.8, -1.8, 14.9, 15.7, 2.0, "thorax"),
    (-1.6, -0.6, 14.6, 15.4, 1.6, "thorax"),
    (-0.4, 0.6, 14.2, 15.0, 1.4, "thorax"),
    (0.8, 1.8, 13.8, 14.6, 1.2, "spine_lumbar"),
    (2.0, 3.0, 13.5, 14.3, 1.1, "spine_lumbar"),
    (3.2, 4.2, 13.2, 14.0, 1.0, "spine_lumbar"),
    (4.4, 5.4, 13.0, 13.8, 1.0, "spine_lumbar"),
    (5.6, 6.6, 12.8, 13.6, 0.9, "spine_lumbar"),
    (6.8, 7.8, 12.6, 13.4, 0.8, "hips"),
    (8.0, 9.0, 12.3, 13.1, 0.7, "hips"),
]


def part_spine(rig: Rig) -> None:
    """12 节独立椎体：椎体 + 带尖的棘突 + 左右横突，节间留缝。"""
    for i, (z0, z1, y0, y1, sh, bone) in enumerate(SPINE_SEGMENTS):
        zc = (z0 + z1) / 2
        box(rig, bone, f"vertebra_body_{i}", (-0.6, y0, z0), (0.6, y1, z1), mat="bone_white")
        # 棘突两段：粗基 + 细尖，朝后微倾（头朝 -Z，棘突向 +Z 倾）
        bar(rig, bone, f"spinous_base_{i}", (0.0, y1, zc), (0.0, y1 + sh * 0.55, zc + 0.12), 0.32, 0.34, mat="bone_white",
            up=(1, 0, 0))
        bar(rig, bone, f"spinous_tip_{i}", (0.0, y1 + sh * 0.55, zc + 0.12), (0.0, y1 + sh, zc + 0.3), 0.2, 0.22,
            mat="fang_white", up=(1, 0, 0))
        for s in (-1.0, 1.0):
            bar(rig, bone, side_name(f"transverse_{i}", s), (mx(0.6, s), y0 + 0.45, zc), (mx(1.25, s), y0 + 0.35, zc), 0.2, 0.3,
                mat="bone_white", up=(0, 1, 0))
    # 尾根与荐骨相接处的一块骨盘（髂骨翼 / 坐骨）
    for s in (-1.0, 1.0):
        bar(rig, "hips", side_name("ilium_wing", s), (mx(0.9, s), 13.6, 6.0), (mx(2.7, s), 12.7, 7.2), 0.55, 0.22,
            mat="bone_white", up=(0, 1, 0))
        bar(rig, "hips", side_name("ischium", s), (mx(2.2, s), 12.4, 8.0), (mx(1.7, s), 11.5, 9.8), 0.38, 0.3,
            mat="bone_white", up=(1, 0, 0))
        box(rig, "hips", side_name("pelvis_scab", s), (mx(1.0, s), 11.6, 6.4), (mx(1.7, s), 12.4, 8.2), mat="flesh_scab")


# ===========================================================================
# 3. 肋骨架
# ===========================================================================
# (z 中心, 赤道半宽, 椎侧 y, 骨骼)
RIBS = [
    (-4.8, 2.6, 14.9, "thorax"),
    (-3.6, 3.0, 15.0, "thorax"),
    (-2.4, 3.25, 14.9, "thorax"),
    (-1.2, 3.3, 14.6, "thorax"),
    (0.0, 3.2, 14.2, "thorax"),
    (1.2, 3.0, 13.8, "spine_lumbar"),
    (2.4, 2.6, 13.5, "spine_lumbar"),
    (3.5, 2.1, 13.2, "spine_lumbar"),
]
RIB_DEPTH = 5.2  # 肋弓从椎侧到最低点的竖直长度


def rib_depth(i: int) -> float:
    return RIB_DEPTH - 0.3 * max(0, i - 4)


def rib_points(i: int, s: float) -> list[Vec]:
    """第 i 根肋中心线的 5 个折点：椎侧 → 肋背 → 赤道 → 下弯 → 肋端。残皮贴肋也靠这个。"""
    zc, w, yt, _ = RIBS[i]
    depth = rib_depth(i)
    return [
        (mx(0.65, s), yt, zc),
        (mx(w * 0.78, s), yt - 0.55, zc),
        (mx(w, s), yt - depth * 0.45, zc),
        (mx(w * 0.86, s), yt - depth * 0.78, zc),
        (mx(w * 0.5, s), yt - depth, zc),
    ]


def part_ribcage(rig: Rig) -> None:
    """每根肋 4 段：自椎侧向外上翘，过赤道后向下内收；肋间透空，胸腔里衬暗色。"""
    for i, (zc, w, yt, bone) in enumerate(RIBS):
        for s in (-1.0, 1.0):
            pts = rib_points(i, s)
            width = 0.3 - 0.02 * max(0, i - 3)
            for k in range(4):
                bar(rig, bone, side_name(f"rib{i}_{k}", s), pts[k], pts[k + 1], width, 0.36, mat="bone_white",
                    up=(0, 0, 1), extend=0.1)
    # 前五根肋底部接软骨小块（暗红），把肋弓和胸骨连起来；后三根是浮肋
    for s in (-1.0, 1.0):
        for k, zc in enumerate((-4.8, -3.6, -2.4, -1.2, 0.0)):
            box(rig, "thorax", side_name(f"costal_cartilage{k}", s), (mx(0.3, s), 9.4, zc - 0.3), (mx(1.5, s), 9.9, zc + 0.3),
                mat="flesh_scab")
    # 胸骨：龙骨一列小块 + 剑突
    for k in range(5):
        z = -4.8 + k * 1.2
        box(rig, "thorax", f"sternebra_{k}", (-0.4, 9.2, z - 0.45), (0.4, 10.0, z + 0.45), mat="bone_white")
    box(rig, "thorax", "xiphoid", (-0.3, 9.4, 0.3), (0.3, 9.9, 2.0), mat="bone_white")
    # 胸腔暗衬：肋间透出的只能是黑暗，肋才读得出来
    box(rig, "thorax", "cavity_lining", (-1.6, 10.4, -5.4), (1.6, 13.9, 0.9), mat="bone_dark")
    box(rig, "spine_lumbar", "cavity_lining_rear", (-1.5, 11.6, 0.9), (1.5, 13.4, 4.1), mat="bone_dark")
    box(rig, "thorax", "lung_remnant", (-1.0, 11.0, -3.8), (1.0, 12.2, -1.2), mat="flesh_scab")


# ===========================================================================
# 4. 残皮
# ===========================================================================
def skin_strip(rig: Rig, bone: str, name: str, pts: list[Vec], widths: list[float], s: float, mat: str,
               thickness: float = 0.17) -> None:
    """沿折线铺一条薄皮：每段是一根薄板，宽度（z 向）逐点给定，薄的一维朝外。

    折线在 XY 平面内（z 不变或微变）。每段的外法线取 (-dy, dx) 中朝 (s, +y) 一侧的那个，
    所以贴着肋弓铺的前半段和垂下的后半段都能把薄的一维转到「朝外」。
    """
    for k in range(len(pts) - 1):
        a, b = pts[k], pts[k + 1]
        dx, dy = b[0] - a[0], b[1] - a[1]
        n = (-dy, dx, 0.0)
        if n[0] * s + n[1] < 0:
            n = (dy, -dx, 0.0)
        bar(rig, bone, f"{name}_{k}", a, b, (widths[k] + widths[k + 1]) / 4, thickness, mat=mat, up=n)


def rag(rig: Rig, bone: str, name: str, top: Vec, length: float, width: float, s: float,
        rng: random.Random | None, mat: str, flare_deg: float = 12.0) -> None:
    """一片锯齿下垂的残皮：三段逐渐收窄，向外微翻；rng 给定时带抖动和第二齿，不给则完全规整（用于必须左右镜像的骨）。"""
    flare = math.tan(math.radians(flare_deg)) * s

    def jit(amount: float) -> float:
        return rng.uniform(-amount, amount) if rng else 0.0

    pts = [top]
    for t in (0.42, 0.78, 1.0):
        pts.append((top[0] + length * t * flare + jit(0.12), top[1] - length * t, top[2] + jit(0.2)))
    skin_strip(rig, bone, name, pts, [width, width * 0.8, width * 0.6, width * 0.35], s, mat)
    if rng and rng.random() < 0.6:
        # 第二个齿：从中段岔出去，长度不同，形成锯齿缺口
        a = lerp3(pts[1], pts[2], 0.3)
        dz = rng.choice((-1, 1)) * width * 0.55
        b = (a[0] + length * 0.2 * flare, a[1] - length * rng.uniform(0.35, 0.5), a[2] + dz)
        skin_strip(rig, bone, f"{name}_prong", [a, b], [width * 0.4, width * 0.15], s, mat, thickness=0.15)


def rib_outer_x(i: int, y: float) -> float:
    """第 i 根肋在高度 y 处的外缘 x（右侧、肋中心线）。肋弓之外的高度取端点。"""
    pts = rib_points(i, 1.0)
    if y >= pts[0][1]:
        return pts[0][0]
    for a, b in zip(pts, pts[1:]):
        if b[1] <= y <= a[1]:
            return a[0] + (b[0] - a[0]) * (a[1] - y) / (a[1] - b[1])
    return pts[-1][0]


def drape(rig: Rig, bone: str, name: str, i: int, s: float, hang: float, mat: str, rng: random.Random,
          width: float = 1.15) -> None:
    """一片贴着第 i 根肋外侧铺下来的残皮：从椎侧翻过肋背，沿肋外缘到赤道，再自由垂下 hang 个单位。

    贴肋的部分离肋中心线 0.45（肋半厚 0.3 + 皮半厚 0.17 − 少许咬合）；垂下的部分向外翻，
    末端 3 段逐渐收窄成锯齿，并随机岔出一个短齿。
    """
    zc, w, yt, _ = RIBS[i]
    y_eq = yt - 0.45 * rib_depth(i)
    off = 0.43
    path = [
        (mx(1.1, s), yt + 0.4, zc),
        (mx(rib_outer_x(i, yt - 0.55) + off, s), yt - 0.55, zc),
        (mx(w + off, s), y_eq, zc),
    ]
    flare = 0.28 * s
    for t in (0.45, 0.8, 1.0):
        path.append((path[2][0] + hang * t * flare + rng.uniform(-0.1, 0.1), y_eq - hang * t, zc + rng.uniform(-0.15, 0.15)))
    widths = [width, width, width * 0.95, width * 0.8, width * 0.6, width * 0.32]
    skin_strip(rig, bone, name, path, widths, s, mat)
    if rng.random() < 0.6:
        a = lerp3(path[3], path[4], 0.3)
        dz = rng.choice((-1, 1)) * width * 0.5
        b = (a[0] + hang * 0.1 * s, a[1] - hang * rng.uniform(0.3, 0.45), a[2] + dz)
        skin_strip(rig, bone, f"{name}_prong", [a, b], [width * 0.4, width * 0.15], s, mat, thickness=0.15)


def tuft(rig: Rig, bone: str, name: str, base: Vec, height: float, width: float, mat: str) -> None:
    """一根向后（+Z）倾的毛刺：两段，下粗上细，尖端朝后上方。"""
    mid = (base[0], base[1] + height * 0.55, base[2] + height * 0.12)
    tip = (base[0], base[1] + height, base[2] + height * 0.38)
    bar(rig, bone, f"{name}_a", base, mid, width * 0.5, width * 0.4, mat=mat, up=(1, 0, 0))
    bar(rig, bone, f"{name}_b", mid, tip, width * 0.3, width * 0.25, mat=mat, up=(1, 0, 0))


def part_pelt(rig: Rig) -> None:
    """残皮挂在肩、脊侧、腰窝和后腿上，肋侧大片裸露。左右两侧的撕裂各不相同。"""
    rng = random.Random(20261013)
    mats_cycle = ["pelt_dark", "pelt_rust", "pelt_mangy", "pelt_rust", "pelt_dark"]

    # 肩鬃：肩峰上残留的一丛粗毛，是全身毛最完整的一处。成排的毛刺向后倾，不做方块。
    box(rig, "thorax", "mane_base", (-1.7, 15.5, -4.9), (1.7, 16.1, -2.6), mat="pelt_dark")
    for k, (z, h) in enumerate(((-4.6, 1.5), (-3.9, 2.1), (-3.2, 2.4), (-2.5, 1.9))):
        for s in (-1.0, 1.0):
            for row, xo in enumerate((0.45, 1.25)):
                tuft(rig, "thorax", side_name(f"mane_tuft{k}_{row}", s), (mx(xo, s), 16.0, z), h - 0.5 * row, 0.7,
                     "pelt_rust" if (k + row) % 2 else "pelt_dark")
    for s in (-1.0, 1.0):
        flank, shoulder = side_name("pelt_flank", s), side_name("pelt_shoulder", s)
        # 披在肋上的皮：每片都是「椎侧 → 肋背 → 赤道」贴肋铺下，再自由垂下，末端锯齿。
        # 前三根肋（0、1 和 2 的大半）全裸；左右撕裂的位置不同，各露出两段肋。
        drape(rig, shoulder, side_name("drape_shoulder", s), 2, s, rng.uniform(1.2, 2.0), "pelt_dark", rng, width=0.8)
        drape_plan = ((4, 3.6), (5, 1.4), (7, 1.3)) if s < 0 else ((3, 2.4), (4, 4.4), (6, 2.0), (7, 1.2))
        for i, hang in drape_plan:
            drape(rig, flank, side_name(f"drape{i}", s), i, s, hang * rng.uniform(0.8, 1.2),
                  mats_cycle[(i + (1 if s > 0 else 0)) % 5], rng, width=rng.uniform(0.8, 1.35))
        # 后腿后侧挂的碎皮（像一条破裤腿）
        for k, z in enumerate((7.2, 8.2, 9.2)):
            rag(rig, flank, side_name(f"breech_rag{k}", s), (mx(3.95, s), 11.8, z), rng.uniform(2.2, 3.6), 0.9, s, rng,
                mats_cycle[(k + 3 + (1 if s > 0 else 0)) % 5], flare_deg=10.0)
        # 腹下：干瘪的肉膜 + 两片挂着的碎皮
        bar(rig, "spine_lumbar", side_name("belly_membrane", s), (mx(1.9, s), 11.8, 1.6), (mx(1.4, s), 10.9, 5.2), 0.2, 1.7,
            mat="flesh_scab", up=(0, 1, 0))
        for k, z in enumerate((4.4, 5.3)):
            rag(rig, flank, side_name(f"belly_rag{k}", s), (mx(1.5, s), 10.9, z), rng.uniform(1.2, 2.0), 0.9, s, rng,
                "flesh_scab" if k == 0 else "pelt_rust", flare_deg=6.0)
        # 臀上还连着的残毛：一层薄毡 + 向后倾的毛刺
        box(rig, "hips", side_name("rump_fur", s), (mx(1.0, s), 13.1, 6.6), (mx(2.5, s), 13.8, 9.0), mat="pelt_dark")
        box(rig, "hips", side_name("rump_fur_low", s), (mx(2.4, s), 11.4, 7.4), (mx(2.9, s), 13.1, 9.0), mat="pelt_mangy")
        for k, z in enumerate((6.9, 7.8, 8.7)):
            tuft(rig, "hips", side_name(f"rump_tuft{k}", s), (mx(1.7 - 0.2 * k, s), 13.8, z), 1.4 + 0.3 * (k % 2), 0.7,
                 "pelt_mangy" if k % 2 else "pelt_dark")
        # 颈鬃垂下的毛（左右镜像，所以不带随机）
        for k, z in enumerate((-6.7, -5.7)):
            rag(rig, "neck_base", side_name(f"neck_fringe{k}", s), (mx(2.05, s), 15.7, z), 2.6 + 0.6 * k, 0.95, s, None,
                "pelt_dark" if k == 0 else "pelt_rust", flare_deg=8.0)

    # 秃尾根的一撮毛
    box(rig, "tail_1", "tail_root_fur", (-0.85, 11.5, 9.4), (0.85, 12.9, 10.9), mat="pelt_mangy")


# ===========================================================================
# 5. 四肢
# ===========================================================================
def toe_group(rig: Rig, bone: str, s: float, tag: str, base_z: float, x0: float, forward: float) -> None:
    """一只脚 4 趾：中间两趾长、外侧两趾短；每趾一段趾骨 + 一枚弯向地面的黑爪。

    forward 取 -1（朝 -Z 前）。趾尖落在 y≈0.2，爪尖贴地。
    """
    offs = (-0.85, -0.3, 0.3, 0.85)
    lens = (0.95, 1.4, 1.4, 0.95)
    for k, (o, ln) in enumerate(zip(offs, lens)):
        xo = x0 + s * o
        a = (xo, 0.95, base_z)
        b = (xo + s * o * 0.12, 0.45, base_z + forward * ln)
        c = (xo + s * o * 0.2, 0.18, base_z + forward * (ln + 0.85))
        bar(rig, bone, side_name(f"toe_{tag}{k}", s), a, b, 0.27, 0.27, mat="bone_white")
        bar(rig, bone, side_name(f"claw_{tag}{k}", s), b, c, 0.17, 0.17, mat="bone_dark")
    box(rig, bone, side_name(f"pad_{tag}", s), (x0 - 0.6, 0.0, base_z + forward * 0.35 - 0.45), (x0 + 0.6, 0.45, base_z + forward * 0.35 + 0.45),
        mat="bone_dark")


def front_leg(rig: Rig, s: float) -> None:
    """肩胛 → 肱骨 → 桡尺骨 → 腕 / 掌 → 趾。"""
    sh, ar, fo = side_name("shoulder", s), side_name("arm", s), side_name("forearm", s)
    pf, tf = side_name("paw_f", s), side_name("toes_f", s)
    shoulder, elbow = (mx(3.3, s), 11.0, -5.2), (mx(3.4, s), 7.4, -3.4)
    # 肩胛：扁板贴在肋外侧（x 比肋赤道再外 ≥0.2），+ 肩胛冈 + 残皮
    bar(rig, sh, side_name("scapula", s), (mx(3.0, s), 15.4, -3.9), (mx(3.75, s), 11.0, -5.2), 0.75, 0.2, mat="bone_white",
        up=(s, 0, 0.1))
    bar(rig, sh, side_name("scapula_spine", s), (mx(3.15, s), 15.0, -4.3), (mx(4.0, s), 12.2, -4.9), 0.18, 0.28, mat="fang_white",
        up=(0, 0, 1))
    bar(rig, sh, side_name("scapula_pelt", s), (mx(3.25, s), 14.6, -4.6), (mx(3.95, s), 12.6, -5.0), 0.45, 0.16, mat="pelt_mangy",
        up=(s, 0, 0.1))
    # 肱骨：骨干 + 两条筋 + 肉块残余
    bar(rig, ar, side_name("humerus", s), shoulder, elbow, 0.5, 0.5, mat="bone_white")
    bar(rig, ar, side_name("humerus_sinew", s), (mx(3.75, s), 10.6, -5.0), (mx(3.8, s), 8.0, -3.8), 0.18, 0.2, mat="flesh_scab")
    bar(rig, ar, side_name("humerus_muscle", s), (mx(3.6, s), 11.0, -5.0), (mx(3.7, s), 9.2, -4.4), 0.45, 0.35, mat="pelt_rust")
    box(rig, ar, side_name("shoulder_knob", s), (mx(2.9, s), 10.4, -5.7), (mx(3.7, s), 11.5, -4.8), mat="bone_white")
    # 肘：鹰嘴后翘
    bar(rig, fo, side_name("olecranon", s), (mx(3.4, s), 7.6, -3.4), (mx(3.4, s), 8.0, -2.4), 0.4, 0.4, mat="bone_white")
    # 前臂：桡骨 + 尺骨并行，之间露缝
    bar(rig, fo, side_name("radius", s), (mx(3.6, s), 7.3, -3.5), (mx(3.3, s), 3.1, -4.7), 0.27, 0.27, mat="bone_white")
    bar(rig, fo, side_name("ulna", s), (mx(3.05, s), 7.3, -3.2), (mx(2.9, s), 3.1, -4.4), 0.27, 0.27, mat="bone_white")
    bar(rig, fo, side_name("forearm_sinew", s), (mx(3.3, s), 6.6, -3.2), (mx(3.1, s), 4.4, -4.2), 0.2, 0.2, mat="flesh_scab")
    bar(rig, fo, side_name("forearm_tuft", s), (mx(3.5, s), 7.0, -3.6), (mx(3.55, s), 5.4, -4.2), 0.3, 0.3, mat="pelt_mangy")
    # 腕 + 掌骨
    box(rig, pf, side_name("carpus", s), (mx(2.6, s), 2.5, -5.2), (mx(3.6, s), 3.5, -4.2), mat="bone_white")
    box(rig, pf, side_name("carpal_pad", s), (mx(2.8, s), 2.8, -3.9), (mx(3.4, s), 3.6, -3.5), mat="bone_dark")
    for k, o in enumerate((-0.45, 0.45)):
        bar(rig, pf, side_name(f"metacarpal{k}", s), (mx(3.1 + o, s), 2.7, -4.6), (mx(3.1 + o, s), 1.0, -5.5), 0.24, 0.24, mat="bone_white")
    toe_group(rig, tf, s, "f", -5.55, mx(3.1, s), -1.0)


def hind_leg(rig: Rig, s: float) -> None:
    """股骨 → 胫腓骨 → 跗（跟骨后折）→ 跖 → 趾。后腿 Z 字形反关节：膝向前、跗向后。"""
    th, sn, hk = side_name("thigh", s), side_name("shin", s), side_name("hock", s)
    pb, tb = side_name("paw_b", s), side_name("toes_b", s)
    hip, stifle, hock, mtp = (mx(3.1, s), 12.0, 6.8), (mx(3.2, s), 7.6, 3.8), (mx(3.1, s), 4.0, 7.6), (mx(3.1, s), 0.95, 6.7)
    # 股骨 + 大腿残肉（上半截带毛，下半截露骨）
    bar(rig, th, side_name("femur", s), hip, stifle, 0.55, 0.55, mat="bone_white")
    bar(rig, th, side_name("thigh_muscle", s), (mx(3.6, s), 12.2, 6.8), (mx(3.5, s), 9.6, 5.4), 1.1, 0.8, mat="pelt_rust", up=(s, 0, 0))
    bar(rig, th, side_name("thigh_fur_top", s), (mx(3.6, s), 13.0, 6.8), (mx(3.55, s), 11.6, 6.0), 0.95, 0.65, mat="pelt_dark", up=(s, 0, 0))
    bar(rig, th, side_name("thigh_sinew", s), (mx(3.9, s), 9.4, 5.4), (mx(3.85, s), 7.8, 4.2), 0.2, 0.22, mat="flesh_scab")
    box(rig, th, side_name("trochanter", s), (mx(3.55, s), 11.2, 6.2), (mx(3.9, s), 12.4, 7.3), mat="bone_white")
    # 膝：髌骨在前（-Z）
    box(rig, th, side_name("patella", s), (mx(2.7, s), 7.3, 2.9), (mx(3.7, s), 8.4, 3.7), mat="fang_white")
    # 胫骨 + 腓骨，向后下斜
    bar(rig, sn, side_name("tibia", s), stifle, hock, 0.45, 0.45, mat="bone_white")
    bar(rig, sn, side_name("fibula", s), (mx(2.75, s), 7.4, 4.3), (mx(2.7, s), 4.3, 7.4), 0.2, 0.2, mat="bone_white")
    bar(rig, sn, side_name("shin_tuft", s), (mx(3.55, s), 7.4, 4.6), (mx(3.6, s), 5.2, 6.4), 0.35, 0.25, mat="pelt_mangy")
    # 跗：跟骨向后上翘，跟腱从胫骨绷到跟骨尖
    bar(rig, hk, side_name("calcaneus", s), hock, (mx(3.1, s), 5.6, 9.1), 0.42, 0.42, mat="bone_white")
    bar(rig, hk, side_name("achilles", s), (mx(3.4, s), 7.0, 5.6), (mx(3.4, s), 5.6, 9.0), 0.17, 0.2, mat="flesh_scab")
    box(rig, hk, side_name("hock_joint", s), (mx(2.55, s), 3.4, 6.9), (mx(3.65, s), 4.6, 8.0), mat="bone_white")
    # 跖骨两根并行，近乎竖直
    for k, o in enumerate((-0.4, 0.4)):
        bar(rig, pb, side_name(f"metatarsal{k}", s), (mx(3.1 + o, s), 3.6, 7.3), (mx(3.1 + o, s), 1.05, 6.7), 0.24, 0.24, mat="bone_white")
    bar(rig, pb, side_name("metatarsal_tuft", s), (mx(3.6, s), 3.4, 7.3), (mx(3.6, s), 1.6, 6.9), 0.25, 0.3, mat="pelt_dark")
    toe_group(rig, tb, s, "b", 6.65, mx(3.1, s), -1.0)


def part_limbs(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        front_leg(rig, s)
        hind_leg(rig, s)


# ===========================================================================
# 6. 尾
# ===========================================================================
TAIL_NODES = [
    # (起点 y,z → 终点 y,z, 半径, 骨骼)
    ((12.2, 9.5), (11.8, 10.7), 0.8, "tail_1"),
    ((11.8, 10.7), (11.0, 11.9), 0.72, "tail_2"),
    ((11.0, 11.9), (10.0, 13.1), 0.64, "tail_3"),
    ((10.0, 13.1), (8.8, 14.3), 0.56, "tail_4"),
    ((8.8, 14.3), (7.5, 15.5), 0.48, "tail_5"),
    ((7.5, 15.5), (6.1, 16.7), 0.4, "tail_6"),
    ((6.1, 16.7), (4.6, 17.9), 0.32, "tail_7"),
]


def part_tail(rig: Rig) -> None:
    """7 节骨节尾：每节一根椎体、两侧横突、背刺；尾尖留一缕残毛。"""
    for i, ((y0, z0), (y1, z1), r, bone) in enumerate(TAIL_NODES):
        a, b = (0.0, y0, z0), (0.0, y1, z1)
        bar(rig, bone, f"tail_vertebra_{i}", lerp3(a, b, 0.08), lerp3(a, b, 0.92), r * 0.8, r * 0.8, mat="bone_white")
        mid = lerp3(a, b, 0.5)
        for s in (-1.0, 1.0):
            bar(rig, bone, side_name(f"tail_knob_{i}", s), (mx(r * 0.4, s), mid[1], mid[2]), (mx(r * 1.35, s), mid[1] - 0.05, mid[2]), 0.2, 0.26,
                mat="bone_white", up=(0, 1, 0))
        bar(rig, bone, f"tail_spur_{i}", (0.0, mid[1] + r * 0.6, mid[2]), (0.0, mid[1] + r * 1.4, mid[2] + 0.2), 0.16, 0.2,
            mat="fang_white", up=(1, 0, 0))
    bar(rig, "tail_7", "tail_tip", (0.0, 4.6, 17.9), (0.0, 3.6, 18.9), 0.22, 0.22, mat="bone_white")
    bar(rig, "tail_7", "tail_wisp_mid", (0.0, 4.4, 18.2), (0.0, 1.8, 19.6), 0.24, 0.24, mat="pelt_dark")
    for s in (-1.0, 1.0):
        bar(rig, "tail_7", side_name("tail_wisp", s), (mx(0.25, s), 4.4, 18.0), (mx(0.75, s), 2.4, 19.2), 0.2, 0.2, mat="pelt_mangy")


PARTS = {
    "head": ("1. 长吻狼头 + 张开下颌 + 颧弓 + 撕裂耳", part_head),
    "spine": ("2. 12 节脊椎与骨盆", part_spine),
    "ribcage": ("3. 8 对 4 段弧形透空肋骨架", part_ribcage),
    "pelt": ("4. 残皮：肩鬃 / 脊皮带 / 锯齿垂片", part_pelt),
    "limbs": ("5. 四肢：大腿-小腿-跗-掌 + 4 趾爪", part_limbs),
    "tail": ("6. 7 节骨节尾", part_tail),
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
# 对称门只认 _l/_r 成对；这些是有意不对称的撕裂件与皮片
ASYM = ("ear_l", "ear_r", "pelt_flank_l", "pelt_flank_r", "pelt_shoulder_l", "pelt_shoulder_r")

# 软覆盖：皮 / 肉 / 筋彼此叠压是造型意图（残皮堆叠）；皮与骨互穿仍然要报
_SOFT_MATS = ("pelt_dark", "pelt_mangy", "pelt_rust", "flesh_scab")
SOFT_OVER = frozenset(
    frozenset((a, b)) for i, a in enumerate(_SOFT_MATS) for b in _SOFT_MATS[i + 1:]
)


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制（枯皮狼纵长 40px），去掉越界门。"""

    def specs(self):
        out = []
        for key, label, fn, injector in super().specs():
            if key == "overflow":
                continue
            if key == "interpenetrating":
                fn = self._skip_drape_on_rib(fn)
            out.append((key, label, fn, injector))
        return tuple(out)

    @staticmethod
    def _skip_drape_on_rib(fn):
        """残皮（drape*）就是贴着肋弓铺的：平行斜板的轴对齐包围盒必然互相咬 0.7px，
        而真实间隙是 肋半厚 0.3 + 皮半厚 0.17 对 offset 0.43，刚好贴合。只放行这一对名字，
        其余皮×骨互穿照报。"""
        def wrapped(rig):
            return [v for v in fn(rig)
                    if not (v.startswith("rib") and " × drape" in v) and not (v.startswith("drape") and " × rib" in v)]
        return wrapped


GATES = CreatureGates(
    "枯皮狼 / ku_pi_lang",
    tuple(MATS),
    asym=ASYM,
    soft_over=SOFT_OVER,
)


def self_test() -> None:
    print("运行 gen_ku_pi_lang.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    cubes = len(rig.elements)
    moving = moving_bones(rig)

    assert abs(lo[1]) < 0.12, f"贴地失败：最低点 y={lo[1]:.3f}，爪尖/垫应落在 0 附近"
    assert 16.0 <= hi[1] <= 22.0, f"体高 {hi[1]:.1f}px 超出 16..22"
    length = hi[2] - lo[2]
    assert 35.0 <= length <= 46.0, f"吻尾纵长 {length:.1f}px 超出 35..46"
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert cubes >= 400, f"部件数 {cubes} < 400（Round 2 为 251）"
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
# 兽头朝 -Z，所以左 = -X。Round 2 的旧接触表把 yaw=0 标成「头吻」，是反的。
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


def bone_tree_lines(rig: Rig) -> list[str]:
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines: list[str] = []

    def walk(name: str, depth: int) -> None:
        cnt = len(rig.bones[name]["children"])
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{name} ({cnt})")
        for kid in children.get(name, []):
            walk(kid, depth + 1)

    walk("root", 0)
    return lines


def bone_tree_panel(rig: Rig, height: int) -> Image.Image:
    lines = bone_tree_lines(rig)
    font_h, font_s, font_t = load_font(16), load_font(13), load_font(12)
    step = 15
    panel = Image.new("RGB", (330, max(height, 80 + len(lines) * step)), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.text((12, 10), "骨骼层级树（括号内为挂载 cube 数）", fill=(180, 235, 200), font=font_h)
    d.text((12, 34), f"骨骼 {len(rig.bones)}（可动 {len(moving_bones(rig))}，要求 ≥ {MIN_MOVING_BONES}）",
           fill=(240, 210, 150), font=font_s)
    d.text((12, 54), f"cube {len(rig.elements)}（Round 2 为 251）", fill=(190, 210, 235), font=font_s)
    y = 76
    for ln in lines:
        d.text((12, y), ln, fill=(230, 230, 235), font=font_t)
        y += step
    return panel


def render_check(rig: Rig, now_path: Path) -> None:
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    font_h = load_font(18)
    # 同取景：PREV 与 NOW 用同一个 focus（以 NOW 的包围盒为准），才能看出差异而不是缩放
    focus = focus_of(rig)
    sheet_now = six_view_sheet(now_path, f"NOW {ROUND_TAG}｜{len(rig.bones)} 骨 / {len(rig.elements)} cube", focus)
    sheet_now.save(REVIEW_DIR / "render.png")
    sheet_now.save(REVIEW_DIR / "contact_sheet.png")
    tree = bone_tree_panel(rig, sheet_now.height)
    tree.save(REVIEW_DIR / "bones_tree.png")

    row_h = max(sheet_now.height, tree.height)
    ref = Image.open(EXPLODED_REF).convert("RGB") if EXPLODED_REF.exists() else Image.new("RGB", (400, row_h), (50, 50, 50))
    ref = ref.resize((int(ref.width * row_h / ref.height), row_h), Image.Resampling.LANCZOS)

    cols = [("爆炸参考图 EXPLODED REF", ref)]
    if PREV_BBMODEL.exists():
        prev = six_view_sheet(PREV_BBMODEL, f"PREV {PREV_TAG}（同取景）", focus)
        cols.append((f"PREV：{PREV_TAG} 同取景（251 cube / 35 骨）", prev))
    cols.append((f"NOW：{ROUND_TAG}（{len(rig.elements)} cube / {len(rig.bones)} 骨）", sheet_now))
    cols.append((f"骨骼树（{len(rig.bones)} 骨）", tree))

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
    ap = argparse.ArgumentParser(description="枯皮狼 (ku_pi_lang) 生成器 Round 3/3")
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
        out = CREATURE_DIR / f"KuPiLang_{args.part}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(f"KuPiLang_{args.part}", namespace="bong"), ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"✓ {out}（{len(rig.elements)} cube）")
        return

    rig = build_full()
    doc = rig.bbmodel("KuPiLang", namespace="bong")
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
