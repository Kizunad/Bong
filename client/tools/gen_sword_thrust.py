#!/usr/bin/env python3
"""sword_thrust —— 收剑于腰侧后直刺、侧身送肩（P1 批次一重制）。

cast_ticks=10 → endTick ∈ [14,18]，取 16（借用方 sword_path.qi_slash cast=20
在 allowlist，16 ∉ [24,28] 维持 fail 不破坏棘轮）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→6   剑收腰侧 + 拧腰蓄势（easeOut 族 OUTSINE）
  strike       6→12  直刺全伸 + 侧身送肩 + body.z 前冲（easeIn 族 INQUAD），
                     发力顶点 = tick 10（cast 完成瞬间），hold 10→12 定格
  recovery     12→16 回中立 guard（INOUTSINE）
endTick=16，stopTick=18，非循环。主打击轴：rightArm.pitch / rightArm.bend /
torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

GUARD = dict(
    easing="INOUTSINE",
    # round 3：所有用到的 body 轴在首尾帧显式归位（防非循环残值偏移）。
    body=dict(x=+0.02, y=0.0, z=0.0),
    head=dict(yaw=-6),
    torso=dict(pitch=+3, yaw=+10),
    # round 2：guard 收肘（旧值 bend 35 手臂近乎伸直 z -9.4 僵尸手）。
    rightArm=dict(pitch=-64, yaw=-8, roll=+18, bend=65, axis=180),
    leftArm=dict(pitch=-52, yaw=+14, roll=-15, bend=55, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.07),
    rightLeg=dict(pitch=+8, bend=12, z=+0.05),
)

# 全伸刺击顶点（tick 10 = cast 完成瞬间）。
IMPACT = dict(
    easing="INQUAD",
    body=dict(x=-0.05, y=+0.02, z=+0.26),
    head=dict(yaw=+12),
    torso=dict(pitch=+6, yaw=-28),
    rightArm=dict(pitch=-94, yaw=-18, roll=+2, bend=2, axis=180),
    # round 2：后手拉回髋侧（hikite 反拉，y +9 髋高），旧值停在身前半空。
    leftArm=dict(pitch=-8, yaw=+26, roll=-8, bend=50, axis=180),
    leftLeg=dict(pitch=-30, bend=24, z=-0.13),
    rightLeg=dict(pitch=+22, bend=30, z=+0.09),
)

POSE = {
    0: GUARD,
    # 收剑中段：剑柄往腰侧回拉，躯干开始右拧。
    3: dict(
        easing="OUTSINE",
        body=dict(x=+0.04, z=-0.05),
        head=dict(yaw=-12),
        torso=dict(pitch=+5, yaw=+20),
        rightArm=dict(pitch=-20, yaw=-13, roll=+12, bend=68, axis=180),
        leftArm=dict(pitch=-58, yaw=+16, roll=-12, bend=50, axis=180),
        leftLeg=dict(pitch=-8, bend=13, z=-0.07),
        rightLeg=dict(pitch=+11, bend=16, z=+0.06),
    ),
    # 蓄势顶点：剑贴腰侧（bend 98 收紧）、torso.yaw +30 拧满，左手前引瞄准。
    6: dict(
        easing="OUTSINE",
        body=dict(x=+0.06, z=-0.09),
        head=dict(yaw=-16),
        torso=dict(pitch=+7, yaw=+30),
        # round 2：真收腰——pitch +16 肘拉到躯干后、手落髋侧（z -3.7），旧值
        # bend 98 手停在胸前（z -6.9）拉不到腰。
        rightArm=dict(pitch=+16, yaw=-14, roll=+10, bend=65, axis=180),
        leftArm=dict(pitch=-64, yaw=+18, roll=-12, bend=45, axis=180),
        leftLeg=dict(pitch=-6, bend=14, z=-0.06),
        rightLeg=dict(pitch=+13, bend=20, z=+0.07),
    ),
    # 出剑中段：加速前送。
    8: dict(
        easing="INQUAD",
        body=dict(x=0.0, z=+0.08),
        head=dict(yaw=+2),
        torso=dict(pitch=+6, yaw=+5),
        rightArm=dict(pitch=-60, yaw=-16, roll=+8, bend=45, axis=180),
        leftArm=dict(pitch=-45, yaw=+16, roll=-10, bend=55, axis=180),
        leftLeg=dict(pitch=-18, bend=18, z=-0.10),
        rightLeg=dict(pitch=+16, bend=24, z=+0.07),
    ),
    # 发力顶点 = cast 完成（tick 10）：手臂完全伸直（bend 2）、肩随 torso.yaw
    # -28 送出（58° 总扭矩）、body.z +0.26 前冲。
    10: IMPACT,
    # 打击定格（hold 2 tick）：刃尖微颤沉降。
    12: inherit(
        IMPACT,
        easing="INOUTSINE",
        body=dict(x=-0.05, y=+0.02, z=+0.25),
        rightArm=dict(pitch=-92, bend=4),
        leftArm=dict(pitch=-10, bend=52),
        leftLeg=dict(pitch=-29, bend=23),
        rightLeg=dict(bend=29),
    ),
    # 收剑中段。
    14: dict(
        easing="INOUTSINE",
        body=dict(x=-0.01, z=+0.10),
        head=dict(yaw=+2),
        torso=dict(pitch=+4, yaw=-8),
        rightArm=dict(pitch=-70, yaw=-12, roll=+8, bend=30, axis=180),
        leftArm=dict(pitch=-30, yaw=+20, roll=-10, bend=52, axis=180),
        leftLeg=dict(pitch=-18, bend=16, z=-0.10),
        rightLeg=dict(pitch=+14, bend=18, z=+0.07),
    ),
    16: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_thrust",
        description=(
            "P1 重制直刺：anticipation 0→6 收剑腰侧（pitch -64→+16 肘拉身后手落髋）+拧腰"
            "（torso.yaw +10→+30 / body.z -0.09），strike 6→10 直刺全伸"
            "（pitch -94 / bend 2）+侧身送肩（torso.yaw -28）+前冲（body.z +0.26），"
            "hold 10→12 定格，recovery 12→16 回 guard。"
        ),
        end_tick=16,
        stop_tick=18,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
