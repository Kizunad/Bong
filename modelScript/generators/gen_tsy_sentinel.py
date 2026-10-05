#!/usr/bin/env python3
"""秘境守灵 (tsy_sentinel) Blockbench .bbmodel 程序化生成器（逐部件重做版）。

按 `.task-creature-stepwise.md` 的流程一次只做一个部件，每个部件做完出单件图、对照卡和累计图，
等调度回复后才做下一个。旧的五大部件整件生成器（光滑方盒）已整体作废，见 git 历史。

部件（01~07 已全部通过，用户 2026-10-05 在审阅页终审通过）：
  01_core_frame 铁木内骨 / 02_head 石面具头 / 03_chest_plate 石胸甲 / 04_pauldrons 石肩甲 /
  05_arms_fists 双臂与石拳 / 06_belt_tabard 腰带与破布前襟 / 07_legs_feet 石桩腿与石座脚。
  输出 ModelScript/models/TsySentinelV2.bbmodel：身体中轴平移到原点、面朝 +Z（v2 约定）。

约定：
  - 建模源面朝 +Z（v2 生物约定，导出时再统一转成游戏里的 -Z，见 creatures/fauna_v2/gen_rig.py）。
  - 单位是 bbmodel 像素，身体中线在 x=8、z=8，脚底 y=0。
  - 比例取自 parts_ref 参考图：约 30 个参考图像素 = 1 个模型单位。

配色表（调度计划原文，只用这些色）：
  风化石 #b8b2a4 / #9c968a / #7a756c；苔斑 #6f7a4a；锈铜关节 #8a5a34 / #6b4a2e；
  深影 #2e2a26；旧布 #a8906e / #7a634a。
  铁木肢体用深影 #2e2a26 与暗锈铜 #6b4a2e 之间的一档过渡做斑驳，亮边用 #7a756c 点缀。
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
from typing import Callable, Dict, List

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from bbmodel_maker.gates.coplanar import check_coplanar_faces  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "TsySentinelV2.bbmodel"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/tsy_sentinel")
PARTS_DIR = REVIEW_DIR / "parts"
PARTS_REF_DIR = REVIEW_DIR / "parts_ref"

RES = 64
CENTER_X = 8.0
CENTER_Z = 8.0

# ── 配色表 ──
PALETTE = {
    "stone_light": [184, 178, 164],   # #b8b2a4
    "stone_mid": [156, 150, 138],     # #9c968a
    "stone_dark": [122, 117, 108],    # #7a756c
    "moss": [111, 122, 74],           # #6f7a4a
    "copper_lit": [138, 90, 52],      # #8a5a34
    "copper_dark": [107, 74, 46],     # #6b4a2e
    "shadow": [46, 42, 38],           # #2e2a26
    "cloth_light": [168, 144, 110],   # #a8906e
    "cloth_dark": [122, 99, 74],      # #7a634a
}

# 贴图 64x64 切成 4x4 个 16x16 色块，每种材质一块，面上叠斑驳。
# frame_wood：铁木肢体；frame_wood_lit：肢体亮边与节带。
MATERIALS = [
    "frame_wood", "frame_wood_lit", "copper_lit", "copper_dark",
    "shadow", "stone_dark", "stone_mid", "moss", "stone_light",
    "cloth_light", "cloth_dark",   # 06 起的旧布，追加在末尾，不改动已通过部件的贴图色块
]
MAT_UV = {
    name: [(i % 4) * 16, (i // 4) * 16, (i % 4) * 16 + 16, (i // 4) * 16 + 16]
    for i, name in enumerate(MATERIALS)
}

Cube = Dict[str, object]


def _box(name: str, group: str, material: str, lo: List[float], hi: List[float]) -> Cube:
    return {"name": name, "group": group, "material": material, "from": list(lo), "to": list(hi)}


def _mirror(cubes: List[Cube]) -> List[Cube]:
    """把 +x 一侧的立方体镜像到 -x 一侧，名字里的 `_r` 换成 `_l`。"""
    out = []
    for c in cubes:
        lo, hi = c["from"], c["to"]
        out.append({
            **c,
            "name": str(c["name"]).replace("_r", "_l", 1),
            "from": [2 * CENTER_X - hi[0], lo[1], lo[2]],
            "to": [2 * CENTER_X - lo[0], hi[1], hi[2]],
        })
    return out


# ───────────────────────── 圆形锈铜环关节 ─────────────────────────

def _ring_on_face(
    name: str, group: str, axis: str, sign: int, center: List[float], face: float,
    outer: float, hole: float, thick: float,
) -> List[Cube]:
    """在核心方块的一个面上贴一圈八角形锈铜环，环心嵌一块深影洞底。

    axis 是环的法线（"z" 或 "x"），sign 是朝向（+1 / -1），face 是该面坐标，
    center 是关节中心 (x, y, z)。环由上下两根横条 + 左右两根竖条组成，
    竖条比横条薄 0.15，且根部嵌进核心 0.06，两端面都与横条错开，避免同向面共面闪烁；
    四角缺角形成八角轮廓。
    """
    half = outer / 2
    inner = hole / 2
    chamfer = outer * 0.2
    lo_n, hi_n = (face, face + thick) if sign > 0 else (face - thick, face)
    embed = 0.06
    if sign > 0:
        side_lo, side_hi = face - embed, face + thick - 0.15
    else:
        side_lo, side_hi = face - thick + 0.15, face + embed

    def place(u0: float, u1: float, v0: float, v1: float, near: tuple, mat: str, tag: str) -> Cube:
        if axis == "z":
            lo = [center[0] + u0, center[1] + v0, near[0]]
            hi = [center[0] + u1, center[1] + v1, near[1]]
        else:
            lo = [near[0], center[1] + v0, center[2] + u0]
            hi = [near[1], center[1] + v1, center[2] + u1]
        return _box(f"{name}_{tag}", group, mat, lo, hi)

    cubes = [
        place(-(half - chamfer), half - chamfer, inner, half, (lo_n, hi_n), "copper_lit", "top"),
        place(-(half - chamfer), half - chamfer, -half, -inner, (lo_n, hi_n), "copper_lit", "bot"),
        place(inner, half, -(half - chamfer), half - chamfer, (side_lo, side_hi), "copper_lit", "rt"),
        place(-half, -inner, -(half - chamfer), half - chamfer, (side_lo, side_hi), "copper_lit", "lf"),
    ]
    plate_lo, plate_hi = (face, face + 0.1) if sign > 0 else (face - 0.1, face)
    cubes.append(place(-inner, inner, -inner, inner, (plate_lo, plate_hi), "shadow", "hole"))
    return cubes


def _joint_hub(
    name: str, group: str, center: List[float], diameter: float,
    faces: str = "zx", outer_sign: int = 1,
) -> List[Cube]:
    """圆形锈铜环关节：一个暗锈铜核心方块，前后（z）和外侧（x）面各贴一圈环。

    faces 里的字母表示哪些面有环：z 是前后两面，x 是 outer_sign 指向的外侧面。
    """
    core = diameter * 0.78
    half_core = core / 2
    thick = diameter * 0.12
    hole = diameter * 0.26
    cx, cy, cz = center
    cubes = [_box(
        f"{name}_core", group, "copper_dark",
        [cx - half_core, cy - half_core, cz - half_core],
        [cx + half_core, cy + half_core, cz + half_core],
    )]
    if "z" in faces:
        for sign, tag in ((1, "f"), (-1, "b")):
            cubes += _ring_on_face(
                f"{name}_ring_{tag}", group, "z", sign, center, cz + sign * half_core,
                diameter, hole, thick,
            )
    if "x" in faces:
        cubes += _ring_on_face(
            f"{name}_ring_o", group, "x", outer_sign, center, cx + outer_sign * half_core,
            diameter - 0.2, hole, thick,  # 小 0.2：与前后环的边角错开，避免共面
        )
    return cubes


# ───────────────────────── 部件 01：铁木内骨 ─────────────────────────

def part_core_frame() -> List[Cube]:
    """01_core_frame：躯干芯（颅芯、脊柱、锁骨梁、肋骨、骨盆）+ 四肢芯 + 八个环关节。

    右半边（+x）写一遍再镜像。中线 x=8、z=8，脚底 y=0。尺寸来自 parts_ref/01_core_frame.png。
    """
    g = "core_frame"
    cx, cz = CENTER_X, CENTER_Z
    cubes: List[Cube] = []

    # —— 颅芯与脖颈：小方块颅芯，侧面一圈小环，顶上收窄一级 ——
    cubes.append(_box("core_skull", g, "frame_wood", [cx - 1.3, 28.3, cz - 1.4], [cx + 1.3, 30.8, cz + 1.4]))
    cubes.append(_box("core_skull_crown", g, "frame_wood_lit", [cx - 0.9, 30.8, cz - 1.0], [cx + 0.9, 31.4, cz + 1.0]))
    for sign, tag in ((1, "r"), (-1, "l")):
        cubes += _ring_on_face(
            f"core_skull_ring_{tag}", g, "x", sign, [cx, 29.5, cz], cx + sign * 1.3,
            1.4, 0.45, 0.25,
        )
    cubes.append(_box("core_neck", g, "frame_wood", [cx - 0.8, 27.4, cz - 0.8], [cx + 0.8, 28.3, cz + 0.8]))

    # —— 脊柱：背侧一根竖直脊柱，交替宽窄的椎节，从颈根一直通到骨盆 ——
    spine_edges = [17.4, 18.5, 19.6, 20.7, 21.8, 22.9, 24.0, 25.1, 26.2, 27.4]
    for i in range(len(spine_edges) - 1):
        wide = 0.95 if i % 2 == 0 else 0.75
        cubes.append(_box(
            f"core_spine_{i}", g, "frame_wood_lit" if i % 2 == 0 else "frame_wood",
            [cx - wide, spine_edges[i], cz - 1.8], [cx + wide, spine_edges[i + 1] - 0.05, cz - 0.4],
        ))

    # —— 锁骨梁：一根横梁连接左右肩关节 ——
    cubes.append(_box("core_clavicle", g, "frame_wood_lit", [cx - 3.6, 26.1, cz - 1.0], [cx + 3.6, 27.5, cz + 1.0]))

    # —— 胸骨：胸前一块竖直板，比肋骨条略凸 ——
    cubes.append(_box("core_sternum", g, "frame_wood_lit", [cx - 0.85, 22.4, cz + 1.8], [cx + 0.85, 26.1, cz + 2.4]))

    # —— 肋骨：四根横条，上宽下窄，前横条 + 两侧条接到背脊，条间留缝透出暗腔 ——
    rib_tops = [25.9, 24.9, 23.9, 22.9]
    rib_half_widths = [3.4, 3.2, 2.9, 2.6]
    for i, (top, half_w) in enumerate(zip(rib_tops, rib_half_widths)):
        low = top - 0.65
        cubes.append(_box(f"core_rib_front_{i}", g, "frame_wood", [cx - half_w, low, cz + 1.4], [cx + half_w, top, cz + 2.1]))
        for sign, tag in ((1, "r"), (-1, "l")):
            x_in, x_out = sorted((cx + sign * (half_w - 0.65), cx + sign * half_w))
            cubes.append(_box(
                f"core_rib_side_{tag}_{i}", g, "frame_wood",
                [x_in, low, cz - 1.8], [x_out, top, cz + 1.4],
            ))
    # 肋骨腔后壁：深影一块，让条间缝里看到暗色而不是穿透
    cubes.append(_box("core_chest_hollow", g, "shadow", [cx - 2.4, 22.35, cz - 0.4], [cx + 2.4, 25.85, cz - 0.1]))

    # —— 骨盆：横杆连两个髋环，下面一块骨盆块 ——
    cubes.append(_box("core_pelvis_bar", g, "frame_wood_lit", [cx - 1.5, 16.2, cz - 1.2], [cx + 1.5, 17.4, cz + 1.2]))
    cubes.append(_box("core_pelvis_block", g, "frame_wood", [cx - 1.0, 14.7, cz - 1.0], [cx + 1.0, 16.2, cz + 1.0]))

    # —— 右半边关节与肢体（+x），随后镜像 ——
    right: List[Cube] = []
    # 肩环：直径 4.0，在锁骨梁外端
    right += _joint_hub("core_shoulder_r", g, [cx + 5.1, 26.5, cz], 4.0)
    # 上臂：略向外，下端一道亮节带
    right.append(_box("core_upperarm_r", g, "frame_wood", [cx + 5.2, 21.6, cz - 1.2], [cx + 7.6, 24.9, cz + 1.2]))
    right.append(_box("core_upperarm_strip_r", g, "frame_wood_lit", [cx + 5.3, 21.9, cz + 1.2], [cx + 5.9, 24.5, cz + 1.4]))
    right.append(_box("core_upperarm_band_r", g, "frame_wood_lit", [cx + 5.0, 21.5, cz - 1.45], [cx + 7.8, 22.3, cz + 1.45]))
    # 肘环：直径 2.8
    right += _joint_hub("core_elbow_r", g, [cx + 7.0, 20.3, cz], 2.8)
    # 前臂 + 腕部锈铜节带 + 拳芯
    right.append(_box("core_forearm_r", g, "frame_wood", [cx + 6.3, 15.5, cz - 1.15], [cx + 8.6, 19.1, cz + 1.15]))
    right.append(_box("core_forearm_strip_r", g, "frame_wood_lit", [cx + 6.4, 16.0, cz + 1.15], [cx + 7.0, 18.8, cz + 1.35]))
    right.append(_box("core_wrist_band_r", g, "copper_dark", [cx + 6.1, 14.7, cz - 1.5], [cx + 8.8, 15.5, cz + 1.5]))
    right.append(_box("core_fist_r", g, "frame_wood", [cx + 6.4, 12.5, cz - 1.5], [cx + 9.4, 14.7, cz + 1.5]))
    # 髋环：直径 3.3
    right += _joint_hub("core_hip_r", g, [cx + 2.8, 16.5, cz], 3.3)
    # 大腿 + 节带
    right.append(_box("core_thigh_r", g, "frame_wood", [cx + 1.8, 11.2, cz - 1.1], [cx + 4.0, 14.95, cz + 1.1]))
    right.append(_box("core_thigh_strip_r", g, "frame_wood_lit", [cx + 1.9, 11.7, cz + 1.1], [cx + 2.5, 14.5, cz + 1.3]))
    right.append(_box("core_thigh_band_r", g, "frame_wood_lit", [cx + 1.6, 11.1, cz - 1.35], [cx + 4.2, 11.9, cz + 1.35]))
    # 膝环：直径 3.3，略向外
    right += _joint_hub("core_knee_r", g, [cx + 3.5, 9.7, cz], 3.3)
    # 小腿 + 踝部锈铜节带 + 脚芯（前伸一点，脚底 y=0）
    right.append(_box("core_shin_r", g, "frame_wood", [cx + 2.7, 3.9, cz - 1.1], [cx + 4.9, 8.35, cz + 1.1]))
    right.append(_box("core_shin_strip_r", g, "frame_wood_lit", [cx + 2.8, 4.3, cz + 1.1], [cx + 3.4, 8.0, cz + 1.3]))
    right.append(_box("core_ankle_band_r", g, "copper_dark", [cx + 2.5, 3.0, cz - 1.35], [cx + 5.1, 3.9, cz + 1.35]))
    right.append(_box("core_foot_r", g, "frame_wood", [cx + 2.5, 0.0, cz - 1.4], [cx + 5.3, 3.0, cz + 2.0]))

    cubes += right + _mirror(right)
    return cubes


# ───────────────────────── 部件 02：石面具头 ─────────────────────────

def part_head() -> List[Cube]:
    """02_head：小方石面具头，坐在 01 的颈上。

    比例与五官位置按 parts_ref/02_head.png 的正面量取：头外包络 3.6 宽 x 3.8 高 x 约 4.0 深，
    参考图的 1 个像素约 0.0064 个模型单位（宽）/ 0.0061（高）。五官自上而下：
    头顶竖脊（锈铜）→ 粗眉梁 → 窄深眼缝（上缘一条锈铜眼睑）→ 竖直鼻梁 → 颧块 → 嘴缝 → 平下巴；
    两侧耳位各一个锈铜环。石面不做成整块：穹顶分台阶、左右半边高度和后缘不一样，
    再加几块突出的崩缺石片和薄苔斑，避免光滑盒子。
    """
    g = "head"
    cx, cz = CENTER_X, CENTER_Z

    def px_x(px: float) -> float:
        return cx + (px - 470) / 560 * 3.6

    def px_y(py: float) -> float:
        return 31.6 - (py - 150) / 620 * 3.8

    def slab(name: str, mat: str, x0: float, x1: float, y0: float, y1: float, z0: float, z1: float) -> Cube:
        return _box(f"head_{name}", g, mat, [x0, y0, z0], [x1, y1, z1])

    cubes: List[Cube] = []

    # —— 颅壳：台阶穹顶，左右半边高度和后缘不一，造崩缺 ——
    cubes += [
        slab("dome_cap", "stone_light", 7.1, 8.9, 31.25, 31.6, 6.9, 9.1),
        slab("dome_l", "stone_mid", 6.6, 8.0, 30.55, 31.25, 6.5, 9.4),
        slab("dome_r", "stone_mid", 8.0, 9.4, 30.45, 31.15, 6.6, 9.35),
        slab("skull_l", "stone_mid", 6.2, 8.0, 29.2, 30.55, 6.2, 9.2),
        slab("skull_r", "stone_dark", 8.0, 9.8, 29.2, 30.45, 6.3, 9.2),
        slab("nape", "stone_dark", 6.6, 9.4, 28.2, 29.2, 6.6, 9.0),
        # 崩缺石片：凸出穹顶的碎块
        slab("chip_a", "stone_light", 6.55, 6.95, 30.5, 30.85, 7.0, 7.7),
        slab("chip_b", "stone_mid", 9.0, 9.45, 30.35, 30.7, 7.3, 8.1),
    ]

    # —— 头顶竖脊：锈铜一条，前额向上、过顶、背面向下 ——
    cubes += [
        slab("crest_front", "copper_lit", px_x(440), px_x(490), 30.0, 31.55, 9.25, 9.75),
        slab("crest_top", "copper_lit", px_x(440), px_x(490), 31.6, 31.72, 6.7, 9.2),
        slab("crest_back", "copper_dark", px_x(440), px_x(490), 29.6, 31.6, 6.08, 6.2),
    ]

    # —— 前额板：左右各一块，厚度不同 ——
    cubes += [
        slab("forehead_l", "stone_light", 6.59, px_x(430), 30.25, 31.35, 9.2, 9.6),
        slab("forehead_r", "stone_mid", px_x(500), 9.41, 30.3, 31.3, 9.2, 9.55),
    ]

    # —— 面部凹腔：条块之间露出的暗腔，侧下方露出锈铜底色 ——
    cubes += [
        slab("face_cavity", "shadow", 6.9, 9.1, 29.1, 29.7, 9.0, 9.4),
        slab("face_base", "copper_dark", 6.7, 9.3, 28.0, 29.1, 8.9, 9.25),
        slab("jaw_side_l", "copper_dark", 6.2, 6.78, 27.95, 29.15, 8.2, 9.3),
        slab("jaw_side_r", "copper_dark", 9.22, 9.8, 27.95, 29.15, 8.2, 9.3),
    ]
    # 颚侧石块压在锈铜底上，外缘比底更凸
    cubes += [
        slab("jaw_stone_l", "stone_dark", 6.3, 6.8, 28.25, 29.0, 8.6, 9.5),
        slab("jaw_stone_r", "stone_mid", 9.2, 9.7, 28.3, 29.05, 8.5, 9.45),
    ]

    # —— 粗眉梁：全脸最宽、最凸的一条 ——
    cubes.append(slab("brow", "stone_light", px_x(280), px_x(690), 29.7, 30.22, 9.35, 10.05))
    # 眼缝：暗腔中的两条锈铜眼睑，眼缝本身是眉梁与颧块之间的暗隙
    cubes += [
        slab("lid_l", "copper_lit", px_x(300), px_x(410), 29.36, 29.48, 9.5, 9.6),
        slab("lid_r", "copper_lit", px_x(530), px_x(640), 29.36, 29.48, 9.5, 9.6),
    ]

    # —— 鼻梁 + 颧块 + 嘴缝 + 平下巴 ——
    cubes += [
        slab("nose", "stone_light", px_x(425), px_x(500), 28.84, 29.75, 9.3, 10.2),
        slab("cheek_l", "stone_mid", px_x(280), px_x(400), 28.35, 29.2, 9.3, 9.8),
        slab("cheek_r", "stone_light", px_x(520), px_x(650), 28.35, 29.2, 9.3, 9.7),
        slab("mouth_plate", "stone_dark", 7.1, 8.9, 28.2, 28.84, 9.2, 9.55),
        slab("mouth_slit", "shadow", 7.6, 8.4, 28.45, 28.55, 9.55, 9.6),
        slab("chin", "stone_light", px_x(385), px_x(530), 27.8, 28.29, 9.3, 10.0),
    ]

    # —— 耳位锈铜环 ——
    for sign, tag in ((1, "r"), (-1, "l")):
        cubes += _ring_on_face(
            f"head_ear_ring_{tag}", g, "x", sign, [cx, 29.2, cz + 0.5], cx + sign * 1.8,
            1.2, 0.4, 0.2,
        )

    # —— 苔斑：贴在石面上的薄片 ——
    cubes += [
        slab("moss_dome", "moss", 6.7, 7.05, 31.25, 31.3, 7.0, 8.0),
        slab("moss_cheek", "moss", px_x(285), px_x(340), 28.35, 28.75, 9.8, 9.84),
        slab("moss_back", "moss", 8.2, 9.2, 29.2, 30.4, 6.15, 6.19),
        slab("moss_brow", "moss", px_x(540), px_x(660), 30.22, 30.27, 9.4, 9.9),
        slab("moss_forehead", "moss", px_x(270), px_x(340), 31.0, 31.4, 9.6, 9.64),
        slab("moss_side_r", "moss", 9.8, 9.84, 29.9, 30.3, 7.2, 8.3),
        slab("moss_side_l", "moss", 6.16, 6.2, 29.7, 30.2, 6.8, 7.8),
    ]
    return cubes


# ───────────────────────── 部件 03：石胸甲 ─────────────────────────

# 胸甲的轮廓：自上而下的几行，每行 (下沿 y, 上沿 y, 半宽)。上窄中宽、再收成一块小腹舌，
# 上面两行比肩环内缘（|x| = 3.55）窄，肩环才露得出来。
CHEST_ROWS = [
    (24.4, 26.4, 3.0),
    (21.8, 24.4, 4.6),
    (19.4, 21.8, 4.2),
    (17.6, 19.4, 3.4),
    (16.2, 17.6, 2.5),
    (14.7, 16.2, 1.7),
]
CHEST_BASE_FRONT = 2.9       # 底板前表面到身体中线 cz 的距离；胸骨最前在 cz+2.4，底板包在它外面
CHEST_RELIEFS = [0.1, 0.3, 0.5, 0.7]   # 石块相对底板凸出的可选厚度，决定前表面的凹凸
DISC_CENTER_Y = 22.4
DISC_FRONT = 3.4             # 圆盘底面到 cz 的距离，高于大部分石块（BASE_FRONT + 0.5），圆盘压在石块上
DISC_CELL = 0.25             # 圆盘体素格边长
DISC_THICK = 0.3
DISC_RADIUS = 2.45
# 圆盘从中心向外的同心带：(半径比例上限, 材质)。中心暗点 → 亮石 → 亮铜环 → 石 → 暗铜环 → 亮石边
DISC_BANDS = [
    (0.20, "shadow"),
    (0.50, "stone_light"),
    (0.64, "copper_lit"),
    (0.84, "stone_mid"),
    (0.97, "copper_dark"),
    (1.12, "stone_light"),
]


def part_chest_plate() -> List[Cube]:
    """03_chest_plate：覆盖躯干正面的大石胸甲，正中同心圆阵纹圆盘，带苔斑。

    结构（按 parts_ref/03_chest_plate.png）：
    1. 锈铜底板：CHEST_ROWS 勾出的轮廓，石块之间的缝里露出它的锈褐色，对应参考图里石块间的褐色缝；
    2. 石块拼面：每一行随机切成一或两层子行，再切成宽 0.9~1.5 的碎块，每块厚薄不同
       （凸出底板 0.1~0.7），风化石三档加少量整块苔斑（苔斑就是 moss 材质的整块石块），所以前表面是凹凸的，不是光滑一整板；
    3. 阵纹圆盘：体素圆盘，按半径分带，中心暗点、亮石、亮铜环、石、暗铜环、亮石边，同心；
    4. 侧翼石墙：每行两端各一块向后包到肋骨侧面，给胸甲以厚度；
    5. 领口崩石：脖子两侧各两块高低不一的石块，向上冒出胸甲上缘。
    石块位置由固定种子生成，重跑结果一致。
    """
    g = "chest"
    cx, cz = CENTER_X, CENTER_Z
    rng = np.random.default_rng(2026_1005_03)
    cubes: List[Cube] = []

    # —— 底板：逐行的锈铜板 ——
    for index, (y0, y1, half) in enumerate(CHEST_ROWS):
        cubes.append(_box(
            f"chest_base_{index}", g, "copper_dark",
            [cx - half, y0, cz + 2.45], [cx + half, y1, cz + CHEST_BASE_FRONT],
        ))

    # —— 石块拼面 ——
    stone_choices = ["stone_light", "stone_mid", "stone_dark", "moss"]
    stone_weights = [0.42, 0.33, 0.20, 0.05]
    gap = 0.04
    for row_index, (y0, y1, half) in enumerate(CHEST_ROWS):
        # 一半的行再按高度劈成上下两层子行，错开竖缝，拼面更像乱石
        if y1 - y0 > 1.5 and rng.random() < 0.6:
            split = y0 + (y1 - y0) * float(rng.uniform(0.4, 0.6))
            sub_rows = [(y0, split), (split, y1)]
        else:
            sub_rows = [(y0, y1)]
        for sub_index, (sub_y0, sub_y1) in enumerate(sub_rows):
            x = -half
            cell = 0
            while x < half - 0.3:
                width = float(rng.uniform(0.9, 1.5))
                x_end = min(x + width, half)
                if half - x_end < 0.5:
                    x_end = half
                relief = float(rng.choice(CHEST_RELIEFS))
                material = str(rng.choice(stone_choices, p=stone_weights))
                cubes.append(_box(
                    f"chest_stone_{row_index}_{sub_index}_{cell}", g, material,
                    [cx + x + gap, sub_y0 + gap, cz + CHEST_BASE_FRONT],
                    [cx + x_end - gap, sub_y1 - gap, cz + CHEST_BASE_FRONT + relief],
                ))
                x = x_end
                cell += 1

    # —— 侧翼石墙：每行两端向后包到肋骨侧面（肋骨半宽最大 3.4，墙内缘取 3.5 以外） ——
    for row_index, (y0, y1, half) in enumerate(CHEST_ROWS[:5]):
        inner = max(3.5, half - 1.0)
        for sign, tag in ((1, "r"), (-1, "l")):
            x0, x1 = sorted((cx + sign * inner, cx + sign * half))
            if x1 - x0 < 0.3:
                continue
            material = str(rng.choice(["stone_dark", "stone_mid"]))
            cubes.append(_box(
                f"chest_wing_{tag}_{row_index}", g, material,
                [x0, y0 + gap, cz + float(rng.uniform(0.2, 0.9))], [x1, y1 - gap, cz + 2.45],
            ))

    # —— 阵纹圆盘：体素圆盘，每格按到圆心的距离落进某个同心带 ——
    disc_z = cz + DISC_FRONT
    steps = int(DISC_RADIUS * 1.2 / DISC_CELL) + 1
    for i in range(-steps, steps):
        for j in range(-steps, steps):
            dx = (i + 0.5) * DISC_CELL
            dy = (j + 0.5) * DISC_CELL
            ratio = math.hypot(dx, dy) / DISC_RADIUS
            band = next((mat for limit, mat in DISC_BANDS if ratio < limit), None)
            if band is None:
                continue
            cubes.append(_box(
                f"chest_disc_{i + steps}_{j + steps}", g, band,
                [cx + i * DISC_CELL, DISC_CENTER_Y + j * DISC_CELL, disc_z],
                [cx + (i + 1) * DISC_CELL, DISC_CENTER_Y + (j + 1) * DISC_CELL, disc_z + DISC_THICK],
            ))

    # —— 领口崩石：脖子两侧各两块高低石块，左右各自随机；x 不超过肩环核心内缘 3.4 ——
    for sign, tag in ((1, "r"), (-1, "l")):
        x = 1.95
        for cell in range(2):
            width = float(rng.uniform(0.7, 1.0))
            height = float(rng.uniform(0.8, 2.0))
            depth_back = float(rng.uniform(8.6, 9.2))
            x_end = min(x + width, 3.4)
            x0, x1 = sorted((cx + sign * x, cx + sign * x_end))
            cubes.append(_box(
                f"chest_collar_{tag}_{cell}", g, str(rng.choice(["stone_light", "stone_mid", "stone_dark"])),
                [x0, 26.2, depth_back], [x1, 26.2 + height, cz + 2.2],
            ))
            x = x_end + 0.05

    return cubes


# ───────────────────────── 部件 04：石肩甲 ─────────────────────────

# 肩甲自下而上四层 (下沿 y, 上沿 y, 内缘 x, 外缘 x, 后缘 z 偏移, 前缘 z 偏移)，x 是相对身体中线的距离，
# z 是相对 cz 的偏移。第二层最宽，整体盖在肩关节环外侧、明显宽于上臂（上臂 5.2..7.6）。
# 内缘 4.8 以内留给胸甲侧翼（半宽最大 4.6）和肩环的内半边。
PAULDRON_TIERS = [
    (23.2, 25.2, 5.2, 10.0, -2.6, 2.8),
    (25.2, 27.4, 4.8, 10.4, -3.0, 3.2),
    (27.4, 29.0, 5.0, 9.8, -2.7, 2.7),
    (29.0, 29.8, 5.6, 9.2, -2.2, 2.2),
]
PAULDRON_SEEDS = {"r": 2026_1005_041, "l": 2026_1005_042}   # 左右各自的种子，崩缺不对称


def part_pauldrons() -> List[Cube]:
    """04_pauldrons：两块巨大方石肩甲，盖在肩关节铜环外侧。

    结构（按 parts_ref/04_pauldrons.png）：
    1. 每侧四层叠起的粗石堆，每层切成 1.2~2.2 见方的大块，块的上下沿各自随机起伏 ±0.3，
       层与层互相咬合；
    2. 边角的块按概率整块缺掉（崩缺），所以轮廓参差、不是方盒；
    3. 肩甲下沿露出一圈锈铜轴套（肩关节轴的外端），对应参考图里肩甲下的褐色圆柱；
    4. 风化石三档加少量整块苔斑。
    左右用不同种子，崩缺位置不同；固定种子，重跑一致。
    """
    g = "pauldron"
    cx, cz = CENTER_X, CENTER_Z
    cubes: List[Cube] = []
    stone_choices = ["stone_light", "stone_mid", "stone_dark", "moss"]
    stone_weights = [0.40, 0.34, 0.19, 0.07]
    gap = 0.04

    for sign, tag in ((1, "r"), (-1, "l")):
        rng = np.random.default_rng(PAULDRON_SEEDS[tag])
        for tier_index, (y0, y1, x_in, x_out, z_back, z_front) in enumerate(PAULDRON_TIERS):
            # 把 [x_in, x_out] x [z_back, z_front] 切成不规则的格子
            x_edges = [x_in]
            while x_edges[-1] < x_out - 0.8:
                x_edges.append(min(x_edges[-1] + float(rng.uniform(1.2, 2.2)), x_out))
            if x_out - x_edges[-1] > 0.05:
                x_edges.append(x_out)
            z_edges = [z_back]
            while z_edges[-1] < z_front - 0.8:
                z_edges.append(min(z_edges[-1] + float(rng.uniform(1.2, 2.2)), z_front))
            if z_front - z_edges[-1] > 0.05:
                z_edges.append(z_front)

            for xi in range(len(x_edges) - 1):
                for zi in range(len(z_edges) - 1):
                    on_edge = xi in (0, len(x_edges) - 2) or zi in (0, len(z_edges) - 2)
                    corner = xi in (0, len(x_edges) - 2) and zi in (0, len(z_edges) - 2)
                    if corner and tier_index > 0 and rng.random() < 0.45:
                        continue  # 崩缺：角上的整块掉了
                    if on_edge and not corner and tier_index == len(PAULDRON_TIERS) - 1 and rng.random() < 0.3:
                        continue
                    low = y0 + float(rng.uniform(-0.15, 0.2))
                    high = y1 + float(rng.uniform(-0.3, 0.3))
                    xa, xb = sorted((cx + sign * x_edges[xi], cx + sign * x_edges[xi + 1]))
                    cubes.append(_box(
                        f"pauldron_{tag}_t{tier_index}_{xi}_{zi}", g,
                        str(rng.choice(stone_choices, p=stone_weights)),
                        [xa + gap, low, cz + z_edges[zi] + gap], [xb - gap, high, cz + z_edges[zi + 1] - gap],
                    ))

        # 肩甲下沿的锈铜轴套
        x0, x1 = sorted((cx + sign * 5.4, cx + sign * 9.6))
        cubes.append(_box(f"pauldron_{tag}_axle", g, "copper_dark", [x0, 22.8, cz - 1.8], [x1, 23.5, cz + 1.8]))
    return cubes


# ───────────────────────── 部件 05：双臂与石拳 ─────────────────────────

ARM_SEEDS = {"r": 2026_1005_051, "l": 2026_1005_052}   # 左右各自的种子，碎块位置不对称


def _stone_block_grid(
    prefix: str, group: str, rng: np.random.Generator,
    x_range: tuple, y_range: tuple, z_range: tuple, cell: tuple, chip_corner: float,
    min_cell: float = 0.0,
) -> List[Cube]:
    """把一个长方体范围切成 cell 大小上下浮动的石块网格，块的各面随机凸凹，角上的块按概率缺掉。

    x_range 以 (内缘, 外缘) 给出，调用方已按左右侧换算成绝对坐标并排好序；
    y_range、z_range 同理。每块之间留 0.04 的缝，外沿随机多伸或缩进，避免光滑盒子。
    """
    stone_choices = ["stone_light", "stone_mid", "stone_dark", "moss"]
    stone_weights = [0.40, 0.34, 0.19, 0.07]

    def edges(lo: float, hi: float, size: float) -> List[float]:
        out = [lo]
        while out[-1] < hi - size * 0.6:
            out.append(min(out[-1] + float(rng.uniform(size * 0.75, size * 1.25)), hi))
        if min_cell > 0.0 and len(out) > 1 and hi - out[-1] < min_cell:
            out[-1] = hi
        elif hi - out[-1] > 0.05:
            out.append(hi)
        return out

    x_edges = edges(*x_range, cell[0])
    y_edges = edges(*y_range, cell[1])
    z_edges = edges(*z_range, cell[2])
    cubes: List[Cube] = []
    gap = 0.04
    for xi in range(len(x_edges) - 1):
        for yi in range(len(y_edges) - 1):
            for zi in range(len(z_edges) - 1):
                on_x = xi in (0, len(x_edges) - 2)
                on_z = zi in (0, len(z_edges) - 2)
                on_y = yi in (0, len(y_edges) - 2)
                if on_x and on_z and on_y and rng.random() < chip_corner:
                    continue  # 崩缺：角上的整块掉了
                lift = float(rng.uniform(-0.1, 0.2)) if on_x or on_z else 0.0
                cube = _box(
                    f"{prefix}_{xi}_{yi}_{zi}", group,
                    str(rng.choice(stone_choices, p=stone_weights)),
                    [x_edges[xi] + gap - (lift if xi == 0 else 0.0), y_edges[yi] + gap,
                     z_edges[zi] + gap - (lift if zi == 0 else 0.0)],
                    [x_edges[xi + 1] - gap + (lift if xi == len(x_edges) - 2 else 0.0), y_edges[yi + 1] - gap,
                     z_edges[zi + 1] - gap + (lift if zi == len(z_edges) - 2 else 0.0)],
                )
                # 末尾碎格扣掉缝后可能成为零厚或反向的薄片，导出 geo 时会被拒；
                # 放在随机数全部取完之后再丢，已通过部件的其余形体不变。
                if any(cube["to"][axis] - cube["from"][axis] <= 0.01 for axis in range(3)):
                    continue
                cubes.append(cube)
    return cubes


def part_arms_fists() -> List[Cube]:
    """05_arms_fists：下垂双臂，分节石块上臂 / 前臂由 01 的锈铜肘环相连，末端大方石拳。

    按 parts_ref/05_arms_fists.png，自上而下：
    1. 上臂石套：肩甲下沿（y=23.2）到肘环上缘（y≈21.7）之间一段石块，裹在 01 的上臂芯外；
    2. 前臂石套：肘环下缘（y≈18.9）到腕部之间，比上臂略宽、碎块更多；
    3. 腕部锈铜宽环：比 01 的腕带更粗，把前臂和拳隔开；
    4. 大方石拳：比前臂宽，下半排四个指节石块朝前下垂。
    左右各自的种子，碎块位置不对称；固定种子，重跑一致。
    """
    g = "arm"
    cx, cz = CENTER_X, CENTER_Z
    cubes: List[Cube] = []

    for sign, tag in ((1, "r"), (-1, "l")):
        rng = np.random.default_rng(ARM_SEEDS[tag])

        def span(x_in: float, x_out: float) -> tuple:
            return tuple(sorted((cx + sign * x_in, cx + sign * x_out)))

        # —— 上臂石套 ——
        cubes += _stone_block_grid(
            f"arm_{tag}_upper", g, rng, span(4.95, 8.25), (21.75, 23.15), (cz - 2.0, cz + 2.0),
            (1.7, 0.75, 1.7), chip_corner=0.2,
        )
        # —— 前臂石套 ——
        cubes += _stone_block_grid(
            f"arm_{tag}_fore", g, rng, span(5.85, 9.05), (15.65, 18.85), (cz - 2.0, cz + 2.0),
            (1.6, 1.1, 1.6), chip_corner=0.3,
        )
        # —— 腕部锈铜宽环：下缘比 01 的腕带低，上缘与前臂石套留 0.05 缝 ——
        x0, x1 = span(5.95, 9.0)
        cubes.append(_box(f"arm_{tag}_wrist_ring", g, "copper_lit", [x0, 14.95, cz - 1.8], [x1, 15.6, cz + 1.8]))
        # —— 大方石拳：主体 + 前下垂的四个指节 ——
        cubes += _stone_block_grid(
            f"arm_{tag}_fist", g, rng, span(5.8, 9.9), (13.15, 14.9), (cz - 2.1, cz + 2.1),
            (1.6, 0.9, 1.6), chip_corner=0.25,
        )
        finger_edges = [cz - 2.1, cz - 1.0, cz + 0.1, cz + 1.2, cz + 2.3]
        for index in range(4):
            low = 12.0 + float(rng.uniform(0.0, 0.5))
            x0, x1 = span(6.0, 9.7)
            cubes.append(_box(
                f"arm_{tag}_finger_{index}", g,
                str(rng.choice(["stone_light", "stone_mid", "stone_dark"], p=[0.4, 0.4, 0.2])),
                [x0, low, finger_edges[index] + 0.04], [x1, 13.1, finger_edges[index + 1] - 0.04],
            ))
    return cubes


# ───────────────────────── 部件 06：腰带与破布前襟 ─────────────────────────

TABARD_SEED = 2026_1005_061
BELT_Y = (15.0, 16.2)
BELT_FRONT_Z = (3.75, 4.2)    # 胸甲小腹舌前表面最远 cz+3.6，腰带前板压在它前面
BELT_BACK_Z = (-3.3, -2.85)
BELT_SIDE_X = (4.9, 5.4)


def _cloth_strips(
    prefix: str, group: str, rng: np.random.Generator, x_half: float, y_top: float,
    z_near: float, z_dir: int, bottom_center: float, bottom_edge: float,
) -> List[Cube]:
    """一片破布：宽 0.45~0.75 的竖条并排，条长自中间向两边渐短再随机撕参差。

    z_near 是布片靠身体一侧的 z 偏移，z_dir 指向 +1（前襟）或 -1（后襟）；
    每条的厚度和前后位置各自抖动，条与条错开同向面，避免共面闪烁。
    """
    cubes: List[Cube] = []
    x = -x_half
    index = 0
    while x < x_half - 0.3:
        width = float(rng.uniform(0.45, 0.75))
        x1 = min(x + width, x_half)
        mid = abs((x + x1) / 2) / x_half
        bottom = bottom_center + (bottom_edge - bottom_center) * mid + float(rng.uniform(-1.3, 0.9))
        near = z_near + float(rng.uniform(0.0, 0.25)) * z_dir
        far = near + float(rng.uniform(0.25, 0.4)) * z_dir
        z0, z1 = sorted((CENTER_Z + near, CENTER_Z + far))
        material = str(rng.choice(["cloth_light", "cloth_dark"], p=[0.5, 0.5]))
        cubes.append(_box(
            f"{prefix}_{index}", group, material,
            [CENTER_X + x + 0.03, bottom, z0], [CENTER_X + x1 - 0.03, y_top, z1],
        ))
        x = x1
        index += 1
    return cubes


def part_belt_tabard() -> List[Cube]:
    """06_belt_tabard：锈铜腰带 + 前后长垂的破布 + 腰侧石片，底边撕成参差布条。

    参照 parts_ref/06_belt_tabard.png：
    1. 腰带：前、后、左右四块锈铜板围一圈，前面两侧各一个方环扣（锈铜亮 + 深影孔）；
    2. 前襟：中间最长、向两边渐短的旧布竖条，底边每条长短不一；
    3. 后襟：同样做法，更宽更长；
    4. 腰侧石片：每侧一块碎石片垂在腰带下，带苔斑，盖在大腿外侧前方；
    5. 腰侧碎布：左右腰带下各挂一排更窄的短布条，侧视能看到层层垂落。
    固定种子，重跑一致。
    """
    g = "belt_tabard"
    cx, cz = CENTER_X, CENTER_Z
    rng = np.random.default_rng(TABARD_SEED)
    y0, y1 = BELT_Y
    cubes: List[Cube] = []

    # —— 腰带 ——
    cubes.append(_box("belt_front", g, "copper_dark", [cx - 5.3, y0, cz + BELT_FRONT_Z[0]], [cx + 5.3, y1, cz + BELT_FRONT_Z[1]]))
    cubes.append(_box("belt_back", g, "copper_dark", [cx - 5.25, y0 + 0.05, cz + BELT_BACK_Z[0]], [cx + 5.25, y1 - 0.05, cz + BELT_BACK_Z[1]]))
    for sign, tag in ((1, "r"), (-1, "l")):
        x_lo, x_hi = sorted((cx + sign * BELT_SIDE_X[0], cx + sign * BELT_SIDE_X[1]))
        cubes.append(_box(f"belt_side_{tag}", g, "copper_dark", [x_lo, y0 + 0.1, cz + BELT_BACK_Z[0] + 0.05], [x_hi, y1 - 0.1, cz + BELT_FRONT_Z[1] - 0.05]))
        cubes += _ring_on_face(
            f"belt_buckle_{tag}", g, "z", 1, [cx + sign * 3.9, (y0 + y1) / 2, cz],
            cz + BELT_FRONT_Z[1], 1.1, 0.4, 0.25,
        )

    # —— 前襟 / 后襟 ——
    cubes += _cloth_strips("tabard_front", g, rng, 2.2, y0, BELT_FRONT_Z[1] + 0.05, 1, 6.2, 9.6)
    cubes += _cloth_strips("tabard_back", g, rng, 3.0, y0, BELT_BACK_Z[0] - 0.05, -1, 6.0, 9.0)

    # —— 腰侧石片 + 碎布 ——
    for sign, tag in ((1, "r"), (-1, "l")):
        x_lo, x_hi = sorted((cx + sign * 3.1, cx + sign * 5.2))
        slab_rng = np.random.default_rng(TABARD_SEED + (1 if sign > 0 else 2))
        cubes += _stone_block_grid(
            f"tabard_stone_{tag}", g, slab_rng, (x_lo, x_hi), (9.8, 14.9), (cz + 3.8, cz + 4.5),
            (1.1, 1.8, 0.7), chip_corner=0.3,
        )
        side_x_lo, side_x_hi = sorted((cx + sign * 5.5, cx + sign * 5.9))
        side_rng = np.random.default_rng(TABARD_SEED + 10 + (1 if sign > 0 else 2))
        z = cz - 3.0
        index = 0
        while z < cz + 3.6:
            depth = float(side_rng.uniform(0.5, 0.9))
            bottom = float(side_rng.uniform(7.2, 11.5))
            cubes.append(_box(
                f"tabard_side_{tag}_{index}", g,
                str(side_rng.choice(["cloth_light", "cloth_dark"])),
                [side_x_lo + float(side_rng.uniform(0.0, 0.1)), bottom, z + 0.03],
                [side_x_hi, y1 - 0.15, min(z + depth, cz + 4.2) - 0.03],
            ))
            z += depth
            index += 1
    return cubes


# ───────────────────────── 部件 07：石桩腿与石座脚 ─────────────────────────

LEG_SEEDS = {"r": 2026_1005_071, "l": 2026_1005_072}   # 左右各自的种子，碎块位置不对称


def part_legs_feet() -> List[Cube]:
    """07_legs_feet：两条粗短石桩腿 + 锈铜膝环 + 宽扁石座脚，脚底贴地 y=0。

    按 parts_ref/07_legs_feet.png，自上而下：
    1. 大腿石套：髋环下缘（y≈14.85）到膝环上缘（y≈11.35）之间，裹在 01 的大腿芯外；
    2. 膝环沿用 01 的锈铜环关节，露在两段石套之间；
    3. 小腿石套：膝环下缘（y≈8.05）到踝部，比大腿略宽；
    4. 踝部锈铜宽环；
    5. 石座脚：宽扁的碎石块底座，向前后外侧都比小腿宽，上面再叠一级后跟石台；
       底座网格的下沿故意取 -0.04，扣掉块间缝之后正好是 y=0。
    左右各自种子；固定种子，重跑一致。
    """
    g = "leg"
    cx, cz = CENTER_X, CENTER_Z
    cubes: List[Cube] = []

    for sign, tag in ((1, "r"), (-1, "l")):
        rng = np.random.default_rng(LEG_SEEDS[tag])

        def span(x_in: float, x_out: float) -> tuple:
            return tuple(sorted((cx + sign * x_in, cx + sign * x_out)))

        cubes += _stone_block_grid(
            f"leg_{tag}_thigh", g, rng, span(1.5, 4.75), (11.45, 14.8), (cz - 2.0, cz + 2.0),
            (1.6, 1.1, 1.7), chip_corner=0.3, min_cell=0.6,
        )
        cubes += _stone_block_grid(
            f"leg_{tag}_shin", g, rng, span(2.2, 5.45), (3.8, 8.0), (cz - 2.0, cz + 2.0),
            (1.6, 1.4, 1.7), chip_corner=0.3, min_cell=0.6,
        )
        x0, x1 = span(2.3, 5.3)
        cubes.append(_box(f"leg_{tag}_ankle_ring", g, "copper_lit", [x0, 3.1, cz - 1.7], [x1, 3.7, cz + 1.7]))
        cubes += _stone_block_grid(
            f"leg_{tag}_foot", g, rng, span(1.4, 6.5), (-0.04, 2.2), (cz - 2.3, cz + 3.5),
            (1.7, 1.1, 1.7), chip_corner=0.35, min_cell=0.6,
        )
        cubes += _stone_block_grid(
            f"leg_{tag}_heel", g, rng, span(2.3, 5.6), (2.2, 2.9), (cz - 1.8, cz + 1.5),
            (1.6, 0.7, 1.6), chip_corner=0.2, min_cell=0.4,
        )
    return cubes


PART_BUILDERS: List[Callable[[], List[Cube]]] = [
    part_core_frame, part_head, part_chest_plate, part_pauldrons, part_arms_fists, part_belt_tabard,
    part_legs_feet,
]


# 头部石壳整个包住并取代 01 里的颅芯占位（颅芯 / 颅顶 / 颅侧小环），拼装时去掉，
# 否则内外两层同向面大面积重合。颈（core_neck）仍保留，接在头下。
HEAD_REPLACES_PREFIX = "core_skull"
# 同理，07 的石座脚整个包住 01 的脚芯（core_foot_r/l）并与它同在 y=0 底面，拼装时去掉脚芯。
FOOT_REPLACES_PREFIX = "core_foot_"


def assemble(builders: List[Callable[[], List[Cube]]]) -> List[Cube]:
    cubes = [c for build in builders for c in build()]
    if part_head in builders:
        cubes = [c for c in cubes if not str(c["name"]).startswith(HEAD_REPLACES_PREFIX)]
    if part_legs_feet in builders:
        cubes = [c for c in cubes if not str(c["name"]).startswith(FOOT_REPLACES_PREFIX)]
    return cubes


def all_cubes() -> List[Cube]:
    """当前累积的全部部件。每过一件，调度回复后才往这里加下一件。"""
    return assemble(PART_BUILDERS)


# ───────────────────────── 门禁 ─────────────────────────

def assert_no_coplanar(cubes: List[Cube]) -> None:
    violations = check_coplanar_faces(cubes)
    if violations:
        raise AssertionError(violations[0])


# ───────────────────────── 贴图 ─────────────────────────

def _blend(a: List[int], b: List[int], t: float) -> List[int]:
    return [round(a[i] * (1 - t) + b[i] * t) for i in range(3)]


def _paint_tile(draw: ImageDraw.ImageDraw, ox: int, oy: int, base: List[int],
                accent: List[int], accent_ratio: float, seed: int) -> None:
    """16x16 色块：底色逐像素在 base 与 accent 之间做斑驳。"""
    rng = np.random.default_rng(seed)
    for y in range(16):
        for x in range(16):
            colour = _blend(base, accent, 0.55) if rng.random() < accent_ratio else base
            if rng.random() < 0.08:
                colour = _blend(colour, PALETTE["shadow"], 0.5)
            draw.point((ox + x, oy + y), fill=tuple(colour))


def build_texture(res: int = RES) -> Image.Image:
    image = Image.new("RGB", (res, res), tuple(PALETTE["shadow"]))
    draw = ImageDraw.Draw(image)
    wood = _blend(PALETTE["shadow"], PALETTE["copper_dark"], 0.28)
    specs = {
        "frame_wood": (wood, PALETTE["copper_dark"], 0.22),
        "frame_wood_lit": (_blend(wood, PALETTE["stone_dark"], 0.40), PALETTE["stone_dark"], 0.28),
        "copper_lit": (PALETTE["copper_lit"], PALETTE["copper_dark"], 0.45),
        "copper_dark": (PALETTE["copper_dark"], PALETTE["shadow"], 0.25),
        "shadow": (PALETTE["shadow"], PALETTE["copper_dark"], 0.05),
        "stone_dark": (PALETTE["stone_dark"], PALETTE["stone_mid"], 0.35),
        "stone_mid": (PALETTE["stone_mid"], PALETTE["stone_light"], 0.35),
        "stone_light": (PALETTE["stone_light"], PALETTE["stone_mid"], 0.35),
        "moss": (PALETTE["moss"], PALETTE["shadow"], 0.20),
        "cloth_light": (PALETTE["cloth_light"], PALETTE["cloth_dark"], 0.35),
        "cloth_dark": (PALETTE["cloth_dark"], PALETTE["shadow"], 0.25),
    }
    for index, name in enumerate(MATERIALS):
        u0, v0, _, _ = MAT_UV[name]
        base, accent, ratio = specs[name]
        _paint_tile(draw, u0, v0, base, accent, ratio, seed=100 + index)
    return image


# ───────────────────────── bbmodel 输出 ─────────────────────────

UV_PIXELS_PER_UNIT = 3.5  # 贴图密度：1 个模型单位取几个贴图像素，保证斑驳颗粒在各面大小一致


def _face_uvs(cube: Cube) -> Dict[str, dict]:
    """每个面在材质色块里取一个与面尺寸等比的窗口，避免把 16x16 的斑驳拉成条纹。"""
    tile_u0, tile_v0, _, _ = MAT_UV[str(cube["material"])]
    dx, dy, dz = (cube["to"][i] - cube["from"][i] for i in range(3))
    face_sizes = {
        "north": (dx, dy), "south": (dx, dy),
        "east": (dz, dy), "west": (dz, dy),
        "up": (dx, dz), "down": (dx, dz),
    }
    faces = {}
    for side, (width, height) in face_sizes.items():
        win_u = min(16, max(2, round(width * UV_PIXELS_PER_UNIT)))
        win_v = min(16, max(2, round(height * UV_PIXELS_PER_UNIT)))
        faces[side] = {"uv": [tile_u0, tile_v0, tile_u0 + win_u, tile_v0 + win_v], "texture": 0}
    return faces

def recenter_to_origin(cubes: List[Cube]) -> List[Cube]:
    """把身体中轴从 (x=8, z=8) 平移到 (0, 0)，与其它 v2 生物建模源一致（gen_rig 按原点做绑定）。

    只平移，不改朝向：建模源本来就面朝 +Z，脸在 FRONT。
    """
    return [
        {
            **cube,
            "from": [cube["from"][0] - CENTER_X, cube["from"][1], cube["from"][2] - CENTER_Z],
            "to": [cube["to"][0] - CENTER_X, cube["to"][1], cube["to"][2] - CENTER_Z],
        }
        for cube in cubes
    ]


def generate_bbmodel(out_path: Path = BBMODEL_OUT, cubes_override: List[Cube] | None = None) -> Path:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = cubes_override if cubes_override is not None else all_cubes()
    assert_no_coplanar(cubes)
    cubes = recenter_to_origin(cubes)

    buffer = io.BytesIO()
    build_texture(RES).save(buffer, format="PNG")
    texture_source = "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")

    elements = []
    for cube in cubes:
        faces = _face_uvs(cube)
        elements.append({
            "name": cube["name"],
            "box_uv": False,
            "from": cube["from"],
            "to": cube["to"],
            "faces": faces,
            "uuid": str(uuid.uuid5(uuid.NAMESPACE_URL, f"bong/tsy_sentinel/{cube['name']}")),
        })

    groups: Dict[str, List[str]] = {}
    for element, cube in zip(elements, cubes):
        groups.setdefault(str(cube["group"]), []).append(element["uuid"])

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "tsy_sentinel_v2",
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": [{"name": g, "origin": [0.0, 0.0, 0.0], "children": ids} for g, ids in groups.items()],
        "textures": [{
            "name": "tsy_sentinel", "folder": "entity", "namespace": "bong", "id": 0,
            "source": texture_source,
        }],
    }
    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    print(f"✓ TsySentinel bbmodel 写入成功: {out_path}")
    return out_path


# ───────────────────────── 渲染与对照卡 ─────────────────────────

TARGET_H = 600
CARD_BG = (28, 30, 34)


def _crop_tight(image: Image.Image, threshold: float = 15.0) -> Image.Image:
    arr = np.array(image.convert("RGB")).astype(float)
    background = arr[0, 0]
    ys, xs = np.where(np.linalg.norm(arr - background, axis=2) > threshold)
    if len(xs) == 0:
        return image
    return image.crop((xs.min(), ys.min(), xs.max() + 1, ys.max() + 1))


def _scale_to_height(image: Image.Image, height: int) -> Image.Image:
    width = max(1, int(image.width * height / image.height))
    return image.resize((width, height), Image.Resampling.LANCZOS)


def _render_views(model_path: Path) -> List[Image.Image]:
    """正面 / 侧面 / 3/4。"""
    from bbmodel_maker.render.render_bbmodel import render

    views = []
    for yaw, pitch in ((0, 0), (90, 0), (-35, 20)):
        image, _ = render(str(model_path), yaw=yaw, pitch=pitch, size=TARGET_H)
        views.append(_scale_to_height(_crop_tight(image), TARGET_H))
    return views


def _reference_halves(ref_name: str) -> List[Image.Image]:
    """参考图左半是正面、右半是侧面，各自按非背景像素裁准。"""
    reference = Image.open(PARTS_REF_DIR / ref_name).convert("RGB")
    arr = np.array(reference).astype(float)
    background = arr[0, 0]
    mask = np.linalg.norm(arr - background, axis=2) > 22
    # 正面与侧面之间的空列：在中间 30%..70% 范围内取最靠中间的空列，不要硬切在宽度一半
    empty_columns = [x for x in range(int(reference.width * 0.3), int(reference.width * 0.7))
                     if not mask[:, x].any()]
    middle = min(empty_columns, key=lambda x: abs(x - reference.width // 2)) if empty_columns \
        else reference.width // 2
    halves = []
    for x_start, x_end in ((0, middle), (middle, reference.width)):
        ys, xs = np.where(mask[:, x_start:x_end])
        halves.append(_scale_to_height(
            reference.crop((xs.min() + x_start, ys.min(), xs.max() + x_start + 1, ys.max() + 1)),
            TARGET_H,
        ))
    return halves


def _compose(sections: List[tuple], out_path: Path) -> None:
    """sections：[(标题, [图...]), ...]，同一节内图并排，节间留分隔线。"""
    gap = 20
    widths = [sum(img.width for img in imgs) + gap * (len(imgs) - 1) for _, imgs in sections]
    card = Image.new("RGB", (sum(widths) + gap * (len(sections) + 1), TARGET_H + 80), CARD_BG)
    draw = ImageDraw.Draw(card)
    x = gap
    for index, (title, imgs) in enumerate(sections):
        draw.text((x, 20), title, fill=(210, 205, 190))
        cursor = x
        for img in imgs:
            card.paste(img, (cursor, 60))
            cursor += img.width + gap
        x += widths[index] + gap
        if index < len(sections) - 1:
            draw.line([(x - gap // 2, 20), (x - gap // 2, card.height - 20)], fill=(80, 84, 92), width=2)
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    card.save(out_path)
    print(f"→ {out_path}")


def render_part(number: str, part_name: str, builders: List[Callable[[], List[Cube]]]) -> None:
    """单件图 + 对照卡 + （第 2 件起）累计图。builders 是到当前为止的全部部件。"""
    tmp = Path("/tmp") / f"TsySentinel_part_{number}.bbmodel"
    generate_bbmodel(tmp, cubes_override=builders[-1]())
    single = _render_views(tmp)
    ref_name = f"{number}_{part_name}.png"
    _compose([(f"NOW {number}_{part_name}  FRONT / SIDE / 3/4", single)], PARTS_DIR / f"{number}_{part_name}.png")
    _compose(
        [(f"REF parts_ref/{ref_name}", _reference_halves(ref_name)),
         (f"NOW {number}_{part_name}  FRONT / SIDE / 3/4", single)],
        PARTS_DIR / f"check_{number}_{part_name}.png",
    )
    if len(builders) > 1:
        generate_bbmodel(tmp, cubes_override=assemble(builders))
        _compose(
            [(f"ACCUM 01..{number}  FRONT / SIDE / 3/4", _render_views(tmp))],
            PARTS_DIR / f"accum_{number}_{part_name}.png",
        )
    tmp.unlink(missing_ok=True)


# ───────────────────────── 自证 ─────────────────────────

def self_test() -> None:
    """差分自证：当前模型无共面；注入一个与现有立方体同向共面的缺陷必须被抓到。"""
    print("运行 gen_tsy_sentinel.py 差分自证...")
    cubes = all_cubes()
    try:
        assert_no_coplanar(cubes)
    except AssertionError as error:
        print(f"  [FAIL] 正常立方体集出现共面冲突: {error}")
        sys.exit(1)
    print(f"  [OK] {len(cubes)} 个立方体无共面冲突")

    spine = next(c for c in cubes if c["name"] == "core_spine_0")
    lo, hi = spine["from"], spine["to"]
    defect = _box("inject_coplanar_fail", "core_frame", "frame_wood",
                  [lo[0] + 0.2, lo[1], lo[2] + 0.2], [hi[0] - 0.2, hi[1], hi[2] - 0.2])
    try:
        assert_no_coplanar(cubes + [defect])
    except AssertionError as error:
        print(f"  [OK] 成功捕获注入缺陷: {error}")
    else:
        print("  [FAIL] 注入缺陷未被门禁捕获！")
        sys.exit(1)

    # 镜像对称：左右两侧关节核心关于 x=8 对称，防止镜像函数写歪
    cores = {c["name"]: c for c in cubes if c["name"].endswith("_core")}
    pairs = 0
    for name, right in cores.items():
        if "_r_" not in name and not name.endswith("_r_core"):
            continue
        left = cores[name.replace("_r", "_l", 1)]
        if abs((right["from"][0] + left["to"][0]) - 2 * CENTER_X) > 1e-9:
            print(f"  [FAIL] {name} 与左侧不对称")
            sys.exit(1)
        pairs += 1
    if pairs == 0:
        print("  [FAIL] 没找到任何成对的关节核心，对称检查没有生效")
        sys.exit(1)
    print(f"  [OK] {pairs} 对关节核心左右对称")

    # 平移到原点后：整体 x 范围左右对称（中轴在 0），脸（眉梁）仍在 +Z 一侧
    centered = recenter_to_origin(cubes)
    x_min = min(c["from"][0] for c in centered)
    x_max = max(c["to"][0] for c in centered)
    brow = next(c for c in centered if c["name"] == "head_brow")
    if abs(x_min + x_max) > 1e-6 or brow["from"][2] <= 0:
        print(f"  [FAIL] 平移后中轴或朝向不对: x=[{x_min:.2f}, {x_max:.2f}] 眉梁 z={brow['from'][2]:.2f}")
        sys.exit(1)
    print(f"  [OK] 平移到原点后 x 范围 [{x_min:.2f}, {x_max:.2f}] 左右对称，眉梁在 +Z（z={brow['from'][2]:.2f}）")
    print("✓ gen_tsy_sentinel.py 差分自证全绿")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成秘境守灵 (tsy_sentinel) .bbmodel")
    parser.add_argument("--self-test", action="store_true", help="运行门禁缺陷注入差分自证")
    parser.add_argument("--part", default=None, help="渲染并对照指定编号部件 (如 01)；缺省只写 bbmodel，不渲染")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    generate_bbmodel(args.out)
    if args.part == "01":
        render_part("01", "core_frame", [part_core_frame])
    elif args.part == "02":
        render_part("02", "head", [part_core_frame, part_head])
    elif args.part == "03":
        render_part("03", "chest_plate", [part_core_frame, part_head, part_chest_plate])
    elif args.part == "04":
        render_part("04", "pauldrons", [part_core_frame, part_head, part_chest_plate, part_pauldrons])
    elif args.part == "05":
        render_part("05", "arms_fists", [part_core_frame, part_head, part_chest_plate, part_pauldrons, part_arms_fists])
    elif args.part == "06":
        render_part("06", "belt_tabard", PART_BUILDERS[:6])
    elif args.part == "07":
        render_part("07", "legs_feet", PART_BUILDERS)


if __name__ == "__main__":
    main()
