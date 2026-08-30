#!/usr/bin/env python3
"""竹节双锏（bamboo_jian）Blockbench .bbmodel 生成器。

依参考实物（竹节钢锏一对）复刻形制，自柄尾向锏尖：

    黄铜瓜棱柄首 → 深木握把（收腰）→ 铜箍 → 黄铜龙首吞口（龙口衔锏身）
    → 九节竹节锏身（每节顶端一道凸环，逐节收细）→ 黑钢钝尖

锏身是圆截面而非四棱：用"轴对齐盒 + 同尺寸 45° 盒"叠成八角柱近似圆柱
（octagon()），竹节的凸环同法加粗一圈。45° 单轴旋转在 vanilla item model
JSON 里也是合法值，走 OBJ / GeckoLib 均可。

尺寸（MC px，16px = 1 格）：单锏总长 24.0 ≈ 1.5 格，约玩家模型（32px）的
75%，换算真人尺度 ≈ 1.35m。握把 Ø1.6px（玩家手宽 4px，一握正好），锏身根径
1.8px、身长:根径 ≈ 8.5:1，锏身+尖占全长 71%（对齐参考图比例）。整体缩放只需
动 BLADE_Y0 / BLADE_LEN 与各 *_Y 常量，五金件按比例跟着收。

旋转只用单轴、且只取 ±45（八角柱）与 ±22.5（龙角上翘）——这样即便日后走
vanilla item model JSON 路线也是合法值，不必重做几何。

用法:
    python3 modelScript/generators/gen_bamboo_jian.py               # 双锏
    python3 modelScript/generators/gen_bamboo_jian.py --single      # 单根（导手持 item 模型用）
    python3 modelScript/generators/gen_bamboo_jian.py --preview-only
    bbmodel-render modelScript/models/BambooJian.bbmodel --three-view
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
PAIR_DX = 4.4  # 双锏各自中轴的 x 偏移（间距 8.8px）

# ── 纵向分段（y，柄尾 = 0）────────────────────────────────────────────────
POMMEL_Y = (0.00, 1.70)   # 瓜棱铜球
GRIP_Y = (1.70, 4.90)     # 木握把（收腰）
FERRULE_Y = (4.90, 5.30)  # 铜箍
HEAD_Y = (5.30, 7.00)     # 龙首吞口
BLADE_Y0 = 7.00           # 竹节锏身起点
BLADE_LEN = 15.40         # 锏身总长（九节，占全长 ~64%，加尖 ~71%）
TIP_LEN = 1.60            # 黑钢钝尖

NODES = 9                 # 竹节数
NODE_LEN0 = 1.90          # 第 0 节长（逐节递减 NODE_LEN_STEP）
NODE_LEN_STEP = 0.065
HW_ROOT = 0.90            # 锏身根部半宽（长:根径 ≈ 8.5:1，参考图是细长身）
HW_TIP = 0.46             # 第 8 节半宽
RING_BULGE = 0.15         # 竹节凸环比节身粗多少（凸太多读成螺纹钉）
RING_H = 0.34             # 凸环高

BONE_ORDER = ["blade", "head", "grip", "pommel"]
BONE_COLORS = {
    "blade": (168, 172, 180),
    "head": (176, 142, 72),
    "grip": (80, 59, 42),
    "pommel": (188, 154, 82),
}
# 贴图分区（同一张 64²，按材质划带；Packer 各自在带内打 UV）
MAT_ZONE = {
    "steel": (0, 0, RES, 24),
    "wood": (0, 24, RES, 36),
    "brass": (0, 36, RES, 58),
    "dark": (0, 58, RES, RES),
}


def segments():
    """九节竹节：返回 [(y0, y1, hw)]，逐节收细、逐节变短。"""
    lens = [NODE_LEN0 - i * NODE_LEN_STEP for i in range(NODES)]
    scale = BLADE_LEN / sum(lens)          # 归一到 BLADE_LEN，改节数不破总长
    lens = [ln * scale for ln in lens]
    out, y = [], BLADE_Y0
    for i, ln in enumerate(lens):
        hw = HW_ROOT + (HW_TIP - HW_ROOT) * (i / (NODES - 1))
        out.append((y, y + ln, hw))
        y += ln
    return out


def build_cubes(dx: float = 0.0, side: str = "r"):
    """返回 [(bone, material, name, from, to, rot_y)]；rot_y=45 的盒与同尺寸
    轴对齐盒叠加 = 八角柱（近似圆截面）。"""
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

    # ── pommel —— 黄铜瓜棱球（棱瓣靠贴图纵纹，几何走三层八角）──────
    octagon("pommel", "brass", "pommel_knob", 0.52, POMMEL_Y[0], POMMEL_Y[0] + 0.30)
    octagon("pommel", "brass", "pommel_bulb", 1.22, POMMEL_Y[0] + 0.30, 1.25)
    octagon("pommel", "brass", "pommel_neck", 0.76, 1.25, POMMEL_Y[1])

    # ── grip —— 深木握把，中段收腰（参考图握把是两头略粗的木柄）────
    octagon("grip", "wood", "grip_low", 0.80, GRIP_Y[0], 2.70)
    octagon("grip", "wood", "grip_mid", 0.70, 2.70, 3.95)
    octagon("grip", "wood", "grip_up", 0.82, 3.95, GRIP_Y[1])
    octagon("grip", "brass", "ferrule", 0.93, FERRULE_Y[0], FERRULE_Y[1])

    # ── head —— 黄铜龙首吞口：宽颅 + 侧角 + 眼 + 衔住锏身的口环 + 獠牙 ──
    octagon("head", "brass", "head_base", 0.96, HEAD_Y[0], 5.70)
    octagon("head", "brass", "head_skull", 1.30, 5.70, 6.55)
    octagon("head", "brass", "head_maw", 0.90, 6.55, HEAD_Y[1])
    # 张口：上颚前伸压住吻、下颚略缩——1.7px 高度里靠这条缝读出"衔"
    block("head", "brass", "jaw_upper", -0.70, 0.70, 6.22, 6.72, 0.95, 1.92)
    block("head", "brass", "jaw_lower", -0.60, 0.60, 5.62, 6.04, 0.95, 1.70)
    block("head", "dark", "maw_gap", -0.52, 0.52, 6.04, 6.22, 1.00, 1.62)
    for sx, tag in ((-1, "l"), (1, "r")):
        # 侧角：内段平出、外段上翘（rot_z 单轴 ±22.5，vanilla JSON 合法值）
        x_in, x_out = (1.05, 1.80) if sx > 0 else (-1.80, -1.05)
        block("head", "brass", f"horn_in_{tag}", x_in, x_out, 6.10, 6.60, -0.34, 0.34)
        x2_in, x2_out = (1.68, 2.46) if sx > 0 else (-2.46, -1.68)
        block("head", "brass", f"horn_out_{tag}", x2_in, x2_out, 6.24, 6.72, -0.28, 0.28,
              rot=(0.0, 0.0, 22.5 * sx))
        # 眼：吻侧上方的暗嵌（黑钢），放大到 MC 尺度还剩得下
        block("head", "dark", f"eye_{tag}", sx * 0.92 - 0.31, sx * 0.92 + 0.31, 6.16, 6.66, 0.72, 1.20)
        # 獠牙：口环两侧朝锏身方向的小尖
        block("head", "brass", f"fang_{tag}", sx * 0.56 - 0.19, sx * 0.56 + 0.19, 6.98, 7.46, -0.19, 0.19)

    # ── blade —— 九节竹节锏身：每节柱身 + 顶端凸环 ──────────────────
    for i, (y0, y1, hw) in enumerate(segments()):
        octagon("blade", "steel", f"seg_{i}", hw, y0, y1 - RING_H)
        octagon("blade", "steel", f"ring_{i}", hw + RING_BULGE, y1 - RING_H, y1)

    # ── blade —— 黑钢钝尖（参考图尖端发黑，两段收锥不收刃）───────────
    y_tip = BLADE_Y0 + BLADE_LEN
    octagon("blade", "steel", "tip_cone", 0.36, y_tip, y_tip + TIP_LEN * 0.5)
    octagon("blade", "dark", "tip_point", 0.25, y_tip + TIP_LEN * 0.5, y_tip + TIP_LEN)

    return [c for c in cubes if c is not None]


# ── 贴图 ──────────────────────────────────────────────────────────────────
def make_texture(res=RES, seed=73):
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 抛光钢 y[0,24)：细密纵向高光条（任何 UV 位置取到都读作圆柱反光）+ 磨痕
    smask = y < 24
    sheen = 0.5 + 0.5 * np.sin(x * 2.1)
    scol = np.array([158, 164, 174], float)[None, None, :] + (sheen[..., None] - 0.45) * 62
    scol += (rng.random((res, res, 1)) - 0.5) * 8
    for _ in range(10):  # 使用痕/暗蚀（末法：擦得亮但打过很多次）
        cx, cy = rng.integers(0, res), rng.integers(0, 24)
        ln = rng.integers(2, 6)
        for k in range(ln):
            scol[np.clip(cy + (k % 2), 0, 23), np.clip(cx + k, 0, res - 1)] *= 0.74
    scol = np.clip(scol, 78, 232)
    img[smask, :3] = scol[smask].astype(np.uint8)

    # 深木 y[24,36)：竖木纹 + 深浅年轮带
    wmask = (y >= 24) & (y < 36)
    grain = 0.5 + 0.5 * np.sin(x * 1.3 + np.sin(y * 0.5) * 0.9)
    wcol = np.array([80, 59, 42], float)[None, None, :] + (grain[..., None] - 0.5) * 30
    wcol += (rng.random((res, res, 1)) - 0.5) * 9
    wcol = np.clip(wcol, 34, 142)
    img[wmask, :3] = wcol[wmask].astype(np.uint8)

    # 黄铜 y[36,58)：纵向棱瓣明暗（喂给瓜棱球/龙首）+ 铜绿蚀点
    bmask = (y >= 36) & (y < 58)
    lobe = 0.5 + 0.5 * np.sin(x * 1.55)
    bcol = np.array([170, 138, 74], float)[None, None, :] + (lobe[..., None] - 0.5) * 54
    bcol += (rng.random((res, res, 1)) - 0.5) * 10
    for _ in range(8):  # 铜绿
        cx, cy = rng.integers(1, res - 1), rng.integers(37, 57)
        rr = ((x - cx) ** 2 + (y - cy) ** 2) < rng.integers(2, 7)
        bcol[rr] = bcol[rr] * 0.55 + np.array([104, 124, 86]) * 0.45
    bcol = np.clip(bcol, 62, 226)
    img[bmask, :3] = bcol[bmask].astype(np.uint8)

    # 黑钢 y[58,64)：锏尖与龙眼
    dmask = y >= 58
    dcol = np.array([40, 41, 48], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 16
    dcol += (0.5 + 0.5 * np.sin(x * 1.8))[..., None] * 12
    dcol = np.clip(dcol, 20, 92)
    img[dmask, :3] = dcol[dmask].astype(np.uint8)

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
    """去掉 _r/_l 后缀——左右两根锏共用同一套 UV，贴图压力不翻倍。"""
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
        # 旋转中心取该盒自身中轴（不是 bone pivot，否则 45° 会把盒甩离锏轴）
        cx = (frm[0] + to[0]) / 2
        cy = (frm[1] + to[1]) / 2
        cz = (frm[2] + to[2]) / 2
        elements.append({
            "name": name, "box_uv": False, "rescale": False, "locked": False,
            "render_order": "default", "allow_mirror_modeling": True, "type": "cube",
            "uuid": str(uuid.uuid4()),
            "from": [round(v, 3) for v in frm], "to": [round(v, 3) for v in to],
            "autouv": 0, "color": BONE_ORDER.index(bone),
            "origin": [round(cx, 3), round(cy, 3), round(cz, 3)],
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
            "name": "jian_right" if side == "r" else "jian_left",
            "origin": [dx, 0.0, 0.0], "color": 0, "uuid": str(uuid.uuid4()), "export": True,
            "mirror_uv": False, "isOpen": True, "locked": False, "visibility": True,
            "autouv": 0, "children": children,
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


# ── 示意预览（45° 盒按旋转后 AABB 画；真长相以 render_bbmodel.py 为准）────
def _aabb(frm, to, rot):
    """示意图用：只按 Y 旋转算 AABB（Z 轴的龙角上翘在示意图里忽略，真长相看渲染器）。"""
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
                        fill=lit(BONE_COLORS[bone], 1.0) + (255,), outline=(18, 16, 14, 255))
        return im

    tiles = [ortho(0, 1, "FRONT (X-Y) 双锏"), ortho(2, 1, "SIDE (Z-Y) 龙首侧"),
             ortho(0, 2, "TOP (X-Z) 八角截面")]
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
    d.text((gap, th + gap * 2), "TEXTURE 64x64 (x3) — steel / wood / brass / dark",
           fill=(200, 200, 200))
    d.text((gap * 2 + tex_big.width, th + gap * 2 + 14),
           "bones: " + "  ".join(BONE_ORDER), fill=(180, 180, 180))
    out.parent.mkdir(parents=True, exist_ok=True)  # out/ 不入库，干净 checkout 上不存在
    canvas.save(out)
    return out


def summarize(cubes, pair: bool):
    xs = [v for _b, _m, _n, f, t, r in cubes for v in (_aabb(f, t, r)[0][0], _aabb(f, t, r)[1][0])]
    ys = [v for _b, _m, _n, f, t, _r in cubes for v in (f[1], t[1])]
    zs = [v for _b, _m, _n, f, t, r in cubes for v in (_aabb(f, t, r)[0][2], _aabb(f, t, r)[1][2])]
    bb = (max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    print(f"  bbox  : {bb[0]:.1f}×{bb[1]:.1f}×{bb[2]:.1f}px = "
          f"{bb[0] / PX:.2f}W × {bb[1] / PX:.2f}H × {bb[2] / PX:.2f}D 格")
    print(f"  单锏长: {bb[1]:.1f}px（玩家模型 32px 的 {bb[1] / 32 * 100:.0f}%）"
          f"{'  ×2 并列' if pair else ''}")
    print(f"  竹节  : {NODES} 节，根径 {HW_ROOT * 2:.2f}px → 尖径 {HW_TIP * 2:.2f}px")
    print(f"  cubes : {len(cubes)}  ("
          + ", ".join(f"{b}:{sum(1 for c in cubes if c[0] == b)}" for b in BONE_ORDER) + ")")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--single", action="store_true", help="只生成一根（导手持 item 模型用）")
    ap.add_argument("--preview-only", action="store_true")
    args = ap.parse_args()

    pair = not args.single
    # 单根走独立文件名——别拿单根覆盖双锏源（手改过的更亏）
    out_bb = BBMODEL_OUT if pair else BBMODEL_OUT.with_name("BambooJianSingle.bbmodel")
    out_png = PREVIEW_OUT if pair else PREVIEW_OUT.with_name("bamboo_jian_single_preview.png")

    model, cubes, tex = build_bbmodel(pair=pair)
    print("竹节双锏 / bamboo_jian:" if pair else "竹节锏 / bamboo_jian (single):")
    summarize(cubes, pair)
    if not args.preview_only:
        out_bb.parent.mkdir(parents=True, exist_ok=True)
        out_bb.write_text(json.dumps(model, ensure_ascii=False, indent=1))
        print(f"  → bbmodel: {out_bb.relative_to(REPO)} ({out_bb.stat().st_size} B)")
    p = render_preview(cubes, tex, out=out_png)
    print(f"  → preview: {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
