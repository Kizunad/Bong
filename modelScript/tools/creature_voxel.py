"""末世生物生成器共用的「小方块堆形」工具：抖动方块、体素椭球、沿折线的粗糙肢干、逐级收细的锥链。

为什么单独一个文件：游商傀儡那条路线是「每个部件手写十几个方块」，细节够但只适合规整的骨头 / 木件。
毛、皮、树皮、弧形躯干这类有机形体要靠几百上千个小方块参差堆出来，四只兽（旱骨蛇 / 枯枝螳 / 断角野牛 /
碎岩猿）都用同一套手法，所以工具放这里，生成器只写自己的形状。

约定：
  · 所有方块都是不旋转的轴对齐方块（共面门只查无旋转件，旋转件会躲过检查）；
  · 每块的位置和尺寸都带小抖动（尺寸只增不减），相邻方块的面因此几乎不会恰好共面；
  · ``k`` 是整组放大倍数，用于「按 1.0 比例写、整体放大」的头部之类。
"""

from __future__ import annotations

import math
import random
from typing import Callable, Iterable, Sequence

import numpy as np

Vec = Sequence[float]
CORE_CAP = 3.2        # limb 里单块芯 / 表层块的最大边长（px）


def smooth(t: float) -> float:
    t = min(1.0, max(0.0, t))
    return t * t * (3.0 - 2.0 * t)


def pick(rng: random.Random, table: Sequence[tuple[str, float]]) -> str:
    """按权重抽一个材质名。"""
    r = rng.random() * sum(w for _, w in table)
    for mat, w in table:
        r -= w
        if r <= 0:
            return mat
    return table[-1][0]


def jcube(b, bone: str, name: str, center: Vec, size, mat: str, rng: random.Random,
          jit: float = 0.1, k: float = 1.0) -> None:
    """以 center 为中心放一块；size 可以是标量或 (x, y, z)。位置与尺寸带抖动，尺寸不小于 0.4px。"""
    sz = (size, size, size) if isinstance(size, (int, float)) else size
    sz = tuple(max(0.4, v * k + rng.uniform(0.0, 0.18)) for v in sz)
    c = tuple(center[i] * k + rng.uniform(-jit, jit) for i in range(3))
    b.box(bone, name, tuple(c[i] - sz[i] / 2 for i in range(3)), tuple(c[i] + sz[i] / 2 for i in range(3)), mat)


_NEIGHBOURS = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))


def ellipsoid_shell(b, bone: str, tag: str, center: Vec, radii: Vec, cell: float, mats, rng: random.Random,
                    carve: Iterable[tuple[Vec, float]] = (), clip: Callable[[float, float, float], bool] | None = None,
                    k: float = 1.0, size_mul: float = 1.0) -> int:
    """把椭球按 cell 大小体素化，只留表面一层（逐级收边的弧面）；返回放了多少块。

    carve 是 (中心, 半径) 的球，落在里面的体素挖掉；clip(x, y, z) 返回 False 的体素也挖掉。
    carve / clip 用的都是 center 所在的局部坐标（未乘 k）。
    """
    cx, cy, cz = center
    rx, ry, rz = radii

    def inside(i: int, j: int, kk: int) -> bool:
        x, y, z = i * cell, j * cell, kk * cell
        if (x / rx) ** 2 + (y / ry) ** 2 + (z / rz) ** 2 > 1.0:
            return False
        px, py, pz = cx + x, cy + y, cz + z
        if clip and not clip(px, py, pz):
            return False
        return not any((px - c[0]) ** 2 + (py - c[1]) ** 2 + (pz - c[2]) ** 2 <= r * r for c, r in carve)

    ni, nj, nk = int(rx / cell) + 1, int(ry / cell) + 1, int(rz / cell) + 1
    placed = 0
    for i in range(-ni, ni + 1):
        for j in range(-nj, nj + 1):
            for kk in range(-nk, nk + 1):
                if not inside(i, j, kk):
                    continue
                if all(inside(i + di, j + dj, kk + dk) for di, dj, dk in _NEIGHBOURS):
                    continue
                jcube(b, bone, f"{tag}_{i}_{j}_{kk}", (cx + i * cell, cy + j * cell, cz + kk * cell),
                      cell * size_mul, pick(rng, mats), rng, jit=0.08, k=k)
                placed += 1
    return placed


def perpendicular_basis(t: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """与单位向量 t 垂直的两根单位向量。"""
    helper = np.array([0.0, 1.0, 0.0]) if abs(t[1]) < 0.9 else np.array([1.0, 0.0, 0.0])
    u = np.cross(t, helper)
    u /= np.linalg.norm(u)
    return u, np.cross(t, u)


def limb(b, bone: str, tag: str, pts: Sequence[Vec], mats, rng: random.Random, bumps: int = 3,
         bump_mats=None, step: float | None = None, k: float = 1.0) -> list[tuple[np.ndarray, np.ndarray, float]]:
    """沿折线 pts（每点 (x, y, z, 半径)）铺一根粗糙的肢干：每隔一步一块芯 + 几块向外凸的树皮 / 皮疙瘩。

    半径沿折线线性插值。返回每个采样点的 (位置, 切线, 半径)，调用者可以接着在上面挂刺 / 枝杈。
    """
    arr = [np.array(p, dtype=float) for p in pts]
    out: list[tuple[np.ndarray, np.ndarray, float]] = []
    for a, c in zip(arr, arr[1:]):
        seg = float(np.linalg.norm(c[:3] - a[:3]))
        if seg < 1e-6:
            continue
        t = (c[:3] - a[:3]) / seg
        r_mid = float((a[3] + c[3]) / 2)
        stp = step or max(0.7, r_mid * 0.95)
        n = max(1, int(seg / stp))
        u, v = perpendicular_basis(t)
        for i in range(n):
            f = (i + 0.5) / n
            pos = a[:3] + (c[:3] - a[:3]) * f
            rad = float(a[3] + (c[3] - a[3]) * f)
            core = min(2.0 * rad * rng.uniform(0.85, 1.05), CORE_CAP)
            jcube(b, bone, f"{tag}_{len(out)}_core", pos, core, pick(rng, mats), rng, jit=0.05, k=k)
            # 粗肢（半径 > 1.7）的芯被 CORE_CAP 压小了，改由更多的表层块把外形撑起来，不再是几块大方砖
            extra = int(max(0.0, rad - 1.7) * 3.0)
            for m in range(bumps + extra):
                ang = rng.uniform(0.0, 2.0 * math.pi)
                off = rad * rng.uniform(0.7, 1.0) * (math.cos(ang) * u + math.sin(ang) * v)
                jcube(b, bone, f"{tag}_{len(out)}_b{m}", pos + off, min(rad * rng.uniform(0.7, 1.05), CORE_CAP),
                      pick(rng, bump_mats or mats), rng, jit=0.08, k=k)
            out.append((pos, t, rad))
    return out


def taper_chain(b, bone: str, tag: str, p0: Vec, p1: Vec, w0: float, w1: float, mat, rng: random.Random,
                curl: Vec = (0.0, 0.0, 0.0), overlap: float = 0.6, k: float = 1.0, jit: float = 0.04,
                stretch: float = 1.0) -> None:
    """从 p0 到 p1 一串逐级收细的方块（刺、牙、角、毛簇）：宽度 w0 → w1，相邻两块相接。

    curl 是中段额外偏出多少（二次曲线，两端为 0），让它弯而不是直。mat 可以是材质名，也可以是 (材质表) 抽样。
    stretch > 1 时每块沿链的主方向（|分量| 最大的那根轴）拉长这么多倍，一条毛 / 一根丝用更少的块读成「一条」
    而不是「一摞方块」。
    """
    a, c = np.array(p0, dtype=float), np.array(p1, dtype=float)
    curl_v = np.array(curl, dtype=float)
    length = float(np.linalg.norm(c - a))
    axis = int(np.argmax(np.abs(c - a)))
    # 相邻两块的间距按「较细一端」算：方块最小 0.4px，间距比它大就会在尖端断开（悬空）
    thinnest = max(0.4, min(w0, w1)) * k
    n = max(2, int(length / (thinnest * stretch * overlap)) + 1)
    for i in range(n):
        f = i / (n - 1)
        pos = a + (c - a) * f + curl_v * (4.0 * f * (1.0 - f))
        w = w0 + (w1 - w0) * f
        m = pick(rng, mat) if not isinstance(mat, str) else mat
        size = [w, w, w]
        size[axis] = w * stretch
        jcube(b, bone, f"{tag}_{i}", pos, tuple(size), m, rng, jit=jit, k=k)


def strand(b, bone: str, tag: str, start: Vec, length: float, width: float, rng: random.Random, mats,
           sway: float = 0.35, k: float = 1.0, stretch: float = 2.2) -> None:
    """一绺垂毛：从 start 往下垂 length，宽 width 逐渐收到 0.45 倍；末段略偏出一点，像垂着的湿毛。
    每块沿竖直方向拉长 stretch 倍，读成一条细毛而不是一摞方块。"""
    x, y, z = start
    end = (x + rng.uniform(-sway, sway), y - length, z + rng.uniform(-sway, sway))
    taper_chain(b, bone, tag, start, end, width, width * 0.45, mats, rng, overlap=0.7, stretch=stretch, k=k)


def fur_cover(b, bone_of: Callable[[float, float, float], str], tag: str, center: Vec, radii: Vec, rng: random.Random, mats,
              spacing: float = 1.5, length: tuple[float, float] = (5.0, 10.0), floor: float = 8.5,
              width: tuple[float, float] = (0.8, 1.05), skip: Callable[[float, float, float], bool] | None = None,
              k: float = 1.0) -> int:
    """给一个椭球的表面披毛：顶面按 (x, z) 栅格、两侧按 (y, z) 栅格各起一批垂毛，下端不低于 floor。

    侧面的毛从表面起笔，一排压着一排往下垂，像层层叠叠的瀑布。bone_of(x, y, z) 决定每绺归哪块骨；
    skip(x, y, z) 为真的起点不长毛（脸、掌心这类裸皮处）。返回长了多少绺。
    """
    cx, cy, cz = center
    rx, ry, rz = radii
    n = 0

    def grow(px: float, py: float, pz: float, sx_: float) -> None:
        nonlocal n
        if skip and skip(sx_, py, pz):
            return
        ln = min(rng.uniform(*length), py - floor)
        if ln > 1.5:
            strand(b, bone_of(sx_, py, pz), f"{tag}{n}", (px, py, pz), ln, rng.uniform(*width), rng, mats, k=k)
            n += 1

    for x in np.arange(-rx, rx + 0.01, spacing):                           # 顶面
        for z in np.arange(cz - rz, cz + rz + 0.01, spacing):
            s = 1.0 - (x / rx) ** 2 - ((z - cz) / rz) ** 2
            if s <= 0.0:
                continue
            y = cy + ry * math.sqrt(s)
            px, pz = x + rng.uniform(-0.35, 0.35), z + rng.uniform(-0.35, 0.35)
            grow(cx + px * 0.96, y - 0.3, pz, cx + px)
    for z in np.arange(cz - rz, cz + rz + 0.01, spacing):                  # 两侧
        y = cy + ry * 0.85
        while y > floor + 1.5:
            s = 1.0 - ((y - cy) / ry) ** 2 - ((z - cz) / rz) ** 2
            if s > 0.0:
                x = rx * math.sqrt(s)
                for sign in (1, -1):
                    py, pz = y + rng.uniform(-0.35, 0.35), z + rng.uniform(-0.35, 0.35)
                    grow(cx + sign * x * 0.93, py, pz, cx + sign * x)
            y -= spacing * 1.3
    return n


def rock_slab(b, bone: str, tag: str, center: Vec, size: Vec, rng: random.Random, mats, cell: float = 1.4,
              bite: float = 0.25) -> None:
    """一块有棱角的岩片：按 cell 切成一格格小方块，各格随机取岩色（亮 / 暗 / 苔），边角格随机缺掉一些，轮廓参差。
    center / size 是整块的中心和三边长。"""
    cx, cy, cz = center
    nx, ny, nz = (max(1, round(s / cell)) for s in size)
    for i in range(nx):
        for j in range(ny):
            for kk in range(nz):
                edge = int(i in (0, nx - 1)) + int(j in (0, ny - 1)) + int(kk in (0, nz - 1))
                if edge >= 3 and rng.random() < 0.8:                       # 顶角：大概率缺掉
                    continue
                if edge == 2 and rng.random() < bite:                      # 棱：偶尔缺一块
                    continue
                pos = (cx + (i - (nx - 1) / 2) * size[0] / nx, cy + (j - (ny - 1) / 2) * size[1] / ny,
                       cz + (kk - (nz - 1) / 2) * size[2] / nz)
                jcube(b, bone, f"{tag}_{i}_{j}_{kk}", pos, (size[0] / nx + 0.05, size[1] / ny + 0.05, size[2] / nz + 0.05),
                      pick(rng, mats), rng, jit=0.12)
