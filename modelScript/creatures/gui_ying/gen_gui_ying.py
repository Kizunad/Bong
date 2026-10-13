#!/usr/bin/env python3
"""诡影（空兽 / gui_ying）建模源：无实体的真空漩涡。Round 1 只做几何，不贴图、不做动画、不接线。

正典依据（docs/worldview.md §二 L51）：负灵域的特有生态——看不见的真空漩涡「诡影（空兽）」，撞上直接扣真元，
是唯一敢待在负灵域的生物。**没有实体、几乎看不见**，所以外形不是一只兽，而是一道旋涡：

    part_ring(i)    六层内凹、互相错开旋转角度的环状扭曲层（越往深处半径越小，每层绕轴多转 17°），
    wall_cells()    旋涡的喉管壁：贴着最内层往里收的一圈深色管壁；
    void_cells()    最深处的空洞（近黑，全模型唯一最暗的一块）；
    debris_group_cells()  外圈绕旋着的碎灰（1px 灰块）和枯叶碎片（3×1×2 的薄片），沿螺旋线从开口一直拖到尖端，
                    切成 10 组，每组一根骨头。

**骨骼（任务卡「骨骼数下限」）**：按能独立动的关节分，不按部件分。无实体漩涡：每一圈环、每组碎片各一根，共 19 根会动的骨
（funnel 总枢轴 + 6 圈环 + 喉管 + 空洞 + 10 组碎片）+ 一个静止的 root；枢轴都在漩涡轴线上。动画约定（调度意见）：
**整体旋转只由 funnel 总枢轴承担**（漏斗整体绕轴转）；各圈环骨只做小幅摆动（几度），**不做单环自转**，否则环带的石块图案会互相错位打架。
层级树见 `bone_table()` / `bone_tree_lines()`，接触表上标了骨骼数并附层级树。

**轮廓只靠深色边缘和高对比的环线读出来，不做半透明特效**（门禁里有对比度判据，先注入低对比缺陷确认拦得住）。

做法：旋涡的轴向前上方张开（朝上 38°），整体是一个朝前上的漏斗。每个部件先在**局部坐标**（u, v, w；w = 开口方向）
里判定每个 1px 体素是否属于它，再按轴向 38° 的倾斜转到世界坐标，逐体素落成**不带旋转的轴对齐立方体**（和参考图一样
是体素块，不是光滑曲面），最后沿 x 向把同材质的连续体素合并成长条，减少立方体数。因此整件的 `_assert_no_coplanar_faces`
共面门禁是真有效的（没有旋转块被跳过）。

坐标：1 block = 16px，原点在地面中心（悬空 5px），+y 向上，开口朝 +z 前上方。

用法：
    python gen_gui_ying.py            # 自检 + 写 bbmodel
    python gen_gui_ying.py --sheet    # 另出 model-review/img/creatures/gui_ying/{parts.png, contact_sheet.png, check.png}
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import random
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Sequence, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "generators" / "meridian_factory"))
from common_parts import _assert_no_coplanar_faces  # noqa: E402

MODEL_ID = "gui_ying"
MODEL_DIR = REPO / "modelScript" / "models" / "gui_ying"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/gui_ying")

Cube = dict

# ── 配色：只有深色边 + 一条高对比的骨灰色环线，没有半透明 ─────────────────────────
PALETTE: Dict[str, Tuple[int, int, int]] = {
    "void": (8, 7, 12),            # 空洞：全模型最暗
    "rim": (50, 47, 54),           # 环的外缘 / 内缘：深灰（不再是近黑）
    "body": (88, 84, 90),          # 环体：深灰石块
    "body_dark": (70, 67, 74),     # 石块的暗面
    "body_lit": (112, 108, 112),   # 石块的亮面
    "line": (184, 180, 172),       # 环线：浅灰（不再是近白），和深边仍拉开亮度差
    "ash": (152, 148, 142),        # 碎石亮面
    "ash_dark": (96, 92, 90),      # 碎石暗面
    "leaf": (118, 98, 68),         # 枯叶叶片
    "leaf_dark": (74, 60, 42),     # 枯叶叶脉
}
MATERIALS = tuple(PALETTE)


def luminance(rgb: Tuple[int, int, int]) -> float:
    return 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]


# ── 形体参数（门禁直接读这张表，所以改形体改这里） ───────────────────────────────

AXIS_PITCH_DEG = 38.0                       # 开口方向朝前上方 38°
CENTER = np.array([0.0, 20.0, 8.0])         # 旋涡中心（局部原点），世界坐标；z 向前挪 8，让整团（含拖到尖端的碎屑）在 ±24 内
HOVER_Y = 3.0                               # 整体最低点悬空 4px
LAYER_COUNT = 6
LAYERS = [
    # 外半径, 环宽, 深度 w（开口 = 0，往里为负）, 相位（度，每层多转 17°）
    {"r_out": 15.5, "width": 3.4, "w": 0.0, "phase": 0.0},
    {"r_out": 13.3, "width": 3.2, "w": -3.6, "phase": 17.0},
    {"r_out": 11.2, "width": 3.0, "w": -7.2, "phase": 34.0},
    {"r_out": 9.3, "width": 2.8, "w": -10.8, "phase": 51.0},
    {"r_out": 7.7, "width": 2.6, "w": -14.4, "phase": 68.0},
    {"r_out": 6.4, "width": 2.4, "w": -18.0, "phase": 85.0},
]
LAYER_THICK = 2.4                           # 每层沿轴向的厚度
SECTORS = 12                                # 一圈 12 个扇区，约 18% 缺失 → 断弧
SPIRAL_SHEAR = 0.06                         # 弧线随半径斜一点（rad / px），读出旋转感
THROAT_R = (3.2, 4.0)                       # 喉管壁半径
THROAT_W = (-21.0, -5.0)                    # 喉管壁沿轴向的范围
VOID_R = 3.2                                # 最深处空洞半径
VOID_W = (-22.5, -20.0)
DEBRIS_COUNT = 96
DEBRIS_GROUPS = 10                          # 碎屑沿螺旋线切成 10 组，每组一根骨头
DEBRIS_R = (18.2, 21.0)                     # 碎屑绕旋半径：必须在最外层之外
DEBRIS_W = (4.0, -20.0)
CONTRAST_MIN = 120.0                        # 环线与深色边的亮度差下限
CELL = 1.0                                  # 体素边长（px）：立方体数必须 < 2000（调度意见③），所以回到 1px 体素；环宽 2.4~3.4px 靠 3 格宽 + 环线读出

U_HAT = np.array([1.0, 0.0, 0.0])
_PITCH = math.radians(AXIS_PITCH_DEG)
W_HAT = np.array([0.0, math.sin(_PITCH), math.cos(_PITCH)])
V_HAT = np.cross(W_HAT, U_HAT)

Cell = Tuple[int, int, int]
Cells = Dict[Cell, str]

GRID_X = range(int(-28 / CELL), int(28 / CELL))      # 单位：CELL
GRID_Y = range(0, int(56 / CELL))
GRID_Z = range(int(-30 / CELL), int(30 / CELL))


def _grid() -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """所有体素中心的世界坐标（px）。"""
    xs, ys, zs = np.meshgrid((np.array(GRID_X) + 0.5) * CELL, (np.array(GRID_Y) + 0.5) * CELL,
                             (np.array(GRID_Z) + 0.5) * CELL, indexing="ij")
    return xs, ys, zs


def _local(xs, ys, zs):
    """世界体素中心 → 局部 (u, v, w, r, θ)。"""
    px, py, pz = xs - CENTER[0], ys - CENTER[1], zs - CENTER[2]
    u = px * U_HAT[0] + py * U_HAT[1] + pz * U_HAT[2]
    v = px * V_HAT[0] + py * V_HAT[1] + pz * V_HAT[2]
    w = px * W_HAT[0] + py * W_HAT[1] + pz * W_HAT[2]
    return u, v, w, np.hypot(u, v), np.arctan2(v, u)


def _sector_index(theta: np.ndarray, r: np.ndarray, phase_deg: float) -> np.ndarray:
    angle = theta + SPIRAL_SHEAR * r - math.radians(phase_deg)
    return np.floor((angle % (2 * math.pi)) / (2 * math.pi / SECTORS)).astype(int)


def _sector_kept(sector: np.ndarray, layer: int) -> np.ndarray:
    """断弧掩码：按哈希留约 82%（每层缺口位置不同）。"""
    return ((sector * 37 + layer * 91 + sector * layer * 11) % 11) > 1


def _surface(mask: np.ndarray) -> np.ndarray:
    """只留表面体素（至少一个 6 邻居是空的）：内部体素从外面看不见，留着只会白烧立方体。"""
    inner = mask.copy()
    for axis in range(3):
        for step in (1, -1):
            shifted = np.roll(mask, step, axis=axis)
            edge = [slice(None)] * 3
            edge[axis] = slice(0, 1) if step == 1 else slice(-1, None)
            shifted[tuple(edge)] = False
            inner &= shifted
    return mask & ~inner


def _to_cells(mask: np.ndarray, material: np.ndarray) -> Cells:
    cells: Cells = {}
    for ix, iy, iz in zip(*np.nonzero(mask)):
        cells[(GRID_X[0] + int(ix), GRID_Y[0] + int(iy), GRID_Z[0] + int(iz))] = MATERIALS[int(material[ix, iy, iz])]
    return cells


def ring_cells(i: int) -> Cells:
    """第 i 层环：断弧 + 外缘深色边 + 外缘内侧一道骨灰色环线（只画在朝开口的那一面，1px 宽、连续）+ 内缘深色边 + 零星外刺。"""
    spec = LAYERS[i]
    xs, ys, zs = _grid()
    u, v, w, r, theta = _local(xs, ys, zs)
    r_in = spec["r_out"] - spec["width"]
    in_depth = np.abs(w - spec["w"]) <= LAYER_THICK / 2
    sector = _sector_index(theta, r, spec["phase"])
    kept = _sector_kept(sector, i)
    spike = (r > spec["r_out"]) & (r <= spec["r_out"] + 1.5) & (sector % 4 == 1) & kept & in_depth
    body = in_depth & (r >= r_in) & (r <= spec["r_out"]) & kept
    mask = _surface(body | spike)

    mat = np.full(xs.shape, MATERIALS.index("body"), dtype=int)
    # 石块纹：2px 一块的石砖，三档明暗按哈希铺开（粒度取 2px 是为了不把体素合并打碎）
    block = (np.floor(u / 2) * 7 + np.floor(v / 2) * 13 + np.floor(w / 2) * 5 + i * 3) % 7
    mat[block == 0] = MATERIALS.index("body_lit")
    mat[block >= 4] = MATERIALS.index("body_dark")
    front = w >= spec["w"] + LAYER_THICK / 2 - CELL * 1.5                   # 朝开口的那一面
    line_band = (r >= spec["r_out"] - 2.0) & (r <= spec["r_out"] - 1.0) & front
    mat[line_band] = MATERIALS.index("line")
    mat[(r > spec["r_out"] - 0.9) | (r < r_in + 0.9)] = MATERIALS.index("rim")
    mat[spike] = MATERIALS.index("rim")
    mat[line_band & (r <= spec["r_out"] - 1.0)] = MATERIALS.index("line")
    return _to_cells(mask, mat)


def wall_cells() -> Cells:
    """喉管壁：贴着最内层往里收的深色管壁（带几道略亮的纹）。"""
    xs, ys, zs = _grid()
    u, v, w, r, theta = _local(xs, ys, zs)
    wall = (r >= THROAT_R[0]) & (r <= THROAT_R[1]) & (w >= THROAT_W[0]) & (w <= THROAT_W[1])
    mat = np.full(xs.shape, MATERIALS.index("rim"), dtype=int)
    streak = wall & ((np.floor(w) + np.floor(theta * 3)) % 4 == 0)
    mat[streak] = MATERIALS.index("body")
    return _to_cells(_surface(wall), mat)


def void_cells() -> Cells:
    """最深处的空洞：全模型最暗的 void。"""
    xs, ys, zs = _grid()
    u, v, w, r, theta = _local(xs, ys, zs)
    wall = (r >= THROAT_R[0]) & (r <= THROAT_R[1]) & (w >= THROAT_W[0]) & (w <= THROAT_W[1])
    hole = (r < VOID_R + 0.3) & (w >= VOID_W[0]) & (w <= VOID_W[1]) & ~wall       # 和喉管壁不重叠（同一个体素只属于一根骨头）
    mat = np.full(xs.shape, MATERIALS.index("void"), dtype=int)
    return _to_cells(_surface(hole), mat)


def debris_group_cells(seed: int = 7) -> Dict[str, Cells]:
    """外圈碎屑：沿螺旋线从开口一直拖到尖端，碎灰（1px 灰块）+ 枯叶碎片（3×1×2 的薄片，朝向随机）。1px = 2 个 CELL。
    沿螺旋线按顺序切成 DEBRIS_GROUPS 组（每组 ≈ DEBRIS_COUNT / DEBRIS_GROUPS 片），**每组一根骨头**，所以每一段螺旋能独立转。"""
    rng = random.Random(seed)
    groups: Dict[str, Cells] = {f"debris_{g:02d}": {} for g in range(DEBRIS_GROUPS)}
    taken: set = set()                 # 已被某一组占掉的体素：同一个体素只属于一根骨头，互相重叠的碎片后到的让位
    per_px = int(round(1 / CELL))
    for k in range(DEBRIS_COUNT):
        t = (k + rng.random() * 0.8) / DEBRIS_COUNT
        w = DEBRIS_W[0] + (DEBRIS_W[1] - DEBRIS_W[0]) * t
        radius = DEBRIS_R[0] + (DEBRIS_R[1] - DEBRIS_R[0]) * rng.random()
        theta = 5.2 * math.pi * t + rng.uniform(-0.5, 0.5)
        local = np.array([radius * math.cos(theta), radius * math.sin(theta), w])
        world = CENTER + local[0] * U_HAT + local[1] * V_HAT + local[2] * W_HAT
        base = [int(math.floor(world[a])) * per_px for a in range(3)]
        shape = rubble_shape(rng) if rng.random() < 0.5 else leaf_shape(rng)
        cells = groups[f"debris_{min(DEBRIS_GROUPS - 1, k * DEBRIS_GROUPS // DEBRIS_COUNT):02d}"]
        for (dx, dy, dz), material in shape.items():
            cell = (base[0] + dx, base[1] + dy, base[2] + dz)
            if cell not in taken:
                taken.add(cell)
                cells[cell] = material
    return groups


def rubble_shape(rng: random.Random) -> Cells:
    """多面碎石：菱形截面的小石块（|x|/a + |y|/b + |z|/c ≤ 1，所以每个面都是斜切的 facet），
    朝一个方向的几个面是亮面 ash，另一侧是暗面 ash_dark，读成有棱有角的石头而不是方块。"""
    a, b, c = (rng.choice((2.0, 2.5, 3.0)) * 0.5 / CELL for _ in range(3))      # 半径 1~1.5px，随体素边长换算成格数
    reach = int(math.ceil(max(a, b, c)))
    light = np.array([rng.choice((-1, 1)), 1, rng.choice((-1, 1))])
    shape: Cells = {}
    for x in range(-reach, reach + 1):
        for y in range(-reach, reach + 1):
            for z in range(-reach, reach + 1):
                if abs(x) / a + abs(y) / b + abs(z) / c <= 1.05:
                    shape[(x, y, z)] = "ash" if (np.array([x, y, z]) * light).sum() >= 0 else "ash_dark"
    return shape


def leaf_shape(rng: random.Random) -> Cells:
    """枯叶片：一格厚的椭圆叶片（长 7~9 格、宽 4~5 格），中轴一条叶脉（leaf_dark）加两对斜侧脉；叶片所在平面随机取三个轴向之一。"""
    length, width = max(3, round(rng.choice((7, 8, 9)) * 0.5 / CELL)), max(2, round(rng.choice((4, 5)) * 0.5 / CELL))
    axes = list(range(3))
    rng.shuffle(axes)
    shape: Cells = {}
    for i in range(-(length // 2), length // 2 + 1):
        for j in range(-(width // 2), width // 2 + 1):
            if (i / (length / 2 + 0.3)) ** 2 + (j / (width / 2 + 0.3)) ** 2 > 1.0:
                continue
            vein = j == 0 or (abs(j) == 1 and abs(i) in (1, 3) and i * j > 0 if abs(i) <= 3 else False)
            cell = [0, 0, 0]
            cell[axes[0]], cell[axes[1]] = i, j
            shape[tuple(cell)] = "leaf_dark" if vein else "leaf"
    return shape


# ── 体素 → 立方体（沿 x 合并同材质连续体素） ─────────────────────────────────────


def cells_to_cubes(cells: Cells, name: str, group: str, y_shift: float = 0.0) -> List[Cube]:
    """贪心三维合并：同材质的体素先沿 x 连成条、再沿 y 叠成面、再沿 z 叠成块，一个块 = 一个立方体。"""
    remaining = dict(cells)
    cubes: List[Cube] = []
    for cell in sorted(cells, key=lambda c: (c[2], c[1], c[0])):
        material = remaining.get(cell)
        if material is None:
            continue
        x0, y0, z0 = cell
        x1 = x0
        while remaining.get((x1 + 1, y0, z0)) == material:
            x1 += 1
        y1 = y0
        while all(remaining.get((x, y1 + 1, z0)) == material for x in range(x0, x1 + 1)):
            y1 += 1
        z1 = z0
        while all(remaining.get((x, y, z1 + 1)) == material for x in range(x0, x1 + 1) for y in range(y0, y1 + 1)):
            z1 += 1
        for x in range(x0, x1 + 1):
            for y in range(y0, y1 + 1):
                for z in range(z0, z1 + 1):
                    del remaining[(x, y, z)]
        cubes.append({"name": f"{name}_{len(cubes):04d}",
                      "from": [round(x0 * CELL, 4), round(y0 * CELL + y_shift, 4), round(z0 * CELL, 4)],
                      "to": [round((x1 + 1) * CELL, 4), round((y1 + 1) * CELL + y_shift, 4), round((z1 + 1) * CELL, 4)],
                      "group": group, "material": material})
    return cubes


def _min_y(cell_sets: Sequence[Cells]) -> float:
    return min(y for cells in cell_sets for (_, y, _) in cells) * CELL


_CACHE: Dict[str, Cells] = {}


def _cells(key: str) -> Cells:
    """各部件体素缓存（一次进程内只算一遍）。"""
    if not _CACHE:
        for i in range(LAYER_COUNT):
            _CACHE[f"ring_{i}"] = ring_cells(i)
        _CACHE["throat"] = wall_cells()
        _CACHE["void"] = void_cells()
        _CACHE.update(debris_group_cells())
    return _CACHE[key]


def _part_keys() -> List[str]:
    return [f"ring_{i}" for i in range(LAYER_COUNT)] + ["throat", "void"] + [f"debris_{g:02d}" for g in range(DEBRIS_GROUPS)]


def y_shift_for_hover() -> float:
    """整体平移（px）到最低点悬空 HOVER_Y。"""
    return HOVER_Y - _min_y([_cells(k) for k in _part_keys()])


def parts(y_shift: float | None = None) -> Dict[str, List[Cube]]:
    shift = y_shift_for_hover() if y_shift is None else y_shift
    return {key: cells_to_cubes(_cells(key), key.replace("_", ""), key, shift) for key in _part_keys()}


def all_cubes() -> List[Cube]:
    cubes: List[Cube] = []
    for part in parts().values():
        cubes.extend(part)
    return cubes


# ── 骨骼层级：按能独立动的关节分，不按部件分 ─────────────────────────────────────
# 任务卡「骨骼数下限」：无实体漩涡——每一圈环、每组碎片各一骨，≥ 16。root 只是静止的根，不计入。
#
#   root                      （静止，不计）
#   └─ funnel                 整个漩涡（整体旋转的唯一总枢轴，在漩涡中心）
#      ├─ ring_0 … ring_5     六圈环，只做小幅摆动，不单环自转（枢轴在各自深度的轴心上）
#      ├─ throat              喉管壁
#      ├─ void                最深处的空洞（可以单独收缩 / 张开）
#      └─ debris_00 … 09      十段螺旋碎屑，随漏斗整体转，可各自轻微错开相位
#
# 枢轴都在轴线上（漩涡中心 + 深度 w × 轴向）。做动画时：funnel 绕轴转整圈，ring_* 只给几度的摆动，不给转速。


def bone_table() -> Dict[str, Tuple[str | None, Tuple[float, float, float]]]:
    """骨名 → (父骨, 枢轴世界坐标 px，已含悬空平移)。"""
    shift = y_shift_for_hover()
    origin = CENTER + np.array([0.0, shift, 0.0])

    def on_axis(w: float) -> Tuple[float, float, float]:
        p = origin + w * W_HAT
        return (round(float(p[0]), 3), round(float(p[1]), 3), round(float(p[2]), 3))

    table: Dict[str, Tuple[str | None, Tuple[float, float, float]]] = {
        "root": (None, (0.0, 0.0, 0.0)),
        "funnel": ("root", on_axis(0.0)),
    }
    for i, spec in enumerate(LAYERS):
        table[f"ring_{i}"] = ("funnel", on_axis(spec["w"]))
    table["throat"] = ("funnel", on_axis(sum(THROAT_W) / 2))
    table["void"] = ("funnel", on_axis(sum(VOID_W) / 2))
    for g in range(DEBRIS_GROUPS):
        table[f"debris_{g:02d}"] = ("funnel", on_axis(0.0))
    return table


def bone_tree_lines(table=None) -> List[str]:
    """骨骼层级树（文本）。"""
    table = table or bone_table()
    children: Dict[str | None, List[str]] = {}
    for name, (parent, _) in table.items():
        children.setdefault(parent, []).append(name)
    lines: List[str] = []

    def walk(name: str, prefix: str, last: bool, top: bool) -> None:
        lines.append(name if top else f"{prefix}{'`- ' if last else '+- '}{name}")
        kids = children.get(name, [])
        for idx, kid in enumerate(kids):
            walk(kid, "" if top else prefix + ("   " if last else "|  "), idx == len(kids) - 1, False)

    walk("root", "", True, True)
    return lines


def moving_bones(table=None) -> List[str]:
    """会动的骨头：除了静止的 root 以外全部。"""
    return [name for name in (table or bone_table()) if name != "root"]


# ── 门禁（每道先注入缺陷再信） ─────────────────────────────────────────────────


def check_spec(layers: List[dict]) -> None:
    """形体参数门禁：六层；越往深半径越小、深度越负（内凹）；相邻层相位差 ≥8°（互相错开旋转）。"""
    assert len(layers) == 6, f"应为 6 层，实际 {len(layers)}"
    for a, b in zip(layers, layers[1:]):
        assert b["r_out"] < a["r_out"], f"层半径没有往深处收小: {a['r_out']} → {b['r_out']}"
        assert b["w"] < a["w"], f"层没有内凹（深度不递减）: {a['w']} → {b['w']}"
        assert abs(b["phase"] - a["phase"]) >= 8.0, f"相邻层相位差 <8°（没有错开旋转）: {a['phase']} → {b['phase']}"


def check_contrast(palette: Dict[str, Tuple[int, int, int]]) -> None:
    """轮廓只靠深边和高对比环线读出来：环线与深边亮度差 ≥ CONTRAST_MIN，空洞比深边更暗。"""
    gap = luminance(palette["line"]) - luminance(palette["rim"])
    assert gap >= CONTRAST_MIN, f"环线与深色边亮度差 {gap:.0f} < {CONTRAST_MIN:.0f}"
    assert luminance(palette["void"]) < luminance(palette["rim"]), "空洞不是全模型最暗的"


def check_cubes(cubes: List[Cube]) -> None:
    """成品门禁：共面（无旋转块被跳过）、外包 48×48×48 内且悬空 ≥0、六层环都在、空洞在、碎屑在环外且数量够。"""
    assert len(cubes) < MAX_CUBES, f"立方体 {len(cubes)} 个，要 < {MAX_CUBES}"
    _assert_no_coplanar_faces(cubes)
    lo = [min(c["from"][i] for c in cubes) for i in range(3)]
    hi = [max(c["to"][i] for c in cubes) for i in range(3)]
    assert lo[0] >= -24 and hi[0] <= 24 and lo[2] >= -24 and hi[2] <= 24 and lo[1] >= 0 and hi[1] <= 48, f"外包超出 48³: {lo} {hi}"
    for i in range(LAYER_COUNT):
        assert any(c["group"] == f"ring_{i}" for c in cubes), f"缺第 {i} 层环"
    void = [c for c in cubes if c["material"] == "void"]
    assert void, "没有空洞"
    debris = [c for c in cubes if c["group"].startswith("debris_")]
    assert len(debris) >= 60, f"碎屑只有 {len(debris)} 条（要 ≥60）"
    ring_cubes = [c for c in cubes if c["group"].startswith("ring_")]

    def centre(c):
        return np.array([(c["from"][k] + c["to"][k]) / 2 for k in range(3)])

    shift = y_shift_for_hover()
    origin = CENTER + np.array([0.0, shift, 0.0])
    outer = LAYERS[0]["r_out"]
    ring_extent = max(float(np.linalg.norm(((centre(c) - origin) - np.dot(centre(c) - origin, W_HAT) * W_HAT))) for c in ring_cubes)
    inside = [c for c in debris if float(np.linalg.norm((centre(c) - origin) - np.dot(centre(c) - origin, W_HAT) * W_HAT)) < outer + 0.5]
    assert not inside, f"{len(inside)} 条碎屑落在环内（半径 < {outer + 0.5}）"
    assert ring_extent <= outer + 2.5, f"环体半径 {ring_extent:.1f} 超出 {outer}"


MIN_MOVING_BONES = 16
MAX_CUBES = 2000                            # 调度意见③：一只生物 6741 个立方体太多，压到 2000 以内


def check_bones(cubes: List[Cube], table=None, minimum: int = MIN_MOVING_BONES) -> int:
    """骨骼门禁（任务卡「骨骼数下限」）：会动的骨头 ≥16；每圈环、每组碎片各自一根骨头；父骨都存在且无环；
    每个立方体恰好属于一根骨头（组名 = 骨名），没有空骨头（不带立方体的骨头只算分组，不计数）。返回会动的骨头数。"""
    table = table or bone_table()
    for name, (parent, _) in table.items():
        assert parent is None or parent in table, f"{name} 的父骨 {parent} 不存在"
        seen, cur = {name}, parent
        while cur is not None:
            assert cur not in seen, f"骨骼层级有环: {name}"
            seen.add(cur)
            cur = table[cur][0]
    groups = {c["group"] for c in cubes}
    assert groups <= set(table), f"立方体归了不存在的骨头: {sorted(groups - set(table))}"
    for i in range(LAYER_COUNT):
        assert f"ring_{i}" in table and f"ring_{i}" in groups, f"第 {i} 圈环没有自己的骨头"
    for g in range(DEBRIS_GROUPS):
        assert f"debris_{g:02d}" in table and f"debris_{g:02d}" in groups, f"第 {g} 组碎片没有自己的骨头"
    carrying = [b for b in moving_bones(table) if b in groups]
    assert len(carrying) >= minimum, f"带几何的会动骨头只有 {len(carrying)} 根，要 ≥{minimum}"
    return len(carrying)


def self_test() -> None:
    check_spec(LAYERS)
    check_contrast(PALETTE)
    cubes = all_cubes()
    check_cubes(cubes)
    bones = check_bones(cubes)
    print(f"  [OK] {len(cubes)} 个立方体：共面 0 冲突（全部轴对齐）、六层内凹且逐层错开、空洞 {sum(1 for c in cubes if c['material'] == 'void')} 条、"
          f"碎屑 {sum(1 for c in cubes if c['group'].startswith('debris_'))} 条、对比度 {luminance(PALETTE['line']) - luminance(PALETTE['rim']):.0f}")
    print(f"  [OK] 骨骼：会动的 {bones} 根（≥{MIN_MOVING_BONES}），每圈环 / 每组碎片各一根，另有静止的 root")

    def must_fail(name: str, fn) -> None:
        try:
            fn()
        except AssertionError as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e)[:56]}")
            return
        raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    must_fail("各层相位相同（没有错开旋转）", lambda: check_spec([dict(l, phase=0.0) for l in LAYERS]))
    must_fail("各层同一深度（没有内凹）", lambda: check_spec([dict(l, w=0.0) for l in LAYERS]))
    must_fail("层半径不收小", lambda: check_spec([dict(l, r_out=15.5) for l in LAYERS]))
    must_fail("少一层", lambda: check_spec(LAYERS[:5]))
    must_fail("环线对比度过低", lambda: check_contrast(dict(PALETTE, line=(70, 66, 76))))
    must_fail("空洞比深边亮", lambda: check_contrast(dict(PALETTE, void=(90, 86, 96))))
    must_fail("没有空洞", lambda: check_cubes([c for c in cubes if c["material"] != "void"]))
    must_fail("碎屑太少", lambda: check_cubes([c for c in cubes if not c["group"].startswith("debris_")]))
    origin = CENTER + np.array([0.0, y_shift_for_hover(), 0.0])
    inner = {"name": "debris_inject", "from": [float(origin[0]), float(origin[1]), float(origin[2])],
             "to": [float(origin[0]) + 1, float(origin[1]) + 1, float(origin[2]) + 1], "group": "debris_00", "material": "ash"}
    must_fail("碎屑落进环内", lambda: check_cubes(cubes + [inner]))
    must_fail("立方体太多", lambda: check_cubes(cubes * 2))
    twin = dict(cubes[0], name="dup")
    must_fail("共面", lambda: check_cubes(cubes + [twin]))

    table = bone_table()
    must_fail("碎片合并成一根骨头", lambda: check_bones([dict(c, group="debris_00") if c["group"].startswith("debris_") else c for c in cubes]))
    must_fail("六圈环合并成一根骨头", lambda: check_bones([dict(c, group="ring_0") if c["group"].startswith("ring_") else c for c in cubes]))
    must_fail("骨骼数不足（下限抬到 40，当前只有 18 根带几何的）", lambda: check_bones(cubes, table, minimum=40))
    must_fail("父骨不存在", lambda: check_bones(cubes, dict(table, ring_0=("ghost", table["ring_0"][1]))))
    must_fail("层级有环", lambda: check_bones(cubes, dict(table, funnel=("ring_0", table["funnel"][1]))))
    must_fail("立方体归了不存在的骨", lambda: check_bones(cubes + [dict(cubes[0], name="stray", group="nowhere")]))


# ── bbmodel / 渲染 ──────────────────────────────────────────────────────────


def build_texture() -> Image.Image:
    """每个材质一块 16² 平色（带一点点噪声），Round 1 不贴图。"""
    rng = np.random.RandomState(11)
    im = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    for k, name in enumerate(MATERIALS):
        tile = np.zeros((16, 16, 4), dtype=np.uint8)
        tile[:, :] = (*PALETTE[name], 255)
        if name not in ("void", "line"):
            noise = rng.randint(-5, 6, size=(16, 16))
            for ch in range(3):
                tile[:, :, ch] = np.clip(tile[:, :, ch].astype(int) + noise, 0, 255)
        im.paste(Image.fromarray(tile, mode="RGBA"), ((k % 4) * 16, (k // 4) * 16))
    return im


def write_bbmodel(cubes: List[Cube], out_path: Path, name: str = MODEL_ID, with_bones: bool = True) -> Path:
    """写 bbmodel。with_bones=True 时 outliner 是真的骨骼层级（嵌套分组，枢轴在轴线上）；单件预览用 False（只按组平铺）。"""
    buf = io.BytesIO()
    build_texture().save(buf, format="PNG")
    source = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")
    elements, by_group = [], {}
    for c in cubes:
        k = MATERIALS.index(c["material"])
        u0, v0 = (k % 4) * 16, (k // 4) * 16
        uv = [u0, v0, u0 + 16, v0 + 16]
        elem = {"name": c["name"], "box_uv": False, "from": c["from"], "to": c["to"],
                "faces": {s: {"uv": uv, "texture": 0} for s in ("north", "south", "east", "west", "up", "down")},
                "uuid": str(uuid.uuid4())}
        elements.append(elem)
        by_group.setdefault(c["group"], []).append(elem["uuid"])
    if with_bones:
        table = bone_table()
        nodes = {n: {"name": n, "origin": list(pivot), "children": list(by_group.get(n, []))} for n, (_, pivot) in table.items()}
        outliner = []
        for n, (parent, _) in table.items():
            if parent is None:
                outliner.append(nodes[n])
            else:
                nodes[parent]["children"].append(nodes[n])
    else:
        outliner = [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in by_group.items()]
    doc = {"meta": {"format_version": "4.10", "model_format": "free"}, "name": name,
           "resolution": {"width": 64, "height": 64}, "elements": elements, "outliner": outliner,
           "textures": [{"name": name, "folder": "entity", "namespace": "bong", "id": 0, "source": source}]}
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return out_path


VIEWS = (
    ("FRONT +Z (yaw0 pitch0)", 0.0, 0.0),
    ("BACK -Z (yaw180 pitch0)", 180.0, 0.0),
    ("SIDE +X (yaw90 pitch0)", 90.0, 0.0),
    ("SIDE -X (yaw270 pitch0)", 270.0, 0.0),
    ("TOP +Y (pitch90)", 0.0, 89.9),
    ("3/4 FRONT-RIGHT (yaw35 pitch25)", 35.0, 25.0),
)


def bone_tree_panel(height: int) -> Image.Image:
    """骨骼层级树图：标题标出骨骼数，下面是缩进树（root 静止，不计数）。"""
    lines = bone_tree_lines()
    moving = len(moving_bones())
    header = [f"BONES: {moving} moving (+ static root)", f"  1 funnel + {LAYER_COUNT} rings + throat + void + {DEBRIS_GROUPS} debris", ""]
    font_h = 13
    width = 330
    panel = Image.new("RGB", (width, max(height, (len(lines) + len(header)) * font_h + 20)), (35, 36, 40))
    draw = ImageDraw.Draw(panel)
    y = 10
    for text in header:
        draw.text((10, y), text, fill=(180, 220, 210))
        y += font_h
    for text in lines:
        draw.text((10, y), text, fill=(230, 230, 230))
        y += font_h
    return panel


def render_sheets(prev_model: Path | None = None) -> None:
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = Path("/tmp/claude-1000/gui_ying_parts")
    tmp.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    whole = write_bbmodel(all_cubes(), MODEL_DIR / "GuiYing_round1.bbmodel")
    bones = len(moving_bones())

    # 逐件预览（每件单独一张 3/4）：每件 = 一根骨头
    tiles = []
    for name, cubes in parts().items():
        path = write_bbmodel(cubes, tmp / f"{name}.bbmodel", name, with_bones=False)
        im, _ = render(path, yaw=35.0, pitch=25.0, size=220, bg=bg)
        tiles.append((f"{name} ({len(cubes)})", im))
    cols = 6
    rows = (len(tiles) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * 230 + 10, rows * 250 + 30), (35, 36, 40))
    draw = ImageDraw.Draw(sheet)
    draw.text((10, 6), f"PARTS = BONES  ({bones} moving bones, each part is one bone)", fill=(180, 220, 210))
    for idx, (label, im) in enumerate(tiles):
        x, y = 10 + (idx % cols) * 230, 28 + (idx // cols) * 250
        draw.text((x + 4, y), label, fill=(230, 230, 230))
        sheet.paste(im, (x, y + 18))
    sheet.save(REVIEW_DIR / "parts.png")

    # 骨骼层级树
    tree = bone_tree_panel(420)
    tree.save(REVIEW_DIR / "bones_tree.png")

    # 六视角接触表
    shots = [(label, render(whole, yaw=yaw, pitch=pitch, size=420, bg=bg)[0]) for label, yaw, pitch in VIEWS]
    contact = Image.new("RGB", (3 * 430 + 10, 2 * 450 + 30), (35, 36, 40))
    draw = ImageDraw.Draw(contact)
    draw.text((10, 6), f"gui_ying round3 |{bones} moving bones | {len(all_cubes())} cubes", fill=(180, 220, 210))
    for idx, (label, im) in enumerate(shots):
        x, y = 10 + (idx % 3) * 430, 28 + (idx // 3) * 450
        draw.text((x + 4, y), label, fill=(230, 230, 230))
        contact.paste(im, (x, y + 18))
    contact.save(REVIEW_DIR / "contact_sheet.png")

    # check.png：左参考图 | 同取景渲染 | 骨骼树；下面六视角接触表
    ref = Image.open(REVIEW_DIR / "ref.png").convert("RGB")
    h = 560
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    same_view, _ = render(whole, yaw=-30.0, pitch=32.0, size=h, bg=bg)
    prev_view = render(prev_model, yaw=-30.0, pitch=32.0, size=h, bg=bg)[0] if prev_model else None
    tree_panel = bone_tree_panel(h)
    top_w = ref.width + (h if prev_view else 0) + h + tree_panel.width + 50
    small_w = min(top_w - 20, 1300)
    small = contact.resize((small_w, int(small_w * contact.height / contact.width)), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (top_w, 30 + h + 10 + small.height + 10), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    canvas.paste(ref, (10, 30))
    draw.text((14, 8), "REFERENCE (ref.png)", fill=(210, 200, 180))
    x = ref.width + 20
    if prev_view:
        canvas.paste(prev_view, (x, 30))
        draw.text((x + 4, 8), "PREVIOUS ROUND, same framing", fill=(210, 160, 150))
        x += h + 10
    canvas.paste(same_view, (x, 30))
    draw.text((x + 4, 8), f"NOW (round 3), same framing yaw-30 pitch32 ({len(all_cubes())} cubes)", fill=(180, 220, 210))
    x += h + 10
    canvas.paste(tree_panel, (x, 30))
    draw.text((x + 4, 8), f"BONE HIERARCHY  |  {bones} moving bones", fill=(180, 220, 210))
    canvas.paste(small, (10, 30 + h + 10))
    canvas.save(REVIEW_DIR / "check.png")
    print(f"[sheets] {REVIEW_DIR}  (bones={bones})")


# ── 像素贴图（画笔库 handheld_texture，诡影的材质见该文件 "slate" 等条目） ───────────────

TEXTURE_MATERIAL = {
    "void": "void", "rim": "slate_rim", "body": "slate", "body_dark": "slate_dark", "body_lit": "slate_lit",
    "line": "ash_band", "ash": "rubble", "ash_dark": "rubble_dark", "leaf": "dry_leaf", "leaf_dark": "dry_leaf_vein",
}


def write_textured(cubes: List[Cube], out_path: Path, flat: bool = False) -> Path:
    """每面独立 UV 的像素贴图版（flat=True 出平色对照）。write_textured_bbmodel 只会按组平铺 outliner，
    这里再把它改回真骨骼层级（枢轴在轴线上），动画才绑得上。"""
    sys.path.insert(0, str(REPO / "modelScript" / "generators"))
    from handheld_texture import flat_override, handheld_override  # noqa: E402
    from pixel_texture import write_textured_bbmodel  # noqa: E402

    mapped = [dict(c, material=TEXTURE_MATERIAL[c["material"]]) for c in cubes]
    write_textured_bbmodel(mapped, out_path, MODEL_ID, None, flat_override if flat else handheld_override)
    doc = json.loads(out_path.read_text(encoding="utf-8"))
    by_group = {g["name"]: g["children"] for g in doc["outliner"]}
    table = bone_table()
    nodes = {n: {"name": n, "origin": list(pivot), "children": list(by_group.get(n, []))} for n, (_, pivot) in table.items()}
    outliner = []
    for n, (parent, _) in table.items():
        (outliner if parent is None else nodes[parent]["children"]).append(nodes[n])
    doc["outliner"] = outliner
    doc["textures"][0]["folder"] = "entity"
    out_path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return out_path


def render_check_tex() -> None:
    """check_tex.png：平色 | 贴图，三个视角各一对。"""
    from bbmodel_maker.render.render_bbmodel import render

    flat = write_textured(all_cubes(), MODEL_DIR / "GuiYing_flat_tmp.bbmodel", flat=True)
    tex = write_textured(all_cubes(), MODEL_DIR / "GuiYing_tex.bbmodel")
    bg, size = (119, 119, 119), 440
    views = (("3/4 FRONT-RIGHT (yaw-30 pitch32)", -30.0, 32.0), ("FRONT +Z (yaw0 pitch0)", 0.0, 0.0), ("SIDE +X (yaw90 pitch0)", 90.0, 0.0))
    canvas = Image.new("RGB", (len(views) * 2 * (size + 8), size + 30), (28, 29, 33))
    draw = ImageDraw.Draw(canvas)
    x = 0
    for label, yaw, pitch in views:
        for title, path in (("FLAT", flat), ("TEXTURED", tex)):
            canvas.paste(render(path, yaw=yaw, pitch=pitch, size=size, bg=bg, shading="mc")[0], (x, 30))
            draw.text((x + 4, 8), f"{title}: {label}", fill=(230, 230, 230))
            x += size + 8
    flat.unlink()
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    canvas.save(REVIEW_DIR / "check_tex.png")
    print(f"[tex] {tex.relative_to(REPO)}  check_tex.png → {REVIEW_DIR}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tex", action="store_true", help="只出像素贴图版 GuiYing_tex.bbmodel 与 check_tex.png")
    parser.add_argument("--sheet", action="store_true", help="另出 parts.png / contact_sheet.png / check.png")
    parser.add_argument("--prev", type=Path, help="上一轮的 bbmodel，check.png 里按同取景并排")
    args = parser.parse_args()
    print("gen_gui_ying 自检…")
    self_test()
    if args.tex:
        render_check_tex()
        return
    path = write_bbmodel(all_cubes(), MODEL_DIR / "GuiYing_round1.bbmodel")
    print(f"[bbmodel] {path.relative_to(REPO)}")
    if args.sheet:
        render_sheets(args.prev)


if __name__ == "__main__":
    main()
