#!/usr/bin/env python3
"""sword_ride — 御剑飞行 (蹲踞踏剑冲刺姿)。

身法节奏（40 tick / 2s 循环）：
  tick 0/40 base    蹲踞稳态
  tick 10 gust-L    风阻左偏 (袖被风刮)
  tick 20 mid       恢复 + 速度起伏
  tick 30 gust-R    风阻右偏

**Java v5 根因教训**: v1/v2/v3 "看不见腿抬" 不是参数弱, 是循环 axis 只在 tick 0 加
单帧会被 KeyframeAnimationPlayer.Axis.findAfter 自动拼接 endTick+1 → defaultValue 虚拟
帧, 导致中段插值回默认。循环动画所有 axis 必须在 tick=end_tick 补同值帧。

**腿腹断连教训**: v3 加 leg.z=-0.25 把 pivot 移出 body 下方, 反而加剧断连。正解是
降低 pitch (v5 40°, 减轻顶面倾斜 ±1.29px), 用 bend 105° 补蹲的视觉强度。

反僵硬要点：
  - 躯干前倾 +25° 冲刺姿 (身体压风)
  - 双腿 pitch=+40° + bend=105° axis=180° + yaw 外开 ±8° → 蹲踞踏剑 (大腿前抬、小腿折下)
  - 双臂 pitch=+60° + yaw=±55° bend=15° axis=180° → 后展 (风阻平衡)
  - 风阻颤抖: 腿 bend 105°↔110° / 臂 yaw ±55°↔±58° 周期性
  - 袖摆方向差: 左右不同相位 (tick 10 右袖被刮内收 yaw -52° / 左袖外翻 +58°)
  - head pitch -10° 抬头盯前方
  - body.y -0.12 蹲踞下沉 (Java v3 校准)
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=-0.12),
        head=dict(pitch=-10),
        torso=dict(pitch=+25),
        rightArm=dict(pitch=+60, yaw=-55, roll=0, bend=15, axis=180),
        leftArm=dict(pitch=+60, yaw=+55, roll=0, bend=15, axis=180),
        rightLeg=dict(pitch=+40, yaw=-8, bend=105, axis=180),
        leftLeg=dict(pitch=+40, yaw=+8, bend=105, axis=180),
    ),
    10: dict(  # 风刮左
        easing="INOUTSINE",
        body=dict(y=-0.10),
        head=dict(pitch=-11, yaw=+4),  # 微回头迎风
        torso=dict(pitch=+24, yaw=+2),
        rightArm=dict(pitch=+60, yaw=-52, roll=-5, bend=15, axis=180),
        leftArm=dict(pitch=+61, yaw=+58, roll=+8, bend=16, axis=180),
        rightLeg=dict(pitch=+40, yaw=-8, bend=108, axis=180),
        leftLeg=dict(pitch=+40, yaw=+8, bend=108, axis=180),
    ),
    20: dict(
        easing="INOUTSINE",
        body=dict(y=-0.07),
        head=dict(pitch=-10),
        torso=dict(pitch=+23),
        rightArm=dict(pitch=+60, yaw=-56, roll=0, bend=15, axis=180),
        leftArm=dict(pitch=+60, yaw=+56, roll=0, bend=15, axis=180),
        rightLeg=dict(pitch=+40, yaw=-8, bend=110, axis=180),
        leftLeg=dict(pitch=+40, yaw=+8, bend=110, axis=180),
    ),
    30: dict(  # 风刮右
        easing="INOUTSINE",
        body=dict(y=-0.10),
        head=dict(pitch=-11, yaw=-4),
        torso=dict(pitch=+24, yaw=-2),
        rightArm=dict(pitch=+61, yaw=-58, roll=+8, bend=16, axis=180),
        leftArm=dict(pitch=+60, yaw=+52, roll=-5, bend=15, axis=180),
        rightLeg=dict(pitch=+40, yaw=-8, bend=108, axis=180),
        leftLeg=dict(pitch=+40, yaw=+8, bend=108, axis=180),
    ),
    40: dict(  # 闭环
        easing="INOUTSINE",
        body=dict(y=-0.12),
        head=dict(pitch=-10),
        torso=dict(pitch=+25),
        rightArm=dict(pitch=+60, yaw=-55, roll=0, bend=15, axis=180),
        leftArm=dict(pitch=+60, yaw=+55, roll=0, bend=15, axis=180),
        rightLeg=dict(pitch=+40, yaw=-8, bend=105, axis=180),
        leftLeg=dict(pitch=+40, yaw=+8, bend=105, axis=180),
    ),
}

DESCRIPTION = (
    "v1 JSON 御剑: 40 tick 蹲踞冲刺循环, 躯干前倾 +25°, 双腿 pitch+40° bend105° axis=180° "
    "踏剑, 双臂 pitch+60° yaw±55° 后展平衡, 风阻颤抖 (腿 bend ±3°, 臂 yaw ±3° + roll 左右"
    "交替 5-8°), head -10° 盯前方, body.y -0.12 下沉。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_ride",
        description=DESCRIPTION,
        end_tick=40,
        stop_tick=43,
        is_loop=True,
    )
