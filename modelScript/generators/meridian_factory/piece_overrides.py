#!/usr/bin/env python3
"""个别模型的专属贴图画笔（几何不动，只换某几个立方体的贴图）。

标准材质画笔（pixel_texture.py）管「这种材质长什么样」；这里管「这个模型的这几块要画特定图案」：
- b03 端口：碗底三条亮筋从碗沿汇聚到中心，中心最亮；
- b08 联轴：喇叭口内壁画放射亮纹，从中心往外散；
- b04 外壁罐：膜面上 7 个发光囊泡；
- b06 缓冲槽：内壁膜面放射状细筋。

用法：retexture.py 把这里的函数登记成 Variant.override，传给 write_textured_bbmodel。
"""

from __future__ import annotations

import math
from typing import Optional

from pixel_texture import (
    FIBER_DARK,
    FIBER_NET,
    FLESH_LIT,
    hex_color,
    TUBE_CENTER,
    TUBE_HIGHLIGHT,
    Color,
    Painter,
    Vec,
    mix,
    shade,
    paint_flesh,
    texel_noise,
    value_noise,
)

# ---- b03 端口 ----------------------------------------------------------------

PORT_BOWL_CENTER = (0.0, 0.0)       # 碗心在 xz 平面的位置
PORT_RIB_REACH = 5.0                # 亮筋从碗沿到碗心的半径（px）
PORT_BRIGHT_RIBS = ("n", "se", "sw")  # 三条亮筋：各条放射筋按方向名区分，彼此约 120° 错开
PORT_WELL_RADIUS = 3.0              # 碗底孔（6×6）的半宽


def _radial(p: Vec) -> float:
    return math.hypot(p[0] - PORT_BOWL_CENTER[0], p[2] - PORT_BOWL_CENTER[1])


def _bright_rib(p: Vec, axis: int) -> Color:
    """亮筋：碗沿一端 #e8bca8，越靠碗心越亮，中心 #f6dcc4。"""
    closeness = max(0.0, 1.0 - _radial(p) / PORT_RIB_REACH)
    color = mix(TUBE_HIGHLIGHT, TUBE_CENTER, closeness ** 0.7)
    return mix(color, TUBE_HIGHLIGHT, 0.12 * texel_noise(p, 101))


def _well_glow(p: Vec, axis: int) -> Color:
    """碗底中心的亮点：中心 #f6dcc4，向孔边回落到亮肉色 #b05050。"""
    closeness = max(0.0, 1.0 - _radial(p) / PORT_WELL_RADIUS)
    return mix(FLESH_LIT, TUBE_CENTER, closeness ** 0.8)


def port_override(cube: dict, default: Painter) -> Optional[Painter]:
    name = cube["name"]
    if "dark_well_bottom" in name:
        return _well_glow
    if "_ridge_" in name:
        direction = name.split("_")[2]      # "in_ridge_n_outer" -> "n"
        if direction in PORT_BRIGHT_RIBS:
            return _bright_rib
    return None


# ---- b08 联轴 ----------------------------------------------------------------

COUPLING_AXIS_Y = 5.0       # 喇叭口轴线在 y 方向的位置（x = 0）
COUPLING_Z_START = -3.0     # 喇叭底的 z
COUPLING_Z_SPAN = 11.0      # 喇叭口从底到口沿的 z 长度
COUPLING_RAYS = 16          # 放射亮纹条数
RAY_HALF_WIDTH = 0.3
RAY_GLOW_WIDTH = 0.45


def _inner_wall_streaks(p: Vec, axis: int) -> Color:
    """内壁：亮肉底色上画放射亮纹。纹路在喇叭底（中心）最亮最粗，往口沿渐弱。"""
    base = mix(paint_flesh(p, axis), FLESH_LIT, 0.5)
    dx, dy = p[0], p[1] - COUPLING_AXIS_Y
    radius = math.hypot(dx, dy)
    sector = 2 * math.pi / COUPLING_RAYS
    offset = ((math.atan2(dy, dx) + sector / 2) % sector) - sector / 2
    distance = abs(offset) * radius        # 到最近一条射线的弧长（px）
    depth = min(1.0, max(0.0, (p[2] - COUPLING_Z_START) / COUPLING_Z_SPAN))
    strength = 1.0 - 0.5 * depth
    if distance < RAY_HALF_WIDTH + 0.2 * (1 - depth):
        return mix(base, TUBE_CENTER, 0.9 * strength)
    if distance < RAY_HALF_WIDTH + RAY_GLOW_WIDTH:
        return mix(base, TUBE_HIGHLIGHT, 0.5 * strength)
    return base


def coupling_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") == "flaring_bell" and cube.get("material") == "flesh_lit":
        return _inner_wall_streaks
    return None


# ---- b04 外壁罐：膜面上的发光囊泡 ---------------------------------------------

VESICLE_RING = hex_color("#8a4a40")     # 外圈 1px 深环
VESICLE_BODY = hex_color("#f6dcc4")     # 球体
VESICLE_HEART = hex_color("#fff8ec")    # 正中 2×2 最亮
VESICLE_RING_WIDTH = 1.0
VESICLE_HEART_RADIUS = 0.5              # 2×2 texel（= 1px 见方）的半径
# (x, y, z, 球体半径)：落在 12×12×12 筋囊四个侧面（x/z = ±6）的 8×8 膜面上，共 7 个，
# 南面 2、东面 2、西面 2、北面 1，大小略有差别；含外环的总半径不超过 2.2，同面两球不相碰
VESICLES = (
    (-2.2, 8.0, 6.0, 1.1), (2.3, 5.6, 6.0, 0.9),
    (6.0, 9.0, -1.8, 1.0), (6.0, 6.0, 2.0, 1.2),
    (-6.0, 8.5, 1.5, 0.9), (-6.0, 5.2, -2.2, 1.1),
    (1.0, 7.0, -6.0, 1.2),
)


def _vesicle_painter(default: Painter) -> Painter:
    """在默认的膜面画笔上叠囊泡，由外到内：1px 深环 #8a4a40 → 球体 #f6dcc4 → 正中 2×2 #fff8ec。"""

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        for cx, cy, cz, radius in VESICLES:
            distance = math.dist(p, (cx, cy, cz))
            if distance < VESICLE_HEART_RADIUS:
                return VESICLE_HEART
            if distance < radius:
                return VESICLE_BODY
            if distance < radius + VESICLE_RING_WIDTH:
                return VESICLE_RING
        return color

    return paint


def tank_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("material") == "tendon_highlight" and "glow_wall" in cube["name"]:
        return _vesicle_painter(default)
    return None


# ---- b06 缓冲槽：内壁放射状细筋 -----------------------------------------------

BUFFER_RAYS = 14
BUFFER_RAY_HALF_WIDTH = 0.18
BUFFER_RAY_GLOW_WIDTH = 0.4


def _radial_ribs_painter(cube: dict, default: Painter) -> Painter:
    """膜面中心往外的放射细筋：#c07868 细线，两侧各描一道 #e8bca8。只画在膜面最大的两个面上。"""
    lo, hi = cube["from"], cube["to"]
    size = [hi[i] - lo[i] for i in range(3)]
    thin_axis = size.index(min(size))
    plane = [i for i in range(3) if i != thin_axis]
    center = [(lo[i] + hi[i]) / 2 for i in plane]
    sector = 2 * math.pi / BUFFER_RAYS

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis != thin_axis:
            return color
        du, dv = p[plane[0]] - center[0], p[plane[1]] - center[1]
        radius = math.hypot(du, dv)
        offset = ((math.atan2(dv, du) + sector / 2) % sector) - sector / 2
        distance = abs(offset) * radius
        if distance < BUFFER_RAY_HALF_WIDTH + 0.04 * radius:
            return mix(color, FIBER_NET, 0.95)
        if distance < BUFFER_RAY_HALF_WIDTH + BUFFER_RAY_GLOW_WIDTH:
            return mix(color, VESICLE_RING, 0.6)
        return color

    return paint


def buffer_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("material") == "tendon_highlight" and "glow_wall" in cube["name"]:
        return _radial_ribs_painter(cube, default)
    return None


# ---- 线条类专属图案（#c07868 细线）-----------------------------------------------

LINE_COLOR = hex_color("#c07868")
LINE_MIX = 0.92


def _segment_distance(px: float, py: float, a: tuple, b: tuple) -> float:
    """点到线段 ab 的距离（平面内）。"""
    ax, ay = a
    bx, by = b
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


# ---- o01 肺：肺叶正面的支气管树 ------------------------------------------------

# 右肺（x<0）的树枝，左肺取镜像。每段 (起点, 终点, 线宽 px)，坐标为 (x, y)。
# 骨质主支（几何）在 y 18..31：下支末端在 (-13.5, 18)，上支末端在 (-9.5, 30.5)。
# 画的枝从这两个末端长出来：1 级 2px 主枝，2、3 级 1px 细枝。
LUNG_DOWN_END = (-13.5, 18.0)
LUNG_UP_END = (-9.5, 30.5)
LUNG_BRANCHES = (
    # 下支末端 → 向下分叉
    (LUNG_DOWN_END, (-14.5, 15.0), 2.0), (LUNG_DOWN_END, (-11.5, 15.0), 2.0),
    ((-14.5, 15.0), (-15.5, 12.5), 1.0), ((-14.5, 15.0), (-13.5, 12.0), 1.0),
    ((-11.5, 15.0), (-12.0, 12.5), 1.0), ((-11.5, 15.0), (-9.5, 12.5), 1.0),
    ((-15.5, 12.5), (-16.0, 10.6), 1.0), ((-13.5, 12.0), (-14.0, 10.6), 1.0),
    ((-12.0, 12.5), (-11.5, 10.6), 1.0), ((-9.5, 12.5), (-8.0, 10.6), 1.0),
    # 上支末端 → 向外、向上分叉
    (LUNG_UP_END, (-12.5, 28.5), 2.0), (LUNG_UP_END, (-9.5, 33.0), 2.0),
    ((-12.5, 28.5), (-15.0, 28.0), 1.0), ((-12.5, 28.5), (-14.0, 25.0), 1.0),
    ((-9.5, 33.0), (-6.5, 35.0), 1.0), ((-9.5, 33.0), (-6.0, 31.5), 1.0),
    ((-15.0, 28.0), (-15.5, 25.5), 1.0), ((-14.0, 25.0), (-16.0, 23.5), 1.0),
    ((-6.5, 35.0), (-4.5, 35.5), 1.0), ((-6.0, 31.5), (-4.0, 33.0), 1.0),
)
LUNG_FRONT_Z = 10.0     # 只画在肺叶前后面（法线沿 z）上，z 绝对值大于这个值的面


def _lung_branch_painter(default: Painter) -> Painter:
    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis != 2 or abs(p[2]) < LUNG_FRONT_Z:
            return color
        x = -abs(p[0])      # 左右肺镜像：折到右肺
        for a, b, width in LUNG_BRANCHES:
            if _segment_distance(x, p[1], a, b) < width / 2:
                return mix(color, LINE_COLOR, LINE_MIX)
        return color

    return paint


def lung_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") in ("right_lobe", "left_lobe"):
        return _lung_branch_painter(default)
    return None


# ---- o03 臂肌：每个肌束 3~4 条贯穿全长的平行弧线 ---------------------------------------

FIBER_LINE_WIDTH = 0.3
FIBER_SKIP_CHANCE = 0.3     # o04 腿肌的纤维线会随机断开几条
ARM_ARC_OFFSETS = (-0.62, -0.22, 0.22, 0.62)    # 弧线在肌束截面里的相对位置（-1..1，占半宽的比例）
ARM_ARC_BULGE = 0.15                            # 弧线随肌腹隆起外扩的幅度
ARM_Z_START, ARM_Z_END = 8.0, 47.0              # 肌束沿 z 的全长，弧线的隆起相位按它算，各段首尾相接


def _muscle_fiber_painter(cube: dict, default: Painter) -> Painter:
    """肌束沿 z 走：顶 / 底面上的弧线沿 x 排布，左 / 右侧面上沿 y 排布。
    线位置按「截面半宽的比例」给定，所以肌束各段变粗变细时线跟着走，整条不断；比例里再叠一个沿全长的
    正弦隆起，让平行线呈弧形外鼓。"""
    lo, hi = cube["from"], cube["to"]

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis == 2:
            return color
        across = 0 if axis == 1 else 1          # 顶底面看 x，侧面看 y
        center = (lo[across] + hi[across]) / 2
        half = (hi[across] - lo[across]) / 2
        phase = math.pi * min(1.0, max(0.0, (p[2] - ARM_Z_START) / (ARM_Z_END - ARM_Z_START)))
        for offset in ARM_ARC_OFFSETS:
            line = center + offset * (1 + ARM_ARC_BULGE * math.sin(phase)) * half
            if abs(p[across] - line) < FIBER_LINE_WIDTH:
                return mix(color, LINE_COLOR, 0.85)
        return color

    return paint


def arm_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") == "muscle_bundles" and cube.get("material") in ("flesh_main", "flesh_lit") \
            and not cube["name"].endswith("_hl"):
        return _muscle_fiber_painter(cube, default)
    return None


# ---- b09 地砖：血管线 ---------------------------------------------------------

FLOOR_VESSEL_CORE = hex_color("#8a2a2a")
FLOOR_VESSEL_LIGHT = hex_color("#c07868")   # 血管一侧 1px 高光
FLOOR_VESSEL_SHADE = hex_color("#5a1a1a")   # 另一侧 1px 暗边
FLOOR_VESSEL_CORE_HALF = 0.5                # 芯 1px；加两侧各 1px，总宽 3px
FLOOR_VESSEL_RIM = 1.0
FLOOR_NOISE_KEEP = 0.5                      # 底色噪点幅度压到原来的一半，血管才跳得出来
FLOOR_BASE = hex_color("#8a2a2a")           # 压噪点时向它收拢
FLOOR_TOP_Y = 16.0              # 地砖肉块顶面高度
FLOOR_EDGE_DARK_DEPTH = 1.0     # 侧面顶沿压暗的宽度（px）
FLOOR_EDGE_DARK = 0.85          # 顶沿压暗时混入 #5a1a1a 的比例
FLOOR_CURVE_STEPS = 96          # 把平滑曲线采样成折线，每个 texel 取到折线的最近点

# 3 条沿 z 贯穿整面的主血管：x(z) = 中线 + 振幅 * sin(2π z / 波长 + 相位)；之间的间距 ≥ 4px
FLOOR_VESSELS = (
    # (x 中线, 振幅, 波长, 相位)
    (-5.0, 1.3, 20.0, 0.4),
    (0.0, 1.5, 16.0, 2.3),
    (5.0, 1.3, 22.0, 4.1),
)
# 一处分叉：从第 1 条血管上的分叉点出发，二次贝塞尔曲线弯向 -x 侧的边缘
FLOOR_FORK_FROM_VESSEL = 0
FLOOR_FORK_Z = -1.5
FLOOR_FORK_BULGE = (-5.2, 4.0)    # 控制点 (x, z)
FLOOR_FORK_END = (-8.0, 6.5)      # 终点 (x, z)，落在 -x 边缘


def _vessel_x(vessel: tuple, z: float) -> float:
    center, amplitude, wavelength, phase = vessel
    return center + amplitude * math.sin(2 * math.pi * z / wavelength + phase)


def _sample_vessel(vessel: tuple) -> list:
    half = 8.0
    return [(_vessel_x(vessel, z), z) for z in
            (-half + 2 * half * i / FLOOR_CURVE_STEPS for i in range(FLOOR_CURVE_STEPS + 1))]


def _sample_fork() -> list:
    start = (_vessel_x(FLOOR_VESSELS[FLOOR_FORK_FROM_VESSEL], FLOOR_FORK_Z), FLOOR_FORK_Z)
    points = []
    for i in range(FLOOR_CURVE_STEPS // 2 + 1):
        t = i / (FLOOR_CURVE_STEPS // 2)
        points.append(tuple((1 - t) ** 2 * start[k] + 2 * (1 - t) * t * FLOOR_FORK_BULGE[k] + t * t * FLOOR_FORK_END[k]
                            for k in range(2)))
    return points


FLOOR_FORK_START = (_vessel_x(FLOOR_VESSELS[FLOOR_FORK_FROM_VESSEL], FLOOR_FORK_Z), FLOOR_FORK_Z)
FLOOR_FORK_GAP_RADIUS = 2.5     # 分叉点周围这个半径内，主血管的高光断开
FLOOR_CURVES = [_sample_vessel(v) for v in FLOOR_VESSELS] + [_sample_fork()]


def _curve_offset(curve: list, x: float, z: float) -> float:
    """点到折线的有符号距离：取最近线段，符号看点落在行进方向（沿 z 增大）的哪一侧，+x 侧为正。"""
    best, best_offset = 1e9, 0.0
    for (ax, az), (bx, bz) in zip(curve, curve[1:]):
        dx, dz = bx - ax, bz - az
        length_sq = dx * dx + dz * dz
        t = max(0.0, min(1.0, ((x - ax) * dx + (z - az) * dz) / length_sq))
        qx, qz = ax + t * dx, az + t * dz
        distance = math.hypot(x - qx, z - qz)
        if distance < best:
            length = math.sqrt(length_sq)
            normal = (dz / length, -dx / length)
            best = distance
            best_offset = (x - qx) * normal[0] + (z - qz) * normal[1]
    return best_offset


def _vessel_color(color: Color, offset: float) -> Color:
    """芯 #8a2a2a（1px）+ 正侧 1px #e8bca8 高光 + 负侧 1px #5a1a1a 暗边。"""
    if abs(offset) < FLOOR_VESSEL_CORE_HALF:
        return mix(color, FLOOR_VESSEL_CORE, 0.95)
    if 0 < offset < FLOOR_VESSEL_CORE_HALF + FLOOR_VESSEL_RIM:
        return mix(color, FLOOR_VESSEL_LIGHT, 0.95)
    if -(FLOOR_VESSEL_CORE_HALF + FLOOR_VESSEL_RIM) < offset < 0:
        return mix(color, FLOOR_VESSEL_SHADE, 0.95)
    return color


def _floor_painter(default: Painter) -> Painter:
    """平整肉块：底色噪点压半；顶面画 3 条贯穿的平滑血管 + 一处分叉；侧面纯底色，只在顶沿压暗 1px。"""

    def paint(p: Vec, axis: int) -> Color:
        color = mix(default(p, axis), FLOOR_BASE, 1 - FLOOR_NOISE_KEEP)
        if axis == 1:
            if p[1] < FLOOR_TOP_Y - 1e-6:
                return color
            for index, curve in enumerate(FLOOR_CURVES):
                offset = _curve_offset(curve, p[0], p[2])
                if abs(offset) >= FLOOR_VESSEL_CORE_HALF + FLOOR_VESSEL_RIM:
                    continue
                near_fork = math.hypot(p[0] - FLOOR_FORK_START[0], p[2] - FLOOR_FORK_START[1]) < FLOOR_FORK_GAP_RADIUS
                if index == FLOOR_FORK_FROM_VESSEL and offset > 0 and near_fork:
                    continue    # 分叉处主血管的高光让位，不和分支的高光并在一起
                return _vessel_color(color, offset)
            return color
        if p[1] > FLOOR_TOP_Y - FLOOR_EDGE_DARK_DEPTH:
            return mix(color, FLOOR_VESSEL_SHADE, FLOOR_EDGE_DARK)
        return color

    return paint


def floor_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") == "base_block":
        return _floor_painter(default)
    return None


# ---- o02 胃：胃液高光 ---------------------------------------------------------

LIQUID_HIGHLIGHT = hex_color("#f6dcc4")
LIQUID_RING = hex_color("#e8bca8")
LIQUID_RING_WIDTH = 0.5
# 泡沫光斑：胃液面上 7 个大小不一的圆斑，(x, y, 半径 px)，按液面正面（法线 z）的绝对坐标
FOAM_SPOTS = (
    (1.0, 28.5, 1.0), (-6.0, 25.5, 1.8), (6.0, 24.5, 1.1), (-2.0, 22.0, 2.3),
    (8.0, 20.5, 1.4), (-7.0, 19.0, 1.1), (2.0, 17.5, 1.7),
)


def _liquid_foam_painter(cube: dict, default: Painter) -> Painter:
    lo, hi = cube["from"], cube["to"]
    thin_axis = [hi[i] - lo[i] for i in range(3)].index(min(hi[i] - lo[i] for i in range(3)))

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis != thin_axis:
            return color
        for x, y, radius in FOAM_SPOTS:
            distance = math.hypot(p[0] - x, p[1] - y)
            if distance < radius:
                return mix(color, LIQUID_HIGHLIGHT, 0.95)
            if distance < radius + LIQUID_RING_WIDTH:
                return mix(color, LIQUID_RING, 0.85)
        return color

    return paint


def stomach_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("material") == "amber_fluid":
        return _liquid_foam_painter(cube, default)
    return None


# =============================================================================
# 第 2 批
# =============================================================================

def _face_radius(cube: dict, p: Vec, axis: int) -> float:
    """点在所在面上的归一化径向距离：面中心 0，面边缘 1（取两个面内方向里较大的那个）。"""
    lo, hi = cube["from"], cube["to"]
    others = [i for i in range(3) if i != axis]
    return max(abs(p[i] - (lo[i] + hi[i]) / 2) / max((hi[i] - lo[i]) / 2, 1e-6) for i in others)


# ---- 血管：o06 肝叶脉 / o07 脾血管 / o08 肾血管 ---------------------------------

def _vein_painter(cube: dict, default: Painter) -> Painter:
    """血管：#c07868 本色，沿血管走向的中线描一道 #e8bca8 高光。"""
    lo, hi = cube["from"], cube["to"]
    size = [hi[i] - lo[i] for i in range(3)]
    long_axis = size.index(max(size))

    def paint(p: Vec, axis: int) -> Color:
        color = mix(LINE_COLOR, FIBER_DARK, 0.25 * value_noise(p, 1.4, 131))
        if axis == long_axis:
            return color
        across = 3 - long_axis - axis
        if abs(p[across] - (lo[across] + hi[across]) / 2) < 0.22:
            return mix(color, TUBE_HIGHLIGHT, 0.8)
        return color

    return paint


def liver_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") == "leaf_veins":
        return _vein_painter(cube, default)
    return None


def spleen_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") == "veins":
        return _vein_painter(cube, default)
    return None


# ---- o05 心：余烬斑高对比亮暗渐变 ------------------------------------------------

EMBER_EDGE = hex_color("#3a0e0a")
EMBER_MID = hex_color("#c84a3a")
EMBER_CORE = hex_color("#ffd090")
EMBER_HOT = hex_color("#fff0dc")


def _ember_painter(cube: dict) -> Painter:
    """余烬：外缘近黑的暗红 → #c84a3a → 中心发白的橙黄，亮暗差拉到最大；texel 级闪烁。"""

    def paint(p: Vec, axis: int) -> Color:
        brightness = 1.0 - _face_radius(cube, p, axis) ** 1.3
        brightness += (texel_noise(p, 141) - 0.5) * 0.12
        brightness = max(0.0, min(1.0, brightness))
        if brightness < 0.5:
            return mix(EMBER_EDGE, EMBER_MID, brightness / 0.5)
        if brightness < 0.85:
            return mix(EMBER_MID, EMBER_CORE, (brightness - 0.5) / 0.35)
        return mix(EMBER_CORE, EMBER_HOT, (brightness - 0.85) / 0.15)

    return paint


# 心肌表面的发光血管网：主干从顶部三根大血管下来，分 3 级。只画在朝前（+z）的面上。
HEART_BASE = hex_color("#7a2424")       # 心肌底色压暗，血管网才亮得出来
VESSEL_CORE = hex_color("#f6dcc4")
VESSEL_SIDE = hex_color("#e8bca8")
VESSEL_SIDE_WIDTH = 0.45                # 线芯两侧高光的宽度
# 三根主干的起点 (x, y)：对着顶部三根大血管（x = 0、-9、+9）的根部
HEART_TRUNK_ROOTS = ((0.0, 39.0), (-9.0, 38.0), (9.0, 37.0))
# 每级：(长度, 线芯半宽, 分叉数, 分叉半角度)
HEART_BRANCH_LEVELS = (
    (9.0, 0.55, 2, 26.0),
    (6.5, 0.42, 3, 22.0),
    (4.5, 0.32, 0, 0.0),
)
# 余烬斑：几何里是 6×6 的 ember_plate，贴图上把光晕画到两倍大
EMBER_CENTER = (0.0, 19.0)
EMBER_GLOW_RADIUS = 6.0
HEART_FRONT_Z = 5.0
HEART_MUSCLE_GROUPS = ("ventricles", "atria")


def _heart_segments() -> list:
    """主干 + 分叉的线段表：(起点, 终点, 线芯半宽)。方向用「偏离正下方的角度」，分叉左右对称展开，
    主干朝心尖（余烬斑）略微收拢。"""
    segments = []

    def grow(x: float, y: float, heading: float, level: int):
        length, half_width, forks, spread = HEART_BRANCH_LEVELS[level]
        end = (x + length * math.sin(math.radians(heading)), y - length * math.cos(math.radians(heading)))
        segments.append(((x, y), end, half_width))
        if forks == 0:
            return
        for k in range(forks):
            offset = (k - (forks - 1) / 2) * 2 * spread / max(forks - 1, 1)
            grow(end[0], end[1], heading + offset, level + 1)

    for root_x, root_y in HEART_TRUNK_ROOTS:
        toward_center = -math.degrees(math.atan2(root_x, 20.0)) * 0.5   # 左边的向右收，右边的向左收
        grow(root_x, root_y, -toward_center, 0)
    return segments


HEART_SEGMENTS = _heart_segments()


def _heart_surface_painter(default: Painter) -> Painter:
    def paint(p: Vec, axis: int) -> Color:
        color = mix(default(p, axis), HEART_BASE, 0.7)
        # 前半个心的所有朝向都画：心肌是逐行堆的台阶，只画朝前的面，网会被台阶的朝上面截断
        if p[2] < HEART_FRONT_Z:
            return color

        glow = math.hypot(p[0] - EMBER_CENTER[0], p[1] - EMBER_CENTER[1]) / EMBER_GLOW_RADIUS
        if glow < 1.0:
            warmth = (1.0 - glow) ** 1.5
            color = mix(color, mix(EMBER_MID, EMBER_CORE, warmth), min(1.0, warmth * 1.5))

        for start, end, half_width in HEART_SEGMENTS:
            distance = _segment_distance(p[0], p[1], start, end)
            if distance < half_width:
                return VESSEL_CORE
            if distance < half_width + VESSEL_SIDE_WIDTH:
                color = mix(color, VESSEL_SIDE, 0.9)
        return color

    return paint


def heart_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("material") == "fire":
        return _ember_painter(cube)
    if cube.get("group") in HEART_MUSCLE_GROUPS:
        return _heart_surface_painter(default)
    return None


# ---- b11 阀门：括约肌环放射褶纹 ---------------------------------------------------

VALVE_AXIS_Y = 5.0          # 管腔中线（y），环绕 z 轴
VALVE_FOLDS = 18            # 放射褶数
FLESH_FOLD_LIT = hex_color("#b05050")
FLESH_FOLD_DARK = hex_color("#4a1414")


LENGTHWISE_FOLD_PERIOD = 1.4    # 侧面纵向褶纹的间距（px），褶沿 z（管轴）方向走


def _lengthwise_fold(color: Color, p: Vec, axis: int) -> Color:
    """括约肌环的外侧面（顶 / 底 / 左 / 右）：沿管轴方向的纵向褶纹，比端面的放射褶淡一点。"""
    across = p[0] if axis == 1 else p[1]        # 顶底面上褶沿 x 排列，左右面上沿 y 排列
    wave = math.cos(2 * math.pi * across / LENGTHWISE_FOLD_PERIOD)
    if wave > 0.4:
        return mix(color, FLESH_FOLD_LIT, 0.35)
    if wave < -0.4:
        return mix(color, FLESH_FOLD_DARK, 0.45)
    return color


def _radial_fold_painter(default: Painter) -> Painter:
    """括约肌环的前后端面：绕管腔一圈放射褶，亮褶 #b05050 与暗沟交替，越靠孔口越暗。"""

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis != 2:
            return _lengthwise_fold(color, p, axis)
        dx, dy = p[0], p[1] - VALVE_AXIS_Y
        wave = math.cos(VALVE_FOLDS * math.atan2(dy, dx))
        radius = math.hypot(dx, dy)
        color = mix(color, FLESH_FOLD_DARK, max(0.0, 0.5 - radius / 20.0))     # 近孔口压暗
        if wave > 0.35:
            return mix(color, FLESH_FOLD_LIT, 0.65)
        if wave < -0.35:
            return mix(color, FLESH_FOLD_DARK, 0.75)
        return color

    return paint


def valve_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") in ("sphincter", "sphincter_folds"):
        return _radial_fold_painter(default)
    return None


# ---- b14 损伤三态：裂纹 / 断口 / 污染血块 -----------------------------------------

CRACK_CORE = hex_color("#120505")
CRACK_RIM = hex_color("#c07868")
CRACK_RIM_WIDTH = 0.25
WOUND_EDGE = hex_color("#6a1a1a")
WOUND_MID = hex_color("#c84040")
WOUND_CORE = hex_color("#f0a090")
CLOT_SHEEN = hex_color("#9a5a90")


def _crack_painter(cube: dict) -> Painter:
    """裂纹：近黑深沟，沟沿一圈 #c07868 的翻开的肉边，沟与周围筋管反差拉满。"""
    lo, hi = cube["from"], cube["to"]

    def paint(p: Vec, axis: int) -> Color:
        in_plane = [i for i in range(3) if i != axis]
        edge = min(min(p[i] - lo[i], hi[i] - p[i]) for i in in_plane)
        if edge < CRACK_RIM_WIDTH:
            return mix(CRACK_CORE, CRACK_RIM, 0.85)
        return mix(CRACK_CORE, FLESH_FOLD_DARK, 0.4 * value_noise(p, 1.2, 151))

    return paint


def _wound_painter(cube: dict) -> Painter:
    """断口：湿红的肉面，中心偏亮，间杂 #e8bca8 的断筋头。"""

    def paint(p: Vec, axis: int) -> Color:
        brightness = 1.0 - _face_radius(cube, p, axis) ** 1.2
        color = mix(WOUND_EDGE, WOUND_MID, min(1.0, brightness * 1.6))
        if brightness > 0.5:
            color = mix(color, WOUND_CORE, (brightness - 0.5) * 1.2)
        if texel_noise(p, 161) < 0.1:
            color = mix(color, TUBE_HIGHLIGHT, 0.85)
        return color

    return paint


def _clot_painter(cube: dict, default: Painter) -> Painter:
    """污染血块：深紫黑底，表面带湿润的亮斑，边缘一圈暗，反差比平色版大得多。"""
    lo, hi = cube["from"], cube["to"]

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        in_plane = [i for i in range(3) if i != axis]
        edge = min(min(p[i] - lo[i], hi[i] - p[i]) for i in in_plane)
        if edge < 0.25:
            return shade(color, -0.45)
        if texel_noise(p, 171) < 0.09:
            return mix(color, CLOT_SHEEN, 0.8)
        return shade(color, (value_noise(p, 1.3, 172) - 0.5) * 0.5)

    return paint


def damage_override(cube: dict, default: Painter) -> Optional[Painter]:
    group = cube.get("group")
    if group == "cracks":
        return _crack_painter(cube)
    if group == "sever_ends":
        return _wound_painter(cube)
    if group in ("blood_clots", "taint_veins"):
        return _clot_painter(cube, default)
    return None


# ---- b13 收缩口：肉褶脊、褶间暗沟、口内亮芯 -------------------------------------------

RIDGE_COLOR = hex_color("#b05050")      # 肉褶脊
GROOVE_COLOR = hex_color("#5a1a1a")     # 褶间压暗
APERTURE_EDGE = hex_color("#f6dcc4")    # 口内亮芯：边缘
APERTURE_CENTER = hex_color("#fff8ec")  # 口内亮芯：正中


def _fold_painter(base: Color, seed: int) -> Painter:
    """褶脊 / 褶间：一个固定底色，只留很轻的明暗起伏，两种底色的反差就是「褶」。"""

    def paint(p: Vec, axis: int) -> Color:
        return shade(base, (value_noise(p, 1.6, seed) - 0.5) * 0.18 + (texel_noise(p, seed + 1) - 0.5) * 0.06)

    return paint


def _aperture_core_painter(cube: dict) -> Painter:
    """口内亮芯：边缘 #f6dcc4 渐变到正中 #fff8ec。"""

    def paint(p: Vec, axis: int) -> Color:
        return mix(APERTURE_EDGE, APERTURE_CENTER, 1.0 - _face_radius(cube, p, axis))

    return paint


def emitter_override(cube: dict, default: Painter) -> Optional[Painter]:
    group, material = cube.get("group"), cube.get("material")
    if group == "dome" and material == "flesh_lit":
        return _fold_painter(RIDGE_COLOR, 181)
    if group == "dome" and material == "flesh_dark":
        return _fold_painter(GROOVE_COLOR, 183)
    if group == "aperture" and material == "qi_glow":
        return _aperture_core_painter(cube)
    return None


# ---- b05 分流器 / b12 合流器：中心腔十字亮通道 + 肌肉卷肋环 ----------------------------

CROSS_CENTER = hex_color("#fff0dc")
CROSS_BASE = hex_color("#a86858")       # 通道往臂端渐变到的底色，也是顶膜底色
CROSS_REACH = 4.0                       # 十字从中心到腔壁的距离（px）
MEMBRANE_KEEP = 0.2                     # 顶膜保留多少默认膜面的起伏，其余用底色压平

RIB_LIT_COLOR = hex_color("#8a2a2a")    # 偶数肋
RIB_DARK_COLOR = hex_color("#5a1a1a")   # 奇数肋
RIB_HIGHLIGHT = hex_color("#c86a5a")
RIB_RIM_WIDTH = 0.5                     # 端面外圈高光的宽度（px，= 1 texel）


def _cross_gradient_painter(default: Painter) -> Painter:
    """十字通道：以腔体中心为原点，沿臂方向（x 或 z 取大者）从 #fff0dc 渐变到 #a86858。"""

    def paint(p: Vec, axis: int) -> Color:
        reach = max(abs(p[0]), abs(p[2]))
        t = min(1.0, max(0.0, (reach - 1.0) / (CROSS_REACH - 1.0)))   # 中心 2×2 保持最亮
        color = mix(CROSS_CENTER, CROSS_BASE, t ** 0.8)
        color = mix(color, default(p, axis), 0.1)
        return color if axis == 1 else shade(color, -0.1)      # 十字的侧面略压暗，顶面保持原亮度

    return paint


def _membrane_base_painter(default: Painter) -> Painter:
    """中心腔顶膜：压成 #a86858 底色，只留一点默认起伏，十字通道贴在它上面才亮得出来。"""

    def paint(p: Vec, axis: int) -> Color:
        return mix(CROSS_BASE, default(p, axis), MEMBRANE_KEEP)

    return paint


def _roll_rib_painter(cube: dict, default: Painter) -> Painter:
    """肌肉卷肋环：#8a2a2a / #5a1a1a 交替；沿臂方向的端面外圈描 1 texel 高光，侧面整条略提亮。"""
    lo, hi = cube["from"], cube["to"]
    size = [hi[i] - lo[i] for i in range(3)]
    arm_axis = size.index(min(size))
    base = RIB_LIT_COLOR if cube["name"].endswith(("_0", "_2")) else RIB_DARK_COLOR
    plane = [i for i in range(3) if i != arm_axis]

    def paint(p: Vec, axis: int) -> Color:
        color = mix(default(p, axis), base, 0.85)
        if axis != arm_axis:
            return mix(color, RIB_HIGHLIGHT, 0.35)
        edge = min(min(p[i] - lo[i], hi[i] - p[i]) for i in plane)
        if edge < RIB_RIM_WIDTH:
            return mix(color, RIB_HIGHLIGHT, 0.7)
        return color

    return paint


def flow_block_override(cube: dict, default: Painter) -> Optional[Painter]:
    group, name = cube.get("group"), cube["name"]
    if group == "glow_cross":
        return _cross_gradient_painter(default)
    if name == "chamber_membrane":
        return _membrane_base_painter(default)
    if group and group.startswith("arm_") and "_rib_" in name:
        return _roll_rib_painter(cube, default)
    return None


# ---- o04 腿肌：两条 Y 形肌腱带亮芯 + 肌束沿长轴（竖直）的纤维线 --------------------------------

TENDON_BODY = hex_color("#e8bca8")
TENDON_CORE = hex_color("#fff0dc")
TENDON_CORE_EDGE = hex_color("#f6dcc4")
TENDON_EDGE_SHADE = -0.12               # 腱带两侧略压暗，亮芯更突出
LEG_FIBER_SPACING = 1.0                 # 肌束纤维线的间距（px），沿 y 方向竖直走


def _tendon_body_painter(default: Painter) -> Painter:
    def paint(p: Vec, axis: int) -> Color:
        return mix(TENDON_BODY, default(p, axis), 0.15)

    return paint


def _tendon_core_painter(cube: dict) -> Painter:
    """腱带正面那条亮芯：中线 #fff0dc，往两侧收到 #f6dcc4。"""

    def paint(p: Vec, axis: int) -> Color:
        return mix(TENDON_CORE, TENDON_CORE_EDGE, _face_radius(cube, p, axis))

    return paint


def _vertical_fiber_painter(default: Painter) -> Painter:
    """肌束在这个模型里自下而上排成阶梯，长轴是 y：侧面画竖线，顶 / 底面不画。"""

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis == 1:
            return color
        across = p[0] if axis == 2 else p[2]
        slot = math.floor(across / LEG_FIBER_SPACING)
        if texel_noise((slot, axis, round(p[1] / 4)), 131) > FIBER_SKIP_CHANCE:
            if (across - slot * LEG_FIBER_SPACING) < FIBER_LINE_WIDTH:
                return mix(color, LINE_COLOR, 0.8)
        return color

    return paint


def leg_override(cube: dict, default: Painter) -> Optional[Painter]:
    group, material = cube.get("group"), cube.get("material")
    if group == "tendons" and material == "tendon_trans":
        return _tendon_body_painter(default)
    if group == "tendons" and material == "qi_glow":
        return _tendon_core_painter(cube)
    if group == "muscle_bundles" and material in ("flesh_main", "flesh_lit"):
        return _vertical_fiber_painter(default)
    return None
