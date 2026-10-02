#!/usr/bin/env python3
"""axe_iron_v2_use —— 生铁斧（v2 模型）双手过顶重劈。

只落资产，不接线：产出 player_animation/axe_iron_v2_use.json，没有任何代码播放它。

生铁斧斧头大而沉（柄长 27.7px，斧刃在最顶端），所以做成**双手**、**慢起快落**：
  - 14 tick（0.7s），是六件里最慢的；windup 占 5 tick，劈落只用 3 tick；
  - 左手上柄：两臂 pitch 同步，左臂 yaw 往内收到柄上；
  - 落斧后有 1 tick 「震手」停顿（tick 9→10 几乎不动），读得出斧头吃进木头；
  - 腿用 bend 下沉承重，leg.pitch 全程 ≤ 24°（§7.2 库坑）。

display 把斧柄摆成顺着前臂延长，斧头方向 ≈ 前臂方向。

节奏：
  tick 0  guard       斧斜提在身前右下
  tick 2  anticipation 重心下沉、躯干微后仰
  tick 5  windup      双手举斧过头，身体拔高后仰
  tick 8  impact      劈到前下方，躯干前压、下沉前冲
  tick 9  overshoot   斧头再下 10°
  tick 10 hold        震手停顿
  tick 14 guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-2, yaw=-4),
    torso=dict(pitch=+4, yaw=+8),
    rightArm=dict(pitch=-45, yaw=-16, roll=+6, bend=22, axis=180),
    leftArm=dict(pitch=-50, yaw=+32, roll=-12, bend=30, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-12, yaw=+4, bend=16, z=-0.10),
)

POSE = {
    0: GUARD,
    2: dict(  # anticipation
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.03, z=-0.02),
        head=dict(pitch=-6, yaw=-4),
        torso=dict(pitch=-2, yaw=+10),
        rightArm=dict(pitch=-60, yaw=-14, roll=+6, bend=22, axis=180),
        leftArm=dict(pitch=-62, yaw=+30, roll=-12, bend=30, axis=180),
        rightLeg=dict(pitch=+10, yaw=+4, bend=18, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=22, z=-0.10),
    ),
    5: dict(  # windup —— 双手举过头，身体拔高后仰
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.05, z=-0.06),
        head=dict(pitch=-14, yaw=-2),
        torso=dict(pitch=-12, yaw=+6),
        rightArm=dict(pitch=-172, yaw=-8, roll=+2, bend=16, axis=180),
        leftArm=dict(pitch=-165, yaw=+20, roll=-4, bend=22, axis=180),
        rightLeg=dict(pitch=+10, yaw=+4, bend=6, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=8, z=-0.10),
    ),
    8: dict(  # impact
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.14, z=+0.14),
        head=dict(pitch=+14, yaw=-2),
        torso=dict(pitch=+22, yaw=+2),
        rightArm=dict(pitch=-48, yaw=-12, roll=+4, bend=6, axis=180),
        leftArm=dict(pitch=-50, yaw=+26, roll=-8, bend=12, axis=180),
        rightLeg=dict(pitch=+18, yaw=+4, bend=34, z=+0.05),
        leftLeg=dict(pitch=-24, yaw=+4, bend=40, z=-0.12),
    ),
    9: dict(  # overshoot
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.15, z=+0.13),
        head=dict(pitch=+15, yaw=-2),
        torso=dict(pitch=+24, yaw=+2),
        rightArm=dict(pitch=-38, yaw=-12, roll=+4, bend=10, axis=180),
        leftArm=dict(pitch=-40, yaw=+26, roll=-8, bend=16, axis=180),
        rightLeg=dict(pitch=+18, yaw=+4, bend=36, z=+0.05),
        leftLeg=dict(pitch=-24, yaw=+4, bend=42, z=-0.12),
    ),
    10: dict(  # hold —— 斧头吃进木头的震手停顿
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.14, z=+0.13),
        head=dict(pitch=+14, yaw=-2),
        torso=dict(pitch=+23, yaw=+2),
        rightArm=dict(pitch=-40, yaw=-12, roll=+4, bend=9, axis=180),
        leftArm=dict(pitch=-42, yaw=+26, roll=-8, bend=15, axis=180),
        rightLeg=dict(pitch=+18, yaw=+4, bend=35, z=+0.05),
        leftLeg=dict(pitch=-24, yaw=+4, bend=41, z=-0.12),
    ),
    14: GUARD,
}

DESCRIPTION = (
    "生铁斧 v2 双手过顶重劈：guard 斧斜提右下 → windup 双手举过头(-172°/-165°)、身体拔高后仰 → "
    "impact 劈到前下方、躯干前压 +22°、下沉 0.14 前冲 0.14 → overshoot → 1 tick 震手停顿 → 回 guard。"
    "14 tick 慢起快落。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="axe_iron_v2_use", description=DESCRIPTION,
              end_tick=14, stop_tick=16, is_loop=False)
