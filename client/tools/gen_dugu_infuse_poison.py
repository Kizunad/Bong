#!/usr/bin/env python3
"""dugu_infuse_poison —— 灌毒蛊：举针凝视→覆手淬毒→腕封（P3 批次三，去共用）。

通道核验（P3 第一性原理，2026-07-19）：`resolve_infuse_dugu_poison_skill`
（server/src/cultivation/dugu.rs:714）resolver 立即结算——同 tick 发
`InfuseDuguPoisonIntent`（:741）+ `insert_instant_cast`（:746 → :774，
duration_ticks=1 的 instant 标记 Casting，非可打断窗），cast_ticks=1 →
**瞬发域**（[6,12]），endTick=10。动画走事件旁路：
`DuguObfuscationDisruptedEvent` → `emit_dugu_needle_visual_triggers` 灌毒分支
（network/vfx_animation_trigger.rs，本批改指 `ANIM_DUGU_INFUSE_POISON`）。

去共用：原与凝针共用 `bong:dugu_needle_throw`（两招动画字符串完全相同，仅
去重 id 1/2 区分，远观无法分辨「淬毒」与「射针」）→ 专属
`bong:dugu_infuse_poison`。母题「淬毒」：右手举针至面前凝视 → 左掌自上而下
覆过针身（灌毒抹拭）+ 右腕拧转 → 腕封收势。**无掷出动作**——与凝针的鞭甩
出手完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   举针凝视（右臂 -95 举至面前 / 俯首 +6 / 左手迎上）
  strike       3→6   覆手淬毒（左掌沿针身下抹 -85→-35 / 右腕 roll -12 拧针），
                     顶点 = tick 6（腕封 roll +22 快拧）
  recovery     6→10  双臂落定归中立（INOUTSINE）
endTick=10，stopTick=12，非循环。主打击轴：leftArm.pitch / rightArm.roll /
torso.yaw。
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
        rightArm=dict(pitch=-40, yaw=-6, roll=0, bend=55, axis=180),
        leftArm=dict(pitch=-20, yaw=+6, roll=0, bend=40, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.02),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 举针中段：右臂抬、左手开始迎上。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+4, yaw=-2),
        torso=dict(pitch=+3, yaw=+2),
        rightArm=dict(pitch=-75, yaw=-10, roll=+6, bend=60, axis=180),
        leftArm=dict(pitch=-55, yaw=+12, roll=-4, bend=62, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.02),
        rightLeg=dict(pitch=+4, bend=7, z=+0.02),
    ),
    # 举针凝视：针至面前，左掌悬于针上。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+6, yaw=-3),
        torso=dict(pitch=+4, yaw=+3),
        rightArm=dict(pitch=-95, yaw=-12, roll=+10, bend=65, axis=180),
        leftArm=dict(pitch=-85, yaw=+16, roll=-6, bend=58, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 覆手淬毒：左掌沿针身下抹、右腕反拧凝毒。
    5: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=0.0),
        head=dict(pitch=+7, yaw=-2),
        torso=dict(pitch=+5, yaw=+6),
        rightArm=dict(pitch=-92, yaw=-11, roll=-12, bend=63, axis=180),
        leftArm=dict(pitch=-52, yaw=+20, roll=-10, bend=66, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 腕封顶点（tick 6）：右腕快拧封毒、左掌抹到针尾。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=0.0),
        head=dict(pitch=+7, yaw=-1),
        torso=dict(pitch=+5, yaw=+7),
        rightArm=dict(pitch=-90, yaw=-10, roll=+22, bend=62, axis=180),
        leftArm=dict(pitch=-35, yaw=+22, roll=-12, bend=70, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 落臂中段。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+3, yaw=0),
        torso=dict(pitch=+3, yaw=+3),
        rightArm=dict(pitch=-45, yaw=-6, roll=+10, bend=40, axis=180),
        leftArm=dict(pitch=-16, yaw=+10, roll=-6, bend=32, axis=180),
        leftLeg=dict(pitch=-4, bend=6, z=-0.02),
        rightLeg=dict(pitch=+4, bend=5, z=+0.01),
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
        name="dugu_infuse_poison",
        description=(
            "P3 灌毒蛊专属（10t 瞬发，与凝针解除共用）：举针至面前凝视（右臂 -95 "
            "/ 俯首）→ 左掌沿针身下抹淬毒（-85→-35）+ 右腕拧转（roll -12→+22 "
            "腕封，顶点=t6）→ 落臂归中立。无掷出——与凝针鞭甩完全区分。"
        ),
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
