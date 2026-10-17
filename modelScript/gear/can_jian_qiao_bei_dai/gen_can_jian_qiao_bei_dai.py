#!/usr/bin/env python3
"""残剑鞘背带 can_jian_qiao_bei_dai Blockbench .bbmodel 程序化生成器 —— Round 1 几何（按参考图重做，整个重写）。

对应 NPC：残峰散修。坐标直接用「穿在玩家身上」的玩家坐标（脚在 y=0，+z 为背面，右臂在 +x）。

形制（对照参考图 ``can_jian_qiao_bei_dai_ref.png``）：
- 一整条斜挎背带：右肩上沿翻过（骑在右臂上沿，不碰头盒）→ 胸前斜下到左胯 → 绕过左胯外侧 → 背面再斜回右肩；
  左胯外侧一个麻绳结，垂几缕碎布；
- 剑鞘斜背在**身后**（z 中心 5.6，鞘厚 2.4，躯干背面 z=2 与鞘之间留出缠带的空隙），
  鞘轴自右肩外上方向左后腰下方倾 22°：整件从柄尾到鞘尖约 24.5 px、垂直跨度 22.7 px（占玩家身高 71%）；
- 沿鞘轴从上到下：柄（铁尾钉、缠麻绳、铸铁护手、断剑刃根的犬牙断茬）→ 一小段空隙
  → 鞘口铁箍里露出三片参差的断刃（断口用碎布缠住）→ 兽皮鞘身（18 px 长，宽 3.4→2.5、厚 2.4，缝线、一处开线露木衬、
  两道破布缠带、两个铁环系背带）→ 鞘尾锥形铁包头；
- 末法：只有鞘身上一道暗掉的刻线槽，没有发光部件。

比例对照没有用剪影：参考图是透视拼版，改成「各部件占玩家身高的百分比」，参考图一列是人从拼版第三幅（背面）量的。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

import gear_kit  # noqa: E402
import weapon_shape_kit as kit  # noqa: E402

WEAPON_ID = "can_jian_qiao_bei_dai"
MODEL_NAME = "CanJianQiaoBeiDai"
OUT_DIR = REPO / "modelScript" / "gear" / WEAPON_ID
GEAR_REVIEW_ROOT = kit.REVIEW_ROOT.parent / "gear"
REVIEW_DIR = GEAR_REVIEW_ROOT / WEAPON_ID
REF_PNG = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
TOTAL_LEN = 24.5          # 柄尾到鞘尖，沿鞘轴
GRIP_Y = 0.0
POSE = "worn"
WORN_OFFSET = (0.0, 0.0, 0.0)          # 模型本身就是玩家坐标
WORN_TITLE = "穿戴 · 斜挎背带（右肩→左后腰），剑鞘在身后"
WORN_VIEWS = [("穿戴: 正面 FRONT (相机 -z)", 180.0, 0.0), ("穿戴: 背面 BACK (相机 +z)", 0.0, 0.0),
              ("穿戴: 3/4 背右斜", -35.0, 15.0)]
SKIP_IOU = True
PROP_YAW = 0.0

THETA = math.radians(22.0)             # 鞘轴离竖直的倾角
TOP = np.array([7.1, 33.2])            # 柄尾（鞘轴最高点）
AXIS = np.array([-math.sin(THETA), -math.cos(THETA)])    # 沿鞘轴向下的单位向量
SIDE = np.array([math.cos(THETA), -math.sin(THETA)])     # 平面内垂直鞘轴的单位向量
Z_AXIS = 5.6
ROT_Z = -22.0                          # 竖直方块绕 z 转到鞘轴方向

MATS = {
    "hide": (106, 70, 48),
    "hide_dk": (70, 48, 31),
    "hide_lt": (132, 92, 62),
    "wood": (110, 80, 52),
    "iron": (88, 80, 76),
    "iron_lt": (140, 134, 128),
    "rust": (122, 66, 36),
    "cord": (168, 140, 88),
    "cord_dk": (122, 96, 56),
    "cloth": (196, 184, 150),
    "cloth_dk": (150, 136, 104),
    "steel": (150, 148, 142),
    "groove": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("strap_shoulder", "root", (4.8, 24.3, 0.0), "右肩上沿翻过的背带段（骑在右臂上沿）"),
    ("strap_front", "strap_shoulder", (1.0, 18.0, -2.3), "胸前斜下到左胯的背带 + 铁扣 + 针孔"),
    ("strap_back", "strap_shoulder", (1.0, 18.0, 2.3), "背面斜回右肩的背带 + 两条系鞘的短带"),
    ("hip_wrap", "strap_front", (-4.3, 11.4, 0.0), "绕过左胯外侧的一段背带"),
    ("knot_tails", "hip_wrap", (-4.6, 10.8, 0.0), "左胯外侧麻绳结 + 垂下的碎布缕"),
    ("scabbard", "root", (2.0, 20.0, Z_AXIS), "兽皮剑鞘：缝线、开线露木衬、补丁"),
    ("scabbard_trim", "scabbard", (3.0, 25.0, Z_AXIS), "鞘口铁箍、鞘尾锥形铁包头、两个背带铁环"),
    ("cloth_wraps", "scabbard", (1.0, 19.0, Z_AXIS), "两道破布缠带与垂下的碎布缕"),
    ("blade_shards", "scabbard", (4.5, 26.5, Z_AXIS), "鞘口露出的三片参差断刃 + 缠断口的碎布"),
    ("hilt", "root", (6.4, 31.5, Z_AXIS), "残柄：铁尾钉、缠绳握把、铸铁护手、刃根犬牙断茬"),
    ("hilt_wrap", "hilt", (6.4, 30.5, Z_AXIS), "握把上的麻绳缠结"),
]


def pt(s: float, u: float = 0.0, w: float = 0.0) -> tuple[float, float, float]:
    """鞘轴局部坐标 → 玩家坐标：s 沿轴向下，u 垂直轴向（平面内），w 沿 z。"""
    xy = TOP + s * AXIS + u * SIDE
    return float(xy[0]), float(xy[1]), Z_AXIS + w


def lbox(rig: Rig, bone: str, name: str, s0: float, s1: float, u: float, wid: float, w: float,
         thk: float, mat: str) -> None:
    """鞘轴局部的方块：s0–s1 沿轴，宽 wid（u 向）、厚 thk（z 向），中心偏移 (u, w)，整体绕 z 转 22°。"""
    cx, cy, cz = pt((s0 + s1) / 2, u, w)
    ln = s1 - s0
    rig.cube(bone, name, (cx - wid / 2, cy - ln / 2, cz - thk / 2), (cx + wid / 2, cy + ln / 2, cz + thk / 2),
             rot=(0.0, 0.0, ROT_Z), org=(cx, cy, cz), mat=mat)


def strap(rig: Rig, bone: str, prefix: str, pts: list, half_w: float, half_t: float, up, mat: str) -> None:
    kit.strip_along(rig, bone, prefix, pts, [half_w] * len(pts), [half_t] * len(pts), mat, up=up, extend=0.06)


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 残柄（s 0–4.85）：铁尾钉、缠绳握把、铸铁护手、刃根断茬
    lbox(rig, "hilt", "pommel", 0.0, 0.8, 0, 1.9, 0, 1.3, "iron")
    lbox(rig, "hilt", "pommel_cap", 0.0, 0.2, 0, 1.5, 0, 1.0, "rust")
    lbox(rig, "hilt", "grip_core", 0.8, 4.2, 0, 1.2, 0, 1.0, "wood")
    for i in range(8):
        lbox(rig, "hilt_wrap", f"turn_{i}", 0.9 + i * 0.4, 1.15 + i * 0.4, 0, 1.55, 0, 1.3,
             "cord" if i % 2 == 0 else "cord_dk")
    lbox(rig, "hilt", "guard", 4.2, 4.85, 0, 3.4, 0, 1.0, "iron")
    for side, u in enumerate((-1.75, 1.75)):
        lbox(rig, "hilt", f"guard_tip_{side}", 4.15, 4.9, u, 0.4, 0, 0.8, "iron_lt")
    lbox(rig, "hilt", "guard_rust", 4.3, 4.75, 0.9, 0.8, 0.52, 0.06, "rust")
    lbox(rig, "hilt", "blade_root", 4.85, 5.5, 0, 1.2, 0, 0.45, "steel")
    for i, (u, ln) in enumerate(((-0.4, 0.55), (0.0, 0.85), (0.4, 0.4))):    # 犬牙断茬
        lbox(rig, "hilt", f"root_tooth_{i}", 5.5, 5.5 + ln, u, 0.34, 0, 0.4, "steel")

    # 2. 鞘口露出的断刃三片 + 缠断口的碎布（s 5.0–6.15 在鞘口之上）
    for i, (u, top_s, wd) in enumerate(((-0.75, 5.35, 0.7), (0.0, 4.9, 0.8), (0.75, 5.6, 0.65))):
        lbox(rig, "blade_shards", f"shard_{i}", top_s, 7.0, u, wd, 0, 0.42, "steel")
        lbox(rig, "blade_shards", f"shard_lip_{i}", top_s, top_s + 0.25, u, wd * 0.8, 0, 0.48, "iron_lt")
    lbox(rig, "blade_shards", "break_wrap", 6.0, 6.55, 0, 2.5, 0, 0.95, "cloth")
    lbox(rig, "blade_shards", "break_wrap_dk", 6.2, 6.4, 0, 2.6, 0, 1.0, "cloth_dk")
    for i, u in enumerate((-1.1, 0.9)):
        lbox(rig, "blade_shards", f"wrap_tail_{i}", 6.45, 7.6 + 0.4 * i, u, 0.5, 0.52, 0.1, "cloth")

    # 3. 鞘口铁箍（s 6.15–7.6）与鞘身（s 7.6–22.7）
    mouth = 6.15
    lbox(rig, "scabbard_trim", "throat", mouth, 7.6, 0, 3.8, 0, 2.7, "iron")
    lbox(rig, "scabbard_trim", "throat_lip", mouth - 0.1, mouth + 0.25, 0, 4.0, 0, 2.8, "iron_lt")
    for i, u in enumerate((-1.2, 1.2)):
        lbox(rig, "scabbard_trim", f"throat_rivet_{i}", 6.6, 7.0, u, 0.4, 1.4, 0.2, "rust")
    s_body0, s_body1 = 7.6, 22.7
    n = 20
    for k in range(n):
        s0 = s_body0 + k * (s_body1 - s_body0) / n
        s1 = s0 + (s_body1 - s_body0) / n + 0.04
        t = (k + 0.5) / n
        wid = 3.4 - 0.9 * t * t
        thk = 2.4 - 0.3 * t
        mat = "hide" if k % 3 != 1 else "hide_dk"
        lbox(rig, "scabbard", f"body_a_{k:02d}", s0, s1, 0, wid, 0, thk * 0.68, mat)
        lbox(rig, "scabbard", f"body_b_{k:02d}", s0, s1, 0, wid * 0.74, 0, thk, mat)
    for i, (s, u) in enumerate(((9.4, 0.6), (13.2, -0.9), (19.6, 0.5))):     # 补丁
        lbox(rig, "scabbard", f"patch_{i}", s, s + 1.3, u, 1.0, 1.22, 0.08, "hide_lt")
    # 缝线：沿背面（+z，朝身体的反面是 -z；缝在外侧 +z）
    for i in range(11):
        s = 8.2 + i * 1.25
        if 12.0 <= s <= 13.9:
            continue
        lbox(rig, "scabbard", f"stitch_{i:02d}", s, s + 0.5, 0.0, 0.16, 1.22, 0.07, "cord")
    # 一处开线：露出木衬
    lbox(rig, "scabbard", "open_seam", 12.0, 13.9, 0.0, 0.42, 1.2, 0.1, "groove")
    lbox(rig, "scabbard", "wood_lining", 12.1, 13.8, 0.0, 0.7, 1.16, 0.06, "wood")
    lbox(rig, "scabbard", "dead_line", 15.0, 17.6, -1.0, 0.1, 1.22, 0.07, "groove")

    # 4. 鞘尾铁包头（三级收尖）与鞘身的两个背带铁环
    lbox(rig, "scabbard_trim", "chape_0", 21.6, 22.8, 0, 2.7, 0, 2.2, "iron")
    lbox(rig, "scabbard_trim", "chape_1", 22.8, 23.7, 0, 1.9, 0, 1.6, "iron")
    lbox(rig, "scabbard_trim", "chape_2", 23.7, 24.5, 0, 0.9, 0, 0.9, "iron_lt")
    lbox(rig, "scabbard_trim", "chape_lip", 21.5, 21.85, 0, 2.9, 0, 2.35, "rust")
    ring_s = (12.8, 18.0)
    for i, s in enumerate(ring_s):
        lbox(rig, "scabbard_trim", f"ring_{i}", s - 0.35, s + 0.35, 0, 1.3, -1.45, 0.45, "iron")
        lbox(rig, "scabbard_trim", f"ring_hole_{i}", s - 0.12, s + 0.12, 0, 0.7, -1.47, 0.5, "groove")

    # 5. 两道破布缠带 + 垂下的碎布缕
    for i, s in enumerate((9.6, 19.8)):
        lbox(rig, "cloth_wraps", f"wrap_{i}", s, s + 1.1, 0, 3.7 - 0.45 * i, 0, 2.95 - 0.2 * i, "cloth")
        lbox(rig, "cloth_wraps", f"wrap_dk_{i}", s + 0.2, s + 0.45, 0, 3.8 - 0.45 * i, 0, 3.0 - 0.2 * i, "cloth_dk")
        for j, (u, ln) in enumerate(((-1.5, 1.7), (-1.1, 2.4), (1.5, 1.4))):
            lbox(rig, "cloth_wraps", f"frill_{i}_{j}", s + 1.0, s + 1.0 + ln, u, 0.45, 0.2, 0.12, "cloth")

    # 6. 背带：肩段骑在右臂上沿，胸前 / 背面各一条斜带回到左胯
    half_w, half_t = 0.7, 0.25
    shoulder_x, shoulder_y = 4.85, 24.3
    rig.cube("strap_shoulder", "over_shoulder", (shoulder_x - 0.75, 24.02, -2.55), (shoulder_x + 0.75, 24.55, 2.55), mat="hide")
    rig.cube("strap_shoulder", "over_shoulder_edge", (shoulder_x - 0.78, 24.45, -2.5), (shoulder_x + 0.78, 24.6, 2.5), mat="hide_lt")
    for z, nm in ((-2.3, "f"), (2.3, "b")):
        rig.cube("strap_shoulder", f"drop_{nm}", (shoulder_x - 0.75, 23.45, z - 0.25), (shoulder_x + 0.75, 24.55, z + 0.25), mat="hide")
    front = [(4.85, 23.6, -2.3), (3.2, 21.3, -2.3), (1.2, 18.6, -2.3), (-0.8, 15.8, -2.3), (-2.8, 12.9, -2.3), (-3.5, 11.15, -2.3)]
    strap(rig, "strap_front", "front", front, half_w, half_t, (0.0, 0.0, -1.0), "hide")
    back = [(4.85, 23.6, 2.3), (3.2, 21.3, 2.3), (1.2, 18.6, 2.3), (-0.8, 15.8, 2.3), (-2.8, 12.9, 2.3), (-3.5, 11.15, 2.3)]
    strap(rig, "strap_back", "back", back, half_w, half_t, (0.0, 0.0, 1.0), "hide")
    # 前带：铁扣 + 针孔 + 缝线
    for i, (x, y) in enumerate(((2.4, 20.0), (-1.8, 14.6))):
        rig.cube("strap_front", f"buckle_{i}", (x - 0.55, y - 0.95, -2.62), (x + 0.55, y + 0.95, -2.5), mat="iron")
        rig.cube("strap_front", f"buckle_pin_{i}", (x - 0.1, y - 0.5, -2.7), (x + 0.1, y + 0.5, -2.62), mat="iron_lt")
    for i, (x, y) in enumerate(((3.9, 22.4), (3.6, 21.9), (0.2, 17.2), (-0.2, 16.7), (-3.4, 12.2), (-3.2, 12.6))):
        rig.cube("strap_front", f"hole_{i}", (x - 0.07, y - 0.07, -2.58), (x + 0.07, y + 0.07, -2.52), mat="groove")
    # 背带绕过左胯外侧（x≈-4.3，贴着腿外侧、在手臂摆动盒下面）
    hip = [(-3.5, 11.15, -2.4), (-4.2, 11.05, -2.35), (-4.45, 11.0, -1.9), (-4.5, 10.95, 0.0), (-4.45, 11.0, 1.9), (-4.2, 11.05, 2.35), (-3.5, 11.15, 2.4)]
    strap(rig, "hip_wrap", "hip_f", hip[:3], 0.62, 0.22, (0.0, 0.0, -1.0), "hide")
    strap(rig, "hip_wrap", "hip_s", hip[2:5], 0.62, 0.22, (-1.0, 0.0, 0.0), "hide")
    strap(rig, "hip_wrap", "hip_b", hip[4:], 0.62, 0.22, (0.0, 0.0, 1.0), "hide")
    # 背面的两条短带：从鞘背铁环拉到背面斜带
    for i, s in enumerate(ring_s):
        rx, ry, rz = pt(s, 0, -1.45)
        k = (23.6 - ry) / (23.6 - 11.15)
        bx = 4.85 + (-3.5 - 4.85) * k
        mid = ((rx + bx) / 2, (ry + ry) / 2, (rz + 2.3) / 2)
        strap(rig, "strap_back", f"link_{i}", [(rx, ry, rz + 0.1), mid, (bx, ry, 2.3)], 0.42, 0.18, (0.0, 1.0, 0.0), "hide_dk")

    # 6b. 背带细节：两条斜带上的缝线与铆钉，边缘磨损的浅色棱
    for nm, zc, sign, pts_ in (("f", -2.3, -1.0, front), ("b", 2.3, 1.0, back)):
        for i in range(len(pts_) - 1):
            a, b = pts_[i], pts_[i + 1]
            for j in range(3):
                t = (j + 0.5) / 3
                x, y = a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t
                rig.cube("strap_front" if nm == "f" else "strap_back", f"stitch_{nm}_{i}_{j}",
                         (x - 0.45, y - 0.05, zc + sign * 0.25), (x + 0.45, y + 0.05, zc + sign * 0.31), mat="cord")
        for i, (x, y, _) in enumerate(pts_[1:-1]):
            rig.cube("strap_front" if nm == "f" else "strap_back", f"stud_{nm}_{i}", (x - 0.12, y - 0.12, zc + sign * 0.25),
                     (x + 0.12, y + 0.12, zc + sign * 0.38), mat="iron")
    # 6c. 鞘身两侧的缝边与鞘口下一圈压边皮
    for i, u in enumerate((-1.35, 1.35)):
        lbox(rig, "scabbard", f"edge_{i}", 7.7, 22.0, u, 0.22, 0.1, 1.5, "hide_dk")
    for i, s_ in enumerate(range(9, 22, 2)):
        for j, u in enumerate((-1.42, 1.42)):
            lbox(rig, "scabbard", f"stud_{i}_{j}", s_ - 0.12, s_ + 0.12, u, 0.2, 0.2, 0.7, "rust")
    lbox(rig, "scabbard", "collar_skin", 7.6, 8.4, 0, 3.55, 0, 2.5, "hide_lt")

    # 7. 左胯外侧麻绳结 + 碎布缕
    kx, ky = -4.75, 11.15
    rig.cube("knot_tails", "knot_a", (kx - 0.35, ky - 0.3, -0.45), (kx + 0.35, ky + 0.35, 0.45), mat="cord_dk")
    rig.cube("knot_tails", "knot_b", (kx - 0.45, ky - 0.15, -0.3), (kx + 0.25, ky + 0.2, 0.3), mat="cord")
    for i, (z, ln) in enumerate(((-0.7, 3.4), (-0.25, 4.3), (0.3, 3.0), (0.75, 3.7))):
        rig.cube("knot_tails", f"tail_{i}", (kx - 0.12, ky - 0.3 - ln, z - 0.22), (kx + 0.12, ky - 0.3, z + 0.22), mat="cloth" if i % 2 == 0 else "cloth_dk")
    return rig


def part_extent(rig: Rig, bones: list[str], axis: int = 1) -> tuple[float, float]:
    lo, hi = 1e9, -1e9
    for b in bones:
        a, c = kit.rig_part_spans(rig, b, axis)
        lo, hi = min(lo, a), max(hi, c)
    return lo, hi


TITLE = "#2 残剑鞘背带 can_jian_qiao_bei_dai (Round 1)"

# 参考图拼版第三幅（背面，玩家高 920 px = 32 px）里人从图上量的：
#   柄尾最高 y=55，护手下沿 y≈185，鞘口铁箍上沿 y≈215，鞘尖 y≈705，头顶 y=90，脚底 y=1010
REF_PANEL = (730, 0, 1115, 1024)
REF_NUMBERS = {
    "hilt_top": 100.0 * (1010 - 55) / 920,        # 柄尾高度 / 身高
    "tip_height": 100.0 * (1010 - 705) / 920,     # 鞘尖离地 / 身高
    "hilt_span": 100.0 * (185 - 55) / 920,        # 柄（尾钉到护手）垂直跨度 / 身高
    "sheath_span": 100.0 * (705 - 215) / 920,     # 鞘垂直跨度 / 身高
    "total_span": 100.0 * (705 - 55) / 920,       # 整件垂直跨度 / 身高
}


def custom_proportion(rig: Rig, doc: dict):
    """比例对照：参考图第三幅 vs 模型穿在灰色玩家身上的背面渲染；数值按「占玩家身高 32 px 的百分比」。"""
    import json

    worn, _ = kit.worn_doc(doc, WORN_OFFSET)
    path = OUT_DIR / "_tmp_worn_prop.bbmodel"
    path.write_text(json.dumps(worn), encoding="utf-8")
    model_img, _ = render(path, yaw=0.0, pitch=0.0, size=900, bg=(40, 42, 46))   # 深底，裁图时才会带上灰色玩家
    path.unlink()
    ref_crop = Image.open(REF_PNG).convert("RGB").crop(REF_PANEL)
    lo_h, hi_h = part_extent(rig, ["hilt", "hilt_wrap"])
    lo_s, hi_s = part_extent(rig, ["scabbard", "scabbard_trim"])
    lo_a, hi_a = part_extent(rig, ["hilt", "hilt_wrap", "scabbard", "scabbard_trim", "blade_shards"])
    # 柄的垂直跨度只算到护手下沿（s=4.85 处），鞘跨度从鞘口铁箍上沿算
    guard_y = pt(4.85)[1]
    mouth_y = pt(6.15)[1]
    model = {
        "hilt_top": 100.0 * hi_a / 32.0,
        "tip_height": 100.0 * lo_a / 32.0,
        "hilt_span": 100.0 * (hi_h - guard_y) / 32.0,
        "sheath_span": 100.0 * (mouth_y - lo_s) / 32.0,
        "total_span": 100.0 * (hi_a - lo_a) / 32.0,
    }
    labels = {
        "hilt_top": "柄尾最高处 / 身高（>100% = 高过头顶）",
        "tip_height": "鞘尖离地 / 身高",
        "hilt_span": "残柄垂直跨度（尾钉到护手）/ 身高",
        "sheath_span": "鞘垂直跨度（鞘口铁箍到鞘尖）/ 身高",
        "total_span": "整件垂直跨度 / 身高",
    }
    metrics = [(labels[k], REF_NUMBERS[k], model[k]) for k in labels]
    return ref_crop, model_img, metrics, 0.0


def main() -> None:
    import weapon_check

    kit.REVIEW_ROOT = GEAR_REVIEW_ROOT
    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
