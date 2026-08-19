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
THREE_VIEW_ANGLES = (
    ("FRONT", 180.0, 0.0),
    ("SIDE", 90.0, 0.0),
    ("3/4", 145.0, 15.0),
)

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


def load_bbmodel(path, xform=None):
    """xform: {element uuid: 4x4 刚体矩阵}，在元素自身 origin+rotation 之后再叠。

    骨骼动画预览用——本模块只认元素级旋转，骨树是无视的；摆姿时由调用方（rig.py）
    算好每个元素的世界矩阵传进来，省得把姿态烘回 from/to（轴对齐盒装不下任意旋转）。
    """
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
            M = xform.get(e.get("uuid")) if xform else None
            if M is not None:
                cs = [M[:3, :3] @ c + M[:3, 3] for c in cs]
                # 法线走**逆转置**，不是同一个矩阵。纯旋转时两者相同（历史上只喂过旋转，
                # 所以一直没暴露），一旦 xform 带非均匀缩放就完全不同：法线被转歪、长度也
                # 跟着缩放变，而下面的着色用的是未归一化的 n·ld —— 被压扁那一轴上的面会
                # 亮成纯白。羽层的收展动画给羽做轴向缩放（9.5 / 1.6 / 0.17），第一次踩到。
                A = M[:3, :3]
                n = np.linalg.inv(A).T @ n if abs(np.linalg.det(A)) > 1e-9 else A @ n
            ln = float(np.linalg.norm(n))
            if ln > 1e-9:
                n = n / ln
            # 四角 → 两三角 (0,1,2),(0,2,3)
            for a, b in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[a], cs[b]]),
                             np.array([uvs[0], uvs[a], uvs[b]]), n))
    return tris, tex, (rw, rh), d.get("name", Path(path).stem)


# MC 原版的面亮度是按**朝向查表**的，不是 lambert：上 1.0 / 下 0.5 / 南北(±z) 0.8 /
# 东西(±x) 0.6。相邻朝向最多差 1.33 倍。而本模块默认的 lambert 光会把 +x 夹到 0.32、
# +z 给到 0.80 —— 差 2.5 倍。判"体素球够不够圆滑"这类问题时，默认光会把阶梯面的
# 明暗差放大一倍，看起来一身条纹，进游戏其实没那么糟。要贴近游戏观感就用 shading="mc"。
MC_FACE_SHADE = ((0.60, 0.60), (1.00, 0.50), (0.80, 0.80))  # 每轴 (正向, 负向)


def _mc_shade(n):
    axis = int(max(range(3), key=lambda i: abs(n[i])))
    return MC_FACE_SHADE[axis][0 if n[axis] >= 0 else 1]


def render(path, yaw=-35.0, pitch=22.0, size=600, bg=(22, 23, 26), light=(-0.35, 0.6, 0.72),
           xform=None, focus=None, shading="lambert"):
    """focus: (center3, span) 固定取景——逐帧自动取景会让动画整体抖动（每帧包围盒不同）。

    shading: "lambert"（默认，保持既有行为）或 "mc"（按 MC 原版面亮度表，
    预览更贴近进游戏后的实际观感）。
    """
    tris, tex, (rw, rh), name = load_bbmodel(path, xform=xform)
    th, tw = tex.shape[:2]
    R = _rotmat(pitch, 0) @ _rotmat(yaw, 1)
    ld = np.array(light, float)
    ld /= np.linalg.norm(ld)

    # 视空间
    if focus is not None:
        center = np.asarray(focus[0], float)
        view = [((R @ (vs - center).T).T, uvs, R @ n) for vs, uvs, n in tris]
        scale = (size - 60) / float(focus[1])
        off = np.array([size, size], float) / 2
    else:
        allv = np.array([v for tri in tris for v in tri[0]])
        center = (allv.min(0) + allv.max(0)) / 2
        view = [((R @ (vs - center).T).T, uvs, R @ n) for vs, uvs, n in tris]

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
        shade = _mc_shade(n) if shading == "mc" else 0.32 + 0.68 * max(0.0, float(n @ ld))
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
        # 重心权重：w0=λC (AB 边), w1=λA (BC 边), w2=λB (余) → 顶点 0,1,2 取 w1,w2,w0。
        # 曾错位成 (w2,w0,w1)：深度场在三角形内被旋转，倾斜面深度高估 → 薄壁模型
        # 内腔面赢过外墙渲出黑楔（LootCrateVineChest 实证）。
        b0, b1, b2 = w1, w2, w0
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


def render_three_view(path, size=360, bg=(22, 23, 26), shading="lambert"):
    """真实纹理三视图：正面、侧面、前侧 3/4；供模型资产每轮自评。"""
    tiles = []
    name = Path(path).stem
    for label, yaw, pitch in THREE_VIEW_ANGLES:
        rendered, name = render(path, yaw=yaw, pitch=pitch, size=size, bg=bg, shading=shading)
        tiles.append((label, rendered))

    gap = 12
    label_height = 18
    canvas = Image.new(
        "RGB",
        (size * len(tiles) + gap * (len(tiles) + 1), size + label_height + gap * 2),
        (14, 15, 17),
    )
    from PIL import ImageDraw

    draw = ImageDraw.Draw(canvas)
    x = gap
    for label, rendered in tiles:
        draw.text((x + 4, 5), label, fill=(220, 220, 212))
        canvas.paste(rendered, (x, gap + label_height))
        x += size + gap
    return canvas, name


def render_mode_summary(three_view: bool, yaw: float, pitch: float) -> str:
    """返回与实际渲染分支一致的验收角度摘要。"""
    if not three_view:
        return f"yaw={yaw} pitch={pitch}"
    angles = "; ".join(
        f"{label} yaw={view_yaw} pitch={view_pitch}"
        for label, view_yaw, view_pitch in THREE_VIEW_ANGLES
    )
    return f"three-view [{angles}]"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file", nargs="?")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--yaw", type=float, default=-35.0)
    ap.add_argument("--pitch", type=float, default=22.0)
    ap.add_argument("--size", type=int, default=600)
    ap.add_argument("--three-view", action="store_true")
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
    if args.three_view:
        im, name = render_three_view(fp, size=args.size)
        out = OUTDIR / f"render_{Path(fp).stem}_three_view.png"
    else:
        im, name = render(fp, args.yaw, args.pitch, size=args.size)
        out = OUTDIR / f"render_{Path(fp).stem}.png"
    im.save(out)
    print(f"→ {out.relative_to(REPO)}  ({name}, {render_mode_summary(args.three_view, args.yaw, args.pitch)})")


if __name__ == "__main__":
    main()
