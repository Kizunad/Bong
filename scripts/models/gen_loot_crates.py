#!/usr/bin/env python3
"""末法 LootCrate 五变种 Blockbench .bbmodel 生成器。

世界散布战利品容器（loot crate）家族，对齐延寿棺/货箱管线（gen_trade_crate.py 同款）：

    local_models/LootCrate<Variant>.bbmodel   ← 本脚本产物（Blockbench 源）
    → Blockbench 导出 → assets/bong/geo/loot_crate_<variant>.geo.json
    → 贴图 assets/bong/textures/entity/loot_crate_<variant>.png

五变种（剪影/材质刻意拉开差异，末法残土视觉语言）：
    bone_lash  骨扎皮箱 —— 兽骨框架 + 兽皮蒙面 + 筋腱捆扎（拾荒者手作）
    talisman   符封遗匣 —— 宗门制式漆木箱 + 铜角 + 十字封条符纸（废墟遗产）
    rust_trunk 锈铁行军箱 —— 铆钉铁皮 + 挂锁 + 侧提手（末法军旅残留）
    vine_chest 藤蚀腐木箱 —— 缺板漏风 + 苔藓 + 藤蔓缠角 + 半开盖（野外朽弃）
    ash_urn    残灰陶瓮 —— 阶梯瓮身 + 封坛红布 + 草绳捆扎（埋藏窖藏）

所有变种统一带 lid（或 seal）独立骨骼 + 后侧铰链 pivot，供开箱搜索动画复用。

用法:
    python3 scripts/models/gen_loot_crates.py                 # 生成全部 5 个
    python3 scripts/models/gen_loot_crates.py --variant ash_urn
    python3 scripts/models/gen_loot_crates.py --preview-only
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
LOCAL_MODELS = REPO / "local_models"
PREVIEW_DIR = REPO / "scripts" / "models"

PX = 16.0
RES = 64


# ═══════════════════════════════════════════════════════════════════
# 共享基建：UV 打包 / bbmodel 组装 / 预览渲染（对齐 gen_trade_crate.py）
# ═══════════════════════════════════════════════════════════════════

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
    """UV 矩形写入时按 packer 分区钳制——超区大面允许贴图压缩，
    绝不越出材质分区（越界会污染相邻材质行 / 光栅化越界撕裂）。"""
    dx, dy, dz = to[0] - frm[0], to[1] - frm[1], to[2] - frm[2]
    dims = {
        "north": (dx, dy), "south": (dx, dy),
        "east": (dz, dy), "west": (dz, dy),
        "up": (dx, dz), "down": (dx, dz),
    }
    faces = {}
    for name, (w, h) in dims.items():
        w = min(abs(w), packer.x1 - packer.x0)
        h = min(abs(h), packer.y1 - packer.y0)
        ox, oy = packer.place(w, h)
        faces[name] = {"uv": [round(ox, 2), round(oy, 2),
                              round(min(ox + w, packer.x1), 2),
                              round(min(oy + h, packer.y1), 2)],
                       "texture": 0}
    return faces


def png_data_url(img):
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def build_bbmodel(spec, cubes, tex):
    """spec: VariantSpec；cubes: [(bone, material, name, from, to)]。"""
    packers = {m: Packer(x0, y0, x1, y1)
               for m, (x0, y0, x1, y1) in spec.material_zones.items()}
    elements = []
    bone_children = {b: [] for b in spec.bone_order}

    for bone, material, name, frm, to in cubes:
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
            "color": spec.bone_order.index(bone),
            "origin": list(spec.bone_pivots[bone]),
            "faces": cube_faces_uv(frm, to, packers[material]),
        })
        bone_children[bone].append(euid)

    outliner = []
    for bone in spec.bone_order:
        outliner.append({
            "name": bone,
            "origin": list(spec.bone_pivots[bone]),
            "color": spec.bone_order.index(bone),
            "uuid": str(uuid.uuid4()),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children[bone],
        })

    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": f"loot_crate_{spec.key}",
        "model_identifier": f"geometry.bong.loot_crate_{spec.key}",
        "visible_box": [2, 2, 0.5],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "", "name": f"loot_crate_{spec.key}.png", "folder": "entity",
            "namespace": "bong", "id": "0", "width": RES, "height": RES,
            "uv_width": RES, "uv_height": RES, "particle": False,
            "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model


def render_preview(spec, cubes, tex, out):
    scale, pad, gap = 8, 16, 24
    colors = spec.bone_colors

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
                        fill=lit(colors[bone], 1.0) + (255,),
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
            col = colors[bone]
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
    d.text((gap, H_ - 16), f"{spec.key}  bones: " + "  ".join(spec.bone_order),
           fill=(180, 180, 180))
    canvas.save(out)
    return out


# ═══════════════════════════════════════════════════════════════════
# 贴图纹理原语
# ═══════════════════════════════════════════════════════════════════

def _zone_mask(res, y0, y1):
    y, _ = np.mgrid[0:res, 0:res]
    return (y >= y0) & (y < y1)


def tex_wood(img, y0, y1, base, rng, plank_w=8, rot=False):
    """竖板木纹。rot=True 时更朽（板缝更宽、更多虫洞）。"""
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    plank_id = (x // plank_w).astype(float)
    seam = ((x % plank_w) < 0.9) | ((x % plank_w) > plank_w - 1.0)
    grain = 0.5 + 0.5 * np.sin(y * 0.5 + plank_id * 1.9)
    grain += 0.22 * np.sin(y * 0.19 + plank_id * 3.3)
    grain = np.clip(grain, 0, 1)
    tone = ((rng.random(res // plank_w + 2) - 0.5) * (28 if rot else 22))[plank_id.astype(int)]
    col = np.array(base, float)[None, None, :] + (grain[..., None] - 0.5) * 30 + tone[..., None]
    col[seam] *= 0.72 if rot else 0.55
    holes = 6 if rot else 4
    for _ in range(holes):
        cx, cy = rng.integers(2, res - 2), rng.integers(y0 + 2, y1 - 2)
        r = rng.integers(1, 4)
        d = np.hypot(x - cx, y - cy)
        col[d < r] *= 0.5
    img[m, :3] = np.clip(col, 14, 220)[m].astype(np.uint8)


def tex_metal(img, y0, y1, base, rng, rust=0.0, rivets=True):
    """金属。rust>0 时叠锈斑流痕。"""
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    col = np.array(base, float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 26
    col += (np.sin(x * 0.8 + y * 0.3)[..., None]) * 6
    if rust > 0:
        rust_col = np.array([132, 74, 40], float)
        for _ in range(int(10 * rust)):
            cx = rng.integers(2, res - 2)
            cy = rng.integers(y0 + 1, y1 - 1)
            ln = rng.integers(3, 12)
            for dy in range(ln):
                yy = min(cy + dy, res - 1)
                w = max(1, int(2 - dy * 0.15))
                k = 1.0 - dy / ln * 0.7
                xs = slice(max(0, cx - w), min(res, cx + w))
                col[yy, xs] = col[yy, xs] * (1 - 0.75 * k) + rust_col[None, :] * (0.75 * k)
    if rivets:
        for cx in range(5, res, 12):
            for cy in range(y0 + 4, y1, 8):
                d = np.hypot(x - cx, y - cy)
                col[d < 1.5] += 42
                col[(d >= 1.5) & (d < 2.3)] -= 26
    img[m, :3] = np.clip(col, 18, 215)[m].astype(np.uint8)


def tex_hide(img, y0, y1, base, rng):
    """兽皮：暖褐 + 皱褶暗纹 + 缝线点。"""
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    wrinkle = np.sin(x * 0.45 + np.sin(y * 0.3) * 2.4) * 0.5 + np.sin(y * 0.23 + x * 0.11) * 0.5
    col = np.array(base, float)[None, None, :] + wrinkle[..., None] * 14
    col += (rng.random((res, res, 1)) - 0.5) * 18
    for _ in range(4):  # 深色补丁块
        cx, cy = rng.integers(4, res - 4), rng.integers(y0 + 3, y1 - 3)
        w, h = rng.integers(6, 14), rng.integers(4, 9)
        patch = (np.abs(x - cx) < w // 2) & (np.abs(y - cy) < h // 2)
        col[patch] *= 0.78
        edge = (np.abs(np.abs(x - cx) - w // 2) < 0.8) & (np.abs(y - cy) < h // 2)
        col[edge & ((x + y) % 3 < 1)] += 46  # 缝线
    img[m, :3] = np.clip(col, 22, 205)[m].astype(np.uint8)


def tex_bone(img, y0, y1, rng):
    """骨料：象牙白 + 纵向骨纹 + 关节暗环。"""
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    base = np.array([206, 196, 172], float)
    col = base[None, None, :] + np.sin(x * 1.1)[..., None] * 7
    col += (rng.random((res, res, 1)) - 0.5) * 14
    for cy in range(y0 + 3, y1, 6):  # 关节暗环
        col[np.abs(y - cy) < 1] *= 0.82
    img[m, :3] = np.clip(col, 60, 230)[m].astype(np.uint8)


def tex_flat(img, y0, y1, base, rng, noise=14):
    res = img.shape[0]
    m = _zone_mask(res, y0, y1)
    col = np.array(base, float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * noise
    img[m, :3] = np.clip(col, 14, 225)[m].astype(np.uint8)


def tex_ceramic(img, y0, y1, rng):
    """陶：残灰釉 + 裂纹网。"""
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    base = np.array([148, 138, 124], float)
    col = base[None, None, :] + np.sin(y * 0.35)[..., None] * 9
    col += (rng.random((res, res, 1)) - 0.5) * 16
    for _ in range(7):  # 裂纹折线
        cx, cy = rng.integers(3, res - 3), rng.integers(y0 + 2, y1 - 2)
        for _seg in range(rng.integers(3, 6)):
            ln = rng.integers(3, 7)
            dx, dy = rng.choice([-1, 0, 1]), rng.choice([-1, 1])
            for i in range(ln):
                px = int(np.clip(cx + dx * i, 0, res - 1))
                py = int(np.clip(cy + dy * i, y0, y1 - 1))
                col[py, px] *= 0.62
            cx, cy = px, py
    img[m, :3] = np.clip(col, 30, 210)[m].astype(np.uint8)


def tex_moss(img, y0, y1, rng):
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    base = np.array([86, 112, 58], float)
    blob = np.sin(x * 0.6 + rng.random() * 9) * np.sin(y * 0.7 + rng.random() * 9)
    col = base[None, None, :] + blob[..., None] * 18 + (rng.random((res, res, 1)) - 0.5) * 20
    img[m, :3] = np.clip(col, 26, 170)[m].astype(np.uint8)


def tex_paper(img, y0, y1, rng):
    """符纸：亮陈黄底 + 朱砂符纹（纵横双向笔画，任意 UV 切片都见红纹）。"""
    res = img.shape[0]
    y, x = np.mgrid[0:res, 0:res]
    m = _zone_mask(res, y0, y1)
    base = np.array([228, 210, 162], float)
    col = base[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 14
    cinnabar = np.array([170, 44, 36], float)
    for cx in range(3, res, 8):  # 竖向符笔画
        stroke = (np.abs(x - cx) < 1) & ((y % 5) != 0)
        col[stroke & m] = cinnabar
    for cy in range(y0 + 2, y1, 4):  # 横向断笔
        stroke = (np.abs(y - cy) < 1) & ((x % 7) < 4)
        col[stroke & m] = cinnabar
    img[m, :3] = np.clip(col, 30, 232)[m].astype(np.uint8)


# ═══════════════════════════════════════════════════════════════════
# VariantSpec + 五变种几何
# ═══════════════════════════════════════════════════════════════════

class VariantSpec:
    def __init__(self, key, title, bone_order, bone_pivots, bone_colors,
                 material_zones, build_fn, texture_fn):
        self.key = key
        self.title = title
        self.bone_order = bone_order
        self.bone_pivots = bone_pivots
        self.bone_colors = bone_colors
        self.material_zones = material_zones  # material -> (x0,y0,x1,y1) UV 区
        self.build_fn = build_fn
        self.texture_fn = texture_fn


# ── 1. bone_lash 骨扎皮箱 ────────────────────────────────────────────
def build_bone_lash():
    HW, HD, TOP, H = 7.2, 7.2, 11.6, 13.6
    c = []
    # 皮革箱身（内收，露骨框）
    c.append(("body", "hide", "hide_body", [-HW + 0.9, 0.4, -HD + 0.9], [HW - 0.9, TOP, HD - 0.9]))
    # 骨框：4 角骨柱（两段式 + 关节鼓包）
    for sx in (-1, 1):
        for sz in (-1, 1):
            xa = sx * HW - (1.8 if sx > 0 else 0)
            xb = sx * HW + (0 if sx > 0 else 1.8)
            za = sz * HD - (1.8 if sz > 0 else 0)
            zb = sz * HD + (0 if sz > 0 else 1.8)
            c.append(("frame", "bone", f"post_{sx}_{sz}", [xa, 0.0, za], [xb, TOP, zb]))
            # 关节鼓包（中段外凸）
            c.append(("frame", "bone", f"knob_{sx}_{sz}",
                      [xa - 0.35, TOP * 0.44, za - 0.35], [xb + 0.35, TOP * 0.44 + 2.0, zb + 0.35]))
    # 上下骨横梁（四面环绕）
    for yy, tag in ((0.2, "bot"), (TOP - 1.6, "top")):
        for sz in (-1, 1):
            zr = [HD - 1.2, HD + 0.2] if sz > 0 else [-HD - 0.2, -HD + 1.2]
            c.append(("frame", "bone", f"rail_{tag}_z{sz}",
                      [-HW + 1.8, yy, zr[0]], [HW - 1.8, yy + 1.4, zr[1]]))
        for sx in (-1, 1):
            xr = [HW - 1.2, HW + 0.2] if sx > 0 else [-HW - 0.2, -HW + 1.2]
            c.append(("frame", "bone", f"rail_{tag}_x{sx}",
                      [xr[0], yy, -HD + 1.8], [xr[1], yy + 1.4, HD - 1.8]))
    # 前面阶梯斜撑骨（3 段错位 = 斜杠视觉）
    for i, (x0, y0) in enumerate([(-4.6, 2.2), (-1.4, 5.2), (1.8, 8.2)]):
        c.append(("frame", "bone", f"diag_{i}",
                  [x0, y0, HD - 0.9], [x0 + 3.0, y0 + 1.2, HD + 0.35]))
    # 筋腱捆扎带（角柱上下缠绕）
    for sx in (-1, 1):
        for sz in (-1, 1):
            xa = sx * HW - (2.05 if sx > 0 else -0.25)
            xb = sx * HW + (0.25 if sx > 0 else 2.05)
            za = sz * HD - (2.05 if sz > 0 else -0.25)
            zb = sz * HD + (0.25 if sz > 0 else 2.05)
            for yy, tag in ((1.6, "b"), (TOP - 3.2, "t")):
                c.append(("lash", "sinew", f"lash_{tag}_{sx}_{sz}",
                          [xa, yy, za], [xb, yy + 1.0, zb]))
    # 盖：皮蒙框 + 2 骨肋（铰链后上）
    c.append(("lid", "hide", "lid_hide", [-HW + 0.5, TOP, -HD + 0.5], [HW - 0.5, TOP + 1.2, HD - 0.5]))
    for sx in (-1, 1):
        xc = sx * (HW - 2.8)
        c.append(("lid", "bone", f"lid_rib_{sx}",
                  [xc - 0.8, TOP + 1.2, -HD + 1.0], [xc + 0.8, H, HD - 1.0]))
    # 盖前沿骨扣
    c.append(("lid", "bone", "lid_toggle", [-1.2, TOP + 0.1, HD - 0.4], [1.2, TOP + 1.5, HD + 0.5]))
    return c


def tex_bone_lash(rng):
    img = np.zeros((RES, RES, 4), np.uint8)
    img[..., 3] = 255
    tex_hide(img, 0, 40, [104, 72, 48], rng)
    tex_bone(img, 40, 56, rng)
    tex_flat(img, 56, 64, [58, 40, 30], rng)  # 筋腱深褐
    return Image.fromarray(img, "RGBA")


SPEC_BONE_LASH = VariantSpec(
    "bone_lash", "骨扎皮箱",
    ["body", "frame", "lash", "lid"],
    {"body": [0, 0, 0], "frame": [0, 0, 0], "lash": [0, 0, 0],
     "lid": [0.0, 11.6, -6.7]},
    {"body": (122, 88, 58), "frame": (206, 196, 172),
     "lash": (74, 54, 40), "lid": (188, 172, 146)},
    {"hide": (0, 0, RES, 40), "bone": (0, 40, RES, 56), "sinew": (0, 56, RES, 64)},
    build_bone_lash, tex_bone_lash,
)


# ── 2. talisman 符封遗匣 ─────────────────────────────────────────────
def build_talisman():
    HW, HD, TOP = 7.5, 5.6, 9.0
    c = []
    # 漆木箱身
    c.append(("body", "lacquer", "body", [-HW, 0.0, -HD], [HW, TOP, HD]))
    # 底座裙边
    c.append(("body", "lacquer", "skirt", [-HW - 0.4, 0.0, -HD - 0.4], [HW + 0.4, 1.2, HD + 0.4]))
    # 铜角件（上下 4 角）+ 铜锁面
    for sx in (-1, 1):
        for sz in (-1, 1):
            xa, xb = (sx * HW - 2.2, sx * HW + 0.35) if sx > 0 else (sx * HW - 0.35, sx * HW + 2.2)
            za, zb = (sz * HD - 2.2, sz * HD + 0.35) if sz > 0 else (sz * HD - 0.35, sz * HD + 2.2)
            c.append(("fittings", "bronze", f"corner_b_{sx}_{sz}", [xa, -0.15, za], [xb, 2.6, zb]))
            c.append(("fittings", "bronze", f"corner_t_{sx}_{sz}", [xa, TOP - 2.6, za], [xb, TOP + 0.15, zb]))
    c.append(("fittings", "bronze", "lock_plate", [-1.9, 3.2, HD - 0.1], [1.9, 6.4, HD + 0.55]))
    c.append(("fittings", "bronze", "lock_hasp", [-0.8, 4.6, HD + 0.55], [0.8, 6.6, HD + 1.05]))
    # 拱盖两级（铰链后上）
    c.append(("lid", "lacquer", "lid_lower", [-HW - 0.3, TOP, -HD - 0.3], [HW + 0.3, TOP + 1.6, HD + 0.3]))
    c.append(("lid", "lacquer", "lid_upper", [-HW + 1.6, TOP + 1.6, -HD + 1.4], [HW - 1.6, TOP + 3.0, HD - 1.4]))
    c.append(("lid", "bronze", "lid_spine", [-HW + 2.4, TOP + 3.0, -0.8], [HW - 2.4, TOP + 3.5, 0.8]))
    # 封条符纸：一条自盖顶中线翻过前沿、垂到锁扣（3 段贴面），一条横贴前脸 = 十字封
    c.append(("seal", "paper", "strip_v_top", [-1.2, TOP + 3.0 + 0.08, -1.0], [1.2, TOP + 3.32, HD - 1.35]))
    c.append(("seal", "paper", "strip_v_step", [-1.2, TOP + 1.6 + 0.08, HD - 1.65], [1.2, TOP + 3.15, HD + 0.42]))
    c.append(("seal", "paper", "strip_v_front", [-1.2, 3.2, HD + 0.6], [1.2, TOP + 1.85, HD + 0.86]))
    c.append(("seal", "paper", "strip_h_front", [-HW + 1.2, 4.4, HD + 0.62], [HW - 1.2, 6.0, HD + 0.84]))
    return c


def tex_talisman(rng):
    img = np.zeros((RES, RES, 4), np.uint8)
    img[..., 3] = 255
    # 漆面：深朱漆 + 剥落露木
    y, x = np.mgrid[0:RES, 0:RES]
    m = _zone_mask(RES, 0, 40)
    base = np.array([96, 40, 34], float)
    col = base[None, None, :] + (np.sin(x * 0.3 + y * 0.17))[..., None] * 8
    col += (rng.random((RES, RES, 1)) - 0.5) * 14
    for _ in range(6):  # 剥落斑（露灰木底）
        cx, cy = rng.integers(4, RES - 4), rng.integers(3, 36)
        w, h = rng.integers(3, 9), rng.integers(2, 5)
        flake = (np.abs(x - cx) < w) & (np.abs(y - cy) < h)
        col[flake] = np.array([124, 106, 84], float) + (rng.random(3) - 0.5) * 10
    img[m, :3] = np.clip(col, 20, 200)[m].astype(np.uint8)
    # 铜件：暗铜底 + 铜绿蚀斑
    m2 = _zone_mask(RES, 40, 56)
    bronze = np.array([88, 66, 34], float)[None, None, :] + (rng.random((RES, RES, 1)) - 0.5) * 18
    for _ in range(8):
        cx, cy = rng.integers(3, RES - 3), rng.integers(42, 54)
        d = np.hypot(x - cx, y - cy)
        verd = d < rng.integers(2, 4)
        bronze[verd] = np.array([74, 118, 92], float) + (rng.random(3) - 0.5) * 12
    img[m2, :3] = np.clip(bronze, 20, 190)[m2].astype(np.uint8)
    tex_paper(img, 56, 64, rng)
    return Image.fromarray(img, "RGBA")


SPEC_TALISMAN = VariantSpec(
    "talisman", "符封遗匣",
    ["body", "fittings", "lid", "seal"],
    {"body": [0, 0, 0], "fittings": [0, 0, 0],
     "lid": [0.0, 9.0, -5.9], "seal": [0.0, 9.0, -5.9]},
    {"body": (96, 40, 34), "fittings": (140, 112, 62),
     "lid": (120, 52, 44), "seal": (214, 196, 150)},
    {"lacquer": (0, 0, RES, 40), "bronze": (0, 40, RES, 56), "paper": (0, 56, RES, 64)},
    build_talisman, tex_talisman,
)


# ── 3. rust_trunk 锈铁行军箱 ─────────────────────────────────────────
def build_rust_trunk():
    HW, HD, TOP = 8.0, 5.0, 8.6
    c = []
    c.append(("body", "iron", "shell", [-HW, 0.0, -HD], [HW, TOP, HD]))
    # 竖向加强筋（前后各 3）
    for sz in (-1, 1):
        for xc in (-5.0, 0.0, 5.0):
            zr = [HD, HD + 0.5] if sz > 0 else [-HD - 0.5, -HD]
            c.append(("ribs", "darkiron", f"rib_z{sz}_{int(xc)}",
                      [xc - 0.8, 0.3, zr[0]], [xc + 0.8, TOP - 0.3, zr[1]]))
    # 边沿包条（上口一圈）
    c.append(("ribs", "darkiron", "rim_f", [-HW - 0.3, TOP - 1.0, HD - 0.2], [HW + 0.3, TOP + 0.1, HD + 0.35]))
    c.append(("ribs", "darkiron", "rim_b", [-HW - 0.3, TOP - 1.0, -HD - 0.35], [HW + 0.3, TOP + 0.1, -HD + 0.2]))
    # 凹陷补丁（一块外凸错位板 = 砸瘪修补感）
    c.append(("ribs", "darkiron", "dent_patch", [2.2, 1.6, HD + 0.12], [6.4, 4.2, HD + 0.42]))
    # 侧提手（U 形 3 段 × 2 侧）
    for sx in (-1, 1):
        xo = sx * HW
        xr = [xo, xo + 0.5] if sx > 0 else [xo - 0.5, xo]
        c.append(("ribs", "darkiron", f"handle_bar_{sx}",
                  [xr[0] + (0.5 if sx > 0 else -0.5), 4.6, -2.2],
                  [xr[1] + (0.5 if sx > 0 else -0.5), 5.4, 2.2]))
        for zc in (-2.2, 2.2):
            c.append(("ribs", "darkiron", f"handle_leg_{sx}_{int(zc)}",
                      [xr[0], 4.0, zc - 0.4], [xr[1], 5.4, zc + 0.4]))
    # 平盖 + 前唇（铰链后上）
    c.append(("lid", "iron", "lid_plate", [-HW - 0.35, TOP, -HD - 0.35], [HW + 0.35, TOP + 1.5, HD + 0.35]))
    c.append(("lid", "iron", "lid_lip", [-HW + 1.0, TOP - 0.8, HD + 0.35], [HW - 1.0, TOP, HD + 0.6]))
    # 搭扣 + 挂锁（黄铜锁体醒目 + U 梁两腿一梁）
    c.append(("lock", "darkiron", "hasp", [-1.3, TOP - 2.0, HD + 0.4], [1.3, TOP + 0.9, HD + 0.85]))
    c.append(("lock", "brass", "pad_body", [-1.7, 2.9, HD + 0.7], [1.7, 5.4, HD + 1.9]))
    for sx in (-1, 1):
        c.append(("lock", "darkiron", f"pad_leg_{sx}",
                  [sx * 0.9 - 0.35, 5.4, HD + 1.0], [sx * 0.9 + 0.35, 6.6, HD + 1.6]))
    c.append(("lock", "darkiron", "pad_bar", [-1.25, 6.6, HD + 1.0], [1.25, 7.3, HD + 1.6]))
    return c


def tex_rust_trunk(rng):
    img = np.zeros((RES, RES, 4), np.uint8)
    img[..., 3] = 255
    tex_metal(img, 0, 44, [126, 128, 134], rng, rust=2.2, rivets=True)
    tex_metal(img, 44, 56, [66, 68, 76], rng, rust=0.4, rivets=False)
    tex_metal(img, 56, 64, [150, 118, 58], rng, rust=0.3, rivets=False)  # 黄铜锁体
    return Image.fromarray(img, "RGBA")


SPEC_RUST_TRUNK = VariantSpec(
    "rust_trunk", "锈铁行军箱",
    ["body", "ribs", "lock", "lid"],
    {"body": [0, 0, 0], "ribs": [0, 0, 0], "lock": [0, 0, 0],
     "lid": [0.0, 8.6, -5.35]},
    {"body": (126, 128, 134), "ribs": (66, 68, 76),
     "lock": (150, 118, 58), "lid": (140, 130, 116)},
    {"iron": (0, 0, RES, 44), "darkiron": (0, 44, RES, 56), "brass": (0, 56, RES, 64)},
    build_rust_trunk, tex_rust_trunk,
)


# ── 4. vine_chest 藤蚀腐木箱 ─────────────────────────────────────────
def build_vine_chest():
    HW, HD, TOP = 7.2, 6.2, 10.4
    c = []
    # 内衬暗腔（严格内缩于四壁之内，防 z-fight，从缺板缝看进去）
    c.append(("body", "dark", "cavity", [-HW + 1.1, 1.0, -HD + 1.1], [HW - 1.1, TOP - 0.6, HD - 1.1]))
    # 前脸 4 块竖板缺 1（x∈[-1.2,1.8) 缺口）
    for i, (x0, x1) in enumerate([(-HW, -4.2), (-4.2, -1.2), (1.8, 4.6), (4.6, HW)]):
        c.append(("body", "rotwood", f"front_plank_{i}", [x0, 0.0, HD - 0.9], [x1, TOP, HD]))
    # 其余三面整板
    c.append(("body", "rotwood", "back", [-HW, 0.0, -HD], [HW, TOP, -HD + 0.9]))
    for sx in (-1, 1):
        xr = [HW - 0.9, HW] if sx > 0 else [-HW, -HW + 0.9]
        c.append(("body", "rotwood", f"side_{sx}", [xr[0], 0.0, -HD + 0.9], [xr[1], TOP, HD - 0.9]))
    c.append(("body", "rotwood", "bottom", [-HW, 0.0, -HD], [HW, 0.9, HD]))
    # 断裂板残茬（缺口下半截残留）
    c.append(("body", "rotwood", "broken_stub", [-1.2, 0.0, HD - 0.85], [1.8, 3.4, HD + 0.05]))
    # 半开盖：整体抬起 + 前缘垫石（无旋转，用错位读出"翘开"）
    LIFT = 1.7
    c.append(("lid", "rotwood", "lid_board",
              [-HW - 0.3, TOP + LIFT, -HD - 0.3], [HW + 0.3, TOP + LIFT + 1.3, HD + 0.6]))
    c.append(("lid", "rotwood", "lid_rib",
              [-HW + 2.2, TOP + LIFT + 1.3, -1.0], [HW - 2.2, TOP + LIFT + 2.1, 1.0]))
    c.append(("prop", "dark", "prop_stone", [3.6, TOP - 0.4, HD - 2.2], [5.8, TOP + LIFT, HD - 0.2]))
    # 苔藓斑（盖顶 2 + 侧上沿 1 + 前板 1）
    c.append(("moss", "moss", "moss_lid_1", [-5.4, TOP + LIFT + 1.3, -3.4], [-1.2, TOP + LIFT + 1.75, 0.6]))
    c.append(("moss", "moss", "moss_lid_2", [1.6, TOP + LIFT + 1.3, 1.0], [5.2, TOP + LIFT + 1.7, 4.4]))
    c.append(("moss", "moss", "moss_side", [-HW - 0.25, TOP - 2.6, -3.0], [-HW + 0.35, TOP + 0.4, 1.8]))
    c.append(("moss", "moss", "moss_front", [2.6, TOP - 1.8, HD - 0.2], [5.4, TOP + 0.3, HD + 0.28]))
    # 藤蔓：左前角自上而下缠绕（竖 3 段 + 横搭 2 段错位）
    vx = -HW
    segs = [
        ([vx - 0.4, 6.4, HD - 1.6], [vx + 0.35, TOP + LIFT + 2.0, HD - 0.7]),
        ([vx - 0.4, 2.4, HD - 3.4], [vx + 0.35, 7.0, HD - 2.5]),
        ([vx - 0.4, 0.0, HD - 5.4], [vx + 0.35, 3.0, HD - 4.5]),
        ([vx - 0.45, 4.9, HD - 3.35], [vx + 0.4, 5.7, HD - 1.5]),
        ([vx - 0.45, 8.4, HD - 2.55], [vx + 0.4, 9.2, HD - 0.9]),
    ]
    for i, (f, t) in enumerate(segs):
        c.append(("vine", "vine", f"vine_{i}", f, t))
    # 右后角一条短藤
    c.append(("vine", "vine", "vine_r1", [HW - 0.35, 4.0, -HD - 0.4], [HW + 0.4, TOP + LIFT + 1.6, -HD + 0.5]))
    return c


def tex_vine_chest(rng):
    img = np.zeros((RES, RES, 4), np.uint8)
    img[..., 3] = 255
    tex_wood(img, 0, 36, [112, 100, 74], rng, plank_w=9, rot=True)
    tex_moss(img, 36, 50, rng)
    tex_flat(img, 50, 57, [58, 78, 44], rng)   # 藤蔓深绿
    tex_flat(img, 57, 64, [34, 30, 26], rng, noise=8)  # 内腔暗
    return Image.fromarray(img, "RGBA")


SPEC_VINE_CHEST = VariantSpec(
    "vine_chest", "藤蚀腐木箱",
    ["body", "prop", "moss", "vine", "lid"],
    {"body": [0, 0, 0], "prop": [0, 0, 0], "moss": [0, 0, 0],
     "vine": [0, 0, 0], "lid": [0.0, 10.4, -6.5]},
    {"body": (116, 102, 76), "prop": (70, 66, 60), "moss": (86, 112, 58),
     "vine": (58, 78, 44), "lid": (132, 116, 88)},
    {"rotwood": (0, 0, RES, 36), "moss": (0, 36, RES, 50),
     "vine": (0, 50, RES, 57), "dark": (0, 57, RES, 64)},
    build_vine_chest, tex_vine_chest,
)


# ── 5. ash_urn 残灰陶瓮 ──────────────────────────────────────────────
def build_ash_urn():
    c = []
    # 阶梯瓮身：足→下腹→大腹→肩→颈（居中方台阶读作圆瓮）
    profile = [
        ("foot",     3.4, 0.0, 1.4),
        ("belly_lo", 5.2, 1.4, 4.6),
        ("belly",    6.2, 4.6, 9.8),
        ("shoulder", 5.0, 9.8, 12.2),
        ("neck",     3.2, 12.2, 14.2),
    ]
    for name, hw, y0, y1 in profile:
        c.append(("body", "ceramic", name, [-hw, y0, -hw], [hw, y1, hw]))
    # 草绳：腹部两道箍（每道 = 4 面贴壁窄带，非实心板）+ 肩部十字压绳 + 侧结
    B = 6.2  # 大腹半宽
    for yy, tag in ((5.6, "lo"), (8.4, "hi")):
        for sz in (-1, 1):
            zr = [B, B + 0.5] if sz > 0 else [-B - 0.5, -B]
            c.append(("rope", "rope", f"hoop_{tag}_z{sz}",
                      [-B - 0.5, yy, zr[0]], [B + 0.5, yy + 0.9, zr[1]]))
        for sx in (-1, 1):
            xr = [B, B + 0.5] if sx > 0 else [-B - 0.5, -B]
            c.append(("rope", "rope", f"hoop_{tag}_x{sx}",
                      [xr[0], yy, -B], [xr[1], yy + 0.9, B]))
    c.append(("rope", "rope", "knot_side", [6.55, 6.3, -0.9], [7.55, 8.1, 0.9]))
    # 经典封坛：红布蒙口（盖过颈沿）→ 十字压绳勒在布上 → 顶心绳结；布四角垂坠
    c.append(("seal", "cloth", "cloth_top", [-4.1, 14.2, -4.1], [4.1, 14.8, 4.1]))
    for sx in (-1, 1):
        for sz in (-1, 1):
            c.append(("seal", "cloth", f"cloth_tab_{sx}_{sz}",
                      [sx * 3.55 - 0.6, 13.1, sz * 3.55 - 0.6],
                      [sx * 3.55 + 0.6, 14.35, sz * 3.55 + 0.6]))
    c.append(("seal", "rope", "tie_cross_x", [-4.1, 14.8, -0.8], [4.1, 15.45, 0.8]))
    c.append(("seal", "rope", "tie_cross_z", [-0.8, 14.8, -4.1], [0.8, 15.45, 4.1]))
    c.append(("seal", "rope", "tie_knot", [-1.0, 15.45, -1.0], [1.0, 16.3, 1.0]))
    return c


def tex_ash_urn(rng):
    img = np.zeros((RES, RES, 4), np.uint8)
    img[..., 3] = 255
    tex_ceramic(img, 0, 44, rng)
    # 草绳：干草黄偏深 + 斜纹搓绳（与灰陶拉开对比）
    y, x = np.mgrid[0:RES, 0:RES]
    m = _zone_mask(RES, 44, 56)
    base = np.array([150, 120, 62], float)
    twist = np.sin((x + y) * 1.15) * 20
    col = base[None, None, :] + twist[..., None] + (rng.random((RES, RES, 1)) - 0.5) * 16
    img[m, :3] = np.clip(col, 34, 200)[m].astype(np.uint8)
    tex_flat(img, 56, 64, [142, 46, 40], rng, noise=20)  # 封坛褪色红布
    return Image.fromarray(img, "RGBA")


SPEC_ASH_URN = VariantSpec(
    "ash_urn", "残灰陶瓮",
    ["body", "rope", "seal"],
    {"body": [0, 0, 0], "rope": [0, 0, 0],
     "seal": [0.0, 14.2, 0.0]},  # seal 骨骼绕颈口提起（开坛动画）
    {"body": (148, 138, 124), "rope": (172, 148, 92), "seal": (142, 46, 40)},
    {"ceramic": (0, 0, RES, 44), "rope": (0, 44, RES, 56), "cloth": (0, 56, RES, 64)},
    build_ash_urn, tex_ash_urn,
)


VARIANTS = {s.key: s for s in
            [SPEC_BONE_LASH, SPEC_TALISMAN, SPEC_RUST_TRUNK, SPEC_VINE_CHEST, SPEC_ASH_URN]}


# ═══════════════════════════════════════════════════════════════════
# 汇总渲染 + main
# ═══════════════════════════════════════════════════════════════════

def summarize(spec, cubes):
    xs = [v for _, _, _, f, t in cubes for v in (f[0], t[0])]
    ys = [v for _, _, _, f, t in cubes for v in (f[1], t[1])]
    zs = [v for _, _, _, f, t in cubes for v in (f[2], t[2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    parts = ", ".join(f"{b}:{sum(1 for c in cubes if c[0] == b)}" for b in spec.bone_order)
    print(f"  bbox : {bb[0]/PX:.2f}W × {bb[1]/PX:.2f}H × {bb[2]/PX:.2f}D 格 | cubes {len(cubes)} ({parts})")


def combine_previews(paths, out):
    imgs = [Image.open(p) for p in paths]
    w = max(i.width for i in imgs)
    h = sum(i.height for i in imgs) + 8 * (len(imgs) - 1)
    canvas = Image.new("RGBA", (w, h), (12, 12, 14, 255))
    y = 0
    for im in imgs:
        canvas.paste(im, (0, y), im)
        y += im.height + 8
    canvas.save(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--variant", choices=sorted(VARIANTS), help="只生成指定变种")
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    keys = [args.variant] if args.variant else list(VARIANTS)
    previews = []
    for key in keys:
        spec = VARIANTS[key]
        rng = np.random.default_rng(sum(ord(ch) for ch in key) * 7 + 3)
        cubes = spec.build_fn()
        tex = spec.texture_fn(rng)
        print(f"{spec.title} / loot_crate_{key}:")
        summarize(spec, cubes)
        if not args.preview_only:
            out = LOCAL_MODELS / f"LootCrate{''.join(w.capitalize() for w in key.split('_'))}.bbmodel"
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(json.dumps(build_bbmodel(spec, cubes, tex), ensure_ascii=False, indent=1))
            print(f"  → bbmodel: {out.relative_to(REPO)} ({out.stat().st_size} B)")
        p = render_preview(spec, cubes, tex, PREVIEW_DIR / f"loot_crate_{key}_preview.png")
        previews.append(p)
        print(f"  → preview: {p.relative_to(REPO)}")

    if len(previews) > 1:
        allp = PREVIEW_DIR / "loot_crates_render_all.png"
        combine_previews(previews, allp)
        print(f"→ combined: {allp.relative_to(REPO)}")


if __name__ == "__main__":
    main()
