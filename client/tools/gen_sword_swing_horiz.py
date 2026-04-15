#!/usr/bin/env python3
"""sword_swing_horiz — 双手剑左上→右下的横扫。

身法节奏（10 tick / 0.5s）：
  tick 0 guard   刀斜举左肩上方，腰微扭蓄好"拧"的感觉
  tick 3 LOAD    拉到极限——刀绕至左后方 + 腰扭到底 (torso yaw +28°)
  tick 5 IMPACT  过中线，刀速峰值——腰反扭猛发 + body.z 前冲 + 直臂
  tick 6 over    刀甩到右前下 (overshoot) + 翻腕 roll
  tick 7 tail    收势末 (尾速衰减)
  tick 10 回     回到 guard

反僵硬要点：
  - 躯干 yaw 从 +28° → -25° 打出 53° 扭矩 —— 腰是横扫真正的发动机
  - body.x 重心切换 (+0.04 → +0.08 → -0.10) 让重心从后脚移到前脚
  - 左臂 counter-pull (IMPACT 时反向拉到 +40° yaw) 像真实挥剑的双手稳定
  - axis=180° 让前臂在 LOAD 蓄力时朝前折贴胸（不是朝背后别扭）
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=+0.04, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+10),
        torso=dict(pitch=+3, yaw=+12),
        rightArm=dict(pitch=-62, yaw=+32, roll=-8, bend=35, axis=180),
        leftArm=dict(pitch=-55, yaw=+15, roll=-18, bend=60, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=12),
        leftLeg=dict(pitch=+6, yaw=+4, bend=10, z=+0.03),
    ),
    3: dict(  # LOAD —— 扭到极限
        easing="INOUTSINE",
        body=dict(x=+0.08, y=-0.02, z=-0.04),
        head=dict(pitch=-2, yaw=+6),  # 头偏小一点（腰已经扭了，头回正盯目标）
        torso=dict(pitch=+2, yaw=+28),
        rightArm=dict(pitch=-78, yaw=+40, roll=-12, bend=55, axis=180),
        leftArm=dict(pitch=-60, yaw=+22, roll=-22, bend=75, axis=180),
        rightLeg=dict(pitch=-10, yaw=+4, bend=18),
        leftLeg=dict(pitch=+10, yaw=+4, bend=12, z=+0.05),
    ),
    5: dict(  # IMPACT —— 过中线 直臂
        easing="OUTQUAD",
        body=dict(x=-0.10, y=-0.02, z=+0.12),
        head=dict(pitch=-6, yaw=-12),  # 头跟着剑
        torso=dict(pitch=+4, yaw=-25),
        rightArm=dict(pitch=-30, yaw=-10, roll=+5, bend=3, axis=180),
        leftArm=dict(pitch=-50, yaw=+40, roll=-15, bend=85, axis=180),  # counter-pull
        rightLeg=dict(pitch=-14, yaw=+6, bend=22),
        leftLeg=dict(pitch=+4, yaw=+6, bend=8, z=+0.02),
    ),
    6: dict(  # overshoot —— 剑甩到右前下
        easing="OUTQUAD",
        body=dict(x=-0.08, y=0.0, z=+0.10),
        head=dict(pitch=-4, yaw=-16),
        torso=dict(pitch=+6, yaw=-22),
        rightArm=dict(pitch=-5, yaw=-48, roll=+20, bend=8, axis=180),
        leftArm=dict(pitch=-55, yaw=+35, roll=-10, bend=90, axis=180),
        rightLeg=dict(pitch=-12, yaw=+6, bend=18),
        leftLeg=dict(pitch=+4, yaw=+6, bend=8),
    ),
    7: dict(  # tail —— 尾速衰减
        easing="OUTQUAD",
        body=dict(x=-0.04, y=0.0, z=+0.06),
        head=dict(pitch=-2, yaw=-10),
        torso=dict(pitch=+6, yaw=-14),
        rightArm=dict(pitch=+18, yaw=-52, roll=+28, bend=18, axis=180),
        leftArm=dict(pitch=-50, yaw=+25, roll=-12, bend=80, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=14),
        leftLeg=dict(pitch=+4, yaw=+4, bend=10),
    ),
    10: dict(  # 回 guard
        easing="INOUTSINE",
        body=dict(x=+0.04, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=+10),
        torso=dict(pitch=+3, yaw=+12),
        rightArm=dict(pitch=-62, yaw=+32, roll=-8, bend=35, axis=180),
        leftArm=dict(pitch=-55, yaw=+15, roll=-18, bend=60, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=12),
        leftLeg=dict(pitch=+6, yaw=+4, bend=10, z=+0.03),
    ),
}

DESCRIPTION = (
    "v1 JSON 横扫: 腰先扭到 +28° → IMPACT 反扭到 -25° (53° 扭矩), "
    "左臂 counter-pull 提供稳定, body.z 前冲 0.12 + body.x 重心切换, axis=180° 前臂折前。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_swing_horiz",
        description=DESCRIPTION,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
