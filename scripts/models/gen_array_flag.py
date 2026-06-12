#!/usr/bin/env python3
"""阵旗·凡（array_flag_basic）Blockbench .bbmodel 生成器。

产物：
    local_models/ArrayFlagBasic.bbmodel
    scripts/models/array_flag_basic_preview.png

造型目标：凡阶组网阵的低矮阵旗。不是华丽法宝，而是旧木杆、骨钉压脚、褪色布幡、
青白弱光阵纹。旗面拆成多片薄 cuboid，避免“一块板贴图”偷懒。

用法：
    python3 scripts/models/gen_array_flag.py
    python3 scripts/models/gen_array_flag.py --preview-only
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
BBMODEL_OUT = REPO / "local_models" / "ArrayFlagBasic.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "array_flag_basic_preview.png"

PX = 16.0
RES = 64

BONE_ORDER = ["base", "pole", "cloth", "rune", "bone"]
BONE_PIVOTS = {name: [0.0, 0.0, 0.0] for name in BONE_ORDER}
BONE_COLORS = {
    "base": (92, 78, 62),
    "pole": (124, 88, 52),
    "cloth": (126, 88, 58),
    "rune": (128, 218, 230),
    "bone": (218, 206, 184),
}
MAT_ZONE = {
    "wood": (0, 0, RES, 18),
    "cloth": (0, 18, RES, 42),
    "rune": (0, 42, RES, 54),
    "bone": (0, 54, RES, RES),
}


def part_base():
    return [
        ("base", "wood", "base_stake", [-1.9, 0.0, -1.9], [1.9, 0.75, 1.9]),
        ("base", "wood", "front_wedge", [-1.2, 0.75, 1.1], [1.2, 1.25, 1.9]),
        ("bone", "bone", "base_bone_pin_l", [-1.7, 1.25, -0.35], [-0.75, 1.85, 0.35]),
        ("bone", "bone", "base_bone_pin_r", [0.75, 1.25, -0.35], [1.7, 1.85, 0.35]),
    ]


def part_pole():
    return [
        ("pole", "wood", "main_pole", [-0.25, 0.7, -0.25], [0.25, 14.4, 0.25]),
        ("pole", "wood", "top_crossbar", [-0.25, 12.95, -0.22], [5.8, 13.45, 0.22]),
        ("pole", "wood", "lower_crossbar", [-0.15, 7.1, -0.18], [4.7, 7.45, 0.18]),
        ("bone", "bone", "top_cap", [-0.5, 14.35, -0.5], [0.5, 15.1, 0.5]),
    ]


def part_cloth():
    return [
        ("cloth", "cloth", "banner_top", [0.25, 11.35, 0.05], [5.65, 12.9, 0.22]),
        ("cloth", "cloth", "banner_mid", [0.25, 8.65, 0.0], [5.15, 11.35, 0.18]),
        ("cloth", "cloth", "banner_lower_left", [0.25, 6.75, 0.04], [2.15, 8.65, 0.2]),
        ("cloth", "cloth", "banner_lower_mid", [2.2, 7.25, -0.03], [3.55, 8.65, 0.15]),
        ("cloth", "cloth", "banner_lower_right", [3.65, 6.45, 0.08], [4.85, 8.65, 0.24]),
    ]


def part_runes():
    return [
        ("rune", "rune", "rune_vertical", [2.55, 8.15, 0.25], [2.85, 12.35, 0.34]),
        ("rune", "rune", "rune_upper_slash", [1.25, 10.9, 0.26], [4.25, 11.18, 0.35]),
        ("rune", "rune", "rune_lower_slash", [1.6, 8.85, 0.26], [4.65, 9.13, 0.35]),
        ("rune", "rune", "rune_eye", [2.25, 9.65, 0.27], [3.15, 10.55, 0.36]),
        ("bone", "bone", "cloth_bone_tag", [4.5, 7.1, 0.24], [5.05, 7.95, 0.42]),
    ]


def all_cubes():
    return part_base() + part_pole() + part_cloth() + part_runes()


def make_texture(res=RES, seed=77):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    wood = y < 18
    grain = 0.5 + 0.5 * np.sin(x * 0.55 + y * 0.2)
    wcol = np.array([124, 88, 52], float)[None, None, :] + (grain[..., None] - 0.5) * 28
    wcol += (rng.random((res, res, 1)) - 0.5) * 12
    img[wood, :3] = np.clip(wcol, 24, 175)[wood].astype(np.uint8)

    cloth = (y >= 18) & (y < 42)
    thread = 0.5 + 0.5 * np.sin(x * 0.8) * np.sin(y * 1.1)
    ccol = np.array([126, 88, 58], float)[None, None, :] + (thread[..., None] - 0.5) * 22
    stain = rng.random((res, res, 1))
    ccol -= (stain > 0.94) * rng.uniform(18, 38, (res, res, 1))
    img[cloth, :3] = np.clip(ccol, 35, 155)[cloth].astype(np.uint8)

    rune = (y >= 42) & (y < 54)
    rbg = np.array([12, 30, 36], float)[None, None, :]
    glow = np.array([128, 218, 230], float)
    lines = (np.minimum(x % 8, 8 - (x % 8)) < 1.0) | (np.minimum((y - 42) % 4, 4 - ((y - 42) % 4)) < 0.8)
    rcol = np.broadcast_to(rbg, (res, res, 3)).copy()
    rcol[lines] = glow
    rcol += (rng.random((res, res, 1)) - 0.5) * 10
    img[rune, :3] = np.clip(rcol, 10, 245)[rune].astype(np.uint8)

    bone = y >= 54
    bcol = np.array([218, 206, 184], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 18
    pores = rng.random((res, res, 1)) > 0.95
    bcol -= pores * 38
    img[bone, :3] = np.clip(bcol, 42, 235)[bone].astype(np.uint8)

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
    cubes = all_cubes()
    packers = {name: Packer(*zone) for name, zone in MAT_ZONE.items()}
    elements = []
    bone_children = {bone: [] for bone in BONE_ORDER}
    for bone, material, name, frm, to in cubes:
        euid = str(uuid.uuid4())
        elements.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False,
            "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
            "uuid": euid, "from": [round(v, 3) for v in frm], "to": [round(v, 3) for v in to],
            "autouv": 0, "color": BONE_ORDER.index(bone), "origin": list(BONE_PIVOTS[bone]),
            "faces": cube_faces_uv(frm, to, packers[material]),
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
        "name": "array_flag_basic", "model_identifier": "geometry.bong.array_flag_basic",
        "visible_box": [1, 2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "array_flag_basic_intact.png", "folder": "entity", "namespace": "bong",
            "id": "0", "width": RES, "height": RES, "uv_width": RES, "uv_height": RES,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


def render_preview(cubes, tex, out=PREVIEW_OUT):
    scale, pad, gap = 10, 16, 24

    def lit(color, k):
        return tuple(int(np.clip(c * k, 0, 255)) for c in color)

    def ortho(ax_u, ax_v, title):
        us = [v for _, _, _, f, t in cubes for v in (f[ax_u], t[ax_u])]
        vs = [v for _, _, _, f, t in cubes for v in (f[ax_v], t[ax_v])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (28, 28, 32, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(220, 220, 220))

        def to_px(u, v):
            return pad + (u - umin) * scale, pad + 14 + ((vmax - vmin) * scale - (v - vmin) * scale)

        depth_axis = ({0, 1, 2} - {ax_u, ax_v}).pop()
        for bone, _, _, frm, to in sorted(cubes, key=lambda c: c[3][depth_axis]):
            x0, y0 = to_px(frm[ax_u], frm[ax_v])
            x1, y1 = to_px(to[ax_u], to[ax_v])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,), outline=(18, 16, 14, 255))
        return im

    front = ortho(0, 1, "FRONT (X-Y) 布幡阵纹")
    side = ortho(2, 1, "SIDE  (Z-Y)")
    top = ortho(0, 2, "TOP   (X-Z)")

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
        im = Image.new("RGBA", (wpx, hpx), (28, 28, 32, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), "ISO (front-right)", fill=(220, 220, 220))

        def to_px(point):
            return pad + (point[0] - umin) * scale, pad + 14 + (point[1] - vmin) * scale

        for bone, _, _, frm, to in sorted(cubes, key=lambda c: c[3][0] + c[3][1] + c[3][2]):
            x0, y0, z0 = frm
            x1, y1, z1 = to
            col = BONE_COLORS[bone]
            for verts, k in [
                ([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.16),
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 1.0),
                ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.66),
            ]:
                d.polygon([to_px(proj(*v)) for v in verts],
                          fill=lit(col, k) + (255,), outline=(18, 16, 14, 255))
        return im

    iso_im = iso()
    tex_big = tex.resize((RES * 3, RES * 3), Image.NEAREST)
    top_tiles = [front, side, top]
    top_w = sum(t.width for t in top_tiles) + gap * (len(top_tiles) + 1)
    top_h = max(t.height for t in top_tiles)
    bot_h = max(iso_im.height, tex_big.height)
    canvas = Image.new("RGBA", (max(top_w, iso_im.width + tex_big.width + gap * 3),
                                top_h + bot_h + gap * 3), (18, 18, 20, 255))
    x = gap
    for tile in top_tiles:
        canvas.paste(tile, (x, gap), tile)
        x += tile.width + gap
    canvas.paste(iso_im, (gap, top_h + gap * 2), iso_im)
    canvas.paste(tex_big, (gap * 2 + iso_im.width, top_h + gap * 2), tex_big)
    d = ImageDraw.Draw(canvas)
    d.text((gap * 2 + iso_im.width, top_h + gap * 2 - 12),
           "TEXTURE 64x64 (x3) — wood / cloth / rune / bone", fill=(200, 200, 200))
    d.text((gap, canvas.height - 16), "parts: " + "  ".join(BONE_ORDER), fill=(180, 180, 180))
    canvas.save(out)
    return out


def summarize(cubes):
    xs = [v for _, _, _, f, t in cubes for v in (f[0], t[0])]
    ys = [v for _, _, _, f, t in cubes for v in (f[1], t[1])]
    zs = [v for _, _, _, f, t in cubes for v in (f[2], t[2])]
    print("阵旗·凡 / array_flag_basic:")
    print(f"  bbox  : {(max(xs)-min(xs)):.1f}×{(max(ys)-min(ys)):.1f}×{(max(zs)-min(zs)):.1f}px = "
          f"{(max(xs)-min(xs))/PX:.3f}W × {(max(ys)-min(ys))/PX:.3f}H × {(max(zs)-min(zs))/PX:.3f}D 格")
    print(f"  cubes : {len(cubes)}  ("
          + ", ".join(f"{b}:{sum(1 for c in cubes if c[0] == b)}" for b in BONE_ORDER) + ")")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--preview-only", action="store_true")
    args = parser.parse_args()

    model, cubes, tex = build_bbmodel()
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    preview = render_preview(cubes, tex)
    print(f"  → preview: {preview.relative_to(REPO)}")


if __name__ == "__main__":
    main()
