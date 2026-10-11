#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b12: merger (合流器)

风格：A 有机型 (活体血肉、骨轨立柱、T 形传送带、中心紧凑筋壳、外置三角合流骨楔)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md「第 2 批」表格 b12 那行及「传送带与分流器的统一约定」
- 爆炸参考图：model-review/img/meridian_factory/exploded/x12_merger.png

设计说明（就是 b05 分流器反过来）：
1. 拓扑与走向：
   - -X 面 (西) 与 +X 面 (东) 两面进一条带；
   - +Z 面 (南) 出一条带 (T 形)；
   - 北壁 (-Z 面) 为封闭盲端后壁；
2. 传送带与骨轨立柱：
   - 带面宽 8px 居中，顶高 y=4 (y in [3.8, 4.0])，表面交替红肌纤维横肋 (#8a2a2a 与 #5a1a1a)；
   - 带面下方 y=0–4 是暖粉筋质底座 (#d9a08c)，侧面附斜筋丝 (#c07868)；
   - 骨轨：截面 2 宽 × 2 高，底在 y=4、顶在 y=6；
   - 立柱：截面 2×2，底在 y=4、顶在 y=8，三条带外端两侧各一根立柱 (柱面带 #b8a888 暗纹节带)，转角处设有立柱连通；
   - 骨轨外侧到方块边界留 2px 空，侧视能清晰看到筋质底座与斜筋丝；
3. 中心紧凑筋质外壳：
   - 外廓收缩到 x/z in [-3, 3] (对应 0–16 网格的 5–11 范围)、y in [0, 8]，绝不遮挡带面；
   - 顶面在 y in [7.2, 8.0] 向内微缩 0.5px 呈阶梯圆钝顶盖，带 #e8bca8 亮面与 #c07868 筋丝；
   - 北面 (-Z 面) 为盲端封闭壁，配有 #e8bca8 亮面板与 2 条斜交筋丝；
4. 合流骨楔：
   - 外壳和骨楔照 b05 已放行版，但骨楔尖改朝 +Z 出口方向；
   - 紧贴外壳南壁 (z=3.0) 外面，底宽 6px，尖刀正朝 +Z 伸出 3px 到 z=6.0 (立于出料带面之上，高 y in [4.05, 8.0])；
   - 尖刀迎向出料方向，正中带有一道 #b8a888 刀锋暗面脊线，将左右两路进料流优雅导引汇入 +Z 出口；
5. 输出图：
   - render.png（俯视 TOP VIEW + 3/4 视 + 侧视 + 正视 2×2 高清拼版）；
   - check.png（左爆炸参考图 x12_merger.png，右 3/4 等轴视与俯视并排等高对标卡）。
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
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/merger")
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x12_merger.png")
SINGLE_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b12_merger.png")

from modelScript.generators.meridian_factory.common_parts import (
    PALETTE,
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    bone_post,
    bone_rail,
)


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_conveyors_in() -> List[dict]:
    """左右两条进料传送带 (-X 进与 +X 进) 及北侧护轨系统：
    - 左进料带 (-X 进)：x in [-8.0, -4.0]，带面宽 8px 居中 (z in [-4, 4])，顶高 y=4；
    - 右进料带 (+X 进)：x in [ 4.0,  8.0]，带面宽 8px 居中 (z in [-4, 4])，顶高 y=4；
    - 带面下 y in [0.0, 3.8] 是暖粉筋底座 (#d9a08c)，侧面附斜筋丝 (#c07868)；
    - 北侧骨轨：截面 2 宽 × 2 高，底在 y=4、顶在 y=6 (x in [-6, -3] 与 [3, 6]，z in [-6, -4])；
    - 外端立柱：西端两侧各一根、东端两侧各一根 (截面 2x2，底在 y=4、顶在 y=8，带 #b8a888 暗纹)。
    """
    cubes = []

    # ═══════════ 左进料带 (-X 进，西向) ═══════════
    # 1. 筋质底座 (x in [-8.0, -4.0], y in [0.0, 3.8], z in [-4.0, 4.0])
    cubes.append({"name": "in_l_base", "from": [-8.0, 0.0, -4.0], "to": [-4.0, 3.8, 4.0], "group": "conveyors_in", "material": "tendon_tube"})

    # 底座北侧斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "in_l_base_fiber_n", "from": [-7.5, 0.8, -4.05], "to": [-5.5, 2.8, -4.0], "group": "conveyors_in", "material": "tendon_fiber"})

    # 2. 带面横肋 (y in [3.8, 4.0]，垂直于进料方向向内运动)
    cubes.append({"name": "in_l_rib_1", "from": [-8.0, 3.8, -4.0], "to": [-6.0, 4.0, 4.0], "group": "conveyors_in", "material": "flesh_main"})
    cubes.append({"name": "in_l_rib_2", "from": [-6.0, 3.8, -4.0], "to": [-4.0, 4.0, 4.0], "group": "conveyors_in", "material": "flesh_dark"})

    # 3. 外端两侧各一根立柱 (截面 2x2，底在 y=4、顶在 y=8: y in [4.0, 8.0], x in [-8.0, -6.0])
    # 北立柱: z in [-6.0, -4.0]
    cubes.append({"name": "in_l_post_n", "from": [-8.0, 4.0, -6.0], "to": [-6.0, 8.0, -4.0], "group": "conveyors_in", "material": "bone_main"})
    cubes.append({"name": "in_l_post_n_stripe", "from": [-7.5, 5.5, -6.05], "to": [-6.5, 6.5, -3.95], "group": "conveyors_in", "material": "bone_dark"})
    # 南立柱: z in [4.0, 6.0]
    cubes.append({"name": "in_l_post_s", "from": [-8.0, 4.0,  4.0], "to": [-6.0, 8.0,  6.0], "group": "conveyors_in", "material": "bone_main"})
    cubes.append({"name": "in_l_post_s_stripe", "from": [-7.5, 5.5,  3.95], "to": [-6.5, 6.5,  6.05], "group": "conveyors_in", "material": "bone_dark"})

    # 4. 北骨轨 (截面 2x2，底在 y=4、顶在 y=6: x in [-6.0, -3.0], z in [-6.0, -4.0])
    cubes.append({"name": "in_l_rail_n", "from": [-6.0, 4.0, -6.0], "to": [-3.0, 6.0, -4.0], "group": "conveyors_in", "material": "bone_main"})

    # ═══════════ 右进料带 (+X 进，东向) ═══════════
    # 1. 筋质底座 (x in [4.0, 8.0], y in [0.0, 3.8], z in [-4.0, 4.0])
    cubes.append({"name": "in_r_base", "from": [4.0, 0.0, -4.0], "to": [8.0, 3.8, 4.0], "group": "conveyors_in", "material": "tendon_tube"})

    # 底座北侧斜筋丝
    cubes.append({"name": "in_r_base_fiber_n", "from": [5.5, 0.8, -4.05], "to": [7.5, 2.8, -4.0], "group": "conveyors_in", "material": "tendon_fiber"})

    # 2. 带面横肋 (y in [3.8, 4.0])
    cubes.append({"name": "in_r_rib_1", "from": [4.0, 3.8, -4.0], "to": [6.0, 4.0, 4.0], "group": "conveyors_in", "material": "flesh_dark"})
    cubes.append({"name": "in_r_rib_2", "from": [6.0, 3.8, -4.0], "to": [8.0, 4.0, 4.0], "group": "conveyors_in", "material": "flesh_main"})

    # 3. 外端两侧各一根立柱 (截面 2x2，底在 y=4、顶在 y=8: y in [4.0, 8.0], x in [6.0, 8.0])
    # 北立柱: z in [-6.0, -4.0]
    cubes.append({"name": "in_r_post_n", "from": [6.0, 4.0, -6.0], "to": [8.0, 8.0, -4.0], "group": "conveyors_in", "material": "bone_main"})
    cubes.append({"name": "in_r_post_n_stripe", "from": [6.5, 5.5, -6.05], "to": [7.5, 6.5, -3.95], "group": "conveyors_in", "material": "bone_dark"})
    # 南立柱: z in [4.0, 6.0]
    cubes.append({"name": "in_r_post_s", "from": [6.0, 4.0,  4.0], "to": [8.0, 8.0,  6.0], "group": "conveyors_in", "material": "bone_main"})
    cubes.append({"name": "in_r_post_s_stripe", "from": [6.5, 5.5,  3.95], "to": [7.5, 6.5,  6.05], "group": "conveyors_in", "material": "bone_dark"})

    # 4. 北骨轨 (截面 2x2，底在 y=4、顶在 y=6: x in [3.0, 6.0], z in [-6.0, -4.0])
    cubes.append({"name": "in_r_rail_n", "from": [3.0, 4.0, -6.0], "to": [6.0, 6.0, -4.0], "group": "conveyors_in", "material": "bone_main"})

    return cubes


def part_02_conveyor_out() -> List[dict]:
    """南侧出料传送带 (+Z 出，从 z = 3.0 到 z = 8.0)：
    - 宽 8px 居中 (x in [-4, 4])，带面顶高 y=4 (y in [3.8, 4.0])；
    - 沿 Z 轴运动方向每 2px 一条横肋 (#8a2a2a 与 #5a1a1a 交替)；
    - 带面下方 y in [0.0, 3.8] 是暖粉筋底座 (#d9a08c)，两侧附斜筋丝；
    - 端头外侧两侧各一根立柱 2×2，底在 y=4、顶在 y=8 (z in [6, 8])；
    - 西南角与东南角转角 2x2 立柱 (z in [4, 6], x in [-6, -4] 与 [4, 6]) 连通进料南立柱。
    """
    cubes = []

    # 1. 筋质底座 (宽 8px，y in [0.0, 3.8]，z in [3.0, 8.0])
    cubes.append({"name": "out_base", "from": [-4.0, 0.0, 3.0], "to": [4.0, 3.8, 8.0], "group": "conveyor_out", "material": "tendon_tube"})

    # 底座侧面斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "out_base_fiber_w", "from": [-4.05, 0.8, 5.5], "to": [-4.0, 2.8, 7.5], "group": "conveyor_out", "material": "tendon_fiber"})
    cubes.append({"name": "out_base_fiber_e", "from": [ 4.0,  0.8, 5.5], "to": [ 4.05, 2.8, 7.5], "group": "conveyor_out", "material": "tendon_fiber"})

    # 2. 带面横肋 (顶高 y=4: y in [3.8, 4.0]，沿向南流动方向排列)
    cubes.append({"name": "out_belt_rib_1", "from": [-4.0, 3.8, 3.0], "to": [4.0, 4.0, 4.0], "group": "conveyor_out", "material": "flesh_main"})
    cubes.append({"name": "out_belt_rib_2", "from": [-4.0, 3.8, 4.0], "to": [4.0, 4.0, 6.0], "group": "conveyor_out", "material": "flesh_dark"})
    cubes.append({"name": "out_belt_rib_3", "from": [-4.0, 3.8, 6.0], "to": [4.0, 4.0, 8.0], "group": "conveyor_out", "material": "flesh_main"})

    # 3. 外端两侧各一根立柱 (截面 2x2，底在 y=4、顶在 y=8: y in [4.0, 8.0], z in [6.0, 8.0])
    # 西立柱: x in [-6.0, -4.0]
    cubes.append({"name": "out_post_w", "from": [-6.0, 4.0, 6.0], "to": [-4.0, 8.0, 8.0], "group": "conveyor_out", "material": "bone_main"})
    cubes.append({"name": "out_post_w_stripe", "from": [-6.05, 5.5, 6.5], "to": [-3.95, 6.5, 7.5], "group": "conveyor_out", "material": "bone_dark"})
    # 东立柱: x in [4.0, 6.0]
    cubes.append({"name": "out_post_e", "from": [ 4.0, 4.0, 6.0], "to": [ 6.0, 8.0, 8.0], "group": "conveyor_out", "material": "bone_main"})
    cubes.append({"name": "out_post_e_stripe", "from": [ 3.95, 5.5, 6.5], "to": [ 6.05, 6.5, 7.5], "group": "conveyor_out", "material": "bone_dark"})

    # 4. 连通转角 2x2 骨立柱 (西南角与东南角，连通进料南端与出料西/东端)
    # 西南转角立柱: x in [-6.0, -4.0], z in [4.0, 6.0]
    cubes.append({"name": "corner_post_sw", "from": [-6.0, 4.0, 4.0], "to": [-4.0, 8.0, 6.0], "group": "conveyor_out", "material": "bone_main"})
    cubes.append({"name": "corner_post_sw_stripe", "from": [-6.05, 5.5, 4.5], "to": [-3.95, 6.5, 5.5], "group": "conveyor_out", "material": "bone_dark"})
    # 东南转角立柱: x in [4.0, 6.0], z in [4.0, 6.0]
    cubes.append({"name": "corner_post_se", "from": [ 4.0, 4.0, 4.0], "to": [ 6.0, 8.0, 6.0], "group": "conveyor_out", "material": "bone_main"})
    cubes.append({"name": "corner_post_se_stripe", "from": [ 3.95, 5.5, 4.5], "to": [ 6.05, 6.5, 5.5], "group": "conveyor_out", "material": "bone_dark"})

    return cubes


def part_03_central_sac() -> List[dict]:
    """中心紧凑筋质外壳 (x/z in [-3, 3], y 0–8)：
    - 绝不盖住带面，三条带在外壳边缘截止钻入；
    - 顶面圆钝阶梯顶盖：y 7.2..8.0 向内微缩 0.5px (x/z in [-2.5, 2.5])；
    - 顶面透光亮面 #e8bca8 与斜筋丝 #c07868；
    - 北面 (-Z 面，即 z = -3.0 处) 为盲端封闭壁，配有 #e8bca8 亮面板与 2 条斜交筋丝。
    """
    cubes = []

    # 1. 筋质外壳主体下半部 (y in [0.0, 7.2]，x/z in [-3.0, 3.0])
    cubes.append({"name": "sac_body", "from": [-3.0, 0.0, -3.0], "to": [3.0, 7.2, 3.0], "group": "central_sac", "material": "tendon_tube"})

    # 2. 顶面圆钝阶梯顶盖 (y in [7.2, 8.0]，四周内缩 0.5px: x/z in [-2.5, 2.5])
    cubes.append({"name": "sac_top_cap", "from": [-2.5, 7.2, -2.5], "to": [2.5, 8.0, 2.5], "group": "central_sac", "material": "tendon_tube"})

    # 顶面透光亮板 (#e8bca8 tendon_highlight，贴于顶面微凸 0.05px: y in [8.0, 8.06])
    cubes.append({"name": "sac_top_glow", "from": [-1.8, 8.0, -1.8], "to": [1.8, 8.06, 1.8], "group": "central_sac", "material": "tendon_highlight"})

    # 顶面斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "sac_top_fiber", "from": [-1.6, 8.06, -0.8], "to": [1.6, 8.12, 0.8], "group": "central_sac", "material": "tendon_fiber"})

    # 3. 北面 (-Z 面，即 z = -3.0 背侧盲端封闭壁) 亮色衬板 (#e8bca8 tendon_highlight，z in [-3.06, -3.0], y in [1.5, 6.5])
    cubes.append({"name": "sac_back_glow", "from": [-2.0, 1.5, -3.06], "to": [2.0, 6.5, -3.0], "group": "central_sac", "material": "tendon_highlight"})

    # 北面 (-Z 面) 斜筋丝 (#c07868 tendon_fiber)
    cubes.append({"name": "sac_back_fiber_1", "from": [-2.5, 2.0, -3.12], "to": [-0.5, 4.0, -3.06], "group": "central_sac", "material": "tendon_fiber"})
    cubes.append({"name": "sac_back_fiber_2", "from": [ 0.5, 4.5, -3.12], "to": [ 2.5, 6.5, -3.06], "group": "central_sac", "material": "tendon_fiber"})

    return cubes


def part_04_bone_wedge() -> List[dict]:
    """合流骨楔看得见：放在外壳朝 +Z 出口那一面 (z=3.0) 外面，
    三角形俯视轮廓（底宽 6px、尖朝 +Z 伸出 3px 到 z=6.0），高 y in [4.05, 8.0]，#d8ccb0。
    立在出料带面 (y=4) 之上，将左右两路汇入物料流导引合流向 +Z 出料带。
    """
    cubes = []

    # 1. 根部大底座 (紧贴外壳南壁 z=3.0，深 z in [3.0, 4.0]，宽 5.9px ≈ 6px: x in [-2.95, 2.95]，高 y in [4.05, 8.0])
    cubes.append({"name": "wedge_base", "from": [-2.95, 4.05, 3.0], "to": [2.95, 8.0, 4.0], "group": "bone_wedge", "material": "bone_main"})

    # 2. 中段过渡梯形 (深 z in [4.0, 5.0]，宽 3.4px: x in [-1.7, 1.7]，高 y in [4.05, 8.0])
    cubes.append({"name": "wedge_mid", "from": [-1.7, 4.05, 4.0], "to": [1.7, 8.0, 5.0], "group": "bone_wedge", "material": "bone_main"})

    # 3. 迎流尖刀 (尖端位于 z = 6.0，深 z in [5.0, 6.0]，宽 1.4px: x in [-0.7, 0.7]，高 y in [4.05, 8.0])
    cubes.append({"name": "wedge_tip", "from": [-0.7, 4.05, 5.0], "to": [0.7, 8.0, 6.0], "group": "bone_wedge", "material": "bone_main"})

    # 4. 骨楔正中迎流刀锋暗面脊线 (#b8a888 bone_dark，沿劈尖最前端凸出 0.05px: z in [6.0, 6.06])
    cubes.append({"name": "wedge_ridge_spine", "from": [-0.25, 4.5, 6.0], "to": [0.25, 7.8, 6.06], "group": "bone_wedge", "material": "bone_dark"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体。"""
    return (
        part_01_conveyors_in()
        + part_02_conveyor_out()
        + part_03_central_sac()
        + part_04_bone_wedge()
    )


# =============================================================================
# 导出与渲染
# =============================================================================

def generate_bbmodel(out_path: Path) -> Path:
    """导出 merger 的 bbmodel 文件。"""
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
        mat = c.get("material", "tendon_tube")
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

    name = "merger"
    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "merger")
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
    """输出包含（俯视 + 3/4 + 侧视 + 正视）的综合拼图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)

    # 1. 渲染各视角：俯视 (TOP)、3/4 等轴视 (Isometric)、侧视 (SIDE: 观察带面与筋底座)、正视 (FRONT)
    im_top, _ = render(model_p, yaw=0.0, pitch=89.9, size=500, bg=bg_color)
    im_iso, _ = render(model_p, yaw=-35.0, pitch=30.0, size=500, bg=bg_color)
    im_side, _ = render(model_p, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_front, _ = render(model_p, yaw=0.0, pitch=0.0, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格拼版：俯视、3/4 视、侧视、正视)
    cell_w, cell_h = 500, 500
    canvas_w = cell_w * 2 + 30
    canvas_h = cell_h * 2 + 30
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("TOP VIEW (T-Conveyor: ±X In, +Z Out & Wedge Tip)", im_top, 10, 10),
        ("3/4 ISOMETRIC (Compact Shell, Wedge & Posts)", im_iso, cell_w + 20, 10),
        ("SIDE VIEW (Rails y:4-6, Posts y:4-8, Base y:0-4)", im_side, 10, cell_h + 20),
        ("FRONT VIEW (-Z Closed Blind Wall)", im_front, cell_w + 20, cell_h + 20),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 440, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(235, 235, 235))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png (俯视 + 3/4 + 侧视 + 正视 2x2 拼版) 已输出: {render_path}")

    # 3. 拼装 check.png (左为 x12 爆炸参考图，右为当前模型 3/4 等轴视与俯视并排等高对标)
    ref_image_to_use = EXPLODED_REF if EXPLODED_REF.exists() else SINGLE_REF
    if ref_image_to_use.exists():
        ref_im = Image.open(ref_image_to_use).convert("RGB")
        target_h = 550
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        thumb_size = 500
        c_iso = im_iso.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)
        c_top = im_top.resize((thumb_size, thumb_size), Image.Resampling.LANCZOS)

        right_w = thumb_size * 2 + 16
        total_w = ref_w + right_w + 36
        check_cv = Image.new("RGB", (total_w, target_h + 46), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考图
        check_cv.paste(ref_scaled, (12, 34))
        c_draw.text((16, 10), f"EXPLODED REFERENCE ({ref_image_to_use.name})", fill=(210, 200, 180))

        # 贴右渲染图 (左 3/4 视，右 TOP 俯视)
        rx = ref_w + 24
        check_cv.paste(c_iso, (rx, 34 + 10))
        check_cv.paste(c_top, (rx + thumb_size + 8, 34 + 10))

        c_draw.text((rx + 4, 10), "NOW RENDER (LEFT: 3/4 Isometric View, RIGHT: TOP View | Visible Wedge Tip +Z & T-Conveyors)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 爆炸图并排对照图 (含 3/4 视角与俯视对标) 已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_merger.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] merger ({len(cubes)} 个立方体) 正常立方体集无共面冲突")

    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 0.0, 3.0],
        "to":   [ 4.0, 3.8, 8.0],
        "material": "tendon_tube",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] merger 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: merger 注入共面冲突未被拦截!")

    print("✓ gen_merger.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b12 merger 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    self_test()
    p = generate_bbmodel(out_path=MODEL_DIR / "merger.bbmodel")
    render_views(p)


if __name__ == "__main__":
    main()
