#!/usr/bin/env python3
"""守家傀儡（niche_house_puppet）GeckoLib 骨骼 + 动画。

外形**一块都不改**：几何、UV、贴图全部取自已落地的带贴图版 `modelScript/models/niche_house_puppet_tex.bbmodel`（91 个立方体、
512×32 图集）。这里做两件事：**按关节分骨**、**从零写动画**（没有套任何已有动画）。

## 第 2 次返工（用户原话：「应该可以从台阶上下来移动」）

1. **底座单独成一根不动的骨 `base`，傀儡本体整个挂在 `puppet` 骨上**——`puppet` 的位移 / 转身就能让傀儡离开底座，
   底座原地不动。
2. **腿拆成大腿 / 小腿 / 脚三段**（`thigh_*` / `shin_*` / `foot_*`）：原来的 `leg_shin_*` 立方体在膝关节（y=4.0）处
   一刀切成两块（上半归大腿、下半归小腿，体积、外形、UV 比例都不变），所以能迈步、能屈膝。
3. **脊分两节**（`pelvis` 胯、`chest` 胸），爪每指两节（`claw_*` 指根、`claw_*_tip` 爪尖）。
4. 动画从 4 段扩到 8 段：dormant / wake（保持在底座上）、strike（在底座上）、**step_down（从底座台阶迈下来落地）**、
   **walk（地面上循环的沉重步态，阵核随步伐微亮）**、**strike_ground（地面上的扑击，从 walk 的第 0 帧衔接）**、
   **return（转身走回、迈上底座、转回 dormant 姿态）**、exhausted（阵核耗尽，改成真正的屈膝瘫倒）。

## 第 3 次返工（用户原话：「考虑到行走时底座会单独留在原地，建议分成两个部件」）

**底座拆成独立模型**：`niche_house_puppet_base`（只有 `base` 一根骨、没有动画，放在原地不动）；
`niche_house_puppet`（傀儡本体）的 geo 里**不再有 `base` 骨和 `root` 骨**，`puppet` 就是最顶层的总枢轴。
动画里的 `puppet` 位移（`GROUND` 等）都是**相对傀儡自身出生点（站在底座上的位置）的偏移**，底座是另一个实体，不在这份动画里。
预览 GIF 用一份临时合成的 geo（傀儡 + 静止底座，只给预览用，不交付）才看得到「傀儡走开、底座留在原地」。

## 骨骼（傀儡本体会动的 29 根；任务卡人形 / 傀儡 ≥ 28）

    puppet                          傀儡本体的总枢轴（最顶层）：位移 / 转身都动它
    ├─ thigh_l → shin_l → foot_l       thigh_r → shin_r → foot_r
    └─ pelvis                       胯（脊第 1 节）
       └─ chest                     胸（脊第 2 节）
          ├─ core                   胸口笼格 + 阵符石（阵核，缩放表现亮灭）
          ├─ spine                  背脊 + 五对木刺
          ├─ head → jaw             颅骨 + 颈、下颌
          └─ arm_l/r → forearm_l/r → claw_{l,r}_{0,1,2} → claw_{l,r}_{0,1,2}_tip

    niche_house_puppet_base：base（单骨，静止，独立 geo）

坐标：bbmodel 的正面在 +Z，GeckoLib 生物约定面朝 -Z，导出 geo 时把 z 取反（傀儡左右对称，不会出错）；动画里「向前」
是 -Z，「向上」+Y，转角单位度，位移单位 px。转角符号（实测）：位于枢轴**下方**的肢体（臂、腿）x 为正 = 向后摆，
为负 = 向前摆；位于枢轴**上方**的部分（胸、头）x 为正 = 向前倾。

**落地约定**：底座高 1.5px。傀儡站在底座上时 `puppet` 位移为 0；站在地面上时 `puppet` 位移 `GROUND = (0, -1.5, -9)`
（下沉 1.5、向前 9px，脚后跟离底座前沿 2.7px）。地面上的 walk / strike_ground 都带这个位移；底座始终留在原点。

产物写到 `modelScript/models/niche_house_puppet_anim/`（geo ×2 / animations / textures 套件，布局和 client 资源树一致），
**暂不装进 client 资源树、不接线**。

用法：
    python gen_niche_house_puppet_anim.py            # 写三件套 + 自检
    python gen_niche_house_puppet_anim.py --gif      # 另出全部 GIF 到 model-review/anim/niche_house_puppet/
"""

from __future__ import annotations

import argparse
import base64
import copy
import json
import math
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable, Dict, List, Optional, Tuple

REPO = Path(__file__).resolve().parents[2]
SOURCE = REPO / "modelScript" / "models" / "niche_house_puppet_tex.bbmodel"      # 带像素贴图的版本（平色版贴图只有几百字节）
OUT_ROOT = REPO / "modelScript" / "models" / "niche_house_puppet_anim"
GIF_DIR = Path(__file__).resolve().parents[1] / "out" / "review" / "anim" / "niche_house_puppet"
NAME = "niche_house_puppet"
BASE_NAME = "niche_house_puppet_base"
IDENTIFIER = f"geometry.bong.{NAME}"
BASE_IDENTIFIER = f"geometry.bong.{BASE_NAME}"
CLIP_PREFIX = f"animation.bong.{NAME}."

BASE_HEIGHT = 1.5
GROUND = (0.0, -BASE_HEIGHT, -9.0)          # 站在地面上时 puppet 骨的位移
KNEE_Y = 4.0                                  # 膝关节高度：leg_shin 立方体在这里切成大腿 / 小腿
MIN_MOVING_BONES = 28

# ── 骨架 ──────────────────────────────────────────────────────────────────

CLAW_X = {"l": (-7.95, -6.5, -5.05), "r": (5.05, 6.5, 7.95)}     # 三指爪各指的 x（臂中心 ∓6.5，指间距 1.45）
CLAW_BASE_END_Z = 3.5                                              # 指根 z 1.5..3.5，爪尖接在 3.5 之后

BONES: Dict[str, Tuple[Optional[str], Tuple[float, float, float]]] = {
    "puppet": (None, (0.0, 0.0, 0.0)),
    "pelvis": ("puppet", (0.0, 5.5, 0.0)),
    "chest": ("pelvis", (0.0, 6.0, 0.0)),
    "core": ("chest", (0.0, 9.0, 1.0)),
    "spine": ("chest", (0.0, 9.5, -3.5)),
    "head": ("chest", (0.0, 12.0, 0.0)),
    "jaw": ("head", (0.0, 13.0, -2.0)),
}
for side, sx in (("l", -1.0), ("r", 1.0)):
    BONES[f"thigh_{side}"] = ("puppet", (sx * 2.5, 5.0, 0.3))
    BONES[f"shin_{side}"] = (f"thigh_{side}", (sx * 2.5, KNEE_Y, 0.3))
    BONES[f"foot_{side}"] = (f"shin_{side}", (sx * 2.5, 2.8, 0.3))
    BONES[f"arm_{side}"] = ("chest", (sx * 6.5, 11.7, 0.0))
    BONES[f"forearm_{side}"] = (f"arm_{side}", (sx * 6.5, 7.2, 0.0))
    for k in range(3):
        BONES[f"claw_{side}_{k}"] = (f"forearm_{side}", (CLAW_X[side][k], 2.4, 1.5))
        BONES[f"claw_{side}_{k}_tip"] = (f"claw_{side}_{k}", (CLAW_X[side][k], 2.3, CLAW_BASE_END_Z))

def moving_bones() -> List[str]:
    """傀儡本体 geo 里的骨头全部会动（底座在另一个 geo 里，不算）。"""
    return list(BONES)


def is_base_cube(cube_name: str) -> bool:
    return cube_name.startswith("base_")


def bone_of(cube_name: str) -> str:
    """每个傀儡本体立方体归哪根骨头——只看名字（leg_shin 的拆分在 `split_shin`，拆出来的两块名字带后缀）。
    底座立方体不归傀儡的任何骨头（见 `is_base_cube`，它们进独立的底座 geo）。"""
    n = cube_name
    m = re.match(r"leg_foot_(l|r)$", n)
    if m:
        return f"foot_{m.group(1)}"
    m = re.match(r"leg_shin_(l|r)_calf$", n)
    if m:
        return f"shin_{m.group(1)}"
    m = re.match(r"leg_(shin|knee)_(l|r)(_thigh)?$", n)
    if m:
        return f"thigh_{m.group(2)}"
    if n.startswith(("hip_", "torso_bottom")):
        return "pelvis"
    if n.startswith("torso_"):
        return "chest"
    if n.startswith(("cage_", "rune_")):
        return "core"
    if n.startswith("spine_"):
        return "spine"
    if n.startswith("jaw"):
        return "jaw"
    if n.startswith(("neck", "skull_", "eye_socket_")):
        return "head"
    m = re.match(r"claw_(l|r)_(\d)(_tip)?$", n)
    if m:
        return f"claw_{m.group(1)}_{m.group(2)}" + ("_tip" if m.group(3) else "")
    m = re.match(r"(shoulder|arm_upper)_(l|r)$", n)
    if m:
        return f"arm_{m.group(2)}"
    m = re.match(r"(arm_elbow|arm_fore)_(l|r)$", n)
    if m:
        return f"forearm_{m.group(2)}"
    m = re.match(r"guard_(l|r)_", n)
    if m:
        return f"forearm_{m.group(1)}"
    raise ValueError(f"立方体 {cube_name} 没有归属的骨头")


# ── geo 导出 ──────────────────────────────────────────────────────────────


def _uv(face: str, rect: List[float]) -> dict:
    """bbmodel 面 UV [u0,v0,u1,v1] → Bedrock {uv, uv_size}（up / down 在 bbmodel 里是反向角，见 import_creature_geo）。"""
    a, b, c, d = rect
    if face in ("up", "down"):
        return {"uv": [c, d], "uv_size": [a - c, b - d]}
    return {"uv": [a, b], "uv_size": [c - a, d - b]}


def split_shin(element: dict) -> List[dict]:
    """把 `leg_shin_*` 在膝高（y=KNEE_Y）一刀切成大腿（上）+ 小腿（下）两块：体积、外形不变，UV 按高度比例切。

    竖直四面（东西南北）的 UV 矩形在高度方向上按比例分给上下两块（v0 在上）；上下面分别归大腿顶 / 小腿底，
    切出来的内部面复用原 UV（看不见）。"""
    lo, hi = element["from"], element["to"]
    frac_top = (hi[1] - KNEE_Y) / (hi[1] - lo[1])
    thigh, calf = copy.deepcopy(element), copy.deepcopy(element)
    thigh["name"], calf["name"] = element["name"] + "_thigh", element["name"] + "_calf"
    thigh["from"], calf["to"] = [lo[0], KNEE_Y, lo[2]], [hi[0], KNEE_Y, hi[2]]
    for face in ("north", "south", "east", "west"):
        u0, v0, u1, v1 = element["faces"][face]["uv"]
        cut = v0 + (v1 - v0) * frac_top
        thigh["faces"][face]["uv"] = [u0, v0, u1, cut]
        calf["faces"][face]["uv"] = [u0, cut, u1, v1]
    return [thigh, calf]


def expanded_elements(bb: dict) -> List[dict]:
    out: List[dict] = []
    for element in bb["elements"]:
        if re.match(r"leg_shin_(l|r)$", element["name"]):
            out.extend(split_shin(element))
        else:
            out.append(element)
    return out


def _geo_cube(element: dict) -> dict:
    lo, hi = element["from"], element["to"]
    faces = {}
    for face, data in element["faces"].items():
        out_face = {"north": "south", "south": "north"}.get(face, face)          # z 取反后南北互换
        faces[out_face] = _uv(face, data["uv"])
    return {
        "origin": [lo[0], lo[1], -hi[2]],                                         # z 取反：[-hi_z, -lo_z]
        "size": [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]],
        "uv": faces,
    }


def _geo_document(bb: dict, identifier: str, bones: List[dict]) -> dict:
    width, height = bb["resolution"]["width"], bb["resolution"]["height"]
    return {
        "format_version": "1.12.0",
        "minecraft:geometry": [{
            "description": {"identifier": identifier, "texture_width": width, "texture_height": height,
                            "visible_bounds_width": 4, "visible_bounds_height": 2, "visible_bounds_offset": [0, 1, 0]},
            "bones": bones,
        }],
    }


def build_geo(bb: dict) -> dict:
    """傀儡本体 geo：没有底座、没有 base / root 骨，`puppet` 是最顶层。"""
    by_bone: Dict[str, List[dict]] = {name: [] for name in BONES}
    for element in expanded_elements(bb):
        if not is_base_cube(element["name"]):
            by_bone[bone_of(element["name"])].append(_geo_cube(element))
    bones = []
    for name, (parent, pivot) in BONES.items():
        bone = {"name": name, "pivot": [pivot[0], pivot[1], -pivot[2]]}
        if parent:
            bone["parent"] = parent
        if by_bone[name]:
            bone["cubes"] = by_bone[name]
        bones.append(bone)
    return _geo_document(bb, IDENTIFIER, bones)


def build_base_geo(bb: dict) -> dict:
    """底座独立 geo：只有 `base` 一根骨，不带动画，放在原地不动。"""
    cubes = [_geo_cube(e) for e in bb["elements"] if is_base_cube(e["name"])]
    return _geo_document(bb, BASE_IDENTIFIER, [{"name": "base", "pivot": [0.0, 0.0, 0.0], "cubes": cubes}])


def build_preview_geo(geo: dict, base_geo: dict) -> dict:
    """只给预览 GIF 用：傀儡 + 静止底座合成一份 geo（底座骨没有关键帧，所以留在原地）。不交付、不进资源树。"""
    merged = copy.deepcopy(geo)
    merged["minecraft:geometry"][0]["bones"] += copy.deepcopy(base_geo["minecraft:geometry"][0]["bones"])
    return merged


# ── 动画 ──────────────────────────────────────────────────────────────────
# 姿态 = {骨: {"rot": [x,y,z]°, "pos": [x,y,z]px, "scale": [x,y,z]}}，没写的骨 / 通道取静止值。
# 生成 JSON 时：用到的每根骨的每个通道在**所有时间点都写一个关键帧**
# （循环段首尾同值，不会被插值衰减回默认；一次性段末帧就是停留姿态）。

Pose = Dict[str, Dict[str, List[float]]]
REST = {"rot": [0.0, 0.0, 0.0], "pos": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]}
CHANNEL = {"rot": "rotation", "pos": "position", "scale": "scale"}


def merge(*poses: Pose) -> Pose:
    out: Pose = {}
    for p in poses:
        for bone, channels in p.items():
            out.setdefault(bone, {}).update(channels)
    return out


def both(arm: str, **values) -> Pose:
    """左右对称的姿态：左 = 参数原值，右 = 转角的 y / z 分量取反。"""
    out: Pose = {}
    for side, flip in (("l", 1.0), ("r", -1.0)):
        channels = {}
        for ch, vec in values.items():
            channels[ch] = [vec[0], vec[1] * flip, vec[2] * flip] if ch == "rot" else list(vec)
        out[arm.format(side=side)] = channels
    return out


def curl(side: str, deg: float, spread: float = 0.0, tip: Optional[float] = None) -> Pose:
    """三指爪：指根 x 向弯曲 deg°，爪尖再弯 tip°（缺省 = deg 的一半）；外侧两指再向外 / 内侧各张 spread°（z 向，左右镜像）。"""
    sign = -1.0 if side == "l" else 1.0
    tip_deg = deg * 0.5 if tip is None else tip
    out: Pose = {}
    for k, z in ((0, sign * -spread), (1, 0.0), (2, sign * spread)):
        out[f"claw_{side}_{k}"] = {"rot": [deg, 0.0, z]}
        out[f"claw_{side}_{k}_tip"] = {"rot": [tip_deg, 0.0, 0.0]}
    return out


def arms(shoulder: Tuple[float, float, float], elbow: Tuple[float, float, float], claw: float, spread: float) -> Pose:
    """两臂同姿态（z 向外展自动镜像）+ 爪。"""
    out = merge(both("arm_{side}", rot=shoulder), both("forearm_{side}", rot=elbow))
    for side in ("l", "r"):
        out = merge(out, curl(side, claw, spread))
    return out


def spine(rot: Tuple[float, float, float] = (0.0, 0.0, 0.0), pos: Tuple[float, float, float] = (0.0, 0.0, 0.0)) -> Pose:
    """脊两节：转角 35% 给胯、65% 给胸（合起来比单骨更像一条弯曲的脊）；位移给胯。"""
    return {"pelvis": {"rot": [v * 0.35 for v in rot], "pos": list(pos)}, "chest": {"rot": [v * 0.65 for v in rot]}}


def leg(side: str, thigh: float, shin: float, foot: float) -> Pose:
    """一条腿三段的 x 转角（大腿 / 脚：正 = 向后，负 = 向前；小腿 正 = 向后折）。"""
    return {f"thigh_{side}": {"rot": [thigh, 0.0, 0.0]}, f"shin_{side}": {"rot": [shin, 0.0, 0.0]},
            f"foot_{side}": {"rot": [foot, 0.0, 0.0]}}


def legs(l: Tuple[float, float, float], r: Tuple[float, float, float]) -> Pose:
    return merge(leg("l", *l), leg("r", *r))


def puppet(pos: Tuple[float, float, float] = (0.0, 0.0, 0.0), yaw: float = 0.0) -> Pose:
    return {"puppet": {"pos": list(pos), "rot": [0.0, yaw, 0.0]}}


def core(scale: float) -> Pose:
    return {"core": {"scale": [scale, scale, scale]}}


def _fmt(t: float) -> str:
    return "0.0" if not t else f"{t:.3f}".rstrip("0").rstrip(".")


def build_clip(length: float, loop: bool, keys: List[Tuple[float, Pose]]) -> dict:
    bones_used = sorted({bone for _, p in keys for bone in p})
    bones: dict = {}
    for bone in bones_used:
        channels_used = sorted({ch for _, p in keys for ch in p.get(bone, {})})
        bones[bone] = {CHANNEL[ch]: {_fmt(t): p.get(bone, {}).get(ch, REST[ch]) for t, p in keys} for ch in channels_used}
    clip = {"animation_length": length, "bones": bones}
    if loop:
        clip["loop"] = True
    return clip


# 站姿（准备攻击 / 激活完成）：头抬正、双臂微外展、爪半张，两腿并直
READY = merge(
    spine((-2.0, 0.0, 0.0)), {"head": {"rot": [-4.0, 0.0, 0.0]}},
    arms((6.0, 0.0, 10.0), (-14.0, 0.0, 4.0), 8.0, 10.0),
    legs((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)), core(1.0),
)
# 沉睡姿态：垂头含胸，双臂内收下垂，爪蜷着，阵核暗淡
SLUMP = merge(
    spine((8.0, 0.0, 0.0), (0.0, -0.5, 0.0)),
    {"head": {"rot": [22.0, 0.0, 0.0]}, "jaw": {"rot": [3.0, 0.0, 0.0]}, "spine": {"rot": [5.0, 0.0, 0.0]}},
    arms((10.0, 0.0, -4.0), (-6.0, 0.0, -2.0), 28.0, 0.0),
    legs((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)), core(0.9),
)
GROUND_READY = merge(READY, puppet(GROUND))


def dormant() -> dict:
    """待机（循环 4s，在底座上）：呼吸式的微起伏。身体慢慢上下、头随呼吸点一点、爪尖偶尔蜷一下、阵核暗暗明灭。
    每根骨头的节拍错开，不会读成「整体一起晃」。"""
    def breath(body_y: float, head_x: float, core_s: float, claw: float, sway: float) -> Pose:
        return merge(
            SLUMP, spine((8.0, 0.0, 0.0), (0.0, body_y, 0.0)), {"head": {"rot": [head_x, 0.0, 0.0]}}, core(core_s),
            arms((10.0 + sway, 0.0, -4.0), (-6.0 - sway, 0.0, -2.0), claw, 0.0),
        )

    keys = [(0.0, breath(-0.50, 22.0, 0.90, 28.0, 0.0)), (1.2, breath(-0.28, 20.6, 0.97, 24.0, 1.2)),
            (2.0, breath(-0.30, 20.9, 0.99, 24.5, 1.4)), (3.2, breath(-0.46, 22.4, 0.92, 31.0, -0.4)),
            (4.0, breath(-0.50, 22.0, 0.90, 28.0, 0.0))]
    return build_clip(4.0, True, keys)


def wake() -> dict:
    """激活（一次 1.4s，在底座上）：阵核先亮一下，然后**关节逐节张开**——脊背 → 肩 → 肘 → 爪 → 头抬起 → 下颌张合，站定。"""
    t0 = SLUMP
    flash = merge(t0, core(1.35))
    spine_up = merge(flash, spine((2.0, 0.0, 0.0), (0.0, -0.1, 0.0)), {"spine": {"rot": [0.0, 0.0, 0.0]}}, core(1.1))
    shoulders = merge(spine_up, both("arm_{side}", rot=(0.0, 0.0, 16.0)))
    elbows = merge(shoulders, both("forearm_{side}", rot=(-22.0, 0.0, 8.0)))
    claws = merge(elbows, curl("l", 0.0, 22.0), curl("r", 0.0, 22.0))
    head_up = merge(claws, {"head": {"rot": [-6.0, 0.0, 0.0]}, "jaw": {"rot": [3.0, 0.0, 0.0]}}, spine((-3.0, 0.0, 0.0)))
    jaw_open = merge(head_up, {"jaw": {"rot": [24.0, 0.0, 0.0]}, "head": {"rot": [-9.0, 0.0, 0.0]}})
    keys = [(0.0, t0), (0.14, flash), (0.34, spine_up), (0.54, shoulders), (0.74, elbows), (0.92, claws),
            (1.08, head_up), (1.2, jaw_open), (1.4, READY)]
    return build_clip(1.4, False, keys)


def _strike_keys(start: Pose, rest: Pose, offset: Tuple[float, float, float],
                 crouch_leg: Pose, lunge_leg: Pose) -> List[Tuple[float, Pose]]:
    ox, oy, oz = offset
    windup = merge(
        rest, puppet((ox, oy - 0.7, oz + 0.8)), spine((-12.0, 0.0, 0.0)),
        {"head": {"rot": [-12.0, 0.0, 0.0]}, "jaw": {"rot": [26.0, 0.0, 0.0]}}, core(1.25), crouch_leg,
        arms((62.0, 0.0, 22.0), (22.0, 0.0, 6.0), -6.0, 26.0),     # 手臂在肩下面：x 为正 = 向后上举
    )
    lunge = merge(
        rest, puppet((ox, oy + 0.5, oz - 3.2)), spine((20.0, 0.0, 0.0)),
        {"head": {"rot": [10.0, 0.0, 0.0]}, "jaw": {"rot": [2.0, 0.0, 0.0]}}, core(1.1), lunge_leg,
        arms((-74.0, 0.0, -6.0), (-24.0, 0.0, -4.0), 26.0, 4.0),   # x 为负 = 向前上抓
    )
    hold = merge(lunge, puppet((ox, oy + 0.3, oz - 3.6)), spine((22.0, 0.0, 0.0)))
    return [(0.0, start), (0.2, windup), (0.34, lunge), (0.46, hold), (0.7, rest)]


def strike() -> dict:
    """触发攻击（一次 0.7s，在底座上）：下蹲后仰蓄势、双臂向后举起、下颌张开 → 猛地前扑，双臂合抓、下颌合上 → 停一拍 → 收回站姿。
    腿跟着屈：蓄势时两膝前顶、前扑时一条腿跨出。"""
    crouch = legs((-26.0, 46.0, -16.0), (-26.0, 46.0, -16.0))
    lunge = legs((-30.0, 22.0, -10.0), (22.0, 10.0, 14.0))
    return build_clip(0.7, False, _strike_keys(READY, READY, (0.0, 0.0, 0.0), crouch, lunge))


def strike_ground() -> dict:
    """地面上的扑击（一次 0.7s）：动作同 strike，但带地面位移 GROUND，起点是 walk 的第 0 帧（左脚在前着地），
    所以能从 walk 直接衔接；终点是站定的 GROUND_READY。"""
    crouch = legs((-30.0, 52.0, -18.0), (-14.0, 40.0, -14.0))
    lunge = legs((-34.0, 24.0, -12.0), (24.0, 12.0, 16.0))
    return build_clip(0.7, False, _strike_keys(walk_pose(0.0), GROUND_READY, GROUND, crouch, lunge))


def exhausted() -> dict:
    """阵核耗尽瘫倒（一次 2.2s，在底座上）：阵核剧烈闪烁、越闪越暗 → **膝盖弯下去**（大腿前顶、小腿向后折、脚掌翻起）、
    上身前倒 → 头垂下、下颌松开、双臂耷拉 → 轻微回弹后彻底不动。"""
    flicker1 = merge(READY, core(0.55))
    flicker2 = merge(READY, core(1.25), spine((-3.0, 0.0, 0.0)))
    flicker3 = merge(READY, core(0.4), {"pelvis": {"rot": [0.0, 0.0, 1.0]}})
    sag = merge(
        READY, core(0.6), spine((10.0, 0.0, 2.0), (0.0, -0.6, 0.0)), {"head": {"rot": [14.0, 0.0, 0.0]}},
        legs((-26.0, 40.0, -14.0), (-22.0, 34.0, -12.0)),
        arms((14.0, 0.0, 12.0), (-4.0, 0.0, 6.0), 14.0, 8.0),
    )
    fall = merge(
        core(0.3), spine((26.0, 0.0, 6.0), (0.0, -1.5, -0.6)),
        {"head": {"rot": [40.0, 0.0, 0.0]}, "jaw": {"rot": [16.0, 0.0, 0.0]}, "spine": {"rot": [10.0, 0.0, 0.0]}},
        legs((-62.0, 98.0, -34.0), (-54.0, 88.0, -30.0)),
        arms((26.0, 0.0, 22.0), (14.0, 0.0, 8.0), 44.0, 6.0),
    )
    bounce = merge(fall, spine((22.0, 0.0, 5.0), (0.0, -1.3, -0.5)), {"head": {"rot": [34.0, 0.0, 0.0]}, "jaw": {"rot": [10.0, 0.0, 0.0]}}, core(0.36))
    rest = merge(fall, spine((27.0, 0.0, 6.0), (0.0, -1.6, -0.6)), {"head": {"rot": [42.0, 0.0, 0.0]}, "jaw": {"rot": [18.0, 0.0, 0.0]}}, core(0.2))
    keys = [(0.0, READY), (0.12, flicker1), (0.26, flicker2), (0.4, flicker3), (0.55, merge(READY, core(0.9))),
            (0.7, flicker1), (0.95, sag), (1.35, fall), (1.6, bounce), (2.2, rest)]
    return build_clip(2.2, False, keys)


# ── 行走 ──────────────────────────────────────────────────────────────────

WALK_PERIOD = 1.6          # 沉重：一个周期（左右各一步）1.6s


def walk_pose(phase: float, offset: Tuple[float, float, float] = GROUND, yaw: float = 0.0) -> Pose:
    """沉重步态在 phase（0~1，一个周期）处的姿态；phase 0 = 左脚在前着地。

    - 腿：大腿前后摆 ±34°，摆动腿抬起时小腿折 62°、脚掌翻起，两条腿交替清楚可见；
    - 身体：落脚时下沉、摆腿时抬起（起伏 0.8px），重心随支撑腿左右移 0.8px，胯左右倾 ±5°、胸反向拧 ±6°（重物步态）；
    - 手臂和同侧腿反向摆 26°，前臂随之屈伸；头随步点一点；
    - 阵核在每次落脚时亮一下（1.0 → 1.15，随步伐微亮）。"""
    a = 2 * math.pi * phase
    lift_l = max(0.0, -math.sin(a))          # 左腿在 phase 0.5~1 向前摆：抬起
    lift_r = max(0.0, math.sin(a))           # 右腿在 phase 0~0.5 向前摆
    bob = -0.4 * math.cos(2 * a)             # 落脚（phase 0 / 0.5）最低，摆腿中段最高，起伏共 0.8px
    sway = -0.8 * math.sin(a)                # 重心移向支撑腿（右腿摆动时移向左）
    pulse = 1.0 + 0.15 * max(0.0, math.cos(2 * a)) ** 2
    return merge(
        puppet((offset[0] + sway, offset[1] + bob, offset[2]), yaw),
        legs((-34.0 * math.cos(a), 62.0 * lift_l, -16.0 * lift_l + 6.0 * max(0.0, math.cos(a))),
             (34.0 * math.cos(a), 62.0 * lift_r, -16.0 * lift_r + 6.0 * max(0.0, -math.cos(a)))),
        {"pelvis": {"rot": [-1.0, 0.0, 5.0 * math.sin(a)]}, "chest": {"rot": [-1.0, 6.0 * math.cos(a), -3.0 * math.sin(a)]},
         "head": {"rot": [-4.0 + 2.0 * math.cos(2 * a), -2.0 * math.cos(a), 0.0]}, "jaw": {"rot": [2.0, 0.0, 0.0]}},
        {"arm_l": {"rot": [26.0 * math.cos(a), 0.0, 9.0]}, "arm_r": {"rot": [-26.0 * math.cos(a), 0.0, -9.0]},
         "forearm_l": {"rot": [-16.0 - 8.0 * math.cos(a), 0.0, 3.0]}, "forearm_r": {"rot": [-16.0 + 8.0 * math.cos(a), 0.0, -3.0]}},
        curl("l", 12.0, 6.0), curl("r", 12.0, 6.0), core(pulse),
    )


def walk() -> dict:
    """行走（循环 1.6s，地面上）：沉重的傀儡步态，每 0.1s 一个关键帧（16 个）；首尾同值。"""
    keys = [(i * 0.1, walk_pose(i / 16.0)) for i in range(16)]
    keys.append((WALK_PERIOD, keys[0][1]))
    return build_clip(WALK_PERIOD, True, keys)


def step_down() -> dict:
    """从底座台阶迈下来（一次 2.3s）：起点是 wake 的 READY（在底座上）。
    ① 重心移到左腿（身体略右后仰、微下沉）→ ② **右腿抬起**（大腿前提 52°、小腿折 70°，身子仍在底座上方，微升）
    → ③ 右脚向前探出底座前沿（重心前移、上身前倾）→ ④ **整体下沉一个底座高度（1.5px）**，右脚落地，
    左腿被拖在台阶上向后伸 → ⑤ **左腿抬起跟下**（小腿折起绕过前沿）→ ⑥ 左脚落地并拢，起身站稳。
    终点是 GROUND_READY（puppet 位移 = GROUND）。"""
    shift = merge(READY, puppet((-0.7, -0.15, 0.0)), spine((-3.0, 0.0, 4.0)),
                  legs((2.0, 4.0, 0.0), (0.0, 6.0, 0.0)),
                  {"arm_l": {"rot": [8.0, 0.0, 10.0]}, "arm_r": {"rot": [-8.0, 0.0, -10.0]}})
    lift_r = merge(READY, puppet((-0.5, 0.25, -0.8)), spine((2.0, 0.0, 3.0)),
                   legs((4.0, 6.0, 0.0), (-52.0, 70.0, -12.0)),
                   {"arm_l": {"rot": [16.0, 0.0, 10.0]}, "arm_r": {"rot": [-16.0, 0.0, -10.0]}})
    reach_r = merge(READY, puppet((0.0, 0.0, -3.4)), spine((14.0, 0.0, 0.0)),
                    legs((16.0, 14.0, 4.0), (-48.0, 26.0, -30.0)),
                    {"arm_l": {"rot": [20.0, 0.0, 10.0]}, "arm_r": {"rot": [-20.0, 0.0, -10.0]}})
    drop_r = merge(READY, puppet((0.0, -1.5, -6.6)), spine((12.0, 0.0, 0.0)),
                   legs((34.0, 6.0, 10.0), (-28.0, 20.0, -2.0)),
                   {"arm_l": {"rot": [4.0, 0.0, 10.0]}, "arm_r": {"rot": [-4.0, 0.0, -10.0]}}, core(1.14))
    lift_l = merge(READY, puppet((0.0, -1.5, -7.6)), spine((8.0, 0.0, 0.0)),
                   legs((-50.0, 72.0, -14.0), (12.0, 4.0, 2.0)),
                   {"arm_l": {"rot": [-12.0, 0.0, 10.0]}, "arm_r": {"rot": [12.0, 0.0, -10.0]}})
    land_l = merge(READY, puppet((0.0, -1.5, -9.0)), spine((4.0, 0.0, 0.0)),
                   legs((-16.0, 14.0, 0.0), (14.0, 6.0, 4.0)), core(1.12))
    keys = [(0.0, READY), (0.3, shift), (0.7, lift_r), (1.0, reach_r), (1.3, drop_r), (1.65, lift_l), (1.95, land_l),
            (2.3, GROUND_READY)]
    return build_clip(2.3, False, keys)


def return_clip() -> dict:
    """走回底座（一次 3.9s）：起点 GROUND_READY → 转身 180°（头先转）→ 沉重地走回底座前沿 → 右脚、左脚依次迈上台阶（身子升回
    原位）→ 站定后转回正面（再转 180°，累计 360°）、含胸垂头、阵核暗下去 → 终点就是 dormant 的第 0 帧（在底座上）。"""
    turn_head = merge(GROUND_READY, {"head": {"rot": [-4.0, 70.0, 0.0]}}, spine((-2.0, 30.0, 0.0)), puppet(GROUND, 40.0))
    turned = merge(walk_pose(0.0, GROUND, 180.0), {"head": {"rot": [-4.0, 0.0, 0.0]}})
    walks = [walk_pose(i / 8.0, (0.0, -1.5, -9.0 + 0.9 * i), 180.0) for i in range(6)]
    step_r = merge(READY, puppet((0.0, -1.0, -3.2), 180.0), spine((8.0, 0.0, 0.0)), legs((6.0, 4.0, 0.0), (-52.0, 58.0, -24.0)),
                   {"arm_l": {"rot": [8.0, 0.0, 10.0]}, "arm_r": {"rot": [-8.0, 0.0, -10.0]}})
    up_r = merge(READY, puppet((0.0, -0.1, -2.0), 180.0), spine((10.0, 0.0, 0.0)), legs((20.0, 8.0, 4.0), (-30.0, 22.0, -10.0)))
    step_l = merge(READY, puppet((0.0, 0.3, -0.8), 180.0), spine((6.0, 0.0, 0.0)), legs((-50.0, 56.0, -24.0), (18.0, 6.0, 4.0)))
    on_base = merge(READY, puppet((0.0, 0.0, 0.0), 180.0), spine((4.0, 0.0, 0.0)), legs((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)), core(1.05))
    # 转回正面时身体保持 READY 站姿（竖直、不含胸），只转 puppet 的 yaw；转完站定后再原地收成 dormant 的含胸垂头姿态，
    # 避免「边转身边含胸」把侧面的含胸姿态叠在转角上，读成侧倾 / 压扁。
    turning = merge(READY, puppet((0.0, 0.0, 0.0), 270.0), core(1.05))
    facing_front = merge(READY, puppet((0.0, 0.0, 0.0), 360.0), core(1.05))
    dormant_end = merge(SLUMP, puppet((0.0, 0.0, 0.0), 360.0), core(0.9))
    keys = [(0.0, GROUND_READY), (0.3, turn_head), (0.6, turned), (0.8, walks[1]), (1.0, walks[2]), (1.2, walks[3]),
            (1.4, walks[4]), (1.6, walks[5]), (1.85, step_r), (2.1, up_r), (2.35, step_l), (2.6, on_base),
            (2.95, turning), (3.3, facing_front), (3.9, dormant_end)]
    return build_clip(3.9, False, keys)


CLIPS: Dict[str, Callable[[], dict]] = {
    "dormant": dormant, "wake": wake, "strike": strike, "step_down": step_down, "walk": walk,
    "strike_ground": strike_ground, "return": return_clip, "exhausted": exhausted,
}


def build_animations() -> dict:
    return {"format_version": "1.8.0", "animations": {f"{CLIP_PREFIX}{name}": fn() for name, fn in CLIPS.items()}}


# ── 门禁 ──────────────────────────────────────────────────────────────────


def check_rig(bb: dict, geo: dict, base_geo: dict) -> None:
    """外形不变：傀儡 + 底座两个 geo 合起来，立方体体积、整体包围盒都和原 bbmodel 一致（个数 = 原个数 + 每条腿拆出的 1 块）；
    傀儡 geo 里没有底座立方体、没有 base / root 骨，`puppet` 是唯一的顶层骨，其余骨头全挂在它下面；
    父骨存在、无环；骨头 ≥ 28；腿三段都有几何；底座 geo 只有一根无父的 base 骨。"""
    base_bones = base_geo["minecraft:geometry"][0]["bones"]
    assert [b["name"] for b in base_bones] == ["base"], f"底座 geo 的骨头应只有 base: {[b['name'] for b in base_bones]}"
    assert base_bones[0].get("cubes") and base_bones[0].get("parent") is None, "base 骨要有几何且没有父骨"
    bones = geo["minecraft:geometry"][0]["bones"]
    by_name = {b["name"]: b for b in bones}
    assert "base" not in by_name and "root" not in by_name, "傀儡 geo 里不该再有 base / root 骨（底座已拆成独立模型）"
    for b in bones:
        assert b.get("parent") is None or b["parent"] in by_name, f"{b['name']} 的父骨 {b.get('parent')} 不存在"
        seen, cur = {b["name"]}, b.get("parent")
        while cur is not None:
            assert cur not in seen, f"骨骼层级有环: {b['name']}"
            seen.add(cur)
            cur = by_name[cur].get("parent")
    puppet_cubes = [c for b in bones for c in b.get("cubes", [])]
    cubes = puppet_cubes + base_bones[0]["cubes"]
    base_count = sum(1 for e in bb["elements"] if is_base_cube(e["name"]))
    assert len(base_bones[0]["cubes"]) == base_count, f"底座 geo 立方体数 {len(base_bones[0]['cubes'])} != 底座原个数 {base_count}"
    assert len(cubes) == len(bb["elements"]) + 2, f"立方体数 {len(cubes)} != 原 {len(bb['elements'])} + 2（两条腿各拆一块）"
    vol_geo = sum(c["size"][0] * c["size"][1] * c["size"][2] for c in cubes)
    vol_bb = sum((e["to"][0] - e["from"][0]) * (e["to"][1] - e["from"][1]) * (e["to"][2] - e["from"][2]) for e in bb["elements"])
    assert abs(vol_geo - vol_bb) < 1e-6, f"体积 {vol_geo} != 原 {vol_bb}"
    lo = [min(c["origin"][i] for c in cubes) for i in range(3)]
    hi = [max(c["origin"][i] + c["size"][i] for c in cubes) for i in range(3)]
    blo = [min(e["from"][i] for e in bb["elements"]) for i in range(3)]
    bhi = [max(e["to"][i] for e in bb["elements"]) for i in range(3)]
    assert lo[0] == blo[0] and hi[0] == bhi[0] and lo[1] == blo[1] and hi[1] == bhi[1], "x/y 包围盒变了"
    assert abs(lo[2] + bhi[2]) < 1e-9 and abs(hi[2] + blo[2]) < 1e-9, "z 包围盒不是原 bbmodel 的镜像"
    assert len(by_name) >= MIN_MOVING_BONES, f"会动的骨头只有 {len(by_name)} 根，要 ≥{MIN_MOVING_BONES}"
    assert by_name["puppet"].get("parent") is None, "puppet 应是最顶层的骨"
    for name in by_name:
        cur = name
        while by_name[cur].get("parent") is not None:
            cur = by_name[cur]["parent"]
        assert cur == "puppet", f"{name} 没有挂在 puppet 下（傀儡本体要能整体离开底座）"
    for side in ("l", "r"):
        for part in ("thigh", "shin", "foot"):
            assert by_name[f"{part}_{side}"].get("cubes"), f"{part}_{side} 没有几何（腿没拆成三段）"


def _lerp_pose(clip: dict, t: float) -> Dict[Tuple[str, str], List[float]]:
    """clip 在 t 秒的姿态（线性插值），键 = (骨, 通道)。"""
    out: Dict[Tuple[str, str], List[float]] = {}
    for bone, channels in clip["bones"].items():
        for channel, keys in channels.items():
            frames = sorted((float(k), v) for k, v in keys.items())
            value = frames[0][1]
            for i, (kt, kv) in enumerate(frames):
                if t < kt:
                    if i:
                        pt, pv = frames[i - 1]
                        value = [a + (b - a) * (t - pt) / (kt - pt) for a, b in zip(pv, kv)]
                    break
                value = kv
            out[(bone, channel)] = list(value)
    return out


def _same_pose(a: dict, b: dict, ta: float, tb: float, tol: float = 0.6) -> Optional[str]:
    """两段动画在各自的 ta / tb 处的姿态是否一致（缺省取静止值；rotation 的 y 轴按 360° 取模）；不一致返回第一处差异。"""
    pa, pb = _lerp_pose(a, ta), _lerp_pose(b, tb)
    rests = {"rotation": [0.0] * 3, "position": [0.0] * 3, "scale": [1.0] * 3}
    for key in sorted(set(pa) | set(pb)):
        channel = key[1]
        va, vb = pa.get(key, rests[channel]), pb.get(key, rests[channel])
        for axis, (x, y) in enumerate(zip(va, vb)):
            diff = abs(x - y)
            if channel == "rotation" and axis == 1:
                diff = min(diff % 360.0, 360.0 - diff % 360.0)
            if diff > tol:
                return f"{key[0]}.{channel}[{axis}]: {x:.2f} vs {y:.2f}"
    return None


def check_animations(animations: dict) -> None:
    """每段都有；循环段首尾同值；一次性段末帧在长度上；动画里没有底座骨；在底座上的段傀儡不离开出生点（位移相对傀儡自身）；
    段与段能接上：wake 末 = step_down 起，step_down 末 = return 起，return 末 = dormant 起，walk 起 = strike_ground 起。"""
    clips = animations["animations"]
    assert set(clips) == {f"{CLIP_PREFIX}{n}" for n in CLIPS}, f"动画段不对: {sorted(clips)}"
    for full, clip in clips.items():
        for bone, channels in clip["bones"].items():
            assert bone in BONES, f"{full}: 未知骨 {bone}（底座已拆出，动画里不许再有 base）"
            for channel, keys in channels.items():
                times = sorted(float(t) for t in keys)
                if clip.get("loop"):
                    first, last = keys[min(keys, key=float)], keys[max(keys, key=float)]
                    assert first == last, f"{full}.{bone}.{channel} 循环首尾不同值: {first} vs {last}"
                else:
                    assert abs(times[-1] - clip["animation_length"]) < 1e-6, f"{full}.{bone}.{channel} 末帧 {times[-1]} 不在动画长度上"

    def clip(name: str) -> dict:
        return clips[f"{CLIP_PREFIX}{name}"]

    for (a, ta, b, tb) in (("wake", None, "step_down", 0.0), ("step_down", None, "return", 0.0),
                           ("return", None, "dormant", 0.0), ("walk", 0.0, "strike_ground", 0.0)):
        end_a = clip(a)["animation_length"] if ta is None else ta
        diff = _same_pose(clip(a), clip(b), end_a, tb)
        assert diff is None, f"{a} → {b} 接不上: {diff}"
    for name in ("step_down", "walk", "return", "strike_ground"):
        assert "puppet" in clip(name)["bones"], f"{name} 没有 puppet 位移，傀儡离不开底座"
    # 在底座上的段：傀儡不能离开底座（x 偏移 ≤0.5，向前最多 4px 仍在底座上方，不许下沉到台阶外）
    for name in ("dormant", "wake", "strike", "exhausted"):
        track = clip(name)["bones"].get("puppet", {}).get("position", {})
        for t, v in track.items():
            assert abs(v[0]) <= 0.5 and -4.0 <= v[2] <= 1.0 and v[1] >= -1.0, f"{name}@{t} 傀儡离开了底座: {v}"
    last = _fmt(clip("step_down")["animation_length"])
    assert clip("step_down")["bones"]["puppet"]["position"][last] == list(GROUND), "step_down 末帧没落到 GROUND"


def self_test() -> None:
    bb = json.loads(SOURCE.read_text(encoding="utf-8"))
    geo, base_geo = build_geo(bb), build_base_geo(bb)
    check_rig(bb, geo, base_geo)
    animations = build_animations()
    check_animations(animations)
    print(f"  [OK] 傀儡骨架 {len(BONES)} 根（≥{MIN_MOVING_BONES}，无 base / root）+ 独立底座 geo（1 根骨）、"
          f"{len(bb['elements']) + 2} 个立方体（体积 / 包围盒与原 bbmodel 一致），{len(CLIPS)} 段动画、衔接门禁通过")

    def must_fail(name: str, fn) -> None:
        try:
            fn()
        except (AssertionError, ValueError, KeyError) as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e)[:56]}")
            return
        raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    def bone_of_geo(g: dict, name: str) -> dict:
        return next(b for b in g["minecraft:geometry"][0]["bones"] if b["name"] == name)

    broken = copy.deepcopy(geo)
    bone_of_geo(broken, "chest")["cubes"].pop()
    must_fail("丢一个立方体", lambda: check_rig(bb, broken, base_geo))
    no_shin = copy.deepcopy(geo)
    bone_of_geo(no_shin, "shin_l").pop("cubes")
    must_fail("小腿没有几何（腿没拆成三段）", lambda: check_rig(bb, no_shin, base_geo))
    detached = copy.deepcopy(geo)
    bone_of_geo(detached, "pelvis")["parent"] = None
    must_fail("胯没挂在 puppet 下", lambda: check_rig(bb, detached, base_geo))
    with_base = copy.deepcopy(geo)
    with_base["minecraft:geometry"][0]["bones"].append({"name": "base", "pivot": [0, 0, 0], "parent": "puppet",
                                                        "cubes": copy.deepcopy(base_geo["minecraft:geometry"][0]["bones"][0]["cubes"])})
    must_fail("傀儡 geo 里还留着 base 骨", lambda: check_rig(bb, with_base, base_geo))
    hollow_base = copy.deepcopy(base_geo)
    hollow_base["minecraft:geometry"][0]["bones"][0]["cubes"].pop()
    must_fail("底座 geo 少一个立方体", lambda: check_rig(bb, geo, hollow_base))
    bad_loop = copy.deepcopy(animations)
    track = bad_loop["animations"][f"{CLIP_PREFIX}walk"]["bones"]["thigh_l"]["rotation"]
    track[max(track, key=float)] = [99.0, 0.0, 0.0]
    must_fail("循环段末帧不等于首帧", lambda: check_animations(bad_loop))
    bad_end = copy.deepcopy(animations)
    bad_end["animations"][f"{CLIP_PREFIX}wake"]["animation_length"] = 5.0
    must_fail("一次性段末帧不在长度上", lambda: check_animations(bad_end))
    base_moves = copy.deepcopy(animations)
    base_moves["animations"][f"{CLIP_PREFIX}walk"]["bones"]["base"] = {"position": {"0.0": [0, 0, 0], "1.6": [0, 0, 0]}}
    must_fail("动画里还有底座骨", lambda: check_animations(base_moves))
    cut = copy.deepcopy(animations)
    cut["animations"][f"{CLIP_PREFIX}step_down"]["bones"]["puppet"]["position"]["2.3"] = [0.0, 0.0, 0.0]
    must_fail("step_down 没落到地面", lambda: check_animations(cut))
    broken_chain = copy.deepcopy(animations)
    broken_chain["animations"][f"{CLIP_PREFIX}return"]["bones"]["head"]["rotation"]["3.9"] = [40.0, 0.0, 0.0]
    must_fail("return 末帧接不上 dormant", lambda: check_animations(broken_chain))
    broken_walk = copy.deepcopy(animations)
    broken_walk["animations"][f"{CLIP_PREFIX}strike_ground"]["bones"]["thigh_l"]["rotation"]["0.0"] = [-5.0, 0.0, 0.0]
    must_fail("strike_ground 起点和 walk 第 0 帧不一致", lambda: check_animations(broken_walk))
    wandering = copy.deepcopy(animations)
    wandering["animations"][f"{CLIP_PREFIX}wake"]["bones"]["puppet"] = {"position": {"0.0": [0, 0, -8.0], "1.4": [0, 0, -8.0]}}
    must_fail("底座上的段傀儡走出了底座", lambda: check_animations(wandering))
    must_fail("没有归属的立方体", lambda: bone_of("mystery_cube"))


def write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


def write_assets() -> Path:
    bb = json.loads(SOURCE.read_text(encoding="utf-8"))
    geo, base_geo, animations = build_geo(bb), build_base_geo(bb), build_animations()
    check_rig(bb, geo, base_geo)
    check_animations(animations)
    write_json(OUT_ROOT / "geo" / f"{NAME}.geo.json", geo)
    write_json(OUT_ROOT / "geo" / f"{BASE_NAME}.geo.json", base_geo)
    write_json(OUT_ROOT / "animations" / f"{NAME}.animation.json", animations)
    texture = base64.b64decode(bb["textures"][0]["source"].split(",", 1)[1])
    texture_dir = OUT_ROOT / "textures" / "entity" / "fauna"
    texture_dir.mkdir(parents=True, exist_ok=True)
    for name in (NAME, BASE_NAME):                    # 底座和傀儡共用同一张图集
        (texture_dir / f"{name}.png").write_bytes(texture)
    return OUT_ROOT


def render_gifs(size: int) -> None:
    """GIF 用临时合成的预览资源（傀儡 + 静止底座），看得到傀儡走开、底座留在原地。不交付。"""
    bb = json.loads(SOURCE.read_text(encoding="utf-8"))
    preview_root = Path(tempfile.mkdtemp(prefix="house_puppet_preview_"))
    write_json(preview_root / "geo" / f"{NAME}.geo.json", build_preview_geo(build_geo(bb), build_base_geo(bb)))
    write_json(preview_root / "animations" / f"{NAME}.animation.json", build_animations())
    texture_dir = preview_root / "textures" / "entity" / "fauna"
    texture_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy(OUT_ROOT / "textures" / "entity" / "fauna" / f"{NAME}.png", texture_dir / f"{NAME}.png")
    GIF_DIR.mkdir(parents=True, exist_ok=True)
    cmd = [sys.executable, str(REPO / "modelScript" / "tools" / "creature_anim_frames.py"), "--name", NAME,
           "--assets", str(preview_root), "--gif", "--size", str(size), "--out", str(GIF_DIR), "--facing=-z"]
    try:
        subprocess.run(cmd, check=True, cwd=REPO / "modelScript" / "tools")
    finally:
        shutil.rmtree(preview_root, ignore_errors=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gif", action="store_true", help="另出全部 GIF")
    parser.add_argument("--size", type=int, default=440)
    args = parser.parse_args()
    print("gen_niche_house_puppet_anim 自检…")
    self_test()
    print(f"[assets] {write_assets().relative_to(REPO)}")
    if args.gif:
        render_gifs(args.size)


if __name__ == "__main__":
    main()
