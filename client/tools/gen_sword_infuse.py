#!/usr/bin/env python3
"""sword_infuse —— 注剑灌注：竖剑抚刃循环蓄力段（异兽脊骨剑垂直握姿口径重做）。

cast_ticks=40，真实引导窗（`cast_sword_infuse` 插 `Casting` + `PendingSwordInfuse`
两段式通道）。本条是**蓄力段**：isLoop 28t（id 沿用 `sword_infuse`——v1 资产清单
pin 该文件名）；release 段见 `gen_sword_infuse_release.py`。打断路径由 cast_emit 的
三打断分支表驱动 StopAnim。

母题：竖剑抚刃。右手把剑近乎竖直举在身前，左掌自护手上方沿刃身向剑尖推抚灌注真元
（去程 0→14），到位后收掌回护手（回程 14→28），随呼吸微沉浮、目光随掌走刃。

## 用户摆的是**抚刃行程的两端**，不是循环的首尾

用户在 Blockbench 里给了 t0 和 t28 两个**不同**的姿态：t0 剑近乎垂直、左掌搭在护手
上方（沿刃 +2.5px）；t28 剑略外倾、左掌推到刃身中段（沿刃 +10.4px）。

循环动画首尾必须逐轴同值——`anim_common._check_loop_closure` 直接挡，PlayerAnimator
那条"循环单帧衰减"的坑也要求闭环（`isLooped=true` 时只在 tick 0 放帧的 axis 会被插
值回 defaultValue）。首尾不同值的循环在引擎里就是每 1.4s 抽一下。

所以这两帧读作**推抚的近端与远端**：近端放 t0 / t28（闭环），远端放 t14，去程回程
各占一半。这就是"过渡算法调整一下"的实际内容。

## 闭环是机械保证的，不是靠对齐数值

t28 直接取 t0 的 dict 本体（`POSE[28] = dict(NEAR, easing=...)`），逐轴同值，改 NEAR
时两端一起动，不会分叉。

## 左掌够不到刃身远端

用户想推到沿刃 +10.4px，实际解到 +8.5px 就到臂长上限了（剑举得高，刃身中段已经在
左肩够得着的球面之外）。掌离刃轴 4.9~5.8px（贴着刃面推，不是穿进去）。

## 剑尖只能在半径 21~25.7px 的球面上

握姿是剑身⊥小臂（见 `gen_beast_spine_sword_player_anim`），剑尖离肩的距离几乎只由
肘弯决定。这条动画的 r 在 24.2~25.5 之间走，对应肘弯 8.9°~27.8°——举剑竖持本来就是
近乎伸臂的姿态。

时序：0→14 推抚去程，14→28 收掌回程。endTick=28，stopTick=30，isLoop=true。
不用 head/torso.roll（v1 资产清单边界复位断言域）。

两段式交接（`AnimCastTicksAlignmentTest.twoStageHandoffHoldsAcrossEveryReachableLoopPhase`）：
本段与 `sword_infuse_release` **必须声明同一组轴**——单侧声明的非中立轴会在交接瞬间
凭空跳变。腿的 `z=±0.03` 就是为此保留的（release 的 t0 也写着）。
"""

from anim_common import emit_json

# 抚刃近端 = 循环的首尾帧本体。改这里两端一起动，闭环不会分叉。
NEAR = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.02, z=-0.015),
    head=dict(pitch=+9, yaw=+4),
    torso=dict(pitch=+5, yaw=-6),
    rightArm=dict(pitch=-66.7, yaw=+1.4, roll=+4.3, bend=8.9, axis=180),
    leftArm=dict(pitch=-85.6, yaw=+59.9, roll=-1.0, bend=7.1, axis=180),
    rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
    leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
)

POSE = {
    # 0 / 28 = NEAR 本体：左掌搭在护手上方（沿刃 +1.8px），剑近乎竖直
    0: NEAR,
    4: dict(  # 推抚启动：掌离护手，沿刃外推一分
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.0275, z=-0.0171),
        head=dict(pitch=+9.6, yaw=+0.7),
        torso=dict(pitch=+5.6, yaw=-4.8),
        rightArm=dict(pitch=-56.6, yaw=-2.6, roll=+9.6, bend=16.3, axis=180),
        leftArm=dict(pitch=-89.7, yaw=+57.1, roll=+2.2, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    7: dict(  # 推抚中段：掌至刃身中部，身体渐沉
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.03375, z=-0.01885),
        head=dict(pitch=+10.1, yaw=-2.05),
        torso=dict(pitch=+6.1, yaw=-3.8),
        rightArm=dict(pitch=-49.2, yaw=-6.0, roll=+14.0, bend=21.7, axis=180),
        leftArm=dict(pitch=-92.8, yaw=+53.3, roll=+4.9, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    10: dict(  # 推抚远段：掌近臂展极限，剑随之外倾
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.04, z=-0.0206),
        head=dict(pitch=+10.6, yaw=-4.8),
        torso=dict(pitch=+6.6, yaw=-2.8),
        rightArm=dict(pitch=-43.3, yaw=-9.2, roll=+18.4, bend=25.7, axis=180),
        leftArm=dict(pitch=-96.0, yaw=+48.5, roll=+7.7, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    14: dict(  # 去程顶点 = 用户手摆的远端：掌沿刃 +8.5px，身体最沉，目光在掌
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.045, z=-0.022),
        head=dict(pitch=+11, yaw=-7),
        torso=dict(pitch=+7, yaw=-2),
        rightArm=dict(pitch=-40.0, yaw=-11.5, roll=+21.6, bend=27.8, axis=180),
        leftArm=dict(pitch=-98.7, yaw=+44.2, roll=+10.1, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    18: dict(  # 回程启动：掌离远端回撤
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.04, z=-0.0206),
        head=dict(pitch=+10.6, yaw=-4.8),
        torso=dict(pitch=+6.6, yaw=-2.8),
        rightArm=dict(pitch=-43.2, yaw=-9.0, roll=+18.6, bend=25.8, axis=180),
        leftArm=dict(pitch=-96.1, yaw=+48.4, roll=+7.8, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    21: dict(  # 回程中段：掌回刃身中部，身体回浮
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.03375, z=-0.01885),
        head=dict(pitch=+10.1, yaw=-2.05),
        torso=dict(pitch=+6.1, yaw=-3.8),
        rightArm=dict(pitch=-49.0, yaw=-5.6, roll=+14.3, bend=21.8, axis=180),
        leftArm=dict(pitch=-92.9, yaw=+53.2, roll=+5.0, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    24: dict(  # 回程近柄：掌将落回护手
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.0275, z=-0.0171),
        head=dict(pitch=+9.6, yaw=+0.7),
        torso=dict(pitch=+5.6, yaw=-4.8),
        rightArm=dict(pitch=-56.6, yaw=-2.1, roll=+10.0, bend=16.3, axis=180),
        leftArm=dict(pitch=-89.8, yaw=+57.2, roll=+2.4, bend=6.0, axis=180),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03, axis=0),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03, axis=0),
    ),
    # endTick = NEAR 本体：每轴与 tick 0 同值闭环（库坑 #1 的机械保证）
    28: NEAR,
}

DESCRIPTION = (
    "注剑灌注·竖剑抚刃 (sword_infuse): 28t 循环蓄力段。右手竖举剑于身前，"
    "左掌自护手沿刃身推抚至中段（去程 0->14）再收回护手（回程 14->28），"
    "随呼吸沉浮、目光随掌走刃。首尾取同一 NEAR 本体，逐轴闭环。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_infuse",
        description=DESCRIPTION,
        end_tick=28,
        stop_tick=30,
        is_loop=True,
        return_tick=0,
    )
