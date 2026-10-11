#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o07: spleen (脾，土)

规格（meridian_factory.md 第 2 批表）：外包 2 × 2 × 2（32×32×32 px），原点在底面中心。
单个弯豆形侧躺（不是一对）；主色 #6a2a3a、#8a2a2a 斑；表面 #b08a40 细脉（土）；
内凹口（脾门）处一根入口管 + 一个 8×6 输出口。

接口件（筋管、肉箍、骨环、输出口）一律 import common_parts，本文件不再写第二份。

坐标：x 向右、y 向上、z 向 +Z（正面）。豆形轮廓在 XY 平面，凹口朝 -X，z 方向是厚度。

逐行宽度表（每行 2px 高，从下往上，y 起点 3.5，所以行边界都落在 x.5，
避开 common_parts 筋管和套管 y=1/2/8/9/16/22 的面，防共面）：

行  y       x 左..右   宽   厚(z 总厚)   说明
0   3.5..5.5    -10..2     12   16           下角（胃侧尖端）
1   5.5..7.5  -12..6     18   19.6          下角向左勾出
2   7..9    -12..9     21   22
3   9..11   -11..11    22   24
4   11..13  -9..13     22   24 (11.8 错开)
5   13..15  -6..14     20   24           脾门凹口开始
6   15..17  -3..15     18   24 (11.8)
7   17..19  -2..15     17   24           凹口最深
8   19..21  -3..15     18   24 (11.8)
9   21..23  -6..15     21   24
10  23..25  -9..14     23   24 (11.8)
11  25..27  -11..13    24   22
12  27..29  -12..10    22   20           上角向左勾出
13  29..31  -10..6     16   16           上端收圆

厚度相邻行刻意差 0.2px（12.0 / 11.8）：同方向同坐标的面会 z-fighting，
共面门禁会拦；这 0.2px 同时给肉体一点起伏，不是笔误。

部件拆分（各自独立 part_*，check.png 里单独渲染）：
- part_body        豆形肉体 + 脾门亮肉唇
- part_earth_veins 正面 #b08a40 土色细脉 + #8a2a2a 斑块
- part_inlet       脾门入口管（沿 -X，筋管 + 肉箍 + 肉箍顶面骨环，同 b01）
- part_outlet      下角 +Z 方向 8×6 输出口（筋管 + outlet_8x6）
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common_parts import (  # noqa: E402
    MAT_UV as COMMON_MAT_UV,
    PALETTE as COMMON_PALETTE,
    bone_ring_port,
    flesh_collar,
    outlet_8x6,
    sinew_tube,
)

REPO = Path(__file__).resolve().parents[3]
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "spleen.bbmodel"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")
REVIEW_DIR = REVIEW_ROOT / "spleen"
EXPLODED_REF = REVIEW_ROOT / "exploded" / "x07_spleen.png"
SINGLE_REF = REVIEW_ROOT / "refs" / "o07_spleen.png"

# 在公共调色板之上只加脾自己的两色（配色表：脾主色 #6a2a3a、土 #b08a40）
PALETTE = dict(COMMON_PALETTE)
PALETTE["spleen_main"] = (106, 42, 58, 255)    # #6a2a3a 脾主色
PALETTE["earth_vein"] = (176, 138, 64, 255)    # #b08a40 土色细脉

MAT_UV = dict(COMMON_MAT_UV)
MAT_UV["spleen_main"] = [16, 32, 32, 48]
MAT_UV["earth_vein"] = [32, 32, 48, 48]

RES = 64

# 豆形逐行表：(y0, x_left, x_right, 半厚)
BEAN_ROWS: List[Tuple[float, float, float, float]] = [
    (3.5,  -10.0,  2.0,  8.0),
    (5.5,  -12.0,  6.0,  9.8),
    (7.5,  -12.0,  9.0, 11.0),
    (9.5,  -11.0, 11.0, 12.0),
    (11.5,  -9.0, 13.0, 11.9),
    (13.5,  -6.0, 14.0, 12.0),
    (15.5,  -3.0, 15.0, 11.9),
    (17.5,  -2.0, 15.0, 12.0),
    (19.5,  -3.0, 15.0, 11.9),
    (21.5,  -6.0, 15.0, 12.0),
    (23.5,  -9.0, 14.0, 11.9),
    (25.5, -11.0, 13.0, 11.0),
    (27.5, -12.0, 10.0, 10.0),
    (29.5, -10.0,  6.0,  8.0),
]
ROW_H = 2.0
HILUM_ROWS = range(5, 10)          # 凹口范围（行号）

# 正面土色细脉：(行号, x0, x1)，从脾门向右扇形分叉，逐行相接
EARTH_VEIN_CELLS: List[Tuple[int, float, float]] = [
    # 主干：凹口最深处向右
    (7, 0.0, 6.0),
    (7, 8.0, 13.0),
    # 上行分支 A
    (8, 4.0, 6.0), (9, 6.0, 8.0), (10, 8.0, 10.0), (11, 9.0, 11.0), (12, 8.0, 10.0),
    # 上行分支 A2（靠近凹口的小支）
    (8, 1.0, 3.0), (9, 1.0, 3.0), (10, 0.0, 2.0), (11, -1.0, 1.0),
    # 下行分支 B
    (6, 5.0, 7.0), (5, 7.0, 9.0), (4, 9.0, 11.0), (3, 10.0, 12.0),
    # 下行分支 B2
    (6, 2.0, 4.0), (5, 2.0, 4.0), (4, 1.0, 3.0), (3, 0.0, 2.0),
]
VEIN_LIFT = 0.5

# 正面 #8a2a2a 斑块：(行号, x0, x1)，避开脉
FLESH_PATCH_CELLS: List[Tuple[int, float, float]] = [
    (1, -7.5, -3.5), (2, 2.0, 6.0), (3, 6.0, 10.0), (4, 3.0, 7.0),
    (5, 9.0, 12.0), (6, 9.0, 13.0), (8, 8.0, 12.0), (9, 10.0, 14.0),
    (10, -6.0, -2.0), (11, 3.0, 7.0), (12, -6.0, -1.0), (13, -4.0, 0.0),
]
PATCH_LIFT = 0.3

# 入口管：脾门凹口最深处向 -X 伸出，管中心 y=19（行 8 中心），筋管 y 16..22
INLET_Y_OFFSET = 14.0
INLET_X0, INLET_X1 = -16.0, -2.0
# 输出口：下角 +Z 方向，筋管 y 2..8（统一截面），套管中心 x=-4
OUTLET_CENTER_X = -4.0
OUTLET_CENTER_Y = 5.0
OUTLET_TUBE_Z0, OUTLET_TUBE_Z1 = 4.0, 10.0
OUTLET_LENGTH = 6.0


# =============================================================================
# 部件
# =============================================================================

def part_body() -> List[dict]:
    """豆形肉体：逐行堆叠，行内 #6a2a3a；凹口边缘一圈 #b05050 亮肉唇。"""
    cubes = []
    for i, (y0, xl, xr, hz) in enumerate(BEAN_ROWS):
        cubes.append({
            "name": f"bean_row_{i:02d}",
            "from": [xl, y0, -hz],
            "to":   [xr, y0 + ROW_H, hz],
            "group": "bean_body",
            "material": "spleen_main",
        })
    # 脾门亮肉唇：贴在凹口各行的左缘正面，宽 1.5px
    for i in HILUM_ROWS:
        y0, xl, _, hz = BEAN_ROWS[i]
        cubes.append({
            "name": f"hilum_lip_{i:02d}",
            "from": [xl, y0, hz],
            "to":   [xl + 1.5, y0 + ROW_H, hz + 0.3],
            "group": "bean_body",
            "material": "flesh_lit",
        })
    return cubes


def part_earth_veins() -> List[dict]:
    """正面土色细脉（#b08a40）+ #8a2a2a 斑块；都浮起在正面，不抠洞。"""
    cubes = []
    for n, (row, x0, x1) in enumerate(EARTH_VEIN_CELLS):
        y0, _, _, hz = BEAN_ROWS[row]
        cubes.append({
            "name": f"earth_vein_{n:02d}",
            "from": [x0, y0, hz],
            "to":   [x1, y0 + ROW_H, hz + VEIN_LIFT],
            "group": "earth_veins",
            "material": "earth_vein",
        })
    for n, (row, x0, x1) in enumerate(FLESH_PATCH_CELLS):
        y0, _, _, hz = BEAN_ROWS[row]
        cubes.append({
            "name": f"flesh_patch_{n:02d}",
            "from": [x0, y0, hz],
            "to":   [x1, y0 + ROW_H, hz + PATCH_LIFT],
            "group": "earth_veins",
            "material": "flesh_main",
        })
    return cubes


def part_inlet() -> List[dict]:
    """脾门入口管：沿 -X 的暖粉筋管 + 端口肉箍 + 肉箍顶面小骨环（b01 同款）。"""
    cubes = []
    cubes += sinew_tube(INLET_X0, INLET_X1, axis="x", offset_y=INLET_Y_OFFSET,
                        group="inlet", pfx="inlet_tube")
    collar_center = INLET_X0 + 1.5
    cubes += flesh_collar(collar_center, length=3.0, axis="x", offset_y=INLET_Y_OFFSET,
                          group="inlet", pfx="inlet_collar")
    cubes += bone_ring_port(center_x=collar_center, center_y=9.0 + INLET_Y_OFFSET, center_z=0.0,
                            face="up", group="inlet", pfx="inlet_ring")
    return cubes


def part_outlet() -> List[dict]:
    """下角输出口：一小节 8×6 筋管埋进下角，末端接 outlet_8x6 肉质套管，朝 +Z。"""
    cubes = []
    cubes += sinew_tube(OUTLET_TUBE_Z0, OUTLET_TUBE_Z1, axis="z", offset_x=OUTLET_CENTER_X,
                        group="outlet", pfx="outlet_tube")
    cubes += outlet_8x6(center_x=OUTLET_CENTER_X, center_y=OUTLET_CENTER_Y, center_z=OUTLET_TUBE_Z1,
                        facing="+z", length=OUTLET_LENGTH, group="outlet", pfx="outlet_port")
    return cubes


PARTS = {
    "body": part_body,
    "veins": part_earth_veins,
    "inlet": part_inlet,
    "outlet": part_outlet,
}


def all_cubes() -> List[dict]:
    return part_body() + part_earth_veins() + part_inlet() + part_outlet()


# =============================================================================
# 门禁
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """同方向同坐标且投影相交的面 = z-fighting，直接报错。"""
    faces: Dict[Tuple[str, float], List[tuple]] = {}
    for c in cubes:
        f, t = c["from"], c["to"]
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (min(f[1], t[1]), min(f[2], t[2]), max(f[1], t[1]), max(f[2], t[2]))
            elif axis == 1:
                rect = (min(f[0], t[0]), min(f[2], t[2]), max(f[0], t[0]), max(f[2], t[2]))
            else:
                rect = (min(f[0], t[0]), min(f[1], t[1]), max(f[0], t[0]), max(f[1], t[1]))
            faces.setdefault((side, round(val, 4)), []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                du = min(r1[2], r2[2]) - max(r1[0], r2[0])
                dv = min(r1[3], r2[3]) - max(r1[1], r2[1])
                if du > 0.001 and dv > 0.001:
                    conflicts.append(f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠 {du:.3f}x{dv:.3f}")
    if conflicts:
        raise AssertionError("\n".join(conflicts))


def _bounds(cubes: List[dict]) -> Tuple[List[float], List[float]]:
    lo = [min(c["from"][a] for c in cubes) for a in range(3)]
    hi = [max(c["to"][a] for c in cubes) for a in range(3)]
    return lo, hi


def _assert_inside_envelope(cubes: List[dict]):
    """外包必须在 2×2×2 方块（x/z ±16，y 0..32）内。"""
    lo, hi = _bounds(cubes)
    if lo[0] < -16 or hi[0] > 16 or lo[1] < 0 or hi[1] > 32 or lo[2] < -16 or hi[2] > 16:
        raise AssertionError(f"越出 32×32×32 外包: lo={lo} hi={hi}")


# =============================================================================
# 贴图与 bbmodel
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """脾主色带 #8a2a2a 斑 + #5a1a1a 暗点；其余沿用 common_parts 的噪声做法。"""
    rng = np.random.RandomState(20261011)
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base = PALETTE[mat_name]
        w, h = u1 - u0, v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base
        amp = 8 if mat_name in ("bone_main", "bone_dark", "qi_glow") else 10
        noise = rng.randint(-amp, amp, size=(h, w))
        for ch in range(3):
            tile[:, :, ch] = np.clip(tile[:, :, ch].astype(int) + noise, 0, 255)

        if mat_name == "spleen_main":
            for y in range(h):
                for x in range(w):
                    if (x * 7 + y * 11) % 13 in (0, 1):
                        tile[y, x, :3] = PALETTE["flesh_main"][:3]
                    elif (x * 5 + y * 3) % 11 == 0:
                        tile[y, x, :3] = PALETTE["flesh_dark"][:3]

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))
    return im


def build_bbmodel_doc(cubes: List[dict], name: str = "Spleen") -> dict:
    buf = io.BytesIO()
    build_texture().save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    groups: Dict[str, List[str]] = {}
    for c in cubes:
        elem_uuid = str(uuid.uuid4())
        groups.setdefault(c.get("group", "spleen"), []).append(elem_uuid)
        u0, v0, u1, v1 = MAT_UV.get(c.get("material", "flesh_main"), [0, 0, 16, 16])
        uv = [float(u0), float(v0), float(u1), float(v1)]
        faces = {side: {"uv": uv, "texture": 0} for side in ("north", "east", "south", "west", "up", "down")}
        elements.append({
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in c["from"]],
            "to": [round(float(v), 4) for v in c["to"]],
            "autouv": 0,
            "color": 0,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        })

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "color": 0, "uuid": str(uuid.uuid4()),
         "isOpen": True, "children": ids}
        for g, ids in groups.items()
    ]
    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "model_identifier": name.lower(),
        "visible_box": [2, 2, 2],
        "geometry_name": name.lower(),
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "name": name.lower(), "folder": "meridian_factory", "namespace": "bong", "id": "0",
            "particle": False, "render_mode": "default", "visible": True, "mode": "bitmap",
            "saved": True, "uuid": str(uuid.uuid4()), "source": tex_b64,
            "width": RES, "height": RES,
        }],
    }


def write_bbmodel(cubes: List[dict], out_path: Path, name: str = "Spleen") -> Path:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(build_bbmodel_doc(cubes, name), indent=2, ensure_ascii=False), encoding="utf-8")
    return out_path


def generate_bbmodel() -> Path:
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    _assert_inside_envelope(cubes)
    write_bbmodel(cubes, BBMODEL_OUT)
    print(f"✓ 脾 bbmodel 写入: {BBMODEL_OUT.relative_to(REPO)} ({len(cubes)} 个立方体)")
    return BBMODEL_OUT


# =============================================================================
# 审阅图：render.png / check.png
# =============================================================================

BG = (119, 119, 119)


def _label(img: Image.Image, text: str) -> Image.Image:
    out = Image.new("RGB", (img.width, img.height + 22), (24, 25, 28))
    out.paste(img, (0, 22))
    ImageDraw.Draw(out).text((6, 5), text, fill=(230, 230, 230))
    return out


def _hstack(imgs: List[Image.Image], gap: int = 8) -> Image.Image:
    h = max(i.height for i in imgs)
    w = sum(i.width for i in imgs) + gap * (len(imgs) - 1)
    canvas = Image.new("RGB", (w, h), (28, 29, 33))
    x = 0
    for i in imgs:
        canvas.paste(i, (x, 0))
        x += i.width + gap
    return canvas


def render_views(bbmodel_path: Path = BBMODEL_OUT):
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)

    # render.png：正视 + 3/4 + 侧视
    front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=520, bg=BG)
    iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=520, bg=BG)
    side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=520, bg=BG)
    strip = _hstack([_label(front, "FRONT (+Z)"), _label(iso, "3/4"), _label(side, "SIDE (+X)")])
    strip.save(REVIEW_DIR / "render.png")
    print(f"✓ render.png: {REVIEW_DIR / 'render.png'}")

    # check.png：左爆炸图，右 = 四个部件拆开渲染 + 拼好渲染
    scratch = REVIEW_DIR / "_parts"
    scratch.mkdir(exist_ok=True)
    part_tiles = []
    for key, fn in PARTS.items():
        p = write_bbmodel(fn(), scratch / f"{key}.bbmodel", name=f"Spleen_{key}")
        img, _ = render(p, yaw=-35.0, pitch=25.0, size=340, bg=BG)
        part_tiles.append(_label(img, f"part: {key}"))
    whole, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=520, bg=BG)

    target_h = 760
    ref = Image.open(EXPLODED_REF).convert("RGB")
    ref = ref.resize((int(ref.width * target_h / ref.height), target_h), Image.Resampling.LANCZOS)

    grid = Image.new("RGB", (340 * 2 + 8, (340 + 22) * 2 + 8), (28, 29, 33))
    for n, tile in enumerate(part_tiles):
        grid.paste(tile, ((n % 2) * 348, (n // 2) * 370))
    whole = _label(whole, "ASSEMBLED 3/4")
    whole = whole.resize((int(whole.width * target_h / whole.height), target_h), Image.Resampling.LANCZOS)
    grid = grid.resize((int(grid.width * target_h / grid.height), target_h), Image.Resampling.LANCZOS)

    check = _hstack([_label(ref, "REFERENCE x07_spleen (exploded)"), grid, whole])
    check.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png: {REVIEW_DIR / 'check.png'}")


# =============================================================================
# 自证
# =============================================================================

def self_test():
    """门禁差分自证：正常立方体集通过，注入的共面 / 越界缺陷必须被拦截。"""
    print("运行 gen_spleen.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    _assert_inside_envelope(cubes)
    print("  [OK] 正常立方体集：无共面冲突，且在 32×32×32 外包内")

    dup = dict(cubes[0], name="inject_coplanar")
    try:
        _assert_no_coplanar_faces(cubes + [dup])
    except AssertionError as e:
        print(f"  [OK] 拦截注入共面: {e.args[0].splitlines()[0]}")
    else:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")

    out_of_box = {"name": "inject_out", "from": [0, 0, 0], "to": [40, 4, 4], "material": "flesh_main"}
    try:
        _assert_inside_envelope(cubes + [out_of_box])
    except AssertionError as e:
        print(f"  [OK] 拦截注入越界: {e.args[0]}")
    else:
        raise RuntimeError("门禁失效: 注入越界未被拦截!")
    print("✓ gen_spleen.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o07 spleen 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁差分自证")
    args = parser.parse_args()

    self_test()
    if args.self_test:
        return
    render_views(generate_bbmodel())


if __name__ == "__main__":
    main()
