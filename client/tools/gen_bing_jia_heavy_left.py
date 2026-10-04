#!/usr/bin/env python3
"""bing_jia_heavy_left —— 兵甲手套（铁甲覆手）左重拳。

只落资产，不接线：产出 player_animation/bing_jia_heavy_left.json。

兵甲手套特征：铁片覆整个手背、骨节加固，重量约 3 倍于布缠。
设计出发点（与 hand_wrap_jab_left 刻意对比）：
  - **慢起**：windup 要 5 tick，布缠 windup 只 3 tick。
  - **腰胯带动**：torso.yaw 带动扭矩 50°（布缠 28°），还有 body.x 侧倾。
  - **落点沉**：左拳的 pitch 落到 -104°（手背朝下扣打），比布缠的 -88° 俯角更深。
  - **收势余震**：impact→hold 1 tick，再 overshoot 另一帧，共 3 帧消化冲击，
                  比布缠的 1 帧 overshoot 多一步。
  - 全程 12 tick，布缠 8 tick。

节奏：
  tick 0  guard       左拳提在左肩前，微微前伸（护架）
  tick 2  load        躯干右拧、腰下沉蓄势，左拳收回腰间
  tick 5  windup      左臂提到头侧，体重转换完成
  tick 8  impact      铁甲重拳扣打，pitch -104°，躯干反拧、前冲、下沉
  tick 9  hold        一帧停顿：手停在落点，躯干完成反转
  tick 10 overshoot   震手：手再下沉 8°，躯干多转 6°
  tick 12 guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=+0.02, y=0.0, z=+0.02),
    head=dict(pitch=-2, yaw=+4),
    torso=dict(pitch=+6, yaw=-10),
    # 左拳前伸护架，手腕内扣（铁甲重量让手腕更内旋）
    leftArm=dict(pitch=-75, yaw=+12, roll=-28, bend=80, axis=180),
    rightArm=dict(pitch=-55, yaw=-16, roll=+24, bend=96, axis=180),
    leftLeg=dict(pitch=-16, yaw=+6, bend=20, z=-0.10),
    rightLeg=dict(pitch=+10, yaw=+4, bend=14, z=+0.04),
)

POSE = {
    0: GUARD,

    2: dict(  # load：躯干右拧、腰下沉，左拳收回腰间
        easing="INOUTSINE",
        body=dict(x=+0.04, y=+0.04, z=0.0),   # 腰下沉
        head=dict(pitch=-1, yaw=-2),
        torso=dict(pitch=+8, yaw=-24),           # 开始右拧
        leftArm=dict(pitch=-42, yaw=+18, roll=-32, bend=108, axis=180),  # 收回腰间
        rightArm=dict(pitch=-60, yaw=-14, roll=+22, bend=92, axis=180),
        leftLeg=dict(pitch=-18, yaw=+6, bend=28, z=-0.11),   # 弯膝下沉
        rightLeg=dict(pitch=+12, yaw=+4, bend=20, z=+0.05),
    ),

    5: dict(  # windup：左臂提到头侧，体重转换完成
        easing="INOUTSINE",
        body=dict(x=+0.06, y=-0.02, z=-0.04),  # 重心右移拔高
        head=dict(pitch=-6, yaw=-8),
        torso=dict(pitch=+2, yaw=-32),            # 右拧到极限
        leftArm=dict(pitch=-148, yaw=+24, roll=-22, bend=48, axis=180),  # 大幅提臂
        rightArm=dict(pitch=-58, yaw=-14, roll=+20, bend=90, axis=180),
        leftLeg=dict(pitch=-14, yaw=+5, bend=14, z=-0.08),   # 重心起身
        rightLeg=dict(pitch=+10, yaw=+4, bend=10, z=+0.04),
    ),

    8: dict(  # impact：铁甲重拳扣打，pitch -104°（手背朝下）
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.12, z=+0.14),  # 前冲+下沉
        head=dict(pitch=+8, yaw=+8),
        torso=dict(pitch=+16, yaw=+18),           # 反拧完成（总量 50°）
        leftArm=dict(pitch=-104, yaw=+2, roll=-6, bend=10, axis=180),
        rightArm=dict(pitch=-62, yaw=-16, roll=+26, bend=108, axis=180),  # 护拳猛收
        leftLeg=dict(pitch=-22, yaw=+5, bend=38, z=-0.13),
        rightLeg=dict(pitch=+14, yaw=+4, bend=30, z=+0.06),
    ),

    9: dict(  # hold：一帧停顿，手停在落点
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.12, z=+0.14),
        head=dict(pitch=+8, yaw=+8),
        torso=dict(pitch=+16, yaw=+18),
        leftArm=dict(pitch=-104, yaw=+2, roll=-6, bend=10, axis=180),
        rightArm=dict(pitch=-62, yaw=-16, roll=+26, bend=108, axis=180),
        leftLeg=dict(pitch=-22, yaw=+5, bend=38, z=-0.13),
        rightLeg=dict(pitch=+14, yaw=+4, bend=30, z=+0.06),
    ),

    10: dict(  # overshoot：震手，手再下 8°，躯干多转 6°
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.13, z=+0.13),
        head=dict(pitch=+9, yaw=+8),
        torso=dict(pitch=+18, yaw=+22),
        leftArm=dict(pitch=-112, yaw=+3, roll=-4, bend=14, axis=180),
        rightArm=dict(pitch=-64, yaw=-16, roll=+26, bend=110, axis=180),
        leftLeg=dict(pitch=-22, yaw=+5, bend=40, z=-0.13),
        rightLeg=dict(pitch=+14, yaw=+4, bend=32, z=+0.06),
    ),

    12: GUARD,
}

DESCRIPTION = (
    "兵甲手套铁甲左重拳：12 tick，比缠手左直拳长 4 tick。"
    "guard 左拳前伸护架 → load 躯干右拧 -24°、腰下沉、左拳收腰间 → "
    "windup 左臂大幅提到头侧(-148°)、右拧到 -32° → "
    "impact 铁甲重拳扣打 pitch=-104°（手背朝下）、躯干反拧总量 50°、前冲 0.14 下沉 0.12 → "
    "hold 一帧停顿（与缠手对比：多此帧）→ overshoot 震手 8° → 回 guard。"
    "识别特征：腰胯带动、落点深、余震明显，与布缠轻拳的快收形成鲜明对比。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bing_jia_heavy_left", description=DESCRIPTION,
              end_tick=12, stop_tick=14, is_loop=False)
