#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b03: port (经脉端口，含 port_in 与 port_out 两个变体)

风格：A 有机型 (活体血肉、骨环领、放射肉脊、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 参考图：model-review/img/meridian_factory/refs/b03_port.png (左入口 port_in、右出口 port_out)

规范落实：
1. 半高块尺寸契约：
   - 方块空间为 16×8×16：x in [-8.0, 8.0], y in [0.0, 8.0], z in [-8.0, 8.0]；
   - 整体高度严格不超过 8.0px (半高块)。
2. 中间圆形肉质插座与内孔：
   - 外径约 10px (x/z in [-5.0, 5.0])，材质为血肉主色 #8a2a2a (flesh_main)；
   - 内孔直径约 6px (x/z in [-3.0, 3.0])，孔径完全对齐 8×6 统一截面的经脉管腔；
   - 孔内采用深色暗血肉 #5a1a1a (flesh_dark)，深陷通底形成贯通接插井。
3. 外面一圈骨环领：
   - 环绕在插座外周，外径约 14px (x/z in [-7.0, 7.0])，宽正好 2.0px；
   - 材质采用骨色 #d8ccb0 (bone_main)，带有骨暗面 #b8a888 角爪。
4. 两个变体的 8 条放射状肉脊 (#b05050 flesh_lit)：
   - port_in (入口插座)：肉脊做成指向孔心的楔形 (外端宽 1.4px，内端收细尖锐至 0.5px，呈向心汇聚流向)；
   - port_out (出口插座)：肉脊做成指向外侧的楔形 (内端宽 1.4px，外端收细尖锐至 0.5px，呈离心发散喷涌流向)。
5. 门禁验证：_assert_no_coplanar_faces 0 共面冲突，--self-test 缺陷拦截自测全绿。
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
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/port")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b03_port.png")

PALETTE = {
    "flesh_dark":  (90, 26, 26, 255),    # #5a1a1a 暗血肉 (底座衬底 / 孔底深井)
    "flesh_main":  (138, 42, 42, 255),   # #8a2a2a 血肉 (圆形肉质插座主体)
    "bone_main":   (216, 204, 176, 255), # #d8ccb0 骨 (外圈 2px 骨环领)
    "bone_dark":   (184, 168, 136, 255), # #b8a888 骨暗面 / 角锁
    "flesh_lit":   (176, 80, 80, 255),   # #b05050 亮肉粉 (8 条放射状肉脊)
}

MAT_UV = {
    "flesh_dark":  [0, 0, 16, 16],
    "flesh_main":  [16, 0, 32, 16],
    "bone_main":   [32, 0, 48, 16],
    "bone_dark":   [48, 0, 64, 16],
    "flesh_lit":   [0, 16, 16, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_base_pad(is_in_port: bool = True) -> List[dict]:
    """底座血肉衬垫 (#5a1a1a flesh_dark)：贴地 y: 0.0..1.0，外 14x14，内空 6x6。"""
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    cubes.append({"name": f"{pfx}base_n", "from": [-7.0, 0.0, -7.0], "to": [ 7.0, 1.0, -3.0], "group": "base", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}base_s", "from": [-7.0, 0.0,  3.0], "to": [ 7.0, 1.0,  7.0], "group": "base", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}base_w", "from": [-7.0, 0.0, -3.0], "to": [-3.0, 1.0,  3.0], "group": "base", "material": "flesh_dark"})
    cubes.append({"name": f"{pfx}base_e", "from": [ 3.0, 0.0, -3.0], "to": [ 7.0, 1.0,  3.0], "group": "base", "material": "flesh_dark"})

    # 中央深井底面 (y: 0.0..0.1, 深邃暗血肉)
    cubes.append({"name": f"{pfx}port_dark_well", "from": [-3.0, 0.0, -3.0], "to": [3.0, 0.1, 3.0], "group": "flesh_socket", "material": "flesh_dark"})
    return cubes


def part_02_bone_collar(is_in_port: bool = True) -> List[dict]:
    """外面一圈骨环领：宽 2px (#d8ccb0 bone_main)，外径 14px，内径 10px，y: 1.0..3.8。"""
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    cubes.append({"name": f"{pfx}bone_n", "from": [-7.0, 1.0, -7.0], "to": [ 7.0, 3.8, -5.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bone_s", "from": [-7.0, 1.0,  5.0], "to": [ 7.0, 3.8,  7.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bone_w", "from": [-7.0, 1.0, -5.0], "to": [-5.0, 3.8,  5.0], "group": "bone_collar", "material": "bone_main"})
    cubes.append({"name": f"{pfx}bone_e", "from": [ 5.0, 1.0, -5.0], "to": [ 7.0, 3.8,  5.0], "group": "bone_collar", "material": "bone_main"})
    return cubes


def part_03_flesh_socket(is_in_port: bool = True) -> List[dict]:
    """圆形肉质插座主体 (#8a2a2a flesh_main)：外径约 10px (x/z in [-5.0, 5.0])，内孔约 6px (x/z in [-3.0, 3.0])。
    高度 y: 1.0..7.2。
    """
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    cubes.append({"name": f"{pfx}socket_n", "from": [-5.0, 1.0, -5.0], "to": [ 5.0, 7.2, -3.0], "group": "flesh_socket", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}socket_s", "from": [-5.0, 1.0,  3.0], "to": [ 5.0, 7.2,  5.0], "group": "flesh_socket", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}socket_w", "from": [-5.0, 1.0, -3.0], "to": [-3.0, 7.2,  3.0], "group": "flesh_socket", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}socket_e", "from": [ 3.0, 1.0, -3.0], "to": [ 5.0, 7.2,  3.0], "group": "flesh_socket", "material": "flesh_main"})
    return cubes


def part_04_radial_ridges(is_in_port: bool = True) -> List[dict]:
    """插座表面 8 条放射状肉脊 (#b05050 flesh_lit，浮起 y: 7.22..7.85)：
    port_in:  肉脊做成指向孔心的楔形 (外宽 1.4px，向孔心收尖至 0.5px，呈向心汇聚流向)；
    port_out: 肉脊做成指向外侧的楔形 (内端宽 1.4px，向外圈收尖至 0.5px，呈离心发散流向)。
    """
    cubes = []
    pfx = "in_" if is_in_port else "out_"

    if is_in_port:
        # ── port_in: 指向孔心 (外宽内窄) ──
        # 正北
        cubes.append({"name": f"{pfx}ridge_n_out", "from": [-0.7, 7.22, -4.95], "to": [0.7, 7.80, -3.95], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_n_in",  "from": [-0.25, 7.22, -3.95], "to": [0.25, 7.85, -3.05], "group": "ridges", "material": "flesh_lit"})
        # 正南
        cubes.append({"name": f"{pfx}ridge_s_out", "from": [-0.7, 7.22,  3.95], "to": [0.7, 7.80,  4.95], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_s_in",  "from": [-0.25, 7.22,  3.05], "to": [0.25, 7.85,  3.95], "group": "ridges", "material": "flesh_lit"})
        # 正西
        cubes.append({"name": f"{pfx}ridge_w_out", "from": [-4.95, 7.22, -0.7], "to": [-3.95, 7.80, 0.7], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_w_in",  "from": [-3.95, 7.22, -0.25], "to": [-3.05, 7.85, 0.25], "group": "ridges", "material": "flesh_lit"})
        # 正东
        cubes.append({"name": f"{pfx}ridge_e_out", "from": [ 3.95, 7.22, -0.7], "to": [ 4.95, 7.80, 0.7], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_e_in",  "from": [ 3.05, 7.22, -0.25], "to": [ 3.95, 7.85, 0.25], "group": "ridges", "material": "flesh_lit"})
        # 四斜角 (NW, NE, SW, SE: 外侧基部宽，内侧尖端细)
        cubes.append({"name": f"{pfx}ridge_nw_base", "from": [-4.6, 7.22, -4.6], "to": [-3.9, 7.75, -3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_nw_tip",  "from": [-3.9, 7.22, -3.9], "to": [-3.1, 7.85, -3.1], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_ne_base", "from": [ 3.9, 7.22, -4.6], "to": [ 4.6, 7.75, -3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_ne_tip",  "from": [ 3.1, 7.22, -3.9], "to": [ 3.9, 7.85, -3.1], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_sw_base", "from": [-4.6, 7.22,  3.9], "to": [-3.9, 7.75,  4.6], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_sw_tip",  "from": [-3.9, 7.22,  3.1], "to": [-3.1, 7.85,  3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_se_base", "from": [ 3.9, 7.22,  3.9], "to": [ 4.6, 7.75,  4.6], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_se_tip",  "from": [ 3.1, 7.22,  3.1], "to": [ 3.9, 7.85,  3.9], "group": "ridges", "material": "flesh_lit"})
    else:
        # ── port_out: 指向外侧 (内宽外窄) ──
        # 正北
        cubes.append({"name": f"{pfx}ridge_n_in",  "from": [-0.7, 7.22, -3.95], "to": [0.7, 7.80, -3.05], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_n_out", "from": [-0.25, 7.22, -4.95], "to": [0.25, 7.85, -3.95], "group": "ridges", "material": "flesh_lit"})
        # 正南
        cubes.append({"name": f"{pfx}ridge_s_in",  "from": [-0.7, 7.22,  3.05], "to": [0.7, 7.80,  3.95], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_s_out", "from": [-0.25, 7.22,  3.95], "to": [0.25, 7.85,  4.95], "group": "ridges", "material": "flesh_lit"})
        # 正西
        cubes.append({"name": f"{pfx}ridge_w_in",  "from": [-3.95, 7.22, -0.7], "to": [-3.05, 7.80, 0.7], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_w_out", "from": [-4.95, 7.22, -0.25], "to": [-3.95, 7.85, 0.25], "group": "ridges", "material": "flesh_lit"})
        # 正东
        cubes.append({"name": f"{pfx}ridge_e_in",  "from": [ 3.05, 7.22, -0.7], "to": [ 3.95, 7.80, 0.7], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_e_out", "from": [ 3.95, 7.22, -0.25], "to": [ 4.95, 7.85, 0.25], "group": "ridges", "material": "flesh_lit"})
        # 四斜角 (NW, NE, SW, SE: 内侧基部宽，外侧尖端细)
        cubes.append({"name": f"{pfx}ridge_nw_base", "from": [-3.9, 7.22, -3.9], "to": [-3.1, 7.75, -3.1], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_nw_tip",  "from": [-4.6, 7.22, -4.6], "to": [-3.9, 7.85, -3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_ne_base", "from": [ 3.1, 7.22, -3.9], "to": [ 3.9, 7.75, -3.1], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_ne_tip",  "from": [ 3.9, 7.22, -4.6], "to": [ 4.6, 7.85, -3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_sw_base", "from": [-3.9, 7.22,  3.1], "to": [-3.1, 7.75,  3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_sw_tip",  "from": [-4.6, 7.22,  3.9], "to": [-3.9, 7.85,  4.6], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_se_base", "from": [ 3.1, 7.22,  3.1], "to": [ 3.9, 7.75,  3.9], "group": "ridges", "material": "flesh_lit"})
        cubes.append({"name": f"{pfx}ridge_se_tip",  "from": [ 3.9, 7.22,  3.9], "to": [ 4.6, 7.85,  4.6], "group": "ridges", "material": "flesh_lit"})

    return cubes


def all_cubes(is_in_port: bool = True) -> List[dict]:
    """汇总指定变体所有部件的立方体。"""
    return (
        part_01_base_pad(is_in_port)
        + part_02_bone_collar(is_in_port)
        + part_03_flesh_socket(is_in_port)
        + part_04_radial_ridges(is_in_port)
    )


# =============================================================================
# 门禁与无共面面核验
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标同旋转且重叠的共面冲突。"""
    faces: Dict[Tuple[str, float, tuple], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        rot = tuple(c.get("rotation", [0, 0, 0]))
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (f[1], f[2], t[1], t[2])
            elif axis == 1:
                rect = (f[0], f[2], t[0], t[2])
            else:
                rect = (f[0], f[1], t[0], t[1])
            key = (side, round(val, 4), rot)
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val, rot), entries in faces.items():
        if len(entries) < 2:
            continue
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                u0 = max(r1[0], r2[0])
                u1 = min(r1[2], r2[2])
                v0 = max(r1[1], r2[1])
                v1 = min(r1[3], r2[3])
                if u1 - u0 > 0.001 and v1 - v0 > 0.001:
                    conflicts.append(
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}, rot={rot}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


# =============================================================================
# 贴图与 bbmodel 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if "flesh" in mat_name:
            noise = rng.randint(-12, 12, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(is_in_port: bool, out_path: Path) -> Path:
    """导出指定变体的 bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes(is_in_port)
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
        uv = MAT_UV.get(mat, [16, 0, 32, 16])
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

    name = "port_in" if is_in_port else "port_out"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "port")
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


# =============================================================================
# 渲染与并排对标卡输出
# =============================================================================

def render_views(p_in_path: Path, p_out_path: Path):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各变体视角
    # port_in 顶视 (俯瞰肉脊指向孔心) 与 3/4 视
    im_in_top, _ = render(p_in_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_in_iso, _ = render(p_in_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)

    # port_out 顶视 (俯瞰肉脊指向外侧) 与 3/4 视
    im_out_top, _ = render(p_out_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_out_iso, _ = render(p_out_path, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格：展示两个变体的顶视与 3/4 视对比)
    canvas_w = 1040
    canvas_h = 1040
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("PORT_IN (TOP: Ridges Point IN)", im_in_top, 20, 20),
        ("PORT_OUT (TOP: Ridges Point OUT)", im_out_top, 540, 20),
        ("PORT_IN (3/4 Isometric)", im_in_iso, 20, 540),
        ("PORT_OUT (3/4 Isometric)", im_out_iso, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 280, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png 已输出: {render_path}")

    # 3. 拼装 check.png (左参考图与右渲染并排)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        in_scale_w = int(im_in_top.width * target_h / im_in_top.height)
        out_scale_w = int(im_out_top.width * target_h / im_out_top.height)
        in_scaled = im_in_top.resize((in_scale_w, target_h), Image.Resampling.LANCZOS)
        out_scaled = im_out_top.resize((out_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = in_scale_w + out_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (b03_port.png: Left In, Right Out)", fill=(210, 200, 180))

        rx = ref_w + 24
        check_cv.paste(in_scaled, (rx, 30))
        check_cv.paste(out_scaled, (rx + in_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (LEFT: port_in, RIGHT: port_out)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图已输出: {check_path}")


def self_test():
    """运行门禁差分自证：两个变体正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_port.py 差分自证...")
    for is_in in [True, False]:
        name = "port_in" if is_in else "port_out"
        cubes = all_cubes(is_in)
        _assert_no_coplanar_faces(cubes)
        print(f"  [OK] {name} 正常立方体集无共面冲突")

        defect_cubes = list(cubes) + [{
            "name": "inject_coplanar_fail",
            "from": [-7.0, 0.0, -7.0],
            "to":   [ 7.0, 1.0, -3.0],
            "material": "flesh_main",
        }]
        caught = False
        try:
            _assert_no_coplanar_faces(defect_cubes)
        except AssertionError as e:
            caught = True
            print(f"  [OK] {name} 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

        if not caught:
            raise RuntimeError(f"门禁失效: {name} 注入共面冲突未被拦截!")

    print("✓ gen_port.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b03 port 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p_in = generate_bbmodel(is_in_port=True, out_path=MODEL_DIR / "port_in.bbmodel")
    p_out = generate_bbmodel(is_in_port=False, out_path=MODEL_DIR / "port_out.bbmodel")
    # 同时生成默认 port.bbmodel 作为主文件
    generate_bbmodel(is_in_port=True, out_path=MODEL_DIR / "port.bbmodel")
    render_views(p_in, p_out)


if __name__ == "__main__":
    main()
