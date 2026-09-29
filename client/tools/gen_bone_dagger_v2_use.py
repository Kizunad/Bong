#!/usr/bin/env python3
"""bone_dagger_v2_use —— 兽骨刀（v2 模型）贴身直刺。

只落资产，不接线：产出 player_animation/bone_dagger_v2_use.json，没有任何代码播放它。

动作要点：
  - 短、快：8 tick（0.4s），比剑刀的 10 tick 快一截，远处一眼能分出「捅」和「砍」；
  - 刀从腰前**直线**送出，不画弧：右臂只走 pitch（-40° → -86°）+ bend（75° → 12°）；
  - 送到**自然的一臂距离**就停：身体只前冲 0.10，肘留 12° 不锁死。Round 1 版肘伸到 4°、
    前冲 0.20，22px 的獠牙刃戳出去太远，人工闸门判「伸出距离过长」；肘弯再大（试过 24°）
    刀尖会翘成朝上挑，不再是刺；
  - 蓄势全交给躯干（右肩后拧 +26°）和重心后坐，右臂在 anticipation 里不反向（§2.3）；
  - 兽骨刀是大弯獠牙刃，刃长 22px，display 顺前臂延长，所以送出时刀尖正对前方。

节奏：
  tick 0  guard     刀收在右腰前（前臂上折，刀尖朝前上），左手护胸
  tick 2  chamber   躯干右拧后坐，右臂原样
  tick 4  impact    右臂送出一臂距离，躯干反拧、身体前冲 0.10
  tick 5  overshoot 手腕再送 3°，肘微回弹
  tick 8  guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=+0.02, y=0.0, z=0.0),
    head=dict(pitch=-2, yaw=-8),
    torso=dict(pitch=+4, yaw=+12),
    rightArm=dict(pitch=-40, yaw=-6, roll=+6, bend=75, axis=180),
    leftArm=dict(pitch=-55, yaw=+24, roll=-18, bend=95, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=18, z=-0.10),
)

POSE = {
    0: GUARD,
    2: dict(  # chamber —— 只拧躯干、后坐
        easing="INOUTSINE",
        body=dict(x=+0.05, y=+0.02, z=-0.05),
        head=dict(pitch=-3, yaw=-12),
        torso=dict(pitch=+2, yaw=+26),
        rightArm=dict(pitch=-42, yaw=-6, roll=+6, bend=78, axis=180),
        leftArm=dict(pitch=-52, yaw=+26, roll=-16, bend=88, axis=180),
        rightLeg=dict(pitch=+12, yaw=+4, bend=16, z=+0.05),
        leftLeg=dict(pitch=-12, yaw=+4, bend=16, z=-0.10),
    ),
    4: dict(  # impact —— 送出一臂距离
        easing="OUTQUAD",
        body=dict(x=-0.03, y=+0.02, z=+0.10),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+6, yaw=-18),
        rightArm=dict(pitch=-86, yaw=-10, roll=+4, bend=12, axis=180),
        leftArm=dict(pitch=-45, yaw=+18, roll=-20, bend=118, axis=180),
        rightLeg=dict(pitch=+14, yaw=+3, bend=14, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=26, z=-0.12),
    ),
    5: dict(  # overshoot
        easing="OUTQUAD",
        body=dict(x=-0.03, y=+0.02, z=+0.09),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+6, yaw=-20),
        rightArm=dict(pitch=-88, yaw=-10, roll=+2, bend=15, axis=180),
        leftArm=dict(pitch=-47, yaw=+19, roll=-19, bend=112, axis=180),
        rightLeg=dict(pitch=+13, yaw=+3, bend=14, z=+0.05),
        leftLeg=dict(pitch=-19, yaw=+3, bend=24, z=-0.12),
    ),
    8: GUARD,
}

DESCRIPTION = (
    "兽骨刀 v2 贴身直刺：guard 刀收右腰前 → chamber 只拧躯干后坐 → "
    "impact 右臂送出一臂距离(pitch -86° 肘留 12°)、躯干反拧 44°、前冲 0.10 → overshoot 再送 3° → 回 guard。"
    "8 tick，比剑刀快。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bone_dagger_v2_use", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=False)
