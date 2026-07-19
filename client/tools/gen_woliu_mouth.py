#!/usr/bin/env python3
"""woliu_mouth —— 涡口：拧身列位→探爪开口虹吸（P3 批次三，借用解除）。

通道核验（P3 第一性原理，2026-07-19）：`cast_mouth`
（server/src/combat/woliu_v2/skills.rs:184）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算（零 Casting/零引导窗），cast_ticks=6 为元数据——短 cast
按 P1 短招惯例走**三段式**标准域（与 zhenmai/tuike 同判，instant 分类仅收
cast≥40 无窗招），endTick = cast(6) + recovery 6 = 12 ∈ [10,14]。

借用解除：原 visual_for `"bong:palm_thrust"`（通用推掌 105KF 模板）→ 专属
`bong:woliu_mouth`。母题「开涡口虹吸」：拧身把双手列到右侧 → 右爪前探开口、
左手撕回腰际（虹吸对拉）→ 探爪保持微颤（远端吸取）→ 收。前爪后拉的**对拉
张力**是本招签名，与 vacuum_palm（刺-回拖）/ burst（对称外弹）区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   拧身列位（torso.yaw +14 / 双手右侧聚）
  strike       4→8   探爪开口（右臂 -78 前探 / 左手撕回腰际 / body.z +0.16），
                     顶点 = tick 6（cast 完成），6→8 开口定格微颤（hold 归 strike）
  recovery     8→12  对拉松开、收臂归中立（INOUTSINE）
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+3, yaw=+4),
        rightArm=dict(pitch=-28, yaw=+4, roll=0, bend=48, axis=180),
        leftArm=dict(pitch=-24, yaw=-4, roll=0, bend=50, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 拧身中段。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=+4, yaw=+6),
        torso=dict(pitch=+5, yaw=+10),
        rightArm=dict(pitch=-38, yaw=+14, roll=-6, bend=62, axis=180),
        leftArm=dict(pitch=-34, yaw=-12, roll=+6, bend=66, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.04),
        rightLeg=dict(pitch=+6, bend=9, z=+0.03),
    ),
    # 列位顶点：双手聚右侧腰胸之间。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.03, z=-0.04),
        head=dict(pitch=+5, yaw=+8),
        torso=dict(pitch=+6, yaw=+14),
        rightArm=dict(pitch=-45, yaw=+18, roll=-8, bend=70, axis=180),
        leftArm=dict(pitch=-40, yaw=-14, roll=+8, bend=76, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.04),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 探爪顶点（tick 6 = cast 完成）：右爪前探开口、左手撕回、身前送。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.01, z=+0.16),
        head=dict(pitch=+3, yaw=-4),
        torso=dict(pitch=+10, yaw=-12),
        rightArm=dict(pitch=-78, yaw=-6, roll=-18, bend=6, axis=180),
        leftArm=dict(pitch=+30, yaw=-25, roll=+10, bend=55, axis=180),
        leftLeg=dict(pitch=-16, bend=16, z=-0.08),
        rightLeg=dict(pitch=+13, bend=14, z=+0.05),
    ),
    # 开口定格微颤（虹吸远端）：爪保持全探、roll 微拧。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=-0.01, z=+0.14),
        head=dict(pitch=+3, yaw=-3),
        torso=dict(pitch=+9, yaw=-10),
        rightArm=dict(pitch=-76, yaw=-8, roll=-8, bend=10, axis=180),
        leftArm=dict(pitch=+26, yaw=-22, roll=+9, bend=58, axis=180),
        leftLeg=dict(pitch=-15, bend=15, z=-0.07),
        rightLeg=dict(pitch=+12, bend=13, z=+0.05),
    ),
    # 松开对拉：收臂半程。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=-0.01, z=+0.06),
        head=dict(pitch=+2, yaw=-1),
        torso=dict(pitch=+5, yaw=-4),
        rightArm=dict(pitch=-36, yaw=-4, roll=-4, bend=28, axis=180),
        leftArm=dict(pitch=+10, yaw=-10, roll=+4, bend=30, axis=180),
        leftLeg=dict(pitch=-9, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=9, z=+0.03),
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
        name="woliu_mouth",
        description=(
            "P3 涡口专属（12t 三段式，解除 palm_thrust 借用）：拧身列位（torso.yaw "
            "+14 双手右聚）→ 右爪前探开口+左手撕回腰际对拉（pitch -78 / body.z "
            "+0.16，顶点=t6 cast 完成，6→8 定格微颤）→ 松开收臂归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
