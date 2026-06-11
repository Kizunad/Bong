#!/usr/bin/env python3
"""货箱（trade_crate）Blockbench .bbmodel 生成器。

plan-placeable-container-blocks-v1 P2 的可放置世界容器之一（货箱，集市搬运用，
通用仓储 accept=[] 全收）。本脚本对齐延寿棺管线（gen_mundane_coffin.py 同款）：

    local_models/TradeCrate.bbmodel       ← 本脚本产物（Blockbench 源）
    → Blockbench 导出 → assets/bong/geo/trade_crate.geo.json
    → 贴图 assets/bong/textures/entity/trade_crate.png

视觉语言：六面木板钉成的厚重货箱 + 四角铁件加固（worldview 末法集市搬运）。
结构 = 实心板箱身（panels）+ 外包木框（frame：4 角柱 + 上下横档）+ 掀盖（lid，
铰链在后上长棱，可做开盖搜索动画）+ 4 角铁角件（iron，加固）。

尺寸（MC 格，1 格 = 16px）：宽 0.94 × 高 0.94 × 深 0.94，近 1 格立方大箱。

用法:
    python3 scripts/models/gen_trade_crate.py
    python3 scripts/models/gen_trade_crate.py --preview-only
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
BBMODEL_OUT = REPO / "local_models" / "TradeCrate.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "trade_crate_preview.png"

PX = 16.0
RES = 64

# ── 尺寸（MC 格）──────────────────────────────────────────────────
W_BLK = 0.94
H_BLK = 0.94
D_BLK = 0.94
HALF_W = W_BLK * PX / 2.0   # 7.52
HALF_D = D_BLK * PX / 2.0   # 7.52
H = H_BLK * PX              # 15.04

# ── 结构参数 ─────────────────────────────────────────────────────
BODY_INSET = 0.7        # 箱身相对外框内收（露出外框=笼状）
BODY_TOP = 13.0         # 箱身顶（盖子在其上）
POST_T = 2.0            # 角柱见方
BATTEN_T = 0.7          # 横档凸出箱身的厚度
BATTEN_H = 2.2          # 横档高
IRON_T = 0.6            # 铁件凸出
IRON_SZ = 3.0           # 角铁件见方

BW = HALF_W - BODY_INSET   # 箱身外半宽 6.82
BD = HALF_D - BODY_INSET   # 箱身外半深 6.82

LID_Y0 = BODY_TOP          # 盖底 13.0
LID_Y1 = H                 # 盖顶 15.04

# 横档三道（底/中/顶）的 y 区间
BATTEN_BANDS = [
    (0.4, 0.4 + BATTEN_H),          # 底档
    (H / 2 - BATTEN_H / 2, H / 2 + BATTEN_H / 2),  # 中档
    (BODY_TOP - BATTEN_H, BODY_TOP),               # 顶档
]


def build_cubes():
    """返回 (bone, material, name, from[xyz], to[xyz])。material ∈ {wood, iron}。"""
    cubes = []

    # ── panels —— 实心板箱身（略内收，留外框露出）──────────────
    cubes.append(("panels", "wood", "body",
                  [-BW, 0.0, -BD], [BW, BODY_TOP, BD]))

    # ── frame —— 4 角柱（外包，full height）─────────────────────
    for sx in (-1, 1):
        for sz in (-1, 1):
            xa = HALF_W - POST_T if sx > 0 else -HALF_W
            xb = HALF_W if sx > 0 else -HALF_W + POST_T
            za = HALF_D - POST_T if sz > 0 else -HALF_D
            zb = HALF_D if sz > 0 else -HALF_D + POST_T
            cubes.append(("frame", "wood", f"post_{sx}_{sz}",
                          [xa, 0.0, za], [xb, BODY_TOP, zb]))

    # ── frame —— 上中下三道横档，绕 4 面一圈（凸出箱身）──────────
    for bi, (y0, y1) in enumerate(BATTEN_BANDS):
        # 前(+Z)/后(-Z) 面：沿 X 跨两柱之间，z 落在外框面
        for sz in (1, -1):
            zr = [BD, HALF_D] if sz > 0 else [-HALF_D, -BD]
            cubes.append(("frame", "wood", f"batten_z{sz}_{bi}",
                          [-HALF_W + POST_T, y0, zr[0]], [HALF_W - POST_T, y1, zr[1]]))
        # 左(-X)/右(+X) 面：沿 Z 跨两柱之间
        for sx in (1, -1):
            xr = [BW, HALF_W] if sx > 0 else [-HALF_W, -BW]
            cubes.append(("frame", "wood", f"batten_x{sx}_{bi}",
                          [xr[0], y0, -HALF_D + POST_T], [xr[1], y1, HALF_D - POST_T]))

    # ── iron —— 8 角铁件（上下各 4，包角加固，向外凸出 IRON_T）────
    for sx in (-1, 1):
        for sz in (-1, 1):
            x_in = HALF_W - IRON_SZ if sx > 0 else -HALF_W + IRON_SZ
            x_out = HALF_W + IRON_T if sx > 0 else -HALF_W - IRON_T
            z_in = HALF_D - IRON_SZ if sz > 0 else -HALF_D + IRON_SZ
            z_out = HALF_D + IRON_T if sz > 0 else -HALF_D - IRON_T
            xa, xb = min(x_in, x_out), max(x_in, x_out)
            za, zb = min(z_in, z_out), max(z_in, z_out)
            cubes.append(("iron", "iron", f"iron_top_{sx}_{sz}",
                          [xa, BODY_TOP - IRON_SZ, za], [xb, BODY_TOP + 0.2, zb]))
            cubes.append(("iron", "iron", f"iron_bot_{sx}_{sz}",
                          [xa, -0.2, za], [xb, IRON_SZ, zb]))

    # ── lid —— 掀盖：盖板 + 顶上两道压条（铰链在后上长棱）────────
    cubes.append(("lid", "wood", "lid_board",
                  [-HALF_W + 0.3, LID_Y0, -HALF_D + 0.3], [HALF_W - 0.3, LID_Y0 + 1.4, HALF_D - 0.3]))
    for sx in (-1, 1):
        xc = sx * (HALF_W - 2.4)
        cubes.append(("lid", "wood", f"lid_rib_{sx}",
                      [xc - 0.9, LID_Y0 + 1.4, -HALF_D + 1.0], [xc + 0.9, LID_Y1, HALF_D - 1.0]))
    # 盖前沿一道铁扣（搭扣视觉）
    cubes.append(("lid", "iron", "lid_latch",
                  [-1.6, LID_Y0 + 0.2, HALF_D - 0.6], [1.6, LID_Y0 + 1.6, HALF_D + 0.3]))

    return cubes


BONE_ORDER = ["panels", "frame", "iron", "lid"]
BONE_PIVOTS = {
    "panels": [0.0, 0.0, 0.0],
    "frame": [0.0, 0.0, 0.0],
    "iron": [0.0, 0.0, 0.0],
    # 铰链：后上长棱（-Z 后侧），绕 X 轴向后掀盖
    "lid": [0.0, BODY_TOP, -HALF_D + 0.3],
}
BONE_COLORS = {
    "panels": (158, 120, 78),
    "frame": (128, 94, 58),
    "iron": (96, 100, 110),
    "lid": (176, 138, 92),
}


# ── 双材质贴图（64×64：上 48 行木纹 / 下 16 行铁件）────────────────
def make_texture(res=RES, seed=11):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 木纹区 y[0,48)：新松木竖板纹，比寿材浅
    wood_h = 48
    wmask = y < wood_h
    plank_w = 8
    plank_id = (x // plank_w).astype(float)
    seam = ((x % plank_w) < 1) | ((x % plank_w) > plank_w - 1.1)
    grain = 0.5 + 0.5 * np.sin(y * 0.5 + plank_id * 1.9)
    grain += 0.22 * np.sin(y * 0.19 + plank_id * 3.3)
    grain = np.clip(grain, 0, 1)
    plank_tone = (rng.random(res // plank_w + 2) - 0.5) * 22
    tone = plank_tone[plank_id.astype(int)]
    wbase = np.array([168, 132, 86], float)  # 新松木黄褐
    wcol = wbase[None, None, :] + (grain[..., None] - 0.5) * 30 + tone[..., None]
    wcol[seam] *= 0.6
    for _ in range(5):
        cx, cy = rng.integers(3, res - 3), rng.integers(3, wood_h - 3)
        r = rng.integers(2, 4)
        d = np.hypot(x - cx, y - cy)
        wcol[d < r] *= 0.52
        wcol[(d >= r) & (d < r + 1)] *= 0.76
    wcol = np.clip(wcol, 20, 220)
    img[wmask, :3] = wcol[wmask].astype(np.uint8)

    # 铁件区 y[48,64)：暗冷灰金属 + 铆钉 + 划痕
    imask = ~wmask
    ibase = np.array([84, 88, 98], float)
    metal = ibase[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 26
    # 横向锻打划痕
    metal += (np.sin(x * 0.8 + y * 0.3)[..., None]) * 6
    # 铆钉点
    for cx in range(6, res, 14):
        for cy in range(wood_h + 4, res, 8):
            d = np.hypot(x - cx, y - cy)
            metal[d < 1.6] += 40
            metal[(d >= 1.6) & (d < 2.4)] -= 24
    metal = np.clip(metal, 24, 210)
    img[imask, :3] = metal[imask].astype(np.uint8)

    return Image.fromarray(img, "RGBA")


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class Packer:
    """分区货架打包：在 [x0,y0,x1,y1) 内逐面摆放，溢出回绕复用（材质均匀无碍）。"""

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
    dims = {
        "north": (dx, dy), "south": (dx, dy),
        "east": (dz, dy), "west": (dz, dy),
        "up": (dx, dz), "down": (dx, dz),
    }
    faces = {}
    for name, (w, h) in dims.items():
        ox, oy = packer.place(abs(w), abs(h))
        faces[name] = {"uv": [round(ox, 2), round(oy, 2),
                              round(ox + abs(w), 2), round(oy + abs(h), 2)],
                       "texture": 0}
    return faces


def build_bbmodel():
    cubes = build_cubes()
    wood_packer = Packer(0, 0, RES, 48)
    iron_packer = Packer(0, 48, RES, RES)
    elements = []
    bone_children = {b: [] for b in BONE_ORDER}

    for bone, material, name, frm, to in cubes:
        euid = str(uuid.uuid4())
        packer = iron_packer if material == "iron" else wood_packer
        elements.append({
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": euid,
            "from": [round(v, 3) for v in frm],
            "to": [round(v, 3) for v in to],
            "autouv": 0,
            "color": BONE_ORDER.index(bone),
            "origin": list(BONE_PIVOTS[bone]),
            "faces": cube_faces_uv(frm, to, packer),
        })
        bone_children[bone].append(euid)

    outliner = []
    for bone in BONE_ORDER:
        outliner.append({
            "name": bone,
            "origin": list(BONE_PIVOTS[bone]),
            "color": BONE_ORDER.index(bone),
            "uuid": str(uuid.uuid4()),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children[bone],
        })

    tex = make_texture()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "trade_crate",
        "model_identifier": "geometry.bong.trade_crate",
        "visible_box": [2, 2, 0.5],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "", "name": "trade_crate.png", "folder": "entity",
            "namespace": "bong", "id": "0", "width": RES, "height": RES,
            "uv_width": RES, "uv_height": RES, "particle": False,
            "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


# ── 预览：三视图 + 等距（按骨骼上色）────────────────────────────────
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
            pu = (u - umin) * scale
            pv = (vmax - vmin) * scale - (v - vmin) * scale
            return pad + pu, pad + 14 + pv

        order = sorted(cubes, key=lambda c: c[3][3 - ax_u - ax_v])
        for bone, _, _, frm, to in order:
            x0, y0 = to_px(frm[ax_u], frm[ax_v])
            x1, y1 = to_px(to[ax_u], to[ax_v])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,),
                        outline=(20, 16, 12, 255))
        return im

    front = ortho(0, 1, "FRONT (X-Y)")
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
        d.text((pad, 3), "ISO", fill=(220, 220, 220))

        def to_px(p):
            return pad + (p[0] - umin) * scale, pad + 14 + (p[1] - vmin) * scale

        order = sorted(cubes, key=lambda c: (c[3][0] + c[3][2] + c[3][1]))
        for bone, _, _, frm, to in order:
            x0, y0, z0 = frm
            x1, y1, z1 = to
            col = BONE_COLORS[bone]
            faces = [
                ([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.18),
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.82),
                ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.6),
            ]
            for verts, k in faces:
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
    legend = "  ".join(f"{b}" for b in BONE_ORDER)
    d.text((gap, H_ - 16), "bones: " + legend, fill=(180, 180, 180))
    canvas.save(out)
    return out


def summarize(cubes):
    xs = [v for _, _, _, f, t in cubes for v in (f[0], t[0])]
    ys = [v for _, _, _, f, t in cubes for v in (f[1], t[1])]
    zs = [v for _, _, _, f, t in cubes for v in (f[2], t[2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox  : {bb[0]:.1f}×{bb[1]:.1f}×{bb[2]:.1f}px = "
          f"{bb[0]/PX:.3f}W × {bb[1]/PX:.3f}H × {bb[2]/PX:.3f}D 格")
    print(f"  target: {W_BLK}W × {H_BLK}H × {D_BLK}D 格")
    print(f"  cubes : {len(cubes)}  ("
          + ", ".join(f"{b}:{sum(1 for c in cubes if c[0]==b)}" for b in BONE_ORDER) + ")")
    print(f"  lid hinge: {BONE_PIVOTS['lid']} (绕 X 后掀)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    model, cubes, tex = build_bbmodel()
    print("货箱 / trade_crate:")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    p = render_preview(cubes, tex)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
