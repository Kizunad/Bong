#!/usr/bin/env python3
"""生成赤髓草 (ChiSuiCao) Blockbench .bbmodel 与三视图预览 (Round 3 终极打磨·血谷玛瑙剑草完美版)。

【世界观正典与原画依据】：
- 定位：《末法残土》卷首核心剧情物（血谷必争之草），《末法药材十七种》稀见五味之八。
  “仅生于血谷东侧红砂岩缝。乃凝脉散之高阶替品……叶中暗含血髓，犹如活物跳动。”
- 原画特征 (client/src/main/resources/assets/bong-client/textures/gui/botany/chi_sui_cao.png)：
  1. 斜生暗红血质主茎 (Dark Red Stalk)，自岩缝中斜向上挺立微曲；
  2. 5 片互生/对生的卵圆剑形暗红玛瑙厚叶 (Agate Blade Leaves)，每片叶子肉质如黑红玛瑙宝石，叶缘微泛血光；
  3. 贯穿每片叶心的鲜红搏动血髓脉 (Pulsing Marrow Veins)，随叶身舒展微翘；
  4. 基部断口处凝结悬垂的活物般鲜血髓滴 (Dripping Blood Droplet)；
  5. 夹峙斜切的血谷风化红砂岩缝基座 (Red Sandstone Chasm Base)。
"""

from __future__ import annotations

import base64
import io
import json
import math
import uuid
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = REPO / "local_models"
PREVIEW_DIR = REPO / "scripts" / "models"
TEXTURE_RES = 64
UUID_NAMESPACE = uuid.UUID("c7b8d9e0-f1a2-4b3c-9d4e-5f6a7b8c9d0e")


@dataclass(frozen=True)
class Cube:
    bone: str
    name: str
    origin: tuple[float, float, float]
    target: tuple[float, float, float]
    front_uv: str
    side_uv: str = "sandstone_dark"
    back_uv: str = "sandstone_dark"
    top_uv: str = "sandstone_dark"
    bottom_uv: str = "sandstone_dark"
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    rot_origin: tuple[float, float, float] = (0.0, 0.0, 0.0)


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(UUID_NAMESPACE, ":".join(parts)))


# ─────────────────────────────────────────────────────────────────────────────
# 1. 结构拆解：红砂岩缝基座、斜生主茎、五瓣玛瑙剑叶与发光血髓
# ─────────────────────────────────────────────────────────────────────────────

def part_sandstone_chasm() -> list[Cube]:
    """部件1：血谷红砂岩缝基座 (part_sandstone_chasm)。
    位于 Y: 0.0 ~ 3.8，斜切层叠的红砂岩，形成深邃夹峙岩缝。
    """
    cubes: list[Cube] = []

    # 1. 底层沉稳主岩台 (Y: 0.0 ~ 0.8)
    cubes.append(
        Cube(
            "chasm",
            "rock_base_slab",
            (4.0, 0.0, 4.0),
            (12.0, 0.8, 12.0),
            front_uv="sandstone_slab",
            side_uv="sandstone_dark",
            back_uv="sandstone_dark",
            top_uv="sandstone_slab",
            bottom_uv="sandstone_dark",
        )
    )

    # 2. 左侧夹峙红砂岩峭壁 (Left Chasm Rock Y: 0.6 ~ 3.6, X: 3.0 ~ 6.6)
    cubes.append(
        Cube(
            "chasm",
            "rock_left_cliff_main",
            (3.0, 0.6, 5.0),
            (6.6, 3.4, 10.6),
            front_uv="sandstone_cliff",
            side_uv="sandstone_dark",
            back_uv="sandstone_dark",
            top_uv="sandstone_slab",
            bottom_uv="sandstone_dark",
            rotation=(4.0, 16.0, -8.0),
            rot_origin=(4.8, 0.6, 7.8),
        )
    )
    # 左岩前侧斜切碎台
    cubes.append(
        Cube(
            "chasm",
            "rock_left_step_front",
            (2.4, 0.0, 4.2),
            (4.8, 1.8, 8.2),
            front_uv="sandstone_cliff",
            side_uv="sandstone_dark",
            back_uv="sandstone_dark",
            top_uv="sandstone_slab",
            bottom_uv="sandstone_dark",
            rotation=(0.0, 22.0, -4.0),
            rot_origin=(3.6, 0.0, 6.2),
        )
    )

    # 3. 右侧夹峙红砂岩峭壁 (Right Chasm Rock Y: 0.6 ~ 3.4, X: 9.4 ~ 13.0)
    cubes.append(
        Cube(
            "chasm",
            "rock_right_cliff_main",
            (9.4, 0.6, 5.4),
            (13.0, 3.2, 11.0),
            front_uv="sandstone_cliff",
            side_uv="sandstone_dark",
            back_uv="sandstone_dark",
            top_uv="sandstone_slab",
            bottom_uv="sandstone_dark",
            rotation=(-4.0, -20.0, 8.0),
            rot_origin=(11.2, 0.6, 8.2),
        )
    )
    # 右岩后侧斜切碎台
    cubes.append(
        Cube(
            "chasm",
            "rock_right_step_back",
            (11.0, 0.0, 7.8),
            (13.6, 1.6, 12.2),
            front_uv="sandstone_cliff",
            side_uv="sandstone_dark",
            back_uv="sandstone_dark",
            top_uv="sandstone_slab",
            bottom_uv="sandstone_dark",
            rotation=(0.0, -12.0, 4.0),
            rot_origin=(12.3, 0.0, 10.0),
        )
    )

    # 4. 岩缝内深色阴影嵌块 (Chasm Fissure Shadow Bed)
    cubes.append(
        Cube(
            "chasm",
            "chasm_fissure_bed",
            (6.4, 0.4, 6.2),
            (9.6, 1.4, 9.8),
            front_uv="sandstone_dark",
            side_uv="sandstone_dark",
            back_uv="sandstone_dark",
            top_uv="sandstone_dark",
            bottom_uv="sandstone_dark",
        )
    )

    return cubes


def part_stalk_and_blood_drop() -> list[Cube]:
    """部件2：斜生暗红血质主茎与断口血滴 (part_stalk_and_blood_drop)。
    位于 Y: 0.8 ~ 10.2，自左下方岩缝向右上方挺拔斜生，并在基部断口凝聚滴落血髓。
    """
    cubes: list[Cube] = []
    STALK_ROT = (10.0, 0.0, -16.0)
    STALK_ORG = (7.2, 2.0, 7.6)

    # 1. 主茎秆下段 (自岩缝伸出 Y: 1.6 ~ 4.6)
    cubes.append(
        Cube(
            "stalk",
            "stalk_lower_segment",
            (6.8, 1.6, 7.2),
            (7.8, 4.6, 8.2),
            front_uv="stalk_dark_bark",
            side_uv="stalk_dark_bark",
            back_uv="stalk_dark_bark",
            top_uv="stalk_dark_bark",
            bottom_uv="marrow_core_pulse",
            rotation=STALK_ROT,
            rot_origin=STALK_ORG,
        )
    )
    # 2. 主茎秆中段 (分叉主中枢 Y: 4.4 ~ 7.4)
    cubes.append(
        Cube(
            "stalk",
            "stalk_mid_segment",
            (6.9, 4.4, 7.3),
            (7.7, 7.4, 8.1),
            front_uv="stalk_dark_bark",
            side_uv="stalk_dark_bark",
            back_uv="stalk_dark_bark",
            top_uv="stalk_dark_bark",
            bottom_uv="stalk_dark_bark",
            rotation=STALK_ROT,
            rot_origin=STALK_ORG,
        )
    )
    # 3. 主茎秆上梢 (承托顶叶 Y: 7.2 ~ 9.8)
    cubes.append(
        Cube(
            "stalk",
            "stalk_upper_segment",
            (7.0, 7.2, 7.4),
            (7.6, 9.8, 8.0),
            front_uv="stalk_dark_bark",
            side_uv="stalk_dark_bark",
            back_uv="stalk_dark_bark",
            top_uv="stalk_dark_bark",
            bottom_uv="stalk_dark_bark",
            rotation=STALK_ROT,
            rot_origin=STALK_ORG,
        )
    )

    # 4. 根部断口凝聚滴落血髓珠 (Dripping Blood Droplet Y: 0.2 ~ 1.8)
    cubes.append(
        Cube(
            "stalk",
            "blood_severed_node",
            (6.5, 1.0, 7.0),
            (7.7, 1.9, 8.2),
            front_uv="blood_drop_bright",
            side_uv="blood_drop_bright",
            back_uv="blood_drop_bright",
            top_uv="marrow_core_pulse",
            bottom_uv="blood_drop_bright",
            rotation=(0.0, 0.0, 0.0),
            rot_origin=(7.1, 1.4, 7.6),
        )
    )
    # 悬垂拉丝血滴尖 (Hanging Blood Drip)
    cubes.append(
        Cube(
            "stalk",
            "blood_drip_string",
            (6.9, 0.2, 7.4),
            (7.3, 1.1, 7.8),
            front_uv="blood_drop_bright",
            side_uv="blood_drop_bright",
            back_uv="blood_drop_bright",
            top_uv="blood_drop_bright",
            bottom_uv="blood_drop_bright",
        )
    )

    return cubes


def make_blade_leaf(
    blade_id: str,
    base_pos: tuple[float, float, float],
    length: float,
    width: float,
    yaw_pitch_roll: tuple[float, float, float],
) -> list[Cube]:
    """生成一片带有 3D 浮雕鲜红血髓脉的平滑连续玛瑙剑叶。
    - 叶身：上凸下拱的暗红黑玛瑙厚叶片 (厚度 ~0.5)；
    - 髓脉：嵌在正中轴线微凸起的发光鲜红经脉 (高出表面 0.12 格)。
    """
    cubes: list[Cube] = []
    bx, by, bz = base_pos
    rx, ry, rz = yaw_pitch_roll
    rot_org = (bx, by, bz)

    # 1. 叶柄短根 (Petiole connection Y: 0.0 ~ 1.0)
    cubes.append(
        Cube(
            blade_id,
            f"{blade_id}_petiole",
            (bx - 0.35, by, bz - 0.35),
            (bx + 0.35, by + 1.2, bz + 0.35),
            front_uv="stalk_dark_bark",
            side_uv="stalk_dark_bark",
            back_uv="stalk_dark_bark",
            top_uv="stalk_dark_bark",
            bottom_uv="stalk_dark_bark",
            rotation=(rx, ry, rz),
            rot_origin=rot_org,
        )
    )

    # 2. 叶身主板 (Leaf Blade Body - 卵圆剑形连续分段)
    # (1) 叶身下部 (膨大过渡)
    cubes.append(
        Cube(
            blade_id,
            f"{blade_id}_blade_lower",
            (bx - width * 0.42, by + 1.0, bz - 0.28),
            (bx + width * 0.42, by + length * 0.48, bz + 0.28),
            front_uv="agate_blade_main",
            side_uv="agate_blade_edge",
            back_uv="agate_blade_back",
            top_uv="agate_blade_edge",
            bottom_uv="agate_blade_edge",
            rotation=(rx, ry, rz),
            rot_origin=rot_org,
        )
    )
    # (2) 叶身中最宽处
    cubes.append(
        Cube(
            blade_id,
            f"{blade_id}_blade_mid",
            (bx - width * 0.52, by + length * 0.42, bz - 0.28),
            (bx + width * 0.52, by + length * 0.78, bz + 0.28),
            front_uv="agate_blade_main",
            side_uv="agate_blade_edge",
            back_uv="agate_blade_back",
            top_uv="agate_blade_edge",
            bottom_uv="agate_blade_edge",
            rotation=(rx, ry, rz),
            rot_origin=rot_org,
        )
    )
    # (3) 叶梢尖收束 (Tip Taper)
    cubes.append(
        Cube(
            blade_id,
            f"{blade_id}_blade_tip",
            (bx - width * 0.32, by + length * 0.72, bz - 0.25),
            (bx + width * 0.32, by + length, bz + 0.25),
            front_uv="agate_blade_tip",
            side_uv="agate_blade_edge",
            back_uv="agate_blade_back",
            top_uv="agate_blade_tip",
            bottom_uv="agate_blade_edge",
            rotation=(rx, ry, rz),
            rot_origin=rot_org,
        )
    )

    # 3. 3D 凸起鲜红搏动血髓脉 (3D Blood Marrow Line)
    # 贯穿叶片正面中线 (Z: bz - 0.38 ~ bz - 0.24)
    cubes.append(
        Cube(
            blade_id,
            f"{blade_id}_marrow_lower",
            (bx - 0.25, by + 1.0, bz - 0.38),
            (bx + 0.25, by + length * 0.58, bz - 0.22),
            front_uv="marrow_core_pulse",
            side_uv="marrow_side_red",
            back_uv="marrow_side_red",
            top_uv="marrow_core_pulse",
            bottom_uv="marrow_core_pulse",
            rotation=(rx, ry, rz),
            rot_origin=rot_org,
        )
    )
    cubes.append(
        Cube(
            blade_id,
            f"{blade_id}_marrow_upper",
            (bx - 0.20, by + length * 0.52, bz - 0.35),
            (bx + 0.20, by + length * 0.96, bz - 0.20),
            front_uv="marrow_core_pulse",
            side_uv="marrow_side_red",
            back_uv="marrow_side_red",
            top_uv="marrow_core_pulse",
            bottom_uv="marrow_core_pulse",
            rotation=(rx, ry, rz),
            rot_origin=rot_org,
        )
    )

    return cubes


def part_five_agate_blades() -> list[Cube]:
    """部件3：五瓣玛瑙剑叶簇 (5 Agate Blade Leaves)。
    按原画精确定位：
    1. 顶端挺拔主剑叶 (Top Crown Blade)
    2. 左上斜展剑叶 (Upper Left Blade)
    3. 右上斜展剑叶 (Upper Right Blade)
    4. 左下微垂剑叶 (Lower Left Blade)
    5. 右下横生剑叶 (Lower Right Blade)
    """
    cubes: list[Cube] = []

    # 1. 顶端主剑叶 (Top Blade - 挺立在主茎顶梢)
    # 基点: (7.3, 9.4, 7.7), 长 4.4, 宽 2.0, 倾角 (-14°, 4°, 8°)
    cubes.extend(
        make_blade_leaf(
            blade_id="blade_top",
            base_pos=(7.3, 9.4, 7.7),
            length=4.4,
            width=2.0,
            yaw_pitch_roll=(-14.0, 4.0, 8.0),
        )
    )

    # 2. 左上主剑叶 (Upper Left Blade)
    # 基点: (7.1, 7.4, 7.6), 向左上方舒展
    cubes.extend(
        make_blade_leaf(
            blade_id="blade_upper_left",
            base_pos=(7.1, 7.4, 7.6),
            length=4.2,
            width=2.1,
            yaw_pitch_roll=(-6.0, 24.0, -36.0),
        )
    )

    # 3. 右上主剑叶 (Upper Right Blade)
    # 基点: (7.4, 7.8, 7.7), 向右上方挺拔舒展
    cubes.extend(
        make_blade_leaf(
            blade_id="blade_upper_right",
            base_pos=(7.4, 7.8, 7.7),
            length=4.5,
            width=2.2,
            yaw_pitch_roll=(-10.0, -22.0, 32.0),
        )
    )

    # 4. 左下主剑叶 (Lower Left Blade)
    # 基点: (6.9, 5.0, 7.4), 向左前侧斜展
    cubes.extend(
        make_blade_leaf(
            blade_id="blade_lower_left",
            base_pos=(6.9, 5.0, 7.4),
            length=4.0,
            width=2.0,
            yaw_pitch_roll=(8.0, 32.0, -52.0),
        )
    )

    # 5. 右下主剑叶 (Lower Right Blade)
    # 基点: (7.3, 5.4, 7.5), 向右前侧横生展开
    cubes.extend(
        make_blade_leaf(
            blade_id="blade_lower_right",
            base_pos=(7.3, 5.4, 7.5),
            length=4.2,
            width=2.1,
            yaw_pitch_roll=(6.0, -28.0, 48.0),
        )
    )

    return cubes


def all_cubes() -> list[Cube]:
    return (
        part_sandstone_chasm()
        + part_stalk_and_blood_drop()
        + part_five_agate_blades()
    )


# ─────────────────────────────────────────────────────────────────────────────
# 2. 64x64 UV Atlas 高清贴图烘焙 (专业三层结构 + 手绘像素质感)
# ─────────────────────────────────────────────────────────────────────────────

UV_MAP = {
    # ── 粗粝风化红砂岩 ──
    "sandstone_slab":     [2.0, 2.0, 20.0, 18.0],
    "sandstone_cliff":    [20.0, 2.0, 38.0, 18.0],
    "sandstone_dark":     [38.0, 2.0, 48.0, 14.0],

    # ── 暗红木质血茎 ──
    "stalk_dark_bark":    [48.0, 2.0, 62.0, 16.0],

    # ── 半透明暗红黑玛瑙剑叶 ──
    "agate_blade_main":   [2.0, 18.0, 18.0, 38.0],
    "agate_blade_edge":   [18.0, 18.0, 28.0, 38.0],
    "agate_blade_back":   [28.0, 18.0, 40.0, 38.0],
    "agate_blade_tip":    [40.0, 18.0, 48.0, 30.0],

    # ── 搏动鲜红真元血髓 ──
    "marrow_core_pulse":  [48.0, 18.0, 56.0, 38.0],
    "marrow_side_red":    [56.0, 18.0, 62.0, 30.0],
    "blood_drop_bright":  [54.0, 30.0, 62.0, 42.0],
}


def create_texture() -> Image.Image:
    """烘焙 64x64 UV Atlas 贴图：
    遵循三层贴图架构：
    1. 大尺度明暗：红砂岩顶面暖光、侧面横向岩层沉降、玛瑙剑叶顺叶身纵向深浅渐变；
    2. 材质特征：
       - 红砂岩 (0x823824 ~ 0x481E14)：粗粝砂粒层理、微小石英沙砾反光；
       - 暗红黑玛瑙叶 (0x441016 ~ 0x1E060A)：黑曜宝石底色、叶缘暗红透光光泽；
       - 鲜红真元血髓 (0xD81628 ~ 0xFF3E50)：高饱和光纤与发光核心晶点；
    3. 微小细节：玛瑙内部絮状血纹、断口血滴高光与阴影凹槽。
    """
    img = Image.new("RGBA", (TEXTURE_RES, TEXTURE_RES), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # ─────────────────────────────────────────────────────────────
    # 1. 红砂岩基盘与岩壁 (0,0)~(48,18)
    # ─────────────────────────────────────────────────────────────
    for x in range(48):
        for y in range(18):
            # 砂岩横向层理方程
            layer = math.sin(y * 0.65 + x * 0.15) * 3
            noise = ((x * 17 + y * 23) % 7) - 3
            # 基础红砂岩 0x7E3626 (126, 54, 38)
            base_r = int(np.clip(122 + layer * 2.5 + noise * 1.5, 75, 165))
            base_g = int(np.clip(54 + layer * 1.2 + noise * 1.0, 30, 92))
            base_b = int(np.clip(38 + layer * 1.0 + noise * 0.8, 20, 72))

            # 顶面暖光提亮 (y < 4 且 x < 20)
            if y < 4 and x < 20:
                base_r += 24
                base_g += 14
                base_b += 10

            # 石英矿晶砂粒
            if (x * 11 + y * 19) % 23 == 0:
                base_r += 28
                base_g += 20
                base_b += 18

            img.putpixel((x, y), (base_r, base_g, base_b, 255))

    # 砂岩岩壁层理裂纹 (20,2)~(38,18)
    draw.line([(22, 7), (36, 9)], fill=(68, 28, 20, 255), width=1)
    draw.line([(24, 13), (35, 14)], fill=(62, 24, 18, 255), width=1)

    # 岩缝暗阴影区 (38,2)~(48,14)
    for x in range(38, 48):
        for y in range(2, 14):
            noise = ((x * 13 + y * 17) % 5) - 2
            img.putpixel((x, y), (56 + noise, 22 + noise, 16 + noise, 255))

    # ─────────────────────────────────────────────────────────────
    # 2. 暗红木质血茎区 (48,2)~(62,16)
    # ─────────────────────────────────────────────────────────────
    for x in range(48, 62):
        for y in range(2, 16):
            noise = ((x * 19 + y * 29) % 7) - 3
            # 纵向木质纤维纹理
            is_fiber = (x % 3 == 0)
            base_r = 82 if not is_fiber else 102
            base_g = 22 if not is_fiber else 28
            base_b = 30 if not is_fiber else 38
            r = int(np.clip(base_r + noise * 1.5, 55, 130))
            g = int(np.clip(base_g + noise * 0.8, 12, 45))
            b = int(np.clip(base_b + noise * 1.0, 18, 55))
            img.putpixel((x, y), (r, g, b, 255))

    # ─────────────────────────────────────────────────────────────
    # 3. 玛瑙剑叶区 (0,18)~(48,38)
    # ─────────────────────────────────────────────────────────────
    for x in range(48):
        for y in range(18, 38):
            v = y - 18
            # 顺叶身纵向渐变：基部深暗褐红 (0x300A10) -> 中部深玛瑙红 (0x54121A) -> 尖梢明红 (0x7C1824)
            grad = (20 - v) / 20.0
            noise = ((x * 13 + y * 29) % 7) - 3

            r = int(np.clip(46 + (1.0 - grad) * 58 + noise, 32, 135))
            g = int(np.clip(10 + (1.0 - grad) * 16 + noise * 0.5, 6, 38))
            b = int(np.clip(14 + (1.0 - grad) * 22 + noise * 0.6, 8, 48))

            # 边缘半透明微透光泽
            if x in range(18, 28) or x in range(40, 48):
                r = int(r * 1.30)
                g = int(g * 1.18)
                b = int(b * 1.25)

            img.putpixel((x, y), (r, g, b, 255))

    # 玛瑙宝石内部天然絮状血色云纹
    draw.line([(6, 22), (12, 34)], fill=(110, 24, 34, 255), width=1)
    draw.line([(32, 24), (36, 32)], fill=(98, 20, 28, 255), width=1)

    # ─────────────────────────────────────────────────────────────
    # 4. 搏动鲜红真元血髓脉 (48,18)~(64,44)
    # ─────────────────────────────────────────────────────────────
    for x in range(48, 64):
        for y in range(18, 44):
            u = x - 48
            noise = ((x * 19 + y * 31) % 7) - 3
            if u in range(0, 8):
                # 主血脉：高饱和鲜血红 0xD81628 ~ 0xFF3E50
                r = int(np.clip(228 + noise * 3.5, 185, 255))
                g = int(np.clip(26 + noise * 1.2, 12, 65))
                b = int(np.clip(38 + noise * 1.5, 18, 85))
                # 脉芯发光核心 (高光晶亮)
                if u in (3, 4):
                    r = 255
                    g = int(np.clip(72 + noise * 2, 48, 115))
                    b = int(np.clip(82 + noise * 2, 58, 130))
            elif u in range(8, 14):
                # 侧边分叉微脉
                r = int(np.clip(185 + noise * 3.0, 145, 235))
                g = int(np.clip(20 + noise * 1.0, 10, 50))
                b = int(np.clip(32 + noise * 1.2, 14, 65))
            else:
                # 血管侧边暗红
                r, g, b = 135, 18, 28

            img.putpixel((x, y), (r, g, b, 255))

    # 滴落血髓珠高光核心 (54,30)~(62,42)
    for x in range(54, 62):
        for y in range(30, 42):
            dx = abs(x - 58)
            dy = abs(y - 36)
            d = math.sqrt(dx * dx + dy * dy)
            if d < 1.6:
                img.putpixel((x, y), (255, 145, 160, 255))
            elif d < 3.4:
                img.putpixel((x, y), (238, 28, 46, 255))
            else:
                img.putpixel((x, y), (145, 18, 26, 255))

    return img


# ─────────────────────────────────────────────────────────────────────────────
# 3. BBModel 文件生成
# ─────────────────────────────────────────────────────────────────────────────

def build_bbmodel() -> dict:
    cubes = all_cubes()
    tex_img = create_texture()
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    outlines = {}

    for idx, c in enumerate(cubes):
        elem_uuid = stable_uuid("element", str(idx), c.name)

        front_box = UV_MAP.get(c.front_uv, [0, 0, 16, 16])
        side_box = UV_MAP.get(c.side_uv, front_box)
        back_box = UV_MAP.get(c.back_uv, front_box)
        top_box = UV_MAP.get(c.top_uv, side_box)
        bot_box = UV_MAP.get(c.bottom_uv, side_box)

        faces = {
            "north": {"uv": front_box, "texture": 0},
            "south": {"uv": back_box, "texture": 0},
            "east":  {"uv": side_box, "texture": 0},
            "west":  {"uv": side_box, "texture": 0},
            "up":    {"uv": top_box, "texture": 0},
            "down":  {"uv": bot_box, "texture": 0},
        }

        elem = {
            "name": c.name,
            "box_uv": False,
            "type": "cube",
            "uuid": elem_uuid,
            "from": [round(float(x), 4) for x in c.origin],
            "to": [round(float(x), 4) for x in c.target],
            "faces": faces,
        }

        if any(abs(r) > 1e-4 for r in c.rotation):
            elem["rotation"] = [round(float(r), 4) for r in c.rotation]
            elem["origin"] = [round(float(o), 4) for o in c.rot_origin]

        elements.append(elem)

        if c.bone not in outlines:
            outlines[c.bone] = []
        outlines[c.bone].append(elem_uuid)

    outliner = []
    for bone_name, children_uuids in outlines.items():
        outliner.append(
            {
                "name": bone_name,
                "origin": [8.0, 0.0, 8.0],
                "color": 0,
                "uuid": stable_uuid("group", bone_name),
                "export": True,
                "isOpen": True,
                "locked": False,
                "visibility": True,
                "autouv": 0,
                "children": children_uuids,
            }
        )

    model_dict = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "ChiSuiCao",
        "model_identifier": "chi_sui_cao",
        "visible_box": [1, 1, 0],
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "chi_sui_cao.png",
                "name": "chi_sui_cao",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": stable_uuid("texture", "chi_sui_cao"),
                "source": tex_b64,
            }
        ],
    }
    return model_dict


def main():
    LOCAL_MODELS.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)

    bbmodel = build_bbmodel()
    model_path = LOCAL_MODELS / "ChiSuiCao.bbmodel"
    model_path.write_text(json.dumps(bbmodel, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✅ Generated BBModel: {model_path.relative_to(REPO)}")

    tex = create_texture()
    tex_path = PREVIEW_DIR / "chi_sui_cao_texture_r3.png"
    tex.save(tex_path)
    print(f"✅ Saved Texture: {tex_path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
