#!/usr/bin/env python3
"""dagger_reverse_slash — 匕首反握大斜斩（从右上向左下斜划撕裂）。

动作要领：
    - 姿态：反手持匕（刀尖朝小臂方向或斜下伸出）
    - 轨迹：右上过顶蓄势 → 扭腰转跨斜劈左前下方 → 刀锋划过破甲撕扯 → 反手收势架护
    - 错峰动力学：后腿蹬地 t2 → 转腰蓄力 t3 → 斜挥斩击 t5 → 腕臂回抽 t6 → 回架 t8
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 反手斜架，小臂微曲，刀尖向内侧下探
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-30),
        head=dict(pitch=-2, yaw=+20),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=-30, yaw=-18, roll=+45, bend=70, axis=180),
        leftArm=dict(pitch=-24, yaw=+22, roll=-12, bend=90, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=14, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.04),
    ),
    2: dict(  # 蹬地蓄势 —— 重心压后，右手抬过右肩
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.01, z=-0.03, yaw=-30),
        head=dict(pitch=-3, yaw=+18),
        torso=dict(pitch=+4, yaw=+36),
        rightArm=dict(pitch=-65, yaw=-25, roll=+70, bend=85, axis=180),
        leftArm=dict(pitch=-18, yaw=+25, roll=-10, bend=80, axis=180),
        rightLeg=dict(pitch=+16, yaw=+4, bend=32, z=+0.05),
        leftLeg=dict(pitch=-8, yaw=+4, bend=16, z=-0.04),
    ),
    3: dict(  # LOAD —— 蓄力极点，手肘高抬，准备自右上向左下重劈
        easing="INCUBIC",
        body=dict(x=+0.05, y=+0.02, z=-0.05, yaw=-30),
        head=dict(pitch=-4, yaw=+15),
        torso=dict(pitch=+5, yaw=+45),
        rightArm=dict(pitch=-80, yaw=-30, roll=+85, bend=95, axis=180),
        leftArm=dict(pitch=-12, yaw=+28, roll=-8, bend=70, axis=180),
        rightLeg=dict(pitch=+18, yaw=+4, bend=38, z=+0.05),
        leftLeg=dict(pitch=-6, yaw=+4, bend=14, z=-0.03),
    ),
    5: dict(  # IMPACT —— 腰部反向猛转，反手大斜斩划破左前下方
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.02, z=+0.15, yaw=-30),
        head=dict(pitch=+4, yaw=+35),
        torso=dict(pitch=+6, yaw=-25),
        rightArm=dict(pitch=+15, yaw=+32, roll=-15, bend=45, axis=180),
        leftArm=dict(pitch=-28, yaw=+8, roll=-25, bend=115, axis=180),  # 左手猛收护胸
        rightLeg=dict(pitch=+2, yaw=+6, bend=10, z=+0.02),
        leftLeg=dict(pitch=-22, yaw=+2, bend=36, z=-0.08),
    ),
    6: dict(  # overshoot —— 刀尖斜下划出，腕臂滞后翻转
        easing="INOUTSINE",
        body=dict(x=-0.03, y=-0.02, z=+0.18, yaw=-30),
        head=dict(pitch=+5, yaw=+36),
        torso=dict(pitch=+6, yaw=-28),
        rightArm=dict(pitch=+22, yaw=+36, roll=-20, bend=40, axis=180),
        leftArm=dict(pitch=-26, yaw=+6, roll=-24, bend=110, axis=180),
        rightLeg=dict(pitch=+1, yaw=+6, bend=8, z=+0.02),
        leftLeg=dict(pitch=-20, yaw=+2, bend=34, z=-0.07),
    ),
    8: dict(  # 回架 == tick 0
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-30),
        head=dict(pitch=-2, yaw=+20),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=-30, yaw=-18, roll=+45, bend=70, axis=180),
        leftArm=dict(pitch=-24, yaw=+22, roll=-12, bend=90, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=14, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.04),
    ),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_reverse_slash",
        description="匕首反握大斜斩：右上过顶蓄力，自上而下斜扫左前，反手撕裂划割",
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
