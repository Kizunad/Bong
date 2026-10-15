#!/usr/bin/env python3
"""残羽鸮（can_yu_xiao）GeckoLib 动画：idle / walk / fly / attack / hurt / death 六段，关键帧从零写。

只落资产、不接线：产物写到暂存目录（modelScript/out/can_yu_xiao/），不装进 client 资源树。
流程：贴图版 CanYuXiao_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/can_yu_xiao/。

动作设计（羽毛脱落一半的大猫头鹰，残翼飞不稳，夜里悄无声息地扑食）：
  idle    站立呼吸，翅膀半垂；隔一阵把头一圈圈转过去 180° 盯向身后（颈转 100° + 头转 80°），停住不眨，再转回来；
          耳羽与残尾羽轻颤。
  walk    猫头鹰式的一摇一摆：双足交替迈步，身子随步左右倾，头反向补偿保持平稳（鸟头不乱晃），翅膀收着微微颤。
  fly     残翼拍得不稳：右翼拍满、左翼幅度小而慢半拍，机身一边拍一边侧滚和下沉又被拉起，爪子收在腹下，尾羽乱扇。
  attack  俯冲抓击：先压低身子、翅膀向后抬起蓄势；一蹬跃起再整只俯冲下扑，翅膀后掠，两腿向前伸、趾爪张开抓向前方；
          抓住后停一拍，再撑回站姿。
  hurt    被打：整只向后一仰，翅膀猛地张开，头甩向一侧，飞羽乱颤，晃两下回稳。
  death   侧翻：腿软、翅膀垂张，整只向右侧倒地，头歪垂在地上，脚爪蜷起，之后不动。

坐标与符号（面朝 -Z，+X 是右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 竖着往上长的部件（躯干 / 颈）：rx > 0 = 顶端向后倒，rx < 0 = 向前倾；头的脸朝前：rx > 0 = 抬头；
  - 下垂的肢体（腿）：rx > 0 = 脚往前（-z）摆；
  - 右侧翅从肩往 +x 伸出：rz > 0 = 抬起翼尖，ry > 0 = 向前掠；左侧由 both() 镜像（y / z 分量取反）；
  - 尾向 +z 伸出：rx > 0 = 尾尖下垂；position：-Z = 前，+Y = 上，单位 px。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/can_yu_xiao/gen_can_yu_xiao_anim.py
    ... gen_can_yu_xiao_anim.py --no-gif | --clip fly
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
NAME = "can_yu_xiao"
MODEL = CREATURE_DIR / "CanYuXiao_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim") / NAME

GROUND_TOLERANCE = 0.8   # 落地后最低点入地不得超过它（px）
SAMPLES = 36             # 每段采样数，build_tracks 再裁掉共线帧
TAU = math.tau
AIRBORNE = {"fly"}       # 腾空段：不做贴地约束
SNAP_BOTH = {"death"}    # 翻倒后整只落到地面上，而不是悬在半空


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
def wing(poser: Poser, pose: Pose, side: str, lift: float, sweep: float = 0.0, fold: float = 0.0, flutter: float = 0.0) -> None:
    """一侧翅：lift = 整翅上抬（度，肩臂转 lift，前臂 / 腕手各再补一点，越往外越弯）；sweep = 向前掠（> 0）/ 向后掠（< 0）；
    fold = 前臂和腕手向内折（收翅）；flutter = 飞羽的颤动幅度（度）。"""
    sign = 1.0 if side == "r" else -1.0
    poser.turn(pose, f"wing_1_{side}", rot=(0, sign * sweep, sign * lift))
    poser.turn(pose, f"wing_2_{side}", rot=(0, sign * 0.3 * sweep, sign * (0.5 * lift - fold)))
    poser.turn(pose, f"wing_3_{side}", rot=(0, 0, sign * (0.4 * lift - 0.6 * fold)))
    poser.turn(pose, f"sec_{side}", rot=(flutter, 0, sign * 0.3 * flutter))
    poser.turn(pose, f"prim_{side}", rot=(1.3 * flutter, 0, sign * 0.5 * flutter))


def leg(poser: Poser, pose: Pose, side: str, swing: float, knee: float = 0.0, toes: float = 0.0, talon: float = 0.0) -> None:
    """一条腿：swing = 大腿前后摆（度，+ = 脚往前）；knee = 跗跖相对大腿向后折（> 0 折）；
    toes = 脚趾下压 / 抓握（> 0）；talon = 爪尖再向下钩。"""
    poser.turn(pose, f"leg_1_{side}", rot=(swing, 0, 0))
    poser.turn(pose, f"leg_2_{side}", rot=(-knee, 0, 0))
    poser.turn(pose, f"foot_{side}", rot=(toes - 0.3 * swing + 0.4 * knee, 0, 0))
    poser.turn(pose, f"talon_{side}", rot=(talon, 0, 0))


def head_turn(poser: Poser, pose: Pose, yaw: float, nod: float = 0.0, tilt: float = 0.0) -> None:
    """头颈转向：yaw 的 55% 给颈、45% 给头（猫头鹰能转 270°，这里最多用到 180°）；nod = 抬头（> 0）；tilt = 歪头。"""
    poser.turn(pose, "neck", rot=(0.3 * nod, 0.55 * yaw, 0.4 * tilt))
    poser.turn(pose, "head", rot=(0.7 * nod, 0.45 * yaw, 0.6 * tilt))


def tail(poser: Poser, pose: Pose, droop: float, fan: float = 0.0) -> None:
    poser.turn(pose, "tail_1", rot=(droop, fan, 0))
    poser.turn(pose, "tail_2", rot=(0.7 * droop, 1.3 * fan, 0))


def tufts(poser: Poser, pose: Pose, deg: float) -> None:
    poser.both(pose, "tuft", rot_r=(0, 0, deg))


# ---------------------------------------------------------------- 动作
def can_yu_xiao_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 6s 循环：0~0.12 呼吸；0.12~0.32 把头转到身后 180°；0.32~0.52 停住盯着；0.52~0.72 转回；之后呼吸
        pose = Pose()
        breath = wave(t, 4, 0.0)
        look = keys(t, [(0, 0), (0.12, 0), (0.32, 1), (0.52, 1), (0.72, 0), (1, 0)])
        peek = math.sin(math.pi * (t - 0.52) / 0.14) if 0.52 < t < 0.66 else 0.0      # 转回之前歪一下头
        poser.turn(pose, "body_chest", rot=(0.6 * breath, 0, 0), scale=(1.0 + 0.008 * breath, 1.0 + 0.012 * breath, 1.0))
        poser.turn(pose, "body_hips", rot=(0, 0, 0.5 * wave(t, 1, 0.2)))
        head_turn(poser, pose, 180 * look, nod=1.5 * wave(t, 4, 0.3) * (1 - look) - 2 * look, tilt=3.0 * peek)
        tufts(poser, pose, 3.0 * wave(t, 6, 0.1) * (1 - look) + 4.0 * look * wave(t, 12, 0.0))
        poser.turn(pose, "beak_low", rot=(1.0 * max(0.0, wave(t, 4, 0.5)), 0, 0))
        tail(poser, pose, 1.5 * wave(t, 4, 0.4), fan=2.0 * wave(t, 2, 0.1))
        for side in ("r", "l"):
            wing(poser, pose, side, 1.0 * wave(t, 4, 0.15), flutter=1.5 * wave(t, 8, 0.3))
        return pose

    def walk(t: float) -> Pose:
        # 一周期两步：右脚迈（0~0.5）、左脚迈（0.5~1）；身子随步左右倾，头反向补偿
        pose = Pose()
        a = wave(t, 1, 0.0)
        step_r = max(0.0, wave(t, 1, 0.25))
        step_l = max(0.0, wave(t, 1, 0.75))
        bob = 0.5 - 0.5 * wave(t, 2, 0.25)
        poser.turn(pose, "root", pos=(0, 0.5 * bob, 0))
        poser.turn(pose, "body_hips", rot=(0, 3.0 * a, 4.0 * a))
        poser.turn(pose, "body_chest", rot=(-2.0 + 1.0 * bob, -2.5 * a, -2.0 * a))
        head_turn(poser, pose, 4.0 * a, nod=1.0 * bob, tilt=-6.0 * a)
        tufts(poser, pose, 3.0 * wave(t, 2, 0.3))
        tail(poser, pose, 2.0 + 2.0 * bob, fan=3.0 * a)
        for side in ("r", "l"):
            wing(poser, pose, side, 1.5 + 1.0 * wave(t, 2, 0.1), fold=4.0, flutter=3.0 * wave(t, 2, 0.4))
        leg(poser, pose, "r", 24 * a, knee=20 * step_r, toes=-6 * step_r)
        leg(poser, pose, "l", -24 * a, knee=20 * step_l, toes=-6 * step_l)
        return pose

    def fly(t: float) -> Pose:
        # 1.0s 循环。右翼拍满，左翼幅度小、慢半拍（残翼）；机身随拍打侧滚、一沉一拉，爪收在腹下
        pose = Pose()
        r = wave(t, 1, 0.0)
        l = wave(t, 1, -0.12) * 0.55 + 0.25 * wave(t, 2, 0.3)
        poser.turn(pose, "root", pos=(0, 20.0 + 3.0 * wave(t, 1, 0.3) + 1.2 * wave(t, 2, 0.1), 0), rot=(-8.0, 0, 6.0 * (r - l)))
        poser.turn(pose, "body_hips", rot=(4.0 * wave(t, 1, 0.55), 0, 2.0 * wave(t, 2, 0.2)))
        poser.turn(pose, "body_chest", rot=(-3.0 * wave(t, 1, 0.5), 0, 3.0 * l))
        head_turn(poser, pose, 6.0 * wave(t, 1, 0.6), nod=3.0 * wave(t, 2, 0.5), tilt=-4.0 * (r - l))
        tufts(poser, pose, -8.0 + 3.0 * wave(t, 2, 0.4))
        tail(poser, pose, 8.0 + 8.0 * wave(t, 1, 0.45), fan=5.0 * wave(t, 2, 0.1))
        wing(poser, pose, "r", 12 + 48 * r, sweep=-10 + 10 * wave(t, 1, 0.25), fold=-4 * r, flutter=10 * wave(t, 1, 0.2))
        wing(poser, pose, "l", 6 + 30 * l, sweep=-6 + 6 * wave(t, 1, 0.13), fold=3 + 6 * (1 - l), flutter=14 * wave(t, 2, 0.35))
        for side in ("r", "l"):
            leg(poser, pose, side, -30, knee=-40, toes=26, talon=18)
        return pose

    def attack(t: float) -> Pose:
        # 蓄力（0~0.22）：压低身子、翅膀向后抬起；扑出（0.22~0.52）：整只向前向上再俯冲下压；
        # 抓住（0.52~0.68）停一拍；撑回（0.68~1）
        pose = Pose()
        crouch = keys(t, [(0, 0), (0.22, 1), (0.28, 0.5), (0.36, 0), (1, 0)])
        dive = keys(t, [(0, 0), (0.24, 0), (0.46, 1), (0.68, 1), (0.9, 0), (1, 0)])
        rise = keys(t, [(0, 0), (0.24, 0), (0.34, 1), (0.48, 0), (1, 0)])
        grab = keys(t, [(0, 0), (0.42, 0), (0.52, 1), (0.66, 1), (0.82, 0), (1, 0)])
        flare = keys(t, [(0, 0), (0.2, 1), (0.3, 0.8), (0.46, 0.2), (0.7, 0.3), (1, 0)])
        poser.turn(pose, "root", pos=(0, 9.0 * rise - 1.5 * crouch, -16.0 * dive), rot=(-52 * dive + 10 * crouch, 0, 0))
        poser.turn(pose, "body_hips", rot=(-6 * crouch, 0, 0))
        poser.turn(pose, "body_chest", rot=(-10 * crouch - 6 * dive, 0, 0))
        head_turn(poser, pose, 0, nod=30 * dive - 8 * crouch)
        poser.turn(pose, "beak_low", rot=(18 * dive, 0, 0))
        tufts(poser, pose, -14 * dive)
        tail(poser, pose, -6 * crouch - 14 * dive, fan=10 * dive)
        for side in ("r", "l"):
            wing(poser, pose, side, 40 * flare - 14 * dive, sweep=-34 * dive - 8 * flare, fold=-6 * flare, flutter=8 * flare)
            leg(poser, pose, side, 14 * crouch + 62 * dive + 10 * grab, knee=-22 * dive + 14 * crouch, toes=-30 * dive + 40 * grab, talon=-24 * dive + 50 * grab)
        return pose

    def hurt(t: float) -> Pose:
        # 被打：向后一仰（0~0.15），翅膀猛张、头甩向一侧、飞羽乱颤，晃两下回稳
        pose = Pose()
        hit = keys(t, [(0, 0), (0.14, 1), (0.34, 0.5), (0.55, 0.2), (1, 0)])
        shake = hit * wave(t, 3, 0.0)
        poser.turn(pose, "root", rot=(10 * hit, 3 * shake, 4 * shake), pos=(0, 0, 2.0 * hit))
        poser.turn(pose, "body_chest", rot=(8 * hit, 0, 3 * shake))
        head_turn(poser, pose, 26 * hit * wave(t, 1.5, 0.1), nod=14 * hit, tilt=14 * shake)
        poser.turn(pose, "beak_low", rot=(20 * hit, 0, 0))
        tufts(poser, pose, 18 * hit)
        tail(poser, pose, -12 * hit, fan=8 * shake)
        for side in ("r", "l"):
            wing(poser, pose, side, 44 * hit, sweep=10 * hit, flutter=14 * shake)
            leg(poser, pose, side, -8 * hit, knee=10 * hit, toes=8 * hit)
        return pose

    def death(t: float) -> Pose:
        # 腿软（0~0.3）→ 侧翻（0.22~0.65）→ 翅膀垂张、头歪垂、脚爪蜷起，之后不动
        pose = Pose()
        stagger = math.sin(math.pi * min(1.0, t / 0.2)) if t < 0.2 else 0.0
        sag = keys(t, [(0, 0), (0.15, 0), (0.45, 1), (1, 1)])
        fall = keys(t, [(0, 0), (0.2, 0), (0.62, 1), (1, 1)])
        curl = keys(t, [(0, 0), (0.5, 0), (0.95, 1), (1, 1)])
        poser.turn(pose, "root", rot=(0, 5 * stagger, 82 * fall + 4 * stagger))
        poser.turn(pose, "body_chest", rot=(10 * sag, 0, 0))
        head_turn(poser, pose, 22 * fall, nod=-30 * sag, tilt=26 * fall)
        poser.turn(pose, "beak_low", rot=(24 * sag, 0, 0))
        tufts(poser, pose, -22 * sag)
        tail(poser, pose, 14 * sag)
        poser.turn(pose, "wing_1_r", rot=(0, 0, 46 * sag))
        poser.turn(pose, "wing_2_r", rot=(0, 0, 20 * sag))
        poser.turn(pose, "wing_3_r", rot=(0, 0, 10 * sag))
        poser.turn(pose, "wing_1_l", rot=(0, 0, -30 * sag - 16 * fall))
        poser.turn(pose, "wing_2_l", rot=(0, 0, -16 * sag))
        poser.turn(pose, "wing_3_l", rot=(0, 0, -8 * sag))
        for side in ("r", "l"):
            leg(poser, pose, side, 18 * sag, knee=20 * sag + 20 * curl, toes=40 * curl, talon=50 * curl)
        return pose

    return {
        "idle": (6.0, True, idle), "walk": (1.2, True, walk), "fly": (1.0, True, fly),
        "attack": (1.4, False, attack), "hurt": (0.7, False, hurt), "death": (2.4, False, death),
    }


# ---------------------------------------------------------------- 落地约束 / 自检
def grounded(rig: PoseRig, poser: Poser, sampler, clip: str):
    """腾空段（fly）不约束；其余段整只最低点入地的话把 root 抬到刚好贴地；死亡翻倒两个方向都拉回，倒下后整只落在地面上。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        if clip in AIRBORNE:
            return pose
        lowest = rig.lowest(pose)
        if lowest < 0.0 or clip in SNAP_BOTH:
            poser.turn(pose, "root", pos=(0.0, -lowest, 0.0))
        return pose

    return sample


def check_ground(rig: PoseRig, sampler, clip: str) -> float:
    lowest = min(rig.lowest(sampler(i / 12)) for i in range(13))
    if clip not in AIRBORNE and lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{NAME}.{clip}: 最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    if clip in AIRBORNE and lowest < 6.0:
        raise ValueError(f"{NAME}.{clip}: 飞行时最低点只有 {lowest:.2f}px，没有离地")
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
    author = can_yu_xiao_clips(poser)

    animations = []
    for clip, (length, loop, raw) in author.items():
        sampler = grounded(rig, poser, raw, clip)
        line = f"  {clip}: 最低点 {check_ground(rig, sampler, clip):+.2f}px"
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
