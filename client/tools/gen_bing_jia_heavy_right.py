#!/usr/bin/env python3
"""bing_jia_heavy_right —— 兵甲手套（铁甲覆手）右重拳。

只落资产，不接线：产出 player_animation/bing_jia_heavy_right.json。

右重拳 vs 左重拳区别：
  - 右手是惯用手，**更深的腰转**：torso.yaw 差值 58°（左拳 50°）。
  - 落点更高（打对方头部/下颌），pitch=-96°，比左拳 -104° 略抬。
  - windup 更大幅：右臂提到 pitch=-158°（左臂 -148°），发力更长。
  - 体重前送更多：body.z 冲出 0.18m（左拳 0.14m）。
  - 全程 13 tick（左拳 12 tick），慢 1 tick 体现右手的充分蓄力。

节奏：
  tick 0  guard
  tick 2  load        躯干左拧、腰下沉
  tick 6  windup      右臂提到右耳上方，体重完全转到左腿
  tick 9  impact      铁甲重拳打对方头部，pitch -96°，躯干大幅反拧
  tick 10 hold        一帧停顿
  tick 11 overshoot   震手
  tick 13 guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=-0.02, y=0.0, z=+0.02),
    head=dict(pitch=-2, yaw=-4),
    torso=dict(pitch=+6, yaw=+12),
    # 右拳护右颊，铁甲内扣
    rightArm=dict(pitch=-52, yaw=-22, roll=+28, bend=100, axis=180),
    # 左拳前伸护架
    leftArm=dict(pitch=-72, yaw=+10, roll=-24, bend=84, axis=180),
    rightLeg=dict(pitch=+10, yaw=-4, bend=14, z=+0.04),
    leftLeg=dict(pitch=-16, yaw=+5, bend=20, z=-0.10),
)

POSE = {
    0: GUARD,

    2: dict(  # load：躯干左拧、腰下沉，右拳收到腰间
        easing="INOUTSINE",
        body=dict(x=-0.04, y=+0.04, z=0.0),
        head=dict(pitch=-1, yaw=+4),
        torso=dict(pitch=+8, yaw=+26),            # 左拧积矩
        rightArm=dict(pitch=-40, yaw=-28, roll=+32, bend=112, axis=180),
        leftArm=dict(pitch=-68, yaw=+8, roll=-22, bend=88, axis=180),
        rightLeg=dict(pitch=+14, yaw=-4, bend=24, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+5, bend=28, z=-0.11),
    ),

    6: dict(  # windup：右臂大幅提到右耳上方
        easing="INOUTSINE",
        body=dict(x=-0.08, y=-0.02, z=-0.04),
        head=dict(pitch=-6, yaw=+8),
        torso=dict(pitch=+2, yaw=+38),            # 左拧到极限
        rightArm=dict(pitch=-158, yaw=-28, roll=+22, bend=44, axis=180),
        leftArm=dict(pitch=-66, yaw=+10, roll=-22, bend=84, axis=180),
        rightLeg=dict(pitch=+10, yaw=-4, bend=10, z=+0.04),
        leftLeg=dict(pitch=-14, yaw=+4, bend=14, z=-0.08),
    ),

    9: dict(  # impact：铁甲右拳打头部，pitch -96°
        easing="OUTQUAD",
        body=dict(x=+0.05, y=+0.12, z=+0.18),
        head=dict(pitch=+8, yaw=-6),
        torso=dict(pitch=+16, yaw=-20),           # 反拧总量 58°（+38 → -20）
        rightArm=dict(pitch=-96, yaw=-6, roll=+8, bend=8, axis=180),
        leftArm=dict(pitch=-64, yaw=+8, roll=-24, bend=110, axis=180),
        rightLeg=dict(pitch=+14, yaw=-3, bend=32, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=40, z=-0.14),
    ),

    10: dict(  # hold：一帧停顿
        easing="OUTQUAD",
        body=dict(x=+0.05, y=+0.12, z=+0.18),
        head=dict(pitch=+8, yaw=-6),
        torso=dict(pitch=+16, yaw=-20),
        rightArm=dict(pitch=-96, yaw=-6, roll=+8, bend=8, axis=180),
        leftArm=dict(pitch=-64, yaw=+8, roll=-24, bend=110, axis=180),
        rightLeg=dict(pitch=+14, yaw=-3, bend=32, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=40, z=-0.14),
    ),

    11: dict(  # overshoot：震手
        easing="OUTQUAD",
        body=dict(x=+0.05, y=+0.13, z=+0.17),
        head=dict(pitch=+9, yaw=-6),
        torso=dict(pitch=+18, yaw=-24),
        rightArm=dict(pitch=-104, yaw=-6, roll=+6, bend=12, axis=180),
        leftArm=dict(pitch=-66, yaw=+8, roll=-24, bend=112, axis=180),
        rightLeg=dict(pitch=+14, yaw=-3, bend=34, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=42, z=-0.14),
    ),

    13: GUARD,
}

DESCRIPTION = (
    "兵甲手套铁甲右重拳（惯用手）：13 tick。"
    "guard 右拳护颊、左拳前伸护架 → load 躯干左拧 +26°、右拳收腰间 → "
    "windup 右臂提到右耳上(-158°)、左拧极限 +38° → "
    "impact 铁甲扣打头部 pitch=-96°、扭矩总量 58°、前冲 0.18m → "
    "hold 一帧停顿 → overshoot 震手 → 回 guard。"
    "比左重拳更长（13 vs 12 tick）、前冲更深（0.18 vs 0.14m）、扭矩更大（58 vs 50°）。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bing_jia_heavy_right", description=DESCRIPTION,
              end_tick=13, stop_tick=15, is_loop=False)
