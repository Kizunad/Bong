#!/usr/bin/env python3
"""锈甲鳖虫 GeckoLib 动画：idle / walk / attack / hurt / death + 翻身三段（flip / flail / righting）+ 背甲开合（shell_open）。

只落资产，不接线、不装进 client：
  - XiuJiaBie_tex_animated.bbmodel   带 animations 的贴图版
  - out/xiu_jia_bie.animation.json   经 creature_codec 导出的 GeckoLib 动画（暂存目录，不进资源树）
  - model-review/anim/xiu_jia_bie/*.gif

坐标约定：模型面朝 -Z（游戏约定），动画直接按 bbmodel 空间写。
腿：rot.y = side * 前摆量（side：右 +1 / 左 -1，正 = 脚向 -Z 前摆）；
    rot.z = side * 抬腿量（正 = 脚向上抬，腿往身下收）。
钳颚：rot.y = -side * 张开量（正 = 向外张）。
鞘翅：左 rot.z = -开角，右 rot.z = +开角（外缘向上掀）。
翻身：绕 root 的 Z 轴（虫身长轴）转 180°，抬高量由 grounded() 兜底。

先跑 gen_xiu_jia_bie_tex.py，再跑本文件：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/xiu_jia_bie/gen_xiu_jia_bie_anim.py
"""

from __future__ import annotations

import math
import subprocess
import sys
from pathlib import Path
from typing import Callable

CREATURE_DIR = Path(__file__).resolve().parent
REPO = CREATURE_DIR.parents[2]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "exporters"))

from bbmodel_maker.render import framing  # noqa: E402
from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, write_animated_bbmodel  # noqa: E402
from gif_timing import gif_schedule  # noqa: E402
from PIL import Image  # noqa: E402
from review_creature_assets import focus  # noqa: E402

TEX_MODEL = CREATURE_DIR / "XiuJiaBie_tex.bbmodel"
ANIMATED_MODEL = CREATURE_DIR / "XiuJiaBie_tex_animated.bbmodel"
STAGING = CREATURE_DIR / "out"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
GIF_DIR = Path(__file__).resolve().parents[2] / "out" / "review" / "anim" / "xiu_jia_bie"
MODEL_NAME = "xiu_jia_bie"  # creature_codec 只收小写蛇形名

SAMPLES = 24
GIF_FPS = 20
GIF_BACKGROUND = (22, 23, 26)

Sampler = Callable[[float], Pose]

LEG_NAMES = ("front", "mid", "rear")
SIDES = (("l", -1), ("r", 1))
TRIPOD_A = {("front", "l"), ("mid", "r"), ("rear", "l")}  # 另一组是 front_r / mid_l / rear_r，错半周期
SEGS = ("seg_1", "seg_2", "seg_3", "seg_4")
SHELL_BONES = ("shell_pronotum", "shell_elytra_l", "shell_elytra_r", "shell_rear_l", "shell_rear_r",
               "shell_skirt_l", "shell_skirt_r")


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    return math.sin(math.tau * (cycles * t + phase))


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep：points 是 (t01, 值) 有序表，一次性动作用它排节奏。"""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


def turn(pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0)) -> None:
    channel = pose[bone]
    channel.rot = [a + b for a, b in zip(channel.rot, rot)]
    channel.pos = [a + b for a, b in zip(channel.pos, pos)]


def leg(pose: Pose, name: str, side_name: str, swing: float, lift: float, fold: float = 0.0) -> None:
    """一条腿：swing 前摆（°）、lift 整腿抬起（°）、fold 胫节再往身下收（°）。左右符号在这里统一。"""
    side = 1 if side_name == "r" else -1
    prefix = f"leg_{name}_{side_name}"
    turn(pose, f"{prefix}_femur", rot=(0.0, side * swing, side * lift))
    turn(pose, f"{prefix}_tibia", rot=(0.0, 0.0, side * fold))
    turn(pose, f"{prefix}_tarsus", rot=(0.0, 0.0, -side * fold * 0.4))


def all_legs(pose: Pose, fn: Callable[[str, str, int], tuple[float, float, float]]) -> None:
    for i, name in enumerate(LEG_NAMES):
        for side_name, _ in SIDES:
            swing, lift, fold = fn(name, side_name, i)
            leg(pose, name, side_name, swing, lift, fold)


def mandibles(pose: Pose, open_deg: float, tip: float = 0.0) -> None:
    """open_deg > 0 = 双钳向外张；tip = 尖节再多张 / 合的量。"""
    for side_name, _ in SIDES:
        side = 1 if side_name == "r" else -1
        turn(pose, f"mand_{side_name}1", rot=(0.0, -side * open_deg, 0.0))
        turn(pose, f"mand_{side_name}2", rot=(0.0, -side * tip, 0.0))


def elytra(pose: Pose, open_deg: float, skirt: float = 0.0, rear: float = 0.0, pronotum: float = 0.0) -> None:
    for side_name, _ in SIDES:
        side = 1 if side_name == "r" else -1
        turn(pose, f"shell_elytra_{side_name}", rot=(0.0, 0.0, side * open_deg))
        turn(pose, f"shell_skirt_{side_name}", rot=(0.0, 0.0, side * skirt))
        turn(pose, f"shell_rear_{side_name}", rot=(0.0, 0.0, side * rear))
    turn(pose, "shell_pronotum", rot=(-pronotum, 0.0, 0.0))


def antennae(pose: Pose, t: float, amount: float, speed: float = 2.0, droop: float = 0.0) -> None:
    for side_name, _ in SIDES:
        side = 1 if side_name == "r" else -1
        phase = 0.0 if side_name == "l" else 0.37
        turn(pose, f"antenna_{side_name}", rot=(amount * wave(t, speed, phase) - droop, side * amount * 0.6 * wave(t, speed, phase + 0.25), 0.0))


# ---------------------------------------------------------------- 动画
def idle(t: float) -> Pose:
    """3 秒循环：伏着微微起伏，触角探动，钳颚偶尔啃一下，腿各自轻挪。"""
    pose = Pose()
    turn(pose, "root", pos=(0, 0.12 * wave(t, 1), 0))
    for i, seg in enumerate(SEGS):
        turn(pose, seg, rot=(0.6 * wave(t, 1, 0.1 * i), 0.8 * wave(t, 1, 0.15 * i), 0))
    turn(pose, "head", rot=(3.0 * wave(t, 1, 0.2), 4.0 * wave(t, 2), 0))
    antennae(pose, t, 8.0, speed=2.0)
    chew = max(0.0, wave(t, 3, 0.2)) ** 2
    mandibles(pose, 4.0 + 6.0 * chew, tip=-4.0 * chew)
    for i, name in enumerate(LEG_NAMES):
        for side_name, _ in SIDES:
            twitch = max(0.0, math.sin(math.tau * (t * 2 + i * 0.21 + (0.5 if side_name == "r" else 0)))) ** 10
            leg(pose, name, side_name, 3.0 * twitch, 6.0 * twitch, 5.0 * twitch)
    elytra(pose, 0.8 + 0.8 * wave(t, 1, 0.3))                 # 背甲随呼吸微微开合
    turn(pose, "pygidium", rot=(2.0 * wave(t, 1, 0.5), 0, 0))
    return pose


def walk(t: float) -> Pose:
    """1.6 秒一个步周期：三足交替（前左 / 中右 / 后左一组），身子扁平低重心，左右轻扭。"""
    pose = Pose()

    def step(name: str, side_name: str, i: int) -> tuple[float, float, float]:
        phase = 0.0 if (name, side_name) in TRIPOD_A else 0.5
        u = t + phase
        lift = max(0.0, math.cos(math.tau * u))                 # 摆动相抬腿
        amp = (16.0, 12.0, 14.0)[i]
        return amp * math.sin(math.tau * u), 14.0 * lift ** 1.3, 10.0 * lift ** 1.3

    all_legs(pose, step)
    turn(pose, "root", pos=(0, 0.25 * abs(wave(t, 2)), 0))
    for i, seg in enumerate(SEGS):
        turn(pose, seg, rot=(0, 2.4 * wave(t, 1, 0.12 * i), 1.0 * wave(t, 2, 0.1 * i)))
    turn(pose, "head", rot=(2.0 * wave(t, 2, 0.1), 3.0 * wave(t, 1, 0.3), 0))
    antennae(pose, t, 12.0, speed=2.0)
    mandibles(pose, 6.0 + 3.0 * wave(t, 2, 0.1))
    elytra(pose, 0.6 * (1 + wave(t, 2, 0.2)))                  # 走动时甲片轻轻掀合
    turn(pose, "pygidium", rot=(0, 5.0 * wave(t, 1, 0.5), 0))
    return pose


def attack(t: float) -> Pose:
    """1.0 秒一次性：压低 + 钳颚大张蓄势 → 猛地前冲、双钳合拢夹击 → 停一拍 → 收回。"""
    pose = Pose()
    wind = keys(t, [(0.0, 0), (0.32, 1), (0.42, 1), (0.52, 0)])
    bite = keys(t, [(0.0, 0), (0.42, 0), (0.52, 1), (0.72, 1), (1.0, 0)])
    turn(pose, "root", pos=(0, -0.6 * wind, 1.0 * wind - 3.2 * bite))
    turn(pose, "seg_1", rot=(6.0 * wind - 8.0 * bite, 0, 0))
    turn(pose, "seg_2", rot=(2.0 * wind - 3.0 * bite, 0, 0))
    turn(pose, "head", rot=(10.0 * wind - 14.0 * bite, 0, 0))
    mandibles(pose, 38.0 * wind - 30.0 * bite + 6.0, tip=-8.0 * wind + 22.0 * bite)   # 张到最大，再向内合拢夹住
    antennae(pose, t, 4.0, speed=4.0, droop=14.0 * bite)
    for i, name in enumerate(LEG_NAMES):
        for side_name, _ in SIDES:
            if name == "front":
                leg(pose, name, side_name, -8.0 * wind + 20.0 * bite, 6.0 * wind - 4.0 * bite, 4.0 * bite)
            else:
                leg(pose, name, side_name, 10.0 * wind - 16.0 * bite, -2.0 * wind, 2.0 * bite)   # 中后腿蹬地送力
    elytra(pose, 4.0 * wind - 2.0 * bite, pronotum=3.0 * wind)
    turn(pose, "pygidium", rot=(-10.0 * wind + 8.0 * bite, 0, 0))
    return pose


def hurt(t: float) -> Pose:
    """0.5 秒一次性：被打得缩一下，腿收拢，甲片抖开，钳颚本能张开。"""
    pose = Pose()
    k = keys(t, [(0.0, 0), (0.18, 1), (0.5, 0.35), (1.0, 0)])
    shake = wave(t, 4) * k
    turn(pose, "root", pos=(0, -0.5 * k, 1.4 * k), rot=(0, 3.0 * shake, 0))
    turn(pose, "seg_1", rot=(5.0 * k, 0, 0))
    turn(pose, "head", rot=(10.0 * k, 0, 0))
    mandibles(pose, 24.0 * k, tip=8.0 * k)
    antennae(pose, t, 6.0, speed=4.0, droop=-16.0 * k)
    all_legs(pose, lambda name, side, i: (0.0, 12.0 * k, 14.0 * k))
    elytra(pose, 9.0 * k + 3.0 * shake, skirt=6.0 * k, rear=5.0 * k)
    turn(pose, "pygidium", rot=(10.0 * k, 0, 0))
    return pose


def death(t: float) -> Pose:
    """2 秒一次性：腿一根根失力收向身下，身体沉下去，甲片松开垂落，钳颚松垮张开。"""
    pose = Pose()
    sag = keys(t, [(0.0, 0), (0.55, 1), (1.0, 1)])
    twitch = keys(t, [(0.0, 0), (0.15, 1), (0.3, 0), (1.0, 0)])
    for i, name in enumerate(LEG_NAMES):
        for side_name, _ in SIDES:
            delay = 0.12 * i + (0.06 if side_name == "r" else 0.0)
            k = keys(t, [(0.0, 0), (0.2 + delay, 0), (0.7 + delay, 1), (1.0, 1)])
            leg(pose, name, side_name, 6.0 * k, 22.0 * k, 36.0 * k)
    turn(pose, "root", pos=(0, -0.8 * sag, 0), rot=(0, 0, 4.0 * sag))
    turn(pose, "seg_1", rot=(6.0 * sag, 0, 0))
    turn(pose, "seg_4", rot=(-4.0 * sag, 0, 0))
    turn(pose, "head", rot=(14.0 * sag + 4.0 * twitch, 0, 0))
    mandibles(pose, 20.0 * sag, tip=-12.0 * sag)
    antennae(pose, t, 2.0, speed=1.0, droop=-30.0 * sag)
    elytra(pose, 8.0 * sag + 5.0 * twitch, skirt=14.0 * sag, rear=10.0 * sag)
    turn(pose, "pygidium", rot=(14.0 * sag, 0, 0))
    return pose


def _roll(t: float, points: list[tuple[float, float]]) -> float:
    return keys(t, points)


FLIP_ROLL = [(0.0, 0.0), (0.16, 14.0), (0.3, -8.0), (0.7, 180.0), (1.0, 180.0)]


def flip(t: float) -> Pose:
    """1.6 秒一次性：被掀得先晃一下，四脚朝天翻过去（腾空一小跳），落下后腿还在空中蹬。"""
    pose = Pose()
    roll = _roll(t, FLIP_ROLL)
    hop = math.sin(math.pi * min(1.0, max(0.0, (t - 0.28) / 0.45))) * 6.0
    turn(pose, "root", pos=(0, hop, 0), rot=(0, 0, roll))
    settle = keys(t, [(0.0, 0), (0.7, 0), (0.9, 1), (1.0, 1)])
    kick = keys(t, [(0.0, 0), (0.28, 1), (0.7, 1), (0.9, 0.6), (1.0, 0.6)])          # 翻的过程中腿乱张
    all_legs(pose, lambda name, side, i: (14.0 * wave(t, 3, i * 0.2) * kick, 22.0 * kick, 12.0 * kick))
    mandibles(pose, 20.0 * kick, tip=-6.0 * kick * wave(t, 3))
    elytra(pose, 10.0 * kick - 4.0 * settle, skirt=8.0 * kick)                      # 翻身时甲片被掀开一线
    antennae(pose, t, 14.0, speed=3.0)
    return pose


def flail(t: float) -> Pose:
    """1.2 秒循环：背朝地的挣扎（肚皮朝上）：六足对角乱蹬，钳颚开合，身子左右摇。"""
    pose = Pose()
    turn(pose, "root", rot=(0, 0, 180.0 + 5.0 * wave(t, 1)), pos=(0, 0, 0))

    def kick(name: str, side_name: str, i: int) -> tuple[float, float, float]:
        phase = 0.0 if (name, side_name) in TRIPOD_A else 0.5
        u = t + phase
        return 24.0 * math.sin(math.tau * u), 16.0 + 14.0 * wave(t, 1, phase + 0.25), 10.0 + 12.0 * max(0.0, wave(t, 1, phase))

    all_legs(pose, kick)
    for i, seg in enumerate(SEGS):
        turn(pose, seg, rot=(0, 2.5 * wave(t, 1, 0.1 * i), 1.5 * wave(t, 2, 0.1 * i)))
    mandibles(pose, 16.0 + 14.0 * wave(t, 2), tip=-6.0 * wave(t, 2))
    elytra(pose, 6.0 + 3.0 * wave(t, 2, 0.25), skirt=5.0)
    antennae(pose, t, 14.0, speed=3.0)
    turn(pose, "pygidium", rot=(8.0 * wave(t, 2, 0.4), 6.0 * wave(t, 1), 0))
    return pose


def righting(t: float) -> Pose:
    """1.4 秒一次性：先用腿撑住地面蓄力、背甲一侧顶起，再翻回正面落稳。"""
    pose = Pose()
    roll = keys(t, [(0.0, 180.0), (0.25, 192.0), (0.6, 150.0), (0.8, 10.0), (0.92, -4.0), (1.0, 0.0)])
    hop = math.sin(math.pi * min(1.0, max(0.0, (t - 0.5) / 0.4))) * 4.0
    turn(pose, "root", pos=(0, hop, 0), rot=(0, 0, roll))
    effort = keys(t, [(0.0, 0), (0.25, 1), (0.6, 1), (0.9, 0)])
    land = keys(t, [(0.0, 0), (0.85, 0), (0.92, 1), (1.0, 0)])
    all_legs(pose, lambda name, side, i: (8.0 * wave(t, 2, i * 0.15) * effort, 30.0 * effort, 14.0 * effort))
    mandibles(pose, 14.0 * effort)
    elytra(pose, 8.0 * effort + 6.0 * land, skirt=6.0 * effort)
    turn(pose, "seg_2", rot=(0, 0, 0))
    return pose


def shell_open(t: float) -> Pose:
    """2.4 秒一次性：背甲开合——侧裙先松，鞘翅向两侧掀开露出里面的体壁，停一拍，再依次合拢。"""
    pose = Pose()
    skirt = keys(t, [(0.0, 0), (0.18, 1), (0.7, 1), (0.9, 0), (1.0, 0)])
    wing = keys(t, [(0.0, 0), (0.3, 0), (0.55, 1), (0.72, 1), (0.95, 0), (1.0, 0)])
    rear = keys(t, [(0.0, 0), (0.4, 0), (0.62, 1), (0.78, 1), (0.97, 0), (1.0, 0)])
    elytra(pose, 52.0 * wing, skirt=24.0 * skirt, rear=36.0 * rear, pronotum=14.0 * wing)
    turn(pose, "root", pos=(0, -0.2 * wing, 0))
    turn(pose, "head", rot=(-4.0 * wing, 0, 0))
    antennae(pose, t, 6.0, speed=2.0)
    mandibles(pose, 6.0 + 4.0 * wing)
    turn(pose, "pygidium", rot=(-8.0 * rear, 0, 0))
    return pose


CLIPS: dict[str, tuple[float, bool, Sampler]] = {
    "idle": (3.0, True, idle),
    "walk": (1.6, True, walk),
    "attack": (1.0, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.0, False, death),
    "flip": (1.6, False, flip),
    "flail": (1.2, True, flail),
    "righting": (1.4, False, righting),
    "shell_open": (2.4, False, shell_open),
}


# ---------------------------------------------------------------- 渲染与导出
def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    """整体抬到地面以上：任何一帧最低点低于 0 就把 root 抬起来（翻身后肚皮朝上也靠它落到壳上）。"""

    def wrapped(t: float) -> Pose:
        pose = sampler(t)
        low = rig.lowest(pose)
        if low < 0:
            turn(pose, "root", pos=(0, -low, 0))
        return pose

    return wrapped


def render_gif(name: str, rig: PoseRig, length: float, loop: bool, sampler: Sampler, size: int = 440) -> None:
    count = max(2, round(length * GIF_FPS))
    schedule = gif_schedule(length, count, round(1000 / GIF_FPS), loop, 500)
    poses = [sampler(min(1.0, t / length)) for t, _ in schedule]
    view = framing.view_by_name("-z", "3/4")
    camera = focus(rig, poses, [view])
    frames = []
    for pose in poses:
        image = render(TEX_MODEL, yaw=view.yaw, pitch=view.pitch, size=size, bg=GIF_BACKGROUND,
                       focus=camera, xform=rig.element_xform(pose), shading="mc")[0]
        frames.append(image.convert("RGB").convert("P", palette=Image.ADAPTIVE, colors=96))
    GIF_DIR.mkdir(parents=True, exist_ok=True)
    durations = [duration for _, duration in schedule]
    out = GIF_DIR / f"{name}.gif"
    frames[0].save(out, save_all=True, append_images=frames[1:], duration=durations, loop=0, disposal=2, optimize=True)
    print(f"  {name}: {len(frames)} 帧 / 一轮 {sum(durations)}ms → {out}")


def main() -> None:
    rig = PoseRig(TEX_MODEL)
    clips = {name: (length, loop, grounded(rig, sampler)) for name, (length, loop, sampler) in CLIPS.items()}
    for name, (length, _, sampler) in clips.items():
        lows = [rig.lowest(sampler(i / 12)) for i in range(13)]
        print(f"  {name}: 钳制后最低点 {min(lows):+.2f}px")

    animations = []
    for name, (length, loop, sampler) in clips.items():
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": name, "length": length, "loop": loop, "tracks": tracks})
    write_animated_bbmodel(rig, animations, ANIMATED_MODEL, MODEL_NAME)
    STAGING.mkdir(exist_ok=True)
    subprocess.run(["node", str(CODEC), str(ANIMATED_MODEL), str(STAGING), MODEL_NAME], check=True)
    print(f"✓ {ANIMATED_MODEL.name}，GeckoLib 导出在 {STAGING}")

    for name, (length, loop, sampler) in clips.items():
        render_gif(name, rig, length, loop, sampler)


if __name__ == "__main__":
    main()
