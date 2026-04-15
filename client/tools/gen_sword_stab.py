#!/usr/bin/env python3
"""sword_stab — 单手剑直刺。

身法节奏（8 tick / 0.4s）：
  tick 0 guard   Fencer's stance 侧身 (torso yaw +20°), 剑斜前
  tick 3 LOAD    剑收腰 (pitch=-20° bend=95°) + 身体后坐 (body.z -0.08) + 更侧身
  tick 5 IMPACT  **body.z +0.28 前扑!** + 直臂前刺 (pitch=-88° bend=3°) + 转正
  tick 6 over    再刺一寸 (body.z +0.32)
  tick 8 回      guard

反僵硬要点：
  - 直刺的**视觉主体是 body.z**，不是手臂伸直 —— 0.36 格的前后位移让观众感到"扑"
  - torso yaw 从 +20° → +30° → -6° (36° 转正扭矩) 让"出剑"是全身工程
  - 前弓步强烈 (左腿 pitch=-28° bend=42° = 深弓) 对应前冲
  - 左臂 counter-pull 猛抬 (pitch=-40° bend=85°) 像剑术真实的"双刃平衡"
  - axis=180° 让 LOAD 收腰时前臂贴身前而非别到后腰
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0),
        head=dict(pitch=-2, yaw=-10),
        torso=dict(pitch=+2, yaw=+20),
        rightArm=dict(pitch=-42, yaw=-6, roll=-5, bend=40, axis=180),
        leftArm=dict(pitch=-30, yaw=+18, roll=-8, bend=50, axis=180),
        rightLeg=dict(pitch=+6, yaw=+4, bend=12, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.06),
    ),
    3: dict(  # LOAD —— 剑收腰 后坐
        easing="INOUTSINE",
        body=dict(x=+0.06, y=+0.02, z=-0.08),
        head=dict(pitch=-3, yaw=-12),
        torso=dict(pitch=+4, yaw=+30),
        rightArm=dict(pitch=-22, yaw=-18, roll=-10, bend=95, axis=180),
        leftArm=dict(pitch=-25, yaw=+14, roll=-5, bend=40, axis=180),
        rightLeg=dict(pitch=+16, yaw=+4, bend=42, z=+0.06),
        leftLeg=dict(pitch=-6, yaw=+4, bend=15, z=-0.05),
    ),
    5: dict(  # IMPACT —— body.z 猛前冲
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.02, z=+0.28),
        head=dict(pitch=+4, yaw=+2),
        torso=dict(pitch=+5, yaw=-6),
        rightArm=dict(pitch=-88, yaw=+2, roll=+8, bend=3, axis=180),
        leftArm=dict(pitch=-40, yaw=+28, roll=-18, bend=85, axis=180),  # counter
        rightLeg=dict(pitch=+4, yaw=+8, bend=12, z=+0.02),
        leftLeg=dict(pitch=-28, yaw=+4, bend=42, z=-0.10),  # 深弓
    ),
    6: dict(  # overshoot —— 再刺深一寸
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.01, z=+0.32),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+5, yaw=-4),
        rightArm=dict(pitch=-95, yaw=+4, roll=+12, bend=6, axis=180),
        leftArm=dict(pitch=-42, yaw=+30, roll=-20, bend=88, axis=180),
        rightLeg=dict(pitch=+2, yaw=+8, bend=12),
        leftLeg=dict(pitch=-30, yaw=+4, bend=44, z=-0.10),
    ),
    8: dict(  # 回 guard
        easing="INOUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0),
        head=dict(pitch=-2, yaw=-10),
        torso=dict(pitch=+2, yaw=+20),
        rightArm=dict(pitch=-42, yaw=-6, roll=-5, bend=40, axis=180),
        leftArm=dict(pitch=-30, yaw=+18, roll=-8, bend=50, axis=180),
        rightLeg=dict(pitch=+6, yaw=+4, bend=12, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.06),
    ),
}

DESCRIPTION = (
    "v1 JSON 直刺: body.z +0.28 前扑为视觉主体 (不是手臂伸直), "
    "torso yaw +30° → -6° 转正扭矩 36°, 左腿深弓步 pitch=-28° bend=42°, "
    "左臂 counter-pull 猛抬 85°, axis=180°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_stab",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
