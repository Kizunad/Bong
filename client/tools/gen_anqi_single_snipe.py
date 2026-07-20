#!/usr/bin/env python3
"""anqi_single_snipe —— 单射狙击：侧身瞄准线→骨镖弹射出手→随镖目送（P2 批次二前半）。

cast_ticks=6，去复用（原借 sword_stab）。对拍区间 endTick ∈ [10,14]，取 12。

母题：暗器狙击（≠剑刺）。侧身站架——左臂前伸做瞄准线、右手骨镖扣于耳侧，
一记鞭甩弹射出手（腕弹为主的轻脆爆发，非剑的躯干突刺），出手后手型张开、
抬头随镖目送远去，再收回侧身架。

**easing 管辖语义**（PlayerAnimator 1.0.2 实证，非直觉）：本仓动画一律不声明
`easeBeforeKeyframe`，`KeyframeAnimation.isEasingBefore` 因而取默认 false，
`KeyframeAnimationPlayer#getValueFromKeyframes` 走 `before.ease`——即**某帧上
写的 easing 管辖「本帧 → 下一帧」这一段**。故各段的 easing 要写在该段的
**起始侧帧**上：strike 段的 easeIn 落在 tick 4（管辖 4→6），写到顶点帧
tick 6 上会跑去管收势段 6→8。由
`AnimCastTicksAlignmentTest#strikePhaseCarriesEaseInDrive` 机械锁定。

时序（精度标准 #1/#2/#3；cast=6 短蓄势压缩到 4 tick）：
  anticipation 0→4   侧身瞄准：骨镖扣至耳侧最深（bend 95→112）、拧腰 -25°、
                     左臂瞄准线绷直（easeOut 族 OUTSINE 写在 t0/t2）
  strike       4→6   弹射出手：右臂鞭甩前送（pitch -80→-88 / bend 112→6）、
                     躯干甩正 +8（easeIn 族 INQUAD 写在 t4），顶点 = tick 6
  recovery     6→12  随镖目送（t8 手型漂移+抬头）→ 收回侧身架（INOUTSINE）
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / rightArm.bend /
torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 侧身投掷 guard：右肩后收、左臂前伸瞄准线、面部朝向目标。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=+0.02, y=0.0, z=-0.02),
    head=dict(pitch=-2, yaw=+14),
    torso=dict(pitch=+2, yaw=-18),
    rightArm=dict(pitch=-70, yaw=-18, roll=+12, bend=95, axis=180),
    leftArm=dict(pitch=-78, yaw=+8, roll=-6, bend=8, axis=180),
    leftLeg=dict(pitch=-14, bend=14, z=-0.08),
    rightLeg=dict(pitch=+12, bend=12, z=+0.06),
)

# 弹射顶点（tick 6 = cast 完成瞬间）：右臂全伸展前送、躯干甩正、重心压前。
# easing 管辖 6→8（见模块 docstring 的库语义），即目送段起手 → 收势族 INOUTSINE。
# 发力用的 easeIn 归 tick 4 那帧（它才管辖 strike 段 4→6）。
APEX = dict(
    easing="INOUTSINE",
    body=dict(x=-0.03, y=0.0, z=+0.14),
    head=dict(pitch=-2, yaw=+2),
    torso=dict(pitch=+6, yaw=+8),
    rightArm=dict(pitch=-88, yaw=+4, roll=-4, bend=6, axis=180),
    leftArm=dict(pitch=-30, yaw=+18, roll=-14, bend=45, axis=180),
    leftLeg=dict(pitch=-22, bend=20, z=-0.10),
    rightLeg=dict(pitch=+16, bend=20, z=+0.07),
)

POSE = {
    0: GUARD,
    # 瞄准收紧：镖向耳侧扣深、瞄准线绷直。
    2: dict(
        easing="OUTSINE",
        body=dict(x=+0.03, y=-0.02, z=-0.05),
        head=dict(pitch=-2, yaw=+17),
        torso=dict(pitch=+2, yaw=-22),
        rightArm=dict(pitch=-76, yaw=-22, roll=+13, bend=105, axis=180),
        leftArm=dict(pitch=-80, yaw=+6, roll=-6, bend=5, axis=180),
        leftLeg=dict(pitch=-12, bend=15, z=-0.07),
        rightLeg=dict(pitch=+14, bend=16, z=+0.07),
    ),
    # 蓄势顶点：镖扣最深、拧腰 -25°、后腿承重最深。
    # 本帧 easing 管辖 strike 段 4→6（鞭甩弹射），故取 easeIn 族 INQUAD——
    # 蓄势段自身的 easeOut 由 tick 0 / 2 两帧承担。
    4: dict(
        easing="INQUAD",
        body=dict(x=+0.04, y=-0.03, z=-0.06),
        head=dict(pitch=-3, yaw=+19),
        torso=dict(pitch=+3, yaw=-25),
        rightArm=dict(pitch=-80, yaw=-24, roll=+14, bend=112, axis=180),
        leftArm=dict(pitch=-82, yaw=+5, roll=-6, bend=3, axis=180),
        leftLeg=dict(pitch=-11, bend=15, z=-0.06),
        rightLeg=dict(pitch=+15, bend=18, z=+0.08),
    ),
    # 弹射顶点 = cast 完成（tick 6）。
    6: APEX,
    # 目送起点：手型张开漂移、抬头随镖。
    8: dict(
        easing="INOUTSINE",
        body=dict(x=-0.02, y=0.0, z=+0.10),
        head=dict(pitch=-5, yaw=+4),
        torso=dict(pitch=+4, yaw=+4),
        rightArm=dict(pitch=-80, yaw=+8, roll=0, bend=14, axis=180),
        leftArm=dict(pitch=-35, yaw=+16, roll=-13, bend=40, axis=180),
        leftLeg=dict(pitch=-18, bend=18, z=-0.09),
        rightLeg=dict(pitch=+14, bend=17, z=+0.06),
    ),
    # 收回中段：躯干回拧、双臂回位。
    10: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=+0.04),
        head=dict(pitch=-3, yaw=+8),
        torso=dict(pitch=+3, yaw=-8),
        rightArm=dict(pitch=-74, yaw=-6, roll=+6, bend=55, axis=180),
        leftArm=dict(pitch=-55, yaw=+12, roll=-9, bend=25, axis=180),
        leftLeg=dict(pitch=-16, bend=16, z=-0.08),
        rightLeg=dict(pitch=+13, bend=14, z=+0.06),
    ),
    # 收势回侧身 guard。
    12: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="anqi_single_snipe",
        description=(
            "P2 单射狙击专属：anticipation 0→4 侧身瞄准（rightArm bend 95→112 "
            "扣耳侧 / torso.yaw -25 / leftArm 瞄准线 bend 3），strike 4→6 弹射"
            "出手（bend 112→6 鞭甩 / torso 甩正 +8 / body.z +0.14），recovery "
            "6→12 随镖目送（t8 手型张开+抬头）收回侧身架。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
