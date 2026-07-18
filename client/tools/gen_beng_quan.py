#!/usr/bin/env python3
"""beng_quan —— 崩拳：沉马蓄劲 → 拳从腰际炸出 → 震颤收（P1 批次一重制）。

cast_ticks 三借用方约束（burst_meridian 三招共用本动画，且三条均出 allowlist）：
beng_quan cast=8 → [12,16]；tie_shan_kao cast=10 → [14,18]；xue_beng_bu
cast=6 → [10,14]。交集 = {14} —— endTick 固定 14。

时序（精度标准 #1/#2/#3）：
  anticipation 0→5   沉马蓄劲：重心下坐 + 拳收腰际 + 拧腰（easeOut 族 OUTSINE）
  strike       5→11  拳炸出（tick 8 = cast 完成 = 发力顶点）→ 震颤 overshoot
                     （t10）→ 回颤（t11），全部 strike 段内
  recovery     11→14 回中立 guard（INOUTSINE）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.pitch / rightArm.bend /
torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(yaw=-4),
    torso=dict(pitch=+3, yaw=+8),
    rightArm=dict(pitch=-30, yaw=-10, roll=+12, bend=95, axis=180),
    leftArm=dict(pitch=-45, yaw=+12, roll=-15, bend=75, axis=180),
    leftLeg=dict(pitch=-10, bend=14, z=-0.08),
    rightLeg=dict(pitch=+8, bend=12, z=+0.05),
)

POSE = {
    0: GUARD,
    # 沉马中段：重心下坐、右拳往腰际收。
    3: dict(
        easing="OUTSINE",
        body=dict(y=+0.06, z=-0.05),
        head=dict(yaw=-10),
        torso=dict(pitch=+6, yaw=+20),
        rightArm=dict(pitch=-14, yaw=-14, roll=+16, bend=120, axis=180),
        leftArm=dict(pitch=-52, yaw=+14, roll=-16, bend=82, axis=180),
        leftLeg=dict(pitch=-14, bend=24, z=-0.09),
        rightLeg=dict(pitch=+12, bend=26, z=+0.06),
    ),
    # 蓄劲顶点：马步最深（body.y +0.09 / 双腿 bend 30+）、拳贴腰（bend 132）、
    # 拧腰 +26。
    5: dict(
        easing="OUTSINE",
        body=dict(y=+0.09, z=-0.08),
        head=dict(yaw=-14),
        torso=dict(pitch=+8, yaw=+26),
        rightArm=dict(pitch=-10, yaw=-16, roll=+18, bend=132, axis=180),
        leftArm=dict(pitch=-56, yaw=+15, roll=-17, bend=86, axis=180),
        leftLeg=dict(pitch=-16, bend=30, z=-0.10),
        rightLeg=dict(pitch=+14, bend=32, z=+0.07),
    ),
    # 出拳中段：从腰际启动加速。
    7: dict(
        easing="INQUAD",
        body=dict(y=+0.05, z=+0.06),
        head=dict(yaw=0),
        torso=dict(pitch=+5, yaw=+4),
        rightArm=dict(pitch=-55, yaw=-14, roll=+10, bend=60, axis=180),
        leftArm=dict(pitch=-40, yaw=+13, roll=-14, bend=95, axis=180),
        leftLeg=dict(pitch=-20, bend=26, z=-0.11),
        rightLeg=dict(pitch=+16, bend=28, z=+0.07),
    ),
    # 发力顶点 = cast 完成（tick 8）：拳全伸炸出（bend 4）、torso.yaw -22
    # （48° 总扭矩）、body.z +0.20 前冲，左拳收紧护肋（load-snap 反相）。
    8: dict(
        easing="INQUAD",
        body=dict(x=-0.04, y=+0.03, z=+0.20),
        head=dict(yaw=+8),
        torso=dict(pitch=+6, yaw=-22),
        rightArm=dict(pitch=-86, yaw=-18, roll=+4, bend=4, axis=180),
        leftArm=dict(pitch=-58, yaw=+14, roll=-18, bend=105, axis=180),
        leftLeg=dict(pitch=-24, bend=22, z=-0.12),
        rightLeg=dict(pitch=+18, bend=26, z=+0.08),
    ),
    # 震颤 overshoot：拳再压深 4°、劲透到底（§2.6 弹性过冲）。
    10: dict(
        easing="OUTQUAD",
        body=dict(x=-0.05, y=+0.04, z=+0.22),
        head=dict(yaw=+9),
        torso=dict(pitch=+7, yaw=-25),
        rightArm=dict(pitch=-90, yaw=-19, roll=+3, bend=2, axis=180),
        leftArm=dict(pitch=-60, yaw=+14, roll=-18, bend=108, axis=180),
        leftLeg=dict(pitch=-25, bend=23, z=-0.12),
        rightLeg=dict(pitch=+18, bend=27, z=+0.08),
    ),
    # 回颤：弹回少许（震颤第二拍，strike 段收尾）。
    11: dict(
        easing="INOUTSINE",
        body=dict(x=-0.04, y=+0.04, z=+0.19),
        head=dict(yaw=+7),
        torso=dict(pitch=+6, yaw=-21),
        rightArm=dict(pitch=-84, yaw=-17, roll=+5, bend=10, axis=180),
        leftArm=dict(pitch=-57, yaw=+14, roll=-17, bend=102, axis=180),
        leftLeg=dict(pitch=-23, bend=22, z=-0.11),
        rightLeg=dict(pitch=+17, bend=25, z=+0.08),
    ),
    # 收拳中段。
    12: dict(
        easing="INOUTSINE",
        body=dict(y=+0.02, z=+0.10),
        head=dict(yaw=+2),
        torso=dict(pitch=+4, yaw=-8),
        rightArm=dict(pitch=-60, yaw=-13, roll=+9, bend=55, axis=180),
        leftArm=dict(pitch=-50, yaw=+13, roll=-16, bend=88, axis=180),
        leftLeg=dict(pitch=-16, bend=18, z=-0.09),
        rightLeg=dict(pitch=+12, bend=18, z=+0.06),
    ),
    14: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="beng_quan",
        description=(
            "P1 重制崩拳：anticipation 0→5 沉马蓄劲（body.y +0.09 / 双腿 bend 30+ / "
            "拳贴腰 bend 132 / torso.yaw +26），strike 5→8 拳炸出（bend 132→4 / "
            "torso.yaw -22 / body.z +0.20）+8→11 震颤 overshoot 回颤，"
            "recovery 11→14 回 guard。endTick=14 为三借用方 cast 区间交集。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
