#!/usr/bin/env python3
"""sword_path_resonance —— 共鸣：双手持剑胸前颤鸣蓄振→振荡外放（P2 批次二前半）。

cast_ticks=30，去复用（原借 sword_cleave）。对拍区间 endTick ∈ [34,38]，取 36。

母题：剑身共鸣。双手举剑竖于胸前中线，剑身颤鸣蓄振——用**往复微颤关键帧**
（双臂 roll/yaw 交替 ± 摆、振幅逐轮增大）体现"颤"，蓄满后双臂把振荡沿刃
向前外放（推剑爆发），cast 完成瞬间 = 外放顶点。

时序（精度标准 #1/#2/#3）：
  anticipation 0→14  举剑入中线 + 初级微颤（±1.5°~±3°，easeOut 族 OUTSINE）
  strike       14→30 颤鸣渐强（±5°~±8° 往复）→ t28 极限压缩 → t30 振荡外放
                     （双臂推剑前送 bend 96→22，easeIn 族 INQUAD/INSINE），
                     顶点 = tick 30（cast 完成瞬间）
  recovery     30→36 由外放位收回 guard（INOUTSINE，t33 中段帧）
endTick=36，stopTick=38，非循环。主打击轴：rightArm.roll / leftArm.roll /
rightArm.bend / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 双手持剑 guard（与 sword_cleave 同族但剑位更居中）。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-4),
    torso=dict(pitch=+4, yaw=-6),
    rightArm=dict(pitch=-68, yaw=-10, roll=+22, bend=72, axis=180),
    leftArm=dict(pitch=-64, yaw=+14, roll=-22, bend=78, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.06),
    rightLeg=dict(pitch=+8, bend=10, z=+0.04),
)

# 中线持剑基准位（微颤围绕此位摆动）：双手并拢、剑身竖于胸前中线。
CENTER = dict(
    body=dict(y=-0.02, z=-0.03),
    head=dict(pitch=-8),
    torso=dict(pitch=0, yaw=0),
    rightArm=dict(pitch=-85, yaw=+4, roll=+30, bend=85, axis=180),
    leftArm=dict(pitch=-80, yaw=-4, roll=-30, bend=90, axis=180),
    leftLeg=dict(pitch=-8, bend=14, z=-0.05),
    rightLeg=dict(pitch=+8, bend=14, z=+0.05),
)

# 振荡外放顶点（tick 30 = cast 完成瞬间）：双臂推剑前送、躯干前压、马步扎实。
APEX = dict(
    easing="INQUAD",
    body=dict(y=+0.02, z=+0.16),
    head=dict(pitch=-4),
    torso=dict(pitch=+10, yaw=0),
    rightArm=dict(pitch=-92, yaw=-2, roll=+12, bend=22, axis=180),
    leftArm=dict(pitch=-88, yaw=+2, roll=-12, bend=28, axis=180),
    leftLeg=dict(pitch=-20, bend=24, z=-0.10),
    rightLeg=dict(pitch=+16, bend=22, z=+0.07),
)

POSE = {
    0: GUARD,
    # 举剑入中线中段。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=-0.02),
        head=dict(pitch=-6),
        torso=dict(pitch=+2, yaw=-2),
        rightArm=dict(pitch=-78, yaw=-2, roll=+26, bend=78, axis=180),
        leftArm=dict(pitch=-74, yaw=+6, roll=-26, bend=84, axis=180),
        leftLeg=dict(pitch=-9, bend=13, z=-0.05),
        rightLeg=dict(pitch=+8, bend=12, z=+0.04),
    ),
    # 中线基准位：剑竖胸前，颤鸣即将开始。
    8: dict(easing="OUTSINE", **CENTER),
    # 微颤 +1（±1.5°）。
    11: dict(
        easing="OUTSINE",
        body=dict(y=-0.025, z=-0.035),
        head=dict(pitch=-7),
        torso=dict(pitch=0, yaw=+1.5),
        rightArm=dict(pitch=-84, yaw=+6, roll=+33, bend=86, axis=180),
        leftArm=dict(pitch=-81, yaw=-6, roll=-27, bend=89, axis=180),
        leftLeg=dict(pitch=-8, bend=14, z=-0.05),
        rightLeg=dict(pitch=+8, bend=14, z=+0.05),
    ),
    # 微颤 -1。
    14: dict(
        easing="OUTSINE",
        body=dict(y=-0.028, z=-0.04),
        head=dict(pitch=-9),
        torso=dict(pitch=0, yaw=-1.5),
        rightArm=dict(pitch=-86, yaw=+2, roll=+27, bend=87, axis=180),
        leftArm=dict(pitch=-79, yaw=-2, roll=-33, bend=91, axis=180),
        leftLeg=dict(pitch=-8, bend=15, z=-0.05),
        rightLeg=dict(pitch=+8, bend=15, z=+0.05),
    ),
    # 颤鸣渐强 +2（±2.5°，进入 strike 段）。
    17: dict(
        easing="INSINE",
        body=dict(y=-0.03, z=-0.045),
        head=dict(pitch=-8),
        torso=dict(pitch=+1, yaw=+2.5),
        rightArm=dict(pitch=-83, yaw=+8, roll=+36, bend=88, axis=180),
        leftArm=dict(pitch=-82, yaw=-8, roll=-24, bend=92, axis=180),
        leftLeg=dict(pitch=-9, bend=16, z=-0.06),
        rightLeg=dict(pitch=+9, bend=15, z=+0.05),
    ),
    # 颤鸣渐强 -2。
    20: dict(
        easing="INSINE",
        body=dict(y=-0.035, z=-0.05),
        head=dict(pitch=-10),
        torso=dict(pitch=+1, yaw=-2.5),
        rightArm=dict(pitch=-87, yaw=0, roll=+24, bend=90, axis=180),
        leftArm=dict(pitch=-78, yaw=0, roll=-36, bend=94, axis=180),
        leftLeg=dict(pitch=-9, bend=17, z=-0.06),
        rightLeg=dict(pitch=+9, bend=16, z=+0.05),
    ),
    # 颤鸣极大 +3（±3.5°，双手渐收紧）。
    23: dict(
        easing="INSINE",
        body=dict(y=-0.04, z=-0.055),
        head=dict(pitch=-9),
        torso=dict(pitch=+2, yaw=+3.5),
        rightArm=dict(pitch=-82, yaw=+9, roll=+38, bend=90, axis=180),
        leftArm=dict(pitch=-83, yaw=-9, roll=-22, bend=95, axis=180),
        leftLeg=dict(pitch=-10, bend=18, z=-0.06),
        rightLeg=dict(pitch=+10, bend=17, z=+0.06),
    ),
    # 颤鸣极大 -3。
    26: dict(
        easing="INSINE",
        body=dict(y=-0.045, z=-0.06),
        head=dict(pitch=-10),
        torso=dict(pitch=+2, yaw=-3.5),
        rightArm=dict(pitch=-88, yaw=-1, roll=+22, bend=92, axis=180),
        leftArm=dict(pitch=-77, yaw=+1, roll=-38, bend=97, axis=180),
        leftLeg=dict(pitch=-10, bend=19, z=-0.07),
        rightLeg=dict(pitch=+10, bend=18, z=+0.06),
    ),
    # 极限压缩：双臂收紧到最深、腿弓沉，蓄势待放。
    28: dict(
        easing="INQUAD",
        body=dict(y=-0.05, z=-0.07),
        head=dict(pitch=-11),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=-90, yaw=+6, roll=+32, bend=96, axis=180),
        leftArm=dict(pitch=-86, yaw=-6, roll=-32, bend=100, axis=180),
        leftLeg=dict(pitch=-12, bend=20, z=-0.07),
        rightLeg=dict(pitch=+11, bend=18, z=+0.06),
    ),
    # 振荡外放顶点 = cast 完成（tick 30）。
    30: APEX,
    # 收势中段：推剑位回落一半。
    33: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=+0.07),
        head=dict(pitch=-5),
        torso=dict(pitch=+7, yaw=-3),
        rightArm=dict(pitch=-78, yaw=-6, roll=+18, bend=50, axis=180),
        leftArm=dict(pitch=-74, yaw=+8, roll=-18, bend=56, axis=180),
        leftLeg=dict(pitch=-14, bend=17, z=-0.08),
        rightLeg=dict(pitch=+12, bend=16, z=+0.05),
    ),
    # 收势回 guard。
    36: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_path_resonance",
        description=(
            "P2 共鸣专属：anticipation 0→14 举剑入中线+初级微颤（roll ±3° 往复），"
            "strike 14→30 颤鸣渐强（±5°~±8° 交替帧）→ t28 极限压缩（bend 96/100）"
            "→ t30 振荡外放（双臂推剑 bend→22 / torso +10 / body.z +0.16），"
            "recovery 30→36 经 t33 中段帧回 guard。"
        ),
        end_tick=36,
        stop_tick=38,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
