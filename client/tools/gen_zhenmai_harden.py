#!/usr/bin/env python3
"""zhenmai_harden —— 真脉抱臂沉桩硬化（P1 批次一重制）。

cast_ticks=5 → endTick ∈ [9,13]，取 11。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   双臂外张开劲、吸气微仰（easeOut 族 OUTSINE）
  strike       3→8   双臂猛收交叉抱胸 + 沉桩（tick 5 = cast 完成 = 硬化定桩）
                     + 肌肉紧咬 clench（t7→8，strike 段内 load-snap）
  recovery     8→11  松劲起身回 guard（INOUTSINE）
endTick=11，stopTick=13，非循环。主打击轴：rightArm.bend / leftArm.bend /
body.y。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0),
    head=dict(pitch=-2),
    torso=dict(pitch=+2, yaw=0),
    rightArm=dict(pitch=-40, yaw=-10, roll=+8, bend=50, axis=180),
    leftArm=dict(pitch=-40, yaw=+10, roll=-8, bend=50, axis=180),
    leftLeg=dict(pitch=-8, bend=10, z=-0.05),
    rightLeg=dict(pitch=+6, bend=10, z=+0.04),
)

POSE = {
    0: GUARD,
    # 开劲：双臂外张、掌心向外，吸气微仰。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.03),
        head=dict(pitch=-4),
        torso=dict(pitch=-4, yaw=0),
        # round 2：yaw 符号修正——前举 pitch 下外张 = rArm yaw+ / lArm yaw-
        # （旧值反号把"开劲"收到了中线）。
        rightArm=dict(pitch=-55, yaw=+28, roll=+14, bend=25, axis=180),
        leftArm=dict(pitch=-55, yaw=-28, roll=-14, bend=25, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.05),
        rightLeg=dict(pitch=+4, bend=8, z=+0.04),
    ),
    # 开劲顶点：双臂张到最大。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.04),
        head=dict(pitch=-5),
        torso=dict(pitch=-5, yaw=0),
        rightArm=dict(pitch=-58, yaw=+34, roll=+16, bend=20, axis=180),
        leftArm=dict(pitch=-58, yaw=-34, roll=-16, bend=20, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.05),
        rightLeg=dict(pitch=+4, bend=8, z=+0.04),
    ),
    # 硬化定桩 = cast 完成（tick 5）：双臂猛收交叉抱胸（右上左下）+ 深沉桩
    # （body.y +0.09 / 双腿 bend 30）。
    5: dict(
        easing="INQUAD",
        body=dict(y=+0.09, z=+0.02),
        head=dict(pitch=+6),
        torso=dict(pitch=+8, yaw=0),
        # round 2：交叉内扣 = rArm yaw- / lArm yaw+（旧值反号变成外张），
        # 双手收到中线下颌高（x ≈ ∓2.4）交叠抱胸。
        rightArm=dict(pitch=-62, yaw=-30, roll=+20, bend=112, axis=180),
        leftArm=dict(pitch=-58, yaw=+30, roll=-20, bend=116, axis=180),
        leftLeg=dict(pitch=-14, bend=30, z=-0.07),
        rightLeg=dict(pitch=+10, bend=30, z=+0.05),
    ),
    # 紧咬 clench：真元灌注、抱得更紧再深一分（load-snap 深化）。
    7: dict(
        easing="INOUTSINE",
        body=dict(y=+0.10, z=+0.02),
        head=dict(pitch=+7),
        torso=dict(pitch=+9, yaw=0),
        rightArm=dict(pitch=-64, yaw=-32, roll=+21, bend=116, axis=180),
        leftArm=dict(pitch=-60, yaw=+32, roll=-21, bend=120, axis=180),
        leftLeg=dict(pitch=-15, bend=32, z=-0.07),
        rightLeg=dict(pitch=+11, bend=32, z=+0.05),
    ),
    # clench 微松（strike 段末帧）。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=+0.09, z=+0.02),
        head=dict(pitch=+6),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-63, yaw=-30, roll=+20, bend=114, axis=180),
        leftArm=dict(pitch=-59, yaw=+30, roll=-20, bend=118, axis=180),
        leftLeg=dict(pitch=-14, bend=31, z=-0.07),
        rightLeg=dict(pitch=+10, bend=31, z=+0.05),
    ),
    # 松劲起身。
    9: dict(
        easing="INOUTSINE",
        body=dict(y=+0.05),
        head=dict(pitch=+2),
        torso=dict(pitch=+5, yaw=0),
        rightArm=dict(pitch=-52, yaw=-8, roll=+6, bend=90, axis=180),
        leftArm=dict(pitch=-50, yaw=+8, roll=-6, bend=92, axis=180),
        leftLeg=dict(pitch=-11, bend=20, z=-0.06),
        rightLeg=dict(pitch=+8, bend=20, z=+0.04),
    ),
    11: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="zhenmai_harden",
        description=(
            "P1 重制抱臂硬化：anticipation 0→3 双臂外张开劲（yaw ±34 / bend 20），"
            "strike 3→5 猛收交叉抱胸（bend 112/116 / yaw 内扣 ∓30）+沉桩"
            "（body.y +0.09 / 双腿 bend 30）+5→8 紧咬 clench 深化，"
            "recovery 8→11 松劲回 guard。"
        ),
        end_tick=11,
        stop_tick=13,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
