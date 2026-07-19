#!/usr/bin/env python3
"""baomai_full_power_charge —— 全力一击·抱脉沉桩蓄力循环（P3 批次三，借用解除）。

通道核验（P3 第一性原理，2026-07-19）：`cast_full_power_charge`
（server/src/combat/baomai_v3/skills.rs:200）→ `full_power_strike::start_charge_fn`
插入 **`ChargingState` 自定充能状态机**（full_power_strike.rs:169-175，逐 tick
灌真元直到玩家释放），受击打断走 `charge_interrupt_system`（:492）。**双退出
路径 StopAnim 已接线**：释放（FullPowerReleasedEvent）与打断
（ChargeInterruptedEvent）均经 `emit_full_power_charging_clear_payloads` →
`stop_windup_charge_anim`（network/full_power_emit.rs:66-105）→ **持续维持型
循环**（按住蓄力，同 shield_block 形态），入对拍测试 SUSTAINED_LOOP_EXCEPTIONS
+ segment loop manifest。cast_ticks=1 是入口元数据。

借用解除：原内联 `"bong:windup_charge"`（skills.rs 高举蓄力通用模板）→ 专属
`bong:baomai_full_power_charge`（server 共享常量
`full_power_strike::FULL_POWER_CHARGE_ANIM_ID`，播/停同源）。母题「抱脉沉桩」：
马步双拳收腰，呼吸节律一沉一浮、拳面震颤（真元灌注），与 windup_charge（高举）
/ anqi 封骨结印（胸前合掌）/ 各 stance 完全区分。

循环红线（§13 #5/#6，库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 同值闭环（loopSeamViolations 机械为空）。

时序（24t 呼吸周期）：
  0→6   提息微浮：拳随吸气微抬（pitch -12→-18）、身浮 y -0.08→-0.05
  6→12  灌压沉桩：拳收更紧、身沉 y -0.11、俯首 +10
  12→18 底点震颤：双拳 roll 交替脉冲（+14/-14 ↔ +4/-4）、torso 微摆
  18→24 回浮归位：全轴回 BASE（endTick 同值闭环）
endTick=24，stopTick=26，isLoop=true。主轴：rightArm.roll / body.y / torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 抱脉桩基位：马步沉身、双拳收腰侧、含胸蓄劲。BASE 枚举全部 part.axis。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.08, z=-0.02),
    head=dict(pitch=+7, yaw=0),
    torso=dict(pitch=+8, yaw=0, roll=0),
    rightArm=dict(pitch=-12, yaw=-14, roll=+8, bend=96, axis=180),
    leftArm=dict(pitch=-12, yaw=+14, roll=-8, bend=96, axis=180),
    leftLeg=dict(pitch=-10, bend=22, z=-0.06),
    rightLeg=dict(pitch=+9, bend=20, z=+0.05),
)

POSE = {
    0: BASE,
    # 提息：吸气微浮、双拳微抬。
    3: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.065, z=-0.02),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+7, yaw=0, roll=0),
        rightArm=dict(pitch=-15, yaw=-14, roll=+9, bend=98, axis=180),
        leftArm=dict(pitch=-15, yaw=+14, roll=-9, bend=98, axis=180),
    ),
    # 提息顶点：吸满悬持。
    6: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.05, z=-0.018),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+6, yaw=0, roll=0),
        rightArm=dict(pitch=-18, yaw=-13, roll=+10, bend=100, axis=180),
        leftArm=dict(pitch=-18, yaw=+13, roll=-10, bend=100, axis=180),
    ),
    # 灌压：呼气沉桩、拳收紧、俯首。
    9: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.095, z=-0.024),
        head=dict(pitch=+9, yaw=0),
        torso=dict(pitch=+9, yaw=0, roll=0),
        rightArm=dict(pitch=-10, yaw=-15, roll=+11, bend=99, axis=180),
        leftArm=dict(pitch=-10, yaw=+15, roll=-11, bend=99, axis=180),
    ),
    # 沉桩底点：最低位、真元灌注开始震颤。
    12: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.11, z=-0.028),
        head=dict(pitch=+10, yaw=-1),
        torso=dict(pitch=+10, yaw=-2, roll=+1),
        rightArm=dict(pitch=-9, yaw=-16, roll=+14, bend=98, axis=180),
        leftArm=dict(pitch=-9, yaw=+16, roll=-14, bend=98, axis=180),
    ),
    # 底点震颤 A→B：双拳 roll 反向脉冲、torso 微摆。
    15: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.105, z=-0.026),
        head=dict(pitch=+9.5, yaw=+1),
        torso=dict(pitch=+9.5, yaw=+2, roll=-1),
        rightArm=dict(pitch=-10, yaw=-15, roll=+4, bend=97, axis=180),
        leftArm=dict(pitch=-10, yaw=+15, roll=-4, bend=97, axis=180),
    ),
    # 震颤收束：脉冲衰减。
    18: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.098, z=-0.024),
        head=dict(pitch=+8.5, yaw=0),
        torso=dict(pitch=+9, yaw=0, roll=0),
        rightArm=dict(pitch=-11, yaw=-14.5, roll=+10, bend=97, axis=180),
        leftArm=dict(pitch=-11, yaw=+14.5, roll=-10, bend=97, axis=180),
    ),
    # 回浮：缓缓抬回基位。
    21: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.088, z=-0.021),
        head=dict(pitch=+7.5, yaw=0),
        torso=dict(pitch=+8.3, yaw=0, roll=0),
        rightArm=dict(pitch=-11.5, yaw=-14, roll=+9, bend=96.5, axis=180),
        leftArm=dict(pitch=-11.5, yaw=+14, roll=-9, bend=96.5, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    24: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="baomai_full_power_charge",
        description=(
            "P3 全力一击蓄力段专属（isLoop 24t，解除 windup_charge 借用）：抱脉"
            "沉桩呼吸循环——提息微浮（body.y -0.08→-0.05 / 拳抬 -18）→ 灌压沉桩"
            "（y -0.11 / 俯首 +10）→ 底点双拳 roll 交替震颤（+14/-14 ↔ +4/-4）→ "
            "回浮归位。全轴 0/24 同值闭环；StopAnim 双退出路径见 full_power_emit.rs。"
        ),
        end_tick=24,
        stop_tick=26,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
