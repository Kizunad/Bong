#!/usr/bin/env python3
"""诡影（空兽 / gui_ying）建模源：无实体的真空漩涡。Round 1 只做几何，不贴图、不做动画、不接线。

正典依据（docs/worldview.md §二 L51）：负灵域的特有生态——看不见的真空漩涡「诡影（空兽）」，撞上直接扣真元，
是唯一敢待在负灵域的生物。**没有实体、几乎看不见**，所以外形不是一只兽，而是一道旋涡：

    part_ring(i)    六层内凹、互相错开旋转角度的环状扭曲层（越往深处半径越小，每层绕轴多转 17°），
                    每层是被撕开的断弧：外缘深色边、环线是高对比的骨灰色细线、内缘再压一圈深色；
    part_throat()   旋涡的喉管：贴着最内层往里收的一圈深色管壁 + 最深处的空洞（近黑，唯一最暗的一块）；
    part_debris()   外圈绕旋着的碎灰（1px 灰块）和枯叶碎片（3×1×2 的薄片），沿螺旋线从开口一直拖到尖端。

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
    "rim": (26, 24, 31),           # 环的外缘 / 内缘深色边
    "body": (58, 54, 64),          # 环体
    "body_lit": (84, 80, 90),      # 环体偶尔亮一块，避免一整片死色
    "line": (214, 208, 197),       # 环线：骨灰色，和深边的亮度差拉满
    "ash": (152, 148, 142),        # 碎灰
    "ash_dark": (96, 92, 90),
    "leaf": (118, 98, 68),         # 枯叶
    "leaf_dark": (78, 64, 46),
}
MATERIALS = tuple(PALETTE)


def luminance(rgb: Tuple[int, int, int]) -> float:
    return 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]


# ── 形体参数（门禁直接读这张表，所以改形体改这里） ───────────────────────────────

AXIS_PITCH_DEG = 38.0                       # 开口方向朝前上方 38°
CENTER = np.array([0.0, 20.0, 8.0])         # 旋涡中心（局部原点），世界坐标；z 向前挪 8，让整团（含拖到尖端的碎屑）在 ±24 内
HOVER_Y = 4.0                               # 整体最低点悬空 4px
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
DEBRIS_R = (18.2, 21.0)                     # 碎屑绕旋半径：必须在最外层之外
DEBRIS_W = (4.0, -20.0)
CONTRAST_MIN = 120.0                        # 环线与深色边的亮度差下限
CELL = 0.5                                  # 体素边长（px）：环宽只有 2.4~3.4px，1px 体素会把环打成噪点

U_HAT = np.array([1.0, 0.0, 0.0])
_PITCH = math.radians(AXIS_PITCH_DEG)
W_HAT = np.array([0.0, math.sin(_PITCH), math.cos(_PITCH)])
V_HAT = np.cross(W_HAT, U_HAT)

Cell = Tuple[int, int, int]
Cells = Dict[Cell, str]

GRID_X = range(-56, 56)                     # 单位：CELL
GRID_Y = range(0, 112)
GRID_Z = range(-60, 60)


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
    lit = ((np.floor(u / 2) * 7 + np.floor(v / 2) * 13 + np.floor(w / 2) * 5 + i * 3) % 8) == 0
    mat[lit] = MATERIALS.index("body_lit")
    front = w >= spec["w"] + LAYER_THICK / 2 - CELL * 1.5                   # 朝开口的那一面
    line_band = (r >= spec["r_out"] - 2.0) & (r <= spec["r_out"] - 1.0) & front
    mat[line_band] = MATERIALS.index("line")
    mat[(r > spec["r_out"] - 0.9) | (r < r_in + 0.9)] = MATERIALS.index("rim")
    mat[spike] = MATERIALS.index("rim")
    mat[line_band & (r <= spec["r_out"] - 1.0)] = MATERIALS.index("line")
    return _to_cells(mask, mat)


def throat_cells() -> Cells:
    """喉管：贴着最内层往里收的深色管壁 + 最深处的空洞（全模型最暗的 void）。"""
    xs, ys, zs = _grid()
    u, v, w, r, theta = _local(xs, ys, zs)
    wall = (r >= THROAT_R[0]) & (r <= THROAT_R[1]) & (w >= THROAT_W[0]) & (w <= THROAT_W[1])
    hole = (r < VOID_R + 0.3) & (w >= VOID_W[0]) & (w <= VOID_W[1])
    mask = _surface(wall | hole)
    mat = np.full(xs.shape, MATERIALS.index("rim"), dtype=int)
    mat[hole] = MATERIALS.index("void")
    streak = wall & ((np.floor(w) + np.floor(theta * 3)) % 4 == 0)
    mat[streak] = MATERIALS.index("body")
    return _to_cells(mask, mat)


def debris_cells(seed: int = 7) -> Cells:
    """外圈碎屑：沿螺旋线从开口一直拖到尖端，碎灰（1px 灰块）+ 枯叶碎片（3×1×2 的薄片，朝向随机）。1px = 2 个 CELL。"""
    rng = random.Random(seed)
    cells: Cells = {}
    per_px = int(round(1 / CELL))
    for k in range(DEBRIS_COUNT):
        t = (k + rng.random() * 0.8) / DEBRIS_COUNT
        w = DEBRIS_W[0] + (DEBRIS_W[1] - DEBRIS_W[0]) * t
        radius = DEBRIS_R[0] + (DEBRIS_R[1] - DEBRIS_R[0]) * rng.random()
        theta = 5.2 * math.pi * t + rng.uniform(-0.5, 0.5)
        local = np.array([radius * math.cos(theta), radius * math.sin(theta), w])
        world = CENTER + local[0] * U_HAT + local[1] * V_HAT + local[2] * W_HAT
        base = [int(math.floor(world[a])) * per_px for a in range(3)]
        if rng.random() < 0.55:
            size = (1, 1, 1)
            material = "ash" if rng.random() < 0.6 else "ash_dark"
        else:
            size = rng.choice(((3, 1, 2), (2, 1, 3), (3, 2, 1), (1, 2, 3), (2, 3, 1)))
            material = "leaf" if rng.random() < 0.6 else "leaf_dark"
        for dx in range(size[0] * per_px):
            for dy in range(size[1] * per_px):
                for dz in range(size[2] * per_px):
                    cells[(base[0] + dx, base[1] + dy, base[2] + dz)] = material
    return cells


# ── 体素 → 立方体（沿 x 合并同材质连续体素） ─────────────────────────────────────


def cells_to_cubes(cells: Cells, name: str, group: str, y_shift: float = 0.0) -> List[Cube]:
    rows: Dict[Tuple[int, int, str], List[int]] = {}
    for (x, y, z), material in cells.items():
        rows.setdefault((y, z, material), []).append(x)
    cubes: List[Cube] = []
    for (y, z, material), xs in sorted(rows.items()):
        xs.sort()
        start = prev = xs[0]
        for x in xs[1:] + [None]:
            if x is not None and x == prev + 1:
                prev = x
                continue
            cubes.append({"name": f"{name}_{len(cubes):04d}",
                          "from": [round(start * CELL, 4), round(y * CELL + y_shift, 4), round(z * CELL, 4)],
                          "to": [round((prev + 1) * CELL, 4), round((y + 1) * CELL + y_shift, 4), round((z + 1) * CELL, 4)],
                          "group": group, "material": material})
            if x is not None:
                start = prev = x
    return cubes


def _min_y(cell_sets: Sequence[Cells]) -> float:
    return min(y for cells in cell_sets for (_, y, _) in cells) * CELL


_CACHE: Dict[str, Cells] = {}


def _cells(key: str) -> Cells:
    """各部件体素缓存（一次进程内只算一遍）。"""
    if key not in _CACHE:
        if key.startswith("ring_"):
            _CACHE[key] = ring_cells(int(key.split("_")[1]))
        elif key == "throat":
            _CACHE[key] = throat_cells()
        else:
            _CACHE[key] = debris_cells()
    return _CACHE[key]


def _part_keys() -> List[str]:
    return [f"ring_{i}" for i in range(LAYER_COUNT)] + ["throat", "debris"]


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
    _assert_no_coplanar_faces(cubes)
    lo = [min(c["from"][i] for c in cubes) for i in range(3)]
    hi = [max(c["to"][i] for c in cubes) for i in range(3)]
    assert lo[0] >= -24 and hi[0] <= 24 and lo[2] >= -24 and hi[2] <= 24 and lo[1] >= 0 and hi[1] <= 48, f"外包超出 48³: {lo} {hi}"
    for i in range(LAYER_COUNT):
        assert any(c["group"] == f"ring_{i}" for c in cubes), f"缺第 {i} 层环"
    void = [c for c in cubes if c["material"] == "void"]
    assert void, "没有空洞"
    debris = [c for c in cubes if c["group"] == "debris"]
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


def self_test() -> None:
    check_spec(LAYERS)
    check_contrast(PALETTE)
    cubes = all_cubes()
    check_cubes(cubes)
    print(f"  [OK] {len(cubes)} 个立方体：共面 0 冲突（全部轴对齐）、六层内凹且逐层错开、空洞 {sum(1 for c in cubes if c['material'] == 'void')} 条、"
          f"碎屑 {sum(1 for c in cubes if c['group'] == 'debris')} 条、对比度 {luminance(PALETTE['line']) - luminance(PALETTE['rim']):.0f}")

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
    must_fail("碎屑太少", lambda: check_cubes([c for c in cubes if c["group"] != "debris"]))
    origin = CENTER + np.array([0.0, y_shift_for_hover(), 0.0])
    inner = {"name": "debris_inject", "from": [float(origin[0]), float(origin[1]), float(origin[2])],
             "to": [float(origin[0]) + 1, float(origin[1]) + 1, float(origin[2]) + 1], "group": "debris", "material": "ash"}
    must_fail("碎屑落进环内", lambda: check_cubes(cubes + [inner]))
    twin = dict(cubes[0], name="dup")
    must_fail("共面", lambda: check_cubes(cubes + [twin]))


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


def write_bbmodel(cubes: List[Cube], out_path: Path, name: str = MODEL_ID) -> Path:
    buf = io.BytesIO()
    build_texture().save(buf, format="PNG")
    source = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")
    elements, groups = [], {}
    for c in cubes:
        k = MATERIALS.index(c["material"])
        u0, v0 = (k % 4) * 16, (k // 4) * 16
        uv = [u0, v0, u0 + 16, v0 + 16]
        elem = {"name": c["name"], "box_uv": False, "from": c["from"], "to": c["to"],
                "faces": {s: {"uv": uv, "texture": 0} for s in ("north", "south", "east", "west", "up", "down")},
                "uuid": str(uuid.uuid4())}
        elements.append(elem)
        groups.setdefault(c["group"], []).append(elem["uuid"])
    doc = {"meta": {"format_version": "4.10", "model_format": "free"}, "name": name,
           "resolution": {"width": 64, "height": 64}, "elements": elements,
           "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in groups.items()],
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


def render_sheets() -> None:
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = Path("/tmp/claude-1000/gui_ying_parts")
    tmp.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    whole = write_bbmodel(all_cubes(), MODEL_DIR / "GuiYing_round1.bbmodel")

    # 逐件预览（每件单独一张 3/4）
    tiles = []
    for name, cubes in parts().items():
        path = write_bbmodel(cubes, tmp / f"{name}.bbmodel", name)
        im, _ = render(path, yaw=35.0, pitch=25.0, size=300, bg=bg)
        tiles.append((f"{name} ({len(cubes)} cubes)", im))
    cols = 4
    rows = (len(tiles) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * 310 + 10, rows * 330 + 10), (35, 36, 40))
    draw = ImageDraw.Draw(sheet)
    for idx, (label, im) in enumerate(tiles):
        x, y = 10 + (idx % cols) * 310, 10 + (idx // cols) * 330
        draw.text((x + 4, y), label, fill=(230, 230, 230))
        sheet.paste(im, (x, y + 18))
    sheet.save(REVIEW_DIR / "parts.png")

    # 六视角接触表
    shots = [(label, render(whole, yaw=yaw, pitch=pitch, size=420, bg=bg)[0]) for label, yaw, pitch in VIEWS]
    contact = Image.new("RGB", (3 * 430 + 10, 2 * 450 + 10), (35, 36, 40))
    draw = ImageDraw.Draw(contact)
    for idx, (label, im) in enumerate(shots):
        x, y = 10 + (idx % 3) * 430, 10 + (idx // 3) * 450
        draw.text((x + 4, y), label, fill=(230, 230, 230))
        contact.paste(im, (x, y + 18))
    contact.save(REVIEW_DIR / "contact_sheet.png")

    # check.png：左参考图 | 右同取景（参考图是 3/4 俯视，开口朝观众）对比，下面一行六视角缩略
    ref = Image.open(REVIEW_DIR / "ref.png").convert("RGB")
    h = 560
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    same_view, _ = render(whole, yaw=-30.0, pitch=32.0, size=h, bg=bg)
    thumb_w = 330
    canvas = Image.new("RGB", (max(ref.width + h + 30, 6 * thumb_w // 2 + 20), h + 40 + 360), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    canvas.paste(ref, (10, 30))
    draw.text((14, 8), "REFERENCE (ref.png)", fill=(210, 200, 180))
    canvas.paste(same_view, (ref.width + 20, 30))
    draw.text((ref.width + 24, 8), "NOW RENDER, same framing (yaw-30 pitch32)", fill=(180, 220, 210))
    small = contact.resize((int(contact.width * 340 / contact.height * 1.0) if False else min(canvas.width - 20, 1300), int(min(canvas.width - 20, 1300) * contact.height / contact.width)), Image.Resampling.LANCZOS)
    canvas = canvas.crop((0, 0, canvas.width, 30 + h + 10 + small.height + 10)) if canvas.height < 30 + h + 10 + small.height + 10 else canvas
    canvas.paste(small, (10, 30 + h + 10))
    canvas.save(REVIEW_DIR / "check.png")
    print(f"[sheets] {REVIEW_DIR}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sheet", action="store_true", help="另出 parts.png / contact_sheet.png / check.png")
    args = parser.parse_args()
    print("gen_gui_ying 自检…")
    self_test()
    path = write_bbmodel(all_cubes(), MODEL_DIR / "GuiYing_round1.bbmodel")
    print(f"[bbmodel] {path.relative_to(REPO)}")
    if args.sheet:
        render_sheets()


if __name__ == "__main__":
    main()
