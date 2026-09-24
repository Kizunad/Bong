#!/usr/bin/env python3
"""baomai_full_power_release —— 全力一击·崩拳双锤释放（P3 批次三，借用解除）。

通道核验（P3 第一性原理，2026-07-19）：`cast_full_power_release`
（server/src/combat/baomai_v3/skills.rs:252）→
`full_power_strike::release_full_power_with_exhaust`（full_power_strike.rs:230-282）
**消费 `ChargingState` 即时结算**（remove 组件 + FullPowerAttackIntent + Exhausted，
无 Casting/无引导窗），cast_ticks=1 → **瞬发域**（[6,12] 爆发帧+收势），
endTick=12。释放同时 full_power_emit.rs 对蓄力循环段发 StopAnim（接力先例同
anqi charge_carrier release）。

借用解除：原内联 `"bong:release_burst"`（4t 通用爆发模板 81KF）→ 专属
`bong:baomai_full_power_release`（server 常量
`full_power_strike::FULL_POWER_RELEASE_ANIM_ID`）。母题「崩拳双锤」：**起手帧
= 蓄力段 BASE 位**（抱脉沉桩，与 loop 稳定帧无缝衔接，差值靠 fade_in 平滑），
微坐拧腰 → 双拳同时崩出全身前送 → 泄力垂臂虚脱意（对应 Exhausted debuff）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   自蓄力桩微坐拧腰（body.z -0.08 / torso.yaw -12）
  strike       3→6   双拳崩出（双臂 pitch -75/-72 / body.z +0.30 / torso.pitch
                     +16 前压），顶点 = tick 6
  recovery     6→12  泄力：拳头垂落、身形塌半分（虚脱意）→ 归中立
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
body.z / torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手 = 蓄力段 BASE 位（抱脉沉桩，无缝接力）。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.08, z=-0.02),
        head=dict(pitch=+7, yaw=0),
        torso=dict(pitch=+8, yaw=0, roll=0),
        rightArm=dict(pitch=-12, yaw=-14, roll=+8, bend=96, axis=180),
        leftArm=dict(pitch=-12, yaw=+14, roll=-8, bend=96, axis=180),
        leftLeg=dict(pitch=-10, bend=22, z=-0.06),
        rightLeg=dict(pitch=+9, bend=20, z=+0.05),
    ),
    # 微坐拧腰：最后一口气压进拳里。
    3: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.11, z=-0.08),
        head=dict(pitch=+9, yaw=+4),
        torso=dict(pitch=+10, yaw=-12, roll=-2),
        rightArm=dict(pitch=-8, yaw=-18, roll=+12, bend=104, axis=180),
        leftArm=dict(pitch=-8, yaw=+18, roll=-12, bend=104, axis=180),
        leftLeg=dict(pitch=-12, bend=26, z=-0.07),
        rightLeg=dict(pitch=+11, bend=24, z=+0.05),
    ),
    # 崩出中段：双拳离腰加速。
    5: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.04, z=+0.16),
        head=dict(pitch=+5, yaw=-2),
        torso=dict(pitch=+13, yaw=+6, roll=+1),
        rightArm=dict(pitch=-52, yaw=-8, roll=+4, bend=42, axis=180),
        leftArm=dict(pitch=-48, yaw=+8, roll=-4, bend=46, axis=180),
        leftLeg=dict(pitch=-20, bend=22, z=-0.10),
        rightLeg=dict(pitch=+16, bend=20, z=+0.06),
    ),
    # 崩击顶点（tick 6）：双拳同炸、全身前送弓步。
    6: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.02, z=+0.30),
        head=dict(pitch=+4, yaw=0),
        torso=dict(pitch=+16, yaw=+2, roll=0),
        rightArm=dict(pitch=-75, yaw=-6, roll=+2, bend=12, axis=180),
        leftArm=dict(pitch=-72, yaw=+6, roll=-2, bend=14, axis=180),
        leftLeg=dict(pitch=-26, bend=24, z=-0.12),
        rightLeg=dict(pitch=+20, bend=22, z=+0.08),
    ),
    # 泄力一段：拳沉、肘弯回落（虚脱开始）。
    8: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.05, z=+0.16),
        head=dict(pitch=+8, yaw=0),
        torso=dict(pitch=+12, yaw=0, roll=0),
        rightArm=dict(pitch=-40, yaw=-6, roll=+3, bend=30, axis=180),
        leftArm=dict(pitch=-36, yaw=+6, roll=-3, bend=32, axis=180),
        leftLeg=dict(pitch=-18, bend=20, z=-0.09),
        rightLeg=dict(pitch=+14, bend=18, z=+0.06),
    ),
    # 泄力二段：垂臂塌肩。
    10: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.03, z=+0.06),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+6, yaw=0, roll=0),
        rightArm=dict(pitch=-12, yaw=-4, roll=+2, bend=14, axis=180),
        leftArm=dict(pitch=-10, yaw=+4, roll=-2, bend=16, axis=180),
        leftLeg=dict(pitch=-10, bend=12, z=-0.05),
        rightLeg=dict(pitch=+8, bend=10, z=+0.03),
    ),
    # 归中立。
    12: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="baomai_full_power_release",
        description=(
            "P3 全力一击释放专属（12t 瞬发，解除 release_burst 借用）：起手=蓄力"
            "段 BASE 位无缝接力 → 微坐拧腰（body.z -0.08 / torso.yaw -12）→ 双拳"
            "崩出（pitch -75/-72 / body.z +0.30，顶点=t6）→ 泄力垂臂虚脱意归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
