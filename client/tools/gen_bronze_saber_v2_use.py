#!/usr/bin/env python3
"""bronze_saber_v2_use —— 青铜单刀（v2 模型）右肩起刀的斜劈（袈裟斩）。

只落资产，不接线：产出 player_animation/bronze_saber_v2_use.json，没有任何代码播放它。

和铁剑直劈的区别（远处要分得清）：
  - 刀从**右肩外上方**起，斜着劈到**左胯前**，轨迹是一条对角线，不是竖线；
  - 躯干扭转幅度大（+30° → -26°），靠腰带刀，手臂只负责走斜线；
  - 单刀刀身 32px、前段上弯，比剑长，收势时刀尖落得更低、更靠左。

模型事实：display 把刀身摆成顺着前臂延长（thirdperson rotation [-80, 90, 0]），
刀尖方向 ≈ 前臂方向。

节奏（10 tick / 0.5s，结构对齐 saber_slash_down）：
  tick 0  guard       刀横在右腰前，刀尖朝前
  tick 1  anticipation 躯干反拧、微后仰，右臂不动
  tick 3  windup      刀举到右肩外上方，右肩后拧到极限
  tick 5  strike      斜劈中途，躯干开始反拧
  tick 6  impact      刀尖落到左前下方，身体前冲
  tick 7  overshoot   刀尖再往左下走 10°
  tick 10 guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=+0.02, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=-6),
    torso=dict(pitch=+2, yaw=+12),
    rightArm=dict(pitch=-55, yaw=+8, roll=-8, bend=30, axis=180),
    leftArm=dict(pitch=-45, yaw=+20, roll=-14, bend=85, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=10, z=+0.04),
    leftLeg=dict(pitch=-12, yaw=+4, bend=14, z=-0.10),
)

POSE = {
    0: GUARD,
    1: dict(  # anticipation
        easing="INOUTSINE",
        body=dict(x=+0.04, y=-0.01, z=-0.02),
        head=dict(pitch=-6, yaw=-8),
        torso=dict(pitch=-2, yaw=+18),
        rightArm=dict(pitch=-60, yaw=+10, roll=-10, bend=30, axis=180),
        leftArm=dict(pitch=-43, yaw=+22, roll=-12, bend=80, axis=180),
        rightLeg=dict(pitch=+11, yaw=+4, bend=11, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=14, z=-0.10),
    ),
    3: dict(  # windup —— 刀到右肩外上方
        easing="INOUTSINE",
        body=dict(x=+0.06, y=-0.03, z=-0.05),
        head=dict(pitch=-8, yaw=-4),
        torso=dict(pitch=-5, yaw=+30),
        rightArm=dict(pitch=-150, yaw=+32, roll=-25, bend=28, axis=180),
        leftArm=dict(pitch=-40, yaw=+24, roll=-10, bend=72, axis=180),
        rightLeg=dict(pitch=+13, yaw=+4, bend=14, z=+0.05),
        leftLeg=dict(pitch=-10, yaw=+4, bend=12, z=-0.10),
    ),
    5: dict(  # strike
        easing="INQUAD",
        body=dict(x=0.0, y=+0.03, z=+0.10),
        head=dict(pitch=+4, yaw=+4),
        torso=dict(pitch=+8, yaw=-8),
        rightArm=dict(pitch=-100, yaw=-12, roll=+15, bend=12, axis=180),
        leftArm=dict(pitch=-50, yaw=-6, roll=+12, bend=105, axis=180),
        rightLeg=dict(pitch=+14, yaw=+3, bend=18, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+3, bend=22, z=-0.11),
    ),
    6: dict(  # impact —— 刀尖落左前下
        easing="OUTQUAD",
        body=dict(x=-0.05, y=+0.06, z=+0.16),
        head=dict(pitch=+8, yaw=+10),
        torso=dict(pitch=+12, yaw=-26),
        rightArm=dict(pitch=-60, yaw=-40, roll=+30, bend=6, axis=180),
        leftArm=dict(pitch=-55, yaw=-12, roll=+18, bend=115, axis=180),
        rightLeg=dict(pitch=+16, yaw=+2, bend=22, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+2, bend=28, z=-0.12),
    ),
    7: dict(  # overshoot
        easing="OUTQUAD",
        body=dict(x=-0.06, y=+0.05, z=+0.14),
        head=dict(pitch=+8, yaw=+11),
        torso=dict(pitch=+13, yaw=-28),
        rightArm=dict(pitch=-50, yaw=-48, roll=+34, bend=10, axis=180),
        leftArm=dict(pitch=-53, yaw=-10, roll=+20, bend=110, axis=180),
        rightLeg=dict(pitch=+15, yaw=+2, bend=21, z=+0.05),
        leftLeg=dict(pitch=-21, yaw=+2, bend=26, z=-0.12),
    ),
    10: GUARD,
}

DESCRIPTION = (
    "青铜单刀 v2 斜劈：guard 刀横右腰前 → windup 刀举右肩外上方、躯干右拧 +30° → "
    "impact 刀尖落左前下方、躯干反拧到 -26°（56° 扭矩）、身体前冲 0.16 → overshoot 再往左下 10° → 回 guard。"
    "左手 load-snap（bend 72°→115°）。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bronze_saber_v2_use", description=DESCRIPTION,
              end_tick=10, stop_tick=12, is_loop=False)
