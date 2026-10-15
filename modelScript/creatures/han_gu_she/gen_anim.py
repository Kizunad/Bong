#!/usr/bin/env python3
"""旱骨蛇动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧）：

  idle    3.4s 循环  盘身立颈的静候：颈随呼吸缓缓起伏、慢慢左右扫，舌一伸一缩（试探空气），下颌微张微合，两侧残皮飘，盘圈微微蠕动
  walk    1.4s 循环  蛇形 slither：颈前倾低伏，一道 S 形侧向波自头向尾传，盘圈各环跟着小幅蠕动，头随波左右点，舌急速探吐
  strike  1.3s 一次  盘身立颈张口扑咬：颈向后蓄成更深的 S、下颌张到最大、舌急颤 → 颈猛然前射（头下扑）→ 下颌合拢咬住 → 叼住甩头撕扯 → 回缩
  attack  1.1s 一次  快咬（近身防御）：颈微后仰一缩 → 小幅前探一口，下颌一张一合，毒牙露出，回缩（比 strike 短而轻）
  hurt    0.5s 一次  被击后整条颈猛地后仰并偏向一侧，下颌大张，残皮炸开，盘圈一震
  death   2.6s 一次  抽搐 → 颈失去支撑向前一节一节软倒、伏在盘圈上，下颌松开垂落，舌耷拉，残皮垂下，盘圈松散

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
骨链：root → seg_00（尾尖）→ seg_01 → … → seg_20（颈根到颈顶）→ head → jaw / fang / shred；舌挂在下颌上。
符号约定（对着模型的 S 形颈核过）：颈大致竖直，对竖直的颈节 rot.x 为正 = 它上面的部分向后（+Z）仰，为负 = 向前（-Z）倾；
rot.z = 向侧面（±X）弯；下颌 rot.x 为负 = 张口、为正 = 合拢（下颌在模型里已是张开姿态，再合会咬住）。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/han_gu_she/gen_anim.py
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

import gen_han_gu_she as geo  # noqa: E402  环组骨名与盘圈 / 颈的分界直接取建模稿的权威定义

NAME = "han_gu_she"
SOURCE = HERE / "HanGuShe_tex.bbmodel"
ANIMATED = HERE / "HanGuShe_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 28

Sampler = Callable[[float], Pose]
COIL = [name for g, name in enumerate(geo.SEG_BONES) if g * geo.GROUP < geo.N_COIL]       # 尾尖 → 盘圈各环组
NECK = [name for g, name in enumerate(geo.SEG_BONES) if g * geo.GROUP >= geo.N_COIL]      # 颈的各环组（下 → 上）
N_NECK = len(NECK)


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


# ---------------------------------------------------------------- 部件
def neck_bend(pose: Pose, pitch: float, side: float = 0.0, taper: float = 1.0) -> None:
    """整条颈的姿态：pitch 是总俯仰（度，正 = 头后仰，负 = 向前倾），side 是总侧弯，平均分给各颈节；
    taper < 1 时靠近头的节分得少一点（颈根先动、颈梢跟着）。"""
    weights = [taper ** (N_NECK - 1 - k) for k in range(N_NECK)]
    total = sum(weights)
    for bone, w in zip(NECK, weights):
        turn(pose, bone, rot=(pitch * w / total, 0, side * w / total))


def neck_wave(pose: Pose, t: float, amp: float, cycles: float = 1.0, lag: float = 0.1, axis: int = 2) -> None:
    """颈上自头向尾传的侧向波：相位随节号增加（离头越近越晚），axis 2 = 侧弯、0 = 俯仰。"""
    for k, bone in enumerate(NECK):
        rot = [0.0, 0.0, 0.0]
        rot[axis] = amp * wave(t, cycles, lag * k)
        turn(pose, bone, rot=tuple(rot))


def coil_ripple(pose: Pose, t: float, amp: float, cycles: float = 1.0, lag: float = 0.07) -> None:
    """盘圈各环组的小幅蠕动：绕竖直轴来回拧一点点（逐节累加，所以每节只给零点几度）。"""
    for k, bone in enumerate(COIL):
        turn(pose, bone, rot=(0, amp * wave(t, cycles, lag * k), 0))


def tongue(pose: Pose, flick: float, spread: float = 0.0) -> None:
    """舌：flick > 0 = 向前探（舌根抬起、叉尖分开），< 0 = 缩回垂下。"""
    turn(pose, "tongue_1", rot=(-10.0 * flick, 0, 0))
    turn(pose, "tongue_2", rot=(-16.0 * flick, spread * 8.0, 0))


def jaw(pose: Pose, open_: float) -> None:
    """下颌：open_ ∈ [-1, 1]，1 = 再张大 22°，-1 = 合拢（压向上颌）。"""
    turn(pose, "jaw", rot=(-22.0 * open_, 0, 0))


def shreds(pose: Pose, t: float, sway: float, cycles: float = 1.0, lift: float = 0.0) -> None:
    """头两侧的残皮：向外飘、绕前后轴摆；lift > 0 被气流掀向外上。"""
    for bone, sign in (("shred_l", -1.0), ("shred_r", 1.0)):
        turn(pose, bone, rot=(sway * 0.4 * wave(t, cycles, 0.2), 0, sign * (sway * wave(t, cycles, 0.0 if sign > 0 else 0.3) + lift)))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breath = wave(t, 1)
    neck_bend(pose, pitch=2.5 * breath, side=7.0 * wave(t, 1, 0.3))                  # 颈随呼吸起伏、缓缓左右扫
    neck_wave(pose, t, 0.8, cycles=1, lag=0.05)
    coil_ripple(pose, t, 0.35, cycles=1)
    turn(pose, "head", rot=(1.5 * wave(t, 2, 0.1), 5.0 * wave(t, 1, 0.45), 2.0 * wave(t, 1, 0.2)))
    jaw(pose, 0.15 + 0.25 * max(0.0, wave(t, 2, 0.35)))
    flick = max(0.0, wave(t, 3, 0.1)) ** 2 * (1.0 if wave(t, 1, 0.25) > 0.2 else 0.15)       # 舌三伸三缩，只在一段时间里伸得长
    tongue(pose, flick - 0.2, spread=flick)
    shreds(pose, t, 6.0, cycles=2)
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    neck_bend(pose, pitch=-20.0 + 4.0 * wave(t, 2, 0.1), side=0.0, taper=0.8)        # 颈前倾低伏，头探向前
    neck_wave(pose, t, 5.5, cycles=1, lag=0.11)                                     # S 形侧向波自头向尾传
    coil_ripple(pose, t, 1.1, cycles=1, lag=0.09)
    turn(pose, "root", rot=(0, 2.5 * wave(t, 1, 0.5), 0), pos=(0, 0.25 * abs(wave(t, 2, 0.2)), 0))
    turn(pose, "head", rot=(4.0 * wave(t, 2, 0.35), 6.0 * wave(t, 1, 0.1), 3.0 * wave(t, 1, 0.3)))
    jaw(pose, 0.2)
    flick = max(0.0, wave(t, 3, 0.0)) ** 1.5
    tongue(pose, flick, spread=flick)
    shreds(pose, t, 8.0, cycles=2, lift=4.0)
    return pose


def strike(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.34 颈向后蓄成更深的 S、下颌张到最大、舌急颤；0.34–0.5 颈猛然前射（头下扑）；
    #       0.5–0.58 一口咬合（下颌合拢）；0.58–0.86 叼住甩头撕扯；0.86–1 回缩
    coil = keys(t, [(0.0, 0.0), (0.34, 1.0), (0.42, 0.6), (0.5, 0.0), (1.0, 0.0)])         # 蓄力（颈后仰）
    lunge = keys(t, [(0.0, 0.0), (0.34, 0.0), (0.48, 1.0), (0.7, 0.85), (0.9, 0.2), (1.0, 0.0)])   # 前射
    gape = keys(t, [(0.0, 0.0), (0.2, 0.7), (0.34, 1.0), (0.5, 1.0), (0.56, -0.8), (0.86, -0.5), (0.95, 0.0), (1.0, 0.0)])
    tear = smooth((t - 0.58) / 0.06) * (1.0 - smooth((t - 0.84) / 0.06))                # 撕扯包络
    shake = math.sin(math.tau * 3.0 * (t - 0.58)) * tear
    neck_bend(pose, pitch=30.0 * coil - 62.0 * lunge, side=10.0 * shake, taper=0.85)
    neck_wave(pose, t, 2.5 * coil + 6.0 * tear, cycles=3, lag=0.08)
    coil_ripple(pose, t, 0.8 * coil + 0.5 * lunge, cycles=2)
    turn(pose, "root", pos=(0, 0, 0.6 * coil - 0.8 * lunge))
    turn(pose, "head", rot=(-6.0 * coil - 24.0 * lunge, 24.0 * shake, 12.0 * shake))
    jaw(pose, gape)
    flick = math.sin(math.tau * 6.0 * t) * coil * (1.0 - lunge)
    tongue(pose, 0.5 * flick + 0.4 * coil - 0.2, spread=abs(flick))
    shreds(pose, t, 6.0 + 6.0 * tear, cycles=3, lift=10.0 * lunge)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.3 颈微后仰一缩；0.3–0.5 小幅前探；0.5–0.6 一口咬合；0.6–1 回缩
    coil = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.42, 0.4), (0.5, 0.0), (1.0, 0.0)])
    lunge = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.46, 1.0), (0.62, 0.7), (0.8, 0.0), (1.0, 0.0)])
    gape = keys(t, [(0.0, 0.0), (0.24, 0.8), (0.46, 1.0), (0.54, -0.8), (0.7, 0.0), (1.0, 0.0)])
    neck_bend(pose, pitch=16.0 * coil - 32.0 * lunge, side=5.0 * wave(t, 2, 0.0) * lunge, taper=0.85)
    neck_wave(pose, t, 2.0 * coil, cycles=2)
    turn(pose, "head", rot=(-4.0 * coil - 12.0 * lunge, 0, 0))
    jaw(pose, gape)
    tongue(pose, 0.3 * coil - 0.1, spread=0.3 * coil)
    shreds(pose, t, 5.0, cycles=3, lift=6.0 * lunge)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.5, 0.0)])         # 受击冲量：快进慢回
    shiver = math.sin(math.tau * 5.0 * t) * (1.0 - smooth((t - 0.14) / 0.3)) * smooth(t / 0.04)
    neck_bend(pose, pitch=38.0 * kick, side=-18.0 * kick, taper=0.9)
    neck_wave(pose, t, 5.0 * shiver, cycles=3, lag=0.1)
    coil_ripple(pose, t, 1.4 * kick * math.sin(math.tau * 4.0 * t), cycles=1, lag=0.1)
    turn(pose, "head", rot=(12.0 * kick, -8.0 * kick, 6.0 * kick))
    jaw(pose, 1.0 * kick)
    tongue(pose, -0.5 * kick)
    shreds(pose, t, 12.0 * shiver, cycles=3, lift=16.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    twitch = keys(t, [(0.0, 0.0), (0.08, 1.0), (0.25, 0.6), (0.4, 0.0), (1.0, 0.0)])
    # 颈由根到梢一节一节软倒（前倾为负）：越靠近颈梢，开始得越晚、倒得越深
    for k, bone in enumerate(NECK):
        start = 0.2 + 0.28 * k / N_NECK
        fall = keys(t, [(start, 0.0), (start + 0.4, 1.0), (1.0, 1.0)])
        turn(pose, bone, rot=(-(8.0 + 2.5 * k) * fall + 2.0 * twitch * wave(t, 3, 0.1 * k), 0, 3.0 * twitch * wave(t, 4, 0.12 * k) + 1.5 * fall))
    relax = keys(t, [(0.3, 0.0), (0.9, 1.0), (1.0, 1.0)])
    for k, bone in enumerate(COIL):                                 # 盘圈松散：各环组向外微微散开
        turn(pose, bone, rot=(0, 1.3 * relax * (1.0 if k % 2 else -1.0), 0))
    turn(pose, "head", rot=(-14.0 * relax + 8.0 * twitch, 6.0 * relax, 8.0 * relax))
    jaw(pose, 0.6 * twitch + 0.8 * relax)                           # 下颌松开垂落
    tongue(pose, 0.8 * twitch - 0.9 * relax)
    shreds(pose, t, 4.0 * (1.0 - relax), cycles=2, lift=-8.0 * relax)
    return pose


CLIPS = {
    "idle": (3.4, True, idle),
    "walk": (1.4, True, walk),
    "strike": (1.3, False, strike),
    "attack": (1.1, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.6, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 8.0          # 一帧最多托起多少 px；超过说明动作本身把身体压进地里了，是动作错而不是需要托


def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < -GROUND_TOLERANCE:
            turn(pose, "root", pos=(0, -depth, 0))
        return pose

    return sample


def check_ground(clip: str, rig: PoseRig, sampler: Sampler) -> tuple[float, float]:
    """返回 (最低点, 最高腾空) 的 px，并在入地深度超过 MAX_LIFT 时直接报错。"""
    lowest, highest = math.inf, -math.inf
    for step in range(SAMPLES + 1):
        height = rig.lowest(sampler(step / SAMPLES))
        lowest, highest = min(lowest, height), max(highest, height)
    if lowest < -MAX_LIFT:
        raise ValueError(f"{NAME}.{clip}: 最低点 {lowest:.2f}px，入地超过 {MAX_LIFT}px，动作把身体压进地里了")
    return lowest, highest


def seam_gap(a: Pose, b: Pose) -> float:
    """两个姿态的最大旋转差（度）。循环动画首尾必须接上。"""
    return max((max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)) for bone in set(a) | set(b)), default=0.0)


def build() -> list[dict]:
    rig = PoseRig(SOURCE)
    animations = []
    for clip, (length, loop, raw) in CLIPS.items():
        if loop:
            gap = seam_gap(raw(0.0), raw(1.0))
            if gap > 0.5:
                raise ValueError(f"{NAME}.{clip}: 循环首尾旋转差 {gap:.2f}° 接不上")
        sampler = grounded(rig, raw)
        lowest, highest = check_ground(clip, rig, sampler)
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})
        print(f"  {clip:7s} {length:.2f}s {'loop' if loop else 'once'}  最低点 {lowest:+.2f}px  最高 {highest:+.2f}px  骨轨道 {len(tracks)}")
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
