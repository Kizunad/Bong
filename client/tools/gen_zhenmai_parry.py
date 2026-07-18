#!/usr/bin/env python3
"""zhenmai_parry —— 真脉单手快速拍挡（金脉点穴系，P1 批次一重制）。

cast_ticks=1（瞬发）→ 总长 ∈ [6,12]，取 endTick=8：爆发帧 + 收势，
不因 cast 短而砍收势（精度标准 #2 瞬发分支）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→1   一拍极短蓄势：掌心内收上提（OUTQUAD）
  strike       1→4   掌横拍过中线（tick 2 = 拍击点）→ 顺势推送（t4，strike 段内）
  recovery     4→8   回中立 guard（INOUTSINE）
endTick=8，stopTick=10，非循环。主打击轴：rightArm.pitch / rightArm.yaw。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    # round 3：body.x/z 首尾显式归位（防非循环残值偏移）。
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(yaw=-3),
    torso=dict(pitch=+2, yaw=+6),
    rightArm=dict(pitch=-50, yaw=-6, roll=+8, bend=70, axis=180),
    leftArm=dict(pitch=-38, yaw=+10, roll=-8, bend=55, axis=180),
    leftLeg=dict(pitch=-8, bend=10, z=-0.05),
    rightLeg=dict(pitch=+6, bend=8, z=+0.04),
)

POSE = {
    0: GUARD,
    # 瞬发蓄势（1 tick）：掌微收上提、腕内旋，重心一沉。
    1: dict(
        easing="OUTQUAD",
        body=dict(y=+0.02, z=-0.02),
        head=dict(yaw=-6),
        torso=dict(pitch=+3, yaw=+12),
        rightArm=dict(pitch=-42, yaw=-10, roll=+12, bend=85, axis=180),
        leftArm=dict(pitch=-40, yaw=+11, roll=-9, bend=58, axis=180),
        leftLeg=dict(pitch=-9, bend=12, z=-0.05),
        rightLeg=dict(pitch=+7, bend=10, z=+0.04),
    ),
    # 拍击点（tick 2）：掌刀横扫过中线、腕外翻甩劲，torso 反向拧转。
    2: dict(
        easing="INQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.04),
        head=dict(yaw=+4),
        torso=dict(pitch=+3, yaw=-10),
        # round 2：yaw 符号修正——+26 是拍向体侧外；改 yaw -26 / roll +18 拍到
        # 面前中线（x -1.8 过中）。
        rightArm=dict(pitch=-78, yaw=-26, roll=+18, bend=18, axis=180),
        leftArm=dict(pitch=-42, yaw=+11, roll=-10, bend=62, axis=180),
        leftLeg=dict(pitch=-11, bend=13, z=-0.06),
        rightLeg=dict(pitch=+8, bend=12, z=+0.05),
    ),
    # 顺势推送：荡开来势的余劲（strike 段收尾）。
    4: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.02, z=+0.03),
        head=dict(yaw=+2),
        torso=dict(pitch=+2, yaw=-7),
        rightArm=dict(pitch=-74, yaw=-32, roll=+20, bend=24, axis=180),
        leftArm=dict(pitch=-40, yaw=+10, roll=-9, bend=58, axis=180),
        leftLeg=dict(pitch=-10, bend=12, z=-0.05),
        rightLeg=dict(pitch=+7, bend=10, z=+0.04),
    ),
    # 收掌中段。
    6: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=0.0),
        head=dict(yaw=-1),
        torso=dict(pitch=+2, yaw=0),
        rightArm=dict(pitch=-58, yaw=-10, roll=+8, bend=50, axis=180),
        leftArm=dict(pitch=-39, yaw=+10, roll=-8, bend=56, axis=180),
        leftLeg=dict(pitch=-9, bend=11, z=-0.05),
        rightLeg=dict(pitch=+6, bend=9, z=+0.04),
    ),
    8: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="zhenmai_parry",
        description=(
            "P1 重制真脉拍挡（瞬发）：anticipation 0→1 掌收上提（bend 70→85），"
            "strike 1→2 掌刀横拍到面前中线（yaw -6→-26 过中 / torso.yaw +12→-10）"
            "+2→4 顺势推送（yaw -32），recovery 4→8 回 guard。爆发帧+收势。"
        ),
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
