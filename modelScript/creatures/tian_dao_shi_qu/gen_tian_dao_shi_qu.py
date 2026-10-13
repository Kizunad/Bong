#!/usr/bin/env python3
"""天道尸蛆（tian_dao_shi_qu）多体节生物模型程序化生成器 —— Round 1/3。

依据 worldview.md §十八 后期：天道尸体高度腐烂后诞生的强大蛆虫，终末大混战里与修士争夺最后的残存养分。终局 Boss。

外形（任务卡第四批 + 参考图 exploded.png）：
  · 14 节白胖体节，每节一圈十二边形「皮环」（顶部奶白、腹部肉粉），中段最粗；
  · 中段体节两侧开「透皮窗」：窗里看得见暗红脏器和吞进去的发光灵光碎块（皮是半透的，几何上只能开窗）
  · 节与节之间一圈暗色腐缝，缝底挂着滴落的腐液；背上有肉粉褶皱棱和褐色渍斑
  · 头：一圈厚唇（四瓣可开合）围着三层同心倒钩齿环，齿环由外到里收窄，最里是暗色咽喉；头侧两对感觉坑 + 一对触须
  · 腹下 12 对肉垫短足（每足两节：足桩 + 带爪垫）
  · 尾：三节收细的尾环 + 一根上翘的呼吸管，管口是带辐条的气门板

骨骼：14 体节 + 每节脏器 / 腐液各 1 骨 + 24 条两节足 + 头 / 四瓣唇 / 三层齿环 / 咽喉 / 触须 / 尾环 / 呼吸管，远超「多体节 ≥ 30」。

每个编号部件是一个 part_*()，可单独出预览。

用法：
  python3 modelScript/creatures/tian_dao_shi_qu/gen_tian_dao_shi_qu.py             # 全身 + 逐件预览 + check.png
  python3 modelScript/creatures/tian_dao_shi_qu/gen_tian_dao_shi_qu.py --self-test # 门禁差分自证
  python3 modelScript/creatures/tian_dao_shi_qu/gen_tian_dao_shi_qu.py --list

坐标约定：16 单位 = 1 格，地面 y=0，头朝 -Z，左侧是 -X（与髓蛀蜈同）。
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

CREATURE_DIR = REPO / "modelScript" / "creatures" / "tian_dao_shi_qu"
MODELS_DIR = REPO / "modelScript" / "models"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/tian_dao_shi_qu")
PARTS_DIR = REVIEW_DIR / "parts"
EXPLODED_REF = REVIEW_DIR / "exploded.png"
BBMODEL_OUT = CREATURE_DIR / "TianDaoShiQu.bbmodel"
BBMODEL_COPY = MODELS_DIR / "TianDaoShiQu.bbmodel"

ROUND_TAG = "Round 3/3"

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "skin_cream": (232, 222, 198),    # 奶白蛆皮
    "skin_pink": (214, 172, 156),     # 肉粉：腹部、褶皱、足
    "rot_dark": (58, 36, 34),         # 腐缝、咽喉、感觉坑
    "organ_red": (122, 54, 50),       # 暗红脏器、齿环底
    "glint_amber": (238, 186, 84),    # 吞进去的灵光碎块
    "glint_core": (255, 238, 176),    # 灵光碎块的亮芯
    "tooth_ivory": (242, 232, 206),   # 倒钩齿、辐条
    "ichor_brown": (96, 64, 40),      # 腐液、渍斑、气门板
}

# ---------------------------------------------------------------------------
# 体型参数（px；16 = 1 格）。A 是皮环外切圆半径（边心距），不是顶点半径。
# ---------------------------------------------------------------------------
N_SEG = 14
SEG_PITCH = 9.0
SEG_SKIN_Z = 7.0                    # 皮环 z 厚；其余 2.0 是腐缝
GROOVE_Z = SEG_PITCH - SEG_SKIN_Z
SEG_Z0 = -50.0                      # 第 0 节中心 z
GROUND_GAP = 5.5                    # 腹底离地：足桩垫在这里
SKIN_T = 2.5                        # 皮环径向厚度
SEG_A = [15.5, 18.5, 21.0, 22.5, 23.0, 22.5, 22.0, 21.0, 19.5, 18.0, 16.5, 14.5, 12.5, 10.0]
HEAD_LIFT = 12.0                    # 头昂起的高度
SEG_LIFT = {0: 8.0, 1: 5.0, 2: 3.0, 3: 1.5, 4: 0.5}
WINDOW_SEGS = range(2, 12)          # 开透皮窗的体节
LEG_SEGS = range(1, 13)             # 有足的体节
BARREL_SLICES = 5                   # 每节皮环沿 z 切的片数
BARREL_SLICE_Z = SEG_SKIN_Z / BARREL_SLICES
BARREL_RADIUS = 6.5                 # 皮环侧视圆弧的曲率半径（px）：越小边收得越狠
GROOVE_INSET = 2.4                  # 腐缝比相邻较小皮环缩进的量
TAN15 = math.tan(math.radians(15.0))
BODY_SIDES = 24                     # 体节皮环的边数（头、齿环、尾环仍是十二边形）
MIN_MOVING_BONES = 30
MIN_SEGMENTS = 14


def seg_z(i: int) -> float:
    return SEG_Z0 + SEG_PITCH * i


def seg_y(i: int) -> float:
    return SEG_A[i] + GROUND_GAP + SEG_LIFT.get(i, 0.0)


HEAD_A = 17.5
HEAD_ZLEN = 12.0
HEAD_Z = seg_z(0) - SEG_SKIN_Z / 2 - GROOVE_Z - HEAD_ZLEN / 2      # 头壳中心 z = -61.5
HEAD_Y = HEAD_A + GROUND_GAP + HEAD_LIFT
HEAD_T = 3.0
HEAD_Z_FRONT = HEAD_Z - HEAD_ZLEN / 2                                # 头壳前缘 -67.5

# 咽口三层齿环：外 → 内，各占 3.5 的 z 厚板，紧接头壳前缘往后叠
MOUTH_RINGS = [  # (外切圆半径 A, 径向厚 t, 齿数, 材质)
    (14.5, 3.2, 12, "organ_red"),
    (11.8, 3.2, 10, "rot_dark"),
    (9.1, 3.2, 8, "organ_red"),
]
MOUTH_RING_Z = 3.5
THROAT_Z = 1.5

# 尾
SEG_LAST = N_SEG - 1
TAIL_Z0 = seg_z(SEG_LAST) + SEG_SKIN_Z / 2 + GROOVE_Z               # 第一尾环前缘 z
TAIL_RINGS = [(9.0, 2.5), (7.2, 2.5), (5.5, 2.5)]                    # (A, t)，z 厚都是 3
TAIL_RING_Z = 3.0
TAIL_Y = seg_y(SEG_LAST)
TUBE_PATH = [  # (y 相对尾心, z 相对尾塞后端) 管折点
    (0.0, 0.0), (4.0, 5.0), (9.0, 9.0), (12.0, 11.0), (12.0, 17.0),
]
TUBE_R = [2.4, 2.1, 1.8, 1.8]


def side_tag(s: float) -> str:
    return "l" if s < 0 else "r"


def side_name(base: str, s: float) -> str:
    return f"{base}_{side_tag(s)}"


# ---------------------------------------------------------------------------
# 骨骼
# ---------------------------------------------------------------------------
def leg_geom(i: int, s: float) -> dict[str, float]:
    """第 i 节 s 侧足桩：贴在腹底（边心距 A 处）正下方，x 落在底边外表面范围内。"""
    a = SEG_A[i]
    half = min(2.25, max(1.2, 0.1 * a))
    return {"cx": s * 0.14 * a, "half": half, "top": seg_y(i) - a}


def define_bones(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0), None)
    prev = "root"
    for i in range(N_SEG):
        prev = rig.bone(f"seg_{i}", (0.0, seg_y(i), seg_z(i)), prev)
    for i in range(N_SEG):
        rig.bone(f"gut_{i}", (0.0, seg_y(i), seg_z(i)), f"seg_{i}")
        rig.bone(f"ooze_{i}", (0.0, seg_y(i), seg_z(i) + 3.0), f"seg_{i}")
        if i in LEG_SEGS:
            for s in (-1.0, 1.0):
                g = leg_geom(i, s)
                rig.bone(side_name(f"leg_{i}_a", s), (g["cx"], g["top"], seg_z(i)), f"seg_{i}")
                rig.bone(side_name(f"leg_{i}_b", s), (g["cx"], g["top"] * 0.4, seg_z(i)), side_name(f"leg_{i}_a", s))
    # 头：四瓣唇 + 三层齿环 + 咽喉 + 触须
    rig.bone("head", (0.0, HEAD_Y, HEAD_Z + HEAD_ZLEN / 2), "seg_0")
    z_back = HEAD_Z + HEAD_ZLEN / 2 - 1.0
    rig.bone("lip_top", (0.0, HEAD_Y + HEAD_A, z_back), "head")
    rig.bone("lip_bot", (0.0, HEAD_Y - HEAD_A, z_back), "head")
    rig.bone("lip_l", (-HEAD_A, HEAD_Y, z_back), "head")
    rig.bone("lip_r", (HEAD_A, HEAD_Y, z_back), "head")
    for k in range(len(MOUTH_RINGS)):
        rig.bone(f"ring_{k}", (0.0, HEAD_Y, mouth_ring_z(k)), "head")
    rig.bone("throat", (0.0, HEAD_Y, mouth_ring_z(len(MOUTH_RINGS)) + THROAT_Z / 2 - MOUTH_RING_Z / 2), "head")
    for s in (-1.0, 1.0):
        rig.bone(side_name("palp_a", s), (s * 2.5, HEAD_Y + HEAD_A, HEAD_Z_FRONT + 1.5), "head")
        rig.bone(side_name("palp_b", s), (s * 2.5, HEAD_Y + HEAD_A + 2.4, HEAD_Z_FRONT + 2.5), side_name("palp_a", s))
    # 尾环 + 呼吸管
    prev = f"seg_{SEG_LAST}"
    for k in range(len(TAIL_RINGS)):
        prev = rig.bone(f"telson_{k}", (0.0, TAIL_Y, TAIL_Z0 + TAIL_RING_Z * k), prev)
    tube_z0 = TAIL_Z0 + TAIL_RING_Z * len(TAIL_RINGS) + 1.5
    prev = "telson_2"
    for k in range(3):
        dy, dz = TUBE_PATH[k]
        prev = rig.bone(f"tube_{k}", (0.0, TAIL_Y + dy, tube_z0 + dz), prev)
    dy, dz = TUBE_PATH[-1]
    rig.bone("plate", (0.0, TAIL_Y + dy, tube_z0 + dz), "tube_2")


def mouth_ring_z(k: int) -> float:
    """第 k 层齿环（k = len 时为咽喉）板的中心 z，紧贴头壳前缘往后叠。"""
    return HEAD_Z_FRONT + MOUTH_RING_Z * (k + 0.5)


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


def edge_name(prefix: str, ang: int) -> str:
    """十二边形第 ang° 边的名字。左右镜像边共用同一个词干（折到 x≥0 一侧），才对得上镜像门。"""
    cx = math.cos(math.radians(ang))
    if abs(cx) < 1e-6:
        return f"{prefix}_a{ang}"
    stem = ang if cx > 0 else (180 - ang) % 360
    return f"{prefix}_a{stem}_{'r' if cx > 0 else 'l'}"


def ring(rig: Rig, bone: str, prefix: str, yc: float, zc: float, a: float, t: float, zlen: float, *,
         mat_for, omit: frozenset[int] = frozenset(), only: frozenset[int] | None = None, sides: int = 12) -> None:
    """正 sides 边形环：外切圆半径 a、径向厚 t、z 厚 zlen，边心角 0°, 360/sides°, …（0° = +X）。

    每条边一根定向柱，端点落在中心线顶点上，再各向外延 t/2·tan(半边角) 把外角补平。
    """
    step = 360 // sides
    half = math.radians(step / 2)
    centre_apothem = a - t / 2
    vertex_r = centre_apothem / math.cos(half)
    for k in range(sides):
        ang = step * k
        if ang in omit or (only is not None and ang not in only):
            continue
        p = [(vertex_r * math.cos(math.radians(ang) + d), yc + vertex_r * math.sin(math.radians(ang) + d), zc) for d in (-half, half)]
        bar(rig, bone, edge_name(prefix, ang), p[0], p[1], t / 2, zlen / 2, mat=mat_for(ang), up=(0, 0, 1), extend=t / 2 * math.tan(half))


def skin_mat(ang: int) -> str:
    """上半圈奶白、下半圈肉粉；0°/180° 侧边归肉粉。"""
    return "skin_cream" if math.sin(math.radians(ang)) > 0.1 else "skin_pink"


def groove_a(i: int) -> float:
    """第 i 节后缘腐缝环的外切圆半径。"""
    a_next = SEG_A[i + 1] if i + 1 < N_SEG else TAIL_RINGS[0][0]
    return min(SEG_A[i], a_next) - GROOVE_INSET


def arc_edges(centre: int, half_width: int) -> frozenset[int]:
    """以 centre° 为中心、±half_width° 内的 BODY_SIDES 边形边心角。"""
    step = 360 // BODY_SIDES
    return frozenset(ang for ang in range(0, 360, step) if abs((ang - centre + 180) % 360 - 180) <= half_width)


def window_omit(i: int) -> frozenset[int]:
    """透皮窗去掉的边：奇数节只开侧边（±15°），偶数节开侧边 ±30°，窗大小交替。"""
    if i not in WINDOW_SEGS:
        return frozenset()
    half_width = 15 if i % 2 else 30
    return arc_edges(0, half_width) | arc_edges(180, half_width)


# ===========================================================================
# 1. 头：四瓣厚唇 + 感觉坑
# ===========================================================================
LIP_EDGES = {
    "lip_top": frozenset({60, 90, 120}),
    "lip_bot": frozenset({240, 270, 300}),
    "lip_l": frozenset({150, 180, 210}),
    "lip_r": frozenset({330, 0, 30}),
}


def part_head(rig: Rig) -> None:
    """头壳 = 十二边形厚环，按四瓣挂到四根唇骨；前缘一圈肉粉唇边；两侧各两个感觉坑。"""
    for bone, edges in LIP_EDGES.items():
        ring(rig, bone, "head", HEAD_Y, HEAD_Z, HEAD_A, HEAD_T, HEAD_ZLEN, mat_for=skin_mat, only=edges)
        # 前缘唇边：贴着壳外缘往前长出的一圈肉粉褶，每瓣自成一块
        ring(rig, bone, "lipfold", HEAD_Y, HEAD_Z_FRONT - 0.6, HEAD_A + 1.0, 2.2, 1.2, mat_for=lambda a: "skin_pink", only=edges)
        # 壳顶 / 壳底各一道褶皱棱
        if bone in ("lip_top", "lip_bot"):
            ring(rig, bone, "headrib", HEAD_Y, HEAD_Z + 1.5, HEAD_A + 1.0, 1.0, 2.0, mat_for=lambda a: "skin_pink", only=edges)
    # 感觉坑：0° 边外表面是平的（x = ±A），坑厚 0.6 贴在上面
    for s, bone in ((-1.0, "lip_l"), (1.0, "lip_r")):
        for k, (dy, dz) in enumerate(((1.8, -3.0), (-1.8, 0.5))):
            box(rig, bone, side_name(f"pit{k}", s), (min(s * HEAD_A, s * (HEAD_A + 0.6)), HEAD_Y + dy - 1.0, HEAD_Z + dz - 1.0),
                (max(s * HEAD_A, s * (HEAD_A + 0.6)), HEAD_Y + dy + 1.0, HEAD_Z + dz + 1.0), mat="rot_dark")


# ===========================================================================
# 2. 咽口：三层倒钩齿环 + 咽喉
# ===========================================================================
def part_mouth(rig: Rig) -> None:
    """三层同心齿环，外层齿多、内层齿少；每枚齿两段：粗基向内伸，细尖向前钩（-Z）并沿切向歪。

    相邻两层的齿角错开半个齿距，齿尖朝前不超出自己那块 z 板，三层不互咬。
    """
    for k, (a, t, count, mat) in enumerate(MOUTH_RINGS):
        zc = mouth_ring_z(k)
        bone = f"ring_{k}"
        ring(rig, bone, f"mring{k}", HEAD_Y, zc, a, t, MOUTH_RING_Z, mat_for=lambda ang, m=mat: m)
        inner = a - t
        twist = 0.45 * (1 if k % 2 == 0 else -1)
        for j in range(count):
            phi = 360.0 / count * (j + (0.5 if k % 2 else 0.0))
            theta = math.radians(phi)
            # 内壁是十二边形，齿根要落在 phi 方向的边面上
            delta = math.radians(((phi % 30.0) - 15.0))
            r_in = inner / math.cos(delta)
            r_mid = r_in - 2.6
            r_tip = r_mid - 2.4
            a0 = (r_in * math.cos(theta), HEAD_Y + r_in * math.sin(theta), zc)
            a1 = (r_mid * math.cos(theta + twist * 0.5), HEAD_Y + r_mid * math.sin(theta + twist * 0.5), zc - 0.6)
            a2 = (r_tip * math.cos(theta + twist), HEAD_Y + r_tip * math.sin(theta + twist), zc - 1.3)
            bar(rig, bone, f"tooth{k}_{j}_base", a0, a1, 0.55, 0.55, mat="tooth_ivory")
            bar(rig, bone, f"tooth{k}_{j}_tip", a1, a2, 0.32, 0.32, mat="tooth_ivory")
    # 咽喉：暗色塞板，盖住最内圈的洞
    z0 = mouth_ring_z(len(MOUTH_RINGS)) - MOUTH_RING_Z / 2
    z1 = z0 + THROAT_Z
    box(rig, "throat", "throat_h", (-7.8, HEAD_Y - 4.0, z0), (7.8, HEAD_Y + 4.0, z1), mat="rot_dark")
    box(rig, "throat", "throat_v", (-4.0, HEAD_Y - 7.8, z0), (4.0, HEAD_Y + 7.8, z1), mat="rot_dark")
    rig.cube("throat", "throat_d", (-5.4, HEAD_Y - 5.4, z0), (5.4, HEAD_Y + 5.4, z1), rot=(0.0, 0.0, 45.0),
             org=(0.0, HEAD_Y, (z0 + z1) / 2), mat="rot_dark")


# ===========================================================================
# 3. 触须
# ===========================================================================
def part_palps(rig: Rig) -> None:
    """头顶前缘一对短触须：座块 + 两节肉粉柱，尖端一粒暗色感觉点。"""
    top = HEAD_Y + HEAD_A
    for s in (-1.0, 1.0):
        x = s * 2.5
        box(rig, side_name("palp_a", s), side_name("palp_base", s), (x - 1.2, top, HEAD_Z_FRONT + 0.3),
            (x + 1.2, top + 2.4, HEAD_Z_FRONT + 2.7), mat="skin_pink")
        p0 = (x, top + 2.4, HEAD_Z_FRONT + 1.5)
        p1 = (s * 3.2, top + 6.0, HEAD_Z_FRONT - 2.5)
        p2 = (s * 3.8, top + 8.0, HEAD_Z_FRONT - 6.0)
        bar(rig, side_name("palp_a", s), side_name("palp_seg0", s), p0, p1, 0.95, 0.95, mat="skin_pink")
        bar(rig, side_name("palp_b", s), side_name("palp_seg1", s), p1, p2, 0.7, 0.7, mat="skin_pink")
        box(rig, side_name("palp_b", s), side_name("palp_dot", s), (p2[0] - 0.5, p2[1] - 0.5, p2[2] - 1.0),
            (p2[0] + 0.5, p2[1] + 0.5, p2[2] - 0.0), mat="rot_dark")


# ===========================================================================
# 4. 体节皮环 + 腐缝 + 褶皱棱
# ===========================================================================
def part_body(rig: Rig) -> None:
    for i in range(N_SEG):
        a, y, z = SEG_A[i], seg_y(i), seg_z(i)
        bone = f"seg_{i}"
        omit = window_omit(i)
        # 桶形皮环：沿 z 切 5 片，按半径 BARREL_RADIUS 的圆弧收边（中片取满半径，前后两片收得最多），
        # 侧视是一圈圈鼓出来的圆环，而不是三级台阶
        for n in range(BARREL_SLICES):
            dz = (n - (BARREL_SLICES - 1) / 2) * BARREL_SLICE_Z
            inset = BARREL_RADIUS * (1.0 - math.sqrt(1.0 - (dz / (SEG_SKIN_Z / 2)) ** 2))
            ring(rig, bone, f"seg{i}s{n}", y, z + dz, a - inset, SKIN_T, BARREL_SLICE_Z, mat_for=skin_mat, omit=omit, sides=BODY_SIDES)
        # 腐缝：贴在本节后缘，半径取相邻两节较小者再缩 GROOVE_INSET，缝里比皮环凹进去
        a_next = SEG_A[i + 1] if i + 1 < N_SEG else TAIL_RINGS[0][0]
        ring(rig, bone, f"groove{i}", y, z + SEG_SKIN_Z / 2 + GROOVE_Z / 2, groove_a(i), SKIN_T, GROOVE_Z, mat_for=lambda ang: "rot_dark", sides=BODY_SIDES)
        # 背上肉粉褶皱棱贴着中段顶面 3 条边（中段 z 只有 ±1.5）；闭合节腹面再加一道
        top = arc_edges(90, 30)
        ring(rig, bone, f"rib_a{i}", y, z - 0.8, a + 1.0, 1.0, 1.4, mat_for=lambda ang: "skin_pink", only=top, sides=BODY_SIDES)
        if i % 2:
            ring(rig, bone, f"rib_b{i}", y, z + 0.9, a + 1.0, 1.0, 1.2, mat_for=lambda ang: "skin_pink", only=top, sides=BODY_SIDES)
        if i not in WINDOW_SEGS:
            ring(rig, bone, f"rib_c{i}", y, z, a + 1.0, 1.0, 2.6, mat_for=lambda ang: "skin_pink",
                 only=arc_edges(270, 30), sides=BODY_SIDES)


# ===========================================================================
# 5. 脏器与灵光碎块（透皮窗里看到的）
# ===========================================================================
def part_guts(rig: Rig) -> None:
    """每个开窗节一根中柱（上下顶着皮环内面）+ 两侧脏器叶 + 吞进去的灵光碎块。

    脏器和碎块挂在 gut_N 骨下：随机撒、左右不对称，镜像门豁免，皮环仍被逐件盯着。
    """
    rng = random.Random(20261013)
    for i in WINDOW_SEGS:
        a, y, z = SEG_A[i], seg_y(i), seg_z(i)
        ai = a - SKIN_T                                   # 皮环内切圆半径
        bone = f"gut_{i}"
        box(rig, bone, f"core{i}", (-2.0, y - ai, z - 3.0), (2.0, y + ai, z + 3.0), mat="organ_red")
        for s in (-1.0, 1.0):
            tag = side_tag(s)
            lobe_a_x = 0.5 * ai
            lobe_b_x = 0.6 * ai
            box(rig, bone, f"lobe_a{i}_{tag}", (min(s * 2.0, s * lobe_a_x), y - 0.6 * ai, z - 2.6),
                (max(s * 2.0, s * lobe_a_x), y + 0.6 * ai, z + 2.6), mat="organ_red")
            box(rig, bone, f"lobe_b{i}_{tag}", (min(s * lobe_a_x, s * lobe_b_x), y - 0.32 * ai, z - 2.2),
                (max(s * lobe_a_x, s * lobe_b_x), y + 0.32 * ai, z + 2.2), mat="rot_dark")
            # 一到两块灵光碎块嵌在叶外面；有的节只有一侧有
            if rng.random() < 0.3:
                continue
            for n in range(rng.choice((1, 2))):
                depth_max = 0.88 * ai - lobe_b_x
                size = min(rng.uniform(2.6, 4.4), depth_max)
                yc = y + rng.uniform(-0.26, 0.26) * ai + (n * 1.0 - 0.5) * 0.0
                zc = z + rng.uniform(-0.6, 0.6)
                x0, x1 = lobe_b_x, lobe_b_x + size
                box(rig, bone, f"glint{i}_{tag}{n}", (min(s * x0, s * x1), yc - size / 2, zc - size / 2),
                    (max(s * x0, s * x1), yc + size / 2, zc + size / 2), mat="glint_amber")
                core = size * 0.5
                box(rig, bone, f"glint_core{i}_{tag}{n}", (min(s * x1, s * (x1 + 0.5)), yc - core / 2, zc - core / 2),
                    (max(s * x1, s * (x1 + 0.5)), yc + core / 2, zc + core / 2), mat="glint_core")
            # 偶尔夹一粒碎骨
            if rng.random() < 0.5:
                ys = y + rng.choice((-1, 1)) * rng.uniform(0.42, 0.55) * ai   # 躲开 lobe_b（±0.32ai）
                box(rig, bone, f"shard{i}_{tag}", (min(s * lobe_a_x, s * (lobe_a_x + 1.6)), ys - 0.8, z - 0.8),
                    (max(s * lobe_a_x, s * (lobe_a_x + 1.6)), ys + 0.8, z + 0.8), mat="tooth_ivory")


# ===========================================================================
# 6. 腹下肉垫足
# ===========================================================================
def part_legs(rig: Rig) -> None:
    """12 对两节短足：足桩垫在腹底、带爪垫贴地，三枚暗色小钩。"""
    for i in LEG_SEGS:
        z = seg_z(i)
        for s in (-1.0, 1.0):
            g = leg_geom(i, s)
            cx, half, top = g["cx"], g["half"], g["top"]
            stub_h = top * 0.6
            leg_a, leg_b = side_name(f"leg_{i}_a", s), side_name(f"leg_{i}_b", s)
            box(rig, leg_a, side_name(f"stub{i}", s), (cx - half, top - stub_h, z - 1.5), (cx + half, top, z + 1.5), mat="skin_pink")
            foot_top = top - stub_h
            box(rig, leg_b, side_name(f"pad{i}", s), (cx - half - 0.4, 1.0, z - 2.5), (cx + half + 0.4, foot_top, z + 2.5), mat="skin_cream")
            for k, dz in enumerate((-1.5, 0.0, 1.5)):
                box(rig, leg_b, side_name(f"hook{i}_{k}", s), (cx - 0.4, 0.0, z + dz - 0.45), (cx + 0.4, 1.0, z + dz + 0.45), mat="rot_dark")


# ===========================================================================
# 7. 腐液滴挂 + 渍斑
# ===========================================================================
def part_ooze(rig: Rig) -> None:
    """腐缝底下挂 2~3 滴褐色腐液（长短不一），背上撒几块渍斑；都挂在 ooze_N 骨下，镜像豁免。"""
    rng = random.Random(7041)
    for i in range(N_SEG):
        a, y, z = SEG_A[i], seg_y(i), seg_z(i)
        a_groove = groove_a(i)
        bone = f"ooze_{i}"
        gz = z + SEG_SKIN_Z / 2 + GROOVE_Z / 2
        base_y = y - a_groove
        for n in range(rng.choice((2, 3))):
            x = rng.uniform(-0.22, 0.22) * a_groove
            h = rng.uniform(2.2, min(6.0, base_y - 0.5))
            box(rig, bone, f"drip{i}_{n}", (x - 0.5, base_y - h, gz - 0.6), (x + 0.5, base_y, gz + 0.6), mat="ichor_brown")
            if n == 0:
                box(rig, bone, f"drip_bead{i}", (x - 0.7, base_y - h - 1.4, gz - 0.7), (x + 0.7, base_y - h, gz + 0.7),
                    mat="ichor_brown")
        # 渍斑：贴在中段顶边外表面（y = Yc + A），落在 z+0.2..z+1.5，避开 z-1.5..z-0.1 那道褶皱棱
        for n in range(rng.choice((2, 3, 4))):
            x = rng.uniform(-0.2, 0.2) * a
            w, d = rng.uniform(1.6, 3.0), rng.uniform(0.8, 1.2)
            zz = z + 0.85
            box(rig, bone, f"stain{i}_{n}", (x - w / 2, y + a, zz - d / 2), (x + w / 2, y + a + 0.5, zz + d / 2), mat="ichor_brown")


# ===========================================================================
# 8. 尾环 + 呼吸管 + 气门板
# ===========================================================================
def part_tail(rig: Rig) -> None:
    """三节逐渐收细的尾环，末端一块塞板，上翘的三段呼吸管，管口是带辐条的气门板。"""
    for k, (a, t) in enumerate(TAIL_RINGS):
        zc = TAIL_Z0 + TAIL_RING_Z * (k + 0.5)
        ring(rig, f"telson_{k}", f"telson{k}", TAIL_Y, zc, a, t, TAIL_RING_Z,
             mat_for=skin_mat if k < 2 else (lambda ang: "skin_pink"))
    z_end = TAIL_Z0 + TAIL_RING_Z * len(TAIL_RINGS)
    box(rig, "telson_2", "tail_plug", (-3.4, TAIL_Y - 3.4, z_end), (3.4, TAIL_Y + 3.4, z_end + 1.5), mat="skin_pink")
    z0 = z_end + 1.5
    pts = [(0.0, TAIL_Y + dy, z0 + dz) for dy, dz in TUBE_PATH]
    # 四节管：前三节斜上，末节平伸
    bar(rig, "tube_0", "tube_seg0", pts[0], pts[1], TUBE_R[0], TUBE_R[0], mat="skin_pink")
    bar(rig, "tube_1", "tube_seg1", pts[1], pts[2], TUBE_R[1], TUBE_R[1], mat="skin_pink")
    bar(rig, "tube_2", "tube_seg2", pts[2], pts[3], TUBE_R[2], TUBE_R[2], mat="skin_pink")
    bar(rig, "tube_2", "tube_seg3", pts[3], pts[4], TUBE_R[3], TUBE_R[3], mat="skin_pink")
    # 气门板：褐色十二边形厚缘 + 暗色板心 + 三根象牙辐条
    zp = z0 + TUBE_PATH[-1][1]
    yp = TAIL_Y + TUBE_PATH[-1][0]
    ring(rig, "plate", "plate_rim", yp, zp + 0.8, 5.5, 1.6, 1.6, mat_for=lambda ang: "ichor_brown")
    box(rig, "plate", "plate_h", (-3.9, yp - 2.1, zp), (3.9, yp + 2.1, zp + 1.6), mat="rot_dark")
    box(rig, "plate", "plate_v", (-2.1, yp - 3.9, zp), (2.1, yp + 3.9, zp + 1.6), mat="rot_dark")
    for k, rz in enumerate((0.0, 60.0, 120.0)):
        rig.cube("plate", f"plate_spoke{k}", (-3.7, yp - 0.35, zp + 1.6), (3.7, yp + 0.35, zp + 2.3), rot=(0.0, 0.0, rz),
                 org=(0.0, yp, zp + 1.95), mat="tooth_ivory")


PARTS = {
    "head": ("1. 头壳：四瓣厚唇 + 感觉坑", part_head),
    "mouth": ("2. 咽口：三层倒钩齿环 + 咽喉", part_mouth),
    "palps": ("3. 头顶一对触须", part_palps),
    "body": ("4. 14 节皮环 / 腐缝 / 褶皱棱", part_body),
    "guts": ("5. 脏器与灵光碎块", part_guts),
    "legs": ("6. 12 对肉垫短足", part_legs),
    "ooze": ("7. 腐液滴挂 + 渍斑", part_ooze),
    "tail": ("8. 尾环 + 呼吸管 + 气门板", part_tail),
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
# 镜像门只认 _l/_r 成对。这些骨本来就不左右对称：脏器 / 腐液随机撒；齿环的齿有旋向。
ASYM = (tuple(f"gut_{i}" for i in range(N_SEG)) + tuple(f"ooze_{i}" for i in range(N_SEG))
        + tuple(f"ring_{k}" for k in range(len(MOUTH_RINGS))))


class CreatureGates(gatekit.AssetGates):
    """实体模型不受 0..16 方块空间限制（天道尸蛆纵长 170px），去掉越界门。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


# 穿模门按 AABB 判：十二边形皮环的斜边 AABB 很大，会和同一圈里的脏器 / 灵光 / 齿环 / 足桩虚报互穿。
# 这些「内含物 × 皮」配对放行；皮 × 皮、内含物 × 内含物仍照常查。
_SKIN = ("skin_cream", "skin_pink")
SOFT_OVER = frozenset(
    frozenset((skin, inner))
    for skin in _SKIN
    for inner in ("organ_red", "rot_dark", "tooth_ivory", "glint_amber")
)
GATES = CreatureGates("天道尸蛆 / tian_dao_shi_qu", tuple(MATS), asym=ASYM, soft_over=SOFT_OVER)


def self_test() -> None:
    print("运行 gen_tian_dao_shi_qu.py 门禁差分自证 ...")
    rig = build_full()
    lo, hi = rig.bounds()
    moving = moving_bones(rig)
    segs = [b for b in rig.bone_order if b.startswith("seg_")]

    assert len(segs) >= MIN_SEGMENTS, f"体节 {len(segs)} < {MIN_SEGMENTS}"
    assert len(moving) >= MIN_MOVING_BONES, f"可动骨 {len(moving)} < {MIN_MOVING_BONES}"
    assert abs(lo[1]) < 0.15, f"贴地失败：最低点 y={lo[1]:.3f}，爪钩应落在 0 附近"
    length = hi[2] - lo[2]
    assert 140.0 <= length <= 200.0, f"纵长 {length:.1f}px 超出 140..200"
    print(f"  [OK] 体节 {len(segs)} 可动骨 {len(moving)} cube {len(rig.elements)} 纵长 {length:.1f}px 高 {hi[1]:.1f}px 最低 y={lo[1]:.2f}")

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


# yaw 方向（枯皮狼那轮用标记块实测）：yaw=0 相机在 +Z（照到尾）；yaw=180 相机在 -Z（照到头）；
# yaw=90 相机在 -X（照到左侧，头朝画面左）；yaw=-90 相机在 +X（照到右侧，头朝画面右）。
def six_views(rig: Rig) -> list[tuple[str, float, float, tuple[tuple[float, float, float], float]]]:
    lo, hi = rig.bounds()
    whole = focus_of(rig)
    xy_span = max(hi[0] - lo[0], hi[1] - lo[1]) * 1.1
    head_focus = ((0.0, (lo[1] + hi[1]) / 2, lo[2] + 20.0), xy_span)
    tail_focus = ((0.0, (lo[1] + hi[1]) / 2, hi[2] - 20.0), xy_span)
    return [
        ("头吻面 FRONT：相机在 -Z，对准环形口器", 180.0, 0.0, head_focus),
        ("尾臀面 REAR：相机在 +Z，对准呼吸管", 0.0, 0.0, tail_focus),
        ("左侧面 LEFT：相机在 -X，照到 -X 面", 90.0, 0.0, whole),
        ("右侧面 RIGHT：相机在 +X，照到 +X 面", -90.0, 0.0, whole),
        ("脊背面 TOP：相机在 +Y，照到 +Y 面", 0.0, 89.9, whole),
        ("左前上 3/4：相机在 -X -Z +Y，对准头与前半身", 135.0, 25.0, ((-0.0, HEAD_Y - 6.0, seg_z(2)), 120.0)),
    ]


CLOSEUP_PARTS = frozenset({"body", "guts", "legs", "ooze"})
HEAD_PARTS = frozenset({"head", "mouth", "palps"})


def render_part_previews() -> None:
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(16), load_font(13)
    print("渲染逐件预览 ...")
    for i, (key, (title, func)) in enumerate(PARTS.items(), start=1):
        rig = build_rig()
        func(rig)
        path = _tmp_model(rig, f"part_{key}")
        if key in CLOSEUP_PARTS:
            focus = ((0.0, seg_y(4), seg_z(4)), 62.0)       # 最粗的第 4 节特写
        elif key in HEAD_PARTS:
            focus = ((0.0, HEAD_Y, HEAD_Z - 2.0), 50.0)
        else:
            focus = ((0.0, TAIL_Y + 5.0, TAIL_Z0 + 12.0), 52.0)
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
    """版面：第一行两个端面 + 3/4（方格）；其余三个长条视角（左 / 右 / 脊背）各占整行宽幅。"""
    font_h, font_l = load_font(18), load_font(13)
    width = cell * 3 + 10
    band = 500
    views = six_views(rig)
    sheet = Image.new("RGB", (width + 10, 34 + cell + 10 + 3 * (band + 10) + 6), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), header, fill=(235, 235, 235), font=font_h)

    def tag(px: int, py: int, label: str) -> None:
        d.rectangle([px, py, px + 340, py + 22], fill=(22, 23, 26))
        d.text((px + 6, py + 3), label, fill=(235, 235, 235), font=font_l)

    for slot, idx in enumerate((0, 1, 5)):
        label, yaw, pitch, focus = views[idx]
        px, py = 5 + slot * (cell + 5), 34
        im, _ = render(path, yaw=yaw, pitch=pitch, size=cell, bg=BG, focus=focus)
        sheet.paste(im, (px, py))
        tag(px, py, label)
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
    """骨骼树。体节链每节的父骨是上一节，写成平铺的一列；seg_0（带头）、seg_3（带足与脏器）、seg_13（带尾）展开，
    其余体节只写一行并注明结构同 seg_3。"""
    children: dict[str | None, list[str]] = {}
    for n in rig.bone_order:
        children.setdefault(rig.bones[n]["parent"], []).append(n)
    lines = ["root"]

    def walk(name: str, depth: int) -> None:
        lines.append(f"{'  ' * depth}└ {name} ({len(rig.bones[name]['children'])})")
        for kid in children.get(name, []):
            if not kid.startswith("seg_"):
                walk(kid, depth + 1)

    for i in range(N_SEG):
        name = f"seg_{i}"
        if i in (0, 3, N_SEG - 1):
            walk(name, 1)
        else:
            others = [k for k in children[name] if not k.startswith("seg_")]
            sub = sum(1 + _count(k, children) for k in others)
            lines.append(f"  └ {name} ({len(rig.bones[name]['children'])}) ＋{sub} 根子骨，结构同 seg_3")
    return lines


def _count(name: str, children: dict[str | None, list[str]]) -> int:
    return sum(1 + _count(k, children) for k in children.get(name, []))


def bone_tree_panel(rig: Rig, height: int) -> Image.Image:
    lines = bone_tree_lines(rig)
    font_h, font_s, font_t = load_font(16), load_font(13), load_font(11)
    step = 13
    panel = Image.new("RGB", (430, max(height, 90 + len(lines) * step)), (32, 33, 37))
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
    ap = argparse.ArgumentParser(description="天道尸蛆 (tian_dao_shi_qu) 生成器 Round 1/3")
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
    doc = rig.bbmodel("TianDaoShiQu", namespace="bong")
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
