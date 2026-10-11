#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b11: valve (经脉阀门，含 open / half / closed 三变体) [调度审第 3 次重做版]

风格：A 有机型 (活体血肉、暖粉半透明筋管、正中显眼括约肌环、顶置骨柄与骨轮、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md (第 2 批表格与规范)
- 爆炸参考图：model-review/img/meridian_factory/exploded/x11_valve.png
- b11 调度审第 1 次 / 第 2 次指示：三变体要在 3/4 视角和正对管口两个角度都能一眼分出开度

pi-a 两次返工后三变体仍然「看起来完全一样」，读过现场渲染和渲染器源码后找到两个真实的
工程 bug（不是参数没调对，是渲染管线本身把差异抹掉了）：

1. **`render()` 不传 `focus` 时逐图各自取景**：每张图按自己的包围盒独立居中缩放去填满
   画布。三个变体即使实际尺寸不同（12×10 vs 13×11 vs 14×12），各自填满同一张 500px
   画布后在屏幕上看起来**完全一样大**——这正是调度两轮看到的"三个变体几乎一样"。
   本版在 `_shared_focus()` 里对三个变体的 bbmodel 取并集包围盒，算出一个公共
   `(center, span)` 喂给所有渲染调用，让"谁更大"这件事能被看见。
2. **"正对管口"直视被自己的前段筋管挡住**：`sinew_tube` 生成的是实心立方体（不是空心
   管），从 -Z 轴正对拍过去，相机看到的第一片不透明表面永远是肉箍后面那截前段筋管的
   端面——而那截筋管在三个变体里形状完全相同，根本拍不到后面的括约肌环和内孔。
   本版新增 `port_view_cubes()`：只取括约肌环 + 内孔两个部件（不含筋管/肉箍），临时
   导出一个裁剪版 bbmodel 专门拍"正对管口"这张，相机才真正看得到 8×6 / 4×3 / 2×2
   的孔径差异。两张图分工明确：`render.png` 第一行是完整装配的 3/4 视，第二行是裁剪
   出的括约肌环正视。

在上述两处修好之后，再把环的尺寸级差从 b11 调度审第 1 次给的 "+1px/+1px" 放大到
"+1.6px/+1.2px" 左右（仍在 16×16×16 方块内留足顶置骨轮的净空），双重保险避免级差太小
在画面上读不出来。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import tempfile
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
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    sinew_tube,
    flesh_collar,
    bone_ring_port,
    bone_wheel,
)

VARIANTS = ("open", "half", "closed")

# 括约肌环尺寸表：z_half 决定厚度 (z in [-z_half, z_half])，r_x/r_y 决定外廓半宽/半高。
# 比 b11 调度审第 1 次给的 "+1px/+1px" 级差更大，确保三变体在共享取景下目视可辨。
# r_y 上限受顶置骨柄 (2px) + 骨轮 (厚 1px) 净空约束：r_y + 3.0 必须 < 16 (方块顶)。
RING_SPEC: Dict[str, Dict[str, float]] = {
    "open":   {"z_half": 2.0, "r_x": 6.0, "r_y": 10.0},
    "half":   {"z_half": 2.8, "r_x": 7.0, "r_y": 11.6},
    "closed": {"z_half": 3.6, "r_x": 7.8, "r_y": 12.8},
}

# 内孔开度表 (宽, 高)，居中于 y=5.0 (管腔纵向中点)。按统一截面约定：open = 8x6 满开。
HOLE_SPEC: Dict[str, Tuple[float, float]] = {
    "open":   (8.0, 6.0),
    "half":   (4.0, 3.0),
    "closed": (2.0, 2.0),
}


# =============================================================================
# 各部件几何定义 (part_* 拆分并复用 common_parts)
# =============================================================================

def part_01_pipe_and_collars(variant: str) -> List[dict]:
    """生成前后管段、两端改窄肉箍与贴面骨环端口。

    两端肉箍改窄为 2px 长 (调度审第 1 次要求)，筋管在肉箍与括约肌环之间大面积露出。
    """
    cubes = []
    pfx = f"{variant}_"
    z_half = RING_SPEC[variant]["z_half"]

    # 1. 前后露出筋管段 (复用 common_parts.sinew_tube)
    cubes.extend(sinew_tube(-6.0, -z_half, axis="z", with_inner_qi=True, with_highlights=True, pfx=f"{pfx}tube_rear"))
    cubes.extend(sinew_tube(z_half, 6.0, axis="z", with_inner_qi=True, with_highlights=True, pfx=f"{pfx}tube_front"))

    # 2. 两端改窄肉箍 (长 2px: center=-7.0 与 center=7.0)
    cubes.extend(flesh_collar(center_pos=-7.0, length=2.0, axis="z", pfx=f"{pfx}collar_rear"))
    cubes.extend(flesh_collar(center_pos=7.0, length=2.0, axis="z", pfx=f"{pfx}collar_front"))

    # 3. 肉箍顶面与底面骨环端口
    cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=-7.0, face="up", pfx=f"{pfx}port_r_t"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=1.0, center_z=-7.0, face="down", pfx=f"{pfx}port_r_b"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=7.0, face="up", pfx=f"{pfx}port_f_t"))
    cubes.extend(bone_ring_port(center_x=0.0, center_y=1.0, center_z=7.0, face="down", pfx=f"{pfx}port_f_b"))

    return cubes


def part_02_sphincter_ring(variant: str) -> List[dict]:
    """生成正中括约肌环与表面 6 条环向褶纹。

    外廓随 open → half → closed 逐级鼓大 (RING_SPEC)，厚度同步增长，八角切边形成
    圆钝肌环轮廓；表面 #b05050 / #5a1a1a 交替环向褶各浮起 0.8~1.0px。
    """
    cubes = []
    pfx = f"{variant}_"
    spec = RING_SPEC[variant]
    z_min, z_max = -spec["z_half"], spec["z_half"]
    r_x, r_y = spec["r_x"], spec["r_y"]

    # 1. 环体四段无交集分区外框 (#8a2a2a flesh_main)
    cubes.append({"name": f"{pfx}sph_bot_mid", "from": [-4.0, 0.0, z_min], "to": [4.0, 2.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_top_mid", "from": [-4.0, 8.0, z_min], "to": [4.0, r_y, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_l_mid", "from": [-r_x, 2.0, z_min], "to": [-4.0, 8.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_r_mid", "from": [4.0, 2.0, z_min], "to": [r_x, 8.0, z_max], "group": "sphincter", "material": "flesh_main"})

    # 2. 四角倒角块 (外切角 1x1，形成圆钝肌环)
    cubes.append({"name": f"{pfx}sph_c_bl", "from": [-r_x + 1.0, 1.0, z_min], "to": [-4.0, 2.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_c_br", "from": [4.0, 1.0, z_min], "to": [r_x - 1.0, 2.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_c_tl", "from": [-r_x + 1.0, 8.0, z_min], "to": [-4.0, r_y - 1.0, z_max], "group": "sphincter", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}sph_c_tr", "from": [4.0, 8.0, z_min], "to": [r_x - 1.0, r_y - 1.0, z_max], "group": "sphincter", "material": "flesh_main"})

    # 3. 表面 6 条 #b05050 / #5a1a1a 交替环向褶 (前端面 3 条 + 后端面 3 条)
    cubes.append({"name": f"{pfx}fold_f1_top", "from": [-r_x + 1.8, r_y - 1.5, z_min - 0.8], "to": [r_x - 1.8, r_y - 0.5, z_min], "group": "sphincter_folds", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}fold_f2_l", "from": [-r_x + 0.5, 3.5, z_min - 0.8], "to": [-r_x + 1.5, 7.5, z_min], "group": "sphincter_folds", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}fold_f3_r", "from": [r_x - 1.5, 3.5, z_min - 0.8], "to": [r_x - 0.5, 7.5, z_min], "group": "sphincter_folds", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}fold_b1_top", "from": [-r_x + 1.8, r_y - 1.5, z_max], "to": [r_x - 1.8, r_y - 0.5, z_max + 0.8], "group": "sphincter_folds", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}fold_b2_l", "from": [-r_x + 0.5, 3.5, z_max], "to": [-r_x + 1.5, 7.5, z_max + 0.8], "group": "sphincter_folds", "material": "flesh_lit"})
    cubes.append({"name": f"{pfx}fold_b3_r", "from": [r_x - 1.5, 3.5, z_max], "to": [r_x - 0.5, 7.5, z_max + 0.8], "group": "sphincter_folds", "material": "flesh_dark"})

    return cubes


def part_03_valve_lumen(variant: str) -> List[dict]:
    """生成三变体内孔开闭形态 (在括约肌环厚度范围内)。

    open: 内孔 8×6，与管腔完全贯通；half: 收缩至 4×3；closed: 收到中心 2×2 暗孔，
    孔内壁统一 #5a1a1a。宽高取自 HOLE_SPEC，居中于管腔纵向中点 y=5.0。
    """
    cubes = []
    pfx = f"{variant}_"
    spec = RING_SPEC[variant]
    z_min, z_max = -spec["z_half"], spec["z_half"]
    hole_w, hole_h = HOLE_SPEC[variant]
    hx, hy = hole_w / 2.0, hole_h / 2.0
    cy = 5.0

    if variant == "open":
        # 8x6 贯通暖粉管腔 + 2x2 真元光芯
        cubes.append({"name": f"{pfx}lumen_tube", "from": [-hx, cy - hy, z_min], "to": [hx, cy + hy, z_max], "group": "valve_lumen", "material": "tendon_tube"})
        cubes.append({"name": f"{pfx}lumen_qi", "from": [-1.0, 4.0, z_min + 0.05], "to": [1.0, 6.0, z_max - 0.05], "group": "valve_lumen", "material": "qi_glow"})
    else:
        # 四周收缩/紧闭的深红肌肉层 (#8a2a2a flesh_main)，中心留 hole_w x hole_h 的暗孔
        cubes.append({"name": f"{pfx}con_bot", "from": [-4.0, 2.0, z_min], "to": [4.0, cy - hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_top", "from": [-4.0, cy + hy, z_min], "to": [4.0, 8.0, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_l", "from": [-4.0, cy - hy, z_min], "to": [-hx, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_r", "from": [hx, cy - hy, z_min], "to": [4.0, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_main"})
        cubes.append({"name": f"{pfx}con_hole", "from": [-hx, cy - hy, z_min], "to": [hx, cy + hy, z_max], "group": "valve_lumen", "material": "flesh_dark"})

    return cubes


def part_04_stem_and_wheel(variant: str) -> List[dict]:
    """骨轮 + 骨柄：轮下一根 2×2×2 骨柄连到环，骨轮贴环顶正中。"""
    cubes = []
    pfx = f"{variant}_"
    r_y = RING_SPEC[variant]["r_y"]

    cubes.append({
        "name": f"{pfx}bone_stem",
        "from": [-1.0, r_y, -1.0],
        "to": [1.0, r_y + 2.0, 1.0],
        "group": "bone_wheel",
        "material": "bone_dark",
    })
    cubes.extend(bone_wheel(center_x=0.0, center_y=r_y + 2.5, center_z=0.0, radius=3.0, thickness=1.0, axis="y", pfx=f"{pfx}wheel"))

    return cubes


def all_cubes(variant: str) -> List[dict]:
    """汇总指定变体的完整装配立方体 (管段 + 肉箍 + 括约肌环 + 内孔 + 骨柄骨轮)。"""
    return (
        part_01_pipe_and_collars(variant)
        + part_02_sphincter_ring(variant)
        + part_03_valve_lumen(variant)
        + part_04_stem_and_wheel(variant)
    )


def port_view_cubes(variant: str) -> List[dict]:
    """只取括约肌环 + 内孔 (不含筋管/肉箍)，专供"正对管口"裁剪视角渲染。

    完整装配沿 Z 轴正对拍摄时，镜头看到的第一片不透明表面永远是肉箍后方那截前段
    筋管的端面——三变体那截筋管形状相同，根本拍不到后面的孔径差异。这个子集把挡镜头
    的筋管/肉箍去掉，只留下孔径真正变化的部件，让"正对管口"这张图名副其实。
    """
    return part_02_sphincter_ring(variant) + part_03_valve_lumen(variant)


# =============================================================================
# 导出与渲染
# =============================================================================

def _cubes_to_bbmodel(cubes: List[dict], name: str) -> dict:
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
        faces = {side: {"uv": uv, "texture": 0} for side in ["north", "south", "east", "west", "up", "down"]}
        elements.append({
            "name": c["name"],
            "box_uv": False,
            "from": f,
            "to": t,
            "faces": faces,
            "uuid": str(uuid.uuid4()),
        })

    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "valve")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [{"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list} for g, u_list in groups_map.items()]

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": name,
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [{"name": name, "folder": "block", "namespace": "bong", "id": 0, "source": tex_base64}],
    }


def generate_bbmodel(cubes: List[dict], name: str, out_path: Path) -> Path:
    """校验共面并写出指定立方体集合的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    _assert_no_coplanar_faces(cubes)
    bbmodel = _cubes_to_bbmodel(cubes, name)
    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    try:
        rel = out_path.relative_to(REPO)
    except ValueError:
        rel = out_path
    print(f"✓ {name} bbmodel 写入成功: {rel}")
    return out_path


def _shared_focus(paths: List[Path], yaw_pitch_list: List[Tuple[float, float]], margin: float = 1.08):
    """对多个 bbmodel 文件取并集包围盒，算出一组视角下的公共 (center, span)。

    render() 不传 focus 时逐图各自取景——三个变体各自填满同一张画布后在屏幕上看起来
    一样大，实际尺寸差异被"自动适配"抹平了。本函数用同一把尺子量三个变体，谁更大
    谁就在画面里占得更多，比较才有意义。
    """
    from bbmodel_maker.render import framing

    lo = hi = None
    for p in paths:
        l, h = framing.model_bounds(p)
        lo = l if lo is None else np.minimum(lo, l)
        hi = h if hi is None else np.maximum(hi, h)

    center = (lo + hi) / 2.0
    corners = np.array(
        [[lo[0] if i & 1 else hi[0], lo[1] if i & 2 else hi[1], lo[2] if i & 4 else hi[2]] for i in range(8)],
        float,
    ) - center

    span = 0.0
    for yaw, pitch in yaw_pitch_list:
        v = framing.View(name="_", yaw=yaw, pitch=pitch, shows="")
        proj = (framing.view_matrix(v) @ corners.T).T[:, :2]
        span = max(span, float((proj.max(0) - proj.min(0)).max()))
    span = max(span * margin, 1e-6)
    return (float(center[0]), float(center[1]), float(center[2])), span


def render_views(paths: Dict[str, Path]):
    """输出两行三列综合拼图 render.png (3/4 装配视 + 正对管口裁剪视) 与爆炸图对照卡 check.png。

    两行视角都用各自的共享 focus 渲染——这是让三变体差异真正"看得见"的关键，不是
    render() 的默认逐图自动取景。
    """
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    full_paths = [paths[v] for v in VARIANTS]
    iso_angle = (-35.0, 30.0)
    focus_iso = _shared_focus(full_paths, [iso_angle])

    with tempfile.TemporaryDirectory() as tmp:
        tmp_dir = Path(tmp)
        port_paths = {}
        for v in VARIANTS:
            p = generate_bbmodel(port_view_cubes(v), f"valve_{v}_port_view", tmp_dir / f"valve_{v}_port_view.bbmodel")
            port_paths[v] = p

        front_angle = (0.0, 0.0)
        focus_front = _shared_focus([port_paths[v] for v in VARIANTS], [front_angle])

        renders_iso = {}
        renders_front = {}
        for v in VARIANTS:
            im_iso, _ = render(paths[v], yaw=iso_angle[0], pitch=iso_angle[1], size=500, bg=bg_color, focus=focus_iso)
            im_front, _ = render(port_paths[v], yaw=front_angle[0], pitch=front_angle[1], size=500, bg=bg_color, focus=focus_front)
            renders_iso[v] = im_iso
            renders_front[v] = im_front

        # 拼装 render.png (2 行 3 列：行 1 完整装配 3/4 视共享取景，行 2 括约肌环正对管口裁剪视共享取景)
        cell_w, cell_h = 500, 500
        canvas_w = cell_w * 3 + 40
        canvas_h = cell_h * 2 + 30
        canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
        draw = ImageDraw.Draw(canvas)

        ring_labels = {
            "open": f"Ring {RING_SPEC['open']['r_x']*2:g}x{RING_SPEC['open']['r_y']:g}",
            "half": f"Ring {RING_SPEC['half']['r_x']*2:g}x{RING_SPEC['half']['r_y']:g} (+swollen)",
            "closed": f"Ring {RING_SPEC['closed']['r_x']*2:g}x{RING_SPEC['closed']['r_y']:g} (max swollen)",
        }
        hole_labels = {
            "open": "Lumen 8x6 FULL OPEN",
            "half": "Lumen 4x3 CONSTRICTED",
            "closed": "Lumen 2x2 SEALED (dark hole)",
        }

        titles = [
            (f"valve_open (3/4 ASSEMBLY: {ring_labels['open']})", renders_iso["open"], 10, 10),
            (f"valve_half (3/4 ASSEMBLY: {ring_labels['half']})", renders_iso["half"], cell_w + 20, 10),
            (f"valve_closed (3/4 ASSEMBLY: {ring_labels['closed']})", renders_iso["closed"], cell_w * 2 + 30, 10),
            (f"PORT VIEW open ({hole_labels['open']}, tube/collar hidden)", renders_front["open"], 10, cell_h + 20),
            (f"PORT VIEW half ({hole_labels['half']}, tube/collar hidden)", renders_front["half"], cell_w + 20, cell_h + 20),
            (f"PORT VIEW closed ({hole_labels['closed']}, tube/collar hidden)", renders_front["closed"], cell_w * 2 + 30, cell_h + 20),
        ]

        for title, im_v, px, py in titles:
            canvas.paste(im_v, (px, py))
            draw.rectangle([px, py, px + 480, py + 26], fill=(24, 25, 28))
            draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

        render_path = REVIEW_DIR / "render.png"
        canvas.save(render_path)
        print(f"✓ render.png (3/4 共享取景装配图 + 正对管口裁剪共享取景图) 已输出: {render_path}")

        # 拼装 check.png (左为 x11 爆炸参考图，右为三变体 3/4 装配视并排对标)
        ref_image_to_use = EXPLODED_REF if EXPLODED_REF.exists() else SINGLE_REF
        if ref_image_to_use.exists():
            ref_im = Image.open(ref_image_to_use).convert("RGB")
            target_h = 550
            ref_w = int(ref_im.width * target_h / ref_im.height)
            ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

            thumb_size = 500
            c_open = renders_iso["open"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
            c_half = renders_iso["half"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
            c_closed = renders_iso["closed"].resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

            right_w = thumb_size * 3 + 24
            total_w = ref_w + right_w + 36
            check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
            c_draw = ImageDraw.Draw(check_cv)

            check_cv.paste(ref_scaled, (12, 34))
            c_draw.text((16, 10), f"EXPLODED REFERENCE ({ref_image_to_use.name})", fill=(210, 200, 180))

            rx = ref_w + 24
            check_cv.paste(c_open, (rx, 34 + 10))
            check_cv.paste(c_half, (rx + thumb_size + 8, 34 + 10))
            check_cv.paste(c_closed, (rx + (thumb_size + 8) * 2, 34 + 10))
            c_draw.text((rx + 4, 10), "NOW RENDER, shared scale (LEFT: open, MID: half, RIGHT: closed)", fill=(180, 220, 210))

            check_path = REVIEW_DIR / "check.png"
            check_cv.save(check_path)
            print(f"✓ check.png 爆炸图并排对照卡 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。同时核验内孔/环尺寸级差单调递增。"""
    print("运行 gen_valve.py 差分自证...")

    for v in VARIANTS:
        cubes = all_cubes(v)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] valve_{v} ({len(cubes)} 个立方体) 完整装配无共面冲突")

        pv_cubes = port_view_cubes(v)
        _assert_no_coplanar_faces(pv_cubes)
        print(f"  [OK] valve_{v}_port_view ({len(pv_cubes)} 个立方体) 裁剪视无共面冲突")

        defect = list(cubes) + [{
            "name": "inject_coplanar_fail",
            "from": [-4.0, 2.0, -6.0],
            "to": [4.0, 8.0, -2.0],
            "material": "tendon_tube",
        }]
        caught = False
        try:
            _assert_no_coplanar_faces(defect)
        except AssertionError as e:
            caught = True
            print(f"  [OK] valve_{v} 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")
        if not caught:
            raise RuntimeError(f"门禁失效: valve_{v} 注入共面冲突未被拦截!")

    # 环外廓 / 内孔尺寸必须随 open → half → closed 单调变化，否则"开度可辨"这件事从根上就不成立
    r_x_seq = [RING_SPEC[v]["r_x"] for v in VARIANTS]
    r_y_seq = [RING_SPEC[v]["r_y"] for v in VARIANTS]
    z_half_seq = [RING_SPEC[v]["z_half"] for v in VARIANTS]
    hole_area_seq = [HOLE_SPEC[v][0] * HOLE_SPEC[v][1] for v in VARIANTS]
    if not (r_x_seq[0] < r_x_seq[1] < r_x_seq[2]):
        raise RuntimeError(f"门禁失效: 环外廓半宽未单调递增 {r_x_seq}")
    if not (r_y_seq[0] < r_y_seq[1] < r_y_seq[2]):
        raise RuntimeError(f"门禁失效: 环外廓半高未单调递增 {r_y_seq}")
    if not (z_half_seq[0] < z_half_seq[1] < z_half_seq[2]):
        raise RuntimeError(f"门禁失效: 环厚度未单调递增 {z_half_seq}")
    if not (hole_area_seq[0] > hole_area_seq[1] > hole_area_seq[2]):
        raise RuntimeError(f"门禁失效: 内孔面积未单调递减 {hole_area_seq}")
    print(f"  [OK] 环外廓单调递增 r_x={r_x_seq} r_y={r_y_seq} z_half={z_half_seq}；内孔面积单调递减 {hole_area_seq}")

    # 方块边界门禁：环 + 骨柄 + 骨轮必须落在 16x16x16 内 (x/z in [-8,8], y in [0,16])
    for v in VARIANTS:
        spec = RING_SPEC[v]
        if spec["r_x"] > 8.0:
            raise RuntimeError(f"门禁失效: valve_{v} 环半宽 {spec['r_x']} 超出方块边界 8.0")
        wheel_top = spec["r_y"] + 3.0  # 骨柄 2px + 骨轮厚 1px
        if wheel_top > 16.0:
            raise RuntimeError(f"门禁失效: valve_{v} 骨轮顶 {wheel_top} 超出方块高度 16.0")
    print("  [OK] 三变体环宽/骨轮顶均在 16x16x16 方块边界内")

    print("✓ gen_valve.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b11 valve 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()

    paths = {}
    for v in VARIANTS:
        out_p = MODEL_DIR / f"valve_{v}.bbmodel"
        paths[v] = generate_bbmodel(all_cubes(v), f"valve_{v}", out_p)

    generate_bbmodel(all_cubes("open"), "valve", MODEL_DIR / "valve.bbmodel")

    render_views(paths)


if __name__ == "__main__":
    main()
