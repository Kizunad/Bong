#!/usr/bin/env python3
"""sword_parry —— 剑斜举格挡弹开、短促脆（P1 批次一重制）。

cast_ticks=4 → endTick ∈ [8,12]，取 10（sword.parry 不在 allowlist，重制后仍达标）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   重心下沉、剑开始横起（easeOut 族 OUTQUAD，短促）
  strike       2→6   斜举架格到位（tick 4 = cast 完成 = 格挡定架）+ 弹开外推
                     （4→6 deflect snap，strike 段内）
  recovery     6→10  回中立 guard（INOUTSINE）
endTick=10，stopTick=12，非循环。主打击轴：rightArm.pitch / rightArm.yaw /
torso.yaw。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-3),
    torso=dict(pitch=+3, yaw=+8),
    rightArm=dict(pitch=-58, yaw=-8, roll=+10, bend=35, axis=180),
    leftArm=dict(pitch=-42, yaw=+14, roll=-8, bend=28, axis=180),
    leftLeg=dict(pitch=-9, bend=12, z=-0.06),
    rightLeg=dict(pitch=+7, bend=10, z=+0.04),
)

POSE = {
    0: GUARD,
    # 反应帧：重心下沉、剑刃开始横向抬起。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=+0.03, z=-0.04),
        head=dict(pitch=-2),
        torso=dict(pitch=-2, yaw=-2),
        rightArm=dict(pitch=-85, yaw=-8, roll=+12, bend=42, axis=180),
        leftArm=dict(pitch=-48, yaw=+16, roll=-10, bend=45, axis=180),
        leftLeg=dict(pitch=-12, bend=16, z=-0.07),
        rightLeg=dict(pitch=+9, bend=16, z=+0.05),
    ),
    # 格挡定架 = cast 完成（tick 4）：剑斜举横过身前上方，左臂内扣支撑。
    4: dict(
        easing="INQUAD",
        body=dict(y=+0.05, z=-0.08),
        head=dict(pitch=-4),
        torso=dict(pitch=-4, yaw=-10),
        # round 2：yaw/roll 符号修正——前举 pitch 下 yaw+ = 甩向体侧外；改
        # yaw -14 / roll +20 把定架收到面前中线（x -2.2, y -4.5 脸高）。
        rightArm=dict(pitch=-112, yaw=-14, roll=+20, bend=48, axis=180),
        leftArm=dict(pitch=-55, yaw=+18, roll=-12, bend=58, axis=180),
        leftLeg=dict(pitch=-14, bend=20, z=-0.08),
        rightLeg=dict(pitch=+10, bend=20, z=+0.06),
    ),
    # 弹开（deflect snap）：刃面外推把来势荡开——strike 段内的第二拍。
    6: dict(
        easing="OUTQUAD",
        body=dict(y=+0.03, z=-0.05),
        head=dict(pitch=-2),
        torso=dict(pitch=-2, yaw=-14),
        # round 2：弹开 = 从中线向外侧荡出（x -2.2 → -9.3），yaw +6 / roll -2。
        rightArm=dict(pitch=-118, yaw=+6, roll=-14, bend=38, axis=180),
        leftArm=dict(pitch=-50, yaw=+16, roll=-10, bend=48, axis=180),
        leftLeg=dict(pitch=-12, bend=17, z=-0.07),
        rightLeg=dict(pitch=+9, bend=17, z=+0.05),
    ),
    # 卸力回落。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=-0.02),
        head=dict(pitch=-2),
        torso=dict(pitch=+1, yaw=0),
        rightArm=dict(pitch=-80, yaw=-2, roll=+6, bend=36, axis=180),
        leftArm=dict(pitch=-45, yaw=+15, roll=-9, bend=35, axis=180),
        leftLeg=dict(pitch=-10, bend=14, z=-0.06),
        rightLeg=dict(pitch=+8, bend=13, z=+0.05),
    ),
    10: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_parry",
        description=(
            "P1 重制格挡：anticipation 0→2 沉重心横剑（body.y +0.03），strike 2→4 "
            "斜举定架于面前中线（pitch -112 / yaw -14 / roll +20）+4→6 弹开外荡"
            "（yaw +6 外推至体侧 / torso.yaw -14），recovery 6→10 回 guard。短促脆。"
        ),
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
