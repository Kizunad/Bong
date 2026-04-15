#!/usr/bin/env python3
"""dodge_back — 后跃闪避。

身法节奏（8 tick / 0.4s）：
  tick 0 guard    neutral
  tick 2 CROUCH   蹲蓄 (双腿 bend=42° + body.y -0.22 + torso +16° 前倾)
  tick 4 AIRBORNE 腾空最高点 (body.y +0.28, z=-0.50 后退, 双腿蜷 bend=65°, 躯干仰 -12°)
  tick 6 LAND     落地缓冲 (body.y -0.10 下沉吸震, 双腿 bend=30°, z=-0.35 仍后)
  tick 8 归位     guard

反僵硬要点：
  - body.y / body.z 是视觉主体 (conventions §2 规则 5 身体位移主导)
  - 蹲 → 腾 → 落 的三段高度变化 (-0.22 → +0.28 → -0.10 = 0.5 格幅度)
  - 双臂和腿反相位: 蹲时臂微前抱，腾空时臂后展摆动维持平衡
  - tick 2→4 body.z 从 +0.05 向前蹲蓄 → -0.50 猛退, 这个"先压后弹"是关键
  - 落地 torso 前倾 +8° 表示缓冲吸震 + head pitch +5° 补位
  - axis=180° 手臂折向前 (腾空时像抱胸收缩, 不是张开)
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    2: dict(  # CROUCH —— 蹲蓄
        easing="OUTQUAD",
        body=dict(y=-0.22, z=+0.05),
        head=dict(pitch=+8),  # 低头蓄
        torso=dict(pitch=+16),
        rightArm=dict(pitch=-20, yaw=-6, bend=40, axis=180),
        leftArm=dict(pitch=-20, yaw=+6, bend=40, axis=180),
        rightLeg=dict(pitch=+18, bend=42),
        leftLeg=dict(pitch=+18, bend=42),
    ),
    4: dict(  # AIRBORNE —— 腾空
        easing="OUTQUAD",
        body=dict(y=+0.28, z=-0.50),
        head=dict(pitch=-6),  # 仰头
        torso=dict(pitch=-12),
        rightArm=dict(pitch=-35, yaw=-12, bend=55, axis=180),
        leftArm=dict(pitch=-35, yaw=+12, bend=55, axis=180),
        rightLeg=dict(pitch=-28, bend=65),
        leftLeg=dict(pitch=-28, bend=65),
    ),
    6: dict(  # LAND —— 落地吸震
        easing="OUTQUAD",
        body=dict(y=-0.10, z=-0.35),
        head=dict(pitch=+5),
        torso=dict(pitch=+8),
        rightArm=dict(pitch=-20, yaw=-6, bend=40, axis=180),
        leftArm=dict(pitch=-20, yaw=+6, bend=40, axis=180),
        rightLeg=dict(pitch=+8, bend=30),
        leftLeg=dict(pitch=+8, bend=30),
    ),
    8: dict(  # 归位
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
}

DESCRIPTION = (
    "v1 JSON 后跃: 8 tick 三段式 蹲 → 腾 → 落, body.y 变化 0.5 格 (-0.22→+0.28→-0.10), "
    "body.z 后退 0.55 格 (+0.05→-0.50), 双腿 bend 42°→65°→30° 蜷缩-伸展-吸震, "
    "双臂反相位配合, torso pitch +16°→-12°→+8° 模拟身体压弹吸。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dodge_back",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=11,
        is_loop=False,
    )
