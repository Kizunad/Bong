#!/usr/bin/env python3
"""guard_raise — 双臂快速交叉护头格挡。

身法节奏（4 tick / 0.2s，最短动作）：
  tick 0 开始      手臂下垂略抬
  tick 2 SNAP-UP   双臂抢抬到**脸前交叉** (pitch=-85° bend=110° yaw±25°)
  tick 4 HOLD      微回稳 + 保持格挡

**pitch 校准记录**: v1 抄 Java 用 pitch=-145° bend=135°, 实算 hand world
z=+0.6（身后!）—— -145° 把手臂抬到脑后而非胸前。正确是 pitch=-85° + bend=110°,
让手臂水平前抬后前臂折向脸前, 数值推导 hand ≈ (-4, -3.5, -2.2) 正好覆盖脸颊。

反僵硬要点：
  - 4 tick 极短，没空间 anticipation —— 用 OUTQUAD 的冲击感替代
  - 双臂 yaw ±25° 内收让前臂在脸前形成 X 交叉 (不是 T 形平举)
  - head pitch +12° 低头收下颌，护 jaw (真实格挡动作)
  - body.y -0.06 微蹲扛接，torso pitch +6° 微前倾
  - axis=180° 让手折到脸前 (护脸) 不是折到脑后
  - 腿 bend=15° 微屈扛冲击力 (承重姿态)
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="OUTQUAD",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=-15, yaw=-5, roll=-8, bend=15, axis=180),
        leftArm=dict(pitch=-15, yaw=+5, roll=+8, bend=15, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    2: dict(  # SNAP-UP —— 脸前交叉
        easing="OUTQUAD",
        body=dict(y=-0.06),
        head=dict(pitch=+12),  # 低头收下颌
        torso=dict(pitch=+6),
        rightArm=dict(pitch=-85, yaw=-25, roll=-20, bend=110, axis=180),
        leftArm=dict(pitch=-85, yaw=+25, roll=+20, bend=110, axis=180),
        rightLeg=dict(bend=16),
        leftLeg=dict(bend=16),
    ),
    4: dict(  # HOLD —— 微回稳
        easing="OUTQUAD",
        body=dict(y=-0.04),
        head=dict(pitch=+10),
        torso=dict(pitch=+5),
        rightArm=dict(pitch=-82, yaw=-22, roll=-16, bend=105, axis=180),
        leftArm=dict(pitch=-82, yaw=+22, roll=+16, bend=105, axis=180),
        rightLeg=dict(bend=14),
        leftLeg=dict(bend=14),
    ),
}

DESCRIPTION = (
    "v1 JSON 格挡: 4 tick snap-up, 双臂脸前交叉 (pitch=-85° bend=110° yaw±25° axis=180°), "
    "hand 世界坐标≈(±4, -3.5, -2.2) 正好覆盖脸前, head 低 +12° 收下颌, "
    "body.y 微蹲 -0.06, 腿 bend 15° 扛冲击。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="guard_raise",
        description=DESCRIPTION,
        end_tick=4,
        stop_tick=7,
        is_loop=False,
    )
