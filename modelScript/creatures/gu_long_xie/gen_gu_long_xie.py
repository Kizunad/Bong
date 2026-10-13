#!/usr/bin/env python3
"""骨笼蟹（gu_long_xie）多足生物模型程序化生成器 —— Round 1/3。

依据 worldview.md §十三 鲸落遗骸 / 荒野：寄居在异变兽颅骨里的大寄居蟹。

外形（任务卡第五批 + 参考图 exploded.png）：
  · 颅骨壳：一层层体素椭球壳（顶部实心、下部中空），眼窝两个、鼻腔一个、两根獠牙（左边那根断了），
    后下方留一个大洞，露出里面卷着的蟹腹；壳面有裂纹、藤壶团和灰绿苔
  · 蟹身：铁锈橙的头胸甲探在颅骨前下沿，一对眼柄从眼窝里伸出来，两根长触须从壳下往前垂
  · 一只大螯（右）：三节臂 + 带瘤节的掌 + 带齿的定指和动指；一只小螯（左）：同构但细小
  · 四对三段步足：骨白甲节 + 锈橙关节 + 尖爪
  · 腹部三节卷在壳后，尾端一片尾扇

骨骼：颅骨壳 / 壳面附着 / 三束苔 + 头胸甲 / 腹 3 节 + 尾扇 / 眼柄 4 / 触须 6 / 口须 2 / 大小螯各 4 / 步足 24，
总数远超多足下限 30。

每个编号部件是一个 part_*()，可单独出预览。

用法：
  python3 modelScript/creatures/gu_long_xie/gen_gu_long_xie.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/gu_long_xie/gen_gu_long_xie.py --self-test # 门禁差分自证
  python3 modelScript/creatures/gu_long_xie/gen_gu_long_xie.py --list

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X（与髓蛀蜈同）。大螯在右（+X），小螯在左（-X）。
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "gu_long_xie"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/gu_long_xie")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "GuLongXie.bbmodel"
BBMODEL_COPY = MODELS_DIR / "GuLongXie.bbmodel"

ROUND_TAG = "Round 1/3"

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "bone_white": (222, 214, 196),    # 骨白
    "bone_grey": (156, 148, 134),     # 旧骨、阴面
    "crack_dark": (44, 38, 34),       # 裂纹、鼻腔、眼窝里的暗
    "rust_orange": (178, 96, 58),     # 蟹壳锈橙
    "rust_dark": (112, 58, 38),       # 蟹壳暗部、关节
    "moss_olive": (118, 124, 62),     # 灰绿苔
    "barnacle_grey": (150, 150, 146), # 藤壶
    "eye_black": (18, 16, 18),        # 眼
}
BONE_MATS = ("bone_white", "bone_grey")

# ---------------------------------------------------------------------------
# 体型参数（px；16 = 1 格）
# ---------------------------------------------------------------------------
MIN_MOVING_BONES = 30

# 颅骨壳：体素椭球壳，中心 SK_C，半轴 (RX, RY, RZ)，底面 y = SK_Y0 处截平
SK_C = (0.0, 22.0, 2.0)
SK_RX, SK_RY, SK_RZ = 13.0, 11.5, 14.0
SK_Y0 = 13.0
SK_STEP = 1.5
SK_WALL = 2.4                       # 环壁厚度
SK_SOLID_ABOVE = 24.0               # 此高度以上整层实心
SK_REAR_OPEN_BELOW = 20.5           # 此高度以下后壁敞开（露出蟹腹）
SOCKET_X = (3.0, 8.5)               # 眼窝 |x| 范围
SOCKET_Y = (18.0, 24.0)
NOSE_X = 1.8                        # 鼻腔半宽
NOSE_Y = (14.5, 17.5)

# 头胸甲
CARA_Y0, CARA_Y1 = 7.5, 13.0
CARA_Z0, CARA_Z1 = -4.0, 7.0
CARA_HALF_X = 5.6
CARA_RIM_X = 8.6

# 步足：四对，yaw 角（°）自前向后，负为朝前
LEG_PAIRS = 4
LEG_Z = (-2.5, 0.5, 3.5, 6.5)
LEG_YAW = (-30.0, -10.0, 10.0, 30.0)
LEG_HIP_X, LEG_HIP_Y = 8.0, 9.0
LEG_REACH = (11.0, 10.0, 5.0)        # 三段水平伸出量
LEG_RISE = 7.0                      # 膝比髋高

# 腹
AB_Z = (7.0, 11.5, 15.5, 19.0)      # 腹三节 + 尾扇的 z 边界


def side_tag(s: float) -> str:
    return "_l" if s < 0 else "_r"


def sx(p, s: float):
    return (p[0] * s, p[1], p[2])


def lerp3(a, b, t: float):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def along(a, b, d: float):
    """从 a 朝 b 走 d 个单位的点。"""
    return lerp3(a, b, d / math.dist(a, b))


# 大小螯（大螯在右 +X，小螯在左 -X；都不对称，骨名进 ASYM）
BIG = {
    "S": (6.5, 10.0, -5.6), "E": (15.0, 12.5, -10.5), "W": (18.0, 10.5, -19.0),
    "palm_c": (19.0, 10.0, -25.5), "palm_h": (6.0, 5.2, 6.5),
}
SMALL = {
    "S": (-6.5, 10.0, -5.6), "E": (-13.5, 11.0, -9.0), "W": (-16.0, 9.5, -13.5),
    "palm_c": (-16.0, 8.5, -17.5), "palm_h": (3.2, 3.0, 4.0),
}
LEG_JOINTS = ("a", "b", "c")


def leg_points(pair: int, s: float):
    """(髋, 膝, 踝, 爪尖)：膝抬高、踝落下、尖爪点地。"""
    a = math.radians(LEG_YAW[pair])
    u = (s * math.cos(a), 0.0, math.sin(a))
    h = (s * LEG_HIP_X, LEG_HIP_Y, LEG_Z[pair])
    k = (h[0] + u[0] * LEG_REACH[0], h[1] + LEG_RISE, h[2] + u[2] * LEG_REACH[0])
    an = (k[0] + u[0] * LEG_REACH[1], 4.8, k[2] + u[2] * LEG_REACH[1])
    t = (an[0] + u[0] * LEG_REACH[2], 0.55, an[2] + u[2] * LEG_REACH[2])
    return h, k, an, t


# ---------------------------------------------------------------------------
# 骨骼
# ---------------------------------------------------------------------------
def define_bones(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0), None)
    rig.bone("body", (0.0, 10.0, 1.5), "root")
    rig.bone("shell", (0.0, SK_Y0, SK_C[2]), "body")
    rig.bone("crust", (0.0, SK_C[1], SK_C[2]), "shell")
    rig.bone("moss_front", (0.0, SK_Y0, SK_C[2] - 8.0), "shell")
    rig.bone("moss_l", (-8.0, SK_Y0, SK_C[2]), "shell")
    rig.bone("moss_r", (8.0, SK_Y0, SK_C[2]), "shell")
    prev = "body"
    for i in range(3):
        prev = rig.bone(f"ab_{i}", (0.0, 11.0, AB_Z[i]), prev)
    rig.bone("tailfan", (0.0, 10.5, AB_Z[3]), "ab_2")
    for s in (-1.0, 1.0):
        t = side_tag(s)
        rig.bone(f"eye_a{t}", (s * 3.8, 13.0, -3.0), "body")
        rig.bone(f"eye_b{t}", (s * 5.0, 18.8, -4.4), f"eye_a{t}")
        rig.bone(f"mouth{t}", (s * 2.4, 8.6, -4.0), "body")
        for k in range(3):
            parent = "body" if k == 0 else f"ant_{k - 1}{t}"
            rig.bone(f"ant_{k}{t}", ANTENNA_PATH_PTS[2 * k] if s > 0 else sx(ANTENNA_PATH_PTS[2 * k], -1.0), parent)
        for p in range(LEG_PAIRS):
            h, k, an, _ = leg_points(p, s)
            rig.bone(f"leg{p}_a{t}", h, "body")
            rig.bone(f"leg{p}_b{t}", k, f"leg{p}_a{t}")
            rig.bone(f"leg{p}_c{t}", an, f"leg{p}_b{t}")
    for name, claw in (("claw_big", BIG), ("claw_small", SMALL)):
        rig.bone(f"{name}_arm", claw["S"], "body")
        rig.bone(f"{name}_fore", claw["E"], f"{name}_arm")
        rig.bone(f"{name}_palm", claw["W"], f"{name}_fore")
        pc = claw["palm_c"]
        ph = claw["palm_h"]
        rig.bone(f"{name}_dactyl", (pc[0], pc[1] + ph[1] * 0.75, pc[2] - ph[2] * 0.6), f"{name}_palm")


# 触须中心线折点（右侧；左侧取 x 反号）：从头胸甲前缘出来，贴着壳下沿往前探，再向外下垂
ANTENNA_PATH_PTS = [
    (2.0, 9.2, -4.0), (3.4, 9.6, -9.0), (4.6, 10.6, -14.0),
    (6.2, 11.4, -20.0), (8.0, 11.4, -26.0), (9.8, 10.6, -31.0), (11.2, 9.2, -35.0),
]


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


def taper(rig: Rig, bone: str, stem: str, pts: list[Vec], radii: list[float], *, mat: str) -> None:
    """沿折线铺一串逐段收细的柱（radii 长度 = pts 数；每段取两端平均）。"""
    for k in range(len(pts) - 1):
        r = (radii[k] + radii[k + 1]) / 2
        bar(rig, bone, f"{stem}{k}", pts[k], pts[k + 1], r, r, mat=mat)


# ===========================================================================
# 1. 颅骨壳：体素椭球壳 + 眼窝 + 鼻腔 + 獠牙
# ===========================================================================
def skull_layers() -> list[dict]:
    """自下而上的各层：y0/y1、半宽 w、半长 l、前缘 z、后缘 z。"""
    layers = []
    k = 0
    while True:
        y0 = SK_Y0 + SK_STEP * k
        ym = y0 + SK_STEP / 2
        dy = ym - SK_C[1]
        if abs(dy) >= SK_RY:
            break
        f = math.sqrt(1.0 - (dy / SK_RY) ** 2)
        w, l = SK_RX * f, SK_RZ * f
        layers.append({"k": k, "y0": y0, "y1": y0 + SK_STEP, "ym": ym, "w": w, "l": l,
                       "zf": SK_C[2] - l, "zb": SK_C[2] + l})
        k += 1
    return layers


SKULL = skull_layers()


def layer_mat(k: int) -> str:
    return "bone_grey" if k % 5 == 2 else "bone_white"


def part_skull(rig: Rig) -> None:
    """壳按层叠：y > SK_SOLID_ABOVE 的实心板；以下是四面墙的环；眼窝、鼻腔在前墙上留洞，后下方敞开。"""
    for L in SKULL:
        k, y0, y1, w, zf, zb = L["k"], L["y0"], L["y1"], L["w"], L["zf"], L["zb"]
        mat = layer_mat(k)
        if L["ym"] > SK_SOLID_ABOVE:
            box(rig, "shell", f"sk{k}_slab", (-w, y0, zf), (w, y1, zb), mat=mat)
            continue
        t = SK_WALL
        for s in (-1.0, 1.0):
            mbox(rig, "shell", f"sk{k}_wall", s, w - t, w, y0, y1, zf, zb, mat=mat)
        inner = w - t
        # 前墙：眼窝（两侧）与鼻腔（中线）挖空，剩下的分段铺：中段（中线件）+ 左右成对的外段
        if y0 < SOCKET_Y[1] and y1 > SOCKET_Y[0]:
            centre, side_x0 = (-SOCKET_X[0], SOCKET_X[0]), SOCKET_X[1]
        elif y0 >= NOSE_Y[0] and y1 <= NOSE_Y[1]:
            centre, side_x0 = None, NOSE_X
        else:
            centre, side_x0 = (-inner, inner), None
        if centre is not None:
            box(rig, "shell", f"sk{k}_front", (centre[0], y0, zf), (centre[1], y1, zf + t), mat=mat)
        if side_x0 is not None:
            for s in (-1.0, 1.0):
                mbox(rig, "shell", f"sk{k}_frontw", s, side_x0, inner, y0, y1, zf, zf + t, mat=mat)
        if L["ym"] >= SK_REAR_OPEN_BELOW:
            box(rig, "shell", f"sk{k}_back", (-inner, y0, zb - t), (inner, y1, zb), mat=mat)
    # 眼窝与鼻腔内壁的暗色衬：贴在前墙内面，让洞读成「深」
    for s in (-1.0, 1.0):
        L = next(L for L in SKULL if L["y0"] <= 20.0 < L["y1"])
        mbox(rig, "shell", "socket_back", s, SOCKET_X[0] + 0.4, SOCKET_X[1] - 0.4, SOCKET_Y[0] + 0.5, SOCKET_Y[1] - 0.5,
             L["zf"] + SK_WALL, L["zf"] + SK_WALL + 0.5, mat="crack_dark")
    # 獠牙：一对，每根三段收细；左边那根只剩半截，断口一块碎骨
    low = SKULL[0]
    zf = low["zf"]
    for s, parts in ((-1.0, 2), (1.0, 3)):
        x = s * 5.2
        pts = [(x, SK_Y0, zf + 1.2), (x + s * 0.1, SK_Y0 - 1.8, zf + 1.15), (x + s * 0.2, SK_Y0 - 3.6, zf + 1.1),
               (x + s * 0.3, SK_Y0 - 5.2, zf + 1.0)]
        radii = [1.5, 1.2, 0.8, 0.3]
        taper(rig, "shell", "fang" + ("_l" if s < 0 else "_r") + "_", pts[:parts + 1], radii[:parts + 1], mat="bone_white")
    # 小门牙：左右各两颗，长短不一
    for k, (x, ln) in enumerate(((1.0, 2.8), (3.0, 2.2))):
        for s in (-1.0, 1.0):
            mbox(rig, "shell", f"incisor{k}", s, x - 0.55, x + 0.55, SK_Y0 - ln, SK_Y0, zf + 0.9, zf + 1.9, mat="bone_white")
    # 眉弓：眼窝上缘一条凸骨，贴在前墙外面
    Lb = next(L for L in SKULL if L["y0"] <= 24.0 < L["y1"] + 0.01 or L["y0"] <= 23.9 < L["y1"])
    for s in (-1.0, 1.0):
        mbox(rig, "shell", "brow", s, 2.6, Lb["w"] - 1.0, Lb["y0"] - 0.2, Lb["y1"] + 0.6, Lb["zf"] - 1.0, Lb["zf"], mat="bone_grey")
    # 颧弓：眼窝下缘外侧一块凸骨
    for s in (-1.0, 1.0):
        mbox(rig, "shell", "cheek", s, 8.5, 10.0, 15.8, 18.0, SKULL[3]["zf"] + 0.2, SKULL[3]["zf"] + 2.6, mat="bone_grey")


# ===========================================================================
# 2. 壳面附着：裂纹 / 藤壶 / 苔
# ===========================================================================
def part_crust(rig: Rig) -> None:
    """裂纹、藤壶团、苔都是随机撒的，挂在 crust 骨下（镜像豁免）；三束垂苔对称，挂 moss_* 骨。"""
    rng = random.Random(20261013)
    side_layers = [L for L in SKULL if L["ym"] < SK_SOLID_ABOVE and L["ym"] > 14.0]
    # 裂纹：每条从某层往下 4~5 层，沿墙外面锯齿下行，每段 0.4 深
    for c in range(5):
        L0 = rng.choice(side_layers[2:])
        face = rng.choice(("l", "r", "f"))
        for n in range(4):
            L = SKULL[max(0, L0["k"] - n)]
            off = (0.5 if n % 2 else -0.5) + rng.uniform(-0.2, 0.2)
            if face == "f":
                x = off * 3.0 + (c - 2) * 2.0
                if abs(x) > L["w"] - SK_WALL - 0.5 or any(SOCKET_X[0] - 0.6 < abs(x) < SOCKET_X[1] + 0.6 for _ in (0,)) and SOCKET_Y[0] - 1 < L["ym"] < SOCKET_Y[1] + 1:
                    continue
                box(rig, "crust", f"crack{c}_{n}", (x - 0.35, L["y0"], L["zf"] - 0.4), (x + 0.35, L["y1"] + 0.4, L["zf"]), mat="crack_dark")
            else:
                s = -1.0 if face == "l" else 1.0
                z = SK_C[2] + off * 6.0
                x0, x1 = (L["w"], L["w"] + 0.4)
                lo, hi = (x0, x1) if s > 0 else (-x1, -x0)
                box(rig, "crust", f"crack{c}_{n}", (lo, L["y0"], z - 0.35), (hi, L["y1"] + 0.4, z + 0.35), mat="crack_dark")
    # 藤壶团：每团 3 枚，底座 + 孔
    for c in range(7):
        L = rng.choice(side_layers)
        face = rng.choice(("l", "r", "r", "l", "f"))
        for n in range(3):
            dz = rng.uniform(-2.0, 2.0)
            dy = rng.uniform(0.0, 1.0)
            y = L["y0"] + dy
            if face == "f":
                x = rng.uniform(-0.5, 0.5) * (L["w"] - SK_WALL - 1.0)
                if SOCKET_X[0] - 1 < abs(x) < SOCKET_X[1] + 1 and SOCKET_Y[0] - 1 < L["ym"] < SOCKET_Y[1] + 1:
                    continue
                box(rig, "crust", f"barn{c}_{n}", (x - 0.8, y, L["zf"] - 1.0), (x + 0.8, y + 1.4, L["zf"]), mat="barnacle_grey")
                box(rig, "crust", f"barn_hole{c}_{n}", (x - 0.4, y + 0.5, L["zf"] - 1.3), (x + 0.4, y + 0.9, L["zf"] - 1.0), mat="crack_dark")
            else:
                s = -1.0 if face == "l" else 1.0
                z = SK_C[2] + dz * 3.0
                x0, x1 = L["w"], L["w"] + 1.0
                lo, hi = (x0, x1) if s > 0 else (-x1, -x0)
                box(rig, "crust", f"barn{c}_{n}", (lo, y, z - 0.8), (hi, y + 1.4, z + 0.8), mat="barnacle_grey")
                x2 = L["w"] + 1.0
                lo2, hi2 = (x2, x2 + 0.3) if s > 0 else (-x2 - 0.3, -x2)
                box(rig, "crust", f"barn_hole{c}_{n}", (lo2, y + 0.5, z - 0.4), (hi2, y + 0.9, z + 0.4), mat="crack_dark")
    # 苔斑：薄片贴在外壁面
    for c in range(9):
        L = rng.choice(side_layers)
        face = rng.choice(("l", "r", "l", "r", "f"))
        wd, ht = rng.uniform(2.0, 4.5), rng.uniform(1.0, 1.4)
        if face == "f":
            x = rng.uniform(-0.6, 0.6) * (L["w"] - SK_WALL - 1.0)
            if SOCKET_X[0] - 1 < abs(x) < SOCKET_X[1] + 1 and SOCKET_Y[0] - 1 < L["ym"] < SOCKET_Y[1] + 1:
                continue
            box(rig, "crust", f"moss_patch{c}", (x - wd / 2, L["y0"], L["zf"] - 0.5), (x + wd / 2, L["y0"] + ht, L["zf"]), mat="moss_olive")
        else:
            s = -1.0 if face == "l" else 1.0
            z = SK_C[2] + rng.uniform(-0.6, 0.6) * L["l"]
            x0, x1 = L["w"], L["w"] + 0.5
            lo, hi = (x0, x1) if s > 0 else (-x1, -x0)
            box(rig, "crust", f"moss_patch{c}", (lo, L["y0"], z - wd / 2), (hi, L["y0"] + ht, z + wd / 2), mat="moss_olive")
    # 垂苔：从第 0 层底面往下挂，三束（前 / 左 / 右），每束三根逐渐变细
    low = SKULL[0]
    for k, dz in enumerate((-3.0, 0.0, 3.0)):
        ln = 1.8 - 0.2 * abs(k - 1)
        for s in (-1.0, 1.0):
            x0 = low["w"] - SK_WALL
            mbox(rig, f"moss_{'l' if s < 0 else 'r'}", f"moss_hang{k}", s, x0 + 0.2, x0 + 1.0, SK_Y0 - ln, SK_Y0, SK_C[2] + dz - 0.4, SK_C[2] + dz + 0.4, mat="moss_olive")
    for s in (-1.0, 1.0):
        mbox(rig, "moss_front", "moss_fh", s, 1.6, 2.4, SK_Y0 - 2.6, SK_Y0, low["zf"] + 0.4, low["zf"] + 1.4, mat="moss_olive")


# ===========================================================================
# 3. 蟹身：头胸甲 + 腹三节 + 尾扇 + 口须
# ===========================================================================
def part_body(rig: Rig) -> None:
    z0, z1 = CARA_Z0, CARA_Z1
    box(rig, "body", "cara_main", (-CARA_HALF_X, 9.0, z0), (CARA_HALF_X, CARA_Y1, z1), mat="rust_orange")
    box(rig, "body", "cara_front", (-CARA_HALF_X + 1.2, 8.4, z0 - 1.6), (CARA_HALF_X - 1.2, 12.0, z0), mat="rust_orange")
    box(rig, "body", "cara_belly", (-CARA_HALF_X + 0.4, CARA_Y0, z0), (CARA_HALF_X - 0.4, 9.0, z1), mat="rust_dark")
    for s in (-1.0, 1.0):
        mbox(rig, "body", "cara_rim", s, CARA_HALF_X, CARA_RIM_X, 7.8, 10.6, z0 + 0.5, z1 - 0.5, mat="rust_dark")
        mbox(rig, "body", "cara_rim_top", s, CARA_HALF_X, CARA_RIM_X - 0.8, 10.6, 11.4, z0 + 1.5, z1 - 1.5, mat="rust_orange")
        # 螯肩座：贴在甲缘前端，螯臂从它前面起
        mbox(rig, "body", "shoulder", s, CARA_HALF_X, CARA_RIM_X, 8.0, 12.0, z0 - 2.1, z0 + 0.5, mat="rust_dark")
        # 眼柄座：头胸甲前缘两个小丘
        mbox(rig, "body", "eye_pad", s, 2.6, 5.0, CARA_Y1, CARA_Y1 + 0.8, z0, z0 + 2.2, mat="rust_dark")
    # 腹三节：一节比一节细，往后微微上翘卷进壳里；每节一块背甲 + 一道暗缝
    sizes = ((4.2, 6.0), (3.8, 5.2), (3.2, 4.4))
    y_c = (11.0, 12.0, 13.4)
    for i in range(3):
        hx, hy = sizes[i]
        za, zb = AB_Z[i], AB_Z[i + 1]
        box(rig, f"ab_{i}", f"ab{i}_core", (-hx, y_c[i] - hy / 2, za), (hx, y_c[i] + hy / 2, zb - 0.8), mat="rust_orange")
        box(rig, f"ab_{i}", f"ab{i}_seam", (-hx + 0.4, y_c[i] - hy / 2 + 0.4, zb - 0.8), (hx - 0.4, y_c[i] + hy / 2 - 0.4, zb), mat="rust_dark")
        for s in (-1.0, 1.0):
            mbox(rig, f"ab_{i}", f"ab{i}_plate", s, hx, hx + 0.8, y_c[i] - hy / 2 + 0.6, y_c[i] + hy / 2 - 0.6, za + 0.3, zb - 1.0, mat="rust_dark")
    # 尾扇：中叶 + 两侧叶
    box(rig, "tailfan", "tailfan_c", (-1.4, 12.2, AB_Z[3] - 0.0), (1.4, 13.0, AB_Z[3] + 3.4), mat="rust_orange")
    for s in (-1.0, 1.0):
        mbox(rig, "tailfan", "tailfan_s", s, 1.4, 3.6, 12.2, 12.9, AB_Z[3] - 0.0, AB_Z[3] + 2.6, mat="rust_dark")
    # 口须：头胸甲前下缘左右各一束
    for s in (-1.0, 1.0):
        bar(rig, f"mouth{side_tag(s)}", f"mouthpart{side_tag(s)}", (s * 2.4, 8.6, z0), (s * 3.0, 7.2, z0 - 3.0), 0.6, 0.6, mat="rust_dark")


# ===========================================================================
# 4. 眼柄 + 触须
# ===========================================================================
def part_eyes(rig: Rig) -> None:
    """眼柄两段从头胸甲前缘斜上，穿过眼窝，末端一颗黑眼；触须三节，贴壳下沿往前探再垂下。"""
    for s in (-1.0, 1.0):
        t = side_tag(s)
        a0, a1, a2 = (s * 3.8, 13.0, -3.0), (s * 5.0, 18.8, -4.4), (s * 5.75, 20.6, -13.5)
        bar(rig, f"eye_a{t}", f"stalk_a{t}", a0, a1, 0.9, 0.9, mat="rust_orange")
        bar(rig, f"eye_b{t}", f"stalk_b{t}", a1, a2, 0.8, 0.8, mat="rust_orange")
        mbox(rig, f"eye_b{t}", "eyeball", s, 4.35, 7.15, 19.8, 22.6, -16.0, -13.5, mat="eye_black")
        # 触须：三骨各管两个折点
        pts = [sx(p, s) for p in ANTENNA_PATH_PTS]
        for k in range(3):
            for j in range(2):
                a, b = pts[2 * k + j], pts[2 * k + j + 1]
                if j == 0 and k > 0:
                    a = along(a, b, 0.4)
                r = 0.55 - 0.07 * (2 * k + j)
                bar(rig, f"ant_{k}{t}", f"ant{2 * k + j}{t}", a, b, r, r, mat="rust_dark" if (2 * k + j) % 2 else "rust_orange")


# ===========================================================================
# 5. 螯：大螯（右） / 小螯（左）
# ===========================================================================
def claw(rig: Rig, name: str, c: dict, big: bool) -> None:
    S, E, W = c["S"], c["E"], c["W"]
    pc, ph = c["palm_c"], c["palm_h"]
    arm_r = 2.8 if big else 1.4
    s = 1.0 if pc[0] > 0 else -1.0
    arm, fore, palm, dact = f"{name}_arm", f"{name}_fore", f"{name}_palm", f"{name}_dactyl"
    bar(rig, arm, f"{name}_upper", S, along(E, S, arm_r * 0.9), arm_r, arm_r, mat="bone_white")
    kn = arm_r * 0.95
    box(rig, fore, f"{name}_elbow", (E[0] - kn, E[1] - kn, E[2] - kn), (E[0] + kn, E[1] + kn, E[2] + kn), mat="rust_dark")
    bar(rig, fore, f"{name}_carpus", along(E, W, kn), W, arm_r * 1.15, arm_r * 1.15, mat="bone_grey")
    if big:
        for k, tt in enumerate((0.35, 0.65)):
            p = lerp3(E, W, tt)
            r = arm_r * 1.15
            box(rig, fore, f"{name}_knob{k}", (p[0] - 0.9, p[1] + r, p[2] - 0.9), (p[0] + 0.9, p[1] + r + 1.0, p[2] + 0.9), mat="bone_white")
    # 掌：三层叠成鼓肚的块 + 背脊上几粒瘤
    x0, x1 = pc[0] - ph[0], pc[0] + ph[0]
    y0, y1 = pc[1] - ph[1], pc[1] + ph[1]
    z0, z1 = pc[2] - ph[2], pc[2] + ph[2]
    inset = 0.9 if big else 0.5
    box(rig, palm, f"{name}_palm_core", (x0, y0 + inset, z0 + 0.0), (x1, y1 - inset, z1), mat="bone_white")
    box(rig, palm, f"{name}_palm_top", (x0 + inset, y1 - inset, z0 + 0.6), (x1 - inset, y1, z1 - 0.6), mat="bone_grey")
    box(rig, palm, f"{name}_palm_low", (x0 + inset, y0, z0 + 0.6), (x1 - inset, y0 + inset, z1 - 0.6), mat="bone_grey")
    nk = 5 if big else 3
    for k in range(nk):
        zk = z0 + 1.2 + (z1 - z0 - 2.4) * k / max(nk - 1, 1)
        kw = 1.0 if big else 0.6
        box(rig, palm, f"{name}_bump{k}", (pc[0] - kw, y1, zk - kw), (pc[0] + kw, y1 + kw * 1.2, zk + kw), mat="bone_white")
    # 定指（固定，随掌）：三段收细 + 内缘齿
    fz = z1
    f0 = (pc[0], y0 + (ph[1] * 0.45 if big else ph[1] * 0.5), fz)
    f_pts = [f0,
             (pc[0] - s * 0.2, f0[1] - 0.4, fz - (4.5 if big else 3.5)),
             (pc[0] - s * 0.8, f0[1] - 0.9, fz - (8.5 if big else 6.5)),
             (pc[0] - s * 1.5, f0[1] - 1.0, fz - (11.5 if big else 8.5))]
    f_pts = [(p[0], p[1], p[2] - 11.0 + 11.0) for p in f_pts]
    # 掌朝前是 -z：手指向 -z 延伸，所以从掌前缘 z0 开始，不是 z1
    fz = z0
    f0 = (pc[0], pc[1] - ph[1] * (0.45 if big else 0.4), fz)
    L1, L2, L3 = (6.5, 12.0, 16.5) if big else (4.5, 8.0, 10.5)
    f_pts = [f0, (f0[0] - s * 0.2, f0[1] - 0.4, fz - L1), (f0[0] - s * 0.8, f0[1] - 0.9, fz - L2), (f0[0] - s * 1.5, f0[1] - 1.0, fz - L3)]
    radii = (2.6, 2.0, 1.3, 0.45) if big else (1.3, 1.0, 0.65, 0.25)
    taper(rig, palm, f"{name}_fixed", f_pts, list(radii), mat="bone_white")
    for k, tt in enumerate((0.3, 0.55, 0.8)):
        p = lerp3(f_pts[0], f_pts[3], tt)
        r = (2.6 - 1.7 * tt) * (1.0 if big else 0.5)
        box(rig, palm, f"{name}_ftooth{k}", (p[0] - 0.4, p[1] + r * 0.6, p[2] - 0.5), (p[0] + 0.4, p[1] + r * 0.6 + 0.9, p[2] + 0.5), mat="bone_grey")
    # 动指（挂 dactyl 骨）：从掌上前缘铰出，弯向定指尖
    d0 = (pc[0], pc[1] + ph[1] * (0.62 if big else 0.55), z0 + 0.2)
    d_pts = [d0, (d0[0] - s * 0.1, d0[1] - 0.3, z0 - L1 * 0.95), (d0[0] - s * 0.6, d0[1] - 1.4, z0 - L2 * 0.95),
             (f_pts[3][0] + s * 0.0, f_pts[3][1] + 0.9, f_pts[3][2] - 0.2)]
    taper(rig, dact, f"{name}_dact", d_pts, list(radii), mat="bone_white")
    for k, tt in enumerate((0.3, 0.55, 0.8)):
        p = lerp3(d_pts[0], d_pts[3], tt)
        r = (2.6 - 1.7 * tt) * (1.0 if big else 0.5)
        box(rig, dact, f"{name}_dtooth{k}", (p[0] - 0.4, p[1] - r * 0.6 - 0.9, p[2] - 0.5), (p[0] + 0.4, p[1] - r * 0.6, p[2] + 0.5), mat="bone_grey")


def part_claw_big(rig: Rig) -> None:
    claw(rig, "claw_big", BIG, True)
    rng = random.Random(55)
    pc, ph = BIG["palm_c"], BIG["palm_h"]
    # 大螯掌上：藤壶 4 枚 + 苔 2 片（贴外侧面 / 顶面），挂掌骨（右侧螯本来不对称，掌骨在 ASYM 里）
    for n in range(4):
        z = pc[2] + rng.uniform(-3.5, 3.5)
        y = pc[1] + rng.uniform(-2.0, 2.0)
        box(rig, "claw_big_palm", f"claw_barn{n}", (pc[0] + ph[0], y, z - 0.8), (pc[0] + ph[0] + 1.0, y + 1.4, z + 0.8), mat="barnacle_grey")
    for n in range(2):
        z = pc[2] + rng.uniform(-3.0, 3.0)
        box(rig, "claw_big_palm", f"claw_moss{n}", (pc[0] + ph[0], pc[1] - 3.0 + n * 3.5, z - 1.3), (pc[0] + ph[0] + 0.5, pc[1] - 2.0 + n * 3.5, z + 1.3), mat="moss_olive")


def part_claw_small(rig: Rig) -> None:
    claw(rig, "claw_small", SMALL, False)


# ===========================================================================
# 6. 步足：四对，每腿三段 + 尖爪
# ===========================================================================
def part_legs(rig: Rig) -> None:
    for p in range(LEG_PAIRS):
        for s in (-1.0, 1.0):
            t = side_tag(s)
            h, k, a, tip = leg_points(p, s)
            la, lb, lc = (f"leg{p}_{j}{t}" for j in LEG_JOINTS)
            # 股：髋座 + 粗骨干，膝处缩进，膝块（锈橙）挂 b 骨
            box(rig, la, f"coxa{p}{t}", (h[0] - 2.0, h[1] - 2.0, h[2] - 1.4), (h[0] + 2.0, h[1] + 2.0, h[2] + 1.4), mat="rust_dark")
            bar(rig, la, f"femur{p}{t}", along(h, k, 1.8), along(k, h, 2.0), 1.9, 1.9, mat="bone_white")
            box(rig, lb, f"knee{p}{t}", (k[0] - 2.0, k[1] - 2.0, k[2] - 2.0), (k[0] + 2.0, k[1] + 2.0, k[2] + 2.0), mat="rust_orange")
            # 胫：长、略细，背面三根小棘
            bar(rig, lb, f"tibia{p}{t}", along(k, a, 2.2), along(a, k, 1.7), 1.5, 1.5, mat="bone_white")
            for n, tt in enumerate((0.3, 0.55, 0.8)):
                c = lerp3(k, a, tt)
                box(rig, lb, f"spike{p}_{n}{t}", (c[0] - 0.4, c[1] + 1.5, c[2] - 0.4), (c[0] + 0.4, c[1] + 2.6, c[2] + 0.4), mat="bone_grey")
            box(rig, lc, f"ankle{p}{t}", (a[0] - 1.5, a[1] - 1.5, a[2] - 1.5), (a[0] + 1.5, a[1] + 1.5, a[2] + 1.5), mat="rust_dark")
            mid = lerp3(a, tip, 0.55)
            bar(rig, lc, f"tarsus{p}{t}", along(a, tip, 1.5), mid, 1.1, 1.1, mat="bone_grey")
            bar(rig, lc, f"claw{p}{t}", mid, tip, 0.7, 0.7, mat="rust_dark")


PARTS = {
    "skull": ("1. 颅骨壳：体素椭球壳 / 眼窝 / 鼻腔 / 獠牙", part_skull),
    "crust": ("2. 壳面：裂纹 / 藤壶团 / 苔", part_crust),
    "body": ("3. 头胸甲 + 腹三节 + 尾扇 + 口须", part_body),
    "eyes": ("4. 眼柄 + 触须", part_eyes),
    "claw_big": ("5. 大螯（右）", part_claw_big),
    "claw_small": ("6. 小螯（左）", part_claw_small),
    "legs": ("7. 四对三段步足", part_legs),
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
# 镜像门只认 _l/_r 成对。这些骨本来就不左右对称：大小螯一大一小；壳面附着随机撒。
ASYM = ("crust",) + tuple(f"{n}_{p}" for n in ("claw_big", "claw_small") for p in ("arm", "fore", "palm", "dactyl"))

# 穿模门按 AABB 判：斜置的骨干和壳、关节块的 AABB 会虚报互穿。骨白 × 骨灰、锈橙 × 锈暗这类同族配对放行。
SOFT_OVER = frozenset({
    frozenset(("bone_white", "bone_grey")), frozenset(("rust_orange", "rust_dark")),
    frozenset(("bone_white", "barnacle_grey")), frozenset(("bone_grey", "barnacle_grey")),
    frozenset(("bone_white", "moss_olive")), frozenset(("bone_grey", "moss_olive")),
    frozenset(("bone_white", "crack_dark")), frozenset(("bone_grey", "crack_dark")),
    # 关节块（锈橙 / 锈暗）套在骨白骨干端头，斜置骨干的 AABB 必然与它相交
    frozenset(("bone_white", "rust_orange")), frozenset(("bone_white", "rust_dark")),
    frozenset(("bone_grey", "rust_orange")), frozenset(("bone_grey", "rust_dark")),
})


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制，去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates("骨笼蟹 / gu_long_xie", tuple(MATS), asym=ASYM, soft_over=SOFT_OVER)


def self_test() -> None:
    print("运行 gen_gu_long_xie.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    moving = moving_bones(rig)
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert abs(lo[1]) < 0.15, f"贴地失败：最低点 y={lo[1]:.3f}，爪尖应落在 0 附近"
    print(f"  [OK] 可动骨 {len(moving)} cube {len(rig.elements)} 宽 {hi[0] - lo[0]:.1f}px 长 {hi[2] - lo[2]:.1f}px 高 {hi[1]:.1f}px")
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
    return [
        ("头吻面 FRONT：相机在 -Z", 180.0, 0.0, whole),
        ("尾臀面 REAR：相机在 +Z", 0.0, 0.0, whole),
        ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0, whole),
        ("左侧面 LEFT：相机在 -X，头朝左", 90.0, 0.0, whole),
        ("右侧面 RIGHT：相机在 +X，头朝右", -90.0, 0.0, whole),
        ("俯视 TOP：相机在 +Y，头朝上", 0.0, 89.9, whole),
    ]


FOCUS = {
    "skull": ((0.0, 22.0, 0.0), 40.0),
    "crust": ((0.0, 22.0, 0.0), 40.0),
    "body": ((0.0, 11.0, 6.0), 36.0),
    "eyes": ((0.0, 15.0, -14.0), 44.0),
    "claw_big": ((15.0, 9.0, -14.0), 40.0),
    "claw_small": ((-12.0, 9.0, -12.0), 36.0),
    "legs": ((0.0, 8.0, 2.0), 66.0),
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
            ("俯视 TOP：相机在 +Y", 0.0, 89.9) if key in ("legs", "body") else ("左侧面 LEFT：相机在 -X", 90.0, 0.0),
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
        d.rectangle([px, py, px + 340, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def bone_tree_lines(rig: Rig) -> list[str]:
    """骨骼树：右侧完整展开，左侧同构的腿 / 眼 / 触须只写一行。"""
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines: list[str] = []

    def count_desc(name: str) -> int:
        return sum(1 + count_desc(k) for k in children.get(name, []))

    def walk(name: str, depth: int) -> None:
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{name} ({len(rig.bones[name]['children'])})")
        if name.endswith("_l") and name.startswith(("leg", "eye_a", "ant_0")):
            if count_desc(name):
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
    ap = argparse.ArgumentParser(description="骨笼蟹 (gu_long_xie) 生成器 Round 1/3")
    ap.add_argument("--self-test", action="store_true", help="门禁差分自证")
    ap.add_argument("--list", action="store_true", help="列出编号部件")
    ap.add_argument("--no-render", action="store_true", help="只写 bbmodel，不出预览图")
    args = ap.parse_args()

    if args.list:
        for key, (title, _) in PARTS.items():
            print(f"  {key:11s} {title}")
        return
    if args.self_test:
        self_test()
        return

    self_test()
    rig = build_full()
    doc = rig.bbmodel("GuLongXie", namespace="bong")
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
