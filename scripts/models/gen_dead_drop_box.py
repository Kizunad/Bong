#!/usr/bin/env python3
"""死信箱（dead_drop_box）Blockbench .bbmodel 生成器。

plan-placeable-container-blocks-v1 P2 的可放置世界容器之一（死信箱，匿名异步投递，
worldview §九:850 阵法防砸，解锁需走私者师承）。对齐延寿棺管线：

    local_models/DeadDropBox.bbmodel      ← 本脚本产物（Blockbench 源）
    → Blockbench 导出 → assets/bong/geo/dead_drop_box.geo.json
    → 贴图 assets/bong/textures/entity/dead_drop_box.png
       （array 区做发光层：导出时 array UV 区单独切出做 emissive overlay）

视觉语言：低矮埋地铁箍木箱 + 正面青光阵纹面（防砸阵法）。结构 = 暗旧木箱身
（body）+ 埋地土石围沿（earth，比箱体宽=半埋感）+ 铁箍横竖箍带+搭扣（bands）+
正面凹陷阵纹发光窗（array，青光）+ 掀盖带铁条（lid）。

尺寸（MC 格）：宽 0.94 × 高 0.72（低矮）× 深 0.84。

用法:
    python3 scripts/models/gen_dead_drop_box.py
    python3 scripts/models/gen_dead_drop_box.py --preview-only
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
BBMODEL_OUT = REPO / "local_models" / "DeadDropBox.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "dead_drop_box_preview.png"

PX = 16.0
RES = 64

HALF_W = 7.2
HALF_D = 6.2
BODY_Y0 = 1.6        # 箱身底（坐在埋地围沿上）
BODY_TOP = 9.0       # 箱身顶（盖在其上）
LID_Y1 = 11.5        # 盖顶

EARTH_OUT = 0.8      # 埋地围沿外凸（比箱体宽=半埋）
EARTH_Y1 = 2.4

BAND_T = 0.3         # 铁箍凸出
BAND_LO = (2.8, 4.0)
BAND_HI = (6.4, 7.6)
STRAP_X = 4.4        # 正面竖箍 x 位


def build_cubes():
    cubes = []

    # ── earth —— 埋地土石围沿（比箱体宽，半埋感）────────────────
    cubes.append(("earth", "earth", "buried_collar",
                  [-HALF_W - EARTH_OUT, 0.0, -HALF_D - EARTH_OUT],
                  [HALF_W + EARTH_OUT, EARTH_Y1, HALF_D + EARTH_OUT]))

    # ── body —— 暗旧木箱身（实心）────────────────────────────────
    cubes.append(("body", "wood", "box",
                  [-HALF_W, BODY_Y0, -HALF_D], [HALF_W, BODY_TOP, HALF_D]))

    # ── bands —— 铁箍：上下两道横箍（绕 4 面）+ 正面两道竖箍 + 搭扣 ─
    for tag, (y0, y1) in (("lo", BAND_LO), ("hi", BAND_HI)):
        cubes.append(("bands", "iron", f"band_{tag}_front",
                      [-HALF_W, y0, HALF_D - BAND_T], [HALF_W, y1, HALF_D + BAND_T]))
        cubes.append(("bands", "iron", f"band_{tag}_back",
                      [-HALF_W, y0, -HALF_D - BAND_T], [HALF_W, y1, -HALF_D + BAND_T]))
        cubes.append(("bands", "iron", f"band_{tag}_left",
                      [-HALF_W - BAND_T, y0, -HALF_D], [-HALF_W + BAND_T, y1, HALF_D]))
        cubes.append(("bands", "iron", f"band_{tag}_right",
                      [HALF_W - BAND_T, y0, -HALF_D], [HALF_W + BAND_T, y1, HALF_D]))
    # 正面两道竖箍（框住阵纹面）
    for sx in (-1, 1):
        xc = sx * STRAP_X
        cubes.append(("bands", "iron", f"strap_{sx}",
                      [xc - 0.6, BODY_Y0, HALF_D - BAND_T], [xc + 0.6, BODY_TOP, HALF_D + BAND_T]))
    # 正面搭扣（锁）
    cubes.append(("bands", "iron", "hasp",
                  [-1.1, 7.4, HALF_D - BAND_T], [1.1, 9.6, HALF_D + 0.7]))

    # ── array —— 正面凹陷阵纹发光窗（青光，框在两竖箍+两横箍之间）──
    cubes.append(("array", "array", "rune_face",
                  [-STRAP_X + 0.6, BAND_LO[1], HALF_D - 0.1], [STRAP_X - 0.6, BAND_HI[0], HALF_D + 0.25]))

    # ── lid —— 掀盖（铰链后上棱）+ 一道纵向铁条 ─────────────────
    cubes.append(("lid", "wood", "lid_board",
                  [-HALF_W, BODY_TOP, -HALF_D], [HALF_W, BODY_TOP + 1.8, HALF_D]))
    cubes.append(("lid", "iron", "lid_strap",
                  [-1.2, BODY_TOP, -HALF_D], [1.2, LID_Y1, HALF_D]))

    return cubes


BONE_ORDER = ["earth", "body", "bands", "array", "lid"]
BONE_PIVOTS = {
    "earth": [0.0, 0.0, 0.0],
    "body": [0.0, 0.0, 0.0],
    "bands": [0.0, 0.0, 0.0],
    "array": [0.0, 0.0, 0.0],
    "lid": [0.0, BODY_TOP, -HALF_D],
}
BONE_COLORS = {
    "earth": (78, 64, 50),
    "body": (110, 84, 58),
    "bands": (92, 96, 106),
    "array": (96, 214, 236),   # 青光（预览高亮）
    "lid": (122, 94, 64),
}

MAT_ZONE = {
    "wood": (0, 0, RES, 32),
    "iron": (0, 32, RES, 44),
    "array": (0, 44, RES, 56),
    "earth": (0, 56, RES, RES),
}


def make_texture(res=RES, seed=41):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 暗旧木 y[0,32)
    wmask = y < 32
    plank_w = 8
    pid = (x // plank_w).astype(float)
    seam = ((x % plank_w) < 1) | ((x % plank_w) > plank_w - 1.1)
    grain = 0.5 + 0.5 * np.sin(y * 0.5 + pid * 1.7)
    wcol = np.array([110, 84, 58], float)[None, None, :] + (grain[..., None] - 0.5) * 26
    wcol += (rng.random((res, res, 1)) - 0.5) * 12
    wcol[seam] *= 0.55
    wcol = np.clip(wcol, 18, 170)
    img[wmask, :3] = wcol[wmask].astype(np.uint8)

    # 暗铁 y[32,44)
    imask = (y >= 32) & (y < 44)
    icol = np.array([92, 96, 106], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 22
    icol += np.sin(x * 0.9 + y * 0.4)[..., None] * 6
    icol = np.clip(icol, 26, 190)
    img[imask, :3] = icol[imask].astype(np.uint8)

    # 阵纹发光 y[44,56)：暗青底 + 青光经纬格 + 对角 + 节点亮点（可平铺）
    amask = (y >= 44) & (y < 56)
    ax = x.astype(float)
    ay = (y - 44).astype(float)
    bg = np.array([14, 34, 42], float)[None, None, :]
    acol = np.broadcast_to(bg, (res, res, 3)).copy()
    glow = np.array([96, 214, 236], float)
    # 经纬格线（每 4px）
    grid = (np.minimum(ax % 4, 4 - (ax % 4)) < 0.7) | (np.minimum(ay % 4, 4 - (ay % 4)) < 0.7)
    # 对角线
    diag = (np.abs(((ax + ay) % 8) - 4) < 0.8) | (np.abs(((ax - ay) % 8) - 4) < 0.8)
    # 节点亮点（格交点）
    node = (np.minimum(ax % 4, 4 - (ax % 4)) < 0.9) & (np.minimum(ay % 4, 4 - (ay % 4)) < 0.9)
    line = (grid | diag).astype(float)
    acol = acol + line[..., None] * (glow - bg.squeeze()) * 0.85
    acol = acol + node[..., None] * (np.array([160, 245, 255]) - bg.squeeze()) * 0.6
    acol = np.clip(acol, 10, 255)
    img[amask, :3] = acol[amask.astype(bool)].astype(np.uint8)

    # 埋地土石 y[56,64)
    emask = y >= 56
    ecol = np.array([78, 64, 50], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 30
    for _ in range(14):
        cx, cy = rng.integers(0, res), rng.integers(56, res)
        ecol[max(0, cy - 1):cy + 1, max(0, cx - 1):cx + 1] *= rng.uniform(0.5, 1.4)
    ecol = np.clip(ecol, 20, 150)
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
        "name": "dead_drop_box", "model_identifier": "geometry.bong.dead_drop_box",
        "visible_box": [2, 2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "dead_drop_box.png", "folder": "entity", "namespace": "bong",
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

    front = ortho(0, 1, "FRONT (X-Y) 阵纹面")
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
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.95),  # +Z 正面（阵纹）
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
    d.text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x3) — wood/iron/阵纹/土",
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
    print(f"  lid hinge: {BONE_PIVOTS['lid']} (绕 X 后掀)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    model, cubes, tex = build_bbmodel()
    print("死信箱 / dead_drop_box:")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    p = render_preview(cubes, tex)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
