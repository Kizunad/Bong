#!/usr/bin/env python3
"""bone_sword_slash —— 骨剑单手水平横斩（刃口朝挥击方向）。

只落资产，不接线：产出 player_animation/bone_sword_slash.json。

骨剑是双刃剑：刃线沿模型 z 轴分布在两侧，刀面法线是模型 x 轴，刃长是模型 y 轴。
**挥击时刃线（z）必须与挥击方向平行，刀面（x）必须垂直于挥击方向**——用刀面（剑背 / 剑面）
去拍是错的。rightItem 三个角由「刃线对齐求解」得出（方法与逐帧检查见 bone_sword_slash.md）。

单手：剑只在右手，左手自然放松。
挥击：剑从身体右后侧拉回（windup），经身前横扫到身前左侧（impact，挥击方向 = 玩家左侧，即 +X），
躯干先向右拧、再向左反拧，手臂跟在躯干之后。

节奏（9 tick，非循环）：
  tick 0  guard       剑在右前上方，刃长朝上
  tick 2  windup      剑拉回右后侧，刃线朝后（与拉回方向对齐）
  tick 5  sweep       剑回到身前中线，躯干开始左转
  tick 7  impact      剑横扫到身前左侧，刃线与挥击方向对齐（对齐度 0.97）
  tick 8  overshoot   继续向左扫，刃线仍对齐
  tick 9  guard       回到守势，与 tick 0 同值
"""

from anim_common import emit_json

# 静止姿态的腕部角度（度）：刃长朝上，刃线朝前左。tick 0 与 tick 9 必须同值。
_REST_ITEM = dict(pitch=0, yaw=-70, roll=10)

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+6),
        torso=dict(pitch=+4, yaw=-10),
        rightArm=dict(pitch=-60, yaw=+22, roll=+10, bend=75, axis=180),
        leftArm=dict(pitch=+2, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+3, bend=14, z=-0.06),
        rightItem=_REST_ITEM,
    ),

    2: dict(
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=-0.02),
        head=dict(pitch=-2, yaw=+4),
        torso=dict(pitch=+4, yaw=+18),
        rightArm=dict(pitch=-25, yaw=+65, roll=+20, bend=100, axis=180),
        leftArm=dict(pitch=+6, yaw=+8, roll=-8, bend=14, axis=180),
        rightLeg=dict(pitch=+10, yaw=-3, bend=14, z=+0.04),
        leftLeg=dict(pitch=-14, yaw=+4, bend=18, z=-0.08),
        rightItem=dict(pitch=0, yaw=-20, roll=0),
    ),

    5: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=+0.02),
        head=dict(pitch=-2, yaw=+2),
        torso=dict(pitch=+3, yaw=-4),
        rightArm=dict(pitch=-78, yaw=+22, roll=+6, bend=60, axis=180),
        leftArm=dict(pitch=+4, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+8, yaw=-2, bend=12, z=+0.03),
        leftLeg=dict(pitch=-12, yaw=+3, bend=16, z=-0.07),
        rightItem=dict(pitch=0, yaw=-20, roll=+20),
    ),

    6: dict(  # 横扫中段：剑沿弧线扫过身前，刃线需要单独对准（见 md 的逐帧检查）
        easing="INOUTSINE",
        body=dict(x=-0.01, y=+0.015, z=+0.04),
        head=dict(pitch=-1.5, yaw=-2),
        torso=dict(pitch=+4.5, yaw=-12),
        rightArm=dict(pitch=-82, yaw=-14, roll=+7, bend=41, axis=180),
        leftArm=dict(pitch=+8, yaw=+2, roll=-6, bend=15, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2.5, bend=16, z=+0.04),
        leftLeg=dict(pitch=-15, yaw=+3.5, bend=23, z=-0.09),
        rightItem=dict(pitch=0, yaw=-25, roll=+20),
    ),

    7: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.06),
        head=dict(pitch=-1, yaw=-6),
        torso=dict(pitch=+6, yaw=-20),
        rightArm=dict(pitch=-86, yaw=-50, roll=+8, bend=22, axis=180),
        leftArm=dict(pitch=+12, yaw=-2, roll=-6, bend=18, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=20, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+4, bend=30, z=-0.11),
        rightItem=dict(pitch=0, yaw=-30, roll=+20),
    ),

    8: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.06),
        head=dict(pitch=-1, yaw=-7),
        torso=dict(pitch=+7, yaw=-24),
        rightArm=dict(pitch=-90, yaw=-66, roll=+6, bend=10, axis=180),
        leftArm=dict(pitch=+14, yaw=-4, roll=-6, bend=20, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=22, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+4, bend=34, z=-0.12),
        rightItem=dict(pitch=0, yaw=-30, roll=+20),
    ),

    9: dict(  # 与 tick 0 同值
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+6),
        torso=dict(pitch=+4, yaw=-10),
        rightArm=dict(pitch=-60, yaw=+22, roll=+10, bend=75, axis=180),
        leftArm=dict(pitch=+2, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+3, bend=14, z=-0.06),
        rightItem=_REST_ITEM,
    ),
}

DESCRIPTION = (
    "骨剑单手水平横斩（刃口朝挥击方向）：9 tick。剑只在右手，左手自然放松。"
    "guard 剑在右前上方 → windup 剑拉回右后侧、刃线朝后 → sweep 剑回身前中线、躯干开始左转 → "
    "impact 剑横扫到身前左侧，刃线与挥击方向对齐 → overshoot 继续向左扫 → 回守势。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bone_sword_slash", description=DESCRIPTION,
              end_tick=9, stop_tick=11, is_loop=False)
