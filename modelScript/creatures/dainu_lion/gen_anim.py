#!/usr/bin/env python3
"""怠怒之狮 —— 动画生成：程序化步态 + 情绪动作，写回 bbmodel 关键帧。

原则和建模层一致：**逐层派生，别凭手感拧角度**。
  · 移动类（walk/run）只给「脚该踩在哪」，关节角全由 rig.solve_leg 逆解 → 支撑相
    脚锁死在世界坐标上，不滑步。躯干怎么起伏摇摆都不会带着脚一起飘。
  · 情绪类（roar/pounce/hurt/…）用姿态关键点插值，但**着地的脚照样逆解**——否则
    低头蓄力时脚会跟着头一起陷进地里。
  · 尾巴一律用行波（逐节相位延迟 + 振幅向尖端递增），不是整条一起摆。

角色设定：怠怒之狮。"怠"是常态——呼吸极缓、头低垂、尾尖偶尔一抽；"怒"是爆发——
咆哮/扑击要给出与怠态的强反差。所以 idle 刻意做得慢而少动，roar/pounce 刻意炸。

输出:
  modelScript/models/dainu_lion/DainuLionRig.bbmodel   带动画的模型（可直接拖进 Blockbench）
  modelScript/models/dainu_lion/dainu_lion.animation.json  GeckoLib 动画（参考/兜底用）

源模型 DainuLionPelt.bbmodel 只读不写——用户在 Blockbench 里的手改留在那份里。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import uuid
import zlib
from pathlib import Path

import numpy as np

from rig import PELT, Pose, Rig

OUT_BB = PELT.parent / "DainuLionRig.bbmodel"
OUT_JSON = PELT.parent / "dainu_lion.animation.json"
NAMESPACE = "bong"
MODEL_ID = "dainu_lion"

TAIL = tuple(f"tail_{i:02d}" for i in range(1, 11))
SPINE_FLEX = ("lumbar", "thorax_back", "thorax_front")  # 猫科奔跑靠脊柱屈伸供能
NECK = ("neck_base", "neck_mid", "neck_top")
LEGS = (("l", False), ("r", False), ("l", True), ("r", True))  # (side, hind)
TOEOFF_PITCH = 8.0   # 蹬离时脚掌翻起的角度；支撑相末尾与摆动相起点共用，保证交界连续


# ---------------------------------------------------------------- 曲线工具

def wrap(u: float) -> float:
    return u - math.floor(u)


def smooth(s: float) -> float:
    s = min(1.0, max(0.0, s))
    return s * s * (3.0 - 2.0 * s)


def ease_out(s: float, p: float = 2.0) -> float:
    return 1.0 - (1.0 - min(1.0, max(0.0, s))) ** p


def pulse(u: float, center: float, width: float) -> float:
    """环形高斯脉冲：用来做"大部分时间不动、某一刻抽一下"。"""
    d = abs(wrap(u - center + 0.5) - 0.5)
    return math.exp(-((d / width) ** 2))


def keyed(t: float, keys: list[tuple[float, float]]) -> float:
    """按 (时间, 值) 列表做平滑插值（段内 smoothstep，不会像线性那样出现折角）。"""
    if t <= keys[0][0]:
        return keys[0][1]
    for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * smooth((t - t0) / (t1 - t0)) if t1 > t0 else v1
    return keys[-1][1]


def jitter(name: str, i: int) -> float:
    """稳定扰动（crc32 不用内置 hash——后者每进程加盐，两次跑出的动画会不一样）。"""
    return (((zlib.crc32(f"{name}{i}".encode()) >> 3) & 1023) / 1023.0) * 2.0 - 1.0


# ---------------------------------------------------------------- 步态

class Gait:
    """四足步态：相位表 + 支撑相占空比 + 落脚窗口。

    落脚窗口 (fwd, back) 是相对静止落点的**非对称**区间：这匹狮子的静止姿前脚已经
    伸得很靠前（前掌在肩前 9.2 单位，肢长几乎用满），再往前伸就是几何不可达——量出来
    前伸 8 单位时残差 0.72、10 单位时 1.88（脚陷地一寸多）。所以往后多给、往前少给。
    """

    def __init__(self, rig: Rig, *, duty: float, phases: dict[str, float],
                 fwd: float, back: float, lift: float, hind_lift: float | None = None,
                 hind_fwd: float | None = None, swing_ease: float = 1.6, lift_shape: float = 1.0):
        self.rig = rig
        self.duty = duty
        self.phases = phases
        self.fwd, self.back = fwd, back
        # 前后肢**跨距必须相同**（每周期身体前进量对两者是同一个数），但窗口可以各自
        # 偏置：前肢静止落点已经很靠前、可达域主要在后方，后肢反过来。用同一套窗口时
        # 后肢在后端顶死限位，脚陷地 0.22、滑步残差 1.24。
        self.hind_fwd = fwd if hind_fwd is None else hind_fwd
        self.hind_back = (fwd + back) - self.hind_fwd
        self.lift = lift
        self.hind_lift = lift if hind_lift is None else hind_lift
        self.swing_ease = swing_ease
        self.lift_shape = lift_shape
        self.rest = rig.rest_stance()

    def key(self, side: str, hind: bool) -> str:
        return f"{'h' if hind else 'f'}{side}"

    def stance(self, side: str, hind: bool, t: float) -> bool:
        return wrap(t + self.phases[self.key(side, hind)]) < self.duty

    def target(self, side: str, hind: bool, t: float) -> tuple[np.ndarray, float]:
        """返回 (脚掌世界目标, 脚掌俯仰)。"""
        k = self.key(side, hind)
        r = self.rest[k].copy()
        u = wrap(t + self.phases[k])
        fwd, back = (self.hind_fwd, self.hind_back) if hind else (self.fwd, self.back)
        z_front, z_back = r[2] - fwd, r[2] + back
        half = self.rig.sole_half(self.rig.leg_chain(side, hind)[-1])
        if u < self.duty:                      # 支撑相：脚锁地，相对身体匀速后移
            s = u / self.duty
            z = z_front + (z_back - z_front) * s
            pitch = -TOEOFF_PITCH * smooth(max(0.0, (s - 0.65) / 0.35))   # 蹬离时掌根翻起
            # 逆解锁的是掌心，但蹬离真正的支点是掌尖：绕掌心翻 8° 会把掌尖压进地里
            # 0.3 单位。抬掌心 half·sin|θ| 就等价于绕掌尖翻。
            y = r[1] + half * abs(math.sin(math.radians(pitch)))
        else:                                   # 摆动相：抬起、前送、落下
            s = (u - self.duty) / (1.0 - self.duty)
            # 前送不能匀速。占空比 0.30 时摆动相占了七成周期，用 smoothstep 摊平的话
            # 腾空帧里后肢还拖在身后，"奔跑"读成"慢动作跨步"。真实的摆动是前段猛甩
            # 到位、后段悬着等落地，所以用 ease-out（run 更陡）。
            z = z_back + (z_front - z_back) * ease_out(s, self.swing_ease)
            lift = self.hind_lift if hind else self.lift
            # 起点必须接上支撑相末尾的 −8°（蹬离翻掌），落点归零准备平掌着地。
            # 早先写成 −16·sin(πs)^0.6，s=0 时是 0：脚掌俯仰在离地那一帧从 −8 硬跳回
            # 0、连带高度掉 0.5 单位，腕关节被逼着一帧折 36°，看着就是抬脚"啪"一下。
            pitch = -TOEOFF_PITCH * (1.0 - s) - 12.0 * math.sin(math.pi * s)
            y = r[1] + lift * math.sin(math.pi * s ** self.lift_shape) ** 0.75 + half * abs(math.sin(math.radians(pitch)))
        return np.array([r[0], y, z]), pitch

    def solve(self, pose: Pose, t: float) -> None:
        for side, hind in LEGS:
            tgt, pitch = self.target(side, hind, t)
            self.rig.solve_leg(pose, side, hind, tgt, foot_pitch=pitch)


# 注意相位的方向：支撑相条件是 wrap(t+phase) < duty，所以某腿的**落地时刻是
# 1−phase**，不是 phase。按 phase 当落地时刻排，猫科的侧对角序（后左→前左→后右→
# 前右）会排成对角序（后左→前右→后右→前左），走起来是马不是猫。
WALK = dict(duty=0.62, phases={"hl": 0.0, "fl": 0.75, "hr": 0.5, "fr": 0.25},
            fwd=6.0, back=10.0, hind_fwd=8.0, lift=3.4, hind_lift=2.9, swing_ease=1.5)
# 横向奔驰（transverse gallop）：后肢一前一后蹬地 → 前肢一前一后落地 → 伸展腾空。
# 相位/占空比要留出真正的腾空段：早先 duty=0.34 + 这组相位算下来支撑相首尾相接，
# 腾空只剩 5%，"奔跑"看着像快走，抬得再高也只是把脚从地里拔出来而已。
RUN = dict(duty=0.30, phases={"hl": 0.0, "hr": 0.10, "fl": 0.35, "fr": 0.45},
           fwd=5.5, back=7.5, hind_fwd=8.0, lift=5.4, hind_lift=4.8, swing_ease=3.0, lift_shape=0.7)
# 腾空窗口。按上面的相位方向算：支撑区间是 hr[0.90,0.20) hl[0,0.30) fr[0.55,0.85)
# fl[0.65,0.95)，四者并集之外只剩 [0.30,0.55)。早先照 phase 当落地时刻反推成
# 中点 0.875，正好把"躯干最高"排在前肢支撑相正中，前掌被架空 2.6 单位拔不下来。
RUN_FLIGHT = (0.30, 0.55)


def flight_arc(t: float) -> float:
    """腾空抛物弧：窗口两端为 0、中点为 1，窗口外为 0。

    躯干高度不能用正弦——正弦在落地瞬间仍处于高位，而前肢余量只有 4 单位，落地帧
    直接够不着地面（实测右前掌悬空 2.9 单位、滑步残差 1.34）。真实腾空是抛物线：
    离地和落地两个时刻高度都回到常态，峰值只在腾空正中。
    """
    f0, f1 = RUN_FLIGHT
    if not (f0 <= t <= f1):
        return 0.0
    s = (t - f0) / (f1 - f0)
    return 4.0 * s * (1.0 - s)


def tail_wave(pose: Pose, t: float, *, amp: float, freq: float, lag: float = 0.11,
              rise: float = 0.0, curl: float = 0.0, vert: float = 0.0) -> None:
    """尾行波：逐节延迟 + 振幅向尖端递增。整条同相摆是塑料尾巴。

    rise / curl 是**整条尾巴的总度数**，函数内部再分摊到 10 节。别写成逐节度数：
    单节 −22° 听着不多，10 节累加是 −220°，尾巴整条卷到背上去（咆哮全力帧实测总
    曲率 −264°，活像只高兴的家猫）。
      rise —— 整条抬起（正=上扬），均摊到每节
      curl —— 尖端额外弯曲（正=尖端上翘），按节权重分摊
    """
    n = len(TAIL)
    wsum = (n + 1) / 2                      # Σ w，w=(i+1)/n
    for i, b in enumerate(TAIL):
        w = (i + 1) / n
        ph = 2.0 * math.pi * (freq * t - lag * i)
        pose[b].rot[1] = amp * w ** 1.6 * math.sin(ph)
        pose[b].rot[0] = -rise / n - curl * w / wsum + vert * w ** 1.4 * math.sin(ph * 2.0 + 0.7)


def breathe(pose: Pose, t: float, *, rate: float, depth: float) -> None:
    """呼吸：胸廓抬合 + 腹部起伏。不用 scale——缩放会把挂在胸椎下的前肢一起拉长。"""
    a = math.sin(2.0 * math.pi * rate * t)
    b = math.sin(2.0 * math.pi * rate * t - 0.5)
    pose["thorax_back"].pos[1] += depth * 0.55 * a
    pose["thorax_front"].pos[1] += depth * 0.42 * b
    pose["thorax_back"].rot[0] += -depth * 0.9 * a
    pose["thorax_front"].rot[0] += depth * 0.7 * b
    pose["lumbar"].pos[1] += depth * 0.3 * a


def plant(rig: Rig, pose: Pose, ground: dict[str, np.ndarray], pitches: dict[str, float] | None = None) -> None:
    """躯干摆完之后，把仍在地上的脚逆解回它们的落点。"""
    for side, hind in LEGS:
        k = f"{'h' if hind else 'f'}{side}"
        if k in ground:
            rig.solve_leg(pose, side, hind, ground[k], foot_pitch=(pitches or {}).get(k, 0.0))


# ---------------------------------------------------------------- 各动画

def anim_idle(rig: Rig, t: float) -> Pose:
    """怠：呼吸极缓，头低垂微晃，尾尖偶尔抽一下。存在感靠"几乎不动"。

    循环动画里所有周期必须是整周数——早先用了 0.34/0.29/0.42 这类频率，t=1 处相位
    落不回 0，接缝逐骨最大差 5.6°，每轮循环肉眼可见地"咯噔"一下。
    """
    p = Pose()
    breathe(p, t, rate=2.0, depth=0.42)          # 7s 两次 ≈ 17 次/分，休息中的大猫
    p["hips"].rot[2] = 0.5 * math.sin(2.0 * math.pi * t)
    p["lumbar"].rot[1] = 0.8 * math.sin(2.0 * math.pi * t - 0.6)
    for i, b in enumerate(NECK):
        p[b].rot[0] += 1.4 + 0.7 * math.sin(2.0 * math.pi * 2.0 * t - 0.4 * i)
        p[b].rot[1] += 1.1 * math.sin(2.0 * math.pi * t - 0.5 * i)
    p["skull"].rot[0] += 2.0 + 1.1 * math.sin(2.0 * math.pi * 2.0 * t - 1.2)
    p["skull"].rot[1] += 2.4 * math.sin(2.0 * math.pi * t - 1.4)
    p["jaw"].rot[0] = 1.0 + 0.8 * math.sin(2.0 * math.pi * 2.0 * t)
    # 尾：底噪很小，0.62 处抽一下。只调振幅不调频率——频率随时间变化会毁掉相位闭合。
    flick = pulse(t, 0.62, 0.055)
    tail_wave(p, t, amp=3.0 + 22.0 * flick, freq=1.0, lag=0.13,
              rise=-6.0 + 26.0 * flick, curl=-12.0, vert=1.5 + 6.0 * flick)
    plant(rig, p, rig.rest_stance())
    return p


def anim_walk(rig: Rig, t: float) -> Pose:
    """慢步：侧对角序（后左→前左→后右→前右），躯干随之起伏侧摆。"""
    p = Pose()
    p["root"].pos[1] = 0.34 * math.sin(2.0 * math.pi * (2.0 * t + 0.12))
    p["hips"].rot[2] = 2.1 * math.sin(2.0 * math.pi * (t + 0.05))
    p["hips"].rot[0] = 0.9 * math.sin(2.0 * math.pi * (2.0 * t))
    p["lumbar"].rot[1] = 2.6 * math.sin(2.0 * math.pi * (t + 0.18))
    p["thorax_back"].rot[1] = 1.9 * math.sin(2.0 * math.pi * (t + 0.30))
    p["thorax_front"].rot[1] = -1.4 * math.sin(2.0 * math.pi * (t + 0.34))
    for i, b in enumerate(NECK):
        p[b].rot[0] = 1.2 + 1.5 * math.sin(2.0 * math.pi * (2.0 * t - 0.10 * i))
        p[b].rot[1] = -1.6 * math.sin(2.0 * math.pi * (t + 0.36 - 0.06 * i))
    p["skull"].rot[0] = 1.5 * math.sin(2.0 * math.pi * (2.0 * t - 0.34))
    p["skull"].rot[1] = -2.0 * math.sin(2.0 * math.pi * (t + 0.42))
    tail_wave(p, t, amp=7.5, freq=1.0, lag=0.12, rise=4.0, curl=-9.0, vert=2.5)
    breathe(p, t, rate=2.0, depth=0.16)
    Gait(rig, **WALK).solve(p, t)
    return p


def anim_run(rig: Rig, t: float) -> Pose:
    """疾驰：横向奔驰 + 脊柱屈伸。猫科的速度来自脊柱这张弓，不只是腿。"""
    p = Pose()
    mid = sum(RUN_FLIGHT) / 2
    arc = flight_arc(t)                                   # 只在腾空段抬升，落地即归零
    flex = math.cos(2.0 * math.pi * (t - mid))            # 脊柱：腾空最伸展，触地段收缩
    p["root"].pos[1] = 2.4 * arc - 0.35
    p["hips"].rot[0] = -4.0 * flex
    p["lumbar"].rot[0] = 7.0 * flex
    p["thorax_back"].rot[0] = 5.0 * math.cos(2.0 * math.pi * (t - mid + 0.06))
    p["thorax_front"].rot[0] = 3.0 * math.cos(2.0 * math.pi * (t - mid + 0.10))
    p["hips"].rot[2] = 1.4 * math.sin(2.0 * math.pi * t)
    for i, b in enumerate(NECK):
        p[b].rot[0] = -3.5 - 3.2 * math.cos(2.0 * math.pi * (t - mid + 0.14 - 0.05 * i))
    p["skull"].rot[0] = 2.0 + 2.4 * math.cos(2.0 * math.pi * (t - mid + 0.20))
    p["jaw"].rot[0] = 7.0 + 3.0 * math.sin(2.0 * math.pi * t)
    tail_wave(p, t, amp=13.0, freq=1.0, lag=0.06, rise=34.0, curl=6.0, vert=4.0)
    Gait(rig, **RUN).solve(p, t)
    return p


def anim_roar(rig: Rig, t: float) -> Pose:
    """咆哮：沉肩蓄力 → 抬头张颌爆发 → 余震 → 收。怒态的开场白。"""
    p = Pose()
    wind = keyed(t, [(0.0, 0.0), (0.22, 1.0), (0.34, 0.0)])       # 蓄力（低头缩颈）
    blast = keyed(t, [(0.28, 0.0), (0.40, 1.0), (0.78, 1.0), (1.0, 0.0)])
    tremor = math.sin(2.0 * math.pi * 11.0 * t) * blast * (1.0 - blast * 0.3)

    p["root"].pos[1] = -1.1 * wind + 0.9 * blast
    p["hips"].rot[0] = 3.0 * wind - 2.0 * blast
    p["lumbar"].rot[0] = 5.0 * wind - 6.0 * blast
    p["thorax_back"].rot[0] = 4.0 * wind - 7.5 * blast
    p["thorax_front"].rot[0] = 3.0 * wind - 6.0 * blast
    for i, b in enumerate(NECK):
        p[b].rot[0] = 11.0 * wind - 13.0 * blast + 0.5 * tremor
    p["skull"].rot[0] = 8.0 * wind - 22.0 * blast + 1.1 * tremor
    p["jaw"].rot[0] = 4.0 * wind + 46.0 * blast + 2.2 * tremor
    tail_wave(p, t, amp=6.0 + 16.0 * blast, freq=1.3, lag=0.1,
              rise=10.0 + 34.0 * blast, curl=8.0, vert=4.0 + 8.0 * blast)
    plant(rig, p, rig.rest_stance())
    return p


def anim_bite(rig: Rig, t: float) -> Pose:
    """扑咬：颈前送、颌张 → 合颌 → 拽回。"""
    p = Pose()
    lunge = keyed(t, [(0.0, 0.0), (0.10, -0.25), (0.40, 1.0), (0.62, 0.95), (1.0, 0.0)])
    jaw = keyed(t, [(0.0, 0.0), (0.34, 1.0), (0.46, 0.02), (0.60, 0.1), (1.0, 0.0)])
    p["root"].pos[2] = -2.2 * max(0.0, lunge)
    p["lumbar"].rot[0] = -3.0 * lunge
    p["thorax_back"].rot[0] = -4.5 * lunge
    p["thorax_front"].rot[0] = -5.0 * lunge
    for i, b in enumerate(NECK):
        p[b].rot[0] = -9.0 * lunge - 2.0 * i * lunge
    p["skull"].rot[0] = -6.0 * lunge
    p["jaw"].rot[0] = 52.0 * jaw
    tail_wave(p, t, amp=10.0, freq=1.4, lag=0.09, rise=18.0, curl=4.0, vert=5.0)
    plant(rig, p, rig.rest_stance())
    return p


def anim_swipe(rig: Rig, t: float) -> Pose:
    """左前爪横扫：重心右移 → 抬爪 → 横挥 → 落回。三条腿仍着地。"""
    p = Pose()
    load = keyed(t, [(0.0, 0.0), (0.20, 1.0), (0.42, 0.7), (0.75, 0.0)])
    sw = keyed(t, [(0.16, 0.0), (0.30, -0.35), (0.52, 1.0), (0.72, 0.85), (1.0, 0.0)])
    p["root"].pos[0] = 1.3 * load
    p["hips"].rot[2] = -3.5 * load
    p["lumbar"].rot[1] = 7.0 * sw
    p["thorax_back"].rot[1] = 11.0 * sw
    p["thorax_front"].rot[1] = 14.0 * sw
    p["thorax_front"].rot[2] = -6.0 * load
    for b in NECK:
        p[b].rot[1] = 6.0 * sw
        p[b].rot[0] = -2.0 * load
    p["skull"].rot[1] = 8.0 * sw
    p["skull"].rot[0] = -5.0 * load
    p["jaw"].rot[0] = 16.0 * max(0.0, sw)
    tail_wave(p, t, amp=16.0, freq=1.1, lag=0.1, rise=26.0, curl=6.0, vert=6.0)

    ground = {k: v for k, v in rig.rest_stance().items() if k != "fl"}
    plant(rig, p, ground)
    r = rig.rest_stance()["fl"]
    # 爪子的俯仰跟着抬起高度走。给恒定 −24° 时，爪还在地面高度就已经压着脚尖，
    # 起手帧整只前掌扎地下 0.9 单位。
    lift = math.sin(math.pi * min(1.0, max(0.0, (t - 0.16) / 0.72))) ** 0.6
    arc = np.array([r[0] - 5.5 * sw, r[1] + 7.5 * lift, r[2] - 3.0 * abs(sw)], float)
    rig.solve_leg(p, "l", False, arc, foot_pitch=-26.0 * lift)
    return p


def anim_pounce(rig: Rig, t: float) -> Pose:
    """扑击：压低蓄势 → 后肢蹬地腾空、前肢前伸 → 落地缓冲。"""
    p = Pose()
    crouch = keyed(t, [(0.0, 0.0), (0.26, 1.0), (0.38, 0.55), (0.44, 0.0)])
    fly = keyed(t, [(0.34, 0.0), (0.52, 1.0), (0.74, 1.0), (0.94, 0.0)])
    land = keyed(t, [(0.80, 0.0), (0.92, 1.0), (1.0, 0.35)])

    p["root"].pos[1] = -3.4 * crouch + 9.0 * fly - 1.6 * land
    p["root"].pos[2] = 1.6 * crouch - 5.0 * fly
    p["hips"].rot[0] = 6.0 * crouch - 9.0 * fly + 3.0 * land
    p["lumbar"].rot[0] = 9.0 * crouch - 14.0 * fly + 5.0 * land
    p["thorax_back"].rot[0] = 5.0 * crouch - 11.0 * fly + 4.0 * land
    p["thorax_front"].rot[0] = 3.0 * crouch - 8.0 * fly + 3.0 * land
    for i, b in enumerate(NECK):
        p[b].rot[0] = 7.0 * crouch - 6.0 * fly + 2.0 * land
    p["skull"].rot[0] = 4.0 * crouch - 3.0 * fly + 6.0 * land
    p["jaw"].rot[0] = 6.0 * crouch + 34.0 * fly
    tail_wave(p, t, amp=6.0, freq=1.0, lag=0.08,
              rise=8.0 + 42.0 * fly - 14.0 * crouch, curl=6.0, vert=4.0)

    rest = rig.rest_stance()
    if fly < 0.05:                      # 还在地上：蓄势时四脚不动
        plant(rig, p, rest)
    else:                               # 腾空：前肢前伸、后肢拖后
        for side, hind in LEGS:
            k = f"{'h' if hind else 'f'}{side}"
            r = rest[k].copy()
            if hind:
                tgt = r + np.array([0.0, 7.0 * fly, 6.5 * fly])
                pitch = 26.0 * fly
            else:
                tgt = r + np.array([0.0, 8.5 * fly, -4.0 * fly])
                pitch = -34.0 * fly
            rig.solve_leg(p, side, hind, tgt, foot_pitch=pitch)
    return p


def anim_hurt(rig: Rig, t: float) -> Pose:
    """受击：一记硬顿挫 + 递减抖动，不是平滑正弦。"""
    p = Pose()
    hit = keyed(t, [(0.0, 0.0), (0.09, 1.0), (0.30, 0.45), (1.0, 0.0)])
    shake = math.sin(2.0 * math.pi * 9.0 * t) * math.exp(-4.5 * t)
    p["root"].pos[1] = -1.5 * hit
    p["root"].pos[2] = 1.7 * hit
    p["hips"].rot[0] = 5.0 * hit
    p["lumbar"].rot[0] = 8.0 * hit + 1.6 * shake
    p["thorax_back"].rot[0] = 7.0 * hit + 1.2 * shake
    p["thorax_front"].rot[2] = 5.0 * hit
    for i, b in enumerate(NECK):
        p[b].rot[0] = 6.0 * hit + 2.0 * shake
        p[b].rot[2] = 3.0 * hit
    p["skull"].rot[0] = -9.0 * hit + 3.0 * shake
    p["skull"].rot[2] = 6.0 * hit
    p["jaw"].rot[0] = 26.0 * hit
    tail_wave(p, t, amp=14.0 * hit + 4.0, freq=2.2, lag=0.08, rise=6.0 + 14.0 * hit, curl=2.0, vert=8.0 * hit)
    plant(rig, p, rig.rest_stance())
    return p


def anim_death(rig: Rig, t: float) -> Pose:
    """倒地：前肢先塌 → 躯干沉 → 侧翻 → 头最后落下，尾巴余一下。

    侧翻绕的是 root（地面高度的轴），身体横跨 x=0，所以翻过去时**背离侧会转到地面
    以下**——必须按帧把最低点夹回地面，而不是拍一个下沉量。早先写死 −13 单位下沉，
    实测颈根扎到 −19.3（约 1.2 米深），整只埋进土里。
    """
    p = Pose()
    buckle = keyed(t, [(0.0, 0.0), (0.30, 1.0)])
    sink = keyed(t, [(0.18, 0.0), (0.62, 1.0)])
    roll = keyed(t, [(0.40, 0.0), (0.80, 1.0)])
    headfall = keyed(t, [(0.52, 0.0), (0.88, 1.0)])

    p["root"].rot[2] = 74.0 * roll
    p["root"].rot[0] = -4.0 * sink
    p["hips"].rot[0] = 6.0 * sink
    p["lumbar"].rot[0] = -7.0 * buckle + 4.0 * sink
    p["thorax_back"].rot[0] = -9.0 * buckle
    p["thorax_front"].rot[0] = -6.0 * buckle
    for i, b in enumerate(NECK):
        p[b].rot[0] = 6.0 * buckle + 9.0 * headfall
        p[b].rot[2] = -4.0 * roll
    p["skull"].rot[0] = 4.0 * buckle + 16.0 * headfall
    p["skull"].rot[2] = -8.0 * roll
    p["jaw"].rot[0] = 14.0 * buckle + 6.0 * (1.0 - headfall)
    tail_wave(p, t, amp=10.0 * (1.0 - sink) + 2.0, freq=0.9, lag=0.12,
              rise=12.0 * (1.0 - sink) - 8.0 * sink, curl=-6.0 * roll, vert=4.0 * (1.0 - sink))

    rest = rig.rest_stance()
    for side, hind in LEGS:
        k = f"{'h' if hind else 'f'}{side}"
        r = rest[k].copy()
        fold = buckle if not hind else sink
        tgt = r + np.array([0.0, 0.0, (4.5 if hind else -3.0) * fold])
        rig.solve_leg(p, side, hind, tgt, foot_pitch=-18.0 * fold)

    # 贴地夹持：翻转过程中把整只抬回地面，别让身体转到地下去
    p["root"].pos[1] -= rig.lowest(p)
    return p


# name → (时长秒, 是否循环, 采样数, 生成函数)
ANIMS = {
    "idle":   (7.0,  True,  36, anim_idle),
    "walk":   (1.25, True,  28, anim_walk),
    "run":    (0.62, True,  26, anim_run),
    "roar":   (2.40, False, 34, anim_roar),
    "bite":   (0.85, False, 24, anim_bite),
    "swipe":  (0.90, False, 24, anim_swipe),
    "pounce": (1.15, False, 30, anim_pounce),
    "hurt":   (0.55, False, 20, anim_hurt),
    "death":  (2.60, False, 30, anim_death),
}


def sample(rig: Rig, name: str, t01: float) -> Pose:
    return ANIMS[name][3](rig, t01)


# ---------------------------------------------------------------- 导出

def _uuid(seed: str) -> str:
    """确定性 v4 uuid。

    早先用 `uuid.UUID(int=crc32(seed) | 1<<100)` 拼，出来的是
    00000010-0000-0000-0000-0000xxxxxxxx 这种非法 v4：熵只有 32 位（9500 个关键帧
    撞车概率约 1%），版本/变体位也不对。Blockbench 拿 uuid 当索引键，不值得冒这个险。
    """
    return str(uuid.UUID(bytes=hashlib.md5(seed.encode()).digest(), version=4))


def _kf(channel: str, time: float, vec, idx: int, aname: str) -> dict:
    """关键帧。字段照 modelScript/models/JianPlayerAnim.bbmodel（用户手上能正常打开的
    带动画工程）逐项对齐：data_points 用**字符串**、bezier 四件套即使走 linear 也写
    全。Blockbench 读盘时这些字段有默认值与否要看版本，缺字段是没必要担的风险。"""
    return {
        "channel": channel,
        "data_points": [{"x": f"{vec[0]:.4f}", "y": f"{vec[1]:.4f}", "z": f"{vec[2]:.4f}"}],
        "uuid": _uuid(f"{aname}{channel}{idx}"),
        "time": round(time, 4),
        "color": -1,
        "interpolation": "linear",
        "bezier_linked": True,
        "bezier_left_time": [-0.1, -0.1, -0.1],
        "bezier_left_value": [0, 0, 0],
        "bezier_right_time": [0.1, 0.1, 0.1],
        "bezier_right_value": [0, 0, 0],
    }


def build_tracks(rig: Rig, name: str) -> tuple[float, bool, dict[str, dict[str, list]]]:
    """采样 → 每骨每通道的 (时间, 三元组) 序列。恒定通道直接丢掉。"""
    length, loop, n, _ = ANIMS[name]
    frames = []
    for i in range(n + 1):
        t = i / n
        if loop and i == n:
            frames.append((length, frames[0][1]))     # 循环末帧 = 首帧，接缝为零
            break
        frames.append((t * length, sample(rig, name, t)))

    tracks: dict[str, dict[str, list]] = {}
    for bone in rig.order:
        for chan, attr, default in (("rotation", "rot", 0.0), ("position", "pos", 0.0), ("scale", "scale", 1.0)):
            vals = [(tt, list(getattr(pz[bone], attr)) if bone in pz else [default] * 3) for tt, pz in frames]
            if all(abs(v[k] - default) < 1e-4 for _, v in vals for k in range(3)):
                continue
            tracks.setdefault(bone, {})[chan] = vals
    return length, loop, tracks


def write_bbmodel(rig: Rig, names: list[str], out: Path) -> None:
    """把动画塞进 bbmodel 的 animations（Blockbench 可直接打开、GeckoLib codec 可导出）。"""
    doc = json.loads(PELT.read_text())
    uuids = {g["name"]: g["uuid"] for g in doc.get("groups", [])}
    anims = []
    for name in names:
        length, loop, tracks = build_tracks(rig, name)
        animators = {}
        for bone, chans in tracks.items():
            kfs = []
            for chan, vals in chans.items():
                for i, (tt, v) in enumerate(vals):
                    kfs.append(_kf(chan, tt, v, i, f"{name}{bone}{chan}"))
            animators[uuids[bone]] = {"name": bone, "type": "bone", "keyframes": kfs}
        anims.append({
            "uuid": _uuid(f"anim:{name}"),
            "name": name,
            "loop": "loop" if loop else "once",
            "override": False,
            "length": round(length, 4),
            "snapping": 24,
            "selected": False,
            "saved": True,
            "path": "",
            "anim_time_update": "",
            "blend_weight": "",
            "start_delay": "",
            "loop_delay": "",
            "animators": animators,
        })
    doc["animations"] = anims
    doc["name"] = "DainuLionRig"
    out.write_text(json.dumps(doc, ensure_ascii=False))


def write_geckolib(rig: Rig, names: list[str], out: Path) -> None:
    """直出 GeckoLib animation.json —— **参考用，未经引擎侧验证，不要直接当资产提交**。

    Bedrock 动画的旋转符号约定和 Blockbench 面板显示是否一致（X/Y 是否取反），我没
    有可对拍的实例：仓库里现有的 geo.json 骨骼旋转全是 0，modelScript/models 里也没有
    bbmodel ↔ animation.json 的同源对。所以正经路径是把 DainuLionRig.bbmodel 交给
    bbmodel_maker.workbench.bbmodel_to_geckolib（驱动 Blockbench 官方 codec 导出），由
    codec 负责这层约定；本函数只用于人眼查看曲线和兜底。
    """
    animations = {}
    for name in names:
        length, loop, tracks = build_tracks(rig, name)
        bones = {}
        for bone, chans in tracks.items():
            entry = {}
            for chan, vals in chans.items():
                entry[chan] = {str(round(tt, 4)): [round(v[0], 4), round(v[1], 4), round(v[2], 4)]
                               for tt, v in vals}
            bones[bone] = entry
        animations[f"animation.{NAMESPACE}.{MODEL_ID}.{name}"] = {
            "loop": bool(loop),
            "animation_length": round(length, 4),
            "bones": bones,
        }
    out.write_text(json.dumps({"format_version": "1.8.0", "animations": animations},
                              indent="\t", ensure_ascii=False))


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮动画生成")
    ap.add_argument("--only", nargs="*", help="只生成这些动画")
    args = ap.parse_args()
    names = args.only or list(ANIMS)
    rig = Rig()
    write_bbmodel(rig, names, OUT_BB)
    write_geckolib(rig, names, OUT_JSON)
    total = 0
    for name in names:
        length, loop, tracks = build_tracks(rig, name)
        kf = sum(len(v) for c in tracks.values() for v in c.values())
        total += kf
        print(f"  {name:<7} {length:4.2f}s {'循环' if loop else '单次'}  骨 {len(tracks):2d}  关键帧 {kf}")
    print(f"→ {OUT_BB.name} / {OUT_JSON.name}  共 {total} 关键帧")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
