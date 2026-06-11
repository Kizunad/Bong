#!/usr/bin/env python3
"""玄石阵棺（进阶延寿棺 灵材二档 / stone_coffin）Blockbench .bbmodel 生成器。

plan-coffin-v1 §遗留：灵材棺材（×0.7/×0.5/×0.3）。玄石阵棺 = 灵材二档（×0.5）。
内核：阵法主动封存灵气（强于寒玉的被动养护），对应正典「符文/阵纹」方向。

四档延寿棺风格递进（每档独立生成器，互不覆盖）：
  ×0.9 凡物棺 MundaneCoffin —— 暗红漆木圆盖 + 枕木（暖棕·朴拙）
  ×0.7 寒玉棺 JadeCoffin     —— 青碧玉须弥座 + 灵脉流光（冷绿·精致）
  ×0.5 玄石阵棺 StoneCoffin  —— 厚重石椁 + 四角镇石 + 阵眼 + 金色阵纹（深灰·阵法）
  ×0.3 （待定，留青铜上古神器收尾）

与 TSY 的 StoneCasket（容器搜刮）无关——本件是延寿棺。

管线同前：local_models/StoneCoffin.bbmodel → assets/bong/geo/stone_coffin.geo.json
(geometry.bong.stone_coffin) → textures/entity/stone_coffin_intact.png。
尺寸同 envelope：1.8×0.9×0.9 格，独立可开盖 lid。
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
BBMODEL_OUT = REPO / "local_models" / "StoneCoffin.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "stone_coffin_preview.png"

PX = 16.0
LEN_BLK, WID_BLK, HGT_BLK = 1.8, 0.9, 0.9
L, W, H = LEN_BLK * PX, WID_BLK * PX, HGT_BLK * PX
HALF_L, HALF_W = L / 2.0, W / 2.0
RES = 64

# ── 结构参数（玄石阵棺）─────────────────────────────────────────
HEAD_Z, FOOT_Z = -HALF_L, HALF_L
WALL_T = 1.6              # 石壁厚重
WALL_IN = 5.0            # 石壁内沿（中空开口 ±5.0，玩家躺其内）
WALL_OUT = 6.6           # 石壁外沿
WALL_TOP = 8.5           # 石椁口沿
WALL_TOP_FOOT = 7.8      # 小头口沿略低
BODY_Y0 = 3.2            # 石椁坐落（石基之上）
TUB_Z0, TUB_Z1 = HEAD_Z + WALL_T, FOOT_Z - WALL_T  # ±12.8 端壁内缘

POST_OUT = 6.8           # 镇石柱外半宽
POST_IN = 4.9            # 镇石柱内半宽
POST_TOP_HEAD = 11.0     # 头侧镇石柱高
POST_TOP_FOOT = 9.6      # 脚侧镇石柱矮（头大尾小）
CAP_TOP = 11.8           # 镇石柱顶帽

LID_TOP = 10.2           # 石板盖顶（嵌在石椁口内，四角镇石高于它）
# 阵眼（顶心法阵核，逐级收成发光尖顶）: (half_w, y0, y1, z0, z1)
ZHENYAN = [
    (3.6, 10.2, 11.2, -8.0, 6.0),   # 阵台
    (2.4, 11.2, 12.3, -5.0, 3.0),   # 阵盘
    (1.5, 12.3, 13.3, -3.2, 1.2),   # 核座
    (0.8, 13.3, H,    -2.4, 0.4),   # 阵眼核（峰 = 14.4）
]

GLOW = (255, 198, 96)    # 金色阵纹辉光（区别寒玉青光）

BONE_ORDER = ["base", "body", "lid"]
BONE_PIVOTS = {
    "base": [0.0, 0.0, 0.0],
    "body": [0.0, BODY_Y0, 0.0],
    "lid": [-WALL_IN, WALL_TOP, 0.0],   # 石盖左长棱铰链，侧掀
}
BONE_COLORS = {  # 预览深板岩
    "base": (66, 72, 84),
    "body": (86, 92, 104),
    "lid": (104, 110, 122),
}
CORE_COLOR = (236, 188, 96)  # 预览：阵眼核高亮


def build_cubes():
    c = []  # (bone, name, from, to[, preview_color])

    # ── base —— 厚重石基（两级）────────────────────────────────
    c.append(("base", "base_slab", [-HALF_W, 0.0, -HALF_L], [HALF_W, 2.0, HALF_L]))
    c.append(("base", "base_step", [-6.8, 2.0, -13.8], [6.8, BODY_Y0, 13.8]))

    # ── body —— 石椁（厚壁中空）+ 四角镇石柱 ───────────────────
    c.append(("body", "floor", [-WALL_OUT, BODY_Y0, TUB_Z0], [WALL_OUT, BODY_Y0 + 1.6, TUB_Z1]))
    c.append(("body", "wall_L", [-WALL_OUT, BODY_Y0 + 1.6, TUB_Z0], [-WALL_IN, WALL_TOP, TUB_Z1]))
    c.append(("body", "wall_R", [WALL_IN, BODY_Y0 + 1.6, TUB_Z0], [WALL_OUT, WALL_TOP, TUB_Z1]))
    c.append(("body", "wall_head", [-WALL_OUT, BODY_Y0 + 1.6, HEAD_Z], [WALL_OUT, WALL_TOP, TUB_Z0]))
    c.append(("body", "wall_foot", [-WALL_OUT, BODY_Y0 + 1.6, TUB_Z1], [WALL_OUT, WALL_TOP_FOOT, FOOT_Z]))

    # 四角镇石柱（头高脚矮）+ 顶帽
    posts = [
        ("HL", -POST_OUT, -POST_IN, HEAD_Z, HEAD_Z + 1.9, POST_TOP_HEAD),
        ("HR", POST_IN, POST_OUT, HEAD_Z, HEAD_Z + 1.9, POST_TOP_HEAD),
        ("FL", -POST_OUT, -POST_IN, FOOT_Z - 1.9, FOOT_Z, POST_TOP_FOOT),
        ("FR", POST_IN, POST_OUT, FOOT_Z - 1.9, FOOT_Z, POST_TOP_FOOT),
    ]
    for tag, x0, x1, z0, z1, top in posts:
        c.append(("body", f"post_{tag}", [x0, BODY_Y0 + 1.6, z0], [x1, top, z1]))
        # 顶帽仅 X 外挑（Z 留在 envelope 内，避免超长）
        c.append(("body", f"cap_{tag}", [x0 - 0.2, top, z0], [x1 + 0.2, CAP_TOP, z1]))

    # ── lid —— 石板盖（嵌口内）+ 阵纹联线 + 顶心阵眼（可掀盖）──
    c.append(("lid", "lid_slab", [-WALL_IN, WALL_TOP, TUB_Z0], [WALL_IN, LID_TOP, TUB_Z1]))
    # 阵纹联线（十字，阵眼 → 石椁四边，发光金纹）
    ay0, ay1 = LID_TOP, LID_TOP + 0.6
    arms = [
        ("arm_head", [-0.7, ay0, TUB_Z0], [0.7, ay1, -8.0]),
        ("arm_foot", [-0.7, ay0, 6.0], [0.7, ay1, TUB_Z1]),
        ("arm_xL", [-WALL_IN, ay0, -0.7], [-3.6, ay1, 0.7]),
        ("arm_xR", [3.6, ay0, -0.7], [WALL_IN, ay1, 0.7]),
    ]
    for name, f, t in arms:
        c.append(("lid", name, f, t, GLOW))
    # 顶心阵眼（四级发光尖顶）
    for i, (hw, y0, y1, z0, z1) in enumerate(ZHENYAN):
        col = CORE_COLOR if i == len(ZHENYAN) - 1 else None
        cube = ("lid", f"zhenyan{i}", [-hw, y0, z0], [hw, y1, z1])
        c.append(cube + (col,) if col else cube)

    return c


# ── 深板岩 + 金色几何阵纹贴图 ────────────────────────────────────
def make_stone_texture(res=RES, seed=23):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    n = (np.sin(x * 0.27 + y * 0.18) + np.sin(x * 0.11 - y * 0.23 + 1.0)
         + np.sin((x - y) * 0.09 + 2.4)) / 3.0
    n = (n + 1) / 2
    base = np.array([70, 78, 92], float)     # 深板岩（冷灰蓝）
    dark = np.array([44, 50, 62], float)
    col = dark[None, None, :] * (1 - n[..., None]) + base[None, None, :] * n[..., None]
    col += (rng.random((res, res, 1)) - 0.5) * 14   # 颗粒

    # 裂纹（暗、锐、随机游走）
    cracks = Image.new("L", (res, res), 0)
    cd = ImageDraw.Draw(cracks)
    for _ in range(4):
        px, py = float(rng.integers(0, res)), float(rng.integers(0, res))
        ang = rng.random() * math.tau
        pts = [(px, py)]
        for _ in range(int(rng.integers(8, 14))):
            ang += (rng.random() - 0.5) * 1.1
            px += math.cos(ang) * 5
            py += math.sin(ang) * 5
            pts.append((px, py))
        cd.line(pts, fill=255, width=1)
    cm = np.asarray(cracks.filter(ImageFilter.GaussianBlur(0.4)), float) / 255.0
    col *= (1 - cm[..., None] * 0.55)

    # 金色阵纹：满铺几何格栅 + 中心法阵 + 节点（区别寒玉有机灵脉）
    f = Image.new("L", (res, res), 0)
    fd = ImageDraw.Draw(f)
    for gx in range(8, res, 16):       # 纵横格栅
        fd.line([(gx, 0), (gx, res)], fill=120, width=1)
    for gy in range(8, res, 16):
        fd.line([(0, gy), (res, gy)], fill=120, width=1)
    cx = cy = res // 2
    fd.rectangle([cx - 22, cy - 22, cx + 22, cy + 22], outline=255, width=1)      # 阵框
    fd.polygon([(cx, cy - 20), (cx + 20, cy), (cx, cy + 20), (cx - 20, cy)],
               outline=255)                                                        # 内菱
    fd.ellipse([cx - 11, cy - 11, cx + 11, cy + 11], outline=255, width=1)        # 阵环
    fd.ellipse([cx - 3, cy - 3, cx + 3, cy + 3], fill=255)                         # 阵眼
    for dx, dy in ((-1, -1), (1, -1), (1, 1), (-1, 1)):                            # 放射
        fd.line([(cx, cy), (cx + dx * 22, cy + dy * 22)], fill=200, width=1)
    for nx, ny in ((cx, 8), (cx, res - 8), (8, cy), (res - 8, cy)):                # 边节点
        fd.ellipse([nx - 2, ny - 2, nx + 2, ny + 2], fill=255)

    halo = np.asarray(f.filter(ImageFilter.GaussianBlur(1.6)), float) / 255.0
    sharp = np.asarray(f.filter(ImageFilter.GaussianBlur(0.4)), float) / 255.0
    g = np.array(GLOW, float)
    ghot = np.array([255, 236, 180], float)
    col = col + halo[..., None] * (g[None, None, :] - col) * 0.6
    col = col + sharp[..., None] * (ghot[None, None, :] - col) * 0.9

    col = np.clip(col, 18, 255).astype(np.uint8)
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
    for cube in cubes:
        bone, name, frm, to = cube[0], cube[1], cube[2], cube[3]
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
    tex = make_stone_texture()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "stone_coffin", "model_identifier": "geometry.bong.stone_coffin",
        "visible_box": [3, 2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "stone_coffin.png", "folder": "entity", "namespace": "bong",
            "id": "0", "width": RES, "height": RES, "uv_width": RES, "uv_height": RES,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


def render_preview(cubes, tex, out=PREVIEW_OUT):
    scale, pad, gap = 7, 16, 24

    def pcol(cube):
        return cube[4] if len(cube) > 4 else BONE_COLORS[cube[0]]

    def lit(c, k):
        return tuple(int(np.clip(v * k, 0, 255)) for v in c)

    def ortho(au, av, title):
        us = [v for c in cubes for v in (c[2][au], c[3][au])]
        vs = [v for c in cubes for v in (c[2][av], c[3][av])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        im = Image.new("RGBA", (int((umax - umin) * scale) + pad * 2, int((vmax - vmin) * scale) + pad * 2 + 14), (26, 27, 30, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(214, 210, 196))

        def tp(u, v):
            return pad + (u - umin) * scale, pad + 14 + ((vmax - vmin) - (v - vmin)) * scale
        for cube in sorted(cubes, key=lambda c: c[2][3 - au - av]):
            f, t = cube[2], cube[3]
            x0, y0 = tp(f[au], f[av])
            x1, y1 = tp(t[au], t[av])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(pcol(cube), 1.0) + (255,), outline=(30, 30, 34, 255))
        return im

    front = ortho(0, 1, "FRONT (X-Y) head@back")
    side = ortho(2, 1, "SIDE  (Z-Y) head<- ->foot")
    top = ortho(0, 2, "TOP   (X-Z) 阵纹/4角镇石")

    def iso():
        ca, sa = math.cos(math.radians(30)), math.sin(math.radians(30))

        def proj(x, y, z):
            return (x - z) * ca, (x + z) * sa - y
        pts = [proj(X, Y, Z) for c in cubes for X in (c[2][0], c[3][0]) for Y in (c[2][1], c[3][1]) for Z in (c[2][2], c[3][2])]
        umin, umax = min(p[0] for p in pts), max(p[0] for p in pts)
        vmin, vmax = min(p[1] for p in pts), max(p[1] for p in pts)
        im = Image.new("RGBA", (int((umax - umin) * scale) + pad * 2, int((vmax - vmin) * scale) + pad * 2 + 14), (26, 27, 30, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), "ISO", fill=(214, 210, 196))

        def tp(p):
            return pad + (p[0] - umin) * scale, pad + 14 + (p[1] - vmin) * scale
        for cube in sorted(cubes, key=lambda c: c[2][0] + c[2][1] + c[2][2]):
            x0, y0, z0 = cube[2]
            x1, y1, z1 = cube[3]
            col = pcol(cube)
            for verts, k in (([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.18),
                             ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.8),
                             ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.6)):
                d.polygon([tp(proj(*v)) for v in verts], fill=lit(col, k) + (255,), outline=(30, 30, 34, 255))
        return im

    iso_im = iso()
    top_row = [front, side, top]
    th = max(t.height for t in top_row)
    tex_big = tex.resize((RES * 2, RES * 2), Image.NEAREST)
    W_ = max(sum(t.width for t in top_row) + gap * 4, iso_im.width + tex_big.width + gap * 3)
    H_ = th + max(iso_im.height, tex_big.height) + gap * 3
    canvas = Image.new("RGBA", (W_, H_), (15, 16, 18, 255))
    xx = gap
    for t in top_row:
        canvas.paste(t, (xx, gap), t)
        xx += t.width + gap
    canvas.paste(iso_im, (gap, th + gap * 2), iso_im)
    canvas.paste(tex_big, (gap * 2 + iso_im.width, th + gap * 2), tex_big)
    ImageDraw.Draw(canvas).text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x2)", fill=(200, 196, 184))
    canvas.save(out)
    return out


def summarize(cubes):
    xs = [v for c in cubes for v in (c[2][0], c[3][0])]
    ys = [v for c in cubes for v in (c[2][1], c[3][1])]
    zs = [v for c in cubes for v in (c[2][2], c[3][2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox: {bb[0]:.2f}×{bb[1]:.2f}×{bb[2]:.2f}px = {bb[0]/PX:.3f}W {bb[1]/PX:.3f}H {bb[2]/PX:.3f}L 格")
    print(f"  cubes: {len(cubes)} (" + ", ".join(f"{b}:{sum(1 for c in cubes if c[0]==b)}" for b in BONE_ORDER) + ")")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()
    model, cubes, tex = build_bbmodel()
    print("玄石阵棺 / stone_coffin (灵材二档 ×0.5):")
    summarize(cubes)
    if not args.preview_only:
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    print(f"  → preview: {render_preview(cubes, tex).relative_to(REPO)}")


if __name__ == "__main__":
    main()
