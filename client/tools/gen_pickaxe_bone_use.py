#!/usr/bin/env python3
"""pickaxe_bone_use —— 骨镐（bone pickaxe）单手轻凿。

只落资产，不接线：产出 player_animation/pickaxe_bone_use.json。

骨镐特征：骨制，比铁镐轻约 40%，柄短，刃头小。
设计要点（与 pickaxe_iron_v2_use 刻意对比）：
  - **单手**：右手持镐凿，左臂只辅助平衡（不像铁镐的双手握柄）。
  - **快凿**：8 tick 一周期，铁镐 14 tick；动作幅度也小。
  - **小幅**：右臂 windup 只提到头侧 pitch=-110°（铁镐 -150° 过顶），
              不借重力向下砸，改为手腕主导的「啄木鸟式」抖腕。
  - **抖腕标志**：impact 帧 rightArm.roll 从 windup +15° 拧到 -25°（手腕内翻），
                   加速落凿；铁镐以 pitch 为主无明显 roll 变化。
  - **循环**：is_loop=True，每个 axis 在 endTick 补同值帧（约定）。

节奏（单次，循环播放 = 连续凿）：
  tick 0  guard       右臂举在右侧偏上、左臂辅助前伸
  tick 3  windup      右臂提到头侧 pitch=-110°，腕内旋 roll=+15
  tick 5  impact      右臂啄下 pitch=-40°（手腕甩）、roll=-25，躯干微前倾
  tick 6  overshoot   再下 8°，腕再翻 -5°
  tick 8  guard       ← endTick，与 tick 0 同值，满足循环闭合
"""

from anim_common import emit_json

# tick 0 和 tick 8 完全相同，手动写出两份（emit_json 要求明确的 endTick 帧）
_GUARD_VALS = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=+0.02),
    head=dict(pitch=-6, yaw=+4),
    torso=dict(pitch=+6, yaw=-8),
    rightArm=dict(pitch=-90, yaw=+8, roll=+10, bend=50, axis=180),
    leftArm=dict(pitch=-60, yaw=+12, roll=-14, bend=68, axis=180),
    rightLeg=dict(pitch=+8, yaw=+3, bend=12, z=+0.03),
    leftLeg=dict(pitch=-12, yaw=+4, bend=16, z=-0.07),
)

POSE = {
    0: _GUARD_VALS,

    3: dict(  # windup：右臂提到头侧，腕内旋
        easing="INOUTSINE",
        body=dict(x=+0.02, y=-0.01, z=0.0),
        head=dict(pitch=-4, yaw=+2),
        torso=dict(pitch=+4, yaw=-12),
        rightArm=dict(pitch=-110, yaw=+12, roll=+15, bend=30, axis=180),
        leftArm=dict(pitch=-56, yaw=+14, roll=-12, bend=72, axis=180),
        rightLeg=dict(pitch=+8, yaw=+3, bend=11, z=+0.03),
        leftLeg=dict(pitch=-12, yaw=+3, bend=15, z=-0.07),
    ),

    5: dict(  # impact：手腕甩，啄下 pitch=-40°，roll=-25（内翻）
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.06, z=+0.10),
        head=dict(pitch=-2, yaw=+2),
        torso=dict(pitch=+12, yaw=-6),
        rightArm=dict(pitch=-40, yaw=+6, roll=-25, bend=20, axis=180),
        leftArm=dict(pitch=-58, yaw=+10, roll=-14, bend=70, axis=180),
        rightLeg=dict(pitch=+10, yaw=+3, bend=15, z=+0.04),
        leftLeg=dict(pitch=-14, yaw=+3, bend=20, z=-0.09),
    ),

    6: dict(  # overshoot：再啄 8°，腕再翻 -5°
        easing="OUTQUAD",
        body=dict(x=+0.01, y=+0.07, z=+0.10),
        head=dict(pitch=-1, yaw=+2),
        torso=dict(pitch=+13, yaw=-5),
        rightArm=dict(pitch=-48, yaw=+6, roll=-30, bend=22, axis=180),
        leftArm=dict(pitch=-58, yaw=+10, roll=-14, bend=70, axis=180),
        rightLeg=dict(pitch=+10, yaw=+3, bend=16, z=+0.04),
        leftLeg=dict(pitch=-14, yaw=+3, bend=21, z=-0.09),
    ),

    8: _GUARD_VALS,  # endTick 帧 = tick 0，闭合循环
}

DESCRIPTION = (
    "骨镐单手轻凿循环：8 tick/凿，is_loop=True。"
    "guard 右臂举在右侧偏上、单手持柄 → windup 提到头侧 pitch=-110°、腕内旋 roll=+15° → "
    "impact 啄下 pitch=-40°、腕内翻 roll=-25°（手腕甩劲，非大臂砸重力）→ "
    "overshoot 再下 8° → 回 guard。"
    "与 pickaxe_iron_v2_use 对比：单手 vs 双手、8 tick vs 14 tick、"
    "pitch-110°(头侧) vs -150°(过顶)、抖腕 vs 重力大劈。"
)

if __name__ == "__main__":
    emit_json(POSE, name="pickaxe_bone_use", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=True)
