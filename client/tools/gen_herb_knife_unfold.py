#!/usr/bin/env python3
"""herb_knife_unfold — 凡铁折叠采药刀甩腕展开出刀动作（Blockbench 预览专用）。

这是一个**演示动画**，用于在 Blockbench 中预览玩家"取出折叠刀并展开"的姿态。
不是游戏中的实战动画，因此不使用 guard pose 框架。

动作设计：
    - 从腰侧取出折叠的采药刀
    - 单手甩腕，利用惯性让刀刃弹出锁定
    - 强调手腕翻转的流畅性和刀刃弹出的清脆感
    - 结构：动作姿态 → vanilla neutral (0,0,0)

时长：8 tick（短促有力）

阶段划分：
    tick 0 = 取刀姿态（右手从腰侧取出折叠刀）
    tick 4 = 甩腕亮刃（手腕外翻，刀刃弹出锁定）
    tick 8 = 回到 vanilla neutral (完全放松站立)
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 运动学改进点：
# 1. 取刀姿态：右臂从腰侧取刀，手腕内扣持握折叠刀
# 2. 甩腕动作：手腕快速外翻（roll 变化），配合 pitch 抬升
# 3. 刀刃弹出：利用惯性，刀刃在 tick 4 完全展开
# 4. 头部跟随：注视刀刃展开的过程
# 5. 左臂平衡：自然摆放，不过度参与

POSE = {
    # ========== 取刀姿态 ==========
    0: dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=+10, yaw=-6),  # 低头看腰侧的刀
        torso=dict(pitch=+4, yaw=+8),  # 身体略右转
        # 右臂：从腰侧取出折叠刀（手腕内扣，持握折叠状态）
        # pitch=-45° 手在腰腹高度，yaw=+15° 微外展取刀
        # roll=-30° 手腕内扣持握，bend=65° 肘弯曲
        rightArm=dict(pitch=-45, yaw=+15, roll=-30, bend=65, axis=180),
        # 左臂：自然垂放
        leftArm=dict(pitch=+10, yaw=-10, roll=+10, bend=20, axis=180),
        # 双腿：略微下蹲（取刀时重心下沉）
        rightLeg=dict(pitch=-5, yaw=0, bend=5, axis=0),
        leftLeg=dict(pitch=+5, yaw=0, bend=5, axis=0),
    ),

    # ========== 甩腕亮刃 ==========
    4: dict(
        easing="OUTQUAD",
        body=dict(x=+0.01, y=0.0, z=+0.02),  # 身体略前倾
        head=dict(pitch=+5, yaw=+2),  # 头部抬起，注视刀刃
        torso=dict(pitch=+2, yaw=-5),  # 躯干微左转配合
        # 右臂：甩腕展开（手腕外翻，刀刃弹出）
        # pitch=-15° 手抬至胸前，yaw=-20° 微内收
        # roll=+25° 手腕外翻到位，bend=20° 肘几乎伸直（甩腕发力）
        rightArm=dict(pitch=-15, yaw=-20, roll=+25, bend=20, axis=180),
        # 左臂：略微抬起平衡
        leftArm=dict(pitch=+15, yaw=-12, roll=+12, bend=25, axis=180),
        # 双腿：起身（重心回升）
        rightLeg=dict(pitch=-4, yaw=0, bend=4, axis=0),
        leftLeg=dict(pitch=+4, yaw=0, bend=4, axis=0),
    ),

    # ========== 回到 vanilla neutral ==========
    8: dict(
        easing="INOUTSINE",
        # 完全放松的站立姿态
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=180),
        leftArm=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=180),
        rightLeg=dict(pitch=0.0, yaw=0.0, bend=0.0, axis=0),
        leftLeg=dict(pitch=0.0, yaw=0.0, bend=0.0, axis=0),
    ),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="herb_knife_unfold",
        description="凡铁采药刀甩腕亮刃展开（Blockbench 预览）",
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
