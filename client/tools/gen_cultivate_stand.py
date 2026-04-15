#!/usr/bin/env python3
"""cultivate_stand — 站桩运功 (抱球呼吸)。

身法节奏（40 tick / 2s 循环）：
  tick 0/40 valley  呼气收 (抱球变小)
  tick 10  mid-in   吸气中
  tick 20 peak      吸气满 (抱球变大)
  tick 30 mid-out   呼气中

**抱球感的关键**: 双臂 pitch=-70° + bend=100° + yaw=±30° + roll=±15° axis=180° 把
双前臂折到胸前呈圆弧, 掌心相对 (roll 让掌朝内/朝上)。球的"呼吸" = bend 100° ↔ 105°
(吸气球变大 bend 减小肘张, 呼气球变小 bend 增大肘收)。

反僵硬要点：
  - 马步: 腿 pitch=-6° (脚略外开) + yaw=±14° + bend=28° (膝微屈, 承重姿)
  - 呼吸三联动: body.y ±0.04 + 抱球 bend ±5° + torso pitch ±3°
  - 肩微颤: rightArm roll 吸气 -12° / 呼气 -15° (掌心微翻动)
  - head pitch -3°→0°→-3° 微仰 (气沉丹田姿)
  - 循环首尾同值 (tick 0 == tick 40)
"""
from anim_common import emit_json

POSE = {
    0: dict(  # 呼气收
        easing="INOUTSINE",
        body=dict(y=-0.16),
        head=dict(pitch=-3),
        torso=dict(pitch=-2),
        rightArm=dict(pitch=-70, yaw=-28, roll=-15, bend=105, axis=180),
        leftArm=dict(pitch=-70, yaw=+28, roll=+15, bend=105, axis=180),
        rightLeg=dict(pitch=-6, yaw=-14, bend=28),
        leftLeg=dict(pitch=-6, yaw=+14, bend=28),
    ),
    10: dict(  # 吸气中
        easing="INOUTSINE",
        body=dict(y=-0.14),
        head=dict(pitch=-2),
        torso=dict(pitch=-1),
        rightArm=dict(pitch=-70, yaw=-29, roll=-13, bend=102, axis=180),
        leftArm=dict(pitch=-70, yaw=+29, roll=+13, bend=102, axis=180),
        rightLeg=dict(pitch=-6, yaw=-14, bend=28),
        leftLeg=dict(pitch=-6, yaw=+14, bend=28),
    ),
    20: dict(  # 吸气满 —— 抱球变大
        easing="INOUTSINE",
        body=dict(y=-0.12),
        head=dict(pitch=0),
        torso=dict(pitch=+1),
        rightArm=dict(pitch=-72, yaw=-30, roll=-12, bend=100, axis=180),
        leftArm=dict(pitch=-72, yaw=+30, roll=+12, bend=100, axis=180),
        rightLeg=dict(pitch=-6, yaw=-14, bend=28),
        leftLeg=dict(pitch=-6, yaw=+14, bend=28),
    ),
    30: dict(  # 呼气中
        easing="INOUTSINE",
        body=dict(y=-0.14),
        head=dict(pitch=-2),
        torso=dict(pitch=-1),
        rightArm=dict(pitch=-70, yaw=-29, roll=-14, bend=103, axis=180),
        leftArm=dict(pitch=-70, yaw=+29, roll=+14, bend=103, axis=180),
        rightLeg=dict(pitch=-6, yaw=-14, bend=28),
        leftLeg=dict(pitch=-6, yaw=+14, bend=28),
    ),
    40: dict(  # 闭环
        easing="INOUTSINE",
        body=dict(y=-0.16),
        head=dict(pitch=-3),
        torso=dict(pitch=-2),
        rightArm=dict(pitch=-70, yaw=-28, roll=-15, bend=105, axis=180),
        leftArm=dict(pitch=-70, yaw=+28, roll=+15, bend=105, axis=180),
        rightLeg=dict(pitch=-6, yaw=-14, bend=28),
        leftLeg=dict(pitch=-6, yaw=+14, bend=28),
    ),
}

DESCRIPTION = (
    "v1 JSON 站桩: 40 tick 抱球呼吸循环, 双臂 pitch-70° bend100° yaw±30° roll±15° axis=180° "
    "掌心相对抱球, 呼吸联动 body.y±0.04 + 抱球 bend±5° + torso±3° + head±3°, "
    "马步 pitch-6° yaw±14° bend28°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="cultivate_stand",
        description=DESCRIPTION,
        end_tick=40,
        stop_tick=43,
        is_loop=True,
    )
