#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o02: stomach (胃)

风格：A 有机型 (活体血肉、半透明筋管、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

调度审第 1 次修改落实：
1. 顶部入口管（肉质食管）：
   - 彻底移除骨环气管，改为 #8a2a2a 肉质圆管（6×6 截面，x in [6.0, 12.0], z in [-3.0, 3.0]）；
   - 从胃囊右上方向上伸出约 8px 直达 y = 48.0；
   - 顶端设有 2×2 空心开口与深色孔底；
   - 配有一条从顶延伸而下的 #e8bca8 亮线内衬。
2. 胃囊逐行轮廓（外包 48×48，每行 3px 高，从上往下严格按调度坐标建）：
   - 行 0–3 (y: 36..48)：只有食管，x_disp 30–36 (x_ours: 6..12)；
   - 行 4 (y: 33..36)：x_disp 18–40 (x_ours: -6..16)；
   - 行 5 (y: 30..33)：x_disp 12–42 (x_ours: -12..18)；
   - 行 6 (y: 27..30)：x_disp 8–44 (x_ours: -16..20)；
   - 行 7 (y: 24..27)：x_disp 6–45 (x_ours: -18..21)；
   - 行 8 (y: 21..24)：x_disp 4–45 (x_ours: -20..21)；
   - 行 9 (y: 18..21)：x_disp 4–44 (x_ours: -20..20)；
   - 行 10 (y: 15..18)：x_disp 4–43 (x_ours: -20..19)；
   - 行 11 (y: 12..15)：x_disp 5–41 (x_ours: -19..17)；
   - 行 12 (y: 9..12)：x_disp 7–38 (x_ours: -17..14)；
   - 行 13 (y: 6..9)：x_disp 10–34 (x_ours: -14..10)；
   - 前后厚度随高度变化：中间几行 28px (z in [-14, 14])，上下各收到 20~24px。
3. 正面椭圆凹口露出琥珀色消化液：
   - 位于行 6–11、x_disp 12–36 (x_ours: -12..12)，向内凹 3px；
   - 凹底为充满活性的琥珀色胃酸消化液 #c88a30（带 #a86a20 斑驳）；
   - 开口一圈配置 1px #e8bca8 亮线内衬。
4. 沿外表弧线的完整 #e8bca8 亮衬线：
   - 从食管右上方起步，沿胃大弯弧线一路向下弯转，直贯幽门出口。
5. 幽门出口与 8×6 统一截面输出端口：
   - 行 12–14 从 x_disp 30 向右伸到 x_disp 46 (x_ours: 6..22)，管径 6 (y: 2..8)；
   - 末端在 x_ours: 14..22 配置 8×6 统一截面的输出端口，外包 2px 厚肉质管套 (#8a2a2a，x: 12..24, y: 0..10)；
   - 端口向正面 +Z 延伸至 z = 16.0，方口内壁衬亮肉衬层 (#b05050) 与琥珀流光内芯；
   - 彻底删除底部红色底板。
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
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "stomach.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/stomach")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/o02_stomach")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/o02_stomach.png")

# 调色板 (严格对齐 meridian_factory.md 色值表)
PALETTE = {
    "flesh_dark":       (90, 26, 26, 255),    # #5a1a1a 暗血肉
    "flesh_main":       (138, 42, 42, 255),   # #8a2a2a 血肉主色 (肉质食管、胃壁、肉套)
    "flesh_lit":        (176, 80, 80, 255),   # #b05050 亮肉 / 黏膜粉 (方口内壁)
    "tendon_highlight": (232, 188, 168, 255), # #e8bca8 亮线内衬 / 外表弧形亮线
    "amber_fluid":      (200, 138, 48, 255),  # #c88a30 胃液琥珀 (消化腔液)
    "amber_dark":       (168, 106, 32, 255),  # #a86a20 消化液深斑
    "qi_glow":          (246, 220, 196, 255), # #f6dcc4 端口光流
}

MAT_UV = {
    "flesh_main":       [0, 0, 16, 16],
    "flesh_lit":        [16, 0, 32, 16],
    "flesh_dark":       [32, 0, 48, 16],
    "tendon_highlight": [48, 0, 64, 16],
    "amber_fluid":      [0, 16, 16, 32],
    "amber_dark":       [16, 16, 32, 32],
    "qi_glow":          [32, 16, 48, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_esophagus() -> List[dict]:
    """1. 顶部入口管：肉质食管 (#8a2a2a, 圆管 6x6, 伸出约 8px, 顶端开口, #e8bca8 内衬亮线)。"""
    cubes = []
    # 顶端空心开口外壁 (y: 46.8..48.0, x: 6.0..12.0, z: -3.0..3.0)
    cubes.append({"name": "esophagus_rim_n", "from": [ 6.0, 46.8, -3.0], "to": [12.0, 48.0, -1.0], "group": "esophagus", "material": "flesh_main"})
    cubes.append({"name": "esophagus_rim_s", "from": [ 6.0, 46.8,  1.0], "to": [12.0, 48.0,  3.0], "group": "esophagus", "material": "flesh_main"})
    cubes.append({"name": "esophagus_rim_w", "from": [ 6.0, 46.8, -1.0], "to": [ 8.0, 48.0,  1.0], "group": "esophagus", "material": "flesh_main"})
    cubes.append({"name": "esophagus_rim_e", "from": [10.0, 46.8, -1.0], "to": [12.0, 48.0,  1.0], "group": "esophagus", "material": "flesh_main"})
    # 开口内腔深色孔底
    cubes.append({"name": "esophagus_stoma_floor", "from": [8.0, 45.6, -1.0], "to": [10.0, 46.6, 1.0], "group": "esophagus", "material": "flesh_dark"})

    # 肉质食管主体 (y: 36.0..46.8, 6x6 截面)
    cubes.append({"name": "esophagus_body_upper", "from": [6.0, 41.0, -3.0], "to": [12.0, 46.8, 3.0], "group": "esophagus", "material": "flesh_main"})
    cubes.append({"name": "esophagus_body_lower", "from": [5.8, 36.0, -3.2], "to": [12.2, 41.0, 3.2], "group": "esophagus", "material": "flesh_main"})

    # 食管内衬亮线 (#e8bca8, 沿食管正面直下, 宽 1px)
    cubes.append({"name": "esophagus_lining_line", "from": [8.5, 36.5, 3.1], "to": [9.5, 47.5, 3.7], "group": "esophagus", "material": "tendon_highlight"})

    return cubes


def part_02_stomach_sac() -> List[dict]:
    """2. 胃囊肌体：严格按调度审定逐行左右边界建 (行 4..13, 每行 3px 高)。"""
    cubes = []
    # 逐行参数：(行号, y0, y1, x0_disp, x1_disp, depth)
    rows_def = [
        ( 4, 33.0, 36.0, 18, 40, 20.0),
        ( 5, 30.0, 33.0, 12, 42, 22.0),
        ( 6, 27.0, 30.0,  8, 44, 28.0),
        ( 7, 24.0, 27.0,  6, 45, 28.0),
        ( 8, 21.0, 24.0,  4, 45, 28.0),
        ( 9, 18.0, 21.0,  4, 44, 28.0),
        (10, 15.0, 18.0,  4, 43, 26.0),
        (11, 12.0, 15.0,  5, 41, 24.0),
        (12,  9.0, 12.0,  7, 38, 22.0),
        (13,  6.0,  9.0, 10, 34, 20.0),
    ]

    # 视窗椭圆在各行 (行 6..11) 的 X 范围 (圆化椭圆：上下两端收窄，中央展开)
    window_x_disp = {
        6:  (20.0, 28.0),  # 顶端收窄
        7:  (15.0, 33.0),
        8:  (12.0, 36.0),
        9:  (12.0, 36.0),
        10: (15.0, 33.0),
        11: (20.0, 28.0),  # 底端收窄
    }

    for r_idx, y0, y1, x0_d, x1_d, depth in rows_def:
        x0 = float(x0_d - 24)
        x1 = float(x1_d - 24)
        hz = depth / 2.0
        z0 = -hz
        z1 = hz

        if r_idx in window_x_disp:
            wx0_d, wx1_d = window_x_disp[r_idx]
            wx0 = float(wx0_d - 24)
            wx1 = float(wx1_d - 24)

            # 背侧主肌体 (从 z0 到凹槽前壁 z1 - 3.0)
            cubes.append({
                "name": f"sac_row_{r_idx}_back",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1 - 3.0],
                "group": "stomach_sac",
                "material": "flesh_main" if r_idx % 2 == 0 else "flesh_dark",
            })
            # 左侧肌柱
            if wx0 > x0:
                cubes.append({
                    "name": f"sac_row_{r_idx}_pillar_l",
                    "from": [x0, y0, z1 - 3.0],
                    "to":   [wx0, y1, z1],
                    "group": "stomach_sac",
                    "material": "flesh_main",
                })
            # 右侧肌柱
            if x1 > wx1:
                cubes.append({
                    "name": f"sac_row_{r_idx}_pillar_r",
                    "from": [wx1, y0, z1 - 3.0],
                    "to":   [x1, y1, z1],
                    "group": "stomach_sac",
                    "material": "flesh_main",
                })
        else:
            # 完整肌囊行
            cubes.append({
                "name": f"sac_row_{r_idx}",
                "from": [x0, y0, z0],
                "to":   [x1, y1, z1],
                "group": "stomach_sac",
                "material": "flesh_main" if r_idx % 2 == 0 else "flesh_lit",
            })

    # ── 左下向上勾的胃底大弯加强肌束 (强化 J 形大弯回勾轮廓) ──
    cubes.append({
        "name": "great_curvature_hook_1",
        "from": [-22.0, 4.8, -8.0],
        "to":   [-14.5, 9.2,  8.0],
        "group": "stomach_sac",
        "material": "flesh_main",
    })
    cubes.append({
        "name": "great_curvature_hook_2",
        "from": [-23.6, 8.2, -7.0],
        "to":   [-17.2, 13.5, 7.0],
        "group": "stomach_sac",
        "material": "flesh_lit",
    })

    return cubes


def part_03_amber_cavity() -> List[dict]:
    """3. 正面椭圆凹口与琥珀色消化液 (行 6–11、x 12–36，向内凹 3px，开口 1px #e8bca8 内衬)。"""
    cubes = []
    window_x_disp = {
        6:  (20.0, 28.0, 27.0, 30.0, 14.0),
        7:  (15.0, 33.0, 24.0, 27.0, 14.0),
        8:  (12.0, 36.0, 21.0, 24.0, 14.0),
        9:  (12.0, 36.0, 18.0, 21.0, 14.0),
        10: (15.0, 33.0, 15.0, 18.0, 13.0),
        11: (20.0, 28.0, 12.0, 15.0, 12.0),
    }

    for r_idx, (wx0_d, wx1_d, y0, y1, hz) in window_x_disp.items():
        wx0 = float(wx0_d - 24)
        wx1 = float(wx1_d - 24)
        z1 = float(hz)

        # 凹口内琥珀色消化液 (向内凹 3px 凹底，厚度 2.2px: z in [z1 - 3.0, z1 - 0.8])
        cubes.append({
            "name": f"amber_fluid_row_{r_idx}",
            "from": [wx0 + 0.5, y0 + 0.2, z1 - 3.0],
            "to":   [wx1 - 0.5, y1 - 0.2, z1 - 0.8],
            "group": "amber_cavity",
            "material": "amber_fluid",
        })
        # 视窗开口 1px 亮线内衬 (#e8bca8)
        cubes.append({
            "name": f"amber_lining_l_{r_idx}",
            "from": [wx0 + 0.05, y0 + 0.15, z1 - 0.7],
            "to":   [wx0 + 0.85, y1 - 0.15, z1 + 0.3],
            "group": "amber_cavity",
            "material": "tendon_highlight",
        })
        cubes.append({
            "name": f"amber_lining_r_{r_idx}",
            "from": [wx1 - 0.85, y0 + 0.15, z1 - 0.7],
            "to":   [wx1 - 0.05, y1 - 0.15, z1 + 0.3],
            "group": "amber_cavity",
            "material": "tendon_highlight",
        })

    return cubes


def part_04_curved_accent_line() -> List[dict]:
    """4. 外表沿弧线 #e8bca8 亮线 (从食管一直弯到幽门)。"""
    cubes = []
    curve_points = [
        ( 8.5, 34.0, 10.4), # 食管交界
        ( 3.0, 32.5, 11.4), # 上胃壁
        (-3.0, 30.0, 14.4), # 大弯上弧
        (-9.0, 27.0, 14.4), # 大弯左上
        (-14.0, 23.0, 14.4),# 大弯最凸点外侧
        (-15.0, 18.0, 14.4),# 大弯左下弧
        (-12.0, 14.0, 13.4),# 胃体下弯
        (-5.0, 11.0, 11.4), # 胃窦底部
        ( 2.0,  8.5,  9.4), # 转向幽门
        ( 8.0,  6.5,  8.4), # 幽门管道
        (14.0,  5.0,  8.4), # 进入输出套管
    ]
    for i in range(len(curve_points) - 1):
        p1 = curve_points[i]
        p2 = curve_points[i + 1]
        z_offset = 0.2 if i % 2 == 0 else 0.35
        z_depth = 0.55 if i % 2 == 0 else 0.45
        z_base = max(p1[2], p2[2])
        x_min = min(p1[0], p2[0]) - (0.4 if i % 2 == 0 else 0.35)
        x_max = max(p1[0], p2[0]) + (0.4 if i % 2 == 0 else 0.35)
        y_min = min(p1[1], p2[1]) - (0.4 if i % 2 == 0 else 0.35)
        y_max = max(p1[1], p2[1]) + (0.4 if i % 2 == 0 else 0.35)
        cubes.append({
            "name": f"sac_accent_line_{i}",
            "from": [round(x_min, 2), round(y_min, 2), round(z_base + z_offset, 2)],
            "to":   [round(x_max, 2), round(y_max, 2), round(z_base + z_offset + z_depth, 2)],
            "group": "curved_accent_line",
            "material": "tendon_highlight",
        })

    return cubes


def part_05_output_port() -> List[dict]:
    """5. 幽门出口与 8x6 统一截面输出端口 (管径 6，末端为 8x6 统一截面 + 2px 肉质管套)。"""
    cubes = []
    # 幽门导管 (行 12-14 从 x 30 向右伸到 x 46，管径 6，y: 2..8, z: -3..3)
    cubes.append({"name": "pylorus_duct_core", "from": [6.0, 2.0, -3.0], "to": [14.0, 8.0, 3.0], "group": "output_port", "material": "flesh_main"})

    # 统一接口截面：宽 8px x 高 6px, 底边离地 2px (x: 14.0..22.0, y: 2.0..8.0)
    # 外包 2px 厚肉质管套 (#8a2a2a): x in [12.0, 24.0], y in [0.0, 10.0], z in [8.0, 16.0]
    cubes.append({"name": "port_sleeve_l", "from": [12.0, 2.0,  8.0], "to": [14.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    cubes.append({"name": "port_sleeve_r", "from": [22.0, 2.0,  8.0], "to": [24.0, 8.0, 16.0], "group": "output_port", "material": "flesh_main"})
    cubes.append({"name": "port_sleeve_t", "from": [12.0, 8.0,  8.0], "to": [24.0, 10.0, 16.0], "group": "output_port", "material": "flesh_main"})
    cubes.append({"name": "port_sleeve_b", "from": [12.0, 0.0,  8.0], "to": [24.0, 2.0, 16.0], "group": "output_port", "material": "flesh_main"})

    # 亮肉内壁衬层 (方口内壁 #b05050)
    cubes.append({"name": "port_lining_l", "from": [14.0, 2.0, 14.8], "to": [14.8, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_r", "from": [21.2, 2.0, 14.8], "to": [22.0, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_t", "from": [14.8, 7.2, 14.8], "to": [21.2, 8.0, 16.0], "group": "output_port", "material": "flesh_lit"})
    cubes.append({"name": "port_lining_b", "from": [14.8, 2.0, 14.8], "to": [21.2, 2.8, 16.0], "group": "output_port", "material": "flesh_lit"})

    # 端口内腔琥珀消化液流光核 (z: 13.0..14.8)
    cubes.append({"name": "port_lumen_amber", "from": [14.8, 2.8, 13.0], "to": [21.2, 7.2, 14.8], "group": "output_port", "material": "amber_fluid"})

    # 幽门转折连接块 (连接胃窦与输出端口, x: 10.0..16.0, y: 1.8..8.2, z: 2.0..8.2)
    cubes.append({"name": "pylorus_elbow_flesh", "from": [10.0, 1.8, 2.0], "to": [16.0, 8.2, 8.2], "group": "output_port", "material": "flesh_dark"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总胃器官全部 5 大部件立方体。"""
    return (
        part_01_esophagus()
        + part_02_stomach_sac()
        + part_03_amber_cavity()
        + part_04_curved_accent_line()
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
    """生成 64×64 RGBA 贴图，严格使用有机型与胃液琥珀配色。"""
    rng = np.random.default_rng(20261009)
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
                    if (x * 7 + y * 11) % 13 in (0, 1):
                        r = int(np.clip(176 + noise, 0, 255))
                        g = int(np.clip(80 + noise, 0, 255))
                        b = int(np.clip(80 + noise, 0, 255))
                    elif (x * 5 + y * 3) % 11 in (0, 1):
                        r = int(np.clip(90 + noise, 0, 255))
                        g = int(np.clip(26 + noise, 0, 255))
                        b = int(np.clip(26 + noise, 0, 255))
                elif mat_name == "amber_fluid":
                    if (x * 3 + y * 5) % 7 in (0, 1):
                        r = int(np.clip(168 + noise, 0, 255))
                        g = int(np.clip(106 + noise, 0, 255))
                        b = int(np.clip(32 + noise, 0, 255))

                arr[y, x] = [r, g, b, base_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "stomach",
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
        g_name = c.get("group", "stomach")
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
    for g_name in ["esophagus", "stomach_sac", "amber_cavity", "curved_accent_line", "output_port"]:
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
        "name": "Stomach",
        "model_identifier": "stomach",
        "visible_box": [3, 3, 2],
        "geometry_name": "stomach",
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
    print(f"✓ 胃器官 bbmodel 写入成功: {rel}")
    return out_path


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(bbmodel_path: Path = BBMODEL_OUT):
    """输出四视角拼图 render.png 与左右并排对照卡 check.png 到 model-review。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 渲染四视角 (正视、侧视、3/4 等轴、俯视)
    im_front, _ = render(bbmodel_path, yaw=0.0, pitch=0.0, size=500, bg=bg_color)
    im_side, _ = render(bbmodel_path, yaw=90.0, pitch=0.0, size=500, bg=bg_color)
    im_iso, _ = render(bbmodel_path, yaw=-35.0, pitch=25.0, size=500, bg=bg_color)
    im_top, _ = render(bbmodel_path, yaw=0.0, pitch=89.9, size=500, bg=bg_color)

    # 2. 拼装 render.png (2x2 网格)
    canvas = Image.new("RGB", (1040, 1040), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    views = [
        ("FRONT (Esophagus, Sac, Amber Window & Port)", im_front, 20, 20),
        ("SIDE (Profile & Depth)",                      im_side, 540, 20),
        ("3/4 ISOMETRIC (Organic Structure)",           im_iso, 20, 540),
        ("TOP (Esophagus Tube & Fundus Arch)",          im_top, 540, 540),
    ]

    for title, im_v, px, py in views:
        canvas.paste(im_v, (px, py))
        draw.rectangle([px, py, px + 340, py + 26], fill=(24, 25, 28))
        draw.text((px + 8, py + 6), title, fill=(230, 230, 230))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 3. 拼装 check.png (左参考图右渲染正面与 3/4 视并排)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 600
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        front_scale_w = int(im_front.width * target_h / im_front.height)
        iso_scale_w = int(im_iso.width * target_h / im_iso.height)
        front_scaled = im_front.resize((front_scale_w, target_h), Image.Resampling.LANCZOS)
        iso_scaled = im_iso.resize((iso_scale_w, target_h), Image.Resampling.LANCZOS)

        right_w = front_scale_w + iso_scale_w + 16
        total_w = ref_w + right_w + 32
        check_cv = Image.new("RGB", (total_w, target_h + 40), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        # 贴左参考
        check_cv.paste(ref_scaled, (12, 30))
        c_draw.text((16, 8), "REFERENCE (o02_stomach.png: Left FRONT / Right 3/4)", fill=(210, 200, 180))

        # 贴右渲染
        rx = ref_w + 24
        check_cv.paste(front_scaled, (rx, 30))
        check_cv.paste(iso_scaled, (rx + front_scale_w + 8, 30))
        c_draw.text((rx + 4, 8), "NOW RENDER (FRONT VIEW + 3/4 ISOMETRIC VIEW)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_stomach.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [6.0, 46.8, -3.0],
        "to":   [12.0, 48.0, -1.0],  # 与 esophagus_rim_n 完全重叠
        "material": "flesh_main",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_stomach.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o02 stomach 生成器")
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
