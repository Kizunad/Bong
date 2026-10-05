#!/usr/bin/env python3
"""末法残土执念 (Zhinian / zhinian) Blockbench .bbmodel 程序化生成器。

严格按 2026-09-29 逐部件建造计划 (.task-creature-stepwise.md) 逐步构建：
当前构建部件：
  01_core_body：细瘦的深色身体芯（躯干 + 两腿），比例细长，后面会被长袍盖住，站立稳固、比例协调。

配色表（严格遵循 zhinian.md 审定色表）：
  最深阴影 / 脸内:       #161514 [22, 21, 20]
  深色长袍主色:         #292624 [41, 38, 36]
  长袍亮面:             #3d3936 [61, 57, 54]
  灰色布料 / 手臂:       #5a5552 [90, 85, 82]
  浅灰缠布 / 碎布亮条:   #a09c96 [160, 156, 150]
  锈褐皮革、肩甲(暗/亮): #6b4a2e [107, 74, 46] / #8a5e36 [138, 94, 54]
  腰带扣、剑格铜色:     #9a7440 [154, 116, 64]
  剑身锈铁 / 锈斑:      #5c5c60 [92, 92, 96] / #7a4a2a [122, 74, 42]
  眼睛冷白:             #e8f2ff [232, 242, 255]

门禁与自检：
  - _assert_no_coplanar_faces 检查共面冲突
  - --self-test 注入缺陷自证门禁有效性
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

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "ZhinianV2.bbmodel"
PARTS_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/zhinian/parts")
PARTS_REF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/zhinian/parts_ref")

RES = 64

# ── 严格配色表 (zhinian.md) ──
PALETTE = {
    "shadow": [22, 21, 20],        # #161514 最深阴影 / 脸内
    "robe_dark": [41, 38, 36],     # #292624 深色长袍主色
    "robe_lit": [61, 57, 54],      # #3d3936 长袍亮面
    "cloth_grey": [90, 85, 82],    # #5a5552 灰色布料 / 手臂
    "wrap_light": [160, 156, 150], # #a09c96 浅灰缠布 / 碎布亮条
    "leather_dark": [107, 74, 46], # #6b4a2e 锈褐皮革(暗)
    "leather_lit": [138, 94, 54],  # #8a5e36 锈褐肩甲(亮)
    "copper": [154, 116, 64],      # #9a7440 腰带扣、剑格铜色
    "sword_iron": [92, 92, 96],    # #5c5c60 剑身锈铁
    "sword_rust": [122, 74, 42],   # #7a4a2a 剑身锈斑
    "eye": [232, 242, 255],        # #e8f2ff 眼睛冷白幽光 (1x1 小亮点核心)
    "eye_glow": [154, 184, 216],   # #9ab8d8 眼睛淡蓝辉光 (1px 光晕)
}


def part_01_core_body() -> List[dict]:
    """01_core_body: 细瘦深色身体芯（骷髅式胸廓 + 细腰稍宽骨盆 + 细长骨节腿 + 肩关节与颈接头）。

    按调度审第 1 次意见重构：
    1. 骷髅胸廓：4~5 根横向浅灰肋骨条 (cloth_grey #5a5552 / wrap_light #a09c96) 留深色间隙 (#161514)，
       从正面和侧面清晰呈现「一条条横纹」；
    2. 正中竖直脊柱 (#3d3936)，背面 4 节明显骨节凸起；
    3. 胸廓下方明显收窄腰部 (宽 2.0px)，接稍宽骨盆块 (宽 3.6px #3d3936)；
    4. 肩部两端各一个小肩关节凸块，为 06_arms_claws 留接口；
    5. 细长挺拔骨节腿 (大腿小腿 #292624，膝踝关节 #5a5552，脚掌 #161514 平贴 y=0.0)。
    """
    cubes = []
    # ── 1. 脚部与腿部 (Legs & Feet) ──
    # 脚底地面 y=0.0
    # 左脚
    cubes.append({
        "name": "core_foot_l",
        "from": [5.8, 0.0, 6.5],
        "to": [7.4, 0.9, 8.9],
        "group": "core_body",
        "material": "shadow",
    })
    # 左踝骨节
    cubes.append({
        "name": "core_ankle_l",
        "from": [5.9, 0.9, 7.3],
        "to": [7.3, 1.6, 8.7],
        "group": "core_body",
        "material": "cloth_grey",
    })
    # 左小腿骨
    cubes.append({
        "name": "core_shin_l",
        "from": [6.05, 1.6, 7.45],
        "to": [7.15, 5.8, 8.55],
        "group": "core_body",
        "material": "robe_dark",
    })
    # 左膝关节凸起 (比小腿外露一圈，骨节感明显，cloth_grey)
    cubes.append({
        "name": "core_knee_l",
        "from": [5.85, 5.8, 7.25],
        "to": [7.35, 6.8, 8.75],
        "group": "core_body",
        "material": "cloth_grey",
    })
    # 左大腿骨
    cubes.append({
        "name": "core_thigh_l",
        "from": [6.0, 6.8, 7.4],
        "to": [7.2, 11.2, 8.6],
        "group": "core_body",
        "material": "robe_dark",
    })

    # 右脚
    cubes.append({
        "name": "core_foot_r",
        "from": [8.6, 0.0, 6.5],
        "to": [10.2, 0.9, 8.9],
        "group": "core_body",
        "material": "shadow",
    })
    # 右踝骨节
    cubes.append({
        "name": "core_ankle_r",
        "from": [8.7, 0.9, 7.3],
        "to": [10.1, 1.6, 8.7],
        "group": "core_body",
        "material": "cloth_grey",
    })
    # 右小腿骨
    cubes.append({
        "name": "core_shin_r",
        "from": [8.85, 1.6, 7.45],
        "to": [9.95, 5.8, 8.55],
        "group": "core_body",
        "material": "robe_dark",
    })
    # 右膝关节凸起
    cubes.append({
        "name": "core_knee_r",
        "from": [8.65, 5.8, 7.25],
        "to": [10.15, 6.8, 8.75],
        "group": "core_body",
        "material": "cloth_grey",
    })
    # 右大腿骨
    cubes.append({
        "name": "core_thigh_r",
        "from": [8.8, 6.8, 7.4],
        "to": [10.0, 11.2, 8.6],
        "group": "core_body",
        "material": "robe_dark",
    })

    # ── 2. 稍宽骨盆 (Pelvis) ──
    # 宽度约 3.6px (x: 6.2..9.8), 高度 y: 11.2..13.2, 厚度 2.2px (z: 6.9..9.1), 材质 robe_lit #3d3936
    cubes.append({
        "name": "core_pelvis_base",
        "from": [6.2, 11.2, 6.9],
        "to": [9.8, 12.4, 9.1],
        "group": "core_body",
        "material": "robe_lit",
    })
    # 骨盆上部收口微台阶
    cubes.append({
        "name": "core_pelvis_top",
        "from": [6.4, 12.4, 7.0],
        "to": [9.6, 13.2, 9.0],
        "group": "core_body",
        "material": "robe_dark",
    })

    # ── 3. 明显收窄的腰身 (Narrow Waist) ──
    # 宽度降到约 2.0px (x: 7.0..9.0), 厚度 1.8px (z: 7.15..8.85), 高度 y: 13.2..16.0, 材质 shadow #161514
    cubes.append({
        "name": "core_waist_narrow",
        "from": [7.0, 13.2, 7.15],
        "to": [9.0, 16.0, 8.85],
        "group": "core_body",
        "material": "shadow",
    })
    # 腰部前腹与后脊细线 (前 x: 7.5..8.5, z: 6.8..7.1)
    cubes.append({
        "name": "core_waist_front_sinew",
        "from": [7.5, 13.5, 6.8],
        "to": [8.5, 15.8, 7.1],
        "group": "core_body",
        "material": "robe_dark",
    })

    # ── 4. 正中竖直脊柱 (Vertical Spine) ──
    # 贯穿腰部至上胸 (y: 13.3..21.2)，宽 1.0px (x: 7.5..8.5)，向后微凸 (z: 8.9..9.4)，材质 robe_lit #3d3936
    # 4 节明显的脊椎骨凸起
    for idx, (y0, y1) in enumerate([(13.4, 14.8), (15.2, 16.6), (17.2, 18.6), (19.2, 20.6)]):
        cubes.append({
            "name": f"core_spine_node_{idx+1}",
            "from": [7.45, y0, 8.9],
            "to": [8.55, y1, 9.4],
            "group": "core_body",
            "material": "robe_lit",
        })
    # 脊椎间隙连接柱 (y: 13.3..20.85)
    cubes.append({
        "name": "core_spine_column",
        "from": [7.6, 13.3, 8.75],
        "to": [8.4, 20.85, 9.05],
        "group": "core_body",
        "material": "robe_dark",
    })

    # ── 5. 骷髅胸廓核心与 5 根横向浅灰肋骨条 (Rib Cage & Ribs) ──
    # 胸廓深色内部核心 (y: 16.0..21.1, 宽 3.4px x: 6.3..9.7, 厚 2.2px z: 6.9..8.8), 材质 shadow #161514
    cubes.append({
        "name": "core_chest_cavity",
        "from": [6.3, 16.0, 6.9],
        "to": [9.7, 21.1, 8.8],
        "group": "core_body",
        "material": "shadow",
    })

    # 5 根横向浅灰肋骨条 (每根高 0.7~0.8px，宽约 4.0~4.8px，前后厚 2.5~2.7px，凸出深色腔体)
    # 肋骨与肋骨之间留出 0.3~0.4px 深色间隙，露出 shadow 腔体！
    rib_specs = [
        # (name, ry0, ry1, rx0, rx1, rz0, rz1, mat)
        ("rib_1_lowest", 16.1, 16.8, 6.1, 9.9, 6.75, 9.05, "cloth_grey"),   # 宽 3.8
        ("rib_2_lower",  17.1, 17.8, 5.9, 10.1, 6.70, 9.10, "cloth_grey"),  # 宽 4.2
        ("rib_3_mid",    18.1, 18.9, 5.7, 10.3, 6.65, 9.15, "wrap_light"),  # 宽 4.6 (最宽、最亮高光)
        ("rib_4_upper",  19.2, 20.0, 5.8, 10.2, 6.70, 9.10, "cloth_grey"),  # 宽 4.4
        ("rib_5_top",    20.25, 20.95, 6.0, 10.0, 6.75, 9.05, "cloth_grey"),  # 宽 4.0
    ]
    for rname, ry0, ry1, rx0, rx1, rz0, rz1, rmat in rib_specs:
        # 左肋骨弧
        cubes.append({
            "name": f"core_{rname}_l",
            "from": [rx0, ry0, rz0],
            "to": [7.4, ry1, rz1],
            "group": "core_body",
            "material": rmat,
        })
        # 右肋骨弧
        cubes.append({
            "name": f"core_{rname}_r",
            "from": [8.6, ry0, rz0],
            "to": [rx1, ry1, rz1],
            "group": "core_body",
            "material": rmat,
        })
        # 正面胸骨前扣 (连接左右肋骨正中央的小骨结)
        cubes.append({
            "name": f"core_{rname}_sternum",
            "from": [7.6, ry0, rz0 - 0.1],
            "to": [8.4, ry1, rz0 + 0.3],
            "group": "core_body",
            "material": "cloth_grey",
        })

    # ── 6. 肩部两端肩关节凸块 (Shoulder Sockets) ──
    # 为后续 06_arms_claws 留接口，位于 y: 20.2..21.4，向外突出至 x: 5.2 (左) 与 x: 10.8 (右)
    cubes.append({
        "name": "core_shoulder_socket_l",
        "from": [5.2, 20.2, 7.3],
        "to": [6.0, 21.4, 8.7],
        "group": "core_body",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "core_shoulder_socket_r",
        "from": [10.0, 20.2, 7.3],
        "to": [10.8, 21.4, 8.7],
        "group": "core_body",
        "material": "cloth_grey",
    })

    # ── 7. 领口与颈部接头 (Neck Joint) ──
    # 为 02_head 预留接入底座 (y: 21.15..23.2)
    cubes.append({
        "name": "core_collar_bracket",
        "from": [6.8, 21.15, 7.05],
        "to": [9.2, 21.8, 8.95],
        "group": "core_body",
        "material": "robe_dark",
    })
    cubes.append({
        "name": "core_neck_post",
        "from": [7.3, 21.8, 7.3],
        "to": [8.7, 23.2, 8.7],
        "group": "core_body",
        "material": "cloth_grey",
    })

    return cubes


def part_02_head() -> List[dict]:
    """02_head: 头部（兜帽 + 凹脸 + 1x1 发光白眼与淡蓝辉光 + 斑驳浅灰破布 + 额头锈棕布条 + 破布垂片）。

    调度审第 2 次修改：
    1. 眼睛：从 2x2 大方块改为 1x1 小亮点核心 (#e8f2ff eye，宽 0.5x0.5px) + 1px 淡蓝辉光 (#9ab8d8 eye_glow，宽 1.0x1.0px)，
       两眼间隔约 1.1px，正面与 3/4 视角均小巧神秘有神；
    2. 兜帽斑驳破布：浅灰 #a09c96 上点缀深一档 #5a5552 污渍与 #3d3936 撕裂暗纹，额带锈棕 #6b4a2e 保持；
    3. 尺寸与比例：宽 4.0px、高约 5.0px，比胸廓窄，挂在 01 颈部接头；
    4. 保持未添加飘发 (留给 03)。
    """
    cubes = []
    # 1. 暗色空洞脸膛 (Dark Face Cavity) - #161514 shadow
    cubes.append({
        "name": "head_face_cavity",
        "from": [6.55, 23.15, 7.05],
        "to": [9.45, 26.05, 7.65],
        "group": "head",
        "material": "shadow",
    })

    # 2. 淡蓝辉光 (1px Pale Blue Glow) - #9ab8d8 eye_glow (宽 1.0x1.0px)
    cubes.append({
        "name": "head_eye_glow_l",
        "from": [6.70, 24.55, 6.94],
        "to": [7.70, 25.55, 7.12],
        "group": "head",
        "material": "eye_glow",
    })
    cubes.append({
        "name": "head_eye_glow_r",
        "from": [8.30, 24.55, 6.94],
        "to": [9.30, 25.55, 7.12],
        "group": "head",
        "material": "eye_glow",
    })

    # 3. 1x1 小发光白眼核心 (Small 1x1 Glowing Eye Core) - #e8f2ff eye (微小亮点，宽 0.5x0.5px)
    cubes.append({
        "name": "head_eye_core_l",
        "from": [6.95, 24.80, 6.84],
        "to": [7.45, 25.30, 7.02],
        "group": "head",
        "material": "eye",
    })
    cubes.append({
        "name": "head_eye_core_r",
        "from": [8.55, 24.80, 6.84],
        "to": [9.05, 25.30, 7.02],
        "group": "head",
        "material": "eye",
    })

    # 4. 兜帽后脑壳与顶部包裹 (Hood Back & Top) - 斑驳浅灰破布 wrap_light
    cubes.append({
        "name": "head_hood_back",
        "from": [6.0, 22.8, 7.55],
        "to": [10.0, 26.8, 9.55],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_hood_nape",
        "from": [6.3, 22.0, 7.85],
        "to": [9.7, 22.8, 9.35],
        "group": "head",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "head_hood_top",
        "from": [6.2, 26.8, 6.25],
        "to": [9.8, 27.5, 9.25],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_hood_peak",
        "from": [7.2, 27.5, 6.85],
        "to": [8.8, 27.9, 8.65],
        "group": "head",
        "material": "cloth_grey",
    })

    # 5. 额头横跨锈棕布条 (Rust-Brown Brow Band) - #6b4a2e leather_dark
    cubes.append({
        "name": "head_brow_band_rust",
        "from": [5.95, 26.1, 5.95],
        "to": [10.05, 26.8, 6.45],
        "group": "head",
        "material": "leather_dark",
    })

    # 6. 兜帽正面洞口两侧脸颊包边 (Hood Cheeks & Brow Rim)
    cubes.append({
        "name": "head_hood_brow_brim",
        "from": [6.4, 26.15, 5.75],
        "to": [9.6, 26.75, 6.0],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_hood_cheek_l",
        "from": [5.95, 23.25, 6.0],
        "to": [6.50, 26.05, 7.5],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_hood_cheek_r",
        "from": [9.50, 23.25, 6.0],
        "to": [10.05, 26.05, 7.5],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_hood_chin_rim",
        "from": [6.55, 22.65, 6.2],
        "to": [9.45, 23.15, 7.2],
        "group": "head",
        "material": "cloth_grey",
    })

    # 7. 兜帽下沿长短不一的浅灰破布垂片 (Tattered Shreds)
    cubes.append({
        "name": "head_shred_l1",
        "from": [5.85, 20.7, 6.1],
        "to": [6.45, 23.2, 7.1],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_shred_l2",
        "from": [6.15, 21.3, 7.25],
        "to": [6.65, 23.2, 8.05],
        "group": "head",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "head_shred_r1",
        "from": [9.55, 20.5, 6.1],
        "to": [10.15, 23.2, 7.1],
        "group": "head",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "head_shred_r2",
        "from": [9.35, 21.5, 7.25],
        "to": [9.85, 23.2, 8.05],
        "group": "head",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "head_shred_chin",
        "from": [7.4, 21.0, 6.15],
        "to": [8.6, 22.7, 6.85],
        "group": "head",
        "material": "wrap_light",
    })

    return cubes


def part_03_hair() -> List[dict]:
    """03_hair: 飘发（调度审第 2 次重构：6~8 缕 1px 细发丝、彼此分开留空隙、S 形波浪飘离躯干 2~4px、几缕往背后飘）。

    核心规格：
    1. 8 缕独立 1px 细发丝（左 3 缕、右 3 缕、后脑 2 缕）；
    2. 细度：截面严格 1px (根部半径 ~0.48px，末端收细至 ~0.26px，即直径从 ~1.0px 收细至 ~0.55px)；
    3. 空间走向：先向外、向后大幅飘开 2~4px，再沿 S 形曲线自然弯垂，末端微翘甩开，绝不贴附躯干；
    4. 透气性：发丝彼此完全错开，缕与缕之间留有 1~2px 空隙，从侧面与正面均清晰可见独立缕条；
    5. 后脑发丝向背后深远飘散 (z 延伸至 14.6)；
    6. 配色：根部暗灰 #3d3936 (robe_lit) -> 中段浅灰高光 #a09c96 (wrap_light) -> 末梢中灰 #5a5552 (cloth_grey)。
    """
    from bbmodel_maker.rig.rigkit import Rig
    from plant_geo_common import curved_vine_chain

    mats = {
        "robe_lit": (61, 57, 54),
        "wrap_light": (160, 156, 150),
        "cloth_grey": (90, 85, 82),
    }

    # 8 缕独立细发丝轨迹定义
    wisp_paths = [
        # 1. 左外高飘发 (hair_l_outer: 大 S 形，向外飘开 2.5~4.6px)
        {
            "name": "hair_l_outer",
            "pts": [
                (5.6, 26.2, 7.6),
                (3.8, 25.6, 9.2),
                (2.4, 23.0, 10.6),
                (1.8, 19.5, 11.8),
                (2.1, 16.0, 12.8),
                (1.5, 13.5, 13.6),
            ],
            "r_start": 0.48, "r_end": 0.26,
        },
        # 2. 左中飘发 (hair_l_mid: 离躯干 2~3px)
        {
            "name": "hair_l_mid",
            "pts": [
                (5.8, 24.6, 8.4),
                (4.5, 23.6, 9.8),
                (3.5, 20.8, 11.2),
                (3.2, 17.8, 12.2),
                (3.6, 15.0, 13.0),
                (3.0, 12.5, 13.8),
            ],
            "r_start": 0.50, "r_end": 0.28,
        },
        # 3. 左下后发 (hair_l_inner: 离躯干 1.5~2.5px，向侧后下垂)
        {
            "name": "hair_l_inner",
            "pts": [
                (6.0, 23.2, 9.2),
                (5.2, 21.6, 10.4),
                (4.8, 19.0, 11.5),
                (4.6, 16.4, 12.4),
                (5.0, 13.8, 13.2),
                (4.4, 11.8, 13.9),
            ],
            "r_start": 0.46, "r_end": 0.26,
        },

        # 4. 右外高飘发 (hair_r_outer: 镜像大 S 形，向外飘开 2.5~4.6px)
        {
            "name": "hair_r_outer",
            "pts": [
                (10.4, 26.2, 7.6),
                (12.2, 25.6, 9.2),
                (13.6, 23.0, 10.6),
                (14.2, 19.5, 11.8),
                (13.9, 16.0, 12.8),
                (14.5, 13.5, 13.6),
            ],
            "r_start": 0.48, "r_end": 0.26,
        },
        # 5. 右中飘发 (hair_r_mid: 离躯干 2~3px)
        {
            "name": "hair_r_mid",
            "pts": [
                (10.2, 24.6, 8.4),
                (11.5, 23.6, 9.8),
                (12.5, 20.8, 11.2),
                (12.8, 17.8, 12.2),
                (12.4, 15.0, 13.0),
                (13.0, 12.5, 13.8),
            ],
            "r_start": 0.50, "r_end": 0.28,
        },
        # 6. 右下后发 (hair_r_inner: 向侧后下垂)
        {
            "name": "hair_r_inner",
            "pts": [
                (10.0, 23.2, 9.2),
                (10.8, 21.6, 10.4),
                (11.2, 19.0, 11.5),
                (11.4, 16.4, 12.4),
                (11.0, 13.8, 13.2),
                (11.6, 11.8, 13.9),
            ],
            "r_start": 0.46, "r_end": 0.26,
        },

        # 7. 后脑左飘发 (hair_b_left: 往背后深远飘散，离后背 2~4px)
        {
            "name": "hair_b_left",
            "pts": [
                (7.1, 24.5, 9.6),
                (7.0, 22.6, 10.8),
                (6.7, 20.0, 12.0),
                (6.5, 17.3, 13.0),
                (6.9, 14.6, 13.8),
                (6.4, 12.2, 14.6),
            ],
            "r_start": 0.50, "r_end": 0.28,
        },
        # 8. 后脑右飘发 (hair_b_right: 往背后深远飘散，离后背 2~4px)
        {
            "name": "hair_b_right",
            "pts": [
                (8.9, 24.5, 9.6),
                (9.0, 22.6, 10.8),
                (9.3, 20.0, 12.0),
                (9.5, 17.3, 13.0),
                (9.1, 14.6, 13.8),
                (9.6, 12.2, 14.6),
            ],
            "r_start": 0.50, "r_end": 0.28,
        },
    ]

    all_hair_cubes = []
    for cfg in wisp_paths:
        name = cfg["name"]
        pts = cfg["pts"]
        r_start = cfg["r_start"]
        r_end = cfg["r_end"]

        sub_rig = Rig(mats)
        sub_rig.bone("hair", (8.0, 24.0, 8.0))
        curved_vine_chain(sub_rig, "hair", name, pts, r_start, r_end, mat="robe_lit")

        for idx, el in enumerate(sub_rig.elements):
            cube_dict = {
                "name": el["name"],
                "from": [round(v, 4) for v in el["from"]],
                "to": [round(v, 4) for v in el["to"]],
                "group": "hair",
                "rotation": [round(v, 3) for v in el.get("rotation", [0, 0, 0])],
                "origin": [round(v, 4) for v in el.get("origin", [0, 0, 0])],
            }
            # 配色：前段暗灰主体 #3d3936，中段浅灰高光 #a09c96，末梢中灰 #5a5552
            if idx in (0, 1):
                cube_dict["material"] = "robe_lit"
            elif idx in (2, 3):
                cube_dict["material"] = "wrap_light"
            else:
                cube_dict["material"] = "cloth_grey"

            all_hair_cubes.append(cube_dict)

    return all_hair_cubes


def part_04_torso_wraps_belt() -> List[dict]:
    from bbmodel_maker.rig.rigkit import shaft_box
    """04_torso_wraps_belt: 胸廓暗色底布 + 2px 宽斜布带几近盖满胸腹 + 锈棕腰带青铜扣（调度审第 2 次重构）。"""
    cubes = []

    # ── 1. 胸廓暗色底布层 (Under-Robe Dark Cloth, #292624 robe_dark) ──
    # 正面胸腹底布 (x: 5.88..10.12, y: 15.25..21.15, z: 6.50..6.82)
    cubes.append({
        "name": "wraps_under_robe_front",
        "from": [5.88, 15.25, 6.50],
        "to": [10.12, 21.15, 6.82],
        "group": "torso_wraps",
        "material": "robe_dark",
    })
    # 侧腹左底布 (x: 5.78..6.12, y: 15.22..21.18, z: 6.82..9.08)
    cubes.append({
        "name": "wraps_under_robe_side_l",
        "from": [5.78, 15.22, 6.82],
        "to": [6.12, 21.18, 9.08],
        "group": "torso_wraps",
        "material": "robe_dark",
    })
    # 侧腹右底布 (x: 9.88..10.22, y: 15.22..21.18, z: 6.82..9.08)
    cubes.append({
        "name": "wraps_under_robe_side_r",
        "from": [9.88, 15.22, 6.82],
        "to": [10.22, 21.18, 9.08],
        "group": "torso_wraps",
        "material": "robe_dark",
    })
    # 后背底布 (x: 5.88..10.12, y: 15.28..21.12, z: 8.85..9.18)
    cubes.append({
        "name": "wraps_under_robe_back",
        "from": [5.88, 15.28, 8.85],
        "to": [10.12, 21.12, 9.18],
        "group": "torso_wraps",
        "material": "robe_dark",
    })

    # ── 2. 腰部锈棕皮带与青铜扣 (保持) ──
    cubes.append({
        "name": "belt_front",
        "from": [6.82, 14.15, 6.45],
        "to": [9.18, 15.25, 6.95],
        "group": "torso_wraps",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "belt_back",
        "from": [6.82, 14.15, 8.95],
        "to": [9.18, 15.25, 9.45],
        "group": "torso_wraps",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "belt_side_l",
        "from": [6.68, 14.18, 6.85],
        "to": [7.12, 15.22, 9.05],
        "group": "torso_wraps",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "belt_side_r",
        "from": [8.88, 14.18, 6.85],
        "to": [9.32, 15.22, 9.05],
        "group": "torso_wraps",
        "material": "leather_dark",
    })

    cubes.append({
        "name": "belt_buckle_frame",
        "from": [7.42, 13.92, 6.25],
        "to": [8.58, 15.48, 6.55],
        "group": "torso_wraps",
        "material": "copper",
    })
    cubes.append({
        "name": "belt_buckle_core",
        "from": [7.72, 14.22, 6.20],
        "to": [8.28, 15.18, 6.38],
        "group": "torso_wraps",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "belt_tongue_drop",
        "from": [8.32, 12.75, 6.35],
        "to": [8.82, 14.12, 6.65],
        "group": "torso_wraps",
        "material": "leather_dark",
    })

    # ── 3. 正面宽 2px 斜布带 (2px Wide X-Cross Bands, 几乎盖满胸腹) ──
    # 组 A：左上 -> 右下 (内层)
    front_group_a = [
        ("wrap_wide_a1", (5.8, 20.8, 6.46), (9.5, 17.1, 6.46), "wrap_light", 0.92),  # 上主带，浅灰
        ("wrap_wide_a2", (6.5, 18.8, 6.41), (10.0, 15.3, 6.41), "cloth_grey", 0.92), # 下主带，暗灰
        ("wrap_wide_a0", (5.8, 18.0, 6.49), (7.4, 15.4, 6.49), "wrap_light", 0.70),  # 左下补角带
    ]

    # 组 B：右上 -> 左下 (外层，叠在组 A 外微凸 0.12px，形成宏伟 X 交叉)
    front_group_b = [
        ("wrap_wide_b1", (10.2, 20.8, 6.32), (6.5, 17.1, 6.32), "cloth_grey", 0.92), # 上主带，暗灰
        ("wrap_wide_b2", (9.5, 18.8, 6.27), (6.0, 15.3, 6.27), "wrap_light", 0.92),  # 下主带，浅灰
        ("wrap_wide_b0", (10.2, 18.0, 6.35), (8.6, 15.4, 6.35), "cloth_grey", 0.70), # 右下补角带
    ]

    for name, p0, p1, mat, rx in front_group_a + front_group_b:
        frm, to, rot, org = shaft_box(p0, p1, rx=rx, rz=0.10)
        cubes.append({
            "name": name,
            "from": [round(v, 4) for v in frm],
            "to": [round(v, 4) for v in to],
            "group": "torso_wraps",
            "material": mat,
            "rotation": [round(v, 3) for v in rot],
            "origin": [round(v, 4) for v in org],
        })

    # ── 4. 背面宽斜布带 (Back Wide Wraps) ──
    back_group = [
        ("wrap_back_a1", (5.8, 20.8, 9.18), (9.5, 17.1, 9.18), "cloth_grey", 0.90),
        ("wrap_back_a2", (6.5, 18.8, 9.22), (10.0, 15.3, 9.22), "wrap_light", 0.90),
        ("wrap_back_b1", (10.2, 20.8, 9.27), (6.5, 17.1, 9.27), "wrap_light", 0.90),
        ("wrap_back_b2", (9.5, 18.8, 9.31), (6.0, 15.3, 9.31), "cloth_grey", 0.90),
    ]
    for name, p0, p1, mat, rx in back_group:
        frm, to, rot, org = shaft_box(p0, p1, rx=rx, rz=0.10)
        cubes.append({
            "name": name,
            "from": [round(v, 4) for v in frm],
            "to": [round(v, 4) for v in to],
            "group": "torso_wraps",
            "material": mat,
            "rotation": [round(v, 3) for v in rot],
            "origin": [round(v, 4) for v in org],
        })

    # ── 5. 肩头翻越衔接布带 (Shoulder Wraps) ──
    cubes.append({
        "name": "wrap_shld_top_l",
        "from": [5.72, 20.55, 6.45],
        "to": [6.88, 21.35, 9.25],
        "group": "torso_wraps",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "wrap_shld_top_r",
        "from": [9.12, 20.55, 6.45],
        "to": [10.28, 21.35, 9.25],
        "group": "torso_wraps",
        "material": "cloth_grey",
    })

    return cubes


def part_05_pauldrons() -> List[dict]:
    """05_pauldrons: 严格按调度审第 2 次尺寸构建的大块罩状层叠片甲。

    尺寸与位置（以肩关节 x=5.6, y=20.8, z=8.0 为原点，宽 4->6，深 4.5->5.2，高 1.2）：
    - 片 1 (最上): 宽 4.0 x 深 4.5 x 高 1.2 (y: 21.2..22.4, x: 3.7..7.7, z: 5.75..10.25)
    - 片 2:       宽 5.0 x 深 5.0 x 高 1.2 (y: 20.2..21.4, x: 2.7..7.7, z: 5.50..10.50), 外偏 +0.5
    - 片 3:       宽 5.5 x 深 5.0 x 高 1.2 (y: 19.2..20.4, x: 1.9..7.4, z: 5.50..10.50), 外偏 +1.0
    - 片 4:       宽 6.0 x 深 5.2 x 高 1.2 (y: 18.2..19.4, x: 1.1..7.1, z: 5.40..10.60), 外偏 +1.5
    - 每片之间上片压下片 0.2，下沿留 1px 暗缝 #292624 (robe_dark)
    - 片 1 顶上锈棕皮带横过 + 青铜扣 #9a7440 (copper)
    - 右肩严格对称镜像 (以 x=8.0 镜像)
    """
    cubes = []

    # ════════════════════════════════════════════════════════════════
    # ── 左肩甲 (Left Pauldron: 罩在肩上的大块层叠片甲) ──
    # ════════════════════════════════════════════════════════════════

    # 片 1（最上）：宽 4 × 深 4.5 × 高 1.2，顶在肩上
    cubes.append({
        "name": "pauldron_l_plate_1",
        "from": [3.72, 21.22, 5.77],
        "to": [7.68, 22.38, 10.23],
        "group": "pauldrons",
        "material": "leather_lit",
    })
    # 片 1 顶上一条锈棕皮带横过
    cubes.append({
        "name": "pauldron_l_belt_strap",
        "from": [5.08, 22.38, 6.78],
        "to": [6.18, 22.68, 9.22],
        "group": "pauldrons",
        "material": "leather_dark",
    })
    # 青铜扣 #9a7440
    cubes.append({
        "name": "pauldron_l_copper_buckle",
        "from": [5.38, 22.58, 7.58],
        "to": [5.88, 22.88, 8.42],
        "group": "pauldrons",
        "material": "copper",
    })
    # 片 1 下沿暗色阴影缝 (robe_dark #292624)
    cubes.append({
        "name": "pauldron_l_shadow_1",
        "from": [3.68, 20.95, 5.72],
        "to": [4.48, 21.25, 10.28],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    # 片 2：宽 5 × 深 5 × 高 1.2，y=-1.0，向外偏 +0.5
    cubes.append({
        "name": "pauldron_l_plate_2",
        "from": [2.72, 20.22, 5.54],
        "to": [7.62, 21.38, 10.46],
        "group": "pauldrons",
        "material": "leather_dark",
    })
    # 片 2 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_l_shadow_2",
        "from": [2.68, 19.95, 5.50],
        "to": [3.48, 20.25, 10.50],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    # 片 3：宽 5.5 × 深 5.06 × 高 1.2，y=-2.0，向外偏 +1.0
    cubes.append({
        "name": "pauldron_l_plate_3",
        "from": [1.92, 19.22, 5.46],
        "to": [7.56, 20.38, 10.54],
        "group": "pauldrons",
        "material": "leather_lit",
    })
    # 片 3 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_l_shadow_3",
        "from": [1.88, 18.95, 5.42],
        "to": [2.68, 19.25, 10.58],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    # 片 4：宽 6 × 深 5.24 × 高 1.2，y=-3.0，向外偏 +1.5
    cubes.append({
        "name": "pauldron_l_plate_4",
        "from": [1.12, 18.22, 5.36],
        "to": [7.50, 19.38, 10.64],
        "group": "pauldrons",
        "material": "leather_dark",
    })
    # 片 4 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_l_shadow_4",
        "from": [1.08, 17.95, 5.32],
        "to": [1.88, 18.25, 10.68],
        "group": "pauldrons",
        "material": "robe_dark",
    })


    # ════════════════════════════════════════════════════════════════
    # ── 右肩甲 (Right Pauldron: 镜像对称大块罩状层叠片甲, x_r = 16.0 - x_l) ──
    # ════════════════════════════════════════════════════════════════

    # 片 1（最上）：x: 8.32..12.28
    cubes.append({
        "name": "pauldron_r_plate_1",
        "from": [8.32, 21.22, 5.77],
        "to": [12.28, 22.38, 10.23],
        "group": "pauldrons",
        "material": "leather_lit",
    })
    # 片 1 顶上皮带
    cubes.append({
        "name": "pauldron_r_belt_strap",
        "from": [9.82, 22.38, 6.78],
        "to": [10.92, 22.68, 9.22],
        "group": "pauldrons",
        "material": "leather_dark",
    })
    # 青铜扣
    cubes.append({
        "name": "pauldron_r_copper_buckle",
        "from": [10.12, 22.58, 7.58],
        "to": [10.62, 22.88, 8.42],
        "group": "pauldrons",
        "material": "copper",
    })
    # 片 1 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_r_shadow_1",
        "from": [11.52, 20.95, 5.72],
        "to": [12.32, 21.25, 10.28],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    # 片 2：x: 8.38..13.28
    cubes.append({
        "name": "pauldron_r_plate_2",
        "from": [8.38, 20.22, 5.54],
        "to": [13.28, 21.38, 10.46],
        "group": "pauldrons",
        "material": "leather_dark",
    })
    # 片 2 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_r_shadow_2",
        "from": [12.52, 19.95, 5.50],
        "to": [13.32, 20.25, 10.50],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    # 片 3：x: 8.44..14.08
    cubes.append({
        "name": "pauldron_r_plate_3",
        "from": [8.44, 19.22, 5.46],
        "to": [14.08, 20.38, 10.54],
        "group": "pauldrons",
        "material": "leather_lit",
    })
    # 片 3 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_r_shadow_3",
        "from": [13.32, 18.95, 5.42],
        "to": [14.12, 19.25, 10.58],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    # 片 4：x: 8.50..14.88
    cubes.append({
        "name": "pauldron_r_plate_4",
        "from": [8.50, 18.22, 5.36],
        "to": [14.88, 19.38, 10.64],
        "group": "pauldrons",
        "material": "leather_dark",
    })
    # 片 4 下沿暗色阴影缝
    cubes.append({
        "name": "pauldron_r_shadow_4",
        "from": [14.12, 17.95, 5.32],
        "to": [14.92, 18.25, 10.68],
        "group": "pauldrons",
        "material": "robe_dark",
    })

    return cubes


def part_06_arms_claws() -> List[dict]:
    cubes = []

    # ════════════════════════════════════════════════════════════════
    # ── 06_arms_claws: 严格按调度审第 2 次要求重构 ──
    # ════════════════════════════════════════════════════════════════
    # 1. 细臂截面 1.5px, 肘部微弯 (~160°), 骨节处略凸
    # 2. 臂根一圈锈棕布带 #6b4a2e (紧贴肩甲下沿)
    # 3. 破袖布条：从上臂外侧垂下 5 条细而长短不一布条 (宽 0.7~0.85, 长 3~6px), #a09c96 / #5a5552 相间，不盖住前臂和手
    # 4. 骨爪（最关键）：
    #    - 截面 0.6x0.6px, 指间缝隙 >= 0.5px, 4 根在 X 方向扇形排开 (掌宽约 3~3.5px)
    #    - 每根分 2 节：上节竖直 2.5px, 下节 1.5px 且整体向内平移 0.6px (看得见的钩)
    #    - 颜色改苍白骨色 #a09c96 (指尖 0.5px 用 #5a5552), 和暗灰手臂拉开明暗对比
    #    - 手臂截面 1.5px: #5a5552 (cloth_grey)

    # ────────────────────────────────────────────────────────────────
    # 左臂 (Left Arm: 位于左身侧 x 约 4.9, 前伸 z 约 6.75)
    # ────────────────────────────────────────────────────────────────

    # 1. 臂根锈棕横带 (Arm Root Band): y: 17.65..18.65, 紧贴肩甲下沿
    cubes.append({
        "name": "arm_l_band",
        "from": [4.05, 17.65, 7.15],
        "to": [5.75, 18.65, 8.85],
        "group": "arms_claws",
        "material": "leather_dark",
    })

    # 2. 上臂 (Upper Arm): 截面 1.5x1.5px, y: 15.15..19.35
    cubes.append({
        "name": "arm_l_upper",
        "from": [4.15, 15.15, 7.25],
        "to": [5.65, 19.35, 8.75],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 3. 肘部节点与微弯 (~160°): y: 14.35..15.15
    cubes.append({
        "name": "arm_l_elbow",
        "from": [4.10, 14.35, 7.30],
        "to": [5.70, 15.15, 9.02],
        "group": "arms_claws",
        "material": "robe_dark",
    })

    # 4. 前臂 (Forearm): 截面 1.5x1.5px, 微向前倾斜 (~160° 夹角), y: 10.15..14.35
    cubes.append({
        "name": "arm_l_forearm",
        "from": [4.15, 10.15, 6.95],
        "to": [5.65, 14.35, 8.45],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 5. 腕部掌骨基座 (Wrist / Palm Base): y: 9.35..10.15, 掌宽扩展至 3.6px
    cubes.append({
        "name": "arm_l_wrist",
        "from": [3.35, 9.35, 6.72],
        "to": [5.95, 10.15, 8.18],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 6. 细长骨爪：4 根，截面 0.6x0.6px, 指间缝隙 0.50px, 上节 2.5px 苍白骨色 #a09c96
    #    下节 1.5px 整体向内 (+X 平移 0.6px)，指尖 0.5px 用 #5a5552
    #    排布在 X 方向：F4 (外 2.5..3.1), F3 (3.6..4.2), F2 (4.7..5.3), F1 (内 5.8..6.4)
    #    Z 深度：位于 z: 6.45..7.05，在腿的前方，完全不穿模！
    z_f_from = 6.45
    z_f_to   = 7.05

    # Finger 4 (最外指 / Pinky Claw): x: 2.50..3.10
    # 上节：竖直 2.5px (y: 6.85..9.35), 苍白骨色 #a09c96 (wrap_light)
    cubes.append({
        "name": "claw_l_f4_upper",
        "from": [2.50, 6.85, z_f_from],
        "to": [3.10, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节：1.5px (y: 5.35..6.85), 向内 (+X) 平移 0.6px -> x: 3.10..3.70
    # 上半段 1.0px (y: 5.85..6.85) 苍白骨色 #a09c96
    cubes.append({
        "name": "claw_l_f4_lower",
        "from": [3.10, 5.85, z_f_from],
        "to": [3.70, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 指尖 0.5px (y: 5.35..5.85), #5a5552 (cloth_grey)
    cubes.append({
        "name": "claw_l_f4_tip",
        "from": [3.15, 5.35, z_f_from + 0.05],
        "to": [3.65, 5.85, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # Finger 3 (次外指 / Ring Claw): x: 3.60..4.20 (与 F4 间隙 3.60 - 3.10 = 0.50px)
    # 上节：竖直 2.5px (y: 6.85..9.35)
    cubes.append({
        "name": "claw_l_f3_upper",
        "from": [3.60, 6.85, z_f_from],
        "to": [4.20, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节：向内平移 0.6px -> x: 4.20..4.80
    cubes.append({
        "name": "claw_l_f3_lower",
        "from": [4.20, 5.85, z_f_from],
        "to": [4.80, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_l_f3_tip",
        "from": [4.25, 5.35, z_f_from + 0.05],
        "to": [4.75, 5.85, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # Finger 2 (中指 / Middle Claw): x: 4.70..5.30 (与 F3 间隙 4.70 - 4.20 = 0.50px)
    # 上节：竖直 2.5px (y: 6.85..9.35)
    cubes.append({
        "name": "claw_l_f2_upper",
        "from": [4.70, 6.85, z_f_from],
        "to": [5.30, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节：向内平移 0.6px -> x: 5.30..5.90, 中指最长下探 0.3px (y: 5.05..6.85)
    cubes.append({
        "name": "claw_l_f2_lower",
        "from": [5.30, 5.75, z_f_from],
        "to": [5.90, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_l_f2_tip",
        "from": [5.35, 5.05, z_f_from + 0.05],
        "to": [5.85, 5.75, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # Finger 1 (最内指 / Index Claw): x: 5.80..6.40 (与 F2 间隙 5.80 - 5.30 = 0.50px)
    # 上节：竖直 2.5px (y: 6.85..9.35)
    cubes.append({
        "name": "claw_l_f1_upper",
        "from": [5.80, 6.85, z_f_from],
        "to": [6.40, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节：向内平移 0.6px -> x: 6.40..7.00
    cubes.append({
        "name": "claw_l_f1_lower",
        "from": [6.40, 5.85, z_f_from],
        "to": [7.00, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_l_f1_tip",
        "from": [6.45, 5.35, z_f_from + 0.05],
        "to": [6.95, 5.85, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 7. 破袖布条 (仅在上臂外侧和后侧垂下，长短不一，绝对不盖住前臂和手)
    cubes.append({
        "name": "sleeve_l_shred_1",
        "from": [3.25, 14.80, 7.05],
        "to": [4.05, 18.80, 7.75],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "sleeve_l_shred_2",
        "from": [3.15, 13.20, 7.65],
        "to": [4.00, 19.20, 8.35],
        "group": "arms_claws",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "sleeve_l_shred_3",
        "from": [3.30, 14.20, 8.25],
        "to": [4.10, 18.60, 8.95],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "sleeve_l_shred_4",
        "from": [4.25, 14.00, 8.85],
        "to": [4.95, 17.40, 9.55],
        "group": "arms_claws",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "sleeve_l_shred_5",
        "from": [3.35, 12.80, 7.85],
        "to": [4.05, 15.80, 8.55],
        "group": "arms_claws",
        "material": "wrap_light",
    })

    # ────────────────────────────────────────────────────────────────
    # 右臂 (Right Arm: 镜像对称于 x=8.0)
    # 对称规则：x_r_from = 16.0 - x_l_to, x_r_to = 16.0 - x_l_from
    # 朝中线平移在右臂为 -X (平移 -0.6px)
    # ────────────────────────────────────────────────────────────────

    # 1. 臂根锈棕横带: x: 10.25..11.95
    cubes.append({
        "name": "arm_r_band",
        "from": [10.25, 17.65, 7.15],
        "to": [11.95, 18.65, 8.85],
        "group": "arms_claws",
        "material": "leather_dark",
    })

    # 2. 上臂: x: 10.35..11.85
    cubes.append({
        "name": "arm_r_upper",
        "from": [10.35, 15.15, 7.25],
        "to": [11.85, 19.35, 8.75],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 3. 肘部节点: x: 10.30..11.90
    cubes.append({
        "name": "arm_r_elbow",
        "from": [10.30, 14.35, 7.30],
        "to": [11.90, 15.15, 9.02],
        "group": "arms_claws",
        "material": "robe_dark",
    })

    # 4. 前臂: x: 10.35..11.85
    cubes.append({
        "name": "arm_r_forearm",
        "from": [10.35, 10.15, 6.95],
        "to": [11.85, 14.35, 8.45],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 5. 腕部掌骨基座: x: 10.05..12.65
    cubes.append({
        "name": "arm_r_wrist",
        "from": [10.05, 9.35, 6.72],
        "to": [12.65, 10.15, 8.18],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 6. 右手骨爪 (向中线 -X 平移 0.6px)
    # Finger 1 (最内指 / Index): x_l: 5.80..6.40 -> x_r: 9.60..10.20
    cubes.append({
        "name": "claw_r_f1_upper",
        "from": [9.60, 6.85, z_f_from],
        "to": [10.20, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节平移 -0.6 -> x: 9.00..9.60
    cubes.append({
        "name": "claw_r_f1_lower",
        "from": [9.00, 5.85, z_f_from],
        "to": [9.60, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_r_f1_tip",
        "from": [9.05, 5.35, z_f_from + 0.05],
        "to": [9.55, 5.85, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # Finger 2 (中指): x_l: 4.70..5.30 -> x_r: 10.70..11.30
    cubes.append({
        "name": "claw_r_f2_upper",
        "from": [10.70, 6.85, z_f_from],
        "to": [11.30, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节平移 -0.6 -> x: 10.10..10.70
    cubes.append({
        "name": "claw_r_f2_lower",
        "from": [10.10, 5.75, z_f_from],
        "to": [10.70, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_r_f2_tip",
        "from": [10.15, 5.05, z_f_from + 0.05],
        "to": [10.65, 5.75, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # Finger 3 (次外指): x_l: 3.60..4.20 -> x_r: 11.80..12.40
    cubes.append({
        "name": "claw_r_f3_upper",
        "from": [11.80, 6.85, z_f_from],
        "to": [12.40, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节平移 -0.6 -> x: 11.20..11.80
    cubes.append({
        "name": "claw_r_f3_lower",
        "from": [11.20, 5.85, z_f_from],
        "to": [11.80, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_r_f3_tip",
        "from": [11.25, 5.35, z_f_from + 0.05],
        "to": [11.75, 5.85, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # Finger 4 (最外指): x_l: 2.50..3.10 -> x_r: 12.90..13.50
    cubes.append({
        "name": "claw_r_f4_upper",
        "from": [12.90, 6.85, z_f_from],
        "to": [13.50, 9.35, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    # 下节平移 -0.6 -> x: 12.30..12.90
    cubes.append({
        "name": "claw_r_f4_lower",
        "from": [12.30, 5.85, z_f_from],
        "to": [12.90, 6.85, z_f_to],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "claw_r_f4_tip",
        "from": [12.35, 5.35, z_f_from + 0.05],
        "to": [12.85, 5.85, z_f_to - 0.05],
        "group": "arms_claws",
        "material": "cloth_grey",
    })

    # 7. 破袖布条 (右臂)
    cubes.append({
        "name": "sleeve_r_shred_1",
        "from": [11.95, 14.80, 7.05],
        "to": [12.75, 18.80, 7.75],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "sleeve_r_shred_2",
        "from": [12.00, 13.20, 7.65],
        "to": [12.85, 19.20, 8.35],
        "group": "arms_claws",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "sleeve_r_shred_3",
        "from": [11.90, 14.20, 8.25],
        "to": [12.70, 18.60, 8.95],
        "group": "arms_claws",
        "material": "wrap_light",
    })
    cubes.append({
        "name": "sleeve_r_shred_4",
        "from": [11.05, 14.00, 8.85],
        "to": [11.75, 17.40, 9.55],
        "group": "arms_claws",
        "material": "cloth_grey",
    })
    cubes.append({
        "name": "sleeve_r_shred_5",
        "from": [11.95, 12.80, 7.85],
        "to": [12.65, 15.80, 8.55],
        "group": "arms_claws",
        "material": "wrap_light",
    })

    return cubes


def part_07_sword() -> List[dict]:
    cubes = []

    # ════════════════════════════════════════════════════════════════
    # ── 07_sword: 右手持长古剑 (调度审第 2 次修改) ──
    # ════════════════════════════════════════════════════════════════
    # 1. 剑格下最宽处严格控制在 2px (x: 10.65..12.65)
    # 2. 剑身连续无凸起砖框，单层厚度 0.46px (z: 7.22..7.68)
    # 3. 逐渐收窄成尖 (2.0 -> 1.8 -> 1.5 -> 1.1 -> 0.8 -> 0.5px)，带 2 处缺口
    # 4. 剑柄、剑格、握持位置保持不动

    # ── A. 剑首 (Pommel) ──
    cubes.append({
        "name": "sword_pommel_cube",
        "from": [11.15, 9.85, 6.90],
        "to": [12.15, 10.55, 8.00],
        "group": "sword",
        "material": "sword_iron",
    })

    # ── B. 剑柄 (Grip) ──
    cubes.append({
        "name": "sword_grip_shaft",
        "from": [11.22, 7.25, 7.02],
        "to": [12.08, 9.85, 7.88],
        "group": "sword",
        "material": "sword_iron",
    })
    cubes.append({
        "name": "sword_grip_leather_wrap",
        "from": [11.12, 8.10, 6.92],
        "to": [12.18, 8.85, 7.98],
        "group": "sword",
        "material": "leather_dark",
    })

    # ── C. 简单十字剑格 (Crossguard: 锈铁色，两端略向下翘) ──
    cubes.append({
        "name": "sword_guard_beam",
        "from": [10.15, 6.65, 6.95],
        "to": [13.15, 7.25, 7.95],
        "group": "sword",
        "material": "sword_iron",
    })
    cubes.append({
        "name": "sword_guard_collar",
        "from": [11.05, 6.35, 7.02],
        "to": [12.25, 6.65, 7.88],
        "group": "sword",
        "material": "sword_iron",
    })
    cubes.append({
        "name": "sword_guard_tip_outer",
        "from": [12.85, 6.25, 7.00],
        "to": [13.45, 6.95, 7.90],
        "group": "sword",
        "material": "sword_iron",
    })
    cubes.append({
        "name": "sword_guard_tip_inner",
        "from": [9.85, 6.25, 7.00],
        "to": [10.45, 6.95, 7.90],
        "group": "sword",
        "material": "sword_iron",
    })

    # ── D. 逐渐收窄成尖的长直剑身 (总长约 6.3px, y: 0.35..6.65, 连续无突出方框) ──
    z_f = 7.22
    z_t = 7.68

    # 1. 剑身上段 (y: 4.85..6.35): 宽严格 2.0px (x: 10.65..12.65, 离腿 0.50px >= 0.5px)
    cubes.append({
        "name": "sword_blade_seg1",
        "from": [10.65, 4.85, z_f],
        "to": [12.65, 6.35, z_t],
        "group": "sword",
        "material": "sword_iron",
    })

    # 2. 缺口 1 (Notch 1: y: 4.55..4.85): 外侧残破凹进，宽收至 1.65px (x: 10.65..12.30)
    cubes.append({
        "name": "sword_blade_notch1",
        "from": [10.65, 4.55, z_f],
        "to": [12.30, 4.85, z_t],
        "group": "sword",
        "material": "sword_iron",
    })

    # 3. 剑身中上段 (y: 3.35..4.55): 宽 1.8px (x: 10.75..12.55)
    cubes.append({
        "name": "sword_blade_seg2",
        "from": [10.75, 3.35, z_f],
        "to": [12.55, 4.55, z_t],
        "group": "sword",
        "material": "sword_iron",
    })

    # 4. 缺口 2 (Notch 2: y: 3.05..3.35): 内侧残破凹进，宽收至 1.60px (x: 10.95..12.55)
    cubes.append({
        "name": "sword_blade_notch2",
        "from": [10.95, 3.05, z_f],
        "to": [12.55, 3.35, z_t],
        "group": "sword",
        "material": "sword_iron",
    })

    # 5. 剑身中下段 (y: 2.00..3.05): 宽 1.5px (x: 10.90..12.40)
    cubes.append({
        "name": "sword_blade_seg3",
        "from": [10.90, 2.00, z_f],
        "to": [12.40, 3.05, z_t],
        "group": "sword",
        "material": "sword_iron",
    })

    # ── E. 最后 1/4 长度逐渐收窄成尖 (y: 0.35..2.00, 宽 1.5 -> 0.5px) ──
    # 6. 收尖第 1 段 (y: 1.25..2.00): 宽 1.1px (x: 11.10..12.20)
    cubes.append({
        "name": "sword_tip_taper1",
        "from": [11.10, 1.25, z_f + 0.02],
        "to": [12.20, 2.00, z_t - 0.02],
        "group": "sword",
        "material": "sword_iron",
    })

    # 7. 收尖第 2 段 (y: 0.75..1.25): 宽 0.8px (x: 11.25..12.05)
    cubes.append({
        "name": "sword_tip_taper2",
        "from": [11.25, 0.75, z_f + 0.04],
        "to": [12.05, 1.25, z_t - 0.04],
        "group": "sword",
        "material": "sword_iron",
    })

    # 8. 终点剑锋 (y: 0.35..0.75): 宽精确 0.5px (x: 11.40..11.90), 几乎触地 (y=0.35)
    cubes.append({
        "name": "sword_tip_sharp",
        "from": [11.40, 0.35, z_f + 0.06],
        "to": [11.90, 0.75, z_t - 0.06],
        "group": "sword",
        "material": "sword_iron",
    })

    return cubes


def part_08_robe_hem() -> List[dict]:
    cubes = []

    # ── A. 腰带下方第一圈锈棕布边 (#6b4a2e leather_dark) ──
    cubes.append({
        "name": "robe_waist_trim_f",
        "from": [6.05, 13.42, 6.68],
        "to":   [9.95, 14.24, 7.22],
        "group": "robe_hem",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "robe_waist_trim_b",
        "from": [6.05, 13.38, 8.78],
        "to":   [9.95, 14.26, 9.32],
        "group": "robe_hem",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "robe_waist_trim_l",
        "from": [5.65, 13.44, 6.78],
        "to":   [6.18, 14.22, 9.22],
        "group": "robe_hem",
        "material": "leather_dark",
    })
    cubes.append({
        "name": "robe_waist_trim_r",
        "from": [9.82, 13.40, 6.78],
        "to":   [10.32, 14.28, 9.22],
        "group": "robe_hem",
        "material": "leather_dark",
    })

    # ── B. 暗色内层衬裙核心 (防走光透空，随 A 字形同步外扩) ──
    # 上段内衬 (y: ~6.85..13.35)
    cubes.append({
        "name": "robe_core_thigh_f",
        "from": [5.55, 6.84, 7.18],
        "to":   [10.42, 13.35, 7.55],
        "group": "robe_hem",
        "material": "robe_dark",
    })
    cubes.append({
        "name": "robe_core_thigh_b",
        "from": [5.55, 6.86, 8.95],
        "to":   [10.45, 13.37, 9.45],
        "group": "robe_hem",
        "material": "robe_dark",
    })
    cubes.append({
        "name": "robe_core_thigh_l",
        "from": [5.12, 6.82, 7.22],
        "to":   [5.62, 13.33, 9.25],
        "group": "robe_hem",
        "material": "robe_lit",
    })
    cubes.append({
        "name": "robe_core_thigh_r",
        "from": [10.38, 6.80, 7.22],
        "to":   [10.88, 13.31, 9.25],
        "group": "robe_hem",
        "material": "robe_lit",
    })

    # 下段内衬 (y: ~1.8..6.75, 宽展开至 x=4.0..10.4)
    cubes.append({
        "name": "robe_core_shin_f",
        "from": [4.32, 1.82, 6.85],
        "to":   [10.32, 6.75, 7.35],
        "group": "robe_hem",
        "material": "robe_dark",
    })
    cubes.append({
        "name": "robe_core_shin_b",
        "from": [4.32, 1.84, 9.22],
        "to":   [10.32, 6.77, 9.82],
        "group": "robe_hem",
        "material": "robe_dark",
    })
    cubes.append({
        "name": "robe_core_shin_l",
        "from": [3.86, 1.78, 7.12],
        "to":   [4.42, 6.73, 9.52],
        "group": "robe_hem",
        "material": "robe_lit",
    })
    cubes.append({
        "name": "robe_core_shin_r",
        "from": [10.26, 1.80, 7.18],
        "to":   [10.38, 6.71, 9.52],
        "group": "robe_hem",
        "material": "robe_lit",
    })

    # ── C. 18 条细布条 A 字大幅外扩 (每往下一段向外平移 0.8~1.0px) ──
    # 辅助生成器：
    def make_strip_flared(prefix, x_pts, y_pts, z_pts, w_pts, d_pts, mat, tip_h=0.6):
        # x_pts: [x_s1, x_s2, x_s3]
        # y_pts: [y_top, y_mid, y_bot, y_end]
        # z_pts: [z_s1, z_s2, z_s3]
        # 段 1: 上段 (y_mid to y_top)
        cubes.append({
            "name": f"{prefix}_s1",
            "from": [round(x_pts[0] - w_pts[0]/2, 3), round(y_pts[1], 3), round(z_pts[0] - d_pts[0]/2, 3)],
            "to":   [round(x_pts[0] + w_pts[0]/2, 3), round(y_pts[0], 3), round(z_pts[0] + d_pts[0]/2, 3)],
            "group": "robe_hem",
            "material": mat,
        })
        # 段 2: 中段 (y_bot to y_mid, 向外平移 0.8px)
        cubes.append({
            "name": f"{prefix}_s2",
            "from": [round(x_pts[1] - w_pts[1]/2, 3), round(y_pts[2], 3), round(z_pts[1] - d_pts[1]/2, 3)],
            "to":   [round(x_pts[1] + w_pts[1]/2, 3), round(y_pts[1] - 0.02, 3), round(z_pts[1] + d_pts[1]/2, 3)],
            "group": "robe_hem",
            "material": mat,
        })
        # 段 3: 下段本体 (y_end + tip_h to y_bot, 再向外平移 0.9px)
        if y_pts[2] - (y_pts[3] + tip_h) > 0.2:
            cubes.append({
                "name": f"{prefix}_s3",
                "from": [round(x_pts[2] - w_pts[2]/2, 3), round(y_pts[3] + tip_h, 3), round(z_pts[2] - d_pts[2]/2, 3)],
                "to":   [round(x_pts[2] + w_pts[2]/2, 3), round(y_pts[2] - 0.02, 3), round(z_pts[2] + d_pts[2]/2, 3)],
                "group": "robe_hem",
                "material": mat,
            })
        # 段 4: 削尖末端 (y_end to y_end + tip_h, 截面收窄为 0.4x0.4)
        cubes.append({
            "name": f"{prefix}_tip",
            "from": [round(x_pts[2] - 0.20, 3), round(y_pts[3], 3),         round(z_pts[2] - 0.20, 3)],
            "to":   [round(x_pts[2] + 0.20, 3), round(y_pts[3] + tip_h, 3), round(z_pts[2] + 0.20, 3)],
            "group": "robe_hem",
            "material": mat,
        })

    # 1. 正面 5 条 (前伸外扩：z 从 6.5 -> 5.7 -> 4.9，向前平移 0.8px/段；X 扇形展开)
    # F1 (中左深灰长拖条: y_end=0.00, robe_dark #292624)
    # X 从 7.6 -> 7.2 -> 6.8 (向左外展 0.4px/段)
    make_strip_flared("robe_f1_dk", [7.65, 7.25, 6.85], [13.42, 9.22, 4.82, 0.00], [6.52, 5.72, 4.92], [0.85, 0.90, 0.85], [0.46, 0.46, 0.46], "robe_dark")
    # F2 (正中浅灰微短条: y_end=1.20, wrap_light #a09c96)
    make_strip_flared("robe_f2_lt", [8.05, 8.05, 8.05], [13.46, 9.42, 5.22, 1.20], [6.45, 5.65, 4.85], [0.75, 0.80, 0.75], [0.45, 0.45, 0.45], "wrap_light")
    # F3 (中右深灰长拖条: y_end=0.08, robe_lit #3d3936)
    # X 从 8.5 -> 8.9 -> 9.3 (向右外展 0.4px/段)
    make_strip_flared("robe_f3_dk", [8.55, 8.95, 9.35], [13.40, 9.20, 4.80, 0.08], [6.52, 5.72, 4.92], [0.85, 0.90, 0.85], [0.46, 0.46, 0.46], "robe_lit")
    # F4 (右侧短残条: y_end=2.30, robe_dark #292624, 贴近剑但收在 10.35 内)
    make_strip_flared("robe_f4_dk", [9.35, 9.85, 10.25], [13.48, 9.62, 5.82, 2.30], [6.58, 5.78, 5.06], [0.80, 0.85, 0.80], [0.45, 0.45, 0.45], "robe_dark")
    # F5 (左侧浅灰中条: y_end=0.85, wrap_light #a09c96, 向左外展至 5.6)
    make_strip_flared("robe_f5_lt", [6.85, 6.25, 5.65], [13.44, 9.32, 5.02, 0.85], [6.50, 5.70, 4.90], [0.80, 0.85, 0.80], [0.45, 0.45, 0.45], "wrap_light")

    # 2. 前角外飘条 (向斜前外倾 15°)
    # C_FL (前左角深灰条: y_end=0.10, robe_lit #3d3936, X 从 6.1 -> 5.1 -> 4.1!)
    make_strip_flared("robe_c_fl_dk", [6.15, 5.15, 4.15], [13.50, 9.02, 4.52, 0.10], [6.65, 5.85, 5.05], [0.85, 0.90, 0.85], [0.48, 0.48, 0.48], "robe_lit")
    # C_FR (前右角深灰条: y_end=1.00, robe_lit #3d3936, 收于剑前内侧 x=10.35)
    make_strip_flared("robe_c_fr_dk", [9.85, 10.15, 10.38], [13.38, 9.22, 4.82, 1.00], [6.65, 5.85, 5.05], [0.75, 0.80, 0.75], [0.46, 0.46, 0.46], "robe_lit")

    # 3. 左身侧 3 条 (大幅向外向左平移：0.9px/段！X 从 5.4 -> 4.3 -> 3.2! 达成 1.8W 外扩)
    # L1 (左前浅灰条: y_end=1.40, wrap_light #a09c96)
    make_strip_flared("robe_l1_lt", [5.45, 4.45, 3.45], [13.45, 9.12, 4.82, 1.40], [7.15, 7.15, 7.15], [0.48, 0.48, 0.48], [0.85, 0.90, 0.85], "wrap_light")
    # L2 (左中深灰大拖条: y_end=0.00 最长到地, robe_dark #292624, 最外展至 x=3.10!)
    make_strip_flared("robe_l2_dk", [5.35, 4.25, 3.10], [13.39, 8.82, 4.22, 0.00], [7.95, 7.95, 7.95], [0.50, 0.50, 0.50], [0.95, 1.00, 0.95], "robe_dark")
    # L3 (左后浅灰条: y_end=2.00, wrap_light #a09c96)
    make_strip_flared("robe_l3_lt", [5.45, 4.45, 3.45], [13.47, 9.32, 5.22, 2.00], [8.75, 8.75, 8.75], [0.48, 0.48, 0.48], [0.85, 0.90, 0.85], "wrap_light")

    # 4. 背面 5 条 (后伸外扩：z 从 9.5 -> 10.3 -> 11.1，向后平移 0.8px/段；X 扇形展开)
    # B1 (后中深灰拖地长条: y_end=0.00, robe_dark #292624)
    make_strip_flared("robe_b1_dk", [7.65, 7.25, 6.85], [13.41, 9.02, 4.52, 0.00], [9.38, 10.22, 11.08], [0.85, 0.90, 0.85], [0.46, 0.46, 0.46], "robe_dark")
    # B2 (后中偏左浅灰长条: y_end=0.20, wrap_light #a09c96)
    make_strip_flared("robe_b2_lt", [8.05, 8.05, 8.05], [13.49, 9.22, 4.82, 0.20], [9.45, 10.28, 11.15], [0.80, 0.85, 0.80], [0.45, 0.45, 0.45], "wrap_light")
    # B3 (后中偏右深灰残条: y_end=2.40, robe_dark #292624)
    make_strip_flared("robe_b3_dk", [8.55, 8.95, 9.35], [13.43, 9.62, 5.62, 2.40], [9.38, 10.22, 11.08], [0.80, 0.85, 0.80], [0.46, 0.46, 0.46], "robe_dark")
    # B4 (后右深灰长条: y_end=0.10, robe_lit #3d3936, X 向右外展至 10.7)
    make_strip_flared("robe_b4_dk", [9.35, 10.05, 10.75], [13.47, 9.02, 4.52, 0.10], [9.32, 10.15, 11.02], [0.85, 0.90, 0.85], [0.46, 0.46, 0.46], "robe_lit")
    # B5 (后左深灰中条: y_end=1.10, robe_dark #292624, X 向左外展至 5.6)
    make_strip_flared("robe_b5_dk", [6.85, 6.25, 5.65], [13.35, 9.12, 4.82, 1.10], [9.40, 10.25, 11.12], [0.80, 0.85, 0.80], [0.45, 0.45, 0.45], "robe_dark")

    # 5. 后角外飘条 (斜后外倾 15°)
    # C_BL (后左角深灰条: y_end=0.20, robe_lit #3d3936, 外展至 x=3.9!)
    make_strip_flared("robe_c_bl_dk", [6.05, 4.95, 3.95], [13.51, 8.92, 4.42, 0.20], [9.25, 10.10, 10.95], [0.85, 0.90, 0.85], [0.48, 0.48, 0.48], "robe_lit")
    # C_BR (后右角浅灰条: y_end=1.20, wrap_light #a09c96, 剑后外展至 x=11.6!)
    make_strip_flared("robe_c_br_lt", [9.95, 10.75, 11.55], [13.43, 9.32, 5.02, 1.20], [9.25, 10.10, 10.95], [0.80, 0.85, 0.80], [0.48, 0.48, 0.48], "wrap_light")

    # 6. 右身侧 2 条 (右侧后方条在剑后大幅外扩至 x=12.1! 右中短条收在 10.35 贴身避让剑)
    # R1 (右后剑后深灰条: y_end=0.00 到地, robe_dark #292624, z在剑后 z=8.65, 外展至 x=12.15!)
    make_strip_flared("robe_r1_dk", [10.45, 11.30, 12.15], [13.41, 8.92, 4.32, 0.00], [8.65, 8.65, 8.65], [0.50, 0.50, 0.50], [0.90, 0.95, 0.90], "robe_dark")
    # R2 (右中浅灰短条: y_end=2.60, wrap_light #a09c96, x保持在 10.32 贴身完全不碰剑)
    make_strip_flared("robe_r2_lt", [10.32, 10.32, 10.32], [13.49, 9.52, 5.76, 2.60], [7.75, 7.75, 7.75], [0.44, 0.44, 0.44], [0.75, 0.75, 0.75], "wrap_light")

    return cubes


def all_cubes() -> List[dict]:
    """当前步骤的累积部件集合。第 1 步仅包含 01_core_body。"""
    cubes = []
    cubes.extend(part_01_core_body())
    cubes.extend(part_02_head())
    cubes.extend(part_03_hair())
    cubes.extend(part_04_torso_wraps_belt())
    cubes.extend(part_05_pauldrons())
    cubes.extend(part_06_arms_claws())
    cubes.extend(part_07_sword())
    cubes.extend(part_08_robe_hem())
    return cubes


TURN_PIVOT_X = 8.0
TURN_PIVOT_Z = 8.0


def turn_to_plus_z(cubes: List[dict]) -> List[dict]:
    """把各部件按「脸朝 -Z」写的坐标整体绕 Y 转 180°，使建模源朝 +Z。

    约定出处：`creatures/fauna_v2/gen_rig.py` 文件头说明，v2 重做生物的建模源都面朝 +Z，
    导出绑定时再统一绕 Y 转 180° 变成游戏里的 -Z。执念各部件当初是脸朝 -Z 写的，
    如果直接交给导出再转一次，游戏里会反成面朝 +Z。

    这是刚体旋转：绕 (TURN_PIVOT_X, TURN_PIVOT_Z) 竖轴转半圈，不动任何部件的相对关系。
    - from/to：x、z 取镜像（16 - v），并对调大小端；
    - 带旋转的 cube：origin 同样转，欧拉角 (rx, ry, rz) -> (-rx, ry, -rz)
      （180° 绕 Y 共轭会把绕 X、Z 轴的转角反号，绕 Y 的不变）。
    贴图六面共用同一块 UV，所以不需要像 gen_rig 那样交换侧面。
    """
    pivot_sum_x = 2 * TURN_PIVOT_X
    pivot_sum_z = 2 * TURN_PIVOT_Z
    turned = []
    for cube in cubes:
        start, end = cube["from"], cube["to"]
        new_cube = dict(cube)
        new_cube["from"] = [pivot_sum_x - end[0], start[1], pivot_sum_z - end[2]]
        new_cube["to"] = [pivot_sum_x - start[0], end[1], pivot_sum_z - start[2]]
        if "origin" in cube:
            ox, oy, oz = cube["origin"]
            new_cube["origin"] = [pivot_sum_x - ox, oy, pivot_sum_z - oz]
        if "rotation" in cube:
            rx, ry, rz = cube["rotation"]
            new_cube["rotation"] = [-rx, ry, -rz]
        turned.append(new_cube)
    return turned


def recenter_to_origin(cubes: List[dict]) -> List[dict]:
    """把身体中轴从 (x=8, z=8) 平移到 (0, 0)，与其它 v2 生物建模源一致。

    其它 v2 生物（如 DaoxiangV2）都以 x=0 为身体中线建模，导出绑定时 gen_rig 直接取
    (x, z) -> (-x, -z) 转向；执念若还停在 x=8，转向后会整体跑到 x=-8。纯平移，不动形体。
    """
    shifted = []
    for cube in cubes:
        new_cube = dict(cube)
        new_cube["from"] = [cube["from"][0] - TURN_PIVOT_X, cube["from"][1], cube["from"][2] - TURN_PIVOT_Z]
        new_cube["to"] = [cube["to"][0] - TURN_PIVOT_X, cube["to"][1], cube["to"][2] - TURN_PIVOT_Z]
        if "origin" in cube:
            ox, oy, oz = cube["origin"]
            new_cube["origin"] = [ox - TURN_PIVOT_X, oy, oz - TURN_PIVOT_Z]
        shifted.append(new_cube)
    return shifted


def _assert_no_coplanar_faces(cubes: List[dict]):
    """检查立方体集是否存在严格同向同坐标且重叠的共面冲突。"""
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
            key = (side, round(val, 4), rot)
            faces.setdefault(key, []).append(c)

    for (side, val, rot), face_cubes in faces.items():
        if len(face_cubes) < 2:
            continue
        axis_char = side[1]
        other_axes = [0, 1, 2]
        other_axes.remove({"X": 0, "Y": 1, "Z": 2}[axis_char])
        ax1, ax2 = other_axes

        for i in range(len(face_cubes)):
            c1 = face_cubes[i]
            for j in range(i + 1, len(face_cubes)):
                c2 = face_cubes[j]
                min1_a, max1_a = c1["from"][ax1], c1["to"][ax1]
                min1_b, max1_b = c1["from"][ax2], c1["to"][ax2]
                min2_a, max2_a = c2["from"][ax1], c2["to"][ax1]
                min2_b, max2_b = c2["from"][ax2], c2["to"][ax2]

                overlap_a = min(max1_a, max2_a) - max(min1_a, min2_a)
                overlap_b = min(max1_b, max2_b) - max(min1_b, min2_b)
                if overlap_a > 1e-4 and overlap_b > 1e-4:
                    raise AssertionError(
                        f"共面冲突: {c1['name']} 与 {c2['name']} 在 {side} 面共面 ({val:.4f}), "
                        f"重叠区域 ({overlap_a:.3f}x{overlap_b:.3f})"
                    )


def build_texture(res: int = 64) -> Image.Image:
    """按配色表合成统一 64x64 贴图。"""
    tex = Image.new("RGBA", (res, res), tuple(PALETTE["robe_dark"] + [255]))
    rng = np.random.default_rng(2026)

    # 1. 最深阴影 / 脸内 [0:16, 0:16]
    for y in range(16):
        for x in range(16):
            n = rng.uniform(-2, 2)
            c = [int(np.clip(v + n, 0, 255)) for v in PALETTE["shadow"]]
            tex.putpixel((x, y), tuple(c + [255]))

    # 2. 深色长袍主色 [16:32, 0:16]
    for y in range(16):
        for x in range(16):
            n = rng.uniform(-3, 3)
            c = [int(np.clip(v + n, 0, 255)) for v in PALETTE["robe_dark"]]
            tex.putpixel((16 + x, y), tuple(c + [255]))

    # 3. 长袍亮面 [32:48, 0:16]
    for y in range(16):
        for x in range(16):
            n = rng.uniform(-3, 3)
            c = [int(np.clip(v + n, 0, 255)) for v in PALETTE["robe_lit"]]
            tex.putpixel((32 + x, y), tuple(c + [255]))

    # 4. 灰色布料 / 手臂 [48:64, 0:16]
    for y in range(16):
        for x in range(16):
            n = rng.uniform(-4, 4)
            c = [int(np.clip(v + n, 0, 255)) for v in PALETTE["cloth_grey"]]
            tex.putpixel((48 + x, y), tuple(c + [255]))

    # 5. 斑驳浅灰破布 [0:16, 16:32]
    # 在浅灰 #a09c96 上点缀深一档 #5a5552 的污渍和几条撕裂暗纹 #3d3936
    wrap_grid = np.zeros((16, 16, 3), dtype=float)
    for y in range(16):
        for x in range(16):
            noise = rng.uniform(-4, 4)
            wrap_grid[y, x] = [np.clip(v + noise, 0, 255) for v in PALETTE["wrap_light"]]

    # 污渍斑块 (patches of cloth_grey [90, 85, 82])
    stains = [(2, 3, 2), (3, 11, 2), (8, 6, 2), (12, 2, 2), (11, 12, 3)]
    for cy, cx, r in stains:
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < 16 and 0 <= nx < 16:
                    dist = np.sqrt(dy * dy + dx * dx)
                    if dist <= r:
                        blend = (1.0 - dist / (r + 0.5)) * 0.75
                        wrap_grid[ny, nx] = wrap_grid[ny, nx] * (1 - blend) + np.array(PALETTE["cloth_grey"]) * blend

    # 撕裂暗纹与折痕磨损 (lines of darker frayed threads)
    tear_lines = [
        [(1, 1), (2, 2), (3, 2), (4, 3), (5, 4)],
        [(7, 9), (8, 10), (9, 11), (10, 11)],
        [(11, 4), (12, 5), (13, 6), (14, 7)],
        [(0, 8), (1, 8), (2, 9), (3, 10)],
    ]
    for pts in tear_lines:
        for py, px in pts:
            if 0 <= py < 16 and 0 <= px < 16:
                wrap_grid[py, px] = np.array(PALETTE["robe_lit"]) * 0.9
                if py + 1 < 16:
                    wrap_grid[py + 1, px] = np.clip(wrap_grid[py + 1, px] * 1.15, 0, 255)

    for y in range(16):
        for x in range(16):
            c = [int(v) for v in wrap_grid[y, x]]
            tex.putpixel((x, 16 + y), tuple(c + [255]))

    # 6. 锈褐皮革(暗) [16:32, 16:32] 与 7. 锈褐肩甲(亮) [32:48, 16:32]
    # 调度审第 2 次要求：铁灰 #7a7470 / #5a5552 为底，片面上大块锈斑 #6b4a2e / #8a5e36 (约占 40%)，边缘磨亮 #9a948e
    iron_base1 = [122, 116, 112]  # #7a7470 铁灰底
    iron_base2 = [90, 85, 82]     # #5a5552 深铁灰
    rust_col1 = [107, 74, 46]     # #6b4a2e 锈斑暗
    rust_col2 = [138, 94, 54]     # #8a5e36 亮锈斑
    edge_lit = [154, 148, 142]    # #9a948e 边缘磨亮

    def make_iron_rust_plate(seed_val):
        r_local = np.random.RandomState(seed_val)
        grid = np.zeros((16, 16, 3), dtype=float)
        # 1. 铁灰底色
        for y in range(16):
            for x in range(16):
                base = iron_base1 if (x + y) % 3 != 0 else iron_base2
                n = r_local.uniform(-3, 3)
                grid[y, x] = [np.clip(v + n, 0, 255) for v in base]

        # 2. 大块锈斑 (约占 40% 面积)
        rust_centers = [(4, 5, 3.5), (10, 11, 4.0), (12, 4, 3.0), (3, 12, 2.5)]
        for cy, cx, r in rust_centers:
            for dy in range(-int(r) - 1, int(r) + 2):
                for dx in range(-int(r) - 1, int(r) + 2):
                    ny, nx = cy + dy, cx + dx
                    if 0 <= ny < 16 and 0 <= nx < 16:
                        dist = np.sqrt(dy * dy + dx * dx)
                        if dist <= r:
                            blend = min(1.0, max(0.0, 1.2 - dist / r))
                            c_rust = rust_col2 if (ny + nx) % 2 == 0 else rust_col1
                            grid[ny, nx] = grid[ny, nx] * (1.0 - blend) + np.array(c_rust) * blend

        # 3. 边缘磨亮 (#9a948e)
        for i in range(16):
            for (ey, ex) in [(0, i), (15, i), (i, 0), (i, 15)]:
                grid[ey, ex] = np.array(edge_lit) * r_local.uniform(0.95, 1.05)

        return grid

    g_dark = make_iron_rust_plate(101)
    g_lit = make_iron_rust_plate(202)

    for y in range(16):
        for x in range(16):
            c_dark = [int(np.clip(v, 0, 255)) for v in g_dark[y, x]]
            tex.putpixel((16 + x, 16 + y), tuple(c_dark + [255]))
            c_lit = [int(np.clip(v, 0, 255)) for v in g_lit[y, x]]
            tex.putpixel((32 + x, 16 + y), tuple(c_lit + [255]))

    # 8. 腰带扣、剑格铜色 [48:64, 16:32]
    for y in range(16):
        for x in range(16):
            n = rng.uniform(-4, 4)
            c = [int(np.clip(v + n, 0, 255)) for v in PALETTE["copper"]]
            tex.putpixel((48 + x, 16 + y), tuple(c + [255]))

    # 9. 剑身锈铁 [0:16, 32:48] 与 10. 剑身锈斑 [16:32, 32:48]
    # 调度审第 2 次要求：去掉每段立方体贴图四周的灰边框，让剑身锈斑连续混杂 (灰白/暗灰/锈棕碎块，无色带无边框)
    c_white = np.array([160, 156, 150], float)  # #a09c96
    c_grey  = np.array([90, 85, 82], float)     # #5a5552
    c_iron  = np.array([92, 92, 96], float)     # #5c5c60
    c_rust_d = np.array([107, 74, 46], float)   # #6b4a2e
    c_rust_l = np.array([138, 90, 52], float)   # #8a5a34
    c_rust_deep = np.array([61, 38, 24], float) # #3d2618

    # 9. 连续无边框的 sword_iron (16x16)
    grid_iron = np.zeros((16, 16, 3), float)
    for y in range(16):
        for x in range(16):
            t = np.sin(x * 0.4) * np.cos(y * 0.35) * 0.5 + 0.5
            base = c_iron * (1 - t) + c_grey * t
            grid_iron[y, x] = base + rng.uniform(-3, 3, 3)

    rust_blobs = [(4, 6, 3.8), (11, 12, 4.2), (13, 2, 3.0), (3, 13, 2.5), (8, 7, 2.0)]
    for cy, cx, r in rust_blobs:
        for dy in range(-int(r) - 1, int(r) + 2):
            for dx in range(-int(r) - 1, int(r) + 2):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < 16 and 0 <= nx < 16:
                    d = np.sqrt(dy * dy + dx * dx)
                    if d <= r:
                        w = np.cos((d / r) * np.pi * 0.5) ** 1.5
                        col = c_rust_l if (ny + nx) % 2 == 0 else c_rust_d
                        if d < r * 0.5:
                            col = c_rust_deep if (ny * 3 + nx) % 4 == 0 else col
                        grid_iron[ny, nx] = grid_iron[ny, nx] * (1 - w) + col * w

    white_specks = [(2, 2), (5, 9), (9, 3), (12, 7), (14, 14), (7, 11), (10, 15)]
    for sy, sx in white_specks:
        if 0 <= sy < 16 and 0 <= sx < 16:
            grid_iron[sy, sx] = c_white + rng.uniform(-3, 3, 3)
            for ddy, ddx in [(0, 1), (1, 0)]:
                if 0 <= sy + ddy < 16 and 0 <= sx + ddx < 16:
                    grid_iron[sy + ddy, sx + ddx] = c_white * 0.8 + c_grey * 0.2

    for y in range(16):
        for x in range(16):
            c = [int(np.clip(v, 0, 255)) for v in grid_iron[y, x]]
            tex.putpixel((x, 32 + y), tuple(c + [255]))

    # 10. 连续无边框的 sword_rust (16x16)
    grid_rust = np.zeros((16, 16, 3), float)
    for y in range(16):
        for x in range(16):
            t = np.cos(x * 0.5 + y * 0.3) * 0.5 + 0.5
            base = c_rust_d * (1 - t) + c_rust_l * t
            grid_rust[y, x] = base + rng.uniform(-4, 4, 3)

    iron_flecks = [(5, 4, 2.5), (10, 10, 3.0), (2, 11, 2.0), (13, 5, 2.2)]
    for cy, cx, r in iron_flecks:
        for dy in range(-int(r) - 1, int(r) + 2):
            for dx in range(-int(r) - 1, int(r) + 2):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < 16 and 0 <= nx < 16:
                    d = np.sqrt(dy * dy + dx * dx)
                    if d <= r:
                        w = np.cos((d / r) * np.pi * 0.5)
                        col = c_grey if (ny + nx) % 3 != 0 else c_white
                        grid_rust[ny, nx] = grid_rust[ny, nx] * (1 - w) + col * w

    for y in range(16):
        for x in range(16):
            c = [int(np.clip(v, 0, 255)) for v in grid_rust[y, x]]
            tex.putpixel((16 + x, 32 + y), tuple(c + [255]))

    # 11. 眼睛冷白幽光核心 [32:48, 32:48]
    for y in range(16):
        for x in range(16):
            tex.putpixel((32 + x, 32 + y), tuple(PALETTE["eye"] + [255]))

    # 12. 眼睛淡蓝幽光光晕 [48:64, 32:48]
    for y in range(16):
        for x in range(16):
            c = [int(np.clip(v + rng.uniform(-3, 3), 0, 255)) for v in PALETTE["eye_glow"]]
            tex.putpixel((48 + x, 32 + y), tuple(c + [255]))

    return tex


MAT_UV = {
    "shadow": [0, 0, 16, 16],
    "robe_dark": [16, 0, 32, 16],
    "robe_lit": [32, 0, 48, 16],
    "cloth_grey": [48, 0, 64, 16],
    "wrap_light": [0, 16, 16, 32],
    "leather_dark": [16, 16, 32, 32],
    "leather_lit": [32, 16, 48, 32],
    "copper": [48, 16, 64, 32],
    "sword_iron": [0, 32, 16, 48],
    "sword_rust": [16, 32, 32, 48],
    "eye": [32, 32, 48, 48],
    "eye_glow": [48, 32, 64, 48],
}


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: List[dict] | None = None) -> Path:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = cubes_override if cubes_override is not None else all_cubes()
    _assert_no_coplanar_faces(cubes)
    cubes = recenter_to_origin(turn_to_plus_z(cubes))

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for c in cubes:
        f = c["from"]
        t = c["to"]
        mat = c.get("material", "robe_dark")
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
        if "rotation" in c:
            elem["rotation"] = c["rotation"]
        if "origin" in c:
            elem["origin"] = c["origin"]

        elements.append(elem)

    groups_map: Dict[str, List[str]] = {}
    for i, e in enumerate(elements):
        g = cubes[i].get("group", "core_body")
        groups_map.setdefault(g, []).append(e["uuid"])

    outliner = [
        {"name": g, "origin": [8.0, 8.0, 8.0], "children": u_list}
        for g, u_list in groups_map.items()
    ]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "zhinian",
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "name": "zhinian",
                "folder": "entity",
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
    print(f"✓ Zhinian bbmodel 写入成功: {rel}")
    return out_path


def render_step_01():
    """按新流程第 3 步：单独渲染 01_core_body 并输出与 parts_ref/01_core_body.png 的并排对照图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    cubes = part_01_core_body()
    tmp_model = Path("/tmp/Zhinian_step_01_core_body.bbmodel")
    generate_bbmodel(tmp_model, cubes_override=cubes)

    # 渲染 正面 (yaw=0, pitch=0), 侧面 (yaw=90, pitch=0), 3/4视角 (yaw=-35, pitch=20)
    print("渲染 01_core_body 单部件视图...")
    img_f, _ = render(str(tmp_model), yaw=0, pitch=0, size=600)
    img_s, _ = render(str(tmp_model), yaw=90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model), yaw=-35, pitch=20, size=600)
    tmp_model.unlink(missing_ok=True)

    # 紧密裁切有效内容
    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/01_core_body.png
    ref_path = PARTS_REF_DIR / "01_core_body.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    y_l, x_l = np.where(diff_ref[:, :768])
    ref_front = ref_im.crop((x_l.min(), y_l.min(), x_l.max() + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 768:])
    ref_side = ref_im.crop((x_r.min() + 768, y_r.min(), x_r.max() + 768 + 1, y_r.max() + 1))

    # 统一纵向高度
    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    # 拼接并排对照图 (左参考、右渲染，裁准部件本身)
    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    # 中缝分割线
    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/01_core_body.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_01_core_body() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_01_core_body.png"
    card.save(check_card_path)
    print(f"✓ 并排对照图已输出: {check_card_path}")

    # 保存 01_core_body 单部件渲染展示图 (中灰/深色底)
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "01_core_body (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "01_core_body.png"
    sheet.save(single_render_path)
    print(f"✓ 单部件渲染图已输出: {single_render_path}")


def render_step_02():
    """按新流程：单独渲染 02_head、输出与 parts_ref/02_head.png 的并排对照图，以及 01+02 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 02_head 单件渲染
    cubes_02 = part_02_head()
    tmp_model_02 = Path("/tmp/Zhinian_step_02_head.bbmodel")
    generate_bbmodel(tmp_model_02, cubes_override=cubes_02)

    print("渲染 02_head 单部件视图...")
    img_f, _ = render(str(tmp_model_02), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_02), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_02), yaw=145, pitch=20, size=600)
    tmp_model_02.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/02_head.png
    ref_path = PARTS_REF_DIR / "02_head.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    # 裁切正面 (x in 92..708) 与侧面 (x in 785..1486)
    y_l, x_l = np.where(diff_ref[:, 92:708])
    ref_front = ref_im.crop((x_l.min() + 92, y_l.min(), x_l.max() + 92 + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 785:1486])
    ref_side = ref_im.crop((x_r.min() + 785, y_r.min(), x_r.max() + 785 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/02_head.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_02_head() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_02_head.png"
    card.save(check_card_path)
    print(f"✓ 02_head 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "02_head (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "02_head.png"
    sheet.save(single_render_path)
    print(f"✓ 02_head 单部件渲染图已输出: {single_render_path}")

    # 2. 01+02 累计拼装渲染 (Cumulative Assembly: 01 + 02)
    print("渲染 01+02 累计拼装视图...")
    cubes_accum = part_01_core_body() + part_02_head()
    tmp_model_accum = Path("/tmp/Zhinian_step_02_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01_core_body + 02_head", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_02_head.png"
    accum_card.save(accum_path)
    print(f"✓ 01+02 累计拼装渲染图已输出: {accum_path}")


def render_step_03():
    """按新流程：单独渲染 03_hair、输出与 parts_ref/03_hair.png 的并排对照图，以及 01+02+03 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 03_hair 单件渲染
    cubes_03 = part_03_hair()
    tmp_model_03 = Path("/tmp/Zhinian_step_03_hair.bbmodel")
    generate_bbmodel(tmp_model_03, cubes_override=cubes_03)

    print("渲染 03_hair 单部件视图...")
    img_f, _ = render(str(tmp_model_03), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_03), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_03), yaw=145, pitch=20, size=600)
    tmp_model_03.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/03_hair.png
    ref_path = PARTS_REF_DIR / "03_hair.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    y_l, x_l = np.where(diff_ref[:, 100:870])
    ref_front = ref_im.crop((x_l.min() + 100, y_l.min(), x_l.max() + 100 + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 930:1480])
    ref_side = ref_im.crop((x_r.min() + 930, y_r.min(), x_r.max() + 930 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/03_hair.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_03_hair() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_03_hair.png"
    card.save(check_card_path)
    print(f"✓ 03_hair 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "03_hair (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "03_hair.png"
    sheet.save(single_render_path)
    print(f"✓ 03_hair 单部件渲染图已输出: {single_render_path}")

    # 2. 01+02+03 累计拼装渲染 (Cumulative Assembly: 01 + 02 + 03)
    print("渲染 01+02+03 累计拼装视图...")
    cubes_accum = part_01_core_body() + part_02_head() + part_03_hair()
    tmp_model_accum = Path("/tmp/Zhinian_step_03_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01_core_body + 02_head + 03_hair", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_03_hair.png"
    accum_card.save(accum_path)
    print(f"✓ 01+02+03 累计拼装渲染图已输出: {accum_path}")


def render_step_04():
    """按新流程：单独渲染 04_torso_wraps_belt、输出与 parts_ref/04_torso_wraps_belt.png 的并排对照图，以及 01~04 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 04_torso_wraps_belt 单件渲染
    cubes_04 = part_04_torso_wraps_belt()
    tmp_model_04 = Path("/tmp/Zhinian_step_04_wraps.bbmodel")
    generate_bbmodel(tmp_model_04, cubes_override=cubes_04)

    print("渲染 04_torso_wraps_belt 单部件视图...")
    img_f, _ = render(str(tmp_model_04), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_04), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_04), yaw=145, pitch=20, size=600)
    tmp_model_04.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/04_torso_wraps_belt.png
    ref_path = PARTS_REF_DIR / "04_torso_wraps_belt.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    y_l, x_l = np.where(diff_ref[:, 180:740])
    ref_front = ref_im.crop((x_l.min() + 180, y_l.min(), x_l.max() + 180 + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 970:1380])
    ref_side = ref_im.crop((x_r.min() + 970, y_r.min(), x_r.max() + 970 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/04_torso_wraps_belt.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_04_torso_wraps_belt() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_04_torso_wraps_belt.png"
    card.save(check_card_path)
    print(f"✓ 04_torso_wraps_belt 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "04_torso_wraps_belt (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "04_torso_wraps_belt.png"
    sheet.save(single_render_path)
    print(f"✓ 04_torso_wraps_belt 单部件渲染图已输出: {single_render_path}")

    # 2. 01~04 累计拼装渲染 (Cumulative Assembly: 01 + 02 + 03 + 04)
    print("渲染 01~04 累计拼装视图...")
    cubes_accum = part_01_core_body() + part_02_head() + part_03_hair() + part_04_torso_wraps_belt()
    tmp_model_accum = Path("/tmp/Zhinian_step_04_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01_core_body + 02_head + 03_hair + 04_torso_wraps_belt", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_04_torso_wraps_belt.png"
    accum_card.save(accum_path)
    print(f"✓ 01~04 累计拼装渲染图已输出: {accum_path}")


def render_step_05():
    """按新流程：单独渲染 05_pauldrons、输出与 parts_ref/05_pauldrons.png 的并排对照图，以及 01~05 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 05_pauldrons 单件渲染
    cubes_05 = part_05_pauldrons()
    tmp_model_05 = Path("/tmp/Zhinian_step_05_pauldrons.bbmodel")
    generate_bbmodel(tmp_model_05, cubes_override=cubes_05)

    print("渲染 05_pauldrons 单部件视图...")
    img_f, _ = render(str(tmp_model_05), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_05), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_05), yaw=145, pitch=20, size=600)
    tmp_model_05.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/05_pauldrons.png
    ref_path = PARTS_REF_DIR / "05_pauldrons.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    y_l, x_l = np.where(diff_ref[:, 60:960])
    ref_front = ref_im.crop((x_l.min() + 60, y_l.min(), x_l.max() + 60 + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 1050:1470])
    ref_side = ref_im.crop((x_r.min() + 1050, y_r.min(), x_r.max() + 1050 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/05_pauldrons.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_05_pauldrons() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_05_pauldrons.png"
    card.save(check_card_path)
    print(f"✓ 05_pauldrons 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "05_pauldrons (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "05_pauldrons.png"
    sheet.save(single_render_path)
    print(f"✓ 05_pauldrons 单部件渲染图已输出: {single_render_path}")

    # 2. 01~05 累计拼装渲染 (Cumulative Assembly: 01 + 02 + 03 + 04 + 05)
    print("渲染 01~05 累计拼装视图...")
    cubes_accum = part_01_core_body() + part_02_head() + part_03_hair() + part_04_torso_wraps_belt() + part_05_pauldrons()
    tmp_model_accum = Path("/tmp/Zhinian_step_05_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01..05 (+ 05_pauldrons)", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_05_pauldrons.png"
    accum_card.save(accum_path)
    print(f"✓ 01~05 累计拼装渲染图已输出: {accum_path}")


def render_step_06():
    """按新流程：单独渲染 06_arms_claws、输出与 parts_ref/06_arms_claws.png 的并排对照图，以及 01~06 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 06_arms_claws 单件渲染
    cubes_06 = part_06_arms_claws()
    tmp_model_06 = Path("/tmp/Zhinian_step_06_arms.bbmodel")
    generate_bbmodel(tmp_model_06, cubes_override=cubes_06)

    print("渲染 06_arms_claws 单部件视图...")
    img_f, _ = render(str(tmp_model_06), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_06), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_06), yaw=145, pitch=20, size=600)
    tmp_model_06.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/06_arms_claws.png
    ref_path = PARTS_REF_DIR / "06_arms_claws.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    y_l, x_l = np.where(diff_ref[:, 260:700])
    ref_front = ref_im.crop((x_l.min() + 260, y_l.min(), x_l.max() + 260 + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 850:1320])
    ref_side = ref_im.crop((x_r.min() + 850, y_r.min(), x_r.max() + 850 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/06_arms_claws.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_06_arms_claws() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_06_arms_claws.png"
    card.save(check_card_path)
    print(f"✓ 06_arms_claws 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "06_arms_claws (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "06_arms_claws.png"
    sheet.save(single_render_path)
    print(f"✓ 06_arms_claws 单部件渲染图已输出: {single_render_path}")

    # 2. 01~06 累计拼装渲染 (Cumulative Assembly: 01~06)
    print("渲染 01~06 累计拼装视图...")
    cubes_accum = (
        part_01_core_body() +
        part_02_head() +
        part_03_hair() +
        part_04_torso_wraps_belt() +
        part_05_pauldrons() +
        part_06_arms_claws()
    )
    tmp_model_accum = Path("/tmp/Zhinian_step_06_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01~06 (core + head + hair + wraps + pauldrons + arms_claws)", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_06_arms_claws.png"
    accum_card.save(accum_path)
    print(f"✓ 01~06 累计拼装渲染图已输出: {accum_path}")


def render_step_07():
    """按新流程：单独渲染 07_sword、输出与 parts_ref/07_sword.png 的并排对照图，以及 01~07 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 07_sword 单件渲染
    cubes_07 = part_07_sword()
    tmp_model_07 = Path("/tmp/Zhinian_step_07_sword.bbmodel")
    generate_bbmodel(tmp_model_07, cubes_override=cubes_07)

    print("渲染 07_sword 单部件视图...")
    img_f, _ = render(str(tmp_model_07), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_07), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_07), yaw=145, pitch=20, size=600)
    tmp_model_07.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/07_sword.png
    ref_path = PARTS_REF_DIR / "07_sword.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    bg_ref = np.array([120, 120, 120])
    diff_ref = np.linalg.norm(arr_ref - bg_ref, axis=2) > 20

    y_l, x_l = np.where(diff_ref[:, 400:625])
    ref_front = ref_im.crop((x_l.min() + 400, y_l.min(), x_l.max() + 400 + 1, y_l.max() + 1))

    y_r, x_r = np.where(diff_ref[:, 975:1080])
    ref_side = ref_im.crop((x_r.min() + 975, y_r.min(), x_r.max() + 975 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/07_sword.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_07_sword() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_07_sword.png"
    card.save(check_card_path)
    print(f"✓ 07_sword 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "07_sword (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "07_sword.png"
    sheet.save(single_render_path)
    print(f"✓ 07_sword 单部件渲染图已输出: {single_render_path}")

    # 2. 01~07 累计拼装渲染 (Cumulative Assembly: 01~07)
    print("渲染 01~07 累计拼装视图...")
    cubes_accum = (
        part_01_core_body() +
        part_02_head() +
        part_03_hair() +
        part_04_torso_wraps_belt() +
        part_05_pauldrons() +
        part_06_arms_claws() +
        part_07_sword()
    )
    tmp_model_accum = Path("/tmp/Zhinian_step_07_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01~07 (core + head + hair + wraps + pauldrons + arms_claws + sword)", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_07_sword.png"
    accum_card.save(accum_path)
    print(f"✓ 01~07 累计拼装渲染图已输出: {accum_path}")


def render_step_08():
    """按新流程：单独渲染 08_robe_hem、输出与 parts_ref/08_robe_hem.png 的并排对照图，以及 01~08 累计拼装图。"""
    from bbmodel_maker.render.render_bbmodel import render
    PARTS_DIR.mkdir(parents=True, exist_ok=True)

    # 1. 08_robe_hem 单件渲染
    cubes_08 = part_08_robe_hem()
    tmp_model_08 = Path("/tmp/Zhinian_step_08_robe.bbmodel")
    generate_bbmodel(tmp_model_08, cubes_override=cubes_08)

    print("渲染 08_robe_hem 单部件视图...")
    img_f, _ = render(str(tmp_model_08), yaw=180, pitch=0, size=600)
    img_s, _ = render(str(tmp_model_08), yaw=-90, pitch=0, size=600)
    img_34, _ = render(str(tmp_model_08), yaw=145, pitch=20, size=600)
    tmp_model_08.unlink(missing_ok=True)

    def crop_tight(im, bg_thresh=15):
        arr = np.array(im)
        bg = arr[0, 0]
        diff = np.linalg.norm(arr - bg, axis=2) > bg_thresh
        ys, xs = np.where(diff)
        if len(xs) == 0:
            return im
        return im.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))

    nf_crop = crop_tight(img_f)
    ns_crop = crop_tight(img_s)
    n34_crop = crop_tight(img_34)

    # 加载参考图 parts_ref/08_robe_hem.png (背景深色，按亮度阈值裁切)
    ref_path = PARTS_REF_DIR / "08_robe_hem.png"
    if not ref_path.exists():
        print(f"[WARN] 未找到参考图: {ref_path}")
        return

    ref_im = Image.open(ref_path).convert("RGB")
    arr_ref = np.array(ref_im)
    diff_ref = arr_ref.mean(axis=2) > 22

    # 左图 (正面): x in 80..760
    y_l, x_l = np.where(diff_ref[:, 80:760])
    ref_front = ref_im.crop((x_l.min() + 80, y_l.min(), x_l.max() + 80 + 1, y_l.max() + 1))

    # 右图 (侧面): x in 880..1480
    y_r, x_r = np.where(diff_ref[:, 880:1480])
    ref_side = ref_im.crop((x_r.min() + 880, y_r.min(), x_r.max() + 880 + 1, y_r.max() + 1))

    TARGET_H = 600

    def scale_to_h(im, h):
        w = max(1, int(im.width * h / im.height))
        return im.resize((w, h), Image.Resampling.LANCZOS)

    rf_s = scale_to_h(ref_front, TARGET_H)
    rs_s = scale_to_h(ref_side, TARGET_H)

    nf_s = scale_to_h(nf_crop, TARGET_H)
    ns_s = scale_to_h(ns_crop, TARGET_H)
    n34_s = scale_to_h(n34_crop, TARGET_H)

    gap = 20
    ref_w = rf_s.width + rs_s.width + gap
    now_w = nf_s.width + ns_s.width + n34_s.width + 2 * gap

    card_w = ref_w + now_w + 3 * gap + 40
    card_h = TARGET_H + 80
    card = Image.new("RGB", (card_w, card_h), (28, 30, 34))
    draw = ImageDraw.Draw(card)

    x_curr = 20
    card.paste(rf_s, (x_curr, 60))
    x_curr += rf_s.width + gap
    card.paste(rs_s, (x_curr, 60))
    x_curr += rs_s.width + gap

    draw.line([(x_curr + gap // 2, 20), (x_curr + gap // 2, card_h - 20)], fill=(80, 84, 92), width=2)
    draw.text((25, 20), "REF: parts_ref/08_robe_hem.png (Front & Side)", fill=(192, 154, 96))

    x_curr += gap + 10
    draw.text((x_curr, 20), "NOW: part_08_robe_hem() (Front / Side / 3/4)", fill=(220, 225, 230))
    card.paste(nf_s, (x_curr, 60))
    x_curr += nf_s.width + gap
    card.paste(ns_s, (x_curr, 60))
    x_curr += ns_s.width + gap
    card.paste(n34_s, (x_curr, 60))

    check_card_path = PARTS_DIR / "check_08_robe_hem.png"
    card.save(check_card_path)
    print(f"✓ 08_robe_hem 并排对照图已输出: {check_card_path}")

    # 单部件渲染展示图
    sheet_w = nf_s.width + ns_s.width + n34_s.width + 4 * gap
    sheet = Image.new("RGB", (sheet_w, TARGET_H + 40), (28, 30, 34))
    s_draw = ImageDraw.Draw(sheet)
    s_draw.text((20, 10), "08_robe_hem (Front / Side / 3/4)", fill=(220, 225, 230))
    sx = gap
    sheet.paste(nf_s, (sx, 30))
    sx += nf_s.width + gap
    sheet.paste(ns_s, (sx, 30))
    sx += ns_s.width + gap
    sheet.paste(n34_s, (sx, 30))

    single_render_path = PARTS_DIR / "08_robe_hem.png"
    sheet.save(single_render_path)
    print(f"✓ 08_robe_hem 单部件渲染图已输出: {single_render_path}")

    # 2. 01~08 累计拼装渲染 (Cumulative Assembly: 01~08 全量最终拼装)
    print("渲染 01~08 全量最终累计拼装视图...")
    cubes_accum = (
        part_01_core_body() +
        part_02_head() +
        part_03_hair() +
        part_04_torso_wraps_belt() +
        part_05_pauldrons() +
        part_06_arms_claws() +
        part_07_sword() +
        part_08_robe_hem()
    )
    tmp_model_accum = Path("/tmp/Zhinian_step_08_accum.bbmodel")
    generate_bbmodel(tmp_model_accum, cubes_override=cubes_accum)

    acc_f, _ = render(str(tmp_model_accum), yaw=180, pitch=0, size=600)
    acc_s, _ = render(str(tmp_model_accum), yaw=-90, pitch=0, size=600)
    acc_34, _ = render(str(tmp_model_accum), yaw=145, pitch=20, size=600)
    tmp_model_accum.unlink(missing_ok=True)

    af_c = crop_tight(acc_f)
    as_c = crop_tight(acc_s)
    a34_c = crop_tight(acc_34)

    af_s = scale_to_h(af_c, TARGET_H)
    as_s = scale_to_h(as_c, TARGET_H)
    a34_s = scale_to_h(a34_c, TARGET_H)

    accum_w = af_s.width + as_s.width + a34_s.width + 4 * gap
    accum_card = Image.new("RGB", (accum_w, TARGET_H + 40), (28, 30, 34))
    a_draw = ImageDraw.Draw(accum_card)
    a_draw.text((20, 10), "Accumulated Assembly: 01~08 Complete (core + head + hair + wraps + pauldrons + arms + sword + robe_hem)", fill=(220, 225, 230))
    ax = gap
    accum_card.paste(af_s, (ax, 30))
    ax += af_s.width + gap
    accum_card.paste(as_s, (ax, 30))
    ax += as_s.width + gap
    accum_card.paste(a34_s, (ax, 30))

    accum_path = PARTS_DIR / "accum_08_robe_hem.png"
    accum_card.save(accum_path)
    print(f"✓ 01~08 累计拼装渲染图已输出: {accum_path}")


def self_test():
    """差分自证：验证当前模型共面校验通过，并能成功捕获注入缺陷。"""
    print("运行 gen_zhinian.py 差分自证...")
    cubes = all_cubes()
    try:
        _assert_no_coplanar_faces(cubes)
        print("  [OK] 正常立方体集无共面冲突")
    except AssertionError as e:
        print(f"  [FAIL] 正常立方体集出现共面冲突: {e}")
        sys.exit(1)

    defect_cubes = list(cubes)
    defect_cubes.append({
        "name": "inject_coplanar_fail",
        "from": [6.0, 11.2, 7.0],
        "to": [10.0, 13.5, 9.0],  # 与 core_pelvis 完全重叠
        "group": "core_body",
        "material": "robe_dark",
    })
    try:
        _assert_no_coplanar_faces(defect_cubes)
        print("  [FAIL] 注入缺陷未被门禁捕获！")
        sys.exit(1)
    except AssertionError as e:
        print(f"  [OK] 成功捕获注入缺陷: {e}")

    twice = turn_to_plus_z(turn_to_plus_z(cubes))
    for original, round_trip in zip(cubes, twice):
        for key in ("from", "to", "origin", "rotation"):
            if key not in original:
                continue
            if any(abs(a - b) > 1e-9 for a, b in zip(original[key], round_trip[key])):
                print(f"  [FAIL] 转向两次未还原 {original['name']}.{key}")
                sys.exit(1)
    eyes = [c for c in cubes if c["name"].startswith("head_eye")]
    eye_z_before = sum((c["from"][2] + c["to"][2]) / 2 for c in eyes) / len(eyes)
    eye_z_after = sum((c["from"][2] + c["to"][2]) / 2 for c in turn_to_plus_z(eyes)) / len(eyes)
    if not (eye_z_before < TURN_PIVOT_Z < eye_z_after):
        print(f"  [FAIL] 转向后眼睛没有越过中轴到 +Z 侧: {eye_z_before:.2f} -> {eye_z_after:.2f}")
        sys.exit(1)
    print(f"  [OK] 整体转向：两次还原，眼睛 z {eye_z_before:.2f} -> {eye_z_after:.2f}（朝 +Z）")

    print("✓ gen_zhinian.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="生成末法残土执念 (Zhinian) .bbmodel")
    parser.add_argument("--self-test", action="store_true", help="运行门禁缺陷注入差分自证")
    parser.add_argument("--part", type=str, default=None, help="渲染并对比指定编号部件 (如 01)；缺省只写 bbmodel，不渲染")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    generate_bbmodel(args.out)
    if args.part == "01":
        render_step_01()
    elif args.part == "02":
        render_step_02()
    elif args.part == "03":
        render_step_03()
    elif args.part == "04":
        render_step_04()
    elif args.part == "05":
        render_step_05()
    elif args.part == "06":
        render_step_06()
    elif args.part == "07":
        render_step_07()
    elif args.part == "08":
        render_step_08()


if __name__ == "__main__":
    main()
