#!/usr/bin/env python3
"""woliu_pull —— 涡引：前探双爪→扣抓撕拽坐身（P3 批次三，去共用）。

通道核验（P3 第一性原理，2026-07-19）：`cast_pull`
（server/src/combat/woliu_v2/skills.rs:193）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算（零 Casting/零引导窗，目标位移在 `emit_cast_events`
同步应用），cast_ticks=5 为元数据——短 cast 走**三段式**标准域，
endTick = cast(5) + recovery 6 = 11 ∈ [9,13]。

去共用：原 visual_for `"bong:woliu_vacuum_lock"`（与进阶真空锁共用一条）→
专属 `bong:woliu_pull`。母题「扣抓撕拽」：双爪前上探出 → 扣死猛然撕拽回身、
重心后坐拧腰（把目标拖过来的反作用力写进全身）。**前探-后拽**的大位移往返是
本招签名，与 vacuum_lock（开臂合拢下压锁困）动向相反。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   前探扣抓（双臂 -85 前上伸 / body.z +0.06）
  strike       3→5   撕拽回身（双臂拉回腰腹 bend 85 / body.z -0.14 后坐 /
                     torso.yaw +16 拧腰），顶点 = tick 5（cast 完成）
  recovery     5→11  松劲起身归中立（INOUTSINE）
endTick=11，stopTick=13，非循环。主打击轴：rightArm.pitch / body.z / torso.yaw。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+3, yaw=0),
        rightArm=dict(pitch=-35, yaw=-4, roll=0, bend=40, axis=180),
        leftArm=dict(pitch=-32, yaw=+4, roll=0, bend=42, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 前探中段：双臂伸出过半。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=+0.04),
        head=dict(pitch=+4, yaw=0),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-66, yaw=-8, roll=-8, bend=24, axis=180),
        leftArm=dict(pitch=-62, yaw=+8, roll=+8, bend=26, axis=180),
        leftLeg=dict(pitch=-8, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=9, z=+0.03),
    ),
    # 扣抓顶点：双爪前上探满、指扣涡口。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=+0.06),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+8, yaw=-6),
        rightArm=dict(pitch=-85, yaw=-10, roll=-12, bend=15, axis=180),
        leftArm=dict(pitch=-80, yaw=+10, roll=+12, bend=17, axis=180),
        leftLeg=dict(pitch=-9, bend=11, z=-0.04),
        rightLeg=dict(pitch=+8, bend=10, z=+0.03),
    ),
    # 撕拽中段：爪收过胸、重心开始后坐。
    4: dict(
        easing="INQUAD",
        body=dict(y=-0.04, z=-0.05),
        head=dict(pitch=+3, yaw=+3),
        torso=dict(pitch=+2, yaw=+8),
        rightArm=dict(pitch=-52, yaw=+2, roll=-4, bend=55, axis=180),
        leftArm=dict(pitch=-48, yaw=-2, roll=+4, bend=58, axis=180),
        leftLeg=dict(pitch=-11, bend=16, z=-0.06),
        rightLeg=dict(pitch=+9, bend=14, z=+0.04),
    ),
    # 撕拽顶点（tick 5 = cast 完成）：拽回腰腹、后坐拧腰到底。
    5: dict(
        easing="INQUAD",
        body=dict(y=-0.06, z=-0.14),
        head=dict(pitch=+2, yaw=+5),
        torso=dict(pitch=-4, yaw=+16),
        rightArm=dict(pitch=-25, yaw=+8, roll=0, bend=85, axis=180),
        leftArm=dict(pitch=-21, yaw=-8, roll=0, bend=88, axis=180),
        leftLeg=dict(pitch=-13, bend=20, z=-0.07),
        rightLeg=dict(pitch=+11, bend=18, z=+0.05),
    ),
    # 松劲一段：拧腰回正一半。
    7: dict(
        easing="INOUTSINE",
        body=dict(y=-0.04, z=-0.08),
        head=dict(pitch=+2, yaw=+2),
        torso=dict(pitch=-1, yaw=+8),
        rightArm=dict(pitch=-16, yaw=+5, roll=0, bend=55, axis=180),
        leftArm=dict(pitch=-13, yaw=-5, roll=0, bend=58, axis=180),
        leftLeg=dict(pitch=-9, bend=14, z=-0.05),
        rightLeg=dict(pitch=+8, bend=12, z=+0.04),
    ),
    # 起身。
    9: dict(
        easing="INOUTSINE",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=+1, yaw=+1),
        torso=dict(pitch=+1, yaw=+3),
        rightArm=dict(pitch=-7, yaw=+2, roll=0, bend=24, axis=180),
        leftArm=dict(pitch=-5, yaw=-2, roll=0, bend=26, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.03),
        rightLeg=dict(pitch=+4, bend=7, z=+0.02),
    ),
    # 归中立。
    11: dict(
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
        name="woliu_pull",
        description=(
            "P3 涡引专属（11t 三段式，与 vacuum_lock 解除共用）：双爪前上探出扣抓"
            "（pitch -85 / body.z +0.06）→ 撕拽回身后坐拧腰（bend 85 / body.z "
            "-0.14 / torso.yaw +16，顶点=t5 cast 完成）→ 松劲起身归中立。"
        ),
        end_tick=11,
        stop_tick=13,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
