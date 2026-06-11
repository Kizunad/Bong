#!/usr/bin/env python3
"""凡木盾（wooden_shield）Blockbench .bbmodel 生成器。

plan-shield-block-v1 的手持盾牌资产之一。worldview §五:432——末法时代真元
外放护盾不可能，防御=处理打到肉体上的物理冲击；凡木盾是醒灵/引气期玩家
无功法也能用的凡人级物理防御。

手持物品最终走 SML+OBJ 管线（client/tools/tripo_to_sml.py 同类产物），本
bbmodel 是设计/手改源：

    local_models/WoodenShield.bbmodel     ← 本脚本产物（Blockbench 源）
    → Blockbench 手改 / 导出 OBJ → assets/bong/models/item/wooden_shield/

视觉语言：竖板拼合圆盾雏形——5 块长短不一的竖木板（手作感）+ 背面双横
撑 + 正面两道铁皮箍带 + 中心铁浮雕盾凸（boss）+ 横撑端头草绳缠扎 + 背面
握把。整体竖立在 XY 面，正面朝 +Z。

尺寸（MC px）：宽 ~13 × 高 18 × 厚 ~5（含 boss 凸起与背部握把）。

用法:
    python3 scripts/models/gen_wooden_shield.py
    python3 scripts/models/gen_wooden_shield.py --preview-only
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
BBMODEL_OUT = REPO / "local_models" / "WoodenShield.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "wooden_shield_preview.png"

PX = 16.0
RES = 64

# 5 块竖板：宽度 + 顶/底参差（手作感）
PLANKS = [
    # (x0, x1, y0, y1)
    (-6.0, -3.6, 0.6, 17.0),
    (-3.6, -1.2, 0.0, 17.8),
    (-1.2, 1.2, 0.2, 18.0),
    (1.2, 3.6, 0.0, 17.6),
    (3.6, 6.0, 0.5, 17.2),
]
PLANK_Z = (0.0, 1.5)

BATTEN_Y = [(3.0, 5.0), (13.0, 15.0)]   # 背面双横撑
BAND_Y = [(5.8, 7.0), (11.0, 12.2)]     # 正面铁皮箍带


def build_cubes():
    cubes = []

    # ── planks —— 竖木板 ×5（长短参差,奇偶板厚度交替=浮雕感）────
    for i, (x0, x1, y0, y1) in enumerate(PLANKS):
        z1 = PLANK_Z[1] if i % 2 == 0 else PLANK_Z[1] - 0.25
        cubes.append(("planks", "wood", f"plank_{i}",
                      [x0, y0, PLANK_Z[0]], [x1, y1, z1]))

    # ── battens —— 背面双横撑（贴板背）──────────────────────────
    for tag, (y0, y1) in zip(("lo", "hi"), BATTEN_Y):
        cubes.append(("battens", "wood", f"batten_{tag}",
                      [-6.0, y0, -1.0], [6.0, y1, 0.0]))

    # ── bands —— 正面两道铁皮箍带（绕到左右侧沿）────────────────
    for tag, (y0, y1) in zip(("lo", "hi"), BAND_Y):
        cubes.append(("bands", "iron", f"band_{tag}",
                      [-6.2, y0, PLANK_Z[1] - 0.1], [6.2, y1, PLANK_Z[1] + 0.4]))
        for sx in (-1, 1):
            cubes.append(("bands", "iron", f"band_{tag}_edge{'_l' if sx < 0 else '_r'}",
                          [sx * 6.0 - (0.4 if sx < 0 else -0.4) - 0.4, y0, -0.3],
                          [sx * 6.0 + (0.4 if sx > 0 else -0.4) + 0.4, y1, PLANK_Z[1] + 0.3]))
        # 箍带铆钉（左右各一颗）
        for sx in (-1, 1):
            xc = sx * 3.4
            cubes.append(("bands", "iron", f"rivet_{tag}{'_l' if sx < 0 else '_r'}",
                          [xc - 0.35, (y0 + y1) / 2 - 0.35, PLANK_Z[1] + 0.4],
                          [xc + 0.35, (y0 + y1) / 2 + 0.35, PLANK_Z[1] + 0.75]))

    # ── boss —— 中心铁盾凸：方底座 + 八角中台 + 凸台 + 顶钉(四级收分=穹顶感)
    cubes.append(("boss", "iron", "boss_base",
                  [-2.4, 7.0, PLANK_Z[1] - 0.1], [2.4, 11.0, PLANK_Z[1] + 0.5]))
    cubes.append(("boss", "iron", "boss_mid",
                  [-1.8, 7.5, PLANK_Z[1] + 0.5], [1.8, 10.5, PLANK_Z[1] + 1.0]))
    cubes.append(("boss", "iron", "boss_dome",
                  [-1.1, 8.1, PLANK_Z[1] + 1.0], [1.1, 9.9, PLANK_Z[1] + 1.5]))
    cubes.append(("boss", "iron", "boss_stud",
                  [-0.45, 8.6, PLANK_Z[1] + 1.5], [0.45, 9.4, PLANK_Z[1] + 1.85]))

    # ── ropes —— 横撑端头草绳缠扎（包住板+撑）───────────────────
    for tag, (y0, y1) in zip(("lo", "hi"), BATTEN_Y):
        for sx, side in ((-1, "l"), (1, "r")):
            xc = sx * 4.8
            cubes.append(("ropes", "rope", f"rope_{tag}_{side}",
                          [xc - 0.6, y0 - 0.4, -1.2], [xc + 0.6, y1 + 0.4, PLANK_Z[1] + 0.2]))

    # ── handle —— 背面竖握把（两支座 + 横杆）────────────────────
    for tag, y0, y1 in (("top", 11.0, 12.2), ("bot", 5.8, 7.0)):
        cubes.append(("handle", "wood", f"grip_mount_{tag}",
                      [-0.8, y0, -2.0], [0.8, y1, -1.0]))
    cubes.append(("handle", "rope", "grip_bar",
                  [-0.7, 6.4, -3.0], [0.7, 11.6, -2.0]))

    return cubes


BONE_ORDER = ["planks", "battens", "bands", "boss", "ropes", "handle"]
BONE_PIVOTS = {b: [0.0, 9.0, 0.0] for b in BONE_ORDER}
BONE_COLORS = {
    "planks": (146, 110, 72),
    "battens": (118, 88, 56),
    "bands": (96, 100, 110),
    "boss": (120, 124, 134),
    "ropes": (164, 138, 84),
    "handle": (134, 104, 66),
}

MAT_ZONE = {
    "wood": (0, 0, RES, 36),
    "iron": (0, 36, RES, 50),
    "rope": (0, 50, RES, RES),
}


def make_texture(res=RES, seed=73):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 暖木 y[0,36)：竖板木纹（竖向 grain，板缝竖线）
    wmask = y < 36
    plank_w = 10
    pid = (x // plank_w).astype(float)
    seam = ((x % plank_w) < 1) | ((x % plank_w) > plank_w - 1.1)
    grain = 0.5 + 0.5 * np.sin(y * 0.45 + pid * 2.3 + np.sin(x * 0.25) * 0.8)
    wcol = np.array([146, 110, 72], float)[None, None, :] + (grain[..., None] - 0.5) * 30
    wcol += (rng.random((res, res, 1)) - 0.5) * 14
    wcol[seam] *= 0.6
    # 几个木节
    for _ in range(5):
        cx, cy = rng.integers(4, res - 4), rng.integers(3, 33)
        rr = ((x - cx) ** 2 + (y - cy) ** 2) < rng.integers(2, 5)
        wcol[rr] *= 0.65
    wcol = np.clip(wcol, 24, 200)
    img[wmask, :3] = wcol[wmask].astype(np.uint8)

    # 铁皮 y[36,50)：暗铁 + 铆钉点
    imask = (y >= 36) & (y < 50)
    icol = np.array([96, 100, 110], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 24
    icol += np.sin(x * 0.7 + y * 0.5)[..., None] * 7
    rivet = ((x % 12) < 2) & (((y - 36) % 7) < 2)
    icol[rivet] = 150.0
    icol = np.clip(icol, 30, 195)
    img[imask, :3] = icol[imask].astype(np.uint8)

    # 草绳 y[50,64)：斜向编织纹
    rmask = y >= 50
    twist = 0.5 + 0.5 * np.sin((x + (y - 50) * 1.6) * 1.1)
    rcol = np.array([164, 138, 84], float)[None, None, :] + (twist[..., None] - 0.5) * 44
    rcol += (rng.random((res, res, 1)) - 0.5) * 16
    rcol = np.clip(rcol, 50, 215)
    img[rmask, :3] = rcol[rmask].astype(np.uint8)

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
        "name": "wooden_shield", "model_identifier": "geometry.bong.wooden_shield",
        "visible_box": [1.5, 1.5, 0.6], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "wooden_shield.png", "folder": "item", "namespace": "bong",
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

    def ortho(ax_u, ax_v, title, flip_u=False):
        us = [v for _, _, _, f, t in cubes for v in (f[ax_u], t[ax_u])]
        vs = [v for _, _, _, f, t in cubes for v in (f[ax_v], t[ax_v])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (30, 30, 34, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(220, 220, 220))

        def to_px(u, v):
            uu = (umax - u) if flip_u else (u - umin)
            return pad + uu * scale, pad + 14 + ((vmax - vmin) * scale - (v - vmin) * scale)

        order = sorted(cubes, key=lambda c: c[3][3 - ax_u - ax_v])
        for bone, _, _, frm, to in order:
            x0, y0 = to_px(frm[ax_u], frm[ax_v])
            x1, y1 = to_px(to[ax_u], to[ax_v])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,), outline=(20, 16, 12, 255))
        return im

    front = ortho(0, 1, "FRONT (X-Y) 盾面")
    side = ortho(2, 1, "SIDE  (Z-Y) 厚度+握把")
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
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.95),
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
    d.text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x3) — wood/iron/rope",
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
    print("凡木盾 / wooden_shield:")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    p = render_preview(cubes, tex)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
