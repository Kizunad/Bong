#!/usr/bin/env python3
"""sword_path_condense_edge —— 凝锋：收剑入鞘式蓄意→拔剑亮刃定势（P2 批次二前半）。

cast_ticks=12，去复用（原借 sword_cleave）。对拍区间 endTick ∈ [16,20]，取 18。

母题：剑意凝于刃。居合式——先把剑收回左腰"入鞘位"蓄意（拧腰俯首视刃），
再一记快拔亮刃定势（剑指前引、昂首送刃），cast 完成瞬间 = 亮刃顶点。

时序（精度标准 #1/#2/#3）：
  anticipation 0→7   收剑入鞘：右手剑交叉收至左腰，拧腰 -20°、俯首视刃、
                     重心后坐下沉（easeOut 族 OUTSINE）
  strike       7→12  快拔亮刃：右臂扫出至前上定势（pitch -8→-85），躯干开至
                     +10°、重心升起前送（easeIn 族 INQUAD），顶点 = tick 12
  recovery     12→18 定势回 guard（INOUTSINE，t15 中段帧）
endTick=18，stopTick=20，非循环。主打击轴：rightArm.pitch / rightArm.yaw /
torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 单手剑 guard：右手主握剑前下位，左手剑指护于胸前，浅前后站架。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-3),
    torso=dict(pitch=+3, yaw=-5),
    rightArm=dict(pitch=-55, yaw=-8, roll=+15, bend=35, axis=180),
    leftArm=dict(pitch=-45, yaw=+20, roll=-10, bend=55, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.05),
    rightLeg=dict(pitch=+8, bend=10, z=+0.04),
)

# 亮刃定势顶点（tick 12 = cast 完成瞬间）：剑呈前上、左剑指前引、昂首送刃。
APEX = dict(
    easing="INQUAD",
    body=dict(y=+0.03, z=+0.10),
    head=dict(pitch=-8),
    torso=dict(pitch=-4, yaw=+10),
    rightArm=dict(pitch=-85, yaw=-14, roll=+8, bend=12, axis=180),
    leftArm=dict(pitch=-65, yaw=+8, roll=-12, bend=18, axis=180),
    leftLeg=dict(pitch=-18, bend=20, z=-0.08),
    rightLeg=dict(pitch=+14, bend=16, z=+0.06),
)

POSE = {
    0: GUARD,
    # 收剑中段：右手剑向左腰划入，躯干开始左拧、俯首。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.03, z=-0.05),
        head=dict(pitch=+6),
        torso=dict(pitch=+6, yaw=-14),
        rightArm=dict(pitch=-25, yaw=+25, roll=+30, bend=55, axis=180),
        leftArm=dict(pitch=-35, yaw=+30, roll=-8, bend=60, axis=180),
        leftLeg=dict(pitch=-12, bend=18, z=-0.06),
        rightLeg=dict(pitch=+10, bend=16, z=+0.05),
    ),
    # 入鞘位蓄意顶点：剑贴左腰，拧腰 -20°、俯首视刃、重心后坐最深。
    7: dict(
        easing="OUTSINE",
        body=dict(y=-0.06, z=-0.08),
        head=dict(pitch=+10),
        torso=dict(pitch=+9, yaw=-20),
        rightArm=dict(pitch=-8, yaw=+38, roll=+40, bend=68, axis=180),
        leftArm=dict(pitch=-30, yaw=+34, roll=-6, bend=66, axis=180),
        leftLeg=dict(pitch=-14, bend=22, z=-0.07),
        rightLeg=dict(pitch=+12, bend=20, z=+0.06),
    ),
    # 快拔中段：剑锋扫出过身前，躯干回正、重心上浮。
    10: dict(
        easing="INQUAD",
        body=dict(y=-0.01, z=+0.02),
        head=dict(pitch=0),
        torso=dict(pitch=+2, yaw=+2),
        rightArm=dict(pitch=-55, yaw=-5, roll=+20, bend=30, axis=180),
        leftArm=dict(pitch=-50, yaw=+12, roll=-10, bend=35, axis=180),
        leftLeg=dict(pitch=-10, bend=14, z=-0.05),
        rightLeg=dict(pitch=+8, bend=12, z=+0.04),
    ),
    # 亮刃定势 = cast 完成（tick 12）。
    12: APEX,
    # 收势中段：定势松开、剑回落一半。
    15: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=+0.05),
        head=dict(pitch=-5),
        torso=dict(pitch=0, yaw=+3),
        rightArm=dict(pitch=-70, yaw=-11, roll=+12, bend=24, axis=180),
        leftArm=dict(pitch=-55, yaw=+14, roll=-11, bend=36, axis=180),
        leftLeg=dict(pitch=-14, bend=16, z=-0.06),
        rightLeg=dict(pitch=+11, bend=13, z=+0.05),
    ),
    # 收势回 guard。
    18: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_path_condense_edge",
        description=(
            "P2 凝锋专属：anticipation 0→7 收剑入鞘式蓄意（rightArm 划至左腰 "
            "yaw +38 / torso.yaw -20 / 俯首 +10），strike 7→12 快拔亮刃定势"
            "（pitch -8→-85 / torso 开至 +10 / body.z +0.10），recovery 12→18 "
            "经 t15 中段帧回 guard。"
        ),
        end_tick=18,
        stop_tick=20,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
