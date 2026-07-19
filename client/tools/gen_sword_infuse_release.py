#!/usr/bin/env python3
"""sword_infuse_release —— 注剑灌注成剑收势（P2 批次二后半）。

两段式 release 段（蓄力段见 gen_sword_infuse.py，isLoop）。灌注自然完成
（`sword_infuse_completion_tick` 成功分支）时由 server StopAnim(蓄力段)+
PlayAnim(本段) 接力；打断/失败分支只 StopAnim，不播本段（打断不奖励收势）。

母题：剑身一振。灌注成剑瞬间左掌按上刃身最后一按（定灌），随即右手把剑
向前上方一记干脆的挑振（真元入刃嗡鸣的物理外化），振定后落剑归位。短促
「按—振—落」，与蓄力段的绵长抚刃循环形成节奏对比。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   定灌一按：左掌按刃、双臂微沉（body.y→-0.05，OUTQUAD）
  strike       4→9   挑振：右臂前上方挑剑（pitch -74→-118 / roll -16→+18）、
                     torso.yaw -6→+10 送肩、body.z +0.14 前送（INQUAD），
                     t9 = 振定顶点
  recovery     9→14  落剑归中立（INOUTSINE，t11 中段帧）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.pitch / rightArm.roll /
torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段横剑位。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.03, z=-0.02),
        head=dict(pitch=+10, yaw=+2),
        torso=dict(pitch=+6, yaw=-6),
        rightArm=dict(pitch=-72, yaw=-22, roll=-12, bend=30, axis=180),
        leftArm=dict(pitch=-66, yaw=+26, roll=-6, bend=52, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03),
    ),
    # 定灌一按：左掌压刃、身体下沉（挑振前的蓄）。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=-0.05, z=-0.03),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+8, yaw=-8),
        rightArm=dict(pitch=-74, yaw=-23, roll=-16, bend=32, axis=180),
        leftArm=dict(pitch=-72, yaw=+14, roll=-8, bend=40, axis=180),
        leftLeg=dict(pitch=-8, bend=11, z=-0.03),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # anticipation 末帧 / strike 起点。
    4: dict(
        easing="INQUAD",
        body=dict(y=-0.045, z=-0.025),
        head=dict(pitch=+11, yaw=0),
        torso=dict(pitch=+7, yaw=-7),
        rightArm=dict(pitch=-75, yaw=-22, roll=-15, bend=31, axis=180),
        leftArm=dict(pitch=-70, yaw=+16, roll=-8, bend=42, axis=180),
        leftLeg=dict(pitch=-8, bend=11, z=-0.03),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 挑振中段：剑起、左掌让开。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=+0.06),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=+2, yaw=+2),
        rightArm=dict(pitch=-96, yaw=-16, roll=+2, bend=16, axis=180),
        leftArm=dict(pitch=-42, yaw=+26, roll=-10, bend=48, axis=180),
        leftLeg=dict(pitch=-10, bend=12, z=-0.04),
        rightLeg=dict(pitch=+9, bend=10, z=+0.04),
    ),
    # 振定顶点：剑挑至前上方定住（嗡鸣定格），送肩前压。
    9: dict(
        easing="OUTSINE",
        body=dict(y=+0.01, z=+0.14),
        head=dict(pitch=-6, yaw=-4),
        torso=dict(pitch=-2, yaw=+10),
        rightArm=dict(pitch=-118, yaw=-10, roll=+18, bend=6, axis=180),
        leftArm=dict(pitch=-26, yaw=+24, roll=-12, bend=38, axis=180),
        leftLeg=dict(pitch=-13, bend=14, z=-0.05),
        rightLeg=dict(pitch=+12, bend=12, z=+0.05),
    ),
    # 落剑中段。
    11: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=+0.06),
        head=dict(pitch=-2, yaw=-1),
        torso=dict(pitch=0, yaw=+4),
        rightArm=dict(pitch=-52, yaw=-14, roll=+6, bend=14, axis=180),
        leftArm=dict(pitch=-12, yaw=+12, roll=-6, bend=18, axis=180),
        leftLeg=dict(pitch=-6, bend=7, z=-0.02),
        rightLeg=dict(pitch=+6, bend=6, z=+0.02),
    ),
    # 归中立。
    14: dict(
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
        name="sword_infuse_release",
        description=(
            "P2 注剑灌注成剑收势（14t 非循环）：anticipation 0→4 定灌一按（左掌"
            "压刃 / body.y -0.05），strike 4→9 前上方挑振（rightArm pitch -75→-118 / "
            "roll -15→+18 / torso.yaw -7→+10 / body.z +0.14），recovery 9→14 落剑"
            "归中立。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
