#!/usr/bin/env python3
"""浮灵母动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

五段动画（全部从零写关键帧）：

  idle    3.2s 循环  伞盖脉动：快收缩、慢舒张，收缩时整只借喷流向上顶一下；裙瓣随收缩内合、舒张外展；
                     十二条触须按相位滞后飘摆（越靠末梢越晚）；口腕轻摆、灵核随脉动一明一暗、光点缓缓晃荡
  walk    1.3s 循环  推进游动：脉动更快更深，身子前倾，触须向后拖成流线，裙瓣收拢
  attack  1.5s 一次  触须缠绕：伞盖先吸气鼓开 → 猛收缩前冲，触须依次向前甩出 → 末梢逐节向内卷紧缠住 → 松开回位
  hurt    0.5s 一次  伞盖被挤扁一下、身子歪斜，触须炸开乱抖，灵核闪烁、光点四散
  death   3.2s 一次  脉动变乱 → 伞盖瘪下摊平、裙瓣外垂，触须逐节折叠瘫在地上，灵核与光点收缩熄灭

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨的旋转 / 缩放 / 位移。
符号约定（对着数值核过）：
  · 垂下的骨（裙瓣 / 触须 / 口腕）要让下端朝水平方向 d 摆 θ°，写成 rot = θ·(-d_z, 0, d_x)（swing()）；
    d = (0,0,-1) 即 rot.x 为正 = 向前（-Z）摆；d = 径向外 u 即外展，d = -u 即内收；
  · 伞盖缩放全挂在最下层 bell_5 上：它的子骨（上面五层、灵核、口腕）整体跟着缩放，裙瓣 / 触须挂在 bell_5 上，
    挂点半径随之收放，所以「伞缘收紧时触须根部一起并拢」不用另写；
  · 灵核 core 挂在顶盘下，随伞盖缩放；单独再给 core 一个缩放做明暗。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/fu_ling_mu/gen_anim.py
"""

from __future__ import annotations

import json
import math
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Callable

from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, write_animated_bbmodel

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CODEC = ROOT / "modelScript" / "exporters" / "creature_codec.cjs"
sys.path.insert(0, str(HERE))

import gen_fu_ling_mu as geo  # noqa: E402  骨名、触须束、裙瓣方位直接取建模稿的权威定义

NAME = "fu_ling_mu"
SOURCE = HERE / "FuLingMu_tex.bbmodel"
ANIMATED = HERE / "FuLingMu_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 32

N_SEG = geo.TENT_SEGS
Sampler = Callable[[float], Pose]

# 触须束：(束名, 径向外单位向量 u, 切向单位向量 t, 本束相位偏移)，相位按方位角错开，整圈触须不会齐步走
TENTACLES = []
for _stem, _centre in geo.TENT_BUNDLES:
    for _s in ((1.0,) if _centre else (-1.0, 1.0)):
        _tag = "" if _centre else geo.side_tag(_s)
        _u, _t = geo.frame(_stem, _s)
        TENTACLES.append((f"tent_{_stem}{_tag}", _u, _t, (_stem + (0 if _s > 0 else 180)) / 360.0))
LAPPETS = [(f"lappet_{stem}{tag}", geo.frame(stem, s)[0]) for stem, s, tag in geo.placed(geo.SIDED_STEMS, geo.CENTER_STEMS)]
ORALS = [(f"oral_{stem}{tag}", geo.frame(stem, s)[0]) for stem, s, tag in geo.placed(geo.ORAL_STEMS, ())]
TIER_BONES = [tier[0] for tier in geo.TIERS]          # 自下而上：bell_5 … bell_0
BELL = TIER_BONES[0]


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t ∈ [0,1] 上走 cycles 个整周期的正弦，循环动画首尾自然相接。"""
    return math.sin(math.tau * (cycles * t + phase))


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep：points 是 (t01, 值) 的有序表，一次性动作用它排节奏。"""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


def turn(pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0)) -> None:
    """往 pose 里累加一根骨的旋转 / 位移（同一根骨多处调用会叠加）。"""
    channel = pose[bone]
    channel.rot = [channel.rot[i] + rot[i] for i in range(3)]
    channel.pos = [channel.pos[i] + pos[i] for i in range(3)]


def grow(pose: Pose, bone: str, scale=(1.0, 1.0, 1.0)) -> None:
    channel = pose[bone]
    channel.scale = [channel.scale[i] * scale[i] for i in range(3)]


def swing(pose: Pose, bone: str, d, deg: float) -> None:
    """垂下的骨，下端朝水平方向 d（单位向量）摆 deg°。"""
    turn(pose, bone, rot=(-d[2] * deg, 0.0, d[0] * deg))


def pulse(phase: float) -> float:
    """水母脉动：phase ∈ [0,1)，0→0.28 快速收缩到 1，之后慢慢舒张回 0（首尾同值）。"""
    phase %= 1.0
    return keys(phase, [(0.0, 0.0), (0.28, 1.0), (1.0, 0.0)])


# ---------------------------------------------------------------- 部件姿态
def bell(pose: Pose, c: float, depth: float = 0.12, lift: float = 0.06) -> None:
    """伞盖收缩 c ∈ [0,1]：水平收窄 depth、垂向拉高 lift；上面各层再按层号略有差分，收缩时下沿比顶盘收得更多。"""
    grow(pose, BELL, (1.0 - depth * c, 1.0 + lift * c, 1.0 - depth * c))
    for k, name in enumerate(TIER_BONES[1:], start=1):
        grow(pose, name, (1.0 + 0.012 * k * c, 1.0, 1.0 + 0.012 * k * c))


def lappets(pose: Pose, inward: float) -> None:
    """裙瓣：inward > 0 向内收、< 0 向外展（度）。"""
    for name, u in LAPPETS:
        swing(pose, name, (-u[0], 0.0, -u[2]), inward)


def tentacle_wave(pose: Pose, t: float, c: float, flow: float, wave_deg: float, cycles: float = 1.0,
                  lag: float = 0.1, trail: tuple[float, float, float] = (0.0, 0.0, 0.0), trail_deg: float = 0.0) -> None:
    """十二条触须：每节在径向 / 切向各摆一个滞后正弦；收缩 c 时内合 flow°；trail 方向再拖 trail_deg°（逐节累加）。"""
    for bundle, u, tv, phase in TENTACLES:
        for k in range(N_SEG):
            seg = f"{bundle}_{k}"
            lagged = t * cycles - lag * k - 0.3 * phase
            swing(pose, seg, (-u[0], 0.0, -u[2]), flow * c)
            swing(pose, seg, u, wave_deg * math.sin(math.tau * lagged))
            swing(pose, seg, tv, wave_deg * 0.7 * math.sin(math.tau * (lagged + 0.25)))
            if trail_deg:
                swing(pose, seg, trail, trail_deg)


def orals(pose: Pose, t: float, sway: float) -> None:
    for name, u in ORALS:
        for suffix, lag in (("a", 0.0), ("b", 0.1)):
            swing(pose, f"{name}_{suffix}", u, sway * math.sin(math.tau * (t - lag + 0.2 * u[0])))


def glow(pose: Pose, level: float) -> None:
    """灵核明暗：靠缩放表现（贴图只有一种亮度），level = 1 原大小。"""
    grow(pose, "core", (level, level, level))


# ---------------------------------------------------------------- 五段动画
def idle(t: float) -> Pose:
    pose = Pose()
    c = pulse(t)
    bell(pose, c)
    turn(pose, "root", pos=(0.0, 3.2 * smooth(c) + 0.6 * wave(t, 1, 0.1) + 0.6, 0.0))    # 收缩喷流向上顶，舒张时缓缓沉
    turn(pose, BELL, rot=(1.5 * wave(t, 1, 0.2), 0.0, 1.5 * wave(t, 1, 0.45)))
    lappets(pose, 11.0 * c - 4.0 * (1.0 - c))
    tentacle_wave(pose, t, pulse(t - 0.12), flow=7.0, wave_deg=4.0, cycles=1.0, lag=0.09)
    orals(pose, t, 7.0)
    glow(pose, 1.0 + 0.12 * c)
    turn(pose, "motes", rot=(0.0, 24.0 * wave(t, 1, 0.1), 0.0), pos=(0.0, 1.2 * wave(t, 1, 0.3), 0.0))
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    c = pulse(t)
    bell(pose, c, depth=0.18, lift=0.1)
    turn(pose, "root", rot=(-9.0 + 3.0 * c, 0.0, 0.0), pos=(0.0, 4.0 * smooth(c) + 1.0, 0.0))     # 身子前倾，收缩时前顶
    lappets(pose, 16.0 * c)
    back = (0.0, 0.0, 1.0)                                                    # 触须向后（+Z）拖成流线
    tentacle_wave(pose, t, pulse(t - 0.1), flow=10.0, wave_deg=3.0, cycles=1.0, lag=0.08, trail=back, trail_deg=11.0)
    orals(pose, t, 5.0)
    glow(pose, 1.0 + 0.16 * c)
    turn(pose, "motes", rot=(0.0, 18.0 * wave(t, 1, 0.1), 0.0))
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.22 吸气鼓开（伞扩、触须外展）；0.22–0.42 猛收缩前冲、触须依次向前甩出；
    #       0.42–0.78 触须末梢逐节内卷缠紧；0.78–1 松开回位
    inhale = keys(t, [(0.0, 0.0), (0.22, 1.0), (0.3, 0.0), (1.0, 0.0)])
    squeeze = keys(t, [(0.0, 0.0), (0.22, 0.0), (0.4, 1.0), (0.56, 0.7), (0.78, 0.5), (1.0, 0.0)])
    lunge = keys(t, [(0.0, 0.0), (0.22, -0.2), (0.42, 1.0), (0.62, 0.9), (0.82, 0.15), (1.0, 0.0)])
    bell(pose, squeeze, depth=0.2, lift=0.1)
    grow(pose, BELL, (1.0 + 0.1 * inhale, 1.0 - 0.08 * inhale, 1.0 + 0.1 * inhale))
    turn(pose, "root", rot=(-12.0 * lunge, 0.0, 0.0), pos=(0.0, 4.0 * lunge + 1.0, -14.0 * lunge))
    lappets(pose, 14.0 * squeeze - 12.0 * inhale)
    forward = (0.0, 0.0, -1.0)
    for bundle, u, tv, phase in TENTACLES:
        for k in range(N_SEG):
            seg = f"{bundle}_{k}"
            # 越靠末梢越晚被甩出，再越晚卷紧，错开成一条会「鞭」的曲线
            whip = keys(t, [(0.0, 0.0), (0.22 + 0.04 * k, 0.0), (0.42 + 0.04 * k, 1.0), (0.82, 0.6), (1.0, 0.0)])
            coil = keys(t, [(0.0, 0.0), (0.42 + 0.05 * k, 0.0), (0.62 + 0.04 * k, 1.0), (0.8, 0.9), (0.96, 0.0), (1.0, 0.0)])
            swing(pose, seg, u, 10.0 * inhale)                                 # 吸气时先向外张开
            swing(pose, seg, forward, 20.0 * whip)                              # 向前甩
            swing(pose, seg, (-u[0], 0.0, -u[2]), 30.0 * coil * (0.5 + 0.18 * k))   # 向内卷紧：越靠末梢卷得越狠
    for name, u in ORALS:
        for suffix in "ab":
            swing(pose, f"{name}_{suffix}", forward, 24.0 * squeeze)
    glow(pose, 1.0 + 0.3 * squeeze)
    turn(pose, "motes", rot=(0.0, 30.0 * squeeze, 0.0))
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.16, 1.0), (0.55, 0.0), (1.0, 0.0)])           # 受击冲量：快进慢回
    shake = math.sin(math.tau * 5.0 * t) * kick
    grow(pose, BELL, (1.0 + 0.14 * kick, 1.0 - 0.18 * kick, 1.0 + 0.14 * kick))     # 伞盖被挤扁
    turn(pose, "root", rot=(5.0 * kick, 0.0, 9.0 * kick), pos=(0.0, -1.0 * kick + 1.0, 2.5 * kick))
    lappets(pose, -22.0 * kick)
    for bundle, u, tv, phase in TENTACLES:
        for k in range(N_SEG):
            seg = f"{bundle}_{k}"
            swing(pose, seg, u, 16.0 * kick)                                    # 触须炸开
            swing(pose, seg, tv, 12.0 * shake * (0.5 + 0.3 * k))
    orals(pose, t, 10.0 * kick)
    glow(pose, 1.0 + 0.45 * kick * (1.0 if int(t * 20.0) % 2 else -0.8))          # 灵核闪烁
    turn(pose, "motes", rot=(0.0, 60.0 * kick, 0.0), pos=(0.0, 2.0 * kick, 0.0))
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    # 0–0.25 脉动变乱（收缩越来越浅、越来越急）→ 0.25–0.7 伞盖瘪下摊平 → 触须逐节折叠瘫地 → 灵光熄灭
    erratic = keys(t, [(0.0, 1.0), (0.25, 0.5), (0.4, 0.0), (1.0, 0.0)])
    c = pulse(t * 3.0) * erratic
    sag = keys(t, [(0.2, 0.0), (0.72, 1.0), (1.0, 1.0)])                      # 伞盖瘪下
    sink = keys(t, [(0.25, 0.0), (0.75, 1.0), (1.0, 1.0)])                    # 整只往下沉
    bell(pose, c, depth=0.1, lift=0.05)
    grow(pose, BELL, (1.0 + 0.28 * sag, 1.0 - 0.5 * sag, 1.0 + 0.28 * sag))
    for k, name in enumerate(TIER_BONES[1:], start=1):                          # 上层比下层瘪得更狠，压成一叠
        grow(pose, name, (1.0 + 0.03 * k * sag, 1.0 - 0.16 * k * sag, 1.0 + 0.03 * k * sag))
    turn(pose, "root", rot=(6.0 * sag, 0.0, -10.0 * sag), pos=(0.0, -20.0 * sink + 1.0, 0.0))
    lappets(pose, -48.0 * sag)
    for bundle, u, tv, phase in TENTACLES:
        for k in range(N_SEG):
            seg = f"{bundle}_{k}"
            fold = keys(t, [(0.2 + 0.05 * k, 0.0), (0.7 + 0.04 * k, 1.0), (1.0, 1.0)])
            # 逐节来回折叠：第一节向外摆，之后每节反向折回，像一堆软塌塌叠起的绳
            sign = 1.0 if k % 2 == 0 else -1.0
            swing(pose, seg, u, sign * (60.0 if k == 0 else 120.0) * fold)
            swing(pose, seg, tv, 8.0 * wave(t, 2, 0.1 * k + phase) * (1.0 - fold))
    for name, u in ORALS:
        for suffix in "ab":
            swing(pose, f"{name}_{suffix}", u, 50.0 * sag)
    out = 1.0 - keys(t, [(0.15, 0.0), (0.8, 1.0)])
    glow(pose, max(0.02, out * (0.85 + 0.15 * wave(t, 6, 0.0) * (1.0 - sag))))        # 灵核渐熄（缩到近乎消失）
    grow(pose, "motes", (max(0.02, out),) * 3)
    return pose


CLIPS = {
    "idle": (3.2, True, idle),
    "walk": (1.3, True, walk),
    "attack": (1.5, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (3.2, False, death),
}

# 地面约束：触须梢刚好点地，任何往下的位移都会让触须入地，托回地面；死亡段整只下沉由触须折叠接住。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 24.0


def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < -GROUND_TOLERANCE:
            turn(pose, "root", pos=(0, -depth, 0))
        return pose

    return sample


def check_ground(clip: str, rig: PoseRig, sampler: Sampler) -> float:
    lowest = min(rig.lowest(sampler(step / SAMPLES)) for step in range(SAMPLES + 1))
    if lowest < -MAX_LIFT:
        raise ValueError(f"{NAME}.{clip}: 最低点 {lowest:.2f}px，入地超过 {MAX_LIFT}px，动作把身体压进地里了")
    return lowest


def seam_gap(a: Pose, b: Pose) -> float:
    """两个姿态的最大旋转 / 位移 / 缩放差。循环动画首尾必须接上。"""
    bones = set(a) | set(b)
    rot = max((max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)) for bone in bones), default=0.0)
    pos = max((max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)) for bone in bones), default=0.0)
    scale = max((max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)) for bone in bones), default=0.0)
    return max(rot, pos * 2.0, scale * 20.0)


def build() -> list[dict]:
    rig = PoseRig(SOURCE)
    animations = []
    for clip, (length, loop, raw) in CLIPS.items():
        if loop:
            gap = seam_gap(raw(0.0), raw(1.0))
            if gap > 0.5:
                raise ValueError(f"{NAME}.{clip}: 循环首尾差 {gap:.2f} 接不上")
        sampler = grounded(rig, raw)
        lowest = check_ground(clip, rig, sampler)
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})
        print(f"  {clip:7s} {length:.1f}s {'loop' if loop else 'once'}  最低点 {lowest:+.2f}px  骨轨道 {len(tracks)}")
    write_animated_bbmodel(rig, animations, ANIMATED, NAME)
    return animations


def export_assets() -> None:
    """codec 导出 geo / animation.json，贴图取 bbmodel 内嵌 PNG；同时布置审阅 GIF 用的资源根。"""
    import base64

    staging = REVIEW_ASSETS / "_codec"
    staging.mkdir(parents=True, exist_ok=True)
    subprocess.run(["node", str(CODEC), str(ANIMATED), str(staging), NAME], check=True)

    doc = json.loads(SOURCE.read_text(encoding="utf-8"))
    png = base64.b64decode(doc["textures"][0]["source"].split(",", 1)[1], validate=True)

    EXPORT_DIR.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(staging / f"{NAME}.geo.json", EXPORT_DIR / f"{NAME}.geo.json")
    shutil.copyfile(staging / f"{NAME}.animation.json", EXPORT_DIR / f"{NAME}.animation.json")
    (EXPORT_DIR / f"{NAME}.png").write_bytes(png)

    for sub in ("geo", "animations", "textures/entity/fauna"):
        (REVIEW_ASSETS / sub).mkdir(parents=True, exist_ok=True)
    shutil.copyfile(EXPORT_DIR / f"{NAME}.geo.json", REVIEW_ASSETS / "geo" / f"{NAME}.geo.json")
    shutil.copyfile(EXPORT_DIR / f"{NAME}.animation.json", REVIEW_ASSETS / "animations" / f"{NAME}.animation.json")
    (REVIEW_ASSETS / "textures" / "entity" / "fauna" / f"{NAME}.png").write_bytes(png)
    print(f"✓ 导出 {EXPORT_DIR.relative_to(ROOT)}；审阅资源根 {REVIEW_ASSETS.relative_to(ROOT)}")


if __name__ == "__main__":
    build()
    export_assets()
