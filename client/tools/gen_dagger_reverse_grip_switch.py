#!/usr/bin/env python3
"""dagger_reverse_grip_switch —— 匕首换握归位（反握 → 正握）。

## 为什么要有这一条

`dagger_grip_switch` 把刀从正握翻到反握，但它是 one-shot emote：`stopTick` 之后
`rightItem` 随混出回到 0，刀自己转回正握。"换完握就一直反握"在单条 emote 里表达不了 ——
要么由一条保持性的持握状态层接手，要么**把回程也做成一招**。本条是后者：玩家按一次
换过去，再按一次换回来，握法状态由"当前播的是哪一条"承载，不需要额外的状态层。

## 它和正向那条是焊死的一对

    dagger_grip_switch          t0 正握低架  → t8 反握胸前架
    dagger_reverse_grip_switch  t0 反握胸前架 → t8 正握低架

两端逐轴相等（`gate_chain` 钉住），所以连招衔接处不跳。这也把用户 2026-08-31 手摆四条
首末帧之后丢掉的那条焊缝补回来了一段 —— 至少换握这一对内部是闭合的。

## 不是把正向那条倒放

倒放读起来是"录像回退"：预备、发力、收势的节奏全反了。本条自己排节奏（t2 反向预备、
t3 起转、t4 刃过横、t6 略过冲），只有**两端**取自正向那条。

## θ 越过 −180 要连续化

反向预备把 theta 送到 −192°，欧拉分解的主值在那里会跳：roll 从 −180 跳到 **+173**，
而 emote 是逐轴线性插值 —— 那一轴会朝反方向绕 353° 再转回来，渲出来是刀在半路猛地
翻一圈。所以三个角走 `anim_common.item_spin_series` 连续化，不逐帧各调各的 `item_spin`。

## 8 tick 分段

    tick 0  反握胸前架（θ=−180）—— 与 dagger_grip_switch 的收势逐轴相同
    tick 2  反向预备 —— 刃再往回压 12°（θ=−192），拳微沉
    tick 3  起转（θ=−162），easing 切 IN 起爆
    tick 4  刃过横（θ=−102）—— 与正向那条同一条走廊：刃指向玩家右外侧，不横穿身前
    tick 5  转过大半（θ=−40），拳同时沉到位
    tick 6  基本到位（θ=−8），拳略过冲
    tick 8  正握低架（θ=0）—— 与 dagger_grip_switch 的起手逐轴相同
"""

from anim_common import emit_json, item_spin_series

DAGGER_DISPLAY_ROT = (0.0, -90.0, 55.0)
BLADE_EDGE_AXIS = (1.0, 0.0, 0.0)

TICKS = (0, 2, 3, 4, 5, 6, 8)
THETAS = (-180.0, -192.0, -162.0, -102.0, -40.0, -8.0, 0.0)
GRIP = dict(zip(TICKS, item_spin_series(DAGGER_DISPLAY_ROT, BLADE_EDGE_AXIS, THETAS)))

POSE = {
    0: dict(  # 反握胸前架 —— 与 dagger_grip_switch 的 t8 逐轴相同
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-10.1, yaw=+19.4, roll=+24.6, bend=+57.1, axis=180),
        leftArm=dict(pitch=-7.1, yaw=+28.4, roll=-5.4, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
        rightItem=GRIP[0],
    ),
    2: dict(  # 反向预备：刃再往回压 12°，拳微沉
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+15.0),
        rightArm=dict(pitch=-13.4, yaw=+21.0, roll=+26.2, bend=+57.9, axis=180),
        leftArm=dict(pitch=-8.4, yaw=+28.5, roll=-4.6, bend=+20.4, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=GRIP[2],
    ),
    3: dict(  # 起转
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+15.0),
        rightArm=dict(pitch=-11.5, yaw=+20.1, roll=+25.3, bend=+57.4, axis=180),
        leftArm=dict(pitch=-7.7, yaw=+28.4, roll=-5.0, bend=+20.2, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=GRIP[3],
    ),
    4: dict(  # 刃过横：指向右外侧，与正向那条同一条走廊
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+6.8, yaw=+11.2, roll=+16.3, bend=+52.9, axis=180),
        leftArm=dict(pitch=-0.3, yaw=+28.1, roll=-9.7, bend=+18.2, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=GRIP[4],
    ),
    5: dict(  # 转过大半，拳沉到位
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+28.7, yaw=+0.5, roll=+5.4, bend=+47.5, axis=180),
        leftArm=dict(pitch=+8.5, yaw=+27.7, roll=-15.3, bend=+15.9, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=GRIP[5],
    ),
    6: dict(  # 基本到位，拳略过冲
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+42.8, yaw=-6.4, roll=-1.5, bend=+44.0, axis=180),
        leftArm=dict(pitch=+14.1, yaw=+27.5, roll=-18.8, bend=+14.4, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=GRIP[6],
    ),
    8: dict(  # 正握低架 —— 与 dagger_grip_switch 的 t0 逐轴相同
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=+36.7, yaw=-3.4, roll=+1.5, bend=+45.5, axis=180),
        leftArm=dict(pitch=+11.7, yaw=+27.6, roll=-17.3, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
        rightItem=GRIP[8],
    ),
}

DESCRIPTION = (
    "v1 匕首换握归位: 刀在手里绕刃口轴转回 180°（反握 → 正握），与 dagger_grip_switch "
    "两端逐轴焊死构成一对; t2 反向预备 12°、t4 刃过横指向右外侧、t6 略过冲; "
    "θ 越过 −180 走 item_spin_series 连续化，避免逐轴插值反绕一圈。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_reverse_grip_switch",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
