#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b11: valve (经脉阀门，含 open / half / closed 三变体)

风格：A 有机型 (活体血肉、暖粉半透明筋管、正中显眼括约肌环、顶置骨柄与骨轮、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md (第 2 批表格与规范)
- 爆炸参考图：model-review/img/meridian_factory/exploded/x11_valve.png
- 调度审第 1 次指示：
  1. 两端肉箍改窄：flesh_collar 宽 2 (z 0–2 对应 [-8, -6]; z 14–16 对应 [6, 8])，筋管 z 2–14 露出来；
  2. 括约肌环放在 z 6–10，外 12×10 (比肉箍大一圈，3/4 里要比肉箍更显眼)，#8a2a2a，表面 6 条 #b05050 / #5a1a1a 交替的环向褶 (每条 1px 浮起)；
  3. 开度要在管口方向和侧面都看得出：
     - 环内孔 open 8×6 (与管腔一样)、half 4×3、closed 2×2，孔内壁 #5a1a1a；
     - 环在 half / closed 时外径与厚度各再鼓 1px (被挤压鼓起)，侧面能看出明显差别；
  4. 骨轮放在环顶正中 (common_parts.bone_wheel)，轮下一根 2×2×2 骨柄连到环；
  5. render 输出：三变体 3/4 视 + 侧视 (看外径鼓起) + 正对管口直视 (看内孔开度)；check.png 爆炸图对标。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/valve")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x11_valve.png")
SINGLE_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b11_valve.png")

from modelScript.generators.meridian_factory.common_parts import (
    PALETTE,
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    sinew_tube,
    flesh_collar,
    bone_ring_port,
    bone_wheel,
)


# =============================================================================
# 各部件几何定义 (part_* 拆分并复用 common_parts)
# =============================================================================

def part_01_pipe_and_collars(variant: str = "open") -> List[dict]:
    """生成前后管段、两端肉箍与贴面骨环端口：
    - 两端肉箍改窄：宽 2px (后箍 [-8, -6], 前箍 [6, 8])
    - 筋管露出来：[-6.0, z_min] 与 [z_max, 6.0] 显眼露出长达 4px 暖粉管壁
    - 贴面骨环端口：贴在两端肉箍顶底面
    """
    cubes = []
    pfx = f"{variant}_"

    # 变体对应的肌环厚度 Z 范围
    if variant == "open":
        z_min, z_max = -2.0, 2.0
    elif variant == "half":
        z_min, z_max = -2.5, 2.5
    else: # closed
        z_min, z_max = -3.0, 3.0

    # 1. 前后露出筋管段 (复用 common_parts.sinew_tube)
    cubes.extend(sinew_tube(-6.0, z_min, axis="z", with_inner_qi=True, with_highlights=True, pfx=f"{pfx}tube_rear"))
    cubes.extend(sinew_tube(z_max,  6.0, axis="z", with_inner_qi=True, with_highlights=True, pfx=f"{pfx}tube_front"))

    # 2. 两端改窄肉箍 (宽 2px: center=-7.0 与 center=7.0, 复用 common_parts.flesh_collar)
    cubes.extend(flesh_collar(center_pos=-7.0, length=2.0, axis="z", pfx=f"{pfx}collar_rear"))
    cubes.extend(flesh_collar(center_pos= 7.0, length=2.0, axis="z", pfx=f"{pfx}collar_front"))

    # 3. 肉箍顶面与底面骨环端口 (复用 common_parts.bone_ring_port)
    cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=-7.0, face="up",   pfx=f"{pfx}port_r_t"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=1.0, center_z=-7.0, face="down", pfx=f"{pfx}port_r_b"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z= 7.0, face="up",   pfx=f"{pfx}port_f_t"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=1.0, center_z= 7.0, face="down", pfx=f"{pfx}port_f_b"))

    return cubes


def part_02_sphincter_ring(variant: str = "open") -> List[dict]:
    """生成正中显眼括约肌环与表面 6 条环向褶纹：
    - open: 外 12×10 (X: [-6, 6], Y: [0, 10]), 厚 4px (Z: [-2, 2])
    - half: 外径与厚度各鼓 1px -> 外 13×11 (X: [-6.5, 6.5], Y: [0, 11]), 厚 5px (Z: [-2.5, 2.5])
    - closed: 外径与厚度再鼓 1px -> 外 14×12 (X: [-7, 7], Y: [0, 12]), 厚 6px (Z: [-3, 3])
    - 表面 6 条 #b05050 / #5a1a1a 交替的环向褶 (每条 1px 浮起)
    """
    cubes = []
    pfx = f"{variant}_"

    if variant == "open":
        z_min, z_max = -2.0, 2.0
        r_x, r_y = 6.0, 10.0
    elif variant == "half":
        z_min, z_max = -2.5, 2.5
        r_x, r_y = 6.5, 11.0
    else: # closed
        z_min, z_max = -3.0, 3.0
        r_x, r_y = 7.0, 12.0

    # 1. 环体无交集分区外框 (#8a2a2a flesh_main)
    # 底中: X in [-4, 4], Y in [0, 2]
    cubes.append({"name": f"{pfx}sph_bot_mid", "from": [-4.0, 0.0, z_min], "to": [ 4.0, 2.0, z_max], "group": "sphincter", "material": "flesh_main"})
    # 顶中: X in [-4, 4], Y in [8, r_y]
    cubes.append({"name": f"{pfx}sph_top_mid", "from": [-4.0, 8.0, z_min], "to": [ 4.0, r_y, z_max], "group": "sphincter", "material": "flesh_main"})
    # 左中: X in [-r_x, -4], Y in [2, 8]
    cubes.append({"name": f"{pfx}sph_l_mid",   "from": [-r_x, 2.0, z_min], "to": [-4.0, 8.0, z_max], "group": "sphincter", "material": "flesh_main"})
    # 右中: X in [4, r_x], Y in [2, 8]
    cubes.append({"name": f"{pfx}sph_r_mid",   "from": [ 4.0, 2.0, z_min], "to": [ r_x, 8.0, z_max], "group": "sphincter", "material": "flesh_main"})

    # 四角倒角块 (外切角 1x1，形成生动饱满的圆形肌环)
    cubes.append({"name": f"{pfx}sph_c_bl", "from": [-r_x + 1.0, 1.0, z_min], "to": [-4.0, 2.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_c_br", "from": [ 4.0, 1.0, z_min], "to": [ r_x - 1.0, 2.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_c_tl", "from": [-r_x + 1.0, 8.0, z_min], "to": [-4.0, r_y - 1.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_c_tr", "from": [ 4.0, 8.0, z_min], "to": [ r_x - 1.0, r_y - 1.0, z_max], "group": "sphincter", "material": "flesh_main"})

    # 2. 表面 6 条 #b05050 / #5a1a1a 交替的环向褶 (每条浮起 0.8px ~ 1.0px)
    # 前端面 (从外到内环绕孔道): 3 条 (上环褶、左环褶、右环褶)
    # 褶 1: 前顶环褶 (#b05050 flesh_lit)
    cubes.append({"name": f"{pfx}fold_f1_top", "from": [-r_x + 1.8, r_y - 1.5, z_min - 0.8], "to": [r_x - 1.8, r_y - 0.5, z_min], "group": "sphincter_folds", "material": "flesh_lit"})
    # 褶 2: 前左立褶 (#5a1a1a flesh_dark)
    cubes.append({"name": f"{pfx}fold_f2_l",   "from": [-r_x + 0.5, 3.5, z_min - 0.8], "to": [-r_x + 1.5, 7.5, z_min], "group": "sphincter_folds", "material": "flesh_dark"})
    # 褶 3: 前右立褶 (#b05050 flesh_lit)
    cubes.append({"name": f"{pfx}fold_f3_r",   "from": [ r_x - 1.5, 3.5, z_min - 0.8], "to": [ r_x - 0.5, 7.5, z_min], "group": "sphincter_folds", "material": "flesh_lit"})

    # 后端面 3 条:
    # 褶 4: 后顶环褶 (#5a1a1a flesh_dark)
    cubes.append({"name": f"{pfx}fold_b1_top", "from": [-r_x + 1.8, r_y - 1.5, z_max], "to": [r_x - 1.8, r_y - 0.5, z_max + 0.8], "group": "sphincter_folds", "material": "flesh_dark"})
    # 褶 5: 后左立褶 (#b05050 flesh_lit)
    cubes.append({"name": f"{pfx}fold_b2_l",   "from": [-r_x + 0.5, 3.5, z_max], "to": [-r_x + 1.5, 7.5, z_max + 0.8], "group": "sphincter_folds", "material": "flesh_lit"})
    # 褶 6: 后右立褶 (#5a1a1a flesh_dark)
    cubes.append({"name": f"{pfx}fold_b3_r",   "from": [ r_x - 1.5, 3.5, z_max], "to": [ r_x - 0.5, 7.5, z_max + 0.8], "group": "sphincter_folds", "material": "flesh_dark"})

    return cubes


def part_03_valve_lumen(variant: str = "open") -> List[dict]:
    """生成三变体内孔开闭形态 (在 z in [z_min, z_max] 核心腔)：
    - open: 内孔 8×6 (与管腔完全贯通，暖粉管腔 + 2×2 真元光芯)；
    - half: 内孔 4×3，孔内壁 #5a1a1a，周围深红肌肉层向内收缩挤压隆起；
    - closed: 内孔 2×2，孔内壁 #5a1a1a，四周完全紧闭阻断。
    """
    cubes = []
    pfx = f"{variant}_"

    if variant == "open":
        z_min, z_max = -2.0, 2.0
        # 8x6 贯通暖粉管腔 + 2x2 真元光芯
        cubes.append({"name": f"{pfx}lumen_tube", "from": [-4.0, 2.0, z_min],        "to": [4.0, 8.0, z_max],        "group": "valve_lumen", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}lumen_qi",   "from": [-1.0, 4.0, z_min + 0.05], "to": [1.0, 6.0, z_max - 0.05], "group": "valve_lumen", "material": "qi_glow"})

    elif variant == "half":
        z_min, z_max = -2.5, 2.5
        # 孔 4x3: X in [-2, 2], Y in [3.5, 6.5]
        hx, hy = 2.0, 1.5
        cy = 5.0
        # 四周收缩挤压肌肉层 (#8a2a2a flesh_main)
        cubes.append({"name": f"{pfx}con_bot", "from": [-4.0, 2.0, z_min], "to": [ 4.0, cy - hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_top", "from": [-4.0, cy + hy, z_min], "to": [ 4.0, 8.0, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_l",   "from": [-4.0, cy - hy, z_min], "to": [-hx, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_r",   "from": [ hx, cy - hy, z_min], "to": [ 4.0, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        # 4x3 孔内壁 (#5a1a1a flesh_dark)
        cubes.append({"name": f"{pfx}con_hole_lining", "from": [-hx, cy - hy, z_min], "to": [hx, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_dark"})

    elif variant == "closed":
        z_min, z_max = -3.0, 3.0
        # 孔 2x2: X in [-1, 1], Y in [4, 6]
        hx, hy = 1.0, 1.0
        cy = 5.0
        # 四周深红肌肉层全面紧缩填充
        cubes.append({"name": f"{pfx}con_bot", "from": [-4.0, 2.0, z_min], "to": [ 4.0, cy - hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_top", "from": [-4.0, cy + hy, z_min], "to": [ 4.0, 8.0, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_l",   "from": [-4.0, cy - hy, z_min], "to": [-hx, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_r",   "from": [ hx, cy - hy, z_min], "to": [ 4.0, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        # 2x2 闭死微孔内壁 (#5a1a1a flesh_dark)
        cubes.append({"name": f"{pfx}con_hole_dark", "from": [-hx, cy - hy, z_min], "to": [hx, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_dark"})

    return cubes


def part_04_stem_and_wheel(variant: str = "open") -> List[dict]:
    """骨轮 + 骨柄：
    - 轮下一根 2×2×2 骨柄连到环 (#d8ccb0 / #b8a888)；
    - 骨轮放在环顶正中 (复用 common_parts.bone_wheel, 6×1×6, 十字辐条+外圆环)。
    """
    cubes = []
    pfx = f"{variant}_"

    if variant == "open":
        r_y = 10.0
    elif variant == "half":
        r_y = 11.0
    else: # closed
        r_y = 12.0

    # 1. 2x2x2 骨柄 (连到环): X in [-1, 1], Z in [-1, 1], Y in [r_y, r_y + 2.0]
    cubes.append({
        "name": f"{pfx}bone_stem",
        "from": [-1.0, r_y, -1.0],
        "to":   [ 1.0, r_y + 2.0, 1.0],
        "group": "bone_wheel",
        "material": "bone_dark",
    })

    # 2. 骨轮放在骨柄顶上 (center_y = r_y + 2.5, radius=3.0, thickness=1.0)
    cubes.extend(bone_wheel(center_x=0.0, center_y=r_y + 2.5, center_z=0.0, radius=3.0, thickness=1.0, axis="y", pfx=f"{pfx}wheel"))

    return cubes


def all_cubes(variant: str = "open") -> List[dict]:
    """汇总指定变体的所有立方体。"""
    return (
        part_01_pipe_and_collars(variant)
        + part_02_sphincter_ring(variant)
        + part_03_valve_lumen(variant)
        + part_04_stem_and_wheel(variant)
    )


# =============================================================================
# 导出与渲染
# =============================================================================

def generate_bbmodel(variant: str, out_path: Path) -> Path:
    """导出指定变体的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(variant)
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
        uv = MAT_UV.get(mat, [0, 0, 16, 16])
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

    name = f"valve_{variant}"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "valve")
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


def render_views(paths: Dict[str, Path]):
    """输出三变体（3/4 视 + 侧视 + 正对管口直视）的综合拼图 render.png 与爆炸图并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    variants = ["open", "half", "closed"]
    renders = {}

    for v in variants:
        p = paths[v]
        # 1. 3/4 等轴视 (看整体大括约肌环、暖粉管露出、顶置骨轮与骨柄)
        im_iso, _ = render(p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
        # 2. 侧视 (yaw=-90.0, pitch=0.0，侧面清楚对比外径与厚度逐级鼓 1px 的挤压形变)
        im_side, _ = render(p, yaw=-90.0, pitch=0.0, size=500, bg=bg_color)
        # 3. 正对管口直视 (yaw=0.0, pitch=0.0，正对 -Z 管口朝 +Z 看进去，直视孔径开度 8x6 / 4x3 / 2x2)
        im_front, _ = render(p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
        renders[v] = (im_iso, im_side, im_front)

    # 1. 拼装 render.png (3 行 3 列：行 1 为 3/4 视，行 2 为侧视看鼓起，行 3 为正对管口看开度)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 3 + 40
    canvas_h = cell_h * 3 + 40
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    titles = [
        # 行 1: 3/4 等轴视
        ("valve_open (3/4 View: Ring 12x10, Stems & Wheel)", renders["open"][0], 10, 10),
        ("valve_half (3/4 View: Ring 13x11, Swollen +1px)", renders["half"][0], cell_w + 20, 10),
        ("valve_closed (3/4 View: Ring 14x12, Swollen +2px)", renders["closed"][0], cell_w * 2 + 30, 10),
        # 行 2: 侧视 (看挤压鼓起与厚度变化)
        ("valve_open (SIDE: Height 10, Thick 4px)", renders["open"][1], 10, cell_h + 20),
        ("valve_half (SIDE: Height 11, Thick 5px, Swollen)", renders["half"][1], cell_w + 20, cell_h + 20),
        ("valve_closed (SIDE: Height 12, Thick 6px, Max Swell)", renders["closed"][1], cell_w * 2 + 30, cell_h + 20),
        # 行 3: 正对管口直视 (看孔径开度)
        ("valve_open (FRONT VIEW: Full 8x6 Open Lumen)", renders["open"][2], 10, cell_h * 2 + 30),
        ("valve_half (FRONT VIEW: Constricted 4x3 Lumen)", renders["half"][2], cell_w + 20, cell_h * 2 + 30),
        ("valve_closed (FRONT VIEW: Sealed 2x2 Dark Hole)", renders["closed"][2], cell_w * 2 + 30, cell_h * 2 + 30),
    ]

    for title, im_v, px, py in titles:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 440, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (三变体 3/4视 + 侧视鼓起 + 正对管口开度 3x3 拼版) 已输出: {render_path}")

    # 2. 拼装 check.png (左为 x11 爆炸参考图，右为三变体 3/4 视角并排等高对标)
    ref_image_to_use = EXPLODED_REF if EXPLODED_REF.exists() else SINGLE_REF
    if ref_image_to_use.exists():
        ref_im = Image.open(ref_image_to_use).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_open = renders["open"][0].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_half = renders["half"][0].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_closed = renders["closed"][0].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 3 + 24
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        ref_title = f"EXPLODED REFERENCE ({ref_image_to_use.name})"
        c_draw.text((16, 10), ref_title, fill=(210, 200, 180))

        # 贴右渲染图 (三变体 3/4 视并排)
        rx = ref_w + 24
        check_cv.paste(c_open, (rx, 34 + 10))
        check_cv.paste(c_half, (rx + thumb_size + 8, 34 + 10))
        check_cv.paste(c_closed, (rx + (thumb_size + 8) * 2, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: valve_open, MID: valve_half, RIGHT: valve_closed)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 爆炸图并排对照卡 (含三变体 3/4 对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_valve.py 差分自证...")

    for v in ["open", "half", "closed"]:
        cubes = all_cubes(v)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] valve_{v} ({len(cubes)} 个立方体) 正常立方体集无共面冲突")

        defect = list(cubes) + [{
            "name": "inject_coplanar_fail",
            "from": [-4.0, 2.0, -6.0],
            "to":   [ 4.0, 8.0, -2.0],
            "material": "tendon_tube",
        }]
        caught = False
        try:
            _assert_no_coplanar_faces(defect)
        except AssertionError as e:
            caught = True
            print(f"  [OK] valve_{v} 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
        if not caught:
            raise RuntimeError(f"门禁失效: valve_{v} 注入共面冲突未被拦截!")

    print("✓ gen_valve.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b11 valve 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()

    paths = {}
    for v in ["open", "half", "closed"]:
        out_p = MODEL_DIR / f"valve_{v}.bbmodel"
        paths[v] = generate_bbmodel(v, out_p)

    generate_bbmodel("open", MODEL_DIR / "valve.bbmodel")

    render_views(paths)


if __name__ == "__main__":
    main()
