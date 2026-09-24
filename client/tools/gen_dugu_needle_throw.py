#!/usr/bin/env python3
"""dugu_needle_throw —— 凝针：耳侧引针→鞭甩掷出→随针目送（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`resolve_shoot_needle_skill`
（server/src/cultivation/dugu.rs:662）resolver 立即结算——同 tick 发
`ShootNeedleIntent`（:695）+ `insert_instant_cast`（:701 → :774，1t instant
标记 Casting 非可打断窗），cast_ticks=1 → **瞬发域**（[6,12]），endTick=10
（原 8t/35KF 快闪，附录 A C 级密度精修）。动画走事件旁路
`QiNeedleChargedEvent` → `emit_dugu_needle_visual_triggers` 凝针分支
（network/vfx_animation_trigger.rs，const ANIM_DUGU_NEEDLE_THROW 不变）。
id 不变原地重制；灌毒蛊本批已拿专属 dugu_infuse_poison，此后本动画为凝针独占。

母题「鞭甩掷针」：右手引针至耳侧（腕蓄）→ 小臂鞭甩向前弹指掷出 → 随针目送
半拍收臂。侧身单臂鞭甩，与灌毒（举针覆手，无掷出）完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   耳侧引针（右臂 -120 引至耳侧 / torso.yaw +16 侧身）
  strike       3→5   鞭甩掷出（pitch -70 / roll -15 腕弹 / body.z +0.12），
                     顶点 = tick 5
  recovery     5→10  随针目送、收臂归中立（INOUTSINE）
endTick=10，stopTick=12，非循环。主打击轴：rightArm.pitch / torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：侧身待发。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=-4),
        torso=dict(pitch=+2, yaw=+6),
        rightArm=dict(pitch=-45, yaw=-8, roll=+4, bend=60, axis=180),
        leftArm=dict(pitch=-25, yaw=+8, roll=-3, bend=36, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 引针中段。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=-0.03),
        head=dict(pitch=+3, yaw=-6),
        torso=dict(pitch=+3, yaw=+12),
        rightArm=dict(pitch=-95, yaw=-12, roll=+9, bend=66, axis=180),
        leftArm=dict(pitch=-32, yaw=+10, roll=-4, bend=38, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.02),
    ),
    # 引针顶点：针至耳侧、腕蓄到底、左臂前指瞄准。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.04),
        head=dict(pitch=+3, yaw=-7),
        torso=dict(pitch=+4, yaw=+16),
        rightArm=dict(pitch=-120, yaw=-14, roll=+12, bend=70, axis=180),
        leftArm=dict(pitch=-35, yaw=+12, roll=-5, bend=40, axis=180),
        leftLeg=dict(pitch=-8, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=9, z=+0.03),
    ),
    # 鞭甩中段：小臂加速过肩。
    4: dict(
        easing="INQUAD",
        body=dict(y=-0.01, z=+0.05),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=+5, yaw=+4),
        rightArm=dict(pitch=-92, yaw=-10, roll=0, bend=35, axis=180),
        leftArm=dict(pitch=-22, yaw=+10, roll=-4, bend=36, axis=180),
        leftLeg=dict(pitch=-11, bend=11, z=-0.05),
        rightLeg=dict(pitch=+9, bend=10, z=+0.04),
    ),
    # 掷出顶点（tick 5）：臂鞭直、腕弹 roll 反拧、身前送。
    5: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=+0.12),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+7, yaw=-8),
        rightArm=dict(pitch=-70, yaw=-8, roll=-15, bend=8, axis=180),
        leftArm=dict(pitch=-14, yaw=+9, roll=-3, bend=32, axis=180),
        leftLeg=dict(pitch=-13, bend=12, z=-0.06),
        rightLeg=dict(pitch=+11, bend=11, z=+0.04),
    ),
    # 随针目送：臂顺势下落、视线追针。
    7: dict(
        easing="INOUTSINE",
        body=dict(y=-0.01, z=+0.06),
        head=dict(pitch=+1, yaw=+2),
        torso=dict(pitch=+4, yaw=-4),
        rightArm=dict(pitch=-35, yaw=-4, roll=-8, bend=18, axis=180),
        leftArm=dict(pitch=-8, yaw=+6, roll=-2, bend=22, axis=180),
        leftLeg=dict(pitch=-8, bend=9, z=-0.04),
        rightLeg=dict(pitch=+7, bend=8, z=+0.03),
    ),
    # 归中立。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
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
        name="dugu_needle_throw",
        description=(
            "P3 凝针重制（10t 瞬发，原 8t 快闪密度补齐）：耳侧引针（pitch -120 / "
            "torso.yaw +16 侧身）→ 鞭甩掷出（pitch -70 / roll -15 腕弹 / body.z "
            "+0.12，顶点=t5）→ 随针目送归中立。"
        ),
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
