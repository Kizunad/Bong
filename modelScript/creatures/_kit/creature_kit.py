#!/usr/bin/env python3
"""生物建模共用工具（过夜批次的腐沼蟾 / 藓衣蜗 / 残羽鸮共用，内容必须逐字相同，改动要同步到各分支）。

每个生物自己的生成器只写：材质表 MATS、骨架 BONES、各 part_*() 函数；下面这些通用件由本模块提供——

  Puppet        Rig 的薄封装（按 BONES 建骨 + 左右镜像成对建件）
  Field         **体素场**：所有 1px 体素格进同一张表（格 → 骨 / 材质），先到先得、一格只属于一件，
                所以件与件之间不会出现共面 / 重叠；只算右半（i ≥ 0），emit() 时沿 x 合并同色连续格并镜像出左半，
                左右严格对称。椭球 / 胶囊 / 盒子三种造型覆盖了体块、肢体、触须。
  gates(...)    几何门禁（孤儿 / 退化 / 悬空 / 穿模 / 镜像 / 共面 / 骨骼数下限），每道门带缺陷注入器自证
  sheets        六视角接触表、骨骼层级树、参考同取景、check.png
  write_textured  像素贴图版（保留真骨骼层级，供动画绑骨）

坐标：1 block = 16px，+y 向上，正面朝 -z（GeckoLib 约定），+x 为右侧（骨名 *_r）。
"""

from __future__ import annotations

import copy
import importlib.util
import json
import math
import sys
import uuid as uuidlib
from pathlib import Path
from typing import Callable, Sequence

from PIL import Image, ImageDraw

from bbmodel_maker.gates import gatekit
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

BG = (119, 119, 119)
Cell = tuple
MIN_COMPONENT = 12             # 面连通碎块小于这么多格就当碎屑删掉
MatFn = Callable[[int, int, int], str]

_spec = importlib.util.spec_from_file_location(
    "repo_coplanar", REPO / "modelScript" / "bbmodel_maker" / "gates" / "coplanar.py")
coplanar = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coplanar)


# ───────────────────────────────────────────────────────────── 木偶 / 体素场
class Puppet:
    """Rig 的薄封装：按 BONES 建骨，左右镜像成对建件。"""

    def __init__(self, mats, bones) -> None:
        self.rig = Rig(mats, tex=64, swatch=8)
        for name, parent, pivot, _ in bones:
            self.rig.bone(name, pivot, parent)

    def box(self, bone, name, f, t, mat, rot=None, org=None):
        return self.rig.cube(bone, name, f, t, mat=mat, rot=rot, org=org)

    def pair(self, bone, name, f, t, mat, sided=True):
        """f/t 按右侧（+x）坐标给，左侧自动镜像。sided=False 时两件都挂 bone（中线骨上的成对件）。"""
        self.rig.cube(f"{bone}_r" if sided else bone, f"{name}_r", f, t, mat=mat)
        self.rig.cube(f"{bone}_l" if sided else bone, f"{name}_l", (-t[0], f[1], f[2]), (-f[0], t[1], t[2]), mat=mat)

    def total(self):
        return len(self.rig.elements)


def hash_cell(a: int, b: int, c: int, salt: int = 0) -> int:
    """位置哈希 0..99：同一个 (a, b, c) 恒定；调用方自己决定用不用 abs()（要左右镜像一致就取 |x|）。"""
    return ((a * 73856093) ^ (b * 19349663) ^ (c * 83492791) ^ (salt * 2654435761)) % 100


def blotch(i: int, j: int, k: int, size: int, salt: int = 0) -> int:
    """size px 一块的斑块哈希（0..99）；斑块越大，同色格越容易沿 x 合并成长条。"""
    return hash_cell(abs(i) // size, j // size, k // size, salt)


def pick(h: int, weights: Sequence[tuple[str, int]]) -> str:
    """按权重表（材质, 百分比）把 0..99 的哈希值映射成材质。"""
    acc = 0
    for mat, w in weights:
        acc += w
        if h < acc:
            return mat
    return weights[-1][0]


class Field:
    """体素场（只算右半 i ≥ 0）。cells: {(i, j, k): (骨名, 材质)}。"""

    def __init__(self, puppet: Puppet) -> None:
        self.p = puppet
        self.cells: dict[Cell, tuple[str, str]] = {}
        self.bone_of: Callable[[str, int, int, int], str] = lambda bone, i, j, k: bone

    # ---- 造型
    def put(self, i: int, j: int, k: int, bone: str, mat: str, overwrite: bool = False) -> None:
        if i < 0:
            return
        if overwrite or (i, j, k) not in self.cells:
            self.cells[(i, j, k)] = (self.bone_of(bone, i, j, k), mat)

    def _shape(self, bone, inside, bounds, mat_fn, surface, clip, cut_mat, overwrite) -> int:
        """通用：在 bounds 内收集 inside() 为真的格；surface=True 只留露在外面的格。"""
        (x0, x1), (y0, y1), (z0, z1) = bounds
        keep = (lambda i, j, k: inside(i, j, k) and clip(i, j, k)) if clip else inside
        n = 0
        for i in range(max(0, x0), x1 + 1):
            for j in range(y0, y1 + 1):
                for k in range(z0, z1 + 1):
                    if not keep(i, j, k):
                        continue
                    exposed = not all(keep(i + d[0], j + d[1], k + d[2]) for d in
                                      ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)))
                    if surface and not exposed:
                        continue
                    on_cut = bool(clip) and cut_mat is not None and any(
                        inside(i + d[0], j + d[1], k + d[2]) and not clip(i + d[0], j + d[1], k + d[2])
                        for d in ((0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)))
                    self.put(i, j, k, bone, cut_mat if on_cut else mat_fn(i, j, k), overwrite)
                    n += 1
        return n

    def ellipsoid(self, bone, center, radii, mat_fn: MatFn, surface=True, clip=None, cut_mat=None, overwrite=False) -> int:
        """椭球：格中心到 center 的归一化距离 ≤ 1。clip(i,j,k) 为假的格被削掉（如只留上半 / 前半），
        被削出的切面格用 cut_mat（口腔内壁之类）。"""
        cx, cy, cz = center
        rx, ry, rz = radii

        def inside(i, j, k):
            return ((i + 0.5 - cx) / rx) ** 2 + ((j + 0.5 - cy) / ry) ** 2 + ((k + 0.5 - cz) / rz) ** 2 <= 1.0

        bounds = ((int(math.floor(cx - rx)) - 1, int(math.ceil(cx + rx)) + 1),
                  (int(math.floor(cy - ry)) - 1, int(math.ceil(cy + ry)) + 1),
                  (int(math.floor(cz - rz)) - 1, int(math.ceil(cz + rz)) + 1))
        return self._shape(bone, inside, bounds, mat_fn, surface, clip, cut_mat, overwrite)

    def capsule(self, bone, a, b, r: float, mat_fn: MatFn, r_end: float | None = None, surface=False, overwrite=False) -> int:
        """胶囊（线段 a→b 加半径 r，r_end 给出则半径沿线段渐变）：肢体 / 触须 / 趾。默认实心。"""
        ax, ay, az = a
        bx, by, bz = b
        dx, dy, dz = bx - ax, by - ay, bz - az
        length2 = dx * dx + dy * dy + dz * dz or 1e-9
        r1 = r if r_end is None else r_end

        def inside(i, j, k):
            px, py, pz = i + 0.5, j + 0.5, k + 0.5
            t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy + (pz - az) * dz) / length2))
            rr = r + (r1 - r) * t
            return math.dist((px, py, pz), (ax + dx * t, ay + dy * t, az + dz * t)) <= rr

        reach = max(r, r1) + 1
        bounds = ((int(math.floor(min(ax, bx) - reach)), int(math.ceil(max(ax, bx) + reach))),
                  (int(math.floor(min(ay, by) - reach)), int(math.ceil(max(ay, by) + reach))),
                  (int(math.floor(min(az, bz) - reach)), int(math.ceil(max(az, bz) + reach))))
        return self._shape(bone, inside, bounds, mat_fn, surface, None, None, overwrite)

    def box(self, bone, lo, hi, mat_fn: MatFn, overwrite=False) -> int:
        """轴对齐盒：格中心落在 [lo, hi] 内的格。"""
        n = 0
        for i in range(int(math.floor(lo[0])), int(math.ceil(hi[0])) + 1):
            for j in range(int(math.floor(lo[1])), int(math.ceil(hi[1])) + 1):
                for k in range(int(math.floor(lo[2])), int(math.ceil(hi[2])) + 1):
                    if lo[0] <= i + 0.5 <= hi[0] and lo[1] <= j + 0.5 <= hi[1] and lo[2] <= k + 0.5 <= hi[2]:
                        self.put(i, j, k, bone, mat_fn(i, j, k), overwrite)
                        n += 1
        return n

    def top_y(self, i: int, k: int, bones: Sequence[str] | None = None) -> int | None:
        """(i, k) 这一列上现有格的最高 j（可限定骨名前缀）；没有返回 None。"""
        best = None
        for (ci, cj, ck), (bone, _) in self.cells.items():
            if ci == i and ck == k and (bones is None or any(bone.startswith(b) for b in bones)):
                best = cj if best is None else max(best, cj)
        return best

    def recolor(self, predicate: Callable[[int, int, int, str, str], str | None]) -> None:
        """遍历现有格重涂色：predicate(i, j, k, bone, mat) 返回新材质或 None（不改）。"""
        for cell, (bone, mat) in list(self.cells.items()):
            new = predicate(cell[0], cell[1], cell[2], bone, mat)
            if new:
                self.cells[cell] = (bone, new)

    def prune_isolated(self) -> int:
        """删掉面连通的碎块：成团不足 MIN_COMPONENT 格的（孤格 / 只靠棱角相连的小团）——它们在悬空门里会判悬空。
        连通按 6 邻接算（右半坐标，x = 0 的格和自己的镜像天然相连）；够大的分离体（如被壳罩住的身体）保留。返回删掉的格数。"""
        unseen = set(self.cells)
        keep: set = set()
        while unseen:
            seed = unseen.pop()
            comp, stack = {seed}, [seed]
            while stack:
                i, j, k = stack.pop()
                for n in ((i + 1, j, k), (i - 1, j, k), (i, j + 1, k), (i, j - 1, k), (i, j, k + 1), (i, j, k - 1)):
                    if n in unseen:
                        unseen.discard(n)
                        comp.add(n)
                        stack.append(n)
            if len(comp) >= MIN_COMPONENT:
                keep |= comp
        removed = len(self.cells) - len(keep)
        self.cells = {c: v for c, v in self.cells.items() if c in keep}
        return removed

    # ---- 落成立方体
    def emit(self) -> int:
        """沿 x 合并同骨同材质的连续格成长条，右半直接落、左半镜像（骨名 *_r → *_l，中线骨不变）。返回立方体数（单侧）。"""
        self.prune_isolated()
        remaining = dict(self.cells)
        counters: dict[str, int] = {}
        emitted = 0
        for (i, j, k) in sorted(self.cells, key=lambda c: (c[2], c[1], c[0])):
            entry = remaining.get((i, j, k))
            if entry is None:
                continue
            i1 = i
            while remaining.get((i1 + 1, j, k)) == entry:
                i1 += 1
            for x in range(i, i1 + 1):
                del remaining[(x, j, k)]
            bone, mat = entry
            stem = bone[:-2] if bone.endswith("_r") else bone
            n = counters.get(stem, 0)
            counters[stem] = n + 1
            self.p.rig.cube(bone, f"{stem}_v{n:04d}_r", (i, j, k), (i1 + 1, j + 1, k + 1), mat=mat)
            left_bone = f"{stem}_l" if bone.endswith("_r") else bone
            self.p.rig.cube(left_bone, f"{stem}_v{n:04d}_l", (-i1 - 1, j, k), (-i, j + 1, k + 1), mat=mat)
            emitted += 1
        return emitted


# ───────────────────────────────────────────────────────────── 门禁
class EntityGates(gatekit.AssetGates):
    """实体模型的门禁：去掉「越出 0..16 方块空间」一道 —— 生物不是方块。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


def soft_pairs(*groups, cross=()):
    """软重叠材质对：组内两两互放行；cross 里的 (组A, 组B) 两组之间也放行。"""
    out = []
    for g in groups:
        out += [(a, b) for a in g for b in g if a != b]
    for ga, gb in cross:
        out += [(a, b) for a in ga for b in gb]
    return frozenset(frozenset(p) for p in out)


class Gates:
    """一只生物的全套门禁：gatekit 七道 + 共面 + 骨骼数下限，每道带缺陷注入器自证。"""

    def __init__(self, title, mats, asym, soft_over, min_bones, build_small, free_floating=frozenset()) -> None:
        self.mats = mats
        self.gates = EntityGates(title, mats, asym=asym, soft_over=soft_over, free_floating=free_floating)
        self.min_bones = min_bones
        self.build_small = build_small            # () -> 只含少数部件的 Puppet，骨骼数门的差分自证用

    def rig_for_gates(self, p: Puppet) -> Rig:
        """rigkit 把 color 写成「材质序号 % 8」，材质多于 8 种会撞号，门禁副本里按贴图色块改回完整序号。"""
        rig = copy.deepcopy(p.rig)
        per_row = rig.tex // rig.swatch
        for el in rig.elements:
            uv = el["faces"]["north"]["uv"]
            el["color"] = int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)
        self.gates.colors = {i: n for i, n in enumerate(list(self.mats))}
        return rig

    @staticmethod
    def coplanar_violations(p: Puppet) -> list[str]:
        return coplanar.check_coplanar_faces([e for e in p.rig.elements if not any(e["rotation"])])

    @staticmethod
    def moving_bones(p: Puppet) -> int:
        return sum(1 for b in p.rig.bones.values() if b["children"])

    def run(self, p: Puppet) -> int:
        total = self.gates.report(self.rig_for_gates(p), px=16.0, note="（实体不受 16px 方块空间限制，已去掉越界一道）")
        cop = self.coplanar_violations(p)
        print(f"  {'✓' if not cop else '✗'} 共面 (Z-fighting): {len(cop)}")
        for line in cop[:8]:
            print(f"      - {line}")
        moving = self.moving_bones(p)
        ok = moving >= self.min_bones
        print(f"  {'✓' if ok else '✗'} 骨骼数: 共 {len(p.rig.bones)}（含 root），带几何的可动骨 {moving}（下限 {self.min_bones}）")
        return total + len(cop) + (0 if ok else 1)

    def self_test(self, p: Puppet) -> None:
        """差分自证：干净模型须全绿，每道门注入缺陷须被抓到。"""
        broken = self.gates.self_test(self.rig_for_gates(p))
        clean = self.coplanar_violations(p)
        flat = [e for e in p.rig.elements if not any(e["rotation"])]
        twin = copy.deepcopy(flat[0])
        twin["name"] += "__twin"
        hits = coplanar.check_coplanar_faces(flat + [twin])
        if clean or not hits:
            broken += 1
            print(f"  ✗ 共面: 干净 {len(clean)} / 注入后 {len(hits)}（须 0 / >0）")
        else:
            print(f"  ✓ 共面: 干净 0 → 注入同位复制件后 {len(hits)}")
        small = self.build_small()
        if self.moving_bones(small) >= self.min_bones:
            broken += 1
            print("  ✗ 骨骼数门没有鉴别力")
        else:
            print(f"  ✓ 骨骼数: 完整 {self.moving_bones(p)} ≥ {self.min_bones}；残缺件 {self.moving_bones(small)} 被拦下")
        if broken:
            raise SystemExit(f"自证未通过：{broken} 道门失效")
        print("✓ 差分自证全绿")


# ───────────────────────────────────────────────────────────── bbmodel / 渲染 / 接触表
def write_model(p: Puppet, path: Path, name: str) -> None:
    path.write_text(json.dumps(p.rig.bbmodel(name), ensure_ascii=False, indent=1), encoding="utf-8")


def render_model(path: Path, yaw: float, pitch: float, size: int) -> Image.Image:
    return render(path, yaw=yaw, pitch=pitch, size=size, bg=BG)[0]


def render_part_previews(parts: dict, build: Callable, tmp: Path, out_dir: Path, name: str, views) -> None:
    """逐件单独预览：每个 part_*() 只放自己的立方体，出三视角。views = ((标签, yaw, pitch), ...)。"""
    out_dir.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(15), load_font(12)
    for idx, (key, fn) in enumerate(parts.items(), start=1):
        part = build([fn])
        write_model(part, tmp, f"{name}_{key}")
        w = 300
        sheet = Image.new("RGB", (w * len(views), w + 44), (32, 33, 36))
        d = ImageDraw.Draw(sheet)
        d.text((8, 5), f"part_{key}: {fn.__doc__.splitlines()[0]}  ({part.total()} cubes)", fill=(235, 235, 235), font=font_t)
        for n, (label, yaw, pitch) in enumerate(views):
            sheet.paste(render_model(tmp, yaw, pitch, w), (n * w, 24))
            d.text((n * w + 6, w + 26), label, fill=(200, 200, 200), font=font_s)
        sheet.save(out_dir / f"part_{idx:02d}_{key}.png")
    print(f"✓ 分件预览 {len(parts)} 张 → {out_dir}")


def render_six_views(model_path: Path, title: str, views, cell: int = 380) -> Image.Image:
    font_h, font_l = load_font(17), load_font(12)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 70), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), title, fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(views):
        x, y = 5 + (i % 3) * (cell + 5), 32 + (i // 3) * (cell + 28)
        sheet.paste(render_model(model_path, yaw, pitch, cell), (x, y + 22))
        d.rectangle([x, y, x + cell - 1, y + 21], fill=(22, 23, 26))
        d.text((x + 6, y + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def bone_tree_lines(p: Puppet, bones) -> list[str]:
    children: dict = {}
    for name, parent, _, _ in bones:
        children.setdefault(parent, []).append(name)
    desc = {name: d for name, _, _, d in bones}
    lines: list[str] = []

    def walk(name, prefix, last, root):
        n = len(p.rig.bones[name]["children"]) if name in p.rig.bones else 0
        branch = "" if root else ("└─ " if last else "├─ ")
        lines.append(f"{prefix}{branch}{name}  [{n}]  {desc[name]}")
        kids = children.get(name, [])
        for i, k in enumerate(kids):
            walk(k, prefix + ("" if root else ("   " if last else "│  ")), i == len(kids) - 1, False)

    walk("root", "", True, True)
    return lines


def bone_tree_panel(p: Puppet, bones, min_bones: int, mats: int, height: int, columns: int = 1) -> Image.Image:
    """骨骼层级树图：标题标骨骼数 / 立方体数，下面是缩进树；骨多时分栏。"""
    lines = bone_tree_lines(p, bones)
    font_h, font_s, font_t = load_font(17), load_font(13), load_font(12)
    step, col_w = 16, 640
    per_col = math.ceil(len(lines) / columns)
    w = col_w * columns
    h = max(height, 100 + step * per_col + 20)
    panel = Image.new("RGB", (w, h), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.rectangle([0, 0, w - 1, h - 1], outline=(70, 75, 85), width=2)
    moving = Gates.moving_bones(p)
    d.text((14, 8), "骨骼层级树 BONE HIERARCHY（[n] = 该骨自带的立方体数）", fill=(180, 235, 200), font=font_h)
    d.text((14, 34), f"总骨骼 {len(p.rig.bones)}（含 root）｜带几何的可动骨 {moving}｜任务卡下限 ≥ {min_bones} "
                     f"{'✓' if moving >= min_bones else '✗'}", fill=(240, 210, 150), font=font_s)
    d.text((14, 54), f"立方体 {p.total()}｜材质 {mats} 种", fill=(190, 210, 235), font=font_s)
    d.line([(12, 78), (w - 12, 78)], fill=(65, 70, 80), width=1)
    for n, ln in enumerate(lines):
        col, row = divmod(n, per_col)
        d.text((14 + col * col_w, 86 + row * step), ln, fill=(228, 228, 234), font=font_t)
    return panel


def figure_box(im: Image.Image, bg_tol: float = 38.0):
    """图里「不是背景」的包围盒：背景取四条边缘像素的中位色。"""
    import numpy as np

    a = np.asarray(im.convert("RGB"), dtype=float)
    edge = np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]])
    mask = np.abs(a - np.median(edge, axis=0)).sum(axis=2) > bg_tol
    ys, xs = np.where(mask)
    return int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1


def fit_height(im: Image.Image, h: int) -> Image.Image:
    box = figure_box(im)
    pad = int(0.03 * (box[3] - box[1]))
    crop = im.crop((max(0, box[0] - pad), max(0, box[1] - pad), min(im.width, box[2] + pad), min(im.height, box[3] + pad)))
    return crop.resize((max(1, round(crop.width * h / crop.height)), h), Image.Resampling.LANCZOS)


def same_framing_row(model_path: Path, ref_path: Path, ref_crop: tuple, views, h: int = 340) -> Image.Image:
    """参考图的一个裁剪（ref_crop = 占整图宽高的比例 (x0, y0, x1, y1)）与本模型的若干视角并排，各自裁到包围盒、缩到同高。
    第一格是参考，后面是模型各视角。"""
    font_h, font_l = load_font(17), load_font(12)
    ref = Image.open(ref_path).convert("RGB")
    x0, y0, x1, y1 = ref_crop
    ref_tile = fit_height(ref.crop((int(ref.width * x0), int(ref.height * y0), int(ref.width * x1), int(ref.height * y1))), h)
    tiles = [("参考图（裁到包围盒）", ref_tile)]
    for label, yaw, pitch in views:
        tiles.append((f"模型 · {label}", fit_height(render_model(model_path, yaw, pitch, 700), h)))
    canvas = Image.new("RGB", (sum(t.width + 12 for _, t in tiles) + 10, h + 56), (32, 33, 36))
    d = ImageDraw.Draw(canvas)
    d.text((10, 6), "同取景对照：左 参考，右 模型（裁到包围盒、同高）", fill=(235, 235, 235), font=font_h)
    x = 5
    for label, tile in tiles:
        d.text((x + 4, 32), label, fill=(215, 205, 180), font=font_l)
        canvas.paste(tile, (x, 50))
        x += tile.width + 12
    return canvas


def make_check(p: Puppet, bones, model_path: Path, review_dir: Path, name: str, title: str, views, min_bones: int,
               gate_total: int, ref_crop: tuple, framing_views, tree_columns: int = 1) -> None:
    """check.png：左 参考（整图）| 中 六视角 + 参考同取景 | 右 骨骼树；并单存 contact_sheet.png / bones_tree.png。"""
    ref_path = review_dir / f"{name}.png"
    font_h = load_font(18)
    six = render_six_views(model_path, f"{title} · {p.total()} cubes · {len(p.rig.bones)} 骨（可动 {Gates.moving_bones(p)}）· "
                                       f"门禁违例 {gate_total}", views)
    row = same_framing_row(model_path, ref_path, ref_crop, framing_views)
    tree = bone_tree_panel(p, bones, min_bones, len(p.rig.mats),
                           six.height + row.height + 10, tree_columns)
    six.save(review_dir / "contact_sheet.png")
    tree.save(review_dir / "bones_tree.png")
    ref = Image.open(ref_path).convert("RGB")
    lw = 640
    ref_s = ref.resize((lw, round(ref.height * lw / ref.width)), Image.Resampling.LANCZOS)
    mid_w = max(six.width, row.width)
    total_h = max(ref_s.height + 60, six.height + row.height + 70, tree.height + 44)
    canvas = Image.new("RGB", (lw + mid_w + tree.width + 50, total_h), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    d.text((10, 8), "参考图 + 爆炸图（gen_exploded_ref.py）", fill=(215, 205, 180), font=font_h)
    canvas.paste(ref_s, (10, 38))
    x = lw + 25
    d.text((x, 8), "本模型（几何 only：无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    canvas.paste(six, (x, 38))
    canvas.paste(row, (x, 38 + six.height + 10))
    x += mid_w + 15
    d.text((x, 8), f"骨骼树（{len(p.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    canvas.save(review_dir / "check.png")
    print(f"✓ check.png → {review_dir / 'check.png'} ({canvas.width}×{canvas.height})")


# ───────────────────────────────────────────────────────────── 像素贴图
def write_textured(p: Puppet, bones, mats, out_path: Path, model_id: str, texture_material: dict, flat: bool = False) -> Path:
    """每面独立 UV 的像素贴图版（flat=True 出平色对照）。write_textured_bbmodel 只按组平铺 outliner，
    这里按 bones 改回真骨骼层级（带枢轴、uuid），动画才绑得上。texture_material: 生成器材质名 → 画笔库材质名。"""
    sys.path.insert(0, str(REPO / "modelScript" / "generators"))
    sys.path.insert(0, str(REPO / "modelScript" / "generators" / "meridian_factory"))
    from handheld_texture import flat_override, handheld_override  # noqa: E402
    from pixel_texture import write_textured_bbmodel  # noqa: E402

    rig = p.rig
    names = list(mats)
    per_row = rig.tex // rig.swatch
    owner = {uid: bone for bone, b in rig.bones.items() for uid in b["children"]}
    cubes = []
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        material = names[int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)]
        cube = {"name": el["name"], "from": el["from"], "to": el["to"], "material": texture_material[material],
                "group": owner[el["uuid"]]}
        if any(el["rotation"]):
            cube["rotation"], cube["origin"] = el["rotation"], el["origin"]
        cubes.append(cube)
    write_textured_bbmodel(cubes, out_path, model_id, None, flat_override if flat else handheld_override)
    doc = json.loads(out_path.read_text(encoding="utf-8"))
    by_group = {g["name"]: g["children"] for g in doc["outliner"]}
    nodes = {name: {"name": name, "uuid": str(uuidlib.uuid4()), "origin": list(pivot), "children": list(by_group.get(name, []))}
             for name, _, pivot, _ in bones}
    outliner = []
    for name, parent, _, _ in bones:
        (outliner if parent is None else nodes[parent]["children"]).append(nodes[name])
    doc["outliner"] = outliner
    for element in doc["elements"]:
        element["type"] = "cube"            # 离线 codec 只认显式声明的 cube
    doc["textures"][0]["folder"] = "entity"
    out_path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return out_path


def render_check_tex(p: Puppet, bones, mats, review_dir: Path, tmp_dir: Path, name: str, model_id: str, texture_material: dict,
                     views, tex_name: str) -> None:
    """check_tex.png：参考 | 平色 | 贴图，每个视角一对。"""
    flat = write_textured(p, bones, mats, tmp_dir / "_tmp_flat.bbmodel", model_id, texture_material, flat=True)
    tex = write_textured(p, bones, mats, tmp_dir / tex_name, model_id, texture_material)
    size = 440
    ref = Image.open(review_dir / f"{name}.png").convert("RGB")
    ref = ref.resize((int(ref.width * size / ref.height), size), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (ref.width + len(views) * 2 * (size + 8) + 8, size + 30), (28, 29, 33))
    draw = ImageDraw.Draw(canvas)
    font = load_font(13)
    canvas.paste(ref, (0, 30))
    draw.text((4, 8), "REFERENCE", fill=(215, 205, 180), font=font)
    x = ref.width + 8
    for label, yaw, pitch in views:
        for title, path in (("FLAT", flat), ("TEXTURED", tex)):
            canvas.paste(render(path, yaw=yaw, pitch=pitch, size=size, bg=BG, shading="mc")[0], (x, 30))
            draw.text((x + 4, 8), f"{title}: {label}", fill=(230, 230, 230), font=font)
            x += size + 8
    flat.unlink()
    review_dir.mkdir(parents=True, exist_ok=True)
    canvas.save(review_dir / "check_tex.png")
    print(f"✓ {tex.name}；check_tex.png → {review_dir}")
