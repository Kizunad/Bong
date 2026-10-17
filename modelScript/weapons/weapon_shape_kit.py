#!/usr/bin/env python3
"""弯形兵器公共工具：参考图剪影 → 刃片 / 弓臂折线，以及统一的 check.png 流程。

为什么存在：戈的胡、钺的新月刃、朴刀上挑的刀头、反曲弓的 M 形弓臂，辨识度全在曲线上，
手写方块坐标很难和参考图对齐。这里让模型直接贴着**参考图剪影**走：

- ``ref_mask`` 从 ``<id>_ref.png`` 的主视图抠出兵器剪影（只取最大连通域，去掉弓弦这类细线）；
- ``MaskFrame`` 把剪影像素坐标换成模型 px（按全长定比例）；
- ``plate_from_mask`` 把剪影栅格成楔形刃片（刃口薄、中脊厚，厚度由到刃口的距离决定）；
- ``strip_along`` 沿折线铺弓臂（每段一个带旋转的方块，端点落在折线上）；
- ``octagon_prism`` 八角近圆柱（杆、柄），``Silhouette`` 比较参考图与模型剪影。

参考图是独立生成的，不是模型自己出题自己判卷；特征百分比表里「参考图」一列是人从剪影量出来填的。
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable

import numpy as np
from PIL import Image
from scipy import ndimage

from bbmodel_maker.rig.rigkit import Rig, shaft_box

REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/weapons")

Vec = tuple[float, float, float]


# --------------------------------------------------------------------------- 参考图剪影

def ref_mask(ref_png: Path, panel_width: int, thresh: float = 28.0, small_hole: int = 400) -> np.ndarray:
    """抠出参考图主视图（左侧大图）的兵器剪影，返回与左侧面板同尺寸的布尔矩阵。

    背景是带轻微渐变的灰底，所以逐行取左边缘的中位数当该行背景；
    闭运算 + 填小孔 + 开运算，去掉弓弦之类的细线和毛刺，再取最大连通域；面积 ≥ small_hole 的孔保留（如钺的圆孔）。
    """
    im = np.array(Image.open(ref_png).convert("RGB")).astype(float)[:, :panel_width]
    row_bg = np.median(im[:, :40], axis=1)
    diff = np.abs(im - row_bg[:, None, :]).max(axis=2)
    mask = diff > thresh
    # 锈斑 / 铜绿的颜色会碰巧接近灰底，先闭运算补上、填掉小孔，再开运算削掉毛刺
    mask = ndimage.binary_closing(mask, structure=np.ones((3, 3)), iterations=2)
    holes, hole_count = ndimage.label(~mask)
    for k, size in enumerate(ndimage.sum(~mask, holes, range(1, hole_count + 1)), start=1):
        if size < small_hole:
            mask[holes == k] = True
    mask = ndimage.binary_opening(mask, structure=np.ones((3, 3)), iterations=2)
    labels, count = ndimage.label(mask)
    sizes = ndimage.sum(mask, labels, range(1, count + 1))
    return labels == (1 + int(np.argmax(sizes)))


def mask_bbox(mask: np.ndarray) -> tuple[int, int, int, int]:
    ys, xs = np.where(mask)
    return int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())


def crop_ref_view(ref_png: Path, mask: np.ndarray, pad: int = 6) -> Image.Image:
    """按剪影包围盒裁出参考图主视图（保留原图内容，给比例对照栏用）。"""
    x0, y0, x1, y1 = mask_bbox(mask)
    full = Image.open(ref_png).convert("RGB")
    return full.crop((max(0, x0 - pad), max(0, y0 - pad), x1 + pad + 1, y1 + pad + 1))


@dataclass
class MaskFrame:
    """剪影像素 ↔ 模型 px 的换算。

    ``px_per_unit`` 由「剪影高度 = 模型全长」得出；``anchor_px`` 是模型 (0,0) 对应的像素
    （x 取轴线，y 取兵器底端）。``u_sign`` 取 -1 可水平翻转。
    """

    px_per_unit: float
    anchor_px: tuple[float, float]
    u_sign: int = 1

    def to_model(self, px: float, py: float) -> tuple[float, float]:
        u = self.u_sign * (px - self.anchor_px[0]) / self.px_per_unit
        v = (self.anchor_px[1] - py) / self.px_per_unit
        return u, v

    def to_px(self, u: float, v: float) -> tuple[float, float]:
        return (self.anchor_px[0] + self.u_sign * u * self.px_per_unit,
                self.anchor_px[1] - v * self.px_per_unit)


def frame_from_mask(mask: np.ndarray, total_len: float, axis_x_px: float, u_sign: int = 1) -> MaskFrame:
    _, y0, _, y1 = mask_bbox(mask)
    return MaskFrame((y1 - y0 + 1) / total_len, (axis_x_px, float(y1) + 0.5), u_sign)


def sampler(mask: np.ndarray, frame: MaskFrame) -> Callable[[float, float, float], float]:
    """返回 cover(u0, v0, size)：模型坐标里 size×size 小格被剪影覆盖的比例。"""

    def cover(u: float, v: float, size: float) -> float:
        xa, ya = frame.to_px(u, v + size)
        xb, yb = frame.to_px(u + size, v)
        x0, x1 = sorted((int(round(xa)), int(round(xb))))
        y0, y1 = sorted((int(round(ya)), int(round(yb))))
        if x1 <= x0:
            x1 = x0 + 1
        if y1 <= y0:
            y1 = y0 + 1
        patch = mask[max(0, y0):max(0, y1), max(0, x0):max(0, x1)]
        return float(patch.mean()) if patch.size else 0.0

    return cover


# --------------------------------------------------------------------------- 刃片

def quant(value: float, step: float = 0.05) -> float:
    return round(round(value / step) * step, 4)


@dataclass
class PlateSpec:
    """楔形刃片参数。

    ``plane`` 是片所在的平面，厚度方向是第三轴：``"zy"`` → 厚度沿 x，``"xy"`` → 厚度沿 z。
    ``edge_mode``：``"all"`` 到任意轮廓的距离（透镜截面，戈 / 钺）；
    ``"left"`` / ``"right"`` 只从该侧刃口算（单刃直背刀，另一侧是平背）。
    """

    plane: str
    cell: float = 0.25
    half_edge: float = 0.07   # 刃口半厚
    half_max: float = 0.2     # 中脊半厚
    ramp: float = 0.8         # 从刃口爬到中脊用的宽度
    levels: int = 4
    edge_mode: str = "all"
    thick_center: float = 0.0


def plate_from_mask(
    rig: Rig,
    bone: str,
    prefix: str,
    cover: Callable[[float, float, float], float],
    bounds: tuple[float, float, float, float],
    spec: PlateSpec,
    mat_edge: str,
    mat_body: str,
    keep: Callable[[float, float], bool] | None = None,
) -> int:
    """把剪影栅格成楔形刃片，返回加了多少方块。bounds=(u0,u1,v0,v1) 是模型 px 范围。"""
    u0, u1, v0, v1 = bounds
    cell = spec.cell
    nu, nv = int(math.ceil((u1 - u0) / cell)), int(math.ceil((v1 - v0) / cell))
    grid = np.zeros((nv, nu), dtype=bool)
    for j in range(nv):
        for i in range(nu):
            cu, cv = u0 + i * cell, v0 + j * cell
            if keep is not None and not keep(cu + cell / 2, cv + cell / 2):
                continue
            grid[j, i] = cover(cu, cv, cell) >= 0.5

    if spec.edge_mode == "all":
        dist = ndimage.distance_transform_edt(np.pad(grid, 1))[1:-1, 1:-1] * cell
    else:
        dist = np.zeros(grid.shape)
        for j in range(nv):
            idx = np.where(grid[j])[0]
            if len(idx) == 0:
                continue
            for i in idx:
                dist[j, i] = (i - idx.min() + 0.5) * cell if spec.edge_mode == "left" else (idx.max() - i + 0.5) * cell

    level = np.zeros(grid.shape, dtype=int)
    for j in range(nv):
        for i in range(nu):
            if grid[j, i]:
                frac = min(1.0, dist[j, i] / spec.ramp)
                level[j, i] = 1 + int(round(frac * (spec.levels - 1)))

    def half(lv: int) -> float:
        frac = (lv - 1) / max(1, spec.levels - 1)
        return quant(spec.half_edge + (spec.half_max - spec.half_edge) * frac, 0.01)

    # 横向合并相同层级的格子，再纵向合并相同 (u 范围, 层级) 的连续行
    runs: dict[tuple[int, int, int], list[int]] = {}
    out: list[tuple[int, int, int, int, int]] = []
    for j in range(nv):
        row_runs: list[tuple[int, int, int]] = []
        i = 0
        while i < nu:
            if level[j, i] == 0:
                i += 1
                continue
            k = i
            while k + 1 < nu and level[j, k + 1] == level[j, i]:
                k += 1
            row_runs.append((i, k, int(level[j, i])))
            i = k + 1
        live = {key: ext for key, ext in runs.items()}
        new_runs: dict[tuple[int, int, int], list[int]] = {}
        for key in row_runs:
            if key in live:
                new_runs[key] = [live[key][0], j]
            else:
                new_runs[key] = [j, j]
        for key, (ja, jb) in runs.items():
            if key not in new_runs:
                out.append((key[0], key[1], ja, jb, key[2]))
        runs = new_runs
    for key, (ja, jb) in runs.items():
        out.append((key[0], key[1], ja, jb, key[2]))

    count = 0
    c0 = spec.thick_center
    for n, (ia, ib, ja, jb, lv) in enumerate(out):
        ua, ub = u0 + ia * cell, u0 + (ib + 1) * cell
        va, vb = v0 + ja * cell, v0 + (jb + 1) * cell
        ht = half(lv)
        mat = mat_edge if lv <= 2 else mat_body
        if spec.plane == "zy":
            frm, to = (c0 - ht, va, ua), (c0 + ht, vb, ub)
        elif spec.plane == "xy":
            frm, to = (ua, va, c0 - ht), (ub, vb, c0 + ht)
        else:
            raise ValueError(f"未知平面 {spec.plane}")
        rig.cube(bone, f"{prefix}_{n:03d}", frm, to, mat=mat)
        count += 1
    return count


# --------------------------------------------------------------------------- 柱与折线

def octagon_prism(rig: Rig, bone: str, name: str, cx: float, cz: float, y0: float, y1: float,
                  r: float, mat: str) -> None:
    """正八角柱（内切半径 r）：十字两块 + 两块 ±45° 斜板，四块拼成。

    只叠两个互转 45° 的方柱会变成八角星（轴向伸出 1.41 倍），所以斜板要窄：
    半宽 r（斜向内切距离）、半厚 0.414 r。
    """
    c = 0.4142 * r
    mid = (y0 + y1) / 2
    rig.cube(bone, f"{name}_a", (cx - r, y0, cz - c), (cx + r, y1, cz + c), mat=mat)
    rig.cube(bone, f"{name}_b", (cx - c, y0, cz - r), (cx + c, y1, cz + r), mat=mat)
    for tag, ang in (("c", 45.0), ("d", -45.0)):
        rig.cube(bone, f"{name}_{tag}", (cx - r, y0, cz - c), (cx + r, y1, cz + c),
                 rot=(0.0, ang, 0.0), org=(cx, mid, cz), mat=mat)


def octagon_taper(rig: Rig, bone: str, name: str, cx: float, cz: float, y0: float, y1: float,
                  r0: float, r1: float, steps: int, mat: str) -> None:
    """分段收分的八角柱（r0 在 y0 端，r1 在 y1 端）。"""
    for i in range(steps):
        ya, yb = y0 + (y1 - y0) * i / steps, y0 + (y1 - y0) * (i + 1) / steps
        r = r0 + (r1 - r0) * (i + 0.5) / steps
        octagon_prism(rig, bone, f"{name}_{i:02d}", cx, cz, ya, yb, r, mat)


def strip_along(rig: Rig, bone: str, prefix: str, pts: list[Vec], half_w: list[float],
                half_t: list[float], mat: str, up: Vec = (0.0, 0.0, 1.0), extend: float = 0.04) -> int:
    """沿折线铺一条带：每段一个方块，局部 X 为宽、局部 Z（朝 up）为厚。返回方块数。"""
    for i, (a, b) in enumerate(zip(pts, pts[1:])):
        w = (half_w[i] + half_w[i + 1]) / 2
        t = (half_t[i] + half_t[i + 1]) / 2
        frm, to, rot, org = shaft_box(a, b, w, t, extend, up)
        rig.cube(bone, f"{prefix}_{i:02d}", frm, to, rot=rot, org=org, mat=mat)
    return len(pts) - 1


def resample(profile: list[tuple[float, float]], step: float) -> list[tuple[float, float]]:
    """把 (v, u) 采样点按 v 线性插值成等间距序列。"""
    vs = [p[0] for p in profile]
    us = [p[1] for p in profile]
    n = int(round((vs[-1] - vs[0]) / step))
    grid = np.linspace(vs[0], vs[-1], n + 1)
    return list(zip(grid.tolist(), np.interp(grid, vs, us).tolist()))


# --------------------------------------------------------------------------- 剪影比较

def rig_part_spans(rig: Rig, bone: str, axis: int = 1) -> tuple[float, float]:
    """某根骨上所有方块（含旋转后的包围盒）在 axis 轴上的 [min, max]。"""
    lo, hi = 1e9, -1e9
    ids = set(rig.bones[bone]["children"])
    for el in rig.elements:
        if el["uuid"] not in ids:
            continue
        rot = el["rotation"]
        f, t = el["from"], el["to"]
        if any(abs(r) > 1e-6 for r in rot):
            org = el["origin"]
            r = max(abs(f[k] - org[k]) for k in range(3)) + max(abs(t[k] - org[k]) for k in range(3))
            lo, hi = min(lo, org[axis] - r / 2), max(hi, org[axis] + r / 2)
        else:
            lo, hi = min(lo, f[axis]), max(hi, t[axis])
    return lo, hi


def silhouette_iou(ref: np.ndarray, model: np.ndarray) -> float:
    """两个布尔剪影缩到同一尺寸后的 IoU（包围盒对包围盒，比较形状不比较绝对大小）。"""
    def tight(m: np.ndarray) -> np.ndarray:
        ys, xs = np.where(m)
        return m[ys.min():ys.max() + 1, xs.min():xs.max() + 1]

    a, b = tight(ref), tight(model)
    height = 480
    width = max(8, int(round(height * (a.shape[1] / a.shape[0]))))
    ai = np.array(Image.fromarray((a * 255).astype(np.uint8)).resize((width, height), Image.Resampling.BILINEAR)) > 127
    bi = np.array(Image.fromarray((b * 255).astype(np.uint8)).resize((width, height), Image.Resampling.BILINEAR)) > 127
    inter = np.logical_and(ai, bi).sum()
    union = np.logical_or(ai, bi).sum()
    return float(inter / union) if union else 0.0


def model_mask(img: Image.Image, bg: tuple[int, int, int], thresh: float = 14.0) -> np.ndarray:
    arr = np.array(img.convert("RGB")).astype(float)
    return np.abs(arr - np.array(bg, float)).max(axis=2) > thresh
