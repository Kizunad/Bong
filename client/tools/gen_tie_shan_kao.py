#!/usr/bin/env python3
"""tie_shan_kao —— 贴山靠：拧腰蓄靠→肩胯撞出→回桩（P3 批次三，借用解除）。

通道核验（P3 第一性原理，2026-07-19）：`resolve_tie_shan_kao`
（server/src/cultivation/burst_meridian.rs:339）在 resolver 内 `insert_casting`
（:389-398 → 共享 helper :725）插入**真实 `Casting` 窗**（duration=cast_ticks=10，
`tick_casts_or_interrupt` 三打断分支可打断、有 CastSync 进度条），效果（AttackIntent/
撕脉/扣 qi）t0 前置结算——与 P1 beng_quan / zhenmai 家族同型（resolver+Casting
混合），不满足瞬发结算型判据「无 Casting/无 timer/无打断窗」→ **三段式**，
endTick = cast(10) + recovery 6 = 16 ∈ [14,18]。

借用解除：原 `TIE_SHAN_KAO_ANIM_ID = "bong:beng_quan"`（burst_meridian.rs:51 借
崩拳出拳）→ 专属 `bong:tie_shan_kao`。母题「肩胯靠撞」：全程手臂**折叠贴身不
外伸**（靠劲在躯干），与崩拳的拳炸出、血崩步的疾步彻底区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→6   拧腰后坐蓄靠（torso.yaw -25 / body.z -0.10，右臂折抱胸前）
  strike       6→10  肩胯靠撞（torso.yaw -25→+28 甩转 / body.z +0.35 前撞 /
                     torso.roll +8 右肩领劲），顶点 = tick 10（cast 完成瞬间）
  recovery     10→16 撞势回弹（body.z 回落经 +0.12）→ 回桩归中立（INOUTSINE）
endTick=16，stopTick=18，非循环。主打击轴：torso.yaw / body.z / torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手浅桩：微沉、双臂收贴。
    0: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+4, yaw=-4, roll=0),
        rightArm=dict(pitch=-18, yaw=-6, roll=+4, bend=62, axis=180),
        leftArm=dict(pitch=-24, yaw=+10, roll=-6, bend=48, axis=180),
        leftLeg=dict(pitch=-8, bend=10, z=-0.04),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03),
    ),
    # 拧腰中段：重心后坐、右臂折得更紧（靠劲蓄在躯干）。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.04, z=-0.06),
        head=dict(pitch=+4, yaw=+8),
        torso=dict(pitch=+6, yaw=-16, roll=-3),
        rightArm=dict(pitch=-24, yaw=-14, roll=+8, bend=84, axis=180),
        leftArm=dict(pitch=-34, yaw=+16, roll=-8, bend=42, axis=180),
        leftLeg=dict(pitch=-12, bend=14, z=-0.06),
        rightLeg=dict(pitch=+9, bend=12, z=+0.04),
    ),
    # 蓄靠顶点：拧到 -25、身体压得最低，右肩收到最后位。
    6: dict(
        easing="OUTSINE",
        body=dict(y=-0.06, z=-0.10),
        head=dict(pitch=+6, yaw=+12),
        torso=dict(pitch=+8, yaw=-25, roll=-5),
        rightArm=dict(pitch=-28, yaw=-18, roll=+10, bend=100, axis=180),
        leftArm=dict(pitch=-40, yaw=+20, roll=-10, bend=38, axis=180),
        leftLeg=dict(pitch=-14, bend=16, z=-0.07),
        rightLeg=dict(pitch=+11, bend=14, z=+0.05),
    ),
    # 撞出中段：躯干甩转过中线、身体前送加速。
    8: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=+0.16),
        head=dict(pitch=+4, yaw=-4),
        torso=dict(pitch=+12, yaw=+6, roll=+3),
        rightArm=dict(pitch=-32, yaw=-20, roll=+8, bend=96, axis=180),
        leftArm=dict(pitch=+8, yaw=+14, roll=-6, bend=30, axis=180),
        leftLeg=dict(pitch=-20, bend=20, z=-0.10),
        rightLeg=dict(pitch=+16, bend=18, z=+0.06),
    ),
    # 撞击顶点（tick 10 = cast 完成）：右肩胯撞满、torso 甩到 +28。
    10: dict(
        easing="INQUAD",
        body=dict(y=+0.02, z=+0.35),
        head=dict(pitch=+6, yaw=-10),
        torso=dict(pitch=+16, yaw=+28, roll=+8),
        rightArm=dict(pitch=-36, yaw=-22, roll=+6, bend=92, axis=180),
        leftArm=dict(pitch=+22, yaw=+10, roll=-4, bend=24, axis=180),
        leftLeg=dict(pitch=-26, bend=26, z=-0.12),
        rightLeg=dict(pitch=+20, bend=22, z=+0.08),
    ),
    # 回弹中段：撞势卸掉一半、躯干开始回拧。
    13: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=+0.12),
        head=dict(pitch=+3, yaw=-4),
        torso=dict(pitch=+9, yaw=+12, roll=+3),
        rightArm=dict(pitch=-26, yaw=-14, roll=+5, bend=76, axis=180),
        leftArm=dict(pitch=+4, yaw=+12, roll=-5, bend=34, axis=180),
        leftLeg=dict(pitch=-16, bend=16, z=-0.08),
        rightLeg=dict(pitch=+12, bend=14, z=+0.05),
    ),
    # 归中立。
    16: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
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
        name="tie_shan_kao",
        description=(
            "P3 贴山靠专属（16t 三段式，解除 beng_quan 借用）：拧腰后坐蓄靠"
            "（torso.yaw -25 / body.z -0.10）→ 肩胯撞出（yaw +28 / body.z +0.35 / "
            "roll +8 右肩领劲，顶点=t10 cast 完成）→ 回弹归桩。手臂全程折叠贴身，"
            "靠劲在躯干——与崩拳出拳完全区分。"
        ),
        end_tick=16,
        stop_tick=18,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
