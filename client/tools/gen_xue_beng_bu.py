#!/usr/bin/env python3
"""xue_beng_bu —— 血崩步：压桩蹬地→疾步窜出→刹步收（P3 批次三，借用解除）。

通道核验（P3 第一性原理，2026-07-19）：`resolve_xue_beng_bu`
（server/src/cultivation/burst_meridian.rs:457）resolver 内 `insert_casting`
（:499-508，duration=cast_ticks=6，真实 Casting 窗可打断）+ 位移 t0 前置结算
（:519-522 服务器权威 Position 前推 4 格）——与 tie_shan_kao 同型混合通道 →
**三段式**，endTick = cast(6) + recovery 6 = 12 ∈ [10,14]。

借用解除：原 `XUE_BENG_BU_ANIM_ID = "bong:beng_quan"`（burst_meridian.rs:65，
位移招播出拳属语义错位）→ 专属 `bong:xue_beng_bu`。母题「疾步窜出」：起跑式
压桩 + 双臂后摆拖尾 + 剪步前窜——全程无出拳、无靠撞，与崩拳/贴山靠彻底区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   压桩蓄步（body.y -0.12 下蹲 / torso.pitch +18 前倾 /
                     双臂后摆）
  strike       4→6   蹬地窜出（body.z +0.40 / torso.pitch +26 / 双腿剪步 /
                     双臂拖尾 +55），顶点 = tick 6（cast 完成 = 位移落定）
  recovery     6→12  刹步直身（经 t8 制动帧）→ 归中立（INOUTSINE）
endTick=12，stopTick=14，非循环。主打击轴：body.z / torso.pitch / rightArm.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：站姿微压。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.02, z=0.0),
        head=dict(pitch=+2),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=+10, yaw=-4, roll=+2, bend=18, axis=180),
        leftArm=dict(pitch=+8, yaw=+4, roll=-2, bend=16, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.04),
        rightLeg=dict(pitch=+6, bend=10, z=+0.03),
    ),
    # 压桩中段：重心快速下沉、双臂开始后摆。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=-0.08, z=-0.04),
        head=dict(pitch=+5),
        torso=dict(pitch=+13, yaw=+3),
        rightArm=dict(pitch=+26, yaw=-8, roll=+4, bend=24, axis=180),
        leftArm=dict(pitch=+22, yaw=+8, roll=-4, bend=22, axis=180),
        leftLeg=dict(pitch=-12, bend=22, z=-0.06),
        rightLeg=dict(pitch=+10, bend=20, z=+0.04),
    ),
    # 蓄步底点：起跑式深蹲、双臂后摆到位。
    4: dict(
        easing="OUTQUAD",
        body=dict(y=-0.12, z=-0.06),
        head=dict(pitch=+7),
        torso=dict(pitch=+18, yaw=+5),
        rightArm=dict(pitch=+40, yaw=-10, roll=+6, bend=28, axis=180),
        leftArm=dict(pitch=+35, yaw=+10, roll=-6, bend=26, axis=180),
        leftLeg=dict(pitch=-14, bend=30, z=-0.08),
        rightLeg=dict(pitch=+12, bend=26, z=+0.05),
    ),
    # 窜出中段：蹬地离位、腿剪开。
    5: dict(
        easing="INQUAD",
        body=dict(y=-0.07, z=+0.22),
        head=dict(pitch=+5),
        torso=dict(pitch=+23, yaw=+2),
        rightArm=dict(pitch=+48, yaw=-14, roll=+7, bend=22, axis=180),
        leftArm=dict(pitch=+44, yaw=+14, roll=-7, bend=20, axis=180),
        leftLeg=dict(pitch=-26, bend=16, z=-0.11),
        rightLeg=dict(pitch=+30, bend=28, z=+0.07),
    ),
    # 窜出顶点（tick 6 = cast 完成 = 位移落定）：身体前射、双臂拖尾。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.04, z=+0.40),
        head=dict(pitch=+4),
        torso=dict(pitch=+26, yaw=0),
        rightArm=dict(pitch=+55, yaw=-16, roll=+8, bend=18, axis=180),
        leftArm=dict(pitch=+50, yaw=+16, roll=-8, bend=16, axis=180),
        leftLeg=dict(pitch=-30, bend=14, z=-0.12),
        rightLeg=dict(pitch=+34, bend=30, z=+0.08),
    ),
    # 刹步：重心接住、躯干直起一半、双臂回落。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=-0.05, z=+0.16),
        head=dict(pitch=+3),
        torso=dict(pitch=+14, yaw=-2),
        rightArm=dict(pitch=+26, yaw=-10, roll=+5, bend=24, axis=180),
        leftArm=dict(pitch=+22, yaw=+10, roll=-5, bend=22, axis=180),
        leftLeg=dict(pitch=-18, bend=24, z=-0.08),
        rightLeg=dict(pitch=+18, bend=22, z=+0.06),
    ),
    # 收步：近直立。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=-0.02, z=+0.05),
        head=dict(pitch=+1),
        torso=dict(pitch=+5, yaw=-1),
        rightArm=dict(pitch=+8, yaw=-4, roll=+2, bend=12, axis=180),
        leftArm=dict(pitch=+6, yaw=+4, roll=-2, bend=10, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.04),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 归中立。
    12: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="xue_beng_bu",
        description=(
            "P3 血崩步专属（12t 三段式，解除 beng_quan 借用）：起跑式压桩"
            "（body.y -0.12 / torso.pitch +18 / 双臂后摆）→ 蹬地窜出（body.z "
            "+0.40 / 双腿剪步 / 双臂拖尾 +55，顶点=t6 cast 完成）→ 刹步直身。"
            "全程无出拳——步法招与崩拳/贴山靠彻底区分。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
