#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b01: meridian_straight (经脉内腔直段) [Round 1 第 2 次修改版]

风格：A 有机型 (活体血肉、暖粉半透明筋管、旧损暗淡配色)
依据：
- .task-meridian-models.md
- model-review/meridian_factory.md

调度审第 2 次修改落实：
1. 配色表修订与管身透光：
   - 筋管壁全面改用暖粉色 #d9a08c (tendon_tube, alpha~60%)；
   - 管壁上下各保留 1px #e8bca8 (tendon_highlight) 亮边；
   - 正面中段配置 6×2 (z: -3.0..3.0, y: 4.0..6.0) 晶莹透光的淡光斑 #f6dcc4 (qi_glow)；
   - 管内贯穿细芯同步改用 #f6dcc4。
2. 斜交筋丝连成两条完整对角线：
   - 筋丝全面改用 #c07868 (tendon_fiber)；
   - 浮起 0.5px，连贯平滑地跨越整个暴露管身 (z: -4.4..4.4, y: 2.4..7.6)，交叉形成完整 X 形纹。
3. 骨环端口缩小：
   - 端口尺寸严格缩小至：外 3×3、内孔 1×1、凸出 0.5px；
   - 贴在肉箍顶面 (y: 9.0..9.5) 与底面 (y: 0.5..1.0)；
   - 孔内采用 #5a1a1a (flesh_dark) 深色内底；
   - 四个角各设一个 (顶前、顶后、底前、底后)。
4. 结构保留：
   - 两端四面包覆肉箍 (#8a2a2a, 宽 3px, 比管身四周各外凸 1px：x: -5..5, y: 1..9)；
   - 中间无红色肉沿；
   - 接口截面宽 8 × 高 6、底边离地 2px，全长 16px。
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

BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "meridian_straight.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_straight")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b01_meridian_straight.png")

from modelScript.generators.meridian_factory.common_parts import (
    PALETTE,
    MAT_UV,
    RES,
    _assert_no_coplanar_faces,
    build_texture,
    sinew_tube,
    flesh_collar,
    bone_ring_port,
)


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_collars() -> List[dict]:
    """两端四面包覆的肉箍：宽 3px，比管身四周外凸 1px (x: -5..5, y: 1..9)。
    后箍：z: -7.5..-4.5 (长 3.0px)
    前箍：z:  4.5..7.5  (长 3.0px)
    中间 (z: -4.5..4.5) 完全无红色肉块。
    """
    cubes = []

    for p_name, z_start, z_end in [("rear", -7.5, -4.5), ("front", 4.5, 7.5)]:
        cz = (z_start + z_end) / 2.0  # -6.0 或 6.0

        # 1. 顶面肉层 (y: 8.0..9.0, 跨度 x: -5.0..5.0, 避让孔道 x: -0.5..0.5, z: cz-0.5..cz+0.5)
        cubes.append({
            "name": f"collar_{p_name}_top_left",
            "from": [-5.0, 8.0, z_start],
            "to":   [-0.5, 9.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_top_right",
            "from": [0.5, 8.0, z_start],
            "to":   [5.0, 9.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_top_mid_b",
            "from": [-0.5, 8.0, z_start],
            "to":   [0.5, 9.0, cz - 0.5],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_top_mid_f",
            "from": [-0.5, 8.0, cz + 0.5],
            "to":   [0.5, 9.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })

        # 2. 底面肉层 (y: 1.0..2.0, 跨度 x: -5.0..5.0, 避让孔道 x: -0.5..0.5, z: cz-0.5..cz+0.5)
        cubes.append({
            "name": f"collar_{p_name}_bot_left",
            "from": [-5.0, 1.0, z_start],
            "to":   [-0.5, 2.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_bot_right",
            "from": [0.5, 1.0, z_start],
            "to":   [5.0, 2.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_bot_mid_b",
            "from": [-0.5, 1.0, z_start],
            "to":   [0.5, 2.0, cz - 0.5],
            "group": "flesh_collars",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"collar_{p_name}_bot_mid_f",
            "from": [-0.5, 1.0, cz + 0.5],
            "to":   [0.5, 2.0, z_end],
            "group": "flesh_collars",
            "material": "flesh_main",
        })

        # 3. 左右侧面包覆肉层 (y: 2.0..8.0，厚 1px，紧贴管壁)
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
    """贴在肉箍顶底面的缩小骨环：外 3×3、内孔 1×1，孔里用 #5a1a1a，凸出 0.5px。
    四个角各一个：顶前、顶后、底前、底后。
    """
    cubes = []

    for p_name, z_start, z_end in [("rear", -7.5, -4.5), ("front", 4.5, 7.5)]:
        cz = (z_start + z_end) / 2.0  # -6.0 或 6.0
        # 外 3x3: x: -1.5..1.5, z: cz-1.5..cz+1.5
        # 内孔 1x1: x: -0.5..0.5, z: cz-0.5..cz+0.5

        # --- A. 顶面小骨环 (y: 9.0..9.5，凸出 0.5px) ---
        cubes.append({
            "name": f"port_top_{p_name}_n",
            "from": [-1.5, 9.0, cz - 1.5],
            "to":   [1.5, 9.5, cz - 0.5],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_s",
            "from": [-1.5, 9.0, cz + 0.5],
            "to":   [1.5, 9.5, cz + 1.5],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_w",
            "from": [-1.5, 9.0, cz - 0.5],
            "to":   [-0.5, 9.5, cz + 0.5],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_e",
            "from": [0.5, 9.0, cz - 0.5],
            "to":   [1.5, 9.5, cz + 0.5],
            "group": "ports",
            "material": "bone_main",
        })
        # 顶面孔内深色底 (#5a1a1a)
        cubes.append({
            "name": f"port_top_{p_name}_core",
            "from": [-0.48, 8.0, cz - 0.48],
            "to":   [0.48, 8.95, cz + 0.48],
            "group": "ports",
            "material": "flesh_dark",
        })

        # --- B. 底面小骨环 (y: 0.5..1.0，向下凸出 0.5px) ---
        cubes.append({
            "name": f"port_bot_{p_name}_n",
            "from": [-1.5, 0.5, cz - 1.5],
            "to":   [1.5, 1.0, cz - 0.5],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_s",
            "from": [-1.5, 0.5, cz + 0.5],
            "to":   [1.5, 1.0, cz + 1.5],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_w",
            "from": [-1.5, 0.5, cz - 0.5],
            "to":   [-0.5, 1.0, cz + 0.5],
            "group": "ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_e",
            "from": [0.5, 0.5, cz - 0.5],
            "to":   [1.5, 1.0, cz + 0.5],
            "group": "ports",
            "material": "bone_main",
        })
        # 底面孔内深色底 (#5a1a1a)
        cubes.append({
            "name": f"port_bot_{p_name}_core",
            "from": [-0.48, 1.05, cz - 0.48],
            "to":   [0.48, 2.0, cz + 0.48],
            "group": "ports",
            "material": "flesh_dark",
        })

    return cubes


def part_03_meridian_tube() -> List[dict]:
    """暖粉半透明筋管主体：宽 8 (x: -4..4), 高 6 (y: 2..8), 长 16 (z: -8..8)。
    管壁上下各留 1px #e8bca8 亮边，正面中段开有一块 6×2 的 #f6dcc4 淡光斑。
    """
    cubes = []

    # ── 左侧管壁 (x: -4.0..-3.2, y: 2.0..8.0, z: -8.0..8.0) ──
    cubes.append({
        "name": "tube_wall_l",
        "from": [-4.0, 2.0, -8.0],
        "to":   [-3.2, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # ── 右侧管壁 (正面观察面) ──
    # 两端隐藏段 (z: -8.0..-4.5 与 4.5..8.0)
    cubes.append({
        "name": "tube_wall_r_rear",
        "from": [3.2, 2.0, -8.0],
        "to":   [4.0, 8.0, -4.5],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_r_front",
        "from": [3.2, 2.0, 4.5],
        "to":   [4.0, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # 中间暴露段 (z: -4.5..4.5) 上下 1px #e8bca8 亮边
    # 底部 1px 亮边 (y: 2.0..3.0)
    cubes.append({
        "name": "tube_wall_r_mid_bot_rim",
        "from": [3.2, 2.0, -4.5],
        "to":   [4.0, 3.0, 4.5],
        "group": "meridian_tube",
        "material": "tendon_highlight",
    })
    # 顶部 1px 亮边 (y: 7.0..8.0)
    cubes.append({
        "name": "tube_wall_r_mid_top_rim",
        "from": [3.2, 7.0, -4.5],
        "to":   [4.0, 8.0, 4.5],
        "group": "meridian_tube",
        "material": "tendon_highlight",
    })

    # 中部侧翼暖粉管身 (z: -4.5..-3.0 与 3.0..4.5, y: 3.0..7.0)
    cubes.append({
        "name": "tube_wall_r_mid_flank_b",
        "from": [3.2, 3.0, -4.5],
        "to":   [4.0, 7.0, -3.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_r_mid_flank_f",
        "from": [3.2, 3.0, 3.0],
        "to":   [4.0, 7.0, 4.5],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # 正面 6×2 淡光斑上下垫块 (y: 3.0..4.0 与 6.0..7.0, z: -3.0..3.0)
    cubes.append({
        "name": "tube_wall_r_mid_sub_bot",
        "from": [3.2, 3.0, -3.0],
        "to":   [4.0, 4.0, 3.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_r_mid_sub_top",
        "from": [3.2, 6.0, -3.0],
        "to":   [4.0, 7.0, 3.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # 正面中段 6×2 淡光斑 (#f6dcc4 qi_glow, z: -3.0..3.0, y: 4.0..6.0)
    cubes.append({
        "name": "tube_wall_r_glow_window",
        "from": [3.2, 4.0, -3.0],
        "to":   [4.0, 6.0, 3.0],
        "group": "meridian_tube",
        "material": "qi_glow",
    })

    # 底壁与顶壁
    cubes.append({
        "name": "tube_wall_bottom",
        "from": [-3.2, 2.0, -8.0],
        "to":   [3.2, 2.8, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_top",
        "from": [-3.2, 7.2, -8.0],
        "to":   [3.2, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    return cubes


def part_04_inner_qi_glow() -> List[dict]:
    """管内沿长轴细芯淡光 (#f6dcc4 qi_glow)：截面 x: -1.0..1.0, y: 4.0..6.0, 长轴 z: -8.0..8.0 贯通。"""
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
    """斜交筋丝 (#c07868 tendon_fiber，0.5px 浮起，连成两条完整贯穿对角线)。"""
    cubes = []

    def make_diagonal_x(is_left: bool = False) -> List[dict]:
        diag_cubes = []
        x_base = -4.0 if is_left else 4.0
        x_in = -4.36 if is_left else 4.36
        x_out = -4.50 if is_left else 4.50
        x0 = min(x_base, x_in)
        x1 = max(x_base, x_in)
        x_br0 = min(x_in, x_out)
        x_br1 = max(x_in, x_out)
        pfx = "l_" if is_left else "r_"

        steps = 8
        z_min, z_max = -4.4, 4.4
        y_min, y_max = 2.4, 7.6
        z_s = np.linspace(z_min, z_max, steps + 1)
        y_s = np.linspace(y_min, y_max, steps + 1)

        # 贯穿对角线 1 (底层筋丝，斜向上：从 z=-4.4, y=2.4 到 z=4.4, y=7.6)
        for i in range(steps):
            diag_cubes.append({
                "name": f"tendon_{pfx}diag1_seg_{i+1}",
                "from": [x0, round(y_s[i], 3), round(z_s[i], 3)],
                "to":   [x1, round(y_s[i+1], 3), round(z_s[i+1], 3)],
                "group": "diagonal_fibers",
                "material": "tendon_fiber",
            })

        # 贯穿对角线 2 (顶层筋丝，斜向下：从 z=-4.4, y=7.6 到 z=4.4, y=2.4，中心段跨越外层)
        for i in range(steps):
            y0_i = round(y_s[steps - i - 1], 3)
            y1_i = round(y_s[steps - i], 3)
            z0_i = round(z_s[i], 3)
            z1_i = round(z_s[i+1], 3)

            # 中心段 (i=3, 4) 跨越
            if i in (3, 4):
                diag_cubes.append({
                    "name": f"tendon_{pfx}diag2_bridge_{i+1}",
                    "from": [x_br0, y0_i, z0_i],
                    "to":   [x_br1, y1_i, z1_i],
                    "group": "diagonal_fibers",
                    "material": "tendon_fiber",
                })
            else:
                diag_cubes.append({
                    "name": f"tendon_{pfx}diag2_seg_{i+1}",
                    "from": [x0, y0_i, z0_i],
                    "to":   [x1, y1_i, z1_i],
                    "group": "diagonal_fibers",
                    "material": "tendon_fiber",
                })

        return diag_cubes

    cubes.extend(make_diagonal_x(is_left=False))
    cubes.extend(make_diagonal_x(is_left=True))
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
    """生成 64×64 RGBA 贴图，严格使用 meridian_factory.md 修订后的暖粉配色。"""
    im = Image.new("RGBA", (res, res), (0, 0, 0, 0))
    rng = np.random.RandomState(42)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_color = PALETTE[mat_name]
        w = u1 - u0
        h = v1 - v0
        tile = np.zeros((h, w, 4), dtype=np.uint8)
        tile[:, :] = base_color

        if mat_name == "tendon_tube":
            # 暖粉半透明筋管壁：纵向柔韧筋膜微纹，保持约 60% 透明度
            for x in range(w):
                fib = rng.randint(-8, 8)
                tile[:, x, 0] = np.clip(tile[:, x, 0].astype(int) + fib, 0, 255)
                tile[:, x, 1] = np.clip(tile[:, x, 1].astype(int) + fib, 0, 255)
                tile[:, x, 2] = np.clip(tile[:, x, 2].astype(int) + fib, 0, 255)
                tile[:, x, 3] = np.clip(base_color[3] + rng.randint(-10, 10), 125, 175)
        elif mat_name == "tendon_highlight":
            # 筋管上下亮边反光
            for y in range(h):
                tile[y, :, :3] = np.clip(tile[y, :, :3].astype(int) + rng.randint(-6, 6), 0, 255)
        elif mat_name == "qi_glow":
            # 晶莹淡光核心 / 6x2 透光斑：中心明亮微光晕
            for y in range(h):
                for x in range(w):
                    r_dist = np.hypot(x - w / 2, y - h / 2) / (w / 2)
                    glow = int(14 * (1.0 - np.clip(r_dist, 0.0, 1.0)))
                    tile[y, x, :3] = np.clip(tile[y, x, :3].astype(int) + glow, 0, 255)
        elif mat_name == "tendon_fiber":
            # 斜交筋丝韧带高光
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
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
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
        ("TOP (3x3 Bone Rings)", im_top, 540, 540),
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
