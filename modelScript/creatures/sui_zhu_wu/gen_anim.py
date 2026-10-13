#!/usr/bin/env python3
"""髓蛀蜈动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

五段动画（全部从零写关键帧）：

  idle    3.5s 循环  体节沿身长的呼吸波、触须慢扫、六圈钻环逆向小幅摆、尾钳缓开缓合
  walk    1.4s 循环  波浪步态（每节腿比前一节晚 0.09 周期，左右腿差半周期），体节蛇行，触须探路
  attack  1.0s 一次  前四节拱起后撤蓄力 → 钻头六圈逆向高速旋转前冲 → 尾钳合拢 → 收回
  hurt    0.5s 一次  整条身子 S 形一缩，头甩起，腿收，触须抖
  death   2.0s 一次  抽搐 → 身体侧向盘卷 + 翻成腹部朝天 → 腿向腹下蜷起，钻环停转

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
符号约定（对着渲染逐段核过）：
  · 体节链 seg_0 → seg_15 是父子链，seg_i 的旋转会带动它后面所有体节，所以身体弯曲用「每节小角度累加」；
  · rot.x 为正 = 该体节之前的一端抬起；rot.y 的符号只决定蛇行向哪边起头，左右对称无所谓；
  · 腿：右腿（+X）rot.z 为正 = 抬起、左腿取反；rot.y 为正 = 右腿向前（-Z）扫、左腿取反；
  · 钻环绕 Z 轴转（rot.z），相邻两圈反向；尾钳 rot.y：右钳为负 = 合拢，左钳取反。

用法::

    python3 modelScript/creatures/sui_zhu_wu/gen_anim.py
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
sys.path.insert(0, str(HERE.parent.parent / "tools"))

NAME = "sui_zhu_wu"
SOURCE = HERE / "SuiZhuWu_tex.bbmodel"
ANIMATED = HERE / "SuiZhuWu_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 32                        # 蜈蚣的相位波比四足密，每段 32 个采样

N_SEG = 16
N_RING = 6
Sampler = Callable[[float], Pose]
SIDES = (("l", -1.0), ("r", 1.0))   # (骨名后缀, 符号)：右侧在 +X


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


def frac(x: float) -> float:
    return x - math.floor(x)


def turn(pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0)) -> None:
    """往 pose 里累加一根骨的旋转 / 位移（同一根骨多处调用会叠加）。"""
    channel = pose[bone]
    channel.rot = [channel.rot[i] + rot[i] for i in range(3)]
    channel.pos = [channel.pos[i] + pos[i] for i in range(3)]


# ---------------------------------------------------------------- 腿 / 触须 / 尾钳
def leg(pose: Pose, i: int, side: str, sign: float, sweep: float, lift: float, fold: float = 0.0) -> None:
    """第 i 节 side 侧的腿：sweep > 0 向前扫，lift > 0 抬起，fold > 0 把腿往腹下蜷（死态用）。"""
    a, b, c = (f"leg_{i}_{j}_{side}" for j in "abc")
    turn(pose, a, rot=(0, sign * sweep, sign * (lift * 0.7 + fold * 0.5)))
    turn(pose, b, rot=(0, 0, sign * (lift * 1.1 + fold * 0.9)))
    turn(pose, c, rot=(0, 0, sign * (-lift * 0.3 + fold * 0.7)))


def leg_cycle(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (扫角, 抬腿弧)。支撑相脚从前往后划，摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    s = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(s) - 1.0), math.sin(math.pi * s)


def legs_walk(pose: Pose, t: float, swing: float, lift: float, lag: float = 0.09, cycles: int = 1,
              first: int = 0, last: int = N_SEG) -> None:
    """波浪步态：第 i 节腿比前一节晚 lag 个周期，右腿比左腿晚半周期。"""
    for i in range(first, last):
        for side, sign in SIDES:
            u = frac(cycles * t - lag * i + (0.5 if sign > 0 else 0.0))
            angle, arc = leg_cycle(u, 0.6, swing)
            leg(pose, i, side, sign, angle, lift * arc)


def antennae(pose: Pose, t: float, yaw: float, pitch: float, cycles: float = 1.0, base_yaw: float = 0.0) -> None:
    """两条三节触须：越靠尖端相位越晚、幅度越大；两侧反相。"""
    for side, sign in SIDES:
        for k in range(1, 4):
            turn(pose, f"antenna_{k}_{side}", rot=(pitch * wave(t, cycles, 0.7 - 0.11 * k) * k / 3,
                                                   sign * (base_yaw + yaw * wave(t, cycles, -0.12 * k)) * k / 3, 0))


def pincers(pose: Pose, openness: float) -> None:
    """openness > 0 张开，< 0 合拢（度）。右钳 rot.y 为负 = 合拢。"""
    for side, sign in SIDES:
        turn(pose, f"pincer_base_{side}", rot=(0, sign * openness * 0.6, 0))
        turn(pose, f"pincer_tip_{side}", rot=(0, sign * openness * 0.9, 0))


def rings(pose: Pose, angle: float) -> None:
    """六圈钻环逆向旋转：相邻两圈反向，越靠近钻尖转得越快。"""
    for k in range(N_RING):
        turn(pose, f"ring_{k}", rot=(0, 0, (1.0 if k % 2 == 0 else -1.0) * angle * (1.0 + 0.12 * k)))


# ---------------------------------------------------------------- 五段动画
def idle(t: float) -> Pose:
    pose = Pose()
    for i in range(N_SEG):
        # 呼吸波沿身长传，体节微微俯仰；整条身子几乎不动
        turn(pose, f"seg_{i}", rot=(0.9 * wave(t, 1, -0.05 * i), 0.5 * wave(t, 1, 0.3 - 0.04 * i), 0))
    turn(pose, "head", rot=(1.5 * wave(t, 1, 0.1), 2.0 * wave(t, 1, 0.35), 0))
    antennae(pose, t, yaw=14.0, pitch=8.0)
    rings(pose, 10.0 * wave(t, 1, 0.2))
    for i in range(0, N_SEG, 2):                        # 隔节腿尖轻点一下，像在探地面
        for side, sign in SIDES:
            tap = max(0.0, wave(t, 2, 0.13 * i + (0.5 if sign > 0 else 0.0))) ** 4
            leg(pose, i, side, sign, 2.0 * tap, 7.0 * tap)
    pincers(pose, 6.0 + 6.0 * wave(t, 1, 0.4))
    turn(pose, "telson", rot=(0, 3.0 * wave(t, 1, 0.5), 0))
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    legs_walk(pose, t, swing=17.0, lift=24.0)
    for i in range(N_SEG):
        # 体节蛇行：每节小角度，沿身长传的波；稍带俯仰，随腿的抬落起伏
        turn(pose, f"seg_{i}", rot=(1.3 * wave(t, 2, -0.09 * i + 0.25), 3.4 * wave(t, 1, -0.09 * i), 0))
    turn(pose, "head", rot=(0, -5.0 * wave(t, 1, -0.02), 0))      # 头在前面带路，反向摆
    antennae(pose, t, yaw=22.0, pitch=10.0, cycles=2, base_yaw=6.0)
    rings(pose, 24.0 * wave(t, 1, 0.1))                          # 行走时钻环缓慢逆向摆
    pincers(pose, 8.0 * wave(t, 1, 0.25))
    turn(pose, "telson", rot=(0, 4.0 * wave(t, 1, -0.9), 0))
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.3 前四节拱起后撤蓄力；0.3–0.55 钻头旋转前冲；0.55–0.7 尾钳合拢；0.7–1 收回
    rise = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.42, 0.55), (0.55, -0.45), (0.7, -0.1), (1.0, 0.0)])
    lunge = keys(t, [(0.0, 0.0), (0.3, -0.22), (0.55, 1.0), (0.7, 0.8), (1.0, 0.0)])
    spin = 1500.0 * keys(t, [(0.0, 0.0), (0.3, 0.1), (0.55, 0.75), (0.75, 1.0), (1.0, 1.0)])
    snap = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.55, 1.0), (0.64, -1.0), (0.8, -0.4), (1.0, 0.0)])
    # 前四节依次抬起、后四节反向补偿，只有前段拱起，后面的身体还在地上
    for i in range(4):
        turn(pose, f"seg_{i}", rot=(9.0 * rise, 0, 0))
    for i in range(4, 8):
        turn(pose, f"seg_{i}", rot=(-9.0 * rise, 0, 0))
    for i in range(N_SEG):
        turn(pose, f"seg_{i}", rot=(0, 1.2 * wave(t, 3, -0.09 * i) * smooth(t / 0.3) * (1.0 - smooth((t - 0.7) / 0.3)), 0))
    turn(pose, "root", pos=(0, 0, 1.2 * rise * (1.0 if rise > 0 else 0.0) - 9.0 * lunge))
    turn(pose, "head", rot=(-14.0 * lunge + 6.0 * rise, 0, 0))
    rings(pose, spin)
    antennae(pose, t, yaw=10.0, pitch=4.0, cycles=3, base_yaw=-26.0 * lunge)     # 前冲时触须向后抿
    pincers(pose, 22.0 * snap)
    turn(pose, "telson", rot=(-6.0 * lunge, 0, 0))
    for i in range(N_SEG):
        for side, sign in SIDES:
            u = frac(2.0 * t - 0.09 * i + (0.5 if sign > 0 else 0.0))
            angle, arc = leg_cycle(u, 0.55, 20.0)
            leg(pose, i, side, sign, angle * lunge, 26.0 * arc * (0.3 + 0.7 * abs(lunge)))
    for i in range(4):                                     # 蓄力时前四节的腿向两侧撑开
        for side, sign in SIDES:
            leg(pose, i, side, sign, 10.0 * rise, 18.0 * rise)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.18, 1.0), (0.5, 0.0)])            # 受击冲量：快进慢回
    for i in range(N_SEG):
        # S 形一缩：前后反向弯，波往后传
        turn(pose, f"seg_{i}", rot=(-1.4 * kick * (1.0 if i < 5 else 0.0), 6.0 * kick * math.sin(math.pi * (i + 1) / 8.0), 0))
    turn(pose, "root", pos=(0, 0, 1.8 * kick))
    turn(pose, "head", rot=(10.0 * kick, -8.0 * kick, 3.0 * kick))
    rings(pose, 40.0 * kick)
    antennae(pose, t, yaw=30.0 * kick, pitch=12.0 * kick, cycles=4, base_yaw=14.0 * kick)
    pincers(pose, 16.0 * kick)
    for i in range(N_SEG):
        for side, sign in SIDES:
            leg(pose, i, side, sign, -9.0 * kick, 14.0 * kick, fold=6.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    twitch = keys(t, [(0.0, 0.0), (0.1, 1.0), (0.3, 0.8), (0.4, 0.0), (1.0, 0.0)])
    curl = keys(t, [(0.0, 0.0), (0.25, 0.0), (0.7, 1.0), (1.0, 1.0)])         # 侧向盘卷
    flip = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.72, 1.0), (1.0, 1.0)])         # 翻成腹部朝天
    tuck = keys(t, [(0.0, 0.0), (0.3, 0.1), (0.8, 1.0), (1.0, 1.0)])          # 腿向腹下蜷起
    for i in range(N_SEG):
        turn(pose, f"seg_{i}", rot=(-1.0 * twitch * (1.0 if i < 5 else 0.0), 6.5 * curl + 2.0 * twitch * wave(t, 3, -0.1 * i), 0))
    turn(pose, "seg_0", rot=(0, 0, 150.0 * flip))
    turn(pose, "head", rot=(8.0 * twitch, 0, 0))
    rings(pose, 120.0 * keys(t, [(0.0, 0.0), (0.3, 1.0), (0.6, 1.2), (1.0, 1.2)]))   # 钻环转着转着停在半途
    antennae(pose, t, yaw=18.0 * (1.0 - tuck), pitch=6.0 * (1.0 - tuck), cycles=3, base_yaw=34.0 * tuck)
    pincers(pose, 26.0 * keys(t, [(0.0, 0.0), (0.3, 1.0), (1.0, 1.0)]))               # 尾钳张开不再合拢
    for i in range(N_SEG):
        for side, sign in SIDES:
            leg(pose, i, side, sign, 0.0, 4.0 * twitch * wave(t, 4, 0.07 * i), fold=52.0 * tuck)
    return pose


CLIPS = {
    "idle": (3.5, True, idle),
    "walk": (1.4, True, walk),
    "attack": (1.0, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.0, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 6.0          # 一帧最多托起多少 px；超过说明动作本身把身体压进地里了


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
