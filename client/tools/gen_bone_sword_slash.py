#!/usr/bin/env python3
"""bone_sword_slash —— 骨剑单手竖劈（刃口朝下、朝挥击方向）。

只落资产，不接线：产出 player_animation/bone_sword_slash.json。

动作：剑从头顶后上方（windup）劈下，挥击瞬间手臂水平前伸（impact），
剑沿手臂朝前伸出，刃口在最低处朝下、随手臂前下方的运动切入。

几何（bone_sword_v2 的 OBJ 与 BoneSwordV2.bbmodel 一致）：
  - 刃线（两侧刃口的连线）沿模型 x 轴，刀面法线沿 z 轴，刃长沿 y 轴；
  - 挥击瞬间手速方向 = 垂直向下，刃线必须与之平行 → 刃口朝下；
  - 刀面法线与手速垂直 → 不是剑面拍下去。
rightItem 三个角由 client/tools/check_blade_edge.py 的判据求解（方法见
model-review/anim/bone_sword_slash.md），整段保持不变：剑是手臂的一段，挥击由手臂完成。

单手：剑只在右手，左手自然放松。
节奏（9 tick，非循环）：
  tick 0  guard     剑在右前下方，剑沿手臂朝前
  tick 3  windup    手臂举过头顶后上方
  tick 6  impact    手臂水平前伸，手速垂直向下，刃口朝下（判据瞬间）
  tick 7  overshoot 手臂继续下压到前下方
  tick 9  guard     与 tick 0 同值
"""

from anim_common import emit_json

# 剑相对手臂的固定握持角（度）。挥击瞬间刃线与手速对齐的解，见 md 的求解记录。
_HOLD_ITEM = dict(pitch=50, yaw=-60, roll=-80)

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+6),
        torso=dict(pitch=+4, yaw=-10),
        rightArm=dict(pitch=-60, yaw=+10, roll=+10, bend=60, axis=180),
        leftArm=dict(pitch=+2, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+3, bend=14, z=-0.06),
        rightItem=_HOLD_ITEM,
    ),

    3: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=-0.02),
        head=dict(pitch=-6, yaw=+8),
        torso=dict(pitch=-4, yaw=+6),
        rightArm=dict(pitch=-150, yaw=+15, roll=+5, bend=45, axis=180),
        leftArm=dict(pitch=-10, yaw=+4, roll=-6, bend=14, axis=180),
        rightLeg=dict(pitch=+8, yaw=-2, bend=12, z=+0.03),
        leftLeg=dict(pitch=-6, yaw=+3, bend=12, z=-0.06),
        rightItem=_HOLD_ITEM,
    ),

    6: dict(  # 挥击瞬间：手臂水平前伸，手速垂直向下，刃口朝下
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.02, z=+0.04),
        head=dict(pitch=-2, yaw=-2),
        torso=dict(pitch=+8, yaw=-12),
        rightArm=dict(pitch=-90, yaw=0, roll=0, bend=15, axis=180),
        leftArm=dict(pitch=+6, yaw=+2, roll=-6, bend=16, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.04),
        leftLeg=dict(pitch=-16, yaw=+4, bend=22, z=-0.09),
        rightItem=_HOLD_ITEM,
    ),

    7: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.03, z=+0.05),
        head=dict(pitch=-2, yaw=-4),
        torso=dict(pitch=+9, yaw=-16),
        rightArm=dict(pitch=-80, yaw=-4, roll=0, bend=10, axis=180),
        leftArm=dict(pitch=+10, yaw=+2, roll=-6, bend=18, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=20, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+4, bend=30, z=-0.11),
        rightItem=_HOLD_ITEM,
    ),

    9: dict(  # 与 tick 0 同值
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+6),
        torso=dict(pitch=+4, yaw=-10),
        rightArm=dict(pitch=-60, yaw=+10, roll=+10, bend=60, axis=180),
        leftArm=dict(pitch=+2, yaw=+6, roll=-6, bend=12, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+3, bend=14, z=-0.06),
        rightItem=_HOLD_ITEM,
    ),
}

DESCRIPTION = (
    "骨剑单手竖劈（刃口朝下、朝挥击方向）：9 tick。剑只在右手，左手自然放松。"
    "guard 剑沿手臂朝前下方 → windup 手臂举过头顶 → impact 手臂水平前伸，手速垂直向下，刃口朝下 → "
    "overshoot 手臂继续下压 → 回守势。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bone_sword_slash", description=DESCRIPTION,
              end_tick=9, stop_tick=11, is_loop=False)
