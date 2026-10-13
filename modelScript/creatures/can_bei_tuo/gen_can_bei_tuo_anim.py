#!/usr/bin/env python3
"""残背驮兽 GeckoLib 动画：idle / walk / attack / hurt / death 五段，关键帧程序化采样。

只落资产，不接线、不装进 client：
  - CanBeiTuo_tex_animated.bbmodel   带 animations 的贴图版（Blockbench 可直接打开）
  - out/CanBeiTuo.animation.json     经 creature_codec 导出的 GeckoLib 动画（暂存目录，不进资源树）
  - model-review/anim/can_bei_tuo/*.gif  原速预览

坐标约定：本模型建模稿就面朝 -Z（游戏约定），动画直接按 bbmodel 空间写，不做 fauna_v2 那种共轭。
右手系：骨上 rotation.x 为正 = 绕 +X 转 = 朝 -Z 的前端抬起；负 = 低头。
大腿 / 肩的枢轴在上端，rotation.x 为正 = 脚尖向前（-Z）摆；膝 / 飞节 rotation.x 为负 = 小腿向后折。

先跑 gen_can_bei_tuo_tex.py 出贴图版，再跑本文件：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/can_bei_tuo/gen_can_bei_tuo_anim.py
"""

from __future__ import annotations

import json
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

TEX_MODEL = CREATURE_DIR / "CanBeiTuo_tex.bbmodel"
ANIMATED_MODEL = CREATURE_DIR / "CanBeiTuo_tex_animated.bbmodel"
STAGING = CREATURE_DIR / "out"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
GIF_DIR = Path(__file__).resolve().parents[2] / "out" / "review" / "anim" / "can_bei_tuo"
MODEL_NAME = "can_bei_tuo"  # creature_codec 只收小写蛇形名

SAMPLES = 24
GIF_FPS = 20
GIF_BACKGROUND = (22, 23, 26)

Sampler = Callable[[float], Pose]

LEGS = {  # 腿名 → (大腿, 膝 / 飞节, 小腿, 脚, 相位组)；FL+BR 一组，FR+BL 一组，错半周期
    "fl": ("leg_fl_thigh", "leg_fl_knee", "leg_fl_shank", "foot_fl", 0.0),
    "br": ("leg_br_thigh", "leg_br_hock", "leg_br_shank", "foot_br", 0.0),
    "fr": ("leg_fr_thigh", "leg_fr_knee", "leg_fr_shank", "foot_fr", 0.5),
    "bl": ("leg_bl_thigh", "leg_bl_hock", "leg_bl_shank", "foot_bl", 0.5),
}
MOSS_BONES = ("moss_hump_crest", "moss_flank_fl", "moss_flank_fr", "moss_spine_mid",
              "moss_flank_bl", "moss_flank_br", "moss_rump_crest")


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


def sway_moss(pose: Pose, t: float, amount: float, lag: float = 0.0) -> None:
    """藓块各自摆：相位错开，像挂在背上的软物被带着晃。"""
    for i, bone in enumerate(MOSS_BONES):
        phase = lag + i * 0.11
        turn(pose, bone, rot=(amount * wave(t, 1, phase), 0.0, amount * 0.7 * wave(t, 1, phase + 0.25)))


# ---------------------------------------------------------------- 五段动画
def idle(t: float) -> Pose:
    """4 秒循环：缓慢呼吸，头沉沉地点，耳尾偶动，藓块随呼吸微晃。"""
    pose = Pose()
    breath = wave(t, 1)
    turn(pose, "thorax", rot=(0.8 * breath, 0, 0), pos=(0, 0.12 * breath, 0))
    turn(pose, "lumbar", rot=(-0.5 * breath, 0, 0))
    turn(pose, "neck_base", rot=(1.5 * wave(t, 1, 0.1), 0, 0))
    turn(pose, "neck_top", rot=(-2.0 * wave(t, 1, 0.2), 0, 1.2 * wave(t, 2)))
    turn(pose, "head", rot=(-2.5 * wave(t, 1, 0.3), 3.0 * wave(t, 2, 0.1), 0))
    turn(pose, "jaw", rot=(2.0 + 2.0 * wave(t, 2, 0.4), 0, 0))
    turn(pose, "ear_l", rot=(0, 0, 6 * max(0.0, wave(t, 2, 0.25)) ** 2))
    turn(pose, "ear_r", rot=(0, 0, -6 * max(0.0, wave(t, 2, 0.75)) ** 2))
    turn(pose, "tail_1", rot=(0, 5 * wave(t, 1, 0.2), 0))
    turn(pose, "tail_2", rot=(0, 7 * wave(t, 1, 0.3), 0))
    turn(pose, "tail_3", rot=(0, 9 * wave(t, 1, 0.4), 0))
    turn(pose, "tail_moss", rot=(3 * wave(t, 2), 4 * wave(t, 1, 0.5), 0))
    turn(pose, "moss_beard", rot=(3 * wave(t, 1, 0.35), 0, 2 * wave(t, 2)))
    turn(pose, "neck_moss_dewlap", rot=(4 * wave(t, 1, 0.45), 0, 0))
    sway_moss(pose, t, 1.2 + 0.6 * breath)
    return pose


def walk(t: float) -> Pose:
    """2 秒一步周期：沉重缓慢。对角步态，每步重心下沉，头随步点，藓块比身体晚一拍摇。"""
    pose = Pose()
    for name, (thigh, joint, shank, foot, phase) in LEGS.items():
        u = t + phase
        front = name[0] == "f"
        swing = 20 * math.sin(math.tau * u)                       # 大腿前后扫
        lift = max(0.0, math.cos(math.tau * u))                   # 摆动相抬腿
        turn(pose, thigh, rot=(swing if front else 0.8 * swing, 0, 0))
        turn(pose, joint, rot=(-32 * lift ** 1.5, 0, 0))
        turn(pose, shank, rot=(8 * lift ** 1.5 if front else 14 * lift, 0, 0))
        turn(pose, foot, rot=(-10 * lift + 6 * math.sin(math.tau * u), 0, 0))
    step = abs(wave(t, 1))                                         # 一个周期两次落脚
    turn(pose, "hips", pos=(0, -0.55 * (1 - abs(wave(t, 2))), 0), rot=(0, 2.0 * wave(t, 1), 1.2 * wave(t, 2, 0.25)))
    turn(pose, "lumbar", rot=(0, -2.5 * wave(t, 1), -1.0 * wave(t, 2, 0.25)))
    turn(pose, "thorax", rot=(0.8 * wave(t, 2, 0.1), 2.0 * wave(t, 1), 1.4 * wave(t, 2, 0.25)))
    turn(pose, "neck_base", rot=(-2.0 * wave(t, 2, 0.15), 0, 0))
    turn(pose, "neck_top", rot=(-3.0 * wave(t, 2, 0.2), 1.5 * wave(t, 1), 0))
    turn(pose, "head", rot=(-3.5 * wave(t, 2, 0.3) - 2.0, 2.5 * wave(t, 1, 0.1), 0))
    turn(pose, "jaw", rot=(2.0 + 1.5 * step, 0, 0))
    turn(pose, "ear_l", rot=(0, 0, 5 * wave(t, 2, 0.3)))
    turn(pose, "ear_r", rot=(0, 0, -5 * wave(t, 2, 0.3)))
    turn(pose, "tail_1", rot=(0, 8 * wave(t, 1, 0.1), 0))
    turn(pose, "tail_2", rot=(0, 11 * wave(t, 1, 0.2), 0))
    turn(pose, "tail_3", rot=(0, 14 * wave(t, 1, 0.3), 0))
    turn(pose, "tail_moss", rot=(4 * wave(t, 2, 0.2), 6 * wave(t, 1, 0.4), 0))
    turn(pose, "moss_beard", rot=(5 * wave(t, 2, 0.4), 0, 3 * wave(t, 1, 0.2)))
    turn(pose, "neck_moss_dewlap", rot=(7 * wave(t, 2, 0.45), 0, 0))
    sway_moss(pose, t, 4.5, lag=0.22)                              # 比躯干晚约 0.2 周期
    return pose


def attack(t: float) -> Pose:
    """1.2 秒一次性：后坐蓄势 → 低头角撞（身体前冲、前腿蹬）→ 顶住停一拍 → 回位。"""
    pose = Pose()
    rear = keys(t, [(0.0, 0), (0.28, 1), (0.40, 1), (0.52, 0)])             # 蓄势：重心压到后腿、头抬起
    ram = keys(t, [(0.0, 0), (0.40, 0), (0.52, 1), (0.70, 1), (1.0, 0)])    # 撞击与顶住
    turn(pose, "hips", pos=(0, -0.8 * rear, 1.2 * rear - 2.4 * ram), rot=(-3 * rear, 0, 0))
    turn(pose, "lumbar", rot=(-3 * rear + 2 * ram, 0, 0))
    turn(pose, "thorax", rot=(6 * rear - 6 * ram, 0, 0))
    turn(pose, "neck_base", rot=(8 * rear - 10 * ram, 0, 0))
    turn(pose, "neck_top", rot=(6 * rear - 8 * ram, 0, 0))
    turn(pose, "head", rot=(8 * rear - 14 * ram, 0, 0))              # 抬头 → 低头角尖向前
    turn(pose, "jaw", rot=(-4 * rear + 6 * ram, 0, 0))
    turn(pose, "ear_l", rot=(0, 0, -14 * ram))
    turn(pose, "ear_r", rot=(0, 0, 14 * ram))
    for name, (thigh, joint, shank, foot, _) in LEGS.items():
        if name[0] == "f":
            turn(pose, thigh, rot=(-10 * rear + 26 * ram, 0, 0))
            turn(pose, joint, rot=(-10 * ram + 6 * rear, 0, 0))
            turn(pose, foot, rot=(-8 * ram, 0, 0))
        else:
            turn(pose, thigh, rot=(8 * rear - 20 * ram, 0, 0))       # 后腿蹬直送力
            turn(pose, joint, rot=(-14 * rear + 10 * ram, 0, 0))
            turn(pose, shank, rot=(10 * rear - 8 * ram, 0, 0))
    turn(pose, "tail_1", rot=(-6 * ram, 0, 0))
    turn(pose, "tail_2", rot=(-8 * ram, 0, 0))
    turn(pose, "tail_3", rot=(-10 * ram, 0, 0))
    # 藓块被撞击的惯性甩起，再慢慢落回
    for i, bone in enumerate(MOSS_BONES):
        turn(pose, bone, rot=(-10 * rear + 14 * keys(t, [(0.0, 0), (0.52, 0), (0.62, 1), (1.0, 0)]) + 3 * wave(t, 2, i * 0.13) * ram, 0, 0))
    turn(pose, "moss_beard", rot=(-14 * ram, 0, 0))
    turn(pose, "neck_moss_dewlap", rot=(-10 * ram, 0, 0))
    return pose


def hurt(t: float) -> Pose:
    """0.5 秒一次性：挨一下向后缩，头甩起，藓块抖落，再回位。"""
    pose = Pose()
    k = keys(t, [(0.0, 0), (0.2, 1), (0.5, 0.35), (1.0, 0)])
    turn(pose, "hips", pos=(0, -0.3 * k, 1.4 * k), rot=(2 * k, 0, 0))
    turn(pose, "thorax", rot=(7 * k, 0, 3 * k * wave(t, 3)))
    turn(pose, "neck_base", rot=(10 * k, 0, 0))
    turn(pose, "neck_top", rot=(8 * k, 0, 4 * wave(t, 3) * k))
    turn(pose, "head", rot=(12 * k, 0, 0))
    turn(pose, "jaw", rot=(10 * k, 0, 0))
    turn(pose, "ear_l", rot=(0, 0, 12 * k))
    turn(pose, "ear_r", rot=(0, 0, -12 * k))
    for name, (thigh, joint, shank, foot, _) in LEGS.items():
        turn(pose, thigh, rot=((-6 if name[0] == "f" else 5) * k, 0, 0))
        turn(pose, joint, rot=(-8 * k, 0, 0))
    turn(pose, "tail_1", rot=(6 * k, 0, 0))
    turn(pose, "tail_2", rot=(8 * k, 0, 0))
    for i, bone in enumerate(MOSS_BONES):
        turn(pose, bone, rot=(6 * k * wave(t, 3, i * 0.15), 0, 5 * k * wave(t, 3, i * 0.15 + 0.25)))
    turn(pose, "moss_beard", rot=(10 * k, 0, 0))
    return pose


def death(t: float) -> Pose:
    """2 秒一次性：前膝先软，前身塌下去，后腿跟着瘫，最后侧倒不动，藓块摊开。"""
    pose = Pose()
    front = keys(t, [(0.0, 0), (0.35, 1), (1.0, 1)])
    back = keys(t, [(0.0, 0), (0.25, 0), (0.6, 1), (1.0, 1)])
    roll = keys(t, [(0.0, 0), (0.55, 0), (1.0, 1)])
    for name, (thigh, joint, shank, foot, _) in LEGS.items():
        if name[0] == "f":
            turn(pose, thigh, rot=(22 * front, 0, -6 * front if name[1] == "l" else 6 * front))
            turn(pose, joint, rot=(-62 * front, 0, 0))
            turn(pose, shank, rot=(30 * front, 0, 0))
        else:
            turn(pose, thigh, rot=(-18 * back, 0, -8 * back if name[1] == "l" else 8 * back))
            turn(pose, joint, rot=(-48 * back, 0, 0))
            turn(pose, shank, rot=(36 * back, 0, 0))
    turn(pose, "hips", pos=(0, -4.0 * back, 0), rot=(4 * back, 0, 0))
    turn(pose, "lumbar", rot=(2 * back, 0, 0))
    turn(pose, "thorax", pos=(0, -3.0 * front, 0), rot=(10 * front, 0, 0))
    turn(pose, "neck_base", rot=(-12 * front, 0, 0))
    turn(pose, "neck_top", rot=(-14 * front, 0, 0))
    turn(pose, "head", rot=(-16 * front, 0, 6 * roll))
    turn(pose, "jaw", rot=(14 * roll, 0, 0))
    turn(pose, "ear_l", rot=(0, 0, -26 * front))
    turn(pose, "ear_r", rot=(0, 0, 26 * front))
    turn(pose, "root", rot=(0, 0, 14 * roll))                        # 最后向一侧歪倒
    turn(pose, "tail_1", rot=(-10 * back, 14 * roll, 0))
    turn(pose, "tail_2", rot=(-12 * back, 10 * roll, 0))
    turn(pose, "tail_3", rot=(-14 * back, 8 * roll, 0))
    # 藓块一块块摊开、垂下，各自落点不同
    for i, bone in enumerate(MOSS_BONES):
        side = -1 if i % 2 else 1
        turn(pose, bone, rot=(-4 * front, 0, side * (10 + 3 * i) * roll))
    turn(pose, "moss_beard", rot=(-14 * front, 0, 0))
    turn(pose, "neck_moss_dewlap", rot=(-16 * front, 0, 0))
    return pose


CLIPS: dict[str, tuple[float, bool, Sampler]] = {
    "idle": (4.0, True, idle),
    "walk": (2.0, True, walk),
    "attack": (1.2, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.0, False, death),
}


# ---------------------------------------------------------------- 渲染与导出
def ground_report(rig: PoseRig) -> None:
    for name, (length, _, sampler) in CLIPS.items():
        lows = [rig.lowest(sampler(i / 12)) for i in range(13)]
        print(f"  {name}: 最低点 {min(lows):+.2f}px（静止最低 {rig.lowest(Pose()):+.2f}）")


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


def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    """整体抬到地面以上：任何一帧最低点低于 0 就把 root 抬起来，不让脚 / 头沉进地里。"""

    def wrapped(t: float) -> Pose:
        pose = sampler(t)
        low = rig.lowest(pose)
        if low < 0:
            turn(pose, "root", pos=(0, -low, 0))
        return pose

    return wrapped


def main() -> None:
    rig = PoseRig(TEX_MODEL)
    ground_report(rig)
    clips = {name: (length, loop, grounded(rig, sampler)) for name, (length, loop, sampler) in CLIPS.items()}

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
