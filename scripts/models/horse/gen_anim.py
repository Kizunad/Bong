#!/usr/bin/env python3
"""马 —— 动画生成：程序化步态 + 行为动作，写回那 9 份皮层 bbmodel 的 animations。

原则和建模层一致：**逐层派生，别凭手感拧角度**。
  · 移动类（walk/trot/canter/gallop）只给「蹄该踩在哪」，关节角全由 rig.solve_leg
    逆解 → 支撑相蹄锁死在世界坐标上，不滑步。躯干怎么起伏摇摆都不会带着蹄一起飘。
  · 行为类（graze/rear/kick/hurt/death）用姿态关键点插值，但**着地的蹄照样逆解**。
  · 尾巴一律用行波（逐节相位延迟 + 振幅向尖端递增），不是整条一起摆。

马与猫科在动画上的分野，四条都落进了参数：
  ① **四种步态**而不是两种。走（四拍侧序）· 快步（对角二拍）· 跑步（三拍）·
     袭步（四拍横向奔驰）。四者的节拍表是马这一物种的身份证。
  ② **落地次序由落地时刻表推**（`phases_from_touchdown`），不手写相位。支撑判据是
     `wrap(t+phase) < duty`，落地时刻是 `1−phase`；直接把 phase 当落地时刻写，
     四足的次序会整个错乱（对角序写成侧序，快步就走成了走）。
  ③ **腾空窗口程序化求**（`flight_window`），不靠人肉推区间。
  ④ **走有头颈点动、快步没有**——马靠颈的前后摆平衡走步，快步是对角支撑天然平衡，
     头几乎不动。这一条是"走"和"快步"在视觉上最好认的差别。

源模型是 local_models/horse/HorsePelt_<coat>_<size>.bbmodel（9 份）。几何与骨骼
pivot 只随体型变，不随毛色变，所以同一体型的动画轨道**三种毛色通用**：按体型算一次，
写进该体型的三份里。

输出:
  local_models/horse/HorsePelt_*.bbmodel        原地追加 animations（交付物）
  local_models/horse/stages/horse_<size>.animation.json  GeckoLib 参考导出
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

from gen_pelt import COATS
from gen_skeleton import PROFILES
from rig import FINAL, Pose, Rig, contact_report

STAGES = FINAL / "stages"
NAMESPACE = "bong"
MODEL_ID = "horse"

TAIL = tuple(f"tail_{i:02d}" for i in range(1, 9))
NECK = ("neck_base", "neck_mid", "neck_top")
LEGS = (("l", False), ("r", False), ("l", True), ("r", True))
TOEOFF_PITCH = 10.0  # 蹬离时蹄翻起的角度；支撑相末尾与摆动相起点共用，保证交界连续


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
def phases_from_touchdown(td: dict[str, float]) -> dict[str, float]:
    """落地时刻表 → 相位表。

    支撑判据是 `wrap(t+phase) < duty`，所以某腿的**落地时刻是 1−phase**。把落地时刻
    直接当 phase 写，四足次序会整个反过来（马的对角序会排成侧序）。这里让代码替人做
    这次换算，别再靠注释提醒。
    """
    return {k: (1.0 - v) % 1.0 for k, v in td.items()}


def flight_window(duty: float, phases: dict[str, float], n: int = 400) -> tuple[float, float] | None:
    """求腾空窗口（四蹄皆离地的那一段），返回最长的一段 (t0, t1)；没有则 None。

    人肉推四条支撑区间的并集补集很容易错一位，而这个窗口直接决定躯干抛物弧的相位——
    错了就会把"躯干最高"排在支撑相正中，蹄被架空拔不下来。交给程序算。
    """
    on = []
    for i in range(n):
        t = i / n
        on.append(any(wrap(t + p) < duty for p in phases.values()))
    if all(on):
        return None
    best = (0, 0)
    i = 0
    # 环形扫描：从任一非腾空点起，找最长的连续 False 段
    start = next(k for k in range(n) if on[k])
    runs = []
    cur = None
    for k in range(n + 1):
        idx = (start + k) % n
        if not on[idx]:
            cur = k if cur is None else cur
        elif cur is not None:
            runs.append((cur, k))
            cur = None
    if cur is not None:
        runs.append((cur, n))
    if not runs:
        return None
    best = max(runs, key=lambda r: r[1] - r[0])
    t0 = ((start + best[0]) % n) / n
    t1 = ((start + best[1]) % n) / n
    return (t0, t1 if t1 > t0 else t1 + 1.0)


_SPAN_CACHE: dict[int, dict[bool, tuple[float, float]]] = {}


def leg_spans(rig: Rig) -> dict[bool, tuple[float, float]]:
    """量出前 / 后肢在地面上的可达域（缓存，一个 rig 只量一次）。"""
    key = id(rig)
    if key not in _SPAN_CACHE:
        _SPAN_CACHE[key] = {h: rig.reach_span("l", h) for h in (False, True)}
    return _SPAN_CACHE[key]


class Gait:
    """四足步态：落地时刻表 + 支撑相占空比 + 落蹄窗口。

    落蹄窗口**不写死数字，而是由实测可达域反推**。量出来的事实是：
      前肢 前伸 0.39W / 后伸 0.21W（合计 0.61W）
      后肢 前伸 0.33W / 后伸 0.17W（合计 0.49W）
    ——极不对称，且**后肢的合计跨距是全身的瓶颈**。前后肢每周期的前进量必须相同，
    所以整套步态的跨距上限由后肢那 0.49W 定。首版按"看着合理"给 0.45–0.59W，
    结果蹬离相里蹄从目标点掉下去最多 2 单位、滑步残差 5.1——静帧完全看不出来。

    这里参数只给 `excursion`（跨距占鬐甲高的比例）和 `fwd_frac`（前伸占比，实测
    两者都在 0.65 上下），再按 0.88 的余量夹进可达域。
    """

    SAFETY = 0.88  # 可达域是静止姿量的；步态里躯干还会起伏俯仰，留出余量

    def __init__(
        self,
        rig: Rig,
        P,
        *,
        duty: float,
        td: dict[str, float],
        excursion: float,
        fwd_frac: float = 0.64,
        lift: float,
        hind_lift: float | None = None,
        swing_ease: float = 1.6,
        lift_shape: float = 1.0,
    ):
        self.rig = rig
        self.P = P
        self.duty = duty
        self.phases = phases_from_touchdown(td)
        w = P.wither
        spans = leg_spans(rig)
        cap = min(sum(spans[False]), sum(spans[True])) * self.SAFETY
        self.excursion = min(excursion * w, cap)
        self.clamped = excursion * w > cap + 1e-6
        self.fwd = self.excursion * fwd_frac
        self.back = self.excursion - self.fwd
        self.hind_fwd, self.hind_back = self.fwd, self.back
        self.lift = lift * w
        self.hind_lift = self.lift if hind_lift is None else hind_lift * w
        self.swing_ease = swing_ease
        self.lift_shape = lift_shape
        self.rest = rig.rest_stance()

    def key(self, side: str, hind: bool) -> str:
        return f"{'h' if hind else 'f'}{side}"

    def stance(self, side: str, hind: bool, t: float) -> bool:
        return wrap(t + self.phases[self.key(side, hind)]) < self.duty

    def stance_u(self, side: str, hind: bool, t: float) -> float | None:
        """支撑相内的相位 u∈[0,duty)，摆动相返回 None（诊断用自变量）。"""
        u = wrap(t + self.phases[self.key(side, hind)])
        return u if u < self.duty else None

    def target(self, side: str, hind: bool, t: float) -> tuple[np.ndarray, float]:
        k = self.key(side, hind)
        r = self.rest[k].copy()
        u = wrap(t + self.phases[k])
        fwd, back = (self.hind_fwd, self.hind_back) if hind else (self.fwd, self.back)
        z_front, z_back = r[2] - fwd, r[2] + back
        half = self.rig.sole_half(self.rig.leg_chain(side, hind)[-1])
        if u < self.duty:  # 支撑相：蹄锁地，相对身体匀速后移
            s = u / self.duty
            z = z_front + (z_back - z_front) * s
            pitch = -TOEOFF_PITCH * smooth(max(0.0, (s - 0.68) / 0.32))
            # 逆解锁的是蹄心，但蹬离真正的支点是蹄尖：绕蹄心翻会把蹄尖压进地里。
            # 抬蹄心 half·sin|θ| 等价于绕蹄尖翻。
            y = r[1] + half * abs(math.sin(math.radians(pitch)))
        else:  # 摆动相：抬起、前送、落下
            s = (u - self.duty) / (1.0 - self.duty)
            # 前送不能匀速：真实的摆动是前段甩到位、后段悬着等落地，所以用 ease-out。
            z = z_back + (z_front - z_back) * ease_out(s, self.swing_ease)
            lift = self.hind_lift if hind else self.lift
            # 起点必须接上支撑相末尾的蹬离翻蹄，落点归零准备平蹄着地。
            pitch = -TOEOFF_PITCH * (1.0 - s) - 14.0 * math.sin(math.pi * s)
            y = r[1] + lift * math.sin(math.pi * s**self.lift_shape) ** 0.75 + half * abs(
                math.sin(math.radians(pitch))
            )
        return np.array([r[0], y, z]), pitch

    def solve(self, pose: Pose, t: float) -> None:
        for side, hind in LEGS:
            tgt, pitch = self.target(side, hind, t)
            self.rig.solve_leg(pose, side, hind, tgt, foot_pitch=pitch)

    def flight(self) -> tuple[float, float] | None:
        return flight_window(self.duty, self.phases)

    def arc(self, t: float) -> float:
        """腾空抛物弧：窗口两端为 0、中点为 1，窗口外为 0。

        躯干高度不能用正弦——正弦在落地瞬间仍处于高位，落地帧直接够不着地面。
        真实腾空是抛物线：离地和落地两个时刻高度都回到常态，峰值只在腾空正中。
        """
        fw = self.flight()
        if fw is None:
            return 0.0
        f0, f1 = fw
        u = t if t >= f0 else t + 1.0
        if not (f0 <= u <= f1):
            return 0.0
        s = (u - f0) / (f1 - f0)
        return 4.0 * s * (1.0 - s)


# 落地时刻表（不是相位！）。四种步态的节拍就写在这四行里。
#   走   四拍侧序：后左 → 前左 → 后右 → 前右，四拍均分
#   快步 对角二拍：左前+右后 同落，右前+左后 同落，中间有短腾空
#   跑步 三拍（右前导）：后左 → (后右+前左) → 前右 → 腾空
#   袭步 四拍横向奔驰（右前导）：后左 → 后右 → 前左 → 前右 → 大腾空
WALK = dict(
    duty=0.62,
    td={"hl": 0.00, "fl": 0.25, "hr": 0.50, "fr": 0.75},
    excursion=0.30, lift=0.055, hind_lift=0.066, swing_ease=1.5,
)
TROT = dict(
    duty=0.44,
    td={"fl": 0.00, "hr": 0.00, "fr": 0.50, "hl": 0.50},
    excursion=0.36, lift=0.105, hind_lift=0.118, swing_ease=2.2,
)
CANTER = dict(
    duty=0.34,
    td={"hl": 0.00, "hr": 0.25, "fl": 0.25, "fr": 0.50},
    excursion=0.40, lift=0.132, hind_lift=0.144, swing_ease=2.6, lift_shape=0.85,
)
GALLOP = dict(
    duty=0.30,
    td={"hl": 0.00, "hr": 0.12, "fl": 0.32, "fr": 0.44},
    excursion=0.43, lift=0.168, hind_lift=0.182, swing_ease=3.0, lift_shape=0.70,
)


def tail_wave(pose: Pose, t: float, *, amp: float, freq: float, lag: float = 0.12,
              rise: float = 0.0, curl: float = 0.0, vert: float = 0.0) -> None:
    """尾行波：逐节延迟 + 振幅向尖端递增。整条同相摆是塑料尾巴。

    rise / curl 是**整条尾巴的总度数**，函数内部再分摊到 8 节。别写成逐节度数：
    单节 −22° 听着不多，8 节累加是 −176°，尾巴整条卷到背上去。
    """
    n = len(TAIL)
    wsum = (n + 1) / 2
    for i, b in enumerate(TAIL):
        w = (i + 1) / n
        ph = 2.0 * math.pi * (freq * t - lag * i)
        pose[b].rot[1] = amp * w**1.6 * math.sin(ph)
        pose[b].rot[0] = -rise / n - curl * w / wsum + vert * w**1.4 * math.sin(ph * 2.0 + 0.7)


def breathe(pose: Pose, t: float, P, *, rate: float, depth: float) -> None:
    """呼吸：胸廓抬合 + 腹部起伏。不用 scale——缩放会把挂在胸椎下的前肢一起拉长。"""
    a = math.sin(2.0 * math.pi * rate * t)
    b = math.sin(2.0 * math.pi * rate * t - 0.5)
    k = P.wither / 24.8
    pose["thorax_back"].pos[1] += depth * 0.55 * a * k
    pose["thorax_front"].pos[1] += depth * 0.42 * b * k
    pose["thorax_back"].rot[0] += -depth * 0.9 * a
    pose["thorax_front"].rot[0] += depth * 0.7 * b
    pose["lumbar"].pos[1] += depth * 0.3 * a * k


def plant(rig: Rig, pose: Pose, ground: dict[str, np.ndarray], pitches: dict[str, float] | None = None) -> None:
    """躯干摆完之后，把仍在地上的蹄逆解回它们的落点。"""
    for side, hind in LEGS:
        k = f"{'h' if hind else 'f'}{side}"
        if k in ground:
            rig.solve_leg(pose, side, hind, ground[k], foot_pitch=(pitches or {}).get(k, 0.0))


def neck_bend(pose: Pose, deg: float, *, skull: float = 0.0, share=(0.42, 0.34, 0.24)) -> None:
    """把总弯曲度数分摊到三节颈骨（正 = 抬头，负 = 低头）+ 颅骨补偿。

    逐节写同一个角度是最常见的错：三节各 −25° 累加是 −75°，头会从胸口穿出去。
    """
    for b, w in zip(NECK, share):
        pose[b].rot[0] += deg * w
    pose["skull"].rot[0] += skull


# ================================================================ 各动画
def anim_idle(rig: Rig, P, t: float) -> Pose:
    """静立：呼吸、重心微移、尾轻摆、偶尔一次甩头。存在感靠"几乎不动"。

    循环动画里所有周期必须是整周数——频率取非整数时 t=1 处相位落不回 0，
    接缝逐骨差几度，每轮循环肉眼可见地"咯噔"一下。
    """
    p = Pose()
    breathe(p, t, P, rate=2.0, depth=0.34)  # 6s 两次 ≈ 20 次/分，安静站立的马
    p["hips"].rot[2] = 0.6 * math.sin(2.0 * math.pi * t)
    p["lumbar"].rot[1] = 0.7 * math.sin(2.0 * math.pi * t - 0.5)
    toss = pulse(t, 0.58, 0.045)  # 甩头：马站着时隔一阵抖一下头驱蝇
    neck_bend(p, 1.2 + 0.8 * math.sin(2.0 * math.pi * 2.0 * t) + 9.0 * toss, skull=1.4 - 14.0 * toss)
    for i, b in enumerate(NECK):
        p[b].rot[1] += 0.9 * math.sin(2.0 * math.pi * t - 0.5 * i) + 5.0 * toss * math.sin(2.0 * math.pi * 6.0 * t)
    p["skull"].rot[1] += 1.8 * math.sin(2.0 * math.pi * t - 1.2) + 7.0 * toss * math.sin(2.0 * math.pi * 6.0 * t)
    p["jaw"].rot[0] = 0.8 + 0.6 * math.sin(2.0 * math.pi * 2.0 * t)
    flick = pulse(t, 0.30, 0.05)
    tail_wave(p, t, amp=2.6 + 16.0 * flick, freq=1.0, lag=0.14, rise=-4.0, curl=-6.0, vert=1.2 + 4.0 * flick)
    plant(rig, p, rig.rest_stance())
    return p


def _gait_pose(rig: Rig, P, t: float, gait: Gait) -> Pose:
    p = Pose()
    return p


def anim_walk(rig: Rig, P, t: float) -> Pose:
    """走：四拍侧序（后左→前左→后右→前右）。

    **头颈随步点前后点动**——马靠这个平衡走步，是"走"最好认的特征；快步没有。
    """
    g = Gait(rig, P, **WALK)
    p = Pose()
    k = P.wither / 24.8
    p["root"].pos[1] = 0.22 * k * math.sin(2.0 * math.pi * (2.0 * t + 0.10))
    p["hips"].rot[2] = 1.9 * math.sin(2.0 * math.pi * (t + 0.05))
    p["hips"].rot[0] = 0.7 * math.sin(2.0 * math.pi * (2.0 * t))
    p["lumbar"].rot[1] = 1.8 * math.sin(2.0 * math.pi * (t + 0.18))
    p["thorax_back"].rot[1] = 1.3 * math.sin(2.0 * math.pi * (t + 0.30))
    p["thorax_front"].rot[1] = -1.0 * math.sin(2.0 * math.pi * (t + 0.34))
    # 颈的点动：每步一次上下（幅度 4-5°），带得整个头一起点
    neck_bend(p, 2.0 + 4.6 * math.sin(2.0 * math.pi * (t + 0.22)), skull=2.6 * math.sin(2.0 * math.pi * (t + 0.30)))
    for i, b in enumerate(NECK):
        p[b].rot[1] = -1.2 * math.sin(2.0 * math.pi * (t + 0.36 - 0.06 * i))
    p["skull"].rot[1] = -1.6 * math.sin(2.0 * math.pi * (t + 0.42))
    tail_wave(p, t, amp=5.0, freq=1.0, lag=0.12, rise=3.0, curl=-6.0, vert=2.0)
    breathe(p, t, P, rate=2.0, depth=0.12)
    g.solve(p, t)
    return p


def anim_trot(rig: Rig, P, t: float) -> Pose:
    """快步：对角二拍。躯干每周期上下**两次**，头颈几乎不动（对角支撑天然平衡）。"""
    g = Gait(rig, P, **TROT)
    p = Pose()
    k = P.wither / 24.8
    arc = g.arc(t) + g.arc(wrap(t + 0.5))  # 二拍：一个周期里两段腾空
    p["root"].pos[1] = 0.95 * k * arc - 0.18 * k
    p["hips"].rot[0] = -1.6 * math.cos(2.0 * math.pi * 2.0 * t)
    p["lumbar"].rot[0] = 2.0 * math.cos(2.0 * math.pi * 2.0 * t)
    p["thorax_back"].rot[0] = 1.6 * math.cos(2.0 * math.pi * (2.0 * t + 0.06))
    p["hips"].rot[2] = 0.9 * math.sin(2.0 * math.pi * t)
    # 头颈刻意压到几乎不动：这是"快步"与"走"最直观的分野
    neck_bend(p, 3.0 + 0.9 * math.cos(2.0 * math.pi * 2.0 * t), skull=0.6)
    tail_wave(p, t, amp=6.0, freq=2.0, lag=0.09, rise=14.0, curl=-2.0, vert=3.0)
    g.solve(p, t)
    return p


def anim_canter(rig: Rig, P, t: float) -> Pose:
    """跑步：三拍（右前导）+ 腾空。躯干像摇椅一样前后俯仰——三拍的特征就在这个摇。"""
    g = Gait(rig, P, **CANTER)
    p = Pose()
    k = P.wither / 24.8
    fw = g.flight() or (0.0, 0.0)
    mid = (fw[0] + fw[1]) / 2
    arc = g.arc(t)
    rock = math.cos(2.0 * math.pi * (t - mid))
    p["root"].pos[1] = 1.5 * k * arc - 0.22 * k
    p["root"].rot[0] = -3.2 * rock  # 摇椅式俯仰
    p["hips"].rot[0] = -3.0 * rock
    p["lumbar"].rot[0] = 4.2 * rock
    p["thorax_back"].rot[0] = 3.0 * math.cos(2.0 * math.pi * (t - mid + 0.06))
    p["hips"].rot[2] = 1.6 * math.sin(2.0 * math.pi * t)  # 导腿侧的骨盆偏摆
    neck_bend(p, -2.0 - 3.4 * math.cos(2.0 * math.pi * (t - mid + 0.12)), skull=1.6 + 2.0 * rock)
    tail_wave(p, t, amp=9.0, freq=1.0, lag=0.08, rise=26.0, curl=2.0, vert=4.0)
    g.solve(p, t)
    return p


def anim_gallop(rig: Rig, P, t: float) -> Pose:
    """袭步：四拍横向奔驰 + 大腾空。颈**前伸压低**、尾平展——全速的两个外形标志。"""
    g = Gait(rig, P, **GALLOP)
    p = Pose()
    k = P.wither / 24.8
    fw = g.flight() or (0.0, 0.0)
    mid = (fw[0] + fw[1]) / 2
    arc = g.arc(t)
    flex = math.cos(2.0 * math.pi * (t - mid))  # 腾空最伸展，触地段收缩
    p["root"].pos[1] = 2.2 * k * arc - 0.32 * k
    p["root"].rot[0] = -2.4 * flex
    p["hips"].rot[0] = -3.6 * flex
    p["lumbar"].rot[0] = 5.4 * flex  # 马的腰段比猫科硬，屈伸幅度只给一半
    p["thorax_back"].rot[0] = 3.4 * math.cos(2.0 * math.pi * (t - mid + 0.06))
    p["thorax_front"].rot[0] = 2.2 * math.cos(2.0 * math.pi * (t - mid + 0.10))
    p["hips"].rot[2] = 1.2 * math.sin(2.0 * math.pi * t)
    neck_bend(p, -9.0 - 3.0 * math.cos(2.0 * math.pi * (t - mid + 0.14)), skull=4.0 + 2.4 * flex)
    p["jaw"].rot[0] = 5.0 + 2.5 * math.sin(2.0 * math.pi * 2.0 * t)
    tail_wave(p, t, amp=11.0, freq=1.0, lag=0.05, rise=42.0, curl=4.0, vert=5.0)
    g.solve(p, t)
    return p


_GRAZE_BEND: dict[int, tuple[float, float]] = {}


GRAZE_FACE = -40.0  # 吃草时头相对静止姿的累计俯仰。静止姿脸已下倾 54°，再加 40 ≈ 垂直
GRAZE_THORAX = -3.0  # 吃草时胸椎前段的下沉。求解与出姿必须用同一个值，否则解出来的高度不作数


def graze_bend(rig: Rig, P) -> tuple[float, float]:
    """扫描求出"吻端刚好落到草面"所需的颈总弯角。

    手写一个度数没法跨体型用：颈长 / 头长 / 枕高三档都不同，而且颈是**三节链**，
    分摊旋转后端点走的是弧不是圆周，落点跟总角度不成正比。首版拍了 −64°，实测吻端
    还悬在 5 单位高。
    改二分也不行——**吻端高度对弯角不单调**：弯过头整条颈连头一起卷回来，吻端反而
    升高（二分直接跑到 −160° 的下界还判"不够低"）。非单调函数只能扫。
    """
    key = id(rig)
    if key in _GRAZE_BEND:
        return _GRAZE_BEND[key]
    head_pts = {n: rig.bone_points(n) for n in ("skull", "jaw")}

    def muzzle_y(bend: float, skull: float) -> float:
        p = Pose()
        p["thorax_front"].rot[0] = GRAZE_THORAX  # 与 anim_graze 同步，漏了会让解出的高度偏 0.3
        neck_bend(p, bend, skull=skull, share=(0.46, 0.32, 0.22))
        W = rig.world(p)
        return min(float((pts @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min()) for n, pts in head_pts.items() if len(pts))

    # 颈弯与颅骨补偿**必须一起解**：只扫颈弯（颅骨按固定系数跟随）时，吻端最低只到 3.3，
    # 因为静止姿的头是相对颈折着的，不把头掰直，整条链根本够不到地。二维扫，代价里带一项
    # 对"脸接近垂直"的偏好（真马吃草脸就是近垂直的），免得解出个头朝天的怪姿势。
    best, best_cost = (-90.0, 40.0), 1e9
    for i in range(71):  # 颈弯 −140 … 0，2° 步长
        bend = -140.0 + i * 2.0
        for j in range(91):  # 颅骨 −60 … +120，2° 步长（上界要够：静止姿的头是折着的，
            skull = -60.0 + j * 2.0  # 不把头掰过来整条链够不到地，+60 的上界三档都顶死）
            my = muzzle_y(bend, skull)
            if my < 0.05:  # 吻端不许扎进草面以下——"接近地面"和"穿过地面"是两回事
                continue
            cost = abs(my - 0.6) + 0.02 * abs(bend + skull - GRAZE_FACE)
            if cost < best_cost:
                best, best_cost = (bend, skull), cost
    _GRAZE_BEND[key] = best
    return best


def anim_graze(rig: Rig, P, t: float) -> Pose:
    """吃草：颈下探到草面、咀嚼、偶尔抬头张望。马一天有一半时间在做这个。"""
    p = Pose()
    down = keyed(t, [(0.0, 1.0), (0.62, 1.0), (0.72, 0.0), (0.86, 0.0), (0.97, 1.0), (1.0, 1.0)])
    chew = math.sin(2.0 * math.pi * 12.0 * t) * down
    bend, skull = graze_bend(rig, P)
    neck_bend(p, bend * down, skull=skull * down, share=(0.46, 0.32, 0.22))
    p["skull"].rot[1] = 3.0 * down * math.sin(2.0 * math.pi * 2.0 * t)
    p["jaw"].rot[0] = (5.0 + 4.0 * chew) * down + 1.0
    p["thorax_front"].rot[0] = GRAZE_THORAX * down
    p["hips"].rot[2] = 0.5 * math.sin(2.0 * math.pi * t)
    breathe(p, t, P, rate=3.0, depth=0.22)
    tail_wave(p, t, amp=4.0, freq=2.0, lag=0.13, rise=-2.0, curl=-6.0, vert=2.0)
    plant(rig, p, rig.rest_stance())
    return p


def anim_rear(rig: Rig, P, t: float) -> Pose:
    """人立：后肢蹲坐蓄力 → 前躯拔起 → 前蹄空中刨动 → 落回。

    前蹄**离地**，所以不参与逆解——照样 plant 会把腾起的前肢硬拉回地面。
    """
    p = Pose()
    crouch = keyed(t, [(0.0, 0.0), (0.14, 1.0), (0.26, 0.4), (1.0, 0.0)])
    up = keyed(t, [(0.10, 0.0), (0.38, 1.0), (0.66, 1.0), (0.90, 0.0), (1.0, 0.0)])
    paw = math.sin(2.0 * math.pi * 3.0 * t) * up
    k = P.wither / 24.8

    p["root"].rot[0] = -46.0 * up
    p["root"].pos[1] = -1.6 * k * crouch
    p["hips"].rot[0] = 10.0 * up + 5.0 * crouch
    p["lumbar"].rot[0] = -8.0 * up
    p["thorax_back"].rot[0] = -6.0 * up
    neck_bend(p, 16.0 * up - 6.0 * crouch, skull=10.0 * up)
    p["jaw"].rot[0] = 16.0 * up
    tail_wave(p, t, amp=8.0, freq=2.0, lag=0.10, rise=18.0 * up - 6.0, curl=-8.0, vert=4.0)

    rest = rig.rest_stance()
    for side, hind in LEGS:
        key = f"{'h' if hind else 'f'}{side}"
        if hind:  # 后蹄始终着地，蹲的时候略向前收
            tgt = rest[key] + np.array([0.0, 0.0, -1.6 * k * crouch])
            rig.solve_leg(p, side, hind, tgt, foot_pitch=-10.0 * crouch)
        else:  # 前肢腾空：不逆解到地面，直接给屈曲姿（刨动靠肘/腕交替）
            ph = 0.0 if side == "l" else math.pi
            p[f"scapula_{side}"].rot[0] = -14.0 * up
            p[f"humerus_{side}"].rot[0] = -42.0 * up + 10.0 * paw * math.cos(ph)
            p[f"radius_{side}"].rot[0] = 62.0 * up + 16.0 * paw * math.cos(ph + 0.8)
            p[f"carpus_{side}"].rot[0] = 22.0 * up
    p["root"].pos[1] -= min(0.0, rig.lowest(p))  # 贴地夹持：别让后蹄转到地下去
    return p


def anim_kick(rig: Rig, P, t: float) -> Pose:
    """后踢：重心前移 → 双后肢向后蹬出 → 收回。马最实用的一招。"""
    p = Pose()
    load = keyed(t, [(0.0, 0.0), (0.22, 1.0), (0.34, 0.9), (0.70, 0.0)])
    kick = keyed(t, [(0.26, 0.0), (0.40, 1.0), (0.52, 0.85), (0.78, 0.0)])
    k = P.wither / 24.8

    p["root"].rot[0] = 14.0 * kick + 4.0 * load
    p["root"].pos[1] = -0.8 * k * load + 0.6 * k * kick
    p["hips"].rot[0] = -10.0 * kick
    p["lumbar"].rot[0] = 8.0 * load - 5.0 * kick
    p["thorax_back"].rot[0] = 4.0 * load
    neck_bend(p, -10.0 * kick - 4.0 * load, skull=-6.0 * kick)
    tail_wave(p, t, amp=10.0, freq=1.5, lag=0.09, rise=30.0 * kick + 6.0, curl=6.0, vert=5.0)

    rest = rig.rest_stance()
    for side, hind in LEGS:
        key = f"{'h' if hind else 'f'}{side}"
        if not hind:  # 前肢承重，蹄锁地
            rig.solve_leg(p, side, hind, rest[key], foot_pitch=0.0)
        else:  # 后肢向后上蹬出（离地，直接给关节角）
            p[f"femur_{side}"].rot[0] = 34.0 * kick - 8.0 * load
            p[f"tibia_{side}"].rot[0] = -26.0 * kick + 14.0 * load
            p[f"tarsus_{side}"].rot[0] = -44.0 * kick + 10.0 * load
            p[f"fetlock_h_{side}"].rot[0] = -18.0 * kick
    return p


def anim_hurt(rig: Rig, P, t: float) -> Pose:
    """受击：一缩、侧闪、抬头。短促，够读出"挨了一下"即可。"""
    p = Pose()
    hit = keyed(t, [(0.0, 0.0), (0.14, 1.0), (0.42, 0.35), (1.0, 0.0)])
    shake = math.sin(2.0 * math.pi * 9.0 * t) * hit
    k = P.wither / 24.8
    p["root"].pos[1] = -0.9 * k * hit
    p["root"].rot[2] = 5.0 * hit + 1.6 * shake
    p["hips"].rot[0] = 7.0 * hit
    p["lumbar"].rot[0] = -9.0 * hit
    p["thorax_back"].rot[0] = -7.0 * hit
    neck_bend(p, 13.0 * hit + 1.8 * shake, skull=9.0 * hit)
    p["jaw"].rot[0] = 20.0 * hit
    tail_wave(p, t, amp=12.0 * hit + 2.0, freq=2.0, lag=0.08, rise=16.0 * hit, curl=-10.0, vert=6.0 * hit)
    plant(rig, p, rig.rest_stance())
    return p


def anim_death(rig: Rig, P, t: float) -> Pose:
    """倒毙：前膝先软 → 侧倒 → 头最后落地。四足动物倒下都是前肢先失力。"""
    p = Pose()
    buckle = keyed(t, [(0.0, 0.0), (0.24, 1.0), (1.0, 1.0)])  # 前膝屈
    sink = keyed(t, [(0.16, 0.0), (0.62, 1.0), (1.0, 1.0)])  # 整体下沉
    roll = keyed(t, [(0.34, 0.0), (0.78, 1.0), (1.0, 1.0)])  # 侧倒
    headfall = keyed(t, [(0.52, 0.0), (0.92, 1.0), (1.0, 1.0)])
    k = P.wither / 24.8

    p["root"].rot[2] = -84.0 * roll
    p["root"].rot[0] = -5.0 * sink
    p["root"].pos[1] = -2.0 * k * sink
    p["hips"].rot[0] = 7.0 * sink
    p["lumbar"].rot[0] = -8.0 * buckle + 5.0 * sink
    p["thorax_back"].rot[0] = -10.0 * buckle
    p["thorax_front"].rot[0] = -7.0 * buckle
    neck_bend(p, 7.0 * buckle - 26.0 * headfall, skull=4.0 * buckle - 20.0 * headfall)
    for b in NECK:
        p[b].rot[2] = -6.0 * roll
    p["skull"].rot[2] = -10.0 * roll
    p["jaw"].rot[0] = 12.0 * buckle + 5.0 * (1.0 - headfall)
    tail_wave(p, t, amp=8.0 * (1.0 - sink) + 1.5, freq=0.5, lag=0.12,
              rise=8.0 * (1.0 - sink) - 6.0 * sink, curl=-5.0 * roll, vert=3.0 * (1.0 - sink))

    rest = rig.rest_stance()
    for side, hind in LEGS:
        key = f"{'h' if hind else 'f'}{side}"
        fold = buckle if not hind else sink
        tgt = rest[key] + np.array([0.0, 0.0, (3.6 * k if hind else -2.4 * k) * fold])
        rig.solve_leg(p, side, hind, tgt, foot_pitch=-16.0 * fold)

    p["root"].pos[1] -= rig.lowest(p)  # 贴地夹持：把整只压回地面，别转到地下去
    return p


# name → (时长秒, 是否循环, 采样数, 生成函数)
ANIMS = {
    "idle": (6.0, True, 32, anim_idle),
    "walk": (1.30, True, 30, anim_walk),
    "trot": (0.80, True, 26, anim_trot),
    "canter": (0.74, True, 26, anim_canter),
    "gallop": (0.56, True, 26, anim_gallop),
    "graze": (5.0, True, 32, anim_graze),
    "rear": (2.20, False, 30, anim_rear),
    "kick": (0.90, False, 24, anim_kick),
    "hurt": (0.50, False, 20, anim_hurt),
    "death": (2.80, False, 32, anim_death),
}
GAITS = {"walk": WALK, "trot": TROT, "canter": CANTER, "gallop": GALLOP}


def sample(rig: Rig, P, name: str, t01: float) -> Pose:
    return ANIMS[name][3](rig, P, t01)


# ---------------------------------------------------------------- 导出
def _uuid(seed: str) -> str:
    """确定性 v4 uuid（Blockbench 拿 uuid 当索引键，得是合法 v4 且熵够）。"""
    return str(uuid.UUID(bytes=hashlib.md5(seed.encode()).digest(), version=4))


def _kf(channel: str, time: float, vec, idx: int, aname: str) -> dict:
    """关键帧。字段照带动画的 bbmodel 工程逐项对齐：data_points 用**字符串**、
    bezier 四件套即使走 linear 也写全（缺字段是没必要担的读盘风险）。"""
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


def decimate(vals: list[tuple[float, list[float]]], tol: float) -> list[tuple[float, list[float]]]:
    """按 Ramer–Douglas–Peucker 抽稀关键帧：偏离"两端直线插值"在 tol 内的帧删掉。

    等间隔密采样出来的曲线里，绝大多数帧落在相邻两帧的连线上（尤其恒速段与静止段）。
    不抽稀时 10 条动画塞出 9907 帧、9 份模型 57 MB——那是把采样密度当成了信息量。
    首末帧强制保留（循环动画靠它们对齐接缝）。
    """
    if len(vals) <= 2:
        return vals
    keep = [False] * len(vals)
    keep[0] = keep[-1] = True
    stack = [(0, len(vals) - 1)]
    while stack:
        i, j = stack.pop()
        if j - i < 2:
            continue
        t0, v0 = vals[i]
        t1, v1 = vals[j]
        span = t1 - t0
        worst, wk = -1.0, -1
        for k in range(i + 1, j):
            tk, vk = vals[k]
            s = (tk - t0) / span if span > 1e-9 else 0.0
            d = max(abs(vk[c] - (v0[c] + (v1[c] - v0[c]) * s)) for c in range(3))
            if d > worst:
                worst, wk = d, k
        if worst > tol:
            keep[wk] = True
            stack.append((i, wk))
            stack.append((wk, j))
    return [v for v, k in zip(vals, keep) if k]


# 抽稀容差：旋转 0.25°、位移 0.03 单位（≈0.2 mm）。都远小于逐帧肉眼可辨的量。
DECIMATE_TOL = {"rotation": 0.40, "position": 0.05, "scale": 0.006}


def build_tracks(rig: Rig, P, name: str) -> tuple[float, bool, dict[str, dict[str, list]]]:
    """采样 → 每骨每通道的 (时间, 三元组) 序列。恒定通道直接丢掉。"""
    length, loop, n, _ = ANIMS[name]
    frames = []
    for i in range(n + 1):
        t = i / n
        if loop and i == n:
            frames.append((length, frames[0][1]))  # 循环末帧 = 首帧，接缝为零
            break
        frames.append((t * length, sample(rig, P, name, t)))

    tracks: dict[str, dict[str, list]] = {}
    for bone in rig.order:
        for chan, attr, default in (("rotation", "rot", 0.0), ("position", "pos", 0.0), ("scale", "scale", 1.0)):
            vals = [(tt, list(getattr(pz[bone], attr)) if bone in pz else [default] * 3) for tt, pz in frames]
            if all(abs(v[k] - default) < 1e-4 for _, v in vals for k in range(3)):
                continue
            tracks.setdefault(bone, {})[chan] = decimate(vals, DECIMATE_TOL[chan])
    return length, loop, tracks


def animations_block(rig: Rig, P, names: list[str]) -> list[dict]:
    anims = []
    for name in names:
        length, loop, tracks = build_tracks(rig, P, name)
        animators = {}
        for bone, chans in tracks.items():
            kfs = []
            for chan, vals in chans.items():
                for i, (tt, v) in enumerate(vals):
                    kfs.append(_kf(chan, tt, v, i, f"{name}{bone}{chan}"))
            animators[rig.bones[bone].uuid] = {"name": bone, "type": "bone", "keyframes": kfs}
        anims.append({
            "uuid": _uuid(f"anim:{MODEL_ID}:{name}"),
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
    return anims


def write_geckolib(rig: Rig, P, names: list[str], out: Path) -> None:
    """直出 GeckoLib animation.json —— **参考用，未经引擎侧验证**。

    Bedrock 动画的旋转符号约定与 Blockbench 面板显示是否一致，没有可对拍的实例。
    正经路径是把带动画的 bbmodel 交给 scripts/models/bbmodel_to_geckolib.py（驱动
    Blockbench 官方 codec 导出），由 codec 负责这层约定；本函数只用于人眼查看曲线。
    """
    animations = {}
    for name in names:
        length, loop, tracks = build_tracks(rig, P, name)
        bones = {}
        for bone, chans in tracks.items():
            entry = {}
            for chan, vals in chans.items():
                entry[chan] = {
                    str(round(tt, 4)): [round(v[0], 4), round(v[1], 4), round(v[2], 4)] for tt, vals_ in [(0, 0)] for tt, v in vals
                }
            bones[bone] = entry
        animations[f"animation.{NAMESPACE}.{MODEL_ID}.{name}"] = {
            "loop": bool(loop),
            "animation_length": round(length, 4),
            "bones": bones,
        }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps({"format_version": "1.8.0", "animations": animations}, indent="\t", ensure_ascii=False))


# ---------------------------------------------------------------- 自检
def footfall_chart(g: Gait, n: int = 40) -> str:
    """落蹄节拍图：四行 ASCII，█ = 支撑相，· = 摆动相。

    步态的身份就是这张图。"走是四拍侧序、快步是对角二拍"这种话，靠肉眼比对连拍是
    比不出来的；打成图一眼就能对：走应当四条错开均分，快步应当 fl/hr 与 fr/hl 两两
    完全重合，跑步应当中间两条重合、袭步四条依次错开且末尾有一整段空白（腾空）。
    """
    rows = []
    for key in ("fl", "fr", "hl", "hr"):
        side, hind = key[1], key[0] == "h"
        cells = "".join("█" if g.stance(side, hind, i / n) else "·" for i in range(n))
        rows.append(f"      {key}  {cells}")
    return "\n".join(rows)


def check(rig: Rig, P, names: list[str]) -> int:
    """步态自检：支撑相贴地 + 滑步残差 + 循环接缝。撞红就是动画有硬伤。"""
    bad = 0
    for name in names:
        length, loop, _n, _ = ANIMS[name]
        if name in GAITS:
            g = Gait(rig, P, **GAITS[name])
            legs = {
                f"{'h' if h else 'f'}{s}": (h, s, (lambda t, s=s, h=h: g.stance_u(s, h, t))) for s, h in LEGS
            }
            txt, worst_y, worst_slip = contact_report(rig, lambda t: sample(rig, P, name, t), legs, length)
            fw = g.flight()
            flight = f"腾空 {fw[0]:.2f}–{fw[1]:.2f}（{(fw[1] - fw[0]) * 100:.0f}%）" if fw else "无腾空"
            ok = worst_y < 0.30 and worst_slip < 0.30
            bad += 0 if ok else 1
            clamp = f" [跨距被可达域夹到 {g.excursion / P.wither:.3f}W]" if g.clamped else ""
            print(
                f"  {name:<7} {length:4.2f}s  跨距 {g.excursion / P.wither:.3f}W  {flight}  "
                f"{'✓' if ok else '✗'} 离地≤{worst_y:.2f} 滑步≤{worst_slip:.2f}{clamp}"
            )
            print(footfall_chart(g))
            if not ok:
                print(txt)
        if loop:  # 循环接缝：首末帧逐骨差
            p0, p1 = sample(rig, P, name, 0.0), sample(rig, P, name, 1.0)
            seam = max(
                (abs(a - b) for bn in set(p0) | set(p1) for a, b in zip(p0[bn].rot + p0[bn].pos, p1[bn].rot + p1[bn].pos)),
                default=0.0,
            )
            if seam > 0.05:
                print(f"  {name:<7} ✗ 循环接缝 {seam:.3f}（首末帧应完全一致）")
                bad += 1
    return bad


def main() -> int:
    ap = argparse.ArgumentParser(description="马动画生成（写回 9 份皮层）")
    ap.add_argument("--profile", choices=[*sorted(PROFILES), "all"], default="all")
    ap.add_argument("--only", nargs="*", help="只生成这些动画")
    ap.add_argument("--check", action="store_true", help="只跑步态自检，不写文件")
    args = ap.parse_args()

    names = args.only or list(ANIMS)
    pkeys = sorted(PROFILES) if args.profile == "all" else [args.profile]
    rc = 0

    for pk in pkeys:
        P = PROFILES[pk]
        src = FINAL / f"HorsePelt_rust_{pk}.bbmodel"
        if not src.is_file():
            print(f"找不到皮层: {src}（先跑 gen_pelt.py）")
            return 2
        rig = Rig(src)
        print(f"【{P.label}({pk})】骨 {len(rig.bones)}")
        rc += check(rig, P, names)
        if args.check:
            continue

        anims = animations_block(rig, P, names)
        total = sum(len(v["keyframes"]) for a in anims for v in a["animators"].values())
        # 同一体型的三种毛色共用同一套轨道（几何与 pivot 只随体型变，不随毛色变）
        for ck in COATS:
            fp = FINAL / f"HorsePelt_{ck}_{pk}.bbmodel"
            doc = json.loads(fp.read_text())
            doc["animations"] = anims
            # 带动画的模型走紧凑 JSON：indent=1 光缩进就占掉近一半体积
            fp.write_text(json.dumps(doc, ensure_ascii=False, separators=(",", ":")))
        write_geckolib(rig, P, names, STAGES / f"horse_{pk}.animation.json")
        print(f"  → 写入 {len(COATS)} 份毛色 · 动画 {len(anims)} 条 · 关键帧 {total}")
    return 1 if rc else 0


if __name__ == "__main__":
    raise SystemExit(main())
