#!/usr/bin/env python3
"""sword_heaven_gate_release —— 天门开阖·释放段：门开巨斩（P2 批次二后半精修）。

heaven_gate release 段：charge 段（`sword_heaven_gate_charge` 60t）蓄满后的
释放巨斩。本版是密度精修（review 返工补 P2 欠账）：旧资产 20t 仅 3 关键帧
（0/8/20，最大帧距 12t），重制为三段式 8 帧（≤3t 主轴帧距）。

母题：门开斩落。t0 承接 charge 末帧（高位拉满、背弓头仰），极限再上引一分
（anticipation dip），随即双手巨斩劈落（躯干前压 +26、弓步前送 body.z +0.18，
顶点 t7），斩透定格后收剑直身。躯干大前压按「鞠躬补偿」：torso+legs 同向
pitch + body.z 前移（防腰断）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   承接拉满 → 极限上引（背弓再深一分、微沉腰）
  strike       2→7   巨斩劈落：t4 下劈半程 → t7 斩透顶点（双臂 pitch +24/+28 /
                     torso.pitch +26 / body.z +0.18，INQUAD）
  recovery     7→20  t10 斩透定格 → t13 收剑半程 → t16 直身 → t20 归中立
                     （INOUTSINE）
endTick=20，stopTick=24，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.pitch / body.z（全程 ≤3t 帧距）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接 charge 末帧：高位拉满、背弓头仰（与 charge t60 姿态吻合）。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.04, z=-0.04),
        head=dict(pitch=-18, yaw=0),
        torso=dict(pitch=-9, yaw=0),
        rightArm=dict(pitch=-154, yaw=-4, roll=+6, bend=8, axis=180),
        leftArm=dict(pitch=-150, yaw=+4, roll=-6, bend=12, axis=180),
        leftLeg=dict(pitch=-11, bend=15, z=-0.05),
        rightLeg=dict(pitch=+9, bend=13, z=+0.04),
    ),
    # 极限上引：背弓再深一分、微沉腰（劈落前的反向拉满）。
    2: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.06, z=-0.06),
        head=dict(pitch=-21, yaw=0),
        torso=dict(pitch=-12, yaw=0),
        rightArm=dict(pitch=-160, yaw=-5, roll=+7, bend=6, axis=180),
        leftArm=dict(pitch=-156, yaw=+5, roll=-7, bend=10, axis=180),
        leftLeg=dict(pitch=-13, bend=18, z=-0.06),
        rightLeg=dict(pitch=+11, bend=15, z=+0.05),
    ),
    # 下劈半程：剑过头顶向前。
    4: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.03, z=+0.06),
        head=dict(pitch=-4, yaw=0),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-70, yaw=-4, roll=+2, bend=10, axis=180),
        leftArm=dict(pitch=-66, yaw=+4, roll=-2, bend=14, axis=180),
        leftLeg=dict(pitch=-16, bend=18, z=-0.07),
        rightLeg=dict(pitch=+13, bend=15, z=+0.05),
    ),
    # 斩透顶点：双手剑劈到底、躯干前压弓步前送（鞠躬补偿：torso+legs 同向 + body.z）。
    7: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.05, z=+0.18),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+26, yaw=0),
        rightArm=dict(pitch=+24, yaw=-6, roll=+2, bend=6, axis=180),
        leftArm=dict(pitch=+28, yaw=+6, roll=-2, bend=10, axis=180),
        leftLeg=dict(pitch=-24, bend=26, z=-0.11),
        rightLeg=dict(pitch=+20, bend=22, z=+0.08),
    ),
    # 斩透定格：斩势微沉坐实。
    10: dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.05, z=+0.16),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+24, yaw=0),
        rightArm=dict(pitch=+20, yaw=-6, roll=+2, bend=8, axis=180),
        leftArm=dict(pitch=+24, yaw=+6, roll=-2, bend=12, axis=180),
        leftLeg=dict(pitch=-22, bend=24, z=-0.10),
        rightLeg=dict(pitch=+18, bend=20, z=+0.07),
    ),
    # 收剑半程。
    13: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.03, z=+0.09),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+13, yaw=0),
        rightArm=dict(pitch=-12, yaw=-5, roll=+1, bend=14, axis=180),
        leftArm=dict(pitch=-8, yaw=+5, roll=-1, bend=16, axis=180),
        leftLeg=dict(pitch=-13, bend=14, z=-0.06),
        rightLeg=dict(pitch=+10, bend=12, z=+0.04),
    ),
    # 直身。
    16: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=+0.03),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+5, yaw=0),
        rightArm=dict(pitch=-6, yaw=-3, roll=+1, bend=8, axis=180),
        leftArm=dict(pitch=-4, yaw=+3, roll=-1, bend=8, axis=180),
        leftLeg=dict(pitch=-6, bend=7, z=-0.03),
        rightLeg=dict(pitch=+5, bend=5, z=+0.02),
    ),
    # 归中立。
    20: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
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
        name="sword_heaven_gate_release",
        description=(
            "P2 天门释放段精修（20t 非循环，旧 3 关键帧重制为三段式 8 帧）："
            "anticipation 0→2 承接 charge 拉满→极限上引，strike 2→7 巨斩劈落"
            "（双臂 -160→+24/+28 / torso.pitch +26 / body.z +0.18，鞠躬补偿），"
            "recovery 7→20 斩透定格→收剑直身归中立。"
        ),
        end_tick=20,
        stop_tick=24,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
