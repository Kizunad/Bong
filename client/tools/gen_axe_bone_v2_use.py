#!/usr/bin/env python3
"""axe_bone_v2_use —— 骨斧（v2 模型）单手甩臂横砍。

只落资产，不接线：产出 player_animation/axe_bone_v2_use.json，没有任何代码播放它。

骨斧比生铁斧轻（骨刃透空、柄短一截），所以和生铁斧的双手过顶重劈**刻意做成不同形状**：
  - 单手、**水平**方向：斧从右后方甩到左前方，轨迹大致与地面平行（砍树干 / 砍腰）；
  - 10 tick，windup 只 2 tick，爆发靠躯干反拧（+34° → -30°，64° 扭矩）；
  - 左臂向前伸出瞄准，出斧时猛收到胸前（load-snap）。

display 把斧柄摆成顺着前臂延长，斧头方向 ≈ 前臂方向；手臂 pitch 保持在 -80°～-95°
（接近水平），斧头就在腰胸高度扫过。

节奏：
  tick 0  guard       斧提在右前，左手前伸
  tick 2  windup      斧甩到右后方，躯干右拧到极限
  tick 4  strike      横扫过身前
  tick 5  impact      斧头停在左前方，躯干反拧到底
  tick 6  overshoot   再往左 8°
  tick 10 guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=+0.02, y=0.0, z=0.0),
    head=dict(pitch=-2, yaw=-6),
    torso=dict(pitch=+2, yaw=+10),
    rightArm=dict(pitch=-70, yaw=+10, roll=-6, bend=30, axis=180),
    leftArm=dict(pitch=-70, yaw=+14, roll=-8, bend=35, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=16, z=-0.10),
)

POSE = {
    0: GUARD,
    2: dict(  # windup —— 甩到右后方
        easing="INOUTSINE",
        body=dict(x=+0.06, y=+0.02, z=-0.04),
        head=dict(pitch=-2, yaw=-12),
        torso=dict(pitch=0, yaw=+34),
        rightArm=dict(pitch=-82, yaw=+55, roll=-10, bend=26, axis=180),
        leftArm=dict(pitch=-78, yaw=+6, roll=-6, bend=25, axis=180),
        rightLeg=dict(pitch=+12, yaw=+4, bend=16, z=+0.05),
        leftLeg=dict(pitch=-12, yaw=+4, bend=14, z=-0.10),
    ),
    4: dict(  # strike —— 横扫过身前
        easing="INQUAD",
        body=dict(x=0.0, y=+0.04, z=+0.08),
        head=dict(pitch=0, yaw=+2),
        torso=dict(pitch=+4, yaw=-4),
        rightArm=dict(pitch=-90, yaw=0, roll=+4, bend=10, axis=180),
        leftArm=dict(pitch=-55, yaw=+18, roll=-16, bend=90, axis=180),
        rightLeg=dict(pitch=+14, yaw=+3, bend=18, z=+0.05),
        leftLeg=dict(pitch=-18, yaw=+3, bend=22, z=-0.11),
    ),
    5: dict(  # impact —— 停在左前方
        easing="OUTQUAD",
        body=dict(x=-0.06, y=+0.05, z=+0.12),
        head=dict(pitch=+2, yaw=+12),
        torso=dict(pitch=+6, yaw=-30),
        rightArm=dict(pitch=-88, yaw=-38, roll=+10, bend=6, axis=180),
        leftArm=dict(pitch=-45, yaw=+22, roll=-20, bend=118, axis=180),
        rightLeg=dict(pitch=+16, yaw=+2, bend=20, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+2, bend=26, z=-0.12),
    ),
    6: dict(  # overshoot
        easing="OUTQUAD",
        body=dict(x=-0.07, y=+0.05, z=+0.11),
        head=dict(pitch=+2, yaw=+13),
        torso=dict(pitch=+6, yaw=-32),
        rightArm=dict(pitch=-86, yaw=-46, roll=+12, bend=12, axis=180),
        leftArm=dict(pitch=-47, yaw=+21, roll=-19, bend=112, axis=180),
        rightLeg=dict(pitch=+15, yaw=+2, bend=19, z=+0.05),
        leftLeg=dict(pitch=-21, yaw=+2, bend=24, z=-0.12),
    ),
    10: GUARD,
}

DESCRIPTION = (
    "骨斧 v2 单手横砍：guard 斧提右前、左手前伸 → windup 斧甩右后方、躯干右拧 +34° → "
    "impact 斧头横扫停在左前方、躯干反拧 -30°（64° 扭矩）→ overshoot 再往左 8° → 回 guard。"
    "与生铁斧的双手过顶劈刻意区分。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="axe_bone_v2_use", description=DESCRIPTION,
              end_tick=10, stop_tick=12, is_loop=False)
