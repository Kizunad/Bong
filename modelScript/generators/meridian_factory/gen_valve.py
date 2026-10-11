#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b11: valve (经脉阀门，含 open / half / closed 三变体)

风格：A 有机型 (活体血肉、暖粉半透明筋管、中间括约肌环、顶置小骨轮、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md (第 2 批表格与规范)
- 爆炸参考图：model-review/img/meridian_factory/exploded/x11_valve.png

调度要求（整格 16×16×16，长轴沿 z 走向）：
1. 沿 z 一节 b01 筋管，两端用 common_parts.sinew_tube + flesh_collar，两端骨环端口；
2. 正中一圈括约肌环（#8a2a2a flesh_main，外 12×10、厚 3px: z in [-1.5, 1.5], x in [-6, 6], y in [0, 10]）：
   表面 4~6 条 #5a1a1a (flesh_dark) 环向褶纹；
3. 三变体：
   - valve_open: 管腔不变（贯通 8×6 暖粉管腔与 2×2 真元光芯）；
   - valve_half: 环向内收 2px（管腔收窄至 4×2，肌肉向内挤压隆起）；
   - valve_closed: 收到中心只剩 2×2 的 #5a1a1a 暗色小孔（四周完全闭合封死）；
4. 环顶用 common_parts.bone_wheel：
   小骨轮（#d8ccb0，外廓 6×1×6，十字辐条 + 外圆环，带 #b8a888 中心轴柱）；
5. 输出图：
   - render.png（三变体 3/4 + 正对管口 2×3 拼版）；
   - check.png（左爆炸图 x11_valve.png，右渲染对标）。
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
    - 后管段: z in [-8.0, -1.5] (长 6.5px)
    - 前管段: z in [1.5, 8.0] (长 6.5px)
    - 两端肉箍: 后箍 center=-6.0, 前箍 center=6.0 (length=3.0)
    - 顶面与底面骨环端口: 共 4 处
    """
    cubes = []
    pfx = f"{variant}_"

    # 1. 前后标准 8x6 暖粉筋管段 (复用 common_parts.sinew_tube)
    cubes.extend(sinew_tube(-8.0, -1.5, axis="z", with_inner_qi=True, with_highlights=True, pfx=f"{pfx}tube_rear"))
    cubes.extend(sinew_tube( 1.5,  8.0, axis="z", with_inner_qi=True, with_highlights=True, pfx=f"{pfx}tube_front"))

    # 2. 两端四面包覆肉箍 (复用 common_parts.flesh_collar)
    cubes.extend(flesh_collar(center_pos=-6.0, length=3.0, axis="z", pfx=f"{pfx}collar_rear"))
    cubes.extend(flesh_collar(center_pos= 6.0, length=3.0, axis="z", pfx=f"{pfx}collar_front"))

    # 3. 肉箍顶面与底面骨环端口 (复用 common_parts.bone_ring_port)
    cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=-6.0, face="up",   pfx=f"{pfx}port_r_t"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=1.0, center_z=-6.0, face="down", pfx=f"{pfx}port_r_b"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z= 6.0, face="up",   pfx=f"{pfx}port_f_t"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=1.0, center_z= 6.0, face="down", pfx=f"{pfx}port_f_b"))

    return cubes


def part_02_sphincter_ring(variant: str = "open") -> List[dict]:
    """生成正中括约肌环 (#8a2a2a，外 12×10、厚 3px: z in [-1.5, 1.5], x in [-6, 6], y in [0, 10])：
    - 四周圆钝阶梯外轮廓；
    - 表面 6 条 #5a1a1a 环向褶纹。
    """
    cubes = []
    pfx = f"{variant}_"

    # 1. 环体无交集分区构型 (外框 12x10: X in [-6, 6], Y in [0, 10], Z in [-1.5, 1.5])
    # 顶壁 (x in [-4, 4], y in [8, 10])
    cubes.append({"name": f"{pfx}sph_top", "from": [-4.0, 8.0, -1.5], "to": [ 4.0, 10.0, 1.5], "group": "sphincter", "material": "flesh_main"})
    # 底壁 (x in [-4, 4], y in [0, 2])
    cubes.append({"name": f"{pfx}sph_bot", "from": [-4.0, 0.0, -1.5], "to": [ 4.0,  2.0, 1.5], "group": "sphincter", "material": "flesh_main"})
    # 左壁 (x in [-6, -4], y in [2, 8])
    cubes.append({"name": f"{pfx}sph_l",   "from": [-6.0, 2.0, -1.5], "to": [-4.0,  8.0, 1.5], "group": "sphincter", "material": "flesh_main"})
    # 右壁 (x in [4, 6], y in [2, 8])
    cubes.append({"name": f"{pfx}sph_r",   "from": [ 4.0, 2.0, -1.5], "to": [ 6.0,  8.0, 1.5], "group": "sphincter", "material": "flesh_main"})

    # 四角倒角阶梯块 (外切角 1x1，呈圆钝活体环形)
    cubes.append({"name": f"{pfx}sph_tl", "from": [-5.0, 8.0, -1.5], "to": [-4.0, 9.0, 1.5], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_tr", "from": [ 4.0, 8.0, -1.5], "to": [ 5.0, 9.0, 1.5], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_bl", "from": [-5.0, 1.0, -1.5], "to": [-4.0, 2.0, 1.5], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_br", "from": [ 4.0, 1.0, -1.5], "to": [ 5.0, 2.0, 1.5], "group": "sphincter", "material": "flesh_main"})

    # 2. 表面 6 条 #5a1a1a 环向褶纹 (贴于前后端面，微浮 0.05px)
    # 前端面褶纹 (z in [-1.55, -1.45])
    cubes.append({"name": f"{pfx}fold_f_top", "from": [-3.5, 8.4, -1.55], "to": [3.5, 9.2, -1.45], "group": "sphincter", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}fold_f_bot", "from": [-3.5, 0.8, -1.55], "to": [3.5, 1.6, -1.45], "group": "sphincter", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}fold_f_l",   "from": [-5.8, 3.0, -1.55], "to": [-5.0, 7.0, -1.45], "group": "sphincter", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}fold_f_r",   "from": [ 5.0, 3.0, -1.55], "to": [ 5.8, 7.0, -1.45], "group": "sphincter", "material": "flesh_dark"})
    # 后端面褶纹 (z in [1.45, 1.55])
    cubes.append({"name": f"{pfx}fold_b_top", "from": [-3.5, 8.4, 1.45], "to": [3.5, 9.2, 1.55], "group": "sphincter", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}fold_b_bot", "from": [-3.5, 0.8, 1.45], "to": [3.5, 1.6, 1.55], "group": "sphincter", "material": "flesh_dark"})

    return cubes


def part_03_valve_lumen(variant: str = "open") -> List[dict]:
    """生成三变体内孔开闭形态 (在 z in [-1.5, 1.5] 核心腔)：
    - open: 管腔不变，贯通 8x6 暖粉管腔与 2x2 真元光芯；
    - half: 环向内收 2px，管腔收窄至 4x2，四周肌肉向内收缩挤压；
    - closed: 收到中心只剩 2x2 #5a1a1a 小孔，四周完全紧闭封死。
    """
    cubes = []
    pfx = f"{variant}_"

    if variant == "open":
        # 8x6 暖粉管腔贯通 + 2x2 真元光芯
        cubes.append({"name": f"{pfx}mid_tube", "from": [-4.0, 2.0, -1.5],  "to": [4.0, 8.0, 1.5],  "group": "valve_lumen", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}mid_qi",   "from": [-1.0, 4.0, -1.45], "to": [1.0, 6.0, 1.45], "group": "valve_lumen", "material": "qi_glow"})

    elif variant == "half":
        # 环向内收 2px (孔道从 8x6 收窄至 4x2: x in [-2, 2], y in [4, 6])
        # 内收肉层 (#8a2a2a)
        cubes.append({"name": f"{pfx}constrict_top", "from": [-4.0, 6.0, -1.5], "to": [ 4.0, 8.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}constrict_bot", "from": [-4.0, 2.0, -1.5], "to": [ 4.0, 4.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}constrict_l",   "from": [-4.0, 4.0, -1.5], "to": [-2.0, 6.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}constrict_r",   "from": [ 2.0, 4.0, -1.5], "to": [ 4.0, 6.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        # 4x2 收窄管腔与细光芯
        cubes.append({"name": f"{pfx}constrict_lumen", "from": [-2.0, 4.0, -1.5],  "to": [2.0, 6.0, 1.5],  "group": "valve_lumen", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}constrict_qi",    "from": [-0.6, 4.5, -1.45], "to": [0.6, 5.5, 1.45], "group": "valve_lumen", "material": "qi_glow"})

    elif variant == "closed":
        # 闭死：收到中心只剩 2x2 #5a1a1a 暗色小孔 (x in [-1, 1], y in [4, 6])
        cubes.append({"name": f"{pfx}closed_flesh_top", "from": [-4.0, 6.0, -1.5], "to": [ 4.0, 8.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}closed_flesh_bot", "from": [-4.0, 2.0, -1.5], "to": [ 4.0, 4.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}closed_flesh_l",   "from": [-4.0, 4.0, -1.5], "to": [-1.0, 6.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}closed_flesh_r",   "from": [ 1.0, 4.0, -1.5], "to": [ 4.0, 6.0, 1.5], "group": "valve_lumen", "material": "flesh_main"})
        # 2x2 闭锁孔暗面
        cubes.append({"name": f"{pfx}closed_hole_dark", "from": [-1.0, 4.0, -1.5], "to": [ 1.0, 6.0, 1.5], "group": "valve_lumen", "material": "flesh_dark"})

    return cubes


def part_04_top_bone_wheel(variant: str = "open") -> List[dict]:
    """环顶小骨轮 (复用 common_parts.bone_wheel)：
    - 安装立轴：#b8a888 bone_dark (y in [9.8, 10.5], 截面 1.2x1.2)；
    - 骨轮主体：6x1x6 外廓，y=11.0，十字辐条 + 外圆环。
    """
    cubes = []
    pfx = f"{variant}_"

    # 支撑立轴 (#b8a888 bone_dark)
    cubes.append({
        "name": f"{pfx}wheel_shaft",
        "from": [-0.6, 9.8, -0.6],
        "to":   [ 0.6, 10.5, 0.6],
        "group": "bone_wheel",
        "material": "bone_dark",
    })

    # 6x1x6 骨轮 (复用 common_parts.bone_wheel)
    cubes.extend(bone_wheel(center_x=0.0, center_y=11.0, center_z=0.0, radius=3.0, thickness=1.0, axis="y", pfx=f"{pfx}wheel"))

    return cubes


def all_cubes(variant: str = "open") -> List[dict]:
    """汇总指定变体的所有立方体。"""
    return (
        part_01_pipe_and_collars(variant)
        + part_02_sphincter_ring(variant)
        + part_03_valve_lumen(variant)
        + part_04_top_bone_wheel(variant)
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
    """输出三变体（3/4 等轴视 + 正对管口直视）的综合拼图 render.png 与爆炸图并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    variants = ["open", "half", "closed"]
    renders = {}

    for v in variants:
        p = paths[v]
        # 3/4 等轴视 (看整体括约肌环、顶置小骨轮、筋管)
        im_iso, _ = render(p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
        # 正对管口直视 (yaw=0.0, pitch=0.0，从 -Z 正视管口内孔开闭)
        im_front, _ = render(p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
        renders[v] = (im_iso, im_front)

    # 1. 拼装 render.png (2 行 3 列：上行三变体 3/4 视，下行三变体正对管口直视)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 3 + 40
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    titles = [
        ("valve_open (3/4 Isometric View)", renders["open"][0], 10, 10),
        ("valve_half (3/4 Isometric View)", renders["half"][0], cell_w + 20, 10),
        ("valve_closed (3/4 Isometric View)", renders["closed"][0], cell_w * 2 + 30, 10),
        ("valve_open (FRONT VIEW: Full 8x6 Lumen)", renders["open"][1], 10, cell_h + 20),
        ("valve_half (FRONT VIEW: Constricted 4x2 Lumen)", renders["half"][1], cell_w + 20, cell_h + 20),
        ("valve_closed (FRONT VIEW: 2x2 Sealed Hole)", renders["closed"][1], cell_w * 2 + 30, cell_h + 20),
    ]

    for title, im_v, px, py in titles:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 440, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (三变体 3/4视 + 正对管口 2x3 拼版) 已输出: {render_path}")

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
            "from": [-4.0, 2.0, -8.0],
            "to":   [ 4.0, 8.0, -1.5],
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
