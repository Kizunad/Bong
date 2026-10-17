"""旧时金属器（青铜 / 铁）Blockbench .bbmodel 生成器的共用审阅工具，建在 ``gear/_kit/gear_kit.py`` 之上。

和末土装备的区别：

* 器物是地上摆件 / 手持小件，不是穿戴件，所以门禁去掉「穿进玩家身体」和「越出 0..16 方块」两道；
* 审阅图 ``check.png`` 多两栏，专治「比例凭感觉」：
  - 比例对照：参考图与模型同视角、整件对整件等高并排，叠一张半透明叠图，下面是各部件占总高 / 总宽的百分比表，
    差超过 ``PROPORTION_TOLERANCE_PP`` 个百分点标 ✗；
  - 并排比例：器物放在灰色占位玩家旁边（不重叠），标出器高 / 人高；
* 手持件再加「持握姿势」栏（见 ``render_hold_panel``）。

坐标约定与 gear_kit 相同：1 方块 = 16px，玩家高 32px，脚底 y=0，玩家面朝 -z，+x 是玩家右侧。
地上摆件的正面朝 -z，底面落在 y=0，足迹以原点为中心。
"""

from __future__ import annotations

import copy
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import BG, PLAYER_BOXES, PLAYER_MATS, REVIEW_ROOT, Gear, GearSpec, load_font  # noqa: E402

from bbmodel_maker.gates import gatekit  # noqa: E402
from bbmodel_maker.render.render_bbmodel import _rotmat, render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig, element_bounds  # noqa: E402

PROPORTION_TOLERANCE_PP = 10.0      # 部件占比与参考图相差超过这么多个百分点就算不合格
PLAYER_HEIGHT_PX = 32.0
PLAYER_GAP_PX = 2.0                 # 并排比例里器物与玩家之间留的空隙

TEXT = (235, 235, 235)
OK_COLOR = (140, 230, 160)
BAD_COLOR = (255, 130, 120)
PANEL_BG = (32, 33, 36)


# ───────────────────────────────────────────────────────────── 门禁与规格
class GroundGates(gatekit.AssetGates):
    """地上摆件 / 手持小件的门禁：去掉「越出 0..16 方块空间」（摆件按 geo 坐标，不受一格限制）。"""

    def specs(self):
        return tuple(s for s in super().specs() if s[0] != "overflow")


@dataclass
class MetalSpec(GearSpec):
    """金属器一件的规格：GearSpec 加上比例对照要用的参考图取景与部件分段。"""

    # 参考图里器物「整件」的像素外框 (x0, y0, x1, y1)：正视 / 侧视各一个
    ref_front_box: tuple = (0, 0, 1, 1)
    ref_side_box: tuple = (0, 0, 1, 1)
    # 高度分段：(标签, 参考图正视里该段的 y 起点 px, 终点 px, 模型里属于该段的件名前缀)
    height_parts: tuple = ()
    # 宽度分段：(视角 "front" / "side", 标签, 参考图里的 x 起点 px, 终点 px, 件名前缀)
    width_parts: tuple = ()
    # 比例对照里模型正视的相机朝向：180 = 从 -z 看（默认，正面）；0 = 从 +z 看（参考图正视是器物背面时用）
    front_yaw: float = 180.0
    # 手持件的持握姿势：None = 不出这一栏。{"arm_deg": 右臂绕肩前抬角度, "anchor": 器物局部坐标里要握住的点,
    #  "lead": 握点在拳心前方（沿臂轴）多远 px, "skip_arm": 与右臂允许交叠的件名前缀（握在手里的纽 / 绳）}
    hold: dict | None = None
    # 比例对照里模型侧视的相机朝向：-90 = 从 +x 看（默认），90 = 从 -x 看（参考图侧视脸朝左时用）
    side_yaw: float = -90.0
    # 比例对照 / 并排比例量总尺寸时不计入的件名前缀（只在手持时才用的绳环等，参考图里没有）
    bounds_skip: tuple = ()
    # 穿模门的咬入下限 px：整件最厚的件都薄于 0.55 时（镜 / 权的薄片）要调小，否则注入缺陷也报不出来
    interpen_bite: float = 0.55
    # 器物放在玩家旁边时的水平错开量 px（None = 按包围盒自动算）
    beside_dx: float | None = None

    def __post_init__(self) -> None:
        self.gates = GroundGates(self.title, self.mats, asym=self.asym,
                                 soft_over=self.soft_over, free_floating=self.free_floating,
                                 interpen_bite=self.interpen_bite)

    @property
    def review_dir(self) -> Path:
        return REVIEW_ROOT / "img" / "metalware" / self.gear_id


def run_gates(spec: MetalSpec, g: Gear) -> int:
    rig = gear_kit.gate_rig(spec, g)
    total = spec.gates.report(rig, px=16.0, note="（摆件按 geo 坐标，不受 16px 方块限制，已去掉越界一道）")
    cop = gear_kit.coplanar_violations(g)
    print(f"  {'✓' if not cop else '✗'} 共面 (Z-fighting): {len(cop)}")
    for line in cop[:20]:
        print(f"      - {line}")
    moving = gear_kit.moving_bone_count(g)
    ok = moving >= spec.min_bones
    print(f"  {'✓' if ok else '✗'} 骨骼数: 共 {len(g.rig.bones)}（含 root），带几何的可动骨 {moving}（下限 {spec.min_bones}）")
    return total + len(cop) + (0 if ok else 1)


# ───────────────────────────────────────────────────────────── 圆形截面的八边形近似（居中，关于 x=0 对称）
OCT = 0.41421356     # 正八边形边长 / 对边宽


def octagon_prism(g: Gear, bone: str, name: str, af: float, y0: float, y1: float, mat: str,
                  cx: float = 0.0, cz: float = 0.0) -> None:
    """居中的八棱柱（对边宽 af）：两块正条 + 两块 45° 斜条叠成正八边形。两块正条上下面错开 0.015px 防共面。"""
    h, e = af / 2, af * OCT / 2
    org = (cx, (y0 + y1) / 2, cz)
    g.box(bone, f"{name}_a", (cx - h, y0, cz - e), (cx + h, y1, cz + e), mat)
    g.box(bone, f"{name}_b", (cx - e, y0 + 0.015, cz - h), (cx + e, y1 - 0.015, cz + h), mat)
    g.box(bone, f"{name}_c", (cx - h, y0 + 0.01, cz - e), (cx + h, y1 - 0.01, cz + e), mat, rot=(0, 45, 0), org=org)
    g.box(bone, f"{name}_d", (cx - h, y0 + 0.01, cz - e), (cx + h, y1 - 0.01, cz + e), mat, rot=(0, -45, 0), org=org)


def octagon_ring(g: Gear, bone: str, name: str, af_out: float, t: float, y0: float, y1: float, mat: str) -> None:
    """居中的八边形环（外对边宽 af_out，壁厚 t）：前后两条居中、左右两条和四条斜条各成对镜像。"""
    L = af_out * OCT                      # 每条边的外沿长度，端头在外角上相接
    half = af_out / 2
    m = half - t / 2                      # 边中线到中心的距离
    g.box(bone, f"{name}_f", (-L / 2, y0, -half), (L / 2, y1, -half + t), mat)
    g.box(bone, f"{name}_b", (-L / 2, y0, half - t), (L / 2, y1, half), mat)
    g.pair(bone, f"{name}_s", (half - t, y0, -L / 2), (half, y1, L / 2), mat)
    c = m * 0.70710678
    ymid = (y0 + y1) / 2
    # Ry(θ) 把局部 x 轴转到 (cosθ, 0, -sinθ)：前右斜边切线 (1,0,1) → θ=-45；后右斜边切线 (1,0,-1) → θ=+45
    g.pair(bone, f"{name}_df", (c - L / 2, y0, -c - t / 2), (c + L / 2, y1, -c + t / 2), mat, rot=(0, -45, 0), org=(c, ymid, -c))
    g.pair(bone, f"{name}_db", (c - L / 2, y0, c - t / 2), (c + L / 2, y1, c + t / 2), mat, rot=(0, 45, 0), org=(c, ymid, c))


# ───────────────────────────────────────────────────────────── 量部件
def extent(g: Gear, prefixes: tuple[str, ...]):
    """件名以 prefixes 之一开头的所有立方体的（旋转后）包围盒 (lo, hi)。"""
    els = [e for e in g.rig.elements if e["name"].startswith(prefixes)]
    if not els:
        raise ValueError(f"没有件名以 {prefixes} 开头的立方体")
    return element_bounds(els)


def model_bounds(g: Gear, skip: tuple = ()):
    els = [e for e in g.rig.elements if not e["name"].startswith(skip)] if skip else g.rig.elements
    return element_bounds(els)


# ───────────────────────────────────────────────────────────── 渲染辅助
def _write_tmp(rig: Rig, path: Path, name: str) -> Path:
    path.write_text(json.dumps(rig.bbmodel(name), ensure_ascii=False), encoding="utf-8")
    return path


def render_view(path: Path, yaw: float, pitch: float, size: int, center, span: float,
                shading: str | None = None) -> Image.Image:
    return render(path, yaw=yaw, pitch=pitch, size=size, bg=BG, focus=(center, span),
                  **({"shading": shading} if shading else {}))[0]


def _screen_scale(size: int, span: float) -> float:
    return (size - 60) / span


def _dashed_hline(d: ImageDraw.ImageDraw, y: float, x0: float, x1: float, color, dash: int = 6) -> None:
    x = x0
    while x < x1:
        d.line([(x, y), (min(x + dash, x1), y)], fill=color, width=1)
        x += dash * 2


# ───────────────────────────────────────────────────────────── 栏一：比例对照
def _crop_ref(ref: Image.Image, box: tuple, height_px: int) -> Image.Image:
    crop = ref.crop(box)
    return crop.resize((round(crop.width * height_px / crop.height), height_px), Image.Resampling.LANCZOS)


def _model_silhouette_crop(path: Path, yaw: float, g: Gear, height_px: int, size: int = 900, skip: tuple = ()):
    """模型某视角的整件正交渲染，裁到器物外框，缩放到 height_px 高。返回 (图, 外框 lo/hi 的米制说明)。"""
    lo, hi = model_bounds(g, skip)
    center = ((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2)
    span = max(hi[i] - lo[i] for i in range(3)) * 1.06
    img = render_view(path, yaw, 0.0, size, center, span)
    scale = _screen_scale(size, span)
    horiz = (hi[0] - lo[0]) if yaw in (180.0, 0.0) else (hi[2] - lo[2])
    vert = hi[1] - lo[1]
    box = (round(size / 2 - horiz / 2 * scale), round(size / 2 - vert / 2 * scale),
           round(size / 2 + horiz / 2 * scale), round(size / 2 + vert / 2 * scale))
    crop = img.crop(box)
    return crop.resize((round(crop.width * height_px / crop.height), height_px), Image.Resampling.LANCZOS)


def _overlay(ref_img: Image.Image, model_img: Image.Image) -> Image.Image:
    """参考图与模型等高、底边与水平中心对齐，模型按与背景的差异抠出来半透明叠在参考图上。"""
    w = max(ref_img.width, model_img.width)
    h = ref_img.height
    out = Image.new("RGB", (w, h), BG)
    out.paste(ref_img, ((w - ref_img.width) // 2, 0))
    mask = Image.new("L", model_img.size, 0)
    mp, kp = model_img.load(), mask.load()
    for y in range(model_img.height):
        for x in range(model_img.width):
            if sum(abs(mp[x, y][i] - BG[i]) for i in range(3)) > 24:
                kp[x, y] = 150
    tint = Image.new("RGB", model_img.size, (255, 90, 160))
    out.paste(Image.blend(model_img, tint, 0.35), ((w - model_img.width) // 2, 0), mask)
    return out


def _pct_rows(spec: MetalSpec, g: Gear) -> list[tuple[str, float, float, bool]]:
    """(标签, 参考占比 %, 模型占比 %, 是否超差) —— 高度分段 + 宽度分段。"""
    rows = []
    fx0, fy0, fx1, fy1 = spec.ref_front_box
    ref_h = fy1 - fy0
    lo, hi = model_bounds(g, spec.bounds_skip)
    model_h = hi[1] - lo[1]
    for label, y0, y1, prefixes in spec.height_parts:
        elo, ehi = extent(g, prefixes)
        ref_pct = (y1 - y0) / ref_h * 100
        model_pct = (ehi[1] - elo[1]) / model_h * 100
        rows.append((f"高 {label}", ref_pct, model_pct, abs(ref_pct - model_pct) > PROPORTION_TOLERANCE_PP))
    for view, label, x0, x1, prefixes in spec.width_parts:
        box = spec.ref_front_box if view == "front" else spec.ref_side_box
        axis = 0 if view == "front" else 2
        ref_w = box[2] - box[0]
        model_w = hi[axis] - lo[axis]
        elo, ehi = extent(g, prefixes)
        ref_pct = (x1 - x0) / ref_w * 100
        model_pct = (ehi[axis] - elo[axis]) / model_w * 100
        name = "宽" if view == "front" else "深"
        rows.append((f"{name} {label}", ref_pct, model_pct, abs(ref_pct - model_pct) > PROPORTION_TOLERANCE_PP))
    return rows


def render_proportion_panel(spec: MetalSpec, g: Gear, tmp: Path, height_px: int = 400) -> tuple[Image.Image, int]:
    """比例对照栏：正视 / 侧视各一行「参考图 | 模型 | 叠图」，下面是占比表。返回 (图, 超差行数)。"""
    ref = Image.open(spec.review_dir / f"{spec.gear_id}_ref.png").convert("RGB")
    model_path = _write_tmp(g.rig, tmp, spec.model_name)
    font_h, font_s = load_font(17), load_font(13)

    rows_img = []
    for view, box, yaw in (("FRONT 正视", spec.ref_front_box, spec.front_yaw), ("SIDE 侧视", spec.ref_side_box, spec.side_yaw)):
        ref_img = _crop_ref(ref, box, height_px)
        model_img = _model_silhouette_crop(model_path, yaw, g, height_px, skip=spec.bounds_skip)
        rows_img.append((view, ref_img, model_img, _overlay(ref_img, model_img)))

    pct = _pct_rows(spec, g)
    bad = sum(1 for r in pct if r[3])
    cell_w = max(max(r[1].width, r[2].width, r[3].width) for r in rows_img) + 8
    width = cell_w * 3 + 16
    line_h = 17
    table_h = 30 + line_h * (len(pct) + 1)
    panel = Image.new("RGB", (width, 34 + (height_px + 30) * 2 + table_h), PANEL_BG)
    d = ImageDraw.Draw(panel)
    d.text((8, 6), "比例对照：参考图 vs 模型，同视角整件对整件等高（粉色 = 模型叠在参考图上）", fill=TEXT, font=font_h)
    y = 34
    for view, ref_img, model_img, over in rows_img:
        for k, (title, img) in enumerate((("参考图", ref_img), ("模型", model_img), ("叠图", over))):
            x = 8 + k * cell_w
            d.text((x, y), f"{view} · {title}", fill=(200, 200, 200), font=font_s)
            panel.paste(img, (x, y + 20))
        y += height_px + 30
    d.text((8, y), "各部件占总高 / 总宽（总高 = 整件高；总宽 / 总深 = 该视角整件宽）", fill=TEXT, font=font_s)
    y += line_h + 4
    d.text((8, y), f"{'部件':<14}{'参考 %':>8}{'模型 %':>8}{'差 pp':>8}", fill=(190, 190, 200), font=font_s)
    for label, rp, mp, flag in pct:
        y += line_h
        color = BAD_COLOR if flag else OK_COLOR
        d.text((8, y), f"{label:<14}{rp:>8.1f}{mp:>8.1f}{mp - rp:>+8.1f}  {'✗ 超 10pp' if flag else '✓'}", fill=color, font=font_s)
    return panel, bad


# ───────────────────────────────────────────────────────────── 栏二：并排比例
def player_with_item(g: Gear, shift: tuple, arm_deg: float | None = None) -> Rig:
    """器物（整体平移 shift）+ 灰色占位玩家，合成一个 Rig，只用于渲染；arm_deg 给出时右臂绕肩 (6,22,0) 前抬。"""
    mats = {**g.rig.mats, **PLAYER_MATS}
    w = Rig(mats, tex=g.rig.tex, swatch=g.rig.swatch)
    for bone in g.rig.bone_order:
        w.bones[bone] = copy.deepcopy(g.rig.bones[bone])
        w.bone_order.append(bone)
    w.elements = copy.deepcopy(g.rig.elements)
    for el in w.elements:
        for key in ("from", "to", "origin"):
            if el.get(key):
                for a in range(3):
                    el[key][a] += shift[a]
    w.bone("wearer", (0.0, 0.0, 0.0))
    inset = 0.03
    for label, (lo, hi) in PLAYER_BOXES.items():
        mat = "wearer_body" if label in ("头", "躯干") else "wearer_limb"
        rot = (arm_deg, 0.0, 0.0) if (arm_deg is not None and label == "右臂") else None
        org = SHOULDER if rot else None
        w.cube("wearer", f"player_{label}", tuple(v + inset for v in lo), tuple(v - inset for v in hi),
               mat=mat, rot=rot, org=org)
    for sx in (-2.0, 1.0):
        w.cube("wearer", "player_eye", (sx, 27.0, -4.1), (sx + 1.0, 28.0, -4.0 + inset), mat="wearer_face")
    return w


def render_beside_panel(spec: MetalSpec, g: Gear, tmp: Path, cell: int = 520) -> Image.Image:
    """并排比例栏：器物在玩家右手边（间隔 2px，不重叠），正面 + 3/4 两张，标器高 / 人高。"""
    lo, hi = model_bounds(g, spec.bounds_skip)
    half = (hi[0] - lo[0]) / 2
    dx = spec.beside_dx if spec.beside_dx is not None else 8.0 + PLAYER_GAP_PX + half - (lo[0] + hi[0]) / 2
    rig = player_with_item(g, (dx, 0.0, 0.0))
    path = _write_tmp(rig, tmp, "Beside")
    obj_h = hi[1] - lo[1]
    x_min, x_max = -8.0, hi[0] + dx
    center = ((x_min + x_max) / 2, PLAYER_HEIGHT_PX / 2, 0.0)
    span = max(x_max - x_min, PLAYER_HEIGHT_PX) * 1.08
    scale = _screen_scale(cell, span)
    font_h, font_s = load_font(17), load_font(13)
    panel = Image.new("RGB", (cell * 2 + 15, cell + 62), PANEL_BG)
    d = ImageDraw.Draw(panel)
    ratio = obj_h / PLAYER_HEIGHT_PX * 100
    d.text((8, 6), f"并排比例：器高 {obj_h:.1f}px / 人高 {PLAYER_HEIGHT_PX:.0f}px = {ratio:.0f}%；"
                   f"器宽 {hi[0] - lo[0]:.1f}px × 深 {hi[2] - lo[2]:.1f}px，与玩家间隔 {PLAYER_GAP_PX:.0f}px 不重叠",
           fill=TEXT, font=font_h)
    for k, (label, yaw, pitch) in enumerate((("FRONT 正面（相机在 -z）", 180.0, 0.0), ("3/4 右前上方", -150.0, 14.0))):
        x = 5 + k * (cell + 5)
        d.rectangle([x, 32, x + cell - 1, 53], fill=(22, 23, 26))
        d.text((x + 6, 35), label, fill=TEXT, font=font_s)
        shot = render_view(path, yaw, pitch, cell, center, span)
        panel.paste(shot, (x, 56))
        if pitch == 0.0:
            # 正面是正交投影且 pitch=0：屏幕 y 与世界 y 线性对应，可画器高线与人高线
            ground = 56 + cell / 2 + PLAYER_HEIGHT_PX / 2 * scale
            for h, color, text in ((PLAYER_HEIGHT_PX, (255, 220, 120), "人高 32px"),
                                   (obj_h, (120, 230, 255), f"器高 {obj_h:.1f}px")):
                yy = ground - h * scale
                _dashed_hline(d, yy, x, x + cell, color)
                d.text((x + 4, yy - 15), text, fill=color, font=font_s)
    return panel


# ───────────────────────────────────────────────────────────── 栏三：持握姿势（手持件）
SHOULDER = (6.0, 22.0, 0.0)          # 右肩（臂绕它转）
FIST_REST = (6.0, 13.5, 0.0)         # 手臂垂下时的拳心
OVERLAP_EPS = 1e-4


def _euler_matrix(rot) -> np.ndarray:
    rot = rot or (0.0, 0.0, 0.0)
    return _rotmat(rot[2], 2) @ _rotmat(rot[1], 1) @ _rotmat(rot[0], 0)


def _obb(lo, hi, rot=None, org=None, shift=(0.0, 0.0, 0.0)):
    """一个（可旋转的）盒的 (8 个角, 3 条轴)。org 缺省 = 盒心；shift 在旋转之后叠加。"""
    lo, hi = np.array(lo, float), np.array(hi, float)
    org = (lo + hi) / 2 if org is None else np.array(org, float)
    R = _euler_matrix(rot)
    corners = np.array([[x, y, z] for x in (lo[0], hi[0]) for y in (lo[1], hi[1]) for z in (lo[2], hi[2])])
    corners = (R @ (corners - org).T).T + org + np.array(shift, float)
    return corners, [R[:, 0], R[:, 1], R[:, 2]]


def obb_overlap_depth(a, b) -> float:
    """两个 OBB 的分离轴检验：存在分离轴（含刚好贴面）返回 0，否则返回各轴重叠深度的最小值（> 0 = 体积交叠）。"""
    (ca, aa), (cb, ab) = a, b
    axes = list(aa) + list(ab)
    for u in aa:
        for v in ab:
            n = np.cross(u, v)
            if np.linalg.norm(n) > 1e-6:
                axes.append(n / np.linalg.norm(n))
    depth = math.inf
    for axis in axes:
        pa, pb = ca @ axis, cb @ axis
        overlap = min(pa.max(), pb.max()) - max(pa.min(), pb.min())
        if overlap <= OVERLAP_EPS:
            return 0.0
        depth = min(depth, overlap)
    return depth


def hold_transform(spec: MetalSpec) -> tuple[tuple, np.ndarray, np.ndarray]:
    """(器物平移量, 拳心, 臂轴单位向量)：握点放在拳心沿臂轴往前 lead px 处。"""
    cfg = spec.hold
    R = _euler_matrix((cfg["arm_deg"], 0.0, 0.0))
    fist = R @ (np.array(FIST_REST) - np.array(SHOULDER)) + np.array(SHOULDER)
    axis = R @ np.array([0.0, -1.0, 0.0])
    target = fist + axis * cfg["lead"]
    shift = tuple(target - np.array(cfg["anchor"], float))
    return shift, fist, axis


def hold_elements(spec: MetalSpec, g: Gear) -> list[dict]:
    """持握姿势里实际拿在手上的件（hold["only"] 给出时只取这些前缀的件）。"""
    only = tuple(spec.hold.get("only", ()))
    return [e for e in g.rig.elements if not only or e["name"].startswith(only)]


def hold_overlaps(spec: MetalSpec, g: Gear) -> tuple[dict, dict]:
    """器物每个立方体与玩家头 / 躯干 / 左臂 / 双腿、以及抬起的右臂的交叠。

    返回 (与身体各部分交叠的件数, 与右臂交叠的件名列表)；握在手里的件（skip_arm 前缀）不查右臂。
    """
    cfg = spec.hold
    shift, _, _ = hold_transform(spec)
    body = {k: _obb(*PLAYER_BOXES[k]) for k in ("头", "躯干", "左臂", "右腿", "左腿")}
    arm = _obb(*PLAYER_BOXES["右臂"], rot=(cfg["arm_deg"], 0.0, 0.0), org=SHOULDER)
    hits: dict[str, list] = {k: [] for k in body}
    arm_hits = []
    for el in hold_elements(spec, g):
        box = _obb(el["from"], el["to"], el.get("rotation"), el.get("origin"), shift)
        for label, other in body.items():
            if obb_overlap_depth(box, other) > 0:
                hits[label].append(el["name"])
        if not el["name"].startswith(tuple(cfg.get("skip_arm", ()))) and obb_overlap_depth(box, arm) > 0:
            arm_hits.append(el["name"])
    return hits, {"右臂": arm_hits}


def render_hold_panel(spec: MetalSpec, g: Gear, tmp: Path, cell: int = 520) -> Image.Image:
    """持握姿势栏：右臂整体前抬，握点在拳前；FRONT / SIDE / 3/4 三张，并写出与身体各部分的交叠体积检查。"""
    cfg = spec.hold
    shift, fist, _ = hold_transform(spec)
    held = copy.copy(g)
    held.rig = copy.copy(g.rig)
    held.rig.elements = hold_elements(spec, g)
    rig = player_with_item(held, shift, cfg["arm_deg"])
    path = _write_tmp(rig, tmp, "Hold")
    hits, arm_hits = hold_overlaps(spec, g)
    bad = sum(len(v) for v in hits.values()) + len(arm_hits["右臂"])
    center, span = (4.0, 20.0, -4.0), 34.0
    font_h, font_s = load_font(17), load_font(13)
    views = (("FRONT 正面（相机在 -z）", 180.0, 0.0), ("RIGHT 右侧（相机在 +x）", -90.0, 0.0), ("3/4 右前上方", -150.0, 14.0))
    panel = Image.new("RGB", (cell * 3 + 20, cell + 90), PANEL_BG)
    d = ImageDraw.Draw(panel)
    d.text((8, 6), f"持握姿势：右臂绕肩前抬 {cfg['arm_deg']:.0f}°（整臂抬起），握点在拳心前 {cfg['lead']:.2f}px；"
                   f"拳心 ({fist[0]:.1f}, {fist[1]:.1f}, {fist[2]:.1f})", fill=TEXT, font=font_h)
    checked = f"逐块 OBB 分离轴检查 {len(held.rig.elements)} 块 × 头 / 躯干 / 左臂 / 右腿 / 左腿 + 抬起的右臂（握在手里的 {', '.join(cfg.get('skip_arm', ()))} 不查右臂）"
    color = OK_COLOR if bad == 0 else BAD_COLOR
    verdict = "交叠体积 0 ✓" if bad == 0 else f"✗ 有 {bad} 处交叠：" + "; ".join(f"{k}:{len(v)}" for k, v in {**hits, **arm_hits}.items() if v)
    d.text((8, 30), f"{checked} → {verdict}", fill=color, font=font_s)
    for k, (label, yaw, pitch) in enumerate(views):
        x = 5 + k * (cell + 5)
        d.rectangle([x, 54, x + cell - 1, 75], fill=(22, 23, 26))
        d.text((x + 6, 57), label, fill=TEXT, font=font_s)
        panel.paste(render_view(path, yaw, pitch, cell, center, span), (x, 78))
    print(f"  持握姿势：{verdict}")
    return panel


# ───────────────────────────────────────────────────────────── 栏四：六视图 / 骨骼树 / 总装
SIX_VIEWS = (
    ("FRONT 正面（相机在 -z）", 180.0, 0.0),
    ("3/4 右前上方", -150.0, 18.0),
    ("RIGHT 右侧（相机在 +x）", -90.0, 0.0),
    ("3/4 左后上方", 40.0, 18.0),
    ("3/4 俯视内腔", -165.0, 52.0),
    ("TOP 俯视（脸朝画面下方）", 180.0, 89.9),
)


def render_six(spec: MetalSpec, g: Gear, model_path: Path, title: str, cell: int = 340) -> Image.Image:
    lo, hi = model_bounds(g)
    center = tuple((lo[i] + hi[i]) / 2 for i in range(3))
    span = max(hi[i] - lo[i] for i in range(3)) * 1.18
    font_h, font_l = load_font(17), load_font(12)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 70), PANEL_BG)
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), title, fill=TEXT, font=font_h)
    for i, (label, yaw, pitch) in enumerate(SIX_VIEWS):
        x, y = 5 + (i % 3) * (cell + 5), 32 + (i // 3) * (cell + 28)
        sheet.paste(render_view(model_path, yaw, pitch, cell, center, span), (x, y + 22))
        d.rectangle([x, y, x + cell - 1, y + 21], fill=(22, 23, 26))
        d.text((x + 6, y + 3), label, fill=TEXT, font=font_l)
    return sheet


def make_check(spec: MetalSpec, g: Gear, model_path: Path, gate_total: int, extra_panels: tuple = ()) -> int:
    """汇总 check.png：参考图 + 爆炸图 | 六视图 + 比例对照 + 并排比例（+ 持握姿势）| 骨骼树。返回比例超差行数。"""
    rd = spec.review_dir
    font_h = load_font(18)
    tmp = spec.out_dir / "_tmp_review.bbmodel"
    try:
        six = render_six(spec, g, model_path,
                         f"{spec.title} {spec.round_label} · {g.total()} cubes · {len(g.rig.bones)} 骨"
                         f"（可动 {gear_kit.moving_bone_count(g)}）· 门禁违例 {gate_total}")
        prop, bad = render_proportion_panel(spec, g, tmp)
        beside = render_beside_panel(spec, g, tmp)
    finally:
        tmp.unlink(missing_ok=True)
    tree = gear_kit.bone_tree_panel(g, spec.min_bones, six.height)

    refs = []
    for kind, label in (("ref", "参考图（生图，gen_metal_ref.py）"), ("exploded", "爆炸分解图")):
        path = rd / f"{spec.gear_id}_{kind}.png"
        refs.append((label, Image.open(path).convert("RGB") if path.exists() else None))
    lw = 640
    scaled = [(lab, im.resize((lw, round(im.height * lw / im.width)), Image.Resampling.LANCZOS) if im else None)
              for lab, im in refs]
    ref_h = sum((im.height if im else 200) + 40 for _, im in scaled)

    mid_w = max(six.width, prop.width, beside.width, *(p.width for _, p in extra_panels))
    mid_h = six.height + 10 + prop.height + 10 + beside.height + sum(p.height + 10 for _, p in extra_panels)
    total_h = max(ref_h + 20, mid_h + 50)
    canvas = Image.new("RGB", (lw + mid_w + tree.width + 50, total_h), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    y = 8
    for lab, im in scaled:
        d.text((10, y), lab, fill=(215, 205, 180), font=font_h)
        if im:
            canvas.paste(im, (10, y + 30))
        y += (im.height if im else 200) + 40
    x = lw + 25
    d.text((x, 8), "本模型（几何平涂版；无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    yy = 38
    for panel in (six, prop, beside, *(p for _, p in extra_panels)):
        canvas.paste(panel, (x, yy))
        yy += panel.height + 10
    x += mid_w + 15
    d.text((x, 8), f"骨骼树（{len(g.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    rd.mkdir(parents=True, exist_ok=True)
    out = rd / "check.png"
    canvas.save(out)
    print(f"✓ check.png → {out} ({canvas.width}×{canvas.height})；比例超差 {bad} 行")
    return bad


# ───────────────────────────────────────────────────────────── Round 3：贴图审阅图 check_tex.png
TEX_VIEWS = (
    ("FRONT 正面（-z）", 180.0, 0.0),
    ("3/4 右前上方", -150.0, 18.0),
    ("RIGHT 右侧（+x）", -90.0, 0.0),
    ("3/4 左后上方", 40.0, 18.0),
)


def count_magenta(img: Image.Image) -> int:
    """渲染图里的洋红像素数（缺贴图 / UV 越界的底色；R>200、B>200、G<80）。要求 0。"""
    px = img.convert("RGB").load()
    return sum(1 for y in range(img.height) for x in range(img.width)
               if px[x, y][0] > 200 and px[x, y][2] > 200 and px[x, y][1] < 80)


def render_check_tex(spec: MetalSpec, g: Gear, tmp: Path) -> tuple[Image.Image, int]:
    """check_tex.png：上排 参考图 + 4 个贴图视角（mc 光照）；中排 同视角平色；下排 并排比例（贴图版）+ 贴图特写。

    同时写出交付物 <Model>_tex.bbmodel（每面独立 UV、2 texel / px 的像素贴图），并统计洋红像素。
    """
    flat = gear_kit.write_textured(spec, g, spec.out_dir / "_tmp_flat.bbmodel", flat=True)
    tex = gear_kit.write_textured(spec, g, spec.out_dir / f"{spec.model_name}_tex.bbmodel")
    try:
        lo, hi = model_bounds(g, spec.bounds_skip)
        center = tuple((lo[i] + hi[i]) / 2 for i in range(3))
        span = max(hi[i] - lo[i] for i in range(3)) * 1.18
        cell = 440
        font_h, font_s = load_font(17), load_font(13)
        ref = Image.open(spec.review_dir / f"{spec.gear_id}_ref.png").convert("RGB")
        ref = ref.resize((round(ref.width * cell / ref.height), cell), Image.Resampling.LANCZOS)
        magenta = 0
        shots = []
        for label, yaw, pitch in TEX_VIEWS:
            t_img = render_view(tex, yaw, pitch, cell, center, span, shading="mc")
            f_img = render_view(flat, yaw, pitch, cell, center, span)
            magenta += count_magenta(t_img)
            shots.append((label, t_img, f_img))
        # 贴图特写：正面高倍取景，看锈斑是不是「成片」、有没有碎噪点
        close_span = span * 0.42
        close = render_view(tex, 180.0, 0.0, 700, center, close_span, shading="mc")
        magenta += count_magenta(close)
        # 并排比例（贴图版）
        half = (hi[0] - lo[0]) / 2
        dx = spec.beside_dx if spec.beside_dx is not None else 8.0 + PLAYER_GAP_PX + half - (lo[0] + hi[0]) / 2
        rig = player_with_item(g, (dx, 0.0, 0.0))
        doc = gear_kit.textured_rig_doc(rig, "Beside", {**spec.texture_material(), **gear_kit.TEXTURE_WEARER})
        beside_path = tmp
        beside_path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
        x_min, x_max = -8.0, hi[0] + dx
        bcenter = ((x_min + x_max) / 2, PLAYER_HEIGHT_PX / 2, 0.0)
        bspan = max(x_max - x_min, PLAYER_HEIGHT_PX) * 1.08
        beside = [render_view(beside_path, yaw, pitch, cell, bcenter, bspan, shading="mc")
                  for yaw, pitch in ((180.0, 0.0), (-150.0, 14.0))]
        magenta += sum(count_magenta(b) for b in beside)
    finally:
        (spec.out_dir / "_tmp_flat.bbmodel").unlink(missing_ok=True)

    w = ref.width + 4 * (cell + 6) + 12
    obj_h = hi[1] - lo[1]
    canvas = Image.new("RGB", (w, cell * 2 + 140 + 700 + 40), (28, 29, 33))
    d = ImageDraw.Draw(canvas)
    d.text((8, 6), f"{spec.title} {spec.round_label} · 贴图版 · 洋红像素 {magenta}（要求 0）{'✓' if magenta == 0 else '✗'}",
           fill=OK_COLOR if magenta == 0 else BAD_COLOR, font=font_h)
    d.text((8, 34), "REFERENCE 参考图", fill=(215, 205, 180), font=font_s)
    canvas.paste(ref, (8, 54))
    x = ref.width + 14
    for label, t_img, f_img in shots:
        d.text((x, 34), f"TEXTURED 贴图 · {label}", fill=TEXT, font=font_s)
        canvas.paste(t_img, (x, 54))
        canvas.paste(f_img, (x, 54 + cell + 26))
        d.text((x, 54 + cell + 8), f"FLAT 平色 · {label}", fill=(190, 190, 200), font=font_s)
        x += cell + 6
    y = 54 + (cell + 26) * 2
    d.text((8, y), f"并排比例（贴图版）：器高 {obj_h:.1f}px / 人高 {PLAYER_HEIGHT_PX:.0f}px = {obj_h / PLAYER_HEIGHT_PX * 100:.0f}%，"
                   f"与玩家间隔 {PLAYER_GAP_PX:.0f}px 不重叠", fill=TEXT, font=font_s)
    canvas.paste(beside[0], (8, y + 20))
    canvas.paste(beside[1], (8 + cell + 6, y + 20))
    d.text((2 * cell + 40, y), "贴图特写（正面，约 2.4 倍）：看锈是不是成片、阵纹是不是暗灰", fill=TEXT, font=font_s)
    canvas.paste(close, (2 * cell + 40, y + 20))
    d_h = y + 20 + max(cell, 700) + 8
    canvas = canvas.crop((0, 0, w, min(canvas.height, d_h)))
    print(f"✓ {tex.name}；洋红像素 {magenta}")
    return canvas, magenta


def full_run(spec: MetalSpec, extra_panel_fns: tuple = ()) -> None:
    """全量（Round 1 只做几何）：自证 → 全套门禁 → 写 bbmodel → check.png。"""
    g = spec.build()
    gear_kit.self_test(spec, g)
    total = run_gates(spec, g)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = spec.out_dir / f"{spec.model_name}.bbmodel"
    gear_kit.write_model(g, model, spec.model_name)
    print(f"✓ {model}: {g.total()} cubes / {len(g.rig.bones)} bones")
    tmp = spec.out_dir / "_tmp_extra.bbmodel"
    if spec.hold is not None:
        extra_panel_fns = (("hold", render_hold_panel),) + tuple(extra_panel_fns)
    try:
        extras = tuple((name, fn(spec, g, tmp)) for name, fn in extra_panel_fns)
    finally:
        tmp.unlink(missing_ok=True)
    make_check(spec, g, model, total, extras)
    if spec.texture_overrides:
        tmp_tex = spec.out_dir / "_tmp_tex_beside.bbmodel"
        try:
            sheet, magenta = render_check_tex(spec, g, tmp_tex)
        finally:
            tmp_tex.unlink(missing_ok=True)
        out = spec.review_dir / "check_tex.png"
        sheet.save(out)
        print(f"✓ check_tex.png → {out} ({sheet.width}×{sheet.height})")
        if magenta:
            raise SystemExit(f"贴图审阅图里有 {magenta} 个洋红像素（缺贴图 / UV 越界）")


def main(spec: MetalSpec, extra_panel_fns: tuple = ()) -> None:
    import argparse
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--table", action="store_true", help="只打印比例占比表（不渲染）")
    args = ap.parse_args()
    if args.self_test:
        gear_kit.self_test(spec, spec.build())
    elif args.gates:
        run_gates(spec, spec.build())
    elif args.table:
        g = spec.build()
        lo, hi = model_bounds(g)
        print(f"bbox {hi[0] - lo[0]:.2f} × {hi[1] - lo[1]:.2f} × {hi[2] - lo[2]:.2f}  cubes {g.total()}")
        for label, rp, mp, flag in _pct_rows(spec, g):
            print(f"{label:<14}{rp:>8.1f}{mp:>8.1f}{mp - rp:>+8.1f}  {'✗' if flag else '✓'}")
    else:
        full_run(spec, extra_panel_fns)
