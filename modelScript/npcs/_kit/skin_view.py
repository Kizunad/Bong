"""皮肤审阅图：把 64×64 皮肤穿到标准玩家模型上渲染（只离线预览，不是交付物）。

玩家盒子与 gear_kit.PLAYER_BOXES 同坐标（脚在 y=0，前面 -z，穿戴者的右手在 +x），带第二层外扩盒子，
第二层的透明像素按 alpha 剔除。装备直接读 modelScript/gear/<id>/*_tex.bbmodel 的贴图版几何叠上去。
"""

from __future__ import annotations

import json
import math
import subprocess
import tempfile
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from bbmodel_maker.render import render_bbmodel as R

from skin_kit import BOXES, INFLATE, OVERLAY_OF, Skin, face_rect  # noqa: E402

REPO = Path(__file__).resolve().parents[3]
sys_path_tools = REPO / "modelScript" / "tools"
import sys  # noqa: E402

sys.path.insert(0, str(sys_path_tools))
from cjk_font import load_font  # noqa: E402

BG = (119, 119, 119)

# part → 世界坐标盒（from, to）。穿戴者右手在 +x，所以右臂 / 右腿在 +x 一侧。
PLAYER_BOX = {
    "head": ((-4, 24, -4), (4, 32, 4)),
    "body": ((-4, 12, -2), (4, 24, 2)),
    "rarm": ((4, 12, -2), (8, 24, 2)),
    "larm": ((-8, 12, -2), (-4, 24, 2)),
    "rleg": ((0, 0, -2), (4, 12, 2)),
    "lleg": ((-4, 0, -2), (0, 12, 2)),
}


def _box_faces(part: str) -> dict:
    """box-uv 展开成每面 uv：东 = 穿戴者右侧，北 = 正面（-z）。"""
    ou, ov, sx, sy, sz = BOXES[part]
    return {
        "east": [ou, ov + sz, ou + sz, ov + sz + sy],
        "north": [ou + sz, ov + sz, ou + sz + sx, ov + sz + sy],
        "west": [ou + sz + sx, ov + sz, ou + 2 * sz + sx, ov + sz + sy],
        "south": [ou + 2 * sz + sx, ov + sz, ou + 2 * sz + 2 * sx, ov + sz + sy],
        "up": [ou + sz, ov, ou + sz + sx, ov + sz],
        "down": [ou + sz + sx, ov, ou + sz + 2 * sx, ov + sz],
    }


def player_tris(skin: Image.Image, tmp: Path) -> tuple:
    """用 render_bbmodel.load_bbmodel 读一份临时 bbmodel（贴图内嵌），拿到三角形和贴图数组。"""
    import base64
    import io

    els = []
    for part in BOXES:
        base = OVERLAY_OF.get(part, part)
        lo, hi = PLAYER_BOX[base]
        g = INFLATE.get(part, 0.0)
        els.append({
            "name": part, "type": "cube", "uuid": str(uuid.uuid4()),
            "from": [lo[0] - g, lo[1] - g, lo[2] - g], "to": [hi[0] + g, hi[1] + g, hi[2] + g],
            "origin": [0, 0, 0], "rotation": [0, 0, 0],
            "faces": {f: {"uv": uv, "texture": 0} for f, uv in _box_faces(part).items()},
        })
    buf = io.BytesIO()
    skin.save(buf, format="PNG")
    doc = {"name": "skin_player", "resolution": {"width": 64, "height": 64}, "elements": els,
           "textures": [{"name": "skin.png", "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()}]}
    tmp.write_text(json.dumps(doc), encoding="utf-8")
    return R.load_bbmodel(tmp)


GEAR_CACHE = Path(tempfile.gettempdir()) / "bong_gear_cache"


def gear_model_path(gear_id: str) -> Path:
    """装备贴图版 bbmodel：先找工作树里的，没有就从 origin/feat/gear-<id> 取出来放进临时缓存（只预览用）。"""
    local = REPO / "modelScript" / "gear" / gear_id
    found = sorted(local.glob("*_tex.bbmodel")) if local.exists() else []
    if found:
        return found[0]
    GEAR_CACHE.mkdir(exist_ok=True)
    cached = sorted(GEAR_CACHE.glob(f"{gear_id}__*_tex.bbmodel"))
    if cached:
        return cached[0]
    listing = subprocess.run(["git", "-C", str(REPO), "ls-tree", "-r", "--name-only", f"origin/feat/gear-{gear_id}",
                              f"modelScript/gear/{gear_id}"], capture_output=True, text=True, check=True).stdout.split()
    name = next(p for p in listing if p.endswith("_tex.bbmodel"))
    data = subprocess.run(["git", "-C", str(REPO), "show", f"origin/feat/gear-{gear_id}:{name}"],
                          capture_output=True, check=True).stdout
    out = GEAR_CACHE / f"{gear_id}__{Path(name).name}"
    out.write_bytes(data)
    return out


def render_models(models: list, yaw: float, pitch: float, size: int, focus, bg=BG) -> Image.Image:
    """models = [(tris, tex(RGBA float), depth_bias)]；MC 面亮度表着色，alpha<128 的像素不画。"""
    Rm = R._rotmat(pitch, 0) @ R._rotmat(yaw, 1)
    center = np.asarray(focus[0], float)
    scale = (size - 20) / float(focus[1])
    off = np.array([size, size], float) / 2
    img = np.zeros((size, size, 3), float)
    img[:] = bg
    zbuf = np.full((size, size), -1e9)
    for tris, tex, bias in models:
        th, tw = tex.shape[:2]
        for vs, uvs, n in tris:
            nv = Rm @ n
            if nv[2] <= 0.02:
                continue
            shade = R._mc_shade(nv)
            p = (Rm @ (vs - center).T).T
            xs = p[:, 0] * scale + off[0]
            ys = -p[:, 1] * scale + off[1]
            zs = p[:, 2] + bias
            x0, x1 = int(max(0, math.floor(xs.min()))), int(min(size - 1, math.ceil(xs.max())))
            y0, y1 = int(max(0, math.floor(ys.min()))), int(min(size - 1, math.ceil(ys.max())))
            if x0 > x1 or y0 > y1:
                continue
            ax, ay, bx, by, cx, cy = xs[0], ys[0], xs[1], ys[1], xs[2], ys[2]
            area = (bx - ax) * (cy - ay) - (cx - ax) * (by - ay)
            if abs(area) < 1e-6:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
            w0 = ((bx - ax) * (gy - ay) - (by - ay) * (gx - ax)) / area
            w1 = ((cx - bx) * (gy - by) - (cy - by) * (gx - bx)) / area
            w2 = 1 - w0 - w1
            inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
            if not inside.any():
                continue
            b0, b1, b2 = w1, w2, w0
            depth = b0 * zs[0] + b1 * zs[1] + b2 * zs[2]
            u = b0 * uvs[0, 0] + b1 * uvs[1, 0] + b2 * uvs[2, 0]
            v = b0 * uvs[0, 1] + b1 * uvs[1, 1] + b2 * uvs[2, 1]
            ui = np.clip(u.astype(int), 0, tw - 1)
            vi = np.clip(v.astype(int), 0, th - 1)
            texel = tex[vi, ui]
            sub_z = zbuf[y0:y1 + 1, x0:x1 + 1]
            win = inside & (depth > sub_z) & (texel[..., 3] >= 128)
            if not win.any():
                continue
            sub_img = img[y0:y1 + 1, x0:x1 + 1]
            sub_img[win] = texel[..., :3][win] * shade
            sub_z[win] = depth[win]
    return Image.fromarray(np.clip(img, 0, 255).astype(np.uint8))


VIEWS = (
    ("FRONT 正面", 180.0, 0.0),
    ("RIGHT 右侧（穿戴者右）", -90.0, 0.0),
    ("BACK 背面", 0.0, 0.0),
    ("3/4 右前斜", 212.0, 10.0),
)
FOCUS = ((0.0, 16.0, 0.0), 33.0)


def checker(w: int, h: int, cell: int = 8) -> Image.Image:
    im = Image.new("RGB", (w, h), (70, 70, 74))
    d = ImageDraw.Draw(im)
    for y in range(0, h, cell):
        for x in range(0, w, cell):
            if (x // cell + y // cell) % 2:
                d.rectangle([x, y, x + cell - 1, y + cell - 1], fill=(88, 88, 92))
    return im


def make_sheet(npc_id: str, title: str, skin: Image.Image, gear_ids: list[str], palette: list[tuple[str, str]],
               out: Path, notes: str = "", cell: int = 340) -> None:
    font_h, font_l, font_s = load_font(18), load_font(14), load_font(12)
    tmp = Path(tempfile.gettempdir()) / f"skin_player_{npc_id}.bbmodel"
    pt, ptex, _, _ = player_tris(skin, tmp)
    skin_models = [(pt, ptex, 0.0)]

    gear_models = []
    for gid in gear_ids:
        try:
            gt, gtex, _, _ = R.load_bbmodel(gear_model_path(gid))
            gear_models.append((gt, gtex, 0.35))
        except Exception as exc:  # 缺装备就如实标注，不静默跳过
            print(f"! 装备 {gid} 载入失败：{exc}")

    left_w = 64 * 8 + 24
    top = 56
    rows = 2
    canvas = Image.new("RGB", (left_w + 4 * (cell + 6) + 6, top + rows * (cell + 34) + 8), (28, 29, 33))
    d = ImageDraw.Draw(canvas)
    d.text((8, 8), f"{title} · {npc_id} · 64×64 classic 皮肤（含第二层）", fill=(235, 235, 235), font=font_h)

    big = checker(512, 512)
    big_skin = skin.resize((512, 512), Image.Resampling.NEAREST)
    big.paste(big_skin, (0, 0), big_skin)
    # 描出各盒子展开范围，方便对位
    bd = ImageDraw.Draw(big)
    for part in BOXES:
        ou, ov, sx, sy, sz = BOXES[part]
        w, h = 2 * sz + 2 * sx, sz + sy
        bd.rectangle([ou * 8, ov * 8, (ou + w) * 8 - 1, (ov + h) * 8 - 1], outline=(255, 255, 255, 60))
    canvas.paste(big, (8, top))
    d.text((8, top + 518), "皮肤 PNG ×8 放大（棋盘格 = 透明；细框 = 各盒子展开范围）", fill=(200, 200, 205), font=font_s)
    # 色板
    py = top + 540
    for k, (name, hexs) in enumerate(palette):
        px_ = 8 + (k % 4) * 128
        py_ = py + (k // 4) * 22
        d.rectangle([px_, py_, px_ + 16, py_ + 16], fill=hexs)
        d.text((px_ + 22, py_), f"{name} {hexs}", fill=(215, 215, 220), font=font_s)

    def view(models, yaw, pitch):
        return render_models(models, yaw, pitch, cell, FOCUS)

    x0 = left_w
    for idx, (label, yaw, pitch) in enumerate(VIEWS):
        x = x0 + idx * (cell + 6)
        d.rectangle([x, top - 6, x + cell - 1, top + 14], fill=(22, 23, 26))
        d.text((x + 6, top - 5), label, fill=(235, 235, 235), font=font_l)
        canvas.paste(view(skin_models, yaw, pitch), (x, top + 18))
    y2 = top + cell + 34
    d.text((x0, y2 - 22), "皮肤 + 复用装备（已做的贴图版 bbmodel，装备沿用各自贴图；专属新装备尚未做，不在内）：" + " / ".join(gear_ids),
           fill=(215, 205, 180), font=font_l)
    for idx, (label, yaw, pitch) in enumerate((VIEWS[0], VIEWS[2], VIEWS[1], VIEWS[3])):
        x = x0 + idx * (cell + 6)
        d.rectangle([x, y2 - 2, x + cell - 1, y2 + 16], fill=(22, 23, 26))
        d.text((x + 6, y2 - 1), label, fill=(235, 235, 235), font=font_l)
        canvas.paste(view(skin_models + gear_models, yaw, pitch), (x, y2 + 18))
    if notes:
        d.text((8, canvas.height - 24), notes, fill=(190, 190, 195), font=font_s)
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    print(f"✓ skin_check.png → {out} ({canvas.width}×{canvas.height})")
