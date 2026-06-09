#!/usr/bin/env python3
"""草药筐（herb_crate）Blockbench .bbmodel 生成器。

plan-placeable-container-blocks-v1 P2 的可放置/随身容器之一（灵草筐，批量存灵草
少磨损，filter=Herb）。本脚本对齐延寿棺管线（gen_mundane_coffin.py 同款）：

    local_models/HerbCrate.bbmodel        ← 本脚本产物（Blockbench 源）
    → Blockbench 导出 → assets/bong/geo/herb_crate.geo.json
    → 贴图 assets/bong/textures/entity/herb_crate.png

视觉语言：藤编敞口筐 + 内衬粗布 + 草药露头（与货箱区分——开顶、能看进去、
自然柔和）。结构 = 藤编筐身（basket：4 壁 + 底，开顶）+ 加厚藤编筐沿（rim）+
内衬粗布唇边（liner）+ 草药束（herbs，下沉入筐、叶刚冒沿）。无盖（敞口）。

⚠️ 用户已在 Blockbench 手改本模型（HerbCrate.bbmodel 升 fmt5.0，草药下沉）。
   HERB_CUBES 已对齐其成品坐标。**本脚本默认只刷预览，不写 bbmodel**（保护手改
   源不被覆盖）；确需重写须显式 --write。

尺寸（MC 格，1 格 = 16px）：宽 0.88 × 高 0.81 × 深 0.88（对齐手改版）。

用法:
    python3 scripts/models/gen_herb_crate.py            # 仅刷预览（安全）
    python3 scripts/models/gen_herb_crate.py --write    # 覆盖手改 bbmodel（慎用）
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
BBMODEL_OUT = REPO / "local_models" / "HerbCrate.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "herb_crate_preview.png"

PX = 16.0
RES = 64

# ── 尺寸（MC 格）──────────────────────────────────────────────────
W_BLK = 0.88
HALF_W = W_BLK * PX / 2.0   # 7.04
HALF_D = HALF_W
H = 13.0

# ── 结构参数 ─────────────────────────────────────────────────────
WALL_T = 1.5
FLOOR_TOP = 2.5
WALL_TOP = 11.5
RIM_Y0, RIM_Y1 = 11.0, 13.0     # 加厚筐沿
RIM_OUT = 0.45                  # 筐沿外凸
LINER_Y0, LINER_Y1 = 8.5, 12.6  # 内衬粗布唇边（露出筐口）
LINER_T = 0.6

# 草药束（已对齐用户 Blockbench 手改版 HerbCrate.bbmodel fmt5.0：草药下沉入筐、
# 叶片刚冒筐沿，max y≈11 被筐沿 13 罩住，整体落进 0.81 格高）。
# 每束 = (stem_from, stem_to, leaf_from, leaf_to)，坐标直取自用户成品。
HERB_CUBES = [
    ([-3.8, 3.5, -2.4], [-3.0, 7.7, -1.6], [-4.7, 6.4, -3.3], [-2.1, 9.0, -0.7]),
    ([0.6, 3.5, 2.2], [1.4, 8.7, 3.0], [-0.5, 7.2, 1.1], [2.5, 10.2, 4.1]),
    ([2.8, 3.5, -3.2], [3.6, 6.9, -2.4], [2.1, 5.8, -3.9], [4.3, 8.0, -1.7]),
    ([-2.0, 3.5, 2.6], [-1.2, 7.6, 3.4], [-2.8, 6.4, 1.8], [-0.4, 8.8, 4.2]),
    ([-0.2, 3.5, -0.8], [0.6, 10.0, 0.0], [-0.8, 9.0, -1.4], [1.2, 11.0, 0.6]),
]


def build_cubes():
    cubes = []

    # ── basket —— 4 壁 + 底（开顶中空）─────────────────────────
    cubes.append(("basket", "wicker", "floor",
                  [-HALF_W, 0.0, -HALF_D], [HALF_W, FLOOR_TOP, HALF_D]))
    cubes.append(("basket", "wicker", "wall_front",
                  [-HALF_W, FLOOR_TOP, HALF_D - WALL_T], [HALF_W, WALL_TOP, HALF_D]))
    cubes.append(("basket", "wicker", "wall_back",
                  [-HALF_W, FLOOR_TOP, -HALF_D], [HALF_W, WALL_TOP, -HALF_D + WALL_T]))
    cubes.append(("basket", "wicker", "wall_left",
                  [-HALF_W, FLOOR_TOP, -HALF_D], [-HALF_W + WALL_T, WALL_TOP, HALF_D]))
    cubes.append(("basket", "wicker", "wall_right",
                  [HALF_W - WALL_T, FLOOR_TOP, -HALF_D], [HALF_W, WALL_TOP, HALF_D]))

    # ── rim —— 加厚藤编筐沿（4 边一圈，外凸）────────────────────
    ro = RIM_OUT
    cubes.append(("rim", "wicker", "rim_front",
                  [-HALF_W - ro, RIM_Y0, HALF_D - WALL_T - ro], [HALF_W + ro, RIM_Y1, HALF_D + ro]))
    cubes.append(("rim", "wicker", "rim_back",
                  [-HALF_W - ro, RIM_Y0, -HALF_D - ro], [HALF_W + ro, RIM_Y1, -HALF_D + WALL_T + ro]))
    cubes.append(("rim", "wicker", "rim_left",
                  [-HALF_W - ro, RIM_Y0, -HALF_D - ro], [-HALF_W + WALL_T + ro, RIM_Y1, HALF_D + ro]))
    cubes.append(("rim", "wicker", "rim_right",
                  [HALF_W - WALL_T - ro, RIM_Y0, -HALF_D - ro], [HALF_W + ro, RIM_Y1, HALF_D + ro]))

    # ── liner —— 内衬粗布唇边（贴壁内侧，露出筐口）──────────────
    li = WALL_T  # 内衬贴在壁内侧
    cubes.append(("liner", "cloth", "liner_front",
                  [-HALF_W + li, LINER_Y0, HALF_D - WALL_T - LINER_T], [HALF_W - li, LINER_Y1, HALF_D - WALL_T]))
    cubes.append(("liner", "cloth", "liner_back",
                  [-HALF_W + li, LINER_Y0, -HALF_D + WALL_T], [HALF_W - li, LINER_Y1, -HALF_D + WALL_T + LINER_T]))
    cubes.append(("liner", "cloth", "liner_left",
                  [-HALF_W + WALL_T, LINER_Y0, -HALF_D + li], [-HALF_W + WALL_T + LINER_T, LINER_Y1, HALF_D - li]))
    cubes.append(("liner", "cloth", "liner_right",
                  [HALF_W - WALL_T - LINER_T, LINER_Y0, -HALF_D + li], [HALF_W - WALL_T, LINER_Y1, HALF_D - li]))

    # ── herbs —— 草药束（对齐用户手改：下沉入筐、叶刚冒沿）────────
    for i, (sf, st, lf, lt) in enumerate(HERB_CUBES):
        cubes.append(("herbs", "herb", f"sprig_stem_{i}", list(sf), list(st)))
        cubes.append(("herbs", "herb", f"sprig_leaf_{i}", list(lf), list(lt)))

    return cubes


BONE_ORDER = ["basket", "rim", "liner", "herbs"]
BONE_PIVOTS = {
    "basket": [0.0, 0.0, 0.0],
    "rim": [0.0, 0.0, 0.0],
    "liner": [0.0, 0.0, 0.0],
    "herbs": [0.0, WALL_TOP, 0.0],
}
BONE_COLORS = {
    "basket": (186, 156, 104),
    "rim": (160, 128, 78),
    "liner": (198, 190, 172),
    "herbs": (96, 142, 70),
}


# ── 三材质贴图（64×64：藤编 y[0,40) / 粗布 y[40,52) / 草药 y[52,64)）─
def make_texture(res=RES, seed=23):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 藤编 y[0,40)：横向编纹 + 经纬 over/under
    wk_h = 40
    wmask = y < wk_h
    over_under = ((x // 3 + y // 4) % 2).astype(float)  # 经纬交错
    band = 0.5 + 0.5 * np.sin(y * 0.8)                  # 横向藤条
    wk = np.array([190, 158, 106], float)[None, None, :]
    wcol = wk + (over_under[..., None] - 0.5) * 30 + (band[..., None] - 0.5) * 18
    wcol += (rng.random((res, res, 1)) - 0.5) * 12
    # 藤条间缝压暗
    seam = (y % 4) < 0.8
    wcol[seam] *= 0.7
    wcol = np.clip(wcol, 30, 222)
    img[wmask, :3] = wcol[wmask].astype(np.uint8)

    # 粗布 y[40,52)：亚麻米白 + 织线噪点
    cl_lo, cl_hi = 40, 52
    cmask = (y >= cl_lo) & (y < cl_hi)
    cl = np.array([198, 190, 172], float)[None, None, :]
    ccol = cl + (rng.random((res, res, 1)) - 0.5) * 18
    thread = ((x % 2 == 0)[..., None]).astype(float) * 6
    ccol += thread
    ccol = np.clip(ccol, 60, 226)
    img[cmask, :3] = ccol[cmask].astype(np.uint8)

    # 草药 y[52,64)：叶绿 + 叶脉 + 色变
    hmask = y >= cl_hi
    hb = np.array([96, 142, 70], float)[None, None, :]
    hcol = hb + (rng.random((res, res, 1)) - 0.5) * 26
    vein = 0.5 + 0.5 * np.sin(x * 1.3 + y * 0.4)
    hcol[..., 1:2] += (vein[..., None]) * 14   # 绿通道随叶脉变
    hcol = np.clip(hcol, 30, 200)
    img[hmask, :3] = hcol[hmask].astype(np.uint8)

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


MAT_ZONE = {"wicker": (0, 0, RES, 40), "cloth": (0, 40, RES, 52), "herb": (0, 52, RES, RES)}


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
        "name": "herb_crate", "model_identifier": "geometry.bong.herb_crate",
        "visible_box": [2, 2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "herb_crate.png", "folder": "entity", "namespace": "bong",
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
    top = ortho(0, 2, "TOP   (X-Z) 开口")

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
        d.text((pad, 3), "ISO", fill=(220, 220, 220))

        def to_px(p):
            return pad + (p[0] - umin) * scale, pad + 14 + (p[1] - vmin) * scale

        order = sorted(cubes, key=lambda c: (c[3][0] + c[3][2] + c[3][1]))
        for bone, _, _, frm, to in order:
            x0, y0, z0 = frm
            x1, y1, z1 = to
            col = BONE_COLORS[bone]
            for verts, k in [
                ([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.18),
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.82),
                ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.6),
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
    d.text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x3)", fill=(200, 200, 200))
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
    ap.add_argument("--write", action="store_true",
                    help="覆盖 local_models/HerbCrate.bbmodel（默认只刷预览，保护用户手改源）")
    args = ap.parse_args()

    model, cubes, tex = build_bbmodel()
    print("草药筐 / herb_crate:")
    summarize(cubes)
    if args.write:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  ⚠️ 已覆盖手改源 → {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    else:
        print("  (默认仅刷预览，未写 bbmodel；保护用户 Blockbench 手改源。如确需覆盖用 --write)")
    p = render_preview(cubes, tex)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
