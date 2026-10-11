#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b13: emitter (经脉发射器 / 喷嘴)

风格：A 有机型 (活体血肉、半高镂空红肉骨架底座、细高三级收窄喷嘴、节间外凸肉褶环、外翻口沿、口中真元光芯、四索对角斜拉筋索)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md「第 2 批」表格 b13 那行
- 爆炸参考图：model-review/img/meridian_factory/exploded/x13_emitter.png
- 调度审第 1 次指示（2026-10-11 15:2x）：
  1. 底座：b04 那种半高框 (y 0–5)，四面镂空只留红肉棱条和 8 个骨节角 (能透过空框看到里面)，框内底面中心 common_parts.outlet_8x6 朝下当入口；
  2. 喷嘴 (y 5–16，细高)：截面逐级收窄 8×8 (y 5–9) → 6×6 (y 9–13) → 4×4 (y 13–15)，顶上一圈外翻口沿 6×6 × 1px 高 (y 15–16)，口内 2×2 #f6dcc4 真元内光；
     每级之间加一圈 1px 外凸的 #5a1a1a 肉褶环；
  3. 4 条 #c07868 筋丝从口沿斜着垂到底座四角的骨节上 (不是贴在喷嘴面上的竖条)；
  4. 只改 b13，重出图，写自查，保持 b13-waiting 停下。
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

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
if str(REPO) not in sys.path:
    sys.path.insert(0, str(REPO))

MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/emitter")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x13_emitter.png")
SINGLE_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b13_emitter.png")

from modelScript.generators.meridian_factory.common_parts import (
    PALETTE,
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    bone_post,
    outlet_8x6,
)


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_base_frame() -> List[dict]:
    """底座半高镂空框 (y 0–5)：
    - 四面完全镂空，能看穿底座内部；
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


def part_02_nozzle_tiers() -> List[dict]:
    """细高肉质喷嘴 3 级 (y 5–16，细高挺拔)：
    - 级 1: 截面 8x8 (x/z in [-4, 4]), y in [5.0, 9.0]；
    - 级 1-2 肉褶环: 截面 10x10, y in [8.5, 9.5], 外凸 1px #5a1a1a；
    - 级 2: 截面 6x6 (x/z in [-3, 3]), y in [9.0, 13.0]；
    - 级 2-3 肉褶环: 截面 8x8, y in [12.5, 13.5], 外凸 1px #5a1a1a；
    - 级 3: 截面 4x4 (x/z in [-2, 2]), y in [13.0, 15.0]；
    - 顶上一圈外翻口沿 6x6 x 1px 高 (y in [15.0, 16.0])，中心 2x2 #f6dcc4 真元内光。
    """
    cubes = []

    # 1. 级 1 (截面 8x8, y in [5.0, 9.0])
    cubes.append({"name": "nozzle_tier1", "from": [-4.0, 5.0, -4.0], "to": [4.0, 9.0, 4.0], "group": "nozzle", "material": "flesh_main"})

    # 2. 级 1-2 之间外凸 1px #5a1a1a 肉褶环 (截面 10x10, y in [8.5, 9.5])
    cubes.append({"name": "fold_ring_1_2", "from": [-5.0, 8.5, -5.0], "to": [5.0, 9.5, 5.0], "group": "nozzle", "material": "flesh_dark"})

    # 3. 级 2 (截面 6x6, y in [9.0, 13.0])
    cubes.append({"name": "nozzle_tier2", "from": [-3.0, 9.0, -3.0], "to": [3.0, 13.0, 3.0], "group": "nozzle", "material": "flesh_main"})

    # 4. 级 2-3 之间外凸 1px #5a1a1a 肉褶环 (截面 8x8, y in [12.5, 13.5])
    cubes.append({"name": "fold_ring_2_3", "from": [-4.0, 12.5, -4.0], "to": [4.0, 13.5, 4.0], "group": "nozzle", "material": "flesh_dark"})

    # 5. 级 3 (截面 4x4, y in [13.0, 15.0])
    cubes.append({"name": "nozzle_tier3", "from": [-2.0, 13.0, -2.0], "to": [2.0, 15.0, 2.0], "group": "nozzle", "material": "flesh_main"})

    # 6. 顶上一圈外翻口沿 6x6 x 1px 高 (y in [15.0, 16.0])
    # 四周外翻肉唇 (#b05050 flesh_lit)
    cubes.append({"name": "lip_n", "from": [-3.0, 15.0, -3.0], "to": [ 3.0, 16.0, -1.0], "group": "nozzle_lip", "material": "flesh_lit"})
    cubes.append({"name": "lip_s", "from": [-3.0, 15.0,  1.0], "to": [ 3.0, 16.0,  3.0], "group": "nozzle_lip", "material": "flesh_lit"})
    cubes.append({"name": "lip_w", "from": [-3.0, 15.0, -1.0], "to": [-1.0, 16.0,  1.0], "group": "nozzle_lip", "material": "flesh_lit"})
    cubes.append({"name": "lip_e", "from": [ 1.0, 15.0, -1.0], "to": [ 3.0, 16.0,  1.0], "group": "nozzle_lip", "material": "flesh_lit"})

    # 7. 口内 2x2 真元内光 (#f6dcc4 qi_glow, y in [14.0, 15.8])
    cubes.append({"name": "orifice_qi_core", "from": [-1.0, 14.0, -1.0], "to": [1.0, 15.8, 1.0], "group": "nozzle_lip", "material": "qi_glow"})

    return cubes


def part_03_diagonal_guy_wires() -> List[dict]:
    """4 条 #c07868 筋丝从口沿斜着垂到底座四角骨节上：
    - NW, NE, SW, SE 四个对角空间完全悬空的斜拉索；
    - 从口沿外角 (y=15.5) 阶梯斜拉延伸到底座角骨节 (y=5.0)；
    - 绝非贴在喷嘴面上的竖条，在 3/4 与侧视呈现强烈的有机斜拉索张力感。
    """
    cubes = []

    strands = [
        ("nw", -1.0, -1.0),
        ("ne",  1.0, -1.0),
        ("sw", -1.0,  1.0),
        ("se",  1.0,  1.0),
    ]

    for sname, sx, sz in strands:
        # 段 1 (近口沿): y in [12.8, 15.2], x/z 偏移 ~2.6
        x1_0, x1_1 = (min(sx * 2.2, sx * 3.1), max(sx * 2.2, sx * 3.1))
        z1_0, z1_1 = (min(sz * 2.2, sz * 3.1), max(sz * 2.2, sz * 3.1))
        cubes.append({"name": f"guy_{sname}_seg1", "from": [x1_0, 12.8, z1_0], "to": [x1_1, 15.2, z1_1], "group": "guy_wires", "material": "tendon_fiber"})

        # 段 2 (中上部): y in [10.2, 12.8], x/z 偏移 ~3.7
        x2_0, x2_1 = (min(sx * 3.15, sx * 4.2), max(sx * 3.15, sx * 4.2))
        z2_0, z2_1 = (min(sz * 3.15, sz * 4.2), max(sz * 3.15, sz * 4.2))
        cubes.append({"name": f"guy_{sname}_seg2", "from": [x2_0, 10.2, z2_0], "to": [x2_1, 12.8, z2_1], "group": "guy_wires", "material": "tendon_fiber"})

        # 段 3 (中下部): y in [7.4, 10.2], x/z 偏移 ~4.8
        x3_0, x3_1 = (min(sx * 4.25, sx * 5.4), max(sx * 4.25, sx * 5.4))
        z3_0, z3_1 = (min(sz * 4.25, sz * 5.4), max(sz * 4.25, sz * 5.4))
        cubes.append({"name": f"guy_{sname}_seg3", "from": [x3_0, 7.4, z3_0], "to": [x3_1, 10.2, z3_1], "group": "guy_wires", "material": "tendon_fiber"})

        # 段 4 (下部锚入底座骨柱): y in [4.8, 7.4], x/z 偏移 ~6.0
        x4_0, x4_1 = (min(sx * 5.45, sx * 6.6), max(sx * 5.45, sx * 6.6))
        z4_0, z4_1 = (min(sz * 5.45, sz * 6.6), max(sz * 5.45, sz * 6.6))
        cubes.append({"name": f"guy_{sname}_seg4", "from": [x4_0, 4.8, z4_0], "to": [x4_1, 7.4, z4_1], "group": "guy_wires", "material": "tendon_fiber"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_base_frame()
        + part_02_nozzle_tiers()
        + part_03_diagonal_guy_wires()
    )


# =============================================================================
# 导出与渲染
# =============================================================================

def generate_bbmodel(out_path: Path) -> Path:
    """导出 emitter 的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for c in cubes:
        f = c["from"]
        t = c["to"]
        mat = c.get("material", "flesh_main")
        uv = MAT_UV.get(mat, [0, 16, 16, 32])
        faces = {}
        for side in ["north", "south", "east", "west", "up", "down"]:
            faces[side] = {"uv": uv, "texture": 0}

        elem = {
            "name": c["name"],
            "box_uv": False,
            "from": f,
            "to": t,
            "faces": faces,
            "uuid": str(uuid.uuid4()),
        }
        elements.append(elem)

    name = "emitter"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "emitter")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": name,
                "folder": "block",
                "namespace": "bong",
                "id": 0,
                "source": tex_base64,
            }
        ],
    }

    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    try:
        rel = out_path.relative_to(REPO)
    except ValueError:
        rel = out_path
    print(f"✓ {name} bbmodel 写入成功: {rel}")
    return out_path


def render_views(model_p: Path):
    """输出包含（3/4 + 侧视 + 俯视 + 正视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 渲染各视角：
    im_iso, _   = render(model_p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_side, _  = render(model_p, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_top, _   = render(model_p, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_front, _ = render(model_p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)

    # 拼装 render.png (2x2 网格拼版：3/4 视、侧视、俯视、正视)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("3/4 ISOMETRIC (Open Base y:0-5, Slender Nozzle y:5-16, Guys)", im_iso, 10, 10),
        ("SIDE VIEW (Hollow Open Base Frame & Slender Nozzle Tiers)", im_side, cell_w + 20, 10),
        ("TOP VIEW (Flaring Lip 6x6, Orifice Qi-Glow 2x2 & 4 Guys)", im_top, 10, cell_h + 20),
        ("FRONT VIEW (Diagonal Guy-Wires Anchored to Base Corners)", im_front, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 440, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (3/4 + 侧视 + 俯视 + 正视 2x2 拼版) 已输出: {render_path}")

    # 拼装 check.png (左爆炸参考图，右当前模型 3/4 等轴视与侧视并排等高对标)
    ref_image_to_use = EXPLODED_REF if EXPLODED_REF.exists() else SINGLE_REF
    if ref_image_to_use.exists():
        ref_im = Image.open(ref_image_to_use).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_iso = im_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_side = im_side.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), f"EXPLODED REFERENCE ({ref_image_to_use.name})", fill=(210, 200, 180))

        # 贴右渲染图 (左 3/4 视，右侧视)
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_side, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: SIDE View | Slender Nozzle y:5-16)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 爆炸图并排对照图 (含 3/4 视角与侧视对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_emitter.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] emitter ({len(cubes)} 个立方体) 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 5.0, -4.0],
        "to":   [ 4.0, 9.0,  4.0],
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] emitter 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: emitter 注入共面冲突未被拦截!")

    print("✓ gen_emitter.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b13 emitter 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p = generate_bbmodel(out_path=MODEL_DIR / "emitter.bbmodel")
    render_views(p)


if __name__ == "__main__":
    main()
