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
  crow_black / crow_dark / crow_char / crow_ash   焦羽（黑 / 暗 / 焦褐带焦斑 / 灰烬）：顺羽向细枝、羽轴亮线；crow_beak / crow_claw 喙与爪
  bell_cream / bell_grey / bell_frill   水母伞肉 / 口腕：半透浅色胶质、淡纹、冷暗云；bell_vein / bell_vein_slate 深色脉纹
  bolt_blue / eye_white / spirit_core / spirit_mote   雷痕 / 眼白 / 灵核 / 光点：面心最亮、边缘渐暗（只靠对比）
  grub_cream / grub_pink   蛆皮（半透奶白 / 肉粉）：柔和明暗、淡红血管、环向皱；rot_brown 腐缝 / organ_red 脏器 / ichor 腐液
  glint_amber / glint_core   吞进去的灵光碎块：面心亮、边缘渐暗成琥珀（只靠对比）
  bark / bark_dark / bark_light / bark_grey / bark_rust / bark_knot   枯树皮（枯枝螳）：纵裂 + 横裂切成长条鳞片、翘起亮皮屑、灰白风化斑；节疤是深褐同心环
  gear_weave(_lt/_dk/_rim)   草编：篾条一上一下的格子 + 格缝；gear_linen(_dk/_lt/_grey/_brn) 粗麻布；gear_vine(_lt/_dk/_dry) 枯藤
  gear_cord(_dk) 草绳；gear_hide(_dk/_lt/_grey/_pale/_stain/_spot) 拼缝兽皮；gear_patch(_dk) 补丁皮；gear_iron(_lt/_dk) / gear_rust(_lt/_dk) 锈铁件
  gear_clay(_dk/_lt) 陶；gear_gourd(_dk/_lt) 干葫芦；gear_cork 软木塞；gear_wearer_* 灰色占位玩家（穿戴件 check_tex 用）
  gear_herb_*(green/green_dk/pale/brown/brown_dk) 干草药叶：居中粗叶脉 + 斜侧脉 + 卷枯叶缘；gear_herb_seed 干籽穗（1px 一粒带暗缝）
  gear_ore_*(grey/dk/rust/fleck) 矿石：三档量化切面 + 暗裂 + 亮晶点；gear_cinnabar(_dk) 丹砂粉涂；gear_moss(_dk/_lt/_ash) 灰烬苔簇
  gear_leather(_dk/_lt) 皮；gear_stitch 针脚；gear_fur 浅毛；gear_hoof(_dk) 蹄角；gear_wood(_lt/_dk) 旧木板；gear_rag_*/gear_red_cord 破布 / 红绳；gear_hide_pale_dk 深一档浅兽皮
  skull_bone / skull_bone_old   颅骨壳：象牙骨 / 旧骨；crab_shell / crab_shell_dark 锈橙蟹甲；lichen 壳上苔；barnacle 藤壶

没有列出的材质返回 None，调用方自己处理（经脉工厂的肉、膜等走 pixel_texture 的画笔）。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path
from typing import Callable, Dict, Optional, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent / "meridian_factory"))

from pixel_texture import Color, Painter, Vec, build_atlas, hex_color, mix, shade, texel_noise, value_noise  # noqa: E402

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


def _fur_upright(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """毛：沿竖直（y）方向的细长毛束明暗相间，一簇一簇深浅不同，零星亮毛尖和暗根。"""
    a, b = hex_color(base_a), hex_color(base_b)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            strand = value_noise((p[0] * 3.0, p[1] * 0.35, p[2] * 3.0), 1.0, seed)
            color = mix(a, b, strand * 1.3 - 0.1)
            color = shade(color, (value_noise(p, 1.6, seed + 1) - 0.5) * 0.24)
            grain = texel_noise(p, seed + 2)
            if grain > 0.93:
                color = shade(color, 0.22)              # 亮毛尖
            elif grain < 0.05:
                color = shade(color, -0.3)              # 暗毛根
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


def _skin(base: str, seed: int, vein: float = 0.03) -> Callable[[dict], Painter]:
    """裸皮 / 肉：细皱纹 + 斑驳 + 极细的暗红血管线（vein = 血管线的粗细，0 则不画）。"""
    color_base = hex_color(base)
    crease, vein_color = shade(color_base, -0.28), mix(shade(color_base, -0.3), hex_color("#8a2a2a"), 0.5)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(color_base, (value_noise(p, 1.5, seed) - 0.5) * 0.26 + (texel_noise(p, seed + 1) - 0.5) * 0.08)
            if abs(value_noise(p, 1.1, seed + 2) - 0.5) < 0.04:
                color = mix(color, crease, 0.6)          # 皱纹
            if vein and abs(value_noise(p, 2.2, seed + 3) - 0.5) < vein:
                color = mix(color, vein_color, 0.75)     # 血管
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


def _membrane(base: str, seed: int, vein: bool = False) -> Callable[[dict], Painter]:
    """皮膜（蝠翼）：暗褐革质，细皱 + 大块明暗斑 + 细密暗纹；vein=True 是血管格：暗红底，中心一线更亮。"""
    color_base = hex_color(base)
    crease = shade(color_base, -0.3)
    core = shade(color_base, 0.28)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(color_base, (value_noise(p, 2.4, seed) - 0.5) * 0.3 + (texel_noise(p, seed + 1) - 0.5) * 0.1)
            if abs(value_noise(p, 0.9, seed + 2) - 0.5) < 0.05:
                color = mix(color, crease, 0.55)
            if vein and abs(value_noise(p, 1.8, seed + 3) - 0.5) < 0.12:
                color = mix(color, core, 0.5)
            return color

        return paint

    return factory


def _scales(base_a: str, base_b: str, seed: int, cell: float = 1.5) -> Callable[[dict], Painter]:
    """鳞：错缝排布的小鳞片，每片自己的明暗，片间一线暗缝，片的上缘一线亮边。"""
    a, b = hex_color(base_a), hex_color(base_b)
    seam = shade(a, -0.5)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            u, v = _face_plane(p, axis)
            row = math.floor(v / cell)
            shifted = u + (cell / 2 if row % 2 else 0.0)
            col = math.floor(shifted / cell)
            color = mix(a, b, texel_noise((col, row, axis), seed))
            color = shade(color, (texel_noise(p, seed + 1) - 0.5) * 0.1)
            fv, fu = (v - row * cell) / cell, (shifted - col * cell) / cell
            if fv < 0.22 or fu < 0.22:
                return mix(color, seam, 0.7)             # 鳞缝
            if fv > 0.78:
                return shade(color, 0.14)                # 鳞片上缘亮边
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


# ---- 枯灯蛾专用画笔（与兽皮 / 蝠翼的 _fur / _membrane / _chitin 同名异义，所以加 _moth 前缀） -----


def _moth_fur(base: str, seed: int) -> Callable[[dict], Painter]:
    """绒毛（蛾胸腹）：沿 z（身体轴）拉长的明暗毛流，零星亮毛尖与暗隙。"""
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            flow = value_noise((p[0] * 1.7, p[1] * 1.7, p[2] * 0.55), 1.0, seed)
            color = shade(color_base, (flow - 0.5) * 0.5)
            roll = texel_noise(p, seed + 1)
            if roll > 0.88:
                color = shade(color, 0.28)                   # 亮毛尖
            elif roll < 0.07:
                color = shade(color, -0.4)                   # 毛间暗隙
            return color

        return paint

    return factory


def _moth_membrane(base: str, seed: int) -> Callable[[dict], Painter]:
    """翼膜（蛾）：浅褐灰底，沿翅展方向（x）的细纤维，零星深斑点，贴近边缘处磨暗。"""
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            fiber = value_noise((p[0] * 0.5, p[1] * 1.5, p[2] * 1.8), 1.0, seed)
            color = shade(color_base, (fiber - 0.5) * 0.28)
            roll = texel_noise(p, seed + 1)
            if roll > 0.93:
                color = shade(color, -0.22)                  # 灰斑
            elif roll < 0.04:
                color = shade(color, 0.16)
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, -0.12)                  # 边缘发暗
            return color

        return paint

    return factory


def _eyespot(base: str, seed: int) -> Callable[[dict], Painter]:
    """翅上的眼斑：深褐底，成团的浅色环点（噪声峰值处留一个浅芯）。"""
    color_base = hex_color(base)
    core = hex_color("#d8c8a0")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            blob = value_noise(p, 1.6, seed)
            color = shade(color_base, (texel_noise(p, seed + 1) - 0.5) * 0.18)
            if blob > 0.62:
                return mix(color, core, 0.85)                # 眼斑浅芯
            if blob > 0.54:
                return shade(color_base, -0.4)               # 芯外一圈深环
            return color

        return paint

    return factory


def _moth_chitin(base: str, seed: int) -> Callable[[dict], Painter]:
    """几丁质（触角干 / 腿节 / 关节）：顺长轴的细亮纹，零星刮痕。"""
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 2.2, 0.15, seed)
            color = shade(color_base, (streak - 0.5) * 0.4)
            if texel_noise(p, seed + 1) > 0.92:
                color = shade(color, 0.3)
            return color

        return paint

    return factory


# ---- 腐沼蟾专用画笔（湿滑蟾皮 / 金色凸眼 / 口腔与舌的湿亮肉色） ------------------------------------


def _toad_skin(base_a: str, base_b: str, seed: int, pits: float = 0.07) -> Callable[[dict], Painter]:
    """蟾皮：两档橄榄色的大斑块，细碎的深色皮孔，零星一点湿亮高光；边缘略亮。"""
    a, b = hex_color(base_a), hex_color(base_b)
    pit, sheen = shade(a, -0.42), shade(b, 0.32)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            blotch = value_noise(p, 3.0, seed) * 0.6 + value_noise(p, 1.4, seed + 1) * 0.4
            color = mix(a, b, blotch * 1.3 - 0.1)
            color = shade(color, (texel_noise(p, seed + 2) - 0.5) * 0.12)
            roll = texel_noise(p, seed + 3)
            if roll < pits:
                color = mix(color, pit, 0.7)                 # 皮孔
            elif roll > 0.955:
                color = mix(color, sheen, 0.6)               # 湿亮点
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.05)
# ---- 藓衣蜗专用画笔（湿滑蜗肉 / 螺壳 / 地衣壳斑 / 黏液膜） --------------------------------------------


def _slug_flesh(base_a: str, base_b: str, seed: int, sheen: float = 0.1) -> Callable[[dict], Painter]:
    """湿滑的蜗肉：横向一道道褶皱（褶沟深、褶脊浅），半透的深浅斑块，零星一点黏液高光（sheen 控制亮点的多少）。"""
    a, b = hex_color(base_a), hex_color(base_b)
    crease, gloss = shade(a, -0.3), shade(b, 0.4)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            patch = value_noise(p, 3.0, seed) * 0.6 + value_noise(p, 1.5, seed + 1) * 0.4
            color = mix(a, b, patch * 1.2 - 0.05)
            wave = 0.5 + 0.5 * math.sin(p[2] * 2.1 + 1.6 * value_noise(p, 1.8, seed + 2))     # 沿身长的横向褶
            if wave < 0.22:
                color = mix(color, crease, 0.55)                 # 褶沟
            elif wave > 0.86:
                color = mix(color, gloss, 0.45)                  # 褶脊上的水光
            if texel_noise(p, seed + 3) > 1.0 - sheen:
                color = mix(color, gloss, 0.7)                   # 黏液亮点
            return color

        return paint

    return factory


def _toad_eye(seed: int) -> Callable[[dict], Painter]:
    """蟾的虹膜：金黄底，零星深褐斑点（虹膜上的细纹），偶有一点高光。"""
    gold, fleck, glint = hex_color("#d8b44c"), hex_color("#7c5a22"), hex_color("#f6e4a0")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(gold, (value_noise(p, 1.5, seed) - 0.5) * 0.3)
            roll = texel_noise(p, seed + 1)
            if roll < 0.18:
                color = mix(color, fleck, 0.65)
            elif roll > 0.94:
                color = mix(color, glint, 0.7)
# ---- 腐根熊专用画笔（托架菌的盖与褶；熊毛 / 枯根复用 _fur / _wood，只换色） ----------------------------


def _shelf_cap(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """托架菌盖：以盖中心为圆心的同心生长环（深浅相间），盖缘一圈奶白，零星小疣点。"""
    a, b = hex_color(base_a), hex_color(base_b)
    rim = hex_color("#e4d2a8")

    def factory(cube: dict) -> Painter:
        center = [(cube["from"][i] + cube["to"][i]) / 2 for i in range(3)]

        def paint(p: Vec, axis: int) -> Color:
            r = math.hypot(p[0] - center[0], p[2] - center[2])
            ring = 0.5 + 0.5 * math.sin(r * 3.0 + 2.0 * value_noise(p, 1.4, seed))
            color = shade(mix(a, b, ring), (texel_noise(p, seed + 1) - 0.5) * 0.12)
            if texel_noise(p, seed + 2) > 0.94:
                color = shade(color, -0.22)                  # 疣点
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, rim, 0.55)                # 盖缘发白
def _snail_shell(base_a: str, base_b: str, seed: int, ridge: float = 0.0) -> Callable[[dict], Painter]:
    """螺壳：浅褐的壳面上细密的生长纹（绕壳心的同心细线），磨损的暗点；ridge > 0 时再加一层圈脊的亮边。
    壳心取藓衣蜗的 (y, z) = (13, 5)（壳面朝 ±x，螺旋轴沿 x 穿过这里）。"""
    a, b = hex_color(base_a), hex_color(base_b)
    line = shade(a, -0.4)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = mix(a, b, value_noise(p, 2.4, seed) * 1.1)
            if (math.hypot(p[1] - 13.0, p[2] - 5.0) * 2.6 + 1.5 * value_noise(p, 1.6, seed + 1)) % 1.0 < 0.22:
                color = mix(color, line, 0.6)                    # 生长纹
            if texel_noise(p, seed + 2) > 0.93:
                color = shade(color, -0.2)                       # 磨损的暗点
            if ridge > 0 and _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, ridge)
            return color

        return paint

    return factory


def _lichen_crust(base: str, seed: int) -> Callable[[dict], Painter]:
    """地衣壳斑：淡黄绿的一块块硬壳，斑块内带细小的深色裂缝，斑缘一圈暗线。"""
    color_base = hex_color(base)
    rim, crack = shade(color_base, -0.45), shade(color_base, -0.25)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            blob = value_noise(p, 2.0, seed)
            color = shade(color_base, (blob - 0.5) * 0.25 + (texel_noise(p, seed + 1) - 0.5) * 0.1)
            if abs(value_noise(p, 1.2, seed + 2) - 0.5) < 0.05:
                color = mix(color, crack, 0.6)                   # 裂缝
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, rim, 0.5)
            return color

        return paint

    return factory


def _wet_flesh(base: str, seed: int) -> Callable[[dict], Painter]:
    """口腔 / 舌：湿亮的肉红，顺长轴一道道浅深条纹，零星白亮的水光。"""
    color_base = hex_color(base)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 3.0, 0.3, seed)
            color = shade(color_base, (streak - 0.5) * 0.36)
            if texel_noise(p, seed + 1) > 0.9:
                color = shade(color, 0.34)                   # 水光
def _shelf_gill(base: str, seed: int) -> Callable[[dict], Painter]:
    """菌褶 / 菌管：奶黄底，密而细的深色褶线（沿长轴），边缘磨亮。"""
    color_base = hex_color(base)
    line = shade(color_base, -0.4)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            color = shade(color_base, (value_noise(p, 1.6, seed) - 0.5) * 0.2)
            if (across * 2.4 + value_noise((across, along, 0.0), 1.2, seed + 1)) % 1.0 < 0.3:
                color = mix(color, line, 0.7)                # 褶线
            if _edge_distance(cube, p, axis) < 0.4:
                color = shade(color, 0.1)
def _slime_film(base: str, seed: int, bright: float = 0.0) -> Callable[[dict], Painter]:
    """黏液膜：半透的灰绿底，顺爬行方向的细长水光，零星亮泡；bright > 0 时整体更亮（亮泡用）。"""
    color_base = hex_color(base)
    gloss = shade(color_base, 0.35)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = value_noise((p[0] * 2.4, p[1] * 2.4, p[2] * 0.5), 1.0, seed)
            color = shade(color_base, (streak - 0.5) * 0.3 + bright)
            if texel_noise(p, seed + 1) > 0.9:
                color = mix(color, gloss, 0.6)
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
    # 杂尾蝠补的材质：毛 / 皮膜 / 裸皮肉 / 三种尾鳞
    "bat_fur_dark": _fur_upright("#2a282c", "#403c40", 601),
    "fur": _fur_upright("#403c40", "#5c5658", 602),
    "fur_lit": _fur_upright("#5c5658", "#807a7a", 603),
    "belly_fur": _fur_upright("#9a8c7a", "#b8aa94", 604),
    "membrane": _membrane("#50402f", 611),
    "membrane_lit": _membrane("#645038", 612),
    "membrane_vein": _membrane("#64362e", 613, vein=True),
    "muscle_flesh": _skin("#7c3c3a", 621),
    "ear_skin": _skin("#ac706e", 622),
    "ear_vein": _skin("#8a5658", 623, vein=0.0),
    "nose_pink": _skin("#a05c5c", 624, vein=0.0),
    "gum": _skin("#94424a", 625),
    "flesh_seam": _skin("#98403c", 626, vein=0.05),
    "rat_skin": _skin("#a27c76", 627),
    "rat_skin_dark": _skin("#805e5c", 628),
    "hair_dark": _ink("#342a2a", 629),
    "claw_dark": _ink("#2c2828", 630),
    "lizard_scale": _scales("#626c4a", "#7a8458", 631),
    "lizard_scale_dark": _scales("#44502f", "#566238", 632),
    "lizard_scale_lit": _scales("#828860", "#989e72", 633),
    "snake_scale": _scales("#3a4042", "#4c5456", 634, 1.2),
    "snake_scale_dark": _scales("#262a2e", "#363c40", 635, 1.2),
    "snake_belly": _scales("#948c76", "#aca48a", 636, 1.2),
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
    "slate_ink": _ink("#26262c", 562),
    "ember": _ink("#f0a048", 563),
    # 诡影（负灵域漩涡）：深灰石环 / 浅灰带 / 碎石 / 枯叶，轮廓全靠明暗对比
    "slate": _stone_chipped("#585560", "#6e6a72", 601),
    "slate_dark": _stone_chipped("#46434c", "#58555e", 602),
    "slate_lit": _stone_chipped("#706c74", "#8a868c", 603),
    "slate_rim": _stone_chipped("#2c2a32", "#3a3740", 604),
    "ash_band": _stone_chipped("#b4b0a6", "#cac6ba", 605),
    "rubble": _stone_chipped("#98948c", "#b0aca2", 606),
    "rubble_dark": _stone_chipped("#5e5a58", "#767270", 607),
    "dry_leaf": _leather("#76624a", 608),
    "dry_leaf_vein": _leather("#4a3c2a", 609),
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
    "bat_fur_dark": "#34303a", "fur": "#4c484a", "fur_lit": "#6c6668", "belly_fur": "#aa9c88", "membrane": "#58482f",
    "membrane_lit": "#6c5840", "membrane_vein": "#6c382e", "muscle_flesh": "#7c3c3a", "ear_skin": "#ac706e",
    "ear_vein": "#8a5658", "nose_pink": "#a05c5c", "gum": "#94424a", "flesh_seam": "#98403c", "rat_skin": "#a27c76",
    "rat_skin_dark": "#805e5c", "hair_dark": "#342a2a", "claw_dark": "#2c2828", "lizard_scale": "#6c7650",
    "lizard_scale_dark": "#4c5834", "lizard_scale_lit": "#8c9268", "snake_scale": "#444c4e", "snake_scale_dark": "#2e3236",
    "snake_belly": "#a09880",
    "paper": "#c8b48a", "paper_ochre": "#b09664", "paper_frost": "#808e98", "paper_scorch": "#3a3230",
    "ink_ochre": "#a8741c", "ink_blue": "#32546f", "ink_cinnabar": "#b02a20", "cinnabar": "#a02820",
    "cinnabar_dark": "#5c1814", "void": "#181418",
    "bone_stain": "#8a7658", "wood_lit": "#947048", "rope_dark": "#4e3c26", "copper_dark": "#7a4a2a",
    "cloth_grey": "#6a6c6a", "cloth_sage": "#60704f", "rune_lit": "#a8f0d8", "slate_ink": "#26262c", "ember": "#f0a048",
    "fur_dark": "#4a4038", "fur_mangy": "#6a5e50", "fur_rust": "#7c4632", "scab_flesh": "#6a2e2c", "eye_amber": "#e8962a",
    "chitin_white": "#cac1aa", "chitin_ochre": "#ae9a68", "chitin_grey": "#7e7868", "chitin_dark": "#3a342c",
    "marrow_amber": "#c8964a", "claw_rust": "#78563a", "bone_dust": "#f4f2ea",
    "slate": "#605d68", "slate_dark": "#4c4954", "slate_lit": "#7a767c", "slate_rim": "#312f38",
    "ash_band": "#bdb9ae", "rubble": "#a09c94", "rubble_dark": "#665f5d",
    "dry_leaf": "#76624a", "dry_leaf_vein": "#4a3c2a",
}
FLAT_COLORS.update({
    "hide": "#685d50", "hide_dark": "#494239", "hide_light": "#8a7d6a", "hide_fold": "#342f2a",
    "moss_dark": "#364c30", "moss": "#52703f", "moss_light": "#7a9a5e", "moss_flower": "#e6e6da",
    "horn": "#7a6e5e", "horn_dark": "#5a5044", "hoof": "#2c2826", "eye": "#1c1a18",
})


# ---- 金属 / 甲壳 / 软腹（锈甲鳖虫批次补充）-----------------------------------------------


def _gold(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """碎金片：顺长轴的拉丝金色，成片发暗的氧化斑，零星亮划痕，边缘磨亮。"""
    a, b = hex_color(base_a), hex_color(base_b)
    tarnish = hex_color("#5a4426")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 3.0, 0.4, seed)
            color = mix(a, b, streak * 1.2 - 0.1)
            if value_noise(p, 2.2, seed + 1) > 0.7:
                color = mix(color, tarnish, 0.55)             # 氧化斑
            if texel_noise(p, seed + 2) > 0.95:
                color = shade(color, 0.22)                    # 亮划痕
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.12)
            return color

        return paint

    return factory


PAINTERS.update({
    "gold": _gold("#a88a42", "#d2b062", 751),
    "gold_dull": _gold("#7a6234", "#a8884a", 752),
    "rivet": _iron("#2e2622", 0.25, 753),
    "shell_dark": _horn("#3a3230", 754),
    "shell_mid": _horn("#5e5248", 755),
    "belly_pale": _hide("#b8aa8c", "#d2c6a8", 756, crease_amount=0.3),
    "belly_seg": _hide("#7e6e56", "#9a8a6c", 757, crease_amount=0.8),
})
FLAT_COLORS.update({
    "gold": "#bc9a50", "gold_dull": "#92763e", "rivet": "#2e2622", "shell_dark": "#3a3230", "shell_mid": "#5e5248",
    "belly_pale": "#c4b698", "belly_seg": "#8a7a5e",
})

# 枯灯蛾（ku_deng_e）：绒毛 / 翼膜 / 翅脉 / 眼斑 / 几丁质 / 腹尖残光（残光只靠亮黄的高对比，不做发光层）
PAINTERS.update({
    "moth_fur_dk": _moth_fur("#3e3832", 701),
    "moth_fur": _moth_fur("#5e564a", 702),
    "moth_fur_lt": _moth_fur("#857a68", 703),
    "moth_fur_pale": _moth_fur("#b8aa8c", 704),
    "moth_band": _moth_fur("#b09a74", 705),
    "moth_chit_dk": _moth_chitin("#2a221c", 711),
    "moth_chit": _moth_chitin("#4a3a2c", 712),
    "moth_chit_lt": _moth_chitin("#7a6248", 713),
    "moth_eye": _moth_chitin("#16121a", 714),
    "moth_eye_hi": _moth_chitin("#8a7c78", 715),
    "moth_wing": _moth_membrane("#c4b498", 721),
    "moth_wing_lt": _moth_membrane("#dcd2bc", 722),
    "moth_eyespot": _eyespot("#6a5238", 723),
    "moth_vein": _ink("#3e2a1c", 724),
    "moth_wing_edge": _ink("#5e4228", 725),
    "moth_glow": _ink("#ffd23c", 731),
    "moth_glow_hi": _ink("#fff3a0", 732),
    "moth_ember": _ink("#d89020", 733),
})
FLAT_COLORS.update({
    "moth_fur_dk": "#4a433b", "moth_fur": "#6a6152", "moth_fur_lt": "#8e8370", "moth_fur_pale": "#bcae90", "moth_band": "#b4a07a",
    "moth_chit_dk": "#30271f", "moth_chit": "#4f3f31", "moth_chit_lt": "#7e664c", "moth_eye": "#1a161e", "moth_eye_hi": "#8e807c",
    "moth_wing": "#c8b89c", "moth_wing_lt": "#e0d6c0", "moth_eyespot": "#7a6040", "moth_vein": "#46301f", "moth_wing_edge": "#62462c",
    "moth_glow": "#ffd23c", "moth_glow_hi": "#fff3a0", "moth_ember": "#d89020",
})

# 腐沼蟾（fu_zhao_chan）：橄榄蟾皮三档 / 疣 / 金眼 / 湿亮口腔与舌
PAINTERS.update({
    "toad_skin_dk": _toad_skin("#3a3622", "#524a2c", 801),
    "toad_skin": _toad_skin("#524a2c", "#6e6338", 802),
    "toad_skin_lt": _toad_skin("#6e6338", "#8e8148", 803),
    "toad_wart": _toad_skin("#6a5238", "#85694a", 804, pits=0.12),
    "toad_wart_lt": _toad_skin("#85694a", "#a88a62", 805, pits=0.1),
    "toad_eye": _toad_eye(811),
    "wet_mouth": _wet_flesh("#964a4e", 821),
    "wet_tongue": _wet_flesh("#d27880", 822),
    "wet_tongue_dk": _wet_flesh("#a85a64", 823),
})
FLAT_COLORS.update({
    "toad_skin_dk": "#46412a", "toad_skin": "#5e5632", "toad_skin_lt": "#7e7240", "toad_wart": "#74593e", "toad_wart_lt": "#947650",
    "toad_eye": "#d8b44c", "wet_mouth": "#964a4e", "wet_tongue": "#d27880", "wet_tongue_dk": "#a85a64",
})

# 藓衣蜗（xian_yi_wo）：蜗肉三档 / 内脏轮廓 / 触角 / 暗紫光（靠对比度）/ 螺壳四档 / 地衣 / 黏液膜
PAINTERS.update({
    "slug_flesh": _slug_flesh("#bcc0b4", "#d8dccf", 841),
    "slug_flesh_lt": _slug_flesh("#d6dacb", "#eef0e4", 842, sheen=0.16),
    "slug_flesh_dk": _slug_flesh("#9ca29a", "#b6bbb0", 843),
    "slug_organ": _slug_flesh("#4e3e58", "#6c5878", 844, sheen=0.05),
    "slug_organ_red": _slug_flesh("#744a52", "#92626a", 845, sheen=0.05),
    "slug_stalk": _slug_flesh("#8e7e84", "#aa9aa0", 846),
    "slug_stalk_dk": _slug_flesh("#665a66", "#80727e", 847),
    "slug_mouth": _slug_flesh("#94706a", "#b48a82", 848, sheen=0.14),
    "glow_purple": _ink("#8a6ed0", 849),
    "glow_purple_hi": _ink("#e2ccff", 850),
    "snail_shell": _snail_shell("#8c7c64", "#a8977c", 851),
    "snail_shell_dk": _snail_shell("#665a48", "#7a6c58", 852),
    "snail_shell_deep": _snail_shell("#403830", "#524838", 853),
    "snail_shell_lt": _snail_shell("#b4a488", "#cdbfa4", 854, ridge=0.1),
    "lichen_crust": _lichen_crust("#c2c696", 855),
    "slime_film": _slime_film("#aebcae", 856),
    "slime_film_dk": _slime_film("#7c8e82", 857),
    "slime_bubble": _slime_film("#d6e4d6", 858, bright=0.1),
})
FLAT_COLORS.update({
    "slug_flesh": "#c4c8bc", "slug_flesh_lt": "#dde0d2", "slug_flesh_dk": "#a2a89f", "slug_organ": "#58485f", "slug_organ_red": "#7c545b",
    "slug_stalk": "#96868c", "slug_stalk_dk": "#6e626e", "slug_mouth": "#9c766f", "glow_purple": "#8a6ed0", "glow_purple_hi": "#e2ccff",
    "snail_shell": "#928268", "snail_shell_dk": "#6c604c", "snail_shell_deep": "#443c32", "snail_shell_lt": "#b8a88c",
    "lichen_crust": "#c2c696", "slime_film": "#b2c0b2", "slime_film_dk": "#82948a", "slime_bubble": "#dae8da",
})


# ---- 冰晶 / 霜毛 / 白雾（冻髓鹿批次补充）-----------------------------------------------


def _ice(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """结晶灵气：顺晶柱长轴的竖向棱面（一条条明暗不同的刻面），内部几道更亮的白纹，尖端和边缘最亮；
    不做发光，只靠面与面的明暗对比读出「冰」。"""
    deep, light = hex_color(base_a), hex_color(base_b)
    glint = hex_color("#f2fafc")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            facet = value_noise((across * 2.2, along * 0.25, 0.0), 1.0, seed)
            color = mix(deep, light, 0.25 + 0.75 * facet)
            if value_noise((across * 4.0, along * 0.35, 0.0), 1.0, seed + 1) > 0.78:
                color = mix(color, glint, 0.55)               # 内部白纹
            if texel_noise(p, seed + 2) > 0.94:
                color = shade(color, -0.2)                    # 零星气泡暗点
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, glint, 0.3)                # 棱线反光
            return color

        return paint

    return factory


def _mist(seed: int) -> Callable[[dict], Painter]:
    """白雾：近白，边缘一圈略暗略蓝，中间偶有更亮的絮点；靠和背景的明度差读出来。"""
    core, rim = hex_color("#f2f6f8"), hex_color("#c4d2dc")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = mix(core, rim, 0.5 * value_noise(p, 1.6, seed) + 0.3 * texel_noise(p, seed + 1))
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, rim, 0.5)
            return color

        return paint

    return factory


PAINTERS.update({
    "frost_fur_ash": _fur("#a29c90", "#c0bbb0", 761, bald=0.04),
    "frost_fur_pale": _fur("#c8c8c2", "#e4e4de", 762),
    "frost_fur_dark": _fur("#5a524b", "#7a7068", 763, bald=0.1),
    "bone_grey": _bone("#bcb6a6", 764),
    "ice_pale": _ice("#9cc0d6", "#d6ecf6", 765),
    "ice_deep": _ice("#4c82b4", "#86b4da", 766),
    "mist": _mist(767),
})
FLAT_COLORS.update({
    "frost_fur_ash": "#b0aa9e", "frost_fur_pale": "#d4d4ce", "frost_fur_dark": "#6a6058", "bone_grey": "#bcb6a6",
    "ice_pale": "#bcdcec", "ice_deep": "#74a4cc", "mist": "#e4ecf0",
})


# ---- 焦羽 / 雷痕（劫雷鸦批次补充）-----------------------------------------------------------


def _glint(base: str, core: str, seed: int) -> Callable[[dict], Painter]:
    """面心最亮、向边缘渐暗的小亮件（灵光碎块 / 雷痕 / 眼白 / 灵核 / 光点）：不做发光层，只靠对比，零星亮点。"""
    a, b = hex_color(base), hex_color(core)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            depth = min(_edge_distance(cube, p, axis), 2.0) / 2.0
            color = mix(a, b, depth * 0.85)
            return shade(color, (texel_noise(p, seed) - 0.5) * 0.08)

        return paint

    return factory


def _feather(base_a: str, base_b: str, seed: int, scorch: float = 0.0) -> Callable[[dict], Painter]:
    """羽片：顺长轴的细羽枝（横向细密明暗），中线一道更亮的羽轴，边缘一圈偏亮的羽缘；
    scorch > 0 时再撒焦斑（发暗的卷边）与零星灰烬亮点。"""
    a, b = hex_color(base_a), hex_color(base_b)
    char, ash = shade(a, -0.4), hex_color("#9a9690")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 4.2, 0.35, seed)
            color = mix(a, b, streak)
            if value_noise(p, 2.4, seed + 1) > 0.62:
                color = shade(color, -0.12)                   # 一片一片发暗的羽区
            long_axis = _long_axis(cube)
            if axis != long_axis:
                across_axis = 3 - long_axis - axis
                centre = (cube["from"][across_axis] + cube["to"][across_axis]) / 2
                if abs(p[across_axis] - centre) < 0.3:
                    color = shade(color, 0.16)                # 羽轴
            if scorch and value_noise(p, 1.8, seed + 2) > 1.0 - 0.45 * scorch:
                color = mix(color, char, 0.6)                 # 焦斑
            if scorch and texel_noise(p, seed + 3) > 0.96:
                color = mix(color, ash, 0.5)                  # 灰烬点
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.1)
            return color

        return paint

    return factory


PAINTERS.update({
    "crow_black": _feather("#1c1a22", "#322e3a", 811),
    "crow_dark": _feather("#2e2a36", "#4a4454", 812),
    "crow_char": _feather("#6a4430", "#946240", 813, scorch=1.0),
    "crow_ash": _feather("#68666c", "#8c8a90", 814, scorch=0.6),
    "bolt_blue": _glint("#6ea8e8", "#eaf8ff", 815),
    "eye_white": _glint("#cdd2ec", "#ffffff", 816),
    "crow_beak": _horn("#524238", 817),
    "crow_claw": _horn("#26222a", 818),
})
FLAT_COLORS.update({
    "crow_black": "#282630", "crow_dark": "#3c3846", "crow_char": "#7e5236", "crow_ash": "#7c7a80",
    "bolt_blue": "#a0d0ff", "eye_white": "#fafaff", "crow_beak": "#4e4038", "crow_claw": "#24202a",
})


# ---- 水母伞肉 / 灵核（浮灵母批次补充）-------------------------------------------------------


def _jelly(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """伞肉 / 口腕：近半透的浅色胶质——两档色柔和过渡、顺长轴的细淡纹、一团团偏冷的暗云、零星高光点，边缘提亮。"""
    a, b = hex_color(base_a), hex_color(base_b)
    cloud = hex_color("#7890a0")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            streak = _grain_noise(cube, p, axis, 2.4, 0.3, seed)
            color = mix(a, b, 0.55 * value_noise(p, 2.2, seed + 1) + 0.45 * streak)
            if value_noise(p, 3.6, seed + 2) > 0.66:
                color = mix(color, cloud, 0.16)               # 偏冷的暗云
            if texel_noise(p, seed + 3) > 0.97:
                color = shade(color, 0.14)                    # 高光点
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.08)
            return color

        return paint

    return factory


PAINTERS.update({
    "bell_cream": _jelly("#dedcc8", "#f0eee0", 821),
    "bell_grey": _jelly("#9cb0b6", "#c2d0d4", 822),
    "bell_frill": _jelly("#cdbf9e", "#e2d8bc", 823),
    "bell_vein": _ink("#2a3342", 824),
    "bell_vein_slate": _ink("#62708a", 825),
    "spirit_core": _glint("#6cdccc", "#d6fff6", 826),
    "spirit_mote": _glint("#d8f4ee", "#ffffff", 827),
})
FLAT_COLORS.update({
    "bell_cream": "#e6e4d2", "bell_grey": "#b0c0c4", "bell_frill": "#d4c8a8", "bell_vein": "#2e3644",
    "bell_vein_slate": "#667284", "spirit_core": "#8cecdc", "spirit_mote": "#f0fffa",
})


# ---- 蛆 / 脏器 / 灵光（天道尸蛆批次补充）-----------------------------------------------------


def _grub(base: str, seed: int) -> Callable[[dict], Painter]:
    """蛆皮：近半透的奶白底，大块柔和的明暗（皮下透出脏器的暗影）、细淡红血管、环向细皱，零星湿亮点；边缘略亮。"""
    color_base = hex_color(base)
    vein_color, crease = hex_color("#b8665a"), shade(color_base, -0.22)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(color_base, (value_noise(p, 2.8, seed) - 0.5) * 0.2 + (texel_noise(p, seed + 1) - 0.5) * 0.05)
            if abs(value_noise(p, 1.9, seed + 2) - 0.5) < 0.035:
                color = mix(color, vein_color, 0.4)          # 皮下血管
            if abs(value_noise(p, 1.2, seed + 3) - 0.5) < 0.05:
                color = mix(color, crease, 0.6)              # 环向皱
            if texel_noise(p, seed + 4) > 0.96:
                color = shade(color, 0.12)                   # 湿亮点
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.05)
            return color

        return paint

    return factory


PAINTERS.update({
    "grub_cream": _grub("#e8dcc2", 801),
    "grub_pink": _grub("#d8aa9c", 802),
    "rot_brown": _hide("#38221f", "#4c2f28", 803, crease_amount=1.0),
    "organ_red": _skin("#86382f", 804, vein=0.05),
    "glint_amber": _glint("#d8902c", "#ffe08a", 805),
    "glint_core": _glint("#ffe9a0", "#fffbe8", 806),
    "ichor": _hide("#5e3e20", "#7c5630", 807, crease_amount=0.3),
})
FLAT_COLORS.update({
    "grub_cream": "#e0d4b8", "grub_pink": "#d0a294", "rot_brown": "#42291f", "organ_red": "#86382f",
    "glint_amber": "#eaaa44", "glint_core": "#fff0b8", "ichor": "#6c4a28",
})


# ---- 颅骨壳 / 蟹甲 / 藤壶 / 苔（骨笼蟹批次补充）-------------------------------------------


def _lichen(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """壳上的苔：深浅两档灰绿成团，团内细碎的暗点（苔缝）与浅色尖点（苔梢），边缘略发暗。"""
    a, b = hex_color(base_a), hex_color(base_b)
    gap, tip = shade(a, -0.35), shade(b, 0.28)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = mix(a, b, value_noise(p, 1.6, seed) * 0.7 + value_noise(p, 0.8, seed + 1) * 0.3)
            if texel_noise(p, seed + 2) > 0.82:
                color = mix(color, gap, 0.7)                  # 苔缝
            elif texel_noise(p, seed + 3) > 0.9:
                color = mix(color, tip, 0.7)                  # 苔梢
            if _edge_distance(cube, p, axis) < 0.4:
                color = shade(color, -0.12)
            return color

        return paint

    return factory


def _barnacle(base: str, seed: int) -> Callable[[dict], Painter]:
    """藤壶：灰白钙壳，一圈圈生长纹（同心环）+ 密密的小坑 + 零星暗点。"""
    c = hex_color(base)
    ring_dark, pit = shade(c, -0.3), shade(c, -0.55)

    def factory(cube: dict) -> Painter:
        centre = [(cube["from"][i] + cube["to"][i]) / 2 for i in range(3)]

        def paint(p: Vec, axis: int) -> Color:
            d = math.dist(p, centre)
            color = shade(c, (value_noise(p, 1.4, seed) - 0.5) * 0.22)
            if (d * 1.3) % 1.0 < 0.22:
                color = mix(color, ring_dark, 0.55)           # 生长纹
            if texel_noise(p, seed + 1) > 0.9:
                color = mix(color, pit, 0.7)                  # 小坑
            return color

        return paint

    return factory


PAINTERS.update({
    "skull_bone": _bone("#dcd2b6", 831),
    "skull_bone_old": _bone("#a49c8a", 832),
    "crab_shell": _chitin("#b8683c", "#d88a56", 833, 0.6),
    "crab_shell_dark": _chitin("#6e3a26", "#8c4e34", 834),
    "lichen": _lichen("#566230", "#8c9a4c", 835),
    "barnacle": _barnacle("#a4a49c", 836),
})
FLAT_COLORS.update({
    "skull_bone": "#d2c8ac", "skull_bone_old": "#9a9282", "crab_shell": "#c47848", "crab_shell_dark": "#7c4430",
    "lichen": "#76844a", "barnacle": "#a4a49c",
})

# 腐根熊（fu_gen_xiong）：棕毛两档 / 枯根（苍白）/ 菌柄木 / 托架菌盖 / 菌褶
PAINTERS.update({
    "bear_fur": _fur("#664329", "#946444", 831),
    "bear_fur_dark": _fur("#3a281b", "#5c402a", 832),
    "bear_root": _wood("#c4b790", "#a49872", 833),
    "bear_stalk": _wood("#7a5a3c", "#946e48", 834),
    "shelf_cap": _shelf_cap("#b85c30", "#d8924e", 835),
    "shelf_gill": _shelf_gill("#e4cc98", 836),
})
FLAT_COLORS.update({
    "bear_fur": "#5c3e2a", "bear_fur_dark": "#38281c", "bear_root": "#b4a880", "bear_stalk": "#86643f",
    "shelf_cap": "#c87440", "shelf_gill": "#e4cc98",
})


# ---- 红肉 / 肌腱 / 红毛 / 剥皮硬皮（剥皮豺批次补充）-------------------------------------------


def _muscle(base_a: str, base_b: str, seed: int, sinew: float = 0.0) -> Callable[[dict], Painter]:
    """裸露的肌肉：顺长轴的肌束纤维（深浅交替的细条），束间一线暗缝，湿亮的高光点；sinew > 0 时再有发白的筋腱条。"""
    a, b = hex_color(base_a), hex_color(base_b)
    gap, wet, tendon = shade(a, -0.4), shade(b, 0.3), hex_color("#d2b8a4")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            fibre = _grain_noise(cube, p, axis, 5.0, 0.3, seed)
            color = mix(a, b, fibre)
            color = shade(color, (value_noise(p, 1.9, seed + 1) - 0.5) * 0.18)
            if abs(_grain_noise(cube, p, axis, 2.4, 0.25, seed + 2) - 0.5) < 0.04:
                color = mix(color, gap, 0.7)                  # 肌束间的暗缝
            if texel_noise(p, seed + 3) > 0.94:
                color = mix(color, wet, 0.6)                  # 湿亮点
            if sinew and abs(_grain_noise(cube, p, axis, 3.2, 0.2, seed + 4) - 0.5) < 0.05 * sinew:
                color = mix(color, tendon, 0.65)              # 筋腱
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, -0.1)
            return color

        return paint

    return factory


PAINTERS.update({
    "muscle_raw": _muscle("#a8262e", "#d4424a", 841, sinew=0.6),
    "muscle_dark": _muscle("#5a1620", "#7e2430", 842, sinew=1.0),
    "dhole_fur_red": _fur("#8a3228", "#b04a3a", 843, bald=0.06),
    "dhole_fur_dark": _fur("#4a201c", "#6a3028", 844, bald=0.06),
    "dhole_skin_pale": _skin("#c8947c", 845, vein=0.05),
    "spine_bone": _bone("#e6dcc4", 846),
})
FLAT_COLORS.update({
    "muscle_raw": "#bc343c", "muscle_dark": "#6a1c28", "dhole_fur_red": "#9c3c30", "dhole_fur_dark": "#58281f",
    "dhole_skin_pale": "#c8947c", "spine_bone": "#dcd0b6",
})


# ---- 野牛垂毛 / 角 / 旧伤疤（断角野牛批次补充）------------------------------------------------
# 垂毛是竖直的细长毛条，用 _fur_upright（沿 y 拉长的毛束），不用沿身长 z 的 _fur。
PAINTERS.update({
    "bison_fur_dark": _fur_upright("#3a2f26", "#52443a", 851),
    "bison_fur": _fur_upright("#5c4b3b", "#7a6650", 852),
    "bison_fur_lit": _fur_upright("#7a6852", "#9a8468", 853),
    "bison_fur_rust": _fur_upright("#8a603a", "#a87a48", 854),
    "bison_horn": _horn("#bcae90", 855),
    "bison_horn_dark": _horn("#7a6c56", 856),
    "bison_horn_fresh": _horn("#e4d8bc", 857),
    "scar_tissue": _skin("#b49a88", 858, vein=0.0),
})
FLAT_COLORS.update({
    "bison_fur_dark": "#46392f", "bison_fur": "#6a5844", "bison_fur_lit": "#8a765e", "bison_fur_rust": "#98703f",
    "bison_horn": "#b0a284", "bison_horn_dark": "#6e6050", "bison_horn_fresh": "#d8ccb0", "scar_tissue": "#b49a88",
})


# ---- 旱蛇的干皮 / 碎鳞 / 血痂 / 干舌（旱骨蛇批次补充）------------------------------------------
# 库里的 snake_scale 是青灰色的活蛇鳞；旱骨蛇是晒干的赭黄纸皮，鳞片小而翘、裂纹多。
PAINTERS.update({
    "dry_scale": _scales("#9a8062", "#b49a78", 861, 1.3),
    "dry_scale_dark": _scales("#605040", "#7a6650", 862, 1.3),
    "dry_scale_lit": _scales("#bca888", "#d6c4a4", 863, 1.3),
    "dry_flake": _scales("#d2bea0", "#e6d6b8", 864, 1.0),
    "dry_rust": _hide("#7a4a34", "#946248", 865, crease_amount=0.8),
    "dry_crack": _ink("#3a2e24", 866),
    "dry_tongue": _skin("#5c3830", 867, vein=0.0),
})
FLAT_COLORS.update({
    "dry_scale": "#a48a6c", "dry_scale_dark": "#6a5844", "dry_scale_lit": "#c8b494", "dry_flake": "#dcc9aa",
    "dry_rust": "#86563c", "dry_crack": "#3a2e24", "dry_tongue": "#5c3830",
})


def _bark(base_a: str, base_b: str, seed: int, flake: float = 0.0, weather: float = 0.0) -> Callable[[dict], Painter]:
    """枯树皮：顺长轴的深色纵裂（疏密不匀、略带弯）+ 隔一段一道横裂，把皮切成长条鳞片；
    端面是细年轮；flake > 0 时撒翘起的亮色皮屑，weather > 0 时撒灰白风化斑。"""
    a, b = hex_color(base_a), hex_color(base_b)
    fissure, scale_gap, curl, ash = shade(a, -0.5), shade(a, -0.35), shade(b, 0.3), hex_color("#9a9686")

    def factory(cube: dict) -> Painter:
        long_axis = _long_axis(cube)

        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            if axis == long_axis:       # 端面：细年轮
                ring = 0.5 + 0.5 * math.sin(across * 4.0 + 2.0 * value_noise(p, 1.4, seed))
                return shade(mix(a, b, ring), -0.15)
            base = value_noise((across * 1.8, along * 0.3, 0.0), 1.0, seed)
            color = mix(a, b, (0.65 * base + 0.35 * texel_noise(p, seed + 1)) * 1.3 - 0.15)
            wobble = 1.4 * value_noise((across * 0.7, along * 0.2, 0.0), 1.0, seed + 2)
            if (across * 0.95 + wobble) % 1.0 < 0.2:
                color = mix(color, fissure, 0.8)               # 纵裂
            if (along * 0.45 + 0.8 * value_noise(p, 1.6, seed + 3)) % 1.0 < 0.09:
                color = mix(color, scale_gap, 0.7)             # 横裂：皮条的断口
            if flake and texel_noise(p, seed + 4) > 1.0 - flake:
                color = mix(color, curl, 0.7)                  # 翘起的亮皮屑
            if weather and value_noise(p, 2.6, seed + 5) > 1.0 - weather:
                color = mix(color, ash, 0.55)                  # 风化灰斑
            if _edge_distance(cube, p, axis) < 0.4:
                color = shade(color, -0.1)
            return color

        return paint

    return factory


def _bark_knot(cube: dict) -> Painter:
    """节疤：深褐同心环，中心最暗，向外一圈圈变亮。"""
    core, ring = hex_color("#2c2018"), hex_color("#6a5036")

    def paint(p: Vec, axis: int) -> Color:
        depth = min(_edge_distance(cube, p, axis), 2.0) / 2.0
        wave = 0.5 + 0.5 * math.sin(depth * 6.0 + texel_noise(p, 841))
        return mix(core, ring, 0.25 + 0.5 * depth * wave)

    return paint


PAINTERS.update({
    "bark": _bark("#76624a", "#9a8566", 840, flake=0.04),
    "bark_dark": _bark("#4a3c2c", "#6e5a42", 842),
    "bark_light": _bark("#9c8668", "#c2ab88", 843, flake=0.1),
    "bark_grey": _bark("#847c6a", "#aaa28c", 844, weather=0.3),
    "bark_rust": _bark("#885c3a", "#b08052", 845, flake=0.05),
    "bark_knot": _bark_knot,
})
FLAT_COLORS.update({
    "bark": "#76624a", "bark_dark": "#4e402e", "bark_light": "#9c8668", "bark_grey": "#847c6a", "bark_rust": "#885c3a",
    "bark_knot": "#3c2c20",
})


# ---- 灰褐长毛 / 苔岩 / 草绳 / 深色猿脸（碎岩猿批次补充）---------------------------------------
# 长毛是竖直垂下的毛束（_fur_upright）；岩是崩口片状的灰岩（_stone_chipped）；草绳是拧股的干草（_rope）；
# 脸和掌是深色裸皮（_skin，不画血管）。灰绿苔用库里已有的 lichen。
PAINTERS.update({
    "ape_fur_dark": _fur_upright("#3c352f", "#544b42", 871),
    "ape_fur": _fur_upright("#5e554a", "#7a6f60", 872),
    "ape_fur_lit": _fur_upright("#7e7364", "#9a8e7c", 873),
    "ape_fur_grey": _fur_upright("#a49a88", "#c0b6a2", 874),
    "ape_skin": _skin("#54423a", 875, vein=0.0),
    "ape_skin_lit": _skin("#705a4c", 876, vein=0.0),
    "rock_grey": _stone_chipped("#7c7870", "#9a968a", 877),
    "rock_grey_dark": _stone_chipped("#565250", "#6e6a64", 878),
    "rock_grey_lit": _stone_chipped("#a29e90", "#bab6a6", 879),
    "rock_grey_crack": _stone_chipped("#2c2a2a", "#3c3a38", 880),
    "straw_rope": _rope("#a08850", "#c0a468", 881),
    "straw_rope_dark": _rope("#6e5636", "#8a6e44", 882),
})
FLAT_COLORS.update({
    "ape_fur_dark": "#483f37", "ape_fur": "#6a6054", "ape_fur_lit": "#8c8070", "ape_fur_grey": "#b2a894",
    "ape_skin": "#54423a", "ape_skin_lit": "#705a4c", "rock_grey": "#8a867c", "rock_grey_dark": "#625e5a",
    "rock_grey_lit": "#aeaa9a", "rock_grey_crack": "#343232", "straw_rope": "#b09a5c", "straw_rope_dark": "#7c6240",
})


def _plucked_skin(base_a: str, base_b: str, seed: int, flush: float = 0.0) -> Callable[[dict], Painter]:
    """拔光羽毛后的灰粉皱皮：两档灰粉色成片、细皱纹、一粒粒毛囊小点（暗点带浅边）；
    flush > 0 时带血色的红斑（羽根桩 / 新伤处）。"""
    a, b = hex_color(base_a), hex_color(base_b)
    crease, pore, blood = shade(a, -0.3), shade(a, -0.45), hex_color("#9c4a48")

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = mix(a, b, value_noise(p, 2.2, seed) * 0.7 + value_noise(p, 1.1, seed + 1) * 0.3)
            if abs(value_noise(p, 1.4, seed + 2) - 0.5) < 0.05:
                color = mix(color, crease, 0.6)               # 皱纹
            if texel_noise(p, seed + 3) > 0.88:
                color = mix(color, pore, 0.65)                # 毛囊点
            elif texel_noise(p, seed + 4) > 0.95:
                color = shade(color, 0.12)                    # 毛囊的浅边
            if flush and value_noise(p, 1.9, seed + 5) > 1.0 - 0.5 * flush:
                color = mix(color, blood, 0.5)                # 血色红斑
            if _edge_distance(cube, p, axis) < 0.4:
                color = shade(color, -0.08)
            return color

        return paint

    return factory


PAINTERS.update({
    "owl_feather": _feather("#54443a", "#86725a", 850),
    "owl_feather_dk": _feather("#30281f", "#524434", 851),
    "owl_feather_lt": _feather("#86745c", "#b09c7c", 852),
    "owl_disc": _feather("#a89878", "#cbbb98", 853),
    "owl_quill": _bone("#d8c8a0", 854),
    "owl_bare": _plucked_skin("#9a766e", "#b8928a", 855),
    "owl_bare_dk": _plucked_skin("#725650", "#8e6c66", 856),
    "owl_root": _plucked_skin("#a8605c", "#c88078", 857, flush=1.0),
    "owl_eye_cloud": _glint("#c4c4b8", "#f4f4e8", 858),
    "owl_eye_cloud_dk": _glint("#848480", "#b8b8ae", 859),
    "owl_eye_ring": _ink("#4a2c10", 860),
    "owl_scale": _chitin("#7c6c5c", "#9a8a78", 861, 0.8),
    "owl_scale_dk": _chitin("#54483c", "#6e6050", 862, 0.6),
    "owl_talon_tip": _horn("#78685a", 863),
})
FLAT_COLORS.update({
    "owl_feather": "#76644e", "owl_feather_dk": "#4a3e32", "owl_feather_lt": "#a08e72", "owl_disc": "#b0a07e",
    "owl_quill": "#d0c098", "owl_bare": "#a07c74", "owl_bare_dk": "#785a54", "owl_root": "#a8645e",
    "owl_eye_cloud": "#d6d6ca", "owl_eye_cloud_dk": "#96968e", "owl_eye_ring": "#503014",
    "owl_scale": "#82725f", "owl_scale_dk": "#584c40", "owl_talon_tip": "#7c6c5e",
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


def _bleed_face_edges(atlas, uvs):
    """每个面的 UV 矩形向右、向下各外延一个 texel，抄边缘色（只改没用到的洋红区）。

    离线渲染器按 int(u)/int(v) 取样，面边缘的像素会落在矩形外一个 texel 上（右 / 下边是 u == u2 的整数边界，
    左 / 上边是浮点误差）；那里若是洋红底就会在边缘漏出一行 / 一列洋红点。真正的 UV 越界仍然是洋红，
    只是不再被边缘取样冒充。
    """
    magenta = (255, 0, 255, 255)
    atlas = atlas.copy()
    px = atlas.load()
    for face_uvs in uvs:
        for u1, v1, u2, v2 in face_uvs.values():
            x0, x1 = int(min(u1, u2)), int(max(u1, u2))
            y0, y1 = int(min(v1, v2)), int(max(v1, v2))
            if x1 <= x0 or y1 <= y0:
                continue
            for y in range(max(y0 - 1, 0), min(y1 + 1, atlas.height)):
                for x in range(max(x0 - 1, 0), min(x1 + 1, atlas.width)):
                    if x0 <= x < x1 and y0 <= y < y1:
                        continue
                    if px[x, y] == magenta:
                        px[x, y] = px[min(max(x, x0), x1 - 1), min(max(y, y0), y1 - 1)]
    return atlas


def textured_rig_doc(rig, name: str, material_map: Dict[str, str], flat: bool = False) -> dict:
    """生物流水线共用：在 rigkit.Rig 的绑定稿 bbmodel 上换成每面独立 UV 的像素图集（骨骼层级、几何不动）。

    rig 里每个立方体的材质名按贴图色块位置反推（rigkit 的 color 字段会撞号）；material_map 把生成器自己的
    材质名映射成本库材质名，同名的可以不列。flat=True 出平色对照版。
    """
    import base64
    import copy
    import io

    per_row = rig.tex // rig.swatch
    names = list(rig.mats)
    cubes = []
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        material = names[int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)]
        material = material_map.get(material, material)
        if material not in PAINTERS:
            raise KeyError(f"画笔库里没有材质 {material}（立方体 {el['name']}）")
        cubes.append({"name": el["name"], "from": el["from"], "to": el["to"], "material": material})
    atlas, uvs = build_atlas(cubes, override=flat_override if flat else handheld_override)
    atlas = _bleed_face_edges(atlas, uvs)
    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    doc = copy.deepcopy(rig.bbmodel(name))
    doc["resolution"] = {"width": atlas.width, "height": atlas.height}
    doc["textures"][0]["source"] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")
    for el, face_uvs in zip(doc["elements"], uvs):
        for face, uv in face_uvs.items():
            el["faces"][face]["uv"] = uv
    return doc


# ---- 灰白穴鳞（灰鳞穴蜥批次补充）---------------------------------------------------------
# 库里的 lizard_scale 是绿色沼泽蜥；地穴蜥没有日照，鳞是灰白色，骨瘤比鳞亮一档、更粗糙。
PAINTERS.update({
    "cave_scale": _scales("#a2a49e", "#bebeb6", 771),
    "cave_scale_pale": _scales("#cfcfc4", "#e8e8dc", 772),
    "cave_scale_dark": _scales("#6e7076", "#8a8c92", 773),
    "cave_belly": _scales("#d8d0ba", "#ece4cc", 774, 1.8),
    "cave_scar": _skin("#b49490", 775, vein=0.0),
})
FLAT_COLORS.update({
    "cave_scale": "#b0b0a8", "cave_scale_pale": "#dcdcd0", "cave_scale_dark": "#7c7e84",
    "cave_belly": "#d0c8b0", "cave_scar": "#b49490",
})


# ---- 末土穿戴装备（骨架背篓 / 锈框背架 / 杂皮行囊 / 粗麻斗篷 / 枯藤腰带）补充 ------------------------
# 草编、粗麻、枯藤、陶、葫芦是新画法；骨 / 锈铁 / 皮 / 绳复用上面的工厂，只换色调。
# 面内方向一律用 _face_plane（水平 / 竖直），不依赖立方体长轴，薄片与方块上的编纹都不会错向。


def _weave(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """草编：篾条一上一下交织——面内 1.5px 见方的格子，奇偶格明暗交替、纹理走向相反，
    格缝一线深色，零星翘起的亮毛刺。"""
    a, b = hex_color(base_a), hex_color(base_b)
    gap, fuzz = shade(a, -0.4), shade(b, 0.2)
    cell = 1.5

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            u, v = _face_plane(p, axis)
            over = (math.floor(u / cell) + math.floor(v / cell)) % 2 == 0
            color = mix(a, b, 0.75 if over else 0.2)
            stretch = (0.4, 2.4) if over else (2.4, 0.4)           # 篾条的纹理沿各自的走向拉长
            grain = value_noise((u * stretch[0], v * stretch[1], 0.0), 1.0, seed)
            color = shade(color, (grain - 0.5) * 0.22 + (texel_noise(p, seed + 1) - 0.5) * 0.1)
            fu, fv = (u / cell) % 1.0, (v / cell) % 1.0
            if min(fu, 1 - fu, fv, 1 - fv) < 0.17:
                color = mix(color, gap, 0.65)                      # 格缝
            if texel_noise(p, seed + 2) > 0.95:
                color = mix(color, fuzz, 0.7)                      # 翘起的毛刺
            return color

        return paint

    return factory


def _burlap(base: str, seed: int, grime: float = 0.25) -> Callable[[dict], Painter]:
    """粗麻布：粗经粗纬、线粗细不匀（有线结），线交叉处透暗孔，边缘毛须，一块块脏污（grime = 脏污占比）。"""
    c = hex_color(base)
    gap, slub, dirt = shade(c, -0.38), shade(c, 0.16), shade(c, -0.28)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            u, v = _face_plane(p, axis)
            warp = value_noise((u * 1.9, v * 0.12, 0.0), 1.0, seed)        # 经线：竖向粗线
            weft = value_noise((u * 0.12, v * 1.9, 0.0), 1.0, seed + 1)    # 纬线：横向粗线
            over = (math.floor(u) + math.floor(v)) % 2 == 0
            color = shade(c, ((warp if over else weft) - 0.5) * 0.3)
            if (u % 1.0) < 0.2 and (v % 1.0) < 0.2:
                color = mix(color, gap, 0.6)                               # 线交叉处的孔
            if texel_noise(p, seed + 2) > 0.93:
                color = mix(color, slub, 0.7)                              # 线结
            if value_noise(p, 1.8, seed + 3) > 1.0 - grime:
                color = mix(color, dirt, 0.55)                             # 脏污
            if _edge_distance(cube, p, axis) < 0.5 and texel_noise(p, seed + 4) > 0.45:
                color = shade(color, 0.14)                                 # 边缘毛须
            return color

        return paint

    return factory


def _vine(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """枯藤：斜向拧绞的藤股（比绳粗、股间缝宽），藤皮有纵向裂纹与疙瘩，零星剥落露出浅色木芯。"""
    dark, light = hex_color(base_a), hex_color(base_b)
    core = shade(light, 0.22)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            phase = ((along * 0.8 + across * 1.1) / 2.2) % 1.0
            band = 0.5 + 0.5 * math.sin(phase * 2 * math.pi)
            color = mix(dark, light, band * 0.75 + 0.25 * texel_noise(p, seed))
            if abs(value_noise((across * 2.0, along * 0.3, 0.0), 1.0, seed + 1) - 0.5) < 0.06:
                color = shade(color, -0.32)                                # 纵向裂纹
            knot = value_noise(p, 1.7, seed + 2)
            if knot > 0.84:
                color = mix(color, shade(dark, -0.3), 0.7)                 # 疙瘩
            elif texel_noise(p, seed + 3) > 0.94:
                color = mix(color, core, 0.8)                              # 剥落露出木芯
            return color

        return paint

    return factory


def _clay(base: str, seed: int) -> Callable[[dict], Painter]:
    """陶：烧成的橙褐斑驳 + 细砂粒 + 烟熏暗斑 + 细裂纹，边缘略亮。"""
    c = hex_color(base)
    soot, grit, crack = shade(c, -0.35), shade(c, 0.22), shade(c, -0.55)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(c, (value_noise(p, 2.2, seed) - 0.5) * 0.28)
            if value_noise(p, 1.5, seed + 1) > 0.78:
                color = mix(color, soot, 0.6)                              # 烟熏
            if texel_noise(p, seed + 2) > 0.9:
                color = mix(color, grit, 0.7)                              # 砂粒
            if abs(value_noise(p, 2.8, seed + 3) - 0.5) < 0.025:
                color = crack
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.08)
            return color

        return paint

    return factory


def _gourd(base: str, seed: int) -> Callable[[dict], Painter]:
    """干葫芦：蜡黄光面带大块深浅斑，细皱，偶有磕痕。"""
    c = hex_color(base)
    spot, scar = shade(c, -0.3), shade(c, -0.5)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            color = shade(c, (value_noise(p, 1.3, seed) - 0.5) * 0.3)
            if value_noise(p, 2.0, seed + 1) > 0.75:
                color = mix(color, spot, 0.55)                             # 深斑
            if abs(value_noise(p, 2.6, seed + 2) - 0.5) < 0.04:
                color = mix(color, spot, 0.7)                              # 皱纹
            if texel_noise(p, seed + 3) > 0.96:
                color = scar
            return color

        return paint

    return factory


def _plain(base: str, seed: int) -> Callable[[dict], Painter]:
    """纯色占位（check_tex 里穿装备的灰色占位玩家）：只带极淡的噪点，不抢装备的戏。"""
    c = hex_color(base)

    def factory(cube: dict) -> Painter:
        return lambda p, axis: shade(c, (texel_noise(p, seed) - 0.5) * 0.04)

    return factory


# 名字 → (画笔工厂, 平色对照版的颜色)；平色取装备 Round 1 调色板的原色，方便和几何版对照
GEAR_MATERIALS: Dict[str, Tuple[Callable[[dict], Painter], str]] = {
    "gear_weave": (_weave("#7c6638", "#a88e58", 901), "#a08a5c"),
    "gear_weave_lt": (_weave("#9a8248", "#c4aa6c", 902), "#baa470"),
    "gear_weave_dk": (_weave("#4e3e24", "#6e5834", 903), "#705e3e"),
    "gear_weave_rim": (_weave("#3e3018", "#5c4a2a", 904), "#604e34"),
    "gear_cord": (_rope("#6e5430", "#a88c58", 905), "#967e54"),
    "gear_cord_dk": (_rope("#46361e", "#7a6038", 906), "#68583a"),
    "gear_hide": (_hide("#6a4630", "#86604a", 907), "#764e38"),
    "gear_hide_dk": (_hide("#46301f", "#5e4230", 908), "#543a2a"),
    "gear_hide_lt": (_hide("#8a643e", "#a8805a", 909), "#966c4e"),
    "gear_hide_grey": (_hide("#6c665e", "#8a8478", 910), "#706a62"),
    "gear_hide_pale": (_hide("#a08a64", "#bea680", 911), "#b09670"),
    "gear_hide_stain": (_hide("#54402e", "#6e5640", 912, crease_amount=0.8), "#5c4634"),
    "gear_hide_spot": (_hide("#3a2c24", "#54443a", 913), "#46362c"),
    "gear_iron": (_iron("#58504c", 0.3, 914), "#5c5450"),
    "gear_iron_lt": (_iron("#807870", 0.15, 915, hone=True), "#807872"),
    "gear_iron_dk": (_iron("#363230", 0.25, 916), "#3c3836"),
    "gear_rust": (_iron("#7a4224", 0.85, 917), "#965430"),
    "gear_rust_lt": (_iron("#8e5030", 0.9, 918), "#ba703e"),
    "gear_rust_dk": (_iron("#4e2a1a", 0.7, 919), "#683822"),
    "gear_clay": (_clay("#966046", 920), "#966046"),
    "gear_clay_dk": (_clay("#684230", 921), "#684230"),
    "gear_clay_lt": (_clay("#b27c5c", 922), "#b27e5c"),
    "gear_linen": (_burlap("#928064", 923), "#928064"),
    "gear_linen_dk": (_burlap("#705f4a", 924), "#70604a"),
    "gear_linen_lt": (_burlap("#ac9a7a", 925), "#ac9a7a"),
    "gear_linen_grey": (_burlap("#78726a", 926), "#787268"),
    "gear_linen_brn": (_burlap("#64503e", 927, grime=0.4), "#64503e"),
    "gear_patch": (_leather("#624436", 928), "#624436"),
    "gear_patch_dk": (_leather("#483226", 929), "#48322a"),
    "gear_vine": (_vine("#4e3a24", "#76593a", 930), "#745a3a"),
    "gear_vine_lt": (_vine("#6e5434", "#96794e", 931), "#967a50"),
    "gear_vine_dk": (_vine("#32261a", "#50402a", 932), "#503e2a"),
    "gear_vine_dry": (_vine("#8a7048", "#a88e62", 933), "#a88e62"),
    "gear_gourd": (_gourd("#9e6638", 934), "#9e6638"),
    "gear_gourd_dk": (_gourd("#764a2c", 935), "#764a2c"),
    "gear_gourd_lt": (_gourd("#bc8452", 936), "#bc8452"),
    "gear_cork": (_wood("#8a6c48", "#a08258", 937), "#967450"),
    "gear_wearer_body": (_plain("#969698", 938), "#969698"),
    "gear_wearer_limb": (_plain("#7a7a7e", 939), "#7a7a7e"),
    "gear_wearer_face": (_plain("#464650", 940), "#464650"),
}
PAINTERS.update({name: factory for name, (factory, _) in GEAR_MATERIALS.items()})
FLAT_COLORS.update({name: flat for name, (_, flat) in GEAR_MATERIALS.items()})


# ---- 末土腰挂件（灵草囊 / 矿石袋 / 水囊 / 防潮包 / 骨币匣）补充 ----------------------------------------
# 干草药叶 / 籽穗、矿石、丹砂粉、灰烬苔是新画法；皮 / 毛 / 角 / 木 / 绳 / 粗麻布复用上面的工厂，只换色调。


def _herb(base_a: str, base_b: str, seed: int) -> Callable[[dict], Painter]:
    """干草药叶：顺叶长的粗叶脉居中，两侧斜向细脉，叶面明暗不匀，边缘卷枯发深。"""
    dark, light = hex_color(base_a), hex_color(base_b)
    vein, curl = shade(light, 0.2), shade(dark, -0.3)

    def factory(cube: dict) -> Painter:
        long_axis = _long_axis(cube)

        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            color = mix(dark, light, 0.25 + 0.6 * value_noise(p, 1.6, seed))
            if axis != long_axis:
                centre = (cube["from"][3 - long_axis - axis] + cube["to"][3 - long_axis - axis]) / 2
                offset = abs(across - centre)
                if offset < 0.3:
                    return shade(vein, (texel_noise(p, seed + 1) - 0.5) * 0.1)         # 中脉
                if ((along * 0.9 - offset * 1.3) / 1.7) % 1.0 < 0.14:
                    color = mix(color, vein, 0.55)                                      # 侧脉
            if _edge_distance(cube, p, axis) < 0.5:
                color = mix(color, curl, 0.5)                                           # 卷枯的叶缘
            return shade(color, (texel_noise(p, seed + 2) - 0.5) * 0.12)

        return paint

    return factory


def _seedhead(base: str, seed: int) -> Callable[[dict], Painter]:
    """干籽穗：1px 一粒的籽，粒与粒之间一线暗缝，粒面亮暗交替，零星空壳。"""
    c = hex_color(base)
    gap, husk = shade(c, -0.42), shade(c, -0.18)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            u, v = _face_plane(p, axis)
            fu, fv = u % 1.0, v % 1.0
            cell = (math.floor(u) * 7 + math.floor(v) * 13) % 5
            color = shade(c, (cell - 2) * 0.06 + (texel_noise(p, seed) - 0.5) * 0.1)
            if min(fu, 1 - fu) < 0.14 or min(fv, 1 - fv) < 0.14:
                color = mix(color, gap, 0.6)
            if cell == 0 and texel_noise(p, seed + 1) > 0.5:
                color = husk                                                            # 空壳
            return color

        return paint

    return factory


def _ore(base: str, fleck: str, seed: int) -> Callable[[dict], Painter]:
    """矿石：大块切面——低频噪声量化成三档明暗（崩口的平面感），面间一道暗裂，零星亮晶点，边棱最亮。"""
    c, spark = hex_color(base), hex_color(fleck)
    crack = shade(c, -0.5)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            step = math.floor(value_noise(p, 2.6, seed) * 3.0) / 3.0
            color = shade(c, (step - 0.33) * 0.55 + (texel_noise(p, seed + 1) - 0.5) * 0.08)
            if abs(value_noise(p, 2.2, seed + 2) - 0.5) < 0.03:
                color = mix(color, crack, 0.75)
            if texel_noise(p, seed + 3) > 0.93:
                color = mix(color, spark, 0.85)
            if _edge_distance(cube, p, axis) < 0.5:
                color = shade(color, 0.14)
            return color

        return paint

    return factory


def _cinnabar_powder(base: str, seed: int) -> Callable[[dict], Painter]:
    """丹砂：朱红粉涂，厚薄不匀（厚处发暗结块、薄处透出底木），零星亮橙砂粒。"""
    c = hex_color(base)
    thick, thin, grain = shade(c, -0.3), mix(c, hex_color("#c88a60"), 0.4), shade(c, 0.35)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            n = value_noise(p, 1.9, seed)
            color = mix(c, thick, max(0.0, n - 0.5) * 1.6) if n > 0.5 else mix(c, thin, (0.5 - n) * 0.8)
            if texel_noise(p, seed + 1) > 0.9:
                color = mix(color, grain, 0.7)
            return shade(color, (texel_noise(p, seed + 2) - 0.5) * 0.1)

        return paint

    return factory


# 名字 → (画笔工厂, 平色对照版的颜色)；平色取各件 Round 1 调色板的原色
GEAR_WAIST_MATERIALS: Dict[str, Tuple[Callable[[dict], Painter], str]] = {
    "gear_leather": (_leather("#7c4832", 951), "#7c4832"),
    "gear_leather_dk": (_leather("#54322a", 952), "#54322a"),
    "gear_leather_lt": (_leather("#965c40", 953), "#965c40"),
    "gear_stitch": (_plain("#3e3024", 954), "#3e3024"),
    "gear_hide_pale_dk": (_hide("#8a7452", "#a89270", 955), "#a08c6c"),
    "gear_fur": (_fur("#b8a47c", "#d8c8a0", 956), "#d6c4a0"),
    "gear_hoof": (_horn("#3a2e28", 957), "#3a2e28"),
    "gear_hoof_dk": (_horn("#241c1a", 958), "#261e1c"),
    "gear_wood": (_wood("#7c5638", "#946a46", 959), "#8c6442"),
    "gear_wood_lt": (_wood("#946c48", "#b08256", 960), "#a67c54"),
    "gear_wood_dk": (_wood("#4e3624", "#664630", 961), "#5c4030"),
    "gear_cinnabar": (_cinnabar_powder("#ac2c24", 962), "#ac2c24"),
    "gear_cinnabar_dk": (_cinnabar_powder("#7e221e", 963), "#7e221e"),
    "gear_rag_beige": (_burlap("#b8ac8c", 964), "#bcb090"),
    "gear_rag_red": (_burlap("#94463a", 965), "#96362e"),
    "gear_red_cord": (_rope("#6e1c18", "#a83a30", 966), "#962c26"),
    "gear_herb_green": (_herb("#4e6234", "#7c9450", 967), "#70844e"),
    "gear_herb_green_dk": (_herb("#34462a", "#566e3a", 968), "#4e6238"),
    "gear_herb_pale": (_herb("#a08e64", "#c8b888", 969), "#c8b88a"),
    "gear_herb_seed": (_seedhead("#d8c898", 970), "#e2d6aa"),
    "gear_herb_brown": (_herb("#6e4a2c", "#94683c", 971), "#8c603a"),
    "gear_herb_brown_dk": (_herb("#4a3020", "#6c4a2e", 972), "#664428"),
    "gear_ore_grey": (_ore("#6a6c74", "#b8bcc8", 973), "#6a6c74"),
    "gear_ore_dk": (_ore("#4a4c54", "#9094a0", 974), "#4a4c54"),
    "gear_ore_rust": (_ore("#9c5e38", "#e8a860", 975), "#9c5e38"),
    "gear_ore_fleck": (_ore("#c88246", "#fff0c0", 976), "#c88246"),
    "gear_moss": (_moss("#46583a", "#7c9460", 977), "#70824e"),
    "gear_moss_dk": (_moss("#2c3a26", "#4c6240", 978), "#485638"),
    "gear_moss_lt": (_moss("#667a4c", "#a4b682", 979), "#96a064"),
    "gear_moss_ash": (_moss("#868672", "#bcbca6", 980), "#989888"),
}
PAINTERS.update({name: factory for name, (factory, _) in GEAR_WAIST_MATERIALS.items()})
FLAT_COLORS.update({name: flat for name, (_, flat) in GEAR_WAIST_MATERIALS.items()})


# ---- 末土第二批装备（护目镜琉璃 / 骨灯烈焰）补充 ----------------------------------------------------


def _glass(base_a: str, base_b: str, seed: int, crack_amount: float = 0.08) -> Callable[[dict], Painter]:
    """浑浊碎琉璃：青灰绿矿石底，中心有透亮的高对比水光高光，间杂细小暗气泡点与裂纹。"""
    dark, light = hex_color(base_a), hex_color(base_b)
    sheen, crack = shade(light, 0.45), shade(dark, -0.45)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            n = value_noise(p, 2.2, seed)
            color = mix(dark, light, n * 1.2 - 0.1)
            roll = texel_noise(p, seed + 1)
            if roll < crack_amount or abs(value_noise(p, 3.2, seed + 2) - 0.5) < 0.04:
                color = mix(color, crack, 0.75)
            elif roll > 0.93:
                color = mix(color, sheen, 0.8)
            if _edge_distance(cube, p, axis) < 0.4:
                color = shade(color, -0.15)
            return shade(color, (texel_noise(p, seed + 3) - 0.5) * 0.08)

        return paint

    return factory


def _flame(base: str, core: str, seed: int) -> Callable[[dict], Painter]:
    """跃动火舌：面心最亮、边缘渐变烈焰色，高对比度耀眼火光。"""
    c_base, c_core = hex_color(base), hex_color(core)

    def factory(cube: dict) -> Painter:
        def paint(p: Vec, axis: int) -> Color:
            along, across = _grain_coords(cube, p, axis)
            depth = min(_edge_distance(cube, p, axis), 1.8) / 1.8
            color = mix(c_base, c_core, depth * 0.9)
            flicker = value_noise((along * 2.0, across * 3.0, 0.0), 1.2, seed)
            color = shade(color, (flicker - 0.5) * 0.22)
            if texel_noise(p, seed + 1) > 0.92:
                color = mix(color, hex_color("#ffffff"), 0.6)
            return color

        return paint

    return factory


GEAR_BATCH2_MATERIALS: Dict[str, Tuple[Callable[[dict], Painter], str]] = {
    "gear_glass": (_glass("#6e9c8e", "#b2d8cc", 981, crack_amount=0.06), "#9cb6ac"),
    "gear_glass_dk": (_glass("#365048", "#5a786e", 982, crack_amount=0.25), "#48645b"),
    "gear_glow_core": (_glint("#ffe888", "#ffffff", 985), "#fff2aa"),
    "gear_glow_flame": (_flame("#d84e14", "#ffb428", 986), "#f5af3c"),
}
PAINTERS.update({name: factory for name, (factory, _) in GEAR_BATCH2_MATERIALS.items()})
FLAT_COLORS.update({name: flat for name, (_, flat) in GEAR_BATCH2_MATERIALS.items()})
