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

def ref_mask(ref_png: Path, panel_width: int, thresh: float = 28.0, small_hole: int = 400,
             shadow_px: int = 22, string_px: int = 1,
             box: tuple[int, int, int, int] | None = None) -> np.ndarray:
    """抠出参考图主视图（左侧大图）的兵器剪影，返回与左侧面板同尺寸的布尔矩阵。

    背景是带轻微渐变的灰底，所以逐行取左边缘的中位数当该行背景；
    闭运算 + 填小孔 + 开运算去毛刺，再取最大连通域；面积 ≥ small_hole 的孔保留（如钺的圆孔）。
    ``box`` 只取拼版里的一幅，``shadow_px`` 去地面阴影，``string_px`` > 1 时用横向开运算削掉弓弦（弓用）。
    """
    im = np.array(Image.open(ref_png).convert("RGB")).astype(float)[:, :panel_width]
    row_bg = np.median(im[:, :40], axis=1)
    diff = np.abs(im - row_bg[:, None, :]).max(axis=2)
    mask = diff > thresh
    if box is not None:      # 参考图是多视图拼版时，只取 box=(x0, y0, x1, y1) 里的那一幅
        keep = np.zeros_like(mask)
        keep[box[1]:box[3], box[0]:box[2]] = True
        mask &= keep
    # 锈斑 / 铜绿的颜色会碰巧接近灰底，先闭运算补上、填掉小孔，再开运算削掉毛刺
    mask = ndimage.binary_closing(mask, structure=np.ones((3, 3)), iterations=2)
    holes, hole_count = ndimage.label(~mask)
    for k, size in enumerate(ndimage.sum(~mask, holes, range(1, hole_count + 1)), start=1):
        if size < small_hole:
            mask[holes == k] = True
    mask = ndimage.binary_opening(mask, structure=np.ones((3, 3)), iterations=2)
    labels, count = ndimage.label(mask)
    sizes = ndimage.sum(mask, labels, range(1, count + 1))
    mask = labels == (1 + int(np.argmax(sizes)))
    # 地面阴影是贴着底边的一条扁楔，竖直方向很薄；杆、镦、锥尖是竖直的。
    # 只在底部 8% 的行里做一次竖向开运算，把扁楔去掉。
    ys = np.where(mask.any(axis=1))[0]
    floor_from = int(ys.max() - 0.08 * (ys.max() - ys.min()))
    opened = ndimage.binary_opening(mask, structure=np.ones((shadow_px, 1)))
    mask[floor_from:] = opened[floor_from:]
    # 弓弦是竖直的细线，竖向开运算留得住它；横向开运算（宽 string_px）才削得掉
    mask = ndimage.binary_opening(mask, structure=np.ones((1, string_px)))
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


def octagon_prism_x(rig: Rig, bone: str, name: str, cy: float, cz: float, x0: float, x1: float,
                    r: float, mat: str) -> None:
    """沿 x 轴的正八角柱（内切半径 r），结构同 octagon_prism，只是轴向换成 x。"""
    c = 0.4142 * r
    mid = (x0 + x1) / 2
    rig.cube(bone, f"{name}_a", (x0, cy - r, cz - c), (x1, cy + r, cz + c), mat=mat)
    rig.cube(bone, f"{name}_b", (x0, cy - c, cz - r), (x1, cy + c, cz + r), mat=mat)
    for tag, ang in (("c", 45.0), ("d", -45.0)):
        rig.cube(bone, f"{name}_{tag}", (x0, cy - r, cz - c), (x1, cy + r, cz + c),
                 rot=(ang, 0.0, 0.0), org=(mid, cy, cz), mat=mat)


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


def silhouette_iou(ref: np.ndarray, model: np.ndarray, height: int = 480, max_shift: int = 24) -> float:
    """两个布尔剪影缩到同一高度（各自保持宽高比，不拉伸）后的 IoU，水平方向允许 ±max_shift 像素对位。

    比较的是形状：全长对齐，横向靠平移找最佳重合（弓的梢端有细小的绳套 / 弦槽，包围盒会差一点）。
    """
    def prep(m: np.ndarray) -> np.ndarray:
        ys, xs = np.where(m)
        m = m[ys.min():ys.max() + 1, xs.min():xs.max() + 1]
        width = max(4, int(round(height * m.shape[1] / m.shape[0])))
        img = Image.fromarray((m * 255).astype(np.uint8)).resize((width, height), Image.Resampling.BILINEAR)
        return np.array(img) > 127

    a, b = prep(ref), prep(model)
    canvas = max(a.shape[1], b.shape[1]) + 2 * max_shift
    best = 0.0
    for shift in range(-max_shift, max_shift + 1):
        ca = np.zeros((height, canvas), bool)
        cb = np.zeros((height, canvas), bool)
        ca[:, max_shift:max_shift + a.shape[1]] = a
        x0 = max_shift + shift
        cb[:, x0:x0 + b.shape[1]] = b
        union = np.logical_or(ca, cb).sum()
        if union:
            best = max(best, float(np.logical_and(ca, cb).sum() / union))
    return best


def model_mask(img: Image.Image, bg: tuple[int, int, int], thresh: float = 14.0) -> np.ndarray:
    arr = np.array(img.convert("RGB")).astype(float)
    return np.abs(arr - np.array(bg, float)).max(axis=2) > thresh


# --------------------------------------------------------------------------- 弓

@dataclass
class BowProfile:
    """弓的侧视中心线：v 向上（0 = 下梢端），z 向弓背（弦所在平面 z=0），half_t 是半厚。"""

    v: list[float]
    z: list[float]
    half_t: list[float]

    def at(self, v: float) -> tuple[float, float]:
        return float(np.interp(v, self.v, self.z)), float(np.interp(v, self.v, self.half_t))

    def point(self, i: int) -> Vec:
        return (0.0, self.v[i], self.z[i])


def bow_profile(mask: np.ndarray, total_len: float, string_x_px: float, step: float = 0.7,
                smooth: int = 2, min_half_t: float = 0.14) -> BowProfile:
    """从弓的侧视剪影取中心线与厚度（距离变换的脊线），上下对称化后按 step 重采样。

    参考图里弓背朝左、弦朝右，所以 z = (弦所在像素列 - 中心线像素列) / 比例；
    半厚取该点到轮廓的最近距离，也就是弓臂沿法向的真实半厚。
    """
    _, y0, _, y1 = mask_bbox(mask)
    scale = (y1 - y0 + 1) / total_len
    edt = ndimage.distance_transform_edt(mask)
    n = int(round(total_len / step))
    vs = np.linspace(0.0, total_len, n + 1)
    zs, ts = [], []
    for v in vs:
        row = int(round(min(y1, max(y0, y1 - v * scale))))
        xc = int(np.argmax(edt[row]))
        zs.append((string_x_px - xc) / scale)
        ts.append(max(min_half_t, float(edt[row, xc]) / scale))
    z, t = np.array(zs), np.array(ts)
    z, t = (z + z[::-1]) / 2.0, (t + t[::-1]) / 2.0      # 上下对称
    if smooth:
        # 端点附近缩小窗口，不然边缘填充会把梢端拉离真实位置
        def blur(a: np.ndarray) -> np.ndarray:
            out = a.copy()
            for i in range(len(a)):
                w = min(smooth, i, len(a) - 1 - i)
                out[i] = a[i - w:i + w + 1].mean()
            return out
        z, t = blur(z), blur(t)
    return BowProfile(vs.tolist(), z.tolist(), t.tolist())


def profile_normals(prof: BowProfile) -> list[tuple[float, float]]:
    """每个采样点的法向 (dv, dz)，朝弓背（+z）为正。"""
    out = []
    n = len(prof.v)
    for i in range(n):
        a, b = max(0, i - 1), min(n - 1, i + 1)
        tv, tz = prof.v[b] - prof.v[a], prof.z[b] - prof.z[a]
        length = math.hypot(tv, tz) or 1.0
        out.append((-tz / length, tv / length))
    return out


def layered_limb(rig: Rig, bone: str, prefix: str, prof: BowProfile, i0: int, i1: int,
                 layers: list[tuple[str, float, float]], half_w_fn: Callable[[float], float]) -> int:
    """沿 [i0, i1] 段铺多层复合弓臂。

    layers = [(材质, 层中心偏移系数, 层半厚系数)]：偏移与半厚都是「该点半厚」的倍数，
    例如背筋 (+0.66, 0.33)、木芯 (0, 0.33)、牛角 (-0.66, 0.33)。返回方块数。
    """
    normals = profile_normals(prof)
    count = 0
    for tag, (mat, off, half) in enumerate(layers):
        pts, hw, ht = [], [], []
        for i in range(i0, i1 + 1):
            dv, dz = normals[i]
            ox = off * prof.half_t[i]
            pts.append((0.0, prof.v[i] + dv * ox, prof.z[i] + dz * ox))
            hw.append(half_w_fn(prof.v[i]))
            ht.append(max(0.05, half * prof.half_t[i]))
        count += strip_along(rig, bone, f"{prefix}_l{tag}", pts, hw, ht, mat)
    return count


BOW_DISP_AT = (0.60, 0.70, 0.80, 0.90, 0.97)


def bow_measure(mask: np.ndarray) -> dict[str, float]:
    """弓侧视剪影（弓背朝左，弦已除去）的形状度量，全部是全长的百分比。

    ``disp@xx``：高度 xx（自下数，占全长）处的中心线相对握把中心的横向位移，朝弓背为正；
    ``thick_*``：该处弓臂厚度（距离变换脊线上的两倍内切半径）；
    ``depth``：剪影包围盒宽；``peak``：中心线最靠弓背的点离握把中心的高度差。
    参考图和模型渲染用同一份，所以数值差就是外形差。
    """
    _, y0, x1, y1 = mask_bbox(mask)
    xmin = mask_bbox(mask)[0]
    height = y1 - y0 + 1
    edt = ndimage.distance_transform_edt(mask)

    def centre(frac: float) -> tuple[int, float]:
        row = int(round(y1 - frac * (height - 1)))
        xc = int(np.argmax(edt[row]))
        return xc, float(edt[row, xc])

    grip_x, grip_r = centre(0.5)
    out: dict[str, float] = {"depth": 100.0 * (x1 - xmin + 1) / height, "thick_grip": 200.0 * grip_r / height}
    for f in BOW_DISP_AT:
        out[f"disp@{int(f * 100)}"] = 100.0 * (grip_x - centre(f)[0]) / height
    fracs = np.linspace(0.5, 0.97, 48)
    xs = [centre(float(f))[0] for f in fracs]
    out["peak"] = 100.0 * (float(fracs[int(np.argmin(xs))]) - 0.5)
    out["thick_mid"] = 200.0 * centre(0.70)[1] / height
    out["thick_tip"] = 200.0 * centre(0.92)[1] / height
    return out


# --------------------------------------------------------------------------- 穿戴 / 挂件

def worn_doc(weapon_doc: dict, offset: tuple[float, float, float]) -> tuple[dict, int]:
    """把挂件模型平移 offset 后和灰色占位玩家合成一份 bbmodel 文档，并对玩家六个方块做分离轴相交检查。

    返回 (文档, 交叠个数)。方块带旋转时，origin 一起平移，相交检查按旋转后的角点算。
    """
    import copy

    import weapon_review_kit as wrk

    doc = {"meta": {"format_version": "4.10", "model_format": "free", "box_uv": False}, "name": "Worn",
           "resolution": weapon_doc.get("resolution", {"width": 64, "height": 64}), "elements": [],
           "outliner": [], "textures": copy.deepcopy(weapon_doc.get("textures", []))}
    body_uv, eye_uv = wrk.inject_grey_player_texture(doc)
    faces = lambda uv: {f: {"uv": list(uv), "texture": 0} for f in ("north", "south", "east", "west", "up", "down")}  # noqa: E731
    for label, (lo, hi) in wrk.PLAYER_BOXES.items():
        doc["elements"].append({"name": f"player_{label}", "box_uv": False, "from": list(lo), "to": list(hi),
                                "faces": faces(body_uv), "uuid": f"p_{label}"})
    for i, sx in enumerate((-2.0, 1.0)):
        doc["elements"].append({"name": f"eye_{i}", "box_uv": False, "from": [sx, 27.0, -4.05],
                                "to": [sx + 1.0, 28.0, -3.95], "faces": faces(eye_uv), "uuid": f"p_eye_{i}"})
    off = np.array(offset, float)
    collisions = 0
    for idx, e in enumerate(weapon_doc["elements"]):
        n = copy.deepcopy(e)
        n["from"] = [round(float(v), 4) for v in np.array(e["from"]) + off]
        n["to"] = [round(float(v), 4) for v in np.array(e["to"]) + off]
        if any(abs(r) > 1e-9 for r in e.get("rotation", [0, 0, 0])):
            n["origin"] = [round(float(v), 4) for v in np.array(e["origin"]) + off]
        n["uuid"] = f"w_{idx}"
        doc["elements"].append(n)
        f, t = np.array(n["from"]), np.array(n["to"])
        rot = n.get("rotation", [0, 0, 0])
        corners = [np.array([x, y, z]) for x in (f[0], t[0]) for y in (f[1], t[1]) for z in (f[2], t[2])]
        Re = np.eye(3)
        if any(abs(r) > 1e-9 for r in rot):
            Re = wrk.rotmat(rot[2], 2) @ wrk.rotmat(rot[1], 1) @ wrk.rotmat(rot[0], 0)
            org = np.array(n["origin"], float)
            corners = [Re @ (c - org) + org for c in corners]
        axes = [Re @ np.eye(3)[k] for k in range(3)]
        for lo, hi in wrk.PLAYER_BOXES.values():
            if wrk.obb_aabb_intersect(corners, axes, lo, hi):
                collisions += 1
    doc["outliner"] = [{"name": "root", "origin": [0, 0, 0], "children": [e["uuid"] for e in doc["elements"]]}]
    return doc, collisions


def worn_panel(weapon_doc: dict, offset: tuple[float, float, float], out_dir: Path,
               weapon_len: float, title_b: str, views_b=None):
    """挂件的「并排比例」+「穿戴」两栏（5 格，宽度与持握版一致）。

    ``offset`` 是把模型原点（挂点）平移到玩家身上的位置；``views_b`` 是穿戴三格的
    [(标题, yaw, pitch), ...]，默认背面 / 左侧 / 3/4 背左斜。
    """
    import json

    import gear_kit
    import weapon_review_kit as wrk
    from bbmodel_maker.render.render_bbmodel import render
    from cjk_font import load_font
    from PIL import ImageDraw

    font_h, font_l = load_font(16), load_font(12)
    stand_bb, worn_bb = out_dir / "_tmp_stand.bbmodel", out_dir / "_tmp_worn.bbmodel"
    wrk.make_standing_bbmodel(weapon_doc, stand_bb)
    doc, collisions = worn_doc(weapon_doc, offset)
    worn_bb.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    views_b = views_b or [("穿戴: 背面 BACK (相机 +z)", 0.0, 0.0), ("穿戴: 左侧 LEFT (相机 -x)", 90.0, 0.0),
                          ("穿戴: 3/4 背左斜", 35.0, 20.0)]

    cell, pad = 380, 8
    shots_a = [render(stand_bb, yaw=yaw, pitch=0.0, size=cell, bg=gear_kit.BG)[0] for yaw in (180.0, -90.0)]
    shots_b = [render(worn_bb, yaw=y, pitch=p, size=cell, bg=gear_kit.BG)[0] for _, y, p in views_b]
    stand_bb.unlink(missing_ok=True)
    worn_bb.unlink(missing_ok=True)

    panel = Image.new("RGB", (cell * 5 + pad * 6, cell + 68), (28, 29, 33))
    d = ImageDraw.Draw(panel)
    d.text((pad, 6), f"(A) 并排比例 · 挂件全长 {weapon_len:.1f} px，玩家高 32 px · 立在玩家右侧，间隙 2 px，底端对齐脚底",
           fill=(240, 225, 175), font=font_h)
    col = "0 处交叠 (体积严格为 0)" if collisions == 0 else f"{collisions} 处交叠"
    d.text((pad + cell * 2 + pad * 2, 6), f"(B) {title_b} · 几何相交: {col}" + (" ✓" if collisions == 0 else " ✗"),
           fill=(150, 235, 180) if collisions == 0 else (240, 120, 100), font=font_h)
    titles_a = ("并排: 正面 FRONT", "并排: 侧面 SIDE")
    titles_b = [v[0] for v in views_b]
    for i, (t, im) in enumerate(zip(titles_a, shots_a)):
        x = pad + i * (cell + pad)
        d.rectangle([x, 32, x + cell - 1, 52], fill=(20, 22, 26))
        d.text((x + 6, 35), t, fill=(230, 230, 230), font=font_l)
        panel.paste(im, (x, 54))
    for i, (t, im) in enumerate(zip(titles_b, shots_b)):
        x = pad + cell * 2 + pad * 2 + i * (cell + pad)
        d.rectangle([x, 32, x + cell - 1, 52], fill=(20, 22, 26))
        d.text((x + 6, 35), t, fill=(230, 230, 230), font=font_l)
        panel.paste(im, (x, 54))
    return panel, collisions
