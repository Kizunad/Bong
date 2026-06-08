#!/usr/bin/env python3
"""凡物棺材（延寿棺 / mundane_coffin）Blockbench .bbmodel 生成器。

plan-coffin-v1 的延寿棺当前用双 vanilla CHEST 占位（worldview §十四 灵龛挂机设施）。
本脚本生成专用模型源文件，对齐仓库管线：

    local_models/MundaneCoffin.bbmodel   ← 本脚本产物（Blockbench 源）
    → Blockbench 导出 → assets/bong/geo/mundane_coffin.geo.json (geometry.bong.mundane_coffin)
    → 贴图 assets/bong/textures/entity/mundane_coffin_intact.png

视觉语言沿用物资棺三档（CoffinCommon.geo.json）：头大尾小 + 拱形阶梯棺盖。
差异：延寿棺玩法是「玩家躺进去」，故棺身做成中空（四壁+底板，开顶），
棺盖为独立 lid 骨骼、铰链 pivot 在左上长棱，可做开盖动画（进棺时掀盖）。

尺寸（用户指定，单位 MC 格，1 格 = 16px）：长 1.8 / 宽 0.9 / 高 0.9。

用法:
    python3 scripts/models/gen_mundane_coffin.py            # 写 bbmodel + 预览
    python3 scripts/models/gen_mundane_coffin.py --preview-only
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
BBMODEL_OUT = REPO / "local_models" / "MundaneCoffin.bbmodel"
PREVIEW_OUT = REPO / "scripts" / "models" / "mundane_coffin_preview.png"

PX = 16.0  # px per MC block

# ── 尺寸参数（MC 格）──────────────────────────────────────────────
LEN_BLK = 1.8   # 长（Z）
WID_BLK = 0.9   # 宽（X）
HGT_BLK = 0.9   # 高（Y，含拱顶）

L = LEN_BLK * PX   # 28.8
W = WID_BLK * PX   # 14.4
H = HGT_BLK * PX   # 14.4
HALF_L = L / 2.0   # 14.4
HALF_W = W / 2.0   # 7.2

RES = 64  # 贴图分辨率（对齐 coffin_common 凡物档）

# ── 结构参数（中式寿材）─────────────────────────────────────────
# 形制：①垫两道枕木 ②厚重棺身、头大尾小 ③头端碑形大板（大头）
#        ④高耸圆弧顶盖（元宝形，偏头端隆起，占近半高）。
# 全部锚在 ±HALF_W=±7.2 / 0..H envelope 内，总尺寸恰好 0.9×0.9×1.8 格。
ZHENMU_H = 2.0          # 枕木高（棺身离地）
FLOOR_TOP = 4.0         # 棺底板顶（坐在枕木上）
WALL_T = 1.8            # 棺帮厚（厚重）

HEAD_Z = -HALF_L        # 大头端 Z（-14.4）
FOOT_Z = HALF_L         # 小头端 Z（+14.4）
BOARD_T = 2.0           # 头/脚挡板厚
TUB_Z0 = HEAD_Z + BOARD_T   # 棺帮起（-12.4）
TUB_Z1 = FOOT_Z - BOARD_T   # 棺帮止（+12.4）

WALL_TOP_HEAD = 7.5     # 棺帮头段顶（高）
WALL_TOP_FOOT = 6.5     # 棺帮脚段顶（矮）
SEG_Z = 2.0             # 头段/脚段分界（>0 偏脚，头段更长 = 头大）
HALF_W_FOOT = 6.0       # 脚段棺帮外半宽（尾小，头段满 7.2）

HEAD_BOARD_TOP = 11.0   # 碑形大板主体高
HEAD_CAP1_TOP = 12.4    # 碑形第一级
HEAD_CAP2_TOP = 13.2    # 碑形顶（圆头）
FOOT_BOARD_TOP = 6.5    # 脚挡板高（尾小，矮）
FOOT_HALF_W = 5.0       # 脚挡板半宽

# 顶盖：6 级圆顶（X 半宽随高递减 = 横剖圆弧）。下部数层近满长
# = 一条圆木整盖；上部收向头端 = 元宝头隆起。(half_w, y0, y1, z_foot)
LID_DOME = [
    (7.2,  7.0,  8.8,  12.2),
    (6.3,  8.8, 10.2,  11.6),
    (5.1, 10.2, 11.5,  10.0),
    (3.9, 11.5, 12.6,   7.0),
    (2.6, 12.6, 13.6,   3.0),
    (1.3, 13.6, H,     -3.5),
]


# ── 几何：(bone, name, from[xyz], to[xyz]) ────────────────────────
def build_cubes():
    cubes = []

    # ── base bone —— 两道枕木 ──────────────────────────────────
    cubes.append(("base", "zhenmu_head",
                  [-HALF_W, 0.0, -11.0], [HALF_W, ZHENMU_H, -8.4]))
    cubes.append(("base", "zhenmu_foot",
                  [-HALF_W + 1.6, 0.0, 7.6], [HALF_W - 1.6, ZHENMU_H, 10.2]))

    # ── body bone —— 厚重棺身（头大尾小，开顶中空，玩家躺其内）──
    # 棺底板
    cubes.append(("body", "floor",
                  [-HALF_W, ZHENMU_H, TUB_Z0], [HALF_W, FLOOR_TOP, TUB_Z1]))
    # 棺帮头段（满宽、较高）
    cubes.append(("body", "wall_head_L",
                  [-HALF_W, FLOOR_TOP, TUB_Z0], [-HALF_W + WALL_T, WALL_TOP_HEAD, SEG_Z]))
    cubes.append(("body", "wall_head_R",
                  [HALF_W - WALL_T, FLOOR_TOP, TUB_Z0], [HALF_W, WALL_TOP_HEAD, SEG_Z]))
    # 棺帮脚段（内收一级、较矮 = 尾小）
    cubes.append(("body", "wall_foot_L",
                  [-HALF_W_FOOT, FLOOR_TOP, SEG_Z], [-HALF_W_FOOT + WALL_T, WALL_TOP_FOOT, TUB_Z1]))
    cubes.append(("body", "wall_foot_R",
                  [HALF_W_FOOT - WALL_T, FLOOR_TOP, SEG_Z], [HALF_W_FOOT, WALL_TOP_FOOT, TUB_Z1]))
    # 头端碑形大板（大头）—— 主体 + 两级圆头
    cubes.append(("body", "head_board",
                  [-HALF_W, ZHENMU_H, HEAD_Z], [HALF_W, HEAD_BOARD_TOP, TUB_Z0]))
    cubes.append(("body", "head_cap1",
                  [-HALF_W + 1.6, HEAD_BOARD_TOP, HEAD_Z + 0.4], [HALF_W - 1.6, HEAD_CAP1_TOP, TUB_Z0]))
    cubes.append(("body", "head_cap2",
                  [-3.4, HEAD_CAP1_TOP, HEAD_Z + 0.6], [3.4, HEAD_CAP2_TOP, TUB_Z0 - 0.2]))
    # 脚端挡板（尾小，窄+矮）
    cubes.append(("body", "foot_board",
                  [-FOOT_HALF_W, ZHENMU_H, TUB_Z1], [FOOT_HALF_W, FOOT_BOARD_TOP, FOOT_Z]))

    # ── lid bone —— 高耸元宝圆顶（6 级，偏头隆起向脚跌落，可掀盖）──
    for i, (hw, y0, y1, zf) in enumerate(LID_DOME):
        cubes.append(("lid", f"dome{i}",
                      [-hw, y0, TUB_Z0], [hw, y1, zf]))

    return cubes


BONE_ORDER = ["base", "body", "lid"]
BONE_PIVOTS = {
    "base": [0.0, 0.0, 0.0],
    "body": [0.0, ZHENMU_H, 0.0],
    # 铰链：左上长棱，绕 Z 轴掀向 +X 侧（侧开盖，掀盖入棺）
    "lid": [-HALF_W + 0.3, WALL_TOP_HEAD, 0.0],
}
BONE_COLORS = {  # 预览用
    "base": (120, 92, 58),
    "body": (150, 116, 74),
    "lid": (176, 138, 92),
}


# ── 木纹贴图（64×64，竖向板纹 + 节子 + 暗箍带）────────────────────
def make_wood_texture(res=RES, seed=7):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    # 竖向板纹：每 ~9px 一块板，板缝压暗
    plank_w = 9
    plank_id = (x // plank_w).astype(float)
    seam = ((x % plank_w) < 1) | ((x % plank_w) > plank_w - 1.2)
    # 沿 y 的长纹理
    grain = 0.5 + 0.5 * np.sin(y * 0.55 + plank_id * 1.7)
    grain += 0.25 * np.sin(y * 0.17 + plank_id * 3.1)
    grain = np.clip(grain, 0, 1)
    # 每板基色微差
    plank_tone = (rng.random(int(res / plank_w) + 2) - 0.5) * 24
    tone = plank_tone[plank_id.astype(int)]

    base = np.array([120, 70, 50], float)  # 寿材暗红栗（旧漆木）
    col = base[None, None, :] + (grain[..., None] - 0.5) * 34 + tone[..., None]
    col[..., 0] += grain * 12  # 漆面偏红
    col[seam] *= 0.58  # 板缝压暗
    # 节子（旧木，少量）
    for _ in range(4):
        cx, cy = rng.integers(4, res - 4, 2)
        r = rng.integers(2, 4)
        d = np.hypot(x - cx, y - cy)
        m = d < r
        col[m] *= 0.5
        col[(d >= r) & (d < r + 1)] *= 0.74
    col = np.clip(col, 14, 210).astype(np.uint8)
    img = np.dstack([col, np.full((res, res), 255, np.uint8)])
    return Image.fromarray(img, "RGBA")


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


# ── UV 货架打包（每面按像素尺寸放置，溢出则回绕到木纹区，不拉伸）──
class ShelfPacker:
    def __init__(self, res=RES):
        self.res = res
        self.x = self.y = self.rowh = 0.0

    def place(self, w, h):
        w = min(w, self.res)
        h = min(h, self.res)
        if self.x + w > self.res:
            self.x = 0.0
            self.y += self.rowh
            self.rowh = 0.0
        if self.y + h > self.res:
            self.y = 0.0  # 回绕复用木纹（uniform 木纹无碍）
        ox, oy = self.x, self.y
        self.x += w
        self.rowh = max(self.rowh, h)
        return ox, oy


def cube_faces_uv(frm, to, packer):
    dx = to[0] - frm[0]
    dy = to[1] - frm[1]
    dz = to[2] - frm[2]
    # 各面像素尺寸 (u,v)
    dims = {
        "north": (dx, dy), "south": (dx, dy),
        "east": (dz, dy), "west": (dz, dy),
        "up": (dx, dz), "down": (dx, dz),
    }
    faces = {}
    for name, (w, h) in dims.items():
        ox, oy = packer.place(w, h)
        faces[name] = {"uv": [round(ox, 2), round(oy, 2),
                              round(ox + w, 2), round(oy + h, 2)],
                       "texture": 0}
    return faces


# ── 组装 .bbmodel ────────────────────────────────────────────────
def build_bbmodel():
    cubes = build_cubes()
    packer = ShelfPacker(RES)
    elements = []
    bone_children = {b: [] for b in BONE_ORDER}

    for bone, name, frm, to in cubes:
        euid = str(uuid.uuid4())
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

    tex = make_wood_texture()
    model = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "mundane_coffin",
        "model_identifier": "geometry.bong.mundane_coffin",
        "visible_box": [3, 2, 0.5],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "",
            "name": "mundane_coffin.png",
            "folder": "entity",
            "namespace": "bong",
            "id": "0",
            "width": RES,
            "height": RES,
            "uv_width": RES,
            "uv_height": RES,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": str(uuid.uuid4()),
            "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


# ── 正交 + 等距预览（按骨骼上色，便于核验比例/头大尾小/拱盖）──────
def render_preview(cubes, tex, out=PREVIEW_OUT):
    scale = 7
    pad = 16
    gap = 24

    def lit(color, k):
        return tuple(int(np.clip(c * k, 0, 255)) for c in color)

    def ortho(ax_u, ax_v, flip_u, flip_v, title):
        us, vs = [], []
        for _, _, frm, to in cubes:
            for cu in (frm[ax_u], to[ax_u]):
                us.append(cu)
            for cv in (frm[ax_v], to[ax_v]):
                vs.append(cv)
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (30, 30, 34, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(220, 220, 220))

        def to_px(u, v):
            pu = (u - umin) * scale
            pv = (v - vmin) * scale
            if flip_u:
                pu = (umax - umin) * scale - pu
            if not flip_v:  # 屏幕 y 向下：默认把模型 +Y 翻到上方
                pv = (vmax - vmin) * scale - pv
            return pad + pu, pad + 14 + pv

        order = sorted(cubes, key=lambda c: c[2][3 - ax_u - ax_v])
        for bone, _, frm, to in order:
            x0, y0 = to_px(frm[ax_u], frm[ax_v])
            x1, y1 = to_px(to[ax_u], to[ax_v])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,),
                        outline=(25, 20, 16, 255))
        return im

    # 三视图
    front = ortho(0, 1, False, True, "FRONT  (X-Y)  head@back")
    side = ortho(2, 1, False, True, "SIDE   (Z-Y)  head<-  ->foot")
    top = ortho(0, 2, False, True, "TOP    (X-Z)  head/foot taper")

    # 等距视图
    def iso():
        ca, sa = math.cos(math.radians(30)), math.sin(math.radians(30))
        pts = []

        def proj(x, y, z):
            px = (x - z) * ca
            py = (x + z) * sa - y
            return px, py
        for _, _, frm, to in cubes:
            for X in (frm[0], to[0]):
                for Y in (frm[1], to[1]):
                    for Z in (frm[2], to[2]):
                        pts.append(proj(X, Y, Z))
        umin = min(p[0] for p in pts)
        umax = max(p[0] for p in pts)
        vmin = min(p[1] for p in pts)
        vmax = max(p[1] for p in pts)
        wpx = int((umax - umin) * scale) + pad * 2
        hpx = int((vmax - vmin) * scale) + pad * 2 + 14
        im = Image.new("RGBA", (wpx, hpx), (30, 30, 34, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), "ISO", fill=(220, 220, 220))

        def to_px(p):
            return pad + (p[0] - umin) * scale, pad + 14 + (p[1] - vmin) * scale

        order = sorted(cubes, key=lambda c: (c[2][0] + c[2][2] + c[2][1]))
        for bone, _, frm, to in order:
            x0, y0, z0 = frm
            x1, y1, z1 = to
            col = BONE_COLORS[bone]
            faces = [
                ([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.15),  # top
                ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.8),   # front (+z)
                ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.62),  # right (+x)
            ]
            for verts, k in faces:
                poly = [to_px(proj(*v)) for v in verts]
                d.polygon(poly, fill=lit(col, k) + (255,), outline=(25, 20, 16, 255))
        return im

    iso_im = iso()

    # 拼接：上排 front/side/top，下排 iso + 贴图放大
    tiles_top = [front, side, top]
    tw = sum(t.width for t in tiles_top) + gap * (len(tiles_top) + 1)
    th = max(t.height for t in tiles_top)
    tex_big = tex.resize((RES * 2, RES * 2), Image.NEAREST)
    bot_h = max(iso_im.height, tex_big.height)
    W_ = max(tw, iso_im.width + tex_big.width + gap * 3)
    H_ = th + bot_h + gap * 3
    canvas = Image.new("RGBA", (W_, H_), (18, 18, 20, 255))
    x = gap
    for t in tiles_top:
        canvas.paste(t, (x, gap), t)
        x += t.width + gap
    canvas.paste(iso_im, (gap, th + gap * 2), iso_im)
    canvas.paste(tex_big, (gap * 2 + iso_im.width, th + gap * 2), tex_big)
    d = ImageDraw.Draw(canvas)
    d.text((gap * 2 + iso_im.width, th + gap * 2 - 12), "TEXTURE 64x64 (x2)",
           fill=(200, 200, 200))
    canvas.save(out)
    return out


def summarize(cubes):
    xs = [v for _, _, f, t in cubes for v in (f[0], t[0])]
    ys = [v for _, _, f, t in cubes for v in (f[1], t[1])]
    zs = [v for _, _, f, t in cubes for v in (f[2], t[2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bounding box  : {bb[0]:.1f} × {bb[1]:.1f} × {bb[2]:.1f} px"
          f"  = {bb[0]/PX:.3f} W × {bb[1]/PX:.3f} H × {bb[2]/PX:.3f} L 格")
    print(f"  target        : {W:.1f} × {H:.1f} × {L:.1f} px"
          f"  = {WID_BLK} W × {HGT_BLK} H × {LEN_BLK} L 格")
    print(f"  cube count    : {len(cubes)}  ("
          + ", ".join(f"{b}:{sum(1 for c in cubes if c[0]==b)}" for b in BONE_ORDER) + ")")
    print(f"  lid hinge pivot: {BONE_PIVOTS['lid']}  (绕 Z 侧开盖)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    model, cubes, tex = build_bbmodel()
    print("凡物棺材 / mundane_coffin:")
    summarize(cubes)

    if not args.preview_only:
        BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel     : {BBMODEL_OUT.relative_to(REPO)}  ({BBMODEL_OUT.stat().st_size} B)")
    p = render_preview(cubes, tex)
    print(f"  → preview     : {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
