#!/usr/bin/env python3
"""hand_wrap_jab_right —— 缠手（布缠）右直拳（轻拳，快、碎）。

只落资产，不接线：产出 player_animation/hand_wrap_jab_right.json。
左式见 gen_hand_wrap_jab_left.py：由本文件的右式镜像得到，左右两式完全对称。

缠手是轻拳，设计要点（对照兵甲重拳的「沉」）：
  - **护脸更紧**：guard 两拳都贴在脸侧，肘折满（bend 125°+），拳面离脸很近。
  - **出拳短**：手臂不伸直，落点 bend≈20°，只有肩到拳约 8/10 的伸展。
  - **手腕翻转明显**：impact 时右腕 roll 从 +34° 翻到 -24°（约 58°），靠手腕的甩，不靠大臂。
  - **身体几乎不晃**：躯干 yaw 只在 -8°~+3° 之间，body 位移 ≤ 0.04 格。
  - **快收**：impact 后 2 tick 即回 guard，左右两式之间间隔短（8 tick 一个循环）。

节奏（右拳，8 tick）：
  tick 0  guard       两拳贴脸，右拳 roll +30°，左拳同样贴脸
  tick 1  anticipation 右肘微收，腕先外旋（roll +34°），躯干几乎不动
  tick 3  snap        右拳出，左拳同步收紧（反相 load-snap）
  tick 5  impact      右拳短距离打出，腕翻到 roll -24°，躯干微拧 -8°
  tick 6  overshoot   腕再翻 6°，拳面过冲 2°
  tick 7  recover     右臂回拉，腕回中
  tick 8  guard       回守，与 tick 0 同值
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-6, yaw=-4),       # 下颌微收，护脸
    torso=dict(pitch=+2, yaw=-4),
    # 右拳贴右脸，肘折满，腕外旋 roll +30°
    rightArm=dict(pitch=-78, yaw=-12, roll=+30, bend=128, axis=180),
    # 左拳贴左脸（守势手，与右拳对称）
    leftArm=dict(pitch=-76, yaw=+14, roll=-26, bend=126, axis=180),
    rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
    leftLeg=dict(pitch=-8, yaw=+2, bend=14, z=-0.05),
)

POSE = {
    0: GUARD,

    1: dict(  # anticipation：右肘微收，腕先外旋；躯干几乎不动
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=-0.01),
        head=dict(pitch=-6, yaw=-4),
        torso=dict(pitch=+2, yaw=-2),
        rightArm=dict(pitch=-74, yaw=-12, roll=+34, bend=132, axis=180),
        leftArm=dict(pitch=-76, yaw=+14, roll=-26, bend=126, axis=180),
        rightLeg=dict(pitch=+6, yaw=-2, bend=10, z=+0.03),
        leftLeg=dict(pitch=-8, yaw=+2, bend=14, z=-0.05),
    ),

    3: dict(  # snap：右拳出，左拳同步收紧（反相 load-snap）
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=+0.01),
        head=dict(pitch=-5, yaw=-4),
        torso=dict(pitch=+3, yaw=-4),
        rightArm=dict(pitch=-88, yaw=-6, roll=+6, bend=20, axis=180),
        leftArm=dict(pitch=-80, yaw=+14, roll=-26, bend=132, axis=180),
        rightLeg=dict(pitch=+8, yaw=-2, bend=12, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+2, bend=16, z=-0.06),
    ),

    5: dict(  # impact：短距离打出，腕翻到位
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=+0.04),
        head=dict(pitch=-3, yaw=-6),
        torso=dict(pitch=+3, yaw=-8),
        rightArm=dict(pitch=-94, yaw=-6, roll=-24, bend=26, axis=180),
        leftArm=dict(pitch=-78, yaw=+14, roll=-26, bend=130, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.05),
        leftLeg=dict(pitch=-12, yaw=+2, bend=20, z=-0.08),
    ),

    6: dict(  # overshoot：腕再翻 6°，拳面过冲 2°
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=+0.04),
        head=dict(pitch=-3, yaw=-6),
        torso=dict(pitch=+3, yaw=-8),
        rightArm=dict(pitch=-94, yaw=-6, roll=-30, bend=16, axis=180),
        leftArm=dict(pitch=-78, yaw=+14, roll=-26, bend=130, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.05),
        leftLeg=dict(pitch=-12, yaw=+2, bend=20, z=-0.08),
    ),

    7: dict(  # recover：右臂回拉，腕回中
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=+0.01),
        head=dict(pitch=-5, yaw=-4),
        torso=dict(pitch=+2, yaw=-4),
        rightArm=dict(pitch=-82, yaw=-10, roll=-6, bend=62, axis=180),
        leftArm=dict(pitch=-77, yaw=+14, roll=-26, bend=128, axis=180),
        rightLeg=dict(pitch=+7, yaw=-2, bend=12, z=+0.03),
        leftLeg=dict(pitch=-9, yaw=+2, bend=15, z=-0.06),
    ),

    8: GUARD,
}

DESCRIPTION = (
    "缠手布缠右直拳（轻拳）：8 tick，快、碎。"
    "guard 两拳贴脸、肘折满 → anticipation 右腕外旋 +34° → snap 右拳出、左拳同步收紧 → "
    "impact 短距离打出（肘收 26°，不伸直），腕翻 roll +34°→-24°，躯干只拧 -8° → overshoot 腕再翻 6° → "
    "recover 右臂回拉 → 回 guard。"
    "身体几乎不晃，靠手腕的快翻出拳；左式由本式镜像，左右间隔短。"
)

if __name__ == "__main__":
    emit_json(POSE, name="hand_wrap_jab_right", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=False)
