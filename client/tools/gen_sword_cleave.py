#!/usr/bin/env python3
"""sword_cleave —— 双手举剑过头竖劈 + 弓步前压（P1 批次一重制）。

cast_ticks=16。借用方约束：sword_path.condense_edge（cast=12，不在 allowlist）
要求 endTick ∈ [16,20]，sword.cleave 要求 [20,24]，交集 = 20 —— endTick 固定 20。

时序（精度标准 #1/#2/#3）：
  anticipation 0→10  双手举剑过头 + 拧腰后坐（easeOut 族 OUTSINE）
  strike       10→18 过身下劈 + torso 前压 + body.z 前送 + 前腿弓步（easeIn 族
                     INQUAD），发力顶点 = tick 16（cast 完成瞬间），hold 16→18 定格
  recovery     18→20 回中立 guard（INOUTSINE）
endTick=20，stopTick=22，非循环。主打击轴：rightArm.pitch / torso.pitch / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 双手持剑 guard：右手主握、左手辅握向中线并拢，浅前后站架。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-4),
    torso=dict(pitch=+4, yaw=-6),
    rightArm=dict(pitch=-62, yaw=-10, roll=+8, bend=30, axis=180),
    leftArm=dict(pitch=-55, yaw=+16, roll=-10, bend=38, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.06),
    rightLeg=dict(pitch=+8, bend=10, z=+0.04),
)

# 打击定格（t16 impact 的微沉降版本，避免"到位即冻结"）。
IMPACT = dict(
    easing="INQUAD",
    body=dict(y=+0.06, z=+0.30),
    head=dict(pitch=+14),
    torso=dict(pitch=+18, yaw=+14),
    rightArm=dict(pitch=+52, yaw=-6, roll=+2, bend=8, axis=180),
    leftArm=dict(pitch=+42, yaw=+12, roll=-4, bend=14, axis=180),
    leftLeg=dict(pitch=-34, bend=30, z=-0.14),
    rightLeg=dict(pitch=+26, bend=34, z=+0.10),
)

POSE = {
    0: GUARD,
    # 举剑中段：双臂过肩、躯干开始后拧。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=-4),
        torso=dict(pitch=-2, yaw=-13),
        rightArm=dict(pitch=-105, yaw=-8, roll=+6, bend=42, axis=180),
        leftArm=dict(pitch=-95, yaw=+14, roll=-8, bend=48, axis=180),
        leftLeg=dict(pitch=-8, bend=14, z=-0.06),
        rightLeg=dict(pitch=+11, bend=16, z=+0.05),
    ),
    # 接近顶点：剑近头顶，重心后坐蓄势。
    8: dict(
        easing="OUTSINE",
        body=dict(y=-0.04, z=-0.06),
        head=dict(pitch=-8),
        torso=dict(pitch=-7, yaw=-18),
        rightArm=dict(pitch=-142, yaw=-6, roll=+4, bend=52, axis=180),
        leftArm=dict(pitch=-132, yaw=+12, roll=-6, bend=58, axis=180),
        leftLeg=dict(pitch=-6, bend=16, z=-0.05),
        rightLeg=dict(pitch=+13, bend=20, z=+0.06),
    ),
    # 蓄势顶点：举剑过头到极限、拧腰到 -20°，仰视剑锋。
    10: dict(
        easing="OUTSINE",
        body=dict(y=-0.05, z=-0.08),
        head=dict(pitch=-10),
        torso=dict(pitch=-8, yaw=-20),
        rightArm=dict(pitch=-152, yaw=-5, roll=+3, bend=55, axis=180),
        leftArm=dict(pitch=-140, yaw=+11, roll=-5, bend=60, axis=180),
        leftLeg=dict(pitch=-5, bend=15, z=-0.05),
        rightLeg=dict(pitch=+14, bend=22, z=+0.07),
    ),
    # 下劈中段：加速过身。
    13: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=+0.10),
        head=dict(pitch=+2),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-55, yaw=-8, roll=+4, bend=25, axis=180),
        leftArm=dict(pitch=-45, yaw=+12, roll=-6, bend=30, axis=180),
        leftLeg=dict(pitch=-18, bend=18, z=-0.10),
        rightLeg=dict(pitch=+18, bend=24, z=+0.06),
    ),
    # 发力顶点 = cast 完成（tick 16）：剑劈到身前下方，弓步前压。
    16: IMPACT,
    # 打击定格（strike 段内 hold，2 tick）：刃口微沉。
    18: inherit(
        IMPACT,
        easing="INOUTSINE",
        body=dict(y=+0.06, z=+0.29),
        head=dict(pitch=+13),
        rightArm=dict(pitch=+55, bend=10),
        leftArm=dict(pitch=+44, bend=16),
    ),
    # 收势回 guard。
    20: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_cleave",
        description=(
            "P1 重制竖劈：anticipation 0→10 双手举剑过头（rightArm.pitch -62→-152）"
            "+拧腰后坐（torso.yaw -6→-20 / body.z -0.08），strike 10→16 过身下劈"
            "（pitch -152→+52）+前压（torso.pitch -8→+18）+前冲（body.z +0.30）"
            "+弓步（前腿 pitch -34 bend 30），hold 16→18 定格，recovery 18→20 回 guard。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
