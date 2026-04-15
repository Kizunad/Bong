#!/usr/bin/env python3
"""cast_invoke — 引动法宝 (双手从身侧抬到胸前合拢凝气)。

身法节奏（15 tick / 0.75s, 非循环）：
  tick 0  guard   双臂下垂
  tick 5  raise   双手抬到身侧 (pitch=-60° bend=65°)
  tick 10 gather  合拢胸前 (pitch=-70° bend=120° yaw 内收 ±25°)
  tick 13 settle  凝气微沉 (pitch=-72° bend=128°)
  tick 15 hold    末态保持 (INOUTSINE 让收招有"顿住"感)

**"凝气"感从哪来**: 不是单纯把手抬到位, 而是 tick 10→13 有一个 body.y -0.04 的沉气
下坐 + torso pitch -5° 挺直, 让观众感觉"气沉了下去"。Java 原版只做双臂运动, 缺了
这个气劲的视觉锚点。

反僵硬要点：
  - tick 5 raise 阶段双手已经微内收 yaw=±10°, 不是等到 tick 10 才收 —— 这样手轨迹
    是弧线不是折线
  - 双手合拢时掌心相对: roll ±15° (右臂 roll -15° = 掌心朝内左, 左臂 +15° = 朝内右)
  - head pitch 0° → +8° → +18° 渐进低头凝视 (不是 Java 的单帧跳)
  - torso pitch 0° → -2° → -5° 挺直 (气沉丹田姿)
  - axis=180° 所有 bend 朝前折
  - 腿微屈 (bend=10°) 表示承气不是站直木
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
    5: dict(  # raise —— 双手抬到身侧
        easing="INOUTSINE",
        body=dict(y=-0.02),
        head=dict(pitch=+6),
        torso=dict(pitch=-2),
        rightArm=dict(pitch=-60, yaw=-10, roll=-5, bend=65, axis=180),
        leftArm=dict(pitch=-60, yaw=+10, roll=+5, bend=65, axis=180),
        rightLeg=dict(bend=6),
        leftLeg=dict(bend=6),
    ),
    10: dict(  # gather —— 合拢胸前
        easing="INOUTSINE",
        body=dict(y=-0.04),
        head=dict(pitch=+15),
        torso=dict(pitch=-4),
        rightArm=dict(pitch=-70, yaw=-25, roll=-15, bend=120, axis=180),
        leftArm=dict(pitch=-70, yaw=+25, roll=+15, bend=120, axis=180),
        rightLeg=dict(bend=10),
        leftLeg=dict(bend=10),
    ),
    13: dict(  # settle —— 凝气微沉
        easing="INOUTSINE",
        body=dict(y=-0.05),
        head=dict(pitch=+18),
        torso=dict(pitch=-5),
        rightArm=dict(pitch=-72, yaw=-26, roll=-16, bend=128, axis=180),
        leftArm=dict(pitch=-72, yaw=+26, roll=+16, bend=128, axis=180),
        rightLeg=dict(bend=10),
        leftLeg=dict(bend=10),
    ),
    15: dict(  # hold
        easing="INOUTSINE",
        body=dict(y=-0.04),
        head=dict(pitch=+18),
        torso=dict(pitch=-5),
        rightArm=dict(pitch=-72, yaw=-25, roll=-16, bend=126, axis=180),
        leftArm=dict(pitch=-72, yaw=+25, roll=+16, bend=126, axis=180),
        rightLeg=dict(bend=10),
        leftLeg=dict(bend=10),
    ),
}

DESCRIPTION = (
    "v1 JSON 引动法宝: 15 tick 非循环, guard → 抬 → 合拢 → 沉气凝, "
    "双手 pitch-72° bend128° yaw±25° roll±16° axis=180° 胸前掌心相对, "
    "body.y -0.05 沉气 + torso -5° 挺胸 + head +18° 凝视掌心, 腿微屈 bend10°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="cast_invoke",
        description=DESCRIPTION,
        end_tick=15,
        stop_tick=18,
        is_loop=False,
    )
