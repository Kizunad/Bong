#!/usr/bin/env python3
"""寒玉棺（进阶延寿棺 / jade_coffin）Blockbench .bbmodel 生成器。

plan-coffin-v1 §遗留：灵材棺材（×0.7/×0.5/×0.3 按灵材等级）。寒玉棺为灵材
一档（×0.7），世界观内核「玉养身防腐」（金缕玉衣意象）→ 最贴延寿主题。

与凡物棺（MundaneCoffin，暗红漆木圆盖+枕木）整体换风格：
  · 须弥座束腰底（非枕木）
  · 玉壁带上下边框 molding（panel 感）
  · 玉板盖 + 中脊 + 头端莲蕾/宝珠宝顶（非粗木圆盖）
  · 通体青碧玉 + 灵脉流光贴图

管线同凡物档：
  local_models/JadeCoffin.bbmodel → 导出 assets/bong/geo/jade_coffin.geo.json
  (geometry.bong.jade_coffin) → 贴图 textures/entity/jade_coffin_intact.png

尺寸同凡物档 envelope：长 1.8 / 宽 0.9 / 高 0.9 格，独立可开盖 lid。
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
from PIL import Image, ImageDraw, ImageFilter

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = REPO / "local_models" / "JadeCoffin.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "jade_coffin_preview.png"

PX = 16.0
LEN_BLK, WID_BLK, HGT_BLK = 1.8, 0.9, 0.9
L, W, H = LEN_BLK * PX, WID_BLK * PX, HGT_BLK * PX
HALF_L, HALF_W = L / 2.0, W / 2.0
RES = 64

# ── 结构参数（寒玉棺）─────────────────────────────────────────────
HEAD_Z, FOOT_Z = -HALF_L, HALF_L          # 大头 -Z / 小头 +Z
BOARD_T = 2.0
TUB_Z0, TUB_Z1 = HEAD_Z + BOARD_T, FOOT_Z - BOARD_T   # ±12.4

# 须弥座（下枋 / 束腰 / 上枋）: (half_w, y0, y1, z_half)
PEDESTAL = [
    (7.2, 0.0, 1.6, 14.4),   # 下枋（满底，最宽）
    (6.2, 1.6, 3.2, 13.4),   # 束腰（内收）
    (7.0, 3.2, 4.4, 13.9),   # 上枋（外挑）
]
BODY_Y0 = 4.4               # 棺身坐落高度

BODY_HALF_W = 6.6          # 棺壁外半宽（内收于上枋 → 露座沿）
WALL_T = 1.0               # 玉壁较薄精致
WALL_TOP = 7.6
MOLD_PROUD = 6.9           # 边框 molding 外凸半宽
HEAD_TOP = 10.0            # 大头玉板高
HEAD_CAP_TOP = 10.8        # 大头顶 molding
FOOT_TOP = 6.6             # 小头玉板高
FOOT_HALF_W = 4.6          # 小头半宽（尾小，更清晰）

# 玉板盖
LID_SLAB_Y0, LID_SLAB_Y1 = 7.6, 9.0
LID_SLAB_HALF_W = 6.9      # 出檐
LID_SLAB_Z = 13.0
RIM_Y1 = 9.6               # 盖面边框 rim 顶
RIM_IN = 6.3              # 边框内沿（中间凹陷 panel）

# 中脊（偏头）+ 头端莲蕾宝顶: 各 (half_w, y0, y1, z_head, z_foot)
LID_SPINE = [
    (2.6, 9.0, 10.6, -13.0, 9.0),
    (1.6, 10.6, 11.8, -13.0, 4.0),
]
LID_FINIAL = [
    (2.2, 11.8, 12.6, -13.0, -8.6),   # 莲座
    (1.5, 12.6, 13.4, -12.6, -9.2),   # 莲蕾
    (0.8, 13.4, H,   -12.2, -9.8),    # 宝珠尖（峰 = 14.4）
]

BONE_ORDER = ["base", "body", "lid"]
BONE_PIVOTS = {
    "base": [0.0, 0.0, 0.0],
    "body": [0.0, BODY_Y0, 0.0],
    "lid": [-LID_SLAB_HALF_W + 0.3, WALL_TOP, 0.0],   # 左上长棱铰链，侧开盖
}
BONE_COLORS = {  # 预览青碧玉调
    "base": (150, 186, 176),
    "body": (176, 210, 198),
    "lid": (202, 230, 220),
}


def build_cubes():
    c = []

    # ── base —— 须弥座束腰底 ──────────────────────────────────
    for i, (hw, y0, y1, zh) in enumerate(PEDESTAL):
        c.append(("base", f"pedestal{i}", [-hw, y0, -zh], [hw, y1, zh]))

    # ── body —— 玉棺身（带边框 molding，开顶中空）─────────────
    c.append(("body", "floor", [-BODY_HALF_W, BODY_Y0, TUB_Z0], [BODY_HALF_W, BODY_Y0 + 1.4, TUB_Z1]))
    # 两侧玉壁
    c.append(("body", "wall_L", [-BODY_HALF_W, BODY_Y0 + 1.4, TUB_Z0], [-BODY_HALF_W + WALL_T, WALL_TOP, TUB_Z1]))
    c.append(("body", "wall_R", [BODY_HALF_W - WALL_T, BODY_Y0 + 1.4, TUB_Z0], [BODY_HALF_W, WALL_TOP, TUB_Z1]))
    # 边框 molding（上下横 + 头脚竖 → 完整回字框，阴刻 panel）
    for side, sx in (("L", -1), ("R", 1)):
        xo, xi = sorted((sx * BODY_HALF_W, sx * MOLD_PROUD))
        c.append(("body", f"mold_top_{side}", [xo, WALL_TOP - 0.8, TUB_Z0], [xi, WALL_TOP, TUB_Z1]))
        c.append(("body", f"mold_bot_{side}", [xo, BODY_Y0 + 1.4, TUB_Z0], [xi, BODY_Y0 + 2.2, TUB_Z1]))
        for zc, tag in ((TUB_Z0, "h"), (TUB_Z1 - 1.2, "f")):
            c.append(("body", f"vrail_{side}{tag}", [xo, BODY_Y0 + 2.2, zc], [xi, WALL_TOP - 0.8, zc + 1.2]))
    # 大头玉板（头大）+ 顶 molding
    c.append(("body", "head_board", [-BODY_HALF_W, BODY_Y0, HEAD_Z, ], [BODY_HALF_W, HEAD_TOP, TUB_Z0]))
    c.append(("body", "head_cap", [-BODY_HALF_W + 1.2, HEAD_TOP, HEAD_Z + 0.4], [BODY_HALF_W - 1.2, HEAD_CAP_TOP, TUB_Z0]))
    # 小头玉板（尾小）
    c.append(("body", "foot_board", [-FOOT_HALF_W, BODY_Y0, TUB_Z1], [FOOT_HALF_W, FOOT_TOP, FOOT_Z]))

    # ── lid —— 玉板盖 + 边框 rim + 中脊 + 莲蕾宝顶（可掀盖）────
    c.append(("lid", "lid_slab", [-LID_SLAB_HALF_W, LID_SLAB_Y0, -LID_SLAB_Z], [LID_SLAB_HALF_W, LID_SLAB_Y1, LID_SLAB_Z]))
    # 盖面边框 rim（四边外圈凸起，中央凹 panel）
    c.append(("lid", "rim_L", [-LID_SLAB_HALF_W, LID_SLAB_Y1, -LID_SLAB_Z], [-RIM_IN, RIM_Y1, LID_SLAB_Z]))
    c.append(("lid", "rim_R", [RIM_IN, LID_SLAB_Y1, -LID_SLAB_Z], [LID_SLAB_HALF_W, RIM_Y1, LID_SLAB_Z]))
    c.append(("lid", "rim_head", [-RIM_IN, LID_SLAB_Y1, -LID_SLAB_Z], [RIM_IN, RIM_Y1, -LID_SLAB_Z + 0.8]))
    c.append(("lid", "rim_foot", [-RIM_IN, LID_SLAB_Y1, LID_SLAB_Z - 0.8], [RIM_IN, RIM_Y1, LID_SLAB_Z]))
    # 中脊
    for i, (hw, y0, y1, zh, zf) in enumerate(LID_SPINE):
        c.append(("lid", f"spine{i}", [-hw, y0, zh], [hw, y1, zf]))
    # 头端莲蕾宝顶
    for i, (hw, y0, y1, zh, zf) in enumerate(LID_FINIAL):
        c.append(("lid", f"finial{i}", [-hw, y0, zh], [hw, y1, zf]))

    return c


# ── 青碧玉 + 灵脉流光贴图 ─────────────────────────────────────────
def make_jade_texture(res=RES, seed=11):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    n = (np.sin(x * 0.21 + y * 0.13)
         + np.sin(x * 0.07 - y * 0.19 + 1.3)
         + np.sin((x + y) * 0.11 + 2.1)) / 3.0
    n = (n + 1) / 2
    base = np.array([146, 196, 178], float)   # 青碧玉（更冷绿）
    hi = np.array([214, 238, 228], float)     # 白玉高光
    col = base[None, None, :] * (1 - n[..., None]) + hi[None, None, :] * n[..., None]
    col += (rng.random((res, res, 1)) - 0.5) * 9   # 细斑

    # 灵脉流光：随机游走亮脉，分「核心」（锐亮）+「辉晕」（外发光）两层
    core = Image.new("L", (res, res), 0)
    cd = ImageDraw.Draw(core)

    def vein(px, py, steps, turn=0.8, seg=4.2, w=1):
        ang = rng.random() * math.tau
        pts = [(px, py)]
        for _ in range(steps):
            ang += (rng.random() - 0.5) * turn
            px += math.cos(ang) * seg
            py += math.sin(ang) * seg
            pts.append((px, py))
        cd.line(pts, fill=255, width=w)

    for _ in range(7):   # 细脉
        vein(float(rng.integers(0, res)), float(rng.integers(0, res)), int(rng.integers(10, 18)))
    for _ in range(2):   # 主脉（更长更亮）
        vein(float(rng.integers(0, res)), float(rng.integers(0, res)), int(rng.integers(18, 26)), turn=0.5, w=1)

    halo = np.asarray(core.filter(ImageFilter.GaussianBlur(2.2)), float) / 255.0
    sharp = np.asarray(core.filter(ImageFilter.GaussianBlur(0.5)), float) / 255.0
    vcol = np.array([150, 245, 230], float)    # 灵脉青光
    vhot = np.array([232, 255, 250], float)    # 脉芯近白
    col = col + halo[..., None] * (vcol[None, None, :] - col) * 0.7   # 外辉
    col = col + sharp[..., None] * (vhot[None, None, :] - col) * 0.95  # 脉芯

    col = np.clip(col, 40, 255).astype(np.uint8)
    img = np.dstack([col, np.full((res, res), 255, np.uint8)])
    return Image.fromarray(img, "RGBA")


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class ShelfPacker:
    def __init__(self, res=RES):
        self.res = res
        self.x = self.y = self.rowh = 0.0

    def place(self, w, h):
        w, h = min(w, self.res), min(h, self.res)
        if self.x + w > self.res:
            self.x, self.y, self.rowh = 0.0, self.y + self.rowh, 0.0
        if self.y + h > self.res:
            self.y = 0.0
        ox, oy = self.x, self.y
        self.x += w
        self.rowh = max(self.rowh, h)
        return ox, oy


def cube_faces_uv(frm, to, packer):
    dx, dy, dz = (to[i] - frm[i] for i in range(3))
    dims = {"north": (dx, dy), "south": (dx, dy), "east": (dz, dy),
            "west": (dz, dy), "up": (dx, dz), "down": (dx, dz)}
    faces = {}
    for name, (w, h) in dims.items():
        ox, oy = packer.place(w, h)
        faces[name] = {"uv": [round(ox, 2), round(oy, 2), round(ox + w, 2), round(oy + h, 2)], "texture": 0}
    return faces


def build_bbmodel():
    cubes = build_cubes()
    packer = ShelfPacker(RES)
    elements = []
    bone_children = {b: [] for b in BONE_ORDER}
    for bone, name, frm, to in cubes:
        euid = str(uuid.uuid4())
        elements.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False,
            "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
            "uuid": euid, "from": [round(v, 3) for v in frm], "to": [round(v, 3) for v in to],
            "autouv": 0, "color": BONE_ORDER.index(bone), "origin": list(BONE_PIVOTS[bone]),
            "faces": cube_faces_uv(frm, to, packer),
        })
        bone_children[bone].append(euid)
    outliner = [{
        "name": bone, "origin": list(BONE_PIVOTS[bone]), "color": BONE_ORDER.index(bone),
        "uuid": str(uuid.uuid4()), "export": True, "mirror_uv": False, "isOpen": True,
        "locked": False, "visibility": True, "autouv": 0, "children": bone_children[bone],
    } for bone in BONE_ORDER]
    tex = make_jade_texture()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "jade_coffin", "model_identifier": "geometry.bong.jade_coffin",
        "visible_box": [3, 2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "jade_coffin.png", "folder": "entity", "namespace": "bong",
            "id": "0", "width": RES, "height": RES, "uv_width": RES, "uv_height": RES,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


def render_preview(cubes, tex, out=PREVIEW_OUT):
    scale, pad, gap = 7, 16, 24

    def lit(c, k):
        return tuple(int(np.clip(v * k, 0, 255)) for v in c)

    def ortho(au, av, title):
        us = [v for _, _, f, t in cubes for v in (f[au], t[au])]
        vs = [v for _, _, f, t in cubes for v in (f[av], t[av])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (28, 30, 32, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(220, 230, 228))

        def tp(u, v):
            return pad + (u - umin) * scale, pad + 14 + ((vmax - vmin) - (v - vmin)) * scale
        for bone, _, f, t in sorted(cubes, key=lambda c: c[2][3 - au - av]):
            x0, y0 = tp(f[au], f[av])
            x1, y1 = tp(t[au], t[av])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,), outline=(40, 60, 56, 255))
        return im

    front = ortho(0, 1, "FRONT (X-Y) head@back")
    side = ortho(2, 1, "SIDE  (Z-Y) head<- ->foot")
    top = ortho(0, 2, "TOP   (X-Z) taper")

    def iso():
        ca, sa = math.cos(math.radians(30)), math.sin(math.radians(30))

        def proj(x, y, z):
            return (x - z) * ca, (x + z) * sa - y
        pts = [proj(X, Y, Z) for _, _, f, t in cubes
               for X in (f[0], t[0]) for Y in (f[1], t[1]) for Z in (f[2], t[2])]
        umin, umax = min(p[0] for p in pts), max(p[0] for p in pts)
        vmin, vmax = min(p[1] for p in pts), max(p[1] for p in pts)
        im = Image.new("RGBA", (int((umax - umin) * scale) + pad * 2, int((vmax - vmin) * scale) + pad * 2 + 14), (28, 30, 32, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), "ISO", fill=(220, 230, 228))

        def tp(p):
            return pad + (p[0] - umin) * scale, pad + 14 + (p[1] - vmin) * scale
        for bone, _, f, t in sorted(cubes, key=lambda c: c[2][0] + c[2][1] + c[2][2]):
            x0, y0, z0 = f
            x1, y1, z1 = t
            col = BONE_COLORS[bone]
            for verts, k in (([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.15),
                             ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.82),
                             ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.64)):
                d.polygon([tp(proj(*v)) for v in verts], fill=lit(col, k) + (255,), outline=(40, 60, 56, 255))
        return im

    iso_im = iso()
    top_row = [front, side, top]
    th = max(t.height for t in top_row)
    tex_big = tex.resize((RES * 2, RES * 2), Image.NEAREST)
    W_ = max(sum(t.width for t in top_row) + gap * 4, iso_im.width + tex_big.width + gap * 3)
    H_ = th + max(iso_im.height, tex_big.height) + gap * 3
    canvas = Image.new("RGBA", (W_, H_), (16, 18, 20, 255))
    xx = gap
    for t in top_row:
        canvas.paste(t, (xx, gap), t)
        xx += t.width + gap
    canvas.paste(iso_im, (gap, th + gap * 2), iso_im)
    canvas.paste(tex_big, (gap * 2 + iso_im.width, th + gap * 2), tex_big)
    ImageDraw.Draw(canvas).text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x2)", fill=(200, 210, 208))
    canvas.save(out)
    return out


def summarize(cubes):
    xs = [v for _, _, f, t in cubes for v in (f[0], t[0])]
    ys = [v for _, _, f, t in cubes for v in (f[1], t[1])]
    zs = [v for _, _, f, t in cubes for v in (f[2], t[2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox: {bb[0]:.2f}×{bb[1]:.2f}×{bb[2]:.2f}px = {bb[0]/PX:.3f}W {bb[1]/PX:.3f}H {bb[2]/PX:.3f}L 格")
    print(f"  target: {W:.1f}×{H:.1f}×{L:.1f}px = {WID_BLK}×{HGT_BLK}×{LEN_BLK} 格")
    print(f"  cubes: {len(cubes)} (" + ", ".join(f"{b}:{sum(1 for c in cubes if c[0]==b)}" for b in BONE_ORDER) + ")")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()
    model, cubes, tex = build_bbmodel()
    print("寒玉棺 / jade_coffin:")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    print(f"  → preview: {render_preview(cubes, tex).relative_to(REPO)}")


if __name__ == "__main__":
    main()
