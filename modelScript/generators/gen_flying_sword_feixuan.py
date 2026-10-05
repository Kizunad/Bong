#!/usr/bin/env python3
"""末法残土飞玄剑 (FlyingSwordFeixuan / flying_sword_feixuan) Blockbench .bbmodel 程序化生成器。

严格依据 three_view.png 与调度审部件第 2 轮意见打磨：
- aerodynamic_blade: 暗枪灰剑身（#3a3a42）基础上加入 15~20% 面积的灰白碎斑（#8a8680 / #b8b2a8 小点，如旧石纹），
  从剑格到剑尖 4 段等差渐窄匀速收尖，两侧极薄压暗锋线（#8a8e96）。
- swallow_wing_guard: 燕翼微翘剑格（微翘 18°），贴图精绘米白 + 暗褐花斑（#c8c0b0 / #6a5444 / #2a2622 混杂），
  倒角微亮高光，上接薄吞口。
- silk_wrapped_hilt: 紧致丝绳缠柄（通过保持）。短柄紧实缠绕深黑丝线，结构精悍利落，两端配有玄铁金属加固套箍。
- tassel_pommel: 环首短链小环与散尾剑穗。从上到下完整结构：
  环首（暗铁大圆环，外径约 1.8px，中空）→ 2 节微细连接短链 → 第二个小圆环（外径约 1.3px，中空）→
  上窄下宽的长剑穗（顶部扎口 0.8px，向下自然散开到 1.6px 宽，长约 2.5px，底端参差错落）。
  环首采用暗铁并带米白花斑与剑格呼应。

门禁与自检：
  - _assert_no_coplanar_faces 检查共面冲突 (Z-fighting)
  - --self-test 注入缺陷自证门禁有效性
  - --parts 逐个导出各部件并在中灰背景 (122, 122, 122) 下渲染存入 parts/，同时严格按调度指定的像素范围从 three_view.png 裁切放大生成等高对标卡
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "FlyingSwordFeixuanV2.bbmodel"
PARTS_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/flying_sword_feixuan/parts")

RES = 64

# ── 调色板基准 (对齐调度第 2 次审定意见) ──
BLADE_GUNMETAL     = [58, 58, 66]     # 暗枪灰剑身底色 #3a3a42
BLADE_SPECK_LIGHT  = [184, 178, 168]  # 灰白碎斑高光 #b8b2a8
BLADE_SPECK_MID    = [138, 134, 128]  # 灰白碎斑中灰 #8a8680
BLADE_PURPLE_DARK  = [42, 36, 48]     # 黑紫微斑 #2a2430
BLADE_EDGE_SUBTLE  = [138, 142, 150]  # 压暗薄锋线 #8a8e96

GUARD_MOTTLED_BASE = [42, 38, 34]     # 剑格深底色 #2a2622
GUARD_MOTTLED_BROWN= [106, 84, 68]    # 剑格暗褐花斑 #6a5444
GUARD_MOTTLED_WHITE= [200, 192, 176]  # 剑格米白花斑 #c8c0b0

SILK_BLACK_BASE    = [32, 34, 38]     # 深黑紧致缠丝
SILK_BLACK_RIDGE   = [48, 52, 60]     # 编织凸棱微灰光
FERRULE_METAL      = [100, 104, 114]  # 金属套箍

POMMEL_IRON_RING   = [62, 64, 70]     # 暗铁圆环首 #3e4046
POMMEL_RING_WHITE  = [188, 184, 174]  # 环首呼应剑格的米白花斑
TASSEL_SILK_BLACK  = [22, 24, 26]     # 黑色长剑穗 #16181a


def part_pommel() -> List[dict]:
    """1. tassel_pommel: 环首 (大圆环) → 2 节短链 → 第二个小圆环 → 上窄下宽剑穗。

    落实调度第 2 轮要求：
    - 大圆环首（外径约 1.8px，中空）带米白花斑；
    - 2 节交错短链；
    - 第二个小圆环（外径约 1.3px，中空）；
    - 上窄下宽的剑穗（顶部扎口 0.8px，往下散开到约 1.6px 宽、长约 2.5px，底端参差错落）。
    """
    cubes = []
    # ── 1. 连接握柄底部的金属颈套 (y: 4.80..5.10, x: 7.45..8.55, z: 7.45..8.55) ──
    cubes.append({
        "name": "pommel_collar",
        "from": [7.45, 4.80, 7.45],
        "to": [8.55, 5.10, 8.55],
        "group": "pommel",
        "material": "pommel_iron",
    })

    # ── 2. 大圆环首 (外径约 1.8px，y: 3.30..4.95, x: 7.10..8.90, 中间中空 1.0x1.0px: z: 7.50..8.50) ──
    # 上横边
    cubes.append({
        "name": "ring_top",
        "from": [7.35, 4.60, 7.50],
        "to": [8.65, 4.95, 8.50],
        "group": "pommel",
        "material": "pommel_iron",
    })
    # 下横边
    cubes.append({
        "name": "ring_bot",
        "from": [7.35, 3.30, 7.50],
        "to": [8.65, 3.65, 8.50],
        "group": "pommel",
        "material": "pommel_iron",
    })
    # 左立边
    cubes.append({
        "name": "ring_l",
        "from": [7.10, 3.55, 7.52],
        "to": [7.45, 4.70, 8.48],
        "group": "pommel",
        "material": "pommel_iron",
    })
    # 右立边
    cubes.append({
        "name": "ring_r",
        "from": [8.55, 3.55, 7.52],
        "to": [8.90, 4.70, 8.48],
        "group": "pommel",
        "material": "pommel_iron",
    })
    # 八角倒角切角板 (打磨出圆润环首外形)
    cubes.append({
        "name": "ring_tl",
        "from": [7.14, 4.45, 7.54],
        "to": [7.56, 4.87, 8.46],
        "group": "pommel",
        "material": "pommel_iron",
        "rotation": [0.0, 0.0, 45.0],
    })
    cubes.append({
        "name": "ring_tr",
        "from": [8.44, 4.45, 7.54],
        "to": [8.86, 4.87, 8.46],
        "group": "pommel",
        "material": "pommel_iron",
        "rotation": [0.0, 0.0, -45.0],
    })
    cubes.append({
        "name": "ring_bl",
        "from": [7.14, 3.38, 7.54],
        "to": [7.56, 3.80, 8.46],
        "group": "pommel",
        "material": "pommel_iron",
        "rotation": [0.0, 0.0, -45.0],
    })
    cubes.append({
        "name": "ring_br",
        "from": [8.44, 3.38, 7.54],
        "to": [8.86, 3.80, 8.46],
        "group": "pommel",
        "material": "pommel_iron",
        "rotation": [0.0, 0.0, 45.0],
    })

    # ── 3. 2 节微细连接短链 (y: 1.70..3.35) ──
    # 链节 1 (纵向链环，穿过大环底梁): y in 2.50..3.35, x in 7.80..8.20, z in 7.60..8.40
    cubes.append({
        "name": "chain_link_1",
        "from": [7.80, 2.50, 7.60],
        "to": [8.20, 3.35, 8.40],
        "group": "pommel",
        "material": "pommel_iron",
    })
    # 链节 2 (横向交错链环): y in 1.70..2.55, x in 7.65..8.35, z in 7.80..8.20
    cubes.append({
        "name": "chain_link_2",
        "from": [7.65, 1.70, 7.80],
        "to": [8.35, 2.55, 8.20],
        "group": "pommel",
        "material": "pommel_iron",
    })

    # ── 4. 第二个小圆环 (外径约 1.3px，y: 0.60..1.75, x: 7.35..8.65, z: 7.65..8.35, 中空) ──
    cubes.append({
        "name": "small_ring_top",
        "from": [7.50, 1.45, 7.65],
        "to": [8.50, 1.75, 8.35],
        "group": "pommel",
        "material": "pommel_iron",
    })
    cubes.append({
        "name": "small_ring_bot",
        "from": [7.50, 0.60, 7.65],
        "to": [8.50, 0.90, 8.35],
        "group": "pommel",
        "material": "pommel_iron",
    })
    cubes.append({
        "name": "small_ring_l",
        "from": [7.35, 0.85, 7.66],
        "to": [7.58, 1.50, 8.34],
        "group": "pommel",
        "material": "pommel_iron",
    })
    cubes.append({
        "name": "small_ring_r",
        "from": [8.42, 0.85, 7.66],
        "to": [8.65, 1.50, 8.34],
        "group": "pommel",
        "material": "pommel_iron",
    })

    # ── 5. 上窄下宽的黑色剑穗 (顶部扎口 0.8px -> 向下散开到 1.6px，长约 2.5px，底端参差) ──
    # 顶部扎口颈部 (宽 0.8px, y: 0.05..0.65)
    cubes.append({
        "name": "tassel_neck",
        "from": [7.60, 0.05, 7.60],
        "to": [8.40, 0.65, 8.40],
        "group": "pommel",
        "material": "silk_black",
    })
    # 扎线金属环箍
    cubes.append({
        "name": "tassel_tie_band",
        "from": [7.55, 0.20, 7.55],
        "to": [8.45, 0.45, 8.45],
        "group": "pommel",
        "material": "pommel_iron",
    })
    # 穗身上段 (宽 1.2px: x in 7.40..8.60, y: -0.90..0.10)
    cubes.append({
        "name": "tassel_body_upper",
        "from": [7.40, -0.90, 7.40],
        "to": [8.60, 0.10, 8.60],
        "group": "pommel",
        "material": "silk_black",
    })
    # 穗身下段 (展开到 1.6px 宽: x in 7.20..8.80, y: -1.80..-0.85)
    cubes.append({
        "name": "tassel_body_lower",
        "from": [7.20, -1.80, 7.20],
        "to": [8.80, -0.85, 8.80],
        "group": "pommel",
        "material": "silk_black",
    })
    # 底端参差散丝 (长约 0.6~0.7px，长短错落，绝无共面)
    cubes.append({
        "name": "tassel_fray_c",
        "from": [7.50, -2.48, 7.50],
        "to": [8.50, -1.74, 8.50],
        "group": "pommel",
        "material": "silk_black",
    })
    cubes.append({
        "name": "tassel_fray_l",
        "from": [7.15, -2.38, 7.30],
        "to": [7.60, -1.78, 8.20],
        "group": "pommel",
        "material": "silk_black",
        "rotation": [0.0, 0.0, 8.0],
    })
    cubes.append({
        "name": "tassel_fray_r",
        "from": [8.40, -2.38, 7.80],
        "to": [8.85, -1.78, 8.70],
        "group": "pommel",
        "material": "silk_black",
        "rotation": [0.0, 0.0, -8.0],
    })

    return cubes


def part_grip() -> List[dict]:
    """2. silk_wrapped_hilt: 紧致丝绳缠柄 (通过保持)。

    短柄紧实缠绕深黑丝线，结构精悍利落，两端配有玄铁金属加固套箍。
    """
    cubes = []
    # ── 握柄内芯灵木柱 (y: 5.10..10.40, 截面 1.16x1.16px, x: 7.42..8.58, z: 7.42..8.58) ──
    cubes.append({
        "name": "grip_core",
        "from": [7.42, 5.10, 7.42],
        "to": [8.58, 10.40, 8.58],
        "group": "grip",
        "material": "silk_black",
    })
    # ── 下端加固套箍 (y: 5.08..5.52, x: 7.28..8.72, z: 7.28..8.72) ──
    cubes.append({
        "name": "grip_ferrule_b",
        "from": [7.28, 5.08, 7.28],
        "to": [8.72, 5.52, 8.72],
        "group": "grip",
        "material": "metal_iron",
    })
    # ── 上端贴格加固套箍 (y: 10.00..10.42, x: 7.28..8.72, z: 7.28..8.72) ──
    cubes.append({
        "name": "grip_ferrule_t",
        "from": [7.28, 10.00, 7.28],
        "to": [8.72, 10.42, 8.72],
        "group": "grip",
        "material": "metal_iron",
    })

    # ── 5 段深黑丝线斜向紧缠圈环 (y: 5.52..10.00) ──
    wraps = [
        ("w0", 5.52, 6.42, 7.33, 8.75, 7.24, 8.66),
        ("w1", 6.42, 7.32, 7.25, 8.67, 7.34, 8.76),
        ("w2", 7.32, 8.22, 7.33, 8.75, 7.24, 8.66),
        ("w3", 8.22, 9.12, 7.25, 8.67, 7.34, 8.76),
        ("w4", 9.12, 10.00, 7.33, 8.75, 7.24, 8.66),
    ]
    for tag, y0, y1, x0, x1, z0, z1 in wraps:
        cubes.append({
            "name": f"grip_wrap_{tag}",
            "from": [x0, y0, z0],
            "to": [x1, y1, z1],
            "group": "grip",
            "material": "silk_black",
        })
    return cubes


def part_guard() -> List[dict]:
    """3. swallow_wing_guard: 燕翼微翘剑格 (米白 + 暗褐花斑)。

    短小紧凑的上翘燕翼剑格（微翘 18°），降低风阻，贴图精绘米白 + 暗褐花斑（#c8c0b0 / #6a5444 / #2a2622），
    倒角微亮高光，上接薄吞口。
    """
    cubes = []
    # ── 1. 中央紧凑套筒枢纽 (x: 7.15..8.85, y: 10.40..12.45, z: 7.15..8.85) ──
    cubes.append({
        "name": "guard_center",
        "from": [7.15, 10.40, 7.15],
        "to": [8.85, 12.45, 8.85],
        "group": "guard",
        "material": "guard_mottled",
    })

    # ── 2. 正反面流线小破风棱脊 (微凸 0.35px) ──
    cubes.append({
        "name": "guard_ridge_f",
        "from": [7.45, 10.65, 8.84],
        "to": [8.55, 12.35, 9.20],
        "group": "guard",
        "material": "guard_mottled",
    })
    cubes.append({
        "name": "guard_ridge_b",
        "from": [7.45, 10.65, 6.80],
        "to": [8.55, 12.35, 7.16],
        "group": "guard",
        "material": "guard_mottled",
    })

    # ── 3. 上接薄吞口套筒 (Habaki, y: 12.35..12.75, x: 7.05..8.95, z: 7.05..8.95) ──
    cubes.append({
        "name": "guard_habaki",
        "from": [7.05, 12.35, 7.05],
        "to": [8.95, 12.75, 8.95],
        "group": "guard",
        "material": "guard_mottled",
    })

    # ── 4. 右侧燕翼分叉翅翎 (+X, 短小紧凑微翘 18°，末端上挑 24°) ──
    cubes.append({
        "name": "guard_wing_r_main",
        "from": [8.80, 10.95, 7.35],
        "to": [10.65, 12.15, 8.65],
        "group": "guard",
        "material": "guard_mottled",
        "rotation": [0.0, 0.0, 18.0],
    })
    cubes.append({
        "name": "guard_wing_r_tip",
        "from": [10.60, 11.45, 7.50],
        "to": [11.55, 12.25, 8.50],
        "group": "guard",
        "material": "guard_mottled",
        "rotation": [0.0, 0.0, 24.0],
    })
    cubes.append({
        "name": "guard_wing_r_sub",
        "from": [8.82, 10.60, 7.45],
        "to": [10.25, 11.45, 8.55],
        "group": "guard",
        "material": "guard_mottled",
        "rotation": [0.0, 0.0, 8.0],
    })

    # ── 5. 左侧燕翼分叉翅翎 (-X, 严格对称) ──
    cubes.append({
        "name": "guard_wing_l_main",
        "from": [5.35, 10.95, 7.35],
        "to": [7.20, 12.15, 8.65],
        "group": "guard",
        "material": "guard_mottled",
        "rotation": [0.0, 0.0, -18.0],
    })
    cubes.append({
        "name": "guard_wing_l_tip",
        "from": [4.45, 11.45, 7.50],
        "to": [5.40, 12.25, 8.50],
        "group": "guard",
        "material": "guard_mottled",
        "rotation": [0.0, 0.0, -24.0],
    })
    cubes.append({
        "name": "guard_wing_l_sub",
        "from": [5.75, 10.60, 7.45],
        "to": [7.18, 11.45, 8.55],
        "group": "guard",
        "material": "guard_mottled",
        "rotation": [0.0, 0.0, -8.0],
    })

    return cubes


def part_blade() -> List[dict]:
    """4. aerodynamic_blade: 暗枪灰带灰白碎斑剑身，4段等差渐窄匀速收尖，压暗薄锋线。

    落实调度审第 2 轮要求：
    - 形状保持（等差收尖压暗到位）；
    - 贴图再加一层参考里的灰白碎斑（占 15~20% 面积的 #8a8680 / #b8b2a8 小点，如旧石纹）；
    - 锋线保持极薄且压暗到 #8a8e96。
    """
    cubes = []

    # ── 1. 4 段等差渐窄匀速收尖主身段 ──
    # 段 1 (刃根段, y: 12.75..17.20, 宽 2.40px: x in 6.80..9.20, 厚 0.56px: z in 7.72..8.28)
    cubes.append({
        "name": "blade_seg_1",
        "from": [6.80, 12.75, 7.72],
        "to": [9.20, 17.20, 8.28],
        "group": "blade",
        "material": "blade_gunmetal",
        "faces": {
            "south": {"uv": [2.0, 20.0, 30.0, 32.0], "texture": 0},
            "north": {"uv": [2.0, 20.0, 30.0, 32.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 8.0],   "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 8.0],   "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
        },
    })

    # 段 2 (中下段, y: 17.20..21.80, 宽 2.00px: x in 7.00..9.00, 厚 0.54px: z in 7.73..8.27)
    cubes.append({
        "name": "blade_seg_2",
        "from": [7.00, 17.20, 7.73],
        "to": [9.00, 21.80, 8.27],
        "group": "blade",
        "material": "blade_gunmetal",
        "faces": {
            "south": {"uv": [4.0, 12.0, 28.0, 20.0], "texture": 0},
            "north": {"uv": [4.0, 12.0, 28.0, 20.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 10.0],  "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 10.0],  "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
        },
    })

    # 段 3 (中上段, y: 21.80..26.20, 宽 1.60px: x in 7.20..8.80, 厚 0.52px: z in 7.74..8.26)
    cubes.append({
        "name": "blade_seg_3",
        "from": [7.20, 21.80, 7.74],
        "to": [8.80, 26.20, 8.26],
        "group": "blade",
        "material": "blade_gunmetal",
        "faces": {
            "south": {"uv": [6.0, 6.0, 26.0, 12.0], "texture": 0},
            "north": {"uv": [6.0, 6.0, 26.0, 12.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 10.0],  "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 10.0],  "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
        },
    })

    # 段 4 (尖前收分段, y: 26.20..29.80, 宽 1.10px: x in 7.45..8.55, 厚 0.48px: z in 7.76..8.24)
    cubes.append({
        "name": "blade_seg_4",
        "from": [7.45, 26.20, 7.76],
        "to": [8.55, 29.80, 8.24],
        "group": "blade",
        "material": "blade_gunmetal",
        "faces": {
            "south": {"uv": [9.0, 2.0, 23.0, 6.0], "texture": 0},
            "north": {"uv": [9.0, 2.0, 23.0, 6.0], "texture": 0},
            "east":  {"uv": [0.0, 0.0, 2.0, 8.0],  "texture": 0},
            "west":  {"uv": [0.0, 0.0, 2.0, 8.0],  "texture": 0},
            "up":    {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
            "down":  {"uv": [0.0, 0.0, 4.0, 2.0],   "texture": 0},
        },
    })

    # 段 5 (极锐破风刺针点, y: 29.80..30.80, 宽 0.24px: x in 7.88..8.12)
    cubes.append({
        "name": "blade_tip_point",
        "from": [7.88, 29.80, 7.86],
        "to": [8.12, 30.80, 8.14],
        "group": "blade",
        "material": "blade_edge_subtle",
    })

    # ── 2. 左右两侧极薄压暗锋线棱带 (宽度仅 0.16px，颜色压暗到 #8a8e96，消除白色长条) ──
    cubes.append({"name": "blade_edge_l_1", "from": [9.20, 12.75, 7.86], "to": [9.36, 17.20, 8.14], "group": "blade", "material": "blade_edge_subtle"})
    cubes.append({"name": "blade_edge_r_1", "from": [6.64, 12.75, 7.86], "to": [6.80, 17.20, 8.14], "group": "blade", "material": "blade_edge_subtle"})
    cubes.append({"name": "blade_edge_l_2", "from": [9.00, 17.20, 7.86], "to": [9.16, 21.80, 8.14], "group": "blade", "material": "blade_edge_subtle"})
    cubes.append({"name": "blade_edge_r_2", "from": [6.84, 17.20, 7.86], "to": [7.00, 21.80, 8.14], "group": "blade", "material": "blade_edge_subtle"})
    cubes.append({"name": "blade_edge_l_3", "from": [8.80, 21.80, 7.86], "to": [8.96, 26.20, 8.14], "group": "blade", "material": "blade_edge_subtle"})
    cubes.append({"name": "blade_edge_r_3", "from": [7.04, 21.80, 7.86], "to": [7.20, 26.20, 8.14], "group": "blade", "material": "blade_edge_subtle"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总所有部件立方体定义。"""
    cubes = []
    cubes.extend(part_pommel())
    cubes.extend(part_grip())
    cubes.extend(part_guard())
    cubes.extend(part_blade())
    return cubes


def _assert_no_coplanar_faces(cubes: List[dict]):
    """门禁：严格检测任意两立方体之间的共面接触 (Z-fighting)。"""
    n = len(cubes)
    tol = 1e-4
    for i in range(n):
        c1 = cubes[i]
        b1_min = c1["from"]
        b1_max = c1["to"]
        for j in range(i + 1, n):
            c2 = cubes[j]
            b2_min = c2["from"]
            b2_max = c2["to"]

            overlap_x = min(b1_max[0], b2_max[0]) - max(b1_min[0], b2_min[0])
            overlap_y = min(b1_max[1], b2_max[1]) - max(b1_min[1], b2_min[1])
            overlap_z = min(b1_max[2], b2_max[2]) - max(b1_min[2], b2_min[2])

            if overlap_x > tol and overlap_y > tol and overlap_z > tol:
                for axis, name in [(0, "X"), (1, "Y"), (2, "Z")]:
                    if abs(b1_min[axis] - b2_min[axis]) < tol:
                        other_axes = [a for a in range(3) if a != axis]
                        oa_span = [
                            min(b1_max[a], b2_max[a]) - max(b1_min[a], b2_min[a])
                            for a in other_axes
                        ]
                        raise AssertionError(
                            f"共面冲突: {c1['name']} 与 {c2['name']} 在 -{name} 面共面 "
                            f"({b1_min[axis]:.4f}), 重叠区域 ({oa_span[0]:.3f}x{oa_span[1]:.3f})"
                        )
                    if abs(b1_max[axis] - b2_max[axis]) < tol:
                        other_axes = [a for a in range(3) if a != axis]
                        oa_span = [
                            min(b1_max[a], b2_max[a]) - max(b1_min[a], b2_min[a])
                            for a in other_axes
                        ]
                        raise AssertionError(
                            f"共面冲突: {c1['name']} 与 {c2['name']} 在 +{name} 面共面 "
                            f"({b1_max[axis]:.4f}), 重叠区域 ({oa_span[0]:.3f}x{oa_span[1]:.3f})"
                        )


def make_texture_atlas() -> Image.Image:
    """生成 64x64 Texture Atlas，严格落实调度要求的灰白碎斑、米白暗褐花斑剑格与暗铁环首。"""
    atlas = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.default_rng(20261004)

    # ── 1. Q1 (0..32, 0..32): 暗枪灰带灰白碎斑剑身 (aerodynamic_blade) ──
    # 底色 #3a3a42 [58, 58, 66]，占 15~20% 面积的灰白碎斑 #8a8680 / #b8b2a8，锋线 #8a8e96
    q1 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-3, 4)
            r = int(58 + noise)
            g = int(58 + noise)
            b = int(66 + noise)

            # 15~20% 面积的灰白碎斑 (旧石纹质感)
            speck_phase = (x * 7 + y * 13 + (x ^ y)) % 17
            if speck_phase in (0, 1):
                # 浅灰白小点 #b8b2a8 [184, 178, 168]
                r = int(np.clip(BLADE_SPECK_LIGHT[0] + noise, 0, 255))
                g = int(np.clip(BLADE_SPECK_LIGHT[1] + noise, 0, 255))
                b = int(np.clip(BLADE_SPECK_LIGHT[2] + noise, 0, 255))
            elif speck_phase in (2, 3):
                # 中灰白碎点 #8a8680 [138, 134, 128]
                r = int(np.clip(BLADE_SPECK_MID[0] + noise, 0, 255))
                g = int(np.clip(BLADE_SPECK_MID[1] + noise, 0, 255))
                b = int(np.clip(BLADE_SPECK_MID[2] + noise, 0, 255))
            elif (x * 3 + y * 5) % 13 == 0:
                # 偶见微沉暗黑斑
                r = max(0, r - 16)
                g = max(0, g - 18)
                b = max(0, b - 14)

            # 极薄压暗锋线 (最外侧仅 1 像素，#8a8e96 [138, 142, 150])
            if x == 0 or x == 31:
                r, g, b = 138, 142, 150
            elif x == 1 or x == 30:
                r, g, b = 92, 96, 104

            q1[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]

    # 正中破风突脊
    for y in range(32):
        q1[y, 15, :3] = [84, 88, 100]
        q1[y, 16, :3] = [62, 68, 80]

    atlas.paste(Image.fromarray(q1, "RGBA"), (0, 0))

    # ── 2. Q2 (32..64, 0..32): 燕翼微翘剑格 (swallow_wing_guard, 米白+暗褐花斑 #c8c0b0 / #6a5444 / #2a2622) ──
    q2 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-4, 5)
            # 基础混杂花斑
            m = (x * 5 + y * 7 + (x * y) % 9) % 19
            if m < 6:
                # 米白花斑 #c8c0b0 [200, 192, 176]
                base = GUARD_MOTTLED_WHITE
            elif m < 12:
                # 暗褐花斑 #6a5444 [106, 84, 68]
                base = GUARD_MOTTLED_BROWN
            else:
                # 深底色 #2a2622 [42, 38, 34]
                base = GUARD_MOTTLED_BASE

            q2[y, x] = [
                int(np.clip(base[0] + noise, 0, 255)),
                int(np.clip(base[1] + noise, 0, 255)),
                int(np.clip(base[2] + noise, 0, 255)),
                255
            ]
    atlas.paste(Image.fromarray(q2, "RGBA"), (32, 0))

    # ── 3. Q3 (0..32, 32..64): 紧致丝绳缠柄 (silk_wrapped_hilt, 通过保持) ──
    q3 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-3, 4)
            if y < 24:
                twist = (x * 2 + y * 2) % 6
                if twist in (0, 1):
                    r, g, b = 48, 52, 60
                elif twist in (2, 3):
                    r, g, b = 32, 34, 38
                else:
                    r, g, b = 18, 20, 22
            else:
                r, g, b = 100, 104, 114
                if y == 24 or y == 31:
                    r, g, b = 140, 145, 158
            q3[y, x] = [int(np.clip(r + noise, 0, 255)), int(np.clip(g + noise, 0, 255)), int(np.clip(b + noise, 0, 255)), 255]
    atlas.paste(Image.fromarray(q3, "RGBA"), (0, 32))

    # ── 4. Q4 (32..64, 32..64): 暗铁圆环首 (带米白花斑呼应) + 黑色长剑穗 (tassel_pommel) ──
    q4 = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            noise = rng.integers(-3, 4)
            if y < 14:
                # 环首: 暗铁 #3e4046 [62, 64, 70]，带少量米白花斑 #c8c0b0 呼应剑格
                cx = (x - 32) if x >= 32 else x
                dist_c = np.hypot(cx - 16, y - 7)
                if (cx * 3 + y * 2) % 7 == 0:
                    r, g, b = 188, 184, 174 # 米白碎斑
                elif dist_c < 2.5:
                    r, g, b = 24, 26, 28    # 环心镂空暗影
                elif dist_c < 5.0:
                    r, g, b = 116, 120, 130 # 金属倒角高光
                else:
                    r, g, b = 62, 64, 70    # 暗铁外壁
            else:
                # 黑色长剑穗: 丝身深黑，细密丝缕微光
                if x % 3 == 0:
                    r, g, b = 46, 50, 56
                elif x % 3 == 1:
                    r, g, b = 24, 26, 28
                else:
                    r, g, b = 14, 16, 18
            q4[y, x] = [int(np.clip(r + noise, 0, 255)), int(np.clip(g + noise, 0, 255)), int(np.clip(b + noise, 0, 255)), 255]
    atlas.paste(Image.fromarray(q4, "RGBA"), (32, 32))

    return atlas


MATERIAL_UV_BOXES = {
    "blade_gunmetal": [0.0, 0.0, 32.0, 32.0],
    "blade_edge_subtle": [28.0, 0.0, 32.0, 16.0],
    "guard_mottled": [32.0, 0.0, 64.0, 32.0],
    "silk_black": [0.0, 32.0, 32.0, 56.0],
    "metal_iron": [0.0, 56.0, 32.0, 64.0],
    "pommel_iron": [32.0, 32.0, 64.0, 46.0],
}


def build_bbmodel_data(cubes: List[dict], tex_img: Image.Image) -> dict:
    """生成标准 Blockbench 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "flying_sword_feixuan",
        "folder": "item",
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

    for cube in cubes:
        elem_uuid = str(uuid.uuid4())
        group_name = cube.get("group", "main")
        groups_map.setdefault(group_name, []).append(elem_uuid)

        f = cube["from"]
        t = cube["to"]
        mat = cube["material"]

        if "faces" in cube:
            faces = cube["faces"]
        else:
            uv_box = MATERIAL_UV_BOXES.get(mat, [0.0, 0.0, 4.0, 4.0])
            faces = {
                face_name: {
                    "uv": uv_box,
                    "texture": 0,
                }
                for face_name in ("north", "east", "south", "west", "up", "down")
            }

        elem = {
            "name": cube["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in f],
            "to": [round(float(v), 4) for v in t],
            "autouv": 0,
            "color": 0,
            "origin": [8.0, 8.0, 8.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }

        if "rotation" in cube:
            center = [
                (float(f[0]) + float(t[0])) / 2.0,
                (float(f[1]) + float(t[1])) / 2.0,
                (float(f[2]) + float(t[2])) / 2.0,
            ]
            elem["origin"] = center
            elem["rotation"] = [round(float(r), 2) for r in cube["rotation"]]

        elements.append(elem)

    outliner = []
    for g_name in ["pommel", "grip", "guard", "blade"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [8.0, 8.0, 8.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {
            "format_version": "4.8",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "FlyingSwordFeixuan",
        "model_identifier": "flying_sword_feixuan",
        "visible_box": [1, 1, 0],
        "geometry_name": "flying_sword_feixuan",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_bbmodel(out_path: Path, cubes_override: List[dict] | None = None) -> Path:
    """输出完整的 FlyingSwordFeixuan.bbmodel 文件。"""
    cubes = cubes_override if cubes_override is not None else all_cubes()
    _assert_no_coplanar_faces(cubes)
    tex_img = make_texture_atlas()
    doc = build_bbmodel_data(cubes, tex_img)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
    rel = out_path.relative_to(REPO) if out_path.is_relative_to(REPO) else out_path
    print(f"✓ FlyingSwordFeixuan bbmodel 写入成功: {rel}")
    return out_path


def render_parts_individually():
    """将 4 大部件分别导出独立单件 bbmodel 并在中灰背景 (122, 122, 122) 下渲染存入 parts/，同时输出与 three_view 对标卡。"""
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    parts_map = {
        "tassel_pommel": (part_pommel(), -35.0, 20.0),
        "silk_wrapped_hilt": (part_grip(), -35.0, 20.0),
        "swallow_wing_guard": (part_guard(), -35.0, 20.0),
        "aerodynamic_blade": (part_blade(), -35.0, 20.0),
    }

    from bbmodel_maker.render.render_bbmodel import render
    print("开始逐部件单件渲染 (4 大部件，中灰背景)...")
    for part_name, (cubes, yaw, pitch) in parts_map.items():
        tmp_model = Path(f"/tmp/FlyingSwordFeixuan_part_{part_name}.bbmodel")
        generate_bbmodel(tmp_model, cubes_override=cubes)
        out_png = PARTS_DIR / f"{part_name}.png"
        img, _ = render(str(tmp_model), yaw=yaw, pitch=pitch, size=600, bg=(122, 122, 122))
        img.save(out_png)
        tmp_model.unlink(missing_ok=True)
        print(f"  ✓ 单部件渲染完成: {out_png}")

    print("开始生成部件与 three_view.png 参考图的对标比对卡 (严格裁准调度指定部位放大、中灰背景)...")
    ref_path = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/flying_sword_feixuan/three_view.png")
    if not ref_path.exists():
        print(f"  [WARN] 未找到参考图: {ref_path}")
        return

    ref_img = Image.open(ref_path).convert("RGB")

    # 严格按照调度在 md 末尾指定的确切原图像素坐标裁切放大：
    # 环首 + 剑穗: x 70..125, y 440..640
    # 剑格: x 150..215, y 480..550
    # 缠柄 (微调避开手臂白色): x 120..160, y 465..520
    # 剑身: x 185..465, y 500..760
    ref_crops = {
        "aerodynamic_blade":  ref_img.crop((185, 500, 465, 760)),
        "swallow_wing_guard": ref_img.crop((150, 480, 215, 550)),
        "silk_wrapped_hilt":  ref_img.crop((120, 465, 160, 520)),
        "tassel_pommel":      ref_img.crop((70, 440, 125, 640)),
    }

    for part_name, ref_crop in ref_crops.items():
        part_img = Image.open(PARTS_DIR / f"{part_name}.png")
        target_h = 600
        p_w = int(part_img.width * (target_h / part_img.height))
        p_scaled = part_img.resize((p_w, target_h), Image.Resampling.LANCZOS)
        r_w = int(ref_crop.width * (target_h / ref_crop.height))
        r_scaled = ref_crop.resize((r_w, target_h), Image.Resampling.LANCZOS)

        gap = 30
        card_w = p_w + r_w + gap + 40
        card_h = target_h + 80
        # 统一中灰背景 (122, 122, 122)
        card = Image.new("RGB", (card_w, card_h), (122, 122, 122))
        draw = ImageDraw.Draw(card)

        card.paste(p_scaled, (20, 60))
        card.paste(r_scaled, (20 + p_w + gap, 60))

        # 中缝分割线与参考图边框
        div_x = 20 + p_w + gap // 2
        draw.line([(div_x, 15), (div_x, card_h - 15)], fill=(70, 70, 70), width=2)
        draw.rectangle([20 + p_w + gap - 1, 59, 20 + p_w + gap + r_w, 60 + target_h], outline=(70, 70, 70), width=1)

        draw.text((25, 20), f"NOW (Single Part: {part_name})", fill=(20, 20, 20))
        draw.text((25 + p_w + gap, 20), f"REF (three_view.png: {part_name})", fill=(20, 20, 20))

        card_path = PARTS_DIR / f"check_{part_name}_vs_ref.png"
        card.save(card_path)
        print(f"  ✓ 比对卡已输出: {card_path}")

    print("✓ 全部 4 个单部件渲染与比对卡已输出完毕！")


def self_test():
    """差分自证：验证正常模型共面校验通过，并能成功捕获注入的共面缺陷。"""
    print("运行 gen_flying_sword_feixuan.py 差分自证...")
    cubes = all_cubes()
    try:
        _assert_no_coplanar_faces(cubes)
        print("  [OK] 正常立方体集无共面冲突")
    except AssertionError as e:
        print(f"  [FAIL] 正常立方体集出现共面冲突: {e}")
        sys.exit(1)

    # 注入缺陷：故意引入完全共面的重叠面
    defect_cubes = list(cubes)
    defect_cubes.append({
        "name": "inject_coplanar_fail",
        "from": [7.15, 10.40, 7.15],
        "to": [8.85, 12.45, 8.85],  # 与 guard_center 完全重叠
        "group": "guard",
        "material": "guard_mottled",
    })
    try:
        _assert_no_coplanar_faces(defect_cubes)
        print("  [FAIL] 未能捕获注入的共面缺陷！")
        sys.exit(1)
    except AssertionError as e:
        print(f"  [OK] 成功捕获注入缺陷: {e}")

    print("✓ gen_flying_sword_feixuan.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="生成末法残土飞玄剑 Blockbench 模型")
    parser.add_argument("--self-test", action="store_true", help="运行差分自证门禁检查")
    parser.add_argument("--parts", action="store_true", help="逐个导出并渲染 4 大部件")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    generate_bbmodel(BBMODEL_OUT)

    if args.parts:
        render_parts_individually()


if __name__ == "__main__":
    main()
