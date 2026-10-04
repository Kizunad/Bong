#!/usr/bin/env python3
"""iron_sword_v2_use —— 铁剑（v2 模型）单手过顶直劈。

只落资产，不接线：产出 player_animation/iron_sword_v2_use.json，没有任何代码播放它。

模型事实（决定了动作怎么设计）：
  - iron_sword_v2 全长 26.6px，握柄在最下端，display 把剑身摆成**顺着前臂延长**
    （thirdperson rotation [-80, 90, 0]）。所以剑尖方向 ≈ 前臂方向：
    右臂 pitch -160° = 剑举过头指向身后上方，pitch -45° = 剑尖指前下方。
  - 单手剑，左手不上柄，做护胸 + load-snap。

节奏（10 tick / 0.5s，参考 sword_swing_vert 的五段结构，但改成单手、落点更高）：
  tick 0  guard       剑斜举在右前方（剑尖朝前上）
  tick 1  anticipation 只有 torso / head 微后仰，右臂不反向（§2.3）
  tick 3  windup      剑举过头，躯干后仰 + 右肩后拧
  tick 5  strike      劈落中途，重心开始前压
  tick 6  impact      剑尖停在前下方胸腹高度，身体前冲下沉
  tick 7  overshoot   剑尖再下 10°
  tick 10 guard       回到 tick 0，可连劈
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=-6),
    torso=dict(pitch=0, yaw=+10),
    rightArm=dict(pitch=-70, yaw=-10, roll=0, bend=25, axis=180),
    leftArm=dict(pitch=-40, yaw=+22, roll=-10, bend=70, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=10, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=16, z=-0.10),
)

POSE = {
    0: GUARD,
    1: dict(  # anticipation —— 只动躯干和头
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.02, z=-0.02),
        head=dict(pitch=-8, yaw=-6),
        torso=dict(pitch=-4, yaw=+14),
        rightArm=dict(pitch=-80, yaw=-10, roll=0, bend=25, axis=180),
        leftArm=dict(pitch=-42, yaw=+22, roll=-10, bend=66, axis=180),
        rightLeg=dict(pitch=+10, yaw=+4, bend=10, z=+0.04),
        leftLeg=dict(pitch=-14, yaw=+4, bend=16, z=-0.10),
    ),
    3: dict(  # windup —— 剑过头，右肩后拧，左手放松（load）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=-0.04, z=-0.05),
        head=dict(pitch=-12, yaw=-4),
        torso=dict(pitch=-8, yaw=+22),
        rightArm=dict(pitch=-172, yaw=-6, roll=0, bend=12, axis=180),
        leftArm=dict(pitch=-55, yaw=+26, roll=-8, bend=50, axis=180),
        rightLeg=dict(pitch=+12, yaw=+4, bend=8, z=+0.05),
        leftLeg=dict(pitch=-12, yaw=+4, bend=12, z=-0.10),
    ),
    5: dict(  # strike —— 劈落中途
        easing="INQUAD",
        body=dict(x=0.0, y=+0.04, z=+0.09),
        head=dict(pitch=+4, yaw=-2),
        torso=dict(pitch=+8, yaw=+2),
        rightArm=dict(pitch=-85, yaw=-8, roll=+4, bend=8, axis=180),
        leftArm=dict(pitch=-32, yaw=+18, roll=-14, bend=100, axis=180),
        rightLeg=dict(pitch=+14, yaw=+4, bend=14, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+4, bend=24, z=-0.11),
    ),
    6: dict(  # impact —— 剑尖停在前下方，左手猛收（snap）
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.08, z=+0.16),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+15, yaw=-6),
        rightArm=dict(pitch=-45, yaw=-8, roll=+6, bend=4, axis=180),
        leftArm=dict(pitch=-30, yaw=+14, roll=-16, bend=112, axis=180),
        rightLeg=dict(pitch=+16, yaw=+4, bend=18, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+4, bend=30, z=-0.12),
    ),
    7: dict(  # overshoot —— 剑尖再下 10°
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.07, z=+0.14),
        head=dict(pitch=+9, yaw=0),
        torso=dict(pitch=+14, yaw=-7),
        rightArm=dict(pitch=-35, yaw=-8, roll=+8, bend=8, axis=180),
        leftArm=dict(pitch=-32, yaw=+15, roll=-15, bend=106, axis=180),
        rightLeg=dict(pitch=+15, yaw=+4, bend=17, z=+0.05),
        leftLeg=dict(pitch=-21, yaw=+4, bend=28, z=-0.12),
    ),
    10: GUARD,
}

DESCRIPTION = (
    "铁剑 v2 单手过顶直劈：guard 剑斜举右前 → windup 剑过头后倾(-172°)、躯干后仰右肩后拧 → "
    "impact 剑尖停前下方(-45°)、身体前冲 0.16 下沉 0.08 → overshoot -35° → 回 guard。"
    "左手护胸 load-snap（bend 50°→112°）。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="iron_sword_v2_use", description=DESCRIPTION,
              end_tick=10, stop_tick=12, is_loop=False)
