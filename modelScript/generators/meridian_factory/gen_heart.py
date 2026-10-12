#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o05: heart (心，火)

依据：
- model-review/meridian_factory.md「统一约定」、10-09 修订配色表、「第 2 批」o05 行、「第 2 批调整」
- 爆炸图 model-review/img/meridian_factory/exploded/x05_heart.png，单件图 refs/o05_heart.png
- 接口件（8×6 输出口）一律来自 common_parts.outlet_8x6，这里不重写。

外包 2 宽 × 3 高 × 2 深（32×48×32 px），原点在底面中心，正面朝 +Z，输出口开在底面（-Y）。

逐行宽度表（每行 2px 高，自下而上；半宽 = X 向 ±，半深 = Z 向 ±，单位 px）：

  心室（y 2..30，共 14 行）
    行   y        半宽   半深
     0   2..4      6      5      ← 心尖，罩住底面 8×6 输出口管套（±5 × ±4）
     1   4..6      8      6
     2   6..8     10      8
     3   8..10   11.5    9.5
     4  10..12    13     11
     5  12..14    14     12
     6  14..16    15     13
     7  16..18   15.5   13.5
     8  18..20   15.5    14     ← 行 8~11 最宽（31 × 28）
     9  20..22   15.5    14
    10  22..24   15.5    14
    11  24..26   15.5    14
    12  26..28    15     13.5
    13  28..30   14.5     13

  心房圆鼓（y 30..，左右各一个，中间留 6px 心凹给主动脉；内侧固定 |x| = 3）
    左鼓 5 行  外缘 14.5 / 14 / 13 / 12.5 / 12，半深 12 / 11.5 / 10.5 / 9.5 / 8，顶 y=40
    右鼓 4 行  外缘 14.5 / 14 / 13 / 12，      半深 12 / 11.5 / 10.5 / 9，  顶 y=38（比左鼓矮 2px，左右略不对称）

  每一行沿 X 切成若干段（中央 |x| ≤ 3 一段不起伏，两侧每段约 4.5px），每段前后各自 ±1px 起伏，
  材质按段在 #8a2a2a / #b05050 间交替，读出「斑驳的肌肉块」而不是一摞平板。

顶部 3 根 6×6 大血管（中间 y 30..48，左 y 40..47，右 y 38..45），各带骨环口。
胸前 6×6 #c84a3a 余烬光（火），表面 4 条 #d8ccb0 冠状血管纹，底尖 8×6 输出口。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

from PIL import Image, ImageDraw

from common_parts import (
    MAT_UV as COMMON_MAT_UV,
    PALETTE as COMMON_PALETTE,
    _assert_no_coplanar_faces,
    build_texture as build_common_texture,
    outlet_8x6,
)

REPO = Path(__file__).resolve().parents[3]
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "heart.bbmodel"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")
REVIEW_DIR = REVIEW_ROOT / "heart"
EXPLODED_REF = REVIEW_ROOT / "exploded" / "x05_heart.png"

# 配色：沿用 common_parts 的统一色板，只补火属性真元色（配色表「真元载荷·火」#c84a3a）
PALETTE = dict(COMMON_PALETTE)
PALETTE["fire"] = (200, 74, 58, 255)  # #c84a3a 火
MAT_UV = dict(COMMON_MAT_UV)
MAT_UV["fire"] = [16, 32, 32, 48]

# 渲染角度：本批生成器沿用第 1 批（肺）的约定，yaw=0 看到的是 +Z 正面（已用血管纹/余烬光实渲验证）
YAW_FRONT, YAW_SIDE, YAW_34, PITCH_34 = 0.0, 90.0, -35.0, 22.0

# ─────────────────────────────────────────────────────────────────────────────
# 逐行宽度表
# ─────────────────────────────────────────────────────────────────────────────

VENTRICLE_Y0 = 2.0
ROW_H = 2.0
# (半宽, 半深)，自下而上共 14 行
VENTRICLE_ROWS: List[Tuple[float, float]] = [
    (6.0, 5.0),
    (8.0, 6.0),
    (10.0, 8.0),
    (11.5, 9.5),
    (13.0, 11.0),
    (14.0, 12.0),
    (15.0, 13.0),
    (15.5, 13.5),
    (15.5, 14.0),
    (15.5, 14.0),
    (15.5, 14.0),
    (15.5, 14.0),
    (15.0, 13.5),
    (14.5, 13.0),
]

ATRIUM_Y0 = VENTRICLE_Y0 + ROW_H * len(VENTRICLE_ROWS)  # 30
ATRIUM_INNER_X = 3.0  # 两鼓内侧，给中间主动脉留 6px
# (外缘 |x|, 半深)，自下而上
ATRIUM_LEFT_ROWS: List[Tuple[float, float]] = [(14.5, 12.0), (14.0, 11.5), (13.0, 10.5), (12.5, 9.5), (12.0, 8.0)]
ATRIUM_RIGHT_ROWS: List[Tuple[float, float]] = [(14.5, 12.0), (14.0, 11.5), (13.0, 10.5), (12.0, 9.0)]

CENTRAL_HALF = 3.0  # 每行中央 |x| ≤ 3 一段不起伏，余烬光和血管纹贴在上面
SEGMENT_TARGET_W = 4.5
# 段起伏表：值是前 / 后表面各自的 Z 增减（px）
BUMP = [0, 1, 0, -1, 1, 0, -1, 0]

# 顶部大血管 (x 中心, 底 y, 顶 y)，6×6 截面
VESSELS = {
    "center": (0.0, ATRIUM_Y0, 48.0),
    "left": (-9.0, 40.0, 47.0),
    "right": (9.0, 38.0, 45.0),
}
VESSEL_HALF = 3.0
RING_H = 1.5  # 骨环口高
RING_T = 1.25  # 骨环壁厚

# 血管树（几何）：一条中央主脉 + 左右对称各 2 级支脉。
# 主脉 2×2：从顶部中间那根大血管的前面（z=3）往下，翻过心体顶面（y=30），再沿心正面中线（|x|≤1）下到余烬斑上方。
# 支脉 1×1：以 1px 格 (ix, iy) 记，沿心肌表面斜向下逐格走（4 连通阶梯），左侧写出、右侧镜像。
# 每格贴在所在位置的心肌前表面上，两格表面高度不同（过台阶）时，在较低那格上补一块 0.5px 的半格，
# 把台阶拆成两级半格，血管不断。
# 格的 y 整体错开 0.5（心肌行界都在偶数），支脉格的 x 也错开 0.5（避开中央段 |x|=3 等整数界），
# 免得侧面和心肌段共面。
MAIN_X = (-1.0, 1.0)
MAIN_PROUD = 2.0
MAIN_TOP_Y = 46.0            # 顶端留在大血管骨环口（y 46.5）之下
MAIN_BRIDGE_Y = (30.0, 32.0)  # 翻过心体顶面的一段
MAIN_FRONT_Y = (22, 30)       # 沿心正面往下的格（iy 22..29，y 22.5..30.5）
BRANCH_PROUD = 1.0
Y_OFFSET = 0.5
X_OFFSET_BRANCH = 0.5
BRANCH_Y_OFFSET = 0.25      # 支脉的 y 与主脉（0.5）错开，起点格和主脉相交处才不共面
# 左侧支脉路点 (ix, iy)：一级支从主脉左缘（余烬斑上方）斜向下，二级支再从一级支分出
BRANCH_L1 = [(-2, 26), (-6, 23), (-9, 19), (-12, 13)]
BRANCH_L2 = [
    [(-6, 23), (-12, 22), (-14, 18)],   # 向外
    [(-9, 19), (-7, 12), (-5, 8)],      # 向内下
]
# 贴身埋进心肌的深度：要大于相邻段 ±1 起伏之差，否则低段处会露缝；各组错开 0.1，免得背面共面。
SINK = 2.5
SINK_STEP = 0.1

EMBER_X = (-3.0, 3.0)
EMBER_Y = (16.0, 22.0)  # 6×6
EMBER_PROUD = 1.0  # 余烬块凸出体表 1px（原来 0.5 像一块平板）

# 输出口：+Z 向标准套管，开口底边离地 2
OUTLET_CENTER_Y = 5.05
OUTLET_Z0 = 3.0
OUTLET_LEN = 8.0


def _cube(name: str, lo, hi, group: str, material: str) -> dict:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi], "group": group, "material": material}


# ─────────────────────────────────────────────────────────────────────────────
# 表面段表：身体和血管纹、余烬光共用，保证贴合
# ─────────────────────────────────────────────────────────────────────────────

def _side_edges(inner: float, outer: float) -> List[float]:
    """把 [inner, outer] 等分成宽约 SEGMENT_TARGET_W 的若干段，返回分界坐标。"""
    n = max(1, round((outer - inner) / SEGMENT_TARGET_W))
    step = (outer - inner) / n
    return [inner + step * i for i in range(n)] + [outer]


def _bump(row: int, seg: int, side: int, face: int) -> float:
    """第 row 行第 seg 段（side=0 左 / 1 右，face=0 前 / 1 后）的 Z 起伏。"""
    return float(BUMP[(row * 3 + seg * 5 + side * 2 + face * 3) % len(BUMP)])


def _material(row: int, seg: int, side: int) -> str:
    return "flesh_lit" if (row * 7 + seg * 3 + side) % 5 == 0 else "flesh_main"


def _build_surface() -> List[dict]:
    """心室 + 心房全部表面段，每段 dict(x0,x1,y0,y1,zf,zb,mat,part,tag)。

    心室每行：中央 |x|≤3 一段不起伏（血管纹和余烬光贴在上面），两侧各 n 段起伏；
    心尖最下两行不起伏，免得把输出口罩不住。心房每行：内侧固定在 |x|=3，向外分段。
    """
    segs: List[dict] = []

    for r, (half_w, half_d) in enumerate(VENTRICLE_ROWS):
        y0 = VENTRICLE_Y0 + r * ROW_H
        calm = r < 2
        segs.append({"x0": -CENTRAL_HALF, "x1": CENTRAL_HALF, "y0": y0, "y1": y0 + ROW_H, "zf": half_d, "zb": half_d,
                     "mat": "flesh_main", "part": "ventricle", "tag": f"v{r}_c"})
        edges = _side_edges(CENTRAL_HALF, half_w)
        for k in range(len(edges) - 1):
            for side in (0, 1):
                a, b = edges[k], edges[k + 1]
                x0, x1 = (-b, -a) if side == 0 else (a, b)
                bf = 0.0 if calm else _bump(r, k, side, 0)
                bb = 0.0 if calm else _bump(r, k, side, 1)
                segs.append({"x0": x0, "x1": x1, "y0": y0, "y1": y0 + ROW_H, "zf": half_d + bf, "zb": half_d + bb,
                             "mat": _material(r, k, side), "part": "ventricle", "tag": f"v{r}_{'lr'[side]}{k}"})

    for side, rows in ((0, ATRIUM_LEFT_ROWS), (1, ATRIUM_RIGHT_ROWS)):
        for r, (outer, half_d) in enumerate(rows):
            y0 = ATRIUM_Y0 + r * ROW_H
            edges = _side_edges(ATRIUM_INNER_X, outer)
            for k in range(len(edges) - 1):
                a, b = edges[k], edges[k + 1]
                x0, x1 = (-b, -a) if side == 0 else (a, b)
                segs.append({"x0": x0, "x1": x1, "y0": y0, "y1": y0 + ROW_H,
                             "zf": half_d + _bump(r + 20, k, side, 0), "zb": half_d + _bump(r + 20, k, side, 1),
                             "mat": _material(r + 20, k, side), "part": "atrium", "tag": f"a{'lr'[side]}{r}_{k}"})
    return segs


SURFACE = _build_surface()


def _front_z(x0: float, x1: float, y0: float, y1: float) -> float:
    """区域 [x0,x1]×[y0,y1] 内身体（含心房、中间大血管）最靠前的 Z。"""
    candidates = [
        s["zf"] for s in SURFACE
        if s["x1"] > x0 and s["x0"] < x1 and s["y1"] > y0 and s["y0"] < y1
    ]
    _, center_bottom, _ = VESSELS["center"]
    if x0 >= -VESSEL_HALF and x1 <= VESSEL_HALF and y0 >= center_bottom:
        candidates.append(VESSEL_HALF)
    if not candidates:
        raise ValueError(f"区域 x[{x0},{x1}] y[{y0},{y1}] 没有落在心体表面上")
    return max(candidates)


# ─────────────────────────────────────────────────────────────────────────────
# 部件
# ─────────────────────────────────────────────────────────────────────────────

def part_01_ventricles() -> List[dict]:
    """心室：14 行逐行宽度表，上宽下尖，每行按段起伏。"""
    return [
        _cube(s["tag"], (s["x0"], s["y0"], -s["zb"]), (s["x1"], s["y1"], s["zf"]), "ventricles", s["mat"])
        for s in SURFACE if s["part"] == "ventricle"
    ]


def part_02_atria() -> List[dict]:
    """心房：左右两个圆鼓，中间留 6px 心凹，右鼓比左鼓矮 2px。"""
    return [
        _cube(s["tag"], (s["x0"], s["y0"], -s["zb"]), (s["x1"], s["y1"], s["zf"]), "atria", s["mat"])
        for s in SURFACE if s["part"] == "atrium"
    ]


def _vessel(name: str, cx: float, y_bot: float, y_top: float) -> List[dict]:
    """一根 6×6 大血管：三列肉柱（中列亮肉）+ 顶端骨环口 + 口内火色。"""
    g = "vessels"
    h = VESSEL_HALF
    body_top = y_top - RING_H
    cubes = [
        _cube(f"{name}_col_a", (cx - h, y_bot, -h), (cx - h / 3, body_top, h), g, "flesh_main"),
        _cube(f"{name}_col_b", (cx - h / 3, y_bot, -h), (cx + h / 3, body_top, h), g, "flesh_lit"),
        _cube(f"{name}_col_c", (cx + h / 3, y_bot, -h), (cx + h, body_top, h), g, "flesh_main"),
    ]
    inner = h - RING_T
    cubes += [
        _cube(f"{name}_ring_n", (cx - h, body_top, -h), (cx + h, y_top, -inner), g, "bone_main"),
        _cube(f"{name}_ring_s", (cx - h, body_top, inner), (cx + h, y_top, h), g, "bone_main"),
        _cube(f"{name}_ring_w", (cx - h, body_top, -inner), (cx - inner, y_top, inner), g, "bone_main"),
        _cube(f"{name}_ring_e", (cx + inner, body_top, -inner), (cx + h, y_top, inner), g, "bone_main"),
        _cube(f"{name}_mouth", (cx - inner, body_top, -inner), (cx + inner, body_top + 0.4, inner), g, "fire"),
    ]
    return cubes


def part_03_vessels() -> List[dict]:
    """顶部 3 根 6×6 大血管。

    骨环口是 6×6 的粗环，common_parts.bone_ring_port 的 3×3 小环套不上这个截面，所以环口在这里单做。
    """
    cubes: List[dict] = []
    for name, (cx, yb, yt) in VESSELS.items():
        cubes += _vessel(name, cx, yb, yt)
    return cubes


def _stair(c0: int, r0: int, c1: int, r1: int) -> set:
    """两格之间的 4 连通阶梯：沿大方向每步走一格，小方向按斜率进位，斜走那步补一个肘格。"""
    dc, dr = c1 - c0, r1 - r0
    steps = max(abs(dc), abs(dr))
    out = {(c0, r0)}
    prev = (c0, r0)
    for i in range(1, steps + 1):
        cur = (c0 + round(dc * i / steps), r0 + round(dr * i / steps))
        if cur[0] != prev[0] and cur[1] != prev[1]:
            out.add((cur[0], prev[1]))
        out.add(cur)
        prev = cur
    return out


def _path_cells(waypoints: List[Tuple[int, int]]) -> set:
    cells: set = set()
    for (c0, r0), (c1, r1) in zip(waypoints, waypoints[1:]):
        cells |= _stair(c0, r0, c1, r1)
    return cells


def _mirror_cells(cells: set) -> set:
    """左侧格镜像到右侧：格 (ix, iy) 占 x [ix+0.5, ix+1.5]，镜像后占 [-ix-1.5, -ix-0.5]，即格号 -ix-2。"""
    return {(-ix - 2, iy) for ix, iy in cells}


def _relax(tops: dict, steps: tuple) -> dict:
    """让相邻格前表面高差不超过 0.5px：较低的格往上垫半格（0.5 的整数倍），一直垫到和高处差 ≤0.5。
    垫高会影响它自己的邻格，所以反复迭代到稳定（等价于沿路径逐级爬坡）。"""
    final = dict(tops)
    changed = True
    while changed:
        changed = False
        for (ix, iy), h in list(final.items()):
            for dx, dy in steps:
                nb = (ix + dx, iy + dy)
                if nb in final and final[nb] - h > 0.5 + 1e-9:
                    # 抬到比邻格低 0.5 以内，取半格整数倍
                    need = final[nb] - 0.5
                    final[(ix, iy)] = h = max(h, tops[(ix, iy)] + 0.5 * math.ceil((need - tops[(ix, iy)]) / 0.5 - 1e-9))
                    changed = True
    return final


def _vein_cells_group(levels: List[Tuple[str, set, float]], proud: float) -> List[dict]:
    """一侧的全部 1×1 格（一级、二级一起算）：每格贴在所在位置心肌前表面上；
    表面更高的相邻格之间（不分级），在较低那格上垫半格，高差过大时垫到 ≤0.5。levels = [(名字, 格集合, 埋深)]。"""
    g = "vessel_tree"
    boxes = {}
    tops = {}
    owner = {}
    for name, cells, sink in levels:
        for ix, iy in sorted(cells):
            x0, x1 = ix + X_OFFSET_BRANCH, ix + X_OFFSET_BRANCH + 1.0
            y0, y1 = iy + BRANCH_Y_OFFSET, iy + BRANCH_Y_OFFSET + 1.0
            try:
                tops[(ix, iy)] = _front_z(x0, x1, y0, y1) + proud
            except ValueError:
                continue   # 落在心体外的格不画
            boxes[(ix, iy)] = (x0, y0, x1, y1)
            owner[(ix, iy)] = (name, sink)
    cubes = []
    for cell, top in tops.items():
        x0, y0, x1, y1 = boxes[cell]
        name, sink = owner[cell]
        cubes.append(_cube(f"vein_{name}_{cell[0]}_{cell[1]}", (x0, y0, top - sink), (x1, y1, top), g, "tendon_highlight"))
    final = _relax(tops, ((1, 0), (-1, 0), (0, 1), (0, -1)))
    for cell, top in tops.items():
        if final[cell] - top > 1e-6:
            x0, y0, x1, y1 = boxes[cell]
            cubes.append(_cube(f"vein_{owner[cell][0]}_{cell[0]}_{cell[1]}_half", (x0, y0, top), (x1, y1, final[cell]), g, "tendon_highlight"))
    return cubes


def part_04_vessel_tree() -> List[dict]:
    """血管树：中央主脉（2×2）+ 左右对称各 2 级支脉（1×1），全部沿心肌表面连续走。"""
    g = "vessel_tree"
    x0, x1 = MAIN_X
    cubes: List[dict] = []

    # 主脉 A：顶部中间大血管前面（z 3..5）往下
    front_a = VESSEL_HALF
    cubes.append(_cube("vein_main_a", (x0, MAIN_BRIDGE_Y[1], front_a), (x1, MAIN_TOP_Y, front_a + MAIN_PROUD), g, "tendon_highlight"))
    # 主脉 B：翻过心体顶面（y 30..32），向前一直到心正面
    front_c = _front_z(x0, x1, 28.0, 30.0)
    cubes.append(_cube("vein_main_b", (x0 - 0.05, MAIN_BRIDGE_Y[0], front_a), (x1 + 0.05, MAIN_BRIDGE_Y[1], front_c + MAIN_PROUD + 0.5), g, "tendon_highlight"))
    # 主脉 C：沿心正面中线往下，按格走（2 格宽），表面台阶处补半格
    main_cells = set()
    for iy in range(MAIN_FRONT_Y[0], MAIN_FRONT_Y[1]):
        main_cells |= {(-1, iy), (0, iy)}
    tops = {}
    for ix, iy in main_cells:
        tops[(ix, iy)] = _front_z(ix, ix + 1.0, iy + Y_OFFSET, iy + Y_OFFSET + 1.0) + MAIN_PROUD
    for (ix, iy), top in tops.items():
        cubes.append(_cube(f"vein_main_c_{ix}_{iy}", (ix, iy + Y_OFFSET, top - SINK - 0.5), (ix + 1.0, iy + Y_OFFSET + 1.0, top), g, "tendon_highlight"))
    final = _relax(tops, ((0, 1), (0, -1)))
    for (ix, iy), top in tops.items():
        if final[(ix, iy)] - top > 1e-6:
            cubes.append(_cube(f"vein_main_c_{ix}_{iy}_half", (ix, iy + Y_OFFSET, top), (ix + 1.0, iy + Y_OFFSET + 1.0, final[(ix, iy)]), g, "tendon_highlight"))

    # 支脉：一级 + 二级，左侧写出，右侧镜像
    level1 = _path_cells(BRANCH_L1)
    level2 = set()
    for wp in BRANCH_L2:
        level2 |= _path_cells(wp)
    level2 -= level1
    for tag, flip in (("l", lambda c: c), ("r", _mirror_cells)):
        cubes += _vein_cells_group([(f"b1{tag}", flip(level1), SINK), (f"b2{tag}", flip(level2), SINK + SINK_STEP)], BRANCH_PROUD)
    return cubes


def part_05_ember() -> List[dict]:
    """胸前 6×6 #c84a3a 余烬光，中心 2×2 #f6dcc4 内光。"""
    zf = _front_z(EMBER_X[0], EMBER_X[1], EMBER_Y[0], EMBER_Y[1])
    zt = zf + EMBER_PROUD
    # 余烬光整块贴在中央段表面上（不埋进体内：它的四边正好落在行格线上，埋进去会与体表共面）
    return [
        _cube("ember_plate", (EMBER_X[0], EMBER_Y[0], zf), (EMBER_X[1], EMBER_Y[1], zt), "ember", "fire"),
        _cube("ember_core", (-1.0, 18.0, zt), (1.0, 20.0, zt + 0.3), "ember", "qi_glow"),
    ]


def part_06_outlet() -> List[dict]:
    """正面下方 8×6 统一截面输出口：标准 outlet_8x6 套管朝 +Z，开口 y 2..8（底边离地 2），
    套管从心尖前壁里伸出，不再是朝下只有 2px 高的扁套。

    center_y 取 5.05 而不是 5.0：套管上下沿 y=8 会和心室行格线（偶数）在 -Y 面共面，错开 0.05。
    """
    return outlet_8x6(center_x=0.0, center_y=OUTLET_CENTER_Y, center_z=OUTLET_Z0, facing="+z", length=OUTLET_LEN, group="outlet", pfx="outlet")


PARTS = [
    ("ventricles", part_01_ventricles),
    ("atria", part_02_atria),
    ("vessels", part_03_vessels),
    ("vessel_tree", part_04_vessel_tree),
    ("ember", part_05_ember),
    ("outlet", part_06_outlet),
]


def all_cubes() -> List[dict]:
    cubes: List[dict] = []
    for _, fn in PARTS:
        cubes += fn()
    return cubes


# ─────────────────────────────────────────────────────────────────────────────
# 贴图 / bbmodel
# ─────────────────────────────────────────────────────────────────────────────

def build_texture() -> Image.Image:
    """common_parts 的统一贴图 + 火色块。"""
    tex = build_common_texture()
    u0, v0, u1, v1 = MAT_UV["fire"]
    tex.paste(Image.new("RGBA", (u1 - u0, v1 - v0), PALETTE["fire"]), (u0, v0))
    return tex


def build_bbmodel_doc(cubes: List[dict]) -> dict:
    buf = io.BytesIO()
    build_texture().save(buf, format="PNG")
    source = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    groups: Dict[str, List[str]] = {}
    for c in cubes:
        uid = str(uuid.uuid4())
        uv = [float(v) for v in MAT_UV[c["material"]]]
        elements.append({
            "name": c["name"],
            "box_uv": False,
            "from": [round(v, 4) for v in c["from"]],
            "to": [round(v, 4) for v in c["to"]],
            "faces": {side: {"uv": uv, "texture": 0} for side in ("north", "south", "east", "west", "up", "down")},
            "uuid": uid,
        })
        groups.setdefault(c["group"], []).append(uid)

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "heart",
        "model_identifier": "heart",
        "visible_box": [2, 3, 2],
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in groups.items()],
        "textures": [{"name": "heart", "folder": "block", "namespace": "bong", "id": 0, "source": source}],
    }


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes: List[dict] | None = None) -> Path:
    cubes = all_cubes() if cubes is None else cubes
    _assert_no_coplanar_faces(cubes)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(build_bbmodel_doc(cubes), indent=2, ensure_ascii=False), encoding="utf-8")
    return out_path


# ─────────────────────────────────────────────────────────────────────────────
# 审阅图
# ─────────────────────────────────────────────────────────────────────────────

BG = (119, 119, 119)
CANVAS_BG = (35, 36, 40)


def _label(canvas: Image.Image, text: str, xy: Tuple[int, int], width: int) -> None:
    draw = ImageDraw.Draw(canvas)
    draw.rectangle([xy[0], xy[1], xy[0] + width, xy[1] + 22], fill=(24, 25, 28))
    draw.text((xy[0] + 6, xy[1] + 5), text, fill=(235, 235, 235))


def _fit(im: Image.Image, h: int) -> Image.Image:
    return im.resize((int(im.width * h / im.height), h), Image.Resampling.LANCZOS)


def render_views(bbmodel: Path = BBMODEL_OUT) -> None:
    """render.png：正视 + 3/4 + 侧视。check.png：左爆炸图，右部件拆开渲染 + 拼好渲染。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    size = 560
    front, _ = render(bbmodel, yaw=YAW_FRONT, pitch=0.0, size=size, bg=BG)
    side, _ = render(bbmodel, yaw=YAW_SIDE, pitch=0.0, size=size, bg=BG)
    iso, _ = render(bbmodel, yaw=YAW_34, pitch=PITCH_34, size=size, bg=BG)

    canvas = Image.new("RGB", (size * 3 + 40, size + 54), CANVAS_BG)
    for i, (title, im) in enumerate((("FRONT (+Z)", front), ("3/4", iso), ("SIDE", side))):
        x = 10 + i * (size + 10)
        canvas.paste(im, (x, 40))
        _label(canvas, title, (x, 10), 160)
    canvas.save(REVIEW_DIR / "render.png")

    # 部件拆开：每个 part 单独导一个临时 bbmodel 再渲染
    tmp = Path("/tmp/claude-1000/-home-serverkizuna-Code-Bong--agent-worktrees-model-b/e8024e15-89b9-4c94-bc09-9db5a4565345/scratchpad/heart_parts")
    tmp.mkdir(parents=True, exist_ok=True)
    tiles = []
    for name, fn in PARTS:
        p = generate_bbmodel(tmp / f"{name}.bbmodel", fn())
        im, _ = render(p, yaw=YAW_34, pitch=PITCH_34, size=340, bg=BG)
        tiles.append((name, im))

    ref = _fit(Image.open(EXPLODED_REF).convert("RGB"), 680)
    asm = _fit(iso, 680)
    grid_w, grid_h = 340 * 3, 340 * 2
    total_w = ref.width + grid_w + asm.width + 50
    check = Image.new("RGB", (total_w, 740), CANVAS_BG)
    check.paste(ref, (10, 40))
    _label(check, "EXPLODED REFERENCE (x05_heart)", (10, 10), 260)
    gx = ref.width + 20
    for i, (name, im) in enumerate(tiles):
        x, y = gx + (i % 3) * 340, 40 + (i // 3) * 340
        check.paste(im, (x, y))
        _label(check, name, (x, y), 130)
    _label(check, "PARTS (exploded render)", (gx, 10), 220)
    ax = gx + grid_w + 10
    check.paste(asm, (ax, 40))
    _label(check, "ASSEMBLED", (ax, 10), 160)
    check.save(REVIEW_DIR / "check.png")


# ─────────────────────────────────────────────────────────────────────────────
# 门禁
# ─────────────────────────────────────────────────────────────────────────────

def _bounds(cubes: List[dict]) -> Tuple[List[float], List[float]]:
    lo = [min(c["from"][i] for c in cubes) for i in range(3)]
    hi = [max(c["to"][i] for c in cubes) for i in range(3)]
    return lo, hi


def self_test() -> None:
    """共面门禁 + 外包尺寸门禁，各自先注入缺陷确认拦得住。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] {len(cubes)} 个立方体 0 项共面冲突")

    lo, hi = _bounds(cubes)
    assert lo[0] >= -16 and hi[0] <= 16 and lo[2] >= -16 and hi[2] <= 16 and lo[1] >= 0 and hi[1] <= 48, (
        f"外包超出 32×48×32: lo={lo} hi={hi}"
    )
    print(f"  [OK] 外包 x[{lo[0]},{hi[0]}] y[{lo[1]},{hi[1]}] z[{lo[2]},{hi[2]}] ⊂ 32×48×32")

    # 注入：与 ember_plate 同面同位的立方体
    defect = cubes + [dict(next(c for c in cubes if c["name"] == "ember_plate"), name="inject_dup")]
    try:
        _assert_no_coplanar_faces(defect)
    except AssertionError:
        print("  [OK] 共面门禁拦住了注入的重叠立方体")
    else:
        raise RuntimeError("门禁失效：注入的共面缺陷没被拦住")

    # 注入：超出 48 高度
    tall = cubes + [_cube("inject_tall", (0, 0, 0), (1, 49, 1), "x", "flesh_main")]
    _, hi2 = _bounds(tall)
    assert hi2[1] > 48, "外包门禁失效：注入的超高立方体没把包围盒顶出 48"
    print("  [OK] 外包门禁能识别超高缺陷")

    check_round3(cubes)
    print("  [OK] 余烬块凸出体表 1px；输出口是 +Z 向 8×6 标准套管")
    flat = [dict(c, to=[c["to"][0], c["to"][1], c["from"][2] + 0.5]) if c["name"] == "ember_plate" else c for c in cubes]
    squashed = [dict(c, to=[c["to"][0], c["from"][1] + 1.0, c["to"][2]]) if c["name"] == "outlet_lining" else c for c in cubes]
    for name, bad in (("余烬块回到平板", flat), ("输出口被压扁", squashed)):
        try:
            check_round3(bad)
        except AssertionError:
            print(f"  [OK] 注入「{name}」被拦截")
        else:
            raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    check_vessel_tree(cubes)
    print("  [OK] 血管树：主脉 2 宽、支脉 1×1、左右对称各 2 级、整棵连成一块、相邻格台阶 ≤0.5px")
    tree = [c for c in cubes if c["group"] == "vessel_tree"]
    rest = [c for c in cubes if c["group"] != "vessel_tree"]
    cut = next(c for c in tree if c["name"] == "vein_b1l_-9_19")   # 一级支和两条二级支的分叉点，拿掉就断
    broken = rest + [c for c in tree if c is not cut and not c["name"].startswith(cut["name"] + "_")]
    no_fill = rest + [c for c in tree if not c["name"].endswith("_half")]
    fat = rest + [dict(c, to=[c["to"][0] + 1.0, c["to"][1], c["to"][2]]) if c["name"].startswith("vein_b2l_") and not c["name"].endswith("_half") else c for c in tree]
    no_l2 = rest + [c for c in tree if not c["name"].startswith("vein_b2r_")]
    for name, bad in (("拿掉一格支脉", broken), ("去掉过台阶半格", no_fill), ("支脉变 2 宽", fat), ("右侧少二级支", no_l2)):
        try:
            check_vessel_tree(bad)
        except AssertionError:
            print(f"  [OK] 注入「{name}」被拦截")
        else:
            raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")


def _touch(a: dict, b: dict) -> bool:
    return all(a["from"][i] <= b["to"][i] + 1e-6 and b["from"][i] <= a["to"][i] + 1e-6 for i in range(3))


def check_vessel_tree(cubes: List[dict]) -> None:
    """血管树门禁：主脉 2 宽、支脉格 1×1；左右两侧各有一级和二级；所有血管块连成一块；
    同组相邻格（含半格）的前表面高度差 ≤0.5px，也就是过台阶不断。"""
    tree = [c for c in cubes if c["group"] == "vessel_tree"]
    assert tree, "没有血管树"
    for c in tree:
        if c["name"].startswith("vein_main_c_") and not c["name"].endswith("_half"):
            assert abs(c["to"][0] - c["from"][0] - 1.0) < 1e-6, f"{c['name']} 主脉格应 1 宽（两格并成 2 宽）"
    main_c = [c for c in tree if c["name"].startswith("vein_main_c_") and not c["name"].endswith("_half")]
    assert abs(max(c["to"][0] for c in main_c) - min(c["from"][0] for c in main_c) - 2.0) < 1e-6, "主脉应 2 宽"
    for c in tree:
        if c["name"].startswith(("vein_b1", "vein_b2")) and not c["name"].endswith("_half"):
            assert abs(c["to"][0] - c["from"][0] - 1.0) < 1e-6 and abs(c["to"][1] - c["from"][1] - 1.0) < 1e-6, f"{c['name']} 支脉应是 1×1"
    for tag in ("b1l", "b1r", "b2l", "b2r"):
        assert any(c["name"].startswith(f"vein_{tag}_") for c in tree), f"缺 {tag} 支脉"
    n_l = sum(1 for c in tree if c["name"].startswith(("vein_b1l_", "vein_b2l_")) and not c["name"].endswith("_half"))
    n_r = sum(1 for c in tree if c["name"].startswith(("vein_b1r_", "vein_b2r_")) and not c["name"].endswith("_half"))
    assert abs(n_l - n_r) <= 4, f"左右支脉格数差太多: {n_l} vs {n_r}"

    # 连成一块（按包围盒接触）
    left = list(range(len(tree)))
    seen = {0}
    stack = [0]
    while stack:
        i = stack.pop()
        for j in left:
            if j not in seen and _touch(tree[i], tree[j]):
                seen.add(j)
                stack.append(j)
    assert len(seen) == len(tree), f"血管树断成多块: 连通 {len(seen)} / {len(tree)}"

    # 相邻格前表面高度差 ≤0.5：把半格算进所在格的顶
    tops = {}
    for c in tree:
        if c["name"].startswith(("vein_b1", "vein_b2")):
            key = c["name"].removesuffix("_half")
            tops[key] = max(tops.get(key, 0.0), c["to"][2])
    cells = {}
    for c in tree:
        if c["name"].startswith(("vein_b1", "vein_b2")) and not c["name"].endswith("_half"):
            cells[(round(c["from"][0] * 2), round(c["from"][1] * 4))] = c["name"]
    for (kx, ky), name in cells.items():
        for nb in ((kx + 2, ky), (kx, ky + 4)):
            other = cells.get(nb)
            if other is not None:
                assert abs(tops[name] - tops[other]) <= 0.5 + 1e-6, f"{name} 与 {other} 前表面高差 {abs(tops[name] - tops[other])} > 0.5"


def check_round3(cubes: List[dict]) -> None:
    """第 3 轮门禁：余烬块凸出体表 ≥1px（身体表面处的 z 取自 _front_z）；输出口内衬 8 宽 × 6 高、底边离地 2。"""
    plate = next(c for c in cubes if c["name"] == "ember_plate")
    assert plate["to"][2] - plate["from"][2] >= 1.0 - 1e-6, f"余烬块应凸出 1px: {plate['to'][2] - plate['from'][2]}"
    lining = next(c for c in cubes if c["name"] == "outlet_lining")
    assert abs(lining["to"][0] - lining["from"][0] - 8.0) < 1e-6 and abs(lining["to"][1] - lining["from"][1] - 6.0) < 1e-6, "输出口内衬应为 8 宽 × 6 高"
    assert abs(lining["from"][1] - 2.0) < 0.2, f"输出口底边应离地 2: {lining['from'][1]}"


def main() -> None:
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o05 heart 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁")
    args = parser.parse_args()
    print("gen_heart 自检…")
    self_test()
    if args.self_test:
        return
    path = generate_bbmodel()
    print(f"✓ bbmodel: {path}")
    render_views(path)
    print(f"✓ 审阅图: {REVIEW_DIR}")


if __name__ == "__main__":
    main()
