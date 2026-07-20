#!/usr/bin/env python3
"""yidao_mass_meridian_repair_release —— 群体接经共振收器收势（P4）。

两段式 release 段（蓄力段见 gen_yidao_mass_meridian_repair_loop.py，
isLoop）。引导自然完成且结算有效时由 server StopAnim(蓄力段)+PlayAnim(本段)
接力；打断/无效完成只 StopAnim。

母题：共振收器。环视扫毕回正，法器向天顶再举一寸催出最后一波共振
（catch），随即双臂对称把法器沉落抱定于胸前（沉腕一顿，body.y 下砸半分
让「共振落地」有重量），尔后抱器徐徐放平归位。双臂全程对称同步是本收势
与续命「单臂纵贯」的辨识分界；「高举→抱胸」的纵向沉落对比排异散烟的
横向开扇。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   催振：法器向天顶再举一寸（双臂 -148→-158，OUTQUAD 蓄）
  strike       3→7   沉落抱定：双臂对称沉落至胸前抱器（pitch -158→-64 /
                     bend 收满 52），body.y 下砸一顿，t7 = 抱定顶点
  recovery     7→12  放平归中立（INOUTSINE，t9 中段帧）
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
body.y。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段高举位（回正后）。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.03, z=0.0),
        head=dict(pitch=-7, yaw=0),
        torso=dict(pitch=-2, yaw=0),
        rightArm=dict(pitch=-148, yaw=-13, roll=-4, bend=12, axis=180),
        leftArm=dict(pitch=-148, yaw=+13, roll=+4, bend=12, axis=180),
        leftLeg=dict(pitch=-4, bend=7, z=-0.03),
        rightLeg=dict(pitch=+4, bend=6, z=+0.03),
    ),
    # 催振：法器向天顶再举一寸。
    3: dict(
        easing="OUTQUAD",
        body=dict(y=+0.01, z=-0.01),
        head=dict(pitch=-11, yaw=0),
        torso=dict(pitch=-4, yaw=0),
        rightArm=dict(pitch=-158, yaw=-11, roll=-6, bend=8, axis=180),
        leftArm=dict(pitch=-158, yaw=+11, roll=+6, bend=8, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.03),
    ),
    # 沉落中段：双臂对称落过面前。
    5: dict(
        easing="INQUAD",
        body=dict(y=-0.03, z=+0.01),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+2, yaw=0),
        rightArm=dict(pitch=-108, yaw=-12, roll=-2, bend=30, axis=180),
        leftArm=dict(pitch=-108, yaw=+12, roll=+2, bend=30, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03),
    ),
    # 抱定顶点：法器沉落抱于胸前、body.y 下砸一顿。
    7: dict(
        easing="OUTSINE",
        body=dict(y=-0.07, z=+0.02),
        head=dict(pitch=+8, yaw=0),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=-64, yaw=-14, roll=-6, bend=52, axis=180),
        leftArm=dict(pitch=-64, yaw=+14, roll=+6, bend=52, axis=180),
        leftLeg=dict(pitch=-8, bend=12, z=-0.03),
        rightLeg=dict(pitch=+8, bend=11, z=+0.03),
    ),
    # 放平中段。
    9: dict(
        easing="INOUTSINE",
        body=dict(y=-0.035, z=+0.01),
        head=dict(pitch=+3, yaw=0),
        torso=dict(pitch=+3, yaw=0),
        rightArm=dict(pitch=-30, yaw=-8, roll=-3, bend=24, axis=180),
        leftArm=dict(pitch=-30, yaw=+8, roll=+3, bend=24, axis=180),
        leftLeg=dict(pitch=-4, bend=6, z=-0.02),
        rightLeg=dict(pitch=+4, bend=5, z=+0.02),
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
        name="yidao_mass_meridian_repair_release",
        description=(
            "P4 群体接经共振收器收势（12t 非循环）：anticipation 0→3 法器向天"
            "催振（双臂 -148→-158），strike 3→7 对称沉落抱定胸前（-158→-64 / "
            "bend 8→52 / body.y 下砸 -0.07），recovery 7→12 放平归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
