#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b13: emitter (经脉发射器 / 喷嘴)

风格：A 有机型 (活体血肉、半高红肉骨架底座、向上三级收窄喷嘴、口中真元光芯、四面斜拉筋索)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md「第 2 批」表格 b13 那行
- 爆炸参考图：model-review/img/meridian_factory/exploded/x13_emitter.png

调度指标落地：
1. 底座：b04 同款红肉棱 + 骨节角的半高框 (y 0–6)；
   - 4 根角骨节柱 (截面 2x2，底在 y=0、顶在 y=6，复用 common_parts.bone_post，带 #b8a888 暗纹节带)；
   - 底面 4 条水平红肉棱 (y in [0, 2]) 与顶面 4 条水平红肉棱 (y in [4, 6])，材质 #8a2a2a (flesh_main)，附 #5a1a1a 暗纹；
   - 底面中心用 common_parts.outlet_8x6 (facing='-y') 做入口；
2. 上面向上收窄的肉质喷嘴 3 级 (截面 12×12 → 9×9 → 6×6，到 y 14)：
   - 级 1: 截面 12x12 (x/z in [-6, 6])，y in [6.0, 8.5]；
   - 级 2: 截面 9x9 (x/z in [-4.5, 4.5])，y in [8.5, 11.0]；
   - 级 3: 截面 6x6 (x/z in [-3, 3])，y in [11.0, 14.0]；
   - 材质全部采用活体红肌主色 #8a2a2a (flesh_main)；
3. 顶口内壁与内光：
   - 顶口内壁衬 #b05050 (flesh_lit) 亮红肌肉衬层；
   - 口中嵌入一块 2×2 的 #f6dcc4 (qi_glow) 真元内光；
4. 四面各一条 #c07868 筋丝从口沿到底座：
   - 北、南、西、东四面外壁各一条 #c07868 (tendon_fiber) 筋丝；
   - 紧贴三级阶梯肉质喷嘴外壁一路走台阶下延，牢牢拉索锚固到底座上；
5. 输出图：
   - render.png（3/4 视 + 侧视 + 俯视 + 正视 2×2 高清拼版）；
   - check.png（左爆炸参考图 x13_emitter.png，右 3/4 等轴视与侧视并排等高对标卡）。
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
    """底座半高框 (y 0–6)：
    - 4 根角骨节柱 (截面 2x2，底在 y=0、顶在 y=6，复用 common_parts.bone_post)；
    - 底面 4 条水平红肉棱 (y in [0, 2]) 与顶面 4 条水平红肉棱 (y in [4, 6])；
    - 肉棱外表面附微浮 #5a1a1a 暗纹；
    - 底面中心用 common_parts.outlet_8x6 (facing='-y') 做入口。
    """
    cubes = []

    # 1. 四根角骨节柱 (截面 2x2, y in [0.0, 6.0]，位于四外角)
    corners = [
        ("nw", -7.0, -7.0),
        ("ne",  7.0, -7.0),
        ("sw", -7.0,  7.0),
        ("se",  7.0,  7.0),
    ]
    for cname, cx, cz in corners:
        cubes.extend(bone_post(center_x=cx, center_z=cz, y_bot=0.0, height=6.0, group="base_frame", pfx=f"post_{cname}"))

    # 2. 底面 4 条水平红肉棱 (截面 2x2, y in [0.0, 2.0], 长 12px: x/z in [-6.0, 6.0])
    cubes.append({"name": "base_bot_rib_n", "from": [-6.0, 0.0, -8.0], "to": [ 6.0, 2.0, -6.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_bot_rib_s", "from": [-6.0, 0.0,  6.0], "to": [ 6.0, 2.0,  8.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_bot_rib_w", "from": [-8.0, 0.0, -6.0], "to": [-6.0, 2.0,  6.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_bot_rib_e", "from": [ 6.0, 0.0, -6.0], "to": [ 8.0, 2.0,  6.0], "group": "base_frame", "material": "flesh_main"})

    # 3. 顶面 4 条水平红肉棱 (截面 2x2, y in [4.0, 6.0], 长 12px: x/z in [-6.0, 6.0])
    cubes.append({"name": "base_top_rib_n", "from": [-6.0, 4.0, -8.0], "to": [ 6.0, 6.0, -6.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_top_rib_s", "from": [-6.0, 4.0,  6.0], "to": [ 6.0, 6.0,  8.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_top_rib_w", "from": [-8.0, 4.0, -6.0], "to": [-6.0, 6.0,  6.0], "group": "base_frame", "material": "flesh_main"})
    cubes.append({"name": "base_top_rib_e", "from": [ 6.0, 4.0, -6.0], "to": [ 8.0, 6.0,  6.0], "group": "base_frame", "material": "flesh_main"})

    # 4. 肉棱暗纹条 (#5a1a1a flesh_dark，贴于顶棱外侧微浮 0.05px)
    cubes.append({"name": "base_stripe_n", "from": [-3.0, 4.5, -8.05], "to": [3.0, 5.5, -7.95], "group": "base_frame", "material": "flesh_dark"})
    cubes.append({"name": "base_stripe_s", "from": [-3.0, 4.5,  7.95], "to": [3.0, 5.5,  8.05], "group": "base_frame", "material": "flesh_dark"})
    cubes.append({"name": "base_stripe_w", "from": [-8.05, 4.5, -3.0], "to": [-7.95, 5.5, 3.0], "group": "base_frame", "material": "flesh_dark"})
    cubes.append({"name": "base_stripe_e", "from": [ 7.95, 4.5, -3.0], "to": [ 8.05, 5.5, 3.0], "group": "base_frame", "material": "flesh_dark"})

    # 5. 底面中心入口 (common_parts.outlet_8x6，facing='-y')
    cubes.extend(outlet_8x6(center_x=0.0, center_y=1.0, center_z=0.0, facing="-y", length=2.0, pfx="base_inlet"))

    return cubes


def part_02_nozzle_tiers() -> List[dict]:
    """肉质喷嘴 3 级 (截面 12×12 → 9×9 → 6×6，到 y 14)：
    - 级 1: 截面 12x12 (x/z in [-6, 6]), y in [6.0, 8.5]；
    - 级 2: 截面 9x9 (x/z in [-4.5, 4.5]), y in [8.5, 11.0]；
    - 级 3: 截面 6x6 (x/z in [-3, 3]), y in [11.0, 14.0]；
    - 顶口内壁衬 #b05050，口中 #f6dcc4 2x2 真元光芯。
    """
    cubes = []

    # 1. 级 1 (截面 12x12, y in [6.0, 8.5])
    cubes.append({"name": "nozzle_t1", "from": [-6.0, 6.0, -6.0], "to": [6.0, 8.5, 6.0], "group": "nozzle", "material": "flesh_main"})

    # 2. 级 2 (截面 9x9, y in [8.5, 11.0])
    cubes.append({"name": "nozzle_t2", "from": [-4.5, 8.5, -4.5], "to": [4.5, 11.0, 4.5], "group": "nozzle", "material": "flesh_main"})

    # 3. 级 3 外壁四边围绕中心 4x4 顶口 (截面 6x6, y in [11.0, 14.0])
    # 北壁
    cubes.append({"name": "nozzle_t3_n", "from": [-3.0, 11.0, -3.0], "to": [ 3.0, 14.0, -2.0], "group": "nozzle", "material": "flesh_main"})
    # 南壁
    cubes.append({"name": "nozzle_t3_s", "from": [-3.0, 11.0,  2.0], "to": [ 3.0, 14.0,  3.0], "group": "nozzle", "material": "flesh_main"})
    # 西壁
    cubes.append({"name": "nozzle_t3_w", "from": [-3.0, 11.0, -2.0], "to": [-2.0, 14.0,  2.0], "group": "nozzle", "material": "flesh_main"})
    # 东壁
    cubes.append({"name": "nozzle_t3_e", "from": [ 2.0, 11.0, -2.0], "to": [ 3.0, 14.0,  2.0], "group": "nozzle", "material": "flesh_main"})

    # 4. 顶口内壁亮红衬层 (#b05050 flesh_lit)
    cubes.append({"name": "orifice_lining_n", "from": [-2.0, 12.0, -2.0], "to": [ 2.0, 14.0, -1.8], "group": "nozzle", "material": "flesh_lit"})
    cubes.append({"name": "orifice_lining_s", "from": [-2.0, 12.0,  1.8], "to": [ 2.0, 14.0,  2.0], "group": "nozzle", "material": "flesh_lit"})
    cubes.append({"name": "orifice_lining_w", "from": [-2.0, 12.0, -1.8], "to": [-1.8, 14.0,  1.8], "group": "nozzle", "material": "flesh_lit"})
    cubes.append({"name": "orifice_lining_e", "from": [ 1.8, 12.0, -1.8], "to": [ 2.0, 14.0,  1.8], "group": "nozzle", "material": "flesh_lit"})
    cubes.append({"name": "orifice_bottom",   "from": [-2.0, 11.8, -2.0], "to": [ 2.0, 12.0,  2.0], "group": "nozzle", "material": "flesh_lit"})

    # 5. 口中真元内光 (#f6dcc4 qi_glow, 2x2, x/z in [-1.0, 1.0], y in [12.5, 14.0])
    cubes.append({"name": "orifice_qi_glow", "from": [-1.0, 12.5, -1.0], "to": [1.0, 14.0, 1.0], "group": "nozzle", "material": "qi_glow"})

    return cubes


def part_03_nozzle_fibers() -> List[dict]:
    """四面各一条 #c07868 筋丝从口沿到底座：
    - 北、南、西、东四面外壁各一条；
    - 紧贴 3 级收窄阶梯台阶肉壁走台阶，从口沿 (y=14) 牢牢拉索锚固到底座 (y=6)。
    """
    cubes = []

    # 1. 北面筋丝 (沿北壁阶梯向下)
    cubes.append({"name": "fib_n_v3", "from": [-0.5, 11.06, -3.06], "to": [0.5, 14.0, -3.0], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_n_h2", "from": [-0.5, 11.0, -4.56], "to": [0.5, 11.06, -3.0], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_n_v2", "from": [-0.5, 8.56, -4.56], "to": [0.5, 11.0, -4.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_n_h1", "from": [-0.5, 8.5, -6.06], "to": [0.5, 8.56, -4.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_n_v1", "from": [-0.5, 6.0, -6.06], "to": [0.5, 8.5, -6.0], "group": "fibers", "material": "tendon_fiber"})

    # 2. 南面筋丝 (沿南壁阶梯向下)
    cubes.append({"name": "fib_s_v3", "from": [-0.5, 11.06, 3.0], "to": [0.5, 14.0, 3.06], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_s_h2", "from": [-0.5, 11.0, 3.0], "to": [0.5, 11.06, 4.56], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_s_v2", "from": [-0.5, 8.56, 4.5], "to": [0.5, 11.0, 4.56], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_s_h1", "from": [-0.5, 8.5, 4.5], "to": [0.5, 8.56, 6.06], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_s_v1", "from": [-0.5, 6.0, 6.0], "to": [0.5, 8.5, 6.06], "group": "fibers", "material": "tendon_fiber"})

    # 3. 西面筋丝 (沿西壁阶梯向下)
    cubes.append({"name": "fib_w_v3", "from": [-3.06, 11.06, -0.5], "to": [-3.0, 14.0, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_w_h2", "from": [-4.56, 11.0, -0.5], "to": [-3.0, 11.06, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_w_v2", "from": [-4.56, 8.56, -0.5], "to": [-4.5, 11.0, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_w_h1", "from": [-6.06, 8.5, -0.5], "to": [-4.5, 8.56, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_w_v1", "from": [-6.06, 6.0, -0.5], "to": [-6.0, 8.5, 0.5], "group": "fibers", "material": "tendon_fiber"})

    # 4. 东面筋丝 (沿东壁阶梯向下)
    cubes.append({"name": "fib_e_v3", "from": [ 3.0, 11.06, -0.5], "to": [ 3.06, 14.0, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_e_h2", "from": [ 3.0, 11.0, -0.5], "to": [ 4.56, 11.06, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_e_v2", "from": [ 4.5, 8.56, -0.5], "to": [ 4.56, 11.0, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_e_h1", "from": [ 4.5, 8.5, -0.5], "to": [ 6.06, 8.56, 0.5], "group": "fibers", "material": "tendon_fiber"})
    cubes.append({"name": "fib_e_v1", "from": [ 6.0, 6.0, -0.5], "to": [ 6.06, 8.5, 0.5], "group": "fibers", "material": "tendon_fiber"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_base_frame()
        + part_02_nozzle_tiers()
        + part_03_nozzle_fibers()
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
        ("3/4 ISOMETRIC (Half-Height Base & 3-Tier Nozzle)", im_iso, 10, 10),
        ("SIDE VIEW (Nozzle y:6-14, Base y:0-6 & Fibers)", im_side, cell_w + 20, 10),
        ("TOP VIEW (Qi-Glow Core 2x2 & Orifice Lining)", im_top, 10, cell_h + 20),
        ("FRONT VIEW (Base Posts, Ribs & Symmetrical Fibers)", im_front, cell_w + 20, cell_h + 20),
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

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: SIDE View | Base y:0-6, Nozzle y:6-14)", fill=(180, 220, 210))

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
        "from": [-6.0, 6.0, -6.0],
        "to":   [ 6.0, 8.5,  6.0],
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
