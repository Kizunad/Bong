#!/usr/bin/env python3
"""真实渲染 .bbmodel —— 读几何 + UV + 内嵌贴图，z-buffer 遮挡 + 纹理采样 + 方向光。

不同于各 gen_*.py 里的示意预览（平涂盒子、画家排序），本脚本忠实还原模型实际长相：
逐面光栅化、深度缓冲正确遮挡、按 UV 采样内嵌贴图（最近邻，像素风）、按法线方向光着色。
支持 element 的 origin+rotation（读得了 Blockbench 存盘的 fmt 5.0 文件）。

用法:
  python3 modelScript/core/render_bbmodel.py modelScript/models/BronzeCoffin.bbmodel
  python3 modelScript/core/render_bbmodel.py --all              # 四档拼一张
  python3 modelScript/core/render_bbmodel.py <file> --yaw -35 --pitch 22
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import framing  # noqa: E402
import workspace  # noqa: E402

# 项目根和产出目录由 workspace 解析，不再靠 `parents[2]` 猜「本文件住在
# <仓库根>/modelScript/core/ 下」——那个假设在库被搬进独立 repo 后必然崩。
_WS = workspace.Workspace.discover(start=Path(__file__))

REPO = _WS.root
MODELS = _WS.models
OUTDIR = _WS.out

# 三视图的角度不再硬编，改由 framing 按**声明的朝向**派生 —— 名字必须和实际照到的
# 轴面对得上。默认 facing 沿用历史假定 `-z`（yaw=180 叫 FRONT），于是角度一个没变：
# FRONT 180 / SIDE_R 90 / 3/4 145。唯一改名的是 SIDE → SIDE_R：yaw=90 照的是 −x 面，
# 那是 FRONT 视里观者右手边的一侧，叫「SIDE」等于把左右两侧混成一个名字。
THREE_VIEW_NAMES = ("FRONT", "SIDE_R", "3/4")
THREE_VIEWS = framing.views_for(framing.LEGACY_FACING, THREE_VIEW_NAMES)
THREE_VIEW_ANGLES = tuple((v.name, v.yaw, v.pitch) for v in THREE_VIEWS)

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


def _load_texture(entry, model_path):
    """贴图源有两种：Blockbench 内嵌的 data URI，和链接到磁盘的相对路径。

    历史上只认 data URI，路径引用会被当成 base64 直接解 —— 报的是
    `binascii.Error: Incorrect padding` 这种和"贴图找不到"毫不相干的错，
    仓库 55 个 bbmodel 里有 11 个（全是多状态实体）因此渲不出来。
    """
    src = entry.get("source", "")
    if src.startswith("data:"):
        return np.asarray(
            Image.open(io.BytesIO(base64.b64decode(src.split(",", 1)[1]))).convert("RGBA"),
            float,
        )
    if not src:
        raise ValueError(f"贴图 {entry.get('name')!r} 既无 source 也无内嵌数据")
    # 路径按项目根解析（bbmodel 里存的是 client/src/main/... 这种仓库相对路径），
    # 找不到再退回按 bbmodel 自身所在目录解析。
    try:
        path = _WS.resolve_texture(src, near=model_path)
    except FileNotFoundError as exc:
        raise FileNotFoundError(f"贴图 {entry.get('name')!r} 的{exc}") from None
    return np.asarray(Image.open(path).convert("RGBA"), float)


def _texture_index(texes, texture, model_path):
    """texture 可以是 None（取第 0 张）、int 索引，或贴图名（可省 .png 后缀）。"""
    if texture is None:
        return 0
    if isinstance(texture, int):
        if not 0 <= texture < len(texes):
            raise IndexError(
                f"{Path(model_path).name} 只有 {len(texes)} 张贴图，索引 {texture} 越界"
            )
        return texture
    names = [t.get("name", "") for t in texes]
    for i, n in enumerate(names):
        if n == texture or Path(n).stem == texture:
            return i
    raise KeyError(f"{Path(model_path).name} 没有名为 {texture!r} 的贴图；有 {names}")


def texture_names(path):
    """列出某个 bbmodel 的贴图名，供预览工具枚举状态变体。"""
    return [t.get("name", "") for t in json.loads(Path(path).read_text()).get("textures", [])]


def load_bbmodel(path, xform=None, texture=None):
    """xform: {element uuid: 4x4 刚体矩阵}，在元素自身 origin+rotation 之后再叠。

    骨骼动画预览用——本模块只认元素级旋转，骨树是无视的；摆姿时由调用方（rig.py）
    算好每个元素的世界矩阵传进来，省得把姿态烘回 from/to（轴对齐盒装不下任意旋转）。
    """
    d = json.loads(Path(path).read_text())
    res = d.get("resolution", {"width": 64, "height": 64})
    rw, rh = res["width"], res["height"]
    texes = d.get("textures", [])
    if not texes:
        raise ValueError(f"{Path(path).name} 没有贴图，无法渲染")
    ti = _texture_index(texes, texture, path)
    tex = _load_texture(texes[ti], path)

    tris = []  # (verts[3x3], uvs[3x2], normal[3])
    for e in d["elements"]:
        if e.get("type", "cube") != "cube":
            continue
        # 注意 face["texture"] 这个索引**不能**拿来筛元素。多贴图模型（idle/working、
        # intact/searching/looted…）的每张贴图都是覆盖同一套 UV 的整体状态皮肤，而元素上
        # 的索引是生成时按序号轮着写的垃圾值——ForgeStation 的底座/砧/锤各被切成 0,1,0,1，
        # 按它过滤会把一个铁匠台拆成互不相干的碎块。贴图是整体选，几何永远全画。
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
           xform=None, focus=None, shading="lambert", texture=None):
    """focus: (center3, span) 固定取景——逐帧自动取景会让动画整体抖动（每帧包围盒不同）。

    shading: "lambert"（默认，保持既有行为）或 "mc"（按 MC 原版面亮度表，
    预览更贴近进游戏后的实际观感）。
    """
    tris, tex, (rw, rh), name = load_bbmodel(path, xform=xform, texture=texture)
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


def render_three_view(path, size=360, bg=(22, 23, 26), shading="lambert", texture=None,
                     facing=framing.LEGACY_FACING):
    """真实纹理三视图：正面、右侧、前侧 3/4；供模型资产每轮自评。

    facing 声明资产的正面朝哪个轴（`+z` / `-z` / `+x` / `-x`），视角名由它派生。缺省沿
    用历史假定 `-z`，既有调用点行为不变。

    **三张共用一个取景**：render() 自动取景是每张图各算各的包围盒中心与缩放，于是
    「侧视比正视矮一截」这种观察全是取景噪声，跨轮叠图更是完全对不上。这里一次算出
    覆盖三个角度的公共 focus 传下去，三张图的屏幕坐标从此可比。
    """
    views = framing.views_for(facing, THREE_VIEW_NAMES)
    focus = framing.focus_for(path, views)
    tiles = []
    name = Path(path).stem
    for view in views:
        rendered, name = render(path, yaw=view.yaw, pitch=view.pitch, size=size, bg=bg,
                                focus=focus, shading=shading, texture=texture)
        tiles.append((view.label, rendered))

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
        out.parent.mkdir(parents=True, exist_ok=True)
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
    out.parent.mkdir(parents=True, exist_ok=True)
    im.save(out)
    print(f"→ {out.relative_to(REPO)}  ({name}, {render_mode_summary(args.three_view, args.yaw, args.pitch)})")


if __name__ == "__main__":
    main()
