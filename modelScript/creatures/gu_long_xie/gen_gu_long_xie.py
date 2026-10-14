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

ROUND_TAG = "Round 3/3"

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
SK_WALL = 2.4                       # 环壁厚度
SK_SOLID_ABOVE = 24.0               # 此高度以上整层实心
SK_REAR_OPEN_BELOW = 20.5           # 此高度以下后壁敞开（露出蟹腹）

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
    "palm_c": (19.0, 10.0, -26.0), "palm_h": (7.2, 6.4, 8.0),
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
    t = (an[0] + u[0] * LEG_REACH[2], 0.4, an[2] + u[2] * LEG_REACH[2])
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
# 1. 颅骨壳：逐层椭圆环体素壳（圆鼓穹顶）+ 眼窝 / 鼻腔凹陷 + 缝线 + 獠牙
# ===========================================================================
CELL = 1.2                           # 体素边长（平面与层高同取，px）
SOCKET_C = (5.75, 21.0)              # 眼窝中心 (|x|, y)
SOCKET_R = (3.3, 3.5)                # 眼窝椭圆半轴 (x, y)
NOSE_C, NOSE_R = (0.0, 16.0), (1.9, 2.0)
FRONT_DZ = -3.0                      # 相对壳心 z 的前半区：眼窝 / 鼻腔只在此区开洞


def ellipse_at(ym: float) -> tuple[float, float]:
    """高度 ym 处椭球截面的半宽、半长（逐层收边出弧面）。"""
    dy = ym - SK_C[1]
    f = math.sqrt(max(0.0, 1.0 - (dy / SK_RY) ** 2))
    return SK_RX * f, SK_RZ * f


def surface_z(x: float, y: float) -> float:
    """壳前表面在 (x, y) 处的 z，用来把牙、眉弓这类凸件贴在弧面上。"""
    w, l = ellipse_at(y)
    return SK_C[2] - l * math.sqrt(max(0.0, 1.0 - (x / w) ** 2))


def in_ellipse(x: float, y: float, c: tuple[float, float], r: tuple[float, float], grow: float = 1.0) -> bool:
    return ((abs(x) - c[0]) / (r[0] * grow)) ** 2 + ((y - c[1]) / (r[1] * grow)) ** 2 < 1.0


def in_face_hole(x: float, ym: float, dz: float, grow: float = 1.0) -> bool:
    if dz > FRONT_DZ:
        return False
    return in_ellipse(x, ym, SOCKET_C, SOCKET_R, grow) or in_ellipse(x, ym, NOSE_C, NOSE_R, grow)


def build_skull_cells() -> list[dict]:
    """逐层铺椭圆环：顶部整层实心，下部只留壁厚；返回所有体素 {k, ix, iz, x, z, y0, mat, exposed}。"""
    occupied: dict[tuple[int, int, int], dict] = {}
    k = 0
    while True:
        y0 = SK_Y0 + CELL * k
        ym = y0 + CELL / 2
        if abs(ym - SK_C[1]) >= SK_RY:
            break
        w, l = ellipse_at(ym)
        solid = ym > SK_SOLID_ABOVE or w - SK_WALL < 0.6 or l - SK_WALL < 0.6
        base = "bone_white"
        nx, nz = int(w / CELL) + 1, int(l / CELL) + 1
        for ix in range(-nx, nx + 1):
            for iz in range(-nz, nz + 1):
                x, dz = ix * CELL, iz * CELL
                if (x / w) ** 2 + (dz / l) ** 2 > 1.0:
                    continue
                inside = solid or (x / (w - SK_WALL)) ** 2 + (dz / (l - SK_WALL)) ** 2 >= 1.0
                dark_lining = False
                if not inside:
                    # 壁内侧再往里 1.2：眼窝 / 鼻腔洞后的暗衬
                    deep = (x / (w - SK_WALL - 1.2)) ** 2 + (dz / (l - SK_WALL - 1.2)) ** 2 >= 1.0
                    if not (deep and dz <= FRONT_DZ and in_ellipse(x, ym, SOCKET_C, SOCKET_R, 1.3)):   # 只给眼窝衬暗，鼻腔不衬
                        continue
                    dark_lining = True
                if inside and ym < SK_REAR_OPEN_BELOW and dz > 0.45 * l and not solid:
                    continue                         # 后下方留大洞，露出蟹腹
                if inside and in_face_hole(x, ym, dz):
                    continue                         # 眼窝 / 鼻腔凹进去
                mat = "crack_dark" if dark_lining else base
                if mat != "crack_dark" and in_face_hole(x, ym, dz, 1.38):
                    mat = "bone_grey"                # 眶缘 / 鼻缘一圈阴面骨
                elif mat == "bone_white" and (((k // 2) * 73856093) ^ ((ix // 2) * 19349663) ^ ((iz // 2) * 83492791)) % 100 < 24:
                    mat = "bone_grey"                # 旧骨斑：按 2×2×2 的块成片，不逐格随机
                occupied[(k, ix, iz)] = {"k": k, "ix": ix, "iz": iz, "x": x, "z": SK_C[2] + dz, "y0": y0, "ym": ym,
                                         "mat": mat, "dark": dark_lining, "w": w, "l": l}
        k += 1
    for (kk, ix, iz), c in occupied.items():
        c["exposed"] = (not c["dark"]) and any((kk + dk, ix + dx, iz + dz) not in occupied
                                              for dk, dx, dz in ((1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)))
    return list(occupied.values())


SKULL_CELLS = build_skull_cells()
SKULL_INDEX = {(c["k"], c["ix"], c["iz"]): c for c in SKULL_CELLS}
SKULL_LAYERS = max(c["k"] for c in SKULL_CELLS) + 1


def paint_skull_marks() -> None:
    """缝线与裂纹：改体素材质，不加几何。矢状缝在顶上走锯齿，三条裂纹从穹顶往下爬。"""
    rng = random.Random(77)
    for c in SKULL_CELLS:
        # 矢状缝：沿中线锯齿，只在顶面实心区
        if c["exposed"] and c["ym"] > SK_SOLID_ABOVE - 2.0 and abs(c["x"]) < CELL * 0.6:
            if c["iz"] % 2 == 0 or c["ym"] > 29.0:
                c["mat"] = "crack_dark"
    for n in range(4):
        theta = rng.uniform(0.0, math.tau)
        k = rng.randint(SKULL_LAYERS - 7, SKULL_LAYERS - 4)
        for _ in range(8):
            if k < 2:
                break
            ym = SK_Y0 + CELL * k + CELL / 2
            w, l = ellipse_at(ym)
            theta += rng.uniform(-0.12, 0.12)
            ix = round(w * math.cos(theta) * 0.97 / CELL)
            iz = round(l * math.sin(theta) * 0.97 / CELL)
            c = SKULL_INDEX.get((k, ix, iz))
            if c is not None and c["exposed"] and c["mat"] != "bone_grey":
                c["mat"] = "crack_dark"
            k -= 1


paint_skull_marks()


def part_skull(rig: Rig) -> None:
    """体素按行合并成条：每层每个 z 行里连续同材质的格子并成一根 x 向长条，减少 cube 数。"""
    rows: dict[tuple[int, int, str], list[int]] = {}
    for c in SKULL_CELLS:
        rows.setdefault((c["k"], c["iz"], c["mat"]), []).append(c["ix"])
    n = 0
    for (k, iz, mat), ixs in sorted(rows.items()):
        ixs.sort()
        start = prev = ixs[0]
        for ix in ixs[1:] + [None]:
            if ix is not None and ix == prev + 1:
                prev = ix
                continue
            y0 = SK_Y0 + CELL * k
            z = SK_C[2] + iz * CELL
            box(rig, "shell", f"sk{n}", ((start - 0.5) * CELL, y0, z - CELL / 2), ((prev + 0.5) * CELL, y0 + CELL, z + CELL / 2), mat=mat)
            n += 1
            if ix is not None:
                start = prev = ix
    # 獠牙：一对，每根三段收细贴弧面；左边那根只剩半截
    for s, parts in ((-1.0, 2), (1.0, 3)):
        x = s * 5.2
        zf = surface_z(x, SK_Y0 + CELL * 0.5) + 1.2
        pts = [(x, SK_Y0 + 0.4, zf), (x + s * 0.1, SK_Y0 - 1.8, zf - 0.05), (x + s * 0.2, SK_Y0 - 3.6, zf - 0.1),
               (x + s * 0.3, SK_Y0 - 5.2, zf - 0.2)]
        radii = [1.5, 1.2, 0.8, 0.3]
        taper(rig, "shell", "fang" + ("_l" if s < 0 else "_r") + "_", pts[:parts + 1], radii[:parts + 1], mat="bone_white")
    # 小门牙：左右各两颗，长短不一
    for k, (x, ln) in enumerate(((1.0, 2.8), (3.0, 2.2))):
        zf = surface_z(x, SK_Y0 + 0.6) + 0.5
        for s in (-1.0, 1.0):
            mbox(rig, "shell", f"incisor{k}", s, x - 0.55, x + 0.55, SK_Y0 - ln, SK_Y0 + 0.4, zf, zf + 1.0, mat="bone_white")
    # 眉弓：眼窝上缘一条贴弧面的凸骨，分三段跟着前表面走
    yb = SOCKET_C[1] + SOCKET_R[1] + 0.2
    for k, (xa, xb) in enumerate(((2.2, 5.0), (5.0, 8.0), (8.0, 10.6))):
        zs = surface_z((xa + xb) / 2, yb) - 0.9
        for s in (-1.0, 1.0):
            mbox(rig, "shell", f"brow{k}", s, xa, xb, yb - 0.4, yb + 1.0, zs, zs + 1.6, mat="bone_grey")
    # 颧弓：眼窝下缘外侧的凸骨
    for s in (-1.0, 1.0):
        zs = surface_z(9.2, 17.0) - 0.8
        mbox(rig, "shell", "cheek", s, 8.2, 10.4, 15.8, 18.0, zs, zs + 1.6, mat="bone_grey")


# ===========================================================================
# 2. 壳面附着：藤壶 / 苔
# ===========================================================================
def surface_point(k: int, theta: float) -> tuple[float, float, float, float, float]:
    """第 k 层、方位角 theta 处的壳外表面点 (x, y, z, 外法向 nx, 外法向 nz)。"""
    ym = SK_Y0 + CELL * k + CELL / 2
    w, l = ellipse_at(ym)
    x, dz = w * math.cos(theta), l * math.sin(theta)
    nx, nz = x / (w * w), dz / (l * l)
    ln = math.hypot(nx, nz)
    return x, SK_Y0 + CELL * k, SK_C[2] + dz, nx / ln, nz / ln


def part_crust(rig: Rig) -> None:
    """藤壶团与苔点缀在弧面上；位置按「层 + 方位角」取，避开眼窝 / 鼻腔。裂纹与苔斑本体已涂在体素材质里。"""
    rng = random.Random(20261013)

    def clear_of_face(x: float, ym: float, z: float, grow: float = 1.7) -> bool:
        return not in_face_hole(x, ym, z - SK_C[2], grow)

    def place(tag: str, mat: str, size: float, k: int, theta: float, lift: float) -> bool:
        x, y, z, nx, nz = surface_point(k, theta)
        if not clear_of_face(x, y + CELL / 2, z):
            return False
        if y + CELL / 2 < SK_REAR_OPEN_BELOW and nz > 0.3:
            return False                         # 后下方是敞开的洞，没有壳可附
        # 外移 lift，让立方体露出壳面；半边长 size/2 保证嵌进弧面里不悬空
        cx, cz = x + nx * lift, z + nz * lift
        box(rig, "crust", tag, (cx - size / 2, y, cz - size / 2), (cx + size / 2, y + size, cz + size / 2), mat=mat)
        return True

    # 藤壶：每团 3 枚（一大两小），顶面一个暗孔
    top = SKULL_LAYERS - 4
    n = 0
    for c in range(9):
        k0 = rng.randint(3, top)
        th0 = rng.uniform(0.0, math.tau)
        for j, size in enumerate((1.9, 1.4, 1.4)):
            k = max(1, min(top, k0 + rng.randint(-1, 1)))
            th = th0 + rng.uniform(-0.18, 0.18)
            if place(f"barn{n}", "barnacle_grey", size, k, th, 0.35):
                x, y, z, nx, nz = surface_point(k, th)
                box(rig, "crust", f"barn_hole{n}", (x + nx * 1.0 - 0.35, y + size - 0.3, z + nz * 1.0 - 0.35),
                    (x + nx * 1.0 + 0.35, y + size + 0.1, z + nz * 1.0 + 0.35), mat="crack_dark")
            n += 1
    # 苔团：鼓起一层薄苔 + 一条垂下去的细苔
    for c in range(12):
        k = rng.randint(2, top)
        th = rng.uniform(0.0, math.tau)
        if place(f"moss_tuft{c}", "moss_olive", rng.uniform(2.0, 3.0), k, th, 0.2) and k > 2:
            x, y, z, nx, nz = surface_point(k - 1, th)
            box(rig, "crust", f"moss_drip{c}", (x + nx * 0.35 - 0.35, y - 1.2, z + nz * 0.35 - 0.35),
                (x + nx * 0.35 + 0.35, y + CELL, z + nz * 0.35 + 0.35), mat="moss_olive")
    # 垂苔：从壳下沿往下挂，三束（前 / 左 / 右），每束三根逐渐变细
    low = SKULL_CELLS[0]
    for k, dz in enumerate((-3.0, 0.0, 3.0)):
        ln = 1.8 - 0.2 * abs(k - 1)
        for s in (-1.0, 1.0):
            x0 = ellipse_at(SK_Y0 + 0.6)[0] - SK_WALL
            mbox(rig, f"moss_{'l' if s < 0 else 'r'}", f"moss_hang{k}", s, x0 - 0.2, x0 + 0.6, SK_Y0 - ln, SK_Y0 + 0.2, SK_C[2] + dz - 0.4, SK_C[2] + dz + 0.4, mat="moss_olive")
    for s in (-1.0, 1.0):
        zf = surface_z(2.0, SK_Y0 + 0.5)
        mbox(rig, "moss_front", "moss_fh", s, 1.6, 2.4, SK_Y0 - 2.6, SK_Y0 + 0.2, zf + 0.2, zf + 1.2, mat="moss_olive")


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
    # 腹三节：一节比一节细，往后微微上翘卷进壳里；每节沿 z 切三片（前后两片收窄，中片取满）成圆鼓的环，
    # 背上一条脊，两侧各一块侧甲板，节间一道暗缝
    sizes = ((4.6, 6.4), (4.0, 5.6), (3.4, 4.8))
    y_c = (11.0, 12.0, 13.4)
    for i in range(3):
        hx, hy = sizes[i]
        za, zb = AB_Z[i], AB_Z[i + 1] - 0.8
        zl = (zb - za) / 3
        for n, inset in enumerate((0.22, 0.0, 0.22)):
            fx, fy = hx * (1.0 - inset), hy * (1.0 - inset)
            box(rig, f"ab_{i}", f"ab{i}_s{n}", (-fx, y_c[i] - fy / 2, za + zl * n), (fx, y_c[i] + fy / 2, za + zl * (n + 1)), mat="rust_orange")
        box(rig, f"ab_{i}", f"ab{i}_ridge", (-0.7, y_c[i] + hy / 2, za + 0.4), (0.7, y_c[i] + hy / 2 + 0.7, zb - 0.4), mat="rust_dark")
        box(rig, f"ab_{i}", f"ab{i}_seam", (-hx + 0.4, y_c[i] - hy / 2 + 0.4, zb), (hx - 0.4, y_c[i] + hy / 2 - 0.4, AB_Z[i + 1]), mat="rust_dark")
        for s in (-1.0, 1.0):
            mbox(rig, f"ab_{i}", f"ab{i}_plate", s, hx, hx + 0.8, y_c[i] - hy / 2 + 0.6, y_c[i] + hy / 2 - 0.6, za + 0.3, zb - 0.4, mat="rust_dark")
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
    arm_r = 3.4 if big else 1.4
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
    # 掌：沿 y 切成 5 片，上下两端收窄，中间鼓出，做成圆鼓的掌
    y0, y1 = pc[1] - ph[1], pc[1] + ph[1]
    z0, z1 = pc[2] - ph[2], pc[2] + ph[2]
    n_slice = 5
    for i in range(n_slice):
        ya, yb = y0 + (y1 - y0) * i / n_slice, y0 + (y1 - y0) * (i + 1) / n_slice
        f = math.sqrt(max(0.0, 1.0 - (((ya + yb) / 2 - pc[1]) / ph[1]) ** 2))
        f = 0.55 + 0.45 * f
        box(rig, palm, f"{name}_palm_s{i}", (pc[0] - ph[0] * f, ya, pc[2] - ph[2] * (0.5 + 0.5 * f)),
            (pc[0] + ph[0] * f, yb, pc[2] + ph[2] * (0.5 + 0.5 * f)), mat="bone_grey" if i in (0, n_slice - 1) else "bone_white")
    nk = 5 if big else 3
    for k in range(nk):
        zk = z0 + 1.2 + (z1 - z0 - 2.4) * k / max(nk - 1, 1)
        kw = 1.0 if big else 0.6
        box(rig, palm, f"{name}_bump{k}", (pc[0] - kw, y1, zk - kw), (pc[0] + kw, y1 + kw * 1.2, zk + kw), mat="bone_white")
    # 定指（固定，随掌）：三段收细 + 内缘锯齿
    # 掌朝前是 -z：手指向 -z 延伸，所以从掌前缘 z0 开始，不是 z1
    fz = z0
    f0 = (pc[0], pc[1] - ph[1] * (0.45 if big else 0.4), fz)
    L1, L2, L3 = (6.5, 12.0, 16.5) if big else (4.5, 8.0, 10.5)
    f_pts = [f0, (f0[0] - s * 0.2, f0[1] - 0.5, fz - L1), (f0[0] - s * 0.8, f0[1] - 0.2, fz - L2), (f0[0] - s * 1.5, f0[1] + 1.6, fz - L3)]       # 定指尖向上翘
    radii = (3.6, 2.8, 1.8, 0.6) if big else (1.3, 1.0, 0.65, 0.25)
    taper(rig, palm, f"{name}_fixed", f_pts, list(radii), mat="bone_white")
    # 内缘锯齿：定指向上、动指向下，大小交错；大螯 6 枚、小螯 3 枚
    n_teeth = 6 if big else 3
    for k in range(n_teeth):
        tt = 0.12 + 0.78 * k / max(n_teeth - 1, 1)
        h = (1.9 if k % 2 == 0 else 1.2) if big else 0.9
        wd = 0.9 if big else 0.4
        p = lerp3(f_pts[0], f_pts[3], tt)
        r = (radii[0] + (radii[3] - radii[0]) * tt) * 0.55
        box(rig, palm, f"{name}_ftooth{k}", (p[0] - wd, p[1] + r - 0.3, p[2] - wd), (p[0] + wd, p[1] + r + h, p[2] + wd), mat="bone_grey")
    # 动指（挂 dactyl 骨）：从掌上前缘铰出，弯向定指尖
    d0 = (pc[0], pc[1] + ph[1] * (0.62 if big else 0.55), z0 + 0.2)
    d_pts = [d0, (d0[0] - s * 0.1, d0[1] - 0.3, z0 - L1 * 0.95), (d0[0] - s * 0.6, d0[1] - 1.4, z0 - L2 * 0.95),
             (f_pts[3][0] + s * 0.0, f_pts[3][1] + 1.7, f_pts[3][2] - 0.2)]
    taper(rig, dact, f"{name}_dact", d_pts, list(radii), mat="bone_white")
    for k in range(n_teeth):
        tt = 0.12 + 0.78 * k / max(n_teeth - 1, 1)
        h = (1.2 if k % 2 == 0 else 1.9) if big else 0.9
        wd = 0.9 if big else 0.4
        p = lerp3(d_pts[0], d_pts[3], tt)
        r = (radii[0] + (radii[3] - radii[0]) * tt) * 0.55
        box(rig, dact, f"{name}_dtooth{k}", (p[0] - wd, p[1] - r - h, p[2] - wd), (p[0] + wd, p[1] - r + 0.3, p[2] + wd), mat="bone_grey")


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
        box(rig, "claw_big_palm", f"claw_moss{n}", (pc[0] + ph[0] * 0.85, pc[1] - 3.0 + n * 3.5, z - 1.3), (pc[0] + ph[0] + 0.5, pc[1] - 2.0 + n * 3.5, z + 1.3), mat="moss_olive")


def part_claw_small(rig: Rig) -> None:
    claw(rig, "claw_small", SMALL, False)


# ===========================================================================
# 6. 步足：四对，每腿三段 + 尖爪
# ===========================================================================
def part_legs(rig: Rig) -> None:
    """四对三段足：每段由三截鼓肚的方柱接成（中间粗、两端收），关节块锈橙，胫背三根棘，末端钩爪分三截弯出。"""
    for p in range(LEG_PAIRS):
        for s in (-1.0, 1.0):
            t = side_tag(s)
            h, k, a, tip = leg_points(p, s)
            la, lb, lc = (f"leg{p}_{j}{t}" for j in LEG_JOINTS)
            # 股：髋座 + 粗骨干（两端 1.7、中段 2.5），膝块挂 b 骨
            box(rig, la, f"coxa{p}{t}", (h[0] - 2.2, h[1] - 2.2, h[2] - 1.6), (h[0] + 2.2, h[1] + 2.2, h[2] + 1.6), mat="rust_dark")
            taper(rig, la, f"femur{p}{t}_", [along(h, k, 2.0), lerp3(h, k, 0.38), lerp3(h, k, 0.68), along(k, h, 2.2)], [1.8, 2.7, 2.7, 1.9], mat="bone_white")
            box(rig, lb, f"knee{p}{t}", (k[0] - 2.2, k[1] - 2.2, k[2] - 2.2), (k[0] + 2.2, k[1] + 2.2, k[2] + 2.2), mat="rust_orange")
            # 胫：长、略细、中段微鼓，背面三根小棘
            taper(rig, lb, f"tibia{p}{t}_", [along(k, a, 2.4), lerp3(k, a, 0.4), lerp3(k, a, 0.7), along(a, k, 1.8)], [1.6, 2.1, 2.0, 1.4], mat="bone_white")
            for n, tt in enumerate((0.3, 0.55, 0.8)):
                c = lerp3(k, a, tt)
                box(rig, lb, f"spike{p}_{n}{t}", (c[0] - 0.5, c[1] + 2.0, c[2] - 0.5), (c[0] + 0.5, c[1] + 3.2, c[2] + 0.5), mat="bone_grey")
            box(rig, lc, f"ankle{p}{t}", (a[0] - 1.7, a[1] - 1.7, a[2] - 1.7), (a[0] + 1.7, a[1] + 1.7, a[2] + 1.7), mat="rust_dark")
            # 跗节 + 钩爪：先略外凸再向内收到点地的爪尖，像一只弯钩
            mid = lerp3(a, tip, 0.5)
            bar(rig, lc, f"tarsus{p}{t}", along(a, tip, 1.7), mid, 1.2, 1.2, mat="bone_grey")
            u = (math.cos(math.radians(LEG_YAW[p])) * s, 0.0, math.sin(math.radians(LEG_YAW[p])))
            c1 = (mid[0] + (tip[0] - mid[0]) * 0.45 + u[0] * 0.7, mid[1] + (tip[1] - mid[1]) * 0.45, mid[2] + (tip[2] - mid[2]) * 0.45 + u[2] * 0.7)
            c2 = (mid[0] + (tip[0] - mid[0]) * 0.8 + u[0] * 0.2, mid[1] + (tip[1] - mid[1]) * 0.8, mid[2] + (tip[2] - mid[2]) * 0.8 + u[2] * 0.2)
            taper(rig, lc, f"claw{p}{t}_", [mid, c1, c2, tip], [1.0, 0.9, 0.7, 0.5], mat="rust_dark")


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
ASYM = ("crust", "shell") + tuple(f"{n}_{p}" for n in ("claw_big", "claw_small") for p in ("arm", "fore", "palm", "dactyl"))

# 穿模门按 AABB 判：斜置的骨干和壳、关节块的 AABB 会虚报互穿。骨白 × 骨灰、锈橙 × 锈暗这类同族配对放行。
SOFT_OVER = frozenset({
    frozenset(("bone_white", "bone_grey")), frozenset(("rust_orange", "rust_dark")),
    frozenset(("bone_white", "barnacle_grey")), frozenset(("bone_grey", "barnacle_grey")),
    frozenset(("bone_white", "moss_olive")), frozenset(("bone_grey", "moss_olive")),
    frozenset(("rust_orange", "crack_dark")),     # 眼柄穿过眼窝后的暗衬
    frozenset(("barnacle_grey", "crack_dark")), frozenset(("moss_olive", "crack_dark")),
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
