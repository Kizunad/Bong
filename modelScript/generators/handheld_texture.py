#!/usr/bin/env python3
"""手持物 / 器物共用的像素贴图画笔库：按材质名取画笔，所有件共用。

每个材质一套画笔，贴图沿用经脉工厂的统一入口（`meridian_factory/pixel_texture.write_textured_bbmodel`：
每面独立 UV，2 texel / px，每个 texel 按它在模型里的 3D 位置上色，所以拼接的立方体之间纹理连续）。

用法（pi-a / pi-b 也这样调）::

    from handheld_texture import handheld_override, flat_override, painter_for
    from pixel_texture import write_textured_bbmodel      # 需要 meridian_factory 在 sys.path 里

    # 1) 立方体的 "material" 字段写下面材质表里的名字
    cubes = [{"name": "blade", "from": [...], "to": [...], "material": "rust_iron", "group": "blade"}, ...]
    # 2) 带贴图的模型
    write_textured_bbmodel(cubes, out_path, name, override=handheld_override)
    # 3) 平色对照版（check_tex.png 的中间一列）：每个材质一个纯色
    write_textured_bbmodel(cubes, flat_path, name, override=flat_override)
    # 单独取一个画笔（Painter = (点 xyz, 面法线轴) -> RGB）：
    painter = painter_for("copper_edge", cube)

材质表（名字 → 画法）：
  iron           铁：深灰蓝，顺长轴的拉丝刮痕，零星亮划痕
  rust_iron      锈铁：同上，再大片锈斑（橙褐）
  copper_green   铜绿：绿色斑驳，局部露出铜色
  copper_edge    铜色刃口：亮铜色，贴近边缘处磨亮
  stone / stone_dark / stone_lit / base_stone   石：深灰褐错缝砖块、暗色砖缝、细裂纹、少量苔绿与褐渍
  bone_main / bone_dark / bone_lit / bone        骨：象牙色，顺长轴细纹，蚀孔
  wood / wood_dark   木：顺长轴木纹，端面年轮，偶有木节
  rope           麻绳：斜向拧股
  cloth / cloth_dark / cloth_red   布幡：经纬编织，边缘磨损
  paper / paper_ochre / paper_frost / paper_scorch   符纸：纤维、斑渍、边缘发焦
  ink_ochre / ink_blue / ink_cinnabar / cinnabar / cinnabar_dark   颜料：干笔飞白
  void           深坑 / 暗孔：近黑

没有列出的材质返回 None，调用方自己处理（经脉工厂的肉、膜等走 pixel_texture 的画笔）。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path
from typing import Callable, Dict, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent / "meridian_factory"))

from pixel_texture import Color, Painter, Vec, hex_color, mix, shade, texel_noise, value_noise  # noqa: E402

RENDER_COMPENSATION = -0.08     # 渲染器的 mc 光照会把贴图提亮一档，整体先压暗一点，实际看到的接近指定色


def _long_axis(cube: dict) -> int:
    size = [cube["to"][i] - cube["from"][i] for i in range(3)]
    return size.index(max(size))


def _grain_coords(cube: dict, p: Vec, axis: int) -> Tuple[float, float]:
    """(沿长轴的坐标, 垂直于长轴的横向坐标)。端面（法线 = 长轴）上返回 (0, 到中心的径向距离)。"""
    long_axis = _long_axis(cube)
    if axis == long_axis:
        others = [i for i in range(3) if i != long_axis]
        radial = math.hypot(*(p[i] - (cube["from"][i] + cube["to"][i]) / 2 for i in others))
        return 0.0, radial
    across_axis = 3 - long_axis - axis
    return p[long_axis], p[across_axis]


def _edge_distance(cube: dict, p: Vec, axis: int) -> float:
    """点在所在面上，到该面最近的一条边的距离（px）。"""
    others = [i for i in range(3) if i != axis]
    return min(min(p[i] - cube["from"][i], cube["to"][i] - p[i]) for i in others)


def _grain_noise(cube: dict, p: Vec, axis: int, across_scale: float, along_scale: float, seed: int) -> float:
    along, across = _grain_coords(cube, p, axis)
    return value_noise((across * across_scale, along * along_scale, 0.0), 1.0, seed)


# ---- 各材质的画笔工厂：factory(cube) -> Painter ---------------------------------------

BRICK_W, BRICK_H = 4.0, 2.0     # 砖块大小（px），上下行错开半块
SEAM_WIDTH = 0.5                # 砖缝宽（1 texel）


def _face_plane(p: Vec, axis: int) -> Tuple[float, float]:
    """点所在面内的两个坐标 (水平, 竖直)；顶底面用 (x, z)。"""
    if axis == 1:
        return p[0], p[2]
    if axis == 0:
        return p[2], p[1]
    return p[0], p[1]


def _stone(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """石：深灰褐的错缝砖块（每块明暗不同）+ 暗色砖缝 + 细裂纹 + 少量苔绿和褐渍。"""
    a, b = hex_color(base_a), hex_color(base_b)
    seam, moss, stain = shade(a, -0.5), hex_color("#566a44"), hex_color("#8a5a40")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            u, v = _face_plane(p, axis)
            row = math.floor(v / BRICK_H)
            shifted = u + (BRICK_W / 2 if row % 2 else 0.0)
            col = math.floor(shifted / BRICK_W)
            brick_tone = texel_noise((col, row, axis), seed + 5) - 0.5          # 每块砖自己的明暗
            t = 0.55 * value_noise(p, 1.6, seed) + 0.45 * texel_noise(p, seed + 1)
            color = shade(mix(a, b, t * 1.6 - 0.3), brick_tone * 0.3 + (value_noise(p, 0.9, seed + 4) - 0.5) * 0.3)
            if (v - row * BRICK_H) < SEAM_WIDTH or (shifted - col * BRICK_W) < SEAM_WIDTH:
                color = mix(color, seam, 0.8)                                    # 砖缝
            elif abs(value_noise(p, 3.6, seed + 2) - 0.5) < 0.02:
                color = mix(color, seam, 0.5)                                    # 细裂纹
            if value_noise(p, 3.0, seed + 6) > 0.8 and texel_noise(p, seed + 7) > 0.35:
                color = mix(color, moss, 0.55)                                   # 苔绿
            elif value_noise(p, 4.0, seed + 3) > 0.78:
                color = mix(color, stain, 0.3)
            return color

        return paint

    return factory


def _wood(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    a, b = hex_color(base_a), hex_color(base_b)

    def factory(cube: dict) -> Painter:
        long_axis = _long_axis(cube)

        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            if axis == long_axis:       # 端面：年轮
                ring = 0.5 + 0.5 * math.sin(across * 3.2 + 2.0 * value_noise(p, 1.5, seed))
                color = shade(mix(a, b, ring), -0.12)
            else:                       # 侧面：顺长轴的木纹
                grain = value_noise((across * 3.2, along * 0.28, 0.0), 1.0, seed)
                color = mix(a, b, (0.65 * grain + 0.35 * texel_noise(p, seed + 1)) * 1.5 - 0.25)
                if (across * 1.1 + 2.0 * value_noise(p, 2.0, seed + 5)) % 1.0 < 0.2:
                    color = shade(color, -0.2)      # 木纹深线
            if value_noise(p, 2.2, seed + 2) > 0.87:
                color = shade(color, -0.28)         # 木节
            return color

        return paint

    return factory


def _bone(base: str, seed: int) -> Callable[[dict], Painter]:
    light, dark = hex_color(base), shade(hex_color(base), -0.22)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 4.5, 0.45, seed)
            color = mix(dark, light, 0.45 + 0.55 * streak)
            if texel_noise(p, seed + 1) > 0.93:
                color = mix(color, dark, 0.6)       # 蚀孔
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.08)           # 边缘磨亮
            return color

        return paint

    return factory


def _rope(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    dark, light = hex_color(base_a), hex_color(base_b)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            twist = (along * 1.1 + across * 1.1) % 1.0
            band = abs(twist - 0.5) * 2
            color = mix(dark, light, band)
            return shade(color, (texel_noise(p, seed) - 0.5) * 0.16)

        return paint

    return factory


def _paper(base: str, seed: int) -> Callable[[dict], Painter]:
    color_base = hex_color(base)
    burnt = shade(color_base, -0.45)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(color_base, (texel_noise(p, seed) - 0.5) * 0.14 + (value_noise(p, 2.4, seed + 1) - 0.5) * 0.22)
            edge = _edge_distance(cube, p, axis)
            if axis == 1 and edge < 0.8:
                color = mix(color, burnt, 0.4 * (1 - edge / 0.8))
            return color

        return paint

    return factory


def _ink(base: str, seed: int) -> Callable[[dict], Painter]:
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 1.4, 0.2, seed)
            color = shade(color_base, (streak - 0.5) * 0.3)
            if texel_noise(p, seed + 1) > 0.9:
                color = shade(color, 0.22)          # 干笔飞白
            return color

        return paint

    return factory


def _iron(base: str, rust_amount: float, seed: int) -> Callable[[dict], Painter]:
    color_base = hex_color(base)
    rust_dark, rust_lit = hex_color("#8a4a2a"), hex_color("#b0623a")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            brush = _grain_noise(cube, p, axis, 5.0, 0.2, seed)
            color = shade(color_base, (brush - 0.5) * 0.3)
            if texel_noise(p, seed + 1) > 0.95:
                color = shade(color, 0.25)          # 亮划痕
            if rust_amount > 0:
                patch = value_noise(p, 2.2, seed + 2)
                if patch > 1.0 - rust_amount:
                    color = mix(color, mix(rust_dark, rust_lit, texel_noise(p, seed + 3)), 0.8)
            return color

        return paint

    return factory


def _copper_green(seed: int) -> Callable[[dict], Painter]:
    green_a, green_b, copper = hex_color("#3f7f68"), hex_color("#5aa088"), hex_color("#9a5a30")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = mix(green_a, green_b, 0.6 * value_noise(p, 1.7, seed) + 0.4 * texel_noise(p, seed + 1))
            if value_noise(p, 1.9, seed + 2) > 0.78:
                color = mix(color, copper, 0.7)     # 铜绿剥落处露出铜色
            return color

        return paint

    return factory


def _copper_edge(seed: int) -> Callable[[dict], Painter]:
    body, bright = hex_color("#c98a4a"), hex_color("#e6b070")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(body, (texel_noise(p, seed) - 0.5) * 0.14)
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, bright, 0.8)     # 刃口磨亮
            return color

        return paint

    return factory


def _cloth(base: str, seed: int) -> Callable[[dict], Painter]:
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            weave = (math.floor(along * 2) + math.floor(across * 2)) & 1
            color = shade(color_base, 0.07 if weave else -0.07)
            color = shade(color, (value_noise(p, 2.6, seed) - 0.5) * 0.2)
            if _edge_distance(cube, p, axis) < 0.4 and texel_noise(p, seed + 1) > 0.5:
                color = shade(color, -0.2)          # 毛边
            return color

        return paint

    return factory


def _void(cube: dict) -> Painter:
    return lambda p, axis: shade(hex_color("#181418"), (texel_noise(p, 9) - 0.5) * 0.15)


PAINTERS: Dict[str, Callable[[dict], Painter]] = {
    "iron": _iron("#4e525a", 0.0, 401),
    "rust_iron": _iron("#4e525a", 0.42, 411),
    "copper_green": _copper_green(421),
    "copper_edge": _copper_edge(431),
    "stone": _stone("#6a665c", "#837e72", 441),
    "stone_dark": _stone("#35322d", "#47433d", 442),
    "stone_lit": _stone("#837e72", "#9d978a", 443),
    "base_stone": _stone("#504e48", "#625f58", 444),
    "bone": _bone("#d8ccb0", 451),
    "bone_main": _bone("#d8ccb0", 451),
    "bone_dark": _bone("#b8a888", 452),
    "bone_lit": _bone("#ece2c8", 453),
    "wood": _wood("#6b4a2f", "#7d5a3a", 461),
    "wood_dark": _wood("#4a3220", "#5c4028", 462),
    "rope": _rope("#7a5030", "#a07848", 471),
    "cloth": _cloth("#7a5a46", 481),
    "cloth_dark": _cloth("#4a3a30", 482),
    "cloth_red": _cloth("#7a2a24", 483),
    "paper": _paper("#c8b48a", 491),
    "paper_ochre": _paper("#b09664", 492),
    "paper_frost": _paper("#808e98", 493),
    "paper_scorch": _paper("#3a3230", 494),
    "ink_ochre": _ink("#a8741c", 501),
    "ink_blue": _ink("#32546f", 502),
    "ink_cinnabar": _ink("#b02a20", 503),
    "cinnabar": _ink("#a02820", 504),
    "cinnabar_dark": _ink("#5c1814", 505),
    "void": _void,
}

# 平色对照版：每个材质一个纯色（取该材质画笔的中间色）
FLAT_COLORS: Dict[str, str] = {
    "iron": "#4e525a", "rust_iron": "#6a5048", "copper_green": "#4a8a70", "copper_edge": "#c98a4a",
    "stone": "#756f64", "stone_dark": "#3e3a35", "stone_lit": "#8d877b", "base_stone": "#585650",
    "bone": "#d8ccb0", "bone_main": "#d8ccb0", "bone_dark": "#b8a888", "bone_lit": "#ece2c8",
    "wood": "#745230", "wood_dark": "#523824", "rope": "#8e6038",
    "cloth": "#7a5a46", "cloth_dark": "#4a3a30", "cloth_red": "#7a2a24",
    "paper": "#c8b48a", "paper_ochre": "#b09664", "paper_frost": "#808e98", "paper_scorch": "#3a3230",
    "ink_ochre": "#a8741c", "ink_blue": "#32546f", "ink_cinnabar": "#b02a20", "cinnabar": "#a02820",
    "cinnabar_dark": "#5c1814", "void": "#181418",
}


def painter_for(material: str, cube: dict) -> Optional[Painter]:
    """材质名 → 画笔（带整体压暗补偿）。不认识的材质返回 None。"""
    factory = PAINTERS.get(material)
    if factory is None:
        return None
    painter = factory(cube)
    return lambda p, axis: shade(painter(p, axis), RENDER_COMPENSATION)


def handheld_override(cube: dict, default: Painter) -> Optional[Painter]:
    """write_textured_bbmodel 的 override：认识的材质走本库画笔，不认识的走默认。"""
    return painter_for(cube.get("material", ""), cube)


def flat_override(cube: dict, default: Painter) -> Optional[Painter]:
    """平色版：每个材质一个纯色，不认识的材质走默认画笔。"""
    code = FLAT_COLORS.get(cube.get("material", ""))
    if code is None:
        return None
    color = hex_color(code)
    return lambda p, axis: color
