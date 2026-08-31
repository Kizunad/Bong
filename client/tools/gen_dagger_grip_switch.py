#!/usr/bin/env python3
"""dagger_grip_switch —— 匕首转刀（换握）。

## v3：方向反了过来，而且不再是「原地转腕」

v2 是正握 → 反握、握把不挪窝的原地转腕。2026-08-31 用户手摆的两端把它改成了
**反握 → 正握、边抬边翻**：

| | 握把 | 刃向 | 肘 bend |
|---|---|---|---|
| t0 起手 | (−3.7, 13.1, −5.7) | 刃朝下偏前，−46.3° | 45.5° |
| t8 收势 | (−4.3, 21.5, −11.3) | 刃朝前偏上，+19.1° | 57.1° |

握把整段抬高 8.4px、前伸 5.6px，世界刃向转过 67°。

## 门禁跟着改（`knife_anim_gates`）

`gate_flip` 的两个数都得重标 —— 它原来假设「手不挪窝」。四条动画同口径实测：

    dagger_stab              握把行程  6.2px   刃向转过 26.7°   ← 完全没换握的参照
    dagger_grip_switch                11.3px            67.0°   ← 本条
    dagger_slash                      15.1px            87.1°   ← 一次完整挥砍的参照

下限取 55°（远高于「等于没换握」的 26.7，低于本设计的 67）、上限取 13px（卡在本条的
11.3 与「一次挥砍」的 15.1 之间）。两个数都夹在实测的坏值与好值之间，不是贴着天花板
设的 —— 转刀要是退化成挥一刀，这条会红。

## 过冲只能往前给，不能往上

用户手摆的末帧刀尖已经在 y=25.1，离「下巴线 26」只剩 0.9px，再抬一点就撞
`gate_torch`。第一版把 t6 写成仰角 +25° 的上抬，求解器被末帧拽回 +19.0 —— t6 和 t8
几乎一模一样，最后两 tick 成了死时间。改成沿刺出方向多送 1.5px（t6 握把 z=−12.8 →
t8 −11.3）再收回来，同时肘先弹开到 31° 再合回 57°：幅度小，但动作是活的。

## 中间三帧仍然反解，而且要压住「手在半路鼓出去」

这条的握把轨迹几乎是一条直线，靠 `straight_from` 把每一段的中间帧钉在两端连线上。
不加这条，欧拉插值会让手在半路鼓出去（实测鼓到 6.8px，而两端本来只隔 3.1px），
读成"挥了一下"而不是"转了个腕"。现在实测每段都贴着连线走。

## 8 tick 分段

    tick 0  用户手摆：反握低位，刃朝下（刀尖 y=5.0）
    tick 2  微沉 —— 先往下坐一点做反向蓄势（刀尖 y=3.6，仰角 −52.6°）
    tick 4  翻到一半 —— 刃过水平前（仰角 −18.5°），肘同时张开到 77.8°
    tick 5  刃转正朝前（+5.5°），拳继续上抬
    tick 6  到位并前送一寸（刀尖 y=25.0，握把 z=−12.8，肘弹开到 31°）
    tick 8  用户手摆的正握胸前架（收回 1.5px，肘合回 57°）

## 衔接说明（**已不成立，留作记录**）

v2 的两端焊死在 `dagger_stab` / `dagger_reverse_slash` 的架势帧上，`gate_chain` 逐轴
钉住。用户手摆之后，四条动画的起手式各摆各的，谁也不等于谁 —— 实测本条 t0 与
`dagger_reverse_slash` t0 最大差 138.4°（rightArm.pitch）、t8 与 `dagger_stab` t0
最大差 33.8°（rightArm.roll）。所以 `chain_links` 从 SUITE 里撤掉了，连招衔接处会有
一次跳变。**这是设计层的取舍，不是本次遗漏**：要恢复的话，得由人决定「以哪一条的
起手式为准」，再把其余几条的端点对齐过去。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 用户手摆的反握低位 —— 刃朝下（刀尖 y=5.0）
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=+36.7, yaw=-3.4, roll=+1.5, bend=+45.5, axis=180),
        leftArm=dict(pitch=+11.7, yaw=+27.6, roll=-17.3, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
    2: dict(  # 微沉 —— 先往下坐一点做反向蓄势
        easing="INQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+33.9, yaw=-11.5, roll=+4.3, bend=+37.0, axis=180),
        leftArm=dict(pitch=+9.0, yaw=+28.5, roll=-15.9, bend=+17.7, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
    ),
    4: dict(  # 翻到一半 —— 刃过水平前，肘同时张开到 77.8°
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+42.5, yaw=+6.8, roll=+8.1, bend=+77.8, axis=180),
        leftArm=dict(pitch=+4.7, yaw=+30.2, roll=-12.0, bend=+24.1, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
    ),
    5: dict(  # 刃转正朝前（+5.5°），拳继续上抬
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+18.2, yaw=+12.9, roll=+10.9, bend=+76.3, axis=180),
        leftArm=dict(pitch=+1.1, yaw=+31.3, roll=-10.0, bend=+25.8, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
    ),
    6: dict(  # 到位并前送一寸（握把 z=−12.8，肘弹开到 31°）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=-32.9, yaw=+19.5, roll=+25.5, bend=+31.1, axis=180),
        leftArm=dict(pitch=-2.3, yaw=+32.6, roll=-7.7, bend=+25.5, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
    ),
    8: dict(  # 用户手摆的正握胸前架（收回 1.5px，肘合回 57°）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-10.1, yaw=+19.4, roll=+24.6, bend=+57.1, axis=180),
        leftArm=dict(pitch=-7.1, yaw=+28.4, roll=-5.4, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
}

DESCRIPTION = (
    "v3 匕首转刀: 首末两帧人手摆（反握低位 → 正握胸前架），中间反解; 握把沿直线抬高 "
    "8.4px、前伸 5.6px，世界刃向转过 67°; t6 前送一寸 + 肘弹开 31° 再合回，末两 tick "
    "不留死时间。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_grip_switch",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
