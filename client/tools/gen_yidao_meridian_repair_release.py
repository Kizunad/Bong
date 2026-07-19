#!/usr/bin/env python3
"""yidao_meridian_repair_release —— 接经术收针收势（P4）。

两段式 release 段（蓄力段见 gen_yidao_meridian_repair_loop.py，isLoop）。
引导自然完成且结算有效（`complete_yidao_casts` 有效结算分支）时由 server
StopAnim(蓄力段)+PlayAnim(本段) 接力；打断/无效完成只 StopAnim，不播本段
（收势只奖励有效结算）。

母题：收针。最后一针按定（定针一按），随即右腕干脆上提把针拔出（提针
一挑，捻转 roll 甩净），身体自俯身直起，右手拂袖把针收回袖中归位。俯→立
的姿态迁移是本收势的辨识核心（其余 yidao 收势无此「直腰」大位移）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   定针一按：右腕沉、身体再俯半分（OUTQUAD 蓄）
  strike       4→8   提针直身：右臂上挑拔针（pitch -30→-96 / roll 捻甩
                     +8→+24），躯干由 +23 直起至 -2，t8 = 拔针顶点
  recovery     8→14  拂袖收针归中立（INOUTSINE，t11 中段帧）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.pitch / rightArm.roll /
torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段俯身持针位。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.10, z=+0.07),
        head=dict(pitch=+16, yaw=-3),
        torso=dict(pitch=+22, yaw=-4),
        rightArm=dict(pitch=-38, yaw=-10, roll=-6, bend=52, axis=180),
        leftArm=dict(pitch=-44, yaw=+16, roll=+4, bend=38, axis=180),
        leftLeg=dict(pitch=-8, bend=16, z=-0.03),
        rightLeg=dict(pitch=-7, bend=15, z=+0.03),
    ),
    # 定针一按：右腕沉到底、身体再俯半分（拔针前的蓄）。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=-0.125, z=+0.08),
        head=dict(pitch=+18, yaw=-3),
        torso=dict(pitch=+24, yaw=-5),
        rightArm=dict(pitch=-28, yaw=-10, roll=+8, bend=44, axis=180),
        leftArm=dict(pitch=-48, yaw=+14, roll=+5, bend=32, axis=180),
        leftLeg=dict(pitch=-9, bend=18, z=-0.03),
        rightLeg=dict(pitch=-8, bend=17, z=+0.03),
    ),
    # anticipation 末帧 / strike 起点。
    4: dict(
        easing="INQUAD",
        body=dict(y=-0.118, z=+0.078),
        head=dict(pitch=+17, yaw=-3),
        torso=dict(pitch=+23, yaw=-5),
        rightArm=dict(pitch=-30, yaw=-10, roll=+8, bend=46, axis=180),
        leftArm=dict(pitch=-47, yaw=+14, roll=+5, bend=33, axis=180),
        leftLeg=dict(pitch=-9, bend=17, z=-0.03),
        rightLeg=dict(pitch=-8, bend=16, z=+0.03),
    ),
    # 提针中段：针离皮、身体开始直起。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.05, z=+0.03),
        head=dict(pitch=+6, yaw=-1),
        torso=dict(pitch=+10, yaw=-2),
        rightArm=dict(pitch=-66, yaw=-8, roll=+16, bend=30, axis=180),
        leftArm=dict(pitch=-30, yaw=+10, roll=+2, bend=24, axis=180),
        leftLeg=dict(pitch=-5, bend=10, z=-0.02),
        rightLeg=dict(pitch=-4, bend=9, z=+0.02),
    ),
    # 拔针顶点：右臂上挑捻甩、身体直起带轻微后张。
    8: dict(
        easing="OUTSINE",
        body=dict(y=+0.01, z=-0.02),
        head=dict(pitch=-4, yaw=+2),
        torso=dict(pitch=-2, yaw=+2),
        rightArm=dict(pitch=-96, yaw=-4, roll=+24, bend=14, axis=180),
        leftArm=dict(pitch=-14, yaw=+6, roll=-2, bend=14, axis=180),
        leftLeg=dict(pitch=-3, bend=5, z=-0.02),
        rightLeg=dict(pitch=+3, bend=4, z=+0.02),
    ),
    # 拂袖中段：右手拂向袖口。
    11: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=-0.01),
        head=dict(pitch=-1, yaw=+1),
        torso=dict(pitch=-1, yaw=+1),
        rightArm=dict(pitch=-40, yaw=-10, roll=+8, bend=26, axis=180),
        leftArm=dict(pitch=-6, yaw=+3, roll=-1, bend=8, axis=180),
        leftLeg=dict(pitch=-2, bend=3, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
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
        name="yidao_meridian_repair_release",
        description=(
            "P4 接经术收针收势（14t 非循环）：anticipation 0→4 定针一按（俯身"
            "再沉），strike 4→8 提针直身（rightArm pitch -30→-96 / roll +8→+24 "
            "捻甩、torso +23→-2 直腰），recovery 8→14 拂袖收针归中立。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
