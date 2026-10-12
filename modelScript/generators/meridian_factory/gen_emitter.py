#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b13: emitter (经脉发射器 / 喷嘴)

风格：A 有机型 (活体血肉、半高镂空红肉骨架底座、细高三级收窄宝塔喷嘴、深色肉褶收拢环、外翻口沿、口中真元光芯、四索 1x1 阶梯斜拉筋络)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md「第 2 批」表格 b13 那行
- 爆炸参考图：model-review/img/meridian_factory/exploded/x13_emitter.png
- 调度审第 2 次指标（2026-10-11 16:0x）：
  1. 底座镂空框保留：b04 那种半高框 (y 0–5)，四面完全镂空，只留红肉棱条和 4 根角骨节柱 (能看到里面)，框内底面中心 common_parts.outlet_8x6 朝下当入口；
  2. 喷嘴细高重做 (x/z 居中，以 0–16 计)：
     - 芯柱 4×4 (x 6–10 对应 [-2, 2]) 从 y5 一直到 y14，贯穿全部裙层；
     - 裙 1：y5–7 为 10×10 (x 3–13 对应 [-5, 5])，y7–8 收到 8×8 (对应 [-4, 4])；
     - 裙 2：y8–10 为 8×8 (对应 [-4, 4])，y10–11 收到 6×6 (对应 [-3, 3])；
     - 裙 3：y11–13 为 6×6 (对应 [-3, 3])，y13–14 收到 4×4 (对应 [-2, 2])；
     - 每层裙之间露出 1 格芯柱，芯柱用 #5a1a1a 做肉褶色；
     - 顶口 y14–16 是 6×6 外翻环 (中间 4×4 镂空，不要封板)，环口颜色用 #f6dcc4 当亮芯；
  3. 四根筋络：从顶口下沿四角出发，用 1×1 方块斜着阶梯往下落到底座四个骨角，不要竖直立柱；
  4. 几何以外不动，输出 render.png 与 check.png。
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


def part_02_slender_nozzle_tiers() -> List[dict]:
    """细高肉质喷嘴 3 级 (y 5–16，细高宝塔收窄造型)：
    - 芯柱 4×4 从 y5 到 y14 贯穿全部裙层；
    - 裙 1：y5–7 为 10×10 (x in [-5, 5])，y7–8 收到 8×8 (x in [-4, 4])；
    - 裙 2：y8–10 为 8×8 (x in [-4, 4])，y10–11 收到 6×6 (x in [-3, 3])；
    - 裙 3：y11–13 为 6×6 (x in [-3, 3])，y13–14 收到 4×4 (x in [-2, 2])；
    - 每层裙之间露出 1 格芯柱，芯柱用 #5a1a1a 做肉褶色；
    - 顶口 y14–16 是 6×6 外翻环 (中间 4×4 镂空，不要封板)，环口颜色用 #f6dcc4 当亮芯。
    """
    cubes = []

    # 1. 裙 1 主体: y in [5.0, 7.0], 截面 10x10 (x/z in [-5.0, 5.0]), #8a2a2a (flesh_main)
    cubes.append({"name": "skirt1_main", "from": [-5.0, 5.0, -5.0], "to": [5.0, 7.0, 5.0], "group": "nozzle_skirt1", "material": "flesh_main"})

    # 2. 裙 1 收束 / 露出芯柱肉褶: y in [7.0, 8.0], 截面 8x8 (x/z in [-4.0, 4.0]), #5a1a1a (flesh_dark)
    cubes.append({"name": "skirt1_fold", "from": [-4.0, 7.0, -4.0], "to": [4.0, 8.0, 4.0], "group": "nozzle_skirt1", "material": "flesh_dark"})

    # 3. 裙 2 主体: y in [8.0, 10.0], 截面 8x8 (x/z in [-4.0, 4.0]), #8a2a2a (flesh_main)
    cubes.append({"name": "skirt2_main", "from": [-4.0, 8.0, -4.0], "to": [4.0, 10.0, 4.0], "group": "nozzle_skirt2", "material": "flesh_main"})

    # 4. 裙 2 收束 / 露出芯柱肉褶: y in [10.0, 11.0], 截面 6x6 (x/z in [-3.0, 3.0]), #5a1a1a (flesh_dark)
    cubes.append({"name": "skirt2_fold", "from": [-3.0, 10.0, -3.0], "to": [3.0, 11.0, 3.0], "group": "nozzle_skirt2", "material": "flesh_dark"})

    # 5. 裙 3 主体: y in [11.0, 13.0], 截面 6x6 (x/z in [-3.0, 3.0]), #8a2a2a (flesh_main)
    cubes.append({"name": "skirt3_main", "from": [-3.0, 11.0, -3.0], "to": [3.0, 13.0, 3.0], "group": "nozzle_skirt3", "material": "flesh_main"})

    # 6. 裙 3 收到 4x4 / 露出 1 格 4x4 芯柱: y in [13.0, 14.0], 截面 4x4 (x/z in [-2.0, 2.0]), #5a1a1a (flesh_dark)
    cubes.append({"name": "core_exposed", "from": [-2.0, 13.0, -2.0], "to": [2.0, 14.0, 2.0], "group": "nozzle_core", "material": "flesh_dark"})

    # 7. 顶口 y14–16 是 6x6 外翻环 (中间 4x4 镂空，不要封板)
    # 外翻环四边唇壁 (外 6x6: x/z in [-3, 3], 内 4x4 镂空: x/z in [-2, 2], 高 y in [14.0, 16.0])
    # 北壁唇
    cubes.append({"name": "lip_n", "from": [-3.0, 14.0, -3.0], "to": [ 3.0, 16.0, -2.0], "group": "nozzle_lip", "material": "flesh_lit"})
    # 南壁唇
    cubes.append({"name": "lip_s", "from": [-3.0, 14.0,  2.0], "to": [ 3.0, 16.0,  3.0], "group": "nozzle_lip", "material": "flesh_lit"})
    # 西壁唇
    cubes.append({"name": "lip_w", "from": [-3.0, 14.0, -2.0], "to": [-2.0, 16.0,  2.0], "group": "nozzle_lip", "material": "flesh_lit"})
    # 东壁唇
    cubes.append({"name": "lip_e", "from": [ 2.0, 14.0, -2.0], "to": [ 3.0, 16.0,  2.0], "group": "nozzle_lip", "material": "flesh_lit"})

    # 8. 环口中心深处 2x2 真元亮芯 (#f6dcc4 qi_glow, y in [13.2, 14.5], x/z in [-1.0, 1.0])
    cubes.append({"name": "orifice_glow_core", "from": [-1.0, 13.2, -1.0], "to": [1.0, 14.5, 1.0], "group": "nozzle_lip", "material": "qi_glow"})

    return cubes


def part_03_stepped_guy_wires() -> List[dict]:
    """四根筋络：从顶口下沿四角出发，用 1×1 方块斜着阶梯往下落到底座四个骨角，不要竖直立柱。
    - 四个角 (NW, NE, SW, SE) 各一条；
    - 5 级阶梯步进，每步截面 1x1，高 1.8px；
    - 从顶口下沿四角 (y=14.0, x/z=±3.0) 沿对角线阶梯下落到底座角骨柱 (y=5.0, x/z=±6.6)；
    - 形成生动强烈的四角阶梯斜拉索，彻底避免竖直立柱感。
    """
    cubes = []

    strands = [
        ("nw", -1.0, -1.0),
        ("ne",  1.0, -1.0),
        ("sw", -1.0,  1.0),
        ("se",  1.0,  1.0),
    ]

    # 5 级 1x1 阶梯台阶 (y0, y1, 内外径 d0, d1: 每步截面正好 1x1)
    steps = [
        (12.2, 14.0, 2.5, 3.5),
        (10.4, 12.2, 3.4, 4.4),
        ( 8.6, 10.4, 4.3, 5.3),
        ( 6.8,  8.6, 5.2, 6.2),
        ( 5.0,  6.8, 6.1, 7.1),
    ]

    for sname, sx, sz in strands:
        for idx, (y0, y1, d0, d1) in enumerate(steps, start=1):
            x_min = min(sx * d0, sx * d1)
            x_max = max(sx * d0, sx * d1)
            z_min = min(sz * d0, sz * d1)
            z_max = max(sz * d0, sz * d1)
            cubes.append({
                "name": f"guy_{sname}_step{idx}",
                "from": [x_min, y0, z_min],
                "to":   [x_max, y1, z_max],
                "group": "guy_wires",
                "material": "tendon_fiber",
            })

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_base_frame()
        + part_02_slender_nozzle_tiers()
        + part_03_stepped_guy_wires()
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
        ("3/4 ISOMETRIC (Slender Nozzle y:5-16, 1x1 Stepped Guy-Wires)", im_iso, 10, 10),
        ("SIDE VIEW (Open Base y:0-5, Nozzle Tiers 10->8->6->4->6)", im_side, cell_w + 20, 10),
        ("TOP VIEW (Flaring Lip 6x6, Hollow 4x4 & Qi-Glow Core)", im_top, 10, cell_h + 20),
        ("FRONT VIEW (Diagonal 1x1 Stepped Guy-Wires to Base Corners)", im_front, cell_w + 20, cell_h + 20),
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

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: SIDE View | Slender Nozzle y:5-16, Stepped Guys)", fill=(180, 220, 210))

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
        "from": [-5.0, 5.0, -5.0],
        "to":   [ 5.0, 7.0,  5.0],
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
