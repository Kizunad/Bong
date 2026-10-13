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
  rust_heavy     重锈铁：整片锈红，只在零星处露出铁色（锈铁矛头）
  steel / steel_hi   银白灵钢：浅色拉丝，steel_hi 更亮（灵锋剑身与亮脊）
  iron_edge      暗铁刃（工具刃口）：暗锈铁身，只有贴边一线磨亮
  stone_edge     燧石刃：比粗石亮一档，贝壳状断口弧纹，锋口一线最亮
  spirit_wood    灵木：浅米黄带淡青，纹线偏青灰
  spirit_green   灵纹绿：深绿底 + 亮绿纹线 + 极亮线芯，靠对比显亮（不做发光）
  glow_edge      刃口青光：青白色，靠对比出光感（不做发光层）
  silk           暗色蛛丝绳：深褐拧股
  stone_chipped  粗打石：无砖缝，崩口片状亮暗面与凹坑（石矛头）
  copper_green   铜绿：绿色斑驳，局部露出铜色
  copper_edge    铜色刃口：亮铜色，贴近边缘处磨亮
  stone / stone_dark / stone_lit / base_stone   石：深灰褐错缝砖块、暗色砖缝、细裂纹、少量苔绿与褐渍
  bone_main / bone_dark / bone_lit / bone        骨：象牙色，顺长轴细纹，蚀孔
  wood / wood_dark   木：顺长轴木纹，端面年轮，偶有木节
  rope           麻绳：斜向拧股
  cloth / cloth_dark / cloth_red / cloth_tan   布幡：经纬编织，边缘毛边磨损
  leather        皮（夹垫、缠皮）：深褐，细密皱纹，边缘磨浅，零星裂口
  paper / paper_ochre / paper_frost / paper_scorch   符纸：纤维、斑渍、边缘发焦
  ink_ochre / ink_blue / ink_cinnabar / cinnabar / cinnabar_dark   颜料：干笔飞白
  void           深坑 / 暗孔：近黑
  fur_dark / fur_mangy / fur_rust   枯毛（生物用）：顺身长的毛流、深色毛根与浅色毛尖，一块块脱毛露出灰褐的皮
  scab_flesh     结痂暗红肉 / 牙龈：痂块、湿亮点、裂缝
  eye_amber      琥珀眼：深褐眼缘 + 亮琥珀 + 近白高光（靠对比，不做发光）
  chitin_white / chitin_ochre / chitin_grey / chitin_dark   骨质甲壳（蜈蚣 / 虫类）：沿长轴的棱纹、细裂纹、骨粉点、蚀孔
  marrow_amber / claw_rust / bone_dust   髓渍 / 锈褐爪尖 / 白色骨粉

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
ROPE_PERIOD = 1.5               # 绳股一个明暗周期的长度（px）


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


MIN_EDGE_SPAN = 1.4             # 面在某个方向上不到这么宽，就不在这个方向上算「边缘」（细条 / 薄片整面都贴边，会被边缘效果吃满）


def _edge_distance(cube: dict, p: Vec, axis: int) -> float:
    """点在所在面上，到该面最近的一条边的距离（px）。只算宽度 ≥ MIN_EDGE_SPAN 的方向，窄方向忽略；都太窄返回很大的数。"""
    best = 1e9
    for i in range(3):
        if i == axis:
            continue
        if cube["to"][i] - cube["from"][i] >= MIN_EDGE_SPAN:
            best = min(best, p[i] - cube["from"][i], cube["to"][i] - p[i])
    return best


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


def _stone_chipped(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """粗打石（矛头、石刃用）：没有砖缝，崩口的片状亮暗面、棱线、碎屑小亮点、凹坑，贴近边缘处崩出一粒粒暗点。"""
    a, b = hex_color(base_a), hex_color(base_b)
    pit, stain, chip = shade(a, -0.5), hex_color("#8a6a48"), shade(a, -0.62)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            flake = value_noise(p, 1.3, seed)
            color = mix(a, b, flake * 1.5 - 0.25)
            if abs(value_noise(p, 1.9, seed + 1) - 0.5) < 0.06:
                color = shade(color, -0.25)                  # 崩口的棱线
            if texel_noise(p, seed + 2) > 0.9:
                color = mix(color, pit, 0.75)                # 凹坑
            elif texel_noise(p, seed + 4) > 0.94:
                color = shade(color, 0.3)                    # 碎屑反光的小亮点
            if value_noise(p, 3.2, seed + 3) > 0.8:
                color = mix(color, stain, 0.3)
            if _edge_distance(cube, p, axis) < 0.5 and texel_noise(p, seed + 5) > 0.45:
                color = mix(color, chip, 0.7)                # 崩口暗点
            return color

        return paint

    return factory


def _wood(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """木：顺长轴的木纹——一条条深色纹线（疏密不匀）夹在浅色底上；端面是年轮；木节是深色核加一圈浅边。"""
    a, b = hex_color(base_a), hex_color(base_b)
    line_dark, knot_core, knot_ring = shade(a, -0.35), shade(a, -0.55), shade(b, 0.12)

    def factory(cube: dict) -> Painter:
        long_axis = _long_axis(cube)

        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            if axis == long_axis:       # 端面：年轮
                ring = 0.5 + 0.5 * math.sin(across * 3.2 + 2.0 * value_noise(p, 1.5, seed))
                color = shade(mix(a, b, ring), -0.12)
            else:                       # 侧面：顺长轴的木纹
                base = value_noise((across * 2.2, along * 0.2, 0.0), 1.0, seed)
                color = mix(a, b, (0.7 * base + 0.3 * texel_noise(p, seed + 1)) * 1.4 - 0.2)
                wobble = 1.6 * value_noise((across * 0.8, along * 0.18, 0.0), 1.0, seed + 5)
                if (across * 1.05 + wobble) % 1.0 < 0.32:     # 深色木纹线
                    color = mix(color, line_dark, 0.8)
            knot = value_noise(p, 1.9, seed + 2)
            if knot > 0.86:
                color = knot_core                              # 木节核
            elif knot > 0.8:
                color = mix(color, knot_ring, 0.55)            # 木节边
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
    """绳：斜向拧股，一个周期 1.5px 分「暗缝 / 中 / 亮脊」三档，明暗交替读得出股线；股脊上点一点高光。"""
    dark, light = hex_color(base_a), hex_color(base_b)
    mid = mix(dark, light, 0.5)
    crest = shade(light, 0.18)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            phase = ((along + across) / ROPE_PERIOD) % 1.0
            if phase < 0.3:
                color = dark                                  # 股与股之间的暗缝
            elif phase < 0.65:
                color = mid
            else:
                color = crest if texel_noise(p, seed + 1) > 0.7 else light
            return shade(color, (texel_noise(p, seed) - 0.5) * 0.12)

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


RUST_DEEP, RUST_MID, RUST_LIT = hex_color("#5a2818"), hex_color("#9a4e26"), hex_color("#c8803a")
PIT_DARK = hex_color("#241410")
HONED = hex_color("#a8acb0")            # 刃口磨亮的一线
EDGE_HONE_WIDTH = 0.5                   # 1 texel


def _iron(base: str, rust_amount: float, seed: int, hone: bool = False) -> Callable[[dict], Painter]:
    """铁 / 锈铁（hone=False，整体不磨亮）；steel / iron_edge 带 hone=True：刃口一线磨亮。拉丝底色 + 零星亮划痕；锈分三层——大片锈斑（深 / 中 / 亮三档橙褐）、小锈点、点蚀坑
    （深色小坑带一圈亮边）；贴近边缘的一个 texel 磨亮成一线刃口。rust_amount = 锈斑占面积的比例。"""
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            brush = _grain_noise(cube, p, axis, 5.0, 0.2, seed)
            color = shade(color_base, (brush - 0.5) * 0.3)
            if texel_noise(p, seed + 1) > 0.95:
                color = shade(color, 0.25)                  # 亮划痕
            if rust_amount > 0:
                patch = value_noise(p, 2.4, seed + 2)
                fine = value_noise(p, 0.9, seed + 4)
                if patch > 1.0 - rust_amount:
                    depth = value_noise(p, 1.3, seed + 6)   # 锈斑里深浅不一
                    tone = RUST_DEEP if depth < 0.35 else (RUST_MID if depth < 0.7 else RUST_LIT)
                    color = mix(color, mix(tone, shade(tone, 0.1), texel_noise(p, seed + 3)), 0.9)
                elif fine > 0.78 and texel_noise(p, seed + 5) > 0.3:
                    color = mix(color, RUST_MID, 0.7)       # 小锈点
                # 点蚀坑：深色小坑，坑边一圈略亮
                pit = value_noise(p, 0.75, seed + 7)
                if pit > 0.8 and texel_noise(p, seed + 8) > 0.45:
                    color = mix(color, PIT_DARK, 0.85)
                elif pit > 0.72:
                    color = shade(color, 0.12)
            if hone and _edge_distance(cube, p, axis) < EDGE_HONE_WIDTH:
                color = mix(color, HONED, 0.75 if rust_amount < 0.6 else 0.55)    # 刃口磨亮一线
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


def _copper_edge(seed: int, body_code: str = "#c98a4a", bright_code: str = "#e6b070") -> Callable[[dict], Painter]:
    body, bright = hex_color(body_code), hex_color(bright_code)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(body, (texel_noise(p, seed) - 0.5) * 0.14)
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, bright, 0.8)     # 刃口磨亮
            return color

        return paint

    return factory


def _leather(base: str, seed: int) -> Callable[[dict], Painter]:
    """皮（夹垫、旗杆缠皮）：深褐，细密皱纹（两个方向的细线网），边缘磨浅，零星裂口。"""
    color_base = hex_color(base)
    crease, worn = shade(color_base, -0.35), shade(color_base, 0.2)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(color_base, (value_noise(p, 1.8, seed) - 0.5) * 0.3)
            wrinkle = abs(value_noise(p, 1.1, seed + 1) - 0.5)
            if wrinkle < 0.05:
                color = mix(color, crease, 0.7)              # 皱纹
            if texel_noise(p, seed + 2) > 0.94:
                color = mix(color, crease, 0.6)              # 裂口
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, worn, 0.45)                # 边缘磨浅
            return color

        return paint

    return factory


def _cloth(base: str, seed: int) -> Callable[[dict], Painter]:
    """布：不规则经纬（经线、纬线各自疏密不匀，不成棋盘）+ 少量褶皱暗线 + 边缘毛边。"""
    color_base = hex_color(base)
    fold = shade(color_base, -0.3)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            warp = value_noise((across * 1.6, along * 0.15, 0.0), 1.0, seed + 3)      # 经线：顺 along 的细条，疏密不匀
            weft = value_noise((across * 0.15, along * 1.6, 0.0), 1.0, seed + 4)      # 纬线：横向细条
            color = shade(color_base, (warp - 0.5) * 0.14 + (weft - 0.5) * 0.14)
            color = shade(color, (value_noise(p, 2.6, seed) - 0.5) * 0.16)
            if abs(value_noise(p, 3.4, seed + 5) - 0.5) < 0.03:
                color = mix(color, fold, 0.6)                # 褶皱暗线
            if _edge_distance(cube, p, axis) < 0.4 and texel_noise(p, seed + 1) > 0.5:
                color = shade(color, -0.2)                   # 毛边
            return color

        return paint

    return factory
def _stone_edge(seed: int) -> Callable[[dict], Painter]:
    """燧石刃：比粗石亮一档的灰白石，面上是贝壳状断口（同心的弧纹，亮暗相间），边缘一线最亮。"""
    a, b = hex_color("#b4b0a2"), hex_color("#d2cebf")
    ring_dark = hex_color("#7e7a6e")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            cx, cy, cz = value_noise(p, 4.0, seed) * 4, value_noise(p, 4.0, seed + 1) * 4, value_noise(p, 4.0, seed + 2) * 4
            radius = math.sqrt((p[0] - cx) ** 2 + (p[1] - cy) ** 2 + (p[2] - cz) ** 2)
            conchoidal = 0.5 + 0.5 * math.sin(radius * 2.4 + 3.0 * value_noise(p, 2.0, seed + 3))
            color = mix(a, b, conchoidal * 0.8 + 0.2 * texel_noise(p, seed + 4))
            if conchoidal < 0.12:
                color = mix(color, ring_dark, 0.5)           # 断口弧线的暗棱
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.12)                    # 锋口一线最亮
            return color

        return paint

    return factory


def _spirit_wood(seed: int) -> Callable[[dict], Painter]:
    """灵木：浅色木料带淡青——浅米黄底，纹线偏青灰，木节核偏深青。"""
    a, b = hex_color("#d6d2ae"), hex_color("#bcc8a8")
    line, knot = hex_color("#6f8a7c"), hex_color("#44645a")

    def factory(cube: dict) -> Painter:
        long_axis = _long_axis(cube)

        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            if axis == long_axis:
                ring = 0.5 + 0.5 * math.sin(across * 3.2 + 2.0 * value_noise(p, 1.5, seed))
                return mix(a, line, 0.25 + 0.3 * ring)
            base = value_noise((across * 2.2, along * 0.2, 0.0), 1.0, seed)
            color = mix(a, b, base * 1.2)
            wobble = 1.6 * value_noise((across * 0.8, along * 0.18, 0.0), 1.0, seed + 5)
            if (across * 1.05 + wobble) % 1.0 < 0.3:
                color = mix(color, line, 0.7)                 # 淡青纹线
            if value_noise(p, 1.9, seed + 2) > 0.86:
                color = knot
            return color

        return paint

    return factory


def _spirit_green(seed: int) -> Callable[[dict], Painter]:
    """灵纹绿：不做发光，靠对比——深绿底、亮绿纹线、极亮的纹线中心，明暗差拉大。"""
    deep, mid, bright, core = hex_color("#16382e"), hex_color("#2f7a58"), hex_color("#6fe0a8"), hex_color("#d8fff0")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            vein = abs(value_noise(p, 2.2, seed) - 0.5)
            if vein < 0.03:
                return core                                    # 纹线中心
            if vein < 0.09:
                return bright
            return mix(deep, mid, value_noise(p, 1.5, seed + 1) * 1.2 + 0.2 * texel_noise(p, seed + 2))

        return paint

    return factory


def _fur(base_a: str, base_b: str, seed: int, bald: float = 0.0) -> Callable[[dict], Painter]:
    """枯毛（生物用）：顺身长（z）的细毛流——z 向拉长的噪声当毛束，毛束明暗不匀，零星深色毛根和浅色毛尖；
    bald > 0 时有一块块脱毛露出灰褐的皮（占面积 bald 的比例），脱毛边缘最暗。"""
    a, b = hex_color(base_a), hex_color(base_b)
    skin, skin_dark = hex_color("#7a6454"), hex_color("#4a3a32")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            strand = value_noise((p[0] * 2.6, p[1] * 2.6, p[2] * 0.4), 1.0, seed)
            color = mix(a, b, 0.65 * strand + 0.35 * texel_noise(p, seed + 1))
            color = shade(color, (value_noise(p, 2.4, seed + 2) - 0.5) * 0.2)
            if texel_noise(p, seed + 3) > 0.9:
                color = shade(color, -0.35)                  # 深色毛根
            elif texel_noise(p, seed + 4) > 0.93:
                color = shade(color, 0.22)                   # 浅色毛尖
            if bald > 0:
                patch = value_noise(p, 2.1, seed + 9)
                if patch > 1.0 - bald:
                    color = mix(skin, skin_dark, value_noise(p, 1.2, seed + 10))     # 脱毛露皮
                elif patch > 1.0 - bald - 0.05:
                    color = mix(color, skin_dark, 0.6)                              # 脱毛边缘
            return color

        return paint

    return factory


def _scab(seed: int) -> Callable[[dict], Painter]:
    """结痂的暗红肉 / 牙龈：暗红底，一块块更暗的痂，偶有湿亮的小点，细裂缝。"""
    a, b = hex_color("#5c2826"), hex_color("#7a3630")
    crust, wet, crack = hex_color("#34181a"), hex_color("#b0584c"), hex_color("#220e10")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = mix(a, b, value_noise(p, 1.6, seed) * 1.2)
            if value_noise(p, 2.0, seed + 1) > 0.7:
                color = mix(color, crust, 0.7)               # 结痂
            if texel_noise(p, seed + 2) > 0.95:
                color = mix(color, wet, 0.6)                 # 湿亮点
            if abs(value_noise(p, 3.2, seed + 3) - 0.5) < 0.03:
                color = crack
            return color

        return paint

    return factory


def _eye_amber(cube: dict) -> Painter:
    """琥珀眼：不做发光，靠对比——深褐眼缘、亮琥珀眼体、一点近白的高光。"""
    rim, body, lit = hex_color("#5a3410"), hex_color("#e8962a"), hex_color("#ffe39a")

    def paint(p: Vec, axis: int) -> Color:
        if _edge_distance(cube, p, axis) < 0.35:
            return rim
        return lit if texel_noise(p, 77) > 0.8 else mix(body, lit, 0.25 * texel_noise(p, 78))

    return paint


def _chitin(base_a: str, base_b: str, seed: int, crack_amount: float = 1.0) -> Callable[[dict], Painter]:
    """骨质甲壳（蜈蚣 / 虫类用）：骨白到灰黄的底，沿长轴的细棱纹，一道道发暗的细裂纹，
    零星白色骨粉小点，贴边处磨亮。crack_amount 控制裂纹疏密。"""
    a, b = hex_color(base_a), hex_color(base_b)
    crack, dust = shade(a, -0.5), hex_color("#f4f0e4")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            ridge = _grain_noise(cube, p, axis, 3.2, 0.5, seed)
            color = mix(a, b, 0.5 * ridge + 0.5 * value_noise(p, 1.8, seed + 1))
            if abs(value_noise(p, 2.8, seed + 2) - 0.5) < 0.03 * crack_amount:
                color = mix(color, crack, 0.75)              # 裂纹
            if texel_noise(p, seed + 3) > 0.9:
                color = mix(color, dust, 0.6)                # 骨粉
            elif texel_noise(p, seed + 4) > 0.94:
                color = shade(color, -0.25)                  # 蚀孔
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.08)
            return color

        return paint

    return factory


def _void(cube: dict) -> Painter:
    return lambda p, axis: shade(hex_color("#181418"), (texel_noise(p, 9) - 0.5) * 0.15)


# ---- 生物材质（兽皮 / 藓 / 角 / 蹄 / 眼）：末世生物批次补进同一个库 ---------------------------


def _hide(base_a: str, base_b: str, seed: int, crease_amount: float = 0.5) -> Callable[[dict], Painter]:
    """兽皮：两档色的斑块底 + 细碎皱纹暗线 + 零星浅色硬皮斑；边缘磨浅。"""
    a, b = hex_color(base_a), hex_color(base_b)
    crease, callus = shade(a, -0.38), shade(b, 0.16)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            blotch = value_noise(p, 3.4, seed) * 0.65 + value_noise(p, 1.4, seed + 1) * 0.35
            color = mix(a, b, blotch * 1.25 - 0.1)
            color = shade(color, (texel_noise(p, seed + 2) - 0.5) * 0.14)
            if abs(value_noise(p, 1.3, seed + 3) - 0.5) < 0.07 * crease_amount:
                color = mix(color, crease, 0.65)              # 皱纹
            if value_noise(p, 2.0, seed + 4) > 0.8 and texel_noise(p, seed + 5) > 0.35:
                color = mix(color, callus, 0.55)              # 硬皮斑
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.07)
            return color

        return paint

    return factory


def _moss(base_dark: str, base_light: str, seed: int) -> Callable[[dict], Painter]:
    """噬灵藓：成簇绒团（团内亮、团缝暗），簇顶灰白孢尖，偶有枯褐斑；靠对比，不做发光。"""
    dark, light = hex_color(base_dark), hex_color(base_light)
    spore, dead = shade(light, 0.38), hex_color("#6a5e3c")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            clump = value_noise(p, 1.7, seed)
            fine = texel_noise(p, seed + 1)
            color = shade(mix(dark, light, clump * 1.5 - 0.2), (fine - 0.5) * 0.2)
            if clump > 0.66 and fine > 0.6:
                color = mix(color, spore, 0.7)
            if value_noise(p, 3.0, seed + 2) > 0.84:
                color = mix(color, dead, 0.5)
            return color

        return paint

    return factory


def _horn(base: str, seed: int) -> Callable[[dict], Painter]:
    """角质：顺长轴细棱，隔一段一道深色环脊，裂纹稀疏。"""
    light = hex_color(base)
    dark = shade(light, -0.3)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            streak = value_noise((across * 3.0, along * 0.5, 0.0), 1.0, seed)
            color = mix(dark, light, 0.35 + 0.65 * streak)
            if (along * 0.9 + 1.5 * value_noise(p, 2.0, seed + 1)) % 3.0 < 0.5:
                color = mix(color, dark, 0.55)
            if texel_noise(p, seed + 2) > 0.95:
                color = mix(color, dark, 0.7)
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.08)
            return color

        return paint

    return factory


def _hoof(seed: int) -> Callable[[dict], Painter]:
    """蹄：近黑角质，细竖纹，崩口处露出灰白。"""
    base, chip = hex_color("#2c2826"), hex_color("#6a645a")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(base, (value_noise((p[0] * 2.5, p[1] * 0.4, p[2] * 2.5), 1.0, seed) - 0.5) * 0.35)
            if texel_noise(p, seed + 1) > 0.93:
                color = mix(color, chip, 0.7)
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.12)
            return color

        return paint

    return factory


def _eye(cube: dict) -> Painter:
    """眼：近黑浑浊面，中心偏上一小块亮斑（靠对比读出眼神）。"""
    center = [(cube["from"][i] + cube["to"][i]) / 2 for i in range(3)]

    def paint(p: Vec, axis: int) -> Color:
        if math.dist(p, (center[0], center[1] + 0.2, center[2])) < 0.45:
            return hex_color("#a8a290")
        return shade(hex_color("#1c1a18"), (texel_noise(p, 601) - 0.5) * 0.2)

    return paint


def _flower(seed: int) -> Callable[[dict], Painter]:
    """藓花：灰白花瓣，顶面带暖黄花芯，瓣缘淡褐。"""
    petal, core, rim = hex_color("#e6e6da"), hex_color("#d8c070"), hex_color("#b8a890")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(petal, (texel_noise(p, seed) - 0.5) * 0.1)
            if axis == 1:
                color = mix(color, core, 0.45)
            if _edge_distance(cube, p, axis) < 0.4:
                color = mix(color, rim, 0.5)
            return color

        return paint

    return factory


PAINTERS: Dict[str, Callable[[dict], Painter]] = {
    "iron": _iron("#4e525a", 0.0, 401),
    "rust_iron": _iron("#4e525a", 0.42, 411),
    "rust_heavy": _iron("#6a3a2c", 0.78, 412),
    "steel": _iron("#aab4be", 0.0, 521, hone=True),
    "iron_edge": _iron("#5a5c60", 0.3, 524, hone=True),
    "stone_edge": _stone_edge(531),
    "spirit_wood": _spirit_wood(541),
    "spirit_green": _spirit_green(551),
    "steel_hi": _iron("#d4dce4", 0.0, 522),
    "glow_edge": _ink("#8fe0e2", 523),
    "silk": _rope("#2a201c", "#4a3a30", 511),
    "stone_chipped": _stone_chipped("#9c988c", "#bcb8aa", 445),
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
    "cloth_tan": _cloth("#8a6c3e", 484),
    "leather": _leather("#5c4230", 485),
    "paper": _paper("#c8b48a", 491),
    "paper_ochre": _paper("#b09664", 492),
    "paper_frost": _paper("#808e98", 493),
    "paper_scorch": _paper("#3a3230", 494),
    "ink_ochre": _ink("#a8741c", 501),
    "ink_blue": _ink("#32546f", 502),
    "ink_cinnabar": _ink("#b02a20", 503),
    "cinnabar": _ink("#a02820", 504),
    "cinnabar_dark": _ink("#5c1814", 505),
    # 生物用（枯皮狼 / 髓蛀蜈）
    "fur_dark": _fur("#3e3630", "#5a4e44", 601, bald=0.18),
    "fur_mangy": _fur("#5c5146", "#7e7060", 602, bald=0.3),
    "fur_rust": _fur("#6e3a2c", "#8e5238", 603, bald=0.22),
    "scab_flesh": _scab(604),
    "eye_amber": _eye_amber,
    "chitin_white": _chitin("#dcd4c0", "#bcb298", 611),
    "chitin_ochre": _chitin("#c0ac78", "#9c8a5c", 612),
    "chitin_grey": _chitin("#8e8878", "#6e6a5e", 613),
    "chitin_dark": _chitin("#2e2a24", "#463e34", 614, 0.5),
    "marrow_amber": _ink("#c8964a", 615),
    "claw_rust": _ink("#78563a", 616),
    "bone_dust": _ink("#f4f2ea", 617),
    "void": _void,
    # 游商傀儡补的材质：骨渍 / 浅木 / 暗绳 / 暗铜 / 灰青布 / 阵盘亮纹与暗底 / 眼窝余烬（亮纹与余烬只靠亮度对比）
    "bone_stain": _bone("#8a7658", 454),
    "wood_lit": _wood("#8e6a44", "#a07c52", 463),
    "rope_dark": _rope("#3a2c1c", "#6e5638", 472),
    "copper_dark": _copper_edge(432, "#7a4a2a", "#a86a3c"),
    "cloth_grey": _cloth("#6a6c6a", 485),
    "cloth_sage": _cloth("#60704f", 486),
    "rune_lit": _ink("#a8f0d8", 561),
    "slate_dark": _ink("#26262c", 562),
    "ember": _ink("#f0a048", 563),
}

# 生物材质（兽皮 / 藓 / 角 / 蹄 / 眼）
PAINTERS.update({
    "hide": _hide("#5c5248", "#7a6e5e", 701),
    "hide_dark": _hide("#403a34", "#575048", 702),
    "hide_light": _hide("#7a6e5e", "#9a8c78", 703),
    "hide_fold": _hide("#2c2824", "#3c3631", 704, crease_amount=1.0),
    "moss_dark": _moss("#2e382a", "#53684a", 711),
    "moss": _moss("#44553a", "#78905f", 712),
    "moss_light": _moss("#62784c", "#a2b284", 713),
    "moss_flower": _flower(721),
    "horn": _horn("#8a7c6a", 731),
    "horn_dark": _horn("#6a5e50", 732),
    "hoof": _hoof(741),
    "eye": _eye,
})

# 平色对照版：每个材质一个纯色（取该材质画笔的中间色）
FLAT_COLORS: Dict[str, str] = {
    "iron": "#4e525a", "rust_iron": "#6a5048", "rust_heavy": "#7a4a38", "steel": "#aab4be", "steel_hi": "#d4dce4", "iron_edge": "#5a5c60",
    "stone_edge": "#c0bcae", "spirit_wood": "#c8d0a8", "spirit_green": "#2f7a58",
    "glow_edge": "#8fe0e2", "silk": "#3a2e28", "stone_chipped": "#aaa699", "copper_green": "#4a8a70", "copper_edge": "#c98a4a",
    "stone": "#756f64", "stone_dark": "#3e3a35", "stone_lit": "#8d877b", "base_stone": "#585650",
    "bone": "#d8ccb0", "bone_main": "#d8ccb0", "bone_dark": "#b8a888", "bone_lit": "#ece2c8",
    "wood": "#745230", "wood_dark": "#523824", "rope": "#8e6038",
    "cloth": "#7a5a46", "cloth_dark": "#4a3a30", "cloth_red": "#7a2a24", "cloth_tan": "#8a6c3e", "leather": "#5c4230",
    "paper": "#c8b48a", "paper_ochre": "#b09664", "paper_frost": "#808e98", "paper_scorch": "#3a3230",
    "ink_ochre": "#a8741c", "ink_blue": "#32546f", "ink_cinnabar": "#b02a20", "cinnabar": "#a02820",
    "cinnabar_dark": "#5c1814", "void": "#181418",
    "bone_stain": "#8a7658", "wood_lit": "#947048", "rope_dark": "#4e3c26", "copper_dark": "#7a4a2a",
    "cloth_grey": "#6a6c6a", "cloth_sage": "#60704f", "rune_lit": "#a8f0d8", "slate_dark": "#26262c", "ember": "#f0a048",
    "fur_dark": "#4a4038", "fur_mangy": "#6a5e50", "fur_rust": "#7c4632", "scab_flesh": "#6a2e2c", "eye_amber": "#e8962a",
    "chitin_white": "#cac1aa", "chitin_ochre": "#ae9a68", "chitin_grey": "#7e7868", "chitin_dark": "#3a342c",
    "marrow_amber": "#c8964a", "claw_rust": "#78563a", "bone_dust": "#f4f2ea",
}
FLAT_COLORS.update({
    "hide": "#685d50", "hide_dark": "#494239", "hide_light": "#8a7d6a", "hide_fold": "#342f2a",
    "moss_dark": "#364c30", "moss": "#52703f", "moss_light": "#7a9a5e", "moss_flower": "#e6e6da",
    "horn": "#7a6e5e", "horn_dark": "#5a5044", "hoof": "#2c2826", "eye": "#1c1a18",
})


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
