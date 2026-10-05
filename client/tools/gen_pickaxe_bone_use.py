#!/usr/bin/env python3
"""pickaxe_bone_use —— 骨镐（bone pickaxe）纯单手挥凿（循环）。

只落资产，不接线：产出 player_animation/pickaxe_bone_use.json。

纯单手：镐只在右手。左手自然垂放，只随身体轻微摆动，不做任何举手、护架、平衡姿势。
挥凿：镐从右前上方抬起，越过头侧，向前下方啄下，腕部把镐头压向地面；落下后回弹，
再回到起手位，循环。

节奏（8 tick 一周期，首末帧同值，循环闭合）：
  tick 0  raise       镐在右前上方，镐头朝上
  tick 3  top         镐抬到头侧最高点，手腕内收
  tick 5  strike      镐啄向前下方，腕部压住镐头，躯干前倾
  tick 6  follow      镐头再往下 4°，腕回弹一点
  tick 8  raise       回到起手位（与 tick 0 同值）
"""

from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-6, yaw=+4),
        torso=dict(pitch=+4, yaw=-6),
        rightArm=dict(pitch=-70, yaw=+6, roll=+8, bend=60, axis=180),
        leftArm=dict(pitch=+4, yaw=+4, roll=-4, bend=8, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-8, yaw=+2, bend=12, z=-0.05),
    ),

    3: dict(
        easing="INOUTSINE",
        body=dict(x=+0.01, y=-0.01, z=-0.01),
        head=dict(pitch=-8, yaw=+6),
        torso=dict(pitch=+2, yaw=-10),
        rightArm=dict(pitch=-120, yaw=+14, roll=+10, bend=40, axis=180),
        leftArm=dict(pitch=+6, yaw=+4, roll=-4, bend=10, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=12, z=+0.03),
        leftLeg=dict(pitch=-8, yaw=+2, bend=14, z=-0.05),
    ),

    5: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.04, z=+0.04),
        head=dict(pitch=-3, yaw=+2),
        torso=dict(pitch=+12, yaw=-4),
        rightArm=dict(pitch=-40, yaw=-2, roll=-8, bend=20, axis=180),
        leftArm=dict(pitch=+2, yaw=+4, roll=-4, bend=12, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+2, bend=18, z=-0.07),
    ),

    6: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.05, z=+0.04),
        head=dict(pitch=-2, yaw=+2),
        torso=dict(pitch=+13, yaw=-5),
        rightArm=dict(pitch=-36, yaw=-2, roll=-12, bend=24, axis=180),
        leftArm=dict(pitch=+3, yaw=+4, roll=-4, bend=12, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+2, bend=18, z=-0.07),
    ),

    8: dict(  # 与 tick 0 同值，循环闭合
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-6, yaw=+4),
        torso=dict(pitch=+4, yaw=-6),
        rightArm=dict(pitch=-70, yaw=+6, roll=+8, bend=60, axis=180),
        leftArm=dict(pitch=+4, yaw=+4, roll=-4, bend=8, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-8, yaw=+2, bend=12, z=-0.05),
    ),
}

DESCRIPTION = (
    "骨镐纯单手挥凿（循环）：8 tick。镐只在右手；左手自然垂放、只随身体轻摆。"
    "raise 镐在右前上方 → top 镐抬到头侧、手腕内收 → strike 镐啄向前下方、躯干前倾 → "
    "follow 镐头再压 4°、腕回弹 → 回起手位闭合循环。"
)

if __name__ == "__main__":
    emit_json(POSE, name="pickaxe_bone_use", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=True)
