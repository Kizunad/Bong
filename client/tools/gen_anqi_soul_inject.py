#!/usr/bin/env python3
"""anqi_soul_inject —— 凝魂注射：单手举镖凝神灌注→刺送注入（P2 批次二前半）。

cast_ticks=20，去复用（原借 cast_invoke）。对拍区间 endTick ∈ [24,28]，取 26。

母题：凝魂灌注。右手把骨镖高举至面前凝神灌注（仰腕举镖、俯首注视、左手
结印护于胸口），灌满后一记向前下方的刺送把镖"注入"（俯身压送，非投掷），
cast 完成瞬间 = 注入顶点。与单射（鞭甩弹射）/齐射（开扇）动向完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→12  举镖灌注：右臂举至面前高位（pitch -55→-112）、注视凝神
                     （head -16）、左手胸口结印、重心微沉（easeOut 族 OUTSINE）
  strike       12→20 刺送注入：右臂向前下方压送（pitch -112→-68 / bend 98→8）、
                     俯身 +16、body.z 前送 +0.18（easeIn 族 INQUAD），
                     顶点 = tick 20（cast 完成瞬间）
  recovery     20→26 由注入位直起收回 guard（INOUTSINE，t23 中段帧）
endTick=26，stopTick=28，非循环。主打击轴：rightArm.pitch / rightArm.bend /
torso.pitch / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 持镖 guard：右手中位持镖、左手半护胸前。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-3),
    torso=dict(pitch=+3, yaw=-4),
    rightArm=dict(pitch=-55, yaw=-5, roll=+12, bend=55, axis=180),
    leftArm=dict(pitch=-45, yaw=+12, roll=-10, bend=60, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.05),
    rightLeg=dict(pitch=+8, bend=10, z=+0.04),
)

# 注入顶点（tick 20 = cast 完成瞬间）：右臂前下全伸展压送、俯身、弓步前压。
APEX = dict(
    easing="INQUAD",
    body=dict(y=+0.01, z=+0.18),
    head=dict(pitch=+6),
    torso=dict(pitch=+16, yaw=+10),
    rightArm=dict(pitch=-68, yaw=-10, roll=+2, bend=8, axis=180),
    leftArm=dict(pitch=-30, yaw=+14, roll=-20, bend=50, axis=180),
    leftLeg=dict(pitch=-24, bend=24, z=-0.11),
    rightLeg=dict(pitch=+18, bend=22, z=+0.07),
)

POSE = {
    0: GUARD,
    # 举镖起手：镖上提、目光跟随、左手向胸口收。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=-8),
        torso=dict(pitch=+1, yaw=-8),
        rightArm=dict(pitch=-85, yaw=+6, roll=+20, bend=80, axis=180),
        leftArm=dict(pitch=-52, yaw=+22, roll=-14, bend=75, axis=180),
        leftLeg=dict(pitch=-10, bend=14, z=-0.05),
        rightLeg=dict(pitch=+9, bend=12, z=+0.04),
    ),
    # 举镖高位：镖至面前、俯首注视开始灌注。
    # round 2：初版 bend 92/98 把手折到头顶后方，降 bend 让镖悬在面前视线上。
    8: dict(
        easing="OUTSINE",
        body=dict(y=-0.04, z=-0.05),
        head=dict(pitch=-13),
        torso=dict(pitch=-1, yaw=-11),
        rightArm=dict(pitch=-100, yaw=+10, roll=+24, bend=78, axis=180),
        leftArm=dict(pitch=-55, yaw=+26, roll=-15, bend=82, axis=180),
        leftLeg=dict(pitch=-11, bend=16, z=-0.06),
        rightLeg=dict(pitch=+9, bend=14, z=+0.05),
    ),
    # 灌注顶点：举镖面前最高位、凝神最深、左手结印贴胸。
    12: dict(
        easing="OUTSINE",
        body=dict(y=-0.055, z=-0.065),
        head=dict(pitch=-16),
        torso=dict(pitch=-2, yaw=-13),
        rightArm=dict(pitch=-108, yaw=+12, roll=+28, bend=84, axis=180),
        leftArm=dict(pitch=-58, yaw=+28, roll=-16, bend=86, axis=180),
        leftLeg=dict(pitch=-12, bend=18, z=-0.06),
        rightLeg=dict(pitch=+10, bend=16, z=+0.05),
    ),
    # 刺送启动：镖转向前、躯干开始前压。
    15: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=+0.03),
        head=dict(pitch=-6),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-95, yaw=+2, roll=+16, bend=55, axis=180),
        leftArm=dict(pitch=-48, yaw=+20, roll=-14, bend=70, axis=180),
        leftLeg=dict(pitch=-14, bend=18, z=-0.07),
        rightLeg=dict(pitch=+12, bend=16, z=+0.05),
    ),
    # 刺送中段：前下压送加速、左臂后拉抵消。
    18: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=+0.12),
        head=dict(pitch=+2),
        torso=dict(pitch=+12, yaw=+6),
        rightArm=dict(pitch=-75, yaw=-6, roll=+6, bend=20, axis=180),
        leftArm=dict(pitch=-35, yaw=+16, roll=-18, bend=55, axis=180),
        leftLeg=dict(pitch=-20, bend=20, z=-0.09),
        rightLeg=dict(pitch=+15, bend=18, z=+0.06),
    ),
    # 注入顶点 = cast 完成（tick 20）。
    20: APEX,
    # 收回中段：直起、镖手回撤。
    23: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=+0.09),
        head=dict(pitch=0),
        torso=dict(pitch=+9, yaw=+2),
        rightArm=dict(pitch=-62, yaw=-7, roll=+8, bend=32, axis=180),
        leftArm=dict(pitch=-38, yaw=+14, roll=-14, bend=55, axis=180),
        leftLeg=dict(pitch=-18, bend=18, z=-0.08),
        rightLeg=dict(pitch=+13, bend=15, z=+0.05),
    ),
    # 收势回 guard。
    26: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="anqi_soul_inject",
        description=(
            "P2 凝魂注射专属：anticipation 0→12 单手举镖凝神灌注（rightArm "
            "pitch -55→-112 举面前 / head -16 注视 / 左手胸口结印），strike "
            "12→20 刺送注入（pitch→-68 前下压送 / bend 98→8 / 俯身 torso +16 / "
            "body.z +0.18），recovery 20→26 经 t23 中段帧直起回 guard。"
        ),
        end_tick=26,
        stop_tick=28,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
