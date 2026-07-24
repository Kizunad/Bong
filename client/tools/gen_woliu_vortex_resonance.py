#!/usr/bin/env python3
"""woliu_vortex_resonance —— 涡流共鸣 80t 持续引导站姿（bugfix：循环单帧衰减）。

bugfix 上下文（归 docs/plans-skeleton/plan-bughunt-woliu-resonance-loop-arm-decay-v1）：
原始资产 `woliu_vortex_resonance.json` 是手写 JSON（无对应生成器），`emote.isLoop=
true`、`endTick=80`、`returnTick=0`，但 `rightArm`/`leftArm` 的全部轴只在 tick 0
（起手内收）和 tick 40（峰值托举展开）有关键帧，`endTick=80` 一个都没补——命中
PlayerAnimator 库坑 #1（见 docs/player-animation-conventions.md、
client/tools/anim_common.py 顶部说明）：循环回绕时 `KeyframeAnimationPlayer.
Axis.findAfter` 会向虚拟 `endTick+1` 的 `defaultValue` 插值，4 秒引导窗后半段
（tick 40→80）双臂逐渐松垮回默认站姿，削弱涡流共鸣的战斗可读性。附带
`torso.pitch` 只在 tick 40/80 有、tick 0 缺，回绕(80→0)同样触发该判定
（`AnimCastTicksAlignmentTest#loopSeamViolations` 的 endTick-无关键帧/值不等两种
形态都命中）。

本生成器改用 `anim_common.emit_json`（自带 `_check_loop_closure` 强校验），只补
循环闭合缺帧、不改中段 tick 40 的托举姿态：
  - `rightArm`/`leftArm` 在 endTick(80) 补 == tick0 值的关键帧（回到起手内收）；
  - `torso.pitch` 在 tick0 补 == endTick(80) 值(0.0)的关键帧。

原始资产用弧度手写（pitch=-0.50 / yaw=-0.40 / roll=-0.20 / bend=0.95 /
axis=π 等）。`anim_common` 的接口是角度，此处显式 `math.degrees(<原始弧度>)`
换算，度数→弧度往返在 `round(...,7)` 精度下与原始弧度值逐位相等（不改变涡流
共鸣的原始设计姿态，只补循环闭合关键帧）。
"""

from __future__ import annotations

import math

from anim_common import emit_json

# 起手内收托掌（tick 0，循环闭合后也是 endTick 80 的值）。
_RIGHT_ARM_TUCKED = dict(
    pitch=math.degrees(-0.50),
    yaw=math.degrees(-0.40),
    roll=math.degrees(-0.20),
    bend=math.degrees(0.95),
    axis=180.0,
)
_LEFT_ARM_TUCKED = dict(
    pitch=math.degrees(-0.50),
    yaw=math.degrees(0.40),
    roll=math.degrees(0.20),
    bend=math.degrees(0.95),
    axis=180.0,
)

# 峰值托举展开（tick 40，中段姿态，本次 bugfix 不改动）。
_RIGHT_ARM_LIFTED = dict(
    pitch=math.degrees(-1.35),
    yaw=math.degrees(-0.32),
    roll=math.degrees(-0.50),
    bend=math.degrees(0.68),
    axis=180.0,
)
_LEFT_ARM_LIFTED = dict(
    pitch=math.degrees(-1.35),
    yaw=math.degrees(0.32),
    roll=math.degrees(0.50),
    bend=math.degrees(0.68),
    axis=180.0,
)

_TORSO_LIFTED_PITCH = math.degrees(-0.06)

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0),
        rightArm=_RIGHT_ARM_TUCKED,
        leftArm=_LEFT_ARM_TUCKED,
        # bugfix：torso.pitch 循环闭合补帧 —— 必须 == endTick(80) 的值 0.0。
        torso=dict(pitch=0.0),
    ),
    40: dict(
        easing="INOUTSINE",
        body=dict(y=0.18),
        torso=dict(pitch=_TORSO_LIFTED_PITCH),
        rightArm=_RIGHT_ARM_LIFTED,
        leftArm=_LEFT_ARM_LIFTED,
    ),
    80: dict(
        easing="INOUTSINE",
        body=dict(y=0.0),
        torso=dict(pitch=0.0),
        # bugfix：双臂循环闭合补帧（库坑 #1 主症状）—— 必须 == tick0 的值，
        # 否则 tick40→80 回绕会向 defaultValue 插值，双臂松垮回默认站姿。
        rightArm=_RIGHT_ARM_TUCKED,
        leftArm=_LEFT_ARM_TUCKED,
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="woliu_vortex_resonance",
        description="woliu-v3 slow floating resonance stance with palms raised.",
        end_tick=80,
        stop_tick=84,
        is_loop=True,
        return_tick=0,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
