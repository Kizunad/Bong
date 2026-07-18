#!/usr/bin/env python3
"""zhenmai_multipoint —— 真脉连环三点指（多点快戳，P1 批次一重制）。

cast_ticks=6 → endTick ∈ [10,14]，取 12。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   双手内收蓄指劲（easeOut 族 OUTSINE）
  strike       2→9   连环三戳：右高（t3）→ 左中（t5）→ 右深（t6 = cast 完成
                     = 最重一指）→ 指劲余震（t8→9，strike 段内）
  recovery     9→12  回中立 guard（INOUTSINE）
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / rightArm.bend。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    # round 3：body.x/z 首尾显式归位（防非循环残值偏移）。
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(yaw=-3),
    torso=dict(pitch=+2, yaw=+8),
    rightArm=dict(pitch=-52, yaw=-8, roll=+6, bend=68, axis=180),
    leftArm=dict(pitch=-46, yaw=+10, roll=-8, bend=60, axis=180),
    leftLeg=dict(pitch=-8, bend=10, z=-0.05),
    rightLeg=dict(pitch=+6, bend=9, z=+0.04),
)

POSE = {
    0: GUARD,
    # 蓄指劲：双手内收贴身、躯干右拧。
    2: dict(
        easing="OUTSINE",
        body=dict(y=+0.02, z=-0.03),
        head=dict(yaw=-8),
        torso=dict(pitch=+4, yaw=+16),
        rightArm=dict(pitch=-40, yaw=-12, roll=+10, bend=95, axis=180),
        leftArm=dict(pitch=-52, yaw=+12, roll=-10, bend=72, axis=180),
        leftLeg=dict(pitch=-10, bend=13, z=-0.06),
        rightLeg=dict(pitch=+8, bend=12, z=+0.05),
    ),
    # 第一指：右手高位快戳。
    3: dict(
        easing="INQUAD",
        body=dict(y=+0.01, z=+0.05),
        head=dict(yaw=0),
        torso=dict(pitch=+3, yaw=-2),
        # round 2：第一指抬到高位线（pitch -102，y -1.2 面高）与第三指分层。
        rightArm=dict(pitch=-102, yaw=-14, roll=+2, bend=12, axis=180),
        leftArm=dict(pitch=-50, yaw=+12, roll=-10, bend=75, axis=180),
        leftLeg=dict(pitch=-12, bend=13, z=-0.06),
        rightLeg=dict(pitch=+9, bend=13, z=+0.05),
    ),
    # 半收：右手回抽换劲。
    4: dict(
        easing="OUTQUAD",
        body=dict(y=+0.02, z=+0.01),
        head=dict(yaw=-4),
        torso=dict(pitch=+3, yaw=+8),
        rightArm=dict(pitch=-48, yaw=-10, roll=+8, bend=80, axis=180),
        leftArm=dict(pitch=-55, yaw=+13, roll=-11, bend=68, axis=180),
        leftLeg=dict(pitch=-11, bend=12, z=-0.06),
        rightLeg=dict(pitch=+8, bend=12, z=+0.05),
    ),
    # 第二指：左手中位补戳（右手同拍收腰）。
    5: dict(
        easing="INQUAD",
        body=dict(x=+0.02, y=+0.02, z=+0.05),
        head=dict(yaw=+2),
        torso=dict(pitch=+3, yaw=+14),
        rightArm=dict(pitch=-45, yaw=-11, roll=+9, bend=88, axis=180),
        leftArm=dict(pitch=-85, yaw=+16, roll=-2, bend=10, axis=180),
        leftLeg=dict(pitch=-11, bend=13, z=-0.06),
        rightLeg=dict(pitch=+9, bend=13, z=+0.05),
    ),
    # 第三指 = cast 完成（tick 6）：右手最深一戳，前冲压劲（最重）。
    6: dict(
        easing="INQUAD",
        body=dict(x=-0.03, y=+0.03, z=+0.14),
        head=dict(yaw=+6),
        torso=dict(pitch=+5, yaw=-18),
        # round 2：第三指压到中低位线（pitch -86），三戳高/中/低分层且最深。
        rightArm=dict(pitch=-86, yaw=-18, roll=0, bend=3, axis=180),
        leftArm=dict(pitch=-38, yaw=+12, roll=-10, bend=85, axis=180),
        leftLeg=dict(pitch=-18, bend=16, z=-0.09),
        rightLeg=dict(pitch=+13, bend=18, z=+0.06),
    ),
    # 指劲余震：指尖维持伸展、微幅回弹。
    8: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.12),
        head=dict(yaw=+5),
        torso=dict(pitch=+4, yaw=-15),
        rightArm=dict(pitch=-82, yaw=-16, roll=+2, bend=14, axis=180),
        leftArm=dict(pitch=-40, yaw=+12, roll=-10, bend=80, axis=180),
        leftLeg=dict(pitch=-16, bend=15, z=-0.08),
        rightLeg=dict(pitch=+12, bend=16, z=+0.06),
    ),
    # 余震收尾（strike 段末帧）。
    9: dict(
        easing="INOUTSINE",
        body=dict(y=+0.02, z=+0.08),
        head=dict(yaw=+2),
        torso=dict(pitch=+3, yaw=-8),
        rightArm=dict(pitch=-75, yaw=-12, roll=+4, bend=35, axis=180),
        leftArm=dict(pitch=-43, yaw=+11, roll=-9, bend=70, axis=180),
        leftLeg=dict(pitch=-13, bend=13, z=-0.07),
        rightLeg=dict(pitch=+10, bend=13, z=+0.05),
    ),
    # 收势中段。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=+0.04),
        head=dict(yaw=-1),
        torso=dict(pitch=+2, yaw=0),
        rightArm=dict(pitch=-60, yaw=-10, roll=+5, bend=55, axis=180),
        leftArm=dict(pitch=-45, yaw=+10, roll=-8, bend=62, axis=180),
        leftLeg=dict(pitch=-10, bend=11, z=-0.06),
        rightLeg=dict(pitch=+8, bend=10, z=+0.04),
    ),
    12: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="zhenmai_multipoint",
        description=(
            "P1 重制连环三点指：anticipation 0→2 内收蓄劲，strike 2→6 三连戳"
            "（右高 t3 → 左中 t5 → 右深 t6=cast 顶点，pitch -96 / bend 3 / "
            "torso.yaw +16→-18）+6→9 指劲余震，recovery 9→12 回 guard。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
