#!/usr/bin/env python3
"""诡影（gui_ying）动画：idle / drift / attack / hurt / death 五段，关键帧从零写。

骨架来自 ``GuiYing_tex.bbmodel``（gen_gui_ying.py --tex 产出）：funnel 总枢轴 + 6 圈环 + 喉管 + 空洞 + 10 组碎片。
约定（见 gen_gui_ying.py 文档）：**整体旋转只由 funnel 承担**，各环只摆几度；碎屑组可各自错开相位、被吸入或炸开。

漏斗开口朝前上方 38°，所以「绕漏斗轴自转」不是某一个欧拉轴，而是绕斜轴 W 的旋转；
这里用 Rodrigues 公式构造旋转矩阵再解回 Blockbench 的 Rz·Ry·Rx 欧拉角（animkit.euler_of）。
循环动画的总转角必须是 360° 的整数倍，首尾姿态才会一致。

产物（不装进 client 资源树）：
    modelScript/out/gui_ying/GuiYing_animated.bbmodel     带动画的作者稿
    modelScript/out/gui_ying/gui_ying.{geo,animation}.json 经离线 codec 导出的 GeckoLib 文件
    <scratch>/assets/{geo,animations,textures/entity/fauna}/  给 creature_anim_frames.py 用的临时资源根

用法：
    python gen_anim.py                       # 导出 + 校验
    python gen_anim.py --gif <审阅目录>       # 另出每段 GIF
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

import numpy as np
from bbmodel_maker.rig.animkit import Pose, PoseRig, build_tracks, euler_of, smooth, write_animated_bbmodel

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).parent))
import gen_gui_ying as model  # noqa: E402

MODEL_ID = model.MODEL_ID
SOURCE = model.MODEL_DIR / "GuiYing_tex.bbmodel"
OUT = REPO / "modelScript" / "out" / MODEL_ID
CODEC = REPO / "modelScript" / "exporters" / "creature_codec.cjs"
SAMPLES = 48

Sampler = Callable[[float], Pose]

RINGS = [f"ring_{i}" for i in range(model.LAYER_COUNT)]
DEBRIS = [f"debris_{g:02d}" for g in range(model.DEBRIS_GROUPS)]
EPS_SCALE = 0.001                      # death 末帧的「消散」：缩到近零（0 会让 codec 的缩放矩阵退化）


# ---------------------------------------------------------------- 曲线工具
def wave(t: float, cycles: float = 1.0, phase: float = 0.0) -> float:
    """t ∈ [0,1] 上走 cycles 个整周期的正弦；循环动画首尾自然相接。"""
    return math.sin(math.tau * (cycles * t + phase))


def ramp(t: float, t0: float, t1: float) -> float:
    """t0→t1 之间 0→1 的 smoothstep。"""
    return smooth(min(1.0, max(0.0, (t - t0) / (t1 - t0))))


def spin_euler(degrees: float) -> list[float]:
    """绕漏斗轴 W 转 degrees 度，换成 (x, y, z) 欧拉角（度）。"""
    axis = model.W_HAT / np.linalg.norm(model.W_HAT)
    a = math.radians(degrees)
    k = np.array([[0, -axis[2], axis[1]], [axis[2], 0, -axis[0]], [-axis[1], axis[0], 0]])
    rot = np.eye(3) + math.sin(a) * k + (1 - math.cos(a)) * (k @ k)
    return [float(v) for v in euler_of(rot)]


def debris_direction(g: int) -> np.ndarray:
    """第 g 组碎屑在漏斗局部系里的径向方向（和 debris_group_cells 里的螺旋角一致）。"""
    t = (g + 0.5) / model.DEBRIS_GROUPS
    theta = 5.2 * math.pi * t
    return np.array([math.cos(theta), math.sin(theta), 0.0])


def set_scale(pose: Pose, bone: str, s: float) -> None:
    pose[bone].scale = [s, s, s]


def spin(pose: Pose, degrees: float) -> None:
    pose["funnel"].rot = spin_euler(degrees)


# ---------------------------------------------------------------- 五段动作
def idle(t: float) -> Pose:
    """漏斗慢旋（8s 一整圈）；各环绕 x/z 做几度错相摆动；空洞轻微呼吸；碎屑沿轴缓缓起伏。6s 内转 270° 不闭合，所以 idle 取 8s。"""
    pose = Pose()
    spin(pose, 360.0 * t)
    for i, ring in enumerate(RINGS):
        pose[ring].rot = [3.0 * wave(t, 1, i / 6), 0.0, 3.0 * wave(t, 1, i / 6 + 0.25)]
    set_scale(pose, "void", 1.0 + 0.05 * wave(t, 2))
    for g, bone in enumerate(DEBRIS):
        pose[bone].pos = [0.0, 0.6 * wave(t, 2, g / 10), 0.0]
        pose[bone].rot = [0.0, 0.0, 4.0 * wave(t, 1, g / 10)]
    return pose


def drift(t: float) -> Pose:
    """移动：整体前倾并左右蛇行，自转加快（2s 一整圈），环朝前进方向拖出相位差。"""
    pose = Pose()
    spin(pose, 360.0 * t)
    pose["root"].rot = [10.0, 0.0, 6.0 * wave(t, 1)]
    pose["root"].pos = [2.0 * wave(t, 1), 0.8 * wave(t, 2), 0.0]
    for i, ring in enumerate(RINGS):
        pose[ring].rot = [5.0 * wave(t, 2, i / 8), 0.0, 5.0 * wave(t, 2, i / 8 + 0.25)]
    for g, bone in enumerate(DEBRIS):
        pose[bone].pos = [0.0, 0.0, -1.5 * wave(t, 2, g / 10)]
    return pose


def attack(t: float) -> Pose:
    """猛吸（1s，一次性）：0–0.35 蓄势——环向中心收缩、碎屑被吸向喉管、自转加速；0.35–0.55 吸满停顿；0.55–1 缓缓松开。
    总转角 720°，首尾同姿态。"""
    pull = ramp(t, 0.0, 0.35) - ramp(t, 0.55, 1.0)            # 0 → 1 → 0
    pose = Pose()
    spin(pose, 720.0 * smooth(t))
    for i, ring in enumerate(RINGS):
        lag = 0.04 * i                                         # 外圈先收，内圈稍后
        local = ramp(t, lag, 0.35 + lag * 0.5) - ramp(t, 0.55, 1.0)
        set_scale(pose, ring, 1.0 - 0.45 * max(0.0, local))
    for g, bone in enumerate(DEBRIS):
        set_scale(pose, bone, 1.0 - 0.7 * pull)
        pose[bone].pos = [0.0, 0.0, -5.0 * pull]
    set_scale(pose, "throat", 1.0 + 0.25 * pull)
    set_scale(pose, "void", 1.0 + 0.5 * pull)
    pose["root"].pos = [0.0, 0.0, 1.5 * pull]
    return pose


def hurt(t: float) -> Pose:
    """受击（0.5s）：整体被打得后仰抖动并衰减，环瞬间胀开又回弹，碎屑被震得外飘。自转保持 0 起点 0 终点（只叠加抖动）。"""
    decay = 1.0 - ramp(t, 0.0, 1.0)
    jolt = math.sin(math.pi * min(1.0, t * 4.0)) if t < 0.25 else 0.0
    pose = Pose()
    spin(pose, 30.0 * wave(t, 3) * decay)
    pose["root"].rot = [-8.0 * jolt, 0.0, 0.0]
    pose["root"].pos = [0.0, 0.0, 2.5 * jolt]
    for i, ring in enumerate(RINGS):
        set_scale(pose, ring, 1.0 + 0.10 * jolt * (1 - i / 8))
        pose[ring].rot = [4.0 * wave(t, 2, i / 6) * decay, 0.0, 0.0]
    for g, bone in enumerate(DEBRIS):
        d = debris_direction(g)
        pose[bone].pos = [float(d[0]) * 2.0 * jolt, float(d[1]) * 2.0 * jolt, 0.0]
    return pose


def death(t: float) -> Pose:
    """塌缩消散（2s，一次性）：自转越来越快，环自外而内依次缩进喉管，空洞先张大再消失；碎屑朝各自径向炸开并缩小；整体末帧近零。"""
    pose = Pose()
    spin(pose, 1080.0 * t * t)
    for i, ring in enumerate(RINGS):
        start = 0.10 + 0.10 * i
        s = 1.0 - ramp(t, start, start + 0.35)
        set_scale(pose, ring, max(EPS_SCALE, s))
    for g, bone in enumerate(DEBRIS):
        d = debris_direction(g)
        fly = ramp(t, 0.05, 0.9)
        set_scale(pose, bone, max(EPS_SCALE, 1.0 - ramp(t, 0.40 + 0.02 * g, 0.78 + 0.02 * g)))
        pose[bone].pos = [float(d[0]) * 26.0 * fly, float(d[1]) * 26.0 * fly, 8.0 * fly]
    set_scale(pose, "throat", max(EPS_SCALE, 1.0 - ramp(t, 0.55, 0.95)))
    set_scale(pose, "void", max(EPS_SCALE, 1.0 + 0.8 * ramp(t, 0.0, 0.5) - 1.8 * ramp(t, 0.6, 1.0)))
    set_scale(pose, "funnel", max(EPS_SCALE, 1.0 - 0.9 * ramp(t, 0.8, 1.0)))
    return pose


CLIPS: dict[str, tuple[float, bool, Sampler]] = {
    "idle": (8.0, True, idle),
    "drift": (2.0, True, drift),
    "attack": (1.0, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.0, False, death),
}


def check_loops() -> None:
    """循环段首尾姿态必须一致（旋转差按 360° 取模）。"""
    for name, (_, loop, sampler) in CLIPS.items():
        if not loop:
            continue
        a, b = sampler(0.0), sampler(1.0)
        for bone in set(a) | set(b):
            for x, y in zip(a[bone].rot, b[bone].rot):
                diff = abs((x - y + 180.0) % 360.0 - 180.0)
                assert diff < 0.01, f"{name}: {bone} 首尾旋转差 {diff:.3f}°，循环接不上"
            assert max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)) < 0.01, f"{name}: {bone} 首尾位移不同"
            assert max(abs(x - y) for x, y in zip(a[bone].scale, b[bone].scale)) < 0.001, f"{name}: {bone} 首尾缩放不同"


def check_loop_gate_catches_defect() -> None:
    """门禁自证：把 idle 的总转角改成 300°，首尾对不上必须被拦住。"""
    original = CLIPS["idle"]

    def broken(t: float) -> Pose:
        pose = idle(t)
        spin(pose, 300.0 * t)
        return pose

    CLIPS["idle"] = (8.0, True, broken)
    try:
        check_loops()
    except AssertionError:
        return
    finally:
        CLIPS["idle"] = original
    raise RuntimeError("门禁失效：自转 300° 的 idle 没被循环门禁拦住")


def export() -> Path:
    check_loops()
    check_loop_gate_catches_defect()
    rig = PoseRig(SOURCE)
    for name, (_, _, sampler) in CLIPS.items():
        unknown = [b for b in sampler(0.5) if b not in rig.bones]
        assert not unknown, f"{name}: 动画引用了不存在的骨 {unknown}"
    animations = []
    for name, (length, loop, sampler) in CLIPS.items():
        tracks = build_tracks(rig, sampler, length, loop, SAMPLES)
        animations.append({"name": name, "length": length, "loop": loop, "tracks": tracks})
        keyframes = sum(len(v) for chans in tracks.values() for v in chans.values())
        print(f"  {name}: {length}s {'loop' if loop else 'once'}，{len(tracks)} 骨有轨道，{keyframes} 个关键帧")
    OUT.mkdir(parents=True, exist_ok=True)
    animated = OUT / "GuiYing_animated.bbmodel"
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
    parser.add_argument("--scratch", type=Path, default=OUT / "gif_assets")
    args = parser.parse_args()
    export()
    if args.gif:
        assets = stage_assets(args.scratch)
        subprocess.run([sys.executable, str(REPO / "modelScript" / "tools" / "creature_anim_frames.py"),
                        "--name", MODEL_ID, "--assets", str(assets), "--gif", "--facing", "+z", "--size", "360",
                        "--out", str(args.gif)], check=True)


if __name__ == "__main__":
    main()
