#!/usr/bin/env python3
"""spirit_sword_use —— 灵剑单手前刺（重心前送，剑身朝前直出）。

只落资产，不接线：产出 player_animation/spirit_sword_use.json，没有任何代码播放它。

模型事实（决定了动作怎么设计）：
  - spirit_sword_v2 的手持 display 与 iron_sword_v2 完全相同（thirdperson rotation
    [-80, 90, 0]）。剑身顺着前臂延长，手臂 roll 0 时刃口竖直、剑身水平朝前。
  - 护手较宽（沿刃口方向展开约 0.5 格），前刺时护手竖着摆，不会挡住视线。
  - 动画里**不写任何 rightItem 关键帧**：剑按 display 自然握持。

前刺为什么这样做（和铁剑、骨剑的挥砍都不同）：
  挥砍看的是刃线横扫；前刺看的是剑尖沿剑身方向直出。这里手臂 roll 保持 0，
  剑身随前臂水平朝前（pitch -90 时剑身朝 -Z）。
    - tick 2 收势：肘大幅屈起（bend 95°），前臂收到胸前，剑尖斜指前上方蓄力。
    - tick 4 出刺：肘伸直（bend 4°），肩 pitch 从 -62° 送到 -90°，手臂水平前伸；
      躯干前倾 +10°，把重心一起送出去。剑尖向前推出约 1 格。
    - tick 5 停住：肩再压一点（-92°），躯干前倾 +12°，剑尖停在最远点。
    - 左手放松，随躯干反向后摆做平衡（leftArm pitch 随躯干变化），不做怪姿势。
  注意：刃线判据（刃线 ∥ 手速）不适用于突刺。突刺是剑尖沿剑身方向直线推出，
  判据改为：tick 2–4 剑尖前送 ≥ 1 格，且 tick 4 时剑身朝前（剑身轴 · 朝前 ≥ 0.85）。
  上述两条由 tip 位移与剑身朝向复核，见 model-review/anim/spirit_sword_use.md。

节奏（8 tick / 0.4s，非循环）：
  tick 0  guard       剑在胸前斜指前上方
  tick 2  draw        剑收到胸前，肘屈起蓄力
  tick 4  thrust      肘伸直、肩送出、躯干前倾，剑尖直刺
  tick 5  hold        停在最远点，躯干再压一点
  tick 8  guard       与 tick 0 同值，可连刺衔接
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=-2),
    torso=dict(pitch=+2, yaw=+4),
    rightArm=dict(pitch=-72, yaw=+4, roll=0, bend=62, axis=180),
    leftArm=dict(pitch=0, yaw=+6, roll=-6, bend=25, axis=180),
    rightLeg=dict(pitch=+10, yaw=0, bend=10),
    leftLeg=dict(pitch=-12, yaw=0, bend=16),
)

POSE = {
    0: GUARD,
    2: dict(  # draw —— 剑收到胸前，肘屈起蓄力，躯干微后仰
        easing="INQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=0),
        torso=dict(pitch=-2, yaw=+3),
        rightArm=dict(pitch=-62, yaw=+3, roll=0, bend=95, axis=180),
        leftArm=dict(pitch=+6, yaw=+10, roll=-6, bend=30, axis=180),
        rightLeg=dict(pitch=+12, yaw=0, bend=12),
        leftLeg=dict(pitch=-10, yaw=0, bend=14),
    ),
    4: dict(  # thrust —— 肘伸直，肩送出，躯干前倾，剑尖直刺
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=+10, yaw=-3),
        rightArm=dict(pitch=-90, yaw=0, roll=0, bend=4, axis=180),
        leftArm=dict(pitch=+12, yaw=+8, roll=-6, bend=35, axis=180),
        rightLeg=dict(pitch=+16, yaw=0, bend=14),
        leftLeg=dict(pitch=-18, yaw=0, bend=22),
    ),
    5: dict(  # hold —— 停在最远点，躯干再压一点
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=+3, yaw=-2),
        torso=dict(pitch=+12, yaw=-3),
        rightArm=dict(pitch=-92, yaw=0, roll=0, bend=2, axis=180),
        leftArm=dict(pitch=+12, yaw=+8, roll=-6, bend=35, axis=180),
        rightLeg=dict(pitch=+16, yaw=0, bend=14),
        leftLeg=dict(pitch=-18, yaw=0, bend=22),
    ),
    8: GUARD,
}

DESCRIPTION = (
    "灵剑单手前刺（重心前送）：8 tick。剑按 display 自然握持，无 rightItem。"
    "guard 剑斜指胸前 → draw 肘屈起蓄力 → thrust 肘伸直、肩送出、躯干前倾直刺 → "
    "hold 停在最远点 → 回 guard。剑身 roll 0 朝前，判据看剑尖前送，不看刃线。"
    "与铁剑、骨剑的横扫与劈砍区分。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="spirit_sword_use", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=False)
