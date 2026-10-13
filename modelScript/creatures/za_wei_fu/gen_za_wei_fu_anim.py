#!/usr/bin/env python3
"""杂尾蝠（za_wei_fu）GeckoLib 动画：idle / walk / run / attack / hurt / death 六段，关键帧从零写。

只落资产、不接线：产物写到暂存目录（modelScript/out/za_wei_fu/），不装进 client 资源树。
流程：贴图版 ZaWeiFu_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/za_wei_fu/。

蝠是飞行生物，悬停姿态：身体直立、翼向两侧撑开、尾垂在身后下方。三段飞行：
  idle  原地悬停小幅振翅；walk  巡航（移动时的缓振翅，躯干微前倾）；run  快速扑飞（躯干前压、翼后掠急振、尾后飘）。

坐标与符号（面朝 -Z，+X 是右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 右翼：rz > 0 = 翼尖上扬、rz < 0 = 下拍；左翼相反（镜像时 y / z 分量取反，由 Poser.both 统一处理）；
  - 翼 ry > 0（右翼）= 翼尖往前掠，< 0 = 往后掠；
  - 躯干 / 头在枢轴之上：rx < 0 = 前倾 / 低头；下垂的尾 / 腿：rx > 0 = 往前摆；下颌 rx < 0 = 张口；
  - position：-Z = 前，+Y = 上，单位 px。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/za_wei_fu/gen_za_wei_fu_anim.py
    ... gen_za_wei_fu_anim.py --no-gif | --clip run
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
NAME = "za_wei_fu"
MODEL = CREATURE_DIR / "ZaWeiFu_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim") / NAME

GROUND_TOLERANCE = 0.6   # 落地后（death 末段）最低点入地不得超过它（px）
SAMPLES = 36             # 每段采样数，build_tracks 再裁掉共线帧
TAU = math.tau

TAIL = ("tail_1", "tail_2", "tail_3", "tail_4", "tail_5", "tail_6")
FINGERS = ("finger_a", "finger_b", "finger_c")


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


# ---------------------------------------------------------------- 振翅与身体
# 翼链（肩 → 上臂 → 前臂 → 腕 → 指）上的旋转沿骨链累加，所以「翼尖总拍幅」要按份额分到各段，
# 不能每段都给满幅（否则累加成 3 倍，骨条甩出去而翼膜没跟上）。肩占一半，其余逐段递减。
WING_CHAIN = ("wing_shoulder", "wing_arm", "wing_fore", "wing_hand", "finger")
WING_SHARE = (0.5, 0.15, 0.15, 0.12, 0.1)


def wing_chain(poser: Poser, pose: Pose, tip: list[float], fold: float = 0.0) -> None:
    """tip[i] = 第 i 段骨自己这一刻的「翼尖总拍幅」目标（度，已含各段滞后）；按 WING_SHARE 分到每段。
    fold = 肘腕额外折收量（度，翼收拢下垂用，分摊在前臂 / 腕 / 指）。"""
    for i, (stem, share) in enumerate(zip(WING_CHAIN, WING_SHARE)):
        extra = (0.0, 0.0, 0.4 * fold, 0.35 * fold, 0.25 * fold)[i]
        rz = share * tip[i] - extra
        if stem == "finger":
            for k, finger in enumerate(FINGERS):
                poser.both(pose, finger, rot_r=(0, 0, rz))
        else:
            poser.both(pose, stem, rot_r=(0, 0, rz))


def flap(poser: Poser, pose: Pose, t: float, cycles: float, amp: float, sweep: float, lag: float = 0.05,
         phase: float = 0.0) -> None:
    """翼逐段滞后地上下拍：每段比上一段晚 lag 个周期，amp = 翼尖总拍幅（度）；
    sweep = 下拍时翼往前掠的幅度（度，上扬时往后收）。"""
    phi = cycles * t + phase
    stroke = math.sin(TAU * phi)
    wing_chain(poser, pose, [amp * math.sin(TAU * (phi - i * lag)) for i in range(5)])
    poser.both(pose, "wing_shoulder", rot_r=(0, -0.6 * sweep * stroke, 0))
    poser.both(pose, "wing_arm", rot_r=(0, -0.4 * sweep * stroke, 0))
    for i, finger in enumerate(FINGERS):
        poser.both(pose, finger, rot_r=(0, (i - 1) * 3.0 * stroke, 0))
    poser.both(pose, "thumb", rot_r=(0, 0, 0.1 * amp * math.sin(TAU * (phi - 3 * lag))))


def tail_wave(poser: Poser, pose: Pose, t: float, cycles: float, amp: float, base: float = 0.0, side: float = 0.0) -> None:
    """尾巴六节逐节滞后摆动（前后摆 + 一点左右）；base 是整条尾的静态前后偏角（正 = 往前翘 / 往上飘）。"""
    for i, bone in enumerate(TAIL):
        grow = 0.55 + 0.3 * i
        poser.turn(pose, bone, rot=(base / len(TAIL) + amp * grow * wave(t, cycles, -0.1 * i), 0,
                                    side * grow * wave(t, cycles, -0.1 * i + 0.25)))


def ears_and_head(poser: Poser, pose: Pose, t: float, cycles: float, flick: float, pitch: float = 0.0) -> None:
    poser.turn(pose, "neck", rot=(-0.4 * pitch, 0, 0))
    poser.turn(pose, "head", rot=(-0.6 * pitch + 1.5 * wave(t, cycles, 0.3), 3.0 * wave(t, 1, 0.2), 1.0 * wave(t, 1, 0.55)))
    poser.both(pose, "ear", rot_r=(0, 0, flick * wave(t, cycles * 2, 0.1)), rot_l=(0, 0, -flick * wave(t, cycles * 2, 0.35)))
    poser.both(pose, "ear_tip", rot_r=(0, 0, 1.6 * flick * wave(t, cycles * 2, 0.3)),
               rot_l=(0, 0, -1.6 * flick * wave(t, cycles * 2, 0.55)))


def tuck_legs(poser: Poser, pose: Pose, thigh: float, shin: float, foot: float, splay: float = 0.0) -> None:
    poser.both(pose, "leg_thigh", rot_r=(thigh, 0, splay))
    poser.both(pose, "leg_shin", rot_r=(shin, 0, 0))
    poser.both(pose, "foot", rot_r=(foot, 0, 0))


def za_wei_fu_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 原地悬停：每秒约 1.5 次缓振翅，身体随拍翅轻起伏，尾巴慢摆，下颌偶尔张合，耳朵抖动
        pose = Pose()
        cycles = 3.0
        flap(poser, pose, t, cycles, amp=44, sweep=8)
        poser.turn(pose, "body_hips", pos=(0, -0.7 * wave(t, cycles), 0), rot=(1.5 * wave(t, cycles, 0.2), 0, 0))
        poser.turn(pose, "body_chest", rot=(-1.5 * wave(t, cycles, 0.25), 1.5 * wave(t, 1, 0.1), 0))
        ears_and_head(poser, pose, t, cycles, flick=3.0)
        gape = math.sin(math.pi * smooth((t - 0.55) / 0.2)) if 0.55 < t < 0.75 else 0.0
        poser.turn(pose, "jaw", rot=(-5.0 - 14.0 * gape, 0, 0))
        tuck_legs(poser, pose, thigh=-12 + 3 * wave(t, cycles, 0.3), shin=20, foot=14, splay=4)
        tail_wave(poser, pose, t, 1, amp=5.0, side=3.0)
        return pose

    def walk(t: float) -> Pose:
        # 巡航：2 Hz 振翅，躯干微前倾，尾往后拖，腿收在腹下
        pose = Pose()
        cycles = 2.0
        flap(poser, pose, t, cycles, amp=54, sweep=12)
        poser.turn(pose, "body_hips", pos=(0, -0.9 * wave(t, cycles), -0.4), rot=(-5 + 1.5 * wave(t, cycles, 0.2), 0, 0))
        poser.turn(pose, "body_chest", rot=(-5 - 1.5 * wave(t, cycles, 0.25), 2 * wave(t, 1, 0.1), 0))
        ears_and_head(poser, pose, t, cycles, flick=3.5, pitch=5)
        poser.turn(pose, "jaw", rot=(-4.0 - 3.0 * max(0.0, wave(t, cycles, 0.3)), 0, 0))
        tuck_legs(poser, pose, thigh=-20, shin=30, foot=18, splay=4)
        tail_wave(poser, pose, t, cycles, amp=6.0, base=-8, side=4.0)
        return pose

    def run(t: float) -> Pose:
        # 快速扑飞：每秒 5 次急振翅，翼后掠，躯干前压成俯冲姿，头低下张口，尾向后上方飘，爪前伸预备抓人
        pose = Pose()
        cycles = 3.0
        flap(poser, pose, t, cycles, amp=62, sweep=-14, lag=0.045)
        poser.turn(pose, "body_hips", pos=(0, -1.2 * wave(t, cycles), -1.2), rot=(-26 + 2 * wave(t, cycles, 0.2), 0, 0))
        poser.turn(pose, "body_chest", rot=(-12 - 2 * wave(t, cycles, 0.25), 2.5 * wave(t, cycles, 0.4), 0))
        ears_and_head(poser, pose, t, cycles, flick=5.0, pitch=18)
        poser.both(pose, "ear", rot_r=(-14, 0, 0))
        poser.turn(pose, "jaw", rot=(-16.0 - 6.0 * max(0.0, wave(t, cycles, 0.3)), 0, 0))
        tuck_legs(poser, pose, thigh=40, shin=-30, foot=-16, splay=8)          # 爪前伸
        tail_wave(poser, pose, t, cycles, amp=9.0, base=40, side=7.0)           # 尾被气流带向后上方
        return pose

    def attack(t: float) -> Pose:
        # 扑咬：先后仰收翼蓄力（爪缩起、口张开），再整体前冲、翼急拍一下、双爪前抓、猛咬，最后回稳
        pose = Pose()
        wind = keys(t, [(0, 0), (0.3, 1), (0.42, 1), (0.55, 0), (1, 0)])
        lunge = keys(t, [(0, 0), (0.3, 0), (0.5, 1), (0.7, 0.8), (1, 0)])
        bite = keys(t, [(0, 0), (0.3, 0.8), (0.5, 1), (0.6, 0), (0.7, 0), (1, 0)])
        poser.turn(pose, "root", pos=(0, 0.6 * wind, -6.0 * lunge))
        poser.turn(pose, "body_hips", rot=(10 * wind - 22 * lunge, 0, 0))
        poser.turn(pose, "body_chest", rot=(8 * wind - 12 * lunge, 0, 0))
        poser.turn(pose, "neck", rot=(6 * wind - 10 * lunge, 0, 0))
        poser.turn(pose, "head", rot=(8 * wind - 14 * lunge, 0, 0))
        poser.turn(pose, "jaw", rot=(-34 * bite, 0, 0))
        # 翼：蓄力时上举收拢，前冲时猛拍到下方（翼尖总拍幅，逐段晚 0.02 个动作时长）
        def tip(u: float) -> float:
            return keys(u, [(0, 0), (0.3, 55), (0.42, 55), (0.57, -60), (0.78, -12), (1, 0)])

        wing_chain(poser, pose, [tip(t - 0.02 * i) for i in range(5)])
        poser.both(pose, "wing_shoulder", rot_r=(0, 12 * lunge, 0))
        # 腿爪：蓄力缩起，前冲时前伸张开抓
        tuck_legs(poser, pose, thigh=-18 * wind + 52 * lunge, shin=26 * wind - 28 * lunge, foot=16 * wind - 22 * lunge, splay=10 * lunge)
        tail_wave(poser, pose, t, 1, amp=0.0, base=-14 * wind + 34 * lunge)
        poser.both(pose, "ear", rot_r=(-10 * lunge, 0, 4 * wind))
        return pose

    def hurt(t: float) -> Pose:
        # 被打：身体被打得后仰，翼乱了拍子（一边猛上扬一边下垂），尾甩开，口张开
        pose = Pose()
        hit = math.sin(math.pi * min(1.0, t * 1.1)) * (1 - 0.25 * t)
        rattle = math.sin(TAU * 3 * t) * (1 - t)
        poser.turn(pose, "root", pos=(0, 1.0 * hit, 2.4 * hit), rot=(0, 0, 8 * rattle))
        poser.turn(pose, "body_hips", rot=(24 * hit, 6 * rattle, 0))
        poser.turn(pose, "body_chest", rot=(10 * hit, 0, 0))
        poser.turn(pose, "head", rot=(14 * hit, 8 * rattle, 0))
        poser.turn(pose, "jaw", rot=(-26 * hit, 0, 0))
        # 一边翼被打得猛上扬、另一边下垂：右翼 / 左翼各用自己的翼尖幅度（右翼上扬 +，左翼镜像规则下 + 也是上扬）
        for side, tip_amp in (("r", 62 * hit + 10 * rattle), ("l", 24 * hit + 12 * rattle)):
            for stem, share in zip(WING_CHAIN[:4], WING_SHARE[:4]):
                sign = 1.0 if side == "r" else -1.0
                poser.turn(pose, f"{stem}_{side}", rot=(0, 0, sign * share * tip_amp))
            for finger in FINGERS:
                poser.turn(pose, f"{finger}_{side}", rot=(0, 0, (1.0 if side == "r" else -1.0) * WING_SHARE[4] * tip_amp))
        poser.both(pose, "ear", rot_r=(0, 0, 14 * hit), rot_l=(0, 0, -14 * hit))
        tuck_legs(poser, pose, thigh=20 * hit, shin=-20 * hit, foot=-10 * hit, splay=12 * hit)
        tail_wave(poser, pose, t, 3, amp=10.0 * (1 - t), base=30 * hit, side=14 * (1 - t))
        return pose

    def death(t: float) -> Pose:
        # 坠落：先挣扎着拍两下，力竭后翼收拢下垂，身体翻转着栽向地面，侧躺落地；落地后四肢和尾松垂不动
        pose = Pose()
        weak = keys(t, [(0, 1), (0.28, 0.55), (0.45, 0)])
        fall = keys(t, [(0, 0), (0.22, 0), (0.78, 1), (1, 1)])
        roll = keys(t, [(0, 0), (0.25, 0), (0.8, 1), (1, 1)])
        settle = keys(t, [(0, 0), (0.78, 0), (0.86, 1), (1, 0.2)])
        phi = 2.0 * min(t / 0.45, 1.0)
        stroke = math.sin(TAU * phi) * weak
        sag = smooth((t - 0.25) / 0.4)
        poser.turn(pose, "root", pos=(0, -14.0 * fall, -3.0 * fall), rot=(-32 * fall + 6 * settle, 0, 84 * roll - 6 * settle))
        poser.turn(pose, "body_hips", rot=(14 * sag, 0, 0))
        poser.turn(pose, "body_chest", rot=(-10 * sag, 0, 0))
        poser.turn(pose, "head", rot=(-14 * sag, 0, 8 * sag))
        poser.turn(pose, "jaw", rot=(-30 * keys(t, [(0, 0), (0.3, 0.9), (1, 1)]), 0, 0))
        # 翼：先无力地拍两下（翼尖总拍幅 40°×weak），随后整条翼往下垂到约 -90°
        wing_chain(poser, pose, [40 * stroke - 90 * sag] * 5, fold=20 * sag)
        poser.both(pose, "ear", rot_r=(0, 0, 18 * sag), rot_l=(0, 0, -18 * sag))
        tuck_legs(poser, pose, thigh=14 * sag, shin=-24 * sag, foot=-14 * sag, splay=16 * sag)
        tail_wave(poser, pose, t, 2, amp=7.0 * weak, base=40 * sag, side=10 * sag)
        return pose

    return {
        "idle": (2.0, True, idle), "walk": (1.0, True, walk), "run": (0.6, True, run),
        "attack": (0.9, False, attack), "hurt": (0.5, False, hurt), "death": (1.8, False, death),
    }


# ---------------------------------------------------------------- 落地约束 / 自检
def landed(rig: PoseRig, poser: Poser, sampler):
    """death 专用：身体落到地面后，整体被地面托住（最低点不入地）。坠落过程在空中，不做约束，
    所以托起量随 t 在 0.55~0.8 之间渐入，不会在起手处把整只蝠顶一下。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < 0.0:
            poser.turn(pose, "root", pos=(0.0, -depth * smooth((t - 0.55) / 0.25), 0.0))
        return pose

    return sample


def check_landing(rig: PoseRig, sampler) -> float:
    """death 末段（t ≥ 0.85）的最低点；入地超过容差报错。"""
    lowest = min(rig.lowest(sampler(0.85 + 0.15 * i / 6)) for i in range(7))
    if lowest < -GROUND_TOLERANCE:
        raise ValueError(f"{NAME}.death: 落地后最低点 {lowest:.2f}px，入地超过 {GROUND_TOLERANCE}px")
    return lowest


def loop_gap(sampler) -> tuple[float, float]:
    """循环段首尾姿态的最大差（旋转度数、位移 px）。"""
    a, b = sampler(0.0), sampler(1.0)
    rot = pos = 0.0
    for bone in set(a) | set(b):
        rot = max(rot, max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)))
        pos = max(pos, max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)))
    return rot, pos


# ---------------------------------------------------------------- 导出
def export() -> Path:
    rig = PoseRig(MODEL)
    poser = Poser(rig)
    author = za_wei_fu_clips(poser)

    animations = []
    for clip, (length, loop, raw) in author.items():
        sampler = landed(rig, poser, raw) if clip == "death" else raw
        line = f"  {clip}:"
        if clip == "death":
            line += f" 落地后最低点 {check_landing(rig, sampler):+.2f}px"
        if loop:
            rot, pos = loop_gap(sampler)
            if rot > 0.5 or pos > 0.1:
                raise ValueError(f"{NAME}.{clip}: 循环首尾不同值（旋转差 {rot:.2f}° 位移差 {pos:.2f}px）")
            line += f" 首尾差 {rot:.2f}° / {pos:.2f}px"
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
