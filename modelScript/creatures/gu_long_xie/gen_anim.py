#!/usr/bin/env python3
"""骨笼蟹动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

五段动画（全部从零写关键帧）：

  idle    3.2s 循环  驻足：颅骨壳随呼吸微起伏，两根眼柄各自慢慢张望，触须扫探，大螯缓缓开合、小螯偶尔颤，口须不停翕动
  walk    1.2s 循环  横列步态：四对步足逐对错相（前对先、后对后；左右错半周期），身体随步子侧晃 / 起伏，双螯半举前探，眼柄稳住
  attack  1.3s 一次  大螯重击：壳身后仰、大螯外撇高举并张开 → 身子前冲、大螯内扫前劈、钳口猛合 → 小螯补一下 → 收回
  hurt    0.5s 一次  受击：壳身后仰一抖，眼柄缩回，双螯炸开、步足撑开，触须乱甩
  death   2.6s 一次  抽搐 → 步足逐对软折、身子沉到地上 → 眼柄软垂、双螯松开摊在身前，腹部松卷、触须垂落

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
符号约定（对着数值核过；s = +1 右侧 / -1 左侧，写成 s·角度）：
  · 步足：leg_a rot.y 为正（右）= 足向前（-Z）摆，rot.z 为正（右）= 抬腿；leg_b rot.z 为正（右）= 小腿抬起；
  · 螯（大螯在右）：arm / fore rot.x 为正 = 螯向上抬（并略后撤），rot.y 为正 = 螯向内前扫，rot.z 为正 = 螯向外上抬；
    dactyl rot.x 为正 = 钳口张开（rest 时钳口是合着的）；
  · 眼柄 eye_a rot.x 为正 = 向后仰（缩回），rot.z 为 s·正 = 向内偏；触须 ant_k rot.y 为 s·正 = 向内摆；
  · 腹 ab_k rot.x 为正 = 向下卷；body rot.x 为正 = 机头（螯 / 眼）抬起。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/gu_long_xie/gen_anim.py
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

import gen_gu_long_xie as geo  # noqa: E402  步足对数、骨名取建模稿的权威定义

NAME = "gu_long_xie"
SOURCE = HERE / "GuLongXie_tex.bbmodel"
ANIMATED = HERE / "GuLongXie_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 24

LEG_PAIRS = geo.LEG_PAIRS
SIDES = (("l", -1.0), ("r", 1.0))   # (骨名后缀, 符号)：右侧在 +X
Sampler = Callable[[float], Pose]


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


# ---------------------------------------------------------------- 部件
def leg(pose: Pose, p: int, side: str, s: float, sweep: float, lift: float, bend: float = 0.0) -> None:
    """第 p 对 side 侧的足：sweep > 0 向前扫，lift > 0 抬腿，bend > 0 小腿再抬起（度）。"""
    turn(pose, f"leg{p}_a_{side}", rot=(0, s * sweep, s * lift))
    turn(pose, f"leg{p}_b_{side}", rot=(0, 0, s * bend))


def leg_cycle(u: float, duty: float, swing: float) -> tuple[float, float]:
    """腿的相位 u∈[0,1) → (扫角, 抬腿弧)。支撑相脚从前往后划，摆动相脚抬起送回前方。"""
    if u < duty:
        return swing * (1.0 - 2.0 * u / duty), 0.0
    k = (u - duty) / (1.0 - duty)
    return swing * (2.0 * smooth(k) - 1.0), math.sin(math.pi * k)


def eyes(pose: Pose, t: float, scan: float, retract: float = 0.0, droop: float = 0.0, cycles: float = 1.0) -> None:
    """两根眼柄：scan = 各自慢慢张望的幅度（度），retract = 缩回（向后仰）的角度，droop = 软垂（向前扑倒）的角度。"""
    for side, s in SIDES:
        lag = 0.0 if s > 0 else 0.37                              # 两只眼不同步
        turn(pose, f"eye_a_{side}", rot=(scan * wave(t, cycles, lag) + retract - droop, 0, s * scan * 1.2 * wave(t, cycles, lag + 0.25)))
        turn(pose, f"eye_b_{side}", rot=(scan * 0.6 * wave(t, cycles, lag - 0.1), 0, s * scan * 0.6 * wave(t, cycles, lag + 0.15)))


def antennae(pose: Pose, t: float, sway: float, cycles: float = 1.0, in_deg: float = 0.0, limp: float = 0.0) -> None:
    """触须三节：越靠末梢相位越晚、幅度越大；in_deg 向内扫，limp 向下垂。"""
    for side, s in SIDES:
        for k in range(3):
            turn(pose, f"ant_{k}_{side}", rot=(limp * (k + 1) / 3.0, s * (in_deg + sway * wave(t, cycles, -0.12 * k + (0.0 if s > 0 else 0.3))) * (k + 1) / 3.0, 0))


def mouthparts(pose: Pose, t: float, amount: float, cycles: float = 2.0) -> None:
    for side, s in SIDES:
        turn(pose, f"mouth_{side}", rot=(amount * wave(t, cycles, 0.0 if s > 0 else 0.5), s * amount * 0.6 * wave(t, cycles, 0.25), 0))


def abdomen(pose: Pose, t: float, sway: float, curl: float = 0.0, cycles: float = 1.0) -> None:
    """腹三节 + 尾扇：curl > 0 向下卷（度，逐节累加），sway 是左右轻晃。"""
    for k in range(3):
        turn(pose, f"ab_{k}", rot=(curl, sway * wave(t, cycles, -0.1 * k), 0))
    turn(pose, "tailfan", rot=(curl * 0.5, sway * 1.5 * wave(t, cycles, -0.3), 0))


def claw(pose: Pose, name: str, s: float, raise_: float = 0.0, out: float = 0.0, sweep: float = 0.0, open_: float = 0.0,
         fore_raise: float = 0.0) -> None:
    """大 / 小螯：raise_ 抬臂，out 向外上抬，sweep 向内前扫，open_ 钳口张开角，fore_raise 前臂再抬（度）。"""
    turn(pose, f"{name}_arm", rot=(raise_, s * sweep, s * out))
    turn(pose, f"{name}_fore", rot=(fore_raise, s * sweep * 0.5, 0))
    turn(pose, f"{name}_dactyl", rot=(open_, 0, 0))


def mosses(pose: Pose, t: float, sway: float) -> None:
    for bone, phase in (("moss_front", 0.0), ("moss_l", 0.3), ("moss_r", 0.6)):
        turn(pose, bone, rot=(sway * wave(t, 2, phase), 0, sway * 0.7 * wave(t, 2, phase + 0.25)))


# ---------------------------------------------------------------- 五段动画
def idle(t: float) -> Pose:
    pose = Pose()
    breathe = wave(t, 1, 0.0)
    turn(pose, "body", rot=(0.9 * breathe, 0, 0.6 * wave(t, 1, 0.3)))
    turn(pose, "shell", rot=(0.6 * wave(t, 1, 0.1), 1.2 * wave(t, 1, 0.55), 0.8 * wave(t, 1, 0.2)))
    eyes(pose, t, 9.0)
    antennae(pose, t, 14.0, in_deg=4.0)
    mouthparts(pose, t, 6.0)
    abdomen(pose, t, 2.0, curl=0.5 * breathe)
    claw(pose, "claw_big", 1.0, raise_=2.5 * wave(t, 1, 0.2), open_=7.0 + 7.0 * wave(t, 1, 0.1), sweep=1.5 * wave(t, 1, 0.5))
    claw(pose, "claw_small", -1.0, raise_=2.0 * wave(t, 2, 0.4), open_=max(0.0, wave(t, 2, 0.0)) ** 3 * 12.0)
    mosses(pose, t, 3.0)
    for p in range(LEG_PAIRS):                                   # 隔对足轻轻点地换重心
        for side, s in SIDES:
            tap = max(0.0, wave(t, 1, 0.25 * p + (0.0 if s > 0 else 0.5))) ** 6
            leg(pose, p, side, s, 2.0 * tap, 3.0 * tap, 5.0 * tap)
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    for p in range(LEG_PAIRS):
        for side, s in SIDES:
            u = frac(t - 0.25 * p + (0.5 if s > 0 else 0.0))
            sweep, arc = leg_cycle(u, 0.6, 16.0)
            leg(pose, p, side, s, sweep, 14.0 * arc, 20.0 * arc)
    bob = abs(wave(t, 1, 0.0))                                   # 每迈一步（半周期）起伏一次
    turn(pose, "root", pos=(0, 0.5 * bob, 0))
    turn(pose, "body", rot=(1.2 * wave(t, 2, 0.1), 2.0 * wave(t, 1, 0.0), 1.8 * wave(t, 1, 0.25)))
    turn(pose, "shell", rot=(0.8 * wave(t, 2, 0.35), 1.5 * wave(t, 1, 0.5), 1.0 * wave(t, 1, 0.1)))
    eyes(pose, t, 3.0, cycles=2)                                 # 眼柄稳住，只有轻微补偿
    antennae(pose, t, 10.0, cycles=2, in_deg=2.0)
    mouthparts(pose, t, 8.0, cycles=3)
    abdomen(pose, t, 3.5, curl=0.6 * wave(t, 2, 0.2), cycles=1)
    claw(pose, "claw_big", 1.0, raise_=7.0 + 3.0 * wave(t, 2, 0.2), sweep=4.0 * wave(t, 1, 0.0), open_=5.0 + 5.0 * wave(t, 1, 0.3))
    claw(pose, "claw_small", -1.0, raise_=6.0 + 2.0 * wave(t, 2, 0.6), sweep=5.0 * wave(t, 1, 0.5), open_=4.0 * max(0.0, wave(t, 2, 0.1)))
    mosses(pose, t, 4.0)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.3 壳身后仰、大螯外撇高举并张开；0.3–0.5 身子前冲、大螯内扫前劈；0.5–0.58 钳口猛合；
    #       0.58–0.78 小螯补一下；0.78–1 收回
    wind = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.4, 0.8), (0.5, 0.0), (1.0, 0.0)])         # 蓄力（高举 / 后仰）
    strike = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.5, 1.0), (0.62, 0.8), (0.82, 0.1), (1.0, 0.0)])   # 前劈
    jaw = keys(t, [(0.0, 0.0), (0.12, 0.4), (0.3, 1.0), (0.5, 1.0), (0.56, -0.1), (0.66, 0.0), (1.0, 0.0)])   # 钳口张开度
    jab = keys(t, [(0.0, 0.0), (0.56, 0.0), (0.68, 1.0), (0.8, 0.0), (1.0, 0.0)])         # 小螯补刀
    turn(pose, "body", rot=(8.0 * wind - 4.0 * strike, 0, 0))
    turn(pose, "shell", rot=(-6.0 * strike, 0, 0))
    turn(pose, "root", pos=(0, 0, -7.0 * strike + 1.5 * wind))
    claw(pose, "claw_big", 1.0, raise_=24.0 * wind - 8.0 * strike, out=22.0 * wind, sweep=34.0 * strike, open_=40.0 * jaw,
         fore_raise=10.0 * wind)
    claw(pose, "claw_small", -1.0, raise_=10.0 * wind, sweep=20.0 * jab, open_=20.0 * jab)
    eyes(pose, t, 2.0, retract=22.0 * strike)                    # 前冲时眼柄缩回
    antennae(pose, t, 6.0, cycles=2, in_deg=-14.0 * wind + 20.0 * strike)
    mouthparts(pose, t, 6.0 + 10.0 * strike, cycles=3)
    abdomen(pose, t, 2.0, curl=6.0 * strike)
    for p in range(LEG_PAIRS):                                   # 蓄力时步足撑稳、前冲时蹬地
        for side, s in SIDES:
            leg(pose, p, side, s, 7.0 * strike - 5.0 * wind, 6.0 * wind, 0.0)
    mosses(pose, t, 5.0 * strike)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.5, 0.0), (1.0, 0.0)])         # 受击冲量：快进慢回
    shake = wave(t, 5, 0.0) * kick
    turn(pose, "body", rot=(9.0 * kick, 0, 5.0 * shake))
    turn(pose, "shell", rot=(0, 4.0 * shake, 6.0 * shake))
    turn(pose, "root", pos=(0, 0, 3.0 * kick))
    eyes(pose, t, 2.0, retract=34.0 * kick)
    antennae(pose, t, 24.0 * kick, cycles=4, in_deg=10.0 * kick)
    mouthparts(pose, t, 12.0 * kick, cycles=4)
    abdomen(pose, t, 5.0 * kick, curl=8.0 * kick, cycles=3)
    claw(pose, "claw_big", 1.0, raise_=14.0 * kick, out=26.0 * kick, open_=24.0 * kick)
    claw(pose, "claw_small", -1.0, raise_=10.0 * kick, out=22.0 * kick, open_=18.0 * kick)
    for p in range(LEG_PAIRS):
        for side, s in SIDES:
            leg(pose, p, side, s, -6.0 * kick, 10.0 * kick, 12.0 * kick)
    mosses(pose, t, 8.0 * kick)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    twitch = keys(t, [(0.0, 0.0), (0.08, 1.0), (0.25, 0.7), (0.4, 0.0), (1.0, 0.0)])
    sink = keys(t, [(0.2, 0.0), (0.7, 1.0), (1.0, 1.0)])                       # 身子沉下来
    limp = keys(t, [(0.3, 0.0), (0.8, 1.0), (1.0, 1.0)])                       # 眼柄 / 螯 / 触须松垂
    turn(pose, "body", rot=(-8.0 * sink + 6.0 * twitch, 0, 4.0 * twitch * wave(t, 3, 0.0) + 3.0 * sink))
    turn(pose, "shell", rot=(0, 3.0 * twitch * wave(t, 4, 0.0), 0))
    turn(pose, "root", pos=(0, -5.0 * sink, 0))
    eyes(pose, t, 3.0 * (1.0 - limp), retract=20.0 * twitch, droop=62.0 * limp)
    antennae(pose, t, 10.0 * (1.0 - limp), cycles=3, in_deg=-8.0 * limp, limp=-26.0 * limp)
    mouthparts(pose, t, 8.0 * twitch, cycles=4)
    abdomen(pose, t, 2.0 * twitch, curl=-14.0 * limp, cycles=3)               # 腹部松卷（向上翘开）
    claw(pose, "claw_big", 1.0, raise_=-10.0 * limp + 8.0 * twitch, out=12.0 * limp, open_=26.0 * limp)
    claw(pose, "claw_small", -1.0, raise_=-8.0 * limp, out=10.0 * limp, open_=18.0 * limp)
    for p in range(LEG_PAIRS):
        # 步足逐对软下来：前对先、后对后，微微收起、小腿软折，壳身随之沉到地上
        fold = keys(t, [(0.15 + 0.1 * p, 0.0), (0.55 + 0.08 * p, 1.0), (1.0, 1.0)])
        for side, s in SIDES:
            leg(pose, p, side, s, 6.0 * fold * (1.0 if p % 2 else -1.0), 15.0 * fold + 4.0 * twitch * wave(t, 4, 0.1 * p), 30.0 * fold)
    mosses(pose, t, 4.0 * twitch + 10.0 * limp)
    return pose


CLIPS = {
    "idle": (3.2, True, idle),
    "walk": (1.2, True, walk),
    "attack": (1.3, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.6, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
# 死亡段整只往下沉（步足蜷起后壳身贴地），所以只托不压。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 12.0


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
    """两个姿态的最大旋转 / 位移差。循环动画首尾必须接上。"""
    bones = set(a) | set(b)
    rot = max((max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)) for bone in bones), default=0.0)
    pos = max((max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)) for bone in bones), default=0.0)
    return max(rot, pos * 2.0)


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
