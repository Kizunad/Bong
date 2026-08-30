#!/usr/bin/env python3
"""珂珂达 —— 动画生成：程序化步态 + 情绪/生理动作，写回 bbmodel 关键帧。

原则和建模层一致：**逐层派生，别凭手感拧角度**。

  · 走 / 跑只给「脚该踩在哪」，关节角由 rig.solve_limb 逆解，支撑相里脚锁死在世界
    坐标上。躯干怎么摇摆起伏都不会带着脚一起飘。
  · 摇摆幅度不是手调的：给定「质心要压到支撑脚上多少」，侧倾角由 asin 反解
    （见 rig.Waddle）。鹅之所以摇摆着走，就是因为髋距只有体宽的四成。
  · 原地动作一律 `settle()`：质心自动稳在静止姿那个位置上。颈一伸头就是个大杠杆，
    手写的躯干前倾角每改一次颈姿就过时。
  · 颈只有两个旋钮 —— straight 管多长、aim 管朝哪（见 rig.Goose.neck）。

角色设定：一只**呆萌**的大白鹅。所以基调是"慢、圆、有点笨"：idle 几乎不动，
走路一摇一摆，威吓时倒是很凶（鹅是真的凶），生理动作则要有明确的节拍和释放感。

输出:
  modelScript/models/kekeda_goose/KekedaGooseRig.bbmodel     带动画（可直接拖进 Blockbench）
  modelScript/models/kekeda_goose/kekeda_goose.animation.json  GeckoLib（参考/兜底）

源模型 KekedaPlume.bbmodel 只读不写。
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

from bbmodel_maker.rig.anim_rig import (  # noqa: E402
    Pose, build_tracks, decay_shake, keyed, pulse, write_bbmodel, write_geckolib,
)
from rig import PLUME, Goose, Waddle, leg_chain  # noqa: E402

OUT_BB = PLUME.parent / "KekedaGooseRig.bbmodel"
OUT_JSON = PLUME.parent / "kekeda_goose.animation.json"
NAMESPACE = "bong"
MODEL_ID = "kekeda_goose"

REST_AIM = 66.0     # 静止颈仰角，各动画都相对它给

#: 一次性动画里"东西该在这一刻掉出来"的归一化时刻。客户端拿它对齐粒子 / 掉落物，
#: 别自己数帧 —— 这两个数是动画节拍的一部分，改了动画就得跟着改。
#: 生成物的世界坐标走 rig.Goose.vent(pose)，check_anim 会在这一帧实测它下方是否通畅。
RELEASE = {"poop": 0.545, "lay_egg": 0.78}


# ---------------------------------------------------------------- 步态参数
# fwd/back 是相对静止落点的落脚窗口，**往后偏**。不是风格选择：髋关节长在脚的后方
# （髋 z +1.50，掌心 z −1.51），所以往前伸才是腿的极限方向 —— 实测把 fwd 从 2.0 加到
# 2.4，两连杆最大伸展就从 92.4% 涨到 93.8%，而 back 从 2.6 加到 3.2 一点没动。
# 跨距（fwd+back）决定步幅，前后如何分配决定会不会撞 IK 奇异点，两件事。
WALK = dict(duty=0.62, fwd=2.0, back=4.0, lift=1.9, swing_ease=1.8, toeoff=14.0)
# duty < 0.5 才有腾空段。0.46 → 每步两段各 4% 周期双脚离地，够读出"小跑"又不夸张。
# swing_ease 是摆动相前送的"前重"程度：f'(0) 正比于它。2.6 时脚在离地那一瞬以
# 62 单位/秒甩出去（tarsus 峰值 2225°/s，是全程均值的 8 倍），读成腿在抽而不是在跑。
# 1.5 + 更宽的抬脚缓冲把峰值压到均值的 3 倍左右。
# 跑起来步频高、单支撑相短，质心来不及压到脚上 —— 把静态平衡的目标比例调高一档补回来
TROT = dict(duty=0.48, fwd=1.8, back=4.6, lift=1.9, swing_ease=1.4, toeoff=13.0,
            hover_ramp=0.45, balance=0.82)
TROT_FLIGHT = (0.48, 0.50)   # 由 duty 与相位表算出的腾空窗口（右脚离地 → 左脚落地）


def flight_arc(t: float, window: tuple[float, float]) -> float:
    """腾空抛物弧：窗口两端为 0、中点为 1。

    不能用正弦 —— 正弦在落地瞬间仍在高位，腿够不着地面。真实腾空是抛物线：离地和
    落地两个时刻高度都回到常态，峰值只在正中。
    """
    f0, f1 = window
    span = (f1 - f0) % 1.0 or 1.0
    s = ((t - f0) % 1.0) / span
    return 4.0 * s * (1.0 - s) if 0.0 <= s <= 1.0 else 0.0


# ---------------------------------------------------------------- 各动画
def anim_idle(g: Goose, t: float) -> Pose:
    """呆站。存在感靠"几乎不动"——但不能真不动，静止的模型在游戏里读成卡死。

    循环动画里每个周期都必须是整周数：频率写成 1/2/3 倍基频，t=1 处相位正好回到 0。
    用 0.7 这种非整数倍的话接缝处逐骨差好几度，每轮循环肉眼可见地"咯噔"一下。
    """
    p = Pose()
    g.breathe(p, t, rate=2.0, depth=0.34)          # 4s 两次 ≈ 30 次/分
    # 头部微晃：两个不同倍频叠加，避免读成机械摆动
    sway = 1.6 * math.sin(2 * math.pi * t) + 0.7 * math.sin(2 * math.pi * 3 * t + 1.1)
    look = pulse(t, 0.62, 0.045)                   # 0.62 处快速瞥一眼
    g.neck(p, straight=0.02 * math.sin(2 * math.pi * 2 * t) - 0.06 * look,
           yaw=sway * 0.8 + 26.0 * look,
           aim=REST_AIM + 1.8 * math.sin(2 * math.pi * t - 0.9) + 5.0 * look)
    g.head(p, pitch=-1.5 * look, yaw=1.4 * sway + 14.0 * look)
    g.bill(p, 1.2 + 0.9 * math.sin(2 * math.pi * 2 * t))
    g.tail(p, lift=1.4 * math.sin(2 * math.pi * t + 0.4) + 5.0 * pulse(t, 0.28, 0.035),
           yaw=2.0 * math.sin(2 * math.pi * 2 * t))
    g.wings(p, fold=0.6 * math.sin(2 * math.pi * t - 0.3), asym=0.5)
    g.settle(p)
    return p


def anim_walk(g: Goose, t: float) -> Pose:
    """摇摆步。一步一摇是几何逼出来的，不是风格：髋距 ±1.70 而质心高 8.22，
    要把质心压到 ±1.88 的脚上，整个身子就得侧倾 9°。"""
    p = Pose()
    w = Waddle(g, **WALK)
    lean = w.lean(t)
    # 竖直起伏走 2 倍步频。相位不能凑：**最低点必须落在双支撑**——那一刻两脚一前一后
    # 分得最开，腿最需要够，身体再抬高就顶到满伸了（实测相位错开时伸展 95.2%，撞奇异区）。
    # 最高点在单支撑正中（脚就在髋正下方，最省腿），也就是 duty/2 处。
    p["root"].pos[1] = 0.26 * math.cos(2 * math.pi * 2 * (t - w.duty / 2)) - 0.34
    p["trunk_back"].rot[0] = 1.4 * math.sin(2 * math.pi * 2 * t)
    p["trunk_front"].rot[0] = -1.1 * math.sin(2 * math.pi * 2 * t + 0.4)
    p["trunk_back"].rot[1] = -2.2 * lean          # 身子往支撑侧扭一点，尾巴才不像块板
    p["trunk_front"].rot[1] = 1.6 * lean
    # 头节奏：鸟走路头有前后小幅"点动"，2 倍步频；再叠一点跟着摇摆的侧扭
    g.neck(p, straight=0.12 + 0.05 * math.sin(2 * math.pi * 2 * t - 1.2), yaw=-4.0 * lean,
           aim=REST_AIM + 3.2 * math.sin(2 * math.pi * 2 * t - 0.6))
    g.head(p, pitch=2.4 * math.sin(2 * math.pi * 2 * t - 1.4), yaw=-3.0 * lean)
    g.tail(p, lift=2.0 + 1.6 * math.sin(2 * math.pi * 2 * t), yaw=6.0 * lean)
    g.wings(p, fold=1.2 * math.sin(2 * math.pi * 2 * t + 0.8), lift=1.0 * abs(lean), asym=0.6)
    w.apply(p, t)
    return p


def anim_run(g: Goose, t: float) -> Pose:
    """小跑。鹅跑起来颈前伸压低、双翼半张扑打借力 —— 光靠腿倒腾快只会读成快放的走。"""
    p = Pose()
    w = Waddle(g, **TROT)
    lean = w.lean(t)
    arc = flight_arc(t, TROT_FLIGHT) + flight_arc(t, (TROT_FLIGHT[0] + 0.5, TROT_FLIGHT[1] + 0.5))
    # 跑起来整体压得比走更低：跨距大 + 腾空抬升，两样都吃腿长余量
    p["root"].pos[1] = 0.24 * arc + 0.22 * math.cos(2 * math.pi * 2 * (t - w.duty / 2)) - 1.80
    p["root"].rot[0] = -5.0                       # 整只前倾，重心压在前面
    p["trunk_back"].rot[0] = 2.2 * math.sin(2 * math.pi * 2 * t)
    p["trunk_back"].rot[1] = -3.0 * lean
    p["trunk_front"].rot[1] = 2.4 * lean
    g.neck(p, straight=0.42, yaw=-6.0 * lean, aim=30.0 + 4.0 * math.sin(2 * math.pi * 2 * t))
    g.head(p, pitch=6.0, yaw=-4.0 * lean)
    g.bill(p, 8.0 + 3.0 * math.sin(2 * math.pi * 2 * t))     # 跑起来微张着喘
    g.tail(p, lift=8.0, yaw=7.0 * lean)
    # 翅膀跟腿同频扑打（不是飞，是借一点力 + 平衡），左右给一点反相差
    beat = math.sin(2 * math.pi * t)
    g.wings(p, spread=16.0 + 10.0 * beat, lift=6.0 * beat, fold=-4.0, asym=2.5)
    w.apply(p, t, roll_scale=0.75)                # 跑起来摇摆收敛，摆太大会读成瘸
    return p


def anim_honk(g: Goose, t: float) -> Pose:
    """引颈高鸣：缩一下 → 颈猛地伸直上举、张喙 → 持续鸣叫带颤 → 收。"""
    p = Pose()
    wind = keyed(t, [(0.0, 0.0), (0.14, 1.0), (0.26, 0.0)])
    call = keyed(t, [(0.18, 0.0), (0.34, 1.0), (0.74, 0.94), (1.0, 0.0)])
    tremor = math.sin(2 * math.pi * 13.0 * t) * call

    # 抬身只给一点点：鸣叫的高度是**颈**给的（喙尖能上到 y≈21），身体一抬腿就满伸了
    p["root"].pos[1] = -0.5 * wind + 0.22 * call
    p["trunk_front"].rot[0] = 3.0 * wind - 5.0 * call
    p["trunk_back"].rot[0] = 2.0 * wind - 3.0 * call
    g.neck(p, straight=0.08 - 0.06 * wind + 0.58 * call,
           arc=-6.0 * call,
           aim=REST_AIM - 4.0 * wind + 18.0 * call + 1.2 * tremor)
    g.head(p, pitch=-4.0 * wind + 16.0 * call + 1.4 * tremor)
    g.bill(p, 3.0 * wind + 26.0 * call + 3.0 * tremor)
    g.tail(p, lift=2.0 + 10.0 * call)
    g.wings(p, spread=4.0 * wind + 14.0 * call, lift=8.0 * call, asym=1.0)
    g.settle(p, lean=-0.35 * call)                # 鸣叫时把重心送出去一点，才有气势
    return p


def anim_threat(g: Goose, t: float) -> Pose:
    """威吓：颈**平伸压低**、张喙嘶、双翼张开。这是鹅真实的恐吓姿势 ——
    和 honk 的"上举"必须一眼分得开，否则两个动作在远处是同一个剪影。"""
    p = Pose()
    coil = keyed(t, [(0.0, 0.0), (0.16, 1.0), (0.30, 0.55)])          # 先缩起来
    lunge = keyed(t, [(0.20, 0.0), (0.38, 1.0), (0.80, 0.92), (1.0, 0.0)])
    hiss = math.sin(2 * math.pi * 17.0 * t) * lunge

    p["root"].pos[1] = -0.9 * coil - 0.4 * lunge
    p["root"].rot[0] = -3.0 * lunge
    p["trunk_front"].rot[0] = 5.0 * coil - 9.0 * lunge
    p["trunk_back"].rot[0] = 3.0 * coil - 4.0 * lunge
    # coil 是"缩起来蓄势"，但缩过头头就埋进胸口 —— 按安全区表，aim 抬高一点才缩得起
    g.neck(p, straight=0.06 - 0.10 * coil + 0.84 * lunge,
           aim=REST_AIM + 10.0 * coil - 62.0 * lunge + 1.0 * hiss)
    g.head(p, pitch=-3.0 * coil + 6.0 * lunge + 0.8 * hiss)
    g.bill(p, 2.0 * coil + 22.0 * lunge + 2.4 * hiss)
    g.tail(p, lift=-4.0 * coil + 16.0 * lunge)
    # 翅膀张开是威吓的一半体量 —— 只伸脖子的话远处看不出这只鹅变大了
    g.wings(p, spread=8.0 * coil + 52.0 * lunge, lift=6.0 * coil + 18.0 * lunge,
            fold=-8.0 * lunge, asym=3.0)
    g.settle(p, lean=-0.9 * lunge)
    return p


def anim_poop(g: Goose, t: float) -> Pose:
    """拉粑粑。节拍是全部：**顿 → 蹲 → 绷 → 弹 → 抖 → 若无其事**。

    没有那记"弹"就只是蹲了一下；没有末尾的抖尾羽就没有"完事了"的收尾。

    头**全程抬着**，只在努责时快速一沉又弹回来。两个理由：一是笑点在"屁股在忙、
    脸上若无其事"；二是头一缩进肩里，剪影就读不出这是只鹅了 —— 首版把颈缩到
    straight −0.28，渲出来中段整只是一团白（见 rig.Goose.neck 的安全区表）。
    """
    p = Pose()
    halt = keyed(t, [(0.0, 0.0), (0.12, 1.0), (0.30, 0.85), (0.82, 0.5), (1.0, 0.0)])
    squat = keyed(t, [(0.08, 0.0), (0.34, 1.0), (0.58, 0.92), (0.80, 0.0)])
    # 努责：短促一记，峰值紧贴释放时刻之前
    strain = keyed(t, [(0.34, 0.0), (0.48, 1.0), (0.54, 0.9), (0.60, 0.0)])
    # 释放：尾巴猛地弹起，然后过冲回落一点。写成对称的凸包会读成"慢慢抬起又放下"
    flick = keyed(t, [(0.50, 0.0), (0.545, 1.0), (0.62, -0.35), (0.72, 0.0)])
    pop = max(0.0, flick)
    shake = keyed(t, [(0.62, 0.0), (0.70, 1.0), (0.92, 0.0)])         # 抖尾羽
    quiver = decay_shake(max(0.0, t - 0.62) * 10.0, 1.9, 1.4) * shake

    # 尾羽只有两块薄片，光靠 tail_base 转四十几度在侧视里几乎看不出来 —— 所以"翘尾"
    # 得由**整只前倾撅臀**一起演：root 负 pitch = 鼻子下沉、屁股抬起。首版怕读成"俯冲"
    # 而把这一档压掉了，其实俯冲感来自头一起沉；头保住之后就可以给足。
    p["root"].pos[1] = -2.05 * squat - 0.30 * strain + 0.35 * pop
    p["root"].rot[0] = -7.0 * squat - 3.0 * strain
    p["trunk_back"].rot[0] = 5.0 * squat + 6.0 * strain
    p["trunk_front"].rot[0] = -2.0 * squat - 3.0 * strain
    p["hips"].rot[0] = 5.0 * squat + 3.5 * strain
    p["hips"].rot[2] = 3.2 * quiver
    g.neck(p, straight=0.10 + 0.12 * squat - 0.06 * strain + 0.10 * pop,
           yaw=9.0 * halt * (1.0 - squat),
           aim=REST_AIM + 12.0 * squat + 4.0 * halt - 10.0 * strain + 9.0 * pop)
    g.head(p, pitch=-4.0 * strain + 6.0 * pop, yaw=6.0 * halt * (1.0 - squat))
    g.bill(p, 1.0 + 4.0 * strain + 9.0 * pop)
    # 尾必须抬够：出口挂在 tail_base 上，尾不抬就是对着自己的臀羽
    g.tail(p, lift=52.0 * squat + 14.0 * strain + 16.0 * flick, yaw=15.0 * quiver)
    g.wings(p, fold=3.0 * strain - 5.0 * pop, spread=6.0 * abs(quiver) + 10.0 * pop,
            lift=6.0 * pop, asym=1.0)
    g.settle(p)
    return p


def anim_lay_egg(g: Goose, t: float) -> Pose:
    """下蛋。比拉粑粑慢一倍、重一档：**察看 → 蹲坐 → 三次递强的努责 → 蛋出 →
    起身 → 回头看**。

    三次努责而不是一次：一次只是个动作，三次递强才读得出"在使劲"。最后那一眼回头
    是这段动画的句号 —— 少了它，蛋出来之后这只鹅像什么都没发生。
    """
    p = Pose()
    check = pulse(t, 0.06, 0.05)                                   # 蹲下前左右看一眼
    squat = keyed(t, [(0.08, 0.0), (0.26, 1.0), (0.74, 1.0), (0.90, 0.0)])
    relief = keyed(t, [(0.74, 0.0), (0.80, 1.0), (0.90, 0.35), (1.0, 0.0)])
    stand = keyed(t, [(0.80, 0.0), (0.92, 1.0), (1.0, 1.0)])
    glance = keyed(t, [(0.86, 0.0), (0.94, 1.0), (1.0, 0.30)])     # 回头看蛋

    # 三记努责，一次比一次狠。写成列表而不是三段 keyed：改节拍时只动这三个数。
    strain = sum(a * pulse(t, c, w) for c, a, w in
                 ((0.36, 0.55, 0.030), (0.50, 0.80, 0.036), (0.66, 1.0, 0.045)))
    strain = min(1.0, strain)

    # 蹲到底 = 站姿之外再沉 2.30（合计 2.90）。3.4 以上蹼板就开始扎进地里，
    # 而不是"腿折不动了" —— 极限是贴地，不是关节。
    p["root"].pos[1] = -2.30 * squat - 0.50 * strain - 0.45 * relief
    p["root"].pos[2] = 0.30 * strain
    p["root"].rot[0] = -4.5 * squat - 2.0 * strain + 2.0 * relief
    p["trunk_back"].rot[0] = 8.0 * squat + 9.0 * strain - 5.0 * relief
    p["trunk_front"].rot[0] = -4.0 * squat - 6.0 * strain + 3.0 * relief
    p["hips"].rot[0] = 5.0 * squat + 5.0 * strain
    # 颈：努责时快速一沉，蛋出来那一下猛地抬起张嘴（无声的"啊"），最后伸长回头看蛋。
    # straight 全程留正值 —— 回头看那一下 aim 压到 40，按安全区表颈必须同时伸长，
    # 否则头就埋进背里了（首版 straight −0.34 时头身间隙 −3.21，整只没有脖子）。
    g.neck(p, straight=0.08 + 0.10 * squat - 0.05 * strain + 0.22 * relief + 0.28 * glance,
           yaw=30.0 * check + 44.0 * glance,
           aim=REST_AIM + 10.0 * squat - 9.0 * strain + 16.0 * relief - 26.0 * glance)
    g.head(p, pitch=-5.0 * strain + 10.0 * relief - 16.0 * glance,
           yaw=16.0 * check + 22.0 * glance)
    g.bill(p, 1.0 + 5.0 * strain + 16.0 * relief)
    g.tail(p, lift=40.0 * squat + 14.0 * strain + 6.0 * relief,
           yaw=5.0 * math.sin(2 * math.pi * 3.0 * t) * strain)
    # 努责时翅膀夹紧，蛋出来那一下松开外张 —— 这一收一放是"用力/解脱"的读点
    g.wings(p, fold=6.0 * strain - 3.0 * relief, spread=4.0 * squat + 20.0 * relief,
            lift=10.0 * relief, asym=1.5)
    g.settle(p, lean=0.25 * squat - 0.30 * stand * glance)
    return p


def anim_hurt(g: Goose, t: float) -> Pose:
    """受击：一记硬顿挫 + 递减抖动，不是平滑正弦。翅膀炸开是鸟受惊的第一反应。"""
    p = Pose()
    hit = keyed(t, [(0.0, 0.0), (0.14, 1.0), (0.34, 0.42), (1.0, 0.0)])
    shake = decay_shake(t, 7.0, 5.0)
    p["root"].pos[1] = -0.9 * hit
    p["root"].pos[2] = 1.3 * hit
    p["root"].rot[2] = 4.0 * shake
    p["trunk_back"].rot[0] = 7.0 * hit
    p["trunk_front"].rot[0] = 5.0 * hit + 2.0 * shake
    g.neck(p, straight=0.05 - 0.14 * hit, yaw=8.0 * shake, aim=REST_AIM + 14.0 * hit)
    g.head(p, pitch=10.0 * hit, roll=6.0 * shake)
    g.bill(p, 24.0 * hit)
    g.tail(p, lift=16.0 * hit, yaw=8.0 * shake)
    g.wings(p, spread=34.0 * hit, lift=16.0 * hit, fold=-8.0 * hit, asym=4.0)
    g.settle(p, lean=0.35 * hit)
    return p


def anim_death(g: Goose, t: float) -> Pose:
    """倒地：腿一软 → 胸着地 → 侧翻 → 颈最后瘫下去，翅膀半摊开。

    侧翻绕的是 root（地面高度的轴），身体横跨 x=0，所以翻过去时背离侧会转到地面
    以下 —— 必须按帧把最低点夹回地面，而不是拍一个下沉量。
    """
    p = Pose()
    buckle = keyed(t, [(0.0, 0.0), (0.26, 1.0)])
    sink = keyed(t, [(0.16, 0.0), (0.56, 1.0)])
    roll = keyed(t, [(0.38, 0.0), (0.78, 1.0)])
    limp = keyed(t, [(0.50, 0.0), (0.90, 1.0)])

    p["root"].rot[2] = 72.0 * roll
    p["root"].rot[0] = -5.0 * sink
    p["root"].pos[1] = -3.2 * sink
    p["trunk_back"].rot[0] = -7.0 * buckle + 4.0 * sink
    p["trunk_front"].rot[0] = -5.0 * buckle
    p["hips"].rot[0] = 5.0 * sink
    g.neck(p, straight=-0.18 * buckle + 0.55 * limp, yaw=-16.0 * roll,
           aim=REST_AIM + 10.0 * buckle - 78.0 * limp)
    g.head(p, pitch=-14.0 * limp, yaw=-12.0 * roll, roll=-20.0 * roll)
    g.bill(p, 16.0 * buckle - 10.0 * limp)
    g.tail(p, lift=-10.0 * sink, yaw=-12.0 * roll)
    g.wings(p, spread=30.0 * buckle + 16.0 * roll, lift=-14.0 * sink, asym=6.0 * roll)

    # 腿：先逆解（还站着，脚要锁地），侧翻开始后**停掉逆解**，混到一个蜷起的死姿。
    # 否则身体都躺平了逆解还在硬把脚拽向地面目标，两连杆一路顶到 100% 满伸 —— 既进
    # 奇异区抽搐，看着也像被吊着的木偶而不是一只死鹅。
    rest = g.rest_feet()
    g.plant(p, {s: r + np.array([0.0, 0.0, 0.6 * buckle]) for s, r in rest.items()},
            {s: -14.0 * buckle for s in rest})
    dead = max(roll, 0.85 * sink, 0.45 * buckle)                 # 身子一沉就开始交棒，别等翻过去才停
    for s in ("l", "r"):
        for bone, curl in zip(leg_chain(s), (-16.0, 38.0, 52.0, -24.0)):
            p[bone].rot[0] = p[bone].rot[0] * (1.0 - dead) + curl * dead
            p[bone].rot[2] *= 1.0 - dead
    p["root"].pos[1] -= g.lowest(p)               # 贴地夹持：别让身体转到地下去
    return p


# name → (时长秒, 是否循环, 采样数, 生成函数)
#
# 采样数是**反向搜出来的**：对每条动画二分找「插值与解析姿态的几何偏差仍不超过 0.26
# 单位」的最小值，再加约 15% 余量（那个判据不是严格单调的，别贴着最小值用）。
# 别凭感觉给 —— 给少了动作峰值被线性插值削平，给多了纯是文件体积。改完动作要重跑
# check_anim，采样保真那一项会告诉你够不够。
ANIMS = {
    "idle":    (4.00, True, 48, anim_idle),
    "walk":    (0.90, True, 28, anim_walk),
    "run":     (0.68, True, 36, anim_run),
    "honk":    (1.30, False, 48, anim_honk),
    "threat":  (1.80, False, 40, anim_threat),
    "poop":    (1.50, False, 52, anim_poop),
    "lay_egg": (4.20, False, 66, anim_lay_egg),
    "hurt":    (0.45, False, 36, anim_hurt),
    "death":   (2.20, False, 28, anim_death),
}


def sample(g: Goose, name: str, t01: float) -> Pose:
    return ANIMS[name][3](g, t01)


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达动画生成")
    ap.add_argument("--only", nargs="*", help="只生成这些动画")
    args = ap.parse_args()
    names = args.only or list(ANIMS)
    g = Goose()

    entries = []
    for name in names:
        length, loop, n, _ = ANIMS[name]
        entries.append((name, length, loop, build_tracks(g, lambda t, nm=name: sample(g, nm, t),
                                                         length, loop, n)))
    write_bbmodel(PLUME, OUT_BB, "KekedaGooseRig", entries)
    write_geckolib(OUT_JSON, NAMESPACE, MODEL_ID, entries)

    total = 0
    for name, length, loop, tracks in entries:
        kf = sum(len(v) for c in tracks.values() for v in c.values())
        total += kf
        mark = f"  释放 @{RELEASE[name]:.2f}" if name in RELEASE else ""
        print(f"  {name:<8} {length:4.2f}s {'循环' if loop else '单次'}  "
              f"骨 {len(tracks):2d}  关键帧 {kf}{mark}")
    print(f"→ {OUT_BB.name} / {OUT_JSON.name}  共 {total} 关键帧")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
