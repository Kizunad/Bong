#!/usr/bin/env python3
"""herb_harvest — 凡铁采药刀俯身采割灵草动作（Blockbench 预览专用）。

这是一个**演示动画**，用于在 Blockbench 中预览玩家持刀采集的姿态。
不是游戏中的实战动画，因此不使用 guard pose 框架。

动作设计：
    - 玩家深蹲俯身，左手按住灵草基底，右手持采药刀勾割根茎
    - 强调稳定、精确、流畅，体现专业采药动作
    - 结构：动作姿态 → vanilla neutral (0,0,0)

时长：12 tick

阶段划分：
    tick 0  = 深蹲俯身姿态（左手探地，右手持刀抬起蓄势）
    tick 4  = 下刀勾割（刀刃贴地切入根茎）
    tick 8  = 提刃收势（刀尖向后拉割）
    tick 12 = 回到 vanilla neutral (完全放松站立)
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 运动学改进点：
# 1. 深蹲时双腿不对称（右腿承重深蹲，左腿前伸平衡）
# 2. 俯身时 torso + head 协同，避免颈椎过度折叠
# 3. 右臂持刀下刀时，torso 微扭配合，形成 kinetic chain
# 4. body.y 下沉要配合双腿 bend，避免"悬浮蹲"
# 5. 左臂探地时 pitch 不超 50°（避免 MC 无 IK 导致的断臂）

POSE = {
    # ========== 深蹲俯身姿态 ==========
    0: dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.24, z=+0.09),  # 重心下沉并前移
        head=dict(pitch=+32, yaw=-5),  # 头部低垂注视地面
        torso=dict(pitch=+28, yaw=+8),  # 躯干前倾
        # 右臂：持刀抬起蓄势（准备下刀）
        rightArm=dict(pitch=-42, yaw=+12, roll=-26, bend=58, axis=180),
        # 左臂：探地按住灵草基底（pitch 控制在 48° 避免断臂）
        leftArm=dict(pitch=+48, yaw=-16, roll=+22, bend=42, axis=180),
        # 右腿：深蹲承重（bend 38 配合 body.y 下沉）
        rightLeg=dict(pitch=-36, yaw=0, bend=38, axis=0),
        # 左腿：前伸辅助平衡
        leftLeg=dict(pitch=+16, yaw=0, bend=34, axis=0),
    ),

    # ========== 下刀勾割 ==========
    4: dict(
        easing="INOUTQUAD",
        body=dict(x=+0.02, y=-0.26, z=+0.11),  # 身体略前倾施力
        head=dict(pitch=+38, yaw=-3),  # 专注注视切割点
        torso=dict(pitch=+32, yaw=+12),  # 躯干微扭配合右臂
        # 右臂：刀刃贴地斜切入土（pitch +28° 向下，配合 torso 扭转）
        rightArm=dict(pitch=+28, yaw=+6, roll=-32, bend=36, axis=180),
        # 左臂：按紧植物稳定姿势（略微用力下压）
        leftArm=dict(pitch=+50, yaw=-14, roll=+26, bend=46, axis=180),
        # 双腿：保持深蹲（稳定下盘）
        rightLeg=dict(pitch=-38, yaw=0, bend=40, axis=0),
        leftLeg=dict(pitch=+18, yaw=0, bend=36, axis=0),
    ),

    # ========== 提刃收势 ==========
    8: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.23, z=+0.08),  # 重心回收
        head=dict(pitch=+34, yaw=-4),  # 头部开始抬起
        torso=dict(pitch=+26, yaw=+6),  # 躯干回正
        # 右臂：刀尖向后拉割（pitch 回到 +12°，提刃动作）
        rightArm=dict(pitch=+12, yaw=-12, roll=-18, bend=62, axis=180),
        # 左臂：松开植物，手掌离地
        leftArm=dict(pitch=+38, yaw=-18, roll=+18, bend=40, axis=180),
        # 双腿：开始起身（bend 减小）
        rightLeg=dict(pitch=-34, yaw=0, bend=36, axis=0),
        leftLeg=dict(pitch=+14, yaw=0, bend=34, axis=0),
    ),

    # ========== 回到 vanilla neutral ==========
    12: dict(
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
        name="herb_harvest",
        description="凡铁采药刀俯身专注采割灵草（Blockbench 预览）",
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
