#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o05: heart (心脏)

风格：A 有机型 (活体血肉、君主之官真元余烬火光、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

结构与规范落实：
1. 外包围与尺寸：2 宽 × 3 高 × 2 深方块 (严格落在 32×48×32 px 空间内：x in [-16..16], y in [0..48], z in [-16..16])，
   原点位于底面中心 (0.0, 0.0, 0.0)。
2. 心脏主体 (逐行宽度表，上宽下尖、上沿双圆鼓)：
   - 心尖收拢于底部 (y: 6..14)；
   - 心室沿左右外扩膨大 (y: 14..28，最宽处达 x in [-15.5, 15.5]，宽 31px，厚 28px)；
   - 心房双穹顶圆鼓 (y: 34..40)，中间设心凹中缝，左右两心房各呈圆拱饱满收尖。
3. 顶部 3 根短大血管管 (6×6 截面，高 6~8px，带骨环口与深色内孔)：
   - 左血管管: x in [-10.5, -4.5], y: 39.5..47.5；
   - 中主动脉管: x in [-3.0, 3.0], y: 33.2..48.0；
   - 右血管管: x in [4.5, 10.5], y: 39.5..47.0；
   - 每根管顶配有 #d8ccb0 骨环口外箍。
4. 胸前 6×6 区域 #c84a3a 余烬光 (火属性核心)：
   - 位于心体正面前突区 (y: 20.0..26.0, x: -3.0..3.0, z: 14.0..14.25)，中心带 #e06850 高光核心。
5. 表面 3~4 条 #d8ccb0 冠状血管纹：
   - 骨质冠状脉网从心凹中缝斜向左右心室前表面蜿蜒分叉而下 (浮起 0.35px)。
6. 底尖处 8×6 输出端口 (统一连接截面)：
   - 内孔宽 8px × 高 6px、居中 x in [-4, 4]、底边离地 2px y in [2, 8]；
   - 外包 2px 厚肉质管套 #8a2a2a (底边贴地 y=0.0)，朝正面 +Z 延伸至 z = 16.0；
   - 内壁衬 #b05050 亮肉衬层与 #f6dcc4 真元内光。
7. 门禁自证全绿 (0 共面冲突，--self-test 缺陷拦截验证 PASS)。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
BBMODEL_OUT = MODEL_DIR / "heart.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/heart")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o05_heart")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o05_heart.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_main":    (138, 42, 42, 255),   # #8a2a2a 红心肌主色与管套
    "flesh_lit":     (176, 80, 80, 255),   # #b05050 心肌斑纹与方口内衬
    "flesh_dark":    (90, 26, 26, 255),    # #5a1a1a 心肌暗部与管口内底
    "bone_main":     (216, 204, 176, 255), # #d8ccb0 骨环口与冠状血管纹
    "bone_dark":     (184, 168, 136, 255), # #b8a888 骨暗面
    "ember_fire":    (200, 74, 58, 255),   # #c84a3a 胸前余烬真元火光
    "ember_core":    (224, 104, 80, 255),  # #e06850 余烬中心高光点
    "qi_glow":       (246, 220, 196, 255), # #f6dcc4 底口真元内核
}

MAT_UV = {
    "flesh_main":    [0,  0, 16, 16],
    "flesh_lit":     [16, 0, 32, 16],
    "flesh_dark":    [32, 0, 48, 16],
    "bone_main":     [48, 0, 64, 16],
    "bone_dark":     [0, 16, 16, 32],
    "ember_fire":    [16, 16, 32, 32],
    "ember_core":    [32, 16, 48, 32],
    "qi_glow":       [48, 16, 64, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_top_vessels() -> List[dict]:
    """1. 顶部 3 根短血管 (6x6 截面，高 6~8，顶端带骨环口与深色孔腔)。"""
    cubes = []

    # ── 左侧大血管 (x in [-10.5, -4.5], y: 39.5..47.5, z in [-3.0, 3.0]) ──
    cubes.append({"name": "vessel_l_body",  "from": [-10.5, 39.5, -3.0], "to": [-4.5, 45.8,  3.0], "group": "top_vessels", "material": "flesh_main"})
    cubes.append({"name": "vessel_l_rim_w", "from": [-10.5, 45.8, -3.0], "to": [-9.5, 47.5,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_l_rim_e", "from": [ -5.5, 45.8, -3.0], "to": [-4.5, 47.5,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_l_rim_n", "from": [ -9.5, 45.8, -3.0], "to": [-5.5, 47.5, -2.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_l_rim_s", "from": [ -9.5, 45.8,  2.0], "to": [-5.5, 47.5,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_l_lumen", "from": [ -9.5, 45.0, -2.0], "to": [-5.5, 46.5,  2.0], "group": "top_vessels", "material": "flesh_dark"})

    # ── 中央主动脉 (x in [-3.0, 3.0], y: 33.2..48.0, z in [-3.0, 3.0]) ──
    cubes.append({"name": "vessel_m_body",  "from": [-3.0, 33.2, -3.0], "to": [ 3.0, 46.2,  3.0], "group": "top_vessels", "material": "flesh_main"})
    cubes.append({"name": "vessel_m_rim_w", "from": [-3.0, 46.2, -3.0], "to": [-2.0, 48.0,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_m_rim_e", "from": [ 2.0, 46.2, -3.0], "to": [ 3.0, 48.0,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_m_rim_n", "from": [-2.0, 46.2, -3.0], "to": [ 2.0, 48.0, -2.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_m_rim_s", "from": [-2.0, 46.2,  2.0], "to": [ 2.0, 48.0,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_m_lumen", "from": [-2.0, 45.5, -2.0], "to": [ 2.0, 47.0,  2.0], "group": "top_vessels", "material": "flesh_dark"})

    # ── 右侧大血管 (x in [4.5, 10.5], y: 39.5..47.0, z in [-3.0, 3.0]) ──
    cubes.append({"name": "vessel_r_body",  "from": [ 4.5, 39.5, -3.0], "to": [10.5, 45.4,  3.0], "group": "top_vessels", "material": "flesh_main"})
    cubes.append({"name": "vessel_r_rim_w", "from": [ 4.5, 45.4, -3.0], "to": [ 5.5, 47.0,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_r_rim_e", "from": [ 9.5, 45.4, -3.0], "to": [10.5, 47.0,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_r_rim_n", "from": [ 5.5, 45.4, -3.0], "to": [ 9.5, 47.0, -2.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_r_rim_s", "from": [ 5.5, 45.4,  2.0], "to": [ 9.5, 47.0,  3.0], "group": "top_vessels", "material": "bone_main"})
    cubes.append({"name": "vessel_r_lumen", "from": [ 5.5, 44.5, -2.0], "to": [ 9.5, 46.0,  2.0], "group": "top_vessels", "material": "flesh_dark"})

    return cubes


def part_02_heart_body() -> List[dict]:
    """2. 心脏主体：逐行宽度表 (上宽下尖、心房双穹顶鼓起)。"""
    cubes = []
    # 逐行参数定义：(行号, y0, y1, [ (x0, x1, z0, z1), ... ])
    rows_def = [
        # 行 0..3: 心尖下部，向底尖输出口收拢
        ( 0,  6.0,  8.0, [(-4.0,  4.0, -8.0,  8.0)]),
        ( 1,  8.0, 10.0, [(-5.5,  5.5, -9.0,  8.8)]),
        ( 2, 10.0, 12.0, [(-7.0,  7.0, -10.0, 10.0)]),
        ( 3, 12.0, 14.0, [(-8.5,  8.5, -11.0, 11.0)]),
        # 行 4..7: 心室中下段，梯级外扩
        ( 4, 14.0, 16.0, [(-10.0, 10.0, -12.0, 12.0)]),
        ( 5, 16.0, 18.0, [(-11.5, 11.5, -13.0, 13.0)]),
        ( 6, 18.0, 20.0, [(-13.0, 13.0, -13.0, 13.0)]),
        ( 7, 20.0, 22.0, [(-14.0, 14.0, -14.0, 14.0)]),
        # 行 8..10: 心肌最宽心腰部 (宽 30~31px, 厚 28px)
        ( 8, 22.0, 24.0, [(-15.0, 15.0, -14.0, 14.0)]),
        ( 9, 24.0, 26.0, [(-15.5, 15.5, -14.0, 14.0)]),
        (10, 26.0, 28.0, [(-15.5, 15.5, -14.0, 14.0)]),
        # 行 11..13: 心房过渡段
        (11, 28.0, 30.0, [(-15.0, 15.0, -13.0, 13.0)]),
        (12, 30.0, 32.0, [(-14.5, 14.5, -13.0, 13.0)]),
        (13, 32.0, 34.0, [(-14.0, 14.0, -12.0, 12.0)]),
        # 行 14..16: 双心房双穹顶鼓起 (中空留出心凹中缝)
        (14, 34.0, 36.0, [(-13.5, -1.0, -11.0, 11.0), (1.0, 13.5, -11.0, 11.0)]),
        (15, 36.0, 38.0, [(-12.0, -2.0,  -9.0,  9.0), (2.0, 12.0,  -9.0,  9.0)]),
        (16, 38.0, 40.0, [(-10.0, -4.0,  -7.0,  7.0), (4.0, 10.0,  -7.0,  7.0)]),
    ]

    for r_idx, y0, y1, segments in rows_def:
        for s_idx, (x0, x1, z0, z1) in enumerate(segments):
            mat = "flesh_main" if (r_idx + s_idx) % 2 == 0 else "flesh_lit"
            cubes.append({
                "name": f"sac_r{r_idx}_s{s_idx}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "heart_body",
                "material": mat,
            })

    return cubes


def part_03_ember_glow() -> List[dict]:
    """3. 胸前 6×6 余烬光（火属性真元晶核，#c84a3a，中心高光点 #e06850）。"""
    cubes = []
    # 6x6 余烬火光区域 (x in [-3, 3], y in [20, 26], 浮于心肌前表面 z=14.0 处 0.25px)
    cubes.append({
        "name": "ember_glow_core",
        "from": [-3.0, 20.0, 14.0],
        "to":   [ 3.0, 26.0, 14.25],
        "group": "ember_glow",
        "material": "ember_fire",
    })
    # 2x2 中心高光点 (浮起 0.45px)
    cubes.append({
        "name": "ember_glow_highlight",
        "from": [-1.0, 22.0, 14.25],
        "to":   [ 1.0, 24.0, 14.45],
        "group": "ember_glow",
        "material": "ember_core",
    })
    return cubes


def part_04_coronary_veins() -> List[dict]:
    """4. 表面 3~4 条 #d8ccb0 冠状血管纹 (浮起 0.35px，自心顶中凹向左右心室与心尖蜿蜒)。"""
    cubes = []

    # 冠状主干 1 (向左心室前壁蜿蜒至心腰)
    c1 = [
        (-0.5,  0.5, 32.2, 34.0, 12.0, 12.35),
        (-2.0, -0.5, 29.2, 32.2, 13.0, 13.35),
        (-4.8, -3.1, 25.2, 29.2, 14.0, 14.35),
        (-7.0, -4.8, 21.2, 25.2, 14.0, 14.35),
        (-9.0, -7.0, 16.2, 21.2, 13.0, 13.35),
    ]
    for idx, (x0, x1, y0, y1, z0, z1) in enumerate(c1):
        cubes.append({"name": f"coronary_1_{idx}", "from": [x0, y0, z0], "to": [x1, y1, z1], "group": "coronary_veins", "material": "bone_main"})

    # 冠状分支 2 (向右心室前壁蜿蜒)
    c2 = [
        (0.5, 2.5, 29.2, 32.2, 13.0, 13.35),
        (3.1, 5.0, 25.2, 29.2, 14.0, 14.35),
        (5.0, 7.5, 21.2, 25.2, 14.0, 14.35),
        (7.5, 9.5, 17.2, 21.2, 13.0, 13.35),
    ]
    for idx, (x0, x1, y0, y1, z0, z1) in enumerate(c2):
        cubes.append({"name": f"coronary_2_{idx}", "from": [x0, y0, z0], "to": [x1, y1, z1], "group": "coronary_veins", "material": "bone_main"})

    # 冠状分支 3 (自左侧分叉向心尖前缘延伸)
    c3 = [
        (-4.0, -2.5, 18.2, 22.2, 13.0, 13.35),
        (-2.5, -1.0, 13.2, 18.2, 11.0, 11.35),
        (-1.0,  0.5,  8.2, 13.2,  9.0,  9.35),
    ]
    for idx, (x0, x1, y0, y1, z0, z1) in enumerate(c3):
        cubes.append({"name": f"coronary_3_{idx}", "from": [x0, y0, z0], "to": [x1, y1, z1], "group": "coronary_veins", "material": "bone_main"})

    return cubes


def part_05_output_port() -> List[dict]:
    """5. 底尖处 8x6 输出端口 (统一连接截面：宽 8, 高 6, 底边离地 2px, z 延伸至 16.0)。"""
    cubes = []
    # 肉质管套外壳 (x in [-6, 6], y in [0, 10], z in [8.85, 16.0], 底边贴地 y=0.0)
    cubes.append({"name": "port_sleeve_base", "from": [-6.0, 0.0,  8.85], "to": [6.0, 2.0, 16.0], "group": "output_port", "material": "flesh_main"})
    cubes.append({"name": "port_sleeve_top",  "from": [-6.0, 8.0,  8.85], "to": [6.0, 9.7, 16.0], "group": "output_port", "material": "flesh_main"})
    cubes.append({"name": "port_sleeve_l",    "from": [-6.0, 2.0,  8.85], "to": [-4.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    cubes.append({"name": "port_sleeve_r",    "from": [ 4.0, 2.0,  8.85], "to": [ 6.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})

    # 8x6 方口内衬 (#b05050)
    cubes.append({"name": "port_lining_l", "from": [-4.0, 2.0, 14.8], "to": [-3.2, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_r", "from": [ 3.2, 2.0, 14.8], "to": [ 4.0, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_t", "from": [-3.2, 7.2, 14.8], "to": [ 3.2, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_b", "from": [-3.2, 2.0, 14.8], "to": [ 3.2, 2.8, 16.0], "group": "output_port", "material": "flesh_lit"})

    # 真元光流内核 (#f6dcc4)
    cubes.append({"name": "port_lumen_glow", "from": [-3.2, 2.8, 12.0], "to": [ 3.2, 7.2, 14.8], "group": "output_port", "material": "qi_glow"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总心脏全部 5 大部件立方体。"""
    return (
        part_01_top_vessels()
        + part_02_heart_body()
        + part_03_ember_glow()
        + part_04_coronary_veins()
        + part_05_output_port()
    )


# =============================================================================
# 门禁与共面冲突自检
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """严格检查立方体集是否存在同向同坐标且投影相交的共面冲突 (Z-fighting)。"""
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
                rect = (min(f[1], t[1]), min(f[2], t[2]), max(f[1], t[1]), max(f[2], t[2]))
            elif axis == 1:
                rect = (min(f[0], t[0]), min(f[2], t[2]), max(f[0], t[0]), max(f[2], t[2]))
            else:
                rect = (min(f[0], t[0]), min(f[1], t[1]), max(f[0], t[0]), max(f[1], t[1]))
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
# 贴图与 Blockbench 序列化
# =============================================================================

def build_texture(res: int = RES) -> Image.Image:
    """生成 64×64 RGBA 贴图。"""
    rng = np.random.default_rng(20261011)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    for mat_name, (u0, v0, u1, v1) in MAT_UV.items():
        base_c = PALETTE[mat_name]
        for y in range(v0, v1):
            for x in range(u0, u1):
                noise = rng.integers(-4, 5)
                r = int(np.clip(base_c[0] + noise, 0, 255))
                g = int(np.clip(base_c[1] + noise, 0, 255))
                b = int(np.clip(base_c[2] + noise, 0, 255))

                if mat_name == "flesh_main":
                    # 心肌纤维丝缕
                    if (x + y * 2) % 5 == 0:
                        r = int(np.clip(r + 10, 0, 255))
                        g = int(np.clip(g + 4, 0, 255))
                elif mat_name == "ember_fire":
                    # 余烬微粒跃动光斑
                    if (x * 3 + y * 5) % 7 in (0, 1):
                        r = int(np.clip(r + 18, 0, 255))
                        g = int(np.clip(g + 12, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "heart",
        "folder": "meridian_factory",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": texture_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: Dict[str, List[str]] = {}

    for idx, c in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        g_name = c.get("group", "heart")
        groups_map.setdefault(g_name, []).append(elem_uuid)

        mat_key = c.get("material", "flesh_main")
        u0, v0, u1, v1 = MAT_UV.get(mat_key, [0, 0, 16, 16])
        span_x = max(1, u1 - u0 - 4)
        span_y = max(1, v1 - v0 - 4)
        uv_u = u0 + (idx * 2) % span_x
        uv_v = v0 + (idx * 3) % span_y
        uv_box = [float(uv_u), float(uv_v), float(uv_u + 4), float(uv_v + 4)]

        faces = {
            face_name: {"uv": uv_box, "texture": 0}
            for face_name in ("north", "east", "south", "west", "up", "down")
        }

        element = {
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in c["from"]],
            "to":   [round(float(v), 4) for v in c["to"]],
            "autouv": 0,
            "color": 0,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        elements.append(element)

    outliner = []
    for g_name in ["top_vessels", "heart_body", "ember_glow", "coronary_veins", "output_port"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [0.0, 0.0, 0.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "Heart",
        "model_identifier": "heart",
        "visible_box": [2, 3, 2],
        "geometry_name": "heart",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_bbmodel(out_path: Path = BBMODEL_OUT) -> Path:
    """执行标准生成流程并落盘 bbmodel。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    tex = build_texture()
    doc = build_bbmodel_doc(cubes, tex)

    out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    rel = out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path
    print(f"✓ 心脏器官 bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出三视角拼图 render.png (正视 + 3/4 + 侧视) 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 渲染三视角 (正面直视、3/4 等轴视、侧视图)
    im_front, _ = render(bbmodel_path, yaw=0.0,   pitch=0.0,  size=500, bg=bg_color)
    im_iso, _   = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_side, _  = render(bbmodel_path, yaw=90.0,  pitch=0.0,  size=500, bg=bg_color)

    # 2. 拼装 render.png (1 行 3 列并排大图)
    item_w = 460
    item_h = 460
    pad = 16
    margin_x = 24
    margin_y = 36

    total_w = margin_x * 2 + 3 * item_w + 2 * pad
    total_h = margin_y * 2 + item_h + 20

    canvas = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    draw.text((margin_x, 12), "o05 heart (Left: FRONT View; Middle: 3/4 ISOMETRIC View; Right: SIDE View)", fill=(230, 230, 230))

    views = [
        ("FRONT VIEW (Ember Glow & Coronary Veins)", im_front),
        ("3/4 ISOMETRIC VIEW (Full Anatomy & Atria)", im_iso),
        ("SIDE VIEW (Depth Profile & Port)", im_side),
    ]

    for idx, (title, im_v) in enumerate(views):
        px = margin_x + idx * (item_w + pad)
        py = margin_y + 14
        scaled_v = im_v.resize((item_w, item_h), Image.Resampling.LANCZOS)
        canvas.paste(scaled_v, (px, py))
        draw.rectangle([px, py, px + item_w, py + 24], fill=(24, 25, 28))
        draw.text((px + 8, py + 5), title, fill=(220, 220, 220))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 3. 拼装 check.png (左参考图，右渲染图等高并排对标)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 560
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        item_chk_w = int(target_h * 0.75)
        right_sub_w = item_chk_w * 2 + 16
        right_cv = Image.new("RGB", (right_sub_w, target_h), (119, 119, 119))

        scaled_front_chk = im_front.resize((item_chk_w, item_chk_w), Image.Resampling.LANCZOS)
        scaled_iso_chk   = im_iso.resize((item_chk_w, item_chk_w), Image.Resampling.LANCZOS)

        ry = (target_h - item_chk_w) // 2
        right_cv.paste(scaled_front_chk, (0, ry))
        right_cv.paste(scaled_iso_chk,   (item_chk_w + 16, ry))

        total_w_check = ref_w + right_sub_w + 32
        check_cv = Image.new("RGB", (total_w_check, target_h + 36), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 28))
        c_draw.text((16, 6), "REFERENCE (o05_heart.png: Left FRONT / Right 3/4)", fill=(210, 200, 180))

        check_cv.paste(right_cv, (ref_w + 20, 28))
        c_draw.text((ref_w + 20, 6), "NOW RENDER (FRONT + 3/4 Views Parallel)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_heart.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-3.0, 20.0, 14.0],
        "to":   [ 3.0, 26.0, 14.25],  # 与 ember_glow_core 完全重叠
        "material": "ember_fire",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_heart.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o05 heart 生成器")
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
