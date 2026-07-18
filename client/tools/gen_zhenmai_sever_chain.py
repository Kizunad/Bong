#!/usr/bin/env python3
"""zhenmai_sever_chain —— 真脉手刀斩脉（横切断链，P1 批次一重制）。

cast_ticks=8 → endTick ∈ [12,16]，取 14。

时序（精度标准 #1/#2/#3；round 2 语义定稿：手刀引至**右耳侧**、右肩后拧
蓄势 → 横斩**向左**过中线——与 torso 拧转方向一致，右肩先蓄后送）：
  anticipation 0→5   手刀引到右耳侧、躯干右拧蓄势（easeOut 族 OUTSINE）
  strike       5→11  横斩过身向左（tick 8 = cast 完成 = 斩断点）→ 顺势
                     overshoot（t10）→ 定劲（t11，strike 段内）
  recovery     11→14 回中立 guard（INOUTSINE）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.yaw / rightArm.pitch /
torso.yaw。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0),
    head=dict(yaw=-3),
    torso=dict(pitch=+2, yaw=+5),
    rightArm=dict(pitch=-48, yaw=-8, roll=+8, bend=55, axis=180),
    leftArm=dict(pitch=-42, yaw=+10, roll=-8, bend=50, axis=180),
    leftLeg=dict(pitch=-8, bend=10, z=-0.05),
    rightLeg=dict(pitch=+6, bend=10, z=+0.04),
)

POSE = {
    0: GUARD,
    # 引刀中段：手刀抬起引向右耳侧。
    3: dict(
        easing="OUTSINE",
        body=dict(x=+0.03, y=-0.01, z=-0.04),
        head=dict(yaw=-10),
        torso=dict(pitch=+3, yaw=+18),
        rightArm=dict(pitch=-70, yaw=+18, roll=-12, bend=75, axis=180),
        leftArm=dict(pitch=-48, yaw=+12, roll=-9, bend=58, axis=180),
        leftLeg=dict(pitch=-7, bend=11, z=-0.05),
        rightLeg=dict(pitch=+8, bend=13, z=+0.05),
    ),
    # 蓄势顶点：手刀提到右耳侧高位（x -9.9 / y -2.0）、躯干右拧满 +28。
    5: dict(
        easing="OUTSINE",
        body=dict(x=+0.05, y=-0.02, z=-0.06),
        head=dict(yaw=-14),
        torso=dict(pitch=+4, yaw=+28),
        rightArm=dict(pitch=-82, yaw=+32, roll=-18, bend=88, axis=180),
        leftArm=dict(pitch=-52, yaw=+14, roll=-10, bend=62, axis=180),
        leftLeg=dict(pitch=-6, bend=12, z=-0.05),
        rightLeg=dict(pitch=+10, bend=16, z=+0.06),
    ),
    # 横斩中段：刀锋扫过中线。
    7: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=+0.01, z=+0.06),
        head=dict(yaw=0),
        torso=dict(pitch=+4, yaw=+4),
        rightArm=dict(pitch=-78, yaw=-15, roll=-4, bend=40, axis=180),
        leftArm=dict(pitch=-45, yaw=+12, roll=-9, bend=68, axis=180),
        leftLeg=dict(pitch=-14, bend=14, z=-0.07),
        rightLeg=dict(pitch=+12, bend=18, z=+0.06),
    ),
    # 斩断点 = cast 完成（tick 8）：手刀全伸横切过中线向左（bend 4 / round 2
    # yaw -50 → x +1.2 过中）、torso.yaw -24（52° 总扭矩）、body 前冲，
    # 左臂反向收紧（counter-pull）。
    8: dict(
        easing="INQUAD",
        body=dict(x=-0.06, y=+0.02, z=+0.16),
        head=dict(yaw=+10),
        torso=dict(pitch=+5, yaw=-24),
        rightArm=dict(pitch=-74, yaw=-50, roll=+10, bend=4, axis=180),
        leftArm=dict(pitch=-35, yaw=+12, roll=-10, bend=78, axis=180),
        leftLeg=dict(pitch=-20, bend=18, z=-0.10),
        rightLeg=dict(pitch=+15, bend=22, z=+0.07),
    ),
    # 顺势 overshoot：刀势透出再荡开 6°（§2.6 弹性过冲）。
    10: dict(
        easing="OUTQUAD",
        body=dict(x=-0.07, y=+0.02, z=+0.17),
        head=dict(yaw=+11),
        torso=dict(pitch=+5, yaw=-27),
        rightArm=dict(pitch=-72, yaw=-56, roll=+12, bend=2, axis=180),
        leftArm=dict(pitch=-36, yaw=+12, roll=-10, bend=80, axis=180),
        leftLeg=dict(pitch=-21, bend=19, z=-0.10),
        rightLeg=dict(pitch=+15, bend=23, z=+0.07),
    ),
    # 定劲（strike 段末帧）：斩势收止。
    11: dict(
        easing="INOUTSINE",
        body=dict(x=-0.05, y=+0.02, z=+0.14),
        head=dict(yaw=+8),
        torso=dict(pitch=+4, yaw=-22),
        rightArm=dict(pitch=-70, yaw=-50, roll=+10, bend=12, axis=180),
        leftArm=dict(pitch=-38, yaw=+12, roll=-9, bend=74, axis=180),
        leftLeg=dict(pitch=-18, bend=17, z=-0.09),
        rightLeg=dict(pitch=+13, bend=20, z=+0.06),
    ),
    # 收刀中段。
    12: dict(
        easing="INOUTSINE",
        body=dict(x=-0.02, y=+0.01, z=+0.07),
        head=dict(yaw=+3),
        torso=dict(pitch=+3, yaw=-8),
        rightArm=dict(pitch=-58, yaw=-30, roll=+9, bend=35, axis=180),
        leftArm=dict(pitch=-40, yaw=+11, roll=-9, bend=62, axis=180),
        leftLeg=dict(pitch=-13, bend=13, z=-0.07),
        rightLeg=dict(pitch=+10, bend=15, z=+0.05),
    ),
    14: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="zhenmai_sever_chain",
        description=(
            "P1 重制手刀斩脉：anticipation 0→5 手刀引右耳侧（yaw +32 / torso.yaw "
            "+28 右肩后蓄），strike 5→8 横斩过中线向左全伸（yaw -50 / bend 4 / "
            "torso.yaw -24 右肩送出）+8→11 顺势 overshoot 定劲，"
            "recovery 11→14 回 guard。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
