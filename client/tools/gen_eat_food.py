#!/usr/bin/env python3
"""eat_food — 通用吃食物/服丹动作。

40 tick 非循环：右手抬物到口边 → 轻微仰头吞咽 → 手落回。作为毒丹路径首个接入点，
后续普通食物/灵茶/灵酒可复用同一 `bong:eat_food` 资源。
"""

from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
    ),
    8: dict(
        easing="OUTQUAD",
        body=dict(y=0.02),
        head=dict(pitch=4),
        torso=dict(pitch=2),
        rightArm=dict(pitch=-58, yaw=-18, roll=8, bend=54, axis=180),
        leftArm=dict(pitch=-12, yaw=12, roll=-4, bend=18, axis=180),
    ),
    18: dict(
        easing="INOUTSINE",
        body=dict(y=0.03),
        head=dict(pitch=-8),
        torso=dict(pitch=3),
        rightArm=dict(pitch=-76, yaw=-12, roll=10, bend=72, axis=180),
        leftArm=dict(pitch=-16, yaw=10, roll=-4, bend=18, axis=180),
    ),
    28: dict(
        easing="INOUTSINE",
        body=dict(y=0.01),
        head=dict(pitch=6),
        torso=dict(pitch=1),
        rightArm=dict(pitch=-48, yaw=-10, roll=6, bend=40, axis=180),
        leftArm=dict(pitch=-10, yaw=8, roll=-3, bend=12, axis=180),
    ),
    40: dict(
        easing="INOUTSINE",
        body=dict(y=0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
    ),
}

DESCRIPTION = "通用吃食物/服丹动作: 右手送入口边, 仰头吞咽, 40 tick 收束。"

if __name__ == "__main__":
    emit_json(
        POSE,
        name="eat_food",
        description=DESCRIPTION,
        end_tick=40,
        stop_tick=43,
        is_loop=False,
    )
