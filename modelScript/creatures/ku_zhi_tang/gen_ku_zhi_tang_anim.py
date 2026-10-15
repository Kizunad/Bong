#!/usr/bin/env python3
"""枯枝螳（ku_zhi_tang）GeckoLib 动画：idle / walk / attack / strike / hurt / death 六段，关键帧从零写。

只落资产、不接线：产物写到暂存目录（modelScript/out/ku_zhi_tang/），不装进 client 资源树。
流程：贴图版 KuZhiTang_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/ku_zhi_tang/。

动作设计（伏击型枯枝螳螂）：
  idle    拟态成枯枝：几乎不动，只像一根枝条被风吹得慢慢摇，触角轻颤，腹尖偶尔一点，镰刀在胸前纹丝不动。
  walk    三角步态：中足右 + 后足左 / 中足左 + 后足右 两组交替，足尖抬起再落下；身子一顿一顿地前后摇，
          头随步点点；镰刀收在胸前不晃，触角探路。
  attack  双镰交替下劈：先后仰蓄力、颚张开，右镰自上而下劈出，收回，左镰再劈一次，身子随劈前探。
  strike  特色闪击：先缓缓僵成一根枯枝（镰刀折紧贴胸、触角贴后、头微垂，只剩风中的微摆）→ 极短的一颤蓄势 →
          两把镰刀瞬间弹出、前胸整个前冲、颚张开 → 停住 → 慢慢收回拟态。
  hurt    被打：整只向后一仰，镰刀甩开，翅鞘猛地张开，触角乱抽，足一阵碎步，抖两下回稳。
  death   侧翻：足先软下去，镰刀垂落，整只向一侧翻倒，六足缩起卷向胸腹，触角垂下，翅鞘半张，之后不动。

坐标与符号（面朝 -Z，+X 是右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 竖着往上长的部件（前胸 / 触角）：rx > 0 = 顶端向后倒，rx < 0 = 顶端向前倾；
  - 朝前（-z）伸的部件（头的脸）：rx > 0 = 抬头；下垂的部件（镰刀胫节 / 腹）：rx > 0 = 末端往前（腹是往下卷）；
  - 向外伸的足（右侧 +x）：ry > 0 = 向前摆，rz > 0 = 外端往上抬；左侧由 both() 镜像（y / z 分量取反）；
  - position：-Z = 前，+Y = 上，单位 px。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/ku_zhi_tang/gen_ku_zhi_tang_anim.py
    ... gen_ku_zhi_tang_anim.py --no-gif | --clip strike
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import subprocess
import sys
from base64 import b64decode
from pathlib import Path

from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, write_animated_bbmodel

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
NAME = "ku_zhi_tang"
MODEL = CREATURE_DIR / "KuZhiTang_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim") / NAME

GROUND_TOLERANCE = 0.8   # 落地后最低点入地不得超过它（px）
SAMPLES = 36             # 每段采样数，build_tracks 再裁掉共线帧
TAU = math.tau
SNAP_BOTH = {"death"}    # 这些段两个方向都贴地：翻倒后整只落到地面上，而不是悬在半空

NECK = ("prothorax_1", "prothorax_2", "prothorax_3")
ABDOMEN = ("abd_0", "abd_1", "abd_2", "abd_3", "abd_4")
LEG_GROUPS = (("midleg", "hindleg"),)


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t01 上走 cycles 个整周期的正弦；循环动画首尾自然相接。"""
    return math.sin(TAU * (cycles * t + phase))


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep：points 为 (t01, 值)，一次性动作用它排节奏。"""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


class Poser:
    """往 Pose 里累加骨骼变换；骨名拼错直接报错。"""

    def __init__(self, rig: PoseRig):
        self.rig = rig

    def turn(self, pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0), scale=None) -> None:
        if bone not in self.rig.bones:
            raise KeyError(f"{NAME}: 没有骨 {bone}")
        channel = pose[bone]
        channel.rot = [a + b for a, b in zip(channel.rot, rot)]
        channel.pos = [a + b for a, b in zip(channel.pos, pos)]
        if scale is not None:
            channel.scale = [float(v) for v in scale]

    def both(self, pose: Pose, stem: str, rot_r=(0.0, 0.0, 0.0), rot_l=None) -> None:
        """左右成对的骨：rot_r 给右侧；rot_l 缺省按镜像（y / z 分量取反）。"""
        if rot_l is None:
            rot_l = (rot_r[0], -rot_r[1], -rot_r[2])
        self.turn(pose, f"{stem}_r", rot=rot_r)
        self.turn(pose, f"{stem}_l", rot=rot_l)


# ---------------------------------------------------------------- 共用动作件
def sickle(poser: Poser, pose: Pose, side: str, femur: float, tibia: float, claw: float = 0.0, spread: float = 0.0) -> None:
    """一侧镰刀：femur = 股节转角（< 0 向前下方伸，> 0 抬起收回）；tibia = 胫节展开（> 0 向前展开，< 0 折得更紧）；
    claw = 弯爪；spread = 整条镰臂向外张（度）。"""
    sign = 1.0 if side == "r" else -1.0
    poser.turn(pose, f"foreleg_coxa_{side}", rot=(0, 0, sign * -0.4 * spread))
    poser.turn(pose, f"foreleg_femur_{side}", rot=(femur, 0, sign * -spread))
    poser.turn(pose, f"foreleg_tibia_{side}", rot=(tibia, 0, 0))
    poser.turn(pose, f"foreleg_claw_{side}", rot=(claw, 0, 0))


def walker(poser: Poser, pose: Pose, name: str, side: str, swing: float, lift: float, droop: float = 0.0) -> None:
    """一条步足：swing = 股节向前摆（度，+ = 向前）；lift = 胫节外端上抬；droop = 股节下垂。
    右侧 ry > 0 向前、rz > 0 上抬；左侧骨系镜像，ry / rz 取反才是同一方向的动作。"""
    sign = 1.0 if side == "r" else -1.0
    poser.turn(pose, f"{name}_femur_{side}", rot=(0, sign * swing, -sign * droop))
    poser.turn(pose, f"{name}_tibia_{side}", rot=(0, 0, sign * lift))
    poser.turn(pose, f"{name}_foot_{side}", rot=(0, 0, sign * 0.5 * lift))


def walkers(poser: Poser, pose: Pose, swing: float, lift: float, droop: float = 0.0) -> None:
    """中后足四条同做一个动作（拟态 / 闪击 / 受击 / 死亡时用）。"""
    for name in ("midleg", "hindleg"):
        for side in ("r", "l"):
            walker(poser, pose, name, side, swing, lift, droop)


def neck_lean(poser: Poser, pose: Pose, lean: float) -> None:
    """前胸三段整体前倾（lean > 0 = 向前），按 4 : 3 : 3 分到三段。"""
    for bone, share in zip(NECK, (0.4, 0.3, 0.3)):
        poser.turn(pose, bone, rot=(-lean * share, 0, 0))


def antennae(poser: Poser, pose: Pose, t: float, cycles: float, amp: float, back: float = 0.0, phase: float = 0.0) -> None:
    """触角轻颤；back > 0 = 整根向后贴（拟态时贴着头）。"""
    for sx, side in ((1, "r"), (-1, "l")):
        poser.turn(pose, f"antenna1_{side}", rot=(back + amp * 0.6 * wave(t, cycles, phase + 0.1 * sx), 0, sx * amp * 0.6 * wave(t, cycles, phase + 0.3)))
        poser.turn(pose, f"antenna2_{side}", rot=(back * 0.5 + amp * wave(t, cycles, phase + 0.25 + 0.15 * sx), 0, sx * amp * wave(t, cycles, phase + 0.5)))


def jaws(poser: Poser, pose: Pose, open_deg: float) -> None:
    poser.both(pose, "jaw", rot_r=(0, 0, open_deg))


def abdomen_curl(poser: Poser, pose: Pose, curl: float, sway: float = 0.0) -> None:
    """腹部五节：curl > 0 = 末端往下卷，< 0 = 向上翘；sway = 左右摆。"""
    for n, bone in enumerate(ABDOMEN):
        poser.turn(pose, bone, rot=(curl * (0.6 + 0.12 * n), sway * (0.5 + 0.2 * n), 0))


def wing_flare(poser: Poser, pose: Pose, deg: float) -> None:
    poser.both(pose, "wing", rot_r=(0, 0, deg))


def cerci(poser: Poser, pose: Pose, deg: float) -> None:
    poser.both(pose, "cercus", rot_r=(0, deg, 0))


# ---------------------------------------------------------------- 动作
def ku_zhi_tang_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 一根被风吹的枯枝：整身慢摇（0.33 Hz），触角轻颤，腹尖偶尔一点，镰刀不动
        pose = Pose()
        sway = wave(t, 1, 0.0)
        poser.turn(pose, "root", rot=(0, 0.6 * wave(t, 1, 0.3), 1.0 * sway))
        poser.turn(pose, "metathorax", rot=(0.4 * wave(t, 1, 0.2), 0, 0.5 * wave(t, 1, 0.1)))
        neck_lean(poser, pose, 1.2 * wave(t, 1, 0.45))
        poser.turn(pose, "head", rot=(1.2 * wave(t, 1, 0.15), 3.0 * wave(t, 1, 0.6), 1.5 * wave(t, 1, 0.1)))
        jaws(poser, pose, 1.0 + 1.0 * max(0.0, wave(t, 2, 0.25)))
        antennae(poser, pose, t, 3, 4.0)
        abdomen_curl(poser, pose, 0.8 * wave(t, 1, 0.55), sway=1.0 * wave(t, 1, 0.25))
        cerci(poser, pose, 2.0 * wave(t, 3, 0.1))
        wing_flare(poser, pose, 0.6 * wave(t, 1, 0.35))
        sickle(poser, pose, "r", 0.5 * wave(t, 1, 0.2), 0.0)
        sickle(poser, pose, "l", 0.5 * wave(t, 1, 0.45), 0.0)
        return pose

    def walk(t: float) -> Pose:
        # 三角步态：A 组 = 中右 + 后左，B 组 = 中左 + 后右；一周期 1.4s，每组各抬一次足
        pose = Pose()
        a = wave(t, 1, 0.0)
        b = wave(t, 1, 0.5)
        step_a = max(0.0, wave(t, 1, 0.25))
        step_b = max(0.0, wave(t, 1, 0.75))
        rock = wave(t, 2, 0.0)
        poser.turn(pose, "root", pos=(0, -0.3 * (0.5 - 0.5 * rock), 0))
        poser.turn(pose, "metathorax", rot=(1.5 * rock, 2.0 * a, 1.0 * b))
        neck_lean(poser, pose, 2.0 + 1.5 * rock)
        poser.turn(pose, "head", rot=(-1.5 * rock, 3.0 * a, 0))
        antennae(poser, pose, t, 2, 7.0, phase=0.1)
        abdomen_curl(poser, pose, 1.2 * rock, sway=2.0 * a)
        cerci(poser, pose, 3.0 * wave(t, 2, 0.2))
        # 步足：摆幅 ±11°，抬足在向前摆的半周期；A 组走 a 相位，B 组走 b 相位
        walker(poser, pose, "midleg", "r", 16 * a, 32 * step_a)
        walker(poser, pose, "hindleg", "l", 13 * a, 32 * step_a)
        walker(poser, pose, "midleg", "l", 16 * b, 32 * step_b)
        walker(poser, pose, "hindleg", "r", 13 * b, 32 * step_b)
        for side in ("r", "l"):          # 镰刀收在胸前，随身子微微颠
            sickle(poser, pose, side, 10 + 1.0 * rock, -10)
        return pose

    def attack(t: float) -> Pose:
        # 蓄力（0~0.25）：身子后仰、双镰抬起、颚张开；右镰劈（0.25~0.45）→ 收（0.45~0.58）；
        # 左镰劈（0.55~0.75）→ 收（0.75~1）
        pose = Pose()
        wind = keys(t, [(0, 0), (0.22, 1), (0.5, 0.7), (0.56, 1), (0.78, 0), (1, 0)])
        cut_r = keys(t, [(0, 0), (0.22, 0), (0.32, 1), (0.42, 0.8), (0.56, 0), (1, 0)])
        cut_l = keys(t, [(0, 0), (0.54, 0), (0.64, 1), (0.74, 0.8), (0.9, 0), (1, 0)])
        lunge = max(cut_r, cut_l)
        poser.turn(pose, "metathorax", rot=(-4 * wind + 5 * lunge, 0, 0))
        neck_lean(poser, pose, -6 * wind + 14 * lunge)
        poser.turn(pose, "head", rot=(4 * wind - 8 * lunge, 4 * (cut_l - cut_r), 0))
        jaws(poser, pose, 22 * wind)
        antennae(poser, pose, t, 4, 5.0, back=-8 * wind + 10 * lunge)
        abdomen_curl(poser, pose, -6 * wind + 4 * lunge)
        wing_flare(poser, pose, 5 * wind)
        sickle(poser, pose, "r", 24 * wind * (1 - cut_r) - 30 * cut_r, -18 * wind * (1 - cut_r) + 112 * cut_r, 14 * cut_r, spread=8 * wind)
        sickle(poser, pose, "l", 24 * wind * (1 - cut_l) - 30 * cut_l, -18 * wind * (1 - cut_l) + 112 * cut_l, 14 * cut_l, spread=8 * wind)
        return pose

    def strike(t: float) -> Pose:
        # 0~0.2 缓缓僵成枯枝；到 0.46 只剩风中微摆；0.46~0.52 一颤蓄势；0.52~0.575 镰刀弹出；
        # 0.575~0.72 停住；0.72~1 慢慢收回拟态
        pose = Pose()
        fold = keys(t, [(0, 0), (0.2, 1), (1, 1)])
        quiver = keys(t, [(0, 0), (0.46, 0), (0.49, 1), (0.52, 0.3), (0.575, 0), (1, 0)])
        snap = keys(t, [(0, 0), (0.52, 0), (0.575, 1), (0.72, 1), (0.95, 0), (1, 0)])
        calm = fold * (1 - snap)
        poser.turn(pose, "root", rot=(0, 0, 0.5 * wave(t, 2, 0.3) * calm))
        poser.turn(pose, "metathorax", rot=(2.5 * fold - 3 * quiver + 4 * snap, 0, 0))
        neck_lean(poser, pose, -3 * fold - 4 * quiver + 20 * snap)
        poser.turn(pose, "head", rot=(-7 * fold * (1 - snap) + 4 * quiver - 8 * snap, 0, 0.8 * wave(t, 2, 0.1) * calm))
        jaws(poser, pose, -1.0 * fold * (1 - snap) + 30 * snap)
        antennae(poser, pose, t, 2, 1.5 * (1 - fold) + 0.8 * fold, back=14 * fold * (1 - snap) - 12 * snap)
        abdomen_curl(poser, pose, 2.0 * fold - 8 * snap - 4 * quiver)
        cerci(poser, pose, 4 * snap)
        wing_flare(poser, pose, 0.5 * fold * wave(t, 2, 0.1) + 6 * snap)
        femur = 11 * fold * (1 - snap) + 22 * quiver - 28 * snap
        tibia = -12 * fold * (1 - snap) - 14 * quiver + 125 * snap
        for side in ("r", "l"):
            sickle(poser, pose, side, femur, tibia, claw=-6 * fold * (1 - snap) + 16 * snap, spread=3 * fold * (1 - snap) + 5 * snap)
        walkers(poser, pose, -2 * snap, 0.0)
        return pose

    def hurt(t: float) -> Pose:
        # 被打：向后一仰（0~0.15），镰刀甩开、翅鞘张开、触角乱抽，晃两下回稳
        pose = Pose()
        hit = keys(t, [(0, 0), (0.12, 1), (0.3, 0.5), (0.5, 0.2), (1, 0)])
        shake = hit * wave(t, 3, 0.0)
        poser.turn(pose, "root", rot=(-4 * hit, 3 * shake, 2 * shake), pos=(0, 0, 1.6 * hit))
        poser.turn(pose, "metathorax", rot=(-6 * hit, 0, 3 * shake))
        neck_lean(poser, pose, -16 * hit)
        poser.turn(pose, "head", rot=(8 * hit, 8 * shake, 5 * shake))
        jaws(poser, pose, 26 * hit)
        antennae(poser, pose, t, 6, 16 * hit, back=-6 * hit)
        abdomen_curl(poser, pose, -10 * hit, sway=8 * shake)
        cerci(poser, pose, 12 * hit)
        wing_flare(poser, pose, 26 * hit)
        for side in ("r", "l"):
            sickle(poser, pose, side, 30 * hit, -24 * hit, claw=-10 * hit, spread=16 * hit)
        for name, sw in (("midleg", 6), ("hindleg", -6)):
            for side in ("r", "l"):
                walker(poser, pose, name, side, sw * shake, 8 * hit * max(0.0, wave(t, 4, 0.1)))
        return pose

    def death(t: float) -> Pose:
        # 足软（0~0.3）→ 镰刀垂落 → 侧翻（0.25~0.7）→ 六足缩起卷向胸腹、触角垂下、翅鞘半张，之后不动
        pose = Pose()
        stagger = math.sin(math.pi * min(1.0, t / 0.22)) if t < 0.22 else 0.0
        sag = keys(t, [(0, 0), (0.12, 0), (0.45, 1), (1, 1)])
        fall = keys(t, [(0, 0), (0.22, 0), (0.62, 1), (1, 1)])
        curl = keys(t, [(0, 0), (0.45, 0), (0.95, 1), (1, 1)])
        poser.turn(pose, "root", rot=(0, 4 * stagger, 76 * fall + 3 * stagger))
        poser.turn(pose, "metathorax", rot=(6 * sag, 0, 0))
        neck_lean(poser, pose, 10 * sag)
        poser.turn(pose, "head", rot=(-14 * sag, 10 * fall, 12 * fall))
        jaws(poser, pose, 16 * keys(t, [(0, 0), (0.4, 1), (1, 1)]))
        antennae(poser, pose, t, 0, 0.0, back=-14 * sag)
        abdomen_curl(poser, pose, 18 * sag, sway=6 * fall)
        cerci(poser, pose, 10 * sag)
        wing_flare(poser, pose, 22 * fall)
        for side in ("r", "l"):
            sickle(poser, pose, side, 28 * sag, -20 * sag + 40 * curl, claw=-20 * curl, spread=12 * sag + 10 * curl)
        walkers(poser, pose, -6 * sag, 38 * sag + 40 * curl, droop=18 * sag + 6 * curl)
        for name in ("midleg", "hindleg"):
            poser.both(pose, f"{name}_foot", rot_r=(0, 0, 30 * curl))
        return pose

    return {
        "idle": (3.0, True, idle), "walk": (1.4, True, walk), "attack": (1.2, False, attack),
        "strike": (2.8, False, strike), "hurt": (0.7, False, hurt), "death": (2.6, False, death),
    }


# ---------------------------------------------------------------- 落地约束 / 自检
def grounded(rig: PoseRig, poser: Poser, sampler, both: bool):
    """整只最低点入地的话把 root 抬到刚好贴地；both=True（死亡翻倒）时反方向也拉回来，倒下后整只落在地面上。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        lowest = rig.lowest(pose)
        if lowest < 0.0 or both:
            poser.turn(pose, "root", pos=(0.0, -lowest, 0.0))
        return pose

    return sample


def check_ground(rig: PoseRig, sampler) -> float:
    lowest = min(rig.lowest(sampler(i / 12)) for i in range(13))
    if lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{NAME}: 最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    return lowest


def loop_gap(sampler) -> tuple[float, float]:
    """循环段首尾姿态的最大差（旋转度数、位移 px；缩放差 0.01 折成 0.1）。"""
    a, b = sampler(0.0), sampler(1.0)
    rot = pos = 0.0
    for bone in set(a) | set(b):
        rot = max(rot, max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)))
        pos = max(pos, max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)))
        pos = max(pos, 10 * max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)))
    return rot, pos


# ---------------------------------------------------------------- 导出
def export() -> Path:
    rig = PoseRig(MODEL)
    poser = Poser(rig)
    author = ku_zhi_tang_clips(poser)

    animations = []
    for clip, (length, loop, raw) in author.items():
        sampler = grounded(rig, poser, raw, clip in SNAP_BOTH)
        line = f"  {clip}: 最低点 {check_ground(rig, sampler):+.2f}px"
        if loop:
            rot, pos = loop_gap(sampler)
            if rot > 0.5 or pos > 0.1:
                raise ValueError(f"{NAME}.{clip}: 循环首尾不同值（旋转差 {rot:.2f}° 位移 / 缩放差 {pos:.2f}）")
            line += f"，首尾差 {rot:.2f}° / {pos:.2f}"
        print(line)
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": clip, "length": length, "loop": loop, "tracks": tracks})

    STAGING.mkdir(parents=True, exist_ok=True)
    animated = STAGING / f"{NAME}_animated.bbmodel"
    write_animated_bbmodel(rig, animations, animated, NAME)
    subprocess.run(["node", str(CODEC), str(animated), str(STAGING), NAME], check=True)

    # creature_anim_frames.py 按 <assets>/{geo,animations,textures/entity/fauna} 取文件，把暂存目录摆成这个形状
    for sub, name in (("geo", f"{NAME}.geo.json"), ("animations", f"{NAME}.animation.json")):
        (STAGING / sub).mkdir(exist_ok=True)
        shutil.copyfile(STAGING / name, STAGING / sub / name)
    texture = json.loads(MODEL.read_text(encoding="utf-8"))["textures"][0]["source"].split(",", 1)[1]
    (STAGING / "textures" / "entity" / "fauna").mkdir(parents=True, exist_ok=True)
    (STAGING / "textures" / "entity" / "fauna" / f"{NAME}.png").write_bytes(b64decode(texture))

    produced = json.loads((STAGING / f"{NAME}.animation.json").read_text(encoding="utf-8"))["animations"]
    expected = {f"animation.bong.{NAME}.{a['name']}" for a in animations}
    if set(produced) != expected:
        raise ValueError(f"导出的动画集 {sorted(produced)} 与作者稿 {sorted(expected)} 不一致")
    print(f"{NAME}: " + ", ".join(f"{a['name']} {a['length']}s{' loop' if a['loop'] else ''}" for a in animations)
          + f" → {STAGING}")
    return STAGING


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-gif", action="store_true")
    parser.add_argument("--clip", action="append", help="只重渲这几段 GIF（短名）")
    args = parser.parse_args()
    staging = export()
    if args.no_gif:
        return
    GIF_DIR.mkdir(parents=True, exist_ok=True)
    subprocess.run([sys.executable, str(REPO / "modelScript" / "tools" / "creature_anim_frames.py"),
                    "--name", NAME, "--assets", str(staging), "--gif", "--size", "440", "--out", str(GIF_DIR),
                    *[a for c in (args.clip or []) for a in ("--clip", c)]], check=True)


if __name__ == "__main__":
    main()
