#!/usr/bin/env python3
"""经脉工厂第 2 批器官生成器共用的 bbmodel 落盘 + 审阅图代码。

只管「把立方体列表变成 bbmodel」和「出 render.png / check.png」，不含任何器官几何。
色板、接口件仍来自 common_parts；器官自己的特殊色（如肝的木色）用 extend_materials 追加。

gen_heart.py 是在本模块之前写的，里面有一份等价的实现，暂未改用本模块（避免动已放行的几何）。
"""

from __future__ import annotations

import base64
import io
import json
import math
import uuid
from pathlib import Path
from typing import Callable, Dict, List, Sequence, Tuple

from PIL import Image, ImageDraw

from common_parts import MAT_UV as COMMON_MAT_UV
from common_parts import PALETTE as COMMON_PALETTE
from common_parts import _assert_no_coplanar_faces, build_texture as build_common_texture

Cubes = List[dict]

REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")

# 渲染角度沿用第 1 批：yaw=0 看到的是 +Z 正面
YAW_FRONT, YAW_SIDE, YAW_34, PITCH_34 = 0.0, 90.0, -35.0, 22.0
BG = (119, 119, 119)
CANVAS_BG = (35, 36, 40)


def extend_materials(extra: Dict[str, Tuple[Tuple[int, int, int, int], List[int]]]):
    """在 common_parts 的色板上追加材质：extra = {名字: (RGBA, [u0, v0, u1, v1])}。"""
    palette = dict(COMMON_PALETTE)
    mat_uv = dict(COMMON_MAT_UV)
    for name, (color, uv) in extra.items():
        palette[name] = color
        mat_uv[name] = uv
    return palette, mat_uv


def build_texture(palette, mat_uv) -> Image.Image:
    """common_parts 统一贴图 + 追加材质的纯色块。"""
    tex = build_common_texture()
    for name, uv in mat_uv.items():
        if name not in COMMON_MAT_UV:
            u0, v0, u1, v1 = uv
            tex.paste(Image.new("RGBA", (u1 - u0, v1 - v0), palette[name]), (u0, v0))
    return tex


def build_bbmodel_doc(name: str, cubes: Cubes, palette, mat_uv, visible_box: Sequence[int]) -> dict:
    buf = io.BytesIO()
    build_texture(palette, mat_uv).save(buf, format="PNG")
    source = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    groups: Dict[str, List[str]] = {}
    for c in cubes:
        uid = str(uuid.uuid4())
        uv = [float(v) for v in mat_uv[c["material"]]]
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
        "name": name,
        "model_identifier": name,
        "visible_box": list(visible_box),
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in groups.items()],
        "textures": [{"name": name, "folder": "block", "namespace": "bong", "id": 0, "source": source}],
    }


def write_bbmodel(path: Path, name: str, cubes: Cubes, palette, mat_uv, visible_box: Sequence[int]) -> Path:
    _assert_no_coplanar_faces(cubes)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(build_bbmodel_doc(name, cubes, palette, mat_uv, visible_box), indent=2, ensure_ascii=False), encoding="utf-8")
    return path


def bounds(cubes: Cubes) -> Tuple[List[float], List[float]]:
    lo = [min(c["from"][i] for c in cubes) for i in range(3)]
    hi = [max(c["to"][i] for c in cubes) for i in range(3)]
    return lo, hi


def _label(canvas: Image.Image, text: str, xy: Tuple[int, int], width: int) -> None:
    draw = ImageDraw.Draw(canvas)
    draw.rectangle([xy[0], xy[1], xy[0] + width, xy[1] + 22], fill=(24, 25, 28))
    draw.text((xy[0] + 6, xy[1] + 5), text, fill=(235, 235, 235))


def _fit(im: Image.Image, h: int) -> Image.Image:
    return im.resize((int(im.width * h / im.height), h), Image.Resampling.LANCZOS)


def render_review(
    organ_id: str,
    bbmodel: Path,
    parts: Sequence[Tuple[str, Callable[[], Cubes]]],
    exploded_ref: Path,
    palette,
    mat_uv,
    visible_box: Sequence[int],
    tmp_dir: Path,
) -> Path:
    """出 render.png（正视 + 3/4 + 侧视）和 check.png（左爆炸图，中部件拆开，右拼好）。"""
    from bbmodel_maker.render.render_bbmodel import render

    out_dir = REVIEW_ROOT / organ_id
    out_dir.mkdir(parents=True, exist_ok=True)
    size = 560
    front, _ = render(bbmodel, yaw=YAW_FRONT, pitch=0.0, size=size, bg=BG)
    side, _ = render(bbmodel, yaw=YAW_SIDE, pitch=0.0, size=size, bg=BG)
    iso, _ = render(bbmodel, yaw=YAW_34, pitch=PITCH_34, size=size, bg=BG)

    canvas = Image.new("RGB", (size * 3 + 40, size + 54), CANVAS_BG)
    for i, (title, im) in enumerate((("FRONT (+Z)", front), ("3/4", iso), ("SIDE", side))):
        x = 10 + i * (size + 10)
        canvas.paste(im, (x, 40))
        _label(canvas, title, (x, 10), 160)
    canvas.save(out_dir / "render.png")

    tmp_dir.mkdir(parents=True, exist_ok=True)
    tiles = []
    for part_name, build in parts:
        p = write_bbmodel(tmp_dir / f"{part_name}.bbmodel", part_name, build(), palette, mat_uv, visible_box)
        im, _ = render(p, yaw=YAW_34, pitch=PITCH_34, size=340, bg=BG)
        tiles.append((part_name, im))

    ref = _fit(Image.open(exploded_ref).convert("RGB"), 680)
    asm = _fit(iso, 680)
    cols = 3
    rows = (len(tiles) + cols - 1) // cols
    grid_w = 340 * cols
    check = Image.new("RGB", (ref.width + grid_w + asm.width + 50, 740), CANVAS_BG)
    check.paste(ref, (10, 40))
    _label(check, f"EXPLODED REFERENCE ({exploded_ref.stem})", (10, 10), 300)
    gx = ref.width + 20
    for i, (part_name, im) in enumerate(tiles):
        x, y = gx + (i % cols) * 340, 40 + (i // cols) * 340
        check.paste(im, (x, y))
        _label(check, part_name, (x, y), 130)
    _label(check, "PARTS (exploded render)", (gx, 10), 220)
    ax = gx + grid_w + 10
    check.paste(asm, (ax, 40))
    _label(check, "ASSEMBLED", (ax, 10), 160)
    check.save(out_dir / "check.png")
    return out_dir


# ─────────────────────────────────────────────────────────────────────────────
# 贴表面的分叉血管树（肝 / 脾共用）：1px 格 + 过台阶补半格
# ─────────────────────────────────────────────────────────────────────────────

Cell = Tuple[int, int]


def stair_cells(c0: int, r0: int, c1: int, r1: int) -> set:
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


def path_cells(waypoints: Sequence[Cell]) -> set:
    """路点折线 → 4 连通阶梯格集合。"""
    cells: set = set()
    for (c0, r0), (c1, r1) in zip(waypoints, waypoints[1:]):
        cells |= stair_cells(c0, r0, c1, r1)
    return cells


def thicken(cells: set) -> set:
    """每格扩成 2×2（向右、向上各多一格），用来画 2×2 截面的主干。"""
    return {(ix + dx, iy + dy) for ix, iy in cells for dx in (0, 1) for dy in (0, 1)}


def relax_heights(tops: Dict[Cell, float], steps=((1, 0), (-1, 0), (0, 1), (0, -1))) -> Dict[Cell, float]:
    """让相邻格前表面高差不超过 0.5px：较低的格往上垫半格（0.5 的整数倍），迭代到稳定。"""
    final = dict(tops)
    changed = True
    while changed:
        changed = False
        for (ix, iy), h in list(final.items()):
            for dx, dy in steps:
                nb = (ix + dx, iy + dy)
                if nb in final and final[nb] - h > 0.5 + 1e-9:
                    base = tops[(ix, iy)]
                    final[(ix, iy)] = max(h, base + 0.5 * math.ceil((final[nb] - 0.5 - base) / 0.5 - 1e-9))
                    changed = True
    return final


def surface_vein_cubes(
    levels: Sequence[Tuple[str, set, float, str, float]],
    front_z: Callable[[float, float, float, float], float],
    x_off: float,
    y_off: float,
    group: str,
) -> Cubes:
    """把若干组 1×1 格画成贴表面的血管块。levels = [(名字, 格集合, 埋深, 材质, 凸出 px)]；所有组一起算过台阶补半格。

    每格 x∈[ix+x_off, ix+x_off+1]、y∈[iy+y_off, iy+y_off+1]，前表面取 front_z(x0,x1,y0,y1) + proud；
    落在体外的格（front_z 抛 ValueError）不画。相邻格高差大于 0.5 的，在较低那格上垫半格（名字加 _half）。
    """
    boxes, tops, owner = {}, {}, {}
    for name, cells, sink, material, proud in levels:
        for ix, iy in sorted(cells):
            x0, y0 = ix + x_off, iy + y_off
            try:
                tops[(ix, iy)] = front_z(x0, x0 + 1.0, y0, y0 + 1.0) + proud
            except ValueError:
                continue
            boxes[(ix, iy)] = (x0, y0, x0 + 1.0, y0 + 1.0)
            owner[(ix, iy)] = (name, sink, material)
    cubes: Cubes = []
    for cell, top in tops.items():
        x0, y0, x1, y1 = boxes[cell]
        name, sink, material = owner[cell]
        cubes.append({"name": f"{name}_{cell[0]}_{cell[1]}", "from": [x0, y0, top - sink], "to": [x1, y1, top], "group": group, "material": material})
    final = relax_heights(tops)
    for cell, top in tops.items():
        if final[cell] - top > 1e-6:
            x0, y0, x1, y1 = boxes[cell]
            name, _, material = owner[cell]
            cubes.append({"name": f"{name}_{cell[0]}_{cell[1]}_half", "from": [x0, y0, top], "to": [x1, y1, final[cell]], "group": group, "material": material})
    return cubes
