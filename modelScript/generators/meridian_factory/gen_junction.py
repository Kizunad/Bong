#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b10: junction (经脉三通 / 四通) [Round 3 终轮几何收尾]

风格：A 有机型 (活体血肉、暖粉半透明筋管、两端肉箍小骨环、中心肉质枢纽、交叉筋丝)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md
- 爆炸参考图：model-review/img/meridian_factory/exploded/x10_junction.png
- 调度审第 2 批 Round 3 指令：
  * 遗留：整体偏方块；
  * 改法：中间枢纽的 8 条竖棱和顶面 4 条棱各削 1px 倒角；
  * 三条管臂和枢纽之间的肉箍各加 1px 外鼓；
  * 出 junction/check.png，commit (round 3/3) + PROMISE 块；
  * 不写 manifest，不碰 bug，不杀别人进程。
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
    """中心肉质枢纽 (Round 3 削角圆润化)：
    - 中间枢纽 8 条竖棱削 1px 倒角：切除四个直角棱角，形成平滑过渡的八边形柱体；
    - 顶面 4 条棱削 1px 倒角：在 y 8–9 处四周各向内收缩，顶面由台阶与中心圆穹平台构成穹顶过渡；
    - 顶面架设 #c07868 十字筋丝 (浮起 0.4px) 与中心亮结；
    - 内部 #f6dcc4 真元内光核心；
    - T 形在南面 (+Z) 加 #5a1a1a 盲端凹点。
    """
    cubes = []
    pfx = "x_" if is_cross else "t_"

    # 1. 枢纽中下段八边形柱体 (Y in [1.0, 8.0])
    # 核心中心柱
    cubes.append({
        "name": f"{pfx}hub_core_center",
        "from": [-1.4, 1.0, -1.4],
        "to":   [ 1.4, 8.0,  1.4],
        "group": "central_hub",
        "material": "flesh_main",
    })
    # 四向主凸缘 (北南与东西各延伸到 +/-2.0，宽 2.8px)
    cubes.append({
        "name": f"{pfx}hub_core_n",
        "from": [-1.4, 1.0, -2.0],
        "to":   [ 1.4, 8.0, -1.4],
        "group": "central_hub",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}hub_core_s",
        "from": [-1.4, 1.0,  1.4],
        "to":   [ 1.4, 8.0,  2.0],
        "group": "central_hub",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}hub_core_w",
        "from": [-2.0, 1.0, -1.4],
        "to":   [-1.4, 8.0,  1.4],
        "group": "central_hub",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}hub_core_e",
        "from": [ 1.4, 1.0, -1.4],
        "to":   [ 2.0, 8.0,  1.4],
        "group": "central_hub",
        "material": "flesh_main",
    })

    # 四个切角斜棱小柱 (切除四个外直角，8 条竖棱平滑八边形过渡)
    cubes.append({"name": f"{pfx}hub_bevel_nw", "from": [-1.8, 1.0, -1.8], "to": [-1.4, 8.0, -1.4], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_bevel_ne", "from": [ 1.4, 1.0, -1.8], "to": [ 1.8, 8.0, -1.4], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_bevel_sw", "from": [-1.8, 1.0,  1.4], "to": [-1.4, 8.0,  1.8], "group": "central_hub", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}hub_bevel_se", "from": [ 1.4, 1.0,  1.4], "to": [ 1.8, 8.0,  1.8], "group": "central_hub", "material": "flesh_main"})

    # 2. 顶面 4 条棱削角段 (Y in [8.0, 9.0]，四周各削 1px 倒角收缩)
    cubes.append({
        "name": f"{pfx}hub_top_step",
        "from": [-1.6, 8.0, -1.6],
        "to":   [ 1.6, 8.4,  1.6],
        "group": "central_hub",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}hub_top_crown",
        "from": [-1.0, 8.4, -1.0],
        "to":   [ 1.0, 9.0,  1.0],
        "group": "central_hub",
        "material": "flesh_main",
    })

    # 3. 顶面十字筋丝 (Y in [9.0, 9.35])
    cubes.append({
        "name": f"{pfx}hub_cross_x",
        "from": [-0.9, 9.0, -0.3],
        "to":   [ 0.9, 9.35, 0.3],
        "group": "central_hub",
        "material": "tendon_fiber",
    })
    cubes.append({
        "name": f"{pfx}hub_cross_z_n",
        "from": [-0.3, 9.0, -0.9],
        "to":   [ 0.3, 9.35, -0.3],
        "group": "central_hub",
        "material": "tendon_fiber",
    })
    cubes.append({
        "name": f"{pfx}hub_cross_z_s",
        "from": [-0.3, 9.0,  0.3],
        "to":   [ 0.3, 9.35,  0.9],
        "group": "central_hub",
        "material": "tendon_fiber",
    })
    cubes.append({
        "name": f"{pfx}hub_cross_ctr",
        "from": [-0.35, 9.32, -0.35],
        "to":   [ 0.35, 9.42,  0.35],
        "group": "central_hub",
        "material": "tendon_highlight",
    })

    # 4. 内部真元内光核心
    cubes.append({
        "name": f"{pfx}hub_qi_glow",
        "from": [-0.8, 4.0, -0.8],
        "to":   [ 0.8, 6.0,  0.8],
        "group": "central_hub",
        "material": "qi_glow",
    })

    # 5. T 形南面 (+Z): 枢纽 +Z 面加 #5a1a1a 盲端凹点
    if not is_cross:
        cubes.append({
            "name": "t_hub_blind_dot",
            "from": [-0.8, 4.0, 1.9],
            "to":   [ 0.8, 6.0, 2.05],
            "group": "central_hub",
            "material": "flesh_dark",
        })

    return cubes


def part_02_arms_and_interfaces(is_cross: bool = False) -> List[dict]:
    """生成管臂与枢纽连接外鼓肉箍、分支细管与统一接口段：
    - 管臂与枢纽之间的肉箍各加 1px 外鼓：高 Y in [2.0, 8.0] (比细管 3..7 上下各鼓 1px)，横向外展至 3.2px；
    - 细臂截面 6×4 (y 3–7, 居中)，长约 2.8px 露出暖粉筋管；
    - 边缘 0–2 (2px): 放大到 8×6 统一接口 + 2px 肉箍 + 骨环端口贴在肉箍顶面。
    """
    cubes = []
    pfx = "x_" if is_cross else "t_"

    # ═════════════════════════════════════════════════════════════════════════
    # A. 根部外鼓肉箍 (环绕枢纽，比 6x4 细管四周各加 1px 外鼓，采用无共面九宫格切分)
    # ═════════════════════════════════════════════════════════════════════════
    # 1. 北臂根部肉箍 (Z in [-3.2, -2.0], 高 Y in [2.0, 8.0])
    cubes.append({
        "name": f"{pfx}col_n_mid",
        "from": [-2.0, 2.0, -3.2],
        "to":   [ 2.0, 8.0, -2.0],
        "group": "arms",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}col_n_wrinkle",
        "from": [-1.9, 1.95, -2.8],
        "to":   [ 1.9, 8.05, -2.4],
        "group": "arms",
        "material": "flesh_dark",
    })

    # 2. 西臂根部肉箍 (X in [-3.2, -2.0], 高 Y in [2.0, 8.0])
    cubes.append({
        "name": f"{pfx}col_w_mid",
        "from": [-3.2, 2.0, -2.0],
        "to":   [-2.0, 8.0,  2.0],
        "group": "arms",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}col_w_wrinkle",
        "from": [-2.8, 1.95, -1.9],
        "to":   [-2.4, 8.05,  1.9],
        "group": "arms",
        "material": "flesh_dark",
    })

    # 3. 东臂根部肉箍 (X in [2.0, 3.2], 高 Y in [2.0, 8.0])
    cubes.append({
        "name": f"{pfx}col_e_mid",
        "from": [ 2.0, 2.0, -2.0],
        "to":   [ 3.2, 8.0,  2.0],
        "group": "arms",
        "material": "flesh_main",
    })
    cubes.append({
        "name": f"{pfx}col_e_wrinkle",
        "from": [ 2.4, 1.95, -1.9],
        "to":   [ 2.8, 8.05,  1.9],
        "group": "arms",
        "material": "flesh_dark",
    })

    # 4. 四个外鼓角块 (高度 Y in [2.2, 7.8]，圆润肌肉过渡，比管身 3..7 上下外鼓近 1px)
    cubes.append({"name": f"{pfx}col_corner_nw", "from": [-3.2, 2.2, -3.2], "to": [-2.0, 7.8, -2.0], "group": "arms", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}col_corner_ne", "from": [ 2.0, 2.2, -3.2], "to": [ 3.2, 7.8, -2.0], "group": "arms", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}col_corner_sw", "from": [-3.2, 2.2,  2.0], "to": [-2.0, 7.8,  3.2], "group": "arms", "material": "flesh_main"})
    cubes.append({"name": f"{pfx}col_corner_se", "from": [ 2.0, 2.2,  2.0], "to": [ 3.2, 7.8,  3.2], "group": "arms", "material": "flesh_main"})

    if is_cross:
        # 南臂根部肉箍 (Z in [2.0, 3.2], 高 Y in [2.0, 8.0])
        cubes.append({
            "name": f"{pfx}col_s_mid",
            "from": [-2.0, 2.0,  2.0],
            "to":   [ 2.0, 8.0,  3.2],
            "group": "arms",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"{pfx}col_s_wrinkle",
            "from": [-1.9, 1.95,  2.4],
            "to":   [ 1.9, 8.05,  2.8],
            "group": "arms",
            "material": "flesh_dark",
        })

    # ═════════════════════════════════════════════════════════════════════════
    # B. 分支细管 (6x4 截面) 与 方块边 8x6 统一接口 + 肉箍
    # ═════════════════════════════════════════════════════════════════════════
    dirs = ["n", "s", "w", "e"] if is_cross else ["n", "w", "e"]

    for d in dirs:
        if d == "n":  # -Z 方向 (北)
            # 6x4 细管主体 (Z in [-6.0, -3.2], X in [-3, 3], Y in [3, 7])
            cubes.append({"name": f"{pfx}arm_n_6x4", "from": [-3.0, 3.0, -6.0], "to": [3.0, 7.0, -3.2], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_n_hl_bot", "from": [-2.9, 2.95, -5.95], "to": [2.9, 3.5, -3.25], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_n_hl_top", "from": [-2.9, 6.5,  -5.95], "to": [2.9, 7.05, -3.25], "group": "arms", "material": "tendon_highlight"})

            # 8x6 统一接口短管 (Z in [-8, -6])
            cubes.append({"name": f"{pfx}inlet_n_tube", "from": [-4.0, 2.0, -8.0], "to": [4.0, 8.0, -6.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_n_hl_b", "from": [-3.9, 1.95, -7.95], "to": [3.9, 2.7, -6.05], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_n_hl_t", "from": [-3.9, 7.3,  -7.95], "to": [3.9, 8.05, -6.05], "group": "interface", "material": "tendon_highlight"})

            # 贯穿真元光芯
            cubes.append({"name": f"{pfx}arm_n_qi", "from": [-0.8, 4.5, -7.95], "to": [0.8, 5.5, -2.05], "group": "arms", "material": "qi_glow"})

            # 方块边肉箍与骨环端口 (复用 common_parts)
            cubes.extend(flesh_collar(center_pos=-6.5, length=2.0, axis="z", pfx=f"{pfx}collar_n", group="interface"))
            cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=-6.5, face="up", pfx=f"{pfx}port_n", group="interface"))

        elif d == "s":  # +Z 方向 (南，仅十字四通拥有)
            cubes.append({"name": f"{pfx}arm_s_6x4", "from": [-3.0, 3.0, 3.2], "to": [3.0, 7.0, 6.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_s_hl_bot", "from": [-2.9, 2.95, 3.25], "to": [2.9, 3.5, 5.95], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_s_hl_top", "from": [-2.9, 6.5,  3.25], "to": [2.9, 7.05, 5.95], "group": "arms", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}inlet_s_tube", "from": [-4.0, 2.0, 6.0], "to": [4.0, 8.0, 8.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_s_hl_b", "from": [-3.9, 1.95, 6.05], "to": [3.9, 2.7, 7.95], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_s_hl_t", "from": [-3.9, 7.3,  6.05], "to": [3.9, 8.05, 7.95], "group": "interface", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_s_qi", "from": [-0.8, 4.5, 2.05], "to": [0.8, 5.5, 7.95], "group": "arms", "material": "qi_glow"})

            cubes.extend(flesh_collar(center_pos=6.5, length=2.0, axis="z", pfx=f"{pfx}collar_s", group="interface"))
            cubes.extend(bone_ring_port(center_x=0.0, center_y=9.0, center_z=6.5, face="up", pfx=f"{pfx}port_s", group="interface"))

        elif d == "w":  # -X 方向 (西)
            cubes.append({"name": f"{pfx}arm_w_6x4", "from": [-6.0, 3.0, -3.0], "to": [-3.2, 7.0, 3.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_w_hl_bot", "from": [-5.95, 2.95, -2.9], "to": [-3.25, 3.5, 2.9], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_w_hl_top", "from": [-5.95, 6.5,  -2.9], "to": [-3.25, 7.05, 2.9], "group": "arms", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}inlet_w_tube", "from": [-8.0, 2.0, -4.0], "to": [-6.0, 8.0, 4.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_w_hl_b", "from": [-7.95, 1.95, -3.9], "to": [-6.05, 2.7, 3.9], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_w_hl_t", "from": [-7.95, 7.3,  -3.9], "to": [-6.05, 8.05, 3.9], "group": "interface", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}arm_w_qi", "from": [-7.95, 4.5, -0.8], "to": [-2.05, 5.5, 0.8], "group": "arms", "material": "qi_glow"})

            cubes.extend(flesh_collar(center_pos=-6.5, length=2.0, axis="x", pfx=f"{pfx}collar_w", group="interface"))
            cubes.extend(bone_ring_port(center_x=-6.5, center_y=9.0, center_z=0.0, face="up", pfx=f"{pfx}port_w", group="interface"))

        elif d == "e":  # +X 方向 (东)
            cubes.append({"name": f"{pfx}arm_e_6x4", "from": [ 3.2, 3.0, -3.0], "to": [ 6.0, 7.0, 3.0], "group": "arms", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}arm_e_hl_bot", "from": [ 3.25, 2.95, -2.9], "to": [ 5.95, 3.5, 2.9], "group": "arms", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}arm_e_hl_top", "from": [ 3.25, 6.5,  -2.9], "to": [ 5.95, 7.05, 2.9], "group": "arms", "material": "tendon_highlight"})

            cubes.append({"name": f"{pfx}inlet_e_tube", "from": [ 6.0, 2.0, -4.0], "to": [ 8.0, 8.0, 4.0], "group": "interface", "material": "tendon_tube"})
            cubes.append({"name": f"{pfx}inlet_e_hl_b", "from": [ 6.05, 1.95, -3.9], "to": [ 7.95, 2.7, 3.9], "group": "interface", "material": "tendon_highlight"})
            cubes.append({"name": f"{pfx}inlet_e_hl_t", "from": [ 6.05, 7.3,  -3.9], "to": [ 7.95, 8.05, 3.9], "group": "interface", "material": "tendon_highlight"})

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
        ("junction_t (3/4 ISOMETRIC VIEW - Round 3 Polish)", im_t_iso, 10, 10),
        ("junction_x (3/4 ISOMETRIC VIEW - Round 3 Polish)", im_x_iso, cell_w + 20, 10),
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

    # 2. 拼装 check.png (左为 x10 爆炸参考图，右为两变体 3/4 等轴视大图并排对标)
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

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: junction_t 3-way, RIGHT: junction_x 4-way Cross - Round 3)", fill=(180, 220, 210))

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
    parser = argparse.ArgumentParser(description="经脉工厂内景 b10 junction 生成器 (Round 3)")
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
