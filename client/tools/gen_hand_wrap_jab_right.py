#!/usr/bin/env python3
"""hand_wrap_jab_right —— 缠手（布缠）右直拳。

只落资产，不接线：产出 player_animation/hand_wrap_jab_right.json。

与 hand_wrap_jab_left 的主要区别：
  - 右手是惯用手，**后手重拳**：右拳打出时腰髋带动更多，torso.yaw 差值
    约 38°（左拳 28°），借到腰力。
  - 右拳起始位置在右颊护位（类似拳击「右钩」），而非左拳的肩前提拳。
  - 右臂伸直时 yaw 偏左（穿过身体中线打向对方），左拳 jab 基本沿自身轴线。
  - 收势：右臂收回比左臂慢 1 tick，体现后手回弹的惯性。
  - 同为 9 tick，比左拳的 8 tick 略慢，节奏感区分。

节奏：
  tick 0  guard       右拳护右颊，左拳提在肩前
  tick 2  windup      躯干左拧积攒扭矩，右拳收到右耳旁 chambered
  tick 4  strike      右拳开始穿出，腰髋开始反转
  tick 6  impact      右拳打向对方胸口高度（略低于左拳 jab 的下颌），腕翻到位
  tick 7  overshoot   再前 8°，扭矩略超
  tick 9  guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=-0.02, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=-6),
    torso=dict(pitch=+4, yaw=+14),   # 惯用手侧躯干偏右
    # 右拳护右颊，手腕中立偏外旋
    rightArm=dict(pitch=-52, yaw=-20, roll=+26, bend=105, axis=180),
    # 左拳提在肩前，前手
    leftArm=dict(pitch=-64, yaw=+10, roll=-22, bend=88, axis=180),
    rightLeg=dict(pitch=+8, yaw=-4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=18, z=-0.08),
)

POSE = {
    0: GUARD,

    2: dict(  # windup：躯干左拧，右拳收到右耳旁
        easing="INOUTSINE",
        body=dict(x=-0.04, y=+0.01, z=-0.03),
        head=dict(pitch=-3, yaw=+6),
        torso=dict(pitch=+5, yaw=+26),   # 右拧更深，积攒扭矩
        rightArm=dict(pitch=-44, yaw=-30, roll=+30, bend=120, axis=180),
        leftArm=dict(pitch=-60, yaw=+8, roll=-20, bend=90, axis=180),
        rightLeg=dict(pitch=+9, yaw=-4, bend=14, z=+0.05),
        leftLeg=dict(pitch=-16, yaw=+4, bend=20, z=-0.09),
    ),

    4: dict(  # strike：右拳开始出，腰髋开始反转
        easing="INQUAD",
        body=dict(x=-0.01, y=0.0, z=+0.04),
        head=dict(pitch=-2, yaw=+2),
        torso=dict(pitch=+4, yaw=+10),   # 正在反转中
        rightArm=dict(pitch=-72, yaw=-14, roll=+20, bend=60, axis=180),
        leftArm=dict(pitch=-64, yaw=+12, roll=-24, bend=96, axis=180),
        rightLeg=dict(pitch=+10, yaw=-3, bend=16, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+3, bend=22, z=-0.10),
    ),

    6: dict(  # impact：右拳打到对方胸口，腕翻到位（roll 从 +26 → -10）
        easing="OUTQUAD",
        body=dict(x=+0.03, y=0.0, z=+0.10),
        head=dict(pitch=-2, yaw=-8),
        torso=dict(pitch=+3, yaw=-12),   # 反弹总扭矩 38°（26→-12）
        # 右臂几乎伸直，穿过中线偏左（yaw=+14）
        rightArm=dict(pitch=-90, yaw=+14, roll=-10, bend=8, axis=180),
        # 左拳猛收紧护位
        leftArm=dict(pitch=-58, yaw=+8, roll=-24, bend=112, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=18, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=28, z=-0.11),
    ),

    7: dict(  # overshoot：再前 8°，扭矩略超
        easing="OUTQUAD",
        body=dict(x=+0.04, y=0.0, z=+0.10),
        head=dict(pitch=-2, yaw=-9),
        torso=dict(pitch=+3, yaw=-16),
        rightArm=dict(pitch=-98, yaw=+16, roll=-14, bend=6, axis=180),
        leftArm=dict(pitch=-60, yaw=+8, roll=-24, bend=114, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=19, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=29, z=-0.11),
    ),

    9: GUARD,   # 收回略慢（9 tick vs 左拳 8 tick），体现后手惯性
}

DESCRIPTION = (
    "缠手布缠右直拳（后手重拳）：9 tick，比左直拳慢 1 tick。"
    "guard 右拳护右颊 → windup 躯干右拧积矩 +26°、右拳收到右耳旁 → "
    "strike 腰髋开始反转 → impact 右拳穿过中线打向对方胸口、腕翻 roll +26°→-10°、扭矩总量 38° → "
    "overshoot 再前 8° → 回 guard。"
    "借腰比左直拳多（扭矩 38° vs 28°），落点略低（胸口 vs 下颌），穿中线区分左右手。"
)

if __name__ == "__main__":
    emit_json(POSE, name="hand_wrap_jab_right", description=DESCRIPTION,
              end_tick=9, stop_tick=11, is_loop=False)
