#!/usr/bin/env python3
"""harvest_crouch — 采药刀专属蹲伏采药动作。

设计：循环动画，玩家蹲下采药的持续姿态。
右手持采药刀向地面探取，左手辅助稳定。

循环动画特点：
  - isLoop=true
  - 每个用到的轴在 endTick 补同值关键帧（§7.1 防单帧衰减）
  - 微小呼吸感：body.y 略微起伏

帧点：
  tick 0 = 蹲伏姿态（身体压低，右手向地）
  tick 10 = 微微起身（呼吸感）
  tick 20 = 回到 tick 0（循环点）
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 单位：角度用度数（emit 时转弧度），xyz 用米
POSE_V1 = {
    # ═══ 蹲伏采药姿态（循环起点）═══
    0: dict(
        easing="INOUTSINE",
        # 身体压低 0.32m（蹲伏）
        body=dict(x=0.0, y=+0.32, z=+0.05),
        head=dict(pitch=+15, yaw=0, roll=0),
        # torso 前倾看地面
        torso=dict(pitch=+26, yaw=0, roll=0),
        # 右臂：持刀向下探取，肘弯
        rightArm=dict(pitch=-65, yaw=-15, roll=+8, bend=95, axis=180),
        # 左臂：辅助平衡，手按膝盖
        leftArm=dict(pitch=+35, yaw=+25, roll=-10, bend=60, axis=180),
        # 右腿：蹲伏主力腿，膝盖大弯
        rightLeg=dict(pitch=+25, yaw=+5, bend=75, z=+0.08),
        # 左腿：前腿支撑
        leftLeg=dict(pitch=-20, yaw=+5, bend=65, z=-0.18),
    ),

    # ═══ 微微起身（呼吸感）═══
    10: dict(
        easing="INOUTSINE",
        # 身体略微抬高（呼吸起伏）
        body=dict(x=0.0, y=+0.28, z=+0.05),
        head=dict(pitch=+14, yaw=0, roll=0),
        torso=dict(pitch=+24, yaw=0, roll=0),
        # 右臂：略微上提
        rightArm=dict(pitch=-62, yaw=-15, roll=+8, bend=92, axis=180),
        # 左臂：微调
        leftArm=dict(pitch=+33, yaw=+25, roll=-10, bend=58, axis=180),
        # 腿部：膝盖略微伸展
        rightLeg=dict(pitch=+23, yaw=+5, bend=72, z=+0.08),
        leftLeg=dict(pitch=-19, yaw=+5, bend=62, z=-0.18),
    ),

    # ═══ 回到蹲伏（循环点，必须 = tick 0）═══
    20: dict(
        easing="INOUTSINE",
        # 完全回到 tick 0 的值（§7.1 循环动画单帧衰减规则）
        body=dict(x=0.0, y=+0.32, z=+0.05),
        head=dict(pitch=+15, yaw=0, roll=0),
        torso=dict(pitch=+26, yaw=0, roll=0),
        rightArm=dict(pitch=-65, yaw=-15, roll=+8, bend=95, axis=180),
        leftArm=dict(pitch=+35, yaw=+25, roll=-10, bend=60, axis=180),
        rightLeg=dict(pitch=+25, yaw=+5, bend=75, z=+0.08),
        leftLeg=dict(pitch=-20, yaw=+5, bend=65, z=-0.18),
    ),
}

DESCRIPTION_V1 = (
    "采药刀蹲伏采药：循环动画（isLoop=true），"
    "身体压低 +0.32m 蹲伏姿态，"
    "右手持刀向地探取（pitch -65° / bend 95°），"
    "左手按膝辅助平衡，"
    "微小呼吸感（body.y 0.32 ↔ 0.28m 起伏），"
    "tick 0 == tick 20（§7.1 防单帧衰减）。"
)

if __name__ == "__main__":
    emit_json(
        POSE_V1,
        name="harvest_crouch",
        description=DESCRIPTION_V1,
        end_tick=20,
        stop_tick=22,
        is_loop=True,
    )
