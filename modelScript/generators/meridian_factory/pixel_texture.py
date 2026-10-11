#!/usr/bin/env python3
"""经脉工厂程序化像素贴图工具（贴图工序样板）。

做什么：给一组「每面一种材质」的立方体，生成一张图集贴图 + 每个面的 UV。
为什么不用 common_parts 那种「每种材质一块 16×16 平色」：平色贴图上每个面都是同一小块噪声，
读不出血管、斑驳、透光渐变；参考图（refs/b01_meridian_straight.png）的观感全在贴图里。

怎么做：
- 每个面按 1 模型 px = 1 贴图 texel（即 16px/格）分配图集里的一块矩形；
- 每个 texel 取其**在模型空间里的 3D 位置**交给材质画笔上色。因为画笔只看 3D 位置，
  所以同一材质被切成很多小立方体时，纹理依然连续（血管不会在切缝处断开）；
- 材质画笔各一套：血肉 / 暗血肉 / 骨 / 筋管。

发光效果的做法（用户 10-11 要求）：**不做发光层 / 自发光**，也不依赖半透明。
筋管靠贴图自身的亮暗对比表现「透光」：中心近 #f6dcc4，边缘压到 #a86858，筋丝用更深的线拉开反差。
贴图 alpha 一律不透明；将来要加半透明是在不改贴图的前提下另加材质层。
"""

from __future__ import annotations

import base64
import io
import json
import math
import uuid
from pathlib import Path
from typing import Callable, Dict, List, Tuple

import numpy as np
from PIL import Image

from bbmodel_maker.render.render_bbmodel import FACES

Color = Tuple[int, int, int]
Vec = Tuple[float, float, float]


def hex_color(code: str) -> Color:
    code = code.lstrip("#")
    return int(code[0:2], 16), int(code[2:4], 16), int(code[4:6], 16)


def mix(a: Color, b: Color, t: float) -> Color:
    t = max(0.0, min(1.0, t))
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


# 配色表颜色 + 样板新增的两档（用户 10-11 指定的边缘暗色 #a86858；筋丝深线 #7a4036）
FLESH = hex_color("#8a2a2a")
FLESH_DARK = hex_color("#5a1a1a")
FLESH_LIT = hex_color("#b05050")
BONE = hex_color("#d8ccb0")
BONE_SHADE = hex_color("#b8a888")
BONE_PORE = hex_color("#6a5a48")
TUBE_EDGE = hex_color("#a86858")
TUBE_MID = hex_color("#d9a08c")
TUBE_CENTER = hex_color("#f6dcc4")
TUBE_FIBER_DEEP = hex_color("#7a4036")


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
    return _hash3(math.floor(p[0] * 2), math.floor(p[1] * 2), math.floor(p[2] * 2), seed)


# =============================================================================
# 材质画笔：paint(point, face_axis) -> RGB。face_axis = 法线所在轴 0/1/2
# =============================================================================

def paint_flesh(p: Vec, axis: int) -> Color:
    """血肉：暗红底 + 不规则深浅斑 + 细血管线。"""
    blotch = value_noise(p, 1.6, 11) * 0.55 + value_noise(p, 0.8, 12) * 0.45
    color = FLESH
    if blotch > 0.58:
        color = mix(FLESH, FLESH_LIT, (blotch - 0.58) * 5.0)
    elif blotch < 0.42:
        color = mix(FLESH, FLESH_DARK, (0.42 - blotch) * 5.0)
    # 细血管：噪声的等值线附近 1 texel 宽
    ridge = abs(value_noise(p, 2.4, 21) - 0.5)
    if ridge < 0.035:
        color = mix(color, FLESH_DARK, 0.75)
    elif ridge < 0.06:
        color = mix(color, FLESH_LIT, 0.3)
    grain = texel_noise(p, 31)
    return mix(color, FLESH_DARK if grain < 0.5 else FLESH_LIT, 0.08 + 0.08 * abs(grain - 0.5))


def paint_flesh_dark(p: Vec, axis: int) -> Color:
    """暗血肉：深底，只有一点点斑。"""
    n = value_noise(p, 2.0, 41)
    return mix(FLESH_DARK, FLESH, 0.35 * n + 0.1 * texel_noise(p, 42))


def paint_bone(p: Vec, axis: int) -> Color:
    """骨：米白底 + 多孔暗点 + 缓慢的明暗。"""
    n = value_noise(p, 2.5, 51)
    color = mix(BONE_SHADE, BONE, 0.55 + 0.6 * n)
    pore = texel_noise(p, 52)
    if pore < 0.14:
        color = mix(color, BONE_PORE, 0.85)
    elif pore < 0.22:
        color = mix(color, BONE_SHADE, 0.6)
    return color


# 筋管几何（b01 统一截面）：x ±4、y 2..8（中心 5）、z ±8
TUBE_HALF_W = 4.0
TUBE_CENTER_Y = 5.0
TUBE_HALF_H = 3.0
TUBE_HALF_L = 8.0
FIBER_PERIOD = 8.0
FIBER_SLOPE = 0.75
FIBER_HALF_WIDTH = 0.45


def _tube_ramp(b: float) -> Color:
    """亮度 0..1 → 边缘 #a86858 / 中段 #d9a08c / 中心 #f6dcc4。"""
    if b < 0.5:
        return mix(TUBE_EDGE, TUBE_MID, b / 0.5)
    return mix(TUBE_MID, TUBE_CENTER, (b - 0.5) / 0.5)


def paint_tube(p: Vec, axis: int) -> Color:
    """筋管：靠亮暗对比表现透光。
    - 中心亮（≈#f6dcc4）、边缘压暗到 #a86858；管长方向中点最亮；
    - 侧面画两条交叉对角筋丝，用 #7a4036 深线拉开反差。
    """
    x, y, z = p
    if axis == 0:
        across = abs(y - TUBE_CENTER_Y) / TUBE_HALF_H
    elif axis == 1:
        across = abs(x) / TUBE_HALF_W
    else:
        across = max(abs(x) / TUBE_HALF_W, abs(y - TUBE_CENTER_Y) / TUBE_HALF_H)
    along = abs(z) / TUBE_HALF_L if axis != 2 else 1.0
    brightness = 1.0 - min(1.0, 0.55 * across ** 1.2 + 0.45 * along ** 1.6)
    brightness += (value_noise(p, 2.0, 61) - 0.5) * 0.10 + (texel_noise(p, 62) - 0.5) * 0.05
    color = _tube_ramp(max(0.0, min(1.0, brightness)))

    if axis == 0:
        # 对角筋丝只画在侧面（参考图顶面只有渐变）：沿管长每 8px 一个 X 形
        v = y - TUBE_CENTER_Y
        pz = ((z + FIBER_PERIOD / 2) % FIBER_PERIOD) - FIBER_PERIOD / 2
        on_line = (
            abs(v - FIBER_SLOPE * pz) < FIBER_HALF_WIDTH or abs(v + FIBER_SLOPE * pz) < FIBER_HALF_WIDTH
        )
        if on_line:
            color = mix(color, TUBE_FIBER_DEEP, 0.7)
    return color


PAINTERS: Dict[str, Callable[[Vec, int], Color]] = {
    "flesh_main": paint_flesh,
    "flesh_lit": paint_flesh,
    "flesh_dark": paint_flesh_dark,
    "bone_main": paint_bone,
    "bone_dark": paint_bone,
    "tendon_tube": paint_tube,
    "tendon_highlight": paint_tube,
    "qi_glow": paint_tube,
    "tendon_fiber": paint_tube,
}

FACE_NAMES = ("north", "south", "east", "west", "up", "down")


# =============================================================================
# 图集：每面一块矩形，逐 texel 取 3D 位置上色
# =============================================================================

def _face_geometry(cube: dict, face: str):
    """返回 (TL, TR, BL 三个角点, 宽 texel 数, 高 texel 数, 法线轴)。"""
    corner_fn, normal = FACES[face]
    f = np.array(cube["from"], float)
    t = np.array(cube["to"], float)
    tl, tr, br, bl = [np.array(c, float) for c in corner_fn(f, t)]
    w = max(1, int(round(np.linalg.norm(tr - tl))))
    h = max(1, int(round(np.linalg.norm(bl - tl))))
    axis = int(np.argmax(np.abs(normal)))
    return tl, tr, bl, w, h, axis


def build_atlas(cubes: List[dict], width: int = 256) -> Tuple[Image.Image, List[Dict[str, List[float]]]]:
    """返回 (图集图片, 与 cubes 等长的 [面名 → uv] 列表)。货架式排布，高度按需。"""
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
        painter = PAINTERS.get(cubes[ci].get("material", "flesh_main"), paint_flesh)
        for j in range(h):
            for i in range(w):
                s, t = (i + 0.5) / w, (j + 0.5) / h
                point = tl + (tr - tl) * s + (bl - tl) * t
                atlas[oy + j, ox + i, :3] = painter(tuple(point), axis)
                atlas[oy + j, ox + i, 3] = 255
        uvs[ci][face] = [float(ox), float(oy), float(ox + w), float(oy + h)]
    return Image.fromarray(atlas, "RGBA"), uvs


def write_textured_bbmodel(cubes: List[dict], out_path: Path, name: str) -> Image.Image:
    """把 cubes 导出成每面独立 UV 的 bbmodel（贴图内嵌），返回图集图片。"""
    atlas, uvs = build_atlas(cubes)
    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    source = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for cube, face_uvs in zip(cubes, uvs):
        elements.append({
            "name": cube["name"],
            "box_uv": False,
            "from": [round(float(v), 4) for v in cube["from"]],
            "to": [round(float(v), 4) for v in cube["to"]],
            "faces": {face: {"uv": face_uvs[face], "texture": 0} for face in FACE_NAMES},
            "uuid": str(uuid.uuid4()),
        })
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
