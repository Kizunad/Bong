#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b13: emitter v2 (经脉发射器 / 可蠕动的括约肌收缩口，含 open / half / closed 三变体)

风格：A 有机型。用户审阅意见原话：「做成可蠕动的收缩口」，原先的细高喷嘴方案作废。
依据：
- model-review/meridian_factory.md「第 2 批」b13 那行 + 派单（2026-10-11）
- 爆炸参考图：model-review/img/meridian_factory/exploded/x13_emitter_v2.png（本机 CPA gpt-image-2.5-sunburst 出的）
- 保留 pi-a 那版已对的镂空骨框底座（part_01，y 0–5，原样不动）。

外包 16×16×16（x/z ±8，y 0..16），原点在底面中心，正面朝 +Z。

新设计（单位 px，1 block = 16px）：
1. 底座 part_01：y 0–5 半高镂空框（4 根角骨节柱 + 红肉棱 + 框内底面 outlet_8x6 朝下当入口）。
2. 肉质穹顶 part_02（y 5–约 12）：1×1px 一根的柱子排成 12×12 的圆角方形（柱心半径 ≤ 6，底宽 12），
   顶面高度按柱心半径取：r≤3.5 为 11、≤4.5 为 10.5、≤5.5 为 9、≤6.5 为 7，所以宽 12 往上收到 8（上平台直径 ~8）。
   顶面按角度叠 8 道放射肉褶：cos(8θ) > 0.3 的柱是褶脊（#b05050，高 +amp），< -0.3 是褶间沟（#5a1a1a，低 -amp），
   其余为肉体 #8a2a2a。褶振幅 amp：open 0.5 / half 1.0 / closed 1.5（越收越深）。
3. 收缩口 part_03：穹顶正中把 |x|,|z| < a 的柱子去掉，open a=2（4×4）、half a=1（2×2）、closed a=0（无孔，
   中央 2×2 柱压低成一个 #5a1a1a 的星形接缝，8 道褶尖全挤到中心）。孔里一圈 0.5px 厚 #5a1a1a 内衬，
   孔底（y=7）一块 #f6dcc4 亮芯（靠对比出发光，不加发光层），亮芯边长 = 孔宽 - 1。
4. 外圈肉箍 part_04：穹顶底（y 5 起）一圈圆环（和穹顶同一张柱网，接在穹顶圆外面），外缘 open ±7.0 / half ±7.5 / closed ±8.0，
   高 open 2 / half 2.5 / closed 3。越收越外鼓，参考 b11 阀门括约肌环的做法。
轮换 open → half → closed → half → open 就是收缩蠕动。

输出：emitter_open / emitter_half / emitter_closed.bbmodel（emitter.bbmodel = open）；
render.png（三态 3/4 + 俯视）、parts.png（open 态四个部件拆开）、check.png（左爆炸参考，右三态并排）。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/emitter")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x13_emitter_v2.png")
TMP_DIR = Path("/tmp/claude-1000/-home-serverkizuna-Code-Bong--agent-worktrees-model-b/e8024e15-89b9-4c94-bc09-9db5a4565345/scratchpad/emitter_parts")

from modelScript.generators.meridian_factory.common_parts import (
    PALETTE,
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    bone_post,
    outlet_8x6,
)

VARIANTS = ["open", "half", "closed"]

DOME_Y0 = 5.0
FOLDS = 10  # 放射肉褶道数（派单要求 8~10）
FOLD_THRESHOLD = 0.3  # cos(nθ) 超过 ±threshold 才算褶脊 / 褶间沟
DOME_RADIUS = 6.0  # 柱心半径上限，穹顶底宽 12
DOME_HALF = 6  # 柱网范围 ±6
STEP = 0.5  # 柱网步长（0.5px 一根，放射褶才不至于被 1px 网格压成十字）
# (柱心半径上限, 穹顶顶面高度)，自内向外
DOME_PROFILE: List[Tuple[float, float]] = [(4.0, 12.0), (4.5, 11.5), (5.0, 10.5), (5.4, 9.5), (5.8, 8.5), (6.0, 7.5)]
FOLD_AMPLITUDE = {"open": 0.5, "half": 1.0, "closed": 1.5}
APERTURE_HALF = {"open": 2, "half": 1, "closed": 0}  # 收缩口半宽 a（口宽 2a）
SEAM_DROP = 1.5  # closed 态中央 2×2 压低量
APERTURE_FLOOR_Y = 7.0  # 孔底（亮芯顶面在 y=7.5）
LINER_THICK = 0.5
COLLAR_OUTER = {"open": 7.1, "half": 7.6, "closed": 8.1}  # 最远角半径上限，实际外缘取到 0.5px 网格：7.0 / 7.5 / 8.0
COLLAR_HEIGHT = {"open": 1.5, "half": 2.0, "closed": 2.5}


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_base_frame() -> List[dict]:
    """底座半高镂空框 (y 0–5)：pi-a 那版已放行的部分，原样保留。
    - 四面完全镂空，能看穿底座内部空腔；
    - 4 根角骨节柱 (截面 2x2，底在 y=0、顶在 y=5，复用 common_parts.bone_post，中段带 #b8a888 节纹暗面)；
    - 底面 4 条水平红肉棱 (y in [0.0, 1.5]) 与顶面 4 条水平红肉棱 (y in [3.5, 5.0])；
    - 肉棱外表面附微浮 #5a1a1a 暗纹；
    - 框内底面中心用 common_parts.outlet_8x6 (facing='-y') 朝下当入口。
    """
    cubes = []

    # 1. 四根角骨节柱 (截面 2x2, y in [0.0, 5.0]，位于四外角，中段附暗面节带)
    corners = [
        ("nw", -6.5, -6.5),
        ("ne",  6.5, -6.5),
        ("sw", -6.5,  6.5),
        ("se",  6.5,  6.5),
    ]
    for cname, cx, cz in corners:
        cubes.extend(bone_post(center_x=cx, center_z=cz, y_bot=0.0, height=5.0, group="base_frame", pfx=f"post_{cname}"))

    # 2. 底面 4 条水平红肉棱 (截面 1.5x1.5, y in [0.0, 1.5], 连接四角骨柱)
    cubes.append({"name": "base_bot_rib_n", "from": [-5.5, 0.0, -7.5], "to": [ 5.5, 1.5, -6.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_bot_rib_s", "from": [-5.5, 0.0,  6.0], "to": [ 5.5, 1.5,  7.5], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_bot_rib_w", "from": [-7.5, 0.0, -5.5], "to": [-6.0, 1.5,  5.5], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_bot_rib_e", "from": [ 6.0, 0.0, -5.5], "to": [ 7.5, 1.5,  5.5], "group": "base_frame", "material": "flesh_main"})

    # 3. 顶面 4 条水平红肉棱 (截面 1.5x1.5, y in [3.5, 5.0], 连接四角骨柱)
    cubes.append({"name": "base_top_rib_n", "from": [-5.5, 3.5, -7.5], "to": [ 5.5, 5.0, -6.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_top_rib_s", "from": [-5.5, 3.5,  6.0], "to": [ 5.5, 5.0,  7.5], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_top_rib_w", "from": [-7.5, 3.5, -5.5], "to": [-6.0, 5.0,  5.5], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_top_rib_e", "from": [ 6.0, 3.5, -5.5], "to": [ 7.5, 5.0,  5.5], "group": "base_frame", "material": "flesh_main"})

    # 4. 肉棱外侧暗纹条 (#5a1a1a flesh_dark，微浮 0.05px)
    cubes.append({"name": "base_stripe_n", "from": [-2.5, 3.8, -7.55], "to": [2.5, 4.7, -7.45], "group": "base_frame", "material": "flesh_dark"})
    cubes.append({"name": "base_stripe_s", "from": [-2.5, 3.8,  7.45], "to": [2.5, 4.7,  7.55], "group": "base_frame", "material": "flesh_dark"})
    cubes.append({"name": "base_stripe_w", "from": [-7.55, 3.8, -2.5], "to": [-7.45, 4.7, 2.5], "group": "base_frame", "material": "flesh_dark"})
    cubes.append({"name": "base_stripe_e", "from": [ 7.45, 3.8, -2.5], "to": [ 7.55, 4.7, 2.5], "group": "base_frame", "material": "flesh_dark"})

    # 5. 框内底面中心入口 (common_parts.outlet_8x6，facing='-y' 朝下)
    cubes.extend(outlet_8x6(center_x=0.0, center_y=1.0, center_z=0.0, facing="-y", length=2.0, pfx="base_inlet"))

    return cubes


def _dome_base_height(r: float) -> float:
    """穹顶顶面基础高度：按柱心半径查 DOME_PROFILE。"""
    for limit, height in DOME_PROFILE:
        if r <= limit:
            return height
    raise ValueError(f"柱心半径 {r} 超出穹顶")


def _dome_columns() -> List[Tuple[float, float, float, float]]:
    """穹顶柱网：[(柱左缘 x, 柱左缘 z, 柱心半径, 柱心角度)]，±DOME_HALF 内按 STEP 切、柱心半径 ≤ DOME_RADIUS 的。"""
    cols = []
    n = int(2 * DOME_HALF / STEP)
    for i in range(n):
        for j in range(n):
            x, z = -DOME_HALF + i * STEP, -DOME_HALF + j * STEP
            cx, cz = x + STEP / 2, z + STEP / 2
            r = math.hypot(cx, cz)
            if r <= DOME_RADIUS:
                cols.append((x, z, r, math.atan2(cz, cx)))
    return cols


def _fold_kind(theta: float, folds: int) -> str:
    """该角度落在褶脊 / 褶间沟 / 肉体哪一类。"""
    f = math.cos(folds * theta)
    if f > FOLD_THRESHOLD:
        return "ridge"
    if f < -FOLD_THRESHOLD:
        return "groove"
    return "body"


def part_02_dome(variant: str = "open", folds: int = FOLDS) -> List[dict]:
    """肉质穹顶 (y 5–约 12)：1×1 柱网 + 放射肉褶。收缩口范围内的柱子不建（part_03 负责孔内）。"""
    amp = FOLD_AMPLITUDE[variant]
    a = APERTURE_HALF[variant]
    cubes = []
    for x, z, r, theta in _dome_columns():
        cx, cz = x + STEP / 2, z + STEP / 2
        if a and abs(cx) < a and abs(cz) < a:
            continue
        kind = _fold_kind(theta, folds)
        top = _dome_base_height(r)
        material = "flesh_main"
        if kind == "ridge":
            top, material = top + amp, "flesh_lit"
        elif kind == "groove":
            top, material = top - amp, "flesh_dark"
        if not a and abs(cx) < 1 and abs(cz) < 1:
            top, material = _dome_base_height(r) - SEAM_DROP, "flesh_dark"  # closed 态中央星形接缝
        cubes.append({
            "name": f"dome_x{round(x / STEP)}_z{round(z / STEP)}",
            "from": [x, DOME_Y0, z],
            "to": [x + STEP, top, z + STEP],
            "group": "dome",
            "material": material,
        })
    return cubes


def part_03_aperture(variant: str = "open") -> List[dict]:
    """收缩口内部：孔底填块 + 0.5px #5a1a1a 内衬 + 孔底亮芯。closed 无孔，返回空。"""
    a = float(APERTURE_HALF[variant])
    if not a:
        return []
    liner_top = _dome_base_height(0.0) - FOLD_AMPLITUDE[variant]  # 比孔沿最低的褶间沟还低一点，不顶出孔沿
    inner = a - LINER_THICK
    g = "aperture"
    return [
        {"name": "aperture_floor", "from": [-a, DOME_Y0, -a], "to": [a, APERTURE_FLOOR_Y, a], "group": g, "material": "flesh_dark"},
        {"name": "aperture_liner_n", "from": [-a, APERTURE_FLOOR_Y, -a], "to": [a, liner_top, -inner], "group": g, "material": "flesh_dark"},
        {"name": "aperture_liner_s", "from": [-a, APERTURE_FLOOR_Y, inner], "to": [a, liner_top, a], "group": g, "material": "flesh_dark"},
        {"name": "aperture_liner_w", "from": [-a, APERTURE_FLOOR_Y, -inner], "to": [-inner, liner_top, inner], "group": g, "material": "flesh_dark"},
        {"name": "aperture_liner_e", "from": [inner, APERTURE_FLOOR_Y, -inner], "to": [a, liner_top, inner], "group": g, "material": "flesh_dark"},
        {"name": "aperture_glow_core", "from": [-inner, APERTURE_FLOOR_Y, -inner], "to": [inner, APERTURE_FLOOR_Y + 0.5, inner], "group": g, "material": "qi_glow"},
    ]


def part_04_collar(variant: str = "open") -> List[dict]:
    """外圈肉箍：穹顶底（y 5 起）一圈圆环，和穹顶同一张 STEP 柱网，柱心半径 > DOME_RADIUS 的柱子接在穹顶外面；
    柱子最远角到中心的距离 ≤ COLLAR_OUTER，所以外缘不会超过该值，外径随开度外鼓。"""
    outer = COLLAR_OUTER[variant]
    top = DOME_Y0 + COLLAR_HEIGHT[variant]
    n = int(2 * math.ceil(outer) / STEP)
    origin = -math.ceil(outer)
    cubes = []
    for i in range(n):
        for j in range(n):
            x, z = origin + i * STEP, origin + j * STEP
            cx, cz = x + STEP / 2, z + STEP / 2
            far = math.hypot(abs(cx) + STEP / 2, abs(cz) + STEP / 2)
            if math.hypot(cx, cz) > DOME_RADIUS and far <= outer:
                cubes.append({
                    "name": f"collar_x{round(x / STEP)}_z{round(z / STEP)}",
                    "from": [x, DOME_Y0, z],
                    "to": [x + STEP, top, z + STEP],
                    "group": "collar",
                    "material": "flesh_main",
                })
    return cubes


def part_builders(variant: str):
    """[(部件名, 构造函数)]，check / parts 图和 all_cubes 共用。"""
    return [
        ("base_frame", part_01_base_frame),
        ("dome", lambda: part_02_dome(variant)),
        ("aperture", lambda: part_03_aperture(variant)),
        ("collar", lambda: part_04_collar(variant)),
    ]


def all_cubes(variant: str = "open") -> List[dict]:
    """汇总指定变体的所有部件立方体。"""
    cubes: List[dict] = []
    for _, build in part_builders(variant):
        cubes += build()
    return cubes


# =============================================================================
# 导出与渲染
# =============================================================================

def generate_bbmodel(variant: str, out_path: Path, cubes: List[dict] | None = None) -> Path:
    """导出指定变体的 bbmodel 文件（cubes 不传就是整个变体）。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(variant) if cubes is None else cubes
    _assert_no_coplanar_faces(cubes)

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    groups_map: Dict[str, List[str]] = {}
    for c in cubes:
        uv = MAT_UV.get(c.get("material", "flesh_main"), [0, 16, 16, 32])
        uid = str(uuid.uuid4())
        elements.append({
            "name": c["name"],
            "box_uv": False,
            "from": c["from"],
            "to": c["to"],
            "faces": {side: {"uv": uv, "texture": 0} for side in ("north", "south", "east", "west", "up", "down")},
            "uuid": uid,
        })
        groups_map.setdefault(c.get("group", "emitter"), []).append(uid)

    name = f"emitter_{variant}"
    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in groups_map.items()],
        "textures": [{"name": name, "folder": "block", "namespace": "bong", "id": 0, "source": tex_base64}],
    }
    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    print(f"✓ {name} bbmodel 写入成功: {out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path}")
    return out_path


def _label(canvas: Image.Image, text: str, xy: Tuple[int, int], width: int) -> None:
    draw = ImageDraw.Draw(canvas)
    draw.rectangle([xy[0], xy[1], xy[0] + width, xy[1] + 22], fill=(24, 25, 28))
    draw.text((xy[0] + 6, xy[1] + 5), text, fill=(235, 235, 235))


def render_views(paths: Dict[str, Path]):
    """出 render.png（三态 × 3/4 + 俯视）、parts.png（open 态部件拆开）、check.png（左参考，右三态并排）。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    size = 440
    iso = {v: render(paths[v], yaw=-35.0, pitch=30.0, size=size, bg=bg)[0] for v in VARIANTS}
    top = {v: render(paths[v], yaw=0.0, pitch=89.9, size=size, bg=bg)[0] for v in VARIANTS}
    side = {v: render(paths[v], yaw=0.0, pitch=0.0, size=size, bg=bg)[0] for v in VARIANTS}

    # render.png：三列三态，三行 3/4 / 俯视 / 正视
    canvas = Image.new("RGB", (size * 3 + 40, size * 3 + 3 * 34 + 10), (35, 36, 40))
    rows = (("3/4", iso), ("TOP", top), ("FRONT", side))
    for ri, (row_name, imgs) in enumerate(rows):
        for ci, v in enumerate(VARIANTS):
            x, y = 10 + ci * (size + 10), 10 + ri * (size + 34)
            canvas.paste(imgs[v], (x, y + 24))
            _label(canvas, f"emitter_{v} ({row_name})", (x, y), 240)
    canvas.save(REVIEW_DIR / "render.png")
    print(f"✓ render.png 已输出: {REVIEW_DIR / 'render.png'}")

    # parts.png：open 态四个部件各自单独渲染
    TMP_DIR.mkdir(parents=True, exist_ok=True)
    tiles = []
    for part_name, build in part_builders("open"):
        p = generate_bbmodel("open", TMP_DIR / f"{part_name}.bbmodel", cubes=build())
        tiles.append((part_name, render(p, yaw=-35.0, pitch=30.0, size=340, bg=bg)[0]))
    parts = Image.new("RGB", (340 * len(tiles) + 10 * (len(tiles) + 1), 340 + 44), (35, 36, 40))
    for i, (part_name, im) in enumerate(tiles):
        x = 10 + i * 350
        parts.paste(im, (x, 34))
        _label(parts, part_name, (x, 10), 160)
    parts.save(REVIEW_DIR / "parts.png")
    print(f"✓ parts.png 已输出: {REVIEW_DIR / 'parts.png'}")

    # check.png：左爆炸参考，右三态并排（上排 3/4，下排俯视）
    ref = Image.open(EXPLODED_REF).convert("RGB")
    target_h = 2 * 330 + 34 + 24
    ref = ref.resize((int(ref.width * target_h / ref.height), target_h), Image.Resampling.LANCZOS)
    cell = 330
    check = Image.new("RGB", (ref.width + cell * 3 + 40, target_h + 44), (28, 29, 33))
    check.paste(ref, (10, 34))
    _label(check, f"EXPLODED REFERENCE ({EXPLODED_REF.name})", (10, 10), 340)
    rx = ref.width + 20
    for ci, v in enumerate(VARIANTS):
        x = rx + ci * (cell + 5)
        check.paste(iso[v].resize((cell, cell), Image.Resampling.LANCZOS), (x, 34))
        check.paste(top[v].resize((cell, cell), Image.Resampling.LANCZOS), (x, 34 + cell + 10))
        _label(check, f"emitter_{v}  (3/4 above, TOP below)", (x, 10), 280)
    check.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png 已输出: {REVIEW_DIR / 'check.png'}")


# =============================================================================
# 门禁
# =============================================================================

def _ridge_clusters(variant: str, folds: int) -> int:
    """数穹顶上褶脊柱（#b05050）在柱心半径 3..5.5 的圆环里按角度聚成几簇，簇间隔 > 17° 算分开。"""
    angles = []
    for cube in part_02_dome(variant, folds):
        if cube["material"] != "flesh_lit":
            continue
        cx = (cube["from"][0] + cube["to"][0]) / 2
        cz = (cube["from"][2] + cube["to"][2]) / 2
        if 3.0 <= math.hypot(cx, cz) <= 5.5:
            angles.append(math.degrees(math.atan2(cz, cx)) % 360)
    angles.sort()
    gaps = [(angles[(i + 1) % len(angles)] - angles[i]) % 360 for i in range(len(angles))]
    return sum(1 for gap in gaps if gap > 17)


def _hole_area(variant: str) -> float:
    """从顶往下看收缩口的开口面积（px²）：穹顶圆内没有被任何柱子占的柱位数 × 柱底面积。"""
    covered = {(c["from"][0], c["from"][2]) for c in part_02_dome(variant)}
    missing = sum(1 for x, z, _, _ in _dome_columns() if (x, z) not in covered)
    return missing * STEP * STEP


def self_test():
    """门禁差分自证：共面 / 外包 / 10 道肉褶 / 三态开度与肉箍单调，各自先注入缺陷确认拦得住。"""
    print("运行 gen_emitter.py 差分自证...")

    for v in VARIANTS:
        cubes = all_cubes(v)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] emitter_{v} ({len(cubes)} 个立方体) 无共面冲突")
        try:
            _assert_no_coplanar_faces(cubes + [dict(cubes[0], name="inject_dup")])
        except AssertionError:
            pass
        else:
            raise RuntimeError(f"门禁失效: emitter_{v} 注入共面冲突未被拦截!")

        # y 下限 -1：pi-a 底座框内入口 outlet_8x6 (center_y=1, length=2) 的肉套往下探出地面 1px，底座原样保留
        lo = [min(c["from"][i] for c in cubes) for i in range(3)]
        hi = [max(c["to"][i] for c in cubes) for i in range(3)]
        assert lo[0] >= -8 and hi[0] <= 8 and lo[2] >= -8 and hi[2] <= 8 and lo[1] >= -1 and hi[1] <= 16, f"{v} 外包超出 16×16×16: lo={lo} hi={hi}"
        print(f"  [OK] emitter_{v} 外包 x[{lo[0]},{hi[0]}] y[{lo[1]},{hi[1]}] z[{lo[2]},{hi[2]}] ⊂ 16³")

    for v in VARIANTS:
        n = _ridge_clusters(v, FOLDS)
        assert n == FOLDS, f"emitter_{v} 应有 {FOLDS} 道褶脊，数出 {n}"
    assert _ridge_clusters("open", 8) != FOLDS, "褶数门禁失效: 注入 8 道褶没被识别出来"
    print(f"  [OK] 三态各 {FOLDS} 道放射褶脊；注入 8 道褶能识别为不符")

    areas = [_hole_area(v) for v in VARIANTS]
    assert areas == [16.0, 4.0, 0.0], f"三态开口面积应为 4×4=16 / 2×2=4 / 0，实际 {areas}"
    extents = [max(c["to"][0] for c in part_04_collar(v)) for v in VARIANTS]
    heights = [max(c["to"][1] for c in part_04_collar(v)) for v in VARIANTS]
    assert extents == sorted(set(extents)), f"肉箍外缘应随 open→half→closed 严格加大，实际 {extents}"
    assert heights == sorted(set(heights)), f"肉箍高度应随 open→half→closed 严格加大，实际 {heights}"
    assert all(e <= COLLAR_OUTER[v] for e, v in zip(extents, VARIANTS)), "肉箍不应超出各态外缘上限"
    print(f"  [OK] 开口面积 {areas}（4×4 / 2×2 / 闭合），肉箍外缘 {extents}、高 {heights} 严格递增")

    print("✓ gen_emitter.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b13 emitter v2 (括约肌收缩口) 生成器")
    parser.add_argument("--self-test", action="store_true", help="只运行门禁差分自证")
    args = parser.parse_args()

    self_test()
    if args.self_test:
        return

    paths = {v: generate_bbmodel(v, MODEL_DIR / f"emitter_{v}.bbmodel") for v in VARIANTS}
    generate_bbmodel("open", MODEL_DIR / "emitter.bbmodel")
    render_views(paths)


if __name__ == "__main__":
    main()
