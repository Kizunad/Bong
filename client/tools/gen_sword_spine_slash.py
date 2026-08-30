#!/usr/bin/env python3
"""sword_spine_slash — 异兽脊骨剑 (BeastSpineSword) 专属沉重骨刃撕裂下劈。

动作特色与世界观逻辑：
1. 异兽脊椎骨剑（带棘突倒钩与厚重骨节）不同于轻盈凡铁直剑，带有强烈的锯齿咬合与撕裂感。
2. 动作形制为「过肩微后引蓄力 → 沉重暴烈下劈 → 骨刺撕扯拉回」：
   - tick 0: guard 握姿，剑尖斜指前方偏上，中轴稳定。
   - tick 3: LOAD 蓄势，双手过肩抬剑，躯干后倾并轻扭蓄力。
   - tick 5: IMPACT 劈击，腰部猛烈前折发力，重剑自上而下沉重劈下，直臂送达极限。
   - tick 6: OVERSHOOT 剑尖带微前冲沉过身前，棘突破空撕裂。
   - tick 7: RIP PULL 顺势后拉微收，体现倒钩骨刺撕裂肌肉的收招顿挫。
   - tick 10: 回复初始持剑 guard。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 双手稳握，剑身斜前上方约 45°
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-12),
        head=dict(pitch=-2, yaw=+14),
        torso=dict(pitch=+2, yaw=+14),
        rightArm=dict(pitch=-56, yaw=-18, roll=-12, bend=42, axis=180),
        leftArm=dict(pitch=-64, yaw=+28, roll=-24, bend=72, axis=180),
        rightLeg=dict(pitch=-6, yaw=+4, bend=14),
        leftLeg=dict(pitch=+4, yaw=+4, bend=12, z=+0.02),
    ),
    3: dict(  # LOAD —— 过肩蓄势，腰部拧转，双手将沉重骨剑举至右肩后上方
        easing="INCUBIC",
        body=dict(x=+0.06, y=-0.02, z=-0.05, yaw=-16),
        head=dict(pitch=-4, yaw=+16),
        torso=dict(pitch=+4, yaw=+30),
        rightArm=dict(pitch=-86, yaw=+24, roll=-18, bend=65, axis=180),
        leftArm=dict(pitch=-72, yaw=+38, roll=-32, bend=88, axis=180),
        rightLeg=dict(pitch=-12, yaw=+4, bend=22),
        leftLeg=dict(pitch=+8, yaw=+4, bend=14, z=+0.04),
    ),
    5: dict(  # IMPACT —— 爆发力劈，躯干前折，直臂将重骨剑劈至身前中线
        easing="OUTQUAD",
        body=dict(x=-0.08, y=-0.02, z=+0.10, yaw=-10),
        head=dict(pitch=-8, yaw=-6),
        torso=dict(pitch=+8, yaw=-16),
        rightArm=dict(pitch=-22, yaw=-8, roll=+6, bend=8, axis=180),
        leftArm=dict(pitch=-42, yaw=+32, roll=-16, bend=76, axis=180),  # 左手辅助拉拽
        rightLeg=dict(pitch=-15, yaw=+6, bend=24),
        leftLeg=dict(pitch=+6, yaw=+6, bend=10, z=+0.02),
    ),
    6: dict(  # OVERSHOOT —— 剑身顺重力下沉，骨尖与侧刺撕裂空气
        easing="OUTQUAD",
        body=dict(x=-0.06, y=0.0, z=+0.08, yaw=-8),
        head=dict(pitch=-6, yaw=-8),
        torso=dict(pitch=+10, yaw=-12),
        rightArm=dict(pitch=+4, yaw=-22, roll=+16, bend=14, axis=180),
        leftArm=dict(pitch=-46, yaw=+28, roll=-12, bend=82, axis=180),
        rightLeg=dict(pitch=-10, yaw=+5, bend=18),
        leftLeg=dict(pitch=+4, yaw=+5, bend=8),
    ),
    7: dict(  # RIP PULL —— 撕裂滞留与回抽，倒钩撕扯停顿
        easing="INOUTSINE",
        body=dict(x=-0.02, y=0.0, z=+0.04, yaw=-10),
        head=dict(pitch=-4, yaw=+4),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-15, yaw=-18, roll=+10, bend=26, axis=180),
        leftArm=dict(pitch=-52, yaw=+25, roll=-16, bend=78, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=16),
        leftLeg=dict(pitch=+4, yaw=+4, bend=10),
    ),
    10: dict(  # 回到 guard
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-12),
        head=dict(pitch=-2, yaw=+14),
        torso=dict(pitch=+2, yaw=+14),
        rightArm=dict(pitch=-56, yaw=-18, roll=-12, bend=42, axis=180),
        leftArm=dict(pitch=-64, yaw=+28, roll=-24, bend=72, axis=180),
        rightLeg=dict(pitch=-6, yaw=+4, bend=14),
        leftLeg=dict(pitch=+4, yaw=+4, bend=12, z=+0.02),
    ),
}

DESCRIPTION = (
    "异兽脊骨剑双手撕裂下劈 (sword_spine_slash): 10-tick 沉重劈砍撕扯，"
    "右肩后引蓄力 -> 躯干前折下劈 -> 棘突撕扯后拉回位。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_spine_slash",
        description=DESCRIPTION,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
