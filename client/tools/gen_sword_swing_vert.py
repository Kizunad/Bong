#!/usr/bin/env python3
"""sword_swing_vert — 双手剑下劈。

身法节奏（10 tick / 0.5s）：
  tick 0 guard   剑举右肩上 (pitch=-90°)
  tick 3 LOAD    极限高举过头 (pitch=-150°) + 躯干后仰蓄势 + 身体拔高
  tick 6 IMPACT  水平劈落 (pitch=-15°) + 躯干前压 + 身体下沉 + 前冲
  tick 7 over    小幅 overshoot (pitch=+5° 继续下劈一点)
  tick 10 回     回 guard

反僵硬要点：
  - 躯干 pitch 大幅波动 -10° → +15° (25° 弯腰弧度) —— 上下劈用"弯腰"不"扭腰"
  - body.y 从 -0.05 拔高 → +0.10 下沉 (0.15 格高度变化) 让剑走得更"猛"
  - body.z -0.06 后坐 → +0.18 前冲，整个人扑出去
  - 左手在 LOAD 同步举高 (pitch=-140° bend=10°) = 双手剑，IMPACT 时 counter-pull
  - axis=180° 让 guard 状态前臂朝前折 (剑柄不会别扭)
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-5),
        torso=dict(pitch=-2, yaw=+5),
        rightArm=dict(pitch=-90, yaw=-8, roll=-5, bend=30, axis=180),
        leftArm=dict(pitch=-80, yaw=+15, roll=+5, bend=50, axis=180),
        rightLeg=dict(bend=10),
        leftLeg=dict(bend=10),
    ),
    3: dict(  # LOAD —— 极限高举后仰
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.05, z=-0.06),
        head=dict(pitch=-14),
        torso=dict(pitch=-10, yaw=+12),
        rightArm=dict(pitch=-150, yaw=-4, roll=-3, bend=5, axis=180),
        leftArm=dict(pitch=-140, yaw=+10, roll=+3, bend=10, axis=180),
        rightLeg=dict(bend=6),
        leftLeg=dict(bend=6),
    ),
    6: dict(  # IMPACT —— 水平劈落猛前压
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.10, z=+0.18),
        head=dict(pitch=+22),
        torso=dict(pitch=+15, yaw=-6),
        rightArm=dict(pitch=-15, yaw=+6, roll=+22, bend=3, axis=180),
        leftArm=dict(pitch=-35, yaw=+22, roll=-15, bend=75, axis=180),  # counter-pull
        rightLeg=dict(bend=32, pitch=-6),
        leftLeg=dict(bend=28, pitch=-4),
    ),
    7: dict(  # overshoot —— 剑再向下一点
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.08, z=+0.14),
        head=dict(pitch=+18),
        torso=dict(pitch=+12, yaw=-3),
        rightArm=dict(pitch=+5, yaw=+8, roll=+28, bend=6, axis=180),
        leftArm=dict(pitch=-32, yaw=+20, roll=-10, bend=72, axis=180),
        rightLeg=dict(bend=28, pitch=-4),
        leftLeg=dict(bend=25),
    ),
    10: dict(  # 回 guard
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-5),
        torso=dict(pitch=-2, yaw=+5),
        rightArm=dict(pitch=-90, yaw=-8, roll=-5, bend=30, axis=180),
        leftArm=dict(pitch=-80, yaw=+15, roll=+5, bend=50, axis=180),
        rightLeg=dict(bend=10),
        leftLeg=dict(bend=10),
    ),
}

DESCRIPTION = (
    "v1 JSON 下劈: 极限高举 (pitch=-150°) → 水平劈落 (pitch=-15°), "
    "躯干 -10° 后仰 → +15° 前压 (25° 弯腰弧度), body.y 0.15 格拔高-下沉, "
    "双手剑双臂同步挥砍 + 左手 counter-pull, axis=180°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_swing_vert",
        description=DESCRIPTION,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
