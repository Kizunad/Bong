#!/usr/bin/env python3
"""woliu_burst —— 瞬涡：交臂压涡→双掌弹开退步（P3 批次三，借用解除）。

通道核验（P3 第一性原理，2026-07-19）：`cast_burst`
（server/src/combat/woliu_v2/skills.rs:175）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算（零 Casting/零引导窗，`push_skill_cast_started_sync`
因无 Casting 直接 return），cast_ticks=1 → **瞬发域**（[6,12]），endTick=8。

借用解除：原 visual_for `"bong:palm_strike"`（通用单掌推击 6t 模板 81KF）→
专属 `bong:woliu_burst`。母题「弹反」：双臂交叉紧压（把来劲压进涡里）→ 双掌
猛然外弹 + 小退步卸力——**双掌对称外弹 + 后退**，与单掌前推的 palm_strike、
前刺的 vacuum_palm 完全反向。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   交臂紧压（双臂 X 交叉 bend 110 / 微沉）
  strike       2→4   双掌外弹（yaw ∓45 甩开 / body.z -0.10 退步 / 身浮 +0.02），
                     顶点 = tick 4
  recovery     4→8   卸力落臂归中立（INOUTSINE）
endTick=8，stopTick=10，非循环。主打击轴：rightArm.yaw / leftArm.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手即快速交臂（瞬发招压缩蓄势）。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.01, z=-0.02),
        head=dict(pitch=+4),
        torso=dict(pitch=+4, yaw=0),
        rightArm=dict(pitch=-50, yaw=+24, roll=-8, bend=100, axis=180),
        leftArm=dict(pitch=-46, yaw=-22, roll=+8, bend=102, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 压涡底点：交叉压到最紧、身体微沉。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=-0.03, z=-0.04),
        head=dict(pitch=+6),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=-55, yaw=+28, roll=-10, bend=110, axis=180),
        leftArm=dict(pitch=-51, yaw=-26, roll=+10, bend=112, axis=180),
        leftLeg=dict(pitch=-7, bend=11, z=-0.03),
        rightLeg=dict(pitch=+6, bend=10, z=+0.02),
    ),
    # 弹开中段：双掌拆开加速外甩。
    3: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=-0.07),
        head=dict(pitch=+1),
        torso=dict(pitch=+1, yaw=0),
        rightArm=dict(pitch=-40, yaw=-14, roll=+8, bend=60, axis=180),
        leftArm=dict(pitch=-36, yaw=+16, roll=-8, bend=62, axis=180),
        leftLeg=dict(pitch=-9, bend=12, z=-0.05),
        rightLeg=dict(pitch=+8, bend=11, z=+0.03),
    ),
    # 弹反顶点（tick 4）：双掌外弹到极限、小退步卸力。
    4: dict(
        easing="INQUAD",
        body=dict(y=+0.02, z=-0.10),
        head=dict(pitch=-2),
        torso=dict(pitch=-4, yaw=0),
        rightArm=dict(pitch=-25, yaw=-45, roll=+18, bend=20, axis=180),
        leftArm=dict(pitch=-22, yaw=+45, roll=-18, bend=22, axis=180),
        leftLeg=dict(pitch=-11, bend=13, z=-0.06),
        rightLeg=dict(pitch=+10, bend=12, z=+0.04),
    ),
    # 卸力中段。
    6: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=-0.04),
        head=dict(pitch=-1),
        torso=dict(pitch=-1, yaw=0),
        rightArm=dict(pitch=-12, yaw=-20, roll=+8, bend=12, axis=180),
        leftArm=dict(pitch=-10, yaw=+20, roll=-8, bend=14, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 归中立。
    8: dict(
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
        name="woliu_burst",
        description=(
            "P3 瞬涡弹反专属（8t 瞬发，解除 palm_strike 借用）：交臂紧压"
            "（bend 110 X 臂微沉）→ 双掌对称外弹（yaw ∓45 / body.z -0.10 退步，"
            "顶点=t4）→ 卸力归中立。双掌外弹+后退，与单掌前推完全反向。"
        ),
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
