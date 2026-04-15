#!/usr/bin/env python3
"""tribulation_brace — 抗劫姿态 (长时间抗天雷, 循环颤抖)。

身法节奏（20 tick / 1s 循环）：
  tick 0/20  base    稳态 (双臂交叉护头 + 微蹲)
  tick 5     shake-R 右震 (body.x +0.03, head yaw +3°)
  tick 10    low     body.y 最低 (-0.14 承压)
  tick 15    shake-L 左震

**Java pitch=-150° bend=135° 的陷阱**: 和 guard_raise v1 一样, 会把手臂绕过头顶指
向身后下方 (axis 默认 0° 让 bend 朝身后折)。改 pitch=-95° (垂直略前) + bend=130° +
yaw=±35° + axis=180° 实现双臂举过头顶交叉于面前上方, 肘微外展 (防御姿比 guard_raise
更"举高" —— 这是长时间抗天雷)。

和 guard_raise 对比:
  - guard_raise: 4 tick 快闪格挡, pitch=-85° (脸前交叉), 一次性
  - tribulation_brace: 20 tick 长抗循环, pitch=-95° (头顶交叉), 加颤抖

反僵硬要点：
  - body.x 颤抖 LINEAR (不是 sine): 被雷劈的肌肉抽搐不是平滑摆
  - body.y 承压: 0→-0.10→-0.14→-0.10 高频下压 (天雷"嘣" 一下的感觉)
  - head yaw ±3° + pitch 6°↔7° 紧咬牙关微颤
  - torso pitch +12° 前倾承压 (不动, 用 body/head 的震动承载)
  - 腿 bend=22° 稳固站姿 (不屈膝跪, 抗姿)
  - 循环首尾同值
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.10),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+12),
        rightArm=dict(pitch=-95, yaw=-35, roll=-10, bend=130, axis=180),
        leftArm=dict(pitch=-95, yaw=+35, roll=+10, bend=130, axis=180),
        rightLeg=dict(bend=22),
        leftLeg=dict(bend=22),
    ),
    5: dict(  # shake-R
        easing="LINEAR",
        body=dict(x=+0.03, y=-0.12),
        head=dict(pitch=+6, yaw=+3),
        torso=dict(pitch=+12),
        rightArm=dict(pitch=-94, yaw=-35, roll=-10, bend=130, axis=180),
        leftArm=dict(pitch=-96, yaw=+36, roll=+10, bend=131, axis=180),
        rightLeg=dict(bend=22),
        leftLeg=dict(bend=22),
    ),
    10: dict(  # 低谷 —— 承压最重
        easing="LINEAR",
        body=dict(x=-0.03, y=-0.14),
        head=dict(pitch=+7, yaw=-2),
        torso=dict(pitch=+13),
        rightArm=dict(pitch=-96, yaw=-36, roll=-11, bend=132, axis=180),
        leftArm=dict(pitch=-94, yaw=+35, roll=+9, bend=130, axis=180),
        rightLeg=dict(bend=23),
        leftLeg=dict(bend=23),
    ),
    15: dict(  # shake-L
        easing="LINEAR",
        body=dict(x=+0.02, y=-0.12),
        head=dict(pitch=+6, yaw=+2),
        torso=dict(pitch=+12),
        rightArm=dict(pitch=-95, yaw=-36, roll=-10, bend=131, axis=180),
        leftArm=dict(pitch=-95, yaw=+35, roll=+11, bend=130, axis=180),
        rightLeg=dict(bend=22),
        leftLeg=dict(bend=22),
    ),
    20: dict(  # 闭环
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.10),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+12),
        rightArm=dict(pitch=-95, yaw=-35, roll=-10, bend=130, axis=180),
        leftArm=dict(pitch=-95, yaw=+35, roll=+10, bend=130, axis=180),
        rightLeg=dict(bend=22),
        leftLeg=dict(bend=22),
    ),
}

DESCRIPTION = (
    "v1 JSON 抗劫: 20 tick 循环, 双臂举过头交叉 (pitch-95° bend130° yaw±35° axis=180°), "
    "body.x ±0.03 LINEAR 颤抖 + body.y -0.10→-0.14 承压, head yaw±3° + pitch±1°, "
    "torso+12° 前倾, 腿 bend22° 稳站。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="tribulation_brace",
        description=DESCRIPTION,
        end_tick=20,
        stop_tick=23,
        is_loop=True,
    )
