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
    FIBER_NET,
    FLESH_LIT,
    hex_color,
    TUBE_CENTER,
    TUBE_HIGHLIGHT,
    Color,
    Painter,
    Vec,
    mix,
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

# 右肺（x<0）的树枝，左肺取镜像。每段 (起点, 终点)，坐标为 (x, y)。
# 骨质主支（几何）在 y 18..31，这里补它们往肺叶深处延伸的细枝。
LUNG_BRANCHES = (
    ((-4, 24), (-9, 19)), ((-9, 19), (-13, 14)), ((-9, 19), (-8, 12)),
    ((-6, 26), (-12, 28)), ((-12, 28), (-15, 31)), ((-12, 28), (-14, 24)),
    ((-4, 24), (-5, 15)), ((-5, 15), (-3, 11)), ((-5, 15), (-8, 13)),
    ((-13, 14), (-15, 11)), ((-13, 14), (-16, 15)),
)
LUNG_LINE_HALF_WIDTH = 0.35
LUNG_FRONT_Z = 10.0     # 只画在肺叶前后面（法线沿 z）上，z 绝对值大于这个值的面


def _lung_branch_painter(default: Painter) -> Painter:
    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis != 2 or abs(p[2]) < LUNG_FRONT_Z:
            return color
        x = -abs(p[0])      # 左右肺镜像：折到右肺
        for a, b in LUNG_BRANCHES:
            if _segment_distance(x, p[1], a, b) < LUNG_LINE_HALF_WIDTH:
                return mix(color, LINE_COLOR, LINE_MIX)
        return color

    return paint


def lung_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") in ("right_lobe", "left_lobe"):
        return _lung_branch_painter(default)
    return None


# ---- o03 臂肌：肌纤维沿肌束长轴走 ------------------------------------------------

FIBER_SPACING = 1.25        # 相邻纤维线的间距（px）
FIBER_LINE_WIDTH = 0.3
FIBER_SKIP_CHANCE = 0.3     # 有的纤维线断掉，免得像条码


def _muscle_fiber_painter(cube: dict, default: Painter) -> Painter:
    lo, hi = cube["from"], cube["to"]
    size = [hi[i] - lo[i] for i in range(3)]
    long_axis = size.index(max(size))

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis == long_axis:
            return color
        across_axis = 3 - long_axis - axis
        slot = math.floor((p[across_axis] - lo[across_axis]) / FIBER_SPACING)
        offset = (p[across_axis] - lo[across_axis]) - slot * FIBER_SPACING
        if offset < FIBER_LINE_WIDTH and texel_noise((slot, axis, round(lo[long_axis])), 111) > FIBER_SKIP_CHANCE:
            return mix(color, LINE_COLOR, 0.85)
        return color

    return paint


def arm_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("group") == "muscle_bundles" and cube.get("material") in ("flesh_main", "flesh_lit"):
        return _muscle_fiber_painter(cube, default)
    return None


# ---- b09 地砖：血管线 ---------------------------------------------------------

VESSEL_SCALES = ((5.0, 121), (8.0, 122))   # (噪声格大小 px, 种子)：两层等值线叠出分叉的血管
VESSEL_HALF_WIDTH = 0.012


def _vessel_painter(default: Painter) -> Painter:
    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        for scale, seed in VESSEL_SCALES:
            if abs(value_noise(p, scale, seed) - 0.5) < VESSEL_HALF_WIDTH:
                return mix(color, LINE_COLOR, LINE_MIX)
        return color

    return paint


def floor_override(cube: dict, default: Painter) -> Optional[Painter]:
    group = cube.get("group")
    if group == "base_block":
        return _vessel_painter(default)
    if group == "veins":
        return lambda p, axis: mix(default(p, axis), LINE_COLOR, 0.6)
    return None


# ---- o02 胃：胃液高光 ---------------------------------------------------------

LIQUID_HIGHLIGHT = hex_color("#f6dcc4")
HIGHLIGHT_POSITION = 0.3    # 高光线在胃液面短边方向上的位置（占比）
HIGHLIGHT_WIDTH = 0.5


def _liquid_highlight_painter(cube: dict, default: Painter) -> Painter:
    lo, hi = cube["from"], cube["to"]
    size = [hi[i] - lo[i] for i in range(3)]
    thin_axis = size.index(min(size))
    plane = [i for i in range(3) if i != thin_axis]
    short_axis = plane[0] if size[plane[0]] < size[plane[1]] else plane[1]

    def paint(p: Vec, axis: int) -> Color:
        color = default(p, axis)
        if axis != thin_axis:
            return color
        position = (p[short_axis] - lo[short_axis]) / max(size[short_axis], 1e-6)
        if abs(position - HIGHLIGHT_POSITION) * size[short_axis] < HIGHLIGHT_WIDTH / 2:
            return mix(color, LIQUID_HIGHLIGHT, 0.9)
        return color

    return paint


def stomach_override(cube: dict, default: Painter) -> Optional[Painter]:
    if cube.get("material") == "amber_fluid":
        return _liquid_highlight_painter(cube, default)
    return None
