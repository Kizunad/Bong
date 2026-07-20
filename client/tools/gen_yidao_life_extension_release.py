#!/usr/bin/env python3
"""yidao_life_extension_release —— 续命术接引封定收势（P4）。

两段式 release 段（蓄力段见 gen_yidao_life_extension_loop.py，isLoop）。
引导自然完成且结算有效时由 server StopAnim(蓄力段)+PlayAnim(本段) 接力；
打断/无效完成只 StopAnim。

母题：接引落封。咏唱毕，对天接引手先向天再够一寸（把业力线拉满），随即
右臂自天顶纵向斩落引到患者身前与左手合拢，双掌交叠往下一封（封定续来的
命数），身体随封微顿，尔后双手缓缓松开归位。右臂「自天顶纵贯而下」的大
弧线是本收势辨识核心（对比排异散烟的横向对称开扇）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   拉满：右臂向天再够一寸（pitch -152→-165，头随仰）
  strike       4→8   引落合封：右臂纵贯斩落至身前与左手合拢（pitch -165→-52），
                     躯干前压 body.z 前送，双掌交叠下封，t8 = 封定顶点
  recovery     8→14  松手归中立（INOUTSINE，t11 中段帧）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.pitch / torso.pitch /
body.z。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段定格位（左手托丹、右臂对天）。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.02, z=-0.01),
        head=dict(pitch=-14, yaw=+4),
        torso=dict(pitch=-5, yaw=+3),
        rightArm=dict(pitch=-152, yaw=-8, roll=-6, bend=14, axis=180),
        leftArm=dict(pitch=-34, yaw=+12, roll=+6, bend=26, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.03),
        rightLeg=dict(pitch=+6, bend=7, z=+0.03),
    ),
    # 拉满：右臂向天再够一寸、头仰到底。
    4: dict(
        easing="OUTQUAD",
        body=dict(y=-0.005, z=-0.025),
        head=dict(pitch=-19, yaw=+3),
        torso=dict(pitch=-8, yaw=+2),
        rightArm=dict(pitch=-165, yaw=-5, roll=-10, bend=8, axis=180),
        leftArm=dict(pitch=-36, yaw=+13, roll=+7, bend=28, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+7, bend=8, z=+0.03),
    ),
    # 引落中段：右臂纵贯过顶斩落，目光随手下移。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.03, z=+0.02),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+2, yaw=+4),
        rightArm=dict(pitch=-104, yaw=-8, roll=+4, bend=20, axis=180),
        leftArm=dict(pitch=-42, yaw=+14, roll=+8, bend=30, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.03),
        rightLeg=dict(pitch=+8, bend=9, z=+0.03),
    ),
    # 封定顶点：双掌交叠身前下封、躯干前压一顿。
    8: dict(
        easing="OUTSINE",
        body=dict(y=-0.06, z=+0.07),
        head=dict(pitch=+12, yaw=+2),
        torso=dict(pitch=+10, yaw=+1),
        rightArm=dict(pitch=-52, yaw=-12, roll=+8, bend=34, axis=180),
        leftArm=dict(pitch=-48, yaw=+12, roll=+8, bend=32, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.03),
        rightLeg=dict(pitch=+9, bend=11, z=+0.03),
    ),
    # 松手中段。
    11: dict(
        easing="INOUTSINE",
        body=dict(y=-0.02, z=+0.02),
        head=dict(pitch=+4, yaw=+1),
        torso=dict(pitch=+4, yaw=0),
        rightArm=dict(pitch=-24, yaw=-8, roll=+3, bend=16, axis=180),
        leftArm=dict(pitch=-20, yaw=+8, roll=+3, bend=14, axis=180),
        leftLeg=dict(pitch=-4, bend=6, z=-0.02),
        rightLeg=dict(pitch=+4, bend=5, z=+0.02),
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
        name="yidao_life_extension_release",
        description=(
            "P4 续命术接引封定收势（14t 非循环）：anticipation 0→4 接引手向天"
            "拉满（pitch -152→-165），strike 4→8 右臂纵贯斩落合掌下封（-165→-52 / "
            "torso -8→+10 / body.z +0.07 前压），recovery 8→14 松手归中立。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
