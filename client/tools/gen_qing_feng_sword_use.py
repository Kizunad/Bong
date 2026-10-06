#!/usr/bin/env python3
"""qing_feng_sword_use —— 青锋剑单手横扫（右到左，身形灵巧）。

产出 player_animation/qing_feng_sword_use.json。服务端 vfx_animation_trigger.rs 的 held_attack_anim 表按 qing_feng_sword 选用它。

模型事实（决定了动作怎么设计）：
  - qing_feng_sword_v2 的手持 display 与 iron_sword_v2 完全相同（thirdperson rotation
    [-80, 90, 0]）。剑身顺着前臂延长，刃线（模型 x 轴）与刀面法线（z 轴）由右臂决定。
  - 动画里**不写任何 rightItem 关键帧**：剑按 display 自然握在手里。
  - 刃线要与手速平行，刀面法线要与手速垂直，否则是剑面拍下去。

横扫怎么做（和铁剑过顶直劈、骨剑斜劈都不同）：
  - 右臂 roll = -90°：手臂水平前伸时，这一转把剑变成「刃线横向、刀面朝上」。
  - 在这个姿态下，手臂的 pitch 摆动（抬手 / 放手）经 roll 转换后，手速是水平的，
    恰好沿刃线方向。所以横扫的主体是 pitch 从 -103° 摆到 -56°（肩高水平面内横切），
    肘 bend 基本伸直（0°~7°）。
  - 注意：不能靠 yaw 横扫。roll 在 yaw 之后作用，yaw 的水平速度会被 roll 转成竖直方向，
    刃线就对不上了。
  - 躯干只做小幅辅助（±8° 左右，同向转动），让身形跟着剑走。

判据（刃线 ∥ 手速，刀面 ⟂ 手速，挥击段 tick 3–5）：
  client/tools/check_blade_edge.py --strike 3-5 只算手臂，不含躯干；躯干的小幅转动由预览
  真实手持链路（含躯干）复核：握点与刃身中部的对齐都过线。见 model-review/anim/。

节奏（8 tick / 0.4s，非循环）：
  tick 0  guard       剑平举身前偏右，刃线横向
  tick 2  windup      剑收到身体右前方，躯干右转
  tick 4  cut         剑横切过身前，刃线随手速，躯干左转
  tick 5  overshoot   继续摆到左前方，肘微屈收住
  tick 8  guard       与 tick 0 同值，可连击衔接

左手：不持物，自然放松，随躯干反向摆动做平衡，不做怪姿势。
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=-4),
    torso=dict(pitch=0, yaw=+4),
    rightArm=dict(pitch=-100, yaw=0, roll=-90, bend=20, axis=180),
    leftArm=dict(pitch=0, yaw=+8, roll=-6, bend=25, axis=180),
    rightLeg=dict(pitch=+6, yaw=0, bend=8),
    leftLeg=dict(pitch=-8, yaw=0, bend=12),
)

POSE = {
    0: GUARD,
    2: dict(  # windup —— 剑收到身体右前方，躯干右转
        easing="INQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-2, yaw=+6),
        torso=dict(pitch=+2, yaw=+8),
        rightArm=dict(pitch=-103, yaw=0, roll=-90, bend=0, axis=180),
        leftArm=dict(pitch=+10, yaw=+10, roll=-6, bend=40, axis=180),
        rightLeg=dict(pitch=+10, yaw=0, bend=10),
        leftLeg=dict(pitch=-6, yaw=0, bend=10),
    ),
    4: dict(  # cut —— 剑横切过身前，刃线随手速，躯干左转
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=-8),
        torso=dict(pitch=+4, yaw=-8),
        rightArm=dict(pitch=-78, yaw=0, roll=-90, bend=0, axis=180),
        leftArm=dict(pitch=-6, yaw=-6, roll=-6, bend=35, axis=180),
        rightLeg=dict(pitch=+12, yaw=0, bend=12),
        leftLeg=dict(pitch=-12, yaw=0, bend=16),
    ),
    5: dict(  # overshoot —— 继续摆到左前方，肘微屈收住
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=+3, yaw=-10),
        torso=dict(pitch=+5, yaw=-12),
        rightArm=dict(pitch=-56, yaw=0, roll=-90, bend=7, axis=180),
        leftArm=dict(pitch=-4, yaw=-8, roll=-6, bend=40, axis=180),
        rightLeg=dict(pitch=+12, yaw=0, bend=12),
        leftLeg=dict(pitch=-12, yaw=0, bend=16),
    ),
    8: GUARD,
}

DESCRIPTION = (
    "青锋剑单手横扫（右到左）：8 tick。剑按 display 自然握持，无 rightItem。"
    "guard 剑平举身前偏右 → windup 剑收到右前方、躯干右转 → cut 剑横切过身前，刃线随手速 → "
    "overshoot 继续摆到左前方 → 回 guard。右臂 roll -90 把剑转成刃线横向、刀面朝上，"
    "靠 pitch 摆动横切（不靠 yaw）。与铁剑过顶直劈、骨剑斜劈区分。"
)

if __name__ == "__main__":
    emit_json(POSE, name="qing_feng_sword_use", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=False)
