#!/usr/bin/env python3
"""枯灯蛾（ku_deng_e）动画：idle / fly / attack / hurt / death 五段，关键帧从零写。

骨架来自 ``KuDengE_tex.bbmodel``（gen_ku_deng_e.py --tex 产出）：thorax 胸、head 头、3 节触角、4 节腹 + 腹尖、
前后翅各 3 段（翅根 / 翅中 / 翅尖）、三对腿各 3 段。**作者空间**：身体朝 -z，右侧 +x；骨的旋转按 Blockbench 的
R = Rz·Ry·Rx 绕自己的枢轴。本文件的符号约定（对着公式推出来的，不是凭感觉）：

    翅（沿 ±x 伸出）  上扬 = 右翅 rot z 为正、左翅为负      → 记 s = +1(右) / -1(左)，rot z = s·角度
    翅后掠            右翅 rot y 为负、左翅为正              → rot y = -s·角度
    身体 / 腹 / 腿    rot x 为正 = 头（-z 端）抬起、腹（+z 端）下垂、下垂的腿向前（-z）抬
    触角（朝上）      rot x 为正 = 向后仰

腹尖「残光明暗」：骨动画只能改变换，不能改颜色，所以用腹尖的**缩放**脉动代替（亮的时候大、暗的时候小）；
颜色明暗留给之后接线时的发光层。循环段首尾姿态必须一致（门禁里自证）。

产物（不装进 client 资源树）：
    modelScript/out/ku_deng_e/KuDengE_animated.bbmodel      带动画的作者稿
    modelScript/out/ku_deng_e/ku_deng_e.{geo,animation}.json 经离线 codec 导出的 GeckoLib 文件
    <scratch>/assets/{geo,animations,textures/entity/fauna}/ 给 creature_anim_frames.py 用的临时资源根

用法：
    python gen_anim.py                        # 导出 + 校验
    python gen_anim.py --gif <审阅目录>        # 另出每段 GIF
"""

from __future__ import annotations

import argparse
import base64
import json
import math
import subprocess
import sys
from pathlib import Path
from typing import Callable

from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, smooth, write_animated_bbmodel

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
SOURCE = CREATURE_DIR / "KuDengE_tex.bbmodel"
MODEL_ID = "ku_deng_e"
OUT = REPO / "modelScript" / "out" / MODEL_ID
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
SAMPLES = 40

Sampler = Callable[[float], Pose]

SIDES = (("r", 1.0), ("l", -1.0))
LEGS = ("f", "m", "h")
GROUND_TOLERANCE = 0.05


# ---------------------------------------------------------------- 曲线 / 摆姿工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t ∈ [0,1] 上走 cycles 个整周期的正弦，循环动画首尾自然相接。"""
    return math.sin(math.tau * (cycles * t + phase))


def ramp(t: float, t0: float, t1: float) -> float:
    return smooth(min(1.0, max(0.0, (t - t0) / (t1 - t0))))


def keys(t: float, points: list[tuple[float, float]]) -> float:
    """分段 smoothstep 插值：points 是 (t01, 值) 的有序表。"""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0))
    return points[-1][1]


def add(pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0)) -> None:
    """往骨上累加旋转 / 位移（同一骨多次调用会叠加）。"""
    channel = pose[bone]
    channel.rot = [a + b for a, b in zip(channel.rot, rot)]
    channel.pos = [a + b for a, b in zip(channel.pos, pos)]


def scale(pose: Pose, bone: str, s: float) -> None:
    pose[bone].scale = [s, s, s]


def wings(pose: Pose, fore, hind, sweep=0.0) -> None:
    """翅：fore / hind 各是三段（根 / 中 / 尖）的上扬角度（度）；sweep 是整体后掠角。"""
    for kind, angles in (("fw", fore), ("hw", hind)):
        for seg, angle in enumerate(angles, start=1):
            for side, s in SIDES:
                add(pose, f"{kind}_{seg}_{side}", rot=(0.0, -s * sweep * (seg / 3.0), s * angle))


def legs(pose: Pose, front: float, mid: float, hind: float, curl: float = 1.0, splay: float = 0.0) -> None:
    """腿：三对各给股节的前抬角（度），胫 / 跗节按 curl 倍率跟着弯；splay 是整体外撇。"""
    for key, base in zip(LEGS, (front, mid, hind)):
        for side, s in SIDES:
            add(pose, f"leg_{key}1_{side}", rot=(base, 0.0, s * splay))
            add(pose, f"leg_{key}2_{side}", rot=(base * 0.6 * curl, 0.0, 0.0))
            add(pose, f"leg_{key}3_{side}", rot=(base * 0.4 * curl, 0.0, 0.0))


def abdomen(pose: Pose, bend: float, lag: float = 0.0, t: float = 0.0, cycles: float = 1.0) -> None:
    """腹四节：每节绕 x 弯 bend 度（正 = 下垂）；lag > 0 时按节错相摆动（t, cycles 给波形）。"""
    for i in range(1, 5):
        add(pose, f"abd_{i}", rot=(bend + (lag * wave(t, cycles, -0.1 * i) if lag else 0.0), 0.0, 0.0))


def antennae(pose: Pose, back: float = 0.0, sway: float = 0.0, t: float = 0.0, cycles: float = 1.0) -> None:
    for seg in (1, 2, 3):
        for side, s in SIDES:
            add(pose, f"ant_{seg}_{side}", rot=(back / 3.0, 0.0, s * sway * wave(t, cycles, 0.12 * seg)))


def glow(pose: Pose, size: float) -> None:
    scale(pose, "abd_tip", size)


# ---------------------------------------------------------------- 五段动作
def idle(t: float) -> Pose:
    """停栖（4s 循环）：翅半张、缓慢开合；触角轻颤；腹随呼吸起伏；残光慢慢明灭；腿抓着不动。"""
    pose = Pose()
    open_ = 0.5 + 0.5 * wave(t, 1)                              # 0..1
    wings(pose, (8 + 12 * open_, 3 * wave(t, 1, -0.08), 2 * wave(t, 1, -0.16)),
          (5 + 10 * open_, 3 * wave(t, 1, -0.1), 2 * wave(t, 1, -0.2)), sweep=6 * (1 - open_))
    legs(pose, 10, 6, 4, splay=2)
    abdomen(pose, 2.0, lag=2.0, t=t)
    antennae(pose, sway=4.0, t=t, cycles=2)
    add(pose, "thorax", pos=(0.0, 0.25 * wave(t, 1), 0.0))
    add(pose, "proboscis", rot=(4.0 * wave(t, 1, 0.3), 0.0, 0.0))
    glow(pose, 1.0 + 0.2 * (0.5 + 0.5 * wave(t, 1)))
    return pose


def fly(t: float) -> Pose:
    """扑翅（0.5s 循环）：翅根大幅上下、翅中翅尖依次滞后（波浪外传）；身体随扑翅起伏；腿后拖；腹尖残光一周期明灭两次。"""
    pose = Pose()
    wings(pose,
          (8 + 36 * wave(t, 1), 24 * wave(t, 1, -0.07), 18 * wave(t, 1, -0.14)),
          (4 + 30 * wave(t, 1, -0.04), 20 * wave(t, 1, -0.11), 14 * wave(t, 1, -0.18)),
          sweep=8 * (0.5 + 0.5 * wave(t, 1, 0.25)))
    add(pose, "thorax", rot=(3.0 * wave(t, 1, 0.25), 0.0, 0.0), pos=(0.0, -1.0 * wave(t, 1, 0.05), 0.0))
    add(pose, "head", rot=(-2.0 * wave(t, 1, 0.25), 0.0, 0.0))
    legs(pose, -18, -26, -34, curl=0.6)
    for key in LEGS:
        for side, _ in SIDES:
            add(pose, f"leg_{key}2_{side}", rot=(5.0 * wave(t, 1, 0.15), 0.0, 0.0))
    abdomen(pose, 4.0, lag=6.0, t=t)
    antennae(pose, back=-6.0, sway=3.0, t=t)
    glow(pose, 1.0 + 0.4 * (0.5 + 0.5 * wave(t, 2)))
    return pose


def attack(t: float) -> Pose:
    """俯冲扑脸（0.9s 一次性）：0–0.3 蓄势（后仰、翅高举、腹蜷起）→ 0.3–0.6 前冲下压（头向下、腿前伸、翅后掠、残光亮起）→ 恢复。
    所有项在 t=0 / t=1 为 0，首尾回到静止姿态。"""
    pull = keys(t, [(0.0, 0.0), (0.3, 1.0), (0.6, 0.0), (1.0, 0.0)])
    dive = keys(t, [(0.0, 0.0), (0.3, 0.0), (0.6, 1.0), (0.8, 0.6), (1.0, 0.0)])
    env = math.sin(math.pi * t)
    pose = Pose()
    wings(pose,
          (40 * pull - 6 * dive + 16 * env * wave(t, 3), 10 * pull + 12 * env * wave(t, 3, -0.08), 6 * pull + 10 * env * wave(t, 3, -0.16)),
          (34 * pull - 4 * dive + 14 * env * wave(t, 3, -0.04), 8 * pull + 10 * env * wave(t, 3, -0.12), 5 * pull + 8 * env * wave(t, 3, -0.2)),
          sweep=38 * dive)
    add(pose, "root", pos=(0.0, 2.0 * pull - 3.0 * dive, 3.0 * pull - 14.0 * dive))
    add(pose, "thorax", rot=(12 * pull - 38 * dive, 0.0, 0.0))
    add(pose, "head", rot=(-6 * pull - 10 * dive, 0.0, 0.0))
    legs(pose, 55 * dive - 10 * pull, 30 * dive - 6 * pull, 10 * dive - 4 * pull, curl=1.0, splay=10 * dive)
    abdomen(pose, 10 * pull - 8 * dive)
    antennae(pose, back=-30 * dive + 12 * pull)
    add(pose, "proboscis", rot=(30 * dive, 0.0, 0.0))
    glow(pose, 1.0 + 0.8 * dive)
    return pose


def hurt(t: float) -> Pose:
    """受击（0.5s 一次性）：整体被打得后仰、翅猛地一抖然后乱扑、腿蜷缩、触角后折、残光闪烁。"""
    decay = 1.0 - ramp(t, 0.0, 1.0)
    flutter = 2.0 * math.sin(math.pi * t) * (1.0 - t)          # 抖动包络：首尾都为 0（带相位差的波不能只靠 decay）
    jolt = math.sin(math.pi * min(1.0, t * 4.0)) if t < 0.25 else 0.0
    pose = Pose()
    wings(pose,
          (30 * jolt + 14 * flutter * wave(t, 4), 10 * flutter * wave(t, 4, -0.08), 8 * flutter * wave(t, 4, -0.16)),
          (24 * jolt + 12 * flutter * wave(t, 4, -0.05), 8 * flutter * wave(t, 4, -0.13), 6 * flutter * wave(t, 4, -0.2)))
    add(pose, "root", pos=(0.0, 0.0, 3.0 * jolt))
    add(pose, "thorax", rot=(18 * jolt, 0.0, 5 * decay * wave(t, 2)))
    legs(pose, 22 * jolt, 28 * jolt, 30 * jolt)
    abdomen(pose, 6 * jolt)
    antennae(pose, back=30 * jolt)
    glow(pose, 1.0 + 0.5 * decay * wave(t, 3))
    return pose


def death(t: float) -> Pose:
    """翅收拢坠落（2s 一次性）：0–0.35 翅向上收拢、残光乱闪；坠落中身体侧翻、腿蜷缩、腹下垂；落地后翅微松、触角垂落、残光熄灭。"""
    fold = ramp(t, 0.0, 0.35)
    fall = ramp(t, 0.1, 0.8) ** 1.5
    land = ramp(t, 0.85, 1.0)
    pose = Pose()
    wings(pose, (70 * fold - 24 * land, 22 * fold, 18 * fold), (62 * fold - 20 * land, 18 * fold, 14 * fold), sweep=24 * fold)
    add(pose, "root", pos=(0.0, -30.0 * fall, 0.0))
    add(pose, "thorax", rot=(-20 * ramp(t, 0.15, 0.8), 0.0, 80 * ramp(t, 0.2, 0.85)))
    curl = ramp(t, 0.3, 0.9)
    legs(pose, 35 * curl, 40 * curl, 45 * curl, curl=1.6, splay=-6 * curl)
    abdomen(pose, 10 * curl)
    antennae(pose, back=-40 * ramp(t, 0.4, 0.95))
    glow(pose, max(0.15, 1.0 + 0.4 * wave(t, 4) * (1 - ramp(t, 0.3, 0.7)) - 0.85 * ramp(t, 0.5, 0.95)))
    return pose


CLIPS: dict[str, tuple[float, bool, Sampler]] = {
    "idle": (4.0, True, idle),
    "fly": (0.5, True, fly),
    "attack": (0.9, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.0, False, death),
}


def grounded(rig: PoseRig, sampler: Sampler) -> Sampler:
    """不入地：动作只管姿态，不知道地面在哪。最低点掉到 0 以下的帧整体被顶起来，其余帧原样。"""

    def sample(t: float) -> Pose:
        pose = sampler(t)
        depth = rig.lowest(pose)
        if depth < 0.0:
            add(pose, "root", pos=(0.0, -depth, 0.0))
        return pose

    return sample


def check_loops() -> None:
    """循环段首尾姿态必须一致。"""
    for name, (_, loop, sampler) in CLIPS.items():
        if not loop:
            continue
        a, b = sampler(0.0), sampler(1.0)
        for bone in set(a) | set(b):
            for x, y in zip(a[bone].rot, b[bone].rot):
                assert abs((x - y + 180.0) % 360.0 - 180.0) < 0.01, f"{name}: {bone} 首尾旋转不同"
            assert max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)) < 0.01, f"{name}: {bone} 首尾位移不同"
            assert max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)) < 0.001, f"{name}: {bone} 首尾缩放不同"


def check_once_clips_return() -> None:
    """攻击 / 受击是一次性动作，要回到静止姿态，才能无缝接 idle / fly。"""
    for name in ("attack", "hurt"):
        end, start = CLIPS[name][2](1.0), CLIPS[name][2](0.0)
        for bone in set(end) | set(start):
            assert max(abs(x - y) for x, y in zip(end[bone].rot, start[bone].rot)) < 0.01, f"{name}: {bone} 末帧没回到起点"
            assert max(abs(x - y) for x, y in zip(end[bone].pos, start[bone].pos)) < 0.01, f"{name}: {bone} 末帧位移没回到起点"


def check_ground(rig: PoseRig) -> None:
    """整段动画最低点不得入地超过容差。"""
    for name, (_, _, sampler) in CLIPS.items():
        low = min(rig.lowest(grounded(rig, sampler)(i / SAMPLES)) for i in range(SAMPLES + 1))
        print(f"  {name}: 最低点 {low:+.2f}px")
        assert low > -GROUND_TOLERANCE, f"{name}: 入地 {low:.2f}px"


def self_test() -> None:
    """门禁自证：循环段注入首尾不一致、一次性段注入没回到起点，都必须被拦住。"""
    check_loops()
    check_once_clips_return()
    original_fly, original_attack = CLIPS["fly"], CLIPS["attack"]

    def broken_fly(t: float) -> Pose:
        pose = fly(t)
        add(pose, "thorax", rot=(10.0 * t, 0.0, 0.0))
        return pose

    def broken_attack(t: float) -> Pose:
        pose = attack(t)
        add(pose, "root", pos=(0.0, 0.0, -5.0 * t))
        return pose

    for name, replacement, gate in (("fly", (0.5, True, broken_fly), check_loops),
                                    ("attack", (0.9, False, broken_attack), check_once_clips_return)):
        CLIPS[name] = replacement
        try:
            gate()
        except AssertionError:
            print(f"  [OK] 注入「{name} 首尾不一致」被拦截")
            continue
        finally:
            CLIPS["fly"], CLIPS["attack"] = original_fly, original_attack
        raise RuntimeError(f"门禁失效：注入的 {name} 缺陷没被拦住")


def export() -> Path:
    self_test()
    rig = PoseRig(SOURCE)
    for name, (_, _, sampler) in CLIPS.items():
        unknown = [b for b in sampler(0.5) if b not in rig.bones]
        assert not unknown, f"{name}: 动画引用了不存在的骨 {unknown}"
    check_ground(rig)
    animations = []
    for name, (length, loop, sampler) in CLIPS.items():
        tracks = build_tracks(rig, grounded(rig, sampler), length, loop, SAMPLES)
        animations.append({"name": name, "length": length, "loop": loop, "tracks": tracks})
        keyframes = sum(len(v) for chans in tracks.values() for v in chans.values())
        print(f"  {name}: {length}s {'loop' if loop else 'once'}，{len(tracks)} 骨有轨道，{keyframes} 个关键帧")
    OUT.mkdir(parents=True, exist_ok=True)
    animated = OUT / "KuDengE_animated.bbmodel"
    write_animated_bbmodel(rig, animations, animated, MODEL_ID)
    subprocess.run(["node", str(CODEC), str(animated), str(OUT), MODEL_ID], check=True)
    return animated


def stage_assets(scratch: Path) -> Path:
    """把导出的三件套摆成 creature_anim_frames.py 认的目录（只在临时目录里，不碰 client 资源树）。"""
    doc = json.loads(SOURCE.read_text(encoding="utf-8"))
    png = base64.b64decode(doc["textures"][0]["source"].split(",", 1)[1])
    for folder, name, data in (("geo", f"{MODEL_ID}.geo.json", (OUT / f"{MODEL_ID}.geo.json").read_bytes()),
                               ("animations", f"{MODEL_ID}.animation.json", (OUT / f"{MODEL_ID}.animation.json").read_bytes()),
                               ("textures/entity/fauna", f"{MODEL_ID}.png", png)):
        target = scratch / folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    return scratch


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gif", type=Path, help="另出每段动画 GIF 到这个目录")
    parser.add_argument("--clip", action="append", help="只出这几段的 GIF")
    parser.add_argument("--scratch", type=Path, default=Path("/tmp/claude-1000/ku_deng_e_assets"))
    args = parser.parse_args()
    export()
    if args.gif:
        assets = stage_assets(args.scratch)
        command = [sys.executable, str(REPO / "modelScript" / "tools" / "creature_anim_frames.py"),
                   "--name", MODEL_ID, "--assets", str(assets), "--gif", "--size", "360", "--out", str(args.gif)]
        for clip in args.clip or ():
            command += ["--clip", clip]
        subprocess.run(command, check=True)


if __name__ == "__main__":
    main()
