#!/usr/bin/env python3
"""breakthrough_burst — 境界突破爆发 (低头蓄 → 仰天展臂 → 稳 → 收)。

身法节奏（60 tick / 3s, 非循环）：
  tick 0   guard   预备
  tick 10  CHARGE  低头蓄 (torso +15° head +25° body.y -0.08 双臂收胸 bend=90°)
  tick 25  BURST   爆发 (head -45° 仰天 + 双臂 pitch=-95° yaw±72° bend=10° 大展 + body.y +0.22 上浮 + torso -12° 挺胸)
  tick 35  peak    放光峰 (body.y +0.25)
  tick 45  stable  稳定回落 (body.y +0.18)
  tick 60  回 guard

**"放量"的关键**: 不是单一放大双臂 yaw, 而是 body.y 上浮 0.25 格 + head 仰天 45° +
torso 挺胸 -12° 形成"整个身体被气劲抬起"的视觉。Java 原版 yaw ±75° bend 10° 这组数
本身 OK, 但 pitch -130° 会把手臂绕过头顶指向身后下方 (axis=0° 默认 + bend 10° 轻微
朝身后折)。改 pitch=-95° (垂直略仰) + yaw=±72° (向两侧外展) + bend=10° axis=180°
实现"十字展开式仰天吸纳天地"姿态 (而非 Java 的"过头抛臂")。

反僵硬要点：
  - 4 阶段节奏变速: 0→10 慢 (INOUTSINE), 10→25 爆发 (OUTQUAD), 25→45 保持 (INOUTSINE),
    45→60 缓收 (INOUTSINE)
  - body.y 三峰: 0 → -0.08 蓄 → +0.22 爆 → +0.25 峰 → +0.18 稳 → 0 收
  - head 反向: 蓄 +25° 低头 → 爆 -45° 仰天 (70° 翻转)
  - 双臂 yaw 反向: 蓄 ±20° 内收 → 爆 ±72° 外展 (92° 翻转)
  - 腿 bend 30° 蓄力 → 5° 挺直支撑
  - axis=180° 让 bend 朝前折 (爆发时手肘略弯, 不是笔直举)
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
    10: dict(  # CHARGE —— 低头蓄
        easing="INOUTSINE",
        body=dict(y=-0.08),
        head=dict(pitch=+25),
        torso=dict(pitch=+15),
        rightArm=dict(pitch=-40, yaw=-20, bend=90, axis=180),
        leftArm=dict(pitch=-40, yaw=+20, bend=90, axis=180),
        rightLeg=dict(bend=30),
        leftLeg=dict(bend=30),
    ),
    25: dict(  # BURST —— 仰天大展
        easing="OUTQUAD",
        body=dict(y=+0.22),
        head=dict(pitch=-45),
        torso=dict(pitch=-12),
        rightArm=dict(pitch=-95, yaw=-72, bend=10, axis=180),
        leftArm=dict(pitch=-95, yaw=+72, bend=10, axis=180),
        rightLeg=dict(bend=5),
        leftLeg=dict(bend=5),
    ),
    35: dict(  # peak —— 放光最高
        easing="INOUTSINE",
        body=dict(y=+0.25),
        head=dict(pitch=-42),
        torso=dict(pitch=-11),
        rightArm=dict(pitch=-96, yaw=-74, bend=8, axis=180),
        leftArm=dict(pitch=-96, yaw=+74, bend=8, axis=180),
        rightLeg=dict(bend=6),
        leftLeg=dict(bend=6),
    ),
    45: dict(  # stable —— 微回落
        easing="INOUTSINE",
        body=dict(y=+0.18),
        head=dict(pitch=-40),
        torso=dict(pitch=-10),
        rightArm=dict(pitch=-90, yaw=-70, bend=12, axis=180),
        leftArm=dict(pitch=-90, yaw=+70, bend=12, axis=180),
        rightLeg=dict(bend=8),
        leftLeg=dict(bend=8),
    ),
    60: dict(  # 收 guard
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
}

DESCRIPTION = (
    "v1 JSON 突破: 60 tick 4 阶段, 蓄 (head+25° torso+15° body.y-0.08 臂收胸 bend90°) → "
    "爆 (head-45° 仰天, 双臂 pitch-95° yaw±72° bend10° axis=180° 大展, body.y+0.22 上浮, "
    "torso-12° 挺胸) → peak body.y+0.25 → 稳 → 收。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="breakthrough_burst",
        description=DESCRIPTION,
        end_tick=60,
        stop_tick=63,
        is_loop=False,
    )
