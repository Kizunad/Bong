#!/usr/bin/env python3
"""hand_wrap_jab_left —— 缠手（布缠）左直拳。

只落资产，不接线：产出 player_animation/hand_wrap_jab_left.json。

缠手质量：布缠 + 无护具，拳头轻、前臂灵活。
设计出发点：
  - **快** ：全程 8 tick（0.4s），比铁甲重拳短 4 tick。
  - **短** ：左拳伸出幅度比铁甲拳小，body 前冲只 0.08m（铁甲 0.18m）。
  - **腕翻** ：impact 帧 leftArm.roll 从 -20° 拧到 +15°，手腕向内旋，
              读作「缠布缠紧的前臂在出拳时自然翻腕」。
  - **肩不大转** ：torso.yaw 差值 28°（铁甲 50°+），肩不借躯干大扭。
  - **收得快** ：impact→guard 只用 2 tick（overshoot 在 tick 6 同时收尾）。

节奏：
  tick 0  guard       左拳抬在左肩前，右拳护右颊
  tick 1  anticipation 躯干微右倾蓄势（只 torso/head 做反向）
  tick 3  windup      左臂收回胸前 chambered
  tick 5  impact      左拳打到对方下颌高度，腕翻到位
  tick 6  overshoot   拳头再前 8°，腕翻超出 5°
  tick 8  guard       收回（= tick 0）

循环约定：is_loop=False，连击从外部循环交替调用左/右。
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=+0.02, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=+6),
    torso=dict(pitch=+4, yaw=-12),
    # 左拳在左肩前方、手腕中立（roll=-20 让缠布纹理朝外）
    leftArm=dict(pitch=-62, yaw=+8, roll=-20, bend=95, axis=180),
    # 右拳护右颊
    rightArm=dict(pitch=-52, yaw=-14, roll=+22, bend=100, axis=180),
    leftLeg=dict(pitch=-14, yaw=+5, bend=18, z=-0.08),
    rightLeg=dict(pitch=+8, yaw=+4, bend=12, z=+0.04),
)

POSE = {
    0: GUARD,

    1: dict(  # anticipation：只 torso/head 反向微右扭，左臂不反向
        easing="INOUTSINE",
        body=dict(x=+0.03, y=+0.01, z=-0.01),
        head=dict(pitch=-4, yaw=+2),
        torso=dict(pitch=+3, yaw=-6),  # 比 guard 小，微收蓄势
        leftArm=dict(pitch=-62, yaw=+8, roll=-20, bend=95, axis=180),  # 不动
        rightArm=dict(pitch=-52, yaw=-14, roll=+22, bend=100, axis=180),
        leftLeg=dict(pitch=-14, yaw=+5, bend=18, z=-0.08),
        rightLeg=dict(pitch=+8, yaw=+4, bend=12, z=+0.04),
    ),

    3: dict(  # windup：左拳收到胸前 chambered
        easing="INOUTSINE",
        body=dict(x=+0.04, y=+0.01, z=-0.02),
        head=dict(pitch=-3, yaw=-4),
        torso=dict(pitch=+5, yaw=-18),  # 躯干右拧到极限
        leftArm=dict(pitch=-52, yaw=+18, roll=-28, bend=115, axis=180),
        rightArm=dict(pitch=-55, yaw=-12, roll=+18, bend=95, axis=180),  # 护手微展
        leftLeg=dict(pitch=-16, yaw=+4, bend=20, z=-0.09),
        rightLeg=dict(pitch=+9, yaw=+4, bend=14, z=+0.05),
    ),

    5: dict(  # impact：左拳打出，腕翻到位
        easing="OUTQUAD",
        body=dict(x=-0.02, y=0.0, z=+0.08),
        head=dict(pitch=-2, yaw=-8),
        torso=dict(pitch=+3, yaw=+10),   # 躯干反弹到左侧（差 28° 扭矩）
        # 左臂接近伸直，bend=6 保留 bendy-lib 弹性，腕翻 roll=+15
        leftArm=dict(pitch=-88, yaw=-2, roll=+15, bend=6, axis=180),
        rightArm=dict(pitch=-58, yaw=-14, roll=+22, bend=110, axis=180),  # 护手猛收
        leftLeg=dict(pitch=-18, yaw=+4, bend=26, z=-0.10),
        rightLeg=dict(pitch=+10, yaw=+4, bend=16, z=+0.05),
    ),

    6: dict(  # overshoot：拳头再前 8°，腕翻超出 5°
        easing="OUTQUAD",
        body=dict(x=-0.03, y=0.0, z=+0.08),
        head=dict(pitch=-2, yaw=-9),
        torso=dict(pitch=+3, yaw=+12),
        leftArm=dict(pitch=-96, yaw=-3, roll=+20, bend=4, axis=180),
        rightArm=dict(pitch=-60, yaw=-14, roll=+22, bend=112, axis=180),
        leftLeg=dict(pitch=-18, yaw=+4, bend=27, z=-0.10),
        rightLeg=dict(pitch=+10, yaw=+4, bend=17, z=+0.05),
    ),

    8: GUARD,  # 回 guard，连击友好
}

DESCRIPTION = (
    "缠手布缠左直拳：8 tick 快出快收。guard 左拳抬肩前 → windup 收到胸前、躯干右拧 -18° → "
    "impact 左臂伸出 pitch=-88°、腕翻 roll 从 -20° 拧到 +15°、躯干反弹扭矩 28° → "
    "overshoot 再前 8° 腕翻超 5° → 2 tick 急收回 guard。"
    "体重前送 0.08m（轻于铁甲），肩不大转，手腕翻转是识别特征。"
)

if __name__ == "__main__":
    emit_json(POSE, name="hand_wrap_jab_left", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=False)
