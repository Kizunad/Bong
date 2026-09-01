#!/usr/bin/env python3
"""herb_knife_slash — 凡铁采药刀短快反手割击动作（Blockbench 预览专用）。

这是一个**演示动画**，用于在 Blockbench 中预览玩家持刀战斗的姿态。
不是游戏中的实战动画，因此不使用 guard pose 框架。

动作设计：
    - 小幅度迅速反手割划，适合采药刀短刃的特点
    - 从侧腰蓄势，向前斜向划出
    - 强调速度和流畅性，不是重型武器的大开大合
    - 结构：动作姿态 → vanilla neutral (0,0,0)

时长：9 tick（比采集动作更快）

阶段划分：
    tick 0 = 反手起手姿态（刀拉至侧腰）
    tick 2 = 蓄势拉至极限（身体扭转，重心下沉）
    tick 5 = 迅捷割击（impact frame，刀尖划出）
    tick 9 = 回到 vanilla neutral (完全放松站立)
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 运动学改进点：
# 1. 蓄势阶段 torso yaw 向右扭转（与 impact 方向相反，形成扭转势能）
# 2. Impact 时 torso 反向扭转 + body.z 前冲，形成 kinetic chain
# 3. 右臂 pitch 从负值（后拉）到正值（前划），形成完整弧线
# 4. 双腿配合重心转移（蓄势时右腿 bend 增加，impact 时左腿 pitch 增加）
# 5. 左臂配合平衡（蓄势时收紧，impact 时展开）

POSE = {
    # ========== 反手起手姿态 ==========
    0: dict(
        easing="OUTSINE",
        body=dict(x=+0.02, y=-0.02, z=0.0),
        head=dict(pitch=-2, yaw=-4),
        torso=dict(pitch=+4, yaw=+10),  # 身体略右转
        # 右臂：反手微拉至侧腰
        rightArm=dict(pitch=-36, yaw=-22, roll=+26, bend=36, axis=180),
        # 左臂：自然摆放
        leftArm=dict(pitch=+16, yaw=+16, roll=-12, bend=16, axis=180),
        # 双腿：略微下蹲准备
        rightLeg=dict(pitch=-8, yaw=+2, bend=10, axis=0),
        leftLeg=dict(pitch=+6, yaw=+2, bend=8, axis=0),
    ),

    # ========== 蓄势拉至极限 ==========
    2: dict(
        easing="INQUAD",
        body=dict(x=+0.04, y=-0.03, z=-0.04),  # 重心后移下沉
        head=dict(pitch=-4, yaw=-3),
        torso=dict(pitch=+2, yaw=+18),  # 躯干右转到极限（蓄势）
        # 右臂：刀拉至侧腰极限位置
        rightArm=dict(pitch=-58, yaw=-28, roll=+32, bend=52, axis=180),
        # 左臂：收紧配合平衡
        leftArm=dict(pitch=+22, yaw=+22, roll=-16, bend=22, axis=180),
        # 右腿：承重下蹲
        rightLeg=dict(pitch=-14, yaw=+2, bend=14, axis=0),
        leftLeg=dict(pitch=+10, yaw=+2, bend=12, axis=0),
    ),

    # ========== 迅捷割击（Impact Frame）==========
    5: dict(
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.02, z=+0.14),  # 重心前冲
        head=dict(pitch=+6, yaw=+8),
        torso=dict(pitch=+12, yaw=-16),  # 躯干左转（反向释放扭转势能）
        # 右臂：刀尖向前斜向划出（pitch 正值，形成完整弧线）
        rightArm=dict(pitch=+32, yaw=+18, roll=-22, bend=18, axis=180),
        # 左臂：后展配合平衡
        leftArm=dict(pitch=-16, yaw=-14, roll=+16, bend=26, axis=180),
        # 双腿：重心前移（左腿前伸）
        rightLeg=dict(pitch=-6, yaw=+2, bend=20, axis=0),
        leftLeg=dict(pitch=+18, yaw=+2, bend=6, axis=0),
    ),

    # ========== 回到 vanilla neutral ==========
    9: dict(
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
        name="herb_knife_slash",
        description="凡铁采药刀短快反手割击（Blockbench 预览）",
        end_tick=9,
        stop_tick=11,
        is_loop=False,
    )
