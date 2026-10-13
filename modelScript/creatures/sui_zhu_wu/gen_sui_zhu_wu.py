#!/usr/bin/env python3
"""髓蛀蜈（sui_zhu_wu）多足生物模型程序化生成器 —— Round 2/3。

依据 worldview.md §十三 鲸落遗骸：钻进鲸落化石里啃骨髓的长节蜈蚣。

外形（任务卡 + 参考图 exploded.png）：
  · 16 节骨白带灰黄的长体，每节一块拱形背甲，顶上有骨质棱脊，背甲上有裂纹
  · 每节一对三段腿（股 / 胫 / 跗 + 爪），左右各一，共 32 条腿
  · 头：宽头甲 + 六圈旋转骨钻（齿环由大到小，相邻环的齿向反向歪斜）+ 两条三节长触须
  · 尾：末节后接一对钳状尾肢（每只两节，内缘带齿）
  · 身上沾的白色骨屑：每节单独一根 dust_N 骨，左右不对称，让镜像门继续盯着体节本身

骨骼：体节 1 骨 + 每腿 3 骨 + 每节骨屑 1 骨，加头 / 钻环 / 触须 / 尾钳，总数远超多足下限 40。

每个编号部件是一个 part_*()，可单独出预览。

用法：
  python3 modelScript/creatures/sui_zhu_wu/gen_sui_zhu_wu.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/sui_zhu_wu/gen_sui_zhu_wu.py --self-test # 门禁差分自证
  python3 modelScript/creatures/sui_zhu_wu/gen_sui_zhu_wu.py --list

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X（与枯皮狼同）。
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "sui_zhu_wu"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/sui_zhu_wu")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "SuiZhuWu.bbmodel"
BBMODEL_COPY = MODELS_DIR / "SuiZhuWu.bbmodel"

ROUND_TAG = "Round 2/3"

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "bone_white": (226, 220, 205),   # 骨白
    "bone_ochre": (188, 168, 118),   # 灰黄，部分体节与关节
    "bone_grey": (140, 134, 120),    # 侧甲、体芯
    "bone_dark": (52, 46, 38),       # 节间缝、裂纹、腹板
    "dust_white": (245, 243, 236),   # 白色骨屑
    "rust_brown": (120, 86, 52),     # 爪尖、关节锈迹
    "tooth_ivory": (238, 228, 200),  # 钻齿
    "marrow_amber": (200, 150, 70),  # 沾着的骨髓渍
}

# ---------------------------------------------------------------------------
# 体型参数
# ---------------------------------------------------------------------------
N_SEG = 16
SEG_PITCH = 3.0
SEG_Z0 = -10.0                      # 第 0 节中心 z
Y_C = 5.3                           # 体轴高度
# 各节半宽：头端细、中段最宽、尾端收细
WIDTHS = [2.7, 3.3, 3.8, 4.1, 4.3, 4.4, 4.4, 4.4, 4.3, 4.2, 4.0, 3.7, 3.4, 3.1, 2.8, 2.5]
MIN_MOVING_BONES = 40
MIN_SEGMENTS = 14


def seg_z(i: int) -> float:
    return SEG_Z0 + SEG_PITCH * i


def seg_half_h(i: int) -> float:
    return WIDTHS[i] * 0.6


# ---------------------------------------------------------------------------
# 骨骼
# ---------------------------------------------------------------------------
LEG_JOINTS = ("a", "b", "c")        # 股 / 胫 / 跗
RING_COUNT = 6
HEAD_Z = -13.0                      # 头甲中心 z
RING_Z0 = -15.6                     # 第 0 圈钻环中心 z
RING_PITCH = 2.1                    # 环间距；环体厚度 ≈ 间距，六圈连成一个圆锥钻头
RING_R0 = 4.2                       # 第 0 圈半径（比头甲半宽略大，钻头从头前凸出来）
RING_R_STEP = 0.7                   # 每圈收细量，第 5 圈半径 0.7，收成钻尖
TAIL_Z = seg_z(N_SEG - 1) + 3.2     # 尾节中心 z
ANTENNA_BONES = 3


def side_name(base: str, s: float) -> str:
    return base + ("_l" if s < 0 else "_r")


def leg_points(i: int, s: float) -> dict[str, tuple[float, float, float]]:
    """第 i 节 s 侧腿的关节点：股向外上方拱起，胫向外下弯成弧，跗向下收成尖爪。

    相邻腿前后错开 0.5（sway），读出步态感。
    """
    w, z = WIDTHS[i], seg_z(i)
    sway = 0.5 if i % 2 == 0 else -0.5
    return {
        "hip": (s * (w + 1.3), Y_C - 0.4, z),
        "knee": (s * (w + 3.4), Y_C + 2.3, z + sway),
        "mid": (s * (w + 5.2), Y_C + 1.6, z + sway * 1.2),
        "ankle": (s * (w + 6.2), 3.2, z + sway * 1.4),
        "toe": (s * (w + 6.8), 1.0, z + sway * 1.6),
        "tip": (s * (w + 7.0), 0.15, z + sway * 1.6 - 0.2),
    }


def define_bones(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0), None)
    # 体节链：头端到尾端，后一节是前一节的子骨
    prev = "root"
    for i in range(N_SEG):
        name = f"seg_{i}"
        rig.bone(name, (0.0, Y_C, seg_z(i)), prev)
        prev = name
    for i in range(N_SEG):
        z, w = seg_z(i), WIDTHS[i]
        rig.bone(f"dust_{i}", (0.0, Y_C + seg_half_h(i), z), f"seg_{i}")
        for s in (-1.0, 1.0):
            pts = leg_points(i, s)
            rig.bone(side_name(f"leg_{i}_a", s), pts["hip"], f"seg_{i}")
            rig.bone(side_name(f"leg_{i}_b", s), pts["knee"], side_name(f"leg_{i}_a", s))
            rig.bone(side_name(f"leg_{i}_c", s), pts["ankle"], side_name(f"leg_{i}_b", s))
    # 头与口器
    rig.bone("head", (0.0, Y_C, HEAD_Z), "seg_0")
    for k in range(RING_COUNT):
        rig.bone(f"ring_{k}", (0.0, Y_C, RING_Z0 - RING_PITCH * k), "head")
    for s in (-1.0, 1.0):
        for k in range(1, ANTENNA_BONES + 1):
            x0, y0, z0 = ANTENNA_PATH[(k - 1) * 2]
            parent = "head" if k == 1 else side_name(f"antenna_{k - 1}", s)
            rig.bone(side_name(f"antenna_{k}", s), (s * x0, y0, z0), parent)
    # 尾节与钳
    rig.bone("telson", (0.0, Y_C - 0.4, TAIL_Z), f"seg_{N_SEG - 1}")
    for s in (-1.0, 1.0):
        rig.bone(side_name("pincer_base", s), (s * 1.4, Y_C - 0.6, TAIL_Z + 1.4), "telson")
        rig.bone(side_name("pincer_tip", s), (s * 4.2, Y_C - 1.0, TAIL_Z + 6.2), side_name("pincer_base", s))


# 触须中心线折点（右侧 x 为正；左侧取负）。六段，每骨管两段。
ANTENNA_PATH = [
    (3.0, 7.6, HEAD_Z - 0.6),
    (6.0, 9.0, HEAD_Z - 1.9),
    (7.4, 9.4, HEAD_Z - 7.0),
    (8.2, 8.0, HEAD_Z - 10.4),
    (9.8, 6.0, HEAD_Z - 13.2),
    (11.2, 4.0, HEAD_Z - 15.4),
    (12.4, 2.2, HEAD_Z - 17.0),
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
        mat: str, up: Vec | None = None, extend: float = 0.0) -> None:
    """端点精确落在 a、b 的定向柱。up 给定时局部 +Z（半厚 rz）指向 up。"""
    frm, to, rot, org = shaft_box(a, b, rx, rz, extend, up)
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


def box(rig: Rig, bone: str, name: str, lo: Vec, hi: Vec, *, mat: str) -> None:
    rig.cube(bone, name, lo, hi, mat=mat)


def mx(x: float, s: float) -> float:
    return x * s


def along(a: Vec, b: Vec, d: float) -> Vec:
    """从 a 朝 b 走 d 个单位的点。用来让相邻两骨的骨干在关节处各自缩进，不互相咬。"""
    length = math.dist(a, b)
    t = d / length
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


# 背甲拱：中心线折点 (x 占半宽比例, y 相对 h 的比例)；甲板半厚见 PLATE_HALF_T
ARCH = [(1.0, 0.2), (0.55, 0.86), (0.2, 1.0)]
PLATE_HALF_T = 0.5
PLATE_HALF_Z = 1.15


def arch_surface_y(i: int, x: float) -> float:
    """第 i 节背甲在横向位置 x 处的外表面高度，骨屑和裂纹贴着它放。"""
    w, h = WIDTHS[i], seg_half_h(i)
    ax = abs(x) / w
    pts = [(0.0, 1.0)] + [(px, py) for px, py in reversed(ARCH)]   # 从中线到肩
    pts = [(0.0, 1.0), (0.2, 1.0), (0.55, 0.86), (1.0, 0.2)]
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        if x0 <= ax <= x1:
            t = (ax - x0) / (x1 - x0) if x1 > x0 else 0.0
            return Y_C + h * (y0 + (y1 - y0) * t) + PLATE_HALF_T
    return Y_C + h * 0.2 + PLATE_HALF_T


# ===========================================================================
# 1. 头：宽头甲 + 六圈逆向骨钻
# ===========================================================================
def part_head(rig: Rig) -> None:
    """头甲 + 旋转骨钻。钻环 0 最大（紧贴头甲），到环 5 收成钻尖。

    相邻两圈的齿向相反方向歪斜（逆向旋转感）；偶数圈象牙色，奇数圈沾髓渍。
    """
    w, h = 3.4, 2.2
    z = HEAD_Z
    # 头甲：拱形背板 + 暗色体芯 + 腹板
    for s in (-1.0, 1.0):
        bar(rig, "head", f"head_plate_shoulder_{'l' if s < 0 else 'r'}", (mx(w, s), Y_C + 0.2 * h, z), (mx(0.55 * w, s), Y_C + 0.86 * h, z),
            0.6, 1.6, mat="bone_ochre", up=(0, 0, 1))
        bar(rig, "head", f"head_plate_crown_{'l' if s < 0 else 'r'}", (mx(0.55 * w, s), Y_C + 0.86 * h, z), (mx(0.2 * w, s), Y_C + h, z),
            0.6, 1.6, mat="bone_white", up=(0, 0, 1))
        bar(rig, "head", f"head_plate_side_{'l' if s < 0 else 'r'}", (mx(w, s), Y_C + 0.2 * h, z), (mx(w - 0.25, s), Y_C - 0.8 * h, z),
            0.45, 1.4, mat="bone_grey", up=(0, 0, 1))
        # 头甲两侧的感觉窝（暗色小坑），不是眼
        box(rig, "head", f"head_pit_{'l' if s < 0 else 'r'}", (mx(w + 0.05, s), Y_C + 0.1, z - 0.9), (mx(w + 0.5, s), Y_C + 0.8, z - 0.1),
            mat="bone_dark")
    bar(rig, "head", "head_plate_top", (-0.2 * w, Y_C + h, z), (0.2 * w, Y_C + h, z), 0.6, 1.6, mat="bone_white", up=(0, 0, 1))
    box(rig, "head", "head_core", (-w + 0.5, Y_C - 0.8 * h, z - 1.45), (w - 0.5, Y_C + 0.8 * h, z + 1.6), mat="bone_grey")
    box(rig, "head", "head_belly", (-0.6 * w, Y_C - h, z - 1.4), (0.6 * w, Y_C - 0.8 * h, z + 1.2), mat="bone_dark")
    # 头甲顶的两道棱
    box(rig, "head", "head_crest", (-0.3, Y_C + h + 0.5, z - 1.4), (0.3, Y_C + h + 1.3, z + 1.2), mat="bone_white")
    # 钻环
    for k in range(RING_COUNT):
        _ring(rig, k)
    # 钻尖
    zt = RING_Z0 - RING_PITCH * (RING_COUNT - 1)
    bar(rig, "ring_5", "drill_point", (0.0, Y_C, zt - 0.9), (0.0, Y_C, zt - 4.2), 0.4, 0.4, mat="tooth_ivory")


def _ring(rig: Rig, k: int) -> None:
    """一圈钻环。环体厚度接近环间距，六圈首尾相接成一个圆锥；齿向前、沿切向扭出去。"""
    radius = RING_R0 - RING_R_STEP * k
    zk = RING_Z0 - RING_PITCH * k
    bone = f"ring_{k}"
    mat_core = "marrow_amber" if k % 2 else "bone_white"      # 奇数圈的环体沾着髓渍
    mat_tooth = "tooth_ivory"
    half = radius * 0.72
    half_z = RING_PITCH / 2 - 0.6     # 留出环间空隙，齿在空隙里不碰下一圈（门禁按 AABB 判互穿）
    # 环体：方块 + 转 45° 的方块叠成八边形
    box(rig, bone, f"ring{k}_core_a", (-half, Y_C - half, zk - half_z), (half, Y_C + half, zk + half_z), mat=mat_core)
    rig.cube(bone, f"ring{k}_core_b", (-half, Y_C - half, zk - half_z), (half, Y_C + half, zk + half_z),
             rot=(0.0, 0.0, 45.0), org=(0.0, Y_C, zk), mat=mat_core)
    # 大圈 12 枚齿、小圈 8 枚；相邻两圈扭向相反（逆向旋转感）。齿尖朝前（-z）伸出一整个环距，
    # 前视就是一圈圈放射的锥形尖刺。
    twist = 0.35 * (1 if k % 2 == 0 else -1)
    count = 12 if k < 3 else 8
    for j in range(count):
        theta = math.radians(360.0 / count * j + (15.0 if k % 2 else 0.0))
        r_in, r_out = radius * 1.0, radius + 0.7
        a = (r_in * math.cos(theta), Y_C + r_in * math.sin(theta), zk - half_z + 0.3)
        b = (r_out * math.cos(theta + twist), Y_C + r_out * math.sin(theta + twist), zk - half_z - 1.1)
        bar(rig, bone, f"tooth_{k}_{j}", a, b, 0.32, 0.32, mat=mat_tooth)


# ===========================================================================
# 2. 触须
# ===========================================================================
def part_antennae(rig: Rig) -> None:
    """左右各一条三节长触须：6 段逐渐收细，每个关节一粒骨节。"""
    radii = [0.5, 0.45, 0.4, 0.34, 0.28, 0.22]
    for s in (-1.0, 1.0):
        for seg in range(6):
            bone = side_name(f"antenna_{seg // 2 + 1}", s)
            ax, ay, az = ANTENNA_PATH[seg]
            bx, by, bz = ANTENNA_PATH[seg + 1]
            start = (mx(ax, s), ay, az)
            if seg in (2, 4):
                # 换骨的关节：这一段从前一节的骨节块之后开始，不咬进去
                start = along(start, (mx(bx, s), by, bz), 0.5)
            bar(rig, bone, side_name(f"antenna_seg{seg}", s), start, (mx(bx, s), by, bz), radii[seg], radii[seg],
                mat="bone_white" if seg % 2 == 0 else "bone_ochre")
            if seg % 2 == 1:
                box(rig, bone, side_name(f"antenna_knob{seg}", s), (mx(bx, s) - 0.45, by - 0.45, bz - 0.45),
                    (mx(bx, s) + 0.45, by + 0.45, bz + 0.45), mat="rust_brown")


# ===========================================================================
# 3. 背甲：16 节拱甲 + 棱脊 + 裂纹 + 体芯
# ===========================================================================
def segment_mats(i: int) -> tuple[str, str]:
    """(顶面甲色, 肩甲色)：中段几节偏灰黄，其余骨白，读出节与节的差别。"""
    if 5 <= i <= 9:
        return "bone_ochre", "bone_white"
    return "bone_white", "bone_ochre" if i % 3 == 0 else "bone_white"


def part_plates(rig: Rig) -> None:
    for i in range(N_SEG):
        w, h, z = WIDTHS[i], seg_half_h(i), seg_z(i)
        bone = f"seg_{i}"
        top_mat, shoulder_mat = segment_mats(i)
        for s in (-1.0, 1.0):
            tag = "l" if s < 0 else "r"
            # 拱：肩甲 + 冠甲，甲板在 z 向略短于体节间距，节间露出暗色体芯
            bar(rig, bone, f"plate_shoulder{i}_{tag}", (mx(w, s), Y_C + 0.2 * h, z), (mx(0.55 * w, s), Y_C + 0.86 * h, z),
                PLATE_HALF_T, PLATE_HALF_Z, mat=shoulder_mat, up=(0, 0, 1))
            bar(rig, bone, f"plate_crown{i}_{tag}", (mx(0.55 * w, s), Y_C + 0.86 * h, z), (mx(0.2 * w, s), Y_C + h, z),
                PLATE_HALF_T, PLATE_HALF_Z, mat=top_mat, up=(0, 0, 1))
            # 侧甲：垂在拱下
            bar(rig, bone, f"plate_side{i}_{tag}", (mx(w, s), Y_C + 0.2 * h, z), (mx(w - 0.25, s), Y_C - 0.8 * h, z),
                0.4, PLATE_HALF_Z - 0.15, mat="bone_grey", up=(0, 0, 1))
            # 沿拱面凸起的骨质棱：从冠部一路压到肩，随体节交替靠前 / 靠后，读出一节节的肋
            rib_z = z + (0.45 if i % 2 else -0.45)
            rib = [(mx(f * w, s), arch_surface_y(i, f * w) + 0.3, rib_z) for f in (0.28, 0.55, 0.85)]
            bar(rig, bone, f"plate_rib_a{i}_{tag}", rib[0], rib[1], 0.34, 0.3, mat="bone_white", up=(0, 0, 1))
            bar(rig, bone, f"plate_rib_b{i}_{tag}", along(rib[1], rib[2], 0.1), rib[2], 0.34, 0.3, mat="bone_ochre", up=(0, 0, 1))
            # 肩上一对小棘
            bar(rig, bone, f"plate_spike{i}_{tag}", (mx(0.55 * w, s), Y_C + 0.86 * h + 0.3, z),
                (mx(0.66 * w, s), Y_C + h + 1.0, z + 0.2), 0.3, 0.3, mat="bone_white")
        bar(rig, bone, f"plate_top{i}", (-0.2 * w, Y_C + h, z), (0.2 * w, Y_C + h, z), PLATE_HALF_T, PLATE_HALF_Z, mat=top_mat,
            up=(0, 0, 1))
        # 体芯（节间缝里能看到的暗色）+ 腹板
        box(rig, bone, f"core{i}", (-w + 0.6, Y_C - 0.8 * h, z - SEG_PITCH / 2), (w - 0.6, Y_C + 0.8 * h, z + SEG_PITCH / 2), mat="bone_dark")
        box(rig, bone, f"belly{i}", (-0.6 * w, Y_C - h, z - 1.0), (0.6 * w, Y_C - 0.8 * h, z + 1.0), mat="bone_grey")
        # 顶上的棱脊：两级，向后倾
        box(rig, bone, f"ridge_base{i}", (-0.28, Y_C + h + 0.4, z - 0.9), (0.28, Y_C + h + 1.1, z + 0.7), mat="bone_white")
        box(rig, bone, f"ridge_tip{i}", (-0.2, Y_C + h + 1.1, z - 0.4), (0.2, Y_C + h + 1.7, z + 0.6), mat="bone_white")


# ===========================================================================
# 4. 腿：每节一对，三段 + 爪
# ===========================================================================
def part_legs(rig: Rig) -> None:
    for i in range(N_SEG):
        w, z = WIDTHS[i], seg_z(i)
        for s in (-1.0, 1.0):
            leg_a, leg_b, leg_c = (side_name(f"leg_{i}_{j}", s) for j in LEG_JOINTS)
            p = leg_points(i, s)
            hip, knee, mid, ankle, toe, tip = (p[k] for k in ("hip", "knee", "mid", "ankle", "toe", "tip"))
            box(rig, leg_a, side_name(f"coxa{i}", s), (mx(w + 0.75, s) - 0.65, Y_C - 1.1, z - 0.8),
                (mx(w + 0.75, s) + 0.65, Y_C + 0.3, z + 0.8), mat="bone_grey")
            # 每段骨干在关节处缩进，关节块（挂下一段的骨）才不会咬进上一段；
            # 股、胫粗 0.9 / 0.75（上一版 0.48 / 0.4 的近两倍），胫分两截折出一道弧
            bar(rig, leg_a, side_name(f"femur{i}", s), hip, along(knee, hip, 1.7), 0.9, 0.9, mat="bone_white")
            box(rig, leg_b, side_name(f"knee{i}", s), (knee[0] - 1.0, knee[1] - 1.0, knee[2] - 1.0),
                (knee[0] + 1.0, knee[1] + 1.0, knee[2] + 1.0), mat="bone_ochre")
            bar(rig, leg_b, side_name(f"tibia_up{i}", s), along(knee, mid, 0.9), mid, 0.75, 0.75, mat="bone_white")
            bar(rig, leg_b, side_name(f"tibia_low{i}", s), along(mid, ankle, 0.1), along(ankle, mid, 0.8), 0.7, 0.7, mat="bone_ochre")
            box(rig, leg_c, side_name(f"ankle{i}", s), (ankle[0] - 0.8, ankle[1] - 0.8, ankle[2] - 0.8),
                (ankle[0] + 0.8, ankle[1] + 0.8, ankle[2] + 0.8), mat="rust_brown")
            claw_mid = along(toe, tip, 0.5 * math.dist(toe, tip))
            bar(rig, leg_c, side_name(f"tarsus{i}", s), along(ankle, toe, 0.8), toe, 0.55, 0.55, mat="bone_ochre")
            bar(rig, leg_c, side_name(f"claw_a{i}", s), toe, claw_mid, 0.4, 0.4, mat="rust_brown")
            bar(rig, leg_c, side_name(f"claw_b{i}", s), claw_mid, tip, 0.22, 0.22, mat="rust_brown")


# ===========================================================================
# 5. 尾钳
# ===========================================================================
def part_tail(rig: Rig) -> None:
    """尾节 + 一对钳状尾肢：每只两节，基节外撑，端节向内弯合，内缘带 3 枚齿。"""
    z = TAIL_Z
    w, h = 2.2, 1.4
    for s in (-1.0, 1.0):
        tag = "l" if s < 0 else "r"
        bar(rig, "telson", f"telson_shoulder_{tag}", (mx(w, s), Y_C - 0.4 + 0.2 * h, z), (mx(0.55 * w, s), Y_C - 0.4 + 0.86 * h, z),
            0.5, 1.1, mat="bone_ochre", up=(0, 0, 1))
        bar(rig, "telson", f"telson_side_{tag}", (mx(w, s), Y_C - 0.4 + 0.2 * h, z), (mx(w - 0.2, s), Y_C - 0.4 - 0.8 * h, z),
            0.4, 1.0, mat="bone_grey", up=(0, 0, 1))
    bar(rig, "telson", "telson_top", (-0.55 * w, Y_C - 0.4 + 0.86 * h, z), (0.55 * w, Y_C - 0.4 + 0.86 * h, z), 0.5, 1.1,
        mat="bone_white", up=(0, 0, 1))
    box(rig, "telson", "telson_core", (-w + 0.5, Y_C - 0.4 - 0.8 * h, z - 1.4), (w - 0.5, Y_C - 0.4 + 0.8 * h, z + 1.4), mat="bone_dark")
    for s in (-1.0, 1.0):
        base_a, base_b = (mx(1.4, s), Y_C - 0.6, z + 1.4), (mx(4.0, s), Y_C - 0.9, z + 5.6)
        tip_b = (mx(1.8, s), Y_C - 1.1, z + 10.4)
        # 基节骨干在关节块前缩进；端节从关节块之后起，两骨之间不互咬
        bar(rig, side_name("pincer_base", s), side_name("pincer_arm", s), base_a, along(base_b, base_a, 0.9), 0.95, 0.85, mat="bone_white")
        box(rig, side_name("pincer_base", s), side_name("pincer_joint", s), (base_b[0] - 0.8, base_b[1] - 0.8, base_b[2] - 0.8),
            (base_b[0] + 0.8, base_b[1] + 0.8, base_b[2] + 0.8), mat="rust_brown")
        bar(rig, side_name("pincer_tip", s), side_name("pincer_blade", s), along(base_b, tip_b, 0.9), tip_b, 0.7, 0.6, mat="bone_ochre")
        # 内缘 3 枚齿，朝钳口（x 向中线）
        for k, t in enumerate((0.3, 0.55, 0.8)):
            p = (base_b[0] + (tip_b[0] - base_b[0]) * t, base_b[1] + (tip_b[1] - base_b[1]) * t, base_b[2] + (tip_b[2] - base_b[2]) * t)
            bar(rig, side_name("pincer_tip", s), side_name(f"pincer_tooth{k}", s), (p[0] - s * 0.4, p[1], p[2]),
                (p[0] - s * 1.2, p[1], p[2] - 0.3), 0.28, 0.28, mat="tooth_ivory")
        bar(rig, side_name("pincer_tip", s), side_name("pincer_point", s), tip_b, (mx(0.9, s), Y_C - 1.2, z + 11.6), 0.3, 0.3,
            mat="rust_brown")


# ===========================================================================
# 6. 裂纹与骨屑
# ===========================================================================
def part_dust_cracks(rig: Rig) -> None:
    """背甲上的暗色裂纹 + 沾着的白色骨屑，都挂在对应体节的 dust_N 骨下。

    裂纹和骨屑都是随机撒的、左右不对称，所以放进 dust_N 骨（镜像门豁免），
    这样体节本身的甲板、侧甲、腿仍然被镜像门逐件盯着。
    """
    rng = random.Random(20261013)
    for i in range(N_SEG):
        z = seg_z(i)
        w = WIDTHS[i]
        bone = f"dust_{i}"
        # 两道曲折裂纹：每道 3 小段，z 向交错
        for c, x0 in enumerate((-0.35 * w, 0.3 * w)):
            for k in range(3):
                zc = z - 0.8 + 0.8 * k
                x = x0 + (0.3 if k % 2 else -0.3)
                y = arch_surface_y(i, x)
                box(rig, bone, f"crack{i}_{c}_{k}", (x - 0.14, y - 0.12, zc - 0.5), (x + 0.14, y + 0.14, zc + 0.5), mat="bone_dark")
                # 裂口两侧翘起的碎骨唇，让裂纹有深度而不是一道黑线
                lip = 0.5 if (i + c + k) % 2 else -0.5
                box(rig, bone, f"crack_lip{i}_{c}_{k}", (x + lip - 0.2, y - 0.05, zc - 0.4), (x + lip + 0.2, y + 0.5, zc + 0.4),
                    mat="bone_ochre")
        # 骨屑：顶面 4 粒、肩上 2 粒、侧甲上 2 粒，边长 0.35~0.5（再大就咬进甲板超过穿模门限）
        for n in range(8):
            if n < 4:
                x = rng.uniform(-0.55, 0.55) * w
            elif n < 6:
                x = rng.choice((-1, 1)) * rng.uniform(0.6, 0.95) * w
            else:
                x = rng.choice((-1, 1)) * (w + 0.1)
            zz = z + rng.uniform(-0.9, 0.9)
            half = rng.uniform(0.35, 0.5) / 2
            y = arch_surface_y(i, x) if n < 6 else Y_C + rng.uniform(-0.6, 0.6)
            box(rig, bone, f"dust{i}_{n}", (x - half, y - half, zz - half), (x + half, y + half, zz + half), mat="dust_white")
    # 爪尖旁落在地上的小堆骨屑
    for i in (2, 6, 10, 14):
        sway = 0.5 if i % 2 == 0 else -0.5
        s = rng.choice((-1.0, 1.0))
        for n in range(3):
            x = mx(WIDTHS[i] + 7.2 + 0.25 * n, s)
            zz = seg_z(i) + sway * 1.6 + 0.3 * (n - 1)
            size = 0.5 - 0.05 * n
            y0 = 0.0 if n < 2 else 0.4
            box(rig, f"dust_{i}", f"dust_pile{i}_{n}", (x - size / 2, y0, zz - size / 2), (x + size / 2, y0 + size, zz + size / 2),
                mat="dust_white")


PARTS = {
    "head": ("1. 头甲 + 六圈逆向骨钻", part_head),
    "antennae": ("2. 两条三节长触须", part_antennae),
    "plates": ("3. 16 节拱形背甲 / 棱脊 / 体芯", part_plates),
    "legs": ("4. 32 条三段腿 + 爪", part_legs),
    "tail": ("5. 尾节 + 一对钳状尾肢", part_tail),
    "dust": ("6. 背甲裂纹 + 白色骨屑", part_dust_cracks),
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
# 镜像门只认 _l/_r 成对。这些骨本来就不左右对称：骨屑随机撒；钻环的齿有旋向（有手性）。
ASYM = tuple(f"dust_{i}" for i in range(N_SEG)) + tuple(f"ring_{k}" for k in range(RING_COUNT))


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制（髓蛀蜈纵长 70+px），去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = CreatureGates("髓蛀蜈 / sui_zhu_wu", tuple(MATS), asym=ASYM)


def self_test() -> None:
    print("运行 gen_sui_zhu_wu.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    moving = moving_bones(rig)
    segs = [b for b in rig.bone_order if b.startswith("seg_")]

    assert len(segs) >= MIN_SEGMENTS, f"体节 {len(segs)} < {MIN_SEGMENTS}"
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert abs(lo[1]) < 0.15, f"贴地失败：最低点 y={lo[1]:.3f}，爪尖应落在 0 附近"
    length = hi[2] - lo[2]
    assert 60.0 <= length <= 95.0, f"纵长 {length:.1f}px 超出 60..95"
    print(f"  [OK] 体节 {len(segs)} 可动骨 {len(moving)} cube {len(rig.elements)} 纵长 {length:.1f}px 最低 y={lo[1]:.2f}")

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


# yaw 方向（枯皮狼那轮用标记块实测）：yaw=0 相机在 +Z（照到尾）；yaw=180 相机在 -Z（照到头）；
# yaw=90 相机在 -X（照到左侧，头朝画面左）；yaw=-90 相机在 +X（照到右侧，头朝画面右）。
def six_views(rig: Rig) -> list[tuple[str, float, float, tuple[tuple[float, float, float], float]]]:
    lo, hi = rig.bounds()
    whole = focus_of(rig)
    xy_span = max(hi[0] - lo[0], hi[1] - lo[1]) * 1.15
    head_focus = ((0.0, Y_C, lo[2] + 8.0), xy_span)
    tail_focus = ((0.0, Y_C, hi[2] - 8.0), xy_span)
    return [
        ("头吻面 FRONT：相机在 -Z，对准钻头", 180.0, 0.0, head_focus),
        ("尾臀面 REAR：相机在 +Z，对准尾钳", 0.0, 0.0, tail_focus),
        ("左侧面 LEFT：相机在 -X，照到 -X 面", 90.0, 0.0, whole),
        ("右侧面 RIGHT：相机在 +X，照到 +X 面", -90.0, 0.0, whole),
        ("脊背面 TOP：相机在 +Y，照到 +Y 面", 0.0, 89.9, whole),
        ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0, whole),
    ]


CLOSEUP_PARTS = frozenset({"plates", "legs", "dust"})


def render_part_previews() -> None:
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(16), load_font(13)
    print("渲染逐件预览 ...")
    for i, (key, (title, func)) in enumerate(PARTS.items(), start=1):
        rig = build_rig()
        func(rig)
        path = _tmp_model(rig, f"part_{key}")
        # 贯穿全身的长部件（背甲 / 腿 / 骨屑）看全身只剩一条线，改成对准第 6 节左右的特写窗口
        focus = ((0.0, Y_C - 0.5, seg_z(6)), 22.0) if key in CLOSEUP_PARTS else focus_of(rig)
        views = [
            ("头吻面 FRONT：相机在 -Z", 180.0, 0.0),
            ("左前上 3/4：相机在 -X -Z +Y", 135.0, 25.0),
            ("左侧面 LEFT：相机在 -X", 90.0, 0.0),
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


def six_view_sheet(rig: Rig, path: Path, header: str, cell: int = 460) -> Image.Image:
    """版面：第一行两个端面 + 3/4（方格）；其余三个长条视角（左 / 右 / 脊背）各占整行宽幅。

    身体纵长 80px、宽只有 17px，塞进方格里只剩一条细线，所以长条视角渲成整行宽再裁掉上下空白。
    脊背面用 yaw=90：相机在 +Y、头朝画面左，和左侧面的头向一致，便于对照。
    """
    font_h, font_l = load_font(18), load_font(13)
    width = cell * 3 + 10
    band = 300
    views = six_views(rig)
    sheet = Image.new("RGB", (width + 10, 34 + cell + 10 + 3 * (band + 10) + 6), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), header, fill=(235, 235, 235), font=font_h)

    def tag(px: int, py: int, label: str) -> None:
        d.rectangle([px, py, px + 340, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)

    # 第一行：头吻面、尾臀面、左前上 3/4
    for slot, idx in enumerate((0, 1, 5)):
        label, yaw, pitch, focus = views[idx]
        px, py = 5 + slot * (cell + 5), 34
        im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)
        sheet.paste(im, (px, py))
        tag(px, py, label)
    # 其余三行：宽幅长条，渲成整行宽再取中间一条
    for row, idx in enumerate((2, 3, 4)):
        label, yaw, pitch, focus = views[idx]
        if idx == 4:
            yaw = 90.0
        py = 34 + cell + 10 + row * (band + 10)
        im, _ = render(path, yaw=yaw, pitch=pitch, size=width, bg=BG, focus=focus)
        top = (width - band) // 2
        sheet.paste(im.crop((0, top, width, top + band)), (5, py))
        tag(5, py, label + ("（头朝左）" if idx != 3 else "（头朝右）"))
    return sheet


def bone_tree_lines(rig: Rig) -> list[str]:
    """骨骼树。seg_0 完整展开，其余体节只写一行并注明「结构同 seg_0」，否则 150 行读不下去。"""
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines: list[str] = []

    def count_desc(name: str) -> int:
        return sum(1 + count_desc(k) for k in children.get(name, []))

    def walk(name: str, depth: int, expand: bool) -> None:
        cnt = len(rig.bones[name]["children"])
        lines.append(f"{'  ' * depth}{'└ ' if depth else ''}{name} ({cnt})")
        kids = children.get(name, [])
        for kid in kids:
            if kid.startswith("seg_"):
                walk(kid, depth + 1, False)
            elif expand:
                walk(kid, depth + 1, True)
        if not expand and name.startswith("seg_") and name != "seg_0":
            lines.append(f"{'  ' * (depth + 1)}（另 {count_desc(name) - sum(1 + count_desc(k) for k in kids if k.startswith('seg_'))} 根子骨，结构同 seg_0：腿 ×6、骨屑 ×1）")

    walk("root", 0, True)
    return lines


def bone_tree_panel(rig: Rig, height: int) -> Image.Image:
    lines = bone_tree_lines(rig)
    font_h, font_s, font_t = load_font(16), load_font(13), load_font(11)
    step = 13
    panel = Image.new("RGB", (400, max(height, 90 + len(lines) * step)), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.text((12, 10), "骨骼层级树（括号内为挂载 cube 数）", fill=(180, 235, 200), font=font_h)
    segs = sum(1 for b in rig.bones if b.startswith("seg_"))
    d.text((12, 34), f"骨骼 {len(rig.bones)}（可动 {len(moving_bones(rig))}，下限 ≥ {MIN_MOVING_BONES}）体节 {segs}",
           fill=(240, 210, 150), font=font_s)
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
        (f"NOW：{ROUND_TAG}（{len(rig.elements)} cube / {len(rig.bones)} 骨 / {N_SEG} 节）", sheet_now),
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
    ap = argparse.ArgumentParser(description="髓蛀蜈 (sui_zhu_wu) 生成器 Round 2/3")
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
    doc = rig.bbmodel("SuiZhuWu", namespace="bong")
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
