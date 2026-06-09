#!/usr/bin/env python3
"""手搓工作台（workbench）Blockbench .bbmodel 重做生成器。

旧 local_models/Workbench.bbmodel 是个 16³ 实心方块（贴图换皮），对「通往 104
配方手搓树的核心台」太敷衍。本脚本重做成真正的散修工作台，对齐延寿棺/容器管线：

    local_models/Workbench.bbmodel        ← 本脚本产物（覆盖旧方块版）
    → Blockbench 导出 → assets/bong/geo/workbench.geo.json
    → 贴图 assets/bong/textures/entity/workbench.png

视觉语言（末法残土散修台）：厚木台面（骨白 + 朱砂阵纹网格，沿用旧贴图立意
#E8DCC8/#5C4A3A/#8B3A3A）+ 四腿（镂空非实心）+ 下层物料架 + 前沿台钳（vise）
+ 台面石砧（anvil）+ 4 角骨钉（保留旧版骨钉母题）。静态方块，无开合动画。

尺寸（MC 格）：约 1.0 × 1.06 × 1.0 格（台钳/石砧略高出台面）。

用法:
    python3 scripts/models/gen_workbench.py
    python3 scripts/models/gen_workbench.py --preview-only
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = REPO / "local_models" / "Workbench.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "workbench_preview.png"

PX = 16.0
RES = 64

HALF = 8.0
LEG_OUT = 7.6
LEG_IN = 5.0
LEG_TOP = 12.0          # 腿顶（台面底）
APRON = (10.0, 12.0)    # 围裙（台面下框）
STRETCH = (3.0, 4.0)    # 低横档（托物料架）
SHELF = (4.0, 5.0)      # 物料架板
TOP_SLAB = (12.0, 14.0)  # 厚木台面
SURF = (14.0, 14.6)     # 骨白阵纹台面薄板


def build_cubes():
    cubes = []

    # ── legs —— 4 腿（镂空，非实心方块）──────────────────────────
    for sx in (-1, 1):
        for sz in (-1, 1):
            xa, xb = sorted([LEG_IN * sx, LEG_OUT * sx])
            za, zb = sorted([LEG_IN * sz, LEG_OUT * sz])
            cubes.append(("legs", "wood", f"leg_{sx}_{sz}",
                          [xa, 0.0, za], [xb, LEG_TOP, zb]))

    # ── frame —— 台面下围裙（4 边）+ 低横档（前后 2 道托架）──────
    a0, a1 = APRON
    cubes.append(("frame", "wood", "apron_front", [-LEG_OUT, a0, LEG_IN], [LEG_OUT, a1, LEG_OUT]))
    cubes.append(("frame", "wood", "apron_back", [-LEG_OUT, a0, -LEG_OUT], [LEG_OUT, a1, -LEG_IN]))
    cubes.append(("frame", "wood", "apron_left", [-LEG_OUT, a0, -LEG_OUT], [-LEG_IN, a1, LEG_OUT]))
    cubes.append(("frame", "wood", "apron_right", [LEG_IN, a0, -LEG_OUT], [LEG_OUT, a1, LEG_OUT]))
    s0, s1 = STRETCH
    cubes.append(("frame", "wood", "stretch_left", [-LEG_OUT, s0, -LEG_OUT], [-LEG_IN, s1, LEG_OUT]))
    cubes.append(("frame", "wood", "stretch_right", [LEG_IN, s0, -LEG_OUT], [LEG_OUT, s1, LEG_OUT]))

    # ── shelf —— 下层物料架板 ───────────────────────────────────
    cubes.append(("shelf", "wood", "shelf_board",
                  [-LEG_OUT, SHELF[0], -LEG_OUT], [LEG_OUT, SHELF[1], LEG_OUT]))

    # ── top —— 厚木台面（略悬挑）────────────────────────────────
    cubes.append(("top", "wood", "top_slab",
                  [-HALF, TOP_SLAB[0], -HALF], [HALF, TOP_SLAB[1], HALF]))

    # ── surface —— 骨白朱砂阵纹台面薄板（略内收，上面朝天）─────────
    cubes.append(("surface", "surface", "work_surface",
                  [-7.4, SURF[0], -7.4], [7.4, SURF[1], 7.4]))

    # ── vise —— 前右台钳（固定颚 + 活动颚 + 螺杆把手）────────────
    cubes.append(("vise", "iron", "vise_fixed", [3.6, SURF[1], 6.6], [6.4, 16.4, 8.6]))
    cubes.append(("vise", "iron", "vise_jaw", [3.6, SURF[1], 5.0], [6.4, 16.4, 6.2]))
    cubes.append(("vise", "iron", "vise_screw", [4.6, 15.0, 8.6], [5.4, 15.8, 10.2]))

    # ── stone —— 台面石砧（后左角，略带錾痕）─────────────────────
    cubes.append(("stone", "stone", "anvil", [-7.2, SURF[1], -7.2], [-3.0, 16.6, -3.6]))
    cubes.append(("stone", "stone", "anvil_face", [-6.6, 16.6, -6.6], [-3.6, 17.2, -4.2]))

    # ── bone —— 4 角骨钉（保留旧版母题，嵌台面四角）──────────────
    for sx in (-1, 1):
        for sz in (-1, 1):
            xa, xb = sorted([6.6 * sx, 7.6 * sx])
            za, zb = sorted([6.6 * sz, 7.6 * sz])
            cubes.append(("bone", "bone", f"bonepin_{sx}_{sz}",
                          [xa, TOP_SLAB[1], za], [xb, 15.0, zb]))

    return cubes


BONE_ORDER = ["legs", "frame", "shelf", "top", "surface", "vise", "stone", "bone"]
BONE_PIVOTS = {b: [0.0, 0.0, 0.0] for b in BONE_ORDER}
BONE_COLORS = {
    "legs": (138, 102, 64),
    "frame": (120, 88, 54),
    "shelf": (150, 114, 72),
    "top": (158, 120, 78),
    "surface": (226, 214, 192),   # 骨白
    "vise": (96, 100, 110),
    "stone": (132, 128, 120),
    "bone": (224, 212, 188),
}

MAT_ZONE = {
    "wood": (0, 0, RES, 28),
    "surface": (0, 28, RES, 48),
    "iron": (0, 48, RES, 57),
    "stone": (0, 57, RES, RES),
}
MAT_OF = {  # bone 复用 surface 骨白区
    "wood": "wood", "surface": "surface", "iron": "iron", "stone": "stone", "bone": "surface",
}


def make_texture(res=RES, seed=53):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 木 y[0,28)
    wmask = y < 28
    pid = (x // 8).astype(float)
    seam = ((x % 8) < 1) | ((x % 8) > 6.9)
    grain = 0.5 + 0.5 * np.sin(y * 0.5 + pid * 1.8)
    wcol = np.array([150, 112, 72], float)[None, None, :] + (grain[..., None] - 0.5) * 28
    wcol += (rng.random((res, res, 1)) - 0.5) * 12
    wcol[seam] *= 0.6
    wcol = np.clip(wcol, 22, 200)
    img[wmask, :3] = wcol[wmask].astype(np.uint8)

    # 骨白朱砂阵纹台面 y[28,48)：每 16px 一个九宫格 + 中心朱砂阵
    smask = (y >= 28) & (y < 48)
    sy = (y - 28).astype(float)
    sx = x.astype(float)
    bone_white = np.array([232, 220, 200], float)
    grid_brown = np.array([92, 74, 58], float)
    cinnabar = np.array([139, 58, 58], float)
    scol = np.broadcast_to(bone_white[None, None, :], (res, res, 3)).copy()
    cell = 16.0
    cx_in = sx % cell
    cy_in = sy % cell
    # 九宫网格线（每 cell 内三等分）
    gl = (np.minimum(cx_in % (cell / 3), cell / 3 - cx_in % (cell / 3)) < 0.7) | \
         (np.minimum(cy_in % (cell / 3), cell / 3 - cy_in % (cell / 3)) < 0.7)
    # cell 边框
    border = (np.minimum(cx_in, cell - cx_in) < 0.8) | (np.minimum(cy_in, cell - cy_in) < 0.8)
    scol[gl] = grid_brown
    scol[border] = grid_brown * 0.85
    # 中心朱砂圆阵
    dc = np.hypot(cx_in - cell / 2, cy_in - cell / 2)
    ring = (np.abs(dc - 3.4) < 0.9) | (dc < 1.0)
    scol[ring] = cinnabar
    scol += (rng.random((res, res, 1)) - 0.5) * 8
    scol = np.clip(scol, 30, 245)
    img[smask, :3] = scol[smask].astype(np.uint8)

    # 铁 y[48,57)
    imask = (y >= 48) & (y < 57)
    icol = np.array([94, 98, 108], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 22
    icol += np.sin(x * 0.9 + y * 0.4)[..., None] * 6
    icol = np.clip(icol, 28, 188)
    img[imask, :3] = icol[imask].astype(np.uint8)

    # 石 y[57,64)
    emask = y >= 57
    ecol = np.array([132, 128, 120], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 26
    for _ in range(10):
        cxp, cyp = rng.integers(0, res), rng.integers(57, res)
        ecol[max(0, cyp - 1):cyp + 1, max(0, cxp - 2):cxp + 2] *= rng.uniform(0.6, 1.3)
    ecol = np.clip(ecol, 40, 200)
    img[emask, :3] = ecol[emask].astype(np.uint8)

    return Image.fromarray(img, "RGBA")


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class Packer:
    def __init__(self, x0, y0, x1, y1):
        self.x0, self.y0, self.x1, self.y1 = x0, y0, x1, y1
        self.x, self.y, self.rowh = x0, y0, 0.0

    def place(self, w, h):
        w = min(w, self.x1 - self.x0)
        h = min(h, self.y1 - self.y0)
        if self.x + w > self.x1:
            self.x = self.x0
            self.y += self.rowh
            self.rowh = 0.0
        if self.y + h > self.y1:
            self.y = self.y0
        ox, oy = self.x, self.y
        self.x += w
        self.rowh = max(self.rowh, h)
        return ox, oy


def cube_faces_uv(frm, to, packer):
    dx, dy, dz = to[0] - frm[0], to[1] - frm[1], to[2] - frm[2]
    dims = {"north": (dx, dy), "south": (dx, dy), "east": (dz, dy),
            "west": (dz, dy), "up": (dx, dz), "down": (dx, dz)}
    faces = {}
    for name, (w, h) in dims.items():
        ox, oy = packer.place(abs(w), abs(h))
        faces[name] = {"uv": [round(ox, 2), round(oy, 2),
                              round(ox + abs(w), 2), round(oy + abs(h), 2)], "texture": 0}
    return faces


def build_bbmodel():
    cubes = build_cubes()
    packers = {m: Packer(*z) for m, z in MAT_ZONE.items()}
    elements = []
    bone_children = {b: [] for b in BONE_ORDER}

    for bone, material, name, frm, to in cubes:
        euid = str(uuid.uuid4())
        packer = packers[MAT_OF[material]]
        elements.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False,
            "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
            "uuid": euid, "from": [round(v, 3) for v in frm], "to": [round(v, 3) for v in to],
            "autouv": 0, "color": BONE_ORDER.index(bone), "origin": list(BONE_PIVOTS[bone]),
            "faces": cube_faces_uv(frm, to, packer),
        })
        bone_children[bone].append(euid)

    outliner = []
    for bone in BONE_ORDER:
        outliner.append({
            "name": bone, "origin": list(BONE_PIVOTS[bone]), "color": BONE_ORDER.index(bone),
            "uuid": str(uuid.uuid4()), "export": True, "mirror_uv": False, "isOpen": True,
            "locked": False, "visibility": True, "autouv": 0, "children": bone_children[bone],
        })

    tex = make_texture()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "workbench", "model_identifier": "geometry.bong.workbench",
        "visible_box": [2, 2.2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "workbench.png", "folder": "entity", "namespace": "bong",
            "id": "0", "width": RES, "height": RES, "uv_width": RES, "uv_height": RES,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


def render_preview(cubes, tex, out=PREVIEW_OUT):
    scale, pad, gap = 8, 16, 24

    def lit(color, k):
        return tuple(int(np.clip(c * k, 0, 255)) for c in color)

    def ortho(ax_u, ax_v, title):
        us = [v for _, _, _, f, t in cubes for v in (f[ax_u], t[ax_u])]
        vs = [v for _, _, _, f, t in cubes for v in (f[ax_v], t[ax_v])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (30, 30, 34, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(220, 220, 220))

        def to_px(u, v):
            return pad + (u - umin) * scale, pad + 14 + ((vmax - vmin) * scale - (v - vmin) * scale)

        order = sorted(cubes, key=lambda c: c[3][3 - ax_u - ax_v])
        for bone, _, _, frm, to in order:
            x0, y0 = to_px(frm[ax_u], frm[ax_v])
            x1, y1 = to_px(to[ax_u], to[ax_v])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,), outline=(20, 16, 12, 255))
        return im

    front = ortho(0, 1, "FRONT (X-Y)")
    side = ortho(2, 1, "SIDE  (Z-Y)")
    top = ortho(0, 2, "TOP   (X-Z) 台面阵纹")

    def iso():
        ca, sa = math.cos(math.radians(30)), math.sin(math.radians(30))

        def proj(x, y, z):
            return (x - z) * ca, (x + z) * sa - y

        pts = [proj(X, Y, Z) for _, _, _, f, t in cubes
               for X in (f[0], t[0]) for Y in (f[1], t[1]) for Z in (f[2], t[2])]
        umin, umax = min(p[0] for p in pts), max(p[0] for p in pts)
        vmin, vmax = min(p[1] for p in pts), max(p[1] for p in pts)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (30, 30, 34, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), "ISO (front-right)", fill=(220, 220, 220))

        def to_px(p):
            return pad + (p[0] - umin) * scale, pad + 14 + (p[1] - vmin) * scale

        order = sorted(cubes, key=lambda c: (c[3][0] + c[3][2] + c[3][1]))
        for bone, _, _, frm, to in order:
            x0, y0, z0 = frm
            x1, y1, z1 = to
            col = BONE_COLORS[bone]
            for verts, k in [
                ([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.18),
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.88),
                ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.62),
            ]:
                poly = [to_px(proj(*v)) for v in verts]
                d.polygon(poly, fill=lit(col, k) + (255,), outline=(20, 16, 12, 255))
        return im

    iso_im = iso()
    tiles = [front, side, top]
    tw = sum(t.width for t in tiles) + gap * (len(tiles) + 1)
    th = max(t.height for t in tiles)
    tex_big = tex.resize((RES * 3, RES * 3), Image.NEAREST)
    bot_h = max(iso_im.height, tex_big.height)
    W_ = max(tw, iso_im.width + tex_big.width + gap * 3)
    H_ = th + bot_h + gap * 3
    canvas = Image.new("RGBA", (W_, H_), (18, 18, 20, 255))
    x = gap
    for t in tiles:
        canvas.paste(t, (x, gap), t)
        x += t.width + gap
    canvas.paste(iso_im, (gap, th + gap * 2), iso_im)
    canvas.paste(tex_big, (gap * 2 + iso_im.width, th + gap * 2), tex_big)
    d = ImageDraw.Draw(canvas)
    d.text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x3) — wood/骨白阵纹/iron/stone",
           fill=(200, 200, 200))
    d.text((gap, H_ - 16), "bones: " + "  ".join(BONE_ORDER), fill=(180, 180, 180))
    canvas.save(out)
    return out


def summarize(cubes):
    xs = [v for _, _, _, f, t in cubes for v in (f[0], t[0])]
    ys = [v for _, _, _, f, t in cubes for v in (f[1], t[1])]
    zs = [v for _, _, _, f, t in cubes for v in (f[2], t[2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox  : {bb[0]:.1f}×{bb[1]:.1f}×{bb[2]:.1f}px = "
          f"{bb[0]/PX:.3f}W × {bb[1]/PX:.3f}H × {bb[2]/PX:.3f}D 格")
    print(f"  cubes : {len(cubes)}  ("
          + ", ".join(f"{b}:{sum(1 for c in cubes if c[0]==b)}" for b in BONE_ORDER) + ")")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    model, cubes, tex = build_bbmodel()
    print("手搓工作台 / workbench (重做):")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    p = render_preview(cubes, tex)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
