#!/usr/bin/env python3
"""青竹削直练习剑（bamboo_jian）Blockbench .bbmodel 生成器。

形制的第一顺位依据是 ``server/assets/items/weapons.toml`` 的权威定义：
``name = "竹剑"``、``description = "削直的青竹练习剑，轻便顺手却经不起硬碰，
适合初学者熟悉剑路。"``。模型因此是一把单件、青绿色、直削且钝脆的竹剑；
竹节是竹管自然变厚的节，不是钢环或金属护具。

``pair=True`` 只保留给旧的 JianPlayer 双手动画/工具测试，用两份相同的单剑几何
并列生成预览；运行时物品和 ``--single`` 源模型始终是一把剑。

几何沿 Y 轴从柄尾到钝尖：简单竹柄 → 朴素竹制护手 → 直削竹片剑身（八个自然竹节）
→ 截平的脆弱剑尖。剑身用扁平盒而不是圆柱截面，避免把同一个拼音误读成另一种兵器。

用法:
    python3 modelScript/generators/gen_bamboo_jian.py               # 双剑兼容预览
    python3 modelScript/generators/gen_bamboo_jian.py --single      # 单件手持 item 源模型
    python3 modelScript/generators/gen_bamboo_jian.py --preview-only
    bbmodel-render modelScript/models/BambooJianSingle.bbmodel --three-view
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
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "BambooJian.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "bamboo_jian_preview.png"

PX = 16.0
RES = 64
PAIR_DX = 3.5  # 旧双手动画预览中两把单剑的 x 偏移

# ── 纵向分段（y，柄尾 = 0）────────────────────────────────────────────────
POMMEL_Y = (0.00, 0.45)   # 竹根截面盖，不做金属 pommel
GRIP_Y = (0.45, 4.05)     # 朴素竹柄
GUARD_Y = (4.05, 4.42)    # 简单竹制护手
BLADE_Y0 = 4.42           # 直削竹片剑身起点
BLADE_LEN = 16.25         # 剑身长，保持轻便而非重型长兵器
TIP_LEN = 0.65            # 截平的脆弱尖端

NODES = 8                 # 竹节数
NODE_LEN0 = 2.15
NODE_LEN_STEP = 0.018
HW_ROOT = 1.16            # 竹片根部半宽；扁平剑身而非圆柱
HW_TIP = 0.72             # 剑尖半宽，仍保留可读的练习剑面
DEPTH_ROOT = 0.28         # 削平竹片的薄厚
DEPTH_TIP = 0.17
NODE_BULGE = 0.13         # 自然竹节稍厚，不做独立金属环
NODE_H = 0.25

BONE_ORDER = ["blade", "guard", "grip", "pommel"]
BONE_COLORS = {
    "blade": (148, 181, 72),
    "guard": (112, 145, 54),
    "grip": (126, 157, 57),
    "pommel": (188, 168, 78),
}
# 贴图分区（同一张 64²，按材质划带；Packer 各自在带内打 UV）
MAT_ZONE = {
    "bamboo": (0, 0, RES, 28),
    "bamboo_node": (0, 28, RES, 42),
    "fiber": (0, 42, RES, 55),
    "cut": (0, 55, RES, RES),
}


def segments():
    """八节竹节：返回 ``[(y0, y1, half_width, half_depth)]``。"""
    lens = [NODE_LEN0 - i * NODE_LEN_STEP for i in range(NODES)]
    scale = BLADE_LEN / sum(lens)
    lens = [ln * scale for ln in lens]
    out, y = [], BLADE_Y0
    for i, ln in enumerate(lens):
        ratio = i / (NODES - 1)
        hw = HW_ROOT + (HW_TIP - HW_ROOT) * ratio
        depth = DEPTH_ROOT + (DEPTH_TIP - DEPTH_ROOT) * ratio
        out.append((y, y + ln, hw, depth))
        y += ln
    return out


def build_cubes(dx: float = 0.0, side: str = "r"):
    """返回 ``[(bone, material, name, from, to, rotation)]``。

    剑身是薄的直削竹片；八角柱只用于柄尾和握把，避免把剑身做成圆柱。
    """
    cubes: list[tuple] = []

    def add(bone, mat, name, hw, y0, y1, rot_y=0.0, hz=None):
        hz = hw if hz is None else hz
        cubes.append((bone, mat, f"{name}_{side}",
                      [dx - hw, y0, -hz], [dx + hw, y1, hz], (0.0, rot_y, 0.0)))

    def octagon(bone, mat, name, hw, y0, y1):
        add(bone, mat, f"{name}_a", hw, y0, y1, 0.0)
        add(bone, mat, f"{name}_b", hw, y0, y1, 45.0)

    def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0)):
        cubes.append((bone, mat, f"{name}_{side}",
                      [dx + x0, y0, z0], [dx + x1, y1, z1], tuple(rot)))

    # ── pommel —— 竹根截面盖：轻、朴素，不加入金属 pommel ──────────────
    octagon("pommel", "cut", "pommel_cap", 0.48, POMMEL_Y[0], POMMEL_Y[0] + 0.18)
    octagon("pommel", "bamboo", "pommel_stem", 0.55, POMMEL_Y[0] + 0.18, POMMEL_Y[1])

    # ── grip —— 直竹柄，只用几道深色纤维标出握持区 ─────────────────────
    octagon("grip", "bamboo", "grip_body", 0.58, GRIP_Y[0], GRIP_Y[1])
    for i, y0 in enumerate((1.10, 2.22, 3.34)):
        octagon("grip", "fiber", f"grip_wrap_{i}", 0.63, y0, y0 + 0.12)

    # ── guard —— 极简竹片护手；不用黄铜龙首或复杂金具 ───────────────────
    block("guard", "bamboo_node", "guard_bar", -1.18, 1.18, GUARD_Y[0], GUARD_Y[1], -0.18, 0.18)
    block("guard", "bamboo", "guard_left_cap", -1.30, -1.08,
          GUARD_Y[0] + 0.04, GUARD_Y[1] - 0.04, -0.22, 0.22)
    block("guard", "bamboo", "guard_right_cap", 1.08, 1.30,
          GUARD_Y[0] + 0.04, GUARD_Y[1] - 0.04, -0.22, 0.22)

    # ── blade —— 直削竹片；每个竹节是同材质自然加厚，不是钢环 ───────────
    for i, (y0, y1, hw, depth) in enumerate(segments()):
        block("blade", "bamboo", f"blade_segment_{i}", -hw, hw,
              y0, y1 - NODE_H, -depth, depth)
        block("blade", "bamboo_node", f"node_{i}", -hw - NODE_BULGE, hw + NODE_BULGE,
              y1 - NODE_H, y1, -depth - 0.06, depth + 0.06)

    # ── blade —— 截平钝尖：刻意保留脆弱练习剑的轻薄末端 ───────────────
    y_tip = BLADE_Y0 + BLADE_LEN
    block("blade", "bamboo", "tip_shoulder", -0.58, 0.58,
          y_tip, y_tip + TIP_LEN * 0.55, -0.14, 0.14)
    block("blade", "cut", "tip_blunt", -0.46, 0.46,
          y_tip + TIP_LEN * 0.55, y_tip + TIP_LEN, -0.11, 0.11)

    return [c for c in cubes if c is not None]


# ── 贴图 ──────────────────────────────────────────────────────────────────
def make_texture(res=RES, seed=73):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 青竹 y[0,28)：黄绿底色 + 纵向竹纤维。颜色必须读作竹，不是金属。
    bmask = y < 28
    grain = 0.5 + 0.5 * np.sin(x * 1.15 + np.sin(y * 0.22) * 0.8)
    bcol = np.array([148, 178, 67], float)[None, None, :] + (grain[..., None] - 0.5) * 34
    bcol += (rng.random((res, res, 1)) - 0.5) * 8
    bcol = np.clip(bcol, 76, 214)
    img[bmask, :3] = bcol[bmask].astype(np.uint8)

    # 竹节 y[28,42)：比节间深、但仍是青竹色；没有钢环材质。
    nmask = (y >= 28) & (y < 42)
    node_grain = 0.5 + 0.5 * np.sin(x * 1.7 + y * 0.11)
    ncol = np.array([103, 139, 46], float)[None, None, :] + (node_grain[..., None] - 0.5) * 28
    ncol += (rng.random((res, res, 1)) - 0.5) * 7
    ncol = np.clip(ncol, 50, 180)
    img[nmask, :3] = ncol[nmask].astype(np.uint8)

    # 纤维缠带 y[42,55)：简单、偏暗的天然纤维。
    fmask = (y >= 42) & (y < 55)
    fgrain = 0.5 + 0.5 * np.sin(x * 2.0 + y * 0.35)
    fcol = np.array([67, 86, 34], float)[None, None, :] + (fgrain[..., None] - 0.5) * 24
    fcol += (rng.random((res, res, 1)) - 0.5) * 6
    fcol = np.clip(fcol, 28, 130)
    img[fmask, :3] = fcol[fmask].astype(np.uint8)

    # 竹根与截平尖端 y[55,64)：浅黄的切面，强调练习剑经不起硬碰。
    cmask = y >= 55
    cgrain = 0.5 + 0.5 * np.sin(x * 1.4)
    ccol = np.array([191, 170, 82], float)[None, None, :] + (cgrain[..., None] - 0.5) * 25
    ccol += (rng.random((res, res, 1)) - 0.5) * 8
    ccol = np.clip(ccol, 92, 228)
    img[cmask, :3] = ccol[cmask].astype(np.uint8)

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


def _base_name(name: str) -> str:
    """去掉 _r/_l 后缀——双手兼容预览的两把剑共用一套 UV。"""
    return name.rsplit("_", 1)[0]


def _side_of(name: str) -> str:
    return name.rsplit("_", 1)[1]


def build_bbmodel(pair: bool = True):
    sides = [(PAIR_DX, "r"), (-PAIR_DX, "l")] if pair else [(0.0, "r")]
    all_cubes = [c for dx, side in sides for c in build_cubes(dx, side)]

    packers = {m: Packer(*z) for m, z in MAT_ZONE.items()}
    uv_cache: dict[str, dict] = {}
    elements = []
    groups = {side: {b: [] for b in BONE_ORDER} for _, side in sides}

    for bone, material, name, frm, to, rot in all_cubes:
        key = _base_name(name)
        if key not in uv_cache:
            uv_cache[key] = cube_faces_uv(frm, to, packers[material])
        # 旋转中心取该盒自身中轴（不是 bone pivot，否则 45° 会把盒甩离剑轴）
        cx = (frm[0] + to[0]) / 2
        cy = (frm[1] + to[1]) / 2
        cz = (frm[2] + to[2]) / 2
        elements.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False,
            "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
            "uuid": str(uuid.uuid4()),
            "from": [round(v, 3) for v in frm], "to": [round(v, 3) for v in to],
            "autouv": 0, "color": BONE_ORDER.index(bone), "origin": [round(cx, 3), round(cy, 3), round(cz, 3)],
            "rotation": [round(r, 3) for r in rot],
            "faces": {k: {"uv": list(v["uv"]), "texture": 0} for k, v in uv_cache[key].items()},
        })
        groups[_side_of(name)][bone].append(elements[-1]["uuid"])

    outliner = []
    for dx, side in sides:
        children = [{
            "name": f"{bone}_{side}", "origin": [dx, 0.0, 0.0],
            "color": BONE_ORDER.index(bone), "uuid": str(uuid.uuid4()), "export": True,
            "mirror_uv": False, "isOpen": False, "locked": False, "visibility": True,
            "autouv": 0, "children": groups[side][bone],
        } for bone in BONE_ORDER]
        outliner.append({
            "name": "bamboo_sword_right" if side == "r" else "bamboo_sword_left",
            "origin": [dx, 0.0, 0.0], "color": 0, "uuid": str(uuid.uuid4()), "export": True,
            "isOpen": True, "locked": False, "visibility": True,
            "mirror_uv": False, "autouv": 0, "children": children,
        })

    tex = make_texture()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "bamboo_jian", "model_identifier": "geometry.bong.bamboo_jian",
        "visible_box": [1.5, 2.0, 1.0], "resolution": {"width": RES, "height": RES},
        "elements": elements, "outliner": outliner,
        "textures": [{
            "path": "", "name": "bamboo_jian.png", "folder": "item", "namespace": "bong",
            "id": "0", "width": RES, "height": RES, "uv_width": RES, "uv_height": RES,
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": False, "uuid": str(uuid.uuid4()), "source": png_data_url(tex),
        }],
    }
    return model, all_cubes, tex


# ── 示意预览（真实长相以 bbmodel-render 为准）──────────────────────────────
def _aabb(frm, to, rot):
    """示意图按 Y 旋转算 AABB；真长相以渲染器为准。"""
    rot_y = rot[1] if isinstance(rot, (tuple, list)) else rot
    if abs(rot_y) < 1e-6:
        return frm, to
    cx, cz = (frm[0] + to[0]) / 2, (frm[2] + to[2]) / 2
    hx, hz = (to[0] - frm[0]) / 2, (to[2] - frm[2]) / 2
    a = math.radians(rot_y)
    ex = abs(hx * math.cos(a)) + abs(hz * math.sin(a))
    ez = abs(hx * math.sin(a)) + abs(hz * math.cos(a))
    return [cx - ex, frm[1], cz - ez], [cx + ex, to[1], cz + ez]


def render_preview(cubes, tex, out=PREVIEW_OUT):
    scale, pad, gap = 12, 16, 24
    boxes = [(b, *_aabb(f, t, r)) for b, _m, _n, f, t, r in cubes]

    def lit(color, k):
        return tuple(int(np.clip(c * k, 0, 255)) for c in color)

    def ortho(ax_u, ax_v, title):
        us = [v for _b, f, t in boxes for v in (f[ax_u], t[ax_u])]
        vs = [v for _b, f, t in boxes for v in (f[ax_v], t[ax_v])]
        umin, umax, vmin, vmax = min(us), max(us), min(vs), max(vs)
        im = Image.new("RGBA", (int((umax - umin) * scale) + pad * 2,
                                int((vmax - vmin) * scale) + pad * 2 + 14), (30, 30, 34, 255))
        d = ImageDraw.Draw(im)
        d.text((pad, 3), title, fill=(220, 220, 220))

        def to_px(u, v):
            return pad + (u - umin) * scale, pad + 14 + ((vmax - vmin) * scale - (v - vmin) * scale)

        for bone, frm, to in sorted(boxes, key=lambda c: c[1][3 - ax_u - ax_v]):
            x0, y0 = to_px(frm[ax_u], frm[ax_v])
            x1, y1 = to_px(to[ax_u], to[ax_v])
            d.rectangle([min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)],
                        fill=lit(BONE_COLORS[bone], 1.0), outline=(18, 16, 14, 255))
        return im

    tiles = [ortho(0, 1, "FRONT (X-Y) 直削竹剑"), ortho(2, 1, "SIDE (Z-Y) 薄竹片侧面"),
             ortho(0, 2, "TOP (X-Z) 扁平竹片")]
    tw = sum(t.width for t in tiles) + gap * (len(tiles) + 1)
    th = max(t.height for t in tiles)
    tex_big = tex.resize((RES * 3, RES * 3), Image.NEAREST)
    canvas = Image.new("RGBA", (max(tw, tex_big.width + gap * 2),
                                th + tex_big.height + gap * 3 + 14), (18, 18, 20, 255))
    x = gap
    for t in tiles:
        canvas.paste(t, (x, gap), t)
        x += t.width + gap
    canvas.paste(tex_big, (gap, th + gap * 2 + 14), tex_big)
    d = ImageDraw.Draw(canvas)
    d.text((gap, th + gap * 2), "TEXTURE 64x64 (x3) — bamboo / node / fiber / cut",
           fill=(200, 200, 200))
    d.text((gap * 2 + tex_big.width, th + gap * 2 + 14),
           "bones: " + "  ".join(BONE_ORDER), fill=(180, 180, 180))
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    return out


def summarize(cubes, pair: bool):
    xs = [v for _b, _m, _n, f, t, r in cubes for v in (_aabb(f, t, r)[0][0], _aabb(f, t, r)[1][0])]
    ys = [v for _b, _m, _n, f, t, _r in cubes for v in (f[1], t[1])]
    zs = [v for _b, _m, _n, f, t, r in cubes for v in (_aabb(f, t, r)[0][2], _aabb(f, t, r)[1][2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox  : {bb[0]:.1f}×{bb[1]:.1f}×{bb[2]:.1f}px = "
          f"{bb[0] / PX:.2f}W × {bb[1] / PX:.2f}H × {bb[2] / PX:.2f}D 格")
    print(f"  单剑长: {bb[1]:.1f}px（玩家模型 32px 的 {bb[1] / 32 * 100:.0f}%）"
          f"{'  ×2 并列' if pair else ''}")
    print(f"  竹节  : {NODES} 节，根宽 {HW_ROOT * 2:.2f}px → 尖宽 {HW_TIP * 2:.2f}px，"
          f"厚度 {DEPTH_ROOT * 2:.2f}px → {DEPTH_TIP * 2:.2f}px")
    print(f"  cubes : {len(cubes)} ("
          + ", ".join(f"{b}:{sum(1 for c in cubes if c[0] == b)}" for b in BONE_ORDER) + ")")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--single", action="store_true", help="只生成一把（导手持 item 模型用）")
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    pair = not args.single
    out_bb = BBMODEL_OUT if pair else BBMODEL_OUT.with_name("BambooJianSingle.bbmodel")
    out_png = PREVIEW_OUT if pair else PREVIEW_OUT.with_name("bamboo_jian_single_preview.png")

    model, cubes, tex = build_bbmodel(pair=pair)
    print("青竹练习剑 / bamboo_jian (pair preview):" if pair else "青竹练习剑 / bamboo_jian (single):")
    summarize(cubes, pair)
    if not args.preview_only:
        out_bb.parent.mkdir(parents=True, exist_ok=True)
        out_bb.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {out_bb.relative_to(REPO)} ({out_bb.stat().st_size} B)")
    p = render_preview(cubes, tex, out=out_png)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
