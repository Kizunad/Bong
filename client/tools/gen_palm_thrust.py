#!/usr/bin/env python3
"""palm_thrust — 双掌气劲前推。

身法节奏（12 tick / 0.6s）：
  tick 0  guard   双臂身侧略抬 (pitch=-30° bend=80°)
  tick 3  吸气    双手收胸前 (pitch=-75° bend=135° yaw 内收±28°) + 后坐 + 躯干微仰 (吸气)
  tick 6  IMPACT  双掌水平前推 (pitch=-90° bend=28°) + body.z +0.20 前冲 + torso pitch+12° 前压
  tick 8  over    气劲推到尽头 (pitch=-94° bend=22°) + body.z +0.22
  tick 12 回     guard

反僵硬要点：
  - 12 tick 的拉长节奏 —— 气功掌劲是凝起来再推出去，不是直拳那种爆发
  - 吸气 body.z -0.06 下沉蓄势，推出 body.z +0.22 与躯干前压同步发力
  - 双臂 yaw 内收→外张：蓄力时 ±28° 手贴胸口，IMPACT 时 ±12° 张开成肩宽 (虎口朝外)
  - 双手 roll ±12° 让掌心朝前 (不是侧面) —— 这是"推"的视觉关键
  - axis=180° 让蓄力时手背贴胸而非反到身后
  - 马步微屈 (腿 bend=25° 蓄 → 15° 推) 脚踝发力
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=-0.04, z=0.0),
        head=dict(pitch=-2),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-30, yaw=-15, roll=-5, bend=80, axis=180),
        leftArm=dict(pitch=-30, yaw=+15, roll=+5, bend=80, axis=180),
        rightLeg=dict(pitch=-4, yaw=-6, bend=8),
        leftLeg=dict(pitch=-4, yaw=+6, bend=8),
    ),
    3: dict(  # 吸气 —— 手收胸前
        easing="INOUTSINE",
        body=dict(y=-0.06, z=-0.06),
        head=dict(pitch=+5),  # 低头凝掌
        torso=dict(pitch=-6, yaw=0),  # 微仰含胸
        rightArm=dict(pitch=-72, yaw=-28, roll=-15, bend=135, axis=180),
        leftArm=dict(pitch=-72, yaw=+28, roll=+15, bend=135, axis=180),
        rightLeg=dict(pitch=-6, yaw=-10, bend=25),
        leftLeg=dict(pitch=-6, yaw=+10, bend=25),
    ),
    6: dict(  # IMPACT —— 双掌前推
        easing="OUTQUAD",
        body=dict(y=-0.02, z=+0.20),
        head=dict(pitch=+3),  # 抬头盯前方 (回一点)
        torso=dict(pitch=+12, yaw=0),
        rightArm=dict(pitch=-90, yaw=-12, roll=-12, bend=28, axis=180),
        leftArm=dict(pitch=-90, yaw=+12, roll=+12, bend=28, axis=180),
        rightLeg=dict(pitch=-6, yaw=-8, bend=14),
        leftLeg=dict(pitch=-6, yaw=+8, bend=14),
    ),
    8: dict(  # overshoot —— 气劲到尽头
        easing="OUTQUAD",
        body=dict(y=-0.02, z=+0.22),
        head=dict(pitch=+2),
        torso=dict(pitch=+10, yaw=0),
        rightArm=dict(pitch=-94, yaw=-10, roll=-15, bend=22, axis=180),
        leftArm=dict(pitch=-94, yaw=+10, roll=+15, bend=22, axis=180),
        rightLeg=dict(pitch=-4, yaw=-8, bend=12),
        leftLeg=dict(pitch=-4, yaw=+8, bend=12),
    ),
    12: dict(  # 回 guard
        easing="INOUTSINE",
        body=dict(y=-0.04, z=0.0),
        head=dict(pitch=-2),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-30, yaw=-15, roll=-5, bend=80, axis=180),
        leftArm=dict(pitch=-30, yaw=+15, roll=+5, bend=80, axis=180),
        rightLeg=dict(pitch=-4, yaw=-6, bend=8),
        leftLeg=dict(pitch=-4, yaw=+6, bend=8),
    ),
}

DESCRIPTION = (
    "v1 JSON 双掌推: 12 tick 慢节奏气功, 吸气收胸 (bend 135° yaw ±28°) → "
    "IMPACT 水平前推 (bend 28° yaw ±12° 张开) + body.z 前冲 0.22 + torso 前压 +12°, "
    "双掌 roll ±12° 掌心朝前, axis=180°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="palm_thrust",
        description=DESCRIPTION,
        end_tick=12,
        stop_tick=15,
        is_loop=False,
    )
