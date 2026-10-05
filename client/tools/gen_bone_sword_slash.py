#!/usr/bin/env python3
"""bone_sword_slash —— 骨剑单手斜劈（右上过头 → 左前下，刃口朝挥击方向）。

只落资产，不接线：产出 player_animation/bone_sword_slash.json，没有任何代码播放它。

模型事实（决定了动作怎么设计）：
  - bone_sword_v2 的手持 display 与 iron_sword_v2 完全相同（thirdperson rotation
    [-80, 90, 0]），剑身顺着前臂延长。所以剑的指向与刃口朝向都由右臂决定。
  - 动画里**不写任何 rightItem 关键帧**：剑按 display 自然握在手里，和通过审阅的
    iron_sword_v2_use / bronze_saber_v2_use 一致。挥砍只靠右臂 + 躯干完成。
  - 刃线（两侧刃口的连线）在挥击瞬间必须与手速平行，刀面法线与手速垂直，否则是剑面
    拍下去。这一条由 client/tools/check_blade_edge.py 逐帧核对。

挥击为什么是斜劈而不是铁剑的过顶直劈：
  铁剑靠矢状面内的 pitch 挥动（举过头顶劈下），刃线竖直。骨剑的右臂在 windup 时
  向外侧倾约 74°（roll），挥击平面随之倾斜：剑从右上方过头，斜着切向左前下方，
  刃线随挥动方向转动。roll 与 pitch 的组合由 scratch 随机搜索求得（挥击段刃线 |cos|
  最小 0.987，刀面 |cos| 最大 0.014）。

节奏（10 tick / 0.5s，非循环）：
  tick 0  guard       剑在右前方，手臂斜下
  tick 3  windup      右臂过头向右外侧举起（pitch -172°，roll +74°），躯干后仰、右肩后拧
  tick 5  strike      斜劈起势（pitch -95°），躯干开始左转
  tick 6  impact      剑尖停在左前下方（pitch -40°），躯干前压左转，刃线与手速对齐
  tick 7  overshoot   继续下压一点（pitch -35°），躯干再转
  tick 10 guard       与 tick 0 同值，可连劈
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=-6),
    torso=dict(pitch=0, yaw=+10),
    rightArm=dict(pitch=-70, yaw=+20, roll=0, bend=25, axis=180),
    leftArm=dict(pitch=-30, yaw=+14, roll=-8, bend=40, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=10, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=16, z=-0.10),
)

POSE = {
    0: GUARD,
    3: dict(  # windup —— 右臂过头向右外侧举起，躯干后仰、右肩后拧
        easing="INOUTSINE",
        body=dict(x=+0.02, y=-0.03, z=-0.04),
        head=dict(pitch=-10, yaw=+6),
        torso=dict(pitch=-6, yaw=+16),
        rightArm=dict(pitch=-172, yaw=-9, roll=+74, bend=22, axis=180),
        leftArm=dict(pitch=-40, yaw=+18, roll=-8, bend=50, axis=180),
        rightLeg=dict(pitch=+12, yaw=+4, bend=10, z=+0.05),
        leftLeg=dict(pitch=-12, yaw=+4, bend=12, z=-0.10),
    ),
    5: dict(  # strike —— 斜劈起势，躯干开始左转
        easing="INQUAD",
        body=dict(x=0.0, y=+0.02, z=+0.06),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=+4, yaw=-4),
        rightArm=dict(pitch=-95, yaw=-5, roll=+82, bend=9, axis=180),
        leftArm=dict(pitch=-30, yaw=+10, roll=-10, bend=60, axis=180),
        rightLeg=dict(pitch=+14, yaw=+4, bend=14, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+4, bend=24, z=-0.11),
    ),
    6: dict(  # impact —— 剑尖停在左前下方，身体前压左转
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.05, z=+0.12),
        head=dict(pitch=+6, yaw=-4),
        torso=dict(pitch=+10, yaw=-12),
        rightArm=dict(pitch=-40, yaw=-3, roll=+66, bend=16, axis=180),
        leftArm=dict(pitch=-20, yaw=+8, roll=-12, bend=80, axis=180),
        rightLeg=dict(pitch=+16, yaw=+4, bend=18, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+4, bend=30, z=-0.12),
    ),
    7: dict(  # overshoot —— 继续下压一点，躯干再转
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.04, z=+0.10),
        head=dict(pitch=+7, yaw=-6),
        torso=dict(pitch=+11, yaw=-16),
        rightArm=dict(pitch=-35, yaw=-3, roll=+66, bend=16, axis=180),
        leftArm=dict(pitch=-18, yaw=+8, roll=-12, bend=84, axis=180),
        rightLeg=dict(pitch=+15, yaw=+4, bend=17, z=+0.05),
        leftLeg=dict(pitch=-21, yaw=+4, bend=28, z=-0.12),
    ),
    10: GUARD,
}

DESCRIPTION = (
    "骨剑单手斜劈（右上过头 → 左前下）：10 tick。剑按 display 自然握持，无 rightItem。"
    "guard 剑在右前方 → windup 右臂过头向右外侧举起（roll +74°）→ "
    "strike 斜劈起势 → impact 剑尖停左前下方，刃线与手速对齐 → overshoot 再下压 → 回 guard。"
    "与铁剑过顶直劈（矢状面挥动、roll 约 +6°）区分。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bone_sword_slash", description=DESCRIPTION,
              end_tick=10, stop_tick=12, is_loop=False)
