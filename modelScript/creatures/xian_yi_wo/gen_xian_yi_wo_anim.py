#!/usr/bin/env python3
"""藓衣蜗（xian_yi_wo）GeckoLib 动画：idle / walk / attack / retreat / hurt / death 六段，关键帧从零写。

只落资产、不接线：产物写到暂存目录（modelScript/out/xian_yi_wo/），不装进 client 资源树。
流程：贴图版 XianYiWo_tex.bbmodel → 采样成骨骼关键帧 → 挂回 bbmodel → 离线 codec 导出 geo / animation.json
→ creature_anim_frames.py 按「客户端真会读的文件」渲 GIF 到 model-review/anim/xian_yi_wo/。

动作设计（贴地爬行的大蜗牛，背着长满藓的壳）：
  idle     原地伏着：身体随呼吸轻轻起伏，两根长眼柄各自慢慢摇摆、偶尔朝前探一下，短触角试探，
           头微微点动；壳上的藓块随呼吸轻颤，黏痕的亮泡缓缓明灭。
  walk     贴地蠕行：一条「足波」从尾向头顺着四节身体传过去（每节依次前伸拉长、再收缩回弹），
           身体随波起伏，头随步点动，眼柄随身体前后晃；身后三段黏痕一前一后地被拖拽（宽窄交替），保持贴地。
  attack   探颈咬击：先把头颈向后缩起蓄力、眼柄后收，再整个颈猛地前伸下压、嘴张开一口咬下，
           壳因前冲而前倾，随后缓缓收回。
  retreat  缩回壳里：眼柄先折倒收短，头与颈收缩下沉、身体沿身长压短，整只往壳下缩，壳随之下沉扣住；
           收到最小后停住不动。
  hurt     被打：眼柄猛地收短后仰，身体一缩（压扁），头缩向壳，壳抖两下回稳。
  death    软瘫：眼柄无力地垂向两侧，身体塌平摊开，头垂到地上、嘴张开，壳歪向一侧陷下去，
           黏痕向外铺开成一滩；之后不动。

坐标与符号（面朝 -Z，+X 是右侧，绕骨自身枢轴转，R = Rz·Ry·Rx）：
  - 头 / 颈（朝前 -z）：rx > 0 = 前端上抬，rx < 0 = 前端下压；
  - 竖直向上的眼柄：rx < 0 = 向前倒，rx > 0 = 向后倒；rz > 0 = 右眼柄向内（-x）倒（左边镜像）；
  - position：-Z = 前，+Y = 上，单位 px；scale 沿骨自身枢轴缩放（子骨跟着缩放）。

用法：
    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/xian_yi_wo/gen_xian_yi_wo_anim.py
    ... gen_xian_yi_wo_anim.py --no-gif | --clip retreat
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
NAME = "xian_yi_wo"
MODEL = CREATURE_DIR / "XianYiWo_tex.bbmodel"
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
STAGING = REPO / "modelScript" / "out" / NAME
GIF_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/anim") / NAME

SAMPLES = 36
TAU = math.tau
BODY = ("body_1", "body_2", "body_3", "body_4")        # 尾 → 颈；身体链从尾开始，子骨跟着父骨走
SLIME = ("slime_1", "slime_2", "slime_3")
MOSS = ("moss_a", "moss_b", "moss_c", "moss_d")


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

    def both(self, pose: Pose, stem: str, rot_r=(0.0, 0.0, 0.0), rot_l=None, scale=None) -> None:
        """左右成对的骨：rot_r 给右侧；rot_l 缺省按镜像（y / z 分量取反）；scale 两侧同值。"""
        if rot_l is None:
            rot_l = (rot_r[0], -rot_r[1], -rot_r[2])
        self.turn(pose, f"{stem}_r", rot=rot_r, scale=scale)
        self.turn(pose, f"{stem}_l", rot=rot_l, scale=scale)


# ---------------------------------------------------------------- 共用动作件
def stalks(poser: Poser, pose: Pose, lean: float = 0.0, sway: float = 0.0, side: float = 0.0, retract: float = 0.0,
           probe: float = 0.0) -> None:
    """两对触角：lean = 长眼柄整体前倒（度，负 = 向前）；sway = 左右摇（度，两侧错相用 side 给）；
    retract = 缩回程度 0~1（眼柄先折倒、再缩短）；probe = 短触角前探 / 下点的幅度（度）。"""
    for stem, ph in (("r", 0.0), ("l", 0.5)):
        sign = 1.0 if stem == "r" else -1.0
        fold = 70.0 * retract
        poser.turn(pose, f"ant_up_1_{stem}", rot=(lean + fold * 0.6, 0, sign * (-sway * (1 if ph == 0 else -1) - 25 * retract + side)),
                   scale=(1.0, 1.0 - 0.55 * retract, 1.0))
        poser.turn(pose, f"ant_up_2_{stem}", rot=(0.5 * lean + fold * 0.7, 0, sign * -0.5 * sway),
                   scale=(1.0, 1.0 - 0.3 * retract, 1.0))
        poser.turn(pose, f"tent_low_{stem}", rot=(probe + 25 * retract, 0, sign * -8 * retract), scale=(1.0, 1.0, 1.0 - 0.5 * retract))


def moss_quiver(poser: Poser, pose: Pose, t: float, cycles: float, amp: float, phase: float = 0.0) -> None:
    """壳上的藓块和垂挂的藓丝：各自相位错开地轻颤。"""
    for n, stem in enumerate(MOSS):
        poser.both(pose, stem, rot_r=(amp * wave(t, cycles, phase + 0.17 * n), 0, amp * 0.8 * wave(t, cycles, phase + 0.17 * n + 0.3)))


def xian_yi_wo_clips(poser: Poser) -> dict[str, tuple[float, bool, object]]:
    def idle(t: float) -> Pose:
        # 伏着呼吸：身体起伏、长眼柄各自慢摇并偶尔前探、短触角试探、头微点、壳上藓块轻颤、黏痕亮泡明灭
        pose = Pose()
        breath = wave(t, 1, 0.0)
        for n, bone in enumerate(BODY):
            poser.turn(pose, bone, rot=(0.7 * wave(t, 1, 0.1 * n), 0, 0), scale=(1.0 + 0.012 * breath, 1.0 + 0.02 * breath, 1.0))
        poser.turn(pose, "head", rot=(2.0 * wave(t, 1, 0.3), 3.0 * wave(t, 1, 0.55), 0))
        poser.turn(pose, "mouth", scale=(1.0, 1.0 + 0.12 * max(0.0, wave(t, 2, 0.2)), 1.0))
        stalks(poser, pose, lean=-6.0 * max(0.0, wave(t, 1, 0.0)) - 3.0, sway=7.0 * wave(t, 1, 0.1), probe=6.0 * wave(t, 2, 0.15))
        poser.both(pose, "shell_boss", rot_r=(0.8 * breath, 0, 0))
        poser.turn(pose, "shell", rot=(0.6 * wave(t, 1, 0.4), 0, 0.8 * wave(t, 1, 0.7)))
        moss_quiver(poser, pose, t, 2, 1.8)
        for n, bone in enumerate(SLIME):
            poser.turn(pose, bone, scale=(1.0 + 0.04 * wave(t, 1, 0.2 * n), 1.0, 1.0))
        return pose

    def walk(t: float) -> Pose:
        # 足波自尾向头传（body_1 → body_4）：每节依次前伸拉长再回弹；身体起伏，头随步点动，眼柄前后晃；黏痕被拖着宽窄交替
        pose = Pose()
        for n, bone in enumerate(BODY):
            phase = -0.16 * n                                       # 波从尾向头：每节晚 0.16 个周期
            stretch = wave(t, 2, phase)
            poser.turn(pose, bone, rot=(1.6 * wave(t, 2, phase + 0.25), 0, 0), pos=(0, 0.25 * wave(t, 2, phase + 0.25), -0.5 * stretch),
                       scale=(1.0 - 0.025 * stretch, 1.0 - 0.02 * stretch, 1.0 + 0.07 * stretch))
        poser.turn(pose, "head", rot=(3.0 * wave(t, 2, -0.7 + 0.25), 2.5 * wave(t, 1, 0.1), 0))
        poser.turn(pose, "mouth", scale=(1.0, 1.0 + 0.1 * max(0.0, wave(t, 2, -0.2)), 1.0))
        stalks(poser, pose, lean=-8.0 + 5.0 * wave(t, 2, -0.35), sway=5.0 * wave(t, 1, 0.3), probe=7.0 * wave(t, 2, -0.5))
        poser.turn(pose, "shell", rot=(1.2 * wave(t, 2, -0.45), 0, 1.5 * wave(t, 1, 0.2)))
        poser.both(pose, "shell_boss", rot_r=(1.0 * wave(t, 2, -0.5), 0, 0))
        moss_quiver(poser, pose, t, 2, 2.4, phase=-0.3)
        for n, bone in enumerate(SLIME):                            # 黏痕：被身体拖着，一节宽一节窄地前后挪
            poser.turn(pose, bone, scale=(1.0 + 0.1 * wave(t, 2, -0.5 - 0.2 * n), 1.0, 1.0 + 0.1 * wave(t, 2, -0.7 - 0.2 * n)),
                       pos=(0, 0, 0.4 * wave(t, 2, -0.6 - 0.2 * n)))
        return pose

    def attack(t: float) -> Pose:
        # 蓄力（0~0.3）：头颈后缩、抬起，眼柄后收；猛咬（0.3~0.5）：整个颈前伸下压、嘴张开一口咬下，壳前倾；收（0.5~1）：缓缓回位
        pose = Pose()
        coil = keys(t, [(0, 0), (0.3, 1), (0.36, 1), (0.5, 0), (1, 0)])
        lunge = keys(t, [(0, 0), (0.3, 0), (0.46, 1), (0.6, 0.8), (1, 0)])
        bite = keys(t, [(0, 0), (0.3, 0), (0.44, 1), (0.54, 1), (0.64, 0.1), (1, 0)])
        poser.turn(pose, "body_4", rot=(10 * coil - 18 * lunge, 0, 0), pos=(0, 0.4 * coil, 1.2 * coil - 2.2 * lunge),
                   scale=(1.0, 1.0, 1.0 - 0.1 * coil + 0.18 * lunge))
        poser.turn(pose, "body_3", rot=(3 * coil - 6 * lunge, 0, 0), pos=(0, 0, 0.5 * coil - 1.4 * lunge))
        poser.turn(pose, "head", rot=(8 * coil - 22 * lunge, 0, 0))
        poser.turn(pose, "mouth", scale=(1.0 + 0.5 * bite, 1.0 + 1.6 * bite, 1.0 + 0.6 * bite))
        stalks(poser, pose, lean=14 * coil - 4 * lunge, retract=0.5 * coil + 0.35 * lunge, probe=-18 * lunge)
        poser.turn(pose, "shell", rot=(-6 * lunge + 3 * coil, 0, 0))
        moss_quiver(poser, pose, t, 3, 3.0 * (coil + lunge))
        return pose

    def retreat(t: float) -> Pose:
        # 缩回壳里：眼柄折倒收短（0~0.4），头与颈收缩下沉、身体压短（0.2~0.8），壳扣下（0.5~1），收到最小停住
        pose = Pose()
        stalk_in = keys(t, [(0, 0), (0.4, 1), (1, 1)])
        head_in = keys(t, [(0, 0), (0.2, 0), (0.7, 1), (1, 1)])
        body_in = keys(t, [(0, 0), (0.3, 0), (0.85, 1), (1, 1)])
        shell_down = keys(t, [(0, 0), (0.5, 0), (0.95, 1), (1, 1)])
        stalks(poser, pose, lean=0.0, retract=stalk_in, probe=0.0)
        poser.turn(pose, "head", rot=(-30 * head_in, 0, 0), pos=(0, -2.0 * head_in, 3.0 * head_in), scale=(1.0 - 0.3 * head_in, 1.0 - 0.3 * head_in, 1.0 - 0.45 * head_in))
        poser.turn(pose, "body_4", rot=(-16 * head_in, 0, 0), pos=(0, -0.8 * head_in, 2.6 * body_in), scale=(1.0 - 0.15 * body_in, 1.0 - 0.15 * body_in, 1.0 - 0.45 * body_in))
        poser.turn(pose, "body_3", pos=(0, -0.6 * body_in, 1.6 * body_in), scale=(1.0 - 0.1 * body_in, 1.0 - 0.1 * body_in, 1.0 - 0.35 * body_in))
        poser.turn(pose, "body_2", pos=(0, 0, 0.8 * body_in), scale=(1.0, 1.0 - 0.08 * body_in, 1.0 - 0.25 * body_in))
        poser.turn(pose, "body_1", scale=(1.0, 1.0, 1.0 - 0.2 * body_in))
        poser.turn(pose, "mouth", scale=(1.0 - 0.3 * head_in, 1.0 - 0.3 * head_in, 1.0 - 0.3 * head_in))
        poser.turn(pose, "shell", rot=(-10 * shell_down, 0, 0), pos=(0, -1.6 * shell_down, -1.4 * shell_down))
        for n, bone in enumerate(SLIME):
            poser.turn(pose, bone, scale=(1.0 - 0.15 * body_in, 1.0, 1.0 - 0.3 * body_in))
        return pose

    def hurt(t: float) -> Pose:
        # 被打：眼柄猛地收短后仰，身体一缩压扁，头缩向壳，壳抖两下回稳
        pose = Pose()
        hit = math.sin(math.pi * min(1.0, t * 1.1)) * (1 - 0.25 * t)
        rattle = math.sin(TAU * 3 * t) * (1 - t)
        stalks(poser, pose, lean=0.0, retract=0.85 * hit, probe=10 * hit)
        for n, bone in enumerate(BODY):
            poser.turn(pose, bone, scale=(1.0 + 0.1 * hit, 1.0 - 0.14 * hit, 1.0 - 0.1 * hit), pos=(0, 0, 0.5 * hit * (n / 3)))
        poser.turn(pose, "head", rot=(-14 * hit, 5 * rattle, 0), pos=(0, -0.6 * hit, 1.2 * hit))
        poser.turn(pose, "mouth", scale=(1.0, 1.0 + 0.4 * hit, 1.0))
        poser.turn(pose, "shell", rot=(3 * rattle, 0, 5 * rattle), pos=(0, -0.4 * hit, 0.5 * hit))
        moss_quiver(poser, pose, t, 3, 5.0 * (1 - t))
        return pose

    def death(t: float) -> Pose:
        # 软瘫：眼柄垂向两侧，身体塌平摊开，头垂到地、嘴张开，壳歪向一侧陷下去，黏痕向外铺成一滩
        pose = Pose()
        droop = keys(t, [(0, 0), (0.4, 1), (1, 1)])
        flat = keys(t, [(0, 0), (0.2, 0), (0.75, 1), (1, 1)])
        tilt = keys(t, [(0, 0), (0.3, 0), (0.85, 1), (1, 1)])
        pool = keys(t, [(0, 0), (0.4, 0), (1, 1)])
        for n, bone in enumerate(BODY):
            poser.turn(pose, bone, scale=(1.0 + 0.25 * flat, 1.0 - 0.45 * flat, 1.0 + 0.05 * flat), pos=(0, -0.1 * flat, 0))
        poser.turn(pose, "head", rot=(-22 * droop, 0, 10 * droop), pos=(0, -1.0 * droop, 0))
        poser.turn(pose, "mouth", scale=(1.0 + 0.3 * droop, 1.0 + 0.9 * droop, 1.0 + 0.3 * droop))
        for stem in ("r", "l"):
            sign = 1.0 if stem == "r" else -1.0
            poser.turn(pose, f"ant_up_1_{stem}", rot=(-35 * droop, 0, sign * -70 * droop), scale=(1.0, 1.0 - 0.25 * droop, 1.0))
            poser.turn(pose, f"ant_up_2_{stem}", rot=(-30 * droop, 0, sign * -40 * droop))
            poser.turn(pose, f"tent_low_{stem}", rot=(20 * droop, 0, sign * -30 * droop))
        poser.turn(pose, "shell", rot=(4 * tilt, 0, 16 * tilt), pos=(0, -1.8 * tilt, 0))
        moss_quiver(poser, pose, t, 1, 2.0 * (1 - tilt))
        for n, bone in enumerate(SLIME):
            poser.turn(pose, bone, scale=(1.0 + 0.45 * pool, 1.0, 1.0 + 0.1 * pool))
        return pose

    return {
        "idle": (3.0, True, idle), "walk": (1.6, True, walk), "attack": (1.0, False, attack),
        "retreat": (1.6, False, retreat), "hurt": (0.6, False, hurt), "death": (2.2, False, death),
    }


# ---------------------------------------------------------------- 贴地约束 / 自检
def grounded(rig: PoseRig, poser: Poser, sampler):
    """所有段共用：整只最低点贴到地面（y = 0）——缩身 / 塌平会让最低点离地，往上抬的会入地，两个方向都把 root 拉回来。
    蜗牛是贴地的，没有腾空段，所以不像四足那样只管入地。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        lowest = rig.lowest(pose)
        poser.turn(pose, "root", pos=(0.0, -lowest, 0.0))
        return pose

    return sample


def check_ground(rig: PoseRig, sampler) -> float:
    """整段里最低点相对地面的最大偏差（snap 后应当 ≈ 0）。"""
    return max(abs(rig.lowest(sampler(i / 12))) for i in range(13))


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
    author = xian_yi_wo_clips(poser)

    animations = []
    for clip, (length, loop, raw) in author.items():
        sampler = grounded(rig, poser, raw)
        line = f"  {clip}: 贴地偏差 {check_ground(rig, sampler):.2f}px"
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
