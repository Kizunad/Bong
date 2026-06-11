#!/usr/bin/env python3
"""真实渲染 .bbmodel —— 读几何 + UV + 内嵌贴图，z-buffer 遮挡 + 纹理采样 + 方向光。

不同于各 gen_*.py 里的示意预览（平涂盒子、画家排序），本脚本忠实还原模型实际长相：
逐面光栅化、深度缓冲正确遮挡、按 UV 采样内嵌贴图（最近邻，像素风）、按法线方向光着色。
支持 element 的 origin+rotation（读得了 Blockbench 存盘的 fmt 5.0 文件）。

用法:
  python3 scripts/models/render_bbmodel.py local_models/BronzeCoffin.bbmodel
  python3 scripts/models/render_bbmodel.py --all              # 四档拼一张
  python3 scripts/models/render_bbmodel.py <file> --yaw -35 --pitch 22
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
MODELS = REPO / "local_models"
OUTDIR = REPO / "scripts" / "models"

# 每面：4 角(取 min=f/max=t 的分量) + 法线。角序 = uv 的 TL,TR,BR,BL。
FACES = {
    "west":  (lambda f, t: [(f[0], t[1], f[2]), (f[0], t[1], t[2]), (f[0], f[1], t[2]), (f[0], f[1], f[2])], (-1, 0, 0)),
    "east":  (lambda f, t: [(t[0], t[1], t[2]), (t[0], t[1], f[2]), (t[0], f[1], f[2]), (t[0], f[1], t[2])], (1, 0, 0)),
    "down":  (lambda f, t: [(f[0], f[1], t[2]), (t[0], f[1], t[2]), (t[0], f[1], f[2]), (f[0], f[1], f[2])], (0, -1, 0)),
    "up":    (lambda f, t: [(f[0], t[1], f[2]), (t[0], t[1], f[2]), (t[0], t[1], t[2]), (f[0], t[1], t[2])], (0, 1, 0)),
    "north": (lambda f, t: [(t[0], t[1], f[2]), (f[0], t[1], f[2]), (f[0], f[1], f[2]), (t[0], f[1], f[2])], (0, 0, -1)),
    "south": (lambda f, t: [(f[0], t[1], t[2]), (t[0], t[1], t[2]), (t[0], f[1], t[2]), (f[0], f[1], t[2])], (0, 0, 1)),
}


def _rotmat(deg, axis):
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    if axis == 0:
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    if axis == 1:
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def load_bbmodel(path):
    d = json.loads(Path(path).read_text())
    res = d.get("resolution", {"width": 64, "height": 64})
    rw, rh = res["width"], res["height"]
    src = d["textures"][0].get("source", "")
    if src.startswith("data:"):
        src = src.split(",", 1)[1]
    tex = np.asarray(Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA"), float)

    tris = []  # (verts[3x3], uvs[3x2], normal[3])
    for e in d["elements"]:
        if e.get("type", "cube") != "cube":
            continue
        f, t = np.array(e["from"], float), np.array(e["to"], float)
        faces = e.get("faces", {})
        rot = e.get("rotation", [0, 0, 0])
        org = np.array(e.get("origin", [0, 0, 0]), float)
        Rm = None
        if any(rot):
            Rm = _rotmat(rot[2], 2) @ _rotmat(rot[1], 1) @ _rotmat(rot[0], 0)
        for fname, (corner_fn, normal) in FACES.items():
            fd = faces.get(fname)
            if not fd:
                continue
            uv = fd["uv"]                       # [u1,v1,u2,v2]
            u1, v1, u2, v2 = uv
            cs = [np.array(c, float) for c in corner_fn(f, t)]
            uvs = [(u1, v1), (u2, v1), (u2, v2), (u1, v2)]
            n = np.array(normal, float)
            if Rm is not None:
                cs = [Rm @ (c - org) + org for c in cs]
                n = Rm @ n
            # 四角 → 两三角 (0,1,2),(0,2,3)
            for a, b in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[a], cs[b]]),
                             np.array([uvs[0], uvs[a], uvs[b]]), n))
    return tris, tex, (rw, rh), d.get("name", Path(path).stem)


def render(path, yaw=-35.0, pitch=22.0, size=600, bg=(22, 23, 26), light=(-0.35, 0.6, 0.72)):
    tris, tex, (rw, rh), name = load_bbmodel(path)
    th, tw = tex.shape[:2]
    R = _rotmat(pitch, 0) @ _rotmat(yaw, 1)
    ld = np.array(light, float)
    ld /= np.linalg.norm(ld)

    # 视空间
    allv = np.array([v for tri in tris for v in tri[0]])
    center = (allv.min(0) + allv.max(0)) / 2
    view = [( (R @ (vs - center).T).T, uvs, R @ n) for vs, uvs, n in tris]

    vv = np.array([v for vs, _, _ in view for v in vs])
    mn, mx = vv[:, :2].min(0), vv[:, :2].max(0)
    span = (mx - mn).max()
    scale = (size - 60) / span
    off = (np.array([size, size]) - (mx + mn) * scale * np.array([1, -1])) / 2

    img = np.zeros((size, size, 3), float)
    img[:] = bg
    zbuf = np.full((size, size), -1e9)

    def to_screen(v):
        sx = v[:, 0] * scale + off[0]
        sy = -v[:, 1] * scale + off[1]
        return np.stack([sx, sy, v[:, 2]], 1)

    for vs, uvs, n in view:
        if n[2] <= 0.02:          # 背面剔除（朝向相机 +z 才画）
            continue
        shade = 0.32 + 0.68 * max(0.0, float(n @ ld))
        p = to_screen(vs)
        xs, ys = p[:, 0], p[:, 1]
        x0, x1 = int(max(0, math.floor(xs.min()))), int(min(size - 1, math.ceil(xs.max())))
        y0, y1 = int(max(0, math.floor(ys.min()))), int(min(size - 1, math.ceil(ys.max())))
        if x0 > x1 or y0 > y1:
            continue
        ax, ay = p[0, :2]
        bx, by = p[1, :2]
        cx, cy = p[2, :2]
        area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay)
        if abs(area) < 1e-6:
            continue
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1), np.arange(y0, y1 + 1))
        gx = gx + 0.5
        gy = gy + 0.5
        w0 = ((bx - ax) * (gy - ay) - (by - ay) * (gx - ax)) / area
        w1 = ((cx - bx) * (gy - by) - (cy - by) * (gx - bx)) / area
        w2 = 1 - w0 - w1
        inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
        if not inside.any():
            continue
        # 重心对应三角 (0->A? ) — w2,w0,w1 对应顶点 0,1,2
        b0, b1, b2 = w2, w0, w1
        depth = b0 * p[0, 2] + b1 * p[1, 2] + b2 * p[2, 2]
        u = b0 * uvs[0, 0] + b1 * uvs[1, 0] + b2 * uvs[2, 0]
        v = b0 * uvs[0, 1] + b1 * uvs[1, 1] + b2 * uvs[2, 1]
        sub_z = zbuf[y0:y1 + 1, x0:x1 + 1]
        win = inside & (depth > sub_z)
        if not win.any():
            continue
        ui = np.clip(u.astype(int), 0, tw - 1)
        vi = np.clip(v.astype(int), 0, th - 1)
        col = tex[vi, ui, :3] * shade
        sub_img = img[y0:y1 + 1, x0:x1 + 1]
        sub_img[win] = col[win]
        sub_z[win] = depth[win]

    return Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)), name


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file", nargs="?")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--yaw", type=float, default=-35.0)
    ap.add_argument("--pitch", type=float, default=22.0)
    ap.add_argument("--size", type=int, default=600)
    args = ap.parse_args()

    if args.all:
        names = ["MundaneCoffin", "JadeCoffin", "StoneCoffin", "BronzeCoffin"]
        tiles = []
        for nm in names:
            fp = MODELS / f"{nm}.bbmodel"
            if not fp.exists():
                continue
            im, _ = render(fp, args.yaw, args.pitch, size=420)
            tiles.append((nm, im))
        gap = 16
        W = sum(t[1].width for t in tiles) + gap * (len(tiles) + 1)
        Hh = max(t[1].height for t in tiles) + gap * 2 + 18
        cv = Image.new("RGB", (W, Hh), (14, 15, 17))
        from PIL import ImageDraw
        d = ImageDraw.Draw(cv)
        x = gap
        for nm, im in tiles:
            cv.paste(im, (x, gap + 18))
            d.text((x + 4, 6), nm, fill=(220, 220, 210))
            x += im.width + gap
        out = OUTDIR / "coffins_render_all.png"
        cv.save(out)
        print(f"→ {out.relative_to(REPO)}")
        return

    fp = Path(args.file) if args.file else MODELS / "BronzeCoffin.bbmodel"
    im, name = render(fp, args.yaw, args.pitch, size=args.size)
    out = OUTDIR / f"render_{Path(fp).stem}.png"
    im.save(out)
    print(f"→ {out.relative_to(REPO)}  ({name}, yaw={args.yaw} pitch={args.pitch})")


if __name__ == "__main__":
    main()
