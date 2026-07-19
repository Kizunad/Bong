#!/usr/bin/env python3
"""woliu_vacuum_lock —— 真空锁：开臂张笼→合拢下压锁困（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_vacuum_lock`
（server/src/combat/woliu_v2/skills.rs:229）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算（Slowed 0.8/3s 经 `apply_v3_runtime_effects`）+ 插
`VortexV2State`，零 Casting/零引导窗，cast_ticks=8 为元数据——短 cast 走
**三段式**标准域，endTick = cast(8) + recovery 5 = 13 ∈ [12,16]（原 10t/32KF
快闪不达标，CAST_ALIGNMENT_ALLOWLIST 条目，本批重制后删除）。id 不变原地重制
（woliu.pull 已在本批拿到专属 woliu_pull，此后本动画为 vacuum_lock 独占）。

母题「合笼锁困」：双臂大开成笼 → 猛然合拢并下压（罩住目标抽空）→ 锁定定格 →
缓开收。**开-合-压**与涡引（前探-后拽）动向相反，与吸掌（单掌刺回）区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   开臂张笼（双臂 -70 外张 yaw ∓50）
  strike       4→8   合拢下压（yaw 收 ∓6 / body.y -0.07 / torso.pitch +10），
                     顶点 = tick 8（cast 完成 = 锁成）
  recovery     8→13  锁定松开、起身归中立（INOUTSINE）
endTick=13，stopTick=15，非循环。主打击轴：rightArm.yaw / leftArm.yaw / body.y。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+2, yaw=0),
        rightArm=dict(pitch=-35, yaw=-14, roll=+4, bend=36, axis=180),
        leftArm=dict(pitch=-32, yaw=+14, roll=-4, bend=38, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 张笼中段。
    2: dict(
        easing="OUTSINE",
        body=dict(y=+0.005, z=-0.01),
        head=dict(pitch=+1, yaw=0),
        torso=dict(pitch=+1, yaw=0),
        rightArm=dict(pitch=-55, yaw=-34, roll=+10, bend=26, axis=180),
        leftArm=dict(pitch=-52, yaw=+34, roll=-10, bend=28, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 张笼顶点：双臂大开、笼口最大。
    4: dict(
        easing="OUTSINE",
        body=dict(y=+0.01, z=-0.02),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-70, yaw=-50, roll=+16, bend=20, axis=180),
        leftArm=dict(pitch=-66, yaw=+50, roll=-16, bend=22, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.02),
    ),
    # 合拢中段：臂扫向中线。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.03, z=0.0),
        head=dict(pitch=+4, yaw=0),
        torso=dict(pitch=+5, yaw=0),
        rightArm=dict(pitch=-56, yaw=-24, roll=+15, bend=38, axis=180),
        leftArm=dict(pitch=-52, yaw=+24, roll=-15, bend=40, axis=180),
        leftLeg=dict(pitch=-9, bend=14, z=-0.04),
        rightLeg=dict(pitch=+8, bend=12, z=+0.03),
    ),
    # 锁困顶点（tick 8 = cast 完成）：合拢近交、下压沉身。
    8: dict(
        easing="INQUAD",
        body=dict(y=-0.07, z=0.0),
        head=dict(pitch=+7, yaw=0),
        torso=dict(pitch=+10, yaw=0),
        rightArm=dict(pitch=-45, yaw=-6, roll=+14, bend=55, axis=180),
        leftArm=dict(pitch=-41, yaw=+6, roll=-14, bend=58, axis=180),
        leftLeg=dict(pitch=-11, bend=20, z=-0.05),
        rightLeg=dict(pitch=+10, bend=18, z=+0.04),
    ),
    # 锁定定格余振：压劲驻留。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=-0.055, z=0.0),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-40, yaw=-8, roll=+10, bend=50, axis=180),
        leftArm=dict(pitch=-36, yaw=+8, roll=-10, bend=52, axis=180),
        leftLeg=dict(pitch=-9, bend=16, z=-0.04),
        rightLeg=dict(pitch=+8, bend=14, z=+0.03),
    ),
    # 归中立。
    13: dict(
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
        name="woliu_vacuum_lock",
        description=(
            "P3 真空锁重制（13t 三段式，原 10t 快闪出 allowlist）：开臂张笼"
            "（yaw ∓50 大开）→ 合拢下压锁困（yaw 收 ∓6 / body.y -0.07，顶点=t8 "
            "cast 完成=锁成）→ 定格松开归中立。"
        ),
        end_tick=13,
        stop_tick=15,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
