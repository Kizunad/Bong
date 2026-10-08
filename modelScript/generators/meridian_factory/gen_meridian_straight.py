#!/usr/bin/env python3
"""经脉工厂内景方块生成器 —— b01: meridian_straight (经脉内腔直段)

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

统一约定落实：
1. 尺寸：严格落在 16×16×16 内 (x: -8..8, y: 0..16, z: -8..8)，中心对齐方块网格。
2. 朝向：建模源正面朝 +Z，输入在 -Z 侧 (z = -8)，输出在 +Z 侧 (z = 8)。
3. 连接面：接口截面统一为 宽 8 px × 高 6 px、居中、底边离地 2 px (x: -4..4, y: 2..8)。
4. 结构要点：
   - 一整格长的矩形筋管，管壁半透明 (#e8d8d0, alpha~60%)，内有淡光 (#f4ece0)；
   - 两侧边缘一圈薄肉沿 (#5a1a1a / #8a2a2a / #b05050)，贴合在地面上；
   - 顶面和底面靠两端各一个圆形端口开口 (骨环基座 #d8ccb0，外径~4.4px，中央开孔直通内腔)。
5. 门禁要求：通过 _assert_no_coplanar_faces 自检，带 --self-test 缺陷拦截验证。
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
from PIL import Image, ImageDraw, ImageFont

REPO = Path(__file__).resolve().parents[3]
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "meridian_straight.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/meridian_straight")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/b01_meridian_straight.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":   (90, 26, 26, 255),    # #5a1a1a 暗血肉 (地面、附着基底)
    "flesh_main":   (138, 42, 42, 255),   # #8a2a2a 血肉 (肌肉、肉沿主色)
    "flesh_lit":    (176, 80, 80, 255),   # #b05050 亮肉 / 肺泡粉 (肉腱、凸缘高光)
    "bone_main":    (216, 204, 176, 255), # #d8ccb0 骨 (端口外环、加固卡箍)
    "bone_dark":    (184, 168, 136, 255), # #b8a888 骨暗面
    "bone_crevice": (106, 90, 72, 255),   # #6a5a48 骨缝 / 阴影
    "tendon_tube":  (232, 216, 208, 155), # #e8d8d0 半透明筋管 (alpha 约 60%)
    "qi_glow":      (244, 236, 224, 255), # #f4ece0 筋管内光 (淡光流)
}

MAT_UV = {
    "flesh_dark":   [0, 0, 16, 16],
    "flesh_main":   [16, 0, 32, 16],
    "flesh_lit":    [32, 0, 48, 16],
    "bone_main":    [48, 0, 64, 16],
    "bone_dark":    [0, 16, 16, 32],
    "bone_crevice": [16, 16, 32, 32],
    "tendon_tube":  [32, 16, 48, 32],
    "qi_glow":      [48, 16, 64, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_flesh_skirt() -> List[dict]:
    """两侧边缘一圈薄肉沿：附着于地面 y: 0.0..1.8，沿 Z: -8.0..8.0 铺展，X 向外延伸至 ±7.6。"""
    cubes = []

    # ── 左侧肉沿 (x < -4.0) ──
    # 左底座主片 (暗血肉 #5a1a1a)
    cubes.append({
        "name": "flesh_skirt_l_base_f",
        "from": [-7.4, 0.0, 0.0],
        "to":   [-4.0, 0.8, 8.0],
        "group": "flesh_skirt",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "flesh_skirt_l_base_b",
        "from": [-7.6, 0.0, -8.0],
        "to":   [-4.0, 0.8, 0.0],
        "group": "flesh_skirt",
        "material": "flesh_dark",
    })
    # 左侧外沿起伏薄舌 (血肉 #8a2a2a)
    cubes.append({
        "name": "flesh_skirt_l_crest_1",
        "from": [-6.8, 0.8, -7.0],
        "to":   [-4.0, 1.4, -2.0],
        "group": "flesh_skirt",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "flesh_skirt_l_crest_2",
        "from": [-6.5, 0.8, -1.5],
        "to":   [-4.0, 1.5, 3.5],
        "group": "flesh_skirt",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "flesh_skirt_l_crest_3",
        "from": [-7.0, 0.8, 4.0],
        "to":   [-4.0, 1.4, 7.5],
        "group": "flesh_skirt",
        "material": "flesh_main",
    })
    # 左侧肌腱攀附管壁斜角 (亮肉 #b05050)
    cubes.append({
        "name": "flesh_skirt_l_tendon_1",
        "from": [-4.6, 1.4, -5.5],
        "to":   [-3.95, 2.6, -3.5],
        "group": "flesh_skirt",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "flesh_skirt_l_tendon_2",
        "from": [-4.6, 1.5, 1.0],
        "to":   [-3.95, 2.7, 3.0],
        "group": "flesh_skirt",
        "material": "flesh_lit",
    })

    # ── 右侧肉沿 (x > 4.0) ──
    # 右底座主片 (暗血肉 #5a1a1a)
    cubes.append({
        "name": "flesh_skirt_r_base_f",
        "from": [4.0, 0.0, 0.0],
        "to":   [7.6, 0.8, 8.0],
        "group": "flesh_skirt",
        "material": "flesh_dark",
    })
    cubes.append({
        "name": "flesh_skirt_r_base_b",
        "from": [4.0, 0.0, -8.0],
        "to":   [7.4, 0.8, 0.0],
        "group": "flesh_skirt",
        "material": "flesh_dark",
    })
    # 右侧外沿起伏薄舌 (血肉 #8a2a2a)
    cubes.append({
        "name": "flesh_skirt_r_crest_1",
        "from": [4.0, 0.8, -7.5],
        "to":   [6.9, 1.4, -3.5],
        "group": "flesh_skirt",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "flesh_skirt_r_crest_2",
        "from": [4.0, 0.8, -2.5],
        "to":   [6.6, 1.5, 2.5],
        "group": "flesh_skirt",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "flesh_skirt_r_crest_3",
        "from": [4.0, 0.8, 3.0],
        "to":   [6.8, 1.4, 7.0],
        "group": "flesh_skirt",
        "material": "flesh_main",
    })
    # 右侧肌腱攀附管壁斜角 (亮肉 #b05050)
    cubes.append({
        "name": "flesh_skirt_r_tendon_1",
        "from": [3.95, 1.4, -4.0],
        "to":   [4.6, 2.6, -2.0],
        "group": "flesh_skirt",
        "material": "flesh_lit",
    })
    cubes.append({
        "name": "flesh_skirt_r_tendon_2",
        "from": [3.95, 1.5, 3.5],
        "to":   [4.6, 2.7, 5.5],
        "group": "flesh_skirt",
        "material": "flesh_lit",
    })

    return cubes


def part_02_meridian_tube() -> List[dict]:
    """矩形半透明筋管：宽 8 (x: -4..4), 高 6 (y: 2..8), 长 16 (z: -8..8)。
    壁厚 0.8px，材质 tendon_tube (#e8d8d0 半透明)。
    开孔避让顶底两端端口 (z: [-5.7, -3.3] 与 [3.3, 5.7], x: [-1.2, 1.2])。
    """
    cubes = []

    # ── 左侧壁 (x: -4.0..-3.2, y: 2.0..8.0, z: -8.0..8.0) ──
    cubes.append({
        "name": "tube_wall_l_rear",
        "from": [-4.0, 2.0, -8.0],
        "to":   [-3.2, 8.0, -2.5],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_l_mid",
        "from": [-4.0, 2.0, -2.5],
        "to":   [-3.2, 8.0, 2.5],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_l_front",
        "from": [-4.0, 2.0, 2.5],
        "to":   [-3.2, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # ── 右侧壁 (x: 3.2..4.0, y: 2.0..8.0, z: -8.0..8.0) ──
    cubes.append({
        "name": "tube_wall_r_rear",
        "from": [3.2, 2.0, -8.0],
        "to":   [4.0, 8.0, -2.5],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_r_mid",
        "from": [3.2, 2.0, -2.5],
        "to":   [4.0, 8.0, 2.5],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_wall_r_front",
        "from": [3.2, 2.0, 2.5],
        "to":   [4.0, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # ── 底壁 (y: 2.0..2.8, x: -3.2..3.2) ──
    cubes.append({
        "name": "tube_bottom_rear_solid",
        "from": [-3.2, 2.0, -8.0],
        "to":   [3.2, 2.8, -5.7],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_bottom_port_b_left",
        "from": [-3.2, 2.0, -5.7],
        "to":   [-1.2, 2.8, -3.3],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_bottom_port_b_right",
        "from": [1.2, 2.0, -5.7],
        "to":   [3.2, 2.8, -3.3],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_bottom_mid_solid",
        "from": [-3.2, 2.0, -3.3],
        "to":   [3.2, 2.8, 3.3],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_bottom_port_f_left",
        "from": [-3.2, 2.0, 3.3],
        "to":   [-1.2, 2.8, 5.7],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_bottom_port_f_right",
        "from": [1.2, 2.0, 3.3],
        "to":   [3.2, 2.8, 5.7],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_bottom_front_solid",
        "from": [-3.2, 2.0, 5.7],
        "to":   [3.2, 2.8, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    # ── 顶壁 (y: 7.2..8.0, x: -3.2..3.2) ──
    cubes.append({
        "name": "tube_top_rear_solid",
        "from": [-3.2, 7.2, -8.0],
        "to":   [3.2, 8.0, -5.7],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_top_port_b_left",
        "from": [-3.2, 7.2, -5.7],
        "to":   [-1.2, 8.0, -3.3],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_top_port_b_right",
        "from": [1.2, 7.2, -5.7],
        "to":   [3.2, 8.0, -3.3],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_top_mid_solid",
        "from": [-3.2, 7.2, -3.3],
        "to":   [3.2, 8.0, 3.3],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_top_port_f_left",
        "from": [-3.2, 7.2, 3.3],
        "to":   [-1.2, 8.0, 5.7],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_top_port_f_right",
        "from": [1.2, 7.2, 3.3],
        "to":   [3.2, 8.0, 5.7],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })
    cubes.append({
        "name": "tube_top_front_solid",
        "from": [-3.2, 7.2, 5.7],
        "to":   [3.2, 8.0, 8.0],
        "group": "meridian_tube",
        "material": "tendon_tube",
    })

    return cubes


def part_03_inner_qi_glow() -> List[dict]:
    """筋管内光：淡光流动核心 (#f4ece0, qi_glow)，透过半透明管壁呈现晶莹温润真元光流。"""
    cubes = []

    cubes.append({
        "name": "inner_qi_glow_rear",
        "from": [-2.4, 3.6, -7.8],
        "to":   [2.4, 6.4, -3.4],
        "group": "inner_qi_glow",
        "material": "qi_glow",
    })
    cubes.append({
        "name": "inner_qi_glow_mid",
        "from": [-2.5, 3.5, -3.4],
        "to":   [2.5, 6.5, 3.4],
        "group": "inner_qi_glow",
        "material": "qi_glow",
    })
    cubes.append({
        "name": "inner_qi_glow_front",
        "from": [-2.4, 3.6, 3.4],
        "to":   [2.4, 6.4, 7.8],
        "group": "inner_qi_glow",
        "material": "qi_glow",
    })

    return cubes


def part_04_top_ports() -> List[dict]:
    """顶面靠两端各一个圆形端口开口 (骨环基座 #d8ccb0, 四周肉领过渡 #8a2a2a, 中心打通)。"""
    cubes = []

    for p_name, cz in [("rear", -4.5), ("front", 4.5)]:
        # 顶端骨环4边围合 (y: 8.0..9.2, 微凸起 1.2px)
        cubes.append({
            "name": f"port_top_{p_name}_north",
            "from": [-1.9, 8.0, cz - 2.1],
            "to":   [1.9, 9.2, cz - 1.2],
            "group": "top_ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_south",
            "from": [-1.9, 8.0, cz + 1.2],
            "to":   [1.9, 9.2, cz + 2.1],
            "group": "top_ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_west",
            "from": [-2.1, 8.0, cz - 1.2],
            "to":   [-1.2, 9.2, cz + 1.2],
            "group": "top_ports",
            "material": "bone_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_east",
            "from": [1.2, 8.0, cz - 1.2],
            "to":   [2.1, 9.2, cz + 1.2],
            "group": "top_ports",
            "material": "bone_main",
        })
        # 四角斜向肉领过渡 (暗血肉 #8a2a2a)
        cubes.append({
            "name": f"port_top_{p_name}_collar_nw",
            "from": [-2.4, 8.0, cz - 2.4],
            "to":   [-1.9, 8.6, cz - 1.9],
            "group": "top_ports",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_collar_ne",
            "from": [1.9, 8.0, cz - 2.4],
            "to":   [2.4, 8.6, cz - 1.9],
            "group": "top_ports",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_collar_sw",
            "from": [-2.4, 8.0, cz + 1.9],
            "to":   [-1.9, 8.6, cz + 2.4],
            "group": "top_ports",
            "material": "flesh_main",
        })
        cubes.append({
            "name": f"port_top_{p_name}_collar_se",
            "from": [1.9, 8.0, cz + 1.9],
            "to":   [2.4, 8.6, cz + 2.4],
            "group": "top_ports",
            "material": "flesh_main",
        })

    return cubes


def part_05_bottom_ports() -> List[dict]:
    """底面靠两端各一个圆形端口开口：肉质承插环，对应顶面端口通孔。"""
    cubes = []

    for p_name, cz in [("rear", -4.5), ("front", 4.5)]:
        cubes.append({
            "name": f"port_bot_{p_name}_north",
            "from": [-1.8, 0.8, cz - 2.0],
            "to":   [1.8, 2.0, cz - 1.2],
            "group": "bottom_ports",
            "material": "flesh_dark",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_south",
            "from": [-1.8, 0.8, cz + 1.2],
            "to":   [1.8, 2.0, cz + 2.0],
            "group": "bottom_ports",
            "material": "flesh_dark",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_west",
            "from": [-2.0, 0.8, cz - 1.2],
            "to":   [-1.2, 2.0, cz + 1.2],
            "group": "bottom_ports",
            "material": "flesh_dark",
        })
        cubes.append({
            "name": f"port_bot_{p_name}_east",
            "from": [1.2, 0.8, cz - 1.2],
            "to":   [2.0, 2.0, cz + 1.2],
            "group": "bottom_ports",
            "material": "flesh_dark",
        })

    return cubes


def part_06_bone_clamps() -> List[dict]:
    """中段加固骨箍：跨越中段 z: -0.5..0.5，紧扣筋管外周，增添活体经脉工程感。"""
    cubes = []

    # 顶梁横骨 (#d8ccb0 bone_main)
    cubes.append({
        "name": "bone_clamp_top",
        "from": [-4.15, 8.0, -0.5],
        "to":   [4.15, 8.55, 0.5],
        "group": "bone_clamps",
        "material": "bone_main",
    })
    # 左侧立柱加固骨 (#b8a888 bone_dark)
    cubes.append({
        "name": "bone_clamp_col_l",
        "from": [-4.25, 1.8, -0.5],
        "to":   [-4.0, 8.0, 0.5],
        "group": "bone_clamps",
        "material": "bone_dark",
    })
    # 右侧立柱加固骨 (#b8a888 bone_dark)
    cubes.append({
        "name": "bone_clamp_col_r",
        "from": [4.0, 1.8, -0.5],
        "to":   [4.25, 8.0, 0.5],
        "group": "bone_clamps",
        "material": "bone_dark",
    })

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有 6 个部件的立方体。"""
    return (
        part_01_flesh_skirt()
        + part_02_meridian_tube()
        + part_03_inner_qi_glow()
        + part_04_top_ports()
        + part_05_bottom_ports()
        + part_06_bone_clamps()
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
            # 半透明筋管壁：带纵向微粉红筋膜细纹，保持 ~60% 透明度
            for x in range(w):
                fib = rng.randint(-10, 10)
                tile[:, x, 0] = np.clip(tile[:, x, 0].astype(int) + fib, 0, 255)
                tile[:, x, 1] = np.clip(tile[:, x, 1].astype(int) + fib, 0, 255)
                tile[:, x, 2] = np.clip(tile[:, x, 2].astype(int) + fib, 0, 255)
                tile[:, x, 3] = np.clip(base_color[3] + rng.randint(-8, 8), 130, 180)
        elif mat_name == "qi_glow":
            # 晶莹淡光核心：中心稍亮向外微晕
            for y in range(h):
                for x in range(w):
                    r_dist = np.hypot(x - w / 2, y - h / 2) / (w / 2)
                    glow = int(12 * (1.0 - np.clip(r_dist, 0.0, 1.0)))
                    tile[y, x, :3] = np.clip(tile[y, x, :3].astype(int) + glow, 0, 255)
        elif "flesh" in mat_name:
            # 活体血肉微起伏噪声
            noise = rng.randint(-14, 14, size=(h, w))
            for c in range(3):
                tile[:, :, c] = np.clip(tile[:, :, c].astype(int) + noise, 0, 255)
        elif "bone" in mat_name:
            # 骨质斑驳与微裂隙
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
        mat = c.get("material", "flesh_dark")
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
    # 正视 (看 +Z 输出端截面)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    # 侧视 (看整个 16px 长管道侧向身姿)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    # 3/4 等轴透视
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    # 俯视 (看顶面两端端口与两侧肉沿展开)
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
        ("TOP (Dual Ports)", im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 210, py + 26], fill=(24, 25, 28))
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
        "to":   [-3.2, 8.0, -2.5],
        "material": "flesh_dark",
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
