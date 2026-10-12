#!/usr/bin/env python3
"""守家傀儡（niche_house_puppet）GeckoLib 骨骼 + 4 段动画：dormant 待机 / wake 激活 / strike 触发攻击 / exhausted 阵核耗尽瘫倒。

外形**一块都不改**：几何、UV、贴图全部取自已落地的 `modelScript/models/niche_house_puppet.bbmodel`（91 个立方体、
512×32 图集），这里只做两件事：

1. **重组分组**：原 bbmodel 的分组是 base / legs / torso / core / arms / head / spine 七组，粒度只够分部件，不够做动画
   （比如整条手臂是一组，没法让「肩 → 肘 → 爪」逐节张开）。这里按关节重新分骨，**每个立方体归哪根骨头只由名字决定**（`bone_of`）：

        root
        ├─ base                       底座（不动）
        ├─ leg_l / leg_r              两条矮腿，枢轴在腿根（膝上）
        └─ body                       胯、躯干；枢轴在胯（0,6,0），整个上身绕它前后倾
           ├─ core                    胸口笼格 + 阵符石（阵核），枢轴在石符中心，缩放表现「阵核亮灭」
           ├─ spine                   背脊 + 五对木刺
           ├─ head                    颈 + 颅骨，枢轴在颈根
           │  └─ jaw                  下颌，枢轴在颌后下角
           ├─ arm_l → forearm_l → claw_l_0/1/2    肩 / 上臂 → 肘 / 前臂 / 护板 → 三指爪（枢轴在指根）
           └─ arm_r → forearm_r → claw_r_0/1/2

2. **写动画**：从零定关键帧（没有套任何已有的动画），每段一个 `animation.bong.niche_house_puppet.<clip>`。

坐标：bbmodel 的正面在 +Z，GeckoLib 生物约定面朝 -Z，所以导出 geo 时把 z 取反（傀儡左右对称，不会出错）；
动画里的「向前」就是 -Z，「向上」+Y，转角单位度，位移单位 px。

产物写到 `modelScript/models/niche_house_puppet_anim/`（geo / animations / textures 三件套，布局和 client 资源树一致），
**暂不装进 client 资源树、不接线**（装进去要连带同步资源包 sha1，由调度决定何时装）。

用法：
    python gen_niche_house_puppet_anim.py            # 写三件套 + 自检
    python gen_niche_house_puppet_anim.py --gif      # 另出 4 段 GIF 到 model-review/anim/niche_house_puppet/
"""

from __future__ import annotations

import argparse
import base64
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

REPO = Path(__file__).resolve().parents[2]
SOURCE = REPO / "modelScript" / "models" / "niche_house_puppet.bbmodel"
OUT_ROOT = REPO / "modelScript" / "models" / "niche_house_puppet_anim"
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim/niche_house_puppet")
NAME = "niche_house_puppet"
IDENTIFIER = f"geometry.bong.{NAME}"
CLIP_PREFIX = f"animation.bong.{NAME}."

# ── 骨架：名字 → (父骨, 枢轴 (x, y, z))，枢轴写 bbmodel 坐标（z 向前为 +），导出时 z 取反 ─────────────────────────

CLAW_X = {"l": (-7.95, -6.5, -5.05), "r": (5.05, 6.5, 7.95)}     # 三指爪各指的 x（臂中心 ∓6.5，指间距 1.45）

BONES: Dict[str, Tuple[Optional[str], Tuple[float, float, float]]] = {
    "root": (None, (0.0, 0.0, 0.0)),
    "base": ("root", (0.0, 0.0, 0.0)),
    "leg_l": ("root", (-2.5, 4.8, 0.3)),
    "leg_r": ("root", (2.5, 4.8, 0.3)),
    "body": ("root", (0.0, 6.0, 0.0)),
    "core": ("body", (0.0, 9.0, 1.0)),
    "spine": ("body", (0.0, 9.5, -3.5)),
    "head": ("body", (0.0, 12.0, 0.0)),
    "jaw": ("head", (0.0, 13.0, -2.0)),
}
for side, sx in (("l", -1.0), ("r", 1.0)):
    BONES[f"arm_{side}"] = ("body", (sx * 6.5, 11.7, 0.0))
    BONES[f"forearm_{side}"] = (f"arm_{side}", (sx * 6.5, 7.2, 0.0))
    for k in range(3):
        BONES[f"claw_{side}_{k}"] = (f"forearm_{side}", (CLAW_X[side][k], 2.4, 1.5))


def bone_of(cube_name: str) -> str:
    """每个立方体归哪根骨头——只看名字。"""
    n = cube_name
    if n.startswith("base_"):
        return "base"
    if n.startswith("leg_"):
        return "leg_l" if n.endswith("_l") else "leg_r"
    if n.startswith(("hip_", "torso_")):
        return "body"
    if n.startswith(("cage_", "rune_")):
        return "core"
    if n.startswith("spine_"):
        return "spine"
    if n.startswith("jaw"):
        return "jaw"
    if n.startswith(("neck", "skull_", "eye_socket_")):
        return "head"
    m = re.match(r"claw_(l|r)_(\d)", n)
    if m:
        return f"claw_{m.group(1)}_{m.group(2)}"
    m = re.match(r"(shoulder|arm_upper)_(l|r)$", n)
    if m:
        return f"arm_{m.group(2)}"
    m = re.match(r"(arm_elbow|arm_fore)_(l|r)$", n) or re.match(r"guard_(l|r)_", n)
    if m:
        side = m.group(2) if m.lastindex and m.lastindex >= 2 else m.group(1)
        return f"forearm_{side}"
    raise ValueError(f"立方体 {cube_name} 没有归属的骨头")


# ── geo 导出 ──────────────────────────────────────────────────────────────


def _uv(face: str, rect: List[float]) -> dict:
    """bbmodel 面 UV [u0,v0,u1,v1] → Bedrock {uv, uv_size}（up / down 在 bbmodel 里是反向角，见 import_creature_geo）。"""
    a, b, c, d = rect
    if face in ("up", "down"):
        return {"uv": [c, d], "uv_size": [a - c, b - d]}
    return {"uv": [a, b], "uv_size": [c - a, d - b]}


def build_geo(bb: dict) -> dict:
    by_bone: Dict[str, List[dict]] = {name: [] for name in BONES}
    for element in bb["elements"]:
        lo, hi = element["from"], element["to"]
        faces = {}
        for face, data in element["faces"].items():
            out_face = {"north": "south", "south": "north"}.get(face, face)      # z 取反后南北互换
            faces[out_face] = _uv(face, data["uv"])
        by_bone[bone_of(element["name"])].append({
            "origin": [lo[0], lo[1], -hi[2]],                                     # z 取反：[-hi_z, -lo_z]
            "size": [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]],
            "uv": faces,
        })
    bones = []
    for name, (parent, pivot) in BONES.items():
        bone = {"name": name, "pivot": [pivot[0], pivot[1], -pivot[2]]}
        if parent:
            bone["parent"] = parent
        if by_bone[name]:
            bone["cubes"] = by_bone[name]
        bones.append(bone)
    width, height = bb["resolution"]["width"], bb["resolution"]["height"]
    return {
        "format_version": "1.12.0",
        "minecraft:geometry": [{
            "description": {"identifier": IDENTIFIER, "texture_width": width, "texture_height": height,
                            "visible_bounds_width": 2, "visible_bounds_height": 2, "visible_bounds_offset": [0, 1, 0]},
            "bones": bones,
        }],
    }


# ── 动画 ──────────────────────────────────────────────────────────────────
# 约定：每段由若干「时间点 → 姿态」组成，姿态 = {骨: {"rot": [x,y,z]°, "pos": [x,y,z]px, "scale": [x,y,z]}}，
# 没写的骨 / 通道取静止值。生成 JSON 时：用到的每根骨的每个通道在**所有时间点都写一个关键帧**
# （循环段首尾同值，不会被插值衰减回默认；一次性段末帧就是停留姿态）。

Pose = Dict[str, Dict[str, List[float]]]
REST = {"rot": [0.0, 0.0, 0.0], "pos": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]}
CHANNEL = {"rot": "rotation", "pos": "position", "scale": "scale"}


def pose(**kwargs) -> Pose:
    """pose(body={"rot": [..]}, head=...) 的写法糖。"""
    return dict(kwargs)


def both(arm: str, **values) -> Pose:
    """左右对称的姿态：值里的 z 向（外展）分量镜像取反，其余相同。左 = 参数原值，右 = z 取反。"""
    out: Pose = {}
    for side, flip in (("l", 1.0), ("r", -1.0)):
        channels = {}
        for ch, vec in values.items():
            channels[ch] = [vec[0], vec[1] * flip, vec[2] * flip] if ch == "rot" else list(vec)
        out[arm.format(side=side)] = channels
    return out


def merge(*poses: Pose) -> Pose:
    out: Pose = {}
    for p in poses:
        for bone, channels in p.items():
            out.setdefault(bone, {}).update(channels)
    return out


def build_clip(length: float, loop: bool, keys: List[Tuple[float, Pose]]) -> dict:
    bones_used = sorted({bone for _, p in keys for bone in p})
    bones: dict = {}
    for bone in bones_used:
        channels_used = sorted({ch for _, p in keys for ch in p.get(bone, {})})
        tracks = {}
        for ch in channels_used:
            tracks[CHANNEL[ch]] = {f"{t:.3f}".rstrip("0").rstrip(".") if t else "0.0": p.get(bone, {}).get(ch, REST[ch])
                                   for t, p in keys}
        bones[bone] = tracks
    clip = {"animation_length": length, "bones": bones}
    if loop:
        clip["loop"] = True
    return clip


def curl(side: str, deg: float, spread: float = 0.0) -> Pose:
    """三指爪：x 向弯曲 deg°，外侧两指再向外 / 内侧各张 spread°（z 向，左右镜像）。"""
    sign = -1.0 if side == "l" else 1.0
    return {f"claw_{side}_0": {"rot": [deg, 0.0, sign * -spread]},
            f"claw_{side}_1": {"rot": [deg, 0.0, 0.0]},
            f"claw_{side}_2": {"rot": [deg, 0.0, sign * spread]}}


def arms(shoulder: Tuple[float, float, float], elbow: Tuple[float, float, float], claw: float, spread: float) -> Pose:
    """两臂同姿态（z 向外展自动镜像）+ 爪。"""
    out = merge(both("arm_{side}", rot=shoulder), both("forearm_{side}", rot=elbow))
    for side in ("l", "r"):
        out = merge(out, curl(side, claw, spread))
    return out


# 站姿（准备攻击 / 激活完成）：头抬正、双臂微外展、爪半张
READY = merge(
    {"body": {"rot": [-2.0, 0.0, 0.0]}, "head": {"rot": [-4.0, 0.0, 0.0]}, "core": {"scale": [1.0, 1.0, 1.0]}},
    arms((6.0, 0.0, 10.0), (-14.0, 0.0, 4.0), 8.0, 10.0),
)
# 沉睡姿态：垂头含胸，双臂内收下垂，爪蜷着，阵核暗淡
SLUMP = merge(
    {"body": {"rot": [8.0, 0.0, 0.0], "pos": [0.0, -0.5, 0.0]}, "head": {"rot": [22.0, 0.0, 0.0]},
     "jaw": {"rot": [3.0, 0.0, 0.0]}, "core": {"scale": [0.9, 0.9, 0.9]}, "spine": {"rot": [5.0, 0.0, 0.0]},
     "leg_l": {"rot": [0.0, 0.0, 0.0]}, "leg_r": {"rot": [0.0, 0.0, 0.0]}},
    arms((10.0, 0.0, -4.0), (-6.0, 0.0, -2.0), 28.0, 0.0),
)


def dormant() -> dict:
    """待机（循环 4s）：呼吸式的微起伏。身体慢慢上下、头随呼吸点一点、爪尖偶尔蜷一下、阵核暗暗明灭。
    每根骨头的节拍错开，不会读成「整体一起晃」。"""
    def breath(body_y: float, head_x: float, core: float, claw: float, sway: float) -> Pose:
        return merge(
            SLUMP,
            {"body": {"rot": [8.0, 0.0, 0.0], "pos": [0.0, body_y, 0.0]}, "head": {"rot": [head_x, 0.0, 0.0]},
             "core": {"scale": [core, core, core]}},
            arms((10.0 + sway, 0.0, -4.0), (-6.0 - sway, 0.0, -2.0), claw, 0.0),
        )

    keys = [
        (0.0, breath(-0.50, 22.0, 0.90, 28.0, 0.0)),
        (1.2, breath(-0.28, 20.6, 0.97, 24.0, 1.2)),
        (2.0, breath(-0.30, 20.9, 0.99, 24.5, 1.4)),
        (3.2, breath(-0.46, 22.4, 0.92, 31.0, -0.4)),
        (4.0, breath(-0.50, 22.0, 0.90, 28.0, 0.0)),
    ]
    return build_clip(4.0, True, keys)


def wake() -> dict:
    """激活（一次 1.4s）：阵核先亮一下，然后**关节逐节张开**——脊背 → 肩 → 肘 → 爪 → 头抬起 → 下颌张合，站定。"""
    t0 = SLUMP
    flash = merge(t0, {"core": {"scale": [1.35, 1.35, 1.35]}})
    spine_up = merge(flash, {"body": {"rot": [2.0, 0.0, 0.0], "pos": [0.0, -0.1, 0.0]}, "spine": {"rot": [0.0, 0.0, 0.0]},
                             "core": {"scale": [1.1, 1.1, 1.1]}})
    shoulders = merge(spine_up, both("arm_{side}", rot=(0.0, 0.0, 16.0)))
    elbows = merge(shoulders, both("forearm_{side}", rot=(-22.0, 0.0, 8.0)))
    claws = merge(elbows, curl("l", 0.0, 22.0), curl("r", 0.0, 22.0))
    head_up = merge(claws, {"head": {"rot": [-6.0, 0.0, 0.0]}, "jaw": {"rot": [3.0, 0.0, 0.0]}, "body": {"rot": [-3.0, 0.0, 0.0]}})
    jaw_open = merge(head_up, {"jaw": {"rot": [24.0, 0.0, 0.0]}, "head": {"rot": [-9.0, 0.0, 0.0]}})
    settle = READY
    keys = [(0.0, t0), (0.14, flash), (0.34, spine_up), (0.54, shoulders), (0.74, elbows), (0.92, claws),
            (1.08, head_up), (1.2, jaw_open), (1.4, settle)]
    return build_clip(1.4, False, keys)


def strike() -> dict:
    """触发攻击（一次 0.7s）：下蹲后仰蓄势、双臂向后举起、下颌张开 → 猛地前扑，双臂合抓、下颌合上 → 停一拍 → 收回站姿。"""
    windup = merge(
        READY,
        {"body": {"rot": [-12.0, 0.0, 0.0], "pos": [0.0, -0.7, 0.8]}, "head": {"rot": [-12.0, 0.0, 0.0]},
         "jaw": {"rot": [26.0, 0.0, 0.0]}, "leg_l": {"rot": [10.0, 0.0, 0.0], "scale": [1.0, 0.88, 1.0]},
         "leg_r": {"rot": [10.0, 0.0, 0.0], "scale": [1.0, 0.88, 1.0]}, "core": {"scale": [1.25, 1.25, 1.25]}},
        arms((62.0, 0.0, 22.0), (22.0, 0.0, 6.0), -6.0, 26.0),     # 手臂在肩下面：x 为正 = 向后上举
    )
    lunge = merge(
        READY,
        {"body": {"rot": [20.0, 0.0, 0.0], "pos": [0.0, 0.5, -3.2]}, "head": {"rot": [10.0, 0.0, 0.0]},
         "jaw": {"rot": [2.0, 0.0, 0.0]}, "leg_l": {"rot": [-6.0, 0.0, 0.0]}, "leg_r": {"rot": [-6.0, 0.0, 0.0]},
         "core": {"scale": [1.1, 1.1, 1.1]}},
        arms((-74.0, 0.0, -6.0), (-24.0, 0.0, -4.0), 26.0, 4.0),   # x 为负 = 向前上抓
    )
    hold = merge(lunge, {"body": {"rot": [22.0, 0.0, 0.0], "pos": [0.0, 0.3, -3.6]}})
    keys = [(0.0, READY), (0.2, windup), (0.34, lunge), (0.46, hold), (0.7, READY)]
    return build_clip(0.7, False, keys)


def exhausted() -> dict:
    """阵核耗尽瘫倒（一次 2.2s）：阵核剧烈闪烁、越闪越暗 → 腿一软、上身前倒 → 头垂下、下颌松开、双臂耷拉 → 轻微回弹后彻底不动。"""
    def core(v: float) -> Pose:
        return {"core": {"scale": [v, v, v]}}

    flicker1 = merge(READY, core(0.55))
    flicker2 = merge(READY, core(1.25), {"body": {"rot": [-3.0, 0.0, 0.0]}})
    flicker3 = merge(READY, core(0.4), {"body": {"rot": [0.0, 0.0, 1.0]}})
    sag = merge(
        READY, core(0.6),
        {"body": {"rot": [10.0, 0.0, 2.0], "pos": [0.0, -1.0, 0.0]}, "head": {"rot": [14.0, 0.0, 0.0]},
         "leg_l": {"rot": [8.0, 0.0, 0.0], "scale": [1.0, 0.85, 1.0]}, "leg_r": {"rot": [8.0, 0.0, 0.0], "scale": [1.0, 0.85, 1.0]}},
        arms((14.0, 0.0, 12.0), (-4.0, 0.0, 6.0), 14.0, 8.0),
    )
    fall = merge(
        core(0.3),
        {"body": {"rot": [26.0, 0.0, 6.0], "pos": [0.0, -2.4, -1.0]}, "head": {"rot": [40.0, 0.0, 0.0]},
         "jaw": {"rot": [16.0, 0.0, 0.0]}, "spine": {"rot": [10.0, 0.0, 0.0]},
         "leg_l": {"rot": [20.0, 0.0, 0.0], "scale": [1.0, 0.62, 1.0]}, "leg_r": {"rot": [16.0, 0.0, 0.0], "scale": [1.0, 0.66, 1.0]}},
        arms((26.0, 0.0, 22.0), (14.0, 0.0, 8.0), 44.0, 6.0),
    )
    bounce = merge(fall, {"body": {"rot": [22.0, 0.0, 5.0], "pos": [0.0, -2.1, -0.8]}, "head": {"rot": [34.0, 0.0, 0.0]},
                          "jaw": {"rot": [10.0, 0.0, 0.0]}, "core": {"scale": [0.36, 0.36, 0.36]}})
    rest = merge(fall, {"body": {"rot": [27.0, 0.0, 6.0], "pos": [0.0, -2.5, -1.0]}, "head": {"rot": [42.0, 0.0, 0.0]},
                        "jaw": {"rot": [18.0, 0.0, 0.0]}, "core": {"scale": [0.2, 0.2, 0.2]}})
    keys = [(0.0, READY), (0.12, flicker1), (0.26, flicker2), (0.4, flicker3), (0.55, merge(READY, core(0.9))),
            (0.7, flicker1), (0.95, sag), (1.35, fall), (1.6, bounce), (2.2, rest)]
    return build_clip(2.2, False, keys)


CLIPS = {"dormant": dormant, "wake": wake, "strike": strike, "exhausted": exhausted}


def build_animations() -> dict:
    return {"format_version": "1.8.0", "animations": {f"{CLIP_PREFIX}{name}": fn() for name, fn in CLIPS.items()}}


# ── 门禁 ──────────────────────────────────────────────────────────────────


def check_rig(bb: dict, geo: dict) -> None:
    """外形不变：geo 里的立方体个数和体积、整体包围盒都和原 bbmodel 一致；每根骨头的父骨存在；每个立方体都归了骨头。"""
    bones = geo["minecraft:geometry"][0]["bones"]
    names = {b["name"] for b in bones}
    for b in bones:
        assert b.get("parent") is None or b["parent"] in names, f"{b['name']} 的父骨 {b.get('parent')} 不存在"
    cubes = [c for b in bones for c in b.get("cubes", [])]
    assert len(cubes) == len(bb["elements"]), f"立方体数 {len(cubes)} != 原 {len(bb['elements'])}"
    vol_geo = sum(c["size"][0] * c["size"][1] * c["size"][2] for c in cubes)
    vol_bb = sum((e["to"][0] - e["from"][0]) * (e["to"][1] - e["from"][1]) * (e["to"][2] - e["from"][2]) for e in bb["elements"])
    assert abs(vol_geo - vol_bb) < 1e-6, f"体积 {vol_geo} != 原 {vol_bb}"
    lo = [min(c["origin"][i] for c in cubes) for i in range(3)]
    hi = [max(c["origin"][i] + c["size"][i] for c in cubes) for i in range(3)]
    blo = [min(e["from"][i] for e in bb["elements"]) for i in range(3)]
    bhi = [max(e["to"][i] for e in bb["elements"]) for i in range(3)]
    assert lo[0] == blo[0] and hi[0] == bhi[0] and lo[1] == blo[1] and hi[1] == bhi[1], "x/y 包围盒变了"
    assert abs(lo[2] + bhi[2]) < 1e-9 and abs(hi[2] + blo[2]) < 1e-9, "z 包围盒不是原 bbmodel 的镜像"


def check_animations(animations: dict) -> None:
    """每段都有；循环段首尾同值（不会被插值衰减）；每个关键帧的骨头都存在；非循环段末帧在动画长度上。"""
    clips = animations["animations"]
    assert set(clips) == {f"{CLIP_PREFIX}{n}" for n in CLIPS}, f"动画段不对: {sorted(clips)}"
    for full, clip in clips.items():
        for bone, channels in clip["bones"].items():
            assert bone in BONES, f"{full}: 未知骨 {bone}"
            for channel, keys in channels.items():
                times = sorted(float(t) for t in keys)
                if clip.get("loop"):
                    first, last = keys[min(keys, key=float)], keys[max(keys, key=float)]
                    assert first == last, f"{full}.{bone}.{channel} 循环首尾不同值: {first} vs {last}"
                else:
                    assert abs(times[-1] - clip["animation_length"]) < 1e-6, f"{full}.{bone}.{channel} 末帧 {times[-1]} 不在动画长度上"


def self_test() -> None:
    bb = json.loads(SOURCE.read_text(encoding="utf-8"))
    geo = build_geo(bb)
    check_rig(bb, geo)
    animations = build_animations()
    check_animations(animations)
    print(f"  [OK] 骨架 {len(BONES)} 根、{len(bb['elements'])} 个立方体（个数 / 体积 / 包围盒与原 bbmodel 一致），4 段动画通过")

    def must_fail(name: str, fn) -> None:
        try:
            fn()
        except (AssertionError, ValueError, KeyError) as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e)[:50]}")
            return
        raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    broken = json.loads(json.dumps(geo))
    next(b for b in broken["minecraft:geometry"][0]["bones"] if b["name"] == "body")["cubes"].pop()
    must_fail("丢一个立方体", lambda: check_rig(bb, broken))
    bad_loop = json.loads(json.dumps(animations))
    track = bad_loop["animations"][f"{CLIP_PREFIX}dormant"]["bones"]["head"]["rotation"]
    track[max(track, key=float)] = [99.0, 0.0, 0.0]
    must_fail("循环段末帧不等于首帧", lambda: check_animations(bad_loop))
    bad_end = json.loads(json.dumps(animations))
    bad_end["animations"][f"{CLIP_PREFIX}wake"]["animation_length"] = 5.0
    must_fail("一次性段末帧不在长度上", lambda: check_animations(bad_end))
    try:
        bone_of("mystery_cube")
    except ValueError as e:
        print(f"  [OK] 注入「没有归属的立方体」被拦截: {e}")
    else:
        raise RuntimeError("门禁失效：未知立方体没被拦住")


def write_assets() -> Path:
    bb = json.loads(SOURCE.read_text(encoding="utf-8"))
    geo, animations = build_geo(bb), build_animations()
    check_rig(bb, geo)
    check_animations(animations)
    (OUT_ROOT / "geo").mkdir(parents=True, exist_ok=True)
    (OUT_ROOT / "animations").mkdir(parents=True, exist_ok=True)
    texture_dir = OUT_ROOT / "textures" / "entity" / "fauna"
    texture_dir.mkdir(parents=True, exist_ok=True)
    (OUT_ROOT / "geo" / f"{NAME}.geo.json").write_text(json.dumps(geo, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (OUT_ROOT / "animations" / f"{NAME}.animation.json").write_text(json.dumps(animations, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    source = bb["textures"][0]["source"].split(",", 1)[1]
    (texture_dir / f"{NAME}.png").write_bytes(base64.b64decode(source))
    return OUT_ROOT


def render_gifs(size: int) -> None:
    GIF_DIR.mkdir(parents=True, exist_ok=True)
    cmd = [sys.executable, str(REPO / "modelScript" / "tools" / "creature_anim_frames.py"), "--name", NAME,
           "--assets", str(OUT_ROOT), "--gif", "--size", str(size), "--out", str(GIF_DIR), "--facing=-z"]
    subprocess.run(cmd, check=True, cwd=REPO / "modelScript" / "tools")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gif", action="store_true", help="另出 4 段 GIF")
    parser.add_argument("--size", type=int, default=440)
    args = parser.parse_args()
    print("gen_niche_house_puppet_anim 自检…")
    self_test()
    print(f"[assets] {write_assets().relative_to(REPO)}")
    if args.gif:
        render_gifs(args.size)


if __name__ == "__main__":
    main()
