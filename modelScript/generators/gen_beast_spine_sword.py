#!/usr/bin/env python3
"""异兽脊骨剑（Beast Spine Sword / 髓骨残剑）Blockbench .bbmodel 生成器。

依据概念图 (Concept Art) 与爆炸分解图 (Exploded Breakdown) 复刻形制，
自柄尾向剑尖分 6 大 Group（Bone 骨骼节点）：

    1. pommel      - 杂铜配重柄首（带八角铜箍与挂环）
    2. tassel      - 挂穗流苏（2条暗红/枯草褐分叉编织流苏，自然下垂）
    3. grip        - 缠绳握把（暗红风干兽筋/粗麻螺旋缠绕，带抓握起伏）
    4. guard       - 杂铜吞口剑格（兽口吞金形制，微翘两翼与咬合口环）
    5. blade_spine - 脊椎骨节剑身（中心髓梁 + 8节逐节递减脊椎骨环 + 双侧对称向后倒钩骨刺棘突 + 朱砂封灵槽）
    6. blade_tip   - 破甲骨尖（收拢锐利骨尖与尖牙收口）

尺寸规范（MC px，16px = 1 格）：
    总长约 24.0px ≈ 1.5 格（剑刃 ~17px，握把+柄首 ~7px）。
    握把半宽约 0.8px（Ø1.6px），适合玩家单手/双手一握。

贴图规范（64×64 四象限 Atlas）：
    - 骨质 (bone): 灰白、米黄、骨孔噪点与骨缝阴影
    - 朱砂封灵 (cinnabar): 暗红、朱砂篆文、渗漏血痕
    - 杂铜五金 (bronze): 暗杂铜、微绿铜锈斑、边缘磨光高光
    - 麻绳流苏 (cord): 暗红/深褐麻绳编织纹、流苏细丝

用法:
    python3 modelScript/generators/gen_beast_spine_sword.py
    python3 modelScript/generators/gen_beast_spine_sword.py --preview-only
    bbmodel-render modelScript/models/BeastSpineSword.bbmodel --three-view
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import tempfile
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "BeastSpineSword.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "beast_spine_sword_preview.png"

PX = 16.0
RES = 64

# ── 纵向分段与尺寸（y: 柄尾=0.0，尖端朝 +Y）─────────────────────────────
POMMEL_Y = (0.00, 1.30)      # 杂铜柄首
GRIP_Y = (1.30, 5.50)        # 缠绳握把（长 4.2px，Ø1.5px）
GUARD_Y = (5.50, 7.20)       # 杂铜兽口吞口剑格（长 1.7px）
BLADE_Y0 = 7.20              # 剑身骨节起点
BLADE_LEN = 17.00            # 10 节脊椎总长
TIP_LEN = 3.60               # 穿刺骨尖长（总长 7.2 + 17.0 + 3.6 = 27.8px ≈ 1.74格）

# ── 授权系 → 出料系 ───────────────────────────────────────────────────────
# 上面的 box 表是**授权系**（柄尾 y=0、尖端朝 +Y），读写都顺手。但 MC 的 display
# 变换绕**方块中心**转，不绕模型原点（`ItemRenderer.renderItem` 在 display 之后
# 还有一个 translate(-0.5,-0.5,-0.5)），所以出料必须按 `held_item_common.emit_offset`
# 把**握把点挪到方块中心**：emit = (8, 8 - grip, 8)。
#
# 差这一步的症状不是"display 数值没调好"，是整整半个方块的系统性偏移：预览里剑飘在
# 拳头外（`preview_player_anim.py --hold` 实测偏出一整个身位），GUI 图标被推到格子角上。
GRIP_PX = (GRIP_Y[0] + GRIP_Y[1]) / 2.0   # 3.4 —— 拳心对准处 = 缠绳握把中点
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - GRIP_PX, BLOCK_CENTRE_PX)

SPINE_SEGS = 10              # 10 节脊椎骨
HW_ROOT = 0.95               # 剑身根部中心椎骨半宽
HW_TIP = 0.45                # 剑身末节中心椎骨半宽
THICK_ROOT = 0.55            # 剑身根部半厚
THICK_TIP = 0.28             # 剑身末节半厚

# Group 定义（严格按层级组织）
BONE_ORDER = ["pommel", "tassel", "grip", "guard", "blade_spine", "blade_tip"]
BONE_COLORS = {
    "pommel": (188, 154, 82),      # 杂铜
    "tassel": (150, 45, 40),       # 暗红流苏
    "grip": (92, 52, 40),          # 缠绳
    "guard": (176, 142, 72),       # 剑格吞口
    "blade_spine": (210, 205, 192),# 脊椎骨
    "blade_tip": (185, 180, 170),  # 骨尖
}

# 贴图象限规划 (64x64)
# Q1 (0..32, 0..32): 骨骼主体 (bone)
# Q2 (32..64, 0..32): 朱砂封灵 (cinnabar)
# Q3 (0..32, 32..64): 杂铜五金 (bronze)
# Q4 (32..64, 32..64): 绳结与流苏 (cord)
MAT_ZONE = {
    "bone": (0, 0, 32, 32),
    "cinnabar": (32, 0, 64, 32),
    "bronze": (0, 32, 32, 64),
    "cord": (32, 32, 64, 64),
}


def build_cubes():
    """构建所有立方体元素，组织进对应 Group。
    返回列表: [(bone_name, mat_key, cube_name, from_xyz, to_xyz, rot_xyz)]
    """
    cubes: list[tuple] = []

    def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0)):
        # 确保 x0<x1, y0<y1, z0<z1
        fx, tx = min(x0, x1), max(x0, x1)
        fy, ty = min(y0, y1), max(y0, y1)
        fz, tz = min(z0, z1), max(z0, z1)
        cubes.append((bone, mat, name, [fx, fy, fz], [tx, ty, tz], tuple(rot)))

    def octagon(bone, mat, name, hw, y0, y1, hz=None):
        """用两个重叠的 0° / 45° 盒拟合八角圆柱"""
        hz = hw if hz is None else hz
        block(bone, mat, f"{name}_0", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 0.0, 0.0))
        block(bone, mat, f"{name}_45", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 45.0, 0.0))

    # ═════════════════════════════════════════════════════════════════════════
    # 1. POMMEL（杂铜柄首）
    # ═════════════════════════════════════════════════════════════════════════
    # 底部配重端盖 + 铜箍包边
    octagon("pommel", "bronze", "pommel_cap", 0.95, POMMEL_Y[0], POMMEL_Y[0] + 0.50)
    octagon("pommel", "bronze", "pommel_ring", 1.15, POMMEL_Y[0] + 0.50, POMMEL_Y[0] + 0.90)
    octagon("pommel", "bronze", "pommel_neck", 0.85, POMMEL_Y[0] + 0.90, POMMEL_Y[1])
    # 挂环（侧向突出的铜环孔）
    block("pommel", "bronze", "pommel_loop_l", -1.25, -0.85, 0.30, 0.80, -0.20, 0.20)
    block("pommel", "bronze", "pommel_loop_r", 0.85, 1.25, 0.30, 0.80, -0.20, 0.20)

    # ═════════════════════════════════════════════════════════════════════════
    # 2. TASSEL（流苏挂穗）
    # ═════════════════════════════════════════════════════════════════════════
    # 从挂环下垂的系绳与双流苏
    # 左侧主流苏
    block("tassel", "cord", "tassel_rope_l1", -1.15, -0.95, -0.80, 0.40, -0.10, 0.10, rot=(0.0, 0.0, 8.0))
    block("tassel", "cord", "tassel_knot_l", -1.30, -0.80, -1.30, -0.80, -0.25, 0.25)
    block("tassel", "cord", "tassel_fringe_l1", -1.35, -0.75, -3.20, -1.30, -0.30, 0.30, rot=(0.0, 0.0, 4.0))
    block("tassel", "cord", "tassel_fringe_l2", -1.25, -0.85, -4.60, -3.20, -0.20, 0.20, rot=(0.0, 0.0, -3.0))

    # 右侧副流苏（略短，自然分叉）
    block("tassel", "cord", "tassel_rope_r1", 0.95, 1.15, -0.50, 0.40, -0.10, 0.10, rot=(0.0, 0.0, -10.0))
    block("tassel", "cord", "tassel_knot_r", 0.80, 1.30, -0.95, -0.50, -0.22, 0.22)
    block("tassel", "cord", "tassel_fringe_r1", 0.75, 1.35, -2.60, -0.95, -0.28, 0.28, rot=(0.0, 0.0, -6.0))
    block("tassel", "cord", "tassel_fringe_r2", 0.85, 1.25, -3.80, -2.60, -0.18, 0.18, rot=(0.0, 0.0, 2.0))

    # ═════════════════════════════════════════════════════════════════════════
    # 3. GRIP（缠绳握把）
    # ═════════════════════════════════════════════════════════════════════════
    # 握把内芯 (暗木/硬骨) + 4 段螺旋凸起的缠绳凸环
    octagon("grip", "cord", "grip_core", 0.72, GRIP_Y[0], GRIP_Y[1])
    # 4 圈外凸缠绳（模拟风干兽筋与粗麻的交错包裹厚度）
    wrap_steps = 4
    for i in range(wrap_steps):
        wy0 = GRIP_Y[0] + i * (GRIP_Y[1] - GRIP_Y[0]) / wrap_steps
        wy1 = wy0 + 0.55
        octagon("grip", "cord", f"grip_wrap_{i}", 0.86, wy0, wy1)

    # ═════════════════════════════════════════════════════════════════════════
    # 4. GUARD（杂铜兽口吞金剑格）
    # ═════════════════════════════════════════════════════════════════════════
    # 剑格基座（衔接握把）
    octagon("guard", "bronze", "guard_collar", 0.98, GUARD_Y[0], GUARD_Y[0] + 0.35)
    # 剑格主体（宽大兽面吞口，前后宽、左右外展）
    block("guard", "bronze", "guard_main", -1.80, 1.80, GUARD_Y[0] + 0.30, GUARD_Y[0] + 1.10, -1.00, 1.00)
    # 左右两翼上翘小角
    block("guard", "bronze", "guard_wing_l", -2.60, -1.75, GUARD_Y[0] + 0.60, GUARD_Y[0] + 1.40, -0.60, 0.60, rot=(0.0, 0.0, -18.0))
    block("guard", "bronze", "guard_wing_r", 1.75, 2.60, GUARD_Y[0] + 0.60, GUARD_Y[0] + 1.40, -0.60, 0.60, rot=(0.0, 0.0, 18.0))
    # 前后兽吻獠牙（突出咬合骨节的口环）
    block("guard", "bronze", "guard_snout_f", -0.90, 0.90, GUARD_Y[0] + 0.70, GUARD_Y[1], 0.95, 1.35)
    block("guard", "bronze", "guard_snout_b", -0.90, 0.90, GUARD_Y[0] + 0.70, GUARD_Y[1], -1.35, -0.95)
    # 吞口侧向獠牙
    block("guard", "bronze", "guard_fang_fl", -1.40, -0.90, GUARD_Y[0] + 0.80, GUARD_Y[1] + 0.40, 0.85, 1.25)
    block("guard", "bronze", "guard_fang_fr", 0.90, 1.40, GUARD_Y[0] + 0.80, GUARD_Y[1] + 0.40, 0.85, 1.25)
    block("guard", "bronze", "guard_fang_bl", -1.40, -0.90, GUARD_Y[0] + 0.80, GUARD_Y[1] + 0.40, -1.25, -0.85)
    block("guard", "bronze", "guard_fang_br", 0.90, 1.40, GUARD_Y[0] + 0.80, GUARD_Y[1] + 0.40, -1.25, -0.85)

    # ═════════════════════════════════════════════════════════════════════════
    # 5. BLADE_SPINE（10 节脊椎骨剑身 + 双侧倒钩棘突 + 朱砂封灵槽）
    # ═════════════════════════════════════════════════════════════════════════
    seg_h = BLADE_LEN / SPINE_SEGS   # 每节约 1.70px

    # (A) 贯穿全剑的中心硬质髓腔主梁与朱砂槽
    block("blade_spine", "cinnabar", "marrow_core_cinnabar", -0.16, 0.16, BLADE_Y0, BLADE_Y0 + BLADE_LEN, -0.16, 0.16)

    for i in range(SPINE_SEGS):
        t = i / (SPINE_SEGS - 1)      # 0.0 -> 1.0 (从根部到末端)
        y0 = BLADE_Y0 + i * seg_h
        y1 = y0 + seg_h
        hw = HW_ROOT + (HW_TIP - HW_ROOT) * t
        th = THICK_ROOT + (THICK_TIP - THICK_ROOT) * t

        # 每节脊椎主体椎体（八角圆盘椎体）
        octagon("blade_spine", "bone", f"vertebra_{i}", hw * 0.72, y0 + 0.08, y1 - 0.08, hz=th)

        # 椎体上下关节突小结节（增加骨骼凹凸生物感）
        block("blade_spine", "bone", f"joint_f_{i}", -hw * 0.40, hw * 0.40, y0, y0 + 0.35, th * 0.7, th * 1.15)
        block("blade_spine", "bone", f"joint_b_{i}", -hw * 0.40, hw * 0.40, y0, y0 + 0.35, -th * 1.15, -th * 0.7)

        # 中轴朱砂凹槽镶嵌面（骨面开槽露出朱砂灵纹）
        block("blade_spine", "cinnabar", f"cinnabar_runic_f_{i}", -0.22, 0.22, y0 + 0.15, y1 - 0.15, th * 0.88, th * 1.02)
        block("blade_spine", "cinnabar", f"cinnabar_runic_b_{i}", -0.22, 0.22, y0 + 0.15, y1 - 0.15, -1.02 * th, -0.88 * th)

        # 左右两侧的骨刺棘突 (Bone Spurs) - 改为双节细长骨刺（内基座 + 锐利斜刺）
        spur_span = 1.10 - 0.50 * t   # 刺长：根部 1.10，尖端 0.60
        spur_h = seg_h * 0.45         # 刺身高度减小到 0.45 seg_h，呈现刀刃刺状而非方块
        spur_y0 = y0 + seg_h * 0.25
        spur_y1 = spur_y0 + spur_h
        spur_thick = 0.16             # 刺片厚度薄（0.16px），更加锋锐

        # 骨刺基座横向过渡段
        block("blade_spine", "bone", f"spur_base_l_{i}", -hw - 0.25, -hw * 0.65, spur_y0, spur_y1, -spur_thick * 1.2, spur_thick * 1.2)
        block("blade_spine", "bone", f"spur_base_r_{i}", hw * 0.65, hw + 0.25, spur_y0, spur_y1, -spur_thick * 1.2, spur_thick * 1.2)

        # 外延斜向骨刺（rot_z = 38°，向柄部倾斜呈锐利倒钩）
        block(
            "blade_spine", "bone", f"spur_blade_l_{i}",
            -hw - spur_span, -hw - 0.10,
            spur_y0, spur_y1,
            -spur_thick, spur_thick,
            rot=(0.0, 0.0, 38.0)
        )
        block(
            "blade_spine", "bone", f"spur_blade_r_{i}",
            hw + 0.10, hw + spur_span,
            spur_y0, spur_y1,
            -spur_thick, spur_thick,
            rot=(0.0, 0.0, -38.0)
        )

    # ═════════════════════════════════════════════════════════════════════════
    # 6. BLADE_TIP（多段穿刺骨尖与破甲倒刺）
    # ═════════════════════════════════════════════════════════════════════════
    y_tip0 = BLADE_Y0 + BLADE_LEN
    y_tip1 = y_tip0 + TIP_LEN * 0.45
    y_tip2 = y_tip0 + TIP_LEN * 0.80
    y_tip3 = y_tip0 + TIP_LEN

    # 骨尖基座收缩段
    block("blade_tip", "bone", "tip_base", -HW_TIP * 0.70, HW_TIP * 0.70, y_tip0, y_tip1, -THICK_TIP * 0.75, THICK_TIP * 0.75)
    block("blade_tip", "cinnabar", "tip_cinnabar_1", -0.15, 0.15, y_tip0, y_tip1, -0.15, 0.15)

    # 骨尖中段（明显收细）
    block("blade_tip", "bone", "tip_mid", -0.32, 0.32, y_tip1, y_tip2, -0.22, 0.22)
    block("blade_tip", "cinnabar", "tip_cinnabar_2", -0.10, 0.10, y_tip1, y_tip2, -0.10, 0.10)

    # 骨尖末端极尖刺刃
    block("blade_tip", "bone", "tip_needle", -0.16, 0.16, y_tip2, y_tip3, -0.14, 0.14)

    # 尖端两侧破甲微倒钩
    block("blade_tip", "bone", "tip_barb_l", -0.45, -0.10, y_tip0 + 0.40, y_tip1 + 0.30, -0.12, 0.12, rot=(0.0, 0.0, 36.0))
    block("blade_tip", "bone", "tip_barb_r", 0.10, 0.45, y_tip0 + 0.40, y_tip1 + 0.30, -0.12, 0.12, rot=(0.0, 0.0, -36.0))

    return cubes


# ── 贴图生成（64×64 四象限 Atlas）───────────────────────────────────────
def make_texture(res=RES, seed=0x7E1B):
    """绘制高质感 64x64 四象限程序化贴图"""
    rng = np.random.default_rng(seed)
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    y, x = np.mgrid[0:res, 0:res]

    # ── Q1: (0..32, 0..32) 骨骼主体 (Bone & Vertebrae) ───────────────────
    # 灰白微黄风干骨质，带有骨髓微孔与细密纵向骨纤维
    q1_mask = (x < 32) & (y < 32)
    bone_base = np.array([218, 212, 196], float)[None, None, :]
    bone_fiber = 0.5 + 0.5 * np.sin(x * 2.4 + np.sin(y * 0.8) * 1.2)
    bone_col = bone_base + (bone_fiber[..., None] - 0.5) * 26
    bone_col += (rng.random((res, res, 1)) - 0.5) * 16

    # 骨孔斑点与干裂缝隙
    for _ in range(24):
        cx, cy = rng.integers(1, 31), rng.integers(1, 31)
        r = rng.integers(1, 3)
        pmask = ((x - cx) ** 2 + (y - cy) ** 2) <= r ** 2
        bone_col[pmask & q1_mask] *= rng.uniform(0.68, 0.84)

    bone_col = np.clip(bone_col, 130, 245)
    img[q1_mask, :3] = bone_col[q1_mask].astype(np.uint8)

    # ── Q2: (32..64, 0..32) 朱砂封灵 (Cinnabar Inlays) ───────────────────
    # 浓郁暗红与朱砂红，带真元流光与渗出龟裂纹理
    q2_mask = (x >= 32) & (y < 32)
    cin_base = np.array([168, 38, 32], float)[None, None, :]
    cin_glow = 0.5 + 0.5 * np.sin(x * 1.8 + y * 1.5)
    cin_col = cin_base + (cin_glow[..., None] - 0.5) * 44
    cin_col += (rng.random((res, res, 1)) - 0.5) * 18

    # 符文篆刻暗金/黑红边缘
    for _ in range(16):
        cx, cy = rng.integers(33, 63), rng.integers(1, 31)
        ln = rng.integers(3, 8)
        for k in range(ln):
            px = np.clip(cx + k, 32, 63)
            py = np.clip(cy + (k % 2), 0, 31)
            cin_col[py, px] = np.array([90, 18, 16])

    cin_col = np.clip(cin_col, 40, 220)
    img[q2_mask, :3] = cin_col[q2_mask].astype(np.uint8)

    # ── Q3: (0..32, 32..64) 杂铜五金 (Weathered Bronze) ───────────────────
    # 暗沉黄铜，伴随边缘磨损高光与零星铜绿蚀点
    q3_mask = (x < 32) & (y >= 32)
    brz_base = np.array([165, 132, 68], float)[None, None, :]
    brz_lobe = 0.5 + 0.5 * np.sin(x * 2.2 + y * 0.6)
    brz_col = brz_base + (brz_lobe[..., None] - 0.5) * 38
    brz_col += (rng.random((res, res, 1)) - 0.5) * 14

    # 铜绿氧化斑 (Verdigris patina)
    for _ in range(12):
        cx, cy = rng.integers(2, 30), rng.integers(34, 62)
        r = rng.integers(1, 4)
        pmask = ((x - cx) ** 2 + (y - cy) ** 2) <= r ** 2
        brz_col[pmask & q3_mask] = brz_col[pmask & q3_mask] * 0.45 + np.array([88, 134, 102]) * 0.55

    brz_col = np.clip(brz_col, 55, 215)
    img[q3_mask, :3] = brz_col[q3_mask].astype(np.uint8)

    # ── Q4: (32..64, 32..64) 绳结与流苏 (Cord & Tassels) ───────────────────
    # 暗红褐与风干兽筋麻绳，带有斜向编织条纹
    q4_mask = (x >= 32) & (y >= 32)
    cord_base = np.array([105, 52, 42], float)[None, None, :]
    cord_weave = 0.5 + 0.5 * np.sin((x + y) * 2.6)
    cord_col = cord_base + (cord_weave[..., None] - 0.5) * 36
    cord_col += (rng.random((res, res, 1)) - 0.5) * 12

    # 流苏纤维暗深阴影线
    for _ in range(14):
        cx = rng.integers(33, 63)
        cord_col[32:64, cx] *= rng.uniform(0.72, 0.88)

    cord_col = np.clip(cord_col, 35, 175)
    img[q4_mask, :3] = cord_col[q4_mask].astype(np.uint8)

    return Image.fromarray(img, "RGBA")


def png_data_url(img: Image.Image) -> str:
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class Packer:
    """在象限矩形区域内进行 UV 排布"""
    def __init__(self, x0, y0, x1, y1):
        self.x0, self.y0, self.x1, self.y1 = x0, y0, x1, y1
        self.x, self.y, self.rowh = x0, y0, 0.0

    def place(self, w, h):
        w = max(0.5, min(w, self.x1 - self.x0))
        h = max(0.5, min(h, self.y1 - self.y0))
        if self.x + w > self.x1:
            self.x = self.x0
            self.y += self.rowh
            self.rowh = 0.0
        if self.y + h > self.y1:
            self.y = self.y0
        ox, oy = self.x, self.y
        self.x += w
        self.rowh = max(self.rowh, h)
        return ox, oy


def cube_faces_uv(frm, to, packer: Packer):
    dx = abs(to[0] - frm[0])
    dy = abs(to[1] - frm[1])
    dz = abs(to[2] - frm[2])
    dims = {
        "north": (dx, dy),
        "south": (dx, dy),
        "east": (dz, dy),
        "west": (dz, dy),
        "up": (dx, dz),
        "down": (dx, dz),
    }
    faces = {}
    for name, (w, h) in dims.items():
        ox, oy = packer.place(w, h)
        faces[name] = {
            "uv": [round(ox, 2), round(oy, 2), round(ox + w, 2), round(oy + h, 2)],
            "texture": 0,
        }
    return faces


def build_bbmodel() -> dict:
    """组装符合 Blockbench 格式规范的字典树"""
    all_cubes = build_cubes()

    packers = {m: Packer(*z) for m, z in MAT_ZONE.items()}
    uv_cache: dict[str, dict] = {}
    elements = []

    # 按照 Group 收集 uuid
    group_children: dict[str, list[str]] = {b: [] for b in BONE_ORDER}

    for bone, material, name, frm, to, rot in all_cubes:
        # UV 在授权系里算（贴图排布只看尺寸，不看绝对位置）
        if name not in uv_cache:
            uv_cache[name] = cube_faces_uv(frm, to, packers[material])

        # 出料系 = 授权系 + EMIT_OFFSET（握把点落方块中心，见文件头常量段）
        frm = [frm[i] + EMIT_OFFSET[i] for i in range(3)]
        to = [to[i] + EMIT_OFFSET[i] for i in range(3)]

        cx = (frm[0] + to[0]) / 2.0
        cy = (frm[1] + to[1]) / 2.0
        cz = (frm[2] + to[2]) / 2.0
        cube_uuid = str(uuid.uuid4())
        group_children[bone].append(cube_uuid)

        elements.append({
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": cube_uuid,
            "from": [round(v, 3) for v in frm],
            "to": [round(v, 3) for v in to],
            "autouv": 0,
            "color": BONE_ORDER.index(bone),
            "origin": [round(cx, 3), round(cy, 3), round(cz, 3)],
            "rotation": [round(r, 3) for r in rot],
            "faces": {k: {"uv": list(v["uv"]), "texture": 0} for k, v in uv_cache[name].items()},
        })

    tex_img = make_texture()
    tex_uuid = str(uuid.uuid4())

    out_groups = []
    for b in BONE_ORDER:
        out_groups.append({
            "name": b,
            "origin": [0.0, 0.0, 0.0],
            "color": BONE_ORDER.index(b),
            "uuid": str(uuid.uuid4()),
            "export": True,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": group_children[b],
        })

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "BeastSpineSword",
        "model_identifier": "beast_spine_sword",
        "visible_box": [1, 1, 0],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": out_groups,
        "textures": [
            {
                "path": "beast_spine_sword.png",
                "name": "beast_spine_sword",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": tex_uuid,
                "source": png_data_url(tex_img),
            }
        ],
    }
    return bbmodel


def render_preview(bbmodel_data: dict, out: Path | None = None) -> Path:
    """出一张 3/4 视角预览图。

    `render_bbmodel.render` 吃的是**文件路径**不是 dict，所以先落一份临时 .bbmodel
    再渲——`--preview-only` 下不能污染 `args.out`，临时文件用完即删。

    `out` 缺省在**调用时**解析成 `PREVIEW_OUT`，不写成默认参数值：默认参数在函数定义
    时就求值了，写死了调用方（和测试）就再也换不掉输出路径。
    """
    from bbmodel_maker.render import render_bbmodel as R

    out = Path(out) if out is not None else PREVIEW_OUT
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        tmp_model = Path(tmp) / "preview.bbmodel"
        tmp_model.write_text(json.dumps(bbmodel_data, ensure_ascii=False), encoding="utf-8")
        img = R.render(str(tmp_model), size=600, shading="mc")
    if isinstance(img, tuple):
        img = img[0]
    img.save(out)
    return out


def main():
    parser = argparse.ArgumentParser(description="生成异兽脊骨剑 .bbmodel 模型")
    parser.add_argument("--preview-only", action="store_true", help="仅渲染预览图，不写 .bbmodel")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    args = parser.parse_args()

    bbmodel_data = build_bbmodel()

    # 与 gen_array_flag / gen_bamboo_jian 同构：--preview-only 只跳过写模型，预览照出。
    # 之前这个参数声明了却没人读，传了照样覆盖写 args.out 且一张预览也不产出（Kody 在
    # PR #2128 上点出来的，属实）。
    if not args.preview_only:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(bbmodel_data, f, indent=2, ensure_ascii=False)
        print(f"✓ 成功导出 .bbmodel: {args.out}")
        print(f"  包含 {len(bbmodel_data['elements'])} 个立方体，{len(bbmodel_data['outliner'])} 个 Group。")

    preview = render_preview(bbmodel_data)
    print(f"  → preview: {preview}")


if __name__ == "__main__":
    main()
