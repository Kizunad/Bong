#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b01: meridian_straight (经脉内腔直段) [Round 1 第 1 次修改版]

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md

调度审第 1 次修改落实：
1. 半透明筋管 + 内光芯 + 斜交筋丝：
   - 管壁使用 #e8d8d0 半透明材质 (alpha~60%)；
   - 管内沿长轴放一根细芯 #f4ece0 (宽2高2贯通Z轴) 作为灵流内光；
   - 管壁外侧浮起 0.5px 配置两组斜交筋丝 (#d8c4b8)，立体交叉呈现鲜明 X 形纹。
2. 两端四面包覆肉箍，彻底删除底部红板：
   - 删掉原先底部的漂浮红板；
   - 两端各设一圈四面包覆的肉箍 (#8a2a2a)，箍宽 3px (z: -7.5..-4.5 与 4.5..7.5)；
   - 比管身四周各外凸 1px (x: -5..5, y: 1..9，四面均包覆)；
   - 中间区段 (z: -4.5..4.5) 完全无红色，纯净展现半透明管身。
3. 贴面小骨环端口：
   - 端口改成贴在肉箍顶面 (y=9.0) 与底面 (y=1.0) 的贴面骨环 (#d8ccb0)；
   - 外框 4×4、内孔 2×2 (孔内用 #5a1a1a 呈现深邃暗孔)，凸出 0.7px (严格不超过 1px)；
   - 四个角各一个 (顶前、顶后、底前、底后)。
4. 统一截面契约：
   - 管腔外轮廓截面严格为 8 宽 × 6 高、居中、底边离地 2px (x: -4..4, y: 2..8)；
   - 整件长度严格为 16px (z: -8..8)。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "meridian_straight.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_straight")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b01_meridian_straight.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":   (90, 26, 26, 255),    # #5a1a1a 暗血肉 (孔内深色底)
    "flesh_main":   (138, 42, 42, 255),   # #8a2a2a 血肉 (两端肉箍主色)
    "bone_main":    (216, 204, 176, 255), # #d8ccb0 骨 (贴面小骨环)
    "tendon_tube":  (232, 216, 208, 150), # #e8d8d0 半透明筋管 (alpha 约 60%)
    "qi_glow":      (244, 236, 224, 255), # #f4ece0 筋管内光细芯
    "tendon_fiber": (216, 196, 184, 255), # #d8c4b8 斜交筋丝 (浮起 0.5px)
}

MAT_UV = {
    "flesh_dark":   [0, 0, 16, 16],
    "flesh_main":   [16, 0, 32, 16],
    "bone_main":    [32, 0, 48, 16],
    "tendon_tube":  [48, 0, 64, 16],
    "qi_glow":      [0, 16, 16, 32],
    "tendon_fiber": [16, 16, 32, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_collars() -> List[dict]:
    """两端四面包覆的肉箍：宽 3px，比管身外凸 1px，四周包覆，彻底删除底部红板。
    管身外径：x: -4.0..4.0, y: 2.0..8.0
    肉箍范围：x: -5.0..5.0, y: 1.0..9.0 (四周各外凸 1px)
    后箍：z: -7.5..-4.5 (宽 3.0px)
    前箍：z:  4.5..7.5  (宽 3.0px)
    中间 (z: -4.5..4.5) 完全无红色肉箍，展现半透明管身。
    """
    cubes = []

    for p_name, z_start, z_end in [("rear", -7.5, -4.5), ("front", 4.5, 7.5)]:
        cz = (z_start + z_end) / 2.0  # -6.0 或 6.0

        # 1. 顶面肉层 (y: 8.0..9.0, 跨度 x: -5.0..5.0)
        # 在中心留出骨环孔道 x: -1.0..1.0, z: cz-1.0..cz+1.0
        cubes.append({
            "name": f"collar_{p_name}_top_left",
            "from": [-5.0, 8.0, z_start],
            "to":   [-1.0, 9.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_top_right",
            "from": [1.0, 8.0, z_start],
            "to":   [5.0, 9.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_top_mid_b",
            "from": [-1.0, 8.0, z_start],
            "to":   [1.0, 9.0, cz - 1.0],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_top_mid_f",
            "from": [-1.0, 8.0, cz + 1.0],
            "to":   [1.0, 9.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })

        # 2. 底面肉层 (y: 1.0..2.0, 跨度 x: -5.0..5.0)
        # 在中心留出骨环孔道 x: -1.0..1.0, z: cz-1.0..cz+1.0
        cubes.append({
            "name": f"collar_{p_name}_bot_left",
            "from": [-5.0, 1.0, z_start],
            "to":   [-1.0, 2.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_bot_right",
            "from": [1.0, 1.0, z_start],
            "to":   [5.0, 2.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_bot_mid_b",
            "from": [-1.0, 1.0, z_start],
            "to":   [1.0, 2.0, cz - 1.0],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_bot_mid_f",
            "from": [-1.0, 1.0, cz + 1.0],
            "to":   [1.0, 2.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })

        # 3. 左右侧面包覆肉层 (y: 2.0..8.0，厚 1px，紧扣管身侧壁)
        cubes.append({
            "name": f"collar_{p_name}_side_l",
            "from": [-5.0, 2.0, z_start],
            "to":   [-4.0, 8.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_side_r",
            "from": [4.0, 2.0, z_start],
            "to":   [5.0, 8.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })

    return cubes


def part_02_ports() -> List[dict]:
    """贴在肉箍顶面/底面的骨环：外 4×4、内孔 2×2，孔里深色 #5a1a1a，凸出不超过 1px。
    四个角各一个：顶前、顶后、底前、底后。
    """
    cubes = []

    for p_name, z_start, z_end in [("rear", -7.5, -4.5), ("front", 4.5, 7.5)]:
        cz = (z_start + z_end) / 2.0  # -6.0 或 6.0
        # 外 4x4: x: -2.0..2.0, z: cz-2.0..cz+2.0
        # 内孔 2x2: x: -1.0..1.0, z: cz-1.0..cz+1.0

        # --- A. 顶面贴面骨环 (y: 9.0..9.7，凸出 0.7px) ---
        cubes.append({
            "name": f"port_top_{p_name}_north",
            "from": [-2.0, 9.0, cz - 2.0],
            "to":   [2.0, 9.7, cz - 1.0],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_south",
            "from": [-2.0, 9.0, cz + 1.0],
            "to":   [2.0, 9.7, cz + 2.0],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_west",
            "from": [-2.0, 9.0, cz - 1.0],
            "to":   [-1.0, 9.7, cz + 1.0],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_east",
            "from": [1.0, 9.0, cz - 1.0],
            "to":   [2.0, 9.7, cz + 1.0],
            "group": "ports",
            "material": "bone_main",
        })
        # 顶面孔内深色底 (#5a1a1a)
        cubes.append({
            "name": f"port_top_{p_name}_dark_core",
            "from": [-0.98, 8.0, cz - 0.98],
            "to":   [0.98, 8.95, cz + 0.98],
            "group": "ports",
            "material": "flesh_dark",
        })

        # --- B. 底面贴面骨环 (y: 0.3..1.0，向下凸出 0.7px) ---
        cubes.append({
            "name": f"port_bot_{p_name}_north",
            "from": [-2.0, 0.3, cz - 2.0],
            "to":   [2.0, 1.0, cz - 1.0],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_south",
            "from": [-2.0, 0.3, cz + 1.0],
            "to":   [2.0, 1.0, cz + 2.0],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_west",
            "from": [-2.0, 0.3, cz - 1.0],
            "to":   [-1.0, 1.0, cz + 1.0],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_east",
            "from": [1.0, 0.3, cz - 1.0],
            "to":   [2.0, 1.0, cz + 1.0],
            "group": "ports",
            "material": "bone_main",
        })
        # 底面孔内深色底 (#5a1a1a)
        cubes.append({
            "name": f"port_bot_{p_name}_dark_core",
            "from": [-0.98, 1.05, cz - 0.98],
            "to":   [0.98, 2.0, cz + 0.98],
            "group": "ports",
            "material": "flesh_dark",
        })

    return cubes


def part_03_meridian_tube() -> List[dict]:
    """矩形半透明筋管主体：宽 8 (x: -4..4), 高 6 (y: 2..8), 长 16 (z: -8..8)。
    材质：tendon_tube (#e8d8d0 半透明)。
    """
    cubes = []

    # 左侧壁
    cubes.append({
        "name": "tube_wall_left",
        "from": [-4.0, 2.0, -8.0],
        "to":   [-3.2, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    # 右侧壁
    cubes.append({
        "name": "tube_wall_right",
        "from": [3.2, 2.0, -8.0],
        "to":   [4.0, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    # 底壁
    cubes.append({
        "name": "tube_wall_bottom",
        "from": [-3.2, 2.0, -8.0],
        "to":   [3.2, 2.8, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    # 顶壁
    cubes.append({
        "name": "tube_wall_top",
        "from": [-3.2, 7.2, -8.0],
        "to":   [3.2, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    return cubes


def part_04_inner_qi_glow() -> List[dict]:
    """管内沿长轴细芯淡光 (#f4ece0 qi_glow)：长轴 z: -8.0..8.0 贯通，截面 x: -1.0..1.0, y: 4.0..6.0。"""
    cubes = []
    cubes.append({
        "name": "inner_qi_core",
        "from": [-1.0, 4.0, -8.0],
        "to":   [1.0, 6.0, 8.0],
        "group": "inner_qi_glow",
        "material": "qi_glow",
    })
    return cubes


def part_05_diagonal_fibers() -> List[dict]:
    """管壁外侧斜交筋丝 (#d8c4b8 tendon_fiber，0.5px 浮起，X 形纹)。"""
    cubes = []

    def make_cross_fibers(is_left: bool = False) -> List[dict]:
        fiber_cubes = []
        x_inner = -4.38 if is_left else 4.38
        x_outer = -4.52 if is_left else 4.52
        x_base = -4.0 if is_left else 4.0

        x0 = min(x_base, x_inner)
        x1 = max(x_base, x_inner)
        pfx = "l_" if is_left else "r_"

        # 筋丝 1（底层斜向上，从 z=-4.2 到 4.2）
        f1_pts = [
            (-4.2, -2.8, 2.5, 3.3),
            (-2.8, -1.4, 3.3, 4.2),
            (-1.4,  0.0, 4.2, 5.0),
            ( 0.0,  1.4, 5.0, 5.8),
            ( 1.4,  2.8, 5.8, 6.7),
            ( 2.8,  4.2, 6.7, 7.5),
        ]
        for i, (z0, z1, y0, y1) in enumerate(f1_pts):
            fiber_cubes.append({
                "name": f"tendon_{pfx}x1_seg_{i+1}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "diagonal_fibers",
                "material": "tendon_fiber",
            })

        # 筋丝 2（顶层斜向下，交叉点外架跨越桥）
        f2_upper = [
            (-4.2, -2.8, 6.7, 7.5),
            (-2.8, -1.4, 5.8, 6.7),
            (-1.4, -0.6, 5.3, 5.8),
        ]
        for i, (z0, z1, y0, y1) in enumerate(f2_upper):
            fiber_cubes.append({
                "name": f"tendon_{pfx}x2_up_{i+1}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "diagonal_fibers",
                "material": "tendon_fiber",
            })

        # 中心跨越节 (在 x1 外层跨越，z: -0.6..0.6, y: 4.65..5.35)
        x_br0 = min(x_inner, x_outer)
        x_br1 = max(x_inner, x_outer)
        fiber_cubes.append({
            "name": f"tendon_{pfx}x2_bridge",
            "from": [x_br0, 4.65, -0.6],
            "to":   [x_br1, 5.35,  0.6],
            "group": "diagonal_fibers",
            "material": "tendon_fiber",
        })

        f2_lower = [
            ( 0.6,  1.4, 4.2, 4.7),
            ( 1.4,  2.8, 3.3, 4.2),
            ( 2.8,  4.2, 2.5, 3.3),
        ]
        for i, (z0, z1, y0, y1) in enumerate(f2_lower):
            fiber_cubes.append({
                "name": f"tendon_{pfx}x2_dn_{i+1}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "diagonal_fibers",
                "material": "tendon_fiber",
            })

        return fiber_cubes

    cubes.extend(make_cross_fibers(is_left=False))
    cubes.extend(make_cross_fibers(is_left=True))
    return cubes


def all_cubes() -> List[dict]:
    """汇总所有 5 个部件的立方体。"""
    return (
        part_01_flesh_collars()
        + part_02_ports()
        + part_03_meridian_tube()
        + part_04_inner_qi_glow()
        + part_05_diagonal_fibers()
    )


# =============================================================================
# 门禁与无共面面核验
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标且重叠的共面冲突。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
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
            key = (side, round(val, 4))
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
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
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


# =============================================================================
# 贴图与 bbmodel 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图，严格使用 meridian_factory.md 的有机型配色。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if mat_name == "tendon_tube":
            # 半透明筋管壁：带纵向浅淡粉膜与半透明渐变，保持 ~60% 透明度
            for x in range(w):
                fib = rng.randint(-8, 8)
                tile[:, x, 0] = np.clip(tile[:, x, 0].astype(int) + fib, 0, 255)
                tile[:, x, 1] = np.clip(tile[:, x, 1].astype(int) + fib, 0, 255)
                tile[:, x, 2] = np.clip(tile[:, x, 2].astype(int) + fib, 0, 255)
                tile[:, x, 3] = np.clip(base_color[3] + rng.randint(-10, 10), 125, 175)
        elif mat_name == "qi_glow":
            # 晶莹淡光核心：中心稍亮向外微晕
            for y in range(h):
                for x in range(w):
                    r_dist = np.hypot(x - w / 2, y - h / 2) / (w / 2)
                    glow = int(12 * (1.0 - np.clip(r_dist, 0.0, 1.0)))
                    tile[y, x, :3] = np.clip(tile[y, x, :3].astype(int) + glow, 0, 255)
        elif mat_name == "tendon_fiber":
            # 斜交筋丝高光质感
            for y in range(h):
                tile[y, :, :3] = np.clip(tile[y, :, :3].astype(int) + rng.randint(-6, 6), 0, 255)
        elif "flesh" in mat_name:
            # 活体血肉微起伏噪声
            noise = rng.randint(-12, 12, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            # 骨质斑驳
            noise = rng.randint(-10, 10, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)

        im.paste(Image.fromarray(tile, mode="RGBA"), (u0, v0))

    return im


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: List[dict] | None = None) -> Path:
    """生成并导出 Blockbench bbmodel 文件。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = cubes_override if cubes_override is not None else all_cubes()
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

    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "meridian_straight")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [0.0, 0.0, 0.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "meridian_straight",
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": "meridian_straight",
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
    print(f"✓ meridian_straight bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 渲染与并排对标卡输出
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图的中性灰底色 (119, 119, 119)

    # 1. 渲染四视角 (正视、侧视、3/4 等轴、俯视)
    # 正视 (看 +Z 输出端截面：显示宽8高6接口与外围肉箍)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    # 侧视 (看整个 16px 长管道侧身：两端红箍+中间半透明管身与斜交X筋丝)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # 3/4 等轴透视 (立体观察斜交筋丝与顶面贴面骨环)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    # 俯视 (看顶面两端贴面小骨环与中间管身)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格)
    canvas_w = 1040
    canvas_h = 1040
    canvas = Image.new("RGB", (canvas_w, canvas_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("FRONT (+Z Output)", im_front, 20, 20),
        ("SIDE (Length 16px)", im_side, 540, 20),
        ("3/4 ISOMETRIC", im_iso, 20, 540),
        ("TOP (Dual Bone Rings)", im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 230, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    render_path = REVIEW_DIR / "render.png"
    canvas.save(render_path)
    print(f"✓ render.png 已输出: {render_path}")

    # 3. 拼装 check.png (左参考右渲染并排)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        # 右侧放置侧视与 3/4 视并排渲染
        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        side_scale_w = int(im_side.width * target_h / im_side.height)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)
        side_scaled = im_side.resize((side_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = side_scale_w + iso_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考
        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (b01_meridian_straight.png)", fill=(210, 200, 180))

        # 贴右渲染
        rx = ref_w + 24
        check_cv.paste(side_scaled, (rx, 30))
        check_cv.paste(iso_scaled, (rx + side_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (SIDE VIEW + 3/4 VIEW)", fill=(180, 220, 210))

        check_path = REVIEW_DIR / "check.png"
        check_cv.save(check_path)
        print(f"✓ check.png 并排对照图已输出: {check_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面，注入冲突能准确拦截。"""
    print("运行 gen_meridian_straight.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-4.0, 2.0, -8.0],
        "to":   [-3.2, 8.0, 8.0],
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_meridian_straight.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景 b01 meridian_straight 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认流程：自测 -> 导出 bbmodel -> 渲染图片
    self_test()
    bb_path = generate_bbmodel()
    render_views(bb_path)


if __name__ == "__main__":
    main()
