#!/usr/bin/env python3
"""levitate — 御空悬浮。

身法节奏（60 tick / 3s 循环）：
  tick 0/60  bottom   浮动最低点
  tick 15   mid-rise  上升中
  tick 30   peak      悬浮最高点
  tick 45   mid-fall  下落中

**悬浮的视觉主体**: body.y 整体上浮 0 → +0.18 → 0 (Java v2 只 ±0.12, 放大到 ±0.18
让观众第一眼就看出"飘起来了"), 而不是腿/臂幅度。

反僵硬要点：
  - 双腿锁定垂直 (pitch=0 bend=0) 必须显式写 tick 0 和 60 双端, 不然循环会被
    findAfter 从 vanilla 走路姿态拉偏 (conventions §7.5 循环静止 axis 两端显式)
  - 袖摆 roll 反相位: right tick 15 -10° / 30 0° / 45 +10°, left 取相反
    这样 shirt 左右交替飘, 不是同步挥舞
  - head pitch -3°→-5°→-3° 微仰 (俯视人间姿, 不能低头看脚)
  - torso pitch -6° 挺胸 + yaw ±1° 随呼吸轻摆
  - 双臂 pitch=-12° yaw=±28° 微张 bend=12° axis=180° 掌心朝下 (飞剑式托掌而非抱球)
  - 气流颤: 臂 yaw tick 30 ±30° (开) tick 60 ±28° (合), 非死定值
"""
from anim_common import emit_json

POSE = {
    0: dict(  # bottom
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=-3),
        torso=dict(pitch=-6, yaw=+1),
        rightArm=dict(pitch=-12, yaw=-28, roll=0, bend=12, axis=180),
        leftArm=dict(pitch=-12, yaw=+28, roll=0, bend=12, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    15: dict(  # rising
        easing="INOUTSINE",
        body=dict(y=+0.10),
        head=dict(pitch=-4),
        torso=dict(pitch=-6, yaw=0),
        rightArm=dict(pitch=-12, yaw=-29, roll=-10, bend=13, axis=180),
        leftArm=dict(pitch=-12, yaw=+29, roll=+10, bend=13, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    30: dict(  # peak
        easing="INOUTSINE",
        body=dict(y=+0.18),
        head=dict(pitch=-5),
        torso=dict(pitch=-7, yaw=-1),
        rightArm=dict(pitch=-13, yaw=-30, roll=0, bend=12, axis=180),
        leftArm=dict(pitch=-13, yaw=+30, roll=0, bend=12, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    45: dict(  # falling
        easing="INOUTSINE",
        body=dict(y=+0.08),
        head=dict(pitch=-4),
        torso=dict(pitch=-6, yaw=0),
        rightArm=dict(pitch=-12, yaw=-29, roll=+10, bend=13, axis=180),
        leftArm=dict(pitch=-12, yaw=+29, roll=-10, bend=13, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    60: dict(  # 闭环
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=-3),
        torso=dict(pitch=-6, yaw=+1),
        rightArm=dict(pitch=-12, yaw=-28, roll=0, bend=12, axis=180),
        leftArm=dict(pitch=-12, yaw=+28, roll=0, bend=12, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
}

DESCRIPTION = (
    "v1 JSON 悬浮: 60 tick 慢循环, body.y ±0.18 主体上浮, 双腿锁垂直 (pitch=bend=0 双端显式), "
    "双臂微张 pitch-12° yaw±28° bend12° axis=180° 托掌, 袖摆 roll 反相位 ±10° 飘逸感, "
    "head pitch -3°→-5° 微仰, torso -6° 挺胸。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="levitate",
        description=DESCRIPTION,
        end_tick=60,
        stop_tick=63,
        is_loop=True,
    )
