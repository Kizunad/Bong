#!/usr/bin/env python3
"""剥皮豺（bo_pi_chai）四足生物模型程序化生成器 —— Round 1/3（只做几何）。

依据：docs/worldview.md §十三 血谷「高级野兽」。任务卡第四批 #13：血谷的群猎豺，皮毛暗红、
背上一道裸露的肌肉条、长獠牙、尾巴分叉；比枯皮狼更矫健、更凶。

和枯皮狼（透空肋骨架的瘦狼）的区别在体量：剥皮豺是「肉还在」的猎手，躯干是一圈圈带毛的
肌肉桶，只在几处被撕掉皮毛，露出下面的生肌和肋骨。
  1. 头      粗颅 + 短而厚的吻 + 张口 28° 的下颌 + 两根长獠牙与成排颊齿 + 尖耳（三段）
  2. 颈      两节粗颈 + 颈鬃 + 一块被剥开的颈侧
  3. 躯干    27 片肌肉桶（每片 3 层：背 / 腰 / 腹）+ 肋侧被撕开的生肌与外露肋骨
  4. 背肌条  沿脊一道裸露的生肌条 + 椎骨突起 + 两侧斜纹肌束
  5. 毛      肩鬃、脊毛刺、肋侧下垂的锯齿皮毛、腹毛、臀毛
  6. 四肢    前：肩 - 肱 - 桡 - 掌 - 趾；后：股 - 胫 - 跗 - 跖 - 趾；肌肉块 + 4 趾 4 爪
  7. 尾      两节尾根 + 在第二节后分成左右两股的叉尾，每股 3 节

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X。

用法：
  python3 modelScript/creatures/bo_pi_chai/gen_bo_pi_chai.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/bo_pi_chai/gen_bo_pi_chai.py --self-test # 门禁差分自证
  python3 modelScript/creatures/bo_pi_chai/gen_bo_pi_chai.py --list
  python3 modelScript/creatures/bo_pi_chai/gen_bo_pi_chai.py --part head
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
import proportion_check as pc  # noqa: E402

from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig, shaft_box  # noqa: E402

CREATURE_DIR = REPO / "modelScript" / "creatures" / "bo_pi_chai"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/bo_pi_chai")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "BoPiChai.bbmodel"
BBMODEL_COPY = MODELS_DIR / "BoPiChai.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质必须 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "fur_red": (146, 54, 44),       # 暗红的主毛色
    "fur_dark": (76, 32, 30),       # 深褐的底毛、腹下、爪背
    "fur_pale": (186, 128, 104),     # 被扯掉毛后露出的苍白硬皮、耳内
    "muscle_raw": (206, 52, 56),    # 裸露的生肌
    "muscle_dark": (96, 26, 34),    # 筋腱、牙龈、肌束暗面
    "bone_white": (228, 216, 192),  # 獠牙、肋骨、椎骨突起、颅骨
    "claw_dark": (34, 30, 30),      # 爪、鼻头、眼窝
    "eye_amber": (234, 172, 50),    # 琥珀眼
}

# ---------------------------------------------------------------------------
# 骨骼：(名称, pivot(左侧 x 为负), 父骨)。右侧由 define_bones 生成。
# ---------------------------------------------------------------------------
# 躯干加长量（返工：用户「身体太短了，按照比例来」）。参考图侧视量得 躯干长 ÷ 肩高 ≈ 1.52、头颈长 ÷ 躯干长 ≈ 0.43；
# 旧模型是 0.76 / 0.95，所以躯干从 13 单位加长到约 27。胸 / 肩 / 颈 / 头 / 前肢留在原处，从腰往后整体后移 REAR_DZ：
# 后腿、尾和叉尾整体平移，躯干在中间多出来的长度由更多的肌肉片、一节新的腰脊骨（spine_mid）和后半段肋侧毛承担。
REAR_DZ = 12.0
REAR_LIMB_BASES = frozenset(("thigh", "shin", "hock", "paw_b", "toes_b", "tail_1", "tail_2", "fork_1", "fork_2", "fork_3"))

CENTER_BONES = [
    ("root", (0.0, 0.0, 0.0), None),
    ("hips", (0.0, 11.6, 6.2 + REAR_DZ), "root"),
    ("spine_mid", (0.0, 12.2, 10.0), "hips"),
    ("spine_lumbar", (0.0, 12.4, 3.2), "spine_mid"),
    ("thorax", (0.0, 12.8, -2.8), "spine_lumbar"),
    ("neck_base", (0.0, 12.6, -5.6), "thorax"),
    ("neck_upper", (0.0, 12.4, -7.6), "neck_base"),
    ("head", (0.0, 12.2, -9.2), "neck_upper"),
    ("snout", (0.0, 11.8, -11.8), "head"),
    ("jaw", (0.0, 10.6, -10.0), "head"),
    ("tail_1", (0.0, 12.3, 6.4), "hips"),
    ("tail_2", (0.0, 11.4, 10.6), "tail_1"),
]

SIDE_BONES = [
    ("ear", (1.6, 15.0, -9.6), "head"),
    ("shoulder", (2.4, 13.4, -4.4), "thorax"),
    ("arm", (2.7, 11.2, -4.6), "shoulder"),
    ("forearm", (2.9, 6.9, -3.2), "arm"),
    ("paw_f", (2.7, 3.0, -4.8), "forearm"),
    ("toes_f", (2.7, 0.9, -5.6), "paw_f"),
    ("thigh", (2.8, 11.2, 6.4), "hips"),
    ("shin", (3.0, 7.0, 3.6), "thigh"),
    ("hock", (2.8, 3.8, 8.0), "shin"),
    ("paw_b", (2.8, 0.9, 7.0), "hock"),
    ("toes_b", (2.8, 0.9, 6.6), "paw_b"),
    ("fur_flank", (2.6, 11.5, 2.5), "spine_lumbar"),
    ("fur_flank_b", (2.6, 11.5, 10.5), "spine_mid"),
    ("fur_shoulder", (3.0, 12.5, -3.5), "thorax"),
    # 叉尾：第二节尾根之后分成左右两股，每股 3 节
    ("fork_1", (0.9, 11.0, 12.0), "tail_2"),
    ("fork_2", (1.6, 10.0, 14.0), "fork_1"),
    ("fork_3", (2.4, 8.7, 16.0), "fork_2"),
]

MIN_MOVING_BONES = 32


def rear_dz(bone: str) -> float:
    """后腿 / 尾 / 叉尾这一族的骨（含 _l / _r 后缀）整体后移 REAR_DZ；其余骨不动。"""
    base = bone[:-2] if bone.endswith(("_l", "_r")) else bone
    return REAR_DZ if base in REAR_LIMB_BASES else 0.0


def define_bones(rig: Rig) -> None:
    for name, (x, y, z), parent in CENTER_BONES:
        rig.bone(name, (x, y, z + rear_dz(name)), parent)
    side_names = {b for b, _, _ in SIDE_BONES}
    for suffix, sx in (("_l", -1.0), ("_r", 1.0)):
        for base, (x, y, z), parent in SIDE_BONES:
            par = parent + suffix if parent in side_names else parent
            rig.bone(base + suffix, (x * sx, y, z + rear_dz(base)), par)


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
    dz = rear_dz(bone)
    frm, to, rot, org = shaft_box((a[0], a[1], a[2] + dz), (b[0], b[1], b[2] + dz), rx, rz, extend, up)
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


def box(rig: Rig, bone: str, name: str, lo: Vec, hi: Vec, *, mat: str) -> None:
    dz = rear_dz(bone)
    rig.cube(bone, name, (lo[0], lo[1], lo[2] + dz), (hi[0], hi[1], hi[2] + dz), mat=mat)


def mx(x: float, s: float) -> float:
    return x * s


def side_name(base: str, s: float) -> str:
    return base + ("_l" if s < 0 else "_r")


def lerp3(a: Vec, b: Vec, t: float) -> Vec:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


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
    """一片锯齿下垂的毛片：三段逐渐收窄，向外微翻；左右两侧用同一串随机数才能严格镜像，所以 x 抖动乘 s。"""
    flare = math.tan(math.radians(flare_deg)) * s

    def jit(amount: float) -> float:
        return rng.uniform(-amount, amount) if rng else 0.0

    pts = [top]
    for t in (0.42, 0.78, 1.0):
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


# ===========================================================================
# 1. 头
# ===========================================================================
JAW_HINGE = (0.0, 10.6, -10.0)
JAW_OPEN_DEG = 28.0


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
    """粗颅 + 厚吻 + 张口下颌 + 两根长獠牙 + 成排颊齿 + 尖耳；鼻梁一道苍白的剥皮。"""
    # ── 颅：后颅毛覆，额骨与眉脊略露，左太阳穴被剥掉一片 ──
    box(rig, "head", "cranium_back", (-2.5, 10.8, -10.2), (2.5, 14.2, -8.2), mat="fur_red")
    box(rig, "head", "cranium_front", (-2.2, 11.2, -12.0), (2.2, 14.0, -10.2), mat="fur_dark")
    box(rig, "head", "forehead_peel", (-0.9, 13.9, -11.8), (0.9, 14.35, -9.4), mat="fur_pale")
    box(rig, "head", "poll_fur", (-1.8, 13.5, -8.2), (1.8, 14.5, -7.2), mat="fur_dark")
    for s in (-1.0, 1.0):
        box(rig, "head", side_name("eye_socket", s), (mx(1.7, s), 12.1, -12.1), (mx(2.55, s), 13.4, -10.9), mat="claw_dark")
        box(rig, "head", side_name("eye_amber", s), (mx(1.85, s), 12.4, -12.25), (mx(2.5, s), 13.1, -11.5), mat="eye_amber")
        bar(rig, "head", side_name("brow_ridge", s), (mx(1.2, s), 13.7, -12.4), (mx(2.8, s), 13.7, -10.2), 0.35, 0.3,
            mat="bone_white", up=(0, 1, 0))
        box(rig, "head", side_name("cheek_fur", s), (mx(2.5, s), 10.8, -11.0), (mx(3.0, s), 13.2, -8.6), mat="fur_red")
        box(rig, "head", side_name("temple_raw", s), (mx(2.5, s), 13.2, -10.2), (mx(2.9, s), 14.0, -8.8), mat="muscle_raw")
        bar(rig, "head", side_name("zygoma", s), (mx(2.4, s), 10.9, -12.3), (mx(3.0, s), 11.2, -9.6), 0.3, 0.3, mat="bone_white")

    # ── 吻：粗而厚，三段收窄；鼻梁覆毛，两侧唇缘外翻露出牙龈 ──
    pts = [(0.0, 12.0, -12.0), (0.0, 11.6, -14.2), (0.0, 11.2, -16.2), (0.0, 10.9, -17.6)]
    hw = [1.9, 1.6, 1.3, 1.1]
    hh = [1.3, 1.15, 1.0, 0.85]
    for i in range(3):
        bar(rig, "snout", f"muzzle_{i}", pts[i], pts[i + 1], (hw[i] + hw[i + 1]) / 2, (hh[i] + hh[i + 1]) / 2,
            mat="fur_red" if i != 1 else "fur_dark", up=(0, 1, 0))
    bar(rig, "snout", "nose_peel", (0.0, 12.9, -12.6), (0.0, 12.45, -15.2), 0.5, 0.2, mat="fur_pale", up=(0, 1, 0))
    box(rig, "snout", "nose_pad", (-1.0, 10.4, -18.5), (1.0, 11.8, -17.6), mat="claw_dark")
    for s in (-1.0, 1.0):
        bar(rig, "snout", side_name("gum_upper", s), (mx(1.75, s), 10.65, -12.2), (mx(1.0, s), 10.35, -17.4), 0.28, 0.32,
            mat="muscle_dark", up=(0, 1, 0))
        bar(rig, "snout", side_name("lip_raw", s), (mx(1.95, s), 11.3, -13.0), (mx(1.4, s), 10.8, -17.0), 0.18, 0.4,
            mat="muscle_raw", up=(s, 0, 0))
        # 长獠牙：两段，根粗尖细，向下微朝前
        bar(rig, "snout", side_name("fang_up_a", s), (mx(1.1, s), 10.4, -15.4), (mx(1.1, s), 8.7, -15.75), 0.42, 0.42,
            mat="bone_white", up=(1, 0, 0))
        bar(rig, "snout", side_name("fang_up_b", s), (mx(1.1, s), 8.7, -15.75), (mx(1.1, s), 7.5, -16.05), 0.24, 0.24,
            mat="bone_white", up=(1, 0, 0))
        for k, zc in enumerate((-12.9, -13.8, -14.5)):
            bar(rig, "snout", side_name(f"premolar_up{k}", s), (mx(1.55 - 0.05 * k, s), 10.5, zc), (mx(1.55 - 0.05 * k, s), 9.2 - 0.1 * k, zc - 0.1),
                0.24, 0.3, mat="bone_white", up=(1, 0, 0))
        for k, xo in enumerate((0.35, 0.8)):
            bar(rig, "snout", side_name(f"incisor_up{k}", s), (mx(xo, s), 10.35, -17.55), (mx(xo, s), 9.4, -17.6), 0.2, 0.22,
                mat="bone_white", up=(1, 0, 0))

    # ── 下颌：绕铰链张开 28°，下颌骨两支 + 下獠牙 + 颊齿 + 舌 ──
    box(rig, "jaw", "jaw_hinge_block", (-1.8, 10.1, -10.8), (1.8, 11.2, -9.4), mat="claw_dark")
    for s in (-1.0, 1.0):
        bar(rig, "jaw", side_name("mandible_back", s), jaw_pt(mx(1.7, s), 10.4, -10.2), jaw_pt(mx(1.4, s), 9.9, -13.0), 0.35, 0.55,
            mat="fur_dark", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("mandible_front", s), jaw_pt(mx(1.4, s), 9.9, -13.0), jaw_pt(mx(1.0, s), 9.7, -16.4), 0.3, 0.5,
            mat="fur_red", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("coronoid", s), jaw_pt(mx(1.7, s), 10.4, -10.4), jaw_pt(mx(1.6, s), 11.9, -10.7), 0.32, 0.22,
            mat="bone_white", up=(1, 0, 0))
        bar(rig, "jaw", side_name("jaw_gum", s), jaw_pt(mx(1.25, s), 10.6, -11.5), jaw_pt(mx(0.95, s), 10.35, -16.2), 0.24, 0.32,
            mat="muscle_dark", up=jaw_dir((0, 1, 0)))
        bar(rig, "jaw", side_name("fang_down_a", s), jaw_pt(mx(0.95, s), 10.5, -15.6), jaw_pt(mx(0.95, s), 12.0, -15.65), 0.34, 0.34,
            mat="bone_white", up=(1, 0, 0))
        bar(rig, "jaw", side_name("fang_down_b", s), jaw_pt(mx(0.95, s), 12.0, -15.65), jaw_pt(mx(0.95, s), 13.1, -15.7), 0.2, 0.2,
            mat="bone_white", up=(1, 0, 0))
        for k, zc in enumerate((-12.4, -13.3, -14.2)):
            bar(rig, "jaw", side_name(f"premolar_down{k}", s), jaw_pt(mx(1.3, s), 10.55, zc), jaw_pt(mx(1.3, s), 11.55 + 0.1 * k, zc - 0.1),
                0.24, 0.3, mat="bone_white", up=(1, 0, 0))
    bar(rig, "jaw", "chin_cap", jaw_pt(0.0, 9.6, -16.9), jaw_pt(0.0, 10.0, -16.1), 0.7, 0.4, mat="fur_dark", up=(1, 0, 0))
    bar(rig, "jaw", "tongue_root", jaw_pt(0.0, 10.4, -12.0), jaw_pt(0.0, 10.4, -14.4), 0.6, 0.3, mat="muscle_raw", up=jaw_dir((0, 1, 0)))
    bar(rig, "jaw", "tongue_tip", jaw_pt(0.0, 10.4, -14.4), jaw_pt(0.0, 9.8, -15.6), 0.45, 0.25, mat="muscle_raw", up=jaw_dir((0, 1, 0)))

    # ── 尖耳：三段收细，耳内苍白，耳尖一道缺口 ──
    for s in (-1.0, 1.0):
        n = (s * 0.4, 0.0, -0.92)
        bar(rig, side_name("ear", s), side_name("ear_base", s), (mx(1.5, s), 14.1, -9.5), (mx(2.2, s), 15.9, -9.4), 1.45, 0.3,
            mat="fur_red", up=n)
        bar(rig, side_name("ear", s), side_name("ear_mid", s), (mx(2.2, s), 15.9, -9.4), (mx(2.7, s), 17.6, -9.3), 1.0, 0.26,
            mat="fur_dark", up=n)
        bar(rig, side_name("ear", s), side_name("ear_tip", s), (mx(2.7, s), 17.6, -9.3), (mx(3.0, s), 19.0, -9.2), 0.5, 0.22,
            mat="fur_dark", up=n)
        bar(rig, side_name("ear", s), side_name("ear_inner", s), (mx(1.6, s), 14.4, -9.75), (mx(2.5, s), 17.6, -9.55), 0.62, 0.12,
            mat="fur_pale", up=n)
    box(rig, "ear_l", "ear_l_notch", (-3.35, 16.6, -9.8), (-2.85, 17.4, -8.8), mat="muscle_dark")


# ===========================================================================
# 2. 颈
# ===========================================================================
NECK_PATH = [(0.0, 12.9, -4.8), (0.0, 12.9, -6.4), (0.0, 12.7, -8.0), (0.0, 12.4, -9.4)]


def part_neck(rig: Rig) -> None:
    """两节粗颈：颈骨 + 肌肉套 + 颈鬃；颈侧被剥开一块，露出生肌。"""
    for i in range(3):
        a, b = NECK_PATH[i], NECK_PATH[i + 1]
        bone = "neck_base" if i < 2 else "neck_upper"
        r = 2.4 - 0.2 * i
        bar(rig, bone, f"neck_fur_{i}", a, b, r, r - 0.2, mat="fur_red" if i % 2 == 0 else "fur_dark", up=(1, 0, 0))
        bar(rig, bone, f"neck_core_{i}", lerp3(a, b, 0.1), lerp3(a, b, 0.9), 0.6, 0.6, mat="bone_white")
    for s in (-1.0, 1.0):
        bar(rig, "neck_base", side_name("neck_peel", s), (mx(2.15, s), 13.8, -5.9), (mx(2.0, s), 12.0, -7.3), 0.7, 0.2,
            mat="muscle_raw", up=(s, 0, 0))
        bar(rig, "neck_upper", side_name("throat_sinew", s), (mx(1.3, s), 11.3, -8.3), (mx(1.2, s), 10.3, -6.4), 0.24, 0.3,
            mat="muscle_dark", up=(1, 0, 0))
    # 颈鬃：成排向后倾的毛刺，两排
    for k, (z, y, h) in enumerate(((-5.2, 14.9, 2.0), (-6.1, 15.0, 2.3), (-7.0, 14.9, 2.2), (-7.9, 14.7, 1.9), (-8.8, 14.5, 1.5))):
        bone = "neck_base" if k < 2 else "neck_upper"
        for s in (-1.0, 1.0):
            for row, xo in enumerate((0.6, 1.5)):
                tuft(rig, bone, side_name(f"mane_tuft{k}_{row}", s), (mx(xo, s), y - 0.45 * row, z), h - 0.5 * row, 0.8,
                     "fur_dark" if (k + row) % 2 else "fur_red")
    for s in (-1.0, 1.0):
        # 左右两侧各自从同一个种子起算，随机抖动才会严格镜像
        rng = random.Random(6103)
        for k, (z, ln) in enumerate(((-5.4, 2.6), (-6.4, 2.2), (-7.4, 1.8))):
            bone = "neck_base" if k < 2 else "neck_upper"
            rag(rig, bone, side_name(f"neck_fringe{k}", s), (mx(2.3 - 0.1 * k, s), 11.8, z), ln, 0.9, s, rng, "fur_red" if k % 2 else "fur_dark",
                flare_deg=8.0)


# ===========================================================================
# 3. 躯干：肌肉桶
# ===========================================================================
SLICE_Z0 = -6.2
SLICES = 25                       # 每片厚 1 单位；旧版 13 片，加长后 25 片（躯干 -6.2 → 约 18.8）
# (z, 半宽, 脊线 y, 腹线 y)：胸深而宽，腰收长一段，臀再宽。前五个点是旧版的胸 / 肩，后面把旧版的 2.5 / 4.5 / 7.0 三个点
# 沿加长后的腰线重新分布（旧版这三个点只隔 7 单位，现在隔 21 单位），并多一个中腰点，免得腰线被拉成一根直管
# Round 3：胸深按参考图侧视加厚——参考图量得 胸深 ÷ 肩高 ≈ 0.55（肩顶到胸底），旧版只有 0.45。脊线 y 不动（肩高、
# 躯干长 ÷ 肩高 = 1.43 和头颈比 0.49 都靠它），只把腹线在胸 / 肋段整体降低 1.3~1.5，腰往后再慢慢收起来，
# 胸宽也各加 0.3~0.4，让胸腔是一只深而圆的桶。
PROFILE = [
    (-6.2, 2.6, 13.9, 8.8),
    (-4.5, 3.5, 14.5, 6.4),
    (-2.5, 3.5, 14.4, 6.3),
    (0.0, 3.3, 14.0, 6.6),
    (4.0, 3.1, 13.6, 7.3),
    (8.5, 2.9, 13.3, 7.9),
    (13.0, 3.0, 13.2, 8.4),
    (7.0 + REAR_DZ, 3.2, 13.0, 8.9),
]
# 每片脊线的小起伏：毛面参差，别成一块平板（按片号循环取）
SLICE_JITTER = [0.15, -0.10, 0.25, -0.05, 0.20, -0.15, 0.10, 0.25, -0.10, 0.15, -0.05, 0.20, -0.10]


def torso_profile(z: float) -> tuple[float, float, float]:
    for (z0, *a), (z1, *b) in zip(PROFILE, PROFILE[1:]):
        if z0 <= z <= z1:
            t = (z - z0) / (z1 - z0)
            return tuple(a[k] + (b[k] - a[k]) * t for k in range(3))  # type: ignore[return-value]
    return tuple(PROFILE[-1][1:]) if z > PROFILE[-1][0] else tuple(PROFILE[0][1:])  # type: ignore[return-value]


def slice_geom(i: int) -> tuple[float, float, float, float, float, float]:
    """第 i 片：(z0, z1, 半宽, 脊线 y, 腹线 y, 腹层顶 y)。"""
    z0 = SLICE_Z0 + i
    zc = z0 + 0.475
    w, ytop, ybot = torso_profile(zc)
    ytop += SLICE_JITTER[i % len(SLICE_JITTER)]
    return z0, z0 + 0.95, w, ytop, ybot, ybot + 0.3 * (ytop - ybot)


def slice_bone(i: int) -> str:
    """胸 / 前腰 / 后腰 / 臀四段，各挂一根脊骨；后三段让躯干能弯。"""
    z = SLICE_Z0 + i + 0.475
    if z < -0.8:
        return "thorax"
    if z < 7.5:
        return "spine_lumbar"
    if z < 4.2 + REAR_DZ:
        return "spine_mid"
    return "hips"


# 撕开的位置：左右错开，每处是一块生肌，边上有一圈苍白硬皮，胸侧还露两三根肋；
# 每侧两处：一处在肩（旧版就有），一处新加在后腰靠近后腿的位置（躯干加长后的大腿外侧）
TORN = {-1.0: ((3, 4, 5), (16, 17, 18)), 1.0: ((4, 5, 6), (17, 18, 19))}
TORN_SLICES = {s: frozenset(i for group in groups for i in group) for s, groups in TORN.items()}
TORN_NEIGHBOURS = {s: TORN_SLICES[s] | frozenset(g[0] - 1 for g in groups) | frozenset(g[-1] + 1 for g in groups)
                   for s, groups in TORN.items()}


def torn_bone(i: int, s: float) -> str:
    """撕口左右不对称，所以挂在成对的侧骨上（门禁按骨豁免镜像检查），不挂中线骨。"""
    return side_name(FLANK_BONE_OF_SPINE[slice_bone(i)], s)


# 肋侧的毛 / 撕口跟着它所在的那一段脊骨走
FLANK_BONE_OF_SPINE = {"thorax": "fur_shoulder", "spine_lumbar": "fur_flank", "spine_mid": "fur_flank_b", "hips": "fur_flank_b"}


def part_torso(rig: Rig) -> None:
    """27 片肌肉桶，每片 3 层（背 / 腰 / 腹）；肋侧每边两处被撕开（肩一处、后腰一处）。"""
    for i in range(SLICES):
        z0, z1, w, ytop, ybot, ylow = slice_geom(i)
        bone = slice_bone(i)
        ymid = ytop - 1.5
        box(rig, bone, f"slice{i}_top", (-w * 0.82, ymid, z0), (w * 0.82, ytop, z1), mat="fur_red" if i % 2 == 0 else "fur_dark")
        box(rig, bone, f"slice{i}_mid", (-w, ylow, z0), (w, ymid, z1), mat="fur_dark" if i % 3 == 0 else "fur_red")
        box(rig, bone, f"slice{i}_low", (-w * 0.8, ybot, z0), (w * 0.8, ylow, z1), mat="fur_dark")
    for s, groups in TORN.items():
        for slices in groups:
            for n, i in enumerate(slices):
                z0, z1, w, ytop, ybot, ylow = slice_geom(i)
                bone = torn_bone(i, s)
                y_lo, y_hi = ylow + 0.2, ytop - 1.7
                box(rig, bone, side_name(f"torn{i}", s), (mx(w - 0.05, s), y_lo, z0), (mx(w + 0.2, s), y_hi, z1),
                    mat="muscle_raw" if n % 2 == 0 else "muscle_dark")
                # 外露的肋：每处撕口 2 根竖直的肋骨
                for k, zr in enumerate((z0 + 0.25, z0 + 0.7)):
                    bar(rig, bone, side_name(f"torn_rib{i}_{k}", s), (mx(w + 0.2, s), y_lo - 0.1, zr), (mx(w + 0.45, s), y_hi + 0.05, zr), 0.18, 0.2,
                        mat="bone_white", up=(0, 0, 1))
            # 撕口边缘的苍白硬皮：撕口首尾相邻的那一片，各贴一条薄边
            first, last = slices[0] - 1, slices[-1] + 1
            for tag, i in (("a", first), ("b", last)):
                z0, z1, w, ytop, ybot, ylow = slice_geom(i)
                box(rig, torn_bone(i, s), side_name(f"torn_edge{slices[0]}{tag}", s), (mx(w - 0.05, s), ylow + 0.2, z0),
                    (mx(w + 0.2, s), ytop - 1.7, z1), mat="fur_pale")


# ===========================================================================
# 4. 背肌条：沿脊一道裸露的生肌
# ===========================================================================
def part_back_strip(rig: Rig) -> None:
    """沿脊 27 节：生肌条 + 椎骨突起；偶数节两侧各一束斜纹肌，奇数节两侧各立一丛毛刺。"""
    for i in range(SLICES):
        z0, z1, w, ytop, ybot, ylow = slice_geom(i)
        bone = slice_bone(i)
        zc = (z0 + z1) / 2
        box(rig, bone, f"strip{i}", (-0.9, ytop, z0), (0.9, ytop + 0.8, z1), mat="muscle_dark" if i % 3 == 1 else "muscle_raw")
        box(rig, bone, f"knob{i}", (-0.34, ytop + 0.8, zc - 0.34), (0.34, ytop + 1.5, zc + 0.34), mat="bone_white")
        for s in (-1.0, 1.0):
            if i % 2 == 0:
                bar(rig, bone, side_name(f"fibre{i}", s), (mx(0.9, s), ytop + 0.9, zc), (mx(2.0, s), ytop + 0.35, zc), 0.4, 0.2,
                    mat="muscle_dark", up=(0, 1, 0))
            else:
                tuft(rig, bone, side_name(f"spine_tuft{i}", s), (mx(w * 0.62, s), ytop, zc), 1.5 + 0.5 * (i % 3), 0.7,
                     "fur_dark" if i % 4 == 1 else "fur_red")


# ===========================================================================
# 5. 毛：肩鬃 / 肋侧垂毛 / 腹毛 / 臀毛
# ===========================================================================
def part_fur(rig: Rig) -> None:
    """肩上一丛粗鬃、肋侧下垂的锯齿皮毛（撕口两侧没有）、腹下碎毛、臀上毛刺。"""
    # 肩鬃：全身毛最完整的一处，三排向后倾的毛刺
    for k, (z, h) in enumerate(((-5.6, 2.2), (-4.9, 2.8), (-4.2, 3.0), (-3.5, 2.6), (-2.8, 2.0))):
        for s in (-1.0, 1.0):
            for row, xo in enumerate((0.45, 1.3, 2.15)):
                tuft(rig, side_name("fur_shoulder", s), side_name(f"mane_tuft{k}_{row}", s), (mx(xo, s), 14.0 + 0.5 * (k % 2) - 0.3 * row, z),
                     h - 0.5 * row, 0.8, "fur_dark" if (k + row) % 2 else "fur_red")
    for s in (-1.0, 1.0):
        rng = random.Random(8211)
        # 肋侧垂毛：撕口所在的片不垂，其余每隔一片垂一条；毛挂在它所在那一段脊骨的肋侧毛骨上
        for i in range(SLICES):
            if i in TORN_NEIGHBOURS[s]:
                continue
            if i % 2 == 1:
                continue
            z0, z1, w, ytop, ybot, ylow = slice_geom(i)
            rag(rig, torn_bone(i, s), side_name(f"flank_rag{i}", s), (mx(w * 0.8, s), ylow + 0.3, (z0 + z1) / 2), rng.uniform(2.0, 3.6), 0.85, s, rng,
                "fur_red" if i % 4 == 0 else "fur_dark", flare_deg=10.0)
        # 腹下碎毛：沿整条腹线每隔约 1.8 单位一撮（旧版只有 4 撮，对应加长前的 7 单位腹线）
        for k in range(12):
            z = -1.5 + 1.8 * k
            bone = side_name("fur_flank" if z < 7.5 else "fur_flank_b", s)
            belly_y = torso_profile(z)[2]
            rag(rig, bone, side_name(f"belly_rag{k}", s), (mx(1.6, s), belly_y - 0.05 + 0.2 * (k % 2), z), rng.uniform(1.0, 1.8), 0.8, s, rng,
                "fur_dark", flare_deg=6.0)
        # 臀上毛刺：向后倾的一排，紧挨后腿上方（旧版 z = 5.6 / 6.4，随臀后移）
        for k, z in enumerate((5.6 + REAR_DZ, 6.4 + REAR_DZ)):
            tuft(rig, side_name("fur_flank_b", s), side_name(f"rump_tuft{k}", s), (mx(1.9, s), 13.1, z), 1.6 + 0.3 * (k % 2), 0.8,
                 "fur_red" if k else "fur_dark")


# ===========================================================================
# 5b. 肩背蓬毛：一片压一片向后倒的毛片（Round 3）
# ===========================================================================
SCRUFF_SEED = 5417


def shingle(rig: Rig, bone: str, name: str, base: Vec, s: float, length: float, width: float, rng: random.Random, mat: str,
            lift: float) -> None:
    """一片向后（+Z）倒的蓬毛片：两段（根宽尖窄），整体向外翻、尖端略翘 lift；薄的一维朝外。
    抖动乘 s、左右两侧用同一串随机数，才能严格镜像。"""
    nrm = (s, 0.45, 0.0)
    p1 = (base[0] + s * rng.uniform(0.15, 0.4), base[1] + lift * 0.4 + rng.uniform(-0.1, 0.15), base[2] + length * 0.55)
    p2 = (p1[0] + s * rng.uniform(0.2, 0.5), p1[1] + lift * 0.6 + rng.uniform(-0.1, 0.2), base[2] + length)
    bar(rig, bone, f"{name}_a", base, p1, width * 0.5, 0.2, mat=mat, up=nrm)
    bar(rig, bone, f"{name}_b", p1, p2, width * 0.34, 0.18, mat=mat, up=nrm)


def part_scruff(rig: Rig) -> None:
    """肩胛上三排、背侧两排的蓬毛片：同排相邻两片交替毛色，排与排错位半片，像没打理过的乱毛。
    肩上的挂 fur_shoulder 骨、背侧的挂 fur_flank / fur_flank_b 骨（都是有意不对称的毛骨，不进镜像门）。
    最高的毛尖不超过肩鬃的最高点，所以不改侧视量尺量出的肩高。"""
    mats = ("fur_red", "fur_dark", "fur_pale")
    for s in (-1.0, 1.0):
        rng = random.Random(SCRUFF_SEED)
        # 肩：3 排 × 7 片，z 从 -5.9 到 -0.9；排高 10.9 / 12.3 / 13.7，越高越向内收（贴着肩背弧）
        for row, y in enumerate((10.9, 12.3, 13.7)):
            for col in range(7):
                z = -5.9 + 0.85 * col + (0.42 if row % 2 else 0.0)
                w = torso_profile(z)[0]
                x = w - 0.05 - 0.25 * row
                ln = rng.uniform(2.6, 3.8) - 0.3 * row
                shingle(rig, side_name("fur_shoulder", s), side_name(f"scruff_sh{row}_{col}", s), (mx(x, s), y + rng.uniform(-0.15, 0.15), z), s,
                        ln, rng.uniform(1.0, 1.4), rng, mats[(row + col) % 2 + (1 if rng.random() < 0.18 else 0)], 0.9 + 0.2 * row)
        # 背侧：从肩后到臀，每 1.45 一片，两排（高排靠脊、低排靠肋）
        for k in range(12):
            z = -0.2 + 1.45 * k
            w, ytop, ybot = torso_profile(z)
            for row, (dy, dx) in enumerate(((0.9, 0.82), (2.6, 1.0))):
                zz = z + (0.7 if row else 0.0)
                bone = side_name("fur_flank" if zz < 7.5 else "fur_flank_b", s)
                ln = rng.uniform(2.2, 3.4)
                shingle(rig, bone, side_name(f"scruff_bk{row}_{k}", s), (mx(w * dx, s), ytop - dy, zz), s, ln, rng.uniform(0.9, 1.3), rng,
                        mats[(k + row) % 2 + (1 if rng.random() < 0.15 else 0)], 0.7)


# ===========================================================================
# 6. 四肢
# ===========================================================================
def toe_group(rig: Rig, bone: str, s: float, tag: str, base_z: float, x0: float, forward: float) -> None:
    """一只脚 4 趾：中间两趾长、外侧两趾短；每趾一段趾节 + 一枚弯向地面的黑爪。"""
    offs = (-1.1, -0.38, 0.38, 1.1)
    lens = (1.0, 1.5, 1.5, 1.0)
    for k, (o, ln) in enumerate(zip(offs, lens)):
        xo = x0 + s * o
        a = (xo, 1.0, base_z)
        b = (xo + s * o * 0.12, 0.5, base_z + forward * ln)
        c = (xo + s * o * 0.2, 0.2, base_z + forward * (ln + 0.95))
        bar(rig, bone, side_name(f"toe_{tag}{k}", s), a, b, 0.4, 0.4, mat="fur_dark")
        bar(rig, bone, side_name(f"claw_{tag}{k}", s), b, c, 0.2, 0.2, mat="claw_dark")
    box(rig, bone, side_name(f"pad_{tag}", s), (x0 - 1.5, 0.0, base_z + forward * 0.35 - 0.5), (x0 + 1.5, 0.5, base_z + forward * 0.35 + 0.5),
        mat="claw_dark")


def front_leg(rig: Rig, s: float) -> None:
    """肩 → 肱（肌肉块 + 毛袖）→ 桡（筋束）→ 腕 / 掌 → 趾。"""
    sh, ar, fo = side_name("shoulder", s), side_name("arm", s), side_name("forearm", s)
    pf, tf = side_name("paw_f", s), side_name("toes_f", s)
    shoulder, elbow, wrist = (mx(2.7, s), 11.2, -4.6), (mx(2.9, s), 6.9, -3.2), (mx(2.7, s), 3.0, -4.8)
    # 肩胛上的肩峰：毛覆的厚块，外侧被撕开一小片露出生肌
    bar(rig, sh, side_name("scapula_mass", s), (mx(3.55, s), 14.3, -4.2), (mx(3.75, s), 11.2, -4.9), 1.35, 0.55, mat="fur_red", up=(s, 0, 0))
    bar(rig, sh, side_name("scapula_raw", s), (mx(4.0, s), 13.5, -4.3), (mx(4.05, s), 12.0, -4.7), 0.7, 0.18, mat="muscle_raw", up=(s, 0, 0))
    box(rig, sh, side_name("shoulder_knob", s), (mx(2.3, s), 10.6, -5.3), (mx(3.3, s), 11.6, -4.1), mat="bone_white")
    # 肱：一整块肌肉 + 上半截毛袖 + 一道筋
    bar(rig, ar, side_name("humerus_muscle", s), shoulder, elbow, 1.15, 1.0, mat="muscle_raw")
    bar(rig, ar, side_name("humerus_fur", s), (mx(3.45, s), 11.4, -4.7), (mx(3.55, s), 8.8, -3.9), 0.85, 0.45, mat="fur_red", up=(s, 0, 0))
    bar(rig, ar, side_name("humerus_sinew", s), (mx(2.15, s), 10.4, -4.5), (mx(2.35, s), 7.4, -3.3), 0.2, 0.22, mat="muscle_dark")
    bar(rig, fo, side_name("olecranon", s), (mx(2.9, s), 7.2, -3.2), (mx(2.9, s), 7.7, -2.0), 0.45, 0.45, mat="bone_white")
    # 桡：细一些的肌肉 + 两侧筋束 + 毛段
    bar(rig, fo, side_name("radius_muscle", s), (mx(3.0, s), 7.0, -3.4), wrist, 0.8, 0.7, mat="muscle_dark")
    bar(rig, fo, side_name("forearm_raw", s), (mx(3.85, s), 6.8, -3.5), (mx(3.65, s), 4.4, -4.5), 0.4, 0.2, mat="muscle_raw", up=(s, 0, 0))
    bar(rig, fo, side_name("forearm_fur", s), (mx(2.3, s), 6.8, -3.0), (mx(2.1, s), 4.6, -4.2), 0.55, 0.3, mat="fur_dark", up=(s, 0, 0))
    # 腕与掌：腕垫 + 粗掌骨（毛覆）
    box(rig, pf, side_name("carpus", s), (mx(2.0, s), 2.5, -5.4), (mx(3.4, s), 3.6, -4.1), mat="fur_dark")
    box(rig, pf, side_name("carpal_pad", s), (mx(2.4, s), 2.8, -4.0), (mx(3.1, s), 3.7, -3.6), mat="claw_dark")
    for k, o in enumerate((-0.5, 0.5)):
        bar(rig, pf, side_name(f"metacarpal{k}", s), (mx(2.7 + o, s), 2.7, -4.8), (mx(2.7 + o, s), 1.0, -5.6), 0.4, 0.4, mat="fur_red")
    toe_group(rig, tf, s, "f", -5.6, mx(2.7, s), -1.0)


def hind_leg(rig: Rig, s: float) -> None:
    """股（大块肌肉 + 毛）→ 胫 → 跗（跟骨后翘）→ 跖 → 趾；后腿 Z 字：膝向前、跗向后。"""
    th, sn, hk = side_name("thigh", s), side_name("shin", s), side_name("hock", s)
    pb, tb = side_name("paw_b", s), side_name("toes_b", s)
    hip, stifle, hock, mtp = (mx(2.8, s), 11.2, 6.4), (mx(3.0, s), 7.0, 3.6), (mx(2.8, s), 3.8, 8.0), (mx(2.8, s), 0.9, 7.0)
    bar(rig, th, side_name("thigh_muscle", s), hip, stifle, 1.6, 1.3, mat="muscle_raw")
    bar(rig, th, side_name("thigh_fur", s), (mx(4.0, s), 12.4, 6.4), (mx(4.15, s), 8.8, 4.8), 1.6, 0.6, mat="fur_red", up=(s, 0, 0))
    bar(rig, th, side_name("thigh_fur_low", s), (mx(4.15, s), 8.8, 4.8), (mx(4.2, s), 7.2, 4.0), 1.0, 0.5, mat="fur_dark", up=(s, 0, 0))
    bar(rig, th, side_name("thigh_sinew", s), (mx(1.9, s), 9.8, 5.0), (mx(2.2, s), 7.6, 3.8), 0.22, 0.24, mat="muscle_dark")
    box(rig, th, side_name("patella", s), (mx(2.4, s), 6.6, 2.7), (mx(3.6, s), 7.7, 3.5), mat="bone_white")
    bar(rig, sn, side_name("shin_muscle", s), stifle, hock, 0.95, 0.85, mat="muscle_dark")
    bar(rig, sn, side_name("shin_fur", s), (mx(3.95, s), 7.0, 3.9), (mx(3.75, s), 4.8, 6.4), 0.55, 0.4, mat="fur_red", up=(s, 0, 0))
    bar(rig, sn, side_name("shin_raw", s), (mx(3.6, s), 6.2, 4.8), (mx(3.5, s), 4.6, 6.8), 0.3, 0.2, mat="muscle_raw", up=(s, 0, 0))
    bar(rig, hk, side_name("calcaneus", s), hock, (mx(2.8, s), 5.9, 9.6), 0.6, 0.6, mat="bone_white")
    bar(rig, hk, side_name("achilles", s), (mx(3.3, s), 6.8, 5.4), (mx(3.3, s), 5.8, 9.4), 0.2, 0.22, mat="muscle_dark")
    box(rig, hk, side_name("hock_joint", s), (mx(2.1, s), 3.2, 7.0), (mx(3.5, s), 4.6, 8.4), mat="fur_dark")
    for k, o in enumerate((-0.5, 0.5)):
        bar(rig, pb, side_name(f"metatarsal{k}", s), (mx(2.8 + o, s), 3.4, 7.4), (mx(2.8 + o, s), 1.05, 7.0), 0.4, 0.4, mat="fur_red")
    toe_group(rig, tb, s, "b", 6.95, mx(2.8, s), -1.0)


def part_limbs(rig: Rig) -> None:
    for s in (-1.0, 1.0):
        front_leg(rig, s)
        hind_leg(rig, s)


# ===========================================================================
# 7. 叉尾
# ===========================================================================
def part_tail(rig: Rig) -> None:
    """尾根第一节从臀部内侧（z≈6）接出，不留缝；两节尾根，第二节之后分成左右两股，每股 3 节渐细下垂；每股上缘一条生肌，尾尖一缕毛。"""
    bar(rig, "tail_1", "tail_1_fur", (0.0, 12.3, 6.0), (0.0, 11.4, 10.6), 1.3, 1.2, mat="fur_red", up=(1, 0, 0))
    bar(rig, "tail_1", "tail_1_core", (0.0, 12.1, 6.6), (0.0, 11.3, 10.4), 0.5, 0.5, mat="bone_white")
    bar(rig, "tail_2", "tail_2_fur", (0.0, 11.4, 10.6), (0.0, 11.0, 12.0), 1.05, 1.0, mat="fur_dark", up=(1, 0, 0))
    bar(rig, "tail_2", "tail_2_raw", (0.0, 12.15, 10.3), (0.0, 11.75, 11.9), 0.45, 0.2, mat="muscle_raw", up=(1, 0, 0))
    nodes = [((0.9, 11.0, 12.0), (1.6, 10.0, 14.0), 0.9), ((1.6, 10.0, 14.0), (2.4, 8.7, 16.0), 0.75), ((2.4, 8.7, 16.0), (3.2, 7.2, 17.8), 0.55)]
    for s in (-1.0, 1.0):
        for k, (a, b, r) in enumerate(nodes):
            bone = side_name(f"fork_{k + 1}", s)
            pa, pb = (mx(a[0], s), a[1], a[2]), (mx(b[0], s), b[1], b[2])
            bar(rig, bone, side_name(f"fork_fur{k}", s), pa, pb, r, r * 0.9, mat="fur_red" if k % 2 == 0 else "fur_dark", up=(0, 1, 0))
            mid = lerp3(pa, pb, 0.5)
            bar(rig, bone, side_name(f"fork_raw{k}", s), (pa[0], pa[1] + r * 0.9 + 0.12, pa[2]), (pb[0], pb[1] + r * 0.9 + 0.12, pb[2]), 0.28, 0.14,
                mat="muscle_raw", up=(1, 0, 0))
            if k == 2:
                bar(rig, bone, side_name("fork_wisp_a", s), (pb[0], pb[1], pb[2]), (mx(3.7, s), 5.4, 19.2), 0.45, 0.4, mat="fur_dark", up=(0, 1, 0))
                bar(rig, bone, side_name("fork_wisp_b", s), (mx(3.7, s), 5.4, 19.2), (mx(3.9, s), 4.0, 20.2), 0.3, 0.25, mat="fur_red", up=(0, 1, 0))
            del mid


PARTS = {
    "head": ("1. 粗颅厚吻 + 张口下颌 + 长獠牙 + 尖耳", part_head),
    "neck": ("2. 两节粗颈 + 颈鬃 + 剥开的颈侧", part_neck),
    "torso": ("3. 27 片肌肉桶 + 撕开的肋侧", part_torso),
    "strip": ("4. 背肌条：生肌 + 椎骨突起 + 斜纹肌束", part_back_strip),
    "fur": ("5. 毛：肩鬃 / 肋侧垂毛 / 腹毛 / 臀毛", part_fur),
    "scruff": ("5b. 肩背蓬毛：成片向后倒的毛片", part_scruff),
    "limbs": ("6. 四肢：肌肉块 + 4 趾 4 爪", part_limbs),
    "tail": ("7. 叉尾：两节尾根 + 左右各 3 节", part_tail),
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
# 软覆盖：毛 / 肌肉在撕口处层层相叠是造型意图；毛包着骨也一样。骨×骨、爪×任何件的互穿仍要报。
_LAYERS = ("fur_red", "fur_dark", "fur_pale", "muscle_raw", "muscle_dark")
SOFT_OVER = frozenset(frozenset((a, b)) for i, a in enumerate(_LAYERS) for b in _LAYERS[i + 1:])
SOFT_OVER |= frozenset(frozenset((f, "bone_white")) for f in _LAYERS)


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制，去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates(
    "剥皮豺 / bo_pi_chai",
    tuple(MATS),
    # 有意不对称：撕口 / 垂毛（侧骨）与左耳缺口
    asym=("fur_flank_l", "fur_flank_r", "fur_flank_b_l", "fur_flank_b_r", "fur_shoulder_l", "fur_shoulder_r", "ear_l", "ear_r"),
    soft_over=SOFT_OVER,
)


def self_test() -> None:
    print("运行 gen_bo_pi_chai.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    cubes = len(rig.elements)
    moving = moving_bones(rig)

    assert abs(lo[1]) < 0.12, f"贴地失败：最低点 y={lo[1]:.3f}，爪垫应落在 0 附近"
    assert 15.0 <= hi[1] <= 24.0, f"体高 {hi[1]:.1f}px 超出 15..24"
    length = hi[2] - lo[2]
    assert 46.0 <= length <= 62.0, f"吻尾纵长 {length:.1f}px 超出 46..62（躯干加长后）"
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
    "root": "根", "hips": "臀", "spine_mid": "后腰脊", "spine_lumbar": "前腰脊", "thorax": "胸脊", "neck_base": "颈根", "neck_upper": "颈上",
    "head": "头", "snout": "吻", "jaw": "下颌", "tail_1": "尾根 1", "tail_2": "尾根 2",
    "ear": "耳", "shoulder": "肩", "arm": "肱", "forearm": "桡", "paw_f": "前掌", "toes_f": "前趾",
    "thigh": "股", "shin": "胫", "hock": "跗", "paw_b": "后跖", "toes_b": "后趾",
    "fur_flank": "前肋侧毛", "fur_flank_b": "后肋侧毛", "fur_shoulder": "肩鬃", "fork_1": "叉尾 1", "fork_2": "叉尾 2", "fork_3": "叉尾 3",
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


# 比例量尺（返工：躯干太短）。参考图是头朝左的侧视，上半张是整只豺；肩前缘 / 臀后缘是人在参考图上点的像素列
# （肩前缘 = 胸前缘，在下颌线与前腿前缘交汇处；臀后缘 = 大腿肌肉后缘，在尾根下方），量尺线会画在 check.png 里。
REF_IMAGE = REVIEW_DIR / "reference.png"
REF_CROP = (0, 0, 1536, 520)
REF_TORSO_X = (430.0, 1060.0)
PREV_MODEL = REVIEW_DIR / "BoPiChai_prev.bbmodel"           # 上一版（躯干 13 单位）的 bbmodel，同取景对比用
PREV_TORSO_Z = (-6.2, 6.75)


def proportion_row(now_path: Path) -> Image.Image | None:
    """参考与模型 LEFT 侧视的两个比值（躯干长/肩高、头颈长/躯干长）和上一版对比；缺参考图时跳过。"""
    if not REF_IMAGE.exists():
        print("没有参考图，跳过比例量尺")
        return None
    return pc.side_ratio_row(load_font(14), REF_IMAGE, REF_CROP, REF_TORSO_X, now_path, (SLICE_Z0, SLICE_Z0 + SLICES - 1 + 0.95),
                             PREV_MODEL if PREV_MODEL.exists() else None, PREV_TORSO_Z)


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

    cols = [("参考图 / 爆炸图 EXPLODED REF（同一只豺）", ref),
            (f"NOW：{ROUND_TAG}（{len(rig.elements)} cube / {len(rig.bones)} 骨）", sheet_now),
            (f"骨骼树（{len(rig.bones)} 骨）", tree)]

    total_w = sum(c.width for _, c in cols) + 10 * (len(cols) + 1)
    ratios = proportion_row(now_path)
    canvas = Image.new("RGB", (max(total_w, ratios.width if ratios else 0), row_h + 48 + (ratios.height + 10 if ratios else 0)), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    x = 10
    for title, im in cols:
        canvas.paste(im, (x, 38))
        d.text((x + 4, 8), title, fill=(215, 205, 180), font=font_h)
        x += im.width + 10
    if ratios:
        canvas.paste(ratios, (0, row_h + 48))
    canvas.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png: {REVIEW_DIR / 'check.png'}")


def main() -> None:
    ap = argparse.ArgumentParser(description="剥皮豺 (bo_pi_chai) 生成器 Round 1/3")
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
        out = CREATURE_DIR / f"BoPiChai_{args.part}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(f"BoPiChai_{args.part}", namespace="bong"), ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"✓ {out}（{len(rig.elements)} cube）")
        return

    rig = build_full()
    doc = rig.bbmodel("BoPiChai", namespace="bong")
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
