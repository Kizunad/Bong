#!/usr/bin/env python3
"""woliu_vacuum_palm —— 吸掌：拧腰收掌→刺出→抽真空回拖（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_vacuum_palm`
（server/src/combat/woliu_v2/skills.rs:211）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算 + 插 `VortexV2State`（turbulence=1.5），零 Casting/
零引导窗，cast_ticks=6 为元数据——短 cast 走**三段式**标准域，
endTick = cast(6) + recovery 6 = 12 ∈ [10,14]（原 8t/18KF 快闪不达标，
CAST_ALIGNMENT_ALLOWLIST 条目，本批重制后删除）。id 不变原地重制。

母题「抽真空」：拧腰收掌 → 单掌平刺 → **爪指收拢猛然回拖**（把空气抽走）——
刺-回拖的往返是本招签名；与涡口（探爪定格对拉）差在回拖收爪、与瞬涡（双掌
外弹）完全不同。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   拧腰收掌（torso.yaw +18 / 右掌收腰际）
  strike       4→6   平掌刺出（右臂 -75 / body.z +0.14 / torso.yaw -10），
                     顶点 = tick 6（cast 完成）
  recovery     6→12  抽真空回拖（t8 爪收半程 roll -20 / body.z 回落）→ 归中立
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / body.z / torso.yaw。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+3, yaw=+5),
        rightArm=dict(pitch=-20, yaw=+6, roll=0, bend=50, axis=180),
        leftArm=dict(pitch=-18, yaw=-6, roll=0, bend=44, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 拧腰中段。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=+3, yaw=+5),
        torso=dict(pitch=+4, yaw=+12),
        rightArm=dict(pitch=-16, yaw=+14, roll=+4, bend=66, axis=180),
        leftArm=dict(pitch=-26, yaw=-10, roll=-4, bend=48, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.04),
        rightLeg=dict(pitch=+6, bend=9, z=+0.03),
    ),
    # 收掌顶点：右掌收到腰际、拧腰到位。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.03, z=-0.05),
        head=dict(pitch=+4, yaw=+6),
        torso=dict(pitch=+5, yaw=+18),
        rightArm=dict(pitch=-12, yaw=+18, roll=+6, bend=80, axis=180),
        leftArm=dict(pitch=-32, yaw=-12, roll=-6, bend=52, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.04),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 刺出中段。
    5: dict(
        easing="INQUAD",
        body=dict(y=-0.01, z=+0.07),
        head=dict(pitch=+3, yaw=+1),
        torso=dict(pitch=+7, yaw=+2),
        rightArm=dict(pitch=-48, yaw=+4, roll=-4, bend=36, axis=180),
        leftArm=dict(pitch=-16, yaw=-14, roll=-4, bend=40, axis=180),
        leftLeg=dict(pitch=-12, bend=14, z=-0.06),
        rightLeg=dict(pitch=+10, bend=12, z=+0.04),
    ),
    # 刺出顶点（tick 6 = cast 完成）：平掌全刺、身前送。
    6: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=+0.14),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=+9, yaw=-10),
        rightArm=dict(pitch=-75, yaw=-8, roll=-8, bend=8, axis=180),
        leftArm=dict(pitch=+6, yaw=-16, roll=-3, bend=34, axis=180),
        leftLeg=dict(pitch=-15, bend=15, z=-0.07),
        rightLeg=dict(pitch=+12, bend=13, z=+0.05),
    ),
    # 抽真空回拖：爪指收拢、臂拉回半程、身回坐。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=-0.02, z=+0.03),
        head=dict(pitch=+3, yaw=0),
        torso=dict(pitch=+5, yaw=-2),
        rightArm=dict(pitch=-50, yaw=-2, roll=-20, bend=45, axis=180),
        leftArm=dict(pitch=-4, yaw=-10, roll=-2, bend=30, axis=180),
        leftLeg=dict(pitch=-10, bend=12, z=-0.05),
        rightLeg=dict(pitch=+8, bend=10, z=+0.03),
    ),
    # 落臂。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=-0.01, z=+0.01),
        head=dict(pitch=+1, yaw=0),
        torso=dict(pitch=+2, yaw=-1),
        rightArm=dict(pitch=-20, yaw=0, roll=-8, bend=20, axis=180),
        leftArm=dict(pitch=-2, yaw=-4, roll=-1, bend=14, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.02),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 归中立。
    12: dict(
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
        name="woliu_vacuum_palm",
        description=(
            "P3 吸掌重制（12t 三段式，原 8t 快闪出 allowlist）：拧腰收掌"
            "（torso.yaw +18）→ 平掌刺出（pitch -75 / body.z +0.14，顶点=t6 "
            "cast 完成）→ 抽真空回拖（roll -20 收爪）归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
