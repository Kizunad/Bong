#!/usr/bin/env python3
"""经脉工厂程序化像素贴图工具（标准贴图步骤）。

做什么：给一组立方体（每个立方体一种材质），生成一张图集贴图 + 每个面的 UV。
为什么不用「每种材质一块 16×16 平色」：平色贴图上每个面都是同一小块噪声，
读不出血管、斑驳、透光渐变；参考图（refs/*.png）的观感全在贴图里。

标准入口（所有生成器共用同一个）：
    write_textured_bbmodel(cubes, out_path, name, palette=None) -> 图集图片
由 retexture.py 按模型编号调用；几何不动，只改贴图和 UV。

怎么做：
- 每个面按 1 模型 px = 2 贴图 texel（即 32px/格）分配图集里的一块矩形；
- 每个 texel 取其**在立方体局部空间里的 3D 位置**交给材质画笔上色。因为画笔只看 3D 位置，
  所以同一材质被切成很多小立方体时，纹理依然连续（血管不会在切缝处断开）；
- 材质按「部件族」取画笔，不是每件各写一份：
    肉族   flesh_main / flesh_lit / flesh_dark / flesh_glow
    骨族   bone_main / bone_dark / bone_lit / bone_crevice
    筋膜族 tendon_tube / tendon_highlight / tendon_fascia / tendon_trans / tendon_fiber / qi_glow
    气团族 shell / core（颜色来自调用方给的 palette）
  其余未登记的材质（amber_fluid、spleen_main 等）按调用方 palette 给的底色做统一的「染色噪声」。

发光效果的做法（用户 10-11 要求）：**不做发光层 / 自发光**，也不依赖半透明。
筋管靠贴图自身的亮暗对比表现「透光」：中心近 #f6dcc4，边缘压到 #a86858，筋丝用更深的细线。
贴图 alpha 一律不透明；将来要加半透明是在不改贴图的前提下另加材质层。
"""

from __future__ import annotations

import base64
import io
import json
import math
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, List, Optional, Tuple

import numpy as np
from PIL import Image

from bbmodel_maker.render.render_bbmodel import FACES

# 贴图密度：每 1 模型 px 取 2 texel（32px/格），细筋丝和细血管才画得出来
TEXELS_PER_PX = 2

Color = Tuple[int, int, int]
Vec = Tuple[float, float, float]
Painter = Callable[[Vec, int], Color]


def hex_color(code: str) -> Color:
    code = code.lstrip("#")
    return int(code[0:2], 16), int(code[2:4], 16), int(code[4:6], 16)


def mix(a: Color, b: Color, t: float) -> Color:
    t = max(0.0, min(1.0, t))
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


def shade(color: Color, factor: float) -> Color:
    """按比例变亮（factor>0）或变暗（factor<0）。"""
    if factor >= 0:
        return mix(color, (255, 255, 255), factor)
    return mix(color, (0, 0, 0), -factor)


# 配色表颜色 + 样板新增的两档（用户 10-11 指定的边缘暗色 #a86858）
FLESH = hex_color("#8a2a2a")
FLESH_DARK = hex_color("#5a1a1a")
FLESH_LIT = hex_color("#b05050")
BONE = hex_color("#d8ccb0")
BONE_SHADE = hex_color("#b8a888")
BONE_LIGHT = hex_color("#e8dcc4")
BONE_DARK_EDGE = hex_color("#a09070")
TUBE_HIGHLIGHT = hex_color("#e8bca8")
TUBE_MID = hex_color("#d9a08c")
TUBE_EDGE = hex_color("#a86858")        # 膜面边缘和底色（中等偏暗，亮芯才有对比）
TUBE_CENTER = hex_color("#fff0dc")      # 膜面中心（#f6dcc4 在对比度要求下再提一档）
FIBER_NET = hex_color("#c07868")
FIBER_DARK = hex_color("#8a4a40")        # X 筋络（压暗）


# =============================================================================
# 确定性噪声（只依赖输入坐标，保证同一位置在任何面上取值一致）
# =============================================================================

def _hash3(ix: int, iy: int, iz: int, seed: int) -> float:
    h = (ix * 374761393 + iy * 668265263 + iz * 2147483647 + seed * 1274126177) & 0xFFFFFFFF
    h = (h ^ (h >> 13)) * 1274126177 & 0xFFFFFFFF
    h ^= h >> 16
    return (h & 0xFFFF) / 65535.0


def value_noise(p: Vec, scale: float, seed: int) -> float:
    """三线性插值值噪声，输出 0..1。scale = 每个噪声格的模型 px 数。"""
    x, y, z = p[0] / scale, p[1] / scale, p[2] / scale
    x0, y0, z0 = math.floor(x), math.floor(y), math.floor(z)
    fx, fy, fz = x - x0, y - y0, z - z0
    fx, fy, fz = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy), fz * fz * (3 - 2 * fz)
    out = 0.0
    for dx in (0, 1):
        for dy in (0, 1):
            for dz in (0, 1):
                w = (fx if dx else 1 - fx) * (fy if dy else 1 - fy) * (fz if dz else 1 - fz)
                out += w * _hash3(x0 + dx, y0 + dy, z0 + dz, seed)
    return out


def texel_noise(p: Vec, seed: int) -> float:
    """每个 texel 一个独立随机数（用位置取整，所以同一 texel 恒定）。"""
    return _hash3(math.floor(p[0] * TEXELS_PER_PX), math.floor(p[1] * TEXELS_PER_PX),
                  math.floor(p[2] * TEXELS_PER_PX), seed)


# =============================================================================
# 肉族 / 骨族画笔：paint(point, face_axis) -> RGB。face_axis = 法线所在轴 0/1/2
# =============================================================================

def paint_flesh(p: Vec, axis: int) -> Color:
    """血肉：暗红底 + 不规则深浅斑 + texel 级深浅格 + 1 texel 宽的细血管线。"""
    blotch = value_noise(p, 1.4, 11) * 0.5 + value_noise(p, 0.6, 12) * 0.5
    color = FLESH
    if blotch > 0.56:
        color = mix(FLESH, FLESH_LIT, (blotch - 0.56) * 5.0)
    elif blotch < 0.44:
        color = mix(FLESH, FLESH_DARK, (0.44 - blotch) * 5.0)
    # 细血管：两层噪声等值线，1 texel 宽的深线 + 旁边一圈亮边
    for scale, seed in ((2.0, 21), (3.1, 22)):
        ridge = abs(value_noise(p, scale, seed) - 0.5)
        if ridge < 0.022:
            color = mix(color, FLESH_DARK, 0.85)
        elif ridge < 0.04:
            color = mix(color, FLESH_LIT, 0.35)
    grain = texel_noise(p, 31)
    return mix(color, FLESH_DARK if grain < 0.5 else FLESH_LIT, 0.10 + 0.18 * abs(grain - 0.5))


def paint_flesh_dark(p: Vec, axis: int) -> Color:
    """暗血肉：深底，只有一点点斑。"""
    n = value_noise(p, 2.0, 41)
    return mix(FLESH_DARK, FLESH, 0.35 * n + 0.1 * texel_noise(p, 42))


def paint_flesh_glow(p: Vec, axis: int) -> Color:
    """亮点肉（地砖 1×1 亮点）：血肉底再整体提亮一档。"""
    return mix(paint_flesh(p, axis), FLESH_LIT, 0.55)


BONE_EDGE_BAND = 0.5        # 骨面边缘压暗的宽度（px，约 1 texel）


def make_bone_painter(cube: dict, base: Color, edge: Color, light: Color) -> Painter:
    """骨：不画斑点（小斑点读成霉点）。
    - 面心是底色 base，只有很缓的明暗；
    - 面的四周一圈（1 texel）压到 edge；
    - 受光面（朝上的面，以及侧面最上面 1 texel）加 light 高光；朝下的面略压暗。
    """
    lo, hi = np.array(cube["from"], float), np.array(cube["to"], float)

    def paint(p: Vec, axis: int) -> Color:
        in_plane = [i for i in range(3) if i != axis]
        edge_dist = min(min(p[i] - lo[i], hi[i] - p[i]) for i in in_plane)
        # 很窄的立方体（如 0.05px 的暗纹片）整面都算边，没意义，边带最多占最小边长的 1/4
        band = min(BONE_EDGE_BAND, 0.25 * min(hi[i] - lo[i] for i in in_plane))
        color = shade(base, (value_noise(p, 3.0, 51) - 0.5) * 0.08)
        facing_up = axis == 1 and abs(p[1] - hi[1]) < 1e-6
        facing_down = axis == 1 and abs(p[1] - lo[1]) < 1e-6
        if facing_up:
            color = mix(color, light, 0.75)
        elif facing_down:
            color = shade(color, -0.12)
        elif axis != 1 and p[1] > hi[1] - BONE_EDGE_BAND:
            color = mix(color, light, 0.6)
        if edge_dist < band:
            color = mix(color, edge, 0.85)
        return color

    return paint


def make_tinted_painter(base: Color) -> Painter:
    """统一的「染色噪声」：给定底色，叠一层缓变明暗 + texel 颗粒。用于没有专属画笔的材质。"""

    def paint(p: Vec, axis: int) -> Color:
        slow = value_noise(p, 2.0, 71) - 0.5
        grain = texel_noise(p, 72) - 0.5
        return shade(base, slow * 0.36 + grain * 0.14)

    return paint


# =============================================================================
# 筋膜族：筋管的亮暗对比（只靠贴图，不做发光层）
# =============================================================================

SPOT_ALONG_RADIUS = 0.8     # 亮斑沿管长的半径（占半长的比例）
SPOT_EDGE_RADIUS = 0.95     # 径向距离到这里亮度降到 0（#a86858）；亮度过半（>#d9a08c）的区域约占膜面 1/3
NET_PERIOD = 4.0            # 细筋网的周期（px），粗筋丝周期 FIBER_PERIOD 的一半
FIBER_PERIOD = 8.0
FIBER_SLOPE = 0.75
FIBER_HALF_WIDTH = 0.28     # 约 1 texel 宽
FIBER_GLOW_WIDTH = 0.5      # 筋丝两侧高光带宽度（约 1 texel）
FIBER_REF_HALF_H = 3.0      # 斜率标定用的标准半高（标准截面 8×6 的半高）
END_FACE_BRIGHTNESS = 0.12
ARC_BAND_HALF_WIDTH = 0.6   # 弧管亮带半宽（占半截面的比例）


@dataclass(frozen=True)
class TubeFrame:
    """一段筋管（或一个筋囊）在立方体局部空间里的包络：中心、半尺寸、长轴。"""

    center: Vec
    half: Vec
    axis: int           # 长轴 0/1/2
    continuous: bool    # 多段旋转拼成的弧管：亮斑不沿长度衰减，免得每段一个亮点
    shift: Vec = (0.0, 0.0, 0.0)    # 亮斑中心相对包络中心的偏移


TUBE_FRAME_MATERIALS = ("tendon_tube", "tube_dark")
TUBE_PAINT_MATERIALS = ("tendon_tube", "tendon_highlight", "tendon_fascia", "tendon_trans", "tube_dark")
TUBE_DARK_SHADE = -0.22     # tube_dark（污染段暗一档的管壁）= 筋膜画笔再压暗这么多
FRAME_TOUCH_TOL = 0.11      # 高光片等贴在管壁表面上的小立方体，离管壁这么近就算同一段管


def _cube_key(cube: dict) -> tuple:
    return (cube.get("group", ""), tuple(cube.get("rotation", (0, 0, 0))), tuple(cube.get("origin", (0, 0, 0))))


def _box(cube: dict) -> Tuple[np.ndarray, np.ndarray]:
    return np.array(cube["from"], float), np.array(cube["to"], float)


def _share_a_face(a: Tuple[np.ndarray, np.ndarray], b: Tuple[np.ndarray, np.ndarray]) -> bool:
    """两个立方体面接触（接触面有面积；只碰棱或角不算）。"""
    overlap = np.minimum(a[1], b[1]) - np.maximum(a[0], b[0])    # 各轴重叠长度，负 = 分离，0 = 刚好相接
    touching = int(np.sum(np.abs(overlap) < 1e-6))
    overlapping = int(np.sum(overlap > 1e-6))
    return touching == 1 and overlapping == 2 or (overlapping == 3)


def _make_frame(lo: np.ndarray, hi: np.ndarray, continuous: bool) -> TubeFrame:
    size = hi - lo
    half = np.maximum(size / 2, 1e-6)
    # 弧管沿 z 走，外弧侧在局部 -x：亮带往外弧偏半个半宽
    shift = (-0.5 * half[0], 0.0, 0.0) if continuous else (0.0, 0.0, 0.0)
    return TubeFrame(
        center=tuple((lo + hi) / 2),
        half=tuple(half),
        axis=2 if continuous else int(np.argmax(size)),   # 弧管每段短而宽，包络最长边不是管轴；管轴恒为局部 z
        continuous=continuous,
        shift=shift,
    )


class TubeFrames:
    """一个模型里所有筋管 / 筋囊的坐标系。

    - 同组、同旋转的筋管立方体，按「面接触」连成一段管（三通的三条臂各是一段管，不会并成一个大包络）；
    - 同一组里有多个旋转（弧管由多段旋转直管拼成）时，每个旋转算一段，且标记为连续弧管。
    """

    def __init__(self, cubes: List[dict]):
        members = [c for c in cubes if c.get("material") in TUBE_FRAME_MATERIALS]
        keys_per_group: Dict[str, set] = {}
        for cube in members:
            key = _cube_key(cube)
            keys_per_group.setdefault(key[0], set()).add(key)

        by_key: Dict[tuple, List[dict]] = {}
        for cube in members:
            by_key.setdefault(_cube_key(cube), []).append(cube)

        self.segments: List[Tuple[tuple, np.ndarray, np.ndarray, TubeFrame]] = []
        for key, key_cubes in by_key.items():
            continuous = len(keys_per_group[key[0]]) > 1
            groups = [key_cubes] if continuous else _face_clusters(key_cubes)
            for group in groups:
                boxes = [_box(c) for c in group]
                lo, hi = np.min([b[0] for b in boxes], axis=0), np.max([b[1] for b in boxes], axis=0)
                self.segments.append((key, lo, hi, _make_frame(lo, hi, continuous)))

    def for_cube(self, cube: dict) -> Optional[TubeFrame]:
        """立方体所属（或紧贴）的那段管的坐标系；找不到返回 None。"""
        key = _cube_key(cube)
        lo, hi = _box(cube)
        for seg_key, seg_lo, seg_hi, frame in self.segments:
            if seg_key == key and np.all(lo <= seg_hi + FRAME_TOUCH_TOL) and np.all(hi >= seg_lo - FRAME_TOUCH_TOL):
                return frame
        return None


def _face_clusters(cubes: List[dict]) -> List[List[dict]]:
    """把立方体按「面接触」分成连通块。"""
    boxes = [_box(c) for c in cubes]
    parent = list(range(len(cubes)))

    def find(i: int) -> int:
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    for i in range(len(cubes)):
        for j in range(i + 1, len(cubes)):
            if _share_a_face(boxes[i], boxes[j]):
                parent[find(i)] = find(j)

    clusters: Dict[int, List[dict]] = {}
    for i, cube in enumerate(cubes):
        clusters.setdefault(find(i), []).append(cube)
    return list(clusters.values())


def _tube_ramp(b: float) -> Color:
    """亮度 0..1 → 边缘 / 底色 #a86858，经 #d9a08c，到中心 #fff0dc。"""
    if b < 0.5:
        return mix(TUBE_EDGE, TUBE_MID, b / 0.5)
    return mix(TUBE_MID, TUBE_CENTER, (b - 0.5) / 0.5)


def _fiber_distance(v: float, z: float, period: float) -> float:
    """到最近一条 X 形斜线的距离（沿 v 方向量），斜线每 period px 重复一次。"""
    pz = ((z + period / 2) % period) - period / 2
    slope = FIBER_SLOPE * (FIBER_PERIOD / period)
    return min(abs(v - slope * pz), abs(v + slope * pz))


def _noisy_ramp(p: Vec, brightness: float) -> Color:
    brightness += (value_noise(p, 1.5, 61) - 0.5) * 0.12 + (texel_noise(p, 62) - 0.5) * 0.06
    return _tube_ramp(max(0.0, min(1.0, brightness)))


def make_tube_painter(frame: TubeFrame) -> Painter:
    """筋膜：中点一团亮斑（中心 #fff0dc）向两端、向边缘径向渐变到 #c88a78；
    竖直侧面（法线水平、非长轴）画 X 形筋络：每 8px 一组 #c07868 的 X，两侧各描一道 #e8bca8 高光；
    每 4px 再叠一组更细、更浅的 #c07868 X（不带高光）。
    弧管（continuous）的亮带整体偏向外弧一侧（局部 -x，gen_meridian_bend 的约定），各段首尾接成一条。
    """
    long_axis = frame.axis
    shift = frame.shift

    def paint(p: Vec, axis: int) -> Color:
        if axis == long_axis:
            return _noisy_ramp(p, END_FACE_BRIGHTNESS)
        cross = 3 - long_axis - axis    # 剩下的那根截面轴
        across = abs(p[cross] - frame.center[cross] - shift[cross]) / frame.half[cross]
        along = 0.0 if frame.continuous else abs(p[long_axis] - frame.center[long_axis]) / frame.half[long_axis]
        radius = math.hypot(along / SPOT_ALONG_RADIUS, across)
        if frame.continuous:
            radius /= ARC_BAND_HALF_WIDTH   # 弧管：收成一条窄亮带，才看得出连续
        color = _noisy_ramp(p, 1.0 - (radius / SPOT_EDGE_RADIUS) ** 1.5)

        if axis != 1:   # 竖直侧面：画筋丝网
            v = (p[cross] - frame.center[cross]) / frame.half[cross] * FIBER_REF_HALF_H
            z = p[long_axis] - frame.center[long_axis]
            strong = _fiber_distance(v, z, FIBER_PERIOD)
            if strong < FIBER_HALF_WIDTH:
                color = mix(color, FIBER_DARK, 0.95)
            elif strong < FIBER_HALF_WIDTH + FIBER_GLOW_WIDTH:
                color = mix(color, TUBE_HIGHLIGHT, 0.85)
            elif _fiber_distance(v, z, NET_PERIOD) < FIBER_HALF_WIDTH * 0.8:
                color = mix(color, FIBER_DARK, 0.45)
        return color

    return paint


def make_spot_painter(cube: dict) -> Painter:
    """独立的亮斑（qi_glow 光斑、没有筋管主体可参照的亮线）：以立方体自己为包络，中心亮、边缘一档暗。"""
    lo, hi = np.array(cube["from"], float), np.array(cube["to"], float)
    center, half = (lo + hi) / 2, np.maximum((hi - lo) / 2, 1e-6)

    def paint(p: Vec, axis: int) -> Color:
        others = [i for i in range(3) if i != axis]
        radius = max(abs(p[i] - center[i]) / half[i] for i in others)
        return _noisy_ramp(p, 1.0 - 0.45 * radius ** 1.5)

    return paint


def paint_fiber(p: Vec, axis: int) -> Color:
    """浮在表面上的筋丝几何：深色细线，带一点明暗。"""
    return mix(FIBER_NET, FIBER_DARK, 0.3 * value_noise(p, 1.2, 81))


# =============================================================================
# 气团族：颜色全来自调用方 palette（五行变体各一套）
# =============================================================================

def make_qi_shell_painter(base: Color) -> Painter:
    """气团外壳：云絮状明暗 + 细格晶纹。"""

    def paint(p: Vec, axis: int) -> Color:
        cloud = value_noise(p, 1.6, 91) * 0.6 + value_noise(p, 0.7, 92) * 0.4
        color = shade(base, (cloud - 0.5) * 0.5)
        if texel_noise(p, 93) < 0.08:
            color = shade(color, 0.18)
        return color

    return paint


def make_qi_core_painter(base: Color, cube: dict) -> Painter:
    """气团面心发光片：中心最亮，边缘回到底色（只靠明暗，不是自发光）。"""
    lo, hi = np.array(cube["from"], float), np.array(cube["to"], float)
    center, half = (lo + hi) / 2, np.maximum((hi - lo) / 2, 1e-6)

    def paint(p: Vec, axis: int) -> Color:
        others = [i for i in range(3) if i != axis]
        radius = max(abs(p[i] - center[i]) / half[i] for i in others)
        # 面心比外层（shell）至少亮一档：边缘 +0.12，中心 +0.40
        return shade(base, 0.12 + 0.28 * (1.0 - radius) + (texel_noise(p, 94) - 0.5) * 0.06)

    return paint


# =============================================================================
# 材质 → 画笔
# =============================================================================

FLESH_PAINTERS: Dict[str, Painter] = {
    "flesh_main": paint_flesh,
    "flesh_lit": paint_flesh,
    "flesh_dark": paint_flesh_dark,
    "flesh_glow": paint_flesh_glow,
}
# 骨族：材质 → (底色, 边缘色, 高光色)
BONE_SCHEMES: Dict[str, Tuple[Color, Color, Color]] = {
    "bone_main": (BONE, BONE_SHADE, BONE_LIGHT),
    "bone_dark": (BONE_SHADE, BONE_DARK_EDGE, BONE),
    "bone_lit": (BONE_LIGHT, BONE, hex_color("#f2e8d2")),
}

# 某个立方体想用专属画笔时的钩子：参数是 (立方体, 它的默认画笔)，返回 None 表示走默认画笔；
# 返回的画笔可以在默认画笔之上再叠图案
Override = Callable[[dict, Painter], Optional[Painter]]

FACE_NAMES = ("north", "south", "east", "west", "up", "down")
DEFAULT_MATERIAL = "flesh_main"


class PainterSet:
    """一个模型的画笔集合：先从全部立方体里算出筋管坐标系，再按立方体的材质取画笔。"""

    def __init__(self, cubes: List[dict], palette: Optional[Dict[str, Color]] = None,
                 override: Optional[Override] = None):
        self.palette = palette or {}
        self.frames = TubeFrames(cubes)
        self.override = override

    def for_cube(self, cube: dict) -> Painter:
        default = self._default(cube)
        if self.override:
            return self.override(cube, default) or default
        return default

    def _default(self, cube: dict) -> Painter:
        material = cube.get("material", DEFAULT_MATERIAL)
        if material in FLESH_PAINTERS:
            return FLESH_PAINTERS[material]
        if material in BONE_SCHEMES:
            return make_bone_painter(cube, *BONE_SCHEMES[material])
        if material in TUBE_PAINT_MATERIALS:
            frame = self.frames.for_cube(cube)
            painter = make_tube_painter(frame) if frame else make_spot_painter(cube)
            if material == "tube_dark":
                return lambda p, axis: shade(painter(p, axis), TUBE_DARK_SHADE)
            return painter
        if material == "qi_glow":
            return make_spot_painter(cube)
        if material == "tendon_fiber":
            return paint_fiber
        if material == "shell":
            return make_qi_shell_painter(self._color(material))
        if material == "core":
            return make_qi_core_painter(self._color(material), cube)
        return make_tinted_painter(self._color(material))

    def _color(self, material: str) -> Color:
        return self.palette.get(material, FLESH)


def palette_to_colors(palette: Dict[str, tuple]) -> Dict[str, Color]:
    """生成器里的 PALETTE 是 RGBA 元组，这里取 RGB。"""
    return {name: tuple(rgba[:3]) for name, rgba in palette.items()}


# =============================================================================
# 图集：每面一块矩形，逐 texel 取 3D 位置上色
# =============================================================================

def _face_geometry(cube: dict, face: str):
    """返回 (TL, TR, BL 三个角点, 宽 texel 数, 高 texel 数, 法线轴)。"""
    corner_fn, normal = FACES[face]
    f = np.array(cube["from"], float)
    t = np.array(cube["to"], float)
    tl, tr, br, bl = [np.array(c, float) for c in corner_fn(f, t)]
    w = max(1, int(round(np.linalg.norm(tr - tl) * TEXELS_PER_PX)))
    h = max(1, int(round(np.linalg.norm(bl - tl) * TEXELS_PER_PX)))
    axis = int(np.argmax(np.abs(normal)))
    return tl, tr, bl, w, h, axis


def build_atlas(
    cubes: List[dict],
    palette: Optional[Dict[str, Color]] = None,
    override: Optional[Override] = None,
    width: int = 512,
) -> Tuple[Image.Image, List[Dict[str, List[float]]]]:
    """返回 (图集图片, 与 cubes 等长的 [面名 → uv] 列表)。货架式排布，高度按需。"""
    painters = PainterSet(cubes, palette, override)
    cube_painters = [painters.for_cube(cube) for cube in cubes]

    jobs = []
    for ci, cube in enumerate(cubes):
        for face in FACE_NAMES:
            tl, tr, bl, w, h, axis = _face_geometry(cube, face)
            jobs.append((ci, face, tl, tr, bl, w, h, axis))

    # 按高度降序放货架
    order = sorted(range(len(jobs)), key=lambda i: -jobs[i][6])
    placements: Dict[int, Tuple[int, int]] = {}
    x = y = shelf_h = 0
    for idx in order:
        w, h = jobs[idx][5], jobs[idx][6]
        if x + w > width:
            x, y, shelf_h = 0, y + shelf_h, 0
        placements[idx] = (x, y)
        x += w
        shelf_h = max(shelf_h, h)
    height = y + shelf_h
    height = 1 << max(4, math.ceil(math.log2(max(height, 1))))

    atlas = np.zeros((height, width, 4), dtype=np.uint8)
    atlas[:, :] = (255, 0, 255, 255)  # 没用到的区域刷洋红，一眼能看出 UV 越界
    uvs: List[Dict[str, List[float]]] = [dict() for _ in cubes]
    for idx, (ci, face, tl, tr, bl, w, h, axis) in enumerate(jobs):
        ox, oy = placements[idx]
        painter = cube_painters[ci]
        for j in range(h):
            for i in range(w):
                s, t = (i + 0.5) / w, (j + 0.5) / h
                point = tl + (tr - tl) * s + (bl - tl) * t
                atlas[oy + j, ox + i, :3] = painter(tuple(point), axis)
                atlas[oy + j, ox + i, 3] = 255
        uvs[ci][face] = [float(ox), float(oy), float(ox + w), float(oy + h)]
    return Image.fromarray(atlas, "RGBA"), uvs


def write_textured_bbmodel(
    cubes: List[dict],
    out_path: Path,
    name: str,
    palette: Optional[Dict[str, Color]] = None,
    override: Optional[Override] = None,
) -> Image.Image:
    """标准贴图入口：把 cubes 导出成每面独立 UV 的 bbmodel（贴图内嵌），返回图集图片。
    立方体的 origin / rotation 原样带过去（几何不动）。
    override：个别立方体的专属画笔（比如端口碗底的亮筋），见 piece_overrides.py。"""
    atlas, uvs = build_atlas(cubes, palette, override)
    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    source = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for cube, face_uvs in zip(cubes, uvs):
        element = {
            "name": cube["name"],
            "box_uv": False,
            "from": [round(float(v), 4) for v in cube["from"]],
            "to": [round(float(v), 4) for v in cube["to"]],
            "faces": {face: {"uv": face_uvs[face], "texture": 0} for face in FACE_NAMES},
            "uuid": str(uuid.uuid4()),
        }
        if "origin" in cube:
            element["origin"] = list(cube["origin"])
        if "rotation" in cube:
            element["rotation"] = list(cube["rotation"])
        elements.append(element)

    groups: Dict[str, List[str]] = {}
    for cube, element in zip(cubes, elements):
        groups.setdefault(cube.get("group", name), []).append(element["uuid"])

    doc = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": atlas.width, "height": atlas.height},
        "elements": elements,
        "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in groups.items()],
        "textures": [{"name": name, "folder": "block", "namespace": "bong", "id": 0, "source": source}],
    }
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=2), encoding="utf-8")
    return atlas
