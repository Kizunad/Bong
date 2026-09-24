#!/usr/bin/env python3
"""ni_mai_hu_ti —— 逆脉护体：交臂引气→结印压封→定桩收（P3 批次三，缺失补齐）。

通道核验（P3 第一性原理，2026-07-19）：`resolve_ni_mai_hu_ti`
（server/src/cultivation/burst_meridian.rs:559）resolver 内 `insert_casting`
（:594-603，duration=cast_ticks=12，真实 Casting 窗可打断）+ 减伤 buff t0 前置
（ApplyStatusEffectIntent DamageReduction 0.35 / 60t）——burst_meridian 家族同型
混合通道 → **三段式**，endTick = cast(12) + recovery 4 = 16 ∈ [16,20]。

缺失补齐：原 `anim_id: None`（burst_meridian.rs:637，完全不发 PlayAnim，玩家
零姿态反馈，MISSING_ANIM_ALLOWLIST 条目）→ 专属 `bong:ni_mai_hu_ti`（server
新增常量 NI_MAI_HU_TI_ANIM_ID，本批同步删 allowlist 条目）。母题「护体结印」：
双臂胸前交叉引气 → 猛然沉桩双掌外压封身 + 逆流震颤——防御姿态，与本系两攻击
招（拳/靠）及步法全部区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→6   交臂引气（双臂交叉上提 / 俯首 / 微沉）
  strike       6→12  沉桩压封（body.y -0.10 / 双掌外压下按 / 逆流震颤 roll
                     交替 t8/t10），顶点 = tick 12（cast 完成 = 护罩成型）
  recovery     12→16 起桩定势归中立（INOUTSINE）
endTick=16，stopTick=18，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
body.y。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：双手自然位、气沉。
    0: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+3),
        torso=dict(pitch=+3, yaw=0, roll=0),
        rightArm=dict(pitch=-20, yaw=+8, roll=0, bend=40, axis=180),
        leftArm=dict(pitch=-18, yaw=-8, roll=0, bend=42, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 引气中段：双臂上提开始交叉。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.01),
        head=dict(pitch=+6),
        torso=dict(pitch=+5, yaw=0, roll=0),
        rightArm=dict(pitch=-48, yaw=+18, roll=-6, bend=52, axis=180),
        leftArm=dict(pitch=-44, yaw=-16, roll=+6, bend=56, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.03),
        rightLeg=dict(pitch=+6, bend=9, z=+0.02),
    ),
    # 交臂顶点：双臂胸前成 X、俯首闭气。
    6: dict(
        easing="OUTSINE",
        body=dict(y=-0.03, z=-0.02),
        head=dict(pitch=+9),
        torso=dict(pitch=+7, yaw=0, roll=0),
        rightArm=dict(pitch=-70, yaw=+26, roll=-10, bend=62, axis=180),
        leftArm=dict(pitch=-66, yaw=-24, roll=+10, bend=66, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.04),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 压封发力：沉桩、双掌拆开外压（逆流第一震）。
    8: dict(
        easing="INQUAD",
        body=dict(y=-0.07, z=0.0),
        head=dict(pitch=+11),
        torso=dict(pitch=+7, yaw=0, roll=+4),
        rightArm=dict(pitch=-42, yaw=-8, roll=+8, bend=44, axis=180),
        leftArm=dict(pitch=-38, yaw=+10, roll=-8, bend=48, axis=180),
        leftLeg=dict(pitch=-10, bend=18, z=-0.05),
        rightLeg=dict(pitch=+9, bend=16, z=+0.04),
    ),
    # 压封续劲：掌位继续外撑下按（逆流第二震，roll 反向）。
    10: dict(
        easing="INQUAD",
        body=dict(y=-0.09, z=0.0),
        head=dict(pitch=+12),
        torso=dict(pitch=+6, yaw=0, roll=-3),
        rightArm=dict(pitch=-28, yaw=-22, roll=+14, bend=36, axis=180),
        leftArm=dict(pitch=-24, yaw=+24, roll=-14, bend=40, axis=180),
        leftLeg=dict(pitch=-11, bend=22, z=-0.06),
        rightLeg=dict(pitch=+10, bend=20, z=+0.05),
    ),
    # 封身顶点（tick 12 = cast 完成 = 护罩成型）：马步定桩、双掌外压到底。
    12: dict(
        easing="INQUAD",
        body=dict(y=-0.10, z=0.0),
        head=dict(pitch=+12),
        torso=dict(pitch=+6, yaw=0, roll=0),
        rightArm=dict(pitch=-20, yaw=-30, roll=+18, bend=30, axis=180),
        leftArm=dict(pitch=-16, yaw=+32, roll=-18, bend=34, axis=180),
        leftLeg=dict(pitch=-12, bend=24, z=-0.06),
        rightLeg=dict(pitch=+11, bend=22, z=+0.05),
    ),
    # 起桩：护罩已成、身形松半分。
    14: dict(
        easing="INOUTSINE",
        body=dict(y=-0.05, z=0.0),
        head=dict(pitch=+6),
        torso=dict(pitch=+3, yaw=0, roll=0),
        rightArm=dict(pitch=-10, yaw=-14, roll=+8, bend=16, axis=180),
        leftArm=dict(pitch=-8, yaw=+16, roll=-8, bend=18, axis=180),
        leftLeg=dict(pitch=-6, bend=12, z=-0.03),
        rightLeg=dict(pitch=+5, bend=10, z=+0.02),
    ),
    # 归中立。
    16: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
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
        name="ni_mai_hu_ti",
        description=(
            "P3 逆脉护体专属（16t 三段式，补齐 anim_id: None 缺口）：双臂胸前"
            "交叉引气（俯首 X 臂）→ 沉桩双掌外压封身（body.y -0.10 / yaw ∓30 外撑 / "
            "roll +4/-3 逆流震颤，顶点=t12 cast 完成=护罩成型）→ 起桩归中立。"
            "防御结印姿态，与本系拳/靠/步全部区分。"
        ),
        end_tick=16,
        stop_tick=18,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
