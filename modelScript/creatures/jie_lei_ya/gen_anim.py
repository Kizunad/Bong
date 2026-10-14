#!/usr/bin/env python3
"""劫雷鸦动画：程序化采样 → 带动画的 bbmodel → 离线 codec → GeckoLib geo / animation.json（只落资产，不接线）。

六段动画（全部从零写关键帧）：

  idle    2.6s 循环  栖枝：双翼收拢贴身、呼吸起伏、歪头四顾、冠羽轻颤、尾尖点动、偶尔张喙
  walk    1.0s 循环  跳步：两腿交替前迈，身体前倾点头，头随步子前探后收，翼收着微颤
  fly     0.9s 循环  振翅飞行：翼臂 / 前臂 / 手逐级滞后拍动，翼尖划椭圆；腿收在腹下，尾扇微张，身体随拍翼起伏
  attack  1.3s 一次  俯冲啄：蹬地跃起展翼 → 收翼向前下俯冲（头朝下）→ 喙一张一合啄中 → 拉起展翼落回
  hurt    0.5s 一次  受击：身子后仰、双翼惊张、头甩、尾羽炸开
  death   2.4s 一次  抽搐 → 翅膀乱拍几下 → 侧身栽倒，双翼摊开软垂，腿蜷起，喙半张

坐标：建模稿朝 -Z（和游戏一致），骨旋转按 Blockbench 的 R = Rz·Ry·Rx 绕自身枢轴，子骨继承父骨变换。
符号约定（对着数值核过）：
  · 翼：右翼（+X）wing_arm rot.z 为正 = 翼尖上扬、左翼取反（下面统一写成 sign·角度，sign = +1 右 / -1 左）；
    rot.y 为负 = 右翼向后收（左翼取反）；折翼 = 臂向后、前臂回折向前、手再回折向后（Z 字叠）；
  · 脊 / 颈 / 头 rot.x 为正 = 前端抬起（仰头 / 抬头）；脊 rot.x 为正 = 机头上扬、尾下沉；
  · 腿：thigh rot.x 为正 = 腿向前摆；下喙 beak_low rot.x 为负 = 张口；
  · 尾羽：tail_r rot.y 为正 = 向 +X 外张，tail_l 取反。

用法::

    ~/.venvs/bong-modelscript/bin/python modelScript/creatures/jie_lei_ya/gen_anim.py
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

NAME = "jie_lei_ya"
SOURCE = HERE / "JieLeiYa_tex.bbmodel"
ANIMATED = HERE / "JieLeiYa_animated.bbmodel"
EXPORT_DIR = HERE / "export"
REVIEW_ASSETS = ROOT / "modelScript" / "out" / "creature_review" / NAME
SAMPLES = 24

SIDES = (("l", -1.0), ("r", 1.0))   # (骨名后缀, 符号)：右侧在 +X
Sampler = Callable[[float], Pose]

# 折翼姿势的三个角（°）：臂向后、前臂回折、手再回折
FOLD_ARM, FOLD_FORE, FOLD_HAND = 78.0, 150.0, 140.0
CRUISE_ALTITUDE = 24.0              # 飞行段整只抬离地面的高度（px）


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


def turn(pose: Pose, bone: str, rot=(0.0, 0.0, 0.0), pos=(0.0, 0.0, 0.0)) -> None:
    """往 pose 里累加一根骨的旋转 / 位移（同一根骨多处调用会叠加）。"""
    channel = pose[bone]
    channel.rot = [channel.rot[i] + rot[i] for i in range(3)]
    channel.pos = [channel.pos[i] + pos[i] for i in range(3)]


# ---------------------------------------------------------------- 翼 / 腿 / 尾 / 头
def wing(pose: Pose, fold: float, lift: float, fore_lift: float = 0.0, hand_lift: float = 0.0,
         sweep: float = 0.0, spread: float = 0.0) -> None:
    """双翼：fold = 0 平展、1 收拢贴身；lift / fore_lift / hand_lift = 臂 / 前臂 / 手上扬角（°，上扬为正）；
    sweep = 平展时整翼向后掠（°）；spread = 初级羽束的扇开角（°）。"""
    for side, s in SIDES:
        turn(pose, f"wing_arm_{side}", rot=(0, -s * (FOLD_ARM * fold + sweep), s * lift))
        turn(pose, f"wing_fore_{side}", rot=(0, s * FOLD_FORE * fold - s * sweep * 0.4, s * fore_lift))
        turn(pose, f"wing_hand_{side}", rot=(0, -s * FOLD_HAND * fold - s * sweep * 0.5, s * hand_lift))
        for g in range(4):
            turn(pose, f"prim_{g}_{side}", rot=(0, s * spread * (g - 1.5) * 0.5, 0))


def legs(pose: Pose, thigh: float, shin: float, tarsus: float, side: str | None = None) -> None:
    """腿三节的摆角（°）；side = None 两腿同值。thigh 为正 = 腿前摆。"""
    for sd, _ in SIDES:
        if side is None or side == sd:
            turn(pose, f"leg_thigh_{sd}", rot=(thigh, 0, 0))
            turn(pose, f"leg_shin_{sd}", rot=(shin, 0, 0))
            turn(pose, f"leg_tarsus_{sd}", rot=(tarsus, 0, 0))


def tuck_legs(pose: Pose, amount: float) -> None:
    """飞行时腿向后收在腹下。"""
    legs(pose, -38.0 * amount, 62.0 * amount, -30.0 * amount)
    for sd, _ in SIDES:
        turn(pose, f"toes_front_{sd}", rot=(-35.0 * amount, 0, 0))


def tail(pose: Pose, t: float, sway: float, fan: float = 0.0, cycles: float = 1.0, phase: float = 0.0, pitch: float = 0.0) -> None:
    """尾两节 + 三簇尾羽；fan = 尾羽扇开角（°）。"""
    turn(pose, "tail_0", rot=(pitch + 0.5 * sway * wave(t, cycles, phase), 0.4 * sway * wave(t, cycles, phase + 0.25), 0))
    turn(pose, "tail_1", rot=(pitch * 0.5 + 0.8 * sway * wave(t, cycles, phase - 0.08), 0.8 * sway * wave(t, cycles, phase + 0.2), 0))
    turn(pose, "tail_c", rot=(0.5 * sway * wave(t, cycles, phase - 0.15), 0, 0))
    turn(pose, "tail_l", rot=(0, -fan, 0))
    turn(pose, "tail_r", rot=(0, fan, 0))


def beak(pose: Pose, open_: float) -> None:
    """open_ ∈ [0,1]：上喙上扬 8°、下喙下压 22°（再多喙尖就穿过颈羽了）。"""
    turn(pose, "beak_up", rot=(8.0 * open_, 0, 0))
    turn(pose, "beak_low", rot=(-22.0 * open_, 0, 0))


def crest(pose: Pose, t: float, amount: float, cycles: float = 1.0) -> None:
    turn(pose, "crest", rot=(amount * wave(t, cycles, 0.2), 0, amount * 0.5 * wave(t, cycles, 0.45)))


def neck(pose: Pose, pitch: float, yaw: float = 0.0, roll: float = 0.0) -> None:
    """颈 + 头整体姿态：pitch/yaw/roll 按 1/3 : 1/3 : 1/3 分给 neck_0 / neck_1 / head。"""
    for bone in ("neck_0", "neck_1", "head"):
        turn(pose, bone, rot=(pitch / 3.0, yaw / 3.0, roll / 3.0))


# ---------------------------------------------------------------- 六段动画
def idle(t: float) -> Pose:
    pose = Pose()
    wing(pose, fold=0.94 + 0.03 * wave(t, 2, 0.1), lift=0.0)
    breathe = wave(t, 1, 0.0)
    turn(pose, "spine_0", rot=(1.2 * breathe, 0, 0))
    turn(pose, "root", pos=(0, 0.25 * breathe, 0))
    # 歪头四顾：一个周期里向左看、停、向右看、停（头 yaw + roll），颈跟着一起
    look = keys(t % 1.0, [(0.0, 0.0), (0.18, 1.0), (0.4, 1.0), (0.55, 0.0), (0.68, -1.0), (0.9, -1.0), (1.0, 0.0)])
    neck(pose, pitch=3.0 * wave(t, 1, 0.25), yaw=36.0 * look, roll=18.0 * look)
    beak(pose, max(0.0, wave(t, 1, 0.05)) ** 6 * 0.7)            # 偶尔张喙
    crest(pose, t, 6.0)
    tail(pose, t, 3.0, fan=2.0 * max(0.0, wave(t, 1, 0.6)))
    legs(pose, 4.0 * breathe, -3.0 * breathe, 0.0)
    return pose


def walk(t: float) -> Pose:
    pose = Pose()
    wing(pose, fold=0.9, lift=0.0)
    for side, s in SIDES:
        u = wave(t, 1, 0.0 if s > 0 else 0.5)
        lift = max(0.0, wave(t, 1, 0.25 + (0.0 if s > 0 else 0.5)))
        legs(pose, 30.0 * u, -36.0 * lift - 8.0 * u, 28.0 * lift, side=side)
        turn(pose, f"toes_front_{side}", rot=(-26.0 * lift, 0, 0))
    bob = abs(wave(t, 1, 0.0))                                   # 每迈一步（半周期）起伏一次
    turn(pose, "root", pos=(0, 0.7 * bob, 0))
    turn(pose, "spine_0", rot=(-6.0 + 2.5 * wave(t, 2, 0.1), 3.0 * wave(t, 1, 0.0), 2.0 * wave(t, 1, 0.5)))
    neck(pose, pitch=10.0 + 6.0 * wave(t, 2, 0.3), yaw=4.0 * wave(t, 1, 0.5))
    turn(pose, "neck_1", pos=(0, 0, -1.2 * wave(t, 2, 0.35)))     # 头随步子前探后收
    crest(pose, t, 5.0, cycles=2)
    tail(pose, t, 5.0, cycles=2, phase=0.1)
    return pose


def fly(t: float) -> Pose:
    pose = Pose()
    # 拍翼：臂主拍，前臂 / 手依次滞后（0.10 / 0.20 周期），翼尖划椭圆；下拍比上拍快（不对称）
    stroke = wave(t, 1, 0.0) + 0.18 * wave(t, 2, 0.0)
    arm = 30.0 * stroke
    fore = 22.0 * (wave(t, 1, -0.10) + 0.18 * wave(t, 2, -0.2))
    hand = 16.0 * (wave(t, 1, -0.20) + 0.18 * wave(t, 2, -0.4))
    down = max(0.0, -wave(t, 1, 0.0))                            # 下拍时初级羽并拢、上拍时扇开
    wing(pose, fold=0.0, lift=arm, fore_lift=fore, hand_lift=hand, sweep=6.0 * (1.0 - stroke) / 2, spread=18.0 * (1.0 - down))
    turn(pose, "root", pos=(0, CRUISE_ALTITUDE + 2.2 * -wave(t, 1, 0.0) * 0.5, 0))
    turn(pose, "spine_0", rot=(4.0 + 3.0 * wave(t, 1, 0.1), 0, 0))
    neck(pose, pitch=-4.0 - 3.0 * wave(t, 1, 0.1))                # 头基本不动，抵消身体起伏
    tuck_legs(pose, 1.0)
    tail(pose, t, 3.0, fan=10.0, pitch=4.0 * wave(t, 1, 0.15))
    crest(pose, t, 8.0, cycles=2)
    return pose


def attack(t: float) -> Pose:
    pose = Pose()
    # 节奏：0–0.2 蹬地跃起、双翼上扬；0.2–0.38 一记展翼拍；0.38–0.58 收翼俯冲（头朝下）；
    #       0.58–0.7 喙张开→合拢啄中；0.7–1 拉起、展翼落回栖姿
    climb = keys(t, [(0.0, 0.0), (0.2, 1.0), (0.38, 1.0), (0.5, 0.35), (0.62, 0.0), (1.0, 0.0)])      # 离地高度
    dive = keys(t, [(0.0, 0.0), (0.2, 0.0), (0.38, 0.1), (0.52, 1.0), (0.66, 0.85), (0.82, 0.0), (1.0, 0.0)])   # 俯冲程度
    spread = keys(t, [(0.0, 0.0), (0.12, 0.3), (0.22, 1.0), (0.38, 1.0), (0.5, 0.0), (0.66, 0.0), (0.78, 0.8), (0.9, 0.5), (1.0, 0.0)])
    flap = keys(t, [(0.0, 0.0), (0.12, -0.6), (0.24, 1.0), (0.38, -0.8), (0.5, 0.0), (0.7, 0.0), (0.8, 1.0), (0.92, -0.4), (1.0, 0.0)])
    stab = keys(t, [(0.0, 0.0), (0.56, 0.0), (0.6, 1.0), (0.64, 0.0), (1.0, 0.0)])       # 啄击冲量
    fold = (1.0 - spread) * (0.94 - 0.5 * dive)                  # 俯冲时翼半收成流线，栖姿时收紧
    wing(pose, fold=fold, lift=50.0 * flap * spread, fore_lift=36.0 * flap * spread, hand_lift=26.0 * flap * spread,
         sweep=40.0 * dive * (1.0 - fold), spread=-10.0 * dive)
    turn(pose, "root", pos=(0, 12.0 * climb - 6.0 * dive, -22.0 * dive - 4.0 * stab))
    turn(pose, "spine_0", rot=(-52.0 * dive + 12.0 * keys(t, [(0.0, 0.0), (0.2, 1.0), (0.4, 0.5), (0.6, 0.0), (1.0, 0.0)]), 0, 0))
    neck(pose, pitch=-18.0 * dive - 22.0 * stab + 8.0 * (1.0 - dive) * climb, yaw=0.0)
    turn(pose, "neck_1", pos=(0, 0, -2.4 * stab))
    beak(pose, keys(t, [(0.0, 0.0), (0.5, 0.0), (0.58, 1.0), (0.62, 1.0), (0.66, 0.0), (1.0, 0.0)]))
    tuck_legs(pose, 0.8 * climb * (1.0 - dive) + 0.25 * dive)
    legs(pose, 22.0 * dive, -10.0 * dive, 10.0 * dive)            # 俯冲末段脚爪前伸张开
    tail(pose, t, 2.0, fan=12.0 * dive + 8.0 * spread, pitch=-8.0 * dive)
    crest(pose, t, 6.0 * dive, cycles=2)
    return pose


def hurt(t: float) -> Pose:
    pose = Pose()
    kick = keys(t, [(0.0, 0.0), (0.14, 1.0), (0.5, 0.0), (1.0, 0.0)])          # 受击冲量：快进慢回
    wing(pose, fold=0.94 * (1.0 - kick), lift=30.0 * kick * wave(t, 3, 0.0), fore_lift=20.0 * kick, hand_lift=14.0 * kick,
         spread=22.0 * kick)
    turn(pose, "root", pos=(0, 2.5 * kick, 3.0 * kick))
    turn(pose, "spine_0", rot=(20.0 * kick, 5.0 * kick * wave(t, 2, 0.0), 0))
    neck(pose, pitch=14.0 * kick, yaw=-22.0 * kick, roll=10.0 * kick)
    beak(pose, 0.9 * kick)
    tail(pose, t, 6.0 * kick, fan=26.0 * kick, cycles=3, pitch=-10.0 * kick)
    crest(pose, t, 14.0 * kick, cycles=3)
    legs(pose, 12.0 * kick, -6.0 * kick, 0.0)
    return pose


def death(t: float) -> Pose:
    pose = Pose()
    twitch = keys(t, [(0.0, 0.0), (0.08, 1.0), (0.3, 0.6), (0.45, 0.0), (1.0, 0.0)])
    flail = keys(t, [(0.0, 0.0), (0.1, 1.0), (0.42, 0.7), (0.55, 0.0), (1.0, 0.0)])        # 乱拍翅
    fall = keys(t, [(0.3, 0.0), (0.62, 1.0)])                                              # 侧身栽倒
    limp = keys(t, [(0.45, 0.0), (0.8, 1.0)])                                              # 翅膀摊开软垂
    # 乱拍：翼先不规则上下扑几下，再慢慢摊平垂软
    flap = wave(t, 4, 0.0) * flail
    wing(pose, fold=0.94 * (1.0 - flail), lift=45.0 * flap - 20.0 * limp, fore_lift=26.0 * flap - 14.0 * limp,
         hand_lift=-18.0 * limp, spread=12.0 * limp)
    turn(pose, "root", rot=(0, 0, 84.0 * fall), pos=(0, 0, 0))
    turn(pose, "spine_0", rot=(14.0 * twitch, 4.0 * wave(t, 3, 0.0) * twitch, 0))
    neck(pose, pitch=-10.0 * fall + 12.0 * twitch, yaw=30.0 * fall, roll=14.0 * twitch)
    beak(pose, 0.7 * keys(t, [(0.0, 0.0), (0.1, 1.0), (0.6, 0.5), (1.0, 0.5)]))
    tail(pose, t, 4.0 * twitch, fan=30.0 * limp, cycles=3, pitch=14.0 * limp)
    legs(pose, -20.0 * fall, 52.0 * fall, -28.0 * fall)               # 蹬腿后蜷起
    crest(pose, t, 10.0 * twitch, cycles=3)
    return pose


CLIPS = {
    "idle": (2.6, True, idle),
    "walk": (1.0, True, walk),
    "fly": (0.9, True, fly),
    "attack": (1.3, False, attack),
    "hurt": (0.5, False, hurt),
    "death": (2.4, False, death),
}

# 地面约束：程序化动作不知道地面在哪，哪一帧有部位压进地面就把整只（root）托起到刚好贴地。
# 飞行 / 跃起的段落 root 本来就在空中，托起量只管「别入地」；侧身栽倒时机体绕脚底转，要托起 6~8 px。
GROUND_TOLERANCE = 0.6
MAX_LIFT = 14.0


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
    rot = max((max(abs(x - y) for x, y in zip(a[bone].rot, b[bone].rot)) for bone in set(a) | set(b)), default=0.0)
    pos = max((max(abs(x - y) for x, y in zip(a[bone].pos, b[bone].pos)) for bone in set(a) | set(b)), default=0.0)
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
