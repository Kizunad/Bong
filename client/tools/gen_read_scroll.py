#!/usr/bin/env python3
"""read_scroll — 双手持卷阅读循环（plan-scroll-reading-v1 P2）。

身法节奏（40 tick / 2s 循环，呼吸感微摆）：
  tick 0/40  valley  呼气谷值 (torso pitch=10°)
  tick 10    peak    吸气峰值 (torso pitch=12°, 卷展开时胸腔微起)
  tick 20    mid     回中 (torso pitch=10°)
  tick 30    trough  呼气谷值 (torso pitch=8°)

**姿态设计**：双手持卷展开于胸前偏下，仿 `gen_bow_salute.py:52-58` 的"鞠躬补偿"惯例——
`body.y -0.04` 下沉 + 双腿 `pitch -8° bend +12°` 微屈，让读卷时的低头前倾不显得腰背断裂。
`rightArm`/`leftArm` `pitch≈-75°, bend≈90°, axis=180°`（前折约定，`anim_common.py:31`），
`yaw ±22°` 让双手在体前偏中线展开卷轴（一手持左端一手持右端）。`head.pitch +12°` 低头
看卷，`torso.pitch +10°` 前倾配合低头（不到鞠躬礼的 +30°，只是专注阅读的浅前倾）。

**呼吸感微摆 ±2°**：只有 `torso.pitch` 随时间在 8°~12° 之间摆动（呼气 8° → 吸气峰 12° →
回中 10°），双臂/双腿/body 全程静止（持卷稳定，不随呼吸晃动，否则显得握不稳）。

**loop 首尾帧全轴对齐**：tick 0 与 tick 40（= tick 0）在每个用到的轴上取值完全相同
（`torso.pitch` 均为 10°，其余部位每 tick 都写相同值）——`_check_loop_closure` 断言必过，
防止 `PlayerAnimator` 在循环边界外插值淡回 T-pose。
"""
from anim_common import emit_json

_ARMS = dict(
    rightArm=dict(pitch=-75, yaw=-22, roll=-6, bend=90, axis=180),
    leftArm=dict(pitch=-75, yaw=+22, roll=+6, bend=90, axis=180),
)
_LEGS = dict(
    rightLeg=dict(pitch=-8, bend=12),
    leftLeg=dict(pitch=-8, bend=12),
)
_BODY = dict(y=-0.04, z=0.0)
_HEAD = dict(pitch=+12)

POSE = {
    0: dict(  # 呼气谷值 (= tick 40)
        easing="INOUTSINE",
        body=_BODY,
        head=_HEAD,
        torso=dict(pitch=10),
        **_ARMS,
        **_LEGS,
    ),
    10: dict(  # 吸气峰值 —— 胸腔微起
        easing="INOUTSINE",
        body=_BODY,
        head=_HEAD,
        torso=dict(pitch=12),
        **_ARMS,
        **_LEGS,
    ),
    20: dict(  # 回中
        easing="INOUTSINE",
        body=_BODY,
        head=_HEAD,
        torso=dict(pitch=10),
        **_ARMS,
        **_LEGS,
    ),
    30: dict(  # 呼气谷值（本次循环的另一极值，制造完整呼吸弧线）
        easing="INOUTSINE",
        body=_BODY,
        head=_HEAD,
        torso=dict(pitch=8),
        **_ARMS,
        **_LEGS,
    ),
    40: dict(  # 循环闭合 (= tick 0)
        easing="INOUTSINE",
        body=_BODY,
        head=_HEAD,
        torso=dict(pitch=10),
        **_ARMS,
        **_LEGS,
    ),
}

DESCRIPTION = (
    "v1 JSON 双手持卷阅读循环：40 tick loop。双臂 pitch=-75° yaw=∓22° bend=90° axis=180° "
    "在胸前偏下展开卷轴（前折约定），双腿 pitch=-8° bend=12° 微屈 + body.y=-0.04 下沉做鞠躬式"
    "承重补偿，head.pitch=+12° 低头看卷，torso.pitch 在 8°~12° 间呼吸摆动 (10→12→10→8→10)，"
    "其余部位全程静止（持卷稳）。loop 首尾 (tick 0 / tick 40) 每轴同值。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="read_scroll",
        description=DESCRIPTION,
        end_tick=40,
        stop_tick=43,
        is_loop=True,
    )
