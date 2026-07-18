#!/usr/bin/env python3
"""zhenmai_neutralize —— 真脉双掌下按化劲（P1 批次一重制）。

cast_ticks=4 → endTick ∈ [8,12]，取 10。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   双掌提到胸前、吸气微仰（easeOut 族 OUTSINE）
  strike       2→7   双掌下按到底（tick 4 = cast 完成 = 按劲顶点）+ 化劲
                     微旋 hold（4→7，strike 段内）
  recovery     7→10  起身回 guard（INOUTSINE）
沉桩姿态走 torso+legs 同向 pitch + body.z 补偿（精度标准 #4，torso/legs 不共祖）。
endTick=10，stopTick=12，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
body.y。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0),
    head=dict(pitch=-2),
    torso=dict(pitch=+3, yaw=0),
    rightArm=dict(pitch=-45, yaw=-8, roll=+6, bend=60, axis=180),
    leftArm=dict(pitch=-45, yaw=+8, roll=-6, bend=60, axis=180),
    leftLeg=dict(pitch=-8, bend=10, z=-0.05),
    rightLeg=dict(pitch=+6, bend=10, z=+0.04),
)

POSE = {
    0: GUARD,
    # 提掌吸气：双掌上提到胸口、躯干微仰。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.03),
        head=dict(pitch=-3),
        torso=dict(pitch=-3, yaw=0),
        rightArm=dict(pitch=-72, yaw=-10, roll=+8, bend=75, axis=180),
        leftArm=dict(pitch=-72, yaw=+10, roll=-8, bend=75, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.05),
        rightLeg=dict(pitch=+5, bend=8, z=+0.04),
    ),
    # 按劲顶点 = cast 完成（tick 4）：双掌压到腰腹高度、身体下沉前俯
    # （torso.pitch +14 与双腿同向 + body.z +0.03 补偿）。
    4: dict(
        easing="INQUAD",
        body=dict(y=+0.08, z=+0.03),
        head=dict(pitch=+10),
        torso=dict(pitch=+14, yaw=0),
        # round 2：双掌略向中线并拢（yaw ∓15），下按更聚劲。
        rightArm=dict(pitch=-18, yaw=-15, roll=+4, bend=30, axis=180),
        leftArm=dict(pitch=-18, yaw=+15, roll=-4, bend=30, axis=180),
        leftLeg=dict(pitch=-12, bend=26, z=-0.06),
        rightLeg=dict(pitch=-8, bend=26, z=+0.04),
    ),
    # 化劲微旋：掌根向外分劲（hold 段内的活性，避免死定格）。
    6: dict(
        easing="INOUTSINE",
        body=dict(y=+0.07, z=+0.03),
        head=dict(pitch=+7),
        torso=dict(pitch=+13, yaw=0),
        rightArm=dict(pitch=-16, yaw=-18, roll=+2, bend=26, axis=180),
        leftArm=dict(pitch=-16, yaw=+18, roll=-2, bend=26, axis=180),
        leftLeg=dict(pitch=-11, bend=24, z=-0.06),
        rightLeg=dict(pitch=-7, bend=24, z=+0.04),
    ),
    # hold 收尾：劲力化尽。
    7: dict(
        easing="INOUTSINE",
        body=dict(y=+0.06, z=+0.02),
        head=dict(pitch=+6),
        torso=dict(pitch=+12, yaw=0),
        rightArm=dict(pitch=-17, yaw=-14, roll=+3, bend=28, axis=180),
        leftArm=dict(pitch=-17, yaw=+14, roll=-3, bend=28, axis=180),
        leftLeg=dict(pitch=-10, bend=22, z=-0.06),
        rightLeg=dict(pitch=-6, bend=22, z=+0.04),
    ),
    # 起身中段。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=+0.03, z=+0.01),
        head=dict(pitch=+2),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-30, yaw=-10, roll=+5, bend=45, axis=180),
        leftArm=dict(pitch=-30, yaw=+10, roll=-5, bend=45, axis=180),
        leftLeg=dict(pitch=-9, bend=16, z=-0.05),
        rightLeg=dict(pitch=0, bend=16, z=+0.04),
    ),
    10: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="zhenmai_neutralize",
        description=(
            "P1 重制化劲下按：anticipation 0→2 双掌提胸吸气（pitch -45→-72），"
            "strike 2→4 双掌下按到底（pitch -18 / body.y +0.08 / torso.pitch +14 "
            "沉桩）+4→7 化劲微旋 hold，recovery 7→10 起身回 guard。"
        ),
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
