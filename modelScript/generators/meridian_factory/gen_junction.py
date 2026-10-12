#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b10: junction (经脉三通 / 四通) [第 2 批通用接口重构版 - 第 2 次修改]

风格：A 有机型 (活体血肉、暖粉半透明筋管、两端肉箍小骨环、中心肉质枢纽、交叉筋丝)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 爆炸参考图：model-review/img/meridian_factory/exploded/x10_junction.png
- 调度审第 2 次修改要求（2026-10-11 11:0x）：
  * 枢纽：x 6–10、z 6–10、y 1–9（4×4 截面、比管高 1px），#8a2a2a，顶面一个 #c07868 十字；
  * 每条臂：改用 6×4 细管（x 或 z 居中 5–11、y 3–7），从枢纽边伸到 z=1 (或 x=1 / 15)；
    臂之间必须留出空隙，俯视能看到四个角是空的；
  * 方块边 0–2 的 2px：flesh_collar 宽 2，截面放大到 8×6 的统一接口；骨环端口贴在肉箍顶面；
  * T 形去掉 +Z 臂，枢纽 +Z 面加 #5a1a1a 2×2 盲端凹点；
  * 十字形四向全通；
  * render 加俯视（T / 十字一眼能分）；
  * check 左爆炸图右渲染。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/junction")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x10_junction.png")
SINGLE_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b10_junction.png")

from modelScript.generators.meridian_factory.common_parts import (
    PALETTE,
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    flesh_collar,
    bone_ring_port,
)


# =============================================================================
# 各部件几何定义 (part_* 拆分并复用 common_parts)
# =============================================================================

def part_01_central_hub(is_cross: bool = False) -> List[dict]:
    """中心肉质枢纽：
    x 6–10、z 6–10、y 1–9 (模型坐标 X/Z in [-2.0, 2.0], Y in [1.0, 9.0]，4x4 截面、高 8px)，
    材质 #8a2a2a (flesh_main)；
    顶面 (y=9.0) 一个 #c07868 (tendon_fiber) 十字 (浮起 0.4px: Y in [9.0, 9.4])，中心交汇亮结；
    内部 #f6dcc4 真元内光；
    T 形在 +Z 面加 #5a1a1a 2x2 盲端凹点。
    """
    cubes = []
    pfx = "x_" if is_cross else "t_"

    # 1. 枢纽主体 (4x4 截面，高 8px: Y in [1.0, 9.0])
    cubes.append({
        "name": f"{pfx}hub_core",
        "from": [-2.0, 1.0, -2.0],
        "to":   [ 2.0, 9.0,  2.0],
        "group": "central_hub",
        "material": "flesh_main",
    })

    # 2. 顶面 #c07868 十字筋丝 (Y in [9.0, 9.4])
    cubes.append({
        "name": f"{pfx}hub_cross_x",
        "from": [-1.8, 9.0, -0.4],
        "to":   [ 1.8, 9.4,  0.4],
        "group": "central_hub",
        "material": "tendon_fiber",
    })
    cubes.append({
        "name": f"{pfx}hub_cross_z_n",
        "from": [-0.4, 9.0, -1.8],
        "to":   [ 0.4, 9.4, -0.4],
        "group": "central_hub",
        "material": "tendon_fiber",
    })
    cubes.append({
        "name": f"{pfx}hub_cross_z_s",
        "from": [-0.4, 9.0,  0.4],
        "to":   [ 0.4, 9.4,  1.8],
        "group": "central_hub",
        "material": "tendon_fiber",
    })
    # 十字中心亮结 (#e8bca8)
    cubes.append({
        "name": f"{pfx}hub_cross_center",
        "from": [-0.45, 9.38, -0.45],
        "to":   [ 0.45, 9.48,  0.45],
        "group": "central_hub",
        "material": "tendon_highlight",
    })

    # 3. 内部真元内光核心
    cubes.append({
        "name": f"{pfx}hub_qi_glow",
        "from": [-0.8, 4.0, -0.8],
        "to":   [ 0.8, 6.0,  0.8],
        "group": "central_hub",
        "material": "qi_glow",
    })

    # 4. T 形南面 (+Z): 枢纽 +Z 面加 #5a1a1a 2×2 盲端凹点 (X in [-1, 1], Y in [4, 6], Z in [1.9, 2.05])
    if not is_cross:
        cubes.append({
            "name": "t_hub_blind_dot",
            "from": [-1.0, 4.0, 1.9],
            "to":   [ 1.0, 6.0, 2.05],
            "group": "central_hub",
            "material": "flesh_dark",
        })

    return cubes


def part_02_arms_and_interfaces(is_cross: bool = False) -> List[dict]:
    """生成分支细管（6×4 截面从枢纽伸到方块边）、统一接口段（方块边 2px 放大到 8×6）、肉箍与顶面骨环端口：
    - 细臂截面 6×4 (y 3–7, 居中)，臂间四角留空，俯视四个角完全镂空；
    - 边缘 0–2 (2px): 放大到 8×6 统一接口 + 2px 肉箍 + 骨环端口贴在肉箍顶面。
    """
    cubes = []
    pfx = "x_" if is_cross else "t_"
    dirs = ["n", "s", "w", "e"] if is_cross else ["n", "w", "e"]

    for d in dirs:
        if d == "n":  # -Z 方向 (北)
            # 8x6 统一接口短管 (Z in [-8, -6])
            cubes.append({"name": f"{pfx}inlet_n_tube", "from": [-4.0, 2.0, -8.0], "to": [4.0, 8.0, -6.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_n_hl_bot", "from": [-3.9, 1.95, -7.95], "to": [3.9, 2.7, -6.05], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_n_hl_top", "from": [-3.9, 7.3,  -7.95], "to": [3.9, 8.05, -6.05], "group": "interface", "material": "tendon_highlight"})

            # 6x4 细管主体 (Z in [-6.0, -3.0], X in [-3, 3], Y in [3, 7])
            cubes.append({"name": f"{pfx}arm_n_6x4", "from": [-3.0, 3.0, -6.0], "to": [3.0, 7.0, -3.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_n_hl_bot", "from": [-2.9, 2.95, -5.95], "to": [2.9, 3.5, -3.05], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_n_hl_top", "from": [-2.9, 6.5,  -5.95], "to": [2.9, 7.05, -3.05], "group": "arms", "material": "tendon_highlight"})

            # 颈缩过渡进入 4x4 枢纽 (Z in [-3.0, -2.0], X in [-2, 2], Y in [3, 7])
            cubes.append({"name": f"{pfx}arm_n_neck", "from": [-2.0, 3.0, -3.0], "to": [2.0, 7.0, -2.0], "group": "arms", "material": "tendon_tube"})

            # 贯穿真元光芯
            cubes.append({"name": f"{pfx}arm_n_qi", "from": [-0.8, 4.5, -7.95], "to": [0.8, 5.5, -2.05], "group": "arms", "material": "qi_glow"})

            # 肉箍与骨环端口 (复用 common_parts)
            cubes.extend(flesh_collar(center_pos=-6.5, length=2.0, axis="z", pfx=f"{pfx}collar_n", group="interface"))
            cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=-6.5, face="up", pfx=f"{pfx}port_n", group="interface"))

        elif d == "s":  # +Z 方向 (南，仅十字四通拥有)
            cubes.append({"name": f"{pfx}inlet_s_tube", "from": [-4.0, 2.0,  6.0], "to": [4.0, 8.0,  8.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_s_hl_bot", "from": [-3.9, 1.95,  6.05], "to": [3.9, 2.7,  7.95], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_s_hl_top", "from": [-3.9, 7.3,   6.05], "to": [3.9, 8.05, 7.95], "group": "interface", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_s_6x4", "from": [-3.0, 3.0,  3.0], "to": [3.0, 7.0,  6.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_s_hl_bot", "from": [-2.9, 2.95,  3.05], "to": [2.9, 3.5,  5.95], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_s_hl_top", "from": [-2.9, 6.5,   3.05], "to": [2.9, 7.05, 5.95], "group": "arms", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_s_neck", "from": [-2.0, 3.0,  2.0], "to": [2.0, 7.0,  3.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_s_qi", "from": [-0.8, 4.5,  2.05], "to": [0.8, 5.5,  7.95], "group": "arms", "material": "qi_glow"})

            cubes.extend(flesh_collar(center_pos=6.5, length=2.0, axis="z", pfx=f"{pfx}collar_s", group="interface"))
            cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=6.5, face="up", pfx=f"{pfx}port_s", group="interface"))

        elif d == "w":  # -X 方向 (西)
            cubes.append({"name": f"{pfx}inlet_w_tube", "from": [-8.0, 2.0, -4.0], "to": [-6.0, 8.0, 4.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_w_hl_bot", "from": [-7.95, 1.95, -3.9], "to": [-6.05, 2.7, 3.9], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_w_hl_top", "from": [-7.95, 7.3,  -3.9], "to": [-6.05, 8.05, 3.9], "group": "interface", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_w_6x4", "from": [-6.0, 3.0, -3.0], "to": [-3.0, 7.0, 3.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_w_hl_bot", "from": [-5.95, 2.95, -2.9], "to": [-3.05, 3.5, 2.9], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_w_hl_top", "from": [-5.95, 6.5,  -2.9], "to": [-3.05, 7.05, 2.9], "group": "arms", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_w_neck", "from": [-3.0, 3.0, -2.0], "to": [-2.0, 7.0, 2.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_w_qi", "from": [-7.95, 4.5, -0.8], "to": [-2.05, 5.5, 0.8], "group": "arms", "material": "qi_glow"})

            cubes.extend(flesh_collar(center_pos=-6.5, length=2.0, axis="x", pfx=f"{pfx}collar_w", group="interface"))
            cubes.extend(bone_ring_port(center_x=-6.5, center_y=9.0, center_z=0.0, face="up", pfx=f"{pfx}port_w", group="interface"))

        elif d == "e":  # +X 方向 (东)
            cubes.append({"name": f"{pfx}inlet_e_tube", "from": [ 6.0, 2.0, -4.0], "to": [ 8.0, 8.0, 4.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_e_hl_bot", "from": [ 6.05, 1.95, -3.9], "to": [ 7.95, 2.7, 3.9], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_e_hl_top", "from": [ 6.05, 7.3,  -3.9], "to": [ 7.95, 8.05, 3.9], "group": "interface", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_e_6x4", "from": [ 3.0, 3.0, -3.0], "to": [ 6.0, 7.0, 3.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_e_hl_bot", "from": [ 3.05, 2.95, -2.9], "to": [ 5.95, 3.5, 2.9], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_e_hl_top", "from": [ 3.05, 6.5,  -2.9], "to": [ 5.95, 7.05, 2.9], "group": "arms", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_e_neck", "from": [ 2.0, 3.0, -2.0], "to": [ 3.0, 7.0, 2.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_e_qi", "from": [ 2.05, 4.5, -0.8], "to": [ 7.95, 5.5, 0.8], "group": "arms", "material": "qi_glow"})

            cubes.extend(flesh_collar(center_pos=6.5, length=2.0, axis="x", pfx=f"{pfx}collar_e", group="interface"))
            cubes.extend(bone_ring_port(center_x=6.5, center_y=9.0, center_z=0.0, face="up", pfx=f"{pfx}port_e", group="interface"))

    return cubes


def all_cubes(is_cross: bool = False) -> List[dict]:
    """汇总指定变体的所有立方体。"""
    return part_01_central_hub(is_cross) + part_02_arms_and_interfaces(is_cross)


# =============================================================================
# 导出与渲染
# =============================================================================

def generate_bbmodel(is_cross: bool, out_path: Path) -> Path:
    """导出 junction 变体的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(is_cross)
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

    name = "junction_x" if is_cross else "junction_t"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "junction")
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


def render_views(p_t_path: Path, p_x_path: Path):
    """输出包含 junction_t 与 junction_x（3/4 视 + 俯视）的综合拼图 render.png 与爆炸图并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 渲染 junction_t (T形三通)
    im_t_iso, _ = render(p_t_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_t_top, _ = render(p_t_path, yaw=0.0,   pitch=89.9, size=500, bg=bg_color)

    # 渲染 junction_x (十字四通)
    im_x_iso, _ = render(p_x_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_x_top, _ = render(p_x_path, yaw=0.0,   pitch=89.9, size=500, bg=bg_color)

    # 1. 拼装 render.png (2x2 网格拼版：上行 3/4 等轴视对比，下行 俯视对比)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("junction_t (3/4 ISOMETRIC VIEW - 3-way)", im_t_iso, 10, 10),
        ("junction_x (3/4 ISOMETRIC VIEW - 4-way)", im_x_iso, cell_w + 20, 10),
        ("junction_t (TOP VIEW - Hollow Corners & T-shape)", im_t_top, 10, cell_h + 20),
        ("junction_x (TOP VIEW - Hollow Corners & Cross)", im_x_top, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 470, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (两变体 3/4 视 + 俯视 2x2 拼版) 已输出: {render_path}")

    # 2. 拼装 check.png (左为 x10 爆炸参考图，右为两变体 3/4 等轴视与俯视并排对标)
    ref_image_to_use = EXPLODED_REF if EXPLODED_REF.exists() else SINGLE_REF
    if ref_image_to_use.exists():
        ref_im = Image.open(ref_image_to_use).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_t_iso = im_t_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_x_iso = im_x_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        ref_title = f"EXPLODED REFERENCE ({ref_image_to_use.name})"
        c_draw.text((16, 10), ref_title, fill=(210, 200, 180))

        # 贴右渲染图 (左 junction_t，右 junction_x)
        rx = ref_w + 24
        check_cv.paste(c_t_iso, (rx, 34 + 10))
        check_cv.paste(c_x_iso, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: junction_t 3-way, RIGHT: junction_x 4-way Cross)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 爆炸图并排对照卡 (含两变体 3/4 对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_junction.py 差分自证...")

    # 1. junction_t 测试
    cubes_t = all_cubes(is_cross=False)
    _assert_no_coplanar_faces(cubes_t)
    print(f"  [OK] junction_t ({len(cubes_t)} 个立方体) 正常立方体集无共面冲突")

    defect_t = list(cubes_t) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [ 4.0, 8.0, -6.0],
        "material": "tendon_tube",
    }]
    caught_t = False
    try:
        _assert_no_coplanar_faces(defect_t)
    except AssertionError as e:
        caught_t = True
        print(f"  [OK] junction_t 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
    if not caught_t:
        raise RuntimeError("门禁失效: junction_t 注入共面冲突未被拦截!")

    # 2. junction_x 测试
    cubes_x = all_cubes(is_cross=True)
    _assert_no_coplanar_faces(cubes_x)
    print(f"  [OK] junction_x ({len(cubes_x)} 个立方体) 正常立方体集无共面冲突")

    defect_x = list(cubes_x) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, 6.0],
        "to":   [ 4.0, 8.0, 8.0],
        "material": "tendon_tube",
    }]
    caught_x = False
    try:
        _assert_no_coplanar_faces(defect_x)
    except AssertionError as e:
        caught_x = True
        print(f"  [OK] junction_x 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")
    if not caught_x:
        raise RuntimeError("门禁失效: junction_x 注入共面冲突未被拦截!")

    print("✓ gen_junction.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b10 junction 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()

    p_t = generate_bbmodel(is_cross=False, out_path=MODEL_DIR / "junction_t.bbmodel")
    p_x = generate_bbmodel(is_cross=True,  out_path=MODEL_DIR / "junction_x.bbmodel")
    generate_bbmodel(is_cross=False, out_path=MODEL_DIR / "junction.bbmodel")

    render_views(p_t, p_x)


if __name__ == "__main__":
    main()
