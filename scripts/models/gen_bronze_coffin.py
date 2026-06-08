#!/usr/bin/env python3
"""青铜饕餮棺（进阶延寿棺 灵材三档·压轴 / bronze_coffin）Blockbench .bbmodel 生成器。

plan-coffin-v1 §遗留：灵材棺材（×0.7/×0.5/×0.3）+ 符文棺材。青铜饕餮棺 = 灵材
三档（×0.3，最强）+ 符文方向：上古灵金铸棺，饕餮镇魂、符文封寿。

四档延寿棺压轴：
  ×0.9 凡物棺  暖棕漆木圆盖 + 枕木
  ×0.7 寒玉棺  青碧玉须弥座 + 灵脉
  ×0.5 玄石阵棺 深板岩石椁 + 阵眼金阵纹
  ×0.3 青铜饕餮棺 鎏金青铜 + 兽蹄足 + 扉棱 + 饕餮面 + 夔龙宝钮 + 云雷纹/符文

** 分件构建 **（用户指定流程：分件做 → 逐件预览 → 拼接）：
  base   —— 鼎足基座（四兽蹄足 + 云雷纹基带）
  body   —— 棺身（厚壁中空）+ 四角扉棱 + 侧铺首
  taotie —— 饕餮面（头端浮雕：鼻扉棱/臣字双目/卷角/獠牙巨口，左右对称）
  lid    —— 青铜盖（覆斗 + 夔龙脊 + 宝钮，可掀盖）

  python3 gen_bronze_coffin.py --part taotie   # 单件预览
  python3 gen_bronze_coffin.py                 # 全件预览 + 拼接写 bbmodel

管线同前；尺寸 1.8×0.9×0.9 格，独立可掀盖 lid。bones：base / body(含扉棱铺首饕餮) / lid。
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
BBMODEL_OUT = REPO / "local_models" / "BronzeCoffin.bbmodel"
PREVIEW_DIR = REPO / "scripts" / "models"

PX = 16.0
LEN_BLK, WID_BLK, HGT_BLK = 1.8, 0.9, 0.9
L, W, H = LEN_BLK * PX, WID_BLK * PX, HGT_BLK * PX
HALF_L, HALF_W = L / 2.0, W / 2.0
RES = 64

HEAD_Z, FOOT_Z = -HALF_L, HALF_L      # 大头（饕餮）-Z / 小头 +Z
BODY_Y0 = 3.4                          # 棺身坐落（兽蹄足之上）
WALL_TOP = 9.5
WALL_IN, WALL_OUT = 5.0, 6.6          # 棺壁内/外沿
TUB_Z0, TUB_Z1 = HEAD_Z + 1.6, FOOT_Z - 1.6   # 端壁内缘 ±12.8
HEAD_PLATE_Z = -13.4                   # 饕餮背板前缘（浮雕在其前 1.0px）

# 预览配色（青铜 + 饕餮分件辨识）
COL = {
    "base": (150, 118, 70), "body": (170, 136, 84), "lid": (186, 152, 98),
    "flange": (196, 162, 104), "puishou": (176, 142, 90),
    "tt_bg": (120, 96, 58), "tt_nose": (198, 164, 106), "tt_eye": (222, 196, 138),
    "tt_pupil": (58, 46, 30), "tt_horn": (190, 156, 100), "tt_mouth": (96, 74, 46),
    "tt_fang": (224, 206, 162), "dragon": (200, 168, 110), "knob": (214, 184, 120),
}
GLOW = (150, 232, 202)  # 符文青光


# ── 分件 ──────────────────────────────────────────────────────────
def part_base():
    """鼎足基座：四兽蹄足 + 云雷纹基带。"""
    c = []
    feet = [(-6.6, -4.6, -13.4, -11.4), (4.6, 6.6, -13.4, -11.4),
            (-6.6, -4.6, 11.4, 13.4), (4.6, 6.6, 11.4, 13.4)]
    for i, (x0, x1, z0, z1) in enumerate(feet):
        c.append(("base", f"hoof{i}", [x0 - 0.3, 0.0, z0 - 0.3], [x1 + 0.3, 0.9, z1 + 0.3], COL["base"]))   # 蹄
        c.append(("base", f"foot{i}", [x0, 0.9, z0], [x1, BODY_Y0, z1], COL["base"]))                       # 足柱
    c.append(("base", "base_band", [-7.0, BODY_Y0 - 0.6, -13.8], [7.0, BODY_Y0 + 0.4, 13.8], COL["base"]))  # 云雷纹基带
    return c


def part_body():
    """棺身：厚壁中空 + 四角扉棱 + 两侧铺首。"""
    c = []
    c.append(("body", "floor", [-WALL_OUT, BODY_Y0, TUB_Z0], [WALL_OUT, BODY_Y0 + 1.4, TUB_Z1], COL["body"]))
    c.append(("body", "wall_L", [-WALL_OUT, BODY_Y0 + 1.4, TUB_Z0], [-WALL_IN, WALL_TOP, TUB_Z1], COL["body"]))
    c.append(("body", "wall_R", [WALL_IN, BODY_Y0 + 1.4, TUB_Z0], [WALL_OUT, WALL_TOP, TUB_Z1], COL["body"]))
    c.append(("body", "wall_foot", [-6.0, BODY_Y0 + 1.4, TUB_Z1], [6.0, WALL_TOP - 1.2, FOOT_Z], COL["body"]))
    # 四角扉棱（竖向锯齿鳍：主鳍 ±6.8 + 外挑齿 ±7.2，封顶 0.9）
    for tag, sx, z in (("HL", -1, TUB_Z0), ("HR", 1, TUB_Z0),
                       ("FL", -1, TUB_Z1 - 1.0), ("FR", 1, TUB_Z1 - 1.0)):
        fo, fi = sorted((sx * 6.8, sx * 6.2))
        c.append(("body", f"flange_{tag}", [fo, BODY_Y0 + 1.4, z], [fi, WALL_TOP + 0.4, z + 1.0], COL["flange"]))
        to, ti = sorted((sx * 7.2, sx * 6.2))
        for k, yy in enumerate((BODY_Y0 + 2.0, BODY_Y0 + 4.2, BODY_Y0 + 6.4)):   # 锯齿
            c.append(("body", f"flange_{tag}_t{k}", [to, yy, z + 0.2], [ti, yy + 0.9, z + 0.8], COL["flange"]))
    # 两侧铺首（兽面凸 + 衔环，外挑至多 ±7.2）
    for side, sx in (("L", -1), ("R", 1)):
        bo, bi = sorted((sx * 7.0, sx * 6.4))
        c.append(("body", f"puishou_{side}", [bo, 5.4, -1.9], [bi, 8.2, 1.9], COL["puishou"]))
        ro, ri = sorted((sx * 7.2, sx * 6.4))
        c.append(("body", f"ring_{side}", [ro, 3.9, -1.5], [ri, 5.0, 1.5], COL["puishou"]))
    return c


def part_taotie():
    """饕餮面（头端浮雕，左右对称）：背板 + 鼻扉棱 + 臣字双目(带瞳) + 卷角 + 獠牙巨口。"""
    c = []
    PF = -14.4   # 浮雕最前（envelope 前缘）
    c.append(("body", "tt_bg", [-6.4, BODY_Y0, HEAD_PLATE_Z], [6.4, 10.4, TUB_Z0], COL["tt_bg"]))   # 背板（recessed，五官在其前浮雕）
    # 鼻扉棱（中轴，最凸）
    c.append(("body", "tt_nose", [-0.9, 4.6, PF], [0.9, 9.8, HEAD_PLATE_Z], COL["tt_nose"]))
    c.append(("body", "tt_nose_br", [-1.6, 9.0, PF + 0.2], [1.6, 9.8, HEAD_PLATE_Z], COL["tt_nose"]))  # 鼻梁眉脊
    for s in (-1, 1):   # 左右对称特征
        # 臣字目（外大内小 + 黑瞳）
        ex0, ex1 = sorted((s * 1.7, s * 4.1))
        c.append(("body", f"tt_eye_{s}", [ex0, 6.4, PF + 0.2], [ex1, 8.7, HEAD_PLATE_Z], COL["tt_eye"]))
        px0, px1 = sorted((s * 2.3, s * 3.5))
        c.append(("body", f"tt_pupil_{s}", [px0, 6.9, PF], [px1, 8.2, HEAD_PLATE_Z], COL["tt_pupil"]))
        # 卷角：眉 → 上卷 → 外展尖（三级，扫向头顶外角）
        b0, b1 = sorted((s * 1.6, s * 4.2))
        c.append(("body", f"tt_brow_{s}", [b0, 8.7, PF + 0.3], [b1, 9.4, HEAD_PLATE_Z], COL["tt_horn"]))
        m0, m1 = sorted((s * 3.4, s * 5.6))
        c.append(("body", f"tt_horn_{s}", [m0, 9.2, PF + 0.4], [m1, 10.0, HEAD_PLATE_Z], COL["tt_horn"]))
        t0, t1 = sorted((s * 5.0, s * 6.4))
        c.append(("body", f"tt_horntip_{s}", [t0, 9.6, PF + 0.4], [t1, 10.4, HEAD_PLATE_Z], COL["tt_horn"]))
        # 云纹颊（眼下小卷）
        k0, k1 = sorted((s * 3.2, s * 5.2))
        c.append(("body", f"tt_cheek_{s}", [k0, 5.6, PF + 0.3], [k1, 6.4, HEAD_PLATE_Z], COL["tt_nose"]))
        # 獠牙（口下，朝下尖）
        fx0, fx1 = sorted((s * 1.4, s * 2.6))
        c.append(("body", f"tt_fang_{s}", [fx0, 3.6, PF + 0.1], [fx1, 4.8, HEAD_PLATE_Z], COL["tt_fang"]))
    # 獠牙巨口（横压口，居中）
    c.append(("body", "tt_mouth", [-3.9, 4.8, PF + 0.1], [3.9, 6.0, HEAD_PLATE_Z], COL["tt_mouth"]))
    return c


def part_lid():
    """青铜盖：覆斗 + 夔龙脊 + 宝钮（可掀盖）。"""
    c = []
    LB = WALL_TOP   # 9.5；盖体压缩进 9.5..14.4（4.9px）
    c.append(("lid", "lid_base", [-6.8, LB, TUB_Z0], [6.8, LB + 1.2, TUB_Z1], COL["lid"]))     # 盖板（出檐）
    c.append(("lid", "lid_dou", [-5.0, LB + 1.2, -10.5], [5.0, LB + 2.3, 10.5], COL["lid"]))   # 覆斗
    # 四角云纹角饰
    for sx in (-1, 1):
        for sz in (-1, 1):
            x0, x1 = sorted((sx * 6.6, sx * 4.8))
            z0, z1 = sorted((sz * 10.2, sz * 8.4))
            c.append(("lid", f"corner_{sx}_{sz}", [x0, LB + 1.2, z0], [x1, LB + 2.1, z1], COL["lid"]))
    # 夔龙脊（中脊 + 两端龙首）
    c.append(("lid", "ridge", [-1.7, LB + 2.3, -10.0], [1.7, LB + 3.2, 10.0], COL["dragon"]))
    for tag, z0, z1 in (("head", -11.0, -9.0), ("foot", 9.0, 11.0)):
        c.append(("lid", f"kui_{tag}", [-2.3, LB + 2.0, z0], [2.3, LB + 3.4, z1], COL["dragon"]))  # 龙首
    # 宝钮（中心，逐级收顶，符文光核）
    c.append(("lid", "knob0", [-2.2, LB + 3.2, -2.4], [2.2, LB + 3.9, 2.4], COL["knob"]))
    c.append(("lid", "knob1", [-1.4, LB + 3.9, -1.6], [1.4, LB + 4.6, 1.6], COL["knob"]))
    c.append(("lid", "knob_core", [-0.8, LB + 4.6, -1.0], [0.8, H, 1.0], GLOW))   # 符文光核（峰=14.4）
    return c


PARTS = {"base": part_base, "body": part_body, "taotie": part_taotie, "lid": part_lid}
BONE_ORDER = ["base", "body", "lid"]
BONE_PIVOTS = {"base": [0.0, 0.0, 0.0], "body": [0.0, BODY_Y0, 0.0], "lid": [-6.8, WALL_TOP, 0.0]}


def all_cubes():
    c = []
    for name in ("base", "body", "taotie", "lid"):
        c += PARTS[name]()
    return c


# ── 青铜 + 云雷纹 + 铜绿 + 符文贴图 ───────────────────────────────
def make_bronze_texture(res=RES, seed=31):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    n = (np.sin(x * 0.3 + y * 0.2) + np.sin(x * 0.13 - y * 0.27 + 0.7)) / 2
    n = (n + 1) / 2
    base = np.array([158, 124, 72], float)    # 鎏金青铜
    dark = np.array([108, 82, 44], float)
    col = dark[None, None, :] * (1 - n[..., None]) + base[None, None, :] * n[..., None]
    col += (rng.random((res, res, 1)) - 0.5) * 10

    # 云雷纹（回纹格栅：每格嵌套方框）
    lw = Image.new("L", (res, res), 0)
    ld = ImageDraw.Draw(lw)
    cell = 11
    for gy in range(1, res, cell):
        for gx in range(1, res, cell):
            ld.rectangle([gx, gy, gx + cell - 3, gy + cell - 3], outline=120)
            ld.rectangle([gx + 2, gy + 2, gx + cell - 5, gy + cell - 5], outline=90)
            ld.line([(gx + cell - 4, gy + 2), (gx + cell - 4, gy + 4)], fill=120)  # 螺口
    lwa = np.asarray(lw, float) / 255.0
    col *= (1 - lwa[..., None] * 0.32)   # 纹路压暗（青铜阴线）

    # 铜绿（verdigris 斑）
    patina = np.array([74, 146, 122], float)
    pm = np.zeros((res, res))
    for _ in range(7):
        cx, cy = rng.integers(0, res, 2)
        r = rng.integers(5, 12)
        pm = np.maximum(pm, np.clip(1 - np.hypot(x - cx, y - cy) / r, 0, 1) ** 1.5)
    pm *= 0.55
    col = col * (1 - pm[..., None]) + patina[None, None, :] * pm[..., None]

    # 符文光点（青光，少量）
    glow = Image.new("L", (res, res), 0)
    gd = ImageDraw.Draw(glow)
    for _ in range(5):
        cx, cy = int(rng.integers(6, res - 6)), int(rng.integers(6, res - 6))
        gd.line([(cx - 2, cy), (cx + 2, cy)], fill=255)
        gd.line([(cx, cy - 2), (cx, cy + 2)], fill=255)
    ga = np.asarray(glow.filter(ImageFilter.GaussianBlur(1.3)), float) / 255.0
    g = np.array(GLOW, float)
    col = col + ga[..., None] * (g[None, None, :] - col) * 0.8

    col = np.clip(col, 22, 248).astype(np.uint8)
    return Image.fromarray(np.dstack([col, np.full((res, res), 255, np.uint8)]), "RGBA")


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class ShelfPacker:
    def __init__(self, res=RES):
        self.res, self.x, self.y, self.rowh = res, 0.0, 0.0, 0.0

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
    dims = {"north": (dx, dy), "south": (dx, dy), "east": (dz, dy), "west": (dz, dy), "up": (dx, dz), "down": (dx, dz)}
    out = {}
    for nm, (w, h) in dims.items():
        ox, oy = packer.place(max(w, 0.1), max(h, 0.1))
        out[nm] = {"uv": [round(ox, 2), round(oy, 2), round(ox + max(w, 0.1), 2), round(oy + max(h, 0.1), 2)], "texture": 0}
    return out


def build_bbmodel(cubes):
    packer = ShelfPacker(RES)
    elements, bone_children = [], {b: [] for b in BONE_ORDER}
    for cube in cubes:
        bone, name, frm, to = cube[0], cube[1], cube[2], cube[3]
        euid = str(uuid.uuid4())
        elements.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False, "render_order": "default",
            "allow_mirror_modeling": True, "type": "cube", "uuid": euid,
            "from": [round(v, 3) for v in frm], "to": [round(v, 3) for v in to],
            "autouv": 0, "color": BONE_ORDER.index(bone), "origin": list(BONE_PIVOTS[bone]),
            "faces": cube_faces_uv(frm, to, packer),
        })
        bone_children[bone].append(euid)
    outliner = [{
        "name": b, "origin": list(BONE_PIVOTS[b]), "color": BONE_ORDER.index(b), "uuid": str(uuid.uuid4()),
        "export": True, "mirror_uv": False, "isOpen": True, "locked": False, "visibility": True,
        "autouv": 0, "children": bone_children[b],
    } for b in BONE_ORDER]
    tex = make_bronze_texture()
    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "bronze_coffin", "model_identifier": "geometry.bong.bronze_coffin",
        "visible_box": [3, 2, 0.5], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "bronze_coffin.png", "folder": "entity", "namespace": "bong", "id": "0",
            "width": RES, "height": RES, "uv_width": RES, "uv_height": RES, "particle": False,
            "render_mode": "default", "visible": True, "mode": "bitmap", "saved": False,
            "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }, tex


# ── 渲染 ──────────────────────────────────────────────────────────
def _lit(c, k):
    return tuple(int(np.clip(v * k, 0, 255)) for v in c)


def _pcol(cube):
    return cube[4] if len(cube) > 4 else COL.get(cube[0], (160, 130, 80))


def _view(cubes, mode, scale=8, pad=14, title=""):
    if mode == "iso":
        ca, sa = math.cos(math.radians(30)), math.sin(math.radians(30))

        def proj(x, y, z):
            return (x - z) * ca, (x + z) * sa - y
        pts = [proj(X, Y, Z) for c in cubes for X in (c[2][0], c[3][0]) for Y in (c[2][1], c[3][1]) for Z in (c[2][2], c[3][2])]
    else:
        # mode → (u轴, v轴, 深度排序reverse)。face = 从头端 -Z 正看饕餮面
        au, av, rev = {"front": (0, 1, False), "side": (2, 1, False),
                       "top": (0, 2, False), "face": (0, 1, True)}[mode]
    if mode != "iso":
        us = [v for c in cubes for v in (c[2][au], c[3][au])]
        vs = [v for c in cubes for v in (c[2][av], c[3][av])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        wpx, hpx = int((umax - umin) * scale) + pad * 2, int((vmax - vmin) * scale) + pad * 2 + 12
        im = Image.new("RGBA", (wpx, hpx), (24, 22, 20, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 2), title, fill=(214, 198, 168))

        def tp(u, v):
            return pad + (u - umin) * scale, pad + 12 + ((vmax - vmin) - (v - vmin)) * scale
        for cube in sorted(cubes, key=lambda c: c[2][3 - au - av], reverse=rev):
            f, t = cube[2], cube[3]
            x0, y0 = tp(f[au], f[av])
            x1, y1 = tp(t[au], t[av])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)], fill=_pcol(cube) + (255,), outline=(20, 16, 12, 255))
        return im
    umin, umax = min(p[0] for p in pts), max(p[0] for p in pts)
    vmin, vmax = min(p[1] for p in pts), max(p[1] for p in pts)
    im = Image.new("RGBA", (int((umax - umin) * scale) + pad * 2, int((vmax - vmin) * scale) + pad * 2 + 12), (24, 22, 20, 255))
    d = ImageDraw.Draw(im)
    d.text((pad, 2), title, fill=(214, 198, 168))

    def tp(p):
        return pad + (p[0] - umin) * scale, pad + 12 + (p[1] - vmin) * scale
    for cube in sorted(cubes, key=lambda c: c[2][0] + c[2][1] + c[2][2]):
        x0, y0, z0 = cube[2]
        x1, y1, z1 = cube[3]
        col = _pcol(cube)
        for verts, k in (([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], 1.18),
                         ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], 0.82),
                         ([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], 0.6)):
            d.polygon([tp(proj(*v)) for v in verts], fill=_lit(col, k) + (255,), outline=(20, 16, 12, 255))
    return im


def _hcat(ims, gap=14, bg=(16, 15, 14, 255)):
    Wd = sum(i.width for i in ims) + gap * (len(ims) + 1)
    Hd = max(i.height for i in ims) + gap * 2
    cv = Image.new("RGBA", (Wd, Hd), bg)
    x = gap
    for i in ims:
        cv.paste(i, (x, gap), i)
        x += i.width + gap
    return cv


def render_part(name, out=None):
    cubes = PARTS[name]()
    views = ["face", "iso"] if name == "taotie" else ["front", "side", "iso"]
    cv = _hcat([_view(cubes, m, scale=9 if name == "taotie" else 7, title=f"{name}·{m}") for m in views])
    out = out or PREVIEW_DIR / f"bronze_part_{name}.png"
    cv.save(out)
    return out, len(cubes)


def render_full(cubes, tex, out=None):
    row = [_view(cubes, m, scale=7, title=m) for m in ("face", "side", "top", "iso")]
    tb = tex.resize((RES * 2, RES * 2), Image.NEAREST)
    cv = _hcat(row + [tb])
    out = out or PREVIEW_DIR / "bronze_coffin_preview.png"
    cv.save(out)
    return out


def summarize(cubes):
    xs = [v for c in cubes for v in (c[2][0], c[3][0])]
    ys = [v for c in cubes for v in (c[2][1], c[3][1])]
    zs = [v for c in cubes for v in (c[2][2], c[3][2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox: {bb[0]:.2f}×{bb[1]:.2f}×{bb[2]:.2f}px = {bb[0]/PX:.3f}W {bb[1]/PX:.3f}H {bb[2]/PX:.3f}L 格")
    print(f"  cubes: {len(cubes)} (" + ", ".join(f"{p}:{len(PARTS[p]())}" for p in PARTS) + ")")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=list(PARTS))
    ap.add_argument("--assemble", action="store_true")
    args = ap.parse_args()
    if args.part:
        out, n = render_part(args.part)
        print(f"件 {args.part}: {n} cube → {out.relative_to(REPO)}")
        return
    cubes = all_cubes()
    print("青铜饕餮棺 / bronze_coffin (灵材三档 ×0.3):")
    summarize(cubes)
    for p in PARTS:
        out, n = render_part(p)
        print(f"  件预览 {p:7s} {n:2d} cube → {out.relative_to(REPO)}")
    model, tex = build_bbmodel(cubes)
    BBMODEL_OUT.write_text(json.dumps(model, ensure_ascii=False, indent=1))
    print(f"  → bbmodel: {BBMODEL_OUT.relative_to(REPO)} ({BBMODEL_OUT.stat().st_size} B)")
    print(f"  → full preview: {render_full(cubes, tex).relative_to(REPO)}")


if __name__ == "__main__":
    main()
