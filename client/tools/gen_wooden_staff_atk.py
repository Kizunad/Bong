#!/usr/bin/env python3
"""wooden_staff_atk —— 木杖单手横扫（右手持杖）。

只落资产，不接线：产出 player_animation/wooden_staff_atk.json。

木杖只在右手。左手自然放松，只随身体轻摆，不做任何平衡、扶杖姿势（原版只有右手能挂物品）。
动作：杖从右侧后方拉起，绕身体右侧横向扫到身前左侧，杖尖在收势前点向前方。
躯干先转（蓄势向右，再向左反拧），手臂跟在躯干后面，腕部最后带动杖尖。

节奏（11 tick，非循环）：
  tick 0  guard      杖在右侧，杖尖斜向上前，握点在右腰高度
  tick 2  windup     杖拉到身体右后方，躯干向右拧（蓄势）
  tick 5  apex       杖尖抬到右前上方，躯干回正
  tick 8  sweep      杖从右前横扫到身前左侧，躯干向左反拧（发力）
  tick 9  follow     杖尖继续向左前送，左手随身体摆动
  tick 11 guard      收回守势，与 tick 0 同值
"""

from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+8),
        torso=dict(pitch=+4, yaw=-12),
        rightArm=dict(pitch=-55, yaw=+4, roll=+12, bend=80, axis=180),
        leftArm=dict(pitch=+2, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+3, bend=14, z=-0.06),
    ),

    2: dict(
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=-0.02),
        head=dict(pitch=-2, yaw=+6),
        torso=dict(pitch=+2, yaw=+22),
        rightArm=dict(pitch=-15, yaw=+55, roll=+22, bend=95, axis=180),
        leftArm=dict(pitch=+6, yaw=+8, roll=-8, bend=14, axis=180),
        rightLeg=dict(pitch=+10, yaw=-3, bend=14, z=+0.04),
        leftLeg=dict(pitch=-14, yaw=+4, bend=18, z=-0.08),
    ),

    5: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=+2),
        torso=dict(pitch=+3, yaw=+6),
        rightArm=dict(pitch=-100, yaw=+24, roll=+6, bend=30, axis=180),
        leftArm=dict(pitch=+4, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+8, yaw=-2, bend=12, z=+0.03),
        leftLeg=dict(pitch=-12, yaw=+3, bend=16, z=-0.07),
    ),

    8: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.06),
        head=dict(pitch=+2, yaw=-8),
        torso=dict(pitch=+6, yaw=-26),
        rightArm=dict(pitch=-92, yaw=-48, roll=+10, bend=14, axis=180),
        leftArm=dict(pitch=+14, yaw=-4, roll=-6, bend=18, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=22, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+4, bend=34, z=-0.12),
    ),

    9: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.06),
        head=dict(pitch=+3, yaw=-9),
        torso=dict(pitch=+7, yaw=-30),
        rightArm=dict(pitch=-94, yaw=-58, roll=+8, bend=8, axis=180),
        leftArm=dict(pitch=+16, yaw=-4, roll=-6, bend=20, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=24, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+4, bend=36, z=-0.12),
    ),

    11: dict(  # 与 tick 0 同值，收回守势
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+8),
        torso=dict(pitch=+4, yaw=-12),
        rightArm=dict(pitch=-55, yaw=+4, roll=+12, bend=80, axis=180),
        leftArm=dict(pitch=+2, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+3, bend=14, z=-0.06),
    ),
}

DESCRIPTION = (
    "木杖单手横扫（右手持杖）：11 tick。左手自然放松。"
    "guard 杖在右侧 → windup 杖拉到右后方、躯干右拧 → apex 杖尖抬到右前上方 → "
    "sweep 杖从右前横扫到身前左侧、躯干左反拧发力 → follow 杖尖继续向左前送 → 收回守势。"
)

if __name__ == "__main__":
    emit_json(POSE, name="wooden_staff_atk", description=DESCRIPTION,
              end_tick=11, stop_tick=13, is_loop=False)
