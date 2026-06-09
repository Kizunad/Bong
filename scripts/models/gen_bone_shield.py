#!/usr/bin/env python3
"""骨盾（bone_shield）Blockbench .bbmodel 生成器。

plan-shield-block-v1 的手持盾牌资产之二。worldview：末法残土骨遍地（骨币
经济），兽骨是凡人最易得的硬质材料——骨盾即"兽脊为梁、肋骨为骨架、皮革
绷面、筋腱缠扎"的凡人级物理防御，比凡木盾更耐打。

手持物品最终走 SML+OBJ 管线（client/tools/tripo_to_sml.py 同类产物），本
bbmodel 是设计/手改源：

    local_models/BoneShield.bbmodel       ← 本脚本产物（Blockbench 源）
    → Blockbench 手改 / 导出 OBJ → assets/bong/models/item/bone_shield/

视觉语言：中央兽脊椎柱（5 节椎骨交替宽窄，前凸）+ 左右各 3 根肋骨横档
（长短参差、外端关节球）+ 皮革绷面背板 + 椎肋交接处筋腱缠扎 + 脊柱上下
端骨节头 + 背面握把。整体竖立在 XY 面，正面朝 +Z。

尺寸（MC px）：宽 ~14 × 高 18 × 厚 ~5.6（含脊凸与背部握把）。

用法:
    python3 scripts/models/gen_bone_shield.py
    python3 scripts/models/gen_bone_shield.py --preview-only
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
BBMODEL_OUT = REPO / "local_models" / "BoneShield.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "bone_shield_preview.png"

PX = 16.0
RES = 64

BACK_Z = (0.0, 1.2)          # 皮革绷面背板厚度
RIB_Z = (1.2, 2.2)           # 肋骨层
SPINE_Z = (1.2, 2.8)         # 脊柱层（最前凸）

# 5 节椎骨：y0, y1, 半宽（交替宽窄）
VERTEBRAE = [
    (1.6, 4.0, 1.2),
    (4.4, 7.0, 1.0),
    (7.4, 10.2, 1.3),
    (10.6, 13.2, 1.0),
    (13.6, 16.2, 1.2),
]

# 3 对主肋骨：y 中心, 半高, 外端 x（长短参差）
RIBS = [
    (4.6, 0.9, 6.4),
    (8.9, 1.0, 7.0),
    (13.2, 0.9, 6.0),
]
# 上下两对短肋（收轮廓成椭圆剪影）：y 中心, 半高, 外端 x
SHORT_RIBS = [
    (15.9, 0.7, 4.2),
    (1.9, 0.7, 4.0),
]


def build_cubes():
    cubes = []

    # ── backing —— 皮革绷面背板（略小于骨架轮廓）────────────────
    cubes.append(("backing", "leather", "hide_main",
                  [-5.6, 1.2, BACK_Z[0]], [5.6, 16.8, BACK_Z[1]]))
    cubes.append(("backing", "leather", "hide_top",
                  [-3.6, 16.8, BACK_Z[0]], [3.6, 17.8, BACK_Z[1]]))
    cubes.append(("backing", "leather", "hide_bot",
                  [-3.6, 0.2, BACK_Z[0]], [3.6, 1.2, BACK_Z[1]]))

    # ── spine —— 中央椎骨柱 ×5（交替宽窄,前凸最高层）────────────
    for i, (y0, y1, hw) in enumerate(VERTEBRAE):
        cubes.append(("spine", "bone", f"vertebra_{i}",
                      [-hw, y0, SPINE_Z[0]], [hw, y1, SPINE_Z[1]]))
    # 脊柱上下端骨节头（关节球，比椎骨宽）
    cubes.append(("spine", "bone", "condyle_top",
                  [-1.7, 16.2, SPINE_Z[0] - 0.2], [1.7, 18.0, SPINE_Z[1] + 0.2]))
    cubes.append(("spine", "bone", "condyle_bot",
                  [-1.7, 0.0, SPINE_Z[0] - 0.2], [1.7, 1.6, SPINE_Z[1] + 0.2]))

    # ── ribs —— 左右各 3 根肋骨横档 + 外端关节球 ────────────────
    for i, (yc, hh, xout) in enumerate(RIBS):
        for sx, side in ((-1, "l"), (1, "r")):
            cubes.append(("ribs", "bone", f"rib_{i}_{side}",
                          [min(sx * 1.0, sx * xout), yc - hh, RIB_Z[0]],
                          [max(sx * 1.0, sx * xout), yc + hh, RIB_Z[1]]))
            cubes.append(("ribs", "bone", f"rib_knob_{i}_{side}",
                          [sx * xout - 0.7, yc - hh - 0.3, RIB_Z[0] - 0.1],
                          [sx * xout + 0.7, yc + hh + 0.3, RIB_Z[1] + 0.2]))

    # ── ribs —— 上下短肋（无关节球,端头略细=收形）────────────────
    for i, (yc, hh, xout) in enumerate(SHORT_RIBS):
        for sx, side in ((-1, "l"), (1, "r")):
            cubes.append(("ribs", "bone", f"short_rib_{i}_{side}",
                          [min(sx * 1.0, sx * xout), yc - hh, RIB_Z[0]],
                          [max(sx * 1.0, sx * xout), yc + hh, RIB_Z[1] - 0.2]))

    # ── lashings —— 椎肋交接处筋腱缠扎（包住脊柱两侧肋根）───────
    for i, (yc, hh, _) in enumerate(RIBS):
        for sx, side in ((-1, "l"), (1, "r")):
            xc = sx * 1.7
            cubes.append(("lashings", "sinew", f"lash_{i}_{side}",
                          [xc - 0.55, yc - hh - 0.4, RIB_Z[0] - 0.15],
                          [xc + 0.55, yc + hh + 0.4, RIB_Z[1] + 0.3]))

    # ── handle —— 背面竖握把（两支座 + 筋腱缠把）────────────────
    for tag, y0, y1 in (("top", 11.2, 12.4), ("bot", 5.6, 6.8)):
        cubes.append(("handle", "bone", f"grip_mount_{tag}",
                      [-0.8, y0, -1.0], [0.8, y1, 0.0]))
    cubes.append(("handle", "sinew", "grip_bar",
                  [-0.7, 6.2, -2.0], [0.7, 11.8, -1.0]))

    return cubes


BONE_ORDER = ["backing", "spine", "ribs", "lashings", "handle"]
BONE_PIVOTS = {b: [0.0, 9.0, 0.0] for b in BONE_ORDER}
BONE_COLORS = {
    "backing": (96, 72, 52),
    "spine": (228, 216, 194),
    "ribs": (212, 198, 174),
    "lashings": (122, 88, 58),
    "handle": (150, 116, 80),
}

MAT_ZONE = {
    "bone": (0, 0, RES, 34),
    "leather": (0, 34, RES, 52),
    "sinew": (0, 52, RES, RES),
}


def make_texture(res=RES, seed=89):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 骨白 y[0,34)：骨质纵纹 + 裂纹 + 微黄渍
    bmask = y < 34
    grain = 0.5 + 0.5 * np.sin(x * 0.35 + np.sin(y * 0.2) * 1.4)
    bcol = np.array([228, 216, 194], float)[None, None, :] + (grain[..., None] - 0.5) * 18
    bcol += (rng.random((res, res, 1)) - 0.5) * 10
    # 黄渍斑
    for _ in range(6):
        cx, cy = rng.integers(4, res - 4), rng.integers(3, 31)
        rr = ((x - cx) ** 2 + (y - cy) ** 2) < rng.integers(4, 12)
        bcol[rr] = bcol[rr] * 0.92 + np.array([16, 8, -10])
    # 细裂纹（短折线）
    for _ in range(7):
        cx, cy = rng.integers(3, res - 6), rng.integers(2, 30)
        ln = rng.integers(3, 7)
        for k in range(ln):
            xx = np.clip(cx + k, 0, res - 1)
            yy = np.clip(cy + (k % 2), 0, 33)
            bcol[yy, xx] *= 0.7
    bcol = np.clip(bcol, 90, 245)
    img[bmask, :3] = bcol[bmask].astype(np.uint8)

    # 皮革 y[34,52)：鞣面斑驳 + 绷缝点线
    lmask = (y >= 34) & (y < 52)
    lcol = np.array([96, 72, 52], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 22
    lcol += np.sin(x * 0.5 + y * 0.7)[..., None] * 8
    stitch = (((x + 3) % 11) < 1.5) & (((y - 34) % 9) < 1.5)
    lcol[stitch] = 140.0
    lcol = np.clip(lcol, 34, 165)
    img[lmask, :3] = lcol[lmask].astype(np.uint8)

    # 筋腱 y[52,64)：紧密斜缠纹（比草绳更细更暗）
    smask = y >= 52
    twist = 0.5 + 0.5 * np.sin((x + (y - 52) * 2.0) * 1.6)
    scol = np.array([122, 88, 58], float)[None, None, :] + (twist[..., None] - 0.5) * 36
    scol += (rng.random((res, res, 1)) - 0.5) * 12
    scol = np.clip(scol, 40, 180)
    img[smask, :3] = scol[smask].astype(np.uint8)

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
        "name": "bone_shield", "model_identifier": "geometry.bong.bone_shield",
        "visible_box": [1.5, 1.5, 0.6], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "bone_shield.png", "folder": "item", "namespace": "bong",
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

    front = ortho(0, 1, "FRONT (X-Y) 骨架面")
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
    d.text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x3) — bone/leather/sinew",
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
    print("骨盾 / bone_shield:")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    p = render_preview(cubes, tex)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
